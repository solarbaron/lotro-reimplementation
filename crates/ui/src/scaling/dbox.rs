// SPDX-License-Identifier: GPL-3.0-or-later
//! The displayed box (`ui-scaling.md` §3.4): a rectangle in UI units relative to the screen,
//! as real numbers, half-open `[l, l + w) × [t, t + h)`.
//!
//! Displayed boxes are the "one space" of §3.5: hit testing, tooltips, drag and drop, saved
//! positions and the plugin API all work in UI units with these boxes. With every node scale
//! 1 and `Se` = 1 a displayed box equals its layout box made absolute (integers), so the
//! integer rules of `ui-layout.md` and `ui-behaviour.md` hold unchanged there.

/// A displayed box: top-left `(l, t)` and size `w` × `h`, in UI units as real numbers
/// (`ui-scaling.md` §3.4). Half-open: it covers `[l, l + w) × [t, t + h)`.
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct DBox {
    /// Left edge.
    pub l: f64,
    /// Top edge.
    pub t: f64,
    /// Width (never negative in a well-formed box; 0 means empty by design, §5.7).
    pub w: f64,
    /// Height.
    pub h: f64,
}

impl DBox {
    /// A box from its top-left and size.
    #[must_use]
    pub const fn new(l: f64, t: f64, w: f64, h: f64) -> Self {
        Self { l, t, w, h }
    }

    /// The exclusive right edge, `l + w`.
    #[must_use]
    pub fn right(&self) -> f64 {
        self.l + self.w
    }

    /// The exclusive bottom edge, `t + h`.
    #[must_use]
    pub fn bottom(&self) -> f64 {
        self.t + self.h
    }

    /// Whether the box covers no point (a zero or negative extent on either axis).
    #[must_use]
    pub fn is_empty(&self) -> bool {
        self.w <= 0.0 || self.h <= 0.0
    }

    /// Whether `(ux, uy)` lies inside the half-open box (`ui-scaling.md` §3.5 hit test).
    #[must_use]
    pub fn contains(&self, ux: f64, uy: f64) -> bool {
        ux >= self.l && ux < self.right() && uy >= self.t && uy < self.bottom()
    }

    /// The box moved by `(dx, dy)`.
    #[must_use]
    pub fn offset(&self, dx: f64, dy: f64) -> Self {
        Self {
            l: self.l + dx,
            t: self.t + dy,
            w: self.w,
            h: self.h,
        }
    }

    /// The common part of two boxes (an empty box when they do not overlap).
    #[must_use]
    pub fn intersect(&self, o: &Self) -> Self {
        let l = self.l.max(o.l);
        let t = self.t.max(o.t);
        let r = self.right().min(o.right());
        let b = self.bottom().min(o.bottom());
        Self {
            l,
            t,
            w: (r - l).max(0.0),
            h: (b - t).max(0.0),
        }
    }

    /// Whether this box lies inside `o` (within `eps` UI units on every edge).
    #[must_use]
    pub fn inside(&self, o: &Self, eps: f64) -> bool {
        self.l >= o.l - eps
            && self.t >= o.t - eps
            && self.right() <= o.right() + eps
            && self.bottom() <= o.bottom() + eps
    }
}
