// SPDX-License-Identifier: GPL-3.0-or-later
//! `.plugindata` files: value model, key rules, reader (no code execution) and writer.
//!
//! Spec: `docs/specs/lua-plugin-api.md` §10.2 (validation), §10.4 (file format),
//! §10.5 (what can be saved), §10.6 (atomic writes).

use std::collections::BTreeMap;
use std::io::Write as _;
use std::path::{Path, PathBuf};
use std::sync::atomic::{AtomicU64, Ordering};

/// Longest key accepted, in bytes (§10.2).
pub const MAX_KEY_LEN: usize = 63;

/// File extension of plugin-data files (§10.3).
pub const EXTENSION: &str = "plugindata";

/// Errors from plugin-data validation, conversion, parsing and I/O.
#[derive(Debug, thiserror::Error)]
pub enum PluginDataError {
    /// Scope is not 1, 2 or 3.
    #[error("invalid data scope")]
    InvalidScope,
    /// The host has no folder for this scope right now.
    #[error("data scope is not available")]
    ScopeUnavailable,
    /// Key breaks the rules of §10.2.
    #[error("invalid plugin data key: {0}")]
    InvalidKey(&'static str),
    /// A value of an unsaveable type (§10.5).
    #[error("cannot save a value of type {0}")]
    Unsupported(&'static str),
    /// A table key that is not a finite number or a string.
    #[error("cannot save a table key of type {0}")]
    UnsupportedKey(&'static str),
    /// NaN or infinity (§10.5).
    #[error("cannot save NaN or infinite numbers")]
    NonFinite,
    /// A table contains itself (§10.5).
    #[error("cannot save a table that contains itself")]
    Cycle,
    /// Nesting deeper than the limit.
    #[error("nesting deeper than {0} levels")]
    TooDeep(usize),
    /// More entries than the limit.
    #[error("more than {0} entries")]
    TooManyEntries(usize),
    /// File larger than the limit.
    #[error("file is {0} bytes, limit is {1}")]
    TooLarge(u64, u64),
    /// A value to save would serialize to more than the limit (so `Load` could not read
    /// it back).
    #[error("saved data would be larger than {0} bytes")]
    ValueTooLarge(u64),
    /// Syntax error in a `.plugindata` file.
    #[error("parse error at byte {offset}: {reason}")]
    Parse {
        /// Byte offset of the error.
        offset: usize,
        /// What was wrong.
        reason: &'static str,
    },
    /// File-system error.
    #[error("i/o error at {path}: {source}")]
    Io {
        /// Path involved.
        path: PathBuf,
        /// Underlying error.
        #[source]
        source: std::io::Error,
    },
}

/// A table key: a finite number (with `-0` folded to `0`) or a byte string.
///
/// Ordering is numbers ascending first, then strings bytewise (§10.4 writing rule).
#[derive(Debug, Clone, PartialEq)]
pub enum DataKey {
    /// Finite number key.
    Number(f64),
    /// Byte-string key.
    String(Vec<u8>),
}

impl DataKey {
    /// Build a number key, rejecting NaN and infinities and folding `-0` to `0`.
    pub fn number(n: f64) -> Result<Self, PluginDataError> {
        if !n.is_finite() {
            return Err(PluginDataError::NonFinite);
        }
        Ok(Self::Number(if n == 0.0 { 0.0 } else { n }))
    }
}

impl Eq for DataKey {}

impl Ord for DataKey {
    fn cmp(&self, other: &Self) -> std::cmp::Ordering {
        use std::cmp::Ordering::{Greater, Less};
        match (self, other) {
            (Self::Number(a), Self::Number(b)) => a.total_cmp(b),
            (Self::Number(_), Self::String(_)) => Less,
            (Self::String(_), Self::Number(_)) => Greater,
            (Self::String(a), Self::String(b)) => a.cmp(b),
        }
    }
}

impl PartialOrd for DataKey {
    fn partial_cmp(&self, other: &Self) -> Option<std::cmp::Ordering> {
        Some(self.cmp(other))
    }
}

/// A saveable value (§10.5).
#[derive(Debug, Clone, PartialEq, Default)]
pub enum DataValue {
    /// `nil` (nothing stored).
    #[default]
    Nil,
    /// Boolean.
    Boolean(bool),
    /// Number (a Lua 5.1 double).
    Number(f64),
    /// Byte string (Lua strings are 8-bit clean).
    String(Vec<u8>),
    /// Table; `nil`-valued entries are never stored.
    Table(BTreeMap<DataKey, DataValue>),
}

/// Bounds applied when reading and writing files.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct ParseLimits {
    /// Largest file accepted, in bytes.
    pub max_bytes: u64,
    /// Deepest table nesting accepted.
    pub max_depth: usize,
    /// Most table entries accepted in total.
    pub max_entries: usize,
}

/// Lua 5.1 stores pending positional constructor items in batches of this many: a batch
/// is written when the field after its last item starts, and the rest at the closing
/// brace. Keyed fields are written as they appear.
const POSITIONAL_BATCH: usize = 50;

impl Default for ParseLimits {
    fn default() -> Self {
        Self {
            max_bytes: 16 * 1024 * 1024,
            max_depth: 64,
            max_entries: 1_000_000,
        }
    }
}

/// Validate a key per §10.2: 1–63 bytes, none `< 0x20` or one of `/ \ : * ? " < > |`, not
/// ending in `.` or space (which also excludes `.` and `..`).
pub fn validate_key(key: &[u8]) -> Result<&str, PluginDataError> {
    if key.is_empty() {
        return Err(PluginDataError::InvalidKey("empty"));
    }
    if key.len() > MAX_KEY_LEN {
        return Err(PluginDataError::InvalidKey("longer than 63 bytes"));
    }
    if key.iter().any(|&b| b < 0x20 || b"/\\:*?\"<>|".contains(&b)) {
        return Err(PluginDataError::InvalidKey(
            "contains a character not allowed in file names",
        ));
    }
    if key.ends_with(b".") || key.ends_with(b" ") {
        return Err(PluginDataError::InvalidKey("ends in '.' or space"));
    }
    std::str::from_utf8(key).map_err(|_| PluginDataError::InvalidKey("not valid UTF-8"))
}

// ---------------------------------------------------------------------------------------
// Writer
// ---------------------------------------------------------------------------------------

/// Serialize `value` in the `.plugindata` layout of §10.4 (`return`, a table constructor,
/// tab indentation, CRLF line ends, sorted bracketed keys).
pub fn serialize(value: &DataValue) -> Vec<u8> {
    let mut out = Vec::new();
    match value {
        DataValue::Table(t) => {
            out.extend_from_slice(b"return\r\n");
            write_table(&mut out, t, 0);
        }
        other => {
            out.extend_from_slice(b"return ");
            write_scalar(&mut out, other);
        }
    }
    out.extend_from_slice(b"\r\n");
    out
}

fn indent(out: &mut Vec<u8>, level: usize) {
    out.extend(std::iter::repeat_n(b'\t', level));
}

/// Writes `{ ... }` with the opening brace at `level` (cursor at line start, unindented).
fn write_table(out: &mut Vec<u8>, t: &BTreeMap<DataKey, DataValue>, level: usize) {
    indent(out, level);
    out.extend_from_slice(b"{\r\n");
    if t.is_empty() {
        out.extend_from_slice(b"\r\n");
    }
    let n = t.len();
    for (i, (k, v)) in t.iter().enumerate() {
        indent(out, level + 1);
        out.push(b'[');
        match k {
            DataKey::Number(n) => out.extend_from_slice(format_number(*n).as_bytes()),
            DataKey::String(s) => write_string(out, s),
        }
        out.extend_from_slice(b"] =");
        match v {
            DataValue::Table(inner) => {
                out.extend_from_slice(b"\r\n");
                write_table(out, inner, level + 1);
            }
            scalar => {
                out.push(b' ');
                write_scalar(out, scalar);
            }
        }
        if i + 1 < n {
            out.push(b',');
        }
        out.extend_from_slice(b"\r\n");
    }
    indent(out, level);
    out.push(b'}');
}

fn write_scalar(out: &mut Vec<u8>, v: &DataValue) {
    match v {
        DataValue::Nil => out.extend_from_slice(b"nil"),
        DataValue::Boolean(true) => out.extend_from_slice(b"true"),
        DataValue::Boolean(false) => out.extend_from_slice(b"false"),
        DataValue::Number(n) => out.extend_from_slice(format_number(*n).as_bytes()),
        DataValue::String(s) => write_string(out, s),
        DataValue::Table(t) => write_table(out, t, 0),
    }
}

fn write_string(out: &mut Vec<u8>, s: &[u8]) {
    out.push(b'"');
    for &b in s {
        match b {
            b'"' => out.extend_from_slice(b"\\\""),
            b'\\' => out.extend_from_slice(b"\\\\"),
            b'\n' => out.extend_from_slice(b"\\n"),
            b'\r' => out.extend_from_slice(b"\\r"),
            b'\t' => out.extend_from_slice(b"\\t"),
            0..=0x1F | 0x7F => out.extend_from_slice(format!("\\{b:03}").as_bytes()),
            _ => out.push(b),
        }
    }
    out.push(b'"');
}

/// Locale-independent number text (§3.4, §10.4): integral values as integers, others with
/// 17 significant digits (`%.17g` style), always `.` as the decimal separator.
pub fn format_number(n: f64) -> String {
    if !n.is_finite() {
        // Never produced by the converters (they reject non-finite numbers); keep the
        // output parseable as an expression anyway.
        return if n.is_nan() {
            "(0/0)".to_owned()
        } else if n > 0.0 {
            "(1/0)".to_owned()
        } else {
            "(-1/0)".to_owned()
        };
    }
    if n == n.trunc() && n.abs() < 9.0e18 {
        return format!("{}", n as i64);
    }
    let sci = format!("{n:.16e}");
    let Some((mant, exp)) = sci.split_once('e') else {
        return format!("{n:?}");
    };
    let Ok(exp) = exp.parse::<i32>() else {
        return format!("{n:?}");
    };
    let neg = mant.starts_with('-');
    let digits: String = mant.chars().filter(char::is_ascii_digit).collect();
    let mut s = String::new();
    if neg {
        s.push('-');
    }
    if !(-4..17).contains(&exp) {
        s.push_str(&digits[..1]);
        let frac = digits[1..].trim_end_matches('0');
        if !frac.is_empty() {
            s.push('.');
            s.push_str(frac);
        }
        s.push_str(&format!(
            "e{}{:02}",
            if exp < 0 { '-' } else { '+' },
            exp.abs()
        ));
    } else if exp >= 0 {
        let int_len = (exp as usize + 1).min(digits.len());
        s.push_str(&digits[..int_len]);
        let frac = digits[int_len..].trim_end_matches('0');
        if !frac.is_empty() {
            s.push('.');
            s.push_str(frac);
        }
    } else {
        s.push_str("0.");
        for _ in 0..(-exp - 1) {
            s.push('0');
        }
        s.push_str(digits.trim_end_matches('0'));
    }
    s
}

// ---------------------------------------------------------------------------------------
// Reader
// ---------------------------------------------------------------------------------------

/// Parse a `.plugindata` file with the restricted grammar of §10.4. No code is executed.
pub fn parse(bytes: &[u8], limits: &ParseLimits) -> Result<DataValue, PluginDataError> {
    if bytes.len() as u64 > limits.max_bytes {
        return Err(PluginDataError::TooLarge(
            bytes.len() as u64,
            limits.max_bytes,
        ));
    }
    let bytes = bytes.strip_prefix(b"\xEF\xBB\xBF").unwrap_or(bytes);
    let mut p = Parser {
        s: bytes,
        i: 0,
        limits,
        entries: 0,
    };
    p.ws()?;
    if p.peek_word() == Some(b"return".as_slice()) {
        p.i += 6;
        p.ws()?;
    }
    let v = p.value(0)?;
    p.ws()?;
    if p.peek() == Some(b';') {
        p.i += 1;
        p.ws()?;
    }
    if p.i != p.s.len() {
        return Err(p.err("trailing data"));
    }
    Ok(v)
}

struct Parser<'a> {
    s: &'a [u8],
    i: usize,
    limits: &'a ParseLimits,
    entries: usize,
}

fn is_name_start(b: u8) -> bool {
    b.is_ascii_alphabetic() || b == b'_'
}

fn is_name_char(b: u8) -> bool {
    b.is_ascii_alphanumeric() || b == b'_'
}

impl<'a> Parser<'a> {
    fn err(&self, reason: &'static str) -> PluginDataError {
        PluginDataError::Parse {
            offset: self.i,
            reason,
        }
    }

    fn peek(&self) -> Option<u8> {
        self.s.get(self.i).copied()
    }

    fn peek_at(&self, off: usize) -> Option<u8> {
        self.s.get(self.i + off).copied()
    }

    /// The identifier starting at the cursor, if any.
    fn peek_word(&self) -> Option<&'a [u8]> {
        let start = self.i;
        if !self.peek().is_some_and(is_name_start) {
            return None;
        }
        let mut end = start;
        while self.s.get(end).copied().is_some_and(is_name_char) {
            end += 1;
        }
        Some(&self.s[start..end])
    }

    /// Skip whitespace and comments.
    fn ws(&mut self) -> Result<(), PluginDataError> {
        loop {
            match self.peek() {
                Some(b' ' | b'\t' | b'\r' | b'\n' | 0x0B | 0x0C) => self.i += 1,
                Some(b'-') if self.peek_at(1) == Some(b'-') => {
                    self.i += 2;
                    if let Some(level) = self.long_bracket_level() {
                        self.long_string(level)?;
                        continue;
                    }
                    while let Some(b) = self.peek() {
                        if b == b'\n' || b == b'\r' {
                            break;
                        }
                        self.i += 1;
                    }
                }
                _ => return Ok(()),
            }
        }
    }

    fn value(&mut self, depth: usize) -> Result<DataValue, PluginDataError> {
        match self.peek() {
            None => Err(self.err("unexpected end of input")),
            Some(b'{') => self.table(depth),
            Some(b'"' | b'\'') => Ok(DataValue::String(self.quoted()?)),
            Some(b'[') => match self.long_bracket_level() {
                Some(level) => Ok(DataValue::String(self.long_string(level)?)),
                None => Err(self.err("unexpected '['")),
            },
            Some(b'-') => {
                // Unary minus, possibly repeated (`- -1`); iterative so hostile input
                // cannot recurse deeply.
                let mut neg = false;
                while self.peek() == Some(b'-') {
                    self.i += 1;
                    neg = !neg;
                    self.ws()?;
                }
                match self.peek() {
                    Some(b) if b.is_ascii_digit() || b == b'.' => {
                        let n = self.numeral()?;
                        Ok(DataValue::Number(if neg { -n } else { n }))
                    }
                    _ => Err(self.err("'-' applied to a non-number")),
                }
            }
            Some(b) if b.is_ascii_digit() || b == b'.' => Ok(DataValue::Number(self.numeral()?)),
            Some(_) => match self.peek_word() {
                Some(b"nil") => {
                    self.i += 3;
                    Ok(DataValue::Nil)
                }
                Some(b"true") => {
                    self.i += 4;
                    Ok(DataValue::Boolean(true))
                }
                Some(b"false") => {
                    self.i += 5;
                    Ok(DataValue::Boolean(false))
                }
                _ => Err(self.err("expected a value")),
            },
        }
    }

    /// A Lua 5.1 numeral (decimal with optional fraction/exponent, or `0x` hex).
    fn numeral(&mut self) -> Result<f64, PluginDataError> {
        let start = self.i;
        while let Some(b) = self.peek() {
            if b.is_ascii_digit() || b == b'.' {
                self.i += 1;
            } else {
                break;
            }
        }
        if matches!(self.peek(), Some(b'e' | b'E')) {
            self.i += 1;
            if matches!(self.peek(), Some(b'+' | b'-')) {
                self.i += 1;
            }
        }
        while self.peek().is_some_and(is_name_char) {
            self.i += 1;
        }
        let text =
            std::str::from_utf8(&self.s[start..self.i]).map_err(|_| self.err("bad number"))?;
        if let Some(hex) = text.strip_prefix("0x").or_else(|| text.strip_prefix("0X")) {
            return u64::from_str_radix(hex, 16)
                .map(|v| v as f64)
                .map_err(|_| self.err("bad hexadecimal number"));
        }
        // Rust's parser also accepts "inf"/"nan"; the first byte is a digit or '.' here, so
        // only genuine numerals reach it. Reject forms Lua's lexer would not produce.
        if text
            .bytes()
            .any(|b| b.is_ascii_alphabetic() && b != b'e' && b != b'E')
        {
            return Err(self.err("bad number"));
        }
        let v: f64 = text.parse().map_err(|_| self.err("bad number"))?;
        if v.is_finite() {
            Ok(v)
        } else {
            Err(self.err("number out of range"))
        }
    }

    /// `[` followed by `=`* and `[`: returns the level without consuming.
    fn long_bracket_level(&self) -> Option<usize> {
        if self.peek() != Some(b'[') {
            return None;
        }
        let mut j = self.i + 1;
        let mut level = 0;
        while self.s.get(j) == Some(&b'=') {
            level += 1;
            j += 1;
        }
        (self.s.get(j) == Some(&b'[')).then_some(level)
    }

    fn long_string(&mut self, level: usize) -> Result<Vec<u8>, PluginDataError> {
        self.i += level + 2;
        self.skip_newline();
        let mut out = Vec::new();
        loop {
            match self.peek() {
                None => return Err(self.err("unfinished long string")),
                Some(b']') => {
                    let mut j = self.i + 1;
                    let mut l = 0;
                    while self.s.get(j) == Some(&b'=') {
                        l += 1;
                        j += 1;
                    }
                    if l == level && self.s.get(j) == Some(&b']') {
                        self.i = j + 1;
                        return Ok(out);
                    }
                    out.push(b']');
                    self.i += 1;
                }
                Some(b'\r' | b'\n') => {
                    self.skip_newline();
                    out.push(b'\n');
                }
                Some(b) => {
                    out.push(b);
                    self.i += 1;
                }
            }
        }
    }

    /// Consume one line break (`\n`, `\r`, `\r\n` or `\n\r`) if present.
    fn skip_newline(&mut self) {
        match (self.peek(), self.peek_at(1)) {
            (Some(b'\r'), Some(b'\n')) | (Some(b'\n'), Some(b'\r')) => self.i += 2,
            (Some(b'\r' | b'\n'), _) => self.i += 1,
            _ => {}
        }
    }

    fn quoted(&mut self) -> Result<Vec<u8>, PluginDataError> {
        let Some(q) = self.peek() else {
            return Err(self.err("expected a string"));
        };
        self.i += 1;
        let mut out = Vec::new();
        loop {
            let Some(b) = self.peek() else {
                return Err(self.err("unfinished string"));
            };
            if b == q {
                self.i += 1;
                return Ok(out);
            }
            match b {
                b'\n' | b'\r' => return Err(self.err("unfinished string")),
                b'\\' => {
                    self.i += 1;
                    let Some(e) = self.peek() else {
                        return Err(self.err("unfinished string"));
                    };
                    match e {
                        b'a' => out.push(0x07),
                        b'b' => out.push(0x08),
                        b'f' => out.push(0x0C),
                        b'n' => out.push(b'\n'),
                        b'r' => out.push(b'\r'),
                        b't' => out.push(b'\t'),
                        b'v' => out.push(0x0B),
                        b'\n' | b'\r' => {
                            self.skip_newline();
                            out.push(b'\n');
                            continue;
                        }
                        b'0'..=b'9' => {
                            let mut v: u32 = 0;
                            let mut n = 0;
                            while n < 3 {
                                match self.peek() {
                                    Some(d @ b'0'..=b'9') => {
                                        v = v * 10 + u32::from(d - b'0');
                                        self.i += 1;
                                        n += 1;
                                    }
                                    _ => break,
                                }
                            }
                            let byte = u8::try_from(v).map_err(|_| self.err("escape too large"))?;
                            out.push(byte);
                            continue;
                        }
                        other => out.push(other),
                    }
                    self.i += 1;
                }
                _ => {
                    out.push(b);
                    self.i += 1;
                }
            }
        }
    }

    fn table(&mut self, depth: usize) -> Result<DataValue, PluginDataError> {
        if depth >= self.limits.max_depth {
            return Err(PluginDataError::TooDeep(self.limits.max_depth));
        }
        self.i += 1; // '{'
        let mut map = BTreeMap::new();
        let mut next_index = 1.0_f64;
        // Positional items not yet stored (Lua 5.1 constructor order, see
        // `POSITIONAL_BATCH`): a positional item overrides an explicit `[n]` key of the
        // same batch whichever comes first.
        let mut pending: Vec<(DataKey, DataValue)> = Vec::new();
        let store = |map: &mut BTreeMap<DataKey, DataValue>, key: DataKey, value: DataValue| {
            if value == DataValue::Nil {
                map.remove(&key);
            } else {
                map.insert(key, value);
            }
        };
        loop {
            self.ws()?;
            if pending.len() >= POSITIONAL_BATCH {
                for (k, v) in pending.drain(..) {
                    store(&mut map, k, v);
                }
            }
            match self.peek() {
                None => return Err(self.err("unfinished table")),
                Some(b'}') => {
                    self.i += 1;
                    for (k, v) in pending.drain(..) {
                        store(&mut map, k, v);
                    }
                    return Ok(DataValue::Table(map));
                }
                _ => {}
            }
            self.entries += 1;
            if self.entries > self.limits.max_entries {
                return Err(PluginDataError::TooManyEntries(self.limits.max_entries));
            }
            let (key, value, positional) = self.field(depth, &mut next_index)?;
            if positional {
                pending.push((key, value));
            } else {
                store(&mut map, key, value);
            }
            self.ws()?;
            match self.peek() {
                Some(b',' | b';') => self.i += 1,
                Some(b'}') => {}
                _ => return Err(self.err("expected ',' or '}'")),
            }
        }
    }

    fn field(
        &mut self,
        depth: usize,
        next_index: &mut f64,
    ) -> Result<(DataKey, DataValue, bool), PluginDataError> {
        // `[key] = value` (but `[[`/`[=` start a long string value).
        if self.peek() == Some(b'[') && self.long_bracket_level().is_none() {
            self.i += 1;
            self.ws()?;
            let key = self.bracket_key()?;
            self.ws()?;
            if self.peek() != Some(b']') {
                return Err(self.err("expected ']'"));
            }
            self.i += 1;
            self.ws()?;
            self.expect_assign()?;
            self.ws()?;
            let v = self.value(depth + 1)?;
            return Ok((key, v, false));
        }
        // `name = value`
        if let Some(word) = self.peek_word()
            && !matches!(word, b"nil" | b"true" | b"false")
        {
            let save = self.i;
            self.i += word.len();
            self.ws()?;
            if self.peek() == Some(b'=') && self.peek_at(1) != Some(b'=') {
                self.i += 1;
                self.ws()?;
                let v = self.value(depth + 1)?;
                return Ok((DataKey::String(word.to_vec()), v, false));
            }
            self.i = save;
            return Err(self.err("expected '=' after a name"));
        }
        // positional value
        let v = self.value(depth + 1)?;
        let key = DataKey::Number(*next_index);
        *next_index += 1.0;
        Ok((key, v, true))
    }

    fn expect_assign(&mut self) -> Result<(), PluginDataError> {
        if self.peek() == Some(b'=') && self.peek_at(1) != Some(b'=') {
            self.i += 1;
            Ok(())
        } else {
            Err(self.err("expected '='"))
        }
    }

    /// Key inside `[...]`: a number (also `12,5` with a comma decimal separator, §10.4) or a
    /// string.
    fn bracket_key(&mut self) -> Result<DataKey, PluginDataError> {
        let neg = if self.peek() == Some(b'-') && self.peek_at(1) != Some(b'-') {
            self.i += 1;
            self.ws()?;
            true
        } else {
            false
        };
        match self.peek() {
            Some(b) if b.is_ascii_digit() || b == b'.' => {
                let mut v = self.numeral()?;
                if self.peek() == Some(b',') && self.peek_at(1).is_some_and(|b| b.is_ascii_digit())
                {
                    let start = self.i + 1;
                    let mut j = start;
                    while self.s.get(j).is_some_and(u8::is_ascii_digit) {
                        j += 1;
                    }
                    let frac = std::str::from_utf8(&self.s[start..j]).unwrap_or("0");
                    let whole = format!("{}.{frac}", format_number(v));
                    v = whole.parse().map_err(|_| self.err("bad number"))?;
                    self.i = j;
                }
                DataKey::number(if neg { -v } else { v }).map_err(|_| self.err("bad number key"))
            }
            _ if neg => Err(self.err("'-' applied to a non-number")),
            Some(b'"' | b'\'') => Ok(DataKey::String(self.quoted()?)),
            Some(b'[') => match self.long_bracket_level() {
                Some(level) => Ok(DataKey::String(self.long_string(level)?)),
                None => Err(self.err("bad key")),
            },
            _ => Err(self.err("key must be a number or a string")),
        }
    }
}

// ---------------------------------------------------------------------------------------
// Files
// ---------------------------------------------------------------------------------------

/// Path of the file holding `key` in `dir`.
pub fn file_path(dir: &Path, key: &str) -> PathBuf {
    dir.join(format!("{key}.{EXTENSION}"))
}

/// Write `value` under `key` in `dir` atomically (temporary file, then rename; §10.6).
/// Creates `dir` if needed. Refuses data that would serialize to more than
/// `limits.max_bytes`, so everything written can be read back by [`load_file`].
pub fn save_file(
    dir: &Path,
    key: &str,
    value: &DataValue,
    limits: &ParseLimits,
) -> Result<(), PluginDataError> {
    static COUNTER: AtomicU64 = AtomicU64::new(0);
    let io = |path: &Path| {
        let path = path.to_path_buf();
        move |source| PluginDataError::Io { path, source }
    };
    let bytes = serialize(value);
    if bytes.len() as u64 > limits.max_bytes {
        return Err(PluginDataError::ValueTooLarge(limits.max_bytes));
    }
    std::fs::create_dir_all(dir).map_err(io(dir))?;
    let target = file_path(dir, key);
    let tmp = dir.join(format!(
        ".{key}.{}.{}.tmp",
        std::process::id(),
        COUNTER.fetch_add(1, Ordering::Relaxed)
    ));
    let result = (|| {
        let mut f = std::fs::File::create(&tmp).map_err(io(&tmp))?;
        f.write_all(&bytes).map_err(io(&tmp))?;
        f.sync_all().map_err(io(&tmp))?;
        drop(f);
        std::fs::rename(&tmp, &target).map_err(io(&target))
    })();
    if result.is_err() {
        let _ = std::fs::remove_file(&tmp);
    }
    result
}

/// Read and parse `key` from `dir`. A missing file is `Ok(None)`.
pub fn load_file(
    dir: &Path,
    key: &str,
    limits: &ParseLimits,
) -> Result<Option<DataValue>, PluginDataError> {
    let path = file_path(dir, key);
    let meta = match std::fs::metadata(&path) {
        Ok(m) => m,
        Err(e) if e.kind() == std::io::ErrorKind::NotFound => return Ok(None),
        Err(source) => return Err(PluginDataError::Io { path, source }),
    };
    if meta.len() > limits.max_bytes {
        return Err(PluginDataError::TooLarge(meta.len(), limits.max_bytes));
    }
    let bytes = std::fs::read(&path).map_err(|source| PluginDataError::Io {
        path: path.clone(),
        source,
    })?;
    parse(&bytes, limits).map(Some)
}

#[cfg(test)]
mod tests {
    use super::*;

    fn s(v: &str) -> DataValue {
        DataValue::String(v.as_bytes().to_vec())
    }

    fn table(entries: Vec<(DataKey, DataValue)>) -> DataValue {
        DataValue::Table(entries.into_iter().collect())
    }

    #[test]
    fn keys() {
        assert!(validate_key(b"").is_err());
        assert!(validate_key(&[b'a'; 64]).is_err());
        assert!(validate_key(&[b'a'; 63]).is_ok());
        assert!(validate_key(b"a").is_ok());
        for bad in [
            &b"a/b"[..],
            b"a:b",
            b"a\x01",
            b"..",
            b".",
            b"a.",
            b"a ",
            b"a|b",
        ] {
            assert!(validate_key(bad).is_err(), "{bad:?}");
        }
        assert!(validate_key(b"My Settings v2").is_ok());
    }

    #[test]
    fn numbers() {
        assert_eq!(format_number(0.0), "0");
        assert_eq!(format_number(-0.0), "0");
        assert_eq!(format_number(42.0), "42");
        assert_eq!(format_number(-7.0), "-7");
        assert_eq!(format_number(0.5), "0.5");
        assert_eq!(format_number(0.1), "0.10000000000000001");
        assert_eq!(format_number(1e-7), "9.9999999999999995e-08");
        assert_eq!(format_number(1e300), "1.0000000000000001e+300");
        for v in [
            0.1,
            1.0 / 3.0,
            -2.5e-5,
            123456.789,
            1e300,
            5e-324,
            f64::MAX,
            0.0001,
        ] {
            assert_eq!(format_number(v).parse::<f64>().unwrap(), v, "{v}");
        }
    }

    #[test]
    fn layout() {
        let v = table(vec![
            (DataKey::String(b"b".to_vec()), table(vec![])),
            (DataKey::String(b"a".to_vec()), DataValue::Number(1.0)),
            (DataKey::Number(2.0), DataValue::Boolean(true)),
            (DataKey::Number(1.0), s("x\"y")),
        ]);
        let out = String::from_utf8(serialize(&v)).unwrap();
        assert_eq!(
            out,
            "return\r\n{\r\n\t[1] = \"x\\\"y\",\r\n\t[2] = true,\r\n\t[\"a\"] = 1,\r\n\t[\"b\"] =\r\n\t{\r\n\r\n\t}\r\n}\r\n"
        );
        assert_eq!(parse(out.as_bytes(), &ParseLimits::default()).unwrap(), v);
    }

    #[test]
    fn reader_accepts_variants() {
        let src = b"-- comment\nreturn {\n  a = 1; ['b'] = [[\nlong]], [3] = -0x10,\n  \"pos\", nil, 'two', [ 1,5 ] = 9.000000, [ [==[k]==] ] = 'v\\65\\\n',\n}";
        let v = parse(src, &ParseLimits::default()).unwrap();
        let DataValue::Table(t) = v else { panic!() };
        assert_eq!(t[&DataKey::String(b"a".to_vec())], DataValue::Number(1.0));
        assert_eq!(t[&DataKey::String(b"b".to_vec())], s("long"));
        assert_eq!(t[&DataKey::Number(3.0)], s("two"));
        assert_eq!(t[&DataKey::Number(1.0)], s("pos"));
        assert_eq!(t[&DataKey::Number(1.5)], DataValue::Number(9.0));
        assert_eq!(t[&DataKey::String(b"k".to_vec())], s("vA\n"));
        assert!(!t.contains_key(&DataKey::Number(2.0)));

        // Positional items are stored after the keyed fields of their batch, as Lua 5.1
        // does, so they win over an explicit `[n]` whichever comes first.
        let limits = ParseLimits::default();
        let one = |src: &str| match parse(src.as_bytes(), &limits).unwrap() {
            DataValue::Table(t) => t.get(&DataKey::Number(1.0)).cloned(),
            other => panic!("{other:?}"),
        };
        assert_eq!(one(r#"return { "b", [1] = "a" }"#), Some(s("b")));
        assert_eq!(one(r#"return { [1] = "a", "b" }"#), Some(s("b")));
        assert_eq!(one(r#"return { nil, [1] = "a" }"#), None);
        // A full batch of 50 is stored when the next field starts, so a later key wins.
        let mut src = String::from("return { ");
        for i in 0..50 {
            src.push_str(&format!("{i}, "));
        }
        src.push_str("[1] = \"late\" }");
        assert_eq!(one(&src), Some(s("late")));
    }

    #[test]
    fn reader_rejects_code_and_garbage() {
        let limits = ParseLimits::default();
        for bad in [
            &b"return os.execute('x')"[..],
            b"return {f()}",
            b"return {a = b}",
            b"return {1 2}",
            b"return {",
            b"return \"abc",
            b"return {[nil] = 1}",
            b"return inf",
            b"return 1 2",
            b"return {[{}] = 1}",
            b"return \"\\999\"",
        ] {
            assert!(
                parse(bad, &limits).is_err(),
                "{:?}",
                String::from_utf8_lossy(bad)
            );
        }
        let deep = format!("return {}{}", "{".repeat(100), "}".repeat(100));
        assert!(matches!(
            parse(deep.as_bytes(), &limits),
            Err(PluginDataError::TooDeep(64))
        ));
    }

    #[test]
    fn hostile_bytes_never_panic() {
        // Deterministic xorshift noise plus mutations of a valid file.
        let base = serialize(&table(vec![
            (DataKey::String(b"k".to_vec()), s("v")),
            (
                DataKey::Number(1.0),
                table(vec![(DataKey::Number(1.0), DataValue::Number(0.25))]),
            ),
        ]));
        let mut x: u64 = 0x9E37_79B9_7F4A_7C15;
        let mut next = || {
            x ^= x << 13;
            x ^= x >> 7;
            x ^= x << 17;
            x
        };
        let limits = ParseLimits::default();
        for _ in 0..2000 {
            let mut buf = base.clone();
            let n = (next() % 8) as usize;
            for _ in 0..n {
                let pos = (next() as usize) % buf.len();
                buf[pos] = next() as u8;
            }
            let _ = parse(&buf, &limits);
            let len = (next() % 64) as usize;
            let noise: Vec<u8> = (0..len).map(|_| next() as u8).collect();
            let _ = parse(&noise, &limits);
        }
    }

    #[test]
    fn files() {
        let dir = tempfile::tempdir().unwrap();
        let sub = dir.path().join("a/b");
        assert!(
            load_file(&sub, "k", &ParseLimits::default())
                .unwrap()
                .is_none()
        );
        let v = table(vec![(
            DataKey::String(b"x".to_vec()),
            DataValue::Number(0.5),
        )]);
        save_file(&sub, "k", &v, &ParseLimits::default()).unwrap();
        assert_eq!(
            load_file(&sub, "k", &ParseLimits::default()).unwrap(),
            Some(v)
        );
        assert!(sub.join("k.plugindata").is_file());
        assert_eq!(std::fs::read_dir(&sub).unwrap().count(), 1);

        // Output larger than the read limit is refused before anything is written.
        let small = ParseLimits {
            max_bytes: 64,
            ..ParseLimits::default()
        };
        let big = s(&"x".repeat(100));
        assert!(matches!(
            save_file(&sub, "big", &big, &small),
            Err(PluginDataError::ValueTooLarge(64))
        ));
        assert!(!sub.join("big.plugindata").exists());
        assert_eq!(std::fs::read_dir(&sub).unwrap().count(), 1);
    }
}
