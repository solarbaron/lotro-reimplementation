// SPDX-License-Identifier: GPL-3.0-or-later
//! Render surfaces, DID prefix 0x41 (spec §5).

use std::fmt;

use crate::bcn::{BcFormat, decode_bcn};
use crate::did::{Did, DidKind};
use crate::error::{FormatError, Result};
use crate::reader::Reader;
use crate::record::{Record, read_prologue};

const SURFACE: &str = "render surface";

/// Largest width or height accepted when decoding an embedded JPEG.
pub const MAX_JPEG_DIMENSION: u32 = 8192;

/// Largest pixel count accepted when decoding an embedded JPEG, whatever the input size.
/// The RGBA8 output is then at most 128 MiB (the largest JPEG surface observed is
/// 4800×4800, about 22 Mi pixels).
pub const MAX_JPEG_PIXELS: u64 = 32 * 1024 * 1024;

/// Pixels an embedded JPEG may decode to per byte of compressed input, on top of
/// [`MIN_JPEG_PIXEL_ALLOWANCE`]. Real surfaces stay below about 100 pixels per byte; the
/// limit stops a tiny stream with a large frame header from expanding into a huge image.
pub const JPEG_PIXELS_PER_INPUT_BYTE: u64 = 1024;

/// Pixel count every embedded JPEG may decode to regardless of its input size (1 Mi pixels).
pub const MIN_JPEG_PIXEL_ALLOWANCE: u64 = 1024 * 1024;

/// Pixel budget for an embedded JPEG of `input_len` compressed bytes: the smaller of
/// [`MAX_JPEG_PIXELS`] and [`MIN_JPEG_PIXEL_ALLOWANCE`] + `input_len` ×
/// [`JPEG_PIXELS_PER_INPUT_BYTE`].
#[must_use]
pub fn jpeg_pixel_budget(input_len: usize) -> u64 {
    let len = u64::try_from(input_len).unwrap_or(u64::MAX);
    len.saturating_mul(JPEG_PIXELS_PER_INPUT_BYTE)
        .saturating_add(MIN_JPEG_PIXEL_ALLOWANCE)
        .min(MAX_JPEG_PIXELS)
}

/// Largest number of heap bytes an embedded JPEG decode may need, as estimated by
/// [`jpeg_decode_bytes`]: the RGBA8 output plus, for progressive streams, the decoder's
/// coefficient buffers. A baseline frame of [`MAX_JPEG_PIXELS`] needs 128 MiB.
pub const MAX_JPEG_DECODE_BYTES: u64 = 256 * 1024 * 1024;

/// Estimated heap bytes needed to decode a `width` × `height` JPEG frame with `components`
/// colour components: 4 bytes per pixel of RGBA8 output, plus, for a progressive frame
/// (whose scans refine the whole image), 2 bytes per coefficient for every component at
/// full resolution with the frame padded to 16-pixel MCUs. The coefficient term ignores
/// chroma subsampling, so it overestimates subsampled frames.
#[must_use]
pub fn jpeg_decode_bytes(width: u32, height: u32, components: u8, progressive: bool) -> u64 {
    let output = u64::from(width)
        .saturating_mul(u64::from(height))
        .saturating_mul(4);
    if !progressive {
        return output;
    }
    let padded =
        (u64::from(width.div_ceil(16)) * 16).saturating_mul(u64::from(height.div_ceil(16)) * 16);
    output.saturating_add(padded.saturating_mul(u64::from(components) * 2))
}

/// Frame-type facts read from a JPEG's start-of-frame marker.
#[cfg(feature = "jpeg")]
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
struct JpegFrame {
    /// SOF2, SOF6, SOF10 or SOF14: the scans refine the whole image progressively.
    progressive: bool,
    /// Number of colour components in the frame.
    components: u8,
    /// Frame width in pixels, as stored in the frame header.
    width: u16,
    /// Frame height in pixels, as stored in the frame header.
    height: u16,
}

/// Walks the marker segments of a JPEG stream up to its start-of-frame marker. `None` if
/// the stream ends, a segment is malformed, or a scan starts before any frame header.
///
/// Fails closed on the marker codes that have no place before a frame header: 0x00 (a
/// stuffed byte), 0x01 (TEM), 0xD0–0xD7 (restart markers) and a second 0xD8 (start of
/// image). Decoders disagree on how to step over them (stand-alone marker, length-carrying
/// segment, or fill), so accepting them would let this walk reach a different frame header
/// from the one the decoder uses, and the memory check below would rely on the wrong frame.
#[cfg(feature = "jpeg")]
fn jpeg_frame(data: &[u8]) -> Option<JpegFrame> {
    let mut pos = 2usize; // after FF D8
    while pos < data.len() {
        if *data.get(pos)? != 0xFF {
            return None;
        }
        // Fill bytes: any number of FF before the marker code.
        while data.get(pos) == Some(&0xFF) {
            pos += 1;
        }
        let marker = *data.get(pos)?;
        pos += 1;
        if matches!(marker, 0x00 | 0x01 | 0xD0..=0xD8) {
            return None;
        }
        let len = usize::from(u16::from_be_bytes([*data.get(pos)?, *data.get(pos + 1)?]));
        if len < 2 {
            return None;
        }
        let segment = data.get(pos + 2..pos.checked_add(len)?)?;
        match marker {
            0xC0..=0xCF if !matches!(marker, 0xC4 | 0xC8 | 0xCC) => {
                // Frame header: precision (1), height (2), width (2), components (1).
                return Some(JpegFrame {
                    progressive: matches!(marker, 0xC2 | 0xC6 | 0xCA | 0xCE),
                    components: *segment.get(5)?,
                    width: u16::from_be_bytes([*segment.get(3)?, *segment.get(4)?]),
                    height: u16::from_be_bytes([*segment.get(1)?, *segment.get(2)?]),
                });
            }
            0xDA | 0xD9 => return None,
            _ => pos += len,
        }
    }
    None
}

/// Pixel format of a render surface (spec §5.2).
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum SurfaceFormat {
    /// Code 20: 3 bytes per pixel, stored B, G, R.
    R8G8B8,
    /// Code 21: 4 bytes per pixel, stored B, G, R, A (straight alpha).
    A8R8G8B8,
    /// Code 28: 1 byte per pixel, single channel.
    A8,
    /// Code 500: embedded JFIF stream; header width/height are 0.
    Jpeg,
    /// FourCC "DXT1" (BC1).
    Dxt1,
    /// FourCC "DXT3" (BC2).
    Dxt3,
    /// FourCC "DXT5" (BC3).
    Dxt5,
}

impl SurfaceFormat {
    /// Code 20.
    pub const CODE_R8G8B8: u32 = 20;
    /// Code 21.
    pub const CODE_A8R8G8B8: u32 = 21;
    /// Code 28.
    pub const CODE_A8: u32 = 28;
    /// Code 500.
    pub const CODE_JPEG: u32 = 500;
    /// "DXT1" as a little-endian u32.
    pub const CODE_DXT1: u32 = u32::from_le_bytes(*b"DXT1");
    /// "DXT3" as a little-endian u32.
    pub const CODE_DXT3: u32 = u32::from_le_bytes(*b"DXT3");
    /// "DXT5" as a little-endian u32.
    pub const CODE_DXT5: u32 = u32::from_le_bytes(*b"DXT5");

    /// Maps a stored format code to a format.
    #[must_use]
    pub fn from_code(code: u32) -> Option<Self> {
        Some(match code {
            Self::CODE_R8G8B8 => Self::R8G8B8,
            Self::CODE_A8R8G8B8 => Self::A8R8G8B8,
            Self::CODE_A8 => Self::A8,
            Self::CODE_JPEG => Self::Jpeg,
            Self::CODE_DXT1 => Self::Dxt1,
            Self::CODE_DXT3 => Self::Dxt3,
            Self::CODE_DXT5 => Self::Dxt5,
            _ => return None,
        })
    }

    /// The stored format code.
    #[must_use]
    pub fn code(self) -> u32 {
        match self {
            Self::R8G8B8 => Self::CODE_R8G8B8,
            Self::A8R8G8B8 => Self::CODE_A8R8G8B8,
            Self::A8 => Self::CODE_A8,
            Self::Jpeg => Self::CODE_JPEG,
            Self::Dxt1 => Self::CODE_DXT1,
            Self::Dxt3 => Self::CODE_DXT3,
            Self::Dxt5 => Self::CODE_DXT5,
        }
    }

    /// Short name.
    #[must_use]
    pub fn name(self) -> &'static str {
        match self {
            Self::R8G8B8 => "R8G8B8",
            Self::A8R8G8B8 => "A8R8G8B8",
            Self::A8 => "A8",
            Self::Jpeg => "JPEG",
            Self::Dxt1 => "DXT1",
            Self::Dxt3 => "DXT3",
            Self::Dxt5 => "DXT5",
        }
    }

    /// Block-compression variant, for the DXT formats.
    #[must_use]
    pub fn block_compression(self) -> Option<BcFormat> {
        match self {
            Self::Dxt1 => Some(BcFormat::Bc1),
            Self::Dxt3 => Some(BcFormat::Bc2),
            Self::Dxt5 => Some(BcFormat::Bc3),
            _ => None,
        }
    }

    /// Data length implied by the dimensions (spec §5.2 "Expected L"); `None` for JPEG
    /// (any length) or on arithmetic overflow.
    #[must_use]
    pub fn expected_len(self, width: u32, height: u32) -> Option<u64> {
        let px = u64::from(width).checked_mul(u64::from(height));
        match self {
            Self::R8G8B8 => px?.checked_mul(3),
            Self::A8R8G8B8 => px?.checked_mul(4),
            Self::A8 => px,
            Self::Jpeg => None,
            Self::Dxt1 | Self::Dxt3 | Self::Dxt5 => {
                self.block_compression()?.image_bytes(width, height)
            }
        }
    }
}

impl fmt::Display for SurfaceFormat {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.write_str(self.name())
    }
}

/// A decoded RGBA8 image: tightly packed, top row first, straight alpha.
#[derive(Clone, PartialEq, Eq)]
pub struct RgbaImage {
    /// Width in pixels.
    pub width: u32,
    /// Height in pixels.
    pub height: u32,
    /// `width * height * 4` bytes, R, G, B, A per pixel.
    pub pixels: Vec<u8>,
}

impl fmt::Debug for RgbaImage {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.debug_struct("RgbaImage")
            .field("width", &self.width)
            .field("height", &self.height)
            .field("pixels", &format_args!("[{} bytes]", self.pixels.len()))
            .finish()
    }
}

/// One render surface: header fields plus a borrowed slice of the image data (spec §5.1).
#[derive(Clone, Copy, PartialEq, Eq)]
pub struct Surface<'a> {
    /// Surface DID.
    pub did: Did,
    /// Category field at offset 4 (meaning OPEN; not needed for decoding).
    pub category: u32,
    /// Width in pixels (0 for JPEG).
    pub width: u32,
    /// Height in pixels (0 for JPEG).
    pub height: u32,
    /// Pixel format.
    pub format: SurfaceFormat,
    /// Raw image data, exactly the stored length.
    pub data: &'a [u8],
}

impl fmt::Debug for Surface<'_> {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.debug_struct("Surface")
            .field("did", &self.did)
            .field("category", &self.category)
            .field("width", &self.width)
            .field("height", &self.height)
            .field("format", &self.format)
            .field("data", &format_args!("[{} bytes]", self.data.len()))
            .finish()
    }
}

impl<'a> Record<'a> for Surface<'a> {
    const NAME: &'static str = "render surface";

    fn did(&self) -> Did {
        self.did
    }

    fn parse(data: &'a [u8]) -> Result<Self> {
        let mut r = Reader::new(data);
        let did = read_prologue(&mut r, Self::NAME, |k| k == DidKind::RenderSurface)?;
        let category = r.u32()?;
        let width = r.u32()?;
        let height = r.u32()?;
        let code = r.u32()?;
        let len_at = r.position();
        let len = r.u32()?;
        let format =
            SurfaceFormat::from_code(code).ok_or(FormatError::UnsupportedSurfaceFormat { code })?;
        if let Some(expected) = format.expected_len(width, height) {
            if expected != u64::from(len) {
                return Err(FormatError::SurfaceLength {
                    format: format.name(),
                    width,
                    height,
                    expected,
                    actual: u64::from(len),
                });
            }
        } else if format != SurfaceFormat::Jpeg {
            return Err(FormatError::malformed(
                Self::NAME,
                8,
                "dimensions overflow the data length",
            ));
        } else if width != 0 || height != 0 {
            tracing::debug!(%did, width, height, "JPEG surface with non-zero header dimensions");
        }
        let len = usize::try_from(len).map_err(|_| {
            FormatError::malformed(Self::NAME, len_at, "data length exceeds address space")
        })?;
        let pixels = r.bytes(len)?;
        r.expect_end(Self::NAME)?;
        Ok(Self {
            did,
            category,
            width,
            height,
            format,
            data: pixels,
        })
    }
}

impl Surface<'_> {
    /// Whether the data is an embedded JPEG stream.
    #[must_use]
    pub fn is_jpeg(&self) -> bool {
        self.format == SurfaceFormat::Jpeg
    }

    /// Converts the surface to RGBA8.
    ///
    /// - R8G8B8 / A8R8G8B8 are swizzled from B, G, R(, A) order; R8G8B8 gets alpha 255.
    /// - A8 is treated as an alpha channel over white: `(255, 255, 255, a)`. Whether the
    ///   channel is alpha, luminance or a mask is OPEN in the spec; use [`Surface::data`]
    ///   directly for other interpretations.
    /// - DXT1/3/5 are software-decoded.
    /// - JPEG is decoded with the `image` crate (feature `jpeg`). The stream must start with
    ///   `FF D8 FF` and end with the `FF D9` end-of-image marker (spec §5.3: the data is a
    ///   complete JFIF stream). Its frame dimensions are checked against
    ///   [`MAX_JPEG_DIMENSION`] and [`jpeg_pixel_budget`], and its estimated decode memory
    ///   ([`jpeg_decode_bytes`], which counts progressive coefficient buffers) against
    ///   [`MAX_JPEG_DECODE_BYTES`], before any pixel buffer is allocated. The image is
    ///   decoded straight into the RGBA8 output buffer, so peak memory is about one RGBA8
    ///   image plus the decoder's working state (for progressive streams, up to 2 bytes
    ///   per sample of every component).
    pub fn to_rgba8(&self) -> Result<RgbaImage> {
        let (width, height) = (self.width, self.height);
        if self.format != SurfaceFormat::Jpeg {
            let expected = self.format.expected_len(width, height).ok_or_else(|| {
                FormatError::malformed(SURFACE, 8, "dimensions overflow the data length")
            })?;
            if expected != self.data.len() as u64 {
                return Err(FormatError::SurfaceLength {
                    format: self.format.name(),
                    width,
                    height,
                    expected,
                    actual: self.data.len() as u64,
                });
            }
        }
        let pixels = match self.format {
            SurfaceFormat::R8G8B8 => self
                .data
                .as_chunks::<3>()
                .0
                .iter()
                .flat_map(|p| [p[2], p[1], p[0], 255])
                .collect(),
            SurfaceFormat::A8R8G8B8 => self
                .data
                .as_chunks::<4>()
                .0
                .iter()
                .flat_map(|p| [p[2], p[1], p[0], p[3]])
                .collect(),
            SurfaceFormat::A8 => self.data.iter().flat_map(|&a| [255, 255, 255, a]).collect(),
            SurfaceFormat::Dxt1 | SurfaceFormat::Dxt3 | SurfaceFormat::Dxt5 => {
                let bc = self.format.block_compression().ok_or(
                    FormatError::UnsupportedSurfaceFormat {
                        code: self.format.code(),
                    },
                )?;
                decode_bcn(bc, width, height, self.data)
                    .ok_or_else(|| FormatError::malformed(SURFACE, 24, "block data too short"))?
            }
            SurfaceFormat::Jpeg => return decode_jpeg(self.data),
        };
        Ok(RgbaImage {
            width,
            height,
            pixels,
        })
    }
}

#[cfg(feature = "jpeg")]
fn decode_jpeg(data: &[u8]) -> Result<RgbaImage> {
    use image::codecs::jpeg::JpegDecoder;
    use image::{ColorType, ImageDecoder, Limits};
    use std::io::Cursor;

    let image_err = |e: image::ImageError| FormatError::Image(e.to_string());
    if !data.starts_with(&[0xFF, 0xD8, 0xFF]) {
        return Err(FormatError::malformed(
            SURFACE,
            24,
            "JPEG data does not start with FF D8 FF",
        ));
    }
    // The decoder pads a truncated scan with filler instead of failing, so a stream that
    // stops early is caught here (spec §5.3: the data is a complete JFIF stream).
    if !data.ends_with(&[0xFF, 0xD9]) {
        return Err(FormatError::malformed(
            SURFACE,
            24 + data.len().saturating_sub(2),
            "JPEG data does not end with the FF D9 end-of-image marker",
        ));
    }
    // Parses the headers only; no pixel memory is allocated yet.
    let mut decoder = JpegDecoder::new(Cursor::new(data)).map_err(image_err)?;
    let (width, height) = decoder.dimensions();
    let pixels = u64::from(width) * u64::from(height);
    let budget = jpeg_pixel_budget(data.len());
    if width > MAX_JPEG_DIMENSION || height > MAX_JPEG_DIMENSION || pixels > budget {
        return Err(FormatError::ImageTooLarge {
            width,
            height,
            budget,
        });
    }
    if pixels == 0 {
        return Err(FormatError::Image(format!(
            "JPEG frame has zero size ({width}x{height})"
        )));
    }
    // Progressive streams keep coefficient buffers for the whole image while they decode,
    // outside the decoder's allocation limit, so the total is checked here up front.
    let frame = jpeg_frame(data)
        .ok_or_else(|| FormatError::malformed(SURFACE, 24, "JPEG frame header not found"))?;
    // Second safeguard: the frame this walk found must be the one the decoder parsed.
    if (u32::from(frame.width), u32::from(frame.height)) != (width, height) {
        return Err(FormatError::malformed(
            SURFACE,
            24,
            "JPEG frame header does not match the frame the decoder read",
        ));
    }
    let need = jpeg_decode_bytes(width, height, frame.components, frame.progressive);
    if need > MAX_JPEG_DECODE_BYTES {
        // Pixels this frame type could have had within the byte budget.
        let per_pixel = if frame.progressive {
            4 + 2 * u64::from(frame.components)
        } else {
            4
        };
        return Err(FormatError::ImageTooLarge {
            width,
            height,
            budget: MAX_JPEG_DECODE_BYTES / per_pixel,
        });
    }
    let mut limits = Limits::default();
    limits.max_image_width = Some(MAX_JPEG_DIMENSION);
    limits.max_image_height = Some(MAX_JPEG_DIMENSION);
    // Advisory only: the decoder does not count every buffer against it (progressive
    // coefficient storage in particular). The checks above bound the real total.
    limits.max_alloc = Some(MAX_JPEG_DECODE_BYTES);
    decoder.set_limits(limits).map_err(image_err)?;
    let channels: usize = match decoder.color_type() {
        ColorType::L8 => 1,
        ColorType::La8 => 2,
        ColorType::Rgb8 => 3,
        ColorType::Rgba8 => 4,
        other => {
            return Err(FormatError::Image(format!(
                "unsupported JPEG colour type {other:?}"
            )));
        }
    };
    // `pixels` is within the budget, so these products fit comfortably in usize.
    let count = usize::try_from(pixels)
        .map_err(|_| FormatError::Image("JPEG pixel count exceeds address space".into()))?;
    let out_len = count * 4;
    let native_len = count * channels;
    if u64::try_from(native_len).ok() != Some(decoder.total_bytes()) {
        return Err(FormatError::Image(format!(
            "JPEG decoder reports {} bytes for {width}x{height} with {channels} channels",
            decoder.total_bytes()
        )));
    }
    let mut buf = Vec::new();
    buf.try_reserve_exact(out_len)
        .map_err(|_| FormatError::Image(format!("cannot allocate {out_len} bytes")))?;
    buf.resize(out_len, 0);
    // Decode into the front of the output buffer, then widen to RGBA in place.
    let native = buf
        .get_mut(..native_len)
        .ok_or_else(|| FormatError::Image("JPEG output buffer too small".into()))?;
    decoder.read_image(native).map_err(image_err)?;
    expand_to_rgba_in_place(&mut buf, count, channels);
    Ok(RgbaImage {
        width,
        height,
        pixels: buf,
    })
}

/// Widens `count` pixels of `channels` bytes each, packed at the front of `buf`, to RGBA8.
///
/// `buf` must be `count * 4` bytes long. Pixels are processed from the last to the first:
/// pixel `i` is read from `i * channels` and written to `i * 4`, which is never below any
/// unread source byte, so no input is overwritten before it is read.
#[cfg(feature = "jpeg")]
fn expand_to_rgba_in_place(buf: &mut [u8], count: usize, channels: usize) {
    if buf.len() != count.saturating_mul(4) {
        return;
    }
    for i in (0..count).rev() {
        let src = i * channels;
        let px = match channels {
            1 => [buf[src], buf[src], buf[src], 255],
            2 => [buf[src], buf[src], buf[src], buf[src + 1]],
            3 => [buf[src], buf[src + 1], buf[src + 2], 255],
            _ => return,
        };
        buf[i * 4..i * 4 + 4].copy_from_slice(&px);
    }
}

#[cfg(not(feature = "jpeg"))]
fn decode_jpeg(_data: &[u8]) -> Result<RgbaImage> {
    Err(FormatError::FeatureDisabled("jpeg"))
}

#[cfg(all(test, feature = "jpeg"))]
mod tests {
    use super::*;

    #[test]
    fn expand_in_place_matches_reference() {
        for channels in 1..=3usize {
            let count = 37;
            let native: Vec<u8> = (0..count * channels).map(|i| (i * 7 + 3) as u8).collect();
            let mut buf = vec![0u8; count * 4];
            buf[..native.len()].copy_from_slice(&native);
            expand_to_rgba_in_place(&mut buf, count, channels);
            for i in 0..count {
                let p = &native[i * channels..(i + 1) * channels];
                let want = match channels {
                    1 => [p[0], p[0], p[0], 255],
                    2 => [p[0], p[0], p[0], p[1]],
                    _ => [p[0], p[1], p[2], 255],
                };
                assert_eq!(
                    &buf[i * 4..i * 4 + 4],
                    &want,
                    "channels {channels} pixel {i}"
                );
            }
        }
    }

    /// A minimal JPEG header: SOI, an APP0 segment, then a frame header with `marker`.
    fn header(marker: u8, components: u8) -> Vec<u8> {
        let mut v = vec![
            0xFF, 0xD8, 0xFF, 0xE0, 0x00, 0x04, 0xAA, 0xBB, 0xFF, 0xFF, marker,
        ];
        let len = 8 + 3 * u16::from(components);
        v.extend_from_slice(&len.to_be_bytes());
        v.extend_from_slice(&[8, 0x10, 0x00, 0x20, 0x00, components]);
        for c in 0..components {
            v.extend_from_slice(&[c + 1, 0x11, 0]);
        }
        v
    }

    #[test]
    fn frame_type_is_read_from_sof() {
        for (marker, progressive) in [(0xC0, false), (0xC1, false), (0xC2, true), (0xCA, true)] {
            assert_eq!(
                jpeg_frame(&header(marker, 3)),
                Some(JpegFrame {
                    progressive,
                    components: 3,
                    width: 0x2000,
                    height: 0x1000,
                }),
                "{marker:#x}"
            );
        }
        // DHT (C4) is not a frame header; a scan before any frame header is rejected.
        let mut dht = header(0xC4, 1);
        dht.extend_from_slice(&[0xFF, 0xDA, 0x00, 0x02]);
        assert_eq!(jpeg_frame(&dht), None);
        // Truncated and garbage inputs give up without panicking.
        let full = header(0xC2, 3);
        for cut in 0..full.len() {
            let _ = jpeg_frame(&full[..cut]);
        }
        assert_eq!(jpeg_frame(&[0xFF, 0xD8, 0x00]), None);
        assert_eq!(jpeg_frame(&[0xFF, 0xD8, 0xFF, 0xE0, 0x00, 0x01]), None);
    }

    /// Marker codes that decoders step over in different ways are rejected before the
    /// frame header, so the walk cannot be steered to a decoy frame.
    #[test]
    fn ambiguous_markers_before_the_frame_are_rejected() {
        for code in [0x00, 0x01, 0xD0, 0xD3, 0xD7, 0xD8] {
            let mut v = vec![0xFF, 0xD8, 0xFF, code];
            v.extend_from_slice(&header(0xC0, 1)[2..]);
            assert_eq!(jpeg_frame(&v), None, "{code:#x}");
        }
    }

    #[test]
    fn progressive_decode_bytes_count_coefficients() {
        assert_eq!(jpeg_decode_bytes(8192, 4096, 3, false), 128 * 1024 * 1024);
        // 8192x4096 colour progressive: 128 MiB output + 192 MiB coefficients.
        assert_eq!(jpeg_decode_bytes(8192, 4096, 3, true), 320 * 1024 * 1024);
        assert!(jpeg_decode_bytes(8192, 4096, 3, true) > MAX_JPEG_DECODE_BYTES);
        // The largest observed surface (4800x4800) fits even if it were progressive.
        assert!(jpeg_decode_bytes(4800, 4800, 3, true) <= MAX_JPEG_DECODE_BYTES);
        // Every baseline frame within the pixel cap fits.
        assert!(jpeg_decode_bytes(8192, 4096, 4, false) <= MAX_JPEG_DECODE_BYTES);
        assert_eq!(jpeg_decode_bytes(u32::MAX, u32::MAX, 255, true), u64::MAX);
    }

    #[test]
    fn pixel_budget_is_bounded() {
        assert_eq!(jpeg_pixel_budget(0), MIN_JPEG_PIXEL_ALLOWANCE);
        assert_eq!(
            jpeg_pixel_budget(1000),
            MIN_JPEG_PIXEL_ALLOWANCE + 1000 * JPEG_PIXELS_PER_INPUT_BYTE
        );
        assert_eq!(jpeg_pixel_budget(usize::MAX), MAX_JPEG_PIXELS);
    }
}
