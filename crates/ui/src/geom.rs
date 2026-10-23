// SPDX-License-Identifier: GPL-3.0-or-later
//! Inclusive boxes, as `ui-layout.md` §8.1 uses them: (left, top, right, bottom) with
//! width = right − left + 1. Coordinates are `i32`; every derived value is computed in 64
//! bits and saturated (`ui-layout.md` §12.1).

/// Saturates a 64-bit value to `i32`.
#[inline]
#[must_use]
pub fn sat(v: i64) -> i32 {
    v.clamp(i64::from(i32::MIN), i64::from(i32::MAX)) as i32
}

/// Truncates a 32-bit float toward zero to an `i32`; NaN gives 0 (`ui-layout.md` §12.1)
/// and out-of-range values saturate.
#[inline]
#[must_use]
pub fn trunc_f32(v: f32) -> i32 {
    if v.is_nan() {
        0
    } else {
        v.trunc().clamp(i32::MIN as f32, i32::MAX as f32) as i32
    }
}

/// Truncates a 64-bit float toward zero; NaN gives 0.
#[inline]
#[must_use]
pub fn trunc_f64(v: f64) -> i32 {
    if v.is_nan() {
        0
    } else {
        v.trunc().clamp(f64::from(i32::MIN), f64::from(i32::MAX)) as i32
    }
}

/// Largest coordinate magnitude the tree stores (Tessera cap against hostile data): boxes,
/// pointer positions and the screen size are clamped to ±2²⁴, so sums of a few of them stay
/// far from `i32` overflow.
pub const COORD_LIMIT: i32 = 1 << 24;

/// Clamps a coordinate to ±[`COORD_LIMIT`].
#[inline]
#[must_use]
pub fn clamp_coord(v: i32) -> i32 {
    v.clamp(-COORD_LIMIT, COORD_LIMIT)
}

/// Saturating `a + b`.
#[inline]
#[must_use]
pub fn add(a: i32, b: i32) -> i32 {
    a.saturating_add(b)
}

/// Saturating `a − b`.
#[inline]
#[must_use]
pub fn sub(a: i32, b: i32) -> i32 {
    a.saturating_sub(b)
}

/// `half(n)`: `n ÷ 2` rounded toward zero (`ui-layout.md` §8.1).
#[inline]
#[must_use]
pub fn half(n: i32) -> i32 {
    n / 2
}

/// An inclusive box relative to some origin (`ui-layout.md` §2, §8.1).
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Default)]
pub struct IBox {
    /// Left column.
    pub l: i32,
    /// Top row.
    pub t: i32,
    /// Right column (inclusive).
    pub r: i32,
    /// Bottom row (inclusive).
    pub b: i32,
}

impl IBox {
    /// A box from its four edges.
    #[must_use]
    pub const fn new(l: i32, t: i32, r: i32, b: i32) -> Self {
        Self { l, t, r, b }
    }

    /// The box of a rectangle at (`x`, `y`) of size `w` × `h`: (x, y, x + w − 1, y + h − 1).
    #[must_use]
    pub fn from_xywh(x: i32, y: i32, w: i32, h: i32) -> Self {
        Self {
            l: x,
            t: y,
            r: sat(i64::from(x) + i64::from(w) - 1),
            b: sat(i64::from(y) + i64::from(h) - 1),
        }
    }

    /// Width, right − left + 1 (may be ≤ 0).
    #[must_use]
    pub fn w(&self) -> i32 {
        sat(i64::from(self.r) - i64::from(self.l) + 1)
    }

    /// Height, bottom − top + 1 (may be ≤ 0).
    #[must_use]
    pub fn h(&self) -> i32 {
        sat(i64::from(self.b) - i64::from(self.t) + 1)
    }

    /// Whether the box covers no pixel.
    #[must_use]
    pub fn is_empty(&self) -> bool {
        self.w() <= 0 || self.h() <= 0
    }

    /// Whether (`x`, `y`) lies inside (inclusive edges).
    #[must_use]
    pub fn contains(&self, x: i32, y: i32) -> bool {
        self.l <= x && x <= self.r && self.t <= y && y <= self.b
    }

    /// The box moved by (`dx`, `dy`).
    #[must_use]
    pub fn offset(&self, dx: i32, dy: i32) -> Self {
        Self {
            l: sat(i64::from(self.l) + i64::from(dx)),
            t: sat(i64::from(self.t) + i64::from(dy)),
            r: sat(i64::from(self.r) + i64::from(dx)),
            b: sat(i64::from(self.b) + i64::from(dy)),
        }
    }

    /// The intersection (possibly empty).
    #[must_use]
    pub fn intersect(&self, o: &Self) -> Self {
        Self {
            l: self.l.max(o.l),
            t: self.t.max(o.t),
            r: self.r.min(o.r),
            b: self.b.min(o.b),
        }
    }

    /// The smallest box holding both (an empty box is ignored).
    #[must_use]
    pub fn union(&self, o: &Self) -> Self {
        if self.is_empty() {
            return *o;
        }
        if o.is_empty() {
            return *self;
        }
        Self {
            l: self.l.min(o.l),
            t: self.t.min(o.t),
            r: self.r.max(o.r),
            b: self.b.max(o.b),
        }
    }

    /// The box with every edge clamped to ±[`COORD_LIMIT`].
    #[must_use]
    pub fn clamped(&self) -> Self {
        Self {
            l: clamp_coord(self.l),
            t: clamp_coord(self.t),
            r: clamp_coord(self.r),
            b: clamp_coord(self.b),
        }
    }

    /// The same box as a half-open paint rectangle.
    #[must_use]
    pub fn to_rect(&self) -> crate::paint::Rect {
        if self.is_empty() {
            return crate::paint::Rect::EMPTY;
        }
        crate::paint::Rect::new(
            self.l,
            self.t,
            sat(i64::from(self.r) + 1),
            sat(i64::from(self.b) + 1),
        )
    }
}

/// Rounds a real coordinate to the nearest integer, halves up (`ui-scaling.md` §3.4–§3.5). A
/// value that is not finite becomes 0; the result is clamped into `i32`.
#[must_use]
pub fn round_half_up(v: f64) -> i32 {
    if !v.is_finite() {
        return 0;
    }
    (v + 0.5)
        .floor()
        .clamp(f64::from(i32::MIN), f64::from(i32::MAX)) as i32
}
