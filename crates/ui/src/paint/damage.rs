// SPDX-License-Identifier: GPL-3.0-or-later
//! Damage by rectangle (decision 0015 Decision 1.4).
//!
//! Every change that can alter pixels adds rectangles in surface (canvas) coordinates to the
//! surface's damage list. Once per frame (when a paint job takes it) the list is merged:
//! overlapping or touching rectangles are joined; above [`MAX_MERGED`] rectangles, or when
//! they cover more than half the surface, the bounding box is used. A change of the canvas
//! size repaints the canvas in full.

use super::geom::Rect;

/// Most rectangles after a merge before the bounding box is used.
pub const MAX_MERGED: usize = 16;

/// Most rectangles kept before merging; past it the list collapses to its bounding box at
/// once, so a hostile stream of tiny rectangles stays bounded (Tessera cap).
pub const MAX_PENDING: usize = 1024;

/// The pending damage of one surface.
#[derive(Debug, Clone, Default, PartialEq, Eq)]
pub struct DamageList {
    rects: Vec<Rect>,
    full: bool,
}

impl DamageList {
    /// No damage.
    #[must_use]
    pub fn new() -> Self {
        Self::default()
    }

    /// Adds a rectangle; an empty one is ignored.
    pub fn add(&mut self, r: Rect) {
        if r.is_empty() || self.full {
            return;
        }
        self.rects.push(r);
        if self.rects.len() > MAX_PENDING {
            let b = self.bounding_box();
            self.rects.clear();
            self.rects.push(b);
        }
    }

    /// Marks the whole canvas damaged (a size change, a first paint, a skin refresh).
    pub fn add_full(&mut self) {
        self.full = true;
        self.rects.clear();
    }

    /// Whether the whole canvas is damaged.
    #[must_use]
    pub fn is_full(&self) -> bool {
        self.full
    }

    /// Whether there is no damage.
    #[must_use]
    pub fn is_empty(&self) -> bool {
        !self.full && self.rects.is_empty()
    }

    /// Whether the pending damage meets `r`: the whole canvas is damaged, or a pending
    /// rectangle overlaps it (`render.md` §6.12 R13 reads it). Each pending rectangle looked
    /// at takes one of `checks`; when fewer are left than there are rectangles, the answer is
    /// `true` (met, the conservative one) without looking.
    #[must_use]
    pub fn meets(&self, r: &Rect, checks: &mut usize) -> bool {
        if self.full {
            return true;
        }
        if self.rects.len() > *checks {
            *checks = 0;
            return true;
        }
        *checks -= self.rects.len();
        self.rects.iter().any(|d| !d.intersect(r).is_empty())
    }

    /// The bounding box of the pending rectangles (not clipped).
    #[must_use]
    pub fn bounding_box(&self) -> Rect {
        self.rects.iter().fold(Rect::EMPTY, |b, r| b.union(r))
    }

    /// Joins `other` into `self`.
    pub fn absorb(&mut self, other: DamageList) {
        if other.full {
            self.add_full();
        }
        for r in other.rects {
            self.add(r);
        }
    }

    /// Takes the damage, merged and clipped to a canvas of `size` (the rule above). Leaves
    /// the list empty.
    pub fn take_merged(&mut self, size: [u32; 2]) -> Vec<Rect> {
        let canvas = Rect::of_size(size[0], size[1]);
        let full = std::mem::take(&mut self.full);
        let rects = std::mem::take(&mut self.rects);
        if full {
            return if canvas.is_empty() {
                Vec::new()
            } else {
                vec![canvas]
            };
        }
        merge(rects.into_iter().map(|r| r.intersect(&canvas)), canvas)
    }
}

/// Merges rectangles as Decision 1.4 says, clipped to `canvas`.
#[must_use]
pub fn merge(rects: impl IntoIterator<Item = Rect>, canvas: Rect) -> Vec<Rect> {
    let mut out: Vec<Rect> = Vec::new();
    for r in rects {
        let mut r = r.intersect(&canvas);
        if r.is_empty() {
            continue;
        }
        // Join with every rectangle it touches, repeatedly, until none touches.
        while let Some(i) = out.iter().position(|o| o.touches(&r)) {
            r = r.union(&out.swap_remove(i));
        }
        out.push(r);
    }
    let area: u64 = out.iter().map(Rect::area).sum();
    if out.len() > MAX_MERGED || area * 2 > canvas.area() {
        let b = out.iter().fold(Rect::EMPTY, |b, r| b.union(r));
        return if b.is_empty() { Vec::new() } else { vec![b] };
    }
    // Keep a stable order (top to bottom, left to right) for banding.
    out.sort_by_key(|r| (r.y0, r.x0));
    out
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn touching_rects_join() {
        let mut d = DamageList::new();
        d.add(Rect::new(0, 0, 10, 10));
        d.add(Rect::new(10, 0, 20, 10));
        d.add(Rect::new(50, 50, 52, 52));
        let m = d.take_merged([1000, 1000]);
        assert_eq!(m, vec![Rect::new(0, 0, 20, 10), Rect::new(50, 50, 52, 52)]);
        assert!(d.is_empty());
    }

    #[test]
    fn many_rects_become_bbox() {
        let mut d = DamageList::new();
        for i in 0..20 {
            d.add(Rect::new(i * 10, 0, i * 10 + 2, 2));
        }
        assert_eq!(d.take_merged([1000, 1000]), vec![Rect::new(0, 0, 192, 2)]);
    }

    #[test]
    fn half_coverage_becomes_bbox() {
        let mut d = DamageList::new();
        d.add(Rect::new(0, 0, 10, 8));
        d.add(Rect::new(12, 0, 20, 8));
        assert_eq!(d.take_merged([20, 10]), vec![Rect::new(0, 0, 20, 8)]);
    }

    #[test]
    fn full_and_bounded() {
        let mut d = DamageList::new();
        for i in 0..5000 {
            d.add(Rect::new(i, i, i + 1, i + 1));
        }
        assert!(d.rects.len() <= MAX_PENDING);
        d.add_full();
        assert_eq!(d.take_merged([3, 4]), vec![Rect::new(0, 0, 3, 4)]);
    }
}
