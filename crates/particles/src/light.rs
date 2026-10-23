// SPDX-License-Identifier: GPL-3.0-or-later
//! The lights of lit systems (`particles.md` §8.14; `particle-runtime.md` §12 rule 13).
//!
//! A system with `use_lighting` = 1 multiplies each particle's colour (after the fade
//! envelope), channel by channel, by a light sum: red, green and blue are the ambient
//! colour's channel plus, for each of at most [`MAX_PARTICLE_LIGHTS`] point lights, the
//! light's record colour × its attenuation at the distance between the particle and the
//! light; alpha is the ambient colour's alpha.
//!
//! By rule 13 the ambient colour is the frame's ambient term at the owner (`lighting.md`
//! §5.3) and the light list is the owner's light list (`lighting.md` §8.5), cut to five
//! (the global light counting as one); a point light's attenuation is 1 / (1 + *k* · *d²*)
//! at every distance, **without** the range cut-off of `lighting.md` §8.3 step 1 and
//! without its smooth fade (rule 13 as revised 2026-10-23, RT48); the global light and any
//! other non-point light count with attenuation 1.
//!
//! [`owner_light_list`] builds an owner's list from the frame's candidate lights: the
//! lights whose reach (the selection radius ρ of `lighting.md` §8.5 step 1) contains the
//! owner's position, key lights first, then the others, each group nearest first (the
//! Tessera Decision of `lighting.md` §8.5 for the order inside a group), cut to five.

use glam::Vec3;

/// Most point lights a lit system sums (`particles.md` §8.14).
pub const MAX_PARTICLE_LIGHTS: usize = 5;

/// One point light of a lit system's light list, in the simulation's world frame.
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct ParticleLight {
    /// The light's position.
    pub position: Vec3,
    /// The light record's colour (`lighting.md` §8.3: Diffuse × the clamped intensity).
    pub colour: [f32; 3],
    /// The record's range *R*, metres. Kept for information: rule 13 has no cut-off at
    /// the range (the range enters only through [`ParticleLight::attenuation`]).
    pub range: f32,
    /// The record's attenuation factor *k* (10 × Falloff / *R²*).
    pub attenuation: f32,
}

impl ParticleLight {
    /// The scene's global light (the main directional light, sun or moon) as the first
    /// entry of a light list (`particles.md` §8.14: "the scene's global light, when there
    /// is one, followed by the lights that affect the owner"; rule 13). A directional light
    /// has no position: by rule 13 (BIN, RT48) every non-point light, the global one
    /// included, counts with attenuation 1 everywhere (*k* = 0) and no direction term.
    #[must_use]
    pub fn global(colour: [f32; 3]) -> Self {
        Self {
            position: Vec3::ZERO,
            colour,
            range: f32::MAX,
            attenuation: 0.0,
        }
    }

    /// A light whose every value is finite (and range and attenuation not negative), or
    /// `None` (hostile input: such a light is left out).
    #[must_use]
    pub fn sanitized(self) -> Option<Self> {
        let ok = self.position.is_finite()
            && self.colour.iter().all(|c| c.is_finite())
            && self.range.is_finite()
            && self.range >= 0.0
            && self.attenuation.is_finite()
            && self.attenuation >= 0.0;
        ok.then_some(self)
    }

    /// The attenuation at `point` (rule 13): 1 / (1 + *k* · *d²*) at every distance, with
    /// no cut-off at the range and no smooth fade; 0 when the result is not finite (a
    /// non-finite point).
    #[must_use]
    pub fn attenuation_at(&self, point: Vec3) -> f32 {
        let d2 = (self.position - point).length_squared();
        if !d2.is_finite() {
            return 0.0;
        }
        let a = 1.0 / (1.0 + self.attenuation * d2);
        if a.is_finite() { a } else { 0.0 }
    }
}

/// A light list of at most [`MAX_PARTICLE_LIGHTS`] lights, in order.
#[derive(Debug, Clone, Copy, PartialEq, Default)]
pub struct LightList {
    lights: [Option<ParticleLight>; MAX_PARTICLE_LIGHTS],
}

impl LightList {
    /// No lights.
    pub const EMPTY: Self = Self {
        lights: [None; MAX_PARTICLE_LIGHTS],
    };

    /// The first [`MAX_PARTICLE_LIGHTS`] usable lights of `lights`, in order (the 5-light
    /// cut of rule 13); lights with a non-finite value are left out.
    #[must_use]
    pub fn new(lights: impl IntoIterator<Item = ParticleLight>) -> Self {
        let mut out = Self::EMPTY;
        for l in lights {
            if !out.push(l) && out.len() >= MAX_PARTICLE_LIGHTS {
                break;
            }
        }
        out
    }

    /// Appends `light` when there is room and it is usable; `true` when it was added.
    pub fn push(&mut self, light: ParticleLight) -> bool {
        let Some(light) = light.sanitized() else {
            return false;
        };
        match self.lights.iter_mut().find(|s| s.is_none()) {
            Some(slot) => {
                *slot = Some(light);
                true
            }
            None => false,
        }
    }

    /// Number of lights.
    #[must_use]
    pub fn len(&self) -> usize {
        self.lights.iter().filter(|l| l.is_some()).count()
    }

    /// Whether the list is empty.
    #[must_use]
    pub fn is_empty(&self) -> bool {
        self.lights.iter().all(Option::is_none)
    }

    /// The lights in order.
    pub fn iter(&self) -> impl Iterator<Item = &ParticleLight> {
        self.lights.iter().flatten()
    }

    /// The list with every light moved by `offset` (a change of frame).
    #[must_use]
    pub fn translated(&self, offset: Vec3) -> Self {
        let mut out = *self;
        for l in out.lights.iter_mut().flatten() {
            l.position += offset;
        }
        out
    }
}

/// The light sum of a lit particle at `point` (`particles.md` §8.14, rule 13): red, green
/// and blue are `ambient`'s channel plus Σ colour × attenuation over `lights`; alpha is
/// `ambient`'s alpha. Non-finite results count as 0.
#[must_use]
pub fn light_sum(ambient: [f32; 4], lights: &LightList, point: Vec3) -> [f32; 4] {
    let mut out = ambient;
    for l in lights.iter() {
        let a = l.attenuation_at(point);
        if a <= 0.0 {
            continue;
        }
        for (o, c) in out.iter_mut().zip(l.colour) {
            *o += c * a;
        }
    }
    out.map(|c| if c.is_finite() { c } else { 0.0 })
}

/// A frame light offered to [`owner_light_list`].
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct CandidateLight {
    /// The light (simulation frame).
    pub light: ParticleLight,
    /// Its selection radius ρ (`lighting.md` §8.5 step 1); the light reaches an owner
    /// whose squared distance to it is less than ρ².
    pub reach: f32,
    /// `KeyLight`: the light goes to the front of the list (§8.5 step 4).
    pub key: bool,
}

/// The light list of an owner at `owner` (rule 13, `lighting.md` §8.5): the candidates
/// whose reach contains the owner, key lights first, then the others, each group nearest
/// first, cut to [`MAX_PARTICLE_LIGHTS`]. Candidates with a non-finite value or a negative
/// reach are left out; equal distances keep the candidates' order.
#[must_use]
pub fn owner_light_list(owner: Vec3, candidates: &[CandidateLight]) -> LightList {
    if !owner.is_finite() {
        return LightList::EMPTY;
    }
    let mut reached: Vec<(bool, f32, usize)> = candidates
        .iter()
        .enumerate()
        .filter_map(|(i, c)| {
            c.light.sanitized()?;
            let d2 = (c.light.position - owner).length_squared();
            let ok = c.reach.is_finite() && c.reach >= 0.0 && d2 < c.reach * c.reach;
            ok.then_some((!c.key, d2, i))
        })
        .collect();
    reached.sort_by(|a, b| a.0.cmp(&b.0).then(a.1.total_cmp(&b.1)).then(a.2.cmp(&b.2)));
    LightList::new(
        reached
            .into_iter()
            .filter_map(|(_, _, i)| candidates.get(i).map(|c| c.light)),
    )
}

#[cfg(test)]
mod tests {
    use super::*;

    fn light(x: f32, colour: [f32; 3]) -> ParticleLight {
        ParticleLight {
            position: Vec3::new(x, 0.0, 0.0),
            colour,
            range: 10.0,
            // Falloff 0.5: k = 10 × 0.5 / 100.
            attenuation: 0.05,
        }
    }

    /// Rule 13 (revised 2026-10-23): 1 / (1 + k d²) at every distance, with no cut-off at
    /// the range and no smooth fade near it.
    #[test]
    fn attenuation_has_no_range_cut_off() {
        let l = light(0.0, [1.0; 3]);
        assert!((l.attenuation_at(Vec3::ZERO) - 1.0).abs() < 1e-6);
        // d = 4: 1 / (1 + 0.05 × 16) = 1 / 1.8.
        let a = l.attenuation_at(Vec3::new(4.0, 0.0, 0.0));
        assert!((a - 1.0 / 1.8).abs() < 1e-6, "{a}");
        // d = 9.9 (d² > 0.75 R²): no fade, 1 / (1 + 0.05 × 98.01).
        let a = l.attenuation_at(Vec3::new(0.0, 9.9, 0.0));
        assert!((a - 1.0 / (1.0 + 0.05 * 98.01)).abs() < 1e-5, "{a}");
        // At and beyond the range the light still adds: d = 10 gives 1 / 6, d = 20 1 / 21.
        let a = l.attenuation_at(Vec3::new(10.0, 0.0, 0.0));
        assert!((a - 1.0 / 6.0).abs() < 1e-6, "{a}");
        let a = l.attenuation_at(Vec3::new(20.0, 0.0, 0.0));
        assert!((a - 1.0 / 21.0).abs() < 1e-6, "{a}");
        // The range field takes no part: a zero range gives the same attenuation.
        let z = ParticleLight { range: 0.0, ..l };
        let p = Vec3::new(3.0, 0.0, 0.0);
        assert_eq!(z.attenuation_at(p), l.attenuation_at(p));
        assert_eq!(l.attenuation_at(Vec3::new(f32::NAN, 0.0, 0.0)), 0.0);
    }

    /// Review finding (hostile *k*): a light with an infinite or NaN *k* is left out of a
    /// list; the largest finite *k* gives a finite attenuation at the light and beyond it,
    /// as does an infinite *k* reaching `attenuation_at` directly (never NaN).
    #[test]
    fn hostile_attenuation_factors_give_finite_values() {
        let inf = ParticleLight {
            attenuation: f32::INFINITY,
            ..light(0.0, [1.0; 3])
        };
        let nan = ParticleLight {
            attenuation: f32::NAN,
            ..light(0.0, [1.0; 3])
        };
        assert!(inf.sanitized().is_none() && nan.sanitized().is_none());
        assert!(LightList::new([inf, nan]).is_empty());
        let kept = LightList::new([inf, light(1.0, [0.5; 3])]);
        assert_eq!(kept.len(), 1, "only the finite light");

        let huge = ParticleLight {
            attenuation: f32::MAX,
            ..light(0.0, [1.0; 3])
        };
        assert!(huge.sanitized().is_some());
        for p in [
            Vec3::ZERO,
            Vec3::new(1e-20, 0.0, 0.0),
            Vec3::X,
            Vec3::splat(1e19),
        ] {
            let a = huge.attenuation_at(p);
            assert!(a.is_finite() && (0.0..=1.0).contains(&a), "{p:?}: {a}");
        }
        assert_eq!(huge.attenuation_at(Vec3::ZERO), 1.0);
        for p in [Vec3::ZERO, Vec3::X] {
            let a = inf.attenuation_at(p);
            assert!(a.is_finite() && (0.0..=1.0).contains(&a), "{p:?}: {a}");
        }
        let s = light_sum([0.2; 4], &LightList::new([huge]), Vec3::X);
        assert!(s.iter().all(|c| c.is_finite()), "{s:?}");
    }

    /// `particles.md` §8.14: ambient plus Σ colour × attenuation for RGB, the ambient's
    /// alpha for alpha.
    #[test]
    fn the_sum_adds_lights_to_the_ambient() {
        let lights = LightList::new([light(0.0, [1.0, 0.5, 0.0]), light(4.0, [0.0, 0.0, 0.9])]);
        let s = light_sum([0.2, 0.2, 0.2, 0.7], &lights, Vec3::ZERO);
        let a2 = 1.0 / 1.8;
        assert!((s[0] - 1.2).abs() < 1e-6, "{s:?}");
        assert!((s[1] - 0.7).abs() < 1e-6, "{s:?}");
        assert!((s[2] - (0.2 + 0.9 * a2)).abs() < 1e-6, "{s:?}");
        assert_eq!(s[3], 0.7, "alpha is the ambient's");
        // Without lights the factor is the ambient.
        assert_eq!(
            light_sum([0.3, 0.4, 0.5, 1.0], &LightList::EMPTY, Vec3::ONE),
            [0.3, 0.4, 0.5, 1.0]
        );
    }

    /// The 5-light cut keeps the first five usable lights in order.
    #[test]
    fn the_list_keeps_five() {
        let all: Vec<ParticleLight> = (0..8).map(|i| light(i as f32, [1.0; 3])).collect();
        let l = LightList::new(all.iter().copied());
        assert_eq!(l.len(), MAX_PARTICLE_LIGHTS);
        let xs: Vec<f32> = l.iter().map(|l| l.position.x).collect();
        assert_eq!(xs, vec![0.0, 1.0, 2.0, 3.0, 4.0]);
        // Hostile lights are left out and do not take a slot.
        let bad = ParticleLight {
            colour: [f32::NAN, 0.0, 0.0],
            ..light(0.0, [1.0; 3])
        };
        let l = LightList::new([bad, light(1.0, [1.0; 3])]);
        assert_eq!(l.len(), 1);
        let neg = ParticleLight {
            range: -1.0,
            ..light(0.0, [1.0; 3])
        };
        assert!(LightList::new([neg]).is_empty());
        // The global light reaches everywhere at full strength and keeps its slot.
        let g = ParticleLight::global([0.5, 0.4, 0.3]);
        assert_eq!(g.attenuation_at(Vec3::new(1.0e6, -3.0e5, 2.0e4)), 1.0);
        let s = light_sum([0.1, 0.1, 0.1, 1.0], &LightList::new([g]), Vec3::ONE);
        assert!(
            (s[0] - 0.6).abs() < 1e-6 && (s[2] - 0.4).abs() < 1e-6,
            "{s:?}"
        );
        let moved = LightList::new([light(1.0, [1.0; 3])]).translated(Vec3::Y);
        assert_eq!(
            moved.iter().next().map(|l| l.position),
            Some(Vec3::new(1.0, 1.0, 0.0))
        );
    }

    /// `lighting.md` §8.5: reached lights only, key lights first, nearest first in each
    /// group, at most five.
    #[test]
    fn owner_lists_follow_the_selection_rules() {
        let c = |x: f32, reach: f32, key: bool| CandidateLight {
            light: light(x, [x, 0.0, 0.0]),
            reach,
            key,
        };
        let candidates = [
            c(3.0, 10.0, false),
            c(1.0, 10.0, false),
            c(6.0, 5.0, false), // beyond its reach (d = 6 ≥ ρ = 5)
            c(8.0, 20.0, true),
            c(2.0, 20.0, true),
            c(4.0, 10.0, false),
            c(5.0, 10.0, false),
            c(7.0, f32::NAN, true),
        ];
        let l = owner_light_list(Vec3::ZERO, &candidates);
        let xs: Vec<f32> = l.iter().map(|l| l.position.x).collect();
        assert_eq!(xs, vec![2.0, 8.0, 1.0, 3.0, 4.0]);
        assert!(owner_light_list(Vec3::NAN, &candidates).is_empty());
        assert!(owner_light_list(Vec3::ZERO, &[]).is_empty());
    }
}
