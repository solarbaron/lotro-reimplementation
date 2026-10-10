// SPDX-License-Identifier: GPL-3.0-or-later
//! The client's integer source-over blend and layer placement (`icon-layers.md` §3.3, §3.4).

use tessera_formats::RgbaImage;

use crate::error::{MaterialError, Result};

/// Blends one straight-alpha RGBA8 pixel `src` over the canvas pixel `dst` with the
/// client's integer source-over rule on stored values (`icon-layers.md` §3.3, normative).
///
/// With *s* the source alpha, *a* = *s* + 1, *d* the canvas alpha, *D* and *S* a canvas and
/// a source channel, and T(·) integer division truncated toward zero:
///
/// 1. *s* = 0: the canvas pixel is unchanged;
/// 2. *s* = 255: the canvas pixel becomes the source pixel (alpha 255);
/// 3. *d* = 0: the source pixel, colour and alpha *s*;
/// 4. *d* = 255 (also every canvas whose bottom layer has no alpha, R8G8B8 decoding to
///    alpha 255): each channel *D* − T((*D* − *S*)·*a* / 256), alpha 255;
/// 5. otherwise *q* = (*d* + 1) + *a* − T((*d* + 1)·*a* / 256): each channel
///    *D* − T((*D* − *S*)·*a* / *q*), alpha *q* − 1.
///
/// The constants 256 and the "+ 1" on alphas are the client's (decision 0012). No
/// linearisation: the values are the stored (sRGB-encoded) ones.
#[must_use]
pub fn source_over(dst: [u8; 4], src: [u8; 4]) -> [u8; 4] {
    let s = i32::from(src[3]);
    if s == 0 {
        return dst;
    }
    let a = s + 1;
    if a == 256 {
        return [src[0], src[1], src[2], 255];
    }
    let d = i32::from(dst[3]);
    if d == 0 {
        return src;
    }
    // |D − S| ≤ 255 and a ≤ 255, so every product stays below 2^16.
    let (den, alpha) = if d == 255 {
        (256, 255)
    } else {
        let q = (d + 1) + a - (d + 1) * a / 256;
        (q, q - 1)
    };
    let mut out = [0u8; 4];
    for (o, (&dc, &sc)) in out.iter_mut().zip(dst.iter().zip(src.iter())).take(3) {
        let (dc, sc) = (i32::from(dc), i32::from(sc));
        *o = clamp_u8(dc - (dc - sc) * a / den);
    }
    out[3] = clamp_u8(alpha);
    out
}

fn clamp_u8(v: i32) -> u8 {
    u8::try_from(v.clamp(0, 255)).unwrap_or(u8::MAX)
}

/// Checks that an image's pixel buffer holds exactly `width × height × 4` bytes.
pub fn validate_image(image: &RgbaImage) -> Result<()> {
    let expected = u64::from(image.width)
        .checked_mul(u64::from(image.height))
        .and_then(|p| p.checked_mul(4))
        .unwrap_or(u64::MAX);
    if image.pixels.len() as u64 != expected {
        return Err(MaterialError::InvalidImage {
            width: image.width,
            height: image.height,
            len: image.pixels.len(),
            expected,
        });
    }
    Ok(())
}

/// The offset that centres a layer of size `layer` on a canvas of size `canvas`:
/// ⌊(canvas − layer) / 2⌋, negative when the layer is larger.
///
/// Not used by the icon composition: the client places every layer at the canvas's
/// top-left corner (`icon-layers.md` §3.4, which withdrew the earlier centring rule). Kept
/// for tools that want a centred preview.
#[must_use]
pub fn centre_offset(canvas: u32, layer: u32) -> i64 {
    (i64::from(canvas) - i64::from(layer)).div_euclid(2)
}

/// Draws `src` over `dst` with [`source_over`], placing the top-left of `src` at
/// (`x`, `y`) in `dst` (either may be negative). Parts outside `dst` are clipped.
///
/// Both images must be valid ([`validate_image`]); an invalid one is an error and
/// leaves `dst` unchanged.
pub fn draw_over(dst: &mut RgbaImage, src: &RgbaImage, x: i64, y: i64) -> Result<()> {
    validate_image(dst)?;
    validate_image(src)?;
    let (dw, dh) = (i64::from(dst.width), i64::from(dst.height));
    let (sw, sh) = (i64::from(src.width), i64::from(src.height));
    // Visible rectangle in destination coordinates. Saturating: offsets may be any i64.
    let x0 = x.max(0);
    let y0 = y.max(0);
    let x1 = x.saturating_add(sw).min(dw);
    let y1 = y.saturating_add(sh).min(dh);
    if x0 >= x1 || y0 >= y1 {
        return Ok(());
    }
    // All coordinates below are within both images (checked above), and image sizes
    // fit in memory, so the usize conversions cannot fail; `get` keeps it panic-free
    // regardless.
    let to_usize = |v: i64| usize::try_from(v).unwrap_or(0);
    let row_len = to_usize(x1 - x0) * 4;
    for dy in y0..y1 {
        let sy = dy - y;
        let d_start = to_usize((dy * dw + x0) * 4);
        let s_start = to_usize((sy * sw + (x0 - x)) * 4);
        let (Some(drow), Some(srow)) = (
            dst.pixels.get_mut(d_start..d_start + row_len),
            src.pixels.get(s_start..s_start + row_len),
        ) else {
            continue;
        };
        for (dp, sp) in drow
            .as_chunks_mut::<4>()
            .0
            .iter_mut()
            .zip(srow.as_chunks::<4>().0)
        {
            *dp = source_over(*dp, *sp);
        }
    }
    Ok(())
}

/// Draws `src` centred on `dst` (offset [`centre_offset`] on each axis), clipped. Not the
/// composition rule (that is [`draw_over`] at (0, 0), `icon-layers.md` §3.4).
pub fn draw_centred(dst: &mut RgbaImage, src: &RgbaImage) -> Result<()> {
    let x = centre_offset(dst.width, src.width);
    let y = centre_offset(dst.height, src.height);
    draw_over(dst, src, x, y)
}

/// Whether every pixel of `image` has alpha 0 (a layer that cannot change a composite).
#[must_use]
pub fn is_fully_transparent(image: &RgbaImage) -> bool {
    image.pixels.as_chunks::<4>().0.iter().all(|p| p[3] == 0)
}

#[cfg(test)]
mod tests {
    use super::*;

    /// The §3.3 rule for a canvas without alpha (R8G8B8 bottom layer), written out per
    /// channel with floor division corrected toward zero (the spec's sweep fixture).
    fn rgb_rule(dc: u8, sc: u8, s: u8) -> u8 {
        if s == 0 {
            return dc;
        }
        let a = i64::from(s) + 1;
        if a == 256 {
            return sc;
        }
        let n = (i64::from(dc) - i64::from(sc)) * a;
        let t = if n < 0 { -((-n) / 256) } else { n / 256 };
        (i64::from(dc) - t) as u8
    }

    #[test]
    fn opaque_and_transparent_sources() {
        let dst = [10, 20, 30, 200];
        assert_eq!(source_over(dst, [1, 2, 3, 255]), [1, 2, 3, 255]);
        assert_eq!(source_over(dst, [1, 2, 3, 0]), dst);
        assert_eq!(source_over([0; 4], [0; 4]), [0; 4]);
        // §3.3 step 1: alpha 0 leaves the canvas pixel unchanged (IL7).
        assert_eq!(source_over([9, 9, 9, 0], [1, 2, 3, 0]), [9, 9, 9, 0]);
    }

    #[test]
    fn spec_worked_examples() {
        // §3.3 table, examples 1–6 (R8G8B8 canvases decode to alpha 255).
        assert_eq!(
            source_over([200, 100, 50, 255], [0, 0, 255, 128]),
            [100, 50, 153, 255]
        );
        assert_eq!(
            source_over([100, 100, 100, 100], [200, 0, 0, 100]),
            [161, 39, 39, 162]
        );
        assert_eq!(
            source_over([10, 20, 30, 255], [250, 0, 0, 64]),
            [70, 15, 23, 255]
        );
        assert_eq!(source_over([50, 60, 70, 0], [1, 2, 3, 77]), [1, 2, 3, 77]);
        assert_eq!(source_over([5, 6, 7, 99], [1, 2, 3, 0]), [5, 6, 7, 99]);
        assert_eq!(source_over([5, 6, 7, 99], [9, 8, 7, 255]), [9, 8, 7, 255]);
        assert_eq!(source_over([5, 6, 7, 255], [9, 8, 7, 255]), [9, 8, 7, 255]);
    }

    #[test]
    fn half_alpha_over_opaque() {
        // 128/255 white over opaque black: a = 129, 0 − T(−255·129/256) = 128.
        let out = source_over([0, 0, 0, 255], [255, 255, 255, 128]);
        assert_eq!(out, [128, 128, 128, 255]);
    }

    #[test]
    fn transparent_destination_keeps_source_colour() {
        // Straight alpha: drawing onto an empty canvas keeps the colour and alpha as is.
        for a in [1u8, 7, 128, 254] {
            assert_eq!(source_over([0; 4], [200, 100, 50, a]), [200, 100, 50, a]);
        }
    }

    #[test]
    fn sweep_on_canvas_without_alpha() {
        // §8: every (D, S, s), D, S ∈ {0, 1, 127, 128, 254, 255}, s 0–255, on an R8G8B8
        // canvas (truncation toward zero, not toward −∞, when D < S).
        let vals = [0u8, 1, 127, 128, 254, 255];
        for &dc in &vals {
            for &sc in &vals {
                for s in 0..=255u8 {
                    let got = source_over([dc, dc, dc, 255], [sc, sc, sc, s]);
                    let want = rgb_rule(dc, sc, s);
                    assert_eq!(got, [want, want, want, 255], "D {dc} S {sc} s {s}");
                }
            }
        }
    }

    proptest::proptest! {
        #[test]
        fn prop_alpha_never_drops_and_stays_in_range(dst: [u8; 4], src: [u8; 4]) {
            let got = source_over(dst, src);
            // The result is at least as opaque as the canvas and the source.
            proptest::prop_assert!(got[3] >= dst[3] || src[3] == 0);
            proptest::prop_assert!(src[3] == 0 || got[3] >= src[3]);
        }
    }

    fn solid(w: u32, h: u32, px: [u8; 4]) -> RgbaImage {
        RgbaImage {
            width: w,
            height: h,
            pixels: px.repeat((w * h) as usize),
        }
    }

    #[test]
    fn offsets() {
        assert_eq!(centre_offset(4, 2), 1);
        assert_eq!(centre_offset(4, 6), -1);
        assert_eq!(centre_offset(32, 32), 0);
        assert_eq!(centre_offset(32, 35), -2); // ⌊−3/2⌋ = −2
        assert_eq!(centre_offset(32, 28), 2);
    }

    #[test]
    fn draw_clips_on_every_side() {
        let mut dst = solid(4, 4, [0, 0, 0, 255]);
        let src = solid(2, 2, [255, 0, 0, 255]);
        for (x, y) in [
            (-1, -1),
            (3, 3),
            (-5, 0),
            (0, 9),
            (i64::MIN / 4, i64::MAX / 4),
            // Offsets at the ends of the range must clip, not overflow.
            (i64::MAX, 0),
            (0, i64::MAX),
            (i64::MAX - 1, i64::MAX - 1),
            (i64::MIN, 0),
            (0, i64::MIN),
            (i64::MIN, i64::MIN),
            (i64::MIN, i64::MAX),
        ] {
            draw_over(&mut dst, &src, x, y).unwrap();
        }
        let red: Vec<usize> = dst
            .pixels
            .as_chunks::<4>()
            .0
            .iter()
            .enumerate()
            .filter(|(_, p)| p[0] == 255)
            .map(|(i, _)| i)
            .collect();
        assert_eq!(red, vec![0, 15]);
    }

    #[test]
    fn invalid_images_are_rejected() {
        let mut dst = solid(2, 2, [0; 4]);
        let bad = RgbaImage {
            width: 3,
            height: 3,
            pixels: vec![0; 5],
        };
        assert!(matches!(
            draw_over(&mut dst, &bad, 0, 0),
            Err(MaterialError::InvalidImage { .. })
        ));
        let huge = RgbaImage {
            width: u32::MAX,
            height: u32::MAX,
            pixels: vec![],
        };
        assert!(validate_image(&huge).is_err());
    }
}
