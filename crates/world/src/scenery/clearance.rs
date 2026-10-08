// SPDX-License-Identifier: GPL-3.0-or-later
//! The static list of `scenery.md` §7.4 step 14a: the landblock-info static entities of
//! type Weenie (6), Physics (7) or PatrolPoint (23) with the clearance volumes of their
//! `Entity_Clearance_BoxList` and `Entity_Clearance_CylinderList` properties, and the
//! overlap tests against them.
//!
//! How an entry is built (§7.4 step 14a, S45):
//! - **Frame.** The entity's position and rotation; an entity without a frame stands at the
//!   landblock origin with the identity rotation (`entity-descriptor.md` §4.4) and is kept.
//! - **Which value.** Each list is looked up separately with the client's lookup on a
//!   static entity (`entity-descriptor.md` §4.9): the entity's own set; then for a 0x47
//!   object the object and its parents; for a game object (0x70) its property set
//!   (DID + 0x09000000), then the 0x47 object named by its `PhysObj` and that object's
//!   parents. The first level that has the property supplies the whole list.
//! - **Reading a list.** Elements are read in order; the first element that is not a struct
//!   with the required fields of the required types ends the list. Boxes are stored with
//!   their corners sorted per axis.
//! - **Filter** (Tessera Decision of step 14a): every entity of the three types is kept
//!   unless it fails the validity check of `entity-descriptor.md` §4.9; Generator targets
//!   are not dropped (the client's offline filter mode is OPEN, §15 item 6).
//!
//! Hostile input (Tessera Decisions of `scenery.md` §13.1, "Static-entity frames" and
//! "Size of the static list"): see [`static_clearances`] and [`volumes_from_values`].

use std::sync::Arc;

use tessera_formats::landblock::StaticEntityType;
use tessera_formats::setup::{ENTITY_PREFIX, Quaternion};
use tessera_formats::{
    Did, LandblockInfo, PropertyDictionary, PropertySchema, PropertySet, PropertyStruct,
    PropertyValue, Record, object_property_set_did,
};

use super::error::SceneryError;
use super::math::{self, EPS};
use super::props::{ObjectProperties, ObjectPropertyCache, read_set};
use crate::context::{PROPERTY_DICTIONARY_DID, WorldContext};
use crate::source::WorldSource;

/// Most volumes of each kind kept per entity (Tessera Decision of `scenery.md` §13.1; the
/// install has at most 4 boxes and 7 cylinders in one list).
pub const MAX_VOLUMES_PER_ENTITY: usize = 256;
/// Most volumes (boxes and cylinders together) kept in one landblock's static list
/// (Tessera Decision of `scenery.md` §13.1: it caps the list's memory and the cost of each
/// step-14a test; the install has at most 5,561 per landblock). Entities after the cap is
/// reached are left out, with a warning.
pub const MAX_STATIC_VOLUMES: usize = 16_384;
/// How far from unit length (|‖q‖ − 1|) a static entity's rotation may be and still be
/// used as stored; a longer or shorter finite one is normalised first (see
/// [`static_clearances`]).
pub const ROTATION_UNIT_TOLERANCE: f32 = 1e-3;

/// Reads and decodes the master property dictionary (`property-system.md` §2) of a world
/// context's source, for the name → PID lookups of [`ClearancePids::from_names`] and
/// [`crate::frills::FrillPids::from_names`]. [`WorldContext`] keeps only the schema, so
/// the dictionary is read again here; read it once and pass it to both.
///
/// # Errors
///
/// When the dictionary is missing, unreadable or does not decode.
pub fn property_dictionary(ctx: &WorldContext) -> Result<PropertyDictionary, SceneryError> {
    let did = Did(PROPERTY_DICTIONARY_DID);
    let bytes = ctx.source().read(did)?.ok_or(SceneryError::MissingRecord {
        did: did.0,
        what: "property dictionary",
    })?;
    PropertyDictionary::parse_expecting(&bytes, did)
        .map_err(|e| SceneryError::format(did.0, "property dictionary", e))
}

/// The property IDs the static list reads, looked up by name at run time.
#[derive(Debug, Clone, Copy, Default, PartialEq, Eq)]
pub struct ClearancePids {
    /// `Entity_Clearance_BoxList` (0x207 in the observed dictionary).
    pub box_list: Option<u32>,
    /// `Entity_Clearance_CylinderList` (0x4CF).
    pub cylinder_list: Option<u32>,
    /// `Entity_Clearance_Box_Min`.
    pub box_min: Option<u32>,
    /// `Entity_Clearance_Box_Max`.
    pub box_max: Option<u32>,
    /// `Entity_Clearance_LowPoint`.
    pub low_point: Option<u32>,
    /// `Entity_Clearance_Radius`.
    pub radius: Option<u32>,
    /// `Entity_Clearance_Height`.
    pub height: Option<u32>,
    /// `PhysObj`: the 0x47 object of a game object (`entity-descriptor.md` §4.9 step 3).
    pub phys_obj: Option<u32>,
    /// `Entity_CreationType`: the validity check of `entity-descriptor.md` §4.9.
    pub creation_type: Option<u32>,
}

impl ClearancePids {
    /// Looks every PID up with `pid_of(name)`.
    #[must_use]
    pub fn from_names(pid_of: impl Fn(&str) -> Option<u32>) -> Self {
        Self {
            box_list: pid_of("Entity_Clearance_BoxList"),
            cylinder_list: pid_of("Entity_Clearance_CylinderList"),
            box_min: pid_of("Entity_Clearance_Box_Min"),
            box_max: pid_of("Entity_Clearance_Box_Max"),
            low_point: pid_of("Entity_Clearance_LowPoint"),
            radius: pid_of("Entity_Clearance_Radius"),
            height: pid_of("Entity_Clearance_Height"),
            phys_obj: pid_of("PhysObj"),
            creation_type: pid_of("Entity_CreationType"),
        }
    }

    /// The PIDs of a world context's property dictionary ([`property_dictionary`]).
    ///
    /// # Errors
    ///
    /// As [`property_dictionary`].
    pub fn from_context(ctx: &WorldContext) -> Result<Self, SceneryError> {
        let dict = property_dictionary(ctx)?;
        Ok(Self::from_names(|n| dict.pid(n)))
    }
}

/// A clearance box in its entity's frame. [`ClearanceBox::from_corners`] stores the
/// corners sorted per axis, as the client does (§7.4 step 14a); the overlap test sorts
/// them again, so a box built with crossed corners behaves the same.
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct ClearanceBox {
    /// Minimum corner.
    pub min: [f32; 3],
    /// Maximum corner.
    pub max: [f32; 3],
}

impl ClearanceBox {
    /// The box spanned by two stored corners: per axis, min is the smaller and max the
    /// larger of the two values (`Box_Min` may exceed `Box_Max` on an axis).
    #[must_use]
    pub fn from_corners(a: [f32; 3], b: [f32; 3]) -> Self {
        Self {
            min: std::array::from_fn(|k| a[k].min(b[k])),
            max: std::array::from_fn(|k| a[k].max(b[k])),
        }
    }

    /// The box with its corners sorted per axis.
    #[must_use]
    pub fn sorted(self) -> Self {
        Self::from_corners(self.min, self.max)
    }
}

/// A clearance cylinder in its entity's frame.
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct ClearanceCylinder {
    /// Centre of the bottom face.
    pub low_point: [f32; 3],
    /// Radius.
    pub radius: f32,
    /// Height above the low point (used only by the `spacing_3d` test).
    pub height: f32,
}

/// One entry of the static list: an entity's frame and its clearance volumes.
#[derive(Debug, Clone, PartialEq)]
pub struct StaticClearance {
    /// Position in the landblock frame.
    pub position: [f32; 3],
    /// Unit rotation from the entity frame to the landblock frame.
    pub rotation: Quaternion,
    /// Boxes.
    pub boxes: Vec<ClearanceBox>,
    /// Cylinders.
    pub cylinders: Vec<ClearanceCylinder>,
}

impl StaticClearance {
    /// Whether the entity carries no volume (it can never overlap).
    #[must_use]
    pub fn is_empty(&self) -> bool {
        self.boxes.is_empty() && self.cylinders.is_empty()
    }

    /// Number of volumes (boxes and cylinders).
    #[must_use]
    pub fn volume_count(&self) -> usize {
        self.boxes.len().saturating_add(self.cylinders.len())
    }

    /// The spacing overlap test of §7.4 step 14a for candidate `p` (landblock frame) with
    /// spacing radius `rho`. `p` is mapped into the entity frame (subtract the position,
    /// rotate by the inverse rotation; no scale), then:
    /// - a box (corners sorted per axis) is skipped when all three extents are at most
    ///   0.0002; otherwise it overlaps when the squared distance from the mapped point to
    ///   it (z only with `spacing_3d`) is at most ρ²;
    /// - a cylinder is skipped unless its radius r is above 0. Without `spacing_3d` it
    ///   overlaps when (x − cx)² + (y − cy)² ≤ (ρ + r)². With `spacing_3d`, z′ is the
    ///   mapped z clamped to cz … cz + h, and it overlaps when
    ///   (x − cx)² + (y − cy)² + (z − z′)² < (ρ + r)² (strictly less).
    ///
    /// Hostile input (Tessera Decision of §13.1): a mapped point that is not finite
    /// overlaps nothing; [`static_clearances`] never builds such an entry. A cylinder with a
    /// negative height clamps z to cz first (see `scenery.md` §7.4 step 14a OPEN).
    #[must_use]
    pub fn overlaps(&self, p: [f32; 3], rho: f32, spacing_3d: bool) -> bool {
        let d = [
            p[0] - self.position[0],
            p[1] - self.position[1],
            p[2] - self.position[2],
        ];
        let inv = Quaternion {
            w: self.rotation.w,
            x: -self.rotation.x,
            y: -self.rotation.y,
            z: -self.rotation.z,
        };
        let m = inv.rotate(d);
        if !m.iter().all(|c| c.is_finite()) {
            return false;
        }
        let rho2 = rho * rho;
        let axes = if spacing_3d { 3 } else { 2 };
        for b in &self.boxes {
            let b = b.sorted();
            let ext = [
                b.max[0] - b.min[0],
                b.max[1] - b.min[1],
                b.max[2] - b.min[2],
            ];
            if ext.iter().all(|e| *e <= EPS) {
                continue;
            }
            let mut d2 = 0.0f32;
            for ((v, lo), hi) in m.iter().zip(b.min).zip(b.max).take(axes) {
                let gap = if *v < lo {
                    lo - v
                } else if *v > hi {
                    v - hi
                } else {
                    0.0
                };
                d2 += gap * gap;
            }
            if d2 <= rho2 {
                return true;
            }
        }
        for c in &self.cylinders {
            if c.radius.is_nan() || c.radius <= 0.0 {
                continue;
            }
            let (dx, dy) = (m[0] - c.low_point[0], m[1] - c.low_point[1]);
            let r = rho + c.radius;
            if spacing_3d {
                let (bottom, top) = (c.low_point[2], c.low_point[2] + c.height);
                let z_clamped = if m[2] < bottom {
                    bottom
                } else if m[2] > top {
                    top
                } else {
                    m[2]
                };
                let dz = m[2] - z_clamped;
                if dx * dx + dy * dy + dz * dz < r * r {
                    return true;
                }
            } else if dx * dx + dy * dy <= r * r {
                return true;
            }
        }
        false
    }
}

/// A vector field of a list element: `None` when the field is absent or not a vector.
fn vector_field(s: &PropertyStruct, pid: Option<u32>) -> Option<[f32; 3]> {
    match s.get(pid?)? {
        PropertyValue::Vector3(v) => Some(*v),
        _ => None,
    }
}

/// A float field of a list element: `None` when the field is absent or not a float.
fn float_field(s: &PropertyStruct, pid: Option<u32>) -> Option<f32> {
    s.get(pid?)?.as_f32()
}

/// Reads the clearance volumes of a box-list and a cylinder-list value (§7.4 step 14a).
///
/// A value that is not an array gives no volumes. Elements are read in order. A box
/// element must be a struct holding `Box_Min` and `Box_Max` as vectors; a cylinder element
/// a struct holding `LowPoint` as a vector and `Radius` and `Height` as floats. The first
/// element that does not meet this **ends** its list: it and every later element are
/// ignored. Boxes are stored with their corners sorted per axis
/// ([`ClearanceBox::from_corners`]).
///
/// Tessera Decisions of §13.1: an element that meets the rule but holds a value that is not
/// finite is skipped (the list goes on); at most [`MAX_VOLUMES_PER_ENTITY`] volumes of
/// each kind are kept, and reading stops once that many are kept.
#[must_use]
pub fn volumes_from_values(
    box_list: Option<&PropertyValue>,
    cylinder_list: Option<&PropertyValue>,
    pids: &ClearancePids,
) -> (Vec<ClearanceBox>, Vec<ClearanceCylinder>) {
    let finite = |v: &[f32]| v.iter().all(|c| c.is_finite());
    let mut boxes = Vec::new();
    if let Some(a) = box_list.and_then(PropertyValue::as_array) {
        for e in &a.elements {
            if boxes.len() >= MAX_VOLUMES_PER_ENTITY {
                break;
            }
            let Some(s) = e.value.as_struct() else {
                break;
            };
            let (Some(min), Some(max)) =
                (vector_field(s, pids.box_min), vector_field(s, pids.box_max))
            else {
                break;
            };
            if finite(&min) && finite(&max) {
                boxes.push(ClearanceBox::from_corners(min, max));
            }
        }
    }
    let mut cylinders = Vec::new();
    if let Some(a) = cylinder_list.and_then(PropertyValue::as_array) {
        for e in &a.elements {
            if cylinders.len() >= MAX_VOLUMES_PER_ENTITY {
                break;
            }
            let Some(s) = e.value.as_struct() else {
                break;
            };
            let (Some(low_point), Some(radius), Some(height)) = (
                vector_field(s, pids.low_point),
                float_field(s, pids.radius),
                float_field(s, pids.height),
            ) else {
                break;
            };
            if finite(&low_point) && radius.is_finite() && height.is_finite() {
                cylinders.push(ClearanceCylinder {
                    low_point,
                    radius,
                    height,
                });
            }
        }
    }
    (boxes, cylinders)
}

/// Builds the static list of a landblock-info record (§7.4 step 14a; module docs): the
/// entities of type Weenie, Physics or PatrolPoint in record order, each with the volumes
/// of the lists found by the lookup of `entity-descriptor.md` §4.9, an entity without a
/// frame at the landblock origin with the identity rotation. Entities without any volume
/// are left out (they never overlap); so are entities that fail the validity check of
/// `entity-descriptor.md` §4.9 (see the module docs).
///
/// Hostile input (Tessera Decisions of `scenery.md` §13.1): an entity whose position or
/// stored rotation is not finite, or whose rotation has zero length, is left out; a finite
/// rotation further than [`ROTATION_UNIT_TOLERANCE`] from unit length is normalised (the
/// inverse rotation of the test is the conjugate, which is only a rotation for a unit
/// quaternion). At most [`MAX_VOLUMES_PER_ENTITY`] volumes of each kind per entity and
/// [`MAX_STATIC_VOLUMES`] in all are kept; property values are borrowed, never copied.
///
/// # Errors
///
/// When a property record could not be read for a reason that is not the data's
/// ([`SceneryError::is_data_error`] false): the list would otherwise silently miss
/// volumes, so the caller should build it again later. Missing or broken records are not
/// errors (their levels supply nothing).
pub fn static_clearances(
    info: &LandblockInfo,
    source: &dyn WorldSource,
    schema: &dyn PropertySchema,
    pids: &ClearancePids,
    objects: &ObjectPropertyCache,
) -> Result<Vec<StaticClearance>, SceneryError> {
    let mut out = Vec::new();
    let mut total = 0usize;
    for e in &info.entities {
        let kind = e.kind();
        if !matches!(
            kind,
            Some(
                StaticEntityType::Weenie
                    | StaticEntityType::Physics
                    | StaticEntityType::PatrolPoint
            )
        ) {
            continue;
        }
        // An entity without a frame stands at the origin with the identity rotation
        // (`entity-descriptor.md` §3.4, §4.4).
        let (position, stored) = e.placement.map_or(([0.0; 3], Quaternion::IDENTITY), |p| {
            (p.position, p.stored_rotation)
        });
        if !position.iter().all(|c| c.is_finite()) {
            continue;
        }
        let Some(rotation) = usable_rotation(stored) else {
            continue;
        };
        if total >= MAX_STATIC_VOLUMES {
            tracing::warn!(
                landblock = %format_args!("{:#010x}", info.did.0),
                cap = MAX_STATIC_VOLUMES,
                "static list volume cap reached; later entities left out"
            );
            break;
        }
        let mut lookup = EntityLookup::new(e.properties.as_ref(), e.object);
        let own_has = |pid: Option<u32>| lookup.own_get(pid).is_some();
        if !(own_has(pids.box_list) && own_has(pids.cylinder_list)) {
            lookup.load(source, schema, pids, objects)?;
        }
        let (mut boxes, mut cylinders) = volumes_from_values(
            lookup.get(pids.box_list),
            lookup.get(pids.cylinder_list),
            pids,
        );
        if boxes.is_empty() && cylinders.is_empty() {
            continue;
        }
        if pids.creation_type.is_some() {
            lookup.load(source, schema, pids, objects)?;
        }
        if !is_valid_entity(kind, e.object, &lookup, pids) {
            tracing::debug!(
                landblock = %format_args!("{:#010x}", info.did.0),
                "static entity fails the validity check; left out of the static list"
            );
            continue;
        }
        let room = MAX_STATIC_VOLUMES - total;
        boxes.truncate(room);
        cylinders.truncate(room - boxes.len());
        let entry = StaticClearance {
            position,
            rotation,
            boxes,
            cylinders,
        };
        total += entry.volume_count();
        out.push(entry);
    }
    Ok(out)
}

/// A static entity's rotation as the step-14a test uses it (see [`static_clearances`]).
pub(super) fn usable_rotation(q: Quaternion) -> Option<Quaternion> {
    if !q.to_wxyz().iter().all(|c| c.is_finite()) {
        return None;
    }
    if math::is_unit(q, ROTATION_UNIT_TOLERANCE) {
        return Some(q);
    }
    q.normalized()
}

/// The class of an `Entity_CreationType` value (`entity-descriptor.md` §4.9 table): 1
/// Dynamic, 2 Static, 3 ClientOnly; and whether the value is "unique".
fn creation_class(value: u32) -> (u8, bool) {
    match value {
        1 | 4 | 6 => (3, true),
        2 | 5 => (3, false),
        7 | 8 => (1, true),
        9 => (2, true),
        // 0, 3 and any other value.
        _ => (2, false),
    }
}

/// The validity check of `entity-descriptor.md` §4.9 as the static list applies it
/// (Tessera Decision of `scenery.md` §7.4 step 14a: keep every entity that is not
/// invalid). An entity is invalid when its object DID is invalid, when its creation type
/// (looked up as in §4.9, a value that is not an integer or enum counting as 0, then
/// step 2 for a game object) is 0, or when it is a Weenie whose final class is Static.
///
/// Engineering choices recorded as an OPEN note in `scenery.md` §7.4 step 14a: "invalid
/// object DID" is taken to be an absent object field (`entity-descriptor.md` §3.4) or the
/// null DID 0; when the dictionary does not define `Entity_CreationType`
/// (`pids.creation_type` is `None`) the creation-type part is not applied.
fn is_valid_entity(
    kind: Option<StaticEntityType>,
    object: Option<Did>,
    lookup: &EntityLookup<'_>,
    pids: &ClearancePids,
) -> bool {
    let Some(object) = object.filter(|d| d.0 != 0) else {
        return false;
    };
    if pids.creation_type.is_none() {
        return true;
    }
    let mut value = match lookup.get(pids.creation_type) {
        Some(PropertyValue::Enum(v)) => *v,
        // A negative or out-of-range integer is "any other value" (class Static, not
        // unique); u32::MAX stands for it.
        Some(PropertyValue::Int(v)) => u32::try_from(*v).unwrap_or(u32::MAX),
        Some(PropertyValue::Int64(v)) => u32::try_from(*v).unwrap_or(u32::MAX),
        _ => 0,
    };
    if object_property_set_did(object).is_some() {
        let (class, unique) = creation_class(value);
        if !unique {
            value = if class == 3 { 6 } else { 8 };
        }
        if creation_class(value).0 == 2 {
            value = 8;
        }
    }
    if value == 0 {
        return false;
    }
    !(kind == Some(StaticEntityType::Weenie) && creation_class(value).0 == 2)
}

/// The levels of the lookup of `entity-descriptor.md` §4.9 for one static entity: its own
/// set; then either the 0x47 object's chain, or the game object's property set followed
/// by the chain of the 0x47 object its `PhysObj` names. The levels behind the own set are
/// read on first need ([`EntityLookup::load`]).
struct EntityLookup<'a> {
    own: Option<&'a PropertySet>,
    object: Option<Did>,
    loaded: bool,
    game_object: Option<PropertySet>,
    chain: Option<Arc<ObjectProperties>>,
}

impl<'a> EntityLookup<'a> {
    fn new(own: Option<&'a PropertySet>, object: Option<Did>) -> Self {
        Self {
            own,
            object,
            loaded: false,
            game_object: None,
            chain: None,
        }
    }

    fn own_get(&self, pid: Option<u32>) -> Option<&PropertyValue> {
        self.own.and_then(|s| s.get(pid?))
    }

    /// Reads the levels behind the own set (once).
    fn load(
        &mut self,
        source: &dyn WorldSource,
        schema: &dyn PropertySchema,
        pids: &ClearancePids,
        objects: &ObjectPropertyCache,
    ) -> Result<(), SceneryError> {
        if self.loaded {
            return Ok(());
        }
        self.loaded = true;
        let Some(object) = self.object else {
            return Ok(());
        };
        if object.prefix() == ENTITY_PREFIX {
            self.chain = objects.get(source, schema, object)?;
            return Ok(());
        }
        let Some(set_did) = object_property_set_did(object) else {
            return Ok(());
        };
        match read_set(source, schema, set_did) {
            Ok(set) => self.game_object = Some(set),
            Err(e) if e.is_data_error() => {
                tracing::debug!(set = %format_args!("{:#010x}", set_did.0), error = %e, "game object properties unusable");
            }
            Err(e) => return Err(e),
        }
        // The 0x47 object named by the game object's `PhysObj`, and its parents.
        let phys = self
            .game_object
            .as_ref()
            .and_then(|s| s.get(pids.phys_obj?))
            .and_then(PropertyValue::as_did)
            .filter(|d| d.prefix() == ENTITY_PREFIX);
        if let Some(phys) = phys {
            self.chain = objects.get(source, schema, phys)?;
        }
        Ok(())
    }

    /// The value of `pid` from the nearest level that has it.
    fn get(&self, pid: Option<u32>) -> Option<&PropertyValue> {
        let pid = pid?;
        self.own
            .and_then(|s| s.get(pid))
            .or_else(|| self.game_object.as_ref().and_then(|s| s.get(pid)))
            .or_else(|| self.chain.as_ref().and_then(|c| c.get(pid)))
    }
}
