// SPDX-License-Identifier: GPL-3.0-or-later
//! Regression tests for the review findings on `tessera_ui::text` (layout, font choice,
//! drawing and the display-list hand-over). The fonts are the synthetic §13.1 font of
//! `fonts.md` and small fonts built here; no game data.

mod text_common;

use std::sync::Arc;

use tessera_ui::paint::{DisplayList, FillRect, MAX_OPS, PaintError, PaintOp, Rect};
use tessera_ui::text::{
    CharListBuilder, DrawParams, FontFace, FontProps, FontSlots, FontStruct, LayoutParams,
    MAX_TEXT_UNITS, SlotSource, StructMember, TextChar, TextError, TextLayout, TextOp, TextPaint,
    draw, layout,
};
use text_common::*;

fn wrapped(text: &str, width: i32) -> (Vec<TextChar>, TextLayout) {
    let fonts = [Some(face_13_1(true))];
    let cs = chars(text);
    let p = LayoutParams {
        width,
        height: 1000,
        ..LayoutParams::default()
    };
    let l = layout(&cs, &fonts, &p, None).expect("layout");
    (cs, l)
}

/// Every line wider than the available width holds a single glyph (§7.3 rule 5), and only
/// the forced-break edge case gives a line without characters.
fn check_widths(l: &TextLayout, avail: i32) {
    for ln in &l.lines {
        if ln.width > avail {
            assert_eq!(ln.end - ln.start, 1, "{ln:?}");
        }
    }
}

fn kinds(p: &TextPaint) -> Vec<&'static str> {
    p.ops
        .iter()
        .map(|o| match o {
            TextOp::Glyphs(_) => "g",
            TextOp::Underline(_) => "u",
            TextOp::Bullet(_) => "b",
        })
        .collect()
}

/// Finding: a break opportunity before a line's own first character ended the line there:
/// an empty line, the leading space consumed and the moved characters untested (§7.3).
#[test]
fn no_break_before_a_lines_first_character() {
    // The overflow on the first space consumes it; the second space starts line 2 as a
    // blank advance (§7.3 "the spaces after it start the next line").
    let (cs, l) = wrapped("A  A", 9);
    assert_eq!(line_texts(&l, &cs), ["A", " ", "A"]);
    assert_eq!(l.lines[1].consumed, Some(1));
    assert_eq!(l.lines[1].width, 3);
    assert!(l.lines.iter().all(|ln| ln.start < ln.end));
    check_widths(&l, 9);

    // After a newline the leading space stays (§7.3 "spaces at the start of the next line
    // stay"); the line breaks as the same text without the newline does. (Width 11: the
    // newline's `?` advance fits on line 1.)
    let (cs, l) = wrapped("A\n AAA", 11);
    assert_eq!(line_texts(&l, &cs), ["A\n", " A", "A", "A"]);
    assert!(l.lines.iter().all(|ln| ln.consumed.is_none()));
    check_widths(&l, 11);
    let (cs, l) = wrapped(" AAA", 11);
    assert_eq!(line_texts(&l, &cs), [" A", "A", "A"]);

    let (cs, l) = wrapped("AA  AAAAAA", 16);
    assert_eq!(line_texts(&l, &cs), ["AA", " A", "AA", "AA", "A"]);
    assert_eq!(l.lines[1].consumed, Some(2));
    assert_eq!(l.places.iter().filter(|p| p.consumed).count(), 1);
    check_widths(&l, 16);

    let (cs, l) = wrapped("A\n AAAA", 20);
    assert_eq!(line_texts(&l, &cs), ["A\n", " AA", "AA"]);
    check_widths(&l, 20);
}

/// Finding: a forced break right after a wrap that consumed a space gave the empty line the
/// consumed space's height and leading; §7.1 case 4 (the reading recorded in the §7.3
/// OPEN) makes it all zero, and the consumed space counts towards the line holding *c*.
#[test]
fn forced_break_empty_line_is_case_4() {
    let fonts = [Some(face_13_1(true))];
    let cs = chars("?? A");
    let p = LayoutParams {
        width: 5,
        height: 1000,
        leading: 0.5,
        last_line_leading: true,
        ..LayoutParams::default()
    };
    let l = layout(&cs, &fonts, &p, None).unwrap();
    assert_eq!(line_texts(&l, &cs), ["?", "?", "", "A"]);
    // The wrap that consumed the space started the empty line (FN17 order), but the space's
    // metrics go to the line holding `A`.
    let e = l.lines[2];
    assert_eq!((e.start, e.end, e.consumed), (3, 3, Some(2)));
    assert_eq!((e.width, e.height, e.baseline, e.leading), (0, 0, 0, 0));
    assert_eq!(l.lines[3].consumed, None);
    assert_eq!((l.lines[3].height, l.lines[3].baseline), (10, 8));
    assert!(l.places[2].consumed);
    // With the space in a taller font, the line holding `A` takes its height.
    let tall = face(20, 16, vec![g(3, 10, 0, 1, 0, 0x20, 0, 0)], 8, 16);
    let mixed = [Some(face_13_1(true)), Some(tall)];
    let mut b = CharListBuilder::new();
    b.push_str("??").slot(1).push_str(" ").slot(0).push_str("A");
    let mc = b.finish();
    let ml = layout(&mc, &mixed, &p, None).unwrap();
    assert_eq!(line_texts(&ml, &mc), ["?", "?", "", "A"]);
    assert_eq!((ml.lines[2].height, ml.lines[2].leading), (0, 0));
    assert_eq!((ml.lines[3].height, ml.lines[3].baseline), (20, 16));

    // Three lines of 10 + leading 5; the empty line adds nothing.
    assert_eq!(l.text_height, 45);
    assert_eq!(l.lines[3].top, l.lines[2].top);
}

/// Finding: `UICore_Text_set_tag_font_by_property` = 0 alone made the tag slot stop using
/// the normal slot's properties; rule 5 says that value names no member and changes nothing.
#[test]
fn by_property_zero_changes_nothing() {
    const LOCALE: u32 = 0x4200_0028;
    let resolves = |d: u32| d >> 24 == 0x42;
    let mut props = FontProps {
        normal: SlotSource {
            fonts: Some(vec![0x4200_000C]),
            ..SlotSource::default()
        },
        ..FontProps::default()
    };
    let mut plain = FontSlots::new(LOCALE);
    plain.apply(&props, 0, resolves);
    assert_eq!((plain.normal, plain.tag), (0x4200_000C, 0x4200_000C));
    props.tag.by_property = Some(0);
    let mut s = FontSlots::new(LOCALE);
    s.apply(&props, 0, resolves);
    assert_eq!(s, plain);

    // A by-property 0 on the normal slot does not stop the default struct member either.
    let props = FontProps {
        normal: SlotSource {
            font_struct: Some(FontStruct {
                members: vec![(100, StructMember::Did(0x4200_0029))],
            }),
            by_property: Some(0),
            ..SlotSource::default()
        },
        font_member: 100,
        set_font_by_property: Some(0),
        ..FontProps::default()
    };
    let mut s = FontSlots::new(LOCALE);
    s.apply(&props, 0, resolves);
    assert_eq!((s.normal, s.tag), (0x4200_0029, 0x4200_0029));
}

/// Finding: the bullet rule of §7.5 step 3 was applied to the first non-newline character.
/// With wrapping off a text starting with a newline has that newline as the line's first
/// character, so a later list-item character gets no bullet and no pen shift.
#[test]
fn bullet_only_on_a_lines_first_character() {
    let fonts = [Some(face_13_1(true))];
    let mut b = CharListBuilder::new();
    b.list_item(false).push_str("A");
    let cs = b.finish();
    assert!(cs[0].is_newline() && cs[1].list_item);
    let dp = DrawParams {
        bullet: Some([5, 3]),
        ..DrawParams::default()
    };
    let one = LayoutParams {
        width: 1000,
        height: 1000,
        one_line: true,
        list_extra: 5,
        ..LayoutParams::default()
    };
    let l = layout(&cs, &fonts, &one, None).unwrap();
    let p = draw(&l, &cs, &fonts, &dp);
    assert_eq!(kinds(&p), ["g"]);
    let TextOp::Glyphs(r) = &p.ops[0] else {
        unreachable!()
    };
    assert_eq!(
        r.glyphs[0].dest[0], 0,
        "a newline moves the drawing pen by 0"
    );

    // Wrapping on: the `A` starts line 2 and gets the bullet.
    let wrap = LayoutParams {
        one_line: false,
        ..one
    };
    let l = layout(&cs, &fonts, &wrap, None).unwrap();
    let p = draw(&l, &cs, &fonts, &dp);
    assert_eq!(kinds(&p), ["b", "g"]);
}

fn underlined(n: usize) -> Vec<TextChar> {
    let mut b = CharListBuilder::new();
    b.underline(Some(0xFF11_2233));
    b.push_units(&vec![u16::from(b'A'); n]);
    b.finish()
}

/// Finding: underlines (and bullets) were emitted for every character, also outside the
/// clip, and each split the glyph run, so a long underlined text in a small box passed the
/// display-list cap. Now they are culled and a contiguous underline is one fill.
#[test]
fn underlines_are_culled_and_merged() {
    let fonts = [Some(face_13_1(true))];
    let cs = underlined(40_000);
    let p = LayoutParams {
        width: 400,
        height: 100,
        ..LayoutParams::default()
    };
    let l = layout(&cs, &fonts, &p, None).unwrap();
    assert!(l.lines.len() > 500);
    let dp = DrawParams {
        clip: Rect::new(0, 0, 400, 100),
        ..DrawParams::default()
    };
    let d = draw(&l, &cs, &fonts, &dp);
    let unders = d
        .ops
        .iter()
        .filter(|o| matches!(o, TextOp::Underline(_)))
        .count();
    let runs = d
        .ops
        .iter()
        .filter(|o| matches!(o, TextOp::Glyphs(_)))
        .count();
    // Lines are 10 pixels: 10 visible lines (and a partly visible 11th underline row).
    assert!(unders <= 11, "{unders} underline fills");
    assert!(runs <= 11, "{runs} glyph runs");
    let mut list = DisplayList::new();
    d.push_to(&mut list, None).expect("fits the list");

    // One word: one fill before one run, the fill spanning the word.
    let cs = underlined(4);
    let l = layout(&cs, &fonts, &p, None).unwrap();
    let d = draw(&l, &cs, &fonts, &DrawParams::default());
    assert_eq!(kinds(&d), ["u", "g"]);
    let TextOp::Underline(f) = d.ops[0] else {
        unreachable!()
    };
    assert_eq!(f.rect, Rect::new(0, 9, 4 * 7 + 1, 10));
}

/// A glyph that overhangs the next character's underline (B < 0) and reaches the underline
/// rows keeps the per-character order of §8.5: the later underline overwrites it, so the
/// two fills are not merged.
#[test]
fn underlines_are_not_merged_across_an_overhanging_glyph() {
    // `B`: 6 wide, B = −2, rows baseline − 6 to baseline + 3 (descends into the underline).
    let deep = face(
        10,
        8,
        vec![
            g(4, 7, 0, 1, 1, 0x3F, 0, 0),
            g(6, 10, 0, -2, 2, 0x42, 4, 0),
            g(6, 7, 0, -2, 1, 0x43, 10, 0),
        ],
        16,
        16,
    );
    let fonts: [Option<Arc<FontFace>>; 1] = [Some(deep)];
    let mut b = CharListBuilder::new();
    b.underline(Some(0xFF00_00FF)).push_str("BB");
    let cs = b.finish();
    let p = LayoutParams {
        width: 1000,
        height: 100,
        ..LayoutParams::default()
    };
    let l = layout(&cs, &fonts, &p, None).unwrap();
    let d = draw(&l, &cs, &fonts, &DrawParams::default());
    assert_eq!(kinds(&d), ["u", "g", "u", "g"]);

    // `C` ends on the baseline: it does not reach the underline rows, so the fills merge.
    let mut b = CharListBuilder::new();
    b.underline(Some(0xFF00_00FF)).push_str("CC");
    let cs = b.finish();
    let l = layout(&cs, &fonts, &p, None).unwrap();
    let d = draw(&l, &cs, &fonts, &DrawParams::default());
    assert_eq!(kinds(&d), ["u", "g"]);
}

/// Finding: `push_to` failed part-way and left the shared list full of a partial text.
#[test]
fn push_to_is_all_or_nothing() {
    let fonts = [Some(face_13_1(true))];
    let cs = underlined(4);
    let wide = LayoutParams {
        width: 1000,
        height: 100,
        ..LayoutParams::default()
    };
    let l = layout(&cs, &fonts, &wide, None).unwrap();
    let d = draw(&l, &cs, &fonts, &DrawParams::default());
    assert_eq!(d.ops.len(), 2);
    let mut list = DisplayList::new();
    let fill = FillRect {
        rect: Rect::new(0, 0, 1, 1),
        clip: Rect::new(0, 0, 1, 1),
        colour: 0xFF00_0000,
    };
    for _ in 0..MAX_OPS - 1 {
        list.push(PaintOp::Fill(fill)).unwrap();
    }
    assert_eq!(
        d.clone().push_to(&mut list, None),
        Err(PaintError::TooManyOps)
    );
    assert_eq!(list.len(), MAX_OPS - 1, "nothing appended");
    let mut list = DisplayList::new();
    for _ in 0..MAX_OPS - 2 {
        list.push(PaintOp::Fill(fill)).unwrap();
    }
    d.push_to(&mut list, None).expect("exactly fits");
    assert_eq!(list.len(), MAX_OPS);
}

/// Findings: the text cap was 1 Mi units and the character-list builder had none. The
/// builder now stops at the cap, and layout refuses more.
#[test]
fn text_caps() {
    assert_eq!(MAX_TEXT_UNITS, 1 << 17);
    let mut b = CharListBuilder::new();
    b.push_str(&"a".repeat(MAX_TEXT_UNITS + 10));
    assert!(b.overflowed());
    let cs = b.finish();
    assert_eq!(cs.len(), MAX_TEXT_UNITS);
    let fonts = [Some(face_13_1(true))];
    let sfx = chars("?");
    assert_eq!(
        layout(&cs, &fonts, &LayoutParams::default(), Some(&sfx)),
        Err(TextError::TextTooLong(MAX_TEXT_UNITS + 1))
    );
    // At the cap, width 0 (one line per character after the empty first line of §7.3 rule
    // 4), drawing a small clip stays cheap and the result is small.
    let l = layout(&cs, &fonts, &LayoutParams::default(), None).unwrap();
    assert_eq!(l.lines.len(), MAX_TEXT_UNITS + 1);
    let d = draw(
        &l,
        &cs,
        &fonts,
        &DrawParams {
            clip: Rect::new(0, 0, 100, 100),
            ..DrawParams::default()
        },
    );
    assert!(d.ops.len() <= 2);
}
