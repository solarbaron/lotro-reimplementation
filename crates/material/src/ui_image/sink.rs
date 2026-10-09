// SPDX-License-Identifier: GPL-3.0-or-later
//! A pixel writer that takes pixels in file order and keeps only the cropped top-left part
//! of the displayed image (D25), so no decoder ever holds more than the decoded size.

use super::{PixelLayout, Result, UiImage, UiImageError, alloc_pixels, cropped};

/// Collects the pixels of a `source_width × source_height` image whose file rows are
/// stored top-down or bottom-up, keeping the top-left
/// [`MAX_DECODED_DIMENSION`](super::MAX_DECODED_DIMENSION) square.
pub(crate) struct Sink {
    src_w: u64,
    src_h: u64,
    out_w: usize,
    out_h: usize,
    top_down: bool,
    pixels: Vec<u32>,
}

impl Sink {
    /// A sink for a source size already checked against D4.
    pub(crate) fn new(src_w: u32, src_h: u32, top_down: bool) -> Result<Self> {
        let (ow, oh) = cropped(src_w, src_h);
        let (out_w, out_h) = (ow as usize, oh as usize);
        Ok(Self {
            src_w: u64::from(src_w),
            src_h: u64::from(src_h),
            out_w,
            out_h,
            top_down,
            pixels: alloc_pixels(out_w * out_h)?,
        })
    }

    /// Total number of pixels of the source image.
    pub(crate) fn source_pixels(&self) -> u64 {
        self.src_w * self.src_h
    }

    /// Number of leading pixels of a file row that are kept.
    pub(crate) fn kept_columns(&self) -> usize {
        self.out_w
    }

    /// The output row of file row `file_row`, if it is kept.
    pub(crate) fn output_row(&self, file_row: u64) -> Option<usize> {
        if file_row >= self.src_h {
            return None;
        }
        let image_row = if self.top_down {
            file_row
        } else {
            self.src_h - 1 - file_row
        };
        usize::try_from(image_row).ok().filter(|&r| r < self.out_h)
    }

    /// The kept part of output row `row` (from [`Sink::output_row`]).
    pub(crate) fn row_mut(&mut self, row: usize) -> &mut [u32] {
        let start = row * self.out_w;
        self.pixels
            .get_mut(start..start + self.out_w)
            .unwrap_or_default()
    }

    /// Writes pixels `start .. start + count` of the file's pixel stream (pixel `p` is
    /// row `p / width`, column `p % width`). `value(i)` gives the `i`-th pixel of the range
    /// and is called **only for kept pixels**, in stream order: pixels that the crop drops
    /// and pixels past the end of the image are never looked at, so their stored values are
    /// neither converted nor checked. The first error of `value` is returned.
    pub(crate) fn write_with(
        &mut self,
        start: u64,
        count: u64,
        mut value: impl FnMut(u64) -> Result<u32>,
    ) -> Result<()> {
        let end = start.saturating_add(count).min(self.source_pixels());
        let out_w = self.out_w as u64;
        let mut p = start;
        while p < end {
            let file_row = p / self.src_w;
            let col = p % self.src_w;
            let take = (end - p).min(self.src_w - col);
            if col < out_w
                && let Some(row) = self.output_row(file_row)
            {
                let stop = (col + take).min(out_w);
                let out = self.row_mut(row);
                for c in col..stop {
                    // c < out_w, which fits usize.
                    if let Some(dst) = out.get_mut(c as usize) {
                        *dst = value(p - start + (c - col))?;
                    }
                }
            }
            p += take;
        }
        Ok(())
    }

    /// Finishes the image.
    pub(crate) fn finish(self, layout: PixelLayout) -> Result<UiImage> {
        let (w, h) = (self.src_w as u32, self.src_h as u32);
        let (ow, oh) = cropped(w, h);
        UiImage::new(ow, oh, w, h, layout, self.pixels).ok_or(UiImageError::BadDimensions {
            width: w,
            height: h,
            max: super::MAX_SOURCE_DIMENSION,
        })
    }
}
