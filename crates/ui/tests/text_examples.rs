// SPDX-License-Identifier: GPL-3.0-or-later
//! The synthetic worked examples of `fonts.md` §13 (and the rules of §5–§9 they illustrate),
//! run through `tessera_ui::text`: glyph lookup, placement, line layout, breaking,
//! alignment, list items, truncation, drawing, underlines, compositing (through
//! `tessera_ui::paint`), font choice and the mouse conversion of §6.3.

mod text_common;

use std::sync::Arc;

use tessera_formats::font::Glyph;
use tessera_ui::paint::{
    CanvasSet, DisplayList, FillRect, GlyphBox, PaintOp, Rect, ops, paint_rect,
};
use tessera_ui::text::{
    CharListBuilder, DrawParams, FontFace, FontIndexSession, FontProps, FontSlots, FontStruct,
    JUSTIFY_BOTTOM, JUSTIFY_CENTER, JUSTIFY_LEFT, JUSTIFY_RIGHT, JUSTIFY_TOP, LayoutParams,
    Margins, SLOT_TAG, SlotSource, StructMember, TextChar, TextLayout, TextOp, TextPaint, draw,
    layout, leading_factor, list_extra, mouse_to_text,
};
use text_common::*;

const WHITE: u32 = 0xFFFF_FFFF;

fn wide() -> LayoutParams {
    LayoutParams {
        width: 1000,
        height: 1000,
        ..LayoutParams::default()
    }
}

fn lay(
    text: &str,
    fonts: &[Option<Arc<FontFace>>],
    p: &LayoutParams,
) -> (Vec<TextChar>, TextLayout) {
    let cs = chars(text);
    let l = layout(&cs, fonts, p, None).expect("layout");
    (cs, l)
}

/// All glyph boxes of a paint, with each run's colour, in drawing order.
fn boxes(p: &TextPaint) -> Vec<(u32, GlyphBox)> {
    p.ops
        .iter()
        .filter_map(|op| match op {
            TextOp::Glyphs(r) => Some(r.glyphs.iter().map(move |b| (r.colour, *b))),
            _ => None,
        })
        .flatten()
        .collect()
}

fn gb(dest: [i32; 2], size: [u32; 2], src: [i32; 2]) -> GlyphBox {
    GlyphBox { dest, size, src }
}

#[test]
fn ex_13_1_record_and_lookup() {
    let f = face_13_1(true);
    assert_eq!(f.line_height(), 10);
    assert_eq!(f.baseline(), 8);
    assert_eq!(f.borders(), [1, 1]);
    assert_eq!(f.glyphs(), &glyphs_13_1()[..]);
    assert_eq!(f.foreground().width(), 32);
    assert_eq!(f.foreground().height(), 12);
    assert!(f.background().is_some());
    let a = f.lookup(0x41);
    assert!(!a.substituted);
    assert_eq!(a.glyph.unwrap().x, 12);
    // §4.2 packing of the fixture: X = 1, 6, 12, 20; the last bordered box ends at 24 ≤ 32.
    let xs: Vec<u16> = f.glyphs().iter().map(|g| g.x).collect();
    assert_eq!(xs, [1, 6, 12, 20]);
    // The face is built on a worker and shared.
    fn send_sync<T: Send + Sync>(_: &T) {}
    send_sync(&*f);
}

#[test]
fn ex_13_1_atlas_mismatch_and_missing_background_are_errors() {
    let (fg, _) = atlases(&glyphs_13_1(), 1, 32, 12);
    let wrong = surface_payload(0x41AB_0099, 13, 32, 12, &fg);
    assert!(FontFace::from_payloads(&hex(RECORD_13_1), &wrong, None).is_err());
    let fgp = surface_payload(FG_DID, 13, 32, 12, &fg);
    assert!(
        FontFace::from_payloads(&hex(RECORD_13_1), &fgp, None).is_err(),
        "the record names a background atlas"
    );
    let short = surface_payload(FG_DID, 13, 32, 12, &fg[..10]);
    assert!(FontFace::from_payloads(&hex(RECORD_13_1), &short, None).is_err());
    assert!(FontFace::from_payloads(&hex(RECORD_13_1)[..79], &fgp, None).is_err());
}

#[test]
fn ex_13_2_one_line_placement() {
    let f = face_13_1(true);
    let fonts = [Some(f)];
    let (cs, l) = lay("jA ?", &fonts, &wide());
    assert_eq!(l.lines.len(), 1);
    let line = l.lines[0];
    assert_eq!(line.indent, 3);
    assert_eq!(
        (line.height, line.baseline, line.leading, line.top),
        (10, 8, 0, 0)
    );
    let pens: Vec<i32> = l.places.iter().map(|p| p.pen).collect();
    assert_eq!(pens, [3, 5, 12, 16]);
    let adv: Vec<i32> = l.places.iter().map(|p| p.advance).collect();
    assert_eq!(adv, [2, 7, 4, 5]);
    assert_eq!(line.width, 20, "21 − T of `?`");
    assert_eq!(l.text_height, 10);

    let p = draw(&l, &cs, &fonts, &DrawParams::default());
    let got: Vec<GlyphBox> = boxes(&p).into_iter().map(|(_, b)| b).collect();
    assert_eq!(
        got,
        [
            gb([1, 2], [3, 8], [20, 1]),
            gb([5, 1], [6, 7], [12, 1]),
            gb([12, 0], [3, 10], [1, 1]),
            gb([16, 1], [4, 7], [6, 1]),
        ]
    );
    // One colour and one atlas: a single run.
    assert_eq!(p.ops.len(), 1);
}

#[test]
fn ex_13_3_substitution() {
    let f = face_13_1(true);
    let s = f.lookup(0x00E9);
    assert!(s.substituted);
    assert_eq!(s.glyph.unwrap().code, 0x3F);
    // A surrogate pair shows as two `?`; control characters too.
    let fonts = [Some(f)];
    let (cs, l) = lay("\u{1F600}\t", &fonts, &wide());
    assert_eq!(cs.len(), 3);
    assert!(l.places.iter().all(|p| p.advance == 5));
    let p = draw(&l, &cs, &fonts, &DrawParams::default());
    assert_eq!(boxes(&p).len(), 3, "a tab is drawn as `?`");
}

#[test]
fn ex_13_3_outline_from_the_background_atlas() {
    let f = face_13_1(true);
    let fonts = [Some(f)];
    let (cs, l) = lay("jA ?", &fonts, &wide());
    let params = DrawParams {
        outline: true,
        outline_colour: 0xFF10_2030,
        ..DrawParams::default()
    };
    let p = draw(&l, &cs, &fonts, &params);
    let all = boxes(&p);
    assert_eq!(all.len(), 8, "four outlines, then four fills");
    assert!(all[..4].iter().all(|(c, _)| *c == 0xFF10_2030));
    assert!(all[4..].iter().all(|(c, _)| *c == WHITE));
    assert_eq!(all[1].1, gb([4, 0], [8, 9], [11, 0]));
    assert_eq!(all[5].1, gb([5, 1], [6, 7], [12, 1]));
    // The outline runs read the background atlas, the fills the foreground.
    let TextOp::Glyphs(first) = &p.ops[0] else {
        panic!()
    };
    let face = fonts[0].as_ref().unwrap();
    assert!(Arc::ptr_eq(&first.coverage, face.background().unwrap()));
}

#[test]
fn ex_13_3_stamped_outline() {
    let f = face_13_1(false);
    assert!(f.background().is_none());
    let fonts = [Some(f)];
    let (cs, l) = lay("jA ?", &fonts, &wide());
    let params = DrawParams {
        outline: true,
        ..DrawParams::default()
    };
    let p = draw(&l, &cs, &fonts, &params);
    let all = boxes(&p);
    assert_eq!(
        all.len(),
        4 * 8 + 4,
        "eight stamps per glyph, then the fills"
    );
    let a = &all[8..16];
    let dests: Vec<[i32; 2]> = a.iter().map(|(_, b)| b.dest).collect();
    assert_eq!(
        dests,
        [
            [4, 0],
            [5, 0],
            [6, 0],
            [4, 1],
            [6, 1],
            [4, 2],
            [5, 2],
            [6, 2]
        ]
    );
    assert!(
        a.iter()
            .all(|(c, b)| *c == 0xFF00_0000 && b.src == [12, 1] && b.size == [6, 7])
    );
    assert_eq!(all[33].1.dest, [5, 1]);
}

fn wrapped(text: &str, width: i32) -> (Vec<TextChar>, TextLayout) {
    let fonts = [Some(face_13_1(true))];
    let p = LayoutParams {
        width,
        height: 1000,
        ..LayoutParams::default()
    };
    lay(text, &fonts, &p)
}

#[test]
fn ex_13_4_breaking() {
    let (cs, l) = wrapped("A A A", 10);
    assert_eq!(line_texts(&l, &cs), ["A", "A", "A"]);
    assert!(l.lines.iter().all(|x| x.width == 6 && x.height == 10));
    assert_eq!(l.text_height, 30);
    assert_eq!(l.lines[1].consumed, Some(1));
    assert!(l.places[1].consumed && l.places[3].consumed);

    // Runs of spaces.
    let (cs, l) = wrapped("A   A", 13);
    assert_eq!(line_texts(&l, &cs), ["A ", " A"]);
    assert_eq!(l.lines[0].width, 10);
    assert_eq!(l.lines[1].width, 10);
    assert_eq!(l.lines[1].consumed, Some(2));

    // Over-long word: forced breaks.
    let (cs, l) = wrapped("AAA", 10);
    assert_eq!(line_texts(&l, &cs), ["A", "A", "A"]);
    assert!(l.lines.iter().all(|x| x.width == 6 && x.consumed.is_none()));
}

#[test]
fn ex_13_4_break_opportunities() {
    // Every character but the space has the `?` metrics here: A + W = 4, advance 5.
    // After a hyphen: the hyphen ends the line, nothing consumed.
    let (cs, l) = wrapped("ab-cd", 16);
    assert_eq!(line_texts(&l, &cs), ["ab-", "cd"]);
    assert_eq!(l.lines[0].width, 14);
    // No opportunity before a comma: forced break; one before the space after it.
    let (cs, l) = wrapped("x, y", 6);
    assert_eq!(line_texts(&l, &cs), ["x", ",", "y"]);
    assert_eq!(l.lines[2].consumed, Some(2));
    // No opportunity before a space that follows `(`: nothing is consumed.
    let (cs, l) = wrapped("( x", 6);
    assert_eq!(line_texts(&l, &cs), ["(", " ", "x"]);
    assert!(l.lines.iter().all(|x| x.consumed.is_none()));
    // Between CJK ideographs always, never before U+3002; moved characters are not
    // re-tested.
    let (cs, l) = wrapped("\u{4E00}\u{4E01}\u{3002}", 9);
    assert_eq!(line_texts(&l, &cs), ["\u{4E00}", "\u{4E01}\u{3002}"]);
    // The first character of the text is never an opportunity; a first glyph wider than
    // the line gives an empty first line (§7.1 case 4).
    let (cs, l) = wrapped("A", 3);
    assert_eq!(line_texts(&l, &cs), ["", "A"]);
    let e = l.lines[0];
    assert_eq!((e.width, e.height, e.baseline, e.leading), (0, 0, 0, 0));
}

#[test]
fn ex_13_5_choosing_fonts() {
    const LOCALE: u32 = 0x4200_0028;
    let resolves = |d: u32| d >> 24 == 0x42 && d != 0x4200_DEAD;

    // No font property anywhere: both slots keep the locale default font.
    let mut s = FontSlots::new(LOCALE);
    s.apply(&FontProps::default(), 0, resolves);
    assert_eq!(s, FontSlots::new(LOCALE));

    // A DID of 0 or one that does not resolve leaves the slot unchanged (rule 4).
    let props = FontProps {
        normal: SlotSource {
            font: Some(0),
            fonts: Some(vec![0x4200_DEAD]),
            ..SlotSource::default()
        },
        ..FontProps::default()
    };
    s.apply(&props, 0, resolves);
    assert_eq!(s, FontSlots::new(LOCALE));

    // The tag slot without properties takes the normal slot's (rule 2).
    let props = FontProps {
        normal: SlotSource {
            fonts: Some(vec![0x4200_000C]),
            ..SlotSource::default()
        },
        ..FontProps::default()
    };
    let mut s = FontSlots::new(LOCALE);
    s.apply(&props, 0, resolves);
    assert_eq!((s.normal, s.tag), (0x4200_000C, 0x4200_000C));

    // Rule 5: by property names a member of the font struct; a missing member or a non-DID
    // changes nothing.
    const MEMBER_DEFAULT: u32 = 100;
    const MEMBER_OTHER: u32 = 101;
    const MEMBER_NOT_DID: u32 = 102;
    let st = FontStruct {
        members: vec![
            (MEMBER_DEFAULT, StructMember::Did(0x4200_0029)),
            (MEMBER_OTHER, StructMember::Did(0x4200_0001)),
            (MEMBER_NOT_DID, StructMember::Other),
        ],
    };
    let mut props = FontProps {
        normal: SlotSource {
            font_struct: Some(st),
            ..SlotSource::default()
        },
        font_member: MEMBER_DEFAULT,
        tag_font_member: 200,
        ..FontProps::default()
    };
    let mut s = FontSlots::new(LOCALE);
    s.apply(&props, 0, resolves);
    assert_eq!(s.normal, 0x4200_0029);
    props.set_font_by_property = Some(MEMBER_OTHER);
    let mut s = FontSlots::new(LOCALE);
    s.apply(&props, 0, resolves);
    assert_eq!((s.normal, s.tag), (0x4200_0001, 0x4200_0001));
    props.set_font_by_property = Some(MEMBER_NOT_DID);
    let mut s = FontSlots::new(LOCALE);
    s.apply(&props, 0, resolves);
    assert_eq!(s.normal, LOCALE);

    // Quest dispenser: two presses of "increase" from 0 → index 2: the 7-entry ladder
    // gives its third entry, the 6-entry twist-name ladder its third. At index 6 the
    // twist name keeps the entry it had.
    let ladder7: Vec<u32> = (0..7).map(|i| 0x4200_0100 + i).collect();
    let ladder6: Vec<u32> = (0..6).map(|i| 0x4200_0200 + i).collect();
    let lens = [ladder7.len(), ladder6.len()];
    let mut session = FontIndexSession::new();
    assert!(session.increase(&lens));
    assert!(session.increase(&lens));
    assert_eq!(session.index(), 2);
    let with = |a: &Vec<u32>| FontProps {
        normal: SlotSource {
            fonts: Some(a.clone()),
            ..SlotSource::default()
        },
        ..FontProps::default()
    };
    let (mut quest, mut twist) = (FontSlots::new(LOCALE), FontSlots::new(LOCALE));
    quest.apply(&with(&ladder7), session.index(), resolves);
    twist.apply(&with(&ladder6), session.index(), resolves);
    assert_eq!((quest.normal, twist.normal), (ladder7[2], ladder6[2]));
    for _ in 0..10 {
        session.increase(&lens);
    }
    assert_eq!(session.index(), 6, "stays within the longest array");
    for i in 3..=6 {
        quest.apply(&with(&ladder7), i, resolves);
        twist.apply(&with(&ladder6), i, resolves);
    }
    assert_eq!((quest.normal, quest.tag), (ladder7[6], ladder7[6]));
    assert_eq!(
        twist.normal, ladder6[5],
        "an index past the end changes nothing"
    );
    let mut s0 = FontIndexSession::new();
    assert!(!s0.decrease(&lens), "a negative request is refused");
    assert_eq!(s0.index(), 0);

    // Normal slot N and tag slot G: "ab", a colour span "cd", a link label "ef".
    let n = face(10, 8, vec![g(4, 7, 0, 1, 1, 0x3F, 0, 0)], 8, 8);
    let gface = face(20, 16, vec![g(8, 14, 0, 2, 2, 0x3F, 0, 0)], 16, 16);
    let mut b = CharListBuilder::new();
    b.push_str("ab").colour(0xFF00_FF00).push_str("cd");
    b.slot(SLOT_TAG).colour(0xFF00_00FF).push_str("ef");
    let cs = b.finish();
    let fonts = [Some(n), Some(gface)];
    let l = layout(&cs, &fonts, &wide(), None).unwrap();
    let adv: Vec<i32> = l.places.iter().map(|p| p.advance).collect();
    assert_eq!(adv, [5, 5, 5, 5, 10, 10]);
    assert_eq!((l.lines[0].height, l.lines[0].baseline), (20, 16));
    let p = draw(&l, &cs, &fonts, &DrawParams::default());
    let cols: Vec<u32> = boxes(&p).iter().map(|(c, _)| *c).collect();
    assert_eq!(
        cols,
        [
            WHITE,
            WHITE,
            0xFF00_FF00,
            0xFF00_FF00,
            0xFF00_00FF,
            0xFF00_00FF
        ]
    );
    // Both fonts share the line's baseline (16): N's `?` top 16 − 8 + 1, G's 16 − 16 + 2.
    let tops: Vec<i32> = boxes(&p).iter().map(|(_, b)| b.dest[1]).collect();
    assert_eq!(tops, [9, 9, 9, 9, 2, 2]);
}

#[test]
fn ex_13_6_blank_lines_and_a_final_newline() {
    let fonts = [Some(face_13_1(true))];
    let f = leading_factor(0.0, 0.2);
    let p = LayoutParams {
        leading: f,
        ..wide()
    };
    let (cs, l) = lay("A\n\nA", &fonts, &p);
    let rows: Vec<(i32, i32, i32, i32, i32)> = l
        .lines
        .iter()
        .map(|x| (x.width, x.height, x.baseline, x.leading, x.top))
        .collect();
    assert_eq!(
        rows,
        [(11, 10, 8, 2, 0), (4, 10, 8, 2, 12), (6, 10, 8, 0, 24)]
    );
    assert_eq!(l.text_height, 34);
    let d = draw(&l, &cs, &fonts, &DrawParams::default());
    assert_eq!(boxes(&d).len(), 2, "neither newline is drawn");

    let (_, l) = lay("A\n", &fonts, &p);
    let rows: Vec<(i32, i32, i32, i32, i32)> = l
        .lines
        .iter()
        .map(|x| (x.width, x.height, x.baseline, x.leading, x.top))
        .collect();
    assert_eq!(rows, [(11, 10, 8, 2, 0), (0, 10, 0, 0, 12)]);
    assert_eq!(l.text_height, 22);

    let (_, l) = lay("", &fonts, &p);
    assert_eq!(l.lines.len(), 1);
    let e = l.lines[0];
    assert_eq!((e.width, e.height, e.baseline, e.leading), (0, 0, 0, 0));
    assert_eq!(l.text_height, 0);

    // Wrapping off.
    let one = LayoutParams {
        one_line: true,
        ..p.clone()
    };
    let (cs, l) = lay("A\nA", &fonts, &one);
    assert_eq!(l.lines.len(), 1);
    assert_eq!(l.lines[0].width, 18);
    let d = draw(&l, &cs, &fonts, &DrawParams::default());
    let xs: Vec<i32> = boxes(&d).iter().map(|(_, b)| b.dest[0]).collect();
    assert_eq!(xs, [0, 7], "the newline does not move the drawing pen");

    let (_, l) = lay("A\n", &fonts, &one);
    let rows: Vec<(i32, i32, i32, i32, i32)> = l
        .lines
        .iter()
        .map(|x| (x.width, x.height, x.baseline, x.leading, x.top))
        .collect();
    assert_eq!(rows, [(11, 10, 8, 2, 0), (0, 10, 0, 0, 12)]);
    assert_eq!(l.text_height, 22);

    let (_, l) = lay("A\nA\n", &fonts, &one);
    assert_eq!(l.lines.len(), 2);
    assert_eq!(
        (l.lines[0].start, l.lines[0].end, l.lines[0].width),
        (0, 4, 23)
    );
    assert_eq!((l.lines[1].width, l.lines[1].baseline), (0, 0));

    // `UICore_Text_last_line_leading` keeps the last line's leading.
    let keep = LayoutParams {
        last_line_leading: true,
        ..p
    };
    let (_, l) = lay("A\n\nA", &fonts, &keep);
    assert_eq!(l.text_height, 36);
    assert_eq!(l.lines[2].leading, 2);
}

#[test]
fn ex_13_7_alignment_with_margins() {
    let fonts = [Some(face_13_1(true))];
    let base = LayoutParams {
        width: 40,
        height: 30,
        margins: Margins {
            left: 2,
            right: 2,
            top: 3,
            bottom: 1,
        },
        ..LayoutParams::default()
    };
    for (h, x) in [
        (JUSTIFY_RIGHT, 32),
        (JUSTIFY_BOTTOM, 32),
        (JUSTIFY_CENTER, 17),
        (0, 2),
        (JUSTIFY_TOP, 2),
        (JUSTIFY_LEFT, 2),
        (77, 2),
    ] {
        let p = LayoutParams {
            horizontal: h,
            ..base.clone()
        };
        let (_, l) = lay("A", &fonts, &p);
        assert_eq!(l.lines[0].x + l.places[0].pen, x, "horizontal {h}");
        assert_eq!(l.content_width, 10);
    }
    for (v, top, glyph_top) in [
        (JUSTIFY_BOTTOM, 15, 16),
        (JUSTIFY_RIGHT, 15, 16),
        (JUSTIFY_CENTER, 9, 10),
        (0, 3, 4),
        (JUSTIFY_TOP, 3, 4),
        (JUSTIFY_LEFT, 3, 4),
    ] {
        let p = LayoutParams {
            vertical: v,
            ..base.clone()
        };
        let (cs, l) = lay("A", &fonts, &p);
        assert_eq!(l.lines[0].top, top, "vertical {v}");
        assert_eq!(l.content_height, 14);
        let d = draw(&l, &cs, &fonts, &DrawParams::default());
        assert_eq!(boxes(&d)[0].1.dest[1], glyph_top);
    }
    // Margins larger than the element: the available extent counts as 0 (Tessera Decision).
    let p = LayoutParams {
        width: 2,
        height: 2,
        margins: Margins {
            left: 5,
            right: 5,
            top: 5,
            bottom: 5,
        },
        horizontal: JUSTIFY_CENTER,
        vertical: JUSTIFY_CENTER,
        ..LayoutParams::default()
    };
    let (_, l) = lay("A", &fonts, &p);
    assert!(l.lines[0].x >= 0);
}

#[test]
fn ex_13_8_compositing_arithmetic() {
    let c = 0xFFC8_6400; // (200, 100, 0), opaque
    assert_eq!(ops::text_pixel(0x0000_0000, 128, c), 0x80C8_6400);
    assert_eq!(ops::text_pixel(0xFF00_00FF, 128, c), 0xFF64_327F);
    assert_eq!(ops::text_pixel(0xFF00_00FF, 0, c), 0xFF00_00FF);
    assert_eq!(ops::text_pixel(0xFF00_00FF, 255, c), 0xFFC8_6400);
    assert_eq!(ops::text_pixel(0x80C8_6400, 64, 0xFF00_00FF), 0xA178_3C66);
    assert_eq!(ops::text_pixel(0xFF00_0000, 255, 0x7FFF_FFFF), 0xFF7F_7F7F);
    assert_eq!(ops::text_pixel(0x0000_0000, 255, 0x7FFF_FFFF), 0xFFFF_FFFF);
    assert_eq!(ops::pack_colour([0.5, 0.25, 1.0, 1.0]), 0xFF7F_3FFF);

    // End to end: a glyph of coverage 128 drawn through text::draw and the painter.
    let glyphs = vec![g(4, 7, 0, 1, 1, 0x3F, 0, 0), g(2, 2, 0, 0, 0, 0x41, 4, 0)];
    let mut data = vec![0u8; 8 * 8];
    data[4] = 128;
    let map = Arc::new(tessera_ui::paint::CoverageMap::new(8, 8, data).unwrap());
    let f = Arc::new(
        FontFace::from_parts(
            tessera_ui::text::FontId::Data(0x4200_0001),
            2,
            2,
            [0, 0],
            glyphs,
            map,
            None,
        )
        .unwrap(),
    );
    let fonts = [Some(f)];
    let mut b = CharListBuilder::new();
    b.colour(c).push_str("A");
    let cs = b.finish();
    let l = layout(&cs, &fonts, &wide(), None).unwrap();
    let paint = draw(
        &l,
        &cs,
        &fonts,
        &DrawParams {
            origin: [1, 0],
            ..DrawParams::default()
        },
    );
    let mut list = DisplayList::new();
    list.push(PaintOp::Fill(FillRect {
        rect: Rect::new(0, 0, 2, 2),
        clip: Rect::new(0, 0, 4, 4),
        colour: 0xFF00_00FF,
    }))
    .unwrap();
    paint.push_to(&mut list, None).unwrap();
    let mut set = CanvasSet::new(4, 4).unwrap();
    paint_rect(&list, &mut set, Rect::new(0, 0, 4, 4));
    let canvas = set.below();
    assert_eq!(
        canvas.pixel(1, 0),
        0xFF64_327F,
        "opaque destination, step 5"
    );
    assert_eq!(
        canvas.pixel(2, 0),
        0x0000_0000,
        "coverage 0 over transparent"
    );
    assert_eq!(canvas.pixel(0, 0), 0xFF00_00FF, "outside the glyph");
}

#[test]
fn ex_13_9_underline() {
    let fonts = [Some(face_13_1(true))];
    let mut b = CharListBuilder::new();
    b.push_str("j").underline(Some(0xFF11_2233)).push_str("A");
    b.underline(None).push_str(" ?");
    let cs = b.finish();
    let l = layout(&cs, &fonts, &wide(), None).unwrap();
    let p = draw(&l, &cs, &fonts, &DrawParams::default());
    let unders: Vec<FillRect> = p
        .ops
        .iter()
        .filter_map(|o| match o {
            TextOp::Underline(f) => Some(*f),
            _ => None,
        })
        .collect();
    assert_eq!(unders.len(), 1);
    assert_eq!(
        unders[0].rect,
        Rect::new(5, 9, 13, 10),
        "row 9, columns 5..=12"
    );
    assert_eq!(unders[0].colour, 0xFF11_2233);
    // The underline comes before its glyph in the fill pass.
    let order: Vec<&str> = p
        .ops
        .iter()
        .map(|o| match o {
            TextOp::Glyphs(_) => "g",
            TextOp::Underline(_) => "u",
            TextOp::Bullet(_) => "b",
        })
        .collect();
    assert_eq!(order, ["g", "u", "g"]);

    // A 20-pixel line with leading 2: t = 2, rows L + 2 to L + 3. A glyph with advance 5
    // and B = −1 at pen 10: columns 10 to 16.
    let f20 = face(
        20,
        16,
        vec![g(4, 7, 0, 1, 1, 0x3F, 0, 0), g(6, 7, 0, -1, 1, 0x42, 4, 0)],
        16,
        16,
    );
    let fonts = [Some(f20)];
    let mut b = CharListBuilder::new();
    b.push_str("??").underline(Some(WHITE)).push_str("B");
    b.underline(None).push_str("\n?");
    let cs = b.finish();
    let p = LayoutParams {
        leading: 0.1,
        ..wide()
    };
    let l = layout(&cs, &fonts, &p, None).unwrap();
    assert_eq!(l.lines[0].leading, 2);
    let d = draw(&l, &cs, &fonts, &DrawParams::default());
    let u = d
        .ops
        .iter()
        .find_map(|o| match o {
            TextOp::Underline(f) => Some(*f),
            _ => None,
        })
        .unwrap();
    assert_eq!(u.rect, Rect::new(10, 18, 17, 20));
}

#[test]
fn ex_13_10_truncation_wrapping_off() {
    let fonts = [Some(face_13_1(true))];
    let suffix = chars("?");
    let cs = chars("AAAAA");
    let p = LayoutParams {
        width: 30,
        height: 10,
        one_line: true,
        ..LayoutParams::default()
    };
    let l = layout(&cs, &fonts, &p, Some(&suffix)).unwrap();
    let t = l.truncation.as_ref().expect("cut");
    assert_eq!((t.line, t.cut, t.suffix_width), (0, 3, 4));
    assert_eq!(l.lines[0].width, 25);
    assert_eq!(l.content_width, 25);
    let d = draw(&l, &cs, &fonts, &DrawParams::default());
    let got: Vec<([i32; 2], [i32; 2])> = boxes(&d).iter().map(|(_, b)| (b.dest, b.src)).collect();
    assert_eq!(
        got,
        [
            ([0, 1], [12, 1]),
            ([7, 1], [12, 1]),
            ([14, 1], [12, 1]),
            ([21, 1], [6, 1])
        ]
    );

    let p35 = LayoutParams { width: 35, ..p };
    let l = layout(&cs, &fonts, &p35, Some(&suffix)).unwrap();
    assert!(!l.is_truncated());
    assert_eq!(
        boxes(&draw(&l, &cs, &fonts, &DrawParams::default())).len(),
        5
    );
}

#[test]
fn ex_13_10_truncation_wrapping_on() {
    let fonts = [Some(face_13_1(true))];
    let suffix = chars("??");
    let cs = chars("AAA AAA AAA");
    let p = LayoutParams {
        width: 30,
        height: 25,
        ..LayoutParams::default()
    };
    let l = layout(&cs, &fonts, &p, Some(&suffix)).unwrap();
    assert_eq!(l.lines.len(), 3);
    let t = l.truncation.as_ref().expect("cut");
    assert_eq!((t.line, t.cut, t.suffix_width), (1, 6, 9));
    assert_eq!(l.content_width, 23);
    assert_eq!(l.content_height, 20);
    let d = draw(&l, &cs, &fonts, &DrawParams::default());
    let got: Vec<[i32; 2]> = boxes(&d).iter().map(|(_, b)| b.dest).collect();
    assert_eq!(
        got,
        [
            [0, 1],
            [7, 1],
            [14, 1],
            [0, 11],
            [7, 11],
            [14, 11],
            [19, 11]
        ],
        "line 0, then \"AA\" and the suffix from pen 14; line 2 not drawn"
    );
}

#[test]
fn ex_13_11_list_item_with_a_bullet() {
    let fonts = [Some(face_13_1(true))];
    let mut b = CharListBuilder::new();
    b.list_item(true).push_str("A");
    let cs = b.finish();
    assert_eq!(cs.len(), 1);
    assert!(cs[0].list_item);
    let x = list_extra(Some(5), 2);
    assert_eq!(x, 7);
    let p = LayoutParams {
        list_extra: x,
        ..wide()
    };
    let l = layout(&cs, &fonts, &p, None).unwrap();
    assert_eq!(l.places[0].advance, 14);
    assert_eq!(l.lines[0].width, 13);
    assert_eq!((l.lines[0].height, l.lines[0].baseline), (10, 8));
    let d = draw(
        &l,
        &cs,
        &fonts,
        &DrawParams {
            bullet: Some([5, 4]),
            ..DrawParams::default()
        },
    );
    let TextOp::Bullet(r) = d.ops[0] else {
        panic!("bullet first")
    };
    assert_eq!(r, Rect::from_xywh(0, 4, 5, 4));
    assert_eq!(boxes(&d)[0].1.dest, [7, 1]);
    // Without an image the pen still moves by X.
    let d = draw(&l, &cs, &fonts, &DrawParams::default());
    assert_eq!(d.ops.len(), 1);
    assert_eq!(boxes(&d)[0].1.dest, [7, 1]);
}

#[test]
fn ex_13_12_mouse_position_in_a_scaled_panel() {
    assert_eq!(
        mouse_to_text([31.0, 16.0], [1.5, 1.5, 1.0], [0, 5]),
        [20, 15]
    );
    assert_eq!(
        mouse_to_text([31.0, 16.0], [1.0, 1.0, 1.0], [0, 5]),
        [31, 21]
    );
}

#[test]
fn missing_question_mark_and_slot_without_font() {
    // §5 step 3: no glyph and no `?`: layout uses a 1 × 1 blank glyph, drawing nothing.
    let f = face(10, 8, vec![g(4, 7, 0, 1, 1, 0x41, 0, 0)], 8, 8);
    let fonts = [Some(f)];
    let (cs, l) = lay("xA", &fonts, &wide());
    assert_eq!(l.places[0].advance, 1);
    assert_eq!(l.places[1].pen, 1);
    let d = draw(&l, &cs, &fonts, &DrawParams::default());
    assert_eq!(boxes(&d)[0].1.dest[0], 0, "the drawing pen advanced 0");
    // §9.1 rule 4: a slot without any font: advance 1 per character, line height 0.
    let (cs, l) = lay("abc", &[None], &wide());
    assert_eq!(l.lines[0].width, 3);
    assert_eq!(l.lines[0].height, 0);
    assert!(
        draw(&l, &cs, &[None], &DrawParams::default())
            .ops
            .is_empty()
    );
}

#[test]
fn clip_and_origin() {
    let fonts = [Some(face_13_1(true))];
    let (cs, l) = lay("AAAA", &fonts, &wide());
    let params = DrawParams {
        origin: [100, 50],
        clip: Rect::new(100, 50, 110, 60),
        ..DrawParams::default()
    };
    let d = draw(&l, &cs, &fonts, &params);
    let got: Vec<[i32; 2]> = boxes(&d).iter().map(|(_, b)| b.dest).collect();
    assert_eq!(
        got,
        [[100, 51], [107, 51]],
        "glyphs outside the clip are dropped"
    );
    assert!(params.clip.contains_rect(&d.area()));
}

#[test]
fn glyph_struct_is_reexported_unchanged() {
    // The face hands out the record's entries as they are.
    let f = face_13_1(true);
    let j: Glyph = f.lookup(0x6A).glyph.unwrap();
    assert_eq!((j.left_bearing, j.top), (-2, 2));
}
