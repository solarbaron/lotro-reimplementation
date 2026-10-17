// SPDX-License-Identifier: GPL-3.0-or-later
//! Keeping windows on screen and anchored saved placements (`ui-scaling.md` §5.8).

use super::dbox::DBox;

/// Keep a root panel inside the screen (`ui-scaling.md` §5.8 "Keep-on-screen"): for a
/// displayed box `(x, y, w, h)` (after §5.6, so `w ≤ Wu` and `h ≤ Hu`),
/// `x = min(max(x, 0), Wu − w)` and `y = min(max(y, 0), Hu − h)`. Returns the clamped
/// top-left; the panel is moved by the difference, never resized. The clamp uses `F`, not the
/// safe area, so the user may drag panels into the wings of a wide screen.
#[must_use]
pub fn keep_on_screen(x: f64, y: f64, w: f64, h: f64, wu: i32, hu: i32) -> (f64, f64) {
    let nx = x.max(0.0).min(f64::from(wu) - w);
    let ny = y.max(0.0).min(f64::from(hu) - h);
    (nx, ny)
}

/// Which reference box a saved placement is anchored to (`ui-scaling.md` §5.8): the safe area
/// `A` for safe-area panels, the full screen `F` for full-screen roots.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum RefBox {
    /// The safe area `A` (a safe-area panel).
    Safe,
    /// The full screen `F` (a full-screen root).
    Screen,
}

/// An anchor on one axis (`ui-scaling.md` §5.8): the near edge, the centre, or the far edge.
/// Horizontally `Start`/`End` are left/right, vertically top/bottom.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Anchor {
    /// Anchored to the reference box's near edge (left or top); the offset is from it.
    Start,
    /// Anchored to the centre; the offset is the distance between the centres.
    Centre,
    /// Anchored to the far edge (right or bottom); the offset is from it (exclusive ends).
    End,
}

/// One saved placement, in UI units (`ui-scaling.md` §5.8). The reference box and the sizes
/// are kept so the placement can be applied at any configuration; `w`/`h` and `shown` are set
/// only for elements with the matching save properties.
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct SavedPlacement {
    /// The reference box the anchors are relative to.
    pub reference: RefBox,
    /// The horizontal anchor.
    pub ax: Anchor,
    /// The horizontal offset from the anchor, in UI units.
    pub dx: f64,
    /// The vertical anchor.
    pub ay: Anchor,
    /// The vertical offset from the anchor.
    pub dy: f64,
    /// The saved layout size (UI units), present only with `UICore_Element_save_size`.
    pub size: Option<(i32, i32)>,
    /// The saved visibility, present only with `UICore_Element_save_visibility`.
    pub shown: Option<bool>,
}

/// The anchor and offset for one axis when saving (`ui-scaling.md` §5.8 "Saving"): the anchor
/// is chosen by which third of the reference box the displayed box's centre lies in.
/// `box_lo`/`box_ext` is the box's near edge and extent, `ref_lo`/`ref_ext` the reference
/// box's. Exclusive ends throughout.
#[must_use]
pub fn anchor_for(box_lo: f64, box_ext: f64, ref_lo: f64, ref_ext: f64) -> (Anchor, f64) {
    let centre = box_lo + box_ext / 2.0;
    let third = ref_ext / 3.0;
    if centre < ref_lo + third {
        (Anchor::Start, box_lo - ref_lo)
    } else if centre >= ref_lo + 2.0 * third {
        // R right − box right (exclusive ends).
        (Anchor::End, (ref_lo + ref_ext) - (box_lo + box_ext))
    } else {
        (Anchor::Centre, centre - (ref_lo + ref_ext / 2.0))
    }
}

/// The near edge of a box placed back from its anchor and offset (`ui-scaling.md` §5.8
/// "Applying"). `ext` is the box's current displayed extent on that axis, `ref_lo`/`ref_ext`
/// the reference box.
#[must_use]
pub fn apply_anchor(anchor: Anchor, offset: f64, ref_lo: f64, ref_ext: f64, ext: f64) -> f64 {
    match anchor {
        Anchor::Start => ref_lo + offset,
        Anchor::End => (ref_lo + ref_ext) - offset - ext,
        Anchor::Centre => (ref_lo + ref_ext / 2.0) + offset - ext / 2.0,
    }
}

impl SavedPlacement {
    /// Builds a saved placement from a displayed box and its reference box
    /// (`ui-scaling.md` §5.8 "Saving"). `size` and `shown` are passed through from the
    /// element's save properties.
    #[must_use]
    pub fn from_box(
        reference: RefBox,
        r: &DBox,
        box_: &DBox,
        size: Option<(i32, i32)>,
        shown: Option<bool>,
    ) -> Self {
        let (ax, dx) = anchor_for(box_.l, box_.w, r.l, r.w);
        let (ay, dy) = anchor_for(box_.t, box_.h, r.t, r.h);
        Self {
            reference,
            ax,
            dx,
            ay,
            dy,
            size,
            shown,
        }
    }

    /// The displayed top-left to place a box of displayed size `w` × `h` at, under the
    /// reference box `r` (`ui-scaling.md` §5.8 "Applying"). Keep-on-screen runs afterwards.
    #[must_use]
    pub fn position(&self, r: &DBox, w: f64, h: f64) -> (f64, f64) {
        (
            apply_anchor(self.ax, self.dx, r.l, r.w, w),
            apply_anchor(self.ay, self.dy, r.t, r.h, h),
        )
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::scaling::safe_area::safe_area;
    use crate::scaling::screen::UiScreen;

    /// §10.4 saved placement across a scale change (C2 → C3).
    #[test]
    fn saved_across_scale_change() {
        // Saved at C2: F = A = 1920 × 1080, box (1500, 40, 400 × 500).
        let r_c2 = DBox::new(0.0, 0.0, 1920.0, 1080.0);
        let box_c2 = DBox::new(1500.0, 40.0, 400.0, 500.0);
        let sp = SavedPlacement::from_box(RefBox::Screen, &r_c2, &box_c2, Some((400, 500)), None);
        assert_eq!(sp.ax, Anchor::End);
        assert!((sp.dx - 20.0).abs() < 1e-9);
        assert_eq!(sp.ay, Anchor::Start);
        assert!((sp.dy - 40.0).abs() < 1e-9);
        // Applied at C3: F = A = 1536 × 864, size 400 × 500.
        let r_c3 = DBox::new(0.0, 0.0, 1536.0, 864.0);
        let (x, y) = sp.position(&r_c3, 400.0, 500.0);
        assert!((x - 1116.0).abs() < 1e-9, "x {x}");
        assert!((y - 40.0).abs() < 1e-9, "y {y}");
        // Still 20 units from the right edge.
        assert!(((r_c3.right() - (x + 400.0)) - 20.0).abs() < 1e-9);
    }

    /// Applied at the configuration where it was saved, the placement restores the box
    /// (invariant I8).
    #[test]
    fn round_trips_at_same_config() {
        let r = DBox::new(0.0, 0.0, 1920.0, 1080.0);
        for b in [
            DBox::new(10.0, 10.0, 200.0, 200.0),
            DBox::new(900.0, 500.0, 120.0, 90.0),
            DBox::new(1600.0, 900.0, 300.0, 170.0),
        ] {
            let sp = SavedPlacement::from_box(RefBox::Screen, &r, &b, None, None);
            let (x, y) = sp.position(&r, b.w, b.h);
            assert!((x - b.l).abs() < 0.01, "x {x} vs {}", b.l);
            assert!((y - b.t).abs() < 0.01, "y {y} vs {}", b.t);
        }
    }

    /// §10.3 keep-on-screen: the minimap at C9 (full width) is pushed back inside F.
    #[test]
    fn keep_on_screen_minimap_c9() {
        // Minimap 224 wide, left would be 4918 → right edge 5142 beyond F = 5120.
        let (x, _y) = keep_on_screen(4918.0, 12.0, 224.0, 280.0, 5120, 1440);
        // Wu − w = 5120 − 224 = 4896.
        assert!((x - 4896.0).abs() < 1e-9, "x {x}");
    }

    #[test]
    fn keep_on_screen_no_move_when_inside() {
        let (x, y) = keep_on_screen(100.0, 100.0, 200.0, 200.0, 1920, 1080);
        assert_eq!((x, y), (100.0, 100.0));
    }

    /// A safe-anchored panel keeps its distance from the safe area's edge on a wide screen.
    #[test]
    fn safe_anchor_follows_safe_area() {
        let f = UiScreen::new(1920, 1080, 1.0).unwrap();
        let a = safe_area(&f, Some(2.4)); // A = F here
        let b = DBox::new(20.0, 20.0, 300.0, 200.0);
        let sp = SavedPlacement::from_box(RefBox::Safe, &a, &b, None, None);
        assert_eq!(sp.ax, Anchor::Start);
        // On a 32:9 screen the safe area moves right; the panel stays at dx from its left.
        let f2 = UiScreen::new(5120, 1440, 1.0).unwrap();
        let a2 = safe_area(&f2, Some(2.4)); // A2 left = 832
        let (x, _) = sp.position(&a2, 300.0, 200.0);
        assert!((x - (832.0 + 20.0)).abs() < 1e-9, "x {x}");
    }
}
