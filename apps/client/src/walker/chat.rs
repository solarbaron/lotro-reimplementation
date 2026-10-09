// SPDX-License-Identifier: GPL-3.0-or-later
//! A minimal chat overlay for the walker (M7): the last [`CHAT_LINES`] lines received,
//! shown for [`LINE_TTL`] (all of them while typing), and a one-line input. Enter opens
//! the input, Enter submits the line, Escape cancels. Submitted lines go to the client's
//! command dispatcher ([`dispatch`], decision 0020): plain text is said on the Say channel,
//! `/play` and the other client commands run locally in every session, and an unknown
//! command posts an error line and sends nothing. M5 builds the real chat window.
//!
//! Chat text is hostile: it is shown as plain text (`tessera-protocol.md` §10.5), control
//! characters become spaces and lines are capped by the HUD rasteriser
//! (`hud::rasterize`). Stored lines and the input are bounded. Received lines are
//! wrapped, and the input shows its tail, to fit the viewport's width.
//!
//! System lines (no sender) reach the screen one way only: in a remote session (`--server`)
//! through this overlay ([`ChatOverlay::receive`] reports the HUD notice that repeats one,
//! for the walker to clear); in the sandbox through the HUD notice alone, so the sandbox's
//! HUD and screenshots stay as they were (`walker.md` §10.8 describes the overlay for
//! `--server` sessions).

use std::collections::VecDeque;
use std::time::{Duration, Instant};

use tessera_input::{KeyCode, RawEvent};
use tessera_net::ChatChannel;
use tessera_net::model::MAX_CHAT_LEN;
use tessera_render::{Pass, Renderer};

use super::WalkerSession;
use super::controls::Controls;
use super::hud::TextPanel;
use crate::commands::{LineKind, LocalLine};
use crate::net::ChatLine;
use crate::plugins::CommandSink;

/// Lines shown (the newest).
pub const CHAT_LINES: usize = 8;
/// How long a line stays on screen while the input is closed.
pub const LINE_TTL: Duration = Duration::from_secs(45);
/// Most characters kept of one received line (the HUD shows fewer).
pub const MAX_STORED_CHARS: usize = 512;
/// Most lines waiting to be sent (Enter pressed faster than frames run).
pub const MAX_OUTGOING: usize = 4;
/// Most screen rows of received lines (after wrapping; the newest are kept).
pub const MAX_ROWS: usize = 16;
/// Pixels between the panel and the viewport's left and right edges.
pub const MARGIN: u32 = 10;

const WHITE: [u8; 3] = [240, 240, 235];
const GOLD: [u8; 3] = [255, 214, 120];
const TELL: [u8; 3] = [220, 160, 255];
const YELL: [u8; 3] = [255, 170, 140];
const INPUT: [u8; 3] = [150, 230, 255];
const ERROR: [u8; 3] = [255, 120, 110];

/// What [`ChatOverlay::handle`] did with an input event.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Routed {
    /// Not for the chat: pass it on to the controls.
    Pass,
    /// The input just opened: release everything the controls hold (the keys typed next
    /// never reach them).
    Opened,
    /// Used by the chat.
    Consumed,
}

/// Forwards one input event: to the chat first, then (when the chat does not use it) to
/// the controls. Opening the input releases everything the controls hold, so a movement
/// key held while pressing Enter does not keep the avatar walking while the user types.
pub(crate) fn route(chat: &mut ChatOverlay, controls: &mut Controls, event: &RawEvent) {
    match chat.handle(event) {
        Routed::Pass => controls.handle(event),
        Routed::Opened => controls.handle(&RawEvent::FocusLost),
        Routed::Consumed => {}
    }
}

/// Feeds the lines submitted since the last call to the command dispatcher `commands`, in
/// order, with the world `session` for channel commands, then shows the lines the client
/// posted (command replies, player music, plugin output). A rejected line is not sent and
/// does not stay queued (decision 0020 item 4).
pub fn dispatch(
    chat: &mut ChatOverlay,
    commands: &mut dyn CommandSink,
    session: &mut Option<&mut dyn WalkerSession>,
    now: Instant,
) {
    while let Some(line) = chat.take_outgoing() {
        commands.submit_chat(&line, crate::commands::shorten(session.as_deref_mut()), now);
    }
    let mut local = Vec::new();
    commands.take_lines(&mut local);
    for line in &local {
        chat.push_local_line(line, now);
    }
}

/// The chat overlay (module docs).
#[derive(Debug, Default)]
pub struct ChatOverlay {
    lines: VecDeque<(String, [u8; 3], Instant)>,
    input: Option<String>,
    outgoing: VecDeque<String>,
    panel: TextPanel,
}

/// One received line as shown: `Sender: text` (Say), `Sender yells: text`, `Sender text`
/// (Emote), `[Tell peer] Sender: text`, or the bare text of a system line.
#[must_use]
pub fn format_line(line: &ChatLine) -> (String, [u8; 3]) {
    let s = &line.sender;
    match &line.channel {
        ChatChannel::System => (line.text.clone(), GOLD),
        _ if s.is_empty() => (line.text.clone(), GOLD),
        ChatChannel::Say => (format!("{s}: {}", line.text), WHITE),
        ChatChannel::Yell => (format!("{s} yells: {}", line.text), YELL),
        ChatChannel::Emote => (format!("{s} {}", line.text), WHITE),
        ChatChannel::Tell { peer } => (format!("[Tell {peer}] {s}: {}", line.text), TELL),
        _ => (format!("{s}: {}", line.text), WHITE),
    }
}

fn is_enter(key: KeyCode) -> bool {
    matches!(key, KeyCode::Enter | KeyCode::NumpadEnter)
}

impl ChatOverlay {
    /// Whether the input line is open.
    #[must_use]
    pub fn is_open(&self) -> bool {
        self.input.is_some()
    }

    /// The text typed so far (`None` while closed).
    #[must_use]
    pub fn input(&self) -> Option<&str> {
        self.input.as_deref()
    }

    /// Routes one input event (Enter opens and sends, Escape cancels, Backspace deletes,
    /// typed characters go into the line). While the input is open every key and text
    /// event is used here; mouse and focus events always pass.
    pub fn handle(&mut self, event: &RawEvent) -> Routed {
        match (event, self.input.as_mut()) {
            (
                RawEvent::Key {
                    key,
                    pressed: true,
                    repeat: false,
                },
                None,
            ) if is_enter(*key) => {
                self.input = Some(String::new());
                Routed::Opened
            }
            (_, None) => Routed::Pass,
            (
                RawEvent::Key {
                    key, pressed: true, ..
                },
                Some(text),
            ) => {
                match *key {
                    k if is_enter(k) => {
                        let line = text.trim().to_owned();
                        self.input = None;
                        if !line.is_empty() {
                            if self.outgoing.len() >= MAX_OUTGOING {
                                self.outgoing.pop_front();
                            }
                            self.outgoing.push_back(line);
                        }
                    }
                    KeyCode::Escape => self.input = None,
                    KeyCode::Backspace => {
                        text.pop();
                    }
                    _ => {}
                }
                Routed::Consumed
            }
            (RawEvent::Text(c), Some(text)) => {
                if !c.is_control() && text.len() + c.len_utf8() <= MAX_CHAT_LEN {
                    text.push(*c);
                }
                Routed::Consumed
            }
            (RawEvent::Key { .. }, Some(_)) => Routed::Consumed,
            (_, Some(_)) => Routed::Pass,
        }
    }

    /// The next line to send, oldest first.
    pub fn take_outgoing(&mut self) -> Option<String> {
        self.outgoing.pop_front()
    }

    /// Takes the lines received this frame. With `show_system` every line is shown: in a
    /// remote session, and in a sandbox session playing a scenario (the scenario owns the
    /// chat, `scenario-feed.md` §3.2). Otherwise (the plain sandbox) only lines with a
    /// sender are shown; system lines stay HUD notices there. Returns whether `notice`, the
    /// HUD notice of this frame, repeats a system line shown here, in which case the caller
    /// clears it (one display path per line).
    pub fn receive(
        &mut self,
        lines: &[ChatLine],
        show_system: bool,
        notice: Option<&str>,
        now: Instant,
    ) -> bool {
        let mut repeated = false;
        for line in lines {
            let system = line.sender.is_empty() || line.channel == ChatChannel::System;
            if system && !show_system {
                continue;
            }
            repeated |= system && notice == Some(line.text.as_str());
            self.push(line, now);
        }
        repeated
    }

    /// Shows a line of the client's own (a chat line that could not be sent) as a system
    /// line.
    pub fn push_local(&mut self, text: &str, now: Instant) {
        let line = ChatLine::system(text);
        self.push(&line, now);
    }

    /// Shows a line the client posted: type 1 `Error` lines in red, type 8 `Log` lines
    /// as system lines.
    pub fn push_local_line(&mut self, line: &LocalLine, now: Instant) {
        let color = match line.kind {
            LineKind::Error => ERROR,
            LineKind::Log => GOLD,
        };
        self.push_text(line.text.clone(), color, now);
    }

    /// Adds a received line (the oldest goes beyond [`CHAT_LINES`]).
    pub fn push(&mut self, line: &ChatLine, now: Instant) {
        let (text, color) = format_line(line);
        self.push_text(text, color, now);
    }

    fn push_text(&mut self, mut text: String, color: [u8; 3], now: Instant) {
        if let Some((cut, _)) = text.char_indices().nth(MAX_STORED_CHARS) {
            text.truncate(cut);
        }
        if self.lines.len() >= CHAT_LINES {
            self.lines.pop_front();
        }
        self.lines.push_back((text, color, now));
    }

    /// The rows to show at `now` in a panel `cols` text columns wide: recent received
    /// lines (all of them while typing) wrapped at `cols` (at most [`MAX_ROWS`] rows, the
    /// newest), then the input line with a cursor (its tail, so the cursor stays visible).
    #[must_use]
    pub fn shown(&self, now: Instant, cols: usize) -> Vec<(String, [u8; 3])> {
        let cols = cols.clamp(4, super::parse::MAX_HUD_LINE);
        let open = self.input.is_some();
        let mut rows: VecDeque<(String, [u8; 3])> = VecDeque::new();
        for (t, c, _) in self
            .lines
            .iter()
            .filter(|(_, _, at)| open || now.saturating_duration_since(*at) < LINE_TTL)
        {
            let chars: Vec<char> = t.chars().collect();
            for chunk in chars.chunks(cols) {
                if rows.len() >= MAX_ROWS {
                    rows.pop_front();
                }
                rows.push_back((chunk.iter().collect(), *c));
            }
        }
        let mut out: Vec<(String, [u8; 3])> = rows.into();
        if let Some(text) = &self.input {
            let max = cols - 3;
            let n = text.chars().count();
            let tail: String = text.chars().skip(n.saturating_sub(max)).collect();
            out.push((format!("> {tail}_"), INPUT));
        }
        out
    }

    /// Re-rasterises the panel when what is shown changed, laid out for a viewport
    /// `width` pixels wide (the panel keeps [`MARGIN`] from both edges).
    pub fn refresh(&mut self, renderer: &mut Renderer<'_>, now: Instant, scale: u32, width: u32) {
        let cols = super::hud::columns(width.saturating_sub(2 * MARGIN), scale);
        let lines = self.shown(now, cols);
        if lines.is_empty() {
            self.panel.release(renderer);
        } else {
            self.panel.set(renderer, lines, scale);
        }
    }

    /// Draws the panel with its bottom-left corner at (`x`, `bottom`).
    pub fn draw(&self, pass: &mut Pass, x: f32, bottom: f32) {
        if let Some((_, h)) = self.panel.size() {
            self.panel.draw(pass, x, (bottom - h as f32).max(0.0));
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

    fn key(key: KeyCode) -> RawEvent {
        RawEvent::Key {
            key,
            pressed: true,
            repeat: false,
        }
    }

    fn line(sender: &str, text: &str, channel: ChatChannel) -> ChatLine {
        ChatLine::from_channel(&channel, Some(sender), text)
    }

    #[test]
    fn enter_opens_types_and_sends() {
        let mut c = ChatOverlay::default();
        assert_eq!(
            c.handle(&RawEvent::Text('w')),
            Routed::Pass,
            "closed: text passes"
        );
        assert_eq!(c.handle(&key(KeyCode::W)), Routed::Pass);
        assert_eq!(c.handle(&key(KeyCode::Enter)), Routed::Opened);
        assert!(c.is_open());
        for ch in "hi there!".chars() {
            assert_eq!(c.handle(&RawEvent::Text(ch)), Routed::Consumed);
        }
        assert_eq!(c.handle(&RawEvent::Text('\u{7}')), Routed::Consumed);
        assert_eq!(
            c.handle(&key(KeyCode::W)),
            Routed::Consumed,
            "keys stay here"
        );
        assert_eq!(
            c.handle(&RawEvent::MouseMotion { dx: 1.0, dy: 0.0 }),
            Routed::Pass
        );
        assert_eq!(c.handle(&key(KeyCode::Backspace)), Routed::Consumed);
        assert_eq!(c.input(), Some("hi there"));
        assert_eq!(c.handle(&key(KeyCode::NumpadEnter)), Routed::Consumed);
        assert!(!c.is_open());
        assert_eq!(c.take_outgoing().as_deref(), Some("hi there"));
        assert_eq!(c.take_outgoing(), None);
    }

    #[test]
    fn escape_cancels_and_empty_lines_are_not_sent() {
        let mut c = ChatOverlay::default();
        c.handle(&key(KeyCode::Enter));
        c.handle(&RawEvent::Text('x'));
        assert_eq!(c.handle(&key(KeyCode::Escape)), Routed::Consumed);
        assert!(!c.is_open());
        assert_eq!(c.take_outgoing(), None);
        c.handle(&key(KeyCode::Enter));
        c.handle(&RawEvent::Text(' '));
        c.handle(&key(KeyCode::Enter));
        assert_eq!(c.take_outgoing(), None);
        // An auto-repeated Enter does not open the input.
        assert_eq!(
            c.handle(&RawEvent::Key {
                key: KeyCode::Enter,
                pressed: true,
                repeat: true
            }),
            Routed::Pass
        );
        // Escape while closed is the controls' (menu).
        assert_eq!(c.handle(&key(KeyCode::Escape)), Routed::Pass);
    }

    #[test]
    fn input_and_queues_are_bounded() {
        let mut c = ChatOverlay::default();
        c.handle(&key(KeyCode::Enter));
        for _ in 0..(MAX_CHAT_LEN + 50) {
            c.handle(&RawEvent::Text('a'));
        }
        assert_eq!(c.input().map(str::len), Some(MAX_CHAT_LEN));
        let shown = c.shown(Instant::now(), usize::MAX);
        assert!(shown.last().unwrap().0.chars().count() <= super::super::parse::MAX_HUD_LINE);
        c.handle(&key(KeyCode::Enter));
        for i in 0..10 {
            c.handle(&key(KeyCode::Enter));
            c.handle(&RawEvent::Text(char::from(b'0' + i)));
            c.handle(&key(KeyCode::Enter));
        }
        let mut sent = Vec::new();
        while let Some(l) = c.take_outgoing() {
            sent.push(l);
        }
        assert_eq!(sent, vec!["6", "7", "8", "9"]);
    }

    #[test]
    fn lines_are_formatted_capped_and_expire() {
        let mut c = ChatOverlay::default();
        let t0 = Instant::now();
        c.push(&line("Bram", "hello", ChatChannel::Say), t0);
        c.push(&line("", "Welcome", ChatChannel::System), t0);
        c.push(&line("Bram", "HEY", ChatChannel::Yell), t0);
        c.push(&line("Bram", "waves", ChatChannel::Emote), t0);
        c.push(
            &line(
                "Bram",
                "psst",
                ChatChannel::Tell {
                    peer: "Aldith".into(),
                },
            ),
            t0,
        );
        let shown = c.shown(t0, 160);
        let texts: Vec<&str> = shown.iter().map(|(t, _)| t.as_str()).collect();
        assert_eq!(
            texts,
            [
                "Bram: hello",
                "Welcome",
                "Bram yells: HEY",
                "Bram waves",
                "[Tell Aldith] Bram: psst"
            ]
        );
        assert_eq!(shown[1].1, GOLD);
        for i in 0..20 {
            c.push(&line("x", &"y".repeat(i * 100), ChatChannel::Say), t0);
        }
        assert_eq!(
            c.shown(t0, 160).len(),
            MAX_ROWS,
            "wrapped, newest rows kept"
        );
        assert!(
            c.lines
                .iter()
                .all(|(t, _, _)| t.chars().count() <= MAX_STORED_CHARS)
        );
        let later = t0 + LINE_TTL + Duration::from_secs(1);
        assert!(c.shown(later, 160).is_empty(), "expired");
        c.handle(&key(KeyCode::Enter));
        assert_eq!(c.shown(later, 160).len(), MAX_ROWS + 1, "all while typing");
        assert_eq!(c.lines.len(), CHAT_LINES);
    }

    #[test]
    fn rows_and_the_input_fit_the_viewport() {
        let t0 = Instant::now();
        let mut c = ChatOverlay::default();
        // 1920 px wide at HUD scale 2: 117 columns (finding: the cursor was off-screen).
        let cols = super::super::hud::columns(1920 - 2 * MARGIN, 2);
        assert_eq!(cols, 117);
        c.push(&line("Bram", &"y".repeat(300), ChatChannel::Say), t0);
        c.handle(&key(KeyCode::Enter));
        for _ in 0..500 {
            c.handle(&RawEvent::Text('a'));
        }
        c.handle(&RawEvent::Text('Z'));
        let shown = c.shown(t0, cols);
        assert!(
            shown.iter().all(|(t, _)| t.chars().count() <= cols),
            "{shown:?}"
        );
        let joined: String = shown[..shown.len() - 1]
            .iter()
            .map(|(t, _)| t.as_str())
            .collect();
        assert_eq!(
            joined,
            format!("Bram: {}", "y".repeat(300)),
            "wrapped, nothing lost"
        );
        let input = &shown.last().unwrap().0;
        assert!(
            input.ends_with("aZ_"),
            "newest text and cursor visible: {input}"
        );
        assert_eq!(input.chars().count(), cols);
        // Tiny viewports still show the cursor.
        assert!(c.shown(t0, 0).last().unwrap().0.ends_with("Z_"));
        // The panel the rasteriser makes fits the width.
        let panel = super::super::hud::rasterize(&shown, 2);
        assert!(panel.width + 2 * MARGIN <= 1920, "{} px", panel.width);
    }

    #[test]
    fn system_lines_take_one_path() {
        let t0 = Instant::now();
        let welcome = line("", "Welcome to the sandbox.", ChatChannel::System);
        let said = line("Bram", "hi", ChatChannel::Say);
        // Sandbox: system lines stay HUD notices (nothing new on its screenshots); player
        // chat is shown.
        let mut c = ChatOverlay::default();
        let repeated = c.receive(
            &[welcome.clone(), said.clone()],
            false,
            Some("Welcome to the sandbox."),
            t0,
        );
        assert!(!repeated, "the notice keeps the system line");
        let texts: Vec<String> = c.shown(t0, 160).into_iter().map(|(t, _)| t).collect();
        assert_eq!(texts, ["Bram: hi"]);
        // Remote: the overlay shows the system line and reports the duplicate notice.
        let mut c = ChatOverlay::default();
        assert!(c.receive(
            &[said.clone(), welcome.clone()],
            true,
            Some("Welcome to the sandbox."),
            t0
        ));
        assert_eq!(c.shown(t0, 160).len(), 2);
        assert!(!c.receive(&[welcome], true, Some("another notice"), t0));
        assert!(
            !c.receive(&[said], true, Some("hi"), t0),
            "only system lines"
        );
        c.push_local("Not in the world: chat not sent", t0);
        assert_eq!(
            c.shown(t0, 160).last().map(|(t, col)| (t.as_str(), *col)),
            Some(("Not in the world: chat not sent", GOLD))
        );
    }

    #[test]
    fn opening_the_chat_releases_held_movement_keys() {
        use super::super::controls::{Controls, MoveMode};
        use tessera_world::WorldPos;
        let keymap = tessera_input::default_keymap();
        let flat = |_: f64, _: f64| Some(100.0_f32);
        let mut controls = Controls::new(&keymap, MoveMode::Walk, 40.0);
        let start = WorldPos::new(1, 20_000.0, 20_000.0, 100.0);
        controls.place(&start, 0.0, 30.0, -0.25);
        let mut chat = ChatOverlay::default();
        route(&mut chat, &mut controls, &key(KeyCode::W));
        for _ in 0..10 {
            let _ = controls.frame(1.0 / 60.0, &flat, None);
        }
        let walked = controls.focus().x;
        assert!(walked > start.x, "W walks while the chat is closed");
        route(&mut chat, &mut controls, &key(KeyCode::Enter));
        assert!(chat.is_open());
        // Typing W (and the W release) stays in the chat.
        route(&mut chat, &mut controls, &key(KeyCode::W));
        route(&mut chat, &mut controls, &RawEvent::Text('w'));
        // The avatar slows to a stop (no key held any more), then stays.
        for _ in 0..60 {
            let _ = controls.frame(1.0 / 60.0, &flat, None);
        }
        let stopped = controls.focus().x;
        assert!(
            stopped - walked < 1.0,
            "stopping, not walking on: {walked} -> {stopped}"
        );
        for _ in 0..60 {
            let _ = controls.frame(1.0 / 60.0, &flat, None);
        }
        let x = controls.focus().x;
        assert!(
            (x - stopped).abs() < 1e-6,
            "the avatar stays while typing: {stopped} -> {x}"
        );
        assert_eq!(chat.input(), Some("w"));
    }
}
