// SPDX-License-Identifier: GPL-3.0-or-later
//! The System status overlay of an official session (`official-entry.md` §8.7 "P10-10J display
//! policy"): the in-world system-message area is the chat window's System line list; when no chat
//! window runs (no game UI attached), the same bounded plain line shows in this overlay, painted by
//! Tessera's own code and labelled "System". A line stays until it is dismissed or the next one
//! replaces it. It takes no command input and shows no markup; it is no plugin-host element.
//!
//! Engineering choices, spec silent: Escape dismisses it (the first Escape, while the chat input is
//! closed, is used for that and goes no further), it sits at the top centre of the viewport, and it
//! shows only in an official session (other sessions keep their chat overlay and HUD notices).

use tessera_render::{Pass, Renderer};

use super::hud::TextPanel;
use crate::net::ChatLine;

/// The overlay's label (§8.7).
pub const LABEL: &str = "System";

/// Most UTF-8 bytes of one shown line (the bound of a notice line, `official-entry.md` §8.2): a
/// longer line is cut at a character boundary (the session drops over-long notices before this).
pub const MAX_LINE_BYTES: usize = 256;

const GOLD: [u8; 3] = [255, 214, 120];

/// The overlay (module docs).
#[derive(Debug, Default)]
pub struct SystemStatus {
    line: Option<String>,
    panel: TextPanel,
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
    }

    /// Dismisses the line; returns whether one showed.
    pub fn dismiss(&mut self) -> bool {
        self.line.take().is_some()
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

    /// Re-rasterises the panel when the line changed.
    pub fn refresh(&mut self, renderer: &mut Renderer<'_>, scale: u32) {
        match self.text() {
            Some(t) => self.panel.set(renderer, vec![(t, GOLD)], scale),
            None => self.panel.release(renderer),
        }
    }

    /// Draws the panel centred on `centre_x` with its top at `top`.
    pub fn draw(&self, pass: &mut Pass, centre_x: f32, top: f32) {
        if self.line.is_none() {
            return;
        }
        if let Some((w, _)) = self.panel.size() {
            self.panel
                .draw(pass, (centre_x - w as f32 / 2.0).max(0.0), top);
        }
    }

    /// Frees the panel's texture.
    pub fn release(&mut self, renderer: &mut Renderer<'_>) {
        self.panel.release(renderer);
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

    #[test]
    fn only_system_lines_count() {
        assert!(is_system_line(&ChatLine::system("x")));
        let mut said = ChatLine::system("x");
        said.sender = "Someone".into();
        said.channel = tessera_net::ChatChannel::Say;
        assert!(!is_system_line(&said));
    }
}
