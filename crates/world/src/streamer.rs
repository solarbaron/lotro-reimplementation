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
//!    the landblock failed until it leaves the rings; a full queue or a shut-down pipeline
//!    is retried with exponential backoff.
//!
//! Resident bytes can exceed the budget until the next update when a bundle turns out
//! larger than its estimate; that update evicts the farthest to compensate.
//!
//! The work is linear in the number of desired landblocks (a few hundred with the default
//! rings); when neither the focus nor the desired set changed, the previous nearest-first
//! order is reused. It is reported in [`FrameReport::main_thread`].

use std::collections::{HashMap, HashSet};
use std::sync::Arc;
use std::time::{Duration, Instant};

use tessera_assets::{
    AssetError, AssetState, AssetStore, Handle, Outcome, Priority, StoreConfig, StoreStats,
    WorkerPool,
};
use tessera_formats::Did;
use tessera_formats::landblock::MinimapTile;

use crate::areas::AreaName;
use crate::bundle::{BlockProps, BundleDecoder, LandblockBundle};
use crate::context::WorldContext;
use crate::coords::{LandblockKey, WorldPos};
use crate::error::{Result, WorldError};
use crate::mapcoords::MapCoords;
use crate::ring::{Ring, RingConfig, RingTracker};
use crate::source::BundleSource;

/// Streamer settings. The radii and budgets are user settings, not constants.
#[derive(Debug, Clone, PartialEq)]
pub struct StreamConfig {
    /// Ring radii and hysteresis.
    pub rings: RingConfig,
    /// Bytes of resident bundles (by [`LandblockBundle::heap_bytes`]).
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
}

impl StreamConfig {
    /// Checks the ring radii ([`RingConfig::validate`]) and that the residency budget,
    /// the integrations per frame and the frame budget are positive.
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
        Ok(())
    }
}

impl Default for StreamConfig {
    fn default() -> Self {
        Self {
            rings: RingConfig::default(),
            residency_budget: 512 << 20,
            cache_budget: 64 << 20,
            max_integrations_per_frame: 32,
            frame_budget: Duration::from_millis(2),
            always_admit_near: true,
            initial_size_estimate: 512 << 10,
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

/// The streamer (module docs).
pub struct WorldStreamer {
    ctx: Arc<WorldContext>,
    store: AssetStore<BundleSource, BundleDecoder>,
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
        Ok(Self {
            ctx,
            store,
            rings,
            config,
            slots: HashMap::new(),
            focus: None,
            resident_bytes: 0,
            loaded_sizes: (0, 0),
            stats: StreamStats::default(),
            order: Vec::new(),
            order_focus: None,
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
                SlotState::Ready(_) => report.resident_blocks += 1,
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
                    if matches!(e, AssetError::QueueFull { .. } | AssetError::Shutdown) {
                        // Pipeline state: retry with exponential backoff.
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
    }

    /// Repeats [`WorldStreamer::update`] at `focus` until an update changes nothing
    /// (nothing loading or waiting to retry, nothing requested, integrated or evicted) or
    /// `timeout` passes (loading screens, tools, tests). Returns whether it settled.
    pub fn settle(&mut self, focus: WorldPos, timeout: Duration) -> bool {
        let deadline = Instant::now() + timeout;
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
            if Instant::now() >= deadline {
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
}
