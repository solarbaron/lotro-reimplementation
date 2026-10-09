// SPDX-License-Identifier: GPL-3.0-or-later
//! The walker's far view (`docs/specs/far-view.md`, `--far-view on`): distant-landscape
//! sectors around the viewer drawn beyond the walker's full-detail terrain, with their
//! colour lists and the detail texture, the sectors' landscape water outside the
//! full-detail landblocks, and the distant sprites from the imposter atlases.
//!
//! - **Near field** (§2, §3.2, Tessera Decision FD9): by default the original's 3 × 3
//!   landblocks around the viewer's ([`NearFieldMode::Original`]; the host streams terrain
//!   within [`NEAR_FIELD_RING`]); with `--view-blocks` the walker's own full-detail ring
//!   ([`NearFieldMode::Ring`]). Only landblocks the terrain layer can draw (meshed, or
//!   deliberately out of the scene) enter the near field ([`near_field_of`]); the far
//!   batches cover the rest of the listed sectors, so an unmeshed near landblock is filled
//!   by the far mesh until its full-detail mesh arrives.
//! - **Hand-off** ([`FarHandOff`]): the terrain and water layers skip exactly the
//!   landblocks the far batches **on the GPU** cover, so while new batches build on the
//!   workers a landblock is drawn by the old far batch or by the full-detail terrain,
//!   never by both and never by neither (as long as the terrain still holds it). Terrain
//!   blocks next to the far field mesh at the finest level and ignore the far neighbour
//!   for their seam, so their edge has every sample the far stitch fans to (§5.2).
//! - **Work** (FD3): sector reads, imposter-table reads, batch builds and sprite placement
//!   run on the layer's own worker threads ([`super::jobs::JobRunner`]); results are swapped
//!   in per sector and uploaded within a per-frame cap ([`MAX_UPLOADS_PER_FRAME`]).
//! - **Textures** come from the object layer's texture store (colour lists, the detail
//!   texture of UNIQUEDB key 0x7A, the two imposter atlases), reference-counted per sector.
//! - **Far entities** (§10): the distant set is kept and walked every frame; drawing its
//!   entities through the object layer is not wired yet (the HUD reports the live count).
//! - **Horizon fill** (§7.3): the mesh of UNIQUEDB key 0x95 is placed as §7.3 says and drawn
//!   through the instanced path, only when its horizontal radius is at least 1.25 × *E*, so
//!   it can never hide listed terrain (its depth state is OPEN, `far-view.md` §12 note).

use std::collections::{HashMap, HashSet, VecDeque};
use std::sync::Arc;

use tessera_formats::{Did, ImposterTable, Record as _, SectorRecord};
use tessera_material::ShaderRecord;
use tessera_render::glam::{Mat4, Vec3};
use tessera_render::{
    DistantSprite, FarFadeParams, FarTerrainData, FarTerrainDraw, FarTerrainMesh, Instance,
    InstanceBatch, Pass, Renderer, SpriteBatch, SpriteDraw, TextureId, WaterDraw, WaterQuad,
};
use tessera_world::far_view::{
    BuildJob, Cell, FarMesh, FarSprite, FarViewRecords, FarViewSettings, FarViewState, MeshBounds,
    NearField, SectorKey, SectorMeshes, SpriteJob, SpriteView, Viewer, horizon,
    read_imposter_table, read_sector, sector_water,
};
use tessera_world::{
    DistanceSettings, LANDBLOCK_SIZE, LandblockKey, WorldContext, WorldError, WorldPos,
};

use super::geometry::{TexRequest, VARIANT_OPAQUE, VARIANT_OWN_ALPHA};
use super::jobs::JobRunner;
use super::objects::{ObjectLayer, TexLookup};
use super::resolver::Resolver;
use super::terrain::TextureStore;
use super::view::Anchor;

/// Most batch and sprite uploads per frame.
pub const MAX_UPLOADS_PER_FRAME: usize = 12;

/// Streaming ring (metres) of the full-detail terrain with the far view on and no
/// `--view-blocks`: two landblocks hold the 3 × 3 near field (§3.2) from any eye position in
/// the viewer's landblock (its farthest square is 160 √2 ≈ 226 m away) with room to spare.
pub const NEAR_FIELD_RING: f64 = 2.0 * LANDBLOCK_SIZE;

/// Which landblocks the walker draws at full detail with the far view on.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub enum NearFieldMode {
    /// The original's near field (§2, §3.2): Chebyshev distance ≤ *N* (1) from the viewer's
    /// landblock.
    #[default]
    Original,
    /// The walker's full-detail ring (`--view-blocks`, FD9 enhancement): every landblock
    /// whose square lies within the ring's radius.
    Ring,
}

/// What the far view hands to the full-detail terrain and water layers each frame (module
/// docs).
#[derive(Debug, Clone, Default, PartialEq, Eq)]
pub struct FarHandOff {
    /// Landblocks the far terrain batches on the GPU draw (and whose sector water the far
    /// view draws): the terrain and water layers skip them.
    pub covered: HashSet<LandblockKey>,
    /// `covered` plus the landblocks the current grid puts in the far field (batches still
    /// building): terrain blocks next to one mesh at the finest level and ignore it as a
    /// seam neighbour.
    pub far: HashSet<LandblockKey>,
}

impl FarHandOff {
    /// Whether a far batch on the GPU draws `key`.
    #[must_use]
    pub fn is_covered(&self, key: LandblockKey) -> bool {
        self.covered.contains(&key)
    }

    /// Whether `key` is (or is about to be) drawn by the far view.
    #[must_use]
    pub fn is_far(&self, key: LandblockKey) -> bool {
        self.far.contains(&key)
    }

    /// Whether a full-detail block at `key` borders the far field across an edge, so its
    /// edge must carry every sample the far stitch joins to (§5.2: the near field's
    /// full-resolution edge).
    #[must_use]
    pub fn borders_far(&self, key: LandblockKey) -> bool {
        !self.is_far(key)
            && [(-1, 0), (0, -1), (1, 0), (0, 1)]
                .iter()
                .any(|&(dx, dy)| key.offset(dx, dy).is_some_and(|n| self.is_far(n)))
    }
}

/// Most queued jobs.
const JOB_CAPACITY: usize = 256;

/// The far view's switches.
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct FarViewConfig {
    /// Draw the far view (`--far-view on`).
    pub enabled: bool,
    /// Which landblocks stay at full detail.
    pub near_field: NearFieldMode,
    /// Radii, template, sprites.
    pub settings: FarViewSettings,
    /// Worker threads.
    pub threads: usize,
}

impl Default for FarViewConfig {
    fn default() -> Self {
        Self {
            enabled: false,
            near_field: NearFieldMode::default(),
            settings: FarViewSettings::default(),
            threads: 2,
        }
    }
}

/// Counters of the last frame.
#[derive(Debug, Clone, Copy, Default, PartialEq, Eq)]
pub struct FarViewStats {
    /// Sectors listed.
    pub listed: usize,
    /// Listed sectors with a decoded record.
    pub ready: usize,
    /// Far-terrain batches drawn (terrain and ceiling).
    pub batches: usize,
    /// Sprite batches drawn.
    pub sprite_batches: usize,
    /// Sprites in the drawn batches.
    pub sprites: usize,
    /// Water quads drawn from the sectors.
    pub water_quads: usize,
    /// Landblocks the far terrain batches on the GPU cover.
    pub covered: usize,
    /// Live far entities of the distant set (not drawn yet).
    pub distant_live: usize,
    /// Jobs queued or running.
    pub pending: usize,
}

enum Job {
    Records,
    Template(Did),
    Sector(SectorKey),
    Table(Did),
    Build(BuildJob),
    Sprites(SpriteJob),
}

enum Done {
    Records(Result<FarViewRecords, WorldError>),
    Template(Did, Option<[f32; 2]>),
    Sector(SectorKey, Box<Result<Option<SectorRecord>, WorldError>>),
    Table(Did, Result<Option<ImposterTable>, WorldError>),
    Built(SectorKey, u64, Box<Result<SectorMeshes, WorldError>>),
    Sprites(SectorKey, u64, [f32; 2], Arc<[FarSprite]>),
}

#[derive(Default)]
struct SectorGpu {
    generation: u64,
    terrain: Option<FarTerrainMesh>,
    ceiling: Option<FarTerrainMesh>,
    /// The landblocks the uploaded terrain batch covers.
    covers: Vec<(u8, u8)>,
    sprite_epoch: u64,
    /// Sprites with the sector offset they were placed for (sector corner relative to the
    /// viewer landblock's).
    sprites: Option<(SpriteBatch, [f32; 2])>,
    textures: Vec<TexRequest>,
    textured: bool,
    atlased: bool,
}

/// The far-view layer (module docs).
pub struct FarViewLayer {
    ctx: Arc<WorldContext>,
    state: FarViewState,
    runner: Option<JobRunner<Job, Done>>,
    backlog: VecDeque<Job>,
    in_flight: usize,
    records: Option<FarViewRecords>,
    records_asked: bool,
    /// Detail repeats per sector from the plain template's transform set 0 (§6.2): `None`
    /// until read (the draws use 4 × 4 meanwhile).
    detail_repeats: Option<[f32; 2]>,
    template_asked: bool,
    template_done: bool,
    near_mode: NearFieldMode,
    hand_off: Arc<FarHandOff>,
    gpu: HashMap<SectorKey, SectorGpu>,
    detail: Option<TexRequest>,
    horizon: Option<u32>,
    horizon_warned: bool,
    water: super::water::WaterSurfaces,
    /// Sprite epoch and placement offset of the sprites swapped in last, per sector.
    sprite_offsets: HashMap<SectorKey, (u64, [f32; 2])>,
    stats: FarViewStats,
}

impl std::fmt::Debug for FarViewLayer {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("FarViewLayer")
            .field("stats", &self.stats)
            .finish_non_exhaustive()
    }
}

/// The near field of the walker (§2, §3.2, FD9) for `mode`: with
/// [`NearFieldMode::Original`] the landblocks within Chebyshev distance `radius` of `eye`'s
/// landblock; with [`NearFieldMode::Ring`] the landblocks of `keys` (the terrain layer's) in
/// `eye`'s region whose square lies within `extent` of `eye` horizontally (the test the
/// terrain layer draws by, `walker.md` §11.3). Either way only landblocks for which
/// `drawable` holds (the terrain layer can draw them) are kept; the far batches cover the
/// others.
#[must_use]
pub fn near_field_of(
    mode: NearFieldMode,
    keys: impl Iterator<Item = LandblockKey>,
    drawable: impl Fn(LandblockKey) -> bool,
    eye: &WorldPos,
    extent: f64,
    radius: u32,
) -> NearField {
    let set = match (mode, eye.landblock()) {
        (NearFieldMode::Original, Some(v)) => {
            let r = i32::try_from(radius.min(40)).unwrap_or(1);
            (-r..=r)
                .flat_map(|dx| (-r..=r).map(move |dy| (dx, dy)))
                .filter_map(|(dx, dy)| v.offset(dx, dy))
                .filter(|k| drawable(*k))
                .map(|k| (k.x(), k.y()))
                .collect()
        }
        (NearFieldMode::Original, None) => std::collections::BTreeSet::new(),
        (NearFieldMode::Ring, _) => keys
            .filter(|k| {
                k.region() == eye.region && k.distance_to(eye.xy()) <= extent && drawable(*k)
            })
            .map(|k| (k.x(), k.y()))
            .collect(),
    };
    NearField::Set(Arc::new(set))
}

/// A far batch as renderer input.
#[must_use]
pub fn far_terrain_data<'a>(label: &'a str, m: &'a FarMesh) -> FarTerrainData<'a> {
    FarTerrainData {
        label,
        positions: &m.positions,
        normals: &m.normals,
        uvs: &m.uvs,
        indices: &m.indices,
        stitch_start: m.stitch_start,
    }
}

/// Placed sprites as renderer input.
#[must_use]
pub fn distant_sprites(sprites: &[FarSprite]) -> Vec<DistantSprite> {
    sprites
        .iter()
        .map(|s| DistantSprite {
            position: s.position,
            size: s.size,
            base: s.base,
            u: s.u,
            v: s.v,
            distance: s.distance,
            mirror: s.mirror,
        })
        .collect()
}

/// The sprite camera values of §9.3 (FD6): the render target's height, the vertical field
/// of view and *M_d*.
#[must_use]
pub fn sprite_view(height: u32, fov_y: f32, distances: &DistanceSettings) -> SpriteView {
    #[allow(clippy::cast_precision_loss)]
    SpriteView {
        target_height: height.max(1) as f32,
        fov_y,
        object_scale: distances.object_multiplier(),
    }
}

/// The far-draw fade of the settings as renderer input.
#[must_use]
pub fn fade_params(settings: &FarViewSettings) -> FarFadeParams {
    let f = settings.fade();
    FarFadeParams {
        start: f.start,
        inv_range: f.inv_range,
    }
}

/// Whether the square of side `size` with south-west corner `origin` (render frame) lies
/// wholly beyond the end of the far-draw fade `fade` seen from `eye`: its nearest point is
/// farther than *s* + 1 / *r* horizontally, so every point of it is at least that far and
/// fully faded (*a* = 0). Never with no fade (*r* = 0) or non-finite values.
#[must_use]
pub fn beyond_fade(origin: [f32; 2], size: f32, eye: Vec3, fade: FarFadeParams) -> bool {
    if !(fade.inv_range.is_finite() && fade.inv_range > 0.0 && fade.start.is_finite()) {
        return false;
    }
    let end = fade.start + 1.0 / fade.inv_range;
    let nx = eye.x.clamp(origin[0], origin[0] + size);
    let ny = eye.y.clamp(origin[1], origin[1] + size);
    let d = (eye.x - nx).hypot(eye.y - ny);
    d.is_finite() && d > end
}

/// The detail repeats of a far-terrain template (§6.2: the defaults of texture transform
/// set 0, parameters 1001 and 1002); `None` when the record is missing, does not decode or
/// holds no positive finite pair.
fn template_repeats(ctx: &WorldContext, did: Did) -> Option<[f32; 2]> {
    let bytes = match ctx.source().read(did) {
        Ok(Some(b)) => b,
        Ok(None) => {
            tracing::warn!(did = %did, "far-terrain template missing");
            return None;
        }
        Err(e) => {
            tracing::warn!(did = %did, error = %e, "far-terrain template unreadable");
            return None;
        }
    };
    let rec = match ShaderRecord::parse_expecting(&bytes, did) {
        Ok(r) => r,
        Err(e) => {
            tracing::warn!(did = %did, error = %e, "far-terrain template does not decode");
            return None;
        }
    };
    template_scale(&rec, PARAM_SCALE_U0, PARAM_SCALE_V0)
}

/// Parameter of transform set 0's *u* scale (`material.md` §8.4).
pub const PARAM_SCALE_U0: u32 = 1001;
/// Parameter of transform set 0's *v* scale (`material.md` §8.4).
pub const PARAM_SCALE_V0: u32 = 1002;

/// The (`u`, `v`) default pair of a template's declarations; `None` unless both are
/// declared as positive finite floats.
#[must_use]
pub fn template_scale(rec: &ShaderRecord, u: u32, v: u32) -> Option<[f32; 2]> {
    let get = |id: u32| {
        rec.declaration(id)
            .and_then(|e| e.value.as_float())
            .filter(|x| x.is_finite() && *x > 0.0)
    };
    Some([get(u)?, get(v)?])
}

fn run(ctx: &WorldContext, job: Job) -> Done {
    match job {
        Job::Records => Done::Records(FarViewRecords::load(ctx)),
        Job::Template(d) => Done::Template(d, template_repeats(ctx, d)),
        Job::Sector(k) => Done::Sector(k, Box::new(read_sector(&**ctx.source(), k, ctx.schema()))),
        Job::Table(d) => Done::Table(d, read_imposter_table(&**ctx.source(), d)),
        Job::Build(j) => Done::Built(j.key, j.generation, Box::new(j.run())),
        Job::Sprites(j) => Done::Sprites(j.key, j.epoch, j.offset, j.run()),
    }
}

impl FarViewLayer {
    /// A layer over `ctx` with `config`.
    #[must_use]
    pub fn new(ctx: Arc<WorldContext>, config: &FarViewConfig) -> Self {
        let worker_ctx = Arc::clone(&ctx);
        let runner = JobRunner::new(
            "far-view",
            config.threads.max(1),
            JOB_CAPACITY,
            move |job: Job| run(&worker_ctx, job),
        )
        .inspect_err(|e| tracing::warn!(error = %e, "far-view workers did not start"))
        .ok();
        let pid = ctx.pids().landblock.entity_content_layers;
        Self {
            ctx,
            state: FarViewState::new(config.settings, pid),
            runner,
            backlog: VecDeque::new(),
            in_flight: 0,
            records: None,
            records_asked: false,
            detail_repeats: None,
            template_asked: false,
            template_done: false,
            near_mode: config.near_field,
            hand_off: Arc::new(FarHandOff::default()),
            gpu: HashMap::new(),
            detail: None,
            horizon: None,
            horizon_warned: false,
            water: super::water::WaterSurfaces::default(),
            sprite_offsets: HashMap::new(),
            stats: FarViewStats::default(),
        }
    }

    /// Counters of the last frame.
    #[must_use]
    pub fn stats(&self) -> FarViewStats {
        self.stats
    }

    /// Which landblocks stay at full detail.
    #[must_use]
    pub fn near_mode(&self) -> NearFieldMode {
        self.near_mode
    }

    /// The near radius *N* of the settings.
    #[must_use]
    pub fn near_radius(&self) -> u32 {
        self.state.settings().near_radius
    }

    /// The landscape extent *E* of the settings (§7.1).
    #[must_use]
    pub fn extent(&self) -> f32 {
        self.state.settings().extent()
    }

    /// The hand-off of the last update (module docs).
    #[must_use]
    pub fn hand_off(&self) -> Arc<FarHandOff> {
        Arc::clone(&self.hand_off)
    }

    /// Whether everything is in: the UNIQUEDB entries and the template read, every listed
    /// sector's record, batches and sprites built **and on the GPU**, their textures loaded
    /// (or failed), and nothing queued. False before the first update with a viewer.
    #[must_use]
    pub fn settled(&self, store: &ObjectLayer) -> bool {
        if self.records.is_none()
            || !self.template_done
            || self.state.viewer().is_none()
            || !self.state.settled()
            || !self.backlog.is_empty()
            || self.in_flight > 0
        {
            return false;
        }
        let pending = |t: TexRequest| matches!(store.lookup(t), TexLookup::Pending);
        if self.detail.is_some_and(pending) {
            return false;
        }
        self.state.listed().iter().all(|k| {
            let Some(slot) = self.state.slot(*k) else {
                return true;
            };
            if slot.ready().is_none() {
                // Missing or failed records are settled (the state checked loading ones).
                return true;
            }
            let Some(g) = self.gpu.get(k) else {
                return false;
            };
            let meshes_up = slot.build_failed == Some(slot.generation)
                || slot
                    .meshes
                    .as_ref()
                    .is_some_and(|m| m.generation == g.generation);
            let sprites_up = slot
                .sprites
                .as_ref()
                .is_none_or(|(e, _)| *e == g.sprite_epoch);
            meshes_up && sprites_up && g.textured && !g.textures.iter().copied().any(pending)
        })
    }

    /// A HUD line.
    #[must_use]
    pub fn hud_line(&self) -> String {
        let s = self.stats;
        format!(
            "far view: {} / {} sectors, {} batches over {} landblocks, {} sprites in {} batches, {} water, {} far entities, {} jobs",
            s.ready,
            s.listed,
            s.batches,
            s.covered,
            s.sprites,
            s.sprite_batches,
            s.water_quads,
            s.distant_live,
            s.pending
        )
    }

    fn release(gpu: SectorGpu, store: &mut dyn TextureStore) {
        if !gpu.textures.is_empty() {
            store.release(&gpu.textures);
        }
    }

    fn take_results(&mut self) {
        let Some(runner) = &self.runner else {
            return;
        };
        let mut done = Vec::new();
        while let Some(d) = runner.try_next() {
            done.push(d);
        }
        for d in done {
            self.in_flight = self.in_flight.saturating_sub(1);
            match d {
                Done::Records(r) => {
                    self.records = Some(r.unwrap_or_else(|e| {
                        tracing::warn!(error = %e, "far-view UNIQUEDB entries unavailable");
                        FarViewRecords::default()
                    }));
                }
                Done::Template(d, r) => {
                    if r.is_none() {
                        tracing::info!(did = %d, "far-terrain template gives no detail repeats; 4 x 4");
                    }
                    self.detail_repeats = r;
                    self.template_done = true;
                }
                Done::Sector(k, r) => self.state.sector_loaded(k, *r),
                Done::Table(d, r) => self.state.table_loaded(d, r),
                Done::Built(k, g, r) => {
                    self.state.built(k, g, *r);
                }
                Done::Sprites(k, e, offset, s) => {
                    if self.state.sprites_placed(k, e, s) {
                        self.sprite_offsets.insert(k, (e, offset));
                    }
                }
            }
        }
    }

    fn submit(&mut self) {
        let Some(runner) = &self.runner else {
            self.backlog.clear();
            return;
        };
        while let Some(job) = self.backlog.pop_front() {
            match runner.submit(job) {
                Ok(()) => self.in_flight += 1,
                Err(job) => {
                    self.backlog.push_front(job);
                    break;
                }
            }
        }
    }

    /// One frame: results in, the viewer's sector list and grid, jobs out, uploads.
    /// `focus` is the viewer's position, `near` the landblocks the walker draws at full
    /// detail, `view` the sprite camera values (§9.3).
    pub fn update(
        &mut self,
        renderer: &mut Renderer<'_>,
        store: &mut ObjectLayer,
        focus: Option<&WorldPos>,
        near: NearField,
        view: SpriteView,
    ) {
        self.take_results();
        if !self.records_asked {
            self.records_asked = true;
            self.backlog.push_back(Job::Records);
        }
        if let (Some(r), None) = (&self.records, &self.detail)
            && let Some(d) = r.detail_texture
        {
            let t = TexRequest {
                did: d.0,
                variant: VARIANT_OPAQUE,
            };
            store.acquire(&[t]);
            self.detail = Some(t);
        }
        if let (Some(r), None) = (&self.records, self.horizon)
            && let Some(d) = r.horizon_mesh
        {
            store.acquire_model_parts(&[d.0]);
            self.horizon = Some(d.0);
        }
        // §6.1: the active template's defaults. Only the plain template is drawn (the
        // normal-mapped one is not implemented, implementer note in `far-view.md` §12).
        if let Some(r) = &self.records
            && !self.template_asked
        {
            self.template_asked = true;
            match r.plain_template {
                Some(d) => self.backlog.push_back(Job::Template(d)),
                None => {
                    tracing::info!(
                        "no plain far-terrain template in UNIQUEDB; 4 x 4 detail repeats"
                    );
                    self.template_done = true;
                }
            }
        }
        let viewer = focus.and_then(WorldPos::landblock).map(|landblock| Viewer {
            landblock,
            instance: 0,
        });
        let work = self.state.update(viewer, near, view);
        if work.discarded {
            for (_, g) in self.gpu.drain() {
                Self::release(g, store);
            }
            self.sprite_offsets.clear();
        }
        for k in &work.freed {
            if let Some(g) = self.gpu.remove(k) {
                Self::release(g, store);
            }
            self.sprite_offsets.remove(k);
        }
        self.backlog
            .extend(work.load_sectors.iter().map(|k| Job::Sector(*k)));
        self.backlog
            .extend(work.load_tables.iter().map(|d| Job::Table(*d)));
        self.backlog.extend(work.build.into_iter().map(Job::Build));
        self.backlog
            .extend(work.sprites.into_iter().map(Job::Sprites));
        self.submit();
        self.upload(renderer, store);
        self.hand_off = Arc::new(self.compute_hand_off(store));
        self.stats.covered = self.hand_off.covered.len();
        // Far entities (§10): walk the distant set (drawing them is not wired yet).
        if let Some(p) = focus {
            let active = self.ctx.options().active_content_layers.clone();
            let _ = self.state.distant_mut().walk(
                p,
                0,
                1.0,
                |_| false,
                |e| {
                    tessera_physics::geometry::content_layers_pass(
                        e.content_layers.as_deref(),
                        &active,
                        0,
                    )
                },
            );
        }
        let listed = self.state.listed();
        self.stats.listed = listed.len();
        self.stats.ready = listed
            .iter()
            .filter(|k| self.state.slot(**k).and_then(|s| s.ready()).is_some())
            .count();
        self.stats.distant_live = self.state.distant().live().count();
        self.stats.pending = self.backlog.len() + self.in_flight;
    }

    fn upload(&mut self, renderer: &mut Renderer<'_>, store: &mut ObjectLayer) {
        let mut budget = MAX_UPLOADS_PER_FRAME;
        let keys: Vec<SectorKey> = self.state.listed().to_vec();
        for k in keys {
            let Some(slot) = self.state.slot(k) else {
                continue;
            };
            let Some(rec) = slot.ready().cloned() else {
                continue;
            };
            let g = self.gpu.entry(k).or_default();
            if !g.textured {
                g.textured = true;
                let mut reqs = Vec::with_capacity(2);
                if rec.colour_list.0 != 0 {
                    reqs.push(TexRequest {
                        did: rec.colour_list.0,
                        variant: VARIANT_OPAQUE,
                    });
                }
                if rec.ceiling_colour_list.0 != 0 {
                    reqs.push(TexRequest {
                        did: rec.ceiling_colour_list.0,
                        variant: VARIANT_OPAQUE,
                    });
                }
                store.acquire(&reqs);
                g.textures.extend(reqs);
            }
            if !g.atlased
                && let Some(t) = self.state.table(rec.imposter_table)
            {
                g.atlased = true;
                let reqs: Vec<TexRequest> = [t.colour_atlas, t.second_atlas]
                    .iter()
                    .filter(|d| d.0 != 0)
                    .map(|d| TexRequest {
                        did: d.0,
                        variant: VARIANT_OWN_ALPHA,
                    })
                    .collect();
                store.acquire(&reqs);
                g.textures.extend(reqs);
            }
            if budget == 0 {
                continue;
            }
            if let Some(m) = &slot.meshes
                && m.generation != g.generation
            {
                budget -= 1;
                let label = format!("far terrain {:#010x}", rec.did.0);
                let up = |mesh: &FarMesh, r: &mut Renderer<'_>| {
                    if mesh.is_empty() {
                        return None;
                    }
                    r.upload_far_terrain(&far_terrain_data(&label, mesh))
                        .inspect_err(|e| tracing::warn!(error = %e, "far terrain upload failed"))
                        .ok()
                };
                g.terrain = up(&m.terrain, renderer);
                g.ceiling = m.ceiling.as_ref().and_then(|c| up(c, renderer));
                g.generation = m.generation;
                g.covers = if g.terrain.is_some() {
                    m.far_landblocks().collect()
                } else {
                    Vec::new()
                };
            }
            if let Some((epoch, sprites)) = &slot.sprites
                && *epoch != g.sprite_epoch
                && let Some((e, offset)) = self.sprite_offsets.get(&k)
                && e == epoch
                && budget > 0
            {
                budget -= 1;
                g.sprite_epoch = *epoch;
                g.sprites = renderer
                    .upload_sprite_batch(
                        &format!("sprites {:#010x}", rec.did.0),
                        &distant_sprites(sprites),
                    )
                    .inspect_err(|e| tracing::warn!(error = %e, "sprite upload failed"))
                    .ok()
                    .map(|b| (b, *offset));
            }
        }
    }

    /// Whether a sector's terrain batch is drawn: its colour list is on the GPU or failed
    /// (a white batch while it loads would flash).
    fn colour_settled(store: &ObjectLayer, rec: &SectorRecord) -> bool {
        rec.colour_list.0 == 0
            || !matches!(
                store.lookup(TexRequest {
                    did: rec.colour_list.0,
                    variant: VARIANT_OPAQUE,
                }),
                TexLookup::Pending
            )
    }

    /// The hand-off of this frame: the landblocks of the far terrain batches that
    /// [`FarViewLayer::draw`] draws, and those the current grid puts in the far field.
    fn compute_hand_off(&self, store: &ObjectLayer) -> FarHandOff {
        let mut out = FarHandOff::default();
        let region = self.state.viewer().map(|v| v.landblock.region());
        for k in self.state.listed() {
            let (Some(slot), Some(g)) = (self.state.slot(*k), self.gpu.get(k)) else {
                continue;
            };
            let Some(rec) = slot.ready() else {
                continue;
            };
            if g.terrain.is_none() || !Self::colour_settled(store, rec) {
                continue;
            }
            out.covered.extend(
                g.covers
                    .iter()
                    .filter_map(|&(x, y)| LandblockKey::new(k.region(), x, y).ok()),
            );
        }
        out.far.clone_from(&out.covered);
        if let (Some(grid), Some(region)) = (self.state.grid(), region) {
            out.far.extend(grid.cells().filter_map(|((x, y), c)| {
                matches!(c, Cell::Far(_))
                    .then(|| LandblockKey::new(region, x, y).ok())
                    .flatten()
            }));
        }
        out
    }

    /// Queues the far terrain, the sector water outside the near field, the sprites and
    /// the horizon fill into `pass`. `eye` is the camera in the render frame, `distance_scale`
    /// *c* of §9.3 (crowd factor × *M_d*).
    #[allow(clippy::too_many_arguments)]
    pub fn draw(
        &mut self,
        pass: &mut Pass,
        anchor: &Anchor,
        eye: Vec3,
        distance_scale: f32,
        resolver: &mut Resolver,
        store: &mut ObjectLayer,
        water: bool,
        time: f64,
    ) {
        let settings = *self.state.settings();
        let fade = fade_params(&settings);
        let lookup = |store: &ObjectLayer, t: Option<TexRequest>| -> Option<TextureId> {
            match t.map(|t| store.lookup(t)) {
                Some(TexLookup::Ready(id)) => Some(id),
                _ => None,
            }
        };
        let detail = lookup(store, self.detail);
        let mut batches = 0usize;
        let mut sprite_draws: Vec<(f32, SpriteDraw)> = Vec::new();
        let mut sprites = 0usize;
        let mut quads: HashMap<Did, Vec<WaterQuad>> = HashMap::new();
        let viewer = self.state.viewer();
        let hand_off = Arc::clone(&self.hand_off);
        let ctx = Arc::clone(&self.ctx);
        let tables = ctx.systems().water.as_ref().map(|w| w.tables());
        for k in self.state.listed() {
            let (Some(slot), Some(g)) = (self.state.slot(*k), self.gpu.get(k)) else {
                continue;
            };
            let Some(rec) = slot.ready() else {
                continue;
            };
            let Some(origin) = k.corner_landblock().and_then(|c| anchor.offset_of(c)) else {
                continue;
            };
            let tf = Mat4::from_translation(origin);
            let colour = lookup(
                store,
                Some(TexRequest {
                    did: rec.colour_list.0,
                    variant: VARIANT_OPAQUE,
                }),
            );
            let near_view = viewer
                .and_then(|v| anchor.offset_of(v.landblock))
                .map(|o| [o.x, o.y]);
            let repeats = self.detail_repeats.unwrap_or([4.0, 4.0]);
            if let Some(m) = &g.terrain
                && Self::colour_settled(store, rec)
            {
                let mut d = FarTerrainDraw::new(m.clone(), tf);
                d.colour = colour;
                d.detail = detail;
                d.detail_repeats = repeats;
                d.fade = fade;
                d.near_view_origin = near_view;
                pass.far_terrain(d);
                batches += 1;
            }
            if let Some(m) = &g.ceiling {
                let mut d = FarTerrainDraw::new(m.clone(), tf);
                d.colour = lookup(
                    store,
                    Some(TexRequest {
                        did: rec.ceiling_colour_list.0,
                        variant: VARIANT_OPAQUE,
                    }),
                );
                d.detail = detail;
                d.detail_repeats = repeats;
                d.fade = fade;
                pass.far_terrain(d);
                batches += 1;
            }
            if let (Some((b, offset)), Some(t)) = (&g.sprites, self.state.table(rec.imposter_table))
            {
                let atlas = lookup(
                    store,
                    Some(TexRequest {
                        did: t.colour_atlas.0,
                        variant: VARIANT_OWN_ALPHA,
                    }),
                );
                let normals = (t.second_atlas.0 != 0)
                    .then(|| {
                        lookup(
                            store,
                            Some(TexRequest {
                                did: t.second_atlas.0,
                                variant: VARIANT_OWN_ALPHA,
                            }),
                        )
                    })
                    .flatten();
                if atlas.is_some() && !b.is_empty() {
                    // Sprite positions are relative to the viewer landblock they were placed
                    // for: the sector corner minus the placement offset.
                    let base = origin - Vec3::new(offset[0], offset[1], 0.0);
                    let mut d = SpriteDraw::new(b.clone(), Mat4::from_translation(base), atlas);
                    d.normal_atlas = normals;
                    d.distance_scale = distance_scale;
                    d.fade = fade;
                    let centre = origin + Vec3::new(240.0, 240.0, 0.0);
                    let dist = (centre - eye).truncate().length();
                    sprites += b.len() as usize;
                    sprite_draws.push((dist, d));
                }
            }
            // Sector water of the landblocks the far batches cover (§8; the walker's water
            // layer draws the others, skipping these: the same hand-off as the terrain).
            if water && let Some(tables) = &tables {
                for (x, y, pair) in sector_water(rec) {
                    let (cx, cy) = k.corner();
                    let covered = LandblockKey::new(k.region(), cx + x, cy + y)
                        .is_ok_and(|lb| hand_off.is_covered(lb));
                    if !covered {
                        continue;
                    }
                    let Some(surface) = tables
                        .water_table(k.region())
                        .and_then(|t| t.select(u32::from(pair.water_type)))
                        .and_then(|t| t.wet.as_ref())
                        .map(|w| w.surface)
                    else {
                        continue;
                    };
                    let size = LANDBLOCK_SIZE as f32;
                    let o = origin + Vec3::new(f32::from(x) * size, f32::from(y) * size, 0.0);
                    // Limited like the far terrain (§8 OPEN, Tessera): a quad wholly beyond
                    // the end of the far-draw fade is not drawn.
                    if beyond_fade([o.x, o.y], size, eye, fade) {
                        continue;
                    }
                    quads.entry(surface).or_default().push(
                        // §8.1: the landblock's place in its 3 × 3 sector.
                        WaterQuad::landblock([o.x, o.y], pair.height)
                            .with_sector_uv(u32::from(x), u32::from(y)),
                    );
                }
            }
        }
        // FD4: far to near.
        sprite_draws.sort_by(|a, b| b.0.total_cmp(&a.0));
        let sprite_batches = sprite_draws.len();
        for (_, d) in sprite_draws {
            pass.distant_sprites(d);
        }
        let mut water_quads = 0usize;
        let region_offset = {
            let o = anchor.origin();
            [-(o[0] as f32), -(o[1] as f32)]
        };
        self.water.begin(time);
        for (surface, list) in quads {
            let super::water::SurfaceState::Ready {
                params,
                animated_scales,
                normal_map,
                textures,
                ..
            } = self.water.surface(surface, resolver, store)
            else {
                continue;
            };
            water_quads += list.len();
            let mut d = WaterDraw::new(list, normal_map, params);
            d.block_offset =
                super::liquid::block_offset(animated_scales, region_offset, [eye.x, eye.y]);
            d.textures = textures;
            // Faded like the far terrain (§7.2, FD1; §8 OPEN, Tessera choice).
            d.fade = fade;
            pass.water(d);
        }
        self.draw_horizon(pass, eye, store, &settings);
        self.stats.batches = batches;
        self.stats.sprite_batches = sprite_batches;
        self.stats.sprites = sprites;
        self.stats.water_quads = water_quads;
    }

    /// The horizon fill (§7.3), drawn only when its horizontal radius reaches 1.25 × *E*.
    fn draw_horizon(
        &mut self,
        pass: &mut Pass,
        eye: Vec3,
        store: &ObjectLayer,
        settings: &FarViewSettings,
    ) {
        let Some(part) = self.horizon else {
            return;
        };
        let Some(model) = store.model(part, 0) else {
            return;
        };
        let aabb = model.info().aabb;
        let bounds = MeshBounds {
            min: aabb.min.to_array(),
            max: aabb.max.to_array(),
        };
        let extent = settings.extent();
        let r = bounds.max[0].max(bounds.max[1]);
        if r < horizon::EXTENT_FACTOR * extent {
            if !self.horizon_warned {
                self.horizon_warned = true;
                tracing::info!(
                    radius = r,
                    extent,
                    "horizon fill smaller than 1.25 E; not drawn (its depth state is OPEN)"
                );
            }
            return;
        }
        let floor = horizon::floor(self.state.ring_floor(), self.state.near_floor());
        let Some(floor) = floor else {
            return;
        };
        let Some(p) = horizon::placement(eye.z, floor, extent, &bounds) else {
            return;
        };
        let tf = Mat4::from_translation(Vec3::new(eye.x, eye.y, eye.z - p.drop))
            * Mat4::from_scale(Vec3::new(1.0, 1.0, p.scale_z));
        let mut batch = InstanceBatch::new(model.clone(), vec![Instance::new(tf)]);
        batch.cast_shadow = false;
        pass.instanced(batch);
    }

    /// Frees every GPU object and texture reference (shutdown, region change of the walker,
    /// and before the object layer is cleared).
    pub fn clear(&mut self, store: &mut ObjectLayer) {
        for (_, g) in self.gpu.drain() {
            Self::release(g, store);
        }
        if let Some(t) = self.detail.take() {
            store.release(&[t]);
        }
        if let Some(p) = self.horizon.take() {
            store.release_model_parts(&[p]);
        }
        self.water.release(store);
        self.sprite_offsets.clear();
        self.hand_off = Arc::new(FarHandOff::default());
        self.state.reset();
    }

    /// Stops the workers.
    pub fn shutdown(&mut self) {
        if let Some(r) = &mut self.runner {
            r.shutdown();
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use tessera_material::FloatCurve;
    use tessera_material::records::{ParamEntry, ParamValue};

    fn lb(x: u8, y: u8) -> LandblockKey {
        LandblockKey::new(1, x, y).unwrap()
    }

    fn eye_in(x: u8, y: u8, fx: f64, fy: f64) -> WorldPos {
        WorldPos::new(
            1,
            (f64::from(x) + fx) * LANDBLOCK_SIZE,
            (f64::from(y) + fy) * LANDBLOCK_SIZE,
            50.0,
        )
    }

    fn set(n: &NearField) -> Vec<(u8, u8)> {
        match n {
            NearField::Set(s) => s.iter().copied().collect(),
            NearField::Radius(_) => panic!("expected a set"),
        }
    }

    #[test]
    fn far_water_ends_with_the_far_terrain_fade() {
        // FD1 at High: s = E − 160, r = 1/160, so nothing is visible beyond E = 2,080.
        let fade = FarFadeParams {
            start: 1_920.0,
            inv_range: 1.0 / 160.0,
        };
        let eye = Vec3::new(0.0, 0.0, 500.0);
        // Nearest point 1,920 or 2,080 away: inside the fade band, drawn.
        assert!(!beyond_fade([1_920.0, -80.0], 160.0, eye, fade));
        assert!(!beyond_fade([2_080.0, 0.0], 160.0, eye, fade));
        // Nearest point just past E: wholly faded, not drawn (whatever the eye height).
        assert!(beyond_fade([2_081.0, 0.0], 160.0, eye, fade));
        assert!(beyond_fade([1_500.0, 1_500.0], 160.0, eye, fade));
        // A quad around the eye is never beyond.
        assert!(!beyond_fade([-80.0, -80.0], 160.0, eye, fade));
        // No fade, or hostile values: never limited.
        assert!(!beyond_fade(
            [1.0e6, 0.0],
            160.0,
            eye,
            FarFadeParams::default()
        ));
        let nan = FarFadeParams {
            start: f32::NAN,
            inv_range: 1.0,
        };
        assert!(!beyond_fade([1.0e6, 0.0], 160.0, eye, nan));
        // The settings' fade is the one the far terrain uses.
        let settings = FarViewSettings::default();
        let f = fade_params(&settings);
        assert_eq!(
            (f.start, f.inv_range),
            (settings.fade().start, settings.fade().inv_range)
        );
    }

    #[test]
    fn original_near_field_is_the_drawable_3_by_3() {
        let eye = eye_in(100, 50, 0.9, 0.1);
        assert_eq!(eye.landblock(), Some(lb(100, 50)));
        // Every landblock drawable: exactly the 3 × 3 around the viewer's, whatever the
        // eye's place in it and whatever other keys the terrain holds.
        let keys = [lb(97, 50), lb(102, 52), lb(100, 50)];
        let n = near_field_of(
            NearFieldMode::Original,
            keys.into_iter(),
            |_| true,
            &eye,
            NEAR_FIELD_RING,
            1,
        );
        let s = set(&n);
        assert_eq!(s.len(), 9);
        for x in 99..=101 {
            for y in 49..=51 {
                assert!(s.contains(&(x, y)), "{x},{y}");
            }
        }
        // Landblocks the terrain cannot draw yet stay with the far view.
        let n = near_field_of(
            NearFieldMode::Original,
            std::iter::empty(),
            |k| k != lb(101, 51),
            &eye,
            NEAR_FIELD_RING,
            1,
        );
        assert_eq!(set(&n).len(), 8);
        assert!(!set(&n).contains(&(101, 51)));
        // At the map edge the square is clipped.
        let n = near_field_of(
            NearFieldMode::Original,
            std::iter::empty(),
            |_| true,
            &eye_in(0, 0, 0.5, 0.5),
            NEAR_FIELD_RING,
            1,
        );
        assert_eq!(set(&n), vec![(0, 0), (0, 1), (1, 0), (1, 1)]);
    }

    #[test]
    fn ring_near_field_follows_the_terrain_keys() {
        let eye = eye_in(100, 50, 0.5, 0.5);
        let keys = [lb(100, 50), lb(103, 50), lb(105, 50), lb(100, 47)];
        let other_region = LandblockKey::new(2, 100, 50).unwrap();
        let n = near_field_of(
            NearFieldMode::Ring,
            keys.into_iter().chain([other_region]),
            |k| k != lb(103, 50),
            &eye,
            3.0 * LANDBLOCK_SIZE,
            1,
        );
        // (103, 50) is not drawable, (105, 50) lies beyond the ring, region 2 is skipped.
        assert_eq!(set(&n), vec![(100, 47), (100, 50)]);
    }

    #[test]
    fn near_field_ring_holds_the_3_by_3_from_any_eye() {
        // The farthest square of the 3 × 3 lies 160 √2 from a corner of the viewer's
        // landblock; the ring must cover it.
        let eye = eye_in(100, 50, 0.0, 0.0);
        for x in 99..=101 {
            for y in 49..=51 {
                assert!(lb(x, y).distance_to(eye.xy()) <= NEAR_FIELD_RING);
            }
        }
    }

    #[test]
    fn hand_off_borders() {
        let mut h = FarHandOff::default();
        h.far.insert(lb(10, 10));
        h.covered.insert(lb(10, 10));
        assert!(h.is_far(lb(10, 10)) && h.is_covered(lb(10, 10)));
        // Edge neighbours border the far field; diagonal ones and the far block do not.
        assert!(h.borders_far(lb(9, 10)));
        assert!(h.borders_far(lb(10, 11)));
        assert!(!h.borders_far(lb(9, 9)));
        assert!(!h.borders_far(lb(10, 10)));
        // A block about to be far (batch still building) counts for the borders, not for
        // the cover.
        h.far.insert(lb(20, 20));
        assert!(h.borders_far(lb(21, 20)));
        assert!(!h.is_covered(lb(20, 20)));
    }

    fn declaration(id: u32, v: f32) -> ParamEntry {
        ParamEntry {
            id,
            group: 1000,
            value: ParamValue::Float(FloatCurve::constant(v)),
            flag: 0,
            bindings: Vec::new(),
        }
    }

    #[test]
    fn template_scale_reads_transform_set_0() {
        let mut rec = ShaderRecord {
            did: Did(0x2B00_0001),
            reserved: 0,
            params: vec![
                declaration(PARAM_SCALE_U0, 4.0),
                declaration(PARAM_SCALE_V0, 4.0),
            ],
            permutations: Vec::new(),
        };
        assert_eq!(
            template_scale(&rec, PARAM_SCALE_U0, PARAM_SCALE_V0),
            Some([4.0, 4.0])
        );
        rec.params[1] = declaration(PARAM_SCALE_V0, f32::NAN);
        assert_eq!(template_scale(&rec, PARAM_SCALE_U0, PARAM_SCALE_V0), None);
        rec.params[1] = declaration(PARAM_SCALE_V0, -1.0);
        assert_eq!(template_scale(&rec, PARAM_SCALE_U0, PARAM_SCALE_V0), None);
        rec.params.truncate(1);
        assert_eq!(template_scale(&rec, PARAM_SCALE_U0, PARAM_SCALE_V0), None);
    }

    #[test]
    fn renderer_inputs() {
        let d = DistanceSettings::default();
        let v = sprite_view(0, 0.8, &d);
        assert_eq!(v.target_height, 1.0);
        assert_eq!(v.fov_y, 0.8);
        assert_eq!(v.object_scale, d.object_multiplier());
        let s = FarViewSettings::default();
        let f = fade_params(&s);
        // FD1 at the built-in level: s = E − 160, r = 1/160.
        assert_eq!(f.start, s.extent() - 160.0);
        assert_eq!(f.inv_range, 1.0 / 160.0);
        let sprite = FarSprite {
            position: [1.0, 2.0, 3.0],
            size: 4.0,
            base: 0.5,
            u: [0.1, 0.2],
            v: [0.3, 0.4],
            distance: 140.0,
            mirror: true,
        };
        let out = distant_sprites(&[sprite]);
        assert_eq!(out.len(), 1);
        let o = &out[0];
        assert_eq!(
            (o.position, o.size, o.base, o.u, o.v, o.distance, o.mirror),
            (
                sprite.position,
                sprite.size,
                sprite.base,
                sprite.u,
                sprite.v,
                sprite.distance,
                sprite.mirror
            )
        );
    }
}
