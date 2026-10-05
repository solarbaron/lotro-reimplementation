// SPDX-License-Identifier: GPL-3.0-or-later
//! Master property dictionary (DID 0x34000000), property types and the raw property value
//! tree.
//!
//! Spec: `docs/specs/property-system.md` §2 (dictionary), §3 (type codes), §4 (value
//! encodings). Property-set records are in [`crate::property_set`].

use std::collections::HashMap;
use std::fmt;

use crate::did::{Did, DidKind};
use crate::error::{FormatError, Result};
use crate::reader::{
    PREALLOC_CAP, Reader, TSize, hash_index_bytes, index_u32, keys_in_bucket_order,
};
use crate::record::{Record, read_prologue};
use crate::string_info::StringInfo;

/// Marker that follows the optional values of every definition (f32 1.5).
pub const DEFINITION_MARKER: u32 = 0x3FC0_0000;

/// Flag bit: a default value is stored.
pub const FLAG_HAS_DEFAULT: u32 = 0x800;
/// Flag bit: a minimum value is stored.
pub const FLAG_HAS_MIN: u32 = 0x1000;
/// Flag bit: a maximum value is stored.
pub const FLAG_HAS_MAX: u32 = 0x2000;

/// Property value type (`property-system.md` §3). Names are descriptive.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum PropertyType {
    /// 1
    String,
    /// 2
    StringToken,
    /// 3
    Waveform,
    /// 4
    Timestamp,
    /// 5
    TriState,
    /// 6
    Vector3,
    /// 7
    InstanceId,
    /// 8
    Enum,
    /// 9
    Float,
    /// 10
    PropertyId,
    /// 11
    Struct,
    /// 12
    Array,
    /// 13
    StringInfo,
    /// 14
    Bitfield64,
    /// 15
    Int,
    /// 16
    Colour,
    /// 17
    Position,
    /// 18
    Bitfield32,
    /// 19
    Int64,
    /// 20
    DidRef,
    /// 21
    Bool,
    /// 22
    BitfieldAny,
    /// Any other code.
    Unknown(u32),
}

impl PropertyType {
    /// Maps a stored type code.
    #[must_use]
    pub fn from_code(code: u32) -> Self {
        match code {
            1 => Self::String,
            2 => Self::StringToken,
            3 => Self::Waveform,
            4 => Self::Timestamp,
            5 => Self::TriState,
            6 => Self::Vector3,
            7 => Self::InstanceId,
            8 => Self::Enum,
            9 => Self::Float,
            10 => Self::PropertyId,
            11 => Self::Struct,
            12 => Self::Array,
            13 => Self::StringInfo,
            14 => Self::Bitfield64,
            15 => Self::Int,
            16 => Self::Colour,
            17 => Self::Position,
            18 => Self::Bitfield32,
            19 => Self::Int64,
            20 => Self::DidRef,
            21 => Self::Bool,
            22 => Self::BitfieldAny,
            other => Self::Unknown(other),
        }
    }

    /// The stored type code.
    #[must_use]
    pub fn code(self) -> u32 {
        match self {
            Self::String => 1,
            Self::StringToken => 2,
            Self::Waveform => 3,
            Self::Timestamp => 4,
            Self::TriState => 5,
            Self::Vector3 => 6,
            Self::InstanceId => 7,
            Self::Enum => 8,
            Self::Float => 9,
            Self::PropertyId => 10,
            Self::Struct => 11,
            Self::Array => 12,
            Self::StringInfo => 13,
            Self::Bitfield64 => 14,
            Self::Int => 15,
            Self::Colour => 16,
            Self::Position => 17,
            Self::Bitfield32 => 18,
            Self::Int64 => 19,
            Self::DidRef => 20,
            Self::Bool => 21,
            Self::BitfieldAny => 22,
            Self::Unknown(c) => c,
        }
    }

    /// Whether definitions of this type never store default/min/max values
    /// (types 10, 11, 12; `property-system.md` §2.2).
    #[must_use]
    pub fn skips_definition_values(self) -> bool {
        matches!(self, Self::PropertyId | Self::Struct | Self::Array)
    }
}

/// Deepest nesting level a property value may sit at: top-level values are depth 0, a
/// struct field or array element of a depth-*d* value is depth *d* + 1, and a value at
/// depth `PROPERTY_MAX_DEPTH` or deeper is rejected. Observed data reaches depth 9
/// (`property-system.md` §5.3 suggests 32).
pub const PROPERTY_MAX_DEPTH: usize = 32;

/// Largest array element count accepted, whatever the remaining input
/// (`property-system.md` §5.3; the largest observed array has 5,211 elements).
pub const PROPERTY_MAX_ARRAY_LEN: u32 = 1_000_000;

/// Smallest encoding of one property entry: a null entry is a bare zero PID
/// (`property-system.md` §5.2). Used to bound counts read from the data.
const MIN_ENTRY_SIZE: usize = 4;

/// Largest initial capacity of a struct's or array's entry list (observed structs have at
/// most 62 fields; most arrays have 1–3 elements). Larger lists grow as entries decode, in
/// charged steps that never take the capacity past the stored count.
const NESTED_PREALLOC_CAP: usize = 64;

/// Type information a property-set decoder needs: the value type of each PID.
///
/// Decoding is dictionary-driven (`property-system.md` §1): a value cannot be skipped
/// without knowing its type. [`PropertyDictionary`] implements this trait; so do hash and
/// B-tree maps from PID to [`PropertyType`], closures `Fn(u32) -> Option<PropertyType>`
/// (handy for fuzzing and tests) and [`NoSchema`].
pub trait PropertySchema {
    /// The value type of `pid`, or `None` if the PID is not defined.
    fn property_type(&self, pid: u32) -> Option<PropertyType>;
}

impl PropertySchema for PropertyDictionary {
    fn property_type(&self, pid: u32) -> Option<PropertyType> {
        self.definition(pid).map(|d| d.ty)
    }
}

impl<S: std::hash::BuildHasher> PropertySchema for HashMap<u32, PropertyType, S> {
    fn property_type(&self, pid: u32) -> Option<PropertyType> {
        self.get(&pid).copied()
    }
}

impl PropertySchema for std::collections::BTreeMap<u32, PropertyType> {
    fn property_type(&self, pid: u32) -> Option<PropertyType> {
        self.get(&pid).copied()
    }
}

impl<F: Fn(u32) -> Option<PropertyType>> PropertySchema for F {
    fn property_type(&self, pid: u32) -> Option<PropertyType> {
        self(pid)
    }
}

/// Name → PID lookup, for code that finds properties by their dictionary names
/// (`property-system.md` §2; names are identifiers stored in the data).
///
/// [`PropertyDictionary`] implements it; so do maps from names to PIDs and closures
/// `Fn(&str) -> Option<u32>` (for tests and fuzzing).
pub trait PropertyNames {
    /// The PID named `name`, if defined.
    fn pid_of(&self, name: &str) -> Option<u32>;
}

impl PropertyNames for PropertyDictionary {
    fn pid_of(&self, name: &str) -> Option<u32> {
        self.pid(name)
    }
}

impl<S: std::hash::BuildHasher> PropertyNames for HashMap<String, u32, S> {
    fn pid_of(&self, name: &str) -> Option<u32> {
        self.get(name).copied()
    }
}

impl PropertyNames for std::collections::BTreeMap<String, u32> {
    fn pid_of(&self, name: &str) -> Option<u32> {
        self.get(name).copied()
    }
}

impl<F: Fn(&str) -> Option<u32>> PropertyNames for F {
    fn pid_of(&self, name: &str) -> Option<u32> {
        self(name)
    }
}

/// A schema that defines no PID. Scalars decode with it; a non-empty struct or array
/// fails with [`FormatError::UnknownProperty`].
#[derive(Debug, Clone, Copy, Default, PartialEq, Eq)]
pub struct NoSchema;

impl PropertySchema for NoSchema {
    fn property_type(&self, _pid: u32) -> Option<PropertyType> {
        None
    }
}

/// Extra data of a subtype-10 waveform.
#[derive(Debug, Clone, PartialEq)]
pub struct WaveformExtra {
    /// The f32 after the ten base values.
    pub scalar: f32,
    /// The bool8 after it.
    pub flag: bool,
    /// The `k` pairs of f32.
    pub pairs: Vec<(f32, f32)>,
}

/// An animated scalar (type 3, `property-system.md` §4.2). Subtype meanings are OPEN.
#[derive(Debug, Clone, PartialEq)]
pub struct Waveform {
    /// Subtype selecting the layout.
    pub subtype: u32,
    /// Base values: 0 (subtype 0), 1 (subtype 1) or 10 floats (any other subtype).
    pub values: Vec<f32>,
    /// Extra data, present only for subtype 10.
    pub extra: Option<WaveformExtra>,
}

/// A position value (type 17, `property-system.md` §4.6).
///
/// Each payload bit of `flags` adds one field, in ascending bit order. Bits 0x40 and 0x80
/// carry no payload; their meaning is OPEN, so they are kept only in [`Position::flags`].
///
/// Stored inline in [`PropertyValue`] (36 bytes): absent fields are kept as zero and the
/// accessors return `None` for them, so a dense array of positions costs no extra heap
/// allocation per element.
#[derive(Debug, Clone, Copy, Default, PartialEq)]
pub struct Position {
    offset: [f32; 3],
    orientation: [f32; 4],
    instance: u16,
    cell: u16,
    landblock: [u8; 2],
    region: u8,
    flags: u8,
}

impl Position {
    /// Flag bit: region present.
    pub const REGION: u8 = 0x01;
    /// Flag bit: landblock X, Y present.
    pub const LANDBLOCK: u8 = 0x02;
    /// Flag bit: instance present.
    pub const INSTANCE: u8 = 0x04;
    /// Flag bit: cell present.
    pub const CELL: u8 = 0x08;
    /// Flag bit: x, y, z present.
    pub const OFFSET: u8 = 0x10;
    /// Flag bit: orientation present.
    pub const ORIENTATION: u8 = 0x20;

    /// A position with the given flag byte and every payload field zero. Use the `with_*`
    /// methods to fill in fields.
    #[must_use]
    pub fn new(flags: u8) -> Self {
        Self {
            flags,
            ..Self::default()
        }
    }

    /// Sets the region and its flag bit.
    #[must_use]
    pub fn with_region(mut self, region: u8) -> Self {
        self.flags |= Self::REGION;
        self.region = region;
        self
    }

    /// Sets the landblock X, Y and its flag bit.
    #[must_use]
    pub fn with_landblock(mut self, x: u8, y: u8) -> Self {
        self.flags |= Self::LANDBLOCK;
        self.landblock = [x, y];
        self
    }

    /// Sets the instance and its flag bit.
    #[must_use]
    pub fn with_instance(mut self, instance: u16) -> Self {
        self.flags |= Self::INSTANCE;
        self.instance = instance;
        self
    }

    /// Sets the cell and its flag bit.
    #[must_use]
    pub fn with_cell(mut self, cell: u16) -> Self {
        self.flags |= Self::CELL;
        self.cell = cell;
        self
    }

    /// Sets x, y, z and its flag bit.
    #[must_use]
    pub fn with_offset(mut self, offset: [f32; 3]) -> Self {
        self.flags |= Self::OFFSET;
        self.offset = offset;
        self
    }

    /// Sets the orientation and its flag bit.
    #[must_use]
    pub fn with_orientation(mut self, orientation: [f32; 4]) -> Self {
        self.flags |= Self::ORIENTATION;
        self.orientation = orientation;
        self
    }

    /// The stored flag byte, all bits included.
    #[must_use]
    pub fn flags(&self) -> u8 {
        self.flags
    }

    fn has(&self, bit: u8) -> bool {
        self.flags & bit != 0
    }

    /// Bit 0x01: region.
    #[must_use]
    pub fn region(&self) -> Option<u8> {
        self.has(Self::REGION).then_some(self.region)
    }

    /// Bit 0x02: landblock X and Y.
    #[must_use]
    pub fn landblock(&self) -> Option<(u8, u8)> {
        self.has(Self::LANDBLOCK)
            .then_some((self.landblock[0], self.landblock[1]))
    }

    /// Bit 0x04: instance (Reported only; never observed).
    #[must_use]
    pub fn instance(&self) -> Option<u16> {
        self.has(Self::INSTANCE).then_some(self.instance)
    }

    /// Bit 0x08: cell (0 = outdoors in observed data).
    #[must_use]
    pub fn cell(&self) -> Option<u16> {
        self.has(Self::CELL).then_some(self.cell)
    }

    /// Bit 0x10: x, y, z (units OPEN).
    #[must_use]
    pub fn offset(&self) -> Option<[f32; 3]> {
        self.has(Self::OFFSET).then_some(self.offset)
    }

    /// Bit 0x20: the orientation's four floats in stored order, which is **(w, x, y, z)**,
    /// scalar first (`coordinates.md` §11.1). Prefer [`Position::rotation`]: passing this
    /// array to a math library that expects (x, y, z, w), such as glam's
    /// `Quat::from_array`, mis-rotates.
    #[must_use]
    pub fn orientation(&self) -> Option<[f32; 4]> {
        self.has(Self::ORIENTATION).then_some(self.orientation)
    }

    /// Bit 0x20: the orientation as a typed rotation, read in the stored (w, x, y, z) order
    /// (`coordinates.md` §11.1) and applied as an active rotation from the object's (or
    /// cell's) frame into the landblock frame (§11.2). The components are returned as
    /// stored; call [`crate::setup::Quaternion::normalized`] before rotating with a value
    /// that may not be unit length.
    #[must_use]
    pub fn rotation(&self) -> Option<crate::setup::Quaternion> {
        self.orientation()
            .map(|[w, x, y, z]| crate::setup::Quaternion { w, x, y, z })
    }

    /// Decodes a position at the reader's position.
    pub fn read(r: &mut Reader<'_>) -> Result<Self> {
        let mut p = Self::new(r.u8()?);
        if p.has(Self::REGION) {
            p.region = r.u8()?;
        }
        if p.has(Self::LANDBLOCK) {
            p.landblock = [r.u8()?, r.u8()?];
        }
        if p.has(Self::INSTANCE) {
            p.instance = r.u16()?;
        }
        if p.has(Self::CELL) {
            p.cell = r.u16()?;
        }
        if p.has(Self::OFFSET) {
            p.offset = [r.f32()?, r.f32()?, r.f32()?];
        }
        if p.has(Self::ORIENTATION) {
            p.orientation = [r.f32()?, r.f32()?, r.f32()?, r.f32()?];
        }
        Ok(p)
    }
}

/// One (PID, value) pair of a property set, struct or array (`property-system.md` §5.2).
#[derive(Debug, Clone, PartialEq)]
pub struct PropertyEntry {
    /// Property ID (never 0; null entries are only counted).
    pub pid: u32,
    /// The decoded value, of the type the schema gives for `pid`.
    pub value: PropertyValue,
}

/// A struct value (type 11, `property-system.md` §4.10).
#[derive(Debug, Clone, PartialEq, Default)]
pub struct PropertyStruct {
    /// Header: bucket-count hint and field count (null entries included).
    pub size: TSize,
    /// Fields in stored (bucket) order. Each PID occurs at most once.
    pub fields: Vec<PropertyEntry>,
}

impl PropertyStruct {
    /// The value of field `pid`, if present.
    #[must_use]
    pub fn get(&self, pid: u32) -> Option<&PropertyValue> {
        self.fields.iter().find(|e| e.pid == pid).map(|e| &e.value)
    }

    /// Number of null (PID 0) entries the header counted.
    #[must_use]
    pub fn null_entries(&self) -> usize {
        (self.size.count as usize).saturating_sub(self.fields.len())
    }

    /// Whether the fields are in bucket order for the header's hint (PS-O2); `None` for a
    /// hint with no known bucket count.
    #[must_use]
    pub fn is_bucket_ordered(&self) -> Option<bool> {
        keys_in_bucket_order(self.size.hint, self.fields.iter().map(|e| e.pid))
    }
}

/// An array value (type 12, `property-system.md` §4.11).
#[derive(Debug, Clone, PartialEq, Default)]
pub struct PropertyArray {
    /// Stored element count (null entries included).
    pub count: u32,
    /// Elements in stored order. Each carries its own PID; PIDs may repeat and differ.
    pub elements: Vec<PropertyEntry>,
}

impl PropertyArray {
    /// Number of null (PID 0) elements the count included.
    #[must_use]
    pub fn null_entries(&self) -> usize {
        (self.count as usize).saturating_sub(self.elements.len())
    }
}

/// A variable-width bitfield (type 22, `property-system.md` §4.4).
#[derive(Debug, Clone, PartialEq, Eq, Default)]
pub struct BitfieldAny {
    /// Number of bits.
    pub bits: u32,
    /// `ceil(bits / 8)` bytes; bit `i` is bit `i % 8` of byte `i / 8`.
    pub bytes: Vec<u8>,
}

impl BitfieldAny {
    /// Whether bit `i` is set. Bits at or past `bits` read as unset.
    #[must_use]
    pub fn bit(&self, i: u32) -> bool {
        i < self.bits
            && self
                .bytes
                .get((i / 8) as usize)
                .is_some_and(|b| b & (1 << (i % 8)) != 0)
    }

    /// Indices of the set bits, in ascending order (unused high bits ignored).
    pub fn set_bits(&self) -> impl Iterator<Item = u32> + '_ {
        (0..self.bits).filter(|&i| self.bit(i))
    }
}

/// A decoded property value (`property-system.md` §4): a raw typed tree with no names.
///
/// Struct and array values contain further entries, so a value is a tree. Names, enum
/// labels and StringInfo text are resolved by higher layers.
#[derive(Debug, Clone, PartialEq)]
#[non_exhaustive]
pub enum PropertyValue {
    /// 1: Latin-1 Pascal string.
    String(String),
    /// 2: string token.
    StringToken(u32),
    /// 3: waveform.
    Waveform(Box<Waveform>),
    /// 4: timestamp (f64; epoch and unit OPEN).
    Timestamp(f64),
    /// 5: tri-state byte (meaning OPEN).
    TriState(u8),
    /// 6: x, y, z.
    Vector3([f32; 3]),
    /// 7: 64-bit instance ID.
    InstanceId(u64),
    /// 8: enum key.
    Enum(u32),
    /// 9: float.
    Float(f32),
    /// 10: property ID.
    PropertyId(u32),
    /// 11: struct.
    Struct(PropertyStruct),
    /// 12: array.
    Array(PropertyArray),
    /// 13: StringInfo.
    StringInfo(Box<StringInfo>),
    /// 14: 64-bit bitfield.
    Bitfield64(u64),
    /// 15: signed 32-bit integer.
    Int(i32),
    /// 16: four colour bytes in stored order (fourth is alpha; RGB order OPEN).
    Colour([u8; 4]),
    /// 17: position (stored inline).
    Position(Position),
    /// 18: 32-bit bitfield.
    Bitfield32(u32),
    /// 19: signed 64-bit integer.
    Int64(i64),
    /// 20: DID reference; 0 means "none".
    DidRef(Did),
    /// 21: boolean.
    Bool(bool),
    /// 22: bitfield of any width.
    BitfieldAny(BitfieldAny),
}

impl PropertyValue {
    /// The type of this value.
    #[must_use]
    pub fn property_type(&self) -> PropertyType {
        match self {
            Self::String(_) => PropertyType::String,
            Self::StringToken(_) => PropertyType::StringToken,
            Self::Waveform(_) => PropertyType::Waveform,
            Self::Timestamp(_) => PropertyType::Timestamp,
            Self::TriState(_) => PropertyType::TriState,
            Self::Vector3(_) => PropertyType::Vector3,
            Self::InstanceId(_) => PropertyType::InstanceId,
            Self::Enum(_) => PropertyType::Enum,
            Self::Float(_) => PropertyType::Float,
            Self::PropertyId(_) => PropertyType::PropertyId,
            Self::Struct(_) => PropertyType::Struct,
            Self::Array(_) => PropertyType::Array,
            Self::StringInfo(_) => PropertyType::StringInfo,
            Self::Bitfield64(_) => PropertyType::Bitfield64,
            Self::Int(_) => PropertyType::Int,
            Self::Colour(_) => PropertyType::Colour,
            Self::Position(_) => PropertyType::Position,
            Self::Bitfield32(_) => PropertyType::Bitfield32,
            Self::Int64(_) => PropertyType::Int64,
            Self::DidRef(_) => PropertyType::DidRef,
            Self::Bool(_) => PropertyType::Bool,
            Self::BitfieldAny(_) => PropertyType::BitfieldAny,
        }
    }

    /// Decodes a value of type `ty` with no schema (for dictionary defaults and scalars).
    ///
    /// A struct or array with at least one non-null entry fails with
    /// [`FormatError::UnknownProperty`]; use [`PropertyValue::read_with`] for those.
    pub fn read(r: &mut Reader<'_>, ty: PropertyType) -> Result<Self> {
        Self::read_with(r, ty, &NoSchema)
    }

    /// Decodes a value of type `ty`, looking up the types of nested struct fields and array
    /// elements in `schema`.
    pub fn read_with<S: PropertySchema + ?Sized>(
        r: &mut Reader<'_>,
        ty: PropertyType,
        schema: &S,
    ) -> Result<Self> {
        let mut d = ValueDecoder::new(schema);
        d.value(r, ty, 0)
    }

    /// Decodes a buffer holding exactly one value of type `ty`.
    pub fn parse<S: PropertySchema + ?Sized>(
        data: &[u8],
        ty: PropertyType,
        schema: &S,
    ) -> Result<Self> {
        let mut r = Reader::new(data);
        let v = Self::read_with(&mut r, ty, schema)?;
        r.expect_end("property value")?;
        Ok(v)
    }

    /// The struct, if this is a struct value.
    #[must_use]
    pub fn as_struct(&self) -> Option<&PropertyStruct> {
        match self {
            Self::Struct(s) => Some(s),
            _ => None,
        }
    }

    /// The array, if this is an array value.
    #[must_use]
    pub fn as_array(&self) -> Option<&PropertyArray> {
        match self {
            Self::Array(a) => Some(a),
            _ => None,
        }
    }

    /// The value as a number, for comparisons: integers, enum keys, bitfields, PIDs, DIDs,
    /// string tokens, booleans (0/1), tri-states, floats and timestamps. `None` for strings,
    /// vectors, positions, colours, StringInfo, structs, arrays and wide bitfields.
    #[must_use]
    pub fn as_f64(&self) -> Option<f64> {
        Some(match self {
            Self::StringToken(v) | Self::Enum(v) | Self::PropertyId(v) | Self::Bitfield32(v) => {
                f64::from(*v)
            }
            Self::DidRef(d) => f64::from(d.0),
            Self::Int(v) => f64::from(*v),
            #[allow(clippy::cast_precision_loss)]
            Self::Int64(v) => *v as f64,
            #[allow(clippy::cast_precision_loss)]
            Self::Bitfield64(v) | Self::InstanceId(v) => *v as f64,
            Self::Float(v) => f64::from(*v),
            Self::Timestamp(v) => *v,
            Self::Bool(b) => f64::from(u8::from(*b)),
            Self::TriState(v) => f64::from(*v),
            _ => return None,
        })
    }

    /// The value as an integer bit pattern, for bit tests: unsigned integers, enum keys,
    /// bitfields (the first 64 bits of a type-22 bitfield), PIDs, DIDs and booleans; signed
    /// integers as their two's-complement bits. `None` for floats and non-scalar values.
    #[must_use]
    pub fn as_bits(&self) -> Option<u64> {
        Some(match self {
            Self::StringToken(v) | Self::Enum(v) | Self::PropertyId(v) | Self::Bitfield32(v) => {
                u64::from(*v)
            }
            Self::DidRef(d) => u64::from(d.0),
            #[allow(clippy::cast_sign_loss)]
            Self::Int(v) => u64::from(*v as u32),
            #[allow(clippy::cast_sign_loss)]
            Self::Int64(v) => *v as u64,
            Self::Bitfield64(v) | Self::InstanceId(v) => *v,
            Self::Bool(b) => u64::from(*b),
            Self::TriState(v) => u64::from(*v),
            Self::BitfieldAny(b) => {
                let mut out = [0u8; 8];
                for (o, i) in out.iter_mut().zip(&b.bytes) {
                    *o = *i;
                }
                u64::from_le_bytes(out)
            }
            _ => return None,
        })
    }

    /// The DID, if this is a DID reference.
    #[must_use]
    pub fn as_did(&self) -> Option<Did> {
        match self {
            Self::DidRef(d) => Some(*d),
            _ => None,
        }
    }

    /// The float, if this is a float value.
    #[must_use]
    pub fn as_f32(&self) -> Option<f32> {
        match self {
            Self::Float(v) => Some(*v),
            _ => None,
        }
    }

    /// Nested entries of a struct (fields) or array (elements); empty for scalars.
    #[must_use]
    pub fn children(&self) -> &[PropertyEntry] {
        match self {
            Self::Struct(s) => &s.fields,
            Self::Array(a) => &a.elements,
            _ => &[],
        }
    }
}

/// Recursive dictionary-driven decoder shared by values and property sets.
pub(crate) struct ValueDecoder<'s, S: ?Sized> {
    schema: &'s S,
    /// Reused buffer for duplicate-PID checks.
    scratch: Vec<u32>,
}

impl<'s, S: PropertySchema + ?Sized> ValueDecoder<'s, S> {
    pub(crate) fn new(schema: &'s S) -> Self {
        Self {
            schema,
            scratch: Vec::new(),
        }
    }

    /// Reads `count` entries; `echo` selects the top-level/struct form (PID, echo, value)
    /// over the array form (PID, value). `depth` is the depth of the entries' values.
    pub(crate) fn entries(
        &mut self,
        r: &mut Reader<'_>,
        count: u32,
        echo: bool,
        depth: usize,
    ) -> Result<Vec<PropertyEntry>> {
        // Nested containers reserve less up front: every level checks its count against the
        // same remaining bytes, so a full reservation per level would multiply with depth.
        let cap_limit = if depth == 0 {
            usize::MAX
        } else {
            NESTED_PREALLOC_CAP
        };
        let mut out = r.alloc_vec(count, MIN_ENTRY_SIZE, cap_limit)?;
        for _ in 0..count {
            if let Some(e) = self.entry(r, echo, depth)? {
                r.push(&mut out, e, count)?;
            }
        }
        if echo {
            self.check_unique(&out, r.position())?;
        }
        Ok(out)
    }

    pub(crate) fn entry(
        &mut self,
        r: &mut Reader<'_>,
        echo: bool,
        depth: usize,
    ) -> Result<Option<PropertyEntry>> {
        let pid_at = r.position();
        let pid = r.u32()?;
        if pid == 0 {
            // Null entry: no echo, no value (Reported; never observed).
            return Ok(None);
        }
        if echo {
            let echo_at = r.position();
            let e = r.u32()?;
            if e != pid {
                return Err(FormatError::malformed(
                    "property entry",
                    echo_at,
                    format!("PID echo {e:#x} differs from PID {pid:#x}"),
                ));
            }
        }
        let ty = self
            .schema
            .property_type(pid)
            .ok_or(FormatError::UnknownProperty {
                pid,
                offset: pid_at,
            })?;
        let value_at = r.position();
        let value = self.value(r, ty, depth).map_err(|e| match e {
            e @ FormatError::PropertyValue { .. } => e,
            e => FormatError::PropertyValue {
                pid,
                type_code: ty.code(),
                offset: value_at,
                source: Box::new(e),
            },
        })?;
        Ok(Some(PropertyEntry { pid, value }))
    }

    fn check_unique(&mut self, entries: &[PropertyEntry], offset: usize) -> Result<()> {
        if entries.len() < 2 {
            return Ok(());
        }
        self.scratch.clear();
        self.scratch.extend(entries.iter().map(|e| e.pid));
        self.scratch.sort_unstable();
        if let Some(w) = self.scratch.windows(2).find(|w| w[0] == w[1]) {
            return Err(FormatError::malformed(
                "property set",
                offset,
                format!("PID {:#x} occurs more than once", w[0]),
            ));
        }
        Ok(())
    }

    pub(crate) fn value(
        &mut self,
        r: &mut Reader<'_>,
        ty: PropertyType,
        depth: usize,
    ) -> Result<PropertyValue> {
        if depth >= PROPERTY_MAX_DEPTH {
            return Err(FormatError::DepthLimit {
                offset: r.position(),
                limit: PROPERTY_MAX_DEPTH,
            });
        }
        Ok(match ty {
            PropertyType::String => PropertyValue::String(r.pascal_string()?),
            PropertyType::StringToken => PropertyValue::StringToken(r.u32()?),
            PropertyType::Waveform => {
                r.charge(std::mem::size_of::<Waveform>())?;
                PropertyValue::Waveform(Box::new(read_waveform(r)?))
            }
            PropertyType::Timestamp => PropertyValue::Timestamp(r.f64()?),
            PropertyType::TriState => PropertyValue::TriState(r.u8()?),
            PropertyType::Vector3 => PropertyValue::Vector3([r.f32()?, r.f32()?, r.f32()?]),
            PropertyType::InstanceId => PropertyValue::InstanceId(r.u64()?),
            PropertyType::Enum => PropertyValue::Enum(r.u32()?),
            PropertyType::Float => PropertyValue::Float(r.f32()?),
            PropertyType::PropertyId => PropertyValue::PropertyId(r.u32()?),
            PropertyType::Struct => {
                // A TSize: hint byte, then the count as a VLE (spec §4.10 OPEN: the VLE
                // reading is byte-identical for counts below 128).
                let size = r.tsize()?;
                let fields = self.entries(r, size.count, true, depth + 1)?;
                PropertyValue::Struct(PropertyStruct { size, fields })
            }
            PropertyType::Array => {
                let count_at = r.position();
                let count = r.u32()?;
                if count > PROPERTY_MAX_ARRAY_LEN {
                    return Err(FormatError::malformed(
                        "property array",
                        count_at,
                        format!("element count {count} exceeds {PROPERTY_MAX_ARRAY_LEN}"),
                    ));
                }
                let elements = self.entries(r, count, false, depth + 1)?;
                PropertyValue::Array(PropertyArray { count, elements })
            }
            PropertyType::StringInfo => {
                r.charge(std::mem::size_of::<StringInfo>())?;
                PropertyValue::StringInfo(Box::new(StringInfo::read(r)?))
            }
            PropertyType::Bitfield64 => PropertyValue::Bitfield64(r.u64()?),
            PropertyType::Int => PropertyValue::Int(r.i32()?),
            PropertyType::Colour => PropertyValue::Colour(r.array()?),
            PropertyType::Position => PropertyValue::Position(Position::read(r)?),
            PropertyType::Bitfield32 => PropertyValue::Bitfield32(r.u32()?),
            PropertyType::Int64 => PropertyValue::Int64(r.i64()?),
            PropertyType::DidRef => PropertyValue::DidRef(r.did()?),
            PropertyType::Bool => PropertyValue::Bool(r.bool8()?),
            PropertyType::BitfieldAny => {
                let bits = r.vle()?;
                let n = usize::try_from(bits.div_ceil(8)).unwrap_or(usize::MAX);
                let bytes = r.bytes(n)?;
                r.charge(n)?;
                PropertyValue::BitfieldAny(BitfieldAny {
                    bits,
                    bytes: bytes.to_vec(),
                })
            }
            PropertyType::Unknown(code) => {
                return Err(FormatError::UnsupportedPropertyType { code });
            }
        })
    }
}

fn read_floats(r: &mut Reader<'_>, n: u32) -> Result<Vec<f32>> {
    let mut v = r.alloc_vec(n, 4, PREALLOC_CAP)?;
    for _ in 0..n {
        let f = r.f32()?;
        r.push(&mut v, f, n)?;
    }
    Ok(v)
}

pub(crate) fn read_waveform(r: &mut Reader<'_>) -> Result<Waveform> {
    let subtype = r.u32()?;
    let (values, extra) = match subtype {
        0 => (Vec::new(), None),
        1 => (read_floats(r, 1)?, None),
        10 => {
            let values = read_floats(r, 10)?;
            let scalar = r.f32()?;
            let flag = r.bool8()?;
            let k = r.u32()?;
            let mut pairs = r.alloc_vec(k, 8, PREALLOC_CAP)?;
            for _ in 0..k {
                let pair = (r.f32()?, r.f32()?);
                r.push(&mut pairs, pair, k)?;
            }
            (
                values,
                Some(WaveformExtra {
                    scalar,
                    flag,
                    pairs,
                }),
            )
        }
        _ => (read_floats(r, 10)?, None),
    };
    Ok(Waveform {
        subtype,
        values,
        extra,
    })
}

/// One property definition (`property-system.md` §2.2).
#[derive(Debug, Clone, PartialEq)]
pub struct PropertyDefinition {
    /// Property ID.
    pub pid: u32,
    /// Value type.
    pub ty: PropertyType,
    /// Unknown field (OPEN).
    pub field_a: u32,
    /// Unknown field (OPEN).
    pub field_b: u32,
    /// Type-dependent field (`property-system.md` §2.5, MP5): the enum mapper or DID map
    /// for type 8, the enum mapper for bitfields (14, 18, 22), the dictionary itself
    /// (0x34000000) for type 10, a DB-object-type code below 0x100 for type 20 (§9; see
    /// [`PropertyDefinition::dbo_type`]), and 0 for every other type. Kept as a [`Did`]
    /// even when it holds a code.
    pub data: Did,
    /// Unknown field (OPEN).
    pub field_c: u32,
    /// Flag bits; see [`FLAG_HAS_DEFAULT`], [`FLAG_HAS_MIN`], [`FLAG_HAS_MAX`].
    pub flags: u32,
    /// Default value.
    pub default: Option<PropertyValue>,
    /// Minimum value.
    pub minimum: Option<PropertyValue>,
    /// Maximum value.
    pub maximum: Option<PropertyValue>,
    /// Five unknown bytes after the marker (OPEN).
    pub field_d: [u8; 5],
    /// Child PIDs (array element or struct fields).
    pub children: Vec<u32>,
    /// Subset list of PIDs (meaning OPEN).
    pub subset: Vec<u32>,
}

impl PropertyDefinition {
    /// For a DID-reference definition (type 20), the DB-object-type code its values refer
    /// to (`property-system.md` §2.5 MP5, §9); `None` for other types or a code outside
    /// 1..0x100. The code's label lives in the "DBOType" enum mapper.
    #[must_use]
    pub fn dbo_type(&self) -> Option<u8> {
        if self.ty != PropertyType::DidRef {
            return None;
        }
        u8::try_from(self.data.0).ok().filter(|&c| c != 0)
    }

    /// Reads one definition keyed by `pid`. Child entries whose two PID copies differ are
    /// added to `mismatched_children` (the caller logs the total once per record).
    fn read(r: &mut Reader<'_>, pid: u32, mismatched_children: &mut u64) -> Result<Self> {
        const WHAT: &str = "property definition";
        let echo_at = r.position();
        let echo = r.u32()?;
        if echo != pid {
            return Err(FormatError::malformed(
                WHAT,
                echo_at,
                format!("pid echo {echo:#x} differs from key {pid:#x}"),
            ));
        }
        let ty_at = r.position();
        let ty = PropertyType::from_code(r.u32()?);
        // property-system.md §3: "Readers report a type code outside 1–22 as malformed."
        if let PropertyType::Unknown(code) = ty {
            return Err(FormatError::malformed(
                WHAT,
                ty_at,
                format!("type code {code} outside 1–22"),
            ));
        }
        let field_a = r.u32()?;
        let field_b = r.u32()?;
        let data = r.did()?;
        let field_c = r.u32()?;
        let flags = r.u32()?;
        let read_opt = |bit: u32, r: &mut Reader<'_>| -> Result<Option<PropertyValue>> {
            if flags & bit != 0 && !ty.skips_definition_values() {
                PropertyValue::read(r, ty).map(Some)
            } else {
                Ok(None)
            }
        };
        let default = read_opt(FLAG_HAS_DEFAULT, r)?;
        let minimum = read_opt(FLAG_HAS_MIN, r)?;
        let maximum = read_opt(FLAG_HAS_MAX, r)?;
        let marker_at = r.position();
        let marker = r.u32()?;
        if marker != DEFINITION_MARKER {
            return Err(FormatError::malformed(
                WHAT,
                marker_at,
                format!("marker {marker:#010x}, expected {DEFINITION_MARKER:#010x}"),
            ));
        }
        let field_d = r.array()?;
        let child_count = r.vle()?;
        let mut children = r.alloc_vec(child_count, 8, PREALLOC_CAP)?;
        for _ in 0..child_count {
            let a = r.u32()?;
            let b = r.u32()?;
            if a != b {
                // The first copy is kept (records.md §13, property-system.md §2.2).
                *mismatched_children = mismatched_children.saturating_add(1);
            }
            r.push(&mut children, a, child_count)?;
        }
        let subset_count = r.u32()?;
        let mut subset = r.alloc_vec(subset_count, 4, PREALLOC_CAP)?;
        for _ in 0..subset_count {
            let pid = r.u32()?;
            r.push(&mut subset, pid, subset_count)?;
        }
        let term_at = r.position();
        let term = r.u32()?;
        if term != 0 {
            return Err(FormatError::malformed(
                WHAT,
                term_at,
                format!("terminator {term:#x}, expected 0"),
            ));
        }
        Ok(Self {
            pid,
            ty,
            field_a,
            field_b,
            data,
            field_c,
            flags,
            default,
            minimum,
            maximum,
            field_d,
            children,
            subset,
        })
    }
}

/// The master property dictionary (`property-system.md` §2.1).
///
/// The decoded lists are read-only so that the lookup indexes always agree with them.
#[derive(Clone, PartialEq)]
pub struct PropertyDictionary {
    /// Record DID (0x34000000).
    pub did: Did,
    /// Eight reserved bytes after the DID (observed zero; meaning OPEN).
    pub reserved: [u8; 8],
    /// TSize header of the name list.
    pub names_size: TSize,
    /// Two-byte separator (observed `00 00`; meaning OPEN).
    pub separator: [u8; 2],
    /// TSize header of the definition list.
    pub definitions_size: TSize,
    names: Vec<(u32, String)>,
    definitions: Vec<PropertyDefinition>,
    /// PID → position in `names` of its first name.
    name_index: HashMap<u32, u32>,
    /// Positions in `names`, sorted by name; among equal names the first stored comes
    /// first. Borrowing the names keeps the index at 4 bytes per name.
    by_name: Vec<u32>,
    def_index: HashMap<u32, u32>,
}

impl fmt::Debug for PropertyDictionary {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.debug_struct("PropertyDictionary")
            .field("did", &self.did)
            .field("reserved", &self.reserved)
            .field("names_size", &self.names_size)
            .field("names", &self.names)
            .field("separator", &self.separator)
            .field("definitions_size", &self.definitions_size)
            .field("definitions", &self.definitions)
            .finish_non_exhaustive()
    }
}

impl PropertyDictionary {
    /// PID → name, in stored (bucket) order.
    #[must_use]
    pub fn names(&self) -> &[(u32, String)] {
        &self.names
    }

    /// Definitions in stored (bucket) order.
    #[must_use]
    pub fn definitions(&self) -> &[PropertyDefinition] {
        &self.definitions
    }

    /// Name of a PID.
    #[must_use]
    pub fn name(&self, pid: u32) -> Option<&str> {
        self.name_index
            .get(&pid)
            .and_then(|&i| self.names.get(i as usize))
            .map(|(_, n)| n.as_str())
    }

    /// PID with the given name (the first stored one if several PIDs share it).
    #[must_use]
    pub fn pid(&self, name: &str) -> Option<u32> {
        let name_at = |i: u32| self.names.get(i as usize).map_or("", |(_, n)| n.as_str());
        let at = self.by_name.partition_point(|&i| name_at(i) < name);
        let &i = self.by_name.get(at)?;
        self.names
            .get(i as usize)
            .filter(|(_, n)| n == name)
            .map(|(pid, _)| *pid)
    }

    /// Definition of a PID.
    #[must_use]
    pub fn definition(&self, pid: u32) -> Option<&PropertyDefinition> {
        self.def_index
            .get(&pid)
            .and_then(|&i| self.definitions.get(i as usize))
    }
}

impl Record<'_> for PropertyDictionary {
    const NAME: &'static str = "property dictionary";

    fn did(&self) -> Did {
        self.did
    }

    fn parse(data: &[u8]) -> Result<Self> {
        let mut r = Reader::new(data);
        let did = read_prologue(&mut r, Self::NAME, |k| k == DidKind::PropertyDictionary)?;
        let reserved = r.array()?;
        let names_size = r.tsize()?;
        let mut names = r.alloc_vec(names_size.count, 5, PREALLOC_CAP)?;
        for _ in 0..names_size.count {
            let pid = r.u32()?;
            let name = r.pascal_string()?;
            r.push(&mut names, (pid, name), names_size.count)?;
        }
        let separator = r.array()?;
        let definitions_size = r.tsize()?;
        // Smallest definition: key + 7 u32 + marker + 5 + 1-byte VLE + subset count + term.
        let mut definitions = r.alloc_vec(
            definitions_size.count,
            4 + 28 + 4 + 5 + 1 + 4 + 4,
            PREALLOC_CAP,
        )?;
        let mut mismatched_children = 0u64;
        for _ in 0..definitions_size.count {
            let pid = r.u32()?;
            let def = PropertyDefinition::read(&mut r, pid, &mut mismatched_children)?;
            r.push(&mut definitions, def, definitions_size.count)?;
        }
        if mismatched_children != 0 {
            // One event per record, however many definitions are affected.
            tracing::debug!(
                %did,
                count = mismatched_children,
                "property child PID copies differ; first copies kept"
            );
        }
        r.expect_end(Self::NAME)?;
        r.log_unknown_hints(Self::NAME);
        // Indexes are built once at their final size, so they never grow by doubling.
        r.charge(
            hash_index_bytes::<u32, u32>(names.len())
                .saturating_add(hash_index_bytes::<u32, u32>(definitions.len()))
                .saturating_add(names.len().saturating_mul(4)),
        )?;
        let mut name_index = HashMap::with_capacity(names.len());
        for (i, (pid, _)) in names.iter().enumerate() {
            name_index.entry(*pid).or_insert(index_u32(i));
        }
        let mut def_index = HashMap::with_capacity(definitions.len());
        for (i, def) in definitions.iter().enumerate() {
            def_index.entry(def.pid).or_insert(index_u32(i));
        }
        let mut by_name: Vec<u32> = (0..names.len()).map(index_u32).collect();
        // Stable: equal names keep their stored order, so `pid` finds the first one.
        by_name.sort_by(|&a, &b| {
            let name = |i: u32| names.get(i as usize).map_or("", |(_, n)| n.as_str());
            name(a).cmp(name(b))
        });
        Ok(Self {
            did,
            reserved,
            names_size,
            names,
            separator,
            definitions_size,
            definitions,
            name_index,
            by_name,
            def_index,
        })
    }
}
