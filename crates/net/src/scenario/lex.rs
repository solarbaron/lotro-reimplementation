// SPDX-License-Identifier: GPL-3.0-or-later
//! Lines, tokens and numbers of the scenario text format (`scenario-feed.md` §4.1, §4.2).

use super::error::{Limit, ScenarioError, ScenarioErrorKind as K};

/// Largest scenario file in bytes.
pub const MAX_FILE_BYTES: usize = 1_048_576;
/// Largest line in bytes, not counting the terminator.
pub const MAX_LINE_BYTES: usize = 4096;
/// Most lines in a file.
pub const MAX_LINES: usize = 65_536;
/// Most tokens on one line.
pub const MAX_TOKENS: usize = 64;
/// Largest string after unescaping.
pub const MAX_STRING_BYTES: usize = 1024;
/// Largest keyword or binding (binding including `$`).
pub const MAX_KEYWORD_BYTES: usize = 32;
/// Largest label including `@`.
pub const MAX_LABEL_BYTES: usize = 128;
/// Largest numeric component in characters.
pub const MAX_NUMBER_CHARS: usize = 32;

/// One piece of a token: raw text outside strings, or an unescaped string.
#[derive(Debug, Clone, PartialEq, Eq)]
pub(crate) enum Seg {
    Raw { text: String, col: u32 },
    Str { text: String, col: u32 },
}

/// A token: a maximal run of non-whitespace characters, strings included.
#[derive(Debug, Clone, PartialEq, Eq)]
pub(crate) struct Token {
    pub col: u32,
    pub segs: Vec<Seg>,
}

impl Token {
    /// The token's text when it holds no string.
    pub(crate) fn raw(&self) -> Option<&str> {
        match self.segs.as_slice() {
            [Seg::Raw { text, .. }] => Some(text),
            _ => None,
        }
    }

    /// The string when the token is exactly one string.
    pub(crate) fn string(&self) -> Option<&str> {
        match self.segs.as_slice() {
            [Seg::Str { text, .. }] => Some(text),
            _ => None,
        }
    }

    /// Whether the token is exactly the word `w`.
    pub(crate) fn is(&self, w: &str) -> bool {
        self.raw() == Some(w)
    }

    /// For a `key=value` token: the key (text before the first `=` of the leading raw
    /// text) and the value as a token of its own (possibly empty).
    pub(crate) fn split_eq(&self) -> Option<(&str, Token)> {
        let Some(Seg::Raw { text, col }) = self.segs.first() else {
            return None;
        };
        let pos = text.find('=')?;
        let key = &text[..pos];
        let rest = &text[pos + 1..];
        let key_chars = u32::try_from(key.chars().count()).unwrap_or(u32::MAX);
        let value_col = col.saturating_add(key_chars).saturating_add(1);
        let mut segs = Vec::with_capacity(self.segs.len());
        if !rest.is_empty() {
            segs.push(Seg::Raw {
                text: rest.to_owned(),
                col: value_col,
            });
        }
        segs.extend(self.segs[1..].iter().cloned());
        Some((
            key,
            Token {
                col: value_col,
                segs,
            },
        ))
    }

    /// Splits the token on `sep` outside strings into sub-tokens (empty pieces kept, as
    /// tokens without segments).
    pub(crate) fn split_on(&self, sep: char) -> Vec<Token> {
        let mut out = Vec::new();
        let mut cur = Token {
            col: self.col,
            segs: Vec::new(),
        };
        for seg in &self.segs {
            match seg {
                Seg::Str { .. } => cur.segs.push(seg.clone()),
                Seg::Raw { text, col } => {
                    let mut start = 0usize;
                    let mut start_col = *col;
                    let mut c = *col;
                    for (i, ch) in text.char_indices() {
                        if ch == sep {
                            if i > start {
                                cur.segs.push(Seg::Raw {
                                    text: text[start..i].to_owned(),
                                    col: start_col,
                                });
                            }
                            out.push(std::mem::replace(
                                &mut cur,
                                Token {
                                    col: c.saturating_add(1),
                                    segs: Vec::new(),
                                },
                            ));
                            start = i + ch.len_utf8();
                            start_col = c.saturating_add(1);
                        }
                        c = c.saturating_add(1);
                    }
                    if start < text.len() {
                        cur.segs.push(Seg::Raw {
                            text: text[start..].to_owned(),
                            col: start_col,
                        });
                    }
                }
            }
        }
        out.push(cur);
        out
    }
}

/// One line of the file after encoding checks: its number and text.
#[derive(Debug, Clone, Copy)]
pub(crate) struct Line<'a> {
    pub number: u32,
    pub text: &'a str,
}

fn is_control(c: char) -> bool {
    let u = u32::from(c);
    (u <= 0x1F && c != '\t') || (0x7F..=0x9F).contains(&u)
}

/// Splits the file into checked lines (§4.1): size, line count, line length, encoding,
/// control characters. Errors are reported in file order.
pub(crate) fn lines(bytes: &[u8]) -> impl Iterator<Item = Result<Line<'_>, ScenarioError>> {
    let mut rest = Some(bytes);
    let mut number: u32 = 0;
    let mut failed = false;
    std::iter::from_fn(move || {
        if failed {
            return None;
        }
        let data = rest?;
        if number == 0 && bytes.len() > MAX_FILE_BYTES {
            failed = true;
            return Some(Err(ScenarioError::new(K::FileTooLarge, 1, 1)));
        }
        if data.is_empty() {
            return None;
        }
        number = number.saturating_add(1);
        let (mut raw, terminated) = match data.iter().position(|&b| b == b'\n') {
            Some(i) => {
                rest = Some(&data[i + 1..]);
                (&data[..i], true)
            }
            None => {
                rest = None;
                (data, false)
            }
        };
        if terminated && raw.last() == Some(&b'\r') {
            raw = &raw[..raw.len() - 1];
        }
        let mut fail = |kind, col| {
            failed = true;
            Some(Err(ScenarioError::new(kind, number, col)))
        };
        if usize::try_from(number).unwrap_or(usize::MAX) > MAX_LINES {
            return fail(K::LimitExceeded(Limit::Lines), 1);
        }
        if raw.len() > MAX_LINE_BYTES {
            return fail(K::LineTooLong, 1);
        }
        let (valid, invalid) = match std::str::from_utf8(raw) {
            Ok(s) => (s, false),
            Err(e) => {
                // The prefix up to `valid_up_to` is valid UTF-8 by construction.
                let s = std::str::from_utf8(&raw[..e.valid_up_to()]).unwrap_or("");
                (s, true)
            }
        };
        let mut col: u32 = 0;
        for c in valid.chars() {
            col = col.saturating_add(1);
            if number == 1 && col == 1 && c == '\u{FEFF}' {
                return fail(K::Encoding, 1);
            }
            if is_control(c) {
                return fail(K::ControlCharacter, col);
            }
        }
        if invalid {
            return fail(K::Encoding, col.saturating_add(1));
        }
        Some(Ok(Line {
            number,
            text: valid,
        }))
    })
}

/// Splits one checked line into tokens (§4.2): strings with escapes, comments, the token
/// limit.
pub(crate) fn tokenize(line: Line<'_>) -> Result<Vec<Token>, ScenarioError> {
    let n = line.number;
    let err = |kind, col| ScenarioError::new(kind, n, col);
    let chars: Vec<char> = line.text.chars().collect();
    let mut tokens: Vec<Token> = Vec::new();
    let mut i = 0usize;
    let col_of = |i: usize| u32::try_from(i + 1).unwrap_or(u32::MAX);
    while i < chars.len() {
        let c = chars[i];
        if c == ' ' || c == '\t' {
            i += 1;
            continue;
        }
        if c == '#' {
            break;
        }
        if tokens.len() >= MAX_TOKENS {
            return Err(err(K::LimitExceeded(Limit::Tokens), col_of(i)));
        }
        let mut tok = Token {
            col: col_of(i),
            segs: Vec::new(),
        };
        let mut raw = String::new();
        let mut raw_col = col_of(i);
        while i < chars.len() {
            let c = chars[i];
            if c == ' ' || c == '\t' || c == '#' {
                break;
            }
            if c == '"' {
                if !raw.is_empty() {
                    tok.segs.push(Seg::Raw {
                        text: std::mem::take(&mut raw),
                        col: raw_col,
                    });
                }
                let open = i;
                i += 1;
                let mut s = String::new();
                let mut closed = false;
                while i < chars.len() {
                    let c = chars[i];
                    if c == '"' {
                        closed = true;
                        i += 1;
                        break;
                    }
                    if c == '\\' {
                        let at = col_of(i);
                        match chars.get(i + 1) {
                            Some('"') => {
                                s.push('"');
                                i += 2;
                            }
                            Some('\\') => {
                                s.push('\\');
                                i += 2;
                            }
                            Some('u') if chars.get(i + 2) == Some(&'{') => {
                                let mut j = i + 3;
                                let mut v: u32 = 0;
                                let mut digits = 0;
                                while let Some(d) = chars.get(j).and_then(|c| c.to_digit(16)) {
                                    digits += 1;
                                    if digits > 6 {
                                        break;
                                    }
                                    v = v * 16 + d;
                                    j += 1;
                                }
                                if digits == 0 || digits > 6 || chars.get(j) != Some(&'}') {
                                    return Err(err(K::BadEscape, at));
                                }
                                match char::from_u32(v) {
                                    Some(ch) if !is_control(ch) => s.push(ch),
                                    _ => return Err(err(K::BadEscape, at)),
                                }
                                i = j + 1;
                            }
                            _ => return Err(err(K::BadEscape, at)),
                        }
                        continue;
                    }
                    s.push(c);
                    i += 1;
                }
                if !closed {
                    return Err(err(K::UnterminatedString, col_of(open)));
                }
                if s.len() > MAX_STRING_BYTES {
                    return Err(err(K::TooLong, tok.col));
                }
                tok.segs.push(Seg::Str {
                    text: s,
                    col: col_of(open),
                });
                raw_col = col_of(i);
                continue;
            }
            if raw.is_empty() {
                raw_col = col_of(i);
            }
            raw.push(c);
            i += 1;
        }
        if !raw.is_empty() {
            tok.segs.push(Seg::Raw {
                text: raw,
                col: raw_col,
            });
        }
        tokens.push(tok);
    }
    Ok(tokens)
}

/// Whether `s` is in the keyword class (§4.2), ignoring length.
pub(crate) fn is_keyword_shape(s: &str) -> bool {
    let mut it = s.bytes();
    matches!(it.next(), Some(b'a'..=b'z'))
        && it.all(|b| b.is_ascii_lowercase() || b.is_ascii_digit() || b == b'-')
}

/// Whether `s` is in the binding class (§4.2), ignoring length.
pub(crate) fn is_binding_shape(s: &str) -> bool {
    let mut it = s.bytes();
    it.next() == Some(b'$')
        && matches!(it.next(), Some(b'a'..=b'z'))
        && it.all(|b| b.is_ascii_lowercase() || b.is_ascii_digit() || b == b'_')
}

/// Whether `s` is in the label class (§4.2), ignoring length.
pub(crate) fn is_label_shape(s: &str) -> bool {
    let mut it = s.bytes();
    it.next() == Some(b'@')
        && matches!(it.next(), Some(b) if b.is_ascii_alphabetic() || b == b'_')
        && it.all(|b| b.is_ascii_alphanumeric() || b == b'_')
}

/// Reads a decimal (or, with `hex`, a `0x` hexadecimal) integer exactly (§4.2).
pub(crate) fn int(s: &str, hex: bool) -> Result<i128, K> {
    if s.chars().count() > MAX_NUMBER_CHARS {
        return Err(K::BadNumber);
    }
    if let Some(h) = s.strip_prefix("0x") {
        if !hex || h.is_empty() || h.len() > 16 || !h.bytes().all(|b| b.is_ascii_hexdigit()) {
            return Err(K::BadNumber);
        }
        return u64::from_str_radix(h, 16)
            .map(i128::from)
            .map_err(|_| K::BadNumber);
    }
    let (neg, digits) = match s.strip_prefix('-') {
        Some(d) => (true, d),
        None => (false, s),
    };
    if digits.is_empty()
        || digits.len() > 20
        || !digits.bytes().all(|b| b.is_ascii_digit())
        || (digits.len() > 1 && digits.starts_with('0'))
    {
        return Err(K::BadNumber);
    }
    let v: i128 = digits.parse().map_err(|_| K::BadNumber)?;
    Ok(if neg { -v } else { v })
}

/// Splits a decimal into sign, integer digits and fraction digits, or `None`.
fn decimal_parts(s: &str) -> Option<(bool, &str, &str)> {
    let (neg, body) = match s.strip_prefix('-') {
        Some(b) => (true, b),
        None => (false, s),
    };
    let (ip, fp) = match body.split_once('.') {
        Some((i, f)) => {
            if f.is_empty() || f.len() > 9 {
                return None;
            }
            (i, f)
        }
        None => (body, ""),
    };
    if ip.is_empty()
        || !ip.bytes().all(|b| b.is_ascii_digit())
        || !fp.bytes().all(|b| b.is_ascii_digit())
    {
        return None;
    }
    Some((neg, ip, fp))
}

/// Reads a decimal (§4.2) to the nearest `f64`; `bad` is the error for a malformed one.
pub(crate) fn decimal(s: &str, bad: K) -> Result<f64, K> {
    if s.chars().count() > MAX_NUMBER_CHARS {
        return Err(K::BadNumber);
    }
    if decimal_parts(s).is_none() {
        return Err(bad);
    }
    let v: f64 = s.parse().map_err(|_| bad)?;
    if v.is_finite() { Ok(v) } else { Err(bad) }
}

/// Reads a duration (§4.2) to whole milliseconds, exactly on the digits.
pub(crate) fn duration(s: &str) -> Result<u64, K> {
    if s.chars().count() > MAX_NUMBER_CHARS {
        return Err(K::BadNumber);
    }
    let (num, unit): (&str, i128) = if let Some(n) = s.strip_suffix("min") {
        (n, 60_000)
    } else if let Some(n) = s.strip_suffix("ms") {
        (n, 1)
    } else if let Some(n) = s.strip_suffix('s') {
        (n, 1000)
    } else {
        return Err(K::BadDuration);
    };
    let (neg, ip, fp) = decimal_parts(num).ok_or(K::BadDuration)?;
    if neg {
        return Err(K::BadDuration);
    }
    let mut digits = String::with_capacity(ip.len() + fp.len());
    digits.push_str(ip);
    digits.push_str(fp);
    let n: i128 = digits.parse().map_err(|_| K::BadDuration)?;
    let scale = 10i128
        .checked_pow(u32::try_from(fp.len()).map_err(|_| K::BadDuration)?)
        .ok_or(K::BadDuration)?;
    let total = n.checked_mul(unit).ok_or(K::BadDuration)?;
    if total % scale != 0 {
        return Err(K::BadDuration);
    }
    u64::try_from(total / scale).map_err(|_| K::OutOfRange)
}

#[cfg(test)]
mod tests {
    use super::*;

    fn toks(s: &str) -> Result<Vec<Token>, ScenarioError> {
        tokenize(Line { number: 1, text: s })
    }

    #[test]
    fn tokens_with_strings_and_comments() {
        let t = toks(r#"actor $a name="Old guide" pos=1,2,3 # x"#).unwrap();
        assert_eq!(t.len(), 4);
        assert_eq!(t[2].col, 10);
        let (k, v) = t[2].split_eq().unwrap();
        assert_eq!(k, "name");
        assert_eq!(v.string(), Some("Old guide"));
        assert_eq!(v.col, 15);
        let t = toks(r#"say "a#b" "\u{41}\"\\""#).unwrap();
        assert_eq!(t[1].string(), Some("a#b"));
        assert_eq!(t[2].string(), Some("A\"\\"));
    }

    #[test]
    fn escape_errors() {
        assert_eq!(toks(r#"title "\q""#).unwrap_err().column, 8);
        assert_eq!(toks(r#"title "\u{7}""#).unwrap_err().kind, K::BadEscape);
        assert_eq!(toks(r#"title "\u{D800}""#).unwrap_err().kind, K::BadEscape);
        assert_eq!(
            toks(r#"title "\u{1234567}""#).unwrap_err().kind,
            K::BadEscape
        );
        let e = toks(r#"title "abc"#).unwrap_err();
        assert_eq!((e.kind, e.column), (K::UnterminatedString, 7));
    }

    #[test]
    fn numbers() {
        assert_eq!(int("0", false), Ok(0));
        assert_eq!(int("-12", false), Ok(-12));
        assert_eq!(int("012", false), Err(K::BadNumber));
        assert_eq!(int("0x1F", true), Ok(31));
        assert_eq!(int("0x1F", false), Err(K::BadNumber));
        assert_eq!(
            int("18446744073709551615", false),
            Ok(18_446_744_073_709_551_615)
        );
        assert_eq!(duration("0.5s"), Ok(500));
        assert_eq!(duration("61min"), Ok(3_660_000));
        assert_eq!(duration("0.0005s"), Err(K::BadDuration));
        assert_eq!(duration("2h"), Err(K::BadDuration));
        assert_eq!(duration("-1s"), Err(K::BadDuration));
        assert_eq!(duration("1.5ms"), Err(K::BadDuration));
        assert_eq!(
            decimal("-99999.999999999", K::BadNumber),
            Ok(-99_999.999_999_999)
        );
        assert_eq!(decimal("1e5", K::BadNumber), Err(K::BadNumber));
        assert_eq!(decimal(&"1".repeat(33), K::BadPosition), Err(K::BadNumber));
    }

    #[test]
    fn line_checks() {
        let r: Vec<_> = lines(b"a\r\nb\xFFc\n").collect();
        assert!(r[0].is_ok());
        let e = r[1].clone().unwrap_err();
        assert_eq!((e.kind, e.line, e.column), (K::Encoding, 2, 2));
        let r: Vec<_> = lines(b"a\rb").collect();
        assert_eq!(r[0].clone().unwrap_err().kind, K::ControlCharacter);
        let r: Vec<_> = lines("\u{FEFF}x".as_bytes()).collect();
        assert_eq!(r[0].clone().unwrap_err().kind, K::Encoding);
    }
}
