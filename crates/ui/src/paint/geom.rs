// SPDX-License-Identifier: GPL-3.0-or-later
//! Integer rectangles in canvas or screen pixels.

/// A half-open integer rectangle `[x0, x1) × [y0, y1)`.
///
/// A rectangle with `x1 <= x0` or `y1 <= y0` is empty. All operations saturate instead of
/// overflowing, so hostile coordinates never panic.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Default)]
pub struct Rect {
    /// Left edge (inclusive).
    pub x0: i32,
    /// Top edge (inclusive).
    pub y0: i32,
    /// Right edge (exclusive).
    pub x1: i32,
    /// Bottom edge (exclusive).
    pub y1: i32,
}

impl Rect {
    /// The empty rectangle at the origin.
    pub const EMPTY: Self = Self::new(0, 0, 0, 0);

    /// A rectangle from its edges.
    #[must_use]
    pub const fn new(x0: i32, y0: i32, x1: i32, y1: i32) -> Self {
        Self { x0, y0, x1, y1 }
    }

    /// A rectangle from its top-left corner and size (saturating).
    #[must_use]
    pub fn from_xywh(x: i32, y: i32, w: u32, h: u32) -> Self {
        Self {
            x0: x,
            y0: y,
            x1: sat(i64::from(x) + i64::from(w)),
            y1: sat(i64::from(y) + i64::from(h)),
        }
    }

    /// The rectangle `[0, w) × [0, h)`.
    #[must_use]
    pub fn of_size(w: u32, h: u32) -> Self {
        Self::from_xywh(0, 0, w, h)
    }

    /// Whether the rectangle holds no pixel.
    #[must_use]
    pub const fn is_empty(&self) -> bool {
        self.x1 <= self.x0 || self.y1 <= self.y0
    }

    /// Width (0 when empty).
    #[must_use]
    pub fn width(&self) -> u32 {
        if self.is_empty() {
            0
        } else {
            u32::try_from(i64::from(self.x1) - i64::from(self.x0)).unwrap_or(u32::MAX)
        }
    }

    /// Height (0 when empty).
    #[must_use]
    pub fn height(&self) -> u32 {
        if self.is_empty() {
            0
        } else {
            u32::try_from(i64::from(self.y1) - i64::from(self.y0)).unwrap_or(u32::MAX)
        }
    }

    /// Number of pixels.
    #[must_use]
    pub fn area(&self) -> u64 {
        u64::from(self.width()) * u64::from(self.height())
    }

    /// The common part of two rectangles (possibly empty).
    #[must_use]
    pub fn intersect(&self, other: &Self) -> Self {
        Self {
            x0: self.x0.max(other.x0),
            y0: self.y0.max(other.y0),
            x1: self.x1.min(other.x1),
            y1: self.y1.min(other.y1),
        }
    }

    /// The bounding box of two rectangles; an empty operand is ignored.
    #[must_use]
    pub fn union(&self, other: &Self) -> Self {
        if self.is_empty() {
            return *other;
        }
        if other.is_empty() {
            return *self;
        }
        Self {
            x0: self.x0.min(other.x0),
            y0: self.y0.min(other.y0),
            x1: self.x1.max(other.x1),
            y1: self.y1.max(other.y1),
        }
    }

    /// Whether `other` lies inside `self` (an empty `other` always does).
    #[must_use]
    pub fn contains_rect(&self, other: &Self) -> bool {
        other.is_empty()
            || (self.x0 <= other.x0
                && self.y0 <= other.y0
                && other.x1 <= self.x1
                && other.y1 <= self.y1)
    }

    /// Whether the pixel (`x`, `y`) lies inside.
    #[must_use]
    pub const fn contains(&self, x: i32, y: i32) -> bool {
        self.x0 <= x && x < self.x1 && self.y0 <= y && y < self.y1
    }

    /// Whether the two rectangles overlap or share an edge (used to merge damage).
    #[must_use]
    pub fn touches(&self, other: &Self) -> bool {
        !self.is_empty()
            && !other.is_empty()
            && self.x0 <= other.x1
            && other.x0 <= self.x1
            && self.y0 <= other.y1
            && other.y0 <= self.y1
    }

    /// The rectangle moved by (`dx`, `dy`) (saturating).
    #[must_use]
    pub fn translate(&self, dx: i32, dy: i32) -> Self {
        Self {
            x0: self.x0.saturating_add(dx),
            y0: self.y0.saturating_add(dy),
            x1: self.x1.saturating_add(dx),
            y1: self.y1.saturating_add(dy),
        }
    }

    /// The parts of `self` outside `cut`, as at most four disjoint rectangles.
    #[must_use]
    pub fn subtract(&self, cut: &Self) -> Vec<Self> {
        let i = self.intersect(cut);
        if i.is_empty() {
            return if self.is_empty() {
                Vec::new()
            } else {
                vec![*self]
            };
        }
        let mut out = Vec::with_capacity(4);
        let parts = [
            Self::new(self.x0, self.y0, self.x1, i.y0),
            Self::new(self.x0, i.y1, self.x1, self.y1),
            Self::new(self.x0, i.y0, i.x0, i.y1),
            Self::new(i.x1, i.y0, self.x1, i.y1),
        ];
        out.extend(parts.into_iter().filter(|r| !r.is_empty()));
        out
    }

    /// The rectangle grown outward to multiples of `step` on every edge (`step` ≥ 1).
    #[must_use]
    pub fn align_out(&self, step: u32) -> Self {
        if self.is_empty() {
            return Self::EMPTY;
        }
        let s = i64::from(step.max(1));
        let down = |v: i32| sat(i64::from(v).div_euclid(s) * s);
        let up = |v: i32| sat((i64::from(v) + s - 1).div_euclid(s) * s);
        Self {
            x0: down(self.x0),
            y0: down(self.y0),
            x1: up(self.x1),
            y1: up(self.y1),
        }
    }
}

/// Saturates an `i64` into the `i32` range.
pub(crate) fn sat(v: i64) -> i32 {
    i32::try_from(v).unwrap_or(if v < 0 { i32::MIN } else { i32::MAX })
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn subtract_covers_difference() {
        let a = Rect::new(0, 0, 10, 10);
        let b = Rect::new(3, 4, 6, 20);
        let parts = a.subtract(&b);
        let total: u64 = parts.iter().map(Rect::area).sum();
        assert_eq!(total, 100 - 3 * 6);
        for p in &parts {
            assert!(p.intersect(&b).is_empty());
            assert!(a.contains_rect(p));
        }
    }

    #[test]
    fn saturating_edges() {
        let r = Rect::from_xywh(i32::MAX - 1, 0, u32::MAX, 1);
        assert_eq!(r.x1, i32::MAX);
        assert_eq!(r.width(), 1);
        assert_eq!(Rect::new(5, 5, 1, 1).area(), 0);
        assert_eq!(
            Rect::new(-3, 1, 257, 300).align_out(256),
            Rect::new(-256, 0, 512, 512)
        );
    }
}
