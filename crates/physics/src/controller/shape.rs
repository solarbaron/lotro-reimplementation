// SPDX-License-Identifier: GPL-3.0-or-later
//! The character's collision shape (`collision.md` §10.6) and the support probe of §11.3.

use crate::error::{PhysicsError, Result};
use crate::geometry::Capsule;

use super::contact::SUPPORT_MIN_RADIUS;

/// Largest capsule radius accepted, m (Tessera bound; playable capsules are 0.33–0.5 in the
/// setup and 0.35–0.36 after the model scale, §10.6).
pub const MAX_CAPSULE_RADIUS: f32 = 5.0;
/// Largest capsule length (cap-centre distance) and lower-cap height accepted, m (Tessera
/// bound; playable values are at most 1.59 in the setup).
pub const MAX_CAPSULE_LENGTH: f32 = 20.0;

/// The mover's vertical capsule relative to its feet: the lower cap centre lies
/// `lower_cap_z` above the feet, the upper one `length` above that, both with `radius`.
/// For a playable setup the lower cap centre height equals the radius, so the capsule's
/// bottom is the feet (§10.6).
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct CharacterShape {
    lower_cap_z: f32,
    length: f32,
    radius: f32,
}

impl CharacterShape {
    /// The capsule shared, after the model scale, by Hobbit, Human, Dwarf, Stout-axe dwarf,
    /// River-hobbit and High-elf (§10.6): radius 0.35, lower cap centre 0.35, length 1.10
    /// (height 1.80).
    pub const PLAYABLE: Self = Self {
        lower_cap_z: 0.35,
        length: 1.10,
        radius: 0.35,
    };

    /// A shape; `InvalidInput` unless every value is finite, the radius is in
    /// (0, [`MAX_CAPSULE_RADIUS`]] and the length and lower-cap height are in
    /// [0, [`MAX_CAPSULE_LENGTH`]].
    ///
    /// # Errors
    ///
    /// [`PhysicsError::InvalidInput`].
    pub fn new(lower_cap_z: f32, length: f32, radius: f32) -> Result<Self> {
        let len_ok = |v: f32| v.is_finite() && (0.0..=MAX_CAPSULE_LENGTH).contains(&v);
        if !(radius.is_finite() && radius > 0.0 && radius <= MAX_CAPSULE_RADIUS) {
            return Err(PhysicsError::InvalidInput("character capsule radius"));
        }
        if !len_ok(length) || !len_ok(lower_cap_z) {
            return Err(PhysicsError::InvalidInput("character capsule length"));
        }
        Ok(Self {
            lower_cap_z,
            length,
            radius,
        })
    }

    /// The capsule of a setup (`setup.md` §6.4: lower cap centre z, length, radius) times
    /// the model scale (`appearance.md` §6.1), §10.6.
    ///
    /// # Errors
    ///
    /// [`PhysicsError::InvalidInput`] when a scaled value is out of range (see
    /// [`CharacterShape::new`]) or the scale is not finite and positive.
    pub fn from_setup_capsule(base_z: f32, length: f32, radius: f32, scale: f32) -> Result<Self> {
        if !(scale.is_finite() && scale > 0.0) {
            return Err(PhysicsError::InvalidInput("character model scale"));
        }
        Self::new(base_z * scale, length * scale, radius * scale)
    }

    /// Height of the lower cap centre above the feet.
    #[must_use]
    pub fn lower_cap_z(&self) -> f32 {
        self.lower_cap_z
    }

    /// Cap-centre distance.
    #[must_use]
    pub fn length(&self) -> f32 {
        self.length
    }

    /// Radius.
    #[must_use]
    pub fn radius(&self) -> f32 {
        self.radius
    }

    /// Top of the capsule above the feet.
    #[must_use]
    pub fn height(&self) -> f32 {
        self.lower_cap_z + self.length + self.radius
    }

    /// The capsule for feet at `feet`.
    #[must_use]
    pub fn capsule_at(&self, feet: [f32; 3]) -> Capsule {
        Capsule::vertical(
            [feet[0], feet[1], feet[2] + self.lower_cap_z],
            self.length,
            self.radius,
        )
    }

    /// The support probe (§11.3: radius at least 0.3 m, the larger of 0.3 and the shape
    /// radius): a sphere whose bottom is the capsule's bottom.
    #[must_use]
    pub fn support_probe(&self, feet: [f32; 3]) -> Capsule {
        let r = self.radius.max(SUPPORT_MIN_RADIUS);
        let bottom = feet[2] + self.lower_cap_z - self.radius;
        let c = [feet[0], feet[1], bottom + r];
        Capsule {
            a: c,
            b: c,
            radius: r,
        }
    }
}

impl Default for CharacterShape {
    fn default() -> Self {
        Self::PLAYABLE
    }
}
