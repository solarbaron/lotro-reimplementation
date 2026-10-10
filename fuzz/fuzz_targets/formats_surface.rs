// SPDX-License-Identifier: GPL-3.0-or-later
//! Render surfaces end to end: `Surface::parse` + `to_rgba8` on arbitrary records, on
//! arbitrary bytes wrapped as an embedded JPEG (code 500), and the DXT block decoder. A
//! surface that decodes must decode to `Surface::decoded_size`, the size read before
//! decoding.
//!
//! Run with a memory cap so that decompression bombs count as failures, e.g.
//! `cargo fuzz run formats_surface -- -malloc_limit_mb=512 -rss_limit_mb=1024`.
//! The JPEG path may legitimately allocate up to 128 MiB of RGBA output plus decoder state.

#![no_main]

use libfuzzer_sys::fuzz_target;
use tessera_formats::{BcFormat, Record, Surface, decode_bcn};

fn render(record: &[u8]) {
    let Ok(s) = Surface::parse(record) else {
        return;
    };
    // The size read without decoding (callers check and charge it before decoding).
    let declared = s.decoded_size();
    if let Ok(img) = s.to_rgba8() {
        assert_eq!(
            img.pixels.len() as u64,
            u64::from(img.width) * u64::from(img.height) * 4
        );
        assert_eq!(declared, Some((img.width, img.height)));
        if !s.is_jpeg() {
            assert_eq!((img.width, img.height), (s.width, s.height));
        }
    }
}

fuzz_target!(|data: &[u8]| {
    let Some((&mode, body)) = data.split_first() else {
        return;
    };
    match mode % 3 {
        // The input is a whole surface record.
        0 => render(body),
        // The input is the payload of a JPEG surface.
        1 => {
            let mut record = Vec::with_capacity(body.len() + 24);
            for v in [0x4100_0001u32, 0, 0, 0, 500] {
                record.extend_from_slice(&v.to_le_bytes());
            }
            record.extend_from_slice(&(body.len() as u32).to_le_bytes());
            record.extend_from_slice(body);
            render(&record);
        }
        // Two bytes of dimensions (up to 255x255), then DXT block data.
        _ => {
            let [w, h, rest @ ..] = body else {
                return;
            };
            for f in [BcFormat::Bc1, BcFormat::Bc2, BcFormat::Bc3] {
                if let Some(px) = decode_bcn(f, u32::from(*w), u32::from(*h), rest) {
                    assert_eq!(px.len(), usize::from(*w) * usize::from(*h) * 4);
                }
            }
        }
    }
});
