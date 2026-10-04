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
use tessera_render::{Indices, MeshData, Submesh};

/// Texture variant: RGB of the texture, alpha forced to 1.
pub const VARIANT_OPAQUE: u32 = 0;
/// Texture variant: RGB and the texture's own alpha.
pub const VARIANT_OWN_ALPHA: u32 = 1;
/// Texture variant: white RGB with the texture's alpha (an opacity map without a base
/// colour texture).
pub const VARIANT_ALPHA_ONLY: u32 = 2;

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
}

impl MaterialPlan {
    /// The plan for a submesh whose material could not be resolved: light grey, opaque.
    pub const FALLBACK: Self = Self {
        texture: None,
        alpha: AlphaKind::Opaque,
        two_sided: false,
        base_color: [0.6, 0.6, 0.6, 1.0],
        unlit: false,
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
            // DIDs 0–2 would collide with the variant codes; they are never real.
            (Some(d), Some(a)) if a > VARIANT_ALPHA_ONLY => Some(TexRequest { did: d, variant: a }),
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
    }
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

    #[test]
    fn srgb_conversion_bounds() {
        assert_eq!(srgb_unit_to_linear(f32::NAN), 0.0);
        assert_eq!(srgb_unit_to_linear(2.0), 1.0);
        assert!((srgb_unit_to_linear(0.5) - 0.214).abs() < 1e-3);
    }
}
