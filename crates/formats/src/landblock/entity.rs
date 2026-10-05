// SPDX-License-Identifier: GPL-3.0-or-later
//! Static entity descriptors (SEDs): the placed objects inside landblock info.
//!
//! Spec: `docs/specs/entity-descriptor.md` §3 (layout), §4 (fields), §5–§7 (placement),
//! §10 (algorithm and limits).

use super::{LANDBLOCK_SIZE, put, read_embedded_set};
use crate::did::Did;
use crate::error::{FormatError, Result};
use crate::property::{PropertySchema, PropertyValue};
use crate::property_set::PropertySet;
use crate::reader::Reader;
use crate::setup::Quaternion;

/// Largest static-entity count a landblock-info record may declare (the largest observed is
/// 7,801; `entity-descriptor.md` §10).
pub const MAX_STATIC_ENTITIES: u32 = 65_536;

/// Every SED flag bit with a known payload size (0x001–0x400). Any bit ≥ 0x800 is rejected:
/// its payload size is unknown, so the rest of the record would be misframed (§3.3).
pub const SED_FLAGS_KNOWN: u32 = 0x7FF;

const F_DID: u32 = 0x001;
const F_TYPE: u32 = 0x002;
const F_IID: u32 = 0x004;
const F_OBJECT: u32 = 0x008;
const F_PLACEMENT: u32 = 0x010;
const F_VECTOR: u32 = 0x020;
const F_WORD: u32 = 0x040;
const F_PROPERTIES: u32 = 0x080;
const F_NAME: u32 = 0x100;
const F_DIDS: u32 = 0x200;
const F_INTERIOR: u32 = 0x400;
/// Bits set in every observed SED (ED1).
const F_ALWAYS: u32 = F_TYPE | F_IID | F_PROPERTIES;
/// Bits only reported by third parties, never observed.
const F_REPORTED_ONLY: u32 = F_DID | F_VECTOR | F_DIDS;

/// Every flag bit that carries a payload (0x001–0x200); the encoder derives them from the
/// fields. Bit 0x400 has no payload and is kept from [`StaticEntity::flags`].
const F_PAYLOAD: u32 = F_DID
    | F_TYPE
    | F_IID
    | F_OBJECT
    | F_PLACEMENT
    | F_VECTOR
    | F_WORD
    | F_PROPERTIES
    | F_NAME
    | F_DIDS;

/// Stored rotations further than this from unit length are reported as renormalised
/// (§10; never observed). Every rotation is normalised regardless.
const RENORMALISE_TOLERANCE: f32 = 0.01;
/// ED5 tolerance: every observed rotation is unit length within ±0.001 (§11).
const ED5_TOLERANCE: f32 = 0.001;

/// Largest absolute value a SED position component may have, in world units. Observed
/// positions lie within about ±960 of the landblock in x, y and at terrain heights in z
/// (§4.4), so this is far beyond real data; it keeps hostile values such as 3e38 out of
/// world matrices, bounds and spatial buckets. A larger component is malformed
/// (`entity-descriptor.md` §10, decoder policy).
pub const MAX_PLACEMENT_COORD: f32 = 1.0e6;
/// Smallest scale component [`StaticEntity::scale`] accepts (observed: 0.01; §7).
pub const MIN_SCALE: f32 = 1.0e-4;
/// Largest scale component [`StaticEntity::scale`] accepts (observed: 100; §7).
pub const MAX_SCALE: f32 = 1.0e4;

/// The SED `type` field (§4.1): an enum value labelled by enum mapper 0x23000091 at run
/// time. Named variants are the values observed on SEDs; any other is kept as a number.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum StaticEntityType {
    /// 2: projected decal (marker object; parameters in properties).
    Decal,
    /// 3: spawn point (editor marker).
    GeneratorPoint,
    /// 6: server-side object placement; `object` names a 0x70 game object.
    Weenie,
    /// 7: placed scenery and props: the only type with geometry to draw (§9).
    Physics,
    /// 9: light source (marker; parameters in properties).
    Light,
    /// 16: arrival point (no object).
    TeleportDestination,
    /// 19: path segment node (marker).
    SegmentNode,
    /// 23: AI patrol waypoint (marker).
    PatrolPoint,
    /// 31: meaning unknown (marker).
    LbiOnly,
    /// Any other value.
    Other(u32),
}

impl StaticEntityType {
    /// Classifies a stored value.
    #[must_use]
    pub fn from_code(code: u32) -> Self {
        match code {
            2 => Self::Decal,
            3 => Self::GeneratorPoint,
            6 => Self::Weenie,
            7 => Self::Physics,
            9 => Self::Light,
            16 => Self::TeleportDestination,
            19 => Self::SegmentNode,
            23 => Self::PatrolPoint,
            31 => Self::LbiOnly,
            other => Self::Other(other),
        }
    }

    /// The stored value.
    #[must_use]
    pub fn code(self) -> u32 {
        match self {
            Self::Decal => 2,
            Self::GeneratorPoint => 3,
            Self::Weenie => 6,
            Self::Physics => 7,
            Self::Light => 9,
            Self::TeleportDestination => 16,
            Self::SegmentNode => 19,
            Self::PatrolPoint => 23,
            Self::LbiOnly => 31,
            Self::Other(c) => c,
        }
    }
}

/// A structured 64-bit instance ID (§4.2).
///
/// | Bits | Content |
/// |---|---|
/// | 56–63 | kind byte (1 or 4 observed; meaning OPEN) |
/// | 52–55 | region of the owning landblock |
/// | 44–51 | landblock X |
/// | 36–43 | landblock Y |
/// | 32–35 | 0 |
/// | 0–31 | serial within the landblock |
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, PartialOrd, Ord, Default)]
pub struct InstanceId(pub u64);

impl InstanceId {
    /// Builds an ID from its parts (bits 32–35 zero).
    #[must_use]
    pub fn new(kind: u8, region: u8, x: u8, y: u8, serial: u32) -> Self {
        Self(
            u64::from(kind) << 56
                | u64::from(region & 0xF) << 52
                | u64::from(x) << 44
                | u64::from(y) << 36
                | u64::from(serial),
        )
    }

    /// Kind byte (bits 56–63).
    #[must_use]
    pub fn kind(self) -> u8 {
        (self.0 >> 56) as u8
    }

    /// Region of the owning landblock (bits 52–55).
    #[must_use]
    pub fn region(self) -> u8 {
        ((self.0 >> 52) & 0xF) as u8
    }

    /// Owning landblock (X, Y) (bits 44–51, 36–43).
    #[must_use]
    pub fn landblock(self) -> (u8, u8) {
        ((self.0 >> 44) as u8, (self.0 >> 36) as u8)
    }

    /// Bits 32–35 (0 in every observed SED).
    #[must_use]
    pub fn reserved_bits(self) -> u8 {
        ((self.0 >> 32) & 0xF) as u8
    }

    /// Serial within the landblock (bits 0–31).
    #[must_use]
    pub fn serial(self) -> u32 {
        self.0 as u32
    }
}

/// Position and rotation of a SED in its owning landblock's frame (§4.4, §5).
///
/// x, y are block-local from the south-west corner (interior entities may lie outside
/// 0–160); z is the absolute height. A decoded placement has finite position components
/// of magnitude at most [`MAX_PLACEMENT_COORD`] and a unit-length `rotation`.
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct Placement {
    /// Block-local position.
    pub position: [f32; 3],
    /// Unit rotation, (w, x, y, z) on disk, active from object space to the landblock frame.
    /// Always the stored rotation normalised, however close to unit length it was stored.
    pub rotation: Quaternion,
    /// The rotation exactly as stored (written back by [`StaticEntity::to_bytes_with`], so
    /// a decoded SED re-encodes byte for byte).
    pub stored_rotation: Quaternion,
    /// Whether the stored rotation was more than 0.01 from unit length, the spec's
    /// renormalise-and-report threshold (§10; never observed). [`Placement::is_off_unit`]
    /// applies the tighter ED5 tolerance.
    pub renormalised: bool,
}

impl Placement {
    /// Builds a placement as the decoder does: `position` must be finite with every
    /// component within ±[`MAX_PLACEMENT_COORD`], and `rotation` finite and non-zero; it is
    /// normalised and kept as the stored value. Errors are [`FormatError::Malformed`].
    pub fn new(position: [f32; 3], rotation: Quaternion) -> Result<Self> {
        placement(position, rotation, 0)
    }

    /// Length of the stored rotation (1 within 0.001 in every observed SED).
    #[must_use]
    pub fn stored_length(&self) -> f32 {
        self.stored_rotation.length()
    }

    /// Whether the stored rotation is further than 0.001 from unit length, breaking ED5
    /// (`entity-descriptor.md` §11).
    #[must_use]
    pub fn is_off_unit(&self) -> bool {
        (self.stored_length() - 1.0).abs() > ED5_TOLERANCE
    }

    /// Places object-space point `v` (§5): multiply by `scale` per axis, rotate, add the
    /// position. The result is block-local; add [`super::region_position`] for region
    /// coordinates.
    ///
    /// For a decoded placement, a scale from [`StaticEntity::scale`] (components at most
    /// [`MAX_SCALE`]) and a finite `v` with components of magnitude below 1e30, the result
    /// is finite.
    #[must_use]
    pub fn transform(&self, v: [f32; 3], scale: [f32; 3]) -> [f32; 3] {
        let s = [v[0] * scale[0], v[1] * scale[1], v[2] * scale[2]];
        let r = self.rotation.rotate(s);
        [
            r[0] + self.position[0],
            r[1] + self.position[1],
            r[2] + self.position[2],
        ]
    }

    /// Transforms an object-space normal of a SED placed with per-axis `scale`: divides
    /// by the scale per axis (the inverse transpose of the scale), rotates, and normalises.
    /// For a uniform scale this is just the rotation. A scale with a zero or non-finite
    /// component is treated as (1, 1, 1). A zero `n` stays zero.
    #[must_use]
    pub fn transform_normal(&self, n: [f32; 3], scale: [f32; 3]) -> [f32; 3] {
        let scaled = [n[0] / scale[0], n[1] / scale[1], n[2] / scale[2]];
        let m = if scale.iter().all(|s| s.is_finite() && *s != 0.0)
            && scaled.iter().all(|c| c.is_finite())
        {
            scaled
        } else {
            n
        };
        let r = self.rotation.rotate(m);
        let len = r.iter().map(|c| f64::from(*c).powi(2)).sum::<f64>().sqrt();
        if len.is_normal() {
            #[allow(clippy::cast_possible_truncation)]
            let unit = r.map(|c| (f64::from(c) / len) as f32);
            unit
        } else {
            r
        }
    }

    /// Whether x and y lie inside the 160-unit landblock square.
    #[must_use]
    pub fn is_inside_block(&self) -> bool {
        let inside = |c: f32| (0.0..=LANDBLOCK_SIZE).contains(&c);
        inside(self.position[0]) && inside(self.position[1])
    }
}

/// One static entity descriptor (§3.1). Optional fields are present exactly when their
/// flag bit is set.
#[derive(Debug, Clone, PartialEq, Default)]
pub struct StaticEntity {
    /// Flag word; never has a bit outside [`SED_FLAGS_KNOWN`].
    pub flags: u32,
    /// 0x001: a DID (Reported only; meaning unknown).
    pub did: Option<Did>,
    /// 0x002: the entity type ([`StaticEntity::kind`]).
    pub entity_type: Option<u32>,
    /// 0x004: instance ID.
    pub iid: Option<InstanceId>,
    /// 0x008: what is placed: a 0x47 physics object, or a 0x70 game object for weenies.
    pub object: Option<Did>,
    /// 0x010: position and rotation.
    pub placement: Option<Placement>,
    /// 0x020: three floats (Reported only; meaning unknown).
    pub vector: Option<[f32; 3]>,
    /// 0x080: embedded property set (scale, draw overrides, content layers, ...).
    pub properties: Option<PropertySet>,
    /// 0x040: a word (stored after the property set; meaning OPEN, §4.6).
    pub word: Option<u32>,
    /// 0x100: world-building label (Latin-1); clients must not depend on it.
    pub name: Option<String>,
    /// 0x200: a DID list (Reported only).
    pub dids: Option<Vec<Did>>,
}

impl StaticEntity {
    /// Human-readable structure name, used in errors.
    pub const NAME: &'static str = "static entity descriptor";

    /// The entity type, if stored.
    #[must_use]
    pub fn kind(&self) -> Option<StaticEntityType> {
        self.entity_type.map(StaticEntityType::from_code)
    }

    /// Whether the interior flag (0x400) is set: the entity belongs to an interior cell
    /// (§4.8).
    #[must_use]
    pub fn is_interior(&self) -> bool {
        self.flags & F_INTERIOR != 0
    }

    /// Top-level property `pid` of the SED's property set.
    #[must_use]
    pub fn property(&self, pid: u32) -> Option<&PropertyValue> {
        self.properties.as_ref()?.get(pid)
    }

    /// The per-axis scale from the vector property `pid` (normally `Render_AdjustableScale`,
    /// §7), or (1, 1, 1) when absent, not a vector, or with any component outside
    /// [[`MIN_SCALE`], [`MAX_SCALE`]] (zero, negative, non-finite or absurd; observed values
    /// are 0.01 – 100). The result is always finite and positive.
    #[must_use]
    pub fn scale(&self, pid: u32) -> [f32; 3] {
        match self.property(pid) {
            Some(PropertyValue::Vector3(v))
                if v.iter().all(|c| (MIN_SCALE..=MAX_SCALE).contains(c)) =>
            {
                *v
            }
            _ => [1.0; 3],
        }
    }

    /// [`StaticEntity::scale`] as one factor when its three components are equal (every
    /// observed SED, ED6), else `None`.
    #[must_use]
    pub fn uniform_scale(&self, pid: u32) -> Option<f32> {
        let [x, y, z] = self.scale(pid);
        (x == y && y == z).then_some(x)
    }

    /// Reads one SED at the reader's position (§10).
    pub fn read<S: PropertySchema + ?Sized>(r: &mut Reader<'_>, schema: &S) -> Result<Self> {
        let flags_at = r.position();
        let flags = r.u32()?;
        if flags & !SED_FLAGS_KNOWN != 0 {
            return Err(FormatError::Unsupported {
                what: "static entity flags",
                offset: flags_at,
                value: flags,
            });
        }
        let has = |bit: u32| flags & bit != 0;
        let mut e = Self {
            flags,
            ..Self::default()
        };
        if has(F_DID) {
            e.did = Some(r.did()?);
        }
        if has(F_TYPE) {
            e.entity_type = Some(r.u32()?);
        }
        if has(F_IID) {
            e.iid = Some(InstanceId(r.u64()?));
        }
        if has(F_OBJECT) {
            e.object = Some(r.did()?);
        }
        if has(F_PLACEMENT) {
            let at = r.position();
            let position = [r.f32()?, r.f32()?, r.f32()?];
            let stored = Quaternion {
                w: r.f32()?,
                x: r.f32()?,
                y: r.f32()?,
                z: r.f32()?,
            };
            e.placement = Some(placement(position, stored, at)?);
        }
        if has(F_VECTOR) {
            e.vector = Some([r.f32()?, r.f32()?, r.f32()?]);
        }
        if has(F_PROPERTIES) {
            e.properties = Some(read_embedded_set(r, schema)?);
        }
        if has(F_WORD) {
            e.word = Some(r.u32()?);
        }
        if has(F_NAME) {
            e.name = Some(r.pascal_string()?);
        }
        if has(F_DIDS) {
            let count = r.vle()?;
            let mut dids = r.alloc_vec(count, 4, usize::MAX)?;
            for _ in 0..count {
                let d = r.did()?;
                r.push(&mut dids, d, count)?;
            }
            e.dids = Some(dids);
        }
        Ok(e)
    }

    /// The first deviation from the observed invariants (ED1, ED3, ED5), or `None`.
    #[must_use]
    pub fn first_unusual_field(&self) -> Option<&'static str> {
        if self.flags & F_ALWAYS != F_ALWAYS {
            return Some("SED lacks type, IID or property set (ED1)");
        }
        if self.flags & F_REPORTED_ONLY != 0 {
            return Some("SED uses a reported-only flag bit (0x001, 0x020, 0x200)");
        }
        if self.iid.is_some_and(|i| i.reserved_bits() != 0) {
            return Some("IID bits 32–35 not zero (ED3)");
        }
        if self.placement.is_some_and(|p| p.is_off_unit()) {
            return Some("rotation not unit length (ED5)");
        }
        None
    }

    /// Encodes the SED with the stored bytes of its property set (this crate has no
    /// property-set encoder). The payload flag bits (0x001–0x200) are derived from the
    /// fields, so a field is written exactly when it is present; bit 0x400 (no payload) is
    /// kept from `flags`. The placement is written with its
    /// [`Placement::stored_rotation`], so a decoded SED re-encodes byte for byte.
    ///
    /// Fails ([`FormatError::Malformed`]) when `flags` has a bit outside
    /// [`SED_FLAGS_KNOWN`], when `property_set_bytes` is empty although `properties` is
    /// present (or not empty although it is absent), or when `name` has a character
    /// outside Latin-1.
    pub fn to_bytes_with(&self, property_set_bytes: &[u8]) -> Result<Vec<u8>> {
        let bad = |reason: &'static str| FormatError::malformed(Self::NAME, 0, reason);
        if self.flags & !SED_FLAGS_KNOWN != 0 {
            return Err(bad("flag bit outside 0x7FF"));
        }
        if self.properties.is_some() == property_set_bytes.is_empty() {
            return Err(bad(
                "property-set bytes must be given exactly when `properties` is present",
            ));
        }
        let name = match &self.name {
            Some(n) => Some(
                n.chars()
                    .map(|c| u8::try_from(u32::from(c)).ok())
                    .collect::<Option<Vec<u8>>>()
                    .ok_or_else(|| bad("name has a character outside Latin-1"))?,
            ),
            None => None,
        };
        let bit = |present: bool, b: u32| if present { b } else { 0 };
        let flags = self.flags & !F_PAYLOAD
            | bit(self.did.is_some(), F_DID)
            | bit(self.entity_type.is_some(), F_TYPE)
            | bit(self.iid.is_some(), F_IID)
            | bit(self.object.is_some(), F_OBJECT)
            | bit(self.placement.is_some(), F_PLACEMENT)
            | bit(self.vector.is_some(), F_VECTOR)
            | bit(self.word.is_some(), F_WORD)
            | bit(self.properties.is_some(), F_PROPERTIES)
            | bit(name.is_some(), F_NAME)
            | bit(self.dids.is_some(), F_DIDS);
        let mut out = Vec::new();
        put::u32(&mut out, flags);
        if let Some(d) = self.did {
            put::u32(&mut out, d.0);
        }
        if let Some(t) = self.entity_type {
            put::u32(&mut out, t);
        }
        if let Some(i) = self.iid {
            out.extend_from_slice(&i.0.to_le_bytes());
        }
        if let Some(o) = self.object {
            put::u32(&mut out, o.0);
        }
        if let Some(p) = self.placement {
            put::f32s(&mut out, &p.position);
            put::f32s(&mut out, &p.stored_rotation.to_wxyz());
        }
        if let Some(v) = self.vector {
            put::f32s(&mut out, &v);
        }
        if self.properties.is_some() {
            out.extend_from_slice(property_set_bytes);
        }
        if let Some(w) = self.word {
            put::u32(&mut out, w);
        }
        if let Some(bytes) = name {
            let len = u32::try_from(bytes.len()).map_err(|_| bad("name too long"))?;
            crate::reader::write_vle(&mut out, len);
            out.extend_from_slice(&bytes);
        }
        if let Some(dids) = &self.dids {
            let len = u32::try_from(dids.len()).map_err(|_| bad("DID list too long"))?;
            crate::reader::write_vle(&mut out, len);
            for d in dids {
                put::u32(&mut out, d.0);
            }
        }
        Ok(out)
    }
}

/// Validates a stored transform: a non-finite component, a position component beyond
/// ±[`MAX_PLACEMENT_COORD`] and a zero-length rotation are malformed. The rotation is always
/// normalised; one off unit length by more than 0.01 is reported through
/// [`Placement::renormalised`] (the record decoder logs the count once per record).
fn placement(position: [f32; 3], stored: Quaternion, offset: usize) -> Result<Placement> {
    let bad =
        |reason: &'static str| FormatError::malformed("static entity placement", offset, reason);
    if !position.iter().all(|c| c.is_finite()) || !stored.is_finite() {
        return Err(bad("non-finite position or rotation"));
    }
    if position.iter().any(|c| c.abs() > MAX_PLACEMENT_COORD) {
        return Err(bad("position component beyond the decoder's bound"));
    }
    let Some(rotation) = stored.normalized() else {
        return Err(bad("zero-length rotation"));
    };
    let renormalised = (stored.length() - 1.0).abs() > RENORMALISE_TOLERANCE;
    Ok(Placement {
        position,
        rotation,
        stored_rotation: stored,
        renormalised,
    })
}
