// SPDX-License-Identifier: GPL-3.0-or-later
//! The landblock streamer (ARCH §9.2, §20.3 step 4): rings around a focus, bundle loading
//! on the asset workers with priorities by distance, cancellation when leaving, a
//! residency budget with eviction, and lookups (area, music, ambient, sky, height,
//! minimap) over what is resident.
//!
//! # Per frame
//!
//! [`WorldStreamer::update`] runs on the main thread and never blocks on I/O or decoding:
//!
//! 0. a focus that is not finite or names no valid region is **ignored** (the previous
//!    focus is used and [`FrameReport::focus_rejected`] is set), so one bad frame from a
//!    controller does not flush the world; call [`WorldStreamer::clear`] to release
//!    everything on purpose;
//! 1. the [`RingTracker`] diffs the desired set against the previous one;
//! 2. leaving landblocks are released: in-flight requests are cancelled, resident bundles
//!    dropped;
//! 3. **admission**: desired landblocks are visited nearest first and admitted while the
//!    cumulative charge fits [`StreamConfig::residency_budget`]. A resident bundle is
//!    charged its [`LandblockBundle::heap_bytes`]; one loaded before (then evicted) its
//!    last real size; one never loaded the running mean ([`WorldStreamer::size_estimate`]).
//!    A near- or mid-ring landblock is admitted together with its *seam closure*: its
//!    desired east, north and north-east neighbours ([`LandblockKey::seam_neighbours`]),
//!    whose heights its terrain edges need (`landblock.md` §3.3, §8 step 5), and theirs in
//!    turn while they are near or mid — or not at all. The first closure that does not
//!    fit ends admission: everything farther is cancelled or evicted, so eviction is
//!    strictly farthest first. Re-admitting a landblock that was evicted needs a margin
//!    of 1/64 of the budget, so a block on the boundary does not flip every frame. When
//!    [`StreamConfig::always_admit_near`] is set, the near ring and the direct seam
//!    neighbours of its landblocks are admitted beyond the budget; a mid-ring neighbour
//!    admitted only that way gets its own neighbours when the budget allows. Admitted
//!    landblocks that are not loaded are requested with the priority of their ring
//!    ([`Ring::Near`] → `Immediate`, [`Ring::Mid`] → `Visible`, [`Ring::Far`] →
//!    `Prefetch`);
//! 4. finished requests are integrated, nearest first, until
//!    [`StreamConfig::max_integrations_per_frame`] or [`StreamConfig::frame_budget`]
//!    (measured from the start of integration) is reached; at least one finished request
//!    is integrated every frame, so streaming always makes progress. Integration only
//!    moves an `Arc`, so it costs microseconds. A data error or a panic in the job marks
//!    the landblock failed until it leaves the rings; a full queue, a shut-down pipeline,
//!    or a bundle job that failed because a record could not be read *now* (its
//!    [`WorldError::is_data_error`] is false) is retried with exponential backoff. A
//!    bundle that is not complete ([`LandblockBundle::is_complete`]: a read failed for a
//!    reason that is not the data's while scenery or collision was built) becomes
//!    resident, and is dropped from the asset store's cache so that it is rebuilt when the
//!    landblock loads again.
//!
//! Resident bytes can exceed the budget until the next update when a bundle turns out
//! larger than its estimate; that update evicts the farthest to compensate.
//!
//! The work is linear in the number of desired landblocks (a few hundred with the default
//! rings); when neither the focus nor the desired set changed, the previous nearest-first
//! order is reused. It is reported in [`FrameReport::main_thread`].
//!
//! # World systems (M4.5)
//!
//! Each bundle is built on a worker with its scenery, water, collision set, lights and LOD
//! classes ([`crate::bundle`] module docs), and its [`LandblockBundle::heap_bytes`]
//! includes them, so the residency budget accounts for collision sets and scenery. When a
//! bundle becomes resident its water joins the streamer's [`WaterMap`]; when it leaves,
//! the water leaves too. The lookups over what is resident:
//!
//! | Lookup | Spec |
//! |---|---|
//! | [`WorldStreamer::water_at`], [`WorldStreamer::is_under_water`], [`WorldStreamer::landblock_water`] | `water.md` §6.3, §7, §10 |
//! | [`WorldStreamer::sky_key_at`], [`WorldStreamer::sky_at`] | `environment.md` §7.1, §11 |
//! | [`WorldStreamer::lights_near`] | `lighting.md` §8.3 |
//! | [`WorldStreamer::lod_values`], [`WorldStreamer::draw_distance`] | `walker.md` §11.3–§11.4 |
//! | [`WorldStreamer::collision`], [`WorldStreamer::collision_view`], [`WorldStreamer::cell_at`] | `collision.md` §5.8.1, §12.1 |
//!
//! Cells are not confined to their landblock's square (the client's own bound is about
//! 2,756 units from the owning landblock's origin, `collision.md` §5.8.1 step 1), so the
//! streamer keeps the region-frame bounds of every resident landblock's cells
//! ([`BlockCollision::cell_bounds`]) and asks every landblock whose cells can contain a
//! point.
//!
//! # Frills
//!
//! The frill ring ([`FrillRing`], `scenery.md` §8.2) streams with the camera:
//! [`WorldStreamer::update_frills`] moves it to the blend of camera and player of
//! [`FrillSettings::player_position_blend`], drops the sectors that left, and generates
//! entering sectors nearest first on the same asset workers, once their landblock is
//! resident with its scenery inputs. At most [`StreamConfig::max_frill_jobs_per_frame`]
//! sectors start per frame and new ones wait while the held sectors exceed
//! [`StreamConfig::frill_budget`] bytes. Frills never collide (§8.1).

use std::collections::{HashMap, HashSet};
use std::sync::{Arc, Mutex, PoisonError};
use std::time::{Duration, Instant};

use tessera_assets::{
    AssetError, AssetState, AssetStore, CancelToken, Handle, Outcome, Priority, StoreConfig,
    StoreStats, TaskHandle, TaskStatus, WorkerPool,
};
use tessera_formats::Did;
use tessera_formats::landblock::MinimapTile;
use tessera_physics::controller::WaterSample;
use tessera_physics::geometry::{CELL_BROAD_PHASE_HALF, CollisionView};

use crate::areas::AreaName;
use crate::bundle::{BlockProps, BundleDecoder, EntityRef, LandblockBundle, PlacedEntity};
use crate::collision::{BlockCollision, CellPortals};
use crate::context::WorldContext;
use crate::coords::{LANDBLOCK_SIZE, LandblockKey, WorldPos};
use crate::draw_distance::{DistanceSettings, LodClasses, LodValues};
use crate::environment::{Evaluation, SkyLibrary, block_sky_key};
use crate::error::{Result, WorldError};
use crate::frills::{
    DensityThinner, FrillRing, FrillSector, FrillSectorKey, FrillSettings, generate_sector,
    ring_centre,
};
use crate::lighting::LightEntity;
use crate::mapcoords::MapCoords;
use crate::ring::{Ring, RingConfig, RingTracker};
use crate::scenery::{FixedWater, SceneryInput};
use crate::source::BundleSource;
use crate::water::{LandblockWater, WaterMap};

/// Streamer settings. The radii and budgets are user settings, not constants.
#[derive(Debug, Clone, PartialEq)]
pub struct StreamConfig {
    /// Ring radii and hysteresis.
    pub rings: RingConfig,
    /// Bytes of resident bundles (by [`LandblockBundle::heap_bytes`], which includes the
    /// world systems: collision sets, generated scenery, water). Default 1 GiB: with the
    /// world systems on, a bundle of the owner's install averages about 0.75 – 1.3 MiB,
    /// and the default rings hold about 380 landblocks.
    pub residency_budget: usize,
    /// Bytes the asset store keeps of recently released bundles, so a landblock that comes
    /// back soon is not decoded again. Counted separately from the residency budget.
    pub cache_budget: usize,
    /// Most finished requests integrated per frame.
    pub max_integrations_per_frame: usize,
    /// Main-thread time after which integration stops for this frame.
    pub frame_budget: Duration,
    /// Admit the near ring, and the east, north and north-east neighbours of its
    /// landblocks, even beyond the residency budget.
    pub always_admit_near: bool,
    /// Size assumed for a bundle before any has loaded.
    pub initial_size_estimate: usize,
    /// The frill engine's options (`scenery.md` §8.4); a radius of 0 (level 0 or
    /// `draw_frills` off) holds no sectors: that is how frills are turned off. Change them
    /// at run time with [`WorldStreamer::set_frill_settings`].
    pub frills: FrillSettings,
    /// Bytes of frill sectors held; beyond it no new sector starts until some leave.
    /// Positive ([`StreamConfig::validate`]).
    pub frill_budget: usize,
    /// Most frill sectors started per frame. Positive ([`StreamConfig::validate`]).
    pub max_frill_jobs_per_frame: usize,
}

impl StreamConfig {
    /// Checks the ring radii ([`RingConfig::validate`]) and that the residency budget,
    /// the integrations per frame, the frame budget, the frill budget and the frill jobs
    /// per frame are positive (frills are turned off through [`StreamConfig::frills`]).
    ///
    /// # Errors
    ///
    /// [`WorldError::Config`] naming the first violated rule.
    pub fn validate(&self) -> Result<()> {
        self.rings.validate()?;
        if self.residency_budget == 0 {
            return Err(WorldError::Config("residency budget must be positive"));
        }
        if self.max_integrations_per_frame == 0 {
            return Err(WorldError::Config(
                "integrations per frame must be positive",
            ));
        }
        if self.frame_budget.is_zero() {
            return Err(WorldError::Config("frame budget must be positive"));
        }
        if self.frill_budget == 0 {
            return Err(WorldError::Config(
                "frill budget must be positive (turn frills off in the frill settings)",
            ));
        }
        if self.max_frill_jobs_per_frame == 0 {
            return Err(WorldError::Config("frill jobs per frame must be positive"));
        }
        Ok(())
    }
}

impl Default for StreamConfig {
    fn default() -> Self {
        Self {
            rings: RingConfig::default(),
            residency_budget: 1 << 30,
            cache_budget: 64 << 20,
            max_integrations_per_frame: 32,
            frame_budget: Duration::from_millis(2),
            always_admit_near: true,
            initial_size_estimate: 512 << 10,
            frills: FrillSettings::default(),
            frill_budget: 64 << 20,
            max_frill_jobs_per_frame: 64,
        }
    }
}

/// Where a desired landblock stands.
#[derive(Debug, Clone)]
pub enum BlockState {
    /// Desired but not requested (over the residency budget, or waiting to retry after a
    /// full queue).
    Deferred,
    /// Requested; loading on a worker.
    Loading,
    /// Resident.
    Ready(Arc<LandblockBundle>),
    /// Could not be loaded (data error or a panic in the job; not retried while it stays
    /// desired).
    Failed(AssetError),
}

enum SlotState {
    Deferred,
    Requested(Handle<LandblockBundle>),
    Ready(Arc<LandblockBundle>),
    Failed(AssetError),
}

struct Slot {
    ring: Ring,
    distance: f64,
    state: SlotState,
    requested_at: Option<Instant>,
    /// Real size of the bundle when it was last resident (kept across eviction).
    known_size: Option<usize>,
    /// Earliest time of the next request after a transient failure.
    retry_after: Option<Instant>,
    /// Consecutive transient failures.
    retries: u32,
    /// Admission decision of the current update.
    admit: bool,
    /// Admission of the current update has covered this landblock's seam closure.
    done: bool,
}

impl Slot {
    fn new(ring: Ring, distance: f64) -> Self {
        Self {
            ring,
            distance,
            state: SlotState::Deferred,
            requested_at: None,
            known_size: None,
            retry_after: None,
            retries: 0,
            admit: false,
            done: false,
        }
    }

    /// The residency charge used by admission.
    fn charge(&self, estimate: usize) -> usize {
        match &self.state {
            SlotState::Ready(b) => b.heap_bytes,
            SlotState::Failed(_) => 0,
            SlotState::Deferred | SlotState::Requested(_) => self.known_size.unwrap_or(estimate),
        }
    }

    /// Whether admitting the slot means loading a bundle that was evicted before.
    fn is_readmission(&self) -> bool {
        matches!(self.state, SlotState::Deferred) && self.known_size.is_some()
    }
}

/// Fraction of the residency budget kept free before re-admitting an evicted bundle.
const READMIT_MARGIN_DIVISOR: usize = 64;
/// Delay before the first retry after a transient failure; doubled per further failure.
const RETRY_BASE: Duration = Duration::from_millis(10);
/// Longest retry delay.
const RETRY_MAX: Duration = Duration::from_secs(2);

/// What one [`WorldStreamer::update`] did.
#[derive(Debug, Clone, Default, PartialEq)]
pub struct FrameReport {
    /// Main-thread time spent in the update.
    pub main_thread: Duration,
    /// Landblocks that became desired.
    pub entered: usize,
    /// Landblocks released.
    pub left: usize,
    /// Landblocks whose ring changed.
    pub changed: usize,
    /// Requests issued.
    pub requested: usize,
    /// Bundles that became resident.
    pub integrated: usize,
    /// Resident bundles dropped for the budget.
    pub evicted: usize,
    /// In-flight requests cancelled (left the rings or over budget).
    pub cancelled: usize,
    /// Requests that failed for good (data error or panic).
    pub failed: usize,
    /// Requests that failed transiently (full queue, shutdown) and will be retried.
    pub retried: usize,
    /// The focus was not finite or named no valid region and was ignored.
    pub focus_rejected: bool,
    /// Resident landblocks after the update.
    pub resident_blocks: usize,
    /// Their bytes.
    pub resident_bytes: usize,
    /// Requests in flight.
    pub loading: usize,
    /// Desired landblocks held back by the budget or waiting to retry.
    pub deferred: usize,
    /// Of those, landblocks waiting to retry after a transient failure.
    pub waiting_retry: usize,
    /// Near-ring landblocks not resident yet (holes the camera might see).
    pub near_missing: usize,
    /// Of [`FrameReport::resident_bytes`], the bytes of resident collision sets.
    pub collision_bytes: usize,
    /// Generated scenery objects in resident bundles.
    pub generated_objects: usize,
    /// Resident landblocks whose scenery is incomplete (a read failed for a reason that is
    /// not the data's; the bundle is dropped from the store's cache when it becomes
    /// resident, so it is rebuilt when the landblock loads again).
    pub incomplete_scenery: usize,
    /// Resident landblocks whose collision set is incomplete (same cause and remedy).
    pub incomplete_collision: usize,
}

/// What one [`WorldStreamer::update_frills`] did.
#[derive(Debug, Clone, Default, PartialEq)]
pub struct FrillReport {
    /// Main-thread time spent.
    pub main_thread: Duration,
    /// Sectors that entered the ring.
    pub entered: usize,
    /// Sectors that left it.
    pub left: usize,
    /// Sector jobs started.
    pub started: usize,
    /// Sectors whose job finished this frame.
    pub finished: usize,
    /// Sectors held and ready.
    pub ready: usize,
    /// Sector jobs running.
    pub running: usize,
    /// Sectors waiting (landblock not resident, budget, or the per-frame limit).
    pub waiting: usize,
    /// Bytes of ready sectors.
    pub bytes: usize,
    /// Frill instances in ready sectors.
    pub instances: usize,
}

/// Cumulative counters of a [`WorldStreamer`].
#[derive(Debug, Clone, Default, PartialEq)]
pub struct StreamStats {
    /// Updates run.
    pub frames: u64,
    /// Longest update.
    pub max_frame: Duration,
    /// Sum of update times.
    pub total_frame: Duration,
    /// Requests issued.
    pub requested: u64,
    /// Bundles integrated.
    pub integrated: u64,
    /// Bundles dropped for the budget.
    pub evicted: u64,
    /// Bundles released when leaving the rings.
    pub released: u64,
    /// Requests cancelled.
    pub cancelled: u64,
    /// Requests failed for good.
    pub failed: u64,
    /// Transient failures that were retried.
    pub retried: u64,
    /// Updates whose focus was rejected.
    pub rejected_focus: u64,
    /// Highest resident bytes after an update.
    pub peak_resident_bytes: usize,
    /// Highest resident landblock count after an update.
    pub peak_resident_blocks: usize,
    /// Highest resident bytes plus the asset store's cache bytes.
    pub peak_total_bytes: usize,
    /// Completed bundle jobs seen.
    pub jobs: u64,
    /// Sum of job read times.
    pub read_time: Duration,
    /// Sum of job decode times.
    pub decode_time: Duration,
    /// Longest job (read + decode).
    pub max_job: Duration,
    /// Updates that ended with near-ring holes.
    pub frames_with_near_missing: u64,
    /// Sum of request-to-resident times of integrated bundles.
    pub load_latency: Duration,
    /// Longest request-to-resident time.
    pub max_load_latency: Duration,
    /// Highest bytes of resident collision sets after an update.
    pub peak_collision_bytes: usize,
    /// Frill sectors generated.
    pub frill_sectors: u64,
    /// Highest bytes of frill sectors held.
    pub peak_frill_bytes: usize,
}

impl StreamStats {
    /// Mean update time.
    #[must_use]
    pub fn mean_frame(&self) -> Duration {
        u32::try_from(self.frames)
            .ok()
            .filter(|&n| n > 0)
            .map_or(Duration::ZERO, |n| self.total_frame / n)
    }

    /// Mean request-to-resident time.
    #[must_use]
    pub fn mean_load_latency(&self) -> Duration {
        u32::try_from(self.integrated)
            .ok()
            .filter(|&n| n > 0)
            .map_or(Duration::ZERO, |n| self.load_latency / n)
    }

    /// Mean job time (read + decode).
    #[must_use]
    pub fn mean_job(&self) -> Duration {
        u32::try_from(self.jobs)
            .ok()
            .filter(|&n| n > 0)
            .map_or(Duration::ZERO, |n| (self.read_time + self.decode_time) / n)
    }
}

/// A minimap tile of a resident landblock (`coordinates.md` §10, `landblock.md` §6.3).
#[derive(Debug, Clone, PartialEq)]
pub struct MinimapRef {
    /// The landblock.
    pub key: LandblockKey,
    /// The 0x41 image for the active content layers.
    pub image: Did,
    /// The tile's georeference.
    pub tile: MinimapTile,
}

/// A light of a resident landblock ([`WorldStreamer::lights_near`]).
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct NearbyLight<'a> {
    /// The landblock that places it.
    pub key: LandblockKey,
    /// Its position in the region frame.
    pub position: WorldPos,
    /// The light.
    pub light: &'a LightEntity,
}

/// A frill sector's state.
enum FrillState {
    /// Waiting for its landblock, the budget or a job slot.
    Waiting,
    /// Generating on a worker; the result arrives in the cell.
    Running(TaskHandle, Arc<Mutex<Option<FrillSector>>>),
    /// Generated (possibly empty).
    Ready(Arc<FrillSector>, usize),
}

/// The streamer (module docs).
pub struct WorldStreamer {
    ctx: Arc<WorldContext>,
    store: AssetStore<BundleSource, BundleDecoder>,
    pool: WorkerPool,
    rings: RingTracker,
    config: StreamConfig,
    slots: HashMap<LandblockKey, Slot>,
    focus: Option<WorldPos>,
    resident_bytes: usize,
    loaded_sizes: (u64, u128),
    stats: StreamStats,
    /// Desired landblocks nearest first, valid for `order_focus`.
    order: Vec<(f64, LandblockKey)>,
    order_focus: Option<[f64; 2]>,
    /// Water of the resident bundles.
    water: WaterMap,
    frill_ring: FrillRing,
    frills: HashMap<FrillSectorKey, FrillState>,
    frill_bytes: usize,
    /// Region-frame bounds (min, max) of the cells of resident landblocks that have
    /// placed cells, grown by the lookup's broad-phase margin.
    cell_bounds: HashMap<LandblockKey, ([f64; 3], [f64; 3])>,
}

/// Adds the water and cell bounds of a bundle that became resident to the lookups.
fn track_resident(
    water: &mut WaterMap,
    cell_bounds: &mut HashMap<LandblockKey, ([f64; 3], [f64; 3])>,
    b: &Arc<LandblockBundle>,
) {
    if let Some(w) = &b.water {
        water.insert(Arc::clone(w));
    }
    if let Some(bounds) = region_cell_bounds(b) {
        cell_bounds.insert(b.key, bounds);
    }
}

/// The region-frame bounds of `b`'s placed cells, grown by [`CELL_BROAD_PHASE_HALF`].
fn region_cell_bounds(b: &LandblockBundle) -> Option<([f64; 3], [f64; 3])> {
    let bb = b.collision.as_ref()?.cell_bounds?;
    let o = b.key.origin();
    let o = [o[0], o[1], 0.0];
    let m = f64::from(CELL_BROAD_PHASE_HALF);
    Some((
        std::array::from_fn(|i| o[i] + f64::from(bb.min[i]) - m),
        std::array::from_fn(|i| o[i] + f64::from(bb.max[i]) + m),
    ))
}

impl std::fmt::Debug for WorldStreamer {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("WorldStreamer")
            .field("focus", &self.focus)
            .field("slots", &self.slots.len())
            .field("resident_bytes", &self.resident_bytes)
            .field("stats", &self.stats)
            .finish_non_exhaustive()
    }
}

fn priority(ring: Ring) -> Priority {
    match ring {
        Ring::Near => Priority::Immediate,
        Ring::Mid => Priority::Visible,
        Ring::Far => Priority::Prefetch,
    }
}

impl WorldStreamer {
    /// A streamer whose bundle jobs run on `pool`.
    ///
    /// # Errors
    ///
    /// [`crate::WorldError::Config`] when [`StreamConfig::validate`] rejects `config`.
    pub fn new(ctx: Arc<WorldContext>, pool: &WorkerPool, config: StreamConfig) -> Result<Self> {
        config.validate()?;
        let rings = RingTracker::new(config.rings)?;
        let store = AssetStore::new(
            pool,
            BundleSource::new(Arc::clone(ctx.source())),
            BundleDecoder::new(Arc::clone(&ctx)),
            StoreConfig {
                cache_budget: config.cache_budget,
                max_completions: 4_096,
                max_completion_bytes: config.residency_budget.max(64 << 20),
                ..StoreConfig::default()
            },
        );
        let frill_ring = FrillRing::new(&config.frills);
        Ok(Self {
            ctx,
            store,
            pool: pool.clone(),
            rings,
            config,
            slots: HashMap::new(),
            focus: None,
            resident_bytes: 0,
            loaded_sizes: (0, 0),
            stats: StreamStats::default(),
            order: Vec::new(),
            order_focus: None,
            water: WaterMap::new(),
            frill_ring,
            frills: HashMap::new(),
            frill_bytes: 0,
            cell_bounds: HashMap::new(),
        })
    }

    /// The context.
    #[must_use]
    pub fn context(&self) -> &Arc<WorldContext> {
        &self.ctx
    }

    /// The configuration.
    #[must_use]
    pub fn config(&self) -> &StreamConfig {
        &self.config
    }

    /// The ring tracker: the desired landblocks and their rings.
    #[must_use]
    pub fn rings(&self) -> &RingTracker {
        &self.rings
    }

    /// The bundle store (statistics, cache).
    #[must_use]
    pub fn store(&self) -> &AssetStore<BundleSource, BundleDecoder> {
        &self.store
    }

    /// The bundle store's counters.
    #[must_use]
    pub fn store_stats(&self) -> StoreStats {
        self.store.stats()
    }

    /// Cumulative counters.
    #[must_use]
    pub fn stats(&self) -> &StreamStats {
        &self.stats
    }

    /// The last focus.
    #[must_use]
    pub fn focus(&self) -> Option<WorldPos> {
        self.focus
    }

    /// Bytes of resident bundles.
    #[must_use]
    pub fn resident_bytes(&self) -> usize {
        self.resident_bytes
    }

    /// The size assumed for a bundle that has not loaded: the mean of those loaded so far.
    #[must_use]
    pub fn size_estimate(&self) -> usize {
        let (n, sum) = self.loaded_sizes;
        if n == 0 {
            self.config.initial_size_estimate
        } else {
            usize::try_from(sum / u128::from(n)).unwrap_or(usize::MAX)
        }
    }

    /// Moves the focus and streams (module docs). Call once per frame.
    ///
    /// A focus that is not finite or names no valid region is ignored: the previous focus
    /// is used instead (nothing happens before the first valid one), a warning is logged
    /// (rate-limited) and [`FrameReport::focus_rejected`] is set.
    pub fn update(&mut self, focus: WorldPos) -> FrameReport {
        let t0 = Instant::now();
        let mut report = FrameReport::default();
        let valid = focus.is_finite() && LandblockKey::new(focus.region, 0, 0).is_ok();
        let focus = if valid {
            focus
        } else {
            report.focus_rejected = true;
            self.stats.rejected_focus += 1;
            if self.stats.rejected_focus.is_power_of_two() {
                tracing::warn!(
                    region = focus.region,
                    x = focus.x,
                    y = focus.y,
                    z = focus.z,
                    count = self.stats.rejected_focus,
                    "streamer focus rejected (not finite or no valid region); keeping the previous focus"
                );
            }
            match self.focus {
                Some(previous) => previous,
                None => {
                    self.finish(t0, &mut report);
                    return report;
                }
            }
        };
        let diff = self.rings.update(&focus);
        self.focus = Some(focus);
        report.entered = diff.entered.len();
        report.left = diff.left.len();
        report.changed = diff.changed.len();

        for key in &diff.left {
            if let Some(slot) = self.slots.remove(key) {
                self.release(slot.state, &mut report);
                self.stats.released += 1;
            }
        }
        for &(key, _, ring) in &diff.changed {
            if let Some(slot) = self.slots.get_mut(&key) {
                slot.ring = ring;
                if let SlotState::Requested(h) = &slot.state {
                    h.set_priority(priority(ring));
                }
            }
        }
        for e in &diff.entered {
            self.slots.insert(e.key, Slot::new(e.ring, e.distance));
        }

        // Nearest first; reused while neither the focus nor the desired set changed.
        let g = focus.xy();
        let mut order = std::mem::take(&mut self.order);
        if !diff.is_empty() || self.order_focus != Some(g) {
            order.clear();
            order.extend(self.slots.iter_mut().map(|(k, s)| {
                s.distance = k.distance_to(g);
                (s.distance, *k)
            }));
            order.sort_unstable_by(|a, b| a.0.total_cmp(&b.0).then(a.1.cmp(&b.1)));
            self.order_focus = Some(g);
        }

        self.admit(&order, &mut report);
        self.poll_jobs();
        self.integrate(&order, &mut report);
        self.order = order;
        self.finish(t0, &mut report);
        report
    }

    /// Fills the summary counts of `report` and the statistics.
    fn finish(&mut self, t0: Instant, report: &mut FrameReport) {
        let now = Instant::now();
        for slot in self.slots.values() {
            match &slot.state {
                SlotState::Deferred => {
                    report.deferred += 1;
                    if slot.retry_after.is_some_and(|t| t > now) {
                        report.waiting_retry += 1;
                    }
                }
                SlotState::Requested(_) => report.loading += 1,
                SlotState::Ready(b) => {
                    report.resident_blocks += 1;
                    report.collision_bytes += b.collision.as_ref().map_or(0, |c| c.heap_bytes);
                    report.generated_objects += b.generated.len();
                    if b.scenery.as_ref().is_some_and(|s| !s.complete) {
                        report.incomplete_scenery += 1;
                    }
                    if b.collision.as_ref().is_some_and(|c| !c.complete) {
                        report.incomplete_collision += 1;
                    }
                }
                SlotState::Failed(_) => {}
            }
            if slot.ring == Ring::Near
                && !matches!(slot.state, SlotState::Ready(_) | SlotState::Failed(_))
            {
                report.near_missing += 1;
            }
        }
        report.resident_bytes = self.resident_bytes;
        report.main_thread = t0.elapsed();

        let s = &mut self.stats;
        s.frames += 1;
        s.total_frame += report.main_thread;
        s.max_frame = s.max_frame.max(report.main_thread);
        s.peak_resident_bytes = s.peak_resident_bytes.max(self.resident_bytes);
        s.peak_resident_blocks = s.peak_resident_blocks.max(report.resident_blocks);
        s.peak_collision_bytes = s.peak_collision_bytes.max(report.collision_bytes);
        if report.near_missing > 0 {
            s.frames_with_near_missing += 1;
        }
    }

    /// Records the asset store's cache bytes into the peak-total statistic. Separate from
    /// [`WorldStreamer::update`] because reading store statistics takes the store lock;
    /// call it when reporting.
    pub fn sample_memory(&mut self) -> usize {
        let total = self.resident_bytes + self.store.stats().cache_bytes;
        self.stats.peak_total_bytes = self.stats.peak_total_bytes.max(total);
        total
    }

    fn release(&mut self, state: SlotState, report: &mut FrameReport) {
        match state {
            SlotState::Requested(h) => {
                if !h.is_finished() {
                    h.cancel();
                    report.cancelled += 1;
                    self.stats.cancelled += 1;
                }
            }
            SlotState::Ready(b) => {
                self.resident_bytes = self.resident_bytes.saturating_sub(b.heap_bytes);
                self.water.remove(b.key);
                self.cell_bounds.remove(&b.key);
            }
            SlotState::Deferred | SlotState::Failed(_) => {}
        }
    }

    /// Admission (module docs, step 3): decides the admitted set nearest first, then
    /// requests, cancels and evicts.
    fn admit(&mut self, order: &[(f64, LandblockKey)], report: &mut FrameReport) {
        let estimate = self.size_estimate();
        let budget = self.config.residency_budget;
        let margin = budget / READMIT_MARGIN_DIVISOR;
        let always_near = self.config.always_admit_near;
        for slot in self.slots.values_mut() {
            slot.admit = false;
            slot.done = false;
        }

        // Pass 1: the admitted set.
        //
        // Under the budget a landblock is admitted with its seam closure: itself and the
        // desired landblocks reachable through east / north / north-east steps that start
        // in the near or mid ring, minus those whose closure is already admitted (`done`).
        // Every member of an admitted closure is `done`: its own seam neighbours are in.
        //
        // A forced near-ring landblock is admitted with its direct seam neighbours only,
        // beyond the budget; a mid-ring neighbour admitted that way ("donor") is not `done`
        // and gets its own closure later in the walk if the budget allows.
        let mut cum: usize = 0;
        let mut cut = false;
        let mut group: Vec<LandblockKey> = Vec::with_capacity(8);
        let mut stack: Vec<LandblockKey> = Vec::with_capacity(8);
        let mut seen: HashSet<LandblockKey> = HashSet::new();
        for &(_, key) in order {
            let Some(slot) = self.slots.get(&key) else {
                continue;
            };
            if slot.done {
                continue;
            }
            let forced = always_near && slot.ring == Ring::Near;
            if forced {
                let direct = key.seam_neighbours();
                for k in std::iter::once(key).chain(direct.into_iter().flatten()) {
                    if let Some(s) = self.slots.get_mut(&k)
                        && !s.admit
                    {
                        cum = cum.saturating_add(s.charge(estimate));
                        s.admit = true;
                    }
                }
                if let Some(s) = self.slots.get_mut(&key) {
                    s.done = true;
                }
                continue;
            }
            if cut {
                continue;
            }
            group.clear();
            seen.clear();
            stack.clear();
            stack.push(key);
            while let Some(k) = stack.pop() {
                let Some(s) = self.slots.get(&k) else {
                    continue;
                };
                if s.done || !seen.insert(k) {
                    continue;
                }
                group.push(k);
                if s.ring != Ring::Far {
                    stack.extend(k.seam_neighbours().into_iter().flatten());
                }
            }
            let mut charge = 0usize;
            let mut readmission = false;
            for k in &group {
                if let Some(s) = self.slots.get(k)
                    && !s.admit
                {
                    charge = charge.saturating_add(s.charge(estimate));
                    readmission |= s.is_readmission();
                }
            }
            let limit = if readmission {
                budget.saturating_sub(margin)
            } else {
                budget
            };
            if cum.saturating_add(charge) <= limit {
                cum = cum.saturating_add(charge);
                for k in &group {
                    if let Some(s) = self.slots.get_mut(k) {
                        s.admit = true;
                        s.done = true;
                    }
                }
            } else {
                cut = true;
            }
        }

        // Pass 2: apply.
        let now = Instant::now();
        for &(_, key) in order {
            let Some(slot) = self.slots.get_mut(&key) else {
                continue;
            };
            if slot.admit {
                let waiting = slot.retry_after.is_some_and(|t| t > now);
                if matches!(slot.state, SlotState::Deferred) && !waiting {
                    let h = self.store.request(key.data_did().0, priority(slot.ring));
                    slot.state = SlotState::Requested(h);
                    slot.requested_at = Some(now);
                    report.requested += 1;
                    self.stats.requested += 1;
                }
                continue;
            }
            match std::mem::replace(&mut slot.state, SlotState::Deferred) {
                SlotState::Requested(h) => {
                    if !h.is_finished() {
                        h.cancel();
                        report.cancelled += 1;
                        self.stats.cancelled += 1;
                    }
                }
                SlotState::Ready(b) => {
                    slot.known_size = Some(b.heap_bytes);
                    self.resident_bytes = self.resident_bytes.saturating_sub(b.heap_bytes);
                    self.water.remove(b.key);
                    self.cell_bounds.remove(&b.key);
                    report.evicted += 1;
                    self.stats.evicted += 1;
                }
                SlotState::Failed(e) => slot.state = SlotState::Failed(e),
                SlotState::Deferred => {}
            }
        }
    }

    /// Drains the store's completion queue for job timings.
    fn poll_jobs(&mut self) {
        for c in self.store.poll_completions(256) {
            if matches!(c.outcome, Outcome::Cancelled) {
                continue;
            }
            let s = &mut self.stats;
            s.jobs += 1;
            s.read_time += c.read_time;
            s.decode_time += c.decode_time;
            s.max_job = s.max_job.max(c.read_time + c.decode_time);
        }
    }

    /// Integration (module docs, step 4).
    fn integrate(&mut self, order: &[(f64, LandblockKey)], report: &mut FrameReport) {
        let start = Instant::now();
        let max = self.config.max_integrations_per_frame.max(1);
        for &(_, key) in order {
            let done = report.integrated + report.failed;
            if done >= max || (done > 0 && start.elapsed() > self.config.frame_budget) {
                break;
            }
            let Some(slot) = self.slots.get_mut(&key) else {
                continue;
            };
            let SlotState::Requested(h) = &slot.state else {
                continue;
            };
            match h.state() {
                AssetState::Ready(b) => {
                    self.resident_bytes = self.resident_bytes.saturating_add(b.heap_bytes);
                    track_resident(&mut self.water, &mut self.cell_bounds, &b);
                    if !b.is_complete() {
                        // Not the data's result: forget it (cache, weak reference), so the
                        // next load of this landblock builds it again.
                        tracing::debug!(landblock = %key, "incomplete bundle resident; rebuilt on the next load");
                        self.store.invalidate(key.data_did().0);
                    }
                    if slot.known_size.is_none() {
                        // Each landblock counts once per stay in the rings, so reloading an
                        // evicted bundle does not skew the mean.
                        self.loaded_sizes.0 += 1;
                        self.loaded_sizes.1 += b.heap_bytes as u128;
                    }
                    slot.known_size = Some(b.heap_bytes);
                    slot.retries = 0;
                    slot.retry_after = None;
                    slot.state = SlotState::Ready(b);
                    report.integrated += 1;
                    self.stats.integrated += 1;
                    if let Some(t) = slot.requested_at.take() {
                        let l = t.elapsed();
                        self.stats.load_latency += l;
                        self.stats.max_load_latency = self.stats.max_load_latency.max(l);
                    }
                }
                AssetState::Failed(e) => {
                    if is_transient(&e) {
                        // Pipeline state, or a record that could not be read now: retry
                        // with exponential backoff. The store remembers decode failures,
                        // so forget this one first.
                        self.store.invalidate(key.data_did().0);
                        slot.retries = slot.retries.saturating_add(1);
                        let delay = RETRY_BASE
                            .saturating_mul(1u32 << slot.retries.min(16))
                            .min(RETRY_MAX);
                        slot.retry_after = Some(Instant::now() + delay);
                        slot.state = SlotState::Deferred;
                        report.retried += 1;
                        self.stats.retried += 1;
                    } else {
                        // Data errors recur; so does a panic on the same record. Not
                        // retried while the landblock stays desired.
                        tracing::warn!(landblock = %key, error = %e, "landblock bundle failed");
                        slot.state = SlotState::Failed(e);
                        report.failed += 1;
                        self.stats.failed += 1;
                    }
                }
                AssetState::Cancelled => slot.state = SlotState::Deferred,
                AssetState::Queued | AssetState::Loading => {}
            }
        }
    }

    /// Makes a hand-built bundle resident in the near ring, as integration would (tests of
    /// the lookups without records for every part).
    #[cfg(test)]
    pub(crate) fn insert_resident(&mut self, b: LandblockBundle) {
        let b = Arc::new(b);
        if let Some(old) = self.slots.remove(&b.key) {
            let mut report = FrameReport::default();
            self.release(old.state, &mut report);
        }
        self.resident_bytes = self.resident_bytes.saturating_add(b.heap_bytes);
        track_resident(&mut self.water, &mut self.cell_bounds, &b);
        let mut slot = Slot::new(Ring::Near, 0.0);
        slot.known_size = Some(b.heap_bytes);
        slot.state = SlotState::Ready(Arc::clone(&b));
        self.slots.insert(b.key, slot);
    }

    /// Releases everything (for example on teleport to another region before the next
    /// focus is known).
    pub fn clear(&mut self) {
        let mut report = FrameReport::default();
        let _ = self.rings.clear();
        for (_, slot) in std::mem::take(&mut self.slots) {
            self.release(slot.state, &mut report);
        }
        self.resident_bytes = 0;
        self.focus = None;
        self.order.clear();
        self.order_focus = None;
        self.water = WaterMap::new();
        self.cell_bounds.clear();
        self.clear_frills();
    }

    /// Repeats [`WorldStreamer::update`] at `focus` until an update changes nothing
    /// (nothing loading or waiting to retry, nothing requested, integrated or evicted) or
    /// `timeout` passes (loading screens, tools, tests). Returns whether it settled.
    /// A timeout too large for an [`Instant`] (such as `Duration::MAX`) means no deadline.
    pub fn settle(&mut self, focus: WorldPos, timeout: Duration) -> bool {
        let deadline = Instant::now().checked_add(timeout);
        loop {
            let r = self.update(focus);
            if r.loading == 0
                && r.waiting_retry == 0
                && r.requested == 0
                && r.integrated == 0
                && r.evicted == 0
            {
                return true;
            }
            if deadline.is_some_and(|d| Instant::now() >= d) {
                return false;
            }
            std::thread::sleep(Duration::from_millis(1));
        }
    }

    /// The state of a desired landblock; `None` when it is not desired.
    #[must_use]
    pub fn state_of(&self, key: LandblockKey) -> Option<BlockState> {
        self.slots.get(&key).map(|s| match &s.state {
            SlotState::Deferred => BlockState::Deferred,
            SlotState::Requested(_) => BlockState::Loading,
            SlotState::Ready(b) => BlockState::Ready(Arc::clone(b)),
            SlotState::Failed(e) => BlockState::Failed(e.clone()),
        })
    }

    /// The ring of a desired landblock.
    #[must_use]
    pub fn ring_of(&self, key: LandblockKey) -> Option<Ring> {
        self.slots.get(&key).map(|s| s.ring)
    }

    /// A resident bundle.
    #[must_use]
    pub fn bundle(&self, key: LandblockKey) -> Option<&Arc<LandblockBundle>> {
        match &self.slots.get(&key)?.state {
            SlotState::Ready(b) => Some(b),
            _ => None,
        }
    }

    /// The resident bundles of the east, north and north-east neighbours of `key`
    /// ([`LandblockKey::seam_neighbours`]), whose edge heights the terrain mesh of `key`
    /// shares. After [`WorldStreamer::settle`] the desired ones are resident for every
    /// resident near-ring landblock, and for every resident mid-ring landblock unless the
    /// residency budget is binding and the block is resident only as a neighbour of the
    /// always-admitted near ring; while streaming, a neighbour can arrive a frame or more
    /// after the block itself. Consumers handle a missing neighbour (one-sided edge
    /// normals, remesh when it arrives).
    #[must_use]
    pub fn seam_neighbours(&self, key: LandblockKey) -> [Option<&Arc<LandblockBundle>>; 3] {
        key.seam_neighbours()
            .map(|n| n.and_then(|n| self.bundle(n)))
    }

    /// The resident bundle of the landblock containing `pos`.
    #[must_use]
    pub fn bundle_at(&self, pos: &WorldPos) -> Option<&Arc<LandblockBundle>> {
        self.bundle(pos.landblock()?)
    }

    /// Every resident bundle with its ring, in no particular order.
    pub fn resident(&self) -> impl Iterator<Item = (LandblockKey, Ring, &Arc<LandblockBundle>)> {
        self.slots.iter().filter_map(|(k, s)| match &s.state {
            SlotState::Ready(b) => Some((*k, s.ring, b)),
            _ => None,
        })
    }

    /// Block-map properties (area, music, ambient, sky) at `pos`, if its landblock is
    /// resident.
    #[must_use]
    pub fn block_props_at(&self, pos: &WorldPos) -> Option<&BlockProps> {
        self.bundle_at(pos).map(|b| &b.block_props)
    }

    /// The area name at `pos`, if its landblock is resident and the name resolved.
    #[must_use]
    pub fn area_at(&self, pos: &WorldPos) -> Option<&Arc<AreaName>> {
        self.bundle_at(pos)?.area.as_ref()
    }

    /// Terrain height at `pos` (`landblock.md` §3.11), if its landblock is resident.
    #[must_use]
    pub fn terrain_height_at(&self, pos: &WorldPos) -> Option<f32> {
        let l = pos.to_local()?;
        self.bundle(l.block)?.terrain_height(l.local[0], l.local[1])
    }

    /// Map coordinates of `pos` with its region's constants (`coordinates.md` §8).
    #[must_use]
    pub fn map_coords(&self, pos: &WorldPos) -> Option<MapCoords> {
        self.ctx
            .map_constants(pos.region)
            .map(|c| c.to_map(pos.xy()))
    }

    /// Minimap tiles of resident landblocks within `radius` landblocks (chessboard
    /// distance) of `center`, choosing variants by `active` content layers. Sorted by
    /// key.
    #[must_use]
    pub fn minimap_tiles(
        &self,
        center: LandblockKey,
        radius: u32,
        active: impl Fn(u32) -> bool,
    ) -> Vec<MinimapRef> {
        let mut out: Vec<MinimapRef> = self
            .resident()
            .filter(|(k, _, _)| k.chebyshev(center).is_some_and(|d| d <= radius))
            .filter_map(|(key, _, b)| {
                let tile = b.minimap.clone()?;
                Some(MinimapRef {
                    key,
                    image: tile.image_for(&active),
                    tile,
                })
            })
            .collect();
        out.sort_unstable_by_key(|m| m.key);
        out
    }

    // -----------------------------------------------------------------------------------
    // World-system lookups (module docs)
    // -----------------------------------------------------------------------------------

    /// The water of the resident landblocks.
    #[must_use]
    pub fn water_map(&self) -> &WaterMap {
        &self.water
    }

    /// The water at `pos` (`water.md` §6.3, §7): the top of the liquid column through it,
    /// over the liquid boxes of its landblock and its eight neighbours; `None` when `pos` is
    /// not in liquid or its landblock is not resident.
    #[must_use]
    pub fn water_at(&self, pos: &WorldPos) -> Option<WaterSample> {
        self.water.water_at(pos)
    }

    /// The client's "is this position under water" (`water.md` §6.3, §10) over what is
    /// resident.
    #[must_use]
    pub fn is_under_water(&self, pos: &WorldPos) -> bool {
        self.water.is_under_water(pos)
    }

    /// The landblock water (`water.md` §6.2) at `pos`, when its landblock is resident and
    /// wet.
    #[must_use]
    pub fn landblock_water(&self, pos: &WorldPos) -> Option<LandblockWater> {
        self.water.landblock_water(pos)
    }

    /// The sky library (sky description, game time, day descriptions), when loaded.
    #[must_use]
    pub fn sky(&self) -> Option<&Arc<SkyLibrary>> {
        self.ctx.systems().sky.as_ref()
    }

    /// The sky key the block map of `pos`'s landblock selects (`environment.md` §7.1), when
    /// the landblock is resident and names one.
    #[must_use]
    pub fn sky_key_at(&self, pos: &WorldPos) -> Option<u32> {
        block_sky_key(
            self.block_props_at(pos)?,
            &self.ctx.systems().pids.environment,
        )
    }

    /// The sky at `pos` for day type `day_type` and clock value `g` (`environment.md`
    /// §11, without cross-fade): [`SkyLibrary::evaluate`] with the sky key of
    /// [`WorldStreamer::sky_key_at`] (the default sky when there is none). `None` without
    /// a sky library. A pure function of the records and its inputs.
    #[must_use]
    pub fn sky_at(&self, pos: &WorldPos, day_type: u32, g: f64) -> Option<Evaluation> {
        Some(self.sky()?.evaluate(self.sky_key_at(pos), day_type, g))
    }

    /// The Light entities of resident landblocks (`lighting.md` §8.3) whose position lies
    /// within `radius` units of `pos` (3-D distance), nearest first (ties by landblock and
    /// entity index). Landblocks within ⌈`radius` / 160⌉ + 1 of `pos`'s (at most 8) are
    /// searched.
    #[must_use]
    pub fn lights_near(&self, pos: &WorldPos, radius: f64) -> Vec<NearbyLight<'_>> {
        let (Some(centre), true) = (pos.landblock(), pos.is_finite() && radius >= 0.0) else {
            return Vec::new();
        };
        #[allow(clippy::cast_possible_truncation)]
        let n = ((radius / LANDBLOCK_SIZE).ceil().min(7.0) as i32) + 1;
        let mut out: Vec<(f64, NearbyLight<'_>)> = Vec::new();
        for dy in -n..=n {
            for dx in -n..=n {
                let Some(key) = centre.offset(dx, dy) else {
                    continue;
                };
                let Some(b) = self.bundle(key) else { continue };
                let o = key.origin();
                for light in &b.lights {
                    let p = WorldPos::new(
                        pos.region,
                        o[0] + f64::from(light.position[0]),
                        o[1] + f64::from(light.position[1]),
                        f64::from(light.position[2]),
                    );
                    let d = ((p.x - pos.x).powi(2) + (p.y - pos.y).powi(2) + (p.z - pos.z).powi(2))
                        .sqrt();
                    if d <= radius {
                        out.push((
                            d,
                            NearbyLight {
                                key,
                                position: p,
                                light,
                            },
                        ));
                    }
                }
            }
        }
        out.sort_by(|a, b| {
            a.0.total_cmp(&b.0)
                .then(a.1.key.cmp(&b.1.key))
                .then(a.1.light.index.cmp(&b.1.light.index))
        });
        out.into_iter().map(|(_, l)| l).collect()
    }

    /// The LOD-class table, when loaded.
    #[must_use]
    pub fn lod_classes(&self) -> Option<&Arc<LodClasses>> {
        self.ctx.systems().lod_classes.as_ref()
    }

    /// The eight LOD-class values of an entity or generated object `e` of `bundle`
    /// (`walker.md` §11.4.3): its class from the bundle, looked up in the LOD-class table
    /// (the render component's initial values without a table or class). `e` is found by
    /// address ([`LandblockBundle::lod_class_of`]); with a copy of an entity use
    /// [`WorldStreamer::lod_values_of`].
    #[must_use]
    pub fn lod_values(
        &self,
        bundle: &LandblockBundle,
        e: &PlacedEntity,
        is_player: bool,
    ) -> LodValues {
        let class = bundle.lod_class_of(e);
        match self.lod_classes() {
            Some(t) => t.values(class, is_player),
            None => LodClasses::default().values(class, is_player),
        }
    }

    /// [`WorldStreamer::lod_values`] of entity or generated object `r` of `bundle`, by its
    /// stable [`EntityRef`] (works on clones and copies of the bundle).
    #[must_use]
    pub fn lod_values_of(
        &self,
        bundle: &LandblockBundle,
        r: EntityRef,
        is_player: bool,
    ) -> LodValues {
        let class = bundle.lod_class(r);
        match self.lod_classes() {
            Some(t) => t.values(class, is_player),
            None => LodClasses::default().values(class, is_player),
        }
    }

    /// The draw distance of `e` of `bundle` (`walker.md` §11.4 step 1) under the global
    /// limit `g` and `settings`.
    #[must_use]
    pub fn draw_distance(
        &self,
        bundle: &LandblockBundle,
        e: &PlacedEntity,
        g: f32,
        settings: &DistanceSettings,
    ) -> f32 {
        self.lod_values(bundle, e, false).draw_distance(g, settings)
    }

    /// The collision of a resident landblock.
    #[must_use]
    pub fn collision(&self, key: LandblockKey) -> Option<&Arc<BlockCollision>> {
        self.bundle(key)?.collision.as_ref()
    }

    /// A collision view (`collision.md` §12.1, decision 0013 "Positions") over the
    /// resident collision sets within `radius` landblocks (chessboard distance, at most 8)
    /// of `centre`'s landblock, plus every resident landblock whose cells
    /// ([`BlockCollision::cell_bounds`]) reach into that square: interiors owned by
    /// landblocks several blocks away (`collision.md` §5.8.1) bring their physics meshes.
    /// Its frame is the region frame shifted so that `centre`'s landblock's south-west
    /// corner (z = 0) is at 0: view = region − origin ([`CollisionView::to_view`]), region
    /// = view + origin ([`CollisionView::to_region`]). Empty when `centre` is off the grid.
    #[must_use]
    pub fn collision_view(&self, centre: &WorldPos, radius: u32) -> CollisionView<'_> {
        let Some(c) = centre.landblock() else {
            return CollisionView::new();
        };
        let o = c.origin();
        let mut view = CollisionView::with_origin([o[0], o[1], 0.0]);
        let r = i32::try_from(radius.min(8)).unwrap_or(8);
        let mut members: Vec<LandblockKey> = Vec::new();
        for dy in -r..=r {
            for dx in -r..=r {
                if let Some(k) = c.offset(dx, dy)
                    && self.collision(k).is_some()
                {
                    members.push(k);
                }
            }
        }
        // The square in the region frame.
        let side = LANDBLOCK_SIZE;
        let lo = [o[0] - f64::from(r) * side, o[1] - f64::from(r) * side];
        let hi = [
            o[0] + f64::from(r + 1) * side,
            o[1] + f64::from(r + 1) * side,
        ];
        let mut extra: Vec<LandblockKey> = self
            .cell_bounds
            .iter()
            .filter(|(k, (bmin, bmax))| {
                k.region() == c.region()
                    && k.chebyshev(c).is_none_or(|d| d > r.unsigned_abs())
                    && bmin[0] <= hi[0]
                    && bmax[0] >= lo[0]
                    && bmin[1] <= hi[1]
                    && bmax[1] >= lo[1]
            })
            .map(|(k, _)| *k)
            .collect();
        extra.sort_unstable();
        members.extend(extra);
        for k in members {
            if let Some(col) = self.collision(k)
                && let Err(e) = view.push_landblock(&col.set)
            {
                tracing::debug!(landblock = %col.key, error = %e, "collision set not added to the view");
            }
        }
        view
    }

    /// The interior cell containing `pos` (`collision.md` §5.8.1), or `None` (outdoors).
    /// Every resident landblock whose cell bounds ([`BlockCollision::cell_bounds`]) contain
    /// `pos` is asked, wherever it lies (cells are not confined to their landblock's square;
    /// the lookup's own bound of §5.8.1 step 1 applies in each owner's frame): `pos`'s own
    /// landblock first, then by chessboard distance, then by key. Within one landblock the
    /// lowest cell number wins (§5.8.1 Decision).
    #[must_use]
    pub fn cell_at(&self, pos: &WorldPos) -> Option<(LandblockKey, &CellPortals)> {
        if !pos.is_finite() {
            return None;
        }
        let own = pos.landblock();
        let p = [pos.x, pos.y, pos.z];
        let mut candidates: Vec<(bool, u32, LandblockKey)> = self
            .cell_bounds
            .iter()
            .filter(|(k, (bmin, bmax))| {
                k.region() == pos.region && (0..3).all(|i| bmin[i] <= p[i] && p[i] <= bmax[i])
            })
            .map(|(k, _)| {
                let d = own.and_then(|o| o.chebyshev(*k)).unwrap_or(u32::MAX);
                (Some(*k) != own, d, *k)
            })
            .collect();
        candidates.sort_unstable();
        for (_, _, key) in candidates {
            let Some(col) = self.collision(key) else {
                continue;
            };
            let o = key.origin();
            #[allow(clippy::cast_possible_truncation)]
            let local = [(p[0] - o[0]) as f32, (p[1] - o[1]) as f32, p[2] as f32];
            if let Some(c) = col.cell_at(local) {
                return Some((key, c));
            }
        }
        None
    }

    /// Replaces the frill settings ([`StreamConfig::frills`]) at run time: every frill
    /// sector is dropped and the frill ring rebuilt with `settings` (the next
    /// [`WorldStreamer::update_frills`] streams it again); resident landblocks are kept.
    pub fn set_frill_settings(&mut self, settings: FrillSettings) {
        self.config.frills = settings;
        self.clear_frills();
    }

    // -----------------------------------------------------------------------------------
    // Frills (module docs)
    // -----------------------------------------------------------------------------------

    /// The frill ring.
    #[must_use]
    pub fn frill_ring(&self) -> &FrillRing {
        &self.frill_ring
    }

    /// A generated frill sector, if held and ready.
    #[must_use]
    pub fn frill_sector(&self, key: FrillSectorKey) -> Option<&Arc<FrillSector>> {
        match self.frills.get(&key)? {
            FrillState::Ready(s, _) => Some(s),
            _ => None,
        }
    }

    /// Every ready frill sector, in no particular order.
    pub fn frill_sectors(&self) -> impl Iterator<Item = (FrillSectorKey, &Arc<FrillSector>)> {
        self.frills.iter().filter_map(|(k, st)| match st {
            FrillState::Ready(s, _) => Some((*k, s)),
            _ => None,
        })
    }

    /// Bytes of the ready frill sectors.
    #[must_use]
    pub fn frill_bytes(&self) -> usize {
        self.frill_bytes
    }

    /// Moves the frill ring to the blend of `camera` and `player`
    /// ([`FrillSettings::player_position_blend`], `scenery.md` §8.2) and streams its
    /// sectors (module docs). Call once per frame after [`WorldStreamer::update`]; never
    /// blocks on a worker.
    pub fn update_frills(&mut self, camera: &WorldPos, player: &WorldPos) -> FrillReport {
        let t0 = Instant::now();
        let mut report = FrillReport::default();
        let centre = ring_centre(camera, player, self.config.frills.player_position_blend);
        let diff = if centre.is_finite() {
            self.frill_ring.update(&centre)
        } else {
            crate::frills::FrillRingDiff::default()
        };
        report.entered = diff.entered.len();
        report.left = diff.left.len();
        for k in &diff.left {
            if let Some(st) = self.frills.remove(k) {
                self.drop_frill(st);
            }
        }
        for k in &diff.entered {
            self.frills.entry(*k).or_insert(FrillState::Waiting);
        }

        // Finished jobs.
        for st in self.frills.values_mut() {
            let FrillState::Running(h, cell) = st else {
                continue;
            };
            let status = h.status();
            if !status.is_finished() {
                continue;
            }
            let result = cell.lock().unwrap_or_else(PoisonError::into_inner).take();
            match (result, status) {
                (Some(sector), _) => {
                    let bytes = sector_bytes(&sector);
                    self.frill_bytes = self.frill_bytes.saturating_add(bytes);
                    *st = FrillState::Ready(Arc::new(sector), bytes);
                    report.finished += 1;
                    self.stats.frill_sectors += 1;
                }
                // A panic in the job: an empty sector, not retried while held.
                (None, TaskStatus::Failed(e)) if !matches!(e, AssetError::Shutdown) => {
                    tracing::warn!(error = %e, "frill sector job failed");
                    *st = FrillState::Ready(Arc::new(FrillSector::default()), 0);
                }
                // Cancelled, shut down or full: try again.
                (None, _) => *st = FrillState::Waiting,
            }
        }

        // Start waiting sectors nearest first.
        if let Some(c) = self.frill_ring.centre() {
            let dist2 = |k: &FrillSectorKey| {
                let dx = i64::from(k.gx) - i64::from(c.gx);
                let dy = i64::from(k.gy) - i64::from(c.gy);
                dx * dx + dy * dy
            };
            let mut waiting: Vec<(i64, FrillSectorKey)> = self
                .frills
                .iter()
                .filter(|(_, st)| matches!(st, FrillState::Waiting))
                .map(|(k, _)| (dist2(k), *k))
                .collect();
            waiting.sort_unstable();
            for (_, k) in waiting {
                if report.started >= self.config.max_frill_jobs_per_frame
                    || self.frill_bytes >= self.config.frill_budget
                {
                    break;
                }
                let Some(bundle) = k.landblock().and_then(|lb| self.bundle(lb)) else {
                    continue;
                };
                let next = if bundle.scenery.is_none() {
                    // No scenery for this landblock (no tables, or generation off): an
                    // empty sector.
                    FrillState::Ready(
                        Arc::new(FrillSector {
                            key: Some(k),
                            ..FrillSector::default()
                        }),
                        0,
                    )
                } else {
                    let cell = Arc::new(Mutex::new(None));
                    let out = Arc::clone(&cell);
                    let ctx = Arc::clone(&self.ctx);
                    let bundle = Arc::clone(bundle);
                    let density = self.config.frills.density();
                    let (sx, sy) = k.local();
                    let spawned =
                        self.pool
                            .spawn(Priority::Visible, CancelToken::new(), move |job| {
                                if job.is_cancelled() {
                                    return;
                                }
                                let mut thinner = DensityThinner::new();
                                let sector = build_frill_sector(
                                    &ctx,
                                    &bundle,
                                    sx,
                                    sy,
                                    density,
                                    &mut thinner,
                                );
                                *out.lock().unwrap_or_else(PoisonError::into_inner) = Some(sector);
                            });
                    match spawned {
                        Ok(h) => {
                            report.started += 1;
                            FrillState::Running(h, cell)
                        }
                        Err(e) => {
                            tracing::debug!(error = %e, "frill sector job not started; retried");
                            break;
                        }
                    }
                };
                self.frills.insert(k, next);
            }
        }
        for st in self.frills.values() {
            match st {
                FrillState::Waiting => report.waiting += 1,
                FrillState::Running(..) => report.running += 1,
                FrillState::Ready(s, _) => {
                    report.ready += 1;
                    report.instances += s.instances.len();
                }
            }
        }
        report.bytes = self.frill_bytes;
        self.stats.peak_frill_bytes = self.stats.peak_frill_bytes.max(self.frill_bytes);
        report.main_thread = t0.elapsed();
        report
    }

    fn drop_frill(&mut self, st: FrillState) {
        match st {
            FrillState::Running(h, _) => h.cancel(),
            FrillState::Ready(_, bytes) => {
                self.frill_bytes = self.frill_bytes.saturating_sub(bytes);
            }
            FrillState::Waiting => {}
        }
    }

    fn clear_frills(&mut self) {
        for (_, st) in std::mem::take(&mut self.frills) {
            self.drop_frill(st);
        }
        self.frill_ring = FrillRing::new(&self.config.frills);
        self.frill_bytes = 0;
    }
}

/// Generates frill sector (`sx`, `sy`) of `bundle`'s landblock (`scenery.md` §8.2) from
/// the scenery inputs the bundle kept ([`crate::BlockScenery`]) and the context's tables
/// and scene records; `thinner` is the density test's random source. An empty sector when
/// the bundle has no scenery inputs or the context no tables. Runs on a worker in
/// [`WorldStreamer::update_frills`]; public for tools and tests.
#[must_use]
pub fn build_frill_sector(
    ctx: &WorldContext,
    bundle: &LandblockBundle,
    sx: u8,
    sy: u8,
    density: f32,
    thinner: &mut DensityThinner,
) -> FrillSector {
    let empty = || FrillSector {
        key: FrillSectorKey::new(bundle.key, sx, sy),
        ..FrillSector::default()
    };
    let Some(sc) = &bundle.scenery else {
        return empty();
    };
    let tables = ctx
        .region(bundle.key.region())
        .and_then(|r| r.scenery.as_ref());
    let (Some(tables), Some(scenes)) = (tables, ctx.systems().scenes.as_ref()) else {
        return empty();
    };
    let water = FixedWater(sc.water);
    let input = SceneryInput {
        key: bundle.key,
        data: &bundle.data,
        terrain: &sc.terrain,
        tables,
        scenes: &**scenes,
        water: &water,
        statics: &sc.statics,
        limits: ctx.options().scenery_limits,
    };
    generate_sector(&input, sx, sy, density, thinner)
}

/// Heap bytes of a frill sector.
fn sector_bytes(s: &FrillSector) -> usize {
    std::mem::size_of::<FrillSector>()
        + s.instances.capacity() * std::mem::size_of::<crate::frills::FrillInstance>()
}

/// Whether a failed bundle job may succeed when tried again: the pipeline's state (full
/// queue, shutdown) or a decoder error that is not the data's (a record that could not be
/// read now, [`WorldError::is_data_error`]).
fn is_transient(e: &AssetError) -> bool {
    match e {
        AssetError::QueueFull { .. } | AssetError::Shutdown => true,
        AssetError::Decode { source, .. } => source
            .downcast_ref::<WorldError>()
            .is_some_and(|w| !w.is_data_error()),
        _ => false,
    }
}
