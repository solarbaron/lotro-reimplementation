// SPDX-License-Identifier: GPL-3.0-or-later
//! The water material chain (`water.md` §8.2).
//!
//! A water surface record (0x31) resolves like any other: shader record (0x2B) and
//! materials (0x30) → effective parameter set (`material.md` §9). The water programs then
//! read the `c_Water…` parameters by name (flag byte 1, `material.md` §8.6).
//! [`WaterMaterial`] gathers the parameters of the §8.2 table, evaluated at a time, plus
//! the textures and the material colour the water drawing of `water.md` §8.3–§8.4 needs.
//! The formulas themselves belong to the renderer.
//!
//! A parameter that the shader record does not declare takes the default that record
//! 0x2B0000A8 declares for it (the "Shader default" column of §8.2,
//! [`WaterParams::SPEC_DEFAULTS`]) and is listed in [`WaterMaterial::undeclared`]; one
//! that it declares with another value type takes that default too and is listed in
//! [`WaterMaterial::mistyped`].
//!
//! The water records belong to program families (`water.md` §8.2 "Program families"):
//! [`water_family`] tells plain water (§8.3–§8.4), plain water on mesh UV and older bump
//! water (§8.5.6) and flow-map water (§8.6) apart, and [`WaterMaterial`] carries the
//! parameters of each ([`FlowParams`], [`BumpParams`]). Every value is evaluated at the time
//! given, so a renderer that evaluates the material every frame draws curve-valued
//! parameters as `water.md` §8.7.1 requires.

use tessera_formats::Did;

use crate::interface::{ProgramFamily, ShaderInterface};
use crate::records::{CurveStates, curve_finite};
use crate::resolve::ResolvedMaterial;
use crate::vocab::ids;

/// Water parameter ids read by the water programs of 0x2B0000A8 (`water.md` §8.2, labels
/// from enum mapper 0x230001A6, `material.md` §8.0).
pub mod water_ids {
    /// `c_WaterWaveSpeed`: vertex waves.
    pub const WAVE_SPEED: u32 = 1044;
    /// `c_WaterWaveFrequency`: vertex waves.
    pub const WAVE_FREQUENCY: u32 = 1045;
    /// `c_WaterWaveAmplitude`: vertex waves.
    pub const WAVE_AMPLITUDE: u32 = 1046;
    /// `c_WaterReflectionBumpDistortionAmount`: reflection offset.
    pub const REFLECTION_DISTORTION: u32 = 1047;
    /// `c_WaterRefractionBumpDistortionAmount`: refraction offset.
    pub const REFRACTION_DISTORTION: u32 = 1048;
    /// `c_WaterFresnelBias`.
    pub const FRESNEL_BIAS: u32 = 1050;
    /// `c_WaterFresnelExponent`.
    pub const FRESNEL_EXPONENT: u32 = 1051;
    /// `c_WaterSpecularPower`: specular exponent.
    pub const SPECULAR_POWER: u32 = 1054;
    /// Scroll speeds of the three normal layers: (U, V) of layer 0, 1, 2
    /// (`c_WaterNormal0SpeedU` … `c_WaterNormal2SpeedV`, 1057–1062).
    pub const NORMAL_SPEED: [[u32; 2]; 3] = [[1057, 1058], [1059, 1060], [1061, 1062]];
    /// Tiling of the three normal layers (`c_WaterNormal0Scale` … `2Scale`, 1063–1065).
    pub const NORMAL_SCALE: [u32; 3] = [1063, 1064, 1065];
    /// `c_WaterOverbrightAmount`: colour scale.
    pub const OVERBRIGHT: u32 = 1066;
    /// `c_WaterFakeSpecularThreshold`: sparkle threshold.
    pub const FAKE_SPECULAR_THRESHOLD: u32 = 1067;
    /// `c_WaterFogAmount`: under-water tint of the refraction.
    pub const FOG_AMOUNT: u32 = 1068;
    /// `c_WaterSpecularColor`: specular tint.
    pub const SPECULAR_COLOR: u32 = 2006;
    /// `c_WaterColor`: colour multiplier.
    pub const COLOR: u32 = 2009;
    /// `c_WaterFogColor`: under-water tint colour.
    pub const FOG_COLOR: u32 = 2010;
    /// `c_WaterShorelineColor`: near-camera refraction tint.
    pub const SHORELINE_COLOR: u32 = 2011;

    /// Every float id of the §8.2 table.
    pub const FLOATS: [u32; 20] = [
        1044, 1045, 1046, 1047, 1048, 1050, 1051, 1054, 1057, 1058, 1059, 1060, 1061, 1062, 1063,
        1064, 1065, 1066, 1067, 1068,
    ];
    /// Every colour id of the §8.2 table.
    pub const COLORS: [u32; 4] = [2006, 2009, 2010, 2011];

    /// `c_WaterBumpTextureScale` (older bump water, `water.md` §8.5.6).
    pub const BUMP_TEXTURE_SCALE: u32 = 1055;
    /// `c_WaterBumpSpeed` (older bump water).
    pub const BUMP_SPEED: u32 = 1056;
    /// `c_WaterFresnelBumpAmount` (older bump water).
    pub const FRESNEL_BUMP_AMOUNT: u32 = 1049;
    /// `c_WaterReflectionAmount` (older bump water).
    pub const REFLECTION_AMOUNT: u32 = 1052;
    /// `c_WaterRefractionAmount` (older bump water).
    pub const REFRACTION_AMOUNT: u32 = 1053;
    /// `c_WaterDeepColor` (older bump water).
    pub const DEEP_COLOR: u32 = 2004;
    /// `c_WaterShallowColor` (older bump water).
    pub const SHALLOW_COLOR: u32 = 2005;
    /// `c_WaterShorelineMaxDepth` (flow-map water, `water.md` §8.6.1).
    pub const SHORELINE_MAX_DEPTH: u32 = 1170;
    /// `c_WaterFogMinDepth`.
    pub const FOG_MIN_DEPTH: u32 = 1171;
    /// `c_WaterFogDepthFactor`.
    pub const FOG_DEPTH_FACTOR: u32 = 1173;
    /// `c_WaterFoamOffsetU`, `…V`.
    pub const FOAM_OFFSET: [u32; 2] = [1182, 1183];
    /// `c_WaterFoamTileU`, `…V`.
    pub const FOAM_TILE: [u32; 2] = [1184, 1185];
    /// `c_WaterDepthOffsetU`, `…V`.
    pub const DEPTH_OFFSET: [u32; 2] = [1186, 1187];
    /// `c_NormalWaveRotation` (radians).
    pub const NORMAL_WAVE_ROTATION: u32 = 1188;
    /// `c_DepthBasedOpacityOffset`.
    pub const DEPTH_OPACITY_OFFSET: u32 = 1189;
    /// `c_FoamAlphaFadeFactor`.
    pub const FOAM_ALPHA_FADE: u32 = 1190;

    /// Whether `id` belongs to the water parameter families of `material.md` §8.6 (flag
    /// byte 1): 1044–1069, 1170, 1171, 1173, 1182–1190, 2004–2011.
    #[must_use]
    pub fn is_water_family(id: u32) -> bool {
        matches!(id, 1044..=1069 | 1170 | 1171 | 1173 | 1182..=1190 | 2004..=2011)
    }
}

/// The water parameters of `water.md` §8.2, evaluated.
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct WaterParams {
    /// 1044, vertex-wave speed.
    pub wave_speed: f32,
    /// 1045, vertex-wave frequency.
    pub wave_frequency: f32,
    /// 1046, vertex-wave amplitude (0: no waves).
    pub wave_amplitude: f32,
    /// 1057–1062, scroll speed (U, V) of normal layers 0, 1, 2 per second.
    pub normal_speed: [[f32; 2]; 3],
    /// 1063–1065, tiling of normal layers 0, 1, 2.
    pub normal_scale: [f32; 3],
    /// 1047, reflection offset by the normal.
    pub reflection_distortion: f32,
    /// 1048, refraction offset by the normal.
    pub refraction_distortion: f32,
    /// 1050, Fresnel bias.
    pub fresnel_bias: f32,
    /// 1051, Fresnel exponent.
    pub fresnel_exponent: f32,
    /// 1054, specular exponent.
    pub specular_power: f32,
    /// 1066, colour scale.
    pub overbright: f32,
    /// 1067, sparkle threshold.
    pub fake_specular_threshold: f32,
    /// 1068, under-water tint amount of the refraction.
    pub fog_amount: f32,
    /// 2006, specular tint (RGBA).
    pub specular_color: [f32; 4],
    /// 2009, colour multiplier (RGBA).
    pub color: [f32; 4],
    /// 2010, under-water tint colour (RGBA).
    pub fog_color: [f32; 4],
    /// 2011, near-camera refraction tint (RGBA).
    pub shoreline_color: [f32; 4],
}

const WHITE: [f32; 4] = [1.0; 4];
/// "black" in the §8.2 table; its alpha is not stated, taken as 1.
const BLACK: [f32; 4] = [0.0, 0.0, 0.0, 1.0];

impl WaterParams {
    /// The shader defaults of record 0x2B0000A8 as tabled in `water.md` §8.2, used for a
    /// parameter that a water shader record does not declare.
    pub const SPEC_DEFAULTS: Self = Self {
        wave_speed: 1.0,
        wave_frequency: 0.2,
        wave_amplitude: 0.0,
        normal_speed: [[0.005, 0.0075], [0.015, 0.02], [-0.01, -0.0125]],
        normal_scale: [2.0, 4.0, 8.0],
        reflection_distortion: 0.1,
        refraction_distortion: 0.04,
        fresnel_bias: 0.1,
        fresnel_exponent: 2.0,
        specular_power: 80.0,
        overbright: 1.0,
        fake_specular_threshold: 0.25,
        fog_amount: 1.0,
        specular_color: WHITE,
        color: WHITE,
        fog_color: BLACK,
        shoreline_color: WHITE,
    };

    fn float_mut(&mut self, id: u32) -> Option<&mut f32> {
        use water_ids as w;
        Some(match id {
            w::WAVE_SPEED => &mut self.wave_speed,
            w::WAVE_FREQUENCY => &mut self.wave_frequency,
            w::WAVE_AMPLITUDE => &mut self.wave_amplitude,
            w::REFLECTION_DISTORTION => &mut self.reflection_distortion,
            w::REFRACTION_DISTORTION => &mut self.refraction_distortion,
            w::FRESNEL_BIAS => &mut self.fresnel_bias,
            w::FRESNEL_EXPONENT => &mut self.fresnel_exponent,
            w::SPECULAR_POWER => &mut self.specular_power,
            1057 => &mut self.normal_speed[0][0],
            1058 => &mut self.normal_speed[0][1],
            1059 => &mut self.normal_speed[1][0],
            1060 => &mut self.normal_speed[1][1],
            1061 => &mut self.normal_speed[2][0],
            1062 => &mut self.normal_speed[2][1],
            1063 => &mut self.normal_scale[0],
            1064 => &mut self.normal_scale[1],
            1065 => &mut self.normal_scale[2],
            w::OVERBRIGHT => &mut self.overbright,
            w::FAKE_SPECULAR_THRESHOLD => &mut self.fake_specular_threshold,
            w::FOG_AMOUNT => &mut self.fog_amount,
            _ => return None,
        })
    }

    fn color_mut(&mut self, id: u32) -> Option<&mut [f32; 4]> {
        use water_ids as w;
        Some(match id {
            w::SPECULAR_COLOR => &mut self.specular_color,
            w::COLOR => &mut self.color,
            w::FOG_COLOR => &mut self.fog_color,
            w::SHORELINE_COLOR => &mut self.shoreline_color,
            _ => return None,
        })
    }

    /// Every float and colour component, for finiteness checks.
    fn components(&self) -> impl Iterator<Item = f32> + '_ {
        [
            self.wave_speed,
            self.wave_frequency,
            self.wave_amplitude,
            self.reflection_distortion,
            self.refraction_distortion,
            self.fresnel_bias,
            self.fresnel_exponent,
            self.specular_power,
            self.overbright,
            self.fake_specular_threshold,
            self.fog_amount,
        ]
        .into_iter()
        .chain(self.normal_speed.into_iter().flatten())
        .chain(self.normal_scale)
        .chain(self.specular_color)
        .chain(self.color)
        .chain(self.fog_color)
        .chain(self.shoreline_color)
    }
}

/// The program family of a water record (`water.md` §8.2 "Program families"), recognised
/// by [`water_family`].
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Default)]
pub enum WaterFamily {
    /// Plain water: three scrolling normal layers at world-position coordinates
    /// (`water.md` §8.3–§8.4).
    #[default]
    Plain,
    /// Plain water on mesh UV: the same drawing with the layers at the mesh's UV set 0
    /// (`water.md` §8.5.6).
    PlainUv,
    /// Older bump water: three layers from 1055 and 1056 on mesh UV (`water.md` §8.5.6).
    OlderBump,
    /// Flow-map water (`water.md` §8.6): 1057 and 1058 are cycle rates of a flow map.
    Flow {
        /// The record declares normal layers 1 and 2 (1059–1062, 1064, 1065), which the
        /// Tessera rule of §8.6.4 adds (0x2B0007FD); without them only steps 1–6 of §8.6.2
        /// apply (0x2B0005EC). It also picks the record's own colour list of the §8.6.4
        /// Tessera rule (revised 2026-10-10): this tells the install's two flow-map records
        /// apart (an observation of this install, not a format rule, §8.6.4 answer).
        normal_layers: bool,
    },
}

/// The shader records of plain water on mesh UV (`water.md` §8.2 table, §8.5.6). No rule in
/// the decoded record sets them apart from plain water (their difference lies inside the
/// programs); Tessera recognises them by these DIDs (Tessera Decision kept by the `water.md`
/// §14 answer (a) of 2026-10-10: an observation of this install, re-checked when the data
/// changes).
pub const PLAIN_UV_RECORDS: [u32; 3] = [0x2B00_084B, 0x2B00_089A, 0x2B00_0A8D];

/// The water family of `shader`, `None` unless it is a water record (`material.md` §11.6):
/// a record that declares 1056 is older bump water, one with a flow-map slot (3075) flow-map
/// water (with normal layers 1 and 2 when it declares 1059), one of [`PLAIN_UV_RECORDS`]
/// plain water on mesh UV, and every other water record plain water.
#[must_use]
pub fn water_family(shader: &ShaderInterface) -> Option<WaterFamily> {
    if shader.family != ProgramFamily::Water {
        return None;
    }
    Some(if shader.declares(water_ids::BUMP_SPEED) {
        WaterFamily::OlderBump
    } else if shader.slot(ids::FLOW).is_some() {
        WaterFamily::Flow {
            normal_layers: shader.declares(1059),
        }
    } else if PLAIN_UV_RECORDS.contains(&shader.did.0) {
        WaterFamily::PlainUv
    } else {
        WaterFamily::Plain
    })
}

/// The flow, foam and depth parameters of flow-map water (`water.md` §8.6.1), evaluated. An
/// undeclared value is 0.
#[derive(Debug, Clone, Copy, PartialEq, Default)]
pub struct FlowParams {
    /// 1182, 1183: foam lookup offset (U, V).
    pub foam_offset: [f32; 2],
    /// 1184, 1185: foam tiling (U, V).
    pub foam_tile: [f32; 2],
    /// 1186, 1187: offset of the flow map's third-channel lookup (U, V).
    pub depth_offset: [f32; 2],
    /// 1188: rotation of the phase coordinates in radians.
    pub rotation: f32,
    /// 1189: depth-based opacity offset (Direct3D 10/11 colour, not drawn by the Tessera rule).
    pub depth_opacity_offset: f32,
    /// 1190: foam alpha fade factor.
    pub foam_fade: f32,
    /// 1170: shoreline maximum depth.
    pub shoreline_max_depth: f32,
    /// 1171: fog minimum depth.
    pub fog_min_depth: f32,
    /// 1173: fog depth factor.
    pub fog_depth_factor: f32,
}

/// The parameters of older bump water (`water.md` §8.5.6), evaluated.
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct BumpParams {
    /// 1055 `c_WaterBumpTextureScale` (1 when undeclared).
    pub texture_scale: f32,
    /// 1056 `c_WaterBumpSpeed` (0 when undeclared).
    pub speed: f32,
    /// 2004 `c_WaterDeepColor` (black when undeclared).
    pub deep_color: [f32; 4],
    /// 2005 `c_WaterShallowColor` (white when undeclared).
    pub shallow_color: [f32; 4],
}

impl Default for BumpParams {
    fn default() -> Self {
        Self {
            texture_scale: 1.0,
            speed: 0.0,
            deep_color: BLACK,
            shallow_color: WHITE,
        }
    }
}

/// A water surface's material (`water.md` §8.2), evaluated at one time.
#[derive(Debug, Clone, PartialEq)]
pub struct WaterMaterial {
    /// The shader record.
    pub shader: Did,
    /// The record's program family ([`water_family`]; [`WaterFamily::Plain`] for a record
    /// that reads water parameters without being a water record).
    pub family: WaterFamily,
    /// The §8.2 parameters.
    pub params: WaterParams,
    /// Flow-map parameters (§8.6.1; zero outside the flow-map family).
    pub flow: FlowParams,
    /// Older bump water parameters (§8.5.6).
    pub bump: BumpParams,
    /// The noise texture of the flow-map programs: the default texture of a sampler no
    /// binding names (§8.6.1, [`ShaderInterface::unbound_defaults`]); the first one. In this
    /// install it is the one of the `NoiseTexture` sampler in every water permutation
    /// (`water.md` W47, §14 answer (b): equivalent).
    pub noise_map: Option<Did>,
    /// Some normal-layer scale (1063–1065) is a curve that changes with time
    /// (`water.md` §8.7.1 Tessera Decision: *B* from the camera's landblock).
    pub animated_scales: bool,
    /// Slot 3011: the normal texture of all three normal layers.
    pub normal_map: Option<Did>,
    /// Slot 3075: flow map (flow permutations; formulas OPEN, `water.md` §8.4).
    pub flow_map: Option<Did>,
    /// Slot 3079: foam map (formulas OPEN).
    pub foam_map: Option<Did>,
    /// Slots 3000 and 3001: the two colour layers of the low-detail permutations.
    pub color_layers: [Option<Did>; 2],
    /// The alpha of the material colour 2000: the output alpha of the water permutations
    /// (§8.4 step 7). The Tessera rule of §8.4 draws water with alpha 1 until the
    /// permutation choice is settled; this value is for a renderer that follows the
    /// original instead.
    pub material_alpha: f32,
    /// The §8.2 ids the shader record does not declare (their values are
    /// [`WaterParams::SPEC_DEFAULTS`]), ascending.
    pub undeclared: Vec<u32>,
    /// The §8.2 ids the shader record declares with another value type than the table's
    /// float or colour (never observed; their values are [`WaterParams::SPEC_DEFAULTS`]
    /// too), ascending.
    pub mistyped: Vec<u32>,
}

impl WaterMaterial {
    /// Whether `shader` declares a parameter of the §8.2 table, i.e. whether its programs
    /// read water parameters. True of the six water shader records of §8.2, and also of a
    /// few non-water records that read water parameters (for example the window family,
    /// `material.md` §20.1): whether a surface is drawn as water is decided by where it
    /// comes from (a water table, `water.md` §4, or a placed water mesh, §8.5).
    #[must_use]
    pub fn reads_water_params(shader: &ShaderInterface) -> bool {
        water_ids::FLOATS
            .iter()
            .chain(&water_ids::COLORS)
            .any(|&id| shader.declares(id))
    }

    /// The water material of `material` at time `t` seconds (`material.md` §5.3.1) with
    /// the instance's curve states, or `None` when its shader reads no water parameter
    /// ([`WaterMaterial::reads_water_params`]). Every float is finite.
    pub fn evaluate(material: &ResolvedMaterial, t: f64, states: &mut CurveStates) -> Option<Self> {
        if !Self::reads_water_params(&material.shader) {
            return None;
        }
        let p = &material.params;
        let mut params = WaterParams::SPEC_DEFAULTS;
        let (mut undeclared, mut mistyped) = (Vec::new(), Vec::new());
        let mut missing = |id: u32| {
            if material.shader.declares(id) {
                mistyped.push(id);
            } else {
                undeclared.push(id);
            }
        };
        for id in water_ids::FLOATS {
            match p.float_at(id, t, states) {
                Some(v) => {
                    if let Some(slot) = params.float_mut(id) {
                        *slot = v;
                    }
                }
                None => missing(id),
            }
        }
        for id in water_ids::COLORS {
            match p.color(id) {
                Some(c) => {
                    if let Some(slot) = params.color_mut(id) {
                        *slot = c.map(curve_finite);
                    }
                }
                None => missing(id),
            }
        }
        undeclared.sort_unstable();
        mistyped.sort_unstable();
        let float =
            |id: u32, d: f32, states: &mut CurveStates| p.float_at(id, t, states).unwrap_or(d);
        let flow = FlowParams {
            foam_offset: water_ids::FOAM_OFFSET.map(|id| float(id, 0.0, states)),
            foam_tile: water_ids::FOAM_TILE.map(|id| float(id, 0.0, states)),
            depth_offset: water_ids::DEPTH_OFFSET.map(|id| float(id, 0.0, states)),
            rotation: float(water_ids::NORMAL_WAVE_ROTATION, 0.0, states),
            depth_opacity_offset: float(water_ids::DEPTH_OPACITY_OFFSET, 0.0, states),
            foam_fade: float(water_ids::FOAM_ALPHA_FADE, 0.0, states),
            shoreline_max_depth: float(water_ids::SHORELINE_MAX_DEPTH, 0.0, states),
            fog_min_depth: float(water_ids::FOG_MIN_DEPTH, 0.0, states),
            fog_depth_factor: float(water_ids::FOG_DEPTH_FACTOR, 0.0, states),
        };
        let colour = |id: u32, d: [f32; 4]| p.color(id).map_or(d, |c| c.map(curve_finite));
        let bump = BumpParams {
            texture_scale: float(water_ids::BUMP_TEXTURE_SCALE, 1.0, states),
            speed: float(water_ids::BUMP_SPEED, 0.0, states),
            deep_color: colour(water_ids::DEEP_COLOR, BLACK),
            shallow_color: colour(water_ids::SHALLOW_COLOR, WHITE),
        };
        let animated_scales = water_ids::NORMAL_SCALE
            .iter()
            .any(|&id| p.curve(id).is_some_and(crate::FloatCurve::is_animated));
        Some(Self {
            shader: material.shader.did,
            family: water_family(&material.shader).unwrap_or_default(),
            params,
            flow,
            bump,
            noise_map: material.shader.unbound_defaults.first().copied(),
            animated_scales,
            normal_map: p.texture(ids::NORMAL),
            flow_map: p.texture(ids::FLOW),
            foam_map: p.texture(ids::FOAM),
            color_layers: [p.texture(ids::DIFFUSE), p.texture(ids::SECOND_LAYER)],
            material_alpha: curve_finite(material.material_alpha()),
            undeclared,
            mistyped,
        })
    }

    /// [`WaterMaterial::evaluate`] at time 0 with fresh states.
    #[must_use]
    pub fn at_rest(material: &ResolvedMaterial) -> Option<Self> {
        Self::evaluate(material, 0.0, &mut CurveStates::new())
    }

    /// Whether every float is finite (always true; for tests and assertions).
    #[must_use]
    pub fn all_finite(&self) -> bool {
        let f = &self.flow;
        let b = &self.bump;
        self.material_alpha.is_finite()
            && self.params.components().all(f32::is_finite)
            && f.foam_offset
                .iter()
                .chain(&f.foam_tile)
                .chain(&f.depth_offset)
                .chain(&[
                    f.rotation,
                    f.depth_opacity_offset,
                    f.foam_fade,
                    f.shoreline_max_depth,
                    f.fog_min_depth,
                    f.fog_depth_factor,
                    b.texture_scale,
                    b.speed,
                ])
                .chain(&b.deep_color)
                .chain(&b.shallow_color)
                .all(|v| v.is_finite())
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn id_tables_agree() {
        let mut p = WaterParams::SPEC_DEFAULTS;
        for id in water_ids::FLOATS {
            assert!(p.float_mut(id).is_some(), "{id}");
            assert!(water_ids::is_water_family(id), "{id}");
        }
        for id in water_ids::COLORS {
            assert!(p.color_mut(id).is_some(), "{id}");
            assert!(water_ids::is_water_family(id), "{id}");
        }
        assert!(p.float_mut(1049).is_none());
        assert!(p.color_mut(2000).is_none());
        assert!(!water_ids::is_water_family(1070));
        assert!(!water_ids::is_water_family(1172));
        assert!(water_ids::is_water_family(1173));
        assert_eq!(p.components().count(), 11 + 6 + 3 + 16);
    }
}
