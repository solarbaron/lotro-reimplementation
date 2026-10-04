// SPDX-License-Identifier: GPL-3.0-or-later
//! Rendering a message with a value map (`docs/specs/string-table.md` §7).
//!
//! Rendering never fails. Malformed constructs fall back to literal text or an absent
//! value; resource limits (nesting depth, output size) are reported as [`Diagnostic`]s.

use std::borrow::Cow;
use std::collections::HashMap;
use std::sync::Arc;

use crate::markup::{Link, MAX_LINK_FIELDS, MarkupState, RichText, Style, interpret, parse_markup};
use crate::message::{Message, Part, Segment};
use crate::tags::{TagSet, select_option, split_tag_group};
use crate::value::{NameDictionary, Value, Values, format_float};

/// Default nesting limit for message values (records.md §4.2 suggests 16).
pub const DEFAULT_MAX_DEPTH: usize = 16;
/// Default cap on the rendered size of one message, in bytes.
pub const DEFAULT_MAX_OUTPUT: usize = 1 << 20;
/// Maximum number of diagnostics kept per render.
const MAX_DIAGNOSTICS: usize = 64;

/// How missing values and lookup misses are shown (spec §8.3, §9).
#[derive(Debug, Clone, Copy, Default, PartialEq, Eq, Hash)]
pub enum RenderMode {
    /// Missing values render as empty text; misses render as the empty string.
    #[default]
    Plain,
    /// Missing values render as `${name}` placeholders and misses as a visible
    /// placeholder, both with [`Style::placeholder`] set.
    Explorer,
}

/// Rendering options.
#[derive(Debug, Clone, Copy)]
pub struct RenderOptions<'a> {
    /// Plain or explorer mode.
    pub mode: RenderMode,
    /// Name dictionary for placeholders (explorer mode).
    pub names: Option<&'a NameDictionary>,
    /// Maximum number of message levels, the outermost counting as 1. A nested message
    /// past the limit renders as absent with [`Diagnostic::DepthLimit`].
    pub max_depth: usize,
    /// Maximum number of text bytes produced across all nesting levels (nested raw text
    /// counts once per level it passes through); text past it is dropped with
    /// [`Diagnostic::OutputLimit`]. Guards against exponential growth when one nested
    /// value fills several slots at every level.
    pub max_output: usize,
}

impl Default for RenderOptions<'_> {
    fn default() -> Self {
        Self {
            mode: RenderMode::Plain,
            names: None,
            max_depth: DEFAULT_MAX_DEPTH,
            max_output: DEFAULT_MAX_OUTPUT,
        }
    }
}

impl<'a> RenderOptions<'a> {
    /// Default options in the given mode.
    #[must_use]
    pub fn mode(mode: RenderMode) -> Self {
        Self {
            mode,
            ..Self::default()
        }
    }

    /// Explorer mode with a name dictionary.
    #[must_use]
    pub fn explorer(names: Option<&'a NameDictionary>) -> Self {
        Self {
            mode: RenderMode::Explorer,
            names,
            ..Self::default()
        }
    }
}

/// Why a StringInfo reference did not resolve (spec §9 step 2).
#[derive(Debug, Clone, PartialEq, Eq, Hash, thiserror::Error)]
#[non_exhaustive]
pub enum MissReason {
    /// The table DID's prefix is not 0x25.
    #[error("not a string-table DID")]
    NotStringTable,
    /// No such table in the selected locale.
    #[error("no such table in the locale")]
    NoTable,
    /// The table exists but has no such token.
    #[error("no such token in the table")]
    NoToken,
    /// The table could not be read or decoded (the message describes why).
    #[error("table unreadable: {0}")]
    Unreadable(String),
}

/// A StringInfo lookup miss. Not an error when rendering: it renders as empty text
/// (plain) or a placeholder (explorer). It is the error type of the lookups that return
/// a message ([`crate::Strings::message`]).
#[derive(Debug, Clone, PartialEq, Eq, Hash, thiserror::Error)]
#[error("string lookup miss: token {token:#010x} in table {table:#010x}: {reason}")]
pub struct Miss {
    /// Table DID.
    pub table: u32,
    /// Token.
    pub token: u32,
    /// Why it missed.
    pub reason: MissReason,
}

impl Miss {
    /// The explorer placeholder text, showing the table and token in hex.
    #[must_use]
    pub fn placeholder(&self) -> String {
        format!("[missing {:#010X}:{:#010X}]", self.table, self.token)
    }
}

/// Something worth reporting about one render. Rendering still produced text.
#[derive(Debug, Clone, PartialEq, Eq, Hash, thiserror::Error)]
#[non_exhaustive]
pub enum Diagnostic {
    /// A nested message exceeded the depth limit and rendered as absent.
    #[error("message nesting limit {limit} reached; the value rendered as absent")]
    DepthLimit {
        /// The limit in effect.
        limit: usize,
    },
    /// The output reached the size limit and was truncated.
    #[error("output truncated at {limit} bytes")]
    OutputLimit {
        /// The limit in effect.
        limit: usize,
    },
    /// A StringInfo reference (outermost or nested) did not resolve.
    #[error("{0}")]
    Miss(Miss),
}

/// The result of rendering.
#[derive(Debug, Clone, Default, PartialEq, Eq)]
pub struct Rendered {
    /// The rich text.
    pub text: RichText,
    /// Diagnostics, at most a bounded number.
    pub diagnostics: Vec<Diagnostic>,
    /// The message's own trailing tag group (spec §4.9), removed from the display text:
    /// the stored tags of a message meant to be inserted into other messages.
    pub tags: Option<String>,
}

impl Rendered {
    /// The text without styles.
    #[must_use]
    pub fn plain_text(&self) -> String {
        self.text.plain_text()
    }

    /// Whether nothing was reported.
    #[must_use]
    pub fn is_clean(&self) -> bool {
        self.diagnostics.is_empty()
    }

    /// The first miss reported, if any.
    #[must_use]
    pub fn miss(&self) -> Option<&Miss> {
        self.diagnostics.iter().find_map(|d| match d {
            Diagnostic::Miss(m) => Some(m),
            _ => None,
        })
    }
}

/// Total bytes of field text kept in a link opener assembled from slot values; the rest
/// is dropped (the annotation is inert and never displayed).
const MAX_LINK_FIELD_BYTES: usize = 4096;

/// What a prepared value shows.
#[derive(Debug)]
enum Display {
    /// Raw text (tag group removed; escapes and markup still encoded), interpreted in its
    /// own markup scope.
    Raw(String),
    /// A nested message, already rendered (each of its values decoded separately).
    Rich(RichText),
    /// An absent value: empty (plain) or the `${name}` placeholder (explorer).
    Absent(String),
}

/// A value prepared for one or more slots (spec §7 step 2).
#[derive(Debug)]
struct Prepared {
    display: Display,
    tags: TagSet,
    /// No text apart from explorer placeholders: what plain mode would show is empty.
    blank: bool,
}

impl Prepared {
    fn is_absent(&self) -> bool {
        matches!(self.display, Display::Absent(_))
    }
}

/// The prepared values of one message: one entry per distinct present argument ID (shared
/// by every slot with that ID) plus one per absent slot.
#[derive(Debug, Default)]
struct Slots {
    prepared: Vec<Prepared>,
    /// Slot index → index into `prepared`.
    map: Vec<usize>,
}

impl Slots {
    fn add(&mut self, p: Prepared) -> usize {
        self.prepared.push(p);
        self.prepared.len() - 1
    }

    fn get(&self, slot: usize) -> Option<&Prepared> {
        self.map.get(slot).and_then(|&j| self.prepared.get(j))
    }
}

/// An item of the evaluated message (after selector evaluation).
#[derive(Debug)]
enum Item<'m> {
    /// Raw literal text from a piece or a chosen option. `inserted` marks selector output.
    Lit { raw: Cow<'m, str>, inserted: bool },
    /// The value of a slot.
    Slot(usize),
    /// A link opener assembled from a piece split by slots (spec §4.8).
    Link(Arc<Link>),
}

pub(crate) struct Renderer<'o> {
    opts: &'o RenderOptions<'o>,
    budget: usize,
    diags: Vec<Diagnostic>,
    top_tags: Option<String>,
}

impl<'o> Renderer<'o> {
    pub(crate) fn new(opts: &'o RenderOptions<'o>) -> Self {
        Self {
            opts,
            budget: opts.max_output,
            diags: Vec::new(),
            top_tags: None,
        }
    }

    /// Records a diagnostic; returns whether it is new (not yet reported in this render).
    pub(crate) fn diag(&mut self, d: Diagnostic) -> bool {
        if self.diags.len() < MAX_DIAGNOSTICS && !self.diags.contains(&d) {
            self.diags.push(d);
            true
        } else {
            false
        }
    }

    /// Records a diagnostic and logs it only the first time it is seen in this render, so
    /// a value shared by many slots cannot flood the log.
    fn report(&mut self, d: Diagnostic) {
        if !self.diag(d.clone()) {
            return;
        }
        match &d {
            Diagnostic::DepthLimit { limit } => {
                tracing::warn!(limit, "message nesting limit reached");
            }
            Diagnostic::Miss(m) => tracing::debug!(
                table = format_args!("{:#010x}", m.table),
                token = format_args!("{:#010x}", m.token),
                reason = %m.reason,
                "nested string lookup miss"
            ),
            Diagnostic::OutputLimit { .. } => {}
        }
    }

    pub(crate) fn finish(self, text: RichText) -> Rendered {
        Rendered {
            text,
            diagnostics: self.diags,
            tags: self.top_tags,
        }
    }

    /// Charges `n` bytes against the output budget; returns how many may be kept.
    fn charge(&mut self, n: usize) -> usize {
        if n <= self.budget {
            self.budget -= n;
            n
        } else {
            let keep = self.budget;
            self.budget = 0;
            self.diag(Diagnostic::OutputLimit {
                limit: self.opts.max_output,
            });
            keep
        }
    }

    fn placeholder_name(&self, msg: &Message, slot: usize) -> String {
        let s = &msg.slots()[slot];
        if let Some(name) = &s.name {
            return name.clone();
        }
        let Some(id) = s.arg_id else {
            return "?".to_owned();
        };
        if let Some(name) = self.opts.names.and_then(|d| d.get(id)) {
            return name.to_owned();
        }
        format!("#{id:07X}")
    }

    fn absent(&self, msg: &Message, slot: usize) -> Prepared {
        let display = match self.opts.mode {
            RenderMode::Plain => String::new(),
            RenderMode::Explorer => format!("${{{}}}", self.placeholder_name(msg, slot)),
        };
        Prepared {
            display: Display::Absent(display),
            tags: TagSet::absent(),
            blank: true,
        }
    }

    fn prepare_text(raw: &str) -> Prepared {
        let (display, group) = split_tag_group(raw);
        let plain = parse_markup(display).plain_text();
        let number = parse_number(&plain);
        Prepared {
            display: Display::Raw(display.to_owned()),
            tags: TagSet::new(group.unwrap_or(""), plain.is_empty(), number),
            blank: plain.is_empty(),
        }
    }

    fn prepare(
        &mut self,
        msg: &Message,
        slot: usize,
        value: Option<&Value>,
        depth: usize,
    ) -> Prepared {
        match value {
            None => self.absent(msg, slot),
            Some(Value::Text(raw)) => Self::prepare_text(raw),
            Some(Value::Integer(n)) => Prepared {
                display: Display::Raw(n.to_string()),
                tags: TagSet::new("", false, Some(*n as f64)),
                blank: false,
            },
            Some(Value::Float(f)) => Prepared {
                display: Display::Raw(format_float(*f)),
                tags: TagSet::new("", false, Some(*f)),
                blank: false,
            },
            Some(Value::Unresolved(d)) => {
                self.report(d.clone());
                self.absent(msg, slot)
            }
            Some(Value::Message(mv)) => {
                if depth + 1 > self.opts.max_depth {
                    self.report(Diagnostic::DepthLimit {
                        limit: self.opts.max_depth,
                    });
                    return self.absent(msg, slot);
                }
                let (rich, group) = self.render_message(&mv.message, &mv.values, depth + 1, false);
                // Selection state is what plain mode would see: explorer placeholders
                // inside the nested message count as empty (spec §8.3).
                let plain = rich.text_without_placeholders();
                let number = parse_number(&plain);
                Prepared {
                    tags: TagSet::new(group.as_deref().unwrap_or(""), plain.is_empty(), number),
                    blank: plain.is_empty(),
                    display: Display::Rich(rich),
                }
            }
        }
    }

    /// Prepares every slot. A present value is prepared once per argument ID and shared by
    /// every slot with that ID; absent slots get their own (small) placeholder, and an ID
    /// whose value turned out absent is not prepared again.
    fn prepare_all(&mut self, msg: &Message, values: &Values, depth: usize) -> Slots {
        let mut by_id: HashMap<u32, Option<usize>> = HashMap::new();
        let mut slots = Slots {
            prepared: Vec::new(),
            map: Vec::with_capacity(msg.slots().len()),
        };
        for (i, slot) in msg.slots().iter().enumerate() {
            let j = match slot.arg_id {
                Some(id) => match by_id.get(&id).copied() {
                    Some(Some(j)) => j,
                    Some(None) => slots.add(self.absent(msg, i)),
                    None => {
                        let p = self.prepare(msg, i, values.get(id), depth);
                        let absent = p.is_absent();
                        let j = slots.add(p);
                        by_id.insert(id, (!absent).then_some(j));
                        j
                    }
                },
                None => slots.add(self.absent(msg, i)),
            };
            slots.map.push(j);
        }
        slots
    }

    /// Evaluates selectors (spec §7 steps 1, 3, 4).
    fn evaluate<'m>(msg: &'m Message, slots: &Slots) -> Vec<Item<'m>> {
        let absent = TagSet::absent();
        let mut items = Vec::new();
        for part in msg.parts() {
            match part {
                Part::Slot(i) => items.push(Item::Slot(i)),
                Part::Piece(k) => {
                    for seg in &msg.pieces()[k].segments {
                        match seg {
                            Segment::Text(t) => items.push(Item::Lit {
                                raw: Cow::Borrowed(t),
                                inserted: false,
                            }),
                            Segment::KeepSpaces => {}
                            Segment::Selector(sel) => {
                                let slot = match sel.label {
                                    Some(n) => msg.binding(n),
                                    None if msg.slots().is_empty() => {
                                        // No slot to bind to: literal text (spec §4.4).
                                        items.push(Item::Lit {
                                            raw: Cow::Borrowed(&sel.raw),
                                            inserted: false,
                                        });
                                        continue;
                                    }
                                    // Nearest preceding slot, or slot 0 (spec §4.4 OPEN).
                                    None => Some(k.saturating_sub(1)),
                                };
                                let tags =
                                    slot.and_then(|s| slots.get(s)).map_or(&absent, |p| &p.tags);
                                let text = select_option(&sel.options, tags)
                                    .and_then(|i| sel.options.get(i))
                                    .map_or("", |o| o.text.as_str());
                                items.push(Item::Lit {
                                    raw: Cow::Borrowed(text),
                                    inserted: true,
                                });
                            }
                        }
                    }
                }
            }
        }
        items
    }

    /// Renders one message to rich text (spec §7 steps 1–7) and returns it with the
    /// message's own trailing tag group. `top` adds step 8 (whitespace normalisation),
    /// which applies to the outermost message only.
    fn render_message(
        &mut self,
        msg: &Message,
        values: &Values,
        depth: usize,
        top: bool,
    ) -> (RichText, Option<String>) {
        let slots = self.prepare_all(msg, values, depth);
        let mut items = Self::evaluate(msg, &slots);
        let tags = strip_trailing_group(&mut items, &slots);
        let items = merge_split_link_openers(items, &slots);
        let base = Style::default();
        let mut state = MarkupState::new();
        let normalise = top && !msg.keep_spaces();
        let mut out = RichText::new();
        let mut prev_inserted = false;
        for item in &items {
            let mut tmp = RichText::new();
            let inserted = match item {
                Item::Lit { raw, inserted } => {
                    interpret(raw, &mut state, &base, &mut tmp);
                    *inserted
                }
                Item::Link(link) => {
                    state.open_link(Arc::clone(link));
                    continue;
                }
                Item::Slot(i) => {
                    let outer = state.style(&base);
                    match slots.get(*i).map(|p| &p.display) {
                        None => {}
                        Some(Display::Absent(s)) => {
                            let style = Style {
                                placeholder: true,
                                ..outer
                            };
                            tmp.push_str(s, &style);
                        }
                        // Value markup is scoped to the value (spec §7 step 6).
                        Some(Display::Raw(s)) => {
                            interpret(s, &mut MarkupState::new(), &outer, &mut tmp);
                        }
                        Some(Display::Rich(rt)) => {
                            for span in rt.spans() {
                                tmp.push_str(&span.text, &outer.overlay(&span.style));
                            }
                        }
                    }
                    true
                }
            };
            // Junction space collapse (spec §7 step 8a).
            if normalise
                && (inserted || prev_inserted)
                && out.last_char() == Some(' ')
                && tmp.first_char() == Some(' ')
            {
                tmp.remove_first_char();
            }
            let len = tmp.len();
            let keep = self.charge(len);
            if keep < len {
                tmp.truncate_bytes(keep);
                out.append(tmp);
                break;
            }
            out.append(tmp);
            prev_inserted = inserted;
        }
        if normalise {
            out.trim_spaces();
        }
        (out, tags)
    }

    /// Renders the outermost message to rich text (all steps including step 8).
    pub(crate) fn render_top(&mut self, msg: &Message, values: &Values) -> RichText {
        let (text, tags) = self.render_message(msg, values, 1, true);
        self.top_tags = tags;
        text
    }
}

/// Whether `s` (the text after `<`) can be the start of a link opener `Kind:Type:…` that
/// continues past the end of the piece.
fn is_link_opener_prefix(s: &str) -> bool {
    let mut parts = s.split(':');
    let ident = |p: &str| {
        let mut b = p.bytes();
        b.next().is_some_and(|c| c.is_ascii_alphabetic()) && b.all(|c| c.is_ascii_alphanumeric())
    };
    matches!((parts.next(), parts.next()), (Some(k), Some(t)) if ident(k) && ident(t))
        && parts.next().is_some()
        && !s.contains(['<', '>'])
}

/// Finds the `>` that closes a split link opener: the index of the item and the byte
/// offset of the `>` in it. Slots in between are skipped; literal text with a `<` before
/// any `>` (or an already assembled link) means there is no closer.
fn find_opener_close(items: &[Item<'_>]) -> Option<(usize, usize)> {
    for (j, item) in items.iter().enumerate() {
        match item {
            Item::Slot(_) => {}
            Item::Link(_) => return None,
            Item::Lit { raw, .. } => {
                let gt = raw.find('>');
                let lt = raw.find('<');
                match gt {
                    Some(g) if lt.is_none_or(|l| g < l) => return Some((j, g)),
                    _ if lt.is_some() => return None,
                    _ => {}
                }
            }
        }
    }
    None
}

/// Builds a link annotation from a split opener without re-parsing concatenated text, so
/// slot values can never end the opener or add fields.
struct LinkBuilder {
    kind: String,
    link_type: String,
    fields: Vec<String>,
    room: usize,
    full: bool,
}

impl LinkBuilder {
    /// `prefix` is the opener text after `<` (checked by [`is_link_opener_prefix`]).
    fn new(prefix: &str) -> Self {
        let mut parts = prefix.splitn(3, ':');
        let kind = parts.next().unwrap_or_default().to_owned();
        let link_type = parts.next().unwrap_or_default().to_owned();
        let mut b = Self {
            kind,
            link_type,
            fields: vec![String::new()],
            room: MAX_LINK_FIELD_BYTES,
            full: false,
        };
        b.push_literal(parts.next().unwrap_or_default());
        b
    }

    /// Appends literal opener text: `:` starts a new field.
    fn push_literal(&mut self, s: &str) {
        for (n, part) in s.split(':').enumerate() {
            if self.full {
                return;
            }
            if n > 0 {
                if self.fields.len() >= MAX_LINK_FIELDS {
                    self.full = true;
                    return;
                }
                self.fields.push(String::new());
            }
            self.append(part);
        }
    }

    /// Appends text to the current field, within the byte budget.
    fn append(&mut self, s: &str) {
        let mut keep = s.len().min(self.room);
        while !s.is_char_boundary(keep) {
            keep -= 1;
        }
        if let Some(last) = self.fields.last_mut() {
            last.push_str(&s[..keep]);
        }
        self.room -= keep;
    }

    /// Appends a slot value's display text (decoded, without markup) to the current
    /// field, verbatim: a `:` or `>` in a value is field text.
    fn push_value(&mut self, p: &Prepared) {
        if self.full || self.room == 0 {
            return;
        }
        match &p.display {
            Display::Absent(_) => {}
            Display::Raw(s) => {
                let mut cut = s.len().min(self.room.saturating_mul(2));
                while !s.is_char_boundary(cut) {
                    cut -= 1;
                }
                let text = parse_markup(&s[..cut]).plain_text();
                self.append(&text);
            }
            Display::Rich(rt) => {
                for span in rt.spans().iter().filter(|s| !s.style.placeholder) {
                    if self.room == 0 {
                        break;
                    }
                    self.append(&span.text);
                }
            }
        }
    }

    fn finish(self) -> Link {
        Link {
            kind: self.kind,
            link_type: self.link_type,
            fields: self.fields,
        }
    }
}

/// The sub-slice `range` of a borrowed-or-owned literal, keeping a borrow when possible.
fn cow_slice<'m>(c: &Cow<'m, str>, range: std::ops::RangeFrom<usize>) -> Cow<'m, str> {
    match c {
        Cow::Borrowed(b) => Cow::Borrowed(&b[range]),
        Cow::Owned(o) => Cow::Owned(o[range].to_owned()),
    }
}

/// Joins link openers whose fields are filled by slots (spec §4.8: in stored templates the
/// field after the Type is a slot, so the opener `<Kind:Type:` ends one piece and its `>`
/// starts a later one). The slot values become the opener's field text. The closer is
/// found before anything is built, and field text is bounded.
fn merge_split_link_openers<'m>(mut items: Vec<Item<'m>>, slots: &Slots) -> Vec<Item<'m>> {
    let mut out: Vec<Item<'m>> = Vec::with_capacity(items.len());
    let empty = || Item::Lit {
        raw: Cow::Borrowed(""),
        inserted: false,
    };
    let mut i = 0;
    while i < items.len() {
        let item = std::mem::replace(&mut items[i], empty());
        i += 1;
        let Item::Lit { raw, inserted } = item else {
            out.push(item);
            continue;
        };
        let start = raw
            .rfind('<')
            .filter(|&p| is_link_opener_prefix(&raw[p + 1..]))
            .filter(|_| matches!(items.get(i), Some(Item::Slot(_))));
        let close = start.and_then(|_| find_opener_close(&items[i..]));
        let (Some(start), Some((offset, gt))) = (start, close) else {
            out.push(Item::Lit { raw, inserted });
            continue;
        };
        let close = i + offset;
        let mut link = LinkBuilder::new(&raw[start + 1..]);
        for it in &items[i..close] {
            match it {
                Item::Slot(s) => {
                    if let Some(p) = slots.get(*s) {
                        link.push_value(p);
                    }
                }
                Item::Lit { raw: r, .. } => link.push_literal(r),
                Item::Link(_) => {}
            }
        }
        let rest = match &items[close] {
            Item::Lit { raw: r, inserted } => {
                link.push_literal(&r[..gt]);
                Item::Lit {
                    raw: cow_slice(r, gt + 1..),
                    inserted: *inserted,
                }
            }
            _ => empty(),
        };
        let prefix = match raw {
            Cow::Borrowed(b) => Cow::Borrowed(&b[..start]),
            Cow::Owned(mut o) => {
                o.truncate(start);
                Cow::Owned(o)
            }
        };
        out.push(Item::Lit {
            raw: prefix,
            inserted,
        });
        out.push(Item::Link(Arc::new(link.finish())));
        // The text after '>' may itself end with another split opener.
        items[close] = rest;
        i = close;
    }
    out
}

/// Removes the message's own trailing tag group (spec §4.9) from the last item with text
/// when that item is literal piece text, and returns the group. Values that plain mode
/// would show as empty (absent values, explorer placeholders) do not count as text.
fn strip_trailing_group(items: &mut [Item<'_>], slots: &Slots) -> Option<String> {
    let pos = items.iter().rposition(|it| match it {
        Item::Lit { raw, .. } => !raw.is_empty(),
        Item::Slot(i) => slots.get(*i).is_some_and(|p| !p.blank),
        Item::Link(_) => false,
    })?;
    let Item::Lit {
        raw,
        inserted: false,
    } = &items[pos]
    else {
        return None;
    };
    let (display, group) = split_tag_group(raw);
    let group = group?.to_owned();
    let display = display.to_owned();
    items[pos] = Item::Lit {
        raw: Cow::Owned(display),
        inserted: false,
    };
    Some(group)
}

/// Parses text that is entirely an optionally signed decimal number (spec §5.4).
fn parse_number(s: &str) -> Option<f64> {
    let b = s.as_bytes();
    let mut i = usize::from(matches!(b.first(), Some(b'+' | b'-')));
    let int_start = i;
    while b.get(i).is_some_and(u8::is_ascii_digit) {
        i += 1;
    }
    if i == int_start {
        return None;
    }
    if b.get(i) == Some(&b'.') {
        i += 1;
        let frac = i;
        while b.get(i).is_some_and(u8::is_ascii_digit) {
            i += 1;
        }
        if i == frac {
            return None;
        }
    }
    if i != b.len() {
        return None;
    }
    s.parse::<f64>().ok()
}

/// Renders a message with a value map (spec §7). Never fails.
#[must_use]
pub fn render(msg: &Message, values: &Values, opts: &RenderOptions<'_>) -> Rendered {
    let mut r = Renderer::new(opts);
    let text = r.render_top(msg, values);
    r.finish(text)
}

/// Renders a message to plain text with default options (plain mode).
#[must_use]
pub fn render_plain(msg: &Message, values: &Values) -> String {
    render(msg, values, &RenderOptions::default()).plain_text()
}

/// The display text of a value string (for example an item name): the trailing tag group
/// is removed, escapes are decoded and markup is dropped.
#[must_use]
pub fn display_text(raw: &str) -> String {
    let (display, _) = split_tag_group(raw);
    parse_markup(display).plain_text()
}

#[cfg(test)]
mod tests {
    use super::*;
    use tessera_formats::name_hash;

    const A: u32 = 0x00AA_AAAA;
    const B: u32 = 0x00BB_BBBB;

    fn msg(pieces: &[&str], ids: &[u32]) -> Message {
        Message::new(
            pieces.iter().map(|s| (*s).to_owned()).collect(),
            ids.to_vec(),
            None,
        )
    }

    fn vals(pairs: &[(u32, Value)]) -> Values {
        pairs.iter().cloned().collect()
    }

    #[test]
    fn spec_end_to_end_example() {
        let m = msg(
            &[
                "#1:{The [!n]}#1:",
                " lifts #1:{her[f]|his[m]|its} lantern",
                ".",
            ],
            &[A, B],
        );
        let v = vals(&[(A, "Copper Golem[e]".into()), (B, "".into())]);
        assert_eq!(render_plain(&m, &v), "The Copper Golem lifts its lantern.");
        let v = vals(&[(A, "Wren Hollow[fn]".into())]);
        assert_eq!(render_plain(&m, &v), "Wren Hollow lifts her lantern.");
    }

    #[test]
    fn label_not_slot_index_and_selector_before_label() {
        // Selector before its label; label 1 sits on slot 1.
        let m = msg(&["#1:{She[f]|He[m]|They} met ", " and #1:", "."], &[A, B]);
        let v = vals(&[(A, "Tobin[mn]".into()), (B, "Wren[fn]".into())]);
        assert_eq!(render_plain(&m, &v), "She met Tobin and Wren.");
    }

    #[test]
    fn negative_label_and_caret() {
        let m = msg(&["#-1:{The [!n]}#-1:^", " waves."], &[A]);
        let v = vals(&[(A, "copper golem[e]".into())]);
        assert_eq!(render_plain(&m, &v), "The copper golem waves.");
    }

    #[test]
    fn undefined_label_is_absent() {
        let m = msg(&["x#9:{ [E]|-[b]}y", ""], &[A]);
        let v = vals(&[(A, "v".into())]);
        assert_eq!(render_plain(&m, &v), "x-yv");
    }

    #[test]
    fn bare_selectors() {
        let m = msg(&["", " waves at {her[f]|him[m]} and smiles."], &[A]);
        let v = vals(&[(A, "Wren[fn]".into())]);
        assert_eq!(render_plain(&m, &v), "Wren waves at her and smiles.");
        // Before any slot: binds slot 0.
        let m = msg(&["{She[f]|He[m]} says: ", ""], &[A]);
        assert_eq!(render_plain(&m, &v), "She says: Wren");
        // No slots: literal.
        let m = msg(&["a {b[f]} c"], &[]);
        assert_eq!(render_plain(&m, &Values::new()), "a {b[f]} c");
    }

    #[test]
    fn numeric_and_blank() {
        let m = msg(&["", " #1:{acorn[1]|acorns}"], &[A]);
        let m = {
            // Attach label 1 to slot 0.
            let mut p: Vec<String> = m.pieces().iter().map(|p| p.raw.clone()).collect();
            p[0] = "#1:".into();
            Message::new(p, vec![A], None)
        };
        assert_eq!(render_plain(&m, &vals(&[(A, 1i64.into())])), "1 acorn");
        assert_eq!(render_plain(&m, &vals(&[(A, 4i64.into())])), "4 acorns");
        assert_eq!(render_plain(&m, &vals(&[(A, "1".into())])), "1 acorn");
        assert_eq!(render_plain(&m, &vals(&[(A, 1.0f64.into())])), "1 acorn");
        assert_eq!(render_plain(&m, &vals(&[(A, 2.5f64.into())])), "2.5 acorns");
        // Time-format style: unit suppressed when blank.
        let m = msg(&["#1:", "#1:{h[!b]}"], &[A]);
        assert_eq!(render_plain(&m, &vals(&[(A, "".into())])), "");
        assert_eq!(render_plain(&m, &vals(&[(A, "3".into())])), "3h");
    }

    #[test]
    fn escapes_once_and_markup_scope() {
        // A value containing "\\n" must decode exactly once.
        let m = msg(&["a ", r"\n<rgb=#FF0000>b"], &[A]);
        let v = vals(&[(A, r"x\\n<b>bold".into())]);
        let r = render(&m, &v, &RenderOptions::default());
        assert_eq!(r.plain_text(), "a x\\nbold\nb");
        let spans = r.text.spans();
        // The value's <b> does not leak into the message text after it.
        let last = spans.last().expect("span");
        assert!(last.style.color.is_some() && !last.style.bold);
        assert!(spans.iter().any(|s| s.style.bold && s.text == "bold"));
    }

    #[test]
    fn whitespace() {
        // Junction collapse and trimming.
        let m2 = msg(&["You see#1:{ [n]| a} #1:", " ."], &[A]);
        let v = vals(&[(A, "Tobin[mn]".into())]);
        assert_eq!(render_plain(&m2, &v), "You see Tobin .");
        let v = vals(&[(A, "kettle[e]".into())]);
        assert_eq!(render_plain(&m2, &v), "You see a kettle .");
        // Authored double spaces are kept; leading/trailing trimmed.
        let m = msg(&["  a  b  "], &[]);
        assert_eq!(render_plain(&m, &Values::new()), "a  b");
        // keepspaces keeps them.
        let m = msg(&[" a ", "  {{keepspaces}}"], &[A]);
        let v = vals(&[(A, " v ".into())]);
        assert_eq!(render_plain(&m, &v), " a  v   ");
        // Empty value between spaces collapses.
        let m = msg(&["Hello ", " world"], &[A]);
        assert_eq!(render_plain(&m, &Values::new()), "Hello world");
        // Tabs and newlines untouched.
        let m = msg(&[r"\n a\t"], &[]);
        assert_eq!(render_plain(&m, &Values::new()), "\n a\t");
    }

    #[test]
    fn explorer_placeholders() {
        let names: NameDictionary = ["COUNT"].into_iter().collect();
        let count = name_hash(b"COUNT");
        let m = msg(&["#1:", " #1:{item[1]|items} from ", "."], &[count, B]);
        let r = render(&m, &Values::new(), &RenderOptions::explorer(Some(&names)));
        assert_eq!(r.plain_text(), "${COUNT} items from ${#0BBBBBB}.");
        assert!(r.text.spans()[0].style.placeholder);
        // Entry names win over the dictionary.
        let m = Message::new(
            vec!["x ".into(), String::new()],
            vec![count],
            Some(vec!["QTY".into()]),
        );
        let r = render(&m, &Values::new(), &RenderOptions::explorer(Some(&names)));
        assert_eq!(r.plain_text(), "x ${QTY}");
    }

    #[test]
    fn repeated_id_and_extra_ids() {
        let m = msg(&["", " and ", ""], &[A, A]);
        assert_eq!(render_plain(&m, &vals(&[(A, "x".into())])), "x and x");
        let m = msg(&["a", "b"], &[A, B, A]);
        assert_eq!(
            render_plain(&m, &vals(&[(A, "1".into()), (B, "2".into())])),
            "a1b21"
        );
        assert_eq!(render_plain(&msg(&[], &[]), &Values::new()), "");
    }

    fn chain(levels: usize) -> Value {
        let mut v = Value::Text("core[n]".into());
        for _ in 0..levels {
            let m = msg(&["(", ")"], &[A]);
            v = Value::message(m, vals(&[(A, v)]));
        }
        v
    }

    #[test]
    fn nested_messages_and_depth_limit() {
        let top = msg(&["", ""], &[A]);
        // 15 nested levels + top = 16 levels: fine.
        let r = render(&top, &vals(&[(A, chain(15))]), &RenderOptions::default());
        assert!(r.is_clean(), "{:?}", r.diagnostics);
        assert_eq!(
            r.plain_text(),
            format!("{}core{}", "(".repeat(15), ")".repeat(15))
        );
        // One past the limit: the innermost renders as absent.
        let r = render(&top, &vals(&[(A, chain(16))]), &RenderOptions::default());
        assert!(
            r.diagnostics
                .contains(&Diagnostic::DepthLimit { limit: 16 })
        );
        assert_eq!(
            r.plain_text(),
            format!("{}{}", "(".repeat(15), ")".repeat(15))
        );
    }

    #[test]
    fn nested_value_tags_and_selectors() {
        // A value that is a message with a selector and a trailing tag group.
        let inner = msg(&["#1:{the [!n]}#1:", "[e]"], &[B]);
        let inner_v = vals(&[(B, "lamp[e]".into())]);
        let top = msg(&["#1:", " #1:{glows[!p]|glow[p]}."], &[A]);
        let v = vals(&[(A, Value::message(inner, inner_v))]);
        assert_eq!(render_plain(&top, &v), "the lamp glows.");
    }

    #[test]
    fn exponential_growth_is_bounded() {
        let mut v = Value::Text("abcdefgh".into());
        for _ in 0..15 {
            let m = msg(&["", "", "", "", ""], &[A, A, A, A]);
            v = Value::message(m, vals(&[(A, v)]));
        }
        let top = msg(&["", ""], &[A]);
        let opts = RenderOptions {
            max_output: 4096,
            ..RenderOptions::default()
        };
        let r = render(&top, &vals(&[(A, v)]), &opts);
        assert!(r.text.len() <= 4096);
        assert!(
            r.diagnostics
                .contains(&Diagnostic::OutputLimit { limit: 4096 })
        );
    }

    #[test]
    fn own_tag_group_removed() {
        let r = render(
            &msg(&["Reed Boots [p] "], &[]),
            &Values::new(),
            &RenderOptions::default(),
        );
        assert_eq!(r.plain_text(), "Reed Boots");
        assert_eq!(r.tags.as_deref(), Some("p"));
        // A group produced by a value is the value's, not the message's.
        let r = render(
            &msg(&["x ", ""], &[A]),
            &vals(&[(A, "y[m]".into())]),
            &RenderOptions::default(),
        );
        assert_eq!(r.plain_text(), "x y");
        assert_eq!(r.tags, None);
        let r = render(
            &msg(&["", " the lamp[e]"], &[A]),
            &Values::new(),
            &RenderOptions::default(),
        );
        assert_eq!(r.plain_text(), "the lamp");
    }

    #[test]
    fn link_opener_split_by_slot() {
        let m = msg(&["<Select:IID:", ">", r"<\\Select> waves."], &[A, B]);
        let v = vals(&[(A, "0x1234".into()), (B, "Wren[fn]".into())]);
        let r = render(&m, &v, &RenderOptions::default());
        assert_eq!(r.plain_text(), "Wren waves.");
        let link = r.text.spans()[0].style.link.as_ref().expect("link");
        assert_eq!(link.kind, "Select");
        assert_eq!(link.fields, vec!["0x1234".to_owned()]);
        // Explorer: the placeholder label carries the link annotation.
        let r = render(&m, &Values::new(), &RenderOptions::explorer(None));
        assert_eq!(r.plain_text(), "${#0BBBBBB} waves.");
        assert!(r.text.spans()[0].style.link.is_some());
        // Unclosed opener stays literal.
        let m = msg(&["<Select:IID:", " no close"], &[A]);
        assert_eq!(
            render_plain(&m, &vals(&[(A, "x".into())])),
            "<Select:IID:x no close"
        );
    }

    #[test]
    fn bare_caret_is_not_rendered() {
        // Regression: a bare '^' before a slot never reaches the output.
        let m = msg(&["{The[!n]} ^", " defeated you."], &[A]);
        let v = vals(&[(A, "Wren Hollow[fn]".into())]);
        assert_eq!(render_plain(&m, &v), "Wren Hollow defeated you.");
        assert_eq!(render_plain(&m, &Values::new()), "The defeated you.");
        let r = render(&m, &Values::new(), &RenderOptions::explorer(None));
        assert_eq!(r.plain_text(), "The ${#0AAAAAA} defeated you.");
    }

    #[test]
    fn explorer_selection_matches_plain() {
        // Regression: placeholders inside a nested value do not make it non-empty.
        let inner = Value::message(msg(&["", ""], &[B]), Values::new());
        let top = msg(&["#1:", "#1:{ [E]|<blank>[b]}x"], &[A]);
        let v = vals(&[(A, inner)]);
        assert_eq!(render_plain(&top, &v), "<blank>x");
        let r = render(&top, &v, &RenderOptions::explorer(None));
        assert_eq!(r.plain_text(), "${#0BBBBBB}<blank>x");
        // The nested placeholder keeps its placeholder style.
        assert!(r.text.spans()[0].style.placeholder);
        assert_eq!(r.text.spans()[0].text, "${#0BBBBBB}");

        // The message's own tag group is found past a trailing absent slot.
        let m = msg(&["Boots[p]", ""], &[A]);
        let p = render(&m, &Values::new(), &RenderOptions::default());
        assert_eq!(
            (p.plain_text().as_str(), p.tags.as_deref()),
            ("Boots", Some("p"))
        );
        let x = render(&m, &Values::new(), &RenderOptions::explorer(None));
        assert_eq!(x.plain_text(), "Boots${#0AAAAAA}");
        assert_eq!(x.tags.as_deref(), Some("p"));

        // Numeric selection on a nested value ignores placeholders too.
        let inner = Value::message(msg(&["", "1"], &[B]), Values::new());
        let top = msg(&["#1:", " #1:{acorn[1]|acorns}"], &[A]);
        let v = vals(&[(A, inner)]);
        assert_eq!(render_plain(&top, &v), "1 acorn");
        let r = render(&top, &v, &RenderOptions::explorer(None));
        assert_eq!(r.plain_text(), "${#0BBBBBB}1 acorn");
    }

    #[test]
    fn nested_values_decode_separately() {
        // Regression: an escape cannot span a value and the next piece of a nested message.
        let inner = msg(&["", "n"], &[A]);
        let inner_v = vals(&[(A, "x\\".into())]);
        assert_eq!(render_plain(&inner, &inner_v), "x\\n");
        let top = msg(&["", ""], &[B]);
        let v = vals(&[(B, Value::message(inner, inner_v))]);
        assert_eq!(render_plain(&top, &v), "x\\n");
        // Markup opened in a nested message's value stays in that value.
        let inner = msg(&["", " tail"], &[A]);
        let inner_v = vals(&[(A, "<b>bold".into())]);
        let top = msg(&["<rgb=#FF0000>", "!"], &[B]);
        let r = render(
            &top,
            &vals(&[(B, Value::message(inner, inner_v))]),
            &RenderOptions::default(),
        );
        assert_eq!(r.plain_text(), "bold tail!");
        let spans = r.text.spans();
        assert!(spans[0].style.bold && spans[0].style.color.is_some());
        assert!(!spans[1].style.bold && spans[1].style.color.is_some());
    }

    #[test]
    fn shared_values_are_prepared_once() {
        // Regression: slots repeating an ID share one prepared value (no per-slot copy).
        let n = 4000;
        let m = Message::new(vec![String::new()], vec![A; n], None);
        let big = "x".repeat(256 << 10);
        let v = vals(&[(A, big.as_str().into())]);
        let opts = RenderOptions {
            max_output: 4096,
            ..RenderOptions::default()
        };
        let mut r = Renderer::new(&opts);
        let slots = r.prepare_all(&m, &v, 1);
        assert_eq!(slots.map.len(), n);
        assert_eq!(slots.prepared.len(), 1);
        let out = render(&m, &v, &opts);
        assert_eq!(out.text.len(), 4096);
        // Absent values: one small placeholder per slot, the value is looked at once.
        let mut r = Renderer::new(&opts);
        let slots = r.prepare_all(&m, &Values::new(), 1);
        assert_eq!(slots.prepared.len(), n);
        assert!(slots.prepared.iter().all(Prepared::is_absent));
    }

    #[test]
    fn depth_limited_shared_value_reports_once() {
        use std::sync::atomic::{AtomicUsize, Ordering};
        use tracing::{Event, Level, Metadata, Subscriber, span};

        struct CountWarn(Arc<AtomicUsize>);
        impl Subscriber for CountWarn {
            fn enabled(&self, _: &Metadata<'_>) -> bool {
                true
            }
            fn new_span(&self, _: &span::Attributes<'_>) -> span::Id {
                span::Id::from_u64(1)
            }
            fn record(&self, _: &span::Id, _: &span::Record<'_>) {}
            fn record_follows_from(&self, _: &span::Id, _: &span::Id) {}
            fn event(&self, e: &Event<'_>) {
                if *e.metadata().level() == Level::WARN {
                    self.0.fetch_add(1, Ordering::Relaxed);
                }
            }
            fn enter(&self, _: &span::Id) {}
            fn exit(&self, _: &span::Id) {}
        }

        let warns = Arc::new(AtomicUsize::new(0));
        let m = Message::new(vec![String::new()], vec![A; 1000], None);
        let v = vals(&[(A, chain(1))]);
        let opts = RenderOptions {
            max_depth: 1,
            ..RenderOptions::default()
        };
        let r = tracing::subscriber::with_default(CountWarn(Arc::clone(&warns)), || {
            render(&m, &v, &opts)
        });
        assert_eq!(r.diagnostics, vec![Diagnostic::DepthLimit { limit: 1 }]);
        assert_eq!(warns.load(Ordering::Relaxed), 1);
    }

    #[test]
    fn link_fields_are_values_not_markup() {
        // Regression: a value with '>' or ':' is field text; it cannot end the opener.
        let m = msg(&["<Select:IID:", ">", r"<\\Select> waves."], &[A, B]);
        let v = vals(&[(A, "a>b:c".into()), (B, "Wren[fn]".into())]);
        let r = render(&m, &v, &RenderOptions::default());
        assert_eq!(r.plain_text(), "Wren waves.");
        let link = r.text.spans()[0].style.link.as_ref().expect("link");
        assert_eq!(link.fields, vec!["a>b:c".to_owned()]);
        // Literal ':' in the template still separates fields.
        let m = msg(&["<RunCmd:String:", ":", ":z>go<\\RunCmd>"], &[A, B]);
        let v = vals(&[(A, "x".into()), (B, "y".into())]);
        let r = render(&m, &v, &RenderOptions::default());
        assert_eq!(r.plain_text(), "go");
        let link = r.text.spans()[0].style.link.as_ref().expect("link");
        assert_eq!(link.fields, vec!["x", "y", "z"]);
        // Two split openers in a row.
        let m = msg(&["<A:B:", "><A:C:", ">x<\\A>"], &[A, B]);
        let r = render(
            &m,
            &vals(&[(A, "1".into()), (B, "2".into())]),
            &RenderOptions::default(),
        );
        assert_eq!(r.plain_text(), "x");
        let link = r.text.spans()[0].style.link.as_ref().expect("link");
        assert_eq!(
            (link.link_type.as_str(), link.fields.clone()),
            ("C", vec!["2".to_owned()])
        );
    }

    #[test]
    fn split_link_opener_is_bounded() {
        // Regression: field text from slot values is capped, and nothing is built when
        // the opener never closes.
        let n = 500;
        let mut pieces = vec!["<A:B:".to_owned()];
        pieces.extend(std::iter::repeat_n(String::new(), n - 1));
        pieces.push(">x".to_owned());
        let m = Message::new(pieces, vec![A; n], None);
        let big = "y".repeat(64 << 10);
        let r = render(
            &m,
            &vals(&[(A, big.as_str().into())]),
            &RenderOptions::default(),
        );
        assert_eq!(r.plain_text(), "x");
        let link = r.text.spans()[0].style.link.as_ref().expect("link");
        assert_eq!(
            link.fields.iter().map(String::len).sum::<usize>(),
            MAX_LINK_FIELD_BYTES
        );
        // Unclosed: literal text, output bounded by the budget.
        let m = Message::new(vec!["<A:B:".to_owned(), String::new()], vec![A; n], None);
        let opts = RenderOptions {
            max_output: 1024,
            ..RenderOptions::default()
        };
        let r = render(&m, &vals(&[(A, big.as_str().into())]), &opts);
        assert_eq!(r.text.len(), 1024);
        assert!(r.text.plain_text().starts_with("<A:B:yyy"));
    }

    #[test]
    fn many_space_spans_render_fast() {
        // Regression: leading-space trimming was quadratic in the number of spans.
        let piece = "<b> </b> ".repeat(50_000) + "x";
        let start = std::time::Instant::now();
        let r = render(
            &msg(&[&piece], &[]),
            &Values::new(),
            &RenderOptions::default(),
        );
        assert_eq!(r.plain_text(), "x");
        assert!(start.elapsed().as_secs() < 10, "took {:?}", start.elapsed());
    }

    #[test]
    fn diagnostics_display() {
        let miss = Miss {
            table: 0x2500_0010,
            token: 1,
            reason: MissReason::NoToken,
        };
        assert_eq!(
            miss.to_string(),
            "string lookup miss: token 0x00000001 in table 0x25000010: no such token in the table"
        );
        let e: Box<dyn std::error::Error> = Box::new(miss.clone());
        assert!(e.to_string().contains("0x25000010"));
        assert_eq!(Diagnostic::Miss(miss.clone()).to_string(), miss.to_string());
        assert_eq!(
            Diagnostic::OutputLimit { limit: 8 }.to_string(),
            "output truncated at 8 bytes"
        );
    }

    #[test]
    fn unresolved_values_render_absent_and_report() {
        let miss = Miss {
            table: 0x2500_0001,
            token: 7,
            reason: MissReason::NoTable,
        };
        let m = msg(&["[", "]"], &[A]);
        let v = vals(&[(A, Value::Unresolved(Diagnostic::Miss(miss.clone())))]);
        let r = render(&m, &v, &RenderOptions::default());
        assert_eq!(r.plain_text(), "[]");
        assert_eq!(r.diagnostics, vec![Diagnostic::Miss(miss)]);
    }

    #[test]
    fn numbers() {
        assert_eq!(parse_number("12"), Some(12.0));
        assert_eq!(parse_number("-1.50"), Some(-1.5));
        assert_eq!(parse_number("+3"), Some(3.0));
        assert_eq!(parse_number("1."), None);
        assert_eq!(parse_number(".5"), None);
        assert_eq!(parse_number("1e3"), None);
        assert_eq!(parse_number(""), None);
        assert_eq!(parse_number("-"), None);
    }

    #[test]
    fn display_text_strips() {
        assert_eq!(
            display_text(r"<rgb=#FFFFFF>Reed\qBoots</rgb>[p]"),
            "Reed\"Boots"
        );
    }
}
