// SPDX-License-Identifier: GPL-3.0-or-later
//! Regression tests for hostile-but-decodable values: issue-cap behaviour of validation,
//! skin reduction with extreme weights and bounds of extreme (finite) positions. All
//! meshes are synthetic (`tessera_mesh::synth`), edited by hand.

use tessera_mesh::synth::MeshSpec;
use tessera_mesh::validate::MAX_ISSUES;
use tessera_mesh::{
    Aabb, BoundingSphere, DecodeOptions, GeometryLimits, IssueClass, IssueKind, LevelKind, Mesh,
    MeshError, Skin,
};

/// A mesh whose first `junk` submeshes are drawn but full of NaNs (quality issues only),
/// followed by the clean original submesh, with level 0 of an unknown kind (a structural
/// issue reported after every per-submesh check).
fn quality_flood(junk: usize) -> (Mesh, u16) {
    let spec = MeshSpec::single(0x3000_0112, 12);
    let one = spec.build().unwrap();
    let mut m = one.clone();
    m.submeshes.clear();
    m.index_buffers.clear();
    m.surfaces.clear();
    m.levels[0].draws.clear();
    let draw = one.levels[0].draws[0];
    for i in 0..=junk {
        let mut sm = one.submeshes[0].clone();
        if i < junk {
            sm.streams.positions.fill([f32::NAN; 3]);
            if let Some(n) = &mut sm.streams.normals {
                n.fill([f32::NAN; 3]);
            }
            sm.stored_aabb.min = [f32::NAN; 3];
            sm.stored_aabb.max = [f32::NAN; 3];
        }
        m.submeshes.push(sm);
        m.index_buffers.push(one.index_buffers[0].clone());
        m.surfaces.push(one.surfaces[0]);
        m.levels[0].draws.push(tessera_mesh::Draw {
            vertex_block: i as u32,
            index_buffer: i as u32,
            surface: i as u32,
            ..draw
        });
    }
    // The clean submesh is drawn last, so the collision vertices (draw 0) now refer to a
    // junk block: still in range, only quality checks see it.
    m.levels[0].kind = LevelKind::Other(7);
    (m, spec.stamp())
}

#[test]
fn structural_issues_survive_a_flood_of_quality_issues() {
    let (m, stamp) = quality_flood(400);
    let r = m.validate();
    let quality = r
        .issues
        .iter()
        .filter(|i| i.kind.class() == IssueClass::Quality)
        .count();
    assert_eq!(quality, MAX_ISSUES);
    assert!(r.dropped > 0);
    assert!(r.has_structural());
    assert!(
        r.issues
            .iter()
            .any(|i| i.kind == IssueKind::UnknownLevelKind)
    );

    // Strict decoding of the same record fails on the structural issue.
    let bytes = m.to_bytes().unwrap();
    let strict = DecodeOptions {
        strict: true,
        ..DecodeOptions::lossless()
    };
    match Mesh::decode(&bytes, Some(stamp), &strict) {
        Err(MeshError::Invalid(issue)) => assert_eq!(issue.kind, IssueKind::UnknownLevelKind),
        other => panic!("expected Invalid(UnknownLevelKind), got {other:?}"),
    }
    assert!(Mesh::decode(&bytes, Some(stamp), &DecodeOptions::lossless()).is_ok());

    // A hand-edited stream of the last submesh is reported after the flood (the
    // `mesh_decode` fuzz harness relies on this).
    let mut edited = m.clone();
    let last = edited.submeshes.len() - 1;
    edited.submeshes[last].streams.positions.pop();
    assert!(
        edited
            .validate()
            .issues
            .iter()
            .any(|i| i.kind == IssueKind::StreamLength && i.submesh == Some(last as u32))
    );
}

#[test]
fn skin_four_never_overflows_into_nan_or_new_bones() {
    // K = 5; the kept sum is tiny and a dropped weight is large and negative, so the f32
    // scale total / kept overflows.
    for weights in [
        [1e-10, 0.0, 0.0, 0.0, -1e30],
        [1e-45, 0.0, 0.0, 0.0, -1.0],
        [1e-30, 1e-30, 0.0, 0.0, -3e38],
    ] {
        let skin = Skin {
            influences: 5,
            joints: vec![1, 7, 7, 7, 1],
            weights: weights.to_vec(),
        };
        let used = skin.bones_used();
        let (j, w) = skin.four(0);
        for k in 0..4 {
            assert!(w[k].is_finite(), "{weights:?} -> {w:?}");
            if w[k] != 0.0 {
                assert!(used.contains(&j[k]), "{weights:?} -> {j:?} {w:?}");
            }
        }
    }

    // Through a decoded-shaped mesh and `skinned_level`.
    let mut m = MeshSpec::single(0x0005_0002, 3).build().unwrap();
    let skin = m.submeshes[0].streams.skin.as_mut().unwrap();
    skin.joints[..5].copy_from_slice(&[1, 7, 7, 7, 1]);
    skin.weights[..5].copy_from_slice(&[1e-10, 0.0, 0.0, 0.0, -1e30]);
    let bones = usize::from(
        m.submeshes[0]
            .streams
            .skin
            .as_ref()
            .unwrap()
            .max_joint()
            .unwrap(),
    ) + 1;
    let sl = m
        .skinned_level(0, &GeometryLimits::default(), Some(bones))
        .unwrap();
    let streams = sl.part_streams(0).unwrap();
    for (j, w) in streams.joints.iter().zip(streams.weights) {
        for k in 0..4 {
            assert!(w[k].is_finite());
            if w[k] != 0.0 {
                assert!(sl.parts[0].bones_used.binary_search(&j[k]).is_ok());
            }
        }
    }
}

#[test]
fn bounds_of_huge_finite_positions_stay_finite() {
    let b = Aabb {
        min: [3e38, 0.0, -3e38],
        max: [3e38, 1.0, 3e38],
    };
    assert_eq!(b.center(), [3e38, 0.5, 0.0]);
    let wide = Aabb {
        min: [f32::MAX; 3],
        max: [f32::MAX; 3],
    };
    assert_eq!(wide.center(), [f32::MAX; 3]);
    let corners = [[-3e38f32; 3], [3e38; 3]];
    let all = Aabb::from_points(&corners).unwrap();
    let s = BoundingSphere::around(&all, &corners);
    assert!(s.center.iter().all(|c| c.is_finite()));
    assert!(s.radius.is_finite());
    assert_eq!(s.radius, f32::MAX);

    // Through a mesh: every position at x = 3e38, a matching stored box.
    let mut m = MeshSpec::single(0x3000_0112, 12).build().unwrap();
    for sm in &mut m.submeshes {
        for p in &mut sm.streams.positions {
            p[0] = 3e38;
        }
        sm.stored_aabb = Aabb::from_points(&sm.streams.positions).unwrap();
    }
    let bounds = m.level_bounds(0).unwrap();
    assert_eq!(bounds.sphere.center[0], 3e38);
    assert!(bounds.sphere.center.iter().all(|c| c.is_finite()));
    assert!(bounds.sphere.radius.is_finite());
    for p in &m.submeshes[0].streams.positions {
        let d: f64 = (0..3)
            .map(|i| (f64::from(p[i]) - f64::from(bounds.sphere.center[i])).powi(2))
            .sum::<f64>()
            .sqrt();
        assert!(d <= f64::from(bounds.sphere.radius));
    }
}
