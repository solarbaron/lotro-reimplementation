// SPDX-License-Identifier: GPL-3.0-or-later
//! Partition invariance (decision 0015 Decision 1.3, a required test property): painting a
//! region by any split of the damage gives the same bytes as one paint of the whole region,
//! in the canvas below and in every canvas above, whatever the tile layout. Also: a job
//! painted in bands over frames equals one paint. Synthetic display lists only.

use std::sync::Arc;

use proptest::prelude::*;
use tessera_material::ui_image::{PixelLayout, UiImage};
use tessera_ui::paint::{
    BandBudget, CanvasSet, CompositeOp, CoverageMap, DisplayList, FillRect, GlyphBox, GlyphRun,
    ImageLayer, OverlayLayer, PaintJob, PaintOp, Rect, paint_rect,
};

const OPS: [CompositeOp; 7] = [
    CompositeOp::Replace,
    CompositeOp::MixColour,
    CompositeOp::Over,
    CompositeOp::Multiply,
    CompositeOp::Screen,
    CompositeOp::Colorize,
    CompositeOp::Grayscale,
];

const LAYOUTS: [PixelLayout; 4] = [
    PixelLayout::A8R8G8B8,
    PixelLayout::A8,
    PixelLayout::R8G8B8,
    PixelLayout::Dxt1,
];

fn rect_in(w: i32, h: i32) -> impl Strategy<Value = Rect> {
    (-20..w + 20, -20..h + 20, 0..w + 40, 0..h + 40)
        .prop_map(|(x, y, rw, rh)| Rect::new(x, y, x + rw, y + rh))
}

fn image() -> impl Strategy<Value = Arc<UiImage>> {
    (1u32..9, 1u32..9, 0usize..4, any::<u64>()).prop_map(|(w, h, l, seed)| {
        let mut x = seed | 1;
        let px = (0..w * h)
            .map(|_| {
                x ^= x << 13;
                x ^= x >> 7;
                x ^= x << 17;
                // Bias alpha towards 0 and 255 to reach every branch.
                let v = x as u32;
                match (x >> 40) % 4 {
                    0 => v & 0x00FF_FFFF,
                    1 => v | 0xFF00_0000,
                    _ => v,
                }
            })
            .collect();
        Arc::new(UiImage::new(w, h, w, h, LAYOUTS[l], px).expect("valid image"))
    })
}

fn colour() -> impl Strategy<Value = u32> {
    prop_oneof![
        any::<u32>(),
        any::<u32>().prop_map(|c| c | 0xFF00_0000),
        any::<u32>().prop_map(|c| c & 0xFF_FFFF)
    ]
}

fn op(w: i32, h: i32) -> impl Strategy<Value = PaintOp> {
    prop_oneof![
        (
            rect_in(w, h),
            rect_in(w, h),
            -10..30i32,
            -10..30i32,
            image(),
            0usize..7,
            any::<u8>()
        )
            .prop_map(
                |(rect, clip, ax, ay, image, k, o)| PaintOp::Image(ImageLayer {
                    anchor: [rect.x0 + ax, rect.y0 + ay],
                    rect,
                    clip,
                    image,
                    op: OPS[k],
                    opacity: if o > 200 { 255 } else { o },
                })
            ),
        (
            rect_in(w, h),
            rect_in(w, h),
            colour(),
            0usize..7,
            proptest::option::of((image(), -10..30i32, -10..30i32))
        )
            .prop_map(
                |(rect, clip, colour, k, img)| PaintOp::Overlay(OverlayLayer {
                    rect,
                    clip,
                    colour,
                    op: OPS[k],
                    image: img.map(|(i, ax, ay)| (i, [rect.x0 + ax, rect.y0 + ay])),
                })
            ),
        (rect_in(w, h), rect_in(w, h), colour())
            .prop_map(|(rect, clip, colour)| PaintOp::Fill(FillRect { rect, clip, colour })),
        (
            rect_in(w, h),
            colour(),
            proptest::collection::vec(any::<u8>(), 64),
            proptest::collection::vec((-5..w, -5..h, 1u32..12, 1u32..12, -3..8i32, -3..8i32), 1..5)
        )
            .prop_map(|(clip, colour, cov, gl)| {
                let map = Arc::new(CoverageMap::new(8, 8, cov).expect("map"));
                PaintOp::Text(Arc::new(GlyphRun {
                    clip,
                    colour,
                    coverage: map,
                    glyphs: gl
                        .into_iter()
                        .map(|(x, y, gw, gh, sx, sy)| GlyphBox {
                            dest: [x, y],
                            size: [gw, gh],
                            src: [sx, sy],
                        })
                        .collect(),
                }))
            }),
        (0u32..4, rect_in(w, h)).prop_map(|(entry, coverage)| PaintOp::Nested { entry, coverage }),
    ]
}

fn list(w: i32, h: i32) -> impl Strategy<Value = DisplayList> {
    proptest::collection::vec(op(w, h), 0..14).prop_map(|ops| {
        let mut l = DisplayList::new();
        for o in ops {
            l.push(o).expect("within caps");
        }
        l
    })
}

/// Every layer's pixels, with absent canvases above as zeros, for entries 0..4.
fn snapshot(set: &CanvasSet) -> Vec<Vec<u32>> {
    let [w, h] = set.size();
    let zeros = vec![0u32; (w * h) as usize];
    let mut v = vec![set.below().to_vec()];
    for e in 0..4 {
        v.push(set.above(e).map_or_else(|| zeros.clone(), |c| c.to_vec()));
    }
    v
}

fn scenario() -> impl Strategy<Value = (u32, u32, DisplayList, Vec<i32>, Vec<i32>, Vec<Rect>)> {
    (1u32..300, 1u32..300).prop_flat_map(|(w, h)| {
        (
            Just(w),
            Just(h),
            list(w as i32, h as i32),
            proptest::collection::vec(0..w as i32, 0..5),
            proptest::collection::vec(0..h as i32, 0..5),
            proptest::collection::vec(rect_in(w as i32, h as i32), 0..6),
        )
    })
}

proptest! {
    #![proptest_config(ProptestConfig { cases: 160, ..ProptestConfig::default() })]

    /// Grid partitions, overlapping rectangle sequences and banded jobs all give the bytes
    /// of one full paint.
    #[test]
    fn any_split_gives_the_same_bytes((w, h, list, mut xs, mut ys, extra) in scenario()) {
        let full = Rect::of_size(w, h);
        let mut whole = CanvasSet::new(w, h).unwrap();
        paint_rect(&list, &mut whole, full);
        let want = snapshot(&whole);

        // A grid partition, cells painted in reverse order.
        xs.extend([0, w as i32]);
        ys.extend([0, h as i32]);
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
        let mut split = CanvasSet::new(w, h).unwrap();
        for c in cells.iter().rev() {
            paint_rect(&list, &mut split, *c);
        }
        prop_assert_eq!(&snapshot(&split), &want);

        // Overlapping repaints over an already painted canvas: arbitrary rectangles, then
        // the grid again; repainting is idempotent.
        for r in &extra {
            paint_rect(&list, &mut split, *r);
        }
        prop_assert_eq!(&snapshot(&split), &want);

        // A job in bands, one band per frame.
        let mut banded = CanvasSet::new(w, h).unwrap();
        let mut job = PaintJob::new(Arc::new(list.clone()), [w, h], cells);
        let mut frames = 0;
        while !job.is_complete() {
            job.run(&mut banded, &mut BandBudget(0));
            frames += 1;
            prop_assert!(frames < 10_000);
        }
        prop_assert_eq!(&snapshot(&banded), &want);
    }
}

/// A list of markers (entries 0..6) and full-canvas fills, each fill with its own colour.
fn marker_scenario() -> impl Strategy<Value = (u32, u32, Vec<(u32, Rect)>)> {
    (1u32..300, 1u32..70).prop_flat_map(|(w, h)| {
        let (wi, hi) = (w as i32, h as i32);
        (
            Just(w),
            Just(h),
            prop::collection::vec((0u32..6, rect_in(wi, hi)), 0..40),
        )
    })
}

/// The canvases by a per-pixel reference: a pixel drawn after markers goes to the canvas
/// above of the last marker before it containing the pixel, else to the canvas below.
fn reference(w: u32, h: u32, markers: &[(u32, Rect)]) -> Vec<Vec<u32>> {
    let mut layers = vec![vec![0u32; (w * h) as usize]; 7];
    for y in 0..h as i32 {
        for x in 0..w as i32 {
            for k in 0..=markers.len() {
                let owner = markers[..k]
                    .iter()
                    .rev()
                    .find(|(_, r)| r.contains(x, y))
                    .map_or(0, |(e, _)| *e as usize + 1);
                layers[owner][(y as u32 * w + x as u32) as usize] = 0xFF00_0000 | k as u32;
            }
        }
    }
    layers
}

proptest! {
    #![proptest_config(ProptestConfig::with_cases(128))]

    /// The owner partition matches a per-pixel scan of the markers.
    #[test]
    fn owners_match_a_per_pixel_reference((w, h, markers) in marker_scenario()) {
        let mut list = DisplayList::new();
        list.push(PaintOp::Fill(FillRect {
            rect: Rect::of_size(w, h),
            clip: Rect::of_size(w, h),
            colour: 0xFF00_0000,
        }))
        .unwrap();
        for (k, (e, r)) in markers.iter().enumerate() {
            list.push(PaintOp::Nested { entry: *e, coverage: *r }).unwrap();
            list.push(PaintOp::Fill(FillRect {
                rect: Rect::of_size(w, h),
                clip: Rect::of_size(w, h),
                colour: 0xFF00_0000 | (k as u32 + 1),
            }))
            .unwrap();
        }
        let mut set = CanvasSet::new(w, h).unwrap();
        paint_rect(&list, &mut set, Rect::of_size(w, h));
        let want = reference(w, h, &markers);
        prop_assert_eq!(set.below().to_vec(), want[0].clone());
        for e in 0..6u32 {
            let got = set.above(e).map_or_else(|| vec![0; (w * h) as usize], |c| c.to_vec());
            prop_assert_eq!(got, want[e as usize + 1].clone(), "entry {}", e);
        }
    }
}

#[test]
fn many_markers_on_one_band() {
    // 512 one-pixel-wide markers, alternating with fills: every column has its own owner.
    let (w, h) = (1024u32, 16u32);
    let mut list = DisplayList::new();
    for e in 0..512u32 {
        list.push(PaintOp::Nested {
            entry: e,
            coverage: Rect::new(2 * e as i32, 0, 2 * e as i32 + 1, h as i32),
        })
        .unwrap();
    }
    list.push(PaintOp::Fill(FillRect {
        rect: Rect::of_size(w, h),
        clip: Rect::of_size(w, h),
        colour: 0xFF12_3456,
    }))
    .unwrap();
    let mut set = CanvasSet::new(w, h).unwrap();
    paint_rect(&list, &mut set, Rect::of_size(w, h));
    for e in 0..512u32 {
        let c = set.above(e).unwrap();
        assert_eq!(c.pixel(2 * e as i32, 3), 0xFF12_3456);
        assert_eq!(c.pixel(2 * e as i32 + 1, 3), 0);
        assert_eq!(set.below().pixel(2 * e as i32 + 1, 3), 0xFF12_3456);
        assert_eq!(set.below().pixel(2 * e as i32, 3), 0);
    }
}

#[test]
fn canvas_set_byte_limit_bounds_canvases_above() {
    // Finding: each marker made a full-size canvas above without any memory bound.
    let (w, h) = (512u32, 512u32);
    let mut list = DisplayList::new();
    for e in 0..40u32 {
        list.push(PaintOp::Nested {
            entry: e,
            coverage: Rect::of_size(w, h),
        })
        .unwrap();
        list.push(PaintOp::Fill(FillRect {
            rect: Rect::of_size(w, h),
            clip: Rect::of_size(w, h),
            colour: 0xFF00_0000 | e,
        }))
        .unwrap();
    }
    let canvas = (w * h * 4) as usize;
    let mut set = CanvasSet::new(w, h).unwrap();
    set.set_byte_limit(4 * canvas);
    paint_rect(&list, &mut set, Rect::of_size(w, h));
    assert!(set.allocated_bytes() <= 4 * canvas);
    assert!(set.take_refused_tiles() > 0);
    assert_eq!(set.take_refused_tiles(), 0);
    // Without a limit every canvas above is full (41 canvases).
    let mut free = CanvasSet::new(w, h).unwrap();
    paint_rect(&list, &mut free, Rect::of_size(w, h));
    assert_eq!(free.allocated_bytes(), 40 * canvas);
}

#[test]
fn band_budget_counts_every_band() {
    // Finding: BandBudget(n) painted n + 1 bands.
    let list = Arc::new(DisplayList::new());
    for (n, want) in [(0u32, 1usize), (1, 1), (2, 2), (5, 5)] {
        let mut set = CanvasSet::new(16, 160).unwrap();
        let mut job = PaintJob::new(Arc::clone(&list), [16, 160], vec![Rect::of_size(16, 160)]);
        assert_eq!(
            job.run(&mut set, &mut BandBudget(n)).len(),
            want,
            "BandBudget({n})"
        );
    }
}
