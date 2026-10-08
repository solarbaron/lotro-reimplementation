// SPDX-License-Identifier: GPL-3.0-or-later
//! Synthetic tests of the UI image decoders (`docs/specs/skin-definition.md` §7, decision
//! 0015 §4). Every image is generated here.

use std::path::Path;

use tessera_formats::{Did, Surface, SurfaceFormat};
use tessera_material::ui_image::{
    ImageFileFormat, ImageOrigin, MAX_DECODED_DIMENSION, MAX_FILE_BYTES, PLACEHOLDER_SIZE,
    PROBE_LEN, PixelLayout, SourceTag, UiImageError, decode_dds, decode_file, decode_jpeg,
    decode_tga, format_for_path, from_surface, placeholder, probe_header,
};

// ---------------------------------------------------------------------------------------
// Builders
// ---------------------------------------------------------------------------------------

#[derive(Clone)]
struct Tga {
    id: Vec<u8>,
    cmap_type: u8,
    image_type: u8,
    cmap_first: u16,
    cmap_bits: u8,
    cmap: Vec<u8>,
    cmap_len: u16,
    width: u16,
    height: u16,
    depth: u8,
    descriptor: u8,
    data: Vec<u8>,
}

impl Tga {
    fn new(width: u16, height: u16, depth: u8, data: Vec<u8>) -> Self {
        Self {
            id: vec![],
            cmap_type: 0,
            image_type: 2,
            cmap_first: 0,
            cmap_bits: 0,
            cmap: vec![],
            cmap_len: 0,
            width,
            height,
            depth,
            descriptor: 0x20,
            data,
        }
    }

    fn bytes(&self) -> Vec<u8> {
        let mut b = vec![self.id.len() as u8, self.cmap_type, self.image_type];
        b.extend_from_slice(&self.cmap_first.to_le_bytes());
        b.extend_from_slice(&self.cmap_len.to_le_bytes());
        b.push(self.cmap_bits);
        b.extend_from_slice(&[0, 0, 0, 0]);
        b.extend_from_slice(&self.width.to_le_bytes());
        b.extend_from_slice(&self.height.to_le_bytes());
        b.push(self.depth);
        b.push(self.descriptor);
        b.extend_from_slice(&self.id);
        b.extend_from_slice(&self.cmap);
        b.extend_from_slice(&self.data);
        b
    }
}

/// A DDS file: header fields, then `data` at offset 128.
#[allow(clippy::too_many_arguments)] // mirrors the header fields one to one
fn dds(
    width: u32,
    height: u32,
    flags: u32,
    fourcc: &[u8; 4],
    bits: u32,
    masks: [u32; 4],
    linear: u32,
    data: &[u8],
) -> Vec<u8> {
    let mut b = vec![0u8; 128];
    b[0..4].copy_from_slice(b"DDS ");
    b[4..8].copy_from_slice(&124u32.to_le_bytes());
    b[12..16].copy_from_slice(&height.to_le_bytes());
    b[16..20].copy_from_slice(&width.to_le_bytes());
    b[20..24].copy_from_slice(&linear.to_le_bytes());
    b[76..80].copy_from_slice(&32u32.to_le_bytes());
    b[80..84].copy_from_slice(&flags.to_le_bytes());
    b[84..88].copy_from_slice(fourcc);
    b[88..92].copy_from_slice(&bits.to_le_bytes());
    for (i, m) in masks.iter().enumerate() {
        b[92 + i * 4..96 + i * 4].copy_from_slice(&m.to_le_bytes());
    }
    b.extend_from_slice(data);
    b
}

const RGB: u32 = 0x40;
const ALPHA: u32 = 0x1;
const FOURCC: u32 = 0x4;

fn dds_rgb(width: u32, height: u32, flags: u32, bits: u32, masks: [u32; 4], px: &[u8]) -> Vec<u8> {
    dds(width, height, flags, &[0; 4], bits, masks, 0, px)
}

/// A JPEG encoded by the `image` crate from a generated RGB picture.
fn jpeg(width: u32, height: u32, f: impl Fn(u32, u32) -> [u8; 3]) -> Vec<u8> {
    let mut rgb = Vec::new();
    for y in 0..height {
        for x in 0..width {
            rgb.extend_from_slice(&f(x, y));
        }
    }
    let mut out = Vec::new();
    image::codecs::jpeg::JpegEncoder::new_with_quality(&mut out, 100)
        .encode(&rgb, width, height, image::ExtendedColorType::Rgb8)
        .expect("encode");
    out
}

fn close(a: u32, b: u32, tol: i32) -> bool {
    a.to_be_bytes()
        .iter()
        .zip(b.to_be_bytes())
        .all(|(&x, y)| (i32::from(x) - i32::from(y)).abs() <= tol)
}

// ---------------------------------------------------------------------------------------
// Dispatch and probe
// ---------------------------------------------------------------------------------------

#[test]
fn dispatch_by_final_extension() {
    use ImageFileFormat as F;
    use ImageOrigin::{Plugin, Skin};
    let ok = |p: &str, o| format_for_path(Path::new(p), o).ok();
    assert_eq!(ok("a/b.TGA", Skin), Some(F::Tga));
    assert_eq!(ok("a/b.Jpg", Skin), Some(F::Jpeg));
    assert_eq!(ok("a/b.dds", Skin), Some(F::Dds));
    // D7: the final extension, not a substring of the path.
    assert_eq!(ok("my.jpg.art/bar.dds.tga", Skin), Some(F::Tga));
    assert_eq!(ok("a/b.iff", Skin), None); // D15
    assert_eq!(ok("a/b.jpeg", Skin), None);
    assert_eq!(ok("a/b", Skin), None);
    assert_eq!(ok("Plugins/X/icon.tga", Plugin), Some(F::Tga));
    assert_eq!(ok("Plugins/X/icon.JPG", Plugin), Some(F::Jpeg));
    assert!(matches!(
        format_for_path(Path::new("X/icon.dds"), Plugin),
        Err(UiImageError::UnsupportedFileType { .. })
    ));
    for ext in ["png", "PNG", "bmp"] {
        let path = format!("X/icon.{ext}");
        assert_eq!(
            format_for_path(Path::new(&path), Plugin),
            Err(UiImageError::RefusedPluginFormat {
                extension: ext.to_ascii_lowercase()
            })
        );
        // A skin .png is just an unknown type.
        assert!(matches!(
            format_for_path(Path::new(&path), Skin),
            Err(UiImageError::UnsupportedFileType { .. })
        ));
    }
    // decode_file refuses before looking at the bytes.
    assert!(matches!(
        decode_file(Path::new("X/a.png"), Plugin, b"\x89PNG\r\n\x1a\n"),
        Err(UiImageError::RefusedPluginFormat { .. })
    ));
}

#[test]
fn probe_tga_reads_size_and_checks_depth() {
    let file = Tga::new(300, 20, 32, vec![0; 300 * 20 * 4]).bytes();
    let p = probe_header(Path::new("a.tga"), ImageOrigin::Plugin, &file).unwrap();
    assert_eq!(p.format, ImageFileFormat::Tga);
    assert_eq!(p.dimensions, Some((300, 20)));
    // Only the first PROBE_LEN bytes are needed.
    let p2 = probe_header(Path::new("a.tga"), ImageOrigin::Plugin, &file[..PROBE_LEN]).unwrap();
    assert_eq!(p, p2);
    let bad = Tga::new(4, 4, 12, vec![]).bytes();
    assert_eq!(
        probe_header(Path::new("a.tga"), ImageOrigin::Plugin, &bad),
        Err(UiImageError::UnsupportedTgaDepth(12))
    );
    assert!(matches!(
        probe_header(Path::new("a.tga"), ImageOrigin::Plugin, &file[..10]),
        Err(UiImageError::Truncated { .. })
    ));
    assert_eq!(
        probe_header(Path::new("a.tga"), ImageOrigin::Plugin, &[]),
        Err(UiImageError::Empty)
    );
    let zero = Tga::new(0, 4, 32, vec![]).bytes();
    assert!(matches!(
        probe_header(Path::new("a.tga"), ImageOrigin::Plugin, &zero),
        Err(UiImageError::BadDimensions { .. })
    ));
}

#[test]
fn probe_jpeg_and_dds() {
    let j = jpeg(40, 24, |x, y| [x as u8, y as u8, 0]);
    let p = probe_header(Path::new("a.jpg"), ImageOrigin::Plugin, &j[..PROBE_LEN]).unwrap();
    assert_eq!(p.format, ImageFileFormat::Jpeg);
    // The encoder writes quantisation tables before the frame header, so 64 bytes may not
    // reach it; when they do, the size must be right.
    if let Some(d) = p.dimensions {
        assert_eq!(d, (40, 24));
    }
    // A minimal stream with the frame header first.
    let sof = [
        0xFF, 0xD8, 0xFF, 0xC0, 0x00, 0x0B, 0x08, 0x00, 0x10, 0x00, 0x20, 0x01, 0x01, 0x11, 0x00,
    ];
    let p = probe_header(Path::new("a.jpg"), ImageOrigin::Skin, &sof).unwrap();
    assert_eq!(p.dimensions, Some((32, 16)));
    assert_eq!(
        probe_header(Path::new("a.jpg"), ImageOrigin::Skin, b"GIF89a"),
        Err(UiImageError::NotJpeg)
    );
    let d = dds_rgb(
        8,
        2,
        RGB | ALPHA,
        32,
        [0xFF_0000, 0xFF00, 0xFF, 0xFF00_0000],
        &[],
    );
    let p = probe_header(Path::new("a.dds"), ImageOrigin::Skin, &d).unwrap();
    assert_eq!(p.dimensions, Some((8, 2)));
    assert_eq!(
        probe_header(
            Path::new("a.dds"),
            ImageOrigin::Skin,
            b"XXXX0000000000000000"
        ),
        Err(UiImageError::BadDdsMagic)
    );
}

// ---------------------------------------------------------------------------------------
// TGA
// ---------------------------------------------------------------------------------------

/// `skin-definition.md` §12.4.
#[test]
fn tga_worked_example_row_order() {
    let (p0, p1, p2, p3) = (
        [1u8, 2, 3, 4],
        [5u8, 6, 7, 8],
        [9u8, 10, 11, 12],
        [13u8, 14, 15, 16],
    );
    let data: Vec<u8> = [p0, p1, p2, p3].concat();
    let argb = |p: [u8; 4]| u32::from_le_bytes(p);
    let mut t = Tga::new(2, 2, 32, data);
    t.descriptor = 0x08;
    let img = decode_tga(&t.bytes()).unwrap();
    assert_eq!(
        img.pixels(),
        &[argb(p2), argb(p3), argb(p0), argb(p1)],
        "bottom-up"
    );
    assert_eq!(img.tag(), SourceTag::Argb32);
    assert_eq!(img.layout(), PixelLayout::A8R8G8B8);
    t.descriptor = 0x28;
    let img = decode_tga(&t.bytes()).unwrap();
    assert_eq!(img.pixels(), &[argb(p0), argb(p1), argb(p2), argb(p3)]);
    // Bit 4 (right-to-left) is ignored.
    t.descriptor = 0x38;
    assert_eq!(decode_tga(&t.bytes()).unwrap().pixels(), img.pixels());
}

#[test]
fn tga_depths_and_tags() {
    // 24-bit: B, G, R → opaque.
    let img = decode_tga(&Tga::new(1, 1, 24, vec![0x30, 0x20, 0x10]).bytes()).unwrap();
    assert_eq!(img.pixels(), &[0xFF10_2030]);
    assert_eq!(img.tag(), SourceTag::NoAlpha);
    // 16-bit A1R5G5B5: alpha bit set, R = 31, G = 0, B = 16.
    let v: u16 = 0x8000 | (31 << 10) | 16;
    let img = decode_tga(&Tga::new(1, 1, 16, v.to_le_bytes().to_vec()).bytes()).unwrap();
    assert_eq!(img.pixels(), &[0xFFFF_0084]);
    assert_eq!(img.tag(), SourceTag::Other);
    // Same value at depth 15: the top bit is ignored, opaque.
    let v: u16 = 31 << 10;
    let img = decode_tga(&Tga::new(1, 1, 15, v.to_le_bytes().to_vec()).bytes()).unwrap();
    assert_eq!(img.pixels(), &[0xFFFF_0000]);
    assert_eq!(img.tag(), SourceTag::NoAlpha);
    let img = decode_tga(&Tga::new(1, 1, 16, 0x7C00u16.to_le_bytes().to_vec()).bytes()).unwrap();
    assert_eq!(img.pixels(), &[0x00FF_0000], "16-bit with alpha bit clear");
    // 8-bit: black, value as alpha (D17).
    let img = decode_tga(&Tga::new(2, 1, 8, vec![0x80, 0xFF]).bytes()).unwrap();
    assert_eq!(img.pixels(), &[0x8000_0000, 0xFF00_0000]);
    assert_eq!(img.tag(), SourceTag::AlphaOnly);
    // Unsupported depths fail.
    for depth in [0u8, 1, 4, 12, 31, 48] {
        assert_eq!(
            decode_tga(&Tga::new(1, 1, depth, vec![0; 8]).bytes()),
            Err(UiImageError::UnsupportedTgaDepth(depth))
        );
    }
}

#[test]
fn tga_skips_id_and_ignores_type_and_trailing_bytes() {
    let mut t = Tga::new(1, 1, 24, vec![1, 2, 3, 0xEE, 0xEE]);
    t.id = b"hello".to_vec();
    t.image_type = 3; // not checked: read as uncompressed
    assert_eq!(decode_tga(&t.bytes()).unwrap().pixels(), &[0xFF03_0201]);
    t.image_type = 0;
    assert_eq!(decode_tga(&t.bytes()).unwrap().pixels(), &[0xFF03_0201]);
}

#[test]
fn tga_truncated_fails() {
    let t = Tga::new(2, 2, 32, vec![0; 15]);
    assert!(matches!(
        decode_tga(&t.bytes()),
        Err(UiImageError::Truncated { .. })
    ));
    assert_eq!(decode_tga(&[]), Err(UiImageError::Empty));
    assert!(matches!(
        decode_tga(&[0; 17]),
        Err(UiImageError::Truncated { .. })
    ));
}

#[test]
fn tga_rle_continuous_stream() {
    // 3 × 2, top-down, 24-bit: a repeat packet of 4 (crosses the scanline, D14), then 2
    // literals.
    let mut data = vec![0x83, 1, 2, 3];
    data.extend_from_slice(&[0x01, 4, 5, 6, 7, 8, 9]);
    let mut t = Tga::new(3, 2, 24, data);
    t.image_type = 10;
    let img = decode_tga(&t.bytes()).unwrap();
    let a = 0xFF03_0201;
    assert_eq!(img.pixels(), &[a, a, a, a, 0xFF06_0504, 0xFF09_0807]);
    // A packet running past the end of the image is accepted, the excess dropped.
    let mut t = Tga::new(2, 1, 8, vec![0xFF, 0x40]);
    t.image_type = 11;
    assert_eq!(
        decode_tga(&t.bytes()).unwrap().pixels(),
        &[0x4000_0000, 0x4000_0000]
    );
    // Truncated packet stream.
    let mut t = Tga::new(4, 4, 8, vec![0x81, 0x40]);
    t.image_type = 11;
    assert!(matches!(
        decode_tga(&t.bytes()),
        Err(UiImageError::Truncated { .. })
    ));
    let mut t = Tga::new(4, 1, 24, vec![0x03, 1, 2]);
    t.image_type = 10;
    assert!(matches!(
        decode_tga(&t.bytes()),
        Err(UiImageError::Truncated { .. })
    ));
}

#[test]
fn tga_rle_bottom_up_matches_uncompressed() {
    let (w, h) = (5u16, 3u16);
    let raw: Vec<u8> = (0..u32::from(w) * u32::from(h) * 4)
        .map(|i| (i * 7) as u8)
        .collect();
    let mut plain = Tga::new(w, h, 32, raw.clone());
    plain.descriptor = 0;
    // Literal packets of at most 128 pixels, deliberately crossing scanlines.
    let mut rle_data = Vec::new();
    for chunk in raw.chunks(4 * 4) {
        rle_data.push((chunk.len() / 4 - 1) as u8);
        rle_data.extend_from_slice(chunk);
    }
    let mut rle = Tga::new(w, h, 32, rle_data);
    rle.descriptor = 0;
    rle.image_type = 10;
    assert_eq!(
        decode_tga(&plain.bytes()).unwrap(),
        decode_tga(&rle.bytes()).unwrap()
    );
}

#[test]
fn tga_colour_map() {
    // 8-bit indices into a 24-bit map starting at entry 2: alpha of a 24-bit colour is 255.
    let mut t = Tga::new(2, 1, 8, vec![2, 3]);
    t.cmap_type = 1;
    t.image_type = 1;
    t.cmap_first = 2;
    t.cmap_len = 2;
    t.cmap_bits = 24;
    t.cmap = vec![1, 2, 3, 4, 5, 6];
    let img = decode_tga(&t.bytes()).unwrap();
    assert_eq!(img.pixels(), &[0xFF00_0000, 0xFF00_0000]);
    assert_eq!(img.tag(), SourceTag::AlphaOnly);
    // 32-bit map: black with the mapped alpha (D17).
    t.cmap_bits = 32;
    t.cmap = vec![1, 2, 3, 0x10, 4, 5, 6, 0x20];
    let img = decode_tga(&t.bytes()).unwrap();
    assert_eq!(img.pixels(), &[0x1000_0000, 0x2000_0000]);
    // Index outside the map.
    t.data = vec![1, 2];
    assert_eq!(
        decode_tga(&t.bytes()),
        Err(UiImageError::ColourMapIndex {
            index: 1,
            first: 2,
            end: 4
        })
    );
    // A colour-mapped 16-bit-index image at depth 32 takes the mapped colour.
    let mut t = Tga::new(1, 1, 16, 1u16.to_le_bytes().to_vec());
    t.cmap_type = 1;
    t.cmap_len = 2;
    t.cmap_bits = 32;
    t.cmap = vec![0, 0, 0, 0, 0x30, 0x20, 0x10, 0x80];
    t.depth = 16;
    let img = decode_tga(&t.bytes()).unwrap();
    assert_eq!(img.pixels(), &[0x8010_2030]);
    // Unsupported entry size.
    t.cmap_bits = 8;
    assert_eq!(
        decode_tga(&t.bytes()),
        Err(UiImageError::UnsupportedColourMapEntry(8))
    );
    // Map shorter than declared.
    let mut t = Tga::new(1, 1, 8, vec![]);
    t.cmap_type = 1;
    t.cmap_len = 300;
    t.cmap_bits = 32;
    assert!(matches!(
        decode_tga(&t.bytes()),
        Err(UiImageError::Truncated { .. })
    ));
}

#[test]
fn tga_crop_keeps_top_left_as_displayed() {
    // 4,100 × 2 bottom-up, 8-bit: file row 0 is the bottom row.
    let w = MAX_DECODED_DIMENSION as usize + 4;
    let mut data = vec![0u8; w * 2];
    for (x, b) in data[..w].iter_mut().enumerate() {
        *b = (x % 251) as u8; // bottom row
    }
    for (x, b) in data[w..].iter_mut().enumerate() {
        *b = ((x + 7) % 251) as u8; // top row
    }
    let mut t = Tga::new(w as u16, 2, 8, data);
    t.descriptor = 0;
    let img = decode_tga(&t.bytes()).unwrap();
    assert_eq!(img.width(), MAX_DECODED_DIMENSION);
    assert_eq!(img.height(), 2);
    assert_eq!(img.source_width(), w as u32);
    assert!(img.is_cropped());
    assert_eq!(img.pixel(0, 0), Some(7 << 24));
    assert_eq!(img.pixel(0, 1), Some(0));
    assert_eq!(img.pixel(4095, 1), Some(((4095 % 251) as u32) << 24));
    assert_eq!(img.pixels().len(), 4096 * 2);

    // 2 × 4,100 bottom-up: the kept rows are the last 4,096 file rows.
    let h = MAX_DECODED_DIMENSION as usize + 4;
    let data: Vec<u8> = (0..h * 2).map(|i| ((i / 2) % 256) as u8).collect();
    let mut t = Tga::new(2, h as u16, 8, data);
    t.descriptor = 0;
    let img = decode_tga(&t.bytes()).unwrap();
    assert_eq!((img.width(), img.height()), (2, MAX_DECODED_DIMENSION));
    // Displayed row 0 is file row h - 1.
    assert_eq!(img.pixel(0, 0), Some((((h - 1) % 256) as u32) << 24));
    assert_eq!(img.pixel(1, 4095), Some((((h - 4096) % 256) as u32) << 24));

    // Run-length crop over both axes with a repeat packet spanning rows.
    let (w, h) = (4100u16, 4100u16);
    let mut data = Vec::new();
    let mut left = u64::from(w) * u64::from(h);
    while left > 0 {
        let n = left.min(128);
        data.push(0x80 | (n - 1) as u8);
        data.push(0x55);
        left -= n;
    }
    let mut t = Tga::new(w, h, 8, data);
    t.image_type = 11;
    let img = decode_tga(&t.bytes()).unwrap();
    assert_eq!((img.width(), img.height()), (4096, 4096));
    assert!(img.pixels().iter().all(|&p| p == 0x5500_0000));
}

#[test]
fn file_limit() {
    let big = vec![0u8; MAX_FILE_BYTES + 1];
    for r in [decode_tga(&big), decode_dds(&big), decode_jpeg(&big)] {
        assert!(matches!(r, Err(UiImageError::FileTooLarge { .. })));
    }
}

// ---------------------------------------------------------------------------------------
// DDS
// ---------------------------------------------------------------------------------------

/// (flags, bits, masks, stored LE value, expected ARGB, layout)
type DdsCase = (u32, u32, [u32; 4], u32, u32, PixelLayout);

#[test]
fn dds_uncompressed_formats() {
    // (flags, bits, masks, stored LE value, expected ARGB, layout)
    let cases: &[DdsCase] = &[
        (
            RGB | ALPHA,
            32,
            [0xFF_0000, 0xFF00, 0xFF, 0xFF00_0000],
            0x8011_2233,
            0x8011_2233,
            PixelLayout::A8R8G8B8,
        ),
        (
            RGB,
            32,
            [0xFF_0000, 0xFF00, 0xFF, 0],
            0x0011_2233,
            0xFF11_2233,
            PixelLayout::X8R8G8B8,
        ),
        (
            RGB,
            32,
            [0xFF, 0xFF00, 0xFF_0000, 0],
            0x0033_2211,
            0xFF11_2233,
            PixelLayout::X8B8G8R8,
        ),
        (
            RGB | ALPHA,
            32,
            [0xFF, 0xFF00, 0xFF_0000, 0xFF00_0000],
            0x8033_2211,
            0x8011_2233,
            PixelLayout::A8B8G8R8,
        ),
        (
            RGB | ALPHA,
            32,
            [0xFF00_0000, 0xFF_0000, 0xFF00, 0xFF],
            0x1122_3380,
            0x8011_2233,
            PixelLayout::EngineR8G8B8A8,
        ),
        (
            RGB,
            16,
            [0xF800, 0x7E0, 0x1F, 0],
            0xF800,
            0xFFFF_0000,
            PixelLayout::R5G6B5,
        ),
        (
            RGB,
            16,
            [0xF800, 0x7E0, 0x1F, 0],
            0x07E0,
            0xFF00_FF00,
            PixelLayout::R5G6B5,
        ),
        (
            RGB,
            16,
            [0x7C00, 0x3E0, 0x1F, 0],
            0x801F,
            0xFF00_00FF,
            PixelLayout::X1R5G5B5,
        ),
        (
            RGB | ALPHA,
            16,
            [0x7C00, 0x3E0, 0x1F, 0x8000],
            0x801F,
            0xFF00_00FF,
            PixelLayout::A1R5G5B5,
        ),
        (
            RGB,
            16,
            [0xF00, 0xF0, 0xF, 0],
            0xF0A5,
            0xFF00_AA55,
            PixelLayout::X4R4G4B4,
        ),
        (
            RGB | ALPHA,
            16,
            [0xF00, 0xF0, 0xF, 0xF000],
            0x80A5,
            0x8800_AA55,
            PixelLayout::A4R4G4B4,
        ),
        (
            RGB | ALPHA,
            32,
            [0x3FF, 0xF_FC00, 0x3FF0_0000, 0xC000_0000],
            (2 << 30) | (0x100 << 20) | (0x3FF << 10) | 0x004,
            0xAA01_FF40,
            PixelLayout::A2B10G10R10,
        ),
        (
            RGB | ALPHA,
            32,
            [0x3FF0_0000, 0xF_FC00, 0x3FF, 0xC000_0000],
            (1 << 30) | (0x004 << 20) | (0x3FF << 10) | 0x100,
            0x5501_FF40,
            PixelLayout::A2R10G10B10,
        ),
    ];
    for &(flags, bits, masks, stored, expected, layout) in cases {
        let px = &stored.to_le_bytes()[..bits as usize / 8];
        let file = dds_rgb(1, 1, flags, bits, masks, px);
        let img = decode_dds(&file).unwrap_or_else(|e| panic!("{layout:?}: {e}"));
        assert_eq!(img.layout(), layout);
        assert_eq!(
            img.pixels(),
            &[expected],
            "{layout:?}: {:#010x}",
            img.pixels()[0]
        );
    }
    // 24-bit formats.
    let img = dds_rgb(
        1,
        1,
        RGB,
        24,
        [0xFF_0000, 0xFF00, 0xFF, 0],
        &[0x33, 0x22, 0x11],
    );
    let img = decode_dds(&img).unwrap();
    assert_eq!(
        (img.layout(), img.pixels()),
        (PixelLayout::R8G8B8, &[0xFF11_2233][..])
    );
    let img = dds_rgb(
        1,
        1,
        RGB,
        24,
        [0, 0xFF, 0xFF00, 0xFF_0000],
        &[0x11, 0x22, 0x33],
    );
    let img = decode_dds(&img).unwrap();
    assert_eq!(img.layout(), PixelLayout::EngineB8G8R8);
    assert_eq!(img.tag(), SourceTag::NoAlpha);
    // A8 (alpha only, without the RGB flag; the alpha flag may be either bit).
    for flags in [0x1, 0x2] {
        let img = decode_dds(&dds_rgb(2, 1, flags, 8, [0, 0, 0, 0xFF], &[0x40, 0xC0])).unwrap();
        assert_eq!(img.pixels(), &[0x4000_0000, 0xC000_0000]);
        assert_eq!(img.tag(), SourceTag::AlphaOnly);
    }
}

#[test]
fn dds_format_selection_rules() {
    // Only the low byte of the bit count is used.
    let f = dds_rgb(
        1,
        1,
        RGB | ALPHA,
        0x100 | 32,
        [0xFF_0000, 0xFF00, 0xFF, 0xFF00_0000],
        &[0; 4],
    );
    assert_eq!(decode_dds(&f).unwrap().layout(), PixelLayout::A8R8G8B8);
    // RGB without alpha does not compare the alpha mask.
    let f = dds_rgb(1, 1, RGB, 32, [0xFF_0000, 0xFF00, 0xFF, 0x1234], &[0; 4]);
    assert_eq!(decode_dds(&f).unwrap().layout(), PixelLayout::X8R8G8B8);
    // Masks are compared exactly.
    let f = dds_rgb(1, 1, RGB, 32, [0xFF_0000, 0xFF00, 0xFE, 0], &[0; 4]);
    assert!(matches!(
        decode_dds(&f),
        Err(UiImageError::UnsupportedDdsFormat { .. })
    ));
    // Neither RGB nor alpha.
    let f = dds_rgb(1, 1, 0, 32, [0xFF_0000, 0xFF00, 0xFF, 0], &[0; 4]);
    assert!(matches!(
        decode_dds(&f),
        Err(UiImageError::UnsupportedDdsFormat { .. })
    ));
    // FourCC decides alone: an unknown one fails even with valid RGB fields.
    let f = dds(
        4,
        4,
        FOURCC | RGB,
        b"ATI2",
        32,
        [0xFF_0000, 0xFF00, 0xFF, 0],
        0,
        &[0; 64],
    );
    assert!(matches!(
        decode_dds(&f),
        Err(UiImageError::UnsupportedDdsFormat { .. })
    ));
    // Bad magic, short header.
    let mut f = dds_rgb(1, 1, RGB, 32, [0xFF_0000, 0xFF00, 0xFF, 0], &[0; 4]);
    f[0] = b'X';
    assert_eq!(decode_dds(&f), Err(UiImageError::BadDdsMagic));
    assert!(matches!(
        decode_dds(b"DDS \0\0"),
        Err(UiImageError::Truncated { .. })
    ));
    // D4 size limits.
    for (w, h) in [(0, 4), (4, 0), (16_388, 4)] {
        let f = dds(w, h, FOURCC, b"DXT1", 0, [0; 4], 0, &[]);
        assert!(matches!(
            decode_dds(&f),
            Err(UiImageError::BadDimensions { .. })
        ));
    }
}

/// `skin-definition.md` §12.6.
#[test]
fn dds_sizes_worked_examples() {
    // 8 × 8 DXT1: 32 bytes; linear size 0 or 32.
    let block = [0u8; 32];
    for linear in [0, 32] {
        let f = dds(8, 8, FOURCC, b"DXT1", 0, [0; 4], linear, &block);
        assert!(decode_dds(&f).is_ok(), "linear {linear}");
    }
    let f = dds(8, 8, FOURCC, b"DXT1", 0, [0; 4], 64, &[0; 64]);
    assert_eq!(
        decode_dds(&f),
        Err(UiImageError::DdsLinearSize {
            stated: 64,
            computed: 32
        })
    );
    // 8 × 8 DXT5: 64 bytes.
    let f = dds(8, 8, FOURCC, b"DXT5", 0, [0; 4], 0, &[0; 63]);
    assert!(matches!(
        decode_dds(&f),
        Err(UiImageError::Truncated { .. })
    ));
    let f = dds(8, 8, FOURCC, b"DXT5", 0, [0; 4], 0, &[0; 64]);
    assert!(decode_dds(&f).is_ok());
    // 6 × 6 and 1 × 1 DXT1 fail (D18).
    for (w, h, linear) in [(6, 6, 32), (6, 6, 0), (1, 1, 0)] {
        let f = dds(w, h, FOURCC, b"DXT1", 0, [0; 4], linear, &[0; 64]);
        assert_eq!(
            decode_dds(&f),
            Err(UiImageError::DxtNotMultipleOf4 {
                width: w,
                height: h
            })
        );
    }
    // 3 × 5 A8R8G8B8: 60 bytes, pitch field ignored, rows of 12 bytes.
    let data: Vec<u8> = (0..60).collect();
    let f = dds(
        3,
        5,
        RGB | ALPHA,
        &[0; 4],
        32,
        [0xFF_0000, 0xFF00, 0xFF, 0xFF00_0000],
        999,
        &data,
    );
    let img = decode_dds(&f).unwrap();
    assert_eq!((img.width(), img.height()), (3, 5));
    assert_eq!(img.pixel(0, 1), Some(u32::from_le_bytes([12, 13, 14, 15])));
    assert_eq!(img.pixel(2, 4), Some(u32::from_le_bytes([56, 57, 58, 59])));
    // Mip levels after the top level are ignored.
    let mut longer = data.clone();
    longer.extend_from_slice(&[0xAA; 32]);
    let f2 = dds(
        3,
        5,
        RGB | ALPHA,
        &[0; 4],
        32,
        [0xFF_0000, 0xFF00, 0xFF, 0xFF00_0000],
        0,
        &longer,
    );
    assert_eq!(decode_dds(&f2).unwrap(), img);
}

/// A DXT1 block of one colour (c0 > c1, all indices 0).
fn dxt1_block(c565: u16) -> [u8; 8] {
    let c0 = c565.to_le_bytes();
    [c0[0], c0[1], 0, 0, 0, 0, 0, 0]
}

#[test]
fn dds_dxt_decoding_and_tags() {
    // 8 × 4 DXT1: left block red, right block a punch-through transparent block.
    let mut data = dxt1_block(0xF800).to_vec();
    // c0 = c1 = 0 selects the 3-colour mode; index 3 everywhere = transparent black.
    data.extend_from_slice(&[0, 0, 0, 0, 0xFF, 0xFF, 0xFF, 0xFF]);
    let img = decode_dds(&dds(8, 4, FOURCC, b"DXT1", 0, [0; 4], 0, &data)).unwrap();
    assert_eq!(img.tag(), SourceTag::Other);
    assert_eq!(img.pixel(0, 0), Some(0xFFFF_0000));
    assert_eq!(img.pixel(3, 3), Some(0xFFFF_0000));
    assert_eq!(img.pixel(4, 0), Some(0x0000_0000));
    // DXT2 and DXT4 decode like DXT3 and DXT5 (D26).
    let mut b16 = [0u8; 16];
    b16[..8].fill(0x77); // DXT3 explicit alpha 7 → 0x77; DXT5 a0 = a1 = 0x77
    b16[8..].copy_from_slice(&dxt1_block(0x001F));
    for (a, b) in [(b"DXT2", b"DXT3"), (b"DXT4", b"DXT5")] {
        let x = decode_dds(&dds(4, 4, FOURCC, a, 0, [0; 4], 0, &b16)).unwrap();
        let y = decode_dds(&dds(4, 4, FOURCC, b, 0, [0; 4], 0, &b16)).unwrap();
        assert_eq!(x.pixels(), y.pixels());
        assert_eq!(x.tag(), SourceTag::Other);
    }
    let y = decode_dds(&dds(4, 4, FOURCC, b"DXT3", 0, [0; 4], 0, &b16)).unwrap();
    assert_eq!(y.pixel(1, 1), Some(0x7700_00FF));
}

#[test]
fn dds_crop() {
    // 4,104 × 8 DXT1: block column k has colour k (as 565).
    let (w, h) = (4104u32, 8u32);
    let bx = w / 4;
    let mut data = Vec::new();
    for _ in 0..h / 4 {
        for k in 0..bx {
            data.extend_from_slice(&dxt1_block(0x8000 | (k as u16 & 0x7FFF)));
        }
    }
    let img = decode_dds(&dds(w, h, FOURCC, b"DXT1", 0, [0; 4], 0, &data)).unwrap();
    assert_eq!((img.width(), img.height()), (4096, 8));
    assert_eq!(img.source_width(), 4104);
    let full = tessera_formats::decode_bcn(tessera_formats::BcFormat::Bc1, w, h, &data).unwrap();
    for (x, y) in [(0u32, 0u32), (4095, 7), (1234, 5)] {
        let i = ((y * w + x) * 4) as usize;
        let p = &full[i..i + 4];
        let expect = u32::from_be_bytes([p[3], p[0], p[1], p[2]]);
        assert_eq!(img.pixel(x, y), Some(expect), "({x}, {y})");
    }
    // 2 × 4,100 A8: the top 4,096 rows are kept.
    let data: Vec<u8> = (0..2 * 4100).map(|i| (i / 2 % 256) as u8).collect();
    let img = decode_dds(&dds_rgb(2, 4100, ALPHA, 8, [0, 0, 0, 0xFF], &data)).unwrap();
    assert_eq!((img.width(), img.height()), (2, 4096));
    assert_eq!(img.pixel(0, 4095), Some((4095 % 256) << 24));
}

// ---------------------------------------------------------------------------------------
// JPEG
// ---------------------------------------------------------------------------------------

#[test]
fn jpeg_decodes_opaque() {
    let f = |x: u32, y: u32| [(x * 8) as u8, (y * 8) as u8, 0x80];
    let file = jpeg(32, 16, f);
    let img = decode_file(Path::new("a/b.jpg"), ImageOrigin::Skin, &file).unwrap();
    assert_eq!((img.width(), img.height()), (32, 16));
    assert_eq!(img.layout(), PixelLayout::Jpeg);
    assert_eq!(img.tag(), SourceTag::NoAlpha);
    assert!(img.pixels().iter().all(|&p| p >> 24 == 0xFF));
    for (x, y) in [(0, 0), (31, 15), (10, 5)] {
        let [r, g, b] = f(x, y);
        let expect = u32::from_be_bytes([0xFF, r, g, b]);
        assert!(
            close(img.pixel(x, y).unwrap(), expect, 12),
            "({x}, {y}): {:#010x} vs {expect:#010x}",
            img.pixel(x, y).unwrap()
        );
    }
    // Same through the plugin path.
    let p = decode_file(Path::new("Plugins/A/b.JPG"), ImageOrigin::Plugin, &file).unwrap();
    assert_eq!(p, img);
}

#[test]
fn jpeg_greyscale() {
    let mut out = Vec::new();
    let grey: Vec<u8> = (0..64).map(|i| (i * 4) as u8).collect();
    image::codecs::jpeg::JpegEncoder::new_with_quality(&mut out, 100)
        .encode(&grey, 8, 8, image::ExtendedColorType::L8)
        .unwrap();
    let img = decode_jpeg(&out).unwrap();
    let p = img.pixel(3, 2).unwrap();
    let [a, r, g, b] = p.to_be_bytes();
    assert_eq!(a, 255);
    assert_eq!((r, r), (g, b));
}

#[test]
fn jpeg_crop() {
    let file = jpeg(
        4100,
        8,
        |x, _| if x < 4096 { [255, 0, 0] } else { [0, 0, 255] },
    );
    let img = decode_jpeg(&file).unwrap();
    assert_eq!((img.width(), img.height()), (4096, 8));
    assert_eq!((img.source_width(), img.source_height()), (4100, 8));
    assert!(close(img.pixel(100, 3).unwrap(), 0xFFFF_0000, 12));
}

#[test]
fn jpeg_rejects_bad_input() {
    assert_eq!(decode_jpeg(&[]), Err(UiImageError::Empty));
    assert_eq!(decode_jpeg(b"not a jpeg"), Err(UiImageError::NotJpeg));
    // A frame header that claims more pixels than the stream can justify.
    let file = jpeg(8, 8, |_, _| [1, 2, 3]);
    let sof = file
        .windows(2)
        .position(|w| w == [0xFF, 0xC0])
        .expect("baseline frame");
    let mut big = file.clone();
    big[sof + 5..sof + 7].copy_from_slice(&16_000u16.to_be_bytes());
    big[sof + 7..sof + 9].copy_from_slice(&16_000u16.to_be_bytes());
    assert!(decode_jpeg(&big).is_err());
    // Zero height.
    let mut zero = file.clone();
    zero[sof + 5..sof + 7].copy_from_slice(&0u16.to_be_bytes());
    assert!(decode_jpeg(&zero).is_err());
    // Every truncation is an error or an image, never a panic.
    for cut in 0..file.len() {
        let _ = decode_jpeg(&file[..cut]);
    }
}

// ---------------------------------------------------------------------------------------
// Render surfaces
// ---------------------------------------------------------------------------------------

fn surface(format: SurfaceFormat, width: u32, height: u32, data: &[u8]) -> Surface<'_> {
    Surface {
        did: Did(0x4100_0001),
        category: 0,
        width,
        height,
        format,
        data,
    }
}

#[test]
fn surfaces_take_the_canonical_form() {
    let s = surface(SurfaceFormat::A8R8G8B8, 1, 1, &[0x30, 0x20, 0x10, 0x80]);
    let img = from_surface(&s).unwrap();
    assert_eq!(
        (img.pixels(), img.tag()),
        (&[0x8010_2030][..], SourceTag::Argb32)
    );
    let s = surface(SurfaceFormat::R8G8B8, 1, 1, &[0x30, 0x20, 0x10]);
    let img = from_surface(&s).unwrap();
    assert_eq!(
        (img.pixels(), img.tag()),
        (&[0xFF10_2030][..], SourceTag::NoAlpha)
    );
    let s = surface(SurfaceFormat::A8, 2, 1, &[0x40, 0xFF]);
    let img = from_surface(&s).unwrap();
    assert_eq!(
        (img.pixels(), img.tag()),
        (&[0x4000_0000, 0xFF00_0000][..], SourceTag::AlphaOnly)
    );
    let block = dxt1_block(0xF800);
    let s = surface(SurfaceFormat::Dxt1, 4, 4, &block);
    let img = from_surface(&s).unwrap();
    assert_eq!(img.tag(), SourceTag::Other);
    assert_eq!(img.pixel(2, 2), Some(0xFFFF_0000));
    let j = jpeg(8, 8, |_, _| [0, 255, 0]);
    let s = surface(SurfaceFormat::Jpeg, 0, 0, &j);
    let img = from_surface(&s).unwrap();
    assert_eq!((img.width(), img.tag()), (8, SourceTag::NoAlpha));
    // A length mismatch is an error.
    let s = surface(SurfaceFormat::A8R8G8B8, 2, 2, &[0; 4]);
    assert!(matches!(from_surface(&s), Err(UiImageError::Surface(_))));
}

#[test]
fn to_rgba8_reorders() {
    let img = decode_tga(&Tga::new(1, 1, 32, vec![0x30, 0x20, 0x10, 0x80]).bytes()).unwrap();
    assert_eq!(img.to_rgba8(), vec![0x10, 0x20, 0x30, 0x80]);
    assert_eq!(img.byte_size(), 4);
}

// ---------------------------------------------------------------------------------------
// skin-definition.md §11 invariants, as written
// ---------------------------------------------------------------------------------------

/// SP12: the placeholder of §6.5.
#[test]
fn sp12_placeholder() {
    let img = placeholder();
    assert_eq!((img.width(), img.height()), (32, 32));
    assert_eq!(PLACEHOLDER_SIZE, 32);
    assert!(!img.is_cropped());
    assert_eq!(img.tag(), SourceTag::Argb32);
    let slate = 0xFF4C_4C66; // (76, 76, 102, 255)
    let white = 0xFFFF_FFFF;
    for (x, y) in [(0, 0), (28, 28), (28, 0), (0, 28), (13, 7)] {
        assert_eq!(img.pixel(x, y), Some(slate), "({x}, {y})");
    }
    for (x, y) in [(29, 0), (0, 29), (31, 31), (30, 12), (12, 31)] {
        assert_eq!(img.pixel(x, y), Some(white), "({x}, {y})");
    }
    assert_eq!(&img.to_rgba8()[..4], &[76, 76, 102, 255]);
    let whites = img.pixels().iter().filter(|&&p| p == white).count();
    assert_eq!(whites, 32 * 32 - 29 * 29);
}

/// SP17, every case as written.
#[test]
fn sp17_dds_sizes() {
    // 8 × 8 DXT1 with linear size 0 or 32 and 32 data bytes decodes.
    for linear in [0, 32] {
        let f = dds(8, 8, FOURCC, b"DXT1", 0, [0; 4], linear, &[0; 32]);
        assert!(decode_dds(&f).is_ok(), "linear {linear}");
    }
    // Linear size 31 fails.
    let f = dds(8, 8, FOURCC, b"DXT1", 0, [0; 4], 31, &[0; 32]);
    assert_eq!(
        decode_dds(&f),
        Err(UiImageError::DdsLinearSize {
            stated: 31,
            computed: 32
        })
    );
    // 31 data bytes fail.
    let f = dds(8, 8, FOURCC, b"DXT1", 0, [0; 4], 0, &[0; 31]);
    assert_eq!(
        decode_dds(&f),
        Err(UiImageError::Truncated {
            format: "DDS",
            needed: 160,
            available: 159
        })
    );
    // 6 × 6 DXT1 fails (D18).
    let f = dds(6, 6, FOURCC, b"DXT1", 0, [0; 4], 0, &[0; 32]);
    assert_eq!(
        decode_dds(&f),
        Err(UiImageError::DxtNotMultipleOf4 {
            width: 6,
            height: 6
        })
    );
    // 4 × 4 A8R8G8B8 with 64 data bytes decodes whatever its header-size (offset 4), flags
    // (8), pitch (20) and mip-count (28) fields hold.
    let data: Vec<u8> = (0..64).collect();
    let masks = [0xFF_0000, 0xFF00, 0xFF, 0xFF00_0000];
    let base = dds(4, 4, RGB | ALPHA, &[0; 4], 32, masks, 0, &data);
    let expect = decode_dds(&base).unwrap();
    assert_eq!(expect.pixel(1, 0), Some(u32::from_le_bytes([4, 5, 6, 7])));
    for (offset, value) in [
        (4usize, 0u32),
        (4, 0xFFFF_FFFF),
        (4, 7),
        (8, 0),
        (8, 0xFFFF_FFFF),
        (8, 0x000A_1007),
        (20, 0xFFFF_FFFF),
        (20, 3),
        (28, 0),
        (28, 13),
        (28, 0xFFFF_FFFF),
    ] {
        let mut f = base.clone();
        f[offset..offset + 4].copy_from_slice(&value.to_le_bytes());
        assert_eq!(
            decode_dds(&f).as_ref(),
            Ok(&expect),
            "offset {offset} = {value:#x}"
        );
    }
    // Bit-count field 0x120 (low byte 0x20) is treated as 32 bits.
    let f = dds(4, 4, RGB | ALPHA, &[0; 4], 0x120, masks, 0, &data);
    assert_eq!(decode_dds(&f).as_ref(), Ok(&expect));
}

/// SP25, every case as written.
#[test]
fn sp25_crop() {
    // 32-bit TGA of 5,000 × 8, top-left origin → 4,096 × 8, same top-left pixels.
    let (w, h) = (5000usize, 8usize);
    let value = |x: usize, y: usize| (x as u32) | ((y as u32) << 16) | 0x8000_0000;
    let mut data = Vec::with_capacity(w * h * 4);
    for y in 0..h {
        for x in 0..w {
            data.extend_from_slice(&value(x, y).to_le_bytes());
        }
    }
    let t = Tga::new(w as u16, h as u16, 32, data);
    let img = decode_tga(&t.bytes()).unwrap();
    assert_eq!((img.width(), img.height()), (4096, 8));
    for y in 0..h {
        for x in (0..4096).step_by(97).chain([4095]) {
            assert_eq!(
                img.pixel(x as u32, y as u32),
                Some(value(x, y)),
                "({x}, {y})"
            );
        }
    }
    // Bottom-up 32-bit TGA of 8 × 5,000 → 8 × 4,096 holding displayed rows 0–4,095 (file
    // rows 4,999 down to 904).
    let (w, h) = (8usize, 5000usize);
    let mut data = Vec::with_capacity(w * h * 4);
    for file_row in 0..h {
        for x in 0..w {
            data.extend_from_slice(&value(x, file_row).to_le_bytes());
        }
    }
    let mut t = Tga::new(w as u16, h as u16, 32, data);
    t.descriptor = 0;
    let img = decode_tga(&t.bytes()).unwrap();
    assert_eq!((img.width(), img.height()), (8, 4096));
    assert_eq!(img.pixel(0, 0), Some(value(0, 4999)));
    assert_eq!(img.pixel(7, 4095), Some(value(7, 904)));
    for y in (0..4096).step_by(311) {
        assert_eq!(img.pixel(3, y), Some(value(3, 4999 - y as usize)));
    }
    // A 4,096 × 4,096 image is not changed (8-bit: a 32-bit one exceeds the file limit).
    let n = 4096usize;
    let data: Vec<u8> = (0..n * n).map(|i| ((i % n) ^ (i / n)) as u8).collect();
    let img = decode_tga(&Tga::new(n as u16, n as u16, 8, data.clone()).bytes()).unwrap();
    assert_eq!((img.width(), img.height()), (4096, 4096));
    assert!(!img.is_cropped());
    assert!(
        img.pixels()
            .iter()
            .zip(&data)
            .all(|(&p, &v)| p == u32::from(v) << 24)
    );
    // A TGA of 16,385 × 1 fails (D4).
    let t = Tga::new(16_385, 1, 32, vec![0; 16_385 * 4]);
    assert_eq!(
        decode_tga(&t.bytes()),
        Err(UiImageError::BadDimensions {
            width: 16_385,
            height: 1,
            max: 16_384
        })
    );
    assert!(matches!(
        probe_header(Path::new("a.tga"), ImageOrigin::Skin, &t.bytes()),
        Err(UiImageError::BadDimensions { .. })
    ));
}

/// (flags, bits, masks, stored LE value, expected ARGB, tag)
type TagCase = (u32, u32, [u32; 4], u32, u32, SourceTag);

/// SP26: the literal pixel examples and the tag per format.
#[test]
fn sp26_tags_and_widening() {
    let one = |flags, bits: u32, masks, stored: u32| {
        let px = &stored.to_le_bytes()[..bits as usize / 8];
        decode_dds(&dds_rgb(1, 1, flags, bits, masks, px)).unwrap()
    };
    let cases: &[TagCase] = &[
        // R5G6B5 0xFC01 → 0xFFFF8208.
        (
            RGB,
            16,
            [0xF800, 0x7E0, 0x1F, 0],
            0xFC01,
            0xFFFF_8208,
            SourceTag::NoAlpha,
        ),
        // A4R4G4B4 0x8F3C → 0x88FF33CC.
        (
            RGB | ALPHA,
            16,
            [0xF00, 0xF0, 0xF, 0xF000],
            0x8F3C,
            0x88FF_33CC,
            SourceTag::Other,
        ),
        // A1R5G5B5 0x7FFF → 0x00FFFFFF and 0x8000 → 0xFF000000.
        (
            RGB | ALPHA,
            16,
            [0x7C00, 0x3E0, 0x1F, 0x8000],
            0x7FFF,
            0x00FF_FFFF,
            SourceTag::Other,
        ),
        (
            RGB | ALPHA,
            16,
            [0x7C00, 0x3E0, 0x1F, 0x8000],
            0x8000,
            0xFF00_0000,
            SourceTag::Other,
        ),
        // A2R10G10B10 0x7FF80001 → 0x55FF8000.
        (
            RGB | ALPHA,
            32,
            [0x3FF0_0000, 0xF_FC00, 0x3FF, 0xC000_0000],
            0x7FF8_0001,
            0x55FF_8000,
            SourceTag::Other,
        ),
        // A8B8G8R8 0x44332211 → 0x44112233.
        (
            RGB | ALPHA,
            32,
            [0xFF, 0xFF00, 0xFF_0000, 0xFF00_0000],
            0x4433_2211,
            0x4411_2233,
            SourceTag::Other,
        ),
        // X8R8G8B8 0x00123456 → 0xFF123456.
        (
            RGB,
            32,
            [0xFF_0000, 0xFF00, 0xFF, 0],
            0x0012_3456,
            0xFF12_3456,
            SourceTag::NoAlpha,
        ),
        // The remaining tags of the list.
        (
            RGB | ALPHA,
            32,
            [0xFF_0000, 0xFF00, 0xFF, 0xFF00_0000],
            0x1234_5678,
            0x1234_5678,
            SourceTag::Argb32,
        ),
        (
            RGB,
            16,
            [0x7C00, 0x3E0, 0x1F, 0],
            0x7FFF,
            0xFFFF_FFFF,
            SourceTag::NoAlpha,
        ),
        (
            RGB,
            16,
            [0xF00, 0xF0, 0xF, 0],
            0x0FFF,
            0xFFFF_FFFF,
            SourceTag::NoAlpha,
        ),
        (
            RGB,
            32,
            [0xFF, 0xFF00, 0xFF_0000, 0],
            0x0033_2211,
            0xFF11_2233,
            SourceTag::NoAlpha,
        ),
        (
            RGB | ALPHA,
            32,
            [0xFF00_0000, 0xFF_0000, 0xFF00, 0xFF],
            0x1122_3380,
            0x8011_2233,
            SourceTag::Other,
        ),
        (
            RGB | ALPHA,
            32,
            [0x3FF, 0xF_FC00, 0x3FF0_0000, 0xC000_0000],
            0xC000_0000,
            0xFF00_0000,
            SourceTag::Other,
        ),
        (
            0x2,
            8,
            [0, 0, 0, 0xFF],
            0x80,
            0x8000_0000,
            SourceTag::AlphaOnly,
        ),
    ];
    for &(flags, bits, masks, stored, expected, tag) in cases {
        let img = one(flags, bits, masks, stored);
        assert_eq!(
            (img.pixels()[0], img.tag()),
            (expected, tag),
            "{:?} {stored:#x}",
            img.layout()
        );
    }
    // 24-bit DDS formats: R8G8B8 and the engine-specific B8G8R8 → no alpha.
    for masks in [[0xFF_0000, 0xFF00, 0xFF, 0], [0, 0xFF, 0xFF00, 0xFF_0000]] {
        let img = decode_dds(&dds_rgb(1, 1, RGB, 24, masks, &[1, 2, 3])).unwrap();
        assert_eq!(img.tag(), SourceTag::NoAlpha);
    }
    // TGA: 32 → ARGB32, 24 and 15 → no alpha, 16 → other, 8-bit value 0x80 → 0x80000000.
    for (depth, bytes, tag) in [
        (32u8, 4usize, SourceTag::Argb32),
        (24, 3, SourceTag::NoAlpha),
        (15, 2, SourceTag::NoAlpha),
        (16, 2, SourceTag::Other),
        (8, 1, SourceTag::AlphaOnly),
    ] {
        let img = decode_tga(&Tga::new(1, 1, depth, vec![0x80; bytes]).bytes()).unwrap();
        assert_eq!(img.tag(), tag, "depth {depth}");
    }
    let img = decode_tga(&Tga::new(1, 1, 8, vec![0x80]).bytes()).unwrap();
    assert_eq!(img.pixels(), &[0x8000_0000]);
    // JPEG → no alpha; DXT1–DXT5 → other.
    assert_eq!(
        decode_jpeg(&jpeg(8, 8, |_, _| [1, 2, 3])).unwrap().tag(),
        SourceTag::NoAlpha
    );
    for (cc, len) in [
        (b"DXT1", 8),
        (b"DXT2", 16),
        (b"DXT3", 16),
        (b"DXT4", 16),
        (b"DXT5", 16),
    ] {
        let img = decode_dds(&dds(4, 4, FOURCC, cc, 0, [0; 4], 0, &vec![0; len])).unwrap();
        assert_eq!(img.tag(), SourceTag::Other);
    }
}

// ---------------------------------------------------------------------------------------
// Review regressions
// ---------------------------------------------------------------------------------------

/// Inserts `junk` before the first segment whose marker is `marker`.
fn insert_before_marker(file: &[u8], marker: u8, junk: &[u8]) -> Vec<u8> {
    let at = file
        .windows(2)
        .position(|w| w == [0xFF, marker])
        .expect("marker");
    let mut out = file[..at].to_vec();
    out.extend_from_slice(junk);
    out.extend_from_slice(&file[at..]);
    out
}

/// Bytes that are not markers between header segments, and standalone markers before the
/// frame header, are skipped (libjpeg resynchronises with a warning; §7.3 OPEN (b)).
#[test]
fn jpeg_tolerates_junk_between_segments() {
    let file = jpeg(16, 16, |x, y| [(x * 16) as u8, (y * 16) as u8, 0x40]);
    let clean = decode_jpeg(&file).unwrap();
    let junk = insert_before_marker(&file, 0xDB, &[0x00, 0x00]);
    assert_eq!(decode_jpeg(&junk).unwrap(), clean);
    // Junk right before the frame header, inside the probed bytes.
    let mut short = vec![0xFF, 0xD8, 0xFF, 0xD0, 0x12, 0x34, 0xFF, 0x01, 0x00];
    short.extend_from_slice(&[
        0xFF, 0xC0, 0x00, 0x0B, 0x08, 0x00, 0x10, 0x00, 0x20, 0x01, 0x01, 0x11, 0x00,
    ]);
    let p = probe_header(Path::new("a.jpg"), ImageOrigin::Plugin, &short).unwrap();
    assert_eq!(p.dimensions, Some((32, 16)));
    // The probe of a junk-carrying real file never fails and, when it finds a size, finds
    // the decoded one.
    let p = probe_header(Path::new("a.jpg"), ImageOrigin::Plugin, &junk).unwrap();
    if let Some(d) = p.dimensions {
        assert_eq!(d, (16, 16));
    }
}

/// Patches every SOF0 size of `file` and appends `pad` zero bytes after the end of the
/// image (counted by the pixel budget, never read by the decoder).
fn with_frame_size(file: &[u8], w: u16, h: u16, pad: usize) -> Vec<u8> {
    let sof = file
        .windows(2)
        .position(|x| x == [0xFF, 0xC0])
        .expect("baseline frame");
    let mut out = file.to_vec();
    out[sof + 5..sof + 7].copy_from_slice(&h.to_be_bytes());
    out[sof + 7..sof + 9].copy_from_slice(&w.to_be_bytes());
    out.resize(out.len() + pad, 0);
    out
}

/// The decode budget counts a baseline frame as progressive (the decoder keeps full-frame
/// coefficients when the first scan does not hold every component) and adds the crop
/// buffer: a 4,600 × 4,600 three-component baseline frame (about 21 Mi pixels, well inside
/// the pixel budget for its length) fails before any pixel buffer is allocated.
#[test]
fn jpeg_budget_counts_baseline_as_progressive() {
    let base = jpeg(8, 8, |_, _| [1, 2, 3]);
    let big = with_frame_size(&base, 4600, 4600, 32 * 1024);
    assert_eq!(
        decode_jpeg(&big),
        Err(UiImageError::TooLarge {
            width: 4600,
            height: 4600
        })
    );
}

/// The budget uses the frame the decoder reads, not a frame found by a separate walk. A
/// length-6 APP0 segment is followed by a fake one-component frame header that the
/// decoder skips as junk; the real frame (three components, 4,600 × 4,600) decides.
#[test]
fn jpeg_budget_uses_the_decoded_frame() {
    let base = jpeg(8, 8, |_, _| [1, 2, 3]);
    let real = with_frame_size(&base, 4600, 4600, 32 * 1024);
    let mut fake = vec![0xFF, 0xD8, 0xFF, 0xE0, 0x00, 0x06, b'A', b'B', b'C', b'D'];
    fake.extend_from_slice(&[
        0xFF, 0xC0, 0x00, 0x0B, 0x08, 0x11, 0xF8, 0x11, 0xF8, 0x01, 0x01, 0x11, 0x00,
    ]);
    fake.extend_from_slice(&real[2..]);
    assert_eq!(
        decode_jpeg(&fake),
        Err(UiImageError::TooLarge {
            width: 4600,
            height: 4600
        })
    );
}

/// A header alone fails as truncated before the output buffer is allocated, with the
/// smallest size the header allows.
#[test]
fn tga_header_only_fails_before_allocating() {
    let t = Tga::new(16_384, 16_384, 32, vec![0]);
    assert_eq!(
        decode_tga(&t.bytes()),
        Err(UiImageError::Truncated {
            format: "TGA",
            needed: 16_384 * 16_384 * 4,
            available: 1
        })
    );
    // Run-length: at least one 5-byte repeat packet per 128 pixels.
    let mut t = Tga::new(16_384, 16_384, 32, vec![0]);
    t.image_type = 10;
    assert_eq!(
        decode_tga(&t.bytes()),
        Err(UiImageError::Truncated {
            format: "TGA",
            needed: 16_384 * 16_384 / 128 * 5,
            available: 1
        })
    );
}

/// A colour-mapped TGA whose final run-length packet runs past the image: the pixels past
/// the end are not looked up.
#[test]
fn tga_rle_ignores_pixels_past_the_end() {
    let mut t = Tga::new(2, 1, 8, vec![0x02, 0, 0, 7]);
    t.cmap_type = 1;
    t.image_type = 9;
    t.cmap_len = 1;
    t.cmap_bits = 32;
    t.cmap = vec![0, 0, 0, 0x90];
    assert_eq!(
        decode_tga(&t.bytes()).unwrap().pixels(),
        &[0x9000_0000, 0x9000_0000]
    );
    // The rest of the packet need not be present either.
    t.data = vec![0x02, 0, 0];
    assert_eq!(
        decode_tga(&t.bytes()).unwrap().pixels(),
        &[0x9000_0000, 0x9000_0000]
    );
    // A run packet past the end with an out-of-map value inside the image still fails.
    t.data = vec![0x81, 3];
    assert_eq!(
        decode_tga(&t.bytes()),
        Err(UiImageError::ColourMapIndex {
            index: 3,
            first: 0,
            end: 1
        })
    );
}

/// One cropped colour-mapped picture stored uncompressed and run-length encoded gives the
/// same result: indices in the cropped-away part are never looked up in either.
#[test]
fn tga_cropped_colour_map_same_raw_and_rle() {
    let w = 4100usize;
    let mut pixels = vec![0u8; w];
    pixels[4098] = 5; // outside the 1-entry map, in a cropped column
    let mut raw = Tga::new(w as u16, 1, 8, pixels.clone());
    raw.cmap_type = 1;
    raw.image_type = 1;
    raw.cmap_len = 1;
    raw.cmap_bits = 32;
    raw.cmap = vec![0, 0, 0, 0x40];
    let mut rle = raw.clone();
    rle.image_type = 9;
    rle.data = Vec::new();
    for chunk in pixels.chunks(128) {
        rle.data.push((chunk.len() - 1) as u8);
        rle.data.extend_from_slice(chunk);
    }
    let a = decode_tga(&raw.bytes()).unwrap();
    let b = decode_tga(&rle.bytes()).unwrap();
    assert_eq!(a, b);
    assert_eq!(a.width(), 4096);
    assert!(a.pixels().iter().all(|&p| p == 0x4000_0000));
    // Bottom-up and taller than the crop: the dropped file rows are not looked up either.
    let h = 4100usize;
    let mut pixels = vec![0u8; h];
    pixels[1] = 9; // file row 1 = displayed row 4,098, cropped away
    let mut raw = Tga::new(1, h as u16, 8, pixels.clone());
    raw.descriptor = 0;
    raw.cmap_type = 1;
    raw.image_type = 1;
    raw.cmap_len = 1;
    raw.cmap_bits = 32;
    raw.cmap = vec![0, 0, 0, 0x40];
    let mut rle = raw.clone();
    rle.image_type = 9;
    rle.data = Vec::new();
    for chunk in pixels.chunks(128) {
        rle.data.push((chunk.len() - 1) as u8);
        rle.data.extend_from_slice(chunk);
    }
    // Also as a run packet over the dropped rows.
    let mut runs = raw.clone();
    runs.image_type = 9;
    // File rows 0–3 are displayed rows 4,099–4,096: dropped.
    runs.data = vec![0x00, 0, 0x00, 9, 0x81, 9];
    let mut left = h - 4;
    while left > 0 {
        let n = left.min(128);
        runs.data.extend_from_slice(&[0x80 | (n - 1) as u8, 0]);
        left -= n;
    }
    let a = decode_tga(&raw.bytes()).unwrap();
    assert_eq!(decode_tga(&rle.bytes()).unwrap(), a);
    assert_eq!(decode_tga(&runs.bytes()).unwrap(), a);
    assert_eq!(a.height(), 4096);
}
