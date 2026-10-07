// SPDX-License-Identifier: GPL-3.0-or-later
//! The seeded soak of random walks (ROADMAP M4.5 criterion 4; the rule of
//! `m4-5-visual.md` §2.5): at least 1,000 walks of 60 s simulated time over the land
//! landblocks of every region with landblock data, ending with 0 fall-throughs and 0
//! non-finite states.
//!
//! | Item | §2.5 step |
//! |---|---|
//! | [`soak_regions`], [`land_landblocks`], [`is_land`] | 1, 2 |
//! | [`SplitMix64`], [`SOAK_SEED`], [`SoakPlanner`] | 3 (and the redraws of step 4) |
//! | [`find_start`], [`StartCheck`] | 4 |
//! | [`SoakSegment`], [`SegmentKind`], [`SoakScript`] | 5 |
//! | [`run_soak_walk`], [`Failure`] | 6 |
//!
//! **Draw order** (Tessera reading; §2.5 names the draws but not their order, recorded as
//! an OPEN item there): per walk the landblock, x, y, heading; each redraw of step 4 draws
//! x then y again; then per segment its length, its kind, its heading change, until the
//! segments cover [`SOAK_SECONDS`]. One generator runs through all walks in order, so walk
//! *k* depends on the redraws of the walks before it; [`SoakPlanner`] therefore plans walks
//! in order, each start checked against the world, and the planned walks can then run in
//! any order or in parallel.

use std::sync::Arc;
use std::sync::atomic::{AtomicUsize, Ordering};

use tessera_formats::landblock::height_from_raw;
use tessera_formats::{BlockMap, LandblockData, Record as _};
use tessera_physics::controller::{COS_PLAYER_WALKABLE, CharacterShape};
use tessera_physics::geometry::{BodySource, CollisionQuery, QueryFilter};
use tessera_world::{LandblockKey, WorldContext, WorldPos};

use super::run::{WalkWorld, yaw_of};
use super::setup::MoverSetup;
use super::{CollisionWorld, MoveInput, PhysicsWalker, STEP_SECONDS};
use crate::walk_path::steps_for;

/// The seed: the bytes of "M45-SOAK" (§2.5 step 3).
pub const SOAK_SEED: u64 = 0x4D34_352D_534F_414B;
/// Simulated seconds per walk (§2.5 step 5).
pub const SOAK_SECONDS: f64 = 60.0;
/// The criterion's walk count.
pub const SOAK_WALKS: usize = 1_000;
/// Feet more than this below the terrain or a cell floor is a fall-through (§2.5 step 6), m.
pub const FALL_THROUGH: f64 = 0.05;
/// Start search height above the terrain (§2.5 step 4), m.
pub const START_HEIGHT: f32 = 3.0;
/// How far below the start search height the downward sweep looks, m (Tessera bound: the
/// terrain itself is 3 m below; 1 m more covers rounding).
pub const START_SWEEP: f32 = 4.0;
/// Most (x, y) redraws for one walk before its landblock is given up and the walk planned
/// without a start (Tessera bound; §2.5 does not cap the redraws, recorded as OPEN there).
pub const MAX_START_DRAWS: u32 = 256;
/// Grid range of land landblocks (§2.5 step 2: 1 ≤ X, Y ≤ 253).
pub const LAND_RANGE: std::ops::RangeInclusive<u8> = 1..=253;

/// The SplitMix64 generator (§2.5 step 3).
#[derive(Debug, Clone)]
pub struct SplitMix64 {
    state: u64,
}

impl SplitMix64 {
    /// A generator with `seed`.
    #[must_use]
    pub fn new(seed: u64) -> Self {
        Self { state: seed }
    }

    /// The next 64-bit output.
    pub fn next_u64(&mut self) -> u64 {
        self.state = self.state.wrapping_add(0x9E37_79B9_7F4A_7C15);
        let mut z = self.state;
        z = (z ^ (z >> 30)).wrapping_mul(0xBF58_476D_1CE4_E5B9);
        z = (z ^ (z >> 27)).wrapping_mul(0x94D0_49BB_1331_11EB);
        z ^ (z >> 31)
    }

    /// One draw u = (next() ≫ 11) / 2⁵³, in [0, 1).
    pub fn uniform(&mut self) -> f64 {
        #[allow(clippy::cast_precision_loss)] // 53 bits are exact
        let v = (self.next_u64() >> 11) as f64;
        v / 9_007_199_254_740_992.0
    }
}

/// The regions of §2.5 step 1: a descriptor exists and the region has landblock data
/// (`has_data` tells), ascending.
#[must_use]
pub fn soak_regions(ctx: &WorldContext, has_data: impl Fn(u8) -> bool) -> Vec<u8> {
    ctx.regions()
        .iter()
        .filter(|(_, info)| info.descriptor.is_some())
        .map(|(r, _)| *r)
        .filter(|r| has_data(*r))
        .collect()
}

/// Whether landblock data `data` is land (§2.5 step 2): its terrain chunk has a layer, its
/// heights are not all equal, and it is not fully submerged (no water height `water`, or at
/// least a quarter of its samples above it).
#[must_use]
pub fn is_land(data: &LandblockData, water: Option<f32>) -> bool {
    if data.terrain.layers.is_empty() {
        return false;
    }
    let (lo, hi) = data.heights.min_max();
    if lo == hi {
        return false;
    }
    match water {
        None => true,
        Some(h) => {
            let mut above = 0usize;
            let mut total = 0usize;
            for j in 0..33 {
                for i in 0..33 {
                    if let Some(z) = data.heights.get(i, j).map(height_from_raw) {
                        total += 1;
                        above += usize::from(z > h);
                    }
                }
            }
            above * 4 >= total
        }
    }
}

/// The landblocks of one region for the soak.
#[derive(Debug, Clone, Default, PartialEq, Eq)]
pub struct LandList {
    /// Land landblocks (§2.5 step 2), sorted by DID.
    pub land: Vec<LandblockKey>,
    /// Landblock-data records found in the grid range of step 2 (step 1 asks whether a
    /// region has landblock data).
    pub records: usize,
}

/// The land landblocks of `region` (§2.5 step 2), sorted by DID, decoded on `threads`
/// threads. Records that do not decode are left out.
#[must_use]
pub fn land_landblocks(ctx: &WorldContext, region: u8, threads: usize) -> LandList {
    let keys: Vec<LandblockKey> = LAND_RANGE
        .flat_map(|x| LAND_RANGE.map(move |y| (x, y)))
        .filter_map(|(x, y)| LandblockKey::new(region, x, y).ok())
        .collect();
    let next = AtomicUsize::new(0);
    let records = AtomicUsize::new(0);
    let found = std::sync::Mutex::new(Vec::new());
    let water = ctx.systems().water.clone();
    std::thread::scope(|s| {
        for _ in 0..threads.max(1) {
            s.spawn(|| {
                let mut mine = Vec::new();
                loop {
                    let k = next.fetch_add(1, Ordering::Relaxed);
                    let Some(&key) = keys.get(k) else { break };
                    let Ok(Some((bytes, stamp))) = ctx.source().read_stamped(key.data_did()) else {
                        continue;
                    };
                    records.fetch_add(1, Ordering::Relaxed);
                    let Ok(data) = LandblockData::parse(&bytes, stamp) else {
                        continue;
                    };
                    let h = water.as_ref().and_then(|w| {
                        let map = ctx
                            .source()
                            .read(key.block_map_did())
                            .ok()
                            .flatten()
                            .and_then(|b| BlockMap::parse(&b).ok());
                        w.tables()
                            .decide(key, map.as_ref(), &data.heights)
                            .ok()
                            .map(|lw| lw.height)
                    });
                    if is_land(&data, h) {
                        mine.push(key);
                    }
                }
                if let Ok(mut f) = found.lock() {
                    f.extend(mine);
                }
            });
        }
    });
    let mut land = found.into_inner().unwrap_or_default();
    land.sort_unstable_by_key(|k| k.data_did().0);
    LandList {
        land,
        records: records.into_inner(),
    }
}

/// One input segment (§2.5 step 5).
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct SoakSegment {
    /// Length, s (2 + 4u).
    pub seconds: f64,
    /// What the walker does.
    pub kind: SegmentKind,
    /// Heading change at the segment start, degrees (180u − 90).
    pub turn: f64,
}

/// The input of a segment (§2.5 step 5).
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum SegmentKind {
    /// Run forward (u < 0.50).
    Run,
    /// Walk forward (< 0.65).
    Walk,
    /// Backward (< 0.75).
    Backward,
    /// Strafe left (< 0.80).
    StrafeLeft,
    /// Strafe right (< 0.85).
    StrafeRight,
    /// Run forward with a jump at the segment start (< 0.95).
    RunJump,
    /// Stand still.
    Stand,
}

impl SegmentKind {
    /// The kind for draw `u`.
    #[must_use]
    pub fn from_draw(u: f64) -> Self {
        match u {
            u if u < 0.50 => Self::Run,
            u if u < 0.65 => Self::Walk,
            u if u < 0.75 => Self::Backward,
            u if u < 0.80 => Self::StrafeLeft,
            u if u < 0.85 => Self::StrafeRight,
            u if u < 0.95 => Self::RunJump,
            _ => Self::Stand,
        }
    }
}

/// One planned soak walk.
#[derive(Debug, Clone, PartialEq)]
pub struct SoakWalk {
    /// Walk number *k*.
    pub index: usize,
    /// The landblock.
    pub block: LandblockKey,
    /// Start (x, y), block-local.
    pub start_xy: [f64; 2],
    /// Feet height at the start (the support found by [`find_start`]); `None` when no
    /// valid start was found within [`MAX_START_DRAWS`] draws.
    pub start_z: Option<f64>,
    /// Compass heading at the start, degrees.
    pub heading: f64,
    /// The input segments.
    pub segments: Vec<SoakSegment>,
    /// Start (x, y) draws taken (1 = the first was valid).
    pub start_draws: u32,
}

/// What [`find_start`] found at a start candidate.
#[derive(Debug, Clone, Copy, PartialEq)]
pub enum StartCheck {
    /// The feet height of the highest walkable support.
    Feet(f64),
    /// The capsule overlaps geometry at the search height or on the support.
    Overlap,
    /// The support is a cell floor.
    CellFloor,
    /// The sweep found no support, or the support is not walkable.
    NoWalkableSupport,
    /// The landblock is not resident or has no terrain height there.
    NotLoaded,
}

/// §2.5 step 4: sweeps `shape` down from [`START_HEIGHT`] above the terrain at block-local
/// `xy` of `block` and classifies what it finds.
#[must_use]
pub fn find_start(
    world: &dyn CollisionWorld,
    shape: &CharacterShape,
    block: LandblockKey,
    xy: [f64; 2],
) -> StartCheck {
    let o = block.origin();
    let probe = WorldPos::new(block.region(), o[0] + xy[0], o[1] + xy[1], 0.0);
    let Some(terrain) = world.terrain_height(&probe) else {
        return StartCheck::NotLoaded;
    };
    let view = world.view(&probe);
    let top = [probe.x, probe.y, f64::from(terrain + START_HEIGHT)];
    let filter = QueryFilter::player();
    let capsule = shape.capsule_at(view.to_view(top));
    let mut contacts = Vec::new();
    view.contacts(&capsule, 0.0, &filter, &mut contacts);
    if contacts.iter().any(|c| c.separation < 0.0) {
        return StartCheck::Overlap;
    }
    let Some(hit) = view.sweep_capsule(&capsule, [0.0, 0.0, -START_SWEEP], &filter) else {
        return StartCheck::NoWalkableSupport;
    };
    let source = view
        .members()
        .get(hit.set as usize)
        .and_then(|(s, _)| s.body(hit.body))
        .map(|b| b.source);
    if matches!(source, Some(BodySource::Cell { .. })) {
        return StartCheck::CellFloor;
    }
    if hit.normal[2] <= COS_PLAYER_WALKABLE {
        return StartCheck::NoWalkableSupport;
    }
    let feet = top[2] - f64::from(hit.fraction * START_SWEEP);
    let at = shape.capsule_at(view.to_view([top[0], top[1], feet]));
    contacts.clear();
    view.contacts(&at, 0.0, &filter, &mut contacts);
    if contacts.iter().any(|c| c.separation < -0.001) {
        return StartCheck::Overlap;
    }
    StartCheck::Feet(feet)
}

/// Plans soak walks in order (§2.5 steps 3–5; draw order in the module docs).
#[derive(Debug, Clone)]
pub struct SoakPlanner {
    rng: SplitMix64,
    lists: Vec<(u8, Arc<Vec<LandblockKey>>)>,
    next: usize,
}

impl SoakPlanner {
    /// A planner over the land lists of the regions of step 1, in ascending region order,
    /// with generator seed `seed`.
    #[must_use]
    pub fn new(seed: u64, lists: Vec<(u8, Arc<Vec<LandblockKey>>)>) -> Self {
        Self {
            rng: SplitMix64::new(seed),
            lists,
            next: 0,
        }
    }

    /// The number of the next walk.
    #[must_use]
    pub fn next_index(&self) -> usize {
        self.next
    }

    /// The landblock walk [`SoakPlanner::next_index`] will use, drawing nothing: the caller
    /// streams around it before [`SoakPlanner::plan`] checks starts. `None` when there is
    /// no region or the region's list is empty.
    #[must_use]
    pub fn peek_block(&self) -> Option<LandblockKey> {
        let (_, list) = self.lists.get(self.next % self.lists.len().max(1))?;
        let mut rng = self.rng.clone();
        #[allow(
            clippy::cast_precision_loss,
            clippy::cast_possible_truncation,
            clippy::cast_sign_loss
        )]
        let i = ((rng.uniform() * list.len() as f64) as usize).min(list.len().checked_sub(1)?);
        list.get(i).copied()
    }

    /// Plans the next walk; `start` checks a start candidate (normally [`find_start`] over
    /// the streamed world). `None` when there is no region or the region's list is empty.
    pub fn plan(
        &mut self,
        mut start: impl FnMut(LandblockKey, [f64; 2]) -> StartCheck,
    ) -> Option<SoakWalk> {
        let (_, list) = self.lists.get(self.next % self.lists.len().max(1))?.clone();
        if list.is_empty() {
            return None;
        }
        let index = self.next;
        self.next += 1;
        let r = &mut self.rng;
        #[allow(
            clippy::cast_precision_loss,
            clippy::cast_possible_truncation,
            clippy::cast_sign_loss
        )]
        let i = ((r.uniform() * list.len() as f64) as usize).min(list.len() - 1);
        let block = list[i];
        let mut xy = [16.0 + 128.0 * r.uniform(), 16.0 + 128.0 * r.uniform()];
        let heading = 360.0 * r.uniform();
        let mut draws = 1;
        let start_z = loop {
            match start(block, xy) {
                StartCheck::Feet(z) => break Some(z),
                StartCheck::NotLoaded => break None,
                _ if draws >= MAX_START_DRAWS => break None,
                _ => {
                    xy = [16.0 + 128.0 * r.uniform(), 16.0 + 128.0 * r.uniform()];
                    draws += 1;
                }
            }
        };
        let mut segments = Vec::new();
        let mut covered = 0.0;
        while covered < SOAK_SECONDS {
            let seconds = 2.0 + 4.0 * r.uniform();
            let kind = SegmentKind::from_draw(r.uniform());
            let turn = 180.0 * r.uniform() - 90.0;
            segments.push(SoakSegment {
                seconds,
                kind,
                turn,
            });
            covered += seconds;
        }
        Some(SoakWalk {
            index,
            block,
            start_xy: xy,
            start_z,
            heading,
            segments,
            start_draws: draws,
        })
    }
}

/// The per-step input of a soak walk (§2.5 step 5).
#[derive(Debug, Clone)]
pub struct SoakScript {
    segments: Vec<SoakSegment>,
    current: usize,
    ends: f64,
    heading: f64,
    started: bool,
}

/// One step's input from a [`SoakScript`].
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct SoakCommand {
    /// The controller input (heading as yaw).
    pub input: MoveInput,
    /// The Walking layer.
    pub walking: bool,
}

impl SoakScript {
    /// The script of `walk`.
    #[must_use]
    pub fn new(walk: &SoakWalk) -> Self {
        Self {
            segments: walk.segments.clone(),
            current: 0,
            ends: walk.segments.first().map_or(f64::INFINITY, |s| s.seconds),
            heading: walk.heading,
            started: false,
        }
    }

    /// The command for the step starting at `t` seconds.
    pub fn command(&mut self, t: f64) -> SoakCommand {
        let mut fresh = !self.started;
        self.started = true;
        while t >= self.ends - 1e-9 && self.current + 1 < self.segments.len() {
            self.current += 1;
            self.ends += self.segments[self.current].seconds;
            fresh = true;
        }
        let Some(seg) = self.segments.get(self.current).copied() else {
            return SoakCommand {
                input: MoveInput::default(),
                walking: false,
            };
        };
        if fresh {
            self.heading = (self.heading + seg.turn).rem_euclid(360.0);
        }
        let (forward, strafe, walking, jump) = match seg.kind {
            SegmentKind::Run => (1.0, 0.0, false, false),
            SegmentKind::Walk => (1.0, 0.0, true, false),
            SegmentKind::Backward => (-1.0, 0.0, false, false),
            SegmentKind::StrafeLeft => (0.0, -1.0, false, false),
            SegmentKind::StrafeRight => (0.0, 1.0, false, false),
            SegmentKind::RunJump => (1.0, 0.0, false, fresh),
            SegmentKind::Stand => (0.0, 0.0, false, false),
        };
        SoakCommand {
            input: MoveInput {
                forward,
                strafe,
                heading: Some(yaw_of(self.heading)),
                jump,
            },
            walking,
        }
    }
}

/// A failure of §2.5 step 6.
#[derive(Debug, Clone, PartialEq)]
pub enum Failure {
    /// Feet below the terrain of their landblock while outdoors.
    BelowTerrain {
        /// Step number.
        step: u64,
        /// Feet.
        feet: [f64; 3],
        /// Terrain height.
        terrain: f64,
    },
    /// Feet below the floor of their cell.
    BelowCellFloor {
        /// Step number.
        step: u64,
        /// Feet.
        feet: [f64; 3],
        /// The floor found above the feet along the capsule's axis.
        floor: f64,
        /// The cell.
        cell: u16,
    },
    /// A non-finite position or velocity.
    NonFinite {
        /// Step number.
        step: u64,
    },
}

/// What one soak walk did.
#[derive(Debug, Clone, PartialEq, Default)]
pub struct SoakOutcome {
    /// Steps run.
    pub steps: u64,
    /// Failures (the walk stops at the first).
    pub failures: Vec<Failure>,
    /// The deepest the feet were below the terrain while outdoors, m (0 if never).
    pub max_below_terrain: f64,
    /// Steps in a water state, in state 6, in the air, inside a cell.
    pub water_steps: u64,
    /// Steps sliding (state 6).
    pub slide_steps: u64,
    /// Steps in the air (state 4).
    pub air_steps: u64,
    /// Steps inside a cell.
    pub cell_steps: u64,
    /// Steps undone at the boundary of an absent landblock.
    pub boundary_steps: u64,
    /// The walk could not run (no start, or sets not loaded in time).
    pub skipped: Option<String>,
    /// Horizontal distance from the start at the end, m.
    pub travelled: f64,
    /// Feet at the end, region frame.
    pub end: [f64; 3],
}

/// Runs `walk` in `world` (§2.5 steps 5–6), waiting at most `timeout` per missing set.
pub fn run_soak_walk<W: WalkWorld>(
    walk: &SoakWalk,
    world: &mut W,
    setup: &MoverSetup,
    timeout: std::time::Duration,
) -> SoakOutcome {
    let mut out = SoakOutcome::default();
    let Some(z) = walk.start_z else {
        out.skipped = Some("no valid start".into());
        return out;
    };
    let o = walk.block.origin();
    let start = [o[0] + walk.start_xy[0], o[1] + walk.start_xy[1], z];
    let mut walker = PhysicsWalker::new(setup, walk.block.region(), start, yaw_of(walk.heading));
    let mut script = SoakScript::new(walk);
    let steps = steps_for(SOAK_SECONDS, f64::from(STEP_SECONDS));
    let shape = setup.shape;
    let filter = QueryFilter::player();
    let mut k = 0u64;
    let mut waited_since: Option<std::time::Instant> = None;
    while k < steps {
        if k.is_multiple_of(super::run::STREAM_EVERY) || waited_since.is_some() {
            world.update(&walker.feet(), &walker.pins());
        }
        #[allow(clippy::cast_precision_loss)]
        let t = k as f64 * f64::from(STEP_SECONDS);
        if !walker.pending(world).is_empty() {
            let since = *waited_since.get_or_insert_with(std::time::Instant::now);
            if since.elapsed() >= timeout {
                out.skipped = Some(format!(
                    "sets not installed after {} s at step {k}",
                    since.elapsed().as_secs()
                ));
                return out;
            }
            std::thread::sleep(std::time::Duration::from_millis(1));
            continue;
        }
        waited_since = None;
        let cmd = script.command(t);
        let Some(rec) = walker.step(&cmd.input, cmd.walking, world) else {
            continue;
        };
        k += 1;
        out.steps = k;
        let s = rec.state;
        let feet = s.position;
        if !s.is_finite() {
            out.failures.push(Failure::NonFinite { step: k });
            break;
        }
        out.boundary_steps += u64::from(rec.at_boundary);
        if s.element.in_water() {
            out.water_steps += 1;
        }
        match s.element {
            super::MovementState::EarthUncontrolledSurface => out.slide_steps += 1,
            super::MovementState::EarthNoSurface => out.air_steps += 1,
            _ => {}
        }
        let pos = WorldPos::new(walk.block.region(), feet[0], feet[1], feet[2]);
        if let Some((_, cell)) = rec.cell {
            out.cell_steps += 1;
            // The floor: the first surface on the capsule's axis from its top down to the
            // feet; a cell physics mesh crossing the axis more than 0.05 m above the feet
            // means the feet are below that cell's floor.
            let view = world.view(&pos);
            let top = view.to_view([feet[0], feet[1], feet[2] + f64::from(shape.height())]);
            let reach = shape.height() - FALL_THROUGH as f32;
            if let Some(hit) = view.raycast(top, [0.0, 0.0, -reach], &filter) {
                let is_cell = view
                    .members()
                    .get(hit.set as usize)
                    .and_then(|(set, _)| set.body(hit.body))
                    .is_some_and(|b| matches!(b.source, BodySource::Cell { .. }));
                if is_cell {
                    let floor = view.to_region(hit.point)[2];
                    out.failures.push(Failure::BelowCellFloor {
                        step: k,
                        feet,
                        floor,
                        cell,
                    });
                    break;
                }
            }
        } else if let Some(terrain) = world.terrain_height(&pos) {
            let below = f64::from(terrain) - feet[2];
            out.max_below_terrain = out.max_below_terrain.max(below);
            if below > FALL_THROUGH {
                out.failures.push(Failure::BelowTerrain {
                    step: k,
                    feet,
                    terrain: f64::from(terrain),
                });
                break;
            }
        }
    }
    let end = walker.state().position;
    out.travelled = (end[0] - start[0]).hypot(end[1] - start[1]);
    out.end = end;
    out
}
