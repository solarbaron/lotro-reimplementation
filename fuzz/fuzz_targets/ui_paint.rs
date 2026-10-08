// SPDX-License-Identifier: GPL-3.0-or-later
//! `tessera-ui::paint` painter (decision 0015 Decision 1.3, 1.7, §7; `ui-layout.md` §9.6,
//! `fonts.md` §8.4): arbitrary display lists (image and overlay layers with every
//! source-format tag and every operation, fills, glyph runs with hostile boxes and atlas
//! offsets, nested-entry markers), canvas sizes across the tile edge, extreme coordinates
//! and damage rectangles. Nothing may panic; nothing is written outside the canvas; a
//! partitioned repaint (grid cells, plus overlapping rectangles) gives the bytes of one full
//! repaint in the canvas below and every canvas above; a banded job gives the same bytes.
//!
//! Input layout: two size bytes, then the bytes every field reads its value from (cycled
//! when short).

#![no_main]

use std::sync::Arc;

use libfuzzer_sys::fuzz_target;
use tessera_material::ui_image::{PixelLayout, UiImage};
use tessera_ui::paint::{
    BandBudget, CanvasSet, CompositeOp, CoverageMap, DisplayList, FillRect, GlyphBox, GlyphRun,
    ImageLayer, OverlayLayer, PaintJob, PaintOp, Rect, paint_rect,
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
    fn u32(&mut self) -> u32 {
        u32::from_le_bytes([self.byte(), self.byte(), self.byte(), self.byte()])
    }
    /// Near the canvas most of the time, raw bits sometimes.
    fn coord(&mut self) -> i32 {
        match self.byte() % 8 {
            0 => self.u32() as i32,
            1 => i32::MIN,
            2 => i32::MAX,
            _ => i32::from(self.byte()) * 2 - 100,
        }
    }
    fn rect(&mut self) -> Rect {
        Rect::new(self.coord(), self.coord(), self.coord(), self.coord())
    }
    fn image(&mut self) -> Arc<UiImage> {
        const LAYOUTS: [PixelLayout; 6] = [
            PixelLayout::A8R8G8B8,
            PixelLayout::A8,
            PixelLayout::R8G8B8,
            PixelLayout::Dxt1,
            PixelLayout::A8B8G8R8,
            PixelLayout::Jpeg,
        ];
        let w = u32::from(self.byte() % 9) + 1;
        let h = u32::from(self.byte() % 9) + 1;
        let layout = LAYOUTS[usize::from(self.byte()) % LAYOUTS.len()];
        let px = (0..w * h).map(|_| self.u32()).collect();
        Arc::new(UiImage::new(w, h, w, h, layout, px).expect("valid synthetic image"))
    }
    fn op(&mut self) -> CompositeOp {
        CompositeOp::from_draw_mode(u32::from(self.byte() % 10)).unwrap_or(CompositeOp::Over)
    }
}

const ENTRIES: u32 = 4;

fn layers(set: &CanvasSet) -> Vec<Vec<u32>> {
    let [w, h] = set.size();
    let zeros = vec![0u32; (w * h) as usize];
    let mut v = vec![set.below().to_vec()];
    for e in 0..ENTRIES {
        v.push(set.above(e).map_or_else(|| zeros.clone(), |c| c.to_vec()));
    }
    v
}

fuzz_target!(|data: &[u8]| {
    let mut b = Bytes { data, pos: 0 };
    let w = u32::from(b.byte()) + u32::from(b.byte() % 2) * 200 + 1;
    // Heights cross the 256-row tile edge too.
    let h = u32::from(b.byte() % 64) + u32::from(b.byte() % 2) * 240 + 1;
    let mut list = DisplayList::new();
    for _ in 0..usize::from(b.byte() % 16) {
        let op = match b.byte() % 5 {
            0 => {
                let rect = b.rect();
                PaintOp::Image(ImageLayer {
                    rect,
                    clip: b.rect(),
                    anchor: [b.coord(), b.coord()],
                    image: b.image(),
                    op: b.op(),
                    opacity: b.byte(),
                })
            }
            1 => {
                let rect = b.rect();
                let image = if b.byte().is_multiple_of(2) {
                    Some((b.image(), [b.coord(), b.coord()]))
                } else {
                    None
                };
                PaintOp::Overlay(OverlayLayer {
                    rect,
                    clip: b.rect(),
                    colour: b.u32(),
                    op: b.op(),
                    image,
                })
            }
            2 => PaintOp::Fill(FillRect {
                rect: b.rect(),
                clip: b.rect(),
                colour: b.u32(),
            }),
            3 => {
                let cw = u32::from(b.byte() % 8) + 1;
                let ch = u32::from(b.byte() % 8) + 1;
                let cov = (0..cw * ch).map(|_| b.byte()).collect();
                let Ok(map) = CoverageMap::new(cw, ch, cov) else {
                    return;
                };
                let glyphs = (0..usize::from(b.byte() % 6))
                    .map(|_| GlyphBox {
                        dest: [b.coord(), b.coord()],
                        size: [u32::from(b.byte() % 20), u32::from(b.byte() % 20)],
                        src: [b.coord(), b.coord()],
                    })
                    .collect();
                PaintOp::Text(Arc::new(GlyphRun {
                    clip: b.rect(),
                    colour: b.u32(),
                    coverage: Arc::new(map),
                    glyphs,
                }))
            }
            _ => PaintOp::Nested {
                entry: u32::from(b.byte()) % ENTRIES,
                coverage: b.rect(),
            },
        };
        if list.push(op).is_err() {
            return;
        }
    }
    let full = Rect::of_size(w, h);
    let mut whole = CanvasSet::new(w, h).expect("valid size");
    paint_rect(&list, &mut whole, full);
    let want = layers(&whole);

    // Grid partition.
    let mut xs = vec![0, w as i32];
    let mut ys = vec![0, h as i32];
    for _ in 0..usize::from(b.byte() % 4) {
        xs.push(i32::from(b.byte()) % w as i32);
        ys.push(i32::from(b.byte()) * 2 % h as i32);
    }
    xs.sort_unstable();
    xs.dedup();
    ys.sort_unstable();
    ys.dedup();
    let mut cells = Vec::new();
    for yy in ys.windows(2) {
        for xx in xs.windows(2) {
            cells.push(Rect::new(xx[0], yy[0], xx[1], yy[1]));
        }
    }
    let mut split = CanvasSet::new(w, h).expect("valid size");
    for c in cells.iter().rev() {
        paint_rect(&list, &mut split, *c);
    }
    assert_eq!(layers(&split), want, "partitioned repaint differs");
    // Arbitrary (also hostile) damage over the painted canvas is idempotent.
    for _ in 0..usize::from(b.byte() % 4) {
        paint_rect(&list, &mut split, b.rect());
    }
    assert_eq!(layers(&split), want, "repaint is not idempotent");

    // Banded job, one band per frame.
    let mut banded = CanvasSet::new(w, h).expect("valid size");
    let mut job = PaintJob::new(Arc::new(list), [w, h], cells);
    while !job.is_complete() {
        job.run(&mut banded, &mut BandBudget(0));
    }
    assert_eq!(layers(&banded), want, "banded job differs");
    // Nothing outside the canvas.
    assert_eq!(whole.below().pixel(w as i32, 0), 0);
    assert_eq!(whole.below().pixel(-1, -1), 0);
});
