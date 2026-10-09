// SPDX-License-Identifier: GPL-3.0-or-later
//! CPU twins of the far-view shading terms (`far-view.md` §6.3–§6.5, §9.3), used by the
//! tests and by the renderer's reference checks. All inputs are in world units.

use super::settings::FarFade;

fn sat(v: f32) -> f32 {
    if v.is_nan() { 0.0 } else { v.clamp(0.0, 1.0) }
}

/// Fog factor *f* = sat((*d* − near) / range) (§6.3 step 2; `lighting.md` §10).
#[must_use]
pub fn fog_factor(d: f32, near: f32, range: f32) -> f32 {
    if !(range.is_finite() && range > 0.0) {
        return 0.0;
    }
    sat((d - near) / range)
}

/// The plain template's output colour (§6.3 steps 5–6): *c* = (colour + detail − 0.5) ×
/// *Σ*, then (1 − *f*) × 2*c* + *f* × fog colour.
#[must_use]
pub fn plain_colour(
    colour: [f32; 3],
    detail: [f32; 3],
    light: [f32; 3],
    fog: f32,
    fog_colour: [f32; 3],
) -> [f32; 3] {
    std::array::from_fn(|k| {
        let c = (colour[k] + detail[k] - 0.5) * light[k];
        (1.0 - fog) * 2.0 * c + fog * fog_colour[k]
    })
}

/// The normal strength *t* = sat((*h* − 80) / 160) (§6.4): 0 within 80 units
/// horizontally, 1 beyond 240.
#[must_use]
pub fn normal_strength(h: f32) -> f32 {
    sat((h - 80.0) / 160.0)
}

/// The near-view mask *m* (§6.4) at (*p*, *q*) = horizontal position minus the near-view
/// origin: sat(max(−*p* − 159, −*q* − 159, *p* − 319, *q* − 319)).
#[must_use]
pub fn near_view_mask(p: f32, q: f32) -> f32 {
    sat((-p - 159.0).max(-q - 159.0).max(p - 319.0).max(q - 319.0))
}

/// The coverage *w* = min(1, *m* + sat((*h* − 80) / 80) × min(1, *n*_up²)) (§6.4).
#[must_use]
pub fn coverage(m: f32, h: f32, n_up: f32) -> f32 {
    (m + sat((h - 80.0) / 80.0) * (n_up * n_up).min(1.0)).min(1.0)
}

/// The seam overlay's coverage on near-field geometry (*m* = 0, §6.5).
#[must_use]
pub fn seam_overlay_coverage(h: f32, n_up: f32) -> f32 {
    coverage(0.0, h, n_up)
}

/// A sprite's opacity factor (§9.3): sat((*d* − (*D* × *c* − 25)) / 10) times the far-draw
/// fade.
#[must_use]
pub fn sprite_opacity(d: f32, distance: f32, c: f32, fade: &FarFade) -> f32 {
    sat((d - (distance * c - 25.0)) / 10.0) * fade.factor(d)
}
