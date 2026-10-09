// SPDX-License-Identifier: GPL-3.0-or-later
//! The layout, drawing and truncation invariants of `fonts.md` §12 (FN15–FN26) as property
//! tests over synthetic fonts and texts, plus no-panic checks with hostile fonts and
//! parameters.

mod text_common;

use std::sync::Arc;

use proptest::prelude::*;
use tessera_formats::font::Glyph;
use tessera_ui::paint::{CanvasSet, DisplayList, Rect, ops, paint_rect};
use tessera_ui::text::{
    CharListBuilder, DrawParams, FontFace, FontId, LayoutParams, Margins, TextChar, TextLayout,
    TextOp, draw, layout, mouse_to_text, underline_thickness,
};
use text_common::{chars, face};

/// The code units texts are drawn from: letters, space, tab, CR, newline, hyphen, brackets,
/// punctuation, CJK, a character no font has.
const ALPHABET: &[u16] = &[
    0x61, 0x62, 0x63, 0x64, 0x20, 0x20, 0x20, 0x09, 0x0D, 0x0A, 0x2D, 0x28, 0x29, 0x2C, 0x2E, 0x3F,
    0x4E00, 0x3002, 0xE9,
];
/// Codes a synthetic font may have.
const FONT_CODES: &[u16] = &[
    0x20, 0x28, 0x29, 0x2C, 0x2D, 0x2E, 0x3F, 0x61, 0x62, 0x63, 0x64,
];

#[derive(Debug, Clone, Copy)]
struct Shape {
    w: u8,
    h: u8,
    a: i8,
    b: i8,
    t: i8,
}

/// A glyph shape with FN5 (*H* ≥ 1, *T* ≥ 0, *A* + *W* + *B* ≥ 0).
fn shape() -> impl Strategy<Value = Shape> {
    (0u8..12, 1u8..12, -4i8..5, -4i8..5, 0i8..4).prop_map(|(w, h, a, b, t)| {
        let b = if i16::from(a) + i16::from(w) + i16::from(b) < 0 {
            -(a + w as i8)
        } else {
            b
        };
        Shape { w, h, a, b, t }
    })
}

/// A synthetic font: shapes for each code of [`FONT_CODES`] (`?` always present when
/// `question`), boxes packed in one row of a blank atlas.
fn font_from(shapes: &[Shape], question: bool, baseline_frac: u8) -> Arc<FontFace> {
    let mut glyphs = Vec::new();
    let mut x = 0u16;
    for (i, &code) in FONT_CODES.iter().enumerate() {
        if code == 0x3F && !question {
            continue;
        }
        let s = shapes[i % shapes.len()];
        glyphs.push(Glyph {
            width: s.w,
            height: s.h,
            left_bearing: s.a,
            right_bearing: s.b,
            top: s.t,
            code,
            x,
            y: 0,
        });
        x += u16::from(s.w);
    }
    let line_height = glyphs
        .iter()
        .map(|g| g.top as u32 + u32::from(g.height))
        .max()
        .unwrap_or(1);
    let baseline = (line_height - 1) * u32::from(baseline_frac) / 255;
    face(line_height, baseline, glyphs, u32::from(x).max(1), 16)
}

fn text() -> impl Strategy<Value = Vec<u16>> {
    proptest::collection::vec(proptest::sample::select(ALPHABET), 0..60)
}

fn char_list(units: &[u16], slots: &[u8]) -> Vec<TextChar> {
    let mut b = CharListBuilder::new();
    for (i, &u) in units.iter().enumerate() {
        b.slot(slots.get(i).copied().unwrap_or(0) % 2);
        b.push_units(&[u]);
    }
    b.finish()
}

/// The layout-side metrics of one character (§5 step 3, §9.1 rule 4).
fn metrics(c: &TextChar, fonts: &[Option<Arc<FontFace>>]) -> (i64, i64, i64) {
    match fonts.get(usize::from(c.font)).and_then(Option::as_ref) {
        None => (0, 1, 0),
        Some(f) => match f.lookup(c.unit).glyph {
            Some(g) => (
                i64::from(g.left_bearing),
                i64::from(g.width),
                i64::from(g.right_bearing),
            ),
            None => (0, 1, 0),
        },
    }
}

fn check_lines(cs: &[TextChar], fonts: &[Option<Arc<FontFace>>], l: &TextLayout) {
    // Trailing values computed independently (§7.2).
    let mut t = Vec::with_capacity(cs.len());
    let mut prev = 0i64;
    for c in cs {
        let (_, w, b) = metrics(c, fonts);
        let v = if prev >= 0 { b } else { b + (prev + w).min(0) };
        t.push(v);
        prev = v;
    }
    assert!(!l.lines.is_empty());
    // FN17: lines plus consumed characters give back the text, in order.
    let mut seen = Vec::new();
    for line in &l.lines {
        if let Some(c) = line.consumed {
            assert!(
                matches!(cs[c].unit, 0x20 | 0x09 | 0x0D),
                "FN17: consumed {c}"
            );
            seen.push(c);
        }
        seen.extend(line.start..line.end);
    }
    assert_eq!(seen, (0..cs.len()).collect::<Vec<_>>(), "FN17");
    for line in &l.lines {
        if line.start == line.end {
            continue;
        }
        // FN15.
        let last = line.end - 1;
        let p = l.places[last];
        let after = i64::from(p.pen) + i64::from(p.advance);
        assert_eq!(i64::from(line.width), after - t[last], "FN15");
        // T carries across lines and consumed characters, so "every B ≥ 0" means every
        // character up to this one.
        let all_b_nonneg = cs[..line.end].iter().all(|c| metrics(c, fonts).2 >= 0);
        if all_b_nonneg {
            let (a, w, _) = metrics(&cs[last], fonts);
            assert_eq!(
                i64::from(line.width),
                i64::from(p.pen) + a + w,
                "FN15 right edge"
            );
        }
        // §7.2: the indent of the first character.
        let (a0, _, _) = metrics(&cs[line.start], fonts);
        assert_eq!(i64::from(line.indent), if a0 < 0 { 1 - a0 } else { 0 });
        assert_eq!(l.places[line.start].pen, line.indent);
    }
    // FN16.
    let sum: i64 = l
        .lines
        .iter()
        .map(|x| i64::from(x.height) + i64::from(x.leading))
        .sum();
    assert_eq!(l.lines.last().unwrap().leading, 0);
    assert_eq!(i64::from(l.text_height), sum, "FN16");
    // Line tops follow §7.1.
    for w in l.lines.windows(2) {
        assert_eq!(w[1].top, w[0].top + w[0].height + w[0].leading);
    }
}

proptest! {
    #![proptest_config(ProptestConfig::with_cases(256))]

    #[test]
    fn layout_invariants(
        shapes in proptest::collection::vec(shape(), 1..8),
        question in any::<bool>(),
        bl in any::<u8>(),
        units in text(),
        slots in proptest::collection::vec(any::<u8>(), 0..60),
        width in -5i32..120,
        one_line in any::<bool>(),
        leading in 0.0f32..0.6,
        second_font in any::<bool>(),
    ) {
        let f = font_from(&shapes, question, bl);
        let other = if second_font {
            Some(font_from(&shapes[1..].iter().chain(&shapes[..1]).copied().collect::<Vec<_>>(), true, bl / 2))
        } else {
            None
        };
        let fonts = [Some(f), other];
        let cs = char_list(&units, &slots);
        let p = LayoutParams {
            width,
            height: 50,
            one_line,
            leading,
            ..LayoutParams::default()
        };
        let l = layout(&cs, &fonts, &p, None).unwrap();
        check_lines(&cs, &fonts, &l);

        // FN18.
        let k = cs.iter().filter(|c| c.is_newline()).count();
        let ends_nl = cs.last().is_some_and(TextChar::is_newline);
        if one_line {
            prop_assert_eq!(l.lines.len(), if ends_nl { 2 } else { 1 });
        }
        let huge = LayoutParams { width: 1 << 20, ..p.clone() };
        let lh = layout(&cs, &fonts, &huge, None).unwrap();
        if !one_line {
            prop_assert_eq!(lh.lines.len(), k + 1);
        }
        if ends_nl {
            let last = lh.lines.last().unwrap();
            let nl = cs.last().unwrap();
            let want = fonts[usize::from(nl.font)].as_ref().map_or(0, |f| f.line_height().min(255) as i32);
            prop_assert_eq!((last.width, last.baseline, last.leading, last.height), (0, 0, 0, want));
        }
        // FN19.
        if cs.is_empty() {
            let e = l.lines[0];
            prop_assert_eq!((l.lines.len(), e.width, e.height, e.baseline, e.leading, l.text_height), (1, 0, 0, 0, 0, 0));
        }

        // Drawing: every fill box is a glyph box of the font, 1 : 1 (§6.2); outline boxes
        // come first (§8.1); underlines follow FN22.
        let mut ul = CharListBuilder::new();
        for (i, &u) in units.iter().enumerate() {
            ul.slot(slots.get(i).copied().unwrap_or(0) % 2).underline(Some(0xFF00_00FF)).push_units(&[u]);
        }
        let ucs = ul.finish();
        let ulay = layout(&ucs, &fonts, &p, None).unwrap();
        let d = draw(&ulay, &ucs, &fonts, &DrawParams { outline: true, ..DrawParams::default() });
        let mut fill_started = false;
        let mut under = 0usize;
        for op in &d.ops {
            match op {
                TextOp::Glyphs(r) => {
                    if r.colour == 0xFF00_0000 {
                        prop_assert!(!fill_started, "outline after a fill");
                    } else {
                        fill_started = true;
                    }
                    for b in &r.glyphs {
                        prop_assert!(i64::from(b.src[0]) + i64::from(b.size[0]) <= i64::from(r.coverage.width()));
                    }
                }
                TextOp::Underline(f) => {
                    fill_started = true;
                    under += 1;
                    let h = f.rect.height() as i32;
                    prop_assert!(h >= 1, "FN22 thickness");
                    prop_assert!(f.rect.width() >= 1, "FN22 columns");
                }
                TextOp::Bullet(_) => {}
            }
        }
        // At most one underline per drawn character that has a glyph (`?` present in both
        // fonts): contiguous underlines are joined where that changes no pixel, and an
        // underline exists exactly when such a character does.
        if question {
            let drawn = ulay
                .lines
                .iter()
                .flat_map(|x| x.start..x.end)
                .filter(|&i| !ucs[i].is_newline() && fonts[usize::from(ucs[i].font)].is_some())
                .count();
            prop_assert!(under <= drawn);
            prop_assert_eq!(under > 0, drawn > 0);
        }
    }

    #[test]
    fn underline_geometry(
        shapes in proptest::collection::vec(shape(), 1..8),
        units in proptest::collection::vec(proptest::sample::select(FONT_CODES), 1..20),
        leading in 0.0f32..2.0,
    ) {
        let f = font_from(&shapes, true, 200);
        let fonts = [Some(f)];
        let mut b = CharListBuilder::new();
        b.underline(Some(0xFF12_3456)).push_units(&units).push_units(&[0x0A, 0x61]);
        let cs = b.finish();
        let p = LayoutParams { width: 1 << 20, height: 10, leading, ..LayoutParams::default() };
        let l = layout(&cs, &fonts, &p, None).unwrap();
        let d = draw(&l, &cs, &fonts, &DrawParams::default());
        let line = l.lines[0];
        let t = underline_thickness(line.height, line.leading);
        prop_assert_eq!(t, (line.height + line.leading) / 20 + 1);
        let y0 = line.top + line.baseline + t;
        let rects: Vec<Rect> = d.ops.iter().filter_map(|o| match o {
            TextOp::Underline(f) if f.rect.y0 == y0 => Some(f.rect),
            _ => None,
        }).collect();
        // FN22 per character; the fills drawn may join contiguous characters, so the
        // columns they cover are compared as sets.
        let mut want = std::collections::BTreeSet::new();
        for (c, pl) in cs.iter().zip(&l.places).take(units.len()) {
            let (_, _, bb) = metrics(c, &fonts);
            let extra = if bb < 0 { -bb } else { 0 };
            let x0 = i64::from(pl.pen);
            want.extend(x0..x0 + i64::from(pl.stored_advance) + 1 + extra);
        }
        let mut got = std::collections::BTreeSet::new();
        for r in &rects {
            prop_assert_eq!(r.height() as i32, t, "FN22 thickness");
            got.extend(i64::from(r.x0)..i64::from(r.x1));
        }
        prop_assert_eq!(got, want, "FN22 columns");
    }

    #[test]
    fn list_item_extra(
        shapes in proptest::collection::vec(shape(), 1..8),
        units in proptest::collection::vec(proptest::sample::select(FONT_CODES), 1..20),
        x in -20i32..40,
    ) {
        let fonts = [Some(font_from(&shapes, true, 128))];
        let plain = chars(&String::from_utf16_lossy(&units));
        let mut b = CharListBuilder::new();
        b.list_item(true).push_units(&units);
        let listed = b.finish();
        let p = LayoutParams { width: 1 << 20, one_line: true, list_extra: x, ..LayoutParams::default() };
        let a = layout(&plain, &fonts, &p, None).unwrap();
        let l = layout(&listed, &fonts, &p, None).unwrap();
        // FN23.
        prop_assert_eq!(l.places[0].advance - a.places[0].advance, x);
        prop_assert_eq!(a.lines[0].height, l.lines[0].height);
        prop_assert_eq!(a.lines[0].baseline, l.lines[0].baseline);
        for i in 1..units.len() {
            prop_assert_eq!(l.places[i].advance, a.places[i].advance);
        }
    }

    #[test]
    fn truncation(
        shapes in proptest::collection::vec(shape(), 1..8),
        units in text(),
        sfx in proptest::collection::vec(proptest::sample::select(FONT_CODES), 1..4),
        width in 0i32..80,
        height in 0i32..60,
        one_line in any::<bool>(),
        ml in 0i32..4,
        mr in 0i32..4,
    ) {
        let fonts = [Some(font_from(&shapes, true, 128))];
        let cs = char_list(&units, &[]);
        let suffix = char_list(&sfx, &[]);
        let p = LayoutParams {
            width,
            height,
            one_line,
            margins: Margins { left: ml, right: mr, top: 1, bottom: 1 },
            ..LayoutParams::default()
        };
        let plain = layout(&cs, &fonts, &p, None).unwrap();
        let l = layout(&cs, &fonts, &p, Some(&suffix)).unwrap();
        let fits = if one_line { plain.content_width <= width } else { plain.content_height <= height };
        // FN24.
        prop_assert_eq!(l.truncation.is_none(), fits);
        let d = draw(&l, &cs, &fonts, &DrawParams::default());
        if let Some(t) = &l.truncation {
            let line = l.lines[t.line];
            if t.suffix_width <= width - ml - mr - line.indent {
                prop_assert!(line.width <= width - ml - mr, "FN24: {} > {}", line.width, width - ml - mr);
            }
            // Nothing after the cut line is drawn.
            let last_top = line.top + line.height.max(fonts[0].as_ref().unwrap().line_height() as i32);
            for op in &d.ops {
                if let TextOp::Glyphs(r) = op {
                    for b in &r.glyphs {
                        prop_assert!(b.dest[1] < last_top + 1);
                    }
                }
            }
        } else {
            let dp = draw(&plain, &cs, &fonts, &DrawParams::default());
            prop_assert_eq!(format!("{:?}", d.ops), format!("{:?}", dp.ops));
        }
    }

    #[test]
    fn compositing_fn20_fn21(d in any::<u32>(), cov in any::<u8>(), rgb in 0u32..0x0100_0000) {
        let colour = 0xFF00_0000 | rgb;
        // FN20.
        if cov == 0 {
            prop_assert_eq!(ops::text_pixel(d, 0, colour), d);
        }
        if cov == 255 {
            prop_assert_eq!(ops::text_pixel(d, 255, colour), colour);
        }
        let transparent = d & 0x00FF_FFFF;
        if cov > 0 {
            prop_assert_eq!(ops::text_pixel(transparent, cov, colour) >> 24, u32::from(cov));
        }
        // FN21.
        let opaque = d | 0xFF00_0000;
        let out = ops::text_pixel(opaque, cov, colour);
        prop_assert_eq!(out >> 24, 255);
        for shift in [16, 8, 0] {
            let dc = ((opaque >> shift) & 0xFF) as i32;
            let cc = ((colour >> shift) & 0xFF) as i32;
            let oc = ((out >> shift) & 0xFF) as i32;
            prop_assert_eq!(oc, dc + (cc - dc) * (i32::from(cov) + 1) / 256);
            prop_assert!((oc - dc).abs() <= (cc - dc).abs());
        }
    }

    #[test]
    fn mouse_fn26(x in -500i32..500, y in -500i32..500, sx in -50i32..50, sy in -50i32..50) {
        prop_assert_eq!(mouse_to_text([x as f32, y as f32], [1.0, 1.0, 1.0], [sx, sy]), [x + sx, y + sy]);
    }

    /// Hostile fonts and parameters: no panic, and the display list and painter accept the
    /// result.
    #[test]
    fn hostile_never_panics(
        raw in proptest::collection::vec((any::<u8>(), any::<u8>(), any::<i8>(), any::<i8>(), any::<i8>(), any::<u16>(), any::<u16>(), any::<u16>()), 0..24),
        lh in any::<u32>(),
        bl in any::<u32>(),
        border in 0u32..300,
        units in proptest::collection::vec(any::<u16>(), 0..80),
        width in any::<i32>(),
        height in any::<i32>(),
        margins in (any::<i32>(), any::<i32>(), any::<i32>(), any::<i32>()),
        hv in (any::<u32>(), any::<u32>()),
        leading in any::<f32>(),
        list_extra in any::<i32>(),
        origin in (any::<i32>(), any::<i32>()),
        sfx in proptest::collection::vec(any::<u16>(), 0..6),
        one_line in any::<bool>(),
    ) {
        let glyphs: Vec<Glyph> = raw.iter().map(|&(w, h, a, b, t, code, x, y)| Glyph {
            width: w, height: h, left_bearing: a, right_bearing: b, top: t, code, x, y,
        }).collect();
        let map = Arc::new(tessera_ui::paint::CoverageMap::new(8, 8, vec![255; 64]).unwrap());
        let f = Arc::new(FontFace::from_parts(FontId::Data(1), lh, bl, [border, border], glyphs, Arc::clone(&map), Some(map)).unwrap());
        let fonts = [Some(f), None];
        let mut b = CharListBuilder::new();
        for (i, &u) in units.iter().enumerate() {
            if i % 7 == 3 { b.list_item(i % 2 == 0); }
            b.slot((u % 3) as u8).underline((u % 5 == 0).then_some(0xFFFF_FFFF)).push_units(&[u]);
        }
        let cs = b.finish();
        let suffix = char_list(&sfx, &[]);
        let p = LayoutParams {
            width,
            height,
            margins: Margins { left: margins.0, right: margins.1, top: margins.2, bottom: margins.3 },
            one_line,
            horizontal: hv.0,
            vertical: hv.1,
            leading,
            last_line_leading: one_line,
            list_extra,
        };
        let l = layout(&cs, &fonts, &p, (!sfx.is_empty()).then_some(&suffix[..])).unwrap();
        let d = draw(&l, &cs, &fonts, &DrawParams {
            origin: [origin.0, origin.1],
            clip: Rect::new(0, 0, 64, 64),
            outline: true,
            outline_colour: 0x80FF_FFFF,
            bullet: Some([3, 3]),
        });
        let mut list = DisplayList::new();
        d.push_to(&mut list, None).unwrap();
        let mut set = CanvasSet::new(64, 64).unwrap();
        paint_rect(&list, &mut set, Rect::new(0, 0, 64, 64));
    }
}

#[test]
fn text_too_long_is_refused() {
    let fonts = [None];
    let cs = vec![TextChar::plain(0x41, 0, 0); tessera_ui::text::MAX_TEXT_UNITS + 1];
    assert!(layout(&cs, &fonts, &LayoutParams::default(), None).is_err());
}
