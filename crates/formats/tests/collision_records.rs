// SPDX-License-Identifier: GPL-3.0-or-later
//! The synthetic worked examples of `docs/specs/collision.md` §14.1–§14.5, exactly, plus
//! property tests of the tree walks and the 0x66 round trip.

use proptest::prelude::*;
use tessera_formats::collision::{
    CellGeometry, CollisionMesh, PLANE_TAG_BPFL, PLANE_TAG_NEXT, PlaneListEnd, TreeBlock, TreeCode,
};
use tessera_formats::landblock::{
    GRID_SAMPLES, GridSurface, HeightGrid, VertexFlags, grid_triangle_key,
};
use tessera_formats::{Did, FormatError, LandblockData, NoSchema};

fn unit(code: &[u8]) -> TreeCode<'_> {
    TreeCode::new([0.0; 3], 65_536.0, code)
}

/// §14.1: filters, then a split on X with overlapping children.
#[test]
fn s14_1_fifteen_byte_code() {
    let code = [
        0x26, 0x00, 0x0A, 0x27, 0x00, 0x0A, 0x28, 0x00, 0x05, 0x10, 0x05, 0x04, 0x01, 0x30, 0x31,
    ];
    assert_eq!(code.len(), 15);
    let t = unit(&code);
    let cases: [([f32; 3], [i64; 3], &[u32]); 5] = [
        ([2.0, 3.0, 1.0], [2, 3, 1], &[0]),
        ([4.5, 1.0, 1.0], [4, 1, 1], &[0, 1]),
        ([8.0, 1.0, 1.0], [8, 1, 1], &[1]),
        ([11.0, 1.0, 1.0], [11, 1, 1], &[]),
        ([-0.5, 1.0, 1.0], [-1, 1, 1], &[]),
    ];
    for (p, bytes, keys) in cases {
        assert_eq!(t.bytes_of(p), Some(bytes), "{p:?}");
        assert_eq!(t.point_keys(p).unwrap(), keys, "{p:?}");
    }
    let w = t.walk().unwrap();
    assert_eq!(w.keys, vec![0, 1]);
    assert_eq!(w.counts.filter, 3);
    assert_eq!(w.counts.axis_split, 1);
    assert_eq!(w.counts.leaf, 2);
    // Box queries: a superset of the point results over the box.
    assert_eq!(
        t.box_keys([2.0, 1.0, 1.0], [8.0, 1.0, 1.0]).unwrap(),
        vec![0, 1]
    );
    assert_eq!(
        t.box_keys([0.0, 0.0, 0.0], [3.0, 1.0, 1.0]).unwrap(),
        vec![0]
    );
    assert!(
        t.box_keys([11.0, 0.0, 0.0], [12.0, 1.0, 1.0])
            .unwrap()
            .is_empty()
    );
    assert_eq!(
        t.box_keys([-5.0, 0.0, 0.0], [0.5, 1.0, 1.0]).unwrap(),
        vec![0]
    );
}

/// §14.2: a rescale and an output base.
#[test]
fn s14_2_rescale_and_base() {
    let code = [0x01, 0x02, 0x00, 0x00, 0x09, 0x20, 0x50, 0x05];
    let t = unit(&code);
    let p = [3.25, 0.5, 0.5];
    assert_eq!(t.quantise(p).unwrap()[0], 212_992);
    assert_eq!(t.point_keys(p).unwrap(), vec![37]);
    assert_eq!(t.walk().unwrap().keys, vec![37]);
}

/// §14.2 again, with filters after the rescale so that the rescaled bytes decide the key:
/// q x = (212,992 − 2 · 2^16) · 2 = 163,840 (byte 2), q y = 32,768 · 2 = 65,536 (byte 1).
/// A rescale that shifted before subtracting (byte 4), used another axis's operand (byte
/// 6) or shifted by opcode − 1 (byte 1) would report nothing.
#[test]
fn s14_2_rescaled_bytes_decide() {
    let code = [
        0x01, 0x02, 0x00, 0x00, 0x26, 0x02, 0x02, 0x27, 0x01, 0x01, 0x09, 0x20, 0x50, 0x05,
    ];
    let t = unit(&code);
    assert_eq!(t.point_keys([3.25, 0.5, 0.5]).unwrap(), vec![37]);
    // 3.6: (235,929 − 131,072) · 2 = 209,714, byte 3.
    assert!(t.point_keys([3.6, 0.5, 0.5]).unwrap().is_empty());
    // 2.9: (190,054 − 131,072) · 2 = 117,964, byte 1.
    assert!(t.point_keys([2.9, 0.5, 0.5]).unwrap().is_empty());
    // y 0.4: 26,214 · 2 = 52,428, byte 0.
    assert!(t.point_keys([3.25, 0.4, 0.5]).unwrap().is_empty());
    // A box whose rescaled x range [byte 1, byte 3] overlaps byte 2.
    assert_eq!(
        t.box_keys([2.9, 0.5, 0.5], [3.6, 0.5, 0.5]).unwrap(),
        vec![37]
    );
    // The same through the integer query: the rescale applies to the given integers.
    let mut keys = Vec::new();
    t.query_quantised([212_992, 32_768, 0], [212_992, 32_768, 0], |k| keys.push(k))
        .unwrap();
    assert_eq!(keys, vec![37]);
    // Opcode 0x02 shifts by 2 with operand 1 on X: (212,992 − 65,536) · 4 = 589,824, byte 9.
    let code = [0x02, 0x01, 0x00, 0x00, 0x26, 0x09, 0x09, 0x31];
    let t = unit(&code);
    assert_eq!(t.point_keys([3.25, 0.5, 0.5]).unwrap(), vec![1]);
    assert!(t.point_keys([3.0, 0.5, 0.5]).unwrap().is_empty());
}

/// §14.3: a split on (x + y) / 2.
#[test]
fn s14_3_diagonal_split() {
    let code = [0x17, 0x90, 0x8C, 0x01, 0x31, 0x30];
    let t = unit(&code);
    let cases: [([f32; 3], &[u32]); 3] = [
        ([100.0, 150.0, 0.0], &[1]),
        ([150.0, 150.0, 0.0], &[0]),
        ([140.0, 144.0, 0.0], &[0, 1]),
    ];
    for (p, keys) in cases {
        assert_eq!(t.point_keys(p).unwrap(), keys, "{p:?}");
    }
    // The box (100..150, 150, 0) spans diagonal values 125–150: both children.
    assert_eq!(
        t.box_keys([100.0, 150.0, 0.0], [150.0, 150.0, 0.0])
            .unwrap(),
        vec![0, 1]
    );
}

/// §14.4: terrain keys and the triangles they name, for both diagonals.
#[test]
fn s14_4_terrain_keys() {
    let (i, j) = (3usize, 7usize);
    assert_eq!(grid_triangle_key(i, j, true), Some(454));
    assert_eq!(grid_triangle_key(i, j, false), Some(455));
    let mut d =
        LandblockData::from_heights(Did(0x8001_0203), HeightGrid::from_raw([0; GRID_SAMPLES]));
    let sorted = |mut t: [(usize, usize); 3]| {
        t.sort_unstable();
        t
    };
    // Bit 7 set at sample (3, 7).
    d.vertex_flags.raw_mut()[33 * i + j] = VertexFlags::SPLIT_SW_NE;
    assert_eq!(
        sorted(d.grid_triangle(GridSurface::Terrain, 454).unwrap()),
        sorted([(3, 7), (4, 8), (3, 8)])
    );
    assert_eq!(
        sorted(d.grid_triangle(GridSurface::Terrain, 455).unwrap()),
        sorted([(3, 7), (4, 7), (4, 8)])
    );
    // Bit 7 clear.
    d.vertex_flags.raw_mut()[33 * i + j] = 0;
    assert_eq!(
        sorted(d.grid_triangle(GridSurface::Terrain, 454).unwrap()),
        sorted([(3, 7), (4, 7), (3, 8)])
    );
    assert_eq!(
        sorted(d.grid_triangle(GridSurface::Terrain, 455).unwrap()),
        sorted([(4, 7), (4, 8), (3, 8)])
    );
}

fn put_f32s(b: &mut Vec<u8>, v: &[f32]) {
    for x in v {
        b.extend_from_slice(&x.to_le_bytes());
    }
}

/// The record of §14.5: an 8 × 8 × 4 m box room with one opening on its east side.
fn box_room() -> Vec<u8> {
    let mut b = Vec::new();
    b.extend_from_slice(&0x0100_0ABCu32.to_le_bytes());
    b.push(0);
    let planes: [[f32; 4]; 6] = [
        [0.0, 1.0, 0.0, 4.0],
        [-1.0, 0.0, 0.0, 4.0],
        [0.0, -1.0, 0.0, 4.0],
        [1.0, 0.0, 0.0, 4.0],
        [0.0, 0.0, -1.0, 4.0],
        [0.0, 0.0, 1.0, 0.0],
    ];
    for (k, p) in planes.iter().enumerate() {
        let tag = if k + 1 == planes.len() {
            PLANE_TAG_BPFL
        } else {
            PLANE_TAG_NEXT
        };
        b.extend_from_slice(&tag.to_le_bytes());
        put_f32s(&mut b, p);
    }
    // One portal on the east face (x = 4).
    b.extend_from_slice(&1u32.to_le_bytes());
    b.push(1);
    b.extend_from_slice(&4u32.to_le_bytes());
    put_f32s(
        &mut b,
        &[4.0, -4.0, 0.0, 4.0, 4.0, 0.0, 4.0, 4.0, 4.0, 4.0, -4.0, 4.0],
    );
    // Five walls: the other faces.
    let walls: [[f32; 12]; 5] = [
        [
            -4.0, -4.0, 0.0, -4.0, 4.0, 0.0, -4.0, 4.0, 4.0, -4.0, -4.0, 4.0,
        ],
        [-4.0, 4.0, 0.0, 4.0, 4.0, 0.0, 4.0, 4.0, 4.0, -4.0, 4.0, 4.0],
        [
            -4.0, -4.0, 0.0, 4.0, -4.0, 0.0, 4.0, -4.0, 4.0, -4.0, -4.0, 4.0,
        ],
        [
            -4.0, -4.0, 0.0, 4.0, -4.0, 0.0, 4.0, 4.0, 0.0, -4.0, 4.0, 0.0,
        ],
        [
            -4.0, -4.0, 4.0, 4.0, -4.0, 4.0, 4.0, 4.0, 4.0, -4.0, 4.0, 4.0,
        ],
    ];
    b.extend_from_slice(&5u32.to_le_bytes());
    for w in &walls {
        b.extend_from_slice(&4u32.to_le_bytes());
        put_f32s(&mut b, w);
    }
    // Eight hull vertices.
    b.push(8);
    for x in [-4.0f32, 4.0] {
        for y in [-4.0f32, 4.0] {
            for z in [0.0f32, 4.0] {
                put_f32s(&mut b, &[x, y, z]);
            }
        }
    }
    // Empty property set (hint 0, count 0), u32 1, byte 0, then a mesh body.
    b.extend_from_slice(&[0, 0]);
    b.extend_from_slice(&1u32.to_le_bytes());
    b.push(0);
    b.extend_from_slice(&1u32.to_le_bytes()); // flags
    b.extend_from_slice(&[0xAA; 12]); // rest of the body (not interpreted here)
    b
}

/// §14.5: the minimal cell record and its point tests.
#[test]
fn s14_5_minimal_cell_record() {
    let bytes = box_room();
    for stamp in [None, Some(7), Some(39)] {
        let c = CellGeometry::parse_expecting(&bytes, 0x0100_0ABCu32, stamp, &NoSchema).unwrap();
        assert!(c.physics_mesh.is_none());
        assert_eq!(c.planes.len(), 6);
        assert_eq!(c.plane_list_end, PlaneListEnd::Bpfl);
        assert_eq!(c.planes[1].normal, [-1.0, 0.0, 0.0]);
        assert_eq!(c.planes[1].distance, 4.0);
        assert_eq!(c.portals.len(), 1);
        assert_eq!(c.portals[0].lead, 1);
        assert_eq!(c.portals[0].corners.len(), 4);
        assert_eq!(c.portals[0].corners[2], [4.0, 4.0, 4.0]);
        assert_eq!(c.walls.len(), 5);
        assert!(c.walls.iter().all(|w| w.corners.len() == 4));
        assert_eq!(c.hull.len(), 8);
        assert!(c.properties.is_empty());
        assert_eq!(c.render_mesh.len(), 16);
        assert_eq!(c.render_mesh_record().len(), 20);
        // (0, 0, 2) is inside: every plane gives >= 0.
        assert!(
            c.planes
                .iter()
                .all(|p| p.signed_distance([0.0, 0.0, 2.0]) >= 0.0)
        );
        // (5, 0, 2) is outside: the plane (-1, 0, 0, 4) gives -1.
        assert_eq!(c.planes[1].signed_distance([5.0, 0.0, 2.0]), -1.0);
        // Every hull vertex satisfies every plane; every portal corner lies on plane 1,
        // (-1, 0, 0, 4), the east face x = 4.
        for v in &c.hull {
            assert!(c.planes.iter().all(|p| p.signed_distance(*v) >= 0.0));
        }
        for v in &c.portals[0].corners {
            assert_eq!(c.planes[1].signed_distance(*v), 0.0);
        }
    }
    // Truncation anywhere before the render mesh's `flags` is complete fails cleanly.
    for cut in 0..bytes.len() - 12 {
        assert!(
            CellGeometry::parse(&bytes[..cut], None, &NoSchema).is_err(),
            "{cut}"
        );
    }
}

/// §14.5 with a physics mesh: the `build_byte` rule and its stamp-less fallback.
#[test]
fn s14_5_with_physics_mesh() {
    let mesh = CollisionMesh {
        vertices: vec![[-4.0, -4.0, 0.0], [4.0, -4.0, 0.0], [4.0, 4.0, 0.0]],
        triangles: vec![[0, 1, 2]],
        tree: TreeBlock {
            origin: [-4.05, -4.05, -0.05],
            scale: (254.0 * 65_536.0 / 8.1) as f32,
            code: vec![0x30],
            build_byte: None,
        },
        ..CollisionMesh::default()
    };
    let room = box_room();
    for build in [None, Some(1u8)] {
        let mut m = mesh.clone();
        m.tree.build_byte = build;
        let mut b = room[..4].to_vec();
        b.push(1);
        b.extend_from_slice(&m.to_bytes());
        b.extend_from_slice(&room[5..]);
        let stamp = if build.is_some() { 38 } else { 33 };
        let c = CellGeometry::parse(&b, Some(stamp), &NoSchema).unwrap();
        let p = c.physics_mesh.as_ref().unwrap();
        assert_eq!(p.tree.build_byte, build);
        assert_eq!(p.triangles, m.triangles);
        assert_eq!(c.planes.len(), 6);
        let f = CellGeometry::parse(&b, None, &NoSchema).unwrap();
        assert_eq!(f.physics_mesh.unwrap().tree.build_byte, build);
        // The wrong stamp fails.
        let wrong = if build.is_some() { 33 } else { 38 };
        assert!(CellGeometry::parse(&b, Some(wrong), &NoSchema).is_err());
    }
}

/// A 0x66 record built from the worked example's code decodes and walks.
#[test]
fn collision_mesh_with_s14_1_code() {
    let m = CollisionMesh {
        vertices: vec![
            [1.0, 1.0, 1.0],
            [5.0, 1.0, 1.0],
            [5.0, 5.0, 1.0],
            [9.0, 1.0, 1.0],
        ],
        triangles: vec![[0, 1, 2], [1, 3, 2]],
        tree: TreeBlock {
            origin: [0.0; 3],
            scale: 65_536.0,
            code: vec![
                0x26, 0x00, 0x0A, 0x27, 0x00, 0x0A, 0x28, 0x00, 0x05, 0x10, 0x05, 0x04, 0x01, 0x30,
                0x31,
            ],
            build_byte: Some(1),
        },
        ..CollisionMesh::default()
    };
    let bytes = m.to_bytes();
    let back = CollisionMesh::parse(&bytes, Some(39)).unwrap();
    assert_eq!(back.to_bytes(), bytes);
    let w = back.tree_code().walk().unwrap();
    assert!(w.keys_below(back.triangles.len() as u64));
    assert_eq!(w.keys.len(), back.triangles.len());
    assert!(matches!(
        CollisionMesh::parse(&bytes[..bytes.len() - 2], Some(39)),
        Err(FormatError::Truncated { .. })
    ));
}

proptest! {
    /// Point and box walks never panic on arbitrary code; a successful point query's keys
    /// are a subset of the structural walk's keys, and of a box query's keys around it.
    #[test]
    fn walks_are_consistent(
        code in proptest::collection::vec(any::<u8>(), 0..64),
        p in proptest::array::uniform3(-2.0f32..300.0),
        pad in 0.0f32..3.0,
    ) {
        let t = unit(&code);
        let point = t.point_keys(p);
        let walk = t.walk();
        let boxed = t.box_keys(p.map(|v| v - pad), p.map(|v| v + pad));
        if let (Ok(point), Ok(walk)) = (&point, &walk) {
            for k in point {
                prop_assert!(walk.contains(*k));
            }
        }
        if let (Ok(point), Ok(boxed)) = (&point, &boxed) {
            for k in point {
                prop_assert!(boxed.binary_search(k).is_ok());
            }
        }
    }

    /// Hand-built well-formed codes: a balanced tree of axis and diagonal splits over 16
    /// leaves. The walk reaches every leaf and a point query never fails.
    #[test]
    fn balanced_trees_walk(
        ops in proptest::collection::vec((0x10u8..=0x1C, any::<u8>(), any::<u8>()), 15),
        p in proptest::array::uniform3(0.0f32..255.0),
    ) {
        // Level-order layout of a complete binary tree: node k at offset 4k, leaves after.
        let mut code = Vec::new();
        fn emit(k: usize, ops: &[(u8, u8, u8)], code: &mut Vec<u8>) {
            if k >= 15 {
                code.push(0x30 + (k - 15) as u8);
                return;
            }
            let (op, hi, lo) = ops[k];
            let at = code.len();
            code.extend_from_slice(&[op, hi, lo, 0]);
            emit(2 * k + 1, ops, code);
            let right = code.len() - (at + 4);
            code[at + 3] = right as u8;
            emit(2 * k + 2, ops, code);
        }
        emit(0, &ops, &mut code);
        let t = unit(&code);
        let w = t.walk().unwrap();
        prop_assert_eq!(w.keys, (0..16).collect::<Vec<u32>>());
        prop_assert!(t.point_keys(p).is_ok());
    }

    /// 0x66 records survive a round trip.
    #[test]
    fn collision_mesh_round_trip(
        verts in proptest::collection::vec(proptest::array::uniform3(-100.0f32..100.0), 1..40),
        tris in proptest::collection::vec(proptest::array::uniform3(any::<u16>()), 0..40),
        code in proptest::collection::vec(any::<u8>(), 0..32),
        build in any::<bool>(),
    ) {
        let n = verts.len() as u16;
        let m = CollisionMesh {
            triangles: tris.into_iter().map(|t| t.map(|i| i % n)).collect(),
            vertices: verts,
            tree: TreeBlock {
                origin: [1.0, 2.0, 3.0],
                scale: 100.0,
                code,
                build_byte: build.then_some(1),
            },
            ..CollisionMesh::default()
        };
        let bytes = m.to_bytes();
        let stamp = if build { 38 } else { 29 };
        let back = CollisionMesh::parse(&bytes, Some(stamp)).unwrap();
        prop_assert_eq!(&back, &m);
        prop_assert_eq!(CollisionMesh::parse(&bytes, None).unwrap().tree.build_byte, m.tree.build_byte);
    }
}
