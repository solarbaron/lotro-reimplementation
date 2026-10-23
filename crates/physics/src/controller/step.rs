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
//! 3. **Velocity** (§12.3 steps 1–3). The target is the original's (§11.6 item 1,
//!    [`target_velocity`]): along the input pair, with the forward/backward and strafe
//!    speeds blended by the pair's components. Per local axis (forward, side) the velocity
//!    moves toward the target's component at the control acceleration, but never faster
//!    than the rate that reaches the axis' top speed from rest in the ramp time, and never
//!    past the target. On walkable support (states 1, 5) the vertical speed is 0, otherwise
//!    the layer's gravity is added. Each local component is multiplied by its damper to the
//!    power of the step length. Then the stored impulse is added (§11.6 item 3, §11.10):
//!    the jump command adds the vertical impulse of the jump script's node for the state,
//!    (`AirTime` + 0.2) × `ExpectedGravity` (6.0 m/s on land, 3.0 m/s in water in this
//!    install, [`ControllerConfig::jump`]), and every impulse lasts one step. The command is
//!    refused while `Movement_IsJumping` is true and in states 4 and 6 (§11.10 gates);
//!    `Movement_IsJumping` is set by the take-off step and cleared at the end of the first
//!    step that ends at least [`JUMP_HOLD_SECONDS`] after it outside state 4 (§11.10
//!    Decision item 1), and the float-line rule of step 4 is skipped while it is true. Without a
//!    jump table the earlier rule applies: on walkable support, a push of the jump
//!    acceleration × the BasePlayer ramp time ([`PlayerMovementRecord::jump_ramp_time`];
//!    5.2 m/s). The speed is then limited to the terminal velocity (and [`MAX_SPEED`]).
//!    The vertical displacement uses the mean of the vertical speeds before and after
//!    gravity (an impulse counting from the step's start), so a jump's apex above its
//!    first step is v²/2g at any step.
//! 4. **Buoyancy** (in water without walkable support, states 2 and 3, except in the step
//!    a jump starts; Decision): the
//!    float line is the water surface minus the float height. Below it (or on it) the
//!    layer's gravity applies (−20 for swimmers lifts the mover), the water takes up any
//!    downward speed (§12.4 (p)) and the feet stop at the line, the upward speed then
//!    zeroed. Above it the gravity's magnitude pulls the mover down (at least
//!    [`MIN_SINK_ACCELERATION`]) and the feet stop at the line, the speed then zeroed, so a
//!    swimmer never overshoots it. The speed is limited again afterwards.
//! 5. **Sweep and slide** (§12.3 step 4): up to [`MAX_SLIDE_ITERATIONS`] sweeps; each hit
//!    keeps the motion up to the contact and removes the motion and velocity into every
//!    contact plane so far (normals merged by §11.3's 0.035 tolerance; two planes slide
//!    along their crease, more stop). Contacts are classified by §11.3 (the face normal
//!    where it is steeper than walkable and less upward than the contact normal, §12.4
//!    (x)). Where that face normal lowered the class of a hit on a triangle (a hit at an
//!    edge or corner, such as a step's riser reported where its top face meets it), the
//!    hit takes the best class (walkable, then steep, then wall, then overhang) among its
//!    own and those of the triangle contacts within four skins (`EDGE_TIE_DISTANCE`) of
//!    its point, found by a contact query at the stop position with the margin of the
//!    hit's gap plus one [`SKIN`]; the hit's plane stays its contact normal, and the class
//!    found decides which of the rules below applies and whether the hit blocks for the
//!    step-up of step 6 (§12.4 (z), `Controller::hit_class`). The ground snap of step 7
//!    classifies its hit the same way; the raised move of a step-up does not (its
//!    contacts are vertical walls):
//!    - on walkable support (states 1, 5), a walkable contact keeps its normal, so walkable
//!      slopes are climbed by the slide itself; a hit whose contact normal does not oppose
//!      the motion while its triangle's face does slides along the face (a crease, §12.4
//!      (n)); a **steep** contact (45°–85° for players) is ridden onto: the motion
//!      keeps its normal, so the capsule can come to rest on the steep face and step 7
//!      classifies that face underfoot (state 6), while the velocity meets it as a vertical
//!      wall, so riding onto it gives no upward speed (§11.3 mechanism, §12.3 step 6,
//!      §12.4 (q)–(s)); every other contact (wall, overhang, ignored surface) counts as a
//!      vertical wall (its normal's horizontal part), so walls and risers never lift the
//!      mover;
//!    - on steep support (states 3, 6), contacts keep their normals and a steep contact
//!      also adds the vertical plane of its normal's horizontal part, so the mover slides
//!      down but never climbs the slope (§11.3 mechanism);
//!    - in the air or swimming, contacts keep their normals.
//!
//!    Keeping the heading (§12.4 (v)) can lengthen the velocity; after the slide and the
//!    step-up it is limited again to the terminal velocity and [`MAX_SPEED`].
//! 6. **Step-up** (§12.3 step 5): when a grounded horizontal move met a contact that is not
//!    walkable (a wall or a steep contact), the move is retried from the position raised
//!    by the step-up height (less where a ceiling stops the rise), covering at least
//!    [`STEP_OVER_FRACTION`] of the capsule radius, then swept down by up to the rise plus
//!    [`SNAP_DISTANCE`]; when the support probe where that sweep lands is not walkable it
//!    is retried once with the full radius (§12.4 (o); a sweep that finds nothing is not
//!    retried). The landing is judged by the support probe alone, never by the single
//!    contact the sweep stops on, so a landing on an edge shared by several triangles does
//!    not depend on their order (§12.4 (k) as revised 2026-10-23, CO63). It is taken when
//!    it lands on walkable support (§11.3), climbs more than
//!    [`STEP_UP_MIN_RISE`] and gets further along the move than the blocked path. When the
//!    raised move covered more than the move itself, the same climb with only the move's
//!    length is preferred if it also lands walkable, passes the same test and climbs in
//!    proportion to the raised move taken (over the length that one covered), so a
//!    step-up never moves the mover faster than its input except to get a capsule resting
//!    against a riser onto the step.
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
//! **Mounted riders** (`mounts.md` §5.1, §5.2): the player controller's speed factor *m*
//! (`collision.md` §11.6 item 1) is [`Controller::set_speed_multiplier`] (1 by default; the
//! normal-mount Decision of `mounts.md` §5.1 sets it), and with a war-steed drive
//! ([`Controller::set_war_steed`]) step 3's horizontal target and acceleration are the
//! war-steed controller's (§5.3–§5.7, [`super::warsteed::WarSteedDrive`]): its mean control
//! velocity moves the mover, undamped, and its stored control velocity carries over.
//!
//! Every value is checked: a step whose result is not finite or moves the mover more than
//! [`MAX_STEP_DISPLACEMENT`] is undone and the velocity zeroed ([`StepOutcome::Reverted`]).
//! A step's work is bounded by its query count and the water query. Capsule sweeps: at
//! most 4 in the slide, 1 for the step-up's rise, 15 for up to three raised moves (the
//! first reach, the full-radius retry and the move's own length, each up to 4 slide sweeps
//! and 1 down sweep) and 1 for the ground snap, 21 in all. Contact queries: at most 4
//! push-out rounds, 1 support probe per raised move (3), 1 edge-tie query per slide sweep
//! (4) and for the ground snap's hit (§12.4 (z)) and the support probe, 13 in all.
//! Each query visits at most [`crate::geometry::MAX_QUERY_ITEMS`] items per set (a sweep
//! in two passes). The step keeps no buffer larger than [`SCRATCH_KEEP`] entries.
//! The arithmetic is plain IEEE without fused operations; the damper power is found by
//! bisection, so only the heading's sine and cosine come from the platform's library.

use std::sync::Arc;

use crate::geometry::{
    CellHit, CollisionQuery, CollisionView, Contact, FeatureKind, LiquidFlags, MAX_COORDINATE,
    QueryFilter, SKIN, SweepHit,
};

use super::contact::{
    ContactClass, always_uncontrolled, best_support, classify, normals_merge, walkable_cos,
};
use super::impulse::{Impulse, JumpTable, StoredImpulse};
use super::params::{
    LayerConditions, MAX_SPEED, MovementParams, MovementPids, PlayerMovementRecord,
};
use super::shape::CharacterShape;
use super::state::{MovementState, Support, WaterDepth};
use super::warsteed::WarSteedDrive;
use super::water::{LiquidWater, NoWater, WaterQuery};

/// Controller steps per second (`collision.md` §12.3; decision 0013).
pub const STEPS_PER_SECOND: u32 = 60;
/// Length of one controller step, s (`collision.md` §12.3; decision 0013).
pub const STEP_SECONDS: f32 = 1.0 / 60.0;
/// How long `Movement_IsJumping` stays true at least after the take-off step, seconds
/// (`collision.md` §11.10 Decision item 1; the length of the Timer that `Loc_Jumping` passes
/// before its clear, `script-runtime.md` §30.7 item 3).
pub const JUMP_HOLD_SECONDS: f32 = 0.2;
/// Tolerance of the [`JUMP_HOLD_SECONDS`] comparison against a sum of
/// [`STEP_SECONDS`] (12 steps make 0.2 s up to rounding).
const JUMP_TIME_EPSILON: f32 = 1e-4;
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
/// Slack of the step-up's "climbs in proportion" test, m (Tessera Decision, `collision.md`
/// §12.4 (k)): the short raised move counts as climbing in
/// proportion when it climbs at least the full move's climb × (move / reach) less this.
pub const PROPORTION_SLACK: f32 = 1e-3;
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
/// How far from a slide's or the ground snap's sweep hit point, m, a triangle contact's
/// point may lie and still take part in the hit's class: four [`SKIN`]s
/// (`collision.md` §12.4 (z) item 2; [`Controller::hit_class`]).
const EDGE_TIE_DISTANCE: f32 = 4.0 * SKIN;
/// Most contact planes one push-out considers (the deepest).
const MAX_PUSH_PLANES: usize = 32;

/// One step's input: the controller's input pair of `collision.md` §11.6 item 1 (for the
/// player, the decoded pair of §11.8 item 5, [`super::PlayerInput`]). Axes are sanitised:
/// non-finite → 0, then clamped to −1..=1 each (the pair is not normalised: §11.6 item 1
/// gives its length).
#[derive(Debug, Clone, Copy, PartialEq, Default)]
pub struct MoveInput {
    /// *y*: forward (+) / backward (−).
    pub forward: f32,
    /// *x*: strafe right (+) / left (−) (§11.8 item 5: strafing right gives *x* > 0).
    pub strafe: f32,
    /// The new heading (yaw, radians, counter-clockwise from +X), or `None` to keep it.
    /// A non-finite value keeps it too.
    pub heading: Option<f32>,
    /// The jump command (the caller latches the press). With a [`ControllerConfig::jump`]
    /// table the step adds the jump impulse of the table's node for the mover's state
    /// (§11.10); without one, Tessera's earlier rule applies (a push of jump acceleration ×
    /// the BasePlayer ramp time, on walkable support only, `collision.md` §12.4 (c), (i)).
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
        Self {
            forward: axis(self.forward),
            strafe: axis(self.strafe),
            heading: self.heading.filter(|h| h.is_finite()),
            jump: self.jump,
        }
    }
}

/// The target velocity of input pair (`strafe` *x*, `forward` *y*), each in −1 … 1, with
/// speed multiplier `multiplier` (`collision.md` §11.6 item 1, `skeleton-anim.md` §7.4;
/// BIN): zero when both |*x*| and |*y*| are at most [`super::INPUT_EPSILON`]; otherwise
/// along (*x*, *y*) normalised with length *m* · max(|*x*|, |*y*|) · *S*, *S* = (*S*side ·
/// *w*x + *S*fwd · *w*y) / (*w*x + *w*y), *w*x = |*x*|, *w*y = |*y*| doubled when both are at
/// most 0.5. *S*fwd is the forward speed for *y* > 0, else the backward speed; *S*side the
/// strafe-left speed for *x* > 0, else the strafe-right speed (the client's pairing,
/// §11.6 OPEN; equal in every observed layer). Returns (forward, side) in m/s, side + to the
/// right. Inputs that are not finite count as 0; a non-finite or negative multiplier as 1.
#[must_use]
pub fn target_velocity(
    strafe: f32,
    forward: f32,
    params: &MovementParams,
    multiplier: f32,
) -> (f32, f32) {
    let axis = |v: f32| {
        if v.is_finite() {
            v.clamp(-1.0, 1.0)
        } else {
            0.0
        }
    };
    let (x, y) = (axis(strafe), axis(forward));
    let eps = super::input::INPUT_EPSILON;
    if x.abs() <= eps && y.abs() <= eps {
        return (0.0, 0.0);
    }
    let m = if multiplier.is_finite() && multiplier >= 0.0 {
        multiplier
    } else {
        1.0
    };
    let wx = x.abs();
    let wy = if x.abs() <= 0.5 && y.abs() <= 0.5 {
        2.0 * y.abs()
    } else {
        y.abs()
    };
    let s_fwd = if y > 0.0 {
        params.forward_speed
    } else {
        params.backward_speed
    };
    let s_side = if x > 0.0 {
        params.strafe_left_speed
    } else {
        params.strafe_right_speed
    };
    let speed = (s_side * wx + s_fwd * wy) / (wx + wy);
    let len = m * wx.max(y.abs()) * speed;
    let norm = (x * x + y * y).sqrt();
    if !(len.is_finite() && norm > 0.0) {
        return (0.0, 0.0);
    }
    let (tf, ts) = (len * y / norm, len * x / norm);
    let lim = MAX_SPEED;
    (tf.clamp(-lim, lim), ts.clamp(-lim, lim))
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
    /// A jump started (the jump command, or any upward impulse, §12.4 (f)).
    pub jumped: bool,
    /// The jump command took off in this step (`Movement_IsJumping` becomes true,
    /// §11.10 Decision item 1).
    pub jump_started: bool,
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
            jump_started: false,
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
    /// The jump nodes of the mover's jump script by state (§11.10), read from the data by
    /// the caller; `None`: Tessera's earlier jump rule (§12.4 (c), (i); [`MoveInput::jump`]).
    pub jump: Option<JumpTable>,
}

impl Default for ControllerConfig {
    fn default() -> Self {
        Self {
            shape: CharacterShape::PLAYABLE,
            filter: QueryFilter::player(),
            is_player: true,
            jump: None,
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
    impulse: StoredImpulse,
    /// `Movement_IsJumping` (`collision.md` §11.10 Decision item 1): the time since the
    /// take-off step ended, seconds, while the property is true.
    jump_time: Option<f32>,
    scratch: Vec<Contact>,
    push_planes: Vec<([f64; 3], f64)>,
    /// The speed factor *m* of `collision.md` §11.6 item 1 (1 by default).
    speed_multiplier: f32,
    /// The war-steed controller, while a war-steed is ridden (`mounts.md` §5.2).
    war_steed: Option<WarSteedDrive>,
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
            impulse: StoredImpulse::default(),
            jump_time: None,
            scratch: Vec::new(),
            push_planes: Vec::new(),
            speed_multiplier: 1.0,
            war_steed: None,
        }
    }

    /// The speed factor *m* of the player controller (`collision.md` §11.6 item 1): the
    /// target's length and the acceleration limit of the ramp scale with it. A value that is
    /// not finite or is negative gives 1. A mounted rider on a normal mount takes the
    /// Decision of `mounts.md` §5.1.
    pub fn set_speed_multiplier(&mut self, m: f32) {
        self.speed_multiplier = if m.is_finite() && m >= 0.0 { m } else { 1.0 };
    }

    /// The speed factor in use.
    #[must_use]
    pub fn speed_multiplier(&self) -> f32 {
        self.speed_multiplier
    }

    /// Sets (or, with `None`, removes) the war-steed controller (`mounts.md` §5.2): while
    /// set, the step's horizontal control velocity is the drive's.
    pub fn set_war_steed(&mut self, drive: Option<WarSteedDrive>) {
        self.war_steed = drive;
    }

    /// The war-steed controller, while set.
    #[must_use]
    pub fn war_steed(&self) -> Option<&WarSteedDrive> {
        self.war_steed.as_ref()
    }

    /// The war-steed controller, while set (the caller updates its gait and gear).
    pub fn war_steed_mut(&mut self) -> Option<&mut WarSteedDrive> {
        self.war_steed.as_mut()
    }

    /// Replaces the jump table of §11.10 (`None`: no jump). A mounted rider uses the
    /// mounted branch's table while the channelling loop runs (`mounts.md` §14.12) and the
    /// on-foot one again afterwards.
    pub fn set_jump_table(&mut self, table: Option<JumpTable>) {
        self.config.jump = table;
    }

    /// `Movement_IsJumping` (`collision.md` §11.10 Decision item 1): true from the step in
    /// which the jump command took off until the end of the first step that ends at least
    /// [`JUMP_HOLD_SECONDS`] after it with `Entity_Element` not 4. While it is true the jump
    /// command starts nothing and the float-line rule is skipped.
    #[must_use]
    pub fn is_jumping(&self) -> bool {
        self.jump_time.is_some()
    }

    /// Sets `Movement_IsJumping` for a jump whose script starts no impulse (`mounts.md`
    /// §14.6 "Jumping while mounted": a war-steed's press at gaits 1 and 3 runs the `Jump`
    /// scripts without a take-off). Call it after the step of the press, in place of that
    /// step's take-off: the flag then clears by the rule of `collision.md` §11.10 Decision
    /// item 1, as after a take-off. Does nothing while the flag is already true.
    pub fn start_jump_without_lift(&mut self) {
        if self.jump_time.is_none() {
            self.jump_time = Some(0.0);
        }
    }

    /// Adds a PhysicsImpulse node's effect to the stored impulse (`collision.md` §11.10
    /// items 1–2); the next step adds it to the velocity once and clears it (item 3).
    /// Returns whether something was added (a jump adds nothing while a vertical impulse
    /// is stored).
    pub fn add_impulse(&mut self, impulse: Impulse) -> bool {
        self.impulse.add(impulse)
    }

    /// The impulse the next step adds.
    #[must_use]
    pub fn stored_impulse(&self) -> &StoredImpulse {
        &self.impulse
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
        let drive_before = self.war_steed;
        let mut report = self.step_inner(input, env);
        let ok = self.state.is_finite() && {
            let p = self.state.position;
            let d2: f64 = (0..3).map(|i| (p[i] - before.position[i]).powi(2)).sum();
            d2 <= f64::from(MAX_STEP_DISPLACEMENT).powi(2)
        };
        if !ok {
            self.revert(before, drive_before);
            report = StepReport::new(StepOutcome::Reverted);
        }
        self.state.steps = before.steps.wrapping_add(1);
        // `Movement_IsJumping` (§11.10 Decision item 1): cleared at the end of the first
        // step that ends at least 0.2 s after the take-off step, out of the air.
        if report.jump_started {
            self.jump_time = Some(0.0);
        } else if let Some(t) = self.jump_time {
            let t = t + STEP_SECONDS;
            self.jump_time = (t + JUMP_TIME_EPSILON < JUMP_HOLD_SECONDS
                || self.state.element == MovementState::EarthNoSurface)
                .then_some(t);
        }
        // §11.10 item 3: the stored impulse acts in one step only.
        self.impulse.clear();
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

    /// Undoes a step (module docs): the state before it with the velocity zeroed, and the
    /// war-steed drive as it was before the step with its stored control velocity zeroed
    /// too, so the next step does not repeat the undone motion (`mounts.md` §5.4: the
    /// drive's velocity is what moves the mover).
    fn revert(&mut self, before: MoverState, drive_before: Option<WarSteedDrive>) {
        self.state = MoverState {
            velocity: [0.0; 3],
            ..before
        };
        self.war_steed = drive_before.map(|mut d| {
            d.velocity = [0.0; 2];
            d.last = None;
            d
        });
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
        let m = self.speed_multiplier;
        let (tf, ts) = target_velocity(input.strafe, input.forward, &params, m);
        let rate = |axis: usize, top: f32| {
            let a = params.control_acceleration[axis];
            if params.ramp_time > 0.0 {
                a.min(m * top / params.ramp_time)
            } else {
                a
            }
        };
        let war_steed = if let Some(drive) = self.war_steed.as_mut() {
            // `mounts.md` §5.4: the war-steed's mean control velocity (sideways, forward).
            let [side, forward] = drive.update(&params, [input.strafe, input.forward], dt);
            vf = forward;
            vs = side;
            true
        } else {
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
            false
        };
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
        // The jump command (§11.10 gates, §12.3 step 2): refused while `Movement_IsJumping`
        // is true and in states 4 and 6; then the jump script's node for the state, else
        // the earlier rule below.
        let jump = input.jump && self.jump_time.is_none() && element.allows_jump();
        if jump
            && let Some(node) = self.config.jump.and_then(|t| t.get(element))
            && self.impulse.add(Impulse::Jump(node))
        {
            report.jump_started = true;
        }
        let vz_start;
        if walk_support {
            vz = 0.0;
            if jump && self.config.jump.is_none() {
                let j = (params.jump_acceleration * self.record.jump_ramp_time()).min(MAX_SPEED);
                if j > 0.0 {
                    vz = j;
                    report.jumped = true;
                    report.jump_started = true;
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
        // The war-steed's control velocity is not damped (`mounts.md` §5.4).
        if !war_steed {
            vf *= d[0];
            vs *= d[1];
        }
        vz *= d[2];
        // The stored impulse (§11.6 item 3, §11.10): added once, after gravity and the
        // damper, the world part turned into the mover's frame. Its vertical part counts
        // from the start of the step, so the step's mean vertical speed includes it; an
        // upward impulse starts a jump (§12.4 (f)).
        let mut vz_start = vz_start;
        if !self.impulse.is_zero() {
            let w = self.impulse.world;
            let l = self.impulse.local;
            vf += l[0] + w[0] * fwd[0] + w[1] * fwd[1];
            vs += l[1] + w[0] * right[0] + w[1] * right[1];
            let up = self.impulse.vertical();
            vz += up;
            vz_start += up;
            if up > 0.0 {
                report.jumped = true;
            }
        }
        let speed_limit = params.terminal_velocity.min(MAX_SPEED);
        let mut local = [vf, vs, vz];
        limit_speed(&mut local, speed_limit);
        let [vf, vs, mut vz] = local;
        let vz_start = vz_start.max(-speed_limit).min(speed_limit);
        let mut dz = 0.5 * (vz_start + vz) * dt;

        // Buoyancy: the feet stop at the float line from either side (not in a jump's step:
        // §11.6 item 4 holds the feet at the line only while not jumping).
        if let Some(t) = float_line.filter(|_| !report.jumped && self.jump_time.is_none()) {
            if above_line {
                if feet[2] + dz < t {
                    dz = t - feet[2];
                    vz = 0.0;
                }
            } else {
                // Below the line the water takes up any downward speed, so a mover that
                // slides or falls in at speed stops instead of diving metres below the line
                // before the lift turns it (`collision.md` §12.4 (p), Tessera reading of
                // `water.md` §7.4).
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
        // Keeping the heading (§12.4 (v)) lifts the clipped velocity onto a walkable plane
        // with its horizontal length kept, which can make it longer than the limit of
        // §12.3 step 3: limit it again (Tessera Decision, `collision.md` §12.4 (y)).
        let mut vel = result.vel;
        limit_speed(&mut vel, speed_limit);
        report.planes = result.planes;

        // Ground snap.
        let swimming = element == MovementState::WaterNoSurface;
        // Rising in the air (§12.4 (f)); in water the swimming gravity lifts the mover
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
                // Edge ties as for a slide's hit (§12.4 (z)): whether the hit is support
                // that the snap moves onto does not depend on whether the sweep reports a
                // step's riser or its top face.
                let stop = [feet[0], feet[1], feet[2] - reach * h.fraction];
                let class = self.hit_class(q, &h, stop, cosw);
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
            // Every contact closer than the target gap is a plane the push must clear
            // (the deepest first, at most MAX_PUSH_PLANES of them). Only contacts with a
            // finite separation and a unit normal (within 10⁻³) count, for the push and
            // for the test whether anything penetrates; others are hostile values and are
            // ignored (§12.4 (g)).
            constraints.clear();
            let mut penetrating = false;
            constraints.extend(self.scratch.iter().filter_map(|c| {
                if !sound_contact(c) {
                    return None;
                }
                let sep = f64::from(c.separation);
                penetrating |= sep < 0.0;
                (sep < target).then_some((c.normal.map(f64::from), target - sep))
            }));
            if !penetrating {
                break;
            }
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
        &mut self,
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
            // The raised move of a step-up classifies its hits plainly (§12.4 (z) excludes
            // it; every contact there is a vertical wall, (k)); every other slide sweep
            // breaks edge ties by (z).
            let class = if mode == Mode::Raised {
                classify(
                    h.normal,
                    h.face_normal,
                    always_uncontrolled(q, h.set, h.body),
                    cosw,
                )
            } else {
                self.hit_class(q, &h, feet, cosw)
            };
            // The plane is the contact normal, whatever class (z) gave the hit.
            let mut n = h.normal;
            if !n.iter().all(|c| c.is_finite()) || len3(n) < 0.5 {
                // No usable normal: stop the motion.
                n = normalize3(scale3(remaining, -1.0)).unwrap_or([0.0, 0.0, 1.0]);
            }
            // A hit whose contact normal (an edge or corner of the triangle) does not oppose
            // the motion would leave the motion unclipped, and the next sweep would report
            // the same hit at the same place, stopping the mover dead on a slight terrain
            // crease. On walkable support, slide along the triangle's face instead when the
            // face opposes the motion (§12.4 (n)).
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
                // `collision.md` §11.3, §12.3 step 6, §12.4 (q)–(s)); the velocity meets it
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
            // Keeping the heading (§12.4 (v)) applies while every plane of the slide is a
            // walkable contact kept as its own plane, whatever the mover's support. The
            // raised move's planes are vertical walls (§12.4 (k)), never walkable planes.
            if mode != Mode::Raised && class == ContactClass::Walkable {
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

    /// The class of a slide's or the ground snap's sweep hit `h`, the capsule stopped by
    /// that sweep with its feet at `feet` (§11.3 with §12.4 (x) and (z); module docs
    /// step 5).
    ///
    /// A sweep that stops at an edge or corner shared by several triangles (a step's riser
    /// and its top face) reports one of them by the tie order of §12.4 (w). So that the
    /// hit's class does not depend on which one, §12.4 (z) applies whenever the hit is on a
    /// triangle and (x) item 4 has replaced its contact normal by a face normal whose class
    /// is lower than the contact normal's:
    /// 1. a contact query at `feet` with the margin of the hit's gap there plus one
    ///    [`SKIN`];
    /// 2. of its contacts, those with triangles (of any body, the hit's own included) whose
    ///    point lies within `EDGE_TIE_DISTANCE` (four skins) of the hit's point are kept,
    ///    the hit's point being the hit triangle's own contact in that query (same set,
    ///    body, kind and feature), or the sweep's hit point when the query has none (an
    ///    implementer reading of item 2: the sweep's point lies up to a few skins before
    ///    the stop position, which is too far when the motion runs along the edge),
    ///    leaving out objects with `Physics_AlwaysUncontrolledSurface` and the hostile
    ///    contacts of §12.4 (g) ([`sound_contact`]);
    /// 3. each kept contact is classified by §11.3 with (x), its face normal turned by the
    ///    contact-query rule of (x) item 1 (as [`CollisionQuery::contacts`] reports it);
    /// 4. the best class among the hit's own and the kept contacts' is returned, in the
    ///    order walkable, steep, wall, overhang ([`tie_rank`]).
    ///
    /// The caller keeps the hit's contact normal as its plane (item 5); only the class
    /// changes. The raised move of a step-up does not call this (its contacts are vertical
    /// walls, §12.4 (k)). A hit away from any shared edge has no other triangle within four
    /// skins and keeps its class. A hit whose point or gap is not finite keeps its class
    /// (engineering choice: there is no position to query from).
    fn hit_class<Q: CollisionQuery + ?Sized>(
        &mut self,
        q: &Q,
        h: &SweepHit,
        feet: [f32; 3],
        cosw: f32,
    ) -> ContactClass {
        let uncontrolled = always_uncontrolled(q, h.set, h.body);
        let class = classify(h.normal, h.face_normal, uncontrolled, cosw);
        if h.kind != FeatureKind::Triangle
            || tie_rank(class) >= tie_rank(classify(h.normal, None, uncontrolled, cosw))
            || !h.point.iter().all(|c| c.is_finite())
            || !h.gap.is_finite()
        {
            return class;
        }
        // Item 1. The query takes no negative margin; when the capsule penetrates the hit
        // triangle (a negative gap), it is run with 0 and its contacts are then held to
        // the margin below.
        let margin = h.gap + SKIN;
        let cap = self.config.shape.capsule_at(feet);
        self.scratch.clear();
        q.contacts(
            &cap,
            margin.max(0.0),
            &self.config.filter,
            &mut self.scratch,
        );
        // Item 2's reference point: the hit triangle's own contact in this query, at the
        // stop position, falling back to the sweep's hit point. The sweep's point is where
        // the gap reached the hit threshold, up to a few skins before the stop; when the
        // motion runs partly along the edge, the edge point moves between the two by about
        // three skins times the tangent of the angle, and measuring from it left the other
        // triangle's contact outside the radius in one triangle order only (implementer
        // reading, proposed for `collision.md` §12.4 (z) item 2).
        let reference = self
            .scratch
            .iter()
            .find(|c| {
                c.set == h.set && c.body == h.body && c.kind == h.kind && c.feature == h.feature
            })
            .map(|c| c.point)
            .filter(|p| p.iter().all(|v| v.is_finite()))
            .unwrap_or(h.point);
        let mut best = class;
        for c in &self.scratch {
            // Item 2.
            let d = sub3(c.point, reference);
            let d2 = dot3(d, d);
            if c.kind != FeatureKind::Triangle
                || !sound_contact(c)
                || (margin < 0.0 && c.separation > margin)
                || d2.is_nan()
                || d2 > EDGE_TIE_DISTANCE * EDGE_TIE_DISTANCE
                || always_uncontrolled(q, c.set, c.body)
            {
                continue;
            }
            // Items 3 and 4.
            let other = classify(c.normal, c.face_normal, false, cosw);
            if tie_rank(other) > tie_rank(best) {
                best = other;
            }
        }
        self.scratch.clear();
        best
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
        // The raised move first covers `reach`; when it lands but the landing is not
        // walkable (the capsule's foot rests on a chamfered step edge) it is retried once
        // with the full shape radius, so the foot clears the edge (§12.4 (o)). A raised move
        // whose down sweep finds nothing has no landing and is not retried.
        let radius = self.config.shape.radius();
        let (full, covered) = match self.raised_move(q, feet, rise, dir, reach, vel, cosw) {
            Raised::Landed(s) => (s, reach),
            Raised::NotWalkable if reach < radius => {
                match self.raised_move(q, feet, rise, dir, radius, vel, cosw) {
                    Raised::Landed(s) => (s, radius),
                    Raised::NotWalkable | Raised::NoLanding => return None,
                }
            }
            Raised::NotWalkable | Raised::NoLanding => return None,
        };
        let climb = full.feet[2] - feet[2];
        let progress = dot3(sub3(full.feet, feet), dir);
        let plain_progress = dot3(sub3(plain.feet, feet), dir);
        if !step_up_accepted(climb, progress, plain_progress) {
            return None;
        }
        // A raised move longer than the move itself is only needed to get a capsule resting
        // against a riser onto the step: when the move's own length also lands walkable,
        // climbs in proportion to the raised move taken (over the length it covered) and
        // passes the same acceptance test (§12.4 (k)), take that instead.
        if covered > hl * (1.0 + 1e-4)
            && let Raised::Landed(short) = self.raised_move(q, feet, rise, dir, hl, vel, cosw)
            && short_preferred(
                short.feet[2] - feet[2],
                dot3(sub3(short.feet, feet), dir),
                plain_progress,
                climb,
                hl,
                covered,
            )
        {
            return Some(short);
        }
        Some(full)
    }

    /// The raised move of a step-up: from `feet` raised by `rise`, `reach` along `dir`
    /// (every contact a vertical wall), then down by up to `rise` + [`SNAP_DISTANCE`]: the
    /// landing when the support probe there is walkable (§12.4 (k), (s); the contact the
    /// down sweep stops on only places the landing and is not classified, so the result
    /// does not depend on which of several triangles touching at the landing point the
    /// sweep reports, CO63), [`Raised::NotWalkable`] when it lands otherwise, and
    /// [`Raised::NoLanding`] when the down sweep finds nothing.
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
    ) -> Raised {
        let filter = self.config.filter;
        let raised = [feet[0], feet[1], feet[2] + rise];
        let moved = self.slide(q, raised, scale3(dir, reach), vel, Mode::Raised, cosw);
        let down = rise + SNAP_DISTANCE;
        let cap = self.config.shape.capsule_at(moved.feet);
        let Some(hit) = q.sweep_capsule(&cap, [0.0, 0.0, -down], &filter) else {
            return Raised::NoLanding;
        };
        // Only where the descent stops comes from its hit. Whether the landing is walkable
        // is the support probe's alone (§12.4 (k), revised 2026-10-23; §11.5 step 5): the
        // hit is one of possibly several triangles touching the capsule at the same point
        // and distance (a step's top face and its riser at their shared edge), picked by
        // the tie order of (w), and classifying it made the result depend on the order of
        // a body's triangles (CO63). The probe, a contact query, sees all of them.
        let landed = [
            moved.feet[0],
            moved.feet[1],
            moved.feet[2] - down * hit.fraction,
        ];
        self.scratch.clear();
        let probe = self.config.shape.support_probe(landed);
        q.contacts(&probe, SUPPORT_MARGIN, &filter, &mut self.scratch);
        if best_support(q, &self.scratch, cosw) != Support::Walkable {
            return Raised::NotWalkable;
        }
        Raised::Landed(Slide {
            feet: landed,
            vel: [moved.vel[0], moved.vel[1], 0.0],
            blocked: false,
            planes: moved.planes,
        })
    }
}

/// How a step-up's raised move ended ([`Controller::raised_move`]).
#[derive(Debug, Clone, Copy)]
enum Raised {
    /// It landed on walkable support.
    Landed(Slide),
    /// It landed, but the support probe there is not walkable (§12.4 (o)).
    NotWalkable,
    /// The down sweep found nothing to land on.
    NoLanding,
}

/// Whether a step-up that climbs `climb` and gets `progress` along the move is taken over
/// the plain slide's `plain_progress` (`collision.md` §12.4 (k)): it climbs more than
/// [`STEP_UP_MIN_RISE`] and gets further along the move (strictly; no slack).
fn step_up_accepted(climb: f32, progress: f32, plain_progress: f32) -> bool {
    climb > STEP_UP_MIN_RISE && progress > plain_progress
}

/// Whether the short raised move (the move's own length `hl`), which climbs `short_climb`
/// and gets `short_progress` along the move, is taken instead of the raised move taken
/// (`climb` over `covered`): it must pass the same acceptance test as any step-up
/// ([`step_up_accepted`]: more than [`STEP_UP_MIN_RISE`] climbed, further than the plain
/// slide) and climb in proportion ([`climbs_in_proportion`]) (§12.4 (k)).
fn short_preferred(
    short_climb: f32,
    short_progress: f32,
    plain_progress: f32,
    climb: f32,
    hl: f32,
    covered: f32,
) -> bool {
    step_up_accepted(short_climb, short_progress, plain_progress)
        && climbs_in_proportion(short_climb, climb, hl, covered)
}

/// Whether the short raised move (the move's own length `hl`) climbs `short_climb` in
/// proportion to the raised move taken, which climbed `climb` over `covered` (the first
/// reach, or the full radius after the retry of §12.4 (o)), less [`PROPORTION_SLACK`]
/// (§12.4 (k)).
fn climbs_in_proportion(short_climb: f32, climb: f32, hl: f32, covered: f32) -> bool {
    short_climb >= climb * (hl / covered) - PROPORTION_SLACK
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
    /// along the first one still ran into the second (`collision.md` §12.4 (u),
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

/// `c` (a motion clipped against walkable planes `planes` only) turned back to the
/// horizontal direction of `orig`, keeping its horizontal length, and lifted onto the
/// highest of the planes: a walker on a side slope keeps its heading instead of drifting
/// downhill (`collision.md` §12.4 (v), Tessera Decision; it names no mover state, so it
/// applies in every mode but the raised move).
fn keep_heading(c: [f32; 3], orig: [f32; 3], planes: &Planes) -> [f32; 3] {
    let Some(dir) = normalize3([orig[0], orig[1], 0.0]) else {
        return c;
    };
    let len = (c[0] * c[0] + c[1] * c[1]).sqrt();
    if len.is_nan() || len <= 1e-9 || c[0] * dir[0] + c[1] * dir[1] <= 0.0 {
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

/// Whether a contact is sound: its separation is finite and its normal has unit length
/// within 10⁻³. Other contacts are hostile values; the push-out ignores them, also in the
/// test whether anything penetrates (`collision.md` §12.4 (g)), and so do the edge ties
/// of §12.4 (z) item 2.
fn sound_contact(c: &Contact) -> bool {
    let n = c.normal.map(f64::from);
    let len = (n[0] * n[0] + n[1] * n[1] + n[2] * n[2]).sqrt();
    c.separation.is_finite() && (len - 1.0).abs() < 1e-3
}

/// The rank of a class for the edge ties of `collision.md` §12.4 (z) item 4: walkable,
/// then steep, then wall, then overhang (higher is better). An ignored contact
/// (`Physics_AlwaysUncontrolledSurface`) ranks lowest; (z) never compares one, since an
/// ignored hit keeps its class and ignored contacts are left out.
fn tie_rank(class: ContactClass) -> u8 {
    match class {
        ContactClass::Walkable => 4,
        ContactClass::Steep => 3,
        ContactClass::Wall => 2,
        ContactClass::Overhang => 1,
        ContactClass::Ignored => 0,
    }
}

fn normalize3(a: [f32; 3]) -> Option<[f32; 3]> {
    let l = len3(a);
    (l.is_finite() && l > 1e-12).then(|| scale3(a, 1.0 / l))
}

#[cfg(test)]
mod tests {
    use super::super::params::{LayerKind, MovementLayer};
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
        // The pair is not normalised: §11.6 item 1 gives its target's length.
        let d = MoveInput {
            forward: 1.0,
            strafe: 1.0,
            ..MoveInput::default()
        }
        .sanitized();
        assert_eq!((d.forward, d.strafe), (1.0, 1.0));
    }

    /// `collision.md` §11.6 item 1 and the §11.8 speed table (CO61), BasePlayer and
    /// Walking values of §10.3.
    #[test]
    fn target_follows_the_original_rule() {
        let base = MovementParams::BUILTIN;
        let walking = MovementParams {
            forward_speed: 5.0,
            backward_speed: 4.5,
            strafe_left_speed: 4.5,
            strafe_right_speed: 4.5,
            ..base
        };
        let speed = |x: f32, y: f32, p: &MovementParams| {
            let (f, s) = target_velocity(x, y, p, 1.0);
            f.hypot(s)
        };
        let close = |a: f32, b: f32| (a - b).abs() < 1e-4;
        assert!(close(speed(0.0, 1.0, &base), 6.5));
        assert!(close(speed(0.0, -1.0, &base), 3.25));
        assert!(close(speed(1.0, 0.0, &base), 5.0));
        assert!(close(speed(-1.0, 1.0, &base), 5.75));
        assert!(close(speed(0.0, 0.5, &walking), 2.5));
        assert!(close(speed(0.0, -0.5, &walking), 2.25));
        assert!(close(speed(0.57, 0.0, &walking), 2.565));
        let diag = 0.57 * (0.57 * 4.5 + 0.5 * 5.0) / 1.07;
        assert!(close(speed(0.57, 0.5, &walking), diag), "{diag}");
        // Direction: along the pair, + side to the right.
        let (f, s) = target_velocity(0.57, 0.5, &walking, 1.0);
        assert!(f > 0.0 && s > 0.0 && close(s / f, 0.57 / 0.5));
        assert_eq!(target_velocity(0.0001, -0.0002, &base, 1.0), (0.0, 0.0));
        assert_eq!(target_velocity(f32::NAN, f32::NAN, &base, 1.0), (0.0, 0.0));
        assert!(close(target_velocity(0.0, 1.0, &base, 2.0).0, 13.0));
        // The client's strafe pairing: x > 0 takes the strafe-left speed.
        let uneven = MovementParams {
            strafe_left_speed: 2.0,
            strafe_right_speed: 4.0,
            ..base
        };
        assert!(close(target_velocity(1.0, 0.0, &uneven, 1.0).1, 2.0));
        assert!(close(target_velocity(-1.0, 0.0, &uneven, 1.0).1, -4.0));
    }

    #[test]
    fn heading_wraps() {
        use std::f32::consts::PI;
        assert!((wrap_heading(3.0 * PI) - PI).abs() < 1e-5);
        assert!((wrap_heading(-0.5) + 0.5).abs() < 1e-6);
        assert_eq!(wrap_heading(f32::NAN), 0.0);
    }

    /// Finding: a step-up was refused unless it got more than 1 mm further than the plain
    /// slide; §12.4 (k) says only "further".
    #[test]
    fn step_up_acceptance_is_strict_without_slack() {
        assert!(step_up_accepted(0.3, 0.1005, 0.1));
        assert!(!step_up_accepted(0.3, 0.1, 0.1));
        assert!(!step_up_accepted(0.3, 0.099, 0.1));
        assert!(!step_up_accepted(STEP_UP_MIN_RISE, 0.5, 0.1));
        assert!(!step_up_accepted(f32::NAN, 0.5, 0.1));
        assert!(!step_up_accepted(0.3, f32::NAN, 0.1));
        // Climbs in proportion: half the reach, half the climb, less the slack.
        assert!(climbs_in_proportion(0.15, 0.3, 0.1, 0.2));
        assert!(climbs_in_proportion(0.15 - PROPORTION_SLACK, 0.3, 0.1, 0.2));
        assert!(!climbs_in_proportion(0.148, 0.3, 0.1, 0.2));
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

    /// Finding (§12.4 (k)): the short raised move was taken after only the proportion
    /// test, so a full climb of 0.012 m over a reach of 0.175 m let a short move climbing
    /// 0.006 m (less than [`STEP_UP_MIN_RISE`]) through.
    #[test]
    fn short_raised_move_passes_the_acceptance_test_too() {
        let (hl, reach) = (0.0875, 0.175);
        // In proportion (0.006 ≥ 0.012 × 0.5 − slack), but it climbs less than 0.01 m.
        assert!(climbs_in_proportion(0.006, 0.012, hl, reach));
        assert!(!short_preferred(0.006, 0.2, 0.0, 0.012, hl, reach));
        // A short move that gets no further than the plain slide is not taken either.
        assert!(!short_preferred(0.2, 0.05, 0.05, 0.3, hl, reach));
        assert!(short_preferred(0.2, 0.0875, 0.0, 0.3, hl, reach));
    }

    /// Finding (§12.4 (k), (o)): after the full-radius retry, "in proportion" divided by
    /// the first reach instead of the radius the raised move taken covered.
    #[test]
    fn proportion_uses_the_length_the_raised_move_covered() {
        let (hl, reach, radius) = (0.0875_f32, 0.175_f32, 0.35_f32);
        // Retry taken: 0.3 m over 0.35 m; the move's own length climbing 0.1 m is in
        // proportion (0.1 ≥ 0.3 × 0.25 − slack), which the first reach would refuse.
        assert!(short_preferred(0.1, 0.0875, 0.0, 0.3, hl, radius));
        assert!(!climbs_in_proportion(0.1, 0.3, hl, reach));
    }

    /// A query with a wall in front of a grounded mover below 0.3 m, free space above it
    /// and, under every downward sweep, either nothing or one steep face. Counts the
    /// downward sweeps.
    struct WallMock {
        steep_floor: bool,
        down_sweeps: std::cell::Cell<u32>,
        first_hit: Option<[f32; 3]>,
        horizontal_sweeps: std::cell::Cell<u32>,
    }

    impl WallMock {
        fn hit(normal: [f32; 3], fraction: f32) -> SweepHit {
            SweepHit {
                fraction,
                gap: 2.0 * crate::geometry::SKIN,
                normal,
                point: [0.0; 3],
                face_normal: None,
                set: 0,
                body: crate::geometry::BodyId(0),
                kind: crate::geometry::FeatureKind::Round,
                feature: 0,
            }
        }
    }

    impl CollisionQuery for WallMock {
        fn sweep_capsule(
            &self,
            capsule: &crate::geometry::Capsule,
            delta: [f32; 3],
            _filter: &QueryFilter,
        ) -> Option<SweepHit> {
            let bottom = capsule.a[2].min(capsule.b[2]) - capsule.radius;
            let horizontal = delta[0] != 0.0 || delta[1] != 0.0;
            if horizontal {
                let n = self.horizontal_sweeps.get();
                self.horizontal_sweeps.set(n + 1);
                if let Some(normal) = self.first_hit {
                    return (n == 0).then(|| Self::hit(normal, 0.0));
                }
                return (bottom < 0.3).then(|| Self::hit([-1.0, 0.0, 0.0], 0.0));
            }
            if delta[2] < 0.0 {
                self.down_sweeps.set(self.down_sweeps.get() + 1);
                return self.steep_floor.then(|| Self::hit([0.8, 0.0, 0.6], 0.5));
            }
            None
        }

        fn contacts(
            &self,
            _capsule: &crate::geometry::Capsule,
            _margin: f32,
            _filter: &QueryFilter,
            _out: &mut Vec<Contact>,
        ) {
        }

        fn cell_at(&self, _p: [f32; 3]) -> Option<CellHit> {
            None
        }

        fn body(&self, _set: u32, _id: crate::geometry::BodyId) -> Option<&crate::geometry::Body> {
            None
        }
    }

    fn grounded(record: PlayerMovementRecord, velocity: [f32; 3]) -> Controller {
        Controller::new(
            ControllerConfig::default(),
            Arc::new(record),
            Arc::new(MovementPids::default()),
            LayerConditions::default(),
            MoverState {
                velocity,
                element: MovementState::EarthControlledSurface,
                support: Support::Walkable,
                ..MoverState::at([0.0; 3], 0.0)
            },
        )
    }

    /// Finding (§12.4 (o)): the full-radius retry ran when the first raised move's down
    /// sweep found nothing at all; it is stated only for a landing that is not walkable.
    #[test]
    fn full_radius_retry_only_after_a_landing_that_is_not_walkable() {
        let input = MoveInput {
            forward: 1.0,
            ..MoveInput::default()
        };
        for (steep_floor, expected) in [(false, 2), (true, 3)] {
            let q = WallMock {
                steep_floor,
                down_sweeps: std::cell::Cell::new(0),
                first_hit: None,
                horizontal_sweeps: std::cell::Cell::new(0),
            };
            let mut c = grounded(PlayerMovementRecord::builtin(), [0.0; 3]);
            let r = c.step(&input, &StepEnv::dry(&q, [0.0; 3]));
            assert_eq!(r.outcome, StepOutcome::Stepped);
            assert!(!r.stepped_up);
            // Nothing below: one raised move and the ground snap. A steep landing: the
            // raised move, its full-radius retry and the ground snap.
            assert_eq!(q.down_sweeps.get(), expected, "steep floor {steep_floor}");
        }
    }

    /// A wall below 0.3 m in front of a grounded mover. The down sweep of a raised move
    /// (bottom above 0.2 m) stops halfway on a hit with `landing` as its (contact normal,
    /// face normal); the support probe above the floor sees one walkable contact when
    /// `probe_walkable` and nothing otherwise.
    struct EdgeMock {
        landing: ([f32; 3], Option<[f32; 3]>),
        probe_walkable: bool,
    }

    impl CollisionQuery for EdgeMock {
        fn sweep_capsule(
            &self,
            capsule: &crate::geometry::Capsule,
            delta: [f32; 3],
            _filter: &QueryFilter,
        ) -> Option<SweepHit> {
            let bottom = capsule.a[2].min(capsule.b[2]) - capsule.radius;
            if delta[0] != 0.0 || delta[1] != 0.0 {
                return (bottom < 0.3).then(|| WallMock::hit([-1.0, 0.0, 0.0], 0.0));
            }
            if delta[2] < 0.0 && bottom > 0.2 {
                let mut hit = WallMock::hit(self.landing.0, 0.5);
                hit.face_normal = self.landing.1;
                hit.kind = crate::geometry::FeatureKind::Triangle;
                return Some(hit);
            }
            None
        }

        fn contacts(
            &self,
            capsule: &crate::geometry::Capsule,
            _margin: f32,
            _filter: &QueryFilter,
            out: &mut Vec<Contact>,
        ) {
            let bottom = capsule.a[2].min(capsule.b[2]) - capsule.radius;
            if bottom.abs() < 1e-3 || (bottom > 0.01 && self.probe_walkable) {
                out.push(Contact {
                    separation: 0.0,
                    normal: [0.0, 0.0, 1.0],
                    point: [0.0, 0.0, bottom],
                    face_normal: Some([0.0, 0.0, 1.0]),
                    set: 0,
                    body: crate::geometry::BodyId(0),
                    kind: crate::geometry::FeatureKind::Triangle,
                    feature: 0,
                });
            }
        }

        fn cell_at(&self, _p: [f32; 3]) -> Option<CellHit> {
            None
        }

        fn body(&self, _set: u32, _id: crate::geometry::BodyId) -> Option<&crate::geometry::Body> {
            None
        }
    }

    /// §12.4 (k) as revised 2026-10-23 (CO63): a step-up landing is judged by the support
    /// probe alone. The contact the down sweep stops on (at a step's edge, the riser and
    /// the top face tie and (w) reports one of them by triangle order) is not classified:
    /// a riser reported there (its face normal replaces the contact normal, §12.4 (x)
    /// item 4, a wall) does not refuse a landing the probe finds walkable, and a walkable
    /// hit does not accept one the probe does not.
    #[test]
    fn step_up_landing_is_judged_by_the_support_probe_alone() {
        let input = MoveInput {
            forward: 1.0,
            ..MoveInput::default()
        };
        let edge = [-0.5, 0.0, 0.75f32.sqrt()];
        let riser = (edge, Some([-1.0, 0.0, 0.0]));
        let top = (edge, Some([0.0, 0.0, 1.0]));
        let cosw = walkable_cos(true);
        assert_eq!(classify(riser.0, riser.1, false, cosw), ContactClass::Wall);
        assert_eq!(classify(top.0, top.1, false, cosw), ContactClass::Walkable);
        for landing in [riser, top] {
            for probe_walkable in [true, false] {
                let q = EdgeMock {
                    landing,
                    probe_walkable,
                };
                let mut c = grounded(PlayerMovementRecord::builtin(), [0.0; 3]);
                let r = c.step(&input, &StepEnv::dry(&q, [0.0; 3]));
                assert_eq!(r.outcome, StepOutcome::Stepped);
                assert_eq!(
                    r.stepped_up, probe_walkable,
                    "landing {landing:?}, probe walkable {probe_walkable}"
                );
            }
        }
    }

    /// Contacts for the push-out of §12.4 (g): a floor 1.5 mm below the capsule (closer
    /// than two skins, not penetrating) and, when `hostile`, a "penetrating" contact whose
    /// normal is not of unit length. No sweep hits anything.
    struct PushMock {
        hostile: bool,
    }

    impl CollisionQuery for PushMock {
        fn sweep_capsule(
            &self,
            _capsule: &crate::geometry::Capsule,
            _delta: [f32; 3],
            _filter: &QueryFilter,
        ) -> Option<SweepHit> {
            None
        }

        fn contacts(
            &self,
            _capsule: &crate::geometry::Capsule,
            _margin: f32,
            _filter: &QueryFilter,
            out: &mut Vec<Contact>,
        ) {
            let contact = |separation: f32, normal: [f32; 3]| Contact {
                separation,
                normal,
                point: [0.0; 3],
                face_normal: None,
                set: 0,
                body: crate::geometry::BodyId(0),
                kind: crate::geometry::FeatureKind::Triangle,
                feature: 0,
            };
            out.push(contact(0.0015, [0.0, 0.0, 1.0]));
            if self.hostile {
                out.push(contact(-0.2, [0.0, 2.0, 0.0]));
            }
        }

        fn cell_at(&self, _p: [f32; 3]) -> Option<CellHit> {
            None
        }

        fn body(&self, _set: u32, _id: crate::geometry::BodyId) -> Option<&crate::geometry::Body> {
            None
        }
    }

    /// Finding (§12.4 (g)): a contact whose normal is not of unit length is a hostile value
    /// and is ignored, also in the test whether anything penetrates; it started a push-out
    /// round that moved the mover off a floor that did not penetrate.
    #[test]
    fn hostile_contacts_do_not_start_a_push_out() {
        for hostile in [false, true] {
            let q = PushMock { hostile };
            let mut c = grounded(PlayerMovementRecord::builtin(), [0.0; 3]);
            let r = c.step(&MoveInput::default(), &StepEnv::dry(&q, [0.0; 3]));
            assert_eq!(r.outcome, StepOutcome::Stepped);
            assert_eq!(r.pushed_out, 0.0, "hostile {hostile}");
            assert_eq!(c.state().position, [0.0; 3], "hostile {hostile}");
        }
    }

    /// Contacts for the edge ties of §12.4 (z): a contact query returns `contacts` (when
    /// its margin is at least the contact's separation, as a real query does) and records
    /// the margins it was asked for. Body 1 has `Physics_AlwaysUncontrolledSurface`.
    struct TieMock {
        contacts: Vec<Contact>,
        margins: std::cell::RefCell<Vec<f32>>,
        uncontrolled: crate::geometry::Body,
    }

    impl TieMock {
        fn new(contacts: Vec<Contact>) -> Self {
            Self {
                contacts,
                margins: std::cell::RefCell::new(Vec::new()),
                uncontrolled: crate::geometry::Body {
                    source: crate::geometry::BodySource::Other { tag: 1 },
                    flags: crate::geometry::CollisionFlags {
                        always_uncontrolled_surface: Some(true),
                        ..crate::geometry::CollisionFlags::default()
                    },
                    purpose: crate::geometry::BodyPurpose::Movement,
                    first_triangle: 0,
                    triangle_count: 0,
                    round_count: 0,
                },
            }
        }
    }

    impl CollisionQuery for TieMock {
        fn sweep_capsule(
            &self,
            _capsule: &crate::geometry::Capsule,
            _delta: [f32; 3],
            _filter: &QueryFilter,
        ) -> Option<SweepHit> {
            None
        }

        fn contacts(
            &self,
            _capsule: &crate::geometry::Capsule,
            margin: f32,
            _filter: &QueryFilter,
            out: &mut Vec<Contact>,
        ) {
            self.margins.borrow_mut().push(margin);
            out.extend(self.contacts.iter().filter(|c| c.separation <= margin));
        }

        fn cell_at(&self, _p: [f32; 3]) -> Option<CellHit> {
            None
        }

        fn body(&self, _set: u32, id: crate::geometry::BodyId) -> Option<&crate::geometry::Body> {
            (id.0 == 1).then_some(&self.uncontrolled)
        }
    }

    /// The unit normal of an edge contact 30° from up (walkable by its contact normal).
    fn edge_normal() -> [f32; 3] {
        [-0.5, 0.0, 0.75f32.sqrt()]
    }

    /// A sweep hit on a triangle at the origin with contact normal `normal`, face normal
    /// `face` and gap `gap`.
    fn tri_hit(normal: [f32; 3], face: Option<[f32; 3]>, gap: f32) -> SweepHit {
        SweepHit {
            fraction: 0.5,
            gap,
            normal,
            point: [0.0; 3],
            face_normal: face,
            set: 0,
            body: crate::geometry::BodyId(0),
            kind: crate::geometry::FeatureKind::Triangle,
            feature: 0,
        }
    }

    /// A triangle contact at `point` with `separation`, contact normal `normal` and face
    /// normal `face`, on body 0.
    fn tri_contact(
        point: [f32; 3],
        separation: f32,
        normal: [f32; 3],
        face: Option<[f32; 3]>,
    ) -> Contact {
        Contact {
            separation,
            normal,
            point,
            face_normal: face,
            set: 0,
            body: crate::geometry::BodyId(0),
            kind: crate::geometry::FeatureKind::Triangle,
            feature: 1,
        }
    }

    /// §12.4 (z): a hit on a step's riser at its edge with the top face (the riser's face
    /// normal makes it a wall by (x) item 4, its contact normal alone is walkable) takes
    /// the top face's walkable class when the top face's contact lies within four skins of
    /// the hit point, from a contact query with the margin of the hit's gap plus one skin;
    /// not when it lies further, is hostile by the (g) test (finite but not of unit
    /// length), is on an object with `Physics_AlwaysUncontrolledSurface`, is not a
    /// triangle, or lies beyond a negative margin.
    #[test]
    fn edge_ties_take_the_best_class_of_the_triangles_at_the_hit_point() {
        let cosw = walkable_cos(true);
        let n = edge_normal();
        let riser = Some([-1.0, 0.0, 0.0]);
        let top = Some([0.0, 0.0, 1.0]);
        let gap = 2.0 * SKIN;
        let hit = tri_hit(n, riser, gap);
        assert_eq!(classify(n, riser, false, cosw), ContactClass::Wall);
        let near = [0.0, 0.0, 3.9 * SKIN];
        let cases: Vec<(&str, Contact, ContactClass)> = vec![
            (
                "top face near",
                tri_contact(near, gap, n, top),
                ContactClass::Walkable,
            ),
            (
                "top face beyond four skins",
                tri_contact([0.0, 0.0, 4.1 * SKIN], gap, n, top),
                ContactClass::Wall,
            ),
            (
                "hostile normal",
                tri_contact(near, gap, n.map(|v| v * 1.002), top),
                ContactClass::Wall,
            ),
            (
                "hostile separation",
                tri_contact(near, f32::NEG_INFINITY, n, top),
                ContactClass::Wall,
            ),
            (
                "always uncontrolled",
                Contact {
                    body: crate::geometry::BodyId(1),
                    ..tri_contact(near, gap, n, top)
                },
                ContactClass::Wall,
            ),
            (
                "round",
                Contact {
                    kind: crate::geometry::FeatureKind::Round,
                    face_normal: None,
                    ..tri_contact(near, gap, n, None)
                },
                ContactClass::Wall,
            ),
            (
                "steep",
                tri_contact(near, gap, [-0.8, 0.0, 0.6], None),
                ContactClass::Steep,
            ),
        ];
        for (what, contact, want) in cases {
            let q = TieMock::new(vec![contact]);
            let mut c = grounded(PlayerMovementRecord::builtin(), [0.0; 3]);
            assert_eq!(c.hit_class(&q, &hit, [0.0; 3], cosw), want, "{what}");
            assert_eq!(*q.margins.borrow(), vec![gap + SKIN], "{what}: margin");
        }

        // The radius is measured from the hit triangle's own contact in the query (same
        // set, body, kind and feature as the hit), not from the sweep's hit point, which
        // lies a few skins back along the motion: a top-face contact 10 skins from the
        // sweep's point but next to the riser's own contact counts.
        let moved = [10.0 * SKIN, 0.0, 0.0];
        let own = Contact {
            feature: 0,
            ..tri_contact(moved, gap, n, riser)
        };
        let top_by_own = tri_contact([10.0 * SKIN, 0.0, 2.0 * SKIN], gap, n, top);
        let q = TieMock::new(vec![own, top_by_own]);
        let mut c = grounded(PlayerMovementRecord::builtin(), [0.0; 3]);
        assert_eq!(
            c.hit_class(&q, &hit, [0.0; 3], cosw),
            ContactClass::Walkable,
            "measured from the hit triangle's own contact"
        );
        let q = TieMock::new(vec![top_by_own]);
        assert_eq!(
            c.hit_class(&q, &hit, [0.0; 3], cosw),
            ContactClass::Wall,
            "no own contact: measured from the sweep's point"
        );

        // A negative gap (the capsule penetrates the hit triangle): the query runs with 0
        // and only contacts within the margin gap + one skin count.
        let gap = -0.01;
        let hit = tri_hit(n, riser, gap);
        for (sep, want) in [
            (-0.005, ContactClass::Wall),
            (-0.02, ContactClass::Walkable),
        ] {
            let q = TieMock::new(vec![tri_contact(near, sep, n, top)]);
            let mut c = grounded(PlayerMovementRecord::builtin(), [0.0; 3]);
            assert_eq!(
                c.hit_class(&q, &hit, [0.0; 3], cosw),
                want,
                "separation {sep}"
            );
            assert_eq!(*q.margins.borrow(), vec![0.0], "separation {sep}: margin");
        }
    }

    /// §12.4 (z) applies only where (x) item 4 lowered the hit's class: a hit whose face
    /// normal does not replace its contact normal, a hit on a round primitive and a hit on
    /// an always-uncontrolled object keep their class without a contact query. The best
    /// class wins in the order walkable, steep, wall, overhang: a wall hit made an
    /// overhang by its face normal takes a wall contact at the same point.
    #[test]
    fn edge_ties_apply_only_where_the_face_normal_lowered_the_class() {
        let cosw = walkable_cos(true);
        let n = edge_normal();
        let gap = 2.0 * SKIN;
        let top_contact = tri_contact([0.0; 3], gap, n, Some([0.0, 0.0, 1.0]));
        let unchanged = [
            (
                "face not substituted",
                tri_hit(n, Some([0.0, 0.0, 1.0]), gap),
            ),
            ("no face normal", tri_hit(n, None, gap)),
            (
                "round",
                SweepHit {
                    kind: crate::geometry::FeatureKind::Round,
                    ..tri_hit(n, Some([-1.0, 0.0, 0.0]), gap)
                },
            ),
            (
                "always uncontrolled",
                SweepHit {
                    body: crate::geometry::BodyId(1),
                    ..tri_hit(n, Some([-1.0, 0.0, 0.0]), gap)
                },
            ),
        ];
        for (what, hit) in unchanged {
            let q = TieMock::new(vec![top_contact]);
            let mut c = grounded(PlayerMovementRecord::builtin(), [0.0; 3]);
            let own = classify(hit.normal, hit.face_normal, hit.body.0 == 1, cosw);
            assert_eq!(c.hit_class(&q, &hit, [0.0; 3], cosw), own, "{what}");
            assert!(q.margins.borrow().is_empty(), "{what}: no query");
        }

        // Wall over overhang: a near-vertical contact normal (a wall) whose face normal
        // points down (an overhang by (x) item 4).
        let wall_n = normalize3([-1.0, 0.0, 0.05]).unwrap();
        let hit = tri_hit(wall_n, Some([0.0, 0.0, -1.0]), gap);
        assert_eq!(
            classify(wall_n, hit.face_normal, false, cosw),
            ContactClass::Overhang
        );
        let overhang = tri_contact([0.0; 3], gap, [0.0, 0.0, -1.0], None);
        let wall = tri_contact([0.0; 3], gap, wall_n, None);
        for (contacts, want) in [
            (vec![overhang], ContactClass::Overhang),
            (vec![overhang, wall], ContactClass::Wall),
            (vec![wall, overhang], ContactClass::Wall),
        ] {
            let q = TieMock::new(contacts);
            let mut c = grounded(PlayerMovementRecord::builtin(), [0.0; 3]);
            assert_eq!(c.hit_class(&q, &hit, [0.0; 3], cosw), want);
        }
    }

    /// Finding (§12.3 step 3, §12.4 (v)): keeping the heading lifted the clipped velocity
    /// onto a walkable plane with its horizontal length kept, and the result was never
    /// limited again: forward and terminal speed 120, damper 1, into a 44° plane at 67°
    /// off its fall line gave about 120.9 m/s.
    #[test]
    fn keeping_the_heading_never_exceeds_the_speed_limit() {
        let base = MovementLayer {
            forward_speed: Some(120.0),
            backward_speed: Some(120.0),
            strafe_left_speed: Some(120.0),
            strafe_right_speed: Some(120.0),
            control_acceleration: Some([1000.0; 3]),
            velocity_damper: Some([1.0; 3]),
            ramp_time: Some(0.0),
            gravity: Some(0.0),
            terminal_velocity: Some(120.0),
            step_up_height: Some(0.5),
            ..MovementLayer::default()
        };
        let mut record = PlayerMovementRecord::default();
        record.layers[LayerKind::BasePlayer.index()] = Some(base);
        let (s, c) = 44.0_f32.to_radians().sin_cos();
        let heading = 67.0_f32.to_radians();
        let q = WallMock {
            steep_floor: false,
            down_sweeps: std::cell::Cell::new(0),
            first_hit: Some([-s, 0.0, c]),
            horizontal_sweeps: std::cell::Cell::new(0),
        };
        let v = [120.0 * heading.cos(), 120.0 * heading.sin(), 0.0];
        let mut ctl = grounded(record, v);
        ctl.state.heading = heading;
        let r = ctl.step(
            &MoveInput {
                forward: 1.0,
                ..MoveInput::default()
            },
            &StepEnv::dry(&q, [0.0; 3]),
        );
        assert_eq!(r.outcome, StepOutcome::Stepped);
        let w = ctl.state().velocity;
        let speed = len3(w);
        assert!(w[2] > 1.0, "lifted onto the plane: {w:?}");
        assert!(speed <= 120.0 * (1.0 + 1e-6), "{speed}");
    }

    /// A reverted step also undoes the war-steed drive's update: its stored control velocity
    /// is the one before the step, zeroed, so the next step does not repeat the motion.
    #[test]
    fn revert_zeroes_the_war_steed_drive() {
        use super::super::warsteed::{GaitMultipliers, WarSteedActor};
        let mut c = grounded(PlayerMovementRecord::builtin(), [3.0, 0.0, 0.0]);
        let before = *c.state();
        let mut d = WarSteedDrive::new(
            WarSteedActor::default(),
            GaitMultipliers {
                gallop_speed: 1.0,
                locked_gear_speed: None,
                turn: 1.0,
            },
            16.0,
        );
        d.velocity = [0.0, 16.0];
        let drive_before = Some(d);
        // The step's update ran (the drive moved on), then the result was undone.
        c.set_war_steed(Some(WarSteedDrive {
            velocity: [0.0, 16.2],
            ..d
        }));
        c.revert(before, drive_before);
        assert_eq!(c.state().velocity, [0.0; 3]);
        let after = c.war_steed().unwrap();
        assert_eq!(after.velocity, [0.0; 2]);
        assert!(after.last.is_none());
        // Without a drive before the step there is none after it.
        c.revert(before, None);
        assert!(c.war_steed().is_none());
        // The jump table can be swapped (mounted branch, `mounts.md` §14.12).
        c.set_jump_table(None);
        assert!(c.config().jump.is_none());
    }
}
