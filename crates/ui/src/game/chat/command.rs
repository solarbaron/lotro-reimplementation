// SPDX-License-Identifier: GPL-3.0-or-later
//! The command line (`chat-window.md` §7): whitespace and trimming, the tokenizer, command
//! word and arguments, running a submitted line, `;` substitutions, the registry and
//! `IsCommand` (§8.2), and the shared history (§6.3).
//!
//! Handlers live outside this crate (the plugin host's `Shell.AddCommand`, the client's
//! own commands); the pipeline reaches them through [`CommandHost`].

use std::collections::BTreeMap;

/// Result: no registry or game context, or an empty line given to execute (§7.4).
pub const RESULT_NO_REGISTRY: i32 = 2;
/// Result: unknown command (§7.4).
pub const RESULT_UNKNOWN: i32 = 3;
/// Result: nothing to run (§7.4).
pub const RESULT_NOTHING: i32 = 5;

/// History capacity (§6.3): the list holds at most capacity − 1 = 19 lines.
pub const HISTORY_CAPACITY: usize = 20;

/// Longest line the pipeline accepts, in characters (Tessera cap; the entry allows 384).
pub const MAX_LINE: usize = 1 << 16;

/// Deepest nested substitution (Tessera cap on top of the recursion guard of §7.5).
pub const MAX_SUBSTITUTION_DEPTH: usize = 16;

/// Longest text a substitution may produce (Tessera cap).
pub const MAX_EXPANDED: usize = 1 << 16;

/// Most substitution values asked for over one whole expansion (Tessera cap, hostile input):
/// with it, values that fan out into many tokens cost bounded work even when their
/// expansions are empty. Once it is spent, the remaining tokens stay as typed.
pub const MAX_SUBSTITUTIONS: usize = 4096;

/// Most registered names (Tessera cap).
pub const MAX_NAMES: usize = 1 << 16;

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

/// The tokenizer of §7.3, including its two quirks.
#[must_use]
pub fn tokenize(text: &str) -> Vec<String> {
    let chars: Vec<char> = text.chars().take(MAX_LINE).collect();
    let n = chars.len();
    let mut tokens = Vec::new();
    let mut current = String::new();
    let mut start = 0usize;
    let mut i = 0usize;
    while i < n {
        if chars[i] == '"' {
            // The token start becomes i + 1 here, but every branch ends by setting it to
            // the new position before it is read again.
            if let Some(off) = chars[i + 1..].iter().position(|c| *c == '"') {
                let j = i + 1 + off;
                current = chars[i + 1..j].iter().collect();
                i = j + 1;
            } else {
                // The quote is ignored; the current token is left unchanged.
                i += 1;
            }
        } else {
            let e = chars[i..]
                .iter()
                .position(|c| is_ws(*c))
                .map_or(n, |off| i + off);
            let s: String = chars[start.min(e)..e].iter().collect();
            current = trim_ws(&s).to_string();
            i = e;
        }
        if !current.is_empty() {
            tokens.push(current.clone());
        }
        start = i;
        // Never past the end.
        i = (i + 1).min(n);
    }
    tokens
}

/// The arguments of §7.4 step 5: the trimmed text with every leading character that
/// occurs anywhere in `word` removed, then trimmed.
#[must_use]
pub fn arguments(trimmed: &str, word: &str) -> String {
    let rest = trimmed.trim_start_matches(|c: char| word.contains(c));
    trim_ws(rest).to_string()
}

/// Case-insensitive name key (§8.2; Tessera folds with Unicode lower-casing, the reading
/// of the locales the client sets, CW37 Probable).
#[must_use]
pub fn fold(name: &str) -> String {
    name.to_lowercase()
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

/// What a registered command is.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct CommandInfo {
    /// The handler's ID in the host.
    pub id: u32,
    /// Whether lines that run it may use `;` substitutions (§7.1; plugin commands do).
    pub substitutes: bool,
}

/// One command registry (§7.1): names stored without the prefix, compared
/// case-insensitively; a name is registered only if it is not held yet.
#[derive(Debug, Clone, Default, PartialEq, Eq)]
pub struct CommandRegistry {
    names: BTreeMap<String, CommandInfo>,
}

impl CommandRegistry {
    /// Registers one name. Returns false when the name is empty, already held, or the
    /// registry is full.
    pub fn register(&mut self, name: &str, info: CommandInfo) -> bool {
        if name.is_empty() || self.names.len() >= MAX_NAMES {
            return false;
        }
        let k = fold(name);
        if self.names.contains_key(&k) {
            return false;
        }
        self.names.insert(k, info);
        true
    }

    /// Registers every name of a name list (§7.1 splitting rule); returns the names taken.
    pub fn register_list(&mut self, entry: &str, info: CommandInfo) -> Vec<String> {
        split_names(entry)
            .into_iter()
            .filter(|n| self.register(n, info))
            .collect()
    }

    /// Removes one name.
    pub fn unregister(&mut self, name: &str) -> bool {
        self.names.remove(&fold(name)).is_some()
    }

    /// Removes every name of a handler.
    pub fn unregister_id(&mut self, id: u32) {
        self.names.retain(|_, i| i.id != id);
    }

    /// The command a name runs.
    #[must_use]
    pub fn lookup(&self, name: &str) -> Option<CommandInfo> {
        if name.is_empty() {
            return None;
        }
        self.names.get(&fold(name)).copied()
    }

    /// `Shell.IsCommand(name)` (§8.2): the name exactly as given (no trimming, no prefix
    /// removal, no splitting), case-insensitively; an empty name is never registered.
    #[must_use]
    pub fn is_command(&self, name: &str) -> bool {
        self.lookup(name).is_some()
    }

    /// Number of names.
    #[must_use]
    pub fn len(&self) -> usize {
        self.names.len()
    }

    /// Whether no name is registered.
    #[must_use]
    pub fn is_empty(&self) -> bool {
        self.names.is_empty()
    }
}

/// The handlers behind the pipeline.
pub trait CommandHost {
    /// Whether a registry and a game context exist (§7.2 step 1).
    fn available(&self) -> bool {
        true
    }

    /// The command a word names (case-insensitively), if any.
    fn lookup(&self, word: &str) -> Option<CommandInfo>;

    /// Runs a handler with the command word as typed, the arguments and the tokens;
    /// returns its result (game handlers use 1 for success).
    fn run(&mut self, info: CommandInfo, word: &str, args: &str, tokens: &[String]) -> i32;

    /// The value of a `;` substitution token (§7.5; game-script callback), or `None` when
    /// the game has none. Tessera has no substitution words in M5 (Q14).
    fn substitute(&mut self, token: &str) -> Option<String> {
        let _ = token;
        None
    }
}

/// A [`CommandHost`] over a [`CommandRegistry`] whose handlers are a closure.
pub struct RegistryHost<'a, F: FnMut(CommandInfo, &str, &str, &[String]) -> i32> {
    /// The registry.
    pub registry: &'a CommandRegistry,
    /// The handlers.
    pub handler: F,
}

impl<F: FnMut(CommandInfo, &str, &str, &[String]) -> i32> std::fmt::Debug for RegistryHost<'_, F> {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("RegistryHost")
            .field("registry", &self.registry)
            .finish_non_exhaustive()
    }
}

impl<F: FnMut(CommandInfo, &str, &str, &[String]) -> i32> CommandHost for RegistryHost<'_, F> {
    fn lookup(&self, word: &str) -> Option<CommandInfo> {
        self.registry.lookup(word)
    }

    fn run(&mut self, info: CommandInfo, word: &str, args: &str, tokens: &[String]) -> i32 {
        (self.handler)(info, word, args, tokens)
    }
}

/// The shared history (§6.3).
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct History {
    /// Lines, newest first.
    pub lines: Vec<String>,
    /// Browsing index *k* (−1: not browsing).
    pub index: i32,
}

impl Default for History {
    fn default() -> Self {
        Self {
            lines: Vec::new(),
            index: -1,
        }
    }
}

impl History {
    /// Adds a submitted text (§6.3 "Adding"): an identical entry is removed, the text
    /// becomes the newest, the list is trimmed below the capacity and the browsing index
    /// is reset.
    pub fn add(&mut self, text: &str) {
        self.lines.retain(|l| l != text);
        self.lines.insert(0, text.to_string());
        while self.lines.len() >= HISTORY_CAPACITY {
            self.lines.pop();
        }
        self.index = -1;
    }

    /// Older (`CursorPreviousLine`): *k* ← min(*k* + 1, count − 1); the entry to show when
    /// *k* ≥ 0.
    pub fn older(&mut self) -> Option<&str> {
        let count = i32::try_from(self.lines.len()).unwrap_or(i32::MAX);
        self.index = (self.index + 1).min(count - 1);
        usize::try_from(self.index)
            .ok()
            .and_then(|k| self.lines.get(k))
            .map(String::as_str)
    }

    /// Newer (`CursorNextLine`): *k* ← *k* − 1; below 0 it becomes −1 and nothing changes
    /// (`None`), else the entry to show.
    pub fn newer(&mut self) -> Option<&str> {
        self.index -= 1;
        if self.index < 0 {
            self.index = -1;
            return None;
        }
        usize::try_from(self.index)
            .ok()
            .and_then(|k| self.lines.get(k))
            .map(String::as_str)
    }
}

/// The `CommandSetup` texts (§7.2, read at run time from the install).
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

/// The command line: setup and the shared history.
#[derive(Debug, Clone, Default, PartialEq, Eq)]
pub struct CommandLine {
    /// The `CommandSetup` texts.
    pub setup: CommandSetup,
    /// The shared history.
    pub history: History,
}

impl CommandLine {
    /// Runs a submitted line (§7.2) with the channel's command word `mode_word` (empty: the
    /// default word) and tell target `target`. Returns the result code.
    pub fn run_line(
        &mut self,
        text: &str,
        mode_word: &str,
        target: &str,
        host: &mut dyn CommandHost,
    ) -> i32 {
        if !host.available() {
            return RESULT_NO_REGISTRY;
        }
        if text.is_empty() {
            return RESULT_NOTHING;
        }
        let text: String = text.chars().take(MAX_LINE).collect();
        self.history.add(&text);
        let mut line = text;
        if self.substitution_test(&line, host) {
            let mut budget = MAX_SUBSTITUTIONS;
            line = expand(&line, host, &mut Vec::new(), 0, &mut budget);
        }
        let t = trim_ws(&line);
        if t.is_empty() {
            return RESULT_NOTHING;
        }
        let p = &self.setup.prefix;
        if !p.is_empty() && t.starts_with(p.as_str()) {
            let rest = t.trim_start_matches(|c: char| p.contains(c));
            if rest.is_empty() {
                return RESULT_NOTHING;
            }
            return execute(rest, host);
        }
        let word = if mode_word.is_empty() {
            self.setup.default_word.as_str()
        } else {
            mode_word
        };
        let built = format!("{word} {target} {t}");
        execute(&built, host)
    }

    /// The substitution test of §7.5.
    fn substitution_test(&self, text: &str, host: &dyn CommandHost) -> bool {
        let p = &self.setup.prefix;
        if p.is_empty() {
            return true;
        }
        let lower = fold(text);
        let Some(pos) = lower.find(&fold(p)) else {
            return true;
        };
        let after = &lower[pos + fold(p).len()..];
        let word = after.split(' ').next().unwrap_or("");
        host.lookup(word).is_some_and(|i| i.substitutes)
    }
}

/// Executes a text (§7.4): trims, tokenizes, takes the command word and arguments, looks
/// the word up and runs its handler.
pub fn execute(text: &str, host: &mut dyn CommandHost) -> i32 {
    if text.is_empty() {
        return RESULT_NO_REGISTRY;
    }
    let t = trim_ws(text);
    if t.is_empty() {
        return RESULT_NOTHING;
    }
    let tokens = tokenize(t);
    let Some(word) = tokens.first().cloned() else {
        return RESULT_NOTHING;
    };
    let args = arguments(t, &word);
    match host.lookup(&word) {
        None => RESULT_UNKNOWN,
        Some(info) => host.run(info, &word, &args, &tokens),
    }
}

/// The expansion of §7.5: every `;` followed by a run of letters and digits is a token;
/// each is replaced by its value (itself expanded) unless the game has none or the same
/// token is being expanded further out. `budget` counts the values still allowed to be
/// asked for ([`MAX_SUBSTITUTIONS`] for the whole expansion).
fn expand(
    text: &str,
    host: &mut dyn CommandHost,
    active: &mut Vec<String>,
    depth: usize,
    budget: &mut usize,
) -> String {
    if depth >= MAX_SUBSTITUTION_DEPTH {
        return text.to_string();
    }
    let chars: Vec<char> = text.chars().collect();
    let mut out = String::new();
    let mut i = 0;
    while i < chars.len() {
        if chars[i] != ';' {
            out.push(chars[i]);
            i += 1;
            continue;
        }
        let mut j = i + 1;
        while j < chars.len() && chars[j].is_alphanumeric() {
            j += 1;
        }
        let token: String = chars[i..j].iter().collect();
        let value = if active.contains(&token) || *budget == 0 {
            None
        } else {
            *budget -= 1;
            host.substitute(&token)
        };
        match value {
            Some(v) => {
                active.push(token);
                let e = expand(&v, host, active, depth + 1, budget);
                active.pop();
                out.push_str(&e);
            }
            None => out.push_str(&token),
        }
        if out.len() > MAX_EXPANDED {
            out.truncate(
                (0..=MAX_EXPANDED)
                    .rev()
                    .find(|n| out.is_char_boundary(*n))
                    .unwrap_or(0),
            );
            return out;
        }
        i = j;
    }
    out
}
