// SPDX-License-Identifier: GPL-3.0-or-later
//! Player music: a reader for the ABC dialect of the original client
//! (`docs/specs/abc-dialect.md`; ROADMAP M6 exit criterion 3).
//!
//! The reader accepts, ignores, truncates and refuses exactly what the spec describes and
//! turns one **part** of a song file into timed note events:
//!
//! | Step | Spec | Here |
//! |---|---|---|
//! | Find the file (`.abc`, then `.txt`) in the `Music` folder | §3.1 | [`find_song`], [`read_song`] |
//! | Byte-order mark, characters, comments, backslashes, positions | §3.2–§3.5 | [`parse`] |
//! | Tokens, number tokens (Tessera's choice of §4.1) | §4 | [`parse`] |
//! | Fields, part selection by X:, defaults | §5 | [`PartRequest`] |
//! | M: L: Q: K: T: values | §6 | [`Performance::timing`], [`Performance::title`] |
//! | Music elements, event limit | §7 | [`MAX_EVENTS`] |
//! | Duration formula, limits, time line, start | §8 | [`duration_seconds`], [`SOLO_START_OFFSET`] |
//! | Note events, ordering, re-attacks, packed word, pitch slots | §9 | [`NoteEvent`], [`pack_note`], [`pitch_slot`] |
//! | Errors and notices as string-table references | §10 | [`AbcError::message`], [`Notice::message`] |
//! | Song listing for `/playlist` | §11 | [`list_songs`] |
//!
//! There is no audio or engine dependency: the instruments (`instruments.md`) consume
//! [`Performance::events`].
//!
//! ```
//! use tessera_audio::abc::{NoteEventKind, PartRequest, parse};
//!
//! let song = b"X:1\nT:Scale\nK:C\nC D E F|\n";
//! let perf = parse(song, PartRequest::Unspecified).unwrap();
//! assert_eq!(perf.title, "Scale");
//! let ons: Vec<(f64, u8)> = perf
//!     .events
//!     .iter()
//!     .filter(|e| e.kind == NoteEventKind::On)
//!     .map(|e| (e.time, e.pitch))
//!     .collect();
//! assert_eq!(ons, vec![(0.0, 60), (0.25, 62), (0.5, 64), (0.75, 65)]);
//! ```

mod chars;
mod error;
mod files;
mod reader;
mod sequence;
mod token;

pub use chars::Position;
pub use error::{
    ARG_COLUMN, ARG_FILE_NAME, ARG_LINE, ARG_TITLE, AbcError, AbcErrorKind, ArgValue,
    MESSAGE_TABLE_KEY, MessageArg, MessageRef, NOTICE_TITLE_MAX_CHARS, Notice,
};
pub use files::{
    LISTING_TITLE_MAX_CHARS, MAX_FILE_BYTES, MAX_LISTED_FILES, MAX_LISTED_PARTS,
    MAX_LISTED_PARTS_TOTAL, MAX_LISTING_BYTES, MUSIC_FOLDER, PartInfo, SongListing, find_song,
    list_songs, read_song,
};
pub use sequence::{
    NoteEvent, NoteEventKind, PITCH_MAX, PITCH_MIN, PITCH_SLOTS, ScheduledNote, duration_seconds,
    note_label, pack_note, pitch_slot,
};

use chars::{Bom, CharStream, detect_bom};

/// Most events (single notes, rests and chords) in one part (§7.12).
pub const MAX_EVENTS: usize = 10_000;

/// Shortest accepted duration, seconds (§8.3).
pub const MIN_DURATION: f64 = 0.06;

/// Longest accepted duration, seconds (§8.3).
pub const MAX_DURATION: f64 = 8.0;

/// Delay between a solo `/play` command and the performance start, seconds (§8.5).
pub const SOLO_START_OFFSET: f64 = 0.7;

/// Event times closer than this count as equal when events are ordered (§9.2).
pub const ORDER_TOLERANCE: f64 = 0.0001;

/// Which part of a file to read (§5.3, §11).
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Default)]
pub enum PartRequest {
    /// No part number: the first X: line selects its part.
    #[default]
    Unspecified,
    /// Read the part whose X: value equals this number.
    Number(i32),
}

impl From<Option<i32>> for PartRequest {
    fn from(v: Option<i32>) -> Self {
        v.map_or(PartRequest::Unspecified, PartRequest::Number)
    }
}

/// How a performance starts (§8.5, §11).
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum StartMode {
    /// `/play` without `sync`: starts [`SOLO_START_OFFSET`] after the command.
    Solo,
    /// `/play … sync`: checked at once, then waits for the start command.
    Synced,
}

/// How reading ended (§2.2, §7.1).
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ReadOutcome {
    /// The end of the file, or the X: line after the selected part.
    Complete,
    /// Reading stopped silently; what was read before plays (§2.2). The cause is an
    /// illegal character where a music element should start (§7.1), or an unclosed quote or
    /// grace group after other music, which swallows the rest of the file (§7.10).
    Truncated {
        /// Where the illegal character is, or the opening quote or `{` of the unclosed
        /// prefix.
        at: Position,
    },
}

/// The part-wide timing values in force at the end of reading (§8.1).
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct Timing {
    /// Meter numerator (chooses default tuplet ratios only).
    pub meter_numerator: u32,
    /// Meter denominator: the beat of the tempo.
    pub meter_denominator: u32,
    /// Unit note length.
    pub unit_length: f64,
    /// Tempo in beats per minute.
    pub tempo: u32,
}

/// An accepted part, ready for the instruments.
#[derive(Debug, Clone, PartialEq)]
pub struct Performance {
    /// The title: the first non-empty T: read, trimmed (§6.5); empty if none. Kept up to
    /// 1024 characters; [`Performance::notice_title`] gives the 63 the notice shows.
    pub title: String,
    /// The X: value of the selected part, or `None` if the file has no X: line before the
    /// end of reading.
    pub part: Option<i32>,
    /// Complete or truncated.
    pub outcome: ReadOutcome,
    /// Part-wide timing values.
    pub timing: Timing,
    /// Every note and rest with start and duration, in reading order (§8.4).
    pub notes: Vec<ScheduledNote>,
    /// The event list of §9.4 after ordering and re-attacks, ending with one
    /// [`NoteEventKind::End`] event.
    pub events: Vec<NoteEvent>,
}

impl Performance {
    /// The title as the "now playing" notice shows it: at most 63 characters (§6.5).
    pub fn notice_title(&self) -> &str {
        match self.title.char_indices().nth(NOTICE_TITLE_MAX_CHARS) {
            Some((i, _)) => &self.title[..i],
            None => &self.title,
        }
    }

    /// The notice shown when the performance starts (§10.4): "now playing" with or without
    /// title for a solo start, "synced start" for a synced one.
    pub fn start_notice(&self, mode: StartMode) -> Notice {
        match mode {
            StartMode::Synced => Notice::SyncedStart,
            StartMode::Solo if self.title.is_empty() => Notice::NowPlayingUntitled,
            StartMode::Solo => Notice::NowPlaying {
                title: self.notice_title().to_owned(),
            },
        }
    }

    /// Delay from the command to time 0 of the events (§8.5): [`SOLO_START_OFFSET`] for a
    /// solo start, `None` for a synced one (the start command supplies the time).
    pub fn start_offset(mode: StartMode) -> Option<f64> {
        match mode {
            StartMode::Solo => Some(SOLO_START_OFFSET),
            StartMode::Synced => None,
        }
    }

    /// The note-on events, in output order.
    pub fn note_ons(&self) -> impl Iterator<Item = &NoteEvent> {
        self.events.iter().filter(|e| e.kind == NoteEventKind::On)
    }

    /// Time of the last event (the last note-off), seconds; 0 when nothing sounds.
    pub fn end_time(&self) -> f64 {
        self.events.last().map_or(0.0, |e| e.time)
    }
}

/// Reads one part of a song file from its bytes (§3.2–§9).
///
/// Errors carry the kind and, for errors inside the file, the line and column; file-level
/// errors carry no path here ([`read_song`] adds it). Input of any size and content is
/// accepted without panicking; work and memory are linear in the input and the event list
/// is bounded by [`MAX_EVENTS`].
pub fn parse(bytes: &[u8], request: PartRequest) -> Result<Performance, AbcError> {
    let chars = match detect_bom(bytes) {
        Bom::Utf16 => return Err(AbcError::bare(AbcErrorKind::EncodingNotSupported)),
        Bom::Unsupported => return Err(AbcError::bare(AbcErrorKind::FileNotFound)),
        Bom::Utf8 => CharStream::new(bytes.get(3..).unwrap_or_default(), true),
        Bom::Plain => CharStream::new(bytes, false),
    };
    let song = reader::read(chars, request)?;
    let (notes, events) = sequence::sequence(&song)?;
    Ok(Performance {
        title: song.title.unwrap_or_default(),
        part: song.part,
        outcome: song.outcome,
        timing: Timing {
            meter_numerator: song.meter_num,
            meter_denominator: song.meter_den,
            unit_length: song.unit,
            tempo: song.tempo,
        },
        notes,
        events,
    })
}
