// SPDX-License-Identifier: GPL-3.0-or-later
//! Synthetic tests of the lighting module: the worked examples of `lighting.md` §14, the
//! light record of §8.3, the selection of §8.5 and Light entities.

use std::collections::HashMap;

use tessera_formats::terrain_type::FloatCurve;
use tessera_formats::{LandblockInfo, PropertyType, Waveform, WaveformExtra};

use super::*;
use crate::coords::LandblockKey;
use crate::testkit::{TestEntity, TestValue, landblock_info};

fn approx(a: f32, b: f32, tol: f32) -> bool {
    (a - b).abs() <= tol
}

#[test]
fn worked_example_point_light() {
    // §14: R² = 100, k = 0.04, d² = 80: 1 / (1 + 3.2) = 0.238; s = 0.8, smoothstep 0.896;
    // a = 0.213.
    assert!(approx(attenuation(80.0, 100.0, 0.04, false), 0.238, 1e-3));
    assert!(approx(attenuation(80.0, 100.0, 0.04, true), 0.213, 1e-3));
    assert_eq!(attenuation(100.0, 100.0, 0.04, true), 0.0);
    assert_eq!(attenuation(150.0, 100.0, 0.04, false), 0.0);
    assert_eq!(attenuation(0.0, 100.0, 0.04, true), 1.0);
    assert_eq!(attenuation(f32::NAN, 100.0, 0.04, true), 0.0);
    // Exactly 0.75 R² is not faded.
    assert_eq!(attenuation(75.0, 100.0, 0.0, true), 1.0);
}

#[test]
fn worked_example_light_record() {
    // §14: range 10, falloff 0.5, diffuse (1.0, 0.6, 0.2), intensity constant 1.0, σ = 1.
    let p = LightParams {
        diffuse: [1.0, 0.6, 0.2],
        intensity: FloatCurve::Constant(1.0),
        range: 10.0,
        falloff: 0.5,
        ..LightParams::default()
    };
    let r = LightRecord::point(
        &p,
        [1.0, 2.0, 3.0],
        p.range,
        p.constant_intensity().unwrap(),
        1.0,
    );
    assert_eq!(r.range_sq, 100.0);
    assert!(approx(r.k, 0.05, 1e-7));
    assert_eq!(r.colour, [1.0, 0.6, 0.2]);
    assert_eq!(r.kind, LightKind::Point);
    assert!(approx(r.attenuation(50.0, true), 0.286, 1e-3));
    assert!(approx(r.attenuation(80.0, true), 0.179, 1e-3));
    // Falloff 0.5: half strength at d = R / √5.
    let d = 10.0 / 5.0_f32.sqrt();
    assert!(approx(r.attenuation(d * d, true), 0.5, 1e-5));
    // Specular: white and no key light → the diffuse colour, × clamp(2σ × intensity).
    assert_eq!(r.specular, [1.0, 0.6, 0.2]);
    let key = LightParams {
        key_light: true,
        ..p.clone()
    };
    assert_eq!(
        LightRecord::point(&key, [0.0; 3], 10.0, 0.25, 1.0).specular,
        [0.5; 3]
    );
    // Colour clamps intensity × σ to [0, 1]; range at least 0.0002.
    let r = LightRecord::point(&p, [0.0; 3], 0.0, 3.0, 1.0);
    assert_eq!(r.colour, [1.0, 0.6, 0.2]);
    assert_eq!(r.range, MIN_RANGE);
    assert_eq!(
        LightRecord::point(&p, [0.0; 3], 10.0, -1.0, 1.0).colour,
        [0.0; 3]
    );
    assert_eq!(
        LightRecord::point(&p, [0.0; 3], f32::NAN, f32::NAN, 1.0).colour,
        [0.0; 3]
    );
}

#[test]
fn defaults_of_a_new_light() {
    let d = LightParams::default();
    assert_eq!(d.range, 25.0);
    assert_eq!(d.falloff, 1.0);
    assert_eq!(d.diffuse, [1.0; 3]);
    assert_eq!(d.specular, [1.0; 3]);
    assert_eq!(d.constant_intensity(), Some(0.5));
    assert!(!d.key_light);
    assert_eq!(d.at_time(true), (25.0, true));
    let driven = LightParams {
        driven_by_day_cycle: true,
        night: DayNightVariant {
            range: Some(12.0),
            activated: Some(true),
        },
        day: DayNightVariant {
            range: None,
            activated: Some(false),
        },
        ..d
    };
    assert_eq!(driven.at_time(true), (12.0, true));
    assert_eq!(driven.at_time(false), (25.0, false));
}

fn r(id: usize, directional: bool, key_light: bool) -> LightRef {
    LightRef {
        id,
        directional,
        key_light,
    }
}

#[test]
fn selection_order_and_limits() {
    let on = SelectionOptions {
        per_pixel: true,
        all_per_pixel: false,
    };
    // Directional main light → light 0; first key point light → point light 0; others to
    // the per-vertex lists in order.
    let s = select_lights(
        Some(r(0, true, false)),
        &[
            r(1, false, false),
            r(2, false, true),
            r(3, false, true),
            r(4, true, false),
        ],
        on,
    );
    assert_eq!(s.directional0, Some(0));
    assert_eq!(s.point0, Some(2));
    assert_eq!(s.vertex_points, vec![1, 3]);
    assert_eq!(s.vertex_directional, Some(4));
    assert!(!s.default_directional);
    assert_eq!(s.len(), 5);
    // Per-pixel off: every light per vertex; a point main light goes there first.
    let s = select_lights(
        Some(r(0, false, false)),
        &[r(1, false, true), r(2, false, false)],
        SelectionOptions::default(),
    );
    assert_eq!(s.point0, None);
    assert_eq!(s.vertex_points, vec![0, 1, 2]);
    // all_per_pixel makes every light a candidate.
    let s = select_lights(
        None,
        &[r(1, false, false), r(2, true, false)],
        SelectionOptions {
            per_pixel: true,
            all_per_pixel: true,
        },
    );
    assert_eq!((s.point0, s.directional0), (Some(1), Some(2)));
    // The key directional light takes light 0, so no default light is added.
    assert!(!s.default_directional);
    assert_eq!(s.len(), 2);
    // At most 8 in all, main light included.
    let many: Vec<LightRef> = (1..=20).map(|i| r(i, false, false)).collect();
    let s = select_lights(Some(r(0, true, false)), &many, on);
    assert_eq!(s.len(), MAX_LIGHTS);
    assert_eq!(s.vertex_points, (1..=7).collect::<Vec<_>>());
    assert_eq!(s.dropped, 13);
    // Without a main light the default directional light holds one of the 8.
    let s = select_lights(None, &many[..8], on);
    assert!(s.default_directional && s.directional0.is_none());
    assert_eq!(s.vertex_points, (1..=7).collect::<Vec<_>>());
    assert_eq!((s.len(), s.dropped), (MAX_LIGHTS, 1));
    // A per-pixel key directional light replaces it in light 0 (lighting.md §8.5 OPEN,
    // Tessera Decision).
    let s = select_lights(None, &[r(7, true, true)], on);
    assert_eq!(s.directional0, Some(7));
    assert!(!s.default_directional);
    assert_eq!(s.len(), 1);
    // Per-pixel off: the directional light goes per vertex and the default one stays.
    let s = select_lights(None, &[r(7, true, true)], SelectionOptions::default());
    assert_eq!(
        (s.vertex_directional, s.default_directional),
        (Some(7), true)
    );
    assert_eq!(s.len(), 2);
    // One extra directional slot only.
    let s = select_lights(None, &[r(1, true, false), r(2, true, false)], on);
    assert_eq!(s.vertex_directional, Some(1));
    assert_eq!(s.dropped, 1);
}

#[test]
fn object_light_list_nearest_first() {
    let lights = [
        LightReach {
            id: 0,
            position: [30.0, 0.0, 0.0],
            range: 10.0,
        },
        LightReach {
            id: 1,
            position: [5.0, 0.0, 0.0],
            range: 1.0,
        },
        LightReach {
            id: 2,
            position: [0.0, 12.0, 0.0],
            range: 10.0,
        },
        LightReach {
            id: 3,
            position: [f32::NAN, 0.0, 0.0],
            range: 10.0,
        },
    ];
    // Bounding sphere radius 3 at the origin: light 1 reaches 1 + 3 = 4 < 5 (no), light 2
    // reaches 13 ≥ 12 (yes), light 0 reaches 13 < 30 (no).
    assert_eq!(object_light_list([0.0; 3], 3.0, &lights), vec![2]);
    assert_eq!(object_light_list([0.0; 3], 4.0, &lights), vec![1, 2]);
    assert!(object_light_list([f32::NAN; 3], 4.0, &lights).is_empty());
}

mod pid {
    pub const DIFFUSE: u32 = 0x7000_0001;
    pub const INTENSITY: u32 = 0x7000_0002;
    pub const RANGE: u32 = 0x7000_0003;
    pub const FALLOFF: u32 = 0x7000_0004;
    pub const KEY: u32 = 0x7000_0005;
    pub const LAYERS: u32 = 0x7000_0006;
    pub const NIGHT_RANGE: u32 = 0x7000_0007;
    pub const DRIVEN: u32 = 0x7000_0008;
    pub const LAYER: u32 = 0x7000_0009;
}

#[test]
fn light_entities_from_landblock_info() {
    use pid::*;
    let names: HashMap<&str, u32> = [
        ("LightDiffuse", DIFFUSE),
        ("LightDiffuseIntensity", INTENSITY),
        ("LightRange", RANGE),
        ("LightFalloff", FALLOFF),
        ("KeyLight", KEY),
        ("Entity_ContentLayers", LAYERS),
        ("Light.Script@Entity.Night.m_fLightRange", NIGHT_RANGE),
        ("Light_DrivenByDayCycle", DRIVEN),
    ]
    .into_iter()
    .collect();
    let pids = LightPids::from_names(|n| names.get(n).copied());
    let schema: HashMap<u32, PropertyType> = [
        (DIFFUSE, 16),
        (INTENSITY, 9),
        (RANGE, 9),
        (FALLOFF, 9),
        (KEY, 21),
        (LAYERS, 12),
        (NIGHT_RANGE, 9),
        (DRIVEN, 21),
        (LAYER, 8),
    ]
    .into_iter()
    .map(|(p, t)| (p, PropertyType::from_code(t)))
    .collect();
    let key = LandblockKey::new(1, 10, 10).unwrap();
    let light = |serial: u32, props: Vec<(u32, TestValue)>| TestEntity {
        kind: 9,
        serial,
        object: Some(0x4700_0001),
        position: Some([10.0, 20.0, 30.0]),
        rotation: [1.0, 0.0, 0.0, 0.0],
        properties: props,
        interior: serial == 2,
    };
    let entities = [
        light(
            1,
            vec![
                // Stored B, G, R, A: blue 0, green 140, red 255.
                (DIFFUSE, TestValue::Colour([0, 140, 255, 255])),
                (INTENSITY, TestValue::Float(0.8)),
                (RANGE, TestValue::Float(10.0)),
                (FALLOFF, TestValue::Float(0.3)),
                (KEY, TestValue::Bool(true)),
                (DRIVEN, TestValue::Bool(true)),
                (NIGHT_RANGE, TestValue::Float(4.0)),
            ],
        ),
        light(2, vec![(RANGE, TestValue::Float(f32::NAN))]),
        // Content layer 0x10000019 only: not in the shared world.
        light(
            3,
            vec![(
                LAYERS,
                TestValue::Array(vec![(LAYER, TestValue::Enum(0x1000_0019))]),
            )],
        ),
        TestEntity::physics(4, 0x4700_0002, [0.0; 3], vec![]),
        TestEntity {
            position: None,
            ..light(5, vec![])
        },
    ];
    let bytes = landblock_info(key, &entities);
    let info = LandblockInfo::parse(&bytes, Some(39), &schema).unwrap();
    let lights = light_entities(&info, &pids, &[1]);
    assert_eq!(lights.len(), 2);
    let a = &lights[0];
    assert_eq!(a.index, 0);
    assert_eq!(a.position, [10.0, 20.0, 30.0]);
    assert!(approx(a.params.diffuse[0], 1.0, 1e-6));
    assert!(approx(a.params.diffuse[1], 140.0 / 255.0, 1e-6));
    assert_eq!(a.params.diffuse[2], 0.0);
    assert_eq!(a.params.constant_intensity(), Some(0.8));
    assert_eq!((a.params.range, a.params.falloff), (10.0, 0.3));
    assert!(a.params.key_light && !a.params.specular_stored);
    assert_eq!(a.params.at_time(true), (4.0, true));
    assert_eq!(a.params.at_time(false), (10.0, true));
    let b = &lights[1];
    assert!(b.interior);
    assert_eq!(b.params.range, DEFAULT_RANGE, "non-finite range falls back");
    // With the named layer active the third light appears.
    assert_eq!(light_entities(&info, &pids, &[1, 0x1000_0019]).len(), 3);
}

#[test]
fn waveforms_become_float_curves() {
    let w = |subtype: u32, values: Vec<f32>, extra: Option<WaveformExtra>| Waveform {
        subtype,
        values,
        extra,
    };
    assert_eq!(
        waveform_curve(&w(1, vec![0.7], None)),
        Some(FloatCurve::Constant(0.7))
    );
    let c = waveform_curve(&w(
        7,
        vec![0.8, 0.2, 2.0, 0.0, 0.0, 0.0, 0.0, 0.0, 0.0, 0.0],
        None,
    ))
    .unwrap();
    assert_eq!(c.mode(), 7);
    let k = waveform_curve(&w(
        10,
        vec![0.0; 10],
        Some(WaveformExtra {
            scalar: 4.0,
            flag: true,
            pairs: vec![(1.0, 0.0), (0.5, 50.0)],
        }),
    ))
    .unwrap();
    assert_eq!(k.mode(), 10);
    assert!(waveform_curve(&w(10, vec![0.0; 10], None)).is_none());
    // Subtypes 0 and above 10: the base term v0 + v1 · t (`property-system.md` §4.2) as a
    // mode-2 curve whose θ is 0.
    let vals: Vec<f32> = (1..=10).map(|i| i as f32).collect();
    for subtype in [0, 11, u32::MAX] {
        let b = waveform_curve(&w(subtype, vals.clone(), None)).unwrap();
        let mut want = [0.0; 10];
        want[0] = 1.0;
        want[1] = 2.0;
        assert_eq!(b, FloatCurve::waveform(2, want).unwrap(), "{subtype}");
    }
    // A Light whose intensity is subtype 0 with v0 = 1, v1 = 0 is no longer the default 0.5.
    let base = waveform_curve(&w(
        0,
        vec![1.0, 0.0, 9.0, 9.0, 9.0, 9.0, 9.0, 9.0, 9.0, 9.0],
        None,
    ))
    .unwrap();
    let mut one = [0.0; 10];
    one[0] = 1.0;
    assert_eq!(base, FloatCurve::waveform(2, one).unwrap());
    assert_ne!(base, FloatCurve::Constant(DEFAULT_INTENSITY));
    assert!(waveform_curve(&w(1, vec![], None)).is_none());
}
