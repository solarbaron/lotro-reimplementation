// SPDX-License-Identifier: GPL-3.0-or-later
//! Synthetic tests of draw distances: the worked examples of `walker.md` §11.12 and
//! `mesh.md` §8.4, §8.7, and the fade.

use std::collections::HashMap;

use tessera_formats::environment::{LodClass, LodClassTable};
use tessera_formats::{Did, PropertySet, PropertyType};

use super::*;
use crate::testkit::{TestValue, property_set};

fn approx(a: f32, b: f32, tol: f32) -> bool {
    (a - b).abs() <= tol
}

fn table() -> LodClasses {
    let entry = |class: u32, d0: f32, d1: f32| LodClass {
        class,
        values: [d0, d1, 16.0, 80.0, 1000.0, 1000.0, 1100.0, -1.0],
    };
    LodClasses::new(LodClassTable {
        did: Did(0x3200_0000),
        entries: vec![
            entry(0xA, 300.0, 100.0),
            entry(0xB, 45.0, 30.0),
            entry(0xC, 80.0, 26.0),
            LodClass {
                class: 0xD,
                values: [f32::NAN, 10.0, 2.0, 80.0, 1000.0, 1000.0, 1100.0, -1.0],
            },
        ],
    })
}

#[test]
fn landscape_extent_by_level() {
    // walker.md §11.3 table.
    let e: Vec<f32> = [
        LandscapeLevel::VeryLow,
        LandscapeLevel::Low,
        LandscapeLevel::Medium,
        LandscapeLevel::High,
        LandscapeLevel::VeryHigh,
    ]
    .map(LandscapeLevel::extent)
    .to_vec();
    assert_eq!(e, vec![640.0, 1_120.0, 1_600.0, 2_080.0, 2_560.0]);
    assert_eq!(LandscapeLevel::Low.groups(), 2);
    assert_eq!(DistanceSettings::default().landscape_extent(), 2_080.0);
    assert_eq!(
        LandscapeLevel::from_name("veryhigh"),
        Some(LandscapeLevel::VeryHigh)
    );
    assert_eq!(LandscapeLevel::from_stored(3), Some(LandscapeLevel::High));
    assert_eq!(LandscapeLevel::from_stored(5), None);
    assert_eq!(
        DetailLevel::from_name(" UltraHigh "),
        Some(DetailLevel::UltraHigh)
    );
    assert_eq!(DetailLevel::from_name("Ultra"), None);
    assert_eq!(DetailLevel::from_stored(4), Some(DetailLevel::UltraHigh));
    assert_eq!(DetailLevel::High.stored(), 2);
}

#[test]
fn multipliers_and_dial_back() {
    let s = DistanceSettings::default();
    assert_eq!(
        (
            s.model_multiplier(),
            s.object_multiplier(),
            s.shadow_multiplier()
        ),
        (1.0, 1.0, 1.0)
    );
    let ultra = DistanceSettings {
        model_detail: DetailLevel::UltraHigh,
        object_draw_distance: DetailLevel::VeryHigh,
        stencil_shadows: 5,
        ..s
    };
    assert_eq!(ultra.model_multiplier(), 2.0);
    assert_eq!(ultra.object_multiplier(), 1.4);
    assert_eq!(ultra.shadow_multiplier(), 2.0);
    // DialBackGraphics: two levels down; Low and Medium fall outside → 2.0.
    let dial = DistanceSettings {
        model_detail: DetailLevel::UltraHigh,
        object_draw_distance: DetailLevel::Medium,
        dial_back: true,
        ..s
    };
    assert_eq!(dial.model_multiplier(), 1.0);
    assert_eq!(dial.object_multiplier(), 2.0);
    let off = DistanceSettings {
        stencil_shadows: 0,
        ..s
    };
    assert_eq!(off.shadow_multiplier(), 1.0);
    assert!(!off.shadows_enabled());
}

#[test]
fn worked_example_landscape_and_draw_distance() {
    // walker.md §11.12: Low landscape, E = 1,120 m; fog far 900 m: G = 900 m.
    let e = LandscapeLevel::Low.extent();
    assert_eq!(e, 1_120.0);
    let g = global_limit(
        Some(e),
        Some(FogClip {
            enabled: true,
            far: 900.0,
        }),
    );
    assert_eq!(g, 900.0);
    let vh = DistanceSettings {
        object_draw_distance: DetailLevel::VeryHigh,
        ..DistanceSettings::default()
    };
    let t = table();
    assert!(approx(
        t.values(Some(0xA), false).draw_distance(g, &vh),
        420.0,
        1e-3
    ));
    let landmark = LodValues([1000.0, 200.0, 16.0, 80.0, 1000.0, 1000.0, 1100.0, -1.0]);
    assert_eq!(landmark.draw_distance(g, &vh), 900.0);
    // With fog far 40 m, G is raised to 60 m.
    let g = global_limit(
        Some(e),
        Some(FogClip {
            enabled: true,
            far: 40.0,
        }),
    );
    assert_eq!(g, 60.0);
    // Fog off: G = E; no landscape and no fog: unbounded.
    assert_eq!(
        global_limit(
            Some(2_080.0),
            Some(FogClip {
                enabled: false,
                far: 900.0
            })
        ),
        2_080.0
    );
    assert_eq!(global_limit(None, None), f32::INFINITY);
    // Fog clip (Tessera: S off): fog far 900 m with G = 2,080 m gives 900 m.
    assert_eq!(
        global_limit(
            Some(2_080.0),
            Some(FogClip {
                enabled: true,
                far: 900.0
            })
        ),
        900.0
    );
}

#[test]
fn worked_example_mesh_8_7() {
    // mesh.md §8.7: d0 80, d2 16, d3 80; ObjectDrawDistance 3, StencilShadows 3,
    // ModelDetail 3; G = 2,080: D = 112 m; shadows to 22.4 m; combined mesh from 80 m.
    let s = DistanceSettings {
        model_detail: DetailLevel::VeryHigh,
        object_draw_distance: DetailLevel::VeryHigh,
        stencil_shadows: 3,
        ..DistanceSettings::default()
    };
    let v = table().values(Some(0xC), false);
    assert!(approx(v.draw_distance(2_080.0, &s), 112.0, 1e-3));
    assert!(approx(v.shadow_distance(2_080.0, &s), 22.4, 1e-4));
    assert_eq!(v.combined_mesh_distance(&s), 80.0);
    assert_eq!(v.draw_distance(90.0, &s), 90.0);
    assert_eq!(
        v.draw_distance(
            global_limit(
                Some(2_080.0),
                Some(FogClip {
                    enabled: true,
                    far: 40.0
                })
            ),
            &s
        ),
        60.0
    );
    // Smoothing at F = 60 with d_s = 100 and d = 40: k = 0.0499998, next d_s = 97.0.
    assert!(approx(smoothing_factor(60.0), 3.0 / 60.0002, 1e-9));
    let mut sm = DistanceSmoother::new();
    assert_eq!(sm.update(100.0, 60.0, false), 100.0);
    assert!(approx(sm.update(40.0, 60.0, false), 97.0, 1e-3));
    // A skinned model uses d at once.
    assert_eq!(sm.update(40.0, 60.0, true), 40.0);
    sm.reset();
    assert_eq!(sm.value(), None);
    assert_eq!(smoothing_factor(0.0), 1.0);
    assert_eq!(smoothing_factor(f32::NAN), 1.0);
}

#[test]
fn worked_example_detail_levels() {
    // mesh.md §8.4: n = 3, R = 60 m.
    assert_eq!(detail_level(3, 60.0, 10.0, true), 0);
    assert_eq!(detail_level(3, 60.0, 20.0, true), 1);
    assert_eq!(detail_level(3, 60.0, 59.0, true), 1);
    assert_eq!(detail_level(3, 60.0, 60.0, true), 2);
    assert_eq!(detail_level(3, 60.0, 600.0, true), 2);
    assert_eq!(detail_level(1, 60.0, 600.0, true), 0);
    assert_eq!(detail_level(0, 60.0, 600.0, true), 0);
    assert_eq!(detail_level(3, 60.0, 600.0, false), 0);
    // Level start table: n = 4: 0.1835 R, 0.4226 R, R; n = 6: …, 0.5528 R, R.
    let r = 1.0;
    assert!(approx(level_start_distance(1, 3, r), 0.2929, 1e-4));
    assert!(approx(level_start_distance(1, 4, r), 0.1835, 1e-4));
    assert!(approx(level_start_distance(2, 4, r), 0.4226, 1e-4));
    assert_eq!(level_start_distance(3, 4, r), 1.0);
    assert!(approx(level_start_distance(4, 6, r), 0.5528, 1e-4));
    assert_eq!(level_start_factor(0, 6), 1.0);
    // The level changes exactly at those distances.
    for n in [2_usize, 3, 4, 6] {
        for k in 1..n {
            let at = level_start_distance(k, n, 100.0);
            assert_eq!(detail_level(n, 100.0, at + 1e-3, true), k, "n {n} k {k}");
            assert_eq!(detail_level(n, 100.0, at - 0.5, true), k - 1, "n {n} k {k}");
        }
    }
    // Hostile values.
    assert_eq!(detail_level(3, f32::NAN, 10.0, true), 2);
    assert_eq!(detail_level(3, 60.0, f32::NAN, true), 2);
    assert_eq!(detail_level(3, 0.0, 0.0, true), 0);
    assert_eq!(detail_level(3, -5.0, 10.0, true), 2);
}

#[test]
fn class_values_component_defaults_and_player() {
    let t = table();
    let s = DistanceSettings::default();
    // No class: the component's initial values 200 m and 60 m; no ×3 for the player.
    let none = t.values(None, true);
    assert_eq!(none, LodValues(COMPONENT_DEFAULTS));
    assert_eq!(none.draw_distance(2_080.0, &s), 200.0);
    assert_eq!(none.detail_range(&s), 60.0);
    // A class without an entry keeps them, and the ×3 still runs for the player.
    assert_eq!(t.values(Some(0x99), false).d0(), 200.0);
    let p = t.values(Some(0x99), true);
    assert_eq!(
        (p.d0(), p.d1(), p.0[5], p.0[6]),
        (600.0, 180.0, 3_000.0, 1_100.0)
    );
    // PlayerAvatar-like class 80 / 26: the player's own character draws to 240 m with a
    // detail range of 78 m (walker.md §11.4.2).
    let me = t.values(Some(0xC), true);
    assert_eq!((me.d0(), me.d1()), (240.0, 78.0));
    // Non-finite entry values keep the initial value.
    assert_eq!(t.values(Some(0xD), false).d0(), 200.0);
    assert_eq!(t.values(Some(0xD), false).d1(), 10.0);
}

#[test]
fn worked_example_class_of_a_placement() {
    // walker.md §11.12: class A (300, 100), class B (45, 30).
    const LOD: u32 = 0x5100_0001;
    let schema: HashMap<u32, PropertyType> =
        [(LOD, PropertyType::from_code(8))].into_iter().collect();
    let set = |class: Option<u32>| {
        let entries: Vec<(u32, TestValue)> = class
            .map(|c| vec![(LOD, TestValue::Enum(c))])
            .unwrap_or_default();
        PropertySet::parse(&property_set(&entries), &schema).unwrap()
    };
    let chain = |levels: &[Option<u32>]| ObjectProperties {
        levels: levels
            .iter()
            .enumerate()
            .map(|(i, c)| crate::scenery::ChainLevel {
                did: Did(0x4700_0001 + u32::try_from(i).unwrap()),
                sets: vec![set(*c)],
            })
            .collect(),
        incomplete: false,
    };
    let t = table();
    let s = DistanceSettings::default();
    // (1) The SED's own set holds A, the 0x47 record B: class A, D = 300.
    let c = placement_lod_class(LOD, None, Some(&set(Some(0xA))), Some(&chain(&[Some(0xB)])));
    assert_eq!(c, Some(0xA));
    assert_eq!(t.values(c, false).draw_distance(2_080.0, &s), 300.0);
    // (2) Only the parent 0x47 holds B: class B, 45 m.
    let c = placement_lod_class(
        LOD,
        None,
        Some(&set(None)),
        Some(&chain(&[None, Some(0xB)])),
    );
    assert_eq!(t.values(c, false).draw_distance(2_080.0, &s), 45.0);
    // (3) Nowhere: 200 m and 60 m; at ObjectDrawDistance Low, 120 m.
    let c = placement_lod_class(LOD, None, Some(&set(None)), Some(&chain(&[None])));
    assert_eq!(c, None);
    let low = DistanceSettings {
        object_draw_distance: DetailLevel::Low,
        ..s
    };
    assert!(approx(
        t.values(c, false).draw_distance(2_080.0, &low),
        120.0,
        1e-4
    ));
    // (4) A class without a table entry keeps 200 m and 60 m.
    let c = placement_lod_class(LOD, None, Some(&set(Some(0x77))), None);
    assert_eq!(t.values(c, false).d0(), 200.0);
    // The instance set comes first.
    let c = placement_lod_class(LOD, Some(&set(Some(0xB))), Some(&set(Some(0xA))), None);
    assert_eq!(c, Some(0xB));
}

/// `water.md` WA19 (§14 (a), W49–W50; `decals.md` §6.5 "Objects"): `Render_DepthBias_Type`
/// is found in the entity lookup order. A SED with an instance set, a value on its 0x47
/// object and none in either set gets the object's value; a value in both sets and on the
/// object gives the sets' value (the one SED of the census with 4 on the SED and 3 on its
/// object gets 4).
#[test]
fn depth_bias_type_follows_the_entity_lookup_order() {
    const BIAS: u32 = 0x5100_0002;
    let schema: HashMap<u32, PropertyType> =
        [(BIAS, PropertyType::from_code(8))].into_iter().collect();
    let set = |ty: Option<u32>| {
        let entries: Vec<(u32, TestValue)> = ty
            .map(|c| vec![(BIAS, TestValue::Enum(c))])
            .unwrap_or_default();
        PropertySet::parse(&property_set(&entries), &schema).unwrap()
    };
    let chain = |ty: Option<u32>| ObjectProperties {
        levels: vec![crate::scenery::ChainLevel {
            did: Did(0x4700_C32A),
            sets: vec![set(ty)],
        }],
        incomplete: false,
    };
    let t = placement_lod_class(
        BIAS,
        Some(&set(None)),
        Some(&set(None)),
        Some(&chain(Some(3))),
    );
    assert_eq!(t, Some(3), "the object's value");
    let t = placement_lod_class(
        BIAS,
        Some(&set(Some(4))),
        Some(&set(Some(4))),
        Some(&chain(Some(3))),
    );
    assert_eq!(t, Some(4), "the sets' value");
    let t = placement_lod_class(BIAS, Some(&set(None)), Some(&set(None)), Some(&chain(None)));
    assert_eq!(t, None, "no source: the caller takes entry 0");
}

#[test]
fn fades_take_one_and_a_half_seconds() {
    let mut f = DrawFade::new();
    assert_eq!(f.opacity(), 0.0);
    // First frame: at once.
    assert_eq!(f.update(50.0, 100.0, 0.016, 0.0, false, true), 1.0);
    // Crossing outward: linear over 1.5 s.
    assert!(approx(
        f.update(150.0, 100.0, 0.75, 1.0, false, true),
        0.5,
        1e-6
    ));
    assert!(f.is_drawn());
    assert_eq!(f.update(150.0, 100.0, 0.75, 1.0, false, true), 0.0);
    assert!(!f.is_drawn());
    // Back in.
    assert!(approx(
        f.update(10.0, 100.0, 0.3, 1.0, false, true),
        0.2,
        1e-6
    ));
    // A camera jump of more than 25 m resets: at once.
    assert_eq!(f.update(10.0, 100.0, 0.016, 30.0, false, true), 1.0);
    assert_eq!(f.update(500.0, 100.0, 0.016, 25.5, false, true), 0.0);
    // Exactly 25 m is not a jump.
    let mut g = DrawFade::new();
    g.update(10.0, 100.0, 0.0, 0.0, false, true);
    assert!(g.update(500.0, 100.0, 0.15, 25.0, false, true) > 0.0);
    // An explicit reset, or distance-based levels disabled.
    assert_eq!(g.update(500.0, 100.0, 0.0, 0.0, true, true), 0.0);
    assert_eq!(g.update(500.0, 100.0, 0.0, 0.0, false, false), 1.0);
    // Non-finite input never panics and counts as far.
    let mut h = DrawFade::new();
    assert_eq!(
        h.update(f32::NAN, 100.0, f32::NAN, f32::NAN, false, true),
        0.0
    );
}

#[test]
fn nearest_point_of_a_box() {
    assert_eq!(aabb_distance([0.0; 3], [-1.0; 3], [1.0; 3]), 0.0);
    assert_eq!(aabb_distance([5.0, 0.0, 0.0], [-1.0; 3], [1.0; 3]), 4.0);
    assert_eq!(aabb_distance([4.0, 5.0, 0.0], [0.0; 3], [1.0; 3]), 5.0);
    assert_eq!(
        aabb_distance([f32::NAN, 0.0, 0.0], [0.0; 3], [1.0; 3]),
        f32::INFINITY
    );
}
