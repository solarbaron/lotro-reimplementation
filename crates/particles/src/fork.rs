// SPDX-License-Identifier: GPL-3.0-or-later
//! Fork effects (prefix 0x44) at run time (`particles.md` §6.5, rule §10.2;
//! `particle-runtime.md` §10).
//!
//! The effect image belongs to a third-party library whose layout and simulation are OPEN
//! (`particles.md` Q1), so Tessera does not simulate or draw fork effects (rule §10.2).
//! [`ForkEffect`] keeps what the client knows without the library: the never-ending
//! classification (`flag`), the start and stop times, the expiry rule of `tail_value`, the
//! node's fade settings, and the effect matrix built from the owner's frame (§6.5).
//! [`fork_shader_key`] is the blend-mode table of `particle-runtime.md` §10.2.
//!
//! Tessera rule of `particle-runtime.md` §10.2 (without the fork library): a fork effect
//! counts as active until the `tail_value` rule expires it (the library's "no live
//! particles" exit is never taken), and since it is local-space and gets no draw instance,
//! the release rule of §7.2 step 3 destroys it at once instead of letting it linger.

use glam::{Mat4, Vec3, Vec4};
use tessera_formats::gameplay::particles::ForkSystem;

/// The UNIQUEDB key of the fork shader for a draw batch's blend-mode index
/// (`particle-runtime.md` §10.2), or `None` (nothing is drawn).
#[must_use]
pub fn fork_shader_key(blend_index: u32) -> Option<u32> {
    match blend_index {
        0 => Some(0x15C),
        1 => Some(0x15E),
        2 => Some(0x15F),
        3 => Some(0x15D),
        4 => Some(0x159),
        5 => Some(0x15A),
        6 => Some(0x15B),
        _ => None,
    }
}

/// Whether a batch with this blend-mode index binds the effect's textures (every index
/// but 2, `particle-runtime.md` §10.2).
#[must_use]
pub fn fork_binds_textures(blend_index: u32) -> bool {
    blend_index != 2
}

/// The name the client asks the library for a record's effect by: `0x` and eight
/// upper-case hexadecimal digits of the DID (`particles.md` §6.5).
#[must_use]
pub fn fork_image_name(did: u32) -> String {
    format!("0x{did:08X}")
}

/// Distance-fade settings of a fork effect (`particles.md` §6.5): from the Particle node
/// that started it, or the defaults.
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct ForkFade {
    /// `ParticleScript_ShouldFade`.
    pub enabled: bool,
    /// Far fade start and end, metres.
    pub far: [f32; 2],
    /// Near fade start and end, metres.
    pub near: [f32; 2],
}

impl Default for ForkFade {
    fn default() -> Self {
        Self {
            enabled: true,
            far: [40.0, 60.0],
            near: [0.0, 0.0],
        }
    }
}

/// One running fork effect (see the module docs).
#[derive(Debug, Clone, PartialEq)]
pub struct ForkEffect {
    /// `flag`: the effect loops until it is stopped.
    pub looping: bool,
    /// `tail_value`: the lifetime window, seconds.
    pub tail_value: f64,
    /// The stored `transform` (row-major as stored), when present.
    pub transform: Option<[f32; 16]>,
    /// Fade settings.
    pub fade: ForkFade,
    start: f64,
    stopped_age: Option<f64>,
}

impl ForkEffect {
    /// A fork effect of `record` started at `now`.
    #[must_use]
    pub fn new(record: &ForkSystem, now: f64, fade: ForkFade) -> Self {
        Self {
            looping: record.flag,
            tail_value: record.tail_value,
            transform: record.transform_matrix(),
            fade,
            start: if now.is_finite() { now } else { 0.0 },
            stopped_age: None,
        }
    }

    /// Tells the effect to stop at `now` (a second stop keeps the first).
    pub fn stop(&mut self, now: f64) {
        if self.stopped_age.is_none() && now.is_finite() {
            self.stopped_age = Some((now - self.start).max(0.0));
        }
    }

    /// Whether it has been stopped.
    #[must_use]
    pub fn is_stopped(&self) -> bool {
        self.stopped_age.is_some()
    }

    /// Whether the effect has expired at `now` (`particles.md` §6.5): it is not (looping
    /// and not stopped) and *w* > `tail_value`, with *w* = age − age at the stop (0 while
    /// not stopped). Without the library a fork effect counts as active until it expires.
    #[must_use]
    pub fn has_expired(&self, now: f64) -> bool {
        if self.looping && self.stopped_age.is_none() {
            return false;
        }
        let age = now - self.start;
        let w = age - self.stopped_age.unwrap_or(0.0);
        // A non-finite tail never expires by itself; NaN comparisons are false.
        w > self.tail_value
    }

    /// The effect matrix (`particles.md` §6.5): the stored matrix (rows as vectors, on the
    /// left) times the basis built from the owner's frame: row 0 the owner's X axis, row 1
    /// its Z axis, row 2 its Y axis negated, row 3 its position divided per axis by its
    /// scale (a scale below 2⁻²³ counts as 1). `owner` is the owner's world matrix (column
    /// vectors), `scale` its per-axis scale. The result is returned as a column-vector
    /// [`Mat4`].
    #[must_use]
    pub fn effect_matrix(&self, owner: Mat4, scale: Vec3) -> Mat4 {
        let axis = |v: Vec4| v.truncate().normalize_or_zero();
        let x = axis(owner.x_axis);
        let y = axis(owner.y_axis);
        let z = axis(owner.z_axis);
        let fix = |s: f32| {
            if s.abs() < f32::EPSILON || !s.is_finite() {
                1.0
            } else {
                s
            }
        };
        let pos = owner.w_axis.truncate() / Vec3::new(fix(scale.x), fix(scale.y), fix(scale.z));
        // Row-vector basis: rows become the columns of the column-vector form.
        let basis = Mat4::from_cols(
            x.extend(0.0),
            z.extend(0.0),
            (-y).extend(0.0),
            pos.extend(1.0),
        );
        let stored = self
            .transform
            .map_or(Mat4::IDENTITY, |m| Mat4::from_cols_array(&m));
        // Row vectors: v · S · B. In column-vector form that is Bᵀ · Sᵀ · v: `basis` holds
        // the rows of B as columns (Bᵀ), and reading the row-major stored array as columns
        // gives Sᵀ.
        basis * stored
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn fork(flag: bool, tail: f64) -> ForkSystem {
        ForkSystem {
            image: Vec::new(),
            flag,
            name: String::new(),
            name_table_hint: 0,
            name_table: Vec::new(),
            properties: tessera_formats::PropertySet::default(),
            transform: Vec::new(),
            resources_hint: 0,
            resources: Vec::new(),
            tail_value: tail,
            bounds_min: [0.0; 3],
            bounds_max: [0.0; 3],
        }
    }

    #[test]
    fn shader_table() {
        let keys: Vec<_> = (0..8).map(fork_shader_key).collect();
        assert_eq!(
            keys,
            vec![
                Some(0x15C),
                Some(0x15E),
                Some(0x15F),
                Some(0x15D),
                Some(0x159),
                Some(0x15A),
                Some(0x15B),
                None
            ]
        );
        assert!(!fork_binds_textures(2) && fork_binds_textures(0));
        assert_eq!(fork_image_name(0x4400_0091), "0x44000091");
    }

    #[test]
    fn expiry_follows_the_tail_value() {
        let once = ForkEffect::new(&fork(false, 2.0), 10.0, ForkFade::default());
        assert!(!once.has_expired(11.9));
        assert!(once.has_expired(12.1));
        let mut looping = ForkEffect::new(&fork(true, 2.0), 10.0, ForkFade::default());
        assert!(!looping.has_expired(100.0));
        looping.stop(50.0);
        assert!(!looping.has_expired(51.9));
        assert!(looping.has_expired(52.1));
        let nan = ForkEffect::new(&fork(false, f64::NAN), 0.0, ForkFade::default());
        assert!(!nan.has_expired(1e9));
    }

    #[test]
    fn effect_matrix_turns_up_into_the_second_axis() {
        let f = ForkEffect::new(&fork(false, 1.0), 0.0, ForkFade::default());
        let m = f.effect_matrix(Mat4::IDENTITY, Vec3::ONE);
        // The library's second axis is the owner's up axis.
        assert!(m.transform_vector3(Vec3::Y).abs_diff_eq(Vec3::Z, 1e-6));
        assert!(m.transform_vector3(Vec3::Z).abs_diff_eq(-Vec3::Y, 1e-6));
        let moved = f.effect_matrix(
            Mat4::from_translation(Vec3::new(2.0, 4.0, 6.0)),
            Vec3::splat(2.0),
        );
        assert!(
            moved
                .transform_point3(Vec3::ZERO)
                .abs_diff_eq(Vec3::new(1.0, 2.0, 3.0), 1e-6)
        );
    }
}
