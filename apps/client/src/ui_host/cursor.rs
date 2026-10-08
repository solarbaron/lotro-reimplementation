// SPDX-License-Identifier: GPL-3.0-or-later
//! The cursor and the pointer lock (decision 0015 §2.5, decision 0014 D7).
//!
//! The lock is client state. A `mouselook` Start is offered to the UI first; the lock goes
//! on only when the UI did not consume it and no item drag is running (during a drag the
//! client consumes the Start itself). The lock goes off after the End of the **last** hold
//! that turned it on. While it is on, the client feeds the mapper no cursor-position and
//! no cursor-left events ([`PointerLock::filter`]), so every position stays at the frozen
//! position *F* of the press; at its end the cursor is moved back to *F* where the
//! platform allows, the grab is released and the cursor shown, and a cursor-left that was
//! withheld during the lock is fed right after ([`PointerLock::take_withheld_left`]).
//!
//! Window operations go through [`CursorWindow`], which the windowed client implements
//! over winit and headless runs leave empty ([`NoWindow`]).

use std::collections::BTreeSet;

/// The window operations the lock needs.
pub trait CursorWindow {
    /// Shows or hides the OS cursor over the window.
    fn set_cursor_visible(&mut self, visible: bool);
    /// Grabs the pointer (`Locked`, falling back to `Confined`) or releases it. Returns
    /// whether a grab is now in effect.
    fn set_grab(&mut self, grab: bool) -> bool;
    /// Moves the cursor to a position in physical pixels. Returns whether the platform
    /// allowed it.
    fn warp(&mut self, x: f64, y: f64) -> bool;
}

/// No window (headless runs and tests): every operation does nothing.
#[derive(Debug, Clone, Copy, Default)]
pub struct NoWindow;

impl CursorWindow for NoWindow {
    fn set_cursor_visible(&mut self, _visible: bool) {}
    fn set_grab(&mut self, _grab: bool) -> bool {
        false
    }
    fn warp(&mut self, _x: f64, _y: f64) -> bool {
        false
    }
}

/// What a raw pointer report should do while the lock may be on.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum PointerReport {
    /// Feed it to the mapper.
    Feed,
    /// Withhold it (the lock is on).
    Withhold,
}

/// The lock state (module docs).
#[derive(Debug, Clone, Default)]
pub struct PointerLock {
    holds: BTreeSet<u64>,
    frozen: Option<(f64, f64)>,
    withheld_left: bool,
    software_hidden: bool,
    locks: u64,
}

/// What a `mouselook` Start did.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum StartOutcome {
    /// The UI consumed it: nothing turned on.
    ConsumedByUi,
    /// An item drag is running: the client consumed it (IB D7).
    ConsumedByClient,
    /// The lock is on (it may already have been).
    Locked,
}

impl PointerLock {
    /// No lock.
    #[must_use]
    pub fn new() -> Self {
        Self::default()
    }

    /// Whether the lock is on.
    #[must_use]
    pub fn is_on(&self) -> bool {
        !self.holds.is_empty()
    }

    /// The frozen position *F* while the lock is on.
    #[must_use]
    pub fn frozen(&self) -> Option<(f64, f64)> {
        self.frozen.filter(|_| self.is_on())
    }

    /// How many times the lock turned on (diagnostics, tests).
    #[must_use]
    pub fn lock_count(&self) -> u64 {
        self.locks
    }

    /// A `mouselook` Start of `hold` at `position`, after the UI's verdict `consumed`.
    /// `item_drag` says whether a drag icon exists. Returns the outcome; the caller's
    /// verdict to the mapper is "consumed" unless the outcome is [`StartOutcome::Locked`].
    pub fn start(
        &mut self,
        hold: u64,
        consumed: bool,
        item_drag: bool,
        position: Option<(f64, f64)>,
        window: &mut dyn CursorWindow,
    ) -> StartOutcome {
        if consumed {
            return StartOutcome::ConsumedByUi;
        }
        if item_drag {
            return StartOutcome::ConsumedByClient;
        }
        let first = self.holds.is_empty();
        self.holds.insert(hold);
        if first {
            self.frozen = position;
            self.locks += 1;
            window.set_cursor_visible(false);
            self.software_hidden = true;
            let _ = window.set_grab(true);
        }
        StartOutcome::Locked
    }

    /// The End of `hold`. Returns `true` when this End turned the lock off.
    pub fn end(&mut self, hold: u64, window: &mut dyn CursorWindow, os_cursor: bool) -> bool {
        if !self.holds.remove(&hold) || !self.holds.is_empty() {
            return false;
        }
        if let Some((x, y)) = self.frozen {
            let _ = window.warp(x, y);
        }
        let _ = window.set_grab(false);
        if os_cursor {
            window.set_cursor_visible(true);
        }
        self.software_hidden = false;
        true
    }

    /// Focus loss ends every hold (the mapper's release, IB D7); the Ends arrive through
    /// the offers, so this only clears a lock whose Ends were lost.
    pub fn reset(&mut self, window: &mut dyn CursorWindow, os_cursor: bool) {
        if self.holds.is_empty() {
            return;
        }
        self.holds.clear();
        let _ = window.set_grab(false);
        if os_cursor {
            window.set_cursor_visible(true);
        }
        self.software_hidden = false;
    }

    /// Whether a raw cursor-position or cursor-left report may reach the mapper. A
    /// withheld cursor-left is remembered (the last report wins).
    pub fn filter(&mut self, is_left: bool) -> PointerReport {
        if !self.is_on() {
            return PointerReport::Feed;
        }
        self.withheld_left = is_left;
        PointerReport::Withhold
    }

    /// After the call whose End turned the lock off: whether a withheld cursor-left must be
    /// fed now.
    pub fn take_withheld_left(&mut self) -> bool {
        if self.is_on() {
            return false;
        }
        std::mem::take(&mut self.withheld_left)
    }

    /// A window scale-factor change while the lock may be on (`keymap.md` §7.8.9): the lock
    /// stays on and *F* becomes *F*′ = *F* × `ratio` (`ratio` = *s*new ÷ *s*old, real-valued,
    /// no rounding), clamped into the client area of `area` physical pixels. Returns *F*′
    /// while the lock is on, `None` otherwise (nothing changes then). A non-finite or
    /// non-positive `ratio` keeps *F*.
    pub fn rescale(&mut self, ratio: f64, area: (u32, u32)) -> Option<(f64, f64)> {
        if !self.is_on() {
            return None;
        }
        let (x, y) = self.frozen?;
        let r = if ratio.is_finite() && ratio > 0.0 {
            ratio
        } else {
            1.0
        };
        let max_x = (f64::from(area.0) - 1.0).max(0.0);
        let max_y = (f64::from(area.1) - 1.0).max(0.0);
        let f = ((x * r).clamp(0.0, max_x), (y * r).clamp(0.0, max_y));
        self.frozen = Some(f);
        Some(f)
    }

    /// Whether the software cursor is hidden by the lock (decision 0015 §2.5 item 1).
    #[must_use]
    pub fn software_hidden(&self) -> bool {
        self.software_hidden
    }
}

/// The `[Input]` section of the settings file (defaults when it cannot be read).
#[must_use]
pub fn input_prefs() -> tessera_settings::InputPrefs {
    match tessera_settings::SettingsStore::open_default() {
        Ok(store) => {
            let loaded = store.load();
            let (p, warnings) = tessera_settings::InputPrefs::from_settings(&loaded.settings);
            for w in warnings {
                tracing::warn!("settings: {w}");
            }
            p
        }
        Err(e) => {
            tracing::debug!("settings unavailable ({e}); input defaults");
            tessera_settings::InputPrefs::default()
        }
    }
}

/// *T*, *W* and *H* for the mapper (`keymap.md` §7.7, decision 0014 D3): the settings'
/// `[Input]` values (a key in the file overrides), else the platform's, else 500 ms and a
/// 4 × 4 rectangle; the rectangle, in logical pixels, is scaled by the window's
/// `scale_factor`. The client reads no platform values yet (winit offers none; a plain
/// Wayland compositor publishes none), so `platform` is `None` today.
#[must_use]
pub fn activation_timing(
    prefs: &tessera_settings::InputPrefs,
    platform: Option<(u32, u32)>,
    scale_factor: f64,
) -> tessera_input::ActivationTiming {
    let px = f64::from(prefs.double_click_px(platform.map(|p| p.1)));
    tessera_input::ActivationTiming::from_logical(
        prefs.double_click_ms(platform.map(|p| p.0)),
        px,
        px,
        if scale_factor.is_finite() && scale_factor > 0.0 {
            scale_factor
        } else {
            1.0
        },
    )
}

/// Builds a software-cursor image: a simple arrow of `size` × `size` pixels as tight
/// B, G, R, A rows (Tessera's own image, used when no cursor media is known).
#[must_use]
pub fn arrow_bgra(size: u32) -> Vec<u8> {
    let s = size.clamp(8, 128);
    let mut px = vec![0u8; (s * s * 4) as usize];
    for y in 0..s {
        // A right triangle with its corner at the hot spot (0, 0), cut at 3/4 of the size.
        let w = (y + 1).min(s * 3 / 4);
        if y >= s * 3 / 4 {
            break;
        }
        for x in 0..w {
            let edge = x == 0 || x + 1 == w || y + 1 == s * 3 / 4;
            let i = ((y * s + x) * 4) as usize;
            let v = if edge { 0x10 } else { 0xF0 };
            px[i] = v;
            px[i + 1] = v;
            px[i + 2] = v;
            px[i + 3] = 0xFF;
        }
    }
    px
}

#[cfg(test)]
mod tests {
    use super::*;

    #[derive(Default)]
    struct Recorder {
        visible: Vec<bool>,
        grabs: Vec<bool>,
        warps: Vec<(f64, f64)>,
    }

    impl CursorWindow for Recorder {
        fn set_cursor_visible(&mut self, v: bool) {
            self.visible.push(v);
        }
        fn set_grab(&mut self, g: bool) -> bool {
            self.grabs.push(g);
            g
        }
        fn warp(&mut self, x: f64, y: f64) -> bool {
            self.warps.push((x, y));
            true
        }
    }

    #[test]
    fn lock_follows_the_last_hold() {
        let mut w = Recorder::default();
        let mut l = PointerLock::new();
        assert_eq!(
            l.start(1, true, false, Some((5.0, 6.0)), &mut w),
            StartOutcome::ConsumedByUi
        );
        assert!(!l.is_on());
        assert_eq!(
            l.start(2, false, true, Some((5.0, 6.0)), &mut w),
            StartOutcome::ConsumedByClient
        );
        assert!(!l.is_on());
        assert_eq!(
            l.start(3, false, false, Some((5.0, 6.0)), &mut w),
            StartOutcome::Locked
        );
        // The run chord: a second mouse-look hold while the first is held.
        assert_eq!(
            l.start(4, false, false, Some((9.0, 9.0)), &mut w),
            StartOutcome::Locked
        );
        assert_eq!(l.frozen(), Some((5.0, 6.0)));
        assert_eq!(l.filter(false), PointerReport::Withhold);
        assert_eq!(l.filter(true), PointerReport::Withhold);
        assert!(!l.end(1, &mut w, true), "a consumed hold changes nothing");
        assert!(!l.end(3, &mut w, true));
        assert!(l.is_on());
        assert!(l.end(4, &mut w, true));
        assert!(!l.is_on());
        assert_eq!(w.warps, vec![(5.0, 6.0)]);
        assert_eq!(w.grabs, vec![true, false]);
        assert_eq!(w.visible, vec![false, true]);
        assert!(l.take_withheld_left());
        assert!(!l.take_withheld_left());
        assert_eq!(l.filter(false), PointerReport::Feed);
        assert_eq!(l.lock_count(), 1);
    }

    #[test]
    fn a_scale_change_rescales_the_frozen_position() {
        let mut w = Recorder::default();
        let mut l = PointerLock::new();
        assert_eq!(l.rescale(2.0, (800, 600)), None, "no lock, nothing changes");
        l.start(1, false, false, Some((100.0, 50.0)), &mut w);
        assert_eq!(l.rescale(2.0, (800, 600)), Some((200.0, 100.0)));
        assert!(l.is_on(), "the lock stays on");
        assert_eq!(l.rescale(10.0, (800, 600)), Some((799.0, 599.0)), "clamped");
        assert_eq!(l.rescale(f64::NAN, (800, 600)), Some((799.0, 599.0)));
        assert!(l.end(1, &mut w, false));
        assert_eq!(w.warps, vec![(799.0, 599.0)], "the warp back uses F′");
        assert_eq!(
            w.visible,
            vec![false],
            "software mode keeps the OS cursor hidden"
        );
    }

    #[test]
    fn timing_follows_settings_and_scale() {
        let none = tessera_settings::InputPrefs::default();
        let t = activation_timing(&none, None, 1.0);
        assert_eq!((t.time_ms(), t.width_px(), t.height_px()), (500, 4, 4));
        // The reference display (scale 1.333): 4 logical pixels are 5 physical ones.
        let t = activation_timing(&none, None, 4.0 / 3.0);
        assert_eq!((t.width_px(), t.height_px()), (5, 5));
        let set = tessera_settings::InputPrefs {
            double_click_ms: Some(300),
            double_click_px: Some(6),
        };
        let t = activation_timing(&set, Some((900, 10)), 2.0);
        assert_eq!((t.time_ms(), t.width_px()), (300, 12), "the file overrides");
        let t = activation_timing(&none, Some((900, 10)), f64::NAN);
        assert_eq!((t.time_ms(), t.width_px()), (900, 10), "platform, scale 1");
    }

    #[test]
    fn arrow_has_the_requested_size() {
        let a = arrow_bgra(32);
        assert_eq!(a.len(), 32 * 32 * 4);
        assert_eq!(a[3], 0xFF, "hot spot pixel is opaque");
        assert_eq!(arrow_bgra(0).len(), 8 * 8 * 4);
    }
}
