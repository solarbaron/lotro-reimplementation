// SPDX-License-Identifier: GPL-3.0-or-later
//! Number and string conversions and the random generator of the original client's C
//! runtime, for states whose profile asks for them.
//!
//! The reference interpreter takes these behaviours from the C library it is built
//! against; a Linux build differs from the original's Windows runtime. With
//! [`Conversions::Compatible`](crate::Conversions) a state replaces the affected library
//! functions so that plugins see the original's results on every operating system:
//!
//! - `tostring` (and `print`, which calls it): `%.14g` with an exponent of at least three
//!   digits (`1e+020`), the runtime's text for non-finite values (`1.#INF`, `-1.#INF`,
//!   `-1.#IND`, `1.#QNAN`), and `<type>: ` followed by 16 upper-case hexadecimal digits for
//!   tables, functions, userdata and threads;
//! - `tonumber`: C-locale decimal conversion without `inf`, `nan` or hexadecimal floating
//!   forms; hexadecimal text through a 32-bit unsigned conversion (saturating at
//!   4294967295, a leading minus wrapping modulo 2^32);
//! - `string.format`: `%d`, `%i`, `%c` on a 32-bit signed value (outside the range:
//!   −2147483648), `%o`, `%u`, `%x`, `%X` on the low 32 bits of the 64-bit truncation,
//!   three-digit exponents for `%e`, `%E`, `%g`, `%G`, and `%s`/`%q` of a number in the
//!   `tostring` form;
//! - `math.random` and `math.randomseed` over [`CrtRandom`], one generator that the host
//!   may share between states.
//!
//! Number-to-text conversions done inside the interpreter itself (the `..` operator,
//! `table.concat`, coercion of a number argument to a string, numerals in source text)
//! keep the platform's behaviour; see the OPEN note in `plugin-runtime.md` §4.4.
//!
//! Spec: `plugin-runtime.md` §4.3 (C locale), §4.4 (conversions), §4.5 (random numbers).

use std::sync::{Arc, Mutex, PoisonError};

use mlua::{Function, Lua, LuaString, MultiValue, Table, Value};

use crate::native::{host_error, native_fn};

/// The C runtime's linear congruential generator (`plugin-runtime.md` §4.5): state *s*
/// (32-bit, wrapping) becomes *s* · 214013 + 2531011, and each draw is bits 16–30 of the
/// new state (0–32767). The initial state is 1.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct CrtRandom {
    state: u32,
}

impl Default for CrtRandom {
    fn default() -> Self {
        Self::new()
    }
}

impl CrtRandom {
    /// A generator in its initial state (1).
    #[must_use]
    pub fn new() -> Self {
        Self { state: 1 }
    }

    /// Set the state (`srand`).
    pub fn seed(&mut self, seed: u32) {
        self.state = seed;
    }

    /// Next value, 0–32767 (`rand`).
    pub fn next_value(&mut self) -> u32 {
        self.state = self.state.wrapping_mul(214_013).wrapping_add(2_531_011);
        (self.state >> 16) & 0x7FFF
    }

    /// `math.random()`'s fraction: (*g* mod 32767) / 32767.
    pub fn fraction(&mut self) -> f64 {
        f64::from(self.next_value() % 32_767) / 32_767.0
    }
}

/// A generator shared by several states (all plugin apartments share one, §4.5).
pub type SharedRandom = Arc<Mutex<CrtRandom>>;

/// `%.14g` as the original's runtime formats it: up to 14 significant digits, an exponent
/// of at least three digits, and the runtime's text for non-finite values.
#[must_use]
pub fn format_number_g14(n: f64) -> String {
    if n.is_nan() {
        return if n.is_sign_negative() {
            "-1.#IND"
        } else {
            "1.#QNAN"
        }
        .to_owned();
    }
    if n.is_infinite() {
        return if n > 0.0 { "1.#INF" } else { "-1.#INF" }.to_owned();
    }
    if n == 0.0 {
        return if n.is_sign_negative() { "-0" } else { "0" }.to_owned();
    }
    let sci = format!("{n:.13e}");
    let Some((mant, exp)) = sci.split_once('e') else {
        return sci;
    };
    let Ok(exp) = exp.parse::<i32>() else {
        return sci;
    };
    if !(-4..14).contains(&exp) {
        let mut m = mant.to_owned();
        trim_fraction(&mut m);
        format!(
            "{m}e{}{:03}",
            if exp < 0 { '-' } else { '+' },
            exp.unsigned_abs()
        )
    } else {
        let decimals = usize::try_from(13 - exp).unwrap_or(0);
        let mut s = format!("{n:.decimals$}");
        trim_fraction(&mut s);
        s
    }
}

/// Remove trailing zeros of a fraction, and the point if nothing is left after it.
fn trim_fraction(s: &mut String) {
    if s.contains('.') {
        while s.ends_with('0') {
            s.pop();
        }
        if s.ends_with('.') {
            s.pop();
        }
    }
}

/// Pad the exponent of a C `%e`/`%g` result to at least three digits.
fn pad_exponent(s: &str) -> String {
    let b = s.as_bytes();
    let Some(e) = b.iter().rposition(|&c| c == b'e' || c == b'E') else {
        return s.to_owned();
    };
    let sign = e + 1;
    if !matches!(b.get(sign), Some(b'+' | b'-')) {
        return s.to_owned();
    }
    let digits = b[sign + 1..]
        .iter()
        .take_while(|c| c.is_ascii_digit())
        .count();
    if digits == 0 || digits >= 3 {
        return s.to_owned();
    }
    let mut out = String::with_capacity(s.len() + 3 - digits);
    out.push_str(&s[..=sign]);
    for _ in digits..3 {
        out.push('0');
    }
    out.push_str(&s[sign + 1..]);
    out
}

/// Apply a `%e`/`%g` field width after the exponent was padded (the original's runtime
/// pads to the three-digit form): `formatted` is the conversion done without width and
/// without the `-` and `0` flags; `flags` and `width` are those of the specification.
fn apply_width(formatted: &str, flags: &[u8], width: usize, finite: bool) -> String {
    let len = formatted.chars().count();
    if len >= width {
        return formatted.to_owned();
    }
    let fill = width - len;
    if flags.contains(&b'-') {
        let mut out = formatted.to_owned();
        out.extend(std::iter::repeat_n(' ', fill));
        out
    } else if flags.contains(&b'0') && finite {
        let sign = usize::from(formatted.starts_with(['+', '-', ' ']));
        let mut out = String::with_capacity(width);
        out.push_str(&formatted[..sign]);
        out.extend(std::iter::repeat_n('0', fill));
        out.push_str(&formatted[sign..]);
        out
    } else {
        let mut out: String = std::iter::repeat_n(' ', fill).collect();
        out.push_str(formatted);
        out
    }
}

/// The `%d` conversion: truncation toward zero to a 32-bit signed value; anything outside
/// the range (and NaN) becomes −2147483648.
#[must_use]
pub(crate) fn to_c_int(n: f64) -> i32 {
    let t = n.trunc();
    if t.is_nan() || t < f64::from(i32::MIN) || t > f64::from(i32::MAX) {
        i32::MIN
    } else {
        t as i32
    }
}

/// The `%x` conversion: truncation to a 64-bit signed value (beyond the range: 0), then
/// the low 32 bits.
#[must_use]
pub(crate) fn to_c_low32(n: f64) -> u32 {
    let t = n.trunc();
    // 2^63 as a double; the conversion of anything at or beyond it is the "indefinite"
    // value, whose low 32 bits are zero.
    if t.is_nan() || t >= 9_223_372_036_854_775_808.0 || t < -9_223_372_036_854_775_808.0 {
        0
    } else {
        (t as i64) as u32
    }
}

/// `luaL_checkint` on a 64-bit build: truncation to the 64-bit integer, then the low 32
/// bits as a signed `int`.
fn check_int_value(n: f64) -> i32 {
    (n.trunc() as i64) as i32
}

fn is_c_space(b: u8) -> bool {
    matches!(b, b' ' | b'\t' | b'\n' | 0x0B | 0x0C | b'\r')
}

/// The prefix of `s` that C-locale `strtod` converts, without infinities, NaN or
/// hexadecimal floating forms: `(value, bytes consumed)`, or `None` when nothing converts.
fn c_strtod(s: &[u8]) -> Option<(f64, usize)> {
    let mut i = 0;
    while s.get(i).copied().is_some_and(is_c_space) {
        i += 1;
    }
    let start = i;
    if matches!(s.get(i), Some(b'+' | b'-')) {
        i += 1;
    }
    let int_digits = s[i..].iter().take_while(|c| c.is_ascii_digit()).count();
    i += int_digits;
    let mut frac_digits = 0;
    if s.get(i) == Some(&b'.') {
        frac_digits = s[i + 1..].iter().take_while(|c| c.is_ascii_digit()).count();
        if int_digits + frac_digits > 0 {
            i += 1 + frac_digits;
        }
    }
    if int_digits + frac_digits == 0 {
        return None;
    }
    if matches!(s.get(i), Some(b'e' | b'E')) {
        let mut j = i + 1;
        if matches!(s.get(j), Some(b'+' | b'-')) {
            j += 1;
        }
        let exp_digits = s[j..].iter().take_while(|c| c.is_ascii_digit()).count();
        if exp_digits > 0 {
            i = j + exp_digits;
        }
    }
    let text = std::str::from_utf8(&s[start..i]).ok()?;
    text.parse::<f64>().ok().map(|v| (v, i))
}

/// A 32-bit `strtoul` of `s` in `base` (2–36; 16 accepts a `0x` prefix): leading space,
/// an optional sign, digits. Magnitudes above 4294967295 give 4294967295; a minus wraps
/// the magnitude modulo 2^32. `(value, bytes consumed)`, or `None` when no digit converts.
fn c_strtoul32(s: &[u8], base: u32) -> Option<(u32, usize)> {
    let mut i = 0;
    while s.get(i).copied().is_some_and(is_c_space) {
        i += 1;
    }
    let mut neg = false;
    if let Some(&c @ (b'+' | b'-')) = s.get(i) {
        neg = c == b'-';
        i += 1;
    }
    if base == 16
        && s.get(i) == Some(&b'0')
        && matches!(s.get(i + 1), Some(b'x' | b'X'))
        && s.get(i + 2)
            .and_then(|&c| char::from(c).to_digit(16))
            .is_some()
    {
        i += 2;
    }
    let digits_start = i;
    let mut mag: u64 = 0;
    let mut overflow = false;
    while let Some(d) = s.get(i).and_then(|&c| char::from(c).to_digit(base)) {
        if !overflow {
            mag = mag * u64::from(base) + u64::from(d);
            if mag > u64::from(u32::MAX) {
                overflow = true;
            }
        }
        i += 1;
    }
    if i == digits_start {
        return None;
    }
    let value = if overflow {
        u32::MAX
    } else if neg {
        (mag as u32).wrapping_neg()
    } else {
        mag as u32
    };
    Some((value, i))
}

/// Lua 5.1's `luaO_str2d` with the original runtime's conversions: the text up to its
/// first NUL, decimal first, hexadecimal when the decimal conversion stops at an `x`, and
/// only trailing space after the number.
#[must_use]
pub(crate) fn str_to_number(s: &[u8]) -> Option<f64> {
    let s = &s[..s.iter().position(|&b| b == 0).unwrap_or(s.len())];
    let (mut v, mut end) = c_strtod(s)?;
    if matches!(s.get(end), Some(b'x' | b'X')) {
        let (h, e) = c_strtoul32(s, 16)?;
        v = f64::from(h);
        end = e;
    }
    s[end..].iter().all(|&b| is_c_space(b)).then_some(v)
}

/// `tonumber(s, base)` for an explicit base: a 32-bit `strtoul`, then only trailing space.
fn str_to_number_base(s: &[u8], base: u32) -> Option<f64> {
    let s = &s[..s.iter().position(|&b| b == 0).unwrap_or(s.len())];
    let (v, end) = c_strtoul32(s, base)?;
    s[end..]
        .iter()
        .all(|&b| is_c_space(b))
        .then_some(f64::from(v))
}

/// Address part of the stock `tostring` of a reference value, rewritten as 16 upper-case
/// digits when it is the default form (`__tostring` results are left alone).
fn rewrite_address(text: &[u8], ptr: usize) -> Option<String> {
    let s = std::str::from_utf8(text).ok()?;
    let (kind, addr) = s.split_once(": ")?;
    if !matches!(kind, "table" | "function" | "userdata" | "thread") {
        return None;
    }
    let hex = addr
        .strip_prefix("0x")
        .or_else(|| addr.strip_prefix("0X"))
        .unwrap_or(addr);
    let parsed = usize::from_str_radix(hex, 16).ok()?;
    (parsed == ptr).then(|| format!("{kind}: {ptr:016X}"))
}

/// The replacement `tostring`.
fn tostring(lua: &Lua, stock: &Function, v: Value) -> mlua::Result<LuaString> {
    match v {
        Value::Number(n) => lua.create_string(format_number_g14(n)),
        Value::Integer(i) => lua.create_string(format_number_g14(i as f64)),
        Value::Table(_)
        | Value::Function(_)
        | Value::UserData(_)
        | Value::Thread(_)
        | Value::LightUserData(_) => {
            let ptr = v.to_pointer() as usize;
            let s: LuaString = stock.call(v)?;
            match rewrite_address(&s.as_bytes(), ptr) {
                Some(r) => lua.create_string(r),
                None => Ok(s),
            }
        }
        other => stock.call(other),
    }
}

fn type_name_at(args: &MultiValue, i: usize) -> &'static str {
    match args.get(i) {
        None => "no value",
        Some(v) => v.type_name(),
    }
}

fn arg_error(n: usize, fname: &str, msg: &str) -> mlua::Error {
    host_error(format!("bad argument #{n} to '{fname}' ({msg})"))
}

fn check_number(lua: &Lua, args: &MultiValue, i: usize, fname: &str) -> mlua::Result<f64> {
    let v = args.get(i).cloned().unwrap_or(Value::Nil);
    let n = match &v {
        Value::Number(n) => Some(*n),
        Value::Integer(n) => Some(*n as f64),
        Value::String(s) => str_to_number(&s.as_bytes()),
        _ => None,
    };
    let _ = lua;
    n.ok_or_else(|| {
        arg_error(
            i + 1,
            fname,
            &format!("number expected, got {}", type_name_at(args, i)),
        )
    })
}

/// The replacement `tonumber`.
fn tonumber(args: &MultiValue) -> mlua::Result<Value> {
    let base = match args.get(1) {
        None | Some(Value::Nil) => 10.0,
        Some(Value::Number(n)) => *n,
        Some(Value::Integer(n)) => *n as f64,
        Some(Value::String(s)) => str_to_number(&s.as_bytes())
            .ok_or_else(|| arg_error(2, "tonumber", "number expected, got string"))?,
        Some(other) => {
            return Err(arg_error(
                2,
                "tonumber",
                &format!("number expected, got {}", other.type_name()),
            ));
        }
    };
    let base = check_int_value(base);
    let Some(v) = args.front() else {
        return Err(arg_error(1, "tonumber", "value expected"));
    };
    if base == 10 {
        return Ok(match v {
            Value::Number(_) | Value::Integer(_) => v.clone(),
            Value::String(s) => str_to_number(&s.as_bytes()).map_or(Value::Nil, Value::Number),
            _ => Value::Nil,
        });
    }
    if !(2..=36).contains(&base) {
        return Err(arg_error(2, "tonumber", "base out of range"));
    }
    let text = match v {
        Value::String(s) => s.as_bytes().to_vec(),
        Value::Number(n) => format_number_g14(*n).into_bytes(),
        Value::Integer(n) => format_number_g14(*n as f64).into_bytes(),
        other => {
            return Err(arg_error(
                1,
                "tonumber",
                &format!("string expected, got {}", other.type_name()),
            ));
        }
    };
    Ok(str_to_number_base(&text, base.unsigned_abs()).map_or(Value::Nil, Value::Number))
}

/// The replacement `string.format`: each conversion is checked and adjusted here, then
/// formatted by the stock function one conversion at a time.
fn format(lua: &Lua, stock: &Function, args: &MultiValue) -> mlua::Result<LuaString> {
    let fmt = match args.front() {
        Some(Value::String(s)) => s.as_bytes().to_vec(),
        Some(Value::Number(n)) => format_number_g14(*n).into_bytes(),
        Some(Value::Integer(n)) => format_number_g14(*n as f64).into_bytes(),
        _ => {
            return Err(arg_error(
                1,
                "format",
                &format!("string expected, got {}", type_name_at(args, 0)),
            ));
        }
    };
    let fmt = &fmt[..fmt.iter().position(|&b| b == 0).unwrap_or(fmt.len())];
    let mut out: Vec<u8> = Vec::with_capacity(fmt.len());
    let mut arg = 0usize;
    let mut i = 0usize;
    let stock_one = |spec: &[u8], v: Value| -> mlua::Result<Vec<u8>> {
        let spec = lua.create_string(spec)?;
        match stock.call::<LuaString>((spec, v)) {
            Ok(s) => Ok(s.as_bytes().to_vec()),
            Err(e) => Err(host_error(crate::native::describe_error(&e).0)),
        }
    };
    while i < fmt.len() {
        let b = fmt[i];
        i += 1;
        if b != b'%' {
            out.push(b);
            continue;
        }
        if fmt.get(i) == Some(&b'%') {
            out.push(b'%');
            i += 1;
            continue;
        }
        let start = i - 1;
        let flags = fmt[i..].iter().take_while(|c| b"-+ #0".contains(c)).count();
        if flags > 5 {
            return Err(host_error("invalid format (repeated flags)"));
        }
        i += flags;
        for _ in 0..2 {
            if fmt.get(i).is_some_and(u8::is_ascii_digit) {
                i += 1;
            }
        }
        if fmt.get(i) == Some(&b'.') {
            i += 1;
            for _ in 0..2 {
                if fmt.get(i).is_some_and(u8::is_ascii_digit) {
                    i += 1;
                }
            }
        }
        if fmt.get(i).is_some_and(u8::is_ascii_digit) {
            return Err(host_error("invalid format (width or precision too long)"));
        }
        let Some(&conv) = fmt.get(i) else {
            return Err(host_error("invalid option '%' to 'format'"));
        };
        i += 1;
        let spec = &fmt[start..i];
        arg += 1;
        let piece = match conv {
            b'c' | b'd' | b'i' => {
                let n = check_number(lua, args, arg, "format")?;
                stock_one(spec, Value::Number(f64::from(to_c_int(n))))?
            }
            b'o' | b'u' | b'x' | b'X' => {
                let n = check_number(lua, args, arg, "format")?;
                stock_one(spec, Value::Number(f64::from(to_c_low32(n))))?
            }
            b'e' | b'E' | b'g' | b'G' => {
                let n = check_number(lua, args, arg, "format")?;
                // Format without the width (and its `-`/`0` flags), pad the exponent to
                // three digits, then apply the width to the padded text.
                let spec_flags = &fmt[start + 1..start + 1 + flags];
                let after_flags = start + 1 + flags;
                let width_len = fmt[after_flags..i - 1]
                    .iter()
                    .take_while(|c| c.is_ascii_digit())
                    .count();
                let width: usize = std::str::from_utf8(&fmt[after_flags..after_flags + width_len])
                    .ok()
                    .and_then(|w| w.parse().ok())
                    .unwrap_or(0);
                let mut bare = vec![b'%'];
                bare.extend(spec_flags.iter().filter(|f| !matches!(f, b'-' | b'0')));
                bare.extend_from_slice(&fmt[after_flags + width_len..i]);
                let s = stock_one(&bare, Value::Number(n))?;
                let padded = pad_exponent(&String::from_utf8_lossy(&s));
                apply_width(&padded, spec_flags, width, n.is_finite()).into_bytes()
            }
            b'f' => {
                let n = check_number(lua, args, arg, "format")?;
                stock_one(spec, Value::Number(n))?
            }
            b'q' | b's' => {
                let v = match args.get(arg) {
                    Some(Value::String(s)) => Value::String(s.clone()),
                    Some(Value::Number(n)) => {
                        Value::String(lua.create_string(format_number_g14(*n))?)
                    }
                    Some(Value::Integer(n)) => {
                        Value::String(lua.create_string(format_number_g14(*n as f64))?)
                    }
                    _ => {
                        return Err(arg_error(
                            arg + 1,
                            "format",
                            &format!("string expected, got {}", type_name_at(args, arg)),
                        ));
                    }
                };
                stock_one(spec, v)?
            }
            other => {
                return Err(host_error(format!(
                    "invalid option '%{}' to 'format'",
                    char::from(other)
                )));
            }
        };
        out.extend_from_slice(&piece);
    }
    lua.create_string(out)
}

/// The replacement `math.random`.
fn random(rng: &Mutex<CrtRandom>, lua: &Lua, args: &MultiValue) -> mlua::Result<f64> {
    let r = rng
        .lock()
        .unwrap_or_else(PoisonError::into_inner)
        .fraction();
    match args.len() {
        0 => Ok(r),
        1 => {
            let u = check_int_value(check_number(lua, args, 0, "random")?);
            if u < 1 {
                return Err(arg_error(1, "random", "interval is empty"));
            }
            Ok((r * f64::from(u)).floor() + 1.0)
        }
        2 => {
            let l = check_int_value(check_number(lua, args, 0, "random")?);
            let u = check_int_value(check_number(lua, args, 1, "random")?);
            if l > u {
                return Err(arg_error(2, "random", "interval is empty"));
            }
            let span = u.wrapping_sub(l).wrapping_add(1);
            Ok((r * f64::from(span)).floor() + f64::from(l))
        }
        _ => Err(host_error("wrong number of arguments")),
    }
}

/// Install the compatible `tostring`, `tonumber` and `string.format` into `globals`.
/// Returns the new `tostring` (for `print`).
pub(crate) fn install_conversions(lua: &Lua, globals: &Table) -> mlua::Result<Function> {
    let stock_tostring: Function = globals.raw_get("tostring")?;
    let ts = native_fn(lua, "tostring", move |lua, args: MultiValue| {
        let Some(v) = args.into_iter().next() else {
            return Err(arg_error(1, "tostring", "value expected"));
        };
        tostring(lua, &stock_tostring, v)
    })?;
    globals.raw_set("tostring", ts.clone())?;
    globals.raw_set(
        "tonumber",
        native_fn(lua, "tonumber", |_, args: MultiValue| tonumber(&args))?,
    )?;
    let string: Table = globals.raw_get("string")?;
    let stock_format: Function = string.raw_get("format")?;
    string.raw_set(
        "format",
        native_fn(lua, "format", move |lua, args: MultiValue| {
            format(lua, &stock_format, &args)
        })?,
    )?;
    Ok(ts)
}

/// Install `math.random` and `math.randomseed` over `rng`.
pub(crate) fn install_random(lua: &Lua, globals: &Table, rng: &SharedRandom) -> mlua::Result<()> {
    let math: Table = globals.raw_get("math")?;
    let r = Arc::clone(rng);
    math.raw_set(
        "random",
        native_fn(lua, "random", move |lua, args: MultiValue| {
            random(&r, lua, &args)
        })?,
    )?;
    let r = Arc::clone(rng);
    math.raw_set(
        "randomseed",
        native_fn(lua, "randomseed", move |lua, args: MultiValue| {
            let n = check_int_value(check_number(lua, &args, 0, "randomseed")?);
            r.lock()
                .unwrap_or_else(PoisonError::into_inner)
                .seed(n as u32);
            Ok(())
        })?,
    )?;
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn g14_text() {
        assert_eq!(format_number_g14(1e20), "1e+020");
        assert_eq!(format_number_g14(1.5e-7), "1.5e-007");
        assert_eq!(format_number_g14(0.1), "0.1");
        assert_eq!(format_number_g14(2f64.powi(53)), "9.007199254741e+015");
        assert_eq!(format_number_g14(-0.0), "-0");
        assert_eq!(
            format_number_g14(123_456_789_012_345.0),
            "1.2345678901234e+014"
        );
        assert_eq!(format_number_g14(1e300), "1e+300");
        assert_eq!(format_number_g14(42.0), "42");
        assert_eq!(format_number_g14(0.0001), "0.0001");
        assert_eq!(format_number_g14(0.00001), "1e-005");
        assert_eq!(format_number_g14(9.999_999_999_999_95), "10");
        assert_eq!(format_number_g14(f64::INFINITY), "1.#INF");
        assert_eq!(format_number_g14(f64::NEG_INFINITY), "-1.#INF");
        assert_eq!(format_number_g14(-f64::NAN), "-1.#IND");
        assert_eq!(format_number_g14(f64::NAN.copysign(1.0)), "1.#QNAN");
    }

    #[test]
    fn text_to_number() {
        assert_eq!(str_to_number(b"inf"), None);
        assert_eq!(str_to_number(b"nan"), None);
        assert_eq!(str_to_number(b"0x1p4"), None);
        assert_eq!(str_to_number(b"1,5"), None);
        assert_eq!(str_to_number(b"0x10"), Some(16.0));
        assert_eq!(str_to_number(b"0x100000000"), Some(4_294_967_295.0));
        assert_eq!(str_to_number(b"-0x10"), Some(4_294_967_280.0));
        assert_eq!(str_to_number(b" 12 "), Some(12.0));
        assert_eq!(str_to_number(b"1e"), None);
        assert_eq!(str_to_number(b".5"), Some(0.5));
        assert_eq!(str_to_number(b"5."), Some(5.0));
        assert_eq!(str_to_number(b"."), None);
        assert_eq!(str_to_number(b""), None);
        assert_eq!(str_to_number(b"12\0junk"), Some(12.0));
        assert_eq!(str_to_number(b"0x"), None);
        assert_eq!(str_to_number_base(b"ff", 16), Some(255.0));
        assert_eq!(str_to_number_base(b"zz", 36), Some(1295.0));
        assert_eq!(str_to_number_base(b"12", 2), None);
    }

    #[test]
    fn integer_conversions() {
        assert_eq!(to_c_int(2f64.powi(31)), i32::MIN);
        assert_eq!(to_c_int(-2.7), -2);
        assert_eq!(to_c_int(f64::NAN), i32::MIN);
        assert_eq!(to_c_low32(-1.0), 0xFFFF_FFFF);
        assert_eq!(to_c_low32(2f64.powi(32) + 5.0), 5);
        assert_eq!(to_c_low32(1e30), 0);
        assert_eq!(pad_exponent("1e+20"), "1e+020");
        assert_eq!(pad_exponent("1.5E-07"), "1.5E-007");
        assert_eq!(pad_exponent("1e+300"), "1e+300");
        assert_eq!(pad_exponent("inf"), "inf");
        assert_eq!(apply_width("1.50e+000", b"", 10, true), " 1.50e+000");
        assert_eq!(apply_width("1.50e+000", b"0", 11, true), "001.50e+000");
        assert_eq!(apply_width("-1.5e+000", b"0", 11, true), "-001.5e+000");
        assert_eq!(apply_width("1.5e+000", b"-", 10, true), "1.5e+000  ");
        assert_eq!(apply_width("inf", b"0", 5, false), "  inf");
        assert_eq!(apply_width("1e+020", b"", 3, true), "1e+020");
    }

    #[test]
    fn random_sequence() {
        let mut r = CrtRandom::new();
        r.seed(1);
        assert_eq!(
            [r.next_value(), r.next_value(), r.next_value()],
            [41, 18467, 6334]
        );
    }
}
