// SPDX-License-Identifier: GPL-3.0-or-later
//! Far entities from the sector entity lists: the entity streaming manager's distant set
//! (`far-view.md` §10, FV21).
//!
//! At each rebuild every entry of every listed sector is added to the set by instance ID
//! (an entry already present is kept); the entries of a sector that left the list are
//! removed and their entities destroyed. After the rebuild the set is walked: an entry
//! whose squared distance to the viewer reaches `view_distance_sq` × *s* is removed (its
//! entity destroyed); otherwise an entity is created for it when none exists with that
//! instance ID. A change of the viewer's instance first removes every distant entity.
//! Tessera Decision FD7 applies the content-layer test of `entity-descriptor.md` §4.5.1 to
//! an entry's own `Entity_ContentLayers`.

use std::collections::{BTreeMap, BTreeSet};

use tessera_formats::landblock::SectorEntity;
use tessera_formats::{Did, PropertyValue};

use super::grid::SectorKey;
use crate::coords::{LANDBLOCK_SIZE, WorldPos};

/// One distant-set entry (`distant-landscape.md` §3.8, §4.9).
#[derive(Debug, Clone, PartialEq)]
pub struct DistantEntry {
    /// Instance ID.
    pub iid: u64,
    /// Entity type (`entity-descriptor.md` §4.1).
    pub entity_type: u32,
    /// Object DID (an entity descriptor 0x47 or an object 0x70).
    pub object: Did,
    /// Uniform scale (the stored three values are equal).
    pub scale: f32,
    /// Region-global position.
    pub position: WorldPos,
    /// Orientation quaternion as stored (`None` when the Position has none).
    pub orientation: Option<[f32; 4]>,
    /// `view_distance_sq`.
    pub view_distance_sq: f32,
    /// The sector that listed it.
    pub sector: SectorKey,
    /// The entry's `Entity_ContentLayers` list, if its property is that one.
    pub content_layers: Option<Vec<u32>>,
}

impl DistantEntry {
    /// The entry of a sector entity (`content_layers_pid`: the PID of
    /// `Entity_ContentLayers`). The region and landblock come from the Position, else from
    /// the instance ID; a missing offset counts as the landblock's corner. `None` for a
    /// non-finite position or view distance.
    #[must_use]
    pub fn from_sector(
        e: &SectorEntity,
        sector: SectorKey,
        content_layers_pid: Option<u32>,
    ) -> Option<Self> {
        let region = e.position.region().unwrap_or_else(|| sector.region());
        let (bx, by) = e.position.landblock().unwrap_or_else(|| e.iid.landblock());
        let off = e.position.offset().unwrap_or([0.0; 3]);
        let position = WorldPos::new(
            region,
            LANDBLOCK_SIZE * f64::from(bx) + f64::from(off[0]),
            LANDBLOCK_SIZE * f64::from(by) + f64::from(off[1]),
            f64::from(off[2]),
        );
        if !position.is_finite() || !e.view_distance_sq.is_finite() {
            return None;
        }
        let content_layers = match (&e.property, content_layers_pid) {
            (Some(p), Some(pid)) if p.pid == pid => match &p.value {
                PropertyValue::Array(a) => Some(
                    a.elements
                        .iter()
                        .filter_map(|el| match &el.value {
                            PropertyValue::Enum(k) | PropertyValue::Bitfield32(k) => Some(*k),
                            #[allow(clippy::cast_sign_loss)]
                            PropertyValue::Int(i) => Some(*i as u32),
                            PropertyValue::DidRef(d) => Some(d.0),
                            _ => None,
                        })
                        .collect(),
                ),
                _ => None,
            },
            _ => None,
        };
        Some(Self {
            iid: e.iid.0,
            entity_type: e.entity_type,
            object: e.object,
            scale: e.scale[0],
            position,
            orientation: e.position.orientation(),
            view_distance_sq: e.view_distance_sq,
            sector,
            content_layers,
        })
    }

    /// Squared 3D distance to `p` (across landblocks); infinity across regions.
    #[must_use]
    pub fn distance_sq(&self, p: &WorldPos) -> f64 {
        if p.region != self.position.region {
            return f64::INFINITY;
        }
        (self.position.x - p.x).powi(2)
            + (self.position.y - p.y).powi(2)
            + (self.position.z - p.z).powi(2)
    }
}

/// What a walk changed (§10 step 3).
#[derive(Debug, Clone, Default, PartialEq)]
pub struct DistantChanges {
    /// Entities to create.
    pub create: Vec<DistantEntry>,
    /// Instance IDs of distant entities to destroy.
    pub destroy: Vec<u64>,
}

/// The distant set (module docs).
#[derive(Debug, Clone, Default)]
pub struct DistantSet {
    entries: BTreeMap<u64, DistantEntry>,
    live: BTreeSet<u64>,
    instance: Option<u16>,
    /// Destroys queued by sector removals, handed out by the next walk.
    pending_destroy: Vec<u64>,
}

impl DistantSet {
    /// Entries held.
    #[must_use]
    pub fn len(&self) -> usize {
        self.entries.len()
    }

    /// Whether no entry is held.
    #[must_use]
    pub fn is_empty(&self) -> bool {
        self.entries.is_empty()
    }

    /// Every entry held.
    pub fn entries(&self) -> impl Iterator<Item = &DistantEntry> + '_ {
        self.entries.values()
    }

    /// Entities currently created from the set.
    pub fn live(&self) -> impl Iterator<Item = &DistantEntry> + '_ {
        self.live.iter().filter_map(|i| self.entries.get(i))
    }

    /// Whether the entity with instance ID `iid` was created from the set.
    #[must_use]
    pub fn is_live(&self, iid: u64) -> bool {
        self.live.contains(&iid)
    }

    /// Adds the entries of a listed sector (§10 step 1); entries already present are kept.
    pub fn add_sector(&mut self, entries: impl IntoIterator<Item = DistantEntry>) {
        for e in entries {
            self.entries.entry(e.iid).or_insert(e);
        }
    }

    /// Removes the entries of a sector that left the list; their entities are destroyed at
    /// the next walk.
    pub fn remove_sector(&mut self, key: SectorKey) {
        let gone: Vec<u64> = self
            .entries
            .iter()
            .filter(|(_, e)| e.sector == key)
            .map(|(i, _)| *i)
            .collect();
        for iid in gone {
            self.entries.remove(&iid);
            if self.live.remove(&iid) {
                self.pending_destroy.push(iid);
            }
        }
    }

    /// Removes everything (a discard, §3.4); every live entity is destroyed at the next walk.
    pub fn clear(&mut self) {
        self.pending_destroy.extend(self.live.iter().copied());
        self.live.clear();
        self.entries.clear();
    }

    /// Walks the set (§10 steps 2–3) for a viewer at `viewer` in instance `instance`, with
    /// manager factor `factor` (*s*, 1.0 at construction). `exists` tells whether an entity
    /// with an instance ID already exists through the main or long-range sets;
    /// `layers_pass` is the content-layer test of FD7.
    pub fn walk(
        &mut self,
        viewer: &WorldPos,
        instance: u16,
        factor: f32,
        exists: impl Fn(u64) -> bool,
        layers_pass: impl Fn(&DistantEntry) -> bool,
    ) -> DistantChanges {
        let mut out = DistantChanges {
            destroy: std::mem::take(&mut self.pending_destroy),
            ..DistantChanges::default()
        };
        if self.instance.is_some_and(|i| i != instance) {
            out.destroy.extend(self.live.iter().copied());
            self.live.clear();
        }
        self.instance = Some(instance);
        let s = if factor.is_finite() && factor >= 0.0 {
            f64::from(factor)
        } else {
            1.0
        };
        for (iid, e) in &self.entries {
            let d2 = e.distance_sq(viewer);
            let limit = f64::from(e.view_distance_sq) * s;
            if limit <= d2 || !layers_pass(e) {
                if self.live.remove(iid) {
                    out.destroy.push(*iid);
                }
            } else if !self.live.contains(iid) && !exists(*iid) {
                self.live.insert(*iid);
                out.create.push(e.clone());
            }
        }
        out
    }
}
