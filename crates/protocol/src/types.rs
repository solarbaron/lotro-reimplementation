// SPDX-License-Identifier: GPL-3.0-or-later
//! Common structures of `tessera-protocol.md` §5.3 and the motion byte of §8.1.

use crate::error::{ProtocolError, Result};
use crate::limits::{
    LANDBLOCK_SIZE, MAX_ANCHOR_OFFSET, MAX_APPEARANCE_SCALE, MAX_EQUIP_SLOT, MAX_EQUIPMENT,
    MAX_LANDBLOCK, MAX_LOCATION_COORDINATE, MAX_LOOK_BYTES, MAX_LOOK_ENTRIES, MAX_LOOK_STRING_LEN,
    MAX_NAME_LEN, MAX_PLACE_LEN, MIN_EQUIP_SLOT, MINOR_LOOKS,
};
use crate::wire::{Reader, TextRule, Writer};

/// A precise, region-qualified position (26 bytes on the wire).
#[derive(Debug, Clone, Copy, PartialEq, Default)]
pub struct Location {
    /// Region number (`coordinates.md` §4). 0 = the abstract region of a world without data.
    pub region: u16,
    /// Metres east, region frame. Finite, |x| ≤ 10⁷.
    pub x: f64,
    /// Metres north. Same rule.
    pub y: f64,
    /// Metres up, absolute height. Same rule.
    pub z: f64,
}

impl Location {
    /// Encoded size.
    pub const LEN: usize = 26;

    /// A location.
    #[must_use]
    pub const fn new(region: u16, x: f64, y: f64, z: f64) -> Self {
        Self { region, x, y, z }
    }

    /// Whether every coordinate is finite and within ±10⁷ m.
    #[must_use]
    pub fn is_valid(&self) -> bool {
        [self.x, self.y, self.z]
            .iter()
            .all(|c| c.is_finite() && c.abs() <= MAX_LOCATION_COORDINATE)
    }

    /// Horizontal distance to `other`, metres (regions are not compared).
    #[must_use]
    pub fn distance_xy(&self, other: &Self) -> f64 {
        (self.x - other.x).hypot(self.y - other.y)
    }

    pub(crate) fn check(&self) -> Result<()> {
        for (field, c) in [
            ("location.x", self.x),
            ("location.y", self.y),
            ("location.z", self.z),
        ] {
            if !c.is_finite() {
                return Err(ProtocolError::NonFinite(field));
            }
            if c.abs() > MAX_LOCATION_COORDINATE {
                return Err(ProtocolError::OutOfRange(field));
            }
        }
        Ok(())
    }

    pub(crate) fn encode(&self, w: &mut Writer<'_>) -> Result<()> {
        self.check()?;
        w.u16(self.region);
        w.f64("location.x", self.x)?;
        w.f64("location.y", self.y)?;
        w.f64("location.z", self.z)
    }

    pub(crate) fn decode(r: &mut Reader<'_>) -> Result<Self> {
        let l = Self {
            region: r.u16()?,
            x: r.f64("location.x")?,
            y: r.f64("location.y")?,
            z: r.f64("location.z")?,
        };
        l.check()?;
        Ok(l)
    }
}

/// The landblock and interior cell that own a position (4 bytes). Its region is the region of
/// the Location it accompanies.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Default)]
pub struct Anchor {
    /// Landblock index east, 0–254.
    pub block_x: u8,
    /// Landblock index north, 0–254.
    pub block_y: u8,
    /// Interior cell, 0 = not known.
    pub cell: u16,
}

impl Anchor {
    /// Encoded size.
    pub const LEN: usize = 4;

    /// An anchor.
    #[must_use]
    pub const fn new(block_x: u8, block_y: u8, cell: u16) -> Self {
        Self {
            block_x,
            block_y,
            cell,
        }
    }

    /// Whether `location` is within ±1,280 m of this block's origin on x and y (§5.3,
    /// `Anchor::owns`). The region is not compared: an Anchor has none of its own.
    #[must_use]
    pub fn owns(&self, location: &Location) -> bool {
        let lx = location.x - LANDBLOCK_SIZE * f64::from(self.block_x);
        let ly = location.y - LANDBLOCK_SIZE * f64::from(self.block_y);
        lx.abs() <= MAX_ANCHOR_OFFSET && ly.abs() <= MAX_ANCHOR_OFFSET
    }

    pub(crate) fn check(&self) -> Result<()> {
        if self.block_x > MAX_LANDBLOCK {
            return Err(ProtocolError::OutOfRange("anchor.block_x"));
        }
        if self.block_y > MAX_LANDBLOCK {
            return Err(ProtocolError::OutOfRange("anchor.block_y"));
        }
        Ok(())
    }

    pub(crate) fn check_owns(&self, location: &Location) -> Result<()> {
        self.check()?;
        if self.owns(location) {
            Ok(())
        } else {
            Err(ProtocolError::AnchorNotOwning)
        }
    }

    pub(crate) fn encode(&self, w: &mut Writer<'_>) -> Result<()> {
        self.check()?;
        w.u8(self.block_x);
        w.u8(self.block_y);
        w.u16(self.cell);
        Ok(())
    }

    pub(crate) fn decode(r: &mut Reader<'_>) -> Result<Self> {
        let a = Self {
            block_x: r.u8()?,
            block_y: r.u8()?,
            cell: r.u16()?,
        };
        a.check()?;
        Ok(a)
    }
}

/// A Location with a heading and an optional owning Anchor (31 or 35 bytes).
#[derive(Debug, Clone, Copy, PartialEq, Default)]
pub struct Pose {
    /// Where.
    pub location: Location,
    /// Radians, 0 = +X east, counter-clockwise seen from above. Any finite value; receivers
    /// reduce it with [`reduce_heading`].
    pub heading: f32,
    /// The anchor, which must own `location`.
    pub anchor: Option<Anchor>,
}

impl Pose {
    pub(crate) fn encode(&self, w: &mut Writer<'_>) -> Result<()> {
        self.location.encode(w)?;
        w.f32("pose.heading", self.heading)?;
        match &self.anchor {
            None => w.u8(0),
            Some(a) => {
                a.check_owns(&self.location)?;
                w.u8(1);
                a.encode(w)?;
            }
        }
        Ok(())
    }

    pub(crate) fn decode(r: &mut Reader<'_>) -> Result<Self> {
        let location = Location::decode(r)?;
        let heading = r.f32("pose.heading")?;
        let anchor = if r.opt("pose.anchor")? {
            let a = Anchor::decode(r)?;
            a.check_owns(&location)?;
            Some(a)
        } else {
            None
        };
        Ok(Self {
            location,
            heading,
            anchor,
        })
    }
}

/// `heading` reduced to (−π, π]. Non-finite input gives 0.
#[must_use]
pub fn reduce_heading(heading: f32) -> f32 {
    if !heading.is_finite() {
        return 0.0;
    }
    let tau = std::f32::consts::TAU;
    let pi = std::f32::consts::PI;
    let mut h = heading.rem_euclid(tau); // [0, τ)
    if h > pi {
        h -= tau;
    }
    h
}

/// Three finite f32 (12 bytes). Velocities are in m/s in the region frame.
#[derive(Debug, Clone, Copy, PartialEq, Default)]
pub struct Vec3 {
    /// X.
    pub x: f32,
    /// Y.
    pub y: f32,
    /// Z.
    pub z: f32,
}

impl Vec3 {
    /// Encoded size.
    pub const LEN: usize = 12;
    /// The zero vector.
    pub const ZERO: Self = Self::new(0.0, 0.0, 0.0);

    /// A vector.
    #[must_use]
    pub const fn new(x: f32, y: f32, z: f32) -> Self {
        Self { x, y, z }
    }

    /// Length of the horizontal (x, y) part.
    #[must_use]
    pub fn horizontal_length(&self) -> f32 {
        self.x.hypot(self.y)
    }

    pub(crate) fn encode(&self, w: &mut Writer<'_>, field: &'static str) -> Result<()> {
        w.f32(field, self.x)?;
        w.f32(field, self.y)?;
        w.f32(field, self.z)
    }

    pub(crate) fn decode(r: &mut Reader<'_>, field: &'static str) -> Result<Self> {
        Ok(Self {
            x: r.f32(field)?,
            y: r.f32(field)?,
            z: r.f32(field)?,
        })
    }
}

/// What a figure is made of (5 or 9 bytes).
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum Body {
    /// Tag 1: a playable body by the data's species key and sex value (opaque).
    Character {
        /// Species key.
        species: u32,
        /// Sex value.
        sex: u32,
    },
    /// Tag 2: a data record that describes the figure (opaque DID, non-zero).
    Object {
        /// The DID.
        did: u32,
    },
}

impl Body {
    pub(crate) fn encode(&self, w: &mut Writer<'_>) -> Result<()> {
        match *self {
            Self::Character { species, sex } => {
                w.u8(1);
                w.u32(species);
                w.u32(sex);
            }
            Self::Object { did } => {
                if did == 0 {
                    return Err(ProtocolError::OutOfRange("body.did"));
                }
                w.u8(2);
                w.u32(did);
            }
        }
        Ok(())
    }

    pub(crate) fn decode(r: &mut Reader<'_>) -> Result<Self> {
        match r.u8()? {
            1 => Ok(Self::Character {
                species: r.u32()?,
                sex: r.u32()?,
            }),
            2 => {
                let did = r.u32()?;
                if did == 0 {
                    return Err(ProtocolError::OutOfRange("body.did"));
                }
                Ok(Self::Object { did })
            }
            t => Err(ProtocolError::InvalidEnum {
                field: "body.tag",
                value: u64::from(t),
            }),
        }
    }
}

/// One equipped item (5, 9 or 13 bytes; at minor ≥ 1 also 6, 10 or 14).
#[derive(Debug, Clone, Copy, PartialEq, Default)]
pub struct Equipped {
    /// DID of the item record, non-zero (opaque).
    pub item: u32,
    /// Primary dye code in [0, 1] (flag bit 0).
    pub dye: Option<f32>,
    /// Secondary dye code in [0, 1] (flag bit 1).
    pub secondary_dye: Option<f32>,
    /// Stowed (flag bit 2).
    pub stowed: bool,
    /// The data's equipment slot key that holds the item, 1–64 (flag bit 3, minor ≥ 1 only;
    /// §5.3, §7.4). At minor 0 it is neither sent nor received.
    pub slot: Option<u8>,
}

fn check_dye(field: &'static str, v: f32) -> Result<()> {
    if !v.is_finite() {
        return Err(ProtocolError::NonFinite(field));
    }
    if !(0.0..=1.0).contains(&v) {
        return Err(ProtocolError::OutOfRange(field));
    }
    Ok(())
}

fn check_slot(slot: u8) -> Result<u8> {
    if (MIN_EQUIP_SLOT..=MAX_EQUIP_SLOT).contains(&slot) {
        Ok(slot)
    } else {
        Err(ProtocolError::OutOfRange("equipped.slot"))
    }
}

impl Equipped {
    pub(crate) fn encode(&self, w: &mut Writer<'_>) -> Result<()> {
        if self.item == 0 {
            return Err(ProtocolError::OutOfRange("equipped.item"));
        }
        // At minor 0 the slot is left off: the v1.0 layout exactly (§15 invariant 10).
        let slot = if w.minor() >= MINOR_LOOKS {
            self.slot.map(check_slot).transpose()?
        } else {
            None
        };
        w.u32(self.item);
        let flags = u8::from(self.dye.is_some())
            | u8::from(self.secondary_dye.is_some()) << 1
            | u8::from(self.stowed) << 2
            | u8::from(slot.is_some()) << 3;
        w.u8(flags);
        if let Some(d) = self.dye {
            check_dye("equipped.dye", d)?;
            w.f32("equipped.dye", d)?;
        }
        if let Some(d) = self.secondary_dye {
            check_dye("equipped.secondary_dye", d)?;
            w.f32("equipped.secondary_dye", d)?;
        }
        if let Some(s) = slot {
            w.u8(s);
        }
        Ok(())
    }

    pub(crate) fn decode(r: &mut Reader<'_>) -> Result<Self> {
        let item = r.u32()?;
        if item == 0 {
            return Err(ProtocolError::OutOfRange("equipped.item"));
        }
        let flags = r.u8()?;
        let reserved = if r.minor() >= MINOR_LOOKS { 0xF0 } else { 0xF8 };
        if flags & reserved != 0 {
            return Err(ProtocolError::ReservedBits("equipped.flags"));
        }
        let dye = if flags & 1 != 0 {
            let d = r.f32("equipped.dye")?;
            check_dye("equipped.dye", d)?;
            Some(d)
        } else {
            None
        };
        let secondary_dye = if flags & 2 != 0 {
            let d = r.f32("equipped.secondary_dye")?;
            check_dye("equipped.secondary_dye", d)?;
            Some(d)
        } else {
            None
        };
        let slot = if flags & 8 != 0 {
            Some(check_slot(r.u8()?)?)
        } else {
            None
        };
        Ok(Self {
            item,
            dye,
            secondary_dye,
            stowed: flags & 4 != 0,
            slot,
        })
    }
}

/// The value of one look entry (minor ≥ 1, §5.3): a tagged value of §7.2 restricted to the
/// ten tags a look allows.
#[derive(Debug, Clone, PartialEq)]
pub enum LookValue {
    /// Tag 0x01.
    Bool(bool),
    /// Tag 0x02.
    Int32(i32),
    /// Tag 0x03.
    Int64(i64),
    /// Tag 0x04, finite.
    Float32(f32),
    /// Tag 0x05: an enum key (opaque).
    Enum(u32),
    /// Tag 0x06: a DID (opaque).
    DataId(u32),
    /// Tag 0x07: 0–128 bytes, each 0x21–0x7E (printable ASCII without space).
    String(String),
    /// Tag 0x09.
    Bitfield32(u32),
    /// Tag 0x0A.
    Bitfield64(u64),
    /// Tag 0x0D: R, G, B, A.
    Colour([u8; 4]),
}

/// Whether `s` is a valid look string: 0–128 bytes, each 0x21–0x7E (§5.3).
#[must_use]
pub fn is_look_string(s: &str) -> bool {
    s.len() <= MAX_LOOK_STRING_LEN && s.bytes().all(|b| (0x21..=0x7E).contains(&b))
}

impl LookValue {
    /// The §7.2 tag byte.
    #[must_use]
    pub fn tag(&self) -> u8 {
        match self {
            Self::Bool(_) => 0x01,
            Self::Int32(_) => 0x02,
            Self::Int64(_) => 0x03,
            Self::Float32(_) => 0x04,
            Self::Enum(_) => 0x05,
            Self::DataId(_) => 0x06,
            Self::String(_) => 0x07,
            Self::Bitfield32(_) => 0x09,
            Self::Bitfield64(_) => 0x0A,
            Self::Colour(_) => 0x0D,
        }
    }

    /// Bytes of the tagged encoding (tag included).
    #[must_use]
    pub fn encoded_len(&self) -> usize {
        1 + match self {
            Self::Bool(_) => 1,
            Self::Int32(_) | Self::Float32(_) | Self::Enum(_) | Self::DataId(_) => 4,
            Self::Bitfield32(_) | Self::Colour(_) => 4,
            Self::Int64(_) | Self::Bitfield64(_) => 8,
            Self::String(s) => 2 + s.len(),
        }
    }

    fn check(&self) -> Result<()> {
        match self {
            Self::Float32(v) if !v.is_finite() => Err(ProtocolError::NonFinite("look.value")),
            Self::String(s) if s.len() > MAX_LOOK_STRING_LEN => Err(ProtocolError::StringLength {
                field: "look.value",
                len: s.len(),
                min: 0,
                max: MAX_LOOK_STRING_LEN,
            }),
            Self::String(s) if !is_look_string(s) => Err(ProtocolError::LookString),
            _ => Ok(()),
        }
    }

    fn encode(&self, w: &mut Writer<'_>) -> Result<()> {
        self.check()?;
        w.u8(self.tag());
        match self {
            Self::Bool(b) => w.bool(*b),
            Self::Int32(v) => w.i32(*v),
            Self::Int64(v) => w.i64(*v),
            Self::Float32(v) => w.f32("look.value", *v)?,
            Self::Enum(v) | Self::DataId(v) | Self::Bitfield32(v) => w.u32(*v),
            Self::Bitfield64(v) => w.u64(*v),
            Self::String(s) => {
                // len <= 128 (checked above).
                w.u16(u16::try_from(s.len()).unwrap_or(u16::MAX));
                w.raw(s.as_bytes());
            }
            Self::Colour(c) => w.raw(c),
        }
        Ok(())
    }

    fn decode(r: &mut Reader<'_>) -> Result<Self> {
        let tag = r.u8()?;
        Ok(match tag {
            0x01 => Self::Bool(r.bool("look.value")?),
            0x02 => Self::Int32(r.i32()?),
            0x03 => Self::Int64(r.i64()?),
            0x04 => Self::Float32(r.f32("look.value")?),
            0x05 => Self::Enum(r.u32()?),
            0x06 => Self::DataId(r.u32()?),
            0x07 => {
                let len = usize::from(r.u16()?);
                if len > MAX_LOOK_STRING_LEN {
                    return Err(ProtocolError::StringLength {
                        field: "look.value",
                        len,
                        min: 0,
                        max: MAX_LOOK_STRING_LEN,
                    });
                }
                let raw = r.bytes(len)?;
                if !raw.iter().all(|b| (0x21..=0x7E).contains(b)) {
                    return Err(ProtocolError::LookString);
                }
                // Printable ASCII is UTF-8.
                Self::String(String::from_utf8_lossy(raw).into_owned())
            }
            0x09 => Self::Bitfield32(r.u32()?),
            0x0A => Self::Bitfield64(r.u64()?),
            0x0D => {
                let b = r.bytes(4)?;
                Self::Colour([b[0], b[1], b[2], b[3]])
            }
            other => return Err(ProtocolError::LookTag(other)),
        })
    }
}

/// One entry of a [`Look`]: an opaque data property ID of the client's install and a value
/// (minor ≥ 1, §5.3).
#[derive(Debug, Clone, PartialEq)]
pub struct LookEntry {
    /// The property ID, non-zero (opaque; unrelated to the protocol's own property IDs).
    pub property: u32,
    /// The value.
    pub value: LookValue,
}

/// A character's creation choices (minor ≥ 1, §5.3, §7.4): 0–48 entries with strictly
/// ascending property IDs, at most 768 bytes encoded. Empty means "no look".
#[derive(Debug, Clone, PartialEq, Default)]
pub struct Look {
    /// The entries, `property` strictly ascending.
    pub entries: Vec<LookEntry>,
}

impl Look {
    /// The empty look ("none").
    #[must_use]
    pub const fn none() -> Self {
        Self {
            entries: Vec::new(),
        }
    }

    /// Whether the look has no entry.
    #[must_use]
    pub fn is_empty(&self) -> bool {
        self.entries.is_empty()
    }

    /// Bytes of the encoding: one count byte, then per entry 4 bytes of ID and the tagged
    /// value.
    #[must_use]
    pub fn encoded_len(&self) -> usize {
        1 + self
            .entries
            .iter()
            .map(|e| 4 + e.value.encoded_len())
            .sum::<usize>()
    }

    /// Checks every wire rule of §5.3: at most 48 entries, IDs non-zero and strictly
    /// ascending, allowed values, at most 768 bytes.
    ///
    /// # Errors
    ///
    /// The first rule broken.
    pub fn check(&self) -> Result<()> {
        if self.entries.len() > MAX_LOOK_ENTRIES {
            return Err(ProtocolError::Count {
                field: "look",
                count: self.entries.len(),
                min: 0,
                max: MAX_LOOK_ENTRIES,
            });
        }
        let mut last = 0u32;
        for e in &self.entries {
            if e.property <= last {
                return Err(ProtocolError::IdOrder("look"));
            }
            last = e.property;
            e.value.check()?;
        }
        let len = self.encoded_len();
        if len > MAX_LOOK_BYTES {
            return Err(ProtocolError::LookTooLarge(len));
        }
        Ok(())
    }

    pub(crate) fn encode(&self, w: &mut Writer<'_>) -> Result<()> {
        self.check()?;
        // <= 48 entries (checked above).
        w.u8(u8::try_from(self.entries.len()).unwrap_or(u8::MAX));
        for e in &self.entries {
            w.u32(e.property);
            e.value.encode(w)?;
        }
        Ok(())
    }

    pub(crate) fn decode(r: &mut Reader<'_>) -> Result<Self> {
        let start = r.remaining();
        let n = usize::from(r.u8()?);
        if n > MAX_LOOK_ENTRIES {
            return Err(ProtocolError::Count {
                field: "look",
                count: n,
                min: 0,
                max: MAX_LOOK_ENTRIES,
            });
        }
        let mut entries = Vec::with_capacity(n);
        let mut last = 0u32;
        for _ in 0..n {
            let property = r.u32()?;
            if property <= last {
                return Err(ProtocolError::IdOrder("look"));
            }
            last = property;
            let value = LookValue::decode(r)?;
            let used = start - r.remaining();
            if used > MAX_LOOK_BYTES {
                return Err(ProtocolError::LookTooLarge(used));
            }
            entries.push(LookEntry { property, value });
        }
        Ok(Self { entries })
    }
}

/// How a figure looks (§5.3, §7.4). Every copy is a full replacement.
#[derive(Debug, Clone, PartialEq)]
pub struct Appearance {
    /// The body.
    pub body: Body,
    /// Uniform scale in (0, 100].
    pub scale: f32,
    /// The data's starting-idle enum key (opaque).
    pub idle: Option<u32>,
    /// At most 32 items.
    pub equipment: Vec<Equipped>,
}

impl Appearance {
    /// An appearance with scale 1, no idle and no equipment.
    #[must_use]
    pub fn new(body: Body) -> Self {
        Self {
            body,
            scale: 1.0,
            idle: None,
            equipment: Vec::new(),
        }
    }

    fn check_scale(scale: f32) -> Result<()> {
        if !scale.is_finite() {
            return Err(ProtocolError::NonFinite("appearance.scale"));
        }
        if !(scale > 0.0 && scale <= MAX_APPEARANCE_SCALE) {
            return Err(ProtocolError::OutOfRange("appearance.scale"));
        }
        Ok(())
    }

    pub(crate) fn encode(&self, w: &mut Writer<'_>) -> Result<()> {
        self.body.encode(w)?;
        Self::check_scale(self.scale)?;
        w.f32("appearance.scale", self.scale)?;
        match self.idle {
            None => w.u8(0),
            Some(i) => {
                w.u8(1);
                w.u32(i);
            }
        }
        let n = self.equipment.len();
        if n > MAX_EQUIPMENT {
            return Err(ProtocolError::Count {
                field: "appearance.equipment",
                count: n,
                min: 0,
                max: MAX_EQUIPMENT,
            });
        }
        // n <= 32.
        w.u8(u8::try_from(n).unwrap_or(u8::MAX));
        for e in &self.equipment {
            e.encode(w)?;
        }
        Ok(())
    }

    pub(crate) fn decode(r: &mut Reader<'_>) -> Result<Self> {
        let body = Body::decode(r)?;
        let scale = r.f32("appearance.scale")?;
        Self::check_scale(scale)?;
        let idle = if r.opt("appearance.idle")? {
            Some(r.u32()?)
        } else {
            None
        };
        let n = usize::from(r.u8()?);
        if n > MAX_EQUIPMENT {
            return Err(ProtocolError::Count {
                field: "appearance.equipment",
                count: n,
                min: 0,
                max: MAX_EQUIPMENT,
            });
        }
        let mut equipment = Vec::with_capacity(n);
        for _ in 0..n {
            equipment.push(Equipped::decode(r)?);
        }
        Ok(Self {
            body,
            scale,
            idle,
            equipment,
        })
    }
}

/// One entry of a character list: `name` (`NAME`), then `body`.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct CharacterSummary {
    /// The character's name.
    pub name: String,
    /// The character's body.
    pub body: Body,
}

pub(crate) const NAME_S2C: TextRule = TextRule::sanitised("name", 1, MAX_NAME_LEN);

impl CharacterSummary {
    pub(crate) fn encode(&self, w: &mut Writer<'_>) -> Result<()> {
        w.str(NAME_S2C, &self.name)?;
        self.body.encode(w)
    }

    pub(crate) fn decode(r: &mut Reader<'_>) -> Result<Self> {
        Ok(Self {
            name: r.str(NAME_S2C)?,
            body: Body::decode(r)?,
        })
    }
}

/// Where a teleport goes (§5.3).
#[derive(Debug, Clone, PartialEq)]
pub enum Destination {
    /// Tag 1: an exact outdoor position.
    Location {
        /// Where.
        location: Location,
        /// Facing.
        heading: f32,
    },
    /// Tag 2: outdoor; the server replaces `z` with its terrain height if it knows one.
    Ground {
        /// Where.
        location: Location,
        /// Facing.
        heading: f32,
    },
    /// Tag 3: an interior, never snapped or re-homed. The anchor must own the location.
    Anchored {
        /// Where.
        location: Location,
        /// Facing.
        heading: f32,
        /// The owning anchor.
        anchor: Anchor,
    },
    /// Tag 4: a place the server knows by name (`PLACE`; content checked by the server).
    Named {
        /// The place name.
        name: String,
    },
}

const PLACE_C2S: TextRule = TextRule::raw("destination.name", MAX_PLACE_LEN);

impl Destination {
    pub(crate) fn encode(&self, w: &mut Writer<'_>) -> Result<()> {
        match self {
            Self::Location { location, heading } | Self::Ground { location, heading } => {
                w.u8(if matches!(self, Self::Location { .. }) {
                    1
                } else {
                    2
                });
                location.encode(w)?;
                w.f32("destination.heading", *heading)
            }
            Self::Anchored {
                location,
                heading,
                anchor,
            } => {
                anchor.check_owns(location)?;
                w.u8(3);
                location.encode(w)?;
                w.f32("destination.heading", *heading)?;
                anchor.encode(w)
            }
            Self::Named { name } => {
                w.u8(4);
                w.str(PLACE_C2S, name)
            }
        }
    }

    pub(crate) fn decode(r: &mut Reader<'_>) -> Result<Self> {
        match r.u8()? {
            tag @ (1 | 2) => {
                let location = Location::decode(r)?;
                let heading = r.f32("destination.heading")?;
                Ok(if tag == 1 {
                    Self::Location { location, heading }
                } else {
                    Self::Ground { location, heading }
                })
            }
            3 => {
                let location = Location::decode(r)?;
                let heading = r.f32("destination.heading")?;
                let anchor = Anchor::decode(r)?;
                anchor.check_owns(&location)?;
                Ok(Self::Anchored {
                    location,
                    heading,
                    anchor,
                })
            }
            4 => Ok(Self::Named {
                name: r.str(PLACE_C2S)?,
            }),
            t => Err(ProtocolError::InvalidEnum {
                field: "destination.tag",
                value: u64::from(t),
            }),
        }
    }
}

/// Gait, bits 0–2 of the motion byte (§8.1).
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Default)]
pub enum Gait {
    /// 0.
    #[default]
    Standing,
    /// 1.
    Walking,
    /// 2.
    Running,
    /// 3: jumping or falling.
    Airborne,
}

/// The motion byte (§8.1): gait and the backward flag.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Default)]
pub struct Motion {
    /// Gait.
    pub gait: Gait,
    /// Moving opposite to the heading (bit 3).
    pub backward: bool,
}

impl Motion {
    /// At rest.
    pub const STANDING: Self = Self {
        gait: Gait::Standing,
        backward: false,
    };

    /// A motion value.
    #[must_use]
    pub const fn new(gait: Gait, backward: bool) -> Self {
        Self { gait, backward }
    }

    /// The wire byte.
    #[must_use]
    pub fn to_byte(self) -> u8 {
        let g = match self.gait {
            Gait::Standing => 0,
            Gait::Walking => 1,
            Gait::Running => 2,
            Gait::Airborne => 3,
        };
        g | u8::from(self.backward) << 3
    }

    /// Parses the wire byte. Gait values 4–7 and reserved bits 4–7 are violations.
    pub fn from_byte(b: u8) -> Result<Self> {
        if b & 0xF0 != 0 {
            return Err(ProtocolError::ReservedBits("motion"));
        }
        let gait = match b & 7 {
            0 => Gait::Standing,
            1 => Gait::Walking,
            2 => Gait::Running,
            3 => Gait::Airborne,
            g => {
                return Err(ProtocolError::InvalidEnum {
                    field: "motion.gait",
                    value: u64::from(g),
                });
            }
        };
        Ok(Self {
            gait,
            backward: b & 8 != 0,
        })
    }

    /// Derives the gait from the controller (§8.1): Airborne when not on the ground;
    /// otherwise Standing below 0.15 m/s horizontal speed, Running above the midpoint of
    /// `walk_speed` and `run_speed`, Walking in between.
    #[must_use]
    pub fn derive(
        on_ground: bool,
        horizontal_speed: f32,
        walk_speed: f32,
        run_speed: f32,
        backward: bool,
    ) -> Self {
        let gait = if !on_ground {
            Gait::Airborne
        } else if horizontal_speed.is_nan() || horizontal_speed < crate::limits::STANDING_SPEED {
            Gait::Standing
        } else if horizontal_speed > (walk_speed + run_speed) * 0.5 {
            Gait::Running
        } else {
            Gait::Walking
        };
        Self { gait, backward }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn motion_bytes() {
        for b in 0..=255u8 {
            match Motion::from_byte(b) {
                Ok(m) => assert_eq!(m.to_byte(), b),
                Err(_) => assert!(b & 0xF0 != 0 || b & 7 > 3),
            }
        }
        assert_eq!(
            Motion::derive(true, 5.0, 2.0, 6.0, false).gait,
            Gait::Running
        );
        assert_eq!(
            Motion::derive(true, 3.0, 2.0, 6.0, false).gait,
            Gait::Walking
        );
        assert_eq!(
            Motion::derive(true, 0.1, 2.0, 6.0, false).gait,
            Gait::Standing
        );
        assert_eq!(
            Motion::derive(false, 0.0, 2.0, 6.0, false).gait,
            Gait::Airborne
        );
    }

    #[test]
    fn headings() {
        use std::f32::consts::PI;
        assert!((reduce_heading(3.0 * PI) - PI).abs() < 1e-5);
        assert!((reduce_heading(-PI) - PI).abs() < 1e-5);
        assert_eq!(reduce_heading(0.5), 0.5);
        assert_eq!(reduce_heading(f32::NAN), 0.0);
    }

    #[test]
    fn anchor_ownership() {
        let l = Location::new(1, 20_250.5, 22_500.25, 401.0);
        assert!(Anchor::new(126, 140, 257).owns(&l));
        assert!(!Anchor::new(100, 140, 0).owns(&l));
    }
}
