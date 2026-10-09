// SPDX-License-Identifier: GPL-3.0-or-later
//! Movement parameters (`collision.md` §10.1–§10.3), their layering (§11.1) and the float
//! height (§10.5, §11.2).
//!
//! Every value comes from archive property sets and is hostile input (decision 0013 "Input
//! validation"). A field whose value is non-finite or outside its documented range counts
//! as **not listed**: the overlay keeps the value of the layer below it, and the last
//! fallback is [`MovementParams::BUILTIN`], the built-in base layer.

use tessera_props::{Did, Property};
use tessera_props::{GameData, PropertyRegistry, PropertySet, RecordSource, Value};

use super::state::MovementState;
use crate::error::{PhysicsError, Result};

/// Largest speed any parameter or the mover's velocity may take, m/s (Tessera bound,
/// decision 0013: the observed values are at most 100, the Slide terminal velocity of
/// §10.3). At 1/60 s per step it keeps the swept motion of one step within 2 m.
pub const MAX_SPEED: f32 = 120.0;
/// Largest control acceleration per axis, m/s² (Tessera bound; observed at most 65).
pub const MAX_ACCELERATION: f32 = 1000.0;
/// Largest |gravity|, m/s² (Tessera bound; observed −20 to 50).
pub const MAX_GRAVITY: f32 = 200.0;
/// Largest jump acceleration, m/s² (Tessera bound; observed at most 230). The jump speed
/// it gives is further limited to [`MAX_SPEED`].
pub const MAX_JUMP_ACCELERATION: f32 = 2000.0;
/// Largest ramp time, s (Tessera bound; observed at most 0.5).
pub const MAX_RAMP_TIME: f32 = 60.0;
/// Largest step-up height, m (Tessera bound; observed 0.4–0.6).
pub const MAX_STEP_UP: f32 = 2.0;
/// Largest float height, m (Tessera bound; observed 0–1.42).
pub const MAX_FLOAT_HEIGHT: f32 = 10.0;
/// The float height when neither the entity nor the movement record gives one (§11.2).
pub const FALLBACK_FLOAT_HEIGHT: f32 = 1.35;
/// `Agent_Species` value of the Troll layer (§11.1 step 2).
pub const TROLL_SPECIES: u32 = 47;
/// Most fields one MoveTo struct may hold (bounded allocation; the §10.1 MoveTo set has
/// eleven).
pub const MAX_MOVE_TO_FIELDS: usize = 64;

/// The interoperability names of the sixteen fields of the movement parameter unit
/// (§10.1), in the order of [`MovementPids::fields`].
pub const MOVEMENT_FIELD_NAMES: [&str; 16] = [
    "GenericPhysicsController_ForwardMoveSpeed",
    "GenericPhysicsController_BackwardMoveSpeed",
    "GenericPhysicsController_StrafeLeftMoveSpeed",
    "GenericPhysicsController_StrafeRightMoveSpeed",
    "GenericPhysicsController_FlyUpMoveSpeed",
    "GenericPhysicsController_FlyDownMoveSpeed",
    "GenericPhysicsController_ControlAcceleration",
    "GenericPhysicsController_VelocityDamper",
    "GenericPhysicsController_RampTime",
    "GenericPhysicsController_Gravity",
    "GenericPhysicsController_PhysicsTerminalVelocity",
    "GenericPhysicsController_JumpAcceleration",
    "GenericPhysicsController_StepUpHeight",
    "GenericPhysicsController_FloatHeight",
    "GenericPhysicsController_ForceStrafe",
    "GenericPhysicsController_AllowContactLoss",
];

/// The layers of the player movement record (§10.3), in the table's order.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, PartialOrd, Ord)]
pub enum LayerKind {
    /// Always applied first.
    BasePlayer,
    /// `Agent_Species` 47.
    Troll,
    /// Mounted.
    Mounted,
    /// Mounted on a ramp-up controller.
    Warsteed,
    /// `Movement_MovementStyle` 1.
    Walking,
    /// Walking while mounted.
    MountedWalking,
    /// `Physics_OnSlide`.
    Slide,
    /// `Physics_OnIce`.
    Ice,
    /// In the air (state 4).
    Falling,
    /// In the air while stealthed.
    StealthedFalling,
    /// On a surface too steep to walk (state 6).
    Slope,
    /// In water (states 1–3).
    Swimming,
    /// In water while stealthed.
    StealthedSwimming,
    /// `Movement_ActiveMovementFrozen`.
    Frozen,
}

impl LayerKind {
    /// Every layer, in table order (the index of [`PlayerMovementRecord`]'s arrays).
    pub const ALL: [Self; 14] = [
        Self::BasePlayer,
        Self::Troll,
        Self::Mounted,
        Self::Warsteed,
        Self::Walking,
        Self::MountedWalking,
        Self::Slide,
        Self::Ice,
        Self::Falling,
        Self::StealthedFalling,
        Self::Slope,
        Self::Swimming,
        Self::StealthedSwimming,
        Self::Frozen,
    ];

    /// The layer's name as it appears in its property name.
    #[must_use]
    pub fn name(self) -> &'static str {
        match self {
            Self::BasePlayer => "BasePlayer",
            Self::Troll => "Troll",
            Self::Mounted => "Mounted",
            Self::Warsteed => "Warsteed",
            Self::Walking => "Walking",
            Self::MountedWalking => "MountedWalking",
            Self::Slide => "Slide",
            Self::Ice => "Ice",
            Self::Falling => "Falling",
            Self::StealthedFalling => "StealthedFalling",
            Self::Slope => "Slope",
            Self::Swimming => "Swimming",
            Self::StealthedSwimming => "StealthedSwimming",
            Self::Frozen => "Frozen",
        }
    }

    /// The name of the record property holding the layer:
    /// `PlayerPhysicsControl_<name>Parameters` (§10.3; the pattern is stated for
    /// BasePlayer and applied to every layer).
    #[must_use]
    pub fn property_name(self) -> String {
        format!("PlayerPhysicsControl_{}Parameters", self.name())
    }

    /// Index in [`LayerKind::ALL`].
    #[must_use]
    pub fn index(self) -> usize {
        self as usize
    }
}

/// The property IDs the movement code reads, looked up by name at run time.
#[derive(Debug, Clone, PartialEq, Eq, Default)]
pub struct MovementPids {
    /// The sixteen §10.1 fields, in the order of [`MOVEMENT_FIELD_NAMES`].
    pub fields: [Option<u32>; 16],
    /// `PlayerPhysicsControl_<layer>Parameters`, by [`LayerKind::index`].
    pub layers: [Option<u32>; 14],
    /// `PlayerPhysicsControl_MovementConfiguration`.
    pub movement_configuration: Option<u32>,
    /// `PlayerPhysicsControl_MoveToConfiguration`.
    pub move_to_configuration: Option<u32>,
    /// `PlayerPhysicsControl_DefaultFloatHeight`.
    pub default_float_height: Option<u32>,
    /// `MoveToController_FloatHeight` (§11.2).
    pub move_to_float_height: Option<u32>,
    /// `Physics_AvatarFloatHeight` (§10.5).
    pub avatar_float_height: Option<u32>,
    /// `Agent_Species`.
    pub species: Option<u32>,
    /// `Mount_Is_Mounted`.
    pub mounted: Option<u32>,
    /// `Mount_Movement_Use_RampUp_Controller`.
    pub ramp_up_controller: Option<u32>,
    /// `Movement_MovementStyle`.
    pub movement_style: Option<u32>,
    /// `Physics_OnSlide`.
    pub on_slide: Option<u32>,
    /// `Physics_OnIce`.
    pub on_ice: Option<u32>,
    /// `Stealth_IsStealthed`.
    pub stealthed: Option<u32>,
    /// `Movement_ActiveMovementFrozen`.
    pub frozen: Option<u32>,
    /// `Entity_Element` (§10.4).
    pub element: Option<u32>,
}

impl MovementPids {
    /// Looks every PID up with `pid_of(name)`.
    #[must_use]
    pub fn from_names(pid_of: impl Fn(&str) -> Option<u32>) -> Self {
        Self {
            fields: MOVEMENT_FIELD_NAMES.map(&pid_of),
            layers: LayerKind::ALL.map(|k| pid_of(&k.property_name())),
            movement_configuration: pid_of("PlayerPhysicsControl_MovementConfiguration"),
            move_to_configuration: pid_of("PlayerPhysicsControl_MoveToConfiguration"),
            default_float_height: pid_of("PlayerPhysicsControl_DefaultFloatHeight"),
            move_to_float_height: pid_of("MoveToController_FloatHeight"),
            avatar_float_height: pid_of("Physics_AvatarFloatHeight"),
            species: pid_of("Agent_Species"),
            mounted: pid_of("Mount_Is_Mounted"),
            ramp_up_controller: pid_of("Mount_Movement_Use_RampUp_Controller"),
            movement_style: pid_of("Movement_MovementStyle"),
            on_slide: pid_of("Physics_OnSlide"),
            on_ice: pid_of("Physics_OnIce"),
            stealthed: pid_of("Stealth_IsStealthed"),
            frozen: pid_of("Movement_ActiveMovementFrozen"),
            element: pid_of("Entity_Element"),
        }
    }

    /// Looks every PID up in the property registry.
    #[must_use]
    pub fn from_registry(registry: &PropertyRegistry) -> Self {
        Self::from_names(|n| registry.pid(n))
    }
}

/// Whether `v` is finite and within `lo..=hi`.
fn in_range(v: f32, lo: f32, hi: f32) -> bool {
    v.is_finite() && v >= lo && v <= hi
}

fn vec_in_range(v: [f32; 3], lo: f32, hi: f32) -> bool {
    v.iter().all(|c| in_range(*c, lo, hi))
}

/// One layer of movement parameters: the fields it lists (`None` = not listed). The
/// field meanings and units are those of §10.1.
#[derive(Debug, Clone, Copy, PartialEq, Default)]
pub struct MovementLayer {
    /// Target speed forward, m/s.
    pub forward_speed: Option<f32>,
    /// Target speed backward, m/s.
    pub backward_speed: Option<f32>,
    /// Target speed strafing left, m/s.
    pub strafe_left_speed: Option<f32>,
    /// Target speed strafing right, m/s.
    pub strafe_right_speed: Option<f32>,
    /// Vertical speed up when flying (0 for players).
    pub fly_up_speed: Option<f32>,
    /// Vertical speed down when flying (0 for players).
    pub fly_down_speed: Option<f32>,
    /// Acceleration toward the target velocity per local axis (forward, side, up), m/s².
    pub control_acceleration: Option<[f32; 3]>,
    /// Damping factor per second per local axis (forward, side, up), 0–1.
    pub velocity_damper: Option<[f32; 3]>,
    /// Seconds to reach the target speed.
    pub ramp_time: Option<f32>,
    /// Downward acceleration, m/s² (negative = upward).
    pub gravity: Option<f32>,
    /// Speed limit, m/s.
    pub terminal_velocity: Option<f32>,
    /// Upward acceleration of a jump, m/s².
    pub jump_acceleration: Option<f32>,
    /// Highest step climbed without jumping, m.
    pub step_up_height: Option<f32>,
    /// Water depth at which the mover floats, m.
    pub float_height: Option<f32>,
    /// `ForceStrafe` (meaning OPEN, §16 Q11; carried, not used).
    pub force_strafe: Option<bool>,
    /// `AllowContactLoss` (meaning OPEN, §16 Q11; carried, not used).
    pub allow_contact_loss: Option<bool>,
}

impl MovementLayer {
    /// Reads the §10.1 fields of one set: a `MovementConfiguration` struct, or a flat
    /// controller set of §10.2. A value of the wrong type or out of range is left out
    /// (it counts as not listed).
    #[must_use]
    pub fn from_set(set: &PropertySet, pids: &MovementPids) -> Self {
        let get = |i: usize| pids.fields[i].and_then(|p| set.get(p));
        let float = |i: usize| get(i).and_then(Value::as_f32);
        let vector = |i: usize| get(i).and_then(Value::as_vector3);
        let boolean = |i: usize| get(i).and_then(value_bool);
        Self {
            forward_speed: float(0),
            backward_speed: float(1),
            strafe_left_speed: float(2),
            strafe_right_speed: float(3),
            fly_up_speed: float(4),
            fly_down_speed: float(5),
            control_acceleration: vector(6),
            velocity_damper: vector(7),
            ramp_time: float(8),
            gravity: float(9),
            terminal_velocity: float(10),
            jump_acceleration: float(11),
            step_up_height: float(12),
            float_height: float(13),
            force_strafe: boolean(14),
            allow_contact_loss: boolean(15),
        }
        .sanitized()
    }

    /// The layer with every out-of-range value removed (decision 0013). Ranges: speeds
    /// 0–[`MAX_SPEED`]; control acceleration 0–[`MAX_ACCELERATION`] per axis; damper 0–1
    /// per axis; ramp time 0–[`MAX_RAMP_TIME`]; gravity ±[`MAX_GRAVITY`]; terminal velocity
    /// 0–[`MAX_SPEED`]; jump acceleration 0–[`MAX_JUMP_ACCELERATION`]; step-up
    /// 0–[`MAX_STEP_UP`]; float height 0–[`MAX_FLOAT_HEIGHT`].
    #[must_use]
    pub fn sanitized(self) -> Self {
        let speed = |v: Option<f32>| v.filter(|v| in_range(*v, 0.0, MAX_SPEED));
        Self {
            forward_speed: speed(self.forward_speed),
            backward_speed: speed(self.backward_speed),
            strafe_left_speed: speed(self.strafe_left_speed),
            strafe_right_speed: speed(self.strafe_right_speed),
            fly_up_speed: speed(self.fly_up_speed),
            fly_down_speed: speed(self.fly_down_speed),
            control_acceleration: self
                .control_acceleration
                .filter(|v| vec_in_range(*v, 0.0, MAX_ACCELERATION)),
            velocity_damper: self.velocity_damper.filter(|v| vec_in_range(*v, 0.0, 1.0)),
            ramp_time: self.ramp_time.filter(|v| in_range(*v, 0.0, MAX_RAMP_TIME)),
            gravity: self
                .gravity
                .filter(|v| in_range(*v, -MAX_GRAVITY, MAX_GRAVITY)),
            terminal_velocity: speed(self.terminal_velocity),
            jump_acceleration: self
                .jump_acceleration
                .filter(|v| in_range(*v, 0.0, MAX_JUMP_ACCELERATION)),
            step_up_height: self
                .step_up_height
                .filter(|v| in_range(*v, 0.0, MAX_STEP_UP)),
            float_height: self
                .float_height
                .filter(|v| in_range(*v, 0.0, MAX_FLOAT_HEIGHT)),
            force_strafe: self.force_strafe,
            allow_contact_loss: self.allow_contact_loss,
        }
    }

    /// `self` laid over `below`: every field `self` lists with a valid value replaces the
    /// value below (§11.1).
    #[must_use]
    pub fn over(self, below: Self) -> Self {
        let s = self.sanitized();
        Self {
            forward_speed: s.forward_speed.or(below.forward_speed),
            backward_speed: s.backward_speed.or(below.backward_speed),
            strafe_left_speed: s.strafe_left_speed.or(below.strafe_left_speed),
            strafe_right_speed: s.strafe_right_speed.or(below.strafe_right_speed),
            fly_up_speed: s.fly_up_speed.or(below.fly_up_speed),
            fly_down_speed: s.fly_down_speed.or(below.fly_down_speed),
            control_acceleration: s.control_acceleration.or(below.control_acceleration),
            velocity_damper: s.velocity_damper.or(below.velocity_damper),
            ramp_time: s.ramp_time.or(below.ramp_time),
            gravity: s.gravity.or(below.gravity),
            terminal_velocity: s.terminal_velocity.or(below.terminal_velocity),
            jump_acceleration: s.jump_acceleration.or(below.jump_acceleration),
            step_up_height: s.step_up_height.or(below.step_up_height),
            float_height: s.float_height.or(below.float_height),
            force_strafe: s.force_strafe.or(below.force_strafe),
            allow_contact_loss: s.allow_contact_loss.or(below.allow_contact_loss),
        }
    }

    /// Whether no field is listed.
    #[must_use]
    pub fn is_empty(&self) -> bool {
        *self == Self::default()
    }
}

/// A boolean value; a value of another type (integer, enum, ...) counts as not listed
/// (§10.3 "Hostile values": a field whose value has the wrong type; §10.1 types
/// `ForceStrafe` and `AllowContactLoss` as boolean). The entity conditions of §11.1 read
/// the same way: only a boolean true makes a condition hold.
fn value_bool(v: &Value) -> Option<bool> {
    match v {
        Value::Bool(b) => Some(*b),
        _ => None,
    }
}

/// A complete, validated parameter set: what the controller steps with.
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct MovementParams {
    /// Target speed forward, m/s.
    pub forward_speed: f32,
    /// Target speed backward, m/s.
    pub backward_speed: f32,
    /// Target speed strafing left, m/s.
    pub strafe_left_speed: f32,
    /// Target speed strafing right, m/s.
    pub strafe_right_speed: f32,
    /// Vertical speed up when flying (not used by the walking controller).
    pub fly_up_speed: f32,
    /// Vertical speed down when flying (not used by the walking controller).
    pub fly_down_speed: f32,
    /// Acceleration toward the target velocity per local axis (forward, side, up), m/s².
    pub control_acceleration: [f32; 3],
    /// Damping factor per second per local axis (forward, side, up).
    pub velocity_damper: [f32; 3],
    /// Seconds to reach the target speed.
    pub ramp_time: f32,
    /// Downward acceleration, m/s² (negative = upward).
    pub gravity: f32,
    /// Speed limit, m/s.
    pub terminal_velocity: f32,
    /// Upward acceleration of a jump, m/s².
    pub jump_acceleration: f32,
    /// Highest step climbed without jumping, m.
    pub step_up_height: f32,
    /// Water depth, from the feet, at which the mover floats, m.
    pub float_height: f32,
    /// `ForceStrafe` (OPEN; not used).
    pub force_strafe: bool,
    /// `AllowContactLoss` (OPEN; not used).
    pub allow_contact_loss: bool,
}

impl MovementParams {
    /// The built-in base layer (decision 0013: the last fallback of every field): the
    /// BasePlayer values observed in §10.3, with the fallback float height of §11.2.
    pub const BUILTIN: Self = Self {
        forward_speed: 6.5,
        backward_speed: 3.25,
        strafe_left_speed: 5.0,
        strafe_right_speed: 5.0,
        fly_up_speed: 0.0,
        fly_down_speed: 0.0,
        control_acceleration: [65.0, 65.0, 0.0],
        velocity_damper: [0.2, 0.2, 1.0],
        ramp_time: 0.13,
        gravity: 0.0,
        terminal_velocity: 50.0,
        jump_acceleration: 40.0,
        step_up_height: 0.5,
        float_height: FALLBACK_FLOAT_HEIGHT,
        force_strafe: false,
        allow_contact_loss: false,
    };

    /// The parameters of `layer`, with [`MovementParams::BUILTIN`] for every field it does
    /// not list validly.
    #[must_use]
    pub fn resolve(layer: &MovementLayer) -> Self {
        let l = layer.sanitized();
        let b = Self::BUILTIN;
        Self {
            forward_speed: l.forward_speed.unwrap_or(b.forward_speed),
            backward_speed: l.backward_speed.unwrap_or(b.backward_speed),
            strafe_left_speed: l.strafe_left_speed.unwrap_or(b.strafe_left_speed),
            strafe_right_speed: l.strafe_right_speed.unwrap_or(b.strafe_right_speed),
            fly_up_speed: l.fly_up_speed.unwrap_or(b.fly_up_speed),
            fly_down_speed: l.fly_down_speed.unwrap_or(b.fly_down_speed),
            control_acceleration: l.control_acceleration.unwrap_or(b.control_acceleration),
            velocity_damper: l.velocity_damper.unwrap_or(b.velocity_damper),
            ramp_time: l.ramp_time.unwrap_or(b.ramp_time),
            gravity: l.gravity.unwrap_or(b.gravity),
            terminal_velocity: l.terminal_velocity.unwrap_or(b.terminal_velocity),
            jump_acceleration: l.jump_acceleration.unwrap_or(b.jump_acceleration),
            step_up_height: l.step_up_height.unwrap_or(b.step_up_height),
            float_height: l.float_height.unwrap_or(b.float_height),
            force_strafe: l.force_strafe.unwrap_or(b.force_strafe),
            allow_contact_loss: l.allow_contact_loss.unwrap_or(b.allow_contact_loss),
        }
    }
}

impl Default for MovementParams {
    fn default() -> Self {
        Self::BUILTIN
    }
}

/// Whether `v` is a value a [`MoveToLayer`] keeps: a number, flag, enum key, vector,
/// bitfield or reference (no heap-allocated value).
#[must_use]
pub fn is_move_to_scalar(v: &Value) -> bool {
    matches!(
        v,
        Value::Float(_)
            | Value::Int(_)
            | Value::Int64(_)
            | Value::Enum(_)
            | Value::Bool(_)
            | Value::Vector3(_)
            | Value::TriState(_)
            | Value::Bitfield32(_)
            | Value::Bitfield64(_)
            | Value::DidRef(_)
            | Value::PropertyId(_)
            | Value::StringToken(_)
            | Value::Timestamp(_)
            | Value::InstanceId(_)
    )
}

/// The fields of one `MoveToConfiguration` struct (§10.3): the `MoveToController_*`
/// values it lists, by PID, in stored order. They drive scripted "move to" motion, which
/// the walking controller does not use; they are layered (§11.1) and handed on as they
/// are.
#[derive(Debug, Clone, PartialEq, Default)]
pub struct MoveToLayer {
    /// (PID, value) pairs, each PID once, at most [`MAX_MOVE_TO_FIELDS`].
    pub fields: Vec<(u32, Value)>,
}

impl MoveToLayer {
    /// The fields of a MoveTo struct: the first [`MAX_MOVE_TO_FIELDS`] whose value is a
    /// fixed-size scalar ([`is_move_to_scalar`]). The `MoveToController_*` fields of §10.3
    /// are floats; a string, struct, array, waveform or text value is left out, so a
    /// hostile record never makes the layering copy a large tree (decision 0013 "Input
    /// validation").
    #[must_use]
    pub fn from_set(set: &PropertySet) -> Self {
        let mut out = Self::default();
        for p in set
            .iter()
            .filter(|p| is_move_to_scalar(&p.value))
            .take(MAX_MOVE_TO_FIELDS)
        {
            out.set(p.pid, p.value.clone());
        }
        out
    }

    /// The value of `pid`.
    #[must_use]
    pub fn get(&self, pid: u32) -> Option<&Value> {
        self.fields.iter().find(|(p, _)| *p == pid).map(|(_, v)| v)
    }

    /// Sets `pid` (replacing, or appending while there is room).
    pub fn set(&mut self, pid: u32, value: Value) {
        if let Some(slot) = self.fields.iter_mut().find(|(p, _)| *p == pid) {
            slot.1 = value;
        } else if self.fields.len() < MAX_MOVE_TO_FIELDS {
            self.fields.push((pid, value));
        }
    }

    /// Lays `upper` over `self` (§11.1): every field it lists replaces the value so far.
    pub fn overlay(&mut self, upper: &Self) {
        for (pid, v) in &upper.fields {
            self.set(*pid, v.clone());
        }
    }

    /// As a property set.
    #[must_use]
    pub fn to_set(&self) -> PropertySet {
        PropertySet::from_properties(
            self.fields
                .iter()
                .map(|(p, v)| Property::new(*p, v.clone())),
        )
    }
}

/// The player movement record (§10.3): one movement layer and one MoveTo layer per
/// [`LayerKind`] (absent when the record does not have that layer) and the default float
/// height.
#[derive(Debug, Clone, PartialEq, Default)]
pub struct PlayerMovementRecord {
    /// Movement layers by [`LayerKind::index`].
    pub layers: [Option<MovementLayer>; 14],
    /// MoveTo layers by [`LayerKind::index`].
    pub move_to: [Option<MoveToLayer>; 14],
    /// `PlayerPhysicsControl_DefaultFloatHeight`, when present and in range.
    pub default_float_height: Option<f32>,
}

impl PlayerMovementRecord {
    /// The documented built-in record (decision 0013): the movement layers observed in
    /// §10.3, every one of the fourteen, with the default float height 1.35 and no MoveTo
    /// layers. It is what a mover uses when the archives hold no player movement record, so
    /// it falls (Falling gravity 15), slides (Slope gravity 10) and floats (Swimming gravity
    /// −20) as the observed record does. [`PlayerMovementRecord::default`] is the record
    /// without any layer (every parameter [`MovementParams::BUILTIN`], gravity 0).
    #[must_use]
    pub fn builtin() -> Self {
        let mut out = Self::default();
        for (kind, layer) in builtin_layers() {
            out.layers[kind.index()] = Some(layer);
        }
        out.default_float_height = Some(FALLBACK_FLOAT_HEIGHT);
        out
    }

    /// Reads the record from its property set. The set is recognised by the presence of
    /// `PlayerPhysicsControl_BasePlayerParameters` (§10.3: locate it by that property, not
    /// by its DID).
    ///
    /// # Errors
    ///
    /// [`PhysicsError::NotMovementRecord`] when the set has no BasePlayer struct.
    pub fn from_set(set: &PropertySet, pids: &MovementPids) -> Result<Self> {
        let base = pids.layers[LayerKind::BasePlayer.index()]
            .and_then(|p| set.get(p))
            .and_then(Value::as_struct);
        if base.is_none() {
            return Err(PhysicsError::NotMovementRecord);
        }
        let mut out = Self::default();
        for kind in LayerKind::ALL {
            let Some(layer) = pids.layers[kind.index()]
                .and_then(|p| set.get(p))
                .and_then(Value::as_struct)
            else {
                continue;
            };
            let movement = pids
                .movement_configuration
                .and_then(|p| layer.get(p))
                .and_then(Value::as_struct);
            out.layers[kind.index()] = Some(
                movement
                    .map(|m| MovementLayer::from_set(m, pids))
                    .unwrap_or_default(),
            );
            out.move_to[kind.index()] = pids
                .move_to_configuration
                .and_then(|p| layer.get(p))
                .and_then(Value::as_struct)
                .map(MoveToLayer::from_set);
        }
        out.default_float_height = pids
            .default_float_height
            .and_then(|p| set.get(p))
            .and_then(Value::as_f32)
            .filter(|v| in_range(*v, 0.0, MAX_FLOAT_HEIGHT));
        Ok(out)
    }

    /// The first of `candidates` (0x78–0x7F property-set DIDs) whose set is the player
    /// movement record, with that DID. Candidates that are absent, fail to decode or are
    /// not the record are skipped (a decode failure is logged at `debug`).
    ///
    /// # Errors
    ///
    /// [`PhysicsError::Record`] when the property registry cannot be loaded.
    pub fn locate<S: RecordSource>(
        data: &GameData<S>,
        candidates: impl IntoIterator<Item = Did>,
    ) -> Result<Option<(Did, Self)>> {
        let registry = data.registry().map_err(|e| PhysicsError::Record {
            did: tessera_props::PROPERTY_DICTIONARY_DID.0,
            what: "property registry",
            message: e.to_string(),
        })?;
        let pids = MovementPids::from_registry(registry);
        let Some(base) = pids.layers[0] else {
            return Ok(None);
        };
        for did in candidates {
            let set = match data.property_set(did) {
                Ok(Some(s)) => s,
                Ok(None) => continue,
                Err(e) => {
                    tracing::debug!(did = %format_args!("{:#010x}", did.0), error = %e, "skipping candidate");
                    continue;
                }
            };
            if set.contains(base)
                && let Ok(r) = Self::from_set(&set, &pids)
            {
                return Ok(Some((did, r)));
            }
        }
        Ok(None)
    }

    /// The movement layer `kind`, when the record has it.
    #[must_use]
    pub fn layer(&self, kind: LayerKind) -> Option<&MovementLayer> {
        self.layers[kind.index()].as_ref()
    }

    /// Composes the active parameters (§11.1) for `conditions` and the mover's current
    /// `element`, then sets the float height (§11.2) in both the movement parameters and
    /// the MoveTo set (`MoveToController_FloatHeight`, when `pids` knows it).
    ///
    /// The controller's step uses [`PlayerMovementRecord::compose_movement`], which builds
    /// no MoveTo set.
    #[must_use]
    pub fn compose(
        &self,
        conditions: &LayerConditions,
        element: MovementState,
        pids: &MovementPids,
    ) -> ComposedParams {
        let applied = conditions.layers(element);
        let mut move_to = MoveToLayer::default();
        for kind in applied.iter() {
            if let Some(m) = &self.move_to[kind.index()] {
                move_to.overlay(m);
            }
        }
        let float = self.float_height(conditions);
        if let Some(pid) = pids.move_to_float_height {
            move_to.set(pid, Value::Float(float));
        }
        ComposedParams {
            movement: self.compose_movement(conditions, element),
            move_to,
            layers: applied,
            kind: conditions.controller_kind(),
        }
    }

    /// The movement parameters of [`PlayerMovementRecord::compose`] alone (§11.1 with the
    /// float height of §11.2): no allocation, nothing cloned.
    #[must_use]
    pub fn compose_movement(
        &self,
        conditions: &LayerConditions,
        element: MovementState,
    ) -> MovementParams {
        let mut movement = self.overlay(conditions, element);
        movement.float_height = Some(self.float_height(conditions));
        MovementParams::resolve(&movement)
    }

    /// The float height for `conditions` (§11.2, [`float_height`]).
    #[must_use]
    pub fn float_height(&self, conditions: &LayerConditions) -> f32 {
        float_height(conditions.avatar_float_height, self.default_float_height)
    }

    /// The ramp time a jump's push is applied for (`collision.md` §12.4 (c): "for one ramp
    /// time"): the BasePlayer layer's ramp time, else [`MovementParams::BUILTIN`]'s.
    /// Tessera Decision `collision.md` §12.4 (i): the BasePlayer value, not the composed
    /// layer's, so the Walking and mounted layers (ramp 0.25 s) keep the stated 5.2 m/s and
    /// ≈ 0.9 m apex.
    #[must_use]
    pub fn jump_ramp_time(&self) -> f32 {
        self.layer(LayerKind::BasePlayer)
            .and_then(|l| l.sanitized().ramp_time)
            .unwrap_or(MovementParams::BUILTIN.ramp_time)
    }

    /// The movement record's layers as one partial layer for `conditions` and `element`
    /// (no float height, no built-in fallback): what §14.6 lists.
    #[must_use]
    pub fn overlay(&self, conditions: &LayerConditions, element: MovementState) -> MovementLayer {
        let mut movement = MovementLayer::default();
        for kind in conditions.layers(element).iter() {
            if let Some(l) = &self.layers[kind.index()] {
                movement = l.over(movement);
            }
        }
        movement
    }
}

/// The movement layers of §10.3 as observed (the built-in record of
/// [`PlayerMovementRecord::builtin`]; numeric constants under decision 0012, cited from the
/// §10.3 table). Columns: forward, backward, strafe left, strafe right, control
/// acceleration, damper, ramp time, gravity, terminal velocity, jump acceleration, step-up.
#[allow(clippy::type_complexity)]
fn builtin_layers() -> [(LayerKind, MovementLayer); 14] {
    type Row = (
        Option<f32>,
        Option<f32>,
        Option<f32>,
        Option<f32>,
        Option<[f32; 3]>,
        Option<[f32; 3]>,
        Option<f32>,
        Option<f32>,
        Option<f32>,
        Option<f32>,
        Option<f32>,
    );
    const N: Option<f32> = None;
    const NV: Option<[f32; 3]> = None;
    let layer = |r: Row| MovementLayer {
        forward_speed: r.0,
        backward_speed: r.1,
        strafe_left_speed: r.2,
        strafe_right_speed: r.3,
        control_acceleration: r.4,
        velocity_damper: r.5,
        ramp_time: r.6,
        gravity: r.7,
        terminal_velocity: r.8,
        jump_acceleration: r.9,
        step_up_height: r.10,
        ..MovementLayer::default()
    };
    fn s<T>(v: T) -> Option<T> {
        Some(v)
    }
    use LayerKind as K;
    [
        (
            K::BasePlayer,
            MovementLayer {
                fly_up_speed: s(0.0),
                fly_down_speed: s(0.0),
                force_strafe: s(false),
                allow_contact_loss: s(false),
                ..layer((
                    s(6.5),
                    s(3.25),
                    s(5.0),
                    s(5.0),
                    s([65.0, 65.0, 0.0]),
                    s([0.2, 0.2, 1.0]),
                    s(0.13),
                    s(0.0),
                    s(50.0),
                    s(40.0),
                    s(0.5),
                ))
            },
        ),
        (K::Troll, layer((N, s(6.0), N, N, NV, NV, N, N, N, N, N))),
        (
            K::Mounted,
            layer((N, s(2.0), N, N, NV, NV, s(0.25), N, N, N, N)),
        ),
        (
            K::Warsteed,
            layer((
                N,
                s(2.0),
                s(3.0),
                s(3.0),
                s([10.0, 10.0, 0.0]),
                NV,
                s(0.25),
                N,
                N,
                N,
                N,
            )),
        ),
        (
            K::Walking,
            layer((s(5.0), s(4.5), s(4.5), s(4.5), NV, NV, s(0.25), N, N, N, N)),
        ),
        (
            K::MountedWalking,
            layer((
                s(3.0),
                s(3.0),
                s(3.0),
                s(3.0),
                s([10.0, 10.0, 0.0]),
                NV,
                s(0.25),
                N,
                N,
                N,
                N,
            )),
        ),
        (
            K::Slide,
            layer((
                s(3.2),
                s(2.0),
                s(2.0),
                s(2.0),
                s([43.0, 25.0, 0.0]),
                s([1.0; 3]),
                s(0.5),
                s(50.0),
                s(100.0),
                s(0.0),
                N,
            )),
        ),
        (
            K::Ice,
            layer((
                s(3.2),
                s(2.0),
                s(2.0),
                s(2.0),
                s([43.0, 25.0, 0.0]),
                s([1.0; 3]),
                s(0.5),
                s(25.0),
                s(50.0),
                s(0.0),
                N,
            )),
        ),
        (
            K::Falling,
            layer((
                s(5.0),
                s(1.0),
                N,
                N,
                s([10.0, 10.0, 0.0]),
                s([1.0; 3]),
                s(0.01),
                s(15.0),
                N,
                s(0.0),
                N,
            )),
        ),
        (
            K::StealthedFalling,
            layer((
                s(2.42),
                s(1.0),
                s(2.42),
                s(2.42),
                s([10.0, 10.0, 0.0]),
                s([1.0; 3]),
                s(0.01),
                s(15.0),
                N,
                s(0.0),
                N,
            )),
        ),
        (
            K::Slope,
            layer((
                s(0.0),
                s(0.0),
                s(0.0),
                s(0.0),
                s([0.0; 3]),
                s([1.0; 3]),
                s(0.1),
                s(10.0),
                N,
                s(0.0),
                N,
            )),
        ),
        (
            K::Swimming,
            layer((
                s(3.2),
                s(2.0),
                s(2.0),
                s(2.0),
                NV,
                s([0.4; 3]),
                s(0.4),
                s(-20.0),
                N,
                s(0.0),
                N,
            )),
        ),
        (
            K::StealthedSwimming,
            layer((
                s(4.15),
                s(2.5),
                s(2.5),
                s(2.5),
                s([35.0, 25.0, 0.0]),
                s([0.4; 3]),
                s(0.4),
                s(-20.0),
                N,
                s(0.0),
                N,
            )),
        ),
        (
            K::Frozen,
            layer((
                s(0.0),
                s(0.0),
                s(0.0),
                s(0.0),
                s([0.0; 3]),
                NV,
                s(0.25),
                N,
                N,
                N,
                N,
            )),
        ),
    ]
}

/// The float height (§11.2): the entity's `Physics_AvatarFloatHeight` if present (and in
/// range, 0–[`MAX_FLOAT_HEIGHT`]), else the record's `PlayerPhysicsControl_DefaultFloatHeight`,
/// else [`FALLBACK_FLOAT_HEIGHT`].
#[must_use]
pub fn float_height(avatar: Option<f32>, record_default: Option<f32>) -> f32 {
    let ok = |v: &f32| in_range(*v, 0.0, MAX_FLOAT_HEIGHT);
    avatar
        .filter(ok)
        .or(record_default.filter(ok))
        .unwrap_or(FALLBACK_FLOAT_HEIGHT)
}

/// The controller kind (§11.1, DID map `PHYSICSCONTROLLER` 0x28000030, §10.2).
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum ControllerKind {
    /// Key 3.
    Generic,
    /// Key 0x10000001, when `Mount_Movement_Use_RampUp_Controller` is true.
    Mounted,
}

impl ControllerKind {
    /// The DID-map key (§10.2).
    #[must_use]
    pub fn key(self) -> u32 {
        match self {
            Self::Generic => 3,
            Self::Mounted => 0x1000_0001,
        }
    }
}

/// The DID map of controller sets (`property-system.md` §8, `collision.md` §10.2).
pub const PHYSICS_CONTROLLER_MAP: Did = Did(0x2800_0030);

/// The flat controller set of `kind` (§10.2), read through the `PHYSICSCONTROLLER` DID
/// map; `Ok(None)` when the map, its entry or the set is absent.
///
/// # Errors
///
/// [`PhysicsError::Record`] when the map, the registry or the set cannot be read.
pub fn controller_set<S: RecordSource>(
    data: &GameData<S>,
    kind: ControllerKind,
) -> Result<Option<MovementLayer>> {
    let err = |did: Did, what: &'static str| {
        move |e: tessera_props::PropsError| PhysicsError::Record {
            did: did.0,
            what,
            message: e.to_string(),
        }
    };
    let Some(entry) = data
        .did_map_entry(PHYSICS_CONTROLLER_MAP, kind.key())
        .map_err(err(PHYSICS_CONTROLLER_MAP, "controller map"))?
    else {
        return Ok(None);
    };
    if entry.value.is_null() {
        return Ok(None);
    }
    let registry = data.registry().map_err(err(
        tessera_props::PROPERTY_DICTIONARY_DID,
        "property registry",
    ))?;
    let pids = MovementPids::from_registry(registry);
    let set = data
        .property_set(entry.value)
        .map_err(err(entry.value, "controller set"))?;
    Ok(set.map(|s| MovementLayer::from_set(&s, &pids)))
}

/// The entity conditions that choose the layers (§11.1). The mover's `Entity_Element` is
/// not here: the controller owns it ([`MovementState`]).
#[derive(Debug, Clone, Copy, PartialEq, Default)]
pub struct LayerConditions {
    /// `Agent_Species`.
    pub species: Option<u32>,
    /// `Mount_Is_Mounted`.
    pub mounted: bool,
    /// `Mount_Movement_Use_RampUp_Controller`.
    pub ramp_up_controller: bool,
    /// `Movement_MovementStyle` is 1.
    pub walking: bool,
    /// `Physics_OnSlide`.
    pub on_slide: bool,
    /// `Physics_OnIce`.
    pub on_ice: bool,
    /// `Stealth_IsStealthed`.
    pub stealthed: bool,
    /// `Movement_ActiveMovementFrozen`.
    pub frozen: bool,
    /// `Physics_AvatarFloatHeight` (§10.5).
    pub avatar_float_height: Option<f32>,
}

impl LayerConditions {
    /// Reads the conditions from an entity's property set (absent properties are false
    /// or `None`), with the client's value types (§11.1 "Value types of the conditions",
    /// BIN, C41): the boolean conditions hold only for a Boolean true, `Agent_Species` and
    /// `Movement_MovementStyle` count only as an enum or property-ID value (another type
    /// reads as 0), the float height only as a float.
    #[must_use]
    pub fn from_set(set: &PropertySet, pids: &MovementPids) -> Self {
        let get = |p: Option<u32>| p.and_then(|p| set.get(p));
        let flag = |p: Option<u32>| get(p).and_then(value_bool).unwrap_or(false);
        let int = |p: Option<u32>| {
            get(p).map(|v| match v {
                Value::Enum(k) | Value::PropertyId(k) => *k,
                _ => 0,
            })
        };
        Self {
            species: int(pids.species),
            mounted: flag(pids.mounted),
            ramp_up_controller: flag(pids.ramp_up_controller),
            walking: int(pids.movement_style) == Some(1),
            on_slide: flag(pids.on_slide),
            on_ice: flag(pids.on_ice),
            stealthed: flag(pids.stealthed),
            frozen: flag(pids.frozen),
            avatar_float_height: get(pids.avatar_float_height).and_then(Value::as_f32),
        }
    }

    /// The layers §11.1 applies, in order, for the mover's `element`.
    #[must_use]
    pub fn layers(&self, element: MovementState) -> LayerList {
        let mut l = LayerList::default();
        l.push(LayerKind::BasePlayer);
        if self.species == Some(TROLL_SPECIES) {
            l.push(LayerKind::Troll);
        }
        if self.mounted {
            l.push(if self.ramp_up_controller {
                LayerKind::Warsteed
            } else {
                LayerKind::Mounted
            });
        }
        if self.walking {
            l.push(if self.mounted {
                LayerKind::MountedWalking
            } else {
                LayerKind::Walking
            });
        }
        if self.on_slide {
            l.push(LayerKind::Slide);
        } else {
            if self.on_ice {
                l.push(LayerKind::Ice);
            }
            match element {
                MovementState::EarthNoSurface => l.push(if self.stealthed {
                    LayerKind::StealthedFalling
                } else {
                    LayerKind::Falling
                }),
                MovementState::EarthUncontrolledSurface => l.push(LayerKind::Slope),
                MovementState::WaterControlledSurface
                | MovementState::WaterNoSurface
                | MovementState::WaterUncontrolledSurface => l.push(if self.stealthed {
                    LayerKind::StealthedSwimming
                } else {
                    LayerKind::Swimming
                }),
                MovementState::EarthControlledSurface => {}
            }
        }
        if self.frozen {
            l.push(LayerKind::Frozen);
        }
        l
    }

    /// Generic, or Mounted when `Mount_Movement_Use_RampUp_Controller` is true (§11.1).
    #[must_use]
    pub fn controller_kind(&self) -> ControllerKind {
        if self.ramp_up_controller {
            ControllerKind::Mounted
        } else {
            ControllerKind::Generic
        }
    }
}

/// The layers applied, in order (at most 7 of the 14).
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub struct LayerList {
    items: [Option<LayerKind>; 8],
    len: usize,
}

impl LayerList {
    fn push(&mut self, k: LayerKind) {
        if let Some(slot) = self.items.get_mut(self.len) {
            *slot = Some(k);
            self.len += 1;
        }
    }

    /// The layers in order.
    pub fn iter(&self) -> impl Iterator<Item = LayerKind> + '_ {
        self.items.iter().take(self.len).flatten().copied()
    }

    /// Number of layers.
    #[must_use]
    pub fn len(&self) -> usize {
        self.len
    }

    /// Whether there are none (never true for a list from [`LayerConditions::layers`]).
    #[must_use]
    pub fn is_empty(&self) -> bool {
        self.len == 0
    }

    /// Whether `kind` is among them.
    #[must_use]
    pub fn contains(&self, kind: LayerKind) -> bool {
        self.iter().any(|k| k == kind)
    }
}

/// The result of [`PlayerMovementRecord::compose`].
#[derive(Debug, Clone, PartialEq)]
pub struct ComposedParams {
    /// The movement parameters, float height included.
    pub movement: MovementParams,
    /// The MoveTo fields, float height included.
    pub move_to: MoveToLayer,
    /// The layers applied, in order.
    pub layers: LayerList,
    /// The controller kind.
    pub kind: ControllerKind,
}

#[cfg(test)]
mod tests {
    use super::*;

    fn pids() -> MovementPids {
        MovementPids::from_names(|n| {
            if let Some(i) = MOVEMENT_FIELD_NAMES.iter().position(|f| *f == n) {
                return u32::try_from(i).ok().map(|i| 100 + i);
            }
            match n {
                "Mount_Is_Mounted" => Some(200),
                "Physics_OnSlide" => Some(201),
                "Physics_OnIce" => Some(202),
                "Stealth_IsStealthed" => Some(203),
                "Movement_ActiveMovementFrozen" => Some(204),
                "Mount_Movement_Use_RampUp_Controller" => Some(205),
                _ => None,
            }
        })
    }

    /// Finding (§10.3 "Hostile values", §10.1): `ForceStrafe` and `AllowContactLoss` read
    /// non-zero integers and enum keys as true; a value of the wrong type counts as not
    /// listed.
    #[test]
    fn movement_booleans_take_only_boolean_values() {
        let p = pids();
        let set = |v: Value| {
            PropertySet::from_properties([Property::new(114, v.clone()), Property::new(115, v)])
        };
        for v in [
            Value::Int(1),
            Value::Enum(1),
            Value::Int64(1),
            Value::Float(1.0),
        ] {
            let l = MovementLayer::from_set(&set(v.clone()), &p);
            assert_eq!(
                (l.force_strafe, l.allow_contact_loss),
                (None, None),
                "{v:?}"
            );
        }
        let l = MovementLayer::from_set(&set(Value::Bool(true)), &p);
        assert_eq!(
            (l.force_strafe, l.allow_contact_loss),
            (Some(true), Some(true))
        );
        // Not listed: the layer below keeps its value.
        let below = MovementLayer {
            force_strafe: Some(true),
            ..MovementLayer::default()
        };
        let over = MovementLayer::from_set(&set(Value::Int(0)), &p).over(below);
        assert_eq!(over.force_strafe, Some(true));
    }

    /// Finding (§11.1): integer and enum values of the boolean entity conditions became
    /// layer conditions; only a boolean true makes a condition hold.
    #[test]
    fn layer_conditions_take_only_boolean_values() {
        let p = pids();
        let all = |v: Value| {
            PropertySet::from_properties((200..=205).map(|pid| Property::new(pid, v.clone())))
        };
        for v in [Value::Int(1), Value::Enum(1), Value::Int64(-1)] {
            assert_eq!(
                LayerConditions::from_set(&all(v.clone()), &p),
                LayerConditions::default(),
                "{v:?}"
            );
        }
        let c = LayerConditions::from_set(&all(Value::Bool(true)), &p);
        assert!(c.mounted && c.on_slide && c.on_ice && c.stealthed && c.frozen);
        assert!(c.ramp_up_controller);
    }

    /// §11.1 "Value types of the conditions" (BIN, C41): `Agent_Species` and
    /// `Movement_MovementStyle` are read only as enum or property-ID values, else as 0; the
    /// float height only as a float.
    #[test]
    fn species_and_style_take_only_enum_or_property_id_values() {
        let mut p = pids();
        p.species = Some(300);
        p.movement_style = Some(301);
        p.avatar_float_height = Some(302);
        let set = |species: Value, style: Value, float: Value| {
            PropertySet::from_properties([
                Property::new(300, species),
                Property::new(301, style),
                Property::new(302, float),
            ])
        };
        let c = LayerConditions::from_set(
            &set(
                Value::Enum(TROLL_SPECIES),
                Value::PropertyId(1),
                Value::Float(1.2),
            ),
            &p,
        );
        assert_eq!(c.species, Some(TROLL_SPECIES));
        assert!(c.walking);
        assert_eq!(c.avatar_float_height, Some(1.2));
        #[allow(clippy::cast_possible_wrap)]
        let troll = TROLL_SPECIES as i32;
        let c =
            LayerConditions::from_set(&set(Value::Int(troll), Value::Int(1), Value::Int(1)), &p);
        assert_eq!(c.species, Some(0));
        assert!(!c.walking);
        assert_eq!(c.avatar_float_height, None);
        assert!(
            !c.layers(MovementState::EarthControlledSurface)
                .contains(LayerKind::Troll)
        );
    }
}
