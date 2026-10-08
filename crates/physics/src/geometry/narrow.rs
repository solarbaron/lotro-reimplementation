// SPDX-License-Identifier: GPL-3.0-or-later
//! Narrow phase: closest points between a capsule's core segment and a triangle, a point
//! or a segment, and the time of impact of a translating capsule (`collision.md` §12.1,
//! §12.3 step 4). All in f64.
//!
//! **Time of impact.** For a capsule (segment *S*, radius *r*) translating by *t* · Δ
//! against a convex item *B* (triangle, sphere core point, capsule core segment) of radius
//! *ρ*, the distance d(t) = dist(S + tΔ, B) is a convex function of *t* (it is the
//! distance from tΔ to the convex set B ⊖ S). Newton steps on a convex function from the
//! left never pass the root, and a non-negative slope proves that the gap never shrinks
//! again. The sweep uses two levels (Tessera Decision; `collision.md` §12.3 leaves the
//! solver to Tessera): an item **stops** the capsule only when its gap would fall to the
//! hit threshold of one skin, and the capsule then **rests** where the gap is still two
//! skins. A capsule resting on a surface therefore never counts the coplanar neighbour
//! across an edge as an obstacle (its gap there is the resting gap, above the threshold).
//! This is exact up to the skin, never tunnels, and needs only a distance routine per item
//! kind.
//!
//! **Solid boxes.** A setup's boxes collide through their 12 triangles (two-sided), and
//! also as solids ([`SolidBox`]): a capsule whose core is inside a box is pushed out
//! through the nearest face instead of being held by the inner faces.

use super::math::{self, D3};

/// Closest points of segments p1–q1 and p2–q2: (s, t, point on the first, point on the
/// second). Handles degenerate segments.
pub(crate) fn segment_segment(p1: D3, q1: D3, p2: D3, q2: D3) -> (f64, f64, D3, D3) {
    const EPS: f64 = 1e-18;
    let d1 = math::sub(q1, p1);
    let d2 = math::sub(q2, p2);
    let r = math::sub(p1, p2);
    let a = math::dot(d1, d1);
    let e = math::dot(d2, d2);
    let f = math::dot(d2, r);
    let (s, t);
    if a <= EPS && e <= EPS {
        return (0.0, 0.0, p1, p2);
    }
    if a <= EPS {
        s = 0.0;
        t = (f / e).clamp(0.0, 1.0);
    } else {
        let c = math::dot(d1, r);
        if e <= EPS {
            t = 0.0;
            s = (-c / a).clamp(0.0, 1.0);
        } else {
            let b = math::dot(d1, d2);
            let denom = a * e - b * b;
            let mut s0 = if denom > EPS * a * e {
                ((b * f - c * e) / denom).clamp(0.0, 1.0)
            } else {
                0.0
            };
            let mut t0 = (b * s0 + f) / e;
            if t0 < 0.0 {
                t0 = 0.0;
                s0 = (-c / a).clamp(0.0, 1.0);
            } else if t0 > 1.0 {
                t0 = 1.0;
                s0 = ((b - c) / a).clamp(0.0, 1.0);
            }
            s = s0;
            t = t0;
        }
    }
    (s, t, math::lerp(p1, q1, s), math::lerp(p2, q2, t))
}

/// Closest point of triangle (a, b, c) to `p` (Voronoi regions). For a degenerate
/// triangle (area ≈ 0) the closest point of its three edges.
pub(crate) fn point_triangle(p: D3, a: D3, b: D3, c: D3) -> D3 {
    let ab = math::sub(b, a);
    let ac = math::sub(c, a);
    let n = math::cross(ab, ac);
    let scale = math::len_sq(ab).max(math::len_sq(ac));
    if math::len_sq(n) <= 1e-24 * scale * scale || scale == 0.0 {
        return closest_on_edges(p, a, b, c);
    }
    let ap = math::sub(p, a);
    let d1 = math::dot(ab, ap);
    let d2 = math::dot(ac, ap);
    if d1 <= 0.0 && d2 <= 0.0 {
        return a;
    }
    let bp = math::sub(p, b);
    let d3 = math::dot(ab, bp);
    let d4 = math::dot(ac, bp);
    if d3 >= 0.0 && d4 <= d3 {
        return b;
    }
    let vc = d1 * d4 - d3 * d2;
    if vc <= 0.0 && d1 >= 0.0 && d3 <= 0.0 {
        let v = d1 / (d1 - d3);
        return math::add(a, math::scale(ab, v));
    }
    let cp = math::sub(p, c);
    let d5 = math::dot(ab, cp);
    let d6 = math::dot(ac, cp);
    if d6 >= 0.0 && d5 <= d6 {
        return c;
    }
    let vb = d5 * d2 - d1 * d6;
    if vb <= 0.0 && d2 >= 0.0 && d6 <= 0.0 {
        let w = d2 / (d2 - d6);
        return math::add(a, math::scale(ac, w));
    }
    let va = d3 * d6 - d5 * d4;
    if va <= 0.0 && (d4 - d3) >= 0.0 && (d5 - d6) >= 0.0 {
        let w = (d4 - d3) / ((d4 - d3) + (d5 - d6));
        return math::add(b, math::scale(math::sub(c, b), w));
    }
    let denom = 1.0 / (va + vb + vc);
    let v = vb * denom;
    let w = vc * denom;
    math::add(a, math::add(math::scale(ab, v), math::scale(ac, w)))
}

fn closest_on_edges(p: D3, a: D3, b: D3, c: D3) -> D3 {
    let mut best = a;
    let mut best_d = f64::INFINITY;
    for (u, v) in [(a, b), (b, c), (c, a)] {
        let (_, _, _, q) = segment_segment(p, p, u, v);
        let d = math::len_sq(math::sub(p, q));
        if d < best_d {
            best_d = d;
            best = q;
        }
    }
    best
}

/// Closest points between segment p–q and triangle (a, b, c): (point on the segment, point
/// on the triangle). Distance 0 when the segment pierces the triangle.
pub(crate) fn segment_triangle(p: D3, q: D3, a: D3, b: D3, c: D3) -> (D3, D3) {
    // Piercing test (two-sided).
    let ab = math::sub(b, a);
    let ac = math::sub(c, a);
    let n = math::cross(ab, ac);
    let nn = math::len_sq(n);
    if nn > 1e-24 {
        let dp = math::dot(n, math::sub(p, a));
        let dq = math::dot(n, math::sub(q, a));
        if (dp <= 0.0 && dq >= 0.0) || (dp >= 0.0 && dq <= 0.0) {
            let denom = dp - dq;
            let s = if denom.abs() > 0.0 { dp / denom } else { 0.0 };
            let x = math::lerp(p, q, s.clamp(0.0, 1.0));
            // Barycentric inside test.
            let ax = math::sub(x, a);
            let c1 = math::dot(math::cross(ab, ax), n);
            let c2 = math::dot(math::cross(math::sub(c, b), math::sub(x, b)), n);
            let c3 = math::dot(math::cross(math::sub(a, c), math::sub(x, c)), n);
            if c1 >= 0.0 && c2 >= 0.0 && c3 >= 0.0 && dp != dq {
                return (x, x);
            }
        }
    }
    let mut best = (p, point_triangle(p, a, b, c));
    let mut best_d = math::len_sq(math::sub(best.0, best.1));
    let mut consider = |s: D3, t: D3| {
        let d = math::len_sq(math::sub(s, t));
        if d < best_d {
            best_d = d;
            best = (s, t);
        }
    };
    consider(q, point_triangle(q, a, b, c));
    for (u, v) in [(a, b), (b, c), (c, a)] {
        let (_, _, s, t) = segment_segment(p, q, u, v);
        consider(s, t);
    }
    best
}

/// A convex obstacle in f64 for the narrow phase.
#[derive(Debug, Clone, Copy)]
pub(crate) enum Convex {
    /// A triangle (two-sided), radius 0.
    Triangle([D3; 3]),
    /// A segment with a radius (a sphere when both ends are equal).
    Segment(D3, D3, f64),
}

impl Convex {
    /// Radius around the core.
    pub(crate) fn radius(&self) -> f64 {
        match self {
            Self::Triangle(_) => 0.0,
            Self::Segment(_, _, r) => *r,
        }
    }

    /// Closest points between segment p–q and the core: (on the segment, on the core).
    pub(crate) fn closest(&self, p: D3, q: D3) -> (D3, D3) {
        match self {
            Self::Triangle([a, b, c]) => segment_triangle(p, q, *a, *b, *c),
            Self::Segment(a, b, _) => {
                let (_, _, s, t) = segment_segment(p, q, *a, *b);
                (s, t)
            }
        }
    }

    /// The triangle's unit face normal, if this is a non-degenerate triangle.
    pub(crate) fn face_normal(&self) -> Option<D3> {
        match self {
            Self::Triangle([a, b, c]) => {
                math::normalize(math::cross(math::sub(*b, *a), math::sub(*c, *a)))
            }
            Self::Segment(..) => None,
        }
    }
}

/// Below this gap (metres) a capsule counts as touching an item: a sweep is then blocked at
/// once when it moves into the item and free otherwise (the f64 distance routines are not
/// meaningful much below it). Not stated by `collision.md` §12.4 (w), which speaks of
/// overlaps only: implementer OPEN there.
pub(crate) const TOUCH_GAP: f64 = 1e-6;

/// A sweep reports an item only when the gap to it would fall to this many skins
/// (`collision.md` §12.4 (w)), or to half the capsule's clearance when that is smaller (not
/// in (w): implementer OPEN in §12.4).
pub(crate) const HIT_SKINS: f64 = 1.0;

/// A sweep that reports an item leaves the capsule this many skins from it ...
pub(crate) const REST_SKINS: f64 = 2.0;

/// ... up to this many skins more.
pub(crate) const REST_BAND_SKINS: f64 = 0.5;

/// The separation of a capsule core p–q (radius `r`) from `item`: (distance between the
/// surfaces, unit normal from the item towards the capsule, contact point on the item's
/// surface).
///
/// When the core touches or crosses a triangle (core distance ≈ 0) the normal is the face
/// normal turned to the side of the core's centre (to `hint`, or +Z, when the centre lies
/// in the plane), and the separation is −(r + the depth of the core's deeper end behind
/// the plane along that normal): moving the capsule by −separation along the normal clears
/// the triangle's plane. Round items whose core touches the capsule's core use `hint`.
pub(crate) fn separation(p: D3, q: D3, r: f64, item: &Convex, hint: D3) -> (f64, D3, D3) {
    let (s, t) = item.closest(p, q);
    let v = math::sub(s, t);
    let d = math::len(v);
    let rho = item.radius();
    if d > 1e-9 {
        let normal = math::scale(v, 1.0 / d);
        let point = math::add(t, math::scale(normal, rho));
        return (d - r - rho, normal, point);
    }
    let fallback = math::normalize(hint).unwrap_or([0.0, 0.0, 1.0]);
    let (Convex::Triangle([a, _, _]), Some(n)) = (item, item.face_normal()) else {
        let point = math::add(t, math::scale(fallback, rho));
        return (-r - rho, fallback, point);
    };
    let centre = math::lerp(p, q, 0.5);
    let side = math::dot(n, math::sub(centre, *a));
    let n = if side < -1e-12 || (side <= 1e-12 && math::dot(n, fallback) < 0.0) {
        math::scale(n, -1.0)
    } else {
        n
    };
    let lowest = math::dot(n, math::sub(p, *a)).min(math::dot(n, math::sub(q, *a)));
    let depth = (-lowest).max(0.0);
    (-(r + depth), n, t)
}

/// Result of one item's time-of-impact search.
#[derive(Debug, Clone, Copy)]
pub(crate) struct Impact {
    /// Fraction of the motion (0 – 1) the capsule may travel: it then stands at least
    /// [`REST_SKINS`] skins (or its starting gap, when that was smaller) from the item.
    pub(crate) t: f64,
    /// Gap between the surfaces at `t` (negative: the capsule started inside).
    pub(crate) gap: f64,
    /// Unit normal from the item to the capsule where the item is reached.
    pub(crate) normal: D3,
    /// Contact point on the item where it is reached.
    pub(crate) point: D3,
}

/// Most Newton steps per item and phase.
pub(crate) const MAX_TOI_STEPS: usize = 48;

/// One Newton step from the left on d(t) − `aim`: (new t, gap, normal, point), or `None`
/// when the gap never reaches `aim` within the motion (a non-negative slope, or a tangent
/// that reaches `aim` only after t = 1; d is convex, so it stays above its tangents).
#[allow(clippy::too_many_arguments)]
fn newton_step(
    p: D3,
    q: D3,
    r: f64,
    delta: D3,
    item: &Convex,
    hint: D3,
    (t, gap, normal): (f64, f64, D3),
    aim: f64,
) -> Option<(f64, f64, D3, D3)> {
    // d'(t) = n · Δ; the gap shrinks at rate −n · Δ.
    let approach = -math::dot(normal, delta);
    if approach <= 1e-12 {
        return None;
    }
    let next = t + (gap - aim) / approach;
    if !next.is_finite() || next > 1.0 {
        return None;
    }
    let off = math::scale(delta, next);
    let (g, n, pt) = separation(math::add(p, off), math::add(q, off), r, item, hint);
    g.is_finite().then_some((next, g, n, pt))
}

/// Whether the capsule core p–q (radius `r`) moving by `delta` is stopped by `item`, and
/// where it may stop (module docs). `clearance` is the capsule's smallest starting gap to
/// anything around it (at most this item's own starting gap).
///
/// - A capsule that starts within [`TOUCH_GAP`] of the item (or overlapping it) is
///   stopped at t = 0 when it moves into the item (−n · Δ > 0) and not at all otherwise:
///   overlaps are resolved by the caller from contacts.
/// - Otherwise the item stops the capsule only when the gap would fall to the **hit
///   threshold** h = min([`HIT_SKINS`] skins, half the clearance) within the motion. The
///   capsule then stops at the last point where the gap is still at least
///   min([`REST_SKINS`] skins, the item's starting gap) (between that and
///   [`REST_BAND_SKINS`] skins more when it moves at all).
///
/// A stopped capsule rests about two skins from what stopped it, while one skin (or half
/// the clearance, when the capsule is closer than that to something) counts as a hit. So a
/// capsule on or against a surface slides along it across the edges of coplanar
/// neighbours: their gap never drops below the gap to the surface itself, which is at
/// least the clearance. Only a capsule within [`TOUCH_GAP`] of a surface can be stopped by
/// a coplanar neighbour.
pub(crate) fn time_of_impact(
    p: D3,
    q: D3,
    r: f64,
    delta: D3,
    item: &Convex,
    skin: f64,
    clearance: f64,
) -> Option<Impact> {
    let hint = math::scale(delta, -1.0);
    let (g0, n0, pt0) = separation(p, q, r, item, hint);
    if !g0.is_finite() {
        return None;
    }
    if g0 < TOUCH_GAP {
        return (-math::dot(n0, delta) > 1e-12).then_some(Impact {
            t: 0.0,
            gap: g0,
            normal: n0,
            point: pt0,
        });
    }
    let hit = (HIT_SKINS * skin).min(0.5 * clearance.clamp(TOUCH_GAP, g0));
    let aim = 0.5 * hit;
    let rest = (REST_SKINS * skin).min(g0);
    let rest_hi = rest + REST_BAND_SKINS * skin;
    let mut state = (0.0f64, g0, n0);
    let mut point = pt0;
    // Phase 1: where the gap comes down to `rest` (Newton from the left never passes it).
    if g0 > rest_hi {
        for _ in 0..MAX_TOI_STEPS {
            let (t, g, n, pt) = newton_step(p, q, r, delta, item, hint, state, rest)?;
            state = (t, g, n);
            point = pt;
            if g <= rest_hi {
                break;
            }
        }
    }
    let (t_rest, gap_rest) = (state.0, state.1);
    // Phase 2: whether the hit threshold is reached before the end of the motion.
    for _ in 0..MAX_TOI_STEPS {
        let (t, g, n, pt) = newton_step(p, q, r, delta, item, hint, state, aim)?;
        state = (t, g, n);
        point = pt;
        if g <= hit {
            break;
        }
    }
    // Reached (or, after slow convergence on a grazing contact, possibly reached: stop
    // conservatively).
    Some(Impact {
        t: t_rest,
        gap: gap_rest,
        normal: state.2,
        point,
    })
}

/// Below this depth a capsule core counts as outside a solid box (its triangles then
/// handle it).
pub(crate) const SOLID_INSIDE_EPS: f64 = 1e-6;

/// A solid box (any parallelepiped) as six outward planes n · x ≤ o, in the face order of
/// the box triangles (−z, +z, −y, +y, −x, +x).
#[derive(Debug, Clone, Copy, PartialEq)]
pub(crate) struct SolidBox {
    planes: [(D3, f64); 6],
}

/// For each face of a box (in [`SolidBox`] order), three of its corners (corner *k*: bit 0
/// x max, bit 1 y max, bit 2 z max).
const FACE_CORNERS: [[usize; 3]; 6] = [
    [0, 2, 3],
    [4, 5, 7],
    [0, 1, 5],
    [2, 6, 7],
    [0, 4, 6],
    [1, 3, 7],
];

impl SolidBox {
    /// The solid of a placed box's eight corners, or `None` when it has no interior (a flat
    /// or degenerate box: such a box is only its triangles).
    pub(crate) fn from_corners(corners: &[[f32; 3]; 8]) -> Option<Self> {
        let c = corners.map(math::d3);
        let centre = math::lerp(c[0], c[7], 0.5);
        let mut planes = [([0.0; 3], 0.0); 6];
        for (f, [i, j, k]) in FACE_CORNERS.iter().enumerate() {
            let n = math::normalize(math::cross(
                math::sub(c[*j], c[*i]),
                math::sub(c[*k], c[*i]),
            ))?;
            let o = math::dot(n, c[*i]);
            let n = if math::dot(n, centre) > o {
                math::scale(n, -1.0)
            } else {
                n
            };
            let o = math::dot(n, c[*i]);
            // The centre must lie properly inside every face.
            if math::dot(n, centre) > o - 1e-6 {
                return None;
            }
            planes[f] = (n, o);
        }
        Some(Self { planes })
    }

    /// Whether some point of segment p–q lies deeper than [`SOLID_INSIDE_EPS`] inside.
    pub(crate) fn core_inside(&self, p: D3, q: D3) -> bool {
        let (mut lo, mut hi) = (0.0f64, 1.0f64);
        let d = math::sub(q, p);
        for (n, o) in &self.planes {
            let a = math::dot(*n, p) - (o - SOLID_INSIDE_EPS);
            let b = math::dot(*n, d);
            if b.abs() <= 1e-300 {
                if a > 0.0 {
                    return false;
                }
                continue;
            }
            let s = -a / b;
            if b > 0.0 {
                hi = hi.min(s);
            } else {
                lo = lo.max(s);
            }
            if lo > hi {
                return false;
            }
        }
        true
    }

    /// For a core inside the box: (separation, outward normal, point on the face, face)
    /// of the face that the capsule (radius `r`) leaves by the shortest push along a face
    /// normal (the first such face on ties). The separation is −(r + push).
    pub(crate) fn penetration(&self, p: D3, q: D3, r: f64) -> (f64, D3, D3, usize) {
        let mut best = (f64::INFINITY, 0usize, p);
        for (f, (n, o)) in self.planes.iter().enumerate() {
            let (dp, dq) = (math::dot(*n, p), math::dot(*n, q));
            let (low, x) = if dq < dp { (dq, q) } else { (dp, p) };
            let push = o - low;
            if push < best.0 {
                best = (push, f, x);
            }
        }
        let (push, f, x) = best;
        let n = self.planes[f].0;
        (-(r + push), n, math::add(x, math::scale(n, push)), f)
    }
}
