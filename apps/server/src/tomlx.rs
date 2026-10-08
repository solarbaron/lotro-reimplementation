// SPDX-License-Identifier: GPL-3.0-or-later
//! Spanned TOML reading with line-numbered findings, and small TOML writing helpers.
//!
//! Every file the server reads (configuration, content, accounts, characters, world state) is
//! TOML 1.0 (`test-server.md` §1). Reading goes through the `toml` crate's spanned document
//! model so every finding can name its line (`<file>:<line>: <RULE>: <message>`, §2.6).
//! Inputs are untrusted: sizes are bounded by the callers (files are read with a size cap),
//! and nothing here panics on malformed input.

use std::fmt;
use std::ops::Range;
use std::path::Path;

use toml::Spanned;
use toml::de::{DeArray, DeTable, DeValue};

/// Largest file the server reads as TOML (configuration, content, accounts, characters,
/// world state). Larger files are refused before parsing.
pub const MAX_TOML_FILE: u64 = 4 * 1024 * 1024;

/// The rule a finding breaks (`test-server.md` §2.6).
#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash)]
pub enum Rule {
    /// Not valid TOML, or not UTF-8.
    Syntax,
    /// Missing or unknown `format`, unknown key, missing required key, wrong type.
    Format,
    /// A value outside its range.
    Range,
    /// No SPDX header comment, or a licence other than GPL-3.0-or-later or CC0-1.0.
    Spdx,
    /// More than 16 positions.
    Positions,
    /// More than 8 opaque references.
    References,
    /// A name or text that breaks its text class.
    Text,
    /// A word or phrase of LEGAL.md §10 rule 1.
    Word,
    /// The file lies in a game installation.
    InstallPath,
    /// A configuration finding.
    Config,
}

impl Rule {
    /// The rule's name as printed.
    #[must_use]
    pub fn name(self) -> &'static str {
        match self {
            Self::Syntax => "SYNTAX",
            Self::Format => "FORMAT",
            Self::Range => "RANGE",
            Self::Spdx => "SPDX",
            Self::Positions => "POSITIONS",
            Self::References => "REFERENCES",
            Self::Text => "TEXT",
            Self::Word => "WORD",
            Self::InstallPath => "INSTALL-PATH",
            Self::Config => "CONFIG",
        }
    }
}

impl fmt::Display for Rule {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.write_str(self.name())
    }
}

/// One finding: a line (1-based; 0 when it concerns the whole file), a rule and a message.
#[derive(Debug, Clone, PartialEq, Eq, PartialOrd, Ord, Hash)]
pub struct Finding {
    /// 1-based line, 0 for the whole file.
    pub line: usize,
    /// The rule.
    pub rule: Rule,
    /// What is wrong, in words.
    pub message: String,
}

impl Finding {
    /// A finding.
    #[must_use]
    pub fn new(line: usize, rule: Rule, message: impl Into<String>) -> Self {
        Self {
            line,
            rule,
            message: message.into(),
        }
    }

    /// `<file>:<line>: <RULE>: <message>` (§2.6).
    #[must_use]
    pub fn display(&self, path: &Path) -> String {
        format!(
            "{}:{}: {}: {}",
            path.display(),
            self.line,
            self.rule,
            self.message
        )
    }
}

/// A source text with a line index.
#[derive(Debug)]
pub(crate) struct Source<'a> {
    pub text: &'a str,
    line_starts: Vec<usize>,
}

impl<'a> Source<'a> {
    pub fn new(text: &'a str) -> Self {
        let mut line_starts = vec![0];
        line_starts.extend(
            text.bytes()
                .enumerate()
                .filter(|(_, b)| *b == b'\n')
                .map(|(i, _)| i + 1),
        );
        Self { text, line_starts }
    }

    /// The 1-based line of a byte offset.
    pub fn line(&self, offset: usize) -> usize {
        match self.line_starts.binary_search(&offset) {
            Ok(i) => i + 1,
            Err(i) => i.max(1),
        }
    }

    pub fn line_of(&self, span: &Range<usize>) -> usize {
        self.line(span.start)
    }
}

/// Parses a TOML document. A syntax error becomes one `SYNTAX` finding.
pub(crate) fn parse<'i>(src: &Source<'i>) -> Result<Spanned<DeTable<'i>>, Finding> {
    DeTable::parse(src.text).map_err(|e| {
        let line = e.span().map_or(0, |s| src.line(s.start));
        let msg = e.message().lines().next().unwrap_or("invalid TOML").trim();
        Finding::new(line, Rule::Syntax, format!("invalid TOML: {msg}"))
    })
}

/// Reads a file as UTF-8 text, refusing files over [`MAX_TOML_FILE`]. `Ok(Err(finding))`
/// for a file that exists but is too large or not UTF-8 (a `SYNTAX` finding).
pub(crate) fn read_text(path: &Path) -> std::io::Result<Result<String, Finding>> {
    use std::io::Read;
    let file = std::fs::File::open(path)?;
    let mut bytes = Vec::new();
    file.take(MAX_TOML_FILE + 1).read_to_end(&mut bytes)?;
    if bytes.len() as u64 > MAX_TOML_FILE {
        return Ok(Err(Finding::new(
            0,
            Rule::Syntax,
            format!("file larger than {MAX_TOML_FILE} bytes"),
        )));
    }
    Ok(String::from_utf8(bytes).map_err(|_| Finding::new(0, Rule::Syntax, "not valid UTF-8")))
}

/// The kind of a TOML value, for messages.
pub(crate) fn kind(v: &DeValue<'_>) -> &'static str {
    match v {
        DeValue::String(_) => "a string",
        DeValue::Integer(_) => "an integer",
        DeValue::Float(_) => "a float",
        DeValue::Boolean(_) => "a boolean",
        DeValue::Datetime(_) => "a date-time",
        DeValue::Array(_) => "an array",
        DeValue::Table(_) => "a table",
    }
}

/// An integer value as i64 (underscores allowed, any radix TOML permits).
pub(crate) fn integer(v: &DeValue<'_>) -> Option<i64> {
    let DeValue::Integer(i) = v else { return None };
    let digits: String = i.as_str().chars().filter(|c| *c != '_').collect();
    i64::from_str_radix(&digits, i.radix()).ok()
}

/// A float value (an integer is accepted too). May be non-finite (`inf`, `nan`).
pub(crate) fn float(v: &DeValue<'_>) -> Option<f64> {
    match v {
        DeValue::Float(f) => {
            let s: String = f.as_str().chars().filter(|c| *c != '_').collect();
            s.parse::<f64>().ok()
        }
        DeValue::Integer(_) => integer(v).map(|i| i as f64),
        _ => None,
    }
}

/// A reader that walks a document and collects findings. `type_rule` is the rule used for
/// unknown keys, missing keys and wrong types (`FORMAT` for content, `CONFIG` for the
/// configuration); `range_rule` for values out of range.
pub(crate) struct Reader<'s, 'i> {
    pub src: &'s Source<'i>,
    pub findings: Vec<Finding>,
    pub type_rule: Rule,
    pub range_rule: Rule,
}

/// A table being read, with its dotted path for messages.
#[derive(Clone, Copy)]
pub(crate) struct View<'a, 'i> {
    pub table: &'a DeTable<'i>,
    /// The line of the table's header (for "missing key" findings).
    pub line: usize,
    pub path: &'a str,
}

impl<'s, 'i> Reader<'s, 'i> {
    pub fn new(src: &'s Source<'i>, type_rule: Rule, range_rule: Rule) -> Self {
        Self {
            src,
            findings: Vec::new(),
            type_rule,
            range_rule,
        }
    }

    pub fn line(&self, span: &Range<usize>) -> usize {
        self.src.line_of(span)
    }

    pub fn push(&mut self, line: usize, rule: Rule, message: impl Into<String>) {
        self.findings.push(Finding::new(line, rule, message));
    }

    fn key_name(path: &str, key: &str) -> String {
        if path.is_empty() {
            key.to_owned()
        } else {
            format!("{path}.{key}")
        }
    }

    /// Reports every key of `view` not in `allowed`.
    pub fn check_keys(&mut self, view: View<'_, '_>, allowed: &[&str]) {
        for (k, _) in view.table.iter() {
            if !allowed.contains(&k.get_ref().as_ref()) {
                let line = self.line(&k.span());
                let name = Self::key_name(view.path, k.get_ref());
                self.push(line, self.type_rule, format!("unknown key `{name}`"));
            }
        }
    }

    /// The raw value of `key`, reporting it when required and missing.
    pub fn value<'a, 'j>(
        &mut self,
        view: View<'a, 'j>,
        key: &str,
        required: bool,
    ) -> Option<&'a Spanned<DeValue<'j>>> {
        let v = view.table.get(key);
        if v.is_none() && required {
            let line = view.line;
            let name = Self::key_name(view.path, key);
            self.push(
                line,
                self.type_rule,
                format!("missing required key `{name}`"),
            );
        }
        v
    }

    fn wrong_type(&mut self, view: View<'_, '_>, key: &str, v: &Spanned<DeValue<'_>>, want: &str) {
        let line = self.line(&v.span());
        let name = Self::key_name(view.path, key);
        let got = kind(v.get_ref());
        self.push(
            line,
            self.type_rule,
            format!("`{name}` must be {want}, not {got}"),
        );
    }

    /// A string value and its line.
    pub fn string(
        &mut self,
        view: View<'_, '_>,
        key: &str,
        required: bool,
    ) -> Option<(String, usize)> {
        let v = self.value(view, key, required)?;
        match v.get_ref() {
            DeValue::String(s) => Some((s.to_string(), self.line(&v.span()))),
            _ => {
                self.wrong_type(view, key, v, "a string");
                None
            }
        }
    }

    /// An integer within `[min, max]` and its line.
    pub fn int(
        &mut self,
        view: View<'_, '_>,
        key: &str,
        required: bool,
        min: i64,
        max: i64,
    ) -> Option<(i64, usize)> {
        let v = self.value(view, key, required)?;
        let line = self.line(&v.span());
        let Some(i) = integer(v.get_ref()) else {
            if matches!(v.get_ref(), DeValue::Integer(_)) {
                let name = Self::key_name(view.path, key);
                self.push(line, self.range_rule, format!("`{name}` is out of range"));
            } else {
                self.wrong_type(view, key, v, "an integer");
            }
            return None;
        };
        if i < min || i > max {
            let name = Self::key_name(view.path, key);
            self.push(
                line,
                self.range_rule,
                format!("`{name}` = {i} is outside {min}–{max}"),
            );
            return None;
        }
        Some((i, line))
    }

    /// A finite float and its line (range checks are the caller's).
    pub fn float(&mut self, view: View<'_, '_>, key: &str, required: bool) -> Option<(f64, usize)> {
        let v = self.value(view, key, required)?;
        let line = self.line(&v.span());
        let Some(f) = float(v.get_ref()) else {
            self.wrong_type(view, key, v, "a number");
            return None;
        };
        if !f.is_finite() {
            let name = Self::key_name(view.path, key);
            self.push(line, self.range_rule, format!("`{name}` must be finite"));
            return None;
        }
        Some((f, line))
    }

    /// A float that must lie in a range; `check` returns false for out-of-range values and
    /// `describe` names the range.
    pub fn float_in(
        &mut self,
        view: View<'_, '_>,
        key: &str,
        required: bool,
        check: impl Fn(f64) -> bool,
        describe: &str,
    ) -> Option<(f64, usize)> {
        let (f, line) = self.float(view, key, required)?;
        if !check(f) {
            let name = Self::key_name(view.path, key);
            self.push(
                line,
                self.range_rule,
                format!("`{name}` = {f} is outside {describe}"),
            );
            return None;
        }
        Some((f, line))
    }

    /// A boolean.
    pub fn bool(&mut self, view: View<'_, '_>, key: &str, required: bool) -> Option<(bool, usize)> {
        let v = self.value(view, key, required)?;
        match v.get_ref() {
            DeValue::Boolean(b) => Some((*b, self.line(&v.span()))),
            _ => {
                self.wrong_type(view, key, v, "a boolean");
                None
            }
        }
    }

    /// An array.
    pub fn array<'a, 'j>(
        &mut self,
        view: View<'a, 'j>,
        key: &str,
        required: bool,
    ) -> Option<(&'a DeArray<'j>, usize)> {
        let v = self.value(view, key, required)?;
        match v.get_ref() {
            DeValue::Array(a) => Some((a, self.line(&v.span()))),
            _ => {
                self.wrong_type(view, key, v, "an array");
                None
            }
        }
    }

    /// A table.
    pub fn table<'a, 'j>(
        &mut self,
        view: View<'a, 'j>,
        key: &str,
        required: bool,
    ) -> Option<(&'a DeTable<'j>, &'a Spanned<DeValue<'j>>)> {
        let v = self.value(view, key, required)?;
        match v.get_ref() {
            DeValue::Table(t) => Some((t, v)),
            _ => {
                self.wrong_type(view, key, v, "a table");
                None
            }
        }
    }

    /// An offset date-time in UTC (`Z` or `+00:00`), returned in its text form.
    pub fn utc_datetime(
        &mut self,
        view: View<'_, '_>,
        key: &str,
        required: bool,
    ) -> Option<(String, usize)> {
        let v = self.value(view, key, required)?;
        let line = self.line(&v.span());
        match v.get_ref() {
            DeValue::Datetime(d) => {
                let text = d.to_string();
                let utc = text.ends_with('Z') || text.ends_with("+00:00");
                if d.date.is_none() || d.time.is_none() || d.offset.is_none() || !utc {
                    let name = Self::key_name(view.path, key);
                    self.push(
                        line,
                        self.range_rule,
                        format!("`{name}` must be an offset date-time in UTC"),
                    );
                    return None;
                }
                Some((text, line))
            }
            _ => {
                self.wrong_type(view, key, v, "a date-time");
                None
            }
        }
    }

    /// The tables of an array of tables (`[[name]]`), each with its span.
    pub fn tables<'a, 'j>(
        &mut self,
        view: View<'a, 'j>,
        key: &str,
    ) -> Vec<(&'a DeTable<'j>, usize)> {
        let Some((arr, _)) = self.array(view, key, false) else {
            return Vec::new();
        };
        let mut out = Vec::new();
        for item in arr.iter() {
            match item.get_ref() {
                DeValue::Table(t) => out.push((t, self.line(&item.span()))),
                other => {
                    let line = self.line(&item.span());
                    let name = Self::key_name(view.path, key);
                    let got = kind(other);
                    self.push(
                        line,
                        self.type_rule,
                        format!("`{name}` must hold tables, not {got}"),
                    );
                }
            }
        }
        out
    }
}

/// Escapes a string as a TOML basic string, quotes included.
#[must_use]
pub fn quote(s: &str) -> String {
    let mut out = String::with_capacity(s.len() + 2);
    out.push('"');
    for c in s.chars() {
        match c {
            '"' => out.push_str("\\\""),
            '\\' => out.push_str("\\\\"),
            '\n' => out.push_str("\\n"),
            '\t' => out.push_str("\\t"),
            '\r' => out.push_str("\\r"),
            c if c.is_control() => out.push_str(&format!("\\u{:04X}", u32::from(c))),
            c => out.push(c),
        }
    }
    out.push('"');
    out
}

/// A finite float in a form TOML parses back to the same value.
#[must_use]
pub fn float_text(v: f64) -> String {
    if v.is_finite() {
        let s = format!("{v:?}");
        if s.contains(['.', 'e', 'E']) {
            s
        } else {
            format!("{s}.0")
        }
    } else {
        "0.0".to_owned()
    }
}

/// Deepest value [`inline_value`] writes back.
const MAX_INLINE_DEPTH: usize = 32;

fn bare_key(k: &str) -> bool {
    !k.is_empty()
        && k.bytes()
            .all(|b| b.is_ascii_alphanumeric() || b == b'_' || b == b'-')
}

/// A parsed value written back as TOML that parses to the same value, using inline tables
/// for every table however it was written (header, dotted keys or inline). `None` for
/// values nested deeper than 32 levels.
#[must_use]
pub fn inline_value(v: &DeValue<'_>) -> Option<String> {
    let mut out = String::new();
    write_inline(v, 0, &mut out).then_some(out)
}

fn write_inline(v: &DeValue<'_>, depth: usize, out: &mut String) -> bool {
    if depth > MAX_INLINE_DEPTH {
        return false;
    }
    match v {
        DeValue::String(s) => out.push_str(&quote(s)),
        DeValue::Integer(i) => {
            out.push_str(match i.radix() {
                16 => "0x",
                8 => "0o",
                2 => "0b",
                _ => "",
            });
            out.push_str(i.as_str());
        }
        DeValue::Float(f) => out.push_str(f.as_str()),
        DeValue::Boolean(b) => out.push_str(if *b { "true" } else { "false" }),
        DeValue::Datetime(d) => out.push_str(&d.to_string()),
        DeValue::Array(a) => {
            out.push('[');
            for (n, item) in a.iter().enumerate() {
                if n > 0 {
                    out.push_str(", ");
                }
                if !write_inline(item.get_ref(), depth + 1, out) {
                    return false;
                }
            }
            out.push(']');
        }
        DeValue::Table(t) => {
            out.push('{');
            for (n, (k, item)) in t.iter().enumerate() {
                out.push_str(if n > 0 { ", " } else { " " });
                let k: &str = k.get_ref();
                if bare_key(k) {
                    out.push_str(k);
                } else {
                    out.push_str(&quote(k));
                }
                out.push_str(" = ");
                if !write_inline(item.get_ref(), depth + 1, out) {
                    return false;
                }
            }
            out.push_str(if t.is_empty() { "}" } else { " }" });
        }
    }
    true
}
