// SPDX-License-Identifier: GPL-3.0-or-later
//! Exact byte comparisons against the specs' worked examples, through the painter:
//! `ui-layout.md` §14.7 (compositor arithmetic), §9.6 (native size, tiling, clipping),
//! §9.6.3 with decision 0015 §4.2 (overlay source-format tags), `fonts.md` §13.8 (text
//! compositing) and §13.9 (underline as an overwrite), and the canvas split of decision
//! 0015 Decision 1.7. Synthetic data only.

use std::sync::Arc;

use tessera_material::ui_image::{PixelLayout, UiImage, placeholder};
use tessera_ui::paint::{
    CanvasSet, CompositeOp, CoverageMap, DisplayList, FillRect, GlyphBox, GlyphRun, ImageLayer,
    OverlayLayer, PaintOp, Rect, opacity_byte, paint_rect,
};

/// (A, R, G, B) → `0xAARRGGBB`.
const fn argb(a: u32, r: u32, g: u32, b: u32) -> u32 {
    (a << 24) | (r << 16) | (g << 8) | b
}

fn img(w: u32, h: u32, layout: PixelLayout, px: Vec<u32>) -> Arc<UiImage> {
    Arc::new(UiImage::new(w, h, w, h, layout, px).unwrap())
}

fn fill(rect: Rect, colour: u32) -> PaintOp {
    PaintOp::Fill(FillRect {
        rect,
        clip: rect,
        colour,
    })
}

fn image_op(rect: Rect, image: Arc<UiImage>, op: CompositeOp, opacity: u8) -> PaintOp {
    PaintOp::Image(ImageLayer {
        rect,
        clip: rect,
        anchor: [rect.x0, rect.y0],
        image,
        op,
        opacity,
    })
}

/// Paints D, then S with `op` and `o`, on a 1 × 1 canvas; returns the pixel.
fn one(op: CompositeOp, d: u32, s: u32, o: u8) -> u32 {
    let r = Rect::of_size(1, 1);
    let mut l = DisplayList::new();
    l.push(fill(r, d)).unwrap();
    l.push(image_op(
        r,
        img(1, 1, PixelLayout::A8R8G8B8, vec![s]),
        op,
        o,
    ))
    .unwrap();
    let mut set = CanvasSet::new(1, 1).unwrap();
    paint_rect(&l, &mut set, r);
    set.below().pixel(0, 0)
}

#[test]
fn ui_layout_14_7_through_the_painter() {
    assert_eq!(
        one(
            CompositeOp::Over,
            argb(128, 200, 100, 0),
            argb(128, 0, 0, 255),
            255
        ),
        argb(192, 67, 34, 170)
    );
    let d = argb(255, 200, 100, 40);
    let s = argb(255, 128, 64, 255);
    assert_eq!(
        one(CompositeOp::Multiply, d, s, 255),
        argb(255, 100, 25, 40)
    );
    // o = 127 is trunc(0.5 × 255) of UICore_Element_alphablendmod = 0.5.
    assert_eq!(opacity_byte(0.5), 127);
    assert_eq!(
        one(CompositeOp::Multiply, d, s, 127),
        argb(255, 150, 63, 40)
    );
    assert_eq!(
        one(CompositeOp::Screen, d, argb(191, 128, 64, 255), 255),
        argb(255, 221, 129, 201)
    );
    assert_eq!(
        one(
            CompositeOp::Colorize,
            argb(255, 100, 50, 20),
            argb(255, 40, 120, 200),
            255
        ),
        argb(255, 20, 60, 100)
    );
    // Grayscale overlays without an image.
    for (dst, c, want) in [
        (d, argb(0xBF, 0, 0, 0), argb(255, 200, 175, 160)),
        (
            argb(255, 255, 10, 10),
            argb(0xFF, 0, 0, 0),
            argb(255, 0, 0, 0),
        ),
    ] {
        let r = Rect::of_size(1, 1);
        let mut l = DisplayList::new();
        l.push(fill(r, dst)).unwrap();
        l.push(PaintOp::Overlay(OverlayLayer {
            rect: r,
            clip: r,
            colour: c,
            op: CompositeOp::Grayscale,
            image: None,
        }))
        .unwrap();
        let mut set = CanvasSet::new(1, 1).unwrap();
        paint_rect(&l, &mut set, r);
        assert_eq!(set.below().pixel(0, 0), want);
    }
}

#[test]
fn layers_are_native_size_tiled_from_the_element_and_clipped() {
    // A 2 × 1 image on a 7 × 2 element at (3, 1), clipped by an ancestor to x < 8.
    let (a, b) = (argb(255, 1, 1, 1), argb(255, 2, 2, 2));
    let image = img(2, 1, PixelLayout::A8R8G8B8, vec![a, b]);
    let rect = Rect::from_xywh(3, 1, 7, 2);
    let mut l = DisplayList::new();
    l.push(PaintOp::Image(ImageLayer {
        rect,
        clip: Rect::new(0, 0, 8, 10),
        anchor: [3, 1],
        image,
        op: CompositeOp::Replace,
        opacity: 255,
    }))
    .unwrap();
    let mut set = CanvasSet::new(12, 4).unwrap();
    paint_rect(&l, &mut set, Rect::of_size(12, 4));
    let c = set.below();
    for y in 0..4 {
        for x in 0..12 {
            let want = if (1..3).contains(&y) && (3..8).contains(&x) {
                if (x - 3) % 2 == 0 { a } else { b }
            } else {
                0
            };
            assert_eq!(c.pixel(x, y), want, "pixel ({x}, {y})");
        }
    }
    // A run-time image offset moves the anchor; tiling covers the box on both sides.
    let image = img(3, 1, PixelLayout::A8R8G8B8, vec![a, b, argb(255, 3, 3, 3)]);
    let rect = Rect::from_xywh(0, 0, 6, 1);
    let mut l = DisplayList::new();
    l.push(PaintOp::Image(ImageLayer {
        rect,
        clip: rect,
        anchor: [2, 0],
        image,
        op: CompositeOp::Replace,
        opacity: 255,
    }))
    .unwrap();
    let mut set = CanvasSet::new(6, 1).unwrap();
    paint_rect(&l, &mut set, rect);
    let row: Vec<u32> = (0..6).map(|x| set.below().pixel(x, 0) & 0xFF).collect();
    assert_eq!(row, vec![2, 3, 1, 2, 3, 1]);
}

#[test]
fn placeholder_tiles_as_a_grid() {
    // skin-definition.md §6.5: the 32 × 32 placeholder repeats across an element.
    let rect = Rect::from_xywh(0, 0, 70, 40);
    let mut l = DisplayList::new();
    l.push(image_op(
        rect,
        Arc::new(placeholder()),
        CompositeOp::Replace,
        255,
    ))
    .unwrap();
    let mut set = CanvasSet::new(70, 40).unwrap();
    paint_rect(&l, &mut set, rect);
    let c = set.below();
    assert_eq!(c.pixel(0, 0), 0xFF4C_4C66);
    assert_eq!(c.pixel(29, 0), 0xFFFF_FFFF);
    assert_eq!(c.pixel(32, 0), 0xFF4C_4C66);
    assert_eq!(c.pixel(61, 5), 0xFFFF_FFFF);
    assert_eq!(c.pixel(5, 33), 0xFF4C_4C66);
    assert_eq!(c.pixel(5, 30), 0xFFFF_FFFF);
}

#[test]
fn overlay_images_follow_the_source_tag() {
    let r = Rect::of_size(1, 1);
    let dst = argb(255, 10, 20, 30);
    let colour = argb(255, 200, 100, 50);
    let run = |layout: PixelLayout, px: u32| {
        let mut l = DisplayList::new();
        l.push(fill(r, dst)).unwrap();
        l.push(PaintOp::Overlay(OverlayLayer {
            rect: r,
            clip: r,
            colour,
            op: CompositeOp::Screen,
            image: Some((img(1, 1, layout, vec![px]), [0, 0])),
        }))
        .unwrap();
        let mut set = CanvasSet::new(1, 1).unwrap();
        paint_rect(&l, &mut set, r);
        set.below().pixel(0, 0)
    };
    // ARGB32: S = (I.A, C.R, C.G, C.B) with o = C.A.
    let screen =
        tessera_ui::paint::ops::base_pixel(CompositeOp::Screen, dst, argb(128, 200, 100, 50), 255);
    assert_eq!(run(PixelLayout::A8R8G8B8, argb(128, 9, 9, 9)), screen);
    // Alpha-only: black with the 8-bit value as alpha, used as I.A.
    assert_eq!(run(PixelLayout::A8, argb(128, 0, 0, 0)), screen);
    // No alpha and other formats draw nothing.
    assert_eq!(run(PixelLayout::R8G8B8, argb(255, 9, 9, 9)), dst);
    assert_eq!(run(PixelLayout::Dxt5, argb(255, 9, 9, 9)), dst);
    assert_eq!(run(PixelLayout::A8B8G8R8, argb(255, 9, 9, 9)), dst);
    // The base layer uses the pixels whatever the tag.
    let mut l = DisplayList::new();
    l.push(image_op(
        r,
        img(1, 1, PixelLayout::Dxt1, vec![argb(255, 7, 7, 7)]),
        CompositeOp::Replace,
        255,
    ))
    .unwrap();
    let mut set = CanvasSet::new(1, 1).unwrap();
    paint_rect(&l, &mut set, r);
    assert_eq!(set.below().pixel(0, 0), argb(255, 7, 7, 7));
}

#[test]
fn draw_modes_that_draw_nothing() {
    assert_eq!(CompositeOp::from_draw_mode(8), None);
    let d = argb(255, 50, 60, 70);
    assert_eq!(one(CompositeOp::Grayscale, d, argb(255, 1, 2, 3), 255), d);
    assert_eq!(one(CompositeOp::Over, d, argb(255, 1, 2, 3), 0), d);
}

#[test]
fn fonts_13_8_through_a_glyph_run() {
    // Four 1-pixel glyph boxes over four prepared destinations, colour (200, 100, 0).
    let cov = CoverageMap::new(4, 1, vec![128, 128, 0, 255]).unwrap();
    let dests = [
        0,
        argb(255, 0, 0, 255),
        argb(255, 0, 0, 255),
        argb(255, 0, 0, 255),
    ];
    let mut l = DisplayList::new();
    for (x, d) in dests.iter().enumerate() {
        l.push(fill(Rect::from_xywh(x as i32, 0, 1, 1), *d))
            .unwrap();
    }
    l.push(PaintOp::Text(Arc::new(GlyphRun {
        clip: Rect::of_size(4, 1),
        colour: argb(255, 200, 100, 0),
        coverage: Arc::new(cov),
        glyphs: (0..4)
            .map(|x| GlyphBox {
                dest: [x, 0],
                size: [1, 1],
                src: [x, 0],
            })
            .collect(),
    })))
    .unwrap();
    let mut set = CanvasSet::new(4, 1).unwrap();
    paint_rect(&l, &mut set, Rect::of_size(4, 1));
    let got: Vec<u32> = (0..4).map(|x| set.below().pixel(x, 0)).collect();
    assert_eq!(
        got,
        vec![
            argb(128, 200, 100, 0),
            argb(255, 100, 50, 127),
            argb(255, 0, 0, 255),
            argb(255, 200, 100, 0),
        ]
    );
}

#[test]
fn fonts_13_9_underline_overwrites() {
    // Row 9, columns 5 to 12 inclusive, overwritten with the colour's alpha as it is.
    let colour = argb(0x80, 1, 2, 3);
    let mut l = DisplayList::new();
    l.push(fill(Rect::of_size(20, 12), argb(255, 255, 255, 255)))
        .unwrap();
    l.push(fill(Rect::new(5, 9, 13, 10), colour)).unwrap();
    let mut set = CanvasSet::new(20, 12).unwrap();
    paint_rect(&l, &mut set, Rect::of_size(20, 12));
    for x in 0..20 {
        let want = if (5..=12).contains(&x) {
            colour
        } else {
            argb(255, 255, 255, 255)
        };
        assert_eq!(set.below().pixel(x, 9), want);
    }
}

#[test]
fn nested_split_follows_decision_1_7() {
    // Parent draws P0 everywhere, then N1 (covers x 0..4), N2 (x 6..8), N3 (x 2..6), then
    // P1 everywhere. Pixel order at x in 2..4 (covered by N1 and N3): below, N1, A1, N3,
    // A3; P1 lands in A3 there and A1/A2 hold nothing.
    let all = Rect::of_size(10, 1);
    let (p0, p1) = (argb(255, 0, 0, 1), argb(255, 0, 0, 2));
    let mut l = DisplayList::new();
    l.push(fill(all, p0)).unwrap();
    l.push(PaintOp::Nested {
        entry: 1,
        coverage: Rect::new(0, 0, 4, 1),
    })
    .unwrap();
    l.push(PaintOp::Nested {
        entry: 2,
        coverage: Rect::new(6, 0, 8, 1),
    })
    .unwrap();
    l.push(PaintOp::Nested {
        entry: 3,
        coverage: Rect::new(2, 0, 6, 1),
    })
    .unwrap();
    l.push(fill(all, p1)).unwrap();
    let mut set = CanvasSet::new(10, 1).unwrap();
    paint_rect(&l, &mut set, all);
    let px = |layer: Option<u32>, x: i32| match layer {
        None => set.below().pixel(x, 0),
        Some(e) => set.above(e).map_or(0, |c| c.pixel(x, 0)),
    };
    for x in 0..10 {
        let owner = match x {
            2..=5 => Some(3),
            0..=1 => Some(1),
            6..=7 => Some(2),
            _ => None,
        };
        // Below holds P0 everywhere drawn before the markers, and P1 where no marker owns.
        assert_eq!(
            px(None, x),
            if owner.is_none() { p1 } else { p0 },
            "below at {x}"
        );
        for e in 1..=3 {
            let want = if owner == Some(e) { p1 } else { 0 };
            assert_eq!(px(Some(e), x), want, "above {e} at {x}");
        }
    }
    // A repaint after the entries moved away releases nothing it should keep: the stale
    // canvases above are dropped by `retain_nested`.
    let mut l2 = DisplayList::new();
    l2.push(fill(all, p0)).unwrap();
    set.retain_nested(&l2);
    paint_rect(&l2, &mut set, all);
    assert!(set.above(1).is_none() && set.above(3).is_none());
}
