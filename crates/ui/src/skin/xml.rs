// SPDX-License-Identifier: GPL-3.0-or-later
//! A strict, bounded XML 1.0 reader for `SkinDefinition.xml` (`skin-definition.md` §4.1, D4,
//! D5).
//!
//! - **Encodings (D5):** UTF-8 with or without a byte-order mark, UTF-16 with a byte-order
//!   mark, and a declared `ISO-8859-1`, `US-ASCII` or `windows-1252`. Anything else is
//!   rejected.
//! - **No DTD processing (D5):** a document type declaration is skipped lexically and its
//!   content ignored; nothing is ever fetched. Only the five predefined entities and
//!   character references are expanded; any other entity reference is an error.
//! - **Non-validating, no namespaces (§4.1):** a prefix is part of a name.
//! - **Well-formedness** is checked completely: names, attribute syntax and uniqueness,
//!   matching end tags, comments, processing instructions, CDATA sections, references,
//!   character ranges and the single root element. A document that fails any check is
//!   rejected as a whole.
//! - **Bounds:** element nesting deeper than [`MAX_XML_DEPTH`] is rejected (D4), and so
//!   is a tag with more than [`MAX_XML_ATTRIBUTES`] attributes (a Tessera bound); the
//!   caller bounds the input size. The reader is iterative (no recursion), allocates only
//!   the attributes of the current tag, and is linear in the input.
//!
//! Attribute values are normalised as XML requires for attributes without a declared type:
//! literal tabs, line feeds, carriage returns and CR LF pairs become one space each, and
//! references are expanded.

use std::collections::HashSet;

/// Deepest element nesting accepted (D4: "element nesting deeper than 64 is rejected as
/// malformed"; the root element is at depth 1).
pub const MAX_XML_DEPTH: usize = 64;

/// Most attributes on one start tag; a tag with more is rejected as malformed. A Tessera
/// bound against memory amplification (the elements of a definition use a handful of
/// attributes each); proposed for D4 in an OPEN note of `skin-definition.md`.
pub const MAX_XML_ATTRIBUTES: usize = 256;

/// Why a document is not accepted.
#[derive(Debug, Clone, PartialEq, Eq, thiserror::Error)]
#[non_exhaustive]
pub enum XmlErrorKind {
    /// The byte-order mark or the declared encoding is not one D5 accepts.
    #[error("unsupported encoding {0:?}")]
    UnsupportedEncoding(String),
    /// Bytes that are not valid in the document's encoding.
    #[error("invalid {0} byte sequence")]
    InvalidEncoding(&'static str),
    /// A character that XML 1.0 does not allow.
    #[error("character U+{0:04X} is not allowed in XML")]
    InvalidChar(u32),
    /// The document ended inside a construct.
    #[error("unexpected end of document")]
    UnexpectedEof,
    /// A construct that the grammar does not allow here.
    #[error("syntax error: {0}")]
    Syntax(&'static str),
    /// An invalid name.
    #[error("invalid name")]
    InvalidName,
    /// An end tag that does not match the open element.
    #[error("end tag </{found}> does not match <{expected}>")]
    MismatchedEndTag {
        /// The open element.
        expected: String,
        /// The end tag's name.
        found: String,
    },
    /// The same attribute twice on one element.
    #[error("duplicate attribute {0:?}")]
    DuplicateAttribute(String),
    /// An entity reference other than the five predefined entities.
    #[error("undefined entity &{0};")]
    UndefinedEntity(String),
    /// A malformed character reference.
    #[error("invalid character reference")]
    InvalidCharRef,
    /// No root element.
    #[error("no root element")]
    NoRoot,
    /// Content after the root element other than comments, processing instructions and
    /// white space.
    #[error("content after the root element")]
    TrailingContent,
    /// Element nesting deeper than [`MAX_XML_DEPTH`].
    #[error("element nesting deeper than {MAX_XML_DEPTH}")]
    TooDeep,
    /// A start tag with more than [`MAX_XML_ATTRIBUTES`] attributes.
    #[error("more than {MAX_XML_ATTRIBUTES} attributes on one tag")]
    TooManyAttributes,
}

/// An XML error with the 1-based line where it was found.
#[derive(Debug, Clone, PartialEq, Eq, thiserror::Error)]
#[error("line {line}: {kind}")]
pub struct XmlError {
    /// 1-based line number (line ends counted as XML counts them).
    pub line: u32,
    /// What went wrong.
    pub kind: XmlErrorKind,
}

/// One attribute of a start tag, value normalised and references expanded.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Attribute {
    /// Name, exactly as written.
    pub name: String,
    /// Normalised value.
    pub value: String,
}

/// An event of [`Reader::next_event`].
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum Event {
    /// A start tag (an empty-element tag gives a `Start` followed by an `End`).
    Start {
        /// Element name, exactly as written (prefix included).
        name: String,
        /// Attributes in document order.
        attributes: Vec<Attribute>,
        /// 1-based line of the `<`.
        line: u32,
    },
    /// The end of the most recently started open element.
    End,
}

impl Event {
    /// The value of attribute `name` (exact, case-sensitive match), or `""` when missing
    /// (`skin-definition.md` §4.1).
    #[must_use]
    pub fn attribute<'a>(attributes: &'a [Attribute], name: &str) -> &'a str {
        attributes
            .iter()
            .find(|a| a.name == name)
            .map_or("", |a| a.value.as_str())
    }
}

/// Decodes a document's bytes to text according to D5.
///
/// # Errors
///
/// [`XmlErrorKind::UnsupportedEncoding`] or [`XmlErrorKind::InvalidEncoding`] (line 1).
pub fn decode_document(bytes: &[u8]) -> Result<String, XmlError> {
    let err = |kind| XmlError { line: 1, kind };
    if let Some(rest) = bytes.strip_prefix(&[0xEF, 0xBB, 0xBF]) {
        let text = std::str::from_utf8(rest)
            .map_err(|_| err(XmlErrorKind::InvalidEncoding("UTF-8")))?
            .to_owned();
        check_declared(&text, &["UTF-8"])?;
        return Ok(text);
    }
    if let Some(rest) = bytes.strip_prefix(&[0xFF, 0xFE]) {
        let text = decode_utf16(rest, false)
            .ok_or_else(|| err(XmlErrorKind::InvalidEncoding("UTF-16")))?;
        check_declared(&text, &["UTF-16", "UTF-16LE"])?;
        return Ok(text);
    }
    if let Some(rest) = bytes.strip_prefix(&[0xFE, 0xFF]) {
        let text =
            decode_utf16(rest, true).ok_or_else(|| err(XmlErrorKind::InvalidEncoding("UTF-16")))?;
        check_declared(&text, &["UTF-16", "UTF-16BE"])?;
        return Ok(text);
    }
    if bytes.starts_with(&[0x00, b'<']) || bytes.starts_with(&[b'<', 0x00]) {
        return Err(err(XmlErrorKind::UnsupportedEncoding(
            "UTF-16 without a byte-order mark".into(),
        )));
    }
    // The declaration, if any, is ASCII; read it through a Latin-1 view of the whole
    // declaration (white space between its pseudo-attributes is unbounded in XML, so it is
    // not cut at a fixed length; the input itself is bounded by the caller).
    let decl_len = if bytes.starts_with(b"<?xml") {
        bytes
            .windows(2)
            .position(|w| w == b"?>")
            .map_or(bytes.len(), |i| i + 2)
    } else {
        0
    };
    let prefix: String = bytes[..decl_len].iter().map(|&b| char::from(b)).collect();
    let declared = declared_encoding(&prefix);
    match declared.as_deref().map(str::to_ascii_uppercase).as_deref() {
        None | Some("UTF-8") => std::str::from_utf8(bytes)
            .map(str::to_owned)
            .map_err(|_| err(XmlErrorKind::InvalidEncoding("UTF-8"))),
        Some("ISO-8859-1") => Ok(bytes.iter().map(|&b| char::from(b)).collect()),
        Some("US-ASCII") => {
            if bytes.is_ascii() {
                Ok(bytes.iter().map(|&b| char::from(b)).collect())
            } else {
                Err(err(XmlErrorKind::InvalidEncoding("US-ASCII")))
            }
        }
        Some("WINDOWS-1252") => Ok(bytes.iter().map(|&b| cp1252(b)).collect()),
        Some(_) => Err(err(XmlErrorKind::UnsupportedEncoding(
            declared.unwrap_or_default(),
        ))),
    }
}

fn check_declared(text: &str, allowed: &[&str]) -> Result<(), XmlError> {
    match declared_encoding(text) {
        Some(enc) if !allowed.iter().any(|a| a.eq_ignore_ascii_case(&enc)) => Err(XmlError {
            line: 1,
            kind: XmlErrorKind::UnsupportedEncoding(enc),
        }),
        _ => Ok(()),
    }
}

fn decode_utf16(bytes: &[u8], big_endian: bool) -> Option<String> {
    let (pairs, rest) = bytes.as_chunks::<2>();
    if !rest.is_empty() {
        return None;
    }
    let units = pairs.iter().map(|&c| {
        if big_endian {
            u16::from_be_bytes(c)
        } else {
            u16::from_le_bytes(c)
        }
    });
    let mut out = String::new();
    out.try_reserve(bytes.len() / 2).ok()?;
    for c in char::decode_utf16(units) {
        out.push(c.ok()?);
    }
    Some(out)
}

/// The windows-1252 mapping of one byte. The five unassigned bytes map to the C1 control
/// of the same value (as the WHATWG encoding standard does).
fn cp1252(b: u8) -> char {
    const HIGH: [u16; 32] = [
        0x20AC, 0x0081, 0x201A, 0x0192, 0x201E, 0x2026, 0x2020, 0x2021, 0x02C6, 0x2030, 0x0160,
        0x2039, 0x0152, 0x008D, 0x017D, 0x008F, 0x0090, 0x2018, 0x2019, 0x201C, 0x201D, 0x2022,
        0x2013, 0x2014, 0x02DC, 0x2122, 0x0161, 0x203A, 0x0153, 0x009D, 0x017E, 0x0178,
    ];
    if (0x80..0xA0).contains(&b) {
        char::from_u32(u32::from(HIGH[usize::from(b - 0x80)])).unwrap_or(char::from(b))
    } else {
        char::from(b)
    }
}

/// The `encoding` pseudo-attribute of an XML declaration at the start of `text`, if any.
/// Only a lexical look-up; the reader validates the declaration itself.
fn declared_encoding(text: &str) -> Option<String> {
    let rest = text.strip_prefix("<?xml")?;
    if !rest.starts_with(is_xml_space) {
        return None;
    }
    let end = rest.find("?>")?;
    let decl = &rest[..end];
    let at = find_pseudo_attr(decl, "encoding")?;
    let after = decl[at + "encoding".len()..].trim_start_matches(is_xml_space);
    let after = after.strip_prefix('=')?.trim_start_matches(is_xml_space);
    let quote = after.chars().next().filter(|&q| q == '"' || q == '\'')?;
    let body = &after[1..];
    let close = body.find(quote)?;
    Some(body[..close].to_owned())
}

fn find_pseudo_attr(decl: &str, name: &str) -> Option<usize> {
    let mut from = 0;
    while let Some(i) = decl[from..].find(name) {
        let at = from + i;
        let before_ok = decl[..at].ends_with(is_xml_space);
        if before_ok {
            return Some(at);
        }
        from = at + name.len();
    }
    None
}

fn is_xml_space(c: char) -> bool {
    matches!(c, ' ' | '\t' | '\n' | '\r')
}

fn is_xml_char(c: char) -> bool {
    matches!(c, '\t' | '\n' | '\r') || (c >= ' ' && c != '\u{FFFE}' && c != '\u{FFFF}')
}

fn is_name_start(c: char) -> bool {
    matches!(c,
        ':' | 'A'..='Z' | '_' | 'a'..='z'
        | '\u{C0}'..='\u{D6}' | '\u{D8}'..='\u{F6}' | '\u{F8}'..='\u{2FF}'
        | '\u{370}'..='\u{37D}' | '\u{37F}'..='\u{1FFF}' | '\u{200C}'..='\u{200D}'
        | '\u{2070}'..='\u{218F}' | '\u{2C00}'..='\u{2FEF}' | '\u{3001}'..='\u{D7FF}'
        | '\u{F900}'..='\u{FDCF}' | '\u{FDF0}'..='\u{FFFD}' | '\u{10000}'..='\u{EFFFF}')
}

fn is_name_char(c: char) -> bool {
    is_name_start(c)
        || matches!(c, '-' | '.' | '0'..='9' | '\u{B7}' | '\u{300}'..='\u{36F}' | '\u{203F}'..='\u{2040}')
}

/// Where the reader is in the document.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum Phase {
    /// Before the root element (declaration, misc, document type).
    Prolog,
    /// Inside the root element.
    Content,
    /// After the root element.
    Epilog,
    /// Finished (end of input reached after the root).
    Done,
}

/// A pull reader over decoded text.
#[derive(Debug)]
pub struct Reader<'a> {
    text: &'a str,
    pos: usize,
    /// Line bookkeeping: `line` is the line at byte `line_pos`.
    line: u32,
    line_pos: usize,
    phase: Phase,
    seen_doctype: bool,
    stack: Vec<String>,
    /// An `End` owed for an empty-element tag.
    pending_end: bool,
}

type R<T> = Result<T, XmlError>;

impl<'a> Reader<'a> {
    /// A reader over `text` (already decoded with [`decode_document`]).
    #[must_use]
    pub fn new(text: &'a str) -> Self {
        Self {
            text,
            pos: 0,
            line: 1,
            line_pos: 0,
            phase: Phase::Prolog,
            seen_doctype: false,
            stack: Vec::new(),
            pending_end: false,
        }
    }

    /// Current element depth (the root is 1).
    #[must_use]
    pub fn depth(&self) -> usize {
        self.stack.len()
    }

    /// The 1-based line of byte offset `pos` (monotonic bookkeeping; linear overall).
    fn line_at(&mut self, pos: usize) -> u32 {
        let pos = pos.min(self.text.len());
        if pos < self.line_pos {
            self.line = 1;
            self.line_pos = 0;
        }
        let seg = &self.text.as_bytes()[self.line_pos..pos];
        // XML line ends: LF, CR LF and lone CR all count once.
        let mut n = 0u32;
        let mut prev_cr = self.line_pos > 0 && self.text.as_bytes()[self.line_pos - 1] == b'\r';
        for &b in seg {
            match b {
                b'\n' if prev_cr => {}
                b'\n' | b'\r' => n = n.saturating_add(1),
                _ => {}
            }
            prev_cr = b == b'\r';
        }
        self.line = self.line.saturating_add(n);
        self.line_pos = pos;
        self.line
    }

    fn err_at<T>(&mut self, pos: usize, kind: XmlErrorKind) -> R<T> {
        let line = self.line_at(pos);
        Err(XmlError { line, kind })
    }

    fn rest(&self) -> &'a str {
        &self.text[self.pos..]
    }

    fn starts_with(&self, s: &str) -> bool {
        self.rest().starts_with(s)
    }

    fn peek(&self) -> Option<char> {
        self.rest().chars().next()
    }

    fn bump(&mut self) -> Option<char> {
        let c = self.peek()?;
        self.pos += c.len_utf8();
        Some(c)
    }

    fn skip_space(&mut self) -> bool {
        let start = self.pos;
        while self.peek().is_some_and(is_xml_space) {
            self.pos += 1;
        }
        self.pos > start
    }

    fn expect(&mut self, s: &str, what: &'static str) -> R<()> {
        if self.starts_with(s) {
            self.pos += s.len();
            Ok(())
        } else if self.pos >= self.text.len() {
            self.err_at(self.pos, XmlErrorKind::UnexpectedEof)
        } else {
            self.err_at(self.pos, XmlErrorKind::Syntax(what))
        }
    }

    fn name(&mut self) -> R<&'a str> {
        let start = self.pos;
        match self.peek() {
            Some(c) if is_name_start(c) => {
                self.bump();
            }
            None => return self.err_at(self.pos, XmlErrorKind::UnexpectedEof),
            Some(_) => return self.err_at(self.pos, XmlErrorKind::InvalidName),
        }
        while self.peek().is_some_and(is_name_char) {
            self.bump();
        }
        Ok(&self.text[start..self.pos])
    }

    /// Checks every character of the document once, up front.
    fn check_chars(&mut self) -> R<()> {
        if let Some((i, c)) = self.text.char_indices().find(|&(_, c)| !is_xml_char(c)) {
            return self.err_at(i, XmlErrorKind::InvalidChar(u32::from(c)));
        }
        Ok(())
    }

    /// The next event, or `None` at the end of a well-formed document.
    ///
    /// # Errors
    ///
    /// The first well-formedness violation found. After an error the reader must not be
    /// used again.
    pub fn next_event(&mut self) -> R<Option<Event>> {
        if self.pending_end {
            self.pending_end = false;
            self.close_element();
            return Ok(Some(Event::End));
        }
        loop {
            match self.phase {
                Phase::Done => return Ok(None),
                Phase::Prolog => {
                    if self.pos == 0 {
                        self.check_chars()?;
                        if self.starts_with("<?xml")
                            && self.text[5..].starts_with(|c: char| is_xml_space(c) || c == '?')
                        {
                            self.xml_decl()?;
                        }
                    }
                    self.skip_space();
                    if self.pos >= self.text.len() {
                        return self.err_at(self.pos, XmlErrorKind::NoRoot);
                    }
                    if self.starts_with("<!--") {
                        self.comment()?;
                    } else if self.starts_with("<?") {
                        self.pi()?;
                    } else if self.starts_with("<!DOCTYPE") {
                        if self.seen_doctype {
                            return self.err_at(self.pos, XmlErrorKind::Syntax("second DOCTYPE"));
                        }
                        self.seen_doctype = true;
                        self.doctype()?;
                    } else if self.starts_with("<") {
                        self.phase = Phase::Content;
                        return self.start_tag().map(Some);
                    } else {
                        return self.err_at(
                            self.pos,
                            XmlErrorKind::Syntax("text before the root element"),
                        );
                    }
                }
                Phase::Epilog => {
                    self.skip_space();
                    if self.pos >= self.text.len() {
                        self.phase = Phase::Done;
                        return Ok(None);
                    }
                    if self.starts_with("<!--") {
                        self.comment()?;
                    } else if self.starts_with("<?") {
                        self.pi()?;
                    } else {
                        return self.err_at(self.pos, XmlErrorKind::TrailingContent);
                    }
                }
                Phase::Content => {
                    if self.pos >= self.text.len() {
                        return self.err_at(self.pos, XmlErrorKind::UnexpectedEof);
                    }
                    if self.starts_with("</") {
                        return self.end_tag().map(Some);
                    } else if self.starts_with("<!--") {
                        self.comment()?;
                    } else if self.starts_with("<![CDATA[") {
                        self.cdata()?;
                    } else if self.starts_with("<?") {
                        self.pi()?;
                    } else if self.starts_with("<!") {
                        return self.err_at(
                            self.pos,
                            XmlErrorKind::Syntax("markup declaration in content"),
                        );
                    } else if self.starts_with("<") {
                        return self.start_tag().map(Some);
                    } else {
                        self.char_data()?;
                    }
                }
            }
        }
    }

    fn close_element(&mut self) {
        self.stack.pop();
        if self.stack.is_empty() {
            self.phase = Phase::Epilog;
        }
    }

    fn xml_decl(&mut self) -> R<()> {
        self.pos += 5;
        if !self.skip_space() {
            return self.err_at(
                self.pos,
                XmlErrorKind::Syntax("XML declaration needs a version"),
            );
        }
        let (name, value) = self.pseudo_attr()?;
        if name != "version"
            || !value
                .strip_prefix("1.")
                .is_some_and(|d| !d.is_empty() && d.bytes().all(|b| b.is_ascii_digit()))
        {
            return self.err_at(
                self.pos,
                XmlErrorKind::Syntax("bad version in XML declaration"),
            );
        }
        let mut seen_encoding = false;
        let mut seen_standalone = false;
        loop {
            let had_space = self.skip_space();
            if self.starts_with("?>") {
                self.pos += 2;
                return Ok(());
            }
            if !had_space {
                return self.err_at(self.pos, XmlErrorKind::Syntax("malformed XML declaration"));
            }
            let (name, value) = self.pseudo_attr()?;
            match name {
                "encoding" if !seen_encoding && !seen_standalone => {
                    seen_encoding = true;
                    let mut chars = value.chars();
                    let ok = chars.next().is_some_and(|c| c.is_ascii_alphabetic())
                        && chars.all(|c| c.is_ascii_alphanumeric() || matches!(c, '.' | '_' | '-'));
                    if !ok {
                        return self.err_at(self.pos, XmlErrorKind::Syntax("bad encoding name"));
                    }
                }
                "standalone" if !seen_standalone => {
                    seen_standalone = true;
                    if value != "yes" && value != "no" {
                        return self.err_at(self.pos, XmlErrorKind::Syntax("bad standalone value"));
                    }
                }
                _ => {
                    return self
                        .err_at(self.pos, XmlErrorKind::Syntax("malformed XML declaration"));
                }
            }
        }
    }

    fn pseudo_attr(&mut self) -> R<(&'a str, &'a str)> {
        let name = self.name()?;
        self.skip_space();
        self.expect("=", "expected '='")?;
        self.skip_space();
        let quote = match self.peek() {
            Some(q @ ('"' | '\'')) => q,
            None => return self.err_at(self.pos, XmlErrorKind::UnexpectedEof),
            Some(_) => return self.err_at(self.pos, XmlErrorKind::Syntax("expected a quote")),
        };
        self.pos += 1;
        let start = self.pos;
        let Some(len) = self.rest().find(quote) else {
            return self.err_at(self.pos, XmlErrorKind::UnexpectedEof);
        };
        self.pos += len + 1;
        Ok((name, &self.text[start..start + len]))
    }

    fn comment(&mut self) -> R<()> {
        let start = self.pos;
        self.pos += 4;
        let Some(i) = self.rest().find("--") else {
            return self.err_at(start, XmlErrorKind::UnexpectedEof);
        };
        self.pos += i;
        if !self.starts_with("-->") {
            return self.err_at(self.pos, XmlErrorKind::Syntax("'--' inside a comment"));
        }
        self.pos += 3;
        Ok(())
    }

    fn pi(&mut self) -> R<()> {
        let start = self.pos;
        self.pos += 2;
        let target = self.name()?;
        if target.eq_ignore_ascii_case("xml") {
            return self.err_at(start, XmlErrorKind::Syntax("misplaced XML declaration"));
        }
        if self.starts_with("?>") {
            self.pos += 2;
            return Ok(());
        }
        if !self.skip_space() {
            return self.err_at(
                self.pos,
                XmlErrorKind::Syntax("malformed processing instruction"),
            );
        }
        let Some(i) = self.rest().find("?>") else {
            return self.err_at(start, XmlErrorKind::UnexpectedEof);
        };
        self.pos += i + 2;
        Ok(())
    }

    fn cdata(&mut self) -> R<()> {
        let start = self.pos;
        self.pos += "<![CDATA[".len();
        let Some(i) = self.rest().find("]]>") else {
            return self.err_at(start, XmlErrorKind::UnexpectedEof);
        };
        self.pos += i + 3;
        Ok(())
    }

    /// Skips a document type declaration lexically (D5: ignored, never fetched).
    fn doctype(&mut self) -> R<()> {
        let start = self.pos;
        self.pos += "<!DOCTYPE".len();
        if !self.skip_space() {
            return self.err_at(self.pos, XmlErrorKind::Syntax("malformed DOCTYPE"));
        }
        self.name()?;
        loop {
            self.skip_space();
            match self.peek() {
                None => return self.err_at(start, XmlErrorKind::UnexpectedEof),
                Some('>') => {
                    self.pos += 1;
                    return Ok(());
                }
                Some(q @ ('"' | '\'')) => {
                    self.pos += 1;
                    let Some(i) = self.rest().find(q) else {
                        return self.err_at(start, XmlErrorKind::UnexpectedEof);
                    };
                    self.pos += i + 1;
                }
                Some('[') => {
                    self.pos += 1;
                    self.internal_subset(start)?;
                }
                Some(c) if is_name_char(c) => {
                    self.name()?;
                }
                Some(_) => return self.err_at(self.pos, XmlErrorKind::Syntax("malformed DOCTYPE")),
            }
        }
    }

    fn internal_subset(&mut self, start: usize) -> R<()> {
        loop {
            if self.starts_with("<!--") {
                self.comment()?;
                continue;
            }
            if self.starts_with("<?") {
                self.pi()?;
                continue;
            }
            match self.bump() {
                None => return self.err_at(start, XmlErrorKind::UnexpectedEof),
                Some(']') => return Ok(()),
                Some(q @ ('"' | '\'')) => {
                    let Some(i) = self.rest().find(q) else {
                        return self.err_at(start, XmlErrorKind::UnexpectedEof);
                    };
                    self.pos += i + 1;
                }
                Some(_) => {}
            }
        }
    }

    fn start_tag(&mut self) -> R<Event> {
        let lt = self.pos;
        self.pos += 1;
        let name = self.name()?.to_owned();
        let mut attributes: Vec<Attribute> = Vec::new();
        // Duplicate detection over names borrowed from the input: a linear scan for the
        // first few, a set beyond.
        let mut seen: Vec<&'a str> = Vec::new();
        let mut index: Option<HashSet<&'a str>> = None;
        loop {
            let had_space = self.skip_space();
            if self.starts_with("/>") {
                self.pos += 2;
                self.open_element(lt, name.clone())?;
                self.pending_end = true;
                break;
            }
            if self.starts_with(">") {
                self.pos += 1;
                self.open_element(lt, name.clone())?;
                break;
            }
            if self.pos >= self.text.len() {
                return self.err_at(self.pos, XmlErrorKind::UnexpectedEof);
            }
            if !had_space {
                return self.err_at(
                    self.pos,
                    XmlErrorKind::Syntax("expected white space before an attribute"),
                );
            }
            let attr_pos = self.pos;
            if attributes.len() >= MAX_XML_ATTRIBUTES {
                return self.err_at(attr_pos, XmlErrorKind::TooManyAttributes);
            }
            let attr_name = self.name()?;
            self.skip_space();
            self.expect("=", "expected '=' after an attribute name")?;
            self.skip_space();
            let value = self.att_value()?;
            let duplicate = match &mut index {
                Some(set) => !set.insert(attr_name),
                None => {
                    let dup = seen.contains(&attr_name);
                    seen.push(attr_name);
                    if seen.len() > 16 {
                        index = Some(seen.iter().copied().collect());
                    }
                    dup
                }
            };
            if duplicate {
                return self.err_at(
                    attr_pos,
                    XmlErrorKind::DuplicateAttribute(attr_name.to_owned()),
                );
            }
            attributes.push(Attribute {
                name: attr_name.to_owned(),
                value,
            });
        }
        let line = self.line_at(lt);
        Ok(Event::Start {
            name,
            attributes,
            line,
        })
    }

    fn open_element(&mut self, lt: usize, name: String) -> R<()> {
        if self.stack.len() >= MAX_XML_DEPTH {
            return self.err_at(lt, XmlErrorKind::TooDeep);
        }
        self.stack.push(name);
        Ok(())
    }

    fn end_tag(&mut self) -> R<Event> {
        let lt = self.pos;
        self.pos += 2;
        let name = self.name()?;
        self.skip_space();
        self.expect(">", "expected '>' in an end tag")?;
        match self.stack.last() {
            Some(open) if open == name => {}
            Some(open) => {
                let expected = open.clone();
                return self.err_at(
                    lt,
                    XmlErrorKind::MismatchedEndTag {
                        expected,
                        found: name.to_owned(),
                    },
                );
            }
            None => return self.err_at(lt, XmlErrorKind::Syntax("end tag without a start tag")),
        }
        self.close_element();
        Ok(Event::End)
    }

    fn att_value(&mut self) -> R<String> {
        let quote = match self.peek() {
            Some(q @ ('"' | '\'')) => q,
            None => return self.err_at(self.pos, XmlErrorKind::UnexpectedEof),
            Some(_) => {
                return self.err_at(
                    self.pos,
                    XmlErrorKind::Syntax("expected a quoted attribute value"),
                );
            }
        };
        self.pos += 1;
        let mut out = String::new();
        loop {
            let rest = self.rest();
            let Some(i) = rest.find([quote, '<', '&', '\t', '\n', '\r']) else {
                return self.err_at(self.pos, XmlErrorKind::UnexpectedEof);
            };
            out.push_str(&rest[..i]);
            self.pos += i;
            match self.bump() {
                Some(c) if c == quote => return Ok(out),
                Some('<') => {
                    return self.err_at(
                        self.pos - 1,
                        XmlErrorKind::Syntax("'<' in an attribute value"),
                    );
                }
                Some('&') => {
                    let c = self.reference()?;
                    out.push(c);
                }
                Some('\r') => {
                    if self.peek() == Some('\n') {
                        self.pos += 1;
                    }
                    out.push(' ');
                }
                Some(_) => out.push(' '),
                None => return self.err_at(self.pos, XmlErrorKind::UnexpectedEof),
            }
        }
    }

    /// Reads a reference after its `&` and returns the character it stands for.
    fn reference(&mut self) -> R<char> {
        let amp = self.pos - 1;
        if self.starts_with("#") {
            self.pos += 1;
            let hex = self.starts_with("x");
            if hex {
                self.pos += 1;
            }
            let Some(end) = self.rest().find(';') else {
                return self.err_at(amp, XmlErrorKind::InvalidCharRef);
            };
            let digits = &self.rest()[..end];
            let radix = if hex { 16 } else { 10 };
            let significant = digits.trim_start_matches('0');
            let valid_digits = !digits.is_empty()
                && significant.len() <= 8
                && digits.chars().all(|c| c.is_digit(radix));
            let value = match (valid_digits, significant.is_empty()) {
                (false, _) => None,
                (true, true) => Some(0),
                (true, false) => u32::from_str_radix(significant, radix).ok(),
            };
            let Some(c) = value.and_then(char::from_u32).filter(|&c| is_xml_char(c)) else {
                return self.err_at(amp, XmlErrorKind::InvalidCharRef);
            };
            self.pos += end + 1;
            return Ok(c);
        }
        let name = self.name()?;
        if !self.starts_with(";") {
            return self.err_at(amp, XmlErrorKind::Syntax("reference without ';'"));
        }
        self.pos += 1;
        match name {
            "lt" => Ok('<'),
            "gt" => Ok('>'),
            "amp" => Ok('&'),
            "apos" => Ok('\''),
            "quot" => Ok('"'),
            other => self.err_at(amp, XmlErrorKind::UndefinedEntity(other.to_owned())),
        }
    }

    fn char_data(&mut self) -> R<()> {
        loop {
            let rest = self.rest();
            let Some(i) = rest.find(['<', '&', ']']) else {
                self.pos = self.text.len();
                return Ok(());
            };
            self.pos += i;
            if self.starts_with("]]>") {
                return self.err_at(self.pos, XmlErrorKind::Syntax("']]>' in text"));
            }
            match self.peek() {
                Some('<') => return Ok(()),
                Some('&') => {
                    self.pos += 1;
                    self.reference()?;
                }
                _ => self.pos += 1,
            }
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn events(text: &str) -> R<Vec<Event>> {
        let mut r = Reader::new(text);
        let mut out = Vec::new();
        while let Some(e) = r.next_event()? {
            out.push(e);
        }
        Ok(out)
    }

    fn kind(text: &str) -> XmlErrorKind {
        events(text).expect_err("should fail").kind
    }

    #[test]
    fn simple_document() {
        let ev = events(
            "<?xml version=\"1.0\"?>\n<opt a='1'><b x=\"&lt;&#65;&#x42;\"/>t</opt>\n<!-- c -->",
        )
        .unwrap();
        assert_eq!(ev.len(), 4);
        match &ev[1] {
            Event::Start {
                name,
                attributes,
                line,
            } => {
                assert_eq!(name, "b");
                assert_eq!(Event::attribute(attributes, "x"), "<AB");
                assert_eq!(*line, 2);
            }
            Event::End => panic!(),
        }
    }

    #[test]
    fn attribute_normalisation() {
        let ev = events("<a v=\"x\ty\r\nz\rw&#10;\"/>").unwrap();
        let Event::Start { attributes, .. } = &ev[0] else {
            panic!()
        };
        assert_eq!(attributes[0].value, "x y z w\n");
    }

    #[test]
    fn rejects() {
        assert_eq!(kind(""), XmlErrorKind::NoRoot);
        assert_eq!(kind("  "), XmlErrorKind::NoRoot);
        assert!(matches!(kind("<a>"), XmlErrorKind::UnexpectedEof));
        assert!(matches!(
            kind("<a></b>"),
            XmlErrorKind::MismatchedEndTag { .. }
        ));
        assert!(matches!(
            kind("<a x='1' x='2'/>"),
            XmlErrorKind::DuplicateAttribute(_)
        ));
        assert!(matches!(
            kind("<a>&foo;</a>"),
            XmlErrorKind::UndefinedEntity(_)
        ));
        assert!(matches!(
            kind("<a><!-- a -- b --></a>"),
            XmlErrorKind::Syntax(_)
        ));
        assert!(matches!(kind("<a/><b/>"), XmlErrorKind::TrailingContent));
        assert!(matches!(kind("<a>]]></a>"), XmlErrorKind::Syntax(_)));
        assert!(matches!(kind("<a>\u{1}</a>"), XmlErrorKind::InvalidChar(1)));
        assert!(matches!(kind("<a x='<'/>"), XmlErrorKind::Syntax(_)));
        assert!(matches!(kind("<a x=1/>"), XmlErrorKind::Syntax(_)));
        assert!(matches!(kind("<a x='1'y='2'/>"), XmlErrorKind::Syntax(_)));
        assert!(matches!(kind("<a>&#0;</a>"), XmlErrorKind::InvalidCharRef));
        assert!(matches!(
            kind("<a>&#xD800;</a>"),
            XmlErrorKind::InvalidCharRef
        ));
        assert!(matches!(
            kind(" <?xml version='1.0'?><a/>"),
            XmlErrorKind::Syntax(_)
        ));
        assert!(matches!(kind("<1a/>"), XmlErrorKind::InvalidName));
        assert!(matches!(kind("text<a/>"), XmlErrorKind::Syntax(_)));
    }

    #[test]
    fn doctype_is_ignored_and_entities_are_not_expanded() {
        let doc = "<!DOCTYPE opt SYSTEM \"http://example.invalid/x.dtd\" [\n<!ENTITY e \"boom\">\n<!-- ] -->\n]>\n<opt/>";
        assert_eq!(events(doc).unwrap().len(), 2);
        let doc = "<!DOCTYPE opt [<!ENTITY e \"boom\">]><opt a='&e;'/>";
        assert!(matches!(kind(doc), XmlErrorKind::UndefinedEntity(_)));
    }

    #[test]
    fn depth_limit() {
        let ok = format!(
            "{}{}",
            "<a>".repeat(MAX_XML_DEPTH),
            "</a>".repeat(MAX_XML_DEPTH)
        );
        assert!(events(&ok).is_ok());
        let deep = format!(
            "{}{}",
            "<a>".repeat(MAX_XML_DEPTH + 1),
            "</a>".repeat(MAX_XML_DEPTH + 1)
        );
        assert_eq!(kind(&deep), XmlErrorKind::TooDeep);
    }

    #[test]
    fn encodings() {
        let utf16le: Vec<u8> = [0xFF, 0xFE]
            .into_iter()
            .chain("<a b='é'/>".encode_utf16().flat_map(u16::to_le_bytes))
            .collect();
        assert_eq!(decode_document(&utf16le).unwrap(), "<a b='é'/>");
        let utf16be: Vec<u8> = [0xFE, 0xFF]
            .into_iter()
            .chain("<a/>".encode_utf16().flat_map(u16::to_be_bytes))
            .collect();
        assert_eq!(decode_document(&utf16be).unwrap(), "<a/>");
        let latin1 = b"<?xml version='1.0' encoding='ISO-8859-1'?><a b='\xE9'/>";
        assert!(decode_document(latin1).unwrap().contains('é'));
        let cp = b"<?xml version='1.0' encoding='windows-1252'?><a b='\x80'/>";
        assert!(decode_document(cp).unwrap().contains('€'));
        let ascii_bad = b"<?xml version='1.0' encoding='US-ASCII'?><a b='\xE9'/>";
        assert!(decode_document(ascii_bad).is_err());
        let other = b"<?xml version='1.0' encoding='Shift_JIS'?><a/>";
        assert!(matches!(
            decode_document(other).unwrap_err().kind,
            XmlErrorKind::UnsupportedEncoding(_)
        ));
        let no_bom16: Vec<u8> = "<a/>".encode_utf16().flat_map(u16::to_le_bytes).collect();
        assert!(decode_document(&no_bom16).is_err());
        assert!(decode_document(b"\xEF\xBB\xBF<a/>").is_ok());
        assert!(decode_document(b"<a b='\xFF'/>").is_err());
    }

    #[test]
    fn lines_count_cr_lf_once() {
        let ev = events("<a>\r\n\r\n<b/>\r<c/></a>").unwrap();
        let lines: Vec<u32> = ev
            .iter()
            .filter_map(|e| match e {
                Event::Start { line, .. } => Some(*line),
                Event::End => None,
            })
            .collect();
        assert_eq!(lines, vec![1, 3, 4]);
    }
}
