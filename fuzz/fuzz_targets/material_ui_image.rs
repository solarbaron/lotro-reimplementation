// SPDX-License-Identifier: GPL-3.0-or-later
//! `tessera-material::ui_image` (`docs/specs/skin-definition.md` §7, decision 0015 §4.4,
//! §4.6, §7): the TGA, DDS and JPEG decoders, the dispatch by path, the header probe and
//! the render-surface conversion. Nothing may panic; a decoded image has
//! `width × height` pixels, sides of at most 4,096 (the crop), a source size within the
//! limits, and pixels that agree with its source-format tag; a TGA or DDS probe that gives
//! a size agrees with a successful decode of the same file.
//!
//! Input layout: one mode byte, then the payload. Modes 0–2 decode the payload as TGA, DDS
//! or JPEG; modes 3 and 4 build a TGA or DDS header from the first payload bytes (small or
//! just-over-the-crop sizes, an accepted depth or format; for DDS also the ignored header
//! fields) so the pixel paths are reached quickly; mode 5 probes and decodes under a path
//! picked from the next byte; mode 6 converts a render surface whose format and size come
//! from the next bytes.

#![no_main]

use std::path::Path;

use libfuzzer_sys::fuzz_target;
use tessera_formats::{Did, Surface, SurfaceFormat};
use tessera_material::ui_image::{
    ImageFileFormat, ImageOrigin, MAX_DECODED_DIMENSION, MAX_SOURCE_DIMENSION, SourceTag, UiImage,
    decode_dds, decode_file, decode_jpeg, decode_tga, from_surface, probe_header,
};

const PATHS: [(&str, ImageOrigin); 10] = [
    ("skin/a.tga", ImageOrigin::Skin),
    ("skin/a.DDS", ImageOrigin::Skin),
    ("skin/a.jpg", ImageOrigin::Skin),
    ("skin/a.iff", ImageOrigin::Skin),
    ("skin/a.jpeg", ImageOrigin::Skin),
    ("Plugins/x/a.Tga", ImageOrigin::Plugin),
    ("Plugins/x/a.jpg", ImageOrigin::Plugin),
    ("Plugins/x/a.png", ImageOrigin::Plugin),
    ("Plugins/x/a.bmp", ImageOrigin::Plugin),
    ("Plugins/x/a.dds", ImageOrigin::Plugin),
];

fn check(img: &UiImage) {
    let (w, h) = (img.width(), img.height());
    assert!((1..=MAX_DECODED_DIMENSION).contains(&w));
    assert!((1..=MAX_DECODED_DIMENSION).contains(&h));
    assert!(img.source_width() >= w && img.source_width() <= MAX_SOURCE_DIMENSION);
    assert!(img.source_height() >= h && img.source_height() <= MAX_SOURCE_DIMENSION);
    assert_eq!(img.pixels().len() as u64, u64::from(w) * u64::from(h));
    assert_eq!(
        img.is_cropped(),
        img.source_width() > MAX_DECODED_DIMENSION || img.source_height() > MAX_DECODED_DIMENSION
    );
    match img.tag() {
        SourceTag::AlphaOnly => assert!(img.pixels().iter().all(|p| p & 0x00FF_FFFF == 0)),
        SourceTag::NoAlpha => assert!(img.pixels().iter().all(|p| p >> 24 == 0xFF)),
        SourceTag::Argb32 | SourceTag::Other => {}
    }
    assert_eq!(img.to_rgba8().len(), img.pixels().len() * 4);
}

fn take<const N: usize>(data: &[u8]) -> ([u8; N], &[u8]) {
    let mut head = [0u8; N];
    let n = data.len().min(N);
    head[..n].copy_from_slice(&data[..n]);
    (head, &data[n..])
}

/// A side length from one byte: mostly small, sometimes just over the crop.
fn side(b: u8) -> u32 {
    match b {
        0xF0..=0xFF => MAX_DECODED_DIMENSION + u32::from(b & 0x0F) * 4,
        _ => u32::from(b % 64) + 1,
    }
}

fn tga_from(data: &[u8]) -> Vec<u8> {
    let ([t, w, h, d, desc, cm], rest) = take::<6>(data);
    let depth = [8u8, 15, 16, 24, 32][usize::from(d % 5)];
    let image_type = [2u8, 10, 1, 9, 3, 11][usize::from(t % 6)];
    let (w, h) = (side(w) as u16, side(h) as u16);
    let cmap = cm & 1 != 0;
    let entry = [15u8, 16, 24, 32][usize::from((cm >> 1) % 4)];
    let mut b = vec![0, u8::from(cmap), image_type, 0, 0];
    b.extend_from_slice(&(if cmap { u16::from(cm >> 3) + 1 } else { 0 }).to_le_bytes());
    b.push(if cmap { entry } else { 0 });
    b.extend_from_slice(&[0, 0, 0, 0]);
    b.extend_from_slice(&w.to_le_bytes());
    b.extend_from_slice(&h.to_le_bytes());
    b.push(depth);
    b.push(desc & 0x30);
    b.extend_from_slice(rest);
    b
}

fn dds_from(data: &[u8]) -> Vec<u8> {
    // DXT1–DXT5 and every uncompressed format of skin-definition.md §7.4.
    const FORMATS: [(u32, &[u8; 4], u32, [u32; 4]); 20] = [
        (0x4, b"DXT1", 0, [0; 4]),
        (0x4, b"DXT2", 0, [0; 4]),
        (0x4, b"DXT3", 0, [0; 4]),
        (0x4, b"DXT4", 0, [0; 4]),
        (0x4, b"DXT5", 0, [0; 4]),
        (0x40, &[0; 4], 24, [0xFF_0000, 0xFF00, 0xFF, 0]),
        (0x40, &[0; 4], 32, [0xFF_0000, 0xFF00, 0xFF, 0]),
        (0x40, &[0; 4], 16, [0xF800, 0x7E0, 0x1F, 0]),
        (0x40, &[0; 4], 16, [0xF00, 0xF0, 0xF, 0]),
        (0x41, &[0; 4], 32, [0xFF_0000, 0xFF00, 0xFF, 0xFF00_0000]),
        (0x41, &[0; 4], 16, [0x7C00, 0x3E0, 0x1F, 0x8000]),
        (
            0x41,
            &[0; 4],
            32,
            [0x3FF, 0xF_FC00, 0x3FF0_0000, 0xC000_0000],
        ),
        (0x41, &[0; 4], 32, [0xFF00_0000, 0xFF_0000, 0xFF00, 0xFF]),
        (0x02, &[0; 4], 8, [0, 0, 0, 0xFF]),
        (0x40, &[0; 4], 24, [0, 0xFF, 0xFF00, 0xFF_0000]),
        (
            0x41,
            &[0; 4],
            32,
            [0x3FF0_0000, 0xF_FC00, 0x3FF, 0xC000_0000],
        ),
        (0x40, &[0; 4], 16, [0x7C00, 0x3E0, 0x1F, 0]),
        (0x40, &[0; 4], 32, [0xFF, 0xFF00, 0xFF_0000, 0]),
        (0x41, &[0; 4], 16, [0xF00, 0xF0, 0xF, 0xF000]),
        (0x41, &[0; 4], 32, [0xFF, 0xFF00, 0xFF_0000, 0xFF00_0000]),
    ];
    let ([f, w, h, lin, extra], rest) = take::<5>(data);
    let (flags, fourcc, bits, masks) = FORMATS[usize::from(f % 20)];
    let (w, h) = (side(w), side(h));
    let mut b = vec![0u8; 128];
    b[0..4].copy_from_slice(b"DDS ");
    // Header size, header flags and mip count are ignored by the decoder (SP17): vary them.
    b[4..8].copy_from_slice(&(124 ^ u32::from(extra & 0x03)).to_le_bytes());
    b[8..12].copy_from_slice(&u32::from(extra >> 2 & 0x07).to_le_bytes());
    b[28..32].copy_from_slice(&u32::from(extra >> 5).to_le_bytes());
    b[12..16].copy_from_slice(&h.to_le_bytes());
    b[16..20].copy_from_slice(&w.to_le_bytes());
    // Linear size 0 (not checked) unless the format byte's top bit asks for the payload
    // byte, so the DXT block paths are reached instead of failing the linear-size check.
    let linear = if f & 0x80 != 0 { u32::from(lin) } else { 0 };
    b[20..24].copy_from_slice(&linear.to_le_bytes());
    b[80..84].copy_from_slice(&flags.to_le_bytes());
    b[84..88].copy_from_slice(fourcc);
    b[88..92].copy_from_slice(&bits.to_le_bytes());
    for (i, m) in masks.iter().enumerate() {
        b[92 + i * 4..96 + i * 4].copy_from_slice(&m.to_le_bytes());
    }
    b.extend_from_slice(rest);
    b
}

fn surface(data: &[u8]) {
    const FORMATS: [SurfaceFormat; 7] = [
        SurfaceFormat::R8G8B8,
        SurfaceFormat::A8R8G8B8,
        SurfaceFormat::A8,
        SurfaceFormat::Jpeg,
        SurfaceFormat::Dxt1,
        SurfaceFormat::Dxt3,
        SurfaceFormat::Dxt5,
    ];
    let ([f, w, h], rest) = take::<3>(data);
    let s = Surface {
        did: Did(0x4100_0000),
        category: 0,
        width: u32::from(w),
        height: u32::from(h),
        format: FORMATS[usize::from(f % 7)],
        data: rest,
    };
    if let Ok(img) = from_surface(&s) {
        check(&img);
    }
}

fuzz_target!(|data: &[u8]| {
    let Some((&mode, payload)) = data.split_first() else {
        return;
    };
    let result = match mode % 7 {
        0 => decode_tga(payload),
        1 => decode_dds(payload),
        2 => decode_jpeg(payload),
        3 => decode_tga(&tga_from(payload)),
        4 => decode_dds(&dds_from(payload)),
        5 => {
            let Some((&p, file)) = payload.split_first() else {
                return;
            };
            let (path, origin) = PATHS[usize::from(p) % PATHS.len()];
            let path = Path::new(path);
            let probe = probe_header(path, origin, file);
            let decoded = decode_file(path, origin, file);
            // TGA and DDS probes read the decoder's own header. A JPEG probe size is
            // advisory (an independent marker walk; the decoder checks its own frame).
            if let (Ok(probe), Ok(img)) = (&probe, &decoded)
                && let Some(dims) = probe.dimensions
                && probe.format != ImageFileFormat::Jpeg
            {
                assert_eq!(dims, (img.source_width(), img.source_height()));
            }
            // A dispatch refusal is the same for both.
            if let Err(e @ tessera_material::ui_image::UiImageError::RefusedPluginFormat { .. }) =
                &probe
            {
                assert_eq!(decoded.as_ref().err(), Some(e));
            }
            decoded
        }
        _ => {
            surface(payload);
            return;
        }
    };
    if let Ok(img) = result {
        check(&img);
    }
});
