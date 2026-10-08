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

use tessera_formats::Did;

use crate::interface::ShaderInterface;
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

/// A water surface's material (`water.md` §8.2), evaluated at one time.
#[derive(Debug, Clone, PartialEq)]
pub struct WaterMaterial {
    /// The shader record.
    pub shader: Did,
    /// The §8.2 parameters.
    pub params: WaterParams,
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
        Some(Self {
            shader: material.shader.did,
            params,
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
        self.material_alpha.is_finite() && self.params.components().all(f32::is_finite)
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
