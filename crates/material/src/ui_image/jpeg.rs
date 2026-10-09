// SPDX-License-Identifier: GPL-3.0-or-later
//! JPEG decoder (`skin-definition.md` §7.3; plugin JPEG files, `lua-plugin-api.md` §13.8),
//! on top of the `zune-jpeg` crate.

use tessera_formats::surface::{MAX_JPEG_DECODE_BYTES, jpeg_decode_bytes, jpeg_pixel_budget};
use zune_jpeg::JpegDecoder;
use zune_jpeg::zune_core::bytestream::ZCursor;
use zune_jpeg::zune_core::colorspace::ColorSpace;
use zune_jpeg::zune_core::options::DecoderOptions;

use super::{
    MAX_SOURCE_DIMENSION, PixelLayout, Result, UiImage, UiImageError, alloc_pixels, argb,
    check_dims, check_file_len, cropped,
};

/// Most progressive scans accepted in one stream (a bound on decode time; the decoder's
/// own default, stated here so it cannot change with a dependency update).
const MAX_SCANS: usize = 100;

/// The frame size from the first start-of-frame marker, read without decoding, for the
/// header probe. `None` when the bytes end first, a segment length is below 2, or
/// a scan or the end of the image comes before any frame header.
///
/// The walk is at least as tolerant as the decoder: bytes other than `FF` between
/// segments are skipped up to the next `FF` (libjpeg resynchronises with a warning,
/// `skin-definition.md` §7.3 OPEN (b)), and markers without a length (`FF 00`, TEM, RSTn,
/// a repeated SOI) are skipped. [`decode_jpeg`] does not rely on this walk: it takes the
/// frame from the decoder itself.
pub(crate) fn frame_size(data: &[u8]) -> Option<(u32, u32)> {
    if !data.starts_with(&[0xFF, 0xD8]) {
        return None;
    }
    let mut pos = 2usize;
    loop {
        // Resynchronise on the next FF, then skip fill bytes.
        while *data.get(pos)? != 0xFF {
            pos += 1;
        }
        while data.get(pos) == Some(&0xFF) {
            pos += 1;
        }
        let marker = *data.get(pos)?;
        pos += 1;
        // A stuffed zero or a standalone marker: no length field follows.
        if matches!(marker, 0x00 | 0x01 | 0xD0..=0xD8) {
            continue;
        }
        let len = usize::from(u16::from_be_bytes([*data.get(pos)?, *data.get(pos + 1)?]));
        if len < 2 {
            return None;
        }
        match marker {
            // Every SOFn; C4, C8 and CC are other markers.
            0xC0..=0xCF if !matches!(marker, 0xC4 | 0xC8 | 0xCC) => {
                if len < 8 {
                    return None;
                }
                let seg = data.get(pos + 2..pos + 7)?;
                let height = u32::from(u16::from_be_bytes([seg[1], seg[2]]));
                let width = u32::from(u16::from_be_bytes([seg[3], seg[4]]));
                return Some((width, height));
            }
            0xDA | 0xD9 => return None,
            _ => pos = pos.checked_add(len)?,
        }
    }
}

/// Estimated heap bytes for decoding a `width` × `height` frame of `components` components
/// and cropping it (D25). Every frame is counted as progressive ([`jpeg_decode_bytes`] with
/// `progressive = true`): the decoder also keeps full-frame coefficient buffers for a
/// baseline frame whose first scan does not hold every component, which is not known
/// before the scans are read. The cropped output (4 bytes per kept pixel), allocated while
/// the decoder's output is still alive, is added.
pub(crate) fn decode_estimate(width: u32, height: u32, components: u8) -> u64 {
    let (ow, oh) = cropped(width, height);
    jpeg_decode_bytes(width, height, components, true)
        .saturating_add(u64::from(ow) * u64::from(oh) * 4)
}

fn jpeg_err(e: impl std::fmt::Display) -> UiImageError {
    UiImageError::Jpeg(e.to_string())
}

/// Decodes a whole JPEG file to an opaque image (tag *no alpha*, `skin-definition.md`
/// §7.3): baseline and progressive streams, greyscale or colour, converted to RGB.
///
/// The full frame is decoded and then cropped to its top-left
/// [`MAX_DECODED_DIMENSION`](super::MAX_DECODED_DIMENSION) square (D25), so before any
/// pixel buffer is allocated the frame the decoder read must fit two budgets:
///
/// - estimated decode memory of at most [`MAX_JPEG_DECODE_BYTES`] (256 MiB): the
///   `tessera-formats` estimate [`jpeg_decode_bytes`] for a **progressive** frame, whatever
///   the stream's frame type, plus the cropped output. For three components that is about
///   14 bytes per pixel up to 4,096 × 4,096 and 10 bytes per pixel plus 64 MiB above, so a
///   square three-component frame stops near 4,480 × 4,480;
/// - at most [`jpeg_pixel_budget`] pixels for the stream's length: the smaller of
///   [`MAX_JPEG_PIXELS`](tessera_formats::surface::MAX_JPEG_PIXELS) (32 Mi) and
///   1 Mi + 1,024 × the file length.
///
/// Larger frames fail with [`UiImageError::TooLarge`]. Bytes other than markers between
/// header segments are skipped, as libjpeg skips them with a warning. A stream whose
/// entropy-coded data ends early decodes with the decoder's filler for the missing part, as
/// libjpeg does. Arithmetic-coded streams are not supported by the decoder and fail
/// (`skin-definition.md` §7.3 OPEN).
///
/// # Errors
///
/// [`UiImageError`] for an empty, oversized or non-JPEG file, a frame size outside D4 or
/// the budgets, and any decoder error.
pub fn decode_jpeg(bytes: &[u8]) -> Result<UiImage> {
    check_file_len(bytes)?;
    if !bytes.starts_with(&[0xFF, 0xD8, 0xFF]) {
        return Err(UiImageError::NotJpeg);
    }
    let max = MAX_SOURCE_DIMENSION as usize;
    let options = DecoderOptions::default()
        .set_max_width(max)
        .set_max_height(max)
        .jpeg_set_max_scans(MAX_SCANS)
        .jpeg_set_out_colorspace(ColorSpace::RGB);
    let mut decoder = JpegDecoder::new_with_options(ZCursor::new(bytes), options);
    decoder.decode_headers().map_err(jpeg_err)?;
    let info = decoder
        .info()
        .ok_or_else(|| UiImageError::Jpeg("no frame header".into()))?;
    let (width, height) = (u32::from(info.width), u32::from(info.height));
    check_dims(width, height)?;
    // The budget inputs come from the frame the decoder decodes, never from a second walk
    // of the stream that could find a different frame.
    let pixels = u64::from(width) * u64::from(height);
    if decode_estimate(width, height, info.components) > MAX_JPEG_DECODE_BYTES
        || pixels > jpeg_pixel_budget(bytes.len())
    {
        return Err(UiImageError::TooLarge { width, height });
    }
    let expected = pixels * 3;
    let size = decoder
        .output_buffer_size()
        .ok_or_else(|| UiImageError::Jpeg("no output size".into()))?;
    if size as u64 != expected {
        return Err(UiImageError::Jpeg(format!(
            "decoder output of {size} bytes for {width}x{height} RGB"
        )));
    }
    let mut rgb = Vec::new();
    rgb.try_reserve_exact(size)
        .map_err(|_| UiImageError::Alloc { bytes: expected })?;
    rgb.resize(size, 0);
    decoder.decode_into(&mut rgb).map_err(jpeg_err)?;

    let (ow, oh) = cropped(width, height);
    let (ow_us, w_us) = (ow as usize, width as usize);
    let mut out = alloc_pixels(ow_us * oh as usize)?;
    for (y, out_row) in out.chunks_exact_mut(ow_us).enumerate() {
        let start = y * w_us * 3;
        let src = rgb.get(start..start + ow_us * 3).unwrap_or(&[]);
        for (dst, p) in out_row.iter_mut().zip(src.as_chunks::<3>().0) {
            *dst = argb(255, p[0], p[1], p[2]);
        }
    }
    drop(rgb);
    UiImage::new(ow, oh, width, height, PixelLayout::Jpeg, out)
        .ok_or_else(|| UiImageError::Jpeg("inconsistent decoded size".into()))
}
