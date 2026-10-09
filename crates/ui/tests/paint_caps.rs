// SPDX-License-Identifier: GPL-3.0-or-later
//! Cap enforcement against hostile input (decision 0015 §7, U5): canvas sizes and the
//! on-screen crop, display-list and glyph caps, surface caps per apartment, in total and in
//! depth, the image cache's hard cap, bounded damage, and painting with extreme
//! coordinates. Synthetic data only.

use std::sync::Arc;

use tessera_material::ui_image::{PixelLayout, UiImage};
use tessera_ui::paint::{
    CacheError, CanvasSet, CapRefusal, CompositeOp, CoverageMap, DamageList, DisplayList, FillRect,
    GlyphBox, GlyphRun, ImageCache, ImageKey, ImageLayer, MAX_NESTED, MAX_OPS, MemoryUse,
    PaintError, PaintLimits, PaintOp, Rect, SurfaceCounter, canvas_extent, paint_rect,
};

#[test]
fn canvas_size_caps() {
    assert!(matches!(
        CanvasSet::new(16_385, 1),
        Err(PaintError::CanvasSize { .. })
    ));
    assert!(CanvasSet::new(0, 10).is_err());
    let c = CanvasSet::new(16_384, 16_384).unwrap();
    assert_eq!(c.allocated_bytes(), 0, "a huge empty canvas costs nothing");
    // Device limit below 16,384: the over-size axis keeps only its on-screen part.
    let lim = PaintLimits::default().with_device_max_dim(8192);
    let e = canvas_extent(
        &Rect::from_xywh(-500, -20, 9000, 300),
        [3840, 2160],
        lim.max_surface_dim,
    )
    .unwrap();
    assert_eq!((e.size, e.origin, e.cropped), ([3840, 300], [500, 0], true));
}

#[test]
fn display_list_caps() {
    let mut l = DisplayList::new();
    for i in 0..MAX_NESTED {
        l.push(PaintOp::Nested {
            entry: i as u32,
            coverage: Rect::EMPTY,
        })
        .unwrap();
    }
    assert_eq!(
        l.push(PaintOp::Nested {
            entry: 0,
            coverage: Rect::EMPTY
        }),
        Err(PaintError::TooManyOps)
    );
    let f = PaintOp::Fill(FillRect {
        rect: Rect::EMPTY,
        clip: Rect::EMPTY,
        colour: 0,
    });
    while l.len() < MAX_OPS {
        l.push(f.clone()).unwrap();
    }
    assert_eq!(l.push(f), Err(PaintError::TooManyOps));

    let map = Arc::new(CoverageMap::new(1, 1, vec![255]).unwrap());
    let run = GlyphRun {
        clip: Rect::EMPTY,
        colour: 0,
        coverage: map,
        glyphs: vec![
            GlyphBox {
                dest: [0, 0],
                size: [1, 1],
                src: [0, 0]
            };
            (1 << 16) + 1
        ],
    };
    assert_eq!(
        DisplayList::new().push(PaintOp::Text(Arc::new(run))),
        Err(PaintError::TooManyGlyphs)
    );
    assert!(CoverageMap::new(2, 2, vec![0; 3]).is_err());
    assert!(CoverageMap::new(8193, 1, vec![0; 8193]).is_err());
}

#[test]
fn surface_caps() {
    let lim = PaintLimits::default();
    let mut c = SurfaceCounter::new(lim);
    for _ in 0..512 {
        c.admit(Some(7), 1, MemoryUse::default()).unwrap();
    }
    assert_eq!(
        c.admit(Some(7), 1, MemoryUse::default()),
        Err(CapRefusal::ApartmentSurfaces(512))
    );
    assert!(
        c.admit(Some(8), 1, MemoryUse::default()).is_ok(),
        "another apartment has its own cap"
    );
    for a in 9..16 {
        for _ in 0..512 {
            let _ = c.admit(Some(a), 0, MemoryUse::default());
        }
    }
    assert_eq!(c.total(), 4096);
    assert_eq!(
        c.admit(None, 0, MemoryUse::default()),
        Err(CapRefusal::TotalSurfaces(4096))
    );
    c.release(Some(7));
    assert_eq!(
        c.admit(None, 9, MemoryUse::default()),
        Err(CapRefusal::Depth(8))
    );
    assert_eq!(
        c.admit(
            None,
            0,
            MemoryUse {
                canvas_now: lim.canvas_bytes,
                ..MemoryUse::default()
            }
        ),
        Err(CapRefusal::Memory),
        "the canvas budget (128 MiB) is full"
    );
    let k = c.counters();
    assert!(k.apartment_surfaces >= 1 && k.total_surfaces >= 1 && k.depth == 1 && k.memory == 1);
}

#[test]
fn image_cache_hard_cap() {
    let img = |s: u32| {
        UiImage::new(s, s, s, s, PixelLayout::A8R8G8B8, vec![0; (s * s) as usize]).unwrap()
    };
    // 64 × 64 images are 16 KiB; hard cap 40 KiB, LRU 20 KiB.
    let mut c = ImageCache::new(20 * 1024, 40 * 1024);
    c.insert_pinned(ImageKey::Did(1), img(64)).unwrap();
    c.insert_pinned(ImageKey::Did(2), img(64)).unwrap();
    assert!(matches!(
        c.insert_pinned(ImageKey::Did(3), img(64)),
        Err(CacheError::PinnedCapExceeded { .. })
    ));
    // A refused image is not kept: the caller draws the placeholder (§4.5).
    assert!(!c.contains(&ImageKey::Did(3)));
    assert!(c.get(&ImageKey::Did(3)).is_none());
    // Unpinned images never push out pinned ones.
    for i in 10..30 {
        c.insert(ImageKey::Did(i), img(64)).unwrap();
    }
    assert!(c.contains(&ImageKey::Did(1)) && c.contains(&ImageKey::Did(2)));
    assert!(c.stats().unpinned_bytes <= 20 * 1024);
    let long = ImageKey::Path("x".repeat(5000));
    assert!(matches!(
        c.insert(long, img(1)),
        Err(CacheError::KeyTooLong(5000))
    ));
}

#[test]
fn damage_is_bounded() {
    let mut d = DamageList::new();
    for i in 0..100_000 {
        d.add(Rect::new(i, -i, i + 1, -i + 1));
    }
    let m = d.take_merged([16_384, 16_384]);
    assert!(m.len() <= 16);
    d.add(Rect::new(i32::MIN, i32::MIN, i32::MAX, i32::MAX));
    assert_eq!(d.take_merged([10, 10]), vec![Rect::new(0, 0, 10, 10)]);
}

#[test]
fn extreme_coordinates_paint_inside_the_canvas_only() {
    let image =
        Arc::new(UiImage::new(3, 2, 3, 2, PixelLayout::A8R8G8B8, vec![0xFFFF_FFFF; 6]).unwrap());
    let huge = Rect::new(i32::MIN, i32::MIN, i32::MAX, i32::MAX);
    let mut l = DisplayList::new();
    l.push(PaintOp::Image(ImageLayer {
        rect: huge,
        clip: huge,
        anchor: [i32::MIN, i32::MAX],
        image,
        op: CompositeOp::Over,
        opacity: 255,
    }))
    .unwrap();
    let map = Arc::new(CoverageMap::new(2, 2, vec![255; 4]).unwrap());
    l.push(PaintOp::Text(Arc::new(GlyphRun {
        clip: huge,
        colour: 0xFF00_0000,
        coverage: map,
        glyphs: vec![GlyphBox {
            dest: [i32::MAX - 1, i32::MIN],
            size: [u32::MAX, u32::MAX],
            src: [i32::MIN, i32::MAX],
        }],
    })))
    .unwrap();
    l.push(PaintOp::Nested {
        entry: u32::MAX,
        coverage: huge,
    })
    .unwrap();
    l.push(PaintOp::Fill(FillRect {
        rect: huge,
        clip: Rect::new(2, 2, 4, 4),
        colour: 0xFF12_3456,
    }))
    .unwrap();
    let mut set = CanvasSet::new(5, 5).unwrap();
    paint_rect(&l, &mut set, huge);
    assert_eq!(set.below().pixel(0, 0), 0xFFFF_FFFF);
    assert_eq!(set.above(u32::MAX).unwrap().pixel(3, 3), 0xFF12_3456);
    assert_eq!(set.below().pixel(3, 3), 0xFFFF_FFFF);
    assert_eq!(
        set.below().pixel(5, 5),
        0,
        "outside reads as transparent black"
    );
}
