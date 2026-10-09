// SPDX-License-Identifier: GPL-3.0-or-later
//! The UI side of the input bridge (decision 0014), as `tessera-ui`'s own API with no
//! dependency on `tessera-input`.
//!
//! - **Offers** (D7): the client, as the mapper's sink, turns each offered event into a
//!   [`UiInput`] and calls [`Ui::offer`], which routes it by the class of its command name
//!   (`ui-behaviour.md` §3.1) and answers the verdict: a key command is consumed when the
//!   keyboard target or a listener consumes it (§5.3), a mouse-button command when its press
//!   had a hit element or a capture (§4.6 Tessera rule), a wheel command when a scroll host
//!   stopped it (§4.7). The mapper uses only a Start's verdict.
//! - **Band requests** (D5): every push and pop of the UI's ancestor walks and hover pushes
//!   goes to an outbox ([`Ui::take_band_changes`]); during an offer the client translates
//!   it into the offer's request list, and after step 7 (and step 8) it drains the rest into
//!   the mapper.
//! - **Step 1 and step 7 entry points** (ARCHITECTURE §20.3, D8): [`Ui::offer`] in step 1;
//!   in step 7 [`Ui::tick_hover`] (queued UI change requests, the hover refresh and the
//!   hover check that delivers `MouseHover`), then the plugin host's `Update` broadcast over
//!   [`Ui::update_subscribers`], then [`Ui::frame_work`].
//! - **UI change requests** (Amendment 1 item 2): code outside steps 1 and 7 queues its UI
//!   changes with [`Ui::request`]; they are applied at the start of step 7, in request order,
//!   screen sizes coalesced to the last one.

use std::collections::BTreeMap;
use std::sync::Arc;

use crate::events::Command;
use crate::tree::ElementKey;
use crate::ui::Ui;

/// Which part of a hold an offered command is (decision 0014 D7).
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum Phase {
    /// The hold begins.
    Start,
    /// An OS auto-repeat of a repeatable hold.
    Repeat,
    /// The hold ends.
    End,
}

/// One offered command.
#[derive(Debug, Clone, PartialEq)]
pub struct CommandOffer {
    /// Start, Repeat or End.
    pub phase: Phase,
    /// The hold the event belongs to (shared by its Start, Repeats and End).
    pub hold: u64,
    /// The command name and ID.
    pub command: Command,
    /// The value (1.0 for button actions).
    pub value: f32,
    /// The pointer position carried by the offer (physical pixels), when known.
    pub position: Option<(f64, f64)>,
}

/// Everything the client offers to the UI (decision 0014 D7, "What is offered").
#[derive(Debug, Clone, PartialEq)]
pub enum UiInput {
    /// An action event with a command.
    Command(CommandOffer),
    /// A typed character.
    Char(char),
    /// The coalesced pointer move.
    PointerMoved {
        /// Physical pixels.
        x: f64,
        /// Physical pixels.
        y: f64,
    },
    /// The pointer left the window.
    PointerLeft,
    /// The window lost focus.
    FocusLost,
}

/// A UI change requested outside frame-loop steps 1 and 7 (decision 0014 Amendment 1
/// item 2).
#[derive(Debug, Clone, PartialEq)]
pub enum UiChange {
    /// Show or hide.
    Show(ElementKey, bool),
    /// Activate (by code).
    Activate(ElementKey),
    /// Give or clear focus.
    Focus(Option<ElementKey>),
    /// Bring to front among siblings of its z-level.
    BringToFront(ElementKey),
    /// Move (layout position in the parent).
    Move(ElementKey, i32, i32),
    /// Element resize.
    Resize(ElementKey, i32, i32),
    /// A new screen size.
    Screen(i32, i32),
}

/// Most queued change requests (Tessera cap).
pub const MAX_CHANGES: usize = 1 << 16;

/// Action ID the UI ignores entirely (`lua-plugin-api.md` §13.13, P48).
pub const IGNORED_ACTION: u32 = 0xAB;

/// Toggle type Impulse (`lua-plugin-api.md` §13.12.1).
pub const TOGGLE_IMPULSE: u8 = 2;

/// Bridge state: the client's toggle-type table (D9).
#[derive(Debug, Clone, Default)]
pub(crate) struct BridgeState {
    pub toggle: BTreeMap<u32, u8>,
    /// Holds routed as mouse buttons, with their command (End releases it).
    pub mouse_holds: BTreeMap<u64, Command>,
    /// Holds whose Start was an Impulse (End ignored).
    pub impulse_holds: Vec<u64>,
}

fn pos_i32(p: (f64, f64)) -> (i32, i32) {
    let c = |v: f64| {
        if v.is_nan() {
            0
        } else {
            v.floor().clamp(
                -f64::from(crate::geom::COORD_LIMIT),
                f64::from(crate::geom::COORD_LIMIT),
            ) as i32
        }
    };
    (c(p.0), c(p.1))
}

impl Ui {
    /// Sets the toggle-type table (command ID → toggle type) read by the client from the
    /// action map (decision 0014 D9). IDs not in the table have type 0.
    pub fn set_toggle_types(&mut self, table: BTreeMap<u32, u8>) {
        self.bridge.toggle = table;
    }

    fn toggle_type(&self, c: &Command) -> u8 {
        c.id.and_then(|id| self.bridge.toggle.get(&id).copied())
            .unwrap_or(0)
    }

    /// Offers one event (decision 0014 D7). Returns the verdict: `true` = consumed.
    pub fn offer(&mut self, input: &UiInput) -> bool {
        match input {
            UiInput::PointerMoved { x, y } => {
                let (x, y) = pos_i32((*x, *y));
                self.pointer_moved(x, y);
                false
            }
            UiInput::PointerLeft => {
                self.pointer_left();
                false
            }
            UiInput::FocusLost => {
                self.focus_lost();
                false
            }
            UiInput::Char(c) => self.text_input(*c),
            UiInput::Command(o) => self.offer_command(o),
        }
    }

    fn offer_command(&mut self, o: &CommandOffer) -> bool {
        if o.command.id == Some(IGNORED_ACTION) {
            return false;
        }
        if self.input.pointer.is_none()
            && self.input.lock.is_none()
            && let Some(p) = o.position
        {
            self.input.pointer = Some(pos_i32(p));
        }
        let impulse = self.toggle_type(&o.command) == TOGGLE_IMPULSE;
        if o.command.is_mouse_button() {
            return match o.phase {
                Phase::Start if impulse => self.mouse_impulse(&o.command),
                Phase::Start => {
                    self.bridge.mouse_holds.insert(o.hold, o.command.clone());
                    self.mouse_press(&o.command)
                }
                Phase::Repeat => false,
                Phase::End => match self.bridge.mouse_holds.remove(&o.hold) {
                    Some(c) => self.mouse_release(&c),
                    None => false,
                },
            };
        }
        if o.command.is_wheel() {
            return match o.phase {
                Phase::Start => {
                    if &*o.command.name == crate::vocab::commands::SCROLL {
                        // `Scroll` (analog) is not handled by scroll hosts (§4.7).
                        false
                    } else {
                        self.wheel(&o.command)
                    }
                }
                _ => false,
            };
        }
        match o.phase {
            Phase::Start | Phase::Repeat => {
                if impulse {
                    self.bridge.impulse_holds.push(o.hold);
                    if self.bridge.impulse_holds.len() > 4096 {
                        self.bridge.impulse_holds.remove(0);
                    }
                }
                self.key_command(&o.command, true, o.value)
            }
            Phase::End => {
                if let Some(i) = self.bridge.impulse_holds.iter().position(|h| *h == o.hold) {
                    self.bridge.impulse_holds.remove(i);
                    return false;
                }
                self.key_command(&o.command, false, o.value)
            }
        }
    }

    /// Queues a UI change made outside steps 1 and 7 (Amendment 1 item 2).
    pub fn request(&mut self, change: UiChange) {
        if self.changes.len() < MAX_CHANGES {
            self.changes.push_back(change);
        }
    }

    /// Applies the queued change requests in order (start of step 7). Screen sizes queued
    /// before one step 7 are applied as one change to the last value.
    pub fn apply_change_requests(&mut self) {
        let last_screen = self.changes.iter().rev().find_map(|c| match c {
            UiChange::Screen(w, h) => Some((*w, *h)),
            _ => None,
        });
        let mut screen_done = false;
        while let Some(c) = self.changes.pop_front() {
            match c {
                UiChange::Show(k, s) => self.set_shown(k, s),
                UiChange::Activate(k) => self.request_activation(k),
                UiChange::Focus(f) => self.set_focus(f),
                UiChange::BringToFront(k) => self.bring_to_front(k),
                UiChange::Move(k, x, y) => self.move_to(k, x, y),
                UiChange::Resize(k, w, h) => self.resize(k, w, h),
                UiChange::Screen(..) => {
                    if !screen_done && let Some((w, h)) = last_screen {
                        self.resize_screen(w, h);
                        screen_done = true;
                    }
                }
            }
        }
    }

    /// Step 7 items 1–2 (decision 0014 D8): the queued change requests, the hover refresh
    /// and the hover check (`MouseHover`, tooltips).
    pub fn tick_hover(&mut self) {
        self.apply_change_requests();
        self.hover_refresh();
        self.hover_check();
    }

    /// Marks the pointer for re-evaluation at the next hover refresh (for changes made by
    /// plugin `Update` handlers; LPA §3.3).
    pub fn invalidate_hover(&mut self) {
        self.input.hover_dirty = true;
    }

    /// The command of a hold the UI routed as a mouse button, if it is still held.
    #[must_use]
    pub fn held_mouse_command(&self, hold: u64) -> Option<Arc<str>> {
        self.bridge.mouse_holds.get(&hold).map(|c| c.name.clone())
    }
}
