// SPDX-License-Identifier: GPL-3.0-or-later
//! Player movement and remote entity motion (`docs/specs/official-messages-play.md` §4).
//!
//! - [`MovementSender`]: the monotonic step counter, the 4-step rolling window of `0x24`
//!   (§4.1–§4.2) and the step history that `0xCC` acknowledgements reconcile against
//!   (§4.3.2).
//! - [`TeleportSync`]: the modulo-256 teleport stamp state machine (§4.5).
//! - [`ServerSetPosition`] (`0xBF`, §4.4.2), [`EntitySplinePath`] (`0x59`, §4.6.1),
//!   [`MotionControllerUpdate`] (`0xED`, §4.6.2) and [`AnimLayerState`] (`0xBD`, §4.6.3).
//!
//! The wire layouts of `0x24`, `0xCC` and `0xCE` live in [`crate::world`]
//! (`official-messages-world.md` §4.3.1, §5.4, §5.5); this module uses them.

use std::collections::VecDeque;

use tessera_net::{Anchor, Landblock, Location, RegionId, Rotation};

use crate::play::PlayError;
use crate::world::{
    ActionPayload, ClientMoveCommand, CompactDid, EncodedPropertyValue, PortalPlacement,
    PropertySchemaLookup, PropertyType, WirePosition, WireReader, WireWriter, decode_heading_u8,
};

/// Event ID of the client movement step event (`client_physics_sync`, §4.2).
pub const OPCODE_CLIENT_PHYSICS_SYNC: u32 = ClientMoveCommand::EVENT_ID;
/// Event ID of the full movement-state setter (`server_set_position`, §4.4.2).
pub const OPCODE_SERVER_SET_POSITION: u32 = 0x0000_00BF;
/// Event ID of the entity path event (`entity_spline_path`, §4.6.1).
pub const OPCODE_ENTITY_SPLINE_PATH: u32 = 0x0000_0059;
/// Event ID of the motion controller event (`entity_motion_state`, §4.6.2).
pub const OPCODE_ENTITY_MOTION_STATE: u32 = 0x0000_00ED;
/// Event ID of the animation layer event (`entity_anim_layer_state`, §4.6.3).
pub const OPCODE_ENTITY_ANIM_LAYER_STATE: u32 = 0x0000_00BD;

/// Nominal step interval while moving: 20 Hz (§4.2 item 1).
pub const STEP_INTERVAL_SEC: f64 = 0.05;
/// Steps of history kept for reconciliation. The spec gives no capacity (it was removed
/// as client-internal); engineering choice.
pub const STEP_HISTORY_CAP: usize = 256;
/// Largest coordinate magnitude of `0xBF` and `0xCE`, metres (§4.4.2, §9.1 rule 4).
pub const MAX_COORDINATE_M: f32 = 100_000.0;
/// Most waypoints accepted in one `0x59` path (engineering cap; the spec bounds the count
/// only by the remaining bytes).
pub const MAX_WAYPOINTS: usize = 1024;
/// Most animation layers accepted in one `0xBD` event (engineering cap).
pub const MAX_ANIM_LAYERS: usize = 256;

// ---------------------------------------------------------------------------
// Wire scalars and coordinates
// ---------------------------------------------------------------------------

/// Encodes a value in `[-1, 1]` as a `SignedNormalizedByte` (`official-messages-world.md`
/// §2.3.1): clamp, multiply by 127, round toward zero. NaN encodes as `0`.
#[must_use]
pub fn encode_signed_normalized(value: f32) -> i8 {
    if !value.is_finite() {
        return if value.is_nan() {
            0
        } else if value > 0.0 {
            127
        } else {
            -127
        };
    }
    let scaled = (value.clamp(-1.0, 1.0) * 127.0).trunc();
    // Within [-127, 127] after the clamp.
    scaled as i8
}

/// Decodes a `SignedNormalizedByte`: `clamp(b / 127, -1, 1)` (§2.3.1).
#[must_use]
pub fn decode_signed_normalized(b: i8) -> f32 {
    (f32::from(b) / 127.0).clamp(-1.0, 1.0)
}

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
/// heading, reading the wire angle as the yaw ψ of `coordinates.md` §11.3 (0 = facing
/// north, counter-clockwise). See the OPEN on this reading in the module's report.
#[must_use]
pub fn wire_heading(raw: u8) -> f32 {
    wrap_pi(decode_heading_u8(raw) + std::f32::consts::FRAC_PI_2)
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

fn angle_diff(a: f32, b: f32) -> f32 {
    wrap_pi(a - b).abs()
}

fn check_world_coordinate(field: &'static str, v: f32) -> Result<(), PlayError> {
    if v.is_finite() && (-MAX_COORDINATE_M..=MAX_COORDINATE_M).contains(&v) {
        Ok(())
    } else {
        Err(PlayError::BadCoordinate(field))
    }
}

fn check_finite(field: &'static str, v: f32) -> Result<(), PlayError> {
    if v.is_finite() {
        Ok(())
    } else {
        Err(PlayError::BadCoordinate(field))
    }
}

// ---------------------------------------------------------------------------
// §4.1–§4.2 Movement steps and 0x24
// ---------------------------------------------------------------------------

/// The local player's movement input for one step: the movement state byte and the three
/// input axes of `0x24` (`official-messages-world.md` §5.5), each in `[-1, 1]`.
#[derive(Debug, Clone, Copy, PartialEq, Default)]
pub struct MovementInput {
    /// The movement state byte (`move_mode` / stance code; values from the server data).
    pub move_state: u8,
    /// Lateral / strafe axis (`axis_a`).
    pub strafe: f32,
    /// Forward / backward axis (`axis_b`).
    pub forward: f32,
    /// Yaw / turn axis (`axis_c`).
    pub turn: f32,
}

/// One quantized step as it goes on the wire.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Default)]
pub struct StepInput {
    /// The movement state byte.
    pub move_state: u8,
    /// `axis_a`, `axis_b`, `axis_c` as `SignedNormalizedByte`s.
    pub axes: [i8; 3],
}

impl StepInput {
    /// Quantizes `input`.
    #[must_use]
    pub fn from_input(input: MovementInput) -> Self {
        Self {
            move_state: input.move_state,
            axes: [
                encode_signed_normalized(input.strafe),
                encode_signed_normalized(input.forward),
                encode_signed_normalized(input.turn),
            ],
        }
    }

    /// Whether any axis is non-zero (this layer's test for "moving", §4.2 item 1).
    #[must_use]
    pub fn is_moving(&self) -> bool {
        self.axes.iter().any(|a| *a != 0)
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
}

/// Spatial tolerance of an acknowledgement (§4.3.2 item 2). The spec gives no values;
/// [`Self::default`] is an engineering choice.
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct ReconcileTolerance {
    /// Largest accepted distance, metres.
    pub distance_m: f64,
    /// Largest accepted heading difference, radians.
    pub heading_rad: f32,
}

impl Default for ReconcileTolerance {
    /// 0.5 m and two heading quanta (2 · 2π / 256).
    fn default() -> Self {
        Self {
            distance_m: 0.5,
            heading_rad: 2.0 * std::f32::consts::TAU / 256.0,
        }
    }
}

/// The outcome of reconciling a `0xCC` acknowledgement (§4.3.2).
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
        /// The authoritative location.
        location: Location,
        /// Its anchor, for an interior cell.
        anchor: Option<Anchor>,
        /// The authoritative heading (Tessera's convention).
        heading: f32,
        /// The linear velocity components of the event (zero when absent).
        velocity: [f32; 3],
        /// The unacknowledged steps to re-apply, oldest first.
        replay: Vec<StepInput>,
    },
}

/// The local player's movement step window and history (§4.1–§4.3).
#[derive(Debug, Clone)]
pub struct MovementSender {
    entity_id: u64,
    seq: u32,
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
            history: VecDeque::new(),
            last_sent: None,
            last_step_time: f64::NEG_INFINITY,
        }
    }

    /// The newest step sequence number.
    #[must_use]
    pub fn sequence(&self) -> u32 {
        self.seq
    }

    /// Steps kept for reconciliation.
    #[must_use]
    pub fn history_len(&self) -> usize {
        self.history.len()
    }

    /// Feeds the input of this frame. A new step is made, and its `0x24` returned, when the
    /// input changed or, while moving, when [`STEP_INTERVAL_SEC`] has passed since the last
    /// step (§4.1 item 1, §4.2 item 1). `stamp` is the current teleport stamp echoed in
    /// `header_byte` (§4.2 item 3); `predicted` is the client's predicted state after the
    /// step, kept for reconciliation.
    pub fn update(
        &mut self,
        input: MovementInput,
        now: f64,
        stamp: u8,
        predicted: Option<PredictedState>,
    ) -> Option<ClientMoveCommand> {
        let step = StepInput::from_input(input);
        let changed = self.last_sent != Some(step);
        let due = step.is_moving() && now - self.last_step_time >= STEP_INTERVAL_SEC;
        if !changed && !due {
            return None;
        }
        self.seq = self.seq.wrapping_add(1);
        if self.history.len() >= STEP_HISTORY_CAP {
            self.history.pop_front();
        }
        self.history.push_back(StepRecord {
            seq: self.seq,
            input: step,
            predicted,
        });
        self.last_sent = Some(step);
        self.last_step_time = now;
        Some(self.window(stamp))
    }

    /// The `0x24` of the current window (§4.1 item 2, §4.2 item 2): slots `0..=3` carry the
    /// steps `max(seq − 3 + k, 0)`; `slot_flags` bits under mask `0xEE` and `trailer_u32`
    /// are sent as `0` (`official-messages-world.md` §5.5 OPEN).
    #[must_use]
    pub fn window(&self, stamp: u8) -> ClientMoveCommand {
        let fallback = self.last_sent.unwrap_or_default();
        let mut move_state = [0u8; 4];
        let mut axes = [[0u8; 4]; 3];
        for k in 0..4u32 {
            let seq = self.seq.saturating_sub(3 - k);
            let step = self
                .history
                .iter()
                .rev()
                .find(|r| r.seq == seq)
                .map_or(fallback, |r| r.input);
            let i = k as usize;
            if let Some(m) = move_state.get_mut(i) {
                *m = step.move_state;
            }
            for (axis, value) in axes.iter_mut().zip(step.axes) {
                if let Some(slot) = axis.get_mut(i) {
                    // The i8 travels as its two's-complement byte.
                    *slot = value as u8;
                }
            }
        }
        let [axis_a, axis_b, axis_c] = axes;
        ClientMoveCommand::new_base(
            self.entity_id,
            stamp,
            self.seq,
            move_state,
            axis_a,
            axis_b,
            axis_c,
            0,
        )
    }

    /// Drops every unacknowledged step after a teleport was accepted (§4.5 item 3).
    pub fn discard_history(&mut self) {
        self.history.clear();
    }

    /// Reconciles a `0xCC` acknowledgement of `ack_step_seq` (§4.3.2).
    ///
    /// # Errors
    ///
    /// [`PlayError::BadCoordinate`] when the authoritative position is not a valid
    /// location.
    pub fn reconcile(
        &mut self,
        placement: &PortalPlacement,
        tolerance: ReconcileTolerance,
    ) -> Result<Reconciliation, PlayError> {
        let ack = placement.sequence_or_reason;
        let (location, anchor) = wire_location(&placement.position)
            .ok_or(PlayError::BadCoordinate("authoritative position"))?;
        let heading = wire_heading(placement.heading_raw);
        let predicted = self
            .history
            .iter()
            .find(|r| r.seq == ack)
            .and_then(|r| r.predicted);
        let within = predicted.is_some_and(|p| {
            p.location.region == location.region
                && distance(&p.location, &location) <= tolerance.distance_m
                && angle_diff(p.heading, heading) <= tolerance.heading_rad
        });
        // Steps up to and including `ack` are acknowledged (item 1).
        self.history.retain(|r| r.seq > ack);
        if within {
            return Ok(Reconciliation::Accepted { ack });
        }
        let replay = self.history.iter().map(|r| r.input).collect();
        // The kept predictions start from the old state; they are no longer comparable.
        for r in &mut self.history {
            r.predicted = None;
        }
        Ok(Reconciliation::Corrected {
            ack,
            location,
            anchor,
            heading,
            velocity: placement.linear_velocity,
            replay,
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
    /// The portal-space wait ended with this event.
    pub wait_cleared: bool,
}

/// The teleport stamp synchronization of §4.5.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub struct TeleportSync {
    entity_stamp: u8,
    waiting: bool,
    expected: u8,
    in_portal_space: bool,
}

impl TeleportSync {
    /// A state with `entity_stamp = stamp`, outside portal space.
    #[must_use]
    pub fn new(stamp: u8) -> Self {
        Self {
            entity_stamp: stamp,
            ..Self::default()
        }
    }

    /// The local player's current stamp (echoed in `0x24` `header_byte`).
    #[must_use]
    pub fn stamp(&self) -> u8 {
        self.entity_stamp
    }

    /// Whether the client waits for the authoritative `0xCE` of a teleport.
    #[must_use]
    pub fn waiting(&self) -> bool {
        self.waiting
    }

    /// Whether the client is in portal space (local steps suspended, §4.5 item 2).
    #[must_use]
    pub fn in_portal_space(&self) -> bool {
        self.in_portal_space
    }

    /// Item 2: a portal-space transition begins with `expected_stamp`. The client waits
    /// when its stamp has not yet reached the expected one (difference non-zero and below
    /// 128). The spec names no event that starts the transition apart from the `0xCE`
    /// that also ends it (item 3); the connector calls this when its trigger fires.
    pub fn begin_portal_space(&mut self, expected_stamp: u8) {
        self.in_portal_space = true;
        let d = expected_stamp.wrapping_sub(self.entity_stamp);
        if d != 0 && d < 0x80 {
            self.waiting = true;
            self.expected = expected_stamp;
        }
    }

    /// Item 3: a `0xCE` for the local player with `header_flags` bit 3 set, carrying
    /// `stamp` in `seq_byte`.
    pub fn on_local_position(&mut self, stamp: u8) -> StampOutcome {
        let snap = stamp_newer_or_equal(stamp, self.entity_stamp);
        if snap {
            self.entity_stamp = stamp;
        }
        let wait_cleared = self.waiting && stamp_newer_or_equal(stamp, self.expected);
        if wait_cleared {
            self.waiting = false;
        }
        StampOutcome { snap, wait_cleared }
    }

    /// Item 4: once the wait is over and loading completed (or the portal-space loading
    /// timeout expired), portal space ends; returns `true` exactly once, when `0xF4`
    /// (`portal_ready_ack`) must be sent and `0x24` resumes.
    pub fn try_exit_portal_space(&mut self, loading_done: bool, timed_out: bool) -> bool {
        if self.in_portal_space && !self.waiting && (loading_done || timed_out) {
            self.in_portal_space = false;
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
        check_world_coordinate(f, v)?;
    }
    for v in pos.orientation {
        check_finite("orientation", v)?;
    }
    Ok(())
}

// ---------------------------------------------------------------------------
// §4.6.1 0x59
// ---------------------------------------------------------------------------

/// Event `0x59` (`entity_spline_path`, §4.6.1): move an entity along waypoints.
#[derive(Debug, Clone, PartialEq)]
pub struct EntitySplinePath {
    /// Interpolation mode / path controller type ID.
    pub path_type: u32,
    /// The moving entity.
    pub entity_id: u64,
    /// `path_flags`; bits 0 and 1 follow the presence of `waypoints` and `speed_param`
    /// when encoding, other bits are kept as given.
    pub path_flags: u16,
    /// Movement speed or timing scalar (bit 1).
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
            reader.ensure_count(count, 1)?;
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

/// One `ActionLayerEntry` of `0xBD` (§4.6.3).
#[derive(Debug, Clone, PartialEq)]
pub struct ActionLayerEntry {
    /// The animation/action DID.
    pub action_did: CompactDid,
    /// The layer sequence number.
    pub action_seq: u32,
    /// The action parameters (`official-messages-world.md` §5.6.2).
    pub action: ActionPayload,
    /// The layer bitfield's bit count.
    pub bit_count: u32,
    /// The bitfield bytes (`ceil(bit_count / 8)`).
    pub bits: Vec<u8>,
}

/// Event `0xBD` (`entity_anim_layer_state`, §4.6.3) for the entity of the blob's
/// `recipientId`.
#[derive(Debug, Clone, PartialEq)]
pub struct AnimLayerState {
    /// The layers.
    pub layers: Vec<ActionLayerEntry>,
}

/// Smallest wire size of one `ActionLayerEntry`: Compact DID 2, `VariableU32` 1,
/// `ActionPayload` 11, bitfield 1.
const MIN_LAYER_BYTES: usize = 15;

impl AnimLayerState {
    /// Encodes the inner payload.
    ///
    /// # Errors
    ///
    /// [`PlayError`] when a layer's bitfield length does not match its bit count, or a
    /// wire error.
    pub fn encode(&self, writer: &mut WireWriter) -> Result<(), PlayError> {
        if self.layers.len() > MAX_ANIM_LAYERS {
            return Err(PlayError::TooMany {
                what: "animation layers",
                max: MAX_ANIM_LAYERS,
            });
        }
        writer.write_base_flags();
        // Bounded by MAX_ANIM_LAYERS.
        writer.write_vle3(self.layers.len() as u32)?;
        for layer in &self.layers {
            layer.action_did.encode(writer)?;
            writer.write_vle4(layer.action_seq);
            layer.action.encode(writer)?;
            if layer.bits.len() != (layer.bit_count as usize).div_ceil(8) {
                return Err(PlayError::TooMany {
                    what: "bitfield bytes",
                    max: (layer.bit_count as usize).div_ceil(8),
                });
            }
            writer.write_vle3(layer.bit_count)?;
            writer.write_bytes(&layer.bits);
        }
        Ok(())
    }

    /// Decodes the inner payload; action property tables use `schema`.
    ///
    /// # Errors
    ///
    /// [`PlayError`] on truncation, a count over the remaining bytes or
    /// [`MAX_ANIM_LAYERS`], or a bad action payload.
    pub fn decode(
        reader: &mut WireReader<'_>,
        schema: &dyn PropertySchemaLookup,
    ) -> Result<Self, PlayError> {
        reader.read_base_flags()?;
        let count = reader.read_vle3()? as usize;
        reader.ensure_count(count, MIN_LAYER_BYTES)?;
        if count > MAX_ANIM_LAYERS {
            return Err(PlayError::TooMany {
                what: "animation layers",
                max: MAX_ANIM_LAYERS,
            });
        }
        let mut layers = Vec::with_capacity(count);
        for _ in 0..count {
            let action_did = CompactDid::decode(reader)?;
            let action_seq = reader.read_vle4()?;
            let action = ActionPayload::decode(reader, schema)?;
            let EncodedPropertyValue::Bitfield { bit_count, bytes } =
                EncodedPropertyValue::decode(reader, PropertyType::Bitfield, schema)?
            else {
                return Err(PlayError::UnsupportedCommand("bitfield"));
            };
            layers.push(ActionLayerEntry {
                action_did,
                action_seq,
                action,
                bit_count,
                bits: bytes,
            });
        }
        Ok(Self { layers })
    }

    /// Decodes a standalone inner payload.
    ///
    /// # Errors
    ///
    /// As [`Self::decode`], plus trailing bytes.
    pub fn from_bytes(bytes: &[u8], schema: &dyn PropertySchemaLookup) -> Result<Self, PlayError> {
        let mut r = WireReader::new(bytes);
        let m = Self::decode(&mut r, schema)?;
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
