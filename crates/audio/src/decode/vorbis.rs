// SPDX-License-Identifier: GPL-3.0-or-later
//! Ogg Vorbis payloads: stream length, the original's played range, whole-buffer decoding
//! and a streaming source (`docs/specs/sound.md` §3.3, §6.4, §12.2 step 4).
//!
//! The played range is computed from the page structure and the audio packets' block sizes
//! before any audio is decoded (§3.3.2):
//!
//! - *T*: samples per channel all audio packets yield together (the first audio packet
//!   yields 0, every later one a quarter of the previous packet's block size plus a quarter
//!   of its own; zero-length packets are skipped and are not "previous", §3.3.3);
//! - *G*: granule of the last page; *N* = min(*G*, *T*), the Vorbis I stream length;
//! - *h*: the page on which the setup header completes, *a* the page after it;
//! - *A* = *D*(*h*), the samples of the audio packets that complete on page *h*;
//! - *Y*: the yields of the packets that begin and complete on page *a*, each counted only
//!   when the packet before it also begins and completes on page *a*;
//! - *P* = max(0, granule(*a*) − *Y*), *L* = *G* − *P*, *M* = min(*L*, *T* − *A*).
//!
//! The original plays decoded samples *A* … *A* + *M* − 1 and loops over exactly that
//! range. Decoding uses `lewton`'s packet-level API on the packets of our own page parser.
//! Nothing is ever zero-filled.
//!
//! **Starting mid-stream.** A Vorbis packet's output depends only on that packet and the
//! one before it (the overlap of their windows, VORBIS §1.3.2). A decoder that must start at
//! decoded sample *s* (the lead *A*, or a loop's restart) therefore begins at an audio
//! packet *p* before the one whose output contains *s*, with an empty window state: *p*
//! yields nothing and only primes the window, and every later sample equals the one a
//! decoder started at the first audio packet outputs. The plan records each audio packet's
//! first decoded-sample index and block size to place that output.
//!
//! One property of the decoder library constrains *p*: where a long block is followed by a
//! short one, the library returns the flat end of the long block's window with the long
//! packet, so the boundary between the two packets' outputs lies (long − short) / 4 samples
//! after the quarter-block arithmetic's (measured on install streams; the samples and their
//! total are unchanged). *p* is never such a long block, so the output after *p* begins
//! exactly where the arithmetic says. A rewind costs two or three packets, not the whole
//! lead.
//!
//! Samples are clamped to [−1, 1]; a non-finite sample (which a hostile header can still
//! produce through overflow in the transform) becomes 0.

use std::fmt;
use std::sync::Arc;

use lewton::audio::{PreviousWindowRight, read_audio_packet_generic};
use lewton::header::{IdentHeader, SetupHeader, read_header_ident, read_header_setup};
use lewton::samples::InterleavedSamples;

use super::DecodeError;
use super::ogg::{GRANULE_NONE, OggIssue, OggStream};
use super::setup::{SetupModes, patch_unused_books, walk_setup};
use crate::pcm::{Pcm, PcmSource};

/// Which samples of an Ogg stream to deliver.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default, Hash)]
pub enum OggRange {
    /// The original client's played range, decoded samples *A* … *A* + *M* − 1 (§3.3.2).
    /// Playback uses this.
    #[default]
    Original,
    /// The Vorbis I stream, decoded samples 0 … *N* − 1 (for tools: sound browser,
    /// exports).
    VorbisI,
    /// Every decoded sample, 0 … *T* − 1, with no end cut (diagnostics).
    Decoded,
}

/// Facts about one Ogg Vorbis payload, computed without decoding audio.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct OggInfo {
    /// Channels (1 or 2).
    pub channels: u16,
    /// Frames per second.
    pub sample_rate: u32,
    /// Short and long block sizes in samples.
    pub block_sizes: (u32, u32),
    /// Number of pages.
    pub pages: usize,
    /// Number of non-empty audio packets.
    pub audio_packets: usize,
    /// Number of zero-length packets after the headers (§3.3.3).
    pub zero_length_packets: usize,
    /// Index of page *h*, on which the setup header completes.
    pub setup_page: u32,
    /// *T*: samples per channel all audio packets yield.
    pub total_samples: u64,
    /// *G*: granule of the last page, `None` when it is −1.
    pub last_granule: Option<u64>,
    /// *N* = min(*G*, *T*): the Vorbis I stream length.
    pub vorbis_len: u64,
    /// *A*: the skipped lead of the original's played range.
    pub played_start: u64,
    /// *L*: the stream length the original's decoder library reports.
    pub library_len: u64,
    /// *M* = min(*L*, *T* − *A*): frames the original plays.
    pub played_len: u64,
    /// Census departures (SN3, SN3a) that did not stop decoding.
    pub issues: Vec<OggIssue>,
}

impl OggInfo {
    /// First frame and frame count of `range` in decoded-sample numbering.
    #[must_use]
    pub fn range(&self, range: OggRange) -> (u64, u64) {
        match range {
            OggRange::Original => (self.played_start, self.played_len),
            OggRange::VorbisI => (0, self.vorbis_len),
            OggRange::Decoded => (0, self.total_samples),
        }
    }
}

/// Parsed headers plus the packet layout: everything needed to decode, shared between a
/// stream's sources.
struct Plan {
    stream: OggStream,
    ident: IdentHeader,
    setup: SetupHeader,
    info: OggInfo,
    /// Every non-empty audio packet, in stream order.
    starts: Vec<AudioPacket>,
}

impl fmt::Debug for Plan {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.debug_struct("Plan")
            .field("info", &self.info)
            .finish_non_exhaustive()
    }
}

/// One non-empty audio packet of a plan.
#[derive(Debug, Clone, Copy)]
struct AudioPacket {
    /// Packet index in the stream (headers are 0–2).
    index: usize,
    /// First decoded-sample index of its output by the quarter-block arithmetic (§3.3.2).
    start: u64,
    /// Whether it is a long block.
    long: bool,
}

fn header_err(reason: &'static str) -> DecodeError {
    DecodeError::VorbisHeader { reason }
}

impl Plan {
    fn new(data: &[u8]) -> Result<Self, DecodeError> {
        let mut stream = OggStream::parse(data)?;
        if stream.packets.len() < 3 {
            return Err(header_err("fewer than three header packets"));
        }
        let ident_packet = stream
            .packet(data, 0)
            .ok_or_else(|| header_err("identification header"))?;
        let ident = read_header_ident(&ident_packet).map_err(|e| DecodeError::Vorbis {
            what: "identification header",
            detail: e.to_string(),
        })?;
        if !(1..=2).contains(&ident.audio_channels) {
            return Err(DecodeError::Unsupported {
                what: "Vorbis channel count",
                value: u64::from(ident.audio_channels),
            });
        }
        let comment = stream
            .packet(data, 1)
            .ok_or_else(|| header_err("comment header"))?;
        if comment.len() < 7 || comment[0] != 3 || &comment[1..7] != b"vorbis" {
            return Err(header_err("second packet is not a comment header"));
        }
        let setup_packet = stream
            .packet(data, 2)
            .ok_or_else(|| header_err("setup header"))?;
        let modes = walk_setup(&setup_packet, ident.audio_channels)?;
        // Placeholder books with no used entry get a codeword, so that the library never
        // reads an empty Huffman tree (see `setup`).
        let patched = patch_unused_books(&setup_packet, &modes);
        let setup = read_header_setup(
            patched.as_deref().unwrap_or(&setup_packet),
            ident.audio_channels,
            (ident.blocksize_0, ident.blocksize_1),
        )
        .map_err(|e| DecodeError::Vorbis {
            what: "setup header",
            detail: e.to_string(),
        })?;
        let mut issues = std::mem::take(&mut stream.issues);
        if ident_packet.len() != 30 {
            issues.push(OggIssue::IdentHeaderLength {
                len: ident_packet.len(),
            });
        }
        let (info, starts) = analyse(&stream, data, &ident, &modes, issues)?;
        Ok(Self {
            stream,
            ident,
            setup,
            info,
            starts,
        })
    }
}

/// Computes *T*, *G*, *N*, *A*, *L* and *M* from the packet layout (§3.3.2), and the first
/// decoded-sample index of every audio packet.
fn analyse(
    stream: &OggStream,
    data: &[u8],
    ident: &IdentHeader,
    modes: &SetupModes,
    mut issues: Vec<OggIssue>,
) -> Result<(OggInfo, Vec<AudioPacket>), DecodeError> {
    let pages = &stream.pages;
    let last = pages.len() - 1;
    let h = stream.packets[2].end_page;
    let a = h + 1;
    let a_exists = (a as usize) < pages.len();
    let short = 1u32 << ident.blocksize_0;
    let long = 1u32 << ident.blocksize_1;
    let mode_bits = modes.mode_bits();

    let mut completed = vec![0u64; pages.len()];
    let mut prev_block: Option<u32> = None;
    let mut prev_within_a = false;
    let mut total = 0u64;
    let mut y = 0u64;
    let mut audio_packets = 0usize;
    let mut zero_length = 0usize;
    let mut first_audio_page: Option<u32> = None;
    let mut starts: Vec<AudioPacket> = Vec::new();
    for (i, p) in stream.packets.iter().enumerate().skip(3) {
        if p.len == 0 {
            zero_length += 1;
            let sole_on_last = p.start_page as usize == last
                && p.end_page as usize == last
                && pages[last].packets_completed == 1;
            if !sole_on_last {
                issues.push(OggIssue::ZeroLengthPacket { page: p.end_page });
            }
            continue;
        }
        let first = data[p.offset];
        if first & 1 != 0 {
            return Err(DecodeError::VorbisAudio {
                packet: i,
                detail: "header packet among the audio packets".into(),
            });
        }
        let mode = (u32::from(first) >> 1) & ((1u32 << mode_bits) - 1);
        let long_block =
            *modes
                .block_flags
                .get(mode as usize)
                .ok_or_else(|| DecodeError::VorbisAudio {
                    packet: i,
                    detail: format!("mode {mode} out of range"),
                })?;
        let block = if long_block { long } else { short };
        let yielded = prev_block.map_or(0, |pb| u64::from(pb / 4 + block / 4));
        prev_block = Some(block);
        starts.push(AudioPacket {
            index: i,
            start: total,
            long: long_block,
        });
        total += yielded;
        completed[p.end_page as usize] += yielded;
        let within_a = a_exists && p.within(a);
        if within_a && prev_within_a {
            y += yielded;
        }
        prev_within_a = within_a;
        audio_packets += 1;
        first_audio_page.get_or_insert(p.end_page);
    }

    // SN3a: the first page completing audio, unless it is the last, must not have a granule
    // below the samples decoded up to it.
    if let Some(fa) = first_audio_page {
        let fa = fa as usize;
        let decoded_to: u64 = completed[..=fa].iter().sum();
        if fa != last && pages[fa].has_granule() && pages[fa].granule < decoded_to {
            issues.push(OggIssue::GranuleBelowDecoded { page: fa as u32 });
        }
    }

    let played_start: u64 = completed[..=h as usize].iter().sum();
    if !a_exists && total > 0 {
        // OPEN in §3.3.3, case (5): every audio packet completes on page h, so A = T and
        // M = 0 by steps 2 and 5. Kept as the steps say; reported.
        issues.push(OggIssue::NoPageAfterSetup);
    }
    let g = pages[last].has_granule().then_some(pages[last].granule);
    let p = if a_exists && pages[a as usize].granule != GRANULE_NONE {
        pages[a as usize].granule.saturating_sub(y)
    } else {
        0
    };
    let library_len = g.map_or(total, |g| g.saturating_sub(p));
    let played_len = library_len.min(total - played_start);
    let vorbis_len = g.map_or(total, |g| g.min(total));
    let info = OggInfo {
        channels: u16::from(ident.audio_channels),
        sample_rate: ident.audio_sample_rate,
        block_sizes: (short, long),
        pages: pages.len(),
        audio_packets,
        zero_length_packets: zero_length,
        setup_page: h,
        total_samples: total,
        last_granule: g,
        vorbis_len,
        played_start,
        library_len,
        played_len,
        issues,
    };
    Ok((info, starts))
}

/// Computes the facts of an Ogg Vorbis payload without decoding audio: page structure,
/// headers, *T*, *N* and the played range.
///
/// # Errors
/// Any [`DecodeError`] the page parser or the header checks report.
pub fn probe_ogg(data: &[u8]) -> Result<OggInfo, DecodeError> {
    Plan::new(data).map(|p| p.info)
}

/// Packet-by-packet decoder over a plan.
struct Core<D> {
    data: D,
    plan: Arc<Plan>,
    pwr: PreviousWindowRight,
    /// Next audio packet to decode, as an index into [`Plan::starts`].
    next_audio: usize,
    /// Decoded-sample index of the next frame the decoder outputs.
    pos: u64,
    scratch: Vec<u8>,
}

impl<D: AsRef<[u8]>> Core<D> {
    fn new(data: D, plan: Arc<Plan>) -> Self {
        Self {
            data,
            plan,
            pwr: PreviousWindowRight::new(),
            next_audio: 0,
            pos: 0,
            scratch: Vec::new(),
        }
    }

    /// Positions the decoder so that the frames it outputs from decoded sample `target` on
    /// are exactly those a decoder started at the first audio packet outputs (see the module
    /// documentation).
    fn seek(&mut self, target: u64) {
        let plan = &*self.plan;
        let starts = &plan.starts;
        self.pwr = PreviousWindowRight::new();
        if target == 0 || starts.len() < 2 {
            self.next_audio = 0;
            self.pos = 0;
            return;
        }
        if target >= plan.info.total_samples {
            self.next_audio = starts.len();
            self.pos = plan.info.total_samples;
            return;
        }
        // j: the audio packet whose output contains `target` (the last one starting at or
        // before it; packet 0 yields nothing and shares its start with packet 1), j ≥ 1.
        let j = starts
            .partition_point(|p| p.start <= target)
            .saturating_sub(1)
            .max(1);
        // The priming packet p < j: the latest one that is not a long block followed by a
        // short one, whose output boundary the library moves. p = 0 is the stream's start.
        let mut p = j - 1;
        while p > 0 && starts[p].long && !starts[p + 1].long {
            p -= 1;
        }
        self.next_audio = p;
        self.pos = if p == 0 { 0 } else { starts[p + 1].start };
    }

    /// Decodes the next audio packet. Returns the decoded-sample index of its first frame
    /// and its interleaved samples (empty for a priming packet), or `None` at the end of the
    /// stream.
    fn next(&mut self) -> Result<Option<(u64, Vec<f32>)>, DecodeError> {
        let plan = &*self.plan;
        let Some(ap) = plan.starts.get(self.next_audio) else {
            return Ok(None);
        };
        self.next_audio += 1;
        let i = ap.index;
        let info = &plan.stream.packets[i];
        let data = self.data.as_ref();
        let packet: &[u8] = if info.start_page == info.end_page {
            &data[info.offset..info.offset + info.len]
        } else {
            plan.stream.packet_into(data, i, &mut self.scratch);
            &self.scratch
        };
        let decoded: InterleavedSamples<f32> =
            read_audio_packet_generic(&plan.ident, &plan.setup, packet, &mut self.pwr).map_err(
                |e| DecodeError::VorbisAudio {
                    packet: i,
                    detail: e.to_string(),
                },
            )?;
        let mut samples = decoded.samples;
        for s in &mut samples {
            *s = if s.is_finite() {
                s.clamp(-1.0, 1.0)
            } else {
                0.0
            };
        }
        let start = self.pos;
        self.pos += (samples.len() / usize::from(plan.info.channels)) as u64;
        Ok(Some((start, samples)))
    }
}

/// Clips a decoded chunk starting at decoded sample `chunk_start` to the window
/// [`start`, `end`) and returns the interleaved slice to keep.
fn clip(chunk: &[f32], chunk_start: u64, channels: usize, start: u64, end: u64) -> &[f32] {
    let frames = (chunk.len() / channels) as u64;
    let lo = start.max(chunk_start).min(chunk_start + frames);
    let hi = end.min(chunk_start + frames).max(lo);
    let a = ((lo - chunk_start) as usize) * channels;
    let b = ((hi - chunk_start) as usize) * channels;
    &chunk[a..b]
}

/// Decodes `range` of an Ogg Vorbis payload into one buffer. Refuses (before decoding)
/// when the range holds more than `max_samples` interleaved samples. The buffer grows as
/// packets decode, so a stream that faults early does not allocate the whole range.
///
/// Blocking: it parses and CRC-checks every page and decodes the whole range. Call it on
/// the asset worker pool, not on the frame loop or the mixer thread.
///
/// # Errors
/// Page, header or packet faults; [`DecodeError::LimitExceeded`] for a range above
/// `max_samples`; [`DecodeError::ShortDecode`] when the packets yield fewer samples than
/// the range needs (never the case for a well-formed stream, §3.3.3).
pub fn decode_ogg(data: &[u8], range: OggRange, max_samples: usize) -> Result<Pcm, DecodeError> {
    let plan = Arc::new(Plan::new(data)?);
    decode_plan(data, plan, range, max_samples)
}

/// Largest up-front reservation of a whole-buffer decode, in samples (1 MiB of `f32`).
const PREALLOC_SAMPLES: usize = 1 << 18;

fn decode_plan(
    data: &[u8],
    plan: Arc<Plan>,
    range: OggRange,
    max_samples: usize,
) -> Result<Pcm, DecodeError> {
    let (start, len) = plan.info.range(range);
    let channels = usize::from(plan.info.channels);
    let wanted = len.saturating_mul(channels as u64);
    if wanted > max_samples as u64 {
        return Err(DecodeError::LimitExceeded {
            what: "decoded samples",
            value: wanted,
            limit: max_samples as u64,
        });
    }
    let end = start + len;
    let (rate, ch) = (plan.info.sample_rate, plan.info.channels);
    let mut out = Vec::with_capacity((wanted as usize).min(PREALLOC_SAMPLES));
    let mut core = Core::new(data, plan);
    core.seek(start);
    while (out.len() as u64) < wanted {
        let Some((at, chunk)) = core.next()? else {
            return Err(DecodeError::ShortDecode {
                expected: len,
                decoded: (out.len() / channels) as u64,
            });
        };
        out.extend_from_slice(clip(&chunk, at, channels, start, end));
    }
    Ok(Pcm {
        sample_rate: rate,
        channels: ch,
        samples: out.into(),
    })
}

/// A streaming [`PcmSource`] over an Ogg Vorbis payload: decodes one packet at a time and
/// delivers the frames of its [`OggRange`]. [`PcmSource::rewind`] restarts at the first
/// frame of the range (looping, §6.4) by decoding from the audio packet before it, so a
/// read after a rewind costs at most a few packets more than any other read. A range of 0
/// frames cannot be rewound (`rewind` returns false).
pub struct VorbisSource {
    core: Core<Arc<[u8]>>,
    start: u64,
    end: u64,
    /// Frames delivered since the last rewind.
    delivered: u64,
    /// Decoded samples not yet delivered, from `buf_pos`.
    buf: Vec<f32>,
    buf_pos: usize,
    /// Set after the end or a decode fault, until the next rewind.
    finished: bool,
    /// Whether a short stream or a decode fault has been logged (once per source).
    warned: bool,
}

impl fmt::Debug for VorbisSource {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.debug_struct("VorbisSource")
            .field("info", &self.core.plan.info)
            .field("start", &self.start)
            .field("end", &self.end)
            .field("delivered", &self.delivered)
            .field("finished", &self.finished)
            .finish_non_exhaustive()
    }
}

impl VorbisSource {
    /// Parses the payload's pages and headers and prepares to stream `range`.
    ///
    /// Blocking: it parses and CRC-checks every page of the payload (about 0.1 s for the
    /// largest music payloads). Build sources on the asset worker pool; only
    /// [`PcmSource::read`] and [`PcmSource::rewind`] belong on the mixer thread.
    ///
    /// # Errors
    /// Any page or header fault (audio packets are decoded later, by
    /// [`PcmSource::read`]).
    pub fn new(data: Arc<[u8]>, range: OggRange) -> Result<Self, DecodeError> {
        let plan = Arc::new(Plan::new(&data)?);
        Ok(Self::from_plan(data, plan, range))
    }

    fn from_plan(data: Arc<[u8]>, plan: Arc<Plan>, range: OggRange) -> Self {
        let (start, len) = plan.info.range(range);
        let mut core = Core::new(data, plan);
        core.seek(start);
        Self {
            core,
            start,
            end: start + len,
            delivered: 0,
            buf: Vec::new(),
            buf_pos: 0,
            finished: false,
            warned: false,
        }
    }

    /// The payload's facts (*T*, *N*, *A*, *L*, *M*, …).
    #[must_use]
    pub fn info(&self) -> &OggInfo {
        &self.core.plan.info
    }

    /// Fills the buffer with the next kept samples. False at the end or on a fault.
    fn refill(&mut self) -> bool {
        let channels = usize::from(self.core.plan.info.channels);
        while !self.finished {
            if self.core.pos >= self.end {
                self.finished = true;
                break;
            }
            match self.core.next() {
                Ok(Some((at, chunk))) => {
                    let kept = clip(&chunk, at, channels, self.start, self.end);
                    if kept.is_empty() {
                        continue;
                    }
                    self.buf.clear();
                    self.buf.extend_from_slice(kept);
                    self.buf_pos = 0;
                    return true;
                }
                Ok(None) => {
                    let wanted = self.end - self.start;
                    if self.delivered < wanted && !self.warned {
                        self.warned = true;
                        tracing::warn!(
                            delivered = self.delivered,
                            wanted,
                            "Ogg stream ended before its played range"
                        );
                    }
                    self.finished = true;
                }
                Err(e) => {
                    if !self.warned {
                        self.warned = true;
                        tracing::warn!(error = %e, "Ogg Vorbis decode fault; ending the stream");
                    }
                    self.finished = true;
                }
            }
        }
        false
    }
}

impl PcmSource for VorbisSource {
    fn sample_rate(&self) -> u32 {
        self.core.plan.info.sample_rate
    }

    fn channels(&self) -> u16 {
        self.core.plan.info.channels
    }

    fn len_frames(&self) -> Option<u64> {
        Some(self.end - self.start)
    }

    fn read(&mut self, out: &mut [f32]) -> usize {
        let channels = usize::from(self.core.plan.info.channels);
        let capacity = out.len() / channels;
        let mut written = 0usize;
        while written < capacity {
            if self.buf_pos >= self.buf.len() && !self.refill() {
                break;
            }
            let avail = (self.buf.len() - self.buf_pos) / channels;
            let n = avail.min(capacity - written);
            out[written * channels..(written + n) * channels]
                .copy_from_slice(&self.buf[self.buf_pos..self.buf_pos + n * channels]);
            self.buf_pos += n * channels;
            written += n;
            self.delivered += n as u64;
        }
        written
    }

    fn rewind(&mut self) -> bool {
        if self.end == self.start {
            return false;
        }
        self.core.seek(self.start);
        self.delivered = 0;
        self.buf.clear();
        self.buf_pos = 0;
        self.finished = false;
        true
    }
}

/// A payload whose pages and headers are parsed once, from which the buffered-or-streamed
/// helper builds either form without parsing again.
pub(crate) struct Prepared {
    plan: Arc<Plan>,
}

impl Prepared {
    pub(crate) fn new(data: &[u8]) -> Result<Self, DecodeError> {
        Ok(Self {
            plan: Arc::new(Plan::new(data)?),
        })
    }

    pub(crate) fn info(&self) -> &OggInfo {
        &self.plan.info
    }

    pub(crate) fn decode(
        &self,
        data: &[u8],
        range: OggRange,
        max_samples: usize,
    ) -> Result<Pcm, DecodeError> {
        decode_plan(data, Arc::clone(&self.plan), range, max_samples)
    }

    pub(crate) fn stream(self, data: Arc<[u8]>, range: OggRange) -> VorbisSource {
        VorbisSource::from_plan(data, self.plan, range)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn clip_window() {
        let chunk: Vec<f32> = (0..10).map(|i| i as f32).collect();
        // Mono chunk covering decoded samples 100..110, window 103..107.
        assert_eq!(clip(&chunk, 100, 1, 103, 107), &[3.0, 4.0, 5.0, 6.0]);
        assert!(clip(&chunk, 100, 1, 120, 130).is_empty());
        assert!(clip(&chunk, 100, 1, 0, 50).is_empty());
        // Stereo: 5 frames covering 0..5, window 1..3.
        assert_eq!(clip(&chunk, 0, 2, 1, 3), &[2.0, 3.0, 4.0, 5.0]);
    }
}
