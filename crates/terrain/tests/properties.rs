// SPDX-License-Identifier: GPL-3.0-or-later
//! Property tests over random synthetic blocks: meshes at any level and stitch pattern
//! cover the block once with counter-clockwise triangles, level-0 meshes agree with the
//! height queries, and edges shared by two random blocks cut from one field are identical.

use proptest::prelude::*;
use tessera_formats::landblock::{GRID_SAMPLES, GRID_SIDE, HeightGrid, VertexFlags};
use tessera_terrain::{
    EdgeLods, HeightSlopeShading, Lod, Neighbours, Side, TerrainBlock, TerrainMesh, VertexNormals,
};

fn block_strategy() -> impl Strategy<Value = TerrainBlock> {
    (
        prop::collection::vec(any::<u16>(), GRID_SAMPLES),
        prop::collection::vec(any::<u8>(), GRID_SAMPLES),
    )
        .prop_map(|(h, f)| {
            let mut heights = HeightGrid::flat(0);
            heights.raw_mut().copy_from_slice(&h);
            let mut flags = VertexFlags::default();
            flags.raw_mut().copy_from_slice(&f);
            TerrainBlock::new(&heights, &flags)
        })
}

fn lod_strategy() -> impl Strategy<Value = Lod> {
    (0u8..6).prop_map(Lod::clamped)
}

fn edges_strategy() -> impl Strategy<Value = EdgeLods> {
    prop::array::uniform4(prop::option::of(lod_strategy())).prop_map(EdgeLods)
}

fn area2(m: &TerrainMesh, t: &[u16]) -> i64 {
    let p = |k: u16| {
        let q = m.positions[usize::from(k)];
        ((q[0] / 5.0) as i64, (q[1] / 5.0) as i64)
    };
    let (a, b, c) = (p(t[0]), p(t[1]), p(t[2]));
    (b.0 - a.0) * (c.1 - a.1) - (b.1 - a.1) * (c.0 - a.0)
}

proptest! {
    #![proptest_config(ProptestConfig::with_cases(64))]

    #[test]
    fn meshes_cover_the_block(block in block_strategy(), lod in lod_strategy(), edges in edges_strategy()) {
        let normals = VertexNormals::compute(&block, &Neighbours::NONE);
        let m = TerrainMesh::build(&block, &normals, &HeightSlopeShading::default(), lod, edges);
        let mut total = 0;
        for t in m.indices.as_chunks::<3>().0 {
            let a = area2(&m, t);
            prop_assert!(a > 0);
            total += a;
        }
        prop_assert_eq!(total, 2 * 32 * 32);
        prop_assert!(m.normals.iter().all(|n| n[2] > 0.0));
        prop_assert!(m.min_height >= block.min_height() && m.max_height <= block.max_height());
    }

    #[test]
    fn level_zero_matches_queries(block in block_strategy(), u in 0f32..1.0, v in 0f32..1.0) {
        let normals = VertexNormals::compute(&block, &Neighbours::NONE);
        let m = TerrainMesh::build(&block, &normals, &HeightSlopeShading::default(), Lod::FINEST, EdgeLods::NONE);
        // The triangle under (x, y), found by a point-in-triangle test on the mesh itself.
        let (x, y) = (f64::from(u) * 160.0, f64::from(v) * 160.0);
        for t in m.indices.as_chunks::<3>().0 {
            let [a, b, c] = [t[0], t[1], t[2]].map(|k| m.positions[usize::from(k)].map(f64::from));
            let d = (b[1] - c[1]) * (a[0] - c[0]) + (c[0] - b[0]) * (a[1] - c[1]);
            let wa = ((b[1] - c[1]) * (x - c[0]) + (c[0] - b[0]) * (y - c[1])) / d;
            let wb = ((c[1] - a[1]) * (x - c[0]) + (a[0] - c[0]) * (y - c[1])) / d;
            let wc = 1.0 - wa - wb;
            if wa >= 1e-6 && wb >= 1e-6 && wc >= 1e-6 {
                let z = wa * a[2] + wb * b[2] + wc * c[2];
                let q = f64::from(block.height_at(x as f32, y as f32).unwrap());
                prop_assert!((z - q).abs() < 5e-3, "{z} vs {q}");
                break;
            }
        }
        // Points within 1e-6 of a triangle edge are not checked.
    }

    #[test]
    fn shared_edges_are_identical(
        field in prop::collection::vec(any::<u16>(), GRID_SIDE * (2 * GRID_SIDE - 1)),
        la in lod_strategy(),
        lb in lod_strategy(),
    ) {
        // A 65 × 33 field cut into a west and an east block sharing column 32.
        let at = |gi: usize, j: usize| field[j * (2 * GRID_SIDE - 1) + gi];
        let west = TerrainBlock::new(&HeightGrid::from_fn(at), &VertexFlags::default());
        let east = TerrainBlock::new(&HeightGrid::from_fn(|i, j| at(i + 32, j)), &VertexFlags::filled(0x80));
        prop_assert!(west.edge_matches(Side::East, &east));
        let nw = VertexNormals::compute(&west, &Neighbours::NONE.with(Side::East, Some(&east)));
        let ne = VertexNormals::compute(&east, &Neighbours::NONE.with(Side::West, Some(&west)));
        let s = HeightSlopeShading::default();
        let mw = TerrainMesh::build(&west, &nw, &s, la, EdgeLods::NONE.with(Side::East, Some(lb)));
        let me = TerrainMesh::build(&east, &ne, &s, lb, EdgeLods::NONE.with(Side::West, Some(la)));
        let edge = |m: &TerrainMesh, x: f32, off: f32| {
            let mut v: Vec<([u32; 3], [u32; 3])> = m
                .positions
                .iter()
                .zip(&m.normals)
                .filter(|(p, _)| p[0] == x)
                .map(|(p, n)| ([(p[0] + off).to_bits(), p[1].to_bits(), p[2].to_bits()], n.map(f32::to_bits)))
                .collect();
            v.sort_unstable();
            v
        };
        prop_assert_eq!(edge(&mw, 160.0, 0.0), edge(&me, 0.0, 160.0));
    }
}
