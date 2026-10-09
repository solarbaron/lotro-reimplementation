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
use tessera_render::{AlphaMode, Indices, MAX_PALETTE_BONES, MeshData, MeshSkin, Submesh};

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

/// Where the gloss *g* of a lit surface comes from (`lighting.md` §5.2 item 5,
/// `material.md` §10.2: the role of the slot whose alpha carries it).
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub enum GlossFrom {
    /// *g* = 1.
    #[default]
    One,
    /// The alpha of the base texture (`ColorAndGloss`).
    BaseAlpha,
    /// The alpha of the normal map (`TSNormalAndGloss`).
    NormalAlpha,
}

/// The inputs only the lit pipeline uses (`lighting.md` §4.1, `material.md` §10–§11), in
/// the original's stored-value colour space (`lighting.md` §5.1).
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct LitInputs {
    /// Material colour *M* (2000) as stored, RGBA (alpha: the material alpha).
    pub color: [f32; 4],
    /// Alpha test and blend state of the lit pipeline (`docs/specs/render.md` §4.3).
    pub alpha: AlphaMode,
    /// The base texture's alpha is the opacity (else the opacity is 1).
    pub opacity_from_base: bool,
    /// Tangent-space normal map (slot 3011), with its own alpha.
    pub normal: Option<TexRequest>,
    /// Emissive texture (slot 3018).
    pub emissive: Option<TexRequest>,
    /// Glow mask (slot 3013).
    pub glow_mask: Option<TexRequest>,
    /// Glow colour *G* (2014) RGB; black without one.
    pub glow: [f32; 3],
    /// Specular colour *Ms* (2001) and power *p* (1000); `None` below power 0.5.
    pub specular: Option<([f32; 3], f32)>,
    /// Gloss source.
    pub gloss: GlossFrom,
}

impl LitInputs {
    /// The lit inputs of a submesh whose material could not be resolved.
    pub const FALLBACK: Self = Self {
        color: [0.8, 0.8, 0.8, 1.0],
        alpha: AlphaMode::Opaque,
        opacity_from_base: false,
        normal: None,
        emissive: None,
        glow_mask: None,
        glow: [0.0; 3],
        specular: None,
        gloss: GlossFrom::One,
    };
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
    /// The inputs of the lit pipeline (M4.5).
    pub lit: LitInputs,
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
        lit: LitInputs::FALLBACK,
    };

    /// Every texture the plan samples: the base texture, the dye mask and the lit
    /// pipeline's normal, emissive and glow textures.
    pub fn textures(&self) -> impl Iterator<Item = TexRequest> + '_ {
        self.texture
            .into_iter()
            .chain(self.dye.map(|d| d.0))
            .chain(self.lit.normal)
            .chain(self.lit.emissive)
            .chain(self.lit.glow_mask)
    }

    /// Whether the instanced scenery pipeline can draw the plan (`docs/specs/render.md`
    /// §5.5): it has no emissive, glow or dye input.
    #[must_use]
    pub fn instanceable(&self) -> bool {
        self.dye.is_none() && self.lit.emissive.is_none() && self.lit.glow_mask.is_none()
    }
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
    let lit = lit_inputs(m, texture, alpha, tint);
    // Gloss in the base alpha of an opaque surface needs that alpha (`material.md`
    // §10.2): the texture keeps its own alpha; the opacity stays 1.
    let texture = match (texture, lit.gloss) {
        (Some(t), GlossFrom::BaseAlpha) if t.variant == VARIANT_OPAQUE => Some(TexRequest {
            did: t.did,
            variant: VARIANT_OWN_ALPHA,
        }),
        (t, _) => t,
    };
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
        lit,
    }
}

/// A finite unit value (0–1), `fallback` when not finite.
fn unit(v: f32, fallback: f32) -> f32 {
    if v.is_finite() {
        v.clamp(0.0, 1.0)
    } else {
        fallback
    }
}

/// The lit-pipeline inputs of a resolved material (`lighting.md` §4.1; `material.md`
/// §10.2 gloss roles, §11.2–§11.5 alpha, blend and feature flags). `texture` and `alpha`
/// are the M2 plan's base texture and alpha use; `tint` the material colour 2000.
fn lit_inputs(
    m: &ResolvedMaterial,
    texture: Option<TexRequest>,
    alpha: AlphaKind,
    tint: [f32; 4],
) -> LitInputs {
    let f = &m.features;
    let slot_tex = |id: u32| m.slot(id).and_then(|s| s.texture).map(|d| d.0);
    let role = |id: u32| m.slot(id).and_then(|s| s.role.as_deref());
    let own = |did: u32| TexRequest {
        did,
        variant: VARIANT_OWN_ALPHA,
    };
    let opacity_from_base =
        texture.is_some_and(|t| t.variant != VARIANT_OPAQUE) && !matches!(alpha, AlphaKind::Opaque);
    let normal = if f.normal_mapped {
        slot_tex(ids::NORMAL)
            .filter(|&d| d > VARIANT_DYE_MASK)
            .map(own)
    } else {
        None
    };
    let specular = if f.specular {
        m.specular().and_then(|(c, p)| {
            (p.is_finite() && p >= 0.5)
                .then(|| ([unit(c[0], 1.0), unit(c[1], 1.0), unit(c[2], 1.0)], p))
        })
    } else {
        None
    };
    let gloss = if specular.is_none() {
        GlossFrom::One
    } else if role(ids::DIFFUSE).is_some_and(|r| r.contains("Gloss")) && texture.is_some() {
        GlossFrom::BaseAlpha
    } else if normal.is_some() && role(ids::NORMAL).is_some_and(|r| r.contains("Gloss")) {
        GlossFrom::NormalAlpha
    } else {
        GlossFrom::One
    };
    let glow = if f.glow {
        m.params.color(ids::GLOW_COLOR).map_or([0.0; 3], |c| {
            [unit(c[0], 0.0), unit(c[1], 0.0), unit(c[2], 0.0)]
        })
    } else {
        [0.0; 3]
    };
    let lit_glow = glow != [0.0; 3];
    let emissive = (f.emissive && lit_glow)
        .then(|| slot_tex(ids::EMISSIVE))
        .flatten()
        .filter(|&d| d > VARIANT_DYE_MASK)
        .map(own);
    let glow_mask = lit_glow
        .then(|| slot_tex(ids::GLOW))
        .flatten()
        .filter(|&d| d > VARIANT_DYE_MASK)
        .map(own);
    let color = [
        unit(tint[0], 1.0),
        unit(tint[1], 1.0),
        unit(tint[2], 1.0),
        unit(tint[3], 1.0),
    ];
    let threshold = f.alpha_test.map(|a| unit(a.threshold, 0.5));
    let translucent = opacity_from_base || color[3] < 1.0;
    let lit_alpha = match f.blend.mode {
        BlendMode::Opaque => AlphaMode::Opaque,
        BlendMode::Cutout => match (threshold, opacity_from_base) {
            (Some(c), true) => AlphaMode::Mask { cutoff: c },
            _ => AlphaMode::Opaque,
        },
        // Unlit records: the record's own blend triple and depth write as a fixed-function
        // blend, with its alpha test (`material.md` §11.3 rule for unlit records, §11.2;
        // `walker.md` §14.5 "Unlit blend pairs"); a triple with a code the renderer cannot
        // express keeps the source-alpha blend below.
        BlendMode::Additive | BlendMode::Custom
            if !f.lit
                && let Some(b) = fixed_blend(f, threshold.filter(|_| opacity_from_base)) =>
        {
            AlphaMode::Fixed(b)
        }
        BlendMode::Additive => AlphaMode::Additive,
        BlendMode::AlphaBlend | BlendMode::Custom if !translucent => AlphaMode::Opaque,
        BlendMode::AlphaBlend | BlendMode::Custom => {
            let cutoff = if opacity_from_base {
                threshold.unwrap_or(-1.0)
            } else {
                -1.0
            };
            if f.blend.depth_write {
                AlphaMode::TestBlend { cutoff }
            } else {
                AlphaMode::Blend
            }
        }
    };
    LitInputs {
        color,
        alpha: lit_alpha,
        opacity_from_base,
        normal,
        emissive,
        glow_mask,
        glow,
        specular,
        gloss,
    }
}

/// The renderer's fixed blend for an unlit record's blend state (`material.md` §11.3, the
/// D3D factor and operation codes of §7.6 as `tessera-material` names them): `None` when a
/// code is one the material crate does not name (no such record is known; the caller keeps
/// a source-alpha blend). `alpha_test` is the record's threshold when it tests an opacity
/// source (§11.2).
#[must_use]
pub fn fixed_blend(
    f: &tessera_material::FeatureFlags,
    alpha_test: Option<f32>,
) -> Option<tessera_render::FixedBlend> {
    use tessera_material::records::{BlendFactor as M, BlendOp as O};
    use tessera_render::{BlendFactor as R, BlendOp};
    let factor = |m: M| match m {
        M::Zero => Some(R::Zero),
        M::One => Some(R::One),
        M::SrcColor => Some(R::SrcColor),
        M::InvSrcColor => Some(R::OneMinusSrcColor),
        M::SrcAlpha => Some(R::SrcAlpha),
        M::InvSrcAlpha => Some(R::OneMinusSrcAlpha),
        M::DestAlpha => Some(R::DstAlpha),
        M::DestColor => Some(R::DstColor),
        M::Other(_) => None,
    };
    let op = match f.blend.op {
        O::Add => BlendOp::Add,
        O::RevSubtract => BlendOp::ReverseSubtract,
        O::Other(_) => return None,
    };
    Some(tessera_render::FixedBlend {
        src: factor(f.blend.src)?,
        dst: factor(f.blend.dst)?,
        op,
        depth_write: f.blend.depth_write,
        alpha_test: alpha_test.map(tessera_render::AlphaThreshold),
    })
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
        binormals: None,
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
            binormals: None,
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
    /// `walker.md` §14.5 "Unlit blend pairs", `material.md` §11.3: an unlit record's own
    /// blend triple and depth write become a fixed blend with its alpha test; a code the
    /// material crate does not name keeps the source-alpha blend.
    fn unlit_blend_triples_become_fixed_blends() {
        use tessera_material::records::{BlendFactor as M, BlendOp as O};
        use tessera_material::{BlendState, FeatureFlags};
        let flags = |src, dst, op, depth_write| FeatureFlags {
            lit: false,
            alpha_test: None,
            opacity_slot: None,
            blend: BlendState {
                mode: BlendMode::Custom,
                src,
                dst,
                op,
                depth_write,
                opacity_slot: None,
            },
            two_sided: false,
            normal_mapped: false,
            specular: false,
            dyeable: false,
            dyeable_2: false,
            emissive: false,
            glow: false,
            environment: false,
            second_layer: false,
            baked_lighting: false,
            uv_transform: false,
            animated: false,
        };
        let b = fixed_blend(&flags(M::DestColor, M::One, O::Add, true), Some(0.25)).unwrap();
        assert_eq!(
            (b.src, b.dst, b.op, b.depth_write),
            (
                tessera_render::BlendFactor::DstColor,
                tessera_render::BlendFactor::One,
                tessera_render::BlendOp::Add,
                true
            )
        );
        assert_eq!(b.alpha_test, Some(tessera_render::AlphaThreshold(0.25)));
        let r = fixed_blend(&flags(M::Zero, M::SrcColor, O::RevSubtract, false), None).unwrap();
        assert_eq!(r.op, tessera_render::BlendOp::ReverseSubtract);
        assert_eq!(r.alpha_test, None);
        assert!(fixed_blend(&flags(M::Other(8), M::One, O::Add, false), None).is_none());
        assert!(fixed_blend(&flags(M::One, M::One, O::Other(2), false), None).is_none());
    }

    #[test]
    fn srgb_conversion_bounds() {
        assert_eq!(srgb_unit_to_linear(f32::NAN), 0.0);
        assert_eq!(srgb_unit_to_linear(2.0), 1.0);
        assert!((srgb_unit_to_linear(0.5) - 0.214).abs() < 1e-3);
    }
}
