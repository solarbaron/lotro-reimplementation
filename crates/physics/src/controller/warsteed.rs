// SPDX-License-Identifier: GPL-3.0-or-later
//! The war-steed controller (`mounts.md` §5.2–§5.7): the ramp-up controller that
//! `collision.md` §11.1 selects for a mount type with a war-steed bit.
//!
//! Everything here is a pure function of the layered movement parameters
//! ([`MovementParams`]), the actor's mount properties ([`WarSteedActor`]) and the gait
//! multipliers of the mounted-combat control (`game-controls.md` §5.3.2, read by
//! `tessera-wstate` and passed in as [`GaitMultipliers`]):
//!
//! | Item | Spec |
//! |---|---|
//! | [`WarSteedParams::build`], [`DEFAULT_TURN_RATE`], [`DEFAULT_GEAR_CHANGE_RATE`] | §5.3 parameter block |
//! | [`target_velocity`], [`integrate`] | §5.4 target and integration |
//! | [`forward_limit`], [`speed_limit`] | §5.5 forward limit (world limit *W*) |
//! | [`turn_degrees`] | §5.6 turning |
//! | [`sliders`] | §5.7 published sliders |
//! | [`step`], [`StepInput`] | one update: §5.4–§5.7 together |
//!
//! The displacement, physics velocity and contacts that follow (`collision.md` §11.6 items
//! 3–7) are the player controller's and are not repeated here. Server blending (§5.9) is
//! out of scope.
//!
//! The strafe slots *S*1 and *S*2 are read from the left and right strafe speeds (the
//! pairing is Inferred in §5.3; every observed layer has equal strafe speeds, so it has no
//! effect on the install's data).

use super::params::MovementParams;

/// The default turn rate *R* of the parameter block, degrees per second (§5.3; a client
/// constant, decision 0012).
pub const DEFAULT_TURN_RATE: f32 = 150.0;
/// The default gear-change rate *G* of the parameter block (§5.3; a client constant).
pub const DEFAULT_GEAR_CHANGE_RATE: f32 = 2.0;
/// The input threshold shared with the player controller (§5.2, `collision.md` §11.8).
pub const INPUT_THRESHOLD: f32 = 0.0002;
/// Gait key Neutral (`game-controls.md` §5.3.2).
pub const GAIT_NEUTRAL: u32 = 2;
/// Gait key Neutral_Forwards: gaits above it are forward gaits (§5.4).
pub const GAIT_NEUTRAL_FORWARDS: u32 = 3;

/// The actor's properties the controller reads (§5.3–§5.7), as found on the actor (`None`
/// when absent or not of the expected type).
#[derive(Debug, Clone, Copy, PartialEq, Default)]
pub struct WarSteedActor {
    /// `Mount_Movement_Max_Speed_Cached`.
    pub max_speed_cached: Option<f32>,
    /// `Mount_Movement_Acceleration_Cached`.
    pub acceleration_cached: Option<f32>,
    /// `Mount_Movement_Turn_Rate_Cached`.
    pub turn_rate_cached: Option<f32>,
    /// `Mount_Movement_Gear_Change_Rate_Cached`.
    pub gear_change_rate_cached: Option<f32>,
    /// `Movement_SpeedMultiplier`.
    pub speed_multiplier: Option<f32>,
    /// `Movement_AccelerationMultiplier`.
    pub acceleration_multiplier: Option<f32>,
    /// `MountMovement_CurrentSpeed` (a gait key; Neutral when absent).
    pub gait: Option<u32>,
    /// `MountedCombat_Current_LockedGear` (a gait key).
    pub locked_gear: Option<u32>,
    /// `GOP_MountedMovement_ReleasedKeys_LoseMomentum`.
    pub lose_momentum: Option<bool>,
    /// `Movement_FreezeRotation`.
    pub freeze_rotation: Option<bool>,
    /// `Movement_ActiveMovementFrozen`.
    pub active_movement_frozen: Option<bool>,
    /// `Movement_PlayerMovementFrozen`.
    pub player_movement_frozen: Option<bool>,
}

fn finite(v: Option<f32>) -> Option<f32> {
    v.filter(|f| f.is_finite())
}

/// The parameter block of §5.3.
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct WarSteedParams {
    /// *S*1: the strafe speed for *x* ≤ 0.
    pub strafe_1: f32,
    /// *S*2: the strafe speed for *x* > 0.
    pub strafe_2: f32,
    /// *F*: the forward speed.
    pub forward: f32,
    /// *B*: the backward speed.
    pub backward: f32,
    /// (*a*x, *a*y, *a*z): sideways, forward and vertical control acceleration.
    pub acceleration: [f32; 3],
    /// *R*: turn rate, degrees per second.
    pub turn_rate: f32,
    /// *G*: gear-change rate (no reader found, §5.8).
    pub gear_change_rate: f32,
}

impl WarSteedParams {
    /// Builds the block from the layered parameters and the actor (§5.3): forward speed,
    /// forward acceleration, turn rate and gear-change rate replaced by the actor's
    /// `_Cached` values when present (a negative turn rate becomes 0); forward and backward
    /// speed times `Movement_SpeedMultiplier`, the three accelerations times
    /// `Movement_AccelerationMultiplier` (each 1 when absent).
    #[must_use]
    pub fn build(layered: &MovementParams, actor: &WarSteedActor) -> Self {
        let speed = finite(actor.speed_multiplier).unwrap_or(1.0);
        let accel = finite(actor.acceleration_multiplier).unwrap_or(1.0);
        let mut a = layered.control_acceleration;
        if let Some(y) = finite(actor.acceleration_cached) {
            a[1] = y;
        }
        Self {
            strafe_1: layered.strafe_left_speed,
            strafe_2: layered.strafe_right_speed,
            forward: finite(actor.max_speed_cached).unwrap_or(layered.forward_speed) * speed,
            backward: layered.backward_speed * speed,
            acceleration: a.map(|c| c * accel),
            turn_rate: finite(actor.turn_rate_cached).map_or(DEFAULT_TURN_RATE, |r| r.max(0.0)),
            gear_change_rate: finite(actor.gear_change_rate_cached)
                .unwrap_or(DEFAULT_GEAR_CHANGE_RATE),
        }
    }
}

/// The gait multipliers of the mounted-combat control (`game-controls.md` §5.3.2) the
/// controller needs for one update.
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct GaitMultipliers {
    /// The speed multiplier of gait 7 (Gallop), 1.0 when the control lacks it.
    pub gallop_speed: f32,
    /// The speed multiplier of the actor's locked gear, when it has one.
    pub locked_gear_speed: Option<f32>,
    /// The turn multiplier plus add-mod (*T*g + *A*g) of the actor's current gait.
    pub turn: f32,
}

/// The forward limit *L* of §5.5: min(*W*, *M*Gallop × *F*). `world_limit` is the world's
/// `World_MountedCombat_MaxSpeed` (the dictionary default when the world has none,
/// Decision of §5.5).
#[must_use]
pub fn forward_limit(params: &WarSteedParams, gallop_speed: f32, world_limit: f32) -> f32 {
    world_limit.min(gallop_speed * params.forward)
}

/// The forward limit *L*′ of §5.4 integration step 3: with a locked gear, that gear's
/// speed multiplier × *F* (the world limit does not apply); otherwise *L*.
#[must_use]
pub fn speed_limit(params: &WarSteedParams, gaits: &GaitMultipliers, world_limit: f32) -> f32 {
    match gaits.locked_gear_speed {
        Some(m) => m * params.forward,
        None => forward_limit(params, gaits.gallop_speed, world_limit),
    }
}

/// The target control velocity of §5.4 in the mover's frame, (sideways, forward). `input`
/// is (*x*, *y*) of `collision.md` §11.8, `limit` the forward limit *L* (without the locked
/// gear), `current` the stored control velocity.
#[must_use]
pub fn target_velocity(
    params: &WarSteedParams,
    actor: &WarSteedActor,
    input: [f32; 2],
    limit: f32,
    current: [f32; 2],
) -> [f32; 2] {
    let gait = actor.gait.unwrap_or(GAIT_NEUTRAL);
    let [x, y] = input;
    let side = if gait == GAIT_NEUTRAL {
        if x > 0.0 {
            x * params.strafe_2
        } else {
            x * params.strafe_1
        }
    } else {
        0.0
    };
    let forward = if y.abs() > INPUT_THRESHOLD {
        if y > 0.0 { limit } else { y * params.backward }
    } else if actor.lose_momentum != Some(true) && gait > GAIT_NEUTRAL_FORWARDS {
        current[1]
    } else {
        0.0
    };
    [side, forward]
}

/// The integration of §5.4 for an update of `t` seconds: `v` moves toward `target` by at
/// most *a*x·*t* sideways and *a*y·*t* forward; returns (mean control velocity of the
/// update, new stored velocity), both limited to −*S*1 … *S*2 sideways and −*B* … `limit`
/// forward (`limit` = *L*′, [`speed_limit`]).
#[must_use]
pub fn integrate(
    params: &WarSteedParams,
    v: [f32; 2],
    target: [f32; 2],
    t: f32,
    limit: f32,
) -> ([f32; 2], [f32; 2]) {
    let step = |cur: f32, goal: f32, a: f32| {
        let max = (a * t).abs();
        (goal - cur).clamp(-max, max)
    };
    let d = [
        step(v[0], target[0], params.acceleration[0]),
        step(v[1], target[1], params.acceleration[1]),
    ];
    let clamp = |w: [f32; 2]| {
        [
            clamp_range(w[0], -params.strafe_1, params.strafe_2),
            clamp_range(w[1], -params.backward, limit),
        ]
    };
    let mean = clamp([v[0] + d[0] / 2.0, v[1] + d[1] / 2.0]);
    let stored = clamp([v[0] + d[0], v[1] + d[1]]);
    (mean, stored)
}

/// `v` limited to [`lo`, `hi`]; a non-finite value or an empty range gives the nearer
/// usable bound (never NaN).
fn clamp_range(v: f32, lo: f32, hi: f32) -> f32 {
    let v = if v.is_finite() { v } else { 0.0 };
    if lo > hi {
        return hi;
    }
    v.clamp(lo, hi)
}

/// The turn of one update, degrees (§5.6): (*T*g + *A*g) × *R* × `t` × `turn_input`, when
/// |`turn_input`| > 0.0002 and `Movement_FreezeRotation` is not true; otherwise 0. (Mouse
/// steering is the caller's: it turns toward the heading by at most the same rate × `t`.)
#[must_use]
pub fn turn_degrees(
    params: &WarSteedParams,
    actor: &WarSteedActor,
    turn_multiplier: f32,
    turn_input: f32,
    t: f32,
) -> f32 {
    if actor.freeze_rotation == Some(true) || turn_input.abs() <= INPUT_THRESHOLD {
        return 0.0;
    }
    let d = turn_multiplier * params.turn_rate * t * turn_input;
    if d.is_finite() { d } else { 0.0 }
}

/// The published sliders of §5.7: (`Slider_Strafe`, `Slider_Propel`) from the stored control
/// velocity `v`. A divisor of magnitude at most 0.0002 gives 0; both are 0 while
/// `Movement_ActiveMovementFrozen` or `Movement_PlayerMovementFrozen` is true.
#[must_use]
pub fn sliders(
    params: &WarSteedParams,
    actor: &WarSteedActor,
    v: [f32; 2],
    limit: f32,
) -> (f32, f32) {
    if actor.active_movement_frozen == Some(true) || actor.player_movement_frozen == Some(true) {
        return (0.0, 0.0);
    }
    let ratio = |num: f32, div: f32| {
        if div.abs() <= INPUT_THRESHOLD {
            0.0
        } else {
            num / div
        }
    };
    let strafe = if v[0] > 0.0 {
        ratio(v[0], params.strafe_2)
    } else {
        ratio(v[0], params.strafe_1)
    };
    let propel = if v[1] > 0.0 {
        ratio(v[1], limit)
    } else {
        ratio(v[1], params.backward)
    };
    (strafe, propel)
}

/// The result of one update ([`step`]).
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct WarSteedStep {
    /// The update's mean control velocity (sideways, forward), for the displacement.
    pub mean: [f32; 2],
    /// The new stored control velocity.
    pub velocity: [f32; 2],
    /// The turn of this update, degrees.
    pub turn_degrees: f32,
    /// `Slider_Strafe`.
    pub strafe_slider: f32,
    /// `Slider_Propel`: the speed as a fraction of the current limit.
    pub propel_slider: f32,
    /// The forward limit used for the target (*L*).
    pub limit: f32,
}

/// The per-update inputs of [`step`].
#[derive(Debug, Clone, Copy, PartialEq, Default)]
pub struct StepInput {
    /// The input pair (*x*, *y*) of `collision.md` §11.8.
    pub input: [f32; 2],
    /// Turn right − turn left.
    pub turn_input: f32,
    /// The stored control velocity (sideways, forward).
    pub velocity: [f32; 2],
    /// The update's length, seconds.
    pub seconds: f32,
}

/// One update of the war-steed controller (§5.4–§5.7): target, integration, turn and
/// sliders.
#[must_use]
pub fn step(
    params: &WarSteedParams,
    actor: &WarSteedActor,
    gaits: &GaitMultipliers,
    world_limit: f32,
    update: StepInput,
) -> WarSteedStep {
    let StepInput {
        input,
        turn_input,
        velocity,
        seconds: t,
    } = update;
    let t = if t.is_finite() { t.max(0.0) } else { 0.0 };
    let input = input.map(|c| {
        if c.is_finite() {
            c.clamp(-1.0, 1.0)
        } else {
            0.0
        }
    });
    let velocity = velocity.map(|c| if c.is_finite() { c } else { 0.0 });
    let limit = forward_limit(params, gaits.gallop_speed, world_limit);
    let target = target_velocity(params, actor, input, limit, velocity);
    let limit_prime = speed_limit(params, gaits, world_limit);
    let (mean, stored) = integrate(params, velocity, target, t, limit_prime);
    let (strafe_slider, propel_slider) = sliders(params, actor, stored, limit);
    WarSteedStep {
        mean,
        velocity: stored,
        turn_degrees: turn_degrees(params, actor, gaits.turn, turn_input, t),
        strafe_slider,
        propel_slider,
        limit,
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn layered() -> MovementParams {
        MovementParams {
            forward_speed: 6.5,
            backward_speed: 2.0,
            strafe_left_speed: 3.0,
            strafe_right_speed: 3.0,
            control_acceleration: [10.0, 10.0, 0.0],
            ..MovementParams::default()
        }
    }

    fn actor() -> WarSteedActor {
        WarSteedActor {
            max_speed_cached: Some(8.3),
            acceleration_cached: Some(16.0),
            turn_rate_cached: Some(120.0),
            speed_multiplier: Some(1.2),
            ..WarSteedActor::default()
        }
    }

    fn close(a: f32, b: f32) -> bool {
        (a - b).abs() < 1e-4
    }

    #[test]
    fn worked_example_parameters_and_limit() {
        // mounts.md §11.2.
        let p = WarSteedParams::build(&layered(), &actor());
        assert!(close(p.forward, 9.96));
        assert!(close(p.backward, 2.4));
        assert_eq!(p.acceleration, [10.0, 16.0, 0.0]);
        assert!(close(p.turn_rate, 120.0));
        assert!(close(p.gear_change_rate, 2.0));
        assert!(close(forward_limit(&p, 1.0, 16.0), 9.96));
        let locked = GaitMultipliers {
            gallop_speed: 1.0,
            locked_gear_speed: Some(0.7),
            turn: 1.0,
        };
        assert!(close(speed_limit(&p, &locked, 16.0), 6.972));
        // Defaults without the actor's values.
        let d = WarSteedParams::build(&layered(), &WarSteedActor::default());
        assert!(close(d.turn_rate, DEFAULT_TURN_RATE) && close(d.forward, 6.5));
        let neg = WarSteedActor {
            turn_rate_cached: Some(-5.0),
            ..WarSteedActor::default()
        };
        assert!(close(
            WarSteedParams::build(&layered(), &neg).turn_rate,
            0.0
        ));
    }

    #[test]
    fn worked_example_one_update() {
        // mounts.md §11.3.
        let p = WarSteedParams::build(&layered(), &actor());
        let a = WarSteedActor {
            gait: Some(7),
            ..actor()
        };
        let g = GaitMultipliers {
            gallop_speed: 1.0,
            locked_gear_speed: None,
            turn: 0.47,
        };
        let s = step(
            &p,
            &a,
            &g,
            16.0,
            StepInput {
                input: [0.57, 1.0],
                turn_input: 1.0,
                velocity: [0.0, 5.0],
                seconds: 0.1,
            },
        );
        assert!(close(s.mean[0], 0.0) && close(s.mean[1], 5.8));
        assert!(close(s.velocity[0], 0.0) && close(s.velocity[1], 6.6));
        assert!(close(s.propel_slider, 6.6 / 9.96));
        assert!(close(s.turn_degrees, 5.64));
    }

    #[test]
    fn momentum_strafe_and_freezes() {
        let p = WarSteedParams::build(&layered(), &actor());
        let neutral = WarSteedActor::default();
        // Neutral: strafes, no forward target without input.
        assert_eq!(
            target_velocity(&p, &neutral, [1.0, 0.0], 9.96, [0.0, 3.0]),
            [3.0, 0.0]
        );
        // A forward gait keeps momentum unless the actor loses it on release.
        let canter = WarSteedActor {
            gait: Some(6),
            ..WarSteedActor::default()
        };
        assert_eq!(
            target_velocity(&p, &canter, [1.0, 0.0], 9.96, [0.0, 3.0]),
            [0.0, 3.0]
        );
        let lose = WarSteedActor {
            lose_momentum: Some(true),
            ..canter
        };
        assert_eq!(
            target_velocity(&p, &lose, [0.0, 0.0], 9.96, [0.0, 3.0]),
            [0.0, 0.0]
        );
        // Backwards input scales B.
        assert_eq!(
            target_velocity(&p, &neutral, [0.0, -0.5], 9.96, [0.0, 0.0]),
            [0.0, -1.2]
        );
        let frozen = WarSteedActor {
            freeze_rotation: Some(true),
            player_movement_frozen: Some(true),
            ..WarSteedActor::default()
        };
        assert!(close(turn_degrees(&p, &frozen, 1.0, 1.0, 1.0), 0.0));
        assert_eq!(sliders(&p, &frozen, [1.0, 1.0], 9.96), (0.0, 0.0));
    }

    #[test]
    fn velocities_stay_within_limits() {
        // MI13 (Decision): never above the forward limit nor outside −S1 … S2.
        let p = WarSteedParams::build(&layered(), &actor());
        let g = GaitMultipliers {
            gallop_speed: 1.0,
            locked_gear_speed: Some(0.5),
            turn: 1.0,
        };
        let s = step(
            &p,
            &actor(),
            &g,
            16.0,
            StepInput {
                input: [1.0, 1.0],
                turn_input: 0.0,
                velocity: [50.0, 50.0],
                seconds: 10.0,
            },
        );
        assert!(s.velocity[1] <= 0.5 * p.forward + 1e-5);
        assert!(s.velocity[0] <= p.strafe_2 + 1e-5);
        let s = step(
            &p,
            &actor(),
            &g,
            16.0,
            StepInput {
                input: [f32::NAN, 1.0],
                turn_input: f32::INFINITY,
                velocity: [f32::NAN, 1.0],
                seconds: f32::NAN,
            },
        );
        assert!(s.velocity.iter().all(|c| c.is_finite()) && s.turn_degrees.is_finite());
    }
}
