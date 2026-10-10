// SPDX-License-Identifier: GPL-3.0-or-later
//! The pixel size of a render surface without decoding it (`records.md` §5): the header
//! fields, or for a JPEG surface (whose header says 0×0) the stream's frame header.

use tessera_formats::Surface;

/// The frame size `(width, height)` declared by a JPEG stream's start-of-frame marker,
/// read without decoding anything by the surface decoder's own frame-header reader
/// ([`tessera_formats::jpeg_frame`]). `None` when the stream does not start with a JPEG
/// start-of-image marker, a segment is malformed or truncated, or a scan starts before
/// any frame header; the decoder then rejects the stream too.
#[cfg(test)]
pub(crate) fn jpeg_frame_size(data: &[u8]) -> Option<(u32, u32)> {
    tessera_formats::jpeg_frame(data).map(|f| (u32::from(f.width), u32::from(f.height)))
}

/// The pixel size a surface decodes to: its header size, or for a JPEG surface the
/// stream's frame size (`None` when it cannot be read).
pub(crate) fn surface_dims(surface: &Surface<'_>) -> Option<(u32, u32)> {
    surface.decoded_size()
}
