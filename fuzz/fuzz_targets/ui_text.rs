// SPDX-License-Identifier: GPL-3.0-or-later
//! `tessera-ui::text` (`fonts.md` §4–§8): a hostile font record (any metrics, codes, atlas
//! positions, borders and baseline) with hostile atlas surfaces goes through
//! `FontFace::from_payloads`; when it builds, arbitrary UTF-16 text with arbitrary slots,
//! colours, underlines and list items is laid out with arbitrary element parameters (sizes,
//! margins, justification values, leading factors including NaN and infinities, list
//! extra, wrapping on or off, an optional truncation suffix), drawn with the outline on or
//! off at an arbitrary origin and clip, pushed to a display list and painted into a small
//! canvas. Nothing may panic; layout invariants FN16 and FN17 are checked.
//!
//! Input layout: a header of parameter bytes, then the bytes every field reads its value
//! from (cycled when short).

#![no_main]

use libfuzzer_sys::fuzz_target;
use tessera_ui::paint::{CanvasSet, DisplayList, Rect, paint_rect};
use tessera_ui::text::{
    CharListBuilder, DrawParams, FontFace, LayoutParams, Margins, draw, layout,
};

struct Bytes<'a> {
    data: &'a [u8],
    pos: usize,
}

impl Bytes<'_> {
    fn byte(&mut self) -> u8 {
        if self.data.is_empty() {
            return 0;
        }
        let b = self.data[self.pos % self.data.len()];
        self.pos += 1;
        b
    }
    fn u16(&mut self) -> u16 {
        u16::from_le_bytes([self.byte(), self.byte()])
    }
    fn u32(&mut self) -> u32 {
        u32::from_le_bytes([self.byte(), self.byte(), self.byte(), self.byte()])
    }
    /// A small value most of the time, any value sometimes.
    fn small(&mut self, max: u32) -> u32 {
        let v = self.u32();
        if self.byte() < 16 { v } else { v % (max + 1) }
    }
    fn coord(&mut self) -> i32 {
        let v = self.u32() as i32;
        if self.byte() < 16 { v } else { v % 200 }
    }
}

fn surface(did: u32, w: u32, h: u32, fill: &mut Bytes<'_>, honest: bool) -> Vec<u8> {
    let len = (w * h) as usize;
    let mut out = Vec::with_capacity(24 + len);
    let stated = if honest {
        len as u32
    } else {
        fill.u32() % 8192
    };
    for v in [did, 13, w, h, 28, stated] {
        out.extend_from_slice(&v.to_le_bytes());
    }
    for _ in 0..stated.min(len as u32 + 64) {
        out.push(fill.byte());
    }
    out
}

fuzz_target!(|data: &[u8]| {
    let mut b = Bytes { data, pos: 0 };
    // Font record (§3.2) built from the bytes.
    let n = usize::from(b.byte() % 48);
    let mut rec = Vec::with_capacity(36 + 11 * n);
    rec.extend_from_slice(&0x4200_0001u32.to_le_bytes());
    let lh = b.small(40);
    rec.extend_from_slice(&lh.to_le_bytes());
    rec.extend_from_slice(&b.small(40).to_le_bytes());
    rec.extend_from_slice(&(n as u32).to_le_bytes());
    for i in 0..n {
        let (w, h) = (b.byte() % 24, b.byte() % 24);
        let (a, bb, t) = (
            b.byte() as i8 % 12,
            b.byte() as i8 % 12,
            b.byte() as i8 % 24,
        );
        let code = if b.byte() < 200 {
            0x20 + (i as u16) * 3
        } else {
            b.u16()
        };
        let (x, y) = (b.u16() % 80, b.u16() % 80);
        rec.extend_from_slice(&[w, h, a as u8, bb as u8, t as u8]);
        rec.extend_from_slice(&code.to_le_bytes());
        rec.extend_from_slice(&x.to_le_bytes());
        rec.extend_from_slice(&y.to_le_bytes());
    }
    let border = b.small(6);
    rec.extend_from_slice(&border.to_le_bytes());
    rec.extend_from_slice(&border.to_le_bytes());
    rec.extend_from_slice(&b.small(40).to_le_bytes());
    let has_bg = b.byte() & 1 == 0;
    rec.extend_from_slice(&0x4100_0001u32.to_le_bytes());
    rec.extend_from_slice(&(if has_bg { 0x4100_0002u32 } else { 0 }).to_le_bytes());
    if b.byte() < 8 {
        // Truncated or padded records must be refused, not panic.
        let cut = usize::from(b.byte()) % (rec.len() + 1);
        rec.truncate(cut);
    }
    let (aw, ah) = (1 + u32::from(b.byte() % 96), 1 + u32::from(b.byte() % 96));
    let honest = b.byte() >= 8;
    let fg = surface(0x4100_0001, aw, ah, &mut b, honest);
    let bg = has_bg.then(|| surface(0x4100_0002, aw, ah, &mut b, true));
    let Ok(face) = FontFace::from_payloads(&rec, &fg, bg.as_deref()) else {
        return;
    };
    let face = std::sync::Arc::new(face);
    let fonts = [Some(face), None];

    // Text.
    let mut cl = CharListBuilder::new();
    let units = usize::from(b.byte()) + usize::from(b.byte() % 4) * 256;
    for _ in 0..units {
        match b.byte() {
            0..=9 => {
                cl.list_item(b.byte() & 1 == 0);
            }
            10..=19 => {
                cl.push_break(b.byte() & 1 == 0);
            }
            20..=29 => {
                cl.slot(b.byte() % 3).colour(b.u32());
            }
            30..=39 => {
                let u = b.u32();
                cl.underline((u & 1 == 0).then_some(u));
            }
            40..=120 => {
                cl.push_units(&[0x20 + u16::from(b.byte() % 64)]);
            }
            121..=140 => {
                cl.push_units(&[[0x20, 0x0A, 0x09, 0x0D, 0x2D, 0x28][usize::from(b.byte() % 6)]]);
            }
            _ => {
                cl.push_units(&[b.u16()]);
            }
        }
    }
    let cs = cl.finish();
    let suffix: Vec<_> = {
        let mut s = CharListBuilder::new();
        for _ in 0..b.byte() % 4 {
            s.push_units(&[0x3F]);
        }
        s.finish()
    };
    let leading = match b.byte() {
        0 => f32::NAN,
        1 => f32::INFINITY,
        2 => f32::NEG_INFINITY,
        3 => f32::MAX,
        v => f32::from(v) / 128.0 - 0.2,
    };
    let p = LayoutParams {
        width: b.coord(),
        height: b.coord(),
        margins: Margins {
            left: b.coord() % 8,
            right: b.coord() % 8,
            top: b.coord() % 8,
            bottom: b.coord() % 8,
        },
        one_line: b.byte() & 1 == 0,
        horizontal: b.small(6),
        vertical: b.small(6),
        leading,
        last_line_leading: b.byte() & 1 == 0,
        list_extra: b.coord(),
    };
    let sfx = (!suffix.is_empty()).then_some(&suffix[..]);
    let Ok(l) = layout(&cs, &fonts, &p, sfx) else {
        return;
    };
    // FN17: lines plus consumed characters give back the text, in order.
    let mut next = 0usize;
    for line in &l.lines {
        if let Some(c) = line.consumed {
            assert_eq!(c, next);
            next += 1;
        }
        assert_eq!(line.start, next);
        next = line.end;
    }
    assert_eq!(next, cs.len());
    // FN16 (saturated values stay ordered).
    assert!(l.text_height >= 0 || l.lines.iter().any(|x| x.height < 0));

    let clip = Rect::new(b.coord(), b.coord(), b.coord(), b.coord());
    let d = draw(
        &l,
        &cs,
        &fonts,
        &DrawParams {
            origin: [b.coord(), b.coord()],
            clip,
            outline: b.byte() & 1 == 0,
            outline_colour: b.u32(),
            bullet: (b.byte() & 1 == 0).then(|| [b.small(8), b.small(8)]),
        },
    );
    let mut list = DisplayList::new();
    if d.push_to(&mut list, None).is_ok() {
        let mut set = CanvasSet::new(64, 64).unwrap();
        paint_rect(&list, &mut set, Rect::new(0, 0, 64, 64));
    }
});
