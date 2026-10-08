// SPDX-License-Identifier: GPL-3.0-or-later
//! The painter on real UI art from a user's own install (read-only), skipped unless
//! `TESSERA_GAME_DIR` is set: a sample of render surfaces (0x41) is converted with
//! `tessera-material::ui_image::from_surface` and drawn as base and overlay layers with
//! every compositor operation over an element larger than the image, anchored off the
//! element's corner. Checks: Replace reproduces the image pixels at native size, tiled from
//! the anchor; an overlay image draws only for the *ARGB32* and *alpha-only* tags (decision
//! 0015 §4.2); repainting by a grid of rectangles that crosses the tile edge gives the bytes
//! of one full repaint; the decoded-image cache keeps its budgets.
//!
//! Run with `TESSERA_GAME_DIR=… cargo test -p tessera-ui --release --test paint_real_data
//! -- --nocapture`. `TESSERA_SAMPLE` overrides the sample size and `TESSERA_SEED` the seed.
//! Only counts are printed; nothing is written.

use std::collections::BTreeMap;
use std::path::PathBuf;
use std::sync::Arc;

use tessera_formats::{Record, Surface};
use tessera_material::ui_image::{SourceTag, from_surface};
use tessera_props::{Did, GameData};
use tessera_ui::paint::{
    CanvasSet, CompositeOp, DisplayList, FillRect, ImageCache, ImageKey, ImageLayer, OverlayLayer,
    PaintOp, Rect, paint_rect,
};

fn game_dir() -> Option<PathBuf> {
    match std::env::var_os("TESSERA_GAME_DIR") {
        Some(d) if !d.is_empty() => Some(PathBuf::from(d)),
        _ => {
            eprintln!("TESSERA_GAME_DIR not set; skipping");
            None
        }
    }
}

fn env_u64(name: &str, default: u64) -> u64 {
    std::env::var(name)
        .ok()
        .and_then(|v| v.parse().ok())
        .unwrap_or(default)
}

struct Rng(u64);

impl Rng {
    fn next(&mut self) -> u64 {
        self.0 ^= self.0 >> 12;
        self.0 ^= self.0 << 25;
        self.0 ^= self.0 >> 27;
        self.0.wrapping_mul(0x2545_F491_4F6C_DD1D)
    }
}

const OPS: [CompositeOp; 7] = [
    CompositeOp::Replace,
    CompositeOp::MixColour,
    CompositeOp::Over,
    CompositeOp::Multiply,
    CompositeOp::Screen,
    CompositeOp::Colorize,
    CompositeOp::Grayscale,
];

#[test]
fn real_surfaces_paint_exactly() {
    let Some(dir) = game_dir() else { return };
    let gd = GameData::open(&dir).expect("open install");
    let mut dids = std::collections::BTreeSet::new();
    for member in gd.source().members() {
        for entry in member.archive.entries_range(0x4100_0000..0x4200_0000) {
            dids.insert(entry.expect("directory walk").did);
        }
    }
    let dids: Vec<u32> = dids.into_iter().collect();
    assert!(!dids.is_empty(), "no render surfaces");
    let sample = env_u64("TESSERA_SAMPLE", 120) as usize;
    let mut rng = Rng(env_u64("TESSERA_SEED", 0x0015) | 1);
    let mut cache = ImageCache::new(8 << 20, 32 << 20);
    let mut by_tag: BTreeMap<String, usize> = BTreeMap::new();
    let mut painted = 0usize;
    for _ in 0..sample {
        let did = dids[(rng.next() % dids.len() as u64) as usize];
        let bytes = gd.read(Did(did)).expect("read").expect("listed DID");
        let Ok(surface) = Surface::parse_expecting(&bytes, did) else {
            continue;
        };
        let Ok(img) = from_surface(&surface) else {
            continue;
        };
        // Keep the canvases small: images up to 512 on a side.
        if img.width() > 512 || img.height() > 512 {
            continue;
        }
        let tag = img.tag();
        *by_tag.entry(format!("{tag:?}")).or_default() += 1;
        let img = cache.insert(ImageKey::Did(did), img).expect("cache");
        let (iw, ih) = (img.width() as i32, img.height() as i32);
        // An element 1.5 times the image plus 3, anchored 2 pixels in, so tiling wraps,
        // and a canvas that crosses the 256-pixel tile edge when the image is large enough.
        let elem = Rect::from_xywh(1, 1, (iw * 3 / 2 + 3) as u32, (ih * 3 / 2 + 3) as u32);
        let anchor = [3, 3];
        let size = [elem.x1 as u32 + 1, elem.y1 as u32 + 1];
        let op = OPS[(rng.next() % OPS.len() as u64) as usize];
        let colour = (rng.next() as u32) | 0x8000_0000;
        let mut list = DisplayList::new();
        list.push(PaintOp::Fill(FillRect {
            rect: elem,
            clip: elem,
            colour: 0xC040_6080,
        }))
        .unwrap();
        list.push(PaintOp::Image(ImageLayer {
            rect: elem,
            clip: elem,
            anchor,
            image: Arc::clone(&img),
            op,
            opacity: 200,
        }))
        .unwrap();
        list.push(PaintOp::Overlay(OverlayLayer {
            rect: elem,
            clip: elem,
            colour,
            op: OPS[(rng.next() % OPS.len() as u64) as usize],
            image: Some((Arc::clone(&img), anchor)),
        }))
        .unwrap();
        let full = Rect::of_size(size[0], size[1]);
        let mut whole = CanvasSet::new(size[0], size[1]).unwrap();
        paint_rect(&list, &mut whole, full);
        let mut split = CanvasSet::new(size[0], size[1]).unwrap();
        let (cx, cy) = ((size[0] / 3) as i32, (size[1] * 2 / 3) as i32);
        for r in [
            Rect::new(0, 0, cx, cy),
            Rect::new(cx, 0, full.x1, cy),
            Rect::new(0, cy, full.x1, full.y1),
        ] {
            paint_rect(&list, &mut split, r);
        }
        assert_eq!(
            whole.below().to_vec(),
            split.below().to_vec(),
            "{did:#010x}: partition"
        );

        // Replace alone reproduces the image at native size from the anchor.
        let mut l2 = DisplayList::new();
        l2.push(PaintOp::Image(ImageLayer {
            rect: elem,
            clip: elem,
            anchor,
            image: Arc::clone(&img),
            op: CompositeOp::Replace,
            opacity: 255,
        }))
        .unwrap();
        let mut c = CanvasSet::new(size[0], size[1]).unwrap();
        paint_rect(&l2, &mut c, full);
        for y in elem.y0..elem.y1 {
            for x in elem.x0..elem.x1 {
                let sx = (x - anchor[0]).rem_euclid(iw) as u32;
                let sy = (y - anchor[1]).rem_euclid(ih) as u32;
                assert_eq!(
                    Some(c.below().pixel(x, y)),
                    img.pixel(sx, sy),
                    "{did:#010x}"
                );
            }
        }
        assert_eq!(c.below().pixel(0, 0), 0, "outside the element");

        // The overlay image draws only for ARGB32 and alpha-only sources.
        let mut l3 = DisplayList::new();
        l3.push(PaintOp::Fill(FillRect {
            rect: elem,
            clip: elem,
            colour: 0xFF20_4060,
        }))
        .unwrap();
        l3.push(PaintOp::Overlay(OverlayLayer {
            rect: elem,
            clip: elem,
            colour: 0xFFFF_FFFF,
            op: CompositeOp::Over,
            image: Some((Arc::clone(&img), anchor)),
        }))
        .unwrap();
        let mut c3 = CanvasSet::new(size[0], size[1]).unwrap();
        paint_rect(&l3, &mut c3, full);
        if !matches!(tag, SourceTag::Argb32 | SourceTag::AlphaOnly) {
            assert!(
                (elem.y0..elem.y1)
                    .all(|y| (elem.x0..elem.x1).all(|x| c3.below().pixel(x, y) == 0xFF20_4060)),
                "{did:#010x}: overlay of a {tag:?} image drew"
            );
        }
        painted += 1;
    }
    let st = cache.stats();
    assert!(st.unpinned_bytes <= 8 << 20);
    eprintln!(
        "painted {painted} real surfaces; by tag {by_tag:?}; cache {} images, {} evictions",
        st.images, st.evictions
    );
    assert!(painted > 0);
}
