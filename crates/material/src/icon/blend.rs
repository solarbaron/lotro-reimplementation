// SPDX-License-Identifier: GPL-3.0-or-later
//! Straight-alpha source-over blending and layer placement (`icon-layers.md` §3.3, §3.4).

use tessera_formats::RgbaImage;

use crate::error::{MaterialError, Result};

/// Blends one straight-alpha RGBA8 pixel `src` over `dst` with the source-over operator
/// (`icon-layers.md` §3.3).
///
/// With values scaled to [0, 1], the result alpha is *r* = *s* + *d*·(1 − *s*) and the
/// result colour is (*S*·*s* + *D*·*d*·(1 − *s*)) / *r*, or 0 when *r* = 0. The integer
/// form used here is exact up to one final round-to-nearest per channel, so it stays
/// within the spec's ±1 tolerance of the real-valued formula.
///
/// Blending happens on the stored (sRGB-encoded) values; whether the original client
/// blends in linear light is OPEN in the spec.
#[must_use]
pub fn source_over(dst: [u8; 4], src: [u8; 4]) -> [u8; 4] {
    let s = u32::from(src[3]);
    if s == 255 {
        return src;
    }
    let d = u32::from(dst[3]);
    if s == 0 {
        // r = d; a fully transparent result has colour 0 (§3.3).
        return if d == 0 { [0; 4] } else { dst };
    }
    let inv = 255 - s;
    // Result alpha scaled by 255²: s·255 + d·(255 − s). At most 65,025.
    let den = s * 255 + d * inv;
    if den == 0 {
        return [0; 4];
    }
    let mut out = [0u8; 4];
    for (o, (&sc, &dc)) in out.iter_mut().zip(src.iter().zip(dst.iter())).take(3) {
        // Colour numerator scaled the same way: S·s·255 + D·d·(255 − s). At most
        // 2 · 255³ < 2³².
        let num = u32::from(sc) * s * 255 + u32::from(dc) * d * inv;
        *o = clamp_u8((num + den / 2) / den);
    }
    out[3] = clamp_u8((den + 127) / 255);
    out
}

fn clamp_u8(v: u32) -> u8 {
    u8::try_from(v).unwrap_or(u8::MAX)
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
/// ⌊(canvas − layer) / 2⌋, negative when the layer is larger (`icon-layers.md` §3.4).
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
        for (dp, sp) in drow.chunks_exact_mut(4).zip(srow.chunks_exact(4)) {
            let d = [dp[0], dp[1], dp[2], dp[3]];
            let s = [sp[0], sp[1], sp[2], sp[3]];
            dp.copy_from_slice(&source_over(d, s));
        }
    }
    Ok(())
}

/// Draws `src` centred on `dst` (offset [`centre_offset`] on each axis), clipped.
pub fn draw_centred(dst: &mut RgbaImage, src: &RgbaImage) -> Result<()> {
    let x = centre_offset(dst.width, src.width);
    let y = centre_offset(dst.height, src.height);
    draw_over(dst, src, x, y)
}

/// Whether every pixel of `image` has alpha 0 (a layer that cannot change a composite).
#[must_use]
pub fn is_fully_transparent(image: &RgbaImage) -> bool {
    image.pixels.chunks_exact(4).all(|p| p[3] == 0)
}

#[cfg(test)]
mod tests {
    use super::*;

    /// The real-valued §3.3 formula, rounded to nearest.
    fn reference(dst: [u8; 4], src: [u8; 4]) -> [u8; 4] {
        let f = |v: u8| f64::from(v) / 255.0;
        let (s, d) = (f(src[3]), f(dst[3]));
        let r = s + d * (1.0 - s);
        let mut out = [0u8; 4];
        if r > 0.0 {
            for i in 0..3 {
                let c = (f(src[i]) * s + f(dst[i]) * d * (1.0 - s)) / r;
                out[i] = (c * 255.0).round() as u8;
            }
        }
        out[3] = (r * 255.0).round() as u8;
        out
    }

    #[test]
    fn opaque_and_transparent_sources() {
        let dst = [10, 20, 30, 200];
        assert_eq!(source_over(dst, [1, 2, 3, 255]), [1, 2, 3, 255]);
        assert_eq!(source_over(dst, [1, 2, 3, 0]), dst);
        assert_eq!(source_over([0; 4], [0; 4]), [0; 4]);
        assert_eq!(source_over([9, 9, 9, 0], [1, 2, 3, 0]), [0; 4]);
    }

    #[test]
    fn half_alpha_over_opaque() {
        // 128/255 white over opaque black: colour ≈ 128, alpha 255.
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
    fn matches_reference_within_one() {
        // A coarse sweep over every channel class; the proptest below covers the rest.
        let vals = [0u8, 1, 2, 63, 127, 128, 129, 200, 254, 255];
        for &sa in &vals {
            for &da in &vals {
                for &sc in &vals {
                    for &dc in &vals {
                        let src = [sc, 255 - sc, sc / 2, sa];
                        let dst = [dc, dc / 3, 255 - dc, da];
                        let got = source_over(dst, src);
                        let want = reference(dst, src);
                        for i in 0..4 {
                            assert!(
                                got[i].abs_diff(want[i]) <= 1,
                                "{dst:?} under {src:?}: got {got:?}, want {want:?}"
                            );
                        }
                    }
                }
            }
        }
    }

    proptest::proptest! {
        #[test]
        fn prop_matches_reference(dst: [u8; 4], src: [u8; 4]) {
            let got = source_over(dst, src);
            let want = reference(dst, src);
            for i in 0..4 {
                proptest::prop_assert!(got[i].abs_diff(want[i]) <= 1, "{:?} {:?} {:?} {:?}", dst, src, got, want);
            }
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
            .chunks_exact(4)
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
