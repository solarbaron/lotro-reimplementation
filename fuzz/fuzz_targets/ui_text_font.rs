// SPDX-License-Identifier: GPL-3.0-or-later
//! `tessera-ui::text` fallback fonts (`fonts.md` §11 items 3–5): a hostile TrueType or
//! OpenType file goes through `TesseraFont::from_bytes` and `TesseraFont::rasterise` at any
//! supported pixel size and code range, and a face that builds lays out and draws a short
//! text. Nothing may panic, abort on allocation or loop: vertical metrics, advances and glyph
//! bounds of any size are refused or left out before the rasteriser allocates.
//!
//! Input layout: byte 0 selects the mode (even: the bundled font with patches, odd: the rest
//! of the input is the font file), byte 1 the pixel size, bytes 2–9 two code ranges (start
//! and a span of at most 255). In patch mode the rest is the patch list: the hhea ascent and
//! descent, the OS/2 typographic ascent and descent (i16 each, applied when the table exists),
//! then (offset u32, byte) pairs written anywhere in the file.

#![no_main]

use std::sync::Arc;

use libfuzzer_sys::fuzz_target;
use tessera_ui::text::{
    CharListBuilder, DrawParams, LayoutParams, MAX_FALLBACK_PX, MIN_FALLBACK_PX, TesseraFont,
    bundled_font_bytes, draw, layout,
};

fn be16(b: &[u8], o: usize) -> Option<usize> {
    Some(usize::from(u16::from_be_bytes([
        *b.get(o)?,
        *b.get(o + 1)?,
    ])))
}

fn be32(b: &[u8], o: usize) -> Option<usize> {
    let v = u32::from_be_bytes([*b.get(o)?, *b.get(o + 1)?, *b.get(o + 2)?, *b.get(o + 3)?]);
    usize::try_from(v).ok()
}

/// The table offset of `tag`, if the directory names one.
fn table(font: &[u8], tag: &[u8; 4]) -> Option<usize> {
    let n = be16(font, 4)?;
    (0..n).find_map(|t| {
        let rec = 12 + 16 * t;
        (font.get(rec..rec + 4)? == tag).then(|| be32(font, rec + 8))?
    })
}

fn put(font: &mut [u8], at: usize, v: [u8; 2]) {
    if let Some(d) = font.get_mut(at..at + 2) {
        d.copy_from_slice(&v);
    }
}

fn patched(rest: &[u8]) -> Vec<u8> {
    let mut font = bundled_font_bytes().to_vec();
    let v = |i: usize| {
        [
            rest.get(i).copied().unwrap_or(0),
            rest.get(i + 1).copied().unwrap_or(0),
        ]
    };
    if let Some(h) = table(&font, b"hhea") {
        put(&mut font, h + 4, v(0));
        put(&mut font, h + 6, v(2));
    }
    if let Some(o) = table(&font, b"OS/2") {
        put(&mut font, o + 68, v(4));
        put(&mut font, o + 70, v(6));
    }
    let len = font.len();
    for p in rest
        .get(8..)
        .unwrap_or(&[])
        .as_chunks::<5>()
        .0
        .iter()
        .take(64)
    {
        let at = u32::from_le_bytes([p[0], p[1], p[2], p[3]]) as usize % len;
        font[at] = p[4];
    }
    font
}

fuzz_target!(|data: &[u8]| {
    if data.len() < 10 {
        return;
    }
    let px = MIN_FALLBACK_PX + u32::from(data[1]) % (MAX_FALLBACK_PX - MIN_FALLBACK_PX + 1);
    let range = |i: usize| {
        let lo = u16::from_le_bytes([data[i], data[i + 1]]);
        let span = u16::from(data[i + 2]);
        (lo, lo.saturating_add(span))
    };
    let ranges = [range(2), range(6)];
    let bytes = if data[0].is_multiple_of(2) {
        patched(&data[10..])
    } else {
        data[10..].to_vec()
    };
    let Ok(font) = TesseraFont::from_bytes(bytes) else {
        return;
    };
    let Ok(face) = font.rasterise(px, &ranges) else {
        return;
    };
    let face = Arc::new(face);
    let fonts = [Some(face)];
    let mut b = CharListBuilder::new();
    b.underline(Some(0xFF00_00FF))
        .push_units(&[ranges[0].0, 0x41, 0x20, ranges[1].1, 0x0A, 0x3F]);
    let cs = b.finish();
    let p = LayoutParams {
        width: 40,
        height: 40,
        ..LayoutParams::default()
    };
    if let Ok(l) = layout(&cs, &fonts, &p, None) {
        let d = draw(
            &l,
            &cs,
            &fonts,
            &DrawParams {
                outline: true,
                ..DrawParams::default()
            },
        );
        let _ = d.area();
    }
});
