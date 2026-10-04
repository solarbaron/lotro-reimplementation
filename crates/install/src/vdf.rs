// SPDX-License-Identifier: GPL-3.0-or-later
//! A small, bounded reader for Steam's text VDF ("KeyValues") files.
//!
//! Spec-Ref: docs/specs/install-layout.md#53-steam-on-linux-and-macos
//!
//! Only the subset used by `libraryfolders.vdf` and `appmanifest_*.acf` is
//! supported: quoted strings, `{` / `}` blocks, backslash escapes, `//`
//! comments. Bare (unquoted) tokens are accepted leniently. Input is treated as
//! hostile: size and nesting depth are bounded and every malformed input is an
//! error, never a panic.

use std::fmt;

/// Largest VDF file this reader accepts, in bytes.
pub const MAX_VDF_SIZE: usize = 1 << 20;

/// Deepest block nesting this reader accepts.
pub const MAX_VDF_DEPTH: usize = 16;

/// A parsed VDF value: either a string or a block of key/value entries.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum VdfValue {
    /// A string value.
    Str(String),
    /// A nested block.
    Block(VdfBlock),
}

impl VdfValue {
    /// The string, if this value is a string.
    pub fn as_str(&self) -> Option<&str> {
        match self {
            Self::Str(s) => Some(s),
            Self::Block(_) => None,
        }
    }

    /// The block, if this value is a block.
    pub fn as_block(&self) -> Option<&VdfBlock> {
        match self {
            Self::Block(b) => Some(b),
            Self::Str(_) => None,
        }
    }
}

/// An ordered list of key/value entries. Keys keep their original case;
/// lookups are ASCII case-insensitive.
#[derive(Debug, Clone, Default, PartialEq, Eq)]
pub struct VdfBlock {
    /// Entries in file order.
    pub entries: Vec<(String, VdfValue)>,
}

impl VdfBlock {
    /// The value of `key` (case-insensitive). If the key repeats, the last
    /// occurrence wins.
    pub fn get(&self, key: &str) -> Option<&VdfValue> {
        self.entries
            .iter()
            .rev()
            .find(|(k, _)| k.eq_ignore_ascii_case(key))
            .map(|(_, v)| v)
    }

    /// The string value of `key`, if present and a string.
    pub fn get_str(&self, key: &str) -> Option<&str> {
        self.get(key).and_then(VdfValue::as_str)
    }

    /// The block value of `key`, if present and a block.
    pub fn get_block(&self, key: &str) -> Option<&VdfBlock> {
        self.get(key).and_then(VdfValue::as_block)
    }

    /// Iterate over the entries in file order.
    pub fn iter(&self) -> impl Iterator<Item = (&str, &VdfValue)> {
        self.entries.iter().map(|(k, v)| (k.as_str(), v))
    }
}

/// What went wrong while parsing VDF text.
#[derive(Debug, Clone, PartialEq, Eq)]
#[non_exhaustive]
pub enum VdfErrorKind {
    /// The input exceeds [`MAX_VDF_SIZE`].
    TooLarge,
    /// A quoted string has no closing quote.
    UnterminatedString,
    /// A `}` appeared with no open block.
    UnexpectedClose,
    /// A `{` appeared where a key was expected.
    UnexpectedOpen,
    /// The input ended inside a block.
    UnclosedBlock,
    /// The input ended after a key with no value.
    MissingValue,
    /// Blocks are nested deeper than [`MAX_VDF_DEPTH`].
    TooDeep,
}

/// A VDF parse error with the 1-based line where it was detected.
#[derive(Debug, Clone, PartialEq, Eq, thiserror::Error)]
pub struct VdfError {
    /// 1-based line number.
    pub line: usize,
    /// The kind of error.
    pub kind: VdfErrorKind,
}

impl fmt::Display for VdfError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        let what = match self.kind {
            VdfErrorKind::TooLarge => "input too large",
            VdfErrorKind::UnterminatedString => "unterminated string",
            VdfErrorKind::UnexpectedClose => "unbalanced '}'",
            VdfErrorKind::UnexpectedOpen => "'{' where a key was expected",
            VdfErrorKind::UnclosedBlock => "unclosed block at end of input",
            VdfErrorKind::MissingValue => "key without a value at end of input",
            VdfErrorKind::TooDeep => "blocks nested too deeply",
        };
        write!(f, "line {}: {what}", self.line)
    }
}

#[derive(Debug, PartialEq, Eq)]
enum Token {
    Str(String),
    Open,
    Close,
}

struct Lexer<'a> {
    chars: std::iter::Peekable<std::str::Chars<'a>>,
    line: usize,
}

impl Lexer<'_> {
    fn err(&self, kind: VdfErrorKind) -> VdfError {
        VdfError {
            line: self.line,
            kind,
        }
    }

    fn next_token(&mut self) -> Result<Option<Token>, VdfError> {
        loop {
            let Some(&c) = self.chars.peek() else {
                return Ok(None);
            };
            match c {
                '\n' => {
                    self.line += 1;
                    self.chars.next();
                }
                c if c.is_whitespace() => {
                    self.chars.next();
                }
                '{' => {
                    self.chars.next();
                    return Ok(Some(Token::Open));
                }
                '}' => {
                    self.chars.next();
                    return Ok(Some(Token::Close));
                }
                '"' => {
                    self.chars.next();
                    return self.quoted().map(|s| Some(Token::Str(s)));
                }
                '/' => {
                    self.chars.next();
                    if self.chars.peek() == Some(&'/') {
                        // Comment to end of line; the newline itself is handled
                        // by the main loop so the line count stays right.
                        while let Some(&c) = self.chars.peek() {
                            if c == '\n' {
                                break;
                            }
                            self.chars.next();
                        }
                    } else {
                        let mut s = String::from('/');
                        self.bare_into(&mut s);
                        return Ok(Some(Token::Str(s)));
                    }
                }
                _ => {
                    let mut s = String::new();
                    self.bare_into(&mut s);
                    return Ok(Some(Token::Str(s)));
                }
            }
        }
    }

    fn quoted(&mut self) -> Result<String, VdfError> {
        let start_line = self.line;
        let mut s = String::new();
        loop {
            match self.chars.next() {
                None => {
                    return Err(VdfError {
                        line: start_line,
                        kind: VdfErrorKind::UnterminatedString,
                    });
                }
                Some('"') => return Ok(s),
                Some('\\') => match self.chars.next() {
                    None => {
                        return Err(VdfError {
                            line: start_line,
                            kind: VdfErrorKind::UnterminatedString,
                        });
                    }
                    Some(c) => {
                        if c == '\n' {
                            self.line += 1;
                        }
                        s.push(c);
                    }
                },
                Some(c) => {
                    if c == '\n' {
                        self.line += 1;
                    }
                    s.push(c);
                }
            }
        }
    }

    fn bare_into(&mut self, s: &mut String) {
        while let Some(&c) = self.chars.peek() {
            if c.is_whitespace() || c == '{' || c == '}' || c == '"' {
                break;
            }
            s.push(c);
            self.chars.next();
        }
    }
}

/// Parse VDF text. The top level is returned as an implicit block.
pub fn parse(input: &str) -> Result<VdfBlock, VdfError> {
    if input.len() > MAX_VDF_SIZE {
        return Err(VdfError {
            line: 1,
            kind: VdfErrorKind::TooLarge,
        });
    }
    parse_unchecked(input)
}

/// Parse without the size check; callers apply [`MAX_VDF_SIZE`] to their own
/// input (raw bytes for [`parse_bytes`]).
fn parse_unchecked(input: &str) -> Result<VdfBlock, VdfError> {
    let input = input.strip_prefix('\u{feff}').unwrap_or(input);
    let mut lx = Lexer {
        chars: input.chars().peekable(),
        line: 1,
    };
    // Stack of (key of the open block, entries collected so far).
    let mut stack: Vec<(String, VdfBlock)> = Vec::new();
    let mut current = VdfBlock::default();

    while let Some(tok) = lx.next_token()? {
        match tok {
            Token::Close => match stack.pop() {
                Some((key, mut parent)) => {
                    let done = std::mem::take(&mut current);
                    parent.entries.push((key, VdfValue::Block(done)));
                    current = parent;
                }
                None => return Err(lx.err(VdfErrorKind::UnexpectedClose)),
            },
            Token::Open => return Err(lx.err(VdfErrorKind::UnexpectedOpen)),
            Token::Str(key) => match lx.next_token()? {
                None => return Err(lx.err(VdfErrorKind::MissingValue)),
                Some(Token::Str(value)) => current.entries.push((key, VdfValue::Str(value))),
                Some(Token::Open) => {
                    if stack.len() >= MAX_VDF_DEPTH {
                        return Err(lx.err(VdfErrorKind::TooDeep));
                    }
                    let parent = std::mem::take(&mut current);
                    stack.push((key, parent));
                }
                Some(Token::Close) => return Err(lx.err(VdfErrorKind::UnexpectedClose)),
            },
        }
    }
    if !stack.is_empty() {
        return Err(lx.err(VdfErrorKind::UnclosedBlock));
    }
    Ok(current)
}

/// Parse VDF bytes (UTF-8, optional BOM; invalid sequences are replaced).
pub fn parse_bytes(bytes: &[u8]) -> Result<VdfBlock, VdfError> {
    if bytes.len() > MAX_VDF_SIZE {
        return Err(VdfError {
            line: 1,
            kind: VdfErrorKind::TooLarge,
        });
    }
    // The cap applies to the raw file bytes: replacement characters for
    // invalid UTF-8 may make the decoded text up to 3x larger (bounded).
    parse_unchecked(&String::from_utf8_lossy(bytes))
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn parses_nested_blocks_and_escapes() {
        let text = "\"root\"\n{\n  \"path\"  \"C:\\\\Program Files\\\\Steam\" // c\n  \"q\" \"a\\\"b\"\n  \"apps\" { \"1\" \"2\" }\n}\n";
        let v = parse(text).unwrap();
        let root = v.get_block("ROOT").unwrap();
        assert_eq!(root.get_str("Path"), Some("C:\\Program Files\\Steam"));
        assert_eq!(root.get_str("q"), Some("a\"b"));
        assert_eq!(root.get_block("apps").unwrap().get_str("1"), Some("2"));
    }

    #[test]
    fn last_duplicate_wins() {
        let v = parse("\"a\" \"1\" \"A\" \"2\"").unwrap();
        assert_eq!(v.get_str("a"), Some("2"));
    }

    #[test]
    fn rejects_malformed() {
        let cases = [
            ("\"a\" { ", VdfErrorKind::UnclosedBlock),
            ("}", VdfErrorKind::UnexpectedClose),
            ("\"a\" }", VdfErrorKind::UnexpectedClose),
            ("{", VdfErrorKind::UnexpectedOpen),
            ("\"a", VdfErrorKind::UnterminatedString),
            ("\"a\" \"b\\", VdfErrorKind::UnterminatedString),
            ("\"a\"", VdfErrorKind::MissingValue),
        ];
        for (text, kind) in cases {
            assert_eq!(parse(text).unwrap_err().kind, kind, "{text:?}");
        }
    }

    #[test]
    fn depth_is_bounded() {
        let ok = "\"k\" {".repeat(MAX_VDF_DEPTH) + &"}".repeat(MAX_VDF_DEPTH);
        assert!(parse(&ok).is_ok());
        let deep = "\"k\" {".repeat(MAX_VDF_DEPTH + 1) + &"}".repeat(MAX_VDF_DEPTH + 1);
        assert_eq!(parse(&deep).unwrap_err().kind, VdfErrorKind::TooDeep);
    }

    #[test]
    fn size_is_bounded() {
        let big = vec![b' '; MAX_VDF_SIZE + 1];
        assert_eq!(parse_bytes(&big).unwrap_err().kind, VdfErrorKind::TooLarge);
    }

    /// The size cap applies to the raw bytes: lossy UTF-8 replacement must
    /// not turn an in-limit file into a `TooLarge` error.
    #[test]
    fn invalid_utf8_within_limit_is_replaced_not_rejected() {
        let mut b = b"\"a\" \"".to_vec();
        b.extend(std::iter::repeat_n(0xFFu8, 400_000));
        b.push(b'"');
        let doc = parse_bytes(&b).unwrap();
        assert_eq!(doc.get_str("a").map(|s| s.chars().count()), Some(400_000));
    }

    #[test]
    fn error_line_numbers() {
        let e = parse("\"a\" \"b\"\n\n}").unwrap_err();
        assert_eq!(e.line, 3);
    }

    proptest::proptest! {
        #[test]
        fn never_panics(s in "[\"{}a-z\\\\/ \n\t\r]{0,2000}") {
            let _ = parse(&s);
        }

        #[test]
        fn never_panics_bytes(b in proptest::collection::vec(proptest::num::u8::ANY, 0..4096)) {
            let _ = parse_bytes(&b);
        }
    }
}
