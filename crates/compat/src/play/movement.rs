// SPDX-License-Identifier: GPL-3.0-or-later
//! Player movement and remote entity motion (`docs/specs/official-messages-play.md` §4).
//!
//! - [`MovementSender`]: the monotonic step counter, the 4-step rolling window of `0x24`
//!   (§4.1–§4.2, a ring of the last four steps) with the planar movement grid
//!   ([`move_state_of`]), the jump and walk bits and the quantized angles, and, apart from
//!   it, the history of the 30 most recent steps that `0xCC` reconciles against (§4.3.2,
//!   0.15 m and one heading unit). An acknowledgement prunes and a teleport clears only
//!   the history; an ack ahead of the newest step also restarts the window there.
//! - [`TeleportSync`]: the modulo-256 teleport stamp state machine and the two-gate portal
//!   exit (§4.5).
//! - [`ServerSetPosition`] (`0xBF`, §4.4.2), [`EntitySplinePath`] (`0x59`, §4.6.1) and
//!   [`MotionControllerUpdate`] (`0xED`, §4.6.2); the play-layer bounds of `0xBD` (§4.6.3)
//!   around [`EntityAnimLayerState`].
//!
//! The wire layouts of `0x24`, `0xCC`, `0xCE` and `0xBD` live in [`crate::world`]
//! (`official-messages-world.md` §4.3.1, §5.4, §5.5, §5.6.1), as do the
//! `SignedNormalizedByte` codec and the coordinate bounds of §9.1 rule 4; this module uses
//! them.

use std::collections::VecDeque;

use tessera_net::{Anchor, Landblock, Location, RegionId, Rotation};

use crate::play::PlayError;
pub use crate::world::{
    ActionLayerEntry, EntityAnimLayerState, decode_signed_normalized, encode_signed_normalized,
};

/// Event ID of the portal-space start (`enter_portal_space`, §4.5 item 2).
pub const OPCODE_ENTER_PORTAL_SPACE: u32 = EnterPortalSpace::EVENT_ID;
/// Event ID of the initial-entry server gate (`login_finished`, §4.5 item 4).
pub const OPCODE_LOGIN_FINISHED: u32 = crate::world::LoginFinished::EVENT_ID;
/// Event ID of the in-world portal server gate (`portal_transition_complete`, §4.5 item 4).
pub const OPCODE_PORTAL_TRANSITION_COMPLETE: u32 = crate::world::PortalTransitionComplete::EVENT_ID;
use crate::world::{
    ClientPhysicsSync, EnterPortalSpace, PortalPositionSync, PropertySchemaLookup, WirePosition,
    WireReader, WireWriter, decode_heading_u8, encode_heading_u8, require_finite,
    require_world_coordinate,
};

/// Event ID of the client movement step event (`client_physics_sync`, §4.2).
pub const OPCODE_CLIENT_PHYSICS_SYNC: u32 = ClientPhysicsSync::EVENT_ID;
/// Event ID of the full movement-state setter (`server_set_position`, §4.4.2).
pub const OPCODE_SERVER_SET_POSITION: u32 = 0x0000_00BF;
/// Event ID of the entity path event (`entity_spline_path`, §4.6.1).
pub const OPCODE_ENTITY_SPLINE_PATH: u32 = 0x0000_0059;
/// Event ID of the motion controller event (`entity_motion_state`, §4.6.2).
pub const OPCODE_ENTITY_MOTION_STATE: u32 = 0x0000_00ED;
/// Event ID of the animation layer event (`entity_anim_layer_state`, §4.6.3).
pub const OPCODE_ENTITY_ANIM_LAYER_STATE: u32 = EntityAnimLayerState::EVENT_ID;

/// Nominal step interval while moving: 20 Hz (§4.2 item 1).
pub const STEP_INTERVAL_SEC: f64 = 0.05;
/// Slack when testing whether a periodic step is due, so that frame times that land on
/// the 50 ms grid up to rounding are not a frame late. Engineering choice, spec silent.
pub const STEP_TIME_EPSILON_SEC: f64 = 1e-6;
/// Steps in the rolling window of `0x24` (§4.1 item 2).
pub const WINDOW_STEPS: usize = 4;
/// Steps of history kept for reconciliation: the 30 most recent (§4.3.2).
pub const STEP_HISTORY_CAP: usize = 30;
/// Position tolerance of an acknowledgement: 0.15 m (§4.3.2 item 2).
pub const RECONCILE_DISTANCE_M: f64 = 0.15;
/// Heading tolerance of an acknowledgement: one quantized heading unit (§4.3.2 item 2).
pub const RECONCILE_HEADING_UNITS: u8 = 1;
/// The stationary `move_state` byte the encoder writes (§4.1 item 4: `0x00` and `0x80`
/// both mean stationary; `0x80` is the grid's centre).
pub const MOVE_STATE_STATIONARY: u8 = 0x80;
/// Columns of the planar movement grid (`forward_axis`, §4.1 item 4).
pub const GRID_COLUMNS: u8 = 15;
/// Rows of the planar movement grid (`strafe_axis`, §4.1 item 4).
pub const GRID_ROWS: u8 = 17;
/// Most waypoints accepted in one `0x59` path. Engineering choice, spec silent (it bounds
/// the count only by the remaining bytes).
pub const MAX_WAYPOINTS: usize = 1024;
/// Smallest wire size of one `0x59` waypoint: a `Full wire Position` is 1 to 36 bytes
/// (§3.1), so the count check of §4.6.1 (`M <= remaining_bytes`) is the tightest bound the
/// spec allows; a 1-byte position (only `flags`) is valid on the wire.
pub const MIN_WAYPOINT_BYTES: usize = 1;
/// Most animation layers accepted in one `0xBD` event. Engineering choice, spec silent.
pub const MAX_ANIM_LAYERS: usize = 256;

// ---------------------------------------------------------------------------
// Wire scalars and coordinates
// ---------------------------------------------------------------------------

/// Whether `new_stamp` is newer than or equal to `current` under modulo-256 serial number
/// arithmetic: `(new - current) mod 256 < 128` (§4.5 item 1).
#[must_use]
pub fn stamp_newer_or_equal(new_stamp: u8, current: u8) -> bool {
    new_stamp.wrapping_sub(current) < 0x80
}

/// The region-qualified location (and, for an interior cell, the anchor) of a wire
/// `Position` (`coordinates.md` §4–§7): region-global `160 · block + offset`. `None` when
/// the position is invalid (region 0, block 0xFF, cell 0xFFFF) or not finite.
#[must_use]
pub fn wire_location(pos: &WirePosition) -> Option<(Location, Option<Anchor>)> {
    if pos.region == 0 || pos.cell == 0xFFFF || !pos.offset.iter().all(|c| c.is_finite()) {
        return None;
    }
    let block = Landblock::new(RegionId(u16::from(pos.region)), pos.block_x, pos.block_y)?;
    let [x, y, z] = pos.offset;
    let location = block.location([f64::from(x), f64::from(y), f64::from(z)]);
    let anchor = (pos.cell != 0).then(|| Anchor::new(block, pos.cell));
    Some((location, anchor))
}

/// Tessera's heading (radians, 0 = +X, counter-clockwise, `(-π, π]`) of a quantized wire
/// heading. The wire angle is measured counter-clockwise from +Y (north) around +Z
/// (`official-messages-world.md` §2.5.2, play §3.1: `0x40` faces west), so Tessera's
/// heading is the wire angle plus π/2.
#[must_use]
pub fn wire_heading(raw: u8) -> f32 {
    wrap_pi(decode_heading_u8(raw) + std::f32::consts::FRAC_PI_2)
}

/// The quantized wire heading of Tessera's `heading` (the inverse of [`wire_heading`],
/// rounding as `official-messages-world.md` §2.5.2 encodes); `0` for a non-finite heading.
#[must_use]
pub fn heading_to_wire(heading: f32) -> u8 {
    if !heading.is_finite() {
        return 0;
    }
    encode_heading_u8(heading - std::f32::consts::FRAC_PI_2)
}

/// The circular difference of two quantized headings in heading units (`0..=128`).
#[must_use]
pub fn heading_units_apart(a: u8, b: u8) -> u8 {
    let d = a.wrapping_sub(b);
    d.min(d.wrapping_neg())
}

/// Tessera's heading of a wire quaternion `(w, x, y, z)`, `None` when degenerate.
#[must_use]
pub fn quaternion_heading(q: [f32; 4]) -> Option<f32> {
    Rotation::new(q[0], q[1], q[2], q[3]).heading()
}

fn wrap_pi(a: f32) -> f32 {
    let tau = std::f32::consts::TAU;
    let r = (a + std::f32::consts::PI).rem_euclid(tau) - std::f32::consts::PI;
    if r <= -std::f32::consts::PI {
        r + tau
    } else {
        r
    }
}

/// The finiteness check of §9.1 rule 4 ([`crate::world`]'s), as a play error.
fn check_finite(field: &'static str, v: f32) -> Result<(), PlayError> {
    require_finite(field, v)?;
    Ok(())
}

// ---------------------------------------------------------------------------
// §4.1–§4.2 Movement steps and 0x24
// ---------------------------------------------------------------------------

/// The `move_state` byte of the body-local planar movement `(forward, strafe)`, each in
/// `[-1, 1]` (§4.1 item 4): `0x80` when both are zero, else `1 + 15 · r + c` with
/// `c = clamp(floor((forward + 1) · 0.5 · 15), 0, 14)` and
/// `r = clamp(floor((strafe + 1) · 0.5 · 17), 0, 16)`. A non-finite axis counts as `0`.
#[must_use]
pub fn move_state_of(forward: f32, strafe: f32) -> u8 {
    let clean = |v: f32| {
        if v.is_finite() {
            v.clamp(-1.0, 1.0)
        } else {
            0.0
        }
    };
    let (f, s) = (clean(forward), clean(strafe));
    if f == 0.0 && s == 0.0 {
        return MOVE_STATE_STATIONARY;
    }
    let cell = |v: f32, n: u8| {
        let q = ((v + 1.0) * 0.5 * f32::from(n)).floor();
        // Clamped to `0..n - 1` (at most 16), so the cast is exact.
        #[allow(clippy::cast_possible_truncation, clippy::cast_sign_loss)]
        let q = q.clamp(0.0, f32::from(n - 1)) as u8;
        q
    };
    let c = cell(f, GRID_COLUMNS);
    let r = cell(s, GRID_ROWS);
    1 + GRID_COLUMNS * r + c
}

/// The body-local planar movement `(forward, strafe)` of a `move_state` byte (§4.1 item 4):
/// `0x00` and `0x80` are stationary; any other byte is the centre of its grid cell, with
/// the centre column and row read as exactly `0`.
#[must_use]
pub fn move_state_axes(move_state: u8) -> (f32, f32) {
    if move_state == 0 || move_state == MOVE_STATE_STATIONARY {
        return (0.0, 0.0);
    }
    let m = move_state - 1;
    let (c, r) = (m % GRID_COLUMNS, m / GRID_COLUMNS);
    let axis = |i: u8, n: u8| {
        if i == n / 2 {
            0.0
        } else {
            (f32::from(i) + 0.5) / f32::from(n) * 2.0 - 1.0
        }
    };
    (axis(c, GRID_COLUMNS), axis(r, GRID_ROWS))
}

/// The local player's movement input for one frame, in the shape the connector builds
/// (`official-messages-play.md` §8). Only `forward` and `strafe` are read: the movement
/// state byte is derived from them ([`move_state_of`]) and the spec has no turn axis, so
/// `move_state` and `turn` are not sent. The heading comes with the prediction
/// ([`crate::play::adapter::PlayAdapter::movement_step`]); [`MoveStep`] carries every field
/// of a step.
#[derive(Debug, Clone, Copy, PartialEq, Default)]
pub struct MovementInput {
    /// Not sent (derived from the axes, §4.1 item 4).
    pub move_state: u8,
    /// Body-local strafe axis (right minus left), `[-1, 1]`.
    pub strafe: f32,
    /// Body-local forward axis, `[-1, 1]`.
    pub forward: f32,
    /// Not sent (no turn axis in §4.1).
    pub turn: f32,
}

/// Everything one movement step reports (§4.1 items 4–6, §8 row `0x24`).
#[derive(Debug, Clone, Copy, PartialEq, Default)]
pub struct MoveStep {
    /// Body-local forward axis, `[-1, 1]`.
    pub forward: f32,
    /// Body-local strafe axis (right minus left), `[-1, 1]`.
    pub strafe: f32,
    /// The entity's heading (Tessera's convention), `axis_a`.
    pub heading: f32,
    /// The camera's heading (Tessera's convention), `axis_b`; `None`: the entity's heading
    /// (§8 row `0x24` quantizes the heading into both).
    pub camera_heading: Option<f32>,
    /// The camera pitch byte `axis_c` (§8 row `0x24`: `0x00`).
    pub camera_pitch: u8,
    /// Jump active during the step (`slot_flags` bit 1).
    pub jump: bool,
    /// Walking rather than running (`slot_flags` bit 6).
    pub walk: bool,
}

impl MoveStep {
    /// The step of a neutral move (`official-messages-play.md` §8 row `0x24`): the
    /// velocity's horizontal direction relative to `heading` as unit body-local axes (no
    /// planar movement below 1 mm/s), the heading as both entity and camera heading, pitch
    /// `0x00`, running. The axes are a direction, not a speed (Tessera's reading; the walk
    /// bit has no neutral source and stays clear). The jump bit (§4.1 item 5: "jump active
    /// during this step") is set while the controller is off the ground and rising
    /// (upward velocity): Tessera's reading, the spec does not say where the bit comes from
    /// (question in `docs/m8-remediation.md` §15). A fall is not a jump, so a fall with no
    /// planar input repeats no step at 20 Hz.
    #[must_use]
    pub fn from_move(heading: f32, velocity: [f32; 3], on_ground: bool) -> Self {
        let [vx, vy, vz] = velocity;
        let speed = (vx * vx + vy * vy).sqrt();
        let mut step = Self {
            heading,
            jump: !on_ground && vz.is_finite() && vz > 1e-3,
            ..Self::default()
        };
        if speed.is_finite() && speed >= 1e-3 && heading.is_finite() {
            // `tessera_net::model`: heading θ faces (cos θ, sin θ); +X east, +Y north, +Z
            // up. The strafe axis is positive to the right of the facing.
            let (s, c) = heading.sin_cos();
            step.forward = ((vx * c + vy * s) / speed).clamp(-1.0, 1.0);
            step.strafe = ((vx * s - vy * c) / speed).clamp(-1.0, 1.0);
        }
        step
    }

    /// The step of the connector's [`MovementInput`] facing `heading`: its axes, no jump,
    /// running.
    #[must_use]
    pub fn from_input(input: MovementInput, heading: f32) -> Self {
        Self {
            forward: input.forward,
            strafe: input.strafe,
            heading,
            ..Self::default()
        }
    }
}

/// One quantized step as it goes on the wire (§4.1 items 4–6).
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub struct StepInput {
    /// The planar movement grid byte.
    pub move_state: u8,
    /// `slot_flags` jump (`0x02`) and walk (`0x40`) bits.
    pub slot_flags: u8,
    /// `axis_a` (entity heading), `axis_b` (camera heading), `axis_c` (camera pitch).
    pub axes: [u8; 3],
}

impl Default for StepInput {
    /// Stationary, no flags, all angles `0x00`.
    fn default() -> Self {
        Self {
            move_state: MOVE_STATE_STATIONARY,
            slot_flags: 0,
            axes: [0; 3],
        }
    }
}

impl StepInput {
    /// Quantizes `step`.
    #[must_use]
    pub fn from_step(step: MoveStep) -> Self {
        let mut flags = 0u8;
        if step.jump {
            flags |= ClientPhysicsSync::SLOT_FLAG_JUMP;
        }
        if step.walk {
            flags |= ClientPhysicsSync::SLOT_FLAG_WALK;
        }
        let heading = heading_to_wire(step.heading);
        Self {
            move_state: move_state_of(step.forward, step.strafe),
            slot_flags: flags,
            axes: [
                heading,
                step.camera_heading.map_or(heading, heading_to_wire),
                step.camera_pitch,
            ],
        }
    }

    /// The entity heading byte (`axis_a`).
    #[must_use]
    pub fn heading(&self) -> u8 {
        self.axes[0]
    }

    /// Planar movement or a jump: the steps that repeat at 20 Hz (§4.2 item 1; a heading
    /// change and queued actions are input changes, sent at once).
    #[must_use]
    pub fn is_moving(&self) -> bool {
        (self.move_state != 0 && self.move_state != MOVE_STATE_STATIONARY)
            || self.slot_flags & ClientPhysicsSync::SLOT_FLAG_JUMP != 0
    }

    /// Whether `self` differs from `other` in what makes a step at once (§4.2 item 1):
    /// `move_state`, the jump or walk bit, or the entity heading. Camera angles alone do
    /// not.
    #[must_use]
    pub fn changed_from(&self, other: &Self) -> bool {
        self.move_state != other.move_state
            || self.slot_flags != other.slot_flags
            || self.heading() != other.heading()
    }
}

/// The client's predicted state at a step, kept for reconciliation (§4.3.2).
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct PredictedState {
    /// Predicted location.
    pub location: Location,
    /// Predicted heading (Tessera's convention).
    pub heading: f32,
}

#[derive(Debug, Clone, Copy, PartialEq)]
struct StepRecord {
    seq: u32,
    input: StepInput,
    predicted: Option<PredictedState>,
    /// An earlier correction replaced this step's prediction and could not re-base it (no
    /// prediction at the corrected step, or another region): its own ack is not compared
    /// again, so it does not correct the player a second time (§4.3.2 item 3).
    superseded: bool,
}

/// The tolerance of an acknowledgement (§4.3.2 item 2); [`Self::default`] is the spec's.
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct ReconcileTolerance {
    /// Largest accepted distance, metres (spec: 0.15).
    pub distance_m: f64,
    /// Largest accepted difference of the quantized headings, in heading units (spec: 1).
    pub heading_units: u8,
}

impl Default for ReconcileTolerance {
    /// 0.15 m and one quantized heading unit (§4.3.2 item 2).
    fn default() -> Self {
        Self {
            distance_m: RECONCILE_DISTANCE_M,
            heading_units: RECONCILE_HEADING_UNITS,
        }
    }
}

/// The authoritative state of a `0xCC` (§4.3.1), as a reconciliation applies it.
#[derive(Debug, Clone, PartialEq)]
pub struct AuthoritativeState {
    /// The authoritative location.
    pub location: Location,
    /// Its anchor, for an interior cell.
    pub anchor: Option<Anchor>,
    /// The authoritative heading (Tessera's convention).
    pub heading: f32,
    /// The linear velocity components of the event (zero when absent).
    pub velocity: [f32; 3],
    /// The angular velocity components of the event (zero when absent).
    pub angular_velocity: [f32; 3],
    /// The movement mode byte.
    pub movement_mode: u8,
    /// The state schema of the boolean and enum values (`official-messages-world.md`
    /// §4.3.1).
    pub state_schema_id: u32,
    /// The boolean state values to apply, in schema order.
    pub bool_states: Vec<bool>,
    /// The enum state values to apply, in schema order.
    pub enum_states: Vec<u32>,
}

/// The outcome of reconciling a `0xCC` (§4.3.2).
#[derive(Debug, Clone, PartialEq)]
pub enum Reconciliation {
    /// The prediction at the acknowledged step was within tolerance (item 2).
    Accepted {
        /// The acknowledged step.
        ack: u32,
    },
    /// Reset to the authoritative state and re-apply the steps after `ack` (item 3).
    Corrected {
        /// The acknowledged step.
        ack: u32,
        /// The authoritative state.
        state: AuthoritativeState,
        /// The unacknowledged steps to re-apply, oldest first.
        replay: Vec<StepInput>,
        /// The state after re-applying `replay` from the authoritative state: the newest
        /// unacknowledged step's prediction moved by the correction at `ack` (Tessera's
        /// reading of the replay of §4.3.2 item 3: the same steps from a shifted start
        /// cover the same displacement). `None` when nothing is replayed or the steps'
        /// predictions could not be moved; the authoritative state applies then.
        replayed: Option<PredictedState>,
        /// The ack was outside the retained step history (ahead of the newest step, or
        /// older than the retained steps): the history was cleared and nothing is replayed
        /// (item 3).
        outside_history: bool,
    },
    /// A forced position or portal placement sync (`sync_flags` bit 0, item 1): the
    /// history was cleared and the state is adopted at once.
    Forced {
        /// The teleport reason code (enum `0x2300038B`).
        reason: u32,
        /// The authoritative state.
        state: AuthoritativeState,
    },
}

impl Reconciliation {
    /// The state to adopt, unless the prediction was accepted.
    #[must_use]
    pub fn state(&self) -> Option<&AuthoritativeState> {
        match self {
            Self::Accepted { .. } => None,
            Self::Corrected { state, .. } | Self::Forced { state, .. } => Some(state),
        }
    }

    /// The state the player takes: the replayed state of a correction when there is one
    /// ([`Self::Corrected`]'s `replayed`), else the authoritative state. `None` when the
    /// prediction was accepted.
    #[must_use]
    pub fn placement(&self) -> Option<(Location, Option<Anchor>, f32, [f32; 3])> {
        match self {
            Self::Accepted { .. } => None,
            Self::Corrected {
                state,
                replayed: Some(r),
                ..
            } => Some((r.location, state.anchor, r.heading, state.velocity)),
            Self::Corrected { state, .. } | Self::Forced { state, .. } => {
                Some((state.location, state.anchor, state.heading, state.velocity))
            }
        }
    }
}

/// Whether step `a` comes after step `b` under 32-bit serial-number arithmetic
/// (RFC 1982 style; engineering choice for the wrap of the `u32` counter, spec silent).
#[must_use]
pub fn step_after(a: u32, b: u32) -> bool {
    a != b && a.wrapping_sub(b) < 0x8000_0000
}

/// The local player's movement step window and history (§4.1–§4.3).
#[derive(Debug, Clone)]
pub struct MovementSender {
    entity_id: u64,
    seq: u32,
    /// The last [`WINDOW_STEPS`] steps `(seq, input)`, oldest first: the `0x24` window.
    ring: VecDeque<(u32, StepInput)>,
    /// The last [`STEP_HISTORY_CAP`] steps with their predictions, for reconciliation.
    history: VecDeque<StepRecord>,
    last_sent: Option<StepInput>,
    last_step_time: f64,
}

impl MovementSender {
    /// A sender for the local player `entity_id`, before any step (`step_seq = 0`).
    #[must_use]
    pub fn new(entity_id: u64) -> Self {
        Self {
            entity_id,
            seq: 0,
            ring: VecDeque::with_capacity(WINDOW_STEPS),
            history: VecDeque::with_capacity(STEP_HISTORY_CAP),
            last_sent: None,
            last_step_time: f64::NEG_INFINITY,
        }
    }

    /// The newest step sequence number.
    #[must_use]
    pub fn sequence(&self) -> u32 {
        self.seq
    }

    /// Steps kept for reconciliation (at most [`STEP_HISTORY_CAP`]).
    #[must_use]
    pub fn history_len(&self) -> usize {
        self.history.len()
    }

    /// Feeds the connector's input of this frame facing `heading` ([`MoveStep::from_input`]).
    pub fn update(
        &mut self,
        input: MovementInput,
        now: f64,
        stamp: u8,
        predicted: Option<PredictedState>,
    ) -> Option<ClientPhysicsSync> {
        let heading = predicted.map_or(0.0, |p| p.heading);
        self.step(MoveStep::from_input(input, heading), now, stamp, predicted)
    }

    /// Feeds the step of this frame. A new step is made, and its `0x24` returned, when the
    /// step changed (`move_state`, jump or walk, entity heading: [`StepInput::changed_from`])
    /// or, while moving ([`StepInput::is_moving`]), when the next 20 Hz tick is due (§4.1
    /// item 1, §4.2 item 1); the stop of a movement is such a change, so it goes at once.
    /// Periodic steps keep an exact 50 ms grid: the tick time advances by
    /// [`STEP_INTERVAL_SEC`] and is reset to `now` only when the caller fell more than one
    /// interval behind (no burst of catch-up steps); a change starts a new grid at `now`.
    /// `stamp` is the current teleport stamp echoed in `header_byte` (§4.2 item 3);
    /// `predicted` is the client's predicted state after the step, kept for
    /// reconciliation.
    pub fn step(
        &mut self,
        step: MoveStep,
        now: f64,
        stamp: u8,
        predicted: Option<PredictedState>,
    ) -> Option<ClientPhysicsSync> {
        let step = StepInput::from_step(step);
        let changed = self.last_sent.is_none_or(|last| step.changed_from(&last));
        let due = step.is_moving()
            && now - self.last_step_time >= STEP_INTERVAL_SEC - STEP_TIME_EPSILON_SEC;
        if !changed && !due {
            return None;
        }
        if changed {
            self.last_step_time = now;
        } else {
            self.last_step_time += STEP_INTERVAL_SEC;
            if now - self.last_step_time >= STEP_INTERVAL_SEC - STEP_TIME_EPSILON_SEC {
                self.last_step_time = now;
            }
        }
        self.seq = self.seq.wrapping_add(1);
        if self.ring.len() >= WINDOW_STEPS {
            self.ring.pop_front();
        }
        self.ring.push_back((self.seq, step));
        if self.history.len() >= STEP_HISTORY_CAP {
            self.history.pop_front();
        }
        self.history.push_back(StepRecord {
            seq: self.seq,
            input: step,
            predicted,
            superseded: false,
        });
        self.last_sent = Some(step);
        Some(self.window(stamp))
    }

    /// The `0x24` of the current window (§4.1 item 2, §4.2 item 2): slot `k` carries step
    /// `max(seq − 3 + k, 0)` from the ring of the last four steps. A slot for step `0`
    /// (before the first step) or for a step not in the ring is [`StepInput::default`].
    /// `slot_flags` carry only the jump and walk bits; the `OPEN` bits and `trailer_u32`
    /// are sent as `0` (`official-messages-world.md` §5.5).
    #[must_use]
    pub fn window(&self, stamp: u8) -> ClientPhysicsSync {
        let mut move_state = [MOVE_STATE_STATIONARY; 4];
        let mut slot_flags = [0u8; 4];
        let mut axes = [[0u8; 4]; 3];
        for k in 0..4u32 {
            let seq = self.seq.saturating_sub(3 - k);
            let step = if seq == 0 {
                StepInput::default()
            } else {
                self.ring
                    .iter()
                    .find(|(s, _)| *s == seq)
                    .map_or_else(StepInput::default, |(_, i)| *i)
            };
            let i = k as usize;
            if let Some(m) = move_state.get_mut(i) {
                *m = step.move_state;
            }
            if let Some(f) = slot_flags.get_mut(i) {
                *f = step.slot_flags;
            }
            for (axis, value) in axes.iter_mut().zip(step.axes) {
                if let Some(slot) = axis.get_mut(i) {
                    *slot = value;
                }
            }
        }
        ClientPhysicsSync::new_base(
            self.entity_id,
            stamp,
            self.seq,
            move_state,
            slot_flags,
            axes,
        )
    }

    /// Drops every unacknowledged step after a teleport was accepted (§4.5 item 3). The
    /// `0x24` window is kept: it reports the last steps made, acknowledged or not.
    pub fn discard_history(&mut self) {
        self.history.clear();
    }

    /// Reconciles a `0xCC` (§4.3.2). Step numbers are compared as serial numbers
    /// ([`step_after`]), so the wrap of the counter is handled.
    ///
    /// - `sync_flags` bit 0 set (item 1): a forced sync; the history is cleared and the
    ///   state adopted ([`Reconciliation::Forced`]).
    /// - The ack is in the history (item 2): steps up to it are acknowledged; within
    ///   0.15 m (same region) and one heading unit of the prediction it is accepted, else
    ///   corrected with the later steps to replay (item 3). A correction moves the kept
    ///   predictions by the same offset, so the acks of the replayed steps compare against
    ///   the corrected path; a kept step whose prediction cannot be moved is not compared
    ///   again (its ack is accepted).
    /// - The ack is ahead of the newest step (item 3): the history and window are cleared,
    ///   the newest step becomes the ack, and the state is adopted (the `0xCC` that places
    ///   the player at world entry, `official-messages-world.md` §4.3.3).
    /// - The ack is behind the retained steps (item 3): the history is cleared and the
    ///   state adopted; the step counter is not moved back. Item 3 says to set
    ///   `newest_step_seq = ack_step_seq` here too, while §4.1 item 1 keeps the counter
    ///   monotonic and a reordered `0xCC` would move it back: engineering choice, question
    ///   in `docs/m8-remediation.md` §15.
    ///
    /// # Errors
    ///
    /// [`PlayError::BadCoordinate`] when the authoritative position is not a valid
    /// location (the history is left as it was).
    pub fn reconcile(
        &mut self,
        placement: &PortalPositionSync,
        tolerance: ReconcileTolerance,
    ) -> Result<Reconciliation, PlayError> {
        let (location, anchor) = wire_location(&placement.position)
            .ok_or(PlayError::BadCoordinate("authoritative position"))?;
        let state = AuthoritativeState {
            location,
            anchor,
            heading: wire_heading(placement.heading_raw),
            velocity: placement.linear_velocity.map(|v| v.unwrap_or(0.0)),
            angular_velocity: placement.angular_velocity.map(|v| v.unwrap_or(0.0)),
            movement_mode: placement.movement_mode,
            state_schema_id: placement.state_schema_id,
            bool_states: placement.bool_states.clone(),
            enum_states: placement.enum_states.clone(),
        };
        let Some(ack) = placement.ack_step_seq() else {
            self.history.clear();
            return Ok(Reconciliation::Forced {
                reason: placement.sequence_or_reason,
                state,
            });
        };
        let record = self.history.iter().find(|r| r.seq == ack).copied();
        let Some(record) = record else {
            if step_after(ack, self.seq) {
                self.seq = ack;
                self.ring.clear();
            }
            self.history.clear();
            return Ok(Reconciliation::Corrected {
                ack,
                state,
                replay: Vec::new(),
                replayed: None,
                outside_history: true,
            });
        };
        let within = record.predicted.is_some_and(|p| {
            p.location.region == location.region
                && distance(&p.location, &location) <= tolerance.distance_m
                && heading_units_apart(heading_to_wire(p.heading), placement.heading_raw)
                    <= tolerance.heading_units
        });
        // Steps up to and including `ack` are acknowledged (item 1).
        self.history.retain(|r| step_after(r.seq, ack));
        if within || record.superseded {
            // A step an earlier correction already replaced is not compared again.
            return Ok(Reconciliation::Accepted { ack });
        }
        let replay = self.history.iter().map(|r| r.input).collect();
        // The kept predictions start from the old state: they move by the correction's
        // offset (same region only), so later acks compare against the corrected path.
        let offset = record
            .predicted
            .filter(|p| p.location.region == location.region)
            .map(|p| {
                (
                    location.x - p.location.x,
                    location.y - p.location.y,
                    location.z - p.location.z,
                )
            })
            .filter(|(dx, dy, dz)| dx.is_finite() && dy.is_finite() && dz.is_finite());
        for r in &mut self.history {
            match (offset, r.predicted.as_mut()) {
                (Some((dx, dy, dz)), Some(q)) if q.location.region == location.region => {
                    q.location.x += dx;
                    q.location.y += dy;
                    q.location.z += dz;
                }
                _ => {
                    r.predicted = None;
                    r.superseded = true;
                }
            }
        }
        let replayed = self.history.back().and_then(|r| r.predicted);
        Ok(Reconciliation::Corrected {
            ack,
            state,
            replay,
            replayed,
            outside_history: false,
        })
    }
}

fn distance(a: &Location, b: &Location) -> f64 {
    let (dx, dy, dz) = (a.x - b.x, a.y - b.y, a.z - b.z);
    (dx * dx + dy * dy + dz * dz).sqrt()
}

// ---------------------------------------------------------------------------
// §4.5 Teleport stamp state machine
// ---------------------------------------------------------------------------

/// What [`TeleportSync::on_local_position`] decided for one `0xCE` with a stamp.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct StampOutcome {
    /// The stamp was newer or equal: adopt it, snap the player to the event's position and
    /// discard the unacknowledged step history.
    pub snap: bool,
    /// The placement wait ended with this event.
    pub wait_cleared: bool,
}

/// The teleport stamp synchronization and two-gate portal exit of §4.5.
///
/// Stamps come only from `0x5E` (the expected stamp, [`Self::begin_portal_space`]) and from
/// local-player `0xCE` with `header_flags` bit 3 ([`Self::on_local_position`]); a `0xCC`
/// carries none (§4.5 item 2). Portal space ends once both gates are set: `game_ready`
/// (`0x7C` or `0xC7`, [`Self::on_server_ready`]) and `client_ready` (the stamp has reached
/// the expected one and loading completed or timed out, [`Self::try_exit_portal_space`]).
///
/// At world entry the entity stamp is unknown ([`Self::default`], [`Self::enter_world`]): the
/// spec gives it no initial value. Until a stamp is known, the first `0x5E` supplies the
/// expected stamp and the client waits for its placement, and the first local-player `0xCE`
/// with a stamp is taken as current without the modulo-256 test (engineering choice,
/// question in `docs/m8-remediation.md` §15).
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub struct TeleportSync {
    entity_stamp: u8,
    /// `entity_stamp` came from the server (or [`Self::new`]).
    known: bool,
    waiting: bool,
    expected: u8,
    in_portal_space: bool,
    game_ready: bool,
    /// The client gate was seen set (stamp reached, loading done or timed out) while the
    /// server gate was still clear.
    client_ready: bool,
    /// A `0x5E` started the current portal space.
    announced: bool,
    scene_id: Option<u32>,
}

impl TeleportSync {
    /// A state with `entity_stamp = stamp` (known), outside portal space.
    #[must_use]
    pub fn new(stamp: u8) -> Self {
        Self {
            entity_stamp: stamp,
            known: true,
            expected: stamp,
            ..Self::default()
        }
    }

    /// Whether the entity stamp is known (a stamp came from the server).
    #[must_use]
    pub fn stamp_known(&self) -> bool {
        self.known
    }

    /// The local player's current stamp (echoed in `0x24` `header_byte`); `0` while it is
    /// not known (no `0x24` goes out then: the first portal space has not ended).
    #[must_use]
    pub fn stamp(&self) -> u8 {
        self.entity_stamp
    }

    /// The expected stamp of the current or last portal space.
    #[must_use]
    pub fn expected_stamp(&self) -> u8 {
        self.expected
    }

    /// The `scene_id` of the last `0x5E`, while its portal space lasts.
    #[must_use]
    pub fn scene_id(&self) -> Option<u32> {
        self.scene_id
    }

    /// Whether the client waits for the placement carrying the expected stamp.
    #[must_use]
    pub fn waiting(&self) -> bool {
        self.waiting
    }

    /// Whether the server gate (`game_ready`) is set.
    #[must_use]
    pub fn game_ready(&self) -> bool {
        self.game_ready
    }

    /// Whether the client gate (`client_ready`) was set in this portal space: the exit
    /// then waits only for the server gate (no loading time-out is pending).
    #[must_use]
    pub fn client_ready(&self) -> bool {
        self.client_ready
    }

    /// Whether the client is in portal space (local steps suspended, §4.5 item 2).
    #[must_use]
    pub fn in_portal_space(&self) -> bool {
        self.in_portal_space
    }

    /// The client enters portal space at world entry, before any `0x5E` (the session's
    /// first placement): both gates clear, no expected stamp yet. Engineering choice: no
    /// `0x24` goes out before the first portal exit (§11 item 1).
    pub fn enter_world(&mut self) {
        *self = Self {
            in_portal_space: true,
            known: self.known,
            ..Self::new(self.entity_stamp)
        };
    }

    /// `0x5E` (`enter_portal_space`, item 2): a portal-space wait begins with
    /// `expected_stamp`; both gates are cleared. The client waits when its stamp has not
    /// yet reached the expected one (difference non-zero and below 128).
    pub fn begin_portal_space(&mut self, expected_stamp: u8) {
        self.in_portal_space = true;
        self.game_ready = false;
        self.client_ready = false;
        self.announced = true;
        self.expected = expected_stamp;
        if !self.known {
            // No stamp yet: the placement carrying this one is awaited.
            self.waiting = true;
            return;
        }
        let d = expected_stamp.wrapping_sub(self.entity_stamp);
        self.waiting = d != 0 && d < 0x80;
    }

    /// [`Self::begin_portal_space`] for a decoded `0x5E`, keeping its `scene_id`.
    pub fn on_enter_portal_space(&mut self, msg: &EnterPortalSpace) {
        self.begin_portal_space(msg.stamp);
        self.scene_id = Some(msg.scene_id);
    }

    /// Item 4: `0x7C` (`login_finished`) or `0xC7` (`portal_transition_complete`) sets the
    /// server gate.
    pub fn on_server_ready(&mut self) {
        self.game_ready = true;
    }

    /// Item 3: a `0xCE` for the local player with `header_flags` bit 3 set, carrying
    /// `stamp` in `seq_byte`. A newer or equal stamp becomes the current stamp; before any
    /// `0x5E` of the current portal space it becomes the expected stamp too (item 2). The
    /// wait ends once the current stamp reaches or passes the expected one.
    pub fn on_local_position(&mut self, stamp: u8) -> StampOutcome {
        // The first stamp is taken as it comes (no current stamp to compare with).
        let snap = !self.known || stamp_newer_or_equal(stamp, self.entity_stamp);
        if snap {
            self.entity_stamp = stamp;
            self.known = true;
            if !self.announced {
                self.expected = stamp;
            }
        }
        let wait_cleared = self.waiting && stamp_newer_or_equal(self.entity_stamp, self.expected);
        if wait_cleared {
            self.waiting = false;
        }
        StampOutcome { snap, wait_cleared }
    }

    /// Item 4: the client gate is set once the stamp has reached the expected one and the
    /// loading completed (`loading_done`) or the 30 s loading timeout expired
    /// (`timed_out`); once the server gate is set too, portal space ends. Returns `true`
    /// exactly once, when `0xF4` (`portal_ready_ack`) must be sent and `0x24` resumes.
    /// Once set, the client gate stays set for this portal space ([`Self::client_ready`]).
    pub fn try_exit_portal_space(&mut self, loading_done: bool, timed_out: bool) -> bool {
        if !self.in_portal_space {
            return false;
        }
        if !self.waiting && (loading_done || timed_out) {
            self.client_ready = true;
        }
        if self.game_ready && self.client_ready {
            self.in_portal_space = false;
            self.client_ready = false;
            self.announced = false;
            self.scene_id = None;
            return true;
        }
        false
    }
}

// ---------------------------------------------------------------------------
// §4.4.2 0xBF
// ---------------------------------------------------------------------------

/// Event `0xBF` (`server_set_position`, §4.4.2): replaces an entity's movement state.
#[derive(Debug, Clone, PartialEq)]
pub struct ServerSetPosition {
    /// The presence bitmask (3-tier VLE). Non-zero gates `position`, bit 2 gates
    /// `velocity`; the other bits are OPEN and ignored.
    pub state_flags: u32,
    /// Full wire `Position`, present iff `state_flags != 0`.
    pub position: Option<WirePosition>,
    /// Velocity `(x, y, z)` in m/s (on the wire in the order x, z, y), present iff
    /// `state_flags` bit 2 is set.
    pub velocity: Option<[f32; 3]>,
    /// The trailing flags word (bits OPEN, ignored).
    pub set_pos_flags: u32,
}

impl ServerSetPosition {
    /// Encodes the inner payload. Presence follows `state_flags`.
    ///
    /// # Errors
    ///
    /// [`PlayError::BadCoordinate`] when a present field is missing or out of bounds, or a
    /// wire error.
    pub fn encode(&self, writer: &mut WireWriter) -> Result<(), PlayError> {
        writer.write_base_flags();
        writer.write_vle3(self.state_flags)?;
        if self.state_flags != 0 {
            let pos = self.position.ok_or(PlayError::BadCoordinate("position"))?;
            check_position(&pos)?;
            pos.encode_wire(writer);
            if self.state_flags & 0x04 != 0 {
                let [x, y, z] = self.velocity.ok_or(PlayError::BadCoordinate("velocity"))?;
                writer.write_f32_le(x);
                writer.write_f32_le(z);
                writer.write_f32_le(y);
            }
        }
        writer.write_u32_le(self.set_pos_flags);
        Ok(())
    }

    /// Decodes the inner payload with the bounds of §4.4.2 and §9.1 rule 4.
    ///
    /// # Errors
    ///
    /// [`PlayError`] on truncation or a coordinate out of bounds.
    pub fn decode(reader: &mut WireReader<'_>) -> Result<Self, PlayError> {
        reader.read_base_flags()?;
        let state_flags = reader.read_vle3()?;
        let (position, velocity) = if state_flags != 0 {
            let pos = WirePosition::decode_wire(reader)?;
            check_position(&pos)?;
            let vel = if state_flags & 0x04 != 0 {
                let x = reader.read_f32_le()?;
                let z = reader.read_f32_le()?;
                let y = reader.read_f32_le()?;
                for (f, v) in [("vel_x", x), ("vel_y", y), ("vel_z", z)] {
                    check_finite(f, v)?;
                }
                Some([x, y, z])
            } else {
                None
            };
            (Some(pos), vel)
        } else {
            (None, None)
        };
        let set_pos_flags = reader.read_u32_le()?;
        Ok(Self {
            state_flags,
            position,
            velocity,
            set_pos_flags,
        })
    }

    /// Decodes a standalone inner payload.
    ///
    /// # Errors
    ///
    /// As [`Self::decode`], plus trailing bytes.
    pub fn from_bytes(bytes: &[u8]) -> Result<Self, PlayError> {
        let mut r = WireReader::new(bytes);
        let m = Self::decode(&mut r)?;
        r.finish()?;
        Ok(m)
    }

    /// Encodes to a standalone inner payload.
    ///
    /// # Errors
    ///
    /// As [`Self::encode`].
    pub fn to_bytes(&self) -> Result<Vec<u8>, PlayError> {
        let mut w = WireWriter::new();
        self.encode(&mut w)?;
        Ok(w.into_vec())
    }
}

fn check_position(pos: &WirePosition) -> Result<(), PlayError> {
    for (f, v) in ["pos_x", "pos_y", "pos_z"].into_iter().zip(pos.offset) {
        require_world_coordinate(f, v)?;
    }
    for v in pos.orientation {
        check_finite("orientation", v)?;
    }
    Ok(())
}

// ---------------------------------------------------------------------------
// §4.6.1 0x59
// ---------------------------------------------------------------------------

/// `0x59` `path_type` `1`: a linear polyline path (§4.6.1).
pub const PATH_TYPE_LINEAR: u32 = 1;
/// `0x59` `path_type` `2`: a smooth spline path (§4.6.1).
pub const PATH_TYPE_SPLINE: u32 = 2;
/// `0x59` `path_type` `3`: a looping (patrol) path (§4.6.1).
pub const PATH_TYPE_LOOP: u32 = 3;

/// Event `0x59` (`entity_spline_path`, §4.6.1): move an entity along waypoints.
#[derive(Debug, Clone, PartialEq)]
pub struct EntitySplinePath {
    /// Interpolation mode: [`PATH_TYPE_LINEAR`], [`PATH_TYPE_SPLINE`] or
    /// [`PATH_TYPE_LOOP`]; other values decode but have no defined meaning.
    pub path_type: u32,
    /// The moving entity.
    pub entity_id: u64,
    /// `path_flags`; bits 0 and 1 follow the presence of `waypoints` and `speed_param`
    /// when encoding, other bits are kept as given.
    pub path_flags: u16,
    /// Movement speed along the path in metres per second (bit 1).
    pub speed_param: Option<f32>,
    /// Waypoints (bit 0), full wire positions.
    pub waypoints: Option<Vec<WirePosition>>,
}

impl EntitySplinePath {
    /// Encodes the inner payload.
    ///
    /// # Errors
    ///
    /// [`PlayError::TooMany`] over [`MAX_WAYPOINTS`].
    pub fn encode(&self, writer: &mut WireWriter) -> Result<(), PlayError> {
        let mut flags = self.path_flags & !0x0003;
        if self.waypoints.is_some() {
            flags |= 0x0001;
        }
        if self.speed_param.is_some() {
            flags |= 0x0002;
        }
        writer.write_base_flags();
        writer.write_u32_le(self.path_type);
        writer.write_u64_le(self.entity_id);
        writer.write_u16_le(flags);
        if let Some(s) = self.speed_param {
            writer.write_f32_le(s);
        }
        if let Some(points) = &self.waypoints {
            if points.len() > MAX_WAYPOINTS {
                return Err(PlayError::TooMany {
                    what: "path waypoints",
                    max: MAX_WAYPOINTS,
                });
            }
            // Bounded by MAX_WAYPOINTS.
            writer.write_u32_le(points.len() as u32);
            for p in points {
                p.encode_wire(writer);
            }
        }
        Ok(())
    }

    /// Decodes the inner payload (§4.6.1, §9.1 rules 1 and 4).
    ///
    /// # Errors
    ///
    /// [`PlayError`] on truncation, a count over the remaining bytes or
    /// [`MAX_WAYPOINTS`], or a non-finite waypoint.
    pub fn decode(reader: &mut WireReader<'_>) -> Result<Self, PlayError> {
        reader.read_base_flags()?;
        let path_type = reader.read_u32_le()?;
        let entity_id = reader.read_u64_le()?;
        let path_flags = reader.read_u16_le()?;
        let speed_param = if path_flags & 0x0002 != 0 {
            let s = reader.read_f32_le()?;
            check_finite("speed_param", s)?;
            Some(s)
        } else {
            None
        };
        let waypoints = if path_flags & 0x0001 != 0 {
            let count = reader.read_u32_le()? as usize;
            reader.ensure_count(count, MIN_WAYPOINT_BYTES)?;
            if count > MAX_WAYPOINTS {
                return Err(PlayError::TooMany {
                    what: "path waypoints",
                    max: MAX_WAYPOINTS,
                });
            }
            let mut points = Vec::with_capacity(count);
            for _ in 0..count {
                let p = WirePosition::decode_wire(reader)?;
                for v in p.offset.iter().chain(p.orientation.iter()) {
                    check_finite("waypoint", *v)?;
                }
                points.push(p);
            }
            Some(points)
        } else {
            None
        };
        Ok(Self {
            path_type,
            entity_id,
            path_flags,
            speed_param,
            waypoints,
        })
    }

    /// Decodes a standalone inner payload.
    ///
    /// # Errors
    ///
    /// As [`Self::decode`], plus trailing bytes.
    pub fn from_bytes(bytes: &[u8]) -> Result<Self, PlayError> {
        let mut r = WireReader::new(bytes);
        let m = Self::decode(&mut r)?;
        r.finish()?;
        Ok(m)
    }

    /// Encodes to a standalone inner payload.
    ///
    /// # Errors
    ///
    /// As [`Self::encode`].
    pub fn to_bytes(&self) -> Result<Vec<u8>, PlayError> {
        let mut w = WireWriter::new();
        self.encode(&mut w)?;
        Ok(w.into_vec())
    }

    /// Whether `path_type` is one §4.6.1 defines (`1..=3`).
    #[must_use]
    pub fn has_defined_type(&self) -> bool {
        (PATH_TYPE_LINEAR..=PATH_TYPE_LOOP).contains(&self.path_type)
    }

    /// The locations of the waypoints that are valid positions, in order.
    #[must_use]
    pub fn waypoint_locations(&self) -> Vec<Location> {
        self.waypoints
            .iter()
            .flatten()
            .filter_map(|p| wire_location(p).map(|(l, _)| l))
            .collect()
    }
}

// ---------------------------------------------------------------------------
// §4.6.2 0xED
// ---------------------------------------------------------------------------

/// Event `0xED` (`entity_motion_state`, §4.6.2) for the entity of the blob's
/// `recipientId`.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum MotionControllerUpdate {
    /// `has_controller = 0`: no controller.
    NoController,
    /// `has_controller = 1` with an all-zero type ID: the controller is cleared.
    Cleared,
    /// A typed controller. The five type IDs are OPEN, so the body is skipped and the
    /// update ignored (§4.6.2 receiver rule); the raw body is kept.
    Typed {
        /// The 16-byte controller type ID.
        type_id: [u8; 16],
        /// The skipped body bytes.
        body: Vec<u8>,
    },
}

impl MotionControllerUpdate {
    /// Encodes the inner payload.
    pub fn encode(&self, writer: &mut WireWriter) {
        writer.write_base_flags();
        match self {
            Self::NoController => writer.write_bool8(false),
            Self::Cleared => {
                writer.write_bool8(true);
                writer.write_bytes(&[0u8; 16]);
            }
            Self::Typed { type_id, body } => {
                writer.write_bool8(true);
                writer.write_bytes(type_id);
                writer.write_bytes(body);
            }
        }
    }

    /// Decodes a standalone inner payload (the bounded event slice).
    ///
    /// # Errors
    ///
    /// [`PlayError`] on truncation, a bad `bool8`, or bytes after a cleared or absent
    /// controller.
    pub fn from_bytes(bytes: &[u8]) -> Result<Self, PlayError> {
        let mut r = WireReader::new(bytes);
        r.read_base_flags()?;
        let msg = if r.read_bool8()? {
            let raw = r.read_bytes(16)?;
            let mut type_id = [0u8; 16];
            type_id.copy_from_slice(raw);
            if type_id == [0u8; 16] {
                Self::Cleared
            } else {
                let body = r.remaining_slice().to_vec();
                let _ = r.read_bytes(body.len())?;
                Self::Typed { type_id, body }
            }
        } else {
            Self::NoController
        };
        r.finish()?;
        Ok(msg)
    }

    /// Encodes to a standalone inner payload.
    #[must_use]
    pub fn to_bytes(&self) -> Vec<u8> {
        let mut w = WireWriter::new();
        self.encode(&mut w);
        w.into_vec()
    }
}

// ---------------------------------------------------------------------------
// §4.6.3 0xBD
// ---------------------------------------------------------------------------

/// Smallest wire size of one `ActionLayerEntry`: Compact DID 2, `VariableU32` 1,
/// `ActionPayload` 11, bitfield 1.
const MIN_LAYER_BYTES: usize = 15;

/// Checks the play-layer bounds of an `0xBD` (§4.6.3): at most [`MAX_ANIM_LAYERS`] layers,
/// and every layer's bitfield holds exactly `ceil(bit_count / 8)` bytes
/// (`official-messages-world.md` §2.7.1; [`crate::world::WireBitfield::encode`] would pad or
/// cut them).
///
/// # Errors
///
/// [`PlayError::TooMany`] or [`PlayError::BitfieldMismatch`].
pub fn check_anim_layers(state: &EntityAnimLayerState) -> Result<(), PlayError> {
    if state.layers.len() > MAX_ANIM_LAYERS {
        return Err(PlayError::TooMany {
            what: "animation layers",
            max: MAX_ANIM_LAYERS,
        });
    }
    for layer in &state.layers {
        let b = &layer.bitfield;
        if b.bytes.len() != (b.bit_count as usize).div_ceil(8) {
            return Err(PlayError::BitfieldMismatch {
                bit_count: b.bit_count,
                bytes: b.bytes.len(),
            });
        }
    }
    Ok(())
}

/// Encodes an `0xBD` inner payload with [`EntityAnimLayerState::encode`] after
/// [`check_anim_layers`].
///
/// # Errors
///
/// As [`check_anim_layers`], or a wire error.
pub fn encode_anim_layers(state: &EntityAnimLayerState) -> Result<Vec<u8>, PlayError> {
    check_anim_layers(state)?;
    Ok(state.to_bytes()?)
}

/// Decodes a standalone `0xBD` inner payload (§4.6.3, §9.1 rule 1); action property tables
/// use `schema`. The layer count is checked against the remaining bytes before anything is
/// allocated, then against [`MAX_ANIM_LAYERS`].
///
/// # Errors
///
/// [`PlayError`] on truncation, a count over the remaining bytes or [`MAX_ANIM_LAYERS`],
/// a bad action payload, or trailing bytes.
pub fn decode_anim_layers(
    bytes: &[u8],
    schema: &dyn PropertySchemaLookup,
) -> Result<EntityAnimLayerState, PlayError> {
    let mut r = WireReader::new(bytes);
    r.read_base_flags()?;
    let count = r.read_vle3()? as usize;
    r.ensure_count(count, MIN_LAYER_BYTES)?;
    if count > MAX_ANIM_LAYERS {
        return Err(PlayError::TooMany {
            what: "animation layers",
            max: MAX_ANIM_LAYERS,
        });
    }
    let state = EntityAnimLayerState::from_bytes(bytes, schema)?;
    check_anim_layers(&state)?;
    Ok(state)
}
