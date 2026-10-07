// SPDX-License-Identifier: GPL-3.0-or-later
//! Sound payload decoding (`docs/specs/sound.md` §3, §12.2): `tessera_audio::decode` on
//! arbitrary bytes.
//!
//! Three shapes of input, chosen by the first byte:
//!
//! - raw container bytes (and the same bytes as a whole 0x0A record);
//! - an Ogg stream assembled from the fuzz bytes with correct page CRCs, so the packet and
//!   Vorbis layers are reached: (flags, granule, packet count, packets…) per page;
//! - the same, preceded by a valid synthetic Vorbis identification, comment and setup
//!   header, so audio packets reach the decoder library;
//! - the same with a rich setup header (lookup 1 and 2, sparse and ordered codebooks,
//!   floor 1 with partition classes and subclass books, residues 0 and 2 with cascades,
//!   two submaps, coupling for stereo, four modes) into which the fuzz bytes first flip up
//!   to 15 bits, so that hostile setup headers that both our walk and the decoder library
//!   accept reach audible decode paths (lookup values, floor curves, residue stages).
//!
//! Nothing may panic. A decoded buffer has 1 or 2 channels and whole frames; for Ogg its
//! frame count equals the probed range; a streaming source delivers the same number of
//! frames before and after a rewind.

#![no_main]

use std::sync::Arc;

use libfuzzer_sys::fuzz_target;
use tessera_audio::decode::ogg::{build_page, lacing_for};
use tessera_audio::decode::{
    DecodeOptions, Decoded, OggRange, Playable, Probe, StreamPolicy, decode_payload_with,
    decode_record, open_payload, probe_payload,
};

const MAX_SAMPLES: usize = 1 << 20;
const MAX_STREAM_FRAMES: u64 = 1 << 20;

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
            if (value >> i) & 1 == 1
                && let Some(b) = self.bytes.last_mut()
            {
                *b |= 1 << self.bit;
            }
            self.bit = (self.bit + 1) % 8;
        }
        self
    }
}

/// Identification, comment and setup headers of a minimal mono or stereo stream with
/// block sizes 256 and 2048 (see `crates/audio/tests/decode_synthetic.rs`).
fn headers(channels: u8) -> [Vec<u8>; 3] {
    let mut ident = vec![1];
    ident.extend_from_slice(b"vorbis");
    ident.extend_from_slice(&0u32.to_le_bytes());
    ident.push(channels);
    ident.extend_from_slice(&22_050u32.to_le_bytes());
    ident.extend_from_slice(&[0; 12]);
    ident.push(0xB8);
    ident.push(1);
    let mut comment = vec![3];
    comment.extend_from_slice(b"vorbis");
    comment.extend_from_slice(&[0, 0, 0, 0, 0, 0, 0, 0, 1]);
    let mut w = BitWriter::default();
    w.put(0, 8);
    w.put(0x56_4342, 24)
        .put(1, 16)
        .put(2, 24)
        .put(0, 1)
        .put(0, 1);
    w.put(0, 5).put(0, 5).put(0, 4);
    w.put(0, 6).put(0, 16);
    w.put(0, 6).put(1, 16).put(0, 5).put(0, 2).put(8, 4);
    w.put(0, 6)
        .put(0, 16)
        .put(0, 24)
        .put(0, 24)
        .put(0, 24)
        .put(0, 6)
        .put(0, 8);
    w.put(0, 3).put(0, 1);
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
    let mut setup = vec![5];
    setup.extend_from_slice(b"vorbis");
    setup.extend_from_slice(&w.bytes);
    [ident, comment, setup]
}

fn setup_bytes(w: BitWriter) -> Vec<u8> {
    let mut v = vec![5];
    v.extend_from_slice(b"vorbis");
    v.extend_from_slice(&w.bytes);
    v
}

/// `float32_unpack` word of 1.0.
const F32_ONE: u32 = (768 << 21) | (1 << 20);

/// A codebook of 2^`len` entries, all of length `len`, no lookup.
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

/// The rich setup header of `crates/audio/tests/decode_synthetic.rs` (`rich_setup`; keep
/// the two in step).
fn rich_setup(channels: u8) -> Vec<u8> {
    let mut w = BitWriter::default();
    w.put(4, 8);
    book_scalar(&mut w, 3);
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
    w.put(0, 6).put(0, 16);
    w.put(1, 6);
    w.put(1, 16).put(2, 5).put(0, 4).put(1, 4);
    w.put(1, 3).put(1, 2).put(0, 8).put(0, 8).put(5, 8);
    w.put(2, 3).put(0, 2).put(1, 8);
    w.put(1, 2).put(7, 4);
    for x in [10, 40, 90, 20, 64] {
        w.put(x, 7);
    }
    w.put(1, 16).put(0, 5).put(0, 2).put(8, 4);
    w.put(1, 6);
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
    w.put(0, 16)
        .put(0, 24)
        .put(64, 24)
        .put(7, 24)
        .put(0, 6)
        .put(4, 8);
    w.put(1, 3).put(0, 1).put(2, 8);
    w.put(1, 6);
    w.put(0, 16).put(0, 1);
    if channels == 2 {
        w.put(1, 1).put(0, 8).put(0, 1).put(1, 1);
    } else {
        w.put(0, 1);
    }
    w.put(0, 2).put(0, 8).put(0, 8).put(0, 8);
    w.put(0, 16).put(1, 1).put(1, 4).put(0, 1).put(0, 2);
    for c in 0..channels {
        w.put(u32::from(c % 2), 4);
    }
    w.put(0, 8).put(1, 8).put(1, 8);
    w.put(0, 8).put(0, 8).put(0, 8);
    w.put(3, 6);
    for (long, mapping) in [(0, 0), (1, 0), (0, 1), (1, 1)] {
        w.put(long, 1).put(0, 16).put(0, 16).put(mapping, 8);
    }
    w.put(1, 1);
    setup_bytes(w)
}

/// Headers with the rich setup header, after flipping the bits the fuzz bytes name: the
/// first byte gives the number of flips (0–15), then two bytes per flip give a bit index
/// past the 7-byte common header. Returns the headers and the remaining bytes.
fn rich_headers(channels: u8, data: &[u8]) -> ([Vec<u8>; 3], &[u8]) {
    let [ident, comment, _] = headers(channels);
    let mut setup = rich_setup(channels);
    let Some((&n, mut rest)) = data.split_first() else {
        return ([ident, comment, setup], data);
    };
    let bits = (setup.len() - 7) * 8;
    for _ in 0..(n % 16) {
        let Some((pair, tail)) = rest.split_first_chunk::<2>() else {
            break;
        };
        rest = tail;
        let bit = usize::from(u16::from_le_bytes(*pair)) % bits;
        setup[7 + bit / 8] ^= 1 << (bit % 8);
    }
    ([ident, comment, setup], rest)
}

/// Builds pages from `data`: per page a flags byte, a u16 granule, a packet count, then
/// per packet a length byte and that many bytes. `prefix` packets go first, one page.
fn assemble(data: &[u8], prefix: Option<[Vec<u8>; 3]>) -> Vec<u8> {
    let mut out = Vec::new();
    let mut seq = 0u32;
    if let Some(h) = prefix {
        let mut lacing = Vec::new();
        let mut body = Vec::new();
        for p in &h {
            lacing.extend(lacing_for(p.len()));
            body.extend_from_slice(p);
        }
        if let Some(page) = build_page(0x02, 0, 1, seq, &lacing, &body) {
            out.extend(page);
        }
        seq += 1;
    }
    let mut it = data.iter().copied();
    while let (Some(flags), Some(g0), Some(g1), Some(count)) =
        (it.next(), it.next(), it.next(), it.next())
    {
        let mut lacing = Vec::new();
        let mut body = Vec::new();
        for _ in 0..(count % 8) {
            let Some(len) = it.next() else { break };
            let bytes: Vec<u8> = it.by_ref().take(usize::from(len)).collect();
            lacing.extend(lacing_for(bytes.len()));
            body.extend(bytes);
        }
        if lacing.len() > 255 {
            break;
        }
        let granule = u64::from(u16::from_le_bytes([g0, g1]));
        if let Some(page) = build_page(flags & 0x07, granule, 1, seq, &lacing, &body) {
            out.extend(page);
        }
        seq += 1;
    }
    out
}

fn exercise(audio: &[u8]) {
    let options = DecodeOptions {
        range: OggRange::Original,
        max_buffered_samples: MAX_SAMPLES,
    };
    let probe = probe_payload(audio);
    if let Ok(Decoded::Pcm(pcm)) = decode_payload_with(audio, &options) {
        assert!(pcm.channels == 1 || pcm.channels == 2);
        assert_eq!(pcm.samples.len() % usize::from(pcm.channels), 0);
        assert!(pcm.samples.iter().all(|s| (-1.0..=1.0).contains(s)));
        match &probe {
            Ok(Probe::Ogg(info)) => {
                assert_eq!(pcm.frames() as u64, info.played_len);
                assert!(info.played_start + info.played_len <= info.total_samples);
                assert!(info.vorbis_len <= info.total_samples);
            }
            Ok(Probe::Wave(f)) => assert_eq!(pcm.frames() as u64, f.frames()),
            other => panic!("decoded but probe gave {other:?}"),
        }
    }
    // Streaming path, forced.
    let policy = StreamPolicy {
        stream_above_secs: 0.0,
        options,
    };
    if let Ok(Playable::Streamed(mut src)) = open_payload(Arc::from(audio), &policy) {
        let mut buf = [0.0f32; 512];
        let mut first = 0u64;
        while first < MAX_STREAM_FRAMES {
            let n = src.read(&mut buf);
            if n == 0 {
                break;
            }
            first += n as u64;
        }
        if first < MAX_STREAM_FRAMES && src.rewind() {
            let mut second = 0u64;
            loop {
                let n = src.read(&mut buf);
                if n == 0 {
                    break;
                }
                second += n as u64;
            }
            assert_eq!(first, second);
            if let Some(len) = src.len_frames() {
                assert!(first <= len);
            }
        }
    }
}

fuzz_target!(|data: &[u8]| {
    let Some((&mode, rest)) = data.split_first() else {
        return;
    };
    match mode % 6 {
        0 => {
            exercise(rest);
            let _ = decode_record(rest);
        }
        1 => exercise(&assemble(rest, None)),
        2 => exercise(&assemble(rest, Some(headers(1)))),
        3 => exercise(&assemble(rest, Some(headers(2)))),
        m => {
            let (h, rest) = rich_headers(if m == 4 { 1 } else { 2 }, rest);
            exercise(&assemble(rest, Some(h)));
        }
    }
});
