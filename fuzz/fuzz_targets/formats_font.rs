// SPDX-License-Identifier: GPL-3.0-or-later
//! Font record (0x42) decoding (`docs/specs/fonts.md` §3, §12) on arbitrary bytes, raw and
//! behind a valid 0x42 DID prologue.
//!
//! Nothing may panic and nothing is allocated before the length rule (36 + 11·n bytes) and
//! the glyph-count cap hold. An accepted record re-encodes to the same bytes, every stored
//! code is found by lookup, and the invariant and atlas checks run.
//!
//! Run with a memory cap so that allocation blow-ups count as failures, e.g.
//! `cargo fuzz run formats_font -- -malloc_limit_mb=512 -rss_limit_mb=1024`.

#![no_main]

use libfuzzer_sys::fuzz_target;
use tessera_formats::font::{GLYPH_LEN, HEADER_LEN, MAX_GLYPHS, TRAILER_LEN, encode_font};
use tessera_formats::{AnyRecord, Font, decode_font};

fn check(data: &[u8], f: &Font) {
    assert!(f.len() <= MAX_GLYPHS as usize);
    assert_eq!(data.len(), HEADER_LEN + TRAILER_LEN + GLYPH_LEN * f.len());
    assert_eq!(encode_font(f), data);
    for g in f.glyphs() {
        let i = f.find(g.code).expect("stored code is found");
        assert_eq!(f.glyphs()[i].code, g.code);
    }
    let _ = f.issues();
    let w = u32::from(data.first().copied().unwrap_or(0)) * 8;
    let _ = f.atlas_issues(w, w, 32);
}

fuzz_target!(|data: &[u8]| {
    if let Ok(f) = decode_font(data) {
        check(data, &f);
    }
    let mut prefixed = 0x4200_0001u32.to_le_bytes().to_vec();
    prefixed.extend_from_slice(data);
    if let Ok(f) = decode_font(&prefixed) {
        check(&prefixed, &f);
    }
    if let Ok(Some(r)) = AnyRecord::decode(0x4200_0001u32, &prefixed) {
        assert_eq!(r.did().0, 0x4200_0001);
    }
});
