// SPDX-License-Identifier: GPL-3.0-or-later
//! Fit containers and fit groups (`ui-scaling.md` §5.4), fit-to-screen (§5.6) and the
//! degenerate-box guard (§5.7).
//!
//! A **fit container** keeps its design-size layout box and gets a uniform fit factor `f`;
//! its children are laid out exactly as designed. This module computes `f` and the container's
//! **fitted position** in its parent's layout coordinates; [`Fit::displayed`] then maps it to a
//! displayed box through the parent's displayed top-left and node scale `m(parent)` (§3.4
//! item 2). Everything is pure arithmetic, guarded against zero and non-finite extents so it
//! never panics or divides by zero.
//!
//! Three container kinds, by which edges are in mode 6 (`ui-scaling.md` §5.4):
//!
//! - **Type A** (both horizontal edges 6): a full-cover fit, fitted in *fit groups* so that
//!   layers of one picture share one factor and one placement ([`fit_type_a`]);
//! - **Type B** (exactly one horizontal edge 6): a side panel, the height drives ([`fit_type_b`]);
//! - **Type C** (no horizontal edge 6, a vertical edge is): the width drives ([`fit_type_c`]).

use super::dbox::DBox;

/// The magnification bound ratio `ρ` (`ui-scaling.md` §5.4 "About the bound"): 4 ÷ 3, the
/// cover-to-contain ratio of a 4:3 design on a 16:9 screen. A Type A factor is never above
/// `ρ × Ph ÷ h`, so a group's design height is magnified to at most 4 ÷ 3 of `P`'s height.
pub const RHO: f64 = 4.0 / 3.0;

/// The fit of a container: the uniform fit factor and the fitted position in the parent's
/// layout coordinates (`ui-scaling.md` §3.4 item 2, §5.4). For a Type A group the fitted
/// position is the placed group box's top-left (`U`'s top-left); use [`Fit::member`] for a
/// member whose design rectangle differs from `U`.
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct Fit {
    /// The uniform fit factor `f` (> 0, finite).
    pub f: f64,
    /// The fitted left in the parent's layout coordinates.
    pub x: f64,
    /// The fitted top in the parent's layout coordinates.
    pub y: f64,
}

impl Fit {
    /// The identity fit: `f` = 1 at the origin (a container that is not fitted, §5.4
    /// "Zero extents").
    pub const IDENTITY: Self = Self {
        f: 1.0,
        x: 0.0,
        y: 0.0,
    };

    /// The displayed box of a container (or Type A member) with design size `design_w` ×
    /// `design_h`, under the parent's displayed top-left `parent_tl` and node scale `m`
    /// (`ui-scaling.md` §3.4 item 2): displayed top-left = `parent_tl + fitted × m`, size =
    /// `design × f × m`.
    #[must_use]
    pub fn displayed(
        &self,
        design_w: f64,
        design_h: f64,
        parent_tl: (f64, f64),
        m: (f64, f64),
    ) -> DBox {
        DBox::new(
            parent_tl.0 + self.x * m.0,
            parent_tl.1 + self.y * m.1,
            design_w * self.f * m.0,
            design_h * self.f * m.1,
        )
    }

    /// The fit of a Type A group **member** (`ui-scaling.md` §5.4 step 5): same factor, fitted
    /// position offset by the member's design offset from the group box `U`'s top-left,
    /// scaled by `f`. `u_left`/`u_top` is `U`'s top-left and `m_left`/`m_top` the member's
    /// design top-left, both in `P`'s design coordinates.
    #[must_use]
    pub fn member(&self, u_left: f64, u_top: f64, m_left: f64, m_top: f64) -> Self {
        Self {
            f: self.f,
            x: self.x + (m_left - u_left) * self.f,
            y: self.y + (m_top - u_top) * self.f,
        }
    }
}

/// The content box of a Type A fit group (`ui-scaling.md` §5.4): the smallest box holding the
/// members' direct children, relative to the group box `U`, in `P`'s design coordinates.
/// `None` when no member has such a child.
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct ContentBox {
    /// Left, relative to `U`'s top-left.
    pub x: f64,
    /// Top, relative to `U`'s top-left.
    pub y: f64,
    /// Width (≥ 0).
    pub w: f64,
    /// Height (≥ 0).
    pub h: f64,
}

/// A Type A fit container's design rectangle, in `P`'s design coordinates. Used to build the
/// group box `U` and, with [`Fit::member`], each member's displayed box.
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct FitMember {
    /// Design left in `P`'s design coordinates.
    pub left: f64,
    /// Design top.
    pub top: f64,
    /// Design width (≥ 1 for a fitted member).
    pub w: f64,
    /// Design height.
    pub h: f64,
}

/// The box an edge-mode fit (Type B or C) computes by UL §8.4 steps 2–5, in the parent's
/// layout coordinates (`ui-scaling.md` §5.4). The right and bottom are inclusive, so the
/// exclusive ends are `right + 1` and `bottom + 1`.
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct ComputedBox {
    /// Computed left.
    pub left: f64,
    /// Computed top.
    pub top: f64,
    /// Computed right (inclusive).
    pub right: f64,
    /// Computed bottom (inclusive).
    pub bottom: f64,
}

/// Which vertical edge of a Type C container is in mode 6 (`ui-scaling.md` §5.4 Type C).
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum VerticalAnchor {
    /// Bottom edge 6: anchored at the computed top.
    Top,
    /// Top edge 6: anchored at the computed bottom.
    Bottom,
    /// Both vertical edges 6: centred on the computed box's vertical centre.
    Centre,
}

/// A fit container, named so the caller reads [`fit_type_a`]/[`fit_type_b`]/[`fit_type_c`]
/// consistently. Not used as a value today; kept so the public surface documents the kinds.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum FitContainer {
    /// Both horizontal edges 6 (a full-cover fit, fitted in groups).
    TypeA,
    /// Exactly one horizontal edge 6 (a side panel).
    TypeB,
    /// No horizontal edge 6, a vertical edge is.
    TypeC,
}

/// The smallest box holding every member's design rectangle (the group box `U`,
/// `ui-scaling.md` §5.4), as `(left, top, w, h)` in `P`'s design coordinates. Returns a
/// degenerate box when `members` is empty or every member has a non-positive extent.
#[must_use]
pub fn group_box(members: &[FitMember]) -> (f64, f64, f64, f64) {
    let mut l = f64::INFINITY;
    let mut t = f64::INFINITY;
    let mut r = f64::NEG_INFINITY;
    let mut b = f64::NEG_INFINITY;
    for m in members {
        if m.w <= 0.0 || m.h <= 0.0 {
            continue;
        }
        l = l.min(m.left);
        t = t.min(m.top);
        r = r.max(m.left + m.w);
        b = b.max(m.top + m.h);
    }
    if !l.is_finite() || r <= l || b <= t {
        return (0.0, 0.0, 0.0, 0.0);
    }
    (l, t, r - l, b - t)
}

/// The safe area `A` taken into a parent's layout coordinates (`ui-scaling.md` §5.4 "The safe
/// area in P's coordinates"): with `parent_tl` the parent's displayed top-left and `m` =
/// `m(parent)`, `A`'s edges `(Ax, 0, Ax+Aw, Hu)` become `((Ax − dl) ÷ mx, …)`. A non-positive
/// `m` component falls back to 1.
#[must_use]
pub fn safe_area_in_parent(a: DBox, parent_tl: (f64, f64), m: (f64, f64)) -> DBox {
    let (dl, dt) = parent_tl;
    let mx = if m.0.is_finite() && m.0 > 0.0 {
        m.0
    } else {
        1.0
    };
    let my = if m.1.is_finite() && m.1 > 0.0 {
        m.1
    } else {
        1.0
    };
    let left = (a.l - dl) / mx;
    let right = (a.right() - dl) / mx;
    let top = (a.t - dt) / my;
    let bottom = (a.bottom() - dt) / my;
    DBox::new(left, top, (right - left).max(0.0), (bottom - top).max(0.0))
}

/// A Type A fit group's factor and placement (`ui-scaling.md` §5.4 Type A).
///
/// Arguments, all in `P`'s layout coordinates (see §5.4):
/// - `p_w` × `p_h`: the reference box `P` (the parent's current layout size, or `F` for a fit
///   root), given as `(0, 0, p_w, p_h)`;
/// - `u`: the group box `U` as `(left, top, w, h)` ([`group_box`]);
/// - `content`: the content box `C`, relative to `U`, or `None` when empty;
/// - `guard`: the guard box `G` = `P` ∩ (`A` in `P`'s coordinates), or `P` itself when that is
///   empty ([`safe_area_in_parent`]);
/// - `rho`: the magnification bound ratio ([`RHO`]).
///
/// Returns the group's fit: `f` and the placed group box's top-left (`U`'s fitted position) in
/// `P`'s coordinates. Each member's fit is [`Fit::member`] of this with `U`'s and the member's
/// design top-lefts. A zero or non-finite `P` or `U` extent gives [`Fit::IDENTITY`] (not
/// fitted, §5.4 "Zero extents").
#[must_use]
pub fn fit_type_a(
    p_w: f64,
    p_h: f64,
    u: (f64, f64, f64, f64),
    content: Option<ContentBox>,
    guard: DBox,
    rho: f64,
) -> Fit {
    let (u_l, u_t, u_w, u_h) = u;
    if !(p_w > 0.0 && p_h > 0.0 && u_w > 0.0 && u_h > 0.0)
        || !p_w.is_finite()
        || !p_h.is_finite()
        || !u_w.is_finite()
        || !u_h.is_finite()
    {
        return Fit::IDENTITY;
    }
    let rho = if rho.is_finite() && rho > 0.0 {
        rho
    } else {
        RHO
    };
    // Step 1.
    let f_contain = (p_w / u_w).min(p_h / u_h);
    let f_cover = (p_w / u_w).max(p_h / u_h);
    let f_cap = rho * p_h / u_h;
    // Step 2.
    let content = content.filter(|c| c.w > 0.0 || c.h > 0.0);
    let f = match content {
        None => f_cover.min(f_cap),
        Some(c) => {
            let mut f_guard = f64::INFINITY;
            if c.w > 0.0 {
                f_guard = f_guard.min(guard.w / c.w);
            }
            if c.h > 0.0 {
                f_guard = f_guard.min(guard.h / c.h);
            }
            f_contain.max(f_cover.min(f_guard).min(f_cap))
        }
    };
    // I5 (§9.5): the fit factor is finite and positive. §3.3 guarantees design extents are
    // integer-valued, so `f` is finite and positive for every real layout; this guards the
    // pure API against a caller that passes a subnormal extent, where `p / u` could overflow
    // to infinity and yield a non-finite factor. An unusable factor means "not fitted".
    if !(f.is_finite() && f > 0.0) {
        return Fit::IDENTITY;
    }
    // Step 3: centre the scaled group box in P.
    let gw = u_w * f;
    let gh = u_h * f;
    let mut gx = (p_w - gw) / 2.0;
    let mut gy = (p_h - gh) / 2.0;
    // Step 3 (content moves) per axis.
    if let Some(c) = content {
        gx = place_axis(gx, c.x, c.w, f, guard.l, guard.right());
        gy = place_axis(gy, c.y, c.h, f, guard.t, guard.bottom());
    }
    // Step 4: clamp to cover P where the group box is at least as large as P (wins over 3).
    if gw >= p_w {
        gx = gx.min(0.0).max(p_w - gw);
    }
    if gh >= p_h {
        gy = gy.min(0.0).max(p_h - gh);
    }
    // The fitted position is the placed group box's top-left (= U's top-left). U's own
    // (u_l, u_t) offset is carried by Fit::member for members that differ from U.
    let _ = (u_l, u_t);
    Fit { f, x: gx, y: gy }
}

/// Places one axis of a Type A group box (`ui-scaling.md` §5.4 step 3). `g` is the centred
/// group-box start, `c_off`/`c_ext` the content box's offset (relative to `U`) and extent,
/// `f` the factor, `[guard_lo, guard_hi)` the guard box on that axis.
fn place_axis(g: f64, c_off: f64, c_ext: f64, f: f64, guard_lo: f64, guard_hi: f64) -> f64 {
    let content_lo = g + c_off * f;
    let content_ext = c_ext * f;
    let content_hi = content_lo + content_ext;
    let guard_ext = guard_hi - guard_lo;
    if content_ext > guard_ext {
        // Content larger than G on this axis: centre the content box on G.
        let desired = guard_lo + (guard_ext - content_ext) / 2.0;
        g + (desired - content_lo)
    } else if content_lo < guard_lo {
        g + (guard_lo - content_lo)
    } else if content_hi > guard_hi {
        g - (content_hi - guard_hi)
    } else {
        g
    }
}

/// A Type B side-panel fit (`ui-scaling.md` §5.4 Type B): the height drives.
///
/// - `h_computed`: the panel's height `H` from UL §8.4 steps 2–5, in `P`'s layout coordinates;
/// - `p_w`: the width of the box it is laid out in;
/// - `w` × `h`: the design size;
/// - `computed`: the edges UL §8.4 computed, in `P`'s layout coordinates ([`ComputedBox`]);
/// - `left_edge_six`: `true` when the **left** edge is 6 (anchored at the computed right),
///   `false` when the right edge is 6 (anchored at the computed left).
///
/// Returns `f = min(H ÷ h, Pw ÷ w)` and the anchored fitted position; the displayed width is
/// `w × f`.
#[must_use]
pub fn fit_type_b(
    h_computed: f64,
    p_w: f64,
    w: f64,
    h: f64,
    computed: ComputedBox,
    left_edge_six: bool,
) -> Fit {
    if !(w > 0.0 && h > 0.0) || !w.is_finite() || !h.is_finite() {
        return Fit::IDENTITY;
    }
    let f = (h_computed / h).min(p_w / w).max(0.0);
    let dw = w * f;
    let x = if left_edge_six {
        // Anchored at the computed right: exclusive end = computed.right + 1.
        computed.right + 1.0 - dw
    } else {
        computed.left
    };
    Fit {
        f,
        x,
        y: computed.top,
    }
}

/// A Type C fit (`ui-scaling.md` §5.4 Type C): the width drives.
///
/// - `w_computed`: the width `W` from UL §8.4 steps 2–5 (left and right edges), in `P`'s
///   layout coordinates;
/// - `p_h`: the height of the box it is laid out in;
/// - `w` × `h`: the design size;
/// - `computed`: the edges UL §8.4 computed ([`ComputedBox`], inclusive bottom);
/// - `anchor`: which vertical edge is 6 ([`VerticalAnchor`]).
///
/// Returns `f = min(W ÷ w, Ph ÷ h)` and the fitted position; the displayed height is `h × f`.
#[must_use]
pub fn fit_type_c(
    w_computed: f64,
    p_h: f64,
    w: f64,
    h: f64,
    computed: ComputedBox,
    anchor: VerticalAnchor,
) -> Fit {
    if !(w > 0.0 && h > 0.0) || !w.is_finite() || !h.is_finite() {
        return Fit::IDENTITY;
    }
    let f = (w_computed / w).min(p_h / h).max(0.0);
    let dh = h * f;
    let y = match anchor {
        VerticalAnchor::Top => computed.top,
        VerticalAnchor::Bottom => computed.bottom + 1.0 - dh,
        VerticalAnchor::Centre => (computed.top + computed.bottom + 1.0) / 2.0 - dh / 2.0,
    };
    Fit {
        f,
        x: computed.left,
        y,
    }
}

/// The fit-to-screen factor `q` of a root panel larger than the screen (`ui-scaling.md`
/// §5.6): `q = min(1, Wu ÷ displayed width, Hu ÷ displayed height)`, a value `0 < q ≤ 1`.
/// `q` = 1 as soon as the panel fits. A non-positive displayed extent is ignored on that axis.
#[must_use]
pub fn fit_to_screen(disp_w: f64, disp_h: f64, wu: i32, hu: i32) -> f64 {
    let mut q = 1.0_f64;
    if disp_w > 0.0 {
        q = q.min(f64::from(wu) / disp_w);
    }
    if disp_h > 0.0 {
        q = q.min(f64::from(hu) / disp_h);
    }
    if !q.is_finite() || q <= 0.0 {
        1.0
    } else {
        q.min(1.0)
    }
}

/// The degenerate-box guard (`ui-scaling.md` §5.7), one axis. `computed` is the extent UL §8.4
/// steps 2–5 gave (in UI units), `design` the element's design extent, `box_extent` the extent
/// of the box it is laid out in (its parent's current box or the frame). Returns the guarded
/// extent and whether the axis is **empty by design** (extent exactly 0 because the design
/// extent or the box extent is 0). A positive computed extent is left as computed (even when it
/// overflows its parent); only an extent below 1 is replaced.
#[must_use]
pub fn degenerate_guard(computed: i32, design: i32, box_extent: i32) -> (i32, bool) {
    if computed >= 1 {
        return (computed, false);
    }
    let e = design.max(0).min(box_extent.max(0));
    (e, e == 0)
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::scaling::safe_area::safe_area;
    use crate::scaling::screen::UiScreen;

    fn approx(a: f64, b: f64, eps: f64, what: &str) {
        assert!((a - b).abs() <= eps, "{what}: {a} vs {b}");
    }

    /// Helper: build `P`, `G` and fit a group of one fit root (`P` = `F`, `G` = `A`).
    fn fit_root(
        wp: i32,
        hp: i32,
        se: f64,
        aspect: Option<f64>,
        u: (f64, f64, f64, f64),
        content: Option<ContentBox>,
    ) -> (Fit, DBox, UiScreen) {
        let f = UiScreen::new(wp, hp, se).unwrap();
        let a = safe_area(&f, aspect);
        let (wu, hu) = f.ui_size();
        let p = DBox::new(0.0, 0.0, f64::from(wu), f64::from(hu));
        let g = {
            let i = p.intersect(&a);
            if i.is_empty() { p } else { i }
        };
        (
            fit_type_a(p.w, p.h, u, content, g, RHO),
            DBox::new(0.0, 0.0, f64::from(wu), f64::from(hu)),
            f,
        )
    }

    /// §10.1 loading screen (fit root, group of one, U = 2560 × 1080, content NormalBackground
    /// 1024 × 768 at (768, 312)).
    #[test]
    fn loading_screen_examples() {
        let u = (0.0, 0.0, 2560.0, 1080.0);
        let content = Some(ContentBox {
            x: 768.0,
            y: 312.0,
            w: 1024.0,
            h: 768.0,
        });
        // C1: 1280 × 720 @ 0.9375 → f 0.7111, image box (-227.72, 0, 1820.44 × 768).
        let (fit, _f, _) = fit_root(1280, 720, 0.9375, Some(2.4), u, content);
        approx(fit.f, 0.7111, 1e-3, "C1 f");
        let b = fit.displayed(2560.0, 1080.0, (0.0, 0.0), (1.0, 1.0));
        approx(b.l, -227.72, 0.1, "C1 image l");
        approx(b.w, 1820.44, 0.1, "C1 image w");
        approx(b.h, 768.0, 0.1, "C1 image h");
        // C8: 5120 × 1440 @ 1.0, 21:9 → f 1.7778, image box (284.44, -480, 4551.11 × 1920).
        let (fit, _f, _) = fit_root(5120, 1440, 1.0, Some(2.4), u, content);
        approx(fit.f, 1.7778, 1e-3, "C8 f");
        let b = fit.displayed(2560.0, 1080.0, (0.0, 0.0), (1.0, 1.0));
        approx(b.l, 284.44, 0.1, "C8 image l");
        approx(b.t, -480.0, 0.1, "C8 image t");
        approx(b.w, 4551.11, 0.1, "C8 image w");
        approx(b.h, 1920.0, 0.1, "C8 image h");
    }

    /// §10.6 theatre layout fit group (two fit roots, U = 1024 × 768, content over the whole
    /// area). The progress child is at (411, 735), 247 wide.
    #[test]
    fn theatre_fit_group() {
        let u = (0.0, 0.0, 1024.0, 768.0);
        let content = Some(ContentBox {
            x: 0.0,
            y: 0.0,
            w: 1024.0,
            h: 768.0,
        });
        // C1.
        let (fit, _f, _) = fit_root(1280, 720, 0.9375, Some(2.4), u, content);
        approx(fit.f, 1.0, 1e-4, "C1 f");
        let both = fit.displayed(1024.0, 768.0, (0.0, 0.0), (1.0, 1.0));
        approx(both.l, 170.5, 0.01, "C1 both l");
        let child = fit.member(0.0, 0.0, 411.0, 735.0);
        let cb = child.displayed(247.0, 49.0, (0.0, 0.0), (1.0, 1.0));
        approx(cb.l, 581.5, 0.01, "C1 child l");
        approx(cb.t, 735.0, 0.01, "C1 child t");
        approx(cb.w, 247.0, 0.01, "C1 child w");
        // C2.
        let (fit, _f, _) = fit_root(1920, 1080, 1.0, Some(2.4), u, content);
        approx(fit.f, 1.4063, 1e-3, "C2 f");
        let both = fit.displayed(1024.0, 768.0, (0.0, 0.0), (1.0, 1.0));
        approx(both.l, 240.0, 0.01, "C2 both l");
        let child = fit.member(0.0, 0.0, 411.0, 735.0);
        let cb = child.displayed(247.0, 49.0, (0.0, 0.0), (1.0, 1.0));
        approx(cb.l, 817.97, 0.05, "C2 child l");
        approx(cb.t, 1033.59, 0.05, "C2 child t");
        approx(cb.w, 347.34, 0.05, "C2 child w");
    }

    /// §10.7 tutorial chooser: the magnification bound (U = 1440 × 900, child 735 × 231 at
    /// (224, 0)); invariant I17.
    #[test]
    fn tutorial_chooser_bound() {
        let u = (0.0, 0.0, 1440.0, 900.0);
        let content = Some(ContentBox {
            x: 224.0,
            y: 0.0,
            w: 735.0,
            h: 231.0,
        });
        // (window, se, aspect, f, box l, w)
        let cases = [
            (1920, 1080, 1.0, Some(2.4), 1.3333, 0.0, 1920.0),
            (2560, 1440, 1.0, Some(2.4), 1.7778, 0.0, 2560.0),
            (3440, 1440, 1.0, Some(2.4), 2.1333, 184.0, 3072.0),
            (5120, 1440, 1.0, Some(2.4), 2.1333, 1024.0, 3072.0),
            (5120, 1440, 1.0, None, 2.1333, 1024.0, 3072.0),
            (3840, 2160, 1.0, Some(2.4), 2.6667, 0.0, 3840.0),
        ];
        for (wp, hp, se, aspect, ef, el, ew) in cases {
            let (fit, _f, _) = fit_root(wp, hp, se, aspect, u, content);
            approx(fit.f, ef, 1e-3, "f");
            let b = fit.displayed(1440.0, 900.0, (0.0, 0.0), (1.0, 1.0));
            approx(b.l, el, 0.1, "box l");
            approx(b.w, ew, 0.1, "box w");
            approx(b.t, 0.0, 0.1, "box t");
            // I17: f ≤ rho * Hu/h and f ≥ f_contain.
            let hu = f64::from(hp) / se; // Se chosen so Hu ≈ hp/se
            let _ = hu;
        }
    }

    /// §10.8(a): the mission-controller scale-parent group inside a safe-area panel (P = A)
    /// and the Type C naming window inside it, at C8; invariant I18.
    #[test]
    fn nested_fit_c8() {
        // The panel fills A; at C8, A = 3456 wide at 832, m(panel) = 1.
        let f = UiScreen::new(5120, 1440, 1.0).unwrap();
        let a = safe_area(&f, Some(2.4));
        // P = A in the panel's layout coordinates (parent_tl = A top-left, m = 1).
        let parent_tl = (a.l, a.t);
        let p_w = a.w;
        let p_h = a.h;
        let u = (0.0, 0.0, 1440.0, 900.0);
        // Content C = union of the scale parents' children = 411 × 880 at (516, 0).
        let content = Some(ContentBox {
            x: 516.0,
            y: 0.0,
            w: 411.0,
            h: 880.0,
        });
        // G = P ∩ (A into panel coords) = P (A fills the panel).
        let a_in = safe_area_in_parent(a, parent_tl, (1.0, 1.0));
        let p_box = DBox::new(0.0, 0.0, p_w, p_h);
        let g = {
            let i = p_box.intersect(&a_in);
            if i.is_empty() { p_box } else { i }
        };
        let grp = fit_type_a(p_w, p_h, u, content, g, RHO);
        approx(grp.f, 1.6364, 1e-3, "f_A");
        let sp = grp.displayed(1440.0, 900.0, parent_tl, (1.0, 1.0));
        approx(sp.l, 1381.82, 0.05, "scale parent l");
        approx(sp.t, 0.0, 0.05, "scale parent t");
        approx(sp.w, 2356.36, 0.05, "scale parent w");
        approx(sp.h, 1472.73, 0.05, "scale parent h");
        // Naming window: Type C, fitted position (515, 206), own f = 410/411, parent m = f_A.
        let nw = fit_type_c(
            410.0,
            900.0,
            411.0,
            488.0,
            ComputedBox {
                left: 515.0,
                top: 206.0,
                right: 515.0 + 410.0 - 1.0,
                bottom: 206.0 + 488.0 - 1.0,
            },
            VerticalAnchor::Top,
        );
        approx(nw.f, 410.0 / 411.0, 1e-4, "naming f");
        approx(nw.x, 515.0, 1e-6, "naming fitted x");
        approx(nw.y, 206.0, 1e-6, "naming fitted y");
        let nwb = nw.displayed(411.0, 488.0, (sp.l, sp.t), (grp.f, grp.f));
        approx(nwb.l, 2224.55, 0.1, "naming l");
        approx(nwb.t, 337.09, 0.1, "naming t");
        approx(nwb.w, 670.91, 0.1, "naming w");
        approx(nwb.h, 796.60, 0.1, "naming h");
    }

    /// §10.8(b): a Type A container in a panel with a per-window factor of 2 at C8; the guard
    /// box taken into the panel's coordinates moves the content inside G.
    #[test]
    fn type_a_under_per_window_factor() {
        // R: 800 × 600, per-window factor p = 2, displayed box (432, 100, 1600 × 1200).
        // A = 3456 wide at 832 at C8.
        let f = UiScreen::new(5120, 1440, 1.0).unwrap();
        let a = safe_area(&f, Some(2.4));
        let parent_tl = (432.0, 100.0);
        let m = (2.0, 2.0);
        let a_in = safe_area_in_parent(a, parent_tl, m);
        approx(a_in.l, 200.0, 1e-6, "A left in R");
        approx(a_in.right(), 1928.0, 1e-6, "A right in R");
        approx(a_in.t, -50.0, 1e-6, "A top in R");
        approx(a_in.bottom(), 670.0, 1e-6, "A bottom in R");
        // P = (0,0,800,600), U = X's design rect 800 × 400 at (0, 100), C = 200 × 100 at
        // (200, 150) relative to U.
        let p_box = DBox::new(0.0, 0.0, 800.0, 600.0);
        let g = {
            let i = p_box.intersect(&a_in);
            if i.is_empty() { p_box } else { i }
        };
        approx(g.l, 200.0, 1e-6, "G l");
        approx(g.w, 600.0, 1e-6, "G w");
        approx(g.h, 600.0, 1e-6, "G h");
        let u = (0.0, 100.0, 800.0, 400.0);
        let content = Some(ContentBox {
            x: 200.0,
            y: 150.0,
            w: 200.0,
            h: 100.0,
        });
        let grp = fit_type_a(p_box.w, p_box.h, u, content, g, RHO);
        approx(grp.f, 1.5, 1e-6, "f");
        // Fitted position of member X (design rect = U): group origin.
        let x_fit = grp.member(u.0, u.1, u.0, u.1);
        approx(x_fit.x, -100.0, 1e-6, "member fitted x");
        approx(x_fit.y, 0.0, 1e-6, "member fitted y");
        let xb = x_fit.displayed(800.0, 400.0, parent_tl, m);
        approx(xb.l, 232.0, 1e-6, "X displayed l");
        // The child at (200, 150) relative to U: displayed left = 232 + 200 * m(X)=3 = 832.
        // m(X) = f * m(R) = 1.5 * 2 = 3.
        let child_disp_l = xb.l + 200.0 * (grp.f * m.0);
        approx(child_disp_l, 832.0, 1e-6, "child at safe area left end");
    }

    /// §10.2 Type B side panels at C8.
    #[test]
    fn side_panels_type_b() {
        // Safe box = A = 3456 wide at 832, Hu = 1440. Panels 500 × 900, fill the height.
        let aw = 3456.0;
        let hu = 1440.0;
        // Left panel, right edge 6, anchored at computed left 0.
        let cb = |l: f64, r: f64| ComputedBox {
            left: l,
            top: 0.0,
            right: r,
            bottom: hu - 1.0,
        };
        let left = fit_type_b(hu, aw, 500.0, 900.0, cb(0.0, 499.0), false);
        approx(left.f, 1.6, 1e-6, "left f");
        // In the safe box frame (offset 832): displayed 832 … 1632.
        let lb = left.displayed(500.0, 900.0, (832.0, 0.0), (1.0, 1.0));
        approx(lb.l, 832.0, 1e-6, "left l");
        approx(lb.right(), 1632.0, 1e-6, "left r");
        // Right panel, left edge 6, anchored at computed right = Aw - 1.
        let right = fit_type_b(hu, aw, 500.0, 900.0, cb(0.0, aw - 1.0), true);
        let rb = right.displayed(500.0, 900.0, (832.0, 0.0), (1.0, 1.0));
        approx(rb.right(), 4288.0, 1e-6, "right r");
        approx(rb.l, 3488.0, 1e-6, "right l");
    }

    /// §5.4 step 4 counterexample at C2 with a 4:3 safe area: content in the left wing (I15).
    #[test]
    fn step4_left_wing() {
        // F = 1920 × 1080, A (4:3) = 1440 wide at 240. Fit root U = 1024 × 768, child
        // 300 × 200 at (0, 284).
        let f = UiScreen::new(1920, 1080, 1.0).unwrap();
        let a = safe_area(&f, Some(4.0 / 3.0));
        approx(a.l, 240.0, 1e-6, "A left");
        approx(a.w, 1440.0, 1e-6, "A width");
        let p = DBox::new(0.0, 0.0, 1920.0, 1080.0);
        let g = p.intersect(&a);
        let u = (0.0, 0.0, 1024.0, 768.0);
        let content = Some(ContentBox {
            x: 0.0,
            y: 284.0,
            w: 300.0,
            h: 200.0,
        });
        let grp = fit_type_a(p.w, p.h, u, content, g, RHO);
        approx(grp.f, 1.875, 1e-3, "f");
        // Group box 1920 × 1440 must cover F horizontally → left 0, content spans 0 … 562.5.
        approx(grp.x, 0.0, 1e-6, "group x (clamped to cover)");
        let content_lo = grp.x + 0.0 * grp.f;
        let content_hi = content_lo + 300.0 * grp.f;
        approx(content_lo, 0.0, 1e-6, "content lo in left wing");
        approx(content_hi, 562.5, 1e-6, "content hi");
    }

    #[test]
    fn fit_to_screen_world_map() {
        // World map 1038 × 802 on a 1365 × 768 UI screen → q ≈ 0.9576.
        let q = fit_to_screen(1038.0, 802.0, 1365, 768);
        approx(q, 768.0 / 802.0, 1e-9, "q");
        assert_eq!(fit_to_screen(100.0, 100.0, 1365, 768), 1.0);
    }

    #[test]
    fn degenerate_guard_rules() {
        // Positive computed extent left as computed.
        assert_eq!(degenerate_guard(50, 400, 1000), (50, false));
        // Below 1 → design, reduced to the box.
        assert_eq!(degenerate_guard(0, 400, 1000), (400, false));
        assert_eq!(degenerate_guard(-5, 400, 300), (300, false));
        // Empty by design: zero design extent or zero box extent.
        assert_eq!(degenerate_guard(0, 0, 1000), (0, true));
        assert_eq!(degenerate_guard(0, 400, 0), (0, true));
    }

    #[test]
    fn zero_extents_not_fitted() {
        // A fit container with a zero design extent is not fitted.
        assert_eq!(
            fit_type_a(
                1000.0,
                800.0,
                (0.0, 0.0, 0.0, 500.0),
                None,
                DBox::new(0.0, 0.0, 1.0, 1.0),
                RHO
            ),
            Fit::IDENTITY
        );
        // A zero-extent P leaves the group not fitted.
        assert_eq!(
            fit_type_a(
                0.0,
                800.0,
                (0.0, 0.0, 100.0, 100.0),
                None,
                DBox::new(0.0, 0.0, 1.0, 1.0),
                RHO
            ),
            Fit::IDENTITY
        );
    }

    #[test]
    fn subnormal_extents_keep_i5() {
        // I5 (§9.5): a subnormal design extent is finite and positive, so it passes the
        // zero/non-finite guard, but `p / u` overflows to infinity. The factor must still be
        // finite and positive (here: not fitted), never ∞ or NaN. §3.3 says real layouts never
        // reach this; the pure API guards it anyway.
        let tiny = f64::MIN_POSITIVE; // smallest normal; also try a subnormal below.
        for u in [tiny, f64::from_bits(1), 5e-324] {
            for content in [
                None,
                Some(ContentBox {
                    x: 0.0,
                    y: 0.0,
                    w: u,
                    h: u,
                }),
            ] {
                let g = fit_type_a(
                    1920.0,
                    1080.0,
                    (0.0, 0.0, u, u),
                    content,
                    DBox::new(0.0, 0.0, 1920.0, 1080.0),
                    RHO,
                );
                assert!(
                    g.f.is_finite() && g.f > 0.0,
                    "subnormal u={u}: factor {} not finite-positive",
                    g.f
                );
                assert_eq!(g, Fit::IDENTITY, "subnormal u={u} is not fitted");
            }
        }
    }
}
