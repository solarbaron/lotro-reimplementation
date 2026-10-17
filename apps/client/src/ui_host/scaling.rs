// SPDX-License-Identifier: GPL-3.0-or-later
//! The client's UI-scaling state (`ui-scaling.md` §3, §4, §5.1).
//!
//! This is the client half of decision 0040 that the UI host owns: it resolves the user
//! scale *S* (a number, or **auto**) to the effective scale *Se*, builds the UI screen *F*
//! and the safe area *A* from the window's physical size, the operating system's scale
//! factor and the settings, and converts between physical pixels and the one UI-unit space
//! (§3.6). The host asks it, each frame, for the UI-unit screen size to give `tessera-ui`
//! (§3.2: the screen root has the size of *F*) and for the pointer in UI units (§3.6), and
//! calls into it to apply a scale change live (§4.5) and to react to window-size and OS-scale
//! changes (§4.6).
//!
//! All the geometry is the pure arithmetic of [`tessera_ui::scaling`]; this type adds the
//! client state around it: the resolved scale, the last good screen (kept across a window
//! with a zero side, §3.2), the automatic default's 250 ms stable-size timer (§4.2) and the
//! 150 ms settle timer that debounces the settings write (§4.5 steps 4–5). It never panics:
//! every window size is clamped by [`tessera_ui::scaling`], and a zero-sided window leaves
//! every value untouched.

use std::time::{Duration, Instant};

use tessera_settings::{SafeAspect, UiScale};
use tessera_ui::scaling::{self, DBox, UiScreen};

/// How long the window size must be stable before the automatic default *S*auto is
/// re-evaluated (`ui-scaling.md` §4.2): 250 ms. Never re-evaluated during a live resize drag.
pub const AUTO_STABLE: Duration = Duration::from_millis(250);

/// How long *Se* must be unchanged before a scale change settles and the settings file is
/// written (`ui-scaling.md` §4.5 steps 4–5): 150 ms.
pub const SETTLE: Duration = Duration::from_millis(150);

/// What a frame tick or a change asks the host to do next.
#[derive(Debug, Clone, Copy, Default, PartialEq, Eq)]
pub struct Applied {
    /// *F* (the UI-unit screen size) changed, so the host re-sizes the `tessera-ui` screen
    /// root and lays the roots out again (`ui-scaling.md` §4.5 step 2).
    pub screen_changed: bool,
}

impl Applied {
    const NONE: Self = Self {
        screen_changed: false,
    };
    const SCREEN: Self = Self {
        screen_changed: true,
    };
}

/// The UI-scaling state (module docs).
#[derive(Debug, Clone)]
pub struct ScaleState {
    /// The user scale *S* (a number, or auto).
    user: UiScale,
    /// The safe-area width (`ui_safe_aspect`).
    safe: SafeAspect,
    /// The operating system's scale factor σ for the window's monitor.
    sigma: f64,
    /// The window's last inner size in physical pixels (either side may be 0 while minimised).
    phys: (i32, i32),
    /// The last good UI screen *F*: built only from a size with both sides ≥ 1, so it keeps
    /// its value while the window has a zero side (`ui-scaling.md` §3.2).
    screen: UiScreen,
    /// The safe area *A*, in UI units relative to *F* (`ui-scaling.md` §5.1).
    safe_box: DBox,
    /// The window height *S*auto was last evaluated at (`ui-scaling.md` §4.2): updated only
    /// when the size has been stable for [`AUTO_STABLE`], so a live drag does not flip the
    /// automatic default mid-drag. *Se* still follows the live size through the design floor.
    auto_height: i32,
    /// When the window size next counts as stable for *S*auto; `None` when it already is.
    stable_at: Option<Instant>,
    /// When the current scale change settles; `None` when nothing is pending.
    settle_at: Option<Instant>,
    /// *S* or the safe aspect changed since the last persist and waits for the settle.
    dirty: bool,
}

impl ScaleState {
    /// A state for a window of `phys` physical pixels at OS scale factor `sigma`, with the
    /// user scale and safe aspect from the settings. A non-positive `sigma` is treated as 1.0
    /// (`ui-scaling.md` §4.2). A zero-sided `phys` still yields a valid floor screen so the
    /// host always has one; the next real size replaces it.
    #[must_use]
    pub fn new(user: UiScale, safe: SafeAspect, phys: (i32, i32), sigma: f64) -> Self {
        let sigma = if sigma.is_finite() && sigma > 0.0 {
            sigma
        } else {
            1.0
        };
        let auto_height = phys.1.max(1);
        let mut st = Self {
            user,
            safe,
            sigma,
            phys,
            // Replaced right below; a placeholder floor screen so the field is always valid.
            screen: UiScreen::new(1, 1, scaling::effective_scale(1.0, 1, 1))
                .expect("floor screen is valid"),
            safe_box: DBox::new(0.0, 0.0, 1024.0, 768.0),
            auto_height,
            stable_at: None,
            settle_at: None,
            dirty: false,
        };
        st.rebuild();
        st
    }

    /// The resolved user scale *S* (`ui-scaling.md` §4.1, §4.2): the fixed value, or the
    /// automatic default from σ and the stable window height.
    #[must_use]
    pub fn resolved_scale(&self) -> f64 {
        match self.user {
            UiScale::Fixed(f) => f64::from(f),
            UiScale::Auto => scaling::s_auto(self.sigma, self.auto_height),
        }
    }

    /// Rebuilds *Se*, *F* and *A* from the current inputs. Keeps the last good screen when
    /// the window has a zero side (`ui-scaling.md` §3.2). Returns whether *F* changed.
    fn rebuild(&mut self) -> Applied {
        let (wp, hp) = self.phys;
        if wp < 1 || hp < 1 {
            // A window with a zero side changes nothing (§3.2).
            return Applied::NONE;
        }
        let se = scaling::effective_scale(self.resolved_scale(), wp, hp);
        let Some(screen) = UiScreen::new(wp, hp, se) else {
            return Applied::NONE;
        };
        let before = self.screen.ui_size();
        self.screen = screen;
        self.safe_box = scaling::safe_area(&screen, self.safe.ratio().map(f64::from));
        if screen.ui_size() == before {
            Applied::NONE
        } else {
            Applied::SCREEN
        }
    }

    /// Marks a user-made change of *S* or the safe aspect pending for the settle (the write
    /// is debounced, `ui-scaling.md` §4.5 steps 4–5).
    fn note_user_change(&mut self, now: Instant) {
        self.dirty = true;
        self.settle_at = Some(now + SETTLE);
    }

    // ----- window and OS changes (§4.6) ---------------------------------------------------

    /// A window resize to `phys` physical pixels (`ui-scaling.md` §4.6). *Se* and *F* follow
    /// at once; the automatic default waits for the size to be stable ([`AUTO_STABLE`], §4.2)
    /// so a live drag does not flip it. A size with a zero side changes nothing (§3.2).
    pub fn set_physical(&mut self, phys: (i32, i32), now: Instant) -> Applied {
        if phys == self.phys {
            return Applied::NONE;
        }
        self.phys = phys;
        if phys.0 < 1 || phys.1 < 1 {
            // Zero side: nothing runs and the stable timer does not start (§3.2 rule 2).
            return Applied::NONE;
        }
        self.stable_at = Some(now + AUTO_STABLE);
        self.rebuild()
    }

    /// An OS scale-factor change (the window moved to another monitor, `ui-scaling.md` §4.6):
    /// σ changes and, with auto, *S*auto is re-evaluated at once (a monitor change is not a
    /// resize drag).
    pub fn set_os_scale(&mut self, sigma: f64) -> Applied {
        let sigma = if sigma.is_finite() && sigma > 0.0 {
            sigma
        } else {
            1.0
        };
        if (sigma - self.sigma).abs() < f64::EPSILON {
            return Applied::NONE;
        }
        self.sigma = sigma;
        self.rebuild()
    }

    /// The per-frame tick: applies the automatic default once the window size has settled for
    /// [`AUTO_STABLE`] (`ui-scaling.md` §4.2). Call once per frame with the frame time.
    pub fn tick(&mut self, now: Instant) -> Applied {
        let mut out = Applied::NONE;
        if let Some(at) = self.stable_at
            && now >= at
        {
            self.stable_at = None;
            if self.phys.1 >= 1 && self.auto_height != self.phys.1 {
                self.auto_height = self.phys.1;
                if matches!(self.user, UiScale::Auto) {
                    out = self.rebuild();
                }
            }
        }
        out
    }

    // ----- user scale changes (§4.1, §4.5, §7.3) ------------------------------------------

    /// Sets the user scale *S* to a fixed value (rounded to the step and clamped,
    /// `ui-scaling.md` §4.1) and applies it live (§4.5). `now` seeds the settle timer.
    pub fn set_scale(&mut self, value: f32, now: Instant) -> Applied {
        self.set_user(UiScale::Fixed(round_scale_f32(value)), now)
    }

    /// Sets the user scale to **auto** and applies it live (`ui-scaling.md` §4.2).
    pub fn set_auto(&mut self, now: Instant) -> Applied {
        self.set_user(UiScale::Auto, now)
    }

    /// Steps the user scale by `delta` (`ui-scaling.md` §7.3 `+`/`-`): from the currently
    /// effective number (the auto value when auto), rounded to the step and clamped. Leaves
    /// auto for a fixed value.
    pub fn step_scale(&mut self, delta: f32, now: Instant) -> Applied {
        let base = match self.user {
            UiScale::Fixed(f) => f,
            // Step away from the automatic value, as the slider does (§7.2 row 2).
            UiScale::Auto => self.resolved_scale() as f32,
        };
        self.set_scale(base + delta, now)
    }

    /// Sets the user scale (no rounding beyond what the value already carries) and applies it.
    fn set_user(&mut self, user: UiScale, now: Instant) -> Applied {
        if user == self.user {
            return Applied::NONE;
        }
        self.user = user;
        self.note_user_change(now);
        self.rebuild()
    }

    /// Sets the safe-area width (`ui-scaling.md` §5.1, §7.3 `aspect`) and applies it live.
    pub fn set_safe_aspect(&mut self, safe: SafeAspect, now: Instant) -> Applied {
        if safe == self.safe {
            return Applied::NONE;
        }
        self.safe = safe;
        self.note_user_change(now);
        self.rebuild()
    }

    /// Takes the settled user change for the settings writer, once the settle elapsed
    /// (`ui-scaling.md` §4.5 step 5). Returns the current *S* and safe aspect to persist, or
    /// `None` while nothing is pending or the settle has not elapsed.
    pub fn take_persist(&mut self, now: Instant) -> Option<(UiScale, SafeAspect)> {
        if !self.dirty {
            return None;
        }
        match self.settle_at {
            Some(at) if now >= at => {
                self.dirty = false;
                self.settle_at = None;
                Some((self.user, self.safe))
            }
            _ => None,
        }
    }

    // ----- accessors ----------------------------------------------------------------------

    /// The current user scale *S* (a number or auto).
    #[must_use]
    pub fn user_scale(&self) -> UiScale {
        self.user
    }

    /// The current safe-area width.
    #[must_use]
    pub fn safe_aspect(&self) -> SafeAspect {
        self.safe
    }

    /// The UI screen *F* (`ui-scaling.md` §3.2), with the axis factors and the pixel-centre
    /// mapping of §3.6.
    #[must_use]
    pub fn screen(&self) -> &UiScreen {
        &self.screen
    }

    /// The UI-unit screen size `Wu` × `Hu` to give the `tessera-ui` screen root
    /// (`ui-scaling.md` §3.2).
    #[must_use]
    pub fn ui_size(&self) -> (i32, i32) {
        self.screen.ui_size()
    }

    /// The effective scale *Se* (`ui-scaling.md` §4.3).
    #[must_use]
    pub fn effective_scale(&self) -> f64 {
        self.screen.effective_scale()
    }

    /// The safe area *A*, in UI units relative to *F* (`ui-scaling.md` §5.1).
    #[must_use]
    pub fn safe_area(&self) -> DBox {
        self.safe_box
    }

    /// Whether *Se* is below the user's *S* because the window is too small (the design
    /// floor, `ui-scaling.md` §4.3): the Interface window says so (§7.2 row 4).
    #[must_use]
    pub fn is_limited(&self) -> bool {
        self.effective_scale() + 1e-9 < self.resolved_scale()
    }

    /// The scale shown in the Interface window and `/uiscale` replies, as a whole-number
    /// percentage: `round(100 × Se)` when limited, else `round(100 × S)` (`ui-scaling.md`
    /// §7.2 rows 2 and 4).
    #[must_use]
    pub fn display_percent(&self) -> u32 {
        let v = if self.is_limited() {
            self.effective_scale()
        } else {
            self.resolved_scale()
        };
        (v * 100.0 + 0.5).floor().max(0.0) as u32
    }

    /// The automatic default's percentage, for the "Auto (150 %)" label (`ui-scaling.md`
    /// §7.2 row 2).
    #[must_use]
    pub fn auto_percent(&self) -> u32 {
        (scaling::s_auto(self.sigma, self.auto_height) * 100.0 + 0.5).floor() as u32
    }

    // ----- pointer conversion (§3.6) ------------------------------------------------------

    /// A pointer at physical pixel (`xp`, `yp`) in UI units, floored to integers for the UI
    /// and the plugin API (`ui-scaling.md` §3.6 "Pointer → UI units"). At *Se* = 1 with
    /// `Kx` = `Ky` = 1 this is the identity (invariant I2).
    #[must_use]
    pub fn pointer_to_ui(&self, xp: i32, yp: i32) -> (i32, i32) {
        (
            self.screen.pointer_x(xp).floor() as i32,
            self.screen.pointer_y(yp).floor() as i32,
        )
    }

    /// A continuous physical position (`xp`, `yp`) in UI units as real numbers
    /// (`ui-scaling.md` §3.6): the linear map `xp ÷ Kx`, `yp ÷ Ky`, which takes the window's
    /// physical extent to *F* exactly. This is what the UI hit test receives (its
    /// `PointerMoved` is real-valued); the integer [`Self::pointer_to_ui`] is for the plugin
    /// API and the integer comparisons of the input behaviour. At *Se* = 1 (`Kx` = `Ky` = 1)
    /// both are the identity (invariant I2).
    #[must_use]
    pub fn physical_to_ui(&self, xp: f64, yp: f64) -> (f64, f64) {
        (xp / self.screen.kx(), yp / self.screen.ky())
    }
}

/// Rounds a user scale to the step and clamps it to range (`ui-scaling.md` §4.1), in `f32`.
#[must_use]
fn round_scale_f32(v: f32) -> f32 {
    scaling::round_scale(f64::from(v)) as f32
}

#[cfg(test)]
mod tests {
    use super::*;

    fn t0() -> Instant {
        Instant::now()
    }

    /// The §9.2 configuration table: `F` (the UI-unit size) at several windows and scales,
    /// at the default 21:9 safe aspect.
    #[test]
    fn ui_screen_matches_configurations() {
        let cases = [
            (UiScale::Fixed(1.0), (1280, 720), (1365, 768)),
            (UiScale::Fixed(1.0), (1920, 1080), (1920, 1080)),
            (UiScale::Fixed(1.25), (1920, 1080), (1536, 864)),
            (UiScale::Fixed(1.0), (2560, 1440), (2560, 1440)),
            (UiScale::Fixed(1.5), (3840, 2160), (2560, 1440)),
            (UiScale::Fixed(2.0), (3840, 2160), (1920, 1080)),
            (UiScale::Fixed(1.0), (3440, 1440), (3440, 1440)),
            (UiScale::Fixed(1.0), (5120, 1440), (5120, 1440)),
        ];
        for (s, phys, f) in cases {
            let st = ScaleState::new(s, SafeAspect::R21_9, phys, 1.0);
            assert_eq!(st.ui_size(), f, "F for {phys:?} @ {s:?}");
        }
    }

    /// Auto on common windows (`ui-scaling.md` §4.2 table).
    #[test]
    fn auto_default_table() {
        // 1920 × 1080 at σ 1.0 → S 1.0, Se 1.0.
        let st = ScaleState::new(UiScale::Auto, SafeAspect::R21_9, (1920, 1080), 1.0);
        assert!((st.resolved_scale() - 1.0).abs() < 1e-9);
        assert!((st.effective_scale() - 1.0).abs() < 1e-9);
        // 3840 × 2160 at σ 1.0 → S 1.5.
        let st = ScaleState::new(UiScale::Auto, SafeAspect::R21_9, (3840, 2160), 1.0);
        assert!((st.resolved_scale() - 1.5).abs() < 1e-9);
        assert_eq!(st.auto_percent(), 150);
        // A monitor at σ 2.0 lifts a 1920 × 1080 window to at least 2.0.
        let st = ScaleState::new(UiScale::Auto, SafeAspect::R21_9, (1920, 1080), 2.0);
        assert!((st.resolved_scale() - 2.0).abs() < 1e-9);
    }

    /// C8: 5120 × 1440 at 21:9 → A 3456 wide at 832 (`ui-scaling.md` §9.2, §10.3).
    #[test]
    fn safe_area_c8() {
        let st = ScaleState::new(UiScale::Fixed(1.0), SafeAspect::R21_9, (5120, 1440), 1.0);
        let a = st.safe_area();
        assert_eq!((a.l, a.w, a.h), (832.0, 3456.0, 1440.0));
        // "full" uses the whole width.
        let st = ScaleState::new(UiScale::Fixed(1.0), SafeAspect::Full, (5120, 1440), 1.0);
        assert_eq!(st.safe_area().w, 5120.0);
    }

    /// The limit flag and the shown percentage at C1 (`ui-scaling.md` §4.3, §7.2 row 4):
    /// 1280 × 720 at S 1.0 is limited to Se 0.9375 → "94 %".
    #[test]
    fn limited_at_c1() {
        let st = ScaleState::new(UiScale::Fixed(1.0), SafeAspect::R21_9, (1280, 720), 1.0);
        assert!(st.is_limited());
        assert_eq!(st.display_percent(), 94);
        // 1920 × 1080 at S 1.0 is not limited → "100 %".
        let st = ScaleState::new(UiScale::Fixed(1.0), SafeAspect::R21_9, (1920, 1080), 1.0);
        assert!(!st.is_limited());
        assert_eq!(st.display_percent(), 100);
    }

    /// Pointer conversion is the identity at Se = 1 (invariant I2) and maps centres at a
    /// non-unit scale (§10.5 at C1).
    #[test]
    fn pointer_conversion() {
        let st = ScaleState::new(UiScale::Fixed(1.0), SafeAspect::R21_9, (1920, 1080), 1.0);
        assert_eq!(st.pointer_to_ui(0, 0), (0, 0));
        assert_eq!(st.pointer_to_ui(500, 400), (500, 400));
        // C1: 1280 × 720 → F 1365 × 768, Kx ≈ 0.9377. The §10.5 example: UI x 609.97.
        let st = ScaleState::new(UiScale::Fixed(1.0), SafeAspect::R21_9, (1280, 720), 1.0);
        let (ux, _) = st.pointer_to_ui(571, 0);
        let (ux2, _) = st.pointer_to_ui(1279, 0);
        assert!(ux < 610);
        assert!(ux2 >= 1279, "last column still inside F ({ux2})");
    }

    /// A window with a zero side changes nothing (`ui-scaling.md` §3.2).
    #[test]
    fn zero_side_keeps_last() {
        let mut st = ScaleState::new(UiScale::Fixed(1.0), SafeAspect::R21_9, (1920, 1080), 1.0);
        let f = st.ui_size();
        let a = st.set_physical((1920, 0), t0());
        assert_eq!(a, Applied::NONE);
        assert_eq!(st.ui_size(), f, "F kept while minimised");
        assert_eq!(st.safe_area().w, 1920.0);
    }

    /// A live change of *S* resizes *F* and marks a settle for the settings write.
    #[test]
    fn set_scale_applies_and_settles() {
        let now = t0();
        let mut st = ScaleState::new(UiScale::Fixed(1.0), SafeAspect::R21_9, (3840, 2160), 1.0);
        assert_eq!(st.ui_size(), (3840, 2160));
        let a = st.set_scale(2.0, now);
        assert!(a.screen_changed);
        assert_eq!(st.ui_size(), (1920, 1080));
        // Not yet settled.
        assert!(st.take_persist(now).is_none());
        // After the settle: the value to persist is handed over once.
        let out = st.take_persist(now + SETTLE).unwrap();
        assert_eq!(out.0, UiScale::Fixed(2.0));
        assert!(st.take_persist(now + SETTLE * 2).is_none());
    }

    /// A safe-aspect change is pending for the settle and handed back once, like a scale
    /// change (`ui-scaling.md` §4.5 steps 4–5, §7.3 `aspect`).
    #[test]
    fn set_safe_aspect_applies_and_settles() {
        let now = t0();
        let mut st = ScaleState::new(UiScale::Fixed(1.0), SafeAspect::R21_9, (5120, 1440), 1.0);
        let a = st.set_safe_aspect(SafeAspect::R16_9, now);
        // The safe area narrows, but F (the UI screen) is unchanged by the aspect alone.
        assert_eq!(a, Applied::NONE);
        assert_eq!(st.safe_aspect(), SafeAspect::R16_9);
        // Not yet settled.
        assert!(st.take_persist(now).is_none());
        // After the settle the current S and the new aspect are handed over, once.
        let out = st.take_persist(now + SETTLE).expect("settled");
        assert_eq!(out, (UiScale::Fixed(1.0), SafeAspect::R16_9));
        assert!(st.take_persist(now + SETTLE * 2).is_none());
    }

    /// A window resize is not a user change: it never marks a persist, so a resize alone never
    /// rewrites `ui_scale`/`ui_safe_aspect` (`ui-scaling.md` §4.4, §4.6).
    #[test]
    fn resize_does_not_persist() {
        let now = t0();
        let mut st = ScaleState::new(UiScale::Fixed(1.0), SafeAspect::R21_9, (1920, 1080), 1.0);
        let a = st.set_physical((2560, 1440), now);
        assert!(a.screen_changed, "F follows the window");
        assert!(
            st.take_persist(now + SETTLE).is_none(),
            "a resize never queues a settings write"
        );
        // An OS scale change is likewise not a user change.
        let mut st2 = ScaleState::new(UiScale::Fixed(1.0), SafeAspect::R21_9, (1920, 1080), 1.0);
        st2.set_os_scale(2.0);
        assert!(st2.take_persist(now + SETTLE).is_none());
    }

    /// `+`/`-` steps from the effective value, leaving auto for a number (`ui-scaling.md`
    /// §7.3).
    #[test]
    fn step_from_auto_and_number() {
        let now = t0();
        // Auto at 3840 × 2160 is 1.5; +0.05 → fixed 1.55.
        let mut st = ScaleState::new(UiScale::Auto, SafeAspect::R21_9, (3840, 2160), 1.0);
        st.step_scale(0.05, now);
        assert_eq!(st.user_scale(), UiScale::Fixed(1.55));
        // -0.05 from 1.55 → 1.5.
        st.step_scale(-0.05, now);
        assert_eq!(st.user_scale(), UiScale::Fixed(1.5));
    }

    /// The automatic default is held during a live resize and re-evaluated once the size is
    /// stable for [`AUTO_STABLE`] (`ui-scaling.md` §4.2).
    #[test]
    fn auto_waits_for_stable_size() {
        let now = t0();
        // Start at 1080 (auto 1.0), then drag up to 2160 (auto 1.5).
        let mut st = ScaleState::new(UiScale::Auto, SafeAspect::R21_9, (1920, 1080), 1.0);
        assert!((st.resolved_scale() - 1.0).abs() < 1e-9);
        st.set_physical((3840, 2160), now);
        // Se follows the live size at once (floor terms), but S stays 1.0 until stable.
        assert!((st.resolved_scale() - 1.0).abs() < 1e-9);
        // Before the stable window: no change.
        assert_eq!(st.tick(now + AUTO_STABLE / 2), Applied::NONE);
        assert!((st.resolved_scale() - 1.0).abs() < 1e-9);
        // After it: S becomes the new auto value.
        let a = st.tick(now + AUTO_STABLE);
        assert!(a.screen_changed);
        assert!((st.resolved_scale() - 1.5).abs() < 1e-9);
    }

    /// An OS scale change re-evaluates auto at once.
    #[test]
    fn os_scale_change() {
        let mut st = ScaleState::new(UiScale::Auto, SafeAspect::R21_9, (1920, 1080), 1.0);
        assert!((st.resolved_scale() - 1.0).abs() < 1e-9);
        st.set_os_scale(1.5);
        assert!((st.resolved_scale() - 1.5).abs() < 1e-9);
    }
}
