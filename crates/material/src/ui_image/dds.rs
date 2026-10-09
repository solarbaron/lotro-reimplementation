// SPDX-License-Identifier: GPL-3.0-or-later
//! DDS decoder (`skin-definition.md` §7.4, D18, D25, D26).

use tessera_formats::{BcFormat, decode_bcn};

use super::sink::Sink;
use super::{PixelLayout, Result, UiImage, UiImageError, argb, check_dims, check_file_len, widen};

const FORMAT: &str = "DDS";
const MAGIC: u32 = u32::from_le_bytes(*b"DDS ");
/// Pixel data starts here (magic + 124-byte header).
const DATA_OFFSET: usize = 128;

const DDPF_ALPHAPIXELS: u32 = 0x1;
const DDPF_ALPHA: u32 = 0x2;
const DDPF_FOURCC: u32 = 0x4;
const DDPF_RGB: u32 = 0x40;

fn u32_at(bytes: &[u8], offset: usize) -> Option<u32> {
    let b = bytes.get(offset..offset + 4)?;
    Some(u32::from_le_bytes([b[0], b[1], b[2], b[3]]))
}

fn truncated(needed: u64, available: usize) -> UiImageError {
    UiImageError::Truncated {
        format: FORMAT,
        needed,
        available: available as u64,
    }
}

/// Magic and size from the first bytes (at least 20), for the header probe.
pub(crate) fn probe(head: &[u8]) -> Result<(u32, u32)> {
    let magic = u32_at(head, 0).ok_or_else(|| truncated(4, head.len()))?;
    if magic != MAGIC {
        return Err(UiImageError::BadDdsMagic);
    }
    let height = u32_at(head, 12).ok_or_else(|| truncated(16, head.len()))?;
    let width = u32_at(head, 16).ok_or_else(|| truncated(20, head.len()))?;
    check_dims(width, height)?;
    Ok((width, height))
}

/// Chooses the pixel format by the accepted-format table of §7.4.
fn choose_layout(flags: u32, fourcc: u32, bits: u8, masks: [u32; 4]) -> Option<PixelLayout> {
    if flags & DDPF_FOURCC != 0 {
        return match &fourcc.to_le_bytes() {
            b"DXT1" => Some(PixelLayout::Dxt1),
            b"DXT2" => Some(PixelLayout::Dxt2),
            b"DXT3" => Some(PixelLayout::Dxt3),
            b"DXT4" => Some(PixelLayout::Dxt4),
            b"DXT5" => Some(PixelLayout::Dxt5),
            _ => None,
        };
    }
    let rgb = flags & DDPF_RGB != 0;
    let alpha = flags & (DDPF_ALPHAPIXELS | DDPF_ALPHA) != 0;
    let [r, g, b, a] = masks;
    match (rgb, alpha) {
        (true, false) => match (bits, r, g, b) {
            (24, 0xFF_0000, 0xFF00, 0xFF) => Some(PixelLayout::R8G8B8),
            (32, 0xFF_0000, 0xFF00, 0xFF) => Some(PixelLayout::X8R8G8B8),
            (16, 0xF800, 0x7E0, 0x1F) => Some(PixelLayout::R5G6B5),
            (16, 0x7C00, 0x3E0, 0x1F) => Some(PixelLayout::X1R5G5B5),
            (16, 0xF00, 0xF0, 0xF) => Some(PixelLayout::X4R4G4B4),
            (32, 0xFF, 0xFF00, 0xFF_0000) => Some(PixelLayout::X8B8G8R8),
            // The only row of this group that names an alpha mask.
            (24, 0, 0xFF, 0xFF00) if a == 0xFF_0000 => Some(PixelLayout::EngineB8G8R8),
            _ => None,
        },
        (true, true) => match (bits, r, g, b, a) {
            (32, 0xFF_0000, 0xFF00, 0xFF, 0xFF00_0000) => Some(PixelLayout::A8R8G8B8),
            (16, 0x7C00, 0x3E0, 0x1F, 0x8000) => Some(PixelLayout::A1R5G5B5),
            (16, 0xF00, 0xF0, 0xF, 0xF000) => Some(PixelLayout::A4R4G4B4),
            (32, 0x3FF, 0xF_FC00, 0x3FF0_0000, 0xC000_0000) => Some(PixelLayout::A2B10G10R10),
            (32, 0xFF, 0xFF00, 0xFF_0000, 0xFF00_0000) => Some(PixelLayout::A8B8G8R8),
            (32, 0x3FF0_0000, 0xF_FC00, 0x3FF, 0xC000_0000) => Some(PixelLayout::A2R10G10B10),
            (32, 0xFF00_0000, 0xFF_0000, 0xFF00, 0xFF) => Some(PixelLayout::EngineR8G8B8A8),
            _ => None,
        },
        (false, true) => (bits == 8 && a == 0xFF).then_some(PixelLayout::A8),
        (false, false) => None,
    }
}

/// Bits per pixel of a layout (the FourCC formats' replacement bit counts, §7.4 step 2).
fn layout_bits(layout: PixelLayout) -> u32 {
    match layout {
        PixelLayout::Dxt1 => 4,
        PixelLayout::Dxt2 | PixelLayout::Dxt3 | PixelLayout::Dxt4 | PixelLayout::Dxt5 => 8,
        PixelLayout::A8 => 8,
        PixelLayout::R8G8B8 | PixelLayout::EngineB8G8R8 => 24,
        PixelLayout::A1R5G5B5
        | PixelLayout::X1R5G5B5
        | PixelLayout::R5G6B5
        | PixelLayout::X4R4G4B4
        | PixelLayout::A4R4G4B4 => 16,
        _ => 32,
    }
}

/// Converts one stored little-endian pixel value to `0xAARRGGBB`.
fn convert(layout: PixelLayout, v: u32) -> u32 {
    let byte = |shift: u32| ((v >> shift) & 0xFF) as u8;
    match layout {
        PixelLayout::A8R8G8B8 => v,
        PixelLayout::R8G8B8 | PixelLayout::X8R8G8B8 => v | 0xFF00_0000,
        PixelLayout::A8 => argb(byte(0), 0, 0, 0),
        PixelLayout::R5G6B5 => argb(255, widen(v >> 11, 5), widen(v >> 5, 6), widen(v, 5)),
        PixelLayout::X1R5G5B5 => argb(255, widen(v >> 10, 5), widen(v >> 5, 5), widen(v, 5)),
        PixelLayout::A1R5G5B5 => argb(
            widen(v >> 15, 1),
            widen(v >> 10, 5),
            widen(v >> 5, 5),
            widen(v, 5),
        ),
        PixelLayout::X4R4G4B4 => argb(255, widen(v >> 8, 4), widen(v >> 4, 4), widen(v, 4)),
        PixelLayout::A4R4G4B4 => argb(
            widen(v >> 12, 4),
            widen(v >> 8, 4),
            widen(v >> 4, 4),
            widen(v, 4),
        ),
        PixelLayout::X8B8G8R8 => argb(255, byte(0), byte(8), byte(16)),
        PixelLayout::A8B8G8R8 => argb(byte(24), byte(0), byte(8), byte(16)),
        PixelLayout::A2B10G10R10 => argb(
            widen(v >> 30, 2),
            widen(v & 0x3FF, 10),
            widen((v >> 10) & 0x3FF, 10),
            widen((v >> 20) & 0x3FF, 10),
        ),
        PixelLayout::A2R10G10B10 => argb(
            widen(v >> 30, 2),
            widen((v >> 20) & 0x3FF, 10),
            widen((v >> 10) & 0x3FF, 10),
            widen(v & 0x3FF, 10),
        ),
        PixelLayout::EngineR8G8B8A8 => argb(byte(0), byte(24), byte(16), byte(8)),
        // OPEN (skin-definition.md §7.4): the channel order of this engine-specific format
        // is not stated; read by its name, R in the low byte and B in the high byte.
        PixelLayout::EngineB8G8R8 => argb(255, byte(0), byte(8), byte(16)),
        _ => 0,
    }
}

/// Decodes a whole DDS file (`skin-definition.md` §7.4): only the top-level image is
/// read; mip levels, cube faces and volume slices after it are ignored.
///
/// - The magic must be `DDS `; the header-size field is not checked.
/// - The pixel format is chosen by the accepted-format table (FourCC `DXT1`–`DXT5`, or the
///   RGB / alpha flags, the low byte of the bit count and the exact masks); any other
///   combination fails.
/// - Size = bit count × height × width ÷ 8 (64-bit, after the D4 limits, D18). DXT images
///   whose sides are not multiples of 4 fail (D18). For a FourCC format a non-zero
///   linear-size field must equal the size. Rows are tightly packed; the pitch field is
///   not used.
/// - DXT2 and DXT4 are decoded like DXT3 and DXT5 (D26); DXT1's 1-bit alpha becomes 0 or
///   255. Block decoding is `tessera_formats::decode_bcn`.
///
/// # Errors
///
/// [`UiImageError`] for every malformed, truncated, oversized or unsupported file.
pub fn decode_dds(bytes: &[u8]) -> Result<UiImage> {
    check_file_len(bytes)?;
    let magic = u32_at(bytes, 0).ok_or_else(|| truncated(4, bytes.len()))?;
    if magic != MAGIC {
        return Err(UiImageError::BadDdsMagic);
    }
    if bytes.len() < DATA_OFFSET {
        return Err(truncated(DATA_OFFSET as u64, bytes.len()));
    }
    let field = |o: usize| u32_at(bytes, o).unwrap_or(0);
    let (height, width, linear_size) = (field(12), field(16), field(20));
    let (flags, fourcc) = (field(80), field(84));
    let bits = field(88) as u8;
    let masks = [field(92), field(96), field(100), field(104)];
    check_dims(width, height)?;
    let layout =
        choose_layout(flags, fourcc, bits, masks).ok_or(UiImageError::UnsupportedDdsFormat {
            flags,
            fourcc,
            bits,
            masks,
        })?;
    let block = match layout {
        PixelLayout::Dxt1 => Some(BcFormat::Bc1),
        PixelLayout::Dxt2 | PixelLayout::Dxt3 => Some(BcFormat::Bc2),
        PixelLayout::Dxt4 | PixelLayout::Dxt5 => Some(BcFormat::Bc3),
        _ => None,
    };
    if block.is_some() && (width % 4 != 0 || height % 4 != 0) {
        return Err(UiImageError::DxtNotMultipleOf4 { width, height });
    }
    let size = u64::from(layout_bits(layout)) * u64::from(height) * u64::from(width) / 8;
    if block.is_some() && linear_size != 0 && u64::from(linear_size) != size {
        return Err(UiImageError::DdsLinearSize {
            stated: linear_size,
            computed: size,
        });
    }
    let available = (bytes.len() - DATA_OFFSET) as u64;
    if size == 0 || available < size {
        return Err(truncated(DATA_OFFSET as u64 + size, bytes.len()));
    }
    // size ≤ available < 64 MiB, so it fits usize.
    let data = bytes
        .get(DATA_OFFSET..DATA_OFFSET + size as usize)
        .ok_or_else(|| truncated(DATA_OFFSET as u64 + size, bytes.len()))?;
    let mut sink = Sink::new(width, height, true)?;
    match block {
        Some(bc) => decode_blocks(bc, width, data, &mut sink)?,
        None => decode_uncompressed(layout, width, data, &mut sink),
    }
    sink.finish(layout)
}

fn decode_uncompressed(layout: PixelLayout, width: u32, data: &[u8], sink: &mut Sink) {
    let bpp = layout_bits(layout) as usize / 8;
    let row_bytes = width as usize * bpp;
    let kept = sink.kept_columns();
    for (y, row) in data.chunks_exact(row_bytes).enumerate() {
        let Some(out_row) = sink.output_row(y as u64) else {
            break;
        };
        let out = sink.row_mut(out_row);
        for (dst, raw) in out.iter_mut().zip(row.chunks_exact(bpp).take(kept)) {
            let v = raw
                .iter()
                .rev()
                .fold(0u32, |acc, &b| (acc << 8) | u32::from(b));
            *dst = convert(layout, v);
        }
    }
}

/// Decodes the kept 4 × 4 block rows and columns, one block row at a time.
fn decode_blocks(bc: BcFormat, width: u32, data: &[u8], sink: &mut Sink) -> Result<()> {
    let blocks_x = width as usize / 4;
    let row_bytes = blocks_x * bc.block_bytes();
    let kept = sink.kept_columns();
    // Kept columns are a multiple of 4: the width is, and so is the crop size.
    let kept_blocks = kept.div_ceil(4);
    let kept_row_bytes = kept_blocks * bc.block_bytes();
    for (by, row) in data.chunks_exact(row_bytes).enumerate() {
        if sink.output_row(by as u64 * 4).is_none() {
            break;
        }
        let rgba = decode_bcn(
            bc,
            (kept_blocks * 4) as u32,
            4,
            row.get(..kept_row_bytes).unwrap_or_default(),
        )
        .ok_or_else(|| truncated(kept_row_bytes as u64, row.len()))?;
        let rgba_row = kept_blocks * 4 * 4;
        for ty in 0..4 {
            let Some(out_row) = sink.output_row(by as u64 * 4 + ty as u64) else {
                break;
            };
            let src = rgba.get(ty * rgba_row..(ty + 1) * rgba_row).unwrap_or(&[]);
            let out = sink.row_mut(out_row);
            for (dst, p) in out.iter_mut().zip(src.as_chunks::<4>().0) {
                *dst = argb(p[3], p[0], p[1], p[2]);
            }
        }
    }
    Ok(())
}
