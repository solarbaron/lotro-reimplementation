// SPDX-License-Identifier: GPL-3.0-or-later
//! Activation timing (`keymap.md` §7.7): the double-click time *T* and
//! rectangle *W* × *H*, and the per-button record of recorded presses.

use std::ops::RangeInclusive;
use std::time::Duration;

use crate::device::Button;

/// *T* the mapper uses until the client sets one (ms).
pub const DEFAULT_DOUBLE_CLICK_MS: u32 = 500;
/// *W* and *H* the mapper uses until the client sets them (physical px).
pub const DEFAULT_DOUBLE_CLICK_PX: u32 = 4;
/// *T* is clamped into this range (ms).
pub const DOUBLE_CLICK_MS_RANGE: RangeInclusive<u32> = 100..=5000;
/// *W* and *H* are clamped into this range (physical px, after scaling).
pub const DOUBLE_CLICK_PX_RANGE: RangeInclusive<u32> = 2..=64;
/// Most buttons whose last recorded press is remembered.
pub const MAX_PRESS_RECORDS: usize = 64;

/// The values that decide `double`, `double_nearby` and `tap` (§7.7):
/// *T* in milliseconds and the nearby rectangle *W* × *H* in physical
/// pixels. Always within [`DOUBLE_CLICK_MS_RANGE`] and
/// [`DOUBLE_CLICK_PX_RANGE`].
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub struct ActivationTiming {
    time_ms: u32,
    width_px: u32,
    height_px: u32,
}

impl Default for ActivationTiming {
    /// 500 ms and 4 × 4 px (§7.7, Tessera rule).
    fn default() -> Self {
        Self::new(
            DEFAULT_DOUBLE_CLICK_MS,
            DEFAULT_DOUBLE_CLICK_PX,
            DEFAULT_DOUBLE_CLICK_PX,
        )
    }
}

impl ActivationTiming {
    /// *T*, *W*, *H*, each clamped into its range.
    #[must_use]
    pub fn new(time_ms: u32, width_px: u32, height_px: u32) -> Self {
        let px = |v: u32| v.clamp(*DOUBLE_CLICK_PX_RANGE.start(), *DOUBLE_CLICK_PX_RANGE.end());
        Self {
            time_ms: time_ms.clamp(*DOUBLE_CLICK_MS_RANGE.start(), *DOUBLE_CLICK_MS_RANGE.end()),
            width_px: px(width_px),
            height_px: px(height_px),
        }
    }

    /// From a rectangle size in **logical** pixels (a platform value or
    /// Tessera's `double_click_px`) and the window's scale factor: each side
    /// is multiplied by the scale factor, rounded to the nearest integer
    /// (halves away from zero) and clamped (§7.7 "Pixel space").
    #[must_use]
    pub fn from_logical(time_ms: u32, width: f64, height: f64, scale_factor: f64) -> Self {
        Self::new(
            time_ms,
            scale_pixels(width, scale_factor),
            scale_pixels(height, scale_factor),
        )
    }

    /// *T* in milliseconds.
    #[must_use]
    pub fn time_ms(self) -> u32 {
        self.time_ms
    }

    /// *W* in physical pixels.
    #[must_use]
    pub fn width_px(self) -> u32 {
        self.width_px
    }

    /// *H* in physical pixels.
    #[must_use]
    pub fn height_px(self) -> u32 {
        self.height_px
    }

    /// *T* as a duration.
    #[must_use]
    pub fn time(self) -> Duration {
        Duration::from_millis(u64::from(self.time_ms))
    }

    /// half(*T*): the longest press that still taps.
    #[must_use]
    pub fn tap_time(self) -> Duration {
        Duration::from_millis(u64::from(self.time_ms / 2))
    }

    /// Whether `now` is inside the half-open rectangle around `previous`:
    /// *x0* − half(*W*) ≤ *x* < *x0* + half(*W*), the same for *y*.
    #[must_use]
    pub fn nearby(self, previous: (f64, f64), now: (f64, f64)) -> bool {
        let hw = f64::from(self.width_px / 2);
        let hh = f64::from(self.height_px / 2);
        let (x0, y0) = previous;
        let (x, y) = now;
        x0 - hw <= x && x < x0 + hw && y0 - hh <= y && y < y0 + hh
    }
}

/// `px × scale` rounded to the nearest integer, halves away from zero;
/// non-finite or negative results give 0 (then clamped by the caller).
#[must_use]
pub fn scale_pixels(px: f64, scale_factor: f64) -> u32 {
    let v = (px * scale_factor).round();
    if v.is_finite() && v > 0.0 {
        if v >= f64::from(u32::MAX) {
            u32::MAX
        } else {
            v as u32
        }
    } else {
        0
    }
}

/// The last recorded press of one button.
#[derive(Debug, Clone, Copy, PartialEq)]
pub(super) struct PressRecord {
    pub(super) button: Button,
    pub(super) time: Duration,
    pub(super) position: Option<(f64, f64)>,
}

/// The last recorded press of the [`MAX_PRESS_RECORDS`] most recently
/// pressed buttons (§7.7 "Per-button tracking").
#[derive(Debug, Clone, Default)]
pub(super) struct PressRecords {
    /// Oldest first.
    records: Vec<PressRecord>,
}

impl PressRecords {
    pub(super) fn get(&self, button: Button) -> Option<&PressRecord> {
        self.records.iter().find(|r| r.button == button)
    }

    /// Record a press, making `button` the most recent; evicts the oldest
    /// record past the limit.
    pub(super) fn record(&mut self, button: Button, time: Duration, position: Option<(f64, f64)>) {
        self.records.retain(|r| r.button != button);
        self.records.push(PressRecord {
            button,
            time,
            position,
        });
        if self.records.len() > MAX_PRESS_RECORDS {
            self.records.remove(0);
        }
    }

    /// Forget every recorded position (scale-factor change); times stay.
    pub(super) fn forget_positions(&mut self) {
        for r in &mut self.records {
            r.position = None;
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::device::{KeyCode, MouseButton};

    #[test]
    fn clamps_and_scaling() {
        let t = ActivationTiming::new(1, 1, 1000);
        assert_eq!((t.time_ms(), t.width_px(), t.height_px()), (100, 2, 64));
        let t = ActivationTiming::from_logical(500, 4.0, 4.0, 2.0);
        assert_eq!((t.width_px(), t.height_px()), (8, 8));
        assert_eq!(scale_pixels(2.5, 1.0), 3);
        assert_eq!(scale_pixels(1.25, 2.0), 3);
        assert_eq!(scale_pixels(f64::NAN, 1.0), 0);
        assert_eq!(scale_pixels(-3.0, 1.0), 0);
        assert_eq!(
            ActivationTiming::default().tap_time(),
            Duration::from_millis(250)
        );
    }

    #[test]
    fn half_open_rectangle() {
        let t = ActivationTiming::new(500, 4, 4);
        let p = (100.0, 50.0);
        assert!(t.nearby(p, (98.0, 48.0)));
        assert!(t.nearby(p, (101.9, 51.9)));
        assert!(!t.nearby(p, (102.0, 50.0)));
        assert!(!t.nearby(p, (97.9, 50.0)));
        let two = ActivationTiming::new(500, 2, 2);
        assert!(two.nearby(p, p));
        assert!(!two.nearby(p, (101.0, 50.0)));
    }

    #[test]
    fn records_are_bounded_and_most_recent_wins() {
        let mut r = PressRecords::default();
        for n in 0..(MAX_PRESS_RECORDS as u16 + 5) {
            r.record(
                Button::Mouse(MouseButton::Other(n + 6)),
                Duration::from_millis(u64::from(n)),
                None,
            );
        }
        assert!(r.get(Button::Mouse(MouseButton::Other(6))).is_none());
        assert!(r.get(Button::Mouse(MouseButton::Other(70))).is_some());
        let k = Button::Key(KeyCode::A);
        r.record(k, Duration::ZERO, Some((1.0, 1.0)));
        r.forget_positions();
        assert_eq!(r.get(k).unwrap().position, None);
    }
}
