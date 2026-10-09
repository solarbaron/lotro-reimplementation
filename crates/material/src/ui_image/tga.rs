// SPDX-License-Identifier: GPL-3.0-or-later
//! TGA decoder (`skin-definition.md` §7.2, D14, D17, D25, D26).

use super::sink::Sink;
use super::{
    PixelLayout, Result, UiImage, UiImageError, alloc_pixels, argb, check_dims, check_file_len,
    widen,
};

const FORMAT: &str = "TGA";
const HEADER_LEN: usize = 18;

/// The fields of the 18-byte header that decoding uses.
#[derive(Debug, Clone, Copy)]
pub(crate) struct Header {
    id_len: u8,
    cmap_type: u8,
    image_type: u8,
    cmap_first: u16,
    cmap_len: u16,
    cmap_entry_bits: u8,
    pub(crate) width: u32,
    pub(crate) height: u32,
    depth: u8,
    descriptor: u8,
}

impl Header {
    /// Parses and checks the header: pixel depth, D4 size and colour-map entry size.
    pub(crate) fn parse(bytes: &[u8]) -> Result<Self> {
        let h: &[u8; HEADER_LEN] = bytes
            .get(..HEADER_LEN)
            .and_then(|s| s.try_into().ok())
            .ok_or(UiImageError::Truncated {
                format: FORMAT,
                needed: HEADER_LEN as u64,
                available: bytes.len() as u64,
            })?;
        let u16_at = |o: usize| u16::from_le_bytes([h[o], h[o + 1]]);
        let header = Self {
            id_len: h[0],
            cmap_type: h[1],
            image_type: h[2],
            cmap_first: u16_at(3),
            cmap_len: u16_at(5),
            cmap_entry_bits: h[7],
            width: u32::from(u16_at(12)),
            height: u32::from(u16_at(14)),
            depth: h[16],
            descriptor: h[17],
        };
        if !matches!(header.depth, 8 | 15 | 16 | 24 | 32) {
            return Err(UiImageError::UnsupportedTgaDepth(header.depth));
        }
        check_dims(header.width, header.height)?;
        if header.cmap_type != 0 && !matches!(header.cmap_entry_bits, 15 | 16 | 24 | 32) {
            return Err(UiImageError::UnsupportedColourMapEntry(
                header.cmap_entry_bits,
            ));
        }
        Ok(header)
    }

    /// Bytes per stored pixel value (or colour-map index).
    fn pixel_bytes(&self) -> usize {
        usize::from(self.depth).div_ceil(8)
    }

    fn layout(&self) -> PixelLayout {
        match self.depth {
            32 => PixelLayout::A8R8G8B8,
            24 => PixelLayout::R8G8B8,
            16 => PixelLayout::A1R5G5B5,
            15 => PixelLayout::X1R5G5B5,
            _ => PixelLayout::A8,
        }
    }
}

/// Reads a little-endian unsigned value of 1–4 bytes.
fn le_value(raw: &[u8]) -> u32 {
    raw.iter()
        .rev()
        .fold(0u32, |acc, &b| (acc << 8) | u32::from(b))
}

/// Converts a stored value of `bits` (32, 24, 16 or 15; 8 is handled by the caller) to
/// `0xAARRGGBB` by the format table of §7.2.
fn convert_colour(value: u32, bits: u8) -> u32 {
    match bits {
        32 => value,
        24 => value | 0xFF00_0000,
        16 | 15 => {
            let a = if bits == 16 {
                widen(value >> 15, 1)
            } else {
                255
            };
            argb(
                a,
                widen(value >> 10, 5),
                widen(value >> 5, 5),
                widen(value, 5),
            )
        }
        _ => argb(0, 0, 0, 0),
    }
}

/// How stored pixel values become canonical pixels.
enum Palette {
    /// No colour map: the value is the pixel.
    Direct(u8),
    /// Colour-mapped: the value indexes `entries` from `first`.
    Mapped {
        first: u32,
        entries: Vec<u32>,
        depth: u8,
    },
}

impl Palette {
    fn convert(&self, raw: &[u8]) -> Result<u32> {
        let value = le_value(raw);
        match self {
            Self::Direct(8) => Ok(argb(value as u8, 0, 0, 0)),
            Self::Direct(depth) => Ok(convert_colour(value, *depth)),
            Self::Mapped {
                first,
                entries,
                depth,
            } => {
                let end = first.saturating_add(entries.len() as u32);
                let colour = value
                    .checked_sub(*first)
                    .and_then(|i| entries.get(i as usize))
                    .copied()
                    .ok_or(UiImageError::ColourMapIndex {
                        index: value,
                        first: *first,
                        end,
                    })?;
                Ok(match depth {
                    // D17: black with the alpha of the mapped colour.
                    8 => colour & 0xFF00_0000,
                    // Formats without alpha are opaque whatever the map holds (§7.2 table).
                    24 | 15 => colour | 0xFF00_0000,
                    _ => colour,
                })
            }
        }
    }
}

/// Decodes a whole TGA file (`skin-definition.md` §7.2).
///
/// - Pixel depths 32 (A8R8G8B8, tag *ARGB32*), 24 (R8G8B8, *no alpha*), 16 (A1R5G5B5,
///   *other*), 15 (X1R5G5B5, *no alpha*) and 8 (A8: black with the value as alpha,
///   *alpha-only*, D17); any other depth fails.
/// - Image types 9, 10 and 11 are run-length encoded, decoded as one continuous stream
///   (D14); any other type is read as uncompressed pixels.
/// - Rows are bottom-up unless descriptor bit 5 is set; bit 4 is ignored.
/// - A colour map (colour-map type non-zero; entries of 15, 16, 24 or 32 bits) is applied
///   to every decoded pixel value before the format conversion. For depth 8 the pixel is
///   black with the mapped colour's alpha (D17); for depths 24 and 15 it is opaque; a
///   decoded value outside the map fails (provisional, `skin-definition.md` §7.2 OPEN).
/// - Only kept pixels are decoded: pixels that the crop of D25 drops are never converted
///   or checked against the colour map, in uncompressed and run-length files alike, so
///   both storages of one picture give the same result.
/// - Data after the last pixel is ignored, including the part of a final run-length packet
///   that runs past it; data that ends before it fails, and so does a file shorter than
///   the smallest size its header allows (checked before any pixel buffer is allocated).
///
/// # Errors
///
/// [`UiImageError`] for every malformed, truncated, oversized or unsupported file.
pub fn decode_tga(bytes: &[u8]) -> Result<UiImage> {
    check_file_len(bytes)?;
    let header = Header::parse(bytes)?;
    let mut pos = HEADER_LEN + usize::from(header.id_len);
    let palette = if header.cmap_type != 0 {
        let entry_bytes = usize::from(header.cmap_entry_bits).div_ceil(8);
        let map_len = usize::from(header.cmap_len) * entry_bytes;
        let map = bytes
            .get(pos..pos + map_len)
            .ok_or(UiImageError::Truncated {
                format: FORMAT,
                needed: (pos + map_len) as u64,
                available: bytes.len() as u64,
            })?;
        let mut entries = alloc_pixels(usize::from(header.cmap_len))?;
        for (e, raw) in entries.iter_mut().zip(map.chunks_exact(entry_bytes)) {
            *e = convert_colour(le_value(raw), header.cmap_entry_bits);
        }
        pos += map_len;
        Palette::Mapped {
            first: u32::from(header.cmap_first),
            entries,
            depth: header.depth,
        }
    } else {
        Palette::Direct(header.depth)
    };
    let data = bytes.get(pos..).unwrap_or_default();
    let rle = matches!(header.image_type, 9..=11);
    // Refuse data too short for the image before the output is allocated, so a header
    // alone costs no pixel buffer: uncompressed pixels need their full size, and a
    // run-length stream at least one shortest packet (a repeat packet: header byte and one
    // value) per 128 pixels.
    let pixels = u64::from(header.width) * u64::from(header.height);
    let bpp = header.pixel_bytes() as u64;
    let needed = if rle {
        pixels.div_ceil(128) * (1 + bpp)
    } else {
        pixels * bpp
    };
    if (data.len() as u64) < needed {
        return Err(truncated(needed, data.len()));
    }
    let top_down = header.descriptor & 0x20 != 0;
    let mut sink = Sink::new(header.width, header.height, top_down)?;
    if rle {
        decode_rle(&header, &palette, data, &mut sink)?;
    } else {
        decode_raw(&header, &palette, data, &mut sink)?;
    }
    sink.finish(header.layout())
}

fn truncated(needed: u64, available: usize) -> UiImageError {
    UiImageError::Truncated {
        format: FORMAT,
        needed,
        available: available as u64,
    }
}

fn decode_raw(header: &Header, palette: &Palette, data: &[u8], sink: &mut Sink) -> Result<()> {
    // decode_tga has checked that the data holds every pixel.
    let bpp = header.pixel_bytes();
    let row_bytes = header.width as usize * bpp;
    let kept = sink.kept_columns();
    for (file_row, row) in data
        .chunks_exact(row_bytes)
        .take(header.height as usize)
        .enumerate()
    {
        let Some(out_row) = sink.output_row(file_row as u64) else {
            continue;
        };
        let out = sink.row_mut(out_row);
        for (dst, raw) in out.iter_mut().zip(row.chunks_exact(bpp).take(kept)) {
            *dst = palette.convert(raw)?;
        }
    }
    Ok(())
}

fn decode_rle(header: &Header, palette: &Palette, data: &[u8], sink: &mut Sink) -> Result<()> {
    let bpp = header.pixel_bytes();
    let total = sink.source_pixels();
    let mut written = 0u64;
    let mut pos = 0usize;
    while written < total {
        let &packet = data
            .get(pos)
            .ok_or_else(|| truncated(pos as u64 + 1, data.len()))?;
        pos += 1;
        // Pixels of a packet past the end of the image are data after the last pixel:
        // ignored, and not required to be present.
        let count = (u64::from(packet & 0x7F) + 1).min(total - written);
        if packet & 0x80 != 0 {
            let raw = data
                .get(pos..pos + bpp)
                .ok_or_else(|| truncated((pos + bpp) as u64, data.len()))?;
            pos += bpp;
            // Converted once, and only when one of its pixels is kept.
            let mut colour = None;
            sink.write_with(written, count, |_| match colour {
                Some(c) => Ok(c),
                None => {
                    let c = palette.convert(raw)?;
                    colour = Some(c);
                    Ok(c)
                }
            })?;
        } else {
            let len = count as usize * bpp;
            let raw = data
                .get(pos..pos + len)
                .ok_or_else(|| truncated((pos + len) as u64, data.len()))?;
            pos += len;
            // Only kept pixels are converted (and, for a colour map, checked), as in
            // decode_raw.
            sink.write_with(written, count, |i| {
                let at = i as usize * bpp;
                raw.get(at..at + bpp)
                    .ok_or_else(|| truncated((pos - len + at + bpp) as u64, data.len()))
                    .and_then(|px| palette.convert(px))
            })?;
        }
        written += count;
    }
    Ok(())
}
