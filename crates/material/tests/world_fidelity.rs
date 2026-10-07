// SPDX-License-Identifier: GPL-3.0-or-later
//! Synthetic tests of the M4.5 additions: float curves over time on hostile values
//! (`material.md` §5.3.1), texture transforms (§7.3, §8.4), per-surface lighting inputs
//! (`lighting.md` §4.1) and the water material chain (`water.md` §8.2). Every fixture is
//! generated here.

use std::collections::BTreeMap;

use proptest::prelude::*;
use tessera_formats::{Did, Record};
use tessera_material::inputs::SPECULAR_POWER_CUTOFF;
use tessera_material::records::{CullMode, VALUE_TYPE_COLOR, VALUE_TYPE_CURVE, VALUE_TYPE_WORD};
use tessera_material::transform::{MAX_TEXTURE_MATRICES, TransformValue};
use tessera_material::water::water_ids;
use tessera_material::{
    CurveState, CurveStates, FloatCurve, FnSource, MaterialLibrary, MaterialRecord, NamedValue,
    ObjectInputs, ShaderInterface, ShaderRecord, SurfaceInputs, TextureMatrixLayout,
    TextureTransform, WaterMaterial, WaterParams, program_time,
};

// ---------------------------------------------------------------------------------------
// Builders (records of `material.md` §4–§7)
// ---------------------------------------------------------------------------------------

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

#[derive(Clone)]
enum V {
    Word(u32),
    Color([f32; 4]),
    Const(f32),
    Wave(u32, [f32; 10]),
}

/// One parameter entry (§5.1) with bindings (semantic, permutation, slot).
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

/// One permutation without programs: samplers by name, then a render-state block
/// (src, dst, op, depth_write, cull).
fn permutation(samplers: &[&str], state: (u8, u8, u8, u8, u8)) -> Vec<u8> {
    let mut w = W::default().u32(1).u32(2).u32(3).u32(4).u32(0);
    w = w.u32(samplers.len() as u32);
    for name in samplers {
        let mut s = [0u8; 40];
        s[8] = 1;
        s[9] = 1;
        w = w.u8(name.len() as u8).bytes(name.as_bytes()).bytes(&s);
    }
    w = w.u32(0);
    let (src, dst, op, dw, cull) = state;
    let mut rs = [0u8; 39];
    rs[..7].copy_from_slice(&[src, dst, op, 7, dw, cull, 0]);
    rs[7..11].copy_from_slice(&1u32.to_le_bytes());
    rs[11..15].copy_from_slice(&0.8f32.to_le_bytes());
    w.bytes(&rs).0
}

fn shader(did: u32, decls: &[Vec<u8>], perms: &[Vec<u8>]) -> Vec<u8> {
    let mut w = W::default().u32(did).u32(0).u32(decls.len() as u32);
    for d in decls {
        w = w.bytes(d);
    }
    w = w.u32(perms.len() as u32);
    for p in perms {
        w = w.bytes(p);
    }
    w.0
}

const LIT_BASE: (u8, u8, u8, u8, u8) = (5, 6, 1, 1, 2);
const LIT_LIGHT: (u8, u8, u8, u8, u8) = (5, 2, 1, 0, 2);
const UNLIT_OPAQUE_NO_CULL: (u8, u8, u8, u8, u8) = (2, 1, 1, 1, 1);

fn wave(mode: u32, w: [f32; 10]) -> FloatCurve {
    let mut params = [0.0; 11];
    params[..10].copy_from_slice(&w);
    FloatCurve {
        mode,
        params,
        param_count: 10,
        loop_flag: 0,
        keys: Vec::new(),
    }
}

fn library(store: BTreeMap<u32, Vec<u8>>) -> MaterialLibrary<impl tessera_material::RecordSource> {
    MaterialLibrary::new(FnSource(move |d: Did| {
        Ok::<_, std::io::Error>(store.get(&d.0).cloned())
    }))
}

// ---------------------------------------------------------------------------------------
// Float curves (§5.3.1)
// ---------------------------------------------------------------------------------------

#[test]
fn spec_examples_through_the_public_api() {
    // Mode 9 example: 32 steps at 16 per second, each adding π × 0.01.
    let c = wave(9, [-0.002, 0., 32.0, 0., 0.01, 0., 16.0, 0., 0., 0.]);
    let mut st = CurveState::new();
    let step = std::f64::consts::PI * 0.01;
    for (t, j) in [
        (0.0, 0.0),
        (0.5, 8.0),
        (1.99, 31.0),
        (2.0, 0.0),
        (3.0, 16.0),
    ] {
        let v = c.evaluate_finite(t, &mut st);
        assert!(
            (f64::from(v) - (-0.002 + j * step)).abs() < 1e-5,
            "t {t}: {v}"
        );
    }

    // Mode 10 example: (1, 0), (0.5, 45), (0.5, 80), (1, 100), period 18 s, loop 0.
    let bytes = material(
        0x3000_0001,
        &[{
            let mut w = W::default()
                .u32(1000)
                .u16(1000)
                .u32(VALUE_TYPE_CURVE)
                .u32(10);
            for _ in 0..10 {
                w = w.f32(0.0);
            }
            w = w.f32(18.0).u8(0).u32(4);
            for (v, p) in [(1.0, 0.0), (0.5, 45.0), (0.5, 80.0), (1.0, 100.0)] {
                w = w.f32(v).f32(p);
            }
            w.u32(0).u8(0).0
        }],
    );
    let m = MaterialRecord::parse(&bytes).expect("decodes");
    let tessera_material::records::ParamValue::Float(curve) = &m.entries[0].value else {
        panic!("not a curve");
    };
    let mut states = CurveStates::new();
    let t0 = 7.0;
    let _ = states.evaluate(1000, curve, t0);
    let at = |states: &mut CurveStates, dt: f64| states.evaluate(1000, curve, t0 + dt);
    assert!((at(&mut states, 4.5) - (1.0 - 25.0 / 45.0 * 0.5)).abs() < 1e-5);
    // Between the two 0.5 keys the value holds at 0.5.
    assert!((at(&mut states, 10.8) - 0.5).abs() < 1e-5);
    let last = at(&mut states, 17.0);
    assert_eq!(at(&mut states, 18.0), last);
    assert_eq!(at(&mut states, 100.0), last);
    assert_eq!(states.len(), 1);
    // A constant curve creates no state.
    assert_eq!(
        states.evaluate(1043, &FloatCurve::constant(0.65), 3.0),
        0.65
    );
    assert_eq!(states.len(), 1);
    states.reset();
    assert!(states.is_empty());
}

#[test]
fn program_time_wraps_at_an_hour() {
    assert_eq!(program_time(0.0), 0.0);
    assert_eq!(program_time(12.5), 12.5);
    assert_eq!(program_time(3_600.0), 3_600.0);
    assert!((program_time(3_605.25) - 5.25).abs() < 1e-3);
    assert_eq!(program_time(f64::NAN), 0.0);
    assert_eq!(program_time(f64::INFINITY), 0.0);
    assert_eq!(program_time(-4.0), 0.0);
}

fn hostile_f32() -> impl Strategy<Value = f32> {
    prop_oneof![
        any::<f32>(),
        Just(f32::NAN),
        Just(f32::INFINITY),
        Just(f32::NEG_INFINITY),
        Just(f32::MAX),
        Just(f32::MIN),
        Just(0.0f32),
        -100.0f32..100.0,
    ]
}

fn hostile_time() -> impl Strategy<Value = f64> {
    prop_oneof![
        any::<f64>(),
        Just(f64::NAN),
        Just(f64::INFINITY),
        Just(-1e300),
        0.0f64..10_000.0,
        -10_000.0f64..0.0,
    ]
}

proptest! {
    #![proptest_config(ProptestConfig::with_cases(2_000))]

    /// Any mode, any stored parameters (NaN, infinities, extremes), any keys and any clock:
    /// no panic, and the finite evaluation is finite.
    #[test]
    fn curves_on_hostile_values(
        mode in 1u32..=10,
        params in proptest::array::uniform11(hostile_f32()),
        loop_flag in 0u8..=1,
        keys in proptest::collection::vec((hostile_f32(), hostile_f32()), 0..8),
        times in proptest::collection::vec(hostile_time(), 1..8),
        seed in any::<u32>(),
    ) {
        let curve = FloatCurve {
            mode,
            params,
            param_count: match mode { 1 => 1, 10 => 11, _ => 10 },
            loop_flag,
            keys: if mode == 10 { keys } else { Vec::new() },
        };
        let mut st = CurveState::with_seed(seed);
        let mut states = CurveStates::with_seed(seed);
        for &t in &times {
            let _ = curve.evaluate_at(t, &mut st);
            let v = curve.evaluate_finite(t, &mut st);
            prop_assert!(v.is_finite(), "mode {mode} t {t}: {v}");
            prop_assert!(states.evaluate(1000, &curve, t).is_finite());
        }
        let _ = curve.evaluate();
        prop_assert!(program_time(times[0]).is_finite());
    }

    /// Finite parameters within a sane range and a finite clock: the raw evaluation is
    /// already finite for the bounded modes (constant, sine, square, rectified sine, noise).
    #[test]
    fn bounded_modes_stay_bounded(
        mode in prop_oneof![Just(1u32), Just(4), Just(5), Just(6), Just(7), Just(8)],
        w in proptest::array::uniform10(-10.0f32..10.0),
        t in 0.0f64..10_000.0,
    ) {
        let mut w = w;
        // No time-dependent base, amplitude or roughness (a growing G makes the mode-8
        // weights overflow, which only the finite evaluation absorbs):
        // |result| ≤ |B| + |A| × the waveform bound.
        w[1] = 0.0;
        w[3] = 0.0;
        w[9] = 0.0;
        let curve = wave(mode, w);
        let v = curve.evaluate_at(t, &mut CurveState::new());
        prop_assert!(v.is_finite());
        // N₂ sums six octaves with weights 2^(−H·i), H = 1 − w[8] ≥ −9: generous bound.
        let bound = f64::from(w[0].abs()) + f64::from(w[2].abs()) * 6.0 * 2f64.powf(9.0 * 5.0) + 1e-3;
        prop_assert!(f64::from(v.abs()) <= bound, "{v} > {bound}");
    }

    /// Any bytes after a valid material header decode or fail, never panic; decoded
    /// curves evaluate finitely.
    #[test]
    fn decoded_curves_never_panic(body in proptest::collection::vec(any::<u8>(), 0..200), t in hostile_time()) {
        let mut bytes = 0x3000_0001u32.to_le_bytes().to_vec();
        bytes.extend_from_slice(&1u32.to_le_bytes());
        bytes.extend_from_slice(&1000u32.to_le_bytes());
        bytes.extend_from_slice(&1000u16.to_le_bytes());
        bytes.extend_from_slice(&VALUE_TYPE_CURVE.to_le_bytes());
        bytes.extend_from_slice(&body);
        if let Ok(m) = MaterialRecord::parse(&bytes) {
            for e in &m.entries {
                if let tessera_material::records::ParamValue::Float(c) = &e.value {
                    prop_assert!(c.evaluate_finite(t, &mut CurveState::new()).is_finite());
                }
            }
        }
    }

    /// Texture transforms from any values compose to a finite matrix.
    #[test]
    fn transforms_on_hostile_values(v in proptest::array::uniform7(hostile_f32()), u in -10.0f32..10.0, w in -10.0f32..10.0) {
        let mut t = TextureTransform::RESET;
        for (value, x) in TransformValue::ALL.into_iter().zip(v) {
            t.set(value, x);
            prop_assert_eq!(t.get(value).to_bits(), x.to_bits());
        }
        let m = t.matrix();
        prop_assert!(m.rows.iter().flatten().all(|x| x.is_finite()));
        let _ = m.apply(u, w);
        let mm = m.then(&m);
        prop_assert!(mm.rows.iter().flatten().all(|x| x.is_finite()), "{mm:?}");
    }
}

// ---------------------------------------------------------------------------------------
// Texture transforms (§7.3, §8.4)
// ---------------------------------------------------------------------------------------

/// A lit shader with a full texture matrix 0 bound by semantics 38–44 (set-0 ids), a
/// matrix 1 with only a scale U bound, an older-semantic binding, an out-of-range matrix
/// index and a conflicting binding.
fn transform_shader() -> Vec<u8> {
    let b = |semantic: u16, slot: u32| [(semantic, 0u32, slot), (semantic, 1, slot)];
    shader(
        0x2B00_0010,
        &[
            entry(3000, V::Word(0x4000_0007), &[(10, 0, 0), (10, 1, 0)], 0),
            entry(1017, V::Const(0.0), &b(38, 0), 0),
            entry(1018, V::Const(0.0), &b(39, 0), 0),
            entry(1033, V::Const(0.0), &b(40, 0), 0),
            entry(1001, V::Const(1.0), &b(41, 0), 0),
            entry(1002, V::Const(1.0), &b(42, 0), 0),
            entry(1070, V::Const(0.0), &b(43, 0), 0),
            entry(1071, V::Const(0.0), &b(44, 0), 0),
            // Matrix 1: scale U only, plus an older semantic (ignored).
            entry(1003, V::Const(3.0), &[(41, 0, 1), (35, 0, 1)], 0),
            // Matrix index 9: out of range.
            entry(1005, V::Const(5.0), &[(41, 0, 9)], 0),
            // Conflict: another parameter on matrix 0's rotation, in one permutation only
            // (1033 is bound in two, so it is kept).
            entry(1034, V::Const(45.0), &[(40, 0, 0)], 0),
        ],
        &[
            permutation(&["ColorAndGloss"], LIT_BASE),
            permutation(&["ColorAndGloss"], LIT_LIGHT),
        ],
    )
}

#[test]
fn texture_matrix_layout_from_bindings() {
    let rec = ShaderRecord::parse(&transform_shader()).expect("decodes");
    let layout = TextureMatrixLayout::from_record(&rec);
    assert_eq!(layout.matrices.len(), 2);
    let m0 = layout.matrix(0).expect("matrix 0");
    assert!(m0.is_complete());
    assert_eq!(m0.param(TransformValue::TranslateU), Some(1017));
    assert_eq!(
        m0.param(TransformValue::Rotate),
        Some(1033),
        "most bindings win"
    );
    assert_eq!(m0.param(TransformValue::EarlyV), Some(1071));
    let m1 = layout.matrix(1).expect("matrix 1");
    assert_eq!(m1.param(TransformValue::ScaleU), Some(1003));
    assert!(!m1.is_complete());
    assert_eq!(layout.older_bindings, 1);
    assert_eq!(layout.out_of_range, 1);
    assert_eq!(layout.conflicts, 1);
    assert!(layout.matrix(MAX_TEXTURE_MATRICES + 1).is_none());
}

#[test]
fn texture_matrices_evaluate_over_materials_and_time() {
    let mut store = BTreeMap::new();
    store.insert(0x2B00_0010, transform_shader());
    // The material rotates 90° about the centre and scrolls U at 0.25 per second
    // (mode 2: B + F·x with w[6] = 0.25).
    store.insert(
        0x3000_0010,
        material(
            0x3000_0010,
            &[
                entry(1033, V::Const(90.0), &[], 0),
                entry(1070, V::Const(-0.5), &[], 0),
                entry(1071, V::Const(-0.5), &[], 0),
                entry(1018, V::Const(0.5), &[], 0),
                entry(
                    1017,
                    V::Wave(2, [0.5, 0., 0., 0., 0., 0., 0.25, 0., 0., 0.]),
                    &[],
                    0,
                ),
            ],
        ),
    );
    store.insert(
        0x3100_0010,
        surface_record(0x3100_0010, 0x2B00_0010, &[0x3000_0010]),
    );
    let lib = library(store);
    let r = lib.resolve_surface(Did(0x3100_0010)).expect("resolves");
    assert!(r.features.uv_transform && r.features.animated);
    assert!(!SurfaceInputs::is_static(&r));
    let mut states = CurveStates::new();
    let at0 = lib
        .surface_inputs(&r, &ObjectInputs::default(), 0.0, &mut states)
        .expect("inputs");
    let m0 = at0.texture_matrix(0).expect("matrix 0");
    // Rotation about (0.5, 0.5): the centre stays, (1, 0.5) goes to (0.5, 1).
    let (u, v) = m0.map.apply(0.5, 0.5);
    assert!((u - 0.5).abs() < 1e-5 && (v - 0.5).abs() < 1e-5);
    let (u, v) = m0.map.apply(1.0, 0.5);
    assert!((u - 0.5).abs() < 1e-5 && (v - 1.0).abs() < 1e-5, "{u} {v}");
    // Matrix 1: scale U 3, every other value the reset value 0 (§8.4).
    let m1 = at0.texture_matrix(1).expect("matrix 1");
    assert_eq!(m1.transform.scale, [3.0, 0.0]);
    assert_eq!(m1.map.apply(1.0, 1.0), (3.0, 0.0));
    // Two seconds later the translation U has moved by 0.5.
    let at2 = lib
        .surface_inputs(&r, &ObjectInputs::default(), 2.0, &mut states)
        .expect("inputs");
    let d = at2.texture_matrix(0).unwrap().transform.translate[0]
        - at0.texture_matrix(0).unwrap().transform.translate[0];
    assert!((d - 0.5).abs() < 1e-5, "{d}");
    assert!(at2.all_finite());
    // The layout is cached with the interface.
    assert_eq!(
        lib.texture_matrices(Did(0x2B00_0010))
            .unwrap()
            .matrices
            .len(),
        2
    );
}

// ---------------------------------------------------------------------------------------
// Lighting inputs (`lighting.md` §4.1)
// ---------------------------------------------------------------------------------------

fn lit_specular_shader() -> Vec<u8> {
    shader(
        0x2B00_0020,
        &[
            entry(3000, V::Word(0x4000_0007), &[(10, 0, 0)], 0),
            entry(2000, V::Color([1.0, 0.5, 0.25, 1.0]), &[(0, 0, 0)], 0),
            entry(2001, V::Color([0.5, 0.5, 0.5, 1.0]), &[(1, 0, 0)], 0),
            entry(1000, V::Const(0.0), &[(2, 0, 0)], 0),
            entry(2014, V::Color([0.0, 1.0, 0.0, 1.0]), &[(3, 0, 0)], 0),
            entry(2003, V::Color([1.0, 0.0, 0.0, 1.0]), &[(4, 0, 0)], 0),
            entry(4003, V::Word(0), &[(5, 0, 0)], 0),
            entry(4001, V::Word(1), &[(6, 0, 0)], 0),
            entry(4002, V::Word(1), &[(7, 0, 0)], 0),
            entry(4004, V::Word(1), &[(8, 0, 0)], 0),
            entry(1043, V::Const(0.65), &[(9, 0, 0)], 0),
            // A named (flag-1) effect constant and colour.
            entry(1174, V::Const(2.5), &[], 1),
            entry(2013, V::Color([0.0, 0.0, 1.0, 1.0]), &[(4, 1, 0)], 1),
        ],
        &[
            permutation(&["ColorAndOpacity"], LIT_BASE),
            permutation(&["ColorAndOpacity"], LIT_LIGHT),
        ],
    )
}

#[test]
fn lighting_inputs_follow_section_4_1() {
    let mut store = BTreeMap::new();
    store.insert(0x2B00_0020, lit_specular_shader());
    // Material A keeps power 0 (no highlight); material B sets 25, double-sided, no
    // depth write and a sine-animated threshold.
    store.insert(0x3000_0021, material(0x3000_0021, &[]));
    store.insert(
        0x3000_0022,
        material(
            0x3000_0022,
            &[
                entry(1000, V::Const(25.0), &[], 0),
                entry(4003, V::Word(1), &[], 0),
                entry(4002, V::Word(0), &[], 0),
                entry(4001, V::Word(0), &[], 0),
                entry(
                    1043,
                    V::Wave(4, [0.5, 0., 0.25, 0., 0., 0., 1.0, 0., 0., 0.]),
                    &[],
                    0,
                ),
            ],
        ),
    );
    store.insert(
        0x3100_0021,
        surface_record(0x3100_0021, 0x2B00_0020, &[0x3000_0021]),
    );
    store.insert(
        0x3100_0022,
        surface_record(0x3100_0022, 0x2B00_0020, &[0x3000_0022]),
    );
    let lib = library(store);
    let object = ObjectInputs {
        tint: [0.5, 1.0, 1.0, 0.5],
        specular_power_scale: 1.0,
    };

    let a = lib.resolve_surface(Did(0x3100_0021)).unwrap();
    let ia = lib
        .surface_inputs(&a, &object, 1.0, &mut CurveStates::new())
        .unwrap();
    assert_eq!(ia.material_color, [0.5, 0.5, 0.25, 0.5], "2000 × tint");
    let s = ia.specular.expect("declares 1000");
    assert_eq!(s.power, 0.0);
    assert_eq!(
        s.color, [0.0; 3],
        "black below power {SPECULAR_POWER_CUTOFF}"
    );
    assert_eq!(ia.glow_color, Some([0.0, 1.0, 0.0, 1.0]));
    assert_eq!(ia.dye_color, Some([1.0, 0.0, 0.0, 1.0]));
    assert_eq!(ia.dye_color_2, Some([0.0, 0.0, 1.0, 1.0]));
    assert_eq!(ia.cull, CullMode::Back);
    assert!(ia.depth_test && ia.depth_write);
    assert_eq!(ia.alpha_test, Some(0.65));
    assert!(ia.texture_matrices.is_empty());
    assert_eq!(ia.named(1174), Some(NamedValue::Float(2.5)));
    assert_eq!(
        ia.named(2013),
        Some(NamedValue::Color([0.0, 0.0, 1.0, 1.0]))
    );
    assert_eq!(
        ia.named(2000),
        None,
        "flag 0: bound by semantic, not by name"
    );
    assert_eq!(ia.time, 1.0);
    assert!(SurfaceInputs::is_static(&a));

    let b = lib.resolve_surface(Did(0x3100_0022)).unwrap();
    let mut states = CurveStates::new();
    let ib = lib
        .surface_inputs(&b, &ObjectInputs::default(), 0.5, &mut states)
        .unwrap();
    let s = ib.specular.unwrap();
    assert_eq!(s.power, 25.0);
    assert_eq!(s.color, [0.5, 0.5, 0.5]);
    assert_eq!(ib.cull, CullMode::None);
    assert!(!ib.depth_test && !ib.depth_write);
    let expected = 0.5 + 0.25 * 0.5f64.sin();
    assert!((f64::from(ib.alpha_test.unwrap()) - expected).abs() < 1e-5);
    // The global specular-power scale multiplies the power.
    let ic = lib
        .surface_inputs(
            &b,
            &ObjectInputs {
                specular_power_scale: 0.01,
                ..ObjectInputs::default()
            },
            0.5,
            &mut states,
        )
        .unwrap();
    assert!((ic.specular.unwrap().power - 0.25).abs() < 1e-6);
    assert_eq!(ic.specular.unwrap().color, [0.0; 3]);
    // Hostile object inputs stay finite.
    let id = lib
        .surface_inputs(
            &b,
            &ObjectInputs {
                tint: [f32::NAN, f32::INFINITY, -1.0, 1.0],
                specular_power_scale: f32::INFINITY,
            },
            f64::NAN,
            &mut states,
        )
        .unwrap();
    assert!(id.all_finite());
}

#[test]
fn unlit_record_without_declarations_gets_defaults() {
    let rec = shader(
        0x2B00_0030,
        &[entry(3000, V::Word(0x4000_0007), &[(10, 0, 0)], 0)],
        &[permutation(&["Color"], UNLIT_OPAQUE_NO_CULL)],
    );
    let iface = std::sync::Arc::new(ShaderInterface::from_record(
        &ShaderRecord::parse(&rec).unwrap(),
    ));
    let r = tessera_material::ResolvedMaterial::resolve(iface, &[]);
    let i = SurfaceInputs::evaluate(
        &r,
        &TextureMatrixLayout::default(),
        &ObjectInputs::default(),
        0.0,
        &mut CurveStates::new(),
    );
    assert_eq!(i.material_color, [1.0; 4]);
    assert!(i.specular.is_none() && i.glow_color.is_none() && i.dye_color.is_none());
    assert_eq!(i.cull, CullMode::None, "most common cull NONE (§11.4)");
    assert!(i.depth_test && i.depth_write);
    assert!(i.alpha_test.is_none());
    assert!(i.named.is_empty());
}

// ---------------------------------------------------------------------------------------
// Water (`water.md` §8.2)
// ---------------------------------------------------------------------------------------

fn water_shader() -> Vec<u8> {
    shader(
        0x2B00_00A8,
        &[
            entry(3011, V::Word(0x4000_000D), &[(10, 0, 0)], 0),
            entry(3000, V::Word(0x4000_0007), &[(10, 1, 0)], 0),
            entry(3001, V::Word(0x4000_0008), &[(10, 1, 1)], 0),
            entry(2000, V::Color([1.0, 1.0, 1.0, 0.75]), &[(0, 1, 0)], 0),
            entry(1044, V::Const(1.0), &[], 1),
            entry(1045, V::Const(0.2), &[], 1),
            entry(1046, V::Const(0.0), &[], 1),
            entry(1047, V::Const(0.1), &[], 1),
            entry(1048, V::Const(0.04), &[], 1),
            entry(1050, V::Const(0.1), &[], 1),
            entry(1051, V::Const(2.0), &[], 1),
            entry(1054, V::Const(80.0), &[], 1),
            entry(1057, V::Const(0.005), &[], 1),
            entry(1058, V::Const(0.0075), &[], 1),
            entry(1063, V::Const(2.0), &[], 1),
            entry(1066, V::Const(1.0), &[], 1),
            entry(1068, V::Const(1.0), &[], 1),
            entry(2009, V::Color([1.0; 4]), &[], 1),
            entry(2010, V::Color([0.0, 0.0, 0.0, 1.0]), &[], 1),
        ],
        &[
            permutation(&["NormalMap"], (2, 1, 1, 1, 1)),
            permutation(&["Color", "Color2"], (5, 6, 1, 1, 2)),
        ],
    )
}

#[test]
fn water_material_chain() {
    let mut store = BTreeMap::new();
    store.insert(0x2B00_00A8, water_shader());
    store.insert(
        0x3000_00A8,
        material(
            0x3000_00A8,
            &[
                entry(3011, V::Word(0x4000_1234), &[], 0),
                entry(2009, V::Color([0.2, 0.4, 0.6, 1.0]), &[], 1),
                entry(1063, V::Const(3.0), &[], 1),
                // Animated wave speed: a ramp of 1 per second from 1.
                entry(
                    1044,
                    V::Wave(2, [1.0, 0., 0., 0., 0., 0., 1.0, 0., 0., 0.]),
                    &[],
                    1,
                ),
            ],
        ),
    );
    store.insert(
        0x3100_00A8,
        surface_record(0x3100_00A8, 0x2B00_00A8, &[0x3000_00A8]),
    );
    store.insert(
        0x2B00_0030,
        shader(0x2B00_0030, &[], &[permutation(&[], LIT_BASE)]),
    );
    store.insert(0x3100_0030, surface_record(0x3100_0030, 0x2B00_0030, &[]));
    let lib = library(store);

    let mut states = CurveStates::new();
    let w = lib
        .water_material(Did(0x3100_00A8), 2.0, &mut states)
        .unwrap()
        .expect("reads water parameters");
    assert_eq!(w.shader, Did(0x2B00_00A8));
    assert_eq!(w.normal_map, Some(Did(0x4000_1234)));
    assert_eq!(
        w.color_layers,
        [Some(Did(0x4000_0007)), Some(Did(0x4000_0008))]
    );
    assert_eq!(w.flow_map, None);
    assert_eq!(w.foam_map, None);
    assert_eq!(w.material_alpha, 0.75);
    assert_eq!(w.params.color, [0.2, 0.4, 0.6, 1.0]);
    assert_eq!(w.params.normal_scale[0], 3.0);
    assert!(
        (w.params.wave_speed - 3.0).abs() < 1e-6,
        "{}",
        w.params.wave_speed
    );
    assert_eq!(w.params.fresnel_exponent, 2.0);
    assert_eq!(w.params.specular_power, 80.0);
    // Undeclared ids take the §8.2 defaults and are listed.
    assert_eq!(
        w.undeclared,
        vec![1059, 1060, 1061, 1062, 1064, 1065, 1067, 2006, 2011]
    );
    let d = WaterParams::SPEC_DEFAULTS;
    assert_eq!(w.params.normal_speed[1], d.normal_speed[1]);
    assert_eq!(w.params.normal_speed[2], [-0.01, -0.0125]);
    assert_eq!(w.params.normal_scale[2], 8.0);
    assert_eq!(w.params.fake_specular_threshold, 0.25);
    assert_eq!(w.params.shoreline_color, [1.0; 4]);
    assert!(w.all_finite());
    assert!(water_ids::is_water_family(water_ids::WAVE_SPEED));

    // A surface whose shader reads no water parameter has no water material.
    assert!(
        lib.water_material(Did(0x3100_0030), 0.0, &mut states)
            .unwrap()
            .is_none()
    );
    let plain = lib.resolve_surface(Did(0x3100_0030)).unwrap();
    assert!(WaterMaterial::at_rest(&plain).is_none());

    // The water surface's other inputs resolve too (two-sided by its most common cull).
    let r = lib.resolve_surface(Did(0x3100_00A8)).unwrap();
    let i = lib
        .surface_inputs(&r, &ObjectInputs::default(), 2.0, &mut states)
        .unwrap();
    assert!(i.all_finite());
    assert!(i.named(1044).is_some());
}

// ---------------------------------------------------------------------------------------
// Review regressions (M4.5 review, 2026-10-06)
// ---------------------------------------------------------------------------------------

/// Set-0 ids and set-1 ids of §8.4, in semantic order 38–44.
const SET0: [u32; 7] = [1017, 1018, 1033, 1001, 1002, 1070, 1071];
const SET1: [u32; 7] = [1019, 1020, 1034, 1003, 1004, 1072, 1073];

/// Two colour permutations (0, 1) bind set 0 to matrix 0 and set 1 to matrix 1; a glow
/// permutation (2) binds set 1 to its matrix 0 (as four records on the reference
/// install do, `material.md` §18).
fn glow_pass_shader() -> Vec<u8> {
    let mut decls = vec![entry(
        3000,
        V::Word(0x4000_0007),
        &[(10, 0, 0), (10, 1, 0)],
        0,
    )];
    for (i, (&s0, &s1)) in SET0.iter().zip(&SET1).enumerate() {
        let semantic = 38 + i as u16;
        let scale = matches!(semantic, 41 | 42);
        let v = V::Const(if scale { 1.0 } else { 0.0 });
        decls.push(entry(
            s0,
            v.clone(),
            &[(semantic, 0, 0), (semantic, 1, 0)],
            0,
        ));
        decls.push(entry(
            s1,
            v,
            &[(semantic, 0, 1), (semantic, 1, 1), (semantic, 2, 0)],
            0,
        ));
    }
    // A second parameter on one (permutation, matrix, value): the first declared is kept.
    decls.push(entry(1035, V::Const(30.0), &[(40, 0, 3)], 0));
    decls.push(entry(1036, V::Const(60.0), &[(40, 0, 3)], 0));
    shader(
        0x2B00_0040,
        &decls,
        &[
            permutation(&["ColorAndGloss"], LIT_BASE),
            permutation(&["ColorAndGloss"], LIT_LIGHT),
            permutation(&["Glow"], LIT_BASE),
        ],
    )
}

#[test]
fn texture_matrices_are_kept_per_permutation() {
    let rec = ShaderRecord::parse(&glow_pass_shader()).expect("decodes");
    let layout = TextureMatrixLayout::from_record(&rec);
    // The record-wide table: set 0 on matrix 0 (two bindings against one), set 1 on 1.
    for (i, value) in TransformValue::ALL.into_iter().enumerate() {
        assert_eq!(layout.matrix(0).unwrap().param(value), Some(SET0[i]));
        assert_eq!(layout.matrix(1).unwrap().param(value), Some(SET1[i]));
    }
    // Seven values of matrix 0, plus the tie on matrix 3's rotation (kept: 1035).
    assert_eq!(layout.conflicts, 8);
    assert_eq!(layout.permutation_conflicts, 1);
    assert_eq!(
        layout.matrix(3).unwrap().param(TransformValue::Rotate),
        Some(1035)
    );
    // The glow permutation's own matrix 0 is set 1, and it binds no matrix 1.
    let glow = layout.for_permutation(2);
    assert_eq!(glow.len(), 1);
    assert_eq!(glow[0].matrix, 0);
    assert_eq!(glow[0].params, SET1.map(Some));
    assert!(layout.differs_from_merged(2));
    // The colour permutations bind what the merged table says (plus matrix 3 in 0).
    assert!(!layout.differs_from_merged(0) && !layout.differs_from_merged(1));
    let p0 = layout.for_permutation(0);
    assert_eq!(p0.len(), 3);
    assert_eq!(p0[2].param(TransformValue::Rotate), Some(1035));
    assert_eq!(layout.for_permutation(1).len(), 2);
    // A permutation without bindings: identity everywhere.
    assert!(layout.for_permutation(7).is_empty());
    assert_eq!(layout.permutation_tables().len(), 3);
    assert_eq!(
        layout
            .permutations()
            .map(|(p, t)| (p, t.len()))
            .collect::<Vec<_>>(),
        vec![(0, 3), (1, 2), (2, 1)]
    );

    // Evaluated: set 1 scales by 0.5 and scrolls; the glow pass applies it on matrix 0.
    let mut store = BTreeMap::new();
    store.insert(0x2B00_0040, glow_pass_shader());
    store.insert(
        0x3000_0040,
        material(
            0x3000_0040,
            &[
                entry(1003, V::Const(0.5), &[], 0),
                entry(1004, V::Const(0.5), &[], 0),
                entry(
                    1019,
                    V::Wave(2, [0.0, 0., 0., 0., 0., 0., 0.25, 0., 0., 0.]),
                    &[],
                    0,
                ),
            ],
        ),
    );
    store.insert(
        0x3100_0040,
        surface_record(0x3100_0040, 0x2B00_0040, &[0x3000_0040]),
    );
    let lib = library(store);
    let r = lib.resolve_surface(Did(0x3100_0040)).unwrap();
    let mut states = CurveStates::new();
    let colour = lib
        .surface_inputs(&r, &ObjectInputs::default(), 2.0, &mut states)
        .unwrap();
    assert_eq!(
        colour.texture_matrix(0).unwrap().map.apply(1.0, 1.0),
        (1.0, 1.0)
    );
    let glow = lib
        .permutation_texture_matrices(&r, 2, 2.0, &mut states)
        .unwrap();
    assert_eq!(glow.len(), 1);
    assert_eq!(glow[0].matrix, 0);
    let (u, v) = glow[0].map.apply(1.0, 1.0);
    assert!((u - 1.0).abs() < 1e-5 && (v - 0.5).abs() < 1e-5, "{u} {v}");
    assert_eq!(
        lib.permutation_texture_matrices(&r, 1, 2.0, &mut states)
            .unwrap(),
        colour.texture_matrices[..2].to_vec()
    );
}

#[test]
fn animated_threshold_falls_back_like_a_constant_one() {
    let mut store = BTreeMap::new();
    store.insert(0x2B00_0020, lit_specular_shader());
    store.insert(
        0x3000_0023,
        material(0x3000_0023, &[entry(1043, V::Const(f32::NAN), &[], 0)]),
    );
    store.insert(
        0x3000_0024,
        material(
            0x3000_0024,
            &[entry(
                1043,
                V::Wave(4, [f32::NAN, 0., 0.25, 0., 0., 0., 1.0, 0., 0., 0.]),
                &[],
                0,
            )],
        ),
    );
    store.insert(
        0x3100_0023,
        surface_record(0x3100_0023, 0x2B00_0020, &[0x3000_0023]),
    );
    store.insert(
        0x3100_0024,
        surface_record(0x3100_0024, 0x2B00_0020, &[0x3000_0024]),
    );
    let lib = library(store);
    let object = ObjectInputs::default();
    let constant = lib.resolve_surface(Did(0x3100_0023)).unwrap();
    let animated = lib.resolve_surface(Did(0x3100_0024)).unwrap();
    let mut states = CurveStates::new();
    let c = lib
        .surface_inputs(&constant, &object, 1.5, &mut states)
        .unwrap()
        .alpha_test
        .expect("alpha test on");
    let a = lib
        .surface_inputs(&animated, &object, 1.5, &mut states)
        .unwrap()
        .alpha_test
        .expect("alpha test on");
    assert!(c.is_finite() && c > 0.0, "{c}");
    assert_eq!(
        a, c,
        "a NaN animated 1043 falls back like a NaN constant one"
    );
}

/// material.md §19 / §18: an animated 1043 that is finite at t = 0 but not at the current
/// time falls back like a non-finite constant one (most common `alpha_ref`, then 0.5), not
/// to the curve's own t = 0 value.
#[test]
fn animated_threshold_finite_at_start_falls_back_like_a_constant_one() {
    let mut store = BTreeMap::new();
    store.insert(0x2B00_0020, lit_specular_shader());
    store.insert(
        0x3000_0023,
        material(0x3000_0023, &[entry(1043, V::Const(f32::NAN), &[], 0)]),
    );
    // Mode 2: 0.3 at t = 0, overflowing to +inf by t = 2.
    store.insert(
        0x3000_0025,
        material(
            0x3000_0025,
            &[entry(
                1043,
                V::Wave(2, [0.3, 0., 0., 0., 0., 0., f32::MAX, 0., 0., 0.]),
                &[],
                0,
            )],
        ),
    );
    store.insert(
        0x3100_0023,
        surface_record(0x3100_0023, 0x2B00_0020, &[0x3000_0023]),
    );
    store.insert(
        0x3100_0025,
        surface_record(0x3100_0025, 0x2B00_0020, &[0x3000_0025]),
    );
    let lib = library(store);
    let object = ObjectInputs::default();
    let constant = lib.resolve_surface(Did(0x3100_0023)).unwrap();
    let animated = lib.resolve_surface(Did(0x3100_0025)).unwrap();
    let mut states = CurveStates::new();
    let threshold = |m, t, states: &mut CurveStates| {
        lib.surface_inputs(m, &object, t, states)
            .unwrap()
            .alpha_test
            .expect("alpha test on")
    };
    let c = threshold(&constant, 2.0, &mut states);
    assert!(c.is_finite() && c > 0.0, "{c}");
    // Finite at t = 0: the curve's own value.
    let start = threshold(&animated, 0.0, &mut states);
    assert!((start - 0.3).abs() < 1e-6, "{start}");
    // Non-finite later: the constant fallback, not 0.3.
    let late = threshold(&animated, 2.0, &mut states);
    assert_eq!(
        late, c,
        "an animated 1043 that overflows falls back like a NaN constant one"
    );
}

#[test]
fn water_ids_declared_with_another_type_are_not_undeclared() {
    let mut store = BTreeMap::new();
    store.insert(
        0x2B00_00A9,
        shader(
            0x2B00_00A9,
            &[
                entry(1044, V::Const(2.0), &[], 1),
                entry(1066, V::Color([1.0; 4]), &[], 1),
                entry(2009, V::Const(0.5), &[], 1),
            ],
            &[permutation(&[], LIT_BASE)],
        ),
    );
    store.insert(0x3100_00A9, surface_record(0x3100_00A9, 0x2B00_00A9, &[]));
    let lib = library(store);
    let w = lib
        .water_material(Did(0x3100_00A9), 0.0, &mut CurveStates::new())
        .unwrap()
        .expect("reads water parameters");
    assert_eq!(w.params.wave_speed, 2.0);
    assert_eq!(w.mistyped, vec![1066, 2009]);
    assert!(!w.undeclared.contains(&1066) && !w.undeclared.contains(&2009));
    assert!(w.undeclared.contains(&1045));
    assert_eq!(w.params.overbright, WaterParams::SPEC_DEFAULTS.overbright);
    assert_eq!(w.params.color, WaterParams::SPEC_DEFAULTS.color);
}
