// SPDX-License-Identifier: GPL-3.0-or-later
//! Free-fly camera controller: no collision, moves along the view
//! direction.

use glam::Vec3;

use super::{MoveIntent, approach, clean_dt, lerp_angle, right_of, view_direction, wrap_angle};

/// Largest coordinate magnitude the controllers keep (beyond it positions
/// are clamped; `f32` world coordinates lose sub-unit precision long before).
pub const MAX_COORD: f32 = 1.0e7;

/// Tunables of [`FlyController`]. Tessera's own tuning; plain data for a
/// settings file.
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct FlyConfig {
    /// Base speed in units per second.
    pub speed: f32,
    /// Speed multiplier while boosting.
    pub boost_multiplier: f32,
    /// Speed multiplier in precision mode.
    pub slow_multiplier: f32,
    /// Acceleration in units per second² (`0` or less = instant).
    pub acceleration: f32,
    /// Keyboard turn rate in radians per second.
    pub turn_rate: f32,
    /// Pitch limit in radians (symmetric).
    pub max_pitch: f32,
    /// Speed range for [`FlyController::scale_speed`].
    pub min_speed: f32,
    /// See `min_speed`.
    pub max_speed: f32,
}

impl Default for FlyConfig {
    fn default() -> Self {
        Self {
            speed: 40.0,
            boost_multiplier: 5.0,
            slow_multiplier: 0.2,
            acceleration: 400.0,
            turn_rate: 2.0,
            max_pitch: 89f32.to_radians(),
            min_speed: 1.0,
            max_speed: 2000.0,
        }
    }
}

/// Free-fly controller state.
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct FlyController {
    /// Tunables.
    pub config: FlyConfig,
    position: Vec3,
    previous: Vec3,
    velocity: Vec3,
    yaw: f32,
    previous_yaw: f32,
    pitch: f32,
}

impl FlyController {
    /// A controller at `position` looking along `yaw`/`pitch`.
    #[must_use]
    pub fn new(position: Vec3, yaw: f32, pitch: f32, config: FlyConfig) -> Self {
        let mut c = Self {
            config,
            position: Vec3::ZERO,
            previous: Vec3::ZERO,
            velocity: Vec3::ZERO,
            yaw: 0.0,
            previous_yaw: 0.0,
            pitch: 0.0,
        };
        c.teleport(position);
        c.set_orientation(yaw, pitch);
        c
    }

    /// Eye position after the last tick.
    #[must_use]
    pub fn position(&self) -> Vec3 {
        self.position
    }

    /// Current velocity.
    #[must_use]
    pub fn velocity(&self) -> Vec3 {
        self.velocity
    }

    /// Heading in radians.
    #[must_use]
    pub fn yaw(&self) -> f32 {
        self.yaw
    }

    /// Pitch in radians.
    #[must_use]
    pub fn pitch(&self) -> f32 {
        self.pitch
    }

    /// Unit view direction.
    #[must_use]
    pub fn forward(&self) -> Vec3 {
        view_direction(self.yaw, self.pitch)
    }

    /// Position interpolated between the previous and the last tick
    /// (`alpha` in `0..=1`).
    #[must_use]
    pub fn interpolated(&self, alpha: f32) -> Vec3 {
        let a = if alpha.is_finite() {
            alpha.clamp(0.0, 1.0)
        } else {
            1.0
        };
        self.previous.lerp(self.position, a)
    }

    /// Heading interpolated between the previous and the last tick along
    /// the shorter arc (keyboard turning is applied per tick; per-frame
    /// [`look`](Self::look) shifts both ends and shows at once).
    #[must_use]
    pub fn interpolated_yaw(&self, alpha: f32) -> f32 {
        lerp_angle(self.previous_yaw, self.yaw, alpha)
    }

    /// Jump to `position` without interpolation; stops motion.
    pub fn teleport(&mut self, position: Vec3) {
        let p = clamp_pos(position);
        self.position = p;
        self.previous = p;
        self.velocity = Vec3::ZERO;
    }

    /// Set the view angles (pitch clamped; no interpolation).
    pub fn set_orientation(&mut self, yaw: f32, pitch: f32) {
        self.yaw = wrap_angle(yaw);
        self.previous_yaw = self.yaw;
        self.pitch = self.clamp_pitch(pitch);
    }

    /// Apply a look change in radians (positive yaw turns left, positive
    /// pitch looks up). Meant to be called every frame, outside the fixed
    /// step, so the view responds immediately.
    pub fn look(&mut self, dyaw: f32, dpitch: f32) {
        let dy = if dyaw.is_finite() { dyaw } else { 0.0 };
        let dp = if dpitch.is_finite() { dpitch } else { 0.0 };
        self.yaw = wrap_angle(self.yaw + dy);
        self.previous_yaw = wrap_angle(self.previous_yaw + dy);
        self.pitch = self.clamp_pitch(self.pitch + dp);
    }

    /// Multiply the base speed by `2^(notches / 4)` within the configured
    /// range (for a speed wheel).
    pub fn scale_speed(&mut self, notches: f32) {
        if !notches.is_finite() {
            return;
        }
        let lo = self.config.min_speed.max(f32::MIN_POSITIVE);
        let hi = self.config.max_speed.max(lo);
        let s = self.config.speed * 2f32.powf(notches.clamp(-64.0, 64.0) / 4.0);
        self.config.speed = if s.is_finite() { s.clamp(lo, hi) } else { lo };
    }

    fn clamp_pitch(&self, p: f32) -> f32 {
        let lim = if self.config.max_pitch.is_finite() {
            self.config.max_pitch.clamp(0.0, 89.9f32.to_radians())
        } else {
            0.0
        };
        if p.is_finite() {
            p.clamp(-lim, lim)
        } else {
            0.0
        }
    }

    /// Advance one fixed step of `dt` seconds.
    pub fn tick(&mut self, intent: &MoveIntent, dt: f32) {
        let dt = clean_dt(dt, 1.0);
        let i = intent.sanitized();
        self.previous = self.position;
        self.previous_yaw = self.yaw;
        let turn_rate = finite_or(self.config.turn_rate, 0.0);
        self.yaw = wrap_angle(self.yaw + i.turn * turn_rate * dt);

        let fwd = view_direction(self.yaw, self.pitch);
        let mut wish = fwd * i.forward + right_of(self.yaw) * i.strafe + Vec3::Z * i.vertical;
        if wish.length_squared() > 1.0 {
            wish = wish.normalize_or_zero();
        }
        let mut speed = finite_or(self.config.speed, 0.0).max(0.0);
        if i.boost {
            speed *= finite_or(self.config.boost_multiplier, 1.0).max(0.0);
        }
        if i.slow {
            speed *= finite_or(self.config.slow_multiplier, 1.0).max(0.0);
        }
        let speed = speed.min(1.0e6);
        let target = wish * speed;
        let accel = finite_or(self.config.acceleration, 0.0);
        self.velocity = if accel <= 0.0 {
            target
        } else {
            approach(self.velocity, target, accel * dt)
        };
        if !self.velocity.is_finite() {
            self.velocity = Vec3::ZERO;
        }
        self.position = clamp_pos(self.position + self.velocity * dt);
    }
}

fn finite_or(v: f32, d: f32) -> f32 {
    if v.is_finite() { v } else { d }
}

pub(crate) fn clamp_pos(p: Vec3) -> Vec3 {
    if p.is_finite() {
        p.clamp(Vec3::splat(-MAX_COORD), Vec3::splat(MAX_COORD))
    } else {
        Vec3::ZERO
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::f32::consts::FRAC_PI_2;

    fn instant() -> FlyConfig {
        FlyConfig {
            acceleration: 0.0,
            ..FlyConfig::default()
        }
    }

    #[test]
    fn flies_along_view() {
        let mut c = FlyController::new(Vec3::ZERO, FRAC_PI_2, 0.0, instant());
        let i = MoveIntent {
            forward: 1.0,
            ..MoveIntent::default()
        };
        for _ in 0..30 {
            c.tick(&i, 1.0 / 30.0);
        }
        // One second at 40 u/s along +Y.
        assert!(
            (c.position() - Vec3::new(0.0, 40.0, 0.0)).length() < 1e-3,
            "{}",
            c.position()
        );
        c.set_orientation(0.0, FRAC_PI_2);
        assert!(c.pitch() < FRAC_PI_2, "pitch is clamped");
        c.tick(&i, 1.0);
        assert!(c.position().z > 39.0);
    }

    #[test]
    fn boost_slow_vertical_and_diagonal_normalized() {
        let mut c = FlyController::new(Vec3::ZERO, 0.0, 0.0, instant());
        let i = MoveIntent {
            forward: 1.0,
            strafe: 1.0,
            vertical: 1.0,
            boost: true,
            ..MoveIntent::default()
        };
        c.tick(&i, 0.1);
        assert!((c.velocity().length() - 200.0).abs() < 1e-2);
        let i = MoveIntent {
            vertical: -1.0,
            slow: true,
            ..MoveIntent::default()
        };
        c.tick(&i, 0.1);
        assert!((c.velocity() - Vec3::new(0.0, 0.0, -8.0)).length() < 1e-4);
    }

    #[test]
    fn acceleration_and_interpolation() {
        let mut c = FlyController::new(Vec3::ZERO, 0.0, 0.0, FlyConfig::default());
        let i = MoveIntent {
            forward: 1.0,
            ..MoveIntent::default()
        };
        c.tick(&i, 0.05);
        assert!((c.velocity().x - 20.0).abs() < 1e-3, "400 u/s² for 0.05 s");
        let p0 = c.interpolated(0.0);
        let p1 = c.interpolated(1.0);
        assert_eq!(p1, c.position());
        assert!(p0.x < p1.x);
        assert_eq!(c.interpolated(f32::NAN), c.position());
    }

    #[test]
    fn keyboard_turn_interpolates_look_does_not_lag() {
        let mut c = FlyController::new(Vec3::ZERO, 0.0, 0.0, instant());
        let i = MoveIntent {
            turn: 1.0,
            ..MoveIntent::default()
        };
        c.tick(&i, 0.1);
        assert_eq!(c.interpolated_yaw(0.0), 0.0);
        assert!((c.interpolated_yaw(0.5) - 0.1).abs() < 1e-6);
        assert!((c.interpolated_yaw(1.0) - 0.2).abs() < 1e-6);
        c.look(0.3, 0.0);
        assert!((c.interpolated_yaw(0.0) - 0.3).abs() < 1e-6);
        assert!((c.interpolated_yaw(1.0) - 0.5).abs() < 1e-6);
    }

    #[test]
    fn hostile_numbers() {
        let mut c = FlyController::new(Vec3::splat(f32::NAN), f32::INFINITY, f32::NAN, instant());
        assert_eq!(c.position(), Vec3::ZERO);
        c.look(f32::NAN, 1e30);
        assert!(c.pitch().is_finite() && c.yaw().is_finite());
        c.config.speed = f32::INFINITY;
        c.tick(
            &MoveIntent {
                forward: 1.0,
                ..MoveIntent::default()
            },
            f32::MAX,
        );
        assert!(c.position().is_finite());
        c.teleport(Vec3::splat(1e30));
        assert_eq!(c.position(), Vec3::splat(MAX_COORD));
        c.scale_speed(8.0);
        assert!(c.config.speed.is_finite());
        c.config.speed = 40.0;
        c.scale_speed(4.0);
        assert!((c.config.speed - 80.0).abs() < 1e-3);
    }
}
