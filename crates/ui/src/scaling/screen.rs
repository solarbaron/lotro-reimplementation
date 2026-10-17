// SPDX-License-Identifier: GPL-3.0-or-later
//! The UI scale, the UI screen *F* and the pixel-centre mapping (`ui-scaling.md` §3.2, §3.6,
//! §4.2, §4.3).
//!
//! All arithmetic is in 64-bit floats, as the spec requires. The entry points guard against
//! non-finite and zero or negative input: a zero-sided window has no screen (§3.2), so
//! [`UiScreen::new`] returns `None` and the caller keeps the last good screen.

/// The largest window dimension the mapping accepts, as a guard against hostile sizes
/// (`ui-scaling.md` §9.6 fuzz bound: 1–16,384). Larger values are clamped to it.
pub const MAX_WINDOW_PX: i32 = 16_384;

/// The lower and upper bounds of the user scale *S* (`ui-scaling.md` §4.1).
pub const SCALE_MIN: f64 = 0.50;
/// The upper bound of the user scale *S* (`ui-scaling.md` §4.1).
pub const SCALE_MAX: f64 = 4.00;
/// The step of the user scale (`ui-scaling.md` §4.1): 0.05.
pub const SCALE_STEP: f64 = 0.05;

/// The design floor width (`ui-scaling.md` §4.3): the UI screen is never narrower than this.
pub const FLOOR_W: f64 = 1024.0;
/// The design floor height (`ui-scaling.md` §4.3): the UI screen is never shorter than this.
pub const FLOOR_H: f64 = 768.0;

/// Rounds a stored or typed user scale to the nearest step and clamps it into range
/// (`ui-scaling.md` §4.1): steps of 0.05 from 0.50 to 4.00, a value exactly halfway rounds up.
/// A non-finite value becomes 1.0.
#[must_use]
pub fn round_scale(s: f64) -> f64 {
    if !s.is_finite() {
        return 1.0;
    }
    let steps = (s / SCALE_STEP + 0.5).floor();
    (steps * SCALE_STEP).clamp(SCALE_MIN, SCALE_MAX)
}

/// The automatic default *S*auto (`ui-scaling.md` §4.2):
///
/// the largest multiple of 0.05 not above `max(1.0, σ, Hp ÷ 1440)`, clamped to 0.50–4.00,
/// where `σ` is the operating system's scale factor for the window's monitor (1.0 when not
/// finite or not positive) and `hp` is the window's inner height in physical pixels. "Not
/// above" uses a tolerance of 10⁻⁶ so that 1.5 stays 1.5.
#[must_use]
pub fn s_auto(sigma: f64, hp: i32) -> f64 {
    let sigma = if sigma.is_finite() && sigma > 0.0 {
        sigma
    } else {
        1.0
    };
    let hp = f64::from(hp.clamp(1, MAX_WINDOW_PX));
    let target = 1.0_f64.max(sigma).max(hp / 1440.0);
    let steps = ((target + 1e-6) / SCALE_STEP).floor();
    (steps * SCALE_STEP).clamp(SCALE_MIN, SCALE_MAX)
}

/// The effective scale *Se* (`ui-scaling.md` §4.3):
///
/// `Se = min(S, Wp ÷ 1024, Hp ÷ 768)`, so the UI screen is never smaller than 1024 × 768 UI
/// units (the design floor). `s` is the resolved user scale (a number, or [`s_auto`] when the
/// user chose auto). The window sides are clamped to 1…[`MAX_WINDOW_PX`]; a zero side is
/// treated as 1 here (the caller skips the frame entirely, §3.2). The result is always `> 0`
/// and never above `s`.
#[must_use]
pub fn effective_scale(s: f64, wp: i32, hp: i32) -> f64 {
    let s = if s.is_finite() && s > 0.0 { s } else { 1.0 };
    let wp = f64::from(wp.clamp(1, MAX_WINDOW_PX));
    let hp = f64::from(hp.clamp(1, MAX_WINDOW_PX));
    s.min(wp / FLOOR_W).min(hp / FLOOR_H)
}

/// The physical column (or row) for a UI coordinate `v` under the axis factor `k`
/// (`ui-scaling.md` §3.6 "Edge → physical"): the smallest integer `X` with
/// `(X + ½) ÷ k ≥ v`. In closed form `⌈v × k − ½⌉`, corrected by one so both sides use the
/// same arithmetic. Used for the screen (`k` = `Kx`, `Ky`) and for canvas sizing
/// (`k` = `Se`, §6.1). A non-finite or non-positive `k`, or a non-finite `v`, gives 0.
#[must_use]
pub fn edge_map(k: f64, v: f64) -> i32 {
    if !k.is_finite() || k <= 0.0 || !v.is_finite() {
        return 0;
    }
    // Closed form, then correct up and down so the inequality holds exactly.
    let mut x = (v * k - 0.5).ceil();
    // The product cannot overflow i64 for the bounded inputs we accept; keep it sane anyway.
    let lo = -(i64::from(MAX_WINDOW_PX) * 4) as f64;
    let hi = (i64::from(MAX_WINDOW_PX) * 4) as f64;
    x = x.clamp(lo, hi);
    while (x + 0.5) / k < v && x < hi {
        x += 1.0;
    }
    while x > lo && (x - 0.5) / k >= v {
        x -= 1.0;
    }
    x as i32
}

/// The UI screen *F* and the axis factors (`ui-scaling.md` §3.2), with the pixel-centre
/// mapping between UI units and physical pixels (§3.6).
///
/// Build one with [`UiScreen::new`] from the window's inner size in physical pixels and the
/// effective scale [`effective_scale`]. `Wu` × `Hu` is the screen in UI units, at least
/// 1024 × 768. The axis factors `Kx`, `Ky` are physical pixels per UI unit; they differ from
/// `Se` by a small amount so that *F* covers the window exactly, with no strip at the right or
/// bottom edge.
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct UiScreen {
    wp: i32,
    hp: i32,
    se: f64,
    wu: i32,
    hu: i32,
    kx: f64,
    ky: f64,
}

impl UiScreen {
    /// Builds the UI screen from the window's inner physical size and the effective scale
    /// (`ui-scaling.md` §3.2). Returns `None` when a side is zero (a minimised window has no
    /// screen, §3.2) or when `se` is not a positive finite number; the caller keeps its last
    /// good screen in that case. Window sides are clamped to [`MAX_WINDOW_PX`].
    #[must_use]
    pub fn new(wp: i32, hp: i32, se: f64) -> Option<Self> {
        if wp < 1 || hp < 1 || !se.is_finite() || se <= 0.0 {
            return None;
        }
        let wp = wp.min(MAX_WINDOW_PX);
        let hp = hp.min(MAX_WINDOW_PX);
        // round half up, in f64.
        let wu = (f64::from(wp) / se + 0.5).floor().max(FLOOR_W) as i32;
        let hu = (f64::from(hp) / se + 0.5).floor().max(FLOOR_H) as i32;
        let kx = f64::from(wp) / f64::from(wu);
        let ky = f64::from(hp) / f64::from(hu);
        Some(Self {
            wp,
            hp,
            se,
            wu,
            hu,
            kx,
            ky,
        })
    }

    /// The window's inner size in physical pixels, `Wp` × `Hp`.
    #[must_use]
    pub fn physical(&self) -> (i32, i32) {
        (self.wp, self.hp)
    }

    /// The effective scale `Se`.
    #[must_use]
    pub fn effective_scale(&self) -> f64 {
        self.se
    }

    /// The UI screen size `Wu` × `Hu`, in UI units (at least 1024 × 768).
    #[must_use]
    pub fn ui_size(&self) -> (i32, i32) {
        (self.wu, self.hu)
    }

    /// The UI screen width `Wu`.
    #[must_use]
    pub fn wu(&self) -> i32 {
        self.wu
    }

    /// The UI screen height `Hu`.
    #[must_use]
    pub fn hu(&self) -> i32 {
        self.hu
    }

    /// The horizontal axis factor `Kx` (physical pixels per UI unit).
    #[must_use]
    pub fn kx(&self) -> f64 {
        self.kx
    }

    /// The vertical axis factor `Ky`.
    #[must_use]
    pub fn ky(&self) -> f64 {
        self.ky
    }

    /// The physical column for a UI x coordinate (`ui-scaling.md` §3.6): `Ex(l)`.
    #[must_use]
    pub fn edge_x(&self, l: f64) -> i32 {
        edge_map(self.kx, l)
    }

    /// The physical row for a UI y coordinate: `Ey(t)`.
    #[must_use]
    pub fn edge_y(&self, t: f64) -> i32 {
        edge_map(self.ky, t)
    }

    /// The UI x coordinate of the centre of physical column `x` (`ui-scaling.md` §3.6
    /// "Pointer → UI units"): `(x + ½) ÷ Kx`.
    #[must_use]
    pub fn pointer_x(&self, x: i32) -> f64 {
        (f64::from(x) + 0.5) / self.kx
    }

    /// The UI y coordinate of the centre of physical row `y`: `(y + ½) ÷ Ky`.
    #[must_use]
    pub fn pointer_y(&self, y: i32) -> f64 {
        (f64::from(y) + 0.5) / self.ky
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::scaling::dbox::DBox;

    /// The §4.2 table (invariant I10).
    #[test]
    fn s_auto_table() {
        assert_eq!(s_auto(1.0, 720), 1.00);
        assert_eq!(s_auto(1.0, 1080), 1.00);
        assert_eq!(s_auto(1.25, 1080), 1.25);
        assert!((s_auto(1.333, 1080) - 1.30).abs() < 1e-9);
        assert_eq!(s_auto(1.0, 1440), 1.00);
        assert_eq!(s_auto(1.0, 2160), 1.50);
        assert_eq!(s_auto(2.0, 2160), 2.00);
        assert_eq!(s_auto(2.0, 2880), 2.00);
        assert_eq!(s_auto(1.0, 4320), 3.00);
        assert_eq!(s_auto(1.5, 900), 1.50);
        // Non-finite sigma falls back to 1.0.
        assert_eq!(s_auto(f64::NAN, 720), 1.00);
    }

    /// The §4.3 effective-scale column of the §9.2 configuration table.
    #[test]
    fn effective_scale_table() {
        assert!((effective_scale(1.0, 1280, 720) - 0.9375).abs() < 1e-12);
        assert_eq!(effective_scale(1.0, 1920, 1080), 1.0);
        assert_eq!(effective_scale(1.25, 1920, 1080), 1.25);
        assert_eq!(effective_scale(1.0, 2560, 1440), 1.0);
        assert_eq!(effective_scale(1.5, 3840, 2160), 1.5);
        assert_eq!(effective_scale(2.0, 3840, 2160), 2.0);
        assert_eq!(effective_scale(3.0, 7680, 4320), 3.0);
        assert!((effective_scale(1.5, 1600, 900) - 1.171875).abs() < 1e-12);
    }

    /// The §9.2 configuration table: `Wu` × `Hu` and the axis factors.
    #[test]
    fn ui_screen_table() {
        let cases = [
            (1280, 720, 0.9375, 1365, 768),
            (1920, 1080, 1.0, 1920, 1080),
            (1920, 1080, 1.25, 1536, 864),
            (2560, 1440, 1.0, 2560, 1440),
            (3840, 2160, 1.5, 2560, 1440),
            (3840, 2160, 2.0, 1920, 1080),
            (3440, 1440, 1.0, 3440, 1440),
            (5120, 1440, 1.0, 5120, 1440),
            (1600, 900, 1.171875, 1365, 768),
        ];
        for (wp, hp, se, wu, hu) in cases {
            let f = UiScreen::new(wp, hp, se).unwrap();
            assert_eq!(f.ui_size(), (wu, hu), "F for {wp}x{hp}@{se}");
            // F covers the window exactly, no strip.
            assert!((f.kx() * f64::from(wu) - f64::from(wp)).abs() < 1e-9);
            assert!((f.ky() * f64::from(hu) - f64::from(hp)).abs() < 1e-9);
        }
    }

    /// Floor: a window smaller than 1024 × 768 keeps a screen of at least the floor
    /// (`ui-scaling.md` §4.3). A 1 × 1 window gets `Se` = 1/1024 (the min of the floor terms),
    /// so the screen is 1024 × 1024, never below 1024 × 768.
    #[test]
    fn design_floor() {
        let se = effective_scale(1.0, 1, 1);
        assert!((se - 1.0 / 1024.0).abs() < 1e-18);
        let f = UiScreen::new(1, 1, se).unwrap();
        let (wu, hu) = f.ui_size();
        assert!(wu >= 1024 && hu >= 768, "F = {wu}x{hu}");
    }

    /// A zero side has no screen (§3.2).
    #[test]
    fn zero_side_no_screen() {
        assert!(UiScreen::new(0, 720, 1.0).is_none());
        assert!(UiScreen::new(1280, 0, 1.0).is_none());
        assert!(UiScreen::new(1280, 720, 0.0).is_none());
        assert!(UiScreen::new(1280, 720, f64::NAN).is_none());
    }

    /// The pixel-centre worked example of §10.5 at C1.
    #[test]
    fn pixel_centre_c1() {
        let f = UiScreen::new(1280, 720, 0.9375).unwrap();
        assert!((f.kx() - 1280.0 / 1365.0).abs() < 1e-12);
        assert_eq!(f.edge_x(609.97), 572);
        assert!(f.pointer_x(571) < 609.97);
        assert!(f.pointer_x(572) >= 609.97);
    }

    /// Invariant I3/I4 property: a pixel lies in an element's physical box exactly when the
    /// pointer at its centre hits the displayed box; abutting edges stay abutting.
    #[test]
    fn hit_and_draw_agree() {
        for &k in &[0.937_729_f64, 1.0, 1.5, 2.25, 3.0] {
            let f = UiScreen::new((1365.0 * k) as i32, (768.0 * k) as i32, k).unwrap();
            let b = DBox::new(100.3, 50.0, 200.7, 80.0);
            let px = f.edge_x(b.l);
            let pr = f.edge_x(b.right());
            for x in (px - 2)..(pr + 2) {
                let inside_box = b.l <= f.pointer_x(x) && f.pointer_x(x) < b.right();
                let inside_phys = px <= x && x < pr;
                assert_eq!(inside_box, inside_phys, "k={k} x={x}");
            }
            // Abutting siblings: the shared edge maps to one column.
            let shared = b.right();
            assert_eq!(f.edge_x(shared), pr);
        }
    }
}
