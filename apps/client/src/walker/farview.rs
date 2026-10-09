// SPDX-License-Identifier: GPL-3.0-or-later
//! The walker's far view (`docs/specs/far-view.md`, `--far-view on`): distant-landscape
//! sectors around the viewer drawn beyond the walker's full-detail terrain, with their
//! colour lists and the detail texture, the sectors' landscape water outside the
//! full-detail landblocks, and the distant sprites from the imposter atlases.
//!
//! - **Near field** (Tessera Decision FD9): the walker keeps its own full-detail rings, so
//!   the landblocks its terrain layer holds are the near field ([`near_field_of`]); the far
//!   batches skip them and stitch to their edges. `--view-blocks 3` gives the original's
//!   look (a 3 × 3 near field plus a ring of hysteresis).
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

use std::collections::{HashMap, VecDeque};
use std::sync::Arc;

use tessera_formats::{Did, ImposterTable, SectorRecord};
use tessera_render::glam::{Mat4, Vec3};
use tessera_render::{
    DistantSprite, FarFadeParams, FarTerrainData, FarTerrainDraw, FarTerrainMesh, Instance,
    InstanceBatch, Pass, Renderer, SpriteBatch, SpriteDraw, TextureId, WaterDraw, WaterParams,
    WaterQuad,
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

/// Most queued jobs.
const JOB_CAPACITY: usize = 256;

/// The far view's switches.
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct FarViewConfig {
    /// Draw the far view (`--far-view on`).
    pub enabled: bool,
    /// Radii, template, sprites.
    pub settings: FarViewSettings,
    /// Worker threads.
    pub threads: usize,
}

impl Default for FarViewConfig {
    fn default() -> Self {
        Self {
            enabled: false,
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
    /// Live far entities of the distant set (not drawn yet).
    pub distant_live: usize,
    /// Jobs queued or running.
    pub pending: usize,
}

enum Job {
    Records,
    Sector(SectorKey),
    Table(Did),
    Build(BuildJob),
    Sprites(SpriteJob),
}

enum Done {
    Records(Result<FarViewRecords, WorldError>),
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
    sprite_epoch: u64,
    /// Sprites with the sector offset they were placed for (sector corner relative to the
    /// viewer landblock's).
    sprites: Option<(SpriteBatch, [f32; 2])>,
    textures: Vec<TexRequest>,
    textured: bool,
    atlased: bool,
}

#[derive(Debug, Clone)]
struct WaterSurface {
    params: Option<WaterParams>,
    normal: Option<TexRequest>,
    resolved: bool,
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
    gpu: HashMap<SectorKey, SectorGpu>,
    detail: Option<TexRequest>,
    horizon: Option<u32>,
    horizon_warned: bool,
    water: HashMap<Did, WaterSurface>,
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

/// The near field of the walker (FD9): the landblocks of `keys` (the terrain layer's) in
/// `eye`'s region whose square lies within `extent` of `eye` horizontally, the test the
/// terrain layer draws by (`walker.md` §11.3), so no landblock is drawn by both and none by
/// neither.
#[must_use]
pub fn near_field_of(
    keys: impl Iterator<Item = LandblockKey>,
    eye: &WorldPos,
    extent: f64,
) -> NearField {
    NearField::Set(Arc::new(
        keys.filter(|k| k.region() == eye.region && k.distance_to(eye.xy()) <= extent)
            .map(|k| (k.x(), k.y()))
            .collect(),
    ))
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

fn run(ctx: &WorldContext, job: Job) -> Done {
    match job {
        Job::Records => Done::Records(FarViewRecords::load(ctx)),
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
            gpu: HashMap::new(),
            detail: None,
            horizon: None,
            horizon_warned: false,
            water: HashMap::new(),
            sprite_offsets: HashMap::new(),
            stats: FarViewStats::default(),
        }
    }

    /// Counters of the last frame.
    #[must_use]
    pub fn stats(&self) -> FarViewStats {
        self.stats
    }

    /// Whether every listed sector is in and nothing is queued.
    #[must_use]
    pub fn settled(&self) -> bool {
        self.state.settled() && self.backlog.is_empty() && self.in_flight == 0
    }

    /// A HUD line.
    #[must_use]
    pub fn hud_line(&self) -> String {
        let s = self.stats;
        format!(
            "far view: {} / {} sectors, {} batches, {} sprites in {} batches, {} water, {} far entities, {} jobs",
            s.ready,
            s.listed,
            s.batches,
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
                let mut reqs = vec![TexRequest {
                    did: rec.colour_list.0,
                    variant: VARIANT_OPAQUE,
                }];
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
        let grid = self.state.grid();
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
            if let Some(m) = &g.terrain {
                let mut d = FarTerrainDraw::new(m.clone(), tf);
                d.colour = colour;
                d.detail = detail;
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
            // Sector water outside the near field (§8; the walker's water layer draws the
            // near field).
            if water && let Some(tables) = &tables {
                for (x, y, pair) in sector_water(rec) {
                    let (cx, cy) = k.corner();
                    let (gx, gy) = (cx + x, cy + y);
                    if !matches!(
                        grid.map(|g| g.cell(i32::from(gx), i32::from(gy))),
                        Some(Cell::Far(_))
                    ) {
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
                    quads
                        .entry(surface)
                        .or_default()
                        .push(WaterQuad::landblock([o.x, o.y], pair.height));
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
        let block_offset = {
            let o = anchor.origin();
            [-(o[0] as f32), -(o[1] as f32)]
        };
        for (surface, list) in quads {
            let Some((params, normal)) = self.water_surface(surface, resolver, store) else {
                continue;
            };
            water_quads += list.len();
            let mut d = WaterDraw::new(list, normal, params);
            d.block_offset = block_offset;
            pass.water(d);
        }
        self.draw_horizon(pass, eye, store, &settings);
        self.stats.batches = batches;
        self.stats.sprite_batches = sprite_batches;
        self.stats.sprites = sprites;
        self.stats.water_quads = water_quads;
    }

    fn water_surface(
        &mut self,
        surface: Did,
        resolver: &mut Resolver,
        store: &mut ObjectLayer,
    ) -> Option<(WaterParams, Option<TextureId>)> {
        let e = self.water.entry(surface).or_insert(WaterSurface {
            params: None,
            normal: None,
            resolved: false,
        });
        if !e.resolved {
            let m = resolver.water(surface)?;
            e.resolved = true;
            e.params = m.as_ref().map(super::water::render_params);
            e.normal = m.as_ref().and_then(|m| m.normal_map).map(|d| TexRequest {
                did: d.0,
                variant: VARIANT_OWN_ALPHA,
            });
            if let Some(t) = e.normal {
                store.acquire(&[t]);
            }
        }
        let normal = match e.normal.map(|t| store.lookup(t)) {
            Some(TexLookup::Ready(id)) => Some(id),
            _ => None,
        };
        Some((e.params.unwrap_or_default(), normal))
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
        for (_, e) in self.water.drain() {
            if let Some(t) = e.normal {
                store.release(&[t]);
            }
        }
        self.sprite_offsets.clear();
        self.state.reset();
    }

    /// Stops the workers.
    pub fn shutdown(&mut self) {
        if let Some(r) = &mut self.runner {
            r.shutdown();
        }
    }
}
