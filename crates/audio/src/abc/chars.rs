// SPDX-License-Identifier: GPL-3.0-or-later
//! Bytes to characters: byte-order mark, text decoding, comments, backslashes, line and
//! column numbers (`abc-dialect.md` §3.2–§3.5).

/// A position in the file: line and column, both counted from 1 (`abc-dialect.md` §3.5).
///
/// Every character read advances the column by one, except line breaks, comment
/// characters and backslashes. A token's position is that of its first character.
#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash)]
pub struct Position {
    /// Line number, from 1.
    pub line: u32,
    /// Column number, from 1 at the start of each line.
    pub column: u32,
}

impl Position {
    /// Line 1, column 1.
    pub const START: Position = Position { line: 1, column: 1 };
}

impl std::fmt::Display for Position {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(f, "line {}, column {}", self.line, self.column)
    }
}

/// What the first bytes of a file say about its encoding (`abc-dialect.md` §3.2).
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(crate) enum Bom {
    /// No byte-order mark (or a file shorter than 4 bytes): plain decoder from byte 0.
    Plain,
    /// UTF-8 mark: UTF-8 decoder from byte 3.
    Utf8,
    /// UTF-16 (or UTF-32 LE) mark: refused, "encoding not supported".
    Utf16,
    /// SCSU, UTF-7, BOCU-1, UTF-32 BE or UTF-EBCDIC mark: treated as a missing file.
    Unsupported,
}

/// Classifies the first bytes of a file (`abc-dialect.md` §3.2). Files shorter than 4
/// bytes have no mark.
pub(crate) fn detect_bom(bytes: &[u8]) -> Bom {
    if bytes.len() < 4 {
        return Bom::Plain;
    }
    match bytes {
        [0xFE, 0xFF, ..] | [0xFF, 0xFE, ..] => Bom::Utf16,
        [0xEF, 0xBB, 0xBF, ..] => Bom::Utf8,
        [0x0E, 0xFE, 0xFF, ..]
        | [0x2B, 0x2F, 0x76, ..]
        | [0xFB, 0xEE, 0x28, ..]
        | [0x00, 0x00, 0xFE, 0xFF, ..]
        | [0x00, 0x00, 0xFF, 0xFE, ..]
        | [0xDD, 0x73, 0x66, 0x73, ..] => Bom::Unsupported,
        _ => Bom::Plain,
    }
}

/// One character after decoding.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(crate) enum Ch {
    /// End of input, a NUL byte or a failed UTF-8 sequence (§3.3). Sticky.
    End,
    /// CR, LF or CR LF (§3.3).
    Break,
    /// Any other character.
    Char(char),
}

/// White space as the tokenizer classifies it (§4): space, tab, VT, FF, and non-ASCII
/// white space by Unicode's definition (the original uses the C runtime's wide-character
/// test, which is not observable here; see the OPEN note in §4).
pub(crate) fn is_space(c: char) -> bool {
    matches!(c, ' ' | '\t' | '\u{0B}' | '\u{0C}') || (!c.is_ascii() && c.is_whitespace())
}

/// Letters (§4): ASCII letters and non-ASCII alphabetic characters.
pub(crate) fn is_letter(c: char) -> bool {
    c.is_ascii_alphabetic() || (!c.is_ascii() && c.is_alphabetic())
}

/// Digits (§4.1, Tessera's choice): the ASCII digits only.
pub(crate) fn is_digit(c: char) -> bool {
    c.is_ascii_digit()
}

/// The character stream: decodes bytes, removes comments and backslashes, numbers lines
/// and columns, and offers one character of look-ahead.
#[derive(Debug)]
pub(crate) struct CharStream<'a> {
    bytes: &'a [u8],
    pos: usize,
    utf8: bool,
    ended: bool,
    line: u32,
    column: u32,
    peeked: Option<(Ch, Position)>,
}

impl<'a> CharStream<'a> {
    /// A stream over `bytes` (already past any byte-order mark); `utf8` selects the UTF-8
    /// decoder, otherwise each byte is one character.
    pub(crate) fn new(bytes: &'a [u8], utf8: bool) -> Self {
        CharStream {
            bytes,
            pos: 0,
            utf8,
            ended: false,
            line: 1,
            column: 1,
            peeked: None,
        }
    }

    /// Next decoded character before comment and backslash handling.
    fn raw(&mut self) -> Ch {
        if self.ended {
            return Ch::End;
        }
        let Some(&b) = self.bytes.get(self.pos) else {
            self.ended = true;
            return Ch::End;
        };
        match b {
            0 => {
                self.ended = true;
                Ch::End
            }
            // Text mode: CR LF arrives as one line break; a lone CR is a line break too.
            b'\r' => {
                self.pos += 1;
                if self.bytes.get(self.pos) == Some(&b'\n') {
                    self.pos += 1;
                }
                Ch::Break
            }
            b'\n' => {
                self.pos += 1;
                Ch::Break
            }
            _ if !self.utf8 => {
                // Plain files: one byte, one character. Bytes 0x80–0xFF map to U+0080–U+00FF
                // (identity; the original depends on the C runtime locale, §3.3 OPEN).
                self.pos += 1;
                Ch::Char(char::from(b))
            }
            0x01..=0x7F => {
                self.pos += 1;
                Ch::Char(char::from(b))
            }
            0x80..=0xBF => {
                // A continuation byte without a lead byte: 0xFF00 + byte.
                self.pos += 1;
                match char::from_u32(0xFF00 + u32::from(b)) {
                    Some(c) => Ch::Char(c),
                    None => {
                        self.ended = true;
                        Ch::End
                    }
                }
            }
            _ => {
                // 0xC0–0xDF: two bytes; 0xE0–0xFF: three bytes (no 4-byte sequences). The
                // conversion is strict; a failure is character 0, which ends the input.
                let n = if b <= 0xDF { 2 } else { 3 };
                let decoded = self
                    .bytes
                    .get(self.pos..self.pos + n)
                    .and_then(|s| std::str::from_utf8(s).ok())
                    .and_then(|s| s.chars().next());
                match decoded {
                    Some(c) if c != '\0' => {
                        self.pos += n;
                        Ch::Char(c)
                    }
                    _ => {
                        self.ended = true;
                        Ch::End
                    }
                }
            }
        }
    }

    /// Next character after comments and backslashes, with its position (§3.4–§3.5).
    fn filtered(&mut self) -> (Ch, Position) {
        loop {
            match self.raw() {
                Ch::Char('%') => {
                    // A comment: drop everything up to the line break, keep the break.
                    loop {
                        match self.raw() {
                            Ch::Char(_) => {}
                            other => return self.place(other),
                        }
                    }
                }
                // A backslash is dropped; the next character is read in its place.
                Ch::Char('\\') => {}
                other => return self.place(other),
            }
        }
    }

    fn place(&mut self, ch: Ch) -> (Ch, Position) {
        let at = Position {
            line: self.line,
            column: self.column,
        };
        match ch {
            Ch::Char(_) => self.column = self.column.saturating_add(1),
            Ch::Break => {
                self.line = self.line.saturating_add(1);
                self.column = 1;
            }
            Ch::End => {}
        }
        (ch, at)
    }

    /// Takes the next character.
    pub(crate) fn next_char(&mut self) -> (Ch, Position) {
        match self.peeked.take() {
            Some(p) => p,
            None => self.filtered(),
        }
    }

    /// Looks at the next character without taking it.
    pub(crate) fn peek(&mut self) -> (Ch, Position) {
        match self.peeked {
            Some(p) => p,
            None => {
                let p = self.filtered();
                self.peeked = Some(p);
                p
            }
        }
    }

    /// Takes characters up to and including the next line break (or the end).
    pub(crate) fn skip_line(&mut self) {
        loop {
            match self.next_char().0 {
                Ch::Char(_) => {}
                Ch::Break | Ch::End => return,
            }
        }
    }

    /// Takes white-space characters (not line breaks).
    pub(crate) fn skip_spaces(&mut self) {
        while let (Ch::Char(c), _) = self.peek() {
            if !is_space(c) {
                break;
            }
            self.next_char();
        }
    }

    /// Reads the rest of the line (taking the line break) as a C `strtol` conversion with
    /// base detection (`0x` hexadecimal, leading `0` octal, else decimal) and a 32-bit
    /// `long`: leading white space and one sign are skipped, conversion stops at the first
    /// character that does not fit, no digits give 0, overflow saturates (§5.3 step 5).
    pub(crate) fn line_strtol(&mut self) -> i64 {
        const POS_MAX: i64 = i32::MAX as i64;
        const NEG_MAG: i64 = -(i32::MIN as i64);
        #[derive(PartialEq)]
        enum St {
            Lead,
            Signed,
            Zero,
            ZeroX,
            Digits,
            Done,
        }
        let mut st = St::Lead;
        let mut negative = false;
        let mut radix: u32 = 10;
        let mut mag: i64 = 0;
        while let Ch::Char(c) = self.next_char().0 {
            if st == St::Done {
                continue;
            }
            let digit = if c.is_ascii() { c.to_digit(16) } else { None };
            match st {
                St::Lead if is_space(c) => {}
                St::Lead if c == '+' || c == '-' => {
                    negative = c == '-';
                    st = St::Signed;
                }
                St::Lead | St::Signed => {
                    if c == '0' {
                        st = St::Zero;
                    } else if let Some(d) = digit.filter(|&d| d < 10) {
                        mag = i64::from(d);
                        st = St::Digits;
                    } else {
                        st = St::Done;
                    }
                }
                St::Zero => {
                    if c == 'x' || c == 'X' {
                        st = St::ZeroX;
                    } else if let Some(d) = digit.filter(|&d| d < 8) {
                        radix = 8;
                        mag = i64::from(d);
                        st = St::Digits;
                    } else {
                        st = St::Done;
                    }
                }
                St::ZeroX => {
                    // "0x" without a hexadecimal digit converts only the "0".
                    if let Some(d) = digit {
                        radix = 16;
                        mag = i64::from(d);
                        st = St::Digits;
                    } else {
                        st = St::Done;
                    }
                }
                St::Digits => {
                    if let Some(d) = digit.filter(|&d| d < radix) {
                        mag = (mag * i64::from(radix) + i64::from(d)).min(NEG_MAG);
                    } else {
                        st = St::Done;
                    }
                }
                St::Done => {}
            }
        }
        if negative { -mag } else { mag.min(POS_MAX) }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn collect(bytes: &[u8], utf8: bool) -> Vec<Ch> {
        let mut s = CharStream::new(bytes, utf8);
        let mut out = Vec::new();
        loop {
            let (c, _) = s.next_char();
            out.push(c);
            if c == Ch::End {
                return out;
            }
        }
    }

    #[test]
    fn bom_table() {
        assert_eq!(detect_bom(b"\xFE\xFFab"), Bom::Utf16);
        assert_eq!(detect_bom(b"\xFF\xFE\x00\x00"), Bom::Utf16);
        assert_eq!(detect_bom(b"\xEF\xBB\xBFG"), Bom::Utf8);
        assert_eq!(detect_bom(b"\xEF\xBB\xBF"), Bom::Plain);
        assert_eq!(detect_bom(b"\x2B\x2F\x76x"), Bom::Unsupported);
        assert_eq!(detect_bom(b"\x00\x00\xFE\xFF"), Bom::Unsupported);
        assert_eq!(detect_bom(b"\xDD\x73\x66\x73"), Bom::Unsupported);
        assert_eq!(detect_bom(b"G A|"), Bom::Plain);
        assert_eq!(detect_bom(b""), Bom::Plain);
    }

    #[test]
    fn breaks_comments_backslashes() {
        use Ch::*;
        assert_eq!(
            collect(b"a\r\nb\rc\n", false),
            vec![Char('a'), Break, Char('b'), Break, Char('c'), Break, End]
        );
        assert_eq!(
            collect(b"a%x\\y\nb\\\\c\\%z\n", false),
            vec![Char('a'), Break, Char('b'), Char('c'), Break, End]
        );
        assert_eq!(collect(b"a\0b", false), vec![Char('a'), End]);
    }

    #[test]
    fn utf8_rules() {
        use Ch::*;
        assert_eq!(
            collect("é€".as_bytes(), true),
            vec![Char('é'), Char('€'), End]
        );
        assert_eq!(
            collect(b"\x80a", true),
            vec![Char('\u{FF80}'), Char('a'), End]
        );
        assert_eq!(collect("a😀b".as_bytes(), true), vec![Char('a'), End]);
        assert_eq!(collect(b"a\xC0\x80b", true), vec![Char('a'), End]);
        assert_eq!(collect(b"a\xE2\x82", true), vec![Char('a'), End]);
    }

    #[test]
    fn columns() {
        let mut s = CharStream::new(b"ab%c\n\\d", false);
        assert_eq!(s.next_char().1, Position { line: 1, column: 1 });
        assert_eq!(s.next_char().1, Position { line: 1, column: 2 });
        assert_eq!(s.next_char(), (Ch::Break, Position { line: 1, column: 3 }));
        assert_eq!(
            s.next_char(),
            (Ch::Char('d'), Position { line: 2, column: 1 })
        );
    }

    #[test]
    fn strtol_base_detection() {
        let v = |t: &str| CharStream::new(t.as_bytes(), false).line_strtol();
        assert_eq!(v("010\n"), 8);
        assert_eq!(v("  12abc"), 12);
        assert_eq!(v("0x1F"), 31);
        assert_eq!(v("0xg"), 0);
        assert_eq!(v("08"), 0);
        assert_eq!(v(""), 0);
        assert_eq!(v("-5"), -5);
        assert_eq!(v("99999999999999999999"), i64::from(i32::MAX));
        assert_eq!(v("-99999999999999999999"), i64::from(i32::MIN));
    }
}
