// SPDX-License-Identifier: GPL-3.0-or-later
//! The fixed step (`collision.md` §12.2–§12.3, Tessera Decision where the original is
//! OPEN) and the controller that runs it.
//!
//! One call of [`Controller::step`] advances the mover by exactly [`STEP_SECONDS`]:
//!
//! 1. **Parameters** from §11.1 for the entity conditions and the state of the previous
//!    step, float height §11.2.
//! 2. **Push-out.** Overlaps with the geometry (a set installed under the mover, hostile
//!    data, a crevice the mover wedged into) are resolved first: up to 4 rounds, each the
//!    shortest push that clears every contact plane closer than two skins (an active set
//!    of at most three planes solved exactly), at most [`MAX_DEPENETRATION`] in all.
//! 3. **Velocity** (§12.3 steps 1–3). Per local axis (forward, side) the velocity moves
//!    toward the target at the control acceleration, but never faster than the rate that
//!    reaches the axis' top speed from rest in the ramp time, and never past the target.
//!    On walkable support (states 1, 5) the vertical speed is 0 unless a jump starts; a
//!    jump's push is the jump acceleration applied for one ramp time, given in the jump's
//!    step (its integral: 40 × 0.13 = 5.2 m/s for players). The ramp time is the BasePlayer
//!    layer's ([`PlayerMovementRecord::jump_ramp_time`]), so walking and mounted jumps
//!    match. Otherwise the layer's gravity is added. Each local component is multiplied by
//!    its damper to the power of the step length, then the speed is limited to the terminal
//!    velocity (and [`MAX_SPEED`]). The vertical displacement uses the mean of the vertical
//!    speeds before and after gravity, so a jump's apex is v²/2g at any step.
//! 4. **Buoyancy** (in water without walkable support, states 2 and 3; Decision): the
//!    float line is the water surface minus the float height. Below it (or on it) the
//!    layer's gravity applies (−20 for swimmers lifts the mover), the water takes up any
//!    downward speed (§12.3 OPEN (p)) and the feet stop at the line, the upward speed then
//!    zeroed. Above it the gravity's magnitude pulls the mover down (at least
//!    [`MIN_SINK_ACCELERATION`]) and the feet stop at the line, the speed then zeroed, so a
//!    swimmer never overshoots it. The speed is limited again afterwards.
//! 5. **Sweep and slide** (§12.3 step 4): up to [`MAX_SLIDE_ITERATIONS`] sweeps; each hit
//!    keeps the motion up to the contact and removes the motion and velocity into every
//!    contact plane so far (normals merged by §11.3's 0.035 tolerance; two planes slide
//!    along their crease, more stop). Contacts are classified by §11.3 (the face normal
//!    where it is steeper than walkable and less upward than the contact normal):
//!    - on walkable support (states 1, 5), a walkable contact keeps its normal, so walkable
//!      slopes are climbed by the slide itself; a hit whose contact normal does not oppose
//!      the motion while its triangle's face does slides along the face (a crease, §12.3
//!      OPEN (n)); a **steep** contact (45°–85° for players) is ridden onto: the motion
//!      keeps its normal, so the capsule can come to rest on the steep face and step 7
//!      classifies that face underfoot (state 6), while the velocity meets it as a vertical
//!      wall, so riding onto it gives no upward speed (§11.3 mechanism, §12.3 step 6,
//!      OPEN (e)–(f)); every other contact (wall, overhang, ignored surface) counts as a
//!      vertical wall (its normal's horizontal part), so walls and risers never lift the
//!      mover;
//!    - on steep support (states 3, 6), contacts keep their normals and a steep contact
//!      also adds the vertical plane of its normal's horizontal part, so the mover slides
//!      down but never climbs the slope (§11.3 mechanism);
//!    - in the air or swimming, contacts keep their normals.
//! 6. **Step-up** (§12.3 step 5): when a grounded horizontal move met a contact that is not
//!    walkable (a wall or a steep contact), the move is retried from the position raised
//!    by the step-up height (less where a ceiling stops the rise), covering at least
//!    [`STEP_OVER_FRACTION`] of the capsule radius (the full radius when that landing is
//!    not walkable, §12.3 OPEN (o)), then swept down by up to the rise plus
//!    [`SNAP_DISTANCE`]. It is taken when it lands on walkable
//!    support (§11.3), climbs more than [`STEP_UP_MIN_RISE`] and gets further along the
//!    move than the blocked path. When the raised move covered more than the move itself,
//!    the same climb with only the move's length is preferred if it also lands walkable
//!    (and climbs in proportion), so a step-up never moves the mover faster than its input
//!    except to get a capsule resting against a riser onto the step.
//! 7. **Ground snap and state** (§12.3 step 6): unless jumping, rising in the air or
//!    swimming, a sweep down by [`SNAP_DISTANCE`] moves the mover onto support it finds; on
//!    walkable support the sweep reaches further by what a walkable slope descends over the
//!    step's horizontal motion (at most the step-up height), so the mover follows downhill
//!    slopes and falls off steps and ledges. The support probe of §11.3 then classifies the
//!    contacts of the lower cap within [`SUPPORT_MARGIN`] (none while rising in the air out
//!    of water), walkable over steep over none: the support underfoot is steep (state 6)
//!    once no walkable contact is left within the margin, as when the capsule has ridden
//!    onto a steep face in step 5, whatever the state of the previous step.
//! 8. **Water** (§12.3 step 7, `water.md` §7.4): the [`super::WaterQuery`] at the feet.
//! 9. **Cell** (§12.3 step 8): the cell lookup of §5.8.1 at the feet, every step.
//!
//! Every value is checked: a step whose result is not finite or moves the mover more than
//! [`MAX_STEP_DISPLACEMENT`] is undone and the velocity zeroed ([`StepOutcome::Reverted`]).
//! A step's work is bounded by its query count (at most 11 capsule sweeps and 7 contact
//! queries, each visiting at most [`crate::geometry::MAX_QUERY_ITEMS`] items per set) and
//! the water query; the step keeps no buffer larger than [`SCRATCH_KEEP`] entries.
//! The arithmetic is plain IEEE without fused operations; the damper power is found by
//! bisection, so only the heading's sine and cosine come from the platform's library.

use std::sync::Arc;

use crate::geometry::{
    CellHit, CollisionQuery, CollisionView, Contact, LiquidFlags, MAX_COORDINATE, QueryFilter,
};

use super::contact::{
    ContactClass, always_uncontrolled, best_support, classify, normals_merge, walkable_cos,
};
use super::params::{
    LayerConditions, MAX_SPEED, MovementParams, MovementPids, PlayerMovementRecord,
};
use super::shape::CharacterShape;
use super::state::{MovementState, Support, WaterDepth};
use super::water::{LiquidWater, NoWater, WaterQuery};

/// Controller steps per second (`collision.md` §12.3; decision 0013).
pub const STEPS_PER_SECOND: u32 = 60;
/// Length of one controller step, s (`collision.md` §12.3; decision 0013).
pub const STEP_SECONDS: f32 = 1.0 / 60.0;
/// Most controller steps per frame; time beyond them is dropped (`collision.md` §12.3).
pub const MAX_STEPS_PER_FRAME: u32 = 5;
/// Most sweeps per step (`collision.md` §12.3 step 4).
pub const MAX_SLIDE_ITERATIONS: usize = 4;
/// The ground-snap distance and the extra reach of the step-up's down sweep, m
/// (`collision.md` §12.3 steps 5–6).
pub const SNAP_DISTANCE: f32 = 0.05;
/// Contacts within this gap count for support classification, m (Tessera Decision: the
/// sweeps leave 2–2.5 skins, 0.002–0.0025 m).
pub const SUPPORT_MARGIN: f32 = 0.01;
/// Most a step pushes the mover out of overlapping geometry, m (Tessera Decision).
pub const MAX_DEPENETRATION: f32 = 0.5;
/// The raised move of a step-up covers at least this fraction of the capsule radius
/// (Tessera Decision): a capsule resting against a riser then gets its centre within
/// r·sin 45° of the edge, so it lands walkable (§11.3), at any approach speed.
pub const STEP_OVER_FRACTION: f32 = 0.5;
/// Most one step may move the mover, m (decision 0013, documented bound): at most 2 m of
/// swept motion at [`MAX_SPEED`] or a step-up of rise, move and descent (≤ 6.1 m), a snap
/// (≤ 2.05 m) and a push-out (≤ 0.5 m). A step that would move further is undone.
pub const MAX_STEP_DISPLACEMENT: f32 = 10.0;
/// A step-up must lift the mover by more than this, m (Tessera Decision): on level ground
/// the raised move is never taken, so sliding along a wall is never sped up.
pub const STEP_UP_MIN_RISE: f32 = 0.01;
/// Least downward acceleration of a mover above its float line in water, m/s² (Tessera
/// Decision; used when the layer's gravity is smaller in magnitude, e.g. 0).
pub const MIN_SINK_ACCELERATION: f32 = 1.0;
/// Feet within this distance above the float line count as on it, m (Tessera Decision).
pub const FLOAT_TOLERANCE: f32 = 0.001;
/// Capacity the controller's scratch buffers are shrunk to after a step that grew them
/// beyond it (bounded memory after hostile geometry).
pub const SCRATCH_KEEP: usize = 1024;
/// Most contact planes one slide keeps.
const MAX_PLANES: usize = 8;
/// Most contact planes one push-out considers (the deepest).
const MAX_PUSH_PLANES: usize = 32;

/// One step's input (`keymap.md` §9.1 conventions). Axes are sanitised: non-finite → 0,
/// then clamped to −1..=1; a combined (forward, strafe) longer than 1 is normalised.
#[derive(Debug, Clone, Copy, PartialEq, Default)]
pub struct MoveInput {
    /// Forward (+) / backward (−).
    pub forward: f32,
    /// Strafe right (+) / left (−).
    pub strafe: f32,
    /// The new heading (yaw, radians, counter-clockwise from +X), or `None` to keep it.
    /// A non-finite value keeps it too.
    pub heading: Option<f32>,
    /// Start a jump (taken only on walkable support; the caller latches the press).
    pub jump: bool,
}

impl MoveInput {
    /// The input with its axes sanitised.
    #[must_use]
    pub fn sanitized(&self) -> Self {
        let axis = |v: f32| {
            if v.is_finite() {
                v.clamp(-1.0, 1.0)
            } else {
                0.0
            }
        };
        let (mut f, mut s) = (axis(self.forward), axis(self.strafe));
        let l2 = f * f + s * s;
        if l2 > 1.0 {
            let l = l2.sqrt();
            f /= l;
            s /= l;
        }
        Self {
            forward: f,
            strafe: s,
            heading: self.heading.filter(|h| h.is_finite()),
            jump: self.jump,
        }
    }
}

/// Wraps a heading into (−π, π].
fn wrap_heading(h: f32) -> f32 {
    use std::f32::consts::{PI, TAU};
    let w = h.rem_euclid(TAU);
    let w = if w > PI { w - TAU } else { w };
    if w.is_finite() { w } else { 0.0 }
}

/// The water the mover was in at the end of a step.
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct WaterContact {
    /// Water surface height, region frame.
    pub surface: f64,
    /// Surface height above the feet, m.
    pub depth: f32,
    /// The liquid boxes' flags.
    pub flags: LiquidFlags,
}

/// The mover's state after a step.
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct MoverState {
    /// Feet position, f64 region frame (decision 0013 "Positions").
    pub position: [f64; 3],
    /// Velocity, m/s, world axes.
    pub velocity: [f32; 3],
    /// Heading (yaw, radians, (−π, π]).
    pub heading: f32,
    /// `Entity_Element` (§10.4).
    pub element: MovementState,
    /// The support found at the end of the step (§11.3).
    pub support: Support,
    /// The water at the feet, if any.
    pub water: Option<WaterContact>,
    /// The cell containing the feet (§5.8.1), if any; its `set` index refers to the
    /// collision query of the step.
    pub cell: Option<CellHit>,
    /// Steps taken.
    pub steps: u64,
}

impl MoverState {
    /// A mover standing still at `position` (state 4 until the first step finds support).
    #[must_use]
    pub fn at(position: [f64; 3], heading: f32) -> Self {
        Self {
            position,
            velocity: [0.0; 3],
            heading: wrap_heading(heading),
            element: MovementState::EarthNoSurface,
            support: Support::None,
            water: None,
            cell: None,
            steps: 0,
        }
    }

    /// Whether every number is finite.
    #[must_use]
    pub fn is_finite(&self) -> bool {
        self.position.iter().all(|c| c.is_finite())
            && self.velocity.iter().all(|c| c.is_finite())
            && self.heading.is_finite()
            && self
                .water
                .is_none_or(|w| w.surface.is_finite() && w.depth.is_finite())
    }
}

/// What a step needs from the world: the collision query, the water query and the
/// region-frame origin of the query's frame (a query point p is region point p + origin).
/// Keep the origin near the mover so f32 positions stay precise (within a landblock or two).
///
/// Over a [`CollisionView`] use [`StepEnv::from_view`], which takes the view's own origin
/// and its column water query. For a single [`crate::geometry::CollisionSet`] the origin
/// is the set's frame in the region frame (its landblock's corner,
/// [`crate::geometry::landblock_origin`]); `StepEnv::new(&set, origin).with_water(&set)`
/// uses the set's column water query.
#[derive(Debug, Clone, Copy)]
pub struct StepEnv<'a, Q: ?Sized, W> {
    /// Collision.
    pub collision: &'a Q,
    /// Water.
    pub water: W,
    /// Region-frame origin of the query frame.
    pub origin: [f64; 3],
}

impl<'a, Q: CollisionQuery + ?Sized> StepEnv<'a, Q, LiquidWater<'a, Q>> {
    /// Collision and water from the same query (its liquid boxes and water planes,
    /// decision 0013 "Water").
    #[must_use]
    pub fn new(collision: &'a Q, origin: [f64; 3]) -> Self {
        Self {
            collision,
            water: LiquidWater(collision),
            origin,
        }
    }
}

impl<'a, 'v> StepEnv<'a, CollisionView<'v>, &'a CollisionView<'v>> {
    /// Collision and water of `view`, with the view's own origin
    /// ([`CollisionView::origin`]), so positions cannot be shifted by a mismatched origin.
    /// The water is the view's column query (`impl WaterQuery for CollisionView`); add
    /// `.with_water(NoWater)` for a dry step.
    #[must_use]
    pub fn from_view(view: &'a CollisionView<'v>) -> Self {
        Self {
            collision: view,
            water: view,
            origin: view.origin(),
        }
    }
}

impl<'a, Q: CollisionQuery + ?Sized> StepEnv<'a, Q, NoWater> {
    /// Collision without water.
    #[must_use]
    pub fn dry(collision: &'a Q, origin: [f64; 3]) -> Self {
        Self {
            collision,
            water: NoWater,
            origin,
        }
    }
}

impl<'a, Q: ?Sized, W> StepEnv<'a, Q, W> {
    /// The same collision with another water query (for example `tessera-world`'s own).
    #[must_use]
    pub fn with_water<W2: WaterQuery>(self, water: W2) -> StepEnv<'a, Q, W2> {
        StepEnv {
            collision: self.collision,
            water,
            origin: self.origin,
        }
    }
}

/// How a step ended.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum StepOutcome {
    /// The step ran.
    Stepped,
    /// The mover is too far from the query frame's origin (beyond
    /// [`MAX_COORDINATE`] − [`MAX_STEP_DISPLACEMENT`] in f32); nothing changed but the step
    /// count (the input's heading is not applied either).
    OutOfRange,
    /// The result was not finite or moved too far; the step was undone and the velocity
    /// zeroed.
    Reverted,
}

/// What one step did.
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct StepReport {
    /// How it ended.
    pub outcome: StepOutcome,
    /// A jump started.
    pub jumped: bool,
    /// A step-up was taken.
    pub stepped_up: bool,
    /// The ground snap moved the mover down.
    pub snapped: bool,
    /// Contact planes met while sliding.
    pub planes: u8,
    /// Distance pushed out of overlaps, m.
    pub pushed_out: f32,
}

impl StepReport {
    fn new(outcome: StepOutcome) -> Self {
        Self {
            outcome,
            jumped: false,
            stepped_up: false,
            snapped: false,
            planes: 0,
            pushed_out: 0.0,
        }
    }
}

/// How the controller is set up.
#[derive(Debug, Clone, PartialEq)]
pub struct ControllerConfig {
    /// The capsule (§10.6).
    pub shape: CharacterShape,
    /// Which bodies the mover collides with (§9.2); a player by default.
    pub filter: QueryFilter,
    /// Whether the mover is a player character (§11.3: walkable limit 45° instead of 70°).
    /// The local avatar is one (Tessera Decision, §11.3).
    pub is_player: bool,
}

impl Default for ControllerConfig {
    fn default() -> Self {
        Self {
            shape: CharacterShape::PLAYABLE,
            filter: QueryFilter::player(),
            is_player: true,
        }
    }
}

/// The character controller (`collision.md` §12).
#[derive(Debug, Clone)]
pub struct Controller {
    config: ControllerConfig,
    record: Arc<PlayerMovementRecord>,
    pids: Arc<MovementPids>,
    conditions: LayerConditions,
    state: MoverState,
    scratch: Vec<Contact>,
    push_planes: Vec<([f64; 3], f64)>,
}

/// The result of one slide.
#[derive(Debug, Clone, Copy)]
struct Slide {
    feet: [f32; 3],
    vel: [f32; 3],
    blocked: bool,
    planes: u8,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum Mode {
    /// In the air or swimming: raw contact normals.
    Free,
    /// On walkable support: walkable contacts keep their normal, steep contacts are ridden
    /// onto (the motion keeps their normal, the velocity meets them as a vertical wall),
    /// every other contact is a vertical wall.
    Grounded,
    /// On steep support (states 3 and 6): raw contact normals, and a steep contact adds
    /// the vertical plane of its normal's horizontal part, which stops motion up the slope.
    Steep,
    /// The raised move of a step-up: every contact is a vertical wall.
    Raised,
}

impl Controller {
    /// A controller for a mover standing at `start` (its heading wrapped, its speed limited
    /// to [`MAX_SPEED`]). The start position must be finite ([`MoverState::is_finite`]):
    /// otherwise every step reports [`StepOutcome::OutOfRange`] until
    /// [`Controller::set_state`] gives a finite one.
    #[must_use]
    pub fn new(
        config: ControllerConfig,
        record: Arc<PlayerMovementRecord>,
        pids: Arc<MovementPids>,
        conditions: LayerConditions,
        start: MoverState,
    ) -> Self {
        Self {
            config,
            record,
            pids,
            conditions,
            state: Self::sanitize_state(start),
            scratch: Vec::new(),
            push_planes: Vec::new(),
        }
    }

    /// The state after the last step.
    #[must_use]
    pub fn state(&self) -> &MoverState {
        &self.state
    }

    /// Replaces the state (teleport, spawn). A non-finite state is refused and the old one
    /// kept; returns whether it was taken. The heading is wrapped and a velocity faster
    /// than [`MAX_SPEED`] is scaled down to it.
    pub fn set_state(&mut self, state: MoverState) -> bool {
        if state.is_finite() {
            self.state = Self::sanitize_state(state);
            true
        } else {
            false
        }
    }

    /// `state` with its heading wrapped and its speed limited to [`MAX_SPEED`] (a
    /// non-finite velocity becomes zero).
    fn sanitize_state(state: MoverState) -> MoverState {
        let mut velocity = state.velocity;
        if !velocity.iter().all(|c| c.is_finite()) {
            velocity = [0.0; 3];
        }
        limit_speed(&mut velocity, MAX_SPEED);
        MoverState {
            heading: wrap_heading(state.heading),
            velocity,
            ..state
        }
    }

    /// The configuration.
    #[must_use]
    pub fn config(&self) -> &ControllerConfig {
        &self.config
    }

    /// The entity conditions of §11.1.
    #[must_use]
    pub fn conditions(&self) -> &LayerConditions {
        &self.conditions
    }

    /// Changes the entity conditions (mounting, walk style, stealth, float height…); the
    /// next step composes its parameters from them.
    pub fn set_conditions(&mut self, conditions: LayerConditions) {
        self.conditions = conditions;
    }

    /// The parameters the next step uses (§11.1 for the current state, float height §11.2;
    /// [`PlayerMovementRecord::compose_movement`], no allocation).
    #[must_use]
    pub fn params(&self) -> MovementParams {
        self.record
            .compose_movement(&self.conditions, self.state.element)
    }

    /// The property IDs the controller was built with.
    #[must_use]
    pub fn pids(&self) -> &Arc<MovementPids> {
        &self.pids
    }

    /// The region-frame box a step may touch: the capsule's box grown on every side by
    /// [`MAX_STEP_DISPLACEMENT`]. The landblocks it overlaps must have installed collision
    /// sets before [`Controller::step`] runs (decision 0013 "Collision-set residency").
    #[must_use]
    pub fn query_bounds(&self) -> ([f64; 3], [f64; 3]) {
        let s = &self.config.shape;
        let m = f64::from(MAX_STEP_DISPLACEMENT);
        let r = f64::from(s.radius());
        let p = self.state.position;
        let bottom = f64::from(s.lower_cap_z()) - r;
        let top = f64::from(s.height());
        (
            [p[0] - r - m, p[1] - r - m, p[2] + bottom - m],
            [p[0] + r + m, p[1] + r + m, p[2] + top + m],
        )
    }

    /// Advances the mover by one step of [`STEP_SECONDS`] (module docs). Never panics; the
    /// state stays finite.
    pub fn step<Q, W>(&mut self, input: &MoveInput, env: &StepEnv<'_, Q, W>) -> StepReport
    where
        Q: CollisionQuery + ?Sized,
        W: WaterQuery,
    {
        let before = self.state;
        let mut report = self.step_inner(input, env);
        let ok = self.state.is_finite() && {
            let p = self.state.position;
            let d2: f64 = (0..3).map(|i| (p[i] - before.position[i]).powi(2)).sum();
            d2 <= f64::from(MAX_STEP_DISPLACEMENT).powi(2)
        };
        if !ok {
            self.state = MoverState {
                velocity: [0.0; 3],
                ..before
            };
            report = StepReport::new(StepOutcome::Reverted);
        }
        self.state.steps = before.steps.wrapping_add(1);
        if self.scratch.capacity() > SCRATCH_KEEP {
            self.scratch.clear();
            self.scratch.shrink_to(SCRATCH_KEEP);
        }
        if self.push_planes.capacity() > SCRATCH_KEEP {
            self.push_planes.clear();
            self.push_planes.shrink_to(SCRATCH_KEEP);
        }
        report
    }

    #[allow(clippy::too_many_lines)]
    fn step_inner<Q, W>(&mut self, input: &MoveInput, env: &StepEnv<'_, Q, W>) -> StepReport
    where
        Q: CollisionQuery + ?Sized,
        W: WaterQuery,
    {
        let dt = STEP_SECONDS;
        let input = input.sanitized();
        let q = env.collision;
        let origin = env.origin;
        #[allow(clippy::cast_possible_truncation)]
        let feet0: [f32; 3] = std::array::from_fn(|i| (self.state.position[i] - origin[i]) as f32);
        let limit = MAX_COORDINATE - MAX_STEP_DISPLACEMENT - 100.0;
        if !feet0.iter().all(|c| c.is_finite() && c.abs() <= limit) {
            return StepReport::new(StepOutcome::OutOfRange);
        }
        if let Some(h) = input.heading {
            self.state.heading = wrap_heading(h);
        }
        let element = self.state.element;
        let params = self.params();
        let cosw = walkable_cos(self.config.is_player);
        let mut report = StepReport::new(StepOutcome::Stepped);

        // Push-out.
        let (mut feet, pushed) = self.depenetrate(q, feet0);
        report.pushed_out = pushed;

        // Velocity.
        let (sin, cos) = self.state.heading.sin_cos();
        let fwd = [cos, sin];
        let right = [sin, -cos];
        let v = self.state.velocity;
        let mut vf = v[0] * fwd[0] + v[1] * fwd[1];
        let mut vs = v[0] * right[0] + v[1] * right[1];
        let mut vz = v[2];
        let tf = if input.forward >= 0.0 {
            input.forward * params.forward_speed
        } else {
            input.forward * params.backward_speed
        };
        let ts = if input.strafe >= 0.0 {
            input.strafe * params.strafe_right_speed
        } else {
            input.strafe * params.strafe_left_speed
        };
        let rate = |axis: usize, top: f32| {
            let a = params.control_acceleration[axis];
            if params.ramp_time > 0.0 {
                a.min(top / params.ramp_time)
            } else {
                a
            }
        };
        vf = approach(
            vf,
            tf,
            rate(0, params.forward_speed.max(params.backward_speed)) * dt,
        );
        vs = approach(
            vs,
            ts,
            rate(1, params.strafe_left_speed.max(params.strafe_right_speed)) * dt,
        );
        let walk_support = element.on_walkable();
        // The float line (buoyancy, module docs step 4): in water without walkable support.
        let float_line = if element.in_water() && !walk_support {
            #[allow(clippy::cast_possible_truncation)]
            self.state
                .water
                .map(|w| (w.surface - origin[2]) as f32 - params.float_height)
                .filter(|t| t.is_finite())
        } else {
            None
        };
        let above_line = float_line.is_some_and(|t| feet[2] > t + FLOAT_TOLERANCE);
        let vz_start;
        if walk_support {
            vz = 0.0;
            if input.jump {
                let j = (params.jump_acceleration * self.record.jump_ramp_time()).min(MAX_SPEED);
                if j > 0.0 {
                    vz = j;
                    report.jumped = true;
                }
            }
            vz_start = vz;
        } else {
            vz_start = vz;
            let gravity = if above_line {
                params.gravity.abs().max(MIN_SINK_ACCELERATION)
            } else {
                params.gravity
            };
            vz -= gravity * dt;
        }
        let d = params.velocity_damper.map(step_factor);
        vf *= d[0];
        vs *= d[1];
        vz *= d[2];
        let speed_limit = params.terminal_velocity.min(MAX_SPEED);
        let mut local = [vf, vs, vz];
        limit_speed(&mut local, speed_limit);
        let [vf, vs, mut vz] = local;
        let vz_start = vz_start.max(-speed_limit).min(speed_limit);
        let mut dz = 0.5 * (vz_start + vz) * dt;

        // Buoyancy: the feet stop at the float line from either side.
        if let Some(t) = float_line {
            if above_line {
                if feet[2] + dz < t {
                    dz = t - feet[2];
                    vz = 0.0;
                }
            } else {
                // Below the line the water takes up any downward speed, so a mover that
                // slides or falls in at speed stops instead of diving metres below the line
                // before the lift turns it (`collision.md` §12.3 OPEN (p), Tessera reading
                // of `water.md` §7.4).
                vz = vz.max(0.0);
                dz = dz.max(0.0);
                if feet[2] + dz > t {
                    dz = t - feet[2];
                    vz = vz.min(0.0);
                }
            }
        }
        // The buoyancy may have changed the vertical speed: limit the speed again.
        let mut local = [vf, vs, vz];
        limit_speed(&mut local, speed_limit);
        let [vf, vs, vz] = local;
        let lim = speed_limit * dt;
        let dz = dz.max(-lim).min(lim);
        let vel = [fwd[0] * vf + right[0] * vs, fwd[1] * vf + right[1] * vs, vz];
        let delta = [vel[0] * dt, vel[1] * dt, dz];

        // Sweep and slide, step-up.
        let grounded = walk_support && !report.jumped;
        let mode = if grounded {
            Mode::Grounded
        } else if element.on_steep() {
            Mode::Steep
        } else {
            Mode::Free
        };
        let feet_before = feet;
        let plain = self.slide(q, feet, delta, vel, mode, cosw);
        let mut result = plain;
        if grounded
            && plain.blocked
            && params.step_up_height > 0.0
            && let Some(s) = self.step_up(q, feet, delta, vel, params.step_up_height, &plain, cosw)
        {
            result = s;
            report.stepped_up = true;
        }
        feet = result.feet;
        let vel = result.vel;
        report.planes = result.planes;

        // Ground snap.
        let swimming = element == MovementState::WaterNoSurface;
        // Rising in the air (§12.3 OPEN (f)); in water the swimming gravity lifts the mover
        // and support stays.
        let rising = !grounded && vel[2] > 0.0 && !element.in_water();
        if !report.jumped && !swimming && !rising {
            let extra = if grounded {
                let moved = [feet[0] - feet_before[0], feet[1] - feet_before[1], 0.0];
                (len3(moved) * walkable_tan(cosw)).min(params.step_up_height)
            } else {
                0.0
            };
            let reach = SNAP_DISTANCE
                + if extra.is_finite() {
                    extra.max(0.0)
                } else {
                    0.0
                };
            let cap = self.config.shape.capsule_at(feet);
            if let Some(h) = q.sweep_capsule(&cap, [0.0, 0.0, -reach], &self.config.filter) {
                let class = classify(
                    h.normal,
                    h.face_normal,
                    always_uncontrolled(q, h.set, h.body),
                    cosw,
                );
                if class.support() != Support::None && h.fraction > 0.0 {
                    feet[2] -= reach * h.fraction;
                    report.snapped = true;
                }
            }
        }

        // Support.
        let support = if report.jumped || rising {
            Support::None
        } else {
            self.scratch.clear();
            let probe = self.config.shape.support_probe(feet);
            q.contacts(
                &probe,
                SUPPORT_MARGIN,
                &self.config.filter,
                &mut self.scratch,
            );
            best_support(q, &self.scratch, cosw)
        };

        // Water.
        let water = env.water.water_at(feet).filter(|w| w.surface.is_finite());
        let depth = water.map(|w| {
            let depth = w.surface - feet[2];
            WaterDepth {
                depth,
                swimming: depth > params.float_height,
            }
        });
        let new_element = MovementState::from_support(support, depth);

        // Cell.
        let cell = q.cell_at(feet);

        // Write back.
        let moved: [f64; 3] = std::array::from_fn(|i| f64::from(feet[i]) - f64::from(feet0[i]));
        let position: [f64; 3] = std::array::from_fn(|i| self.state.position[i] + moved[i]);
        self.state = MoverState {
            position,
            velocity: vel,
            heading: self.state.heading,
            element: new_element,
            support,
            water: water.zip(depth).map(|(w, d)| WaterContact {
                surface: f64::from(w.surface) + origin[2],
                depth: d.depth,
                flags: w.flags,
            }),
            cell,
            steps: self.state.steps,
        };
        report
    }

    /// Pushes the capsule out of overlapping geometry (module docs, step 2).
    fn depenetrate<Q: CollisionQuery + ?Sized>(
        &mut self,
        q: &Q,
        mut feet: [f32; 3],
    ) -> ([f32; 3], f32) {
        let target = 2.0 * f64::from(crate::geometry::SKIN);
        let mut total = 0.0_f32;
        let mut constraints = std::mem::take(&mut self.push_planes);
        for _ in 0..4 {
            self.scratch.clear();
            let cap = self.config.shape.capsule_at(feet);
            q.contacts(&cap, SUPPORT_MARGIN, &self.config.filter, &mut self.scratch);
            if !self
                .scratch
                .iter()
                .any(|c| c.separation < 0.0 && c.normal.iter().all(|v| v.is_finite()))
            {
                break;
            }
            // Every contact closer than the target gap is a plane the push must clear
            // (the deepest first, at most MAX_PUSH_PLANES of them).
            constraints.clear();
            constraints.extend(self.scratch.iter().filter_map(|c| {
                let n = c.normal.map(f64::from);
                let len = (n[0] * n[0] + n[1] * n[1] + n[2] * n[2]).sqrt();
                let sep = f64::from(c.separation);
                (sep.is_finite() && sep < target && (len - 1.0).abs() < 1e-3)
                    .then_some((n, target - sep))
            }));
            if constraints.len() > MAX_PUSH_PLANES {
                constraints.select_nth_unstable_by(MAX_PUSH_PLANES - 1, |a, b| b.1.total_cmp(&a.1));
                constraints.truncate(MAX_PUSH_PLANES);
            }
            constraints.sort_by(|a, b| b.1.total_cmp(&a.1));
            let Some(p) = solve_push(&constraints) else {
                break;
            };
            let len = (p[0] * p[0] + p[1] * p[1] + p[2] * p[2]).sqrt();
            let budget = f64::from(MAX_DEPENETRATION - total);
            if !(len.is_finite() && len > 0.0) || budget <= 0.0 {
                break;
            }
            let k = if len > budget { budget / len } else { 1.0 };
            #[allow(clippy::cast_possible_truncation)]
            let push = p.map(|c| (c * k) as f32);
            feet = add3(feet, push);
            #[allow(clippy::cast_possible_truncation)]
            {
                total += (len * k) as f32;
            }
        }
        constraints.clear();
        self.push_planes = constraints;
        (feet, total)
    }

    /// Sweeps and slides (module docs, step 5).
    fn slide<Q: CollisionQuery + ?Sized>(
        &self,
        q: &Q,
        mut feet: [f32; 3],
        delta: [f32; 3],
        mut vel: [f32; 3],
        mode: Mode,
        cosw: f32,
    ) -> Slide {
        // The planes the motion slides along and those the velocity loses its part into;
        // they differ only for steep contacts ridden from walkable support.
        let mut planes = Planes::default();
        let mut vel_planes = Planes::default();
        let mut remaining = delta;
        let mut blocked = false;
        let vel_in = vel;
        let mut walk_planes = Planes::default();
        let mut only_walkable = true;
        for _ in 0..MAX_SLIDE_ITERATIONS {
            if len3(remaining) <= 1e-7 {
                break;
            }
            let cap = self.config.shape.capsule_at(feet);
            let Some(h) = q.sweep_capsule(&cap, remaining, &self.config.filter) else {
                feet = add3(feet, remaining);
                break;
            };
            let f = if h.fraction.is_finite() {
                h.fraction.clamp(0.0, 1.0)
            } else {
                0.0
            };
            feet = add3(feet, scale3(remaining, f));
            remaining = scale3(remaining, 1.0 - f);
            let class = classify(
                h.normal,
                h.face_normal,
                always_uncontrolled(q, h.set, h.body),
                cosw,
            );
            let mut n = h.normal;
            if !n.iter().all(|c| c.is_finite()) || len3(n) < 0.5 {
                // No usable normal: stop the motion.
                n = normalize3(scale3(remaining, -1.0)).unwrap_or([0.0, 0.0, 1.0]);
            }
            // A hit whose contact normal (an edge or corner of the triangle) does not oppose
            // the motion would leave the motion unclipped, and the next sweep would report
            // the same hit at the same place, stopping the mover dead on a slight terrain
            // crease. On walkable support, slide along the triangle's face instead when the
            // face opposes the motion (§12.3 OPEN (n)).
            if mode == Mode::Grounded
                && dot3(remaining, n) >= 0.0
                && let Some(face) = h.face_normal
                && face.iter().all(|c| c.is_finite())
                && len3(face) > 0.5
                && dot3(remaining, face) < 0.0
            {
                n = face;
            }
            if class != ContactClass::Walkable {
                blocked = true;
            }
            match (mode, class) {
                (Mode::Free, _)
                | (Mode::Grounded | Mode::Steep, ContactClass::Walkable)
                | (
                    Mode::Steep,
                    ContactClass::Wall | ContactClass::Overhang | ContactClass::Ignored,
                ) => {
                    planes.add(n);
                    vel_planes.add(n);
                }
                // A steep contact met from walkable support is ridden onto: the motion
                // slides along it, so the capsule can come to rest on the steep face and
                // the ground snap and support probe classify that face underfoot (state 6,
                // `collision.md` §11.3, §12.3 step 6, OPEN (e)–(f)); the velocity meets it
                // as a vertical wall, so riding onto it gives no upward speed.
                (Mode::Grounded, ContactClass::Steep) => {
                    planes.add(n);
                    vel_planes.add(horizontal(n));
                }
                // On steep support a steep contact also adds the vertical plane of its
                // normal's horizontal part: the mover slides down but cannot climb (§11.3).
                (Mode::Steep, ContactClass::Steep) => {
                    let h = horizontal(n);
                    planes.add(n);
                    planes.add(h);
                    vel_planes.add(n);
                    vel_planes.add(h);
                }
                // Walls, overhangs and ignored surfaces met from walkable support, and every
                // contact of a step-up's raised move: vertical walls, so they never lift the
                // mover.
                (Mode::Grounded, _) | (Mode::Raised, _) => {
                    let h = horizontal(n);
                    planes.add(h);
                    vel_planes.add(h);
                }
            }
            remaining = planes.clip(remaining);
            vel = vel_planes.clip(vel);
            if mode == Mode::Grounded && class == ContactClass::Walkable {
                walk_planes.add(n);
            } else {
                only_walkable = false;
            }
            if only_walkable {
                remaining = keep_heading(remaining, delta, &walk_planes);
                vel = keep_heading(vel, vel_in, &walk_planes);
            }
        }
        Slide {
            feet,
            vel,
            blocked,
            planes: u8::try_from(planes.len).unwrap_or(u8::MAX),
        }
    }

    /// The step-up of module docs step 6, or `None` when it is not taken.
    #[allow(clippy::too_many_arguments)]
    fn step_up<Q: CollisionQuery + ?Sized>(
        &mut self,
        q: &Q,
        feet: [f32; 3],
        delta: [f32; 3],
        vel: [f32; 3],
        height: f32,
        plain: &Slide,
        cosw: f32,
    ) -> Option<Slide> {
        let horizontal_delta = [delta[0], delta[1], 0.0];
        let hl = len3(horizontal_delta);
        if hl.is_nan() || hl <= 1e-6 {
            return None;
        }
        let dir = scale3(horizontal_delta, 1.0 / hl);
        let reach = hl.max(STEP_OVER_FRACTION * self.config.shape.radius());
        let cap = self.config.shape.capsule_at(feet);
        let rise = q
            .sweep_capsule(&cap, [0.0, 0.0, height], &self.config.filter)
            .map_or(height, |h| height * h.fraction);
        if rise.is_nan() || rise <= 1e-3 {
            return None;
        }
        // The raised move first covers `reach`; when its landing is not walkable (the
        // capsule's foot rests on a chamfered step edge) it is retried once with the full
        // shape radius, so the foot clears the edge (§12.3 OPEN (o)).
        let radius = self.config.shape.radius();
        let full = match self.raised_move(q, feet, rise, dir, reach, vel, cosw) {
            Some(s) => s,
            None if reach < radius => self.raised_move(q, feet, rise, dir, radius, vel, cosw)?,
            None => return None,
        };
        let climb = full.feet[2] - feet[2];
        if climb.is_nan() || climb <= STEP_UP_MIN_RISE {
            return None;
        }
        let progress = dot3(sub3(full.feet, feet), dir);
        let plain_progress = dot3(sub3(plain.feet, feet), dir);
        if progress.is_nan() || progress <= plain_progress + 1e-3 {
            return None;
        }
        // A raised move longer than the move itself is only needed to get a capsule resting
        // against a riser onto the step: when the move's own length also lands walkable and
        // climbs in proportion (a slope beside a wall), take that instead.
        if reach > hl * (1.0 + 1e-4)
            && let Some(short) = self.raised_move(q, feet, rise, dir, hl, vel, cosw)
            && short.feet[2] - feet[2] >= climb * (hl / reach) - 1e-3
        {
            return Some(short);
        }
        Some(full)
    }

    /// The raised move of a step-up: from `feet` raised by `rise`, `reach` along `dir`
    /// (every contact a vertical wall), then down by up to `rise` + [`SNAP_DISTANCE`]; the
    /// landing when it is on walkable support (§11.3).
    #[allow(clippy::too_many_arguments)]
    fn raised_move<Q: CollisionQuery + ?Sized>(
        &mut self,
        q: &Q,
        feet: [f32; 3],
        rise: f32,
        dir: [f32; 3],
        reach: f32,
        vel: [f32; 3],
        cosw: f32,
    ) -> Option<Slide> {
        let filter = self.config.filter;
        let raised = [feet[0], feet[1], feet[2] + rise];
        let moved = self.slide(q, raised, scale3(dir, reach), vel, Mode::Raised, cosw);
        let down = rise + SNAP_DISTANCE;
        let cap = self.config.shape.capsule_at(moved.feet);
        let hit = q.sweep_capsule(&cap, [0.0, 0.0, -down], &filter)?;
        let class = classify(
            hit.normal,
            hit.face_normal,
            always_uncontrolled(q, hit.set, hit.body),
            cosw,
        );
        if class != ContactClass::Walkable {
            return None;
        }
        let landed = [
            moved.feet[0],
            moved.feet[1],
            moved.feet[2] - down * hit.fraction,
        ];
        self.scratch.clear();
        let probe = self.config.shape.support_probe(landed);
        q.contacts(&probe, SUPPORT_MARGIN, &filter, &mut self.scratch);
        if best_support(q, &self.scratch, cosw) != Support::Walkable {
            return None;
        }
        Some(Slide {
            feet: landed,
            vel: [moved.vel[0], moved.vel[1], 0.0],
            blocked: false,
            planes: moved.planes,
        })
    }
}

/// Contact planes of one slide (normals merged by §11.3's tolerance).
#[derive(Debug, Default)]
struct Planes {
    n: [[f32; 3]; MAX_PLANES],
    len: usize,
}

impl Planes {
    /// Adds a contact plane. A normal that merges with a plane already listed (§11.3's
    /// tolerance) **replaces** it: the latest contact is the one the remaining motion runs
    /// into. Keeping the older normal instead stopped a mover dead at a shallow concave
    /// crease between two walkable triangles whose normals merge, because the motion slid
    /// along the first one still ran into the second (`collision.md` §12.3 OPEN (u),
    /// Tessera reading).
    fn add(&mut self, n: [f32; 3]) {
        if let Some(m) = self.n[..self.len]
            .iter_mut()
            .find(|m| normals_merge(**m, n))
        {
            *m = n;
            return;
        }
        if self.len < MAX_PLANES {
            self.n[self.len] = n;
            self.len += 1;
        }
    }

    /// `v` with its motion into the planes removed: slid along one plane if that keeps it
    /// out of all others, else along the crease of the last two, else stopped.
    fn clip(&self, v: [f32; 3]) -> [f32; 3] {
        let planes = &self.n[..self.len];
        if planes.is_empty() {
            return v;
        }
        let tol = -1e-5 * len3(v);
        let ok = |c: [f32; 3]| planes.iter().all(|p| dot3(c, *p) >= tol);
        for p in planes.iter().rev() {
            let c = clip1(v, *p);
            if ok(c) {
                return c;
            }
        }
        if let [.., a, b] = planes
            && let Some(dir) = normalize3(cross3(*a, *b))
        {
            let c = scale3(dir, dot3(v, dir));
            if ok(c) {
                return c;
            }
        }
        [0.0; 3]
    }
}

/// The shortest push p with p · nᵢ ≥ needᵢ for every (nᵢ, needᵢ) (unit normals), found
/// with an active set of at most three planes solved exactly; `None` when nothing needs a
/// push or the planes admit no solution the set finds. No heap allocation.
fn solve_push(constraints: &[([f64; 3], f64)]) -> Option<[f64; 3]> {
    let dot = |a: [f64; 3], b: [f64; 3]| a[0] * b[0] + a[1] * b[1] + a[2] * b[2];
    let mut p = [0.0_f64; 3];
    let mut active = Active::default();
    let mut any = false;
    for _ in 0..16 {
        let mut worst: Option<(usize, f64)> = None;
        for (i, (n, need)) in constraints.iter().enumerate() {
            let v = need - dot(p, *n);
            if v > 1e-9 && worst.is_none_or(|(_, w)| v > w) {
                worst = Some((i, v));
            }
        }
        let Some((i, _)) = worst else { break };
        if active.contains(i) {
            break;
        }
        active.push(i);
        if active.len > 3 {
            active.remove(0);
        }
        // Solve on the active planes, dropping planes whose multiplier is negative.
        loop {
            match min_norm_push(constraints, active.as_slice()) {
                Some((q, lambda)) => {
                    if let Some(k) = lambda[..active.len].iter().position(|l| *l < 0.0)
                        && active.len > 1
                    {
                        active.remove(k);
                        continue;
                    }
                    p = q;
                    break;
                }
                None => {
                    active = Active::default();
                    active.push(i);
                    let (n, need) = constraints[i];
                    p = [n[0] * need, n[1] * need, n[2] * need];
                    break;
                }
            }
        }
        any = true;
    }
    (any && p.iter().all(|c| c.is_finite())).then_some(p)
}

/// The active planes of [`solve_push`] (at most four while one is added).
#[derive(Debug, Default, Clone, Copy)]
struct Active {
    items: [usize; 4],
    len: usize,
}

impl Active {
    fn as_slice(&self) -> &[usize] {
        &self.items[..self.len]
    }

    fn contains(&self, i: usize) -> bool {
        self.as_slice().contains(&i)
    }

    fn push(&mut self, i: usize) {
        if let Some(slot) = self.items.get_mut(self.len) {
            *slot = i;
            self.len += 1;
        }
    }

    fn remove(&mut self, k: usize) {
        if k < self.len {
            self.items.copy_within(k + 1..self.len, k);
            self.len -= 1;
        }
    }
}

/// The shortest p meeting p · nᵢ = needᵢ on the planes `active` (one to three), and the
/// multipliers λ with p = Σ λᵢ nᵢ (the first `active.len()` entries); `None` when the planes
/// are (nearly) dependent.
fn min_norm_push(
    constraints: &[([f64; 3], f64)],
    active: &[usize],
) -> Option<([f64; 3], [f64; 3])> {
    let dot = |a: [f64; 3], b: [f64; 3]| a[0] * b[0] + a[1] * b[1] + a[2] * b[2];
    let mut planes = [([0.0_f64; 3], 0.0_f64); 3];
    if active.is_empty() || active.len() > 3 {
        return None;
    }
    for (slot, k) in planes.iter_mut().zip(active) {
        *slot = *constraints.get(*k)?;
    }
    let n = active.len();
    let g = |i: usize, j: usize| dot(planes[i].0, planes[j].0);
    let d = [planes[0].1, planes[1].1, planes[2].1];
    let lambda: [f64; 3] = match n {
        1 => [d[0] / g(0, 0), 0.0, 0.0],
        2 => {
            let (a, b, c) = (g(0, 0), g(0, 1), g(1, 1));
            let det = a * c - b * b;
            if det.abs() < 1e-6 {
                return None;
            }
            [
                (d[0] * c - d[1] * b) / det,
                (a * d[1] - b * d[0]) / det,
                0.0,
            ]
        }
        _ => {
            let m = [
                [g(0, 0), g(0, 1), g(0, 2)],
                [g(1, 0), g(1, 1), g(1, 2)],
                [g(2, 0), g(2, 1), g(2, 2)],
            ];
            let det3 = |m: [[f64; 3]; 3]| {
                m[0][0] * (m[1][1] * m[2][2] - m[1][2] * m[2][1])
                    - m[0][1] * (m[1][0] * m[2][2] - m[1][2] * m[2][0])
                    + m[0][2] * (m[1][0] * m[2][1] - m[1][1] * m[2][0])
            };
            let det = det3(m);
            if det.abs() < 1e-9 {
                return None;
            }
            std::array::from_fn(|col| {
                let mut mc = m;
                for (row, r) in mc.iter_mut().enumerate() {
                    r[col] = d[row];
                }
                det3(mc) / det
            })
        }
    };
    let mut p = [0.0; 3];
    for (l, (nrm, _)) in lambda.iter().zip(&planes).take(n) {
        for a in 0..3 {
            p[a] += l * nrm[a];
        }
    }
    p.iter().all(|c| c.is_finite()).then_some((p, lambda))
}

/// Scales `v` down so its length is at most `limit` (≥ 0), computed in f64 so that huge
/// components never overflow to an infinite speed.
fn limit_speed(v: &mut [f32; 3], limit: f32) {
    let limit = if limit.is_finite() {
        f64::from(limit.max(0.0))
    } else {
        0.0
    };
    let speed = v.iter().map(|c| f64::from(*c).powi(2)).sum::<f64>().sqrt();
    if speed > limit && speed > 0.0 {
        let k = limit / speed;
        for c in v.iter_mut() {
            #[allow(clippy::cast_possible_truncation)]
            let scaled = (f64::from(*c) * k) as f32;
            *c = scaled;
        }
    }
    if !v.iter().all(|c| c.is_finite()) {
        *v = [0.0; 3];
    }
}

/// tan of the walkable limit whose cosine is `cosw` (1 for players' 45°).
fn walkable_tan(cosw: f32) -> f32 {
    if cosw > 0.0 && cosw <= 1.0 {
        (1.0 - cosw * cosw).max(0.0).sqrt() / cosw
    } else {
        0.0
    }
}

/// `v` with its component into the plane of `n` removed.
fn clip1(v: [f32; 3], n: [f32; 3]) -> [f32; 3] {
    let d = dot3(v, n);
    if d < 0.0 { sub3(v, scale3(n, d)) } else { v }
}

/// `c` (a motion clipped against walkable ground planes `planes`) turned back to the
/// horizontal direction of `orig`, keeping its horizontal length, and lifted onto the
/// highest of the planes: a walker on walkable ground keeps its heading instead of drifting
/// downhill on a side slope (`collision.md` §12.3 OPEN (v), Tessera reading).
fn keep_heading(c: [f32; 3], orig: [f32; 3], planes: &Planes) -> [f32; 3] {
    let Some(dir) = normalize3([orig[0], orig[1], 0.0]) else {
        return c;
    };
    let len = (c[0] * c[0] + c[1] * c[1]).sqrt();
    if !(len > 1e-9) || c[0] * dir[0] + c[1] * dir[1] <= 0.0 {
        return c;
    }
    let h = [dir[0] * len, dir[1] * len];
    let mut z = f32::NEG_INFINITY;
    for n in &planes.n[..planes.len] {
        if n[2] <= 1e-3 {
            return c;
        }
        z = z.max(-(n[0] * h[0] + n[1] * h[1]) / n[2]);
    }
    if !z.is_finite() {
        return c;
    }
    [h[0], h[1], z]
}

/// The horizontal part of `n`, normalised; `n` itself when it is (nearly) vertical.
fn horizontal(n: [f32; 3]) -> [f32; 3] {
    normalize3([n[0], n[1], 0.0])
        .filter(|_| n[0] * n[0] + n[1] * n[1] > 1e-6)
        .unwrap_or(n)
}

/// `from` moved toward `to` by at most `step` (≥ 0), never past it.
fn approach(from: f32, to: f32, step: f32) -> f32 {
    let step = if step.is_finite() { step.max(0.0) } else { 0.0 };
    if from < to {
        (from + step).min(to)
    } else {
        (from - step).max(to)
    }
}

/// `d` to the power of [`STEP_SECONDS`] (1/60) for a damper factor 0 ≤ d ≤ 1, by
/// bisection on y⁶⁰ = d in f64 (no library power function, so the same on every platform).
fn step_factor(d: f32) -> f32 {
    if d.is_nan() || d <= 0.0 {
        return 0.0;
    }
    if d >= 1.0 {
        return 1.0;
    }
    let x = f64::from(d);
    let pow60 = |y: f64| {
        let y2 = y * y;
        let y4 = y2 * y2;
        let y8 = y4 * y4;
        let y16 = y8 * y8;
        let y32 = y16 * y16;
        y32 * y16 * y8 * y4
    };
    let (mut lo, mut hi) = (0.0_f64, 1.0_f64);
    for _ in 0..64 {
        let mid = 0.5 * (lo + hi);
        if pow60(mid) < x {
            lo = mid;
        } else {
            hi = mid;
        }
    }
    #[allow(clippy::cast_possible_truncation)]
    let r = hi as f32;
    r
}

fn add3(a: [f32; 3], b: [f32; 3]) -> [f32; 3] {
    [a[0] + b[0], a[1] + b[1], a[2] + b[2]]
}

fn sub3(a: [f32; 3], b: [f32; 3]) -> [f32; 3] {
    [a[0] - b[0], a[1] - b[1], a[2] - b[2]]
}

fn scale3(a: [f32; 3], s: f32) -> [f32; 3] {
    [a[0] * s, a[1] * s, a[2] * s]
}

fn dot3(a: [f32; 3], b: [f32; 3]) -> f32 {
    a[0] * b[0] + a[1] * b[1] + a[2] * b[2]
}

fn cross3(a: [f32; 3], b: [f32; 3]) -> [f32; 3] {
    [
        a[1] * b[2] - a[2] * b[1],
        a[2] * b[0] - a[0] * b[2],
        a[0] * b[1] - a[1] * b[0],
    ]
}

fn len3(a: [f32; 3]) -> f32 {
    dot3(a, a).sqrt()
}

fn normalize3(a: [f32; 3]) -> Option<[f32; 3]> {
    let l = len3(a);
    (l.is_finite() && l > 1e-12).then(|| scale3(a, 1.0 / l))
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn step_factor_is_the_sixtieth_root() {
        for d in [0.2_f32, 0.4, 0.5, 0.99] {
            let y = step_factor(d);
            let back = f64::from(y).powi(60);
            assert!((back - f64::from(d)).abs() < 1e-5, "{d}: {y} → {back}");
        }
        assert_eq!(step_factor(1.0), 1.0);
        assert_eq!(step_factor(0.0), 0.0);
        assert_eq!(step_factor(f32::NAN), 0.0);
    }

    #[test]
    fn approach_never_overshoots() {
        assert_eq!(approach(0.0, 1.0, 0.3), 0.3);
        assert_eq!(approach(0.9, 1.0, 0.3), 1.0);
        assert_eq!(approach(1.0, -1.0, 0.5), 0.5);
        assert_eq!(approach(0.0, 1.0, f32::NAN), 0.0);
    }

    #[test]
    fn input_is_sanitised() {
        let i = MoveInput {
            forward: f32::NAN,
            strafe: 7.0,
            heading: Some(f32::INFINITY),
            jump: true,
        }
        .sanitized();
        assert_eq!((i.forward, i.strafe, i.heading), (0.0, 1.0, None));
        let d = MoveInput {
            forward: 1.0,
            strafe: 1.0,
            ..MoveInput::default()
        }
        .sanitized();
        assert!((d.forward * d.forward + d.strafe * d.strafe - 1.0).abs() < 1e-6);
    }

    #[test]
    fn heading_wraps() {
        use std::f32::consts::PI;
        assert!((wrap_heading(3.0 * PI) - PI).abs() < 1e-5);
        assert!((wrap_heading(-0.5) + 0.5).abs() < 1e-6);
        assert_eq!(wrap_heading(f32::NAN), 0.0);
    }

    #[test]
    fn planes_slide_and_stop() {
        let mut p = Planes::default();
        p.add([1.0, 0.0, 0.0]);
        assert_eq!(p.clip([-1.0, 1.0, 0.0]), [0.0, 1.0, 0.0]);
        p.add([1.0, 0.01, 0.0]); // merged (§11.3 tolerance)
        assert_eq!(p.len, 1);
        p.add([0.0, -1.0, 0.0]);
        // Into both walls of a corner: slides along the crease (vertical), here nothing.
        let c = p.clip([-1.0, 1.0, 0.0]);
        assert!(len3(c) < 1e-6, "{c:?}");
        let up = p.clip([-1.0, 1.0, 1.0]);
        assert!((up[2] - 1.0).abs() < 1e-6 && up[0].abs() < 1e-6 && up[1].abs() < 1e-6);
    }
}
