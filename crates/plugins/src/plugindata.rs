// SPDX-License-Identifier: GPL-3.0-or-later
//! `.plugindata` files: key rule, file naming, writer, the comma decimal separator on
//! reading, Lua-to-value conversion and atomic file I/O, over the core's neutral reader
//! ([`tessera_script::data`]).
//!
//! Spec: `lua-plugin-api.md` §3.4 (comma decimals), §10.2 (validation), §10.3 (file
//! name), §10.4 (format, Tessera's reading and writing), §10.5 (what can be saved), §10.6
//! (atomic writes); `plugin-descriptor.md` §11.1 (keys).

use std::collections::{BTreeMap, HashSet};
use std::io::Write as _;
use std::path::{Path, PathBuf};
use std::sync::atomic::{AtomicU64, Ordering};

use mlua::{Lua, Table, Value};
pub use tessera_script::data::{DataKey, DataValue, ParseLimits, ReadError};
use tessera_script::data::{NumeralHook, NumeralSite, parse_with};

/// Longest key accepted, in UTF-16 units (§10.2; keys are ASCII, so also bytes).
pub const MAX_KEY_LEN: usize = 64;

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
    /// A top-level value of an unsaveable type (§10.5).
    #[error("cannot save a value of type {0}")]
    Unsupported(&'static str),
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
    /// The file does not fit the grammar.
    #[error("{0}")]
    Read(#[from] ReadError),
    /// The conversion was stopped because the calling script's instruction budget is
    /// spent (the work of a conversion is charged to it, `lua-plugin-api.md` §3.3).
    #[error("{}", tessera_script::BUDGET_MESSAGE)]
    Budget,
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

/// Windows device names a key could spell (`plugin-descriptor.md` §11.1 OPEN): refused on
/// a Windows host.
const DEVICE_NAMES: [&str; 22] = [
    "CON", "PRN", "AUX", "NUL", "COM1", "COM2", "COM3", "COM4", "COM5", "COM6", "COM7", "COM8",
    "COM9", "LPT1", "LPT2", "LPT3", "LPT4", "LPT5", "LPT6", "LPT7", "LPT8", "LPT9",
];

/// Validate a key per the Tessera rule of §10.2: 1–64 units, each an ASCII letter, ASCII
/// digit or `_`. On a Windows host a device name is refused as well.
///
/// # Errors
///
/// [`PluginDataError::InvalidKey`].
pub fn validate_key(key: &[u8]) -> Result<&str, PluginDataError> {
    if key.is_empty() {
        return Err(PluginDataError::InvalidKey("empty"));
    }
    if key.len() > MAX_KEY_LEN {
        return Err(PluginDataError::InvalidKey("longer than 64 characters"));
    }
    if !key.iter().all(|&b| b.is_ascii_alphanumeric() || b == b'_') {
        return Err(PluginDataError::InvalidKey(
            "only ASCII letters, digits and '_' are allowed",
        ));
    }
    let s = std::str::from_utf8(key).map_err(|_| PluginDataError::InvalidKey("not ASCII"))?;
    if cfg!(windows) && DEVICE_NAMES.iter().any(|d| d.eq_ignore_ascii_case(s)) {
        return Err(PluginDataError::InvalidKey("a device name"));
    }
    Ok(s)
}

// ---------------------------------------------------------------------------------------
// Writer
// ---------------------------------------------------------------------------------------

/// Serialize `value` in the `.plugindata` layout of §10.4 as Tessera writes it: `return `,
/// tables on their own lines with tab indentation and CR LF line ends, `[key] = value`
/// entries separated by `,`, keys sorted (numbers ascending, then strings bytewise),
/// integral numbers as integers and others with 17 significant digits, `.` as decimal
/// separator, strings in double quotes with `\n`, `\r`, `\"` and `\\` escaped, and no line
/// end after the value.
#[must_use]
pub fn serialize(value: &DataValue) -> Vec<u8> {
    let mut out = b"return ".to_vec();
    write_value(&mut out, value, 0);
    out
}

fn indent(out: &mut Vec<u8>, level: usize) {
    out.extend(std::iter::repeat_n(b'\t', level));
}

fn write_value(out: &mut Vec<u8>, v: &DataValue, depth: usize) {
    match v {
        DataValue::Nil => out.extend_from_slice(b"nil"),
        DataValue::Boolean(true) => out.extend_from_slice(b"true"),
        DataValue::Boolean(false) => out.extend_from_slice(b"false"),
        DataValue::Number(n) if n.is_finite() => {
            out.extend_from_slice(format_number(*n).as_bytes())
        }
        // Non-finite values read back as nil in the original; write that directly so the
        // file stays readable.
        DataValue::Number(_) => out.extend_from_slice(b"nil"),
        DataValue::String(s) => write_string(out, s),
        DataValue::Table(t) => write_table(out, t, depth),
    }
}

/// `⏎`, d tabs, `{⏎`, the entries at depth d + 1 separated by `,⏎`, `⏎`, d tabs, `}`.
fn write_table(out: &mut Vec<u8>, t: &BTreeMap<DataKey, DataValue>, depth: usize) {
    out.extend_from_slice(b"\r\n");
    indent(out, depth);
    out.extend_from_slice(b"{\r\n");
    let n = t.len();
    for (i, (k, v)) in t.iter().enumerate() {
        indent(out, depth + 1);
        out.push(b'[');
        match k {
            DataKey::Number(n) => out.extend_from_slice(format_number(*n).as_bytes()),
            DataKey::String(s) => write_string(out, s),
        }
        out.extend_from_slice(b"] = ");
        write_value(out, v, depth + 1);
        if i + 1 < n {
            out.push(b',');
            out.extend_from_slice(b"\r\n");
        }
    }
    out.extend_from_slice(b"\r\n");
    indent(out, depth);
    out.push(b'}');
}

fn write_string(out: &mut Vec<u8>, s: &[u8]) {
    out.push(b'"');
    for &b in s {
        match b {
            b'"' => out.extend_from_slice(b"\\\""),
            b'\\' => out.extend_from_slice(b"\\\\"),
            b'\n' => out.extend_from_slice(b"\\n"),
            b'\r' => out.extend_from_slice(b"\\r"),
            _ => out.push(b),
        }
    }
    out.push(b'"');
}

/// Locale-independent number text (§3.4, §10.4): integral values as integers, others with
/// 17 significant digits (`%.17g` style), always `.` as the decimal separator.
#[must_use]
pub fn format_number(n: f64) -> String {
    if !n.is_finite() {
        return "nil".to_owned();
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
            exp.unsigned_abs()
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

/// The comma decimal separator of files written under a comma locale (§3.4, §10.4): inside
/// `[…]` and after `=`, a numeral of digits only that is followed by `,` and digits reads
/// as a decimal fraction, when the next token after the digits ends the field (`,`, `;`,
/// `}`, `]` or the end of input).
#[derive(Debug, Clone, Copy, Default)]
pub struct CommaDecimals;

impl NumeralHook for CommaDecimals {
    fn extend(
        &self,
        site: NumeralSite,
        text: &[u8],
        value: f64,
        rest: &[u8],
    ) -> Option<(f64, usize)> {
        if !matches!(site, NumeralSite::Key | NumeralSite::Value)
            || !text.iter().all(u8::is_ascii_digit)
            || rest.first() != Some(&b',')
        {
            return None;
        }
        let frac_len = rest[1..].iter().take_while(|b| b.is_ascii_digit()).count();
        if frac_len == 0 {
            return None;
        }
        let after = rest[1 + frac_len..]
            .iter()
            .find(|b| !b.is_ascii_whitespace())
            .copied();
        if !matches!(after, None | Some(b',' | b';' | b'}' | b']')) {
            return None;
        }
        let whole = format!(
            "{}.{}",
            std::str::from_utf8(text).ok()?,
            std::str::from_utf8(&rest[1..=frac_len]).ok()?
        );
        let v: f64 = whole.parse().ok()?;
        let _ = value;
        Some((v, 1 + frac_len))
    }
}

/// Parse a `.plugindata` file with the restricted grammar of §10.4 (both decimal
/// separators). No code is executed.
///
/// # Errors
///
/// [`ReadError`] for files over the limits or outside the grammar.
pub fn parse(bytes: &[u8], limits: &ParseLimits) -> Result<DataValue, ReadError> {
    parse_with(bytes, limits, &CommaDecimals)
}

// ---------------------------------------------------------------------------------------
// Lua values
// ---------------------------------------------------------------------------------------

/// Convert a Lua value to a saveable [`DataValue`] (§10.5): booleans, numbers, strings,
/// nil and tables of those. A top-level function, userdata or thread fails; inside a
/// table, an entry whose key or value is one of those is skipped; a table met a second
/// time in the same conversion (a cycle or a shared table) becomes nil; metatables are
/// ignored (the entries are read raw). Limits: depth, entry count and a running lower
/// bound of the serialized size.
///
/// # Errors
///
/// [`PluginDataError::Unsupported`] for an unsaveable top-level value, and the limit
/// errors.
pub fn lua_to_data(v: &Value, limits: &ParseLimits) -> Result<DataValue, PluginDataError> {
    convert_top(v, limits, None)
}

/// Budget units charged per table entry visited by [`lua_to_data_metered`] (skipped
/// entries included).
pub const ENTRY_COST: u64 = 32;

/// Entries visited between two budget charges.
const CHARGE_BATCH: u64 = 64;

/// [`lua_to_data`], charging every table entry visited (also the skipped ones) to
/// `budget` ([`ENTRY_COST`] units each).
///
/// # Errors
///
/// As [`lua_to_data`], and [`PluginDataError::Budget`] once the budget is spent.
pub fn lua_to_data_metered(
    v: &Value,
    limits: &ParseLimits,
    budget: &tessera_script::Budget,
) -> Result<DataValue, PluginDataError> {
    convert_top(v, limits, Some(budget))
}

fn convert_top(
    v: &Value,
    limits: &ParseLimits,
    budget: Option<&tessera_script::Budget>,
) -> Result<DataValue, PluginDataError> {
    if let Some(t) = unsaveable(v) {
        return Err(PluginDataError::Unsupported(t));
    }
    let mut walk = Walk {
        limits,
        seen: HashSet::new(),
        depth: 0,
        count: 0,
        bytes: 0,
        budget,
        visited: 0,
    };
    let out = walk.convert(v)?;
    walk.charge_rest()?;
    Ok(out)
}

fn unsaveable(v: &Value) -> Option<&'static str> {
    match v {
        Value::Function(_) => Some("function"),
        Value::UserData(_) | Value::LightUserData(_) => Some("userdata"),
        Value::Thread(_) => Some("thread"),
        Value::Error(_) => Some("userdata"),
        _ => None,
    }
}

struct Walk<'a> {
    limits: &'a ParseLimits,
    seen: HashSet<usize>,
    depth: usize,
    count: usize,
    bytes: u64,
    budget: Option<&'a tessera_script::Budget>,
    /// Entries visited since the last charge.
    visited: u64,
}

impl Walk<'_> {
    fn visit(&mut self) -> Result<(), PluginDataError> {
        self.visited += 1;
        if self.visited >= CHARGE_BATCH {
            self.charge_rest()?;
        }
        Ok(())
    }

    fn charge_rest(&mut self) -> Result<(), PluginDataError> {
        let n = std::mem::take(&mut self.visited);
        match self.budget {
            Some(b) if n > 0 => b
                .charge(n * ENTRY_COST)
                .map_err(|_| PluginDataError::Budget),
            _ => Ok(()),
        }
    }

    fn add_bytes(&mut self, n: u64) -> Result<(), PluginDataError> {
        self.bytes = self.bytes.saturating_add(n);
        if self.bytes > self.limits.max_bytes {
            Err(PluginDataError::ValueTooLarge(self.limits.max_bytes))
        } else {
            Ok(())
        }
    }

    fn convert(&mut self, v: &Value) -> Result<DataValue, PluginDataError> {
        Ok(match v {
            Value::Nil => DataValue::Nil,
            Value::Boolean(b) => DataValue::Boolean(*b),
            Value::Integer(i) => DataValue::Number(*i as f64),
            Value::Number(n) => DataValue::Number(*n),
            Value::String(s) => {
                let bytes = s.as_bytes();
                self.add_bytes(bytes.len() as u64 + 2)?;
                DataValue::String(bytes.to_vec())
            }
            Value::Table(t) => self.table(t)?,
            _ => DataValue::Nil,
        })
    }

    fn table(&mut self, t: &Table) -> Result<DataValue, PluginDataError> {
        let ptr = t.to_pointer() as usize;
        if !self.seen.insert(ptr) {
            return Ok(DataValue::Nil);
        }
        if self.depth >= self.limits.max_depth {
            return Err(PluginDataError::TooDeep(self.limits.max_depth));
        }
        self.depth += 1;
        self.add_bytes(6 + 2 * self.depth as u64)?;
        let mut map = BTreeMap::new();
        for pair in t.pairs::<Value, Value>() {
            self.visit()?;
            let Ok((k, val)) = pair else { continue };
            if unsaveable(&k).is_some() || unsaveable(&val).is_some() {
                continue;
            }
            self.count += 1;
            if self.count > self.limits.max_entries {
                return Err(PluginDataError::TooManyEntries(self.limits.max_entries));
            }
            self.add_bytes(self.depth as u64 + 8)?;
            let key = match &k {
                Value::Integer(i) => DataKey::number(*i as f64),
                Value::Number(n) => DataKey::number(*n),
                Value::String(s) => {
                    let bytes = s.as_bytes();
                    self.add_bytes(bytes.len() as u64 + 2)?;
                    Ok(DataKey::String(bytes.to_vec()))
                }
                Value::Boolean(b) => {
                    // A boolean key is written as `[true]`, which reads back as that key in
                    // the original; Tessera's value model keeps number and string keys
                    // only, so such an entry is skipped.
                    let _ = b;
                    continue;
                }
                _ => continue,
            };
            let Ok(key) = key else {
                // A non-finite number key would make the whole file unreadable.
                continue;
            };
            let dv = self.convert(&val)?;
            if dv != DataValue::Nil {
                map.insert(key, dv);
            }
        }
        self.depth -= 1;
        Ok(DataValue::Table(map))
    }
}

/// Build a fresh Lua value from a [`DataValue`].
///
/// # Errors
///
/// A Lua memory error.
pub fn data_to_lua(lua: &Lua, v: &DataValue) -> mlua::Result<Value> {
    tessera_script::data::data_to_lua(lua, v)
}

// ---------------------------------------------------------------------------------------
// Files
// ---------------------------------------------------------------------------------------

/// Path of the file holding `key` in `dir`.
#[must_use]
pub fn file_path(dir: &Path, key: &str) -> PathBuf {
    dir.join(format!("{key}.{EXTENSION}"))
}

/// Write `bytes` (a serialized value) to `path` atomically (temporary file in the same
/// folder, then rename; §10.6), creating the folders.
///
/// # Errors
///
/// [`PluginDataError::Io`].
pub fn write_atomic(path: &Path, bytes: &[u8]) -> Result<(), PluginDataError> {
    static COUNTER: AtomicU64 = AtomicU64::new(0);
    let io = |p: &Path| {
        let p = p.to_path_buf();
        move |source| PluginDataError::Io { path: p, source }
    };
    let dir = path.parent().unwrap_or_else(|| Path::new("."));
    std::fs::create_dir_all(dir).map_err(io(dir))?;
    let stem = path
        .file_name()
        .map(|n| n.to_string_lossy().into_owned())
        .unwrap_or_default();
    let tmp = dir.join(format!(
        ".{stem}.{}.{}.tmp",
        std::process::id(),
        COUNTER.fetch_add(1, Ordering::Relaxed)
    ));
    let result = (|| {
        let mut f = std::fs::File::create(&tmp).map_err(io(&tmp))?;
        f.write_all(bytes).map_err(io(&tmp))?;
        f.sync_all().map_err(io(&tmp))?;
        drop(f);
        std::fs::rename(&tmp, path).map_err(io(path))
    })();
    if result.is_err() {
        let _ = std::fs::remove_file(&tmp);
    }
    result
}

/// Serialize and write `value` under `key` in `dir` atomically. Refuses data that would
/// serialize to more than `limits.max_bytes`, so everything written can be read back.
///
/// # Errors
///
/// [`PluginDataError::ValueTooLarge`] or [`PluginDataError::Io`].
pub fn save_file(
    dir: &Path,
    key: &str,
    value: &DataValue,
    limits: &ParseLimits,
) -> Result<(), PluginDataError> {
    let bytes = serialize(value);
    if bytes.len() as u64 > limits.max_bytes {
        return Err(PluginDataError::ValueTooLarge(limits.max_bytes));
    }
    write_atomic(&file_path(dir, key), &bytes)
}

/// Read and parse the file at `path`. A missing file is `Ok(None)`.
///
/// # Errors
///
/// [`PluginDataError::TooLarge`], [`PluginDataError::Read`] or [`PluginDataError::Io`].
pub fn load_path(path: &Path, limits: &ParseLimits) -> Result<Option<DataValue>, PluginDataError> {
    match read_path(path, limits)? {
        Some(bytes) => Ok(Some(parse(&bytes, limits)?)),
        None => Ok(None),
    }
}

/// Read the bytes of the file at `path`, refusing one larger than `limits.max_bytes`. A
/// missing file is `Ok(None)`.
///
/// # Errors
///
/// [`PluginDataError::TooLarge`] or [`PluginDataError::Io`].
pub fn read_path(path: &Path, limits: &ParseLimits) -> Result<Option<Vec<u8>>, PluginDataError> {
    let meta = match std::fs::metadata(path) {
        Ok(m) => m,
        Err(e) if e.kind() == std::io::ErrorKind::NotFound => return Ok(None),
        Err(source) => {
            return Err(PluginDataError::Io {
                path: path.to_path_buf(),
                source,
            });
        }
    };
    if meta.len() > limits.max_bytes {
        return Err(PluginDataError::TooLarge(meta.len(), limits.max_bytes));
    }
    let mut bytes = Vec::new();
    std::fs::File::open(path)
        .and_then(|f| {
            use std::io::Read as _;
            f.take(limits.max_bytes.saturating_add(1))
                .read_to_end(&mut bytes)
        })
        .map_err(|source| PluginDataError::Io {
            path: path.to_path_buf(),
            source,
        })?;
    if bytes.len() as u64 > limits.max_bytes {
        return Err(PluginDataError::TooLarge(
            bytes.len() as u64,
            limits.max_bytes,
        ));
    }
    Ok(Some(bytes))
}

/// Read and parse `key` from `dir`. A missing file is `Ok(None)`.
///
/// # Errors
///
/// As [`load_path`].
pub fn load_file(
    dir: &Path,
    key: &str,
    limits: &ParseLimits,
) -> Result<Option<DataValue>, PluginDataError> {
    load_path(&file_path(dir, key), limits)
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
        assert!(validate_key(&[b'a'; 65]).is_err());
        assert!(validate_key(&[b'a'; 64]).is_ok());
        assert!(validate_key(b"a_1").is_ok());
        for bad in [
            &b"a/b"[..],
            b"a:b",
            b"a\x01",
            b"..",
            b".",
            b"a.",
            b"a ",
            b"a|b",
            b"\xc3\xa9",
        ] {
            assert!(validate_key(bad).is_err(), "{bad:?}");
        }
        assert!(validate_key(b"My Settings").is_err());
    }

    #[test]
    fn numbers() {
        assert_eq!(format_number(0.0), "0");
        assert_eq!(format_number(-0.0), "0");
        assert_eq!(format_number(42.0), "42");
        assert_eq!(format_number(0.5), "0.5");
        assert_eq!(format_number(0.1), "0.10000000000000001");
        assert_eq!(format_number(1e-7), "9.9999999999999995e-08");
        for v in [
            0.1,
            1.0 / 3.0,
            -2.5e-5,
            123_456.789,
            1e300,
            5e-324,
            f64::MAX,
            0.0001,
        ] {
            assert_eq!(format_number(v).parse::<f64>().unwrap(), v, "{v}");
        }
    }

    #[test]
    fn byte_exact_examples() {
        assert_eq!(serialize(&DataValue::Number(5.0)), b"return 5");
        assert_eq!(serialize(&s("a\"b")), b"return \"a\\\"b\"");
        assert_eq!(serialize(&DataValue::Boolean(true)), b"return true");
        assert_eq!(serialize(&table(vec![])), b"return \r\n{\r\n\r\n}");
        let v = table(vec![
            (DataKey::String(b"a".to_vec()), DataValue::Boolean(true)),
            (DataKey::Number(1.0), s("x\"y")),
        ]);
        assert_eq!(
            String::from_utf8(serialize(&v)).unwrap(),
            "return \r\n{\r\n\t[1] = \"x\\\"y\",\r\n\t[\"a\"] = true\r\n}"
        );
        let nested = table(vec![(DataKey::String(b"b".to_vec()), table(vec![]))]);
        let text = serialize(&nested);
        assert_eq!(
            String::from_utf8(text.clone()).unwrap(),
            "return \r\n{\r\n\t[\"b\"] = \r\n\t{\r\n\r\n\t}\r\n}"
        );
        assert_eq!(parse(&text, &ParseLimits::default()).unwrap(), nested);
    }

    #[test]
    fn comma_files_read_back() {
        let src = b"return \r\n{\r\n\t[1,000000] = 1,500000,\r\n\t[\"a\"] = 2,250000\r\n}";
        let v = parse(src, &ParseLimits::default()).unwrap();
        let DataValue::Table(t) = v else { panic!() };
        assert_eq!(t[&DataKey::Number(1.0)], DataValue::Number(1.5));
        assert_eq!(t[&DataKey::String(b"a".to_vec())], DataValue::Number(2.25));
        // A positional list keeps its commas as separators.
        let DataValue::Table(t) = parse(b"return {1,5}", &ParseLimits::default()).unwrap() else {
            panic!()
        };
        assert_eq!(t.len(), 2);
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
        let small = ParseLimits {
            max_bytes: 64,
            ..ParseLimits::default()
        };
        assert!(matches!(
            save_file(&sub, "big", &s(&"x".repeat(100)), &small),
            Err(PluginDataError::ValueTooLarge(64))
        ));
        assert!(!sub.join("big.plugindata").exists());
    }

    #[test]
    fn lua_values() {
        let lua = Lua::new();
        let v: Value = lua
            .load(
                "local shared = { 1 }
                 local t = { a = shared, b = shared, f = print, [print] = 1, n = 0/0 }
                 t.self = t
                 return t",
            )
            .eval()
            .unwrap();
        let DataValue::Table(t) = lua_to_data(&v, &ParseLimits::default()).unwrap() else {
            panic!()
        };
        let key = |k: &str| DataKey::String(k.as_bytes().to_vec());
        // `a` and `b` share one table: one of them is written, the other becomes nil.
        assert_eq!(
            usize::from(t.contains_key(&key("a"))) + usize::from(t.contains_key(&key("b"))),
            1
        );
        assert!(!t.contains_key(&key("f")) && !t.contains_key(&key("self")));
        assert!(matches!(t[&key("n")], DataValue::Number(n) if n.is_nan()));
        let f: Value = lua.load("return print").eval().unwrap();
        assert!(matches!(
            lua_to_data(&f, &ParseLimits::default()),
            Err(PluginDataError::Unsupported("function"))
        ));
    }
}
