// SPDX-License-Identifier: GPL-3.0-or-later
//! Sound records, DID prefix 0x0A (spec §9).

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

/// A sound record envelope (spec §9.1) with a borrowed audio slice.
#[derive(Clone, Copy, PartialEq)]
pub struct SoundRecord<'a> {
    /// Record DID.
    pub did: Did,
    /// Audio container bytes.
    pub audio: &'a [u8],
    /// Trailer u32 (meaning OPEN; observed 2, 3, 7).
    pub trailer_u32: u32,
    /// Trailer f32 (meaning OPEN; observed 0.0).
    pub trailer_f32: f32,
}

impl fmt::Debug for SoundRecord<'_> {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.debug_struct("SoundRecord")
            .field("did", &self.did)
            .field("container", &self.container())
            .field("audio", &format_args!("[{} bytes]", self.audio.len()))
            .field("trailer_u32", &self.trailer_u32)
            .field("trailer_f32", &self.trailer_f32)
            .finish()
    }
}

impl SoundRecord<'_> {
    /// Container kind of the audio.
    #[must_use]
    pub fn container(&self) -> AudioContainer {
        AudioContainer::sniff(self.audio)
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
        let trailer_f32 = r.f32()?;
        r.expect_end(Self::NAME)?;
        Ok(Self {
            did,
            audio,
            trailer_u32,
            trailer_f32,
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
    /// Contents of the `data` chunk.
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
    /// Walks the chunks of a RIFF WAVE stream to find `fmt ` and `data`. Unknown chunks
    /// (`JUNK`, `bext`, …) are skipped; chunk sizes are padded to even lengths. Error
    /// offsets are relative to the start of `audio`; a `fmt ` chunk shorter than 16 bytes
    /// is reported as truncated at the chunk body's offset.
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
        let mut data_chunk: Option<&'a [u8]> = None;
        while r.remaining() >= 8 && (fmt_chunk.is_none() || data_chunk.is_none()) {
            let id = r.array::<4>()?;
            let size = usize::try_from(r.u32()?).unwrap_or(usize::MAX);
            let body_at = r.position();
            let body = if &id == b"data" {
                // Tolerate a data chunk that claims more bytes than remain.
                let take = size.min(r.remaining());
                r.bytes(take)?
            } else {
                r.bytes(size)?
            };
            if size % 2 == 1 && !r.is_empty() {
                r.skip(1)?;
            }
            match &id {
                b"fmt " => fmt_chunk = fmt_chunk.or(Some((body_at, body))),
                b"data" => data_chunk = data_chunk.or(Some(body)),
                _ => {}
            }
        }
        let (fmt_at, fmt_body) =
            fmt_chunk.ok_or_else(|| FormatError::malformed(WHAT, 12, "no fmt chunk"))?;
        let data = data_chunk.ok_or_else(|| FormatError::malformed(WHAT, 12, "no data chunk"))?;
        if fmt_body.len() < FMT_CHUNK_MIN {
            return Err(FormatError::Truncated {
                offset: fmt_at,
                needed: FMT_CHUNK_MIN,
                available: fmt_body.len(),
            });
        }
        let mut f = Reader::new(fmt_body);
        Ok(Self {
            format_tag: f.u16()?,
            channels: f.u16()?,
            sample_rate: f.u32()?,
            byte_rate: f.u32()?,
            block_align: f.u16()?,
            bits_per_sample: f.u16()?,
            data,
        })
    }
}
