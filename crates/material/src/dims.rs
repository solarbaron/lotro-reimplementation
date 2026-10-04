// SPDX-License-Identifier: GPL-3.0-or-later
//! The pixel size of a render surface without decoding it (`records.md` §5): the header
//! fields, or for a JPEG surface (whose header says 0×0) the stream's frame header.

use tessera_formats::Surface;

/// The frame size `(width, height)` declared by a JPEG stream's start-of-frame marker,
/// read without decoding anything. `None` when the stream does not start with a JPEG
/// start-of-image marker, a segment is malformed or truncated, or a scan starts before
/// any frame header; the decoder then reports its own error.
pub(crate) fn jpeg_frame_size(data: &[u8]) -> Option<(u32, u32)> {
    if !data.starts_with(&[0xFF, 0xD8]) {
        return None;
    }
    let mut pos = 2usize;
    loop {
        if *data.get(pos)? != 0xFF {
            return None;
        }
        // Fill bytes: any number of FF before the marker code.
        while data.get(pos) == Some(&0xFF) {
            pos += 1;
        }
        let marker = *data.get(pos)?;
        pos += 1;
        if matches!(marker, 0x01 | 0xD0..=0xD7) {
            continue; // stand-alone markers carry no length
        }
        let len = usize::from(u16::from_be_bytes([*data.get(pos)?, *data.get(pos + 1)?]));
        if len < 2 {
            return None;
        }
        let segment = data.get(pos + 2..pos.checked_add(len)?)?;
        match marker {
            // Start of frame (every SOFn; C4, C8 and CC are other markers). The segment
            // holds the sample precision, then the height and width, big-endian.
            0xC0..=0xCF if !matches!(marker, 0xC4 | 0xC8 | 0xCC) => {
                let height = u16::from_be_bytes([*segment.get(1)?, *segment.get(2)?]);
                let width = u16::from_be_bytes([*segment.get(3)?, *segment.get(4)?]);
                return Some((u32::from(width), u32::from(height)));
            }
            0xDA | 0xD9 => return None,
            _ => pos += len,
        }
    }
}

/// The pixel size a surface decodes to: its header size, or for a JPEG surface the
/// stream's frame size ([`jpeg_frame_size`]; `None` when it cannot be read).
pub(crate) fn surface_dims(surface: &Surface<'_>) -> Option<(u32, u32)> {
    if surface.is_jpeg() {
        jpeg_frame_size(surface.data)
    } else {
        Some((surface.width, surface.height))
    }
}
