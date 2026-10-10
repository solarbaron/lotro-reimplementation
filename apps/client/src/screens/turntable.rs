// SPDX-License-Identifier: GPL-3.0-or-later
//! The turntable of the creation screens (`character-screens.md` §2.3, Tessera rule): two
//! hot buttons turn the figure about its vertical axis, two buttons switch the camera to
//! its zoom variant and back.
//!
//! While a turn button is held the figure turns at [`TURN_DEGREES_PER_SECOND`], starting
//! once the button has been held for its first repeat interval (read from the data,
//! [`DEFAULT_FIRST_INTERVAL`] without it); a press released earlier turns it by
//! [`STEP_DEGREES`]. The angle is in degrees, positive when the figure's front turns to the
//! viewer's left (the left-hand button).

use std::time::{Duration, Instant};

/// Turn speed while a button is held.
pub const TURN_DEGREES_PER_SECOND: f32 = 90.0;
/// Turn of a short press.
pub const STEP_DEGREES: f32 = 15.0;
/// The first repeat interval when the data gives none (`UICore_Button_hot_click_first_interval`
/// of this install's turn buttons).
pub const DEFAULT_FIRST_INTERVAL: Duration = Duration::from_millis(50);

/// A turn button.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Turn {
    /// The left-hand button: the figure's front turns to the viewer's left.
    Left,
    /// The right-hand button: to the right.
    Right,
}

impl Turn {
    fn sign(self) -> f32 {
        match self {
            Self::Left => 1.0,
            Self::Right => -1.0,
        }
    }
}

/// The turntable state.
#[derive(Debug, Clone, PartialEq)]
pub struct Turntable {
    /// The figure's turn in degrees, in [0, 360).
    angle: f32,
    /// The zoom variant of the camera is in use.
    zoomed: bool,
    /// The held button, when it was pressed, and the angle at the press.
    held: Option<(Turn, Instant, f32)>,
    /// The hold time before continuous turning starts.
    first_interval: Duration,
}

impl Default for Turntable {
    fn default() -> Self {
        Self {
            angle: 0.0,
            zoomed: false,
            held: None,
            first_interval: DEFAULT_FIRST_INTERVAL,
        }
    }
}

fn wrap(a: f32) -> f32 {
    let w = a.rem_euclid(360.0);
    if w.is_finite() { w } else { 0.0 }
}

impl Turntable {
    /// A turntable whose buttons start turning after `first_interval` (clamped to 0–2 s).
    #[must_use]
    pub fn with_first_interval(first_interval: Duration) -> Self {
        Self {
            first_interval: first_interval.min(Duration::from_secs(2)),
            ..Self::default()
        }
    }

    /// The angle in degrees, [0, 360).
    #[must_use]
    pub fn angle(&self) -> f32 {
        self.angle
    }

    /// Whether the zoom variant of the camera is in use.
    #[must_use]
    pub fn zoomed(&self) -> bool {
        self.zoomed
    }

    /// A turn button went down at `now` (a second press while one is held replaces it).
    pub fn press(&mut self, turn: Turn, now: Instant) {
        self.update(now);
        self.held = Some((turn, now, self.angle));
    }

    /// A turn button went up at `now`: a press shorter than the first interval turns by
    /// [`STEP_DEGREES`]. A release of the other button than the held one is ignored.
    pub fn release(&mut self, turn: Turn, now: Instant) {
        let Some((held, at, start)) = self.held else {
            return;
        };
        if held != turn {
            return;
        }
        let elapsed = now.saturating_duration_since(at);
        if elapsed < self.first_interval {
            self.angle = wrap(start + turn.sign() * STEP_DEGREES);
        } else {
            self.update(now);
        }
        self.held = None;
    }

    /// Advances a held button to `now`.
    pub fn update(&mut self, now: Instant) {
        let Some((turn, at, start)) = self.held else {
            return;
        };
        let elapsed = now.saturating_duration_since(at);
        if let Some(turning) = elapsed.checked_sub(self.first_interval) {
            self.angle =
                wrap(start + turn.sign() * TURN_DEGREES_PER_SECOND * turning.as_secs_f32());
        }
    }

    /// Zoom in: switch to the zoom variant (again: nothing).
    pub fn zoom_in(&mut self) {
        self.zoomed = true;
    }

    /// Zoom out: back to the camera (again: nothing).
    pub fn zoom_out(&mut self) {
        self.zoomed = false;
    }

    /// The angle back to 0 (stage, race or gender changed); a held button is let go.
    pub fn reset_angle(&mut self) {
        self.angle = 0.0;
        self.held = None;
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn short_press_steps_hold_turns() {
        let t0 = Instant::now();
        let mut t = Turntable::default();
        t.press(Turn::Left, t0);
        t.release(Turn::Left, t0 + Duration::from_millis(20));
        assert!((t.angle() - 15.0).abs() < 1e-4);
        t.press(Turn::Right, t0 + Duration::from_secs(1));
        t.update(t0 + Duration::from_millis(2050));
        assert!((t.angle() - 15.0 + 90.0).rem_euclid(360.0) < 1e-3);
        t.release(Turn::Right, t0 + Duration::from_millis(3050));
        assert!((t.angle() - (15.0 - 180.0f32).rem_euclid(360.0)).abs() < 1e-3);
        t.release(Turn::Right, t0 + Duration::from_secs(9));
        assert!(
            (t.angle() - 195.0).abs() < 1e-3,
            "release without press: nothing"
        );
        t.zoom_in();
        t.zoom_in();
        assert!(t.zoomed());
        t.zoom_out();
        assert!(!t.zoomed());
        t.reset_angle();
        assert_eq!(t.angle(), 0.0);
    }
}
