// SPDX-License-Identifier: GPL-3.0-or-later
//! Layout arithmetic: the design-time rule (`ui-layout.md` §8.2), the run-time rule
//! (§8.4 steps 2–6) and the plugin edge-attachment step (`lua-plugin-api.md` §13.16).
//!
//! Everything here is a pure function of boxes and modes. Products and quotients that the
//! specs give as 32-bit float operations are done in `f32` in the stated order (U25: 64-bit
//! floats can change results by one pixel); integer sums use 64 bits and saturate.

use crate::geom::{IBox, half, sat, trunc_f32};

/// Edge modes (`ui-layout.md` §5.8).
pub mod edge {
    /// Keep the current position.
    pub const KEEP: u32 = 0;
    /// Centre.
    pub const CENTRE: u32 = 1;
    /// Mirror.
    pub const MIRROR: u32 = 2;
    /// Scale by the parent's growth ratio.
    pub const SCALE: u32 = 3;
    /// Follow the parent's far side (left/top), near side (right/bottom).
    pub const OPPOSITE: u32 = 4;
    /// Follow the near side (left/top), far side (right/bottom).
    pub const SAME: u32 = 5;
    /// Aspect mode.
    pub const ASPECT: u32 = 6;
}

fn ratio(n: i32, o: i32) -> f32 {
    if o == 0 {
        0.0
    } else {
        let r = n as f32 / o as f32;
        if r.is_nan() { 0.0 } else { r }
    }
}

fn add(a: i32, b: i32) -> i32 {
    sat(i64::from(a) + i64::from(b))
}

fn sub(a: i32, b: i32) -> i32 {
    sat(i64::from(a) - i64::from(b))
}

/// The design-time rule of `ui-layout.md` §8.2: the new box of a child with design box `d`
/// and edge modes `edges` (left, top, right, bottom) when its parent changes from `ow` × `oh`
/// to `nw` × `nh`. Modes 2, 6 and unknown values leave their edge unchanged (the client
/// reports an error for them; the other edges still apply).
#[must_use]
pub fn design_time(d: IBox, edges: [u32; 4], ow: i32, oh: i32, nw: i32, nh: i32) -> IBox {
    let (w, h) = (d.w(), d.h());
    let (dw, dh) = (sub(nw, ow), sub(nh, oh));
    let (rx, ry) = (ratio(nw, ow), ratio(nh, oh));
    let left = match edges[0] {
        edge::CENTRE => sub(half(nw), half(w)),
        edge::SCALE => trunc_f32(d.l as f32 * rx),
        edge::OPPOSITE => add(d.l, dw),
        _ => d.l,
    };
    let top = match edges[1] {
        edge::CENTRE => sub(half(nh), half(h)),
        edge::SCALE => trunc_f32(d.t as f32 * ry),
        edge::OPPOSITE => add(d.t, dh),
        _ => d.t,
    };
    let right = match edges[2] {
        edge::CENTRE => sub(add(half(nw), half(w)), 1),
        edge::SCALE => trunc_f32(d.r as f32 * rx),
        edge::SAME => add(d.r, dw),
        _ => d.r,
    };
    let bottom = match edges[3] {
        edge::CENTRE => sub(add(half(nh), half(h)), 1),
        edge::SCALE => trunc_f32(d.b as f32 * ry),
        edge::SAME => add(d.b, dh),
        _ => d.b,
    };
    IBox::new(left, top, right, bottom)
}

/// Inputs of the run-time rule (`ui-layout.md` §8.4 steps 2–5).
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct RuntimeInput {
    /// The element's design box *D* (relative to its parent).
    pub design: IBox,
    /// The element's current box.
    pub current: IBox,
    /// Edge modes (left, top, right, bottom).
    pub edges: [u32; 4],
    /// The reference box *O* (§8.3).
    pub old: IBox,
    /// The reference box *N* (§8.3).
    pub new: IBox,
    /// Whether the element has finished creation (§8.4 step 4).
    pub finished: bool,
    /// The element's own flag bit 1, keep inside the parent (§8.4 step 5).
    pub keep_inside: bool,
}

/// Steps 2–5 of the run-time rule of `ui-layout.md` §8.4: the element's new box relative to
/// its parent. (Step 1, the Stretch-parent test, and step 6, setting the box, belong to the
/// tree.) A final Tessera step rescues a box that comes out with a non-positive width or
/// height (§8.4 step 6 OPEN, the loadscreen meter and label).
#[must_use]
pub fn runtime(inp: &RuntimeInput) -> IBox {
    let d = inp.design;
    let (o, n) = (inp.old, inp.new);
    let (w, h) = (d.w(), d.h());
    let (ow, oh, nw, nh) = (o.w(), o.h(), n.w(), n.h());
    let (dw, dh) = (sub(nw, ow), sub(nh, oh));
    let (rx, ry) = (ratio(nw, ow), ratio(nh, oh));
    let [ml, mt, mr, mb] = inp.edges;
    // Step 2: first pass.
    let mut l = match ml {
        edge::CENTRE => sub(half(nw), half(w)),
        edge::MIRROR => sub(sub(o.r, o.l), d.r),
        edge::SCALE => trunc_f32(d.l as f32 * rx),
        edge::OPPOSITE => add(d.l, dw),
        edge::ASPECT => trunc_f32(half(nw) as f32 - w as f32 * ry * 0.5),
        _ => d.l,
    };
    let mut t = match mt {
        edge::CENTRE => sub(half(nh), half(h)),
        edge::MIRROR => sub(sub(o.b, o.t), d.b),
        edge::SCALE => trunc_f32(d.t as f32 * ry),
        edge::OPPOSITE => add(d.t, dh),
        edge::ASPECT => trunc_f32(half(nh) as f32 - h as f32 * rx * 0.5),
        _ => d.t,
    };
    let mut r = match mr {
        edge::CENTRE => sub(add(half(nw), half(w)), 1),
        edge::MIRROR => sub(sub(n.r, n.l), d.l),
        edge::SCALE => trunc_f32(d.r as f32 * rx),
        edge::SAME => add(d.r, dw),
        edge::ASPECT => sub(trunc_f32(half(nw) as f32 + w as f32 * ry * 0.5), 1),
        _ => d.r,
    };
    let mut b = match mb {
        edge::CENTRE => sub(add(half(nh), half(h)), 1),
        edge::MIRROR => sub(sub(n.b, n.t), d.t),
        edge::SCALE => trunc_f32(d.b as f32 * ry),
        edge::SAME => add(d.b, dh),
        edge::ASPECT => sub(trunc_f32(half(nh) as f32 + h as f32 * rx * 0.5), 1),
        _ => d.b,
    };
    // Step 3: aspect pass, 32-bit floats (U25).
    let a = if h == 0 { 1.0 } else { w as f32 / h as f32 };
    let bb = if w == 0 { 1.0 } else { h as f32 / w as f32 };
    if ml == edge::ASPECT {
        l = trunc_f32(r as f32 - add(sub(b, t), 1) as f32 * a);
    }
    if mr == edge::ASPECT {
        r = sub(trunc_f32(l as f32 + add(sub(b, t), 1) as f32 * a), 1);
    }
    if mt == edge::ASPECT {
        t = trunc_f32(b as f32 - add(sub(r, l), 1) as f32 * bb);
    }
    if mb == edge::ASPECT {
        b = sub(trunc_f32(t as f32 + add(sub(r, l), 1) as f32 * bb), 1);
    }
    // Step 4: mode 0 keeps the current position.
    let cur = inp.current;
    if cur.w() != 0 || cur.h() != 0 || inp.finished {
        if ml == edge::KEEP {
            l = cur.l;
        }
        if mt == edge::KEEP {
            t = cur.t;
        }
        if mr == edge::KEEP {
            r = cur.r;
        }
        if mb == edge::KEEP {
            b = cur.b;
        }
    }
    // Step 5: keep inside the parent (bounds are the parent's width and height, U25).
    if inp.keep_inside {
        l = l.max(0);
        t = t.max(0);
        r = r.min(nw);
        b = b.min(nh);
    }
    // Tessera fallback for a degenerate box (`ui-layout.md` §8.4 step 6 OPEN, implementer
    // note): when the parent is far smaller than the design parent, an edge in SAME or
    // OPPOSITE mode can carry the far edge past the near edge and give a non-positive width
    // or height (the loadscreen meter and label, `travel-windows.md` §5.2). Such a box draws
    // nothing. Rather than drop the element, keep its design extent on each degenerate axis
    // and shift the box so it lies inside the parent's current box (0 .. Nw − 1, 0 .. Nh −
    // 1). Non-degenerate boxes (including those that overflow the parent) are left untouched.
    if sub(r, l) < 0 {
        let ew = w.clamp(1, nw.max(1));
        r = add(l, sub(ew, 1));
        if r > sub(nw, 1) {
            let s = sub(r, sub(nw, 1));
            l = sub(l, s);
            r = sub(r, s);
        }
        if l < 0 {
            r = sub(r, l);
            l = 0;
        }
    }
    if sub(b, t) < 0 {
        let eh = h.clamp(1, nh.max(1));
        b = add(t, sub(eh, 1));
        if b > sub(nh, 1) {
            let s = sub(b, sub(nh, 1));
            t = sub(t, s);
            b = sub(b, s);
        }
        if t < 0 {
            b = sub(b, t);
            t = 0;
        }
    }
    IBox::new(l, t, r, b)
}

/// Plugin edge-attachment types (`lua-plugin-api.md` Appendix A.6).
pub mod attach {
    /// None.
    pub const NONE: i32 = 0;
    /// Center.
    pub const CENTER: i32 = 1;
    /// Mirror.
    pub const MIRROR: i32 = 2;
    /// Proportional.
    pub const PROPORTIONAL: i32 = 3;
    /// Opposite.
    pub const OPPOSITE: i32 = 4;
    /// Same.
    pub const SAME: i32 = 5;
    /// RatioProportional.
    pub const RATIO: i32 = 6;
}

/// The layout step of `lua-plugin-api.md` §13.16 for a control with attachments
/// `edges`, reference rectangle `r`, parent reference size `q` (`QW`, `QH`), current box
/// `current` and parent size `pw` × `ph`. Returns the new box (before the size limits).
#[must_use]
pub fn attachment_step(
    edges: [i32; 4],
    r: IBox,
    q: (i32, i32),
    current: IBox,
    pw: i32,
    ph: i32,
) -> IBox {
    let (qw, qh) = q;
    let (dw, dh) = (sub(pw, qw), sub(ph, qh));
    let (sx, sy) = (ratio(pw, qw), ratio(ph, qh));
    let (rw, rh) = (r.w(), r.h());
    let mut left = match edges[0] {
        attach::NONE => current.l,
        attach::CENTER => sub(half(pw), half(rw)),
        attach::MIRROR => sub(sub(qw, 1), r.r),
        attach::PROPORTIONAL => trunc_f32(r.l as f32 * sx),
        attach::OPPOSITE => add(r.l, dw),
        attach::RATIO => trunc_f32(half(pw) as f32 - rw as f32 * sy * 0.5),
        _ => r.l,
    };
    let mut top = match edges[1] {
        attach::NONE => current.t,
        attach::CENTER => sub(half(ph), half(rh)),
        attach::MIRROR => sub(sub(qh, 1), r.b),
        attach::PROPORTIONAL => trunc_f32(r.t as f32 * sy),
        attach::OPPOSITE => add(r.t, dh),
        attach::RATIO => trunc_f32(half(ph) as f32 - rh as f32 * sx * 0.5),
        _ => r.t,
    };
    let mut right = match edges[2] {
        attach::NONE => current.r,
        attach::CENTER => add(sub(half(pw), 1), half(rw)),
        attach::MIRROR => sub(sub(pw, 1), r.l),
        attach::PROPORTIONAL => trunc_f32(r.r as f32 * sx),
        attach::OPPOSITE => r.r,
        attach::SAME => add(r.r, dw),
        attach::RATIO => sub(trunc_f32(rw as f32 * sy * 0.5 + half(pw) as f32), 1),
        _ => r.r,
    };
    let mut bottom = match edges[3] {
        attach::NONE => current.b,
        attach::CENTER => add(sub(half(ph), 1), half(rh)),
        attach::MIRROR => sub(sub(ph, 1), r.t),
        attach::PROPORTIONAL => trunc_f32(r.b as f32 * sy),
        attach::OPPOSITE => r.b,
        attach::SAME => add(r.b, dh),
        attach::RATIO => sub(trunc_f32(rh as f32 * sx * 0.5 + half(ph) as f32), 1),
        _ => r.b,
    };
    // `left`/`top` for Same and `right`/`bottom` for Opposite fall in `_` above via their
    // own arms: Same keeps `RL`/`RT`, Opposite keeps `RR`/`RB`.
    if edges[0] == attach::SAME {
        left = r.l;
    }
    if edges[1] == attach::SAME {
        top = r.t;
    }
    let aw = if rh == 0 { 1.0 } else { rw as f32 / rh as f32 };
    let ah = if rw == 0 { 1.0 } else { rh as f32 / rw as f32 };
    if edges[0] == attach::RATIO {
        left = trunc_f32(right as f32 - add(sub(bottom, top), 1) as f32 * aw);
    }
    if edges[2] == attach::RATIO {
        right = sub(
            trunc_f32(left as f32 + add(sub(bottom, top), 1) as f32 * aw),
            1,
        );
    }
    if edges[1] == attach::RATIO {
        top = trunc_f32(bottom as f32 - add(sub(right, left), 1) as f32 * ah);
    }
    if edges[3] == attach::RATIO {
        bottom = sub(
            trunc_f32(top as f32 + add(sub(right, left), 1) as f32 * ah),
            1,
        );
    }
    IBox::new(left, top, right, bottom)
}
