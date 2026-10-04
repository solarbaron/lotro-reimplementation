// SPDX-License-Identifier: GPL-3.0-or-later
//! `tessera-terrain` (`docs/specs/terrain.md`): meshing, normals, shading, height queries,
//! LOD selection and splat-layer conversion on arbitrary inputs. Heights, split flags,
//! neighbours (including ones whose shared edge does not match), levels and neighbour
//! levels, palettes (NaN, unsorted, oversized), query points (NaN, infinite, far outside)
//! and surface chunks (bad windows, out-of-range vertex numbers, overlapping layers) all
//! come from the input. Nothing may panic; every mesh must be an in-range, counter-
//! clockwise cover of the block with finite streams.
//!
//! Input layout: an 8-byte header (level, four neighbour levels, flags, two seeds), then
//! the bytes every value is read from (cycled when short).

#![no_main]

use libfuzzer_sys::fuzz_target;
use tessera_formats::Did;
use tessera_formats::landblock::{
    GRID_SAMPLES, HeightGrid, SurfaceChunk, SurfaceLayer, VertexFlags,
};
use tessera_terrain::{
    ColorStop, EdgeLods, HeightSlopeShading, Lod, LodSelector, Neighbours, Side, TerrainBlock,
    TerrainMesh, TerrainTiles, VertexNormals, splat_layers,
};

/// Reads little-endian values from the input, cycling, zero when it is empty.
struct Bytes<'a> {
    data: &'a [u8],
    pos: usize,
}

impl Bytes<'_> {
    fn byte(&mut self) -> u8 {
        if self.data.is_empty() {
            return 0;
        }
        let b = self.data[self.pos % self.data.len()];
        self.pos += 1;
        b
    }
    fn u16(&mut self) -> u16 {
        u16::from_le_bytes([self.byte(), self.byte()])
    }
    fn u32(&mut self) -> u32 {
        u32::from_le_bytes([self.byte(), self.byte(), self.byte(), self.byte()])
    }
    fn f32(&mut self) -> f32 {
        f32::from_bits(self.u32())
    }
    /// Mostly in-range coordinates, sometimes raw floats (NaN, inf, huge).
    fn coord(&mut self) -> f32 {
        let tag = self.byte();
        if tag & 0x7 == 0 {
            self.f32()
        } else {
            f32::from(self.u16()) / 65_535.0 * 170.0 - 5.0
        }
    }
}

fn block(b: &mut Bytes<'_>, mode: u8) -> TerrainBlock {
    let mut raw = [0u16; GRID_SAMPLES];
    for h in &mut raw {
        *h = match mode % 3 {
            0 => b.u16(),
            1 => 30_000 + u16::from(b.byte()),
            _ => 0,
        };
    }
    let mut flags = VertexFlags::default();
    for f in flags.raw_mut().iter_mut() {
        *f = b.byte();
    }
    TerrainBlock::new(&HeightGrid::from_raw(raw), &flags)
}

fn check_mesh(m: &TerrainMesh) {
    let n = m.positions.len();
    assert!(n <= GRID_SAMPLES);
    assert_eq!(m.normals.len(), n);
    assert_eq!(m.uvs.len(), n);
    assert_eq!(m.colors.len(), n);
    assert_eq!(m.grid_vertices.len(), n);
    assert_eq!(m.indices.len() % 3, 0);
    assert!(m.indices.iter().all(|&i| usize::from(i) < n));
    assert!(m.positions.iter().flatten().all(|c| c.is_finite()));
    assert!(
        m.normals
            .iter()
            .all(|v| v.iter().all(|c| c.is_finite()) && v[2] > 0.0)
    );
    assert!(
        m.colors
            .iter()
            .flatten()
            .all(|c| c.is_finite() && (0.0..=1.0).contains(c))
    );
    let mut area = 0i64;
    for t in m.indices.chunks_exact(3) {
        let p = |k: u16| {
            let q = m.positions[usize::from(k)];
            ((q[0] / 5.0) as i64, (q[1] / 5.0) as i64)
        };
        let (a, b, c) = (p(t[0]), p(t[1]), p(t[2]));
        let a2 = (b.0 - a.0) * (c.1 - a.1) - (b.1 - a.1) * (c.0 - a.0);
        assert!(a2 > 0, "clockwise or degenerate triangle");
        area += a2;
    }
    assert_eq!(area, 2 * 32 * 32);
}

fuzz_target!(|data: &[u8]| {
    if data.len() < 8 {
        return;
    }
    let (head, rest) = data.split_at(8);
    let mut b = Bytes { data: rest, pos: 0 };
    let lod = Lod::clamped(head[0] % 8);
    let edge = |v: u8| (v & 8 != 0).then(|| Lod::clamped(v % 8));
    let edges = EdgeLods([edge(head[1]), edge(head[2]), edge(head[3]), edge(head[4])]);
    let flags = head[5];

    let centre = block(&mut b, head[6]);
    let others: Vec<TerrainBlock> = (0..4)
        .map(|k| {
            if flags & (1 << k) != 0 {
                block(&mut b, head[7] >> k)
            } else {
                centre.clone()
            }
        })
        .collect();
    let neighbours = Neighbours(
        Side::ALL.map(|s| (flags & (0x10 << s.index()) != 0).then(|| &others[s.index()])),
    );
    let normals = VertexNormals::compute(&centre, &neighbours);
    assert!(
        normals
            .as_slice()
            .iter()
            .all(|n| n.iter().all(|c| c.is_finite()))
    );

    let shading = if flags & 0x80 != 0 {
        HeightSlopeShading {
            stops: (0..usize::from(b.byte()))
                .map(|_| ColorStop {
                    height: b.f32(),
                    color: [b.f32(), b.f32(), b.f32()],
                })
                .collect(),
            cliff: [b.f32(), b.f32(), b.f32()],
            cliff_start_deg: b.f32(),
            cliff_full_deg: b.f32(),
        }
    } else {
        HeightSlopeShading::default()
    };
    let mesh = TerrainMesh::build(&centre, &normals, &shading, lod, edges);
    check_mesh(&mesh);
    assert_eq!(mesh.edge_lods, edges.effective(lod));
    let _ = mesh.remap_grid_indices(&[b.u16(), b.u16(), b.u16()]);

    // Level error bounds are finite, non-decreasing and bound every level's surface.
    let errors = centre.lod_errors();
    assert_eq!(errors[0], 0.0);
    assert!(errors.iter().all(|e| e.is_finite() && *e >= 0.0));
    assert!(errors.windows(2).all(|w| w[0] <= w[1]));

    // Height queries never panic and stay within the block's range when they answer.
    for _ in 0..8 {
        let (x, y) = (b.coord(), b.coord());
        if let Some(z) = centre.height_at(x, y) {
            assert!(z.is_finite());
            assert!(z >= centre.min_height() && z <= centre.max_height());
        }
        let _ = centre.height_bilinear(x, y);
        if let Some(n) = centre.normal_at(x, y) {
            assert!(n.iter().all(|c| c.is_finite()) && n[2] > 0.0);
        }
        if let (Some(z), Some(z0)) = (centre.lod_height_at(lod, x, y), centre.height_at(x, y)) {
            assert!(z >= centre.min_height() && z <= centre.max_height());
            assert!((z - z0).abs() <= centre.lod_error(lod) + 1e-2);
        }
        let d = centre.distance_to([x, y, b.coord()]);
        let sel = LodSelector {
            pixel_tolerance: b.coord(),
            projection_scale: b.coord() * 100.0,
            full_detail_radius: b.coord(),
            hysteresis: b.coord(),
        };
        let _ = sel.select(&centre, d, Some(lod));
        assert_eq!(sel.select(&centre, f32::NAN, Some(lod)), lod);
    }

    // Region-global queries.
    let mut tiles = TerrainTiles::new();
    let (tx, ty) = (b.byte(), b.byte());
    if tiles
        .insert(tx, ty, std::sync::Arc::new(centre.clone()))
        .is_ok()
    {
        for _ in 0..4 {
            let gx = f64::from(b.coord()) * 300.0;
            let gy = f64::from(b.coord()) * 300.0;
            let _ = tiles.height_at(gx, gy);
            let _ = tiles.ground_at(gx, gy);
        }
        let _ = tiles.normals(tx, ty);
    }

    // Surface chunk with hostile windows and vertex numbers.
    let layer_count = usize::from(b.byte() % 12);
    let index_count = usize::from(b.u16() % 400);
    let chunk = SurfaceChunk {
        layers: (0..layer_count)
            .map(|_| SurfaceLayer {
                material: b.u32(),
                mask: Did(b.u32()),
                whole: b.byte() & 1 != 0,
                start: b.u16() % 512,
                prims: b.u16() % 160,
                min_vertex: b.u16(),
                vertex_span: b.u16(),
            })
            .collect(),
        indices: (0..index_count).map(|_| b.u16() % 1_100).collect(),
        quads: None,
    };
    if let Ok(layers) = splat_layers(&chunk) {
        let copied: usize = layers.iter().map(|l| l.indices.len()).sum();
        assert!(copied <= chunk.indices.len());
        // Grid numbers always resolve against the full, unstitched level-0 mesh.
        let full = TerrainMesh::build(&centre, &normals, &shading, Lod::FINEST, EdgeLods::NONE);
        for l in &layers {
            assert!(l.indices.iter().all(|&v| usize::from(v) < GRID_SAMPLES));
            assert!(full.remap_grid_indices(&l.indices).is_some());
        }
    }
});
