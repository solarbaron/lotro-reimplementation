// SPDX-License-Identifier: GPL-3.0-or-later
//! Sound payload records, DID prefix 0x0A (`records.md` §9, `sound.md` §3).
//!
//! The record holds the audio container (Ogg Vorbis, RIFF WAVE or a non-audio DirectMusic
//! audio-path configuration) and a two-word trailer: the storage format (`sound.md` §3.2) and
//! a reserved word (§3.1). The sound-info records (0x2A) that name these payloads are in
//! [`crate::sound_info`].

use std::fmt;

use crate::did::{Did, DidKind};
use crate::error::{FormatError, Result};
use crate::reader::Reader;
use crate::record::{Record, read_prologue};

/// Container kind of the audio inside a sound record (spec §9.2).
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum AudioContainer {
    /// `OggS`: Ogg (Vorbis).
    Ogg,
    /// `RIFF` … `WAVE`.
    Wave,
    /// `RIFF` … `DMAP`: DirectMusic audio-path configuration, not audio.
    DirectMusicAudioPath,
    /// Anything else.
    Unknown,
}

impl AudioContainer {
    /// Classifies audio bytes by their magic.
    #[must_use]
    pub fn sniff(audio: &[u8]) -> Self {
        if audio.starts_with(b"OggS") {
            return Self::Ogg;
        }
        if audio.starts_with(b"RIFF") {
            match audio.get(8..12) {
                Some(b"WAVE") => return Self::Wave,
                Some(b"DMAP") => return Self::DirectMusicAudioPath,
                _ => {}
            }
        }
        Self::Unknown
    }

    /// Usual file extension for the container, if it is audio.
    #[must_use]
    pub fn extension(self) -> Option<&'static str> {
        match self {
            Self::Ogg => Some("ogg"),
            Self::Wave => Some("wav"),
            Self::DirectMusicAudioPath | Self::Unknown => None,
        }
    }
}

/// Storage format of a payload: the first trailer word (`sound.md` §3.2).
///
/// A playback hint, not a codec identifier: always classify the data with
/// [`AudioContainer::sniff`].
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum StorageFormat {
    /// 2: the data is kept as stored; Ogg is decoded while playing (streaming possible).
    Stored,
    /// 3: the data is used as PCM; an Ogg stream is decoded in full when the record loads.
    Pcm,
    /// 7: not audio (the DirectMusic audio-path records, `sound.md` §3.5).
    NotAudio,
    /// Any other value (none in the reference install).
    Other(u32),
}

impl StorageFormat {
    /// Classifies a stored trailer word.
    #[must_use]
    pub fn from_raw(raw: u32) -> Self {
        match raw {
            2 => Self::Stored,
            3 => Self::Pcm,
            7 => Self::NotAudio,
            other => Self::Other(other),
        }
    }

    /// The stored word.
    #[must_use]
    pub fn raw(self) -> u32 {
        match self {
            Self::Stored => 2,
            Self::Pcm => 3,
            Self::NotAudio => 7,
            Self::Other(v) => v,
        }
    }
}

/// A violation of the payload invariants SN1/SN2 (`sound.md` §13), reported by
/// [`SoundRecord::validate`].
#[derive(Debug, Clone, Copy, PartialEq, Eq, thiserror::Error)]
#[non_exhaustive]
pub enum SoundIssue {
    /// SN1: the record length is not *S* + 16.
    #[error("record length {record_len} is not audio length {audio_len} + 16")]
    Length {
        /// Record length in bytes.
        record_len: usize,
        /// Stored audio length *S*.
        audio_len: usize,
    },
    /// SN1: the record's first word is not the DID it was looked up under.
    #[error("record DID {found} is not the expected {expected}")]
    Did {
        /// DID the record was looked up under.
        expected: Did,
        /// DID stored in the record.
        found: Did,
    },
    /// SN1: the reserved word is not 0.
    #[error("reserved word is {0:#010X}, not 0")]
    Reserved(u32),
    /// SN2: the data is neither `OggS`, `RIFF…WAVE` nor `RIFF…DMAP`.
    #[error("unknown audio container")]
    UnknownContainer,
    /// SN2: the storage format is not 2, 3 or 7.
    #[error("storage format {0} is not 2, 3 or 7")]
    StorageFormat(u32),
    /// SN2: storage format 7 without DMAP data, or DMAP data with another format.
    #[error("storage format {format} does not fit container {container:?}")]
    FormatMismatch {
        /// The stored storage format.
        format: u32,
        /// The sniffed container.
        container: AudioContainer,
    },
}

/// A sound payload record (`records.md` §9.1, `sound.md` §3.1) with a borrowed audio slice.
///
/// Layout: DID, u32 length *S*, *S* bytes of data, u32 storage format, u32 reserved word;
/// nothing follows. The two trailer words keep their historical field names;
/// [`SoundRecord::storage_format`] and [`SoundRecord::reserved`] give their meanings
/// (`sound.md` §15 X1).
#[derive(Clone, Copy, PartialEq)]
pub struct SoundRecord<'a> {
    /// Record DID.
    pub did: Did,
    /// Audio container bytes.
    pub audio: &'a [u8],
    /// Storage format word (`sound.md` §3.2; observed 2, 3, 7). See
    /// [`SoundRecord::storage_format`].
    pub trailer_u32: u32,
    /// The reserved word as stored, viewed as an f32 (0.0 in every record). Kept under its
    /// historical name and type for existing callers; the word is a u32 (`sound.md` §3.1,
    /// §15 X1), so prefer [`SoundRecord::reserved`], which returns the stored bits exactly.
    pub trailer_f32: f32,
    /// The reserved word as read (a u32), so [`SoundRecord::reserved`] never round-trips the
    /// bits through a float.
    reserved_bits: u32,
}

impl fmt::Debug for SoundRecord<'_> {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.debug_struct("SoundRecord")
            .field("did", &self.did)
            .field("container", &self.container())
            .field("audio", &format_args!("[{} bytes]", self.audio.len()))
            .field("storage_format", &self.storage_format())
            .field("reserved", &format_args!("{:#010X}", self.reserved()))
            .finish()
    }
}

impl SoundRecord<'_> {
    /// Fixed bytes around the data: DID, length, storage format, reserved word (SN1).
    pub const OVERHEAD: usize = 16;

    /// Container kind of the audio.
    #[must_use]
    pub fn container(&self) -> AudioContainer {
        AudioContainer::sniff(self.audio)
    }

    /// The storage format (`sound.md` §3.2): the first trailer word, classified.
    #[must_use]
    pub fn storage_format(&self) -> StorageFormat {
        StorageFormat::from_raw(self.trailer_u32)
    }

    /// The reserved word after the storage format, read as a u32 (`sound.md` §3.1, §15 X1;
    /// 0 in every record, meaning OPEN). The bits are those stored, read as an integer.
    #[must_use]
    pub fn reserved(&self) -> u32 {
        self.reserved_bits
    }

    /// Length of the record this envelope was decoded from: *S* + 16.
    #[must_use]
    pub fn record_len(&self) -> usize {
        self.audio.len().saturating_add(Self::OVERHEAD)
    }

    /// Checks the invariants SN1 and SN2 (`sound.md` §13) for a record looked up under
    /// `expected` whose payload was `record_len` bytes long: length *S* + 16, the first
    /// word equal to the DID, reserved word 0, a known container, storage format 2, 3 or 7,
    /// and 7 exactly for DirectMusic data. Returns the first violation.
    ///
    /// [`SoundRecord::parse`] already rejects a length mismatch; the strict check is for
    /// validation tools, which should report these as data anomalies, not decode errors.
    pub fn validate(&self, expected: impl Into<Did>, record_len: usize) -> Result<(), SoundIssue> {
        let expected = expected.into();
        if record_len != self.record_len() {
            return Err(SoundIssue::Length {
                record_len,
                audio_len: self.audio.len(),
            });
        }
        if self.did != expected {
            return Err(SoundIssue::Did {
                expected,
                found: self.did,
            });
        }
        if self.reserved() != 0 {
            return Err(SoundIssue::Reserved(self.reserved()));
        }
        let container = self.container();
        if container == AudioContainer::Unknown {
            return Err(SoundIssue::UnknownContainer);
        }
        let format = self.storage_format();
        if let StorageFormat::Other(v) = format {
            return Err(SoundIssue::StorageFormat(v));
        }
        let dmap = container == AudioContainer::DirectMusicAudioPath;
        if dmap != (format == StorageFormat::NotAudio) {
            return Err(SoundIssue::FormatMismatch {
                format: format.raw(),
                container,
            });
        }
        Ok(())
    }
}

impl<'a> Record<'a> for SoundRecord<'a> {
    const NAME: &'static str = "sound record";

    fn did(&self) -> Did {
        self.did
    }

    fn parse(data: &'a [u8]) -> Result<Self> {
        let mut r = Reader::new(data);
        let did = read_prologue(&mut r, Self::NAME, |k| k == DidKind::Sound)?;
        let len = r.u32()?;
        let len = usize::try_from(len).unwrap_or(usize::MAX);
        let audio = r.bytes(len)?;
        let trailer_u32 = r.u32()?;
        let reserved_bits = r.u32()?;
        r.expect_end(Self::NAME)?;
        Ok(Self {
            did,
            audio,
            trailer_u32,
            trailer_f32: f32::from_bits(reserved_bits),
            reserved_bits,
        })
    }
}

/// The `fmt ` chunk of a RIFF WAVE file plus its `data` chunk.
#[derive(Clone, Copy, PartialEq, Eq)]
pub struct WaveInfo<'a> {
    /// Format tag (1 = PCM).
    pub format_tag: u16,
    /// Channel count.
    pub channels: u16,
    /// Samples per second.
    pub sample_rate: u32,
    /// Average bytes per second.
    pub byte_rate: u32,
    /// Bytes per sample frame.
    pub block_align: u16,
    /// Bits per sample.
    pub bits_per_sample: u16,
    /// Contents of the `data` chunk: exactly its declared size, a whole number of
    /// `block_align`-byte frames (when `block_align` is not 0).
    pub data: &'a [u8],
}

impl fmt::Debug for WaveInfo<'_> {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.debug_struct("WaveInfo")
            .field("format_tag", &self.format_tag)
            .field("channels", &self.channels)
            .field("sample_rate", &self.sample_rate)
            .field("byte_rate", &self.byte_rate)
            .field("block_align", &self.block_align)
            .field("bits_per_sample", &self.bits_per_sample)
            .field("data", &format_args!("[{} bytes]", self.data.len()))
            .finish()
    }
}

/// Bytes of the `fmt ` chunk fields [`WaveInfo`] reads.
const FMT_CHUNK_MIN: usize = 16;

impl<'a> WaveInfo<'a> {
    /// Walks the chunks of a RIFF WAVE stream to find `fmt ` and `data` (`sound.md` §3.4,
    /// §12.2 step 5). Unknown chunks (`JUNK`, `bext`, …) are skipped; chunk sizes are padded
    /// to even lengths. Error offsets are relative to the start of `audio`.
    ///
    /// # Errors
    ///
    /// [`FormatError::Truncated`] when any chunk, the `data` chunk included, is shorter than
    /// its declared size, or when the `fmt ` chunk has fewer than 16 bytes (reported at the
    /// chunk body's offset); [`FormatError::Malformed`] for a missing magic, form type,
    /// `fmt ` or `data` chunk, or a `data` length that is not a multiple of a non-zero
    /// block align. The format tag, channel count and bit depth are returned as stored, not
    /// checked: the decoder decides which it plays.
    pub fn parse(audio: &'a [u8]) -> Result<Self> {
        const WHAT: &str = "RIFF WAVE";
        let mut r = Reader::new(audio);
        if r.bytes(4)? != b"RIFF" {
            return Err(FormatError::malformed(WHAT, 0, "missing RIFF magic"));
        }
        let _riff_size = r.u32()?;
        if r.bytes(4)? != b"WAVE" {
            return Err(FormatError::malformed(WHAT, 8, "form type is not WAVE"));
        }
        // The fmt chunk body and its offset in `audio`, so errors report absolute offsets.
        let mut fmt_chunk: Option<(usize, &'a [u8])> = None;
        let mut data_chunk: Option<(usize, &'a [u8])> = None;
        while r.remaining() >= 8 && (fmt_chunk.is_none() || data_chunk.is_none()) {
            let id = r.array::<4>()?;
            let size = usize::try_from(r.u32()?).unwrap_or(usize::MAX);
            let body_at = r.position();
            // §12.2 step 5: a chunk (the `data` chunk included) must hold its declared size.
            let body = r.bytes(size)?;
            if size % 2 == 1 && !r.is_empty() {
                r.skip(1)?;
            }
            match &id {
                b"fmt " => fmt_chunk = fmt_chunk.or(Some((body_at, body))),
                b"data" => data_chunk = data_chunk.or(Some((body_at, body))),
                _ => {}
            }
        }
        let (fmt_at, fmt_body) =
            fmt_chunk.ok_or_else(|| FormatError::malformed(WHAT, 12, "no fmt chunk"))?;
        let (data_at, data) =
            data_chunk.ok_or_else(|| FormatError::malformed(WHAT, 12, "no data chunk"))?;
        if fmt_body.len() < FMT_CHUNK_MIN {
            return Err(FormatError::Truncated {
                offset: fmt_at,
                needed: FMT_CHUNK_MIN,
                available: fmt_body.len(),
            });
        }
        let mut f = Reader::new(fmt_body);
        let info = Self {
            format_tag: f.u16()?,
            channels: f.u16()?,
            sample_rate: f.u32()?,
            byte_rate: f.u32()?,
            block_align: f.u16()?,
            bits_per_sample: f.u16()?,
            data,
        };
        let align = usize::from(info.block_align);
        if align != 0 && data.len() % align != 0 {
            return Err(FormatError::malformed(
                WHAT,
                data_at,
                format!(
                    "data length {} is not a multiple of the block align {align}",
                    data.len()
                ),
            ));
        }
        Ok(info)
    }
}
