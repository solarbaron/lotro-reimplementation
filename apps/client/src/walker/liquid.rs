// SPDX-License-Identifier: GPL-3.0-or-later
//! Surfaces drawn by programs of their own, and the per-frame evaluation of animated
//! materials in the walker.
//!
//! - The records of `material.md` §11.6 (water, mirror, liquid-distortion and lava
//!   records) are drawn with the renderer's water pipeline, never through the generic path
//!   (`water.md` §8.5 rule; `walker.md` §14.5–§14.6 OPENs of 2026-10-09). This module turns
//!   their evaluated materials ([`LiquidSurface`]) into the renderer's parameters:
//!   [`water_params`] by program family (plain water §8.3–§8.4, plain water on mesh UV and
//!   older bump water §8.5.6, flow-map water §8.6, lava §8.6.5), [`mirror_params`]
//!   (§8.5.4) and [`distortion_params`] (§8.5.5).
//! - Every curve-valued parameter and every texture matrix is evaluated at the global clock
//!   for every frame drawn (`water.md` §8.7, §8.7.1; `material.md` §5.3.1, §8.4), never
//!   once at load. [`AnimClock`] does it once per surface and frame and keeps each
//!   surface's curve states between frames.

use std::collections::HashMap;
use std::sync::Arc;

use tessera_material::{
    CurveStates, DistortionMaterial, LavaMaterial, LiquidSurface, MirrorMaterial, ResolvedMaterial,
    TextureMatrixLayout, WaterFamily as MaterialFamily, WaterMaterial,
};
use tessera_render::{
    DistortionParams, LiquidShading, MirrorParams, TextureId, UvTransform, WaterFamily,
    WaterFlowParams, WaterParams, WaterTextures,
};

use super::geometry::{OWN_TEXTURES, texture_matrices_at};

/// What a surface's per-frame evaluation needs: its resolved material and the
/// texture-matrix bindings of its shader record.
#[derive(Debug, Clone)]
pub struct SurfaceAnim {
    /// The resolved material.
    pub material: Arc<ResolvedMaterial>,
    /// The texture-matrix bindings of its shader record (`material.md` §7.3, §8.4).
    pub layout: Arc<TextureMatrixLayout>,
}

impl SurfaceAnim {
    /// The own-program material at clock `t`: by the record's family (`material.md`
    /// §11.6), else, for a record of the generic path that reads water parameters (none in
    /// the landblock-water tables of the reference install), plain water; `None` otherwise.
    #[must_use]
    pub fn liquid(&self, t: f64, states: &mut CurveStates) -> Option<LiquidSurface> {
        LiquidSurface::evaluate(&self.material, &self.layout, t, states).or_else(|| {
            WaterMaterial::evaluate(&self.material, t, states).map(LiquidSurface::Water)
        })
    }

    /// Texture matrices 0 and 1 at clock `t`.
    #[must_use]
    pub fn uv(&self, t: f64, states: &mut CurveStates) -> [UvTransform; 2] {
        texture_matrices_at(&self.material, &self.layout, t, states)
    }

    fn key(&self) -> usize {
        Arc::as_ptr(&self.material) as usize
    }
}

/// The renderer's drawing of an own-program surface at one time: the shading and whether
/// a normal-layer scale is a curve (`water.md` §8.7.1 Tessera rule: *B* is then measured
/// from the camera's landblock).
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct LiquidFrame {
    /// The shading; for water records `block_offset` is (0, 0) and set by the caller.
    pub shading: LiquidShading,
    /// Some normal-layer scale changes with time.
    pub animated_scales: bool,
}

/// Frames a surface's curve states are kept without being used.
const KEEP_FRAMES: u64 = 600;

/// Most surfaces whose curve states are kept; beyond it states start fresh each frame.
const MAX_STATES: usize = 1 << 14;

/// Per-frame evaluation of animated surfaces (module docs).
#[derive(Debug, Default)]
pub struct AnimClock {
    frame: u64,
    t: f64,
    states: HashMap<usize, (u64, CurveStates)>,
    uv: HashMap<usize, [UvTransform; 2]>,
    liquid: HashMap<usize, Option<LiquidFrame>>,
}

impl AnimClock {
    /// Starts a frame at clock `t` seconds (`material.md` §5.3.1; non-finite reads as 0).
    pub fn begin(&mut self, t: f64) {
        self.frame = self.frame.wrapping_add(1);
        self.t = if t.is_finite() { t } else { 0.0 };
        self.uv.clear();
        self.liquid.clear();
        let frame = self.frame;
        if self.states.len() > MAX_STATES / 2 {
            self.states
                .retain(|_, (used, _)| frame.wrapping_sub(*used) <= KEEP_FRAMES);
        }
    }

    /// The clock of the frame.
    #[must_use]
    pub fn time(&self) -> f64 {
        self.t
    }

    fn with_states<R>(&mut self, key: usize, f: impl FnOnce(&mut CurveStates) -> R) -> R {
        let frame = self.frame;
        if self.states.len() >= MAX_STATES && !self.states.contains_key(&key) {
            return f(&mut CurveStates::new());
        }
        let e = self
            .states
            .entry(key)
            .or_insert_with(|| (frame, CurveStates::new()));
        e.0 = frame;
        f(&mut e.1)
    }

    /// Texture matrices 0 and 1 of `anim` this frame.
    pub fn uv(&mut self, anim: &SurfaceAnim) -> [UvTransform; 2] {
        let key = anim.key();
        if let Some(uv) = self.uv.get(&key) {
            return *uv;
        }
        let t = self.t;
        let uv = self.with_states(key, |s| anim.uv(t, s));
        self.uv.insert(key, uv);
        uv
    }

    /// The own-program drawing of `anim` this frame (`None`: not an own-program record).
    pub fn liquid(&mut self, anim: &SurfaceAnim) -> Option<LiquidFrame> {
        let key = anim.key();
        if let Some(l) = self.liquid.get(&key) {
            return *l;
        }
        let t = self.t;
        let l = self.with_states(key, |s| anim.liquid(t, s).map(|l| liquid_frame(&l)));
        self.liquid.insert(key, l);
        l
    }
}

fn rgb(c: [f32; 4]) -> [f32; 3] {
    [c[0], c[1], c[2]]
}

/// The render parameters of a water material (`water.md` §8.2), by its program family:
///
/// - plain water and plain water on mesh UV: the §8.2 parameters, three normal layers;
/// - older bump water: 1055 and 1056 for the layer coordinates of §8.5.6, drawn by §8.4 with
///   `WaterColor` white and the refraction tinted by `c_WaterDeepColor` (Tessera rule there);
/// - flow-map water: 1057 and 1058 as the phase rates and 1063 as the scale (layer 0), and
///   layers 1 and 2 only for the records that have them (§8.6.2, §8.6.4 Tessera rule).
#[must_use]
pub fn water_params(m: &WaterMaterial) -> WaterParams {
    let p = &m.params;
    let (family, layers) = match m.family {
        MaterialFamily::Plain => (WaterFamily::Plain, 3),
        MaterialFamily::PlainUv => (WaterFamily::PlainUv, 3),
        MaterialFamily::OlderBump => (WaterFamily::OlderBump, 3),
        MaterialFamily::Flow { normal_layers } => {
            (WaterFamily::Flow, if normal_layers { 3 } else { 1 })
        }
    };
    let bump = family == WaterFamily::OlderBump;
    let f = &m.flow;
    WaterParams {
        family,
        wave_speed: p.wave_speed,
        wave_frequency: p.wave_frequency,
        // §8.2 answered callout: the flow-map records have no waves in any program.
        wave_amplitude: if family == WaterFamily::Flow {
            0.0
        } else {
            p.wave_amplitude
        },
        normal_speed: p.normal_speed,
        normal_scale: p.normal_scale,
        normal_layers: layers,
        reflection_distortion: p.reflection_distortion,
        refraction_distortion: p.refraction_distortion,
        fresnel_bias: p.fresnel_bias,
        fresnel_exponent: p.fresnel_exponent,
        specular_power: p.specular_power,
        overbright: p.overbright,
        fake_specular_threshold: p.fake_specular_threshold,
        fog_amount: p.fog_amount,
        specular_color: rgb(p.specular_color),
        water_color: if bump { [1.0; 3] } else { rgb(p.color) },
        fog_color: if bump {
            rgb(m.bump.deep_color)
        } else {
            rgb(p.fog_color)
        },
        shoreline_color: rgb(p.shoreline_color),
        flow: WaterFlowParams {
            foam_offset: f.foam_offset,
            foam_tile: f.foam_tile,
            depth_offset: f.depth_offset,
            rotation: f.rotation,
        },
        bump: [m.bump.texture_scale, m.bump.speed],
    }
}

/// The render parameters of a lava material (`water.md` §8.6.5): the phase rates (1057,
/// 1058) and scale (1063) in layer 0, the brightness 1066 as the overbright.
#[must_use]
pub fn lava_params(l: &LavaMaterial) -> WaterParams {
    let d = WaterParams::default();
    WaterParams {
        family: WaterFamily::Lava,
        wave_amplitude: 0.0,
        normal_speed: [l.rates, [0.0; 2], [0.0; 2]],
        normal_scale: [l.scale, 1.0, 1.0],
        normal_layers: 1,
        overbright: l.brightness,
        flow: WaterFlowParams {
            foam_offset: l.foam_offset,
            foam_tile: l.foam_tile,
            depth_offset: l.depth_offset,
            rotation: l.rotation,
        },
        ..d
    }
}

/// The render parameters of a mirror material (`water.md` §8.5.4).
#[must_use]
pub fn mirror_params(m: &MirrorMaterial) -> MirrorParams {
    MirrorParams {
        reflection_distortion: m.reflection_distortion,
        fresnel_exponent: m.fresnel_exponent,
        fresnel_bias: m.fresnel_bias,
        normal_scale: m.normal_scale,
        normal_speed: m.normal_speed,
        fresnel_bump: m.fresnel_bump,
        color: m.color,
    }
}

/// The render parameters of a liquid-distortion material (`water.md` §8.5.5).
#[must_use]
pub fn distortion_params(d: &DistortionMaterial) -> DistortionParams {
    DistortionParams {
        lit: d.lit,
        world_coordinates: d.world_coordinates,
        waves: d.waves,
        distortion: d.distortion,
        speed: d.speed,
        scale: d.scale,
        overbright: d.overbright,
        matrix: UvTransform {
            rows: d.matrix.rows,
        }
        .sanitized(),
        color: d.color,
        alpha_test: d.alpha_test,
        two_sided: d.two_sided,
        depth_write: d.depth_write,
    }
}

/// The drawing of an evaluated own-program material.
#[must_use]
pub fn liquid_frame(s: &LiquidSurface) -> LiquidFrame {
    match s {
        LiquidSurface::Water(w) => LiquidFrame {
            shading: LiquidShading::Water {
                params: water_params(w),
                block_offset: [0.0; 2],
            },
            animated_scales: w.animated_scales,
        },
        LiquidSurface::Lava(l) => LiquidFrame {
            shading: LiquidShading::Water {
                params: lava_params(l),
                block_offset: [0.0; 2],
            },
            animated_scales: false,
        },
        LiquidSurface::Mirror(m) => LiquidFrame {
            shading: LiquidShading::Mirror(mirror_params(m)),
            animated_scales: false,
        },
        LiquidSurface::Distortion(d) => LiquidFrame {
            shading: LiquidShading::Distortion(distortion_params(d)),
            animated_scales: false,
        },
    }
}

/// The renderer's textures of an own-program drawing from the resolved textures in
/// [`OWN_TEXTURES`] order: the normal (or distortion) map and the others.
#[must_use]
pub fn draw_textures(ids: [Option<TextureId>; OWN_TEXTURES]) -> (Option<TextureId>, WaterTextures) {
    let [normal, flow, foam, noise, c0, c1, emissive] = ids;
    (
        normal,
        WaterTextures {
            flow,
            foam,
            noise,
            color: [c0, c1],
            emissive,
        },
    )
}

/// The horizontal offset *O* of a water drawing in the render frame (`water.md` §8.3):
/// the region origin (`region_offset`, so *B* is the region position in landblock units,
/// §8.3 Tessera rule), or for a surface with curve-valued scales the south-west corner of
/// the landblock that contains `camera` (§8.7.1 Tessera rule).
#[must_use]
pub fn block_offset(animated_scales: bool, region_offset: [f32; 2], camera: [f32; 2]) -> [f32; 2] {
    if !animated_scales {
        return region_offset;
    }
    let corner = |c: f32| {
        if c.is_finite() {
            (c / 160.0).floor() * 160.0
        } else {
            0.0
        }
    };
    [corner(camera[0]), corner(camera[1])]
}

/// `shading` with its water offset set to `offset` and, for the blended drawings, the
/// material alpha multiplied by the draw-distance fade `fade`.
#[must_use]
pub fn placed(shading: LiquidShading, offset: [f32; 2], fade: f32) -> LiquidShading {
    let fade = if fade.is_finite() {
        fade.clamp(0.0, 1.0)
    } else {
        1.0
    };
    match shading {
        LiquidShading::Water { params, .. } => LiquidShading::Water {
            params,
            block_offset: offset,
        },
        LiquidShading::Mirror(mut m) => {
            m.color[3] *= fade;
            LiquidShading::Mirror(m)
        }
        LiquidShading::Distortion(mut d) => {
            d.color[3] *= fade;
            LiquidShading::Distortion(d)
        }
    }
}

/// Synthetic own-program materials for the walker's unit tests (records built in memory,
/// no game data).
#[cfg(test)]
pub(crate) mod tests_support {
    use std::sync::Arc;

    use tessera_formats::Did;
    use tessera_material::records::{
        Binding, ParamEntry, ParamValue, Permutation, RenderState, Sampler, SamplerState,
    };
    use tessera_material::{
        FloatCurve, ResolvedMaterial, ShaderInterface, ShaderRecord, TextureMatrixLayout,
    };

    use super::SurfaceAnim;

    fn float(id: u32, v: f32) -> ParamEntry {
        ParamEntry {
            id,
            group: 1000,
            value: ParamValue::Float(FloatCurve::constant(v)),
            flag: 1,
            bindings: Vec::new(),
        }
    }

    fn texture(id: u32, did: u32, slot: u32) -> ParamEntry {
        ParamEntry {
            id,
            group: 3000,
            value: ParamValue::Texture(Did(did)),
            flag: 0,
            bindings: vec![Binding {
                semantic: 10,
                group: 3000,
                permutation: 0,
                slot,
            }],
        }
    }

    fn sampler(name: &str, default: u32) -> Sampler {
        let mut raw = [0u8; tessera_material::records::SAMPLER_STATE_SIZE];
        raw[..4].copy_from_slice(&default.to_le_bytes());
        Sampler {
            name: name.into(),
            state: SamplerState { raw },
        }
    }

    /// A water record with a frame-buffer sampler and normal map `normal` (slot 3011):
    /// plain water (0x2B0000A8-like), or with `flow` a flow-map record with normal layers
    /// 1 and 2, a flow map 0x41000002 (slot 3075) and an unbound noise sampler whose default
    /// is 0x41000003 (`water.md` §8.6.1).
    pub(crate) fn water_anim(normal: Option<u32>, flow: bool) -> SurfaceAnim {
        let mut params = vec![float(1057, 0.2), float(1058, 0.2), float(1063, 6.0)];
        let mut samplers = vec![sampler("FrameBufferTexture", 0), sampler("NormalMap", 0)];
        if let Some(n) = normal {
            params.push(texture(3011, n, 1));
        }
        if flow {
            params.extend([1059, 1060, 1061, 1062].map(|id| float(id, 0.01)));
            params.extend([float(1064, 1.0), float(1065, 12.0)]);
            params.push(texture(3075, 0x4100_0002, 2));
            samplers.push(sampler("FlowTexture", 0));
            samplers.push(sampler("NoiseTexture", 0x4100_0003));
        }
        let record = ShaderRecord {
            did: Did(if flow { 0x2B00_07FD } else { 0x2B00_00A8 }),
            reserved: 0,
            params,
            permutations: vec![Permutation {
                key: [0; tessera_material::records::PERMUTATION_KEY_WORDS],
                programs: None,
                samplers,
                extra_items: Vec::new(),
                render_state: RenderState {
                    raw: [0; tessera_material::records::RENDER_STATE_SIZE],
                },
            }],
        };
        anim_of(&record)
    }

    fn anim_of(record: &ShaderRecord) -> SurfaceAnim {
        let layout = Arc::new(TextureMatrixLayout::from_record(record));
        let shader = Arc::new(ShaderInterface::from_record(record));
        SurfaceAnim {
            material: Arc::new(ResolvedMaterial::resolve(shader, &[])),
            layout,
        }
    }

    /// A lit two-layer record `did` (`ColorAndOpacity` on 3000, `ColorAndOpacity2` on 3001,
    /// `material.md` §10.3) whose texture matrix 1 translates V at `rate` repeats per
    /// second (a mode-2 curve; 0 gives a constant) and whose matrix 0 scales U by 2.
    pub(crate) fn two_layer(did: u32, rate: f32) -> SurfaceAnim {
        let matrix = |id: u32, semantic: u16, slot: u32, value: FloatCurve| ParamEntry {
            id,
            group: 1000,
            value: ParamValue::Float(value),
            flag: 0,
            bindings: vec![Binding {
                semantic,
                group: 1000,
                permutation: 0,
                slot,
            }],
        };
        let mut ramp = FloatCurve::constant(0.0);
        if rate != 0.0 {
            ramp.mode = 2;
            ramp.params = [0.0; 11];
            ramp.params[6] = rate;
            ramp.param_count = 10;
        }
        let one = FloatCurve::constant(1.0);
        let zero = FloatCurve::constant(0.0);
        let mut params = vec![texture(3000, 0x4100_0060, 0), texture(3001, 0x4100_0061, 1)];
        // Matrix 0: scale (2, 1); matrix 1: V translation by the ramp (semantics 38–44).
        for (slot, values) in [
            (
                0u32,
                [
                    zero.clone(),
                    zero.clone(),
                    zero.clone(),
                    FloatCurve::constant(2.0),
                    one.clone(),
                    zero.clone(),
                    zero.clone(),
                ],
            ),
            (
                1,
                [
                    zero.clone(),
                    ramp,
                    zero.clone(),
                    one.clone(),
                    one,
                    zero.clone(),
                    zero,
                ],
            ),
        ] {
            for (k, v) in values.into_iter().enumerate() {
                let set = if slot == 0 {
                    [1017, 1018, 1033, 1001, 1002, 1070, 1071]
                } else {
                    [1019, 1020, 1034, 1003, 1004, 1072, 1073]
                };
                params.push(matrix(set[k], 38 + k as u16, slot, v));
            }
        }
        let mut lit_state = [0u8; tessera_material::records::RENDER_STATE_SIZE];
        lit_state[..7].copy_from_slice(&[5, 6, 1, 7, 1, 2, 0]);
        let record = ShaderRecord {
            did: Did(did),
            reserved: 0,
            params,
            permutations: vec![Permutation {
                key: [0; tessera_material::records::PERMUTATION_KEY_WORDS],
                programs: None,
                samplers: vec![
                    sampler("ColorAndOpacity", 0),
                    sampler("ColorAndOpacity2", 0),
                ],
                extra_items: Vec::new(),
                render_state: RenderState { raw: lit_state },
            }],
        };
        anim_of(&record)
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use tessera_formats::Did;

    fn water(family: MaterialFamily) -> WaterMaterial {
        WaterMaterial {
            shader: Did(0x2B00_00A8),
            family,
            params: tessera_material::WaterParams::SPEC_DEFAULTS,
            flow: tessera_material::FlowParams::default(),
            bump: tessera_material::BumpParams::default(),
            noise_map: None,
            animated_scales: false,
            normal_map: None,
            flow_map: None,
            foam_map: None,
            color_layers: [None; 2],
            material_alpha: 1.0,
            undeclared: Vec::new(),
            mistyped: Vec::new(),
        }
    }

    /// The record defaults equal the renderer's defaults for plain water (§8.2).
    #[test]
    fn plain_water_maps_to_the_renderer_defaults() {
        assert_eq!(
            water_params(&water(MaterialFamily::Plain)),
            WaterParams::default()
        );
    }

    /// §8.6: flow-map records are drawn as flow water, with layers 1 and 2 only where the
    /// record has them, and never with waves; the phase rates sit in layer 0.
    #[test]
    fn flow_records_keep_their_family_and_layers() {
        let mut m = water(MaterialFamily::Flow {
            normal_layers: true,
        });
        m.params.normal_speed[0] = [1.0, 1.0];
        m.params.wave_amplitude = 2.0;
        let p = water_params(&m);
        assert_eq!(p.family, WaterFamily::Flow);
        assert_eq!(p.normal_layers, 3);
        assert_eq!(p.normal_speed[0], [1.0, 1.0]);
        assert_eq!(p.wave_amplitude, 0.0);
        let one = water_params(&water(MaterialFamily::Flow {
            normal_layers: false,
        }));
        assert_eq!(one.normal_layers, 1);
    }

    /// §8.5.6 Tessera rule: older bump water is drawn white with the deep colour as the
    /// refraction tint, with 1055 and 1056 for the coordinates.
    #[test]
    fn older_bump_water_uses_its_deep_colour() {
        let mut m = water(MaterialFamily::OlderBump);
        m.bump.deep_color = [0.1, 0.2, 0.3, 1.0];
        m.bump.texture_scale = 0.5;
        m.bump.speed = 0.003;
        m.params.color = [0.5; 4];
        let p = water_params(&m);
        assert_eq!(p.family, WaterFamily::OlderBump);
        assert_eq!(p.water_color, [1.0; 3]);
        assert_eq!(p.fog_color, [0.1, 0.2, 0.3]);
        assert_eq!(p.bump, [0.5, 0.003]);
    }

    /// §8.7.1 Tessera rule: curve-valued scales measure *B* from the camera's landblock.
    #[test]
    fn block_offset_follows_the_camera_landblock_for_animated_scales() {
        assert_eq!(
            block_offset(false, [-320.0, -480.0], [10.0, 20.0]),
            [-320.0, -480.0]
        );
        assert_eq!(
            block_offset(true, [-320.0, -480.0], [170.0, -5.0]),
            [160.0, -160.0]
        );
        assert_eq!(block_offset(true, [0.0; 2], [f32::NAN, 0.0]), [0.0, 0.0]);
    }

    /// The fade scales the blended drawings' material alpha only; the water offset is set.
    #[test]
    fn placed_sets_the_offset_and_fades_blended_drawings() {
        let w = placed(
            LiquidShading::Water {
                params: WaterParams::default(),
                block_offset: [0.0; 2],
            },
            [1.0, 2.0],
            0.5,
        );
        assert!(matches!(
            w,
            LiquidShading::Water {
                block_offset: [1.0, 2.0],
                ..
            }
        ));
        let LiquidShading::Mirror(m) = placed(
            LiquidShading::Mirror(MirrorParams::default()),
            [0.0; 2],
            0.25,
        ) else {
            panic!("mirror expected");
        };
        assert_eq!(m.color[3], 0.25);
        let LiquidShading::Distortion(d) = placed(
            LiquidShading::Distortion(DistortionParams::default()),
            [0.0; 2],
            f32::NAN,
        ) else {
            panic!("distortion expected");
        };
        assert_eq!(d.color[3], 1.0);
    }

    /// The clock keeps one evaluation per surface and frame, and a fresh frame
    /// re-evaluates at its own time (`material.md` §5.3.1, §8.4).
    #[test]
    fn clock_evaluates_matrices_per_frame() {
        let mut c = AnimClock::default();
        c.begin(5.0);
        assert_eq!(c.time(), 5.0);
        let anim = tests_support::two_layer(0x2B00_07A1, 0.5);
        let uv = c.uv(&anim);
        assert!((uv[1].rows[1][2] - 2.5).abs() < 1e-5, "{uv:?}");
        assert!((uv[0].rows[0][0] - 2.0).abs() < 1e-6, "{uv:?}");
        c.begin(6.0);
        let uv = c.uv(&anim);
        assert!((uv[1].rows[1][2] - 3.0).abs() < 1e-5, "{uv:?}");
        c.begin(f64::NAN);
        assert_eq!(c.time(), 0.0);
    }

    /// `water.md` §8.5 rule, `material.md` §11.6: a water record's plan is drawn by its own
    /// program with the textures of its drawing (normal, flow and noise maps), never
    /// instanced and never through the generic path.
    #[test]
    fn own_program_records_are_routed() {
        use crate::walker::geometry::{TexRequest, VARIANT_OWN_ALPHA, plan_material_with};
        let anim = tests_support::water_anim(Some(0x4100_0001), true);
        let plan = plan_material_with(&anim.material, Some(&anim.layout));
        let own = plan.own.expect("routed");
        assert_eq!(own.kind, tessera_material::MaterialKind::Water);
        let req = |did| {
            Some(TexRequest {
                did,
                variant: VARIANT_OWN_ALPHA,
            })
        };
        assert_eq!(own.textures[0], req(0x4100_0001));
        assert_eq!(own.textures[1], req(0x4100_0002));
        assert_eq!(own.textures[3], req(0x4100_0003));
        assert!(plan.diag.routed && !plan.diag.scrolls && !plan.diag.second_layer);
        assert!(!plan.instanceable());
        assert!(plan.textures().any(|t| t.did == 0x4100_0003));
        let mut clock = AnimClock::default();
        clock.begin(2.0);
        let lf = clock.liquid(&anim).expect("water drawing");
        let LiquidShading::Water { params, .. } = lf.shading else {
            panic!("water shading expected");
        };
        assert_eq!(params.family, WaterFamily::Flow);
        assert_eq!(params.normal_layers, 3);
    }

    /// `material.md` §10.3, §8.4: the second colour layer with the record's combine rule,
    /// and the texture matrices at clock 0 with their animation flagged; without the
    /// record's bindings both stay out and the diagnostic says so.
    #[test]
    fn second_layers_and_matrices_are_planned() {
        use crate::walker::geometry::{plan_material, plan_material_with};
        let adds = tests_support::two_layer(0x2B00_0817, 0.5);
        let plan = plan_material_with(&adds.material, Some(&adds.layout));
        let (layer, add) = plan.layer2.expect("second layer");
        assert_eq!(layer.did, 0x4100_0061);
        assert!(add, "0x2B000817 adds its layers");
        assert!(plan.animated_uv);
        assert!((plan.uv[0].rows[0][0] - 2.0).abs() < 1e-6);
        assert!(plan.own.is_none() && !plan.diag.scrolls && !plan.diag.second_layer);
        assert!(plan.textures().any(|t| t.did == 0x4100_0061));
        let multiplies = tests_support::two_layer(0x2B00_07A1, 0.0);
        let plan = plan_material_with(&multiplies.material, Some(&multiplies.layout));
        assert_eq!(plan.layer2.map(|l| l.1), Some(false));
        assert!(!plan.animated_uv);
        let without = plan_material(&adds.material);
        assert!(without.layer2.is_none() && without.diag.second_layer);
        // A record the §10.3 rule does not cover keeps one layer, counted as left out.
        let uncovered = tests_support::two_layer(0x2B00_0A0A, 0.0);
        let plan = plan_material_with(&uncovered.material, Some(&uncovered.layout));
        assert!(plan.layer2.is_none() && plan.diag.second_layer);
    }
}
