// SPDX-License-Identifier: GPL-3.0-or-later
//! Light records and attenuation (`lighting.md` §8.3).

use super::entities::LightParams;

/// Smallest range a light record keeps, metres (`lighting.md` §8.3: max(Range, 0.0002)).
pub const MIN_RANGE: f32 = 0.0002;
/// Tolerance per channel of the "specular equals white" test (`lighting.md` §8.3).
pub const WHITE_TOLERANCE: f32 = 0.0002;
/// Factor of the attenuation constant: *k* = 10 × Falloff / *R*² (`lighting.md` §8.3).
pub const FALLOFF_SCALE: f32 = 10.0;
/// Fraction of *R*² from which the newer back end fades a light to zero at its range
/// (`lighting.md` §8.3 step 3).
pub const FADE_START: f32 = 0.75;

/// Whether a light is a point light or the directional main light (`lighting.md` §8.3).
#[derive(Debug, Clone, Copy, PartialEq)]
pub enum LightKind {
    /// A point light at a position.
    Point,
    /// A directional light travelling along a direction.
    Directional([f32; 3]),
}

/// The per-frame light record (`lighting.md` §8.3, L19).
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct LightRecord {
    /// Point or directional.
    pub kind: LightKind,
    /// Colour (RGB): Diffuse × clamp(intensity × *σ*, 0, 1).
    pub colour: [f32; 3],
    /// Specular colour (RGB).
    pub specular: [f32; 3],
    /// Range *R* = max(Range, 0.0002).
    pub range: f32,
    /// *R*², as uploaded.
    pub range_sq: f32,
    /// Attenuation constant *k* = 10 × Falloff / *R*².
    pub k: f32,
    /// Position (landblock or world frame, as given).
    pub position: [f32; 3],
    /// `KeyLight`: a per-pixel candidate (§8.5).
    pub key_light: bool,
}

fn clamp01(v: f32) -> f32 {
    if v.is_nan() { 0.0 } else { v.clamp(0.0, 1.0) }
}

impl LightRecord {
    /// Builds the record of a point light at `position` from its fields, the value of its
    /// intensity curve at the current global clock (`intensity`) and the run-time scale
    /// *σ* (1 by the Tessera Decision of §8.3), with `range` the range in force (see
    /// [`LightParams::at_time`]).
    #[must_use]
    pub fn point(
        params: &LightParams,
        position: [f32; 3],
        range: f32,
        intensity: f32,
        sigma: f32,
    ) -> Self {
        let scale = clamp01(intensity * sigma);
        let colour = params.diffuse.map(|c| c * scale);
        let spec_scale = clamp01(2.0 * sigma * intensity);
        let white = params
            .specular
            .iter()
            .all(|c| (c - 1.0).abs() <= WHITE_TOLERANCE);
        let spec_base = if !params.key_light && white {
            params.diffuse
        } else {
            params.specular
        };
        let range = if range.is_finite() {
            range.max(MIN_RANGE)
        } else {
            MIN_RANGE
        };
        let range_sq = range * range;
        let falloff = if params.falloff.is_finite() {
            params.falloff
        } else {
            0.0
        };
        Self {
            kind: LightKind::Point,
            colour,
            specular: spec_base.map(|c| c * spec_scale),
            range,
            range_sq,
            k: FALLOFF_SCALE * falloff / range_sq,
            position,
            key_light: params.key_light,
        }
    }

    /// The attenuation of this light at squared distance `d2` (`lighting.md` §8.3);
    /// `fade` selects the newer back end's fade near the range (light 0 and per-pixel
    /// lists), the per-vertex lists use the hard cut-off only.
    #[must_use]
    pub fn attenuation(&self, d2: f32, fade: bool) -> f32 {
        attenuation(d2, self.range_sq, self.k, fade)
    }
}

/// The attenuation of a point light (`lighting.md` §8.3 steps 1–3): 0 when *d²* ≥ *R²*;
/// else *a* = 1 / (1 + *k* · *d²*); with `fade`, when *d²* > 0.75 *R²*, *a* is multiplied
/// by the smoothstep *s*²(3 − 2*s*), *s* = (*R²* − *d²*) / (0.25 *R²*). Non-finite inputs
/// give 0; the result is clamped to [0, 1] (only hostile fields, such as a negative falloff,
/// reach the clamp).
#[must_use]
pub fn attenuation(d2: f32, r2: f32, k: f32, fade: bool) -> f32 {
    if !(d2.is_finite() && r2.is_finite() && k.is_finite()) || d2 >= r2 {
        return 0.0;
    }
    let mut a = 1.0 / (1.0 + k * d2);
    if fade && d2 > FADE_START * r2 {
        let s = (r2 - d2) / ((1.0 - FADE_START) * r2);
        a *= s * s * (3.0 - 2.0 * s);
    }
    // Valid fields keep a in (0, 1]; hostile ones (a negative falloff) are clamped.
    if a.is_finite() {
        a.clamp(0.0, 1.0)
    } else {
        0.0
    }
}
