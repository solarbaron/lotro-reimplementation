// SPDX-License-Identifier: GPL-3.0-or-later
//! The entry line's own rules (`chat-window.md` §6): channel modes and their switch word
//! (§6.5), the key-repeat guard (§6.6), and the log's at-bottom and paging arithmetic
//! (§5.5).

use std::collections::BTreeMap;

use super::command::{CommandSetup, split_names, trim_ws};

/// Chat type 5 `Say`, the initial channel mode (§6.5).
pub const SAY: u32 = 5;
/// Chat type 6 `Tell`.
pub const TELL: u32 = 6;

/// The kind of a mode word (§6.5 table).
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ModeKind {
    /// An ordinary channel word.
    Plain,
    /// The reply word (target from the reply callback).
    Reply,
    /// The tell-again word (target from the tell-again callback).
    TellAgain,
}

/// One row of the mode table: a `Commands` entry's names and its chat type.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ModeRow {
    /// The names of the entry (split by §7.1).
    pub names: Vec<String>,
    /// The chat type.
    pub chat_type: u32,
    /// The word kind.
    pub kind: ModeKind,
}

/// The mode table and its mode words (§6.5).
#[derive(Debug, Clone, Default, PartialEq, Eq)]
pub struct ChannelModes {
    /// The rows, in table order.
    pub rows: Vec<ModeRow>,
    words: BTreeMap<String, usize>,
}

/// Most mode rows (Tessera cap; the table has 31).
pub const MAX_ROWS: usize = 256;

impl ChannelModes {
    /// Builds the table from (`Commands` entry text, chat type, kind) rows: for every name
    /// of every row two mode words, the command prefix followed by the name and the
    /// substitution marker followed by the name. The first row that registers a word keeps
    /// it.
    #[must_use]
    pub fn new(rows: &[(String, u32, ModeKind)], setup: &CommandSetup) -> Self {
        let mut m = Self::default();
        for (entry, chat_type, kind) in rows.iter().take(MAX_ROWS) {
            let names = split_names(entry);
            let idx = m.rows.len();
            for n in &names {
                for pre in [&setup.prefix, &setup.marker] {
                    m.words.entry(format!("{pre}{n}")).or_insert(idx);
                }
            }
            m.rows.push(ModeRow {
                names,
                chat_type: *chat_type,
                kind: *kind,
            });
        }
        m
    }

    /// The switch test of §6.5 steps 1–2: the entry text trimmed of whitespace, stopped
    /// if it contains a space, looked up case-sensitively among the mode words.
    #[must_use]
    pub fn switch_word(&self, entry_text: &str) -> Option<&ModeRow> {
        let t = trim_ws(entry_text);
        if t.contains(' ') {
            return None;
        }
        self.words.get(t).and_then(|i| self.rows.get(*i))
    }

    /// The command word the submit uses for a mode (§6.2 step 2), or empty when the table
    /// has none, so that §7.2 uses the default word. The map chat type → command word is
    /// OPEN (Q12); Tessera's choice, recorded in the implementation note under
    /// `chat-window.md` §6.5, is the first name of the first plain row of that chat type.
    #[must_use]
    pub fn command_word(&self, chat_type: u32) -> &str {
        self.rows
            .iter()
            .filter(|r| r.chat_type == chat_type && r.kind == ModeKind::Plain)
            .find_map(|r| r.names.first())
            .map_or("", String::as_str)
    }
}

/// The key-repeat guard's verdict (§6.6).
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum GuardVerdict {
    /// Accept the character.
    Accept,
    /// Swallow it: release the entry's focus, delete this many characters before the
    /// caret and play UI sound 0x50 (§6.6 step 3).
    Trip {
        /// Characters to delete.
        delete: u32,
    },
}

/// The UI sound the guard plays (§6.6 step 3; which sound record it is, Q13, is OPEN). A UI
/// sound ID, not a DID: [`super::window::Chat::text_input`] reports a trip and the client
/// plays it.
pub const GUARD_SOUND: u32 = 0x50;

/// The key-repeat guard (§6.6), active while `GOP_ChatKeyRepeatProtection` is true.
#[derive(Debug, Clone, Copy, Default, PartialEq)]
pub struct KeyRepeatGuard {
    prev: Option<char>,
    count: u32,
    start: Option<f64>,
}

impl KeyRepeatGuard {
    /// Checks one typed character at time `now` (seconds). Characters below U+0020 and
    /// U+007F are not checked.
    pub fn check(&mut self, ch: char, now: f64) -> GuardVerdict {
        if ch < ' ' || ch == '\u{7F}' {
            return GuardVerdict::Accept;
        }
        if self.prev != Some(ch) {
            self.prev = Some(ch);
            self.count = 0;
            self.start = None;
            return GuardVerdict::Accept;
        }
        self.count = self.count.saturating_add(1);
        let Some(start) = self.start else {
            self.start = Some(now);
            return GuardVerdict::Accept;
        };
        if now - start < 1.0 && self.count > 10 {
            let delete = self.count;
            self.prev = None;
            self.start = Some(now);
            return GuardVerdict::Trip { delete };
        }
        GuardVerdict::Accept
    }
}

/// The at-bottom test of §5.5: offset *o*, visible height *v*, content height *c*, and the
/// last line's height *h* (0 when the log has at most one line).
#[must_use]
pub fn at_bottom(o: i32, v: i32, c: i32, h: i32) -> bool {
    i64::from(o) + i64::from(v) >= i64::from(c) - i64::from(h)
}

/// The page targets of §5.5, after the list box's clamp to [0, max(*c* − *v*, 0)]:
/// (`CursorPreviousPage`, `CursorNextPage`).
#[must_use]
pub fn page_targets(o: i32, v: i32, c: i32) -> (i32, i32) {
    let hi = c.saturating_sub(v).max(0);
    let clamp = |t: i32| t.min(hi).max(0);
    let prev = clamp(o.saturating_sub(v).max(0));
    let next = clamp(o.saturating_add(v).min(c.saturating_sub(v)));
    (prev, next)
}
