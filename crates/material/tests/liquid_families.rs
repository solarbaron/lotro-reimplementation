// SPDX-License-Identifier: GPL-3.0-or-later
//! Synthetic tests of the records with programs of their own (`material.md` §11.6) and of
//! the two-colour-layer rule (`material.md` §10.3): recognition from the interface, the
//! water program families (`water.md` §8.2), the flow-map, lava, mirror and
//! liquid-distortion materials (§8.5.4–§8.6.5) and curve-valued parameters evaluated at
//! the clock (§8.7.1, WA13). Every fixture is generated here.

use std::collections::BTreeMap;

use tessera_formats::Did;
use tessera_material::interface::ProgramFamily;
use tessera_material::layers::{LayerCombine, combine_of, second_layer};
use tessera_material::records::{VALUE_TYPE_COLOR, VALUE_TYPE_CURVE, VALUE_TYPE_WORD};
use tessera_material::{
    CurveStates, FnSource, LiquidSurface, MaterialKind, MaterialLibrary, WaterFamily, water_family,
};

#[derive(Default, Clone)]
struct W(Vec<u8>);

impl W {
    fn u8(mut self, v: u8) -> Self {
        self.0.push(v);
        self
    }
    fn u16(mut self, v: u16) -> Self {
        self.0.extend_from_slice(&v.to_le_bytes());
        self
    }
    fn u32(mut self, v: u32) -> Self {
        self.0.extend_from_slice(&v.to_le_bytes());
        self
    }
    fn f32(mut self, v: f32) -> Self {
        self.0.extend_from_slice(&v.to_le_bytes());
        self
    }
    fn bytes(mut self, b: &[u8]) -> Self {
        self.0.extend_from_slice(b);
        self
    }
}

enum V {
    Word(u32),
    Color([f32; 4]),
    Const(f32),
    /// Mode and the ten curve parameters `w[0..9]` (`material.md` §5.3).
    Wave(u32, [f32; 10]),
}

/// One parameter entry (`material.md` §5.1) with bindings (semantic, permutation, slot).
fn entry(id: u32, v: V, bindings: &[(u16, u32, u32)], flag: u8) -> Vec<u8> {
    let group = (id / 1000 * 1000) as u16;
    let mut w = W::default().u32(id).u16(group);
    w = match v {
        V::Word(x) => w.u32(VALUE_TYPE_WORD).u32(x),
        V::Color(c) => c.iter().fold(w.u32(VALUE_TYPE_COLOR), |w, &x| w.f32(x)),
        V::Const(x) => w.u32(VALUE_TYPE_CURVE).u32(1).f32(x),
        V::Wave(mode, p) => p
            .iter()
            .fold(w.u32(VALUE_TYPE_CURVE).u32(mode), |w, &x| w.f32(x)),
    };
    w = w.u32(bindings.len() as u32).u8(flag);
    for &(semantic, perm, slot) in bindings {
        w = w.u16(semantic).u16(group).u32(perm).u32(slot);
    }
    w.0
}

fn float(id: u32, v: f32) -> Vec<u8> {
    entry(id, V::Const(v), &[], 1)
}

/// A texture slot bound to sampler `slot` of permutation 0.
fn texture(id: u32, did: u32, slot: u32) -> Vec<u8> {
    entry(id, V::Word(did), &[(10, 0, slot)], 0)
}

fn surface_record(did: u32, shader: u32, materials: &[u32]) -> Vec<u8> {
    let w = W::default()
        .u32(did)
        .u32(shader)
        .u32(1)
        .u32(materials.len() as u32);
    materials.iter().fold(w, |w, &m| w.u32(m)).u16(1).0
}

fn material(did: u32, entries: &[Vec<u8>]) -> Vec<u8> {
    let w = W::default().u32(did).u32(entries.len() as u32);
    entries.iter().fold(w, |w, e| w.bytes(e)).0
}

/// One permutation without programs: samplers by (name, default texture), an opaque
/// render state.
fn permutation(samplers: &[(&str, u32)]) -> Vec<u8> {
    let mut w = W::default().u32(1).u32(2).u32(3).u32(4).u32(0);
    w = w.u32(samplers.len() as u32);
    for (name, default) in samplers {
        let mut s = [0u8; 40];
        s[..4].copy_from_slice(&default.to_le_bytes());
        s[8] = 1;
        s[9] = 1;
        w = w.u8(name.len() as u8).bytes(name.as_bytes()).bytes(&s);
    }
    w = w.u32(0);
    let mut rs = [0u8; 39];
    rs[..7].copy_from_slice(&[2, 1, 1, 7, 1, 2, 0]);
    rs[7..11].copy_from_slice(&1u32.to_le_bytes());
    rs[11..15].copy_from_slice(&0.8f32.to_le_bytes());
    w.bytes(&rs).0
}

fn shader(did: u32, decls: &[Vec<u8>], samplers: &[(&str, u32)]) -> Vec<u8> {
    let mut w = W::default().u32(did).u32(0).u32(decls.len() as u32);
    for d in decls {
        w = w.bytes(d);
    }
    w.u32(1).bytes(&permutation(samplers)).0
}

fn library(store: BTreeMap<u32, Vec<u8>>) -> MaterialLibrary<impl tessera_material::RecordSource> {
    MaterialLibrary::new(FnSource(move |d: Did| {
        Ok::<_, std::io::Error>(store.get(&d.0).cloned())
    }))
}

/// A library holding shader `did` with `decls` and `samplers`, a material with
/// `overrides`, and surface 0x31000001 naming both.
fn one_surface(
    did: u32,
    decls: &[Vec<u8>],
    samplers: &[(&str, u32)],
    overrides: &[Vec<u8>],
) -> MaterialLibrary<impl tessera_material::RecordSource + use<>> {
    let mut store = BTreeMap::new();
    store.insert(did, shader(did, decls, samplers));
    store.insert(0x3000_0001, material(0x3000_0001, overrides));
    store.insert(
        0x3100_0001,
        surface_record(0x3100_0001, did, &[0x3000_0001]),
    );
    library(store)
}

const SURFACE: Did = Did(0x3100_0001);
const FB: (&str, u32) = ("FrameBufferTexture", 0);

fn evaluate(
    lib: &MaterialLibrary<impl tessera_material::RecordSource>,
    t: f64,
) -> Option<LiquidSurface> {
    let m = lib.resolve_surface(SURFACE).unwrap();
    let layout = lib.texture_matrices(m.shader.did).unwrap();
    LiquidSurface::evaluate(&m, &layout, t, &mut CurveStates::new())
}

/// `material.md` §11.6 recognition and the `water.md` §8.2 families, from the interface.
#[test]
fn families_follow_the_interface() {
    let family = |did: u32, decls: &[Vec<u8>], samplers: &[(&str, u32)]| {
        let lib = one_surface(did, decls, samplers, &[]);
        let m = lib.resolve_surface(SURFACE).unwrap();
        (
            m.shader.family,
            water_family(&m.shader),
            MaterialKind::of(&m),
        )
    };
    let plain = family(
        0x2B00_0001,
        &[float(1057, 0.005), texture(3011, 0x4100_0001, 1)],
        &[FB, ("NormalMap", 0)],
    );
    assert_eq!(
        plain,
        (
            ProgramFamily::Water,
            Some(WaterFamily::Plain),
            MaterialKind::Water
        )
    );
    // Plain water on mesh UV: the records named in `water.md` §8.2.
    let uv = family(0x2B00_084B, &[float(1057, 0.005)], &[FB]);
    assert_eq!(uv.1, Some(WaterFamily::PlainUv));
    // Older bump water declares 1056.
    let bump = family(0x2B00_0002, &[float(1056, 0.003), float(1055, 0.5)], &[FB]);
    assert_eq!(bump.1, Some(WaterFamily::OlderBump));
    // Flow-map water: a flow slot; layers 1 and 2 when 1059 is declared.
    let flow_decls = |layers: bool| {
        let mut d = vec![float(1057, 1.0), texture(3075, 0x4100_0002, 1)];
        if layers {
            d.push(float(1059, 0.0015));
        }
        d
    };
    let flow = family(0x2B00_0003, &flow_decls(true), &[FB, ("FlowTexture", 0)]);
    assert_eq!(
        flow.1,
        Some(WaterFamily::Flow {
            normal_layers: true
        })
    );
    let flow1 = family(0x2B00_0004, &flow_decls(false), &[FB, ("FlowTexture", 0)]);
    assert_eq!(
        flow1.1,
        Some(WaterFamily::Flow {
            normal_layers: false
        })
    );
    // Mirror: any of 1100–1106.
    let mirror = family(0x2B00_0005, &[float(1103, 1.0)], &[("DiffuseTexture", 0)]);
    assert_eq!(mirror.0, ProgramFamily::Mirror);
    assert_eq!(mirror.2, MaterialKind::Mirror);
    // Liquid distortion: 1057, no frame-buffer sampler, a liquid-normal sampler.
    let liquid = family(
        0x2B00_0006,
        &[float(1057, 0.005)],
        &[("LiquidNormalTexture", 0)],
    );
    assert_eq!(liquid.0, ProgramFamily::LiquidDistortion);
    // Lava: 1057, a flow sampler, no frame-buffer sampler.
    let lava = family(0x2B00_0007, &[float(1057, 0.1)], &[("FlowTexture", 0)]);
    assert_eq!(
        (lava.0, lava.1, lava.2),
        (ProgramFamily::Lava, None, MaterialKind::Lava)
    );
    assert!(lava.2.needs_own_program());
    // The window family: a frame-buffer sampler and water colours, but no 1056 or 1057.
    let window = family(
        0x2B00_0008,
        &[entry(2009, V::Color([1.0; 4]), &[], 1)],
        &[FB],
    );
    assert_eq!(window.0, ProgramFamily::Generic);
    assert!(!window.2.needs_own_program());
}

/// `water.md` §8.6.1: the noise texture is the default of a sampler no binding names.
#[test]
fn the_noise_map_is_an_unbound_sampler_default() {
    let lib = one_surface(
        0x2B00_07FD,
        &[
            float(1057, 1.0),
            float(1058, 1.0),
            float(1059, 0.0015),
            texture(3011, 0x4100_0001, 1),
            texture(3075, 0x4100_0002, 2),
        ],
        &[
            FB,
            ("NormalMap", 0),
            ("FlowTexture", 0),
            ("NoiseTexture", 0x4001_63D4),
        ],
        &[],
    );
    let Some(LiquidSurface::Water(w)) = evaluate(&lib, 0.0) else {
        panic!("water expected");
    };
    assert_eq!(
        w.family,
        WaterFamily::Flow {
            normal_layers: true
        }
    );
    assert_eq!(w.noise_map, Some(Did(0x4001_63D4)));
    assert_eq!(w.flow_map, Some(Did(0x4100_0002)));
    assert_eq!(w.normal_map, Some(Did(0x4100_0001)));
    // 1057 and 1058 are the phase rates in layer 0 (§8.6.2 step 3).
    assert_eq!(w.params.normal_speed[0], [1.0, 1.0]);
}

/// WA13 (`water.md` §8.7.1): a mode-2 speed *S*(*t*) = *a* + *b t* is evaluated at the
/// clock, so its value at *T* is *a* + *b T*, not *a*; a constant curve is the constant.
#[test]
fn curve_valued_speeds_follow_the_clock() {
    let ramp = V::Wave(2, [0.01, 0., 0., 0., 0., 0., 0.002, 0., 0., 0.]);
    let lib = one_surface(
        0x2B00_00A8,
        &[float(1057, 0.005), float(1059, 0.01), float(1063, 2.0)],
        &[FB],
        &[entry(1059, ramp, &[], 1)],
    );
    let speed = |t: f64| match evaluate(&lib, t) {
        Some(LiquidSurface::Water(w)) => w.params.normal_speed[1][0],
        _ => panic!("water expected"),
    };
    assert!((speed(0.0) - 0.01).abs() < 1e-7);
    assert!((speed(100.0) - 0.21).abs() < 1e-5, "{}", speed(100.0));
    let constant = |t: f64| match evaluate(&lib, t) {
        Some(LiquidSurface::Water(w)) => w.params.normal_speed[0][0],
        _ => panic!("water expected"),
    };
    assert_eq!(constant(0.0), constant(500.0));
}

/// §8.7.1: a curve-valued normal-layer scale is flagged (*B* from the camera's landblock).
#[test]
fn curve_valued_scales_are_flagged() {
    let sine = V::Wave(4, [2.0, 0., 0.05, 0., 0.33, 0., 1.0, 0., 0., 0.]);
    let lib = one_surface(
        0x2B00_00A8,
        &[float(1057, 0.005), float(1063, 2.0)],
        &[FB],
        &[entry(1063, sine, &[], 1)],
    );
    let Some(LiquidSurface::Water(w)) = evaluate(&lib, 1.0) else {
        panic!("water expected");
    };
    assert!(w.animated_scales);
    let still = one_surface(
        0x2B00_00A8,
        &[float(1057, 0.005), float(1063, 2.0)],
        &[FB],
        &[],
    );
    let Some(LiquidSurface::Water(w)) = evaluate(&still, 1.0) else {
        panic!("water expected");
    };
    assert!(!w.animated_scales);
}

/// `water.md` §8.5.4 and its worked example: the puddles' 1105 is a mode-2 curve with
/// `w[6]` = 0.00001, so at *t* = 100 s its value is 0.001.
#[test]
fn mirror_parameters_evaluate_at_the_clock() {
    let lib = one_surface(
        0x2B00_00AA,
        &[
            float(1100, 0.0),
            float(1101, 2.0),
            float(1102, 0.0),
            float(1103, 1.0),
            float(1104, 0.0),
            float(1105, 0.0),
            float(1106, 0.0),
            texture(3000, 0x4100_0010, 0),
            texture(3011, 0x4100_0011, 1),
            entry(2000, V::Color([1.0, 1.0, 1.0, 0.5]), &[], 0),
        ],
        &[("DiffuseTexture", 0), ("NormalMap", 0)],
        &[
            float(1100, 0.006),
            float(1101, 1.33),
            float(1106, 0.05),
            entry(
                1105,
                V::Wave(2, [0., 0., 0., 0., 0., 0., 0.000_01, 0., 0., 0.]),
                &[],
                1,
            ),
        ],
    );
    let Some(LiquidSurface::Mirror(m)) = evaluate(&lib, 100.0) else {
        panic!("mirror expected");
    };
    assert!(
        (m.normal_speed[1] - 0.001).abs() < 1e-7,
        "{}",
        m.normal_speed[1]
    );
    assert_eq!(m.reflection_distortion, 0.006);
    assert_eq!(m.fresnel_exponent, 1.33);
    assert_eq!(m.fresnel_bump, 0.05);
    assert_eq!(m.diffuse, Some(Did(0x4100_0010)));
    assert_eq!(m.normal_map, Some(Did(0x4100_0011)));
    assert_eq!(m.color[3], 0.5);
    assert!(m.all_finite());
    let Some(s) = evaluate(&lib, 100.0) else {
        panic!()
    };
    assert_eq!(s.textures(), vec![Did(0x4100_0010), Did(0x4100_0011)]);
}

/// `water.md` §8.6.5: the lava material's rates, scale, brightness and layers.
#[test]
fn lava_material() {
    let lib = one_surface(
        0x2B00_0810,
        &[
            float(1057, 0.1),
            float(1058, 0.1),
            float(1063, 6.0),
            float(1066, 1.5),
            float(1188, 0.0),
            float(1186, 0.0),
            float(1187, 0.0),
            texture(3000, 0x4100_0020, 0),
            texture(3001, 0x4100_0021, 1),
            texture(3018, 0x4100_0022, 2),
            texture(3075, 0x4100_0023, 3),
            texture(3079, 0x4100_0024, 4),
        ],
        &[
            ("ColorAndOpacity", 0),
            ("ColorAndOpacity2", 0),
            ("Emissive", 0),
            ("FlowTexture", 0),
            ("FoamTexture", 0),
            ("NoiseTexture", 0x4100_0025),
        ],
        &[entry(
            1186,
            V::Wave(4, [0., 0., 0.001, 0., 0., 0., 1.0, 0., 0., 0.]),
            &[],
            1,
        )],
    );
    let Some(LiquidSurface::Lava(l)) = evaluate(&lib, 0.5) else {
        panic!("lava expected");
    };
    assert_eq!(l.rates, [0.1, 0.1]);
    assert_eq!(l.scale, 6.0);
    assert_eq!(l.brightness, 1.5);
    assert_eq!(l.emissive, Some(Did(0x4100_0022)));
    assert_eq!(l.flow_map, Some(Did(0x4100_0023)));
    assert_eq!(l.foam_map, Some(Did(0x4100_0024)));
    assert_eq!(l.noise_map, Some(Did(0x4100_0025)));
    assert_eq!(
        l.color_layers,
        [Some(Did(0x4100_0020)), Some(Did(0x4100_0021))]
    );
    // 1186 is a sine evaluated at the clock (§8.6.5 step 6, §8.7.1).
    assert!(
        (l.depth_offset[0] - 0.001 * 0.5f32.sin()).abs() < 1e-6,
        "{}",
        l.depth_offset[0]
    );
    assert!(l.all_finite());
    // The lava record is not covered by the two-layer rule (`material.md` §10.3).
    assert_eq!(combine_of(Did(0x2B00_0810)), None);
}

/// `water.md` §8.5.5: a liquid-distortion record's layers, waves and texture matrix 0.
#[test]
fn distortion_material_reads_texture_matrix_0() {
    // Matrix 0: scale (0.3, 0.2) (semantics 41, 42 on matrix 0), other values bound too.
    let m0 = |id: u32, sem: u16, v: f32| entry(id, V::Const(v), &[(sem, 0, 0)], 0);
    let lib = one_surface(
        0x2B00_04B5,
        &[
            float(1057, 0.005),
            float(1058, -0.1),
            float(1048, 1.0),
            float(1066, 1.15),
            float(1063, 2.0),
            texture(3011, 0x4100_0030, 0),
            texture(3000, 0x4100_0031, 1),
            m0(1017, 38, 0.0),
            m0(1018, 39, 0.0),
            m0(1033, 40, 0.0),
            m0(1001, 41, 0.3),
            m0(1002, 42, 0.2),
            m0(1070, 43, 0.0),
            m0(1071, 44, 0.0),
        ],
        &[("LiquidNormalTexture", 0), ("LiquidDiffuseTexture", 0)],
        &[],
    );
    let Some(LiquidSurface::Distortion(d)) = evaluate(&lib, 0.0) else {
        panic!("distortion expected");
    };
    assert!(!d.world_coordinates);
    assert!(!d.vertex_color, "only 0x2B00063E reads the vertex colour");
    assert_eq!(d.speed[0], [0.005, -0.1]);
    assert_eq!(d.distortion, 1.0);
    assert_eq!(d.overbright, 1.15);
    assert!((d.matrix.rows[0][0] - 0.3).abs() < 1e-6);
    assert!((d.matrix.rows[1][1] - 0.2).abs() < 1e-6);
    assert_eq!(d.normal_map, Some(Did(0x4100_0030)));
    assert_eq!(d.diffuse, Some(Did(0x4100_0031)));
    assert!(d.all_finite());
}

/// `water.md` §8.5.5 step 5: the output of 0x2B00063E is multiplied by the vertex colour;
/// 0x2B000442 reads *B* (step 1).
#[test]
fn distortion_records_by_did() {
    let decls = [
        float(1057, 0.01),
        float(1048, 0.5),
        texture(3011, 0x4100_0030, 0),
        texture(3000, 0x4100_0031, 1),
    ];
    let samplers = [("LiquidNormalTexture", 0), ("LiquidDiffuseTexture", 0)];
    for (did, vertex_color, world) in [
        (0x2B00_063E, true, false),
        (0x2B00_0442, false, true),
        (0x2B00_04B5, false, false),
    ] {
        let lib = one_surface(did, &decls, &samplers, &[]);
        let Some(LiquidSurface::Distortion(d)) = evaluate(&lib, 0.0) else {
            panic!("{did:#x}: distortion expected");
        };
        assert_eq!(d.vertex_color, vertex_color, "{did:#x}");
        assert_eq!(d.world_coordinates, world, "{did:#x}");
    }
}

/// `material.md` §10.3: the second colour layer and its combine rule per record.
#[test]
fn second_layers_follow_the_record_table() {
    let two = |did: u32, role2: &str| {
        let lib = one_surface(
            did,
            &[texture(3000, 0x4100_0040, 0), texture(3001, 0x4100_0041, 1)],
            &[("ColorAndOpacity", 0), (role2, 0)],
            &[],
        );
        let m = lib.resolve_surface(SURFACE).unwrap();
        second_layer(&m)
    };
    assert_eq!(
        two(0x2B00_0817, "ColorAndOpacity2"),
        Some((Did(0x4100_0041), LayerCombine::Add))
    );
    assert_eq!(
        two(0x2B00_07A1, "ColorAndOpacity2"),
        Some((Did(0x4100_0041), LayerCombine::Multiply))
    );
    // Slot 3001 with a role that is not a second colour layer.
    assert_eq!(two(0x2B00_07A1, "DetailMap"), None);
    // A record the rule does not cover.
    assert_eq!(two(0x2B00_0A0A, "ColorAndOpacity2"), None);
}

/// Records of the generic path have no own-program material; every float of the others is
/// finite on hostile values.
#[test]
fn generic_records_have_no_liquid_material_and_values_stay_finite() {
    let lib = one_surface(
        0x2B00_0030,
        &[texture(3000, 0x4100_0050, 0)],
        &[("Color", 0)],
        &[],
    );
    assert!(evaluate(&lib, 1.0).is_none());
    let hostile = one_surface(
        0x2B00_00A8,
        &[
            float(1057, f32::NAN),
            float(1063, f32::INFINITY),
            float(1066, -0.0),
        ],
        &[FB],
        &[],
    );
    let Some(s) = evaluate(&hostile, 1e30) else {
        panic!("water expected");
    };
    assert!(s.all_finite());
    let Some(LiquidSurface::Water(w)) = evaluate(&hostile, f64::NAN) else {
        panic!("water expected");
    };
    assert!(w.all_finite());
}
