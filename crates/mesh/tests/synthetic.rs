// SPDX-License-Identifier: GPL-3.0-or-later
//! Synthetic tests: hand-written records, generated meshes, round trips, error
//! categories, validation and level flattening. No game data is involved.

use proptest::prelude::*;
use tessera_mesh::synth::{LevelSpec, MeshSpec, SubmeshSpec, TreeSpec};
use tessera_mesh::{
    BuildByteRule, CollisionTree, DecodeOptions, Did, FailureCategory, GeometryLimits, Indices,
    IssueKind, LevelKind, MAX_SUBMESH_VERTICES, Mesh, MeshError, MeshHeader, Skin, VertexFormat,
};

/// The 27 format words of `mesh.md` §4.4.
const CENSUS: [u32; 27] = [
    0x0000_0112,
    0x0000_0152,
    0x3000_0112,
    0x3000_0152,
    0x3000_0212,
    0x3000_0252,
    0x3000_0372,
    0x3001_0112,
    0x3001_0152,
    0x3001_0212,
    0x3001_0252,
    0x3001_0352,
    0x3001_0712,
    0x3002_0112,
    0x3002_0152,
    0x3002_0212,
    0x3002_0252,
    0x3003_0112,
    0x3003_0152,
    0x3003_0212,
    0x3003_0252,
    0x3003_0352,
    0x3003_0712,
    0x3004_0112,
    0x3004_0152,
    0x3004_0212,
    0x3004_0252,
];

fn lossless() -> DecodeOptions {
    DecodeOptions::lossless()
}

fn u32s(out: &mut Vec<u8>, v: &[u32]) {
    for x in v {
        out.extend_from_slice(&x.to_le_bytes());
    }
}

fn f32s(out: &mut Vec<u8>, v: &[f32]) {
    for x in v {
        out.extend_from_slice(&x.to_le_bytes());
    }
}

/// A record written byte by byte from the tables of `mesh.md` §3–§10, independent of the
/// crate's encoder: one submesh with `vfmt` 0x30020152 (the §4.4 worked example) and
/// three vertices, one detail level with a collision mesh and tree code, then a proxy
/// level.
fn handmade(build_byte: bool) -> Vec<u8> {
    let mut r = Vec::new();
    u32s(&mut r, &[0x0600_1234, 0x1000_0005, 1, 0x3100_0042, 1]);
    // Submesh 0.
    u32s(&mut r, &[0x3002_0152, 3]);
    let pos = [[0.0f32, 0.0, 0.0], [1.0, 0.0, 0.5], [0.0, 2.0, -1.0]];
    for (i, p) in pos.iter().enumerate() {
        f32s(&mut r, p); // position @0
        f32s(&mut r, &[0.0, 0.0, 1.0]); // normal @12
        r.extend_from_slice(&[10 + i as u8, 20, 30, 200]); // colour @24: B G R A
        f32s(&mut r, &[0.25 * i as f32, -3.0]); // UV @28
        f32s(&mut r, &[1.0, 0.0, 0.0]); // tangent @36
        f32s(&mut r, &[0.0, if i == 2 { -1.0 } else { 1.0 }, 0.0]); // binormal @48
        r.extend_from_slice(&[7, if i == 0 { 0 } else { 9 }]); // indices @60
        if i == 0 {
            f32s(&mut r, &[1.0, 0.0]); // weights @62
        } else {
            f32s(&mut r, &[0.75, 0.25]);
        }
    }
    f32s(&mut r, &[0.0, 0.0, -1.0, 1.0, 2.0, 0.5]); // AABB
    // Influence list (`mesh.md` §6.2): vertices 2, 1, 0, each slot-0 bone once per non-zero
    // weight.
    u32s(&mut r, &[5, 7, 7, 7, 7, 7]);
    // Region C.
    u32s(&mut r, &[1, 3]);
    for i in [0u16, 1, 2] {
        r.extend_from_slice(&i.to_le_bytes());
    }
    // Region D: two levels.
    u32s(&mut r, &[2]);
    // Level 0: detail, one draw.
    u32s(&mut r, &[1, 1]);
    r.push(4);
    u32s(&mut r, &[0, 0, 0]);
    r.push(1); // has_collision_mesh
    u32s(&mut r, &[3]);
    for v in 0u16..3 {
        r.push(0);
        r.extend_from_slice(&v.to_le_bytes());
    }
    u32s(&mut r, &[1]);
    for x in [0u16, 0, 1, 1, 2, 2] {
        r.extend_from_slice(&x.to_le_bytes());
    }
    u32s(&mut r, &[3]);
    for x in [[0u16, 0, 1, 0xFFFF], [1, 0, 2, 0xFFFF], [2, 0, 0, 0xFFFF]] {
        for y in x {
            r.extend_from_slice(&y.to_le_bytes());
        }
    }
    r.push(1); // has_tree
    r.push(1); // has_code
    f32s(&mut r, &[-0.05, -2.05, -1.05, 4.0e6]);
    u32s(&mut r, &[4]);
    r.extend_from_slice(&[0x28, 1, 2, 3]);
    if build_byte {
        r.push(1);
    }
    // Level 1: proxy, same submesh would be drawn twice, so it draws it with no collision.
    u32s(&mut r, &[2, 1]);
    r.push(4);
    u32s(&mut r, &[0, 0, 0]);
    r.push(0);
    r.push(0);
    r
}

#[test]
fn handmade_record_decodes_field_by_field() {
    for (bb, stamp) in [
        (true, Some(39)),
        (false, Some(7)),
        (true, None),
        (false, None),
    ] {
        let bytes = handmade(bb);
        let m = Mesh::decode(&bytes, stamp, &lossless()).unwrap();
        assert_eq!(m.did, Did(0x0600_1234));
        assert!(m.is_skinning_flagged());
        assert_eq!(m.surfaces, vec![Did(0x3100_0042)]);
        let sm = &m.submeshes[0];
        assert_eq!(sm.format.stride(), 70);
        let st = &sm.streams;
        assert_eq!(st.positions[1], [1.0, 0.0, 0.5]);
        assert_eq!(st.normals.as_ref().unwrap()[2], [0.0, 0.0, 1.0]);
        // Stored B G R A → R G B A.
        assert_eq!(st.colors.as_ref().unwrap()[1], [30, 20, 11, 200]);
        assert_eq!(st.uv_sets.len(), 1);
        assert_eq!(st.uv_sets[0][2], [0.5, -3.0]);
        assert_eq!(st.binormals.as_ref().unwrap()[2], [0.0, -1.0, 0.0]);
        let skin = st.skin.as_ref().unwrap();
        assert_eq!(skin.joints_of(1), &[7, 9]);
        assert_eq!(skin.weights_of(1), &[0.75, 0.25]);
        assert_eq!(skin.joints_of(0), &[7, 0]);
        assert_eq!(sm.influence_list, vec![7, 7, 7, 7, 7]);
        assert!(sm.influence_list_matches());
        assert_eq!(m.index_buffers[0].indices, vec![0, 1, 2]);
        assert_eq!(m.levels.len(), 2);
        assert_eq!(m.levels[1].kind, LevelKind::Proxy);
        let cm = m.levels[0].collision.as_ref().unwrap();
        assert_eq!(cm.triangles[0].vertices, [0, 1, 2]);
        assert_eq!(cm.triangles[0].edges, [0, 1, 2]);
        assert_eq!(cm.edges[1].vertices, [1, 2]);
        let Some(CollisionTree::Code(t)) = &m.levels[0].tree else {
            panic!("tree code expected")
        };
        assert_eq!(t.code.as_deref(), Some(&[0x28u8, 1, 2, 3][..]));
        assert_eq!(t.build_byte, bb.then_some(1));
        match stamp {
            Some(s) => assert_eq!(m.build_byte_rule, BuildByteRule::Stamp(s)),
            None => assert_eq!(m.build_byte_rule, BuildByteRule::Fallback),
        }
        let b = m.bounds.unwrap();
        assert_eq!(b.aabb.min, [0.0, 0.0, -1.0]);
        assert_eq!(b.aabb.max, [1.0, 2.0, 0.5]);
        // The proxy level draws submesh 0 a second time: a structural issue, nothing else.
        let report = m.validate();
        let kinds: Vec<IssueKind> = report.issues.iter().map(|i| i.kind).collect();
        assert_eq!(kinds, vec![IssueKind::SubmeshDrawnTwice], "{report:?}");
        // Lossless.
        assert_eq!(m.to_bytes().unwrap(), bytes);
        // Header only.
        let h = MeshHeader::parse(&bytes).unwrap();
        assert_eq!(h.submesh_count, 1);
        assert_eq!(h, m.header());
    }
}

#[test]
fn wrong_stamp_is_an_error_not_a_panic() {
    // A stamp that disagrees with the data misplaces one byte.
    let with = handmade(true);
    let err = Mesh::decode(&with, Some(7), &lossless()).unwrap_err();
    assert!(
        matches!(
            err.category(),
            FailureCategory::Structure
                | FailureCategory::Truncated
                | FailureCategory::TrailingBytes
        ),
        "{err}"
    );
    let without = handmade(false);
    assert!(Mesh::decode(&without, Some(39), &lossless()).is_err());
    // Stamps in the unobserved gap use the fallback.
    for s in 34..=37 {
        let m = Mesh::decode(&with, Some(s), &lossless()).unwrap();
        assert_eq!(m.build_byte_rule, BuildByteRule::Fallback);
        assert!(Mesh::decode(&without, Some(s), &lossless()).is_ok());
    }
}

#[test]
fn every_census_format_round_trips() {
    for f in CENSUS {
        for build_byte in [true, false] {
            let mut spec = MeshSpec::single(f, 37);
            spec.build_byte = build_byte;
            let mesh = spec.build().unwrap();
            assert!(mesh.validate().is_clean(), "{f:#x}: {:?}", mesh.validate());
            let bytes = mesh.to_bytes().unwrap();
            let layout = VertexFormat::new(f).unwrap().layout();
            assert!(bytes.len() > 37 * layout.stride);
            for stamp in [Some(spec.stamp()), None] {
                let back = Mesh::decode(&bytes, stamp, &lossless()).unwrap();
                assert_eq!(back.submeshes, mesh.submeshes, "{f:#x}");
                assert_eq!(back.levels, mesh.levels, "{f:#x}");
                assert_eq!(back.bounds, mesh.bounds);
                assert_eq!(back.to_bytes().unwrap(), bytes);
            }
            // Strict decoding accepts a clean mesh.
            let strict = DecodeOptions {
                strict: true,
                ..lossless()
            };
            Mesh::decode(&bytes, Some(spec.stamp()), &strict).unwrap();
        }
    }
}

fn multi() -> MeshSpec {
    MeshSpec {
        did: 0x0600_0777,
        flags: 0x1000_0003,
        submeshes: vec![
            SubmeshSpec {
                format: 0x3000_0152,
                vertices: 10,
            },
            SubmeshSpec {
                format: 0x3002_0112,
                vertices: 20,
            },
            SubmeshSpec {
                format: 0x0000_0112,
                vertices: 8,
            },
            SubmeshSpec {
                format: 0x3000_0112,
                vertices: 6,
            },
        ],
        levels: vec![
            // Draw order differs from storage order (§8.2).
            LevelSpec::detail(vec![1, 0]),
            LevelSpec {
                tree: TreeSpec::NoCode,
                ..LevelSpec::detail(vec![3])
            },
            LevelSpec::proxy(vec![2]),
        ],
        build_byte: false,
    }
}

#[test]
fn multi_level_mesh_flattens_in_draw_order() {
    let spec = multi();
    let mesh = spec.build().unwrap();
    assert!(mesh.validate().is_clean(), "{:?}", mesh.validate());
    let bytes = mesh.to_bytes().unwrap();
    let back = Mesh::decode(&bytes, Some(spec.stamp()), &lossless()).unwrap();
    assert_eq!(back.levels, mesh.levels);
    assert_eq!(back.detail_levels().collect::<Vec<_>>(), vec![0, 1]);
    assert_eq!(back.triangle_count(0), 18 + 8);
    assert_eq!(back.vertex_count(0), 30);

    let g = back.level_geometry(0, &GeometryLimits::default()).unwrap();
    assert_eq!(g.vertex_count(), 30);
    assert_eq!(g.triangle_count(), 26);
    assert_eq!(g.parts.len(), 2);
    let (p0, p1) = (g.parts[0], g.parts[1]);
    assert_eq!((p0.submesh, p0.base_vertex, p0.vertex_count), (1, 0, 20));
    assert_eq!((p1.submesh, p1.base_vertex, p1.vertex_count), (0, 20, 10));
    assert_eq!(p0.surface, Some(back.surfaces[1]));
    assert_eq!((p0.first_index, p0.index_count), (0, 54));
    assert_eq!((p1.first_index, p1.index_count), (54, 24));
    assert!(p0.skinned && !p1.skinned);
    let Indices::U16(ix) = &g.indices else {
        panic!("u16 expected")
    };
    // Part 1's indices are offset by part 0's vertices.
    assert_eq!(
        ix[54] as usize,
        20 + back.index_buffers[0].indices[0] as usize
    );
    // Streams that only some parts have are filled.
    let colors = g.colors.as_ref().unwrap();
    assert_eq!(colors[0], [255; 4]); // submesh 1 has no colour
    assert_eq!(
        colors[20],
        back.submeshes[0].streams.colors.as_ref().unwrap()[0]
    );
    let w = g.weights.as_ref().unwrap();
    assert_eq!(w[25], [0.0; 4]); // unskinned part
    assert!((w[0].iter().sum::<f32>() - 1.0).abs() < 1e-6);
    assert!(g.tangents.is_some());
    assert_eq!(g.bounds, back.bounds);
    // Binormal (0, -1, 0) with normal ~+Z and tangent +X gives a negative bitangent sign.
    let t = g.tangents.as_ref().unwrap();
    assert_eq!(t[0][3], -1.0);
    assert_eq!(t[1][3], 1.0);

    // The proxy level uses a format without tangents.
    let gp = back.level_geometry(2, &GeometryLimits::default()).unwrap();
    assert!(gp.tangents.is_none());
    let err = back
        .level_geometry(3, &GeometryLimits::default())
        .unwrap_err();
    assert!(matches!(err, MeshError::NoSuchLevel { level: 3, count: 3 }));
    assert_eq!(err.category(), FailureCategory::Usage);
}

#[test]
fn normals_are_generated_when_absent() {
    // Position and one UV set only.
    let mesh = MeshSpec::single(0x0000_0102, 12).build().unwrap();
    assert!(mesh.submeshes[0].streams.normals.is_none());
    let bytes = mesh.to_bytes().unwrap();
    let back = Mesh::decode(&bytes, Some(39), &lossless()).unwrap();
    let g = back.level_geometry(0, &GeometryLimits::default()).unwrap();
    for n in &g.normals {
        let len = (n[0] * n[0] + n[1] * n[1] + n[2] * n[2]).sqrt();
        assert!((len - 1.0).abs() < 1e-4);
    }
}

#[test]
fn large_levels_switch_to_u32_and_respect_limits() {
    let spec = MeshSpec {
        submeshes: vec![
            SubmeshSpec {
                format: 0x3000_0112,
                vertices: 40_000,
            },
            SubmeshSpec {
                format: 0x3000_0112,
                vertices: 40_000,
            },
        ],
        levels: vec![LevelSpec::detail(vec![0, 1])],
        ..MeshSpec::single(0x112, 3)
    };
    let mesh = spec.build().unwrap();
    let g = mesh.level_geometry(0, &GeometryLimits::default()).unwrap();
    let Indices::U32(ix) = &g.indices else {
        panic!("u32 expected")
    };
    assert_eq!(ix.iter().copied().max(), Some(79_999));
    let small = GeometryLimits {
        max_vertices: 50_000,
        max_indices: u64::MAX,
        ..GeometryLimits::default()
    };
    let err = mesh.level_geometry(0, &small).unwrap_err();
    assert_eq!(err.category(), FailureCategory::LimitExceeded);
}

#[test]
fn repeated_draws_cannot_blow_up_flattening() {
    // A tiny record whose level draws the same block 5,000 times.
    let mut spec = MeshSpec::single(0x3000_0112, 60_000);
    spec.levels = vec![LevelSpec {
        collision: false,
        tree: TreeSpec::None,
        ..LevelSpec::detail(vec![0; 5_000])
    }];
    let mesh = spec.build().unwrap();
    let bytes = mesh.to_bytes().unwrap();
    let back = Mesh::decode(&bytes, Some(39), &lossless()).unwrap();
    let err = back
        .level_geometry(0, &GeometryLimits::default())
        .unwrap_err();
    assert!(matches!(err, MeshError::LimitExceeded { .. }));
}

#[test]
fn skin_four_keeps_largest_weights() {
    let skin = Skin {
        influences: 6,
        joints: vec![1, 2, 3, 4, 5, 6],
        weights: vec![0.05, 0.3, 0.05, 0.2, 0.25, 0.15],
    };
    let (j, w) = skin.four(0);
    assert_eq!(j, [2, 5, 4, 6]);
    assert!((w.iter().sum::<f32>() - 1.0).abs() < 1e-6);
    assert!(w[0] > w[1] && w[1] > w[2] && w[2] > w[3]);
    let short = Skin {
        influences: 2,
        joints: vec![3, 0],
        weights: vec![1.0, 0.0],
    };
    assert_eq!(short.four(0), ([3, 0, 0, 0], [1.0, 0.0, 0.0, 0.0]));
    assert_eq!(short.four(9), ([0; 4], [0.0; 4]));
}

// ---------------------------------------------------------------------------------------
// Errors and their categories
// ---------------------------------------------------------------------------------------

fn base_bytes() -> Vec<u8> {
    handmade(true)
}

fn cat(bytes: &[u8]) -> FailureCategory {
    Mesh::decode(bytes, Some(39), &lossless())
        .unwrap_err()
        .category()
}

/// Offsets inside `handmade`.
const SUBMESH_COUNT: usize = 16;
const VFMT: usize = 20;
const VCOUNT: usize = 24;
const VERTS: usize = 28;
const AABB: usize = VERTS + 3 * 70;
const INFL: usize = AABB + 24;
const IB_COUNT: usize = INFL + 4 + 20;
const INDICES: usize = IB_COUNT + 8;
const LEVELS: usize = INDICES + 6;
const DRAW0: usize = LEVELS + 12;
const HAS_COLL: usize = DRAW0 + 13;

fn put(bytes: &mut [u8], at: usize, v: u32) {
    bytes[at..at + 4].copy_from_slice(&v.to_le_bytes());
}

#[test]
fn every_truncation_is_an_error() {
    let bytes = base_bytes();
    for len in 0..bytes.len() {
        let err = Mesh::decode(&bytes[..len], Some(39), &lossless()).unwrap_err();
        assert!(
            matches!(
                err.category(),
                FailureCategory::Truncated | FailureCategory::Structure
            ),
            "{len}: {err}"
        );
    }
}

#[test]
fn categorised_failures() {
    let good = base_bytes();
    // Sanity: the offsets above point where they should.
    assert_eq!(
        u32::from_le_bytes(good[VFMT..VFMT + 4].try_into().unwrap()),
        0x3002_0152
    );
    assert_eq!(good[DRAW0], 4);
    assert_eq!(good[HAS_COLL], 1);

    let mut b = good.clone();
    b.push(0);
    assert_eq!(cat(&b), FailureCategory::TrailingBytes);

    let mut b = good.clone();
    put(&mut b, SUBMESH_COUNT, 2);
    assert_eq!(cat(&b), FailureCategory::Structure); // N ≠ S

    for bad in [0x3002_0150u32, 0x3002_01D2, 0x2002_0152, 0x3002_1152] {
        let mut b = good.clone();
        put(&mut b, VFMT, bad);
        assert_eq!(
            cat(&b),
            FailureCategory::UnsupportedVertexFormat,
            "{bad:#x}"
        );
    }

    let mut b = good.clone();
    put(&mut b, VCOUNT, MAX_SUBMESH_VERTICES + 1);
    assert_eq!(cat(&b), FailureCategory::LimitExceeded);

    let mut b = good.clone();
    put(&mut b, VCOUNT, 4000);
    assert_eq!(cat(&b), FailureCategory::Truncated);

    let mut b = good.clone();
    put(&mut b, INFL, 0xFFFF_FFFF);
    assert_eq!(cat(&b), FailureCategory::Truncated);

    let mut b = good.clone();
    put(&mut b, IB_COUNT, 2);
    assert_eq!(cat(&b), FailureCategory::Structure);

    let mut b = good.clone();
    b[INDICES + 2..INDICES + 4].copy_from_slice(&3u16.to_le_bytes());
    assert_eq!(cat(&b), FailureCategory::IndexOutOfRange);

    let mut b = good.clone();
    put(&mut b, IB_COUNT + 4, 2);
    assert_eq!(cat(&b), FailureCategory::Structure); // not a multiple of 3

    let mut b = good.clone();
    put(&mut b, LEVELS, 0);
    assert_eq!(cat(&b), FailureCategory::Structure);

    let mut b = good.clone();
    put(&mut b, LEVELS, 1_000_000);
    assert_eq!(cat(&b), FailureCategory::Truncated);

    let mut b = good.clone();
    put(&mut b, LEVELS + 8, 0);
    assert_eq!(cat(&b), FailureCategory::Structure); // empty draw list

    let mut b = good.clone();
    b[DRAW0] = 5;
    assert_eq!(cat(&b), FailureCategory::Structure);

    for field in 0..3 {
        let mut b = good.clone();
        put(&mut b, DRAW0 + 1 + 4 * field, 1);
        assert_eq!(cat(&b), FailureCategory::IndexOutOfRange);
    }

    let mut b = good.clone();
    b[HAS_COLL] = 2;
    assert_eq!(cat(&b), FailureCategory::Structure);

    let mut b = good.clone();
    put(&mut b, 8, 0x4000_0000);
    assert_eq!(cat(&b), FailureCategory::Truncated); // surface count

    let err = Mesh::decode_expecting(&good, Did(0x0600_0001), Some(39), &lossless()).unwrap_err();
    assert!(matches!(err, MeshError::DidMismatch { .. }));
    assert_eq!(err.category(), FailureCategory::Structure);
}

#[test]
fn draws_pairing_different_blocks_are_range_checked() {
    let spec = MeshSpec {
        submeshes: vec![
            SubmeshSpec {
                format: 0x3000_0112,
                vertices: 30,
            },
            SubmeshSpec {
                format: 0x3000_0112,
                vertices: 5,
            },
        ],
        levels: vec![LevelSpec::detail(vec![0, 1])],
        ..MeshSpec::single(0x112, 3)
    };
    let mut mesh = spec.build().unwrap();
    // Draw block 1 (5 vertices) with index buffer 0 (indices up to 29).
    mesh.levels[0].draws[1].index_buffer = 0;
    let bytes = mesh.to_bytes().unwrap();
    let err = Mesh::decode(&bytes, Some(39), &lossless()).unwrap_err();
    assert_eq!(err.category(), FailureCategory::IndexOutOfRange);
    // The other way round is fine, and only reported by validation.
    mesh.levels[0].draws[1].index_buffer = 1;
    mesh.levels[0].draws[0].index_buffer = 1;
    let bytes = mesh.to_bytes().unwrap();
    let back = Mesh::decode(&bytes, Some(39), &lossless()).unwrap();
    let kinds: Vec<IssueKind> = back.validate().issues.iter().map(|i| i.kind).collect();
    assert!(kinds.contains(&IssueKind::DrawIndicesDiffer));
    back.level_geometry(0, &GeometryLimits::default()).unwrap();
}

// ---------------------------------------------------------------------------------------
// Validation
// ---------------------------------------------------------------------------------------

fn issue_kinds(m: &Mesh) -> Vec<IssueKind> {
    m.validate().issues.iter().map(|i| i.kind).collect()
}

#[test]
fn validation_categorises_quality_issues() {
    let clean = MeshSpec::single(0x3002_0152, 12).build().unwrap();
    assert!(clean.validate().is_clean());

    let mut m = clean.clone();
    m.submeshes[0].stored_aabb.max[2] += 1.0;
    assert_eq!(issue_kinds(&m), vec![IssueKind::AabbMismatch]);

    let mut m = clean.clone();
    m.submeshes[0].streams.positions[3][0] = f32::NAN;
    let k = issue_kinds(&m);
    assert!(k.contains(&IssueKind::NonFinite(
        tessera_mesh::validate::Attribute::Position
    )));

    let mut m = clean.clone();
    m.submeshes[0].streams.normals.as_mut().unwrap()[0] = [0.0, 0.0, 2.0];
    assert_eq!(issue_kinds(&m), vec![IssueKind::NormalNotUnit]);

    let mut m = clean.clone();
    m.submeshes[0].streams.skin.as_mut().unwrap().weights[0] = 0.5;
    assert!(issue_kinds(&m).contains(&IssueKind::WeightSum));

    let mut m = clean.clone();
    {
        let skin = m.submeshes[0].streams.skin.as_mut().unwrap();
        // Vertex 0 uses one influence; its second slot is a zero slot.
        assert_eq!(skin.weights[1], 0.0);
        skin.joints[1] = 9;
    }
    assert_eq!(issue_kinds(&m), vec![IssueKind::UnusedSlotNonZeroIndex]);

    let mut m = clean.clone();
    m.submeshes[0].influence_list.pop();
    assert_eq!(issue_kinds(&m), vec![IssueKind::InfluenceListLength]);

    // The right length with the wrong content (mesh.md §6.2: the list is redundant).
    let mut m = clean.clone();
    m.submeshes[0].influence_list[0] ^= 1;
    assert_eq!(issue_kinds(&m), vec![IssueKind::InfluenceListContent]);

    let mut m = clean.clone();
    m.index_buffers[0].indices[0..3].copy_from_slice(&[1, 1, 2]);
    let k = issue_kinds(&m);
    assert!(k.contains(&IssueKind::DegenerateTriangle));
    assert!(k.contains(&IssueKind::UnusedVertex));
    assert!(k.contains(&IssueKind::IndexMinNotZero));

    let mut m = clean.clone();
    m.surfaces[0] = Did(0x4100_0001);
    assert_eq!(issue_kinds(&m), vec![IssueKind::NotSurfaceDid]);

    let mut m = clean.clone();
    m.did = Did(0x0700_0001);
    assert!(m.validate().has_structural());

    // Strict decoding does not fail on quality issues (a viewer may still draw, §14)...
    let strict = DecodeOptions {
        strict: true,
        ..lossless()
    };
    let mut m = clean.clone();
    m.submeshes[0].stored_aabb.min[0] -= 1.0;
    let bytes = m.to_bytes().unwrap();
    let decoded = Mesh::decode(&bytes, Some(39), &strict).unwrap();
    assert_eq!(issue_kinds(&decoded), vec![IssueKind::AabbMismatch]);
    // ...but turns the first structural one into an error.
    let mut m = clean.clone();
    m.did = Did(0x0700_0001);
    m.submeshes[0].stored_aabb.min[0] -= 1.0;
    let bytes = m.to_bytes().unwrap();
    let err = Mesh::decode(&bytes, Some(39), &strict).unwrap_err();
    assert_eq!(err.category(), FailureCategory::Invalid);
    assert!(
        matches!(err, MeshError::Invalid(i) if i.kind == IssueKind::NotMeshDid),
        "{err}"
    );
    assert!(Mesh::decode(&bytes, Some(39), &lossless()).is_ok());
}

#[test]
fn validation_categorises_structural_issues() {
    let spec = multi();
    let clean = spec.build().unwrap();

    let mut m = clean.clone();
    m.levels.swap(1, 2);
    assert!(issue_kinds(&m).contains(&IssueKind::ProxyBeforeDetail));

    let mut m = clean.clone();
    m.levels[0].collision = None;
    m.levels[0].collision_counts = None;
    assert_eq!(issue_kinds(&m), vec![IssueKind::DetailWithoutCollision]);

    let mut m = clean.clone();
    m.levels[2].draws.clear();
    assert_eq!(issue_kinds(&m), vec![IssueKind::SubmeshNotDrawn]);

    let mut m = clean.clone();
    m.levels[0].kind = LevelKind::Other(9);
    assert_eq!(issue_kinds(&m), vec![IssueKind::UnknownLevelKind]);

    let mut m = clean.clone();
    if let Some(CollisionTree::Code(t)) = &mut m.levels[0].tree {
        t.build_byte = Some(2);
    }
    // Spec has build_byte false; force one in to test the value check.
    let mut m2 = clean.clone();
    m2.levels[0].tree = m.levels[0].tree.clone().map(|t| match t {
        CollisionTree::Code(mut c) => {
            c.build_byte = Some(2);
            CollisionTree::Code(c)
        }
        other => other,
    });
    assert_eq!(issue_kinds(&m2), vec![IssueKind::BuildByteValue]);

    let mut m = clean.clone();
    m.levels[0].collision.as_mut().unwrap().vertices[0].vertex = 999;
    assert!(issue_kinds(&m).contains(&IssueKind::CollisionVertexOutOfRange));

    let mut m = clean.clone();
    m.levels[0].collision.as_mut().unwrap().vertices.swap(0, 1);
    assert_eq!(
        issue_kinds(&m),
        vec![IssueKind::CollisionVerticesNotAscending]
    );

    let mut m = clean.clone();
    m.levels[0].collision.as_mut().unwrap().edges[0].faces = [3, 1];
    let k = issue_kinds(&m);
    assert!(k.contains(&IssueKind::CollisionIndexOutOfRange));
    assert!(k.contains(&IssueKind::CollisionEdgeFaceOrder));
}

#[test]
fn render_only_options_skip_collision_but_count_it() {
    let mesh = multi().build().unwrap();
    let bytes = mesh.to_bytes().unwrap();
    let m = Mesh::decode(&bytes, Some(7), &DecodeOptions::render_only()).unwrap();
    assert!(m.levels[0].collision.is_none());
    assert_eq!(m.levels[0].collision_counts.unwrap().triangles, 1);
    let Some(CollisionTree::Code(t)) = &m.levels[0].tree else {
        panic!()
    };
    assert!(t.code.is_none());
    assert_eq!(t.code_len, 9);
    assert!(m.to_bytes().is_err());
    assert!(m.heap_size() < mesh.heap_size() + 4096);
    assert!(m.validate().is_clean());
}

#[test]
fn encoder_rejects_inconsistent_streams() {
    let mut m = MeshSpec::single(0x3000_0152, 5).build().unwrap();
    m.submeshes[0].streams.colors = None;
    let err = m.to_bytes().unwrap_err();
    assert!(matches!(err, MeshError::Encode(_)));
    assert_eq!(err.category(), FailureCategory::Usage);
    let mut m = MeshSpec::single(0x3001_0112, 5).build().unwrap();
    m.submeshes[0].streams.skin.as_mut().unwrap().weights.pop();
    assert!(m.to_bytes().is_err());
    let mut m = MeshSpec::single(0x3000_0112, 5).build().unwrap();
    m.surfaces.push(Did(0x3100_0000));
    assert!(m.to_bytes().is_err());
}

// ---------------------------------------------------------------------------------------
// Hostile input
// ---------------------------------------------------------------------------------------

fn seed_records() -> Vec<(Vec<u8>, u16)> {
    let mut out = vec![(handmade(true), 39), (handmade(false), 7)];
    let spec = multi();
    out.push((spec.build().unwrap().to_bytes().unwrap(), spec.stamp()));
    out.push((
        MeshSpec::single(0x3004_0252, 9)
            .build()
            .unwrap()
            .to_bytes()
            .unwrap(),
        39,
    ));
    out
}

fn exercise(bytes: &[u8], stamp: Option<u16>) {
    if let Ok(m) = Mesh::decode(bytes, stamp, &lossless()) {
        // Whatever decodes is reproduced exactly.
        assert_eq!(m.to_bytes().unwrap(), bytes);
        let _ = m.validate();
        for l in 0..m.levels.len() {
            let _ = m.level_geometry(l, &GeometryLimits::default());
        }
    }
    let _ = MeshHeader::parse(bytes);
}

proptest! {
    #![proptest_config(ProptestConfig::with_cases(512))]

    #[test]
    fn random_bytes_never_panic(bytes in proptest::collection::vec(any::<u8>(), 0..600), stamp in proptest::option::of(0u16..64)) {
        exercise(&bytes, stamp);
    }

    #[test]
    fn mutated_records_never_panic(
        which in 0usize..4,
        edits in proptest::collection::vec((any::<usize>(), any::<u8>()), 1..8),
        cut in any::<usize>(),
        stamp in proptest::option::of(prop_oneof![Just(7u16), Just(35), Just(39)]),
    ) {
        let seeds = seed_records();
        let (mut bytes, _) = seeds[which].clone();
        for (at, v) in edits {
            let i = at % bytes.len();
            bytes[i] = v;
        }
        if cut % 4 == 0 {
            bytes.truncate(cut % (bytes.len() + 1));
        }
        exercise(&bytes, stamp);
    }

    #[test]
    fn generated_meshes_round_trip(
        fmt in proptest::sample::select(CENSUS.to_vec()),
        v in 3u32..200,
        build_byte in any::<bool>(),
    ) {
        let mut spec = MeshSpec::single(fmt, v);
        spec.build_byte = build_byte;
        let mesh = spec.build().unwrap();
        let bytes = mesh.to_bytes().unwrap();
        let back = Mesh::decode(&bytes, None, &lossless()).unwrap();
        prop_assert_eq!(&back.submeshes, &mesh.submeshes);
        prop_assert!(back.validate().is_clean());
    }
}

#[test]
fn unobserved_but_well_defined_formats_round_trip() {
    // 15 UV sets and 255 influences; 5 influences without normals.
    for f in [0x30FF_0F72u32, 0x0005_0002] {
        let mesh = MeshSpec::single(f, 20).build().unwrap();
        assert!(mesh.validate().is_clean(), "{f:#x}: {:?}", mesh.validate());
        let bytes = mesh.to_bytes().unwrap();
        let back = Mesh::decode(&bytes, Some(39), &lossless()).unwrap();
        assert_eq!(back.submeshes, mesh.submeshes);
        let g = back.level_geometry(0, &GeometryLimits::default()).unwrap();
        for w in g.weights.as_ref().unwrap() {
            assert!((w.iter().sum::<f32>() - 1.0).abs() < 1e-5, "{w:?}");
        }
    }
}

// ---------------------------------------------------------------------------------------
// Review regressions: tangent sign, flattening bounds, skin, hand-built meshes
// ---------------------------------------------------------------------------------------

#[test]
fn tangent_sign_uses_generated_normals_when_format_has_none() {
    // Tangents and one UV set, no NORMAL bit: normals are generated from the faces.
    let mut m = MeshSpec::single(0x3000_0102, 4).build().unwrap();
    assert!(m.submeshes[0].streams.normals.is_none());
    let st = &mut m.submeshes[0].streams;
    // A unit quad in the XY plane, counter-clockwise seen from +Z: face normal +Z.
    st.positions = vec![
        [0.0, 0.0, 0.0],
        [1.0, 0.0, 0.0],
        [1.0, 1.0, 0.0],
        [0.0, 1.0, 0.0],
    ];
    st.tangents = Some(vec![[1.0, 0.0, 0.0]; 4]);
    // Mirrored V on vertices 2 and 3.
    st.binormals = Some(vec![
        [0.0, 1.0, 0.0],
        [0.0, 1.0, 0.0],
        [0.0, -1.0, 0.0],
        [0.0, -1.0, 0.0],
    ]);
    m.index_buffers[0].indices = vec![0, 1, 2, 0, 2, 3];
    let g = m.level_geometry(0, &GeometryLimits::default()).unwrap();
    for n in &g.normals {
        assert_eq!(*n, [0.0, 0.0, 1.0]);
    }
    // normal × tangent = Z × X = +Y: +1 where the binormal is +Y, −1 where it is −Y.
    let w: Vec<f32> = g.tangents.unwrap().iter().map(|t| t[3]).collect();
    assert_eq!(w, vec![1.0, 1.0, -1.0, -1.0]);

    // The same check against the synthetic generator's mirrored binormals (every third
    // vertex −Y): every sign agrees with the generated normals.
    let m = MeshSpec::single(0x3000_0102, 40).build().unwrap();
    let g = m.level_geometry(0, &GeometryLimits::default()).unwrap();
    let st = &m.submeshes[0].streams;
    let (t, b) = (
        st.tangents.as_ref().unwrap(),
        st.binormals.as_ref().unwrap(),
    );
    let gt = g.tangents.as_ref().unwrap();
    let mut negative = 0;
    for v in 0..40 {
        let n = g.normals[v];
        let c = [
            n[1] * t[v][2] - n[2] * t[v][1],
            n[2] * t[v][0] - n[0] * t[v][2],
            n[0] * t[v][1] - n[1] * t[v][0],
        ];
        let d = c[0] * b[v][0] + c[1] * b[v][1] + c[2] * b[v][2];
        let expected = if d < 0.0 { -1.0 } else { 1.0 };
        assert_eq!(gt[v][3], expected, "vertex {v}");
        negative += usize::from(expected < 0.0);
    }
    assert!(negative > 0);
}

#[test]
fn default_limits_cover_the_observed_maximum_with_headroom() {
    let l = GeometryLimits::default();
    // mesh.md §18: largest observed level 99,478 vertices, 287,712 indices, 28 draws.
    assert!(l.max_vertices >= 8 * 99_478 && l.max_vertices <= 1 << 21);
    assert!(l.max_indices >= 8 * 287_712 && l.max_indices <= 1 << 24);
    assert!(l.max_draws >= 8 * 28);
    assert!(l.max_bytes <= 256 << 20);
}

#[test]
fn many_streams_on_one_small_part_cannot_amplify_flattening() {
    // A 3-vertex part with every stream, then a position-only block drawn 2,047 times:
    // about 822 MB of streams from a 64 KB record before the byte bounds.
    let spec = MeshSpec {
        submeshes: vec![
            SubmeshSpec {
                format: 0x2,
                vertices: 2048,
            },
            SubmeshSpec {
                format: 0x3004_0F52,
                vertices: 3,
            },
        ],
        levels: vec![LevelSpec {
            collision: false,
            tree: TreeSpec::None,
            ..LevelSpec::detail(
                std::iter::once(1)
                    .chain(std::iter::repeat_n(0, 2047))
                    .collect(),
            )
        }],
        ..MeshSpec::single(0x112, 3)
    };
    let mesh = spec.build().unwrap();
    let bytes = mesh.to_bytes().unwrap();
    assert!(bytes.len() < 70_000);
    let back = Mesh::decode(&bytes, Some(39), &lossless()).unwrap();
    let err = back
        .level_geometry(0, &GeometryLimits::default())
        .unwrap_err();
    assert!(matches!(err, MeshError::LimitExceeded { .. }), "{err}");
    // Each bound fires on its own.
    let only = |f: fn(&mut GeometryLimits)| {
        let mut l = GeometryLimits {
            max_vertices: u64::MAX,
            max_indices: u64::MAX,
            max_draws: u64::MAX,
            max_bytes: u64::MAX,
            max_amplification: u64::MAX,
        };
        f(&mut l);
        match back.level_geometry(0, &l) {
            Err(MeshError::LimitExceeded { what, .. }) => what,
            other => panic!("{other:?}"),
        }
    };
    assert_eq!(only(|l| l.max_draws = 1024), "draws in one level");
    assert_eq!(only(|l| l.max_bytes = 128 << 20), "flattened stream bytes");
    assert!(only(|l| l.max_amplification = 64).contains("amplification"));
}

#[test]
fn amplification_bound_allows_every_block_drawn_once() {
    // Fill-in streams alone (a tiny part with every stream next to a position-only block
    // drawn once) stay far below the default amplification.
    let spec = MeshSpec {
        submeshes: vec![
            SubmeshSpec {
                format: 0x2,
                vertices: 60_000,
            },
            SubmeshSpec {
                format: 0x30FF_0F72,
                vertices: 3,
            },
        ],
        levels: vec![LevelSpec::detail(vec![1, 0])],
        ..MeshSpec::single(0x112, 3)
    };
    let m = spec.build().unwrap();
    let g = m.level_geometry(0, &GeometryLimits::default()).unwrap();
    assert_eq!(g.uv_sets.len(), 15);
    assert_eq!(g.vertex_count(), 60_003);
    let per_vertex = 12 + 12 + 16 + 15 * 8 + 4 + 20;
    assert_eq!(
        g.stream_bytes(),
        per_vertex * 60_003 + 2 * g.indices.len() as u64
    );
}

#[test]
fn repeated_skinned_block_is_reduced_once_and_copied() {
    let spec = MeshSpec {
        submeshes: vec![SubmeshSpec {
            format: 0x0006_0112,
            vertices: 30,
        }],
        levels: vec![LevelSpec::detail(vec![0, 0, 0])],
        ..MeshSpec::single(0x112, 3)
    };
    let m = spec.build().unwrap();
    let g = m.level_geometry(0, &GeometryLimits::default()).unwrap();
    let (j, w) = (g.joints.as_ref().unwrap(), g.weights.as_ref().unwrap());
    let skin = m.submeshes[0].streams.skin.as_ref().unwrap();
    for part in &g.parts {
        for v in 0..30 {
            let (ej, ew) = skin.four(v);
            assert_eq!(j[part.base_vertex as usize + v], ej);
            assert_eq!(w[part.base_vertex as usize + v], ew);
        }
    }
}

#[test]
fn huge_influence_counts_are_cheap() {
    // K = 255 drawn many times: refused by the amplification bound before any work.
    let spec = MeshSpec {
        submeshes: vec![SubmeshSpec {
            format: 0x00FF_0002,
            vertices: 206,
        }],
        levels: vec![LevelSpec {
            collision: false,
            tree: TreeSpec::None,
            ..LevelSpec::detail(vec![0; 20_360])
        }],
        ..MeshSpec::single(0x112, 3)
    };
    let m = spec.build().unwrap();
    let start = std::time::Instant::now();
    let err = m.level_geometry(0, &GeometryLimits::default()).unwrap_err();
    assert!(matches!(err, MeshError::LimitExceeded { .. }));
    // Even with no limits, each block is reduced to four influences once.
    let unlimited = GeometryLimits {
        max_vertices: u64::MAX,
        max_indices: u64::MAX,
        max_draws: u64::MAX,
        max_bytes: u64::MAX,
        max_amplification: u64::MAX,
    };
    let g = m.level_geometry(0, &unlimited).unwrap();
    assert_eq!(g.vertex_count(), 206 * 20_360);
    // Generous: the reduction alone took over 1.5 s before (release); copying is fast.
    assert!(start.elapsed() < std::time::Duration::from_secs(20));
}

/// The previous, allocating implementation of [`Skin::four`], as a reference.
fn four_reference(j: &[u8], w: &[f32]) -> ([u8; 4], [f32; 4]) {
    let mut joints = [0u8; 4];
    let mut weights = [0f32; 4];
    if j.len() <= 4 {
        joints[..j.len()].copy_from_slice(j);
        weights[..w.len()].copy_from_slice(w);
        return (joints, weights);
    }
    let mut order: Vec<usize> = (0..j.len()).collect();
    order.sort_by(|&a, &b| w[b].total_cmp(&w[a]));
    let total: f32 = w.iter().filter(|x| x.is_finite()).sum();
    let kept: f32 = order[..4]
        .iter()
        .map(|&i| w[i])
        .filter(|x| x.is_finite())
        .sum();
    let mut scale = if kept > 0.0 && total.is_finite() {
        total / kept
    } else {
        1.0
    };
    // An overflowing renormalisation keeps the stored weights.
    if !scale.is_finite()
        || order[..4]
            .iter()
            .any(|&i| w[i].is_finite() && !(w[i] * scale).is_finite())
    {
        scale = 1.0;
    }
    for (slot, &i) in order[..4].iter().enumerate() {
        joints[slot] = j[i];
        weights[slot] = w[i] * scale;
    }
    (joints, weights)
}

proptest! {
    #[test]
    fn skin_four_matches_a_sorting_reference(
        k in 1u8..12,
        raw in proptest::collection::vec(
            prop_oneof![
                Just(0.0f32), Just(0.25f32), Just(-0.5f32), Just(f32::NAN),
                Just(f32::INFINITY), 0.0f32..1.0
            ],
            12,
        ),
    ) {
        let k = usize::from(k);
        let joints: Vec<u8> = (0..k as u8).map(|x| x * 3 + 1).collect();
        let weights = raw[..k].to_vec();
        let skin = Skin { influences: k as u8, joints: joints.clone(), weights: weights.clone() };
        let (gj, gw) = skin.four(0);
        let (ej, ew) = four_reference(&joints, &weights);
        prop_assert_eq!(gj, ej);
        prop_assert_eq!(gw.map(f32::to_bits), ew.map(f32::to_bits));
    }
}

#[test]
fn skin_accessors_never_overflow_or_panic() {
    let skin = Skin {
        influences: 2,
        joints: vec![0; 4],
        weights: vec![0.5; 4],
    };
    assert!(skin.joints_of(usize::MAX).is_empty());
    assert!(skin.weights_of(usize::MAX).is_empty());
    // 2^63 × 2 wraps to 0 in unchecked release arithmetic.
    assert!(skin.joints_of(usize::MAX / 2 + 1).is_empty());
    assert_eq!(skin.four(usize::MAX / 2 + 1), ([0; 4], [0.0; 4]));
    // Joints and weights of different lengths (hand-built).
    let short = Skin {
        influences: 6,
        joints: vec![1, 2, 3, 4, 5, 6],
        weights: vec![0.5, 0.5],
    };
    assert_eq!(short.four(0), ([0; 4], [0.0; 4]));
    let short = Skin {
        influences: 2,
        joints: vec![7, 8],
        weights: vec![0.25; 6],
    };
    assert_eq!(short.four(0), ([7, 8, 0, 0], [0.25, 0.25, 0.0, 0.0]));
}

#[test]
fn edited_streams_are_reported_and_refused_not_panicked_on() {
    let clean = MeshSpec::single(0x3000_0112, 12).build().unwrap();
    assert!(clean.validate().is_clean());
    type Edit = (&'static str, fn(&mut Mesh));
    let edits: Vec<Edit> = vec![
        ("normals", |m| {
            m.submeshes[0].streams.normals.as_mut().unwrap().pop();
        }),
        ("uv set", |m| m.submeshes[0].streams.uv_sets[0].truncate(3)),
        ("tangents", |m| {
            m.submeshes[0].streams.tangents.as_mut().unwrap().pop();
        }),
        ("binormals", |m| {
            m.submeshes[0]
                .streams
                .binormals
                .as_mut()
                .unwrap()
                .push([0.0; 3]);
        }),
        ("positions", |m| {
            m.submeshes[0].streams.positions.pop();
        }),
        ("positions", |m| m.submeshes[0].vertex_count = u32::MAX),
    ];
    for (stream, edit) in edits {
        let mut m = clean.clone();
        edit(&mut m);
        let report = m.validate();
        assert!(
            report
                .issues
                .iter()
                .any(|i| i.kind == IssueKind::StreamLength),
            "{stream}: {report:?}"
        );
        assert!(report.has_structural());
        let err = m.level_geometry(0, &GeometryLimits::default()).unwrap_err();
        match &err {
            MeshError::StreamLength { stream: s, .. } => assert_eq!(s, &stream),
            other => panic!("{stream}: {other}"),
        }
        assert_eq!(err.category(), FailureCategory::Usage);
        assert!(m.to_bytes().is_err());
    }
    let mut m = MeshSpec::single(0x3002_0112, 12).build().unwrap();
    m.submeshes[0].streams.skin.as_mut().unwrap().weights.pop();
    assert!(matches!(
        m.level_geometry(0, &GeometryLimits::default()),
        Err(MeshError::StreamLength {
            stream: "skin weights",
            ..
        })
    ));
    assert!(issue_kinds(&m).contains(&IssueKind::StreamLength));
}

#[test]
fn synth_refuses_meshes_the_decoder_would_reject() {
    let mut spec = MeshSpec::single(0x112, 3);
    spec.levels.clear();
    assert!(matches!(spec.build(), Err(MeshError::Empty { .. })));
    let mut spec = MeshSpec::single(0x112, 3);
    spec.levels.push(LevelSpec::detail(vec![]));
    let err = spec.build().unwrap_err();
    assert!(matches!(err, MeshError::Empty { ref what } if what == "draw list of level 1"));
}

#[test]
fn header_parse_needs_the_minimum_submesh_bytes() {
    let bytes = handmade(true);
    // DID, flags, S = 1, one surface, N = 1: region A alone is 20 bytes.
    assert!(MeshHeader::parse(&bytes).is_ok());
    let err = MeshHeader::parse(&bytes[..20]).unwrap_err();
    assert_eq!(err.category(), FailureCategory::Truncated);
}

#[test]
fn submesh_and_level_counts_are_capped() {
    use tessera_mesh::{MAX_LEVELS, MAX_SUBMESHES};
    // A surface count above the cap fails before the surfaces are allocated.
    let mut b = Vec::new();
    u32s(&mut b, &[0x0600_0001, 1, MAX_SUBMESHES + 1]);
    b.resize(b.len() + 40 * (MAX_SUBMESHES as usize + 1), 0);
    let err = Mesh::parse(&b).unwrap_err();
    assert!(
        matches!(err, MeshError::LimitExceeded { what, .. } if what.contains("submeshes")),
        "{err}"
    );
    // A level count above the cap.
    let mesh = MeshSpec::single(0x112, 3).build().unwrap();
    let mut b = mesh.to_bytes().unwrap();
    let levels_at = b.len() - mesh_level_bytes(&mesh);
    b.truncate(levels_at);
    u32s(&mut b, &[MAX_LEVELS + 1]);
    b.resize(b.len() + 23 * (MAX_LEVELS as usize + 1), 0);
    let err = Mesh::decode(&b, Some(39), &lossless()).unwrap_err();
    assert!(
        matches!(err, MeshError::LimitExceeded { what, .. } if what.contains("levels")),
        "{err}"
    );
}

/// Bytes of region D (level count and levels) of `mesh`'s encoding.
fn mesh_level_bytes(mesh: &Mesh) -> usize {
    let mut head = mesh.clone();
    let full = mesh.to_bytes().unwrap().len();
    // Region D of a copy with one minimal level: 4 + 23 bytes.
    head.levels = vec![tessera_mesh::Level {
        kind: LevelKind::Proxy,
        draws: vec![mesh.levels[0].draws[0]],
        collision_counts: None,
        collision: None,
        tree: None,
    }];
    let minimal = head.to_bytes().unwrap().len();
    full - (minimal - 27)
}

// ---------------------------------------------------------------------------------------
// Validation coverage
// ---------------------------------------------------------------------------------------

#[test]
fn validation_reports_weight_index_and_vertex_issues() {
    let clean = MeshSpec::single(0x3002_0152, 12).build().unwrap();

    // Vertex 0 has one influence: weights [1, 0].
    let mut m = clean.clone();
    {
        let skin = m.submeshes[0].streams.skin.as_mut().unwrap();
        skin.weights[0] = 1.5;
        skin.weights[1] = -0.5;
    }
    let k = issue_kinds(&m);
    assert!(k.contains(&IssueKind::NegativeWeight), "{k:?}");
    assert!(!k.contains(&IssueKind::WeightSum));

    let mut m = clean.clone();
    {
        let skin = m.submeshes[0].streams.skin.as_mut().unwrap();
        let j0 = skin.joints[0];
        skin.weights[0..2].copy_from_slice(&[0.0, 1.0]);
        skin.joints[0..2].copy_from_slice(&[0, j0]);
    }
    assert_eq!(issue_kinds(&m), vec![IssueKind::FirstWeightNotPositive]);

    let mut m = clean.clone();
    for x in &mut m.index_buffers[0].indices {
        if *x == 11 {
            *x = 10;
        }
    }
    let k = issue_kinds(&m);
    assert!(k.contains(&IssueKind::IndexMaxNotLast), "{k:?}");
    assert!(k.contains(&IssueKind::UnusedVertex));

    let mut m = clean.clone();
    {
        let sm = &mut m.submeshes[0];
        sm.vertex_count = 0;
        let st = &mut sm.streams;
        st.positions.clear();
        st.normals.as_mut().unwrap().clear();
        st.colors.as_mut().unwrap().clear();
        st.tangents.as_mut().unwrap().clear();
        st.binormals.as_mut().unwrap().clear();
        st.uv_sets.iter_mut().for_each(Vec::clear);
        let skin = st.skin.as_mut().unwrap();
        skin.joints.clear();
        skin.weights.clear();
        sm.influence_list.clear();
    }
    m.index_buffers[0].indices.clear();
    let k = issue_kinds(&m);
    assert!(k.contains(&IssueKind::ZeroVertices), "{k:?}");
    assert!(!k.contains(&IssueKind::StreamLength));

    let mut m = clean.clone();
    m.did = Did(0x0700_0001);
    assert_eq!(issue_kinds(&m), vec![IssueKind::NotMeshDid]);
}

#[test]
fn validation_reports_proxy_with_collision() {
    let mut m = multi().build().unwrap();
    m.levels[2].collision = m.levels[0].collision.clone();
    m.levels[2].collision_counts = m.levels[0].collision_counts;
    let k = issue_kinds(&m);
    assert!(k.contains(&IssueKind::ProxyWithCollision), "{k:?}");
    assert!(m.validate().has_structural());
}

#[test]
fn collision_index_overflow_suppresses_face_order_and_is_not_strict() {
    use tessera_mesh::{CollisionEdge, CollisionMesh};
    let clean = MeshSpec::single(0x3000_0112, 12).build().unwrap();
    let base = clean.levels[0].collision.clone().unwrap();
    // 65,537 edges whose face pair is out of order (fA = fB = 0).
    let bad_edge = CollisionEdge {
        vertices: [0, 1],
        faces: [0, 0],
    };
    let mut cm = CollisionMesh {
        edges: vec![bad_edge; 65_537],
        ..base.clone()
    };
    let mut m = clean.clone();
    m.levels[0].collision_counts = Some(tessera_mesh::CollisionCounts {
        vertices: cm.vertices.len() as u32,
        triangles: cm.triangles.len() as u32,
        edges: cm.edges.len() as u32,
    });
    m.levels[0].collision = Some(cm.clone());
    assert_eq!(issue_kinds(&m), vec![IssueKind::CollisionIndexOverflow]);
    // Quality only: strict decoding still succeeds (real data has such levels, §9.2).
    let bytes = m.to_bytes().unwrap();
    let strict = DecodeOptions {
        strict: true,
        ..lossless()
    };
    let back = Mesh::decode(&bytes, Some(39), &strict).unwrap();
    assert_eq!(back.levels[0].collision_counts.unwrap().edges, 65_537);
    // Without the overflow the face order is checked.
    cm.edges.truncate(3);
    let mut m = clean.clone();
    m.levels[0].collision = Some(cm);
    assert!(issue_kinds(&m).contains(&IssueKind::CollisionEdgeFaceOrder));
}

#[test]
fn validation_caps_the_issue_list() {
    use tessera_mesh::validate::MAX_ISSUES;
    let one = MeshSpec::single(0x3000_0112, 12).build().unwrap();
    let mut m = one.clone();
    for _ in 0..1100 {
        m.submeshes.push(one.submeshes[0].clone());
        m.index_buffers.push(one.index_buffers[0].clone());
        m.surfaces.push(one.surfaces[0]);
    }
    // Only submesh 0 is drawn: 1,100 SubmeshNotDrawn issues, structural, so none is
    // dropped.
    let r = m.validate();
    assert_eq!(r.issues.len(), 1100);
    assert_eq!(r.dropped, 0);
    assert!(!r.is_clean());
    assert!(
        r.issues
            .iter()
            .all(|i| i.kind == IssueKind::SubmeshNotDrawn)
    );
    // Quality issues are capped: 1,100 drawn copies with a broken stored box.
    let mut m = one.clone();
    let draw = one.levels[0].draws[0];
    for i in 1..=1100u32 {
        let mut sm = one.submeshes[0].clone();
        sm.stored_aabb.max[0] += 1.0;
        m.submeshes.push(sm);
        m.index_buffers.push(one.index_buffers[0].clone());
        m.surfaces.push(one.surfaces[0]);
        m.levels[0].draws.push(tessera_mesh::Draw {
            vertex_block: i,
            index_buffer: i,
            surface: i,
            ..draw
        });
    }
    let r = m.validate();
    assert_eq!(r.issues.len(), MAX_ISSUES);
    assert_eq!(r.dropped, 1100 - MAX_ISSUES as u64);
    assert!(r.issues.iter().all(|i| i.kind == IssueKind::AabbMismatch));
}

#[test]
fn collision_positions_resolve_through_the_draw_list() {
    let m = multi().build().unwrap();
    let level = &m.levels[0];
    let cm = level.collision.as_ref().unwrap();
    let p = cm.positions(&m, level);
    assert_eq!(p.len(), cm.vertices.len());
    // Level 0 draws submesh 1 first: draw 0 is submesh 1, not submesh 0.
    for (cv, pos) in cm.vertices.iter().zip(&p) {
        assert_eq!(cv.draw, 0);
        assert_eq!(
            *pos,
            Some(m.submeshes[1].streams.positions[usize::from(cv.vertex)])
        );
    }
    let mut cm = cm.clone();
    cm.vertices[0].vertex = 999;
    cm.vertices[1].draw = 9;
    let p = cm.positions(&m, level);
    assert_eq!((p[0], p[1]), (None, None));
    assert!(p[2].is_some());
}

#[test]
fn aabb_union_contains_both() {
    use tessera_mesh::Aabb;
    let a = Aabb {
        min: [0.0, -1.0, 2.0],
        max: [1.0, 1.0, 3.0],
    };
    let b = Aabb {
        min: [-2.0, 0.0, 2.5],
        max: [0.5, 4.0, 2.75],
    };
    let u = a.union(&b);
    assert_eq!(u.min, [-2.0, -1.0, 2.0]);
    assert_eq!(u.max, [1.0, 4.0, 3.0]);
    assert_eq!(a.union(&a), a);
    assert_eq!(b.union(&a), u);
}

// ---------------------------------------------------------------------------------------
// Skinned levels (mesh.md §5.7, §6.2; skeleton-anim.md §4.1)
// ---------------------------------------------------------------------------------------

#[test]
fn skinned_level_reports_bones_and_checks_the_skeleton() {
    let limits = GeometryLimits::default();
    let m = MeshSpec::single(0x3002_0252, 12).build().unwrap();
    let skin = m.submeshes[0].streams.skin.as_ref().unwrap();
    let max = skin.max_joint().expect("the synthetic skin uses bones");
    let bones = usize::from(max) + 1;

    let sl = m.skinned_level(0, &limits, Some(bones)).unwrap();
    assert!(sl.is_skinned());
    assert_eq!(sl.max_joint, Some(max));
    assert_eq!(sl.palette_len(), bones);
    assert_eq!(sl.parts.len(), sl.geometry.parts.len());
    assert!(sl.parts[0].skinned);
    assert_eq!(sl.parts[0].max_joint, Some(max));
    assert_eq!(sl.parts[0].bones_used, skin.bones_used());
    let streams = sl.part_streams(0).expect("skinned part");
    assert_eq!(
        streams.joints.len(),
        sl.geometry.parts[0].vertex_count as usize
    );
    assert_eq!(streams.weights.len(), streams.joints.len());
    // Without a bone count nothing is checked.
    assert!(m.skinned_level(0, &limits, None).is_ok());
    // One bone too few: the largest joint is out of range.
    match m.skinned_level(0, &limits, Some(usize::from(max))) {
        Err(MeshError::JointOutOfRange {
            joint, bone_count, ..
        }) => {
            assert_eq!(joint, max);
            assert_eq!(bone_count, usize::from(max));
        }
        other => panic!("expected JointOutOfRange, got {other:?}"),
    }

    // A hand-edited part range never panics, even when base + count overflows u32.
    let mut edited = sl.clone();
    edited.geometry.parts[0].base_vertex = u32::MAX;
    assert!(edited.part_streams(0).is_none());
    edited.geometry.parts[0].vertex_count = u32::MAX;
    assert!(edited.part_streams(0).is_none());
    assert!(sl.part_streams(99).is_none());

    // A static mesh: no skin, no palette, no streams.
    let s = MeshSpec::single(0x0000_0112, 6).build().unwrap();
    let sl = s.skinned_level(0, &limits, Some(1)).unwrap();
    assert!(!sl.is_skinned());
    assert_eq!(sl.max_joint, None);
    assert_eq!(sl.palette_len(), 0);
    assert!(sl.part_streams(0).is_none());
}
