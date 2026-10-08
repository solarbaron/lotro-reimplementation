// SPDX-License-Identifier: GPL-3.0-or-later
//! Synthetic tests for font records (prefix 0x42), `docs/specs/fonts.md` §3, §4.2, §12,
//! §13.1: the 80-byte worked example, the length rule, the §3.4 limits, the invariant
//! checks FN1–FN7 and FN9, the §12 bound on the FN9 overlap sweep, and round trips. All data
//! is invented.

use proptest::prelude::*;
use tessera_formats::font::{
    AtlasIssue, FontIssue, GLYPH_LEN, HEADER_LEN, MAX_GLYPHS, QUESTION_MARK, TRAILER_LEN,
    encode_font,
};
use tessera_formats::{
    AnyRecord, Did, DidKind, Evidence, Font, FormatError, Glyph, Record, decode_font,
};

/// The 80 bytes of §13.1, transcribed from the spec's hex dump.
const EXAMPLE_13_1: &str = "
01 00 ab 42 0a 00 00 00 06 00 00 00 04 00 00 00
03 0a 00 01 00 20 00 01 00 01 00 04 07 00 01 01
3f 00 06 00 01 00 06 07 00 01 01 41 00 0c 00 01
00 03 08 fe 01 02 6a 00 14 00 01 00 01 00 00 00
01 00 00 00 08 00 00 00 10 00 ab 41 11 00 ab 41";

fn hex(s: &str) -> Vec<u8> {
    s.split_whitespace()
        .map(|b| u8::from_str_radix(b, 16).unwrap())
        .collect()
}

#[allow(clippy::too_many_arguments)]
fn g(width: u8, height: u8, a: i8, b: i8, t: i8, code: u16, x: u16, y: u16) -> Glyph {
    Glyph {
        width,
        height,
        left_bearing: a,
        right_bearing: b,
        top: t,
        code,
        x,
        y,
    }
}

fn example_font() -> Font {
    Font::new(
        Did(0x42AB_0001),
        10,
        6,
        vec![
            g(3, 10, 0, 1, 0, 0x20, 1, 1),
            g(4, 7, 0, 1, 1, 0x3F, 6, 1),
            g(6, 7, 0, 1, 1, 0x41, 12, 1),
            g(3, 8, -2, 1, 2, 0x6A, 20, 1),
        ],
        1,
        1,
        8,
        Did(0x41AB_0010),
        Did(0x41AB_0011),
    )
}

#[test]
fn worked_example_13_1() {
    let data = hex(EXAMPLE_13_1);
    assert_eq!(data.len(), 80);
    assert_eq!(data.len(), HEADER_LEN + TRAILER_LEN + GLYPH_LEN * 4); // FN1
    let f = Font::parse(&data).unwrap();
    assert_eq!(f, example_font());
    assert_eq!(f.did(), Did(0x42AB_0001)); // FN2
    assert_eq!((f.line_height, f.widest_glyph, f.len()), (10, 6, 4));
    assert_eq!((f.border_x, f.border_y, f.baseline), (1, 1, 8));
    assert_eq!(
        (f.foreground, f.background),
        (Did(0x41AB_0010), Did(0x41AB_0011))
    );
    assert!(f.has_background());
    assert!(f.codes_ascending());
    let j = f.glyph(0x6A).unwrap();
    assert_eq!(
        (j.width, j.height, j.left_bearing, j.right_bearing, j.top),
        (3, 8, -2, 1, 2)
    );
    assert_eq!(j.advance(), 2);
    // `A` and `?` end on the baseline; `j` descends 2 below it.
    for c in [0x41u16, QUESTION_MARK] {
        let gl = f.glyph(c).unwrap();
        assert_eq!(gl.top as u32 + u32::from(gl.height), f.baseline);
    }
    assert_eq!(j.top as u32 + u32::from(j.height), f.baseline + 2);
    assert_eq!(f.find(0x42), None);
    assert_eq!(f.issues(), vec![]); // FN3–FN7
    // FN9 against the 32 × 12 atlases; the last bordered box ends at column 24.
    assert_eq!(f.atlas_issues(32, 12, usize::MAX), vec![]);
    assert_eq!(j.bordered_box(1, 1), (19, 0, 5, 10));
    assert!(!f.atlas_issues(23, 12, usize::MAX).is_empty());
    assert!(!f.atlas_issues(32, 11, usize::MAX).is_empty());
    // FN10: the packing rule.
    let mut x = f.border_x;
    for gl in f.glyphs() {
        assert_eq!(u32::from(gl.x), x);
        assert_eq!(u32::from(gl.y), f.border_y);
        x += u32::from(gl.width) + 2 * f.border_x;
    }
    let max_h = f
        .glyphs()
        .iter()
        .map(|g| u32::from(g.height))
        .max()
        .unwrap();
    assert_eq!(f.border_y + max_h + f.border_y, 12);

    assert_eq!(encode_font(&f), data);
    assert_eq!(decode_font(&data).unwrap(), f);
    let any = AnyRecord::decode(0x42AB_0001u32, &data).unwrap().unwrap();
    assert!(matches!(any, AnyRecord::Font(_)));
    assert_eq!(any.type_name(), "font");
    assert!(AnyRecord::is_decodable(0x42AB_0001u32));
    assert_eq!(DidKind::Font.evidence(), Evidence::Decoded);
    assert_eq!(DidKind::UiLayout.evidence(), Evidence::Decoded);
    assert!(matches!(
        AnyRecord::decode(0x42AB_0002u32, &data),
        Err(FormatError::DidMismatch { .. })
    ));
}

#[test]
fn length_must_match_the_count_exactly() {
    let data = hex(EXAMPLE_13_1);
    for n in 0..data.len() {
        assert!(Font::parse(&data[..n]).is_err(), "prefix {n}");
    }
    let mut longer = data.clone();
    longer.extend_from_slice(&[0; GLYPH_LEN]);
    assert!(matches!(
        Font::parse(&longer),
        Err(FormatError::Malformed { .. })
    ));
    longer.truncate(81);
    assert!(Font::parse(&longer).is_err());
    // Count patched to 5 with 4 entries present.
    let mut bad = data.clone();
    bad[12] = 5;
    assert!(matches!(
        Font::parse(&bad),
        Err(FormatError::Malformed { .. })
    ));
}

#[test]
fn hostile_counts_are_capped_before_allocation() {
    let mut data = hex(EXAMPLE_13_1);
    data[12..16].copy_from_slice(&(MAX_GLYPHS + 1).to_le_bytes());
    assert!(matches!(
        Font::parse(&data),
        Err(FormatError::CountLimit { .. })
    ));
    data[12..16].copy_from_slice(&u32::MAX.to_le_bytes());
    assert!(matches!(
        Font::parse(&data),
        Err(FormatError::CountLimit { .. })
    ));
    // The largest allowed count with too few bytes fails on length, not allocation.
    data[12..16].copy_from_slice(&MAX_GLYPHS.to_le_bytes());
    assert!(matches!(
        Font::parse(&data),
        Err(FormatError::Malformed { .. })
    ));
}

#[test]
fn wrong_prefix_is_rejected() {
    let mut data = hex(EXAMPLE_13_1);
    data[3] = 0x41;
    assert!(matches!(
        Font::parse(&data),
        Err(FormatError::WrongRecordType { .. })
    ));
}

#[test]
fn an_empty_font_decodes_and_is_reported() {
    let f = Font::new(
        Did(0x4200_0001),
        0,
        0,
        vec![],
        0,
        0,
        0,
        Did(0x4100_0001),
        Did(0),
    );
    let data = encode_font(&f);
    assert_eq!(data.len(), 36);
    let back = Font::parse(&data).unwrap();
    assert!(back.is_empty());
    let issues = back.issues();
    assert!(issues.contains(&FontIssue::Empty));
    assert!(issues.contains(&FontIssue::BaselineNotInLine));
    assert_eq!(back.find(QUESTION_MARK), None);
}

#[test]
fn issues_name_each_broken_rule() {
    // A font with no background but borders, unequal borders, unsorted codes, a glyph that
    // leaves the line box, a zero-height glyph, a negative advance and wrong maxima.
    let f = Font::new(
        Did(0x4200_0002),
        8,
        2,
        vec![
            g(3, 9, 0, 1, 0, 0x41, 2, 2),
            g(1, 0, -5, 0, -1, 0x20, 8, 2),
            g(1, 2, 0, 0, 0, 0x20, 12, 2),
            g(255, 2, 127, 127, 0, 0x42, 16, 2),
        ],
        2,
        3,
        8,
        Did(0x4100_0001),
        Did(0),
    );
    let (back, issues) = Font::parse_with_issues(&encode_font(&f)).unwrap();
    assert!(!back.codes_ascending());
    // With repeated codes the first match wins.
    assert_eq!(back.find(0x20), Some(1));
    for want in [
        FontIssue::LineHeightNotMaxHeight,
        FontIssue::WidestNotMaxWidth,
        FontIssue::CodesNotAscending(1),
        FontIssue::FirstCodeNotSpace,
        FontIssue::NoQuestionMark,
        FontIssue::ZeroHeight(1),
        FontIssue::GlyphOutsideLine(0),
        FontIssue::GlyphOutsideLine(1),
        FontIssue::NegativeAdvance(1),
        FontIssue::AdvanceAbove255(3),
        FontIssue::BaselineNotInLine,
        FontIssue::BordersDiffer,
        FontIssue::NoBackgroundWithBorder,
    ] {
        assert!(issues.contains(&want), "{want:?} missing from {issues:?}");
        assert!(!want.name().is_empty());
    }
}

#[test]
fn atlas_overlaps_are_found() {
    let f = Font::new(
        Did(0x4200_0003),
        10,
        4,
        vec![
            g(4, 10, 0, 1, 0, 0x20, 1, 1),
            g(4, 10, 0, 1, 0, 0x3F, 4, 1),
            g(4, 10, 0, 1, 0, 0x41, 40, 1),
        ],
        1,
        1,
        8,
        Did(0x4100_0001),
        Did(0x4100_0002),
    );
    let issues = f.atlas_issues(32, 12, usize::MAX);
    assert!(issues.contains(&AtlasIssue::Overlap {
        first: 0,
        second: 1
    }));
    assert!(issues.contains(&AtlasIssue::OutsideAtlas(2)));
    assert_eq!(f.atlas_issues(32, 12, 1).len(), 1);
}

#[test]
fn advances_above_255_are_reported() {
    // FN5 upper bound: W = 255, A = 100, B = 100 gives 455; 254 + 1 and 255 + 0 are 255.
    let f = Font::new(
        Did(0x4200_0004),
        10,
        255,
        vec![
            g(254, 10, 0, 1, 0, 0x20, 0, 0),
            g(255, 10, 100, 100, 0, 0x3F, 0, 20),
            g(255, 10, 0, 0, 0, 0x41, 0, 40),
        ],
        0,
        0,
        8,
        Did(0x4100_0001),
        Did(0),
    );
    assert_eq!(f.glyphs()[1].advance(), 455);
    assert_eq!(f.issues(), vec![FontIssue::AdvanceAbove255(1)]);
    assert_eq!(FontIssue::AdvanceAbove255(1).name(), "advance above 255");
}

/// Every overlapping pair, by comparing all pairs: the reference for the sweep.
fn brute_force_overlaps(f: &Font) -> Vec<AtlasIssue> {
    let boxes: Vec<_> = f
        .glyphs()
        .iter()
        .map(|g| g.bordered_box(f.border_x, f.border_y))
        .collect();
    let mut out = Vec::new();
    for (i, a) in boxes.iter().enumerate() {
        for (j, b) in boxes.iter().enumerate().skip(i + 1) {
            let area = a.2 > 0 && a.3 > 0 && b.2 > 0 && b.3 > 0;
            if area && a.0 < b.0 + b.2 && b.0 < a.0 + a.2 && a.1 < b.1 + b.3 && b.1 < a.1 + a.3 {
                out.push(AtlasIssue::Overlap {
                    first: i,
                    second: j,
                });
            }
        }
    }
    out.sort_by_key(|i| format!("{i:?}"));
    out
}

#[test]
fn atlas_sweep_finds_every_overlap() {
    // Deterministic pseudo-random boxes in a small atlas, so that many overlap.
    let mut state = 0x2545_f491_4f6c_dd1du64;
    let mut next = move |m: u64| {
        state ^= state << 13;
        state ^= state >> 7;
        state ^= state << 17;
        state % m
    };
    for round in 0..200 {
        let n = 1 + next(40) as usize;
        let border = (round % 3) as u32;
        let glyphs: Vec<Glyph> = (0..n)
            .map(|i| {
                g(
                    next(12) as u8,
                    next(12) as u8,
                    0,
                    0,
                    0,
                    i as u16,
                    next(64) as u16,
                    next(64) as u16,
                )
            })
            .collect();
        let f = Font::new(
            Did(0x4200_0005),
            12,
            12,
            glyphs,
            border,
            border,
            8,
            Did(0x4100_0001),
            Did(0x4100_0002),
        );
        let mut found: Vec<AtlasIssue> = f
            .atlas_issues(1 << 20, 1 << 20, usize::MAX)
            .into_iter()
            .filter(|i| matches!(i, AtlasIssue::Overlap { .. }))
            .collect();
        found.sort_by_key(|i| format!("{i:?}"));
        assert_eq!(found, brute_force_overlaps(&f), "round {round}");
    }
}

#[test]
fn atlas_sweep_is_not_quadratic_on_a_full_row() {
    // §3.4 hostile input: the largest record (65,536 glyphs, about 720 KB) with every box in
    // one row band and none overlapping. A pairwise sweep makes about 2·10⁹ comparisons here
    // (a minute in a debug build); the active-set sweep does one lookup per box.
    let n = MAX_GLYPHS as usize;
    for (w, h) in [(1u8, 1u8), (1, 255)] {
        let glyphs: Vec<Glyph> = (0..n)
            .map(|i| g(w, h, 0, 0, 0, i as u16, i as u16, 0))
            .collect();
        let f = Font::new(
            Did(0x4200_0006),
            u32::from(h),
            u32::from(w),
            glyphs,
            0,
            0,
            0,
            Did(0x4100_0001),
            Did(0),
        );
        let t = std::time::Instant::now();
        assert!(f.atlas_issues(65_536, 256, 8).is_empty());
        let spent = t.elapsed();
        assert!(spent.as_secs() < 10, "{w}x{h}: {spent:?}");
    }
    // Interleaved widths 255 and 1, 256 pairs per row of a 65,536-wide atlas, 128 rows.
    let glyphs: Vec<Glyph> = (0..n)
        .map(|i| {
            let pair = i / 2;
            let x = (pair % 256) * 256 + if i % 2 == 0 { 0 } else { 255 };
            let w = if i % 2 == 0 { 255 } else { 1 };
            g(
                w,
                255,
                0,
                0,
                0,
                i as u16,
                x as u16,
                (pair / 256 * 255) as u16,
            )
        })
        .collect();
    let f = Font::new(
        Did(0x4200_0007),
        255,
        255,
        glyphs,
        0,
        0,
        0,
        Did(0x4100_0001),
        Did(0),
    );
    let t = std::time::Instant::now();
    assert!(f.atlas_issues(65_536, 65_535, 8).is_empty());
    assert!(t.elapsed().as_secs() < 10, "{:?}", t.elapsed());
}

fn arb_glyph() -> impl Strategy<Value = Glyph> {
    (
        any::<u8>(),
        any::<u8>(),
        any::<i8>(),
        any::<i8>(),
        any::<i8>(),
        any::<u16>(),
        any::<u16>(),
        any::<u16>(),
    )
        .prop_map(|(w, h, a, b, t, code, x, y)| g(w, h, a, b, t, code, x, y))
}

proptest! {
    #[test]
    fn round_trip(
        glyphs in proptest::collection::vec(arb_glyph(), 0..64),
        head in any::<[u32; 2]>(),
        tail in any::<[u32; 5]>(),
    ) {
        let f = Font::new(
            Did(0x4200_0000 | (tail[3] & 0xFF_FFFF)),
            head[0], head[1], glyphs, tail[0], tail[1], tail[2], Did(tail[3]), Did(tail[4]),
        );
        let data = encode_font(&f);
        let back = Font::parse(&data).unwrap();
        prop_assert_eq!(&back, &f);
        let _ = back.issues();
        let _ = back.atlas_issues(1024, 1024, 64);
        for gl in back.glyphs() {
            prop_assert!(back.glyph(gl.code).is_some());
        }
    }

    #[test]
    fn arbitrary_bytes_never_panic(data in proptest::collection::vec(any::<u8>(), 0..400)) {
        let _ = Font::parse(&data);
        let mut prefixed = 0x4200_0001u32.to_le_bytes().to_vec();
        prefixed.extend_from_slice(&data);
        if let Ok(f) = Font::parse(&prefixed) {
            prop_assert_eq!(prefixed.len(), 36 + 11 * f.len());
            prop_assert_eq!(encode_font(&f), prefixed);
        }
    }
}
