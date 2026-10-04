// SPDX-License-Identifier: GPL-3.0-or-later
//! Pixel operations for the texture viewer: channel selection, size limiting and the
//! alpha checkerboard. They run on asset workers, never on the UI thread.

use eframe::egui::{Color32, ColorImage};
use tessera_formats::RgbaImage;

/// Which channels of a straight-alpha RGBA8 image are shown.
///
/// * One channel selected: shown as opaque greyscale.
/// * Otherwise: unselected colour channels are 0; alpha is the image's alpha when `a` is
///   selected and opaque when not.
///
/// For single-channel A8 surfaces (decoded as white with alpha = channel,
/// `records.md` §5.3) "A only" is the greyscale view and "RGBA" the as-alpha view that
/// `texture-list.md` §7.3 asks for.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub struct ChannelMask {
    /// Red.
    pub r: bool,
    /// Green.
    pub g: bool,
    /// Blue.
    pub b: bool,
    /// Alpha.
    pub a: bool,
}

impl Default for ChannelMask {
    fn default() -> Self {
        Self::RGBA
    }
}

impl ChannelMask {
    /// All four channels.
    pub const RGBA: Self = Self {
        r: true,
        g: true,
        b: true,
        a: true,
    };

    /// Alpha only (greyscale).
    pub const ALPHA: Self = Self {
        r: false,
        g: false,
        b: false,
        a: true,
    };

    /// The mask as 4 bits (R = 1, G = 2, B = 4, A = 8).
    #[must_use]
    pub fn bits(self) -> u8 {
        u8::from(self.r) | u8::from(self.g) << 1 | u8::from(self.b) << 2 | u8::from(self.a) << 3
    }

    /// The mask from [`ChannelMask::bits`]; higher bits are ignored.
    #[must_use]
    pub fn from_bits(bits: u8) -> Self {
        Self {
            r: bits & 1 != 0,
            g: bits & 2 != 0,
            b: bits & 4 != 0,
            a: bits & 8 != 0,
        }
    }

    /// The single selected channel index (0–3), if exactly one is selected.
    #[must_use]
    pub fn single(self) -> Option<usize> {
        let sel = [self.r, self.g, self.b, self.a];
        let mut found = None;
        for (i, on) in sel.into_iter().enumerate() {
            if on {
                if found.is_some() {
                    return None;
                }
                found = Some(i);
            }
        }
        found
    }

    /// Applies the mask to one straight-alpha pixel, giving a straight-alpha pixel.
    #[must_use]
    pub fn apply(self, p: [u8; 4]) -> [u8; 4] {
        if let Some(i) = self.single() {
            let v = p[i];
            return [v, v, v, 255];
        }
        if self.bits() == 0 {
            return [0, 0, 0, 255];
        }
        [
            if self.r { p[0] } else { 0 },
            if self.g { p[1] } else { 0 },
            if self.b { p[2] } else { 0 },
            if self.a { p[3] } else { 255 },
        ]
    }
}

/// Converts a decoded straight-alpha image to an egui (premultiplied) image with `mask`
/// applied, shrinking it by an integer factor (nearest neighbour) when a side exceeds
/// `max_side`. Returns the image and the shrink factor (1 = full size).
///
/// A pixel buffer shorter than `width × height × 4` is padded with transparent pixels
/// instead of failing (decoders guarantee the length, but the viewer stays panic-free).
#[must_use]
pub fn to_color_image(img: &RgbaImage, mask: ChannelMask, max_side: usize) -> (ColorImage, u32) {
    let (w, h) = (img.width as usize, img.height as usize);
    let max_side = max_side.max(1);
    let factor = w.max(h).div_ceil(max_side).max(1);
    let (ow, oh) = (w.div_ceil(factor), h.div_ceil(factor));
    let mut pixels = Vec::with_capacity(ow.saturating_mul(oh));
    for y in 0..oh {
        let sy = y * factor;
        for x in 0..ow {
            let sx = x * factor;
            let i = (sy * w + sx) * 4;
            let p = img
                .pixels
                .get(i..i + 4)
                .and_then(|s| <[u8; 4]>::try_from(s).ok())
                .unwrap_or([0; 4]);
            let [r, g, b, a] = mask.apply(p);
            pixels.push(Color32::from_rgba_unmultiplied(r, g, b, a));
        }
    }
    let factor = u32::try_from(factor).unwrap_or(u32::MAX);
    (ColorImage::new([ow, oh], pixels), factor)
}

/// Converts a straight-alpha image to an egui image without masking or shrinking
/// beyond `max_side` (for icons).
#[must_use]
pub fn icon_image(img: &RgbaImage, max_side: usize) -> ColorImage {
    to_color_image(img, ChannelMask::RGBA, max_side).0
}

/// A 2×2 checkerboard used, repeated, behind transparent images.
#[must_use]
pub fn checkerboard() -> ColorImage {
    let a = Color32::from_gray(0x99);
    let b = Color32::from_gray(0x66);
    ColorImage::new([2, 2], vec![a, b, b, a])
}

/// Simple per-channel statistics of a straight-alpha image, for the texture info panel.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub struct ChannelStats {
    /// Minimum per channel.
    pub min: [u8; 4],
    /// Maximum per channel.
    pub max: [u8; 4],
}

impl ChannelStats {
    /// Computes the statistics; an empty image gives all zeros.
    #[must_use]
    pub fn of(img: &RgbaImage) -> Self {
        let mut min = [u8::MAX; 4];
        let mut max = [0u8; 4];
        let mut any = false;
        for p in img.pixels.chunks_exact(4) {
            any = true;
            for c in 0..4 {
                min[c] = min[c].min(p[c]);
                max[c] = max[c].max(p[c]);
            }
        }
        if !any {
            return Self::default();
        }
        Self { min, max }
    }

    /// Whether every pixel is fully opaque.
    #[must_use]
    pub fn opaque(&self) -> bool {
        self.min[3] == 255
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn img(w: u32, h: u32, f: impl Fn(u32, u32) -> [u8; 4]) -> RgbaImage {
        let mut pixels = Vec::new();
        for y in 0..h {
            for x in 0..w {
                pixels.extend_from_slice(&f(x, y));
            }
        }
        RgbaImage {
            width: w,
            height: h,
            pixels,
        }
    }

    #[test]
    fn mask_bits_round_trip() {
        for bits in 0..16u8 {
            assert_eq!(ChannelMask::from_bits(bits).bits(), bits);
        }
        assert_eq!(ChannelMask::RGBA.bits(), 15);
        assert_eq!(ChannelMask::ALPHA.single(), Some(3));
        assert_eq!(ChannelMask::RGBA.single(), None);
    }

    #[test]
    fn mask_application() {
        let p = [10, 20, 30, 40];
        assert_eq!(ChannelMask::RGBA.apply(p), p);
        assert_eq!(ChannelMask::ALPHA.apply(p), [40, 40, 40, 255]);
        assert_eq!(ChannelMask::from_bits(2).apply(p), [20, 20, 20, 255]);
        assert_eq!(ChannelMask::from_bits(0b0111).apply(p), [10, 20, 30, 255]);
        assert_eq!(ChannelMask::from_bits(0b1001).apply(p), [10, 0, 0, 40]);
        assert_eq!(ChannelMask::from_bits(0).apply(p), [0, 0, 0, 255]);
    }

    #[test]
    fn conversion_and_shrink() {
        let i = img(4, 2, |x, y| [x as u8, y as u8, 7, 255]);
        let (c, f) = to_color_image(&i, ChannelMask::RGBA, 16);
        assert_eq!(f, 1);
        assert_eq!(c.size, [4, 2]);
        assert_eq!(c.pixels[5], Color32::from_rgb(1, 1, 7));
        let (c, f) = to_color_image(&i, ChannelMask::RGBA, 2);
        assert_eq!(f, 2);
        assert_eq!(c.size, [2, 1]);
        assert_eq!(c.pixels[1], Color32::from_rgb(2, 0, 7));
        // Premultiplied output for straight-alpha input.
        let t = img(1, 1, |_, _| [255, 0, 0, 0]);
        let (c, _) = to_color_image(&t, ChannelMask::RGBA, 8);
        assert_eq!(c.pixels[0].a(), 0);
        assert_eq!(c.pixels[0].r(), 0);
    }

    #[test]
    fn short_buffer_is_padded() {
        let bad = RgbaImage {
            width: 3,
            height: 3,
            pixels: vec![1, 2, 3, 4],
        };
        let (c, _) = to_color_image(&bad, ChannelMask::RGBA, 0);
        assert_eq!(c.size, [1, 1]);
        let (c, _) = to_color_image(&bad, ChannelMask::RGBA, 8);
        assert_eq!(c.pixels.len(), 9);
        assert_eq!(c.pixels[8], Color32::TRANSPARENT);
    }

    #[test]
    fn stats() {
        let i = img(2, 1, |x, _| [x as u8 * 100, 5, 6, 200 + x as u8]);
        let s = ChannelStats::of(&i);
        assert_eq!(s.min, [0, 5, 6, 200]);
        assert_eq!(s.max, [100, 5, 6, 201]);
        assert!(!s.opaque());
        assert_eq!(
            ChannelStats::of(&img(0, 0, |_, _| [0; 4])),
            ChannelStats::default()
        );
        assert_eq!(checkerboard().size, [2, 2]);
    }
}
