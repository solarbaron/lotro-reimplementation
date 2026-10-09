// SPDX-License-Identifier: GPL-3.0-or-later
//! Sound payload decoding (`docs/specs/sound.md` §3, §12.2): Ogg Vorbis with the original's
//! played range, RIFF WAVE PCM, and classification of non-audio payloads.
//!
//! Entry points:
//!
//! | Function | Input | Output |
//! |---|---|---|
//! | [`decode_record`] / [`decode_payload`] | a 0x0A record / its container bytes | [`Decoded`]: a whole [`Pcm`] buffer, or a non-audio category |
//! | [`open_record`] / [`open_payload`] | the same | [`Playable`]: a buffer for short sounds, a streaming [`PcmSource`] for long ones ([`StreamPolicy`]) |
//! | [`probe_payload`] | container bytes | [`Probe`]: format, length and played range without decoding audio |
//!
//! Containers are classified by their magic bytes ([`tessera_formats::AudioContainer`]),
//! never by the record's storage format (§3.2). DirectMusic (`RIFF…DMAP`) and unknown
//! containers are a [`NotAudio`] outcome, not an error (§3.5).
//!
//! Ogg payloads are parsed by our own bounded page parser ([`ogg`]), their setup header is
//! checked by a bounded walk ([`setup`]) and their audio packets are decoded by `lewton`.
//! The played range follows §3.3.2–§3.3.3 exactly ([`vorbis`]). WAV payloads follow §3.4
//! ([`wav`]); `smpl` loop points are ignored.
//!
//! Every decoder treats input as hostile: no panics, and allocations bounded by the input
//! length or by [`DecodeOptions::max_buffered_samples`].
//!
//! **Threads.** Every `decode_*`, `open_*` and `probe_*` function is blocking: it parses and
//! CRC-checks every page of the payload (tens of milliseconds for the largest music
//! payloads, §3.1) and the `decode_*` ones decode the whole range. Call them on the asset
//! worker pool, never on the frame loop or the mixer thread. Only [`PcmSource::read`] and
//! [`PcmSource::rewind`] of the sources they return are meant for the mixer thread.

pub mod ogg;
pub mod setup;
pub mod vorbis;
pub mod wav;

use std::sync::Arc;

use tessera_formats::{AudioContainer, FormatError, Record, SoundRecord};

pub use ogg::{OggIssue, OggStream};
pub use vorbis::{OggInfo, OggRange, VorbisSource, decode_ogg, probe_ogg};
pub use wav::{WavSource, WaveFormat, WaveIssue, decode_wav, probe_wav};

use crate::pcm::{Pcm, PcmCursor, PcmSource};

/// A decoding failure.
#[derive(Debug, thiserror::Error)]
#[non_exhaustive]
pub enum DecodeError {
    /// The 0x0A record envelope is malformed (§3.1, §12.2 steps 1–2).
    #[error("sound record: {0}")]
    Record(#[from] FormatError),
    /// An Ogg page fault.
    #[error("Ogg page at {offset}: {reason}")]
    OggPage {
        /// Offset in the container.
        offset: usize,
        /// What is wrong.
        reason: &'static str,
    },
    /// A malformed Vorbis header found by our own checks.
    #[error("Vorbis header: {reason}")]
    VorbisHeader {
        /// What is wrong.
        reason: &'static str,
    },
    /// A Vorbis header the decoder library rejected.
    #[error("Vorbis {what}: {detail}")]
    Vorbis {
        /// Which header.
        what: &'static str,
        /// The library's message.
        detail: String,
    },
    /// An audio packet that cannot be decoded.
    #[error("Vorbis audio packet {packet}: {detail}")]
    VorbisAudio {
        /// Packet index in the stream (headers are 0–2).
        packet: usize,
        /// What is wrong.
        detail: String,
    },
    /// A malformed RIFF WAVE file.
    #[error("WAVE at {offset}: {reason}")]
    Wave {
        /// Offset in the container.
        offset: usize,
        /// What is wrong.
        reason: &'static str,
    },
    /// A well-formed file in a format this decoder does not play.
    #[error("unsupported {what} {value}")]
    Unsupported {
        /// Which field.
        what: &'static str,
        /// Its value.
        value: u64,
    },
    /// A size above a decoder limit.
    #[error("{what} {value} exceeds the limit {limit}")]
    LimitExceeded {
        /// What was counted.
        what: &'static str,
        /// The count.
        value: u64,
        /// The limit.
        limit: u64,
    },
    /// The audio packets yielded fewer frames than the requested range.
    #[error("decoded {decoded} of {expected} frames")]
    ShortDecode {
        /// Frames the range needs.
        expected: u64,
        /// Frames decoded.
        decoded: u64,
    },
}

/// A payload that is not audio (§3.5, §12.2 step 3).
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
#[non_exhaustive]
pub enum NotAudio {
    /// `RIFF…DMAP`: a DirectMusic audio-path configuration. Never played.
    DirectMusic,
    /// A container that is neither Ogg nor RIFF WAVE nor DirectMusic.
    UnknownContainer,
}

/// The outcome of decoding a payload into one buffer.
#[derive(Debug, Clone, PartialEq)]
pub enum Decoded {
    /// Decoded audio: the played range for Ogg, the `data` chunk for WAV.
    Pcm(Pcm),
    /// Not audio.
    NotAudio(NotAudio),
}

/// The facts of a payload, without decoding audio.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum Probe {
    /// Ogg Vorbis.
    Ogg(OggInfo),
    /// RIFF WAVE PCM.
    Wave(WaveFormat),
    /// Not audio.
    NotAudio(NotAudio),
}

impl Probe {
    /// Frames, sample rate and channels of what [`decode_payload_with`] would deliver for
    /// `range` (which only matters for Ogg); `None` for non-audio.
    #[must_use]
    pub fn shape(&self, range: OggRange) -> Option<(u64, u32, u16)> {
        match self {
            Self::Ogg(i) => Some((i.range(range).1, i.sample_rate, i.channels)),
            Self::Wave(w) => Some((w.frames(), w.sample_rate, w.channels)),
            Self::NotAudio(_) => None,
        }
    }
}

/// Options for whole-buffer decoding.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct DecodeOptions {
    /// Which Ogg samples to deliver (default: the original's played range).
    pub range: OggRange,
    /// Largest buffer, in interleaved samples, a whole-buffer decode may allocate.
    pub max_buffered_samples: usize,
}

impl DecodeOptions {
    /// Default buffer limit: 30 s of 192 kHz stereo, 11,520,000 samples (44 MiB of `f32`).
    /// It matches [`StreamPolicy::DEFAULT_STREAM_ABOVE_SECS`] at the highest sample rate
    /// accepted in practice, so that every sound the default policy buffers fits. Tools
    /// that want a whole long track in one buffer raise it explicitly.
    pub const DEFAULT_MAX_BUFFERED_SAMPLES: usize = 30 * 192_000 * 2;
}

impl Default for DecodeOptions {
    fn default() -> Self {
        Self {
            range: OggRange::Original,
            max_buffered_samples: Self::DEFAULT_MAX_BUFFERED_SAMPLES,
        }
    }
}

/// Decodes a whole 0x0A record (envelope, §3.1) with default options.
///
/// # Errors
/// [`DecodeError::Record`] for a malformed envelope, else as [`decode_payload_with`].
pub fn decode_record(record: &[u8]) -> Result<Decoded, DecodeError> {
    let r = SoundRecord::parse(record)?;
    decode_payload(r.audio)
}

/// Decodes a payload's container bytes with default options.
///
/// # Errors
/// As [`decode_payload_with`].
pub fn decode_payload(audio: &[u8]) -> Result<Decoded, DecodeError> {
    decode_payload_with(audio, &DecodeOptions::default())
}

/// Decodes a payload's container bytes into one buffer.
///
/// # Errors
/// Any container fault; [`DecodeError::LimitExceeded`] when the result would exceed
/// [`DecodeOptions::max_buffered_samples`] (use [`open_payload`] to stream instead).
pub fn decode_payload_with(audio: &[u8], options: &DecodeOptions) -> Result<Decoded, DecodeError> {
    match AudioContainer::sniff(audio) {
        AudioContainer::Ogg => {
            decode_ogg(audio, options.range, options.max_buffered_samples).map(Decoded::Pcm)
        }
        AudioContainer::Wave => decode_wav(audio, options.max_buffered_samples).map(Decoded::Pcm),
        AudioContainer::DirectMusicAudioPath => Ok(Decoded::NotAudio(NotAudio::DirectMusic)),
        AudioContainer::Unknown => Ok(Decoded::NotAudio(NotAudio::UnknownContainer)),
    }
}

/// Probes a payload's container bytes: format, length and played range, without decoding
/// audio. A successful probe means the headers are decodable.
///
/// # Errors
/// Any container or header fault.
pub fn probe_payload(audio: &[u8]) -> Result<Probe, DecodeError> {
    match AudioContainer::sniff(audio) {
        AudioContainer::Ogg => probe_ogg(audio).map(Probe::Ogg),
        AudioContainer::Wave => probe_wav(audio).map(Probe::Wave),
        AudioContainer::DirectMusicAudioPath => Ok(Probe::NotAudio(NotAudio::DirectMusic)),
        AudioContainer::Unknown => Ok(Probe::NotAudio(NotAudio::UnknownContainer)),
    }
}

/// When to stream instead of decoding into one buffer.
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct StreamPolicy {
    /// Payloads whose delivered range lasts longer than this many seconds are streamed.
    pub stream_above_secs: f64,
    /// Range and buffer limit; a payload whose buffer would exceed the limit is streamed
    /// whatever its duration.
    pub options: DecodeOptions,
}

impl StreamPolicy {
    /// Default threshold: 30 s. Short effects (format-3 Oggs last at most 21.9 s, WAVs at
    /// most 6.2 s, §3.2) are buffered; music tracks are streamed.
    pub const DEFAULT_STREAM_ABOVE_SECS: f64 = 30.0;

    /// Whether a sound of `frames` at `sample_rate` with `channels` should be streamed.
    #[must_use]
    pub fn should_stream(&self, frames: u64, sample_rate: u32, channels: u16) -> bool {
        let secs = frames as f64 / f64::from(sample_rate.max(1));
        secs > self.stream_above_secs
            || frames.saturating_mul(u64::from(channels)) > self.options.max_buffered_samples as u64
    }
}

impl Default for StreamPolicy {
    fn default() -> Self {
        Self {
            stream_above_secs: Self::DEFAULT_STREAM_ABOVE_SECS,
            options: DecodeOptions::default(),
        }
    }
}

/// A payload ready to play.
#[derive(Debug)]
pub enum Playable {
    /// A short sound, decoded whole.
    Buffered(Pcm),
    /// A long sound, decoded while playing.
    Streamed(Box<dyn PcmSource>),
    /// Not audio.
    NotAudio(NotAudio),
}

impl Playable {
    /// A [`PcmSource`] for either form (a [`PcmCursor`] over a buffer); `None` for
    /// non-audio.
    #[must_use]
    pub fn into_source(self) -> Option<Box<dyn PcmSource>> {
        match self {
            Self::Buffered(pcm) => Some(Box::new(PcmCursor::new(pcm))),
            Self::Streamed(s) => Some(s),
            Self::NotAudio(_) => None,
        }
    }
}

/// Opens a whole 0x0A record: copies its container bytes and calls [`open_payload`].
/// Blocking (see the module documentation).
///
/// # Errors
/// [`DecodeError::Record`] for a malformed envelope, else as [`open_payload`].
pub fn open_record(record: &[u8], policy: &StreamPolicy) -> Result<Playable, DecodeError> {
    let r = SoundRecord::parse(record)?;
    match AudioContainer::sniff(r.audio) {
        AudioContainer::Ogg | AudioContainer::Wave => open_payload(Arc::from(r.audio), policy),
        AudioContainer::DirectMusicAudioPath => Ok(Playable::NotAudio(NotAudio::DirectMusic)),
        AudioContainer::Unknown => Ok(Playable::NotAudio(NotAudio::UnknownContainer)),
    }
}

/// Decides by length between a whole buffer and a streaming source (`policy`), and builds
/// it. Headers are parsed once; a streamed payload's audio packets are decoded later, by
/// [`PcmSource::read`]. Blocking (see the module documentation).
///
/// # Errors
/// Any container or header fault; for a buffered payload, any audio-packet fault.
pub fn open_payload(audio: Arc<[u8]>, policy: &StreamPolicy) -> Result<Playable, DecodeError> {
    let range = policy.options.range;
    let limit = policy.options.max_buffered_samples;
    match AudioContainer::sniff(&audio) {
        AudioContainer::Ogg => {
            let prepared = vorbis::Prepared::new(&audio)?;
            let info = prepared.info();
            let frames = info.range(range).1;
            if policy.should_stream(frames, info.sample_rate, info.channels) {
                Ok(Playable::Streamed(Box::new(prepared.stream(audio, range))))
            } else {
                prepared
                    .decode(&audio, range, limit)
                    .map(Playable::Buffered)
            }
        }
        AudioContainer::Wave => {
            let format = probe_wav(&audio)?;
            if policy.should_stream(format.frames(), format.sample_rate, format.channels) {
                Ok(Playable::Streamed(Box::new(WavSource::from_format(
                    audio, format,
                ))))
            } else {
                wav::decode_format(&audio, &format, limit).map(Playable::Buffered)
            }
        }
        AudioContainer::DirectMusicAudioPath => Ok(Playable::NotAudio(NotAudio::DirectMusic)),
        AudioContainer::Unknown => Ok(Playable::NotAudio(NotAudio::UnknownContainer)),
    }
}
