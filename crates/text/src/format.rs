// SPDX-License-Identifier: GPL-3.0-or-later
//! The `${VAR}` presentation of a message (`docs/specs/string-table.md` §8.1, §8.2).
//!
//! Stored data never contains `${`. The format string is a Tessera presentation for the
//! explorer, logs, diffs and tests, and must never be written back to an archive. Labels,
//! selectors, escapes and markup stay exactly as stored; a literal `$` is written `$$`.
//!
//! Round trip: format → parse → format always gives the same string, and the parsed
//! message renders like the original. The parsed message equals the original except
//! that (a) a slot shown by a name (the entry's own or a dictionary's) gets that name's
//! hash as its ID, which is the original ID whenever the name is the one it was hashed
//! from, and (b) slots past the last piece (more IDs than gaps) come back separated by
//! empty pieces. A slot without an ID is written `${}`; an ID is written `#` plus at least
//! 7 upper-case hex digits (8 for hostile IDs of 2²⁸ or more).

use tessera_formats::name_hash;

use crate::message::{Message, Part};
use crate::render::{RenderOptions, Rendered, render};
use crate::value::{NameDictionary, Values};

/// The name shown for a slot (spec §8.2): the entry's own name, else the dictionary,
/// else `#` and the ID as 7 upper-case hex digits.
fn slot_name(msg: &Message, slot: usize, names: Option<&NameDictionary>) -> String {
    let s = &msg.slots()[slot];
    if let Some(n) = &s.name {
        return n.clone();
    }
    match s.arg_id {
        None => String::new(),
        Some(id) => names
            .and_then(|d| d.get(id))
            .map_or_else(|| format!("#{id:07X}"), str::to_owned),
    }
}

/// The format string of a message: pieces as stored, with `${name}` in each slot and `$`
/// doubled.
#[must_use]
pub fn format_string(msg: &Message, names: Option<&NameDictionary>) -> String {
    let mut out = String::new();
    for part in msg.parts() {
        match part {
            Part::Piece(k) => out.push_str(&msg.pieces()[k].raw.replace('$', "$$")),
            Part::Slot(i) => {
                out.push_str("${");
                out.push_str(&slot_name(msg, i, names));
                out.push('}');
            }
        }
    }
    out
}

/// A format string split into pieces and slot names.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Template {
    /// Literal pieces (`$$` decoded); always one more than `names`.
    pub pieces: Vec<String>,
    /// Slot names in order.
    pub names: Vec<String>,
}

/// Splits a format string: `$$` → `$`, `${name}` → a slot, any other `$` (including an
/// unterminated `${`) is literal.
#[must_use]
pub fn parse_format_string(s: &str) -> Template {
    let mut pieces = Vec::new();
    let mut names = Vec::new();
    let mut cur = String::new();
    let mut i = 0;
    while let Some(c) = s[i..].chars().next() {
        let rest = &s[i..];
        if c == '$' {
            if rest.starts_with("$$") {
                cur.push('$');
                i += 2;
                continue;
            }
            if let Some(inner) = rest.strip_prefix("${")
                && let Some(end) = inner.find('}')
            {
                pieces.push(std::mem::take(&mut cur));
                names.push(inner[..end].to_owned());
                i += 2 + end + 1;
                continue;
            }
        }
        cur.push(c);
        i += c.len_utf8();
    }
    pieces.push(cur);
    Template { pieces, names }
}

/// The argument ID for a slot name: `#` + 1–8 hex digits is a literal ID; anything else
/// is hashed with the name hash (bytes as given).
#[must_use]
pub fn arg_id_for_name(name: &str) -> u32 {
    hex_id(name).unwrap_or_else(|| name_hash(name.as_bytes()))
}

/// The literal ID of a `#XXXXXXX` slot name.
fn hex_id(name: &str) -> Option<u32> {
    let hex = name.strip_prefix('#')?;
    if (1..=8).contains(&hex.len()) && hex.bytes().all(|b| b.is_ascii_hexdigit()) {
        u32::from_str_radix(hex, 16).ok()
    } else {
        None
    }
}

impl Template {
    /// Builds a message: slot names become argument IDs ([`arg_id_for_name`]); names that
    /// are not hex IDs are kept as the slots' variable names. Trailing empty names (`${}`)
    /// are slots without an ID.
    #[must_use]
    pub fn to_message(&self) -> Message {
        let with_id = self
            .names
            .iter()
            .rposition(|n| !n.is_empty())
            .map_or(0, |p| p + 1);
        let ids = self.names[..with_id]
            .iter()
            .map(|n| arg_id_for_name(n))
            .collect();
        let mut msg = Message::new(self.pieces.clone(), ids, None);
        for (i, name) in self.names[..with_id].iter().enumerate() {
            if hex_id(name).is_none() {
                msg.set_slot_name(i, name.clone());
            }
        }
        msg
    }
}

impl Message {
    /// Parses a `${NAME}` format string into a message (see [`parse_format_string`]).
    #[must_use]
    pub fn from_format_string(s: &str) -> Self {
        parse_format_string(s).to_message()
    }

    /// The format string of this message (see [`format_string`]).
    #[must_use]
    pub fn format_string(&self, names: Option<&NameDictionary>) -> String {
        format_string(self, names)
    }
}

/// Renders a `${NAME}` template with a caller-provided variable map. Variables are looked
/// up by the name hash of NAME (or by the literal ID for `${#XXXXXXX}`); the template may
/// use the full inline grammar (labels, selectors, escapes, markup).
#[must_use]
pub fn render_template(template: &str, values: &Values, opts: &RenderOptions<'_>) -> Rendered {
    render(&Message::from_format_string(template), values, opts)
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::render::{RenderMode, render_plain};

    #[test]
    fn round_trip() {
        let m = Message::new(
            vec!["Pay $5 to #1:".into(), " ${x} #1:{him[m]|her[f]}.".into()],
            vec![0x0123_4567],
            None,
        );
        let f = m.format_string(None);
        assert_eq!(f, "Pay $$5 to #1:${#1234567} $${x} #1:{him[m]|her[f]}.");
        let t = parse_format_string(&f);
        assert_eq!(t.pieces, vec!["Pay $5 to #1:", " ${x} #1:{him[m]|her[f]}."]);
        assert_eq!(t.names, vec!["#1234567"]);
        let m2 = t.to_message();
        assert_eq!(m2.slots()[0].arg_id, Some(0x0123_4567));
        assert_eq!(m2.format_string(None), f);
    }

    #[test]
    fn names_from_dictionary_and_entry() {
        let d: NameDictionary = ["NAME"].into_iter().collect();
        let id = name_hash(b"NAME");
        let m = Message::new(vec!["a".into(), "b".into()], vec![id], None);
        assert_eq!(m.format_string(Some(&d)), "a${NAME}b");
        let m = Message::from_format_string("hi ${PLAYER}!");
        assert_eq!(m.slots()[0].name.as_deref(), Some("PLAYER"));
        assert_eq!(m.format_string(None), "hi ${PLAYER}!");
    }

    #[test]
    fn templates() {
        let v = Values::new().with("PLAYER", "Wren[fn]").with("COUNT", 3i64);
        let r = render_template(
            "${PLAYER} found ${COUNT} #1:{acorn[1]|acorns}.",
            &v,
            &RenderOptions::default(),
        );
        // No label is bound: the selector sees an absent value and picks the default.
        assert_eq!(r.plain_text(), "Wren found 3 acorns.");
        let m = Message::from_format_string("#1:${COUNT} #1:{acorn[1]|acorns}");
        let v = Values::new().with("COUNT", 1i64);
        assert_eq!(render_plain(&m, &v), "1 acorn");
        let r = render_template(
            "x ${MISSING} y",
            &Values::new(),
            &RenderOptions::mode(RenderMode::Explorer),
        );
        assert_eq!(r.plain_text(), "x ${MISSING} y");
        // Unterminated and stray dollars are literal.
        let t = parse_format_string("a $ b ${c");
        assert_eq!(t.pieces, vec!["a $ b ${c"]);
    }

    #[test]
    fn hex_names() {
        assert_eq!(arg_id_for_name("#0BBBBBB"), 0x0BB_BBBB);
        assert_eq!(arg_id_for_name("#12345678"), 0x1234_5678);
        assert_eq!(arg_id_for_name("#ZZ"), name_hash(b"#ZZ"));
        assert_eq!(arg_id_for_name("#123456789"), name_hash(b"#123456789"));
    }

    #[test]
    fn hostile_round_trips() {
        // Regression: an ID of 2^28 or more prints with 8 digits and parses back.
        let m = Message::new(vec!["a".into(), "b".into()], vec![0x1234_5678], None);
        let f = m.format_string(None);
        assert_eq!(f, "a${#12345678}b");
        let back = Message::from_format_string(&f);
        assert_eq!(back.slots()[0].arg_id, Some(0x1234_5678));
        assert_eq!(back, m);
        let v: Values = [(0x1234_5678, "X".into())].into_iter().collect();
        assert_eq!(render_plain(&back, &v), render_plain(&m, &v));
        // A slot without an ID is `${}` and parses back without one.
        let m = Message::new(vec!["a".into(), "b".into(), "c".into()], vec![7], None);
        let f = m.format_string(None);
        assert_eq!(f, "a${#0000007}b${}c");
        let back = Message::from_format_string(&f);
        assert_eq!(back, m);
        assert_eq!(back.format_string(None), f);
        // Slots past the last piece: stable string, same rendering.
        let m = Message::new(vec!["a".into(), "b".into()], vec![1, 2, 3], None);
        let f = m.format_string(None);
        let back = Message::from_format_string(&f);
        assert_eq!(back.format_string(None), f);
        let v: Values = [(1, "x".into()), (2, "y".into()), (3, "z".into())]
            .into_iter()
            .collect();
        assert_eq!(render_plain(&back, &v), render_plain(&m, &v));
    }
}
