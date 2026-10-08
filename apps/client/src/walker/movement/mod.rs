// SPDX-License-Identifier: GPL-3.0-or-later
//! Walk mode on collision (ROADMAP M4.5): the avatar moves with the `tessera-physics`
//! character controller over the collision sets that `tessera-world` builds on the asset
//! workers (`collision.md` §10–§12, `water.md` §7.4, decision 0013, ARCHITECTURE §20.3),
//! instead of following terrain heights.
//!
//! | Item | Purpose |
//! |---|---|
//! | [`PhysicsWalker`] | The controller on its own fixed 1/60 s accumulator ([`STEP_SECONDS`], at most [`MAX_STEPS_PER_FRAME`] steps per frame, time beyond dropped), the latched input, the pose interpolated between the last two states, residency waiting and the solid boundary of absent landblocks |
//! | [`CollisionWorld`], [`StreamedCollision`], [`BlockReadiness`] | What a step needs from the world: which landblocks have installed sets, a collision view (water planes and liquid boxes included, so the controller's water state comes from the world's water), terrain heights |
//! | [`StepRecord`], [`ContactInfo`], [`ContactKind`] | One step's movement-log entry: position, state, cell, contacts, penetration |
//! | [`PhysicsWalker::cell`], [`CELL_PROBE_LIFT`] | The mover's cell, tracked through point-in-cell and portals (`collision.md` §5.5, §5.8, §5.8.1) |
//! | [`CellMaskedWater`] | The world's water for the controller's water state, none inside interior cells (`water.md` §7.4 OPEN) |
//! | [`setup::MoverSetup`] | The capsule of the avatar's setup (`collision.md` §10.6), the player movement record (§10.3) and the entity conditions (§11.1, float height §11.2) |
//! | [`run`] | Scripted walks (`--walk-path`, `m4-5-visual.md` §2) headless, without a GPU, with their pass conditions |
//! | [`soak`] | The seeded 1,000-walk soak of `m4-5-visual.md` §2.5 |
//!
//! # Residency (decision 0013, ARCHITECTURE §20.3)
//!
//! A step needs every landblock that the controller's query box
//! ([`Controller::query_bounds`]) overlaps, the feet's landblock with its eight
//! neighbours, the owner of the mover's current cell and the landblocks the caller
//! requires ([`PhysicsWalker::set_required`]; a scripted walk requires its own landblock,
//! the owner of its cells) ([`PhysicsWalker::needed`]). While one of them is still loading, the walker
//! does not step: the frame's accumulated time is dropped, the latched input (a pending jump
//! too) is kept and the mover's state does not change. A landblock the streamer resolved
//! as absent (failed, or off the grid) counts as installed with an empty set and is a
//! **solid boundary**: a step that would move the capsule's footprint into an absent
//! landblock it was not already in is undone with the velocity zeroed (Tessera Decision,
//! recorded in `walker.md` §13).
//!
//! # Placement (`walker.md` §13.1)
//!
//! After [`PhysicsWalker::teleport`] (entering walk mode, a placement or a correction), the
//! first step that can run first checks the feet: outside every interior cell and more
//! than [`PLACE_BELOW_TERRAIN`] below the terrain, they are put on the highest walkable
//! support that a capsule swept down from [`PLACE_SEARCH_HEIGHT`] above the terrain finds
//! (the terrain height itself when there is none, or when it is a ceiling or the capsule
//! starts in an overlap). The controller cannot recover a capsule that starts a metre or
//! more below the terrain.

pub mod run;
pub mod setup;
pub mod soak;

use std::sync::Arc;

use tessera_input::FixedTimestep;
use tessera_physics::controller::{COS_PLAYER_WALKABLE, WaterQuery, WaterSample};
use tessera_physics::geometry::{BodySource, CollisionQuery, CollisionView, Contact};
pub use tessera_physics::{
    Controller, ControllerConfig, MAX_STEPS_PER_FRAME, MoveInput, MovementState, MoverState,
    STEP_SECONDS, STEPS_PER_SECOND, StepEnv, StepOutcome, StepReport,
};
use tessera_world::{BlockState, LANDBLOCK_SIZE, LandblockKey, WorldPos, WorldStreamer};

use self::setup::MoverSetup;

/// Most contacts one [`StepRecord`] keeps (the deepest first).
pub const MAX_LOGGED_CONTACTS: usize = 8;
/// Contacts closer than this to the capsule are logged, m (a resting capsule keeps a gap
/// of one to two skins, 0.001–0.002 m, from what supports it).
pub const CONTACT_LOG_MARGIN: f32 = 0.01;
/// Chessboard radius, in landblocks, of the collision view a step uses: the query box is
/// the capsule grown by at most [`tessera_physics::controller::MAX_STEP_DISPLACEMENT`]
/// (10 m), so the mover's landblock and its eight neighbours cover it.
pub const VIEW_RADIUS: u32 = 1;
/// Most landblocks [`PhysicsWalker::set_required`] keeps.
pub const MAX_REQUIRED: usize = 8;
/// Feet placed more than this below the terrain (outside any cell) are lifted onto the
/// support (module docs, Placement), m.
pub const PLACE_BELOW_TERRAIN: f64 = 0.05;
/// How far above the terrain the placement sweep starts (as the soak's start search,
/// `m4-5-visual.md` §2.5 step 4), m.
pub const PLACE_SEARCH_HEIGHT: f32 = 3.0;
/// The placement puts the feet this far above the terrain when no support is found, m.
pub const PLACE_LIFT: f64 = 0.01;

/// Whether a landblock's collision set can be used.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum BlockReadiness {
    /// Its set is installed.
    Ready,
    /// It is resolved as absent (no data, failed, off the grid): an empty set and a solid
    /// boundary.
    Absent,
    /// Not installed yet (loading, deferred or not yet desired).
    Pending,
}

/// What the walker's steps read from the world.
pub trait CollisionWorld {
    /// Whether `key`'s collision set is installed.
    fn readiness(&self, key: LandblockKey) -> BlockReadiness;
    /// A collision view over the installed sets around `centre` (every landblock within
    /// [`VIEW_RADIUS`] of it, plus the owners of cells reaching there), with their water.
    fn view(&self, centre: &WorldPos) -> CollisionView<'_>;
    /// The terrain height at `pos` (`landblock.md` §3.11), when its landblock is resident.
    fn terrain_height(&self, pos: &WorldPos) -> Option<f32>;
    /// The cells that the portals of cell `cell` of landblock `owner` lead to
    /// (`collision.md` §5.5, §5.8); none by default.
    fn portal_neighbours(&self, owner: LandblockKey, cell: u16) -> Vec<u16> {
        let _ = (owner, cell);
        Vec::new()
    }
}

/// How far above the feet the cell lookup looks (`m4-5-visual.md` CELL rule), m.
pub const CELL_PROBE_LIFT: f64 = 0.05;

/// The water of a collision view, except inside interior cells: a mover in a cell is
/// never in landscape water (Tessera Decision, recorded as OPEN in `water.md` §7.4: a
/// landblock's water volume entity reaches far below its water plane, so without the
/// mask a dungeon under a lake would be flooded).
#[derive(Debug, Clone, Copy)]
pub struct CellMaskedWater<'a, 'v>(pub &'a CollisionView<'v>);

impl WaterQuery for CellMaskedWater<'_, '_> {
    fn water_at(&self, p: [f32; 3]) -> Option<WaterSample> {
        #[allow(clippy::cast_possible_truncation)]
        let probe = [p[0], p[1], p[2] + CELL_PROBE_LIFT as f32];
        if self.0.cell_at(probe).is_some() {
            return None;
        }
        self.0.water_at(p)
    }
}

/// The collision residency of a [`WorldStreamer`].
#[derive(Debug, Clone, Copy)]
pub struct StreamedCollision<'a>(pub &'a WorldStreamer);

impl CollisionWorld for StreamedCollision<'_> {
    fn readiness(&self, key: LandblockKey) -> BlockReadiness {
        match self.0.state_of(key) {
            Some(BlockState::Ready(b)) => {
                if b.collision.is_some() {
                    BlockReadiness::Ready
                } else {
                    // A bundle built without collision has nothing to stand on.
                    BlockReadiness::Absent
                }
            }
            Some(BlockState::Failed(_)) => BlockReadiness::Absent,
            Some(BlockState::Deferred | BlockState::Loading) | None => BlockReadiness::Pending,
        }
    }

    fn view(&self, centre: &WorldPos) -> CollisionView<'_> {
        self.0.collision_view(centre, VIEW_RADIUS)
    }

    fn terrain_height(&self, pos: &WorldPos) -> Option<f32> {
        self.0.terrain_height_at(pos)
    }

    fn portal_neighbours(&self, owner: LandblockKey, cell: u16) -> Vec<u16> {
        self.0
            .collision(owner)
            .and_then(|c| c.cell(cell))
            .map(|c| c.portals.iter().map(|p| p.neighbour).collect())
            .unwrap_or_default()
    }
}

/// What a logged contact touches.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum ContactKind {
    /// The terrain mesh.
    Terrain,
    /// The ceiling mesh.
    Ceiling,
    /// A static entity of landblock info (its index in the entity list).
    Static(u32),
    /// A generated scenery object (its index in the generated list).
    Generated(u32),
    /// A cell's physics mesh (the cell number).
    Cell(u16),
    /// Something else.
    Other,
}

impl ContactKind {
    fn of(source: BodySource) -> Self {
        match source {
            BodySource::Terrain => Self::Terrain,
            BodySource::Ceiling => Self::Ceiling,
            BodySource::Static { index, .. } => Self::Static(index),
            BodySource::Generated { index, .. } => Self::Generated(index),
            BodySource::Cell { cell, .. } => Self::Cell(cell),
            BodySource::Other { .. } => Self::Other,
        }
    }

    /// Whether it is a placed or generated object (not terrain, ceiling or cell).
    #[must_use]
    pub fn is_object(self) -> bool {
        matches!(self, Self::Static(_) | Self::Generated(_))
    }
}

impl std::fmt::Display for ContactKind {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            Self::Terrain => f.write_str("terrain"),
            Self::Ceiling => f.write_str("ceiling"),
            Self::Static(i) => write!(f, "static#{i}"),
            Self::Generated(i) => write!(f, "generated#{i}"),
            Self::Cell(c) => write!(f, "cell{c}"),
            Self::Other => f.write_str("other"),
        }
    }
}

/// One contact of the capsule at the end of a step.
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct ContactInfo {
    /// What it touches.
    pub kind: ContactKind,
    /// The landblock of the touched set, when it has one.
    pub landblock: Option<LandblockKey>,
    /// Gap between the surfaces, m (negative: penetration).
    pub separation: f32,
    /// Up component of the contact normal.
    pub normal_z: f32,
}

/// One step of the movement log.
#[derive(Debug, Clone, PartialEq)]
pub struct StepRecord {
    /// The mover's state after the step.
    pub state: MoverState,
    /// Region of the position.
    pub region: u8,
    /// What the step did.
    pub report: StepReport,
    /// The cell containing the feet after the step (owner landblock and cell number).
    pub cell: Option<(LandblockKey, u16)>,
    /// Contacts within [`CONTACT_LOG_MARGIN`], deepest first (at most
    /// [`MAX_LOGGED_CONTACTS`]); empty unless details are on.
    pub contacts: Vec<ContactInfo>,
    /// Deepest penetration of the capsule into collision geometry after the step, m
    /// (0 without details).
    pub penetration: f32,
    /// The step was undone at the solid boundary of an absent landblock.
    pub at_boundary: bool,
    /// The input of the step.
    pub input: MoveInput,
    /// The Walking layer was in force.
    pub walking: bool,
}

impl StepRecord {
    /// The feet as a world position.
    #[must_use]
    pub fn feet(&self) -> WorldPos {
        let p = self.state.position;
        WorldPos::new(self.region, p[0], p[1], p[2])
    }
}

/// What one frame of [`PhysicsWalker::frame`] did.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub struct FrameSteps {
    /// Steps taken.
    pub steps: u32,
    /// The steps waited for collision sets (time dropped, input kept).
    pub waiting: bool,
}

/// The landblocks a region-frame box overlaps, as (key or `None` off the grid), ascending.
fn blocks_in(region: u8, lo: [f64; 3], hi: [f64; 3]) -> Vec<Option<LandblockKey>> {
    let idx = |v: f64| (v / LANDBLOCK_SIZE).floor();
    let (x0, x1, y0, y1) = (idx(lo[0]), idx(hi[0]), idx(lo[1]), idx(hi[1]));
    if ![x0, x1, y0, y1].iter().all(|v| v.is_finite()) || x1 - x0 > 4.0 || y1 - y0 > 4.0 {
        return vec![None];
    }
    let mut out = Vec::new();
    let mut y = y0;
    while y <= y1 {
        let mut x = x0;
        while x <= x1 {
            let key = if (0.0..255.0).contains(&x) && (0.0..255.0).contains(&y) {
                LandblockKey::new(region, x as u8, y as u8).ok()
            } else {
                None
            };
            out.push(key);
            x += 1.0;
        }
        y += 1.0;
    }
    out
}

/// The character controller on a fixed step (module docs).
#[derive(Debug, Clone)]
pub struct PhysicsWalker {
    controller: Controller,
    timestep: FixedTimestep,
    previous: MoverState,
    region: u8,
    jump: bool,
    walking: bool,
    details: bool,
    last: Option<StepRecord>,
    scratch: Vec<Contact>,
    cell: Option<(LandblockKey, u16)>,
    required: Vec<LandblockKey>,
    check_placement: bool,
}

impl PhysicsWalker {
    /// A walker whose feet stand at region-frame `feet` in `region`, facing `yaw` (radians,
    /// 0 = east, counter-clockwise), with the capsule, record and conditions of `setup`.
    #[must_use]
    pub fn new(setup: &MoverSetup, region: u8, feet: [f64; 3], yaw: f32) -> Self {
        let state = MoverState::at(feet, yaw);
        let controller = Controller::new(
            ControllerConfig {
                shape: setup.shape,
                ..ControllerConfig::default()
            },
            Arc::clone(&setup.record),
            Arc::clone(&setup.pids),
            setup.conditions,
            state,
        );
        let previous = *controller.state();
        Self {
            controller,
            timestep: FixedTimestep::new(f64::from(STEPS_PER_SECOND))
                .with_max_ticks(MAX_STEPS_PER_FRAME),
            previous,
            region,
            jump: false,
            walking: setup.conditions.walking,
            details: false,
            last: None,
            scratch: Vec::new(),
            cell: None,
            required: Vec::new(),
            check_placement: false,
        }
    }

    /// Landblocks every step needs besides those around the feet (at most
    /// [`MAX_REQUIRED`], other regions ignored): a scripted walk requires its own landblock,
    /// the owner of its cells, which may lie several landblocks from them (`collision.md`
    /// §5.8.1). Kept across teleports.
    pub fn set_required(&mut self, keys: &[LandblockKey]) {
        self.required.clear();
        for k in keys {
            if k.region() == self.region
                && !self.required.contains(k)
                && self.required.len() < MAX_REQUIRED
            {
                self.required.push(*k);
            }
        }
    }

    /// The landblocks of [`PhysicsWalker::set_required`].
    #[must_use]
    pub fn required(&self) -> &[LandblockKey] {
        &self.required
    }

    /// Landblocks a streamer should keep resident for this walker wherever they lie: the
    /// required ones and the owner of the current cell.
    #[must_use]
    pub fn pins(&self) -> Vec<LandblockKey> {
        let mut out = self.required.clone();
        if let Some((k, _)) = self.cell
            && !out.contains(&k)
        {
            out.push(k);
        }
        out
    }

    /// Sets the mover's current cell (owner landblock and number), as a scripted walk's
    /// `cell` field declares it; the next step keeps it while it contains the feet
    /// (`walker.md` §13.4). `None`: outdoors.
    pub fn set_cell(&mut self, cell: Option<(LandblockKey, u16)>) {
        self.cell = cell.filter(|(k, _)| k.region() == self.region);
    }

    /// Turns the per-step contact query and penetration measurement of [`StepRecord`] on
    /// (scripted walks) or off (interactive walking, the soak).
    pub fn set_details(&mut self, on: bool) {
        self.details = on;
    }

    /// Puts the feet at `feet` (region `region`) facing `yaw`, standing still (a teleport
    /// or a correction by the session). A non-finite position is refused. The first step
    /// that can run lifts feet found below the terrain (module docs, Placement).
    pub fn teleport(&mut self, region: u8, feet: [f64; 3], yaw: f32) -> bool {
        let state = MoverState {
            steps: self.controller.state().steps,
            ..MoverState::at(feet, yaw)
        };
        if !self.controller.set_state(state) {
            return false;
        }
        self.region = region;
        self.previous = *self.controller.state();
        self.timestep.reset();
        self.last = None;
        self.cell = None;
        self.required.retain(|k| k.region() == region);
        self.check_placement = true;
        true
    }

    /// Whether the next step that can run first checks the placement (after
    /// [`PhysicsWalker::teleport`] or [`PhysicsWalker::check_placement`]).
    #[must_use]
    pub fn placement_pending(&self) -> bool {
        self.check_placement
    }

    /// Makes the next step that can run check the placement first (module docs,
    /// Placement), as after a teleport: for a walker created where the feet may lie below
    /// the terrain (walk mode entered at a camera or session position).
    pub fn check_placement(&mut self) {
        self.check_placement = true;
    }

    /// The placement check (module docs, Placement), run once the needed sets are
    /// installed. Returns whether the feet were moved.
    fn fix_placement(&mut self, view: &CollisionView<'_>, world: &dyn CollisionWorld) -> bool {
        self.check_placement = false;
        let s = *self.controller.state();
        let feet = s.position;
        let probe = view.to_view([feet[0], feet[1], feet[2] + CELL_PROBE_LIFT]);
        if view.cell_at(probe).is_some() {
            return false;
        }
        let Some(terrain) = world.terrain_height(&self.feet()).map(f64::from) else {
            return false;
        };
        if feet[2] >= terrain - PLACE_BELOW_TERRAIN {
            return false;
        }
        let config = self.controller.config();
        let top = [feet[0], feet[1], terrain + f64::from(PLACE_SEARCH_HEIGHT)];
        let capsule = config.shape.capsule_at(view.to_view(top));
        let reach = PLACE_SEARCH_HEIGHT + 1.0;
        self.scratch.clear();
        view.contacts(&capsule, 0.0, &config.filter, &mut self.scratch);
        let overlap = self.scratch.iter().any(|c| c.separation < 0.0);
        let support = if overlap {
            None
        } else {
            view.sweep_capsule(&capsule, [0.0, 0.0, -reach], &config.filter)
                .filter(|hit| {
                    let ceiling = view
                        .members()
                        .get(hit.set as usize)
                        .and_then(|(set, _)| set.body(hit.body))
                        .is_some_and(|b| matches!(b.source, BodySource::Ceiling));
                    !ceiling && hit.normal[2] > COS_PLAYER_WALKABLE
                })
                .map(|hit| top[2] - f64::from(hit.fraction * reach))
        };
        let z = support.unwrap_or(terrain + PLACE_LIFT).max(terrain);
        let placed = MoverState {
            steps: s.steps,
            ..MoverState::at([feet[0], feet[1], z], s.heading)
        };
        if !self.controller.set_state(placed) {
            return false;
        }
        self.previous = *self.controller.state();
        tracing::debug!(
            from = feet[2],
            to = z,
            terrain,
            "walker placed below the terrain; lifted onto the support"
        );
        true
    }

    /// The cell containing the feet after the last step (owner landblock and number).
    #[must_use]
    pub fn cell(&self) -> Option<(LandblockKey, u16)> {
        self.cell
    }

    /// Tracks the mover's cell (`collision.md` §5.8, §5.8.1; `m4-5-visual.md` CELL rule:
    /// the feet raised by [`CELL_PROBE_LIFT`]): the current cell while it still contains the
    /// point (in a doorway both cells do), else a cell its portals lead to that contains it,
    /// else the lookup of §5.8.1 over every set of the view (outdoors when none).
    fn track_cell(
        &mut self,
        view: &CollisionView<'_>,
        world: &dyn CollisionWorld,
        feet: [f64; 3],
    ) -> Option<(LandblockKey, u16)> {
        let p = view.to_view([feet[0], feet[1], feet[2] + CELL_PROBE_LIFT]);
        let key_of = |set: &tessera_physics::geometry::CollisionSet| {
            set.landblock()
                .and_then(|id| LandblockKey::new(id.region, id.x, id.y).ok())
        };
        let contains = |owner: LandblockKey, cell: u16| {
            view.members().iter().any(|(set, offset)| {
                key_of(set) == Some(owner)
                    && set.cells().get(cell).is_some_and(|c| {
                        c.contains([p[0] - offset[0], p[1] - offset[1], p[2] - offset[2]])
                    })
            })
        };
        let found = match self.cell {
            Some((k, c)) if contains(k, c) => Some((k, c)),
            current => current
                .and_then(|(k, c)| {
                    world
                        .portal_neighbours(k, c)
                        .into_iter()
                        .find(|n| contains(k, *n))
                        .map(|n| (k, n))
                })
                .or_else(|| {
                    let hit = view.cell_at(p)?;
                    let (set, _) = view.members().get(hit.set as usize)?;
                    Some((key_of(set)?, hit.cell))
                }),
        };
        self.cell = found;
        found
    }

    /// Turns the facing by `dyaw` radians (counter-clockwise; mouse look), keeping
    /// everything else. Both ends of the interpolation turn, so the whole turn shows at
    /// once ([`PhysicsWalker::interpolated`]; mouse look applies once per frame,
    /// `walker.md` §13.1).
    pub fn turn(&mut self, dyaw: f32) {
        if !dyaw.is_finite() || dyaw == 0.0 {
            return;
        }
        let mut s = *self.controller.state();
        s.heading += dyaw;
        if self.controller.set_state(s) {
            self.previous.heading += dyaw;
        }
    }

    /// The state after the last step.
    #[must_use]
    pub fn state(&self) -> &MoverState {
        self.controller.state()
    }

    /// The controller.
    #[must_use]
    pub fn controller(&self) -> &Controller {
        &self.controller
    }

    /// The region.
    #[must_use]
    pub fn region(&self) -> u8 {
        self.region
    }

    /// The feet as a world position.
    #[must_use]
    pub fn feet(&self) -> WorldPos {
        let p = self.state().position;
        WorldPos::new(self.region, p[0], p[1], p[2])
    }

    /// The last step's log entry.
    #[must_use]
    pub fn last(&self) -> Option<&StepRecord> {
        self.last.as_ref()
    }

    /// Feet and heading interpolated between the last two states by the accumulator's
    /// leftover fraction (ARCHITECTURE §20.3).
    #[must_use]
    pub fn interpolated(&self) -> ([f64; 3], f32) {
        let a = f64::from(self.timestep.alpha().clamp(0.0, 1.0));
        let (p, q) = (self.previous.position, self.state().position);
        let pos = std::array::from_fn(|i| p[i] + (q[i] - p[i]) * a);
        let (h0, h1) = (self.previous.heading, self.state().heading);
        let mut dh = h1 - h0;
        if dh > std::f32::consts::PI {
            dh -= std::f32::consts::TAU;
        } else if dh < -std::f32::consts::PI {
            dh += std::f32::consts::TAU;
        }
        #[allow(clippy::cast_possible_truncation)]
        let heading = h0 + dh * a as f32;
        (pos, heading)
    }

    /// The landblocks the next step needs (`None` = off the grid): those the controller's
    /// query box overlaps (decision 0013), the feet's landblock with its eight neighbours,
    /// the owner of the current cell and the required landblocks
    /// ([`PhysicsWalker::set_required`]). A landblock's placed and generated objects may
    /// reach across its edge, so a neighbour whose set is installed while the mover stands
    /// next to it could put geometry around the capsule (Tessera Decision, `walker.md`
    /// §13.3). A cell's physics mesh belongs to its owner, which may lie several landblocks
    /// away (`collision.md` §5.8.1): without it the mover falls through the cell's floor.
    /// Owners of cells the mover has not entered yet are not known here (`walker.md` §13.3
    /// OPEN).
    #[must_use]
    pub fn needed(&self) -> Vec<Option<LandblockKey>> {
        let (lo, hi) = self.controller.query_bounds();
        let mut out = blocks_in(self.region, lo, hi);
        let mut add = |n: Option<LandblockKey>| {
            if !out.contains(&n) {
                out.push(n);
            }
        };
        if let Some(k) = self.feet().landblock() {
            for dy in -1..=1 {
                for dx in -1..=1 {
                    add(k.offset(dx, dy));
                }
            }
        }
        for k in self.pins() {
            add(Some(k));
        }
        out
    }

    /// The needed landblocks that are not installed yet (empty: the next step can run).
    #[must_use]
    pub fn pending(&self, world: &dyn CollisionWorld) -> Vec<LandblockKey> {
        self.needed()
            .into_iter()
            .flatten()
            .filter(|k| world.readiness(*k) == BlockReadiness::Pending)
            .collect()
    }

    /// Whether the capsule's footprint at `feet` reaches a landblock that is absent or off
    /// the grid.
    fn touches_absent(&self, feet: [f64; 3], world: &dyn CollisionWorld) -> bool {
        let r = f64::from(self.controller.config().shape.radius());
        let lo = [feet[0] - r, feet[1] - r, feet[2]];
        let hi = [feet[0] + r, feet[1] + r, feet[2]];
        blocks_in(self.region, lo, hi)
            .into_iter()
            .any(|k| k.is_none_or(|k| world.readiness(k) == BlockReadiness::Absent))
    }

    /// Runs one step with `input` when every needed set is installed; `None` (nothing
    /// changed) otherwise. `walking` selects the Walking layer (`collision.md` §11.1).
    pub fn step(
        &mut self,
        input: &MoveInput,
        walking: bool,
        world: &dyn CollisionWorld,
    ) -> Option<&StepRecord> {
        if !self.pending(world).is_empty() {
            return None;
        }
        if walking != self.controller.conditions().walking {
            let mut c = *self.controller.conditions();
            c.walking = walking;
            self.controller.set_conditions(c);
        }
        self.walking = walking;
        if self.check_placement {
            let view = world.view(&self.feet());
            self.fix_placement(&view, world);
        }
        let before = *self.controller.state();
        let view = world.view(&self.feet());
        let env = StepEnv::from_view(&view).with_water(CellMaskedWater(&view));
        let report = self.controller.step(input, &env);
        let mut at_boundary = false;
        let after = *self.controller.state();
        if self.touches_absent(after.position, world)
            && !self.touches_absent(before.position, world)
        {
            at_boundary = true;
            let held = MoverState {
                velocity: [0.0; 3],
                steps: after.steps,
                heading: after.heading,
                ..before
            };
            self.controller.set_state(held);
        }
        self.previous = before;
        let state = *self.controller.state();
        let cell = self.track_cell(&view, world, state.position);
        let (contacts, penetration) = if self.details {
            self.contacts(&view, &state)
        } else {
            (Vec::new(), 0.0)
        };
        self.last = Some(StepRecord {
            state,
            region: self.region,
            report,
            cell,
            contacts,
            penetration,
            at_boundary,
            input: *input,
            walking,
        });
        self.last.as_ref()
    }

    /// Contacts of the capsule at `state` within [`CONTACT_LOG_MARGIN`] (deepest first, at
    /// most [`MAX_LOGGED_CONTACTS`]) and the deepest penetration.
    fn contacts(
        &mut self,
        view: &CollisionView<'_>,
        state: &MoverState,
    ) -> (Vec<ContactInfo>, f32) {
        let config = self.controller.config();
        let capsule = config.shape.capsule_at(view.to_view(state.position));
        self.scratch.clear();
        view.contacts(
            &capsule,
            CONTACT_LOG_MARGIN,
            &config.filter,
            &mut self.scratch,
        );
        self.scratch
            .sort_by(|a, b| a.separation.total_cmp(&b.separation));
        let penetration = self
            .scratch
            .first()
            .map_or(0.0, |c| (-c.separation).max(0.0));
        let out = self
            .scratch
            .iter()
            .take(MAX_LOGGED_CONTACTS)
            .map(|c| {
                let set = view.members().get(c.set as usize).map(|(s, _)| *s);
                let kind = set
                    .and_then(|s| s.body(c.body))
                    .map_or(ContactKind::Other, |b| ContactKind::of(b.source));
                let landblock = set
                    .and_then(|s| s.landblock())
                    .and_then(|id| LandblockKey::new(id.region, id.x, id.y).ok());
                ContactInfo {
                    kind,
                    landblock,
                    separation: c.separation,
                    normal_z: c.normal[2],
                }
            })
            .collect();
        if self.scratch.capacity() > 4 * MAX_LOGGED_CONTACTS * 64 {
            self.scratch = Vec::new();
        }
        (out, penetration)
    }

    /// One frame of `dt` seconds (ARCHITECTURE §20.3 step 3): latches `input` (a jump press
    /// is kept until a step takes it), then runs the whole steps the accumulator allows (at
    /// most [`MAX_STEPS_PER_FRAME`]; time beyond is dropped). While a needed set is missing
    /// no step runs, the accumulated time is dropped and the latched input kept.
    /// `turn` (radians per second, counter-clockwise) turns the heading per step.
    pub fn frame(
        &mut self,
        dt: f64,
        input: &MoveInput,
        turn: f32,
        walking: bool,
        world: &dyn CollisionWorld,
    ) -> FrameSteps {
        self.jump |= input.jump;
        let ticks = self
            .timestep
            .advance(if dt.is_finite() { dt.max(0.0) } else { 0.0 });
        let mut out = FrameSteps::default();
        for _ in 0..ticks {
            if !self.pending(world).is_empty() {
                self.timestep.reset();
                out.waiting = true;
                break;
            }
            let heading = input.heading.unwrap_or(self.state().heading)
                + if turn.is_finite() {
                    turn * STEP_SECONDS
                } else {
                    0.0
                };
            let step_input = MoveInput {
                heading: Some(heading),
                jump: self.jump,
                ..*input
            };
            if self.step(&step_input, walking, world).is_some() {
                self.jump = false;
                out.steps += 1;
            }
        }
        out
    }
}
