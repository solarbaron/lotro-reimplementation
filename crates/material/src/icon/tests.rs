// SPDX-License-Identifier: GPL-3.0-or-later
//! Synthetic fixtures of `icon-layers.md` §8.

use std::cell::Cell;
use std::collections::HashMap;

use super::*;

const R8G8B8: u32 = 20;
const A8R8G8B8: u32 = 21;

/// A surface record: DID, category, width, height, format, length, data.
fn surface_record(did: u32, w: u32, h: u32, format: u32, data: &[u8]) -> Vec<u8> {
    let mut v = Vec::new();
    for x in [did, 15, w, h, format, data.len() as u32] {
        v.extend_from_slice(&x.to_le_bytes());
    }
    v.extend_from_slice(data);
    v
}

/// An A8R8G8B8 record from RGBA pixels (stored B, G, R, A).
fn argb(did: u32, w: u32, h: u32, rgba: &[[u8; 4]]) -> Vec<u8> {
    let data: Vec<u8> = rgba.iter().flat_map(|p| [p[2], p[1], p[0], p[3]]).collect();
    surface_record(did, w, h, A8R8G8B8, &data)
}

/// An R8G8B8 record from RGB pixels (stored B, G, R).
fn rgb(did: u32, w: u32, h: u32, px: &[[u8; 3]]) -> Vec<u8> {
    let data: Vec<u8> = px.iter().flat_map(|p| [p[2], p[1], p[0]]).collect();
    surface_record(did, w, h, R8G8B8, &data)
}

type Source = HashMap<u32, Vec<u8>>;

fn composer(
    src: Source,
) -> IconComposer<impl Fn(Did) -> std::result::Result<Option<Vec<u8>>, std::io::Error>> {
    IconComposer::new(move |d: Did| Ok(src.get(&d.0).cloned()))
}

fn props(map: &[(&'static str, u32)]) -> impl Fn(&str) -> Option<Did> + use<> {
    let map: HashMap<&'static str, u32> = map.iter().copied().collect();
    move |n| map.get(n).map(|&v| Did(v))
}

fn px(img: &RgbaImage, x: u32, y: u32) -> [u8; 4] {
    let i = ((y * img.width + x) * 4) as usize;
    [
        img.pixels[i],
        img.pixels[i + 1],
        img.pixels[i + 2],
        img.pixels[i + 3],
    ]
}

fn close(a: [u8; 4], b: [u8; 4]) -> bool {
    a.iter().zip(b.iter()).all(|(x, y)| x.abs_diff(*y) <= 1)
}

const BG: u32 = 0x4100_0001;
const UNDER: u32 = 0x4100_0002;
const SHADOW: u32 = 0x4100_0003;
const IMAGE: u32 = 0x4100_0004;
const OVER: u32 = 0x4100_0005;
const CLEAR: u32 = 0x4100_0006;

#[test]
fn formula_over_opaque_background() {
    let mut src = Source::new();
    src.insert(BG, rgb(BG, 2, 2, &[[100, 50, 25]; 4]));
    src.insert(
        IMAGE,
        argb(
            IMAGE,
            2,
            2,
            &[
                [255, 255, 255, 0],
                [255, 255, 255, 128],
                [0, 200, 0, 255],
                [10, 20, 30, 255],
            ],
        ),
    );
    let c = composer(src).compose_item(props(&[(names::BACKGROUND, BG), (names::IMAGE, IMAGE)]));
    let img = c.image.as_ref().unwrap();
    assert_eq!((img.width, img.height), (2, 2));
    // Alpha 0 takes the background colour.
    assert_eq!(px(img, 0, 0), [100, 50, 25, 255]);
    // Alpha 128: (255·128 + 100·127) / 255 ≈ 177.8, etc.
    assert!(
        close(px(img, 1, 0), [178, 153, 140, 255]),
        "{:?}",
        px(img, 1, 0)
    );
    assert_eq!(px(img, 0, 1), [0, 200, 0, 255]);
    assert_eq!(px(img, 1, 1), [10, 20, 30, 255]);
    assert!(c.skipped().next().is_none());
}

#[test]
fn order_and_placement_with_single_pixels() {
    // Each layer is opaque at one pixel of a 3×3 grid (plus a shared pixel at 2,2 to
    // check which layer ends on top).
    let mut src = Source::new();
    let roles = [(BG, 0), (UNDER, 1), (SHADOW, 3), (IMAGE, 4), (OVER, 5)];
    let colours = [
        [255, 0, 0, 255],
        [0, 255, 0, 255],
        [0, 0, 255, 255],
        [255, 255, 0, 255],
        [0, 255, 255, 255],
    ];
    for (&(did, pos), colour) in roles.iter().zip(colours) {
        let mut p = [[0u8; 4]; 9];
        p[pos] = colour;
        p[8] = colour;
        src.insert(did, argb(did, 3, 3, &p));
    }
    let c = composer(src).compose_item(props(&[
        (names::OVERLAY, OVER),
        (names::IMAGE, IMAGE),
        (names::SHADOW, SHADOW),
        (names::UNDERLAY, UNDER),
        (names::BACKGROUND, BG),
    ]));
    let img = c.image.unwrap();
    for ((_, pos), colour) in roles.iter().zip(colours) {
        let (x, y) = ((*pos % 3) as u32, (*pos / 3) as u32);
        assert_eq!(px(&img, x, y), colour);
    }
    assert_eq!(px(&img, 2, 2), colours[4], "overlay is on top");
    assert_eq!(px(&img, 2, 0), [0; 4], "untouched pixel stays transparent");
}

#[test]
fn soft_shadow_leaves_opaque_image_pixels_unchanged() {
    let mut src = Source::new();
    src.insert(BG, rgb(BG, 2, 2, &[[200, 200, 200]; 4]));
    src.insert(SHADOW, argb(SHADOW, 2, 2, &[[0, 0, 0, 180]; 4]));
    src.insert(
        IMAGE,
        argb(
            IMAGE,
            2,
            2,
            &[[9, 99, 199, 255], [0; 4], [0; 4], [50, 60, 70, 255]],
        ),
    );
    let c = composer(src).compose_item(props(&[
        (names::BACKGROUND, BG),
        (names::SHADOW, SHADOW),
        (names::IMAGE, IMAGE),
    ]));
    let img = c.image.unwrap();
    assert_eq!(px(&img, 0, 0), [9, 99, 199, 255]);
    assert_eq!(px(&img, 1, 1), [50, 60, 70, 255]);
    // Shadow darkens the background where the image is transparent.
    let dark = px(&img, 1, 0);
    assert!(close(dark, [59, 59, 59, 255]), "{dark:?}");
}

#[test]
fn absent_missing_and_transparent_layers_change_nothing() {
    let mut src = Source::new();
    src.insert(
        BG,
        rgb(BG, 2, 2, &[[1, 2, 3], [4, 5, 6], [7, 8, 9], [10, 11, 12]]),
    );
    src.insert(
        IMAGE,
        argb(
            IMAGE,
            2,
            2,
            &[[255, 0, 0, 77], [0; 4], [0, 0, 0, 255], [9, 9, 9, 200]],
        ),
    );
    src.insert(CLEAR, argb(CLEAR, 2, 2, &[[0; 4]; 4]));
    let comp = composer(src);
    let base = comp
        .compose_item(props(&[(names::BACKGROUND, BG), (names::IMAGE, IMAGE)]))
        .image
        .unwrap();
    // Value 0.
    let zero = comp.compose_item(props(&[
        (names::BACKGROUND, BG),
        (names::UNDERLAY, 0),
        (names::IMAGE, IMAGE),
    ]));
    assert_eq!(zero.layers.len(), 2);
    assert_eq!(zero.image.unwrap(), base);
    // A DID no archive holds.
    let missing = comp.compose_item(props(&[
        (names::BACKGROUND, BG),
        (names::SHADOW, 0x4100_7777),
        (names::IMAGE, IMAGE),
    ]));
    let skipped: Vec<_> = missing.skipped().collect();
    assert_eq!(skipped.len(), 1);
    assert!(matches!(skipped[0].1, MaterialError::Missing { .. }));
    assert_eq!(missing.image.unwrap(), base);
    // The transparent placeholder, as Underlay and Shadow.
    let clear = comp.compose_item(props(&[
        (names::BACKGROUND, BG),
        (names::UNDERLAY, CLEAR),
        (names::SHADOW, CLEAR),
        (names::IMAGE, IMAGE),
    ]));
    assert!(matches!(
        clear.layers[1].status,
        LayerStatus::Transparent {
            width: 2,
            height: 2
        }
    ));
    assert_eq!(clear.image.unwrap(), base);
    // Drawing the transparent layer for real gives the same pixels too.
    let mut drawn = base.clone();
    draw_centred(&mut drawn, &comp.layer(Did(CLEAR)).unwrap()).unwrap();
    assert_eq!(drawn, base);
}

#[test]
fn smaller_image_is_centred() {
    let mut src = Source::new();
    src.insert(BG, rgb(BG, 4, 4, &[[0, 0, 0]; 16]));
    src.insert(IMAGE, argb(IMAGE, 2, 2, &[[255, 255, 255, 255]; 4]));
    let img = composer(src)
        .compose_item(props(&[(names::BACKGROUND, BG), (names::IMAGE, IMAGE)]))
        .image
        .unwrap();
    assert_eq!((img.width, img.height), (4, 4));
    for y in 0..4 {
        for x in 0..4 {
            let white = (1..=2).contains(&x) && (1..=2).contains(&y);
            assert_eq!(px(&img, x, y)[0], if white { 255 } else { 0 }, "({x},{y})");
        }
    }
}

#[test]
fn larger_image_is_clipped() {
    let mut src = Source::new();
    src.insert(BG, rgb(BG, 4, 4, &[[0, 0, 0]; 16]));
    // 6×6 image whose pixel value encodes its own coordinates.
    let p: Vec<[u8; 4]> = (0..36u8).map(|i| [i % 6, i / 6, 0, 255]).collect();
    src.insert(IMAGE, argb(IMAGE, 6, 6, &p));
    let img = composer(src)
        .compose_item(props(&[(names::BACKGROUND, BG), (names::IMAGE, IMAGE)]))
        .image
        .unwrap();
    assert_eq!((img.width, img.height), (4, 4));
    // Offset −1: canvas (0,0) shows image (1,1).
    assert_eq!(px(&img, 0, 0), [1, 1, 0, 255]);
    assert_eq!(px(&img, 3, 3), [4, 4, 0, 255]);
}

#[test]
fn canvas_without_background_is_largest_layer() {
    let mut src = Source::new();
    src.insert(SHADOW, argb(SHADOW, 3, 1, &[[0, 0, 0, 255]; 3]));
    src.insert(IMAGE, argb(IMAGE, 1, 5, &[[255, 0, 0, 255]; 5]));
    let img = composer(src)
        .compose_item(props(&[(names::SHADOW, SHADOW), (names::IMAGE, IMAGE)]))
        .image
        .unwrap();
    assert_eq!((img.width, img.height), (3, 5));
    assert_eq!(px(&img, 0, 2), [0, 0, 0, 255]);
    assert_eq!(px(&img, 1, 2), [255, 0, 0, 255]);
    assert_eq!(px(&img, 0, 0), [0; 4]);
    assert_eq!(
        canvas_size(&[(LayerRole::Image, (8, 8)), (LayerRole::Background, (4, 2))]),
        Some((4, 2))
    );
    assert_eq!(canvas_size(&[]), None);
}

#[test]
fn trait_frame_over_base_picture() {
    let (base, frame) = (0x4100_0010, 0x4100_0011);
    let mut src = Source::new();
    src.insert(base, argb(base, 3, 3, &[[0, 0, 200, 255]; 9]));
    let mut f = [[200u8, 150, 0, 255]; 9];
    f[4] = [0; 4]; // transparent centre
    src.insert(frame, argb(frame, 3, 3, &f));
    let c = composer(src).compose(&IconStack::trait_icon(props(&[
        (names::TRAIT_ICON, base),
        (names::TRAIT_STATIC_OVERLAY, frame),
    ])));
    let img = c.image.unwrap();
    assert_eq!(px(&img, 1, 1), [0, 0, 200, 255]);
    assert_eq!(px(&img, 0, 0), [200, 150, 0, 255]);
}

#[test]
fn no_layers_is_no_icon() {
    let c = composer(Source::new()).compose(&IconStack::new());
    assert!(c.image.is_none());
    assert!(c.layers.is_empty());
    assert_eq!(compose_images(&[]).unwrap(), None);
}

#[test]
fn bad_layers_are_skipped_not_fatal() {
    let mut src = Source::new();
    src.insert(BG, rgb(BG, 2, 2, &[[5, 5, 5]; 4]));
    src.insert(UNDER, vec![1, 2, 3]); // truncated
    src.insert(SHADOW, rgb(0x4100_9999, 2, 2, &[[0, 0, 0]; 4])); // wrong DID inside
    src.insert(IMAGE, surface_record(IMAGE, 2, 2, 0xDEAD, &[0; 16])); // unknown format
    src.insert(OVER, rgb(OVER, 4096, 1, &vec![[0, 0, 0]; 4096])); // too wide
    let c = composer(src).compose_item(props(&[
        (names::BACKGROUND, BG),
        (names::UNDERLAY, UNDER),
        (names::SHADOW, SHADOW),
        (names::IMAGE, IMAGE),
        (names::OVERLAY, OVER),
    ]));
    let errs: Vec<_> = c.skipped().map(|(e, err)| (e.role, err)).collect();
    assert_eq!(errs.len(), 4);
    assert!(matches!(errs[0].1, MaterialError::Decode { .. }));
    assert!(matches!(errs[1].1, MaterialError::Decode { .. }));
    assert!(matches!(errs[2].1, MaterialError::Decode { .. }));
    assert!(matches!(errs[3].1, MaterialError::TooLarge { .. }));
    assert_eq!(c.image.unwrap().pixels, [5, 5, 5, 255].repeat(4));
}

#[test]
fn non_surface_dids_and_source_errors() {
    let calls = Cell::new(0);
    let comp = IconComposer::new(|d: Did| {
        calls.set(calls.get() + 1);
        if d.0 == 0x4100_0BAD {
            Err(std::io::Error::other("disk on fire"))
        } else {
            Ok(None)
        }
    });
    assert!(matches!(
        comp.layer(Did(0x7000_0001)),
        Err(MaterialError::NotSurface { .. })
    ));
    assert_eq!(calls.get(), 0, "non-surface DIDs are not read");
    assert!(matches!(
        comp.layer(Did(0x4100_0BAD)),
        Err(MaterialError::Source { .. })
    ));
    assert!(matches!(
        decode_layer(Did(0x4000_0001), &[]),
        Err(MaterialError::NotSurface { .. })
    ));
}

#[test]
fn cache_hits_and_limits() {
    let mut src = Source::new();
    for i in 0..4u32 {
        let did = 0x4100_0100 + i;
        src.insert(did, argb(did, 2, 2, &[[1, 2, 3, 255]; 4]));
    }
    let reads = std::sync::atomic::AtomicUsize::new(0);
    let comp = IconComposer::new(|d: Did| {
        reads.fetch_add(1, std::sync::atomic::Ordering::Relaxed);
        Ok::<_, std::io::Error>(src.get(&d.0).cloned())
    })
    .with_cache_limits(2, 1 << 20);
    comp.layer(Did(0x4100_0100)).unwrap();
    comp.layer(Did(0x4100_0100)).unwrap();
    assert_eq!(reads.load(std::sync::atomic::Ordering::Relaxed), 1);
    assert_eq!(comp.cache_usage(), (1, 16));
    for i in 0..4 {
        comp.layer(Did(0x4100_0100 + i)).unwrap();
    }
    assert!(comp.cache_usage().0 <= 2);
    comp.clear_cache();
    assert_eq!(comp.cache_usage(), (0, 0));
    // Disabled cache.
    let off = IconComposer::new(|d: Did| Ok::<_, std::io::Error>(src.get(&d.0).cloned()))
        .with_cache_limits(0, 0);
    off.layer(Did(0x4100_0100)).unwrap();
    assert_eq!(off.cache_usage(), (0, 0));
    assert!(format!("{off:?}").contains("IconComposer"));
}

#[test]
fn compose_images_validates_input() {
    let ok = RgbaImage {
        width: 1,
        height: 1,
        pixels: vec![1, 2, 3, 4],
    };
    let bad = RgbaImage {
        width: 2,
        height: 1,
        pixels: vec![0; 4],
    };
    assert!(compose_images(&[(LayerRole::Single, &bad)]).is_err());
    let many = vec![(LayerRole::Single, &ok); MAX_STACK_LAYERS + 1];
    assert!(matches!(
        compose_images(&many),
        Err(MaterialError::TooManyLayers { .. })
    ));
    let img = compose_images(&[(LayerRole::Single, &ok)])
        .unwrap()
        .unwrap();
    assert_eq!(img, ok);
}

/// Builds a JPEG-format surface record whose stream declares a `w`×`h` baseline frame
/// but holds no scan data.
fn jpeg_header_only(did: u32, w: u16, h: u16) -> Vec<u8> {
    let mut j = vec![0xFF, 0xD8];
    // An APP0-like segment to skip, then SOF0: length 17, precision 8, height, width,
    // 3 components.
    j.extend_from_slice(&[0xFF, 0xE0, 0x00, 0x04, 0x00, 0x00]);
    j.extend_from_slice(&[0xFF, 0xC0, 0x00, 0x11, 0x08]);
    j.extend_from_slice(&h.to_be_bytes());
    j.extend_from_slice(&w.to_be_bytes());
    j.extend_from_slice(&[3, 1, 0x22, 0, 2, 0x11, 1, 3, 0x11, 1]);
    j.extend_from_slice(&[0xFF, 0xD9]);
    surface_record(did, 0, 0, 500, &j)
}

#[test]
fn jpeg_frame_size_peek() {
    let rec = jpeg_header_only(1, 300, 200);
    assert_eq!(jpeg_frame_size(&rec[24..]), Some((300, 200)));
    // Hostile streams: no panic, no size.
    assert_eq!(jpeg_frame_size(&[]), None);
    assert_eq!(jpeg_frame_size(&[0xFF, 0xD8]), None);
    assert_eq!(jpeg_frame_size(&[0xFF, 0xD8, 0xFF, 0xC0, 0x00, 0x01]), None);
    assert_eq!(jpeg_frame_size(&[0xFF, 0xD8, 0xFF, 0xDA, 0x00, 0x02]), None);
    assert_eq!(jpeg_frame_size(&[0xFF, 0xD8, 0x00]), None);
    for cut in 0..rec.len() - 24 {
        let _ = jpeg_frame_size(&rec[24..24 + cut]);
    }
}

#[test]
fn oversized_jpeg_is_refused_before_decoding_and_remembered() {
    let did = 0x4100_0001;
    let mut src = Source::new();
    src.insert(did, jpeg_header_only(did, 5000, 5000));
    let reads = std::sync::atomic::AtomicUsize::new(0);
    let comp = IconComposer::new(|d: Did| {
        reads.fetch_add(1, std::sync::atomic::Ordering::Relaxed);
        Ok::<_, std::io::Error>(src.get(&d.0).cloned())
    });
    for _ in 0..3 {
        let c = comp.compose(&IconStack::single(Did(did)));
        assert!(c.image.is_none());
        assert!(matches!(
            c.layers[0].status,
            LayerStatus::Skipped(MaterialError::TooLarge {
                width: 5000,
                height: 5000,
                limit: MAX_LAYER_DIMENSION
            })
        ));
    }
    assert_eq!(reads.load(std::sync::atomic::Ordering::Relaxed), 1);
    assert_eq!(comp.cached_failures(), 1);
    // The same refusal from the free function, without decoding (the stream has no
    // scan, so decoding would have failed with a decode error instead).
    assert!(matches!(
        decode_layer(Did(did), &jpeg_header_only(did, 5000, 16)),
        Err(MaterialError::TooLarge { .. })
    ));
    // Within the limit, the decoder runs and reports its own error.
    assert!(matches!(
        decode_layer(Did(did), &jpeg_header_only(did, 16, 16)),
        Err(MaterialError::Decode { .. })
    ));
}

#[test]
fn decode_failures_are_remembered_but_missing_and_source_errors_are_not() {
    let mut src = Source::new();
    src.insert(UNDER, vec![1, 2, 3]); // truncated
    let reads = std::sync::atomic::AtomicUsize::new(0);
    let comp = IconComposer::new(|d: Did| {
        reads.fetch_add(1, std::sync::atomic::Ordering::Relaxed);
        if d.0 == 0x4100_0BAD {
            return Err(std::io::Error::other("transient"));
        }
        Ok(src.get(&d.0).cloned())
    });
    let count = || reads.load(std::sync::atomic::Ordering::Relaxed);
    for _ in 0..2 {
        assert!(matches!(
            comp.layer(Did(UNDER)),
            Err(MaterialError::Decode { .. })
        ));
    }
    assert_eq!(count(), 1);
    for _ in 0..2 {
        assert!(matches!(
            comp.layer(Did(0x4100_7777)),
            Err(MaterialError::Missing { .. })
        ));
        assert!(matches!(
            comp.layer(Did(0x4100_0BAD)),
            Err(MaterialError::Source { .. })
        ));
    }
    assert_eq!(count(), 5);
    assert_eq!(comp.cached_failures(), 1);
    comp.clear_cache();
    assert_eq!(comp.cached_failures(), 0);
    // A disabled cache remembers nothing.
    let off = IconComposer::new(|d: Did| Ok::<_, std::io::Error>(Some(vec![d.0 as u8])))
        .with_cache_limits(0, 0);
    assert!(off.layer(Did(UNDER)).is_err());
    assert_eq!(off.cached_failures(), 0);
}

#[test]
fn empty_layers_are_absent() {
    let mut src = Source::new();
    src.insert(BG, rgb(BG, 0, 0, &[]));
    src.insert(UNDER, rgb(UNDER, 0, 7, &[]));
    src.insert(IMAGE, argb(IMAGE, 2, 2, &[[9, 8, 7, 255]; 4]));
    assert!(matches!(
        decode_layer(Did(BG), &rgb(BG, 0, 0, &[])),
        Err(MaterialError::Empty {
            width: 0,
            height: 0
        })
    ));
    let comp = composer(src);
    // An empty Background does not set a 0×0 canvas: the image alone decides.
    for bg in [BG, UNDER] {
        let c = comp.compose_item(props(&[(names::BACKGROUND, bg), (names::IMAGE, IMAGE)]));
        let img = c.image.unwrap();
        assert_eq!((img.width, img.height), (2, 2));
        assert!(matches!(
            c.layers[0].status,
            LayerStatus::Skipped(MaterialError::Empty { .. })
        ));
        assert!(matches!(
            c.layers[1].status,
            LayerStatus::Drawn {
                width: 2,
                height: 2
            }
        ));
    }
    // A stack of only an empty layer is "no icon".
    assert!(comp.compose(&IconStack::single(Did(UNDER))).image.is_none());
    // The same for caller-built images.
    let empty = RgbaImage {
        width: 0,
        height: 7,
        pixels: vec![],
    };
    let ok = RgbaImage {
        width: 1,
        height: 1,
        pixels: vec![1, 2, 3, 255],
    };
    assert_eq!(
        compose_images(&[(LayerRole::Single, &empty)]).unwrap(),
        None
    );
    let img = compose_images(&[(LayerRole::Background, &empty), (LayerRole::Image, &ok)])
        .unwrap()
        .unwrap();
    assert_eq!(img, ok);
    assert_eq!(canvas_size(&[(LayerRole::Background, (0, 4))]), None);
    assert_eq!(
        canvas_size(&[(LayerRole::Background, (0, 4)), (LayerRole::Image, (3, 2))]),
        Some((3, 2))
    );
}

#[test]
fn larger_transparent_layer_without_background_changes_nothing() {
    // §8: a fully transparent layer gives the same result as leaving it out, even when
    // it is larger than the others and there is no Background.
    let mut src = Source::new();
    src.insert(CLEAR, argb(CLEAR, 4, 4, &[[0; 4]; 16]));
    src.insert(IMAGE, argb(IMAGE, 2, 2, &[[1, 2, 3, 255]; 4]));
    let comp = composer(src);
    let alone = comp
        .compose_item(props(&[(names::IMAGE, IMAGE)]))
        .image
        .unwrap();
    let with_clear = comp.compose_item(props(&[(names::SHADOW, CLEAR), (names::IMAGE, IMAGE)]));
    assert!(matches!(
        with_clear.layers[0].status,
        LayerStatus::Transparent {
            width: 4,
            height: 4
        }
    ));
    assert_eq!(with_clear.image.unwrap(), alone);
    assert_eq!((alone.width, alone.height), (2, 2));
    // Only transparent layers: "no icon", as if none were set.
    let only = comp.compose_item(props(&[(names::SHADOW, CLEAR)]));
    assert!(only.image.is_none());
    // The same through compose_images.
    let clear = comp.layer(Did(CLEAR)).unwrap();
    let image = comp.layer(Did(IMAGE)).unwrap();
    assert_eq!(
        compose_images(&[(LayerRole::Shadow, &clear), (LayerRole::Image, &image)]).unwrap(),
        Some(alone)
    );
    assert_eq!(
        compose_images(&[(LayerRole::Shadow, &clear)]).unwrap(),
        None
    );
}

#[test]
fn absent_override_falls_back_to_base_layer() {
    let mut src = Source::new();
    src.insert(BG, rgb(BG, 2, 2, &[[5, 5, 5]; 4]));
    src.insert(IMAGE, argb(IMAGE, 2, 2, &[[200, 0, 0, 255]; 4]));
    src.insert(OVER, vec![0xAB; 3]); // undecodable override
    let comp = composer(src);
    let base = comp
        .compose_item(props(&[(names::BACKGROUND, BG), (names::IMAGE, IMAGE)]))
        .image
        .unwrap();
    for bad in [0x4100_0099, OVER] {
        let c = comp.compose_item(props(&[
            (names::BACKGROUND, BG),
            (names::IMAGE_OVERRIDE, bad),
            (names::IMAGE, IMAGE),
        ]));
        assert_eq!(c.image.as_ref(), Some(&base), "override {bad:#x}");
        let image = &c.layers[1];
        assert_eq!(image.entry.did, Did(IMAGE));
        assert_eq!(image.entry.property, Some(names::IMAGE));
        assert!(matches!(image.status, LayerStatus::Drawn { .. }));
        let (over, err) = image.superseded.as_ref().unwrap();
        assert_eq!(over.did, Did(bad));
        assert!(matches!(
            err,
            MaterialError::Missing { .. } | MaterialError::Decode { .. }
        ));
        assert!(c.skipped().next().is_none());
    }
    // A valid override still wins, with nothing superseded.
    let c = comp.compose_item(props(&[
        (names::BACKGROUND, BG),
        (names::IMAGE_OVERRIDE, BG),
        (names::IMAGE, IMAGE),
    ]));
    assert_eq!(c.layers[1].entry.did, Did(BG));
    assert!(c.layers[1].superseded.is_none());
    // Both absent: skipped, with the override's error kept.
    let c = comp.compose_item(props(&[
        (names::IMAGE_OVERRIDE, 0x4100_0099),
        (names::IMAGE, 0x4100_0098),
    ]));
    assert!(c.image.is_none());
    assert_eq!(c.skipped().count(), 1);
    assert!(c.layers[0].superseded.is_some());
}

#[test]
fn max_dimension_is_configurable_and_clamped() {
    let wide = 0x4100_0020;
    let mut src = Source::new();
    src.insert(wide, rgb(wide, 3000, 1, &vec![[1, 2, 3]; 3000]));
    let src = std::sync::Arc::new(src);
    let s1 = std::sync::Arc::clone(&src);
    let default = IconComposer::new(move |d: Did| Ok::<_, std::io::Error>(s1.get(&d.0).cloned()));
    assert_eq!(default.max_dimension(), MAX_LAYER_DIMENSION);
    assert!(matches!(
        default.layer(Did(wide)),
        Err(MaterialError::TooLarge { limit: 2048, .. })
    ));
    let s2 = std::sync::Arc::clone(&src);
    let big = IconComposer::new(move |d: Did| Ok::<_, std::io::Error>(s2.get(&d.0).cloned()))
        .with_max_dimension(4096);
    let img = big.compose(&IconStack::single(Did(wide))).image.unwrap();
    assert_eq!((img.width, img.height), (3000, 1));
    assert_eq!(
        decode_layer_with_limit(Did(wide), &src[&wide], 3000)
            .unwrap()
            .width,
        3000
    );
    assert!(decode_layer_with_limit(Did(wide), &src[&wide], 2999).is_err());
    let s3 = std::sync::Arc::clone(&src);
    let huge = IconComposer::new(move |d: Did| Ok::<_, std::io::Error>(s3.get(&d.0).cloned()))
        .with_max_dimension(u32::MAX);
    assert_eq!(huge.max_dimension(), MAX_LAYER_DIMENSION_CEILING);
    let s4 = std::sync::Arc::clone(&src);
    let tiny = IconComposer::new(move |d: Did| Ok::<_, std::io::Error>(s4.get(&d.0).cloned()))
        .with_max_dimension(0);
    assert_eq!(tiny.max_dimension(), 1);
    // compose_images with a limit.
    let layer = big.layer(Did(wide)).unwrap();
    assert!(compose_images(&[(LayerRole::Single, &layer)]).is_err());
    assert!(
        compose_images_with_limit(&[(LayerRole::Single, &layer)], 4096)
            .unwrap()
            .is_some()
    );
}

#[test]
fn cache_byte_budget() {
    let mut src = Source::new();
    for i in 0..4u32 {
        let did = 0x4100_0200 + i;
        src.insert(did, argb(did, 2, 2, &[[1, 2, 3, 255]; 4])); // 16 bytes each
    }
    let big = 0x4100_0300;
    src.insert(big, argb(big, 4, 4, &[[1, 2, 3, 255]; 16])); // 64 bytes
    let comp = composer(src).with_cache_limits(100, 40);
    for i in 0..4 {
        comp.layer(Did(0x4100_0200 + i)).unwrap();
        let (entries, bytes) = comp.cache_usage();
        assert!(bytes <= 40, "{bytes} bytes cached");
        assert_eq!(bytes, entries * 16);
    }
    // Two fit (32 bytes); the third empties the cache first.
    assert_eq!(comp.cache_usage(), (2, 32));
    // A layer above the whole budget is returned but never cached.
    let before = comp.cache_usage();
    assert_eq!(comp.layer(Did(big)).unwrap().pixels.len(), 64);
    assert_eq!(comp.cache_usage(), before);
}

#[test]
fn composer_over_a_record_source() {
    let mut src = Source::new();
    src.insert(BG, rgb(BG, 1, 1, &[[1, 2, 3]]));
    let source: Arc<dyn RecordSource> = Arc::new(crate::FnSource(move |d: Did| {
        Ok::<_, std::io::Error>(src.get(&d.0).cloned())
    }));
    let comp = composer_over(source);
    let c = comp.compose(&IconStack::single(Did(BG)));
    assert_eq!(c.image.unwrap().pixels, [1, 2, 3, 255]);
    assert_eq!(comp.cache_usage(), (1, 4));
    assert!(matches!(
        comp.layer(Did(0x4100_7777)),
        Err(MaterialError::Missing { .. })
    ));
}

#[test]
fn composer_is_send_and_sync_for_a_sync_source() {
    fn assert_send_sync<T: Send + Sync>() {}
    type FnSource = fn(Did) -> std::result::Result<Option<Vec<u8>>, std::io::Error>;
    assert_send_sync::<IconComposer<FnSource>>();
    let source: FnSource = |_| Ok(None);
    let comp = IconComposer::new(source);
    std::thread::scope(|s| {
        for _ in 0..2 {
            s.spawn(|| assert!(comp.layer(Did(0x4100_0001)).is_err()));
        }
    });
}

proptest::proptest! {
    /// Arbitrary bytes as a layer never panic, and composition still yields the other
    /// layers.
    #[test]
    fn prop_hostile_layer_bytes(bytes: Vec<u8>, w in 0u32..6, h in 0u32..6, fmt in proptest::sample::select(vec![20u32, 21, 28, 500, 0x3154_5844, 7])) {
        let mut src = Source::new();
        src.insert(BG, rgb(BG, 1, 1, &[[1, 2, 3]]));
        let mut rec = surface_record(IMAGE, w, h, fmt, &bytes);
        if bytes.len() % 3 == 0 { rec.truncate(rec.len() / 2); }
        src.insert(IMAGE, rec);
        let c = composer(src).compose_item(props(&[(names::BACKGROUND, BG), (names::IMAGE, IMAGE)]));
        let img = c.image.unwrap();
        proptest::prop_assert_eq!((img.width, img.height), (1, 1));
    }
}
