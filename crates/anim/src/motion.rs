// SPDX-License-Identifier: GPL-3.0-or-later
//! Root motion (`skeleton-anim.md` §7): in-place playback, or extracted motion from a
//! clip's reference frame (§7.3).

use glam::{Quat, Vec3};

use crate::clip::{BoundClip, Wrap, clip_time};

/// Whether a playing clip moves its entity (`skeleton-anim.md` §7.2–§7.3).
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Default)]
pub enum RootMotionMode {
    /// The entity is not moved by the clip; bones play as sampled. This is right for the
    /// treadmill locomotion cycles (the movement system moves the entity, §7.2) and for
    /// clips without extracted motion.
    #[default]
    InPlace,
    /// The entity is moved by the clip's extracted-motion reference frame, when it has one
    /// (§7.3). Bones still play as sampled.
    Translate,
}

/// A rigid motion of the entity relative to its own frame before the motion: first
/// translate by `translation` (entity-local axes), then rotate by `rotation`.
///
/// Apply it to a world placement (position *p*, orientation *R*) as
/// *p* ← *p* + *R* · `translation`, *R* ← *R* · `rotation`.
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct MotionDelta {
    /// Translation in the entity's frame before the motion (metres).
    pub translation: Vec3,
    /// Rotation (about the world up axis, +Z, for extracted motion).
    pub rotation: Quat,
}

impl Default for MotionDelta {
    fn default() -> Self {
        Self::IDENTITY
    }
}

impl MotionDelta {
    /// No motion.
    pub const IDENTITY: Self = Self {
        translation: Vec3::ZERO,
        rotation: Quat::IDENTITY,
    };

    /// This motion followed by `next` (expressed in the frame this one ends in).
    #[must_use]
    pub fn then(self, next: MotionDelta) -> MotionDelta {
        MotionDelta {
            translation: self.translation + self.rotation * next.translation,
            rotation: (self.rotation * next.rotation).normalize(),
        }
    }

    /// The opposite motion.
    #[must_use]
    pub fn inverse(self) -> MotionDelta {
        let r = self.rotation.conjugate();
        MotionDelta {
            translation: -(r * self.translation),
            rotation: r,
        }
    }

    /// This motion repeated `k` times (by squaring; `k` is capped at 2³⁰).
    #[must_use]
    pub fn repeat(self, k: u64) -> MotionDelta {
        let mut k = k.min(1 << 30);
        let mut base = self;
        let mut acc = MotionDelta::IDENTITY;
        while k > 0 {
            if k & 1 == 1 {
                acc = acc.then(base);
            }
            base = base.then(base);
            k >>= 1;
        }
        acc
    }

    /// A partial motion: `weight` × translation and the rotation scaled by `weight`
    /// (normalised linear interpolation from identity); for blending motions of layers.
    #[must_use]
    pub fn scaled(self, weight: f32) -> MotionDelta {
        let w = if weight.is_finite() {
            weight.clamp(0.0, 1.0)
        } else {
            0.0
        };
        MotionDelta {
            translation: self.translation * w,
            rotation: Quat::IDENTITY.lerp(self.rotation, w).normalize(),
        }
    }

    /// The yaw angle of the rotation about +Z, in radians (counter-clockwise seen from
    /// above).
    #[must_use]
    pub fn yaw(self) -> f32 {
        let fwd = self.rotation * Vec3::X;
        fwd.y.atan2(fwd.x)
    }

    /// Whether every value is finite.
    #[must_use]
    pub fn is_finite(self) -> bool {
        self.translation.is_finite() && self.rotation.is_finite()
    }
}

/// The motion of the entity at clip time `t` relative to its frame at the start of the
/// clip (or cycle): **E**(*t*) = T(*p*(*t*) − *p*(0)) · R_z(*w*(*t*) − *w*(0)), where
/// (*p*, *w*) is the reference-frame sample on the clip's frame grid (holding the last
/// sample when the array is short, §7.3) and R_z the rotation by angle *w* about the world
/// up axis +Z, as the "Use" paragraph of §7.3 says. The frame's stored `up` member is not
/// used: it is (0, 0, 1) in all but 7 leading frames, whose meaning is OPEN. `None` when
/// the clip has no extracted motion.
///
/// The sign of *w* follows the active-rotation convention: positive is counter-clockwise
/// seen from above (a left turn for a character facing +Y). See the OPEN in §7.3.
#[must_use]
pub fn motion_at(clip: &BoundClip, t: f32) -> Option<MotionDelta> {
    let (rf, _) = clip.clip().reference_frame()?;
    if rf.samples.is_empty() {
        return None;
    }
    let frames = clip.frame_count();
    let duration = clip.duration();
    let s0 = rf.sample_on_grid(0.0, frames, duration);
    let s = rf.sample_on_grid(t, frames, duration);
    let translation = Vec3::new(s[0] - s0[0], s[1] - s0[1], s[2] - s0[2]);
    let angle = s[3] - s0[3];
    let m = MotionDelta {
        translation,
        rotation: Quat::from_rotation_z(angle),
    };
    m.is_finite().then_some(m)
}

/// Splits play time `tau` of a looping clip of duration `d` (finite, > 0) into a whole
/// cycle index *c* and an in-cycle time *t* in `0 ..= d` with *c* · `d` + *t* = `tau`
/// (up to rounding), both from one computation so that they never disagree about which
/// cycle a time a hair below a boundary belongs to. Computed in `f64`: every `f32` product
/// *c* · `d` with |*c*| < 2²⁹ is exact there.
fn split_cycle(tau: f32, d: f32) -> (f64, f32) {
    let (tau, d64) = (f64::from(tau), f64::from(d));
    let mut c = (tau / d64).floor();
    let mut t = tau - c * d64;
    if t >= d64 {
        c += 1.0;
        t -= d64;
    }
    if t < 0.0 {
        c -= 1.0;
        t += d64;
    }
    // Rounding to f32 may give exactly `d`: still the end of cycle `c`, which is the same
    // motion as the start of cycle `c + 1`.
    (c, (t as f32).clamp(0.0, d))
}

/// The motion of an entity playing `clip` from play time `tau0` to `tau1` under `wrap`
/// (`skeleton-anim.md` §7.3): the change of the sampled reference frame since the previous
/// update, as a [`MotionDelta`] in the entity's frame at `tau0`. A looping clip restarts
/// its reference frame each cycle, so whole cycles in between each add the clip's full
/// motion. Playing backwards (`tau1 < tau0`) gives the inverse motion. Identity when the
/// clip has no extracted motion or the times are not finite.
#[must_use]
pub fn motion_between(clip: &BoundClip, tau0: f32, tau1: f32, wrap: Wrap) -> MotionDelta {
    if !(tau0.is_finite() && tau1.is_finite()) || !clip.has_extracted_motion() {
        return MotionDelta::IDENTITY;
    }
    if tau1 < tau0 {
        return motion_between(clip, tau1, tau0, wrap).inverse();
    }
    let d = clip.duration();
    let at = |t: f32| motion_at(clip, t).unwrap_or(MotionDelta::IDENTITY);
    let within = |t0: f32, t1: f32| at(t0).inverse().then(at(t1));
    let r = match wrap {
        Wrap::Once => within(clip_time(tau0, d, wrap), clip_time(tau1, d, wrap)),
        Wrap::Loop => {
            if !(d.is_finite() && d > 0.0) {
                return MotionDelta::IDENTITY;
            }
            let (c0, t0) = split_cycle(tau0, d);
            let (c1, t1) = split_cycle(tau1, d);
            if c1 <= c0 {
                within(t0, t1)
            } else {
                let full = at(d);
                let whole = (c1 - c0 - 1.0).clamp(0.0, (1u64 << 30) as f64) as u64;
                within(t0, d).then(full.repeat(whole)).then(at(t1))
            }
        }
    };
    if r.is_finite() {
        r
    } else {
        MotionDelta::IDENTITY
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn delta_algebra() {
        let a = MotionDelta {
            translation: Vec3::new(1.0, 0.0, 0.0),
            rotation: Quat::from_rotation_z(std::f32::consts::FRAC_PI_2),
        };
        let b = a.then(a);
        assert!(b.translation.abs_diff_eq(Vec3::new(1.0, 1.0, 0.0), 1e-6));
        assert!((b.yaw() - std::f32::consts::PI).abs() < 1e-5);
        let id = a.then(a.inverse());
        assert!(id.translation.abs_diff_eq(Vec3::ZERO, 1e-6));
        assert!(id.rotation.abs_diff_eq(Quat::IDENTITY, 1e-6));
        let four = a.repeat(4);
        assert!(four.translation.abs_diff_eq(Vec3::ZERO, 1e-5));
        assert_eq!(a.repeat(0), MotionDelta::IDENTITY);
        let half = a.scaled(0.5);
        assert!((half.yaw() - std::f32::consts::FRAC_PI_4).abs() < 1e-5);
        assert_eq!(a.scaled(f32::NAN), MotionDelta::IDENTITY);
    }

    #[test]
    fn cycle_split_is_consistent() {
        // Times a hair below a boundary, where floor(tau / d) and rem_euclid disagree in
        // f32 (review findings: tau = -7.45e-9 with d = 2, and tau = 0.4 with d = 4/30).
        let d = 0.133_333_34f32;
        for tau in [
            -7.45e-9f32,
            -1e-9,
            0.4,
            0.8,
            0.2,
            f32::from_bits(0.4f32.to_bits() - 1),
            3.0 * d,
            -3.0 * d,
            1e6,
        ] {
            for d in [d, 2.0, 0.066_6, 1.0] {
                let (c, t) = split_cycle(tau, d);
                assert!((0.0..=d).contains(&t), "{tau} {d}: {t}");
                let back = c * f64::from(d) + f64::from(t);
                assert!(
                    (back - f64::from(tau)).abs() <= 1e-6 * (1.0 + f64::from(tau).abs()),
                    "{tau} {d}: {c} {t}"
                );
            }
        }
    }
}
