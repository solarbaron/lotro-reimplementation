// SPDX-License-Identifier: GPL-3.0-or-later
//! The far view's run-time state: residency, records, grid, batches, sprites and the
//! distant set, driven once per frame (`far-view.md` §3.6; Tessera Decision FD3).
//!
//! The client's landscape manager does the stages of §3.6 on its main thread under a time
//! budget. Tessera splits them into work items that the caller runs wherever it likes (the
//! walker uses its asset workers): [`FarViewState::update`] returns the sectors and imposter
//! tables to read and the batches and sprite lists to build; the caller hands the results
//! back ([`FarViewState::sector_loaded`], [`FarViewState::table_loaded`],
//! [`FarViewState::built`], [`FarViewState::sprites_placed`]) and they are swapped in per
//! sector. A result built for an older generation is dropped.

use std::collections::{BTreeMap, BTreeSet};
use std::sync::Arc;

use tessera_formats::landblock::SectorWaterPair;
use tessera_formats::{Did, ImposterTable, SectorRecord};

use super::entities::{DistantEntry, DistantSet};
use super::grid::{LandblockDescriptor, LandblockGrid, NearField, SECTOR_SIDE, SectorKey};
use super::mesh::{Batch, FarMesh, MeshKind, build_batch};
use super::residency::{Change, Residency, Viewer};
use super::settings::{BLOCK, FarViewSettings};
use super::sprites::{FarSprite, SpriteView, place_sprites};
use crate::error::Result;

/// Where a sector's record stands.
#[derive(Debug, Clone)]
pub enum SectorRecordState {
    /// Requested, not back yet.
    Loading,
    /// No record for this key (DL-E1: nothing is drawn, not an error).
    Missing,
    /// The record failed to read or decode (logged once, nothing drawn, §5.8).
    Failed,
    /// Decoded.
    Ready(Arc<SectorRecord>),
}

/// The built batches of a sector (far batches always; near batches for tools).
#[derive(Debug, Clone, PartialEq, Default)]
pub struct SectorMeshes {
    /// The sector.
    pub key: Option<SectorKey>,
    /// Generation the batches were built for.
    pub generation: u64,
    /// Far terrain batch.
    pub terrain: FarMesh,
    /// Far ceiling batch (records with a ceiling).
    pub ceiling: Option<FarMesh>,
    /// Near terrain batch (only with [`FarViewSettings::near_batches`]).
    pub near_terrain: Option<FarMesh>,
    /// Near ceiling batch (only with [`FarViewSettings::near_batches`]).
    pub near_ceiling: Option<FarMesh>,
}

/// One sector's slot.
#[derive(Debug, Clone)]
pub struct SectorSlot {
    /// The record.
    pub record: SectorRecordState,
    /// Listed now (else cached and detached, §3.5).
    pub listed: bool,
    /// Descriptors of the last build requested.
    pub descriptors: Option<[LandblockDescriptor; 9]>,
    /// Generation of the last build requested.
    pub generation: u64,
    /// The batches swapped in last (possibly of an older generation while a newer one
    /// builds).
    pub meshes: Option<Arc<SectorMeshes>>,
    /// Sprite epoch of the last sprite job requested.
    pub sprite_epoch: u64,
    /// The sprites placed last, with their epoch.
    pub sprites: Option<(u64, Arc<[FarSprite]>)>,
}

impl SectorSlot {
    fn new() -> Self {
        Self {
            record: SectorRecordState::Loading,
            listed: true,
            descriptors: None,
            generation: 0,
            meshes: None,
            sprite_epoch: 0,
            sprites: None,
        }
    }

    /// The record, when decoded.
    #[must_use]
    pub fn ready(&self) -> Option<&Arc<SectorRecord>> {
        match &self.record {
            SectorRecordState::Ready(r) => Some(r),
            _ => None,
        }
    }
}

/// A batch build (runs anywhere; [`BuildJob::run`]).
#[derive(Debug, Clone)]
pub struct BuildJob {
    /// The sector.
    pub key: SectorKey,
    /// Generation.
    pub generation: u64,
    /// The record.
    pub record: Arc<SectorRecord>,
    /// The nine descriptors.
    pub descriptors: [LandblockDescriptor; 9],
    /// Build the near batches too.
    pub near_batches: bool,
}

impl BuildJob {
    /// Builds the batches.
    ///
    /// # Errors
    ///
    /// As [`build_batch`].
    pub fn run(&self) -> Result<SectorMeshes> {
        let r = &self.record;
        let did = r.did;
        let build = |mesh, kind, batch| {
            build_batch(did, mesh, r.block_size, &self.descriptors, kind, batch)
        };
        let terrain = build(&r.terrain, MeshKind::Terrain, Batch::Far)?;
        let ceiling = match &r.ceiling {
            Some(c) => Some(build(c, MeshKind::Ceiling, Batch::Far)?),
            None => None,
        };
        let (near_terrain, near_ceiling) = if self.near_batches {
            (
                Some(build(&r.terrain, MeshKind::Terrain, Batch::Near)?),
                match &r.ceiling {
                    Some(c) => Some(build(c, MeshKind::Ceiling, Batch::Near)?),
                    None => None,
                },
            )
        } else {
            (None, None)
        };
        Ok(SectorMeshes {
            key: Some(self.key),
            generation: self.generation,
            terrain,
            ceiling,
            near_terrain,
            near_ceiling,
        })
    }
}

/// A sprite placement (runs anywhere; [`SpriteJob::run`]).
#[derive(Debug, Clone)]
pub struct SpriteJob {
    /// The sector.
    pub key: SectorKey,
    /// Sprite epoch.
    pub epoch: u64,
    /// The record.
    pub record: Arc<SectorRecord>,
    /// The sector's imposter table.
    pub table: Arc<ImposterTable>,
    /// The sector's south-west corner relative to the viewer landblock's (units).
    pub offset: [f32; 2],
    /// The camera values of §9.3.
    pub view: SpriteView,
}

impl SpriteJob {
    /// Places the sprites (§9.2–§9.5).
    #[must_use]
    pub fn run(&self) -> Arc<[FarSprite]> {
        let mut out = Vec::new();
        let skipped = place_sprites(
            &self.record.sprites,
            &self.table,
            self.offset,
            &self.view,
            &mut out,
        );
        if skipped > 0 {
            tracing::trace!(did = %self.record.did, skipped, "sprites without an imposter entry skipped");
        }
        out.into()
    }
}

/// Work of one frame.
#[derive(Debug, Clone, Default)]
pub struct FarViewWork {
    /// The residency change handled (`None`: no list change this frame).
    pub change: Option<Change>,
    /// Every cached sector was freed (§3.4 discard); the caller drops what it holds.
    pub discarded: bool,
    /// Sector records to read.
    pub load_sectors: Vec<SectorKey>,
    /// Imposter tables to read.
    pub load_tables: Vec<Did>,
    /// Batches to build.
    pub build: Vec<BuildJob>,
    /// Sprite lists to place.
    pub sprites: Vec<SpriteJob>,
    /// Sectors that left the list (detach their meshes).
    pub detached: Vec<SectorKey>,
    /// Sectors freed (drop their GPU objects).
    pub freed: Vec<SectorKey>,
}

#[derive(Debug, Clone)]
enum TableState {
    Loading,
    Missing,
    Ready(Arc<ImposterTable>),
}

/// The far view's state (module docs).
#[derive(Debug)]
pub struct FarViewState {
    settings: FarViewSettings,
    residency: Residency,
    slots: BTreeMap<SectorKey, SectorSlot>,
    tables: BTreeMap<Did, TableState>,
    grid: Option<LandblockGrid>,
    grid_dirty: bool,
    near: NearField,
    view: SpriteView,
    viewer: Option<Viewer>,
    generation: u64,
    sprite_epoch: u64,
    distant: DistantSet,
    content_layers_pid: Option<u32>,
    failures_logged: BTreeSet<SectorKey>,
}

impl FarViewState {
    /// A state with `settings`; `content_layers_pid` is the PID of `Entity_ContentLayers`
    /// (FD7).
    #[must_use]
    pub fn new(settings: FarViewSettings, content_layers_pid: Option<u32>) -> Self {
        Self {
            residency: Residency::new(settings.far_radius, settings.keep_radius()),
            settings,
            slots: BTreeMap::new(),
            tables: BTreeMap::new(),
            grid: None,
            grid_dirty: true,
            near: NearField::Radius(settings.near_radius),
            view: SpriteView::default(),
            viewer: None,
            generation: 0,
            sprite_epoch: 1,
            distant: DistantSet::default(),
            content_layers_pid,
            failures_logged: BTreeSet::new(),
        }
    }

    /// The settings.
    #[must_use]
    pub fn settings(&self) -> &FarViewSettings {
        &self.settings
    }

    /// Changes the settings (§3.4: a radius change rebuilds, a template change discards).
    pub fn set_settings(&mut self, settings: FarViewSettings) {
        if settings.template() != self.settings.template() {
            self.residency.request(Change::Discard);
        }
        if settings.sprites() != self.settings.sprites() {
            self.sprite_epoch += 1;
        }
        if settings.near_batches != self.settings.near_batches {
            for s in self.slots.values_mut() {
                s.descriptors = None;
            }
            self.grid_dirty = true;
        }
        self.residency
            .set_radii(settings.far_radius, settings.keep_radius());
        self.settings = settings;
    }

    /// Requests a discard of every sector at the next update (a reset).
    pub fn reset(&mut self) {
        self.residency.request(Change::Discard);
    }

    /// The sectors listed now.
    #[must_use]
    pub fn listed(&self) -> &[SectorKey] {
        self.residency.listed()
    }

    /// A sector's slot.
    #[must_use]
    pub fn slot(&self, key: SectorKey) -> Option<&SectorSlot> {
        self.slots.get(&key)
    }

    /// Every cached slot.
    pub fn slots(&self) -> impl Iterator<Item = (&SectorKey, &SectorSlot)> + '_ {
        self.slots.iter()
    }

    /// The per-landblock grid of the last rebuild.
    #[must_use]
    pub fn grid(&self) -> Option<&LandblockGrid> {
        self.grid.as_ref()
    }

    /// The viewer of the last update.
    #[must_use]
    pub fn viewer(&self) -> Option<Viewer> {
        self.viewer
    }

    /// The distant set (§10).
    pub fn distant_mut(&mut self) -> &mut DistantSet {
        &mut self.distant
    }

    /// The distant set (§10).
    #[must_use]
    pub fn distant(&self) -> &DistantSet {
        &self.distant
    }

    /// An imposter table, when loaded.
    #[must_use]
    pub fn table(&self, did: Did) -> Option<&Arc<ImposterTable>> {
        match self.tables.get(&did) {
            Some(TableState::Ready(t)) => Some(t),
            _ => None,
        }
    }

    /// Whether every listed sector's record, batches and sprites are in.
    #[must_use]
    pub fn settled(&self) -> bool {
        self.residency.listed().iter().all(|k| {
            self.slots.get(k).is_some_and(|s| match &s.record {
                SectorRecordState::Loading => false,
                SectorRecordState::Missing | SectorRecordState::Failed => true,
                SectorRecordState::Ready(r) => {
                    let meshes = s
                        .meshes
                        .as_ref()
                        .is_some_and(|m| m.generation == s.generation);
                    let sprites = !self.settings.sprites()
                        || r.sprites.is_empty()
                        || matches!(
                            self.tables.get(&r.imposter_table),
                            Some(TableState::Missing)
                        )
                        || s.sprites
                            .as_ref()
                            .is_some_and(|(e, _)| *e == s.sprite_epoch && *e == self.sprite_epoch);
                    meshes && sprites
                }
            })
        })
    }

    fn sector_offset(&self, key: SectorKey) -> Option<[f32; 2]> {
        let v = self.viewer?.landblock;
        let (cx, cy) = key.corner();
        Some([
            (f32::from(cx) - f32::from(v.x())) * BLOCK,
            (f32::from(cy) - f32::from(v.y())) * BLOCK,
        ])
    }

    /// One frame (§3.6): takes the viewer, the near field (the walker's full-detail
    /// landblocks, FD9, or [`NearField::Radius`]) and the sprite camera values; returns the
    /// work to run.
    pub fn update(
        &mut self,
        viewer: Option<Viewer>,
        near: NearField,
        view: SpriteView,
    ) -> FarViewWork {
        let mut work = FarViewWork::default();
        if near != self.near {
            self.near = near;
            self.grid_dirty = true;
        }
        let view_changed = (view.target_height - self.view.target_height).abs() > 0.5
            || (view.fov_y - self.view.fov_y).abs() > 1e-4
            || (view.object_scale - self.view.object_scale).abs() > 1e-4;
        if view_changed {
            self.view = view;
            self.sprite_epoch += 1;
        }
        if let Some(r) = self.residency.update(viewer) {
            work.change = r.change;
            let landblock_changed = self.viewer.map(|v| v.landblock) != viewer.map(|v| v.landblock);
            self.viewer = viewer;
            if landblock_changed {
                self.sprite_epoch += 1;
            }
            if r.discarded {
                work.discarded = true;
                self.slots.clear();
                self.distant.clear();
                self.grid = None;
            }
            for k in &r.detached {
                if let Some(s) = self.slots.get_mut(k) {
                    s.listed = false;
                }
                self.distant.remove_sector(*k);
            }
            for k in &r.freed {
                self.slots.remove(k);
            }
            for k in &r.attached {
                match self.slots.get_mut(k) {
                    Some(s) => {
                        s.listed = true;
                        if let Some(rec) = s.ready().cloned() {
                            self.add_entities(*k, &rec);
                        }
                    }
                    None => {
                        self.slots.insert(*k, SectorSlot::new());
                        work.load_sectors.push(*k);
                    }
                }
            }
            work.detached = r.detached;
            work.freed = r.freed;
            if r.change != Some(Change::Retag) {
                self.grid_dirty = true;
            }
        } else {
            self.viewer = viewer;
        }
        if self.grid_dirty {
            self.grid_dirty = false;
            self.rebuild_grid(&mut work);
        }
        self.queue_sprites(&mut work);
        work
    }

    fn rebuild_grid(&mut self, work: &mut FarViewWork) {
        let Some(v) = self.viewer else {
            self.grid = None;
            return;
        };
        let listed = self.residency.listed().to_vec();
        let slots = &self.slots;
        let grid = LandblockGrid::build(
            v.landblock,
            &listed,
            |k| slots.get(&k).is_some_and(|s| s.ready().is_some()),
            &self.near,
        );
        for k in &listed {
            let Some(slot) = self.slots.get_mut(k) else {
                continue;
            };
            let Some(rec) = slot.ready().cloned() else {
                continue;
            };
            let d = grid.descriptors(*k, rec.ceiling_mask);
            if slot.descriptors.as_ref() != Some(&d) {
                self.generation += 1;
                slot.generation = self.generation;
                slot.descriptors = Some(d);
                work.build.push(BuildJob {
                    key: *k,
                    generation: self.generation,
                    record: rec,
                    descriptors: d,
                    near_batches: self.settings.near_batches,
                });
            }
        }
        self.grid = Some(grid);
    }

    fn queue_sprites(&mut self, work: &mut FarViewWork) {
        if !self.settings.sprites() {
            return;
        }
        let listed = self.residency.listed().to_vec();
        for k in listed {
            let Some(offset) = self.sector_offset(k) else {
                continue;
            };
            let Some(slot) = self.slots.get(&k) else {
                continue;
            };
            let Some(rec) = slot.ready().cloned() else {
                continue;
            };
            if rec.sprites.is_empty() || slot.sprite_epoch == self.sprite_epoch {
                continue;
            }
            let did = rec.imposter_table;
            let table = match self.tables.get(&did) {
                None => {
                    self.tables.insert(did, TableState::Loading);
                    work.load_tables.push(did);
                    continue;
                }
                Some(TableState::Loading | TableState::Missing) => continue,
                Some(TableState::Ready(t)) => Arc::clone(t),
            };
            if let Some(slot) = self.slots.get_mut(&k) {
                slot.sprite_epoch = self.sprite_epoch;
            }
            work.sprites.push(SpriteJob {
                key: k,
                epoch: self.sprite_epoch,
                record: rec,
                table,
                offset,
                view: self.view,
            });
        }
    }

    fn add_entities(&mut self, key: SectorKey, rec: &SectorRecord) {
        let pid = self.content_layers_pid;
        self.distant.add_sector(
            rec.entities
                .iter()
                .filter_map(|e| DistantEntry::from_sector(e, key, pid)),
        );
    }

    /// A sector read finished: `Ok(None)` when it has no record.
    pub fn sector_loaded(&mut self, key: SectorKey, result: Result<Option<SectorRecord>>) {
        let Some(slot) = self.slots.get_mut(&key) else {
            return;
        };
        if !matches!(slot.record, SectorRecordState::Loading) {
            return;
        }
        match result {
            Ok(Some(rec)) => {
                let rec = Arc::new(rec);
                slot.record = SectorRecordState::Ready(Arc::clone(&rec));
                if slot.listed {
                    self.add_entities(key, &rec);
                }
                self.grid_dirty = true;
            }
            Ok(None) => slot.record = SectorRecordState::Missing,
            Err(e) => {
                slot.record = SectorRecordState::Failed;
                if self.failures_logged.insert(key) {
                    tracing::warn!(did = %key.did(), error = %e, "distant-landscape sector skipped");
                }
            }
        }
    }

    /// An imposter-table read finished.
    pub fn table_loaded(&mut self, did: Did, result: Result<Option<ImposterTable>>) {
        let state = match result {
            Ok(Some(t)) => TableState::Ready(Arc::new(t)),
            Ok(None) => TableState::Missing,
            Err(e) => {
                tracing::warn!(did = %did, error = %e, "imposter table skipped");
                TableState::Missing
            }
        };
        self.tables.insert(did, state);
    }

    /// A batch build finished; returns whether it was swapped in (an older generation, a
    /// freed sector or a failed build is dropped).
    pub fn built(&mut self, key: SectorKey, generation: u64, result: Result<SectorMeshes>) -> bool {
        let Some(slot) = self.slots.get_mut(&key) else {
            return false;
        };
        match result {
            Ok(m) if generation == slot.generation => {
                slot.meshes = Some(Arc::new(m));
                true
            }
            Ok(_) => false,
            Err(e) => {
                if self.failures_logged.insert(key) {
                    tracing::warn!(did = %key.did(), error = %e, "distant-landscape sector mesh skipped");
                }
                false
            }
        }
    }

    /// A sprite placement finished; returns whether it was swapped in.
    pub fn sprites_placed(
        &mut self,
        key: SectorKey,
        epoch: u64,
        sprites: Arc<[FarSprite]>,
    ) -> bool {
        match self.slots.get_mut(&key) {
            Some(slot) if slot.sprite_epoch == epoch => {
                slot.sprites = Some((epoch, sprites));
                true
            }
            _ => false,
        }
    }

    /// The lowest floor over the far landblocks on the outer boundary of the grid
    /// (`far-view.md` §7.3 step 1, *z*_ring): the water height of a wet chunk, else the
    /// chunk's `min` *z*.
    #[must_use]
    pub fn ring_floor(&self) -> Option<f32> {
        let grid = self.grid.as_ref()?;
        let mut low: Option<f32> = None;
        for (x, y) in grid.outer_ring() {
            let Some(key) = SectorKey::new(grid.region(), x / SECTOR_SIDE, y / SECTOR_SIDE) else {
                continue;
            };
            let Some(rec) = self.slots.get(&key).and_then(SectorSlot::ready) else {
                continue;
            };
            let (lx, ly) = (usize::from(x % SECTOR_SIDE), usize::from(y % SECTOR_SIDE));
            let z = match rec.water_at(lx, ly) {
                Some(w) => Some(w.height),
                None => rec.terrain.chunk(lx, ly).map(|c| c.min[2]),
            };
            if let Some(z) = z.filter(|z| z.is_finite()) {
                low = Some(low.map_or(z, |l| l.min(z)));
            }
        }
        low
    }

    /// The lowest `min` *z* over the near-field landblocks of the listed sectors
    /// (`far-view.md` §7.3 step 1, *z*_near: the near-field terrain chunks' lowest bound;
    /// the sector chunks hold the same heights as the near terrain, §4.2).
    #[must_use]
    pub fn near_floor(&self) -> Option<f32> {
        let grid = self.grid.as_ref()?;
        let mut low: Option<f32> = None;
        for ((x, y), c) in grid.cells() {
            if !matches!(c, super::grid::Cell::Near(_)) {
                continue;
            }
            let Some(key) = SectorKey::new(grid.region(), x / SECTOR_SIDE, y / SECTOR_SIDE) else {
                continue;
            };
            let Some(rec) = self.slots.get(&key).and_then(SectorSlot::ready) else {
                continue;
            };
            let (lx, ly) = (usize::from(x % SECTOR_SIDE), usize::from(y % SECTOR_SIDE));
            if let Some(z) = rec
                .terrain
                .chunk(lx, ly)
                .map(|c| c.min[2])
                .filter(|z| z.is_finite())
            {
                low = Some(low.map_or(z, |l| l.min(z)));
            }
        }
        low
    }
}

/// The landscape water of a sector record (`far-view.md` §8, `distant-landscape.md` §5.5):
/// one entry per wet landblock (*x*, *y* offsets 0–2) with its water pair.
#[must_use]
pub fn sector_water(rec: &SectorRecord) -> Vec<(u8, u8, SectorWaterPair)> {
    let mut out = Vec::new();
    for x in 0..SECTOR_SIDE {
        for y in 0..SECTOR_SIDE {
            if let Some(w) = rec.water_at(usize::from(x), usize::from(y))
                && w.height.is_finite()
            {
                out.push((x, y, w));
            }
        }
    }
    out
}
