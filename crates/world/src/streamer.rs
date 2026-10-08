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
//!    whose heights its terrain edges need (`landblock.md` §3.3, §8 step 5, §8.1), and
//!    theirs in turn while they are near or mid — or not at all. The west, south and
//!    diagonal neighbours that the normals of `landblock.md` §3.14 also read are not part
//!    of the closure (OPEN in `landblock.md` §8.1, 2026-10-07). The first closure that does not
//!    fit ends admission: everything farther is cancelled or evicted, so eviction is
//!    strictly farthest first. Re-admitting a landblock that was evicted needs a margin
//!    of 1/64 of the budget, so a block on the boundary does not flip every frame; a seam
//!    closure with an evicted member needs the margin as a whole, its never-loaded members
//!    included (Tessera's reading, `landblock.md` §8.1 OPEN on closures). When
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
//!    the landblock failed until it leaves the rings (a failed landblock holds nothing and
//!    is charged nothing in admission); a request refused by the pipeline (full queue,
//!    shut-down pipeline) or a job that ended for a reason that is not the data's (its
//!    [`WorldError::is_data_error`] is false) is retried after 10 ms, doubled for each
//!    further consecutive failure up to 2 s. A bundle
//!    that is not complete ([`LandblockBundle::is_complete`]: a world-system input could
//!    not be read for a reason that is not the data's) becomes resident, and is dropped
//!    from the asset store's cache so that it is rebuilt when the landblock loads again.
//!    These failure rules are Tessera's (`landblock.md` §8.1 OPEN of 2026-10-07).
//!
//! Integration does not check the budget: resident bytes can exceed it until the next
//! update when a bundle turns out larger than its charge at admission (the running mean or
//! the initial estimate); that update evicts the farthest to compensate (Tessera's choice,
//! `landblock.md` §8.1 OPEN on the transient overshoot).
//!
//! The work is linear in the number of desired landblocks (a few hundred with the default
//! rings); when neither the focus nor the desired set changed, the previous nearest-first
//! order is reused. It is reported in [`FrameReport::main_thread`].
//!
//! # Lookahead
//!
//! A bundle can take longer to build than a fast focus needs to cross the rings (a
//! landblock with a large collision set takes over half a second on one worker; at
//! 3,000 m/s the focus crosses the 1,120 m between the default far (1,600 m) and near
//! (480 m) radii in about 0.37 s). So the streamer also loads along the focus's **path**
//! (ARCHITECTURE §9.2): with the focus's velocity *v*
//! ([`WorldStreamer::update_with_velocity`], or estimated by [`WorldStreamer::update`]
//! from the focus positions of the last quarter of a second) the path is the segment from
//! the focus to the focus plus *v* · *T_e*, at most [`StreamConfig::lookahead_max`] long,
//! with *T_e* = max([`StreamConfig::lookahead_early`], [`StreamConfig::lookahead`])
//! (4 s by default). It has two parts:
//!
//! - the **urgent** part, out to *v* · *T* but never beyond the path: its landblocks are
//!   requested with `Visible` priority (`Immediate` once in the near ring). The horizon
//!   *T* ([`WorldStreamer::lookahead_horizon`]) is [`StreamConfig::lookahead`], lengthened
//!   to [`StreamConfig::lookahead_job_margin`] times the *slow-job time* when that is
//!   longer: the longest bundle job (read and decode, wall time on its worker, failed jobs
//!   included) seen recently, a peak that halves every 30 s without a longer job and takes
//!   in at most 10 s per job (Tessera's reading of "the slowest recent bundle job",
//!   `landblock.md` §8.1 OPEN of 2026-10-07);
//! - the **early** part beyond it, out to *T_e*: its landblocks are requested with their
//!   ring's own priority, `Prefetch` ahead of the rings, and raised to `Visible` when the
//!   urgent part reaches them.
//!
//! The velocity estimate restarts on a step faster than 10,000 units per second however
//! long the update took (a long frame is not a jump), on a region change, and needs
//! positions spanning 50 ms; steps up to a landblock are never jumps, and a speed below
//! 1 unit per second has no path (Tessera's choices, same OPEN). It is taken from the
//! newest position and the oldest kept one: the positions of the last quarter second plus
//! the newest one before it, so with updates further apart than a quarter second the span
//! is the time between two updates; at most 256 positions are kept, so above about
//! 1,000 updates per second the span is shorter. A velocity given to
//! [`WorldStreamer::update_with_velocity`] replaces the estimate and discards its
//! positions (the estimate starts over on the next [`WorldStreamer::update`]), a given
//! velocity that is not finite turns lookahead off for that update, and a rejected focus
//! keeps the previous velocity and the estimate's positions, given a velocity or not: the
//! given velocity of such an update is ignored (Tessera's choices, `landblock.md` §8.1
//! lookahead OPEN).
//!
//! A request must start at least one job time before its landblock reaches the near ring,
//! and job times grow with the landblock (a large collision set: M3-b's late landblock
//! r2 (239, 242) took 0.53–0.75 s on a loaded machine and up to about 2 s under a load
//! average of 40, almost all of it the collision set; `walker.md` §14.11, performance
//! note) and with the machine's load. The
//! slow-job time only learns from jobs that have finished, so on its own the urgent part
//! comes too late for the first very slow landblock of a flight. The early part starts
//! such jobs seconds ahead whenever a worker is free while they are still `Prefetch`,
//! without taking workers from the urgent part and the rings. Only a job that has
//! started gains from that: the pool runs older requests first within a priority, but
//! raising a request that is still queued orders it like a new request made at the
//! moment it was raised, behind every older `Visible` one. When the focus keeps going,
//! early requests are work the rings would ask for anyway, only sooner; on a turn they
//! are released (cancelled while queued).
//!
//! The landblocks within the near radius of the path (the ones that will be in the near
//! ring within that time if the focus keeps going) form the *corridor*
//! ([`FrameReport::corridor`], [`FrameReport::urgent`]); those beyond the far radius
//! become desired ahead of their ring ([`FrameReport::ahead`]). A block stays in the
//! corridor (or its urgent part) until it is the rings' hysteresis margin outside the
//! near radius of the path (or of the urgent part), so a path that wavers does not
//! release and request the same blocks every frame (Tessera's choice, same OPEN); an
//! ahead block that leaves the corridor without entering the rings is released. Ahead
//! blocks are charged and admitted like far-ring
//! blocks, by distance from the focus: they lie beyond the far radius, so they are cut
//! before every near- and mid-ring block and every far-ring block nearer than the far
//! radius, but they interleave with far-ring blocks in the hysteresis band (a ring
//! member stays until it is the hysteresis margin outside the far radius). Their bundles
//! are not resident until they enter the rings ([`WorldStreamer::is_ahead`]): neither
//! [`WorldStreamer::resident`] nor [`WorldStreamer::bundle`] nor any lookup sees them, so
//! consumers do not draw or populate them early.
//! Lookahead changes only when bundles are requested, never what a bundle holds: bundles
//! stay the data's deterministic ones and the residency budget binds as before.
//!
//! # World systems (M4.5)
//!
//! Each bundle is built on a worker with its scenery, water, collision set, lights and LOD
//! classes ([`crate::bundle`] module docs), and its [`LandblockBundle::heap_bytes`]
//! includes them, so the residency budget accounts for collision sets and scenery. When a
//! bundle becomes resident as a ring member its water joins the streamer's [`WaterMap`];
//! when it leaves, the water leaves too. A bundle has water whenever it has a collision set
//! and its region's water tables loaded, even with [`crate::BundleOptions::build_water`]
//! off (see that option). The lookups over what is resident (ring members;
//! not bundles loaded ahead of the rings):
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
//! ([`BlockCollision::cell_bounds`]) and of its Light entities, and asks every landblock
//! whose cells can contain a point, or whose lights can lie within a radius of it.
//!
//! # Frills
//!
//! The frill ring ([`FrillRing`], `scenery.md` §8.2) streams with the camera:
//! [`WorldStreamer::update_frills`] moves it to the blend of camera and player of
//! [`FrillSettings::player_position_blend`], drops the sectors that left, and generates
//! entering sectors nearest first on the same asset workers, once their landblock is
//! resident with its scenery inputs. At most [`StreamConfig::max_frill_jobs_per_frame`]
//! sectors start per frame, and a sector starts only while the bytes of the held sectors
//! plus the expected bytes of the running jobs and its own (the mean sector size so far)
//! fit [`StreamConfig::frill_budget`]; before the first sector is known one job runs at a
//! time. Sector jobs run at `Visible` priority; a job that panics leaves an empty sector,
//! not retried while the sector is held (Tessera's choices, `landblock.md` §8.1 OPEN of
//! 2026-10-07). Frills never collide (§8.1).

use std::collections::{HashMap, HashSet, VecDeque};
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
use crate::coords::{LANDBLOCK_SIZE, LandblockKey, MAX_BLOCK, WorldPos};
use crate::draw_distance::{DistanceSettings, LodClasses, LodValues};
use crate::environment::{Evaluation, SkyLibrary, block_sky_key};
use crate::error::{Result, WorldError};
use crate::frills::{
    DensityThinner, FrillRing, FrillSector, FrillSectorKey, FrillSettings, generate_sector,
    ring_centre,
};
use crate::lighting::LightEntity;
use crate::mapcoords::MapCoords;
use crate::ring::{MAX_RING_RADIUS, Ring, RingConfig, RingTracker};
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
    /// Default 64 MiB (Tessera's choice; no spec states it, `landblock.md` §8.1 OPEN on
    /// streamer defaults).
    pub cache_budget: usize,
    /// Most finished requests integrated per frame. Default 32 (Tessera's choice, same
    /// OPEN).
    pub max_integrations_per_frame: usize,
    /// Main-thread time after which integration stops for this frame. Default 2 ms
    /// (Tessera's choice, same OPEN).
    pub frame_budget: Duration,
    /// Admit the near ring, and the east, north and north-east neighbours of its
    /// landblocks, even beyond the residency budget.
    pub always_admit_near: bool,
    /// Charge of a never-loaded bundle before any bundle has loaded; afterwards admission
    /// charges the running mean of loaded bundles ([`WorldStreamer::size_estimate`]).
    /// Default 512 KiB (Tessera's choice, same OPEN).
    pub initial_size_estimate: usize,
    /// The frill engine's options (`scenery.md` §8.4); a radius of 0 (level 0 or
    /// `draw_frills` off) holds no sectors: that is how frills are turned off. Change them
    /// at run time with [`WorldStreamer::set_frill_settings`].
    pub frills: FrillSettings,
    /// Bytes of frill sectors held; beyond it no new sector starts until some leave.
    /// Positive ([`StreamConfig::validate`]). Default 64 MiB (Tessera's choice, same OPEN;
    /// ARCHITECTURE §9.2 says only "within their own byte budget").
    pub frill_budget: usize,
    /// Most frill sector *jobs* started per frame on the asset workers. Positive
    /// ([`StreamConfig::validate`]). Default 64 (Tessera's choice, same OPEN). Not the
    /// walker's limit of 96 sectors *built for drawing* per frame (`walker.md` §14.7).
    pub max_frill_jobs_per_frame: usize,
    /// How far ahead in time the focus's path is loaded with `Visible` priority (module
    /// docs, "Lookahead": the *urgent* part of the path); zero turns lookahead off.
    /// Default 1.5 s.
    pub lookahead: Duration,
    /// How far ahead in time the focus's path is requested at all (module docs,
    /// "Lookahead": beyond the urgent part with the ring's own priority, `Prefetch` ahead
    /// of the rings). A value below [`StreamConfig::lookahead`] is raised to it; slow jobs
    /// do not lengthen it. Default 4 s.
    pub lookahead_early: Duration,
    /// Longest lookahead path in world units; finite, 0 to [`MAX_LOOKAHEAD_DISTANCE`]
    /// ([`StreamConfig::validate`]). Default [`MAX_LOOKAHEAD_DISTANCE`] (12,800, 80
    /// landblocks: 4.3 s at 3,000 units per second).
    pub lookahead_max: f64,
    /// The lookahead horizon is at least this many times the recent slow-job time (module
    /// docs, "Lookahead"); 0 keeps it at [`StreamConfig::lookahead`]. Finite, 0 to
    /// [`MAX_LOOKAHEAD_JOB_MARGIN`] ([`StreamConfig::validate`]). Default 1.5.
    pub lookahead_job_margin: f64,
}

/// Largest [`StreamConfig::lookahead_job_margin`].
pub const MAX_LOOKAHEAD_JOB_MARGIN: f64 = 10.0;

/// Largest [`StreamConfig::lookahead_max`]: twice [`MAX_RING_RADIUS`] (12,800 units, 80
/// landblocks).
pub const MAX_LOOKAHEAD_DISTANCE: f64 = 2.0 * MAX_RING_RADIUS;

impl StreamConfig {
    /// Checks the ring radii ([`RingConfig::validate`]), that the residency budget, the
    /// integrations per frame, the frame budget, the frill budget and the frill jobs per
    /// frame are positive (frills are turned off through [`StreamConfig::frills`]), and the
    /// ranges of the lookahead distance and job margin.
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
        if !(self.lookahead_max.is_finite()
            && (0.0..=MAX_LOOKAHEAD_DISTANCE).contains(&self.lookahead_max))
        {
            return Err(WorldError::Config(
                "lookahead distance must be finite and between 0 and twice the largest ring radius",
            ));
        }
        if !(self.lookahead_job_margin.is_finite()
            && (0.0..=MAX_LOOKAHEAD_JOB_MARGIN).contains(&self.lookahead_job_margin))
        {
            return Err(WorldError::Config(
                "lookahead job margin must be finite and between 0 and 10",
            ));
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
            lookahead: Duration::from_millis(1_500),
            lookahead_early: Duration::from_secs(4),
            lookahead_max: MAX_LOOKAHEAD_DISTANCE,
            lookahead_job_margin: 1.5,
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
    /// Desired only because it lies on the focus's path, not in the rings (module docs,
    /// "Lookahead"); its `ring` is [`Ring::Far`] for admission.
    ahead: bool,
    /// On the focus's path (the corridor).
    corridor: bool,
    /// On the urgent part of the path (requested with `Visible` priority at least).
    urgent: bool,
    /// The priority the request in flight was last given ([`Slot::sync_priority`]);
    /// `None` without one.
    sent: Option<Priority>,
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
            ahead: false,
            corridor: false,
            urgent: false,
            sent: None,
        }
    }

    /// Gives the request in flight [`Slot::priority`] when it holds another one. (The pool
    /// orders a raised request that has not started like a new request made now.)
    fn sync_priority(&mut self) {
        let p = self.priority();
        if let SlotState::Requested(h) = &self.state
            && self.sent != Some(p)
        {
            h.set_priority(p);
            self.sent = Some(p);
        }
    }

    /// The request priority: the ring's, raised to `Visible` on the urgent part of the
    /// focus's path.
    fn priority(&self) -> Priority {
        let p = priority(self.ring);
        if self.corridor && self.urgent {
            p.max(Priority::Visible)
        } else {
            p
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

/// Fraction of the residency budget kept free before re-admitting an evicted bundle. A
/// seam closure that contains an evicted bundle needs the margin as a whole, whatever its
/// other members are (Tessera's reading, `landblock.md` §8.1 OPEN on closures).
const READMIT_MARGIN_DIVISOR: usize = 64;
/// Most job completions the bundle store queues for [`WorldStreamer`]'s job timings
/// between two updates (Tessera's choice, `landblock.md` §8.1 OPEN on streamer defaults).
const STORE_MAX_COMPLETIONS: usize = 4_096;
/// Delay before the first retry after a transient failure; doubled per further failure
/// ([`retry_delay`]; Tessera's policy, `landblock.md` §8.1 OPEN of 2026-10-07).
const RETRY_BASE: Duration = Duration::from_millis(10);
/// Longest retry delay.
const RETRY_MAX: Duration = Duration::from_secs(2);

/// The delay before retry number `n` (1 for the first) after consecutive transient
/// failures: [`RETRY_BASE`], doubled for each failure after the first, at most
/// [`RETRY_MAX`].
fn retry_delay(n: u32) -> Duration {
    let doublings = n.saturating_sub(1).min(16);
    RETRY_BASE.saturating_mul(1u32 << doublings).min(RETRY_MAX)
}
/// Bundle jobs and near-ring load latencies this long are logged (debug).
const SLOW_JOB: Duration = Duration::from_millis(250);
/// Focus positions this old and newer give the velocity estimate.
const VELOCITY_WINDOW: Duration = Duration::from_millis(250);
/// Shortest span of focus positions that gives a velocity estimate.
const VELOCITY_MIN_SPAN: Duration = Duration::from_millis(50);
/// Most focus positions kept for the estimate (bounds the history at very high update
/// rates).
const VELOCITY_MAX_SAMPLES: usize = 256;
/// Fastest focus the velocity estimate follows (units per second; M3-b flies at 3,000;
/// ARCHITECTURE §9.2): a step between two updates faster than this, and longer than
/// [`JUMP_MIN_STEP`], is a jump (a teleport, or the caller moving the focus elsewhere),
/// and the estimate restarts. The speed is the step over the time between the updates,
/// however long that was: a long frame is not a jump.
const MAX_TRACKED_SPEED: f64 = 10_000.0;
/// Steps up to this long are never jumps (two updates very close together, whose clock
/// jitter would otherwise make an ordinary step look faster than [`MAX_TRACKED_SPEED`]).
/// Tessera's choice; `landblock.md` §8.1 OPEN of 2026-10-07.
const JUMP_MIN_STEP: f64 = LANDBLOCK_SIZE;
/// Slowest focus speed (units per second) whose path is loaded ahead; below it the
/// estimate is treated as standing still.
const LOOKAHEAD_MIN_SPEED: f64 = 1.0;
/// Half-life of the slow-job peak (module docs, "Lookahead").
const SLOW_JOB_HALF_LIFE: Duration = Duration::from_secs(30);
/// Longest job time the slow-job peak takes in (a job stuck for longer, say on a stalled
/// disk, would otherwise stretch the horizon to its cap for minutes).
const SLOW_JOB_MAX: Duration = Duration::from_secs(10);

/// Hasher of the streamer's per-landblock maps: a landblock key is three bytes, so a
/// multiplicative hash of them is enough and much cheaper than the default hasher on the
/// main thread (admission looks up every desired landblock several times per update).
/// Keys come from the grid, not from outside input, so flooding is not a concern.
#[derive(Debug, Default, Clone, Copy)]
struct KeyHasher(u64);

impl std::hash::Hasher for KeyHasher {
    fn finish(&self) -> u64 {
        // Fibonacci hashing; the rotation brings the well-mixed high bits down to the
        // bucket index.
        self.0.wrapping_mul(0x9E37_79B9_7F4A_7C15).rotate_left(32)
    }

    fn write(&mut self, bytes: &[u8]) {
        for &b in bytes {
            self.write_u8(b);
        }
    }

    fn write_u8(&mut self, i: u8) {
        self.0 = self.0.rotate_left(8) ^ u64::from(i);
    }
}

/// Builds [`KeyHasher`]s.
type KeyHash = std::hash::BuildHasherDefault<KeyHasher>;

/// A map keyed by landblock with [`KeyHasher`].
type KeyMap<V> = HashMap<LandblockKey, V, KeyHash>;

/// The recent slow-job time (module docs, "Lookahead"): a peak of bundle job times that
/// halves every [`SLOW_JOB_HALF_LIFE`] without a longer job.
#[derive(Debug, Clone, Copy, Default)]
struct SlowJobs {
    peak: Duration,
    at: Option<Instant>,
}

impl SlowJobs {
    /// The peak at `now`.
    fn value(&self, now: Instant) -> Duration {
        let Some(at) = self.at else {
            return Duration::ZERO;
        };
        let halves =
            now.saturating_duration_since(at).as_secs_f64() / SLOW_JOB_HALF_LIFE.as_secs_f64();
        // Finite: the peak is at most SLOW_JOB_MAX and the factor in (0, 1].
        Duration::try_from_secs_f64(self.peak.as_secs_f64() * 0.5f64.powf(halves))
            .unwrap_or(Duration::ZERO)
    }

    /// Takes in a job of `time` that finished at `now`.
    fn observe(&mut self, now: Instant, time: Duration) {
        let time = time.min(SLOW_JOB_MAX);
        if time >= self.value(now) {
            self.peak = time;
            self.at = Some(now);
        }
    }
}

/// The focus positions behind the velocity estimate of [`WorldStreamer::update`].
#[derive(Debug, Default)]
struct Motion {
    region: Option<u8>,
    samples: VecDeque<(Instant, [f64; 2])>,
}

impl Motion {
    fn clear(&mut self) {
        self.region = None;
        self.samples.clear();
    }

    /// Records the focus `p` of region `region` at `now` and returns the velocity over the
    /// last [`VELOCITY_WINDOW`] (units per second), or `None` while the positions span
    /// less than [`VELOCITY_MIN_SPAN`]. A region change or a jump ([`Motion::is_jump`])
    /// starts over.
    fn push(&mut self, now: Instant, region: u8, p: [f64; 2]) -> Option<[f64; 2]> {
        let jump = self
            .samples
            .back()
            .is_some_and(|&(t, q)| Self::is_jump(now.saturating_duration_since(t), q, p));
        if self.region != Some(region) || jump {
            self.samples.clear();
            self.region = Some(region);
        }
        self.samples.push_back((now, p));
        // Keep one position at or before the window's start, so the span covers it.
        while self.samples.len() > VELOCITY_MAX_SAMPLES
            || (self.samples.len() > 2
                && self
                    .samples
                    .get(1)
                    .is_some_and(|(t, _)| now.saturating_duration_since(*t) >= VELOCITY_WINDOW))
        {
            self.samples.pop_front();
        }
        let &(t0, p0) = self.samples.front()?;
        let span = now.saturating_duration_since(t0);
        if span < VELOCITY_MIN_SPAN {
            return None;
        }
        let s = span.as_secs_f64();
        Some([(p[0] - p0[0]) / s, (p[1] - p0[1]) / s])
    }

    /// Whether the step from `q` to `p` taken in `dt` is a jump: longer than
    /// [`JUMP_MIN_STEP`] and faster than [`MAX_TRACKED_SPEED`]. Relative to the time
    /// however long it was, so a long frame (a stall, a loading pause) of a fast flight is
    /// not a jump (ARCHITECTURE §9.2).
    fn is_jump(dt: Duration, q: [f64; 2], p: [f64; 2]) -> bool {
        let step = (p[0] - q[0]).hypot(p[1] - q[1]);
        // A step that is not finite cannot come from a valid focus; treat it as a jump.
        !step.is_finite() || (step > JUMP_MIN_STEP && step > MAX_TRACKED_SPEED * dt.as_secs_f64())
    }
}

/// The distance from the square of `key` to the segment from `p` to `q` (region frame):
/// 0 when they meet, else the least of the distances from the segment's ends to the
/// square and from the square's corners to the segment.
fn square_segment_distance(key: LandblockKey, p: [f64; 2], q: [f64; 2]) -> f64 {
    let [x0, y0] = key.origin();
    let (x1, y1) = (x0 + LANDBLOCK_SIZE, y0 + LANDBLOCK_SIZE);
    // Clip the segment p + t (q − p), t in [0, 1], against the square (slab method).
    let d = [q[0] - p[0], q[1] - p[1]];
    let (mut t0, mut t1) = (0.0f64, 1.0f64);
    let mut meets = true;
    for (o, dir, lo, hi) in [(p[0], d[0], x0, x1), (p[1], d[1], y0, y1)] {
        if dir == 0.0 {
            if o < lo || o > hi {
                meets = false;
            }
        } else {
            let (a, b) = ((lo - o) / dir, (hi - o) / dir);
            t0 = t0.max(a.min(b));
            t1 = t1.min(a.max(b));
        }
    }
    if meets && t0 <= t1 {
        return 0.0;
    }
    let to_segment = |c: [f64; 2]| {
        let len2 = d[0] * d[0] + d[1] * d[1];
        let t = if len2 > 0.0 {
            (((c[0] - p[0]) * d[0] + (c[1] - p[1]) * d[1]) / len2).clamp(0.0, 1.0)
        } else {
            0.0
        };
        (p[0] + t * d[0] - c[0]).hypot(p[1] + t * d[1] - c[1])
    };
    [[x0, y0], [x1, y0], [x0, y1], [x1, y1]]
        .into_iter()
        .map(to_segment)
        .fold(key.distance_to(p).min(key.distance_to(q)), f64::min)
}

/// The landblocks of `region` whose squares lie within `reach` of the segment from `p` to
/// `q`, with that distance, in key order.
fn segment_blocks(region: u8, p: [f64; 2], q: [f64; 2], reach: f64) -> Vec<(LandblockKey, f64)> {
    let mut out = Vec::new();
    if !(p.iter().chain(&q).all(|v| v.is_finite()) && reach.is_finite() && reach >= 0.0) {
        return out;
    }
    let max = f64::from(MAX_BLOCK);
    let range = |a: f64, b: f64| -> Option<(u8, u8)> {
        let lo = ((a.min(b) - reach) / LANDBLOCK_SIZE).floor() - 1.0;
        let hi = ((a.max(b) + reach) / LANDBLOCK_SIZE).floor();
        if hi < 0.0 || lo > max {
            return None;
        }
        #[allow(clippy::cast_possible_truncation, clippy::cast_sign_loss)]
        Some((lo.clamp(0.0, max) as u8, hi.clamp(0.0, max) as u8))
    };
    let Some((bx0, bx1)) = range(p[0], q[0]) else {
        return out;
    };
    let d = [q[0] - p[0], q[1] - p[1]];
    for x in bx0..=bx1 {
        // A point within `reach` of this column's squares lies within `reach` of its x
        // span, so only the y span of that part of the segment (plus `reach`) can hold
        // blocks in reach: a long diagonal path tests a band, not its bounding box. The
        // spans are widened by one unit against rounding; the exact test below decides.
        let cx = f64::from(x) * LANDBLOCK_SIZE;
        let (xlo, xhi) = (cx - reach - 1.0, cx + LANDBLOCK_SIZE + reach + 1.0);
        let (t0, t1) = if d[0] == 0.0 {
            if p[0] < xlo || p[0] > xhi {
                continue;
            }
            (0.0, 1.0)
        } else {
            let (a, b) = ((xlo - p[0]) / d[0], (xhi - p[0]) / d[0]);
            (a.min(b).max(0.0), a.max(b).min(1.0))
        };
        if t0 > t1 {
            continue;
        }
        let (ya, yb) = (p[1] + t0 * d[1], p[1] + t1 * d[1]);
        let Some((by0, by1)) = range(ya.min(yb) - 1.0, ya.max(yb) + 1.0) else {
            continue;
        };
        for y in by0..=by1 {
            let Ok(key) = LandblockKey::new(region, x, y) else {
                continue;
            };
            let d = square_segment_distance(key, p, q);
            if d <= reach {
                out.push((key, d));
            }
        }
    }
    out
}

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
    /// Resident landblocks after the update (ring members and those ahead of the rings).
    pub resident_blocks: usize,
    /// Of [`FrameReport::resident_blocks`], those ahead of the rings (not in
    /// [`WorldStreamer::resident`]).
    pub ahead_resident: usize,
    /// Their bytes.
    pub resident_bytes: usize,
    /// Requests in flight.
    pub loading: usize,
    /// Desired landblocks held back by the budget or waiting to retry.
    pub deferred: usize,
    /// Of those, landblocks waiting to retry after a transient failure.
    pub waiting_retry: usize,
    /// Of [`FrameReport::deferred`], landblocks this update admitted that have no request
    /// yet: waiting to retry after a transient failure, or whose request ended without a
    /// bundle during the update. A later update requests them; [`WorldStreamer::settle`]
    /// waits for them.
    pub pending: usize,
    /// Near-ring landblocks not resident yet (holes the camera might see).
    pub near_missing: usize,
    /// Landblocks desired ahead of the rings, on the focus's path (module docs,
    /// "Lookahead").
    pub ahead: usize,
    /// Desired landblocks on the focus's path (in the rings or ahead of them).
    pub corridor: usize,
    /// Of [`FrameReport::corridor`], those on the urgent part of the path (requested with
    /// `Visible` priority at least).
    pub urgent: usize,
    /// The lookahead horizon of this update ([`WorldStreamer::lookahead_horizon`]) when the
    /// focus had a path, even one that lies wholly inside the rings; zero without a path
    /// (lookahead off, standing still, no velocity yet).
    pub lookahead: Duration,
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
    /// Requests issued for landblocks ahead of the rings (module docs, "Lookahead").
    pub ahead_requested: u64,
    /// Landblocks ahead of the rings that left the focus's path before entering them.
    pub ahead_released: u64,
    /// Longest lookahead horizon used.
    pub max_lookahead: Duration,
    /// Main-thread time spent finding the landblocks on the focus's path (part of
    /// [`FrameReport::main_thread`]), summed over the updates.
    pub corridor_time: Duration,
    /// Longest main-thread time of one update spent on the focus's path.
    pub max_corridor_time: Duration,
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
    slots: KeyMap<Slot>,
    focus: Option<WorldPos>,
    resident_bytes: usize,
    loaded_sizes: (u64, u128),
    stats: StreamStats,
    /// Desired landblocks nearest first, valid for `order_focus`.
    order: Vec<(f64, LandblockKey)>,
    order_focus: Option<[f64; 2]>,
    /// The lookup indexes over the resident bundles of ring members.
    lookups: Lookups,
    frill_ring: FrillRing,
    frills: HashMap<FrillSectorKey, FrillState>,
    frill_bytes: usize,
    /// Sectors generated and their bytes, for the size estimate of a sector job not yet
    /// finished ([`WorldStreamer::update_frills`]).
    frill_sizes: (u64, u128),
    /// The focus positions behind the velocity estimate.
    motion: Motion,
    /// The focus velocity of the last update (units per second), when known.
    velocity: Option<[f64; 2]>,
    /// The recent slow-job time.
    slow_jobs: SlowJobs,
}

/// Region-frame bounds (min, max).
type Bounds = ([f64; 3], [f64; 3]);

/// The indexes the lookups use over the resident bundles of ring members (ARCHITECTURE
/// §9.2, "Lookups": over what is resident; bundles loaded ahead of the rings are not, and
/// join when their landblock enters the rings): their water, the bounds of their placed
/// cells (cells are not confined to their landblock's square) and the bounds of their Light
/// entities (interior lights of such cells lie far outside it too).
#[derive(Debug, Default)]
struct Lookups {
    water: WaterMap,
    /// Region-frame bounds of the placed cells of each landblock that has some, grown by
    /// the lookup's broad-phase margin.
    cell_bounds: HashMap<LandblockKey, Bounds>,
    /// Region-frame bounds of the Light entities of each landblock that has some.
    light_bounds: HashMap<LandblockKey, Bounds>,
}

impl Lookups {
    /// Adds a bundle that became resident as a ring member.
    fn insert(&mut self, b: &Arc<LandblockBundle>) {
        if let Some(w) = &b.water {
            self.water.insert(Arc::clone(w));
        }
        if let Some(bounds) = region_cell_bounds(b) {
            self.cell_bounds.insert(b.key, bounds);
        }
        if let Some(bounds) = region_light_bounds(b) {
            self.light_bounds.insert(b.key, bounds);
        }
    }

    /// Removes landblock `key` (released, evicted, or never added: idempotent).
    fn remove(&mut self, key: LandblockKey) {
        self.water.remove(key);
        self.cell_bounds.remove(&key);
        self.light_bounds.remove(&key);
    }
}

/// The region-frame bounds of `b`'s Light entities.
fn region_light_bounds(b: &LandblockBundle) -> Option<Bounds> {
    let o = b.key.origin();
    let o = [o[0], o[1], 0.0];
    b.lights.iter().fold(None, |acc: Option<Bounds>, l| {
        let p: [f64; 3] = std::array::from_fn(|i| o[i] + f64::from(l.position[i]));
        if !p.iter().all(|c| c.is_finite()) {
            return acc;
        }
        Some(match acc {
            None => (p, p),
            Some((lo, hi)) => (
                std::array::from_fn(|i| lo[i].min(p[i])),
                std::array::from_fn(|i| hi[i].max(p[i])),
            ),
        })
    })
}

/// The distance from `p` to the box `b` (0 inside).
fn box_distance(b: &Bounds, p: [f64; 3]) -> f64 {
    (0..3)
        .map(|i| {
            let d = (b.0[i] - p[i]).max(p[i] - b.1[i]).max(0.0);
            d * d
        })
        .sum::<f64>()
        .sqrt()
}

/// The region-frame bounds of `b`'s placed cells, grown by [`CELL_BROAD_PHASE_HALF`].
fn region_cell_bounds(b: &LandblockBundle) -> Option<Bounds> {
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
                max_completions: STORE_MAX_COMPLETIONS,
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
            slots: KeyMap::default(),
            focus: None,
            resident_bytes: 0,
            loaded_sizes: (0, 0),
            stats: StreamStats::default(),
            order: Vec::new(),
            order_focus: None,
            lookups: Lookups::default(),
            frill_ring,
            frills: HashMap::new(),
            frill_bytes: 0,
            frill_sizes: (0, 0),
            motion: Motion::default(),
            velocity: None,
            slow_jobs: SlowJobs::default(),
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
    ///
    /// The focus's velocity for lookahead (module docs) is estimated from the focus
    /// positions of the last quarter of a second; a region change, or a step between two
    /// updates longer than a landblock and faster than 10,000 units per second (a
    /// teleport, or the caller moving the focus to another place), starts the estimate
    /// over. A caller that knows the velocity, or switches the focus between places,
    /// calls [`WorldStreamer::update_with_velocity`] instead.
    pub fn update(&mut self, focus: WorldPos) -> FrameReport {
        self.update_inner(focus, None)
    }

    /// [`WorldStreamer::update`] with the focus's velocity `velocity` (region-frame units
    /// per second; zero when standing still) instead of the estimate. A velocity that is
    /// not finite turns lookahead off for this update.
    pub fn update_with_velocity(&mut self, focus: WorldPos, velocity: [f64; 2]) -> FrameReport {
        self.update_inner(focus, Some(velocity))
    }

    /// The focus velocity the last update used for lookahead (units per second), when known.
    #[must_use]
    pub fn velocity(&self) -> Option<[f64; 2]> {
        self.velocity
    }

    /// The lookahead horizon *T* now (module docs, "Lookahead"): [`StreamConfig::lookahead`],
    /// or [`StreamConfig::lookahead_job_margin`] times the recent slow-job time when that
    /// is longer; zero when lookahead is off.
    #[must_use]
    pub fn lookahead_horizon(&self) -> Duration {
        self.horizon_at(Instant::now())
    }

    fn horizon_at(&self, now: Instant) -> Duration {
        let base = self.config.lookahead;
        if base.is_zero() {
            return Duration::ZERO;
        }
        // Both factors are bounded (SLOW_JOB_MAX, MAX_LOOKAHEAD_JOB_MARGIN).
        let slow = Duration::try_from_secs_f64(
            self.slow_jobs.value(now).as_secs_f64() * self.config.lookahead_job_margin,
        )
        .unwrap_or(Duration::ZERO);
        base.max(slow)
    }

    /// The recent slow-job time (module docs, "Lookahead").
    #[must_use]
    pub fn slow_job_time(&self) -> Duration {
        self.slow_jobs.value(Instant::now())
    }

    /// The priority `key`'s request in flight was given, or the one it would be requested
    /// with when it has none, when it is desired (tests of the path's parts).
    #[cfg(test)]
    pub(crate) fn request_priority(&self, key: LandblockKey) -> Option<Priority> {
        self.slots.get(&key).and_then(|s| match s.state {
            SlotState::Requested(_) => s.sent,
            _ => Some(s.priority()),
        })
    }

    /// Whether `key`'s bundle is requested and still in flight (tests).
    #[cfg(test)]
    pub(crate) fn is_in_flight(&self, key: LandblockKey) -> bool {
        self.slots
            .get(&key)
            .is_some_and(|s| matches!(s.state, SlotState::Requested(_)))
    }

    /// Takes in a bundle job time as if a job had just finished (tests of the horizon).
    #[cfg(test)]
    pub(crate) fn observe_job_time(&mut self, time: Duration) {
        self.slow_jobs.observe(Instant::now(), time);
    }

    fn update_inner(&mut self, focus: WorldPos, given: Option<[f64; 2]>) -> FrameReport {
        let t0 = Instant::now();
        let mut report = FrameReport::default();
        let valid = focus.is_finite() && LandblockKey::new(focus.region, 0, 0).is_ok();
        // A rejected focus keeps the previous velocity and the estimate's positions, like
        // the previous focus, whether a velocity was given or not: the validity check
        // comes before the given velocity.
        self.velocity = match given {
            _ if !valid => self.velocity,
            Some(v) => {
                self.motion.clear();
                v.iter().all(|c| c.is_finite()).then_some(v)
            }
            None => self.motion.push(t0, focus.region, focus.xy()),
        };
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
                slot.sync_priority();
            }
        }
        for e in &diff.entered {
            match self.slots.get_mut(&e.key) {
                // Loaded ahead of its ring: it keeps its request or bundle, which now joins
                // the lookups.
                Some(slot) => {
                    if let (true, SlotState::Ready(b)) = (slot.ahead, &slot.state) {
                        self.lookups.insert(b);
                    }
                    slot.ahead = false;
                    slot.ring = e.ring;
                    slot.distance = e.distance;
                    slot.sync_priority();
                }
                None => {
                    self.slots.insert(e.key, Slot::new(e.ring, e.distance));
                }
            }
        }
        let corridor_start = Instant::now();
        let ahead_changed = self.update_corridor(&focus, t0, &mut report);
        let corridor_time = corridor_start.elapsed();
        self.stats.corridor_time += corridor_time;
        self.stats.max_corridor_time = self.stats.max_corridor_time.max(corridor_time);

        // Nearest first; reused while neither the focus nor the desired set changed.
        let g = focus.xy();
        let mut order = std::mem::take(&mut self.order);
        if !diff.is_empty() || ahead_changed || self.order_focus != Some(g) {
            order.clear();
            order.extend(self.slots.iter_mut().map(|(k, s)| {
                s.distance = k.distance_to(g);
                (s.distance, *k)
            }));
            order.sort_unstable_by(|a, b| a.0.total_cmp(&b.0).then(a.1.cmp(&b.1)));
            self.order_focus = Some(g);
        }

        // One clock reading for the update: admission and the summary agree on which
        // retries are still waiting.
        self.admit(&order, &mut report, t0);
        self.poll_jobs();
        self.integrate(&order, &mut report);
        self.order = order;
        self.finish(t0, &mut report);
        report
    }

    /// Lookahead (module docs): marks the slots on the focus's path and its urgent part,
    /// adds the landblocks ahead of the rings and releases those that left it. Returns
    /// whether the desired set changed.
    fn update_corridor(
        &mut self,
        focus: &WorldPos,
        now: Instant,
        report: &mut FrameReport,
    ) -> bool {
        let rings = self.config.rings;
        let p = focus.xy();
        let horizon = self.horizon_at(now);
        // The whole path covers the configured time (ARCHITECTURE §9.2: the next 4 s); slow
        // jobs lengthen only its urgent part, never the path.
        let early = self.config.lookahead_early.max(self.config.lookahead);
        // The direction and the lengths of the urgent and the whole path.
        let path = self.velocity.filter(|_| !horizon.is_zero()).and_then(|v| {
            let speed = v[0].hypot(v[1]);
            if !speed.is_finite() || speed < LOOKAHEAD_MIN_SPEED {
                return None;
            }
            let whole = (speed * early.as_secs_f64()).min(self.config.lookahead_max);
            let urgent = (speed * horizon.as_secs_f64()).min(whole);
            let end = |len: f64| [p[0] + v[0] / speed * len, p[1] + v[1] / speed * len];
            (whole > 0.0).then(|| (end(urgent), end(whole)))
        });
        if path.is_some() {
            report.lookahead = horizon;
            self.stats.max_lookahead = self.stats.max_lookahead.max(horizon);
        }
        let reach = rings.near + rings.hysteresis;
        // Each block within reach of the whole path, with its distances to the whole path
        // and to the urgent part.
        let near: KeyMap<(f64, f64)> = path.map_or_else(KeyMap::default, |(qu, qw)| {
            segment_blocks(focus.region, p, qw, reach)
                .into_iter()
                .map(|(k, d)| (k, (d, square_segment_distance(k, p, qu))))
                .collect()
        });
        let mut changed = false;
        let mut leaving: Vec<LandblockKey> = Vec::new();
        for (key, slot) in &mut self.slots {
            let (d, du) = near
                .get(key)
                .copied()
                .unwrap_or((f64::INFINITY, f64::INFINITY));
            let on = d <= rings.near || (slot.corridor && d <= reach);
            let urgent = on && (du <= rings.near || (slot.urgent && du <= reach));
            if on == slot.corridor && urgent == slot.urgent {
                continue;
            }
            slot.corridor = on;
            slot.urgent = urgent;
            if !on && slot.ahead {
                leaving.push(*key);
            } else {
                slot.sync_priority();
            }
        }
        leaving.sort_unstable();
        for key in leaving {
            if let Some(slot) = self.slots.remove(&key) {
                self.release(slot.state, report);
                self.stats.ahead_released += 1;
                changed = true;
            }
        }
        let mut entering: Vec<(LandblockKey, f64)> = near
            .iter()
            .filter(|&(k, &(d, _))| d <= rings.near && !self.slots.contains_key(k))
            .map(|(k, &(_, du))| (*k, du))
            .collect();
        entering.sort_unstable_by_key(|e| e.0);
        for (key, du) in entering {
            let mut slot = Slot::new(Ring::Far, key.distance_to(p));
            slot.ahead = true;
            slot.corridor = true;
            slot.urgent = du <= rings.near;
            self.slots.insert(key, slot);
            changed = true;
        }
        changed
    }

    /// Fills the summary counts of `report` and the statistics. `t0` is the update's clock
    /// reading, the one admission used.
    fn finish(&mut self, t0: Instant, report: &mut FrameReport) {
        for slot in self.slots.values() {
            match &slot.state {
                SlotState::Deferred => {
                    report.deferred += 1;
                    if slot.retry_after.is_some_and(|t| t > t0) {
                        report.waiting_retry += 1;
                    }
                    if slot.admit {
                        report.pending += 1;
                    }
                }
                SlotState::Requested(_) => report.loading += 1,
                SlotState::Ready(b) => {
                    report.resident_blocks += 1;
                    report.ahead_resident += usize::from(slot.ahead);
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
            report.ahead += usize::from(slot.ahead);
            report.corridor += usize::from(slot.corridor);
            report.urgent += usize::from(slot.corridor && slot.urgent);
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
                self.lookups.remove(b.key);
            }
            SlotState::Deferred | SlotState::Failed(_) => {}
        }
    }

    /// Admission (module docs, step 3): decides the admitted set nearest first, then
    /// requests, cancels and evicts. `now` is the update's clock reading.
    fn admit(&mut self, order: &[(f64, LandblockKey)], report: &mut FrameReport, now: Instant) {
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
        let mut seen: HashSet<LandblockKey, KeyHash> = HashSet::default();
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
            if slot.ring == Ring::Far {
                // A far-ring (or ahead) landblock's closure is itself: the walk below
                // extends only from the near and mid rings. Same result, no set.
                let charge = if slot.admit { 0 } else { slot.charge(estimate) };
                let limit = if !slot.admit && slot.is_readmission() {
                    budget.saturating_sub(margin)
                } else {
                    budget
                };
                if cum.saturating_add(charge) <= limit {
                    cum = cum.saturating_add(charge);
                    if let Some(s) = self.slots.get_mut(&key) {
                        s.admit = true;
                        s.done = true;
                    }
                } else {
                    cut = true;
                }
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
        for &(_, key) in order {
            let Some(slot) = self.slots.get_mut(&key) else {
                continue;
            };
            if slot.admit {
                let waiting = slot.retry_after.is_some_and(|t| t > now);
                if matches!(slot.state, SlotState::Deferred) && !waiting {
                    let p = slot.priority();
                    let h = self.store.request(key.data_did().0, p);
                    slot.state = SlotState::Requested(h);
                    slot.sent = Some(p);
                    slot.requested_at = Some(now);
                    report.requested += 1;
                    self.stats.requested += 1;
                    self.stats.ahead_requested += u64::from(slot.ahead);
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
                    self.lookups.remove(b.key);
                    report.evicted += 1;
                    self.stats.evicted += 1;
                }
                SlotState::Failed(e) => slot.state = SlotState::Failed(e),
                SlotState::Deferred => {}
            }
        }
    }

    /// Drains the store's completion queue for job timings and the slow-job time.
    fn poll_jobs(&mut self) {
        let now = Instant::now();
        for c in self.store.poll_completions(256) {
            if matches!(c.outcome, Outcome::Cancelled) {
                continue;
            }
            self.slow_jobs.observe(now, c.read_time + c.decode_time);
            let s = &mut self.stats;
            s.jobs += 1;
            s.read_time += c.read_time;
            s.decode_time += c.decode_time;
            s.max_job = s.max_job.max(c.read_time + c.decode_time);
            if c.read_time + c.decode_time >= SLOW_JOB {
                tracing::debug!(
                    did = %c.key.did,
                    priority = ?c.priority,
                    read_ms = c.read_time.as_secs_f64() * 1e3,
                    decode_ms = c.decode_time.as_secs_f64() * 1e3,
                    "slow landblock bundle job"
                );
            }
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
                    if !slot.ahead {
                        // Ahead of the rings, it joins the lookups when it enters them.
                        self.lookups.insert(&b);
                    }
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
                        if slot.ring == Ring::Near && l >= SLOW_JOB {
                            tracing::debug!(
                                landblock = %key,
                                latency_ms = l.as_secs_f64() * 1e3,
                                distance = slot.distance,
                                "near-ring landblock resident late"
                            );
                        }
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
                        slot.retry_after = Some(Instant::now() + retry_delay(slot.retries));
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
        self.lookups.insert(&b);
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
        self.lookups = Lookups::default();
        self.motion.clear();
        self.velocity = None;
        self.clear_frills();
    }

    /// Repeats [`WorldStreamer::update_with_velocity`] at `focus` with zero velocity (so
    /// nothing is loaded ahead, module docs "Lookahead") until an update changes nothing
    /// (nothing loading, no admitted landblock without a request ([`FrameReport::pending`]),
    /// nothing requested, integrated or evicted) or `timeout` passes (loading screens,
    /// tools, tests). Returns whether it settled: then every admitted landblock is resident
    /// or failed, so the near ring (always admitted by default) has no landblock loading,
    /// deferred or missing (`walker.md` §7).
    /// A timeout too large for an [`Instant`] (such as `Duration::MAX`) means no deadline.
    pub fn settle(&mut self, focus: WorldPos, timeout: Duration) -> bool {
        let deadline = Instant::now().checked_add(timeout);
        loop {
            // The focus stands still: nothing is loaded ahead.
            let r = self.update_with_velocity(focus, [0.0; 2]);
            if r.loading == 0
                && r.pending == 0
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

    /// The state of a desired landblock; `None` when it is not desired. A landblock loaded
    /// ahead of the rings ([`WorldStreamer::is_ahead`]) reports [`BlockState::Ready`] once
    /// its bundle is built, but [`WorldStreamer::bundle`] and the lookups see it only when
    /// it enters the rings.
    #[must_use]
    pub fn state_of(&self, key: LandblockKey) -> Option<BlockState> {
        self.slots.get(&key).map(|s| match &s.state {
            SlotState::Deferred => BlockState::Deferred,
            SlotState::Requested(_) => BlockState::Loading,
            SlotState::Ready(b) => BlockState::Ready(Arc::clone(b)),
            SlotState::Failed(e) => BlockState::Failed(e.clone()),
        })
    }

    /// The ring of a desired landblock ([`Ring::Far`] for one desired ahead of the rings,
    /// [`WorldStreamer::is_ahead`]).
    #[must_use]
    pub fn ring_of(&self, key: LandblockKey) -> Option<Ring> {
        self.slots.get(&key).map(|s| s.ring)
    }

    /// A resident bundle of a ring member. Bundles loaded ahead of the rings are not
    /// resident (ARCHITECTURE §9.2) until their landblock enters the rings: like every
    /// lookup of this type, this one does not see them ([`WorldStreamer::state_of`] does).
    #[must_use]
    pub fn bundle(&self, key: LandblockKey) -> Option<&Arc<LandblockBundle>> {
        let slot = self.slots.get(&key)?;
        match &slot.state {
            SlotState::Ready(b) if !slot.ahead => Some(b),
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

    /// Every resident bundle of a ring member with its ring, in no particular order.
    ///
    /// Bundles loaded ahead of the rings (module docs, "Lookahead") are left out until
    /// their landblock enters the rings: they lie beyond the far radius, so consumers that
    /// draw, upload or populate what is resident would only add and drop them again on a
    /// turn. [`WorldStreamer::bundle`] and the lookups leave them out too (ARCHITECTURE
    /// §9.2); [`WorldStreamer::state_of`] and [`WorldStreamer::is_ahead`] report them.
    pub fn resident(&self) -> impl Iterator<Item = (LandblockKey, Ring, &Arc<LandblockBundle>)> {
        self.slots.iter().filter_map(|(k, s)| match &s.state {
            SlotState::Ready(b) if !s.ahead => Some((*k, s.ring, b)),
            _ => None,
        })
    }

    /// Whether `key` is desired only because it lies on the focus's path ahead of the
    /// rings (module docs, "Lookahead"); such a landblock's [`WorldStreamer::ring_of`] is
    /// [`Ring::Far`] and its bundle is not in [`WorldStreamer::resident`].
    #[must_use]
    pub fn is_ahead(&self, key: LandblockKey) -> bool {
        self.slots.get(&key).is_some_and(|s| s.ahead)
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
        &self.lookups.water
    }

    /// The water at `pos` (`water.md` §6.3, §7): the top of the liquid column through it,
    /// over the liquid boxes of its landblock and its eight neighbours; `None` when `pos` is
    /// not in liquid or its landblock is not resident.
    #[must_use]
    pub fn water_at(&self, pos: &WorldPos) -> Option<WaterSample> {
        self.lookups.water.water_at(pos)
    }

    /// The client's "is this position under water" (`water.md` §6.3, §10) over what is
    /// resident.
    #[must_use]
    pub fn is_under_water(&self, pos: &WorldPos) -> bool {
        self.lookups.water.is_under_water(pos)
    }

    /// The landblock water (`water.md` §6.2) at `pos`, when its landblock is resident and
    /// wet.
    #[must_use]
    pub fn landblock_water(&self, pos: &WorldPos) -> Option<LandblockWater> {
        self.lookups.water.landblock_water(pos)
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
    /// entity index). Every resident landblock of `pos`'s region whose Light entities can
    /// lie that close is searched, wherever it lies: interior lights of cells placed far
    /// from their owner landblock (`walker.md` §13.3, row W-C1) are found too.
    #[must_use]
    pub fn lights_near(&self, pos: &WorldPos, radius: f64) -> Vec<NearbyLight<'_>> {
        if !(pos.is_finite() && radius >= 0.0) {
            return Vec::new();
        }
        let at = [pos.x, pos.y, pos.z];
        let mut out: Vec<(f64, NearbyLight<'_>)> = Vec::new();
        for (&key, bounds) in &self.lookups.light_bounds {
            if key.region() != pos.region || box_distance(bounds, at) > radius {
                continue;
            }
            let Some(b) = self.bundle(key) else { continue };
            let o = key.origin();
            for light in &b.lights {
                let p = WorldPos::new(
                    pos.region,
                    o[0] + f64::from(light.position[0]),
                    o[1] + f64::from(light.position[1]),
                    f64::from(light.position[2]),
                );
                let d =
                    ((p.x - pos.x).powi(2) + (p.y - pos.y).powi(2) + (p.z - pos.z).powi(2)).sqrt();
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
            .lookups
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
            .lookups
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
                    self.frill_sizes.0 += 1;
                    self.frill_sizes.1 += bytes as u128;
                    *st = FrillState::Ready(Arc::new(sector), bytes);
                    report.finished += 1;
                    self.stats.frill_sectors += 1;
                }
                // A panic in the job: an empty sector, not retried while held.
                (None, TaskStatus::Failed(e)) if !matches!(e, AssetError::Shutdown) => {
                    tracing::warn!(error = %e, "frill sector job failed");
                    *st = FrillState::Ready(Arc::new(FrillSector::default()), 0);
                }
                // Cancelled, shut down or full, or the region's tables wait for a retry
                // (`scenery.md` §4.1): try again.
                (None, _) => *st = FrillState::Waiting,
            }
        }

        // Start waiting sectors nearest first, within the byte budget: the bytes held plus
        // the expected bytes of the jobs running and of the one to start (the mean of the
        // sectors generated so far) must fit, so jobs started in one frame cannot all land
        // over the budget. Before any sector has been generated one job runs at a time.
        let mut running = self
            .frills
            .values()
            .filter(|st| matches!(st, FrillState::Running(..)))
            .count();
        let (generated, generated_bytes) = self.frill_sizes;
        let expected = (generated > 0).then(|| {
            usize::try_from(generated_bytes / u128::from(generated)).unwrap_or(usize::MAX)
        });
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
                let fits = match expected {
                    Some(e) => {
                        self.frill_bytes
                            .saturating_add(e.saturating_mul(running.saturating_add(1)))
                            <= self.config.frill_budget
                    }
                    None => running == 0 && self.frill_bytes < self.config.frill_budget,
                };
                if report.started >= self.config.max_frill_jobs_per_frame || !fits {
                    break;
                }
                let Some(bundle) = k.landblock().and_then(|lb| self.bundle(lb)) else {
                    continue;
                };
                if bundle.scenery.is_none() && scenery_tables_waited(bundle) {
                    // Built while its region's tables were unavailable for a reason that
                    // is not the data's (`scenery.md` §4.1): no sector is generated from
                    // it; the sector stays waiting and is tried again on a later update
                    // (the bundle is rebuilt on its landblock's next load).
                    continue;
                }
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
                                // `None` (the region's tables wait for a retry): the cell
                                // stays empty and the sector waits again.
                                let sector = try_build_frill_sector(
                                    &ctx,
                                    &bundle,
                                    sx,
                                    sy,
                                    density,
                                    &mut thinner,
                                );
                                *out.lock().unwrap_or_else(PoisonError::into_inner) = sector;
                            });
                    match spawned {
                        Ok(h) => {
                            report.started += 1;
                            running += 1;
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
    generate_sector(&input, &sc.markers, sx, sy, density, thinner)
}

/// [`build_frill_sector`], except that a sector built while the region's scenery tables
/// are unavailable for a reason that is not the data's (the region waits for a retry,
/// [`WorldContext::scenery_retry_pending`]) is `None`: it holds no records and does not
/// count as generated, so the frill engine generates it again later (`scenery.md` §4.1
/// Tessera Decision).
#[must_use]
pub fn try_build_frill_sector(
    ctx: &WorldContext,
    bundle: &LandblockBundle,
    sx: u8,
    sy: u8,
    density: f32,
    thinner: &mut DensityThinner,
) -> Option<FrillSector> {
    let region = bundle.key.region();
    let tables = ctx.region(region).and_then(|r| r.scenery.as_ref());
    if tables.is_none() && ctx.scenery_retry_pending().any(|r| r == region) {
        return None;
    }
    if bundle.scenery.is_none() && scenery_tables_waited(bundle) {
        return None;
    }
    Some(build_frill_sector(ctx, bundle, sx, sy, density, thinner))
}

/// Whether `bundle` was built without scenery because its region's tables were
/// unavailable for a reason that is not the data's (its transient "scenery" problem,
/// `scenery.md` §4.1).
fn scenery_tables_waited(bundle: &LandblockBundle) -> bool {
    bundle
        .problems
        .iter()
        .any(|p| p.transient && p.what == "scenery")
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

#[cfg(test)]
#[path = "streamer_tests.rs"]
mod tests;
