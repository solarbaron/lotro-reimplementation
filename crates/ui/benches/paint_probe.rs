// SPDX-License-Identifier: GPL-3.0-or-later
//! Painter micro-benchmark: the U1 probe gap of decision 0015 §6 ("Probe gap before U1
//! becomes a criterion": Mix colour and the erase, including the freeing of whole tiles),
//! next to the other compositor operations, text compositing, and the walking-phase minimap
//! repaint of the §6 table (erase + disc Replace + map Mix colour + rim Over over 170 × 170).
//!
//! Run with `cargo bench -p tessera-ui --bench paint_probe` (a release build; add
//! `-- --iters N` to change the repeat count). Prints ns per pixel and the minimap total.
//! Synthetic images only. Results are **indicative** unless taken on the reference machine
//! under rule 1 of `docs/perf/reference-machine.md` (decision 0010); record the conditions
//! next to every number.

use std::hint::black_box;
use std::sync::Arc;
use std::time::{Duration, Instant};

use tessera_material::ui_image::{PixelLayout, UiImage};
use tessera_ui::paint::{
    CanvasSet, CompositeOp, CoverageMap, DisplayList, GlyphBox, GlyphRun, ImageLayer, PaintOp,
    Rect, paint_rect,
};

/// A deterministic image with varied alpha and colour.
fn image(w: u32, h: u32, seed: u32, layout: PixelLayout) -> Arc<UiImage> {
    let mut x = seed.wrapping_mul(2_654_435_761).max(1);
    let px = (0..w * h)
        .map(|_| {
            x ^= x << 13;
            x ^= x >> 17;
            x ^= x << 5;
            x
        })
        .collect();
    Arc::new(UiImage::new(w, h, w, h, layout, px).expect("valid synthetic image"))
}

fn opaque_image(w: u32, h: u32) -> Arc<UiImage> {
    let px = (0..w * h)
        .map(|i| 0xFF00_0000 | (i.wrapping_mul(97) & 0xFF_FFFF))
        .collect();
    Arc::new(UiImage::new(w, h, w, h, PixelLayout::A8R8G8B8, px).expect("valid"))
}

/// A square image from a pixel function.
fn shaped(side: u32, f: impl Fn(u32, u32) -> u32) -> Arc<UiImage> {
    let px = (0..side * side).map(|i| f(i % side, i / side)).collect();
    Arc::new(UiImage::new(side, side, side, side, PixelLayout::A8R8G8B8, px).expect("valid"))
}

fn layer(rect: Rect, image: Arc<UiImage>, op: CompositeOp, opacity: u8) -> PaintOp {
    PaintOp::Image(ImageLayer {
        rect,
        clip: rect,
        anchor: [rect.x0, rect.y0],
        image,
        op,
        opacity,
    })
}

/// Median time of `iters` repaints of `rect`.
fn time_paint(list: &DisplayList, set: &mut CanvasSet, rect: Rect, iters: u32) -> Duration {
    let mut v: Vec<Duration> = (0..iters)
        .map(|_| {
            let t = Instant::now();
            paint_rect(black_box(list), set, rect);
            t.elapsed()
        })
        .collect();
    v.sort_unstable();
    v[v.len() / 2]
}

fn ns_per_px(d: Duration, px: u64) -> f64 {
    d.as_nanos() as f64 / px as f64
}

fn main() {
    let mut iters = 41u32;
    let args: Vec<String> = std::env::args().collect();
    if let Some(i) = args.iter().position(|a| a == "--iters")
        && let Some(n) = args.get(i + 1).and_then(|v| v.parse().ok())
    {
        iters = n;
    }
    let (w, h) = (1024u32, 1024u32);
    let full = Rect::of_size(w, h);
    let px = full.area();
    let base = opaque_image(64, 64);
    let src = image(64, 64, 7, PixelLayout::A8R8G8B8);

    println!("tessera-ui painter probe ({w} x {h} canvas, median of {iters} repaints)");
    println!("{:<34} {:>10}", "operation (incl. erase)", "ns/pixel");

    // Erase alone: a list with nothing in it over an allocated canvas. Repainting an
    // allocated canvas measures the free of whole tiles plus the next allocation.
    let mut set = CanvasSet::new(w, h).expect("size");
    let mut fill = DisplayList::new();
    fill.push(layer(full, Arc::clone(&base), CompositeOp::Replace, 255))
        .expect("push");
    let empty = DisplayList::new();
    let erase = {
        let mut v: Vec<Duration> = (0..iters)
            .map(|_| {
                paint_rect(&fill, &mut set, full);
                let t = Instant::now();
                paint_rect(black_box(&empty), &mut set, full);
                t.elapsed()
            })
            .collect();
        v.sort_unstable();
        v[v.len() / 2]
    };
    println!(
        "{:<34} {:>10.3}",
        "erase, whole tiles freed",
        ns_per_px(erase, px)
    );
    // Partial erase: rows that do not cover whole tiles (zeroing in place).
    let partial = Rect::new(1, 0, w as i32 - 1, h as i32);
    let pe = {
        let mut v: Vec<Duration> = (0..iters)
            .map(|_| {
                paint_rect(&fill, &mut set, full);
                let t = Instant::now();
                paint_rect(black_box(&empty), &mut set, partial);
                t.elapsed()
            })
            .collect();
        v.sort_unstable();
        v[v.len() / 2]
    };
    println!(
        "{:<34} {:>10.3}",
        "erase, tiles zeroed in place",
        ns_per_px(pe, partial.area())
    );

    // A full repaint of an allocated canvas with one opaque Replace layer: the erase frees
    // every tile, and the layer allocates them again (the cost the "net" rows subtract).
    let base_full = time_paint(&fill, &mut set, full, iters);
    println!(
        "{:<34} {:>10.3}",
        "erase + re-allocation + Replace",
        ns_per_px(base_full, px)
    );

    let ops = [
        ("Replace", CompositeOp::Replace),
        ("Mix colour", CompositeOp::MixColour),
        ("Over", CompositeOp::Over),
        ("Multiply", CompositeOp::Multiply),
        ("Screen", CompositeOp::Screen),
        ("Colorize", CompositeOp::Colorize),
    ];
    for (name, op) in ops {
        for (label, o) in [("", 255u8), (" (o = 127)", 127u8)] {
            let mut list = DisplayList::new();
            list.push(layer(full, Arc::clone(&base), CompositeOp::Replace, 255))
                .expect("push");
            list.push(layer(full, Arc::clone(&src), op, o))
                .expect("push");
            let both = time_paint(&list, &mut set, full, iters);
            let net = both.saturating_sub(time_paint(&fill, &mut set, full, iters));
            println!(
                "{:<34} {:>10.3}",
                format!("{name}{label}, net of base"),
                ns_per_px(net, px)
            );
        }
    }

    // Text: a run of 16 × 16 glyph boxes covering the canvas, opaque colour.
    let cov: Vec<u8> = (0..256u32 * 256)
        .map(|i| (i.wrapping_mul(31) & 0xFF) as u8)
        .collect();
    let map = Arc::new(CoverageMap::new(256, 256, cov).expect("map"));
    let glyphs = (0..h as i32 / 16)
        .flat_map(|gy| {
            (0..w as i32 / 16).map(move |gx| GlyphBox {
                dest: [gx * 16, gy * 16],
                size: [16, 16],
                src: [(gx % 16) * 16, (gy % 16) * 16],
            })
        })
        .collect();
    let mut tlist = DisplayList::new();
    tlist
        .push(layer(full, Arc::clone(&base), CompositeOp::Replace, 255))
        .expect("push");
    tlist
        .push(PaintOp::Text(Arc::new(GlyphRun {
            clip: full,
            colour: 0xFFC8_6400,
            coverage: map,
            glyphs,
        })))
        .expect("push");
    let text = time_paint(&tlist, &mut set, full, iters)
        .saturating_sub(time_paint(&fill, &mut set, full, iters));
    println!(
        "{:<34} {:>10.3}",
        "text (fonts.md §8.4), net of base",
        ns_per_px(text, px)
    );

    // The walking-phase minimap of §6: the 170 × 170 viewport repainted. Shapes after
    // `game-panels.md` §8.7 (synthetic): an opaque disc with a transparent outside, opaque
    // map tiles drawn with Mix colour, and a rim that is opaque on a ring and transparent
    // elsewhere. The "random alpha" row uses noise for every alpha (worst case for Over).
    let vp = Rect::from_xywh(27, 55, 170, 170);
    let disc = shaped(170, |x, y| {
        let (dx, dy) = (f64::from(x) - 84.5, f64::from(y) - 84.5);
        if dx.hypot(dy) <= 85.0 { 0xFF20_3040 } else { 0 }
    });
    let rim = shaped(237, |x, y| {
        let (dx, dy) = (f64::from(x) - 118.0, f64::from(y) - 118.0);
        let r = dx.hypot(dy);
        if (84.0..96.0).contains(&r) {
            0xFF80_6040
        } else {
            0
        }
    });
    let map_tiles = opaque_image(128, 128);
    for (label, disc, rim) in [
        ("shaped", disc, rim),
        (
            "random alpha",
            image(170, 170, 3, PixelLayout::A8R8G8B8),
            image(237, 237, 11, PixelLayout::A8R8G8B8),
        ),
    ] {
        let mut mm = CanvasSet::new(224, 280).expect("size");
        let mut list = DisplayList::new();
        list.push(layer(vp, disc, CompositeOp::Replace, 255))
            .expect("push");
        list.push(layer(
            vp,
            Arc::clone(&map_tiles),
            CompositeOp::MixColour,
            255,
        ))
        .expect("push");
        let rim_box = Rect::from_xywh(vp.x0 - 34, vp.y0 - 33, 237, 237);
        list.push(PaintOp::Image(ImageLayer {
            rect: rim_box,
            clip: vp,
            anchor: [rim_box.x0, rim_box.y0],
            image: rim,
            op: CompositeOp::Over,
            opacity: 255,
        }))
        .expect("push");
        let t = time_paint(&list, &mut mm, vp, iters.max(201));
        println!(
            "minimap 170 x 170, {label:<12} (erase, Replace, Mix colour, Over): {:.1} us ({:.3} ns/pixel)",
            t.as_nanos() as f64 / 1000.0,
            ns_per_px(t, vp.area())
        );
    }
    println!("Indicative only; re-measure on the idle reference machine (decision 0010).");
}
