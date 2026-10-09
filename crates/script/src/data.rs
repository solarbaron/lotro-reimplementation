// SPDX-License-Identifier: GPL-3.0-or-later
//! The neutral, no-execution reader of a Lua table constructor of plain values.
//!
//! Grammar (an optional leading `return`, one value, an optional `;`):
//!
//! | Element | Allowed |
//! |---|---|
//! | value | `nil`, `true`, `false`, number, string, table |
//! | number | Lua 5.1 numeral (decimal, exponent, hex `0x…`), optional leading `-` (repeatable) |
//! | string | double- or single-quoted with Lua 5.1 escapes; long brackets `[[…]]`, `[=[…]=]` |
//! | table | `{` fields `}`; field = `[` key `] =` value, `name =` value, or a positional value; separators `,` or `;`, trailing separator allowed |
//! | key | number or string (not nil, not NaN, not a table) |
//!
//! Comments (`--`, `--[[ ]]`) and any whitespace are allowed between tokens. Positional
//! items are stored as Lua 5.1 stores them (in batches of 50 after the keyed fields of
//! their batch), so the result equals what running the text would give. Nothing is ever
//! executed. A [`NumeralHook`] may extend numerals (a host's own numeral rules); the core
//! knows none.
//!
//! The reader is bounded by [`ParseLimits`]: input size, nesting depth and total entries.
//! It never recurses deeper than `max_depth` and never panics.
//!
//! Spec: `lua-plugin-api.md` §10.4 ("Reading (Tessera)"), the grammar of a saved value;
//! decision 0023 (the reader is part of the core, the file naming and numeral rules of
//! plugin data are not).

use std::collections::BTreeMap;

use mlua::{Lua, Value};

/// Errors of the reader and of the value model.
#[derive(Debug, Clone, PartialEq, Eq, thiserror::Error)]
pub enum ReadError {
    /// NaN or an infinity where a finite number is required (a table key).
    #[error("NaN or infinite number")]
    NonFinite,
    /// Nesting deeper than the limit.
    #[error("nesting deeper than {0} levels")]
    TooDeep(usize),
    /// More entries than the limit.
    #[error("more than {0} entries")]
    TooManyEntries(usize),
    /// Input larger than the limit.
    #[error("input is {0} bytes, limit is {1}")]
    TooLarge(u64, u64),
    /// The input does not fit the grammar.
    #[error("parse error at byte {offset}: {reason}")]
    Parse {
        /// Byte offset of the error.
        offset: usize,
        /// What was wrong.
        reason: &'static str,
    },
}

/// A table key: a finite number (with `-0` folded to `0`) or a byte string.
///
/// Ordering is numbers ascending first, then strings bytewise.
#[derive(Debug, Clone, PartialEq)]
pub enum DataKey {
    /// Finite number key.
    Number(f64),
    /// Byte-string key.
    String(Vec<u8>),
}

impl DataKey {
    /// Build a number key, rejecting NaN and infinities and folding `-0` to `0`.
    ///
    /// # Errors
    ///
    /// [`ReadError::NonFinite`] for NaN and infinities.
    pub fn number(n: f64) -> Result<Self, ReadError> {
        if !n.is_finite() {
            return Err(ReadError::NonFinite);
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

/// A plain value: what the reader produces.
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

/// Bounds applied when reading (and, by hosts, when writing).
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct ParseLimits {
    /// Largest input accepted, in bytes.
    pub max_bytes: u64,
    /// Deepest table nesting accepted.
    pub max_depth: usize,
    /// Most table entries accepted in total.
    pub max_entries: usize,
}

impl Default for ParseLimits {
    fn default() -> Self {
        Self {
            max_bytes: 16 * 1024 * 1024,
            max_depth: 64,
            max_entries: 1_000_000,
        }
    }
}

/// Lua 5.1 stores pending positional constructor items in batches of this many: a batch
/// is written when the field after its last item starts, and the rest at the closing
/// brace. Keyed fields are written as they appear.
const POSITIONAL_BATCH: usize = 50;

/// Where a numeral was read.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum NumeralSite {
    /// The top-level value.
    Root,
    /// Inside `[` … `]` as a table key.
    Key,
    /// After `=` in a keyed field.
    Value,
    /// A positional table item.
    Positional,
}

/// Lets a host extend numerals with rules of its own.
pub trait NumeralHook {
    /// Called after a numeral was read at `site`: `text` is the numeral as written (no
    /// sign), `value` its value, `rest` the input that follows it. Return
    /// `Some((value, n))` to use `value` instead and consume `n` more bytes of `rest`, or
    /// `None` to keep the numeral as read.
    fn extend(
        &self,
        site: NumeralSite,
        text: &[u8],
        value: f64,
        rest: &[u8],
    ) -> Option<(f64, usize)>;
}

/// The hook that adds nothing.
#[derive(Debug, Clone, Copy, Default)]
pub struct NoHook;

impl NumeralHook for NoHook {
    fn extend(&self, _: NumeralSite, _: &[u8], _: f64, _: &[u8]) -> Option<(f64, usize)> {
        None
    }
}

/// Build a fresh Lua value from a [`DataValue`] (tables are new tables without
/// metatables).
///
/// # Errors
///
/// A Lua error when the state refuses an allocation.
pub fn data_to_lua(lua: &Lua, v: &DataValue) -> mlua::Result<Value> {
    Ok(match v {
        DataValue::Nil => Value::Nil,
        DataValue::Boolean(b) => Value::Boolean(*b),
        DataValue::Number(n) => Value::Number(*n),
        DataValue::String(s) => Value::String(lua.create_string(s)?),
        DataValue::Table(map) => {
            let t = lua.create_table()?;
            for (k, val) in map {
                let key = match k {
                    DataKey::Number(n) => Value::Number(*n),
                    DataKey::String(s) => Value::String(lua.create_string(s)?),
                };
                t.raw_set(key, data_to_lua(lua, val)?)?;
            }
            Value::Table(t)
        }
    })
}

/// Parse `bytes` as an optional `return` followed by one value of the grammar of the module
/// documentation. No code is executed.
///
/// # Errors
///
/// [`ReadError`] when the input is larger than `limits.max_bytes`, nests deeper than
/// `limits.max_depth`, has more than `limits.max_entries` table entries, or does not fit
/// the grammar.
pub fn parse(bytes: &[u8], limits: &ParseLimits) -> Result<DataValue, ReadError> {
    parse_with(bytes, limits, &NoHook)
}

/// [`parse`] with a [`NumeralHook`] that may extend numerals (for example a host's comma
/// decimal separator).
///
/// # Errors
///
/// As [`parse`].
pub fn parse_with(
    bytes: &[u8],
    limits: &ParseLimits,
    hook: &dyn NumeralHook,
) -> Result<DataValue, ReadError> {
    if bytes.len() as u64 > limits.max_bytes {
        return Err(ReadError::TooLarge(bytes.len() as u64, limits.max_bytes));
    }
    let bytes = bytes.strip_prefix(b"\xEF\xBB\xBF").unwrap_or(bytes);
    let mut p = Parser {
        s: bytes,
        i: 0,
        limits,
        entries: 0,
        hook,
    };
    p.ws()?;
    if p.peek_word() == Some(b"return".as_slice()) {
        p.i += 6;
        p.ws()?;
    }
    let v = p.value(0, NumeralSite::Root)?;
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
    hook: &'a dyn NumeralHook,
}

fn is_name_start(b: u8) -> bool {
    b.is_ascii_alphabetic() || b == b'_'
}

fn is_name_char(b: u8) -> bool {
    b.is_ascii_alphanumeric() || b == b'_'
}

impl<'a> Parser<'a> {
    fn err(&self, reason: &'static str) -> ReadError {
        ReadError::Parse {
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
    fn ws(&mut self) -> Result<(), ReadError> {
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

    fn value(&mut self, depth: usize, site: NumeralSite) -> Result<DataValue, ReadError> {
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
                        let n = self.hooked_numeral(site)?;
                        Ok(DataValue::Number(if neg { -n } else { n }))
                    }
                    _ => Err(self.err("'-' applied to a non-number")),
                }
            }
            Some(b) if b.is_ascii_digit() || b == b'.' => {
                Ok(DataValue::Number(self.hooked_numeral(site)?))
            }
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

    /// A numeral, then whatever the hook adds to it at `site`.
    fn hooked_numeral(&mut self, site: NumeralSite) -> Result<f64, ReadError> {
        let start = self.i;
        let v = self.numeral()?;
        let text = &self.s[start..self.i];
        let rest = &self.s[self.i..];
        match self.hook.extend(site, text, v, rest) {
            Some((n, used)) if used <= rest.len() => {
                if !n.is_finite() {
                    return Err(self.err("number out of range"));
                }
                self.i += used;
                Ok(n)
            }
            _ => Ok(v),
        }
    }

    /// A Lua 5.1 numeral (decimal with optional fraction/exponent, or `0x` hex).
    fn numeral(&mut self) -> Result<f64, ReadError> {
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

    fn long_string(&mut self, level: usize) -> Result<Vec<u8>, ReadError> {
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

    fn quoted(&mut self) -> Result<Vec<u8>, ReadError> {
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

    fn table(&mut self, depth: usize) -> Result<DataValue, ReadError> {
        if depth >= self.limits.max_depth {
            return Err(ReadError::TooDeep(self.limits.max_depth));
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
                return Err(ReadError::TooManyEntries(self.limits.max_entries));
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
    ) -> Result<(DataKey, DataValue, bool), ReadError> {
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
            let v = self.value(depth + 1, NumeralSite::Value)?;
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
                let v = self.value(depth + 1, NumeralSite::Value)?;
                return Ok((DataKey::String(word.to_vec()), v, false));
            }
            self.i = save;
            return Err(self.err("expected '=' after a name"));
        }
        // positional value
        let v = self.value(depth + 1, NumeralSite::Positional)?;
        let key = DataKey::Number(*next_index);
        *next_index += 1.0;
        Ok((key, v, true))
    }

    fn expect_assign(&mut self) -> Result<(), ReadError> {
        if self.peek() == Some(b'=') && self.peek_at(1) != Some(b'=') {
            self.i += 1;
            Ok(())
        } else {
            Err(self.err("expected '='"))
        }
    }

    /// Key inside `[...]`: a number (extended by the hook) or a string.
    fn bracket_key(&mut self) -> Result<DataKey, ReadError> {
        let neg = if self.peek() == Some(b'-') && self.peek_at(1) != Some(b'-') {
            self.i += 1;
            self.ws()?;
            true
        } else {
            false
        };
        match self.peek() {
            Some(b) if b.is_ascii_digit() || b == b'.' => {
                let v = self.hooked_numeral(NumeralSite::Key)?;
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

#[cfg(test)]
mod tests {
    use super::*;

    fn s(v: &str) -> DataValue {
        DataValue::String(v.as_bytes().to_vec())
    }

    #[test]
    fn reader_accepts_variants() {
        let src = b"-- comment\nreturn {\n  a = 1; ['b'] = [[\nlong]], [3] = -0x10,\n  \"pos\", nil, 'two', [ 1.5 ] = 9.000000, [ [==[k]==] ] = 'v\\65\\\n',\n}";
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
        let mut src = String::from("return { ");
        for i in 0..50 {
            src.push_str(&format!("{i}, "));
        }
        src.push_str("[1] = \"late\" }");
        assert_eq!(one(&src), Some(s("late")));
    }

    #[test]
    fn core_reader_has_no_comma_rule() {
        // `[1,5]` is not a key without a host hook.
        assert!(parse(b"return { [1,5] = 1 }", &ParseLimits::default()).is_err());
    }

    struct Comma;
    impl NumeralHook for Comma {
        fn extend(&self, site: NumeralSite, _: &[u8], v: f64, rest: &[u8]) -> Option<(f64, usize)> {
            if site != NumeralSite::Key || rest.first() != Some(&b',') {
                return None;
            }
            let n = rest[1..].iter().take_while(|b| b.is_ascii_digit()).count();
            let frac: f64 = std::str::from_utf8(&rest[1..=n]).ok()?.parse().ok()?;
            Some((v + frac / 10f64.powi(n as i32), n + 1))
        }
    }

    #[test]
    fn hooks_extend_numerals() {
        let v = parse_with(b"return { [1,5] = 2 }", &ParseLimits::default(), &Comma).unwrap();
        let DataValue::Table(t) = v else { panic!() };
        assert_eq!(t[&DataKey::Number(1.5)], DataValue::Number(2.0));
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
            Err(ReadError::TooDeep(64))
        ));
        let small = ParseLimits {
            max_bytes: 4,
            ..ParseLimits::default()
        };
        assert!(matches!(
            parse(b"return 1", &small),
            Err(ReadError::TooLarge(8, 4))
        ));
        let few = ParseLimits {
            max_entries: 2,
            ..ParseLimits::default()
        };
        assert!(matches!(
            parse(b"{1,2,3}", &few),
            Err(ReadError::TooManyEntries(2))
        ));
    }

    #[test]
    fn hostile_bytes_never_panic() {
        let base = b"return {\r\n\t[\"k\"] = \"v\",\r\n\t[1] = { [1] = 0.25 }\r\n}".to_vec();
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
    fn values_reach_lua() {
        let lua = Lua::new();
        let v = parse(
            b"return { a = { 1, 'x' }, [2.5] = true }",
            &ParseLimits::default(),
        )
        .unwrap();
        let t = data_to_lua(&lua, &v).unwrap();
        lua.globals().set("t", t).unwrap();
        let ok: bool = lua
            .load("return t.a[1] == 1 and t.a[2] == 'x' and t[2.5] == true")
            .eval()
            .unwrap();
        assert!(ok);
    }
}
