// SPDX-License-Identifier: GPL-3.0-or-later
//! The per-pixel arithmetic of the compositor (`ui-layout.md` §9.6.1–§9.6.3) and of text
//! compositing (`fonts.md` §8.4).
//!
//! Every function here is a pure function of one destination pixel and one source pixel, in
//! the canonical `0xAARRGGBB` form with straight alpha. All arithmetic is in 32-bit signed
//! integers with division rounded toward zero, as the specs state; results are bytes.

/// The seven compositor operations of `ui-layout.md` §9.6.1 (this crate's names).
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum CompositeOp {
    /// Draw mode 1.
    Colorize,
    /// Draw mode 2, and draw modes 0 and above 8.
    Replace,
    /// Draw mode 3.
    Multiply,
    /// Draw mode 4 ("Alphablend").
    Over,
    /// Draw mode 5 ("Overlay"): a colour mix that keeps the destination's alpha.
    MixColour,
    /// Draw mode 6. The base layer has no routine for it and draws nothing.
    Grayscale,
    /// Draw mode 7.
    Screen,
}

impl CompositeOp {
    /// Maps a `DrawModeType` value to its operation (`ui-layout.md` §9.6.1). Draw mode 8
    /// ("None") gives `None`: the item clears the layer instead of drawing it. Draw mode 0
    /// and every value above 8 give [`CompositeOp::Replace`].
    #[must_use]
    pub const fn from_draw_mode(mode: u32) -> Option<Self> {
        Some(match mode {
            1 => Self::Colorize,
            3 => Self::Multiply,
            4 => Self::Over,
            5 => Self::MixColour,
            6 => Self::Grayscale,
            7 => Self::Screen,
            8 => return None,
            _ => Self::Replace,
        })
    }
}

/// Alpha byte of a pixel.
#[inline]
#[must_use]
pub const fn alpha(p: u32) -> i32 {
    (p >> 24) as i32
}

#[inline]
const fn ch(p: u32, shift: u32) -> i32 {
    ((p >> shift) & 0xFF) as i32
}

#[inline]
const fn pack(a: i32, r: i32, g: i32, b: i32) -> u32 {
    ((a as u32 & 0xFF) << 24)
        | ((r as u32 & 0xFF) << 16)
        | ((g as u32 & 0xFF) << 8)
        | (b as u32 & 0xFF)
}

/// inc(*x*): 0 for 0, otherwise *x* + 1.
#[inline]
const fn inc(x: i32) -> i32 {
    x + (x != 0) as i32
}

/// dec(*x*): 0 for 0, otherwise *x* − 1.
#[inline]
const fn dec(x: i32) -> i32 {
    if x == 0 { 0 } else { x - 1 }
}

/// div256(*x*): *x* ÷ 256 rounded toward zero.
#[inline]
const fn div256(x: i32) -> i32 {
    x / 256
}

/// mix(*d*, *x*, *k*) = *d* − div256((*d* − *x*) × *k*), low 8 bits kept by the caller's pack.
#[inline]
const fn mix(d: i32, x: i32, k: i32) -> i32 {
    d - div256((d - x) * k)
}

/// The opacity byte *o* of `ui-layout.md` §9.6.2: trunc(`alphablendmod` × 255.0) in 32-bit
/// float, after clamping the property to 0–1. NaN gives 0 (draws nothing).
#[must_use]
pub fn opacity_byte(alphablendmod: f32) -> u8 {
    if alphablendmod.is_nan() {
        return 0;
    }
    // Truncation toward zero is the spec's rule; the value is within 0..=255.
    (alphablendmod.clamp(0.0, 1.0) * 255.0) as u8
}

/// Packs a colour of four floats (red, green, blue, alpha; 0.0–1.0) into `0xAARRGGBB` by
/// multiplying each by 255 and truncating toward zero (`fonts.md` §8.4). Values outside
/// 0–1 are clamped first and NaN counts as 0.
#[must_use]
pub fn pack_colour(rgba: [f32; 4]) -> u32 {
    let b = |v: f32| -> i32 {
        if v.is_nan() {
            0
        } else {
            (v.clamp(0.0, 1.0) * 255.0) as i32
        }
    };
    pack(b(rgba[3]), b(rgba[0]), b(rgba[1]), b(rgba[2]))
}

/// Weight rule W1 (Mix colour, Over, Screen): *k*, or 0 when the pixel stays unchanged
/// (a mix with *k* = 0 leaves every channel as it is, so callers need no branch).
#[inline]
const fn w1(sa: i32, o: i32) -> i32 {
    let a = if o == 255 { sa } else { div256(sa * (o + 1)) };
    a + (a != 0) as i32
}

/// Weight rule W2 (Multiply, Colorize): *k*, 0 when the pixel stays unchanged.
#[inline]
const fn w2(sa: i32, o: i32) -> i32 {
    let i = sa + (sa != 0) as i32;
    if o == 255 { i } else { div256(i * (o + 1)) }
}

/// Mixes each colour channel of `d` toward `x` by `k`, keeping `d`'s alpha.
#[inline]
const fn mix_channels(d: u32, xr: i32, xg: i32, xb: i32, k: i32) -> u32 {
    pack(
        alpha(d),
        mix(ch(d, 16), xr, k),
        mix(ch(d, 8), xg, k),
        mix(ch(d, 0), xb, k),
    )
}

/// The Over result once the weight *k* is known (`ui-layout.md` §9.6.2).
#[inline]
fn over_k(d: u32, s: u32, k: i32) -> u32 {
    let da = alpha(d);
    if k == 256 || da == 0 {
        return s;
    }
    if da == 255 {
        return mix_channels(d, ch(s, 16), ch(s, 8), ch(s, 0), k);
    }
    let q = da + 1;
    let r = q + k - div256(q * k);
    let c = |dc: i32, sc: i32| dc - (dc - sc) * k / r;
    pack(
        r - 1,
        c(ch(d, 16), ch(s, 16)),
        c(ch(d, 8), ch(s, 8)),
        c(ch(d, 0), ch(s, 0)),
    )
}

/// The Colorize colour *X* of `ui-layout.md` §9.6.2 (hue and saturation of `s`, value of `d`).
#[must_use]
pub const fn colorize_colour(d: u32, s: u32) -> (i32, i32, i32) {
    let r = inc(ch(s, 16));
    let g = inc(ch(s, 8));
    let b = inc(ch(s, 0));
    let mx = max3(r, g, b);
    let mn = min3(r, g, b);
    let delta = mx - mn;
    let (sat, h) = if delta == 0 {
        (0, 0)
    } else {
        let sat = (256 * delta) / mx;
        let h = if mx == r {
            let v = (4096 * (g - b)) / delta;
            if g - b < 0 { v + 24_576 } else { v }
        } else if mx == g {
            8_192 + (4096 * (b - r)) / delta
        } else {
            16_384 + (4096 * (r - g)) / delta
        };
        (sat, h)
    };
    let v = max3(inc(ch(d, 16)), inc(ch(d, 8)), inc(ch(d, 0)));
    let (xr, xg, xb) = if sat == 0 {
        (v, v, v)
    } else {
        let c = div256(v * sat);
        let m = v - c;
        let i = h / 4096;
        let f = h - 8_192 * ((i + 1) / 2);
        let prod = f * c;
        let mut t = prod / 4096;
        let rem = prod % 4096;
        if rem > 2048 {
            t += 1;
        } else if rem < -2048 {
            t -= 1;
        }
        match i {
            0 => (v, m + t, m),
            1 => (m - t, v, m),
            2 => (m, v, m + t),
            3 => (m, m - t, v),
            4 => (m + t, m, v),
            _ => (v, m, m - t),
        }
    };
    (dec(xr), dec(xg), dec(xb))
}

const fn max3(a: i32, b: i32, c: i32) -> i32 {
    let m = if a > b { a } else { b };
    if m > c { m } else { c }
}

const fn min3(a: i32, b: i32, c: i32) -> i32 {
    let m = if a < b { a } else { b };
    if m < c { m } else { c }
}

/// One pixel of a **base layer** (`ui-layout.md` §9.6.2): destination `d`, source `s`,
/// opacity byte `o`. With `o` = 0 nothing changes; [`CompositeOp::Grayscale`] has no base
/// routine and changes nothing.
///
/// The same function serves the overlay layer (§9.6.3): without an image with `s` = the
/// colour and `o` = 255; with an image with `s` = (*I*.A, *C*.R, *C*.G, *C*.B) and `o` =
/// *C*.A (the "inc(*o*) for (*o* + 1)" substitution only differs at *o* = 0, which draws
/// nothing here too).
#[inline]
#[must_use]
pub fn base_pixel(op: CompositeOp, d: u32, s: u32, o: u8) -> u32 {
    let o = o as i32;
    if o == 0 {
        return d;
    }
    match op {
        CompositeOp::Replace => s,
        CompositeOp::MixColour => mix_channels(d, ch(s, 16), ch(s, 8), ch(s, 0), w1(alpha(s), o)),
        CompositeOp::Over => {
            let k = w1(alpha(s), o);
            if alpha(d) == 255 {
                // Opaque destination: the mix branch, which also gives the two edge cases
                // (k = 0 leaves D; k = 256 only for S.A = 255 at o = 255, giving D = S).
                mix_channels(d, ch(s, 16), ch(s, 8), ch(s, 0), k)
            } else if k == 0 {
                d
            } else {
                over_k(d, s, k)
            }
        }
        CompositeOp::Multiply => {
            let x = |sc: i32, dc: i32| div256(inc(sc) * dc);
            mix_channels(
                d,
                x(ch(s, 16), ch(d, 16)),
                x(ch(s, 8), ch(d, 8)),
                x(ch(s, 0), ch(d, 0)),
                w2(alpha(s), o),
            )
        }
        CompositeOp::Screen => {
            let x = |sc: i32, dc: i32| dc + sc - div256(inc(sc) * dc);
            mix_channels(
                d,
                x(ch(s, 16), ch(d, 16)),
                x(ch(s, 8), ch(d, 8)),
                x(ch(s, 0), ch(d, 0)),
                w1(alpha(s), o),
            )
        }
        CompositeOp::Colorize => {
            let k = w2(alpha(s), o);
            if k == 0 {
                d
            } else {
                let (xr, xg, xb) = colorize_colour(d, s);
                mix_channels(d, xr, xg, xb, k)
            }
        }
        CompositeOp::Grayscale => d,
    }
}

/// One pixel of an **overlay without an image** (`ui-layout.md` §9.6.3): the source is the
/// colour `c` with *a′* = *C*.A.
///
/// Replace fills; Over, Multiply and Colorize follow §9.6.2 with `o` = 255 (the weights
/// *a′* + 1 and inc(*a′*) the table gives); Grayscale is the overlay's own routine. Mix
/// colour and Screen are "not traced (unused)" for this layer: Tessera applies the §9.6.2
/// rule with the same weight, like the traced operations (recorded reading, §9.6.3 OPEN).
#[inline]
#[must_use]
pub fn overlay_fill_pixel(op: CompositeOp, d: u32, c: u32) -> u32 {
    match op {
        CompositeOp::Grayscale => {
            let a = alpha(c);
            if a == 0 {
                return d;
            }
            let k = a + 1;
            let v = max3(inc(ch(d, 16)), inc(ch(d, 8)), inc(ch(d, 0)));
            mix_channels(d, v, v, v, k)
        }
        _ => base_pixel(op, d, c, 255),
    }
}

/// One pixel of an **overlay with an image** (`ui-layout.md` §9.6.3): image pixel `i`
/// (only its alpha is used), colour `c` (its channels are used, its alpha is the opacity).
///
/// Multiply and Screen are traced; the other operations are "not traced (unused)" and
/// follow the same substitution into §9.6.2 (Grayscale, without a base routine, draws
/// nothing). The caller has already checked the image's source-format tag.
#[inline]
#[must_use]
pub fn overlay_image_pixel(op: CompositeOp, d: u32, i: u32, c: u32) -> u32 {
    let s = (i & 0xFF00_0000) | (c & 0x00FF_FFFF);
    base_pixel(op, d, s, (c >> 24) as u8)
}

/// One pixel of a text pass (`fonts.md` §8.4): coverage byte `cov`, pass colour `colour`
/// (`0xAARRGGBB`), destination `d`.
#[inline]
#[must_use]
pub fn text_pixel(d: u32, cov: u8, colour: u32) -> u32 {
    let c = cov as i32;
    let alpha_c = alpha(colour);
    let s = if alpha_c == 255 {
        c
    } else if alpha_c == 0 {
        0
    } else {
        (c * (alpha_c + 1)) / 256
    };
    let a = s + 1;
    let da = alpha(d);
    let (cr, cg, cb) = (ch(colour, 16), ch(colour, 8), ch(colour, 0));
    if da == 255 {
        // Opaque destination, step 5. It also covers step 2 (s = 0 gives a = 1, and a
        // difference below 256 times 1 ÷ 256 truncates to 0) and step 4 (a = 256 needs
        // α = 255 and c = 255, so the written alpha c is 255 and the channels become C).
        let f = |dc: i32, cc: i32| dc - (dc - cc) * a / 256;
        return pack(255, f(ch(d, 16), cr), f(ch(d, 8), cg), f(ch(d, 0), cb));
    }
    if s == 0 {
        return d;
    }
    if a == 256 || da == 0 {
        return pack(c, cr, cg, cb);
    }
    let t = da + 1;
    let k = t + a - (t * a) / 256;
    let f = |dc: i32, cc: i32| dc - (dc - cc) * a / k;
    pack(k - 1, f(ch(d, 16), cr), f(ch(d, 8), cg), f(ch(d, 0), cb))
}

/// [`base_pixel`] for Over when the destination is opaque (*D*.A = 255): the mix branch of
/// §9.6.2, without a branch on *D* (the painter uses it for spans it has checked).
#[inline]
pub(crate) fn over_opaque(d: u32, s: u32, o: u8) -> u32 {
    mix_channels(d, ch(s, 16), ch(s, 8), ch(s, 0), w1(alpha(s), i32::from(o)))
}

/// The coverage multiplier of a text colour: *s* = (*c* × `amul`) ÷ 256 gives step 1 of
/// `fonts.md` §8.4 for every colour alpha (256 for 255, 0 for 0, *α* + 1 otherwise).
#[inline]
pub(crate) const fn text_amul(colour: u32) -> i32 {
    match alpha(colour) {
        255 => 256,
        0 => 0,
        a => a + 1,
    }
}

/// [`text_pixel`] when the destination is opaque (step 5, which also covers steps 2 and 4
/// there), without a branch on the pixel.
#[inline]
pub(crate) fn text_opaque(d: u32, cov: u8, amul: i32, colour: u32) -> u32 {
    let a = ((i32::from(cov) * amul) >> 8) + 1;
    let f = |dc: i32, cc: i32| dc - (dc - cc) * a / 256;
    pack(
        255,
        f(ch(d, 16), ch(colour, 16)),
        f(ch(d, 8), ch(colour, 8)),
        f(ch(d, 0), ch(colour, 0)),
    )
}

#[cfg(test)]
mod tests {
    use super::*;

    /// (A, R, G, B) → `0xAARRGGBB`.
    const fn argb(a: u32, r: u32, g: u32, b: u32) -> u32 {
        (a << 24) | (r << 16) | (g << 8) | b
    }

    #[test]
    fn draw_mode_mapping() {
        assert_eq!(CompositeOp::from_draw_mode(0), Some(CompositeOp::Replace));
        assert_eq!(CompositeOp::from_draw_mode(2), Some(CompositeOp::Replace));
        assert_eq!(CompositeOp::from_draw_mode(5), Some(CompositeOp::MixColour));
        assert_eq!(CompositeOp::from_draw_mode(8), None);
        assert_eq!(CompositeOp::from_draw_mode(9), Some(CompositeOp::Replace));
        assert_eq!(
            CompositeOp::from_draw_mode(u32::MAX),
            Some(CompositeOp::Replace)
        );
    }

    #[test]
    fn opacity_and_colour_packing() {
        assert_eq!(opacity_byte(1.0), 255);
        assert_eq!(opacity_byte(0.5), 127);
        assert_eq!(opacity_byte(f32::NAN), 0);
        assert_eq!(opacity_byte(7.0), 255);
        // fonts.md §13.8: (0.5, 0.25, 1.0, 1.0) packs to 0xFF7F3FFF.
        assert_eq!(pack_colour([0.5, 0.25, 1.0, 1.0]), 0xFF7F_3FFF);
        assert_eq!(pack_colour([0.999, 0.0, 0.0, 1.0]) >> 16 & 0xFF, 254);
    }

    /// `ui-layout.md` §14.7, every row.
    #[test]
    fn ui_layout_worked_examples() {
        use CompositeOp::*;
        let over = base_pixel(Over, argb(128, 200, 100, 0), argb(128, 0, 0, 255), 255);
        assert_eq!(over, argb(192, 67, 34, 170));
        let d = argb(255, 200, 100, 40);
        let s = argb(255, 128, 64, 255);
        assert_eq!(base_pixel(Multiply, d, s, 255), argb(255, 100, 25, 40));
        assert_eq!(base_pixel(Multiply, d, s, 127), argb(255, 150, 63, 40));
        let screen = base_pixel(Screen, d, argb(191, 128, 64, 255), 255);
        assert_eq!(screen, argb(255, 221, 129, 201));
        let col = base_pixel(
            Colorize,
            argb(255, 100, 50, 20),
            argb(255, 40, 120, 200),
            255,
        );
        assert_eq!(col, argb(255, 20, 60, 100));
        assert_eq!(
            overlay_fill_pixel(Grayscale, d, argb(0xBF, 0, 0, 0)),
            argb(255, 200, 175, 160)
        );
        assert_eq!(
            overlay_fill_pixel(Grayscale, argb(255, 255, 10, 10), argb(0xFF, 1, 2, 3)),
            argb(255, 0, 0, 0)
        );
    }

    /// `fonts.md` §13.8, every example.
    #[test]
    fn fonts_worked_examples() {
        let c = argb(255, 200, 100, 0);
        assert_eq!(text_pixel(0, 128, c), argb(128, 200, 100, 0));
        let blue = argb(255, 0, 0, 255);
        assert_eq!(text_pixel(blue, 128, c), argb(255, 100, 50, 127));
        assert_eq!(text_pixel(blue, 0, c), blue);
        assert_eq!(text_pixel(blue, 255, c), argb(255, 200, 100, 0));
        let partial = text_pixel(argb(128, 200, 100, 0), 64, argb(255, 0, 0, 255));
        assert_eq!(partial, argb(161, 120, 60, 102));
        let white = argb(127, 255, 255, 255);
        assert_eq!(
            text_pixel(argb(255, 0, 0, 0), 255, white),
            argb(255, 127, 127, 127)
        );
        assert_eq!(text_pixel(0, 255, white), argb(255, 255, 255, 255));
    }

    #[test]
    fn replace_and_zero_opacity() {
        let d = argb(10, 20, 30, 40);
        let s = argb(0, 1, 2, 3);
        assert_eq!(
            base_pixel(CompositeOp::Replace, d, s, 1),
            s,
            "Replace writes alpha 0 too"
        );
        for op in [
            CompositeOp::Replace,
            CompositeOp::Over,
            CompositeOp::MixColour,
            CompositeOp::Multiply,
            CompositeOp::Screen,
            CompositeOp::Colorize,
        ] {
            assert_eq!(
                base_pixel(op, d, s | 0xFF00_0000, 0),
                d,
                "o = 0 draws nothing"
            );
        }
        assert_eq!(base_pixel(CompositeOp::Grayscale, d, s, 255), d);
        // Over with o < 255 onto a transparent pixel writes S with S's own alpha.
        assert_eq!(
            base_pixel(CompositeOp::Over, 0, argb(200, 9, 8, 7), 100),
            argb(200, 9, 8, 7)
        );
        // Mix colour keeps the destination's alpha.
        let m = base_pixel(
            CompositeOp::MixColour,
            argb(77, 0, 0, 0),
            argb(255, 255, 255, 255),
            255,
        );
        assert_eq!(m, argb(77, 255, 255, 255));
        // Overlay with an image: colour channels, image alpha, colour alpha as opacity.
        let o = overlay_image_pixel(
            CompositeOp::Over,
            0,
            argb(255, 1, 2, 3),
            argb(255, 50, 60, 70),
        );
        assert_eq!(o, argb(255, 50, 60, 70));
        assert_eq!(
            overlay_image_pixel(CompositeOp::Over, d, 0xFF00_0000, 0x00FF_FFFF),
            d
        );
    }

    #[test]
    fn exhaustive_no_overflow() {
        // Every op over a grid of destination/source bytes stays a valid pixel (no panic
        // with overflow checks on).
        let vals = [0u32, 1, 2, 127, 128, 254, 255];
        for &da in &vals {
            for &sa in &vals {
                for &c in &vals {
                    let d = argb(da, c, 255 - c, c / 2);
                    let s = argb(sa, 255 - c, c, 255);
                    for o in [1u8, 127, 254, 255] {
                        for op in [
                            CompositeOp::Replace,
                            CompositeOp::Over,
                            CompositeOp::MixColour,
                            CompositeOp::Multiply,
                            CompositeOp::Screen,
                            CompositeOp::Colorize,
                            CompositeOp::Grayscale,
                        ] {
                            let _ = base_pixel(op, d, s, o);
                            let _ = overlay_fill_pixel(op, d, s);
                            let _ = overlay_image_pixel(op, d, s, s);
                        }
                    }
                    let _ = text_pixel(d, c as u8, s);
                }
            }
        }
    }

    /// The opaque-destination fast paths agree with the general rules everywhere.
    #[test]
    fn opaque_fast_paths_agree() {
        for c in [0u32, 1, 2, 127, 128, 200, 254, 255] {
            for x in [0u32, 7, 128, 255] {
                let d = argb(255, x, 255 - x, c);
                for sa in 0..=255u32 {
                    let s = argb(sa, c, x, 255 - c);
                    for o in [1u8, 2, 100, 127, 254, 255] {
                        assert_eq!(over_opaque(d, s, o), base_pixel(CompositeOp::Over, d, s, o));
                    }
                }
                for ca in [0u32, 1, 127, 128, 254, 255] {
                    let colour = argb(ca, 255 - c, x, c);
                    for cov in 0..=255u8 {
                        assert_eq!(
                            text_opaque(d, cov, text_amul(colour), colour),
                            text_pixel(d, cov, colour)
                        );
                    }
                }
            }
        }
    }

    #[test]
    fn colorize_sectors_cover_all_hues() {
        for s in [
            0x00FF_0000u32,
            0x00FF_FF00,
            0x0000_FF00,
            0x0000_FFFF,
            0x0000_00FF,
            0x00FF_00FF,
        ] {
            let _ = colorize_colour(0xFF80_8080, s | 0xFF00_0000);
        }
    }
}
