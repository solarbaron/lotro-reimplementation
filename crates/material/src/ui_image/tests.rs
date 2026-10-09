// SPDX-License-Identifier: GPL-3.0-or-later
//! Unit tests for the internals; the decoders are tested through the public API in
//! `tests/ui_image.rs`.

use super::*;

#[test]
fn widen_replicates_bits() {
    assert_eq!(widen(0, 1), 0);
    assert_eq!(widen(1, 1), 255);
    assert_eq!(widen(1, 2), 0x55);
    assert_eq!(widen(2, 2), 0xAA);
    assert_eq!(widen(3, 2), 0xFF);
    assert_eq!(widen(0xA, 4), 0xAA);
    assert_eq!(widen(0b10000, 5), 0b1000_0100);
    assert_eq!(widen(31, 5), 255);
    assert_eq!(widen(0b100000, 6), 0b1000_0010);
    assert_eq!(widen(63, 6), 255);
    assert_eq!(widen(0x3FF, 10), 0xFF);
    assert_eq!(widen(0x203, 10), 0x80);
    assert_eq!(widen(0x12, 8), 0x12);
    // Only the low `bits` bits are used.
    assert_eq!(widen(0xFFE0, 5), 0);
}

#[test]
fn widen_is_monotonic_and_spans_full_range() {
    for bits in 1..=7u32 {
        let max = (1u32 << bits) - 1;
        assert_eq!(widen(0, bits), 0);
        assert_eq!(widen(max, bits), 255);
        for v in 1..=max {
            assert!(widen(v, bits) > widen(v - 1, bits), "bits {bits} v {v}");
        }
    }
}

#[test]
fn argb_packs_channels() {
    assert_eq!(argb(0x11, 0x22, 0x33, 0x44), 0x1122_3344);
}

#[test]
fn ui_image_new_checks_invariants() {
    assert!(UiImage::new(2, 2, 2, 2, PixelLayout::A8, vec![0; 4]).is_some());
    assert!(UiImage::new(2, 2, 2, 2, PixelLayout::A8, vec![0; 3]).is_none());
    assert!(UiImage::new(0, 0, 0, 0, PixelLayout::A8, vec![]).is_none());
    assert!(UiImage::new(2, 2, 1, 2, PixelLayout::A8, vec![0; 4]).is_none());
    assert!(
        UiImage::new(
            1,
            1,
            MAX_SOURCE_DIMENSION + 1,
            1,
            PixelLayout::A8,
            vec![0; 1]
        )
        .is_none()
    );
    let img = UiImage::new(1, 1, 5000, 1, PixelLayout::Jpeg, vec![0xFF00_0000]).unwrap();
    assert!(img.is_cropped());
    assert_eq!(img.tag(), SourceTag::NoAlpha);
}

#[test]
fn every_layout_has_the_d26_tag() {
    use PixelLayout as L;
    use SourceTag as T;
    let table = [
        (L::A8R8G8B8, T::Argb32),
        (L::A8, T::AlphaOnly),
        (L::R8G8B8, T::NoAlpha),
        (L::X8R8G8B8, T::NoAlpha),
        (L::R5G6B5, T::NoAlpha),
        (L::X1R5G5B5, T::NoAlpha),
        (L::X4R4G4B4, T::NoAlpha),
        (L::X8B8G8R8, T::NoAlpha),
        (L::EngineB8G8R8, T::NoAlpha),
        (L::Jpeg, T::NoAlpha),
        (L::Dxt1, T::Other),
        (L::Dxt2, T::Other),
        (L::Dxt3, T::Other),
        (L::Dxt4, T::Other),
        (L::Dxt5, T::Other),
        (L::A8B8G8R8, T::Other),
        (L::EngineR8G8B8A8, T::Other),
        (L::A4R4G4B4, T::Other),
        (L::A1R5G5B5, T::Other),
        (L::A2B10G10R10, T::Other),
        (L::A2R10G10B10, T::Other),
    ];
    for (layout, tag) in table {
        assert_eq!(layout.tag(), tag, "{layout:?}");
    }
}

#[test]
fn jpeg_decode_estimate_bounds() {
    use tessera_formats::surface::MAX_JPEG_DECODE_BYTES as LIMIT;
    // 4 (output) + 6 (coefficients) + 4 (crop) bytes per pixel up to the crop.
    assert_eq!(jpeg::decode_estimate(4096, 4096, 3), 224 * 1024 * 1024);
    // The square three-component limit stated in the decode_jpeg docs.
    assert!(jpeg::decode_estimate(4480, 4480, 3) <= LIMIT);
    assert!(jpeg::decode_estimate(4481, 4481, 3) > LIMIT);
    // Four components (CMYK/YCCK) stop earlier; one component near MAX_JPEG_PIXELS.
    assert!(jpeg::decode_estimate(4400, 4400, 4) > LIMIT);
    assert!(jpeg::decode_estimate(5792, 5792, 1) <= LIMIT);
    // The largest frame and component count stay finite (no overflow) and over the limit.
    assert!(jpeg::decode_estimate(u32::from(u16::MAX), u32::from(u16::MAX), 255) > LIMIT);
}

#[test]
fn jpeg_probe_walk_resynchronises() {
    let sof = [
        0xFF, 0xC0, 0x00, 0x0B, 0x08, 0x00, 0x10, 0x00, 0x20, 0x01, 0x01, 0x11, 0x00,
    ];
    let with = |prefix: &[u8]| {
        let mut v = vec![0xFF, 0xD8];
        v.extend_from_slice(prefix);
        v.extend_from_slice(&sof);
        jpeg::frame_size(&v)
    };
    assert_eq!(with(&[]), Some((32, 16)));
    // Junk bytes, a stuffed zero, TEM, RST0–RST7 and a repeated SOI are skipped.
    assert_eq!(with(&[0x00, 0x00, 0x7F]), Some((32, 16)));
    assert_eq!(
        with(&[0xFF, 0x00, 0xFF, 0x01, 0xFF, 0xD7, 0xFF, 0xD8]),
        Some((32, 16))
    );
    // A segment is skipped by its length, junk after it too.
    assert_eq!(
        with(&[0xFF, 0xE0, 0x00, 0x04, 0xC0, 0xC0, 0x55]),
        Some((32, 16))
    );
    // A scan or the end of the image before any frame: no size; bad length: no size.
    assert_eq!(with(&[0xFF, 0xDA, 0x00, 0x02]), None);
    assert_eq!(with(&[0xFF, 0xD9]), None);
    assert_eq!(with(&[0xFF, 0xE1, 0x00, 0x01]), None);
    // Truncated anywhere: no size, no panic.
    let mut full = vec![0xFF, 0xD8, 0x00];
    full.extend_from_slice(&sof);
    for cut in 0..full.len() {
        let _ = jpeg::frame_size(&full[..cut]);
    }
}
