// SPDX-License-Identifier: GPL-3.0-or-later
//! Terrain meshing: geomipmapped triangulation with stitched edges, and engine-neutral
//! vertex streams.
//!
//! # Triangulation
//!
//! At level *l* (step *s* = 2^*l* samples) the block is a grid of (32 / *s*)² quads, each
//! cut into two triangles. Level 0 uses the stored split diagonal of every quad
//! (`landblock.md` §3.6), so its surface is exactly the one [`TerrainBlock::height_at`]
//! evaluates; coarser levels pick the diagonal closest to the quad's centre sample.
//!
//! Each side is meshed at its **edge step**, the coarser of the block's level and the
//! neighbour's ([`EdgeLods::effective`]). Where the edge step is coarser than the block's,
//! the row of quads along that side is replaced by a **stitch strip**: a fan-like zip
//! between the coarse edge vertices and the block's own vertices one quad inside. Two
//! adjacent stitch strips meet on the diagonal through their shared corner quad. The
//! neighbour meshes the same edge with exactly the same vertices, so the shared edge
//! consists of the same segments on both sides: no cracks and no T-junctions, at any
//! difference of levels.
//!
//! Every triangle is counter-clockwise seen from above (+Z), the front-face convention of
//! `tessera-render` (`render.md` §2.1).
//!
//! # Vertex streams
//!
//! [`TerrainMesh`] holds the streams of `tessera_render::MeshData` without depending on the
//! renderer (ARCH §3: terrain is L3, render L4): `positions`, `normals`, `uvs`, `colors`
//! map one to one onto the fields of the same names, and `indices` onto
//! `Indices::U16`. Positions are block-local: x, y in 0–160 from the south-west corner
//! and z the absolute height (`coordinates.md` §12); the caller translates by
//! (160 · (X − X_anchor), 160 · (Y − Y_anchor), 0) at draw time.

use tessera_formats::landblock::{GRID_SAMPLES, GRID_SPACING, QUAD_SIDE, height_from_raw};

use crate::block::{TerrainBlock, vertex_index};
use crate::lod::{EdgeLods, Lod, Side};
use crate::normals::VertexNormals;
use crate::shading::HeightSlopeShading;

/// Maps a side's canonical strip frame to grid samples. In the canonical frame the side's
/// edge is v = 0 with u running counter-clockwise around the block (0 → 32) and v pointing
/// into the block; each map is a rotation, so counter-clockwise triangles stay so.
fn canonical_to_grid(side: Side, u: usize, v: usize) -> (usize, usize) {
    let n = QUAD_SIDE;
    match side {
        Side::South => (u, v),
        Side::East => (n - v, u),
        Side::North => (n - u, n - v),
        Side::West => (v, n - u),
    }
}

/// Triangle list of `block` at level `lod` with neighbours at `edges`, as grid vertex
/// numbers (33·j + i, see [`vertex_index`]), counter-clockwise from above.
///
/// The result covers the block's 160 × 160 square exactly once. Its boundary on each side
/// consists of the segments between consecutive multiples of that side's edge step.
#[must_use]
pub fn triangulate(block: &TerrainBlock, lod: Lod, edges: EdgeLods) -> Vec<u16> {
    let s = lod.step();
    let n = lod.quads_per_side();
    let effective = edges.effective(lod);
    let stitched = effective.map(|e| e.step() > s);
    let st = |side: Side| stitched[side.index()];
    let vid = |(i, j): (usize, usize)| vertex_index(i, j) as u16;
    let mut out = Vec::with_capacity(n * n * 6 + 4 * (n + 2) * 3);

    for qj in 0..n {
        for qi in 0..n {
            if (qj == 0 && st(Side::South))
                || (qi == n - 1 && st(Side::East))
                || (qj == n - 1 && st(Side::North))
                || (qi == 0 && st(Side::West))
            {
                continue;
            }
            let (i, j) = (qi * s, qj * s);
            let (sw, se, nw, ne) = ((i, j), (i + s, j), (i, j + s), (i + s, j + s));
            let tris = if block.lod_split_sw_ne(lod, i, j) {
                [[sw, se, ne], [sw, ne, nw]]
            } else {
                [[sw, se, nw], [se, ne, nw]]
            };
            for t in tris {
                out.extend(t.map(vid));
            }
        }
    }

    for side in Side::ALL {
        if !st(side) {
            continue;
        }
        let e = effective[side.index()].step();
        let start = if st(side.prev()) { s } else { 0 };
        let end = if st(side.next()) {
            QUAD_SIDE - s
        } else {
            QUAD_SIDE
        };
        let outer: Vec<usize> = (0..=QUAD_SIDE).step_by(e).collect();
        let inner: Vec<usize> = (start..=end).step_by(s).collect();
        let p = |k: usize| vid(canonical_to_grid(side, outer[k], 0));
        let q = |k: usize| vid(canonical_to_grid(side, inner[k], s));
        let (mut a, mut b) = (0, 0);
        while a + 1 < outer.len() || b + 1 < inner.len() {
            // Advance along whichever chain's next segment has the smaller midpoint.
            let take_outer = if a + 1 >= outer.len() {
                false
            } else if b + 1 >= inner.len() {
                true
            } else {
                outer[a] + outer[a + 1] <= inner[b] + inner[b + 1]
            };
            if take_outer {
                out.extend([p(a), p(a + 1), q(b)]);
                a += 1;
            } else {
                out.extend([p(a), q(b + 1), q(b)]);
                b += 1;
            }
        }
    }
    out
}

/// One block's terrain mesh at one level of detail, with engine-neutral streams (module
/// docs). Only the grid vertices the triangles use are kept, in grid order.
#[derive(Debug, Clone, PartialEq)]
pub struct TerrainMesh {
    /// The block's level.
    pub lod: Lod,
    /// The level each side was meshed at (south, east, north, west).
    pub edge_lods: [Lod; 4],
    /// Block-local positions (x, y in 0–160; z absolute height).
    pub positions: Vec<[f32; 3]>,
    /// Unit vertex normals (seamless, see [`VertexNormals`]).
    pub normals: Vec<[f32; 3]>,
    /// Block UVs: u = x / 160 east, v = 1 − y / 160 (v = 0 is the north edge, the top row
    /// of a north-up image such as a minimap tile, `landblock.md` §6.3).
    pub uvs: Vec<[f32; 2]>,
    /// Linear RGBA height/slope colours.
    pub colors: Vec<[f32; 4]>,
    /// Triangle list into the streams above, counter-clockwise from above.
    pub indices: Vec<u16>,
    /// The grid vertex number (33·j + i) of each kept vertex, ascending.
    pub grid_vertices: Vec<u16>,
    /// Smallest vertex height.
    pub min_height: f32,
    /// Largest vertex height.
    pub max_height: f32,
}

impl TerrainMesh {
    /// Meshes `block` at `lod` with neighbours at `edges`, taking normals from `normals`
    /// (computed for the same block) and colours from `shading`.
    #[must_use]
    pub fn build(
        block: &TerrainBlock,
        normals: &VertexNormals,
        shading: &HeightSlopeShading,
        lod: Lod,
        edges: EdgeLods,
    ) -> Self {
        let grid = triangulate(block, lod, edges);
        let mut remap = [u16::MAX; GRID_SAMPLES];
        for &g in &grid {
            remap[usize::from(g)] = 0;
        }
        let mut grid_vertices = Vec::new();
        for (v, slot) in remap.iter_mut().enumerate() {
            if *slot == 0 {
                *slot = grid_vertices.len() as u16;
                grid_vertices.push(v as u16);
            }
        }
        let indices = grid.iter().map(|&g| remap[usize::from(g)]).collect();
        let palette = shading.prepare();
        let raw = block.raw_heights();
        let all_normals = normals.as_slice();
        let count = grid_vertices.len();
        let mut positions = Vec::with_capacity(count);
        let mut out_normals = Vec::with_capacity(count);
        let mut uvs = Vec::with_capacity(count);
        let mut colors = Vec::with_capacity(count);
        let (mut lo, mut hi) = (f32::INFINITY, f32::NEG_INFINITY);
        let side = QUAD_SIDE as f32;
        for &g in &grid_vertices {
            let g = usize::from(g);
            let (i, j) = (g % (QUAD_SIDE + 1), g / (QUAD_SIDE + 1));
            let z = height_from_raw(raw[g]);
            lo = lo.min(z);
            hi = hi.max(z);
            let n = all_normals[g];
            positions.push([i as f32 * GRID_SPACING, j as f32 * GRID_SPACING, z]);
            out_normals.push(n);
            uvs.push([i as f32 / side, 1.0 - j as f32 / side]);
            colors.push(palette.color(z, n[2]));
        }
        Self {
            lod,
            edge_lods: edges.effective(lod),
            positions,
            normals: out_normals,
            uvs,
            colors,
            indices,
            grid_vertices,
            min_height: lo,
            max_height: hi,
        }
    }

    /// Number of triangles.
    #[must_use]
    pub fn triangle_count(&self) -> usize {
        self.indices.len() / 3
    }

    /// Converts grid vertex numbers (for example a surface layer's triangles,
    /// [`crate::SplatLayer`]) into indices of this mesh. `None` if one of them is not a
    /// vertex of this mesh (a coarser level, or a stitched level-0 edge).
    #[must_use]
    pub fn remap_grid_indices(&self, grid: &[u16]) -> Option<Vec<u16>> {
        grid.iter()
            .map(|g| self.grid_vertices.binary_search(g).ok().map(|k| k as u16))
            .collect()
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::normals::Neighbours;
    use std::collections::BTreeSet;
    use tessera_formats::landblock::{GRID_SIDE, HeightGrid, VertexFlags};

    fn field(gi: usize, gj: usize) -> u16 {
        let mut h = (gi as u32).wrapping_mul(0x9E37_79B9) ^ (gj as u32).wrapping_mul(0x85EB_CA6B);
        h ^= h >> 13;
        h = h.wrapping_mul(0x2C1B_3C6D);
        h ^= h >> 16;
        (10_000 + (gi * 61 + gj * 23) % 3_000 + (h % 700) as usize) as u16
    }

    fn flags_for(bx: usize, by: usize) -> VertexFlags {
        let mut f = VertexFlags::default();
        for i in 0..GRID_SIDE {
            for j in 0..GRID_SIDE {
                if (field(bx * 32 + i, by * 32 + j) / 7) % 2 == 1 {
                    f.raw_mut()[GRID_SIDE * i + j] = 0x80;
                }
            }
        }
        f
    }

    fn block_at(bx: usize, by: usize) -> TerrainBlock {
        TerrainBlock::new(
            &HeightGrid::from_fn(|i, j| field(bx * 32 + i, by * 32 + j)),
            &flags_for(bx, by),
        )
    }

    fn grid_xy(v: u16) -> (i64, i64) {
        let v = usize::from(v);
        ((v % GRID_SIDE) as i64, (v / GRID_SIDE) as i64)
    }

    /// Twice the signed area (grid units) of a grid triangle.
    fn area2(t: &[u16]) -> i64 {
        let (a, b, c) = (grid_xy(t[0]), grid_xy(t[1]), grid_xy(t[2]));
        (b.0 - a.0) * (c.1 - a.1) - (b.1 - a.1) * (c.0 - a.0)
    }

    fn side_of(a: (i64, i64), b: (i64, i64)) -> Option<Side> {
        let n = QUAD_SIDE as i64;
        if a.1 == 0 && b.1 == 0 {
            Some(Side::South)
        } else if a.0 == n && b.0 == n {
            Some(Side::East)
        } else if a.1 == n && b.1 == n {
            Some(Side::North)
        } else if a.0 == 0 && b.0 == 0 {
            Some(Side::West)
        } else {
            None
        }
    }

    /// Checks that `tris` is a consistently oriented, watertight cover of the square whose
    /// boundary on each side has exactly the vertices of that side's edge step.
    fn check_cover(tris: &[u16], edge_lods: [Lod; 4], what: &str) {
        assert_eq!(tris.len() % 3, 0);
        let mut total = 0;
        let mut directed: Vec<(u16, u16)> = Vec::with_capacity(tris.len());
        for t in tris.chunks_exact(3) {
            let a = area2(t);
            assert!(a > 0, "{what}: triangle {t:?} not counter-clockwise ({a})");
            total += a;
            for k in 0..3 {
                directed.push((t[k], t[(k + 1) % 3]));
            }
        }
        assert_eq!(total, 2 * 32 * 32, "{what}: area");
        directed.sort_unstable();
        for w in directed.windows(2) {
            assert_ne!(w[0], w[1], "{what}: directed edge used twice");
        }
        let mut boundary: [BTreeSet<i64>; 4] = Default::default();
        for &(a, b) in &directed {
            if directed.binary_search(&(b, a)).is_ok() {
                continue;
            }
            let (pa, pb) = (grid_xy(a), grid_xy(b));
            let side = side_of(pa, pb)
                .unwrap_or_else(|| panic!("{what}: open edge {pa:?}->{pb:?} inside the block"));
            let along = |p: (i64, i64)| match side {
                Side::South | Side::North => p.0,
                Side::East | Side::West => p.1,
            };
            let (lo, hi) = (along(pa).min(along(pb)), along(pa).max(along(pb)));
            let e = edge_lods[side.index()].step() as i64;
            assert_eq!(hi - lo, e, "{what}: {side:?} boundary segment {lo}..{hi}");
            assert_eq!(lo % e, 0, "{what}: {side:?} boundary segment {lo}..{hi}");
            boundary[side.index()].insert(lo);
            boundary[side.index()].insert(hi);
        }
        for side in Side::ALL {
            let e = edge_lods[side.index()].step() as i64;
            let expect: BTreeSet<i64> = (0..=32).step_by(e as usize).collect();
            assert_eq!(
                boundary[side.index()],
                expect,
                "{what}: {side:?} boundary vertices"
            );
        }
    }

    fn lod_options(own: Lod) -> Vec<Option<Lod>> {
        let mut v = vec![None];
        v.extend(Lod::all().map(Some));
        // Finer neighbours behave like "none"; keep one to exercise that path.
        v.retain(|l| l.is_none_or(|l| l >= own || l == Lod::FINEST));
        v
    }

    #[test]
    fn every_level_and_stitch_pattern_is_a_watertight_ccw_cover() {
        let block = block_at(2, 3);
        // Every pattern in release builds; a fixed 1-in-5 sample (plus all of LOD 2 and
        // up) in debug builds, which run this about 20 times slower.
        let sample = if cfg!(debug_assertions) { 5 } else { 1 };
        let mut seen = 0usize;
        let mut patterns = 0;
        for lod in Lod::all() {
            let opts = lod_options(lod);
            for &s in &opts {
                for &e in &opts {
                    for &n in &opts {
                        for &w in &opts {
                            let edges = EdgeLods([s, e, n, w]);
                            seen += 1;
                            if lod.level() < 2 && !seen.is_multiple_of(sample) {
                                continue;
                            }
                            let tris = triangulate(&block, lod, edges);
                            check_cover(&tris, edges.effective(lod), &format!("{lod} {edges:?}"));
                            patterns += 1;
                        }
                    }
                }
            }
        }
        assert!(patterns * sample >= 3_000, "{patterns}");
    }

    #[test]
    fn level_zero_unstitched_is_the_stored_triangulation() {
        let block = block_at(1, 1);
        let tris = triangulate(&block, Lod::FINEST, EdgeLods::NONE);
        assert_eq!(tris.len(), 32 * 32 * 6);
        let mut k = 0;
        for j in 0..QUAD_SIDE {
            for i in 0..QUAD_SIDE {
                let expect =
                    tessera_formats::landblock::quad_triangles(i, j, block.split_sw_ne(i, j));
                for t in expect {
                    let got: Vec<_> = tris[k..k + 3].iter().map(|&v| grid_xy(v)).collect();
                    let want: Vec<_> = t.iter().map(|&(a, b)| (a as i64, b as i64)).collect();
                    assert_eq!(got, want);
                    k += 3;
                }
            }
        }
    }

    fn mesh_of(block: &TerrainBlock, lod: Lod, edges: EdgeLods) -> TerrainMesh {
        let normals = VertexNormals::compute(block, &Neighbours::NONE);
        TerrainMesh::build(block, &normals, &HeightSlopeShading::default(), lod, edges)
    }

    #[test]
    fn mesh_streams_are_consistent() {
        let block = block_at(0, 0);
        for lod in Lod::all() {
            let m = mesh_of(&block, lod, EdgeLods::NONE);
            let n = m.positions.len();
            assert_eq!(m.normals.len(), n);
            assert_eq!(m.uvs.len(), n);
            assert_eq!(m.colors.len(), n);
            assert_eq!(m.grid_vertices.len(), n);
            let side = lod.quads_per_side() + 1;
            assert_eq!(n, side * side, "{lod}: only the level's vertices are kept");
            assert!(m.indices.iter().all(|&i| usize::from(i) < n));
            assert!(m.grid_vertices.windows(2).all(|w| w[0] < w[1]));
            assert!(m.min_height <= m.max_height);
            assert_eq!(m.triangle_count(), lod.quads_per_side().pow(2) * 2);
            for (k, p) in m.positions.iter().enumerate() {
                let g = usize::from(m.grid_vertices[k]);
                let (i, j) = (g % GRID_SIDE, g / GRID_SIDE);
                assert_eq!(p[2], block.height(i, j).unwrap());
                assert_eq!(m.uvs[k], [p[0] / 160.0, 1.0 - p[1] / 160.0]);
            }
        }
        // Level 0 keeps every vertex in grid order: grid numbers are mesh indices.
        let m0 = mesh_of(&block, Lod::FINEST, EdgeLods::NONE);
        assert_eq!(m0.positions.len(), GRID_SAMPLES);
        assert_eq!(m0.remap_grid_indices(&[0, 5, 1088]), Some(vec![0, 5, 1088]));
        let m3 = mesh_of(&block, Lod::new(3).unwrap(), EdgeLods::NONE);
        assert_eq!(m3.remap_grid_indices(&[1]), None);
        assert_eq!(m3.remap_grid_indices(&[8]), Some(vec![1]));
    }

    /// The level-0 mesh surface is the surface `height_at` evaluates.
    #[test]
    fn level_zero_mesh_matches_height_queries() {
        let block = block_at(4, 1);
        let m = mesh_of(&block, Lod::FINEST, EdgeLods::NONE);
        for t in m.indices.chunks_exact(3) {
            let [a, b, c] = [t[0], t[1], t[2]].map(|i| m.positions[usize::from(i)]);
            for (wa, wb) in [(0.2, 0.3), (0.6, 0.2), (0.33, 0.33), (0.05, 0.9)] {
                let wc = 1.0 - wa - wb;
                let p: [f64; 3] = std::array::from_fn(|k| {
                    f64::from(a[k]) * wa + f64::from(b[k]) * wb + f64::from(c[k]) * wc
                });
                let z = block.height_at(p[0] as f32, p[1] as f32).unwrap();
                assert!((f64::from(z) - p[2]).abs() < 2e-3, "{p:?}: {z}");
            }
        }
    }

    /// Seam test: across every pair of levels, the east/north neighbours mesh the shared
    /// edge with identical vertices (bit-identical positions after the block offset,
    /// identical normals and colours) and identical segments.
    #[test]
    fn neighbour_seams_are_identical_across_lod_transitions() {
        let blocks = [
            [block_at(0, 0), block_at(1, 0)],
            [block_at(0, 1), block_at(1, 1)],
        ];
        let nb = |x: usize, y: usize| {
            Neighbours(Side::ALL.map(|s| {
                let (dx, dy) = s.offset();
                let (nx, ny) = (x as i32 + dx, y as i32 + dy);
                ((0..2).contains(&nx) && (0..2).contains(&ny))
                    .then(|| &blocks[ny as usize][nx as usize])
            }))
        };
        let shading = HeightSlopeShading::default();
        // (vertex position in the global frame, normal, colour) along an edge, plus segments.
        type Vtx = ([u32; 3], [u32; 3], [u32; 4]);
        let edge_of = |m: &TerrainMesh, side: Side, off: [f32; 2]| {
            let on = |p: [f32; 3]| match side {
                Side::East => p[0] == 160.0,
                Side::West => p[0] == 0.0,
                Side::North => p[1] == 160.0,
                Side::South => p[1] == 0.0,
            };
            let key = |k: usize| -> Vtx {
                let p = m.positions[k];
                (
                    [
                        (p[0] + off[0]).to_bits(),
                        (p[1] + off[1]).to_bits(),
                        p[2].to_bits(),
                    ],
                    m.normals[k].map(f32::to_bits),
                    m.colors[k].map(f32::to_bits),
                )
            };
            let mut verts = BTreeSet::new();
            let mut segs = BTreeSet::new();
            for t in m.indices.chunks_exact(3) {
                for k in 0..3 {
                    let (a, b) = (usize::from(t[k]), usize::from(t[(k + 1) % 3]));
                    if on(m.positions[a]) && on(m.positions[b]) {
                        let (ka, kb) = (key(a), key(b));
                        verts.insert(ka);
                        verts.insert(kb);
                        segs.insert(if ka < kb { (ka, kb) } else { (kb, ka) });
                    }
                }
            }
            (verts, segs)
        };
        let mut checked = 0;
        for la in Lod::all() {
            for lb in Lod::all() {
                // East pair: (0,0) | (1,0).
                let (a, b) = (&blocks[0][0], &blocks[0][1]);
                let ma = TerrainMesh::build(
                    a,
                    &VertexNormals::compute(a, &nb(0, 0)),
                    &shading,
                    la,
                    EdgeLods::NONE.with(Side::East, Some(lb)),
                );
                let mb = TerrainMesh::build(
                    b,
                    &VertexNormals::compute(b, &nb(1, 0)),
                    &shading,
                    lb,
                    EdgeLods::NONE.with(Side::West, Some(la)),
                );
                let ea = edge_of(&ma, Side::East, [0.0, 0.0]);
                let eb = edge_of(&mb, Side::West, [160.0, 0.0]);
                assert_eq!(ea, eb, "east seam {la} | {lb}");
                let steps = 32 / la.max(lb).step();
                assert_eq!(ea.1.len(), steps, "east seam {la} | {lb}: segment count");
                // North pair: (0,0) / (0,1).
                let c = &blocks[1][0];
                let ma = TerrainMesh::build(
                    a,
                    &VertexNormals::compute(a, &nb(0, 0)),
                    &shading,
                    la,
                    EdgeLods::NONE.with(Side::North, Some(lb)),
                );
                let mc = TerrainMesh::build(
                    c,
                    &VertexNormals::compute(c, &nb(0, 1)),
                    &shading,
                    lb,
                    EdgeLods::NONE.with(Side::South, Some(la)),
                );
                let ea = edge_of(&ma, Side::North, [0.0, 0.0]);
                let ec = edge_of(&mc, Side::South, [0.0, 160.0]);
                assert_eq!(ea, ec, "north seam {la} / {lb}");
                checked += 2;
            }
        }
        assert_eq!(checked, 72);
    }

    /// A grid vertex has the same position, normal and colour at every level.
    #[test]
    fn vertices_are_identical_across_levels() {
        let block = block_at(3, 3);
        let normals = VertexNormals::compute(&block, &Neighbours::NONE);
        let shading = HeightSlopeShading::default();
        let full = TerrainMesh::build(&block, &normals, &shading, Lod::FINEST, EdgeLods::NONE);
        for lod in Lod::all() {
            let m = TerrainMesh::build(
                &block,
                &normals,
                &shading,
                lod,
                EdgeLods::uniform(Lod::COARSEST),
            );
            for (k, &g) in m.grid_vertices.iter().enumerate() {
                let g = usize::from(g);
                assert_eq!(m.positions[k], full.positions[g]);
                assert_eq!(m.normals[k], full.normals[g]);
                assert_eq!(m.colors[k], full.colors[g]);
            }
        }
    }

    #[test]
    fn flat_block_coarsest_is_two_triangles() {
        let b = TerrainBlock::flat(3277);
        let m = mesh_of(&b, Lod::COARSEST, EdgeLods::NONE);
        assert_eq!(m.positions.len(), 4);
        assert_eq!(m.triangle_count(), 2);
        assert_eq!(m.min_height, m.max_height);
    }
}
