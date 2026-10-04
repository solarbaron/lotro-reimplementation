// SPDX-License-Identifier: GPL-3.0-or-later
//! Native (engine) classes of object-state bundles and their built-in codecs.
//!
//! A native object's class index selects its codec (`wstate.md` §5.3). Native bodies have
//! no length prefix, so an unknown class stops the bundle (§5.5).
//!
//! Spec: `docs/specs/wstate.md` §5.3–§5.5.

use tessera_formats::{
    Did, Position, PropertyEntry, PropertySchema, PropertySet, PropertyValue, Reader, StringInfo,
    TSize, Utf16String,
};

use crate::error::{FormatContext, Result, WStateError};
use crate::util::{bounded_vec, charge, counted};

/// A reference value: a label of the same bundle, or null (`wstate.md` §8.1).
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, PartialOrd, Ord)]
pub struct Ref(pub u32);

impl Ref {
    /// The null reference.
    pub const NULL: Self = Self(0xFFFF_FFFF);

    /// Whether this is the null reference.
    #[must_use]
    pub fn is_null(self) -> bool {
        self == Self::NULL
    }

    /// The label, or `None` for null.
    #[must_use]
    pub fn label(self) -> Option<u32> {
        (!self.is_null()).then_some(self.0)
    }
}

/// Header of a "small set" (`wstate.md` §5.3): u16 count, a byte that is 0, a bucket hint.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Default)]
pub struct SmallSetHeader {
    /// The byte after the count (0 in all observed sets; meaning OPEN).
    pub reserved: u8,
    /// TSize-style bucket hint.
    pub hint: u8,
}

/// One element of a `refpair_map` (class 138).
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub struct RefPair {
    /// First field: a reference.
    pub first: Ref,
    /// Second field: a plain value.
    pub value: u32,
    /// Third field: a boolean (0 in all observed elements).
    pub flag: bool,
    /// Last field: a reference.
    pub last: Ref,
}

/// One element of a `map_u32_tagged` (class 131).
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub struct TaggedEntry {
    /// Key.
    pub key: u32,
    /// Value.
    pub value: u32,
    /// Boolean (0 in all observed elements).
    pub flag: bool,
    /// A game-object DID.
    pub did: Did,
}

/// One element of a `weighted_table` (class 175).
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub struct WeightedEntry {
    /// Value.
    pub value: u32,
    /// Weight.
    pub weight: u32,
    /// Third word (meaning OPEN).
    pub unknown: u32,
}

/// `sphere` (class 192), 33 bytes. Field meanings are the spec writer's reading (OPEN).
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct Sphere {
    /// Leading byte (1 in all observed).
    pub flag: u8,
    /// 16-byte identifier.
    pub id: [u8; 16],
    /// Centre offset.
    pub centre: [f32; 3],
    /// Radius.
    pub radius: f32,
}

/// One entry of a `scene_info` (class 427).
#[derive(Debug, Clone, PartialEq)]
pub struct SceneEntry {
    /// 1-based index.
    pub index: u32,
    /// Two floats (meaning OPEN).
    pub floats: [f32; 2],
    /// A word (meaning OPEN).
    pub a: u32,
    /// Text.
    pub text: StringInfo,
    /// Two words (meaning OPEN).
    pub b: [u32; 2],
    /// Boolean (meaning OPEN).
    pub flag: bool,
}

/// `scene_info` (class 427, `wstate.md` §5.4).
#[derive(Debug, Clone, PartialEq)]
pub struct SceneInfo {
    /// Title.
    pub title: StringInfo,
    /// DID of the containing record.
    pub owner: Did,
    /// Entries.
    pub entries: Vec<SceneEntry>,
}

/// Sentinel that starts every scene-info entry.
pub const SCENE_ENTRY_SENTINEL: u32 = 0xFCFC_FCFC;

/// `barter_entry` (class 1942, 49 bytes). Field meanings are OPEN (`wstate.md` §5.4).
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub struct BarterEntry {
    /// Offset 0: DID of the containing record.
    pub owner: Did,
    /// Offset 4: a game-object DID or 0.
    pub target: Did,
    /// Offset 8: small integer.
    pub count: u32,
    /// Offset 12: 0 or 1.
    pub flag: u8,
    /// Offset 13: usually 0; a DID when `target` is 0.
    pub alt_target: u32,
    /// Offset 17: usually 0; 1–5 when `target` is 0.
    pub alt_count: u32,
    /// Offset 21: three (DID or 0, quantity) pairs.
    pub pairs: [(Did, u32); 3],
    /// Offset 45: usually 0.
    pub tail: u32,
}

/// One slot record of an options profile (class 415).
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum OptionSlot {
    /// Kind 0: no payload.
    Empty,
    /// Kind 2: u64 + u32 (Reported).
    Wide(u64, u32),
    /// Kinds 6, 7, 9: one u32 (Reported).
    Word(u32, u32),
}

/// One group of the Reported variable form of an options profile.
#[derive(Debug, Clone, PartialEq)]
pub struct OptionGroup {
    /// Leading word.
    pub id: u32,
    /// A byte.
    pub flag: u8,
    /// Elements: (u32, 4 × f32, u8).
    pub elements: Vec<(u32, [f32; 4], u8)>,
}

/// Trailer of an options profile.
#[derive(Debug, Clone, PartialEq)]
pub enum OptionsTail {
    /// `t1` = 1 and `t2` = 2: 24 raw bytes (the observed form).
    Fixed([u8; 24]),
    /// Reported form: `t2` groups then a u16 and six u32 words (whether the u16 and the
    /// six words follow each group or all groups is OPEN; this reader takes "all").
    Groups {
        /// The groups.
        groups: Vec<OptionGroup>,
        /// The u16 after the groups.
        word: u16,
        /// The six u32 after the u16.
        words: [u32; 6],
    },
}

/// `options_profile` (class 415, `wstate.md` §5.4).
#[derive(Debug, Clone, PartialEq)]
pub struct OptionsProfile {
    /// First word (320 in all observed).
    pub a: u32,
    /// Second word (0 in all observed).
    pub b: u32,
    /// Slot sets.
    pub sets: Vec<Vec<OptionSlot>>,
    /// `t1`.
    pub t1: u8,
    /// `t2`.
    pub t2: u8,
    /// Trailer.
    pub tail: OptionsTail,
}

/// A position entry of the quest-target table (class 2479, parts 1 and 3).
#[derive(Debug, Clone, PartialEq)]
pub struct TargetLocation {
    /// The position.
    pub position: Position,
    /// A word.
    pub a: u32,
    /// A byte.
    pub b: u8,
    /// Content-layer IDs (part 3 only; empty in part 1).
    pub layers: Vec<u32>,
}

/// One condition entry of the quest-target table (class 2479, part 4).
#[derive(Debug, Clone, PartialEq)]
pub struct ConditionEntry {
    /// A DID.
    pub did: Did,
    /// Position.
    pub position: Position,
    /// Radius.
    pub radius: f32,
    /// First Pascal string.
    pub name: String,
    /// Filters: a flag byte and one word for each set bit 0x01–0x40.
    pub filters: Vec<(u8, Vec<u32>)>,
    /// Second Pascal string.
    pub tail: String,
}

/// Conditions of one quest objective: per condition, its entries (class 2479, part 4).
pub type ObjectiveConditions = Vec<Vec<ConditionEntry>>;

/// `quest_target_locations` (class 2479, `wstate.md` §5.4).
#[derive(Debug, Clone, PartialEq, Default)]
pub struct QuestTargets {
    /// Part 1: DID → locations.
    pub locations: Vec<(Did, Vec<TargetLocation>)>,
    /// Part 2: seven tables of (u32, list of u32).
    pub tables: Vec<Vec<(u32, Vec<u32>)>>,
    /// Part 3: DID → locations with content layers.
    pub layered: Vec<(Did, Vec<TargetLocation>)>,
    /// Part 4: quest DID → (objective index → conditions → entries).
    pub quests: Vec<(Did, Vec<(u32, ObjectiveConditions)>)>,
    /// The TSize headers of the hashed lists, kept so a writer can reproduce the layout.
    pub sizes: QuestTargetSizes,
}

/// TSize headers of the hashed lists of a [`QuestTargets`] (`wstate.md` §12: a byte-exact
/// writer keeps every hint).
#[derive(Debug, Clone, PartialEq, Eq, Default)]
pub struct QuestTargetSizes {
    /// Part 1 ([`QuestTargets::locations`]).
    pub locations: TSize,
    /// One per part-2 table ([`QuestTargets::tables`]), same order.
    pub tables: Vec<TSize>,
    /// Part 3 ([`QuestTargets::layered`]).
    pub layered: TSize,
    /// Part 4 ([`QuestTargets::quests`]).
    pub quests: TSize,
    /// One per quest of part 4 (its objective list), same order.
    pub objectives: Vec<TSize>,
}

/// A decoded native object body.
///
/// Container variants keep their TSize/small-set header so a writer could reproduce the
/// layout. Reference-holding containers store [`Ref`] values.
#[derive(Debug, Clone, PartialEq)]
#[non_exhaustive]
pub enum Native {
    /// 11 `map_u32`: (key, value) in bucket order.
    MapU32(TSize, Vec<(u32, u32)>),
    /// 13 `multimap_u32`: keys may repeat.
    MultiMapU32(TSize, Vec<(u32, u32)>),
    /// 35 `refmap_u32`: values are references.
    RefMapU32(TSize, Vec<(u32, Ref)>),
    /// 37 `refmultimap_u32`: keys may repeat, values are references.
    RefMultiMapU32(TSize, Vec<(u32, Ref)>),
    /// 17, 25 `array_u32`: integers, DIDs or float bit patterns.
    ArrayU32(Vec<u32>),
    /// 176, 182 `refarray`: references.
    RefArray(Vec<Ref>),
    /// 18 `set_u32`.
    SetU32(SmallSetHeader, Vec<u32>),
    /// 105 `set_u64`.
    SetU64(SmallSetHeader, Vec<u64>),
    /// 134 `set_triple`: (u32, u32, u8).
    SetTriple(SmallSetHeader, Vec<(u32, u32, u8)>),
    /// 23 `map_u32_u64`.
    MapU32U64(TSize, Vec<(u32, u64)>),
    /// 97 `map_u64_u32`.
    MapU64U32(TSize, Vec<(u64, u32)>),
    /// 117 `refmap_u64`: values are references.
    RefMapU64(TSize, Vec<(u64, Ref)>),
    /// 138 `refpair_map`.
    RefPairMap(TSize, Vec<RefPair>),
    /// 131 `map_u32_tagged`.
    MapU32Tagged(TSize, Vec<TaggedEntry>),
    /// 104, 111 `array_u64`.
    ArrayU64(Vec<u64>),
    /// 101 `multimap_u64_u32` (Reported).
    MultiMapU64U32(Vec<(u64, u32)>),
    /// 39 `property`: one property entry without echo; `None` for a null (PID 0) entry.
    Property(Option<Box<PropertyEntry>>),
    /// 166 `property_set`: a property set without the leading DID.
    PropertySet(Box<PropertySet>),
    /// 57 `bitset`: bit count and bytes (bit `i` is bit `i % 8` of byte `i / 8`).
    Bitset {
        /// Number of bits.
        bits: u32,
        /// ⌈bits / 8⌉ bytes.
        bytes: Vec<u8>,
    },
    /// 160, 161 `position`.
    Position(Position),
    /// 175 `weighted_table`; the trailing word was 0 in all observed.
    WeightedTable(Vec<WeightedEntry>, u32),
    /// 199 `string_info`.
    StringInfo(Box<StringInfo>),
    /// 225 `string` (UTF-16).
    String(Utf16String),
    /// 72 `frame`: seven floats (position + quaternion, presumably; OPEN).
    Frame([f32; 7]),
    /// 192 `sphere`.
    Sphere(Box<Sphere>),
    /// 427 `scene_info`.
    SceneInfo(Box<SceneInfo>),
    /// 1942 `barter_entry`.
    BarterEntry(Box<BarterEntry>),
    /// 1980 `opaque6`: six bytes (OPEN).
    Opaque6([u8; 6]),
    /// 415 `options_profile`.
    OptionsProfile(Box<OptionsProfile>),
    /// 2479 `quest_target_locations`.
    QuestTargets(Box<QuestTargets>),
    /// 407 `compressed_bitset` (Reported): the stored bytes (u32 0, u32 unpacked size,
    /// zlib stream), not inflated.
    CompressedBitset(Vec<u8>),
    /// 2567 `vault_item` (Reported).
    VaultItem {
        /// Instance ID.
        instance: u64,
        /// Property set.
        properties: Box<PropertySet>,
        /// One property.
        property: Option<Box<PropertyEntry>>,
    },
    /// 3103 `vault` (Reported).
    Vault {
        /// Header.
        size: TSize,
        /// (id, name) pairs.
        entries: Vec<(u32, Utf16String)>,
        /// Capacity.
        capacity: u32,
        /// Used.
        used: u32,
    },
    /// 3461 `triple_u32` (Reported).
    TripleU32([u32; 3]),
    /// 3740 `u32` (Reported).
    U32(u32),
}

/// Codec name of a native class index, or `None` if the class has no codec here.
#[must_use]
pub fn codec_name(class: u32) -> Option<&'static str> {
    Some(match class {
        11 => "map_u32",
        13 => "multimap_u32",
        35 => "refmap_u32",
        37 => "refmultimap_u32",
        17 | 25 => "array_u32",
        176 | 182 => "refarray",
        18 => "set_u32",
        105 => "set_u64",
        134 => "set_triple",
        23 => "map_u32_u64",
        97 => "map_u64_u32",
        117 => "refmap_u64",
        138 => "refpair_map",
        131 => "map_u32_tagged",
        104 | 111 => "array_u64",
        101 => "multimap_u64_u32",
        39 => "property",
        166 => "property_set",
        57 => "bitset",
        160 | 161 => "position",
        175 => "weighted_table",
        199 => "string_info",
        225 => "string",
        72 => "frame",
        192 => "sphere",
        427 => "scene_info",
        1942 => "barter_entry",
        1980 => "opaque6",
        415 => "options_profile",
        2479 => "quest_target_locations",
        407 => "compressed_bitset",
        2567 => "vault_item",
        3103 => "vault",
        3461 => "triple_u32",
        3740 => "u32",
        _ => return None,
    })
}

/// Whether `class` has a native codec in this crate.
#[must_use]
pub fn has_codec(class: u32) -> bool {
    codec_name(class).is_some()
}

impl Native {
    /// The references this body holds, in stored order (`wstate.md` §8.1). Plain
    /// containers hold none, even when a value happens to equal a label.
    pub fn references(&self) -> Box<dyn Iterator<Item = Ref> + '_> {
        match self {
            Self::RefMapU32(_, v) | Self::RefMultiMapU32(_, v) => {
                Box::new(v.iter().map(|&(_, r)| r))
            }
            Self::RefMapU64(_, v) => Box::new(v.iter().map(|&(_, r)| r)),
            Self::RefArray(v) => Box::new(v.iter().copied()),
            Self::RefPairMap(_, v) => Box::new(v.iter().flat_map(|p| [p.first, p.last])),
            _ => Box::new(std::iter::empty()),
        }
    }

    /// The position, for `position` bodies.
    #[must_use]
    pub fn as_position(&self) -> Option<&Position> {
        match self {
            Self::Position(p) => Some(p),
            _ => None,
        }
    }

    /// The StringInfo, for `string_info` bodies.
    #[must_use]
    pub fn as_string_info(&self) -> Option<&StringInfo> {
        match self {
            Self::StringInfo(s) => Some(s),
            _ => None,
        }
    }

    /// The text, for `string` bodies.
    #[must_use]
    pub fn as_string(&self) -> Option<&Utf16String> {
        match self {
            Self::String(s) => Some(s),
            _ => None,
        }
    }

    /// The elements, for `array_u32` bodies.
    #[must_use]
    pub fn as_array_u32(&self) -> Option<&[u32]> {
        match self {
            Self::ArrayU32(v) => Some(v),
            _ => None,
        }
    }

    /// The property set, for `property_set` bodies.
    #[must_use]
    pub fn as_property_set(&self) -> Option<&PropertySet> {
        match self {
            Self::PropertySet(s) => Some(s),
            _ => None,
        }
    }

    /// The (key, reference) pairs of a `refmap_u32` or `refmultimap_u32` body.
    #[must_use]
    pub fn as_ref_map(&self) -> Option<&[(u32, Ref)]> {
        match self {
            Self::RefMapU32(_, v) | Self::RefMultiMapU32(_, v) => Some(v),
            _ => None,
        }
    }

    /// The (key, value) pairs of a `map_u32` or `multimap_u32` body.
    #[must_use]
    pub fn as_map_u32(&self) -> Option<&[(u32, u32)]> {
        match self {
            Self::MapU32(_, v) | Self::MultiMapU32(_, v) => Some(v),
            _ => None,
        }
    }
}

/// Outcome of [`read_native`].
pub(crate) enum NativeRead {
    /// The body decoded.
    Body(Native),
    /// No codec for this class index.
    Unknown,
}

const WHAT: &str = "native object";

/// Reads a hashed list: TSize, then `count` elements of at least `min_size` bytes.
fn hashed<'a, T>(
    r: &mut Reader<'a>,
    min_size: usize,
    mut f: impl FnMut(&mut Reader<'a>) -> Result<T>,
) -> Result<(TSize, Vec<T>)> {
    let size = r.tsize().ctx(WHAT)?;
    let mut v = bounded_vec(r, size.count, min_size, WHAT)?;
    for _ in 0..size.count {
        v.push(f(r)?);
    }
    Ok((size, v))
}

/// Reads a small set: u16 count, reserved byte, hint byte, then elements.
fn small_set<'a, T>(
    r: &mut Reader<'a>,
    min_size: usize,
    mut f: impl FnMut(&mut Reader<'a>) -> Result<T>,
) -> Result<(SmallSetHeader, Vec<T>)> {
    let count = r.u16().ctx(WHAT)?;
    let reserved = r.u8().ctx(WHAT)?;
    let hint = r.u8().ctx(WHAT)?;
    let mut v = bounded_vec(r, u32::from(count), min_size, WHAT)?;
    for _ in 0..count {
        v.push(f(r)?);
    }
    Ok((SmallSetHeader { reserved, hint }, v))
}

fn u32_(r: &mut Reader<'_>) -> Result<u32> {
    r.u32().ctx(WHAT)
}

fn u64_(r: &mut Reader<'_>) -> Result<u64> {
    r.u64().ctx(WHAT)
}

fn f32_(r: &mut Reader<'_>) -> Result<f32> {
    r.f32().ctx(WHAT)
}

fn bool_(r: &mut Reader<'_>) -> Result<bool> {
    r.bool8().ctx(WHAT)
}

fn string_info(r: &mut Reader<'_>) -> Result<StringInfo> {
    StringInfo::read(r).ctx("StringInfo in native object")
}

fn position(r: &mut Reader<'_>) -> Result<Position> {
    Position::read(r).ctx("position in native object")
}

/// One property entry without echo (class 39): PID then value; PID 0 is a null entry.
fn property_entry<S: PropertySchema + ?Sized>(
    r: &mut Reader<'_>,
    schema: &S,
) -> Result<Option<Box<PropertyEntry>>> {
    let at = r.position();
    let pid = r.u32().ctx(WHAT)?;
    if pid == 0 {
        return Ok(None);
    }
    let ty = schema
        .property_type(pid)
        .ok_or_else(|| WStateError::Format {
            what: "property in native object",
            source: tessera_formats::FormatError::UnknownProperty { pid, offset: at },
        })?;
    charge(r, std::mem::size_of::<PropertyEntry>(), WHAT)?;
    let value = PropertyValue::read_with(r, ty, schema).ctx("property in native object")?;
    Ok(Some(Box::new(PropertyEntry { pid, value })))
}

fn property_set<S: PropertySchema + ?Sized>(
    r: &mut Reader<'_>,
    schema: &S,
) -> Result<Box<PropertySet>> {
    charge(r, std::mem::size_of::<PropertySet>(), WHAT)?;
    Ok(Box::new(
        PropertySet::read(r, schema).ctx("property set in native object")?,
    ))
}

/// Decodes the body of native class `class` at the reader's position.
pub(crate) fn read_native<S: PropertySchema + ?Sized>(
    r: &mut Reader<'_>,
    class: u32,
    schema: &S,
) -> Result<NativeRead> {
    let body = match class {
        11 => {
            let (s, v) = hashed(r, 8, |r| Ok((u32_(r)?, u32_(r)?)))?;
            Native::MapU32(s, v)
        }
        13 => {
            let (s, v) = hashed(r, 8, |r| Ok((u32_(r)?, u32_(r)?)))?;
            Native::MultiMapU32(s, v)
        }
        35 => {
            let (s, v) = hashed(r, 8, |r| Ok((u32_(r)?, Ref(u32_(r)?))))?;
            Native::RefMapU32(s, v)
        }
        37 => {
            let (s, v) = hashed(r, 8, |r| Ok((u32_(r)?, Ref(u32_(r)?))))?;
            Native::RefMultiMapU32(s, v)
        }
        17 | 25 => Native::ArrayU32(counted(r, 4, WHAT, u32_)?),
        176 | 182 => Native::RefArray(counted(r, 4, WHAT, |r| Ok(Ref(u32_(r)?)))?),
        18 => {
            let (h, v) = small_set(r, 4, u32_)?;
            Native::SetU32(h, v)
        }
        105 => {
            let (h, v) = small_set(r, 8, u64_)?;
            Native::SetU64(h, v)
        }
        134 => {
            let (h, v) = small_set(r, 9, |r| Ok((u32_(r)?, u32_(r)?, r.u8().ctx(WHAT)?)))?;
            Native::SetTriple(h, v)
        }
        23 => {
            let (s, v) = hashed(r, 12, |r| Ok((u32_(r)?, u64_(r)?)))?;
            Native::MapU32U64(s, v)
        }
        97 => {
            let (s, v) = hashed(r, 12, |r| Ok((u64_(r)?, u32_(r)?)))?;
            Native::MapU64U32(s, v)
        }
        117 => {
            let (s, v) = hashed(r, 12, |r| Ok((u64_(r)?, Ref(u32_(r)?))))?;
            Native::RefMapU64(s, v)
        }
        138 => {
            let (s, v) = hashed(r, 13, |r| {
                Ok(RefPair {
                    first: Ref(u32_(r)?),
                    value: u32_(r)?,
                    flag: bool_(r)?,
                    last: Ref(u32_(r)?),
                })
            })?;
            Native::RefPairMap(s, v)
        }
        131 => {
            let (s, v) = hashed(r, 13, |r| {
                Ok(TaggedEntry {
                    key: u32_(r)?,
                    value: u32_(r)?,
                    flag: bool_(r)?,
                    did: Did(u32_(r)?),
                })
            })?;
            Native::MapU32Tagged(s, v)
        }
        104 | 111 => Native::ArrayU64(counted(r, 8, WHAT, u64_)?),
        101 => Native::MultiMapU64U32(counted(r, 12, WHAT, |r| Ok((u64_(r)?, u32_(r)?)))?),
        39 => Native::Property(property_entry(r, schema)?),
        166 => Native::PropertySet(property_set(r, schema)?),
        57 => {
            let bits = r.vle().ctx(WHAT)?;
            let n = bits.div_ceil(8);
            let raw = r
                .bytes(usize::try_from(n).unwrap_or(usize::MAX))
                .ctx(WHAT)?;
            charge(r, raw.len(), WHAT)?;
            Native::Bitset {
                bits,
                bytes: raw.to_vec(),
            }
        }
        160 | 161 => Native::Position(position(r)?),
        175 => {
            let v = counted(r, 12, WHAT, |r| {
                Ok(WeightedEntry {
                    value: u32_(r)?,
                    weight: u32_(r)?,
                    unknown: u32_(r)?,
                })
            })?;
            Native::WeightedTable(v, u32_(r)?)
        }
        199 => {
            charge(r, std::mem::size_of::<StringInfo>(), WHAT)?;
            Native::StringInfo(Box::new(string_info(r)?))
        }
        225 => Native::String(r.utf16_string().ctx(WHAT)?),
        72 => {
            let mut f = [0f32; 7];
            for x in &mut f {
                *x = f32_(r)?;
            }
            Native::Frame(f)
        }
        192 => {
            charge(r, std::mem::size_of::<Sphere>(), WHAT)?;
            let flag = r.u8().ctx(WHAT)?;
            let id = r.array::<16>().ctx(WHAT)?;
            let centre = [f32_(r)?, f32_(r)?, f32_(r)?];
            let radius = f32_(r)?;
            Native::Sphere(Box::new(Sphere {
                flag,
                id,
                centre,
                radius,
            }))
        }
        427 => Native::SceneInfo(scene_info(r)?),
        1942 => {
            charge(r, std::mem::size_of::<BarterEntry>(), WHAT)?;
            let owner = Did(u32_(r)?);
            let target = Did(u32_(r)?);
            let count = u32_(r)?;
            let flag = r.u8().ctx(WHAT)?;
            let alt_target = u32_(r)?;
            let alt_count = u32_(r)?;
            let mut pairs = [(Did(0), 0); 3];
            for p in &mut pairs {
                *p = (Did(u32_(r)?), u32_(r)?);
            }
            let tail = u32_(r)?;
            Native::BarterEntry(Box::new(BarterEntry {
                owner,
                target,
                count,
                flag,
                alt_target,
                alt_count,
                pairs,
                tail,
            }))
        }
        1980 => Native::Opaque6(r.array::<6>().ctx(WHAT)?),
        415 => Native::OptionsProfile(options_profile(r)?),
        2479 => Native::QuestTargets(quest_targets(r)?),
        407 => {
            let size = u32_(r)?;
            let raw = r
                .bytes(usize::try_from(size).unwrap_or(usize::MAX))
                .ctx(WHAT)?;
            charge(r, raw.len(), WHAT)?;
            Native::CompressedBitset(raw.to_vec())
        }
        2567 => {
            let instance = u64_(r)?;
            let properties = property_set(r, schema)?;
            let property = property_entry(r, schema)?;
            Native::VaultItem {
                instance,
                properties,
                property,
            }
        }
        3103 => {
            let (size, entries) = hashed(r, 5, |r| Ok((u32_(r)?, r.utf16_string().ctx(WHAT)?)))?;
            Native::Vault {
                size,
                entries,
                capacity: u32_(r)?,
                used: u32_(r)?,
            }
        }
        3461 => Native::TripleU32([u32_(r)?, u32_(r)?, u32_(r)?]),
        3740 => Native::U32(u32_(r)?),
        _ => return Ok(NativeRead::Unknown),
    };
    Ok(NativeRead::Body(body))
}

/// Smallest stored StringInfo (`records.md` §4.1): an empty literal without arguments
/// (is_literal 1, VLE length 0, has_args 0, terminator `01 00`). A table reference is 12.
const MIN_STRING_INFO: usize = 1 + 1 + 1 + 2;

/// Smallest `scene_info` entry: sentinel 4, index 4, floats 8, word 4, StringInfo, two
/// words 8, bool 1.
const MIN_SCENE_ENTRY: usize = 4 + 4 + 8 + 4 + MIN_STRING_INFO + 8 + 1;

fn scene_info(r: &mut Reader<'_>) -> Result<Box<SceneInfo>> {
    charge(r, std::mem::size_of::<SceneInfo>(), WHAT)?;
    let title = string_info(r)?;
    let owner = Did(u32_(r)?);
    let entries = counted(r, MIN_SCENE_ENTRY, WHAT, |r| {
        let at = r.position();
        let sentinel = u32_(r)?;
        if sentinel != SCENE_ENTRY_SENTINEL {
            return Err(WStateError::malformed(
                "scene info",
                at,
                format!("entry sentinel {sentinel:#010x}"),
            ));
        }
        Ok(SceneEntry {
            index: u32_(r)?,
            floats: [f32_(r)?, f32_(r)?],
            a: u32_(r)?,
            text: string_info(r)?,
            b: [u32_(r)?, u32_(r)?],
            flag: bool_(r)?,
        })
    })?;
    Ok(Box::new(SceneInfo {
        title,
        owner,
        entries,
    }))
}

fn options_profile(r: &mut Reader<'_>) -> Result<Box<OptionsProfile>> {
    charge(r, std::mem::size_of::<OptionsProfile>(), WHAT)?;
    let a = u32_(r)?;
    let b = u32_(r)?;
    let sets = counted(r, 4, WHAT, |r| {
        counted(r, 4, WHAT, |r| {
            let at = r.position();
            let kind = u32_(r)?;
            Ok(match kind {
                0 => OptionSlot::Empty,
                2 => OptionSlot::Wide(u64_(r)?, u32_(r)?),
                6 | 7 | 9 => OptionSlot::Word(kind, u32_(r)?),
                k => {
                    return Err(WStateError::malformed(
                        "options profile",
                        at,
                        format!("unknown slot kind {k}"),
                    ));
                }
            })
        })
    })?;
    let t1 = r.u8().ctx(WHAT)?;
    let t2 = r.u8().ctx(WHAT)?;
    let tail = if t1 == 1 && t2 == 2 {
        OptionsTail::Fixed(r.array::<24>().ctx(WHAT)?)
    } else if t1 <= 1 {
        let mut groups = bounded_vec(r, u32::from(t2), 6, WHAT)?;
        for _ in 0..t2 {
            let id = u32_(r)?;
            let flag = r.u8().ctx(WHAT)?;
            let e = r.u8().ctx(WHAT)?;
            let mut elements = bounded_vec(r, u32::from(e), 21, WHAT)?;
            for _ in 0..e {
                elements.push((
                    u32_(r)?,
                    [f32_(r)?, f32_(r)?, f32_(r)?, f32_(r)?],
                    r.u8().ctx(WHAT)?,
                ));
            }
            groups.push(OptionGroup { id, flag, elements });
        }
        let word = r.u16().ctx(WHAT)?;
        let mut words = [0u32; 6];
        for w in &mut words {
            *w = u32_(r)?;
        }
        OptionsTail::Groups {
            groups,
            word,
            words,
        }
    } else {
        return Err(WStateError::malformed(
            "options profile",
            r.position(),
            format!("unsupported trailer form t1 = {t1}, t2 = {t2}"),
        ));
    };
    Ok(Box::new(OptionsProfile {
        a,
        b,
        sets,
        t1,
        t2,
        tail,
    }))
}

fn locations(r: &mut Reader<'_>, layered: bool) -> Result<Vec<TargetLocation>> {
    // Smallest location: position flag byte + u32 + u8 (+ u32 count when layered).
    let min = if layered { 10 } else { 6 };
    counted(r, min, WHAT, |r| {
        let position = position(r)?;
        let a = u32_(r)?;
        let b = r.u8().ctx(WHAT)?;
        let layers = if layered {
            counted(r, 4, WHAT, u32_)?
        } else {
            Vec::new()
        };
        Ok(TargetLocation {
            position,
            a,
            b,
            layers,
        })
    })
}

fn filter(r: &mut Reader<'_>) -> Result<(u8, Vec<u32>)> {
    let at = r.position();
    let flags = r.u8().ctx(WHAT)?;
    if flags & 0x80 != 0 {
        return Err(WStateError::malformed(
            "quest target filter",
            at,
            "flag bit 0x80 is not supported",
        ));
    }
    let n = flags.count_ones();
    let mut words = bounded_vec(r, n, 4, WHAT)?;
    for _ in 0..n {
        words.push(u32_(r)?);
    }
    Ok((flags, words))
}

fn quest_targets(r: &mut Reader<'_>) -> Result<Box<QuestTargets>> {
    charge(r, std::mem::size_of::<QuestTargets>(), WHAT)?;
    let (locations_size, locs) = hashed(r, 8, |r| Ok((Did(u32_(r)?), locations(r, false)?)))?;
    let at = r.position();
    let k = u32_(r)?;
    if k != 7 {
        tracing::debug!(offset = at, count = k, "quest-target table count is not 7");
    }
    let mut tables = bounded_vec(r, k, 2, WHAT)?;
    let mut table_sizes = bounded_vec(r, k, 2, WHAT)?;
    for _ in 0..k {
        let (size, t) = hashed(r, 8, |r| Ok((u32_(r)?, counted(r, 4, WHAT, u32_)?)))?;
        tables.push(t);
        table_sizes.push(size);
    }
    let (layered_size, layered) = hashed(r, 8, |r| Ok((Did(u32_(r)?), locations(r, true)?)))?;
    let mut objective_sizes = Vec::new();
    let (quests_size, quests) = hashed(r, 6, |r| {
        let quest = Did(u32_(r)?);
        let (size, objectives) = hashed(r, 8, |r| {
            let objective = u32_(r)?;
            let conditions = counted(r, 4, WHAT, |r| {
                // Smallest entry: DID, position flag, radius, empty string, u32 g, empty string.
                counted(r, 15, WHAT, |r| {
                    let did = Did(u32_(r)?);
                    let position = position(r)?;
                    let radius = f32_(r)?;
                    let name = r.pascal_string().ctx(WHAT)?;
                    let filters = counted(r, 1, WHAT, filter)?;
                    let tail = r.pascal_string().ctx(WHAT)?;
                    Ok(ConditionEntry {
                        did,
                        position,
                        radius,
                        name,
                        filters,
                        tail,
                    })
                })
            })?;
            Ok((objective, conditions))
        })?;
        // One TSize per quest; the quest list itself was bounded against the input above.
        charge(r, std::mem::size_of::<TSize>(), WHAT)?;
        objective_sizes.push(size);
        Ok((quest, objectives))
    })?;
    Ok(Box::new(QuestTargets {
        locations: locs,
        tables,
        layered,
        quests,
        sizes: QuestTargetSizes {
            locations: locations_size,
            tables: table_sizes,
            layered: layered_size,
            quests: quests_size,
            objectives: objective_sizes,
        },
    }))
}
