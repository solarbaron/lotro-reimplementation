// SPDX-License-Identifier: GPL-3.0-or-later
//! The client's global wind vector (`particles.md` §8.8, physics flag 0x20).
//!
//! One vector, starting at (0, 0, 0). Every environment update replaces its target; the
//! eased value at that moment becomes the new starting value, and the value then eases
//! towards the target with a half-life of [`HALF_LIFE`] seconds, snapping to the target once
//! it is within the client's tolerance on every axis. How the environment derives its wind
//! is OPEN (`particles.md` Q13), so the caller supplies the target.

use glam::Vec3;
use tessera_formats::gameplay::particles::TOLERANCE;

/// Half-life of the wind easing, seconds (§8.8, a client constant).
pub const HALF_LIFE: f64 = 0.3;

/// The eased global wind.
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct Wind {
    from: Vec3,
    target: Vec3,
    since: f64,
}

impl Default for Wind {
    fn default() -> Self {
        Self {
            from: Vec3::ZERO,
            target: Vec3::ZERO,
            since: 0.0,
        }
    }
}

impl Wind {
    /// A calm wind.
    #[must_use]
    pub fn new() -> Self {
        Self::default()
    }

    /// The eased value at clock time `now`.
    #[must_use]
    pub fn value(&self, now: f64) -> Vec3 {
        let t = (now - self.since).max(0.0);
        let k = if t.is_finite() {
            1.0 - (-t / HALF_LIFE).exp2()
        } else {
            1.0
        };
        let v = self.from + (self.target - self.from) * k as f32;
        if (v - self.target).abs().max_element() <= TOLERANCE || !v.is_finite() {
            self.target
        } else {
            v
        }
    }

    /// A new target from the environment update at `now` (non-finite targets count as
    /// calm).
    pub fn set_target(&mut self, now: f64, target: Vec3) {
        let now = if now.is_finite() { now } else { self.since };
        self.from = self.value(now);
        self.since = now;
        self.target = if target.is_finite() {
            target
        } else {
            Vec3::ZERO
        };
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn eases_with_a_half_life() {
        let mut w = Wind::new();
        assert_eq!(w.value(5.0), Vec3::ZERO);
        w.set_target(10.0, Vec3::new(4.0, 0.0, 0.0));
        assert!((w.value(10.3).x - 2.0).abs() < 1e-5);
        assert!((w.value(10.6).x - 3.0).abs() < 1e-5);
        assert_eq!(w.value(30.0), Vec3::new(4.0, 0.0, 0.0));
        // A new target starts from the eased value.
        w.set_target(10.3, Vec3::ZERO);
        assert!((w.value(10.3).x - 2.0).abs() < 1e-5);
        assert!((w.value(10.6).x - 1.0).abs() < 1e-5);
        w.set_target(f64::NAN, Vec3::splat(f32::NAN));
        assert!(w.value(100.0).is_finite());
    }
}
