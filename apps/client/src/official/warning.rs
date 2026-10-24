// SPDX-License-Identifier: GPL-3.0-or-later
//! The ban-risk warning screen (`docs/specs/official-entry.md` §5.4, §5.5, §6.3).
//!
//! A full-window Tessera screen (not an engine dialog of `ui-widgets.md` §18; no install
//! layout). It reads only [`DeviceInput`] values, handed to it directly by the window-event
//! handler through the flow, never through the UI host's queues. Its input rules:
//!
//! 1. device input only (the type rule of [`DeviceInput`]);
//! 2. for its first [`INPUT_DELAY`] on screen every input except Decline is ignored, Tab
//!    included; Decline acts at any time by every route (Escape, closing the window, a click
//!    on Decline, Enter or Space on the focused Decline); any other pointer press counts only
//!    if it starts after the delay on the control, a click is a press and release on the same
//!    control; a key counts only if pressed after the delay, and a key held when the screen
//!    appeared (or pressed during the delay) is ignored until it is released;
//! 3. focus starts on Decline; Tab and Shift+Tab move it among the check box, Decline and
//!    Continue; Space toggles the focused check box; Enter or Space activates the focused
//!    button; only the user's own Tab presses or a pointer press on it give Continue focus;
//! 4. Continue is disabled (ghosted, no input, no focus) until the check box is ticked;
//! 5. no shortcut accepts; Escape is Decline at any time, closing the window is Decline;
//! 6. activating the focused Continue with Enter or Space after ticking the box counts like
//!    a click;
//! 7. until the text has been laid out in full with the bundled font the delay has not begun
//!    and Continue stays disabled (fail closed).
//!
//! The Continue handler (`WarningScreen::activate_continue`) is the only place outside
//! test code that creates a [`tessera_compat::consent::UserConfirmation`] and calls the
//! acceptance writer (§6.3, source scan OE-T6 (d)).

use std::collections::HashSet;
use std::path::PathBuf;
use std::sync::Arc;
use std::time::{Duration, Instant};

use tessera_compat::consent::{Acceptance, PendingWrite, WarningText};

use super::input::{DeviceInput, DeviceInputKind, Key};

/// How long every input but Escape and window close is ignored (§5.4 rule 2).
pub const INPUT_DELAY: Duration = Duration::from_millis(1000);

/// A control of the warning screen that takes input.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum WarningControl {
    /// The check box (`official.consent.checkbox`).
    CheckBox,
    /// Decline (left).
    Decline,
    /// Continue (right).
    Continue,
}

/// How the screen ended.
#[derive(Debug)]
pub enum WarningOutcome {
    /// The user accepted: the acceptance holds now, the record is being written.
    Accepted {
        /// The in-memory acceptance (§5.5).
        acceptance: Acceptance,
        /// The record write on its worker thread.
        write: PendingWrite,
    },
    /// Decline, Escape or closing the window: nothing is written.
    Declined,
}

/// The warning screen's state (module docs).
#[derive(Debug)]
pub struct WarningScreen {
    /// Where the acceptance is written (`None`: nothing is written, §5.5).
    record_dir: Option<PathBuf>,
    text: Arc<WarningText>,
    /// A record with another digest exists: the `.changed` line is shown.
    changed: bool,
    /// When the text was first laid out in full (the delay starts then).
    shown_at: Option<Instant>,
    ticked: bool,
    focus: WarningControl,
    /// Keys ignored until they are released (held at show time or pressed during the delay).
    ignored: HashSet<Key>,
    /// The control a counting pointer press started on.
    pressed_on: Option<WarningControl>,
    pointer: (f64, f64),
    /// Body scroll offset in lines.
    scroll: u32,
}

impl WarningScreen {
    /// A new screen for `text`, whose acceptance would be written to `record_dir` (`None`:
    /// no record directory, nothing is written). `held`: the keys down when the screen
    /// appears (ignored until released).
    #[must_use]
    pub fn new(
        record_dir: Option<PathBuf>,
        text: Arc<WarningText>,
        changed: bool,
        held: impl IntoIterator<Item = Key>,
    ) -> Self {
        Self {
            record_dir,
            text,
            changed,
            shown_at: None,
            ticked: false,
            focus: WarningControl::Decline,
            ignored: held.into_iter().collect(),
            pressed_on: None,
            pointer: (-1.0, -1.0),
            scroll: 0,
        }
    }

    /// The shown text.
    #[must_use]
    pub fn text(&self) -> &WarningText {
        &self.text
    }

    /// Whether the `.changed` line is shown.
    #[must_use]
    pub fn changed(&self) -> bool {
        self.changed
    }

    /// Whether the box is ticked.
    #[must_use]
    pub fn ticked(&self) -> bool {
        self.ticked
    }

    /// The focused control.
    #[must_use]
    pub fn focus(&self) -> WarningControl {
        self.focus
    }

    /// The body's scroll offset in lines.
    #[must_use]
    pub fn scroll(&self) -> u32 {
        self.scroll
    }

    /// Limits the scroll offset to `max` lines (the view knows the body's length).
    pub fn clamp_scroll(&mut self, max: u32) {
        self.scroll = self.scroll.min(max);
    }

    /// Whether the text has been laid out in full (rule 7).
    #[must_use]
    pub fn text_ready(&self) -> bool {
        self.shown_at.is_some()
    }

    /// The view reports that the whole text is laid out with the bundled font at `now`: the
    /// delay starts with the first such report (rule 7).
    pub fn set_text_ready(&mut self, now: Instant) {
        if self.shown_at.is_none() {
            self.shown_at = Some(now);
        }
    }

    /// Whether the delay is over at `t` (never before the text is ready).
    #[must_use]
    pub fn delay_over(&self, t: Instant) -> bool {
        self.shown_at
            .is_some_and(|s| t.saturating_duration_since(s) >= INPUT_DELAY)
    }

    /// Whether Continue takes input (rules 4 and 7).
    #[must_use]
    pub fn continue_enabled(&self) -> bool {
        self.ticked && self.text_ready()
    }

    fn order(&self) -> Vec<WarningControl> {
        let mut v = vec![WarningControl::CheckBox, WarningControl::Decline];
        if self.continue_enabled() {
            v.push(WarningControl::Continue);
        }
        v
    }

    fn move_focus(&mut self, back: bool) {
        let order = self.order();
        let n = order.len();
        let i = order.iter().position(|c| *c == self.focus).unwrap_or(0);
        let j = if back { (i + n - 1) % n } else { (i + 1) % n };
        if let Some(c) = order.get(j) {
            self.focus = *c;
        }
    }

    fn toggle(&mut self) {
        self.ticked = !self.ticked;
        if !self.ticked && self.focus == WarningControl::Continue {
            self.focus = WarningControl::CheckBox;
        }
    }

    /// One device-input event; `hit` names the control under a window position (the view's
    /// layout). Returns the outcome when the screen ends.
    pub fn input(
        &mut self,
        ev: &DeviceInput,
        hit: &dyn Fn(f64, f64) -> Option<WarningControl>,
    ) -> Option<WarningOutcome> {
        if !self.text_ready() {
            return self.input_not_ready(ev, hit);
        }
        let after = self.delay_over(ev.at());
        match ev.kind() {
            DeviceInputKind::CloseRequested => return Some(WarningOutcome::Declined),
            DeviceInputKind::FocusLost => self.pressed_on = None,
            DeviceInputKind::Key {
                key: Key::Escape,
                pressed: true,
                ..
            } => return Some(WarningOutcome::Declined),
            DeviceInputKind::Key {
                key,
                pressed: false,
                ..
            } => {
                self.ignored.remove(key);
            }
            DeviceInputKind::Key {
                key,
                pressed: true,
                repeat,
                held_before,
                modifiers,
                ..
            } => {
                if *held_before || self.ignored.contains(key) {
                    self.ignored.insert(*key);
                    return None;
                }
                // Decline acts at any time, without the delay (rule 2).
                if !*repeat
                    && matches!(key, Key::Enter | Key::Space)
                    && self.focus == WarningControl::Decline
                {
                    return Some(WarningOutcome::Declined);
                }
                if !after {
                    self.ignored.insert(*key);
                    return None;
                }
                if *repeat {
                    return None;
                }
                match key {
                    Key::Tab => self.move_focus(modifiers.shift),
                    Key::Space if self.focus == WarningControl::CheckBox => self.toggle(),
                    Key::Space | Key::Enter => match self.focus {
                        WarningControl::Decline => return Some(WarningOutcome::Declined),
                        WarningControl::Continue => return self.activate_continue(ev),
                        WarningControl::CheckBox => {}
                    },
                    Key::Up => self.scroll = self.scroll.saturating_sub(1),
                    Key::Down => self.scroll = self.scroll.saturating_add(1),
                    Key::PageUp => self.scroll = self.scroll.saturating_sub(10),
                    Key::PageDown => self.scroll = self.scroll.saturating_add(10),
                    _ => {}
                }
            }
            DeviceInputKind::PointerMoved { x, y } => self.pointer = (*x, *y),
            DeviceInputKind::PointerButton { pressed: true } => {
                self.pressed_on = None;
                if !after {
                    // During the delay only a press on Decline counts (rule 2).
                    self.pressed_on = hit(self.pointer.0, self.pointer.1)
                        .filter(|c| *c == WarningControl::Decline);
                    return None;
                }
                let on = hit(self.pointer.0, self.pointer.1)
                    .filter(|c| *c != WarningControl::Continue || self.continue_enabled());
                if let Some(c) = on {
                    self.focus = c;
                }
                self.pressed_on = on;
            }
            DeviceInputKind::PointerButton { pressed: false } => {
                let started = self.pressed_on.take();
                let now_on = hit(self.pointer.0, self.pointer.1);
                if started.is_some() && started == now_on {
                    match started {
                        Some(WarningControl::CheckBox) => self.toggle(),
                        Some(WarningControl::Decline) => return Some(WarningOutcome::Declined),
                        Some(WarningControl::Continue) => return self.activate_continue(ev),
                        None => {}
                    }
                }
            }
            DeviceInputKind::Wheel { lines } if after => {
                let n = lines.abs().round().clamp(0.0, 100.0) as u32;
                self.scroll = if *lines > 0.0 {
                    self.scroll.saturating_sub(n)
                } else {
                    self.scroll.saturating_add(n)
                };
            }
            DeviceInputKind::Wheel { .. } => {}
        }
        None
    }

    /// Input while the text is not laid out (the font is loading or cannot be drawn, rule 7):
    /// the screen offers only Decline, and declining works at any time, without the delay
    /// (a click on Decline, or Enter or Space on the focused Decline). Nothing else acts.
    fn input_not_ready(
        &mut self,
        ev: &DeviceInput,
        hit: &dyn Fn(f64, f64) -> Option<WarningControl>,
    ) -> Option<WarningOutcome> {
        match ev.kind() {
            DeviceInputKind::CloseRequested => Some(WarningOutcome::Declined),
            DeviceInputKind::Key {
                key: Key::Escape,
                pressed: true,
                ..
            } => Some(WarningOutcome::Declined),
            DeviceInputKind::Key {
                key: k @ (Key::Enter | Key::Space),
                pressed: true,
                repeat: false,
                held_before: false,
                ..
            } if self.focus == WarningControl::Decline && !self.ignored.contains(k) => {
                Some(WarningOutcome::Declined)
            }
            DeviceInputKind::Key {
                key, pressed: true, ..
            } => {
                self.ignored.insert(*key);
                None
            }
            DeviceInputKind::Key {
                key,
                pressed: false,
                ..
            } => {
                self.ignored.remove(key);
                None
            }
            DeviceInputKind::PointerMoved { x, y } => {
                self.pointer = (*x, *y);
                None
            }
            DeviceInputKind::PointerButton { pressed: true } => {
                self.pressed_on =
                    hit(self.pointer.0, self.pointer.1).filter(|c| *c == WarningControl::Decline);
                None
            }
            DeviceInputKind::PointerButton { pressed: false } => {
                let started = self.pressed_on.take();
                (started.is_some() && started == hit(self.pointer.0, self.pointer.1))
                    .then_some(WarningOutcome::Declined)
            }
            DeviceInputKind::FocusLost | DeviceInputKind::Wheel { .. } => None,
        }
    }

    /// The Continue handler (§5.5, §6.3): with the box ticked, after the delay, the
    /// device-input event `ev` activated Continue. The only creator of the user-confirmation
    /// value and the only caller of the acceptance writer outside test code.
    fn activate_continue(&mut self, ev: &DeviceInput) -> Option<WarningOutcome> {
        if !self.continue_enabled() || !self.delay_over(ev.at()) {
            return None;
        }
        let confirmation = tessera_compat::consent::UserConfirmation::confirmed_by_user();
        let (acceptance, write) =
            tessera_compat::consent::accept(self.record_dir.as_deref(), &self.text, confirmation);
        tracing::info!(
            revision = self.text.revision,
            "official-server warning accepted on the warning screen"
        );
        Some(WarningOutcome::Accepted { acceptance, write })
    }
}
