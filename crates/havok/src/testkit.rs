// SPDX-License-Identifier: GPL-3.0-or-later
//! Synthetic builders for every structure kind of `havok-data.md` (tests, fuzz seeds,
//! `tessera-anim` tests).
//!
//! Everything here is written by this crate's own code from invented values (§12); no
//! game data is involved. The builders never panic on the values they are given: any
//! values, including NaN or inconsistent counts, are written as asked so that tests and
//! fuzz targets can produce malformed input on purpose.
//!
//! | Builder | Structure kind |
//! |---|---|
//! | [`TagfileWriter`] | container: header, varints, string pool, metadata, objects, struct arrays |
//! | [`SkeletonSpec`], [`example_skeleton_12_1`] | skeleton tagfile, 0x04 fields 1–3 |
//! | [`BlockSpec`], [`encode_block`], [`SplineClipSpec`] | spline blocks and spline clips |
//! | [`InterleavedClipSpec`] | interleaved clips |
//! | [`AnimationSpec`] (annotation names, embedded reference frame) | clip tagfiles, annotation tracks |
//! | [`ReferenceFrameSpec`] | reference frames (leading or embedded) |
//! | [`ClipRecordSpec`] | the 0x05 envelope |
//! | [`encode_quat40`] | 40-bit quaternions |

use crate::tagfile::MAGIC;
use crate::transform::Transform;

/// Appends a sign-magnitude varint (§3.2) in its shortest form.
pub fn write_varint(out: &mut Vec<u8>, v: i32) {
    let negative = v < 0;
    let mut mag = v.unsigned_abs();
    let mut b = (((mag & 0x3F) as u8) << 1) | u8::from(negative);
    mag >>= 6;
    while mag != 0 {
        out.push(b | 0x80);
        b = (mag & 0x7F) as u8;
        mag >>= 7;
    }
    out.push(b);
}

/// Appends a VLE (`records.md` §1.2) in its shortest form.
pub fn write_vle(out: &mut Vec<u8>, v: u32) {
    if v < 0x80 {
        out.push(v as u8);
    } else if v < 0x4000 {
        out.push(0x80 | (v >> 8) as u8);
        out.push(v as u8);
    } else if v < 0x2000_0000 {
        out.push(0xC0 | (v >> 24) as u8);
        out.push((v >> 16) as u8);
        out.extend_from_slice(&(v as u16).to_le_bytes());
    } else {
        out.push(0xE0);
        out.extend_from_slice(&v.to_le_bytes());
    }
}

/// Encodes a quaternion (x, y, z, w) as 40 bits (§12.2): drops the largest-magnitude
/// component, stores its index and sign, and quantises the others to
/// round(c × 2047 × √2) + 2047.
pub fn encode_quat40(q: [f32; 4]) -> [u8; 5] {
    let mut drop = 0;
    for i in 1..4 {
        if q[i].abs() > q[drop].abs() {
            drop = i;
        }
    }
    let sign = u64::from(q[drop] < 0.0);
    let mut fields = [0u64; 3];
    let mut k = 0;
    for (i, &c) in q.iter().enumerate() {
        if i != drop {
            let a = (f64::from(c) * 2047.0 * std::f64::consts::SQRT_2).round() + 2047.0;
            fields[k] = a.clamp(0.0, 4095.0) as u64;
            k += 1;
        }
    }
    let v = fields[0] | fields[1] << 12 | fields[2] << 24 | (drop as u64) << 36 | sign << 38;
    let b = v.to_le_bytes();
    [b[0], b[1], b[2], b[3], b[4]]
}

/// A member descriptor for [`TagfileWriter::class`].
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct MemberDef {
    /// Member name.
    pub name: String,
    /// Raw type code (§3.6), written as given.
    pub ty: i32,
    /// Tuple size, written when `ty` has the tuple bit.
    pub tuple_size: i32,
    /// Class name, written when the basic kind is object or struct.
    pub class_name: Option<String>,
}

impl MemberDef {
    /// A member with no class name.
    pub fn new(name: &str, ty: i32) -> Self {
        MemberDef {
            name: name.into(),
            ty,
            tuple_size: 0,
            class_name: None,
        }
    }

    /// A member with a class name (object or struct kinds).
    pub fn class(name: &str, ty: i32, class_name: &str) -> Self {
        MemberDef {
            name: name.into(),
            ty,
            tuple_size: 0,
            class_name: Some(class_name.into()),
        }
    }
}

/// A value for [`TagfileWriter::object`], written by its variant (not by the declared type,
/// so type mismatches can be produced on purpose).
#[derive(Debug, Clone, PartialEq)]
pub enum WValue {
    /// One raw byte.
    Byte(u8),
    /// Varint.
    Int(i32),
    /// f32.
    Real(f32),
    /// 4 × f32.
    Vec4([f32; 4]),
    /// 12 × f32.
    Vec12([f32; 12]),
    /// Object reference (varint).
    Object(i32),
    /// String (pool reference when already present).
    CString(String),
    /// Byte array / byte column.
    Bytes(Vec<u8>),
    /// Int array / column (width prefix 4).
    IntArray(Vec<i32>),
    /// Real array / column.
    RealArray(Vec<f32>),
    /// Vec4 array / column with component count `components` (1–4 written as given).
    Vec4Array {
        /// Component count *k*.
        components: u8,
        /// Values; only the first `components` floats of each are written.
        values: Vec<[f32; 4]>,
    },
    /// Vec12 array / column.
    Vec12Array(Vec<[f32; 12]>),
    /// String array / column.
    CStringArray(Vec<String>),
    /// Struct array of the member's class: count, bitmap and columns (column values use the
    /// array variants and are written without their count).
    StructArray {
        /// Element count.
        len: usize,
        /// Present columns by member name.
        columns: Vec<(String, WValue)>,
    },
}

#[derive(Debug, Clone)]
struct WClass {
    name: String,
    parent: usize,
    members: Vec<MemberDef>,
}

/// Low-level tagfile writer (§3). Keeps a string pool like a reader would, so repeated
/// strings become references.
#[derive(Debug, Clone)]
pub struct TagfileWriter {
    buf: Vec<u8>,
    pool: Vec<Vec<u8>>,
    classes: Vec<WClass>,
}

impl Default for TagfileWriter {
    fn default() -> Self {
        Self::new()
    }
}

impl TagfileWriter {
    /// Starts a tagfile: magic and file info, version 3.
    pub fn new() -> Self {
        let mut w = Self::empty();
        w.buf.extend_from_slice(&MAGIC);
        w.varint(1);
        w.varint(3);
        w
    }

    /// Starts with no bytes at all (for malformed headers).
    pub fn empty() -> Self {
        TagfileWriter {
            buf: Vec::new(),
            pool: vec![Vec::new(), Vec::new()],
            classes: vec![WClass {
                name: String::new(),
                parent: 0,
                members: Vec::new(),
            }],
        }
    }

    /// Appends raw bytes.
    pub fn raw(&mut self, b: &[u8]) -> &mut Self {
        self.buf.extend_from_slice(b);
        self
    }

    /// Appends a varint.
    pub fn varint(&mut self, v: i32) -> &mut Self {
        write_varint(&mut self.buf, v);
        self
    }

    /// Appends a string: a reference if the pool already holds it (the empty string is
    /// reference 0), else a new entry.
    pub fn string(&mut self, s: &str) -> &mut Self {
        let b = s.as_bytes();
        if let Some(i) = self.pool.iter().position(|p| p == b) {
            self.varint(-(i as i32));
        } else {
            self.varint(b.len() as i32);
            self.buf.extend_from_slice(b);
            self.pool.push(b.to_vec());
        }
        self
    }

    /// Appends an f32.
    pub fn f32(&mut self, v: f32) -> &mut Self {
        self.buf.extend_from_slice(&v.to_le_bytes());
        self
    }

    /// Writes a metadata entry and returns the new class index.
    pub fn class(
        &mut self,
        name: &str,
        version: i32,
        parent: usize,
        members: &[MemberDef],
    ) -> usize {
        self.varint(2);
        self.string(name);
        self.varint(version);
        self.varint(parent as i32);
        self.varint(members.len() as i32);
        for m in members {
            self.string(&m.name);
            self.varint(m.ty);
            if m.ty & 0x20 != 0 {
                self.varint(m.tuple_size);
            }
            if matches!(m.ty & 0x0F, 8 | 9) {
                self.string(m.class_name.as_deref().unwrap_or(""));
            }
        }
        self.classes.push(WClass {
            name: name.into(),
            parent,
            members: members.to_vec(),
        });
        self.classes.len() - 1
    }

    /// Index of the class called `name`.
    pub fn class_index(&self, name: &str) -> Option<usize> {
        self.classes
            .iter()
            .skip(1)
            .position(|c| c.name == name)
            .map(|i| i + 1)
    }

    /// Full member list of `class` (inherited members first).
    pub fn full_members(&self, class: usize) -> Vec<MemberDef> {
        let mut chain = Vec::new();
        let mut c = class;
        while c != 0 && c < self.classes.len() && chain.len() < 64 {
            chain.push(c);
            c = self.classes[c].parent;
        }
        chain
            .iter()
            .rev()
            .flat_map(|&c| self.classes[c].members.iter().cloned())
            .collect()
    }

    fn bitmap(&mut self, full: usize, present: &[usize]) {
        let mut bm = vec![0u8; full.div_ceil(8)];
        for &i in present {
            if let Some(b) = bm.get_mut(i / 8) {
                *b |= 1 << (i % 8);
            }
        }
        self.buf.extend_from_slice(&bm);
    }

    /// Writes a remembered object (tag 4) of `class` with the given member values (in any
    /// order; written in full-list order with the matching bitmap). Names the class does
    /// not declare are skipped; a repeated name maps to its first declaration.
    pub fn object(&mut self, class: usize, values: &[(String, WValue)]) -> &mut Self {
        self.varint(4);
        self.varint(class as i32);
        self.body(class, values);
        self
    }

    fn body(&mut self, class: usize, values: &[(String, WValue)]) {
        let full = self.full_members(class);
        let mut indexed: Vec<(usize, &WValue, &MemberDef)> = values
            .iter()
            .filter_map(|(n, v)| {
                let i = full.iter().position(|m| &m.name == n)?;
                Some((i, v, &full[i]))
            })
            .collect();
        indexed.sort_by_key(|e| e.0);
        let present: Vec<usize> = indexed.iter().map(|e| e.0).collect();
        self.bitmap(full.len(), &present);
        for (_, v, m) in indexed {
            self.value(v, m, true);
        }
    }

    /// Writes a value; `counted` = false writes an array as a struct-array column.
    fn value(&mut self, v: &WValue, m: &MemberDef, counted: bool) {
        let count = |w: &mut Self, n: usize| {
            if counted {
                w.varint(n as i32);
            }
        };
        match v {
            WValue::Byte(b) => {
                self.buf.push(*b);
            }
            WValue::Int(i) => {
                self.varint(*i);
            }
            WValue::Real(r) => {
                self.f32(*r);
            }
            WValue::Vec4(x) => x.iter().for_each(|&f| {
                self.f32(f);
            }),
            WValue::Vec12(x) => x.iter().for_each(|&f| {
                self.f32(f);
            }),
            WValue::Object(r) => {
                self.varint(*r);
            }
            WValue::CString(s) => {
                self.string(s);
            }
            WValue::Bytes(b) => {
                count(self, b.len());
                self.buf.extend_from_slice(b);
            }
            WValue::IntArray(a) => {
                count(self, a.len());
                self.varint(4);
                a.iter().for_each(|&i| {
                    self.varint(i);
                });
            }
            WValue::RealArray(a) => {
                count(self, a.len());
                a.iter().for_each(|&f| {
                    self.f32(f);
                });
            }
            WValue::Vec4Array { components, values } => {
                count(self, values.len());
                self.varint(i32::from(*components));
                let k = usize::from((*components).min(4));
                for x in values {
                    x.iter().take(k).for_each(|&f| {
                        self.f32(f);
                    });
                }
            }
            WValue::Vec12Array(a) => {
                count(self, a.len());
                a.iter().flatten().for_each(|&f| {
                    self.f32(f);
                });
            }
            WValue::CStringArray(a) => {
                count(self, a.len());
                for s in a {
                    self.string(s);
                }
            }
            WValue::StructArray { len, columns } => {
                count(self, *len);
                // An element class that is not declared (yet) leaves just the count, which a
                // reader rejects; that is still useful malformed input.
                let Some(class) = m.class_name.as_deref().and_then(|n| self.class_index(n)) else {
                    return;
                };
                let full = self.full_members(class);
                let mut indexed: Vec<(usize, &WValue, MemberDef)> = columns
                    .iter()
                    .filter_map(|(n, v)| {
                        let i = full.iter().position(|fm| &fm.name == n)?;
                        Some((i, v, full[i].clone()))
                    })
                    .collect();
                indexed.sort_by_key(|e| e.0);
                let present: Vec<usize> = indexed.iter().map(|e| e.0).collect();
                self.bitmap(full.len(), &present);
                for (_, v, fm) in indexed {
                    self.value(v, &fm, false);
                }
            }
        }
    }

    /// Writes the end tag and returns the bytes.
    pub fn finish(mut self) -> Vec<u8> {
        self.varint(7);
        self.buf
    }

    /// Returns the bytes written so far, without an end tag.
    pub fn into_bytes(self) -> Vec<u8> {
        self.buf
    }
}

fn s(name: &str) -> String {
    name.to_string()
}

/// Declares `hkBaseObject` and `hkReferencedObject` (§4.1); returns the latter's index.
pub fn declare_base_classes(w: &mut TagfileWriter) -> usize {
    let base = w.class("hkBaseObject", 0, 0, &[]);
    w.class(
        "hkReferencedObject",
        0,
        base,
        &[
            MemberDef::new("memSizeAndFlags", 0),
            MemberDef::new("referenceCount", 0),
        ],
    )
}

/// Declares the six skeleton-tagfile classes with their §4.1 member lists; returns the
/// `hkaSkeleton` class index.
pub fn declare_skeleton_classes(w: &mut TagfileWriter) -> usize {
    let referenced = declare_base_classes(w);
    let skel = w.class(
        "hkaSkeleton",
        3,
        referenced,
        &[
            MemberDef::new("name", 10),
            MemberDef::new("parentIndices", 18),
            MemberDef::class("bones", 25, "hkaBone"),
            MemberDef::new("referencePose", 22),
            MemberDef::new("referenceFloats", 19),
            MemberDef::new("floatSlots", 26),
            MemberDef::class("localFrames", 25, "hkaSkeletonLocalFrameOnBone"),
        ],
    );
    w.class(
        "hkaBone",
        0,
        0,
        &[
            MemberDef::new("name", 10),
            MemberDef::new("lockTranslation", 1),
        ],
    );
    w.class(
        "hkaSkeletonLocalFrameOnBone",
        0,
        0,
        &[
            MemberDef::class("localFrame", 8, "hkLocalFrame"),
            MemberDef::new("boneIndex", 2),
        ],
    );
    w.class("hkLocalFrame", 0, referenced, &[]);
    skel
}

/// One bone of a [`SkeletonSpec`].
#[derive(Debug, Clone, PartialEq)]
pub struct BoneSpec {
    /// Name (`None` is written as the empty string when other bones have names).
    pub name: Option<String>,
    /// Parent index, −1 for a root (written as given).
    pub parent: i32,
    /// Reference pose.
    pub reference: Transform,
    /// `lockTranslation`.
    pub lock_translation: bool,
}

/// A skeleton tagfile description with the full §4.1 class set.
#[derive(Debug, Clone, PartialEq)]
pub struct SkeletonSpec {
    /// Bones in stored order.
    pub bones: Vec<BoneSpec>,
    /// Member values that replace (by name) or add to the generated ones.
    pub overrides: Vec<(String, WValue)>,
}

impl SkeletonSpec {
    /// A chain of `n` bones named "b0", "b1", ..., each 1 unit along z from its parent.
    pub fn chain(n: usize) -> Self {
        SkeletonSpec {
            bones: (0..n)
                .map(|i| BoneSpec {
                    name: Some(format!("b{i}")),
                    parent: i as i32 - 1,
                    reference: Transform {
                        translation: [0.0, 0.0, if i == 0 { 0.0 } else { 1.0 }],
                        ..Transform::IDENTITY
                    },
                    lock_translation: false,
                })
                .collect(),
            overrides: Vec::new(),
        }
    }

    /// The `hkaSkeleton` member values this spec produces (before overrides).
    pub fn values(&self) -> Vec<(String, WValue)> {
        let n = self.bones.len();
        let mut columns = Vec::new();
        if self.bones.iter().any(|b| b.name.is_some()) {
            columns.push((
                s("name"),
                WValue::CStringArray(
                    self.bones
                        .iter()
                        .map(|b| b.name.clone().unwrap_or_default())
                        .collect(),
                ),
            ));
        }
        if self.bones.iter().any(|b| b.lock_translation) {
            columns.push((
                s("lockTranslation"),
                WValue::Bytes(
                    self.bones
                        .iter()
                        .map(|b| u8::from(b.lock_translation))
                        .collect(),
                ),
            ));
        }
        vec![
            (
                s("parentIndices"),
                WValue::IntArray(self.bones.iter().map(|b| b.parent).collect()),
            ),
            (s("bones"), WValue::StructArray { len: n, columns }),
            (
                s("referencePose"),
                WValue::Vec12Array(self.bones.iter().map(|b| b.reference.to_vec12()).collect()),
            ),
        ]
    }

    /// The skeleton tagfile.
    pub fn to_tagfile(&self) -> Vec<u8> {
        let mut w = TagfileWriter::new();
        let skel = declare_skeleton_classes(&mut w);
        let values = apply_overrides(self.values(), &self.overrides);
        w.object(skel, &values);
        w.finish()
    }

    /// Fields 1–3 of a 0x04 setup record (DID, tagfile length, tagfile) — not a whole setup.
    pub fn to_setup_prefix(&self, did: u32) -> Vec<u8> {
        let t = self.to_tagfile();
        let mut out = did.to_le_bytes().to_vec();
        out.extend_from_slice(&(t.len() as u32).to_le_bytes());
        out.extend_from_slice(&t);
        out
    }
}

fn apply_overrides(
    mut values: Vec<(String, WValue)>,
    overrides: &[(String, WValue)],
) -> Vec<(String, WValue)> {
    for (n, v) in overrides {
        if let Some(slot) = values.iter_mut().find(|(m, _)| m == n) {
            slot.1 = v.clone();
        } else {
            values.push((n.clone(), v.clone()));
        }
    }
    values
}

/// The minimal skeleton tagfile of §12.1 (264 bytes, reduced class set, three bones).
pub fn example_skeleton_12_1() -> Vec<u8> {
    let mut w = TagfileWriter::new();
    let skel = w.class(
        "hkaSkeleton",
        3,
        0,
        &[
            MemberDef::new("parentIndices", 18),
            MemberDef::class("bones", 25, "hkaBone"),
            MemberDef::new("referencePose", 22),
        ],
    );
    w.class(
        "hkaBone",
        0,
        0,
        &[
            MemberDef::new("name", 10),
            MemberDef::new("lockTranslation", 1),
        ],
    );
    let h = std::f32::consts::FRAC_1_SQRT_2;
    let pose = |t: [f32; 3], r: [f32; 4]| {
        Transform {
            translation: t,
            rotation: r,
            scale: [1.0; 3],
        }
        .to_vec12()
    };
    w.object(
        skel,
        &[
            (s("parentIndices"), WValue::IntArray(vec![-1, 0, 1])),
            (
                s("bones"),
                WValue::StructArray {
                    len: 3,
                    columns: vec![(
                        s("name"),
                        WValue::CStringArray(vec![s("b0"), s("b1"), s("b2")]),
                    )],
                },
            ),
            (
                s("referencePose"),
                WValue::Vec12Array(vec![
                    pose([0.0; 3], [0.0, 0.0, 0.0, 1.0]),
                    pose([0.0, 0.0, 1.0], [0.0, 0.0, 0.0, 1.0]),
                    pose([0.0, 0.0, 1.0], [0.0, 0.0, h, h]),
                ]),
            ),
        ],
    );
    w.finish()
}

/// One component of a vector spline channel (§5.4).
#[derive(Debug, Clone, Copy, PartialEq)]
pub enum ComponentSpec {
    /// Neither static nor dynamic: the default (0 position, 1 scale).
    Default,
    /// Static value.
    Static(f32),
    /// Dynamic, with its dequantisation range.
    Dynamic {
        /// Range minimum.
        min: f32,
        /// Range maximum.
        max: f32,
    },
}

/// A position or scale channel of one track in one block.
#[derive(Debug, Clone, PartialEq)]
pub enum VectorSpec {
    /// Constant: `Some` components are static, `None` take the default.
    Constant([Option<f32>; 3]),
    /// Spline channel with raw quantised control points.
    Spline {
        /// Degree *p*.
        degree: u8,
        /// Knot vector (n + p + 2 values for n + 1 control points).
        knots: Vec<u8>,
        /// Per-component kind.
        components: [ComponentSpec; 3],
        /// 8-bit control points (quantisation code 0) instead of 16-bit.
        eight_bit: bool,
        /// Control points, point-major, one value per dynamic component in x, y, z order.
        quantised: Vec<u16>,
    },
}

impl VectorSpec {
    /// The default channel (no data).
    pub const DEFAULT: VectorSpec = VectorSpec::Constant([None; 3]);

    /// A spline channel from float control points (`points[k][c]` is used for the dynamic
    /// components `c` given by `dynamic`, bit 0 = x), 16-bit quantised over each
    /// component's range; other components take `fixed` (`None` = default).
    pub fn spline(
        degree: u8,
        knots: Vec<u8>,
        points: &[[f32; 3]],
        dynamic: u8,
        fixed: [Option<f32>; 3],
    ) -> Self {
        let mut components = [ComponentSpec::Default; 3];
        for c in 0..3 {
            components[c] = if dynamic & (1 << c) != 0 {
                let min = points.iter().map(|p| p[c]).fold(f32::INFINITY, f32::min);
                let max = points
                    .iter()
                    .map(|p| p[c])
                    .fold(f32::NEG_INFINITY, f32::max);
                ComponentSpec::Dynamic { min, max }
            } else if let Some(v) = fixed[c] {
                ComponentSpec::Static(v)
            } else {
                ComponentSpec::Default
            };
        }
        let mut quantised = Vec::new();
        for p in points {
            for c in 0..3 {
                if let ComponentSpec::Dynamic { min, max } = components[c] {
                    let q = if max > min {
                        ((p[c] - min) / (max - min) * 65535.0)
                            .round()
                            .clamp(0.0, 65535.0)
                    } else {
                        0.0
                    };
                    quantised.push(q as u16);
                }
            }
        }
        VectorSpec::Spline {
            degree,
            knots,
            components,
            eight_bit: false,
            quantised,
        }
    }

    fn mask(&self) -> u8 {
        match self {
            VectorSpec::Constant(v) => (0..3)
                .filter(|&c| v[c].is_some())
                .fold(0, |m, c| m | 1 << c),
            VectorSpec::Spline { components, .. } => {
                let mut m = 0;
                for (c, k) in components.iter().enumerate() {
                    match k {
                        ComponentSpec::Static(_) => m |= 1 << c,
                        ComponentSpec::Dynamic { .. } => m |= 1 << (c + 4),
                        ComponentSpec::Default => {}
                    }
                }
                m
            }
        }
    }

    fn eight_bit(&self) -> bool {
        matches!(
            self,
            VectorSpec::Spline {
                eight_bit: true,
                ..
            }
        )
    }
}

/// A rotation channel of one track in one block.
#[derive(Debug, Clone, PartialEq)]
pub enum RotationSpec {
    /// No data.
    Identity,
    /// One quaternion (40-bit encoded).
    Static([f32; 4]),
    /// A quaternion curve (control points 40-bit encoded).
    Dynamic {
        /// Degree *p*.
        degree: u8,
        /// Knot vector.
        knots: Vec<u8>,
        /// Control points (x, y, z, w).
        points: Vec<[f32; 4]>,
    },
}

/// The three channels of one track in one block.
#[derive(Debug, Clone, PartialEq)]
pub struct TrackSpec {
    /// Position channel.
    pub position: VectorSpec,
    /// Rotation channel.
    pub rotation: RotationSpec,
    /// Scale channel.
    pub scale: VectorSpec,
    /// Quantisation byte; the low bits that matter are replaced by the channel's needs
    /// only when [`TrackSpec::new`] built it (0x45, or 0x44/0x05 variants for 8-bit).
    pub quantisation: u8,
}

impl TrackSpec {
    /// A track with the quantisation byte derived from its channels (16-bit unless a
    /// channel asks for 8-bit; 40-bit rotation).
    pub fn new(position: VectorSpec, rotation: RotationSpec, scale: VectorSpec) -> Self {
        let pq = u8::from(!position.eight_bit());
        let sq = u8::from(!scale.eight_bit());
        TrackSpec {
            position,
            rotation,
            scale,
            quantisation: pq | (1 << 2) | (sq << 6),
        }
    }

    /// An all-default track (identity transform).
    pub fn identity() -> Self {
        Self::new(
            VectorSpec::DEFAULT,
            RotationSpec::Identity,
            VectorSpec::DEFAULT,
        )
    }
}

/// One block of a spline clip.
#[derive(Debug, Clone, PartialEq)]
pub struct BlockSpec {
    /// One entry per transform track.
    pub tracks: Vec<TrackSpec>,
}

fn pad4(out: &mut Vec<u8>) {
    while !out.len().is_multiple_of(4) {
        out.push(0);
    }
}

fn write_vector(out: &mut Vec<u8>, v: &VectorSpec) {
    match v {
        VectorSpec::Constant(c) => {
            for x in c.iter().flatten() {
                out.extend_from_slice(&x.to_le_bytes());
            }
        }
        VectorSpec::Spline {
            degree,
            knots,
            components,
            eight_bit,
            quantised,
        } => {
            let dims = components
                .iter()
                .filter(|c| matches!(c, ComponentSpec::Dynamic { .. }))
                .count()
                .max(1);
            let n = (quantised.len() / dims).saturating_sub(1) as u16;
            out.extend_from_slice(&n.to_le_bytes());
            out.push(*degree);
            out.extend_from_slice(knots);
            pad4(out);
            for c in components {
                match c {
                    ComponentSpec::Static(x) => out.extend_from_slice(&x.to_le_bytes()),
                    ComponentSpec::Dynamic { min, max } => {
                        out.extend_from_slice(&min.to_le_bytes());
                        out.extend_from_slice(&max.to_le_bytes());
                    }
                    ComponentSpec::Default => {}
                }
            }
            for &q in quantised {
                if *eight_bit {
                    out.push((q >> 8) as u8);
                } else {
                    out.extend_from_slice(&q.to_le_bytes());
                }
            }
            pad4(out);
        }
    }
}

/// Encodes a block's transform section (§5.2–§5.4): masks, padding, track records. The
/// result's length is the block's `floatBlockOffsets` value. For 8-bit channels the high
/// byte of each quantised value is written.
pub fn encode_block(block: &BlockSpec) -> Vec<u8> {
    let mut out = Vec::new();
    for t in &block.tracks {
        let rot_mask = match t.rotation {
            RotationSpec::Identity => 0,
            RotationSpec::Static(_) => 0x0F,
            RotationSpec::Dynamic { .. } => 0xF0,
        };
        out.extend_from_slice(&[t.quantisation, t.position.mask(), rot_mask, t.scale.mask()]);
    }
    pad4(&mut out);
    for t in &block.tracks {
        write_vector(&mut out, &t.position);
        match &t.rotation {
            RotationSpec::Identity => {}
            RotationSpec::Static(q) => out.extend_from_slice(&encode_quat40(*q)),
            RotationSpec::Dynamic {
                degree,
                knots,
                points,
            } => {
                let n = points.len().saturating_sub(1) as u16;
                out.extend_from_slice(&n.to_le_bytes());
                out.push(*degree);
                out.extend_from_slice(knots);
                for q in points {
                    out.extend_from_slice(&encode_quat40(*q));
                }
            }
        }
        pad4(&mut out);
        write_vector(&mut out, &t.scale);
    }
    out
}

/// A spline-compressed clip description.
#[derive(Debug, Clone, PartialEq)]
pub struct SplineClipSpec {
    /// `duration` (seconds).
    pub duration: f32,
    /// `numFrames` *N*.
    pub num_frames: u32,
    /// `maxFramesPerBlock` *F*.
    pub max_frames_per_block: u32,
    /// Blocks.
    pub blocks: Vec<BlockSpec>,
}

/// Uniform clamped knots for `points` control points of degree `p` over local frames
/// `0 ..= last` (interior knots `k + (p + 1) / 2`), valid when `points ≤ last + 1`.
pub fn uniform_knots(p: u8, points: usize, last: u8) -> Vec<u8> {
    let p = usize::from(p);
    let n = points.saturating_sub(1);
    let mut k = vec![0u8; p + 1];
    for i in 0..n.saturating_sub(p) {
        k.push((i + p.div_ceil(2).max(1)) as u8);
    }
    k.extend(std::iter::repeat_n(last, p + 1));
    k
}

impl SplineClipSpec {
    /// Frame count of block `b`, *F_b*.
    pub fn block_frames(&self, b: usize) -> u32 {
        let f = self.max_frames_per_block.max(2);
        self.num_frames.saturating_sub(b as u32 * (f - 1)).min(f)
    }

    /// Builds a clip from `pose(frame, track)` at every frame: per block and track,
    /// constant channels where the values do not change, else curves of degree
    /// `min(degree, F_b − 1)` with one control point per frame and uniform knots. Degree 1
    /// reproduces every frame (up to quantisation); higher degrees approximate (exact at
    /// block ends). Seams are continuous because both blocks pass through the shared
    /// frame's value.
    pub fn from_frames(
        num_frames: u32,
        tracks: usize,
        frame_duration: f32,
        max_frames_per_block: u32,
        degree: u8,
        pose: impl Fn(u32, usize) -> Transform,
    ) -> Self {
        let num_frames = num_frames.max(1);
        let f = max_frames_per_block.clamp(2, 256);
        let blocks_n = ((num_frames - 1).div_ceil(f - 1)).max(1);
        let mut spec = SplineClipSpec {
            duration: frame_duration * (num_frames - 1) as f32,
            num_frames,
            max_frames_per_block: f,
            blocks: Vec::new(),
        };
        for b in 0..blocks_n {
            let fb = spec.block_frames(b as usize);
            let first = b * (f - 1);
            let mut ts = Vec::with_capacity(tracks);
            for j in 0..tracks {
                let frames: Vec<Transform> = (0..fb).map(|u| pose(first + u, j)).collect();
                let p = degree.max(1).min((fb - 1).min(255) as u8);
                let last = (fb - 1).min(255) as u8;
                let vec_channel = |get: &dyn Fn(&Transform) -> [f32; 3], default: f32| {
                    let vals: Vec<[f32; 3]> = frames.iter().map(get).collect();
                    let mut dynamic = 0u8;
                    let mut fixed = [None; 3];
                    for c in 0..3 {
                        let v0 = vals[0][c];
                        if vals.iter().any(|v| v[c] != v0) {
                            dynamic |= 1 << c;
                        } else if v0 != default {
                            fixed[c] = Some(v0);
                        }
                    }
                    if dynamic == 0 || fb < 2 {
                        VectorSpec::Constant(fixed)
                    } else {
                        VectorSpec::spline(
                            p,
                            uniform_knots(p, vals.len(), last),
                            &vals,
                            dynamic,
                            fixed,
                        )
                    }
                };
                let position = vec_channel(&|t| t.translation, 0.0);
                let scale = vec_channel(&|t| t.scale, 1.0);
                let q0 = frames[0].rotation;
                let rotation = if frames.iter().all(|t| t.rotation == q0) || fb < 2 {
                    if q0 == [0.0, 0.0, 0.0, 1.0] {
                        RotationSpec::Identity
                    } else {
                        RotationSpec::Static(q0)
                    }
                } else {
                    RotationSpec::Dynamic {
                        degree: p,
                        knots: uniform_knots(p, frames.len(), last),
                        points: frames.iter().map(|t| t.rotation).collect(),
                    }
                };
                ts.push(TrackSpec::new(position, rotation, scale));
            }
            spec.blocks.push(BlockSpec { tracks: ts });
        }
        spec
    }

    /// `data`, `blockOffsets` and `floatBlockOffsets` (§5.2, 16-byte block alignment).
    pub fn data(&self) -> (Vec<u8>, Vec<i32>, Vec<i32>) {
        let mut data = Vec::new();
        let mut offsets = Vec::new();
        let mut lengths = Vec::new();
        for b in &self.blocks {
            offsets.push(data.len() as i32);
            let bytes = encode_block(b);
            lengths.push(bytes.len() as i32);
            data.extend_from_slice(&bytes);
            while !data.len().is_multiple_of(16) {
                data.push(0);
            }
        }
        (data, offsets, lengths)
    }

    fn values(&self) -> Vec<(String, WValue)> {
        let tracks = self.blocks.first().map_or(0, |b| b.tracks.len());
        let f = self.max_frames_per_block;
        let frame_duration = self.duration / (self.num_frames.max(2) - 1) as f32;
        let block_duration = (f.saturating_sub(1)) as f32 * frame_duration;
        let (data, offsets, lengths) = self.data();
        vec![
            (s("numFrames"), WValue::Int(self.num_frames as i32)),
            (s("numBlocks"), WValue::Int(self.blocks.len() as i32)),
            (s("maxFramesPerBlock"), WValue::Int(f as i32)),
            (s("maskAndQuantizationSize"), WValue::Int(4 * tracks as i32)),
            (s("blockDuration"), WValue::Real(block_duration)),
            (
                s("blockInverseDuration"),
                WValue::Real(1.0 / block_duration),
            ),
            (s("frameDuration"), WValue::Real(frame_duration)),
            (s("blockOffsets"), WValue::IntArray(offsets)),
            (s("floatBlockOffsets"), WValue::IntArray(lengths)),
            (s("data"), WValue::Bytes(data)),
        ]
    }
}

/// An uncompressed interleaved clip description (§6).
#[derive(Debug, Clone, PartialEq)]
pub struct InterleavedClipSpec {
    /// `duration` (seconds).
    pub duration: f32,
    /// `numberOfTransformTracks`.
    pub tracks: usize,
    /// Frame-major transforms.
    pub transforms: Vec<Transform>,
}

/// An `hkaDefaultAnimatedReferenceFrame` description (§4.5).
#[derive(Debug, Clone, PartialEq)]
pub struct ReferenceFrameSpec {
    /// Up axis.
    pub up: [f32; 4],
    /// Forward axis.
    pub forward: [f32; 4],
    /// Duration.
    pub duration: f32,
    /// Component count of the sample array (written as given).
    pub components: u8,
    /// Samples.
    pub samples: Vec<[f32; 4]>,
}

impl ReferenceFrameSpec {
    fn values(&self) -> Vec<(String, WValue)> {
        vec![
            (s("up"), WValue::Vec4(self.up)),
            (s("forward"), WValue::Vec4(self.forward)),
            (s("duration"), WValue::Real(self.duration)),
            (
                s("referenceFrameSamples"),
                WValue::Vec4Array {
                    components: self.components,
                    values: self.samples.clone(),
                },
            ),
        ]
    }

    fn declare(w: &mut TagfileWriter, referenced: usize) -> usize {
        w.class("hkaAnimatedReferenceFrame", 0, referenced, &[])
    }

    fn declare_default(w: &mut TagfileWriter, arf: usize) -> usize {
        w.class(
            "hkaDefaultAnimatedReferenceFrame",
            0,
            arf,
            &[
                MemberDef::new("up", 4),
                MemberDef::new("forward", 4),
                MemberDef::new("duration", 3),
                MemberDef::new("referenceFrameSamples", 20),
            ],
        )
    }

    /// A leading reference-frame tagfile (envelope field 4): four classes, one object.
    pub fn to_tagfile(&self) -> Vec<u8> {
        let mut w = TagfileWriter::new();
        let referenced = declare_base_classes(&mut w);
        let arf = Self::declare(&mut w, referenced);
        let def = Self::declare_default(&mut w, arf);
        w.object(def, &self.values());
        w.finish()
    }
}

/// The kind-specific part of an [`AnimationSpec`].
#[derive(Debug, Clone, PartialEq)]
pub enum AnimationKindSpec {
    /// Spline-compressed.
    Spline(SplineClipSpec),
    /// Interleaved.
    Interleaved(InterleavedClipSpec),
}

/// A clip tagfile description: animation, annotation names, embedded reference frame.
#[derive(Debug, Clone, PartialEq)]
pub struct AnimationSpec {
    /// Kind-specific data.
    pub kind: AnimationKindSpec,
    /// Annotation track names (`None` = no `annotationTracks` member).
    pub annotation_names: Option<Vec<String>>,
    /// Embedded reference frame (object 2, referenced by `extractedMotion`).
    pub extracted_motion: Option<ReferenceFrameSpec>,
    /// Member values that replace (by name) or add to the generated ones.
    pub overrides: Vec<(String, WValue)>,
}

impl AnimationSpec {
    /// An animation with no annotations, reference frame or overrides.
    pub fn new(kind: AnimationKindSpec) -> Self {
        AnimationSpec {
            kind,
            annotation_names: None,
            extracted_motion: None,
            overrides: Vec::new(),
        }
    }

    /// `numberOfTransformTracks` this spec writes (before overrides).
    pub fn track_count(&self) -> usize {
        match &self.kind {
            AnimationKindSpec::Spline(sp) => sp.blocks.first().map_or(0, |b| b.tracks.len()),
            AnimationKindSpec::Interleaved(i) => i.tracks,
        }
    }

    /// `duration` this spec writes (before overrides).
    pub fn duration(&self) -> f32 {
        match &self.kind {
            AnimationKindSpec::Spline(sp) => sp.duration,
            AnimationKindSpec::Interleaved(i) => i.duration,
        }
    }

    /// The clip tagfile: seven classes (eight with an embedded reference frame).
    pub fn to_tagfile(&self) -> Vec<u8> {
        let mut w = TagfileWriter::new();
        let referenced = declare_base_classes(&mut w);
        let anim = w.class(
            "hkaAnimation",
            3,
            referenced,
            &[
                MemberDef::new("type", 2),
                MemberDef::new("duration", 3),
                MemberDef::new("numberOfTransformTracks", 2),
                MemberDef::new("numberOfFloatTracks", 2),
                MemberDef::class("extractedMotion", 8, "hkaAnimatedReferenceFrame"),
                MemberDef::class("annotationTracks", 25, "hkaAnnotationTrack"),
            ],
        );
        let arf = ReferenceFrameSpec::declare(&mut w, referenced);
        w.class(
            "hkaAnnotationTrack",
            0,
            0,
            &[
                MemberDef::new("trackName", 10),
                MemberDef::class("annotations", 25, "hkaAnnotationTrackAnnotation"),
            ],
        );
        w.class(
            "hkaAnnotationTrackAnnotation",
            0,
            0,
            &[MemberDef::new("time", 3), MemberDef::new("text", 10)],
        );
        let (concrete, type_code, mut values) = match &self.kind {
            AnimationKindSpec::Spline(sp) => {
                let c = w.class(
                    "hkaSplineCompressedAnimation",
                    0,
                    anim,
                    &[
                        MemberDef::new("numFrames", 2),
                        MemberDef::new("numBlocks", 2),
                        MemberDef::new("maxFramesPerBlock", 2),
                        MemberDef::new("maskAndQuantizationSize", 2),
                        MemberDef::new("blockDuration", 3),
                        MemberDef::new("blockInverseDuration", 3),
                        MemberDef::new("frameDuration", 3),
                        MemberDef::new("blockOffsets", 18),
                        MemberDef::new("floatBlockOffsets", 18),
                        MemberDef::new("transformOffsets", 18),
                        MemberDef::new("floatOffsets", 18),
                        MemberDef::new("data", 17),
                        MemberDef::new("endian", 2),
                    ],
                );
                (c, 3, sp.values())
            }
            AnimationKindSpec::Interleaved(i) => {
                let c = w.class(
                    "hkaInterleavedUncompressedAnimation",
                    0,
                    anim,
                    &[
                        MemberDef::new("transforms", 22),
                        MemberDef::new("floats", 19),
                    ],
                );
                (
                    c,
                    1,
                    vec![(
                        s("transforms"),
                        WValue::Vec12Array(i.transforms.iter().map(Transform::to_vec12).collect()),
                    )],
                )
            }
        };
        let def = self
            .extracted_motion
            .as_ref()
            .map(|_| ReferenceFrameSpec::declare_default(&mut w, arf));
        values.splice(
            0..0,
            [
                (s("type"), WValue::Int(type_code)),
                (s("duration"), WValue::Real(self.duration())),
                (
                    s("numberOfTransformTracks"),
                    WValue::Int(self.track_count() as i32),
                ),
            ],
        );
        if self.extracted_motion.is_some() {
            values.push((s("extractedMotion"), WValue::Object(2)));
        }
        if let Some(names) = &self.annotation_names {
            values.push((
                s("annotationTracks"),
                WValue::StructArray {
                    len: names.len(),
                    columns: vec![(s("trackName"), WValue::CStringArray(names.clone()))],
                },
            ));
        }
        let values = apply_overrides(values, &self.overrides);
        w.object(concrete, &values);
        if let (Some(rf), Some(def)) = (&self.extracted_motion, def) {
            w.object(def, &rf.values());
        }
        w.finish()
    }
}

/// A 0x05 clip record description (§2.3).
#[derive(Debug, Clone, PartialEq)]
pub struct ClipRecordSpec {
    /// Envelope duration; `None` = the animation's.
    pub envelope_duration: Option<f32>,
    /// Envelope track count; `None` = the animation's.
    pub envelope_track_count: Option<u32>,
    /// Leading reference frame (field 4).
    pub leading_reference_frame: Option<ReferenceFrameSpec>,
    /// The clip tagfile.
    pub animation: AnimationSpec,
    /// Track weights (field 8).
    pub weights: Vec<f32>,
}

impl ClipRecordSpec {
    /// A record around `animation` with no reference frame and no weights.
    pub fn new(animation: AnimationSpec) -> Self {
        ClipRecordSpec {
            envelope_duration: None,
            envelope_track_count: None,
            leading_reference_frame: None,
            animation,
            weights: Vec::new(),
        }
    }

    /// The record bytes.
    pub fn to_bytes(&self) -> Vec<u8> {
        let mut out = Vec::new();
        out.extend_from_slice(
            &self
                .envelope_duration
                .unwrap_or(self.animation.duration())
                .to_le_bytes(),
        );
        out.extend_from_slice(
            &self
                .envelope_track_count
                .unwrap_or(self.animation.track_count() as u32)
                .to_le_bytes(),
        );
        match &self.leading_reference_frame {
            None => out.push(0),
            Some(rf) => {
                out.push(1);
                let t = rf.to_tagfile();
                out.extend_from_slice(&(t.len() as u32).to_le_bytes());
                out.extend_from_slice(&t);
            }
        }
        let t = self.animation.to_tagfile();
        out.extend_from_slice(&(t.len() as u32).to_le_bytes());
        out.extend_from_slice(&t);
        write_vle(&mut out, self.weights.len() as u32);
        for w in &self.weights {
            out.extend_from_slice(&w.to_le_bytes());
        }
        out
    }
}

/// The spline clip of §12.3 (one track, one block, five frames).
pub fn example_spline_12_3() -> SplineClipSpec {
    let h = std::f32::consts::FRAC_1_SQRT_2;
    SplineClipSpec {
        duration: 4.0 / 30.0,
        num_frames: 5,
        max_frames_per_block: 256,
        blocks: vec![BlockSpec {
            tracks: vec![TrackSpec::new(
                VectorSpec::Spline {
                    degree: 1,
                    knots: vec![0, 0, 4, 4],
                    components: [
                        ComponentSpec::Static(0.5),
                        ComponentSpec::Dynamic { min: 0.0, max: 2.0 },
                        ComponentSpec::Default,
                    ],
                    eight_bit: false,
                    quantised: vec![0, 65535],
                },
                RotationSpec::Dynamic {
                    degree: 1,
                    knots: vec![0, 0, 4, 4],
                    points: vec![[0.0, 0.0, 0.0, 1.0], [0.0, 0.0, h, h]],
                },
                VectorSpec::DEFAULT,
            )],
        }],
    }
}

/// The interleaved clip of §12.6 (two tracks, three frames, duration 1).
pub fn example_interleaved_12_6() -> InterleavedClipSpec {
    let mut transforms = Vec::new();
    for f in 0..3 {
        for j in 0..2 {
            transforms.push(Transform {
                translation: if j == 0 {
                    [f as f32, 0.0, 0.0]
                } else {
                    [0.0, f as f32, 0.0]
                },
                ..Transform::IDENTITY
            });
        }
    }
    InterleavedClipSpec {
        duration: 1.0,
        tracks: 2,
        transforms,
    }
}

/// The reference frame of §12.7 with sample component count `components` (3 or 4).
pub fn example_reference_frame_12_7(components: u8) -> ReferenceFrameSpec {
    ReferenceFrameSpec {
        up: [0.0, 0.0, 1.0, 0.0],
        forward: [1.0, 0.0, 0.0, 0.0],
        duration: 1.0,
        components,
        samples: vec![
            [0.0, 0.0, 0.0, 0.0],
            [0.5, 0.0, 0.0, 0.1],
            [1.0, 0.0, 0.0, 0.2],
        ],
    }
}

/// The record of §12.8 around the §12.6 clip: duration 1, two tracks, weights 1.0 and 0.5.
pub fn example_record_12_8() -> ClipRecordSpec {
    let mut r = ClipRecordSpec::new(AnimationSpec::new(AnimationKindSpec::Interleaved(
        example_interleaved_12_6(),
    )));
    r.weights = vec![1.0, 0.5];
    r
}

/// A spline clip with every structure the format allows: two blocks (*N* = 300, 60 fps),
/// three tracks mixing default, static and spline channels of degree 1 and 3, 8-bit
/// position on one track, annotation names and an embedded reference frame; wrapped in a
/// record with a leading reference frame and weights. Used to seed fuzz targets.
pub fn example_rich_record() -> ClipRecordSpec {
    let pose = |f: u32, j: usize| {
        let t = f as f32 / 60.0;
        let angle = t * (1.0 + j as f32);
        let (sn, cs) = (angle * 0.5).sin_cos();
        Transform {
            translation: [t * j as f32, (t * 3.0).sin(), 0.25],
            rotation: if j == 0 {
                [0.0, 0.0, 0.0, 1.0]
            } else {
                [0.0, 0.0, sn, cs]
            },
            scale: if j == 2 {
                [1.0 + t * 0.1, 1.0, 1.0]
            } else {
                [1.0; 3]
            },
        }
    };
    let mut sp = SplineClipSpec::from_frames(300, 3, 1.0 / 60.0, 256, 3, pose);
    // One 8-bit channel.
    if let Some(t) = sp.blocks[0].tracks.get_mut(1)
        && let VectorSpec::Spline { eight_bit, .. } = &mut t.position
    {
        *eight_bit = true;
        t.quantisation &= !0x03;
    }
    let mut anim = AnimationSpec::new(AnimationKindSpec::Spline(sp));
    anim.annotation_names = Some(vec![s("t0"), s("t1"), s("t2")]);
    let rf = ReferenceFrameSpec {
        up: [0.0, 0.0, 1.0, 0.0],
        forward: [1.0, 0.0, 0.0, 0.0],
        duration: anim.duration(),
        components: 4,
        samples: (0..300)
            .map(|f| [f as f32 * 0.01, 0.0, 0.0, f as f32 * 0.001])
            .collect(),
    };
    anim.extracted_motion = Some(rf.clone());
    let mut r = ClipRecordSpec::new(anim);
    r.leading_reference_frame = Some(rf);
    r.weights = vec![1.0, 0.5, 0.0];
    r
}

/// A byte cursor that turns arbitrary bytes (a fuzzer's input) into builder parameters.
/// Reads past the end return zeros, so every input drives the builders to completion.
#[derive(Debug, Clone)]
pub struct ByteSource<'a> {
    data: &'a [u8],
    pos: usize,
}

impl<'a> ByteSource<'a> {
    /// Wraps `data`.
    pub fn new(data: &'a [u8]) -> Self {
        ByteSource { data, pos: 0 }
    }

    /// Whether every byte has been consumed.
    pub fn is_empty(&self) -> bool {
        self.pos >= self.data.len()
    }

    /// The bytes not yet consumed.
    pub fn rest(&self) -> &'a [u8] {
        self.data.get(self.pos..).unwrap_or(&[])
    }

    /// Next byte (0 past the end).
    pub fn u8(&mut self) -> u8 {
        let b = self.data.get(self.pos).copied().unwrap_or(0);
        self.pos += 1;
        b
    }

    /// Next little-endian u16.
    pub fn u16(&mut self) -> u16 {
        u16::from_le_bytes([self.u8(), self.u8()])
    }

    /// Next little-endian u32.
    pub fn u32(&mut self) -> u32 {
        u32::from_le_bytes([self.u8(), self.u8(), self.u8(), self.u8()])
    }

    /// Next byte reduced to `0 .. n` (0 when `n` is 0).
    pub fn below(&mut self, n: usize) -> usize {
        if n == 0 {
            self.u8();
            0
        } else {
            usize::from(self.u8()) % n
        }
    }

    /// A small float in −8 … 8 from one byte.
    pub fn small_f32(&mut self) -> f32 {
        (f32::from(self.u8()) - 128.0) / 16.0
    }

    /// A float from four raw bytes (any bit pattern, including NaN and infinities).
    pub fn raw_f32(&mut self) -> f32 {
        f32::from_bits(self.u32())
    }

    /// Mostly small floats, sometimes a raw bit pattern.
    pub fn f32(&mut self) -> f32 {
        if self.u8() < 240 {
            self.small_f32()
        } else {
            self.raw_f32()
        }
    }

    /// A time for sampling: mostly within `0 ..= duration`, sometimes far outside or not
    /// finite.
    pub fn time(&mut self, duration: f32) -> f32 {
        match self.u8() % 8 {
            0 => self.raw_f32(),
            1 => -self.small_f32().abs(),
            2 => duration * 2.0 + self.small_f32(),
            _ => duration * f32::from(self.u8()) / 255.0,
        }
    }
}

/// Damages `bytes` under the control of `src`: up to 8 overwrites, insertions or deletions,
/// then an optional cut. Returns `true` if anything changed.
pub fn damage(bytes: &mut Vec<u8>, src: &mut ByteSource<'_>) -> bool {
    let edits = src.below(9);
    let mut changed = false;
    for _ in 0..edits {
        if bytes.is_empty() {
            break;
        }
        let at = usize::from(src.u16()) % bytes.len();
        let v = src.u8();
        match src.u8() % 4 {
            0 | 1 => {
                changed |= bytes[at] != v;
                bytes[at] = v;
            }
            2 => {
                bytes.insert(at, v);
                changed = true;
            }
            _ => {
                bytes.remove(at);
                changed = true;
            }
        }
    }
    if src.u8().is_multiple_of(8) && !bytes.is_empty() {
        let cut = usize::from(src.u16()) % bytes.len();
        bytes.truncate(cut);
        changed = true;
    }
    changed
}

/// A unit quaternion from three angles.
pub fn quat_from_angles(a: f32, b: f32, c: f32) -> [f32; 4] {
    let (s1, c1) = (a * 0.5).sin_cos();
    let (s2, c2) = (b * 0.5).sin_cos();
    let (s3, c3) = (c * 0.5).sin_cos();
    [
        s1 * c2 * c3 - c1 * s2 * s3,
        c1 * s2 * c3 + s1 * c2 * s3,
        c1 * c2 * s3 - s1 * s2 * c3,
        c1 * c2 * c3 + s1 * s2 * s3,
    ]
}
