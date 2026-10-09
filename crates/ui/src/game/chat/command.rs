// SPDX-License-Identifier: GPL-3.0-or-later
//! The chat window's side of the command line (`chat-window.md` §6.2 submit, §6.3, §7.1,
//! §7.2): the whitespace set, the name-list splitting rule (the mode words of §6.5 use
//! it), the `CommandSetup` texts the window needs, and the hand-over of a submitted entry
//! line.
//!
//! The registry (§7.1), the line pipeline (§7.2–§7.4), `IsCommand` (§8.2) and the history
//! (§6.3) are **not** here: the client owns one command dispatcher that every source of
//! typed command text feeds (decision 0020 item 1). The chat window hands each submitted
//! line out as an [`EntryLine`] and browses the dispatcher's history through
//! [`EntryHistory`]; it never parses a command line itself.

/// The whitespace set of §7.2: space, line feed, carriage return, tab, form feed.
#[must_use]
pub fn is_ws(c: char) -> bool {
    matches!(c, ' ' | '\n' | '\r' | '\t' | '\u{000C}')
}

/// Trims the whitespace set at both ends.
#[must_use]
pub fn trim_ws(s: &str) -> &str {
    s.trim_matches(is_ws)
}

/// Splits a string-table name list (§7.1, §8.1): at every `;`, `:` and `,`, each piece
/// trimmed of whitespace, empty pieces ignored.
#[must_use]
pub fn split_names(entry: &str) -> Vec<String> {
    entry
        .split([';', ':', ','])
        .map(trim_ws)
        .filter(|s| !s.is_empty())
        .map(str::to_string)
        .collect()
}

/// The `CommandSetup` texts the window uses (§6.2 `START_COMMAND`, §6.5 mode words; read
/// at run time from the install by the client).
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct CommandSetup {
    /// The command prefix (token 0x0DA28738).
    pub prefix: String,
    /// The substitution marker (token 0x02409858).
    pub marker: String,
    /// The default channel word (token 0x04DA4989).
    pub default_word: String,
}

impl Default for CommandSetup {
    /// Syntax defaults for synthetic use: prefix U+002F, marker U+003B, and an invented
    /// default word (the real word is game text read from the install).
    fn default() -> Self {
        Self {
            prefix: "/".to_string(),
            marker: ";".to_string(),
            default_word: "say".to_string(),
        }
    }
}

/// A line submitted in the entry (§6.2 submit steps 1–3): what the window hands to the
/// client's command dispatcher, which runs §7.2 with it (decision 0020 items 1 and 2) and
/// posts the unknown-command line of §7.6 itself.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct EntryLine {
    /// The entry text as typed (not trimmed; never empty).
    pub text: String,
    /// The channel mode (a chat type, §6.5) at the submit.
    pub mode: u32,
    /// The mode's command word *C* (§6.2 submit step 2); empty when the mode table has
    /// none, and then §7.2 uses the default word.
    pub word: String,
    /// The tell target *G* for the `Tell` mode, else empty.
    pub target: String,
}

/// The one history of the client (§6.3), which belongs to the command registry and so to
/// the client's dispatcher; the entry browses it with line up and down.
pub trait EntryHistory {
    /// Older (`CursorPreviousLine` without Shift or Ctrl): the entry text to show, if any.
    fn older(&mut self) -> Option<String>;
    /// Newer (`CursorNextLine` without Shift or Ctrl): the entry text to show; `None`
    /// leaves the entry as it is.
    fn newer(&mut self) -> Option<String>;
}

/// No history (tools and tests without a dispatcher).
#[derive(Debug, Clone, Copy, Default)]
pub struct NoHistory;

impl EntryHistory for NoHistory {
    fn older(&mut self) -> Option<String> {
        None
    }

    fn newer(&mut self) -> Option<String> {
        None
    }
}
