// SPDX-License-Identifier: GPL-3.0-or-later
//! Escapes and markup inside message text, and the rich-text output model.
//!
//! Implements `docs/specs/string-table.md` §4.6 (escapes), §4.7 (colour, emphasis and
//! list markup) and §4.8 (link markup). The output of rendering is [`RichText`]: a list of
//! [`Span`]s, each holding decoded text and a [`Style`].

use std::fmt;
use std::sync::Arc;

/// Maximum number of simultaneously open markup elements in one scope. Openers beyond
/// this are dropped (hostile input could otherwise make style computation quadratic).
pub const MAX_MARKUP_DEPTH: usize = 64;

/// Maximum number of `:`-separated fields kept in one link opener; further fields are
/// dropped (the annotation is inert, so this only bounds memory on hostile input).
pub const MAX_LINK_FIELDS: usize = 32;

/// An sRGB colour with alpha, from a `<rgb=…>` element.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub struct Rgba {
    /// Red.
    pub r: u8,
    /// Green.
    pub g: u8,
    /// Blue.
    pub b: u8,
    /// Alpha (255 for 6-digit values).
    pub a: u8,
}

impl Rgba {
    /// Parses a colour value as written after `<rgb=` (spec §4.7).
    ///
    /// Accepts an optional `#`, then exactly 6 hex digits (`RRGGBB`, alpha 255) or 8 hex
    /// digits (`AARRGGBB`). Anything else is `None`: the run renders in the default colour.
    #[must_use]
    pub fn parse(value: &str) -> Option<Self> {
        let hex = value.strip_prefix('#').unwrap_or(value);
        if !hex.bytes().all(|b| b.is_ascii_hexdigit()) {
            return None;
        }
        let n = u32::from_str_radix(hex, 16).ok()?;
        let [b3, b2, b1, b0] = n.to_be_bytes();
        match hex.len() {
            6 => Some(Self {
                r: b2,
                g: b1,
                b: b0,
                a: 255,
            }),
            8 => Some(Self {
                a: b3,
                r: b2,
                g: b1,
                b: b0,
            }),
            _ => None,
        }
    }
}

/// An inert link annotation from a `<Kind:Type:…>label<\Kind>` element (spec §4.8).
///
/// Tessera never acts on links in M1; the fields are kept for display and inspection.
#[derive(Debug, Clone, PartialEq, Eq, Hash)]
pub struct Link {
    /// The element kind, for example `Select`.
    pub kind: String,
    /// The type name after the kind, for example `IID`.
    pub link_type: String,
    /// Further `:`-separated fields (often a single empty field in stored templates).
    pub fields: Vec<String>,
}

/// Display attributes of a [`Span`].
#[derive(Debug, Clone, Default, PartialEq, Eq, Hash)]
pub struct Style {
    /// Text colour; `None` is the UI's default colour.
    pub color: Option<Rgba>,
    /// Bold run (`<b>`).
    pub bold: bool,
    /// Italic run (`<i>`).
    pub italic: bool,
    /// Underlined run (`<u>`).
    pub underline: bool,
    /// Inside a list item (`<li>`).
    pub list_item: bool,
    /// Inside a link element.
    pub link: Option<Arc<Link>>,
    /// A placeholder for a missing value or a lookup miss (explorer mode only).
    pub placeholder: bool,
}

impl Style {
    /// `inner` applied on top of `self`: attributes `inner` sets win, the rest come from
    /// `self` (used to insert a rendered nested message into an outer style).
    pub(crate) fn overlay(&self, inner: &Self) -> Self {
        Self {
            color: inner.color.or(self.color),
            bold: self.bold || inner.bold,
            italic: self.italic || inner.italic,
            underline: self.underline || inner.underline,
            list_item: self.list_item || inner.list_item,
            link: inner.link.clone().or_else(|| self.link.clone()),
            placeholder: self.placeholder || inner.placeholder,
        }
    }
}

/// A run of decoded text with one style.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Span {
    /// Decoded text (escapes resolved, markup removed).
    pub text: String,
    /// Display attributes.
    pub style: Style,
}

/// Rendered text: a sequence of styled spans. Adjacent spans never share a style and no
/// span is empty.
#[derive(Debug, Clone, Default, PartialEq, Eq)]
pub struct RichText {
    spans: Vec<Span>,
}

impl RichText {
    /// An empty text.
    #[must_use]
    pub fn new() -> Self {
        Self::default()
    }

    /// A single unstyled span.
    #[must_use]
    pub fn plain(text: &str) -> Self {
        let mut t = Self::new();
        t.push_str(text, &Style::default());
        t
    }

    /// The spans in order.
    #[must_use]
    pub fn spans(&self) -> &[Span] {
        &self.spans
    }

    /// Consumes the text and returns its spans.
    #[must_use]
    pub fn into_spans(self) -> Vec<Span> {
        self.spans
    }

    /// Appends text with a style, merging with the last span when the style is equal.
    pub fn push_str(&mut self, text: &str, style: &Style) {
        if text.is_empty() {
            return;
        }
        if let Some(last) = self.spans.last_mut()
            && last.style == *style
        {
            last.text.push_str(text);
            return;
        }
        self.spans.push(Span {
            text: text.to_owned(),
            style: style.clone(),
        });
    }

    /// Appends all spans of `other`.
    pub fn append(&mut self, other: Self) {
        for span in other.spans {
            self.push_str(&span.text, &span.style);
        }
    }

    /// The text without styles.
    #[must_use]
    pub fn plain_text(&self) -> String {
        let mut s = String::with_capacity(self.len());
        for span in &self.spans {
            s.push_str(&span.text);
        }
        s
    }

    /// Whether there is no text.
    #[must_use]
    pub fn is_empty(&self) -> bool {
        self.spans.is_empty()
    }

    /// Total length of the text in bytes (UTF-8).
    #[must_use]
    pub fn len(&self) -> usize {
        self.spans.iter().map(|s| s.text.len()).sum()
    }

    pub(crate) fn first_char(&self) -> Option<char> {
        self.spans.first().and_then(|s| s.text.chars().next())
    }

    pub(crate) fn last_char(&self) -> Option<char> {
        self.spans.last().and_then(|s| s.text.chars().next_back())
    }

    /// Removes the first character (used for junction space collapse).
    pub(crate) fn remove_first_char(&mut self) {
        if let Some(first) = self.spans.first_mut() {
            let mut chars = first.text.chars();
            chars.next();
            first.text = chars.as_str().to_owned();
            if first.text.is_empty() {
                self.spans.remove(0);
            }
        }
    }

    /// The text of the spans that are not placeholders ([`Style::placeholder`]).
    pub(crate) fn text_without_placeholders(&self) -> String {
        let mut s = String::new();
        for span in self.spans.iter().filter(|s| !s.style.placeholder) {
            s.push_str(&span.text);
        }
        s
    }

    /// Trims leading and trailing U+0020 (never other whitespace).
    pub(crate) fn trim_spaces(&mut self) {
        // Drop all-space leading spans in one pass (removing them one by one would be
        // quadratic in the number of spans).
        let k = self
            .spans
            .iter()
            .position(|s| !s.text.bytes().all(|b| b == b' '))
            .unwrap_or(self.spans.len());
        self.spans.drain(..k);
        if let Some(first) = self.spans.first_mut() {
            let t = first.text.trim_start_matches(' ');
            if t.len() != first.text.len() {
                first.text = t.to_owned();
            }
        }
        while let Some(last) = self.spans.last_mut() {
            let t = last.text.trim_end_matches(' ');
            if t.len() == last.text.len() {
                break;
            }
            if t.is_empty() {
                self.spans.pop();
            } else {
                let keep = t.len();
                last.text.truncate(keep);
                break;
            }
        }
    }

    /// Truncates to at most `max` bytes, at a character boundary.
    pub(crate) fn truncate_bytes(&mut self, max: usize) {
        let mut total = 0usize;
        let mut cut = None;
        for (i, span) in self.spans.iter_mut().enumerate() {
            if total + span.text.len() > max {
                let mut keep = max - total;
                while !span.text.is_char_boundary(keep) {
                    keep -= 1;
                }
                span.text.truncate(keep);
                cut = Some(if keep == 0 { i } else { i + 1 });
                break;
            }
            total += span.text.len();
        }
        if let Some(n) = cut {
            self.spans.truncate(n);
        }
    }
}

impl fmt::Display for RichText {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        for span in &self.spans {
            f.write_str(&span.text)?;
        }
        Ok(())
    }
}

/// Decodes the escape that starts with the backslash at the start of `rest` (spec §4.6)
/// and returns the decoded text and the number of bytes consumed.
fn decode_escape(rest: &str) -> (&'static str, Option<char>, usize) {
    let mut chars = rest.chars();
    chars.next(); // the backslash
    match chars.next() {
        Some('n') => ("\n", None, 2),
        Some('q') => ("\"", None, 2),
        Some('t') => ("\t", None, 2),
        Some('[') => ("[", None, 2),
        Some(']') => ("]", None, 2),
        Some('\\') => ("\\", None, 2),
        // Unknown escape: the backslash and the character are both kept.
        Some(c) => ("\\", Some(c), 1 + c.len_utf8()),
        // Backslash at the end of the text: kept.
        None => ("\\", None, 1),
    }
}

/// Decodes escapes only (spec §4.6), single pass, left to right. Markup is left as is.
///
/// `\n` → line break, `\q` → `"`, `\t` → tab, `\[` `\]` `\\` → the character; any other
/// `\x` keeps both characters; a trailing `\` is kept.
#[must_use]
pub fn decode_escapes(raw: &str) -> String {
    let mut out = String::with_capacity(raw.len());
    let mut i = 0;
    while let Some(c) = raw[i..].chars().next() {
        if c == '\\' {
            let (s, extra, len) = decode_escape(&raw[i..]);
            out.push_str(s);
            if let Some(e) = extra {
                out.push(e);
            }
            i += len;
        } else {
            out.push(c);
            i += c.len_utf8();
        }
    }
    out
}

/// One open markup element.
#[derive(Debug, Clone, PartialEq)]
enum Frame {
    Color(Option<Rgba>),
    Bold,
    Italic,
    Underline,
    ListItem,
    Link(Arc<Link>),
}

/// What a recognised tag closes.
#[derive(Debug, Clone, PartialEq)]
enum Closer {
    Color,
    Bold,
    Italic,
    Underline,
    ListItem,
    Link(String),
}

#[derive(Debug, Clone, PartialEq)]
enum Tag {
    Open(Frame),
    Close(Closer),
}

/// The open-element stack of one markup scope (a message, or one inserted value).
#[derive(Debug, Clone, Default)]
pub(crate) struct MarkupState {
    stack: Vec<Frame>,
}

impl MarkupState {
    pub(crate) fn new() -> Self {
        Self::default()
    }

    /// The style in effect: `base` with every open element applied.
    pub(crate) fn style(&self, base: &Style) -> Style {
        let mut s = base.clone();
        for f in &self.stack {
            match f {
                Frame::Color(c) => s.color = *c,
                Frame::Bold => s.bold = true,
                Frame::Italic => s.italic = true,
                Frame::Underline => s.underline = true,
                Frame::ListItem => s.list_item = true,
                Frame::Link(l) => s.link = Some(Arc::clone(l)),
            }
        }
        s
    }

    /// Opens a link element (a link opener assembled by the renderer, spec §4.8).
    pub(crate) fn open_link(&mut self, link: Arc<Link>) -> bool {
        self.apply(Tag::Open(Frame::Link(link)))
    }

    /// Applies a tag. Returns whether the stack changed.
    fn apply(&mut self, tag: Tag) -> bool {
        match tag {
            Tag::Open(f) => {
                if self.stack.len() >= MAX_MARKUP_DEPTH {
                    tracing::debug!("markup nesting limit reached; opener dropped");
                    return false;
                }
                self.stack.push(f);
                true
            }
            Tag::Close(c) => {
                let pos = self.stack.iter().rposition(|f| match (f, &c) {
                    (Frame::Color(_), Closer::Color)
                    | (Frame::Bold, Closer::Bold)
                    | (Frame::Italic, Closer::Italic)
                    | (Frame::Underline, Closer::Underline)
                    | (Frame::ListItem, Closer::ListItem) => true,
                    (Frame::Link(l), Closer::Link(k)) => l.kind.eq_ignore_ascii_case(k),
                    _ => false,
                });
                match pos {
                    Some(i) => {
                        self.stack.remove(i);
                        true
                    }
                    // A stray closer is ignored (spec §4.7).
                    None => false,
                }
            }
        }
    }
}

fn is_ident(s: &str) -> bool {
    let mut b = s.bytes();
    b.next().is_some_and(|c| c.is_ascii_alphabetic()) && b.all(|c| c.is_ascii_alphanumeric())
}

/// Recognises a markup tag at the start of `rest` (which begins with `<`). Returns the tag
/// and its length in bytes, or `None` when the text is literal (spec §4.7: only the listed
/// names and the link shape are markup; `<placeholder>` words stay literal).
fn parse_tag(rest: &str) -> Option<(Tag, usize)> {
    // The tag ends at the first '>'; a '<' first means this '<' is literal. Stopping at
    // '<' keeps scanning linear on hostile input.
    let end = rest[1..].find(['<', '>'])? + 1;
    if rest.as_bytes().get(end) != Some(&b'>') {
        return None;
    }
    let content = &rest[1..end];
    let len = end + 1;
    let lower = content.to_ascii_lowercase();
    let tag = match lower.as_str() {
        "b" => Tag::Open(Frame::Bold),
        "i" => Tag::Open(Frame::Italic),
        "u" => Tag::Open(Frame::Underline),
        "li" => Tag::Open(Frame::ListItem),
        "/b" => Tag::Close(Closer::Bold),
        "/i" => Tag::Close(Closer::Italic),
        "/u" => Tag::Close(Closer::Underline),
        "/li" => Tag::Close(Closer::ListItem),
        "/rgb" => Tag::Close(Closer::Color),
        _ => {
            if lower.starts_with("rgb=") {
                let value = &content[4..];
                let color = Rgba::parse(value);
                if color.is_none() {
                    tracing::debug!(value, "malformed colour value; default colour used");
                }
                Tag::Open(Frame::Color(color))
            } else if let Some(kind) = content
                .strip_prefix("\\\\")
                .or_else(|| content.strip_prefix('\\'))
            {
                if !is_ident(kind) {
                    return None;
                }
                Tag::Close(Closer::Link(kind.to_owned()))
            } else {
                let mut parts = content.split(':');
                let kind = parts.next()?;
                let link_type = parts.next()?;
                if !is_ident(kind) || !is_ident(link_type) {
                    return None;
                }
                // The opener needs a ':' after the Type (spec §4.8); `<word:word>` stays
                // literal like any other placeholder (§4.7).
                let fields: Vec<String> = parts.take(MAX_LINK_FIELDS).map(str::to_owned).collect();
                if fields.is_empty() {
                    return None;
                }
                Tag::Open(Frame::Link(Arc::new(Link {
                    kind: kind.to_owned(),
                    link_type: link_type.to_owned(),
                    fields,
                })))
            }
        }
    };
    Some((tag, len))
}

/// Decodes escapes and interprets markup in `raw`, appending styled spans to `out`.
///
/// `state` carries open elements across calls within one scope; `base` is the style the
/// scope starts from.
pub(crate) fn interpret(raw: &str, state: &mut MarkupState, base: &Style, out: &mut RichText) {
    let mut buf = String::new();
    let mut style = state.style(base);
    let mut i = 0;
    while let Some(c) = raw[i..].chars().next() {
        match c {
            '\\' => {
                let (s, extra, len) = decode_escape(&raw[i..]);
                buf.push_str(s);
                if let Some(e) = extra {
                    buf.push(e);
                }
                i += len;
            }
            '<' => {
                if let Some((tag, len)) = parse_tag(&raw[i..]) {
                    if state.apply(tag) {
                        out.push_str(&buf, &style);
                        buf.clear();
                        style = state.style(base);
                    }
                    i += len;
                } else {
                    buf.push('<');
                    i += 1;
                }
            }
            _ => {
                buf.push(c);
                i += c.len_utf8();
            }
        }
    }
    out.push_str(&buf, &style);
}

/// Decodes escapes and markup of a standalone text (one scope, default base style).
#[must_use]
pub fn parse_markup(raw: &str) -> RichText {
    let mut out = RichText::new();
    interpret(raw, &mut MarkupState::new(), &Style::default(), &mut out);
    out
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn escapes() {
        assert_eq!(decode_escapes(r"a\nb"), "a\nb");
        assert_eq!(decode_escapes(r"\q hi \q"), "\" hi \"");
        assert_eq!(decode_escapes(r"\[x\]"), "[x]");
        assert_eq!(decode_escapes(r"\\n"), "\\n");
        assert_eq!(decode_escapes(r"a\tb"), "a\tb");
        assert_eq!(decode_escapes(r"a\ b"), "a\\ b");
        assert_eq!(decode_escapes(r"end\"), "end\\");
        assert_eq!(decode_escapes("\\é"), "\\é");
    }

    #[test]
    fn colours() {
        assert_eq!(
            Rgba::parse("#FF8000"),
            Some(Rgba {
                r: 255,
                g: 128,
                b: 0,
                a: 255
            })
        );
        assert_eq!(
            Rgba::parse("#80ff8000"),
            Some(Rgba {
                a: 0x80,
                r: 255,
                g: 128,
                b: 0
            })
        );
        assert_eq!(Rgba::parse("ff8000").map(|c| c.r), Some(255));
        assert_eq!(Rgba::parse("#FF800"), None);
        assert_eq!(Rgba::parse("#FF80000"), None);
        assert_eq!(Rgba::parse("#FFFF"), None);
        assert_eq!(Rgba::parse("#GG8000"), None);
        assert_eq!(Rgba::parse("#+F8000"), None);
    }

    #[test]
    fn nested_and_unbalanced() {
        let t = parse_markup("a<rgb=#FF0000>b<b>c</b>d</rgb>e");
        let s = t.spans();
        assert_eq!(t.plain_text(), "abcde");
        assert_eq!(s.len(), 5);
        assert!(s[1].style.color.is_some() && !s[1].style.bold);
        assert!(s[2].style.color.is_some() && s[2].style.bold);
        assert_eq!(s[4].style, Style::default());

        // Unclosed run ends at the end; stray closer ignored; upper case accepted.
        let t = parse_markup("x</rgb>y<RGB=#00FF00>z");
        assert_eq!(t.plain_text(), "xyz");
        assert_eq!(t.spans().len(), 2);
        assert_eq!(t.spans()[1].style.color.map(|c| c.g), Some(255));
        let t = parse_markup("<Rgb=#00FF00>z</RGB>w");
        assert_eq!(t.spans().len(), 2);
    }

    #[test]
    fn malformed_colour_uses_default() {
        let t = parse_markup("<rgb=#FFFFFF>a<rgb=#12345>b</rgb>c</rgb>");
        let s = t.spans();
        assert_eq!(t.plain_text(), "abc");
        assert!(s[0].style.color.is_some());
        assert_eq!(s[1].style.color, None);
        assert!(s[2].style.color.is_some());
    }

    #[test]
    fn placeholders_stay_literal() {
        assert_eq!(
            parse_markup("/say <message>").plain_text(),
            "/say <message>"
        );
        assert_eq!(parse_markup("a < b > c").plain_text(), "a < b > c");
        assert_eq!(parse_markup("<<b>x").plain_text(), "<x");
        assert_eq!(parse_markup("tail <").plain_text(), "tail <");
        assert!(parse_markup("<li>item</li>").spans()[0].style.list_item);
    }

    #[test]
    fn links() {
        let t = parse_markup(r"go <Select:IID:>there<\\Select> now");
        assert_eq!(t.plain_text(), "go there now");
        let link = t.spans()[1].style.link.as_ref().expect("link");
        assert_eq!(link.kind, "Select");
        assert_eq!(link.link_type, "IID");
        assert_eq!(link.fields, vec![String::new()]);
        assert!(t.spans()[2].style.link.is_none());
        // Single-backslash closer also accepted.
        let t = parse_markup(r"<RunCmd:String:x:y>a<\RunCmd>b");
        assert_eq!(t.spans().len(), 2);
    }

    #[test]
    fn two_part_tag_is_literal() {
        // Regression: `<Kind:Type>` without the ':' after Type is not a link opener.
        let t = parse_markup("Use <channel:message> here");
        assert_eq!(t.plain_text(), "Use <channel:message> here");
        assert_eq!(t.spans().len(), 1);
        assert!(t.spans()[0].style.link.is_none());
        let t = parse_markup("<a:b>x");
        assert_eq!(t.plain_text(), "<a:b>x");
        // Field count is capped.
        let raw = format!("<A:B{}>x", ":f".repeat(MAX_LINK_FIELDS * 3));
        let t = parse_markup(&raw);
        let link = t.spans()[0].style.link.as_ref().expect("link");
        assert_eq!(link.fields.len(), MAX_LINK_FIELDS);
    }

    #[test]
    fn trim_many_space_spans_is_linear() {
        // Regression: trimming used Vec::remove(0) per leading span (quadratic).
        let n = 200_000;
        let raw = "<b> </b> ".repeat(n) + "x";
        let mut t = parse_markup(&raw);
        assert!(t.spans().len() > n);
        let start = std::time::Instant::now();
        t.trim_spaces();
        assert!(start.elapsed().as_secs() < 2, "took {:?}", start.elapsed());
        assert_eq!(t.plain_text(), "x");
        let mut t = RichText::plain("   ");
        t.trim_spaces();
        assert!(t.is_empty());
    }

    #[test]
    fn depth_limit() {
        let raw = "<b>".repeat(MAX_MARKUP_DEPTH * 4) + "x";
        assert_eq!(parse_markup(&raw).plain_text(), "x");
    }

    #[test]
    fn trimming_and_truncation() {
        let mut t = RichText::new();
        t.push_str("  ", &Style::default());
        t.push_str(
            " a ",
            &Style {
                bold: true,
                ..Style::default()
            },
        );
        t.push_str("\n ", &Style::default());
        t.trim_spaces();
        assert_eq!(t.plain_text(), "a \n");
        let mut t = RichText::plain("héllo");
        t.truncate_bytes(2);
        assert_eq!(t.plain_text(), "h");
    }
}
