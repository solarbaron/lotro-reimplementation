// SPDX-License-Identifier: GPL-3.0-or-later
//! Small vector helpers. Public API uses `[f32; 3]` (the data's precision); narrow-phase
//! geometry runs in `f64` ([`D3`]) so that tolerances of 10⁻⁴ m stay meaningful far from
//! the landblock origin. Everything here is plain IEEE arithmetic without fused
//! multiply-add, so results are bit-identical on every platform.

/// A double-precision 3-vector.
pub(crate) type D3 = [f64; 3];

#[inline]
pub(crate) fn d3(v: [f32; 3]) -> D3 {
    [f64::from(v[0]), f64::from(v[1]), f64::from(v[2])]
}

#[inline]
#[allow(clippy::cast_possible_truncation)]
pub(crate) fn f3(v: D3) -> [f32; 3] {
    [v[0] as f32, v[1] as f32, v[2] as f32]
}

#[inline]
pub(crate) fn add(a: D3, b: D3) -> D3 {
    [a[0] + b[0], a[1] + b[1], a[2] + b[2]]
}

#[inline]
pub(crate) fn sub(a: D3, b: D3) -> D3 {
    [a[0] - b[0], a[1] - b[1], a[2] - b[2]]
}

#[inline]
pub(crate) fn scale(a: D3, s: f64) -> D3 {
    [a[0] * s, a[1] * s, a[2] * s]
}

#[inline]
pub(crate) fn dot(a: D3, b: D3) -> f64 {
    a[0] * b[0] + a[1] * b[1] + a[2] * b[2]
}

#[inline]
pub(crate) fn cross(a: D3, b: D3) -> D3 {
    [
        a[1] * b[2] - a[2] * b[1],
        a[2] * b[0] - a[0] * b[2],
        a[0] * b[1] - a[1] * b[0],
    ]
}

#[inline]
pub(crate) fn len_sq(a: D3) -> f64 {
    dot(a, a)
}

#[inline]
pub(crate) fn len(a: D3) -> f64 {
    dot(a, a).sqrt()
}

/// `a` scaled to unit length, or `None` for a zero, tiny or non-finite vector.
#[inline]
pub(crate) fn normalize(a: D3) -> Option<D3> {
    let l = len(a);
    (l.is_finite() && l > 1e-12).then(|| scale(a, 1.0 / l))
}

/// `a + (b − a)·t`.
#[inline]
pub(crate) fn lerp(a: D3, b: D3, t: f64) -> D3 {
    add(a, scale(sub(b, a), t))
}

/// Whether every component is finite.
#[inline]
pub(crate) fn finite3(v: [f32; 3]) -> bool {
    v.iter().all(|c| c.is_finite())
}

/// Largest coordinate magnitude (metres) any shape, query or cell may use. Real data stays
/// within a few thousand metres of a landblock origin (placements are bounded by 10⁶ in
/// `tessera-formats`); far larger values only come from hostile input and would make the
/// f64 narrow phase lose all precision, so they are refused.
pub const MAX_COORDINATE: f32 = 1.0e6;

/// Whether every component is finite and within ±[`MAX_COORDINATE`].
#[inline]
pub(crate) fn bounded3(v: [f32; 3]) -> bool {
    v.iter().all(|c| c.is_finite() && c.abs() <= MAX_COORDINATE)
}

/// Whether `r` is a usable radius or length: finite, 0 ≤ r ≤ [`MAX_COORDINATE`].
#[inline]
pub(crate) fn bounded_len(r: f32) -> bool {
    r.is_finite() && (0.0..=MAX_COORDINATE).contains(&r)
}

/// Component-wise minimum.
#[inline]
pub(crate) fn min3(a: [f32; 3], b: [f32; 3]) -> [f32; 3] {
    [a[0].min(b[0]), a[1].min(b[1]), a[2].min(b[2])]
}

/// Component-wise maximum.
#[inline]
pub(crate) fn max3(a: [f32; 3], b: [f32; 3]) -> [f32; 3] {
    [a[0].max(b[0]), a[1].max(b[1]), a[2].max(b[2])]
}

/// The f32 just below `v` (towards −∞); for conservative f32 bounds of f64 values.
#[inline]
#[allow(clippy::cast_possible_truncation)]
pub(crate) fn f32_down(v: f64) -> f32 {
    let f = v as f32;
    if f64::from(f) > v { f.next_down() } else { f }
}

/// The f32 just above `v` (towards +∞).
#[inline]
#[allow(clippy::cast_possible_truncation)]
pub(crate) fn f32_up(v: f64) -> f32 {
    let f = v as f32;
    if f64::from(f) < v { f.next_up() } else { f }
}
