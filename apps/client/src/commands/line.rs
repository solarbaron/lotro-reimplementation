// SPDX-License-Identifier: GPL-3.0-or-later
//! The command-line pipeline of `chat-window.md` §7.2–§7.4 (whitespace, trimming, the
//! prefix test, the tokenizer with both of its quirks, the command word and the
//! arguments) and the history of §6.3, as pure functions of text.
//!
//! The registry lookup and the handlers are not here ([`super::Dispatcher`]); this module
//! only turns a submitted text into an [`Invocation`] (or "nothing to run"). It depends on
//! `std` only, so the fuzz targets `client_command_line` and `client_player_music` include
//! it by path. Input is bounded by [`MAX_LINE_LEN`] and nothing here panics.

use std::collections::VecDeque;

/// Longest text the pipeline accepts, bytes (decision 0020 item 1: input is bounded by
/// the entry's cap; the walker overlay caps at the protocol's chat length, the M5 entry at
/// 384 characters, both below this). Longer texts are refused ([`LineError::TooLong`]).
pub const MAX_LINE_LEN: usize = 4096;
/// History capacity (`chat-window.md` §6.3): the list holds at most one less.
pub const HISTORY_CAPACITY: usize = 20;
/// The command prefix when the install's `CommandSetup` table cannot be read (decision
/// 0020 item 1).
pub const DEFAULT_PREFIX: &str = "/";

const QUOTE: u16 = 0x22;

/// The whitespace set of `chat-window.md` §7.2: U+0020, U+000A, U+000D, U+0009, U+000C.
#[must_use]
pub fn is_whitespace(c: char) -> bool {
    matches!(c, ' ' | '\n' | '\r' | '\t' | '\u{c}')
}

fn is_whitespace_unit(u: u16) -> bool {
    matches!(u, 0x20 | 0x0A | 0x0D | 0x09 | 0x0C)
}

/// Trims the §7.2 whitespace set at both ends.
#[must_use]
pub fn trim(s: &str) -> &str {
    s.trim_matches(is_whitespace)
}

fn trim_units(mut s: &[u16]) -> &[u16] {
    while let Some((first, rest)) = s.split_first() {
        if !is_whitespace_unit(*first) {
            break;
        }
        s = rest;
    }
    while let Some((last, rest)) = s.split_last() {
        if !is_whitespace_unit(*last) {
            break;
        }
        s = rest;
    }
    s
}

/// Splits `text` into tokens by the rule of `chat-window.md` §7.3, with positions in
/// UTF-16 code units. Never yields an empty token. Quirk 1: one character right after a
/// closing quote at the very end is lost; quirk 2: an unmatched quote re-emits the
/// previous token.
#[must_use]
pub fn tokenize(text: &str) -> Vec<String> {
    let units: Vec<u16> = text.encode_utf16().collect();
    let n = units.len();
    let mut out = Vec::new();
    let mut current: Vec<u16> = Vec::new();
    let mut start = 0usize;
    let mut i = 0usize;
    while let Some(&u) = units.get(i) {
        if u == QUOTE {
            // §7.3 sets the token start to i + 1 here; the quoted token is taken from i + 1
            // directly and the start is set again below before any use.
            let rest = units.get(i + 1..).unwrap_or(&[]);
            match rest.iter().position(|&c| c == QUOTE) {
                Some(off) => {
                    let j = i + 1 + off;
                    current = units.get(i + 1..j).unwrap_or(&[]).to_vec();
                    i = j + 1;
                }
                // The quote is ignored; the current token keeps the previous one.
                None => i += 1,
            }
        } else {
            let rest = units.get(i..).unwrap_or(&[]);
            let e = rest
                .iter()
                .position(|&c| is_whitespace_unit(c))
                .map_or(n, |off| i + off);
            current = trim_units(units.get(start..e).unwrap_or(&[])).to_vec();
            i = e;
        }
        if !current.is_empty() {
            out.push(String::from_utf16_lossy(&current));
        }
        start = i;
        i = (i + 1).min(n);
    }
    out
}

/// What [`execute_parts`] and [`prepare`] give for a line that runs something: the
/// command word, the arguments and the token list of `chat-window.md` §7.4.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Invocation {
    /// The command word *W*: the first token, as typed (without quotes if it was quoted).
    pub word: String,
    /// The arguments *A* (§7.4 step 5): never start or end with §7.2 whitespace.
    pub arguments: String,
    /// Every token, the command word first.
    pub tokens: Vec<String>,
    /// Whether the line was plain text run as the channel command (§7.2 step 5).
    pub plain: bool,
}

impl Invocation {
    /// The tokens after the command word.
    #[must_use]
    pub fn args_tokens(&self) -> &[String] {
        self.tokens.get(1..).unwrap_or(&[])
    }
}

/// Why a text was refused before it ran.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum LineError {
    /// Longer than [`MAX_LINE_LEN`] bytes.
    TooLong,
}

/// Steps 2–5 of "execute" (`chat-window.md` §7.4): `None` when there is nothing to run
/// (result 5: empty after trimming, or no tokens; an empty text, result 2, gives `None`
/// too).
#[must_use]
pub fn execute_parts(text: &str) -> Option<Invocation> {
    let trimmed = trim(text);
    if trimmed.is_empty() {
        return None;
    }
    let tokens = tokenize(trimmed);
    let word = tokens.first()?.clone();
    let arguments = trim(trimmed.trim_start_matches(|c: char| word.contains(c))).to_owned();
    Some(Invocation {
        word,
        arguments,
        tokens,
        plain: false,
    })
}

/// Steps 2–5 of running a line (`chat-window.md` §7.2) for a non-empty `text`: no
/// substitution words exist (§7.5 OPEN Q14, §10 item 4), so step 2 changes nothing.
/// `prefix` is the command prefix *P*; `channel_word` is the channel mode's command word
/// *C* (already defaulted when the mode has none) and `target` the tell target *G*.
/// `Ok(None)`: nothing to run (result 5).
///
/// # Errors
///
/// [`LineError::TooLong`] beyond [`MAX_LINE_LEN`] bytes.
pub fn prepare(
    text: &str,
    prefix: &str,
    channel_word: &str,
    target: &str,
) -> Result<Option<Invocation>, LineError> {
    if text.len() > MAX_LINE_LEN {
        return Err(LineError::TooLong);
    }
    let trimmed = trim(text);
    if trimmed.is_empty() {
        return Ok(None);
    }
    if !prefix.is_empty() && trimmed.starts_with(prefix) {
        let rest = trimmed.trim_start_matches(|c: char| prefix.contains(c));
        if rest.is_empty() {
            return Ok(None);
        }
        return Ok(execute_parts(rest));
    }
    let line = format!("{channel_word} {target} {trimmed}");
    Ok(execute_parts(&line).map(|mut inv| {
        inv.plain = true;
        inv
    }))
}

/// The one history list of `chat-window.md` §6.3: at most [`HISTORY_CAPACITY`] − 1 lines,
/// newest first, no two identical, with a browsing index.
#[derive(Debug, Clone, Default)]
pub struct History {
    lines: VecDeque<String>,
    /// −1: not browsing.
    index: isize,
}

impl History {
    /// An empty history, not browsing.
    #[must_use]
    pub fn new() -> Self {
        Self {
            lines: VecDeque::new(),
            index: -1,
        }
    }

    /// Adds a submitted text (§7.2 step 1, already known to be non-empty): an identical
    /// entry is removed, the text becomes the newest, the list is trimmed and browsing
    /// stops.
    pub fn add(&mut self, text: &str) {
        if text.is_empty() || text.len() > MAX_LINE_LEN {
            return;
        }
        self.lines.retain(|l| l != text);
        self.lines.push_front(text.to_owned());
        while self.lines.len() >= HISTORY_CAPACITY {
            self.lines.pop_back();
        }
        self.index = -1;
    }

    /// The lines, newest first.
    pub fn lines(&self) -> impl Iterator<Item = &str> {
        self.lines.iter().map(String::as_str)
    }

    /// The browsing index (−1: not browsing).
    #[must_use]
    pub fn index(&self) -> isize {
        self.index
    }

    /// Line up without Shift or Ctrl: the entry text to show, if any.
    pub fn older(&mut self) -> Option<&str> {
        let count = isize::try_from(self.lines.len()).unwrap_or(isize::MAX);
        self.index = (self.index + 1).min(count - 1);
        let k = usize::try_from(self.index).ok()?;
        self.lines.get(k).map(String::as_str)
    }

    /// Line down without Shift or Ctrl: the entry text to show; `None` leaves the entry as
    /// it is.
    pub fn newer(&mut self) -> Option<&str> {
        self.index -= 1;
        if self.index < 0 {
            self.index = -1;
            return None;
        }
        let k = usize::try_from(self.index).ok()?;
        self.lines.get(k).map(String::as_str)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn toks(s: &str) -> Vec<String> {
        tokenize(s)
    }

    #[test]
    fn tokenizer_worked_examples() {
        // chat-window.md §11.3.
        assert_eq!(toks("a   b"), ["a", "b"]);
        assert_eq!(toks("x \"a b\"c"), ["x", "a b"]);
        assert_eq!(toks("x \"a b\"cd"), ["x", "a b", "cd"]);
        assert_eq!(toks("go \"hi there"), ["go", "go", "hi", "there"]);
        assert_eq!(toks("\" a \" b"), [" a ", "b"]);
        assert!(toks("\"\"").is_empty());
        // §7.3 consequences.
        assert_eq!(toks("ab\"c"), ["ab\"c"]);
        assert_eq!(toks("\"a\"\"b\""), ["a", "\"b\""]);
        assert_eq!(toks("\"hi"), ["hi"]);
        assert_eq!(toks("  lead\ttrail \u{c}"), ["lead", "trail"]);
        assert!(toks("").is_empty());
        assert!(toks(" \t ").is_empty());
        // One character after the closing quote is one UTF-16 unit only when it is in
        // the BMP: a surrogate pair is two units and survives.
        assert_eq!(toks("\"a\"é"), ["a"]);
        assert_eq!(toks("\"a\"\u{1F600}"), ["a", "\u{1F600}"]);
    }

    #[test]
    fn worked_example_plugin_arguments() {
        // §11.1: `/FOO  a "b c"  `.
        let inv = prepare("/FOO  a \"b c\"  ", "/", "say", "")
            .unwrap()
            .unwrap();
        assert_eq!(inv.word, "FOO");
        assert_eq!(inv.arguments, "a \"b c\"");
        assert_eq!(inv.tokens, ["FOO", "a", "b c"]);
        assert!(!inv.plain);
    }

    #[test]
    fn worked_example_plain_text() {
        // §11.2: ` hello ` in the Say mode, no target.
        let inv = prepare(" hello ", "/", "S", "").unwrap().unwrap();
        assert_eq!(inv.word, "S");
        assert_eq!(inv.arguments, "hello");
        assert_eq!(inv.tokens, ["S", "hello"]);
        assert!(inv.plain);
    }

    #[test]
    fn prefix_rules() {
        // Only the prefix: nothing to run (result 5); a doubled prefix is accepted.
        assert_eq!(prepare("/", "/", "say", ""), Ok(None));
        assert_eq!(prepare("  //  ", "/", "say", ""), Ok(None));
        assert_eq!(prepare("", "/", "say", ""), Ok(None));
        assert_eq!(
            prepare("//play tune", "/", "say", "")
                .unwrap()
                .unwrap()
                .word,
            "play"
        );
        // `/say /x` says `/x`.
        let inv = prepare("/say /x", "/", "say", "").unwrap().unwrap();
        assert_eq!((inv.word.as_str(), inv.arguments.as_str()), ("say", "/x"));
        // A prefix in the middle is plain text.
        let inv = prepare("a/b", "/", "say", "").unwrap().unwrap();
        assert_eq!((inv.word.as_str(), inv.arguments.as_str()), ("say", "a/b"));
        // A quoted command word: nothing is removed from the arguments.
        let inv = prepare("/\"my cmd\" x", "/", "say", "").unwrap().unwrap();
        assert_eq!(inv.word, "my cmd");
        assert_eq!(inv.arguments, "\"my cmd\" x");
        // Leading characters of the word are all removed (stopping at the first other).
        let inv = prepare("/say sayings", "/", "say", "").unwrap().unwrap();
        assert_eq!(inv.arguments, "sayings");
        let inv = prepare("/ab bab", "/", "say", "").unwrap().unwrap();
        assert_eq!(inv.arguments, "bab");
        let inv = prepare("/aab", "/", "say", "").unwrap().unwrap();
        assert_eq!(inv.arguments, "");
        assert_eq!(
            prepare(&"x".repeat(MAX_LINE_LEN + 1), "/", "say", ""),
            Err(LineError::TooLong)
        );
    }

    #[test]
    fn history_worked_example() {
        // §11.4.
        let mut h = History::new();
        for t in ["a", "b", "a"] {
            h.add(t);
        }
        assert_eq!(h.lines().collect::<Vec<_>>(), ["a", "b"]);
        assert_eq!(h.older(), Some("a"));
        assert_eq!(h.older(), Some("b"));
        assert_eq!(h.older(), Some("b"));
        assert_eq!(h.newer(), Some("a"));
        assert_eq!(h.newer(), None);
        assert_eq!(h.index(), -1);
        for i in 0..25 {
            h.add(&format!("line {i}"));
        }
        assert_eq!(h.lines().count(), HISTORY_CAPACITY - 1);
        assert_eq!(h.lines().next(), Some("line 24"));
        let mut h = History::new();
        assert_eq!(h.older(), None);
        assert_eq!(h.index(), -1);
    }
}
