// SPDX-License-Identifier: GPL-3.0-or-later
//! Error kinds, message references and notices (`abc-dialect.md` §10).
//!
//! The text of a message is never part of this crate: an error or notice names a
//! string-table entry (table key + token) and its arguments by argument ID, and the client
//! renders the entry from the user's own string tables at run time (§10.3). The `Display`
//! implementations are short diagnostics in Tessera's own words, for logs and for the
//! fallback when the table or entry is missing.

use std::fmt;
use std::path::PathBuf;

use super::chars::Position;

/// Key of the player-music message table in the named-string-table DID map
/// (`abc-dialect.md` §10.3; label `UI_Music` in the observed install). The table itself is
/// resolved at run time through that map.
pub const MESSAGE_TABLE_KEY: u32 = 0x1000_0069;

/// Argument ID of the line number (§10.2).
pub const ARG_LINE: u32 = 0x0005_0E25;
/// Argument ID of the column number (§10.2).
pub const ARG_COLUMN: u32 = 0x0484_1A1E;
/// Argument ID of the file name (§10.2).
pub const ARG_FILE_NAME: u32 = 0x0E0A_22B5;
/// Argument ID of the title in the "now playing" notice (§10.4).
pub const ARG_TITLE: u32 = 0x0058_E905;

/// Longest title passed to the "now playing" notice, in characters (§6.5, §10.4).
pub const NOTICE_TITLE_MAX_CHARS: usize = 63;

/// Why a part was refused (`abc-dialect.md` §10.2, plus the event limit of §7.12 and
/// Tessera's own file errors).
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum AbcErrorKind {
    /// UTF-16 (or UTF-32 LE) byte-order mark (§3.2).
    EncodingNotSupported,
    /// No `.abc` or `.txt` file, or an unsupported byte-order mark (§3.1, §3.2).
    FileNotFound,
    /// Bad M: value (§6.1).
    MeterMalformed,
    /// Bad Q: value (§6.3).
    TempoMalformed,
    /// Bad L: value (§6.2).
    UnitLengthMalformed,
    /// X: without a number (§5.3).
    NumberExpected,
    /// The requested part does not exist (§5.3).
    PartNotFound,
    /// Bad K: value (§6.4).
    KeyMalformed,
    /// K: mode or word not recognised (§6.4).
    KeyWordUnknown,
    /// A token that cannot start or continue an element (§7.1, §7.3).
    UnexpectedSymbol,
    /// A letter that is not a note, rest, prefix or field (§7.1, §5.2).
    ColonExpected,
    /// Something other than a note, rest or chord inside a tuplet (§7.9).
    NonNoteInTuplet,
    /// A tuplet number outside 2–9 (§7.9).
    TupletNumberOutOfRange,
    /// A volume mark not in the table (§7.11).
    UnknownVolumeMark,
    /// A volume mark without its closing `+` (§7.11).
    ClosingPlusExpected,
    /// A broken rhythm after a chord, or while one is pending (§7.7, §7.8).
    BrokenRhythmMisplaced,
    /// Four or more `>` or `<` (§7.8).
    BrokenRhythmTooLong,
    /// A chord without its `]`, or with something other than notes and rests (§7.7).
    ChordNotClosed,
    /// More than 6 chord entries after dropping duplicates (§7.7).
    TooManyNotesInChord,
    /// A length with denominator 0 (§7.5).
    ZeroLengthDenominator,
    /// A pitch below 48 (§7.3).
    NoteTooLow,
    /// A pitch above 84 (§7.3).
    NoteTooHigh,
    /// A duration below 0.06 s (§8.3).
    NoteTooShort,
    /// A duration above 8.0 s (§8.3). Tessera also uses it for an undefined (NaN) duration,
    /// which only tempo 0 can produce (§14.2).
    NoteTooLong,
    /// A tie that no later note continues (§7.6).
    UnfinishedTie,
    /// More than 10,000 events (§7.12). Shown as a notice outside the message table of
    /// §10.3, whose source is open (§10.4); [`AbcError::message`] gives `None`.
    TooManyEvents,
    /// Tessera's own limit: the file is larger than [`super::MAX_FILE_BYTES`].
    FileTooLarge,
    /// Tessera's own: the file exists but could not be read.
    Io(std::io::ErrorKind),
}

impl AbcErrorKind {
    /// The token of the string-table entry for this error (§10.2), in the table named by
    /// [`MESSAGE_TABLE_KEY`]; `None` for the kinds the original does not show from that
    /// table.
    pub fn token(self) -> Option<u32> {
        use AbcErrorKind::*;
        Some(match self {
            EncodingNotSupported => 0x0604_2C77,
            FileNotFound => 0x0109_98C4,
            MeterMalformed => 0x0B61_FD02,
            TempoMalformed => 0x0B48_93AF,
            UnitLengthMalformed => 0x094B_9348,
            NumberExpected => 0x0CE3_A5F2,
            PartNotFound => 0x0BCA_2D44,
            KeyMalformed => 0x07CB_63F9,
            KeyWordUnknown => 0x075D_3819,
            UnexpectedSymbol => 0x01D6_B85E,
            ColonExpected => 0x03B3_DC8E,
            NonNoteInTuplet => 0x03C9_5A94,
            TupletNumberOutOfRange => 0x0F97_88D5,
            UnknownVolumeMark => 0x0F84_EA5E,
            ClosingPlusExpected => 0x0D3A_EA53,
            BrokenRhythmMisplaced => 0x0634_37FD,
            BrokenRhythmTooLong => 0x0FE8_D55D,
            ChordNotClosed => 0x03FF_4F07,
            TooManyNotesInChord => 0x0DD0_6FA4,
            ZeroLengthDenominator => 0x0316_B41E,
            NoteTooLow => 0x08AE_3917,
            NoteTooHigh => 0x0AE2_4858,
            NoteTooShort => 0x0E53_AE34,
            NoteTooLong => 0x0AE3_93C7,
            UnfinishedTie => 0x0D27_C694,
            TooManyEvents | FileTooLarge | Io(_) => return None,
        })
    }

    /// The argument IDs the entry of [`token`](Self::token) binds, in the order of §10.2.
    pub fn arg_ids(self) -> &'static [u32] {
        use AbcErrorKind::*;
        match self {
            EncodingNotSupported | FileNotFound => &[ARG_FILE_NAME],
            PartNotFound | TooManyEvents | FileTooLarge | Io(_) => &[],
            _ => &[ARG_LINE, ARG_COLUMN],
        }
    }

    /// Every kind with an entry in the message table (§10.2), in the order of that table.
    pub const TABLE: [AbcErrorKind; 25] = {
        use AbcErrorKind::*;
        [
            EncodingNotSupported,
            FileNotFound,
            MeterMalformed,
            TempoMalformed,
            UnitLengthMalformed,
            NumberExpected,
            PartNotFound,
            KeyMalformed,
            KeyWordUnknown,
            UnexpectedSymbol,
            ColonExpected,
            NonNoteInTuplet,
            TupletNumberOutOfRange,
            UnknownVolumeMark,
            ClosingPlusExpected,
            BrokenRhythmMisplaced,
            BrokenRhythmTooLong,
            ChordNotClosed,
            TooManyNotesInChord,
            ZeroLengthDenominator,
            NoteTooLow,
            NoteTooHigh,
            NoteTooShort,
            NoteTooLong,
            UnfinishedTie,
        ]
    };
}

impl fmt::Display for AbcErrorKind {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        use AbcErrorKind::*;
        let s = match self {
            EncodingNotSupported => "encoding not supported",
            FileNotFound => "file not found",
            MeterMalformed => "meter malformed",
            TempoMalformed => "tempo malformed",
            UnitLengthMalformed => "unit length malformed",
            NumberExpected => "number expected",
            PartNotFound => "part not found",
            KeyMalformed => "key malformed",
            KeyWordUnknown => "key word unknown",
            UnexpectedSymbol => "unexpected symbol",
            ColonExpected => "colon expected",
            NonNoteInTuplet => "non-note in tuplet",
            TupletNumberOutOfRange => "tuplet number out of range",
            UnknownVolumeMark => "unknown volume mark",
            ClosingPlusExpected => "closing plus expected",
            BrokenRhythmMisplaced => "broken rhythm misplaced",
            BrokenRhythmTooLong => "broken rhythm too long",
            ChordNotClosed => "chord not closed",
            TooManyNotesInChord => "too many notes in chord",
            ZeroLengthDenominator => "zero length denominator",
            NoteTooLow => "note too low",
            NoteTooHigh => "note too high",
            NoteTooShort => "note too short",
            NoteTooLong => "note too long",
            UnfinishedTie => "unfinished tie",
            TooManyEvents => "too many events",
            FileTooLarge => "file too large",
            Io(kind) => return write!(f, "read error ({kind})"),
        };
        f.write_str(s)
    }
}

/// A refused part: the first error found (§10.1).
#[derive(Debug, Clone, PartialEq, Eq, thiserror::Error)]
pub struct AbcError {
    /// What went wrong.
    pub kind: AbcErrorKind,
    /// Where (line and column of the token, §10.2); `None` for file-level errors and the
    /// event limit.
    pub position: Option<Position>,
    /// The file's path, for file-level errors raised by [`super::read_song`] (§3.1–§3.2).
    pub path: Option<PathBuf>,
}

impl fmt::Display for AbcError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "{}", self.kind)?;
        if let Some(p) = self.position {
            write!(f, " at {p}")?;
        }
        if let Some(path) = &self.path {
            write!(f, " ({})", path.display())?;
        }
        Ok(())
    }
}

impl AbcError {
    pub(crate) fn at(kind: AbcErrorKind, position: Position) -> Self {
        AbcError {
            kind,
            position: Some(position),
            path: None,
        }
    }

    pub(crate) fn bare(kind: AbcErrorKind) -> Self {
        AbcError {
            kind,
            position: None,
            path: None,
        }
    }

    /// The string-table entry that reports this error and its arguments (§10.2–§10.3), or
    /// `None` when the kind has no entry ([`AbcErrorKind::token`]).
    ///
    /// Line and column are integers; the file name is the full path as text (empty when the
    /// error carries no path).
    pub fn message(&self) -> Option<MessageRef> {
        let token = self.kind.token()?;
        let pos = self.position.unwrap_or(Position::START);
        let args = self
            .kind
            .arg_ids()
            .iter()
            .map(|&id| MessageArg {
                id,
                value: match id {
                    ARG_LINE => ArgValue::Integer(i64::from(pos.line)),
                    ARG_COLUMN => ArgValue::Integer(i64::from(pos.column)),
                    _ => ArgValue::Text(
                        self.path
                            .as_ref()
                            .map(|p| p.to_string_lossy().into_owned())
                            .unwrap_or_default(),
                    ),
                },
            })
            .collect();
        Some(MessageRef {
            table_key: MESSAGE_TABLE_KEY,
            token,
            args,
        })
    }
}

/// A reference to a string-table entry plus its arguments: the client looks the table up
/// through the named-string-table DID map by `table_key`, takes the entry `token` from the
/// user's locale and renders it with `args` (`string-table.md` §7, §9).
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct MessageRef {
    /// Key in the named-string-table DID map ([`MESSAGE_TABLE_KEY`]).
    pub table_key: u32,
    /// Token of the entry within that table.
    pub token: u32,
    /// Arguments, bound by argument ID.
    pub args: Vec<MessageArg>,
}

/// One message argument.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct MessageArg {
    /// Argument ID (`string-table.md` §7).
    pub id: u32,
    /// Value.
    pub value: ArgValue,
}

/// An argument value.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum ArgValue {
    /// A number (line, column).
    Integer(i64),
    /// Text (file name, title).
    Text(String),
}

/// Notices around a performance (`abc-dialect.md` §10.4, §7.12).
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum Notice {
    /// A solo performance starts and has a title (at most 63 characters).
    NowPlaying {
        /// The title, already cut to [`NOTICE_TITLE_MAX_CHARS`].
        title: String,
    },
    /// A solo performance starts and has no title.
    NowPlayingUntitled,
    /// The synced start command was processed.
    SyncedStart,
    /// A part was refused for having more than 10,000 events. Its message source is open
    /// (§10.4); the client shows its own text.
    EventLimit,
}

impl Notice {
    /// The string-table entry for this notice, or `None` for [`Notice::EventLimit`].
    pub fn message(&self) -> Option<MessageRef> {
        let (token, args) = match self {
            Notice::NowPlaying { title } => (
                0x00A1_9C55,
                vec![MessageArg {
                    id: ARG_TITLE,
                    value: ArgValue::Text(title.clone()),
                }],
            ),
            Notice::NowPlayingUntitled => (0x05D7_A7A7, Vec::new()),
            Notice::SyncedStart => (0x012D_D4A4, Vec::new()),
            Notice::EventLimit => return None,
        };
        Some(MessageRef {
            table_key: MESSAGE_TABLE_KEY,
            token,
            args,
        })
    }
}
