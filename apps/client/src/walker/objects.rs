// SPDX-License-Identifier: GPL-3.0-or-later
//! Static objects of the resident landblocks: part meshes (0x06) with their materials and
//! textures, loaded on the asset workers and drawn per instance.
//!
//! - **Parts** are requested per unique mesh DID when a landblock comes within the object
//!   range, and reference-counted by the landblocks that use them. A worker reads the
//!   record, decodes the mesh (`mesh.md`), flattens its display level, resolves every
//!   submesh's surface to a [`MaterialPlan`] (`material.md`) and prepares the GPU streams;
//!   the main thread only uploads, within the frame's [`UploadBudget`].
//! - **Textures** are requested when a part is uploaded, keyed by
//!   [`TexRequest`] (base texture and alpha source), decoded and composed on a worker,
//!   capped in size, and reference-counted by uploaded parts.
//! - GPU memory has a budget: unreferenced parts and textures are kept for reuse until
//!   the budget is exceeded or they have been unused for a while, then freed least
//!   recently used first. Part meshes share the renderer's mesh arena with the terrain, so
//!   they never fill it beyond the terrain's share
//!   ([`super::terrain_arena_reserve`]) below its cap; a part that does
//!   not fit waits, and the object range shrinks until what is in range fits.
//! - Drawing culls per landblock (a box around the bounding spheres of its uploaded
//!   instances, [`instance_bounds`]) and per instance (frustum, distance, projected size)
//!   and caps the draws per frame, nearest landblocks first.

use std::collections::{HashMap, HashSet};
use std::sync::{Arc, Mutex, PoisonError};
use std::time::{Duration, Instant};

use tessera_assets::{
    AssetError, AssetKey, AssetState, AssetStore, Decoder, Handle, JobContext, Outcome, Priority,
    RecordSource, StoreConfig, WorkerPool,
};
use tessera_dat::ArchiveSet;
use tessera_formats::{Did, RgbaImage};
use tessera_material::{LibraryConfig, MaterialLibrary};
use tessera_mesh::{DecodeOptions, GeometryLimits, Mesh, StampedSource};
use tessera_render::glam::{Mat4, Vec3};
use tessera_render::{
    AlphaMode, Material, MeshConfig, MeshDraw, MeshId, Pass, PreparedMesh, Renderer, TextureId,
    TextureUpload, Wrap,
};
use tessera_world::{LANDBLOCK_SIZE, LandblockKey};

use super::UploadBudget;
use super::figures::figure_tag;
use super::geometry::{
    AlphaFrom, AlphaKind, MaterialPlan, TexRequest, VARIANT_ALPHA_ONLY, VARIANT_DYE_MASK,
    VARIANT_OPAQUE, VARIANT_OWN_ALPHA, compose_texture, display_level, downscale_to, dye_mask,
    level_mesh_data, plan_material,
};
use super::terrain::Instance;
use super::view::{Anchor, Frustum};

/// Frames an unreferenced GPU resource is kept before it may be freed even under budget.
const IDLE_KEEP: Duration = Duration::from_secs(30);
/// Retry delay after a full queue.
const RETRY_AFTER: Duration = Duration::from_millis(250);
/// Retry delay of a part whose upload found the mesh arena full.
const ARENA_RETRY: Duration = Duration::from_secs(2);
/// How long a full mesh arena keeps the object range shrinking.
const ARENA_PRESSURE: Duration = Duration::from_secs(1);

/// Errors of the part and texture decoders.
#[derive(Debug, thiserror::Error)]
pub enum ObjectError {
    /// The record is shorter than the directory stamp appended by the source.
    #[error("record too short for its stamp")]
    NoStamp,
    /// The mesh record did not decode or flatten.
    #[error(transparent)]
    Mesh(#[from] tessera_mesh::MeshError),
    /// The mesh has no levels.
    #[error("mesh has no levels")]
    NoLevels,
    /// The renderer refused the streams.
    #[error(transparent)]
    Render(#[from] tessera_render::RenderError),
    /// A texture could not be decoded.
    #[error(transparent)]
    Material(#[from] tessera_material::MaterialError),
    /// Nothing to compose (inconsistent images).
    #[error("texture composition failed")]
    Compose,
    /// The job was cancelled.
    #[error("cancelled")]
    Cancelled,
}

/// A decoded part, ready for upload.
#[derive(Debug)]
pub struct PartAsset {
    mesh: Mutex<Option<PreparedMesh>>,
    gpu_bytes: u64,
    /// One plan per renderer submesh.
    pub plans: Vec<MaterialPlan>,
}

/// Decodes part meshes (module docs). Holds the material library shared with textures.
pub struct PartDecoder {
    materials: Arc<MaterialLibrary<Arc<ArchiveSet>>>,
    mesh_config: MeshConfig,
    limits: GeometryLimits,
}

impl std::fmt::Debug for PartDecoder {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("PartDecoder").finish_non_exhaustive()
    }
}

impl Decoder for PartDecoder {
    type Output = PartAsset;
    type Error = ObjectError;
    const NAME: &'static str = "walker-part";

    fn decode(
        &self,
        key: AssetKey,
        mut bytes: Vec<u8>,
        ctx: &JobContext<'_>,
    ) -> Result<PartAsset, ObjectError> {
        // `StampedSource` appends the directory stamp as a little-endian u32.
        let n = bytes.len().checked_sub(4).ok_or(ObjectError::NoStamp)?;
        let stamp = u32::from_le_bytes([bytes[n], bytes[n + 1], bytes[n + 2], bytes[n + 3]]);
        bytes.truncate(n);
        let did = Did(key.did);
        let mesh = Mesh::decode_expecting(
            &bytes,
            did,
            u16::try_from(stamp).ok(),
            &DecodeOptions::render_only(),
        )?;
        drop(bytes);
        let level = display_level(&mesh).ok_or(ObjectError::NoLevels)?;
        let geo = mesh.level_geometry(level, &self.limits)?;
        drop(mesh);
        if ctx.is_cancelled() {
            return Err(ObjectError::Cancelled);
        }
        let data = level_mesh_data(&geo, format!("part {:#010x}", key.did));
        let plans = geo
            .parts
            .iter()
            .map(|p| {
                p.surface.map_or(MaterialPlan::FALLBACK, |s| {
                    match self.materials.resolve_surface(s) {
                        Ok(m) => plan_material(&m),
                        Err(e) => {
                            tracing::debug!(surface = %format_args!("{:#010x}", s.0), "material unresolved: {e}");
                            MaterialPlan::FALLBACK
                        }
                    }
                })
            })
            .collect();
        drop(geo);
        let prepared = data.prepare(&self.mesh_config)?;
        Ok(PartAsset {
            gpu_bytes: prepared.gpu_bytes(),
            mesh: Mutex::new(Some(prepared)),
            plans,
        })
    }

    fn size_of(&self, value: &PartAsset) -> usize {
        usize::try_from(value.gpu_bytes)
            .unwrap_or(usize::MAX)
            .max(1)
    }
}

/// A record source that reads nothing: the texture decoder reads through the material
/// library itself (texture lists, highres fallback).
#[derive(Debug, Clone, Copy)]
pub struct NoRecord;

impl RecordSource for NoRecord {
    fn read_record(&self, _did: u32) -> Result<Vec<u8>, AssetError> {
        Ok(Vec::new())
    }
}

/// Decodes and composes textures for [`TexRequest`]s (`AssetKey { did, variant }`).
pub struct TexDecoder {
    materials: Arc<MaterialLibrary<Arc<ArchiveSet>>>,
    cap: u32,
}

impl std::fmt::Debug for TexDecoder {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("TexDecoder")
            .field("cap", &self.cap)
            .finish_non_exhaustive()
    }
}

impl Decoder for TexDecoder {
    type Output = RgbaImage;
    type Error = ObjectError;
    const NAME: &'static str = "walker-texture";

    fn decode(
        &self,
        key: AssetKey,
        _bytes: Vec<u8>,
        _ctx: &JobContext<'_>,
    ) -> Result<RgbaImage, ObjectError> {
        let load = |did: u32| self.materials.texture(Did(did), Some(self.cap));
        let img = match key.variant {
            VARIANT_ALPHA_ONLY => {
                let a = load(key.did)?;
                compose_texture(None, AlphaFrom::Image(&a.image))
            }
            VARIANT_DYE_MASK => {
                let a = load(key.did)?;
                dye_mask(&a.image)
            }
            VARIANT_OPAQUE | VARIANT_OWN_ALPHA => {
                let b = load(key.did)?;
                let alpha = if key.variant == VARIANT_OPAQUE {
                    AlphaFrom::Opaque
                } else {
                    AlphaFrom::Own
                };
                compose_texture(Some(&b.image), alpha)
            }
            other => {
                let b = load(key.did)?;
                let a = load(other)?;
                compose_texture(Some(&b.image), AlphaFrom::Image(&a.image))
            }
        }
        .ok_or(ObjectError::Compose)?;
        Ok(downscale_to(img, self.cap))
    }

    fn size_of(&self, value: &RgbaImage) -> usize {
        value.pixels.len().max(1)
    }
}

/// Settings of the object layer.
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct ObjectConfig {
    /// Objects of landblocks nearer than this (world units, to the block square) are
    /// loaded and drawn.
    pub distance: f64,
    /// Landblocks within this distance load their parts at `Immediate` priority.
    pub near: f64,
    /// Smallest projected radius drawn, in pixels.
    pub min_pixels: f32,
    /// Most object draws per frame.
    pub max_draws: usize,
    /// GPU bytes for part meshes and textures together.
    pub gpu_budget: u64,
    /// Largest texture edge.
    pub texture_cap: u32,
}

impl Default for ObjectConfig {
    fn default() -> Self {
        Self {
            distance: 900.0,
            near: 480.0,
            min_pixels: 1.5,
            max_draws: 20_000,
            gpu_budget: 768 << 20,
            texture_cap: 512,
        }
    }
}

enum PartState {
    Loading(Handle<PartAsset>),
    Retry(Instant),
    Waiting(Arc<PartAsset>),
    Ready(GpuPart),
    Failed,
}

struct GpuPart {
    mesh: MeshId,
    plans: Vec<MaterialPlan>,
    center: Vec3,
    radius: f32,
    bytes: u64,
}

struct PartEntry {
    refs: u32,
    state: PartState,
    priority: Priority,
    unused_since: Option<Instant>,
}

enum TexState {
    Loading(Handle<RgbaImage>),
    Retry(Instant),
    Waiting(Arc<RgbaImage>),
    Ready(TextureId, u64),
    Failed,
}

struct TexEntry {
    refs: u32,
    state: TexState,
    unused_since: Option<Instant>,
}

struct BlockObjects {
    instances: Arc<[Instance]>,
    parts: Vec<u32>,
    active: bool,
    /// Block-local box around the bounding spheres of the instances whose part is on the
    /// GPU (`None`: none is).
    bounds: Option<(Vec3, Vec3)>,
    /// `bounds` needs recomputing (a part of the block was uploaded, or it activated).
    dirty: bool,
}

/// The block-local box around the bounding spheres of `instances` whose part `sphere`
/// knows (object-space centre and radius), exactly the spheres the per-instance cull
/// tests, so the block box is visible whenever one of them is. `None` when no instance
/// has a sphere. Non-finite spheres are skipped.
#[must_use]
pub fn instance_bounds(
    instances: &[Instance],
    sphere: impl Fn(u32) -> Option<(Vec3, f32)>,
) -> Option<(Vec3, Vec3)> {
    let mut out: Option<(Vec3, Vec3)> = None;
    for inst in instances {
        let Some((center, radius)) = sphere(inst.part) else {
            continue;
        };
        let c = inst.model.transform_point3(center);
        let r = radius * inst.scale;
        if !c.is_finite() || !r.is_finite() {
            continue;
        }
        let (lo, hi) = (c - Vec3::splat(r.abs()), c + Vec3::splat(r.abs()));
        out = Some(match out {
            Some((a, b)) => (a.min(lo), b.max(hi)),
            None => (lo, hi),
        });
    }
    out
}

/// Counters for the HUD and frame log.
#[derive(Debug, Clone, Copy, Default, PartialEq)]
pub struct ObjectStats {
    /// Landblocks with objects tracked.
    pub blocks: usize,
    /// Landblocks whose objects are loaded/drawn.
    pub active_blocks: usize,
    /// Instances in active landblocks.
    pub instances: usize,
    /// Unique parts referenced.
    pub parts: usize,
    /// Parts on the GPU.
    pub parts_ready: usize,
    /// Parts loading or waiting for upload.
    pub parts_pending: usize,
    /// Parts that failed.
    pub parts_failed: usize,
    /// Textures on the GPU.
    pub textures_ready: usize,
    /// Textures loading or waiting for upload.
    pub textures_pending: usize,
    /// GPU bytes of part meshes.
    pub mesh_bytes: u64,
    /// GPU bytes of textures.
    pub texture_bytes: u64,
    /// Object draws recorded last frame.
    pub draws: usize,
    /// Instances culled last frame.
    pub culled: usize,
    /// Whether the draw cap cut the last frame short.
    pub capped: bool,
    /// Object distance in force (shrunk below the configured one while the GPU budget is
    /// exceeded by what is in range).
    pub distance: f64,
}

/// Where a texture of the object layer stands ([`ObjectLayer::texture`]).
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum TexLookup {
    /// On the GPU.
    Ready(TextureId),
    /// Failed for good.
    Failed,
    /// Loading, waiting for upload, or not referenced.
    Pending,
}

/// The object layer (module docs).
pub struct ObjectLayer {
    config: ObjectConfig,
    materials: Arc<MaterialLibrary<Arc<ArchiveSet>>>,
    parts: AssetStore<StampedSource, PartDecoder>,
    textures: AssetStore<NoRecord, TexDecoder>,
    blocks: HashMap<LandblockKey, BlockObjects>,
    gpu_parts: HashMap<u32, PartEntry>,
    gpu_tex: HashMap<TexRequest, TexEntry>,
    mesh_bytes: u64,
    texture_bytes: u64,
    stats: ObjectStats,
    projection_scale: f32,
    /// Object range in force (see [`ObjectStats::distance`]).
    distance: f64,
    /// When the range was last adjusted.
    adjusted: Option<Instant>,
    order: Vec<(f64, LandblockKey)>,
    /// Arena bytes part meshes may fill together (the arena cap less the terrain's share).
    arena_limit: u64,
    /// When a part last had to wait for arena space.
    arena_full_at: Option<Instant>,
}

/// Whether instance `inst` of block `key` is a placed figure the character layer draws
/// (its tag is in `characters`), so the object layer leaves it out.
#[must_use]
pub fn drawn_as_character(key: LandblockKey, inst: &Instance, characters: &HashSet<u64>) -> bool {
    inst.figure
        .is_some_and(|i| characters.contains(&figure_tag(key, i)))
}

impl std::fmt::Debug for ObjectLayer {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("ObjectLayer")
            .field("stats", &self.stats)
            .finish_non_exhaustive()
    }
}

impl ObjectLayer {
    /// A layer whose decode jobs run on `pool`; part meshes fill at most `arena_limit`
    /// bytes of the renderer's mesh arena.
    #[must_use]
    pub fn new(
        set: Arc<ArchiveSet>,
        pool: &WorkerPool,
        config: ObjectConfig,
        mesh_config: MeshConfig,
        arena_limit: u64,
    ) -> Self {
        let materials = Arc::new(MaterialLibrary::with_config(
            Arc::clone(&set),
            LibraryConfig {
                max_texture_dimension: 4096,
                ..LibraryConfig::default()
            },
        ));
        let library = Arc::clone(&materials);
        let parts = AssetStore::new(
            pool,
            StampedSource::new(Arc::clone(&set)),
            PartDecoder {
                materials: Arc::clone(&materials),
                mesh_config,
                limits: GeometryLimits::default(),
            },
            StoreConfig {
                // The prepared streams are moved out on upload: never serve a product
                // twice from the cache.
                cache_budget: 0,
                max_completions: 16_384,
                max_completion_bytes: 512 << 20,
                ..StoreConfig::default()
            },
        );
        let textures = AssetStore::new(
            pool,
            NoRecord,
            TexDecoder {
                materials,
                cap: config.texture_cap.max(1),
            },
            StoreConfig {
                cache_budget: 64 << 20,
                max_completions: 16_384,
                max_completion_bytes: 512 << 20,
                ..StoreConfig::default()
            },
        );
        Self {
            config,
            materials: library,
            parts,
            textures,
            blocks: HashMap::new(),
            gpu_parts: HashMap::new(),
            gpu_tex: HashMap::new(),
            mesh_bytes: 0,
            texture_bytes: 0,
            stats: ObjectStats::default(),
            projection_scale: 900.0,
            distance: config.distance,
            adjusted: None,
            order: Vec::new(),
            arena_limit,
            arena_full_at: None,
        }
    }

    /// The material library the layer decodes with (shared with the character figures).
    #[must_use]
    pub fn materials(&self) -> Arc<MaterialLibrary<Arc<ArchiveSet>>> {
        Arc::clone(&self.materials)
    }

    /// Adds a reference to each texture of `reqs` for a user outside the layer (character
    /// figures): referenced textures are loaded, uploaded within the frame budget and kept
    /// until released ([`ObjectLayer::release_textures`]).
    pub fn acquire_textures(&mut self, reqs: impl IntoIterator<Item = TexRequest>) {
        let now = Instant::now();
        for t in reqs {
            let te = self.gpu_tex.entry(t).or_insert_with(|| TexEntry {
                refs: 0,
                state: TexState::Retry(now - RETRY_AFTER),
                unused_since: None,
            });
            te.refs += 1;
            te.unused_since = None;
        }
    }

    /// Drops one reference to each texture of `reqs` (taken with
    /// [`ObjectLayer::acquire_textures`]); unreferenced textures are evicted like the
    /// layer's own.
    pub fn release_textures(&mut self, reqs: impl IntoIterator<Item = TexRequest>) {
        let now = Instant::now();
        for t in reqs {
            if let Some(te) = self.gpu_tex.get_mut(&t) {
                te.refs = te.refs.saturating_sub(1);
                if te.refs == 0 {
                    te.unused_since = Some(now);
                }
            }
        }
    }

    /// The state of texture `t`.
    #[must_use]
    pub fn texture(&self, t: TexRequest) -> TexLookup {
        match self.gpu_tex.get(&t).map(|e| &e.state) {
            Some(TexState::Ready(id, _)) => TexLookup::Ready(*id),
            Some(TexState::Failed) => TexLookup::Failed,
            _ => TexLookup::Pending,
        }
    }

    /// Pixels per world unit at distance 1 (for the projected-size cull).
    pub fn set_projection_scale(&mut self, s: f32) {
        if s.is_finite() && s > 0.0 {
            self.projection_scale = s;
        }
    }

    /// Counters.
    #[must_use]
    pub fn stats(&self) -> ObjectStats {
        self.stats
    }

    /// Whether every part and texture of the active landblocks is on the GPU or failed.
    #[must_use]
    pub fn settled(&self) -> bool {
        self.gpu_parts
            .values()
            .filter(|e| e.refs > 0)
            .all(|e| matches!(e.state, PartState::Ready(_) | PartState::Failed))
            && self
                .gpu_tex
                .values()
                .filter(|e| e.refs > 0)
                .all(|e| matches!(e.state, TexState::Ready(..) | TexState::Failed))
    }

    /// Sets the instances of `key` (replacing earlier ones).
    pub fn insert(&mut self, key: LandblockKey, instances: Arc<[Instance]>) {
        self.remove(key);
        let mut parts: Vec<u32> = instances.iter().map(|i| i.part).collect();
        parts.sort_unstable();
        parts.dedup();
        self.blocks.insert(
            key,
            BlockObjects {
                instances,
                parts,
                active: false,
                bounds: None,
                dirty: true,
            },
        );
    }

    /// Forgets `key`'s instances (its parts lose a reference).
    pub fn remove(&mut self, key: LandblockKey) {
        if let Some(b) = self.blocks.remove(&key)
            && b.active
        {
            self.release_parts(&b.parts);
        }
    }

    /// Keys tracked.
    pub fn keys(&self) -> impl Iterator<Item = LandblockKey> + '_ {
        self.blocks.keys().copied()
    }

    fn release_parts(&mut self, parts: &[u32]) {
        let now = Instant::now();
        for p in parts {
            let Some(e) = self.gpu_parts.get_mut(p) else {
                continue;
            };
            e.refs = e.refs.saturating_sub(1);
            if e.refs == 0 {
                match &e.state {
                    PartState::Loading(h) => {
                        h.cancel();
                        self.gpu_parts.remove(p);
                    }
                    PartState::Retry(_) | PartState::Failed | PartState::Waiting(_) => {
                        self.gpu_parts.remove(p);
                    }
                    PartState::Ready(_) => e.unused_since = Some(now),
                }
            }
        }
    }

    fn acquire_parts(&mut self, parts: &[u32], priority: Priority) {
        for &p in parts {
            let e = self.gpu_parts.entry(p).or_insert_with(|| PartEntry {
                refs: 0,
                state: PartState::Retry(Instant::now() - RETRY_AFTER),
                priority,
                unused_since: None,
            });
            e.refs += 1;
            e.unused_since = None;
            if priority > e.priority {
                e.priority = priority;
                if let PartState::Loading(h) = &e.state {
                    h.set_priority(priority);
                }
            }
        }
    }

    /// Per frame: activates landblocks within range, requests, collects and uploads
    /// within `budget`, evicts over budget. `camera` is region-global (G_x, G_y).
    pub fn update(
        &mut self,
        renderer: &mut Renderer<'_>,
        camera: [f64; 2],
        budget: &mut UploadBudget,
    ) {
        let now = Instant::now();
        // Activation with a one-landblock hysteresis.
        self.adapt_distance(now);
        let range = self.distance;
        let keys: Vec<LandblockKey> = self.blocks.keys().copied().collect();
        self.order.clear();
        for key in keys {
            let d = key.distance_to(camera);
            self.order.push((d, key));
            let Some(b) = self.blocks.get(&key) else {
                continue;
            };
            let want = if b.active {
                d <= range + LANDBLOCK_SIZE
            } else {
                d <= range
            };
            if want != b.active {
                let parts = b.parts.clone();
                if want {
                    let prio = if d <= self.config.near {
                        Priority::Immediate
                    } else {
                        Priority::Visible
                    };
                    self.acquire_parts(&parts, prio);
                } else {
                    self.release_parts(&parts);
                }
                if let Some(b) = self.blocks.get_mut(&key) {
                    b.active = want;
                    b.dirty = true;
                }
            }
        }
        self.order
            .sort_unstable_by(|a, b| a.0.total_cmp(&b.0).then(a.1.cmp(&b.1)));

        self.request_parts(now);
        self.poll_parts();
        let uploaded = self.upload_parts(renderer, budget, now);
        self.request_textures(now);
        self.poll_textures();
        self.upload_textures(renderer, budget);
        self.evict(renderer, now);
        self.refresh_bounds(&uploaded);
        self.refresh_stats();
    }

    /// Whether part meshes had to wait for arena space within [`ARENA_PRESSURE`].
    fn arena_pressed(&self, now: Instant) -> bool {
        self.arena_full_at
            .is_some_and(|t| now.saturating_duration_since(t) < ARENA_PRESSURE)
    }

    /// Recomputes the boxes of active blocks that use one of `uploaded` (sorted) or were
    /// marked dirty.
    fn refresh_bounds(&mut self, uploaded: &[u32]) {
        let parts = &self.gpu_parts;
        let sphere = |p: u32| match parts.get(&p).map(|e| &e.state) {
            Some(PartState::Ready(g)) => Some((g.center, g.radius)),
            _ => None,
        };
        for b in self.blocks.values_mut().filter(|b| b.active) {
            let touched = b.dirty
                || (!uploaded.is_empty()
                    && b.parts.iter().any(|p| uploaded.binary_search(p).is_ok()));
            if touched {
                b.bounds = instance_bounds(&b.instances, sphere);
                b.dirty = false;
            }
        }
    }

    fn request_parts(&mut self, now: Instant) {
        for (&did, e) in &mut self.gpu_parts {
            if let PartState::Retry(at) = e.state
                && e.refs > 0
                && now >= at
            {
                let h = self.parts.request(did, e.priority);
                e.state = match h.state() {
                    AssetState::Ready(a) => PartState::Waiting(a),
                    AssetState::Failed(err) if err.is_data_error() => {
                        tracing::debug!(part = %format_args!("{did:#010x}"), "part failed: {err}");
                        PartState::Failed
                    }
                    AssetState::Failed(_) | AssetState::Cancelled => {
                        PartState::Retry(now + RETRY_AFTER)
                    }
                    AssetState::Queued | AssetState::Loading => PartState::Loading(h),
                };
            }
        }
    }

    fn poll_parts(&mut self) {
        for c in self.parts.poll_completions(4096) {
            let Some(e) = self.gpu_parts.get_mut(&c.key.did) else {
                continue;
            };
            if !matches!(e.state, PartState::Loading(_)) {
                continue;
            }
            e.state = match c.outcome {
                Outcome::Ready(a) => PartState::Waiting(a),
                Outcome::Failed(err) => {
                    if err.is_data_error() {
                        tracing::debug!(part = %c.key, "part failed: {err}");
                        PartState::Failed
                    } else {
                        PartState::Retry(Instant::now() + RETRY_AFTER)
                    }
                }
                Outcome::Cancelled => PartState::Retry(Instant::now()),
            };
        }
        // Handles that finished without a completion (dropped completions).
        for e in self.gpu_parts.values_mut() {
            if let PartState::Loading(h) = &e.state
                && h.is_finished()
            {
                e.state = match h.state() {
                    AssetState::Ready(a) => PartState::Waiting(a),
                    AssetState::Failed(err) if err.is_data_error() => PartState::Failed,
                    _ => PartState::Retry(Instant::now() + RETRY_AFTER),
                };
            }
        }
    }

    /// Uploads waiting parts, nearest landblocks first; returns the parts uploaded
    /// (sorted).
    fn upload_parts(
        &mut self,
        renderer: &mut Renderer<'_>,
        budget: &mut UploadBudget,
        now: Instant,
    ) -> Vec<u32> {
        let mut uploaded = Vec::new();
        // Nearest landblocks' parts first.
        let mut wanted: Vec<u32> = Vec::new();
        let mut seen: HashSet<u32> = HashSet::new();
        for &(_, key) in &self.order {
            let Some(b) = self.blocks.get(&key) else {
                continue;
            };
            if !b.active {
                continue;
            }
            for &p in &b.parts {
                if matches!(
                    self.gpu_parts.get(&p).map(|e| &e.state),
                    Some(PartState::Waiting(_))
                ) && seen.insert(p)
                {
                    wanted.push(p);
                }
            }
        }
        for p in wanted {
            let Some(e) = self.gpu_parts.get_mut(&p) else {
                continue;
            };
            let PartState::Waiting(asset) = &e.state else {
                continue;
            };
            // Leave the terrain its share of the shared mesh arena.
            if self.mesh_bytes.saturating_add(asset.gpu_bytes) > self.arena_limit {
                self.arena_full_at = Some(now);
                break;
            }
            if !budget.try_take(asset.gpu_bytes) {
                break;
            }
            let prepared = asset
                .mesh
                .lock()
                .unwrap_or_else(PoisonError::into_inner)
                .take();
            let plans = asset.plans.clone();
            let Some(prepared) = prepared else {
                e.state = PartState::Retry(now);
                continue;
            };
            let bytes = prepared.gpu_bytes();
            match renderer.upload_prepared(prepared) {
                Ok(id) => {
                    let (center, radius) = renderer
                        .mesh_info(id)
                        .map_or((Vec3::ZERO, 1.0), |i| (i.sphere.center, i.sphere.radius));
                    self.mesh_bytes += bytes;
                    for t in plans.iter().filter_map(|p| p.texture) {
                        let te = self.gpu_tex.entry(t).or_insert_with(|| TexEntry {
                            refs: 0,
                            state: TexState::Retry(now - RETRY_AFTER),
                            unused_since: None,
                        });
                        te.refs += 1;
                        te.unused_since = None;
                    }
                    e.state = PartState::Ready(GpuPart {
                        mesh: id,
                        plans,
                        center,
                        radius,
                        bytes,
                    });
                    uploaded.push(p);
                }
                Err(tessera_render::RenderError::LimitExceeded {
                    what: "mesh arena bytes",
                    ..
                }) => {
                    // Transient: space comes back as parts are evicted.
                    tracing::debug!(part = %format_args!("{p:#010x}"), "mesh arena full; part retried later");
                    self.arena_full_at = Some(now);
                    e.state = PartState::Retry(now + ARENA_RETRY);
                }
                Err(e2) => {
                    tracing::debug!(part = %format_args!("{p:#010x}"), "part upload failed: {e2}");
                    e.state = PartState::Failed;
                }
            }
        }
        uploaded.sort_unstable();
        uploaded
    }

    fn request_textures(&mut self, now: Instant) {
        for (t, e) in &mut self.gpu_tex {
            if let TexState::Retry(at) = e.state
                && e.refs > 0
                && now >= at
            {
                let h = self
                    .textures
                    .request(AssetKey::new(t.did, t.variant), Priority::Visible);
                e.state = match h.state() {
                    AssetState::Ready(img) => TexState::Waiting(img),
                    AssetState::Failed(err) if err.is_data_error() => {
                        tracing::debug!(texture = %format_args!("{:#010x}/{}", t.did, t.variant), "texture failed: {err}");
                        TexState::Failed
                    }
                    AssetState::Failed(_) | AssetState::Cancelled => {
                        TexState::Retry(now + RETRY_AFTER)
                    }
                    AssetState::Queued | AssetState::Loading => TexState::Loading(h),
                };
            }
        }
    }

    fn poll_textures(&mut self) {
        for c in self.textures.poll_completions(4096) {
            let t = TexRequest {
                did: c.key.did,
                variant: c.key.variant,
            };
            let Some(e) = self.gpu_tex.get_mut(&t) else {
                continue;
            };
            if !matches!(e.state, TexState::Loading(_)) {
                continue;
            }
            e.state = match c.outcome {
                Outcome::Ready(img) => TexState::Waiting(img),
                Outcome::Failed(err) if err.is_data_error() => {
                    tracing::debug!(texture = %c.key, "texture failed: {err}");
                    TexState::Failed
                }
                Outcome::Failed(_) | Outcome::Cancelled => {
                    TexState::Retry(Instant::now() + RETRY_AFTER)
                }
            };
        }
        for e in self.gpu_tex.values_mut() {
            if let TexState::Loading(h) = &e.state
                && h.is_finished()
            {
                e.state = match h.state() {
                    AssetState::Ready(img) => TexState::Waiting(img),
                    AssetState::Failed(err) if err.is_data_error() => TexState::Failed,
                    _ => TexState::Retry(Instant::now() + RETRY_AFTER),
                };
            }
        }
    }

    fn upload_textures(&mut self, renderer: &mut Renderer<'_>, budget: &mut UploadBudget) {
        for (t, e) in &mut self.gpu_tex {
            let TexState::Waiting(img) = &e.state else {
                continue;
            };
            let bytes = img.pixels.len() as u64;
            if !budget.try_take(bytes) {
                break;
            }
            let label = format!("tex {:#010x}/{}", t.did, t.variant);
            match renderer.upload_texture(&TextureUpload {
                label: Some(&label),
                wrap: Wrap::Repeat,
                color_space: if t.variant == VARIANT_DYE_MASK {
                    tessera_render::ColorSpace::Linear
                } else {
                    tessera_render::ColorSpace::Srgb
                },
                ..TextureUpload::rgba8(img.width, img.height, &img.pixels)
            }) {
                Ok(id) => {
                    self.texture_bytes += bytes;
                    e.state = TexState::Ready(id, bytes);
                }
                Err(err) => {
                    tracing::debug!(texture = %label, "texture upload failed: {err}");
                    e.state = TexState::Failed;
                }
            }
        }
    }

    /// Frees unreferenced parts and textures: those idle longer than [`IDLE_KEEP`], and
    /// more (least recently used first) while over the GPU budget.
    fn evict(&mut self, renderer: &mut Renderer<'_>, now: Instant) {
        let pressed = self.arena_pressed(now);
        let over = |s: &Self| pressed || s.mesh_bytes + s.texture_bytes > s.config.gpu_budget;
        let mut idle: Vec<(Instant, u32)> = self
            .gpu_parts
            .iter()
            .filter(|(_, e)| e.refs == 0)
            .filter_map(|(d, e)| e.unused_since.map(|t| (t, *d)))
            .collect();
        idle.sort_unstable();
        for (since, did) in idle {
            if now.saturating_duration_since(since) < IDLE_KEEP && !over(self) {
                break;
            }
            if let Some(e) = self.gpu_parts.remove(&did)
                && let PartState::Ready(g) = e.state
            {
                renderer.destroy_mesh(g.mesh);
                self.mesh_bytes = self.mesh_bytes.saturating_sub(g.bytes);
                for t in g.plans.iter().filter_map(|p| p.texture) {
                    if let Some(te) = self.gpu_tex.get_mut(&t) {
                        te.refs = te.refs.saturating_sub(1);
                        if te.refs == 0 {
                            te.unused_since = Some(now);
                        }
                    }
                }
            }
        }
        // Textures nobody references.
        let mut idle: Vec<(Instant, TexRequest)> = self
            .gpu_tex
            .iter()
            .filter(|(_, e)| e.refs == 0)
            .map(|(t, e)| (e.unused_since.unwrap_or(now), *t))
            .collect();
        idle.sort_unstable();
        for (since, t) in idle {
            let ready = matches!(
                self.gpu_tex.get(&t).map(|e| &e.state),
                Some(TexState::Ready(..))
            );
            if ready && now.saturating_duration_since(since) < IDLE_KEEP && !over(self) {
                continue;
            }
            if let Some(e) = self.gpu_tex.remove(&t) {
                match e.state {
                    TexState::Ready(id, bytes) => {
                        renderer.destroy_texture(id);
                        self.texture_bytes = self.texture_bytes.saturating_sub(bytes);
                    }
                    TexState::Loading(h) => h.cancel(),
                    _ => {}
                }
            }
        }
    }

    /// Keeps GPU memory near the budget: resources still referenced by landblocks in
    /// range cannot be evicted, so while they alone exceed the budget (or parts in range
    /// wait for mesh-arena space) the object range shrinks (10 % per second, down to one
    /// landblock), and it grows back while under 80 % with no arena pressure.
    fn adapt_distance(&mut self, now: Instant) {
        if self
            .adjusted
            .is_some_and(|t| now.saturating_duration_since(t) < Duration::from_secs(1))
        {
            return;
        }
        // Only what landblocks in range reference counts: the rest is a cache that the
        // eviction trims to the budget anyway.
        let mut textures: HashSet<TexRequest> = HashSet::new();
        let mut used = 0u64;
        for e in self.gpu_parts.values().filter(|e| e.refs > 0) {
            if let PartState::Ready(g) = &e.state {
                used += g.bytes;
                textures.extend(g.plans.iter().filter_map(|p| p.texture));
            }
        }
        for t in &textures {
            if let Some(TexEntry {
                state: TexState::Ready(_, b),
                ..
            }) = self.gpu_tex.get(t)
            {
                used += b;
            }
        }
        let budget = self.config.gpu_budget;
        let before = self.distance;
        let pressed = self.arena_pressed(now);
        if used > budget || pressed {
            self.distance = (self.distance * 0.9).max(LANDBLOCK_SIZE);
        } else if used < budget / 10 * 8 {
            self.distance = (self.distance * 1.05).min(self.config.distance);
        }
        if (self.distance - before).abs() > f64::EPSILON {
            tracing::debug!(
                distance = self.distance,
                used,
                budget,
                "object range adjusted"
            );
        }
        self.adjusted = Some(now);
    }

    fn refresh_stats(&mut self) {
        let mut s = ObjectStats {
            blocks: self.blocks.len(),
            draws: self.stats.draws,
            culled: self.stats.culled,
            capped: self.stats.capped,
            mesh_bytes: self.mesh_bytes,
            texture_bytes: self.texture_bytes,
            distance: self.distance,
            ..ObjectStats::default()
        };
        for b in self.blocks.values().filter(|b| b.active) {
            s.active_blocks += 1;
            s.instances += b.instances.len();
        }
        for e in self.gpu_parts.values() {
            if e.refs > 0 {
                s.parts += 1;
            }
            match e.state {
                PartState::Ready(_) => s.parts_ready += 1,
                PartState::Failed => s.parts_failed += 1,
                _ => s.parts_pending += 1,
            }
        }
        for e in self.gpu_tex.values() {
            match e.state {
                TexState::Ready(..) => s.textures_ready += 1,
                TexState::Failed => {}
                _ => s.textures_pending += 1,
            }
        }
        self.stats = s;
    }

    /// Records the visible objects into `pass`. `camera` is the eye in `anchor`'s frame.
    /// `characters` holds the tags ([`figure_tag`]) of the placed figures the character
    /// layer draws: their static instances are left out (`walker.md` §10.2).
    pub fn draw(
        &mut self,
        pass: &mut Pass,
        anchor: &Anchor,
        camera: Vec3,
        frustum: &Frustum,
        characters: &HashSet<u64>,
    ) {
        let mut draws = 0usize;
        let mut culled = 0usize;
        let mut capped = false;
        let max_d = (self.distance + LANDBLOCK_SIZE) as f32;
        let min_px = self.config.min_pixels;
        let scale = self.projection_scale;
        'blocks: for &(_, key) in &self.order {
            let Some(b) = self.blocks.get(&key) else {
                continue;
            };
            if !b.active || b.instances.is_empty() {
                continue;
            }
            let Some(off) = anchor.offset_of(key) else {
                continue;
            };
            // Nothing of the block on the GPU yet, or its box is out of view.
            let Some((lo, hi)) = b.bounds else {
                continue;
            };
            if !frustum.aabb_visible(off + lo, off + hi) {
                culled += b.instances.len();
                continue;
            }
            let block_tf = Mat4::from_translation(off);
            for inst in b.instances.iter() {
                if drawn_as_character(key, inst, characters) {
                    continue;
                }
                let Some(e) = self.gpu_parts.get(&inst.part) else {
                    continue;
                };
                let PartState::Ready(g) = &e.state else {
                    continue;
                };
                let center = off + inst.model.transform_point3(g.center);
                let radius = g.radius * inst.scale;
                let d = (center - camera).length();
                if d - radius > max_d
                    || radius * scale < min_px * (d - radius).max(1.0)
                    || !frustum.sphere_visible(center, radius)
                {
                    culled += 1;
                    continue;
                }
                let tf = block_tf * inst.model;
                for (i, plan) in g.plans.iter().enumerate() {
                    let texture = match plan.texture {
                        None => None,
                        Some(t) => match self.gpu_tex.get(&t).map(|e| &e.state) {
                            Some(TexState::Ready(id, _)) => Some(*id),
                            // Not there yet: skip rather than draw a solid placeholder
                            // (alpha-tested foliage would show as cards).
                            Some(TexState::Failed) if plan.alpha == AlphaKind::Opaque => None,
                            _ => continue,
                        },
                    };
                    if draws >= self.config.max_draws {
                        capped = true;
                        break 'blocks;
                    }
                    let material = Material {
                        base_color: plan.base_color,
                        base_texture: texture,
                        alpha: match plan.alpha {
                            AlphaKind::Opaque => AlphaMode::Opaque,
                            AlphaKind::Mask(c) => AlphaMode::Mask { cutoff: c },
                            AlphaKind::Blend => AlphaMode::Blend,
                        },
                        two_sided: plan.two_sided,
                        unlit: plan.unlit,
                        ..Material::color(plan.base_color)
                    };
                    pass.mesh(
                        MeshDraw::new(g.mesh, tf, material)
                            .with_submesh(u32::try_from(i).unwrap_or(u32::MAX)),
                    );
                    draws += 1;
                }
            }
        }
        self.stats.draws = draws;
        self.stats.culled = culled;
        self.stats.capped = capped;
    }

    /// Frees every GPU resource and cancels outstanding requests.
    pub fn clear(&mut self, renderer: &mut Renderer<'_>) {
        for (_, e) in self.gpu_parts.drain() {
            match e.state {
                PartState::Ready(g) => {
                    renderer.destroy_mesh(g.mesh);
                }
                PartState::Loading(h) => h.cancel(),
                _ => {}
            }
        }
        for (_, e) in self.gpu_tex.drain() {
            match e.state {
                TexState::Ready(id, _) => {
                    renderer.destroy_texture(id);
                }
                TexState::Loading(h) => h.cancel(),
                _ => {}
            }
        }
        for b in self.blocks.values_mut() {
            b.active = false;
            b.bounds = None;
            b.dirty = true;
        }
        self.mesh_bytes = 0;
        self.texture_bytes = 0;
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use tessera_render::{Camera, Perspective};

    fn inst(part: u32, at: Vec3, scale: f32) -> Instance {
        Instance {
            part,
            model: Mat4::from_scale_rotation_translation(
                Vec3::splat(scale),
                tessera_render::glam::Quat::IDENTITY,
                at,
            ),
            scale,
            figure: None,
        }
    }

    #[test]
    fn figure_instances_are_left_out_only_while_drawn_as_characters() {
        let key = LandblockKey::new(1, 10, 20).unwrap();
        let other = LandblockKey::new(1, 13, 20).unwrap();
        let mut fig = inst(1, Vec3::ZERO, 1.0);
        fig.figure = Some(3);
        let plain = inst(2, Vec3::ZERO, 1.0);
        let mut shown = HashSet::new();
        // Pending, failed or released figures stay static instances.
        assert!(!drawn_as_character(key, &fig, &shown));
        shown.insert(figure_tag(key, 3));
        assert!(drawn_as_character(key, &fig, &shown));
        // Whatever the distance (the sandbox's hysteresis ring included): only the tag
        // counts, and only for that block's figure.
        assert!(!drawn_as_character(other, &fig, &shown));
        shown.insert(figure_tag(other, 2));
        assert!(!drawn_as_character(other, &fig, &shown));
        assert!(!drawn_as_character(key, &plain, &shown));
    }

    #[test]
    fn bounds_cover_the_spheres_of_uploaded_parts() {
        let instances = [
            inst(1, Vec3::new(10.0, 20.0, 5.0), 2.0),
            inst(2, Vec3::new(-300.0, 0.0, 0.0), 1.0),
            inst(3, Vec3::new(1.0e6, 0.0, 0.0), 1.0),
        ];
        // Part 1: sphere at (0, 0, 10) radius 3 (scaled ×2); part 2 not uploaded; part 3
        // has a non-finite sphere.
        let sphere = |p: u32| match p {
            1 => Some((Vec3::new(0.0, 0.0, 10.0), 3.0)),
            3 => Some((Vec3::NAN, 1.0)),
            _ => None,
        };
        let (lo, hi) = instance_bounds(&instances, sphere).expect("one sphere");
        assert_eq!(lo, Vec3::new(4.0, 14.0, 19.0));
        assert_eq!(hi, Vec3::new(16.0, 26.0, 31.0));
        assert_eq!(instance_bounds(&instances, |_| None), None);
    }

    /// The review case: a tall part whose instance origin lies outside its block (local
    /// y = 409) and whose top is far above the origin. The old fixed box (x/y −80..240,
    /// z up to origin + 200) was out of view while the part's top was in view.
    #[test]
    fn tall_parts_outside_their_block_stay_visible() {
        let instances = [inst(7, Vec3::new(66.96, 409.17, 400.0), 1.0)];
        let sphere = |_| Some((Vec3::new(0.0, 0.0, 400.0), 30.0));
        let (lo, hi) = instance_bounds(&instances, sphere).unwrap();
        let cam = Camera {
            eye: Vec3::new(66.96, 300.0, 800.0),
            target: Vec3::new(66.96, 409.0, 800.0),
            up: Vec3::Z,
            projection: Perspective {
                fov_y: 1.0,
                near: 0.5,
                far: 4_000.0,
            },
        };
        let f = Frustum::from_view_proj(cam.view_proj(16.0 / 9.0));
        let top = Vec3::new(66.96, 409.17, 800.0);
        assert!(f.sphere_visible(top, 1.0));
        let old = (
            Vec3::new(-80.0, -80.0, 400.0 - 80.0),
            Vec3::new(240.0, 240.0, 400.0 + 200.0),
        );
        assert!(!f.aabb_visible(old.0, old.1), "the old box hid it");
        assert!(f.aabb_visible(lo, hi));
    }
}
