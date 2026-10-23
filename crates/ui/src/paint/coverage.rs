// SPDX-License-Identifier: GPL-3.0-or-later
//! Coverage regions of nested entries (decision 0015 Decision 1.7 steps 1–3, Amendment 1
//! item 7 M5-2).
//!
//! The coverage region of a nested entry *N* is the rectangle of its parent surface *P*'s
//! canvas in which *P*'s content drawn after *N* moves to *N*'s canvas above. It is computed
//! from displayed boxes in the root's unrotated space:
//!
//! 1. **Footprint.** *N*'s displayed box; with an **own rotation** the square around the
//!    circle through its corners, so turning never changes it.
//! 2. **Join** the footprints of *N* and every nested entry below it.
//!
//!    **M5-2, option (a)** (chosen here): when *N* has its own rotation and is **not** a
//!    window, the nested entries below it turn with it about its centre and are not clipped
//!    by it. Their footprints are joined with *N*'s box in *N*'s own (unrotated) frame
//!    first, and the square is then taken around the circle centred on *N*'s centre through
//!    the farthest corner of that join. When *N* is a window its nested entries are clipped
//!    by it, so its own box gives the square.
//! 3. **Convert** to *P*'s canvas: subtract *P*'s displayed origin, divide by *P*'s
//!    composition scale, subtract *P*'s canvas origin offset, round outward, clip to the
//!    canvas.

use super::geom::{Rect, sat};

/// Angles within this many degrees of 0 count as no rotation (decision 0015 §2.3).
pub const ANGLE_EPSILON: f32 = 0.0002;

/// Most nesting levels a footprint tree may have (the depth cap of §7 is 8; deeper input is
/// ignored).
pub const MAX_FOOTPRINT_DEPTH: usize = 16;

/// Whether a surface holding `angles` (degrees, x, y, z) holds a rotation: some angle is
/// finite and not within [`ANGLE_EPSILON`] of 0. Non-finite angles count as 0 (§7).
#[must_use]
pub fn holds_rotation(angles: [f32; 3]) -> bool {
    angles
        .iter()
        .any(|a| a.is_finite() && a.abs() > ANGLE_EPSILON)
}

/// A rectangle with real edges (footprints before rounding).
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct RectF {
    /// Left.
    pub x0: f64,
    /// Top.
    pub y0: f64,
    /// Right.
    pub x1: f64,
    /// Bottom.
    pub y1: f64,
}

impl RectF {
    /// From an integer rectangle.
    #[must_use]
    pub fn from_rect(r: &Rect) -> Self {
        Self {
            x0: f64::from(r.x0),
            y0: f64::from(r.y0),
            x1: f64::from(r.x1),
            y1: f64::from(r.y1),
        }
    }

    fn union(&self, o: &Self) -> Self {
        Self {
            x0: self.x0.min(o.x0),
            y0: self.y0.min(o.y0),
            x1: self.x1.max(o.x1),
            y1: self.y1.max(o.y1),
        }
    }

    /// Rounds outward: left and top down, right and bottom up (saturating).
    #[must_use]
    pub fn round_out(&self) -> Rect {
        let f = |v: f64| -> i32 {
            if v.is_nan() {
                0
            } else {
                sat(v.clamp(i64::MIN as f64, i64::MAX as f64) as i64)
            }
        };
        Rect::new(
            f(self.x0.floor()),
            f(self.y0.floor()),
            f(self.x1.ceil()),
            f(self.y1.ceil()),
        )
    }
}

/// A nested entry and the nested entries below it, for the footprint of step 1–2.
#[derive(Debug, Clone, Default)]
pub struct FootprintNode {
    /// The entry's displayed box, absolute (the root's unrotated space).
    pub displayed: Rect,
    /// Whether its surface holds an own rotation (an angle set while the surface exists,
    /// kept until the surface is released, Decision 1.7 step 1).
    pub own_rotation: bool,
    /// Whether the entry is a window (root panel): its nested entries are clipped by it.
    pub window: bool,
    /// The nested entries directly below it (entries of its own surface), at any depth
    /// through their own `nested`.
    pub nested: Vec<FootprintNode>,
}

/// The footprint of `n` joined with the footprints of every nested entry below it (steps 1
/// and 2, with M5-2 option (a)).
#[must_use]
pub fn footprint(n: &FootprintNode) -> RectF {
    footprint_at(n, 0)
}

fn footprint_at(n: &FootprintNode, depth: usize) -> RectF {
    let own = RectF::from_rect(&n.displayed);
    let mut join = own;
    if depth < MAX_FOOTPRINT_DEPTH && !(n.own_rotation && n.window) {
        for c in &n.nested {
            join = join.union(&footprint_at(c, depth + 1));
        }
    }
    if !n.own_rotation {
        return join;
    }
    // Square around the circle centred on N's centre through the farthest corner of the
    // join (for a window, of its own box).
    let cx = (own.x0 + own.x1) * 0.5;
    let cy = (own.y0 + own.y1) * 0.5;
    let dx = (join.x0 - cx).abs().max((join.x1 - cx).abs());
    let dy = (join.y0 - cy).abs().max((join.y1 - cy).abs());
    let r = dx.hypot(dy);
    RectF {
        x0: cx - r,
        y0: cy - r,
        x1: cx + r,
        y1: cy + r,
    }
}

/// How the parent surface *P* maps the root's unrotated space onto its canvas (step 3).
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct ParentCanvas {
    /// *P*'s displayed box, absolute.
    pub displayed: Rect,
    /// *P*'s full canvas size (its layout box in canvas pixels, also when the canvas is
    /// cropped): the displayed box divided by this gives the composition scale.
    pub canvas: [u32; 2],
    /// *P*'s canvas origin offset (§7; 0 unless the canvas is cropped).
    pub canvas_origin: [i32; 2],
    /// The size of the stored part of *P*'s canvas (§7; equal to `canvas` unless the
    /// canvas is cropped): the region is clipped to it.
    pub stored: [u32; 2],
}

/// The coverage region of nested entry `n` in its parent's canvas (steps 1–3), or `None`
/// when it is empty after the clip to the stored part of the canvas.
#[must_use]
pub fn coverage_region(n: &FootprintNode, parent: &ParentCanvas) -> Option<Rect> {
    let f = footprint(n);
    let p = &parent.displayed;
    let (cw, ch) = (
        f64::from(parent.canvas[0].max(1)),
        f64::from(parent.canvas[1].max(1)),
    );
    let sx = f64::from(p.width()) / cw;
    let sy = f64::from(p.height()) / ch;
    if sx <= 0.0 || sy <= 0.0 {
        return None;
    }
    let (ox, oy) = (f64::from(p.x0), f64::from(p.y0));
    let (cx, cy) = (
        f64::from(parent.canvas_origin[0]),
        f64::from(parent.canvas_origin[1]),
    );
    let local = RectF {
        x0: (f.x0 - ox) / sx - cx,
        y0: (f.y0 - oy) / sy - cy,
        x1: (f.x1 - ox) / sx - cx,
        y1: (f.y1 - oy) / sy - cy,
    };
    let r = local
        .round_out()
        .intersect(&Rect::of_size(parent.stored[0], parent.stored[1]));
    (!r.is_empty()).then_some(r)
}

/// The damage that a change of a nested entry's coverage region causes in its parent's
/// canvas (Decision 1.4): the old and the new region, or nothing when they are equal (so
/// turning an entry that already holds a rotation is not damage).
#[must_use]
pub fn coverage_change_damage(old: Option<Rect>, new: Option<Rect>) -> Vec<Rect> {
    if old == new {
        return Vec::new();
    }
    old.into_iter().chain(new).collect()
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn marker_square_is_40() {
        // decision 0015 Decision 1.7: the 28 × 28 marker gets a 40 × 40 square.
        let n = FootprintNode {
            displayed: Rect::from_xywh(100, 50, 28, 28),
            own_rotation: true,
            ..FootprintNode::default()
        };
        let parent = ParentCanvas {
            displayed: Rect::from_xywh(0, 0, 300, 300),
            canvas: [300, 300],
            canvas_origin: [0, 0],
            stored: [300, 300],
        };
        let r = coverage_region(&n, &parent).unwrap();
        assert_eq!((r.width(), r.height()), (40, 40));
        assert_eq!(r, Rect::new(94, 44, 134, 84));
    }

    #[test]
    fn scaled_parent_divides_out() {
        let n = FootprintNode {
            displayed: Rect::from_xywh(15, 15, 30, 30),
            ..FootprintNode::default()
        };
        let parent = ParentCanvas {
            displayed: Rect::from_xywh(0, 0, 150, 150),
            canvas: [100, 100],
            canvas_origin: [0, 0],
            stored: [100, 100],
        };
        assert_eq!(
            coverage_region(&n, &parent),
            Some(Rect::new(10, 10, 30, 30))
        );
    }

    #[test]
    fn cropped_parent_clips_to_the_stored_part() {
        // A 20,000-wide layout box cropped to the 3,840 pixels on the screen (§7).
        let parent = ParentCanvas {
            displayed: Rect::from_xywh(-1000, 0, 20_000, 100),
            canvas: [20_000, 100],
            canvas_origin: [1000, 0],
            stored: [3840, 100],
        };
        let at = |x| FootprintNode {
            displayed: Rect::from_xywh(x, 10, 100, 10),
            ..FootprintNode::default()
        };
        assert_eq!(
            coverage_region(&at(1500), &parent),
            Some(Rect::new(1500, 10, 1600, 20))
        );
        assert_eq!(
            coverage_region(&at(2800), &parent),
            Some(Rect::new(2800, 10, 2900, 20))
        );
        // Past the stored part: no region.
        assert_eq!(coverage_region(&at(5000), &parent), None);
        // Straddling its right edge: clipped to it.
        assert_eq!(
            coverage_region(&at(3800), &parent),
            Some(Rect::new(3800, 10, 3840, 20))
        );
    }

    #[test]
    fn rotation_detection() {
        assert!(!holds_rotation([0.0, 0.0, 0.0001]));
        assert!(!holds_rotation([f32::NAN, f32::INFINITY, 0.0]));
        assert!(holds_rotation([0.0, 0.0, 90.0]));
    }

    #[test]
    fn unchanged_region_is_no_damage() {
        let r = Some(Rect::new(0, 0, 4, 4));
        assert!(coverage_change_damage(r, r).is_empty());
        assert_eq!(coverage_change_damage(r, None).len(), 1);
    }
}
