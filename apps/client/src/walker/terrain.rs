// SPDX-License-Identifier: GPL-3.0-or-later
//! Terrain of the resident landblocks (`docs/specs/terrain.md`): per block a
//! [`TerrainBlock`] built on a worker, a level of detail chosen per frame by screen-space
//! error, meshes stitched to the neighbours' levels and normals shared across seams, both
//! built and prepared for upload on workers. The main thread only selects levels, queues
//! jobs and uploads finished meshes within the frame's upload budget.
//!
//! # Crack-free swaps
//!
//! Stitching (`terrain.md` §4) is crack-free only when both blocks of a seam are drawn with
//! meshes built for the same edge level. Meshes finish and upload one block at a time, so
//! a new mesh is first **staged** (uploaded, not drawn) and only replaces the drawn one in
//! a frame where every drawn neighbour agrees on the shared edge level, swapping the
//! neighbours' staged meshes in the same frame when needed ([`promotable`]).
//!
//! # Failures
//!
//! A mesh that fails to build or upload is not retried for the same signature until a
//! back-off has passed (doubling up to [`MAX_BACKOFF`]); a block job that fails (its
//! decoder panicked) leaves the block without terrain. Both count as settled, so a
//! screenshot or flight does not wait for them, and both show in [`TerrainStats::failed`].
//!
//! The same block job also turns the bundle's drawable static entities into object
//! instances ([`Instance`]) and renders a small height-shaded overview tile for the
//! minimap fallback, so no per-entity work happens on the main thread.

use std::collections::{HashMap, HashSet};
use std::panic::AssertUnwindSafe;
use std::sync::{Arc, PoisonError, RwLock};
use std::time::{Duration, Instant};

use tessera_render::glam::{Mat4, Quat, Vec3};
use tessera_render::{
    Indices, Material, MeshConfig, MeshData, MeshDraw, MeshId, Pass, PreparedMesh, Renderer,
    Submesh,
};
use tessera_terrain::{
    EdgeLods, HeightSlopeShading, Lod, LodSelector, Neighbours, Side, TerrainBlock, TerrainMesh,
    TerrainTiles, VertexNormals,
};
use tessera_world::{LANDBLOCK_SIZE, LandblockBundle, LandblockKey};

use super::UploadBudget;
use super::jobs::JobRunner;
use super::view::{Anchor, Frustum};
use crate::smoke::linear_to_srgb8;

/// Edge of the overview tile in pixels (one per height sample).
pub const OVERVIEW_EDGE: u32 = 33;
/// Most mesh jobs queued per frame.
const MESH_JOBS_PER_FRAME: usize = 48;
/// Queue capacity of the terrain workers.
const QUEUE_CAPACITY: usize = 512;
/// First retry delay of a failed mesh.
pub const FIRST_BACKOFF: Duration = Duration::from_secs(1);
/// Longest retry delay of a failed mesh.
pub const MAX_BACKOFF: Duration = Duration::from_secs(30);

/// Resident terrain heights of one region, shared between the terrain layer (the only
/// writer) and its readers (the movement controllers, the sandbox's world source), so
/// there is exactly one copy.
#[derive(Clone, Default)]
pub struct SharedTiles(Arc<RwLock<RegionTiles>>);

#[derive(Default)]
struct RegionTiles {
    region: Option<u8>,
    tiles: TerrainTiles,
}

impl std::fmt::Debug for SharedTiles {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        let t = self.read();
        f.debug_struct("SharedTiles")
            .field("region", &t.region)
            .field("blocks", &t.tiles.len())
            .finish()
    }
}

impl SharedTiles {
    fn read(&self) -> std::sync::RwLockReadGuard<'_, RegionTiles> {
        self.0.read().unwrap_or_else(PoisonError::into_inner)
    }

    fn write(&self) -> std::sync::RwLockWriteGuard<'_, RegionTiles> {
        self.0.write().unwrap_or_else(PoisonError::into_inner)
    }

    /// Terrain height at region-global (`gx`, `gy`) of the region held, `None` where no
    /// resident block covers the point.
    #[must_use]
    pub fn height_at(&self, gx: f64, gy: f64) -> Option<f32> {
        self.read().tiles.height_at(gx, gy)
    }

    /// As [`SharedTiles::height_at`], `None` unless the tiles hold `region`.
    #[must_use]
    pub fn height_in(&self, region: u8, gx: f64, gy: f64) -> Option<f32> {
        let t = self.read();
        if t.region == Some(region) {
            t.tiles.height_at(gx, gy)
        } else {
            None
        }
    }

    /// The region held.
    #[must_use]
    pub fn region(&self) -> Option<u8> {
        self.read().region
    }

    /// Blocks held.
    #[must_use]
    pub fn len(&self) -> usize {
        self.read().tiles.len()
    }

    /// Whether no block is held.
    #[must_use]
    pub fn is_empty(&self) -> bool {
        self.len() == 0
    }

    /// Switches to `region`, dropping every block of another one.
    pub fn set_region(&self, region: u8) {
        let mut t = self.write();
        if t.region != Some(region) {
            t.tiles = TerrainTiles::new();
            t.region = Some(region);
        }
    }

    /// Adds `key`'s block when the tiles hold `key`'s region (see
    /// [`SharedTiles::set_region`]). Returns whether it was added.
    pub fn insert(&self, key: LandblockKey, block: Arc<TerrainBlock>) -> bool {
        let mut t = self.write();
        t.region == Some(key.region()) && t.tiles.insert(key.x(), key.y(), block).is_ok()
    }

    /// Drops `key`'s block.
    pub fn remove(&self, key: LandblockKey) {
        let mut t = self.write();
        if t.region == Some(key.region()) {
            t.tiles.remove(key.x(), key.y());
        }
    }

    /// Drops every block.
    pub fn clear(&self) {
        let mut t = self.write();
        t.tiles = TerrainTiles::new();
    }
}

/// One static object part placed in its landblock's frame.
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct Instance {
    /// The 0x06 part mesh.
    pub part: u32,
    /// Part (object) space → block-local frame: translation · rotation · scale
    /// (`entity-descriptor.md` §5).
    pub model: Mat4,
    /// Largest axis scale (for bounding spheres).
    pub scale: f32,
}

/// What the block job produces.
#[derive(Debug)]
pub struct BlockProduct {
    /// Terrain heights and LOD error bounds (computed here).
    pub block: Arc<TerrainBlock>,
    /// Height-shaded overview tile, RGBA8 sRGB, north row first.
    pub overview: Vec<u8>,
    /// Drawable, outdoor static-object parts.
    pub instances: Arc<[Instance]>,
}

/// The signature of a built terrain mesh: own level, effective edge levels and which edge
/// neighbours the normals saw.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
struct MeshSig {
    lod: Lod,
    edges: [Lod; 4],
    present: u8,
}

enum Job {
    Block {
        key: LandblockKey,
        generation: u64,
        bundle: Arc<LandblockBundle>,
        shading: Arc<HeightSlopeShading>,
        interiors: bool,
    },
    Mesh {
        key: LandblockKey,
        generation: u64,
        sig: MeshSig,
        block: Arc<TerrainBlock>,
        neighbours: [Option<Arc<TerrainBlock>>; 4],
        shading: Arc<HeightSlopeShading>,
        config: MeshConfig,
    },
}

enum Output {
    Block {
        key: LandblockKey,
        generation: u64,
        product: BlockProduct,
    },
    /// The block job failed (it panicked); the block gets no terrain.
    BlockFailed {
        key: LandblockKey,
        generation: u64,
        reason: String,
    },
    Mesh {
        key: LandblockKey,
        generation: u64,
        sig: MeshSig,
        mesh: Result<(PreparedMesh, f32, f32), String>,
    },
}

/// The last mesh attempt of a block failed.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
struct Failure {
    sig: MeshSig,
    retry_at: Instant,
    attempts: u32,
}

struct Slot {
    bundle: Arc<LandblockBundle>,
    generation: u64,
    block: Option<Arc<TerrainBlock>>,
    /// The block job failed: no terrain for this bundle.
    block_failed: bool,
    lod: Option<Lod>,
    /// The mesh drawn.
    mesh: Option<GpuTerrain>,
    /// An uploaded mesh waiting for its neighbours to agree on the shared edges.
    staged: Option<GpuTerrain>,
    pending: Option<MeshSig>,
    /// A finished mesh waiting for upload budget.
    ready: Option<(MeshSig, PreparedMesh, f32, f32)>,
    failed: Option<Failure>,
}

impl Slot {
    fn new(bundle: Arc<LandblockBundle>, generation: u64) -> Self {
        Self {
            bundle,
            generation,
            block: None,
            block_failed: false,
            lod: None,
            mesh: None,
            staged: None,
            pending: None,
            ready: None,
            failed: None,
        }
    }

    /// Whether a mesh job for `sig` should be queued at `now`.
    fn wants_mesh(&self, sig: MeshSig, now: Instant) -> bool {
        !(self.pending.is_some()
            || self.mesh.as_ref().is_some_and(|m| m.sig == sig)
            || self.staged.as_ref().is_some_and(|m| m.sig == sig)
            || self.ready.as_ref().is_some_and(|r| r.0 == sig)
            || self
                .failed
                .is_some_and(|f| f.sig == sig && now < f.retry_at))
    }

    /// Records a failed attempt for `sig`; returns the attempt number.
    fn fail(&mut self, sig: MeshSig, now: Instant) -> u32 {
        let attempts = self.failed.map_or(0, |f| f.attempts).saturating_add(1);
        let shift = attempts.saturating_sub(1).min(16);
        let backoff = FIRST_BACKOFF.saturating_mul(1u32 << shift).min(MAX_BACKOFF);
        self.failed = Some(Failure {
            sig,
            retry_at: now.checked_add(backoff).unwrap_or(now),
            attempts,
        });
        attempts
    }

    fn release(&mut self, renderer: &mut Renderer<'_>) {
        for m in [self.mesh.take(), self.staged.take()].into_iter().flatten() {
            renderer.destroy_mesh(m.id);
        }
    }
}

struct GpuTerrain {
    id: MeshId,
    sig: MeshSig,
    min_h: f32,
    max_h: f32,
    bytes: u64,
}

/// Counters for the HUD and the frame log.
#[derive(Debug, Clone, Copy, Default, PartialEq, Eq)]
pub struct TerrainStats {
    /// Landblocks tracked.
    pub blocks: usize,
    /// Blocks with a terrain block built.
    pub built: usize,
    /// Blocks with a mesh on the GPU.
    pub meshed: usize,
    /// Mesh jobs in flight, waiting for upload or staged for a neighbour.
    pub pending: usize,
    /// Blocks whose block job or latest mesh attempt failed.
    pub failed: usize,
    /// Terrain jobs that panicked so far.
    pub panics: u64,
    /// GPU bytes of terrain meshes (drawn and staged).
    pub gpu_bytes: u64,
    /// Meshes drawn last frame.
    pub drawn: usize,
    /// Triangles drawn last frame.
    pub triangles: u64,
}

/// The terrain layer (module docs).
pub struct TerrainLayer {
    runner: JobRunner<Job, Output>,
    slots: HashMap<LandblockKey, Slot>,
    tiles: SharedTiles,
    selector: LodSelector,
    shading: Arc<HeightSlopeShading>,
    mesh_config: MeshConfig,
    next_gen: u64,
    interiors: bool,
    stats: TerrainStats,
    panics: u64,
    /// Finished block products not yet taken by the walker.
    finished: Vec<(LandblockKey, BlockProduct)>,
    order: Vec<(f32, LandblockKey)>,
}

impl std::fmt::Debug for TerrainLayer {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("TerrainLayer")
            .field("stats", &self.stats)
            .finish_non_exhaustive()
    }
}

/// What the edge-consistency check needs of a block: the edge levels of the drawn and the
/// staged mesh, and whether its neighbours should ignore it (its meshing failed, so it
/// may never change).
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
struct EdgeView {
    shown: Option<[Lod; 4]>,
    staged: Option<[Lod; 4]>,
    frozen: bool,
}

/// The blocks whose staged mesh can be drawn this frame: the largest set such that, after
/// swapping them in, every pair of drawn neighbours meshed their shared edge at the same
/// level (`terrain.md` §4). Blocks without a drawn mesh constrain nothing; frozen blocks
/// are ignored by their neighbours.
fn promotable(views: &HashMap<LandblockKey, EdgeView>) -> HashSet<LandblockKey> {
    let mut set: HashSet<LandblockKey> = views
        .iter()
        .filter(|(_, v)| v.staged.is_some())
        .map(|(k, _)| *k)
        .collect();
    let mut work: Vec<LandblockKey> = set.iter().copied().collect();
    while let Some(key) = work.pop() {
        if !set.contains(&key) {
            continue;
        }
        let Some(edges) = views.get(&key).and_then(|v| v.staged) else {
            set.remove(&key);
            continue;
        };
        let agrees = Side::ALL.iter().all(|&side| {
            let (dx, dy) = side.offset();
            let Some(n) = key.offset(dx, dy) else {
                return true;
            };
            let Some(v) = views.get(&n).filter(|v| !v.frozen) else {
                return true;
            };
            let drawn = if set.contains(&n) { v.staged } else { v.shown };
            drawn.is_none_or(|e| e[side.opposite().index()] == edges[side.index()])
        });
        if !agrees {
            set.remove(&key);
            for side in Side::ALL {
                let (dx, dy) = side.offset();
                if let Some(n) = key.offset(dx, dy)
                    && set.contains(&n)
                {
                    work.push(n);
                }
            }
        }
    }
    set
}

/// Runs `f`, turning a panic into an error message.
fn guarded<T>(f: impl FnOnce() -> T) -> Result<T, String> {
    std::panic::catch_unwind(AssertUnwindSafe(f)).map_err(|p| {
        p.downcast_ref::<&str>()
            .map(|s| (*s).to_owned())
            .or_else(|| p.downcast_ref::<String>().cloned())
            .unwrap_or_else(|| "panicked".to_owned())
    })
}

impl TerrainLayer {
    /// A layer with `threads` workers.
    ///
    /// # Errors
    ///
    /// When the worker threads cannot be started.
    pub fn new(
        threads: usize,
        selector: LodSelector,
        mesh_config: MeshConfig,
        interiors: bool,
    ) -> std::io::Result<Self> {
        Ok(Self {
            runner: JobRunner::new("tessera-terrain", threads, QUEUE_CAPACITY, run_job)?,
            slots: HashMap::new(),
            tiles: SharedTiles::default(),
            selector,
            shading: Arc::new(HeightSlopeShading::default()),
            mesh_config,
            next_gen: 1,
            interiors,
            stats: TerrainStats::default(),
            panics: 0,
            finished: Vec::new(),
            order: Vec::new(),
        })
    }

    /// Updates the selector for a new viewport height or field of view.
    pub fn set_selector(&mut self, selector: LodSelector) {
        self.selector = selector;
    }

    /// The resident terrain blocks of the current region (region-global height queries),
    /// shared with other readers.
    #[must_use]
    pub fn tiles(&self) -> &SharedTiles {
        &self.tiles
    }

    /// Counters.
    #[must_use]
    pub fn stats(&self) -> TerrainStats {
        self.stats
    }

    /// Whether every tracked block is built and meshed with no job outstanding (blocks
    /// whose block job or current mesh failed count as settled).
    #[must_use]
    pub fn settled(&self) -> bool {
        self.slots.iter().all(|(key, s)| {
            if s.block_failed {
                return true;
            }
            if s.block.is_none() || s.pending.is_some() || s.ready.is_some() {
                return false;
            }
            if s.failed
                .is_some_and(|f| Some(f.sig) == self.signature(*key))
            {
                return true;
            }
            s.mesh.is_some() && s.staged.is_none()
        })
    }

    /// Block products finished since the last call (instances and overview tiles for the
    /// other layers).
    pub fn take_finished(&mut self) -> Vec<(LandblockKey, BlockProduct)> {
        std::mem::take(&mut self.finished)
    }

    /// Starts tracking `bundle` (new, or replacing the bundle tracked for `key`, whose
    /// meshes are freed). Returns false when the block job could not be queued (call
    /// again next frame; a replaced slot is then kept as it was).
    pub fn insert(
        &mut self,
        key: LandblockKey,
        bundle: &Arc<LandblockBundle>,
        renderer: &mut Renderer<'_>,
    ) -> bool {
        let generation = self.next_gen;
        let job = Job::Block {
            key,
            generation,
            bundle: Arc::clone(bundle),
            shading: Arc::clone(&self.shading),
            interiors: self.interiors,
        };
        if self.runner.submit(job).is_err() {
            return false;
        }
        self.tiles.set_region(key.region());
        self.next_gen += 1;
        if let Some(mut old) = self
            .slots
            .insert(key, Slot::new(Arc::clone(bundle), generation))
        {
            old.release(renderer);
            self.tiles.remove(key);
        }
        true
    }

    /// Whether `key` is tracked with exactly this bundle.
    #[must_use]
    pub fn tracks(&self, key: LandblockKey, bundle: &Arc<LandblockBundle>) -> bool {
        self.slots
            .get(&key)
            .is_some_and(|s| Arc::ptr_eq(&s.bundle, bundle))
    }

    /// Keys tracked.
    pub fn keys(&self) -> impl Iterator<Item = LandblockKey> + '_ {
        self.slots.keys().copied()
    }

    /// Stops tracking `key` and frees its meshes.
    pub fn remove(&mut self, key: LandblockKey, renderer: &mut Renderer<'_>) {
        if let Some(mut slot) = self.slots.remove(&key) {
            slot.release(renderer);
            self.tiles.remove(key);
        }
    }

    /// Frees everything.
    pub fn clear(&mut self, renderer: &mut Renderer<'_>) {
        for (_, mut slot) in self.slots.drain() {
            slot.release(renderer);
        }
        self.tiles.clear();
    }

    /// Per frame: collect finished jobs, choose levels, queue meshes, upload within
    /// `budget`, swap in staged meshes whose seams agree. `camera` is in the render frame
    /// of `anchor`.
    pub fn update(
        &mut self,
        renderer: &mut Renderer<'_>,
        anchor: &Anchor,
        camera: Vec3,
        budget: &mut UploadBudget,
    ) {
        let now = Instant::now();
        self.collect(now);

        // Levels, nearest first.
        self.order.clear();
        for (key, slot) in &mut self.slots {
            let Some(block) = &slot.block else { continue };
            let Some(off) = anchor.offset_of(*key) else {
                continue;
            };
            let local = camera - off;
            let d = block.distance_to(local.to_array());
            slot.lod = Some(self.selector.select(block, d, slot.lod));
            self.order.push((d, *key));
        }
        self.order
            .sort_unstable_by(|a, b| a.0.total_cmp(&b.0).then(a.1.cmp(&b.1)));

        // Mesh jobs for blocks whose signature changed.
        let mut queued = 0usize;
        let order = std::mem::take(&mut self.order);
        for &(_, key) in &order {
            if queued >= MESH_JOBS_PER_FRAME {
                break;
            }
            let Some(sig) = self.signature(key) else {
                continue;
            };
            let Some(slot) = self.slots.get_mut(&key) else {
                continue;
            };
            // A staged mesh made stale by a level that flipped back is not needed.
            if slot.mesh.as_ref().is_some_and(|m| m.sig == sig)
                && let Some(old) = slot.staged.take()
            {
                renderer.destroy_mesh(old.id);
            }
            if !slot.wants_mesh(sig, now) {
                continue;
            }
            let Some(block) = slot.block.clone() else {
                continue;
            };
            let generation = slot.generation;
            let neighbours = Side::ALL.map(|side| self.neighbour_block(key, side));
            let job = Job::Mesh {
                key,
                generation,
                sig,
                block,
                neighbours,
                shading: Arc::clone(&self.shading),
                config: self.mesh_config,
            };
            if self.runner.submit(job).is_err() {
                break;
            }
            if let Some(slot) = self.slots.get_mut(&key) {
                slot.pending = Some(sig);
            }
            queued += 1;
        }

        // Uploads, nearest first, into the staging slot.
        for &(_, key) in &order {
            let Some(slot) = self.slots.get_mut(&key) else {
                continue;
            };
            let Some((sig, prepared, min_h, max_h)) = slot.ready.take() else {
                continue;
            };
            let bytes = prepared.gpu_bytes();
            if !budget.try_take(bytes) {
                slot.ready = Some((sig, prepared, min_h, max_h));
                break;
            }
            match renderer.upload_prepared(prepared) {
                Ok(id) => {
                    slot.failed = None;
                    if let Some(old) = slot.staged.replace(GpuTerrain {
                        id,
                        sig,
                        min_h,
                        max_h,
                        bytes,
                    }) {
                        renderer.destroy_mesh(old.id);
                    }
                }
                Err(e) => {
                    let attempt = slot.fail(sig, now);
                    if attempt == 1 {
                        tracing::warn!(%key, "terrain upload failed (retrying later): {e}");
                    } else {
                        tracing::debug!(%key, attempt, "terrain upload failed again: {e}");
                    }
                }
            }
        }
        self.order = order;
        self.promote(renderer);
        self.refresh_stats();
    }

    /// Swaps in the staged meshes whose seams agree with what their neighbours draw.
    fn promote(&mut self, renderer: &mut Renderer<'_>) {
        if self.slots.values().all(|s| s.staged.is_none()) {
            return;
        }
        let views: HashMap<LandblockKey, EdgeView> = self
            .slots
            .iter()
            .map(|(k, s)| {
                (
                    *k,
                    EdgeView {
                        shown: s.mesh.as_ref().map(|m| m.sig.edges),
                        staged: s.staged.as_ref().map(|m| m.sig.edges),
                        frozen: s.block_failed || s.failed.is_some(),
                    },
                )
            })
            .collect();
        for key in promotable(&views) {
            if let Some(slot) = self.slots.get_mut(&key)
                && let Some(new) = slot.staged.take()
                && let Some(old) = slot.mesh.replace(new)
            {
                renderer.destroy_mesh(old.id);
            }
        }
    }

    fn neighbour_block(&self, key: LandblockKey, side: Side) -> Option<Arc<TerrainBlock>> {
        let (dx, dy) = side.offset();
        let n = key.offset(dx, dy)?;
        self.slots.get(&n)?.block.clone()
    }

    fn signature(&self, key: LandblockKey) -> Option<MeshSig> {
        let slot = self.slots.get(&key)?;
        let lod = slot.lod?;
        let mut edges = EdgeLods::NONE;
        let mut present = 0u8;
        for side in Side::ALL {
            let (dx, dy) = side.offset();
            let n = key.offset(dx, dy).and_then(|n| self.slots.get(&n));
            if let Some(n) = n
                && n.block.is_some()
            {
                present |= 1 << side.index();
                edges = edges.with(side, n.lod);
            }
        }
        Some(MeshSig {
            lod,
            edges: edges.effective(lod),
            present,
        })
    }

    fn collect(&mut self, now: Instant) {
        while let Some(out) = self.runner.try_next() {
            self.apply(out, now);
        }
        let panics = self.runner.panics();
        if panics > self.panics {
            tracing::error!(
                total = panics,
                "terrain jobs panicked outside the job guard"
            );
            self.panics = panics;
        }
    }

    /// Applies one worker result.
    fn apply(&mut self, out: Output, now: Instant) {
        match out {
            Output::Block {
                key,
                generation,
                product,
            } => {
                let Some(slot) = self.slots.get_mut(&key) else {
                    return;
                };
                if slot.generation != generation {
                    return;
                }
                slot.block = Some(Arc::clone(&product.block));
                self.tiles.insert(key, Arc::clone(&product.block));
                self.finished.push((key, product));
            }
            Output::BlockFailed {
                key,
                generation,
                reason,
            } => {
                self.panics += 1;
                let Some(slot) = self.slots.get_mut(&key) else {
                    return;
                };
                if slot.generation != generation {
                    return;
                }
                tracing::error!(%key, "terrain block failed; the block stays without terrain: {reason}");
                slot.block_failed = true;
            }
            Output::Mesh {
                key,
                generation,
                sig,
                mesh,
            } => {
                let Some(slot) = self.slots.get_mut(&key) else {
                    return;
                };
                if slot.generation != generation {
                    return;
                }
                if slot.pending == Some(sig) {
                    slot.pending = None;
                }
                match mesh {
                    Ok((p, lo, hi)) => slot.ready = Some((sig, p, lo, hi)),
                    Err(e) => {
                        let attempt = slot.fail(sig, now);
                        if attempt == 1 {
                            tracing::warn!(%key, "terrain mesh failed (retrying later): {e}");
                        } else {
                            tracing::debug!(%key, attempt, "terrain mesh failed again: {e}");
                        }
                    }
                }
            }
        }
    }

    fn refresh_stats(&mut self) {
        let mut s = TerrainStats {
            blocks: self.slots.len(),
            panics: self.panics,
            drawn: self.stats.drawn,
            triangles: self.stats.triangles,
            ..TerrainStats::default()
        };
        for slot in self.slots.values() {
            s.built += usize::from(slot.block.is_some());
            if let Some(m) = &slot.mesh {
                s.meshed += 1;
                s.gpu_bytes += m.bytes;
            }
            if let Some(m) = &slot.staged {
                s.gpu_bytes += m.bytes;
            }
            s.pending += usize::from(
                slot.pending.is_some() || slot.ready.is_some() || slot.staged.is_some(),
            );
            s.failed += usize::from(slot.block_failed || slot.failed.is_some());
        }
        self.stats = s;
    }

    /// Records the visible terrain meshes into `pass`.
    pub fn draw(
        &mut self,
        renderer: &Renderer<'_>,
        pass: &mut Pass,
        anchor: &Anchor,
        frustum: &Frustum,
    ) {
        let mut drawn = 0usize;
        let mut triangles = 0u64;
        let material = Material::color([1.0, 1.0, 1.0, 1.0]);
        let size = LANDBLOCK_SIZE as f32;
        for (key, slot) in &self.slots {
            let Some(m) = &slot.mesh else { continue };
            let Some(off) = anchor.offset_of(*key) else {
                continue;
            };
            let min = off + Vec3::new(0.0, 0.0, m.min_h);
            let max = off + Vec3::new(size, size, m.max_h);
            if !frustum.aabb_visible(min, max) {
                continue;
            }
            pass.mesh(MeshDraw::new(m.id, Mat4::from_translation(off), material));
            drawn += 1;
            triangles += renderer
                .mesh_info(m.id)
                .map_or(0, |i| u64::from(i.index_count / 3));
        }
        self.stats.drawn = drawn;
        self.stats.triangles = triangles;
    }

    /// Stops the workers.
    pub fn shutdown(&mut self) {
        self.runner.shutdown();
    }
}

fn run_job(job: Job) -> Output {
    match job {
        Job::Block {
            key,
            generation,
            bundle,
            shading,
            interiors,
        } => match guarded(|| build_block(&bundle, &shading, interiors)) {
            Ok(product) => Output::Block {
                key,
                generation,
                product,
            },
            Err(reason) => Output::BlockFailed {
                key,
                generation,
                reason,
            },
        },
        Job::Mesh {
            key,
            generation,
            sig,
            block,
            neighbours,
            shading,
            config,
        } => {
            let mesh = guarded(|| {
                let n = Neighbours(std::array::from_fn(|i| neighbours[i].as_deref()));
                let normals = VertexNormals::compute(&block, &n);
                let edges = EdgeLods(sig.edges.map(Some));
                let mesh = TerrainMesh::build(&block, &normals, &shading, sig.lod, edges);
                let (lo, hi) = (mesh.min_height, mesh.max_height);
                terrain_mesh_data(mesh, key)
                    .prepare(&config)
                    .map(|p| (p, lo, hi))
                    .map_err(|e| e.to_string())
            })
            .and_then(|r| r);
            Output::Mesh {
                key,
                generation,
                sig,
                mesh,
            }
        }
    }
}

/// The renderer streams of a terrain mesh (one submesh).
#[must_use]
pub fn terrain_mesh_data(mesh: TerrainMesh, key: LandblockKey) -> MeshData {
    let count = u32::try_from(mesh.indices.len()).unwrap_or(u32::MAX);
    MeshData {
        label: Some(format!("terrain {key}")),
        positions: mesh.positions,
        normals: Some(mesh.normals),
        tangents: None,
        uvs: Some(mesh.uvs),
        colors: Some(mesh.colors),
        indices: Indices::U16(mesh.indices),
        submeshes: vec![Submesh {
            first_index: 0,
            index_count: count,
        }],
    }
}

/// Builds the terrain block, the overview tile and the object instances of a bundle.
#[must_use]
pub fn build_block(
    bundle: &LandblockBundle,
    shading: &HeightSlopeShading,
    interiors: bool,
) -> BlockProduct {
    let block = TerrainBlock::from_landblock(&bundle.data);
    let _ = block.lod_errors();
    let overview = overview_tile(&block, shading);
    let mut instances = Vec::new();
    for e in bundle.drawable() {
        if e.interior && !interiors {
            continue;
        }
        let Some(visual) = &e.visual else { continue };
        let q = e.placement.rotation;
        let rot = Quat::from_xyzw(q.x, q.y, q.z, q.w);
        let rot = if rot.is_finite() && rot.length_squared() > 0.0 {
            rot.normalize()
        } else {
            Quat::IDENTITY
        };
        let scale = Vec3::from_array(e.scale);
        let scale = if scale.is_finite() && scale.min_element() > 0.0 {
            scale
        } else {
            Vec3::ONE
        };
        let pos = Vec3::from_array(e.placement.position);
        if !pos.is_finite() {
            continue;
        }
        let model = Mat4::from_scale_rotation_translation(scale, rot, pos);
        for &part in visual.parts.iter() {
            instances.push(Instance {
                part: part.0,
                model,
                scale: scale.max_element(),
            });
        }
    }
    BlockProduct {
        block: Arc::new(block),
        overview,
        instances: instances.into(),
    }
}

/// A 33 × 33 height/slope-shaded image of a block, north row first (the minimap
/// convention, `coordinates.md` §10), with a little directional shading so relief shows.
#[must_use]
pub fn overview_tile(block: &TerrainBlock, shading: &HeightSlopeShading) -> Vec<u8> {
    let normals = VertexNormals::compute(block, &Neighbours::NONE);
    let edge = OVERVIEW_EDGE as usize;
    let mut out = Vec::with_capacity(edge * edge * 4);
    let light = Vec3::new(-0.5, 0.5, 0.7).normalize();
    for row in 0..edge {
        let j = edge - 1 - row;
        for i in 0..edge {
            let h = block.height(i, j).unwrap_or(0.0);
            let n = normals.get(i, j).unwrap_or([0.0, 0.0, 1.0]);
            let c = shading.color(h, n[2]);
            let lit = (Vec3::from_array(n).dot(light) * 0.6 + 0.55).clamp(0.3, 1.2);
            out.extend_from_slice(&[
                linear_to_srgb8(c[0] * lit),
                linear_to_srgb8(c[1] * lit),
                linear_to_srgb8(c[2] * lit),
                255,
            ]);
        }
    }
    out
}

#[cfg(test)]
mod tests {
    use super::*;
    use tessera_formats::landblock::{HeightGrid, VertexFlags};

    #[test]
    fn overview_is_north_up_and_sized() {
        let block = TerrainBlock::new(
            &HeightGrid::from_fn(|_, j| (1000 + 2000 * j) as u16),
            &VertexFlags::default(),
        );
        let img = overview_tile(&block, &HeightSlopeShading::default());
        assert_eq!(img.len(), 33 * 33 * 4);
        assert!(img.as_chunks::<4>().0.iter().all(|p| p[3] == 255));
    }

    #[test]
    fn terrain_mesh_streams() {
        let block = TerrainBlock::flat(10_000);
        let normals = VertexNormals::compute(&block, &Neighbours::NONE);
        let mesh = TerrainMesh::build(
            &block,
            &normals,
            &HeightSlopeShading::default(),
            Lod::new(3).unwrap(),
            EdgeLods::NONE,
        );
        let key = LandblockKey::new(1, 2, 3).unwrap();
        let data = terrain_mesh_data(mesh, key);
        let p = data.prepare(&MeshConfig::default()).unwrap();
        assert_eq!(p.info().index_count as usize % 3, 0);
        assert!(p.info().index_count > 0);
    }

    fn lod(n: u8) -> Lod {
        Lod::new(n).unwrap()
    }

    fn key(x: u8, y: u8) -> LandblockKey {
        LandblockKey::new(1, x, y).unwrap()
    }

    fn view(shown: Option<[u8; 4]>, staged: Option<[u8; 4]>) -> EdgeView {
        EdgeView {
            shown: shown.map(|e| e.map(lod)),
            staged: staged.map(|e| e.map(lod)),
            frozen: false,
        }
    }

    // Edge order: south, east, north, west. (10, 10) and (11, 10) share A's east / B's west.
    #[test]
    fn staged_meshes_wait_for_their_neighbours() {
        let (a, b) = (key(10, 10), key(11, 10));
        // A's new mesh meshes the seam at level 2, B still draws a mesh built for level 1.
        let mut views = HashMap::from([
            (a, view(Some([1; 4]), Some([2, 2, 2, 2]))),
            (b, view(Some([1; 4]), None)),
        ]);
        assert!(promotable(&views).is_empty(), "A must wait for B");
        // B's matching mesh arrives: both swap in the same frame.
        views.insert(b, view(Some([1; 4]), Some([1, 1, 1, 2])));
        let p = promotable(&views);
        assert!(p.contains(&a) && p.contains(&b));
        // B's staged mesh disagrees with A's: neither may swap.
        views.insert(b, view(Some([1; 4]), Some([1, 1, 1, 3])));
        assert!(promotable(&views).is_empty());
        // A block that draws nothing does not hold anyone back.
        views.insert(b, view(None, None));
        assert_eq!(promotable(&views), HashSet::from([a]));
        // Nor does a frozen (failed) one.
        views.insert(
            b,
            EdgeView {
                frozen: true,
                ..view(Some([1; 4]), None)
            },
        );
        assert_eq!(promotable(&views), HashSet::from([a]));
    }

    #[test]
    fn promotion_chains_are_resolved() {
        // A row of three: C's staged mesh disagrees with D's drawn one, so C stays, and B
        // (which agrees only with C's staged mesh) must stay too; A only borders B's drawn
        // mesh, which it agrees with.
        let (a, b, c, d) = (key(1, 1), key(2, 1), key(3, 1), key(4, 1));
        let views = HashMap::from([
            (a, view(Some([0; 4]), Some([0, 0, 0, 0]))),
            (b, view(Some([0; 4]), Some([0, 1, 0, 0]))),
            (c, view(Some([0; 4]), Some([0, 1, 0, 1]))),
            (d, view(Some([0; 4]), None)),
        ]);
        assert_eq!(promotable(&views), HashSet::from([a]));
    }

    #[test]
    fn failures_back_off() {
        let bundle = Arc::new(bundle_at(key(5, 5), 0));
        let mut slot = Slot::new(bundle, 1);
        let sig = MeshSig {
            lod: lod(0),
            edges: [lod(0); 4],
            present: 0,
        };
        let now = Instant::now();
        assert!(slot.wants_mesh(sig, now));
        assert_eq!(slot.fail(sig, now), 1);
        assert!(!slot.wants_mesh(sig, now), "same signature: wait");
        assert!(
            slot.wants_mesh(sig, now + FIRST_BACKOFF),
            "retried after the back-off"
        );
        let other = MeshSig { lod: lod(1), ..sig };
        assert!(
            slot.wants_mesh(other, now),
            "a new signature is tried at once"
        );
        let mut last = now;
        for _ in 0..40 {
            slot.fail(sig, now);
            let at = slot.failed.unwrap().retry_at;
            assert!(at >= last);
            last = at;
        }
        assert!(last <= now + MAX_BACKOFF);
    }

    #[test]
    fn panics_become_errors() {
        assert_eq!(guarded(|| 3), Ok(3));
        let e = guarded(|| -> u32 { panic!("bad block") }).unwrap_err();
        assert!(e.contains("bad block"), "{e}");
    }

    fn bundle_at(key: LandblockKey, base: u16) -> LandblockBundle {
        LandblockBundle {
            key,
            data: tessera_formats::landblock::LandblockData::from_heights(
                key.data_did(),
                HeightGrid::from_fn(|i, j| base + (i * 37 + j * 11) as u16 * 20),
            ),
            data_stamp: None,
            info: None,
            block_map: None,
            block_props: tessera_world::BlockProps::default(),
            properties: None,
            minimap: None,
            light_map: None,
            ceiling_light_map: None,
            entities: Vec::new(),
            unplaced_entities: 0,
            area: None,
            problems: Vec::new(),
            heap_bytes: 0,
        }
    }

    fn headless() -> Option<Renderer<'static>> {
        match Renderer::new_headless(&tessera_render::RendererConfig::default()) {
            Ok(r) => Some(r),
            Err(e) => {
                eprintln!("skipping: no GPU: {e}");
                None
            }
        }
    }

    /// Runs `update` until settled (bounded).
    fn settle(layer: &mut TerrainLayer, renderer: &mut Renderer<'_>, anchor: &Anchor) {
        let t = Instant::now();
        loop {
            let mut budget = UploadBudget::new(u64::MAX, Duration::from_secs(1), Instant::now());
            layer.update(renderer, anchor, Vec3::new(80.0, 80.0, 300.0), &mut budget);
            if layer.settled() {
                return;
            }
            assert!(t.elapsed() < Duration::from_secs(60), "never settled");
            std::thread::sleep(Duration::from_millis(2));
        }
    }

    fn seams_agree(layer: &TerrainLayer) -> bool {
        layer.slots.iter().all(|(k, s)| {
            let Some(m) = &s.mesh else { return true };
            Side::ALL.iter().all(|&side| {
                let (dx, dy) = side.offset();
                let n = k.offset(dx, dy).and_then(|n| layer.slots.get(&n));
                n.and_then(|n| n.mesh.as_ref()).is_none_or(|nm| {
                    nm.sig.edges[side.opposite().index()] == m.sig.edges[side.index()]
                })
            })
        })
    }

    /// Insert, settle, replace, remove and switch regions with synthetic bundles on a real
    /// (headless) renderer: meshes are freed, heights follow, seams agree.
    #[test]
    fn layer_tracks_bundles() {
        let Some(mut renderer) = headless() else {
            return;
        };
        let mut layer = TerrainLayer::new(
            2,
            LodSelector::from_fov(720.0, 1.0, 1.5),
            MeshConfig::default(),
            false,
        )
        .unwrap();
        let anchor = Anchor {
            region: 1,
            x: 10,
            y: 10,
        };
        let keys: Vec<LandblockKey> = (8..13)
            .flat_map(|x| (8..13).map(move |y| key(x, y)))
            .collect();
        for k in &keys {
            assert!(layer.insert(*k, &Arc::new(bundle_at(*k, 4_000)), &mut renderer));
        }
        settle(&mut layer, &mut renderer, &anchor);
        assert_eq!(layer.stats().meshed, keys.len());
        assert_eq!(layer.take_finished().len(), keys.len());
        assert_eq!(renderer.mesh_stats().meshes as usize, keys.len());
        assert!(seams_agree(&layer));
        let tiles = layer.tiles().clone();
        assert_eq!(tiles.len(), keys.len());
        assert!(
            tiles
                .height_in(1, 10.0 * 160.0 + 5.0, 10.0 * 160.0 + 5.0)
                .is_some()
        );

        // Move the camera far away: levels change, seams still agree once settled.
        let far = Anchor {
            region: 1,
            x: 0,
            y: 0,
        };
        settle(&mut layer, &mut renderer, &far);
        assert!(seams_agree(&layer));
        assert_eq!(renderer.mesh_stats().meshes as usize, keys.len());

        // Replacing a bundle frees the old mesh at once.
        let k = key(10, 10);
        assert!(layer.insert(k, &Arc::new(bundle_at(k, 9_000)), &mut renderer));
        assert_eq!(renderer.mesh_stats().meshes as usize, keys.len() - 1);
        settle(&mut layer, &mut renderer, &anchor);
        assert_eq!(renderer.mesh_stats().meshes as usize, keys.len());

        // Removing frees the mesh and the heights.
        layer.remove(k, &mut renderer);
        assert_eq!(renderer.mesh_stats().meshes as usize, keys.len() - 1);
        assert!(
            tiles
                .height_in(1, 10.0 * 160.0 + 5.0, 10.0 * 160.0 + 5.0)
                .is_none()
        );
        assert_eq!(tiles.len(), keys.len() - 1);

        // A bundle of another region switches the shared heights to it.
        let other = LandblockKey::new(2, 10, 10).unwrap();
        assert!(layer.insert(other, &Arc::new(bundle_at(other, 4_000)), &mut renderer));
        assert_eq!(tiles.region(), Some(2));
        assert!(tiles.is_empty());
        layer.clear(&mut renderer);
        assert_eq!(renderer.mesh_stats().meshes, 0);
        layer.shutdown();
    }

    /// A failed block job leaves the block settled, without terrain.
    #[test]
    fn failed_block_counts_as_settled() {
        let Some(mut renderer) = headless() else {
            return;
        };
        let mut layer = TerrainLayer::new(
            1,
            LodSelector::from_fov(720.0, 1.0, 1.5),
            MeshConfig::default(),
            false,
        )
        .unwrap();
        let k = key(3, 3);
        assert!(layer.insert(k, &Arc::new(bundle_at(k, 100)), &mut renderer));
        let generation = layer.slots[&k].generation;
        // The worker's real result may arrive later; it is applied after this one and
        // would build the block, so stop the workers first.
        layer.shutdown();
        layer.apply(
            Output::BlockFailed {
                key: k,
                generation,
                reason: "synthetic".into(),
            },
            Instant::now(),
        );
        assert!(layer.settled());
        layer.refresh_stats();
        assert_eq!(layer.stats().failed, 1);
        assert_eq!(layer.stats().panics, 1);
        layer.clear(&mut renderer);
    }
}
