// SPDX-License-Identifier: GPL-3.0-or-later
//! Mesh materials, lighting and debug views — the inputs of a mesh draw.
//!
//! A [`Material`] is plain data (it holds texture handles, not GPU
//! objects), so a caller can change a dye colour or alpha threshold from one
//! frame to the next for free. The renderer picks one of its own WGSL
//! pipelines from the material's [`AlphaMode`] and `two_sided` flag; the
//! remaining features (textures, dye, normal map, specular) are uniform
//! switches in the same shader.
//!
//! Material **semantics** for game data (which texture slot is which, the
//! dye blend rule, alpha thresholds) belong to the material spec; this module
//! only defines what the renderer can draw. See `docs/specs/render.md` §2.3.

use glam::{Mat4, Vec3};

use crate::arena::MeshId;
use crate::frame::Rgba;
use crate::skin::SkinPalette;
use crate::texture::TextureId;

/// How a material's alpha is used.
#[derive(Debug, Clone, Copy, PartialEq, Default)]
pub enum AlphaMode {
    /// Alpha is ignored; depth is written.
    #[default]
    Opaque,
    /// Alpha test: fragments with alpha below `cutoff` are discarded, the
    /// rest are drawn opaque with depth writes.
    Mask {
        /// Threshold in `0..=1` (non-finite values act as 0.5).
        cutoff: f32,
    },
    /// Alpha blending (`src * a + dst * (1 - a)`), no depth writes. Blended
    /// draws of a pass are drawn after everything else, sorted back to
    /// front by the distance of their submesh's bounding-sphere centre.
    ///
    /// In lit passes this is the blend **without** depth write (unlit blend records whose
    /// render state has depth write off). Lit surfaces, whose base passes blend **with**
    /// depth write (`lighting.md` §2.1, `material.md` §11.3), use
    /// [`AlphaMode::TestBlend`] (or [`AlphaMode::BLEND_DEPTH_WRITE`] when the alpha test is
    /// off).
    Blend,
    /// Alpha test **and** blend (lit passes, `material.md` §11.3): fragments with
    /// opacity ≤ `cutoff` are discarded, the rest blend by their alpha
    /// (`src * a + dst * (1 - a)`) **with** depth writes, sorted with the blended
    /// draws. Outside lit passes it draws as [`AlphaMode::Mask`].
    TestBlend {
        /// Threshold, normally in `0..=1` (non-finite values act as 0.5). A negative
        /// cutoff keeps every texel: blend with depth write and no test
        /// ([`AlphaMode::BLEND_DEPTH_WRITE`]).
        cutoff: f32,
    },
    /// Additive blending (`src * a + dst`, `material.md` §11.3 "Additive"), no depth
    /// writes, sorted with the blended draws; in lit passes fog fades it out by
    /// (1 − *f*) (Tessera Decision: `lighting.md` §6 and §10 state that rule for the
    /// additive light passes; lerping an additive effect toward the fog colour would
    /// brighten fogged distance). Outside lit passes it draws as [`AlphaMode::Blend`].
    ///
    /// The same as [`AlphaMode::Fixed`] with [`FixedBlend::ADDITIVE`]. An additive surface
    /// whose render state writes depth (`material.md` §7.6: the unlit SRCALPHA / ONE
    /// shaders have depth write on) uses [`AlphaMode::Fixed`] with `depth_write` set.
    Additive,
    /// The fixed-function blend state of an unlit record (`material.md` §11.3 rule for
    /// unlit records: the most common (src, dst, op) triple of the record's render-state
    /// blocks, the D3D factor codes of §7.6 mapped by the caller, depth write from the
    /// same block). No alpha test; drawn with the blended draws, sorted back to front; casts
    /// no shadow. The opacity a factor reads is the output alpha (opacity × *M*.alpha, as
    /// for [`AlphaMode::Blend`]). In lit passes fog follows [`FixedBlend`]'s fog rule.
    /// Outside lit passes it draws as [`AlphaMode::Blend`].
    Fixed(FixedBlend),
}

/// A blend factor of [`FixedBlend`] (the D3D blend codes of `material.md` §7.6, mapped by
/// the caller: ZERO, ONE, SRCCOLOR, INVSRCCOLOR, SRCALPHA, INVSRCALPHA, DESTALPHA,
/// INVDESTALPHA, DESTCOLOR, INVDESTCOLOR).
#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash)]
pub enum BlendFactor {
    /// 0.
    Zero,
    /// 1.
    One,
    /// The source colour.
    SrcColor,
    /// 1 − the source colour.
    OneMinusSrcColor,
    /// The source alpha (the surface's output alpha).
    SrcAlpha,
    /// 1 − the source alpha.
    OneMinusSrcAlpha,
    /// The destination alpha.
    DstAlpha,
    /// 1 − the destination alpha.
    OneMinusDstAlpha,
    /// The destination colour.
    DstColor,
    /// 1 − the destination colour.
    OneMinusDstColor,
}

/// The operation of [`FixedBlend`] (D3D blend operations, `material.md` §7.6).
#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash)]
pub enum BlendOp {
    /// src · *s* + dst · *d*.
    Add,
    /// src · *s* − dst · *d*.
    Subtract,
    /// dst · *d* − src · *s*.
    ReverseSubtract,
    /// min(src, dst); the factors are ignored.
    Min,
    /// max(src, dst); the factors are ignored.
    Max,
}

/// A fixed-function blend state ([`AlphaMode::Fixed`]): colour = op(src · `src`, dst ·
/// `dst`), and whether the surface writes depth. The target's alpha channel is kept
/// (Tessera's choice; the presented image is opaque), also in a pass with
/// `Pass::premultiplied_alpha`, except for SRCALPHA / INVSRCALPHA, ADD (source-over) and
/// the opaque draws with destination factor ZERO (`docs/specs/render.md` §6.10 "Fixed
/// blends", R12).
#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash)]
pub struct FixedBlend {
    /// Source factor.
    pub src: BlendFactor,
    /// Destination factor.
    pub dst: BlendFactor,
    /// Operation.
    pub op: BlendOp,
    /// Depth write (`depth_write` of the render-state block, `material.md` §7.6, §11.3).
    pub depth_write: bool,
}

/// How a fixed blend state writes the target's alpha (`docs/specs/render.md` §6.10 "Fixed
/// blends", R12). The colour written never depends on it.
#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash)]
pub(crate) enum FixedAlpha {
    /// Keep the destination alpha (source factor zero, destination factor one, add):
    /// every fixed blend outside premultiplied passes, and inside them every blend but the
    /// two below.
    Keep,
    /// Source-over (`One` / `OneMinusSrcAlpha`, add): SRCALPHA / INVSRCALPHA, ADD in a
    /// premultiplied pass.
    Over,
    /// Write alpha 1, as opaque draws do: destination factor ZERO with a source factor that
    /// does not read the destination (ZERO, ONE, SRCALPHA, INVSRCALPHA), ADD, in a
    /// premultiplied pass. Such a blend never reads the destination, so the pipeline draws
    /// without blending and its fragment shader applies the source factor
    /// ([`FixedPipeline::fixed_opaque`]).
    Opaque,
}

/// Name of the pipeline-overridable constant of the lit, instanced and sky shaders
/// (`frame_shading.wgsl`) that makes a fixed blend draw opaque.
pub(crate) const FIXED_OPAQUE: &str = "FIXED_OPAQUE";

/// How a fixed-blend pipeline is built ([`FixedBlend::pipeline`]).
#[derive(Debug, Clone, Copy, PartialEq)]
pub(crate) struct FixedPipeline {
    /// Blend state of the colour target; `None`: no blending (the fragment replaces the
    /// target).
    pub blend: Option<wgpu::BlendState>,
    /// Value of the shaders' [`FIXED_OPAQUE`] override: 0 = the fragment writes the
    /// colour and alpha it computed; otherwise it writes alpha 1 and the colour times the
    /// source factor, 1 = ZERO, 2 = ONE, 3 = SRCALPHA, 4 = INVSRCALPHA.
    pub fixed_opaque: u32,
}

impl FixedPipeline {
    /// The pipeline-overridable constants of the fragment stage.
    pub(crate) fn constants(&self) -> &'static [(&'static str, f64)] {
        match self.fixed_opaque {
            1 => &[(FIXED_OPAQUE, 1.0)],
            2 => &[(FIXED_OPAQUE, 2.0)],
            3 => &[(FIXED_OPAQUE, 3.0)],
            4 => &[(FIXED_OPAQUE, 4.0)],
            _ => &[],
        }
    }
}

/// How fog acts on an [`AlphaMode::Fixed`] surface in lit passes (Tessera Decision,
/// `render.md` §4.3 implementer OPEN: fog fades a blended contribution toward what leaves
/// the destination unchanged).
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(crate) enum FogRule {
    /// Lerp toward the fog colour (as a base pass).
    Lerp,
    /// Scale the colour by (1 − *f*): the destination factor is ONE, so a black source
    /// leaves the destination unchanged (as [`AlphaMode::Additive`]).
    FadeOut,
    /// Lerp toward white: a modulating blend (ZERO / SRCCOLOR or DESTCOLOR / ZERO, add),
    /// where a white source leaves the destination unchanged.
    TowardWhite,
}

impl FixedBlend {
    /// SRCALPHA / ONE, add, no depth write: [`AlphaMode::Additive`].
    pub const ADDITIVE: Self = Self {
        src: BlendFactor::SrcAlpha,
        dst: BlendFactor::One,
        op: BlendOp::Add,
        depth_write: false,
    };

    /// How this blend writes alpha in a pass with (`premultiplied`) or without the
    /// premultiplied option (`docs/specs/render.md` §6.10 "Fixed blends", R12).
    pub(crate) fn alpha_rule(self, premultiplied: bool) -> FixedAlpha {
        use BlendFactor as F;
        if !premultiplied || self.op != BlendOp::Add {
            return FixedAlpha::Keep;
        }
        match (self.src, self.dst) {
            (F::SrcAlpha, F::OneMinusSrcAlpha) => FixedAlpha::Over,
            (F::Zero | F::One | F::SrcAlpha | F::OneMinusSrcAlpha, F::Zero) => FixedAlpha::Opaque,
            _ => FixedAlpha::Keep,
        }
    }

    /// The pipeline of this blend: the colour by the factors and operation (factors ONE
    /// for min and max, which ignore them), the alpha by `alpha` (from
    /// [`Self::alpha_rule`]). Every pipeline built here is checked against R12 by a unit
    /// test of `ui::api`.
    pub(crate) fn pipeline(self, alpha: FixedAlpha) -> FixedPipeline {
        use BlendFactor as F;
        if alpha == FixedAlpha::Opaque {
            // Destination factor ZERO and ADD: colour = source × source factor, no
            // destination read; alpha 1.
            let fixed_opaque = match self.src {
                F::Zero => 1,
                F::One => 2,
                F::SrcAlpha => 3,
                F::OneMinusSrcAlpha => 4,
                // `alpha_rule` gives `Opaque` for these four only; blend as usual otherwise.
                _ => return self.pipeline(FixedAlpha::Keep),
            };
            return FixedPipeline {
                blend: None,
                fixed_opaque,
            };
        }
        let factor = |f: BlendFactor| match f {
            F::Zero => wgpu::BlendFactor::Zero,
            F::One => wgpu::BlendFactor::One,
            F::SrcColor => wgpu::BlendFactor::Src,
            F::OneMinusSrcColor => wgpu::BlendFactor::OneMinusSrc,
            F::SrcAlpha => wgpu::BlendFactor::SrcAlpha,
            F::OneMinusSrcAlpha => wgpu::BlendFactor::OneMinusSrcAlpha,
            F::DstAlpha => wgpu::BlendFactor::DstAlpha,
            F::OneMinusDstAlpha => wgpu::BlendFactor::OneMinusDstAlpha,
            F::DstColor => wgpu::BlendFactor::Dst,
            F::OneMinusDstColor => wgpu::BlendFactor::OneMinusDst,
        };
        let (operation, minmax) = match self.op {
            BlendOp::Add => (wgpu::BlendOperation::Add, false),
            BlendOp::Subtract => (wgpu::BlendOperation::Subtract, false),
            BlendOp::ReverseSubtract => (wgpu::BlendOperation::ReverseSubtract, false),
            BlendOp::Min => (wgpu::BlendOperation::Min, true),
            BlendOp::Max => (wgpu::BlendOperation::Max, true),
        };
        let (src_factor, dst_factor) = if minmax {
            (wgpu::BlendFactor::One, wgpu::BlendFactor::One)
        } else {
            (factor(self.src), factor(self.dst))
        };
        let alpha = if alpha == FixedAlpha::Over {
            wgpu::BlendComponent::OVER
        } else {
            wgpu::BlendComponent {
                src_factor: wgpu::BlendFactor::Zero,
                dst_factor: wgpu::BlendFactor::One,
                operation: wgpu::BlendOperation::Add,
            }
        };
        FixedPipeline {
            blend: Some(wgpu::BlendState {
                color: wgpu::BlendComponent {
                    src_factor,
                    dst_factor,
                    operation,
                },
                alpha,
            }),
            fixed_opaque: 0,
        }
    }

    /// The fog rule of this blend in lit passes.
    pub(crate) fn fog_rule(self) -> FogRule {
        match (self.op, self.src, self.dst) {
            // A black source leaves the destination unchanged.
            (BlendOp::Max, _, _)
            | (BlendOp::Add | BlendOp::ReverseSubtract, _, BlendFactor::One) => FogRule::FadeOut,
            // A white source leaves the destination unchanged.
            (BlendOp::Min, _, _)
            | (BlendOp::Add, BlendFactor::Zero, BlendFactor::SrcColor)
            | (BlendOp::Add, BlendFactor::DstColor, BlendFactor::Zero) => FogRule::TowardWhite,
            _ => FogRule::Lerp,
        }
    }
}

impl AlphaMode {
    /// Blend by the output alpha **with** depth write and no alpha test: the lit
    /// `ColorAndOpacity` surface with alpha test off (`material.md` §11.3, translucent by
    /// its texture alpha), whose base pass writes depth (`lighting.md` §2.1). It is
    /// [`AlphaMode::TestBlend`] with a negative cutoff, so it is sorted with the blended
    /// draws; outside lit passes it draws opaque (a mask that keeps everything).
    pub const BLEND_DEPTH_WRITE: Self = Self::TestBlend { cutoff: -1.0 };
}

/// Where a [`Dye`]'s mask comes from.
///
/// Mask convention (`docs/specs/render.md` §2.3): single-channel masks are
/// read from the **red** channel. A mask decoded from an alpha-only source
/// (e.g. an A8 surface, which `tessera-formats` expands to
/// `(255, 255, 255, a)`) must be swizzled so the value is in red (or in all
/// four channels) before upload; uploaded as decoded it would dye the
/// whole surface. A mask that *is* a base texture's alpha uses
/// [`DyeMask::BaseAlpha`].
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum DyeMask {
    /// The red channel of a separate mask texture (sampled with the mesh
    /// UVs; upload it with [`crate::ColorSpace::Linear`]).
    Texture(TextureId),
    /// The base texture's alpha channel. That alpha then only masks the dye;
    /// coverage alpha comes from the base colour factor and vertex colour. In lit
    /// passes it is never an opacity either: [`Opacity::BaseAlpha`] acts as
    /// [`Opacity::One`] for such a draw (`material.md` §11.2), in the alpha test, the
    /// output alpha and the shadow pass.
    BaseAlpha,
}

/// Dye: `colour = base * mix(1, dye_color, mask)`.
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct Dye {
    /// Linear RGB dye colour.
    pub color: [f32; 3],
    /// Mask source (1 = fully dyed).
    pub mask: DyeMask,
}

/// Blinn-Phong specular highlight.
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct Specular {
    /// Linear RGB highlight colour (scaled by the light colour).
    pub color: [f32; 3],
    /// Blinn-Phong exponent (clamped to `1..=1024` outside lit passes). Lit passes use
    /// it as the specular power *p* of `lighting.md` §5.8: below 0.5 there is no
    /// highlight, above 4096 it is capped (see [`LitSurface`]).
    pub shininess: f32,
    /// Optional mask: its RGB multiplies `color` (upload linear). A
    /// single-channel mask must carry its value in R, G and B; an
    /// alpha-only source (A8 decodes to `(255, 255, 255, a)`) has to be
    /// replicated into RGB first, or the mask is fully on everywhere.
    pub texture: Option<TextureId>,
}

/// Surface description of a mesh draw.
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct Material {
    /// Linear RGBA factor multiplied with the base texture and vertex colour.
    pub base_color: Rgba,
    /// Base colour texture (`None`: white).
    pub base_texture: Option<TextureId>,
    /// Alpha handling.
    pub alpha: AlphaMode,
    /// Draw back faces too. Outside lit passes their normals are flipped for lighting;
    /// lit passes light a back face with the front normal (`material.md` §11.4, see
    /// [`LitSurface`]).
    pub two_sided: bool,
    /// Tangent-space normal map (RGB = XYZ * 0.5 + 0.5, upload linear).
    /// Only used when the mesh has tangents. Red points along the tangent
    /// (increasing `u`); green along increasing `v`, i.e. **down** the
    /// image since `v = 0` is the top row (the "Y-down" convention).
    pub normal_texture: Option<TextureId>,
    /// The normal map's green channel points **up** the image (along
    /// decreasing `v`, the "Y-up" convention) instead.
    ///
    /// Only the M2 pipelines read this flag. Lit passes always use the convention of
    /// `lighting.md` §5.2.4 (*N* = *n*.x · **T** − *n*.y · **B** with **B** = +V: green
    /// points up the image), which is the game data's.
    pub normal_green_up: bool,
    /// Dye tint.
    pub dye: Option<Dye>,
    /// Specular highlight.
    pub specular: Option<Specular>,
    /// Skip lighting: output the base colour as is.
    pub unlit: bool,
}

impl Default for Material {
    fn default() -> Self {
        Self {
            base_color: [1.0; 4],
            base_texture: None,
            alpha: AlphaMode::Opaque,
            two_sided: false,
            normal_texture: None,
            normal_green_up: false,
            dye: None,
            specular: None,
            unlit: false,
        }
    }
}

impl Material {
    /// Opaque material with a constant colour.
    #[must_use]
    pub fn color(base_color: Rgba) -> Self {
        Self {
            base_color,
            ..Self::default()
        }
    }

    /// Opaque material with a base texture.
    #[must_use]
    pub fn textured(texture: TextureId) -> Self {
        Self {
            base_texture: Some(texture),
            ..Self::default()
        }
    }
}

/// Directional light plus ambient term of a world pass.
///
/// The defaults are Tessera's own neutral studio lighting, not values from
/// any other client.
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct Lighting {
    /// Direction **towards** the light (normalised by the renderer; a zero
    /// or non-finite vector falls back to the default).
    pub direction: Vec3,
    /// Linear RGB light colour (intensity included).
    pub color: [f32; 3],
    /// Linear RGB ambient light.
    pub ambient: [f32; 3],
}

impl Default for Lighting {
    fn default() -> Self {
        Self {
            // Up-ish on both the Y-up and Z-up conventions, from the front-right.
            direction: Vec3::new(0.45, 0.6, 0.66),
            color: [0.85, 0.85, 0.82],
            ambient: [0.22, 0.23, 0.26],
        }
    }
}

impl Lighting {
    /// Normalised direction towards the light.
    #[must_use]
    pub fn direction_normalized(&self) -> Vec3 {
        let d = self.direction;
        let m = d.abs().max_element();
        if d.is_finite() && m.is_normal() {
            (d / m).try_normalize()
        } else {
            None
        }
        .unwrap_or_else(|| Self::default().direction.normalize())
    }
}

/// How mesh surfaces are coloured.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub enum MeshShading {
    /// The material, lit.
    #[default]
    Lit,
    /// The material's base colour (texture × factor × vertex colour × dye),
    /// unlit.
    BaseColor,
    /// World-space normal (after normal mapping) as `n * 0.5 + 0.5`.
    Normals,
    /// A procedural checkerboard (8 × 8 cells per UV unit, tinted by the
    /// UV value), lightly lit, to show UV layout and stretching.
    UvChecker,
    /// Surfaces are not drawn (for wireframe-only views).
    Hidden,
}

/// Vertex-normal line overlay.
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct NormalLines {
    /// Line length in world units.
    pub length: f32,
    /// Linear RGBA colour.
    pub color: Rgba,
}

/// Debug view of the meshes in a world pass.
///
/// Debug shadings other than [`MeshShading::Lit`] draw blended materials as
/// opaque; alpha-tested materials keep their cut-outs.
#[derive(Debug, Clone, Copy, PartialEq, Default)]
pub struct MeshDebug {
    /// Surface colouring.
    pub shading: MeshShading,
    /// Draw triangle edges in this colour (needs
    /// [`crate::MeshConfig::wireframe_edges`]).
    pub wireframe: Option<Rgba>,
    /// Draw a line along each vertex normal. Lines cover every vertex of
    /// the mesh, also for draws restricted to one submesh.
    pub normals: Option<NormalLines>,
}

/// Where the gloss *g* of a lit surface comes from (`lighting.md` §5.2 item 5,
/// `material.md` §10.2).
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub enum Gloss {
    /// *g* = 1.
    #[default]
    One,
    /// The base texture's alpha (`ColorAndGloss`).
    BaseAlpha,
    /// The normal map's alpha (`TSNormalAndGloss`).
    NormalAlpha,
    /// The alpha of a separate texture (`GlossAlpha`).
    Texture(TextureId),
}

/// Where the opacity *α* of a lit surface comes from (`material.md` §11.2). It drives the
/// alpha test (also in the shadow pass). Only [`Opacity::BaseAlpha`] also reaches the output
/// alpha (*α* × *M*.alpha); with the other sources the output alpha is *M*.alpha
/// (`lighting.md` §5.12, `material.md` §11.3).
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub enum Opacity {
    /// The base texture's alpha (`ColorAndOpacity`). Acts as [`Opacity::One`] when the
    /// draw's dye is masked by the base alpha ([`DyeMask::BaseAlpha`]).
    #[default]
    BaseAlpha,
    /// *α* = 1 (the base alpha is gloss, a dye mask or unused).
    One,
    /// The emissive texture's alpha (`EmissiveAndOpacity`).
    EmissiveAlpha,
    /// The alpha of a separate texture (`OpacityAlpha`).
    Texture(TextureId),
}

/// Which lights of the pass's light table ([`crate::FrameLighting::lights`]) a draw
/// receives.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub enum LightSelection {
    /// The renderer picks them (`lighting.md` §8.5, `render.md` §4.3): the point lights
    /// whose squared distance to the draw's world box is below the square of their
    /// selection radius ρ ([`crate::LightReach`]), without lights baked into the landscape
    /// on terrain chunks and with the parent switches applied; key lights first, each group
    /// nearest first (Tessera Decision of §8.5); then one directional light; at most
    /// [`crate::MAX_DRAW_LIGHTS`] in all with light 0 counted: 7 from the table (light 0 is
    /// the sun or, without one, the default directional light), 8 when a key directional
    /// table light takes light 0 in a pass without a sun.
    #[default]
    Auto,
    /// No light from the table (the sun and ambient still apply).
    None,
    /// These table indices (`count` ≤ 8 used; indices past the table are skipped; at most
    /// 7 kept, or 8 when the pass has no sun and the list holds a key directional light,
    /// `lighting.md` §8.5).
    List {
        /// Light-table indices.
        indices: [u8; 8],
        /// Number of valid entries.
        count: u8,
    },
}

/// The inputs of a mesh draw that only lit passes use (`lighting.md` §4.1, §5). Plain
/// data like [`Material`]; ignored outside lit passes.
///
/// In a lit pass the [`Material`] supplies the rest: `base_color` is *M*,
/// `base_texture` the albedo *C*, `alpha` the alpha test and blend state, `two_sided` the
/// culling (back faces keep the front normal there, `material.md` §11.4),
/// `normal_texture` the normal map (green convention of `lighting.md` §5.2.4;
/// `normal_green_up` is ignored), `dye` the dye, `specular` the
/// specular colour *Ms* and power *p* (its `texture` is not used: gloss comes from
/// [`LitSurface::gloss`]), and `unlit` selects the unlit pass.
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct LitSurface {
    /// Glow and emissive tint *G* (material parameter 2014).
    pub glow: [f32; 3],
    /// Emissive texture *Em* (slot 3018); adds *Em*.rgb × *G* (§5.9).
    pub emissive: Option<TextureId>,
    /// Glow mask (slot 3013); Tessera Decision: adds mask.rgb × *G* like emissive (the
    /// original's bloom composition is OPEN, §5.9).
    pub glow_mask: Option<TextureId>,
    /// Gloss source.
    pub gloss: Gloss,
    /// Opacity source.
    pub opacity: Opacity,
    /// Multiply *M* by the vertex colour (only records whose programs take a colour input,
    /// §5.13).
    pub vertex_colors: bool,
    /// Apply distance fog to an unlit surface ([`Material::unlit`]; `lighting.md` §7 code
    /// 0x00, §12: fog when its programs read the fog inputs). A lit surface is always
    /// fogged (`lighting.md` §2.1: the base pass always contains distance fog), whatever
    /// this says.
    pub fog: bool,
    /// Cast a sun shadow (opaque and alpha-tested lit surfaces only).
    pub cast_shadow: bool,
    /// Lights from the pass's table.
    pub lights: LightSelection,
}

impl Default for LitSurface {
    fn default() -> Self {
        Self {
            glow: [0.0; 3],
            emissive: None,
            glow_mask: None,
            gloss: Gloss::One,
            opacity: Opacity::BaseAlpha,
            vertex_colors: false,
            fog: true,
            cast_shadow: true,
            lights: LightSelection::Auto,
        }
    }
}

/// One mesh draw in a world pass.
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct MeshDraw {
    /// The mesh.
    pub mesh: MeshId,
    /// Which submesh to draw; `None` draws every submesh with this material.
    pub submesh: Option<u32>,
    /// Local → world transform.
    pub transform: Mat4,
    /// Surface.
    pub material: Material,
    /// Skinning palette of this frame ([`crate::Frame::skin_palette`]).
    /// Used only for meshes uploaded with skin streams
    /// ([`crate::Renderer::upload_skinned_mesh`]); `None` draws a skinned
    /// mesh in its bind pose. The palette maps the mesh's bind-pose model
    /// space to posed model space; `transform` is applied after it.
    pub skin: Option<SkinPalette>,
    /// Inputs of the lit pipeline (used in passes with
    /// [`crate::Pass::frame_lighting`]).
    pub lit: LitSurface,
    /// The caller's handle of the object this draw belongs to, matched against
    /// [`crate::LightReach::parent`] by the automatic light selection (`lighting.md` §8.5
    /// step 3). `None`: the draw is no light's parent.
    pub light_object: Option<u64>,
}

impl MeshDraw {
    /// Draw all submeshes of `mesh` at `transform` with `material`.
    #[must_use]
    pub fn new(mesh: MeshId, transform: Mat4, material: Material) -> Self {
        Self {
            mesh,
            submesh: None,
            transform,
            material,
            skin: None,
            lit: LitSurface::default(),
            light_object: None,
        }
    }

    /// Mark the draw as part of the object with handle `object` (see
    /// [`Self::light_object`]).
    #[must_use]
    pub fn with_light_object(mut self, object: u64) -> Self {
        self.light_object = Some(object);
        self
    }

    /// Set the lit-pipeline inputs (see [`Self::lit`]).
    #[must_use]
    pub fn with_lit(mut self, lit: LitSurface) -> Self {
        self.lit = lit;
        self
    }

    /// Skin the draw with `palette` (see [`Self::skin`]). A palette that
    /// belongs to another frame, or that has fewer matrices than the
    /// mesh's [`crate::SkinInfo::required_palette_len`], is ignored with a
    /// warning and the mesh is drawn in its bind pose.
    #[must_use]
    pub fn with_skin(mut self, palette: SkinPalette) -> Self {
        self.skin = Some(palette);
        self
    }

    /// Restrict the draw to one submesh.
    #[must_use]
    pub fn with_submesh(mut self, submesh: u32) -> Self {
        self.submesh = Some(submesh);
        self
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    /// `AlphaMode::Additive` keeps the blend state it always had (SRCALPHA / ONE, the
    /// target alpha kept); min and max ignore the factors.
    #[test]
    fn fixed_blend_states() {
        let add = FixedBlend::ADDITIVE
            .pipeline(FixedAlpha::Keep)
            .blend
            .expect("blends");
        assert_eq!(add.color.src_factor, wgpu::BlendFactor::SrcAlpha);
        assert_eq!(add.color.dst_factor, wgpu::BlendFactor::One);
        assert_eq!(add.color.operation, wgpu::BlendOperation::Add);
        assert_eq!(add.alpha.src_factor, wgpu::BlendFactor::Zero);
        assert_eq!(add.alpha.dst_factor, wgpu::BlendFactor::One);
        let max = FixedBlend {
            src: BlendFactor::DstColor,
            dst: BlendFactor::Zero,
            op: BlendOp::Max,
            depth_write: true,
        };
        let s = max.pipeline(FixedAlpha::Keep).blend.expect("blends");
        assert_eq!(s.color.src_factor, wgpu::BlendFactor::One);
        assert_eq!(s.color.dst_factor, wgpu::BlendFactor::One);
        assert_eq!(s.color.operation, wgpu::BlendOperation::Max);
    }

    /// The fog rule fades a blended contribution toward what leaves the destination
    /// unchanged.
    #[test]
    fn fixed_blend_fog_rules() {
        let f = |src, dst, op| {
            FixedBlend {
                src,
                dst,
                op,
                depth_write: false,
            }
            .fog_rule()
        };
        use BlendFactor as F;
        assert_eq!(FixedBlend::ADDITIVE.fog_rule(), FogRule::FadeOut);
        assert_eq!(f(F::One, F::One, BlendOp::Add), FogRule::FadeOut);
        assert_eq!(f(F::DstColor, F::One, BlendOp::Add), FogRule::FadeOut);
        assert_eq!(f(F::Zero, F::SrcColor, BlendOp::Add), FogRule::TowardWhite);
        assert_eq!(f(F::DstColor, F::Zero, BlendOp::Add), FogRule::TowardWhite);
        assert_eq!(f(F::One, F::One, BlendOp::Min), FogRule::TowardWhite);
        assert_eq!(
            f(F::SrcAlpha, F::OneMinusSrcAlpha, BlendOp::Add),
            FogRule::Lerp
        );
        assert_eq!(f(F::One, F::Zero, BlendOp::Add), FogRule::Lerp);
    }

    #[test]
    fn light_direction_is_sanitised() {
        let d = Lighting {
            direction: Vec3::new(f32::NAN, 0.0, 0.0),
            ..Lighting::default()
        };
        assert!((d.direction_normalized().length() - 1.0).abs() < 1e-6);
        let z = Lighting {
            direction: Vec3::ZERO,
            ..Lighting::default()
        };
        assert!(z.direction_normalized().is_finite());
        let big = Lighting {
            direction: Vec3::splat(f32::MAX),
            ..Lighting::default()
        };
        assert!((big.direction_normalized() - Vec3::splat(1.0 / 3f32.sqrt())).length() < 1e-6);
    }
}
