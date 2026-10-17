// SPDX-License-Identifier: GPL-3.0-or-later
//! The safe area *A* for wide screens (`ui-scaling.md` §5.1).

use super::dbox::DBox;
use super::screen::UiScreen;

/// The lower bound of a custom safe-area aspect ratio (`ui-scaling.md` §5.1, §7.1): 4 ÷ 3.
pub const ASPECT_MIN: f64 = 4.0 / 3.0;
/// The upper bound of a custom safe-area aspect ratio.
pub const ASPECT_MAX: f64 = 4.0;

/// The safe area *A* (`ui-scaling.md` §5.1): the full height of *F* and a centred width
/// `Aw = min(Wu, round(Hu × a))` at `Ax = ⌊(Wu − Aw) ÷ 2⌋`, where `a` is the safe aspect
/// ratio. `aspect` is `None` for the `"full"` setting (`A` = `F`) or a ratio otherwise; a
/// ratio is clamped to 4 ÷ 3 … 4.0 and a non-finite ratio is treated as `"full"`.
///
/// Because `Hu ≥ 768` and `a ≥ 4 ÷ 3`, `A` is never narrower than 1024 UI units; on screens no
/// wider than `a` the safe area is the whole screen.
#[must_use]
pub fn safe_area(screen: &UiScreen, aspect: Option<f64>) -> DBox {
    let (wu, hu) = screen.ui_size();
    let aw = match aspect {
        Some(a) if a.is_finite() => {
            let a = a.clamp(ASPECT_MIN, ASPECT_MAX);
            let want = (f64::from(hu) * a + 0.5).floor() as i32;
            want.min(wu).max(1)
        }
        _ => wu,
    };
    let ax = (wu - aw) / 2; // ⌊…⌋ for a non-negative numerator (aw ≤ wu).
    DBox::new(f64::from(ax), 0.0, f64::from(aw), f64::from(hu))
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn full_and_narrow_screens_use_whole_width() {
        // C9: "full" → A = F.
        let f = UiScreen::new(5120, 1440, 1.0).unwrap();
        let a = safe_area(&f, None);
        assert_eq!((a.l, a.w, a.h), (0.0, 5120.0, 1440.0));
        // 16:9 screen with the 21:9 default: whole width (A = F).
        let f = UiScreen::new(1920, 1080, 1.0).unwrap();
        let a = safe_area(&f, Some(2.4));
        assert_eq!(a.w, 1920.0);
    }

    /// C8: 5120 × 1440 at 21:9 → A 3456 wide at 832 (`ui-scaling.md` §10.3, §9.2).
    #[test]
    fn c8_safe_area() {
        let f = UiScreen::new(5120, 1440, 1.0).unwrap();
        let a = safe_area(&f, Some(2.4));
        assert_eq!((a.l, a.t, a.w, a.h), (832.0, 0.0, 3456.0, 1440.0));
    }

    /// Clamping: a ratio below 4:3 and above 4.0, and a non-finite ratio.
    #[test]
    fn aspect_clamped() {
        let f = UiScreen::new(8000, 768, 1.0).unwrap();
        // 1.0 clamps up to 4/3 → Aw = round(768 * 4/3) = 1024.
        let a = safe_area(&f, Some(1.0));
        assert_eq!(a.w, 1024.0);
        // 10.0 clamps down to 4.0 → Aw = round(768 * 4) = 3072.
        let a = safe_area(&f, Some(10.0));
        assert_eq!(a.w, 3072.0);
        // NaN → full.
        let a = safe_area(&f, Some(f64::NAN));
        assert_eq!(a.w, 8000.0);
    }

    /// A is never narrower than 1024 UI units (`ui-scaling.md` §5.1).
    #[test]
    fn never_below_1024() {
        for &(wp, hp, se) in &[(1280, 720, 0.9375), (5120, 1440, 1.0), (3840, 2160, 2.0)] {
            let f = UiScreen::new(wp, hp, se).unwrap();
            let a = safe_area(&f, Some(2.4));
            assert!(a.w >= 1024.0, "A width {} < 1024", a.w);
        }
    }
}
