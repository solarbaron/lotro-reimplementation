// SPDX-License-Identifier: GPL-3.0-or-later
//! Movement controllers driven at a fixed time step
//! (`docs/specs/keymap.md` §9).
//!
//! World convention (`mesh.md` §11): right-handed, `+Z` up, metres. **Yaw**
//! is the heading in radians measured counter-clockwise from `+X` seen from
//! above (yaw `0` faces `+X`, `π/2` faces `+Y`); **pitch** is the angle
//! above the horizontal. Tessera's own convention; how it maps to the game's
//! map headings is a matter for the coordinates spec. Character models face
//! `+Y`, so a model is drawn rotated by [`model_rotation`]`(yaw)`.
//!
//! Both controllers keep the position and yaw of the previous tick so the
//! renderer can interpolate between ticks
//! ([`FixedTimestep::alpha`](crate::FixedTimestep::alpha)).
//!
//! These controllers depend only on `glam`; the world layer supplies ground
//! heights through a closure, so it needs nothing from this crate.

mod fly;
mod walk;

pub use fly::{FlyConfig, FlyController, MAX_COORD};
pub use walk::{
    GroundState, HeightQuery, LAND_TOLERANCE, MAX_SUBSTEPS, MAX_TICK_DT, WalkConfig,
    WalkController, WalkEvent,
};

use glam::Vec3;

/// What the player asks for during one tick, independent of devices.
/// Axis values are in `-1..=1`; see [`MoveIntent::sanitized`].
#[derive(Debug, Clone, Copy, PartialEq, Default)]
pub struct MoveIntent {
    /// Forward (`+`) / backward (`-`).
    pub forward: f32,
    /// Right (`+`) / left (`-`).
    pub strafe: f32,
    /// Turn left (`+`, counter-clockwise) / right (`-`), as a fraction of
    /// the controller's turn rate.
    pub turn: f32,
    /// Fly up (`+`) / down (`-`); ignored by the walk controller.
    pub vertical: f32,
    /// Jump was pressed (buffered by the walk controller).
    pub jump: bool,
    /// Walk instead of run (walk controller).
    pub walk: bool,
    /// Speed boost (fly controller).
    pub boost: bool,
    /// Precision speed (fly controller).
    pub slow: bool,
}

impl MoveIntent {
    /// Non-finite axes become `0`, others are clamped to `-1..=1`.
    #[must_use]
    pub fn sanitized(self) -> Self {
        let f = |v: f32| {
            if v.is_finite() {
                v.clamp(-1.0, 1.0)
            } else {
                0.0
            }
        };
        Self {
            forward: f(self.forward),
            strafe: f(self.strafe),
            turn: f(self.turn),
            vertical: f(self.vertical),
            ..self
        }
    }
}

/// Carries frame-rate input into fixed ticks: held axes are taken from the
/// latest frame, edge-triggered requests (`jump`) are kept until a tick
/// consumes them, so a press in a frame that runs no tick is not lost and a
/// frame that runs two ticks does not jump twice.
#[derive(Debug, Clone, Copy, PartialEq, Default)]
pub struct IntentLatch {
    latest: MoveIntent,
    jump: bool,
}

impl IntentLatch {
    /// Record this frame's intent.
    pub fn frame(&mut self, intent: MoveIntent) {
        self.latest = intent;
        self.jump |= intent.jump;
    }

    /// The intent for the next tick; consumes latched edges.
    pub fn tick(&mut self) -> MoveIntent {
        MoveIntent {
            jump: std::mem::take(&mut self.jump),
            ..self.latest
        }
    }
}

/// Unit heading vector on the ground plane for `yaw`.
#[must_use]
pub fn heading(yaw: f32) -> Vec3 {
    Vec3::new(yaw.cos(), yaw.sin(), 0.0)
}

/// Unit vector pointing right of `yaw` on the ground plane.
#[must_use]
pub fn right_of(yaw: f32) -> Vec3 {
    Vec3::new(yaw.sin(), -yaw.cos(), 0.0)
}

/// Unit view direction for `yaw` and `pitch`.
#[must_use]
pub fn view_direction(yaw: f32, pitch: f32) -> Vec3 {
    let (sp, cp) = pitch.sin_cos();
    Vec3::new(yaw.cos() * cp, yaw.sin() * cp, sp)
}

/// Interpolate from angle `a` to `b` along the shorter arc (`alpha` in
/// `0..=1`, non-finite means `1`); the result is wrapped into `-π..=π`.
#[must_use]
pub fn lerp_angle(a: f32, b: f32, alpha: f32) -> f32 {
    let t = if alpha.is_finite() {
        alpha.clamp(0.0, 1.0)
    } else {
        1.0
    };
    wrap_angle(a + wrap_angle(b - a) * t)
}

/// Rotation about `+Z` that turns a model authored facing `+Y` (the data's
/// character convention, `mesh.md` §11.1) to face `yaw` (§9.1: yaw `0`
/// faces `+X`): `yaw − π/2`, wrapped.
#[must_use]
pub fn model_rotation(yaw: f32) -> f32 {
    wrap_angle(yaw - std::f32::consts::FRAC_PI_2)
}

/// Wrap an angle into `-π..=π` (non-finite becomes `0`).
#[must_use]
pub fn wrap_angle(a: f32) -> f32 {
    use std::f32::consts::{PI, TAU};
    if !a.is_finite() {
        return 0.0;
    }
    let mut r = a % TAU;
    if r > PI {
        r -= TAU;
    } else if r < -PI {
        r += TAU;
    }
    r
}

/// A finite `dt` in `0..=max`.
pub(crate) fn clean_dt(dt: f32, max: f32) -> f32 {
    if dt.is_finite() {
        dt.clamp(0.0, max)
    } else {
        0.0
    }
}

/// Move `current` towards `target` by at most `max_delta`.
pub(crate) fn approach(current: Vec3, target: Vec3, max_delta: f32) -> Vec3 {
    let d = target - current;
    let len = d.length();
    if len <= max_delta || len <= f32::EPSILON {
        target
    } else {
        current + d * (max_delta / len)
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::f32::consts::{FRAC_PI_2, PI};

    #[test]
    fn conventions() {
        assert!((heading(0.0) - Vec3::X).length() < 1e-6);
        assert!((heading(FRAC_PI_2) - Vec3::Y).length() < 1e-6);
        assert!((right_of(0.0) - Vec3::NEG_Y).length() < 1e-6);
        assert!((view_direction(0.0, FRAC_PI_2) - Vec3::Z).length() < 1e-6);
        assert!((wrap_angle(3.0 * PI) - PI).abs() < 1e-5);
        assert_eq!(wrap_angle(f32::NAN), 0.0);
        // Shortest arc across ±π.
        let m = lerp_angle(PI - 0.1, -PI + 0.1, 0.5);
        assert!((m.abs() - PI).abs() < 1e-5, "{m}");
        assert!((lerp_angle(0.0, 1.0, 0.25) - 0.25).abs() < 1e-6);
        assert_eq!(lerp_angle(0.0, 1.0, f32::NAN), 1.0);
        // A model facing +Y rotated by model_rotation(yaw) faces heading(yaw).
        for yaw in [0.0, 1.0, -2.5, PI] {
            let r = model_rotation(yaw);
            let model_forward = Vec3::new(-r.sin(), r.cos(), 0.0);
            assert!((model_forward - heading(yaw)).length() < 1e-5, "{yaw}");
        }
        let i = MoveIntent {
            forward: f32::NAN,
            strafe: 5.0,
            ..MoveIntent::default()
        }
        .sanitized();
        assert_eq!((i.forward, i.strafe), (0.0, 1.0));
    }

    #[test]
    fn latch_keeps_edges_until_a_tick() {
        let mut l = IntentLatch::default();
        l.frame(MoveIntent {
            jump: true,
            forward: 1.0,
            ..MoveIntent::default()
        });
        // A frame without a tick, then a frame with two.
        l.frame(MoveIntent {
            forward: 0.5,
            ..MoveIntent::default()
        });
        let a = l.tick();
        let b = l.tick();
        assert!(a.jump && !b.jump);
        assert_eq!((a.forward, b.forward), (0.5, 0.5));
    }
}
