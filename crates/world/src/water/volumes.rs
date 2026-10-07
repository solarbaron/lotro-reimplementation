// SPDX-License-Identifier: GPL-3.0-or-later
//! Water volumes (`water.md` §7): the monitor boxes of an entity's 0x47 record
//! (`Entity_Monitors`, `sound.md` §10.3.4) with the flags of the property sets they apply
//! (§7.2), placed for the landblock water-volume entity (§7.1) and for placed static
//! entities (§7.3).

use std::collections::HashMap;
use std::sync::{Arc, Mutex, PoisonError};

use tessera_formats::setup::{ENTITY_PREFIX, Quaternion};
use tessera_formats::{Did, PropertySchema, PropertyValue};
use tessera_physics::geometry::{
    LiquidBox, LiquidFlags, LiquidPids, Transform, content_layers_pass,
};

use super::WaterError;
use crate::bundle::PlacedEntity;
use crate::scenery::{ObjectProperties, read_set, value_bool};
use crate::source::WorldSource;

/// Most monitors read from one object (the install has at most a handful per descriptor;
/// more is hostile input and is cut, with a debug log).
pub const MAX_MONITORS_PER_OBJECT: usize = 64;
/// Default bound of a [`VolumeLibrary`] in entries (the install has 48 descriptors with
/// liquid monitors and a few hundred with monitors of any kind).
pub const DEFAULT_VOLUME_CACHE_ENTRIES: usize = 4_096;

/// The property IDs of monitors (`sound.md` §10.3.4, `water.md` §7.1), looked up by name
/// at run time.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub struct MonitorPids {
    /// `Entity_Monitors`: the array of monitor structs.
    pub monitors: Option<u32>,
    /// `Monitor_BoxMin` (vec3, entity frame).
    pub box_min: Option<u32>,
    /// `Monitor_BoxMax` (vec3, entity frame).
    pub box_max: Option<u32>,
    /// `Monitor_Offset` (vec3).
    pub offset: Option<u32>,
    /// `Monitor_ScaleWithEntity` (bool).
    pub scale_with_entity: Option<u32>,
    /// `Monitor_VolumePropertiesToApply` (a 0x78 DID).
    pub properties: Option<u32>,
    /// `Monitor_Name` (enum; mapper 0x23000096 in the observed install).
    pub name: Option<u32>,
}

impl MonitorPids {
    /// Looks every PID up with `pid_of(name)`.
    #[must_use]
    pub fn from_names(pid_of: impl Fn(&str) -> Option<u32>) -> Self {
        Self {
            monitors: pid_of("Entity_Monitors"),
            box_min: pid_of("Monitor_BoxMin"),
            box_max: pid_of("Monitor_BoxMax"),
            offset: pid_of("Monitor_Offset"),
            scale_with_entity: pid_of("Monitor_ScaleWithEntity"),
            properties: pid_of("Monitor_VolumePropertiesToApply"),
            name: pid_of("Monitor_Name"),
        }
    }
}

/// Every property ID the water module reads: monitors and the §7.2 volume flags.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub struct WaterPids {
    /// Monitor structure.
    pub monitor: MonitorPids,
    /// Volume flags of the applied property sets.
    pub liquid: LiquidPids,
}

impl WaterPids {
    /// Looks every PID up with `pid_of(name)`.
    #[must_use]
    pub fn from_names(pid_of: impl Fn(&str) -> Option<u32> + Copy) -> Self {
        Self {
            monitor: MonitorPids::from_names(pid_of),
            liquid: LiquidPids::from_names(pid_of),
        }
    }
}

/// One `Entity_Monitor` struct as stored (`sound.md` §10.3.4).
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct Monitor {
    /// `Monitor_Name` enum key (1 WaterSurfaceVolume, 2 WaterVolume, 3 WaterWadeVolume in
    /// the observed mapper; labels are data).
    pub name: Option<u32>,
    /// `Monitor_BoxMin`, entity frame.
    pub min: [f32; 3],
    /// `Monitor_BoxMax`, entity frame.
    pub max: [f32; 3],
    /// `Monitor_Offset`, entity frame; (0, 0, 0) when absent.
    pub offset: [f32; 3],
    /// `Monitor_ScaleWithEntity`. **Tessera Decision** (OPEN in `water.md` §7.1): true when
    /// absent.
    pub scale_with_entity: bool,
    /// `Monitor_VolumePropertiesToApply`, when present and not 0.
    pub properties: Option<Did>,
}

impl Monitor {
    /// The monitor's box in the entity frame with its offset applied: (min + offset,
    /// max + offset). **Tessera Decision** (OPEN in `water.md` §7.1): the offset moves the
    /// box in the entity frame before the entity's scale, rotation and position.
    #[must_use]
    pub fn local_box(&self) -> ([f32; 3], [f32; 3]) {
        (
            std::array::from_fn(|a| self.min[a] + self.offset[a]),
            std::array::from_fn(|a| self.max[a] + self.offset[a]),
        )
    }
}

fn field(s: &tessera_formats::PropertyStruct, pid: Option<u32>) -> Option<&PropertyValue> {
    pid.and_then(|p| s.get(p))
}

fn vec3(v: Option<&PropertyValue>) -> Option<[f32; 3]> {
    match v? {
        PropertyValue::Vector3(v) if v.iter().all(|c| c.is_finite()) => Some(*v),
        _ => None,
    }
}

/// Reads the monitors of an `Entity_Monitors` value (an array of structs). Elements that
/// are not structs, or lack a finite box minimum or maximum, are skipped; at most
/// [`MAX_MONITORS_PER_OBJECT`] are read.
#[must_use]
pub fn monitors_from_value(value: &PropertyValue, pids: &MonitorPids) -> Vec<Monitor> {
    let PropertyValue::Array(array) = value else {
        return Vec::new();
    };
    let mut out = Vec::new();
    for el in &array.elements {
        if out.len() >= MAX_MONITORS_PER_OBJECT {
            tracing::debug!(
                limit = MAX_MONITORS_PER_OBJECT,
                "monitors beyond the limit skipped"
            );
            break;
        }
        let PropertyValue::Struct(s) = &el.value else {
            continue;
        };
        let (Some(min), Some(max)) = (vec3(field(s, pids.box_min)), vec3(field(s, pids.box_max)))
        else {
            continue;
        };
        let name = match field(s, pids.name) {
            Some(PropertyValue::Enum(k)) => Some(*k),
            Some(v) => v.as_bits().and_then(|b| u32::try_from(b).ok()),
            None => None,
        };
        let properties = match field(s, pids.properties) {
            Some(PropertyValue::DidRef(d)) if d.0 != 0 => Some(*d),
            _ => None,
        };
        out.push(Monitor {
            name,
            min,
            max,
            offset: vec3(field(s, pids.offset)).unwrap_or([0.0; 3]),
            scale_with_entity: field(s, pids.scale_with_entity)
                .and_then(value_bool)
                .unwrap_or(true),
            properties,
        });
    }
    out
}

/// One liquid monitor box of an object, in the entity frame (offset applied).
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct VolumeBox {
    /// Box corner, entity frame.
    pub min: [f32; 3],
    /// Opposite corner, entity frame.
    pub max: [f32; 3],
    /// Whether the entity's scale applies to the box (`Monitor_ScaleWithEntity`).
    pub scale_with_entity: bool,
    /// The flags of its applied property set (§7.2); `liquid` is always set.
    pub flags: LiquidFlags,
    /// The applied property set (`Monitor_VolumePropertiesToApply`, a 0x78 DID).
    pub properties: Did,
    /// `Monitor_Name` enum key.
    pub name: Option<u32>,
}

/// The liquid monitor boxes of one 0x47 object (`water.md` §7.1, §7.3). Monitors whose
/// applied property set does not carry `Volume_IsLiquid` (music, sky and other monitors)
/// are not kept.
#[derive(Debug, Clone, PartialEq, Default)]
pub struct VolumeTemplate {
    /// The object.
    pub object: Did,
    /// Its liquid boxes in stored order.
    pub boxes: Vec<VolumeBox>,
    /// Monitors skipped because their property set was missing or did not decode.
    pub unreadable_sets: u32,
}

impl VolumeTemplate {
    /// Builds the template from monitors, with `flags_of(did)` giving the §7.2 flags of an
    /// applied property set (`None` when the set cannot be read). Boxes whose corners are
    /// inverted are skipped.
    pub fn from_monitors(
        object: Did,
        monitors: &[Monitor],
        mut flags_of: impl FnMut(Did) -> Option<LiquidFlags>,
    ) -> Self {
        let mut out = Self {
            object,
            ..Self::default()
        };
        for m in monitors {
            let Some(set) = m.properties else { continue };
            let Some(flags) = flags_of(set) else {
                out.unreadable_sets += 1;
                continue;
            };
            if !flags.liquid {
                continue;
            }
            let (min, max) = m.local_box();
            if (0..3).any(|a| min[a].partial_cmp(&max[a]).is_none_or(|o| o.is_gt())) {
                continue;
            }
            out.boxes.push(VolumeBox {
                min,
                max,
                scale_with_entity: m.scale_with_entity,
                flags,
                properties: set,
                name: m.name,
            });
        }
        out
    }

    /// Whether the object has no liquid box.
    #[must_use]
    pub fn is_empty(&self) -> bool {
        self.boxes.is_empty()
    }

    /// The boxes placed by an entity frame (position and rotation in the landblock frame,
    /// per-axis `scale`), as physics liquid boxes. A box whose monitor does not scale with
    /// the entity gets scale 1. Boxes the physics layer rejects (non-finite or
    /// out-of-range frames) are skipped.
    #[must_use]
    pub fn place(
        &self,
        position: [f32; 3],
        rotation: Quaternion,
        scale: [f32; 3],
    ) -> Vec<LiquidBox> {
        self.boxes
            .iter()
            .filter_map(|b| {
                let transform = Transform {
                    position,
                    rotation,
                    scale: if b.scale_with_entity { scale } else { [1.0; 3] },
                };
                LiquidBox::new(b.min, b.max, &transform, b.flags).ok()
            })
            .collect()
    }

    /// The boxes of the landblock water-volume entity of a wet landblock: placed at
    /// block-local (0, 0, *h*) with identity rotation and scale 1 (`water.md` §7.1; the
    /// rotation and scale are Inferred there).
    #[must_use]
    pub fn place_landblock(&self, height: f32) -> Vec<LiquidBox> {
        self.place([0.0, 0.0, height], Quaternion::IDENTITY, [1.0; 3])
    }
}

/// Reads and caches [`VolumeTemplate`]s per 0x47 object, shared by every worker. Data
/// failures are cached as "no volume"; failures that are not the data's (the asset
/// pipeline's state) are returned and not cached. Bounded by entry count; emptied when
/// full.
#[derive(Debug)]
pub struct VolumeLibrary {
    pids: WaterPids,
    cache: Mutex<HashMap<Did, Option<Arc<VolumeTemplate>>>>,
    max_entries: usize,
}

impl VolumeLibrary {
    /// A library that reads with `pids` and holds at most `max_entries` objects (at least
    /// one).
    #[must_use]
    pub fn new(pids: WaterPids, max_entries: usize) -> Self {
        Self {
            pids,
            cache: Mutex::new(HashMap::new()),
            max_entries: max_entries.max(1),
        }
    }

    /// The property IDs in use.
    #[must_use]
    pub fn pids(&self) -> &WaterPids {
        &self.pids
    }

    /// Objects currently cached.
    #[must_use]
    pub fn len(&self) -> usize {
        self.cache
            .lock()
            .unwrap_or_else(PoisonError::into_inner)
            .len()
    }

    /// Whether nothing is cached.
    #[must_use]
    pub fn is_empty(&self) -> bool {
        self.len() == 0
    }

    /// The liquid boxes of object `did` (its own `Entity_Monitors` or the nearest parent's
    /// along the 0x47 chain), or `None` when it has none. Objects that are not 0x47 records
    /// have none.
    ///
    /// # Errors
    ///
    /// [`WaterError::Records`] for a read failure that is not the data's (retry later).
    pub fn template(
        &self,
        source: &dyn WorldSource,
        schema: &dyn PropertySchema,
        did: Did,
    ) -> Result<Option<Arc<VolumeTemplate>>, WaterError> {
        if did.prefix() != ENTITY_PREFIX || self.pids.monitor.monitors.is_none() {
            return Ok(None);
        }
        if let Some(hit) = self
            .cache
            .lock()
            .unwrap_or_else(PoisonError::into_inner)
            .get(&did)
        {
            return Ok(hit.clone());
        }
        let built = match self.build(source, schema, did) {
            Ok(t) => t,
            Err(e) if e.is_data_error() => {
                tracing::debug!(object = %format_args!("{:#010x}", did.0), error = %e, "water volume object unusable");
                None
            }
            Err(e) => return Err(e),
        };
        let mut cache = self.cache.lock().unwrap_or_else(PoisonError::into_inner);
        if cache.len() >= self.max_entries {
            tracing::debug!(entries = cache.len(), "water volume cache full; emptied");
            cache.clear();
        }
        cache.insert(did, built.clone());
        Ok(built)
    }

    fn build(
        &self,
        source: &dyn WorldSource,
        schema: &dyn PropertySchema,
        did: Did,
    ) -> Result<Option<Arc<VolumeTemplate>>, WaterError> {
        let Some(pid) = self.pids.monitor.monitors else {
            return Ok(None);
        };
        let props = ObjectProperties::load(source, schema, did)?;
        let Some(value) = props.get(pid) else {
            return Ok(None);
        };
        let monitors = monitors_from_value(value, &self.pids.monitor);
        let mut failure = None;
        let template = VolumeTemplate::from_monitors(did, &monitors, |set| {
            match read_set(source, schema, set) {
                Ok(s) => Some(LiquidFlags::from_set(&s, &self.pids.liquid)),
                Err(e) if e.is_data_error() => {
                    tracing::debug!(set = %format_args!("{:#010x}", set.0), error = %e, "monitor property set unusable");
                    None
                }
                Err(e) => {
                    failure.get_or_insert(e);
                    None
                }
            }
        });
        if let Some(e) = failure {
            return Err(e.into());
        }
        Ok((!template.is_empty()).then(|| Arc::new(template)))
    }
}

/// The liquid boxes of a landblock's placed static entities ([`placed_volumes`]).
#[derive(Debug, Clone, PartialEq, Default)]
pub struct PlacedVolumes {
    /// The placed boxes, landblock frame, in entity order; at most the `limit` passed.
    pub boxes: Vec<LiquidBox>,
    /// Placed entities that carried volumes (including those whose boxes were left out).
    pub entities: u32,
    /// Template boxes left out because the limit was reached (an upper bound: boxes whose
    /// frame the physics layer would reject are counted too).
    pub left_out: u32,
}

/// The liquid boxes of a landblock's placed static entities (`water.md` §7.3): every
/// entity that names a 0x47 object with liquid monitors and whose content layers pass for
/// `active_layers` (`entity-descriptor.md` §4.5.1, instance 0), placed by its frame and
/// scale, at most `limit` boxes in all (a landblock's collision set holds at most
/// [`MAX_LIQUID_BOXES`](tessera_physics::geometry::MAX_LIQUID_BOXES); boxes past the limit are left out, counted and logged).
///
/// # Errors
///
/// As [`VolumeLibrary::template`].
pub fn placed_volumes(
    library: &VolumeLibrary,
    source: &dyn WorldSource,
    schema: &dyn PropertySchema,
    entities: &[PlacedEntity],
    active_layers: &[u32],
    limit: usize,
) -> Result<PlacedVolumes, WaterError> {
    let mut out = PlacedVolumes::default();
    for e in entities {
        let Some(object) = e.object else { continue };
        if !content_layers_pass(e.content_layers.as_deref(), active_layers, 0) {
            continue;
        }
        let Some(t) = library.template(source, schema, object)? else {
            continue;
        };
        out.entities = out.entities.saturating_add(1);
        let room = limit.saturating_sub(out.boxes.len());
        if room == 0 {
            out.left_out = out
                .left_out
                .saturating_add(u32::try_from(t.boxes.len()).unwrap_or(u32::MAX));
            continue;
        }
        let placed = t.place(e.placement.position, e.placement.rotation, e.scale);
        let cut = placed.len().saturating_sub(room);
        out.left_out = out
            .left_out
            .saturating_add(u32::try_from(cut).unwrap_or(u32::MAX));
        out.boxes.extend(placed.into_iter().take(room));
    }
    if out.left_out > 0 {
        tracing::debug!(
            limit,
            left_out = out.left_out,
            "placed liquid volumes over the per-landblock limit; boxes left out"
        );
    }
    Ok(out)
}
