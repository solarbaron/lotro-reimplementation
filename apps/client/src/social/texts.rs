// SPDX-License-Identifier: GPL-3.0-or-later
//! The client's [`TextSource`]: the P4 lines and labels (`chat-window.md` §15.10 CT12,
//! `social-panels.md` §2) rendered from the user's own string tables at run time.
//!
//! A [`TextRef`](tessera_ui::game::chat::TextRef) names its table by STRINGTABLE **key**;
//! the named-table map of the install finds the table, and `tessera-text` renders the entry
//! with the argument values. Values from the session (names, channel names, chat text) are
//! inserted as literal text (CT12): every value is a text value (never re-formatted as a
//! number; `tessera-text` still derives its number for plural selection), written in the
//! form the renderer's value path decodes back to the same characters ([`literal`]), so
//! neither the message grammar nor markup inside it is interpreted. The trailing line break
//! of a template is removed. Nothing of the tables' text is built in: a missing table or
//! entry renders `None`, and the caller falls back to Tessera's own wording
//! ([`tessera_ui::game::chat::notice::fallback_text`]).

use std::sync::Arc;

use tessera_text::{Message, RenderOptions, Strings, Value, Values};
use tessera_ui::game::chat::TextSource;

/// Longest value inserted, bytes (session text is at most 1,024 bytes; the bound keeps a
/// hostile value from growing the escaped copy without limit).
pub const MAX_VALUE_BYTES: usize = 4096;

/// Characters standing in for a value's `<` while the entry renders (Unicode private use;
/// the first one that no value contains is used). The renderer's value path has no escape
/// for `<`, and a `<` that opens a valid tag would be read as markup.
const LT_STAND_INS: [char; 4] = ['\u{E000}', '\u{E001}', '\u{E002}', '\u{E003}'];

/// `text` written for the renderer's value path so that it renders as the same characters:
/// `\`, `[` and `]` escaped (the value path decodes `\\`, `\[` and `\]`; an escaped `]` also
/// keeps a trailing `[...]` from being read as a tag group), `<` replaced by `lt` (put back
/// after rendering), control characters as spaces, cut at [`MAX_VALUE_BYTES`] on a
/// character boundary. Every other character (`|`, `$`, `#`, `{`, `@`, ...) is not
/// interpreted inside a value and is kept as is.
#[must_use]
pub fn literal(text: &str, lt: char) -> String {
    let mut out = String::with_capacity(text.len().min(MAX_VALUE_BYTES) + 8);
    for c in text.chars() {
        if out.len() >= MAX_VALUE_BYTES {
            break;
        }
        match c {
            '\\' | '[' | ']' => {
                out.push('\\');
                out.push(c);
            }
            '<' => out.push(lt),
            c if c.is_control() => out.push(' '),
            c => out.push(c),
        }
    }
    out
}

/// Renders `message` with `args` inserted literally (module docs); `None` when the result
/// is blank, or when every stand-in for `<` occurs in a value (hostile input).
#[must_use]
pub fn render_literal(message: &Message, args: &[(u32, String)]) -> Option<String> {
    let lt = LT_STAND_INS
        .into_iter()
        .find(|c| args.iter().all(|(_, v)| !v.contains(*c)))?;
    let mut values = Values::new();
    for (id, v) in args {
        values.insert(*id, Value::Text(literal(v, lt)));
    }
    let text = tessera_text::render(message, &values, &RenderOptions::default()).plain_text();
    let text = text.trim_end_matches(['\n', '\r']);
    if text.trim().is_empty() {
        return None;
    }
    Some(if text.contains(lt) {
        text.replace(lt, "<")
    } else {
        text.to_owned()
    })
}

/// The install's string tables as a [`TextSource`] (module docs).
#[derive(Clone)]
pub struct InstallText {
    strings: Arc<Strings>,
}

impl std::fmt::Debug for InstallText {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("InstallText").finish_non_exhaustive()
    }
}

impl InstallText {
    /// Renders from `strings` (their named-table map finds the tables by key).
    #[must_use]
    pub fn new(strings: Arc<Strings>) -> Self {
        Self { strings }
    }

    /// The string tables.
    #[must_use]
    pub fn strings(&self) -> &Arc<Strings> {
        &self.strings
    }
}

impl TextSource for InstallText {
    fn render(&mut self, table: u32, token: u32, args: &[(u32, String)]) -> Option<String> {
        let did = self.strings.named_tables().by_key(table)?;
        let message = self.strings.message(did, token).ok()?;
        render_literal(&message, args)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    const A: u32 = 0x5277;
    const B: u32 = 0x0005_2615;
    const LT: char = '\u{E000}';

    fn template() -> Message {
        Message::new(
            vec!["From ".into(), ": says '".into(), "'\n".into()],
            vec![B, A],
            None,
        )
    }

    #[test]
    fn literal_escapes_only_what_the_value_path_decodes() {
        assert_eq!(literal("plain", LT), "plain");
        assert_eq!(literal("a[b]", LT), r"a\[b\]");
        assert_eq!(literal("<b>", LT), "\u{E000}b>");
        assert_eq!(literal("50$ @ #1 | {}", LT), "50$ @ #1 | {}");
        assert_eq!(literal(r"a\b", LT), r"a\\b");
        assert_eq!(literal("tab\there", LT), "tab here");
        let long = "x".repeat(MAX_VALUE_BYTES * 2);
        assert!(literal(&long, LT).len() <= MAX_VALUE_BYTES + 4);
        assert_eq!(literal("", LT), "");
    }

    /// Regression (P4 review): session values render as the same characters; numbers keep
    /// their leading zeros and signs; grammar and markup inside a value are not read.
    #[test]
    fn session_values_round_trip_through_the_renderer() {
        let m = template();
        for text in [
            "007",
            "-0",
            "0800",
            "a[b]|c <x> 50$ #1 {y} @z",
            r"<b>bold</b> <rgb=#FF0000>red</rgb> <Select:IID:1>x<\Select>",
            r"back\slash \n \[ \q \\",
            "ends with [tag]",
            "[a] ^ ! $ # @ } { #1:{x|y}",
            "\u{E000} stand-in in the text <b>",
        ] {
            for name in ["Bo", "0042", "[x]", "<i>"] {
                let out = render_literal(&m, &[(A, text.to_owned()), (B, name.to_owned())])
                    .expect("renders");
                assert_eq!(out, format!("From {name}: says '{text}'"), "text {text:?}");
            }
        }
    }

    #[test]
    fn every_stand_in_taken_renders_nothing() {
        let all: String = LT_STAND_INS.iter().collect();
        assert_eq!(render_literal(&template(), &[(A, all)]), None);
    }

    #[test]
    fn a_source_without_tables_renders_nothing() {
        let strings = Arc::new(Strings::new(tessera_text::MemorySource::new()));
        let mut t = InstallText::new(strings);
        assert_eq!(t.render(0x1000_004D, 0x0BCA_09CC, &[]), None);
    }
}
