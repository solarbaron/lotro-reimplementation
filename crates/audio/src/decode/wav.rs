// SPDX-License-Identifier: GPL-3.0-or-later
//! RIFF WAVE payloads (`docs/specs/sound.md` §3.4, §12.2 step 5, §13 SN4).
//!
//! The chunk walk starts at offset 12; each chunk has an 8-byte header (four-character ID,
//! u32 size) and its body is followed by one pad byte when the size is odd. The first
//! `fmt ` and the first `data` chunk are used; everything else (`smpl` loop points
//! included, §3.4) is ignored. PCM (format tag 1) with 8-bit unsigned, 16-bit or 24-bit
//! signed samples and one or two channels is accepted. A `data` chunk shorter than its
//! declared size, or not a whole number of frames, is rejected.

use std::fmt;
use std::sync::Arc;

use super::DecodeError;
use crate::pcm::{Pcm, PcmSource};

/// Format tag of integer PCM.
pub const WAVE_FORMAT_PCM: u16 = 1;

/// A census departure that does not stop decoding (SN4).
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
#[non_exhaustive]
pub enum WaveIssue {
    /// The RIFF size field is not the payload length − 8.
    RiffSize {
        /// Declared size.
        declared: u32,
        /// Payload length − 8.
        expected: u64,
    },
    /// The chunk walk does not end exactly at the end of the payload.
    WalkEnd {
        /// Where the walk stopped.
        ended_at: usize,
        /// Payload length.
        len: usize,
    },
    /// The byte-rate field is not sample rate × block align.
    ByteRate {
        /// Declared byte rate.
        declared: u32,
    },
    /// A chunk after both `fmt ` and `data` runs past the end of the payload.
    TruncatedChunk {
        /// Offset of its header.
        offset: usize,
    },
}

/// The format of a RIFF WAVE payload and where its samples are.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct WaveFormat {
    /// Channels (1 or 2).
    pub channels: u16,
    /// Frames per second.
    pub sample_rate: u32,
    /// Bits per sample (8, 16 or 24).
    pub bits_per_sample: u16,
    /// Bytes per frame (channels × bits / 8).
    pub block_align: u16,
    /// Offset of the `data` chunk body in the payload.
    pub data_offset: usize,
    /// Length of the `data` chunk body.
    pub data_len: usize,
    /// Census departures that did not stop decoding.
    pub issues: Vec<WaveIssue>,
}

impl WaveFormat {
    /// Number of frames in the `data` chunk (0 for a block align of 0, which
    /// [`probe_wav`] never produces).
    #[must_use]
    pub fn frames(&self) -> u64 {
        self.data_len
            .checked_div(usize::from(self.block_align))
            .unwrap_or(0) as u64
    }
}

fn err(offset: usize, reason: &'static str) -> DecodeError {
    DecodeError::Wave { offset, reason }
}

fn u16_at(d: &[u8], at: usize) -> u16 {
    u16::from_le_bytes([d[at], d[at + 1]])
}

fn u32_at(d: &[u8], at: usize) -> u32 {
    u32::from_le_bytes([d[at], d[at + 1], d[at + 2], d[at + 3]])
}

/// Walks the chunks of a RIFF WAVE payload and checks its format.
///
/// # Errors
/// [`DecodeError::Wave`] for a malformed file (no `fmt ` or `data`, short or misaligned
/// `data`, inconsistent block align) and [`DecodeError::Unsupported`] for a format other
/// than 8-, 16- or 24-bit PCM with one or two channels.
pub fn probe_wav(audio: &[u8]) -> Result<WaveFormat, DecodeError> {
    if audio.len() < 12 || &audio[0..4] != b"RIFF" || &audio[8..12] != b"WAVE" {
        return Err(err(0, "not a RIFF WAVE file"));
    }
    let mut issues = Vec::new();
    let declared = u32_at(audio, 4);
    let expected = audio.len() as u64 - 8;
    if u64::from(declared) != expected {
        issues.push(WaveIssue::RiffSize { declared, expected });
    }
    let mut fmt_chunk: Option<(usize, usize)> = None;
    let mut data_chunk: Option<(usize, usize)> = None;
    let mut pos = 12usize;
    while audio.len() - pos >= 8 {
        let id = &audio[pos..pos + 4];
        let size = u32_at(audio, pos + 4) as usize;
        let body = pos + 8;
        let body_end = body.checked_add(size).filter(|&e| e <= audio.len());
        let Some(body_end) = body_end else {
            if id == b"data" && data_chunk.is_none() {
                return Err(err(pos, "data chunk shorter than its declared size"));
            }
            if fmt_chunk.is_some() && data_chunk.is_some() {
                issues.push(WaveIssue::TruncatedChunk { offset: pos });
                pos = audio.len();
                break;
            }
            return Err(err(pos, "chunk runs past the end"));
        };
        match id {
            b"fmt " if fmt_chunk.is_none() => fmt_chunk = Some((body, size)),
            b"data" if data_chunk.is_none() => data_chunk = Some((body, size)),
            _ => {}
        }
        // Odd sizes are followed by one pad byte; tolerate its absence at the very end.
        pos = (body_end + (size & 1)).min(audio.len());
    }
    if pos != audio.len() {
        issues.push(WaveIssue::WalkEnd {
            ended_at: pos,
            len: audio.len(),
        });
    }
    let (fmt_at, fmt_len) = fmt_chunk.ok_or_else(|| err(12, "no fmt chunk"))?;
    let (data_offset, data_len) = data_chunk.ok_or_else(|| err(12, "no data chunk"))?;
    if fmt_len < 16 {
        return Err(err(fmt_at, "fmt chunk shorter than 16 bytes"));
    }
    let tag = u16_at(audio, fmt_at);
    let channels = u16_at(audio, fmt_at + 2);
    let sample_rate = u32_at(audio, fmt_at + 4);
    let byte_rate = u32_at(audio, fmt_at + 8);
    let block_align = u16_at(audio, fmt_at + 12);
    let bits = u16_at(audio, fmt_at + 14);
    if tag != WAVE_FORMAT_PCM {
        return Err(DecodeError::Unsupported {
            what: "WAVE format tag",
            value: u64::from(tag),
        });
    }
    if !(1..=2).contains(&channels) {
        return Err(DecodeError::Unsupported {
            what: "WAVE channel count",
            value: u64::from(channels),
        });
    }
    if !matches!(bits, 8 | 16 | 24) {
        return Err(DecodeError::Unsupported {
            what: "WAVE bits per sample",
            value: u64::from(bits),
        });
    }
    if sample_rate == 0 {
        return Err(err(fmt_at + 4, "sample rate 0"));
    }
    if block_align != channels * bits / 8 {
        return Err(err(fmt_at + 12, "block align is not channels × bits / 8"));
    }
    if data_len % usize::from(block_align) != 0 {
        return Err(err(
            data_offset - 8,
            "data chunk is not a whole number of frames",
        ));
    }
    if u64::from(byte_rate) != u64::from(sample_rate) * u64::from(block_align) {
        issues.push(WaveIssue::ByteRate {
            declared: byte_rate,
        });
    }
    Ok(WaveFormat {
        channels,
        sample_rate,
        bits_per_sample: bits,
        block_align,
        data_offset,
        data_len,
        issues,
    })
}

/// Converts `bytes` (whole samples of `bits` each) to `f32` in [−1, 1] into `out`, which
/// holds exactly one value per sample.
fn convert(bytes: &[u8], bits: u16, out: &mut [f32]) {
    match bits {
        8 => {
            for (o, &b) in out.iter_mut().zip(bytes) {
                *o = (f32::from(b) - 128.0) / 128.0;
            }
        }
        16 => {
            for (o, c) in out.iter_mut().zip(bytes.as_chunks::<2>().0) {
                *o = f32::from(i16::from_le_bytes(*c)) / 32_768.0;
            }
        }
        _ => {
            for (o, c) in out.iter_mut().zip(bytes.as_chunks::<3>().0) {
                let v = i32::from_le_bytes([0, c[0], c[1], c[2]]) >> 8;
                *o = v as f32 / 8_388_608.0;
            }
        }
    }
}

/// Decodes a RIFF WAVE payload into one buffer. Refuses (before allocating) when the
/// `data` chunk holds more than `max_samples` interleaved samples.
///
/// # Errors
/// As [`probe_wav`], plus [`DecodeError::LimitExceeded`].
pub fn decode_wav(audio: &[u8], max_samples: usize) -> Result<Pcm, DecodeError> {
    let f = probe_wav(audio)?;
    decode_format(audio, &f, max_samples)
}

pub(crate) fn decode_format(
    audio: &[u8],
    f: &WaveFormat,
    max_samples: usize,
) -> Result<Pcm, DecodeError> {
    let samples = f.data_len / usize::from(f.bits_per_sample / 8);
    if samples > max_samples {
        return Err(DecodeError::LimitExceeded {
            what: "decoded samples",
            value: samples as u64,
            limit: max_samples as u64,
        });
    }
    let mut out = vec![0.0f32; samples];
    convert(
        &audio[f.data_offset..f.data_offset + f.data_len],
        f.bits_per_sample,
        &mut out,
    );
    Ok(Pcm {
        sample_rate: f.sample_rate,
        channels: f.channels,
        samples: out.into(),
    })
}

/// A streaming [`PcmSource`] over a RIFF WAVE payload: converts frames on demand from the
/// payload bytes. Loops over the whole `data` chunk (§6.4); an empty `data` chunk cannot be
/// rewound (`rewind` returns false).
pub struct WavSource {
    audio: Arc<[u8]>,
    format: WaveFormat,
    /// Next frame to deliver.
    frame: u64,
}

impl fmt::Debug for WavSource {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.debug_struct("WavSource")
            .field("format", &self.format)
            .field("frame", &self.frame)
            .finish_non_exhaustive()
    }
}

impl WavSource {
    /// Checks the payload and prepares to stream it.
    ///
    /// # Errors
    /// As [`probe_wav`].
    pub fn new(audio: Arc<[u8]>) -> Result<Self, DecodeError> {
        let format = probe_wav(&audio)?;
        Ok(Self::from_format(audio, format))
    }

    pub(crate) fn from_format(audio: Arc<[u8]>, format: WaveFormat) -> Self {
        Self {
            audio,
            format,
            frame: 0,
        }
    }

    /// The payload's format.
    #[must_use]
    pub fn format(&self) -> &WaveFormat {
        &self.format
    }
}

impl PcmSource for WavSource {
    fn sample_rate(&self) -> u32 {
        self.format.sample_rate
    }

    fn channels(&self) -> u16 {
        self.format.channels
    }

    fn len_frames(&self) -> Option<u64> {
        Some(self.format.frames())
    }

    fn read(&mut self, out: &mut [f32]) -> usize {
        let ch = usize::from(self.format.channels);
        let align = usize::from(self.format.block_align);
        let left = (self.format.frames() - self.frame) as usize;
        let n = (out.len() / ch).min(left);
        let from = self.format.data_offset + self.frame as usize * align;
        let bytes = &self.audio[from..from + n * align];
        convert(bytes, self.format.bits_per_sample, &mut out[..n * ch]);
        self.frame += n as u64;
        n
    }

    fn rewind(&mut self) -> bool {
        if self.format.frames() == 0 {
            return false;
        }
        self.frame = 0;
        true
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn sample_conversion() {
        let mut v = [0.0f32; 3];
        convert(&[0, 128, 255], 8, &mut v);
        assert_eq!(v, [-1.0, 0.0, 127.0 / 128.0]);
        let mut v = [0.0f32; 2];
        convert(&[0x00, 0x80, 0xFF, 0x7F], 16, &mut v);
        assert_eq!(v, [-1.0, 32_767.0 / 32_768.0]);
        convert(&[0x00, 0x00, 0x80, 0x01, 0x00, 0x00], 24, &mut v);
        assert_eq!(v, [-1.0, 1.0 / 8_388_608.0]);
    }
}
