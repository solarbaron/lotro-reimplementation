// SPDX-License-Identifier: GPL-3.0-or-later
//! Synthetic decode tests (`docs/specs/sound.md` §3, §12.2, §14.2, §14.5).
//!
//! The Ogg fixtures are built here without an encoder. The silent ones use a minimal but
//! complete Vorbis I setup header (one codebook, one floor 1 with no partitions, one
//! residue with an empty range, one mapping, a short and a long mode) and one-byte audio
//! packets whose floors are unused; they check every frame count of the played-range
//! arithmetic with the real decoder library. The audible ones add a VQ codebook and a
//! non-empty residue, and draw each packet's floor and residue values from a seed, so that
//! every packet decodes to different, non-zero samples; they check *which* samples the
//! played range delivers (decoded samples *A* … *A* + *M* − 1), also after a rewind.

use std::sync::Arc;

use tessera_audio::decode::ogg::{FLAG_BOS, FLAG_CONTINUED, FLAG_EOS, build_page, lacing_for};
use tessera_audio::decode::{
    DecodeError, DecodeOptions, Decoded, NotAudio, OggIssue, OggRange, Playable, Probe,
    StreamPolicy, VorbisSource, WavSource, WaveFormat, decode_ogg, decode_payload,
    decode_payload_with, decode_record, open_payload, open_record, probe_ogg, probe_payload,
    probe_wav,
};
use tessera_audio::pcm::{Pcm, PcmSource};

// ---------------------------------------------------------------------------------------
// Vorbis fixture builder
// ---------------------------------------------------------------------------------------

/// LSB-first bit writer (VORBIS §2).
#[derive(Default)]
struct BitWriter {
    bytes: Vec<u8>,
    bit: u32,
}

impl BitWriter {
    fn put(&mut self, value: u32, bits: u32) -> &mut Self {
        for i in 0..bits {
            if self.bit == 0 {
                self.bytes.push(0);
            }
            if (value >> i) & 1 == 1 {
                *self.bytes.last_mut().unwrap() |= 1 << self.bit;
            }
            self.bit = (self.bit + 1) % 8;
        }
        self
    }
}

fn ident_header(channels: u8, rate: u32, bs0: u8, bs1: u8) -> Vec<u8> {
    let mut v = vec![1];
    v.extend_from_slice(b"vorbis");
    v.extend_from_slice(&0u32.to_le_bytes());
    v.push(channels);
    v.extend_from_slice(&rate.to_le_bytes());
    v.extend_from_slice(&[0; 12]);
    v.push((bs1 << 4) | bs0);
    v.push(1);
    assert_eq!(v.len(), 30);
    v
}

fn comment_header() -> Vec<u8> {
    let mut v = vec![3];
    v.extend_from_slice(b"vorbis");
    v.extend_from_slice(&4u32.to_le_bytes());
    v.extend_from_slice(b"test");
    v.extend_from_slice(&0u32.to_le_bytes());
    v.push(1);
    v
}

fn setup_header() -> Vec<u8> {
    let mut w = BitWriter::default();
    // One codebook: 1 dimension, 2 entries of length 1, no lookup.
    w.put(0, 8);
    w.put(0x56_4342, 24)
        .put(1, 16)
        .put(2, 24)
        .put(0, 1)
        .put(0, 1);
    w.put(0, 5).put(0, 5).put(0, 4);
    // One time-domain placeholder.
    w.put(0, 6).put(0, 16);
    // One floor 1 with no partitions, multiplier 1, range bits 8.
    w.put(0, 6).put(1, 16).put(0, 5).put(0, 2).put(8, 4);
    // One residue 0 with an empty range.
    w.put(0, 6)
        .put(0, 16)
        .put(0, 24)
        .put(0, 24)
        .put(0, 24)
        .put(0, 6)
        .put(0, 8);
    w.put(0, 3).put(0, 1);
    // One mapping: one submap, no coupling.
    w.put(0, 6)
        .put(0, 16)
        .put(0, 1)
        .put(0, 1)
        .put(0, 2)
        .put(0, 8)
        .put(0, 8)
        .put(0, 8);
    // Two modes: short (0) and long (1).
    w.put(1, 6);
    w.put(0, 1).put(0, 16).put(0, 16).put(0, 8);
    w.put(1, 1).put(0, 16).put(0, 16).put(0, 8);
    // Framing bit.
    w.put(1, 1);
    let mut v = vec![5];
    v.extend_from_slice(b"vorbis");
    v.extend_from_slice(&w.bytes);
    v
}

/// One audio packet whose floors are unused (silence). `long` selects mode 1; `prev_long`
/// and `next_long` are its window flags. `pad` extra zero bytes make it longer without
/// changing its decode.
fn audio_packet(channels: u8, long: bool, prev_long: bool, next_long: bool, pad: usize) -> Vec<u8> {
    let mut w = BitWriter::default();
    w.put(0, 1).put(u32::from(long), 1);
    if long {
        w.put(u32::from(prev_long), 1).put(u32::from(next_long), 1);
    }
    for _ in 0..channels {
        w.put(0, 1);
    }
    let mut v = w.bytes;
    v.resize(v.len() + pad, 0);
    v
}

/// `n` short audio packets.
fn short_packets(channels: u8, n: usize) -> Vec<Vec<u8>> {
    (0..n)
        .map(|_| audio_packet(channels, false, false, false, 0))
        .collect()
}

/// A part of a page: packet bytes and whether the packet completes on this page.
type Part = (Vec<u8>, bool);

fn whole(packets: &[Vec<u8>]) -> Vec<Part> {
    packets.iter().map(|p| (p.clone(), true)).collect()
}

/// Builds a stream from pages of (flags, granule, parts).
fn stream(pages: &[(u8, u64, Vec<Part>)]) -> Vec<u8> {
    let mut out = Vec::new();
    for (seq, (flags, granule, parts)) in pages.iter().enumerate() {
        let mut lacing = Vec::new();
        let mut body = Vec::new();
        for (bytes, complete) in parts {
            if *complete {
                lacing.extend(lacing_for(bytes.len()));
            } else {
                assert_eq!(bytes.len() % 255, 0, "an open part must be whole segments");
                lacing.extend(std::iter::repeat_n(255u8, bytes.len() / 255));
            }
            body.extend_from_slice(bytes);
        }
        out.extend(build_page(*flags, *granule, 0x5EED, seq as u32, &lacing, &body).unwrap());
    }
    out
}

const BS0: u8 = 8; // 256
const BS1: u8 = 11; // 2048

fn headers(channels: u8) -> (Vec<u8>, Vec<u8>, Vec<u8>) {
    (
        ident_header(channels, 22_050, BS0, BS1),
        comment_header(),
        setup_header(),
    )
}

/// §14.5 row 1: fresh first audio page.
fn case_fresh() -> Vec<u8> {
    let (i, c, s) = headers(1);
    let p = short_packets(1, 11);
    stream(&[
        (FLAG_BOS, 0, whole(&[i])),
        (0, 0, whole(&[c, s])),
        (0, 512, whole(&p[0..5])),
        (FLAG_EOS, 1200, whole(&p[5..11])),
    ])
}

/// §14.5 row 2: granule offset and an empty final packet.
fn case_offset_empty() -> Vec<u8> {
    let (i, c, s) = headers(1);
    let p = short_packets(1, 11);
    stream(&[
        (FLAG_BOS, 0, whole(&[i])),
        (0, 0, whole(&[c, s])),
        (0, 582, whole(&p[0..5])),
        (0, 1350, whole(&p[5..11])),
        (FLAG_EOS, 1350, whole(&[Vec::new()])),
    ])
}

/// §14.5 row 3: audio on the setup page.
fn case_audio_on_setup() -> Vec<u8> {
    let (i, c, s) = headers(1);
    let p = short_packets(1, 11);
    let mut h = whole(&[c, s]);
    h.extend(whole(&p[0..4]));
    stream(&[
        (FLAG_BOS, 0, whole(&[i])),
        (0, 384, h),
        (0, 896, whole(&p[4..8])),
        (FLAG_EOS, 1200, whole(&p[8..11])),
    ])
}

fn expect_info(data: &[u8], t: u64, n: u64, a: u64, l: u64, m: u64) {
    let info = probe_ogg(data).expect("probe");
    assert_eq!(info.channels, 1);
    assert_eq!(info.sample_rate, 22_050);
    assert_eq!(info.block_sizes, (256, 2048));
    assert_eq!(info.total_samples, t, "T");
    assert_eq!(info.vorbis_len, n, "N");
    assert_eq!(info.played_start, a, "A");
    assert_eq!(info.library_len, l, "L");
    assert_eq!(info.played_len, m, "M");
}

fn frames_of(data: &[u8], range: OggRange) -> usize {
    decode_ogg(data, range, 1 << 20).expect("decode").frames()
}

/// Reads a source to its end in chunks of `chunk` frames.
fn drain(src: &mut dyn PcmSource, chunk: usize) -> Vec<f32> {
    let ch = usize::from(src.channels());
    let mut out = Vec::new();
    let mut buf = vec![0.0f32; chunk * ch];
    loop {
        let n = src.read(&mut buf);
        if n == 0 {
            break;
        }
        out.extend_from_slice(&buf[..n * ch]);
    }
    out
}

// ---------------------------------------------------------------------------------------
// §14.5: Ogg stream length and played range
// ---------------------------------------------------------------------------------------

#[test]
fn played_range_fresh_first_audio_page() {
    let data = case_fresh();
    // T = 10 × 128, N = G = 1200, A = 0, Y = 512, P = 0, L = 1200, M = 1200.
    expect_info(&data, 1280, 1200, 0, 1200, 1200);
    let info = probe_ogg(&data).unwrap();
    assert!(info.issues.is_empty(), "{:?}", info.issues);
    assert_eq!(info.setup_page, 1);
    assert_eq!(info.audio_packets, 11);
    assert_eq!(frames_of(&data, OggRange::Original), 1200);
    assert_eq!(frames_of(&data, OggRange::VorbisI), 1200);
    assert_eq!(frames_of(&data, OggRange::Decoded), 1280);
}

#[test]
fn played_range_granule_offset_and_empty_final_packet() {
    let data = case_offset_empty();
    // Y = 512, P = 582 − 512 = 70, L = 1350 − 70 = 1280, M = min(1280, 1280).
    expect_info(&data, 1280, 1280, 0, 1280, 1280);
    let info = probe_ogg(&data).unwrap();
    assert_eq!(info.zero_length_packets, 1);
    // The empty packet is the sole packet of the last page: not an issue (SN3a).
    assert!(info.issues.is_empty(), "{:?}", info.issues);
    // §3.3.3: decoding does not fail on the empty packet and nothing is zero-filled.
    assert_eq!(frames_of(&data, OggRange::Original), 1280);
    assert_eq!(frames_of(&data, OggRange::Decoded), 1280);
}

#[test]
fn played_range_audio_on_setup_page() {
    let data = case_audio_on_setup();
    // A = 384, Y = 3 × 128 (packet 5 not counted), P = 512, L = 688, M = 688.
    expect_info(&data, 1280, 1200, 384, 688, 688);
    assert_eq!(frames_of(&data, OggRange::Original), 688);
    assert_eq!(frames_of(&data, OggRange::VorbisI), 1200);
}

#[test]
fn first_audio_packet_spanning_setup_page() {
    // §3.3.2 census row: the first audio packet begins on the setup page and completes on
    // page a, so it is not counted and the next one's yield is not counted either.
    let (i, c, s) = headers(1);
    let first = audio_packet(1, false, false, false, 299); // 300 bytes
    let rest = short_packets(1, 10);
    let mut h = whole(&[c, s]);
    h.push((first[..255].to_vec(), false));
    let mut a = vec![(first[255..].to_vec(), true)];
    a.extend(whole(&rest[0..4])); // packets 2–5 on page a
    let data = stream(&[
        (FLAG_BOS, 0, whole(&[i])),
        (0, 0, h),
        (FLAG_CONTINUED, 512, a),
        (FLAG_EOS, 1200, whole(&rest[4..10])),
    ]);
    // Page a: packet 1 begins on h; packets 2–5 begin and complete on a, packet 2's
    // predecessor does not, so Y = 3 × 128 = 384 and P = 512 − 384 = 128.
    // A = 0 (no audio packet completes on h), L = 1200 − 128 = 1072, M = 1072: the last
    // 128 samples of N are not played.
    let info = probe_ogg(&data).unwrap();
    assert_eq!(info.played_start, 0);
    assert_eq!(info.library_len, 1072);
    assert_eq!(info.played_len, 1072);
    assert_eq!(info.vorbis_len, 1200);
    assert!(info.issues.is_empty(), "{:?}", info.issues);
    assert_eq!(frames_of(&data, OggRange::Original), 1072);
}

#[test]
fn mixed_block_sizes_total_matches_decoder() {
    // S, L, L, S, S, L: yields 0, 64+512, 1024, 512+64, 128, 64+512 = 2880.
    let (i, c, s) = headers(2);
    let p = [
        audio_packet(2, false, false, false, 0),
        audio_packet(2, true, false, true, 0),
        audio_packet(2, true, true, false, 0),
        audio_packet(2, false, false, false, 0),
        audio_packet(2, false, false, false, 0),
        audio_packet(2, true, false, true, 0),
    ];
    let data = stream(&[
        (FLAG_BOS, 0, whole(&[i])),
        (0, 0, whole(&[c, s])),
        (0, 576, whole(&p[0..2])),
        (FLAG_EOS, 2880, whole(&p[2..6])),
    ]);
    let info = probe_ogg(&data).unwrap();
    assert_eq!(info.channels, 2);
    assert_eq!(info.total_samples, 2880);
    // Y: packet 2 counted (packet 1 also on page a) = 576, P = 0, L = 2880.
    assert_eq!(info.played_len, 2880);
    let pcm = decode_ogg(&data, OggRange::Decoded, 1 << 20).unwrap();
    assert_eq!(pcm.channels, 2);
    assert_eq!(pcm.frames(), 2880);
    assert_eq!(pcm.samples.len(), 5760);
}

#[test]
fn streaming_source_matches_buffer_and_rewinds() {
    for data in [case_fresh(), case_offset_empty(), case_audio_on_setup()] {
        let info = probe_ogg(&data).unwrap();
        let buffered = decode_ogg(&data, OggRange::Original, 1 << 20).unwrap();
        let mut src = VorbisSource::new(Arc::from(data.as_slice()), OggRange::Original).unwrap();
        assert_eq!(src.len_frames(), Some(info.played_len));
        assert_eq!(src.sample_rate(), 22_050);
        for chunk in [1, 7, 100, 5000] {
            assert!(src.rewind());
            let got = drain(&mut src, chunk);
            assert_eq!(got.len() as u64, info.played_len, "chunk {chunk}");
            assert_eq!(&got[..], &buffered.samples[..]);
        }
    }
}

#[test]
fn buffer_limit_is_checked_before_decoding() {
    let data = case_fresh();
    assert!(matches!(
        decode_ogg(&data, OggRange::Original, 100),
        Err(DecodeError::LimitExceeded { value: 1200, .. })
    ));
}

// ---------------------------------------------------------------------------------------
// Hostile Ogg input
// ---------------------------------------------------------------------------------------

#[test]
fn ogg_structural_faults_are_errors() {
    let good = case_fresh();
    // Corrupt one body byte: CRC mismatch.
    let mut bad = good.clone();
    let last = bad.len() - 1;
    bad[last] ^= 0x40;
    assert!(matches!(probe_ogg(&bad), Err(DecodeError::OggPage { .. })));
    // Truncated mid-page.
    assert!(matches!(
        probe_ogg(&good[..good.len() - 3]),
        Err(DecodeError::OggPage { .. })
    ));
    // Only the capture pattern.
    assert!(probe_ogg(b"OggS").is_err());
    // A second logical stream.
    let other = build_page(FLAG_BOS | FLAG_EOS, 0, 0xBEEF, 0, &[1], &[0]).unwrap();
    let mut two = good.clone();
    two.extend(other);
    assert!(matches!(probe_ogg(&two), Err(DecodeError::OggPage { .. })));
    // Fewer than three packets.
    let (i, _, _) = headers(1);
    let one = stream(&[(FLAG_BOS | FLAG_EOS, 0, whole(&[i]))]);
    assert!(matches!(
        probe_ogg(&one),
        Err(DecodeError::VorbisHeader { .. })
    ));
}

#[test]
fn ogg_census_departures_are_issues() {
    let mut data = case_fresh();
    data.extend_from_slice(b"xyz");
    let info = probe_ogg(&data).unwrap();
    assert_eq!(info.issues, vec![OggIssue::TrailingBytes { len: 3 }]);
    // Decoding is unaffected.
    assert_eq!(frames_of(&data, OggRange::Original), 1200);
}

#[test]
fn three_channels_unsupported() {
    let i = ident_header(3, 22_050, BS0, BS1);
    let data = stream(&[
        (FLAG_BOS, 0, whole(&[i])),
        (FLAG_EOS, 0, whole(&[comment_header(), setup_header()])),
    ]);
    assert!(matches!(
        probe_ogg(&data),
        Err(DecodeError::Unsupported { value: 3, .. })
    ));
}

#[test]
fn hostile_setup_counts_are_rejected() {
    // A codebook claiming 2^24 − 1 ordered entries: rejected before any allocation.
    let mut w = BitWriter::default();
    w.put(0, 8)
        .put(0x56_4342, 24)
        .put(1, 16)
        .put(0xFF_FFFF, 24)
        .put(1, 1)
        .put(0, 5);
    w.put(0xFF_FFFF, 24);
    let mut setup = vec![5];
    setup.extend_from_slice(b"vorbis");
    setup.extend_from_slice(&w.bytes);
    let (i, c, _) = headers(1);
    let data = stream(&[
        (FLAG_BOS, 0, whole(&[i])),
        (FLAG_EOS, 0, whole(&[c, setup])),
    ]);
    assert!(matches!(
        probe_ogg(&data),
        Err(DecodeError::LimitExceeded { .. })
    ));
}

// ---------------------------------------------------------------------------------------
// §14.2: WAV payloads
// ---------------------------------------------------------------------------------------

/// The §14.2 record: 0x0A005678, a 48-byte mono 16-bit 22,050 Hz WAV with samples +1000
/// and −1000, storage format 3.
fn record_14_2() -> Vec<u8> {
    let mut r = Vec::new();
    r.extend_from_slice(&[0x78, 0x56, 0x00, 0x0A, 0x30, 0, 0, 0]);
    r.extend_from_slice(b"RIFF\x28\x00\x00\x00WAVE");
    r.extend_from_slice(b"fmt \x10\x00\x00\x00");
    r.extend_from_slice(&[
        0x01, 0x00, 0x01, 0x00, 0x22, 0x56, 0x00, 0x00, 0x44, 0xAC, 0x00, 0x00, 0x02, 0x00, 0x10,
        0x00,
    ]);
    r.extend_from_slice(b"data\x04\x00\x00\x00\xE8\x03\x18\xFC");
    r.extend_from_slice(&[3, 0, 0, 0, 0, 0, 0, 0]);
    assert_eq!(r.len(), 64);
    r
}

/// Builds a WAV from chunks (id, body); pads odd bodies.
fn wav(chunks: &[(&[u8; 4], Vec<u8>)]) -> Vec<u8> {
    let mut body = b"WAVE".to_vec();
    for (id, data) in chunks {
        body.extend_from_slice(*id);
        body.extend_from_slice(&(data.len() as u32).to_le_bytes());
        body.extend_from_slice(data);
        if data.len() % 2 == 1 {
            body.push(0);
        }
    }
    let mut out = b"RIFF".to_vec();
    out.extend_from_slice(&(body.len() as u32).to_le_bytes());
    out.extend(body);
    out
}

fn fmt(channels: u16, rate: u32, bits: u16) -> Vec<u8> {
    let align = channels * bits / 8;
    let mut v = Vec::new();
    v.extend_from_slice(&1u16.to_le_bytes());
    v.extend_from_slice(&channels.to_le_bytes());
    v.extend_from_slice(&rate.to_le_bytes());
    v.extend_from_slice(&(rate * u32::from(align)).to_le_bytes());
    v.extend_from_slice(&align.to_le_bytes());
    v.extend_from_slice(&bits.to_le_bytes());
    v
}

fn pcm_of(d: Decoded) -> Pcm {
    match d {
        Decoded::Pcm(p) => p,
        Decoded::NotAudio(n) => panic!("not audio: {n:?}"),
    }
}

#[test]
fn wav_record_14_2() {
    let pcm = pcm_of(decode_record(&record_14_2()).unwrap());
    assert_eq!(pcm.sample_rate, 22_050);
    assert_eq!(pcm.channels, 1);
    assert_eq!(&pcm.samples[..], &[1000.0 / 32_768.0, -1000.0 / 32_768.0]);
    let f = probe_wav(&record_14_2()[8..56]).unwrap();
    assert!(f.issues.is_empty(), "{:?}", f.issues);
    assert_eq!(f.frames(), 2);
}

#[test]
fn wav_with_odd_junk_chunk_first() {
    let data = vec![0xE8, 0x03, 0x18, 0xFC];
    let w = wav(&[
        (b"JUNK", vec![0x55]),
        (b"fmt ", fmt(1, 22_050, 16)),
        (b"data", data),
    ]);
    let f = probe_wav(&w).unwrap();
    assert!(f.issues.is_empty(), "{:?}", f.issues);
    let pcm = pcm_of(decode_payload(&w).unwrap());
    assert_eq!(&pcm.samples[..], &[1000.0 / 32_768.0, -1000.0 / 32_768.0]);
}

#[test]
fn wav_8_and_24_bit_stereo_and_ignored_smpl() {
    let w8 = wav(&[
        (b"fmt ", fmt(2, 8_000, 8)),
        (b"data", vec![0, 255, 128, 64]),
        (b"smpl", vec![0; 60]),
    ]);
    let pcm = pcm_of(decode_payload(&w8).unwrap());
    assert_eq!(pcm.channels, 2);
    assert_eq!(pcm.frames(), 2);
    assert_eq!(&pcm.samples[..], &[-1.0, 127.0 / 128.0, 0.0, -0.5]);

    let w24 = wav(&[
        (b"fmt ", fmt(2, 48_000, 24)),
        (
            b"data",
            vec![0, 0, 0x80, 0xFF, 0xFF, 0x7F, 0, 0, 0x40, 1, 0, 0],
        ),
    ]);
    let pcm = pcm_of(decode_payload(&w24).unwrap());
    assert_eq!(pcm.frames(), 2);
    assert_eq!(pcm.samples[0], -1.0);
    assert_eq!(pcm.samples[1], 8_388_607.0 / 8_388_608.0);
    assert_eq!(pcm.samples[2], 0.5);
    assert_eq!(pcm.samples[3], 1.0 / 8_388_608.0);
}

#[test]
fn wav_faults() {
    // data chunk shorter than declared.
    let mut w = wav(&[(b"fmt ", fmt(1, 22_050, 16)), (b"data", vec![0; 4])]);
    let n = w.len();
    w[n - 8] = 8; // declared 8, 4 present
    assert!(matches!(probe_wav(&w), Err(DecodeError::Wave { .. })));
    // Misaligned data (3 bytes of 16-bit mono).
    let w = wav(&[(b"fmt ", fmt(1, 22_050, 16)), (b"data", vec![0; 3])]);
    assert!(matches!(probe_wav(&w), Err(DecodeError::Wave { .. })));
    // Non-PCM tag.
    let mut f = fmt(1, 22_050, 16);
    f[0] = 3;
    let w = wav(&[(b"fmt ", f), (b"data", vec![0; 4])]);
    assert!(matches!(
        probe_wav(&w),
        Err(DecodeError::Unsupported { value: 3, .. })
    ));
    // 32-bit.
    let w = wav(&[(b"fmt ", fmt(1, 22_050, 32)), (b"data", vec![0; 4])]);
    assert!(matches!(
        probe_wav(&w),
        Err(DecodeError::Unsupported { value: 32, .. })
    ));
    // Inconsistent block align.
    let mut f = fmt(2, 22_050, 16);
    f[12] = 2;
    let w = wav(&[(b"fmt ", f), (b"data", vec![0; 4])]);
    assert!(matches!(probe_wav(&w), Err(DecodeError::Wave { .. })));
    // No data.
    let w = wav(&[(b"fmt ", fmt(1, 22_050, 16))]);
    assert!(probe_wav(&w).is_err());
    // Short fmt.
    let w = wav(&[(b"fmt ", vec![1, 0, 1, 0]), (b"data", vec![0; 2])]);
    assert!(probe_wav(&w).is_err());
}

#[test]
fn wav_census_departures_are_issues() {
    let mut w = wav(&[(b"fmt ", fmt(1, 22_050, 16)), (b"data", vec![0; 4])]);
    w[4] = w[4].wrapping_add(2); // RIFF size off by 2
    w.extend_from_slice(&[1, 2, 3]); // 3 trailing bytes the walk cannot use
    let f = probe_wav(&w).unwrap();
    assert_eq!(f.issues.len(), 2, "{:?}", f.issues);
}

#[test]
fn wav_source_streams_and_rewinds() {
    let samples: Vec<u8> = (0..200u16).flat_map(|v| (v * 100).to_le_bytes()).collect();
    let w: Arc<[u8]> = wav(&[(b"fmt ", fmt(2, 44_100, 16)), (b"data", samples)]).into();
    let buffered = pcm_of(decode_payload(&w).unwrap());
    let mut src = WavSource::new(Arc::clone(&w)).unwrap();
    assert_eq!(src.len_frames(), Some(100));
    let got = drain(&mut src, 7);
    assert_eq!(&got[..], &buffered.samples[..]);
    assert!(src.rewind());
    assert_eq!(drain(&mut src, 1000).len(), 200);
}

// ---------------------------------------------------------------------------------------
// Classification, probing and the buffered-or-streamed helper
// ---------------------------------------------------------------------------------------

#[test]
fn non_audio_is_a_category_not_an_error() {
    let mut dmap = b"RIFF\x10\x00\x00\x00DMAP".to_vec();
    dmap.extend_from_slice(&[0; 8]);
    assert_eq!(
        decode_payload(&dmap).unwrap(),
        Decoded::NotAudio(NotAudio::DirectMusic)
    );
    assert_eq!(
        probe_payload(&dmap).unwrap(),
        Probe::NotAudio(NotAudio::DirectMusic)
    );
    assert_eq!(
        decode_payload(b"ID3\x03 garbage").unwrap(),
        Decoded::NotAudio(NotAudio::UnknownContainer)
    );
    assert_eq!(
        decode_payload(&[]).unwrap(),
        Decoded::NotAudio(NotAudio::UnknownContainer)
    );
    assert!(matches!(
        open_payload(Arc::from(dmap.as_slice()), &StreamPolicy::default()).unwrap(),
        Playable::NotAudio(NotAudio::DirectMusic)
    ));
}

#[test]
fn record_envelope_faults() {
    let mut r = record_14_2();
    r.pop();
    assert!(matches!(decode_record(&r), Err(DecodeError::Record(_))));
}

#[test]
fn probe_shape_and_ranges() {
    let data = case_audio_on_setup();
    let p = probe_payload(&data).unwrap();
    assert_eq!(p.shape(OggRange::Original), Some((688, 22_050, 1)));
    assert_eq!(p.shape(OggRange::VorbisI), Some((1200, 22_050, 1)));
    let opts = DecodeOptions {
        range: OggRange::VorbisI,
        ..DecodeOptions::default()
    };
    assert_eq!(
        pcm_of(decode_payload_with(&data, &opts).unwrap()).frames(),
        1200
    );
}

#[test]
fn helper_decides_by_length() {
    let data: Arc<[u8]> = case_fresh().into(); // 1200 frames at 22,050 Hz ≈ 54 ms
    let buffered = open_payload(Arc::clone(&data), &StreamPolicy::default()).unwrap();
    let Playable::Buffered(pcm) = buffered else {
        panic!("expected a buffer");
    };
    assert_eq!(pcm.frames(), 1200);

    let policy = StreamPolicy {
        stream_above_secs: 0.01,
        ..StreamPolicy::default()
    };
    let streamed = open_payload(Arc::clone(&data), &policy).unwrap();
    assert!(matches!(streamed, Playable::Streamed(_)));
    let mut src = streamed.into_source().unwrap();
    assert_eq!(src.len_frames(), Some(1200));
    assert_eq!(drain(src.as_mut(), 64).len(), 1200);

    // A buffer limit below the sound's size forces streaming whatever the duration.
    let policy = StreamPolicy {
        options: DecodeOptions {
            max_buffered_samples: 10,
            ..DecodeOptions::default()
        },
        ..StreamPolicy::default()
    };
    assert!(matches!(
        open_payload(Arc::clone(&data), &policy).unwrap(),
        Playable::Streamed(_)
    ));

    // WAV through a whole record.
    let p = open_record(&record_14_2(), &StreamPolicy::default()).unwrap();
    let mut src = p.into_source().unwrap();
    assert_eq!(drain(src.as_mut(), 1).len(), 2);
    assert!(src.rewind());
}

// ---------------------------------------------------------------------------------------
// Audible fixtures: non-silent, position-dependent samples
// ---------------------------------------------------------------------------------------

/// `float32_unpack` word of 1.0 (mantissa 2^20, exponent 768).
const F32_ONE: u32 = (768 << 21) | (1 << 20);

fn setup_bytes(w: BitWriter) -> Vec<u8> {
    let mut v = vec![5];
    v.extend_from_slice(b"vorbis");
    v.extend_from_slice(&w.bytes);
    v
}

/// A codebook of `entries` = 2^`len` entries, all of length `len`, no lookup.
fn book_scalar(w: &mut BitWriter, len: u32) {
    w.put(0x56_4342, 24)
        .put(1, 16)
        .put(1 << len, 24)
        .put(0, 1)
        .put(0, 1);
    for _ in 0..(1u32 << len) {
        w.put(len - 1, 5);
    }
    w.put(0, 4);
}

/// A one-dimensional VQ codebook: 2 entries of length 1, lookup 1 with the given minimum
/// and delta words, one value bit, multiplicands 0 and 1 (values `min`, `min + delta`).
fn book_vq(w: &mut BitWriter, min: u32, delta: u32) {
    w.put(0x56_4342, 24)
        .put(1, 16)
        .put(2, 24)
        .put(0, 1)
        .put(0, 1);
    w.put(0, 5).put(0, 5);
    w.put(1, 4).put(min, 32).put(delta, 32).put(0, 4).put(0, 1);
    w.put(0, 1).put(1, 1);
}

/// Time placeholder and one floor 1 with no partitions (multiplier 1, range bits 8).
fn time_and_flat_floor(w: &mut BitWriter) {
    w.put(0, 6).put(0, 16);
    w.put(0, 6).put(1, 16).put(0, 5).put(0, 2).put(8, 4);
}

/// One residue of `rtype` over 0..32, one partition of 32, one classification with class
/// book 0 and a cascade of bit 0 only, using `book`.
fn residue_one(w: &mut BitWriter, rtype: u32, book: u32) {
    w.put(0, 6)
        .put(rtype, 16)
        .put(0, 24)
        .put(32, 24)
        .put(31, 24)
        .put(0, 6)
        .put(0, 8);
    w.put(1, 3).put(0, 1).put(book, 8);
}

/// One mapping (one submap, no coupling) and the short (0) and long (1) modes, framing.
fn mapping_and_modes(w: &mut BitWriter) {
    w.put(0, 6)
        .put(0, 16)
        .put(0, 1)
        .put(0, 1)
        .put(0, 2)
        .put(0, 8)
        .put(0, 8)
        .put(0, 8);
    w.put(1, 6);
    w.put(0, 1).put(0, 16).put(0, 16).put(0, 8);
    w.put(1, 1).put(0, 16).put(0, 16).put(0, 8);
    w.put(1, 1);
}

/// A setup header whose audio packets decode to non-zero samples: codebook 0 scalar (class
/// book), codebook 1 VQ with `min`/`delta`, a flat floor 1 and a type-1 residue over the
/// first 32 coefficients.
fn audible_setup(min: u32, delta: u32) -> Vec<u8> {
    let mut w = BitWriter::default();
    w.put(1, 8);
    book_scalar(&mut w, 1);
    book_vq(&mut w, min, delta);
    time_and_flat_floor(&mut w);
    residue_one(&mut w, 1, 1);
    mapping_and_modes(&mut w);
    setup_bytes(w)
}

fn audible_headers(channels: u8) -> (Vec<u8>, Vec<u8>, Vec<u8>) {
    (
        ident_header(channels, 22_050, BS0, BS1),
        comment_header(),
        audible_setup(F32_ONE, F32_ONE),
    )
}

/// xorshift64.
fn next_rand(x: &mut u64) -> u64 {
    *x ^= *x << 13;
    *x ^= *x >> 7;
    *x ^= *x << 17;
    *x
}

/// An audio packet for [`audible_setup`]: a used floor with Y values and residue bits drawn
/// from `seed`, so every packet's output differs.
fn audible_packet(
    channels: u8,
    long: bool,
    prev_long: bool,
    next_long: bool,
    seed: u64,
    pad: usize,
) -> Vec<u8> {
    let mut x = seed.wrapping_mul(0x9E37_79B9_7F4A_7C15) | 1;
    let mut w = BitWriter::default();
    w.put(0, 1).put(u32::from(long), 1);
    if long {
        w.put(u32::from(prev_long), 1).put(u32::from(next_long), 1);
    }
    for _ in 0..channels {
        let y0 = 150 + (next_rand(&mut x) % 100) as u32;
        let y1 = 150 + (next_rand(&mut x) % 100) as u32;
        w.put(1, 1).put(y0, 8).put(y1, 8);
    }
    // Residue: one class word per channel, then 32 one-bit codewords per channel.
    for _ in 0..channels {
        w.put(0, 1);
    }
    for _ in 0..channels {
        w.put(next_rand(&mut x) as u32, 32);
    }
    let mut v = w.bytes;
    v.resize(v.len() + pad, 0);
    v
}

fn audible_short_packets(channels: u8, n: usize) -> Vec<Vec<u8>> {
    (0..n)
        .map(|i| audible_packet(channels, false, false, false, i as u64 + 1, 0))
        .collect()
}

/// The three §14.5 layouts with audible packets.
fn audible_cases() -> Vec<(&'static str, Vec<u8>)> {
    let (i, c, s) = audible_headers(1);
    let p = audible_short_packets(1, 11);
    let fresh = stream(&[
        (FLAG_BOS, 0, whole(std::slice::from_ref(&i))),
        (0, 0, whole(&[c.clone(), s.clone()])),
        (0, 512, whole(&p[0..5])),
        (FLAG_EOS, 1200, whole(&p[5..11])),
    ]);
    let offset_empty = stream(&[
        (FLAG_BOS, 0, whole(std::slice::from_ref(&i))),
        (0, 0, whole(&[c.clone(), s.clone()])),
        (0, 582, whole(&p[0..5])),
        (0, 1350, whole(&p[5..11])),
        (FLAG_EOS, 1350, whole(&[Vec::new()])),
    ]);
    let mut h = whole(&[c, s]);
    h.extend(whole(&p[0..4]));
    let on_setup = stream(&[
        (FLAG_BOS, 0, whole(&[i])),
        (0, 384, h),
        (0, 896, whole(&p[4..8])),
        (FLAG_EOS, 1200, whole(&p[8..11])),
    ]);
    vec![
        ("fresh", fresh),
        ("offset and empty packet", offset_empty),
        ("audio on the setup page", on_setup),
    ]
}

/// A stereo stream with mixed block sizes and its first five packets on the setup page, so
/// the lead A spans long blocks.
fn audible_mixed_on_setup() -> Vec<u8> {
    let (i, c, s) = audible_headers(2);
    let flags = [
        (false, false, false),
        (true, false, true),
        (true, true, true),
        (true, true, false),
        (false, false, false),
        (true, false, true),
        (true, true, false),
        (false, false, false),
        (false, false, false),
        (true, false, false),
        (false, false, false),
    ];
    let p: Vec<Vec<u8>> = flags
        .iter()
        .enumerate()
        .map(|(n, &(l, pl, nl))| audible_packet(2, l, pl, nl, 100 + n as u64, 0))
        .collect();
    let mut h = whole(&[c, s]);
    h.extend(whole(&p[0..5]));
    let probe = |pages: &[(u8, u64, Vec<Part>)]| probe_ogg(&stream(pages)).unwrap();
    // Granules: D(h) on page h, D(a) on page a, then the total.
    let draft = [
        (FLAG_BOS, 0, whole(std::slice::from_ref(&i))),
        (0, 0, h.clone()),
        (0, 0, whole(&p[5..8])),
        (FLAG_EOS, 0, whole(&p[8..11])),
    ];
    let t = probe(&draft).total_samples;
    let a = probe(&draft).played_start;
    stream(&[
        (FLAG_BOS, 0, whole(&[i])),
        (0, a, h),
        (0, t - 600, whole(&p[5..8])),
        (FLAG_EOS, t - 100, whole(&p[8..11])),
    ])
}

fn frames_slice(pcm: &Pcm, start: u64, len: u64) -> &[f32] {
    let ch = pcm.channels as usize;
    &pcm.samples[start as usize * ch..(start + len) as usize * ch]
}

#[test]
fn audible_fixture_is_not_silent() {
    for (name, data) in audible_cases() {
        let pcm = decode_ogg(&data, OggRange::Decoded, 1 << 20).unwrap();
        let nonzero = pcm.samples.iter().filter(|s| **s != 0.0).count();
        assert!(
            nonzero > pcm.samples.len() / 2,
            "{name}: {nonzero} non-zero"
        );
        assert!(pcm.samples.iter().all(|s| (-1.0..=1.0).contains(s)));
        // Position-dependent: no two 128-sample blocks are equal.
        let blocks: Vec<&[f32]> = pcm.samples.chunks(128).collect();
        for w in blocks.windows(2) {
            assert_ne!(w[0], w[1], "{name}");
        }
    }
}

#[test]
fn original_range_is_decoded_samples_a_onwards() {
    let mut cases = audible_cases();
    cases.push(("mixed blocks on the setup page", audible_mixed_on_setup()));
    for (name, data) in cases {
        let info = probe_ogg(&data).unwrap();
        let decoded = decode_ogg(&data, OggRange::Decoded, 1 << 22).unwrap();
        assert_eq!(decoded.frames() as u64, info.total_samples, "{name}");
        let original = decode_ogg(&data, OggRange::Original, 1 << 22).unwrap();
        assert_eq!(
            &original.samples[..],
            frames_slice(&decoded, info.played_start, info.played_len),
            "{name}: Original = Decoded[A .. A + M]"
        );
        let vorbis = decode_ogg(&data, OggRange::VorbisI, 1 << 22).unwrap();
        assert_eq!(
            &vorbis.samples[..],
            frames_slice(&decoded, 0, info.vorbis_len),
            "{name}"
        );
        if info.played_start > 0 {
            // The offset matters: the first M decoded samples are different.
            assert_ne!(
                &original.samples[..],
                frames_slice(&decoded, 0, info.played_len),
                "{name}"
            );
        }
        // The streaming source gives the same samples, after every rewind too.
        let mut src = VorbisSource::new(Arc::from(data.as_slice()), OggRange::Original).unwrap();
        for chunk in [5000, 1, 7, 333] {
            let got = drain(&mut src, chunk);
            assert_eq!(&got[..], &original.samples[..], "{name}: chunk {chunk}");
            assert!(src.rewind());
        }
    }
    // The §14.5 row 3 values.
    let (_, data) = &audible_cases()[2];
    let info = probe_ogg(data).unwrap();
    assert_eq!((info.played_start, info.played_len), (384, 688));
}

#[test]
fn mixed_on_setup_has_a_long_lead() {
    let info = probe_ogg(&audible_mixed_on_setup()).unwrap();
    // Yields: 0, 64 + 512, 1024, 1024, 512 + 64 → A = 3200.
    assert_eq!(info.played_start, 3200);
    assert!(info.issues.is_empty(), "{:?}", info.issues);
}

/// A mono stream of audible packets with the given block sizes (true = long; window flags
/// follow the neighbours), the first `on_setup` of them on the setup page, two on page a
/// and the rest on the last page. Granules follow the decoded samples, so A = D(h).
fn audible_blocks_on_setup(blocks: &[bool], on_setup: usize) -> Vec<u8> {
    let (i, c, s) = audible_headers(1);
    let p: Vec<Vec<u8>> = (0..blocks.len())
        .map(|n| {
            let prev = n > 0 && blocks[n - 1];
            let next = n + 1 < blocks.len() && blocks[n + 1];
            audible_packet(1, blocks[n], prev, next, 500 + n as u64, 0)
        })
        .collect();
    let mut h = whole(&[c, s]);
    h.extend(whole(&p[..on_setup]));
    let a_end = on_setup + 2;
    let pages = |ga: u64, gh: u64, gl: u64| {
        stream(&[
            (FLAG_BOS, 0, whole(std::slice::from_ref(&i))),
            (0, gh, h.clone()),
            (0, ga, whole(&p[on_setup..a_end])),
            (FLAG_EOS, gl, whole(&p[a_end..])),
        ])
    };
    let draft = probe_ogg(&pages(0, 0, 0)).unwrap();
    let (t, a) = (draft.total_samples, draft.played_start);
    // D(a): the yields of the two packets on page a, by the quarter-block arithmetic.
    let size = |long: bool| if long { 2048u64 } else { 256 };
    let d_a: u64 = (on_setup..a_end)
        .map(|n| size(blocks[n - 1]) / 4 + size(blocks[n]) / 4)
        .sum();
    pages(a + d_a, a, t)
}

#[test]
fn played_range_after_a_long_block_followed_by_a_short_one() {
    // Regression: the decoder library moves the output boundary between a long block and
    // a following short one by (long − short) / 4 samples. A decoder primed on such a long
    // block placed the whole played range 448 samples early (install payloads with
    // A = 576, 832, 1216, 1600, …). Every lead length over this block pattern must give
    // Decoded[A .. A + M], buffered, streamed and after a rewind.
    let blocks = [
        false, true, true, true, false, false, true, false, true, true, false, false, false, true,
        false, false,
    ];
    for on_setup in 2..=12 {
        let data = audible_blocks_on_setup(&blocks, on_setup);
        let info = probe_ogg(&data).unwrap();
        assert!(info.issues.is_empty(), "{on_setup}: {:?}", info.issues);
        assert!(info.played_start > 0 && info.played_len > 0, "{on_setup}");
        let decoded = decode_ogg(&data, OggRange::Decoded, 1 << 22).unwrap();
        assert_eq!(decoded.frames() as u64, info.total_samples, "{on_setup}");
        let want = frames_slice(&decoded, info.played_start, info.played_len);
        let original = decode_ogg(&data, OggRange::Original, 1 << 22).unwrap();
        assert_eq!(
            &original.samples[..],
            want,
            "{on_setup}: A = {}",
            info.played_start
        );
        let mut src = VorbisSource::new(Arc::from(data.as_slice()), OggRange::Original).unwrap();
        for chunk in [4096, 3, 100] {
            assert_eq!(
                &drain(&mut src, chunk)[..],
                want,
                "{on_setup}: chunk {chunk}"
            );
            assert!(src.rewind());
        }
    }
    // The case the regression came from: three long blocks then a short one on the setup
    // page, A = 0 + 576 + 1024 + 1024.
    let info = probe_ogg(&audible_blocks_on_setup(&blocks, 4)).unwrap();
    assert_eq!(info.played_start, 2624);
}

// ---------------------------------------------------------------------------------------
// Hostile setup headers the decoder library would accept but cannot decode
// ---------------------------------------------------------------------------------------

/// A mono stream with `setup` and six short audio packets of `packet`.
fn stream_with_setup(setup: Vec<u8>, packet: &[u8]) -> Vec<u8> {
    let (i, c, _) = headers(1);
    let p: Vec<Vec<u8>> = (0..6).map(|_| packet.to_vec()).collect();
    stream(&[
        (FLAG_BOS, 0, whole(&[i])),
        (0, 0, whole(&[c, setup])),
        (FLAG_EOS, 640, whole(&p)),
    ])
}

/// Probing must fail with a header error and decoding must fail without panicking.
fn assert_rejected(data: &[u8], what: &str) {
    match probe_ogg(data) {
        Err(DecodeError::VorbisHeader { .. } | DecodeError::Unsupported { .. }) => {}
        other => panic!("{what}: probe gave {other:?}"),
    }
    assert!(decode_payload(data).is_err(), "{what}");
    assert!(
        open_payload(Arc::from(data), &StreamPolicy::default()).is_err(),
        "{what}"
    );
}

#[test]
fn non_finite_lookup_values_are_rejected() {
    // Exponent field 1023: the scale 2^235 overflows f32 (lewton would compute 0 × inf =
    // NaN for a zero mantissa, inf otherwise).
    for min in [0x7FE0_0000, 0x7FE0_0001, 0x7FFF_FFFF, 0xFFFF_FFFF] {
        let data = stream_with_setup(
            audible_setup(min, 0),
            &audible_packet(1, false, false, false, 1, 0),
        );
        assert_rejected(&data, &format!("minimum {min:#X}"));
    }
    // A finite value above f32::MAX.
    let data = stream_with_setup(
        audible_setup(F32_ONE, ((788 + 108) << 21) | 0x1F_FFFF),
        &audible_packet(1, false, false, false, 1, 0),
    );
    assert_rejected(&data, "delta above f32::MAX");
}

#[test]
fn overflowing_samples_become_finite() {
    // Minimum and delta just below f32::MAX: min + delta overflows to inf inside the
    // decoder, and the transform turns inf − inf into NaN. Every delivered sample must
    // still be finite and in [−1, 1].
    let huge = ((788 + 106) << 21) | 0x1F_FFFF;
    let data = stream_with_setup(
        audible_setup(huge, huge),
        &audible_packet(1, false, false, false, 1, 0),
    );
    let pcm = decode_ogg(&data, OggRange::Decoded, 1 << 20).unwrap();
    assert_eq!(pcm.frames(), 640);
    assert!(
        pcm.samples
            .iter()
            .all(|s| s.is_finite() && (-1.0..=1.0).contains(s)),
        "{:?}",
        &pcm.samples[..8]
    );
    let mut src = VorbisSource::new(Arc::from(data.as_slice()), OggRange::Decoded).unwrap();
    let got = drain(&mut src, 100);
    assert!(
        got.iter()
            .all(|s| s.is_finite() && (-1.0..=1.0).contains(s))
    );
}

#[test]
fn codebook_without_used_entries_is_read_without_panic_or_rejected() {
    // Sparse codebook 0 with all four entries unused, used as the residue class book. The
    // library would panic on the read; the header is patched so the read takes entry 0.
    let mut w = BitWriter::default();
    w.put(1, 8);
    w.put(0x56_4342, 24)
        .put(1, 16)
        .put(4, 24)
        .put(0, 1)
        .put(1, 1);
    w.put(0, 1).put(0, 1).put(0, 1).put(0, 1).put(0, 4);
    book_vq(&mut w, F32_ONE, F32_ONE);
    time_and_flat_floor(&mut w);
    residue_one(&mut w, 1, 1);
    mapping_and_modes(&mut w);
    let data = stream_with_setup(
        setup_bytes(w),
        &audible_packet(1, false, false, false, 1, 0),
    );
    probe_payload(&data).expect("a placeholder book is legal in the header");
    let pcm = decode_ogg(&data, OggRange::Decoded, 1 << 20).expect("reads entry 0");
    assert!(pcm.samples.iter().all(|s| s.is_finite()));
    let mut src = VorbisSource::new(Arc::from(data.as_slice()), OggRange::Decoded).unwrap();
    assert!(drain(&mut src, 100).iter().all(|s| s.is_finite()));

    // A codebook with zero entries.
    let mut w = BitWriter::default();
    w.put(1, 8);
    w.put(0x56_4342, 24)
        .put(1, 16)
        .put(0, 24)
        .put(0, 1)
        .put(0, 1)
        .put(0, 4);
    book_vq(&mut w, F32_ONE, F32_ONE);
    time_and_flat_floor(&mut w);
    residue_one(&mut w, 1, 1);
    mapping_and_modes(&mut w);
    let data = stream_with_setup(
        setup_bytes(w),
        &audible_packet(1, false, false, false, 1, 0),
    );
    assert_rejected(&data, "codebook with no entries");
}

#[test]
fn unread_codebook_without_used_entries_is_accepted() {
    // The audible setup plus a third, sparse codebook whose four entries are all unused and
    // that no floor or residue reads: a legal placeholder book (two streams of the install
    // carry one). The stream must decode exactly like the audible setup alone.
    let mut w = BitWriter::default();
    w.put(2, 8);
    book_scalar(&mut w, 1);
    book_vq(&mut w, F32_ONE, F32_ONE);
    w.put(0x56_4342, 24)
        .put(1, 16)
        .put(4, 24)
        .put(0, 1)
        .put(1, 1);
    w.put(0, 1).put(0, 1).put(0, 1).put(0, 1).put(0, 4);
    time_and_flat_floor(&mut w);
    residue_one(&mut w, 1, 1);
    mapping_and_modes(&mut w);
    let packet = audible_packet(1, false, false, false, 1, 0);
    let with_placeholder = stream_with_setup(setup_bytes(w), &packet);
    let plain = stream_with_setup(audible_setup(F32_ONE, F32_ONE), &packet);
    let a = decode_ogg(&with_placeholder, OggRange::Decoded, 1 << 20).unwrap();
    let b = decode_ogg(&plain, OggRange::Decoded, 1 << 20).unwrap();
    assert_eq!(a.frames(), b.frames());
    assert_eq!(a.samples, b.samples);
}

#[test]
fn residue_book_without_lookup_is_rejected() {
    // The residue's stage book is codebook 0, which has no lookup (VORBIS §3.3: an error).
    let mut w = BitWriter::default();
    w.put(1, 8);
    book_scalar(&mut w, 1);
    book_vq(&mut w, F32_ONE, F32_ONE);
    time_and_flat_floor(&mut w);
    residue_one(&mut w, 1, 0);
    mapping_and_modes(&mut w);
    let data = stream_with_setup(
        setup_bytes(w),
        &audible_packet(1, false, false, false, 1, 0),
    );
    assert_rejected(&data, "residue book without lookup");
}

#[test]
fn residue_cascade_bit_7_is_rejected() {
    // The library reads only the books of cascade bits 0–6, so with bit 7 set its header
    // walk and ours fall 8 bits apart. This header is laid out so that both walks would
    // accept it (an extra residue absorbs the difference); its audio packets crashed the
    // library before the walk rejected bit 7.
    let mut w = BitWriter::default();
    w.put(17, 8); // 18 codebooks: 0 scalar, 1 VQ, 2–17 scalar
    book_scalar(&mut w, 1);
    book_vq(&mut w, 0, 0);
    for _ in 2..18 {
        book_scalar(&mut w, 1);
    }
    time_and_flat_floor(&mut w);
    w.put(1, 6); // two residues
    // Residue 0: cascade 0x80 only (low 0, high flag, high 0b10000), book 1 for bit 7.
    w.put(0, 16)
        .put(0, 24)
        .put(64, 24)
        .put(0, 24)
        .put(0, 6)
        .put(0, 8);
    w.put(0, 3).put(1, 1).put(0b1_0000, 5);
    w.put(1, 8);
    // Residue 1: empty range, class book 17, no cascade.
    w.put(0, 16)
        .put(0, 24)
        .put(0, 24)
        .put(0, 24)
        .put(0, 6)
        .put(17, 8)
        .put(0, 3)
        .put(0, 1);
    w.put(0, 6)
        .put(0, 16)
        .put(0, 1)
        .put(0, 1)
        .put(0, 2)
        .put(0, 8)
        .put(0, 8)
        .put(0, 8);
    w.put(0, 6).put(0, 1).put(0, 16).put(0, 16).put(0, 8);
    w.put(1, 1);
    let setup = setup_bytes(w);
    let mut pk = BitWriter::default();
    pk.put(0, 1).put(1, 1).put(0, 8).put(0, 8);
    let mut packet = pk.bytes;
    packet.resize(64, 0x55);
    let data = stream_with_setup(setup, &packet);
    assert!(matches!(
        probe_ogg(&data),
        Err(DecodeError::VorbisHeader { reason }) if reason.contains("bit 7")
    ));
    assert_rejected(&data, "residue cascade bit 7");
}

/// A setup header with one VQ codebook and floor 0 (`order`, `amplitude_bits`).
fn floor0_setup(order: u32, amplitude_bits: u32) -> Vec<u8> {
    let mut w = BitWriter::default();
    w.put(0, 8);
    book_vq(&mut w, F32_ONE, F32_ONE);
    w.put(0, 6).put(0, 16);
    w.put(0, 6)
        .put(0, 16)
        .put(order, 8)
        .put(8000, 16)
        .put(256, 16)
        .put(amplitude_bits, 6)
        .put(1, 8)
        .put(0, 4)
        .put(0, 8);
    w.put(0, 6)
        .put(0, 16)
        .put(0, 24)
        .put(0, 24)
        .put(0, 24)
        .put(0, 6)
        .put(0, 8)
        .put(0, 3)
        .put(0, 1);
    w.put(0, 6)
        .put(0, 16)
        .put(0, 1)
        .put(0, 1)
        .put(0, 2)
        .put(0, 8)
        .put(0, 8)
        .put(0, 8);
    w.put(0, 6).put(0, 1).put(0, 16).put(0, 16).put(0, 8);
    w.put(1, 1);
    setup_bytes(w)
}

#[test]
fn floor_0_is_unsupported() {
    // Order 1 underflowed and indexed out of bounds in the library; amplitude bits 40
    // overflowed a shift. Floor 0 is rejected outright; order 4 with 8 bits too.
    for (order, bits) in [(1, 8), (4, 40), (4, 8)] {
        let mut pk = BitWriter::default();
        pk.put(0, 1);
        let mut left = bits;
        while left > 0 {
            let n = left.min(16);
            pk.put(0xFFFF, n);
            left -= n;
        }
        pk.put(0, 1);
        let mut packet = pk.bytes;
        packet.resize(32, 0xAA);
        let data = stream_with_setup(floor0_setup(order, bits), &packet);
        assert!(
            matches!(
                probe_ogg(&data),
                Err(DecodeError::Unsupported { value: 0, .. })
            ),
            "order {order}, bits {bits}"
        );
        assert_rejected(&data, "floor 0");
    }
}

// ---------------------------------------------------------------------------------------
// §3.3.3 OPEN note: cases steps 1–5 do not cover. These pin Tessera's conservative
// behaviour; update them when the spec answers.
// ---------------------------------------------------------------------------------------

#[test]
fn open_case_1_page_a_without_granule() {
    // Page a completes no packet (granule −1): P = 0, as if page a did not exist.
    let (i, c, s) = headers(1);
    let first = audio_packet(1, false, false, false, 299);
    let rest = short_packets(1, 10);
    let mut last = vec![(first[255..].to_vec(), true)];
    last.extend(whole(&rest));
    let data = stream(&[
        (FLAG_BOS, 0, whole(&[i])),
        (0, 0, whole(&[c, s])),
        (0, u64::MAX, vec![(first[..255].to_vec(), false)]),
        (FLAG_CONTINUED | FLAG_EOS, 1200, last),
    ]);
    // T = 10 × 128, G = 1200, A = 0, P = 0, L = 1200, M = 1200.
    expect_info(&data, 1280, 1200, 0, 1200, 1200);
    let info = probe_ogg(&data).unwrap();
    assert!(info.issues.contains(&OggIssue::GranuleUnset { page: 2 }));
    assert_eq!(frames_of(&data, OggRange::Original), 1200);
}

#[test]
fn open_case_2_last_page_without_granule() {
    // The last page has granule −1: L = T and N = T.
    let (i, c, s) = headers(1);
    let p = short_packets(1, 11);
    let data = stream(&[
        (FLAG_BOS, 0, whole(&[i])),
        (0, 0, whole(&[c, s])),
        (0, 512, whole(&p[0..5])),
        (FLAG_EOS, u64::MAX, whole(&p[5..11])),
    ]);
    expect_info(&data, 1280, 1280, 0, 1280, 1280);
    assert_eq!(probe_ogg(&data).unwrap().last_granule, None);
    assert_eq!(frames_of(&data, OggRange::Original), 1280);
}

#[test]
fn open_case_3_zero_length_packet_on_page_a() {
    // Page a: [empty, p2, p3, p4] after p0, p1 on the setup page. The empty packet is
    // skipped and is not "the packet before" p2, so p2's yield is not counted:
    // Y = 2 × 128, P = 512 − 256 = 256, L = 1200 − 256 = 944, A = 128, M = 944.
    let (i, c, s) = headers(1);
    let p = short_packets(1, 11);
    let mut h = whole(&[c, s]);
    h.extend(whole(&p[0..2]));
    let mut a = whole(&[Vec::new()]);
    a.extend(whole(&p[2..5]));
    let data = stream(&[
        (FLAG_BOS, 0, whole(&[i])),
        (0, 128, h),
        (0, 512, a),
        (FLAG_EOS, 1200, whole(&p[5..11])),
    ]);
    expect_info(&data, 1280, 1200, 128, 944, 944);
    let info = probe_ogg(&data).unwrap();
    assert_eq!(info.zero_length_packets, 1);
    assert_eq!(info.issues, vec![OggIssue::ZeroLengthPacket { page: 2 }]);
    assert_eq!(frames_of(&data, OggRange::Original), 944);
}

#[test]
fn open_case_4_header_packet_after_the_headers() {
    let (i, c, s) = headers(1);
    let mut p = short_packets(1, 11);
    p[6] = vec![0x01, 0, 0];
    let data = stream(&[
        (FLAG_BOS, 0, whole(&[i])),
        (0, 0, whole(&[c, s])),
        (0, 512, whole(&p[0..5])),
        (FLAG_EOS, 1200, whole(&p[5..11])),
    ]);
    assert!(matches!(
        probe_ogg(&data),
        Err(DecodeError::VorbisAudio { packet: 9, .. })
    ));
    assert!(decode_payload(&data).is_err());
}

#[test]
fn open_case_5_no_page_after_the_setup_page() {
    // Setup header and every audio packet on the last page: A = D(h) = T, M = 0, reported
    // as NoPageAfterSetup. Playback delivers nothing and a loop cannot rewind.
    let (i, c, s) = headers(1);
    let mut last = whole(&[c, s]);
    last.extend(whole(&short_packets(1, 11)));
    let data = stream(&[(FLAG_BOS, 0, whole(&[i])), (FLAG_EOS, 1200, last)]);
    expect_info(&data, 1280, 1200, 1280, 1200, 0);
    let info = probe_ogg(&data).unwrap();
    assert_eq!(info.issues, vec![OggIssue::NoPageAfterSetup]);
    assert_eq!(frames_of(&data, OggRange::Original), 0);
    assert_eq!(frames_of(&data, OggRange::VorbisI), 1200);
    let mut src = VorbisSource::new(Arc::from(data.as_slice()), OggRange::Original).unwrap();
    let mut buf = [0.0f32; 64];
    assert_eq!(src.read(&mut buf), 0);
    assert!(!src.rewind());
    let mut src = open_payload(Arc::from(data.as_slice()), &StreamPolicy::default())
        .unwrap()
        .into_source()
        .unwrap();
    assert_eq!(src.read(&mut buf), 0);
    assert!(!src.rewind());
}

// ---------------------------------------------------------------------------------------
// Empty sources and public-struct robustness
// ---------------------------------------------------------------------------------------

#[test]
fn empty_wav_cannot_rewind() {
    let w: Arc<[u8]> = wav(&[(b"fmt ", fmt(1, 22_050, 16)), (b"data", Vec::new())]).into();
    assert_eq!(pcm_of(decode_payload(&w).unwrap()).frames(), 0);
    let mut src = WavSource::new(Arc::clone(&w)).unwrap();
    let mut buf = [0.0f32; 8];
    assert_eq!(src.read(&mut buf), 0);
    assert!(!src.rewind());
    let mut src = open_payload(w, &StreamPolicy::default())
        .unwrap()
        .into_source()
        .unwrap();
    assert!(!src.rewind());
}

#[test]
fn wave_format_with_zero_block_align_does_not_panic() {
    let f = WaveFormat {
        channels: 1,
        sample_rate: 1,
        bits_per_sample: 16,
        block_align: 0,
        data_offset: 0,
        data_len: 4,
        issues: vec![],
    };
    assert_eq!(f.frames(), 0);
    assert_eq!(Probe::Wave(f).shape(OggRange::Original), Some((0, 1, 1)));
}

#[test]
fn default_buffer_limit_matches_the_stream_threshold() {
    // 30 s of 192 kHz stereo; a buffered-policy sound never exceeds it.
    assert_eq!(DecodeOptions::DEFAULT_MAX_BUFFERED_SAMPLES, 11_520_000);
    let p = StreamPolicy::default();
    assert!(!p.should_stream(30 * 192_000, 192_000, 2));
    assert!(p.should_stream(30 * 192_000 + 1, 192_000, 2));
}

// ---------------------------------------------------------------------------------------
// Rich setup header (also the fuzz target's seed header): every decode path of floor 1,
// residues 0 and 2, lookups 1 and 2, sparse and ordered codebooks, submaps and coupling
// ---------------------------------------------------------------------------------------

/// A setup header that exercises most of the decoder: five codebooks (scalar, lookup 2 in
/// two dimensions, lookup 1 with sequence flag, sparse, ordered), two floors 1 (one with
/// two partition classes and subclass books, one flat), a residue 2 over 256 coefficients
/// with two classifications and a cascade of two stages, a residue 0, two mappings (one
/// with coupling for stereo, one with two submaps) and four modes. Kept in step with
/// `fuzz/fuzz_targets/audio_decode.rs`.
fn rich_setup(channels: u8) -> Vec<u8> {
    let mut w = BitWriter::default();
    w.put(4, 8); // 5 codebooks
    // 0: scalar, 8 entries of length 3.
    book_scalar(&mut w, 3);
    // 1: 2 dimensions, 16 entries of length 4, lookup 2, 3-bit values.
    w.put(0x56_4342, 24)
        .put(2, 16)
        .put(16, 24)
        .put(0, 1)
        .put(0, 1);
    for _ in 0..16 {
        w.put(3, 5);
    }
    w.put(2, 4)
        .put((767 << 21) | (1 << 20) | 0x8000_0000, 32)
        .put((764 << 21) | (1 << 20), 32)
        .put(2, 4)
        .put(0, 1);
    for i in 0..32 {
        w.put(i % 8, 3);
    }
    // 2: 1 dimension, 4 entries of length 2, lookup 1, sequence flag set.
    w.put(0x56_4342, 24)
        .put(1, 16)
        .put(4, 24)
        .put(0, 1)
        .put(0, 1);
    for _ in 0..4 {
        w.put(1, 5);
    }
    w.put(1, 4)
        .put(F32_ONE | 0x8000_0000, 32)
        .put((767 << 21) | (1 << 20), 32)
        .put(2, 4)
        .put(1, 1);
    for i in 0..4 {
        w.put(i, 3);
    }
    // 3: sparse, 8 entries, the even ones used with length 2, lookup 1.
    w.put(0x56_4342, 24)
        .put(1, 16)
        .put(8, 24)
        .put(0, 1)
        .put(1, 1);
    for e in 0..8 {
        if e % 2 == 0 {
            w.put(1, 1).put(1, 5);
        } else {
            w.put(0, 1);
        }
    }
    w.put(1, 4)
        .put((766 << 21) | (1 << 20), 32)
        .put((766 << 21) | (1 << 20), 32)
        .put(1, 4)
        .put(0, 1);
    for i in 0..8 {
        w.put(i % 4, 2);
    }
    // 4: ordered, 8 entries of lengths 1, 2, 3, 4, 5, 6, 7, 7; no lookup.
    w.put(0x56_4342, 24)
        .put(1, 16)
        .put(8, 24)
        .put(1, 1)
        .put(0, 5);
    w.put(1, 4)
        .put(1, 3)
        .put(1, 3)
        .put(1, 3)
        .put(1, 3)
        .put(1, 2)
        .put(2, 2);
    w.put(0, 4);
    // Time placeholder.
    w.put(0, 6).put(0, 16);
    // Two floors 1.
    w.put(1, 6);
    // Floor 0: partitions [class 0, class 1]; class 0: 2 dims, 1 subclass bit, master
    // book 0, subclass books none and 4; class 1: 3 dims, no subclasses, book 0;
    // multiplier 2, range bits 7.
    w.put(1, 16).put(2, 5).put(0, 4).put(1, 4);
    w.put(1, 3).put(1, 2).put(0, 8).put(0, 8).put(5, 8);
    w.put(2, 3).put(0, 2).put(1, 8);
    w.put(1, 2).put(7, 4);
    for x in [10, 40, 90, 20, 64] {
        w.put(x, 7);
    }
    // Floor 1: flat.
    w.put(1, 16).put(0, 5).put(0, 2).put(8, 4);
    // Two residues.
    w.put(1, 6);
    // Residue 0: type 2 over 0..256, partitions of 16, 2 classifications, class book 0;
    // class 0 cascade 0b011 (books 1, 2), class 1 cascade 0b1001 (books 3, 2).
    w.put(2, 16)
        .put(0, 24)
        .put(256, 24)
        .put(15, 24)
        .put(1, 6)
        .put(0, 8);
    w.put(0b011, 3).put(0, 1);
    w.put(0b001, 3).put(1, 1).put(0b00001, 5);
    w.put(1, 8).put(2, 8);
    w.put(3, 8).put(2, 8);
    // Residue 1: type 0 over 0..64, partitions of 8, 1 classification, class book 4,
    // cascade bit 0 with book 2.
    w.put(0, 16)
        .put(0, 24)
        .put(64, 24)
        .put(7, 24)
        .put(0, 6)
        .put(4, 8);
    w.put(1, 3).put(0, 1).put(2, 8);
    // Two mappings.
    w.put(1, 6);
    // Mapping 0: one submap, coupling 0/1 for stereo; floor 0, residue 0.
    w.put(0, 16).put(0, 1);
    if channels == 2 {
        w.put(1, 1).put(0, 8).put(0, 1).put(1, 1);
    } else {
        w.put(0, 1);
    }
    w.put(0, 2).put(0, 8).put(0, 8).put(0, 8);
    // Mapping 1: two submaps, channel c → submap c % 2; submap 0 floor 1 residue 1,
    // submap 1 floor 0 residue 0.
    w.put(0, 16).put(1, 1).put(1, 4).put(0, 1).put(0, 2);
    for c in 0..channels {
        w.put(u32::from(c % 2), 4);
    }
    w.put(0, 8).put(1, 8).put(1, 8);
    w.put(0, 8).put(0, 8).put(0, 8);
    // Four modes: (short, mapping 0), (long, 0), (short, 1), (long, 1).
    w.put(3, 6);
    for (long, mapping) in [(0, 0), (1, 0), (0, 1), (1, 1)] {
        w.put(long, 1).put(0, 16).put(0, 16).put(mapping, 8);
    }
    w.put(1, 1);
    setup_bytes(w)
}

#[test]
fn rich_setup_decodes_random_packets() {
    for channels in [1u8, 2] {
        let setup = rich_setup(channels);
        let modes = tessera_audio::decode::setup::walk_setup(&setup, channels).unwrap();
        assert_eq!(modes.block_flags, vec![false, true, false, true]);
        let mut x = 0x1234_5678_9ABC_DEF1u64;
        let modes: Vec<u8> = (0..40).map(|_| (next_rand(&mut x) % 4) as u8).collect();
        let packets: Vec<Vec<u8>> = (0..40)
            .map(|n| {
                let mut p: Vec<u8> = (0..300).map(|_| next_rand(&mut x) as u8).collect();
                // Header bit 0, the mode, and for a long block window flags that match
                // its neighbours (the library rejects a mismatch).
                let long = |m: u8| m % 2 == 1;
                let prev = n > 0 && long(modes[n - 1]);
                let next = n + 1 < modes.len() && long(modes[n + 1]);
                p[0] = (p[0] & !0x1F)
                    | (modes[n] << 1)
                    | (u8::from(prev) << 3)
                    | (u8::from(next) << 4);
                p
            })
            .collect();
        let i = ident_header(channels, 22_050, BS0, BS1);
        let data = stream(&[
            (FLAG_BOS, 0, whole(&[i])),
            (0, 0, whole(&[comment_header(), setup])),
            (0, 0, whole(&packets[..20])),
            (FLAG_EOS, 1 << 20, whole(&packets[20..])),
        ]);
        let info = probe_ogg(&data).unwrap();
        let pcm = decode_ogg(&data, OggRange::Decoded, 1 << 22).unwrap();
        assert_eq!(pcm.frames() as u64, info.total_samples);
        assert!(pcm.samples.iter().all(|s| (-1.0..=1.0).contains(s)));
        let nonzero = pcm.samples.iter().filter(|s| **s != 0.0).count();
        assert!(nonzero > 0, "{channels} channels: silent");
    }
}
