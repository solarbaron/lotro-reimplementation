// SPDX-License-Identifier: GPL-3.0-or-later
//! The static list of `scenery.md` §7.4 step 14a: the landblock-info static entities of
//! type Weenie (6), Physics (7) or PatrolPoint (23) with the clearance volumes of their
//! `Entity_Clearance_BoxList` and `Entity_Clearance_CylinderList` properties (the SED's
//! own value, else its object's), and the overlap tests against them.
//!
//! Tessera Decision of §7.4 step 14a: every entity of the three types is kept (Generator
//! targets are not dropped; the client's offline filter mode is OPEN).

use std::sync::Arc;

use tessera_formats::landblock::StaticEntityType;
use tessera_formats::setup::{ENTITY_PREFIX, Quaternion};
use tessera_formats::{
    Did, LandblockInfo, PropertyDictionary, PropertySchema, PropertySet, PropertyValue, Record,
    object_property_set_did,
};

use super::error::SceneryError;
use super::math::{self, EPS};
use super::props::{ObjectProperties, ObjectPropertyCache, read_set};
use crate::context::{PROPERTY_DICTIONARY_DID, WorldContext};
use crate::source::WorldSource;

/// Most volumes of each kind kept per entity (hostile-input bound; real lists are short).
pub const MAX_VOLUMES_PER_ENTITY: usize = 256;
/// Most volumes (boxes and cylinders together) kept in one landblock's static list
/// (hostile-input bound: it caps the list's memory and the cost of each step-14a test; the
/// install's landblocks hold far fewer, see `scenery_real_data.rs`). Entities after the
/// cap is reached are left out, with a warning.
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

/// The property IDs of the clearance volumes, looked up by name at run time.
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

/// A clearance box in its entity's frame.
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct ClearanceBox {
    /// Minimum corner.
    pub min: [f32; 3],
    /// Maximum corner.
    pub max: [f32; 3],
}

/// A clearance cylinder in its entity's frame.
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct ClearanceCylinder {
    /// Centre of the bottom face.
    pub low_point: [f32; 3],
    /// Radius.
    pub radius: f32,
    /// Height (not used by the 2D test).
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
    /// spacing radius `rho`: p is mapped into the entity frame (no scale); a box overlaps
    /// when the squared distance to it (z only when `spacing_3d`) is at most ρ², unless all
    /// three extents are at most 0.0002; a cylinder with a positive radius overlaps when
    /// the horizontal distance to its axis is at most ρ + radius.
    ///
    /// With `spacing_3d` the cylinder test is the 2D one: the client's sphere-against-
    /// cylinder form is OPEN (§15 item 5) and no node in the install sets the flag.
    ///
    /// Hostile input: when the mapped point is not finite (a NaN or overflowing rotation
    /// or position) nothing overlaps; [`static_clearances`] never builds such an entry.
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
        for b in &self.boxes {
            let ext = [
                b.max[0] - b.min[0],
                b.max[1] - b.min[1],
                b.max[2] - b.min[2],
            ];
            if ext.iter().all(|e| *e <= EPS) {
                continue;
            }
            let axes = if spacing_3d { 3 } else { 2 };
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
            if dx * dx + dy * dy <= r * r {
                return true;
            }
        }
        false
    }
}

/// Reads the clearance volumes of a box-list and a cylinder-list value. Elements that are
/// not structs or lack a field are skipped; non-finite values are skipped; at most
/// [`MAX_VOLUMES_PER_ENTITY`] of each kind.
#[must_use]
pub fn volumes_from_values(
    box_list: Option<&PropertyValue>,
    cylinder_list: Option<&PropertyValue>,
    pids: &ClearancePids,
) -> (Vec<ClearanceBox>, Vec<ClearanceCylinder>) {
    let vec3 = |s: &tessera_formats::PropertyStruct, pid: Option<u32>| match s.get(pid?)? {
        PropertyValue::Vector3(v) if v.iter().all(|c| c.is_finite()) => Some(*v),
        _ => None,
    };
    let float = |s: &tessera_formats::PropertyStruct, pid: Option<u32>| {
        s.get(pid?)?.as_f32().filter(|v| v.is_finite())
    };
    let mut boxes = Vec::new();
    if let Some(a) = box_list.and_then(PropertyValue::as_array) {
        for e in a.elements.iter().take(MAX_VOLUMES_PER_ENTITY) {
            let Some(s) = e.value.as_struct() else {
                continue;
            };
            if let (Some(min), Some(max)) = (vec3(s, pids.box_min), vec3(s, pids.box_max)) {
                boxes.push(ClearanceBox { min, max });
            }
        }
    }
    let mut cylinders = Vec::new();
    if let Some(a) = cylinder_list.and_then(PropertyValue::as_array) {
        for e in a.elements.iter().take(MAX_VOLUMES_PER_ENTITY) {
            let Some(s) = e.value.as_struct() else {
                continue;
            };
            if let (Some(low_point), Some(radius)) =
                (vec3(s, pids.low_point), float(s, pids.radius))
            {
                cylinders.push(ClearanceCylinder {
                    low_point,
                    radius,
                    height: float(s, pids.height).unwrap_or(0.0),
                });
            }
        }
    }
    (boxes, cylinders)
}

/// Builds the static list of a landblock-info record (§7.4 step 14a): entities of type
/// Weenie, Physics or PatrolPoint with a placement, each with the volumes of its own
/// property set, else of its object (a 0x47 object resolved along its chain, §9.2; a
/// 0x70 game object through its own property set at DID + 0x09000000). Entities without
/// any volume are left out (they never overlap).
///
/// Hostile input (Tessera Decisions, `scenery.md` §13.1 OPEN): an entity whose position
/// or rotation is not finite, or whose rotation has zero length, is left out; a finite
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
/// errors (their entities carry no volume).
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
        if !matches!(
            e.kind(),
            Some(
                StaticEntityType::Weenie
                    | StaticEntityType::Physics
                    | StaticEntityType::PatrolPoint
            )
        ) {
            continue;
        }
        let Some(placement) = e.placement else {
            continue;
        };
        if !placement.position.iter().all(|c| c.is_finite()) {
            continue;
        }
        let Some(rotation) = usable_rotation(placement.rotation) else {
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
        let own = e.properties.as_ref();
        let own_get = |pid: Option<u32>| own.and_then(|s| s.get(pid?));
        let own_boxes = own_get(pids.box_list);
        let own_cylinders = own_get(pids.cylinder_list);
        let lists = match e.object {
            Some(object) if own_boxes.is_none() || own_cylinders.is_none() => {
                ObjectLists::load(object, source, schema, objects)?
            }
            _ => ObjectLists::None,
        };
        let (mut boxes, mut cylinders) = volumes_from_values(
            own_boxes.or_else(|| lists.get(pids.box_list)),
            own_cylinders.or_else(|| lists.get(pids.cylinder_list)),
            pids,
        );
        let room = MAX_STATIC_VOLUMES - total;
        boxes.truncate(room);
        cylinders.truncate(room - boxes.len());
        let entry = StaticClearance {
            position: placement.position,
            rotation,
            boxes,
            cylinders,
        };
        if !entry.is_empty() {
            total += entry.volume_count();
            out.push(entry);
        }
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

/// Where an entity's object keeps its clearance lists.
enum ObjectLists {
    /// No object, or one that cannot be used.
    None,
    /// A 0x47 object resolved along its chain.
    Entity(Arc<ObjectProperties>),
    /// A game object's own property set.
    Set(PropertySet),
}

impl ObjectLists {
    fn load(
        object: Did,
        source: &dyn WorldSource,
        schema: &dyn PropertySchema,
        objects: &ObjectPropertyCache,
    ) -> Result<Self, SceneryError> {
        if object.prefix() == ENTITY_PREFIX {
            return Ok(objects
                .get(source, schema, object)?
                .map_or(Self::None, Self::Entity));
        }
        let Some(set_did) = object_property_set_did(object) else {
            return Ok(Self::None);
        };
        match read_set(source, schema, set_did) {
            Ok(set) => Ok(Self::Set(set)),
            Err(e) if e.is_data_error() => Ok(Self::None),
            Err(e) => Err(e),
        }
    }

    fn get(&self, pid: Option<u32>) -> Option<&PropertyValue> {
        let pid = pid?;
        match self {
            Self::None => None,
            Self::Entity(p) => p.get(pid),
            Self::Set(s) => s.get(pid),
        }
    }
}
