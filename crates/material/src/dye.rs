// SPDX-License-Identifier: GPL-3.0-or-later
//! Dyes (`material.md` §12): applying a dye colour to the dyeable area of a base texture,
//! reading dye colours from a dye material, and choosing an appearance's dye block for a
//! dye code.
//!
//! The blend is the smooth form of §12.3, applied to the 8-bit values as stored (the
//! original uses no sRGB conversion, §12.3):
//!
//! - dye weight *w* = 1 − *a*, where *a* is the mask alpha in 0–1 (low = dyeable);
//! - dyed colour = *c* × (1 − *w*) + (*c* × *d*) × *w*.
//!
//! A second channel (2013) dyes the **undyed** base: *c* ← *a2* · (first-dyed colour) +
//! (1 − *a2*) · (*c₀* × *d2*) (§20.1; `lighting.md` §5.2 step 3).

use tessera_formats::RgbaImage;

use crate::error::{MaterialError, Result};
use crate::records::MaterialRecord;
use crate::vocab::ids;

/// One dye layer: a mask image (its alpha channel is used) and a colour.
#[derive(Debug, Clone, Copy)]
pub struct DyeLayer<'a> {
    /// The mask image. Scaled to the base image by nearest sampling if its size differs.
    pub mask: &'a RgbaImage,
    /// Dye colour, RGBA in 0–1. Its alpha is not used (§12.3).
    pub color: [f32; 4],
}

/// The dye weight of a mask alpha byte: 1 − *a* (§12.3).
#[must_use]
pub fn dye_weight(mask_alpha: u8) -> f32 {
    1.0 - f32::from(mask_alpha) / 255.0
}

/// Dyes one 8-bit channel value `c` with dye component `d` (0–1) at weight `w` (§12.3).
#[must_use]
pub fn dye_channel(c: u8, d: f32, w: f32) -> u8 {
    dye_channel_over(c, c, d, w)
}

/// One channel of a later dye layer: `current` (the colour so far) × (1 − `w`) + `base`
/// (the undyed value) × `d` × `w` (§20.1). For the first layer `current` = `base`.
fn dye_channel_over(current: u8, base: u8, d: f32, w: f32) -> u8 {
    let d = if d.is_finite() {
        d.clamp(0.0, 1.0)
    } else {
        1.0
    };
    let w = if w.is_finite() {
        w.clamp(0.0, 1.0)
    } else {
        0.0
    };
    let v = f32::from(current) * (1.0 - w) + f32::from(base) * d * w;
    // In range: v ≤ max(current, base) ≤ 255 because d, w ∈ [0, 1].
    v.round().clamp(0.0, 255.0) as u8
}

/// Checks that `img` is consistent ([`crate::icon::validate_image`], overflow-safe for any
/// width and height) and not empty.
fn check_image(img: &RgbaImage) -> Result<()> {
    crate::icon::validate_image(img)?;
    if img.width == 0 || img.height == 0 {
        return Err(MaterialError::Empty {
            width: img.width,
            height: img.height,
        });
    }
    Ok(())
}

/// Nearest-sample index into a `src`-long axis for output position `i` of `dst`.
fn nearest(i: u32, dst: u32, src: u32) -> u32 {
    let v = (u64::from(i) * u64::from(src)) / u64::from(dst.max(1));
    u32::try_from(v)
        .unwrap_or(u32::MAX)
        .min(src.saturating_sub(1))
}

/// Applies dye layers in order to `base` and returns the dyed RGBA8 image.
///
/// Each layer blends the colour so far with the **undyed** base times its colour,
/// weighted by its mask: *c* ← *a* · *c* + (1 − *a*) · (*c₀* × *d*). For the first layer
/// this is §12.3; for the second channel it is §20.1 (`lighting.md` §5.2 step 3), so
/// overlapping masks do not compound the two dye colours.
///
/// The output alpha is `base`'s alpha when `keep_alpha` is set (the base alpha is
/// opacity), else 255 (the base alpha is a mask or gloss, not opacity). Fails only on an
/// inconsistent or empty image.
pub fn apply_dye(base: &RgbaImage, layers: &[DyeLayer<'_>], keep_alpha: bool) -> Result<RgbaImage> {
    check_image(base)?;
    for layer in layers {
        check_image(layer.mask)?;
    }
    let mut out = base.pixels.clone();
    let undyed = &base.pixels;
    let (w, h) = (base.width, base.height);
    for layer in layers {
        let mask = layer.mask;
        let same = mask.width == w && mask.height == h;
        for y in 0..h {
            let my = if same { y } else { nearest(y, h, mask.height) };
            for x in 0..w {
                let mx = if same { x } else { nearest(x, w, mask.width) };
                let mi = (my as usize * mask.width as usize + mx as usize) * 4;
                let oi = (y as usize * w as usize + x as usize) * 4;
                let (Some(&a), Some(px), Some(c0)) = (
                    mask.pixels.get(mi + 3),
                    out.get_mut(oi..oi + 3),
                    undyed.get(oi..oi + 3),
                ) else {
                    continue;
                };
                let weight = dye_weight(a);
                for ((c, &b), d) in px.iter_mut().zip(c0).zip(layer.color) {
                    *c = dye_channel_over(*c, b, d, weight);
                }
            }
        }
    }
    if !keep_alpha {
        for a in out.iter_mut().skip(3).step_by(4) {
            *a = 255;
        }
    }
    Ok(RgbaImage {
        width: w,
        height: h,
        pixels: out,
    })
}

/// The dye colours a dye material sets (§12.1): 2003 and/or 2013.
#[derive(Debug, Clone, Copy, PartialEq, Default)]
pub struct DyeColors {
    /// Colour 2003, if set.
    pub primary: Option<[f32; 4]>,
    /// Colour 2013, if set.
    pub secondary: Option<[f32; 4]>,
}

impl DyeColors {
    /// Reads the dye colours of a material.
    #[must_use]
    pub fn of_material(material: &MaterialRecord) -> Self {
        let color = |id| material.get(id).and_then(|e| e.value.as_color());
        Self {
            primary: color(ids::DYE_COLOR),
            secondary: color(ids::DYE_COLOR_2),
        }
    }

    /// Whether the material only sets dye colours (a pure dye material, §12.4).
    #[must_use]
    pub fn is_pure_dye(material: &MaterialRecord) -> bool {
        !material.entries.is_empty()
            && material
                .entries
                .iter()
                .all(|e| e.id == ids::DYE_COLOR || e.id == ids::DYE_COLOR_2)
    }
}

/// Half the 0.01 step of dye codes: a block within this of the code matches (§12.4).
pub const DYE_CODE_TOLERANCE: f32 = 0.005;

/// Chooses an appearance's dye block for dye `code` (§12.4 Decision).
///
/// `block_codes` are the blocks' `q` values (sorted ascending in observed data; not
/// required here). `None`, 0.0 or a non-finite code means "no dye": no block is
/// layered. Otherwise: the block within [`DYE_CODE_TOLERANCE`] of the code (the closest
/// one), else the block with the largest `q` ≤ code, else the first block.
#[must_use]
pub fn select_dye_block(block_codes: &[f32], code: Option<f32>) -> Option<usize> {
    let code = code.filter(|c| c.is_finite() && *c != 0.0)?;
    if block_codes.is_empty() {
        return None;
    }
    let close = block_codes
        .iter()
        .enumerate()
        .filter(|(_, q)| (*q - code).abs() <= DYE_CODE_TOLERANCE)
        .min_by(|a, b| (a.1 - code).abs().total_cmp(&(b.1 - code).abs()))
        .map(|(i, _)| i);
    let below = || {
        block_codes
            .iter()
            .enumerate()
            .filter(|(_, q)| q.is_finite() && **q <= code)
            .max_by(|a, b| a.1.total_cmp(b.1))
            .map(|(i, _)| i)
    };
    close.or_else(below).or(Some(0))
}

#[cfg(test)]
mod tests {
    use super::*;

    fn image(w: u32, h: u32, px: [u8; 4]) -> RgbaImage {
        RgbaImage {
            width: w,
            height: h,
            pixels: px.repeat((w * h) as usize),
        }
    }

    #[test]
    fn weight_and_channel() {
        assert_eq!(dye_weight(255), 0.0);
        assert_eq!(dye_weight(0), 1.0);
        // Fully dyeable: multiply.
        assert_eq!(dye_channel(200, 0.5, 1.0), 100);
        // Not dyeable: unchanged.
        assert_eq!(dye_channel(200, 0.5, 0.0), 200);
        // Half: 200 × 0.5 + 100 × 0.5.
        assert_eq!(dye_channel(200, 0.5, 0.5), 150);
        // Hostile colours are clamped.
        assert_eq!(dye_channel(200, f32::NAN, 1.0), 200);
        assert_eq!(dye_channel(200, 7.0, 1.0), 200);
        assert_eq!(dye_channel(200, -1.0, 1.0), 0);
    }

    #[test]
    fn dyes_masked_area_only() {
        // Left pixel dyeable (alpha 0), right pixel not (alpha 255).
        let base = RgbaImage {
            width: 2,
            height: 1,
            pixels: vec![128, 128, 128, 0, 128, 128, 128, 255],
        };
        let red = [1.0, 0.0, 0.0, 1.0];
        let out = apply_dye(
            &base,
            &[DyeLayer {
                mask: &base,
                color: red,
            }],
            false,
        )
        .unwrap();
        assert_eq!(out.pixels, vec![128, 0, 0, 255, 128, 128, 128, 255]);
        let kept = apply_dye(
            &base,
            &[DyeLayer {
                mask: &base,
                color: red,
            }],
            true,
        )
        .unwrap();
        assert_eq!(kept.pixels[3], 0);
    }

    #[test]
    fn second_layer_dyes_the_undyed_base() {
        // Base 200, both masks fully dyeable, both colours 0.5: 100, not 200 → 100 → 50.
        let base = image(1, 1, [200, 200, 200, 255]);
        let mask = image(1, 1, [0, 0, 0, 0]);
        let half = [0.5, 0.5, 0.5, 1.0];
        let layers = [
            DyeLayer {
                mask: &mask,
                color: half,
            },
            DyeLayer {
                mask: &mask,
                color: half,
            },
        ];
        let out = apply_dye(&base, &layers, false).unwrap();
        assert_eq!(out.pixels, vec![100, 100, 100, 255]);
        // Second mask not dyeable (alpha 255): the first dye stays.
        let keep = image(1, 1, [0, 0, 0, 255]);
        let red = [1.0, 0.0, 0.0, 1.0];
        let out = apply_dye(
            &base,
            &[
                DyeLayer {
                    mask: &mask,
                    color: red,
                },
                DyeLayer {
                    mask: &keep,
                    color: half,
                },
            ],
            false,
        )
        .unwrap();
        assert_eq!(out.pixels, vec![200, 0, 0, 255]);
        // Half-dyeable second mask (alpha 128 ≈ 0.502): a2·(200, 0, 0) + (1 − a2)·100.
        let soft = image(1, 1, [0, 0, 0, 128]);
        let out = apply_dye(
            &base,
            &[
                DyeLayer {
                    mask: &mask,
                    color: red,
                },
                DyeLayer {
                    mask: &soft,
                    color: half,
                },
            ],
            false,
        )
        .unwrap();
        let a2 = 128.0f32 / 255.0;
        let expect = |first: f32| (a2 * first + (1.0 - a2) * 100.0).round() as u8;
        assert_eq!(
            out.pixels,
            vec![expect(200.0), expect(0.0), expect(0.0), 255]
        );
    }

    #[test]
    fn mask_is_scaled() {
        let base = image(4, 4, [100, 100, 100, 255]);
        let mask = image(1, 1, [0, 0, 0, 0]);
        let out = apply_dye(
            &base,
            &[DyeLayer {
                mask: &mask,
                color: [0.5, 0.5, 0.5, 1.0],
            }],
            false,
        )
        .unwrap();
        assert!(out.pixels.chunks(4).all(|p| p == [50, 50, 50, 255]));
    }

    #[test]
    fn rejects_bad_images() {
        let bad = RgbaImage {
            width: 2,
            height: 2,
            pixels: vec![0; 3],
        };
        assert!(matches!(
            apply_dye(&bad, &[], false),
            Err(MaterialError::InvalidImage { .. })
        ));
        // Dimensions whose byte count overflows u64: an error, not a panic.
        let huge = RgbaImage {
            width: u32::MAX,
            height: u32::MAX,
            pixels: vec![],
        };
        assert!(matches!(
            apply_dye(&huge, &[], false),
            Err(MaterialError::InvalidImage { .. })
        ));
        let ok = image(1, 1, [0; 4]);
        assert!(matches!(
            apply_dye(
                &ok,
                &[DyeLayer {
                    mask: &huge,
                    color: [1.0; 4]
                }],
                false
            ),
            Err(MaterialError::InvalidImage { .. })
        ));
        let empty = image(0, 3, [0; 4]);
        assert!(matches!(
            apply_dye(&empty, &[], false),
            Err(MaterialError::Empty { .. })
        ));
    }

    #[test]
    fn block_selection() {
        let q = [0.01, 0.02, 0.05, 0.1];
        assert_eq!(select_dye_block(&q, None), None);
        assert_eq!(select_dye_block(&q, Some(0.0)), None);
        assert_eq!(select_dye_block(&q, Some(f32::NAN)), None);
        assert_eq!(select_dye_block(&q, Some(0.0201)), Some(1));
        assert_eq!(select_dye_block(&q, Some(0.07)), Some(2));
        assert_eq!(select_dye_block(&q, Some(0.5)), Some(3));
        assert_eq!(select_dye_block(&q, Some(0.001)), Some(0));
        assert_eq!(select_dye_block(&[], Some(0.1)), None);
    }
}
