// SPDX-License-Identifier: GPL-3.0-or-later
//! Scripted walks (`tessera-client --walk-path FILE`, `m4-5-visual.md` §2): a walk runs
//! headless, without a GPU, on the physics walker over the world's collision residency, and
//! writes a movement log per step (position, state, cell, contacts).
//!
//! 1. The feet are put at the walk's `start` (in its landblock's frame), 0.01 m up so that
//!    a start height rounded to the centimetre never begins inside the support (within the
//!    0.02 m of §2.1), facing its heading.
//! 2. The world streams around the feet until every set the first step needs is installed
//!    (decision 0013: scripted runs start only then; worker timing changes when a step
//!    runs, never what it computes). The walk's own landblock, the owner of its cells, is
//!    required by every step and kept resident wherever it lies
//!    ([`PhysicsWalker::set_required`], [`WalkWorld::update`]); a walk whose `cell` is not 0
//!    starts in that cell ([`PhysicsWalker::set_cell`]).
//! 3. [`SETTLE_SECONDS`] of steps without input, then the walk's duration in steps of
//!    [`STEP_SECONDS`], each with the command of the [`WalkScript`] for its time.
//! 4. The log is checked against the walk's expectation ([`evaluate`], §2.2): at every step
//!    the capsule penetrates nothing deeper than [`PENETRATION_LIMIT`] and the state is
//!    finite, and the kind's own condition holds.

use std::fmt::Write as _;
use std::sync::Arc;
use std::time::Duration;

use tessera_physics::geometry::{CollisionSet, CollisionView, GridMesh};
use tessera_world::{LandblockKey, WorldPos, WorldStreamer};

use super::setup::MoverSetup;
use super::{
    BlockReadiness, CellOwnerIndex, CollisionWorld, MoveInput, MovementState, PhysicsWalker,
    PinStreamers, PinnedCollision, STEP_SECONDS, StepRecord, VIEW_RADIUS,
};
use crate::wait::{LoadWait, Waiter};
use crate::walk_path::{Expect, SETTLE_SECONDS, Walk, WalkScript, steps_for};

/// Deepest penetration allowed at any step, m (ROADMAP M4.5 criterion 4).
pub const PENETRATION_LIMIT: f32 = 0.01;
/// Vertical tolerance of end points (checklist §2.2), m.
pub const END_DZ: f64 = 0.05;
/// How far the feet may stay from the float line once floating (checklist §2.2
/// `wade_swim`), m.
pub const FLOAT_TOLERANCE: f64 = 0.25;
/// Seconds after the first swimming step from which the float line is checked (§2.2).
pub const FLOAT_AFTER: f64 = 3.0;
/// The start is placed this far above `start` (within the 0.02 m of §2.1).
pub const START_LIFT: f64 = 0.01;
/// A `follow` walk counts as blocked at a step where forward is held, at least
/// [`BLOCK_GRACE`] after the inputs began and more than [`BLOCK_END_RANGE`] from its end
/// point, and the horizontal speed is below this fraction of the layer's forward speed
/// (Tessera reading of §2.2 "never blocked").
pub const BLOCKED_SPEED_FRACTION: f32 = 0.5;
/// See [`BLOCKED_SPEED_FRACTION`], s.
pub const BLOCK_GRACE: f64 = 0.5;
/// See [`BLOCKED_SPEED_FRACTION`], m.
pub const BLOCK_END_RANGE: f64 = 1.0;
/// Streamer updates while the walk steps happen at most every this many steps (and
/// whenever a set is missing).
pub const STREAM_EVERY: u64 = 30;

/// A world a walk runs in: its collision residency, streamed around the feet.
pub trait WalkWorld: CollisionWorld {
    /// Streams around `focus` (one update; installs finished sets) and keeps `pins`
    /// ([`PhysicsWalker::pins`]: the walk's own landblock, the owner of the current cell)
    /// resident wherever they lie.
    fn update(&mut self, focus: &WorldPos, pins: &[LandblockKey]);

    /// A progress mark of the loading behind [`WalkWorld::update`]: it changes whenever
    /// some loading work starts, finishes or is given up (waits on progress compare it,
    /// [`LoadWait::Progress`]). Worlds that load nothing return a constant.
    fn load_progress(&self) -> u64 {
        0
    }
}

pub use super::MAX_PINS;

/// A [`WorldStreamer`] as a walk world, with one small streamer per pinned landblock
/// ([`WalkWorld::update`], [`PinStreamers`]) so that a cell owner several landblocks from
/// the feet, beyond the rings, is installed too. Every view holds the pinned sets, whether
/// or not their cells reach the feet, so what a step sees does not depend on the rings
/// ([`PinnedCollision`]).
pub struct StreamingWorld {
    /// The streamer around the feet.
    pub streamer: WorldStreamer,
    pins: PinStreamers,
    /// The owners of cells reaching the feet from further away (`walker.md` §13.3),
    /// pinned with the walker's own pins.
    owners: CellOwnerIndex,
}

impl std::fmt::Debug for StreamingWorld {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("StreamingWorld")
            .field("pins", &self.pins.pinned())
            .finish_non_exhaustive()
    }
}

impl StreamingWorld {
    /// A walk world over `streamer`; with `pool` (the workers `streamer` uses), pinned
    /// landblocks get their own streamers, without it only the rings stream.
    #[must_use]
    pub fn new(streamer: WorldStreamer, pool: Option<tessera_assets::WorkerPool>) -> Self {
        let owners = CellOwnerIndex::from_context(streamer.context());
        Self {
            streamer,
            pins: PinStreamers::new(pool),
            owners,
        }
    }

    /// The landblocks pinned now.
    #[must_use]
    pub fn pinned(&self) -> Vec<LandblockKey> {
        self.pins.pinned()
    }

    fn world(&self) -> PinnedCollision<'_> {
        PinnedCollision {
            main: &self.streamer,
            pins: &self.pins,
            owners: Some(&self.owners),
        }
    }
}

impl CollisionWorld for StreamingWorld {
    fn readiness(&self, key: LandblockKey) -> BlockReadiness {
        self.world().readiness_of(key)
    }

    fn cell_owners(&self, feet: LandblockKey) -> Option<Vec<LandblockKey>> {
        self.owners.owners(feet)
    }

    fn view(&self, centre: &WorldPos) -> CollisionView<'_> {
        self.world().view_at(centre)
    }

    fn terrain_height(&self, pos: &WorldPos) -> Option<f32> {
        self.world().terrain_height_at(pos)
    }

    fn terrain_normal(&self, pos: &WorldPos) -> Option<[f32; 3]> {
        self.world().terrain_normal_at(pos)
    }

    fn portal_neighbours(&self, owner: LandblockKey, cell: u16) -> Vec<u16> {
        self.world().portal_neighbours_of(owner, cell)
    }

    fn collision_set(&self, key: LandblockKey) -> Option<&CollisionSet> {
        self.world().collision_of(key).map(|c| &*c.set)
    }
}

impl WalkWorld for StreamingWorld {
    fn update(&mut self, focus: &WorldPos, pins: &[LandblockKey]) {
        let _ = self.streamer.update(*focus);
        let context = Arc::clone(self.streamer.context());
        let mut pins = pins.to_vec();
        if let Some(feet) = focus.landblock() {
            self.owners.update(feet);
            for k in self.owners.owners(feet).unwrap_or_default() {
                if !pins.contains(&k) {
                    pins.push(k);
                }
            }
        }
        self.pins.update(&context, &pins);
    }

    fn load_progress(&self) -> u64 {
        self.streamer
            .stats()
            .progress()
            .wrapping_add(self.pins.load_progress())
            .wrapping_add(self.owners.load_progress())
    }
}

/// Fixed collision sets by landblock (synthetic tests, tools): every listed landblock is
/// installed, every other one absent.
#[derive(Debug, Default, Clone)]
pub struct StaticWorld {
    /// The sets.
    pub sets: std::collections::BTreeMap<LandblockKey, Arc<CollisionSet>>,
    /// The cells each cell's portals lead to (owner landblock, cell), for
    /// [`CollisionWorld::portal_neighbours`].
    pub portals: std::collections::BTreeMap<(LandblockKey, u16), Vec<u16>>,
}

impl CollisionWorld for StaticWorld {
    fn readiness(&self, key: LandblockKey) -> BlockReadiness {
        if self.sets.contains_key(&key) {
            BlockReadiness::Ready
        } else {
            BlockReadiness::Absent
        }
    }

    fn view(&self, centre: &WorldPos) -> CollisionView<'_> {
        let Some(c) = centre.landblock() else {
            return CollisionView::new();
        };
        let o = c.origin();
        let mut view = CollisionView::with_origin([o[0], o[1], 0.0]);
        let r = i32::try_from(VIEW_RADIUS).unwrap_or(1);
        for dy in -r..=r {
            for dx in -r..=r {
                if let Some(set) = c.offset(dx, dy).and_then(|k| self.sets.get(&k)) {
                    let _ = view.push_landblock(set);
                }
            }
        }
        view
    }

    fn terrain_height(&self, pos: &WorldPos) -> Option<f32> {
        let l = pos.to_local()?;
        let set = self.sets.get(&l.block)?;
        grid_height(set.terrain()?, l.local[0], l.local[1])
    }

    fn terrain_normal(&self, pos: &WorldPos) -> Option<[f32; 3]> {
        let l = pos.to_local()?;
        let set = self.sets.get(&l.block)?;
        grid_normal(set.terrain()?, l.local[0], l.local[1])
    }

    fn portal_neighbours(&self, owner: LandblockKey, cell: u16) -> Vec<u16> {
        self.portals
            .get(&(owner, cell))
            .cloned()
            .unwrap_or_default()
    }

    fn collision_set(&self, key: LandblockKey) -> Option<&CollisionSet> {
        self.sets.get(&key).map(|s| &**s)
    }
}

/// The height of `mesh` at (x, y): the triangle whose projection contains the point,
/// interpolated on its plane (a linear scan over the 2,048 triangles; for synthetic worlds).
#[must_use]
pub fn grid_height(mesh: &GridMesh, x: f32, y: f32) -> Option<f32> {
    let eps = 1e-4;
    for (_, [a, b, c]) in mesh.triangles() {
        let d = (b[1] - c[1]) * (a[0] - c[0]) + (c[0] - b[0]) * (a[1] - c[1]);
        if d.abs() < 1e-12 {
            continue;
        }
        let l1 = ((b[1] - c[1]) * (x - c[0]) + (c[0] - b[0]) * (y - c[1])) / d;
        let l2 = ((c[1] - a[1]) * (x - c[0]) + (a[0] - c[0]) * (y - c[1])) / d;
        let l3 = 1.0 - l1 - l2;
        if l1 >= -eps && l2 >= -eps && l3 >= -eps {
            return Some(l1 * a[2] + l2 * b[2] + l3 * c[2]);
        }
    }
    None
}

/// The unit upward normal of the triangle of `mesh` whose projection contains (x, y)
/// (the triangle [`grid_height`] uses; for synthetic worlds). `None` off the mesh or for a
/// vertical triangle.
#[must_use]
pub fn grid_normal(mesh: &GridMesh, x: f32, y: f32) -> Option<[f32; 3]> {
    let eps = 1e-4;
    for (_, [a, b, c]) in mesh.triangles() {
        let d = (b[1] - c[1]) * (a[0] - c[0]) + (c[0] - b[0]) * (a[1] - c[1]);
        if d.abs() < 1e-12 {
            continue;
        }
        let l1 = ((b[1] - c[1]) * (x - c[0]) + (c[0] - b[0]) * (y - c[1])) / d;
        let l2 = ((c[1] - a[1]) * (x - c[0]) + (a[0] - c[0]) * (y - c[1])) / d;
        let l3 = 1.0 - l1 - l2;
        if l1 >= -eps && l2 >= -eps && l3 >= -eps {
            let (u, v) = (
                [b[0] - a[0], b[1] - a[1], b[2] - a[2]],
                [c[0] - a[0], c[1] - a[1], c[2] - a[2]],
            );
            let mut n = [
                u[1] * v[2] - u[2] * v[1],
                u[2] * v[0] - u[0] * v[2],
                u[0] * v[1] - u[1] * v[0],
            ];
            if n[2] < 0.0 {
                n = n.map(|c| -c);
            }
            let len = (n[0] * n[0] + n[1] * n[1] + n[2] * n[2]).sqrt();
            return (len > 0.0 && n[2] > 0.0).then(|| n.map(|c| c / len));
        }
    }
    None
}

impl WalkWorld for StaticWorld {
    fn update(&mut self, _focus: &WorldPos, _pins: &[LandblockKey]) {}
}

/// Why a walk could not run (as opposed to running and failing its expectation).
#[derive(Debug, Clone, PartialEq, Eq, thiserror::Error)]
pub enum RunError {
    /// The walk's landblock is not on the grid.
    #[error("the walk's landblock is not on the grid")]
    BadLandblock,
    /// The start is not finite.
    #[error("the walk's start is not finite")]
    BadStart,
    /// The sets the walk needs were not installed before the wait gave up.
    #[error("collision sets not installed after {waited} s: {}", key_list(missing))]
    NotLoaded {
        /// Seconds waited.
        waited: u64,
        /// The landblocks still missing.
        missing: Vec<LandblockKey>,
    },
}

fn key_list(keys: &[LandblockKey]) -> String {
    keys.iter()
        .map(ToString::to_string)
        .collect::<Vec<_>>()
        .join(", ")
}

/// One step of a walk's movement log.
#[derive(Debug, Clone, PartialEq)]
pub struct WalkRow {
    /// Seconds after the settle period (negative while settling).
    pub t: f64,
    /// Feet in the frame of the walk's landblock.
    pub local: [f64; 3],
    /// Terrain height under the feet (`landblock.md` §3.11), when resident.
    pub terrain: Option<f32>,
    /// The smallest up component of the terrain normals within the capsule's radius of
    /// the feet (the steepest terrain there, [`steepest_terrain_normal_z`]), when resident;
    /// for the slope allowance of [`terrain_allowance`].
    pub terrain_normal_z: Option<f32>,
    /// Forward speed of the layer in force (m/s).
    pub forward_speed: f32,
    /// The step.
    pub record: StepRecord,
}

impl WalkRow {
    /// Horizontal speed after the step, m/s.
    #[must_use]
    pub fn horizontal_speed(&self) -> f32 {
        let v = self.record.state.velocity;
        v[0].hypot(v[1])
    }

    /// Whether the controller's state is a water state (1, 2 or 3).
    #[must_use]
    pub fn in_water(&self) -> bool {
        self.record.state.element.in_water()
    }
}

/// The outcome of one walk.
#[derive(Debug, Clone)]
pub struct WalkOutcome {
    /// The walk.
    pub walk: Walk,
    /// The movement log: settle steps, then input steps.
    pub rows: Vec<WalkRow>,
    /// The pass condition's result.
    pub verdict: Verdict,
    /// Horizontal and vertical distance of the feet from `start` after settling, m.
    pub settle_drift: [f64; 2],
}

/// Whether a walk passed, and why not.
#[derive(Debug, Clone, Default, PartialEq)]
pub struct Verdict {
    /// Every condition held.
    pub passed: bool,
    /// What failed (empty when passed).
    pub failures: Vec<String>,
    /// The walk failed by less than twice a tolerance (checklist §2.2: reported to the
    /// spec writer, who re-checks the row before the implementation is changed).
    pub near_miss: bool,
    /// Deepest penetration over the walk, m.
    pub max_penetration: f32,
    /// Measured facts, for the report.
    pub notes: Vec<String>,
}

/// Runs `walk` in `world` with `setup`: places, waits for the sets (each wait limited by
/// `wait`), settles, steps and evaluates (module docs).
///
/// # Errors
///
/// [`RunError`] when the walk cannot run at all.
pub fn run_walk<W: WalkWorld>(
    walk: &Walk,
    world: &mut W,
    setup: &MoverSetup,
    wait: LoadWait,
) -> Result<WalkOutcome, RunError> {
    let block = LandblockKey::new(walk.region, walk.landblock.0, walk.landblock.1)
        .map_err(|_| RunError::BadLandblock)?;
    let o = block.origin();
    let start = [
        o[0] + walk.start[0],
        o[1] + walk.start[1],
        walk.start[2] + START_LIFT,
    ];
    if !start.iter().all(|v| v.is_finite()) {
        return Err(RunError::BadStart);
    }
    let to_local = |p: [f64; 3]| [p[0] - o[0], p[1] - o[1], p[2]];
    let yaw = yaw_of(walk.heading);
    let mut walker = PhysicsWalker::new(setup, walk.region, start, yaw);
    walker.set_details(true);
    walker.set_required(&[block]);
    if walk.cell != 0 {
        walker.set_cell(Some((block, walk.cell)));
    }
    let walking = walk.mode.walking();
    let mut rows = Vec::new();
    let settle = steps_for(SETTLE_SECONDS, f64::from(STEP_SECONDS));
    let total = settle + walk.input_steps(f64::from(STEP_SECONDS));
    let mut script = WalkScript::new(walk);
    // Before the first step: the walk's own landblock (the owner of its cells, which may
    // reach several landblocks from its square, `collision.md` §5.8.1; required by every
    // step and pinned) and the landblocks around the feet, so that no set appears under the
    // mover after it started.
    let mut first: Vec<LandblockKey> = vec![block];
    if let Some(k) = walker.feet().landblock() {
        first.extend((-1..=1).flat_map(|dy| (-1..=1).filter_map(move |dx| k.offset(dx, dy))));
    }
    wait_blocks(&walker, world, &first, wait)?;
    for k in 0..total {
        if k.is_multiple_of(STREAM_EVERY) {
            world.update(&walker.feet(), &walker.pins());
        }
        wait_ready(&mut walker, world, wait)?;
        #[allow(clippy::cast_precision_loss)]
        let t = (k as f64 - settle as f64) * f64::from(STEP_SECONDS);
        let input = if k < settle {
            MoveInput::default()
        } else {
            let p = to_local(walker.state().position);
            let c = script.command(t, [p[0], p[1]]);
            MoveInput {
                forward: c.forward,
                strafe: c.strafe,
                heading: c.heading.map(yaw_of),
                jump: c.jump,
            }
        };
        let forward_speed = walker.controller().params().forward_speed;
        let Some(record) = walker.step(&input, walking, world).cloned() else {
            continue;
        };
        let feet = record.feet();
        rows.push(WalkRow {
            t,
            local: to_local(record.state.position),
            terrain: world.terrain_height(&feet),
            terrain_normal_z: steepest_terrain_normal_z(world, &feet, setup.shape.radius()),
            forward_speed,
            record,
        });
    }
    let settle_drift = rows
        .iter()
        .take_while(|r| r.t < 0.0)
        .last()
        .map_or([0.0; 2], |r| {
            [
                (r.local[0] - walk.start[0]).hypot(r.local[1] - walk.start[1]),
                r.local[2] - walk.start[2],
            ]
        });
    let verdict = evaluate(walk, &rows, block, setup.shape.radius());
    Ok(WalkOutcome {
        walk: walk.clone(),
        rows,
        verdict,
        settle_drift,
    })
}

/// The yaw (radians, 0 = east, counter-clockwise) of compass heading `h` degrees
/// (`coordinates.md` §11.4: 0 = north, 90 = east).
#[must_use]
pub fn yaw_of(h: f64) -> f32 {
    let d = (90.0 - h).rem_euclid(360.0);
    let d = if d > 180.0 { d - 360.0 } else { d };
    #[allow(clippy::cast_possible_truncation)]
    let r = d.to_radians() as f32;
    if r.is_finite() { r } else { 0.0 }
}

/// Streams around `walker`'s feet until none of `keys` is pending, or fails when `wait`
/// gives up (with [`LoadWait::Progress`]: polls about 1 ms apart in which neither the
/// world's [`WalkWorld::load_progress`] nor the missing landblocks changed).
///
/// # Errors
///
/// [`RunError::NotLoaded`] when the wait gives up.
pub fn wait_blocks<W: WalkWorld>(
    walker: &PhysicsWalker,
    world: &mut W,
    keys: &[LandblockKey],
    wait: LoadWait,
) -> Result<(), RunError> {
    let mut waiter = Waiter::new(wait);
    loop {
        world.update(&walker.feet(), &walker.pins());
        let missing: Vec<LandblockKey> = keys
            .iter()
            .copied()
            .filter(|k| world.readiness(*k) == BlockReadiness::Pending)
            .collect();
        if missing.is_empty() {
            return Ok(());
        }
        if waiter.give_up((world.load_progress(), missing.clone())) {
            return Err(RunError::NotLoaded {
                waited: waiter.elapsed().as_secs(),
                missing,
            });
        }
        std::thread::sleep(Duration::from_millis(1));
    }
}

/// Streams until the walker's next step can run, or fails when `wait` gives up (as
/// [`wait_blocks`]).
fn wait_ready<W: WalkWorld>(
    walker: &mut PhysicsWalker,
    world: &mut W,
    wait: LoadWait,
) -> Result<(), RunError> {
    if walker.pending(world).is_empty() {
        return Ok(());
    }
    let mut waiter = Waiter::new(wait);
    loop {
        world.update(&walker.feet(), &walker.pins());
        let missing = walker.pending(world);
        if missing.is_empty() {
            return Ok(());
        }
        if waiter.give_up((world.load_progress(), missing.clone())) {
            return Err(RunError::NotLoaded {
                waited: waiter.elapsed().as_secs(),
                missing,
            });
        }
        std::thread::sleep(Duration::from_millis(1));
    }
}

fn hdist(a: [f64; 3], b: [f64; 3]) -> f64 {
    (a[0] - b[0]).hypot(a[1] - b[1])
}

fn hdist2(a: [f64; 3], b: [f64; 2]) -> f64 {
    (a[0] - b[0]).hypot(a[1] - b[1])
}

/// Directions sampled on each ring of [`steepest_terrain_normal_z`].
const SLOPE_RING_SAMPLES: u32 = 16;

/// The smallest up component of the terrain normals (`landblock.md` §3.11) at `feet` and
/// at points on two rings around it (radius `radius` and `radius` / 2, 16 directions
/// each): the steepest terrain triangle a capsule of that radius standing at `feet` can
/// rest on. `None` when the terrain under `feet` is not resident.
pub fn steepest_terrain_normal_z<W: CollisionWorld + ?Sized>(
    world: &W,
    feet: &WorldPos,
    radius: f32,
) -> Option<f32> {
    let mut lowest = world.terrain_normal(feet)?[2];
    let r = f64::from(radius);
    if !(r.is_finite() && r > 0.0) {
        return Some(lowest);
    }
    for ring in [r, 0.5 * r] {
        for k in 0..SLOPE_RING_SAMPLES {
            let a = std::f64::consts::TAU * f64::from(k) / f64::from(SLOPE_RING_SAMPLES);
            let p = WorldPos::new(
                feet.region,
                feet.x + ring * a.cos(),
                feet.y + ring * a.sin(),
                feet.z,
            );
            if let Some(n) = world.terrain_normal(&p) {
                lowest = lowest.min(n[2]);
            }
        }
    }
    Some(lowest)
}

/// How far above the terrain under its axis a capsule of radius `radius` may rest when the
/// steepest terrain within `radius` of the axis has a unit normal with the up component
/// `normal_z`, m: the geometric offset (`radius` + *c*) · sec *θ* − `radius` of
/// `collision.md` §11.7 with Tessera's convex radius *c* = 0 (§12.5), so
/// `radius` · (1 / `normal_z` − 1). On a plane it is the exact rest offset; on any terrain
/// whose slope within the radius is at most *θ* it bounds it, because the surface there
/// lies at most tan *θ* · *ρ* above the point under the axis at horizontal distance *ρ*, and
/// max over *ρ* of tan *θ* · *ρ* + √(*r*² − *ρ*²) is *r* · sec *θ*. 0 for a missing,
/// non-finite or non-positive `normal_z` (checklist §2.2, `follow` and `pass`).
#[must_use]
pub fn terrain_allowance(radius: f32, normal_z: Option<f32>) -> f64 {
    match normal_z {
        Some(nz) if nz.is_finite() && nz > 0.0 && radius.is_finite() && radius > 0.0 => {
            let a = f64::from(radius) * (1.0 / f64::from(nz.min(1.0)) - 1.0);
            if a.is_finite() { a.max(0.0) } else { 0.0 }
        }
        _ => 0.0,
    }
}

/// The step at which the feet are worst outside the band [−`dz`, `dz` + the slope allowance
/// of [`terrain_allowance`]] around the terrain height under them (checklist §2.2, `follow`
/// and `pass`): the step, the offset feet − terrain and the excess over the band. `None`
/// when every step with a terrain height lies within it.
fn terrain_band_miss(rows: &[WalkRow], dz: f64, radius: f32) -> Option<(&WalkRow, f64, f64)> {
    rows.iter()
        .filter_map(|r| {
            let off = r.local[2] - f64::from(r.terrain?);
            let excess = if off < 0.0 {
                -off - dz
            } else {
                off - dz - terrain_allowance(radius, r.terrain_normal_z)
            };
            (excess > 0.0).then_some((r, off, excess))
        })
        .max_by(|a, b| a.2.total_cmp(&b.2))
}

/// Checks `rows` against `walk`'s expectation (checklist §2.2; module docs). `block` is the
/// walk's landblock (the owner of its cells); `radius` the mover's capsule radius (for the
/// slope allowance of `follow` and `pass`).
#[must_use]
pub fn evaluate(walk: &Walk, rows: &[WalkRow], block: LandblockKey, radius: f32) -> Verdict {
    let mut v = Verdict::default();
    // Failures within twice their tolerance; the walk is a near miss when every failure is.
    let mut near_count = 0usize;
    let mut near = |v: &mut Verdict, what: String, miss: f64, tol: f64| {
        if miss <= 2.0 * tol {
            near_count += 1;
        }
        v.failures.push(what);
    };
    let Some(end) = rows.last() else {
        v.failures.push("no step ran".into());
        return v;
    };
    // Every step: penetration and finiteness.
    v.max_penetration = rows
        .iter()
        .map(|r| r.record.penetration)
        .fold(0.0, f32::max);
    if v.max_penetration > PENETRATION_LIMIT {
        let worst = rows
            .iter()
            .max_by(|a, b| a.record.penetration.total_cmp(&b.record.penetration))
            .map_or(String::new(), |r| {
                format!(
                    " at t={:.3} into {}",
                    r.t,
                    r.record
                        .contacts
                        .first()
                        .map_or("?".to_owned(), |c| c.kind.to_string())
                )
            });
        v.failures.push(format!(
            "penetration {:.4} m > {PENETRATION_LIMIT} m{worst}",
            v.max_penetration
        ));
    }
    if let Some(r) = rows.iter().find(|r| !r.record.state.is_finite()) {
        v.failures.push(format!("non-finite state at t={:.3}", r.t));
    }
    let moving: Vec<&WalkRow> = rows.iter().filter(|r| r.t >= 0.0).collect();
    // The state after the settle (`walker.md` §13.6): the last settle step, else the first.
    let start = rows
        .iter()
        .take_while(|r| r.t < 0.0)
        .last()
        .or(rows.first())
        .unwrap_or(end);
    let start_cell = start
        .record
        .cell
        .map_or(0, |(k, c)| if k == block { c } else { u16::MAX });
    if start_cell != walk.cell {
        v.notes.push(format!(
            "cell after the settle {start_cell}, the row's cell {}",
            walk.cell
        ));
    }
    let s = walk.start;
    let dir = {
        let h = walk.heading.to_radians();
        [h.sin(), h.cos()]
    };
    let along = |p: [f64; 3]| (p[0] - s[0]) * dir[0] + (p[1] - s[1]) * dir[1];
    let e = end.local;
    match &walk.expect {
        Expect::Blocked { p, d } => {
            // §2.2 as revised from `collision.md` §11.7: the stop is where the walker first
            // touches the blocker; with forward held it may then slide along or round it.
            let touch = moving
                .iter()
                .position(|r| r.record.input.forward > 0.0 && r.record.obstacle.is_some());
            v.notes.push(format!(
                "end {:.3} m from P, dz {:.3}",
                hdist(e, *p),
                e[2] - p[2]
            ));
            if let Some(f) = first_stop(&moving) {
                v.notes.push(format!(
                    "first stop {:.3} m from P, dz {:.3}",
                    hdist(f, *p),
                    f[2] - p[2]
                ));
            }
            match touch.and_then(|i| moving.get(i).map(|r| (i, r))) {
                None => v.failures.push("never touched a blocker".into()),
                Some((i, r)) => {
                    let f = r.local;
                    let h = hdist(f, *p);
                    // P.z is the support height under the axis (SWEEP); a capsule on a
                    // slope rests above it (`collision.md` §11.7), so the band of
                    // `follow` and `pass` applies.
                    let rise = f[2] - p[2];
                    let z = if rise < 0.0 {
                        -rise
                    } else {
                        rise - terrain_allowance(radius, r.terrain_normal_z)
                    };
                    let kind = r
                        .record
                        .obstacle
                        .map_or("?".to_owned(), |c| c.kind.to_string());
                    v.notes.push(format!(
                        "first touch of {kind} at t={:.3}, {h:.3} m from P, dz {:.3}",
                        r.t,
                        f[2] - p[2]
                    ));
                    if h > *d {
                        near(
                            &mut v,
                            format!("first touch {h:.3} m from P (d {d})"),
                            h - d,
                            *d,
                        );
                    }
                    if z > END_DZ {
                        near(
                            &mut v,
                            format!(
                                "first touch {rise:.3} m above/below P (slope allowance {:.3})",
                                terrain_allowance(radius, r.terrain_normal_z)
                            ),
                            z - END_DZ,
                            END_DZ,
                        );
                    }
                    let limit = along(*p) + d;
                    if let Some(q) = moving.iter().take(i + 1).find(|q| along(q.local) > limit) {
                        let over = along(q.local) - limit;
                        near(
                            &mut v,
                            format!("passed the stop point by {over:.3} m at t={:.3}", q.t),
                            over,
                            *d,
                        );
                    }
                }
            }
        }
        Expect::Reach {
            e: target,
            d,
            risers,
        } => {
            let (h, z) = (hdist(e, *target), (e[2] - target[2]).abs());
            let ups = moving.iter().filter(|r| r.record.report.stepped_up).count();
            v.notes.push(format!(
                "end {h:.3} m from E, dz {:.3}; {ups} step-ups for risers {risers:?}",
                e[2] - target[2]
            ));
            if h > *d {
                near(&mut v, format!("ends {h:.3} m from E (d {d})"), h - d, *d);
            }
            if z > END_DZ {
                near(
                    &mut v,
                    format!("ends {z:.3} m above/below E"),
                    z - END_DZ,
                    END_DZ,
                );
            }
            if moving.iter().any(|r| r.record.report.jumped) {
                v.failures.push("jumped".into());
            }
        }
        Expect::NoClimb { b, zmax, rmax } => {
            let top = moving.iter().map(|r| r.local[2]).fold(f64::MIN, f64::max);
            let reach = along(e);
            let slid = moving
                .iter()
                .any(|r| r.record.state.element == MovementState::EarthUncontrolledSurface);
            v.notes.push(format!(
                "highest feet {top:.3} (zmax {zmax}, B.z {}), along-path end {reach:.3} (rmax {rmax}), slid {slid}",
                b[2]
            ));
            if top > *zmax {
                near(
                    &mut v,
                    format!("climbed to {top:.3} > zmax {zmax}"),
                    top - zmax,
                    0.6,
                );
            }
            if reach > *rmax {
                near(
                    &mut v,
                    format!("ended {reach:.3} m along the path > rmax {rmax}"),
                    reach - rmax,
                    0.6,
                );
            }
            if !slid {
                v.failures
                    .push("never in state 6 (Earth_UncontrolledSurface)".into());
            }
        }
        Expect::Cells {
            from,
            to,
            e: target,
            d,
        } => {
            let cell_of = |r: &WalkRow| {
                r.record
                    .cell
                    .map_or(0, |(k, c)| if k == block { c } else { u16::MAX })
            };
            let (a, z_end) = (start_cell, cell_of(end));
            let (h, z) = (hdist(e, *target), (e[2] - target[2]).abs());
            v.notes.push(format!(
                "cells {a}>{z_end}, end {h:.3} m from E, dz {:.3}",
                e[2] - target[2]
            ));
            if a != *from {
                v.failures.push(format!("start cell {a}, expected {from}"));
            }
            if z_end != *to {
                v.failures.push(format!("end cell {z_end}, expected {to}"));
            }
            if h > *d {
                near(&mut v, format!("ends {h:.3} m from E (d {d})"), h - d, *d);
            }
            if z > END_DZ {
                near(
                    &mut v,
                    format!("ends {z:.3} m above/below E"),
                    z - END_DZ,
                    END_DZ,
                );
            }
        }
        Expect::Follow {
            e: target,
            d,
            dz,
            headroom,
        } => {
            // "At every step" (§2.2): the settle steps too, as for penetration.
            let worst = rows
                .iter()
                .filter_map(|r| r.terrain.map(|t| (r, (r.local[2] - f64::from(t)).abs())))
                .max_by(|a, b| a.1.total_cmp(&b.1));
            let miss = terrain_band_miss(rows, *dz, radius);
            let no_terrain = rows.iter().filter(|r| r.terrain.is_none()).count();
            let blocked = moving.iter().find(|r| {
                r.record.input.forward > 0.0
                    && r.t >= BLOCK_GRACE
                    && hdist(r.local, *target) > BLOCK_END_RANGE
                    && r.horizontal_speed() < BLOCKED_SPEED_FRACTION * r.forward_speed
            });
            let h = hdist(e, *target);
            v.notes.push(format!(
                "largest |feet − terrain| {:.3} (dz {dz}), end {h:.3} m from E, dz {:.3}, headroom {headroom}",
                worst.map_or(0.0, |w| w.1),
                e[2] - target[2]
            ));
            if let Some((r, off, excess)) = miss {
                near(
                    &mut v,
                    format!(
                        "{off:.3} m off the terrain at t={:.3} ({excess:.3} m outside dz and the slope allowance {:.3})",
                        r.t,
                        terrain_allowance(radius, r.terrain_normal_z)
                    ),
                    excess,
                    *dz,
                );
            }
            if no_terrain > 0 {
                v.failures
                    .push(format!("{no_terrain} steps without terrain under the feet"));
            }
            if let Some(r) = blocked {
                v.failures.push(format!(
                    "blocked at t={:.3} ({:.2} m/s)",
                    r.t,
                    r.horizontal_speed()
                ));
            }
            // §2.2: `follow` ends within d of E horizontally; the height is checked at
            // every step against the terrain instead.
            if h > *d {
                near(&mut v, format!("ends {h:.3} m from E (d {d})"), h - d, *d);
            }
        }
        Expect::WadeSwim {
            entry,
            swim,
            h,
            fh,
            d,
        } => {
            let first_wet = rows.iter().find(|r| r.in_water());
            let first_swim = rows
                .iter()
                .find(|r| r.record.state.element == MovementState::WaterNoSurface);
            let line = h - fh;
            match first_wet {
                Some(r) => {
                    let m = hdist2(r.local, *entry);
                    v.notes
                        .push(format!("water at t={:.3}, {m:.3} m from in", r.t));
                    if m > *d {
                        near(
                            &mut v,
                            format!("entered water {m:.3} m from in (d {d})"),
                            m - d,
                            *d,
                        );
                    }
                }
                None => v.failures.push("never in water".into()),
            }
            match first_swim {
                Some(r) => {
                    let m = hdist2(r.local, *swim);
                    v.notes
                        .push(format!("swimming at t={:.3}, {m:.3} m from swim", r.t));
                    if m > *d {
                        near(
                            &mut v,
                            format!("started swimming {m:.3} m from swim (d {d})"),
                            m - d,
                            *d,
                        );
                    }
                    let after = r.t + FLOAT_AFTER;
                    let worst = rows
                        .iter()
                        .filter(|q| q.t >= after)
                        .map(|q| (q, (q.local[2] - line).abs()))
                        .max_by(|a, b| a.1.total_cmp(&b.1));
                    if let Some((q, off)) = worst {
                        v.notes.push(format!(
                            "largest |feet − (h − fh)| after {after:.2} s: {off:.3}"
                        ));
                        if off > FLOAT_TOLERANCE {
                            near(
                                &mut v,
                                format!("{off:.3} m from the float line at t={:.3}", q.t),
                                off - FLOAT_TOLERANCE,
                                FLOAT_TOLERANCE,
                            );
                        }
                    } else {
                        v.failures
                            .push(format!("the walk ends before {FLOAT_AFTER} s of swimming"));
                    }
                }
                None => v.failures.push("never swimming (state 2)".into()),
            }
        }
        Expect::Pass { e: target, d } => {
            let (h, z) = (hdist(e, *target), (e[2] - target[2]).abs());
            // "At any step" / "at every step" (§2.2): the settle steps too.
            let touched = rows.iter().find_map(|r| {
                r.record
                    .contacts
                    .iter()
                    .chain(r.record.obstacle.iter())
                    .find(|c| c.kind.is_object())
                    .map(|c| (r.t, c.kind))
            });
            let miss = terrain_band_miss(rows, END_DZ, radius);
            let worst = rows
                .iter()
                .filter_map(|r| r.terrain.map(|t| (r, (r.local[2] - f64::from(t)).abs())))
                .max_by(|a, b| a.1.total_cmp(&b.1));
            v.notes.push(format!(
                "end {h:.3} m from E, dz {:.3}; largest |feet − terrain| {:.3}",
                e[2] - target[2],
                worst.map_or(0.0, |w| w.1)
            ));
            if h > *d {
                near(&mut v, format!("ends {h:.3} m from E (d {d})"), h - d, *d);
            }
            if z > END_DZ {
                near(
                    &mut v,
                    format!("ends {z:.3} m above/below E"),
                    z - END_DZ,
                    END_DZ,
                );
            }
            if let Some((t, k)) = touched {
                v.failures.push(format!("touched {k} at t={t:.3}"));
            }
            if let Some((r, off, excess)) = miss {
                near(
                    &mut v,
                    format!(
                        "{off:.3} m off the terrain at t={:.3} ({excess:.3} m outside 0.05 m and the slope allowance {:.3})",
                        r.t,
                        terrain_allowance(radius, r.terrain_normal_z)
                    ),
                    excess,
                    END_DZ,
                );
            }
        }
    }
    v.passed = v.failures.is_empty();
    v.near_miss = !v.passed && near_count == v.failures.len();
    v
}

/// Where the feet first came to rest after moving (horizontal speed below a tenth of the
/// layer's forward speed while forward is held), for the report.
fn first_stop(rows: &[&WalkRow]) -> Option<[f64; 3]> {
    let mut moving = false;
    for r in rows {
        let slow = r.horizontal_speed() < 0.1 * r.forward_speed;
        if r.record.input.forward > 0.0 && !slow {
            moving = true;
        } else if moving && r.record.input.forward > 0.0 && slow {
            return Some(r.local);
        }
    }
    None
}

/// The header of [`write_log`].
pub const LOG_HEADER: &str = "walk,step,t,x,y,z,vx,vy,vz,heading_deg,state,support,cell,water_depth,forward,strafe,jump,walking,stepped_up,jumped,snapped,planes,pushed_out,penetration,boundary,terrain,contacts";

/// Writes `outcome`'s movement log as CSV rows (no header; see [`LOG_HEADER`]). Positions are
/// in the frame of the walk's landblock; `heading_deg` is the compass heading; `cell` is
/// `0xXXYY:N` (owner landblock and cell) or 0; `contacts` lists `kind@separation` entries
/// separated by spaces.
///
/// # Errors
///
/// I/O errors of `out`.
pub fn write_log(out: &mut dyn std::io::Write, outcome: &WalkOutcome) -> std::io::Result<()> {
    let mut line = String::new();
    for (i, r) in outcome.rows.iter().enumerate() {
        let s = &r.record.state;
        let rep = &r.record.report;
        line.clear();
        let compass = compass_degrees(s.heading);
        let cell = r.record.cell.map_or_else(
            || "0".to_owned(),
            |(k, c)| format!("0x{:02X}{:02X}:{c}", k.x(), k.y()),
        );
        let contacts: Vec<String> = r
            .record
            .contacts
            .iter()
            .map(|c| format!("{}@{:.4}", c.kind, c.separation))
            .collect();
        let _ = write!(
            line,
            "{},{i},{:.4},{:.4},{:.4},{:.4},{:.3},{:.3},{:.3},{:.2},{},{:?},{cell},{},{:.2},{:.2},{},{},{},{},{},{},{:.4},{:.4},{},{},{}",
            outcome.walk.id,
            r.t,
            r.local[0],
            r.local[1],
            r.local[2],
            s.velocity[0],
            s.velocity[1],
            s.velocity[2],
            compass,
            s.element.key(),
            s.support,
            s.water
                .map_or_else(String::new, |w| format!("{:.3}", w.depth)),
            r.record.input.forward,
            r.record.input.strafe,
            u8::from(r.record.input.jump),
            u8::from(r.record.walking),
            u8::from(rep.stepped_up),
            u8::from(rep.jumped),
            u8::from(rep.snapped),
            rep.planes,
            rep.pushed_out,
            r.record.penetration,
            u8::from(r.record.at_boundary),
            r.terrain.map_or_else(String::new, |t| format!("{t:.4}")),
            contacts.join(" "),
        );
        writeln!(out, "{line}")?;
    }
    out.flush()
}

/// The compass heading (degrees, 0 = north, 90 = east) of yaw `yaw` (radians, 0 = east,
/// counter-clockwise), rounded to the log's 0.01° and in [0, 360): f32 rounding of a
/// northward yaw never prints as 360.00.
#[must_use]
pub fn compass_degrees(yaw: f32) -> f64 {
    let c = (90.0 - f64::from(yaw).to_degrees()).rem_euclid(360.0);
    let c = (c * 100.0).round() / 100.0;
    if c >= 360.0 || !c.is_finite() { 0.0 } else { c }
}

/// One line about `outcome`: id, category, result, failures and measured notes.
#[must_use]
pub fn summary_line(outcome: &WalkOutcome) -> String {
    let v = &outcome.verdict;
    format!(
        "{} [{}] {}{} — max penetration {:.4} m; settle drift {:.3} m / {:+.3} m; {}{}",
        outcome.walk.id,
        outcome.walk.category.name(),
        if v.passed { "PASS" } else { "FAIL" },
        if v.near_miss {
            " (near miss: report to the spec writer)"
        } else {
            ""
        },
        v.max_penetration,
        outcome.settle_drift[0],
        outcome.settle_drift[1],
        v.notes.join("; "),
        if v.failures.is_empty() {
            String::new()
        } else {
            format!(" — failures: {}", v.failures.join("; "))
        }
    )
}

/// Everything walks need from an install: the archives, the world context (with scenery,
/// water and collision built per landblock), the asset workers and the mover.
pub struct WalkSession {
    /// The archives.
    pub set: Arc<tessera_dat::ArchiveSet>,
    /// The world context.
    pub ctx: Arc<tessera_world::WorldContext>,
    /// The asset workers.
    pub pool: tessera_assets::WorkerPool,
    /// The mover.
    pub setup: MoverSetup,
}

impl std::fmt::Debug for WalkSession {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("WalkSession")
            .field("setup", &self.setup)
            .finish_non_exhaustive()
    }
}

/// Ring radii of a walk streamer: the mover's landblock and two rings around it, so that
/// every set a step needs (one landblock around) and the owners of nearby interior cells
/// are resident. Owners farther away are pinned ([`WalkWorld::update`]) once known.
#[must_use]
pub fn walk_rings() -> tessera_world::RingConfig {
    tessera_world::RingConfig {
        near: 2.0 * tessera_world::LANDBLOCK_SIZE,
        mid: 3.0 * tessera_world::LANDBLOCK_SIZE,
        far: 3.0 * tessera_world::LANDBLOCK_SIZE,
        hysteresis: tessera_world::LANDBLOCK_SIZE,
    }
}

impl WalkSession {
    /// Opens the install at `root` read-only, loads the world context with the M4.5 world
    /// systems a walker collides with (scenery, water, collision; no environment, figures or
    /// area names), and the mover of `avatar` (`setup::MoverSetup::load`).
    ///
    /// # Errors
    ///
    /// The archives or the world context cannot be opened.
    pub fn open(
        root: &std::path::Path,
        avatar: &crate::avatar::AvatarSpec,
        content_layers: &[u32],
        workers: usize,
    ) -> anyhow::Result<Self> {
        let set = Arc::new(tessera_dat::ArchiveSet::open(
            root,
            tessera_dat::SetOptions::default(),
        )?);
        let options = tessera_world::BundleOptions {
            active_content_layers: content_layers.to_vec(),
            keep_figures: false,
            resolve_area_names: false,
            load_environment: false,
            ..tessera_world::BundleOptions::default()
        };
        let ctx = Arc::new(tessera_world::WorldContext::from_install(
            Arc::clone(&set),
            options,
        )?);
        let catalog = crate::walker::characters::catalog::CharacterCatalog::build(Arc::clone(&set));
        let setup = MoverSetup::load(&catalog, avatar);
        let pool = tessera_assets::WorkerPool::new(tessera_assets::PoolConfig {
            workers,
            thread_name: "tessera-walk".into(),
            ..tessera_assets::PoolConfig::default()
        });
        Ok(Self {
            set,
            ctx,
            pool,
            setup,
        })
    }

    /// A streamer over the session's world with [`walk_rings`].
    ///
    /// # Errors
    ///
    /// The streamer cannot be created.
    pub fn world(&self) -> anyhow::Result<StreamingWorld> {
        self.world_with(walk_rings())
    }

    /// A streamer over the session's world with `rings` (several may share the session's
    /// workers).
    ///
    /// # Errors
    ///
    /// The streamer cannot be created.
    pub fn world_with(&self, rings: tessera_world::RingConfig) -> anyhow::Result<StreamingWorld> {
        let streamer = WorldStreamer::new(
            Arc::clone(&self.ctx),
            &self.pool,
            tessera_world::StreamConfig {
                rings,
                ..tessera_world::StreamConfig::default()
            },
        )?;
        Ok(StreamingWorld::new(streamer, Some(self.pool.clone())))
    }
}

/// Reads `--walk-path`'s file: one walk in the file form, or, when the text holds the walk
/// table's anchor, every walk of the table (the checklist itself).
///
/// # Errors
///
/// The file cannot be read or does not parse.
pub fn read_walks(path: &std::path::Path) -> anyhow::Result<Vec<Walk>> {
    use anyhow::Context as _;
    use std::io::Read as _;
    let limit = crate::walk_path::MAX_WALK_TABLE_LEN;
    // Read through the limit: a pipe, a device or a growing file reports no useful length.
    let mut text = String::new();
    std::fs::File::open(path)
        .and_then(|f| {
            f.take(u64::try_from(limit).unwrap_or(u64::MAX).saturating_add(1))
                .read_to_string(&mut text)
        })
        .with_context(|| format!("reading {}", path.display()))?;
    if text.len() > limit {
        anyhow::bail!(
            "{} is too large for a walk file (more than {limit} bytes)",
            path.display()
        );
    }
    if text.contains(crate::walk_path::WALK_TABLE_ANCHOR) {
        Ok(crate::walk_path::parse_walk_table(&text)
            .with_context(|| format!("walk table in {}", path.display()))?)
    } else {
        Ok(vec![
            crate::walk_path::parse_walk_file(&text)
                .with_context(|| format!("walk file {}", path.display()))?,
        ])
    }
}

/// What `--walk-path` did.
#[derive(Debug, Clone, Default)]
pub struct WalkReport {
    /// One line per walk ([`summary_line`]), or why it could not run.
    pub lines: Vec<String>,
    /// Walks that passed.
    pub passed: usize,
    /// Walks that failed or could not run.
    pub failed: usize,
}

/// `tessera-client --walk-path FILE` (headless, no GPU): runs every walk of the file in
/// order, writes the movement log to `--walk-log` when given (CSV with [`LOG_HEADER`]) and
/// logs one summary line per walk.
///
/// # Errors
///
/// No install, unreadable walk file or log path, or a log path inside the install.
pub fn run_cli(args: &crate::cli::Args) -> anyhow::Result<WalkReport> {
    use anyhow::Context as _;
    let Some(path) = &args.walk_path else {
        anyhow::bail!("--walk-path not given");
    };
    if let Some(log) = &args.walk_log {
        crate::runtime::refuse_install_path(args, None, log)?;
    }
    let walks = read_walks(path)?;
    let env = tessera_install::SearchEnv::from_process();
    let discovery = tessera_install::discover_with(&env, args.game_dir.as_deref())
        .map_err(|e| anyhow::anyhow!("install not usable: {e}"))?;
    let install = discovery.installs.into_iter().next().with_context(|| {
        format!(
            "no install found (set {} or pass --game-dir)",
            tessera_install::GAME_DIR_ENV
        )
    })?;
    if let Some(log) = &args.walk_log
        && crate::runtime::is_inside(log, &install.root)
    {
        anyhow::bail!(
            "refusing to write {} inside the game install",
            log.display()
        );
    }
    let avatar = args.avatar.clone().unwrap_or_default();
    let session = WalkSession::open(&install.root, &avatar, &args.content_layers, 0)?;
    let mut world = session.world()?;
    let mut log = match &args.walk_log {
        Some(p) => {
            let mut f = std::io::BufWriter::new(
                std::fs::File::create(p).with_context(|| format!("creating {}", p.display()))?,
            );
            std::io::Write::write_all(&mut f, format!("{LOG_HEADER}\n").as_bytes())?;
            Some(f)
        }
        None => None,
    };
    let wait = LoadWait::Timeout(Duration::from_secs(args.settle_timeout));
    let mut report = WalkReport::default();
    for walk in &walks {
        match run_walk(walk, &mut world, &session.setup, wait) {
            Ok(out) => {
                if let Some(f) = &mut log {
                    write_log(f, &out)?;
                }
                let line = summary_line(&out);
                if out.verdict.passed {
                    report.passed += 1;
                    tracing::info!("{line}");
                } else {
                    report.failed += 1;
                    tracing::warn!("{line}");
                }
                report.lines.push(line);
            }
            Err(e) => {
                report.failed += 1;
                let line = format!("{} [{}] NOT RUN — {e}", walk.id, walk.category.name());
                tracing::warn!("{line}");
                report.lines.push(line);
            }
        }
    }
    tracing::info!(
        passed = report.passed,
        failed = report.failed,
        "scripted walks done"
    );
    Ok(report)
}
