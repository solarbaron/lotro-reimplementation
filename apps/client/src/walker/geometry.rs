// SPDX-License-Identifier: GPL-3.0-or-later
//! Pure conversions for the walker's static objects: a decoded mesh level into the
//! renderer's [`MeshData`], a resolved material into a draw plan, and the texture the plan
//! samples (base colour with its opacity merged into alpha).
//!
//! Plain data in, plain data out (no GPU, no archives), so the fuzz target
//! `client_walker_text` includes this file by path. Images whose pixel buffer disagrees
//! with their size are refused, never indexed out of bounds.
//!
//! Conventions (`mesh.md` §11, `render.md` §2.1): positions pass through (Z-up, metres),
//! counter-clockwise front faces in both, UVs unflipped. Material colours are read as
//! sRGB-encoded (`material.md` §19 OPEN), the reading the mesh viewer uses.

use tessera_formats::RgbaImage;
use tessera_material::vocab::ids;
use tessera_material::{BlendMode, ResolvedMaterial};
use tessera_mesh::{Indices as MeshIndices, LevelGeometry, LevelKind, Mesh};
use tessera_render::{Indices, MAX_PALETTE_BONES, MeshData, MeshSkin, Submesh};

/// Texture variant: RGB of the texture, alpha forced to 1.
pub const VARIANT_OPAQUE: u32 = 0;
/// Texture variant: RGB and the texture's own alpha.
pub const VARIANT_OWN_ALPHA: u32 = 1;
/// Texture variant: white RGB with the texture's alpha (an opacity map without a base
/// colour texture).
pub const VARIANT_ALPHA_ONLY: u32 = 2;
/// Texture variant: a dye mask in the renderer's convention (the value in R, G and B,
/// alpha 255) from the texture's alpha, inverted (`material.md` §12.2: low alpha marks
/// dyeable texels; the renderer wants high values there). Uploaded linear.
pub const VARIANT_DYE_MASK: u32 = 3;

/// How a planned material uses alpha.
#[derive(Debug, Clone, Copy, PartialEq)]
pub enum AlphaKind {
    /// Opaque.
    Opaque,
    /// Alpha test with this threshold (0–1).
    Mask(f32),
    /// Alpha blending.
    Blend,
}

/// The texture a plan samples: `did` is a texture list (or surface) DID; `variant` is
/// [`VARIANT_OPAQUE`], [`VARIANT_OWN_ALPHA`], [`VARIANT_ALPHA_ONLY`] or, for any other
/// value, the DID of a separate opacity texture whose alpha replaces `did`'s.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, PartialOrd, Ord)]
pub struct TexRequest {
    /// Base texture (or the opacity texture for [`VARIANT_ALPHA_ONLY`]).
    pub did: u32,
    /// Alpha source (see the type docs).
    pub variant: u32,
}

/// What one submesh needs to be drawn.
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct MaterialPlan {
    /// Base-colour texture, if any.
    pub texture: Option<TexRequest>,
    /// Alpha use.
    pub alpha: AlphaKind,
    /// Draw both faces.
    pub two_sided: bool,
    /// Linear base colour factor.
    pub base_color: [f32; 4],
    /// Skip lighting.
    pub unlit: bool,
    /// Dye: the mask texture ([`VARIANT_DYE_MASK`]) and the linear dye colour. Only
    /// figures use dye ([`plan_material_dyed`]); static objects never do (`walker.md` §4).
    pub dye: Option<(TexRequest, [f32; 3])>,
}

impl MaterialPlan {
    /// The plan for a submesh whose material could not be resolved: light grey, opaque.
    pub const FALLBACK: Self = Self {
        texture: None,
        alpha: AlphaKind::Opaque,
        two_sided: false,
        base_color: [0.6, 0.6, 0.6, 1.0],
        unlit: false,
        dye: None,
    };
}

/// 8-bit sRGB-encoded unit value to linear light (clamped; non-finite → 0).
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

/// Plans the draw of a resolved material (`material.md` §10–§11): the diffuse slot's
/// texture with the opacity source of the alpha test (else of the blend), the alpha mode,
/// two-sidedness, the diffuse tint and whether it is lit. Normal maps, specular and dye
/// are not used by the walker.
#[must_use]
pub fn plan_material(m: &ResolvedMaterial) -> MaterialPlan {
    let f = &m.features;
    let slot_tex = |id: u32| m.slot(id).and_then(|s| s.texture).map(|d| d.0);
    let opacity = f
        .alpha_test
        .map(|a| a.source_slot)
        .or(if f.blend.is_transparent() {
            f.blend.opacity_slot
        } else {
            None
        });
    let diffuse = slot_tex(ids::DIFFUSE);
    let texture = match (diffuse, opacity) {
        (None, None) => None,
        (Some(d), None) => Some(TexRequest {
            did: d,
            variant: VARIANT_OPAQUE,
        }),
        (Some(d), Some(ids::DIFFUSE)) => Some(TexRequest {
            did: d,
            variant: VARIANT_OWN_ALPHA,
        }),
        (d, Some(slot)) => match (d, slot_tex(slot)) {
            // An opacity slot without a texture: opacity 1 (material.md §18 OPEN).
            (Some(d), None) => Some(TexRequest {
                did: d,
                variant: VARIANT_OPAQUE,
            }),
            (Some(d), Some(a)) if a == d => Some(TexRequest {
                did: d,
                variant: VARIANT_OWN_ALPHA,
            }),
            // DIDs 0–3 would collide with the variant codes; they are never real.
            (Some(d), Some(a)) if a > VARIANT_DYE_MASK => Some(TexRequest { did: d, variant: a }),
            (Some(d), Some(_)) => Some(TexRequest {
                did: d,
                variant: VARIANT_OPAQUE,
            }),
            (None, Some(a)) => Some(TexRequest {
                did: a,
                variant: VARIANT_ALPHA_ONLY,
            }),
            (None, None) => None,
        },
    };
    let finite01 = |v: f32, fallback: f32| {
        if v.is_finite() {
            v.clamp(0.0, 1.0)
        } else {
            fallback
        }
    };
    let alpha = match f.blend.mode {
        BlendMode::Opaque => AlphaKind::Opaque,
        BlendMode::Cutout => f.alpha_test.map_or(AlphaKind::Opaque, |a| {
            AlphaKind::Mask(finite01(a.threshold, 0.5))
        }),
        BlendMode::AlphaBlend | BlendMode::Additive | BlendMode::Custom => AlphaKind::Blend,
    };
    // An alpha mode without an alpha source would hide or fade the whole surface.
    let alpha = match (alpha, texture) {
        (AlphaKind::Mask(_) | AlphaKind::Blend, Some(t)) if t.variant == VARIANT_OPAQUE => {
            AlphaKind::Opaque
        }
        (AlphaKind::Mask(_) | AlphaKind::Blend, None) => AlphaKind::Opaque,
        (a, _) => a,
    };
    let tint = m.params.color(ids::DIFFUSE_COLOR).unwrap_or([1.0; 4]);
    MaterialPlan {
        texture,
        alpha,
        two_sided: f.two_sided,
        base_color: [
            srgb_unit_to_linear(tint[0]),
            srgb_unit_to_linear(tint[1]),
            srgb_unit_to_linear(tint[2]),
            1.0,
        ],
        unlit: !f.lit,
        dye: None,
    }
}

/// [`plan_material`] plus the material's dye (`material.md` §12): the first dye channel
/// with a mask texture, unless only the second has a colour other than white (the renderer
/// draws one channel).
#[must_use]
pub fn plan_material_dyed(m: &ResolvedMaterial) -> MaterialPlan {
    let mut plan = plan_material(m);
    if !m.features.dyeable {
        return plan;
    }
    let Some(setup) = m.dye else {
        return plan;
    };
    let channel = |mask: Option<tessera_formats::Did>, color: [f32; 4]| {
        let list = mask?;
        (list.0 > VARIANT_DYE_MASK).then_some((
            TexRequest {
                did: list.0,
                variant: VARIANT_DYE_MASK,
            },
            [
                srgb_unit_to_linear(color[0]),
                srgb_unit_to_linear(color[1]),
                srgb_unit_to_linear(color[2]),
            ],
            color[..3].iter().all(|v| *v >= 0.999),
        ))
    };
    let first = channel(setup.primary.mask.texture, setup.primary.color);
    let second = setup
        .secondary
        .and_then(|c| channel(c.mask.texture, c.color));
    let chosen = pick_dye_channel(
        first.map(|(t, c, white)| ((t, c), white)),
        second.map(|(t, c, white)| ((t, c), white)),
    );
    plan.dye = chosen;
    plan
}

/// The dye channel drawn (`walker.md` §10.4): the first channel that has a mask, unless only
/// the second has a colour other than white. Each channel is `(value, is_white)`.
fn pick_dye_channel<T>(first: Option<(T, bool)>, second: Option<(T, bool)>) -> Option<T> {
    match (first, second) {
        (Some((_, true)), Some((s, false))) => Some(s),
        (f, s) => f.or(s).map(|(v, _)| v),
    }
}

/// The dye mask of `src` ([`VARIANT_DYE_MASK`]): R = G = B = 255 − alpha, alpha 255.
/// `None` for an inconsistent image.
#[must_use]
pub fn dye_mask(src: &RgbaImage) -> Option<RgbaImage> {
    if !image_ok(src) {
        return None;
    }
    let mut out = src.clone();
    for px in out.pixels.as_chunks_mut::<4>().0 {
        let v = 255 - px[3];
        *px = [v, v, v, 255];
    }
    Some(out)
}

/// The level a static object draws: the first detail level (`mesh.md` §8; proxy levels
/// are ignored), else level 0. `None` for a mesh without levels.
#[must_use]
pub fn display_level(mesh: &Mesh) -> Option<usize> {
    if mesh.levels.is_empty() {
        return None;
    }
    Some(
        mesh.levels
            .iter()
            .position(|l| l.kind == LevelKind::Detail)
            .unwrap_or(0),
    )
}

/// The renderer streams of one flattened level: one renderer submesh per draw entry,
/// in draw order; UV set 0, no vertex colours (`mesh.md` §5.4 leaves their use OPEN).
#[must_use]
pub fn level_mesh_data(geo: &LevelGeometry, label: String) -> MeshData {
    let indices = match &geo.indices {
        MeshIndices::U16(v) => Indices::U16(v.clone()),
        MeshIndices::U32(v) => Indices::U32(v.clone()),
    };
    MeshData {
        label: Some(label),
        positions: geo.positions.clone(),
        normals: Some(geo.normals.clone()),
        tangents: None,
        uvs: geo.uv_sets.first().cloned(),
        colors: None,
        indices,
        submeshes: geo
            .parts
            .iter()
            .map(|p| Submesh {
                first_index: p.first_index,
                index_count: p.index_count,
            })
            .collect(),
    }
}

/// The renderer's bone streams of a flattened skinned level, for a skeleton of `bones`
/// bones (`skeleton-anim.md` §4.1, §4.6): joints as stored, weights cleaned (non-finite or
/// negative weights count as 0) and renormalised; a vertex without any weight follows bone
/// 0 rigidly. Returns the skin and the number of vertices that had no weight.
///
/// # Errors
///
/// When `bones` is 0 or above the renderer's palette, the level has no bone streams or
/// their lengths disagree with the positions, or a weighted joint is not below `bones`.
pub fn mesh_skin(geo: &LevelGeometry, bones: usize) -> Result<(MeshSkin, usize), String> {
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
    let mut fixed = 0usize;
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
            for x in &mut w {
                *x /= sum;
            }
        }
        out_w.push(w);
        out_j.push(*j);
    }
    Ok((
        MeshSkin {
            joints: out_j,
            weights: out_w,
            palette_size: u16::try_from(bones).unwrap_or(u16::MAX),
        },
        fixed,
    ))
}

/// Whether `img` is non-empty and holds exactly `width × height × 4` bytes.
#[must_use]
pub fn image_ok(img: &RgbaImage) -> bool {
    img.width > 0
        && img.height > 0
        && (img.width as usize)
            .checked_mul(img.height as usize)
            .and_then(|n| n.checked_mul(4))
            .is_some_and(|n| n == img.pixels.len())
}

/// Where a composed texture's alpha comes from.
#[derive(Debug, Clone, Copy)]
pub enum AlphaFrom<'a> {
    /// Alpha 1.
    Opaque,
    /// The base image's own alpha.
    Own,
    /// The alpha of another image, resampled (nearest) to the base image's size.
    Image(&'a RgbaImage),
}

/// The texture a [`MaterialPlan`] samples: RGB of `base` (white without one) and alpha per
/// `alpha`. `None` when there is nothing to compose or an input is inconsistent.
#[must_use]
pub fn compose_texture(base: Option<&RgbaImage>, alpha: AlphaFrom<'_>) -> Option<RgbaImage> {
    let mask = match alpha {
        AlphaFrom::Image(a) if image_ok(a) => Some(a),
        AlphaFrom::Image(_) => return None,
        _ => None,
    };
    let Some(base) = base else {
        let a = mask?;
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
    match (alpha, mask) {
        (AlphaFrom::Own, _) => {}
        (_, Some(a)) => {
            let (w, h) = (out.width as usize, out.height as usize);
            let (aw, ah) = (a.width as usize, a.height as usize);
            for y in 0..h {
                let sy = (y * ah / h).min(ah - 1);
                for x in 0..w {
                    let sx = (x * aw / w).min(aw - 1);
                    let v = a.pixels.get((sy * aw + sx) * 4 + 3).copied().unwrap_or(255);
                    if let Some(p) = out.pixels.get_mut((y * w + x) * 4 + 3) {
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

/// Halves an image (2 × 2 box filter) until both sides are at most `max` (at least 1).
/// Inconsistent images are returned unchanged.
#[must_use]
pub fn downscale_to(mut img: RgbaImage, max: u32) -> RgbaImage {
    let max = max.max(1);
    while image_ok(&img) && (img.width > max || img.height > max) {
        let (w, h) = (img.width as usize, img.height as usize);
        let (nw, nh) = ((w / 2).max(1), (h / 2).max(1));
        let mut out = vec![0u8; nw * nh * 4];
        for y in 0..nh {
            for x in 0..nw {
                for c in 0..4 {
                    let mut sum = 0u32;
                    for (dx, dy) in [(0, 0), (1, 0), (0, 1), (1, 1)] {
                        let sx = (x * 2 + dx).min(w - 1);
                        let sy = (y * 2 + dy).min(h - 1);
                        sum += u32::from(img.pixels[(sy * w + sx) * 4 + c]);
                    }
                    out[(y * nw + x) * 4 + c] = (sum / 4) as u8;
                }
            }
        }
        img = RgbaImage {
            width: nw as u32,
            height: nh as u32,
            pixels: out,
        };
    }
    img
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
    fn compose_alpha_sources() {
        let base = img(2, 2, |x, _| [10, 20, 30, if x == 0 { 0 } else { 99 }]);
        let own = compose_texture(Some(&base), AlphaFrom::Own).unwrap();
        assert_eq!(own.pixels, base.pixels);
        let opaque = compose_texture(Some(&base), AlphaFrom::Opaque).unwrap();
        assert!(opaque.pixels.iter().skip(3).step_by(4).all(|&a| a == 255));
        let mask = img(1, 1, |_, _| [0, 0, 0, 7]);
        let merged = compose_texture(Some(&base), AlphaFrom::Image(&mask)).unwrap();
        assert!(merged.pixels.iter().skip(3).step_by(4).all(|&a| a == 7));
        assert_eq!(&merged.pixels[..3], &[10, 20, 30]);
        let only = compose_texture(None, AlphaFrom::Image(&mask)).unwrap();
        assert_eq!(only.pixels, vec![255, 255, 255, 7]);
        assert!(compose_texture(None, AlphaFrom::Own).is_none());
        let bad = RgbaImage {
            width: 4,
            height: 4,
            pixels: vec![0; 3],
        };
        assert!(compose_texture(Some(&bad), AlphaFrom::Own).is_none());
        assert!(compose_texture(Some(&base), AlphaFrom::Image(&bad)).is_none());
    }

    #[test]
    fn downscale_halves_until_within_cap() {
        let big = img(8, 3, |x, y| [(x * 10) as u8, (y * 10) as u8, 0, 255]);
        let small = downscale_to(big, 2);
        assert_eq!((small.width, small.height), (2, 1));
        assert!(image_ok(&small));
        let same = downscale_to(img(2, 2, |_, _| [1, 2, 3, 4]), 0);
        assert_eq!((same.width, same.height), (1, 1));
        assert_eq!(&same.pixels, &[1, 2, 3, 4]);
    }

    fn skinned(joints: Vec<[u8; 4]>, weights: Vec<[f32; 4]>) -> LevelGeometry {
        LevelGeometry {
            level: 0,
            positions: vec![[0.0; 3]; joints.len()],
            normals: vec![[0.0, 0.0, 1.0]; joints.len()],
            tangents: None,
            uv_sets: Vec::new(),
            colors: None,
            joints: Some(joints),
            weights: Some(weights),
            indices: MeshIndices::U16(Vec::new()),
            parts: Vec::new(),
            bounds: None,
        }
    }

    #[test]
    fn mesh_skin_cleans_and_renormalises_weights() {
        let geo = skinned(
            vec![[1, 2, 0, 0], [3, 0, 0, 0], [9, 9, 9, 9], [4, 5, 0, 0]],
            vec![
                [2.0, 2.0, 0.0, 0.0],              // renormalised
                [1.0, 0.0, 0.0, 0.0],              // kept
                [0.0, -1.0, f32::NAN, 0.0],        // no weight: bone 0 rigidly
                [0.5, f32::INFINITY, 0.0, 0.5e-5], // non-finite counts as 0
            ],
        );
        let (skin, fixed) = mesh_skin(&geo, 8).unwrap();
        assert_eq!(fixed, 1);
        assert_eq!(skin.palette_size, 8);
        assert_eq!(skin.weights[0], [0.5, 0.5, 0.0, 0.0]);
        assert_eq!(skin.weights[1], [1.0, 0.0, 0.0, 0.0]);
        assert_eq!(
            (skin.joints[2], skin.weights[2]),
            ([0; 4], [1.0, 0.0, 0.0, 0.0])
        );
        let w = skin.weights[3];
        assert!((w.iter().sum::<f32>() - 1.0).abs() < 1e-6 && w[1] == 0.0);
        // Joints out of range are fine while unweighted (vertex 2), an error when weighted.
        let bad = skinned(vec![[8, 0, 0, 0]], vec![[1.0, 0.0, 0.0, 0.0]]);
        assert!(mesh_skin(&bad, 8).is_err());
        assert!(mesh_skin(&bad, 9).is_ok());
        assert!(mesh_skin(&geo, 0).is_err());
        assert!(mesh_skin(&geo, MAX_PALETTE_BONES + 1).is_err());
        let mut short = geo.clone();
        short.positions.push([0.0; 3]);
        assert!(mesh_skin(&short, 8).is_err(), "stream lengths disagree");
        let mut none = geo;
        none.weights = None;
        assert!(mesh_skin(&none, 8).is_err());
    }

    #[test]
    fn dye_mask_inverts_alpha() {
        let src = img(2, 1, |x, _| [1, 2, 3, if x == 0 { 0 } else { 200 }]);
        let m = dye_mask(&src).unwrap();
        assert_eq!(m.pixels, vec![255, 255, 255, 255, 55, 55, 55, 255]);
        let broken = RgbaImage {
            width: 2,
            height: 2,
            pixels: vec![0; 4],
        };
        assert!(dye_mask(&broken).is_none());
    }

    #[test]
    fn dye_channel_choice() {
        let w = |v: u8| Some((v, true));
        let c = |v: u8| Some((v, false));
        assert_eq!(pick_dye_channel(c(1), c(2)), Some(1), "first wins");
        assert_eq!(pick_dye_channel(w(1), w(2)), Some(1), "both white: first");
        assert_eq!(
            pick_dye_channel(w(1), c(2)),
            Some(2),
            "only the second coloured"
        );
        assert_eq!(pick_dye_channel(c(1), w(2)), Some(1));
        assert_eq!(
            pick_dye_channel(None, w(2)),
            Some(2),
            "first without a mask"
        );
        assert_eq!(pick_dye_channel(w(1), None), Some(1));
        assert_eq!(pick_dye_channel::<u8>(None, None), None);
    }

    #[test]
    fn srgb_conversion_bounds() {
        assert_eq!(srgb_unit_to_linear(f32::NAN), 0.0);
        assert_eq!(srgb_unit_to_linear(2.0), 1.0);
        assert!((srgb_unit_to_linear(0.5) - 0.214).abs() < 1e-3);
    }
}
