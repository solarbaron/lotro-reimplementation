// SPDX-License-Identifier: GPL-3.0-or-later
//! The System status overlay of an official session (`official-entry.md` §8.7 "P10-10J display
//! policy"): the in-world system-message area is the chat window's System line list; when no chat
//! window runs (no game UI attached), the same bounded plain line shows in this overlay, painted by
//! Tessera's own code and labelled "System". A line stays until the player activates its visible
//! Dismiss button, or the next one replaces it (§8.7). A click on the button activates it, and so
//! does Tab (which focuses the button) then Enter or Space. Escape keeps its phase-specific close
//! meaning and does not dismiss the overlay. It takes no command input and shows no markup; it is no
//! plugin-host element.
//!
//! Engineering choices, spec silent: the overlay sits at the top centre of the viewport with its
//! button to the right of the line; a second Tab takes the focus off the button again, so Tab's other
//! meanings return; a new line clears the focus; and it shows only in an official session (other
//! sessions keep their chat overlay and HUD notices).

use tessera_input::{KeyCode, MouseButton, RawEvent};
use tessera_render::{Pass, Renderer};

use super::hud::TextPanel;
use crate::net::ChatLine;

/// The overlay's label (§8.7).
pub const LABEL: &str = "System";

/// Most UTF-8 bytes of one shown line (the bound of a notice line, `official-entry.md` §8.2): a
/// longer line is cut at a character boundary (the session drops over-long notices before this).
pub const MAX_LINE_BYTES: usize = 256;

/// The visible Dismiss button's text (§8.7).
pub const DISMISS: &str = "[ Dismiss ]";

/// Pixels between the line and its button.
const BUTTON_GAP: f32 = 8.0;

const GOLD: [u8; 3] = [255, 214, 120];
/// The button's colour while it has the keyboard focus.
const FOCUSED: [u8; 3] = [255, 255, 255];
/// The button's colour otherwise.
const BUTTON: [u8; 3] = [200, 200, 200];

/// The overlay (module docs).
#[derive(Debug, Default)]
pub struct SystemStatus {
    line: Option<String>,
    panel: TextPanel,
    button: TextPanel,
    /// The Dismiss button has the keyboard focus (Tab).
    focused: bool,
    /// The pointer's last position, physical pixels.
    cursor: Option<(f64, f64)>,
}

/// Whether `line` is a system line: no sender, or the System channel, and not one of the chat lines
/// the client derives itself (`ChatLine::shown`).
#[must_use]
pub fn is_system_line(line: &ChatLine) -> bool {
    (line.sender.is_empty() || line.channel == tessera_net::ChatChannel::System)
        && line.shown.is_none()
}

/// `text` as shown: control characters become spaces, cut at [`MAX_LINE_BYTES`].
fn plain(text: &str) -> String {
    let mut out = String::with_capacity(text.len().min(MAX_LINE_BYTES));
    for c in text.chars() {
        let c = if c.is_control() { ' ' } else { c };
        if out.len() + c.len_utf8() > MAX_LINE_BYTES {
            break;
        }
        out.push(c);
    }
    out
}

impl SystemStatus {
    /// Shows `text`, replacing the line shown.
    pub fn show(&mut self, text: &str) {
        self.line = Some(plain(text));
        self.focused = false;
    }

    /// Dismisses the line (the Dismiss button's action); returns whether one showed.
    pub fn dismiss(&mut self) -> bool {
        self.focused = false;
        self.line.take().is_some()
    }

    /// Whether the Dismiss button has the keyboard focus.
    #[must_use]
    pub const fn focused(&self) -> bool {
        self.focused
    }

    /// Where the line and the button are drawn for an overlay centred on `centre_x` with its top at
    /// `top`: the line's top-left corner and the button's `[x, y, width, height]`.
    fn layout(&self, centre_x: f32, top: f32) -> Option<((f32, f32), [f32; 4])> {
        self.line.as_ref()?;
        let (lw, _) = self.panel.size()?;
        let (bw, bh) = self.button.size()?;
        let total = lw as f32 + BUTTON_GAP + bw as f32;
        let x = (centre_x - total / 2.0).max(0.0);
        let bx = x + lw as f32 + BUTTON_GAP;
        Some(((x, top), [bx, top, bw as f32, bh as f32]))
    }

    /// One raw input event for an overlay drawn at `centre_x`, `top`; whether the overlay used it
    /// (it then goes no further). See [`Self::handle_with`].
    pub fn handle(&mut self, event: &RawEvent, centre_x: f32, top: f32) -> bool {
        let button = self.layout(centre_x, top).map(|(_, b)| b);
        self.handle_with(event, button)
    }

    /// One raw input event with the button at `button` (`[x, y, width, height]`, physical pixels;
    /// `None` when it is not drawn): a left click on it dismisses the line; Tab moves the keyboard
    /// focus onto the button (or off it again); Enter or Space with the focus on it dismisses the
    /// line. Escape is never used here (§8.7). Answers whether the event was used.
    pub fn handle_with(&mut self, event: &RawEvent, button: Option<[f32; 4]>) -> bool {
        match event {
            RawEvent::CursorMoved { x, y } => {
                self.cursor = Some((*x, *y));
                return false;
            }
            RawEvent::CursorLeft => {
                self.cursor = None;
                return false;
            }
            _ => {}
        }
        if self.line.is_none() {
            return false;
        }
        match event {
            RawEvent::MouseButton {
                button: MouseButton::Left,
                pressed: true,
            } => {
                let hit = match (button, self.cursor) {
                    (Some([bx, by, bw, bh]), Some((x, y))) => {
                        let (x, y) = (x as f32, y as f32);
                        x >= bx && x < bx + bw && y >= by && y < by + bh
                    }
                    _ => false,
                };
                if hit {
                    self.dismiss();
                }
                hit
            }
            RawEvent::Key {
                key: KeyCode::Tab,
                pressed: true,
                repeat: false,
            } => {
                self.focused = !self.focused;
                true
            }
            RawEvent::Key {
                key: KeyCode::Enter | KeyCode::NumpadEnter | KeyCode::Space,
                pressed: true,
                repeat: false,
            } if self.focused => {
                self.dismiss();
                true
            }
            _ => false,
        }
    }

    /// The line shown, without its label.
    #[must_use]
    pub fn line(&self) -> Option<&str> {
        self.line.as_deref()
    }

    /// The overlay's text: `System: ⟨line⟩`.
    #[must_use]
    pub fn text(&self) -> Option<String> {
        self.line.as_ref().map(|l| format!("{LABEL}: {l}"))
    }

    /// Re-rasterises the line and its button when they changed.
    pub fn refresh(&mut self, renderer: &mut Renderer<'_>, scale: u32) {
        match self.text() {
            Some(t) => {
                self.panel.set(renderer, vec![(t, GOLD)], scale);
                let colour = if self.focused { FOCUSED } else { BUTTON };
                self.button
                    .set(renderer, vec![(DISMISS.to_owned(), colour)], scale);
            }
            None => {
                self.panel.release(renderer);
                self.button.release(renderer);
            }
        }
    }

    /// Draws the line and its Dismiss button centred on `centre_x` with their top at `top`.
    pub fn draw(&self, pass: &mut Pass, centre_x: f32, top: f32) {
        if let Some(((x, y), [bx, by, _, _])) = self.layout(centre_x, top) {
            self.panel.draw(pass, x, y);
            self.button.draw(pass, bx, by);
        }
    }

    /// Frees the panels' textures.
    pub fn release(&mut self, renderer: &mut Renderer<'_>) {
        self.panel.release(renderer);
        self.button.release(renderer);
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn a_line_is_labelled_plain_bounded_and_replaced_until_dismissed() {
        let mut s = SystemStatus::default();
        assert_eq!(s.text(), None);
        s.show("first\u{7}line");
        assert_eq!(s.text().as_deref(), Some("System: first line"));
        s.show("second");
        assert_eq!(s.line(), Some("second"), "the next line replaces it");
        assert!(s.dismiss());
        assert_eq!(s.line(), None);
        assert!(!s.dismiss());
        let long = "é".repeat(300);
        s.show(&long);
        let shown = s.line().expect("shown");
        assert!(shown.len() <= MAX_LINE_BYTES);
        assert!(shown.chars().all(|c| c == 'é'));
    }

    fn key(key: KeyCode) -> RawEvent {
        RawEvent::Key {
            key,
            pressed: true,
            repeat: false,
        }
    }

    /// §8.7: a click on the visible Dismiss button, or Tab then Enter/Space, dismisses the line;
    /// Escape does not.
    #[test]
    fn the_dismiss_button_dismisses_and_escape_does_not() {
        let button = Some([100.0, 10.0, 80.0, 20.0]);
        let mut s = SystemStatus::default();
        s.show("line");
        // Escape keeps its own meaning: not used here, and the line stays.
        assert!(!s.handle_with(&key(KeyCode::Escape), button));
        assert_eq!(s.line(), Some("line"));
        // A click outside the button is not used.
        s.handle_with(&RawEvent::CursorMoved { x: 50.0, y: 15.0 }, button);
        let click = RawEvent::MouseButton {
            button: MouseButton::Left,
            pressed: true,
        };
        assert!(!s.handle_with(&click, button));
        assert_eq!(s.line(), Some("line"));
        // A click on it dismisses.
        s.handle_with(&RawEvent::CursorMoved { x: 120.0, y: 15.0 }, button);
        assert!(s.handle_with(&click, button));
        assert_eq!(s.line(), None);

        // Enter or Space without the focus is not used; Tab then Enter or Space dismisses.
        for activate in [KeyCode::Enter, KeyCode::Space] {
            s.show("again");
            assert!(!s.handle_with(&key(activate), button));
            assert_eq!(s.line(), Some("again"));
            assert!(s.handle_with(&key(KeyCode::Tab), button));
            assert!(s.focused());
            assert!(!s.handle_with(&key(KeyCode::Escape), button));
            assert_eq!(s.line(), Some("again"), "Escape does not dismiss");
            assert!(s.handle_with(&key(activate), button));
            assert_eq!(s.line(), None);
            assert!(!s.focused());
        }
        // A second Tab takes the focus off the button.
        s.show("third");
        assert!(s.handle_with(&key(KeyCode::Tab), button));
        assert!(s.handle_with(&key(KeyCode::Tab), button));
        assert!(!s.focused());
        // With no line nothing is used.
        s.dismiss();
        assert!(!s.handle_with(&key(KeyCode::Tab), button));
    }

    #[test]
    fn only_system_lines_count() {
        assert!(is_system_line(&ChatLine::system("x")));
        let mut said = ChatLine::system("x");
        said.sender = "Someone".into();
        said.channel = tessera_net::ChatChannel::Say;
        assert!(!is_system_line(&said));
    }
}
