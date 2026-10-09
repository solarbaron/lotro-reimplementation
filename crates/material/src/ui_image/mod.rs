// SPDX-License-Identifier: GPL-3.0-or-later
//! Decoders for UI image files: user-skin images and plugin images.
//!
//! | Item | Purpose | Spec |
//! |---|---|---|
//! | [`decode_tga`] | TGA at depths 32, 24, 16, 15 and 8, uncompressed or run-length, with colour maps | `docs/specs/skin-definition.md` §7.2, D14, D17 |
//! | [`decode_dds`] | DDS: DXT1–DXT5 and the uncompressed formats of the accepted-format table | `skin-definition.md` §7.4, D18 |
//! | [`decode_jpeg`] | Baseline and progressive JPEG | `skin-definition.md` §7.3 |
//! | [`decode_file`] | Dispatch by final extension (D7) for a skin or a plugin file, then decode | `skin-definition.md` §7.1, D7, D15; decision 0015 §4.3 |
//! | [`probe_header`] | Format and size from at most [`PROBE_LEN`] bytes, without decoding, for `SetBackground(path)` | decision 0015 §4.4 |
//! | [`from_surface`] | A render surface (0x41) in the same canonical form | decision 0015 §4.2 |
//! | [`placeholder`] | The 32 × 32 image shown for an image that cannot be loaded | `skin-definition.md` §6.5, D16, SP12 |
//!
//! # Canonical form
//!
//! Every decoder returns a [`UiImage`]: 32-bit `0xAARRGGBB` pixels (straight alpha), top
//! row first, plus the [`PixelLayout`] the image was stored in. The layout gives the
//! [`SourceTag`] of decision 0015 §4.2 (`skin-definition.md` D26), which records the
//! **source** format, not what the pixels look like: *ARGB32* for A8R8G8B8 alone,
//! *alpha-only* for A8 and 8-bit TGA (black, the 8-bit value as alpha, D17), *no alpha* for
//! the opaque formats (alpha 255) and *other* for everything else (DXT1–DXT5, the 32-bit
//! layouts in another byte order, and the formats whose channels are widened). Channels of
//! 1, 2, 4, 5 and 6 bits are widened to 8 bits by bit replication; 10-bit channels keep
//! their top 8 bits.
//!
//! # Limits (hostile input)
//!
//! - A file larger than [`MAX_FILE_BYTES`] or empty fails (D4).
//! - A width or height of 0 or above [`MAX_SOURCE_DIMENSION`] fails (D4).
//! - An image with more than [`MAX_DECODED_DIMENSION`] pixels on an axis is **cropped at
//!   decode** to its top-left [`MAX_DECODED_DIMENSION`] pixels on that axis, top-left
//!   meaning the image as displayed (after the TGA row order). TGA and DDS images are
//!   decoded straight into the cropped output, so the decoded image is at most
//!   64 MiB. A JPEG must be decoded whole before it is cropped, so the full frame the
//!   decoder read is first checked against the `tessera-formats` JPEG budgets: the
//!   estimate [`tessera_formats::surface::jpeg_decode_bytes`] for a progressive frame plus
//!   the cropped output against [`tessera_formats::surface::MAX_JPEG_DECODE_BYTES`], and
//!   at most [`tessera_formats::surface::jpeg_pixel_budget`] pixels (never more than
//!   [`tessera_formats::surface::MAX_JPEG_PIXELS`]); a larger frame fails with
//!   [`UiImageError::TooLarge`] (details at [`decode_jpeg`]).
//! - Every output buffer is reserved with `try_reserve`, so an allocation failure is an
//!   error, not an abort.
//!
//! All functions are pure (bytes in, image out) and can run on any asset worker.
//!
//! ```
//! use std::path::Path;
//! use tessera_material::ui_image::{ImageOrigin, SourceTag, decode_file};
//!
//! // A 1 × 1 uncompressed 32-bit TGA (B, G, R, A), top-left origin.
//! let mut tga = vec![0, 0, 2, 0, 0, 0, 0, 0, 0, 0, 0, 0, 1, 0, 1, 0, 32, 0x28];
//! tga.extend_from_slice(&[0x30, 0x20, 0x10, 0x80]);
//! let img = decode_file(Path::new("art/Frame.TGA"), ImageOrigin::Skin, &tga).unwrap();
//! assert_eq!(img.pixels(), &[0x8010_2030]);
//! assert_eq!(img.tag(), SourceTag::Argb32);
//! ```

mod dds;
mod jpeg;
mod sink;
mod tga;

#[cfg(test)]
mod tests;

use std::path::Path;

use tessera_formats::{Surface, SurfaceFormat};

pub use dds::decode_dds;
pub use jpeg::decode_jpeg;
pub use tga::decode_tga;

/// Largest accepted image file, in bytes (`skin-definition.md` D4).
pub const MAX_FILE_BYTES: usize = 64 * 1024 * 1024;

/// Largest accepted width or height of a source image, in pixels (D4).
pub const MAX_SOURCE_DIMENSION: u32 = 16_384;

/// Largest width or height of a decoded image: larger sources are cropped to their
/// top-left part (D25, decision 0015 §4.6).
pub const MAX_DECODED_DIMENSION: u32 = 4_096;

/// Number of bytes [`probe_header`] looks at (decision 0015 §4.4).
pub const PROBE_LEN: usize = 64;

/// Result type of this module.
pub type Result<T> = std::result::Result<T, UiImageError>;

/// Why a UI image could not be decoded or probed. Every variant means the same thing to
/// the UI: the element shows the placeholder (`skin-definition.md` §6.5, D16), or for a
/// plugin `SetBackground(path)` the call raises its error (`lua-plugin-api.md` §13.8).
#[derive(Debug, Clone, PartialEq, Eq, thiserror::Error)]
#[non_exhaustive]
pub enum UiImageError {
    /// The file is empty.
    #[error("image file is empty")]
    Empty,
    /// The file is larger than [`MAX_FILE_BYTES`].
    #[error("image file of {len} bytes exceeds the limit of {max} bytes")]
    FileTooLarge {
        /// File length.
        len: usize,
        /// The limit.
        max: usize,
    },
    /// The path has no extension that the origin's dispatch decodes (D7, D15).
    #[error("unsupported image file type {extension:?}")]
    UnsupportedFileType {
        /// The final extension, lower-cased (empty when there is none).
        extension: String,
    },
    /// A plugin `.png` or `.bmp` file: refused until `plugin-api-usage.md` O6 is answered
    /// (decision 0015 §4.3).
    #[error("plugin image format {extension:?} is not supported (only .tga and .jpg)")]
    RefusedPluginFormat {
        /// `png` or `bmp`.
        extension: String,
    },
    /// The header declares a width or height of 0 or above [`MAX_SOURCE_DIMENSION`].
    #[error("image size {width}x{height} is outside 1..={max} per axis")]
    BadDimensions {
        /// Declared width.
        width: u32,
        /// Declared height.
        height: u32,
        /// [`MAX_SOURCE_DIMENSION`].
        max: u32,
    },
    /// The data ends before the format says it should.
    #[error("{format} data truncated: need {needed} bytes, have {available}")]
    Truncated {
        /// Container name.
        format: &'static str,
        /// Bytes the decoder needed.
        needed: u64,
        /// Bytes available.
        available: u64,
    },
    /// A TGA pixel depth other than 32, 24, 16, 15 or 8.
    #[error("unsupported TGA pixel depth {0}")]
    UnsupportedTgaDepth(u8),
    /// A TGA colour map with an entry size other than 15, 16, 24 or 32 bits.
    #[error("unsupported TGA colour-map entry size {0}")]
    UnsupportedColourMapEntry(u8),
    /// A colour-mapped TGA pixel whose index is not in the colour map.
    #[error("TGA colour-map index {index} outside entries {first}..{end}")]
    ColourMapIndex {
        /// The pixel value.
        index: u32,
        /// First entry index of the map.
        first: u32,
        /// One past the last entry index.
        end: u32,
    },
    /// A DDS file that does not start with `DDS `.
    #[error("not a DDS file (bad magic)")]
    BadDdsMagic,
    /// A DDS pixel format that is not in the accepted-format table (§7.4).
    #[error(
        "unsupported DDS pixel format (flags {flags:#x}, FourCC {fourcc:#010x}, {bits} bits, masks {masks:08x?})"
    )]
    UnsupportedDdsFormat {
        /// Pixel-format flags.
        flags: u32,
        /// FourCC field.
        fourcc: u32,
        /// Low byte of the RGB bit count.
        bits: u8,
        /// Red, green, blue and alpha masks.
        masks: [u32; 4],
    },
    /// A DXT image whose width or height is not a multiple of 4 (D18).
    #[error("DXT image size {width}x{height} is not a multiple of 4")]
    DxtNotMultipleOf4 {
        /// Width.
        width: u32,
        /// Height.
        height: u32,
    },
    /// A FourCC DDS whose non-zero linear-size field differs from the computed size (§7.4
    /// step 4).
    #[error("DDS linear size {stated} differs from the computed size {computed}")]
    DdsLinearSize {
        /// The header field.
        stated: u32,
        /// Bit count × height × width ÷ 8.
        computed: u64,
    },
    /// The data does not start with a JPEG start-of-image marker (`FF D8 FF`).
    #[error("not a JPEG stream (no start-of-image marker)")]
    NotJpeg,
    /// The JPEG decoder rejected the stream.
    #[error("JPEG decode failed: {0}")]
    Jpeg(String),
    /// A JPEG frame whose full decode would exceed the decode budgets.
    #[error("JPEG frame {width}x{height} exceeds the decode budget")]
    TooLarge {
        /// Frame width.
        width: u32,
        /// Frame height.
        height: u32,
    },
    /// The output buffer could not be allocated.
    #[error("cannot allocate {bytes} bytes for a decoded image")]
    Alloc {
        /// Requested bytes.
        bytes: u64,
    },
    /// A render surface that could not be converted.
    #[error("render surface: {0}")]
    Surface(String),
}

/// Where a file image comes from; it decides which formats are decoded.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum ImageOrigin {
    /// A user-skin file (`skin-definition.md` §7): `.dds`, `.jpg` and `.tga`.
    Skin,
    /// A file under `Plugins/` (`lua-plugin-api.md` §13.8): `.tga` and `.jpg`; `.png` and
    /// `.bmp` are refused with [`UiImageError::RefusedPluginFormat`].
    Plugin,
}

/// Container format of a file image.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum ImageFileFormat {
    /// Truevision TGA (§7.2).
    Tga,
    /// DirectDraw Surface (§7.4).
    Dds,
    /// JPEG (§7.3).
    Jpeg,
}

impl ImageFileFormat {
    /// Short name for diagnostics.
    #[must_use]
    pub const fn name(self) -> &'static str {
        match self {
            Self::Tga => "TGA",
            Self::Dds => "DDS",
            Self::Jpeg => "JPEG",
        }
    }
}

/// The pixel layout an image was stored in (formats named by their usual 3D-API names,
/// highest bits first).
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum PixelLayout {
    /// 8-bit A, R, G, B (32-bit TGA, DDS A8R8G8B8, surface format 21).
    A8R8G8B8,
    /// 8-bit R, G, B, no alpha (24-bit TGA, DDS, surface format 20).
    R8G8B8,
    /// 8-bit alpha only (8-bit TGA, DDS A8, surface format 28).
    A8,
    /// 1-bit alpha, 5-bit R, G, B (16-bit TGA, DDS).
    A1R5G5B5,
    /// 5-bit R, G, B, top bit unused (15-bit TGA, DDS).
    X1R5G5B5,
    /// 8-bit R, G, B, top byte unused.
    X8R8G8B8,
    /// 5-bit R, 6-bit G, 5-bit B.
    R5G6B5,
    /// 4-bit R, G, B, top nibble unused.
    X4R4G4B4,
    /// 8-bit B, G, R, top byte unused.
    X8B8G8R8,
    /// 4-bit A, R, G, B.
    A4R4G4B4,
    /// 2-bit A, 10-bit B, G, R.
    A2B10G10R10,
    /// 8-bit A, B, G, R.
    A8B8G8R8,
    /// 2-bit A, 10-bit R, G, B.
    A2R10G10B10,
    /// The engine-specific DDS R8G8B8A8 format (§7.4): R in the top byte, A in the low byte.
    EngineR8G8B8A8,
    /// The engine-specific DDS B8G8R8 format (§7.4).
    EngineB8G8R8,
    /// DXT1 (BC1).
    Dxt1,
    /// DXT2 (premultiplied; decoded like DXT3, D26).
    Dxt2,
    /// DXT3 (BC2).
    Dxt3,
    /// DXT4 (premultiplied; decoded like DXT5, D26).
    Dxt4,
    /// DXT5 (BC3).
    Dxt5,
    /// JPEG (skin, plugin or embedded surface format 500).
    Jpeg,
}

impl PixelLayout {
    /// The source-format tag of decision 0015 §4.2 / D26.
    #[must_use]
    pub const fn tag(self) -> SourceTag {
        match self {
            Self::A8R8G8B8 => SourceTag::Argb32,
            Self::A8 => SourceTag::AlphaOnly,
            Self::R8G8B8
            | Self::X1R5G5B5
            | Self::X8R8G8B8
            | Self::R5G6B5
            | Self::X4R4G4B4
            | Self::X8B8G8R8
            | Self::EngineB8G8R8
            | Self::Jpeg => SourceTag::NoAlpha,
            Self::A1R5G5B5
            | Self::A4R4G4B4
            | Self::A2B10G10R10
            | Self::A8B8G8R8
            | Self::A2R10G10B10
            | Self::EngineR8G8B8A8
            | Self::Dxt1
            | Self::Dxt2
            | Self::Dxt3
            | Self::Dxt4
            | Self::Dxt5 => SourceTag::Other,
        }
    }
}

/// Source-format tag (decision 0015 §4.2): an overlay with an image draws only for
/// [`SourceTag::Argb32`] and [`SourceTag::AlphaOnly`] (`ui-layout.md` §9.6.3); the base
/// layer uses the pixels whatever the tag.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum SourceTag {
    /// Stored as A8R8G8B8.
    Argb32,
    /// Stored as an 8-bit alpha channel; pixels are black with that alpha.
    AlphaOnly,
    /// Stored without alpha; pixels have alpha 255.
    NoAlpha,
    /// Any other format (block-compressed, reordered or widened).
    Other,
}

/// A decoded UI image in the canonical form: `0xAARRGGBB` pixels, straight alpha, row
/// major, top row first, `width × height` of them.
#[derive(Clone, PartialEq, Eq)]
pub struct UiImage {
    width: u32,
    height: u32,
    source_width: u32,
    source_height: u32,
    layout: PixelLayout,
    pixels: Vec<u32>,
}

impl std::fmt::Debug for UiImage {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("UiImage")
            .field("width", &self.width)
            .field("height", &self.height)
            .field("source_width", &self.source_width)
            .field("source_height", &self.source_height)
            .field("layout", &self.layout)
            .field("pixels", &format_args!("[{} pixels]", self.pixels.len()))
            .finish()
    }
}

impl UiImage {
    /// Builds an image from canonical pixels. `None` unless `pixels.len()` is
    /// `width × height`, both sides are 1..=[`MAX_DECODED_DIMENSION`], and the source size
    /// is at least the decoded size on each axis and at most [`MAX_SOURCE_DIMENSION`].
    #[must_use]
    pub fn new(
        width: u32,
        height: u32,
        source_width: u32,
        source_height: u32,
        layout: PixelLayout,
        pixels: Vec<u32>,
    ) -> Option<Self> {
        let ok_axis = |d: u32, s: u32| {
            (1..=MAX_DECODED_DIMENSION).contains(&d) && d <= s && s <= MAX_SOURCE_DIMENSION
        };
        let len = u64::from(width) * u64::from(height);
        (ok_axis(width, source_width)
            && ok_axis(height, source_height)
            && pixels.len() as u64 == len)
            .then_some(Self {
                width,
                height,
                source_width,
                source_height,
                layout,
                pixels,
            })
    }

    /// Decoded width (at most [`MAX_DECODED_DIMENSION`]).
    #[must_use]
    pub fn width(&self) -> u32 {
        self.width
    }

    /// Decoded height (at most [`MAX_DECODED_DIMENSION`]).
    #[must_use]
    pub fn height(&self) -> u32 {
        self.height
    }

    /// Width declared by the file, before the crop.
    #[must_use]
    pub fn source_width(&self) -> u32 {
        self.source_width
    }

    /// Height declared by the file, before the crop.
    #[must_use]
    pub fn source_height(&self) -> u32 {
        self.source_height
    }

    /// Whether the crop of D25 removed part of the image.
    #[must_use]
    pub fn is_cropped(&self) -> bool {
        self.width != self.source_width || self.height != self.source_height
    }

    /// The layout the image was stored in.
    #[must_use]
    pub fn layout(&self) -> PixelLayout {
        self.layout
    }

    /// The source-format tag ([`PixelLayout::tag`]).
    #[must_use]
    pub fn tag(&self) -> SourceTag {
        self.layout.tag()
    }

    /// The `0xAARRGGBB` pixels, top row first.
    #[must_use]
    pub fn pixels(&self) -> &[u32] {
        &self.pixels
    }

    /// Takes the pixel buffer.
    #[must_use]
    pub fn into_pixels(self) -> Vec<u32> {
        self.pixels
    }

    /// The pixel at (`x`, `y`), or `None` outside the image.
    #[must_use]
    pub fn pixel(&self, x: u32, y: u32) -> Option<u32> {
        if x >= self.width || y >= self.height {
            return None;
        }
        let i = usize::try_from(u64::from(y) * u64::from(self.width) + u64::from(x)).ok()?;
        self.pixels.get(i).copied()
    }

    /// Bytes held by the pixel buffer (for cache budgets).
    #[must_use]
    pub fn byte_size(&self) -> usize {
        self.pixels.len() * 4
    }

    /// The pixels as RGBA8 bytes (R, G, B, A per pixel), e.g. for a GPU upload.
    #[must_use]
    pub fn to_rgba8(&self) -> Vec<u8> {
        self.pixels
            .iter()
            .flat_map(|&p| {
                let [b, g, r, a] = p.to_le_bytes();
                [r, g, b, a]
            })
            .collect()
    }
}

/// Packs 8-bit channels into `0xAARRGGBB`.
#[inline]
pub(crate) const fn argb(a: u8, r: u8, g: u8, b: u8) -> u32 {
    (a as u32) << 24 | (r as u32) << 16 | (g as u32) << 8 | b as u32
}

/// Widens a `bits`-wide channel value to 8 bits: bit replication for 1–7 bits, the top 8
/// bits for wider channels.
#[inline]
pub(crate) const fn widen(value: u32, bits: u32) -> u8 {
    if bits == 0 {
        return 0;
    }
    if bits >= 8 {
        return ((value >> (bits - 8)) & 0xFF) as u8;
    }
    let v = value & ((1 << bits) - 1);
    // Repeat the bit pattern from the top bit down until all 8 bits are filled.
    let mut out = 0u32;
    let mut filled = 0;
    while filled < 8 {
        let room = 8 - filled;
        out |= if room >= bits {
            v << (room - bits)
        } else {
            v >> (bits - room)
        };
        filled += bits;
    }
    (out & 0xFF) as u8
}

/// Rejects empty and oversized files.
pub(crate) fn check_file_len(bytes: &[u8]) -> Result<()> {
    if bytes.is_empty() {
        return Err(UiImageError::Empty);
    }
    if bytes.len() > MAX_FILE_BYTES {
        return Err(UiImageError::FileTooLarge {
            len: bytes.len(),
            max: MAX_FILE_BYTES,
        });
    }
    Ok(())
}

/// Checks a declared source size against D4.
pub(crate) fn check_dims(width: u32, height: u32) -> Result<()> {
    let ok = |d: u32| (1..=MAX_SOURCE_DIMENSION).contains(&d);
    if ok(width) && ok(height) {
        Ok(())
    } else {
        Err(UiImageError::BadDimensions {
            width,
            height,
            max: MAX_SOURCE_DIMENSION,
        })
    }
}

/// Allocates a zeroed pixel buffer of `len` pixels with `try_reserve`.
pub(crate) fn alloc_pixels(len: usize) -> Result<Vec<u32>> {
    let mut v = Vec::new();
    v.try_reserve_exact(len).map_err(|_| UiImageError::Alloc {
        bytes: len as u64 * 4,
    })?;
    v.resize(len, 0);
    Ok(v)
}

/// The decoded size of a source size (the crop of D25).
pub(crate) fn cropped(width: u32, height: u32) -> (u32, u32) {
    (
        width.min(MAX_DECODED_DIMENSION),
        height.min(MAX_DECODED_DIMENSION),
    )
}

/// The final extension of `path`, ASCII lower-cased (empty when there is none).
fn extension(path: &Path) -> String {
    path.extension()
        .map(|e| e.to_string_lossy().to_ascii_lowercase())
        .unwrap_or_default()
}

/// The decoder for `path`, chosen by its final extension with ASCII case ignored
/// (`skin-definition.md` D7).
///
/// - [`ImageOrigin::Skin`]: `.dds`, `.jpg`, `.tga` (§7.1). `.iff` is not decoded in M5
///   (D15), nor are `.jpeg`, `.png` and `.bmp` (§7.1).
/// - [`ImageOrigin::Plugin`]: `.tga`, `.jpg` (`lua-plugin-api.md` §13.8, decision 0015
///   §4.1); `.png` and `.bmp` give [`UiImageError::RefusedPluginFormat`] (decision 0015
///   §4.3).
///
/// # Errors
///
/// [`UiImageError::UnsupportedFileType`] or [`UiImageError::RefusedPluginFormat`].
pub fn format_for_path(path: &Path, origin: ImageOrigin) -> Result<ImageFileFormat> {
    let ext = extension(path);
    match (origin, ext.as_str()) {
        (_, "tga") => Ok(ImageFileFormat::Tga),
        (_, "jpg") => Ok(ImageFileFormat::Jpeg),
        (ImageOrigin::Skin, "dds") => Ok(ImageFileFormat::Dds),
        (ImageOrigin::Plugin, "png" | "bmp") => {
            Err(UiImageError::RefusedPluginFormat { extension: ext })
        }
        _ => Err(UiImageError::UnsupportedFileType { extension: ext }),
    }
}

/// Decodes a file of a known container format.
///
/// # Errors
///
/// Whatever that format's decoder returns.
pub fn decode_as(format: ImageFileFormat, bytes: &[u8]) -> Result<UiImage> {
    match format {
        ImageFileFormat::Tga => decode_tga(bytes),
        ImageFileFormat::Dds => decode_dds(bytes),
        ImageFileFormat::Jpeg => decode_jpeg(bytes),
    }
}

/// Picks the decoder for `path` ([`format_for_path`]) and decodes `bytes`, the whole file.
///
/// # Errors
///
/// A dispatch error of [`format_for_path`] or the decoder's error.
pub fn decode_file(path: &Path, origin: ImageOrigin, bytes: &[u8]) -> Result<UiImage> {
    decode_as(format_for_path(path, origin)?, bytes)
}

/// What [`probe_header`] learns from a file's first bytes.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct ImageProbe {
    /// Container format.
    pub format: ImageFileFormat,
    /// `(width, height)` as declared by the file, when the probed bytes hold it. TGA and DDS
    /// always give it; a JPEG only when its frame header lies within the first
    /// [`PROBE_LEN`] bytes. A JPEG size is advisory: the probe's marker walk is
    /// independent of the decoder, and [`decode_jpeg`] checks the frame it decodes.
    pub dimensions: Option<(u32, u32)>,
}

/// Checks a file's format and size from its first bytes, without decoding, for the
/// call-time check of `SetBackground(path)` (decision 0015 §4.4 step 2).
///
/// Only the first [`PROBE_LEN`] bytes of `header` are looked at; pass at least that many
/// (fewer when the file is shorter). The format comes from the path ([`format_for_path`]),
/// and the bytes must agree with it:
///
/// - TGA: an 18-byte header with an accepted pixel depth, a width and height within D4, and
///   for a colour-mapped file an accepted entry size;
/// - JPEG: the start-of-image marker `FF D8 FF`; a frame header within the probed bytes
///   must declare a size within D4;
/// - DDS (skins only): the magic `DDS ` and a width and height within D4 (the pixel format
///   lies beyond 64 bytes and is checked by the decoder).
///
/// A probe that passes does not promise that the full decode succeeds (a corrupt JPEG
/// body is found only then, decision 0015 §4.4).
///
/// # Errors
///
/// The dispatch errors of [`format_for_path`], [`UiImageError::Empty`],
/// [`UiImageError::Truncated`] for a header shorter than its fixed part, and the header
/// errors of the formats' decoders.
pub fn probe_header(path: &Path, origin: ImageOrigin, header: &[u8]) -> Result<ImageProbe> {
    let format = format_for_path(path, origin)?;
    let head = header.get(..PROBE_LEN).unwrap_or(header);
    if head.is_empty() {
        return Err(UiImageError::Empty);
    }
    let dimensions = match format {
        ImageFileFormat::Tga => {
            let h = tga::Header::parse(head)?;
            Some((h.width, h.height))
        }
        ImageFileFormat::Dds => Some(dds::probe(head)?),
        ImageFileFormat::Jpeg => {
            if !head.starts_with(&[0xFF, 0xD8, 0xFF]) {
                return Err(UiImageError::NotJpeg);
            }
            match jpeg::frame_size(head) {
                Some((w, h)) => {
                    check_dims(w, h)?;
                    Some((w, h))
                }
                None => None,
            }
        }
    };
    Ok(ImageProbe { format, dimensions })
}

/// Converts a render surface (0x41, `records.md` §5) to the canonical form of decision
/// 0015 §4.2: format 21 → *ARGB32*; format 28 → *alpha-only* (black, the value as alpha);
/// formats 20 and 500 (JPEG) → *no alpha*; DXT1, DXT3, DXT5 → *other*. The crop of D25
/// applies as for files. Decoding uses [`Surface::to_rgba8`] and its limits.
///
/// # Errors
///
/// [`UiImageError::Surface`] when the surface does not decode, and the dimension errors of
/// this module.
pub fn from_surface(surface: &Surface<'_>) -> Result<UiImage> {
    let layout = match surface.format {
        SurfaceFormat::A8R8G8B8 => PixelLayout::A8R8G8B8,
        SurfaceFormat::R8G8B8 => PixelLayout::R8G8B8,
        SurfaceFormat::A8 => PixelLayout::A8,
        SurfaceFormat::Jpeg => PixelLayout::Jpeg,
        SurfaceFormat::Dxt1 => PixelLayout::Dxt1,
        SurfaceFormat::Dxt3 => PixelLayout::Dxt3,
        SurfaceFormat::Dxt5 => PixelLayout::Dxt5,
    };
    let rgba = surface
        .to_rgba8()
        .map_err(|e| UiImageError::Surface(e.to_string()))?;
    let (w, h) = (rgba.width, rgba.height);
    check_dims(w, h)?;
    let expected = u64::from(w) * u64::from(h) * 4;
    if rgba.pixels.len() as u64 != expected {
        return Err(UiImageError::Surface(format!(
            "decoded {} bytes for {w}x{h}",
            rgba.pixels.len()
        )));
    }
    let (ow, oh) = cropped(w, h);
    let (ow_us, oh_us, w_us) = (ow as usize, oh as usize, w as usize);
    let mut pixels = alloc_pixels(ow_us * oh_us)?;
    for (y, out_row) in pixels.chunks_exact_mut(ow_us).enumerate() {
        let start = y * w_us * 4;
        let src = rgba.pixels.get(start..start + ow_us * 4).unwrap_or(&[]);
        for (dst, p) in out_row.iter_mut().zip(src.as_chunks::<4>().0) {
            *dst = if layout == PixelLayout::A8 {
                argb(p[3], 0, 0, 0)
            } else {
                argb(p[3], p[0], p[1], p[2])
            };
        }
    }
    UiImage::new(ow, oh, w, h, layout, pixels)
        .ok_or_else(|| UiImageError::Surface("inconsistent decoded size".into()))
}

/// Side of the placeholder image, in pixels (`skin-definition.md` §6.5).
pub const PLACEHOLDER_SIZE: u32 = 32;

/// The placeholder's slate texel (R, G, B, A) = (76, 76, 102, 255) as `0xAARRGGBB`.
pub const PLACEHOLDER_SLATE: u32 = 0xFF4C_4C66;

/// The placeholder's white texel, as `0xAARRGGBB`.
pub const PLACEHOLDER_WHITE: u32 = 0xFFFF_FFFF;

/// The image shown for every image source that cannot be loaded (`skin-definition.md`
/// §6.5, D16; checked by SP12): 32 × 32 pixels of [`PLACEHOLDER_SLATE`], with
/// [`PLACEHOLDER_WHITE`] where x ≥ 29 or y ≥ 29 (a 3-pixel white strip along the right and
/// bottom edges, which repeats as a grid across an element). The original creates it in
/// its 32-bit UI format, so the layout is [`PixelLayout::A8R8G8B8`] (tag *ARGB32*).
#[must_use]
pub fn placeholder() -> UiImage {
    let n = PLACEHOLDER_SIZE;
    let pixels = (0..n * n)
        .map(|i| {
            if i % n >= 29 || i / n >= 29 {
                PLACEHOLDER_WHITE
            } else {
                PLACEHOLDER_SLATE
            }
        })
        .collect();
    UiImage {
        width: n,
        height: n,
        source_width: n,
        source_height: n,
        layout: PixelLayout::A8R8G8B8,
        pixels,
    }
}
