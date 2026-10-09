// SPDX-License-Identifier: GPL-3.0-or-later
//! Synthetic tests of the far view: invariants FV-I1–FV-I12 and the worked examples of
//! `far-view.md` §14.

use std::collections::BTreeSet;
use std::sync::Arc;

use tessera_formats::landblock::{
    SectorChunk, SectorEntity, SectorMesh, SectorSample, SectorSprite, SectorWaterPair,
    chunk_index, vertex_block,
};
use tessera_formats::{Did, ImposterEntry, ImposterTable, Position, SectorRecord, TSize};

use super::*;
use crate::coords::{LandblockKey, WorldPos};

fn lb(x: u8, y: u8) -> LandblockKey {
    LandblockKey::new(1, x, y).expect("valid landblock")
}

fn viewer(x: u8, y: u8) -> Viewer {
    Viewer {
        landblock: lb(x, y),
        instance: 0,
    }
}

/// A sector record whose heights rise with x and y (so every sample differs).
fn record(key: SectorKey, ceiling_mask: u32) -> SectorRecord {
    let mut terrain = SectorMesh::default();
    for k in 0..9usize {
        for s in 0..1089usize {
            terrain.samples.push(SectorSample {
                height: u16::try_from(k * 1000 + s).unwrap_or(0),
                normal: [127, 255, 127],
            });
        }
    }
    for x in 0..3 {
        for y in 0..3 {
            let k = u32::try_from(vertex_block(x, y)).unwrap_or(0);
            terrain.chunks[chunk_index(x, y)] = SectorChunk::expected(k, x, y, 0.0, 1.0);
        }
    }
    let ceiling = (ceiling_mask != 0).then(|| {
        let mut c = SectorMesh::default();
        let mut rank = 0u32;
        for x in 0..3 {
            for y in 0..3 {
                if ceiling_mask & (1 << chunk_index(x, y)) != 0 {
                    for _ in 0..1089 {
                        c.samples.push(SectorSample {
                            height: 40_000,
                            normal: [127, 0, 127],
                        });
                    }
                    c.chunks[chunk_index(x, y)] = SectorChunk::expected(rank, x, y, 0.0, 1.0);
                    rank += 1;
                }
            }
        }
        c
    });
    SectorRecord {
        did: key.did(),
        terrain,
        mode: match ceiling_mask {
            0 => 0,
            0x1FF => 1,
            _ => 2,
        },
        ceiling_mask,
        ceiling,
        block_size: 160.0,
        water: Vec::new(),
        sprites: Vec::new(),
        entities: Vec::new(),
        colour_list: Did(0x4000_1000),
        ceiling_colour_list: Did(0),
        tile: Did(0x4100_1000),
        ceiling_tile: Did(0),
        flow_map: Did(0),
        imposter_table: Did(0x2701_0000),
    }
}

// ---------------------------------------------------------------------------------------
// Settings (FV-I1, §2 table)
// ---------------------------------------------------------------------------------------

#[test]
fn radii_and_extent_follow_the_level_table() {
    let want = [
        (2, 1, 640.0),
        (4, 2, 1_120.0),
        (6, 3, 1_600.0),
        (9, 4, 2_080.0),
        (12, 5, 2_560.0),
    ];
    for (level, (r, f, e)) in want.iter().enumerate() {
        let level = u32::try_from(level).unwrap_or(0);
        assert_eq!(settings::landscape_radius(level), *r);
        let s = FarViewSettings::from_level(level);
        assert_eq!(s.far_radius, *f);
        assert_eq!(s.near_radius, 1);
        assert!((s.extent() - e).abs() < 1e-3, "level {level}");
    }
    // Any other stored level gives R = 6.
    assert_eq!(settings::landscape_radius(7), 6);
    assert_eq!(FarViewSettings::from_level(99).far_radius, 3);
}

#[test]
fn far_fade_is_fd1() {
    let f = FarFade::for_extent(2_080.0);
    assert!((f.factor(1_000.0) - 1.0).abs() < 1e-6);
    assert!((f.factor(1_920.0) - 1.0).abs() < 1e-6);
    assert!((f.factor(2_000.0) - 0.5).abs() < 1e-6);
    assert!(f.factor(2_080.0).abs() < 1e-6);
    assert!(f.factor(f32::NAN).abs() < 1e-6);
}

#[test]
fn template_and_sprite_gates() {
    let mut s = FarViewSettings::default();
    assert_eq!(s.template(), Template::Plain);
    s.normal_maps = true;
    s.graphics_core = 1;
    assert_eq!(s.template(), Template::Plain);
    s.graphics_core = 5;
    assert_eq!(s.template(), Template::NormalMapped);
    s.seam_blending = true;
    assert!(s.seam_overlay());
    assert!(s.sprites());
    s.device_class = 2;
    assert!(!s.sprites());
    assert!(FarViewSettings::default().validate().is_ok());
    s.far_radius = 0;
    assert!(s.validate().is_err());
}

// ---------------------------------------------------------------------------------------
// Sector list (FV-I2, §14.1)
// ---------------------------------------------------------------------------------------

#[test]
fn worked_example_14_1_resident_set() {
    let v = lb(100, 50);
    assert_eq!(
        SectorKey::of_landblock(v),
        SectorKey::new(1, 33, 16).unwrap_or_else(|| unreachable!())
    );
    let list = sector_list(v, 4);
    assert_eq!(list.len(), 81);
    let xs: BTreeSet<u8> = list.iter().map(|k| k.sx()).collect();
    let ys: BTreeSet<u8> = list.iter().map(|k| k.sy()).collect();
    assert_eq!((xs.first(), xs.last()), (Some(&29), Some(&37)));
    assert_eq!((ys.first(), ys.last()), (Some(&12), Some(&20)));
    // Landblocks X 87–113, Y 36–62.
    let corner_x: Vec<u8> = list.iter().map(|k| k.corner().0).collect();
    assert_eq!(corner_x.iter().min(), Some(&87));
    assert_eq!(corner_x.iter().max().map(|m| m + 2), Some(113));
    // Column by column: X outer, Y inner.
    assert_eq!(
        list[0],
        SectorKey::new(1, 29, 12).unwrap_or_else(|| unreachable!())
    );
    assert_eq!(
        list[1],
        SectorKey::new(1, 29, 13).unwrap_or_else(|| unreachable!())
    );
    assert_eq!(
        list[9],
        SectorKey::new(1, 30, 12).unwrap_or_else(|| unreachable!())
    );
}

#[test]
fn sector_list_invariant_fv_i2() {
    for f in 1..=5u32 {
        for (x, y) in [(0u8, 0u8), (254, 254), (100, 50), (2, 253), (130, 1)] {
            let v = lb(x, y);
            let list = sector_list(v, f);
            assert!(list.len() <= ((2 * f + 1) * (2 * f + 1)) as usize);
            for k in &list {
                assert!(k.sx() <= 84 && k.sy() <= 84);
                let (cx, cy) = k.corner();
                assert!(u32::from(cx.abs_diff(x)) <= 3 * f + 2);
                assert!(u32::from(cy.abs_diff(y)) <= 3 * f + 2);
            }
            let set: BTreeSet<_> = list.iter().collect();
            assert_eq!(set.len(), list.len(), "no duplicates");
        }
    }
}

#[test]
fn sector_did_shape() {
    let k = SectorKey::new(1, 1, 3).unwrap_or_else(|| unreachable!());
    assert_eq!(k.did(), Did(0x8051_0309));
    assert_eq!(SectorKey::new(1, 85, 0), None);
    assert_eq!(SectorKey::new(0, 1, 0), None);
}

// ---------------------------------------------------------------------------------------
// Grid (FV-I3, FV-I4, §14.2)
// ---------------------------------------------------------------------------------------

#[test]
fn lod_bands() {
    assert_eq!(lod_of(0, 0), 0);
    assert_eq!(lod_of(1, 1), 0);
    assert_eq!(lod_of(2, 0), 1);
    assert_eq!(lod_of(4, 2), 1);
    assert_eq!(lod_of(5, 0), 2);
    assert_eq!(lod_of(9, 4), 2);
    assert_eq!(lod_of(10, 0), 3);
}

fn full_grid(v: LandblockKey, f: u32, near: &NearField) -> (Vec<SectorKey>, LandblockGrid) {
    let list = sector_list(v, f);
    let grid = LandblockGrid::build(v, &list, |_| true, near);
    (list, grid)
}

#[test]
fn grid_invariants_fv_i3_i4() {
    for (x, y) in [(100u8, 50u8), (1, 1), (253, 120), (99, 99)] {
        let v = lb(x, y);
        let (_, grid) = full_grid(v, 5, &NearField::Radius(1));
        for ((gx, gy), c) in grid.cells() {
            let cheb = gx.abs_diff(x).max(gy.abs_diff(y));
            match c {
                Cell::Near(l) => {
                    assert!(cheb <= 1);
                    assert_eq!(l, 0);
                }
                Cell::Far(l) => {
                    assert!(cheb > 1);
                    assert!(l >= 1, "FV-I3: far landblock with LOD 0");
                }
                Cell::Uncovered => {}
            }
            // FV-I4: edge neighbours differ by at most one LOD.
            for (dx, dy) in grid::EDGES {
                let n = grid.cell(i32::from(gx) + dx, i32::from(gy) + dy);
                if let (Some(a), Some(b)) = (c.lod(), n.lod()) {
                    assert!(a.abs_diff(b) <= 1, "FV-I4 at ({gx}, {gy})");
                }
            }
        }
    }
}

#[test]
fn worked_example_14_2_stitching() {
    let v = lb(100, 50);
    let (_, grid) = full_grid(v, 4, &NearField::Radius(1));
    let desc_of = |dx: i32, dy: i32| {
        let (x, y) = (100 + dx, 50 + dy);
        let key = SectorKey::new(
            1,
            u8::try_from(x / 3).unwrap_or(0),
            u8::try_from(y / 3).unwrap_or(0),
        )
        .unwrap_or_else(|| unreachable!());
        let d = grid.descriptors(key, 0);
        let c = usize::try_from((x % 3) + 3 * (y % 3)).unwrap_or(0);
        d[c]
    };
    // (2, 0): far, LOD 1, stitches its west edge to (1, 0) (near): 16 triangles.
    let d = desc_of(2, 0);
    assert!(d.far && d.lod == 1);
    assert!(d.edges[0].stitch && d.edges[0].neighbour_lod == 0);
    assert_eq!(
        mesh::stitch_triangle_count(d.lod, d.edges[0].neighbour_lod),
        16
    );
    // (4, 2): far, LOD 1.
    assert_eq!(desc_of(4, 2).lod, 1);
    // (5, 0): LOD 2, stitches west to (4, 0) LOD 1: 8 triangles.
    let d = desc_of(5, 0);
    assert_eq!(d.lod, 2);
    assert!(d.edges[0].stitch && d.edges[0].neighbour_lod == 1);
    assert_eq!(mesh::stitch_triangle_count(2, 1), 8);
    // (9, 4): LOD 2. (10, 0): LOD 3, stitches west to (9, 0): 4 triangles.
    assert_eq!(desc_of(9, 4).lod, 2);
    let d = desc_of(10, 0);
    assert_eq!(d.lod, 3);
    assert!(d.edges[0].stitch && d.edges[0].neighbour_lod == 2);
    assert_eq!(mesh::stitch_triangle_count(3, 2), 4);
    // The near landblocks are not far and do not stitch.
    let d = desc_of(1, 1);
    assert!(!d.far && d.lod == 0 && d.edges.iter().all(|e| !e.stitch));
    // A coarser far neighbour never makes the finer side stitch.
    let d = desc_of(4, 0);
    assert!(!d.edges[2].stitch, "(4, 0) toward the coarser (5, 0)");
}

#[test]
fn uncovered_neighbours_leave_edges_open() {
    let v = lb(100, 50);
    let list = sector_list(v, 2);
    let missing = SectorKey::new(1, 34, 16).unwrap_or_else(|| unreachable!());
    let grid = LandblockGrid::build(v, &list, |k| k != missing, &NearField::Radius(1));
    // Landblock (101, 50) is near (Chebyshev 1); (102, 50) belongs to the missing sector.
    assert_eq!(grid.cell(102, 50), Cell::Uncovered);
    let own = SectorKey::new(1, 33, 16).unwrap_or_else(|| unreachable!());
    let d = grid.descriptors(own, 0);
    // (101, 50) is chunk x = 2, y = 2 of sector (33, 16) (corner 99, 48).
    let e = d[2 + 3 * 2].edges[2];
    assert!(!e.stitch);
}

#[test]
fn explicit_near_set_stitches_across_a_missing_sector() {
    // The missing sector is (34, 16) (landblocks 102–104 × 48–50). The walker draws
    // (102, 50) at full detail although no record covers it: with an explicit near set it
    // is near, so its far neighbour (102, 51) of sector (34, 17) stitches toward it.
    let v = lb(100, 50);
    let list = sector_list(v, 2);
    let missing = SectorKey::new(1, 34, 16).unwrap_or_else(|| unreachable!());
    let near: BTreeSet<(u8, u8)> = [(101, 50), (102, 50), (100, 50)].into_iter().collect();
    let grid = LandblockGrid::build(v, &list, |k| k != missing, &NearField::Set(Arc::new(near)));
    assert_eq!(grid.cell(102, 50), Cell::Near(1));
    // Other landblocks of the missing sector stay uncovered.
    assert_eq!(grid.cell(103, 50), Cell::Uncovered);
    // (102, 51) of sector (34, 17) is far and stitches its south edge toward (102, 50).
    let above = SectorKey::new(1, 34, 17).unwrap_or_else(|| unreachable!());
    let d = grid.descriptors(above, 0);
    // (102, 51) is chunk x = 0, y = 0 of sector (34, 17) (corner 102, 51).
    let own = d[0];
    assert!(own.far);
    assert!(own.edges[1].stitch, "south edge toward the near (102, 50)");
    assert_eq!(own.edges[1].neighbour_lod, 0);
    // With the original's radius the same landblock stays uncovered (§4).
    let grid = LandblockGrid::build(v, &list, |k| k != missing, &NearField::Radius(1));
    assert_eq!(grid.cell(102, 50), Cell::Uncovered);
}

#[test]
fn far_landblocks_follow_the_descriptors() {
    let v = lb(100, 50);
    let (_, grid) = full_grid(v, 1, &NearField::Radius(1));
    let key = SectorKey::of_landblock(v);
    let rec = record(key, 0);
    let j = BuildJob {
        key,
        generation: 1,
        record: Arc::new(rec),
        descriptors: grid.descriptors(key, 0),
        near_batches: false,
    };
    let m = j.run().unwrap_or_else(|e| panic!("{e}"));
    let far: BTreeSet<(u8, u8)> = m.far_landblocks().collect();
    // Sector (33, 16) holds landblocks 99–101 × 48–50; the near field 99–101 × 49–51
    // leaves its south row far.
    assert_eq!(
        far,
        [(99, 48), (100, 48), (101, 48)]
            .into_iter()
            .collect::<BTreeSet<_>>()
    );
    assert_eq!(m.terrain.landblocks, 3);
}

#[test]
fn failed_build_settles_and_is_logged_apart() {
    let settings = FarViewSettings {
        imposters: false,
        ..FarViewSettings::from_level(0)
    };
    let mut st = FarViewState::new(settings, None);
    let v = viewer(100, 50);
    let w = st.update(Some(v), NearField::Radius(1), SpriteView::default());
    for k in &w.load_sectors {
        st.sector_loaded(*k, Ok(Some(record(*k, 0))));
    }
    let w = st.update(Some(v), NearField::Radius(1), SpriteView::default());
    assert_eq!(w.build.len(), 9);
    let failing = w.build[0].key;
    for j in &w.build {
        if j.key == failing {
            let e = crate::error::WorldError::Config("synthetic build failure");
            assert!(!st.built(j.key, j.generation, Err(e)));
        } else {
            let m = j.run();
            assert!(st.built(j.key, j.generation, m));
        }
    }
    // The failed sector has no batches but no longer holds the view back.
    let slot = st.slot(failing).unwrap_or_else(|| unreachable!());
    assert!(slot.meshes.is_none());
    assert_eq!(slot.build_failed, Some(slot.generation));
    assert!(st.settled());
}

// ---------------------------------------------------------------------------------------
// Meshes (FV-I5, FV-I6)
// ---------------------------------------------------------------------------------------

#[test]
fn far_and_near_batches_partition_the_covered_landblocks_fv_i5() {
    let v = lb(100, 50);
    let (list, grid) = full_grid(v, 3, &NearField::Radius(1));
    let mut far_total = 0u32;
    let mut near_total = 0u32;
    for k in &list {
        let rec = record(*k, 0);
        let d = grid.descriptors(*k, 0);
        let far = build_batch(
            rec.did,
            &rec.terrain,
            160.0,
            &d,
            MeshKind::Terrain,
            Batch::Far,
        )
        .unwrap_or_else(|e| panic!("{e}"));
        let near = build_batch(
            rec.did,
            &rec.terrain,
            160.0,
            &d,
            MeshKind::Terrain,
            Batch::Near,
        )
        .unwrap_or_else(|e| panic!("{e}"));
        far_total += far.landblocks;
        near_total += near.landblocks;
        for i in &far.indices {
            assert!((*i as usize) < far.positions.len());
        }
    }
    assert_eq!(near_total, 9, "the 3 × 3 near field");
    assert_eq!(far_total + near_total, 21 * 21);
}

#[test]
fn lod0_landblock_matches_the_generated_index_list() {
    // One landblock at LOD 0 without stitching gives the generated list's first range
    // (2,048 triangles) over the same samples.
    let key = SectorKey::new(1, 0, 0).unwrap_or_else(|| unreachable!());
    let rec = record(key, 0);
    let mut d = [LandblockDescriptor::default(); 9];
    d[0] = LandblockDescriptor {
        present: true,
        far: true,
        lod: 0,
        ..LandblockDescriptor::default()
    };
    let m = build_batch(
        rec.did,
        &rec.terrain,
        160.0,
        &d,
        MeshKind::Terrain,
        Batch::Far,
    )
    .unwrap_or_else(|e| panic!("{e}"));
    assert_eq!(m.grid_triangles(), 2_048);
    assert_eq!(m.positions.len(), 1_089);
    let list = generated_index_list(MeshKind::Terrain);
    assert_eq!(list.len(), 9 * 8_160);
    // Compare as positions (vertex orders differ).
    let pos_of = |sample: u32| {
        let s = sample as usize % 1_089;
        let (i, j) = (s % 33, s / 33);
        #[allow(clippy::cast_precision_loss)]
        [i as f32 * 5.0, j as f32 * 5.0]
    };
    for t in 0..2_048 {
        for c in 0..3 {
            let g = pos_of(list[3 * t + c]);
            let p = m.positions[m.indices[3 * t + c] as usize];
            assert!(
                (g[0] - p[0]).abs() < 1e-4 && (g[1] - p[1]).abs() < 1e-4,
                "triangle {t}"
            );
        }
    }
    // Counter-clockwise from above.
    for t in m.indices.chunks(3) {
        let [a, b, c] = [t[0], t[1], t[2]].map(|i| m.positions[i as usize]);
        let cross = (b[0] - a[0]) * (c[1] - a[1]) - (b[1] - a[1]) * (c[0] - a[0]);
        assert!(cross > 0.0);
    }
    // Texture coordinates (x / 480, 1 − y / 480).
    for (p, uv) in m.positions.iter().zip(&m.uvs) {
        assert!((uv[0] - p[0] / 480.0).abs() < 1e-5);
        assert!((uv[1] - (1.0 - p[1] / 480.0)).abs() < 1e-5);
    }
}

#[test]
fn generated_ranges_match_the_stored_lod_ranges() {
    let key = SectorKey::new(1, 0, 0).unwrap_or_else(|| unreachable!());
    let rec = record(key, 0);
    let list = generated_index_list(MeshKind::Terrain);
    for c in &rec.terrain.chunks {
        for (l, r) in c.lods.iter().enumerate() {
            let first = r.first_index as usize;
            let end = first + 3 * r.triangles as usize;
            assert!(end <= list.len());
            // Every index of the range lies in the chunk's own block.
            let lo = c.vertex_start;
            for i in &list[first..end] {
                assert!(*i >= lo && *i < lo + 1_089, "LOD {l}");
            }
        }
    }
}

#[test]
fn stitch_counts_fv_i6() {
    for lod in 1..=3u8 {
        let key = SectorKey::new(1, 0, 0).unwrap_or_else(|| unreachable!());
        let rec = record(key, 0);
        let mut d = [LandblockDescriptor::default(); 9];
        d[0] = LandblockDescriptor {
            present: true,
            far: true,
            lod,
            edges: [
                Edge {
                    stitch: true,
                    neighbour_lod: lod - 1,
                },
                Edge::default(),
                Edge::default(),
                Edge::default(),
            ],
            ..LandblockDescriptor::default()
        };
        let m = build_batch(
            rec.did,
            &rec.terrain,
            160.0,
            &d,
            MeshKind::Terrain,
            Batch::Far,
        )
        .unwrap_or_else(|e| panic!("{e}"));
        assert_eq!(m.stitch_triangles(), 32 >> lod);
        let q = 32usize >> lod;
        assert_eq!(m.grid_triangles(), q * q * 2);
        // Every stitching triangle lies on the west edge (x = 0).
        for t in m.indices[m.stitch_start..].chunks(3) {
            for i in t {
                assert!(m.positions[*i as usize][0].abs() < 1e-6);
            }
        }
    }
}

#[test]
fn ceiling_batches_face_down_and_use_their_mask() {
    let key = SectorKey::new(1, 0, 0).unwrap_or_else(|| unreachable!());
    let rec = record(key, 0b1_0000_0001);
    let mut d = [LandblockDescriptor::default(); 9];
    for (c, x) in d.iter_mut().enumerate() {
        *x = LandblockDescriptor {
            present: true,
            far: true,
            ceiling: rec.ceiling_mask & (1 << c) != 0,
            lod: 1,
            ..LandblockDescriptor::default()
        };
    }
    let ceiling = rec.ceiling.as_ref().unwrap_or_else(|| unreachable!());
    let m = build_batch(rec.did, ceiling, 160.0, &d, MeshKind::Ceiling, Batch::Far)
        .unwrap_or_else(|e| panic!("{e}"));
    assert_eq!(m.landblocks, 2);
    for t in m.indices.chunks(3) {
        let [a, b, c] = [t[0], t[1], t[2]].map(|i| m.positions[i as usize]);
        let cross = (b[0] - a[0]) * (c[1] - a[1]) - (b[1] - a[1]) * (c[0] - a[0]);
        assert!(cross < 0.0, "clockwise from above");
    }
}

#[test]
fn hostile_block_size_is_rejected() {
    let key = SectorKey::new(1, 0, 0).unwrap_or_else(|| unreachable!());
    let rec = record(key, 0);
    let d = [LandblockDescriptor {
        present: true,
        far: true,
        ..LandblockDescriptor::default()
    }; 9];
    for bs in [f32::NAN, 0.0, -1.0, f32::INFINITY, 1e9] {
        assert!(build_batch(rec.did, &rec.terrain, bs, &d, MeshKind::Terrain, Batch::Far).is_err());
    }
    // A mesh without samples fails instead of panicking.
    let empty = SectorMesh::default();
    assert!(build_batch(rec.did, &empty, 160.0, &d, MeshKind::Terrain, Batch::Far).is_err());
}

#[test]
fn larger_near_set_is_skipped_and_stitched_with_fans() {
    // FD9: a near field of Chebyshev 4 (walker rings): far landblocks next to it have
    // LOD 2 or 3 and stitch to its full-resolution edge.
    let v = lb(100, 50);
    let mut set = BTreeSet::new();
    for x in 96..=104u8 {
        for y in 46..=54u8 {
            set.insert((x, y));
        }
    }
    let near = NearField::Set(Arc::new(set));
    let (list, grid) = full_grid(v, 3, &near);
    let mut near_blocks = 0;
    for ((_, _), c) in grid.cells() {
        if matches!(c, Cell::Near(_)) {
            near_blocks += 1;
        }
    }
    assert_eq!(near_blocks, 81);
    // (105, 50): far, LOD 2 (d² 25), stitches west to the near edge with fans of 3.
    let key = SectorKey::new(1, 35, 16).unwrap_or_else(|| unreachable!());
    assert!(list.contains(&key));
    let d = grid.descriptors(key, 0);
    let c = 6; // (105, 50) = corner (105, 48) + (0, 2): chunk 0 + 3 × 2.
    assert_eq!(d[c].lod, 2);
    assert!(d[c].edges[0].stitch && d[c].edges[0].neighbour_lod == 0);
    assert_eq!(mesh::stitch_triangle_count(2, 0), 8 * 3);
}

// ---------------------------------------------------------------------------------------
// Shading terms (FV-I7–FV-I10, §14.3–§14.5)
// ---------------------------------------------------------------------------------------

#[test]
fn near_view_mask_fv_i7() {
    for p in [-159.0, 0.0, 100.0, 319.0] {
        for q in [-159.0, 50.0, 319.0] {
            assert!(shading::near_view_mask(p, q).abs() < 1e-6);
        }
    }
    for (p, q) in [(-160.5, 0.0), (0.0, 320.5), (400.0, 400.0), (-1_000.0, 0.0)] {
        assert!(
            (shading::near_view_mask(p, q) - 1.0).abs() < 1e-6,
            "({p}, {q})"
        );
    }
}

#[test]
fn seam_overlay_and_normal_strength_fv_i8_i9_example_14_4() {
    assert!(shading::seam_overlay_coverage(80.0, 1.0).abs() < 1e-6);
    assert!((shading::seam_overlay_coverage(160.0, 0.9) - 0.81).abs() < 1e-5);
    assert!(shading::seam_overlay_coverage(1e6, 2.0) <= 1.0);
    assert!(shading::normal_strength(80.0).abs() < 1e-6);
    assert!((shading::normal_strength(240.0) - 1.0).abs() < 1e-6);
    // §14.4.
    assert!((shading::normal_strength(120.0) - 0.25).abs() < 1e-6);
    assert!((shading::seam_overlay_coverage(120.0, 0.9) - 0.405).abs() < 1e-5);
    assert!((shading::seam_overlay_coverage(200.0, 0.9) - 0.81).abs() < 1e-5);
    assert!((shading::normal_strength(200.0) - 0.75).abs() < 1e-6);
}

#[test]
fn worked_example_14_3_plain_texel() {
    let c = shading::plain_colour(
        [0.40, 0.35, 0.30],
        [0.5, 0.5, 0.5],
        [0.6, 0.6, 0.6],
        0.25,
        [0.70, 0.75, 0.80],
    );
    for (a, b) in c.iter().zip([0.535, 0.5025, 0.47]) {
        assert!((a - b).abs() < 1e-5, "{c:?}");
    }
}

#[test]
fn sprite_fade_fv_i10_example_14_5() {
    let none = FarFade::NONE;
    // Tree sprite, distance 140, c = 1: invisible closer than 115, complete from 125.
    assert!(shading::sprite_opacity(115.0, 140.0, 1.0, &none).abs() < 1e-6);
    assert!((shading::sprite_opacity(125.0, 140.0, 1.0, &none) - 1.0).abs() < 1e-6);
    // M_d = 2: 255 to 265.
    assert!(shading::sprite_opacity(255.0, 140.0, 2.0, &none).abs() < 1e-6);
    assert!((shading::sprite_opacity(265.0, 140.0, 2.0, &none) - 1.0).abs() < 1e-6);
    // Far sprite: S = 12, P = 1,080, φ = 45°: D × c = 66.54.
    let view = SpriteView {
        target_height: 1_080.0,
        fov_y: std::f32::consts::FRAC_PI_4,
        object_scale: 1.0,
    };
    assert!((sprites::far_distance(12.0, &view) - 66.54).abs() < 0.01);
    // M_d cancels: D × c is the same at M_d = 2.
    let view2 = SpriteView {
        object_scale: 2.0,
        ..view
    };
    assert!((sprites::far_distance(12.0, &view2) * 2.0 - 66.54).abs() < 0.01);
}

#[test]
fn mirror_flag_example_14_6_and_fv_i11() {
    assert!(sprites::mirror_flag(200.0, 40.0, 64));
    assert!(!sprites::mirror_flag(200.0, 40.0, 0));
    // FV-I11: only the byte, the position and the viewer landblock matter.
    assert_eq!(
        sprites::mirror_flag(10.0, 20.0, 7),
        sprites::mirror_flag(10.0, 20.0, 7)
    );
}

fn table() -> ImposterTable {
    ImposterTable {
        did: Did(0x2701_0000),
        size: TSize { hint: 1, count: 2 },
        entries: vec![
            ImposterEntry {
                name_key: 600,
                column: 0,
                row: 0,
                value_a: 10.0,
                value_b: -1.0,
            },
            ImposterEntry {
                name_key: 601,
                column: 1,
                row: 0,
                value_a: 4.0,
                value_b: 0.0,
            },
        ],
        colour_atlas: Did(0x4000_2000),
        second_atlas: Did(0x4000_2001),
        cell_u: 0.060_546_875,
        cell_v: 0.484_375,
    }
}

fn sprite(key: u16, east: f32, north: f32, distance: f32, byte: u8) -> SectorSprite {
    SectorSprite {
        name_key: key,
        east,
        height: 50.0,
        north,
        scale: 1.5,
        distance,
        byte,
    }
}

#[test]
fn sprite_placement_takes_the_first_entry_of_a_repeated_key() {
    let mut t = table();
    t.entries.push(ImposterEntry {
        name_key: 600,
        column: 2,
        row: 0,
        value_a: 99.0,
        value_b: 0.0,
    });
    let mut out = Vec::new();
    let skipped = place_sprites(
        &[sprite(600, 10.0, 10.0, 140.0, 0)],
        &t,
        [0.0, 0.0],
        &SpriteView::default(),
        &mut out,
    );
    assert_eq!(skipped, 0);
    assert_eq!(out.len(), 1);
    assert_eq!(
        out[0].size,
        10.0 * 1.5,
        "first entry, as ImposterTable::get"
    );
    assert_eq!(t.get(600).map(|e| e.value_a), Some(10.0));
}

#[test]
fn sprite_placement_skips_unknown_keys_and_picks_distances() {
    let t = table();
    let view = SpriteView::default();
    let mut out = Vec::new();
    // Sector corner = viewer landblock corner (offset 0): (100, 300) is in the near window,
    // (400, 400) outside it.
    let sprites = [
        sprite(600, 100.0, 300.0, 140.0, 0),
        sprite(601, 400.0, 400.0, 80.0, 64),
        sprite(999, 10.0, 10.0, 140.0, 0),
        SectorSprite {
            scale: f32::NAN,
            ..sprite(600, 1.0, 1.0, 140.0, 0)
        },
    ];
    let skipped = place_sprites(&sprites, &t, [0.0, 0.0], &view, &mut out);
    assert_eq!(skipped, 2);
    assert_eq!(out.len(), 2);
    assert!((out[0].distance - 140.0).abs() < 1e-6);
    assert!((out[0].size - 15.0).abs() < 1e-6);
    assert!((out[0].base + 1.5).abs() < 1e-6);
    assert!((out[1].distance - sprites::far_distance(6.0, &view)).abs() < 1e-4);
    // Byte 64 at (400, 400): (400 − 160) × 1 > 0, mirrored: u values swapped minus 1.
    assert!(out[1].mirror);
    let cell = t.cell(&t.entries[1]).unwrap_or_default();
    assert!((out[1].u[0] - (cell.u1 - 1.0)).abs() < 1e-6);
    assert!((out[1].u[1] - (cell.u0 - 1.0)).abs() < 1e-6);
}

#[test]
fn imposter_cells_follow_i7() {
    let t = table();
    let c = t.cell(&t.entries[1]).unwrap_or_default();
    // step_u = cell_u × 32 / 31 = 1 / 16.
    assert!((c.u0 - 1.0 / 16.0).abs() < 1e-6);
    assert!((c.u1 - (1.0 / 16.0 + 0.060_546_875)).abs() < 1e-6);
    assert!(c.v0.abs() < 1e-6 && (c.v1 - 0.484_375).abs() < 1e-6);
}

// ---------------------------------------------------------------------------------------
// Horizon (§14.7)
// ---------------------------------------------------------------------------------------

#[test]
fn worked_example_14_7_horizon() {
    let floor = horizon::floor(Some(120.0), Some(140.0)).unwrap_or_default();
    assert!((floor - 135.0).abs() < 1e-6);
    let bounds = MeshBounds {
        min: [-1_000.0, -1_000.0, -250.0],
        max: [1_000.0, 900.0, 250.0],
    };
    let p = horizon::placement(185.0, floor, 2_080.0, &bounds).unwrap_or_else(|| unreachable!());
    assert!((p.drop - 19.23).abs() < 0.01);
    assert!((p.scale_z - 1.0385).abs() < 1e-4);
    // Eye below the floor: no stretch.
    let p = horizon::placement(100.0, floor, 2_080.0, &bounds).unwrap_or_else(|| unreachable!());
    assert!(p.drop.abs() < 1e-6 && (p.scale_z - 1.0).abs() < 1e-6);
    assert!(horizon::placement(f32::NAN, floor, 2_080.0, &bounds).is_none());
}

// ---------------------------------------------------------------------------------------
// Residency (§3.4, §3.5)
// ---------------------------------------------------------------------------------------

#[test]
fn rebuild_rules() {
    let a = viewer(100, 50);
    assert_eq!(classify(Some(a), Some(a)), Change::None);
    assert_eq!(classify(Some(a), Some(viewer(101, 51))), Change::Rebuild);
    assert_eq!(classify(Some(a), Some(viewer(102, 50))), Change::Discard);
    let other_region = Viewer {
        landblock: LandblockKey::new(2, 100, 50).unwrap_or_else(|_| unreachable!()),
        instance: 0,
    };
    assert_eq!(classify(Some(a), Some(other_region)), Change::Discard);
    let inst = Viewer { instance: 3, ..a };
    assert_eq!(classify(Some(a), Some(inst)), Change::Retag);
    assert_eq!(classify(Some(a), None), Change::Rebuild);
}

#[test]
fn residency_keeps_and_frees_per_fd2() {
    let mut r = Residency::new(1, 2);
    let u = r.update(Some(viewer(32, 30))).unwrap_or_default();
    assert_eq!(u.listed.len(), 9);
    assert_eq!(u.attached.len(), 9);
    assert!(r.update(Some(viewer(32, 30))).is_none());
    // One landblock east, into sector 11: the west column leaves the list but stays cached.
    let u = r.update(Some(viewer(33, 30))).unwrap_or_default();
    assert_eq!(u.change, Some(Change::Rebuild));
    assert_eq!(u.detached.len(), 3);
    assert_eq!(u.attached.len(), 3);
    assert!(u.freed.is_empty());
    for k in &u.detached {
        assert!(r.is_cached(*k));
        assert!(r.unlisted_since(*k).is_some());
    }
    // Back west: re-attached from the cache.
    let u = r.update(Some(viewer(32, 30))).unwrap_or_default();
    assert_eq!(u.attached.len(), 3);
    // Walking far east one landblock at a time frees sectors beyond F + 1.
    let mut freed = 0;
    for x in 33..=45u8 {
        freed += r
            .update(Some(viewer(x, 30)))
            .unwrap_or_default()
            .freed
            .len();
    }
    assert!(freed > 0);
    // A jump discards everything.
    let u = r.update(Some(viewer(100, 100))).unwrap_or_default();
    assert!(u.discarded);
    assert_eq!(u.listed.len(), 9);
}

#[test]
fn state_drives_loads_builds_and_sprites() {
    let mut settings = FarViewSettings::from_level(0); // F = 1.
    settings.imposters = true;
    let mut st = FarViewState::new(settings, None);
    let v = viewer(100, 50);
    let w = st.update(Some(v), NearField::Radius(1), SpriteView::default());
    assert_eq!(w.load_sectors.len(), 9);
    for k in &w.load_sectors {
        let mut rec = record(*k, 0);
        rec.sprites = vec![sprite(600, 10.0, 10.0, 140.0, 3)];
        rec.water = vec![SectorWaterPair {
            water_type: 3,
            height: 12.5,
        }];
        rec.terrain.chunks[0].wet = true;
        let mut e = SectorEntity {
            entity_type: 7,
            object: Did(0x4700_0001),
            iid: tessera_formats::landblock::InstanceId::new(
                1,
                1,
                k.corner().0,
                k.corner().1,
                u32::from(k.sx()),
            ),
            scale: [1.0; 3],
            position: Position::new(0)
                .with_region(1)
                .with_landblock(k.corner().0, k.corner().1)
                .with_cell(0)
                .with_offset([10.0, 20.0, 30.0]),
            property: None,
            view_distance_sq: 1_000.0 * 1_000.0,
        };
        e.scale = [2.0; 3];
        rec.entities = vec![e];
        st.sector_loaded(*k, Ok(Some(rec)));
    }
    assert_eq!(st.distant().len(), 9);
    let w = st.update(Some(v), NearField::Radius(1), SpriteView::default());
    assert_eq!(w.build.len(), 9);
    assert_eq!(w.load_tables, vec![Did(0x2701_0000)]);
    for j in &w.build {
        let m = j.run();
        assert!(st.built(j.key, j.generation, m));
    }
    st.table_loaded(Did(0x2701_0000), Ok(Some(table())));
    let w = st.update(Some(v), NearField::Radius(1), SpriteView::default());
    assert!(w.build.is_empty(), "descriptors unchanged");
    assert_eq!(w.sprites.len(), 9);
    for j in &w.sprites {
        assert!(st.sprites_placed(j.key, j.epoch, j.run()));
    }
    assert!(st.settled());
    // The near field holds exactly the 3 × 3 landblocks: far batches cover 72 landblocks.
    let far: u32 = st
        .slots()
        .filter_map(|(_, s)| s.meshes.as_ref())
        .map(|m| m.terrain.landblocks)
        .sum();
    assert_eq!(far, 81 - 9);
    assert!(st.ring_floor().is_some());
    // A move by one landblock rebuilds descriptors (LODs shift) and sprites.
    let w = st.update(
        Some(viewer(101, 50)),
        NearField::Radius(1),
        SpriteView::default(),
    );
    assert!(!w.build.is_empty());
    assert!(!w.sprites.is_empty());
    // An old generation is dropped.
    let stale = w.build[0].clone();
    let w2 = st.update(
        Some(viewer(100, 50)),
        NearField::Radius(1),
        SpriteView::default(),
    );
    if w2.build.iter().any(|j| j.key == stale.key) {
        assert!(!st.built(stale.key, stale.generation, stale.run()));
    }
    // Water of a record.
    let rec = record(SectorKey::new(1, 0, 0).unwrap_or_else(|| unreachable!()), 0);
    assert!(sector_water(&rec).is_empty());
}

// ---------------------------------------------------------------------------------------
// Distant set (FV-I12)
// ---------------------------------------------------------------------------------------

fn entry(iid: u64, sector: SectorKey, x: f64, vd: f32) -> DistantEntry {
    DistantEntry {
        iid,
        entity_type: 7,
        object: Did(0x4700_0001),
        scale: 1.0,
        position: WorldPos::new(1, x, 0.0, 0.0),
        orientation: None,
        view_distance_sq: vd * vd,
        sector,
        content_layers: None,
    }
}

#[test]
fn distant_set_fv_i12() {
    let s1 = SectorKey::new(1, 0, 0).unwrap_or_else(|| unreachable!());
    let s2 = SectorKey::new(1, 1, 0).unwrap_or_else(|| unreachable!());
    let mut set = DistantSet::default();
    set.add_sector([entry(1, s1, 100.0, 500.0), entry(2, s2, 2_000.0, 500.0)]);
    // An entry already present is kept.
    set.add_sector([entry(1, s2, 9_999.0, 1.0)]);
    let at = WorldPos::new(1, 0.0, 0.0, 0.0);
    let c = set.walk(&at, 0, 1.0, |_| false, |_| true);
    assert_eq!(c.create.iter().map(|e| e.iid).collect::<Vec<_>>(), vec![1]);
    assert!(c.destroy.is_empty());
    // Existing through another set: not created.
    let mut other = DistantSet::default();
    other.add_sector([entry(5, s1, 0.0, 10.0)]);
    let c = other.walk(&at, 0, 1.0, |i| i == 5, |_| true);
    assert!(c.create.is_empty());
    // Moving away destroys it.
    let far = WorldPos::new(1, 1_000.0, 0.0, 0.0);
    let c = set.walk(&far, 0, 1.0, |_| false, |_| true);
    assert_eq!(c.destroy, vec![1]);
    // Sector removal destroys its live entities at the next walk.
    let c = set.walk(
        &WorldPos::new(1, 2_000.0, 0.0, 0.0),
        0,
        1.0,
        |_| false,
        |_| true,
    );
    assert_eq!(c.create.len(), 1);
    set.remove_sector(s2);
    let c = set.walk(
        &WorldPos::new(1, 2_000.0, 0.0, 0.0),
        0,
        1.0,
        |_| false,
        |_| true,
    );
    assert_eq!(c.destroy, vec![2]);
    // An instance change removes every live distant entity first.
    let c = set.walk(&at, 0, 1.0, |_| false, |_| true);
    assert_eq!(c.create.len(), 1);
    let c = set.walk(&at, 7, 1.0, |_| false, |_| true);
    assert_eq!(c.destroy, vec![1]);
    assert_eq!(c.create.len(), 1);
    // The content-layer test of FD7.
    let c = set.walk(&at, 7, 1.0, |_| false, |_| false);
    assert_eq!(c.destroy, vec![1]);
    // Invariant: every live entity is within its view distance.
    let _ = set.walk(&at, 7, 1.0, |_| false, |_| true);
    for e in set.live() {
        assert!(e.distance_sq(&at) < f64::from(e.view_distance_sq));
    }
}
