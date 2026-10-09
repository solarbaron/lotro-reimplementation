// SPDX-License-Identifier: GPL-3.0-or-later
//! Synthetic tests of the decal parameters, builder and shading (`decals.md` §3, §4, §6,
//! §7, §10 builder invariants 6–11, §11 worked examples).

use std::collections::HashMap;

use tessera_formats::gameplay::common::build::{array, entry, set, structure};
use tessera_formats::setup::Quaternion;
use tessera_formats::{Did, PropertySet, PropertyValue};

use super::shading::*;
use super::*;

fn pids() -> DecalPids {
    let m: HashMap<String, u32> = DecalPids::LABELS
        .iter()
        .enumerate()
        .map(|(i, l)| ((*l).to_owned(), 0x100 + u32::try_from(i).unwrap()))
        .collect();
    DecalPids::resolve(&m)
}

fn pid(p: Pid) -> u32 {
    p.id().unwrap()
}

/// Example 1's frame: a turn of −90° about X, so Y′ points down.
fn down() -> Quaternion {
    Quaternion {
        w: std::f32::consts::FRAC_1_SQRT_2,
        x: -std::f32::consts::FRAC_1_SQRT_2,
        y: 0.0,
        z: 0.0,
    }
}

fn close(a: f32, b: f32) -> bool {
    (a - b).abs() < 1e-4
}

fn vertex(p: [f32; 3], n: [f32; 3]) -> ReceiverVertex {
    ReceiverVertex {
        position: p,
        normal: n,
    }
}

/// Two terrain triangles of a flat square at height `z`, half-width `w` around (80, 80).
fn flat(z: f32, w: f32) -> Vec<ReceiverTriangle> {
    let up = [0.0, 0.0, 1.0];
    let a = vertex([80.0 - w, 80.0 - w, z], up);
    let b = vertex([80.0 + w, 80.0 - w, z], up);
    let c = vertex([80.0 + w, 80.0 + w, z], up);
    let d = vertex([80.0 - w, 80.0 + w, z], up);
    vec![
        ReceiverTriangle {
            kind: ReceiverKind::Terrain,
            vertices: [a, b, c],
        },
        ReceiverTriangle {
            kind: ReceiverKind::Terrain,
            vertices: [a, c, d],
        },
    ]
}

#[test]
fn parameters_collect_in_order() {
    let p = pids();
    // The marker chain root first, then the nearer level, then the SED.
    let root = set(vec![
        entry(pid(p.material), PropertyValue::DidRef(Did(0x3100_0001))),
        entry(pid(p.lighting_blend), PropertyValue::Enum(5)),
    ]);
    let near = set(vec![
        entry(pid(p.lighting_blend), PropertyValue::Enum(4)),
        // A value of the wrong type is not assigned.
        entry(pid(p.stick_to_cells), PropertyValue::Int(0)),
    ]);
    let sed = set(vec![
        entry(pid(p.ortho_scale), PropertyValue::Vector3([4.0, 2.0, 6.0])),
        entry(pid(p.colour), PropertyValue::Colour([0, 128, 255, 204])),
    ]);
    let chain: [&PropertySet; 2] = [&root, &near];
    let d = DecalParams::collect(&p, Did(0x3100_0008), &chain, Some(&sed));
    assert_eq!(d.material, Did(0x3100_0001));
    assert_eq!(d.lighting_blend, 4);
    assert!(d.stick_to_cells);
    assert_eq!(d.ortho_scale, [4.0, 2.0, 6.0]);
    assert_eq!(d.colour, [0, 128, 255, 204]);
    let i = DecalParams::initial(Did(0x3100_0008));
    assert_eq!((i.lod_class, i.depth_bias, i.render_phase), (1, 1, 1));
    assert!(close(i.backface_culling_angle, 85.0));
    assert!(p.is_parameter(pid(p.depth_bias)) && !p.is_parameter(pid(p.allow_decals)));
    // The chain helper returns the levels root first and stops at cycles.
    let chain = marker_chain(Did(0x4700_0002), |d| match d.0 {
        0x4700_0002 => Some((near.clone(), Some(Did(0x4700_0001)))),
        0x4700_0001 => Some((root.clone(), Some(Did(0x4700_0002)))),
        _ => None,
    });
    assert_eq!(chain, vec![root.clone(), near.clone()]);
}

#[test]
fn worked_example_1_ground_decal() {
    let b = DecalBox::new([80.0, 80.0, 105.0], down(), [4.0, 2.0, 6.0]);
    let [x, y, z] = b.axes;
    assert!(close(x[0], 1.0) && close(y[2], -1.0) && close(z[1], 1.0));
    assert!(close(b.center[2], 104.0));
    assert_eq!(b.extents, [4.0, 2.0, 6.0]);
    let mut params = DecalParams::initial(Did(0x3100_0008));
    params.ortho_scale = [4.0, 2.0, 6.0];
    let mesh = build_mesh(&params, &b, flat(104.0, 10.0), None);
    // The 4 m × 6 m rectangle, lifted.
    assert!(mesh.triangle_count() >= 2);
    let mut corner_82_83 = None;
    for v in &mesh.vertices {
        assert!(close(v.position[2], 104.008));
        assert!(v.position[0] >= 78.0 - 1e-4 && v.position[0] <= 82.0 + 1e-4);
        assert!(v.position[1] >= 77.0 - 1e-4 && v.position[1] <= 83.0 + 1e-4);
        if close(v.position[0], 82.0) && close(v.position[1], 83.0) {
            corner_82_83 = Some(v.uv);
        }
    }
    let uv = corner_82_83.unwrap();
    assert!(close(uv[0], 1.0) && close(uv[1], 0.0));
    assert!(mesh.center.is_none());
    // A wall in the box with normal (1, 0, 0) is dropped.
    let wall = ReceiverTriangle {
        kind: ReceiverKind::Object,
        vertices: [
            vertex([80.0, 79.0, 103.5], [1.0, 0.0, 0.0]),
            vertex([80.0, 81.0, 103.5], [1.0, 0.0, 0.0]),
            vertex([80.0, 80.0, 104.5], [1.0, 0.0, 0.0]),
        ],
    };
    let m = build_mesh(&params, &b, [wall], None);
    assert_eq!((m.triangle_count(), m.report.culled), (0, 1));
}

#[test]
fn worked_example_2_zero_depth() {
    let b = DecalBox::new([80.0, 80.0, 105.0], down(), [4.0, 0.0, 6.0]);
    assert!(close(b.center[2], 105.0));
    assert_eq!(b.extents, [4.0, 1.0, 6.0]);
    let params = DecalParams::initial(Did(1));
    assert_eq!(
        build_mesh(&params, &b, flat(104.0, 10.0), None).triangle_count(),
        0
    );
    let (lo, hi) = b.scene_bounds();
    assert!(close(lo[2], 105.0) && close(hi[2], 105.0));
    // Invariant 9: below 0.1 gives 1.0, exactly 0.1 gives 0.1.
    let b = DecalBox::new([0.0; 3], Quaternion::IDENTITY, [0.099, 0.1, 0.0]);
    assert_eq!(b.extents, [1.0, 0.1, 1.0]);
}

#[test]
fn builder_invariants() {
    let b = DecalBox::new([80.0, 80.0, 105.0], down(), [4.0, 2.0, 6.0]);
    let params = DecalParams::initial(Did(1));
    // Invariant 8: a triangle inside the box yields itself, lifted.
    let up = [0.0, 0.0, 1.0];
    let inside = ReceiverTriangle {
        kind: ReceiverKind::Object,
        vertices: [
            vertex([79.0, 79.0, 104.0], up),
            vertex([81.0, 79.0, 104.0], up),
            vertex([80.0, 81.0, 104.0], up),
        ],
    };
    let m = build_mesh(&params, &b, [inside], None);
    assert_eq!(m.triangle_count(), 1);
    for (o, i) in m.vertices.iter().zip(&inside.vertices) {
        assert!(close(o.position[2], i.position[2] + LIFT));
        assert!(close(o.position[0], i.position[0]) && close(o.position[1], i.position[1]));
    }
    // Invariants 6 and 7: a large tilted triangle crossing every plane.
    let n = [0.3, 0.2, 0.93];
    let big = ReceiverTriangle {
        kind: ReceiverKind::Object,
        vertices: [
            vertex([60.0, 70.0, 100.0], n),
            vertex([100.0, 75.0, 108.0], n),
            vertex([78.0, 100.0, 104.0], n),
        ],
    };
    let m = build_mesh(&params, &b, [big], None);
    assert!(m.triangle_count() <= 7);
    for v in &m.vertices {
        let before = [
            v.position[0] - v.normal[0] * LIFT,
            v.position[1] - v.normal[1] * LIFT,
            v.position[2] - v.normal[2] * LIFT,
        ];
        assert!(b.contains(before, 1e-4));
        assert!(v.uv.iter().all(|c| (-1e-5..=1.0 + 1e-5).contains(c)));
    }
    // Invariant 11: terrain follows UseLandscapeBackfaceCulling, objects do not.
    let wall_terrain = ReceiverTriangle {
        kind: ReceiverKind::Terrain,
        ..inside
    };
    let mut steep = inside;
    for v in &mut steep.vertices {
        v.normal = [1.0, 0.0, 0.0];
    }
    let steep_terrain = ReceiverTriangle {
        kind: ReceiverKind::Terrain,
        ..steep
    };
    let mut p2 = params;
    p2.use_landscape_backface_culling = false;
    assert_eq!(
        build_mesh(&p2, &b, [steep_terrain], None).triangle_count(),
        1
    );
    assert_eq!(build_mesh(&p2, &b, [steep], None).triangle_count(), 0);
    assert_eq!(
        build_mesh(&p2, &b, [wall_terrain], None).triangle_count(),
        1
    );
    // Precise culling uses the geometric normal (here up, kept).
    let mut p3 = params;
    p3.use_precise_backface_culling = true;
    assert_eq!(build_mesh(&p3, &b, [steep], None).triangle_count(), 1);
    // Water flattens collected vertices onto the water surface.
    let mut p4 = params;
    p4.stick_to_water = true;
    let m = build_mesh(&p4, &b, flat(103.5, 1.0), Some(104.2));
    assert!(m.vertices.iter().all(|v| close(v.position[2], 104.208)));
    // Non-finite input is dropped.
    let mut bad = inside;
    bad.vertices[0].position[0] = f32::NAN;
    assert_eq!(build_mesh(&params, &b, [bad], None).report.invalid, 1);
}

#[test]
fn lit_decal_has_centre_and_mean_normal() {
    let b = DecalBox::new([80.0, 80.0, 105.0], down(), [4.0, 2.0, 6.0]);
    let mut params = DecalParams::initial(Did(1));
    params.lighting_blend = 5;
    let m = build_mesh(&params, &b, flat(104.0, 10.0), None);
    let c = m.center.unwrap();
    assert!(close(c[0], 80.0) && close(c[1], 80.0) && close(c[2], 104.008));
    let n = m.mean_normal.unwrap();
    assert!(close(n[2], 1.0));
}

#[test]
fn worked_example_3_colour() {
    let stored = [0, 128, 255, 204];
    let l = [0.6, 0.6, 0.5, 1.0];
    assert_eq!(decal_colour(stored, l, 1.0, 0), [255, 128, 0, 204]);
    assert_eq!(decal_colour(stored, l, 1.0, 5), [153, 76, 0, 204]);
    assert_eq!(decal_colour(stored, l, 1.0, 1)[3], 115);
    assert_eq!(decal_colour(stored, l, 1.0, 2)[3], 59);
    assert_eq!(decal_colour(stored, l, 1.0, 4), [255, 255, 127, 204]);
    // Invariant 10: mode 0 ignores the light; alpha follows the opacity.
    assert_eq!(decal_colour(stored, [0.0; 4], 0.5, 0), [255, 128, 0, 102]);
    assert_eq!(
        decal_colour(stored, l, 1.0, 99),
        decal_colour(stored, l, 1.0, 0)
    );
}

#[test]
fn worked_example_4_light() {
    let light = DecalLight {
        kind: DecalLightKind::Point {
            position: [80.0, 84.0, 107.0],
            range: 10.0,
            falloff: 0.5,
        },
        colour: [1.0, 0.8, 0.5],
        intensity: 0.9,
    };
    let l = decal_light(
        [0.2, 0.2, 0.25],
        &[light],
        [0.0, 0.0, 1.0],
        [80.0, 80.0, 104.0],
        [1.0; 4],
    );
    assert!(close(l[0], 0.44) && close(l[1], 0.392) && close(l[2], 0.37) && close(l[3], 1.0));
}

#[test]
fn worked_example_5_and_6_distance_and_depth_bias() {
    assert!(close(
        draw_range(2080.0, lod_distance(Some(50.0), 30.0), 1.4),
        70.0
    ));
    assert!(close(
        draw_range(2080.0, lod_distance(None, INITIAL_LOD_DISTANCE), 1.4),
        42.0
    ));
    let m: HashMap<String, u32> = DepthBiasPids::LABELS
        .iter()
        .enumerate()
        .map(|(i, l)| ((*l).to_owned(), 0x200 + u32::try_from(i).unwrap()))
        .collect();
    let p = DepthBiasPids::resolve(&m);
    let def = |index: u32, slope: f32| {
        entry(
            pid(p.definition),
            structure(vec![
                entry(pid(p.index), PropertyValue::Enum(index)),
                entry(pid(p.slope_scale), PropertyValue::Float(slope)),
            ]),
        )
    };
    let engine = set(vec![entry(
        pid(p.list),
        array(vec![def(1, -1.0), def(13, -0.4)]),
    )]);
    let game = set(vec![entry(
        pid(p.list),
        array(vec![def(3, -0.8), def(1, -2.0), def(16, -9.0)]),
    )]);
    let (t, skipped) =
        DepthBiasTable::build(&p, &depth_bias_sets(Some(&engine), Some(&game)), false);
    assert_eq!(skipped, 1);
    assert!(close(t.get(1).slope_scale, -2.0));
    assert!(close(t.get(3).slope_scale, -0.8));
    assert!(close(t.get(13).slope_scale, -0.4));
    assert_eq!(t.get(0), DepthBias::default());
    assert_eq!(t.get(99), DepthBias::default());
    let (n, _) = DepthBiasTable::build(&p, &depth_bias_sets(Some(&engine), Some(&game)), true);
    assert!(close(n.get(1).slope_scale, 2.0));
    assert!(close(slope_for_depth(-1.0, true), 1.0));
}

#[test]
fn opacity_and_receivers() {
    assert!(close(depth_opacity(1.0, 1.0, 2.0), 1.0));
    assert!(close(depth_opacity(10.0, 5.0, 7.5), 0.5));
    assert!(close(depth_opacity(10.0, 0.0, 9.5), 0.5));
    assert!(close(height_alpha(0.8, 104.5, 104.0, 1.0), 0.4));
    assert!(close(height_alpha(0.8, 110.0, 104.0, 0.0), 0.0));
    assert!(entity_accepts_decals(None, None));
    assert!(!entity_accepts_decals(Some(false), None));
    assert!(entity_accepts_decals(Some(false), Some(true)));
    assert!(!cell_accepts_decals(Some(false)));
    assert!(cell_accepts_decals(None));
}
