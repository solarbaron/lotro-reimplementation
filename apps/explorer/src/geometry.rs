// SPDX-License-Identifier: GPL-3.0-or-later
//! Pure conversions for the mesh viewer: a decoded mesh level into the renderer's
//! [`MeshData`] streams, and the texture compositions the viewer uploads (opacity merged
//! into the base colour's alpha, alpha channels moved into single-channel masks,
//! thumbnails).
//!
//! Everything here is plain data in, plain data out, with no GPU, no egui and no archive
//! access, so the fuzz target `explorer_mesh_scene` includes this file by path. Inputs
//! are treated as hostile: images whose pixel buffer disagrees with their size are
//! refused (never indexed out of bounds), and every size computation is checked.
//!
//! Conventions (`docs/specs/mesh.md` §11, `docs/specs/render.md` §2.1): positions are
//! passed through unchanged (Z-up, right-handed, metres), front faces are
//! counter-clockwise in both, UVs are not flipped, and the mesh's tangent sign already
//! follows the renderer's `bitangent = w · (normal × tangent)` rule.

use tessera_formats::RgbaImage;
use tessera_mesh::{Indices as MeshIndices, LevelGeometry};
use tessera_render::{Indices, MAX_PALETTE_BONES, MeshData, MeshSkin, Submesh};

/// Options of [`level_mesh_data`]. The default is vertex colours off and UV set 0.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub struct ConvertOptions {
    /// Pass the mesh's vertex colours (RGB only) to the renderer, where they multiply the
    /// base colour. `mesh.md` §5.4 leaves their use OPEN; a viewer "may modulate the
    /// diffuse colour". Off by default: on real data many vertex colours are dark or carry
    /// low alpha, and multiplying them in blackens or deletes geometry
    /// (`material.md` §19).
    pub vertex_colors: bool,
    /// The UV set used as the texture coordinate (0 = the diffuse set, `mesh.md` §5.5).
    /// A set the level does not have falls back to set 0.
    pub uv_set: usize,
}

/// Converts an 8-bit sRGB-encoded value to linear light (0–1).
///
/// Mesh vertex colours and material colours are taken as sRGB-encoded (both are OPEN in
/// `mesh.md` §5.4 / `material.md` §12.3); multiplying linear values then equals the
/// gamma-space multiply of the stored values for a pure power curve.
#[must_use]
pub fn srgb_to_linear(c: u8) -> f32 {
    srgb_unit_to_linear(f32::from(c) / 255.0)
}

/// [`srgb_to_linear`] for a value in 0–1 (clamped; non-finite → 0).
#[must_use]
pub fn srgb_unit_to_linear(c: f32) -> f32 {
    let c = if c.is_finite() {
        c.clamp(0.0, 1.0)
    } else {
        0.0
    };
    if c <= 0.040_45 {
        c / 12.92
    } else {
        ((c + 0.055) / 1.055).powf(2.4)
    }
}

/// The renderer streams of one flattened mesh level: one renderer submesh per draw entry
/// ([`tessera_mesh::Part`]), in draw order.
///
/// With [`ConvertOptions::vertex_colors`] the colour stream carries the vertex RGB and an
/// alpha of exactly 1: the renderer multiplies the vertex colour into the base colour,
/// so a stored vertex alpha would become coverage for alpha-tested and blended draws,
/// and nothing says it is opacity (`mesh.md` §5.4 OPEN).
///
/// Infallible: the renderer validates the streams again when the result is prepared
/// ([`MeshData::prepare`]), and refuses non-finite positions there.
#[must_use]
pub fn level_mesh_data(geo: &LevelGeometry, options: &ConvertOptions, label: String) -> MeshData {
    let uvs = geo
        .uv_sets
        .get(options.uv_set)
        .or_else(|| geo.uv_sets.first())
        .cloned();
    let colors = if options.vertex_colors {
        geo.colors.as_ref().map(|c| {
            c.iter()
                .map(|&[r, g, b, _]| [srgb_to_linear(r), srgb_to_linear(g), srgb_to_linear(b), 1.0])
                .collect()
        })
    } else {
        None
    };
    let indices = match &geo.indices {
        MeshIndices::U16(v) => Indices::U16(v.clone()),
        MeshIndices::U32(v) => Indices::U32(v.clone()),
    };
    let submeshes = geo
        .parts
        .iter()
        .map(|p| Submesh {
            first_index: p.first_index,
            index_count: p.index_count,
        })
        .collect();
    MeshData {
        label: Some(label),
        positions: geo.positions.clone(),
        normals: Some(geo.normals.clone()),
        tangents: geo.tangents.clone(),
        uvs,
        colors,
        indices,
        submeshes,
    }
}

/// The result of [`mesh_skin`].
#[derive(Debug, Clone, PartialEq)]
pub struct SkinConversion {
    /// Four influences per vertex, weights summing to 1, indices below the bone count.
    pub skin: MeshSkin,
    /// Vertices that had no non-zero weight (rigid vertices inside a skinned level) and
    /// were put fully on bone 0.
    pub fixed: usize,
    /// Vertices whose weights were rescaled to sum to 1 (sums off by more than the
    /// renderer's tolerance).
    pub renormalised: usize,
}

/// The renderer's bone streams of a flattened level ([`LevelGeometry::joints`],
/// [`LevelGeometry::weights`]) for a skeleton of `bones` bones (`skeleton-anim.md` §4.1:
/// indices address the skeleton's bones directly).
///
/// Weights are made drawable: non-finite or negative weights count as 0, weights whose
/// sum differs from 1 are rescaled, and a vertex without any weight (an unskinned part of
/// a level that also has skinned ones) is put fully on bone 0 and counted in
/// [`SkinConversion::fixed`].
///
/// # Errors
///
/// When the level has no bone streams, their lengths differ from the vertex count, the
/// bone count is 0 or above the renderer's palette ([`MAX_PALETTE_BONES`]), or a vertex
/// uses a bone at or above `bones` with a non-zero weight.
pub fn mesh_skin(geo: &LevelGeometry, bones: usize) -> Result<SkinConversion, String> {
    if bones == 0 || bones > MAX_PALETTE_BONES {
        return Err(format!(
            "{bones} bones (the palette holds 1 to {MAX_PALETTE_BONES})"
        ));
    }
    let n = geo.positions.len();
    let (Some(joints), Some(weights)) = (&geo.joints, &geo.weights) else {
        return Err("the level has no bone influences".into());
    };
    if joints.len() != n || weights.len() != n {
        return Err(format!(
            "bone streams of {} and {} entries for {n} vertices",
            joints.len(),
            weights.len()
        ));
    }
    let mut out_w = Vec::with_capacity(n);
    let mut out_j = Vec::with_capacity(n);
    let (mut fixed, mut renormalised) = (0usize, 0usize);
    for (v, (j, w)) in joints.iter().zip(weights).enumerate() {
        let mut w = w.map(|x| if x.is_finite() && x > 0.0 { x } else { 0.0 });
        for k in 0..4 {
            if w[k] > 0.0 && usize::from(j[k]) >= bones {
                return Err(format!(
                    "vertex {v} uses bone {} of a {bones}-bone skeleton",
                    j[k]
                ));
            }
        }
        let sum: f32 = w.iter().sum();
        if !(sum.is_finite() && sum > 0.0) {
            fixed += 1;
            out_w.push([1.0, 0.0, 0.0, 0.0]);
            out_j.push([0; 4]);
            continue;
        }
        if (sum - 1.0).abs() > 1.0e-4 {
            renormalised += 1;
            for x in &mut w {
                *x /= sum;
            }
        }
        out_w.push(w);
        out_j.push(*j);
    }
    Ok(SkinConversion {
        skin: MeshSkin {
            joints: out_j,
            weights: out_w,
            palette_size: u16::try_from(bones).unwrap_or(u16::MAX),
        },
        fixed,
        renormalised,
    })
}

/// Whether an image is non-empty and its pixel buffer has exactly `width × height × 4`
/// bytes (overflow-safe).
#[must_use]
pub fn image_ok(img: &RgbaImage) -> bool {
    img.width > 0
        && img.height > 0
        && (img.width as usize)
            .checked_mul(img.height as usize)
            .and_then(|n| n.checked_mul(4))
            .is_some_and(|n| n == img.pixels.len())
}

/// Nearest-sample index into a `src`-long axis for output position `i` of `dst`.
fn nearest(i: u32, dst: u32, src: u32) -> usize {
    let v = (u64::from(i) * u64::from(src)) / u64::from(dst.max(1));
    (v.min(u64::from(src.saturating_sub(1)))) as usize
}

/// The alpha byte of `img` at output pixel (`x`, `y`) of a `w × h` image (nearest
/// sampling when the sizes differ). `img` must have passed [`image_ok`].
fn alpha_at(img: &RgbaImage, x: u32, y: u32, w: u32, h: u32) -> u8 {
    let (sx, sy) = if img.width == w && img.height == h {
        (x as usize, y as usize)
    } else {
        (nearest(x, w, img.width), nearest(y, h, img.height))
    };
    let i = (sy * img.width as usize + sx) * 4 + 3;
    img.pixels.get(i).copied().unwrap_or(255)
}

/// Where the alpha channel of a base-colour texture comes from.
#[derive(Debug, Clone, Copy)]
pub enum AlphaSource<'a> {
    /// Fully opaque (the base texture's alpha is a gloss or dye mask, or nothing).
    Opaque,
    /// The base texture's own alpha is the opacity.
    Own,
    /// The alpha channel of another texture (a separate opacity map), resampled to the
    /// base texture's size.
    From(&'a RgbaImage),
}

/// The base-colour texture the renderer samples: RGB of `base`, alpha from `alpha`
/// (`material.md` §11.2 opacity source).
///
/// Without a base texture, a separate opacity map still gives white RGB with its alpha.
/// Returns `None` when there is nothing to upload or an input image is inconsistent.
#[must_use]
pub fn base_texture(base: Option<&RgbaImage>, alpha: AlphaSource<'_>) -> Option<RgbaImage> {
    let alpha_img = match alpha {
        AlphaSource::From(a) if image_ok(a) => Some(a),
        AlphaSource::From(_) => return None,
        _ => None,
    };
    let Some(base) = base else {
        let a = alpha_img?;
        let mut out = a.clone();
        for px in out.pixels.as_chunks_mut::<4>().0 {
            px[0] = 255;
            px[1] = 255;
            px[2] = 255;
        }
        return Some(out);
    };
    if !image_ok(base) {
        return None;
    }
    let mut out = base.clone();
    match (alpha, alpha_img) {
        (AlphaSource::Own, _) => {}
        (_, Some(a)) => {
            let (w, h) = (out.width, out.height);
            for y in 0..h {
                for x in 0..w {
                    let i = (y as usize * w as usize + x as usize) * 4 + 3;
                    let v = alpha_at(a, x, y, w, h);
                    if let Some(p) = out.pixels.get_mut(i) {
                        *p = v;
                    }
                }
            }
        }
        _ => {
            for a in out.pixels.iter_mut().skip(3).step_by(4) {
                *a = 255;
            }
        }
    }
    Some(out)
}

/// A single-channel mask in the renderer's convention (`render.md` §2.3: the value in R,
/// replicated to G and B; alpha 255) from the alpha channel of `src`. With `invert` the
/// value is `255 − alpha`: dye masks mark dyeable texels with low alpha (`material.md`
/// §12.2), the renderer's dye mask with high values.
#[must_use]
pub fn alpha_mask(src: &RgbaImage, invert: bool) -> Option<RgbaImage> {
    if !image_ok(src) {
        return None;
    }
    let mut out = src.clone();
    for px in out.pixels.as_chunks_mut::<4>().0 {
        let v = if invert { 255 - px[3] } else { px[3] };
        px[0] = v;
        px[1] = v;
        px[2] = v;
        px[3] = 255;
    }
    Some(out)
}

/// A thumbnail of at most `max_side` pixels a side (aspect kept, at least 1×1), by box
/// filtering over the source pixels each output pixel covers; straight-alpha RGBA8.
/// `None` for an inconsistent image or `max_side` = 0.
#[must_use]
pub fn thumbnail(img: &RgbaImage, max_side: u32) -> Option<RgbaImage> {
    if !image_ok(img) || max_side == 0 {
        return None;
    }
    let (w, h) = (img.width, img.height);
    let scale = (f64::from(max_side) / f64::from(w.max(h))).min(1.0);
    let tw = ((f64::from(w) * scale).round() as u32).clamp(1, max_side);
    let th = ((f64::from(h) * scale).round() as u32).clamp(1, max_side);
    let mut pixels = Vec::with_capacity(tw as usize * th as usize * 4);
    // Cap the samples per output pixel so a huge source stays cheap.
    const MAX_SAMPLES: u32 = 8;
    for ty in 0..th {
        let y0 = nearest(ty, th, h) as u32;
        let y1 = ((u64::from(ty + 1) * u64::from(h)) / u64::from(th))
            .clamp(u64::from(y0) + 1, u64::from(h)) as u32;
        let ystep = (y1 - y0).div_ceil(MAX_SAMPLES).max(1);
        for tx in 0..tw {
            let x0 = nearest(tx, tw, w) as u32;
            let x1 = ((u64::from(tx + 1) * u64::from(w)) / u64::from(tw))
                .clamp(u64::from(x0) + 1, u64::from(w)) as u32;
            let xstep = (x1 - x0).div_ceil(MAX_SAMPLES).max(1);
            let mut acc = [0u32; 4];
            let mut n = 0u32;
            let mut y = y0;
            while y < y1 {
                let mut x = x0;
                while x < x1 {
                    let i = (y as usize * w as usize + x as usize) * 4;
                    if let Some(p) = img.pixels.get(i..i + 4) {
                        for c in 0..4 {
                            acc[c] += u32::from(p[c]);
                        }
                        n += 1;
                    }
                    x += xstep;
                }
                y += ystep;
            }
            let n = n.max(1);
            pixels.extend(acc.map(|v| (v / n) as u8));
        }
    }
    Some(RgbaImage {
        width: tw,
        height: th,
        pixels,
    })
}

/// `img` with its larger side at most `max_side` (aspect kept, box filtered as by
/// [`thumbnail`]); the image itself when it is already small enough. `None` for an
/// inconsistent image or `max_side` = 0.
#[must_use]
pub fn fit_image(img: &RgbaImage, max_side: u32) -> Option<std::borrow::Cow<'_, RgbaImage>> {
    if !image_ok(img) || max_side == 0 {
        return None;
    }
    if img.width.max(img.height) <= max_side {
        return Some(std::borrow::Cow::Borrowed(img));
    }
    thumbnail(img, max_side).map(std::borrow::Cow::Owned)
}

/// The axis-aligned bounds of box (`lo`, `hi`) after the column-major affine `transform`
/// (its eight corners transformed). `None` when an input or the result is not finite.
#[must_use]
pub fn transform_aabb(
    transform: &[f32; 16],
    lo: [f32; 3],
    hi: [f32; 3],
) -> Option<([f32; 3], [f32; 3])> {
    let m = transform;
    let mut out_lo = [f32::INFINITY; 3];
    let mut out_hi = [f32::NEG_INFINITY; 3];
    for corner in 0..8 {
        let p = [0, 1, 2].map(|i| if corner >> i & 1 == 0 { lo[i] } else { hi[i] });
        let q = transform_point(m, p);
        for i in 0..3 {
            out_lo[i] = out_lo[i].min(q[i]);
            out_hi[i] = out_hi[i].max(q[i]);
        }
    }
    out_lo
        .iter()
        .chain(&out_hi)
        .all(|v| v.is_finite())
        .then_some((out_lo, out_hi))
}

/// Point `p` under the column-major affine `transform`.
#[must_use]
pub fn transform_point(m: &[f32; 16], p: [f32; 3]) -> [f32; 3] {
    [0, 1, 2].map(|r| m[r] * p[0] + m[4 + r] * p[1] + m[8 + r] * p[2] + m[12 + r])
}

/// A vector of a Y-up frame (the renderer's procedural shapes, `render.md` §1) in the
/// viewer's Z-up frame (`mesh.md` §11): a +90° rotation about X, so +Y goes to +Z and
/// handedness (winding, tangent sign) is kept.
#[must_use]
pub fn y_up_to_z_up(v: [f32; 3]) -> [f32; 3] {
    [v[0], -v[2], v[1]]
}

/// A "nice" grid cell size (1, 2 or 5 × a power of ten) for a scene of `radius`, giving
/// roughly ten cells across the scene. Non-finite or non-positive radii give 1.
#[must_use]
pub fn grid_cell(radius: f32) -> f32 {
    if !radius.is_finite() || radius <= 0.0 {
        return 1.0;
    }
    let raw = f64::from(radius) / 5.0;
    let exp = raw.log10().floor();
    let base = 10f64.powf(exp);
    let m = raw / base;
    let nice = if m < 1.5 {
        1.0
    } else if m < 3.5 {
        2.0
    } else if m < 7.5 {
        5.0
    } else {
        10.0
    };
    let v = (nice * base) as f32;
    if v.is_finite() && v > 0.0 { v } else { 1.0 }
}

#[cfg(test)]
mod tests {
    use super::*;
    use tessera_mesh::{GeometryLimits, synth::MeshSpec};

    #[test]
    fn skin_streams_for_the_renderer() {
        // Format 0x3002_0112: two influences per vertex, bones up to 61 (tessera-mesh synth).
        let mesh = MeshSpec::single(0x3002_0112, 24).build().unwrap();
        let geo = mesh.level_geometry(0, &GeometryLimits::default()).unwrap();
        let s = mesh_skin(&geo, 62).unwrap();
        assert_eq!(s.skin.joints.len(), geo.vertex_count());
        assert_eq!((s.fixed, s.renormalised), (0, 0));
        assert!(
            s.skin
                .weights
                .iter()
                .all(|w| (w.iter().sum::<f32>() - 1.0).abs() < 1e-4)
        );
        // The renderer accepts them as they are.
        let data = level_mesh_data(&geo, &ConvertOptions::default(), "skin".into());
        assert!(
            data.prepare_skinned(&s.skin, &tessera_render::MeshConfig::default())
                .is_ok()
        );
        // A skeleton too small for the bones used, no skeleton, a palette overflow.
        assert!(mesh_skin(&geo, 10).unwrap_err().contains("bone"));
        assert!(mesh_skin(&geo, 0).is_err());
        assert!(mesh_skin(&geo, MAX_PALETTE_BONES + 1).is_err());
        // Hostile weights: zero rows go to bone 0, odd sums are rescaled, NaN is zero.
        let mut bad = geo.clone();
        if let Some(w) = &mut bad.weights {
            w[0] = [0.0; 4];
            w[1] = [2.0, 2.0, 0.0, 0.0];
            w[2] = [f32::NAN, 1.0, -3.0, 0.0];
        }
        let s = mesh_skin(&bad, 62).unwrap();
        assert_eq!(s.fixed, 1);
        assert_eq!(s.skin.joints[0], [0; 4]);
        assert_eq!(s.renormalised, 1);
        assert_eq!(s.skin.weights[1], [0.5, 0.5, 0.0, 0.0]);
        assert_eq!(s.skin.weights[2][0], 0.0);
        // Missing or short streams are refused.
        let mut none = geo.clone();
        none.joints = None;
        assert!(mesh_skin(&none, 62).is_err());
        let mut short = geo.clone();
        if let Some(j) = &mut short.joints {
            j.pop();
        }
        assert!(mesh_skin(&short, 62).is_err());
        // An unskinned level has no streams.
        let plain = MeshSpec::single(0x3000_0152, 24).build().unwrap();
        let plain = plain.level_geometry(0, &GeometryLimits::default()).unwrap();
        assert!(mesh_skin(&plain, 62).is_err());
    }

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
    fn srgb_curve() {
        assert_eq!(srgb_to_linear(0), 0.0);
        assert!((srgb_to_linear(255) - 1.0).abs() < 1e-6);
        assert!((srgb_to_linear(188) - 0.5).abs() < 0.01);
        assert_eq!(srgb_unit_to_linear(f32::NAN), 0.0);
        assert!((srgb_unit_to_linear(7.0) - 1.0).abs() < 1e-6);
    }

    #[test]
    fn converts_a_synthetic_level() {
        // Position, normal, colour, one UV set, tangents.
        let mut spec = MeshSpec::single(0x3000_0152, 12);
        spec.submeshes.push(spec.submeshes[0]);
        spec.levels[0].draws = vec![1, 0];
        let mesh = spec.build().unwrap();
        let geo = mesh.level_geometry(0, &GeometryLimits::default()).unwrap();
        let with_colors = ConvertOptions {
            vertex_colors: true,
            uv_set: 0,
        };
        let data = level_mesh_data(&geo, &with_colors, "x".into());
        assert_eq!(data.positions.len(), geo.positions.len());
        assert_eq!(data.submeshes.len(), 2);
        assert_eq!(data.submeshes[0].first_index, 0);
        assert_eq!(data.submeshes[1].first_index, data.submeshes[0].index_count);
        assert!(data.colors.is_some() && data.uvs.is_some() && data.tangents.is_some());
        let info = data.validate(&Default::default()).unwrap();
        assert_eq!(info.submeshes.len(), 2);
        let no_colors = level_mesh_data(
            &geo,
            &ConvertOptions {
                vertex_colors: false,
                uv_set: 5,
            },
            "y".into(),
        );
        assert!(no_colors.colors.is_none());
        // A missing UV set falls back to set 0.
        assert_eq!(no_colors.uvs, data.uvs);
    }

    #[test]
    fn opacity_composition() {
        let base = img(4, 4, |x, y| [x as u8, y as u8, 7, 10]);
        let alpha = img(2, 2, |x, _| [0, 0, 0, if x == 0 { 0 } else { 200 }]);
        let own = base_texture(Some(&base), AlphaSource::Own).unwrap();
        assert_eq!(own, base);
        let opaque = base_texture(Some(&base), AlphaSource::Opaque).unwrap();
        assert!(opaque.pixels.chunks(4).all(|p| p[3] == 255));
        let from = base_texture(Some(&base), AlphaSource::From(&alpha)).unwrap();
        assert_eq!(from.pixels[3], 0);
        assert_eq!(from.pixels[3 * 4 + 3], 200);
        assert_eq!(&from.pixels[4..7], &[1, 0, 7]);
        let white = base_texture(None, AlphaSource::From(&alpha)).unwrap();
        assert_eq!(white.width, 2);
        assert_eq!(&white.pixels[4..8], &[255, 255, 255, 200]);
        assert!(base_texture(None, AlphaSource::Opaque).is_none());
        // Inconsistent images are refused, never indexed.
        let bad = RgbaImage {
            width: 9,
            height: 9,
            pixels: vec![0; 3],
        };
        assert!(base_texture(Some(&bad), AlphaSource::Own).is_none());
        assert!(base_texture(Some(&base), AlphaSource::From(&bad)).is_none());
        assert!(alpha_mask(&bad, true).is_none());
        assert!(thumbnail(&bad, 8).is_none());
    }

    #[test]
    fn masks_and_thumbnails() {
        let src = img(3, 1, |x, _| [9, 9, 9, (x * 100) as u8]);
        let dye = alpha_mask(&src, true).unwrap();
        assert_eq!(&dye.pixels[..4], &[255, 255, 255, 255]);
        assert_eq!(&dye.pixels[8..12], &[55, 55, 55, 255]);
        let gloss = alpha_mask(&src, false).unwrap();
        assert_eq!(&gloss.pixels[4..8], &[100, 100, 100, 255]);
        let big = img(256, 64, |x, _| [(x % 2 * 255) as u8, 0, 0, 255]);
        let t = thumbnail(&big, 32).unwrap();
        assert_eq!((t.width, t.height), (32, 8));
        assert_eq!(t.pixels.len(), 32 * 8 * 4);
        // Box filtering averages the stripes.
        assert!((100..=155).contains(&t.pixels[0]));
        let small = thumbnail(&src, 64).unwrap();
        assert_eq!((small.width, small.height), (3, 1));
        assert!(thumbnail(&src, 0).is_none());
    }

    #[test]
    fn vertex_alpha_never_reaches_the_renderer() {
        // Regression: a stored vertex alpha below the alpha-test threshold deleted
        // geometry; the colour stream's alpha is always 1.
        let spec = MeshSpec::single(0x3000_0152, 12);
        let mesh = spec.build().unwrap();
        let mut geo = mesh.level_geometry(0, &GeometryLimits::default()).unwrap();
        let n = geo.positions.len();
        geo.colors = Some(vec![[10, 20, 30, 7]; n]);
        let on = ConvertOptions {
            vertex_colors: true,
            uv_set: 0,
        };
        let data = level_mesh_data(&geo, &on, "a".into());
        let colors = data.colors.unwrap();
        assert_eq!(colors.len(), n);
        assert!(colors.iter().all(|c| c[3] == 1.0));
        assert!((colors[0][2] - srgb_to_linear(30)).abs() < 1e-6);
        // Vertex colours are opt-in.
        assert!(!ConvertOptions::default().vertex_colors);
        assert!(
            level_mesh_data(&geo, &ConvertOptions::default(), "b".into())
                .colors
                .is_none()
        );
    }

    #[test]
    fn images_fit_a_side() {
        let big = img(300, 100, |x, _| [x as u8, 0, 0, 255]);
        let f = fit_image(&big, 64).unwrap();
        assert_eq!((f.width, f.height), (64, 21));
        assert!(matches!(
            fit_image(&big, 300).unwrap(),
            std::borrow::Cow::Borrowed(_)
        ));
        assert!(fit_image(&big, 0).is_none());
        let bad = RgbaImage {
            width: 4,
            height: 4,
            pixels: vec![0; 5],
        };
        assert!(fit_image(&bad, 2).is_none());
    }

    #[test]
    fn boxes_and_frames() {
        let identity: [f32; 16] = [
            1.0, 0.0, 0.0, 0.0, 0.0, 1.0, 0.0, 0.0, 0.0, 0.0, 1.0, 0.0, 0.0, 0.0, 0.0, 1.0,
        ];
        let b = ([-1.0, 0.0, 2.0], [1.0, 3.0, 4.0]);
        assert_eq!(transform_aabb(&identity, b.0, b.1), Some(b));
        // A quarter turn about Z and a translation by (10, 0, 0).
        let m: [f32; 16] = [
            0.0, 1.0, 0.0, 0.0, -1.0, 0.0, 0.0, 0.0, 0.0, 0.0, 1.0, 0.0, 10.0, 0.0, 0.0, 1.0,
        ];
        assert_eq!(transform_point(&m, [1.0, 0.0, 0.0]), [10.0, 1.0, 0.0]);
        let (lo, hi) = transform_aabb(&m, b.0, b.1).unwrap();
        assert_eq!((lo, hi), ([7.0, -1.0, 2.0], [10.0, 1.0, 4.0]));
        let mut nan = identity;
        nan[0] = f32::NAN;
        assert!(transform_aabb(&nan, b.0, b.1).is_none());
        // Y-up → Z-up keeps handedness: X × Y = Z maps to X' × Y' = Z'.
        assert_eq!(y_up_to_z_up([0.0, 1.0, 0.0]), [0.0, 0.0, 1.0]);
        assert_eq!(y_up_to_z_up([0.0, 0.0, 1.0]), [0.0, -1.0, 0.0]);
        assert_eq!(y_up_to_z_up([1.0, 0.0, 0.0]), [1.0, 0.0, 0.0]);
    }

    #[test]
    fn grid_cells() {
        assert_eq!(grid_cell(5.0), 1.0);
        assert_eq!(grid_cell(50.0), 10.0);
        assert_eq!(grid_cell(12.0), 2.0);
        assert_eq!(grid_cell(0.0), 1.0);
        assert_eq!(grid_cell(f32::NAN), 1.0);
        assert!(grid_cell(f32::MAX).is_finite());
        assert!(grid_cell(f32::MIN_POSITIVE) > 0.0);
    }
}
