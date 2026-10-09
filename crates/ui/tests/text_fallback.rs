// SPDX-License-Identifier: GPL-3.0-or-later
//! Tessera's own font (`fonts.md` §11 items 2, 4 and 5): the bundled OFL font is rasterised
//! on the CPU at 1 : 1 into the same face form as the game fonts, keeps the record rules
//! the layout relies on (§3.3, FN4–FN6, FN9 with borders 0), substitutes `?` from itself
//! (§5) and goes through the same layout, drawing and compositing path.

use std::sync::Arc;

use tessera_ui::paint::{CanvasSet, DisplayList, Rect, paint_rect};
use tessera_ui::text::{
    CharListBuilder, DEFAULT_RANGES, DrawParams, FontId, LayoutParams, MAX_FALLBACK_PX,
    MIN_FALLBACK_PX, TesseraFont, TextError, TextOp, bundled_font_bytes, draw, layout,
};

#[test]
fn bundled_font_is_the_documented_ofl_file() {
    let bytes = bundled_font_bytes();
    assert!(bytes.len() > 100_000);
    // TrueType signature.
    assert_eq!(&bytes[..4], &[0, 1, 0, 0]);
    // The font names its own licence (OFL) in its name table (UTF-16BE).
    let ofl: Vec<u8> = "Open Font License"
        .encode_utf16()
        .flat_map(u16::to_be_bytes)
        .collect();
    assert!(bytes.windows(ofl.len()).any(|w| w == ofl.as_slice()));
    let licence = include_str!("../assets/fonts/OFL.txt");
    assert!(licence.contains("SIL OPEN FONT LICENSE Version 1.1"));
}

#[test]
fn rasterised_faces_keep_the_record_rules() {
    let font = TesseraFont::bundled().expect("bundled font");
    for px in [MIN_FALLBACK_PX, 10, 12, 14, 16, 20, 24, 32, MAX_FALLBACK_PX] {
        let face = font.rasterise(px, DEFAULT_RANGES).expect("rasterise");
        assert!(
            matches!(face.id(), FontId::Tessera { font: k, px: p, .. } if k == font.key() && p == px)
        );
        assert!(face.background().is_none(), "stamped outline (§8.3)");
        assert_eq!(face.borders(), [0, 0]);
        let lh = face.line_height();
        assert!(face.baseline() < lh, "FN6 at {px}");
        assert!(lh >= px, "line box holds the em at {px}");
        let gs = face.glyphs();
        // FN4: ascending codes from the space, `?` present.
        assert_eq!(gs[0].code, 0x20);
        assert!(gs.windows(2).all(|w| w[0].code < w[1].code));
        assert!(!face.lookup(0x3F).substituted);
        assert!(
            gs.len() > 300,
            "Latin, Greek and Cyrillic at {px}: {}",
            gs.len()
        );
        let atlas = face.foreground();
        let (aw, ah) = (atlas.width(), atlas.height());
        let mut boxes = Vec::new();
        for g in gs {
            // FN5.
            assert!(g.top >= 0);
            assert!(
                g.top as u32 + u32::from(g.height) <= lh,
                "FN5 at {px}: {g:?}"
            );
            assert!(
                i32::from(g.left_bearing) + i32::from(g.width) + i32::from(g.right_bearing) >= 0
            );
            // FN9 with borders 0: inside the atlas.
            assert!(u32::from(g.x) + u32::from(g.width) <= aw);
            assert!(u32::from(g.y) + u32::from(g.height) <= ah);
            if g.width > 0 && g.height > 0 {
                boxes.push((g.x, g.y, g.width, g.height));
            }
        }
        // FN9: no two boxes overlap (rows of the §4.2 packing).
        boxes.sort_unstable();
        for (i, a) in boxes.iter().enumerate() {
            for b in &boxes[i + 1..] {
                let ox = a.0 < b.0 + u16::from(b.2) && b.0 < a.0 + u16::from(a.2);
                let oy = a.1 < b.1 + u16::from(b.3) && b.1 < a.1 + u16::from(a.3);
                assert!(!(ox && oy), "overlap at {px}: {a:?} {b:?}");
            }
        }
        // Ink only inside the boxes.
        let mut inside = vec![false; (aw * ah) as usize];
        for &(x, y, w, h) in &boxes {
            for yy in y..y + u16::from(h) {
                for xx in x..x + u16::from(w) {
                    inside[usize::from(yy) * aw as usize + usize::from(xx)] = true;
                }
            }
        }
        assert!(atlas.data().iter().zip(&inside).all(|(&c, &i)| c == 0 || i));
        if px >= 12 {
            assert!(atlas.data().contains(&255), "full coverage at {px}");
        }
    }
}

#[test]
fn missing_characters_show_as_question_mark_from_the_same_font() {
    let font = TesseraFont::bundled().unwrap();
    let face = font.rasterise(14, DEFAULT_RANGES).unwrap();
    let q = face.lookup(0x3F).glyph.unwrap();
    for unit in [0x4E00u16, 0xAC00, 0x0009, 0x000A, 0xD83D] {
        let l = face.lookup(unit);
        assert!(l.substituted);
        assert_eq!(l.glyph, Some(q));
    }
    assert!(!face.lookup(u16::from(b'A')).substituted);
    assert!(!face.lookup(0x0416).substituted, "Cyrillic is covered");
    // Only the asked ranges are rasterised.
    let small = font.rasterise(14, &[(0x41, 0x43)]).unwrap();
    let codes: Vec<u16> = small.glyphs().iter().map(|g| g.code).collect();
    assert_eq!(codes, [0x20, 0x3F, 0x41, 0x42, 0x43]);
}

#[test]
fn same_layout_and_drawing_path_as_game_fonts() {
    let font = TesseraFont::bundled().unwrap();
    let face = Arc::new(font.rasterise(16, DEFAULT_RANGES).unwrap());
    let fonts = [Some(Arc::clone(&face))];
    let mut b = CharListBuilder::new();
    b.colour(0xFFFF_FFFF)
        .push_str("Tessera settings: install not found")
        .underline(Some(0xFFFF_FFFF))
        .push_str(" retry");
    let cs = b.finish();
    let p = LayoutParams {
        width: 120,
        height: 200,
        ..LayoutParams::default()
    };
    let l = layout(&cs, &fonts, &p, None).unwrap();
    assert!(l.lines.len() >= 2, "wraps at 120 pixels");
    assert!(l.lines.iter().all(|x| x.width <= 120));
    let d = draw(
        &l,
        &cs,
        &fonts,
        &DrawParams {
            outline: true,
            ..DrawParams::default()
        },
    );
    assert!(d.ops.iter().any(|o| matches!(o, TextOp::Underline(_))));
    let mut list = DisplayList::new();
    let area = d.area();
    d.push_to(&mut list, None).unwrap();
    let mut set = CanvasSet::new(128, 256).unwrap();
    paint_rect(&list, &mut set, Rect::new(0, 0, 128, 256));
    let canvas = set.below();
    let mut white = 0;
    let mut black = 0;
    for y in 0..256 {
        for x in 0..128 {
            let px = canvas.pixel(x, y);
            if px == 0xFFFF_FFFF {
                white += 1;
            }
            if px == 0xFF00_0000 {
                black += 1;
            }
            if px != 0 {
                assert!(area.contains(x, y));
            }
        }
    }
    assert!(white > 50 && black > 50, "fill {white}, outline {black}");
}

#[test]
fn bad_inputs_are_errors() {
    let font = TesseraFont::bundled().unwrap();
    assert!(matches!(
        font.rasterise(MIN_FALLBACK_PX - 1, DEFAULT_RANGES),
        Err(TextError::PixelSize(_))
    ));
    assert!(matches!(
        font.rasterise(MAX_FALLBACK_PX + 1, DEFAULT_RANGES),
        Err(TextError::PixelSize(_))
    ));
    assert!(matches!(
        TesseraFont::from_bytes(vec![0; 64]),
        Err(TextError::FontFile(_))
    ));
    assert!(TesseraFont::from_bytes(bundled_font_bytes().to_vec()).is_ok());
    // The whole BMP at the largest size stays within the caps.
    let all = font.rasterise(MAX_FALLBACK_PX, &[(0, 0xFFFF)]);
    match all {
        Ok(f) => assert!(f.foreground().width() <= 8192),
        Err(e) => assert!(matches!(e, TextError::AtlasFull(_)), "{e}"),
    }
}

/// The bundled font with its vertical metrics replaced (hhea ascender and descender, OS/2
/// typographic and Windows ascent and descent): a hostile user font made by our own code.
fn with_vertical_metrics(ascent: i16, descent: i16) -> Vec<u8> {
    let mut f = bundled_font_bytes().to_vec();
    let be16 = |b: &[u8], o: usize| usize::from(u16::from_be_bytes([b[o], b[o + 1]]));
    let be32 =
        |b: &[u8], o: usize| u32::from_be_bytes([b[o], b[o + 1], b[o + 2], b[o + 3]]) as usize;
    let tables = be16(&f, 4);
    for t in 0..tables {
        let rec = 12 + 16 * t;
        let tag = [f[rec], f[rec + 1], f[rec + 2], f[rec + 3]];
        let off = be32(&f, rec + 8);
        let mut put = |o: usize, v: i16| f[off + o..off + o + 2].copy_from_slice(&v.to_be_bytes());
        match &tag {
            b"hhea" => {
                put(4, ascent);
                put(6, descent);
            }
            b"OS/2" => {
                put(68, ascent);
                put(70, descent);
                put(74, ascent);
                put(76, -descent);
            }
            _ => {}
        }
    }
    f
}

/// Review finding: vertical metrics of a few font units, or none, gave a huge or infinite
/// scale; the rasteriser then allocated gigabytes for one glyph or did not finish. They are
/// refused now, and glyph boxes are checked before drawing.
#[test]
fn hostile_vertical_metrics_are_refused_or_bounded() {
    let started = std::time::Instant::now();
    for (asc, desc) in [(1, 0), (0, 0), (-100, 100), (15, 0), (i16::MIN, i16::MAX)] {
        let font = TesseraFont::from_bytes(with_vertical_metrics(asc, desc)).expect("parses");
        for px in [16, MAX_FALLBACK_PX] {
            let r = font.rasterise(px, &[(0x41, 0x41)]);
            assert!(
                matches!(r, Err(TextError::FontFile(_))),
                "{asc}/{desc} at {px}: {r:?}"
            );
        }
    }
    // 16 units: accepted, but every glyph box is about 100 times too large, so the glyphs
    // are left out before the rasteriser allocates for them.
    let font = TesseraFont::from_bytes(with_vertical_metrics(16, 0)).expect("parses");
    let face = font
        .rasterise(MAX_FALLBACK_PX, &[(0x41, 0x5A)])
        .expect("bounded");
    assert!(
        face.lookup(0x41)
            .glyph
            .is_none_or(|g| g.width == 0 || g.code != 0x41)
    );
    assert!(u64::from(face.foreground().width()) * u64::from(face.foreground().height()) < 1 << 20);
    assert!(
        started.elapsed().as_secs() < 30,
        "took {:?}",
        started.elapsed()
    );
}

/// Review finding: metrics that do not fit a glyph entry were clamped silently. A font
/// whose ascent + descent is small against its em gives an em space 213 pixels wide at
/// 64 px; that glyph is left out (shown as `?`) instead of being stored with B = 127.
#[test]
fn glyphs_whose_metrics_do_not_fit_are_left_out() {
    let normal = TesseraFont::bundled()
        .unwrap()
        .rasterise(14, &[(0x2003, 0x2003)])
        .unwrap();
    let em = normal.lookup(0x2003);
    assert!(!em.substituted, "the bundled font has U+2003");
    let font = TesseraFont::from_bytes(with_vertical_metrics(200, -100)).expect("parses");
    let face = font
        .rasterise(MAX_FALLBACK_PX, &[(0x2003, 0x2003)])
        .expect("rasterises");
    let l = face.lookup(0x2003);
    assert!(l.substituted, "{l:?}");
    let q = l.glyph.expect("the `?` fits");
    assert_eq!(q.code, 0x3F);
}

/// Review finding: every Tessera face at one size had the same id, whichever font file and
/// code ranges made it, so a cache keyed by id could return the wrong face.
#[test]
fn tessera_face_ids_name_font_ranges_and_size() {
    let bundled = TesseraFont::bundled().unwrap();
    let copy = TesseraFont::from_bytes(bundled_font_bytes().to_vec()).unwrap();
    let other = TesseraFont::from_bytes(with_vertical_metrics(1100, -300)).unwrap();
    assert_eq!(bundled.key(), copy.key());
    assert_ne!(bundled.key(), other.key());
    let latin = bundled.rasterise(14, &[(0x20, 0x7E)]).unwrap().id();
    assert_eq!(latin, copy.rasterise(14, &[(0x20, 0x7E)]).unwrap().id());
    assert_eq!(
        latin,
        bundled
            .rasterise(14, &[(0x20, 0x40), (0x41, 0x7E)])
            .unwrap()
            .id(),
        "the same code set"
    );
    assert_ne!(
        latin,
        bundled.rasterise(14, &[(0x400, 0x4FF)]).unwrap().id()
    );
    assert_ne!(latin, other.rasterise(14, &[(0x20, 0x7E)]).unwrap().id());
    assert_ne!(latin, bundled.rasterise(15, &[(0x20, 0x7E)]).unwrap().id());
}
