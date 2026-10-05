// SPDX-License-Identifier: GPL-3.0-or-later
//! Generic binary tagfile reader, version 3 (`havok-data.md` §3, algorithm §7.1).
//!
//! The reader is generic: class descriptors come from the file, members are found by name
//! and values are decoded by their declared type. [`Tagfile`] borrows the input; strings and
//! byte arrays are slices of it.

use std::collections::HashMap;
use std::sync::Arc;

use crate::error::{HavokError, Result};

/// The 8-byte tagfile magic (§3.1).
pub const MAGIC: [u8; 8] = [0x1E, 0x0D, 0xB0, 0xCA, 0xCE, 0xFA, 0x11, 0xD0];
/// The byte-swapped magic of a big-endian tagfile, which this reader rejects.
pub const MAGIC_SWAPPED: [u8; 8] = [0xCA, 0xB0, 0x0D, 0x1E, 0xD0, 0x11, 0xFA, 0xCE];
/// The only tagfile version this reader accepts.
pub const VERSION: i32 = 3;

/// Most classes one tagfile may declare (8 occur; resource limit).
pub const MAX_CLASSES: usize = 4096;
/// Longest full member list of a class (21 occur; resource limit, sizes the bitmap).
pub const MAX_FULL_MEMBERS: usize = 1024;
/// Deepest inheritance chain (3 occur; resource limit).
pub const MAX_INHERITANCE_DEPTH: usize = 64;
/// Most objects one tagfile may hold (2 occur; resource limit).
pub const MAX_OBJECTS: usize = 1 << 16;

/// Tag numbers of §3.3.
mod tag {
    pub const FILE_INFO: i32 = 1;
    pub const METADATA: i32 = 2;
    pub const OBJECT: i32 = 4;
    pub const END: i32 = 7;
}

/// Index of a string in the tagfile's pool (§3.4).
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub struct StrId(pub u32);

/// Basic kind of a member type code (§3.6, low 4 bits).
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum BasicKind {
    /// 0: no data.
    Void,
    /// 1: one raw byte.
    Byte,
    /// 2: varint.
    Int,
    /// 3: f32.
    Real,
    /// 4: 4 × f32.
    Vec4,
    /// 5: 8 × f32 (never occurs; rejected as unsupported).
    Vec8,
    /// 6: 12 × f32.
    Vec12,
    /// 7: 16 × f32 (never occurs; rejected as unsupported).
    Vec16,
    /// 8: object reference.
    Object,
    /// 9: embedded struct.
    Struct,
    /// 10: string.
    CString,
}

/// A validated member type code: basic kind plus array or tuple flag (§3.6).
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub struct TypeCode(u8);

impl TypeCode {
    /// Array flag.
    pub const ARRAY: u8 = 0x10;
    /// Tuple flag.
    pub const TUPLE: u8 = 0x20;

    /// Validates a raw code: kind 0–10, at most one of array and tuple, no bit above 0x3F.
    pub fn new(code: i32) -> Option<Self> {
        if !(0..=0x3F).contains(&code) {
            return None;
        }
        let c = code as u8;
        if c & Self::ARRAY != 0 && c & Self::TUPLE != 0 {
            return None;
        }
        if c & 0x0F > 10 {
            return None;
        }
        Some(TypeCode(c))
    }

    /// The raw code.
    pub fn code(self) -> u8 {
        self.0
    }

    /// The basic kind.
    pub fn kind(self) -> BasicKind {
        match self.0 & 0x0F {
            0 => BasicKind::Void,
            1 => BasicKind::Byte,
            2 => BasicKind::Int,
            3 => BasicKind::Real,
            4 => BasicKind::Vec4,
            5 => BasicKind::Vec8,
            6 => BasicKind::Vec12,
            7 => BasicKind::Vec16,
            8 => BasicKind::Object,
            9 => BasicKind::Struct,
            _ => BasicKind::CString,
        }
    }

    /// Whether the array flag is set.
    pub fn is_array(self) -> bool {
        self.0 & Self::ARRAY != 0
    }

    /// Whether the tuple flag is set.
    pub fn is_tuple(self) -> bool {
        self.0 & Self::TUPLE != 0
    }
}

/// A member descriptor (§3.5).
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Member {
    /// Member name.
    pub name: StrId,
    /// Type code.
    pub ty: TypeCode,
    /// Tuple size, when the tuple flag is set (0 otherwise).
    pub tuple_size: i32,
    /// Class name, for object and struct kinds.
    pub class_name: Option<StrId>,
}

/// A class descriptor (§3.5).
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Class {
    /// Class name (the built-in class 0 has the reserved empty string).
    pub name: StrId,
    /// Stored class version (informational).
    pub version: i32,
    /// Parent class index, 0 for none.
    pub parent: u32,
    /// Members declared by this class (not inherited ones).
    pub members: Vec<Member>,
    /// Length of the full member list (inherited members first).
    pub full_len: usize,
    /// Number of classes in the chain from the root to this class, inclusive.
    depth: usize,
}

/// A decoded value (§3.8). Arrays of scalar kinds are also the columns of struct arrays.
#[derive(Debug, Clone, PartialEq)]
pub enum Value<'a> {
    /// Scalar byte.
    Byte(u8),
    /// Scalar int.
    Int(i32),
    /// Scalar real.
    Real(f32),
    /// Scalar vec4.
    Vec4([f32; 4]),
    /// Scalar vec12.
    Vec12(Box<[f32; 12]>),
    /// Object reference: 0 = null, else an object index.
    Object(u32),
    /// String.
    CString(StrId),
    /// Byte array (or byte column): a slice of the input.
    Bytes(&'a [u8]),
    /// Int array (or column), with its width prefix.
    IntArray {
        /// The varint width prefix (4 in every observed array; informational).
        width: i32,
        /// The values.
        values: Vec<i32>,
    },
    /// Real array (or column).
    RealArray(Vec<f32>),
    /// Vec4 array (or column), missing components filled with 0.
    Vec4Array {
        /// The stored component count *k* (1–4).
        components: u8,
        /// The values.
        values: Vec<[f32; 4]>,
    },
    /// Vec12 array (or column).
    Vec12Array(Vec<[f32; 12]>),
    /// String array (or column).
    CStringArray(Vec<StrId>),
    /// Struct array, stored column by column.
    StructArray(StructArray<'a>),
}

/// A struct array (§3.9): element class, element count and the present columns.
#[derive(Debug, Clone, PartialEq)]
pub struct StructArray<'a> {
    /// Element class index.
    pub class: u32,
    /// Number of elements.
    pub len: usize,
    /// Present columns as (full member index of the element class, column values).
    pub columns: Vec<(usize, Value<'a>)>,
}

/// An object entry (§3.7).
#[derive(Debug, Clone, PartialEq)]
pub struct Object<'a> {
    /// Class index.
    pub class: u32,
    /// Present members as (full member index, value), in full-list order.
    pub values: Vec<(usize, Value<'a>)>,
}

/// A parsed tagfile: string pool, classes and objects.
#[derive(Debug, Clone, PartialEq)]
pub struct Tagfile<'a> {
    strings: Vec<&'a [u8]>,
    classes: Vec<Class>,
    objects: Vec<Object<'a>>,
}

/// Byte reader with tagfile error mapping.
struct Rd<'a> {
    data: &'a [u8],
    pos: usize,
}

impl<'a> Rd<'a> {
    fn remaining(&self) -> usize {
        self.data.len() - self.pos
    }

    fn bytes(&mut self, n: usize, what: &'static str) -> Result<&'a [u8]> {
        if n > self.remaining() {
            return Err(HavokError::Truncated {
                offset: self.pos,
                what,
            });
        }
        let s = &self.data[self.pos..self.pos + n];
        self.pos += n;
        Ok(s)
    }

    fn u8(&mut self, what: &'static str) -> Result<u8> {
        Ok(self.bytes(1, what)?[0])
    }

    fn f32(&mut self, what: &'static str) -> Result<f32> {
        let b = self.bytes(4, what)?;
        Ok(f32::from_le_bytes([b[0], b[1], b[2], b[3]]))
    }

    fn floats<const N: usize>(&mut self, what: &'static str) -> Result<[f32; N]> {
        let b = self.bytes(4 * N, what)?;
        let mut out = [0.0f32; N];
        for (o, &c) in out.iter_mut().zip(b.as_chunks::<4>().0) {
            *o = f32::from_le_bytes(c);
        }
        Ok(out)
    }

    /// Sign-magnitude varint of 1–5 bytes (§3.2).
    fn varint(&mut self, what: &'static str) -> Result<i32> {
        let start = self.pos;
        let b0 = self.u8(what)?;
        let negative = b0 & 1 != 0;
        let mut mag: u64 = u64::from((b0 >> 1) & 0x3F);
        let mut more = b0 & 0x80 != 0;
        let mut shift = 6u32;
        let mut len = 1;
        while more {
            if len == 5 {
                return Err(HavokError::BadVarint {
                    offset: start,
                    reason: "longer than 5 bytes",
                });
            }
            let b = self.u8(what)?;
            mag |= u64::from(b & 0x7F) << shift;
            more = b & 0x80 != 0;
            shift += 7;
            len += 1;
        }
        if mag > i32::MAX as u64 {
            return Err(HavokError::BadVarint {
                offset: start,
                reason: "magnitude out of i32 range",
            });
        }
        let v = mag as i32;
        Ok(if negative { -v } else { v })
    }

    /// A count: a non-negative varint, at most `remaining / min_elem` (bounded allocation).
    fn count(&mut self, min_elem: usize, what: &'static str) -> Result<usize> {
        let at = self.pos;
        let n = self.varint(what)?;
        if n < 0 {
            return Err(HavokError::BadVarint {
                offset: at,
                reason: "negative count",
            });
        }
        let n = n as usize;
        if n.saturating_mul(min_elem.max(1)) > self.remaining() {
            return Err(HavokError::Truncated { offset: at, what });
        }
        Ok(n)
    }
}

/// Reader state while parsing (§7.1).
struct Parser<'a> {
    rd: Rd<'a>,
    strings: Vec<&'a [u8]>,
    classes: Vec<Class>,
    objects: Vec<Object<'a>>,
    refs: Vec<i32>,
    /// Non-empty class name → index of the first class declared with it so far. Struct
    /// classes resolve by name at read time (§3.5); the index keeps that lookup O(1) so parse
    /// time stays linear in the input however many classes are declared.
    class_by_name: HashMap<&'a [u8], u32>,
}

impl<'a> Parser<'a> {
    fn string(&mut self, what: &'static str) -> Result<StrId> {
        let at = self.rd.pos;
        let n = self.rd.varint(what)?;
        if n > 0 {
            let s = self.rd.bytes(n as usize, what)?;
            let id = StrId(self.strings.len() as u32);
            self.strings.push(s);
            Ok(id)
        } else {
            let idx = n.unsigned_abs();
            if idx as usize >= self.strings.len() {
                return Err(HavokError::BadStringRef {
                    offset: at,
                    index: idx,
                    pool: self.strings.len(),
                });
            }
            Ok(StrId(idx))
        }
    }

    fn metadata(&mut self) -> Result<()> {
        let at = self.rd.pos;
        if self.classes.len() >= MAX_CLASSES {
            return Err(HavokError::UnsupportedForm {
                offset: at,
                what: "too many classes",
            });
        }
        let name = self.string("class name")?;
        let version = self.rd.varint("class version")?;
        let parent_at = self.rd.pos;
        let parent = self.rd.varint("parent index")?;
        if parent < 0 || parent as usize >= self.classes.len() {
            return Err(HavokError::BadClassIndex {
                offset: parent_at,
                index: i64::from(parent),
                what: "parent class",
            });
        }
        let parent = parent as u32;
        let (pfull, pdepth) = {
            let p = &self.classes[parent as usize];
            (p.full_len, p.depth)
        };
        let count = self.rd.count(2, "member count")?;
        if pfull + count > MAX_FULL_MEMBERS {
            return Err(HavokError::UnsupportedForm {
                offset: at,
                what: "full member list too long",
            });
        }
        let depth = pdepth + 1;
        if depth > MAX_INHERITANCE_DEPTH {
            return Err(HavokError::UnsupportedForm {
                offset: at,
                what: "inheritance too deep",
            });
        }
        let mut members = Vec::with_capacity(count);
        for _ in 0..count {
            let mname = self.string("member name")?;
            let ty_at = self.rd.pos;
            let code = self.rd.varint("member type")?;
            let ty = TypeCode::new(code).ok_or(HavokError::BadMemberType {
                offset: ty_at,
                code,
            })?;
            let tuple_size = if ty.is_tuple() {
                self.rd.varint("tuple size")?
            } else {
                0
            };
            let class_name = match ty.kind() {
                BasicKind::Object | BasicKind::Struct => Some(self.string("member class name")?),
                _ => None,
            };
            members.push(Member {
                name: mname,
                ty,
                tuple_size,
                class_name,
            });
        }
        let index = self.classes.len() as u32;
        let name_bytes = self.strings[name.0 as usize];
        if !name_bytes.is_empty() {
            // The first declaration wins, as a front-to-back scan would find it.
            self.class_by_name.entry(name_bytes).or_insert(index);
        }
        self.classes.push(Class {
            name,
            version,
            parent,
            members,
            full_len: pfull + count,
            depth,
        });
        Ok(())
    }

    /// Chain of class indices from the root class to `class` (inclusive), in a fixed buffer.
    fn chain(classes: &[Class], class: u32, buf: &mut [u32; MAX_INHERITANCE_DEPTH]) -> usize {
        let mut n = 0;
        let mut c = class;
        while c != 0 && n < MAX_INHERITANCE_DEPTH {
            buf[n] = c;
            n += 1;
            c = classes[c as usize].parent;
        }
        buf[..n].reverse();
        n
    }

    fn bitmap(&mut self, members: usize) -> Result<&'a [u8]> {
        self.rd.bytes(members.div_ceil(8), "presence bitmap")
    }

    /// Reads an object body of `class` (§3.7).
    fn body(&mut self, class: u32) -> Result<Vec<(usize, Value<'a>)>> {
        let full = self.classes[class as usize].full_len;
        let bitmap = self.bitmap(full)?;
        let mut buf = [0u32; MAX_INHERITANCE_DEPTH];
        let n = Self::chain(&self.classes, class, &mut buf);
        let mut values = Vec::new();
        let mut index = 0usize;
        for &c in &buf[..n] {
            let count = self.classes[c as usize].members.len();
            for m in 0..count {
                if bitmap[index / 8] & (1 << (index % 8)) != 0 {
                    let member = self.classes[c as usize].members[m].clone();
                    let v = self.value(&member)?;
                    values.push((index, v));
                }
                index += 1;
            }
        }
        Ok(values)
    }

    /// Resolves a struct class name to a class index, by name (§3.5).
    fn struct_class(&self, member: &Member, at: usize) -> Result<u32> {
        let Some(name) = member.class_name else {
            return Err(HavokError::UnsupportedForm {
                offset: at,
                what: "struct without class name",
            });
        };
        let name = self.strings[name.0 as usize];
        if name.is_empty() {
            return Err(HavokError::UnsupportedForm {
                offset: at,
                what: "struct with empty class name",
            });
        }
        // Class 0 has the empty name and is never indexed.
        self.class_by_name
            .get(name)
            .copied()
            .ok_or(HavokError::BadClassIndex {
                offset: at,
                index: -1,
                what: "struct class name",
            })
    }

    fn value(&mut self, m: &Member) -> Result<Value<'a>> {
        let at = self.rd.pos;
        let ty = m.ty;
        if ty.is_tuple() {
            return Err(HavokError::UnsupportedForm {
                offset: at,
                what: "tuple value",
            });
        }
        if !ty.is_array() {
            return Ok(match ty.kind() {
                BasicKind::Void => {
                    return Err(HavokError::UnsupportedForm {
                        offset: at,
                        what: "void member present",
                    });
                }
                BasicKind::Byte => Value::Byte(self.rd.u8("byte value")?),
                BasicKind::Int => Value::Int(self.rd.varint("int value")?),
                BasicKind::Real => Value::Real(self.rd.f32("real value")?),
                BasicKind::Vec4 => Value::Vec4(self.rd.floats::<4>("vec4 value")?),
                BasicKind::Vec12 => Value::Vec12(Box::new(self.rd.floats::<12>("vec12 value")?)),
                BasicKind::Vec8 | BasicKind::Vec16 => {
                    return Err(HavokError::UnsupportedForm {
                        offset: at,
                        what: "vec8 or vec16 value",
                    });
                }
                BasicKind::Object => {
                    let r = self.rd.varint("object reference")?;
                    if r < 0 {
                        return Err(HavokError::UnresolvedRef { index: r });
                    }
                    if r != 0 {
                        self.refs.push(r);
                    }
                    Value::Object(r as u32)
                }
                BasicKind::Struct => {
                    return Err(HavokError::UnsupportedForm {
                        offset: at,
                        what: "scalar struct value",
                    });
                }
                BasicKind::CString => Value::CString(self.string("string value")?),
            });
        }
        match ty.kind() {
            BasicKind::Struct => self.struct_array(m, at),
            BasicKind::Byte => {
                let n = self.rd.count(1, "byte array")?;
                Ok(Value::Bytes(self.rd.bytes(n, "byte array")?))
            }
            BasicKind::Int
            | BasicKind::Real
            | BasicKind::Vec4
            | BasicKind::Vec12
            | BasicKind::CString => {
                let min = match ty.kind() {
                    BasicKind::Int | BasicKind::CString => 1,
                    BasicKind::Real => 4,
                    // The vec4 element size depends on the prefix; checked again there.
                    BasicKind::Vec4 => 1,
                    _ => 48,
                };
                let n = self.rd.count(min, "array count")?;
                self.column(ty.kind(), n, at)
            }
            BasicKind::Void | BasicKind::Vec8 | BasicKind::Vec16 | BasicKind::Object => {
                Err(HavokError::UnsupportedForm {
                    offset: at,
                    what: "array of void, vec8, vec16 or object",
                })
            }
        }
    }

    /// Reads `n` values of a scalar kind laid out as an array without its count (§3.8, §3.9).
    fn column(&mut self, kind: BasicKind, n: usize, at: usize) -> Result<Value<'a>> {
        let too_short = |rd: &Rd<'_>, need: usize, what| -> Result<()> {
            if need > rd.remaining() {
                Err(HavokError::Truncated {
                    offset: rd.pos,
                    what,
                })
            } else {
                Ok(())
            }
        };
        match kind {
            BasicKind::Byte => {
                too_short(&self.rd, n, "byte column")?;
                Ok(Value::Bytes(self.rd.bytes(n, "byte column")?))
            }
            BasicKind::Int => {
                let width = self.rd.varint("int array width prefix")?;
                too_short(&self.rd, n, "int array")?;
                let mut values = Vec::with_capacity(n);
                for _ in 0..n {
                    values.push(self.rd.varint("int array element")?);
                }
                Ok(Value::IntArray { width, values })
            }
            BasicKind::Real => {
                too_short(&self.rd, n.saturating_mul(4), "real array")?;
                let mut values = Vec::with_capacity(n);
                for _ in 0..n {
                    values.push(self.rd.f32("real array element")?);
                }
                Ok(Value::RealArray(values))
            }
            BasicKind::Vec4 => {
                let k_at = self.rd.pos;
                let k = self.rd.varint("vec4 component count")?;
                if !(1..=4).contains(&k) {
                    return Err(HavokError::UnsupportedForm {
                        offset: k_at,
                        what: "vec4 array component count outside 1-4",
                    });
                }
                let k = k as usize;
                too_short(&self.rd, n.saturating_mul(4 * k), "vec4 array")?;
                let mut values = Vec::with_capacity(n);
                for _ in 0..n {
                    let mut v = [0.0f32; 4];
                    for c in v.iter_mut().take(k) {
                        *c = self.rd.f32("vec4 array element")?;
                    }
                    values.push(v);
                }
                Ok(Value::Vec4Array {
                    components: k as u8,
                    values,
                })
            }
            BasicKind::Vec12 => {
                too_short(&self.rd, n.saturating_mul(48), "vec12 array")?;
                let mut values = Vec::with_capacity(n);
                for _ in 0..n {
                    values.push(self.rd.floats::<12>("vec12 array element")?);
                }
                Ok(Value::Vec12Array(values))
            }
            BasicKind::CString => {
                too_short(&self.rd, n, "string array")?;
                let mut values = Vec::with_capacity(n);
                for _ in 0..n {
                    values.push(self.string("string array element")?);
                }
                Ok(Value::CStringArray(values))
            }
            _ => Err(HavokError::UnsupportedForm {
                offset: at,
                what: "column of void, vec8, vec16, object or struct",
            }),
        }
    }

    /// Struct array, column-wise (§3.9).
    fn struct_array(&mut self, m: &Member, at: usize) -> Result<Value<'a>> {
        let n = self.rd.count(1, "struct array count")?;
        let class = self.struct_class(m, at)?;
        let full = self.classes[class as usize].full_len;
        let bitmap = self.bitmap(full)?;
        let mut buf = [0u32; MAX_INHERITANCE_DEPTH];
        let chain_len = Self::chain(&self.classes, class, &mut buf);
        let mut columns = Vec::new();
        let mut index = 0usize;
        for &c in &buf[..chain_len] {
            let count = self.classes[c as usize].members.len();
            for mi in 0..count {
                if bitmap[index / 8] & (1 << (index % 8)) != 0 {
                    let col_at = self.rd.pos;
                    let ty = self.classes[c as usize].members[mi].ty;
                    if ty.is_array() || ty.is_tuple() {
                        return Err(HavokError::UnsupportedForm {
                            offset: col_at,
                            what: "array or tuple column in a struct array",
                        });
                    }
                    if ty.kind() == BasicKind::Void {
                        return Err(HavokError::UnsupportedForm {
                            offset: col_at,
                            what: "void member present",
                        });
                    }
                    let v = self.column(ty.kind(), n, col_at)?;
                    columns.push((index, v));
                }
                index += 1;
            }
        }
        Ok(Value::StructArray(StructArray {
            class,
            len: n,
            columns,
        }))
    }

    fn object(&mut self) -> Result<()> {
        let at = self.rd.pos;
        if self.objects.len() >= MAX_OBJECTS {
            return Err(HavokError::UnsupportedForm {
                offset: at,
                what: "too many objects",
            });
        }
        let class = self.rd.varint("object class index")?;
        if class < 1 || class as usize >= self.classes.len() {
            return Err(HavokError::BadClassIndex {
                offset: at,
                index: i64::from(class),
                what: "object class",
            });
        }
        let class = class as u32;
        let values = self.body(class)?;
        self.objects.push(Object { class, values });
        Ok(())
    }
}

impl<'a> Tagfile<'a> {
    /// Parses a whole binary tagfile (§7.1). The input must end exactly at the end tag.
    pub fn parse(data: &'a [u8]) -> Result<Self> {
        if data.len() < MAGIC.len() {
            if MAGIC.starts_with(data) {
                return Err(HavokError::Truncated {
                    offset: 0,
                    what: "magic",
                });
            }
            return Err(HavokError::BadMagic { swapped: false });
        }
        if data[..8] == MAGIC_SWAPPED {
            return Err(HavokError::BadMagic { swapped: true });
        }
        if data[..8] != MAGIC {
            return Err(HavokError::BadMagic { swapped: false });
        }
        let mut p = Parser {
            rd: Rd { data, pos: 8 },
            strings: vec![&[], &[]],
            classes: vec![Class {
                name: StrId(0),
                version: 0,
                parent: 0,
                members: Vec::new(),
                full_len: 0,
                depth: 0,
            }],
            objects: Vec::new(),
            refs: Vec::new(),
            class_by_name: HashMap::new(),
        };
        let t_at = p.rd.pos;
        let first = p.rd.varint("tag")?;
        if first != tag::FILE_INFO {
            return Err(HavokError::BadVersion {
                reason: format!("first entry has tag {first} at offset {t_at}, not file info")
                    .into(),
            });
        }
        let version = p.rd.varint("file version")?;
        if version != VERSION {
            return Err(HavokError::BadVersion {
                reason: format!("version {version}, expected {VERSION}").into(),
            });
        }
        loop {
            let at = p.rd.pos;
            let t = p.rd.varint("tag")?;
            match t {
                tag::FILE_INFO => {
                    return Err(HavokError::BadVersion {
                        reason: format!("repeated file info at offset {at}").into(),
                    });
                }
                tag::METADATA => p.metadata()?,
                tag::OBJECT => p.object()?,
                tag::END => break,
                other => {
                    return Err(HavokError::UnknownTag {
                        offset: at,
                        tag: other,
                    });
                }
            }
        }
        if p.rd.remaining() != 0 {
            return Err(HavokError::TagfileTrailingBytes {
                offset: p.rd.pos,
                count: p.rd.remaining(),
            });
        }
        if let Some(&bad) = p.refs.iter().find(|&&r| r as usize > p.objects.len()) {
            return Err(HavokError::UnresolvedRef { index: bad });
        }
        tracing::trace!(
            classes = p.classes.len() - 1,
            objects = p.objects.len(),
            "tagfile parsed"
        );
        Ok(Tagfile {
            strings: p.strings,
            classes: p.classes,
            objects: p.objects,
        })
    }

    /// Bytes of a pool string.
    pub fn string(&self, id: StrId) -> &'a [u8] {
        self.strings.get(id.0 as usize).copied().unwrap_or(&[])
    }

    /// Declared classes; index 0 is the built-in "none" class.
    pub fn classes(&self) -> &[Class] {
        &self.classes
    }

    /// Objects in file order; object index *i* is `objects()[i - 1]`.
    pub fn objects(&self) -> &[Object<'a>] {
        &self.objects
    }

    /// The object with index `index` (1-based, §3.7).
    pub fn object(&self, index: u32) -> Option<&Object<'a>> {
        index
            .checked_sub(1)
            .and_then(|i| self.objects.get(i as usize))
    }

    /// The root object (object 1).
    pub fn root(&self) -> Option<&Object<'a>> {
        self.object(1)
    }

    /// Name of class `class`.
    pub fn class_name(&self, class: u32) -> &'a [u8] {
        self.classes
            .get(class as usize)
            .map_or(&[][..], |c| self.string(c.name))
    }

    /// Finds the member called `name` in the full member list of `class`, returning its full
    /// index and descriptor (§3.11).
    pub fn find_member(&self, class: u32, name: &[u8]) -> Option<(usize, &Member)> {
        if class as usize >= self.classes.len() {
            return None;
        }
        let mut buf = [0u32; MAX_INHERITANCE_DEPTH];
        let n = Parser::chain(&self.classes, class, &mut buf);
        let mut index = 0;
        for &c in &buf[..n] {
            for m in &self.classes[c as usize].members {
                if self.string(m.name) == name {
                    return Some((index, m));
                }
                index += 1;
            }
        }
        None
    }

    /// Looks up a member of an object or struct array by name and checks its declared type.
    ///
    /// Returns `Ok(None)` when the class does not declare the member or the member is
    /// absent (the caller substitutes the default, §3.10), and
    /// [`HavokError::TypeMismatch`] when it is declared with another type code.
    pub(crate) fn typed<'v>(
        &self,
        class: u32,
        values: &'v [(usize, Value<'a>)],
        class_label: &'static str,
        name: &'static str,
        code: u8,
    ) -> Result<Option<&'v Value<'a>>> {
        let Some((index, m)) = self.find_member(class, name.as_bytes()) else {
            return Ok(None);
        };
        if m.ty.code() != code {
            return Err(HavokError::TypeMismatch {
                class: class_label,
                member: name,
                expected: code,
                found: m.ty.code(),
            });
        }
        Ok(values.iter().find(|(i, _)| *i == index).map(|(_, v)| v))
    }
}

/// Owned copies of pool strings, made at most once per pool entry. A string column may name
/// one long pool string from many 1-byte references (§3.4); sharing the copy keeps a decoded
/// model's size linear in its input.
#[derive(Debug, Default)]
pub(crate) struct SharedStrings(HashMap<StrId, Arc<[u8]>>);

impl SharedStrings {
    /// The shared copy of pool string `id` of `tf`.
    pub(crate) fn get(&mut self, tf: &Tagfile<'_>, id: StrId) -> Arc<[u8]> {
        Arc::clone(self.0.entry(id).or_insert_with(|| Arc::from(tf.string(id))))
    }
}

/// Decodes stored string bytes as Latin-1 (§3.4).
pub fn latin1(bytes: &[u8]) -> String {
    bytes.iter().map(|&b| char::from(b)).collect()
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::testkit::{MemberDef, TagfileWriter, WValue};

    #[test]
    fn varints_round_trip_through_an_int_array() {
        let values = vec![
            0,
            1,
            -1,
            -2,
            36,
            81,
            256,
            13_200,
            1 << 20,
            i32::MAX,
            -i32::MAX,
        ];
        let mut w = TagfileWriter::new();
        let a = w.class(
            "A",
            0,
            0,
            &[MemberDef::new("v", 18), MemberDef::new("s", 10)],
        );
        w.object(
            a,
            &[
                ("v".into(), WValue::IntArray(values.clone())),
                ("s".into(), WValue::CString("A".into())),
            ],
        );
        let bytes = w.finish();
        let tf = Tagfile::parse(&bytes).unwrap();
        let root = tf.root().unwrap();
        assert_eq!(
            root.values[0].1,
            Value::IntArray {
                width: 4,
                values: values.clone()
            }
        );
        // The string value is a reference to the class name's pool entry.
        let Value::CString(id) = root.values[1].1 else {
            panic!()
        };
        assert_eq!(tf.string(id), b"A");
        assert_eq!(tf.class_name(1), b"A");
        assert_eq!(tf.find_member(1, b"s").map(|(i, _)| i), Some(1));
        assert!(tf.find_member(1, b"x").is_none());
        assert!(tf.find_member(9, b"s").is_none());
    }

    #[test]
    fn member_names_are_bytes() {
        // Regression (fuzzing): names are matched as stored bytes, not re-encoded text.
        let mut w = TagfileWriter::new();
        let a = w.class("A", 0, 0, &[MemberDef::new("\u{e9}t\u{e9}", 2)]);
        w.object(
            a,
            &[
                ("\u{e9}t\u{e9}".into(), WValue::Int(7)),
                ("unknown".into(), WValue::Int(1)),
            ],
        );
        let bytes = w.finish();
        let tf = Tagfile::parse(&bytes).unwrap();
        let (i, m) = tf.find_member(1, "\u{e9}t\u{e9}".as_bytes()).unwrap();
        assert_eq!((i, m.ty.code()), (0, 2));
        assert_eq!(tf.root().unwrap().values, [(0, Value::Int(7))]);
    }

    #[test]
    fn type_codes() {
        assert!(TypeCode::new(0x2A).is_some());
        assert!(TypeCode::new(0x3A).is_none());
        assert!(TypeCode::new(0x30).is_none());
        assert!(TypeCode::new(0x0B).is_none());
        assert!(TypeCode::new(0x40).is_none());
        assert!(TypeCode::new(-1).is_none());
        let t = TypeCode::new(0x19).unwrap();
        assert_eq!(
            (t.kind(), t.is_array(), t.is_tuple()),
            (BasicKind::Struct, true, false)
        );
    }
}
