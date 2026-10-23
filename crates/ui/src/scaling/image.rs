// SPDX-License-Identifier: GPL-3.0-or-later
//! Canvas pixel sizes (`ui-scaling.md` §6.1) and crisp image resampling (§6.3).
//!
//! The painter runs at the content scale and resamples each image once, per axis, to the
//! target size. This module holds the pure decisions and filter weights: which kind of axis an
//! image is drawn on (fitted, tiled or clipped), the scale of that axis, and the sample
//! weights of the default "sharp" filter (sharp bilinear above 1, the area filter below 1).
//! Compositing, premultiplication and the draw-mode arithmetic stay in the painter.

use super::screen::edge_map;

/// The canvas pixel-count mapping written `E(v)` in `ui-scaling.md` §6.1: the smallest integer
/// `X` with `(X + ½) ÷ Se ≥ v`, the pixel-centre edge mapping of §3.6 with the factor `Se` on
/// both axes. Used to size a surface's unstretched canvas and derived-glyph canvases. A
/// non-positive or non-finite `Se`, or a non-finite `v`, gives 0.
#[must_use]
pub fn canvas_px(se: f64, v: f64) -> i32 {
    edge_map(se, v)
}

/// Which way an image layer is drawn on one axis (`ui-scaling.md` §6.3, SC8).
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum AxisKind {
    /// The image was made for this element (`iw = w`): resample to exactly the physical box.
    Fitted,
    /// The image is smaller than the element (`iw < w`): tile at the content scale.
    Tiled,
    /// The image is larger than the element (`iw > w`): draw once, clipped, at the content scale.
    Clipped,
}

/// The axis kind for an image of native extent `native` on an element of layout extent `extent`
/// (`ui-scaling.md` §6.3).
#[must_use]
pub fn axis_kind(native: i32, extent: i32) -> AxisKind {
    use std::cmp::Ordering::{Equal, Greater, Less};
    match native.cmp(&extent) {
        Equal => AxisKind::Fitted,
        Less => AxisKind::Tiled,
        Greater => AxisKind::Clipped,
    }
}

/// The art filter of `ui-scaling.md` §6.3 (setting `ui_art_filter`, §7.6): which column of the
/// §6.3 filter table the painter resamples images with. At a scale of exactly 1 on both axes all
/// three are the exact 1 : 1 copy (invariant I2).
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Default)]
pub enum ArtFilter {
    /// The default: nearest blocks at an integer scale ≥ 2, sharp bilinear above 1, the area
    /// filter below 1.
    #[default]
    Sharp,
    /// Plain bilinear above 1 (also at integer scales), the area filter below 1.
    Smooth,
    /// Nearest texel at every scale (pixel art).
    Nearest,
}

impl ArtFilter {
    /// The filter for a settings value (`"sharp"`, `"smooth"`, `"nearest"`, ASCII case
    /// ignored; `None` for anything else).
    #[must_use]
    pub fn from_setting(value: &str) -> Option<Self> {
        match value.trim().to_ascii_lowercase().as_str() {
            "sharp" => Some(Self::Sharp),
            "smooth" => Some(Self::Smooth),
            "nearest" => Some(Self::Nearest),
            _ => None,
        }
    }

    /// The settings value of the filter.
    #[must_use]
    pub fn as_setting(self) -> &'static str {
        match self {
            Self::Sharp => "sharp",
            Self::Smooth => "smooth",
            Self::Nearest => "nearest",
        }
    }
}

/// Whether a resample scale counts as an integer ≥ 2 for the §6.3 filter table ("an integer ≥ 2
/// (within 10⁻⁶)"); returns that integer.
#[must_use]
pub fn integer_scale(s: f64) -> Option<u32> {
    if !s.is_finite() || s < 2.0 - 1e-6 {
        return None;
    }
    let r = s.round();
    ((s - r).abs() <= 1e-6 && r <= f64::from(u32::MAX)).then_some(r as u32)
}

/// The **cover** factor of a backdrop image (`ui-scaling.md` §5.5): for an image of native size
/// `native` on a Stretch element of design size `design` laid out at `current` (layout units),
/// `Some(max(current w ÷ design w, current h ÷ design h))` when the image was made for the
/// element (native size = design size, design width and height at least 1) and the layout has
/// resized the element (current ≠ design); `None` otherwise (the image tiles as UL §9.6 says).
/// The caller checks the element side (object mode Stretch, no edge in mode 6, no stretch
/// vector).
#[must_use]
pub fn backdrop_cover(native: [i32; 2], design: [i32; 2], current: [i32; 2]) -> Option<f64> {
    if design[0] < 1 || design[1] < 1 || native != design || current == design {
        return None;
    }
    if current[0] < 1 || current[1] < 1 {
        return None;
    }
    let f = (f64::from(current[0]) / f64::from(design[0]))
        .max(f64::from(current[1]) / f64::from(design[1]));
    (f.is_finite() && f > 0.0).then_some(f)
}

/// The resample scale of one axis (`ui-scaling.md` §6.3): a fitted axis scales to its physical
/// box (`physical ÷ native`), a tiled or clipped axis scales by the content scale `c`. A
/// non-positive `native` falls back to `c`.
#[must_use]
pub fn axis_scale(kind: AxisKind, physical: i32, native: i32, c: f64) -> f64 {
    match kind {
        AxisKind::Fitted if native > 0 => f64::from(physical) / f64::from(native),
        _ => c,
    }
}

/// The sharp-bilinear sample of one destination pixel (`ui-scaling.md` §6.3, the "above 1,
/// not an integer" row): for a destination offset `d` and scale `s > 1`, returns the lower
/// source texel index `i` and the weights of texels `i` and `i + 1`. Each texel boundary turns
/// into at most one blended pixel. `s ≤ 0` or non-finite gives the nearest texel.
#[must_use]
pub fn sharp_bilinear(d: i32, s: f64) -> (i32, f64, f64) {
    if !s.is_finite() || s <= 0.0 {
        return (d, 1.0, 0.0);
    }
    let t = (f64::from(d) + 0.5) / s - 0.5;
    let i = t.floor();
    let phi = t - i;
    let w1 = ((phi - 0.5) * s + 0.5).clamp(0.0, 1.0);
    (i as i32, 1.0 - w1, w1)
}

/// The area-filter weights (`ui-scaling.md` §6.3, the "below 1" row; also the downsampling of
/// glyph coverage, §6.4.4). For each of `dst_len` destination pixels the footprint
/// `[d ÷ s, (d + 1) ÷ s)` in source texels, each source texel weighted by the covered
/// fraction and normalised so the weights of a destination pixel sum to 1 (an average).
/// Indices are clamped into `0 … src_len − 1`. `s` is the destination-per-source scale
/// (`dst_len ÷ src_len`, below 1 for downsampling).
///
/// The result is `dst_len` lists of `(source index, weight)`; it allocates `O(dst_len + …)`
/// and never panics for finite inputs.
#[must_use]
pub fn area_weights(dst_len: usize, src_len: usize, s: f64) -> Vec<Vec<(usize, f64)>> {
    let mut out = vec![Vec::new(); dst_len];
    if src_len == 0 || dst_len == 0 || !s.is_finite() || s <= 0.0 {
        return out;
    }
    let src_max = src_len as f64;
    for (d, slot) in out.iter_mut().enumerate() {
        let lo = (d as f64 / s).clamp(0.0, src_max);
        let hi = ((d as f64 + 1.0) / s).clamp(0.0, src_max);
        let span = hi - lo;
        if span <= 0.0 {
            // Degenerate footprint: take the single covering texel.
            let i = (lo.floor() as usize).min(src_len - 1);
            slot.push((i, 1.0));
            continue;
        }
        let first = lo.floor() as usize;
        let last = ((hi.ceil() as usize).max(first + 1)).min(src_len);
        for i in first..last {
            let overlap = (hi.min((i + 1) as f64) - lo.max(i as f64)).max(0.0);
            if overlap > 0.0 {
                slot.push((i, overlap / span));
            }
        }
        if slot.is_empty() {
            slot.push((first.min(src_len - 1), 1.0));
        }
    }
    out
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn canvas_edge_mapping() {
        assert_eq!(canvas_px(1.0, 10.0), 10);
        assert_eq!(canvas_px(2.0, 10.0), 20);
        assert_eq!(canvas_px(1.5, 100.0), 150);
        // Smallest X with (X+0.5)/Se >= v.
        assert_eq!(canvas_px(2.0, 10.25), 20); // (20+0.5)/2 = 10.25 >= 10.25
        assert_eq!(canvas_px(2.0, 10.26), 21);
    }

    #[test]
    fn axis_kinds() {
        assert_eq!(axis_kind(64, 64), AxisKind::Fitted);
        assert_eq!(axis_kind(2, 64), AxisKind::Tiled);
        assert_eq!(axis_kind(128, 64), AxisKind::Clipped);
    }

    #[test]
    fn axis_scales() {
        // Fitted: image 200 texels in a 437-physical-pixel box → 2.185.
        assert!((axis_scale(AxisKind::Fitted, 437, 200, 1.5) - 2.185).abs() < 1e-9);
        // Tiled/clipped: content scale.
        assert_eq!(axis_scale(AxisKind::Tiled, 437, 2, 1.5), 1.5);
        assert_eq!(axis_scale(AxisKind::Clipped, 437, 128, 1.5), 1.5);
    }

    #[test]
    fn sharp_bilinear_boundaries() {
        // At s = 2, within a texel the weight stays at one texel except across the boundary.
        let s = 2.0;
        // d = 0: t = 0.25/2... let's just assert weights sum to 1 and are in range.
        for d in 0..8 {
            let (i, w0, w1) = sharp_bilinear(d, s);
            assert!((w0 + w1 - 1.0).abs() < 1e-9, "d={d} sum");
            assert!((0.0..=1.0).contains(&w0) && (0.0..=1.0).contains(&w1));
            let _ = i;
        }
        // Non-finite / non-positive scale: nearest texel.
        assert_eq!(sharp_bilinear(5, 0.0), (5, 1.0, 0.0));
    }

    #[test]
    fn area_weights_normalised() {
        // Downsample 8 → 4: s = 0.5, each dst pixel covers 2 source texels equally.
        let w = area_weights(4, 8, 0.5);
        assert_eq!(w.len(), 4);
        for (d, slot) in w.iter().enumerate() {
            let sum: f64 = slot.iter().map(|&(_, wt)| wt).sum();
            assert!((sum - 1.0).abs() < 1e-9, "dst {d} weights sum {sum}");
            assert_eq!(slot.len(), 2, "dst {d}");
        }
        // Indices stay in range.
        for slot in &w {
            for &(i, _) in slot {
                assert!(i < 8);
            }
        }
    }

    #[test]
    fn area_weights_partial_texels() {
        // Downsample 3 → 2: s = 2/3; each dst pixel covers 1.5 source texels.
        let w = area_weights(2, 3, 2.0 / 3.0);
        for slot in &w {
            let sum: f64 = slot.iter().map(|&(_, wt)| wt).sum();
            assert!((sum - 1.0).abs() < 1e-9, "sum {sum}");
        }
    }

    #[test]
    fn art_filter_settings() {
        assert_eq!(ArtFilter::default(), ArtFilter::Sharp);
        for f in [ArtFilter::Sharp, ArtFilter::Smooth, ArtFilter::Nearest] {
            assert_eq!(ArtFilter::from_setting(f.as_setting()), Some(f));
        }
        assert_eq!(ArtFilter::from_setting(" Smooth "), Some(ArtFilter::Smooth));
        assert_eq!(ArtFilter::from_setting("pixel"), None);
    }

    #[test]
    fn integer_scales() {
        assert_eq!(integer_scale(2.0), Some(2));
        assert_eq!(integer_scale(3.000_000_5), Some(3));
        assert_eq!(integer_scale(1.0), None);
        assert_eq!(integer_scale(2.5), None);
        assert_eq!(integer_scale(f64::NAN), None);
    }

    #[test]
    fn backdrop_cover_rule() {
        // §5.5 test of the task: a 1024 × 768 backdrop on a Stretch element resized to
        // 1365 × 768 covers at max(1365 / 1024, 768 / 768) = 1.333.
        let f = backdrop_cover([1024, 768], [1024, 768], [1365, 768]).unwrap();
        assert!((f - 1365.0 / 1024.0).abs() < 1e-12);
        // Not resized, not made for the element, or a degenerate design: no cover.
        assert_eq!(backdrop_cover([1024, 768], [1024, 768], [1024, 768]), None);
        assert_eq!(backdrop_cover([64, 64], [1024, 768], [1365, 768]), None);
        assert_eq!(backdrop_cover([0, 0], [0, 0], [10, 10]), None);
        assert_eq!(backdrop_cover([4, 4], [4, 4], [0, 10]), None);
    }

    #[test]
    fn hostile_inputs_no_panic() {
        let _ = canvas_px(f64::NAN, 10.0);
        let _ = canvas_px(-1.0, 10.0);
        let _ = axis_scale(AxisKind::Fitted, 100, 0, 1.0);
        let _ = sharp_bilinear(i32::MAX, f64::INFINITY);
        let _ = area_weights(0, 0, f64::NAN);
        let _ = area_weights(4, 0, 0.5);
    }
}
