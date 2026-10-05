// SPDX-License-Identifier: GPL-3.0-or-later
//! The message model and the inline grammar parser.
//!
//! Implements `docs/specs/string-table.md` §3 (pieces, slots, argument IDs and names) and
//! the structural part of §4: slot labels (§4.2), `#N:` selectors (§4.3), bare selectors
//! (§4.4) and directives (§4.5). Escapes and markup stay in the text segments; they are
//! decoded at render time (§7 steps 5–6).

use std::collections::HashMap;

use tessera_formats::StringTableEntry;

use crate::tags::{SelectorOption, parse_options};

/// Maximum number of grammar issues kept per message.
const MAX_ISSUES: usize = 32;
/// Maximum number of digits accepted in a label number.
const MAX_LABEL_DIGITS: usize = 18;

/// A slot label `#N:` or `#N:^` (spec §4.2).
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub struct Label {
    /// The label number *N* (local to the entry; not the slot index).
    pub number: i64,
    /// The `^` modifier (meaning OPEN; Tessera renders the value unchanged).
    pub caret: bool,
}

/// A selector (spec §4.3, §4.4).
#[derive(Debug, Clone, PartialEq)]
pub struct Selector {
    /// The label it reads; `None` for a bare selector.
    pub label: Option<i64>,
    /// Options in order.
    pub options: Vec<SelectorOption>,
    /// The selector exactly as stored, used when it must render literally.
    pub raw: String,
}

/// One structural element of a piece.
#[derive(Debug, Clone, PartialEq)]
pub enum Segment {
    /// Literal raw text (escapes and markup still encoded).
    Text(String),
    /// A selector.
    Selector(Selector),
    /// The `{{keepspaces}}` directive.
    KeepSpaces,
}

/// A literal piece of a message.
#[derive(Debug, Clone, PartialEq)]
pub struct Piece {
    /// The piece exactly as stored.
    pub raw: String,
    /// The piece after removing a trailing label, split into segments.
    pub segments: Vec<Segment>,
}

/// An argument slot.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Slot {
    /// The argument ID that fills the slot; `None` when the entry stores fewer IDs than
    /// slots (invariant ST3 violated), which renders as an absent value.
    pub arg_id: Option<u32>,
    /// The label on the piece before this slot, if any.
    pub label: Option<Label>,
    /// The piece before this slot ended with the `^` modifier, either after a label
    /// (`#N:^`) or on its own (spec §4.2.1). The `^` is not rendered; its meaning is OPEN
    /// and Tessera renders the value unchanged.
    pub caret: bool,
    /// The variable name from the entry's own name group, if stored.
    pub name: Option<String>,
}

/// One part of a message in rendering order.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Part {
    /// Piece index.
    Piece(usize),
    /// Slot index.
    Slot(usize),
}

/// A grammar-invariant violation (spec §12). Rendering still succeeds; these are reported
/// for validation and the explorer.
#[derive(Debug, Clone, PartialEq, Eq, Hash)]
#[non_exhaustive]
pub enum GrammarIssue {
    /// SG1: a `#N:{` selector refers to a label not defined in the entry.
    UndefinedLabel(i64),
    /// SG2: `#N:` that is not a slot label and does not start a selector.
    StrayLabel,
    /// SG4: one label names slots with different argument IDs.
    LabelConflict(i64),
    /// SG5: a selector that is not closed within its piece, or whose body has `{`.
    UnterminatedSelector,
    /// SG6: a selector in an entry with no slots.
    SelectorWithoutSlots,
    /// SG7: stored text contains `${`.
    DollarBrace,
    /// SG8: a raw control character other than TAB.
    ControlCharacter,
    /// SG8: a UTF-16 surrogate (lone or paired, i.e. a character above U+FFFF).
    Surrogate,
    /// SG9: `{{keepspaces}}` followed by text or a selector (only slots and empty pieces
    /// may follow it).
    KeepSpacesNotAtEnd,
    /// An unknown `{{word}}` directive (rendered literally).
    UnknownDirective(String),
}

/// A parsed message: pieces with inline grammar, slots, label bindings.
#[derive(Debug, Clone, PartialEq)]
pub struct Message {
    pieces: Vec<Piece>,
    slots: Vec<Slot>,
    bindings: HashMap<i64, usize>,
    keep_spaces: bool,
    issues: Vec<GrammarIssue>,
    /// Rendering cost in work units (see [`Message::render_cost`]).
    cost: usize,
}

/// Work units charged per slot, piece and segment by [`Message::render_cost`]; a text byte
/// counts as one unit.
const COST_PER_PART: usize = 16;

fn push_issue(issues: &mut Vec<GrammarIssue>, issue: GrammarIssue) {
    if issues.len() < MAX_ISSUES && !issues.contains(&issue) {
        issues.push(issue);
    }
}

/// Parses `#`, optional `-`, digits at the start of `s`. Returns the number and the byte
/// length consumed.
fn parse_label_number(s: &str) -> Option<(i64, usize)> {
    let b = s.as_bytes();
    if b.first() != Some(&b'#') {
        return None;
    }
    let mut i = 1;
    if b.get(i) == Some(&b'-') {
        i += 1;
    }
    let start = i;
    while b.get(i).is_some_and(u8::is_ascii_digit) {
        i += 1;
    }
    let digits = i - start;
    if digits == 0 || digits > MAX_LABEL_DIGITS {
        return None;
    }
    let n = s[1..i].parse::<i64>().ok()?;
    Some((n, i))
}

/// Strips a trailing label `#N:` / `#N:^` from a piece (spec §4.2).
fn strip_label(text: &str) -> Option<(&str, Label)> {
    let (rest, caret) = match text.strip_suffix('^') {
        Some(r) => (r, true),
        None => (text, false),
    };
    let rest = rest.strip_suffix(':')?;
    let b = rest.as_bytes();
    let mut i = b.len();
    while i > 0 && b[i - 1].is_ascii_digit() {
        i -= 1;
    }
    if i == b.len() {
        return None;
    }
    if i > 0 && b[i - 1] == b'-' {
        i -= 1;
    }
    if i == 0 || b[i - 1] != b'#' {
        return None;
    }
    let hash = i - 1;
    let (number, len) = parse_label_number(&rest[hash..])?;
    if hash + len != rest.len() {
        return None;
    }
    Some((&rest[..hash], Label { number, caret }))
}

/// Finds the next occurrence of one byte at or after a position, remembering the last
/// answer. Queries made left to right scan each byte at most once, so tokenising stays
/// linear in the piece length however many selector openers it has.
struct NextByte<'a> {
    text: &'a [u8],
    byte: u8,
    /// The last query position and its answer.
    last: Option<(usize, Option<usize>)>,
}

impl<'a> NextByte<'a> {
    fn new(text: &'a str, byte: u8) -> Self {
        Self {
            text: text.as_bytes(),
            byte,
            last: None,
        }
    }

    fn at_or_after(&mut self, pos: usize) -> Option<usize> {
        if let Some((from, at)) = self.last
            && from <= pos
            && at.is_none_or(|a| a >= pos)
        {
            return at;
        }
        let at = self
            .text
            .get(pos..)
            .and_then(|t| t.iter().position(|&c| c == self.byte))
            .map(|r| pos + r);
        self.last = Some((pos, at));
        at
    }
}

/// Splits a piece (label already removed) into segments (spec §7 step 3). Works on raw
/// text: escapes are not interpreted here.
fn tokenize(text: &str, issues: &mut Vec<GrammarIssue>) -> Vec<Segment> {
    let mut segs = Vec::new();
    let mut lit = String::new();
    let flush = |lit: &mut String, segs: &mut Vec<Segment>| {
        if !lit.is_empty() {
            segs.push(Segment::Text(std::mem::take(lit)));
        }
    };
    // A selector body runs to the first '}' (string-table.md §4.3: inside a selector only
    // '|' and '}' are grammar, '{' is ordinary text, selectors never nest). These find the
    // body end, and whether the body has a '[' or a '{', in linear total time.
    let mut close = NextByte::new(text, b'}');
    let mut open = NextByte::new(text, b'{');
    let mut bracket = NextByte::new(text, b'[');
    let mut i = 0;
    while let Some(c) = text[i..].chars().next() {
        let rest = &text[i..];
        // Escapes are decoded after tokenising (spec §7 steps 3 and 5), so a backslash
        // does not protect `#` or `{`: `\#1:{…}` is a backslash followed by a selector,
        // just as `strip_label` reads a trailing `\#1:` as a label.
        match c {
            '#' => {
                if let Some((number, len)) = parse_label_number(rest)
                    && rest[len..].starts_with(':')
                {
                    if rest[len + 1..].starts_with('{') {
                        let body_start = i + len + 2;
                        match close.at_or_after(body_start) {
                            Some(end) => {
                                if open.at_or_after(body_start).is_some_and(|o| o < end) {
                                    // SG5: observed bodies never contain '{'.
                                    push_issue(issues, GrammarIssue::UnterminatedSelector);
                                }
                                flush(&mut lit, &mut segs);
                                segs.push(Segment::Selector(Selector {
                                    label: Some(number),
                                    options: parse_options(&text[body_start..end]),
                                    raw: text[i..=end].to_owned(),
                                }));
                                i = end + 1;
                                continue;
                            }
                            None => push_issue(issues, GrammarIssue::UnterminatedSelector),
                        }
                    } else {
                        push_issue(issues, GrammarIssue::StrayLabel);
                    }
                }
                lit.push('#');
                i += 1;
            }
            '{' => {
                if let Some(inner) = rest.strip_prefix("{{") {
                    if let Some(end) = inner.find(['{', '}'])
                        && inner[end..].starts_with("}}")
                        && end > 0
                    {
                        let word = &inner[..end];
                        let total = 2 + end + 2;
                        if word == "keepspaces" {
                            flush(&mut lit, &mut segs);
                            segs.push(Segment::KeepSpaces);
                        } else {
                            tracing::debug!(word, "unknown directive rendered literally");
                            push_issue(issues, GrammarIssue::UnknownDirective(word.to_owned()));
                            lit.push_str(&rest[..total]);
                        }
                        i += total;
                        continue;
                    }
                } else if let Some(end) = close.at_or_after(i + 1) {
                    // A bare selector needs a tag list; other braces are literal (§4.4).
                    if bracket.at_or_after(i + 1).is_some_and(|b| b < end) {
                        if open.at_or_after(i + 1).is_some_and(|o| o < end) {
                            push_issue(issues, GrammarIssue::UnterminatedSelector);
                        }
                        flush(&mut lit, &mut segs);
                        segs.push(Segment::Selector(Selector {
                            label: None,
                            options: parse_options(&text[i + 1..end]),
                            raw: text[i..=end].to_owned(),
                        }));
                        i = end + 1;
                        continue;
                    }
                }
                lit.push('{');
                i += 1;
            }
            _ => {
                lit.push(c);
                i += c.len_utf8();
            }
        }
    }
    flush(&mut lit, &mut segs);
    segs
}

impl Message {
    /// Builds a message from raw pieces, argument IDs and optional variable names (one per
    /// argument ID, as in an argument-name group).
    ///
    /// Pieces and slots alternate (spec §3.1). If there are more IDs than gaps between
    /// pieces, the extra slots follow the last piece; if there are fewer, the missing
    /// slots have no ID and render as absent values.
    #[must_use]
    pub fn new(pieces: Vec<String>, arg_ids: Vec<u32>, names: Option<Vec<String>>) -> Self {
        let mut issues = Vec::new();
        let slot_count = pieces.len().saturating_sub(1).max(arg_ids.len());
        let mut names = names.map(Vec::into_iter);
        let mut slots: Vec<Slot> = (0..slot_count)
            .map(|i| Slot {
                arg_id: arg_ids.get(i).copied(),
                label: None,
                caret: false,
                name: names.as_mut().and_then(Iterator::next),
            })
            .collect();
        let mut bindings: HashMap<i64, usize> = HashMap::new();
        let mut parsed = Vec::with_capacity(pieces.len());
        let mut keep_spaces = false;
        for (k, raw) in pieces.into_iter().enumerate() {
            if raw.contains("${") {
                push_issue(&mut issues, GrammarIssue::DollarBrace);
            }
            let mut body: &str = &raw;
            if k < slot_count
                && let Some((rest, label)) = strip_label(&raw)
            {
                body = rest;
                if label.caret {
                    tracing::debug!(label = label.number, "label with '^' modifier");
                }
                slots[k].label = Some(label);
                slots[k].caret = label.caret;
                match bindings.get(&label.number) {
                    None => {
                        bindings.insert(label.number, k);
                    }
                    Some(&first) => {
                        if slots[first].arg_id != slots[k].arg_id {
                            push_issue(&mut issues, GrammarIssue::LabelConflict(label.number));
                        }
                    }
                }
            }
            if k < slot_count
                && slots[k].label.is_none()
                && let Some(rest) = body.strip_suffix('^')
            {
                // A bare `^` right before a slot is the same modifier without a label
                // (spec §4.2.1 OPEN): removed from the text, recorded on the slot.
                tracing::debug!(slot = k, "bare '^' modifier before a slot");
                body = rest;
                slots[k].caret = true;
            }
            let segments = tokenize(body, &mut issues);
            for seg in &segments {
                if *seg == Segment::KeepSpaces {
                    keep_spaces = true;
                } else if keep_spaces {
                    // Text or a selector after the directive (slots and empty pieces
                    // after it are allowed).
                    push_issue(&mut issues, GrammarIssue::KeepSpacesNotAtEnd);
                }
            }
            parsed.push(Piece {
                raw: raw.clone(),
                segments,
            });
        }
        let parts = parsed.iter().map(|p| p.segments.len()).fold(
            parsed.len().saturating_add(slots.len()),
            usize::saturating_add,
        );
        let bytes = parsed
            .iter()
            .map(|p| p.raw.len())
            .fold(0, usize::saturating_add);
        let msg = Self {
            pieces: parsed,
            slots,
            bindings,
            keep_spaces,
            issues,
            cost: parts.saturating_mul(COST_PER_PART).saturating_add(bytes),
        };
        msg.finish_issues()
    }

    fn finish_issues(mut self) -> Self {
        let mut issues = std::mem::take(&mut self.issues);
        for piece in &self.pieces {
            for seg in &piece.segments {
                if let Segment::Selector(sel) = seg {
                    if self.slots.is_empty() {
                        push_issue(&mut issues, GrammarIssue::SelectorWithoutSlots);
                    }
                    if let Some(n) = sel.label
                        && !self.bindings.contains_key(&n)
                    {
                        push_issue(&mut issues, GrammarIssue::UndefinedLabel(n));
                    }
                }
            }
        }
        self.issues = issues;
        self
    }

    /// Builds a message from a decoded string-table entry (UTF-16 decoded lossily; lone
    /// surrogates become U+FFFD and are reported as [`GrammarIssue::Surrogate`]).
    #[must_use]
    pub fn from_entry(entry: &StringTableEntry) -> Self {
        let pieces: Vec<String> = entry.pieces.iter().map(|p| p.to_string_lossy()).collect();
        let names = entry
            .arg_names()
            .map(|n| n.iter().map(|s| s.to_string_lossy()).collect());
        let mut msg = Self::new(pieces, entry.arg_ids.clone(), names);
        let mut control = false;
        let mut surrogate = false;
        for unit in entry.pieces.iter().flat_map(|p| p.units().iter().copied()) {
            // C0 controls except TAB, DEL and the C1 controls (all Unicode Cc).
            control |= (unit < 0x20 && unit != 0x09) || (0x7F..=0x9F).contains(&unit);
            surrogate |= (0xD800..=0xDFFF).contains(&unit);
        }
        if control {
            push_issue(&mut msg.issues, GrammarIssue::ControlCharacter);
        }
        if surrogate {
            push_issue(&mut msg.issues, GrammarIssue::Surrogate);
        }
        msg
    }

    /// A single-piece message (a StringInfo literal, spec §9 step 1).
    #[must_use]
    pub fn literal(text: &str) -> Self {
        Self::new(vec![text.to_owned()], Vec::new(), None)
    }

    /// Sets the variable name of a slot (used for names given in a format string).
    pub(crate) fn set_slot_name(&mut self, slot: usize, name: String) {
        if let Some(s) = self.slots.get_mut(slot) {
            s.name = Some(name);
        }
    }

    /// The parsed pieces.
    #[must_use]
    pub fn pieces(&self) -> &[Piece] {
        &self.pieces
    }

    /// The slots.
    #[must_use]
    pub fn slots(&self) -> &[Slot] {
        &self.slots
    }

    /// The slot bound to label `n`, if any (first binding wins).
    #[must_use]
    pub fn binding(&self, n: i64) -> Option<usize> {
        self.bindings.get(&n).copied()
    }

    /// Whether the message carries `{{keepspaces}}`.
    #[must_use]
    pub fn keep_spaces(&self) -> bool {
        self.keep_spaces
    }

    /// Grammar-invariant violations found while parsing (spec §12). Empty for every
    /// observed entry.
    #[must_use]
    pub fn issues(&self) -> &[GrammarIssue] {
        &self.issues
    }

    /// The work units one render of this message costs, beyond the text it outputs: a
    /// fixed amount per slot, piece and segment plus one per byte of piece text. Charged
    /// against [`crate::RenderOptions::max_work`] each time the message is rendered.
    #[must_use]
    pub(crate) fn render_cost(&self) -> usize {
        self.cost
    }

    /// Distinct argument IDs used by the message, in slot order.
    #[must_use]
    pub fn arg_ids(&self) -> Vec<u32> {
        let mut seen = std::collections::HashSet::new();
        self.slots
            .iter()
            .filter_map(|s| s.arg_id)
            .filter(|id| seen.insert(*id))
            .collect()
    }

    /// The parts in rendering order: piece 0, slot 0, piece 1, …; extra slots follow the
    /// last piece.
    pub fn parts(&self) -> impl Iterator<Item = Part> + '_ {
        let p = self.pieces.len();
        let s = self.slots.len();
        (0..p)
            .flat_map(move |k| {
                std::iter::once(Part::Piece(k)).chain((k < s).then_some(Part::Slot(k)))
            })
            .chain((p..s).map(Part::Slot))
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn msg(pieces: &[&str], ids: &[u32]) -> Message {
        Message::new(
            pieces.iter().map(|s| (*s).to_owned()).collect(),
            ids.to_vec(),
            None,
        )
    }

    #[test]
    fn labels() {
        assert_eq!(
            strip_label("You hand it to #1:"),
            Some((
                "You hand it to ",
                Label {
                    number: 1,
                    caret: false
                }
            ))
        );
        assert_eq!(
            strip_label("#-2:^").map(|(_, l)| (l.number, l.caret)),
            Some((-2, true))
        );
        assert_eq!(strip_label("x#:"), None);
        assert_eq!(strip_label("x1:"), None);
        assert_eq!(strip_label("x#1"), None);
        assert_eq!(strip_label(&format!("#{}:", "9".repeat(30))), None);
    }

    #[test]
    fn label_binding_not_slot_index() {
        let m = msg(&["a ", " b #1:", " c #1:{x[E]}"], &[10, 20]);
        assert_eq!(m.binding(1), Some(1));
        assert_eq!(m.slots()[1].label.map(|l| l.number), Some(1));
        assert!(m.issues().is_empty(), "{:?}", m.issues());
    }

    #[test]
    fn last_piece_label_is_literal() {
        let m = msg(&["a #1:"], &[]);
        assert_eq!(m.pieces()[0].segments, vec![Segment::Text("a #1:".into())]);
        assert!(m.issues().contains(&GrammarIssue::StrayLabel));
    }

    #[test]
    fn tokenizer() {
        let m = msg(
            &["#1:{The [!n]}#1:", " x {her[f]|him[m]} {{keepspaces}}"],
            &[5],
        );
        let segs = &m.pieces()[0].segments;
        assert!(matches!(&segs[0], Segment::Selector(s) if s.label == Some(1)));
        let segs = &m.pieces()[1].segments;
        assert_eq!(segs.len(), 4);
        assert!(matches!(&segs[1], Segment::Selector(s) if s.label.is_none()));
        assert_eq!(segs[3], Segment::KeepSpaces);
        assert!(m.keep_spaces());
        assert!(m.issues().is_empty(), "{:?}", m.issues());
    }

    #[test]
    fn literal_braces_and_unterminated() {
        let m = msg(&["{plain} #1:{open", ""], &[1]);
        let segs = &m.pieces()[0].segments;
        assert_eq!(segs, &vec![Segment::Text("{plain} #1:{open".into())]);
        assert!(m.issues().contains(&GrammarIssue::UnterminatedSelector));
        // Escapes do not protect grammar characters (tokenising precedes decoding).
        let m = msg(&[r"\{x[E]}", ""], &[1]);
        assert_eq!(m.pieces()[0].segments.len(), 2);
        assert!(matches!(&m.pieces()[0].segments[1], Segment::Selector(s) if s.label.is_none()));
        let m = msg(&["{{other}}", ""], &[1]);
        assert_eq!(
            m.pieces()[0].segments,
            vec![Segment::Text("{{other}}".into())]
        );
        let m = msg(&["{{", ""], &[1]);
        assert_eq!(m.pieces()[0].segments, vec![Segment::Text("{{".into())]);
    }

    #[test]
    fn selector_body_with_open_brace() {
        // string-table.md §4.3: the body is everything up to the first '}'.
        let m = msg(&["#1:", " #1:{a{b|c[f]}"], &[1]);
        let segs = &m.pieces()[1].segments;
        assert_eq!(segs.len(), 2);
        let Segment::Selector(sel) = &segs[1] else {
            panic!("{segs:?}")
        };
        assert_eq!(sel.raw, "#1:{a{b|c[f]}");
        assert_eq!(sel.options.len(), 2);
        assert_eq!(sel.options[0].text, "a{b");
        assert!(m.issues().contains(&GrammarIssue::UnterminatedSelector));
        let m = msg(&["{x{y[f]}z", ""], &[1]);
        let segs = &m.pieces()[0].segments;
        assert!(matches!(&segs[0], Segment::Selector(s) if s.raw == "{x{y[f]}"));
        assert_eq!(segs[1], Segment::Text("z".into()));
    }

    #[test]
    fn many_selector_openers_tokenise_fast() {
        // Openers whose bodies all run to one far '}' (or to none) stay linear.
        for tail in ["", "[f]}", "}"] {
            for unit in ["#1:{", "{a", "{{"] {
                let piece = unit.repeat(200_000) + tail;
                let start = std::time::Instant::now();
                let m = msg(&[&piece, ""], &[1]);
                assert!(!m.pieces()[0].segments.is_empty());
                assert!(
                    start.elapsed().as_secs() < 10,
                    "{unit:?}{tail:?} took {:?}",
                    start.elapsed()
                );
            }
        }
    }

    #[test]
    fn issues_detected() {
        let m = msg(&["#3:{x[E]}"], &[]);
        assert!(m.issues().contains(&GrammarIssue::SelectorWithoutSlots));
        assert!(m.issues().contains(&GrammarIssue::UndefinedLabel(3)));
        let m = msg(&["#1:", "#1:", ""], &[1, 2]);
        assert!(m.issues().contains(&GrammarIssue::LabelConflict(1)));
        let m = msg(&["{{keepspaces}} x"], &[]);
        assert!(m.issues().contains(&GrammarIssue::KeepSpacesNotAtEnd));
        let m = msg(&["${X}"], &[]);
        assert!(m.issues().contains(&GrammarIssue::DollarBrace));
    }

    #[test]
    fn bare_caret_before_slot() {
        // Regression: a bare '^' ending a piece before a slot is the modifier.
        let m = msg(&["{The[!n]} ^", " defeated you."], &[7]);
        assert!(m.slots()[0].caret);
        assert_eq!(m.slots()[0].label, None);
        assert_eq!(
            m.pieces()[0].segments.last(),
            Some(&Segment::Text(" ".into()))
        );
        // With a label, the caret is recorded on both.
        let m = msg(&["#1:^", ""], &[7]);
        assert!(m.slots()[0].caret);
        assert_eq!(m.slots()[0].label.map(|l| l.caret), Some(true));
        // In the last piece (no slot follows) it stays literal.
        let m = msg(&["a", "b^"], &[7]);
        assert!(!m.slots()[0].caret);
        assert_eq!(m.pieces()[1].segments, vec![Segment::Text("b^".into())]);
    }

    #[test]
    fn escaped_hash_reads_like_unescaped() {
        // Regression: label stripping and tokenising agree on `\#`.
        let m = msg(&[r"x\#1:", r" \#1:{she[f]|he}"], &[7]);
        assert_eq!(m.binding(1), Some(0));
        let segs = &m.pieces()[1].segments;
        assert!(matches!(&segs[1], Segment::Selector(s) if s.label == Some(1)));
    }

    #[test]
    fn c1_and_del_are_control_characters() {
        use tessera_formats::StringTableEntry;
        for c in ['\u{7F}', '\u{80}', '\u{9F}'] {
            let e = StringTableEntry {
                token: 1,
                token_high: 0,
                pieces: vec![format!("a{c}").as_str().into()],
                arg_ids: Vec::new(),
                arg_name_groups: Vec::new(),
            };
            let m = Message::from_entry(&e);
            assert!(
                m.issues().contains(&GrammarIssue::ControlCharacter),
                "{c:?}"
            );
        }
        let e = StringTableEntry {
            token: 1,
            token_high: 0,
            pieces: vec!["a\tb\u{A0}".into()],
            arg_ids: Vec::new(),
            arg_name_groups: Vec::new(),
        };
        assert!(Message::from_entry(&e).issues().is_empty());
    }

    #[test]
    fn parts_order() {
        let m = msg(&["a", "b", "c"], &[1, 2]);
        let parts: Vec<Part> = m.parts().collect();
        assert_eq!(
            parts,
            vec![
                Part::Piece(0),
                Part::Slot(0),
                Part::Piece(1),
                Part::Slot(1),
                Part::Piece(2)
            ]
        );
        let m = msg(&["a", "b"], &[1, 2, 3]);
        let parts: Vec<Part> = m.parts().collect();
        assert_eq!(parts.len(), 5);
        assert_eq!(parts[4], Part::Slot(2));
        let m = msg(&[], &[]);
        assert_eq!(m.parts().count(), 0);
        let m = msg(&["a", "b", "c"], &[7]);
        assert_eq!(m.slots()[1].arg_id, None);
    }
}
