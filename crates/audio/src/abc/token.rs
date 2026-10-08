// SPDX-License-Identifier: GPL-3.0-or-later
//! Tokens (`abc-dialect.md` §4, §4.1).

use std::collections::VecDeque;

use super::chars::{Ch, CharStream, Position, is_digit, is_letter, is_space};

/// Largest value of a number token: the 32-bit `long` maximum (§4.1).
pub(crate) const NUMBER_MAX: u32 = i32::MAX as u32;

/// One token of §4.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(crate) enum Tok {
    End,
    Break,
    Space,
    Letter(char),
    Number(u32),
    Bar,
    DoubleBar,
    ThinThick,
    RepeatStart,
    FirstEnding,
    RepeatEnd,
    SecondEnding,
    DoubleRepeat,
    ThickThin,
    EndingOne,
    EndingTwo,
    Colon,
    ChordOpen,
    ChordClose,
    Open,
    Close,
    GraceOpen,
    GraceClose,
    Quote,
    Apostrophe,
    Comma,
    Plus,
    Hash,
    Tilde,
    Dot,
    Caret,
    Underscore,
    Equals,
    Slash,
    Hyphen,
    Less,
    Greater,
    Illegal(char),
}

/// A token and the position of its first character.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(crate) struct Token {
    pub(crate) tok: Tok,
    pub(crate) pos: Position,
}

/// Tokenizer with a small look-ahead buffer. The reader never looks past a line break, so
/// the buffer holds at most three tokens and is empty after a line break has been taken.
#[derive(Debug)]
pub(crate) struct Lexer<'a> {
    chars: CharStream<'a>,
    buf: VecDeque<Token>,
}

impl<'a> Lexer<'a> {
    pub(crate) fn new(chars: CharStream<'a>) -> Self {
        Lexer {
            chars,
            buf: VecDeque::with_capacity(4),
        }
    }

    /// The token `n` places ahead (0 = next).
    pub(crate) fn peek_at(&mut self, n: usize) -> Token {
        while self.buf.len() <= n {
            let t = self.lex();
            self.buf.push_back(t);
        }
        self.buf[n]
    }

    /// The next token's kind.
    pub(crate) fn peek(&mut self) -> Tok {
        self.peek_at(0).tok
    }

    /// The next token's position.
    pub(crate) fn peek_pos(&mut self) -> Position {
        self.peek_at(0).pos
    }

    /// Takes the next token.
    pub(crate) fn next_token(&mut self) -> Token {
        match self.buf.pop_front() {
            Some(t) => t,
            None => self.lex(),
        }
    }

    /// Direct access to the characters, for readers that work below the token level (titles,
    /// skipped parts). Only valid when no token is buffered.
    pub(crate) fn chars(&mut self) -> &mut CharStream<'a> {
        debug_assert!(self.buf.is_empty(), "token look-ahead past a line break");
        self.buf.clear();
        &mut self.chars
    }

    fn lex(&mut self) -> Token {
        let (ch, pos) = self.chars.next_char();
        let c = match ch {
            Ch::End => return Token { tok: Tok::End, pos },
            Ch::Break => {
                return Token {
                    tok: Tok::Break,
                    pos,
                };
            }
            Ch::Char(c) => c,
        };
        let tok = if is_space(c) {
            while let (Ch::Char(n), _) = self.chars.peek() {
                if !is_space(n) {
                    break;
                }
                self.chars.next_char();
            }
            Tok::Space
        } else if is_letter(c) {
            Tok::Letter(c)
        } else if is_digit(c) {
            self.number(c)
        } else {
            self.punctuation(c)
        };
        Token { tok, pos }
    }

    /// A run of digits (§4.1, Tessera's choice): the whole run is converted, saturating at
    /// 2,147,483,647, with no buffer and no length limit.
    fn number(&mut self, first: char) -> Tok {
        let mut value = u64::from(first as u32 - u32::from(b'0'));
        while let (Ch::Char(c), _) = self.chars.peek() {
            if !is_digit(c) {
                break;
            }
            self.chars.next_char();
            let d = u64::from(c as u32 - u32::from(b'0'));
            value = (value * 10 + d).min(u64::from(NUMBER_MAX));
        }
        Tok::Number(u32::try_from(value).unwrap_or(NUMBER_MAX))
    }

    fn follows(&mut self, c: char) -> bool {
        if self.chars.peek().0 == Ch::Char(c) {
            self.chars.next_char();
            true
        } else {
            false
        }
    }

    fn punctuation(&mut self, c: char) -> Tok {
        match c {
            '|' => {
                if self.follows('|') {
                    Tok::DoubleBar
                } else if self.follows(']') {
                    Tok::ThinThick
                } else if self.follows(':') {
                    Tok::RepeatStart
                } else if self.follows('1') {
                    Tok::FirstEnding
                } else {
                    Tok::Bar
                }
            }
            '[' => {
                if self.follows('|') {
                    Tok::ThickThin
                } else if self.follows('1') {
                    Tok::EndingOne
                } else if self.follows('2') {
                    Tok::EndingTwo
                } else {
                    Tok::ChordOpen
                }
            }
            ':' => {
                if self.follows(':') {
                    Tok::DoubleRepeat
                } else if self.follows('|') {
                    if self.follows('2') {
                        Tok::SecondEnding
                    } else {
                        Tok::RepeatEnd
                    }
                } else {
                    Tok::Colon
                }
            }
            ']' => Tok::ChordClose,
            '(' => Tok::Open,
            ')' => Tok::Close,
            '{' => Tok::GraceOpen,
            '}' => Tok::GraceClose,
            '"' => Tok::Quote,
            '\'' => Tok::Apostrophe,
            ',' => Tok::Comma,
            '+' => Tok::Plus,
            '#' => Tok::Hash,
            '~' => Tok::Tilde,
            '.' => Tok::Dot,
            '^' => Tok::Caret,
            '_' => Tok::Underscore,
            '=' => Tok::Equals,
            '/' => Tok::Slash,
            '-' => Tok::Hyphen,
            '<' => Tok::Less,
            '>' => Tok::Greater,
            other => Tok::Illegal(other),
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn toks(text: &str) -> Vec<Tok> {
        let mut lx = Lexer::new(CharStream::new(text.as_bytes(), false));
        let mut out = Vec::new();
        loop {
            let t = lx.next_token().tok;
            if t == Tok::End {
                return out;
            }
            out.push(t);
        }
    }

    #[test]
    fn combinations() {
        use Tok::*;
        assert_eq!(toks("|2"), vec![Bar, Number(2)]);
        assert_eq!(toks(":|1"), vec![RepeatEnd, Number(1)]);
        assert_eq!(toks("[3"), vec![ChordOpen, Number(3)]);
        assert_eq!(toks("||1"), vec![DoubleBar, Number(1)]);
        assert_eq!(toks(":|:"), vec![RepeatEnd, Colon]);
        assert_eq!(toks(":|2"), vec![SecondEnding]);
        assert_eq!(toks("::|"), vec![DoubleRepeat, Bar]);
        assert_eq!(toks("[|]"), vec![ThickThin, ChordClose]);
        assert_eq!(
            toks("|]|:|1[1[2"),
            vec![ThinThick, RepeatStart, FirstEnding, EndingOne, EndingTwo]
        );
    }

    #[test]
    fn numbers_spaces_letters() {
        use Tok::*;
        assert_eq!(
            toks("Am  \t1\\2x"),
            vec![Letter('A'), Letter('m'), Space, Number(12), Letter('x')]
        );
        assert_eq!(toks("99999999999"), vec![Number(NUMBER_MAX)]);
        let long = "0".repeat(500) + "7";
        assert_eq!(toks(&long), vec![Number(7)]);
        assert_eq!(toks("!$`"), vec![Illegal('!'), Illegal('$'), Illegal('`')]);
    }
}
