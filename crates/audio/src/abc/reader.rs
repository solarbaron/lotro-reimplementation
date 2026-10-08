// SPDX-License-Identifier: GPL-3.0-or-later
//! The reader: lines, fields, part selection and music elements (`abc-dialect.md` §5–§7).
//! Produces the part's items in reading order and the part-wide timing values; timing and
//! events are the sequencer's job (§8–§9, `sequence.rs`).

use std::collections::HashMap;

use super::chars::{CharStream, Position};
use super::error::{AbcError, AbcErrorKind as K};
use super::token::{Lexer, Tok};
use super::{MAX_EVENTS, PartRequest, ReadOutcome};

/// Longest title kept, in characters (after leading white space). Longer titles are cut;
/// only the first 63 characters are ever shown (§6.5).
pub(crate) const TITLE_KEEP_CHARS: usize = 1024;

/// Most entries in a chord after duplicates are dropped (§7.7).
const MAX_CHORD_ENTRIES: usize = 6;

/// Natural pitches of the note letters C D E F G A B (upper case; lower case is +12),
/// §7.3.
const NATURAL: [i64; 7] = [60, 62, 64, 65, 67, 69, 71];

/// Pitch classes of the note letters C D E F G A B, for keys (§6.4 step 5).
const PITCH_CLASS: [i64; 7] = [0, 2, 4, 5, 7, 9, 11];

/// Key-signature table (§6.4.1): rows by signature index, columns C D E F G A B.
const KEY_TABLE: [[i64; 7]; 12] = [
    [0, 0, 0, 0, 0, 0, 0],
    [0, -1, -1, 0, -1, -1, -1],
    [1, 0, 0, 1, 0, 0, 0],
    [0, 0, -1, 0, 0, -1, -1],
    [1, 1, 0, 1, 1, 0, 0],
    [0, 0, 0, 0, 0, 0, -1],
    [1, 1, 1, 1, 1, 1, 0],
    [0, 0, 0, 1, 0, 0, 0],
    [0, -1, -1, 0, 0, -1, -1],
    [1, 0, 0, 1, 1, 0, 0],
    [0, 0, -1, 0, 0, 0, -1],
    [1, 1, 0, 1, 1, 1, 0],
];

/// Mode prefixes and their offsets (§6.4 step 4).
const MODES: [(&str, i64); 9] = [
    ("maj", 0),
    ("ion", 0),
    ("dor", 2),
    ("phr", 4),
    ("lyd", 5),
    ("mix", 7),
    ("min", 9),
    ("aeo", 9),
    ("loc", 11),
];

/// Volume-mark names and values (§7.11).
const VOLUMES: [(&str, u8); 10] = [
    ("pppp", 30),
    ("ppp", 30),
    ("pp", 45),
    ("p", 60),
    ("mp", 75),
    ("mf", 90),
    ("f", 105),
    ("ff", 120),
    ("fff", 127),
    ("ffff", 127),
];

/// Default volume (§5.4).
pub(crate) const DEFAULT_VOLUME: u8 = 90;

/// One note or rest as read (a single element or a chord entry).
#[derive(Debug, Clone)]
pub(crate) struct NoteRec {
    /// Pitch 48–84, or `None` for a rest.
    pub(crate) pitch: Option<u8>,
    /// Written length after broken-rhythm and tuplet factors (§8.2 step 3).
    pub(crate) len: f64,
    /// Tied to a later note of the same pitch (§7.6).
    pub(crate) tied: bool,
    /// Volume in force when the note was read (§7.11).
    pub(crate) volume: u8,
    /// Position of the note's letter (or rest letter).
    pub(crate) pos: Position,
}

/// One event of the part (§7.12): a single note or rest, a chord, or an empty chord.
#[derive(Debug, Clone)]
pub(crate) enum Item {
    Single(NoteRec),
    Chord(Vec<NoteRec>),
    Empty,
}

/// What reading produced.
#[derive(Debug)]
pub(crate) struct Song {
    pub(crate) items: Vec<Item>,
    pub(crate) title: Option<String>,
    pub(crate) part: Option<i32>,
    pub(crate) meter_num: u32,
    pub(crate) meter_den: u32,
    pub(crate) unit: f64,
    pub(crate) tempo: u32,
    pub(crate) outcome: ReadOutcome,
}

/// Result of one element (§7.1).
enum Elem {
    /// A complete element.
    Done,
    /// A field: complete, and it took the rest of its line, so the round ends.
    Field,
    /// The selected part ended at the next X: line; reading stops.
    Stop,
    /// The element could not be read (prefixes, if any, are lost). `empty_chord` marks the
    /// empty chord `[]`, which is added to the part but counts as failed (§7.7 rule 5).
    Failed { empty_chord: bool },
}

/// Result of a note group (prefixes + note, rest or chord + broken rhythm).
enum Group {
    Read,
    Failed { empty_chord: bool },
}

struct Reader<'a> {
    lx: Lexer<'a>,
    request: PartRequest,
    selected: bool,
    part: Option<i32>,
    meter_num: u32,
    meter_den: u32,
    unit: f64,
    tempo: u32,
    key: [i64; 7],
    bar_acc: HashMap<(usize, i64), i64>,
    volume: u8,
    title: Option<String>,
    pending: Option<f64>,
    items: Vec<Item>,
    /// The opening quote or brace of a prefix that was never closed (§7.10): it swallowed
    /// the rest of the file, so reaching the end truncates the part (§2.2).
    unclosed: Option<Position>,
}

/// Reads the part selected by `request` from decoded characters.
pub(crate) fn read(chars: CharStream<'_>, request: PartRequest) -> Result<Song, AbcError> {
    let mut r = Reader {
        lx: Lexer::new(chars),
        request,
        selected: false,
        part: None,
        meter_num: 4,
        meter_den: 4,
        unit: 0.125,
        tempo: 120,
        key: [0; 7],
        bar_acc: HashMap::new(),
        volume: DEFAULT_VOLUME,
        title: None,
        pending: None,
        items: Vec::new(),
        unclosed: None,
    };
    let outcome = r.run()?;
    Ok(Song {
        items: r.items,
        title: r.title,
        part: r.part,
        meter_num: r.meter_num,
        meter_den: r.meter_den,
        unit: r.unit,
        tempo: r.tempo,
        outcome,
    })
}

fn letter_index(c: char) -> Option<usize> {
    match c.to_ascii_uppercase() {
        'C' => Some(0),
        'D' => Some(1),
        'E' => Some(2),
        'F' => Some(3),
        'G' => Some(4),
        'A' => Some(5),
        'B' => Some(6),
        _ => None,
    }
}

fn is_note_letter(t: Tok) -> bool {
    matches!(t, Tok::Letter(c) if c.is_ascii() && letter_index(c).is_some())
}

fn is_rest_letter(t: Tok) -> bool {
    matches!(t, Tok::Letter('z' | 'x'))
}

fn is_note_start(t: Tok) -> bool {
    is_note_letter(t) || matches!(t, Tok::Caret | Tok::Underscore | Tok::Equals)
}

impl Reader<'_> {
    /// Rounds (§7.1).
    fn run(&mut self) -> Result<ReadOutcome, AbcError> {
        loop {
            // Rule 1: line breaks are skipped; the end completes the part (unless an unclosed
            // quote or grace group after other music swallowed the rest of the file, which
            // truncates it, §2.2); an illegal character truncates it.
            while self.lx.peek() == Tok::Break {
                self.lx.next_token();
            }
            match self.lx.peek() {
                Tok::End => {
                    return Ok(match self.unclosed {
                        Some(at) => ReadOutcome::Truncated { at },
                        None => ReadOutcome::Complete,
                    });
                }
                Tok::Illegal(_) => {
                    return Ok(ReadOutcome::Truncated {
                        at: self.lx.peek_pos(),
                    });
                }
                _ => {}
            }
            let mut read_any = false;
            loop {
                match self.element()? {
                    Elem::Done => read_any = true,
                    Elem::Field => break,
                    Elem::Stop => return Ok(ReadOutcome::Complete),
                    Elem::Failed { empty_chord } => {
                        if !read_any {
                            return Err(self.first_element_failed(empty_chord));
                        }
                        break;
                    }
                }
            }
        }
    }

    /// Rule 2 of §7.1: the first element of a round failed.
    fn first_element_failed(&mut self, empty_chord: bool) -> AbcError {
        let t = self.lx.peek_at(0);
        if !empty_chord && let Tok::Letter(_) = t.tok {
            self.lx.next_token();
            return AbcError::at(K::ColonExpected, self.lx.peek_pos());
        }
        AbcError::at(K::UnexpectedSymbol, t.pos)
    }

    fn element(&mut self) -> Result<Elem, AbcError> {
        let t = self.lx.peek();
        Ok(match t {
            Tok::Space | Tok::Close => {
                self.lx.next_token();
                Elem::Done
            }
            Tok::Open => {
                self.lx.next_token();
                if let Tok::Number(_) = self.lx.peek() {
                    self.tuplet()?;
                }
                Elem::Done
            }
            Tok::Bar
            | Tok::DoubleBar
            | Tok::ThinThick
            | Tok::RepeatStart
            | Tok::RepeatEnd
            | Tok::DoubleRepeat
            | Tok::ThickThin => {
                self.lx.next_token();
                self.bar_acc.clear();
                Elem::Done
            }
            Tok::FirstEnding | Tok::SecondEnding | Tok::EndingOne | Tok::EndingTwo => {
                self.lx.next_token();
                Elem::Done
            }
            Tok::Plus => {
                self.volume_mark()?;
                Elem::Done
            }
            Tok::Letter(c) if self.lx.peek_at(1).tok == Tok::Colon => self.field(c)?,
            _ => match self.note_group()? {
                Group::Read => Elem::Done,
                Group::Failed { empty_chord } => Elem::Failed { empty_chord },
            },
        })
    }

    // ----- fields (§5, §6) -----

    fn discard_line(&mut self) {
        loop {
            match self.lx.next_token().tok {
                Tok::Break | Tok::End => return,
                _ => {}
            }
        }
    }

    fn field(&mut self, letter: char) -> Result<Elem, AbcError> {
        self.lx.next_token(); // letter
        self.lx.next_token(); // colon
        let had_space = if self.lx.peek() == Tok::Space {
            self.lx.next_token();
            true
        } else {
            false
        };
        match letter.to_ascii_uppercase() {
            'K' => self.key()?,
            'L' => self.unit_length()?,
            'M' => self.meter()?,
            'Q' => self.tempo()?,
            'T' => {
                self.title(had_space);
                return Ok(Elem::Field);
            }
            'X' => return self.x_field(),
            _ => {}
        }
        self.discard_line();
        Ok(Elem::Field)
    }

    /// X: (§5.3).
    fn x_field(&mut self) -> Result<Elem, AbcError> {
        let t = self.lx.peek_at(0);
        let Tok::Number(n) = t.tok else {
            return Err(AbcError::at(K::NumberExpected, t.pos));
        };
        self.lx.next_token();
        if self.selected {
            return Ok(Elem::Stop);
        }
        let value = i64::from(n);
        let wanted = match self.request {
            PartRequest::Unspecified => None,
            PartRequest::Number(r) => Some(i64::from(r)),
        };
        self.discard_line();
        match wanted {
            Some(r) if r != value => {
                let found = self.skip_to_part(r)?;
                self.part = Some(found);
            }
            _ => self.part = i32::try_from(value).ok(),
        }
        self.selected = true;
        Ok(Elem::Field)
    }

    /// Skips lines until an X: line whose value (C base detection) equals `wanted`
    /// (§5.3 step 5).
    fn skip_to_part(&mut self, wanted: i64) -> Result<i32, AbcError> {
        let chars = self.lx.chars();
        loop {
            chars.skip_spaces();
            let (c, _) = chars.next_char();
            match c {
                super::chars::Ch::End => return Err(AbcError::bare(K::PartNotFound)),
                super::chars::Ch::Break => continue,
                super::chars::Ch::Char(c) => {
                    let field =
                        super::chars::is_letter(c) && chars.peek().0 == super::chars::Ch::Char(':');
                    if field && (c == 'X' || c == 'x') {
                        chars.next_char();
                        let v = chars.line_strtol();
                        if v == wanted {
                            return Ok(i32::try_from(v).unwrap_or(i32::MAX));
                        }
                    } else {
                        chars.skip_line();
                    }
                }
            }
        }
    }

    fn expect_number(&mut self, kind: K) -> Result<u32, AbcError> {
        let t = self.lx.peek_at(0);
        match t.tok {
            Tok::Number(n) => {
                self.lx.next_token();
                Ok(n)
            }
            _ => Err(AbcError::at(kind, t.pos)),
        }
    }

    fn expect(&mut self, tok: Tok, kind: K) -> Result<(), AbcError> {
        let t = self.lx.peek_at(0);
        if t.tok == tok {
            self.lx.next_token();
            Ok(())
        } else {
            Err(AbcError::at(kind, t.pos))
        }
    }

    /// M: (§6.1).
    fn meter(&mut self) -> Result<(), AbcError> {
        let t = self.lx.peek_at(0);
        let (num, den) = match t.tok {
            Tok::Letter('C') => {
                self.lx.next_token();
                if self.lx.peek() == Tok::Bar {
                    self.lx.next_token();
                    (2, 2)
                } else {
                    (4, 4)
                }
            }
            Tok::Number(n) => {
                self.lx.next_token();
                self.expect(Tok::Slash, K::MeterMalformed)?;
                (n, self.expect_number(K::MeterMalformed)?)
            }
            _ => return Err(AbcError::at(K::MeterMalformed, t.pos)),
        };
        self.meter_num = num;
        self.meter_den = den;
        if den != 0 {
            self.unit = if f64::from(num) / f64::from(den) < 0.75 {
                0.0625
            } else {
                0.125
            };
        }
        Ok(())
    }

    /// L: (§6.2).
    fn unit_length(&mut self) -> Result<(), AbcError> {
        let num = self.expect_number(K::UnitLengthMalformed)?;
        self.expect(Tok::Slash, K::UnitLengthMalformed)?;
        let t = self.lx.peek_at(0);
        let den = self.expect_number(K::UnitLengthMalformed)?;
        if den == 0 {
            return Err(AbcError::at(K::UnitLengthMalformed, t.pos));
        }
        self.unit = f64::from(num) / f64::from(den);
        Ok(())
    }

    /// Q: (§6.3).
    fn tempo(&mut self) -> Result<(), AbcError> {
        let t = self.lx.peek_at(0);
        self.tempo = match t.tok {
            Tok::Number(n) => {
                self.lx.next_token();
                if self.lx.peek() == Tok::Slash {
                    self.lx.next_token();
                    self.expect_number(K::TempoMalformed)?;
                    self.expect(Tok::Equals, K::TempoMalformed)?;
                    self.expect_number(K::TempoMalformed)?
                } else {
                    n
                }
            }
            Tok::Letter('C') => {
                self.lx.next_token();
                self.length()?;
                self.expect(Tok::Equals, K::TempoMalformed)?;
                self.expect_number(K::TempoMalformed)?
            }
            _ => return Err(AbcError::at(K::TempoMalformed, t.pos)),
        };
        Ok(())
    }

    /// K: (§6.4).
    fn key(&mut self) -> Result<(), AbcError> {
        let t = self.lx.peek_at(0);
        let tonic = match t.tok {
            Tok::Letter('H') => {
                self.lx.next_token();
                let f = self.lx.peek_at(0);
                let kind = if matches!(f.tok, Tok::Letter('P' | 'p')) {
                    K::KeyWordUnknown
                } else {
                    K::KeyMalformed
                };
                return Err(AbcError::at(kind, f.pos));
            }
            Tok::Letter(c) if is_note_letter(t.tok) => {
                self.lx.next_token();
                letter_index(c).map_or(0, |i| PITCH_CLASS[i])
            }
            _ => return Err(AbcError::at(K::KeyMalformed, t.pos)),
        };
        let mut pc = tonic;
        match self.lx.peek() {
            Tok::Hash => {
                self.lx.next_token();
                pc += 1;
            }
            Tok::Letter('b') => {
                self.lx.next_token();
                pc -= 1;
            }
            _ => {}
        }
        // Mode: directly, or after one space token.
        if self.lx.peek() == Tok::Space && matches!(self.lx.peek_at(1).tok, Tok::Letter(_)) {
            self.lx.next_token();
        }
        let mut offset = 0;
        if let Tok::Letter(a) = self.lx.peek() {
            let first = self.lx.peek_at(0).pos;
            if let Tok::Letter(b) = self.lx.peek_at(1).tok
                && let Tok::Letter(c) = self.lx.peek_at(2).tok
            {
                let word: String = [a, b, c].iter().map(|ch| ch.to_ascii_lowercase()).collect();
                let Some(&(_, off)) = MODES.iter().find(|(m, _)| *m == word) else {
                    return Err(AbcError::at(K::KeyWordUnknown, first));
                };
                offset = off;
                while let Tok::Letter(_) = self.lx.peek() {
                    self.lx.next_token();
                }
            }
            // Fewer than three letters: no mode (major); the letters are trailing text.
        }
        let index = usize::try_from((pc - offset).rem_euclid(12)).unwrap_or(0);
        self.key = KEY_TABLE[index];
        // Explicit accidentals: [space] accidental note-letter, repeatedly.
        loop {
            if self.lx.peek() == Tok::Space {
                self.lx.next_token();
            }
            let Some(acc) = self.accidental_run() else {
                break;
            };
            match self.lx.peek() {
                Tok::Letter(c) if is_note_letter(Tok::Letter(c)) => {
                    self.lx.next_token();
                    if let Some(i) = letter_index(c) {
                        self.key[i] = acc;
                    }
                }
                _ => break,
            }
        }
        Ok(())
    }

    /// T: (§6.5). Reads the rest of the line itself.
    fn title(&mut self, had_space: bool) {
        let mut text = String::new();
        let mut kept = 0usize;
        let mut push = |c: char, text: &mut String| {
            if text.is_empty() && matches!(c, ' ' | '\t' | '\r' | '\n' | '\u{0C}') {
                return;
            }
            if kept < TITLE_KEEP_CHARS {
                text.push(c);
                kept += 1;
            }
        };
        if !had_space {
            match self.lx.next_token().tok {
                Tok::Letter(c) => push(c, &mut text),
                Tok::Break | Tok::End => {
                    self.set_title(text);
                    return;
                }
                _ => {} // a leading number or punctuation mark is lost
            }
        }
        let chars = self.lx.chars();
        while let super::chars::Ch::Char(c) = chars.next_char().0 {
            push(c, &mut text);
        }
        self.set_title(text);
    }

    fn set_title(&mut self, text: String) {
        let trimmed = text.trim_end_matches([' ', '\t', '\r', '\n', '\u{0C}']);
        if self.title.is_none() && !trimmed.is_empty() {
            self.title = Some(trimmed.to_owned());
        }
    }

    // ----- music elements (§7) -----

    /// `+name+` (§7.11).
    fn volume_mark(&mut self) -> Result<(), AbcError> {
        self.lx.next_token(); // '+'
        let first = self.lx.peek_at(0);
        if !matches!(first.tok, Tok::Letter(_)) {
            return Err(AbcError::at(K::UnknownVolumeMark, first.pos));
        }
        let mut name = String::new();
        let mut too_long = false;
        while let Tok::Letter(c) = self.lx.peek() {
            self.lx.next_token();
            if name.len() < 4 && c.is_ascii() {
                name.push(c.to_ascii_lowercase());
            } else {
                too_long = true;
            }
        }
        self.expect(Tok::Plus, K::ClosingPlusExpected)?;
        let value = (!too_long)
            .then(|| VOLUMES.iter().find(|(n, _)| *n == name).map(|&(_, v)| v))
            .flatten();
        match value {
            Some(v) => {
                self.volume = v;
                Ok(())
            }
            None => Err(AbcError::at(K::UnknownVolumeMark, first.pos)),
        }
    }

    /// Prefixes, then a note, rest or chord, then a broken rhythm (§7.3–§7.10).
    fn note_group(&mut self) -> Result<Group, AbcError> {
        for (open, close) in [(Tok::Quote, Tok::Quote), (Tok::GraceOpen, Tok::GraceClose)] {
            let t = self.lx.peek_at(0);
            if t.tok == open {
                self.lx.next_token();
                if !self.skip_past(close) && self.unclosed.is_none() {
                    self.unclosed = Some(t.pos);
                }
            }
        }
        if matches!(
            self.lx.peek(),
            Tok::Tilde | Tok::Dot | Tok::Letter('u' | 'v')
        ) {
            self.lx.next_token();
        }
        let t = self.lx.peek();
        if t == Tok::ChordOpen {
            return self.chord();
        }
        if is_note_start(t) || is_rest_letter(t) {
            let mut note = self.entry()?;
            if let Some(p) = self.pending.take() {
                note.len *= p;
            }
            self.broken_rhythm(&mut note)?;
            self.push_item(Item::Single(note))?;
            return Ok(Group::Read);
        }
        Ok(Group::Failed { empty_chord: false })
    }

    /// Skips tokens up to and including `close`, or to the end (§7.10). Returns `false` if
    /// the end came first. Each token is consumed once, so the scan is linear (§14.2).
    fn skip_past(&mut self, close: Tok) -> bool {
        loop {
            match self.lx.peek() {
                Tok::End => return false,
                t => {
                    self.lx.next_token();
                    if t == close {
                        return true;
                    }
                }
            }
        }
    }

    /// One to three `>` or `<` after a single note or rest (§7.8).
    fn broken_rhythm(&mut self, note: &mut NoteRec) -> Result<(), AbcError> {
        let t = self.lx.peek_at(0);
        if !matches!(t.tok, Tok::Greater | Tok::Less) {
            return Ok(());
        }
        let mut n = 0u32;
        while self.lx.peek() == t.tok {
            self.lx.next_token();
            n = n.saturating_add(1);
        }
        if n > 3 {
            return Err(AbcError::at(K::BrokenRhythmTooLong, t.pos));
        }
        if self.pending.is_some() {
            return Err(AbcError::at(K::BrokenRhythmMisplaced, t.pos));
        }
        let f = f64::from(1u32 << n);
        let long = (2.0 * f - 1.0) / f;
        let short = 1.0 / f;
        if t.tok == Tok::Greater {
            note.len *= long;
            self.pending = Some(short);
        } else {
            note.len *= short;
            self.pending = Some(long);
        }
        Ok(())
    }

    /// A chord (§7.7).
    fn chord(&mut self) -> Result<Group, AbcError> {
        self.lx.next_token(); // '['
        let mut entries: Vec<NoteRec> = Vec::new();
        let mut first = true;
        loop {
            let t = self.lx.peek_at(0);
            if t.tok == Tok::ChordClose {
                self.lx.next_token();
                break;
            }
            if !(is_note_start(t.tok) || is_rest_letter(t.tok)) {
                return Err(AbcError::at(K::ChordNotClosed, t.pos));
            }
            let mut e = self.entry()?;
            if first {
                first = false;
                if let Some(p) = self.pending.take() {
                    e.len *= p;
                }
            }
            let duplicate = entries.iter().any(|x| x.pitch == e.pitch);
            if !duplicate {
                if entries.len() >= MAX_CHORD_ENTRIES {
                    return Err(AbcError::at(K::TooManyNotesInChord, e.pos));
                }
                entries.push(e);
            }
        }
        let after = self.lx.peek_at(0);
        if matches!(after.tok, Tok::Greater | Tok::Less) {
            return Err(AbcError::at(K::BrokenRhythmMisplaced, after.pos));
        }
        if entries.is_empty() {
            self.push_item(Item::Empty)?;
            return Ok(Group::Failed { empty_chord: true });
        }
        self.push_item(Item::Chord(entries))?;
        Ok(Group::Read)
    }

    fn push_item(&mut self, item: Item) -> Result<(), AbcError> {
        if self.items.len() >= MAX_EVENTS {
            // The event-limit notice has no position (§7.12, §10.4).
            return Err(AbcError::bare(K::TooManyEvents));
        }
        self.items.push(item);
        Ok(())
    }

    /// `^…`, `_…` or `=`: the alteration in semitones, or `None` if no accidental follows.
    fn accidental_run(&mut self) -> Option<i64> {
        let t = self.lx.peek();
        let step = match t {
            Tok::Caret => 1,
            Tok::Underscore => -1,
            Tok::Equals => {
                self.lx.next_token();
                return Some(0);
            }
            _ => return None,
        };
        let mut v: i64 = 0;
        while self.lx.peek() == t {
            self.lx.next_token();
            v = v.saturating_add(step);
        }
        Some(v)
    }

    /// A note or rest with its length and tie (§7.3–§7.6), as a single element or a chord
    /// entry. The caller has checked that one starts here.
    fn entry(&mut self) -> Result<NoteRec, AbcError> {
        let start = self.lx.peek_at(0);
        if is_rest_letter(start.tok) {
            self.lx.next_token();
            let len = self.length()?;
            if self.lx.peek() == Tok::Hyphen {
                self.lx.next_token(); // a tie on a rest has no effect
            }
            return Ok(NoteRec {
                pitch: None,
                len,
                tied: false,
                volume: self.volume,
                pos: start.pos,
            });
        }
        let acc = self.accidental_run();
        let lt = self.lx.peek_at(0);
        let Tok::Letter(letter) = lt.tok else {
            return Err(AbcError::at(K::UnexpectedSymbol, lt.pos));
        };
        let Some(idx) = letter_index(letter).filter(|_| letter.is_ascii()) else {
            return Err(AbcError::at(K::UnexpectedSymbol, lt.pos));
        };
        self.lx.next_token();
        let natural = NATURAL[idx] + if letter.is_ascii_lowercase() { 12 } else { 0 };
        let mut marks: i64 = 0;
        match self.lx.peek() {
            Tok::Apostrophe => {
                while self.lx.peek() == Tok::Apostrophe {
                    self.lx.next_token();
                    marks = marks.saturating_add(1);
                }
            }
            Tok::Comma => {
                while self.lx.peek() == Tok::Comma {
                    self.lx.next_token();
                    marks = marks.saturating_sub(1);
                }
            }
            _ => {}
        }
        let base = natural.saturating_add(marks.saturating_mul(12));
        let alteration = match acc {
            Some(a) => {
                self.bar_acc.insert((idx, base), a);
                a
            }
            None => match self.bar_acc.get(&(idx, base)) {
                Some(&a) => a,
                None => self.key[idx],
            },
        };
        let pitch = base.saturating_add(alteration);
        if pitch < 48 {
            return Err(AbcError::at(K::NoteTooLow, self.lx.peek_pos()));
        }
        if pitch > 84 {
            return Err(AbcError::at(K::NoteTooHigh, self.lx.peek_pos()));
        }
        let len = self.length()?;
        let tied = if self.lx.peek() == Tok::Hyphen {
            self.lx.next_token();
            true
        } else {
            false
        };
        Ok(NoteRec {
            pitch: u8::try_from(pitch).ok(),
            len,
            tied,
            volume: self.volume,
            pos: lt.pos,
        })
    }

    /// A length (§7.5): written length as a double.
    fn length(&mut self) -> Result<f64, AbcError> {
        let mut num: u32 = 1;
        if let Tok::Number(n) = self.lx.peek() {
            self.lx.next_token();
            num = n;
        }
        let mut den: u32 = 1;
        if self.lx.peek() == Tok::Slash {
            self.lx.next_token();
            let t = self.lx.peek_at(0);
            match t.tok {
                Tok::Number(d) => {
                    self.lx.next_token();
                    if d == 0 {
                        return Err(AbcError::at(K::ZeroLengthDenominator, t.pos));
                    }
                    den = d;
                }
                Tok::Slash => {
                    self.lx.next_token();
                    if num == 1 {
                        den = 4;
                    }
                }
                _ => {
                    if num == 1 {
                        den = 2;
                    }
                }
            }
        }
        Ok(f64::from(num) / f64::from(den))
    }

    /// A tuplet after `(` (§7.9).
    fn tuplet(&mut self) -> Result<(), AbcError> {
        let p = self.tuplet_number()?;
        let mut q = None;
        let mut r = None;
        match self.lx.peek() {
            Tok::Colon => {
                self.lx.next_token();
                if let Tok::Number(_) = self.lx.peek() {
                    q = Some(self.tuplet_number()?);
                }
                if self.lx.peek() == Tok::Colon {
                    self.lx.next_token();
                    if let Tok::Number(_) = self.lx.peek() {
                        r = Some(self.tuplet_number()?);
                    }
                }
            }
            Tok::DoubleRepeat => {
                self.lx.next_token();
                if let Tok::Number(_) = self.lx.peek() {
                    r = Some(self.tuplet_number()?);
                }
            }
            _ => {}
        }
        let q = q.unwrap_or(match p {
            2 | 4 | 8 => 3,
            3 | 6 => 2,
            _ => {
                if self.meter_num.is_multiple_of(3) {
                    3
                } else {
                    2
                }
            }
        });
        let r = r.unwrap_or(p);
        let factor = f64::from(q) / f64::from(p);
        for _ in 0..r {
            let start = self.lx.peek_pos();
            match self.note_group()? {
                Group::Read => {}
                Group::Failed { empty_chord } => {
                    // Reported at the token where the problem was seen (§10.2): after any
                    // prefixes, the token that is not a note, rest or chord; an empty chord
                    // at its `[` (Tessera's choice; abc-dialect.md §7.9 OPEN).
                    let at = if empty_chord {
                        start
                    } else {
                        self.lx.peek_pos()
                    };
                    return Err(AbcError::at(K::NonNoteInTuplet, at));
                }
            }
            match self.items.last_mut() {
                Some(Item::Single(n)) => n.len *= factor,
                Some(Item::Chord(es)) => es.iter_mut().for_each(|e| e.len *= factor),
                _ => {}
            }
        }
        Ok(())
    }

    fn tuplet_number(&mut self) -> Result<u32, AbcError> {
        let t = self.lx.next_token();
        match t.tok {
            Tok::Number(n) if (2..=9).contains(&n) => Ok(n),
            _ => Err(AbcError::at(K::TupletNumberOutOfRange, t.pos)),
        }
    }
}
