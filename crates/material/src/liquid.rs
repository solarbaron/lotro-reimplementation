// SPDX-License-Identifier: GPL-3.0-or-later
//! The materials of the records that the generic path must not draw (`material.md` §11.6):
//! water records ([`WaterMaterial`], `water.md` §8.2–§8.6), the mirror record of puddles
//! ([`MirrorMaterial`], `water.md` §8.5.4), the liquid-distortion records
//! ([`DistortionMaterial`], `water.md` §8.5.5) and the lava record ([`LavaMaterial`],
//! `water.md` §8.6.5). [`LiquidSurface::evaluate`] picks the one the shader record's
//! program family asks for.
//!
//! Every value is evaluated at the time given with the instance's curve states
//! (`material.md` §5.3.1), so a renderer that evaluates the material every frame draws
//! curve-valued parameters as `water.md` §8.7.1 requires (never once at load). Every float
//! of the result is finite. The formulas themselves belong to the renderer.

use tessera_formats::Did;

use crate::interface::ProgramFamily;
use crate::records::{CurveStates, curve_finite};
use crate::resolve::ResolvedMaterial;
use crate::transform::{TextureMatrix, TextureMatrixLayout};
use crate::vocab::ids;
use crate::water::{WaterMaterial, water_ids};

/// Mirror parameter ids (`water.md` §8.5.4; labels from enum mapper 0x230001A6, for
/// interoperability).
pub mod mirror_ids {
    /// `c_MirrorReflectionBumpDistortionAmount`.
    pub const REFLECTION_DISTORTION: u32 = 1100;
    /// `c_MirrorFresnelExponent`.
    pub const FRESNEL_EXPONENT: u32 = 1101;
    /// `c_MirrorFresnelBias`.
    pub const FRESNEL_BIAS: u32 = 1102;
    /// `c_MirrorNormalTextureScale`.
    pub const NORMAL_TEXTURE_SCALE: u32 = 1103;
    /// `c_MirrorNormalSpeedU`, `c_MirrorNormalSpeedV`.
    pub const NORMAL_SPEED: [u32; 2] = [1104, 1105];
    /// `c_MirrorFresnelBumpAmount`.
    pub const FRESNEL_BUMP_AMOUNT: u32 = 1106;
}

/// Whether a liquid-distortion record's layers read the world position *B* instead of the
/// mesh UV through texture matrix 0 (`water.md` §8.5.5 step 1): by the record-wide interface
/// rule (i) of W52, the record none of whose declarations has a binding with a semantic of
/// 38–44 ([`crate::ShaderInterface::binds_transform_items`]). In the reference install this
/// selects exactly 0x2B000442 among the four liquid-distortion records (re-checked when the
/// data changes, `world_fidelity_real_data`).
#[must_use]
pub fn liquid_reads_world_position(shader: &crate::ShaderInterface) -> bool {
    shader.family == ProgramFamily::LiquidDistortion && !shader.binds_transform_items()
}

/// The liquid-distortion records whose texel is multiplied by the vertex colour, RGBA,
/// before the alpha test (`water.md` §8.5.5 step 5, W45: 0x2B00063E). The interface
/// property (a vertex program with a colour input, W46) needs the program's input signature,
/// which Tessera does not read; Tessera recognises it by DID (Tessera Decision of the
/// §8.5.5 answer of 2026-10-10).
pub const LIQUID_VERTEX_COLOR_RECORDS: [u32; 1] = [0x2B00_063E];

const WHITE: [f32; 4] = [1.0; 4];

fn colour4(m: &ResolvedMaterial, id: u32, d: [f32; 4]) -> [f32; 4] {
    m.params.color(id).map_or(d, |c| c.map(curve_finite))
}

fn float(m: &ResolvedMaterial, id: u32, d: f32, t: f64, states: &mut CurveStates) -> f32 {
    m.params.float_at(id, t, states).map_or(d, curve_finite)
}

/// Texture matrix `index` of `material` at `t` (`material.md` §8.4), the identity when the
/// record binds none.
#[must_use]
pub fn texture_matrix_at(
    material: &ResolvedMaterial,
    layout: &TextureMatrixLayout,
    index: u32,
    t: f64,
    states: &mut CurveStates,
) -> TextureMatrix {
    layout
        .evaluate(&material.params, t, states)
        .into_iter()
        .find(|m| m.matrix == index)
        .map_or(TextureMatrix::IDENTITY, |m| m.map)
}

/// A surface of the mirror record (`water.md` §8.5.4), evaluated at one time.
#[derive(Debug, Clone, PartialEq)]
pub struct MirrorMaterial {
    /// The shader record.
    pub shader: Did,
    /// 1100: reflection offset by the normal (0 when undeclared).
    pub reflection_distortion: f32,
    /// 1101: Fresnel exponent (1 when undeclared).
    pub fresnel_exponent: f32,
    /// 1102: Fresnel bias (0 when undeclared).
    pub fresnel_bias: f32,
    /// 1103: normal-map tiling (1 when undeclared).
    pub normal_scale: f32,
    /// 1104, 1105: the normal-map speeds *S* evaluated at the clock; the program multiplies
    /// them by the time input again (`water.md` §8.5.4).
    pub normal_speed: [f32; 2],
    /// 1106: Fresnel bump amount (0 when undeclared).
    pub fresnel_bump: f32,
    /// Slot 3000: the diffuse image (its alpha is the shape).
    pub diffuse: Option<Did>,
    /// Slot 3011: the normal map.
    pub normal_map: Option<Did>,
    /// The material colour 2000 (RGBA, white when undeclared).
    pub color: [f32; 4],
}

impl MirrorMaterial {
    /// The mirror material of `material` at `t`, `None` unless its shader is the mirror
    /// record (`material.md` §11.6).
    pub fn evaluate(material: &ResolvedMaterial, t: f64, states: &mut CurveStates) -> Option<Self> {
        if material.shader.family != ProgramFamily::Mirror {
            return None;
        }
        use mirror_ids as m;
        let f = |id, d, states: &mut CurveStates| float(material, id, d, t, states);
        Some(Self {
            shader: material.shader.did,
            reflection_distortion: f(m::REFLECTION_DISTORTION, 0.0, states),
            fresnel_exponent: f(m::FRESNEL_EXPONENT, 1.0, states),
            fresnel_bias: f(m::FRESNEL_BIAS, 0.0, states),
            normal_scale: f(m::NORMAL_TEXTURE_SCALE, 1.0, states),
            normal_speed: [
                f(m::NORMAL_SPEED[0], 0.0, states),
                f(m::NORMAL_SPEED[1], 0.0, states),
            ],
            fresnel_bump: f(m::FRESNEL_BUMP_AMOUNT, 0.0, states),
            diffuse: material.params.texture(ids::DIFFUSE),
            normal_map: material.params.texture(ids::NORMAL),
            color: colour4(material, ids::DIFFUSE_COLOR, WHITE),
        })
    }

    /// Whether every float is finite.
    #[must_use]
    pub fn all_finite(&self) -> bool {
        [
            self.reflection_distortion,
            self.fresnel_exponent,
            self.fresnel_bias,
            self.normal_scale,
            self.fresnel_bump,
        ]
        .iter()
        .chain(&self.normal_speed)
        .chain(&self.color)
        .all(|v| v.is_finite())
    }
}

/// A surface of a liquid-distortion record (`water.md` §8.5.5), evaluated at one time.
#[derive(Debug, Clone, PartialEq)]
pub struct DistortionMaterial {
    /// The shader record.
    pub shader: Did,
    /// The record lights its colour like a lit `ColorAndOpacity` surface (0x2B0007D0).
    pub lit: bool,
    /// The layers read the world position *B* instead of the mesh UV through texture
    /// matrix 0 ([`liquid_reads_world_position`]).
    pub world_coordinates: bool,
    /// 1044–1046: vertex waves (speed, frequency, amplitude); the unlit records only.
    pub waves: [f32; 3],
    /// 1048: distortion amount.
    pub distortion: f32,
    /// 1057–1062: scroll speed (U, V) of layers 0, 1, 2.
    pub speed: [[f32; 2]; 3],
    /// 1063–1065: tiling of layers 0, 1, 2.
    pub scale: [f32; 3],
    /// 1066: colour scale of the unlit records (1 when undeclared).
    pub overbright: f32,
    /// The normal (3011 `LiquidNormalTexture`) or distortion (3026 `Distortion`) texture.
    pub normal_map: Option<Did>,
    /// Slot 3000: the colour texture.
    pub diffuse: Option<Did>,
    /// Texture matrix 0 at the time evaluated (`material.md` §8.4).
    pub matrix: TextureMatrix,
    /// The material colour 2000 (RGBA, white when undeclared).
    pub color: [f32; 4],
    /// The alpha-test threshold when the test is on (`material.md` §11.2).
    pub alpha_test: Option<f32>,
    /// Both faces drawn (`material.md` §11.4).
    pub two_sided: bool,
    /// Depth write (`material.md` §11.3).
    pub depth_write: bool,
    /// The texel (colour and alpha) is multiplied by the vertex colour before the alpha
    /// test reads the output alpha ([`LIQUID_VERTEX_COLOR_RECORDS`], `water.md` W45).
    pub vertex_color: bool,
}

impl DistortionMaterial {
    /// The liquid-distortion material of `material` at `t`, with `layout` the texture-matrix
    /// bindings of its shader record; `None` unless its shader is a liquid-distortion record.
    pub fn evaluate(
        material: &ResolvedMaterial,
        layout: &TextureMatrixLayout,
        t: f64,
        states: &mut CurveStates,
    ) -> Option<Self> {
        if material.shader.family != ProgramFamily::LiquidDistortion {
            return None;
        }
        let f = |id, d, states: &mut CurveStates| float(material, id, d, t, states);
        let w = water_ids::NORMAL_SPEED;
        let speed = [
            [f(w[0][0], 0.0, states), f(w[0][1], 0.0, states)],
            [f(w[1][0], 0.0, states), f(w[1][1], 0.0, states)],
            [f(w[2][0], 0.0, states), f(w[2][1], 0.0, states)],
        ];
        let scale = water_ids::NORMAL_SCALE.map(|id| f(id, 1.0, states));
        let p = &material.params;
        Some(Self {
            shader: material.shader.did,
            lit: material.features.lit,
            world_coordinates: liquid_reads_world_position(&material.shader),
            waves: [
                f(water_ids::WAVE_SPEED, 0.0, states),
                f(water_ids::WAVE_FREQUENCY, 0.0, states),
                f(water_ids::WAVE_AMPLITUDE, 0.0, states),
            ],
            distortion: f(water_ids::REFRACTION_DISTORTION, 0.0, states),
            speed,
            scale,
            overbright: f(water_ids::OVERBRIGHT, 1.0, states),
            normal_map: p
                .texture(ids::NORMAL)
                .or_else(|| p.texture(ids::DISTORTION)),
            diffuse: p.texture(ids::DIFFUSE),
            matrix: texture_matrix_at(material, layout, 0, t, states),
            color: colour4(material, ids::DIFFUSE_COLOR, WHITE),
            alpha_test: material
                .features
                .alpha_test
                .map(|a| curve_finite(a.threshold)),
            two_sided: material.features.two_sided,
            depth_write: material.features.blend.depth_write,
            vertex_color: LIQUID_VERTEX_COLOR_RECORDS.contains(&material.shader.did.0),
        })
    }

    /// Whether every float is finite.
    #[must_use]
    pub fn all_finite(&self) -> bool {
        self.waves
            .iter()
            .chain(&[self.distortion, self.overbright])
            .chain(self.speed.iter().flatten())
            .chain(&self.scale)
            .chain(self.matrix.rows.iter().flatten())
            .chain(&self.color)
            .chain(self.alpha_test.iter())
            .all(|v| v.is_finite())
    }
}

/// A surface of the lava record (`water.md` §8.6.5), evaluated at one time.
#[derive(Debug, Clone, PartialEq)]
pub struct LavaMaterial {
    /// The shader record.
    pub shader: Did,
    /// 1057, 1058: flow-phase rates (cycles per second).
    pub rates: [f32; 2],
    /// 1063: scale of the colour layers and of the emissive layer (1 when undeclared).
    pub scale: f32,
    /// 1066: brightness (1 when undeclared).
    pub brightness: f32,
    /// 1182, 1183: foam lookup offset.
    pub foam_offset: [f32; 2],
    /// 1184, 1185: foam tiling.
    pub foam_tile: [f32; 2],
    /// 1186, 1187: offset of the flow map's third-channel lookup.
    pub depth_offset: [f32; 2],
    /// 1188: rotation of the phase coordinates (radians).
    pub rotation: f32,
    /// Slots 3000 and 3001: the two colour layers.
    pub color_layers: [Option<Did>; 2],
    /// Slot 3018: the emissive layer.
    pub emissive: Option<Did>,
    /// Slot 3075: the flow map.
    pub flow_map: Option<Did>,
    /// Slot 3079: the foam map.
    pub foam_map: Option<Did>,
    /// The noise texture (the default texture of a sampler no binding names, §8.6.1).
    pub noise_map: Option<Did>,
}

impl LavaMaterial {
    /// The lava material of `material` at `t`, `None` unless its shader is the lava record.
    pub fn evaluate(material: &ResolvedMaterial, t: f64, states: &mut CurveStates) -> Option<Self> {
        if material.shader.family != ProgramFamily::Lava {
            return None;
        }
        let f = |id, d, states: &mut CurveStates| float(material, id, d, t, states);
        let p = &material.params;
        Some(Self {
            shader: material.shader.did,
            rates: [f(1057, 0.0, states), f(1058, 0.0, states)],
            scale: f(1063, 1.0, states),
            brightness: f(water_ids::OVERBRIGHT, 1.0, states),
            foam_offset: water_ids::FOAM_OFFSET.map(|id| f(id, 0.0, states)),
            foam_tile: water_ids::FOAM_TILE.map(|id| f(id, 0.0, states)),
            depth_offset: water_ids::DEPTH_OFFSET.map(|id| f(id, 0.0, states)),
            rotation: f(water_ids::NORMAL_WAVE_ROTATION, 0.0, states),
            color_layers: [p.texture(ids::DIFFUSE), p.texture(ids::SECOND_LAYER)],
            emissive: p.texture(ids::EMISSIVE),
            flow_map: p.texture(ids::FLOW),
            foam_map: p.texture(ids::FOAM),
            noise_map: material.shader.unbound_defaults.first().copied(),
        })
    }

    /// Whether every float is finite.
    #[must_use]
    pub fn all_finite(&self) -> bool {
        self.rates
            .iter()
            .chain(&[self.scale, self.brightness, self.rotation])
            .chain(&self.foam_offset)
            .chain(&self.foam_tile)
            .chain(&self.depth_offset)
            .all(|v| v.is_finite())
    }
}

/// The own-program material of a surface (`material.md` §11.6), by its record's family.
// The water variant is the largest; values are short-lived (evaluated per frame and turned
// into render parameters), so boxing would only add an allocation per evaluation.
#[allow(clippy::large_enum_variant)]
#[derive(Debug, Clone, PartialEq)]
pub enum LiquidSurface {
    /// A water record.
    Water(WaterMaterial),
    /// The mirror record.
    Mirror(MirrorMaterial),
    /// A liquid-distortion record.
    Distortion(DistortionMaterial),
    /// The lava record.
    Lava(LavaMaterial),
}

impl LiquidSurface {
    /// The own-program material of `material` at `t` (`layout`: its shader record's
    /// texture-matrix bindings), `None` for a record of the generic path.
    pub fn evaluate(
        material: &ResolvedMaterial,
        layout: &TextureMatrixLayout,
        t: f64,
        states: &mut CurveStates,
    ) -> Option<Self> {
        match material.shader.family {
            ProgramFamily::Generic => None,
            ProgramFamily::Water => WaterMaterial::evaluate(material, t, states).map(Self::Water),
            ProgramFamily::Mirror => {
                MirrorMaterial::evaluate(material, t, states).map(Self::Mirror)
            }
            ProgramFamily::LiquidDistortion => {
                DistortionMaterial::evaluate(material, layout, t, states).map(Self::Distortion)
            }
            ProgramFamily::Lava => LavaMaterial::evaluate(material, t, states).map(Self::Lava),
        }
    }

    /// The texture lists the drawing samples, deduplicated, in a fixed order.
    #[must_use]
    pub fn textures(&self) -> Vec<Did> {
        let list: Vec<Option<Did>> = match self {
            Self::Water(w) => vec![w.normal_map, w.flow_map, w.foam_map, w.noise_map],
            Self::Mirror(m) => vec![m.diffuse, m.normal_map],
            Self::Distortion(d) => vec![d.normal_map, d.diffuse],
            Self::Lava(l) => vec![
                l.color_layers[0],
                l.color_layers[1],
                l.emissive,
                l.flow_map,
                l.foam_map,
                l.noise_map,
            ],
        };
        let mut out: Vec<Did> = Vec::with_capacity(list.len());
        for d in list.into_iter().flatten() {
            if d.0 != 0 && !out.contains(&d) {
                out.push(d);
            }
        }
        out
    }

    /// Whether every float is finite.
    #[must_use]
    pub fn all_finite(&self) -> bool {
        match self {
            Self::Water(w) => w.all_finite(),
            Self::Mirror(m) => m.all_finite(),
            Self::Distortion(d) => d.all_finite(),
            Self::Lava(l) => l.all_finite(),
        }
    }
}
