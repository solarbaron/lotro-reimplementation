// SPDX-License-Identifier: GPL-3.0-or-later
//! The water pipeline (M4.5, `docs/specs/water.md` §8–§9; `docs/specs/render.md` §5.3).
//!
//! [`Pass::water`] draws landscape water as horizontal quads at the water height, one per
//! wet landblock (`water.md` §8.1 Tessera rule) or per distant landblock of a 3 × 3
//! sector (§9: the same (type, height) pairs, derived by the caller from the block maps),
//! each optionally subdivided into a grid for vertex waves. The geometry is generated in
//! the vertex shader from one instance per quad, so thousands of quads cost one draw.
//!
//! Shading follows the water permutations of shader record 0x2B0000A8 (§8.3–§8.4): three
//! scrolling normal-map layers, optional waves, a refraction of the scene drawn so far
//! (a copy of the colour target made just before the pass's translucent stage), the
//! refraction standing in for the reflection, Fresnel, specular with the sparkle term, the
//! water colour and overbright, then distance fog. Output alpha 1, written with source ONE
//! and destination ZERO, depth written, both faces drawn (§8.4 Tessera rule).
//!
//! The same pipeline draws the other water program families of `water.md` §8.2 and the
//! records with programs of their own (`material.md` §11.6), on landscape quads and on
//! placed meshes ([`Pass::liquid_mesh`]): plain water on mesh UV and older bump water
//! (§8.5.6), flow-map water (§8.6: the flow normal with layers 1 and 2, the colour of the
//! Tessera rule of §8.6.4), lava (§8.6.5: emissive, unlit, opaque), the mirror record of
//! puddles (§8.5.4, with the Tessera rule's fog-coloured reflection) and the
//! liquid-distortion records (§8.5.5). The opaque families write with ONE / ZERO like the
//! plain water; the mirror and liquid draws blend by source alpha and are drawn last, back
//! to front. Every time-dependent value reads the frame's time input *T* (`water.md` §8.7);
//! curve-valued parameters are evaluated by the caller at the clock each frame (§8.7.1).
//!
//! A draw may carry the far view's far-draw fade (`far-view.md` §7.2, FD1; the far view
//! applies it to the sector water as a Tessera choice, `far-view.md` §8 OPEN): the fragment
//! is blended toward the undistorted scene copy by *a* = 1 − sat((*d* − *s*) × *r*), *d* the
//! distance to the eye, and discarded where *a* < 0.001 (without a scene copy: where
//! *a* < 0.5).

use std::collections::HashMap;
use std::fmt;
use std::ops::Range;
use std::sync::Arc;

use bytemuck::{Pod, Zeroable};
use glam::Mat4;
use wgpu::util::DeviceExt;

use crate::arena::MeshId;
use crate::depth_bias::{BiasKey, DrawDepthBias};
use crate::error::{RenderError, Result};
use crate::far_view::FarFadeParams;
use crate::frame::Pass;
use crate::instanced::{
    light_words, lit_source, max_uniform_bytes, mesh_vertex_layout, sampler_entry, texture_entry,
    uniform_entry, uniform_stride, world_depth,
};
use crate::lighting::MAX_DRAW_LIGHTS;
use crate::lit_pipelines::LightTarget;
use crate::material::{LightSelection, UvTransform};
use crate::mesh_pipelines::is_mirrored;
use crate::pass_hook::{HookRecord, HookStage, PassHook};
use crate::renderer::{FrameStats, ResolvedTexture, WorldFrame};
use crate::texture::TextureId;

const WATER_WGSL: &str = include_str!("shaders/water.wgsl");

/// Landblock edge in world units (`water.md` §8.1).
pub const WATER_BLOCK: f32 = 160.0;

/// Largest grid subdivision of one quad (per side).
pub const MAX_WATER_SUBDIVISIONS: u32 = 64;

/// Largest number of water quads per pass and frame.
pub const MAX_WATER_QUADS: usize = 1 << 16;

/// Largest number of vertices the water of one pass generates (all draws together). Grids
/// are coarsened to stay below; one cell per quad always fits (6 × [`MAX_WATER_QUADS`]).
pub const MAX_WATER_VERTICES: u64 = 1 << 24;

/// Largest number of placed liquid draws ([`Pass::liquid_mesh`]) per pass and frame; draws
/// beyond it are dropped (logged), not an error.
pub const MAX_LIQUID_MESH_DRAWS: usize = 1 << 14;

/// The program family a water draw follows (`water.md` §8.2 "Program families").
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Default)]
pub enum WaterFamily {
    /// Plain water (§8.3–§8.4): three normal layers at *B* × `Scale`ₖ + `Speed`ₖ × *T*.
    #[default]
    Plain,
    /// Plain water on mesh UV (§8.5.6): the layers at UV × `Scale`ₖ + `Speed`ₖ × *T*.
    PlainUv,
    /// Older bump water (§8.5.6): layers *U* + (*b*, *b*), 2*U* + 4(*b*, *b*), 4*U* + 8(*b*,
    /// *b*) with *U* = UV × [`WaterParams::bump`]`[0]`, *b* = `bump[1]` × *T*; the pixel stage
    /// as §8.4 (Tessera rule).
    OlderBump,
    /// Flow-map water (§8.6): [`WaterParams::normal_speed`]`[0]` holds the phase rates 1057
    /// and 1058, `normal_scale[0]` the scale 1063; with [`WaterParams::normal_layers`] 3 the
    /// layers 1 and 2 of §8.6.2 step 7 are added.
    Flow,
    /// Lava (§8.6.5): the phase rates and scale as for [`WaterFamily::Flow`],
    /// [`WaterParams::overbright`] the brightness 1066; the colour layers, emissive, flow,
    /// foam and noise textures of [`WaterTextures`]; no light, opaque.
    Lava,
}

/// The flow, foam and depth parameters of the flow-map and lava families (`water.md`
/// §8.6.1, §8.6.5).
#[derive(Debug, Clone, Copy, PartialEq, Default)]
pub struct WaterFlowParams {
    /// 1182, 1183: foam lookup offset.
    pub foam_offset: [f32; 2],
    /// 1184, 1185: foam tiling.
    pub foam_tile: [f32; 2],
    /// 1186, 1187: offset of the flow map's third-channel lookup.
    pub depth_offset: [f32; 2],
    /// 1188: rotation of the phase coordinates (radians).
    pub rotation: f32,
}

/// The Fresnel form of a flow-map draw, and with it the record's own colour list
/// (`water.md` §8.6.4 Tessera rule, revised 2026-10-10: each flow-map record follows its own
/// OpenGL reflection-permutation list with the reflection replaced by the untinted
/// refraction sample; W42, W43).
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Default)]
pub enum FresnelForm {
    /// `water.md` §8.4 step 5: *f* = bias + (1 − bias) × (1 − |*v* · *n*|)^exponent. The
    /// plain families, and the flow-map record without layers 1 and 2 (0x2B0005EC), whose
    /// list (§8.6.4 steps 1–5 of its own list) tints toward `WaterFogColor` itself and has no
    /// fake-light diffuse term *L*꜀ anywhere (W42).
    #[default]
    Biased,
    /// `water.md` §8.6.4 step 4 (0x2B0007FD): *f* = bias × (1 − |*v* · *n*|)^(1 / exponent),
    /// the bias multiplies and the exponent is inverted; its list tints toward *L*꜀² ×
    /// `WaterFogColor` and multiplies the specular by *L*꜀ (steps 1–6 of the first list).
    /// Read only by [`WaterFamily::Flow`]; the other families always use
    /// [`FresnelForm::Biased`].
    Scaled,
}

/// The water parameters of a surface (`water.md` §8.2: the `c_Water…` material
/// parameters the 0x2B0000A8 programs read). [`Default`] gives that record's shader
/// defaults; a surface's materials override them. Colours are stored-value RGB.
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct WaterParams {
    /// The program family.
    pub family: WaterFamily,
    /// 1044 `c_WaterWaveSpeed`.
    pub wave_speed: f32,
    /// 1045 `c_WaterWaveFrequency`.
    pub wave_frequency: f32,
    /// 1046 `c_WaterWaveAmplitude` (0: no waves).
    pub wave_amplitude: f32,
    /// 1057–1062: (U, V) scroll speed of normal layers 0, 1, 2 (units per second). Flow map
    /// and lava: layer 0 holds the phase rates (cycles per second).
    pub normal_speed: [[f32; 2]; 3],
    /// 1063–1065: tiling of normal layers 0, 1, 2 (repeats per landblock).
    pub normal_scale: [f32; 3],
    /// How many normal layers are used (1–3). Flow map: 3 adds layers 1 and 2 (§8.6.2
    /// step 7), 1 stops after step 6.
    pub normal_layers: u32,
    /// 1047 `c_WaterReflectionBumpDistortionAmount`.
    pub reflection_distortion: f32,
    /// 1048 `c_WaterRefractionBumpDistortionAmount`.
    pub refraction_distortion: f32,
    /// 1050 `c_WaterFresnelBias`.
    pub fresnel_bias: f32,
    /// 1051 `c_WaterFresnelExponent`.
    pub fresnel_exponent: f32,
    /// How 1050 and 1051 combine ([`FresnelForm`]; flow-map draws only).
    pub fresnel_form: FresnelForm,
    /// 1054 `c_WaterSpecularPower`.
    pub specular_power: f32,
    /// 1066 `c_WaterOverbrightAmount` (lava: the brightness).
    pub overbright: f32,
    /// 1067 `c_WaterFakeSpecularThreshold` (sparkle threshold).
    pub fake_specular_threshold: f32,
    /// 1068 `c_WaterFogAmount` (under-water tint of the refraction).
    pub fog_amount: f32,
    /// 2006 `c_WaterSpecularColor`.
    pub specular_color: [f32; 3],
    /// 2009 `c_WaterColor`.
    pub water_color: [f32; 3],
    /// 2010 `c_WaterFogColor`.
    pub fog_color: [f32; 3],
    /// 2011 `c_WaterShorelineColor`.
    pub shoreline_color: [f32; 3],
    /// Flow-map and lava parameters.
    pub flow: WaterFlowParams,
    /// Older bump water: 1055 (texture scale) and 1056 (speed).
    pub bump: [f32; 2],
}

impl Default for WaterParams {
    /// The shader defaults of 0x2B0000A8 (`water.md` §8.2 table; decision 0012).
    fn default() -> Self {
        Self {
            family: WaterFamily::Plain,
            wave_speed: 1.0,
            wave_frequency: 0.2,
            wave_amplitude: 0.0,
            normal_speed: [[0.005, 0.0075], [0.015, 0.02], [-0.01, -0.0125]],
            normal_scale: [2.0, 4.0, 8.0],
            normal_layers: 3,
            reflection_distortion: 0.1,
            refraction_distortion: 0.04,
            fresnel_bias: 0.1,
            fresnel_exponent: 2.0,
            fresnel_form: FresnelForm::Biased,
            specular_power: 80.0,
            overbright: 1.0,
            fake_specular_threshold: 0.25,
            fog_amount: 1.0,
            specular_color: [1.0; 3],
            water_color: [1.0; 3],
            fog_color: [0.0; 3],
            shoreline_color: [1.0; 3],
            flow: WaterFlowParams::default(),
            bump: [1.0, 0.0],
        }
    }
}

/// The textures of a water draw besides its normal map (`water.md` §8.6.1, §8.6.5). An
/// absent or unknown texture is left out (the drawing treats its input as neutral: no flow,
/// no foam, phase offset 0, white colour layers, no emissive).
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub struct WaterTextures {
    /// Slot 3075: the flow map (sampled with clamped addresses).
    pub flow: Option<TextureId>,
    /// Slot 3079: the foam map.
    pub foam: Option<TextureId>,
    /// The noise texture of the phase offset.
    pub noise: Option<TextureId>,
    /// Slots 3000 and 3001: colour layers (lava), the diffuse image (mirror, liquid).
    pub color: [Option<TextureId>; 2],
    /// Slot 3018: the emissive layer (lava).
    pub emissive: Option<TextureId>,
}

/// One horizontal water quad (`water.md` §8.1): the rectangle from `origin` to
/// `origin + size` (world x east, y north) at height `height`.
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct WaterQuad {
    /// South-west corner (world x, y).
    pub origin: [f32; 2],
    /// Extent along x and y (positive).
    pub size: [f32; 2],
    /// Water height *h* (world z).
    pub height: f32,
    /// The mesh UV of the quad (`water.md` §8.1): (u, v) at the south-west corner and the
    /// change (du, dv) to the north-east corner. [`WaterQuad::landblock`] gives one repeat
    /// per quad, `(0, 1, 1, −1)`; [`WaterQuad::with_sector_uv`] the sector coordinates.
    pub uv: [f32; 4],
}

impl WaterQuad {
    /// The 160 × 160 quad of a wet landblock whose south-west corner is at `origin`
    /// (`water.md` §8.1 Tessera rule).
    #[must_use]
    pub fn landblock(origin: [f32; 2], height: f32) -> Self {
        Self {
            origin,
            size: [WATER_BLOCK; 2],
            height,
            uv: [0.0, 1.0, 1.0, -1.0],
        }
    }

    /// The quad with the texture coordinates of `water.md` §8.1: (*i* / 3, 1 − *j* / 3) over
    /// its 3 × 3 sector, `ix`, `iy` (0–2) the landblock's place in the sector.
    #[must_use]
    pub fn with_sector_uv(mut self, ix: u32, iy: u32) -> Self {
        let (i, j) = ((ix % 3) as f32, (iy % 3) as f32);
        self.uv = [i / 3.0, 1.0 - j / 3.0, 1.0 / 3.0, -1.0 / 3.0];
        self
    }

    fn sane(&self) -> Option<Self> {
        let ok = self
            .origin
            .iter()
            .chain(&self.size)
            .chain(&self.uv)
            .all(|v| v.is_finite())
            && self.height.is_finite()
            && self.size.iter().all(|&s| s > 0.0);
        ok.then_some(*self)
    }
}

/// Water of one surface drawn in a pass ([`Pass::water`]).
#[derive(Debug, Clone)]
pub struct WaterDraw {
    /// The quads (shared, so a list can be queued every frame for free).
    pub quads: Arc<[WaterQuad]>,
    /// The surface's normal map (3011). `None`: a flat surface. A map uploaded
    /// [`crate::ColorSpace::Srgb`] is re-encoded after sampling (`docs/specs/render.md`
    /// §5.1), so either upload gives the stored texels.
    pub normal_map: Option<TextureId>,
    /// The surface's water parameters.
    pub params: WaterParams,
    /// Grid cells per quad side (1–[`MAX_WATER_SUBDIVISIONS`]); more than 1 only matters
    /// with waves. Halved until the pass's water fits in [`MAX_WATER_VERTICES`] (draws in
    /// queue order, each leaving one cell per quad for the draws after it).
    pub subdivisions: u32,
    /// The horizontal offset *O* of `water.md` §8.3 (`c_ContiguousWorldBlockOffset`):
    /// normal-layer coordinates are (position − *O*) / 160. Tessera rule: (0, 0) in the
    /// region frame.
    pub block_offset: [f32; 2],
    /// The far-draw fade (`far-view.md` §7.2; module docs). Default: no fade.
    pub fade: FarFadeParams,
    /// The flow-map, foam, noise, colour and emissive textures (flow-map and lava families).
    pub textures: WaterTextures,
}

impl WaterDraw {
    /// Water of `quads` with `params`, one cell per quad, *O* = (0, 0).
    #[must_use]
    pub fn new(
        quads: impl Into<Arc<[WaterQuad]>>,
        normal_map: Option<TextureId>,
        params: WaterParams,
    ) -> Self {
        Self {
            quads: quads.into(),
            normal_map,
            params,
            subdivisions: 1,
            block_offset: [0.0; 2],
            fade: FarFadeParams::default(),
            textures: WaterTextures::default(),
        }
    }
}

/// The parameters of the mirror record (`water.md` §8.5.4), evaluated by the caller at the
/// clock.
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct MirrorParams {
    /// 1100: reflection offset by the normal.
    pub reflection_distortion: f32,
    /// 1101: Fresnel exponent.
    pub fresnel_exponent: f32,
    /// 1102: Fresnel bias.
    pub fresnel_bias: f32,
    /// 1103: normal-map tiling.
    pub normal_scale: f32,
    /// 1104, 1105: the evaluated speeds *S*; the drawing multiplies them by *T* again.
    pub normal_speed: [f32; 2],
    /// 1106: Fresnel bump amount.
    pub fresnel_bump: f32,
    /// The material colour 2000 (stored-value RGBA).
    pub color: [f32; 4],
}

impl Default for MirrorParams {
    fn default() -> Self {
        Self {
            reflection_distortion: 0.0,
            fresnel_exponent: 1.0,
            fresnel_bias: 0.0,
            normal_scale: 1.0,
            normal_speed: [0.0; 2],
            fresnel_bump: 0.0,
            color: [1.0; 4],
        }
    }
}

/// The parameters of a liquid-distortion record (`water.md` §8.5.5), evaluated by the
/// caller at the clock.
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct DistortionParams {
    /// Lit like a lit `ColorAndOpacity` surface (else unlit, × 1066).
    pub lit: bool,
    /// The layers read *B* instead of the mesh UV through texture matrix 0.
    pub world_coordinates: bool,
    /// 1044–1046: vertex waves (speed, frequency, amplitude), unlit records only.
    pub waves: [f32; 3],
    /// 1048: distortion amount.
    pub distortion: f32,
    /// 1057–1062: scroll speed (U, V) of layers 0, 1, 2.
    pub speed: [[f32; 2]; 3],
    /// 1063–1065: tiling of layers 0, 1, 2.
    pub scale: [f32; 3],
    /// 1066: colour scale of the unlit records.
    pub overbright: f32,
    /// Texture matrix 0 (`material.md` §8.4).
    pub matrix: UvTransform,
    /// The material colour 2000 (stored-value RGBA).
    pub color: [f32; 4],
    /// The alpha-test threshold, when on. The unlit records test their output alpha (texel
    /// alpha × the vertex alpha where [`DistortionParams::vertex_color`] × material alpha,
    /// `water.md` §8.5.5 step 5, W45); the lit record tests the texel alpha before any
    /// vertex multiply (`lighting.md` §5.12). A pixel at or below the threshold is
    /// discarded.
    pub alpha_test: Option<f32>,
    /// Both faces drawn.
    pub two_sided: bool,
    /// Depth write.
    pub depth_write: bool,
    /// The texel (colour and alpha) is multiplied by the vertex colour before the alpha
    /// test (`water.md` §8.5.5 step 5: 0x2B00063E; unlit records only).
    pub vertex_color: bool,
}

impl Default for DistortionParams {
    fn default() -> Self {
        Self {
            lit: false,
            world_coordinates: false,
            waves: [0.0; 3],
            distortion: 0.0,
            speed: [[0.0; 2]; 3],
            scale: [1.0; 3],
            overbright: 1.0,
            matrix: UvTransform::IDENTITY,
            color: [1.0; 4],
            alpha_test: None,
            two_sided: false,
            depth_write: false,
            vertex_color: false,
        }
    }
}

/// How a placed liquid surface is drawn ([`LiquidMeshDraw`]).
#[derive(Debug, Clone, Copy, PartialEq)]
pub enum LiquidShading {
    /// A water record (any [`WaterFamily`], lava included) at the mesh's own triangles;
    /// *B* comes from their world positions (`water.md` §8.5 rule item 1).
    Water {
        /// The surface's water parameters.
        params: WaterParams,
        /// The horizontal offset *O* (`water.md` §8.3).
        block_offset: [f32; 2],
    },
    /// The mirror record (`water.md` §8.5.4).
    Mirror(MirrorParams),
    /// A liquid-distortion record (`water.md` §8.5.5).
    Distortion(DistortionParams),
}

impl LiquidShading {
    fn blended(&self) -> bool {
        !matches!(self, Self::Water { .. })
    }
}

/// A placed mesh surface drawn by its own program ([`Pass::liquid_mesh`]; `material.md`
/// §11.6, `water.md` §8.5).
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct LiquidMeshDraw {
    /// The mesh ([`crate::Renderer::upload_mesh`]).
    pub mesh: MeshId,
    /// Which submesh; `None` draws all of them.
    pub submesh: Option<u32>,
    /// Local → world transform.
    pub transform: Mat4,
    /// The drawing.
    pub shading: LiquidShading,
    /// The normal map (3011), or the distortion texture (3026) of a liquid record.
    pub normal_map: Option<TextureId>,
    /// The other textures (the mirror's and liquid's diffuse in `color[0]`).
    pub textures: WaterTextures,
    /// The caller's object handle for the light selection of lit liquids.
    pub light_object: Option<u64>,
    /// The depth bias of the placed entity's `Render_DepthBias_Type` entry, as the
    /// depth-bias table stores it (`decals.md` §6.5); applied as the Tessera default of
    /// §6.5 OPEN (c), which covers every draw of the entity (`water.md` §8.5.4 rule for
    /// puddles, §14). See [`DrawDepthBias`] for the sign, the clamp and the bounds.
    pub depth_bias: DrawDepthBias,
}

impl Pass {
    /// Draw water (`docs/specs/render.md` §5.3). Needs a lit pass
    /// ([`Pass::frame_lighting`]); ignored by screen passes. The pass's colour target is
    /// copied before its translucent stage so the water refracts what was drawn before.
    pub fn water(&mut self, draw: WaterDraw) -> &mut Self {
        self.world.water.push(draw);
        self
    }

    /// Draw a placed surface of a water, mirror, liquid-distortion or lava record with its
    /// own program (`water.md` §8.5, `material.md` §11.6). Needs a lit pass; drawn in the
    /// translucent stage after the landscape water, the blended ones back to front.
    pub fn liquid_mesh(&mut self, draw: LiquidMeshDraw) -> &mut Self {
        self.world.liquid.push(draw);
        self
    }
}

/// Water uniform (must match `Water` in `water.wgsl`).
#[repr(C)]
#[derive(Debug, Clone, Copy, Pod, Zeroable)]
struct WaterUniform {
    /// Mesh draws: model → world.
    model: [[f32; 4]; 4],
    /// speed, frequency, amplitude, normal layer count.
    waves: [f32; 4],
    /// layer 0 (u, v), layer 1 (u, v) speeds.
    speed01: [f32; 4],
    /// layer 2 (u, v) speed, scales 0 and 1.
    speed2_scale01: [f32; 4],
    /// scale 2, reflection distortion, refraction distortion, Fresnel bias.
    misc0: [f32; 4],
    /// Fresnel exponent, specular power, overbright, fake specular threshold.
    misc1: [f32; 4],
    /// fog amount, has scene copy, normal map (0 none, 1 linear, 2 sRGB: re-encode),
    /// subdivisions.
    misc2: [f32; 4],
    specular_color: [f32; 4],
    water_color: [f32; 4],
    fog_color: [f32; 4],
    shoreline_color: [f32; 4],
    /// 1 / width, 1 / height, Oₓ, Oᵧ.
    screen: [f32; 4],
    /// Far-draw fade start *s*, inverse range *r* (0: no fade), unused ×2.
    fade: [f32; 4],
    /// family, textures present, textures sRGB, flags.
    kind: [u32; 4],
    flow0: [f32; 4],
    flow1: [f32; 4],
    extra0: [f32; 4],
    extra1: [f32; 4],
    material: [f32; 4],
    uv_row0: [f32; 4],
    uv_row1: [f32; 4],
    lights: [u32; 4],
}

/// Largest Fresnel exponent and specular power uploaded (the lit pipelines' cap); values
/// below 0 become 0 (`docs/specs/render.md` §5.3, implementer note).
const MAX_WATER_POWER: f32 = 4096.0;

/// Family codes (must match `FAM_*` in `water.wgsl`).
mod fam {
    pub const PLAIN: u32 = 0;
    pub const PLAIN_UV: u32 = 1;
    pub const BUMP: u32 = 2;
    pub const FLOW: u32 = 3;
    pub const LAVA: u32 = 4;
    pub const MIRROR: u32 = 5;
    pub const LIQUID: u32 = 6;
    pub const LIQUID_LIT: u32 = 7;
}

/// Texture bits (must match `T_*` in `water.wgsl`), in binding order.
const TEXTURE_BITS: [u32; 7] = [1, 2, 4, 8, 16, 32, 64];

/// Flag bits (must match `FL_*` in `water.wgsl`).
mod fl {
    pub const WORLD_UV: u32 = 1;
    pub const ALPHA_TEST: u32 = 2;
    /// Flow-map water with the colour list of 0x2B0007FD ([`super::FresnelForm::Scaled`]):
    /// its Fresnel form, the tint toward *L*꜀² × `WaterFogColor` and the specular × *L*꜀.
    pub const FLOW_FAKE_LIGHT: u32 = 4;
    /// The liquid's texel is multiplied by the vertex colour before the alpha test
    /// (0x2B00063E, `water.md` §8.5.5 step 5).
    pub const VERTEX_COLOR: u32 = 8;
}

/// Per-quad instance data (must match `QuadIn` in `water.wgsl`).
#[repr(C)]
#[derive(Debug, Clone, Copy, Pod, Zeroable)]
struct QuadGpu {
    /// origin.xy, size.xy
    rect: [f32; 4],
    /// height, unused ×3
    height: [f32; 4],
    /// u, v at the south-west corner, du, dv
    uv: [f32; 4],
}

const QUAD_ATTRS: [wgpu::VertexAttribute; 3] =
    wgpu::vertex_attr_array![0 => Float32x4, 1 => Float32x4, 2 => Float32x4];

fn fin(v: f32, d: f32) -> f32 {
    if v.is_finite() { v } else { d }
}

/// The textures of one draw, in binding order: normal, flow, foam, noise, colour 0,
/// colour 1, emissive.
type DrawTextures = [Option<TextureId>; 7];

fn draw_textures(normal: Option<TextureId>, t: &WaterTextures) -> DrawTextures {
    [
        normal, t.flow, t.foam, t.noise, t.color[0], t.color[1], t.emissive,
    ]
}

impl WaterUniform {
    /// The common part: water parameters (or the defaults), the scene copy, the textures
    /// that resolved (`resolved`: binding order) and the frame size.
    fn new(
        p: &WaterParams,
        scene: bool,
        resolved: &[Option<ResolvedTexture>; 7],
        subdivisions: u32,
        ctx: &WorldFrame<'_>,
        offset: [f32; 2],
        fade: FarFadeParams,
    ) -> Self {
        let f = fin;
        let d = WaterParams::default();
        let rgb = |c: [f32; 3], dc: [f32; 3]| [f(c[0], dc[0]), f(c[1], dc[1]), f(c[2], dc[2]), 1.0];
        let sp = |k: usize, i: usize| f(p.normal_speed[k][i], d.normal_speed[k][i]);
        let sc = |k: usize| f(p.normal_scale[k], d.normal_scale[k]);
        let normal = resolved[0].as_ref().map(|t| t.srgb);
        let mut present = 0;
        let mut srgb = 0;
        for (i, t) in resolved.iter().enumerate() {
            if let (Some(t), Some(bit)) = (t, TEXTURE_BITS.get(i)) {
                present |= bit;
                if t.srgb {
                    srgb |= bit;
                }
            }
        }
        let family = match p.family {
            WaterFamily::Plain => fam::PLAIN,
            WaterFamily::PlainUv => fam::PLAIN_UV,
            WaterFamily::OlderBump => fam::BUMP,
            WaterFamily::Flow => fam::FLOW,
            WaterFamily::Lava => fam::LAVA,
        };
        let flags = if p.family == WaterFamily::Flow && p.fresnel_form == FresnelForm::Scaled {
            self::fl::FLOW_FAKE_LIGHT
        } else {
            0
        };
        let fl = &p.flow;
        Self {
            model: Mat4::IDENTITY.to_cols_array_2d(),
            waves: [
                f(p.wave_speed, d.wave_speed),
                f(p.wave_frequency, d.wave_frequency),
                f(p.wave_amplitude, 0.0),
                p.normal_layers.clamp(1, 3) as f32,
            ],
            speed01: [sp(0, 0), sp(0, 1), sp(1, 0), sp(1, 1)],
            speed2_scale01: [sp(2, 0), sp(2, 1), sc(0), sc(1)],
            misc0: [
                sc(2),
                f(p.reflection_distortion, d.reflection_distortion),
                f(p.refraction_distortion, d.refraction_distortion),
                f(p.fresnel_bias, d.fresnel_bias),
            ],
            // Exponents in 0..=4096 (as the lit and terrain powers): a negative one would
            // raise 0 to a negative power on the GPU (inf, then NaN).
            misc1: [
                f(p.fresnel_exponent, d.fresnel_exponent).clamp(0.0, MAX_WATER_POWER),
                f(p.specular_power, d.specular_power).clamp(0.0, MAX_WATER_POWER),
                f(p.overbright, d.overbright),
                f(p.fake_specular_threshold, d.fake_specular_threshold),
            ],
            misc2: [
                f(p.fog_amount, d.fog_amount),
                if scene { 1.0 } else { 0.0 },
                match normal {
                    None => 0.0,
                    Some(false) => 1.0,
                    Some(true) => 2.0,
                },
                subdivisions as f32,
            ],
            specular_color: rgb(p.specular_color, d.specular_color),
            water_color: rgb(p.water_color, d.water_color),
            fog_color: rgb(p.fog_color, d.fog_color),
            shoreline_color: rgb(p.shoreline_color, d.shoreline_color),
            screen: [
                1.0 / ctx.size.0.max(1) as f32,
                1.0 / ctx.size.1.max(1) as f32,
                f(offset[0], 0.0),
                f(offset[1], 0.0),
            ],
            fade: [
                f(fade.start, f32::MAX),
                f(fade.inv_range, 0.0).max(0.0),
                0.0,
                0.0,
            ],
            kind: [family, present, srgb, flags],
            flow0: [
                f(fl.foam_offset[0], 0.0),
                f(fl.foam_offset[1], 0.0),
                f(fl.foam_tile[0], 0.0),
                f(fl.foam_tile[1], 0.0),
            ],
            flow1: [
                f(fl.depth_offset[0], 0.0),
                f(fl.depth_offset[1], 0.0),
                f(fl.rotation, 0.0),
                0.0,
            ],
            extra0: [f(p.bump[0], 1.0), f(p.bump[1], 0.0), 0.0, 0.0],
            extra1: [0.0; 4],
            material: [1.0; 4],
            uv_row0: [1.0, 0.0, 0.0, 0.0],
            uv_row1: [0.0, 1.0, 0.0, 0.0],
            lights: [0; 4],
        }
    }

    /// The uniform of a mirror draw (`water.md` §8.5.4).
    fn mirror(&mut self, m: &MirrorParams) {
        let f = fin;
        self.kind[0] = fam::MIRROR;
        self.extra0 = [
            f(m.reflection_distortion, 0.0),
            f(m.fresnel_exponent, 1.0).clamp(0.0, MAX_WATER_POWER),
            f(m.fresnel_bias, 0.0),
            f(m.normal_scale, 1.0),
        ];
        self.extra1 = [
            f(m.normal_speed[0], 0.0),
            f(m.normal_speed[1], 0.0),
            f(m.fresnel_bump, 0.0),
            0.0,
        ];
        self.material = m.color.map(|c| f(c, 1.0));
    }

    /// The uniform of a liquid-distortion draw (`water.md` §8.5.5) with its light list.
    fn distortion(&mut self, d: &DistortionParams, lights: [u32; 3]) {
        let f = fin;
        self.kind[0] = if d.lit { fam::LIQUID_LIT } else { fam::LIQUID };
        let mut flags = 0;
        if d.world_coordinates {
            flags |= fl::WORLD_UV;
        }
        if d.alpha_test.is_some() {
            flags |= fl::ALPHA_TEST;
        }
        if d.vertex_color {
            flags |= fl::VERTEX_COLOR;
        }
        self.kind[3] = flags;
        // Waves only for the unlit records (§8.5.5 step 2).
        let amplitude = if d.lit { 0.0 } else { f(d.waves[2], 0.0) };
        self.waves = [f(d.waves[0], 0.0), f(d.waves[1], 0.0), amplitude, 3.0];
        let sp = |k: usize, i: usize| f(d.speed[k][i], 0.0);
        let sc = |k: usize| f(d.scale[k], 1.0);
        self.speed01 = [sp(0, 0), sp(0, 1), sp(1, 0), sp(1, 1)];
        self.speed2_scale01 = [sp(2, 0), sp(2, 1), sc(0), sc(1)];
        self.misc0 = [sc(2), 0.0, f(d.distortion, 0.0), 0.0];
        self.misc1[2] = f(d.overbright, 1.0);
        self.extra1 = [d.alpha_test.map_or(-1.0, |t| f(t, 0.5)), 0.0, 0.0, 0.0];
        self.material = d.color.map(|c| f(c, 1.0));
        let r = d.matrix.sanitized().rows;
        self.uv_row0 = [r[0][0], r[0][1], r[0][2], 0.0];
        self.uv_row1 = [r[1][0], r[1][1], r[1][2], 0.0];
        self.lights = [lights[0], lights[1], lights[2], 0];
    }
}

/// How a water pipeline draws: geometry source, blending, depth write and culling.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
struct WaterPipeKey {
    format: wgpu::TextureFormat,
    /// Placed meshes (else generated quads).
    mesh: bool,
    /// Source alpha / inverse source alpha (mirror and liquid draws).
    blend: bool,
    depth_write: bool,
    /// `None`: both faces; `Some(false)`: back faces culled; `Some(true)`: front faces
    /// (mirrored transforms).
    cull: Option<bool>,
    /// Depth bias for the reversed depth ([`DrawDepthBias`]).
    bias: BiasKey,
}

/// Pipelines of the water, built lazily.
#[derive(Debug, Default)]
pub(crate) struct WaterState {
    layout: Option<wgpu::BindGroupLayout>,
    samplers: Option<(wgpu::Sampler, wgpu::Sampler)>,
    shader: Option<wgpu::ShaderModule>,
    pipeline_layout: Option<wgpu::PipelineLayout>,
    pipelines: Vec<(WaterPipeKey, wgpu::RenderPipeline)>,
}

impl WaterState {
    fn prepare(
        &mut self,
        ctx: &WorldFrame<'_>,
    ) -> (wgpu::BindGroupLayout, (wgpu::Sampler, wgpu::Sampler)) {
        let device = ctx.device;
        let layout = self
            .layout
            .get_or_insert_with(|| {
                device.create_bind_group_layout(&wgpu::BindGroupLayoutDescriptor {
                    label: Some("tessera-water-bgl"),
                    entries: &[
                        uniform_entry(
                            0,
                            wgpu::ShaderStages::VERTEX_FRAGMENT,
                            true,
                            size_of::<WaterUniform>(),
                        ),
                        texture_entry(1),
                        texture_entry(2),
                        sampler_entry(3),
                        sampler_entry(4),
                        texture_entry(5),
                        texture_entry(6),
                        texture_entry(7),
                        texture_entry(8),
                        texture_entry(9),
                        texture_entry(10),
                        sampler_entry(11),
                        sampler_entry(12),
                        sampler_entry(13),
                    ],
                })
            })
            .clone();
        let samplers = self
            .samplers
            .get_or_insert_with(|| {
                let make = |a: wgpu::AddressMode| {
                    device.create_sampler(&wgpu::SamplerDescriptor {
                        label: Some("tessera-water-sampler"),
                        address_mode_u: a,
                        address_mode_v: a,
                        address_mode_w: a,
                        mag_filter: wgpu::FilterMode::Linear,
                        min_filter: wgpu::FilterMode::Linear,
                        mipmap_filter: wgpu::MipmapFilterMode::Linear,
                        ..Default::default()
                    })
                };
                (
                    make(wgpu::AddressMode::Repeat),
                    make(wgpu::AddressMode::ClampToEdge),
                )
            })
            .clone();
        if self.shader.is_none() {
            self.shader = Some(device.create_shader_module(wgpu::ShaderModuleDescriptor {
                label: Some("tessera-water.wgsl"),
                source: wgpu::ShaderSource::Wgsl(water_source().into()),
            }));
        }
        if self.pipeline_layout.is_none() {
            self.pipeline_layout = Some(device.create_pipeline_layout(
                &wgpu::PipelineLayoutDescriptor {
                    label: Some("tessera-water-layout"),
                    bind_group_layouts: &[Some(ctx.frame_layout), Some(&layout)],
                    immediate_size: 0,
                },
            ));
        }
        (layout, samplers)
    }

    fn pipeline(
        &mut self,
        device: &wgpu::Device,
        key: WaterPipeKey,
    ) -> Option<wgpu::RenderPipeline> {
        if let Some((_, p)) = self.pipelines.iter().find(|(k, _)| *k == key) {
            return Some(p.clone());
        }
        let (shader, pl) = (self.shader.as_ref()?, self.pipeline_layout.as_ref()?);
        let quad_layout = [Some(wgpu::VertexBufferLayout {
            array_stride: size_of::<QuadGpu>() as u64,
            step_mode: wgpu::VertexStepMode::Instance,
            attributes: &QUAD_ATTRS,
        })];
        let mesh_layout = [Some(mesh_vertex_layout())];
        let (entry, buffers): (&str, &[Option<wgpu::VertexBufferLayout<'_>>]) = if key.mesh {
            ("vs_water_mesh", &mesh_layout)
        } else {
            ("vs_water", &quad_layout)
        };
        let p = device.create_render_pipeline(&wgpu::RenderPipelineDescriptor {
            label: Some("tessera-water"),
            layout: Some(pl),
            vertex: wgpu::VertexState {
                module: shader,
                entry_point: Some(entry),
                compilation_options: wgpu::PipelineCompilationOptions::default(),
                buffers,
            },
            primitive: wgpu::PrimitiveState {
                cull_mode: match key.cull {
                    None => None,
                    Some(false) => Some(wgpu::Face::Back),
                    Some(true) => Some(wgpu::Face::Front),
                },
                ..Default::default()
            },
            depth_stencil: Some(wgpu::DepthStencilState {
                bias: key.bias.state(),
                ..world_depth(key.depth_write)
            }),
            multisample: wgpu::MultisampleState::default(),
            fragment: Some(wgpu::FragmentState {
                module: shader,
                entry_point: Some(if key.blend {
                    "fs_water_blend"
                } else {
                    "fs_water"
                }),
                compilation_options: wgpu::PipelineCompilationOptions::default(),
                // ONE / ZERO: the water composites the scene copy itself (§8.4); the mirror
                // and liquid draws blend by source alpha (§8.5.4, §8.5.5).
                targets: &[Some(wgpu::ColorTargetState {
                    format: key.format,
                    blend: key.blend.then_some(wgpu::BlendState::ALPHA_BLENDING),
                    write_mask: wgpu::ColorWrites::ALL,
                })],
            }),
            multiview_mask: None,
            cache: None,
        });
        self.pipelines.push((key, p.clone()));
        Some(p)
    }
}

pub(crate) fn water_source() -> String {
    lit_source(&[WATER_WGSL])
}

/// What one water item draws.
#[derive(Debug)]
enum WaterGeometry {
    /// Generated quads: instances of the pass's quad buffer and vertices per instance.
    Quads { quads: Range<u32>, vertices: u32 },
    /// An arena submesh: the page's buffers and the absolute index range.
    Mesh {
        vertices: wgpu::Buffer,
        indices: wgpu::Buffer,
        range: Range<u32>,
    },
}

struct WaterItem {
    pipeline: wgpu::RenderPipeline,
    /// Index into the hook's bind groups.
    bind_group: usize,
    offset: u32,
    geometry: WaterGeometry,
}

struct WaterHook {
    quads: Option<wgpu::Buffer>,
    bind_groups: Vec<wgpu::BindGroup>,
    items: Vec<WaterItem>,
}

impl fmt::Debug for WaterHook {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.debug_struct("WaterHook")
            .field("items", &self.items.len())
            .finish_non_exhaustive()
    }
}

/// Grid cells per quad side for each draw of a pass, given its valid quad counts and
/// requested subdivisions: each request clamped to 1–[`MAX_WATER_SUBDIVISIONS`] and halved
/// until the draw fits in what [`MAX_WATER_VERTICES`] leaves after the earlier draws and one
/// cell per quad of the later ones. The total never exceeds the budget (with at most
/// [`MAX_WATER_QUADS`] quads, one cell each always fits).
fn grid_cells(draws: &[(u64, u32)]) -> Vec<u32> {
    let mut later: u64 = draws.iter().map(|&(q, _)| q * 6).sum();
    let mut used: u64 = 0;
    draws
        .iter()
        .map(|&(quads, requested)| {
            later -= quads * 6;
            let budget = MAX_WATER_VERTICES.saturating_sub(used.saturating_add(later));
            let mut n = requested.clamp(1, MAX_WATER_SUBDIVISIONS);
            while n > 1 && quads * 6 * u64::from(n) * u64::from(n) > budget {
                n /= 2;
            }
            used += quads * 6 * u64::from(n) * u64::from(n);
            n
        })
        .collect()
}

/// The per-draw uniforms and bind groups of a hook being built.
struct Slots<'a> {
    ctx: &'a WorldFrame<'a>,
    layout: wgpu::BindGroupLayout,
    stride: u64,
    uniforms: Vec<u8>,
    keys: HashMap<DrawTextures, usize>,
    sets: Vec<[Option<ResolvedTexture>; 7]>,
    warned: bool,
}

impl Slots<'_> {
    /// The resolved textures of `ids` and the index of their bind group.
    fn textures(&mut self, ids: DrawTextures) -> ([Option<ResolvedTexture>; 7], usize) {
        let resolved = ids.map(|id| self.ctx.textures.get(id));
        if !self.warned && ids.iter().any(|id| self.ctx.textures.is_missing(*id)) {
            tracing::warn!(?ids, "water texture unknown; drawn without it");
            self.warned = true;
        }
        let key = ids.map(|id| id.filter(|_| self.ctx.textures.get(id).is_some()));
        let index = match self.keys.get(&key) {
            Some(&i) => i,
            None => {
                self.sets.push(resolved.clone());
                self.keys.insert(key, self.sets.len() - 1);
                self.sets.len() - 1
            }
        };
        (resolved, index)
    }

    fn push(&mut self, u: &WaterUniform) -> Result<u32> {
        let offset = self.uniforms.len() as u64;
        let max = max_uniform_bytes(self.ctx.device);
        if offset + self.stride > max {
            return Err(RenderError::LimitExceeded {
                what: "water uniform bytes per pass",
                requested: offset + self.stride,
                max,
            });
        }
        self.uniforms.extend_from_slice(bytemuck::bytes_of(u));
        self.uniforms.resize((offset + self.stride) as usize, 0);
        Ok(offset as u32)
    }
}

pub(crate) fn build_hook(
    state: &mut WaterState,
    draws: Vec<WaterDraw>,
    liquid: Vec<LiquidMeshDraw>,
    ctx: &WorldFrame<'_>,
) -> Result<Option<Arc<dyn PassHook>>> {
    let total: usize = draws.iter().map(|d| d.quads.len()).sum();
    if total == 0 && liquid.is_empty() {
        return Ok(None);
    }
    if total > MAX_WATER_QUADS {
        return Err(RenderError::LimitExceeded {
            what: "water quads per pass",
            requested: total as u64,
            max: MAX_WATER_QUADS as u64,
        });
    }
    let (layout, (repeat, clamp)) = state.prepare(ctx);
    let white = ctx.textures.white();
    let scene = ctx.scene_copy;
    // The valid quads of every draw, back to back.
    let mut quads: Vec<QuadGpu> = Vec::with_capacity(total);
    let mut ranges = Vec::with_capacity(draws.len());
    for draw in &draws {
        let start = quads.len() as u32;
        quads.extend(
            draw.quads
                .iter()
                .filter_map(WaterQuad::sane)
                .map(|q| QuadGpu {
                    rect: [q.origin[0], q.origin[1], q.size[0], q.size[1]],
                    height: [q.height, 0.0, 0.0, 0.0],
                    uv: q.uv,
                }),
        );
        ranges.push(start..quads.len() as u32);
    }
    let cells = grid_cells(
        &draws
            .iter()
            .zip(&ranges)
            .map(|(d, r)| (u64::from(r.end - r.start), d.subdivisions))
            .collect::<Vec<_>>(),
    );
    let mut slots = Slots {
        ctx,
        layout: layout.clone(),
        stride: uniform_stride(ctx.device, size_of::<WaterUniform>()),
        uniforms: Vec::new(),
        keys: HashMap::new(),
        sets: Vec::new(),
        warned: false,
    };
    let mut items = Vec::with_capacity(draws.len() + liquid.len());
    let quad_pipe = state.pipeline(
        ctx.device,
        WaterPipeKey {
            format: ctx.format,
            mesh: false,
            blend: false,
            depth_write: true,
            cull: None,
            bias: BiasKey::NONE,
        },
    );
    for ((draw, range), n) in draws.iter().zip(ranges).zip(cells) {
        if range.is_empty() {
            continue;
        }
        let Some(pipeline) = quad_pipe.clone() else {
            continue;
        };
        let (resolved, bind_group) = slots.textures(draw_textures(draw.normal_map, &draw.textures));
        let u = WaterUniform::new(
            &draw.params,
            scene.is_some(),
            &resolved,
            n,
            ctx,
            draw.block_offset,
            draw.fade,
        );
        let offset = slots.push(&u)?;
        items.push(WaterItem {
            pipeline,
            bind_group,
            offset,
            geometry: WaterGeometry::Quads {
                quads: range,
                vertices: 6 * n * n,
            },
        });
    }

    // Placed surfaces: opaque families first, then the blended ones back to front.
    let mut placed: Vec<(bool, f32, &LiquidMeshDraw)> = Vec::with_capacity(liquid.len());
    let mut dropped = 0usize;
    for d in &liquid {
        if placed.len() >= MAX_LIQUID_MESH_DRAWS || !d.transform.is_finite() {
            dropped += 1;
            continue;
        }
        let Some(mesh) = ctx.arena.get(d.mesh) else {
            dropped += 1;
            continue;
        };
        let centre = d.transform.transform_point3(mesh.info.sphere.center);
        let dist = centre.distance_squared(ctx.eye);
        placed.push((
            d.shading.blended(),
            if dist.is_finite() { dist } else { 0.0 },
            d,
        ));
    }
    if dropped > 0 {
        tracing::debug!(
            dropped,
            "liquid mesh draws dropped (unknown mesh, transform or cap)"
        );
    }
    placed.sort_by(|a, b| a.0.cmp(&b.0).then(b.1.total_cmp(&a.1)));
    let mut chosen: Vec<u8> = Vec::with_capacity(MAX_DRAW_LIGHTS);
    for (blended, _, d) in placed {
        let Some(mesh) = ctx.arena.get(d.mesh) else {
            continue;
        };
        let Some(page) = ctx.arena.page(mesh.page) else {
            continue;
        };
        let (resolved, bind_group) = slots.textures(draw_textures(d.normal_map, &d.textures));
        let (mut u, depth_write, cull) = match &d.shading {
            LiquidShading::Water {
                params,
                block_offset,
            } => (
                WaterUniform::new(
                    params,
                    scene.is_some(),
                    &resolved,
                    1,
                    ctx,
                    *block_offset,
                    FarFadeParams::default(),
                ),
                true,
                None,
            ),
            LiquidShading::Mirror(m) => {
                let mut u = WaterUniform::new(
                    &WaterParams::default(),
                    scene.is_some(),
                    &resolved,
                    1,
                    ctx,
                    [0.0; 2],
                    FarFadeParams::default(),
                );
                u.mirror(m);
                // §8.5.4: depth write on, back faces culled.
                (u, true, Some(false))
            }
            LiquidShading::Distortion(p) => {
                let mut u = WaterUniform::new(
                    &WaterParams::default(),
                    scene.is_some(),
                    &resolved,
                    1,
                    ctx,
                    [0.0; 2],
                    FarFadeParams::default(),
                );
                let lights = if p.lit {
                    ctx.lights.select(
                        LightSelection::Auto,
                        &LightTarget::placed(mesh.info.aabb, d.transform, d.light_object),
                        &mut chosen,
                    );
                    light_words(&chosen)
                } else {
                    [0; 3]
                };
                u.distortion(p, lights);
                (u, p.depth_write, (!p.two_sided).then_some(false))
            }
        };
        u.model = d.transform.to_cols_array_2d();
        let mirrored = is_mirrored(d.transform);
        let Some(pipeline) = state.pipeline(
            ctx.device,
            WaterPipeKey {
                format: ctx.format,
                mesh: true,
                blend: blended,
                depth_write,
                cull: cull.map(|_| mirrored),
                bias: d.depth_bias.reversed(ctx.bias_clamp),
            },
        ) else {
            continue;
        };
        let offset = slots.push(&u)?;
        let submeshes: Vec<_> = match d.submesh {
            None => mesh.submeshes.iter().collect(),
            Some(i) => mesh.submeshes.get(i as usize).into_iter().collect(),
        };
        for s in submeshes {
            let range = mesh.triangles(s);
            if range.is_empty() {
                continue;
            }
            items.push(WaterItem {
                pipeline: pipeline.clone(),
                bind_group,
                offset,
                geometry: WaterGeometry::Mesh {
                    vertices: page.vertex_buffer.clone(),
                    indices: page.index_buffer.clone(),
                    range,
                },
            });
        }
    }
    if items.is_empty() {
        return Ok(None);
    }
    let uniform_buffer = ctx
        .device
        .create_buffer_init(&wgpu::util::BufferInitDescriptor {
            label: Some("tessera-water"),
            contents: &slots.uniforms,
            usage: wgpu::BufferUsages::UNIFORM,
        });
    let scene_view = scene.unwrap_or(&white.view);
    let bind_groups = slots
        .sets
        .iter()
        .map(|set| {
            let view = |i: usize| {
                set.get(i)
                    .and_then(Option::as_ref)
                    .map_or(&white.view, |t| &t.view)
            };
            let sampler = |i: usize| {
                set.get(i)
                    .and_then(Option::as_ref)
                    .map_or(&white.sampler, |t| &t.sampler)
            };
            ctx.device.create_bind_group(&wgpu::BindGroupDescriptor {
                label: Some("tessera-water"),
                layout: &slots.layout,
                entries: &[
                    wgpu::BindGroupEntry {
                        binding: 0,
                        resource: wgpu::BindingResource::Buffer(wgpu::BufferBinding {
                            buffer: &uniform_buffer,
                            offset: 0,
                            size: wgpu::BufferSize::new(size_of::<WaterUniform>() as u64),
                        }),
                    },
                    wgpu::BindGroupEntry {
                        binding: 1,
                        resource: wgpu::BindingResource::TextureView(view(0)),
                    },
                    wgpu::BindGroupEntry {
                        binding: 2,
                        resource: wgpu::BindingResource::TextureView(scene_view),
                    },
                    wgpu::BindGroupEntry {
                        binding: 3,
                        resource: wgpu::BindingResource::Sampler(&repeat),
                    },
                    wgpu::BindGroupEntry {
                        binding: 4,
                        resource: wgpu::BindingResource::Sampler(&clamp),
                    },
                    wgpu::BindGroupEntry {
                        binding: 5,
                        resource: wgpu::BindingResource::TextureView(view(1)),
                    },
                    wgpu::BindGroupEntry {
                        binding: 6,
                        resource: wgpu::BindingResource::TextureView(view(2)),
                    },
                    wgpu::BindGroupEntry {
                        binding: 7,
                        resource: wgpu::BindingResource::TextureView(view(3)),
                    },
                    wgpu::BindGroupEntry {
                        binding: 8,
                        resource: wgpu::BindingResource::TextureView(view(4)),
                    },
                    wgpu::BindGroupEntry {
                        binding: 9,
                        resource: wgpu::BindingResource::TextureView(view(5)),
                    },
                    wgpu::BindGroupEntry {
                        binding: 10,
                        resource: wgpu::BindingResource::TextureView(view(6)),
                    },
                    wgpu::BindGroupEntry {
                        binding: 11,
                        resource: wgpu::BindingResource::Sampler(sampler(4)),
                    },
                    wgpu::BindGroupEntry {
                        binding: 12,
                        resource: wgpu::BindingResource::Sampler(sampler(5)),
                    },
                    wgpu::BindGroupEntry {
                        binding: 13,
                        resource: wgpu::BindingResource::Sampler(sampler(6)),
                    },
                ],
            })
        })
        .collect();
    let quads = (!quads.is_empty()).then(|| {
        ctx.device
            .create_buffer_init(&wgpu::util::BufferInitDescriptor {
                label: Some("tessera-water-quads"),
                contents: bytemuck::cast_slice(&quads),
                usage: wgpu::BufferUsages::VERTEX,
            })
    });
    Ok(Some(Arc::new(WaterHook {
        quads,
        bind_groups,
        items,
    })))
}

impl PassHook for WaterHook {
    fn record(&self, rp: &mut wgpu::RenderPass<'_>, ctx: &HookRecord<'_>, stats: &mut FrameStats) {
        if ctx.stage != HookStage::Translucent {
            return;
        }
        let Some((group0, offset)) = ctx.group0 else {
            return;
        };
        rp.set_bind_group(0, group0, &[offset]);
        for item in &self.items {
            let Some(bg) = self.bind_groups.get(item.bind_group) else {
                continue;
            };
            rp.set_pipeline(&item.pipeline);
            rp.set_bind_group(1, bg, &[item.offset]);
            match &item.geometry {
                WaterGeometry::Quads { quads, vertices } => {
                    let Some(buffer) = &self.quads else { continue };
                    rp.set_vertex_buffer(0, buffer.slice(..));
                    rp.draw(0..*vertices, quads.clone());
                    stats.water_quads = stats.water_quads.saturating_add(quads.end - quads.start);
                }
                WaterGeometry::Mesh {
                    vertices,
                    indices,
                    range,
                } => {
                    rp.set_vertex_buffer(0, vertices.slice(..));
                    rp.set_index_buffer(indices.slice(..), wgpu::IndexFormat::Uint32);
                    rp.draw_indexed(range.clone(), 0, 0..1);
                }
            }
            stats.draw_calls = stats.draw_calls.saturating_add(1);
        }
    }
}

/// CPU twin of the water's height and slope (`water.md` §8.3): the wave height offset at
/// model position (`x`, `y`) and time `t`, and its derivatives along x and y.
#[must_use]
pub fn wave_height(params: &WaterParams, x: f32, y: f32, t: f32) -> (f32, f32, f32) {
    let (mut h, mut dx, mut dy) = (0.0f32, 0.0f32, 0.0f32);
    for k in 0..2 {
        let kf = k as f32;
        let a = params.wave_amplitude * (1.0 - 0.5 * kf);
        let fx = (-1.0 + 0.3 * kf) * params.wave_frequency * (kf + 1.0);
        let fy = 0.7 * kf * params.wave_frequency * (kf + 1.0);
        let s = fx * x + fy * y + params.wave_speed * t * (0.5 + 0.8 * kf);
        let s = wrap_pi(s);
        h += a * s.sin();
        dx += a * s.cos() * fx;
        dy += a * s.cos() * fy;
    }
    (h, dx, dy)
}

/// CPU twin of the normal-layer offsets the water program adds to its layer coordinates at
/// time `t` (seconds of the material clock, `water.md` §8.7), with the parameters as the
/// pipeline uploads them (non-finite values replaced by the defaults): plain water (§8.3,
/// on mesh UV §8.5) offsets layer *k* by speed *k* × `t`; older bump water (§8.5.6) by
/// (*b*, *b*), (4*b*, 4*b*), (8*b*, 8*b*) with *b* = `bump[1]` × `t`; flow-map water (§8.6.2)
/// only layers 1 and 2 when the record has them, by *g* × speed × `t` with *g* =
/// clamp(0.0004 × `view_depth`, 1, 10) (layer 0 follows the flow phases and has no offset);
/// lava none (§8.6.5). In texture repeats; `None` for a layer the family does not scroll.
#[must_use]
pub fn normal_layer_offsets(
    params: &WaterParams,
    t: f32,
    view_depth: f32,
) -> [Option<[f32; 2]>; 3] {
    let d = WaterParams::default();
    let sp = |k: usize| {
        [
            fin(params.normal_speed[k][0], d.normal_speed[k][0]) * t,
            fin(params.normal_speed[k][1], d.normal_speed[k][1]) * t,
        ]
    };
    match params.family {
        WaterFamily::Plain | WaterFamily::PlainUv => [Some(sp(0)), Some(sp(1)), Some(sp(2))],
        WaterFamily::OlderBump => {
            let b = fin(params.bump[1], 0.0) * t;
            [
                Some([b, b]),
                Some([4.0 * b, 4.0 * b]),
                Some([8.0 * b, 8.0 * b]),
            ]
        }
        WaterFamily::Flow if params.normal_layers >= 3 => {
            let g = fin(0.0004 * view_depth, 1.0).clamp(1.0, 10.0);
            let scaled = |k: usize| sp(k).map(|v| g * v);
            [None, Some(scaled(1)), Some(scaled(2))]
        }
        WaterFamily::Flow | WaterFamily::Lava => [None; 3],
    }
}

/// CPU twin of the far-draw fade of a water fragment at distance `d` from the eye (module
/// docs): *a* = 1 − sat((*d* − *s*) × *r*) with the values as the pipeline uploads them
/// (a non-finite start means no fade, a non-finite or negative inverse range 0).
#[must_use]
pub fn water_fade(fade: FarFadeParams, d: f32) -> f32 {
    let s = if fade.start.is_finite() {
        fade.start
    } else {
        f32::MAX
    };
    let r = if fade.inv_range.is_finite() {
        fade.inv_range.max(0.0)
    } else {
        0.0
    };
    1.0 - ((d - s) * r).clamp(0.0, 1.0)
}

/// `s` reduced to [−π, π).
fn wrap_pi(s: f32) -> f32 {
    let tau = std::f32::consts::TAU;
    s - tau * ((s + std::f32::consts::PI) / tau).floor()
}

/// CPU twin of the Fresnel factor for |*v* · *n*| = `v_dot_n`, with the parameters as the
/// pipeline uploads them (non-finite values replaced by the defaults, the exponent clamped
/// to 0–4096) and 0⁰ = 1: `water.md` §8.4 step 5, or for a flow-map draw with
/// [`FresnelForm::Scaled`] §8.6.4 step 4 (bias × (1 − |*v* · *n*|)^(1 / exponent), 0 for
/// an exponent of 0).
#[must_use]
pub fn fresnel(params: &WaterParams, v_dot_n: f32) -> f32 {
    let d = WaterParams::default();
    let f = |v: f32, d: f32| if v.is_finite() { v } else { d };
    let bias = f(params.fresnel_bias, d.fresnel_bias);
    let e = f(params.fresnel_exponent, d.fresnel_exponent).clamp(0.0, MAX_WATER_POWER);
    let x = f(1.0 - v_dot_n.abs(), 0.0).clamp(0.0, 1.0);
    if params.family == WaterFamily::Flow && params.fresnel_form == FresnelForm::Scaled {
        let inv = if e > 0.0 { 1.0 / e } else { 0.0 };
        let p = if inv <= 0.0 {
            1.0
        } else if x <= 0.0 {
            0.0
        } else {
            x.powf(inv)
        };
        // The shader's flow Fresnel: an exponent of 0 gives inv 0 and x⁰ = 1.
        return bias * p;
    }
    let p = if e <= 0.0 {
        1.0
    } else if x <= 0.0 {
        0.0
    } else {
        x.powf(e)
    };
    bias + (1.0 - bias) * p
}

#[cfg(test)]
mod tests {
    use super::*;

    /// Negative exponents are clamped to 0 (x⁰ = 1, also for x = 0), never inf or NaN.
    #[test]
    fn fresnel_with_hostile_exponents() {
        let p = |e: f32| WaterParams {
            fresnel_bias: 0.1,
            fresnel_exponent: e,
            ..WaterParams::default()
        };
        assert!((fresnel(&p(2.0), 0.5) - (0.1 + 0.9 * 0.25)).abs() < 1e-6);
        for v in [0.0, 0.5, 1.0] {
            assert_eq!(fresnel(&p(-3.0), v), 1.0, "v . n = {v}");
            assert_eq!(fresnel(&p(0.0), v), 1.0, "v . n = {v}");
        }
        assert_eq!(fresnel(&p(2.0), 1.0), 0.1);
        assert!(fresnel(&p(f32::NAN), f32::NAN).is_finite());
    }

    /// The layer offsets per family (`water.md` §8.3, §8.5.6, §8.6.2), hostile values
    /// replaced by the defaults.
    #[test]
    fn layer_offsets_by_family() {
        let speeds = [[0.005, 0.0075], [0.015, 0.02], [-0.01, -0.0125]];
        let plain = WaterParams {
            normal_speed: speeds,
            ..WaterParams::default()
        };
        let o = normal_layer_offsets(&plain, 2.0, 0.0);
        for (k, s) in speeds.iter().enumerate() {
            assert_eq!(o[k], Some([2.0 * s[0], 2.0 * s[1]]), "layer {k}");
        }
        let flow = WaterParams {
            family: WaterFamily::Flow,
            ..plain
        };
        let o = normal_layer_offsets(&flow, 1.0, 0.0);
        assert_eq!(o, [None, Some(speeds[1]), Some(speeds[2])]);
        // g = 0.0004 x 10,000 = 4.
        let far = normal_layer_offsets(&flow, 1.0, 10_000.0);
        assert_eq!(far[1], Some([4.0 * speeds[1][0], 4.0 * speeds[1][1]]));
        let one = WaterParams {
            normal_layers: 1,
            ..flow
        };
        assert_eq!(normal_layer_offsets(&one, 5.0, 0.0), [None; 3]);
        let bump = WaterParams {
            family: WaterFamily::OlderBump,
            bump: [1.0, 0.5],
            ..plain
        };
        assert_eq!(normal_layer_offsets(&bump, 2.0, 0.0)[2], Some([8.0, 8.0]));
        let lava = WaterParams {
            family: WaterFamily::Lava,
            ..plain
        };
        assert_eq!(normal_layer_offsets(&lava, 2.0, 0.0), [None; 3]);
        let nan = WaterParams {
            normal_speed: [[f32::NAN; 2]; 3],
            ..plain
        };
        let d = WaterParams::default().normal_speed;
        assert_eq!(normal_layer_offsets(&nan, 1.0, 0.0)[0], Some(d[0]));
    }

    #[test]
    fn water_shader_validates() {
        let src = water_source();
        let module = wgpu::naga::front::wgsl::parse_str(&src)
            .unwrap_or_else(|e| panic!("{}", e.emit_to_string(&src)));
        wgpu::naga::valid::Validator::new(
            wgpu::naga::valid::ValidationFlags::all(),
            wgpu::naga::valid::Capabilities::empty(),
        )
        .validate(&module)
        .unwrap_or_else(|e| panic!("{}", e.emit_to_string(&src)));
    }

    #[test]
    fn layouts_match_the_wgsl() {
        assert_eq!(size_of::<WaterUniform>(), 400);
        assert_eq!(size_of::<QuadGpu>(), 48);
    }

    #[test]
    fn sector_coordinates_follow_section_8_1() {
        let q = WaterQuad::landblock([0.0; 2], 1.0);
        assert_eq!(q.uv, [0.0, 1.0, 1.0, -1.0]);
        let s = q.with_sector_uv(2, 1);
        assert!((s.uv[0] - 2.0 / 3.0).abs() < 1e-6);
        assert!((s.uv[1] - 2.0 / 3.0).abs() < 1e-6);
        assert!((s.uv[2] - 1.0 / 3.0).abs() < 1e-6 && (s.uv[3] + 1.0 / 3.0).abs() < 1e-6);
        // Out-of-range places wrap into the sector.
        assert_eq!(q.with_sector_uv(5, 4).uv, q.with_sector_uv(2, 1).uv);
    }

    #[test]
    fn far_draw_fade_matches_the_far_terrain() {
        let f = FarFadeParams {
            start: 1_920.0,
            inv_range: 1.0 / 160.0,
        };
        assert_eq!(water_fade(f, 0.0), 1.0);
        assert_eq!(water_fade(f, 1_920.0), 1.0);
        assert!((water_fade(f, 2_000.0) - 0.5).abs() < 1e-6);
        assert_eq!(water_fade(f, 2_080.0), 0.0);
        assert_eq!(water_fade(f, 9_000.0), 0.0);
        // No fade by default, and hostile values never fade or produce NaN.
        assert_eq!(water_fade(FarFadeParams::default(), 1.0e9), 1.0);
        for (start, inv_range) in [(f32::NAN, 1.0), (0.0, f32::NAN), (0.0, -1.0)] {
            let a = water_fade(FarFadeParams { start, inv_range }, 100.0);
            assert_eq!(a, 1.0, "{start} {inv_range}");
        }
    }

    #[test]
    fn defaults_are_the_record_defaults() {
        let p = WaterParams::default();
        assert_eq!(p.wave_amplitude, 0.0);
        assert_eq!(p.normal_scale, [2.0, 4.0, 8.0]);
        assert_eq!(p.specular_power, 80.0);
        assert_eq!(p.fog_color, [0.0; 3]);
    }

    #[test]
    fn no_waves_with_zero_amplitude() {
        let p = WaterParams::default();
        assert_eq!(wave_height(&p, 123.0, 456.0, 7.0), (0.0, 0.0, 0.0));
    }

    #[test]
    fn waves_follow_section_8_3() {
        let p = WaterParams {
            wave_amplitude: 2.0,
            wave_frequency: 0.2,
            wave_speed: 1.0,
            ..WaterParams::default()
        };
        // At the origin and t = 0 both phases are 0.
        let (h, dx, dy) = wave_height(&p, 0.0, 0.0, 0.0);
        assert!(h.abs() < 1e-6);
        // dh/dx = A0 * (-1) * f + A1 * (-0.7) * 2f = -2 * 0.2 - 1 * 0.7 * 0.4.
        assert!((dx - (-0.4 - 0.28)).abs() < 1e-5, "{dx}");
        // dh/dy = A1 * 0.7 * 2f = 0.28.
        assert!((dy - 0.28).abs() < 1e-5, "{dy}");
        // The derivatives match central differences at the same point.
        let e = 1e-2;
        let (_, dx, dy) = wave_height(&p, 3.0, 5.0, 2.0);
        let (hxp, ..) = wave_height(&p, 3.0 + e, 5.0, 2.0);
        let (hxm, ..) = wave_height(&p, 3.0 - e, 5.0, 2.0);
        let (hyp, ..) = wave_height(&p, 3.0, 5.0 + e, 2.0);
        let (hym, ..) = wave_height(&p, 3.0, 5.0 - e, 2.0);
        let fdx = (hxp - hxm) / (2.0 * e);
        let fdy = (hyp - hym) / (2.0 * e);
        assert!((fdx - dx).abs() < 1e-3, "{fdx} {dx}");
        assert!((fdy - dy).abs() < 1e-3, "{fdy} {dy}");
    }

    #[test]
    fn grids_fit_the_pass_budget() {
        let verts = |d: &[(u64, u32)], c: &[u32]| -> u64 {
            d.iter()
                .zip(c)
                .map(|(&(q, _), &n)| q * 6 * u64::from(n) * u64::from(n))
                .sum()
        };
        // One small draw keeps its request; the request is clamped to 1-64.
        assert_eq!(grid_cells(&[(4, 16)]), vec![16]);
        assert_eq!(grid_cells(&[(4, 0), (4, 999)]), vec![1, 64]);
        // One draw per quad at the largest grid: the per-pass total stays bounded and
        // every draw keeps at least one cell.
        let many = vec![(1u64, 64u32); MAX_WATER_QUADS];
        let cells = grid_cells(&many);
        assert!(verts(&many, &cells) <= MAX_WATER_VERTICES);
        assert!(cells.iter().all(|&n| n >= 1));
        assert_eq!(cells[0], 64);
        assert_eq!(*cells.last().unwrap_or(&0), 1);
        // A single huge draw is coarsened as before.
        let one = [(MAX_WATER_QUADS as u64, 64)];
        let c = grid_cells(&one);
        assert!(verts(&one, &c) <= MAX_WATER_VERTICES && c[0] >= 1, "{c:?}");
        assert_eq!(grid_cells(&[]), Vec::<u32>::new());
    }

    #[test]
    fn wrap_pi_stays_in_range() {
        for s in [
            -100.0f32,
            -3.2,
            -std::f32::consts::PI,
            0.0,
            3.1,
            3.2,
            1000.0,
        ] {
            let w = wrap_pi(s);
            assert!(
                (-std::f32::consts::PI..std::f32::consts::PI + 1e-5).contains(&w),
                "{s} {w}"
            );
            assert!((w.sin() - s.sin()).abs() < 1e-3);
        }
    }

    /// `water.md` §8.6.4: flow-map water with [`FresnelForm::Scaled`] (0x2B0007FD) multiplies
    /// by the bias and inverts the exponent; with [`FresnelForm::Biased`] (0x2B0005EC, its own
    /// programs) it is the §8.4 step 5 form; the form is ignored outside the flow family.
    #[test]
    fn flow_fresnel_forms() {
        let flow = |form| WaterParams {
            family: WaterFamily::Flow,
            fresnel_bias: 0.01,
            fresnel_exponent: 2.0,
            fresnel_form: form,
            ..WaterParams::default()
        };
        let scaled = flow(FresnelForm::Scaled);
        let biased = flow(FresnelForm::Biased);
        // |v . n| = 0.75: x = 0.25.
        assert!((fresnel(&scaled, 0.75) - 0.01 * 0.5).abs() < 1e-6);
        assert!((fresnel(&biased, 0.75) - (0.01 + 0.99 * 0.0625)).abs() < 1e-6);
        // Grazing: the bias alone (scaled) against 1 (biased).
        assert!((fresnel(&scaled, 0.0) - 0.01).abs() < 1e-6);
        assert!((fresnel(&biased, 0.0) - 1.0).abs() < 1e-6);
        // Straight down: 0 against the bias.
        assert_eq!(fresnel(&scaled, 1.0), 0.0);
        assert!((fresnel(&biased, 1.0) - 0.01).abs() < 1e-6);
        // Exponent 0: x^(1/0) is taken as x^0 = 1, never inf or NaN.
        let zero = WaterParams {
            fresnel_exponent: 0.0,
            ..scaled
        };
        assert!((fresnel(&zero, 0.5) - 0.01).abs() < 1e-6);
        assert!(
            fresnel(
                &WaterParams {
                    fresnel_exponent: f32::NAN,
                    ..scaled
                },
                0.3
            )
            .is_finite()
        );
        // Outside the flow family the form does not apply.
        let plain = WaterParams {
            family: WaterFamily::Plain,
            ..scaled
        };
        assert!((fresnel(&plain, 0.0) - 1.0).abs() < 1e-6);
    }

    #[test]
    fn fresnel_follows_step_5() {
        let p = WaterParams::default();
        // Looking straight down: the bias.
        assert!((fresnel(&p, 1.0) - 0.1).abs() < 1e-6);
        // Grazing: 1.
        assert!((fresnel(&p, 0.0) - 1.0).abs() < 1e-6);
        assert!((fresnel(&p, 0.5) - (0.1 + 0.9 * 0.25)).abs() < 1e-6);
    }

    #[test]
    fn hostile_quads_are_dropped() {
        assert!(WaterQuad::landblock([0.0, 0.0], 1.0).sane().is_some());
        let q = WaterQuad::landblock([0.0; 2], 0.0);
        assert!(
            WaterQuad {
                origin: [f32::NAN, 0.0],
                size: [1.0; 2],
                ..q
            }
            .sane()
            .is_none()
        );
        assert!(
            WaterQuad {
                size: [-1.0, 1.0],
                ..q
            }
            .sane()
            .is_none()
        );
        assert!(
            WaterQuad {
                uv: [f32::INFINITY, 0.0, 1.0, 1.0],
                ..q
            }
            .sane()
            .is_none()
        );
    }
}
