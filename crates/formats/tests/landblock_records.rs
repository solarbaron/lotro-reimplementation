// SPDX-License-Identifier: GPL-3.0-or-later
//! Landblock-family decoders on synthetic payloads (`docs/specs/landblock.md`,
//! `entity-descriptor.md`, `coordinates.md`). Every fixture is built here; none comes from
//! game data.

mod common;

use std::collections::HashMap;
use std::f32::consts::FRAC_1_SQRT_2;

use common::B;
use tessera_formats::landblock::{
    BlockMapEntry, BlockMapProperty, COARSE_SAMPLES, Ceiling, Cell, CollisionTree,
    DistantLandscapeSector, FailureCategory, GRID_SAMPLES, GRID_TRIANGLE_COUNT, GROUP_SAMPLE_COUNT,
    GridSurface, HeightGrid, InstanceId, LandblockBox, LandblockPids, LayerKind, LayoutRule,
    LightMaps, MAX_PLACEMENT_COORD, MAX_STATIC_ENTITIES, PaletteValue, Placement, QUAD_COUNT,
    REGION_DESCRIPTOR_LEN, REGION_DID_MAP_CAPACITY, RegionDidMap, RegionDidMapEntry, RegionTable,
    SectorWaterPair, StaticEntity, StaticEntityType, SurfaceChunk, SurfaceLayer, VertexFlags,
    ceiling_quad_triangles, grid_triangle_key, grid_triangle_quad, height_from_raw, landblock_did,
    light_map_uv, quad_triangles, region_position, vertex_to_height_index, vertex_xy,
};
use tessera_formats::setup::Quaternion;
use tessera_formats::{
    BlockMap, BlockMapDescriptor, Did, DidKind, Evidence, FormatError, LandblockData,
    LandblockGroupRecord, LandblockInfo, LandblockKind, LandblockProperties, Position, PropertySet,
    PropertyType, PropertyValue, Record, RegionDescriptor, decode_landblock_data,
    decode_landblock_info,
};

const DATA: u32 = 0x8001_6478; // region 1, X 0x64 = 100, Y 0x78 = 120
const INFO: u32 = DATA + 0x0020_0000;
const MAP: u32 = DATA + 0x0010_0000;
const PROPS: u32 = DATA + 0x0040_0000;

// ---------------------------------------------------------------- landblock data

fn ramp(i: usize, j: usize) -> u16 {
    (i * 100 + j * 10) as u16
}

fn layer(material: u32, mask: u32, start: u16, prims: u16, min: u16, span: u16) -> SurfaceLayer {
    SurfaceLayer {
        material,
        mask: Did(mask),
        whole: false,
        start,
        prims,
        min_vertex: min,
        vertex_span: span,
    }
}

/// Y-major vertex number of grid sample (x index, y index).
fn v(x: usize, y: usize) -> u16 {
    (33 * y + x) as u16
}

/// A chunk whose single layer covers quad (0, 0) with the triangles the flag predicts.
fn chunk_for_quad0(split_sw_ne: bool) -> SurfaceChunk {
    let tris = quad_triangles(0, 0, split_sw_ne);
    let indices: Vec<u16> = tris.iter().flatten().map(|&(x, y)| v(x, y)).collect();
    let min = *indices.iter().min().unwrap();
    let max = *indices.iter().max().unwrap();
    SurfaceChunk {
        layers: vec![
            layer(5, 0x4100_0001, 0, 2, min, max - min + 1),
            layer(6, 0x4100_0002, 6, 0, 0, 0),
            SurfaceLayer {
                whole: true,
                ..layer(7, 0, 0, 0, 0, 0)
            },
        ],
        indices,
        quads: Some(Box::new([5; QUAD_COUNT])),
    }
}

fn tree(code: &[u8], build: Option<u8>) -> CollisionTree {
    CollisionTree {
        origin: [-0.05, -0.05, 10.0],
        scale: 1234.5,
        code: code.to_vec(),
        build_byte: build,
    }
}

/// A record with every optional part.
fn full_data(build: Option<u8>) -> LandblockData {
    let mut d = LandblockData::from_heights(Did(DATA), HeightGrid::from_fn(ramp));
    d.info = Some(Did(INFO));
    d.vertex_flags.raw_mut()[0] = 0x80 | 127;
    d.coarse[3] = 0x0102;
    d.terrain = chunk_for_quad0(true);
    let mut ceiling_chunk = chunk_for_quad0(true);
    // Ceilings face down: clockwise from above.
    for t in ceiling_chunk.indices.as_chunks_mut::<3>().0 {
        t.swap(1, 2);
    }
    d.ceiling = Some(Ceiling {
        heights: HeightGrid::flat(20_000),
        tree: Some(tree(&[0x28, 1, 2], build)),
        surface: ceiling_chunk,
    });
    d.terrain_tree = Some(tree(&[0x28, 9], build));
    d.boxes = Some(vec![LandblockBox {
        values: [0.0, 40.0, 0.0, 160.0, 45.0, 10.0],
    }]);
    d
}

#[test]
fn minimal_data_round_trips() {
    let d = LandblockData::from_heights(Did(DATA), HeightGrid::flat(10_485));
    let bytes = d.to_bytes();
    assert_eq!(bytes.len(), 4456, "smallest record (§3.1)");
    for stamp in [None, Some(20), Some(35), Some(39)] {
        let back = LandblockData::parse(&bytes, stamp).unwrap();
        // No tree depends on the stamp: `Stamp` when one was given (even an unobserved
        // one), `Fallback` when none was, so the rule tells whether a stamp was supplied.
        let rule = if stamp.is_some() {
            LayoutRule::Stamp
        } else {
            LayoutRule::Fallback
        };
        assert_eq!(back.layout_rule, rule, "stamp {stamp:?}");
        assert_eq!(
            LandblockData {
                layout_rule: d.layout_rule,
                ..back.clone()
            },
            d
        );
        assert_eq!(back.first_unusual_field(), None);
    }
    assert!(d.is_constant_height());
    assert_eq!(d.info, None);
    let lb = d.landblock().unwrap();
    assert_eq!((lb.region, lb.x, lb.y), (1, 100, 120));
    assert!((d.terrain_height(80.0, 80.0).unwrap() - 319.98).abs() < 0.01);
    let (did, info, h) = LandblockData::parse_heights(&bytes).unwrap();
    assert_eq!((did, info), (Did(DATA), None));
    assert_eq!(h, d.heights);
    assert!(LandblockData::parse_expecting(&bytes, Did(DATA + 1), None).is_err());
    assert_eq!(
        <LandblockData as Record>::parse(&bytes).unwrap().did(),
        Did(DATA)
    );
    assert_eq!(decode_landblock_data(&bytes, Some(20)).unwrap(), d);
}

#[test]
fn full_data_round_trips_with_stamp_and_fallback() {
    // Stamp ≥ 38: build bytes present.
    let d = full_data(Some(1));
    let bytes = d.to_bytes();
    let back = LandblockData::parse(&bytes, Some(38)).unwrap();
    assert_eq!(back, d);
    assert_eq!(back.first_unusual_field(), None);
    let fb = LandblockData::parse(&bytes, None).unwrap();
    assert_eq!(fb.layout_rule, LayoutRule::Fallback);
    assert_eq!(
        LandblockData {
            layout_rule: LayoutRule::Stamp,
            ..fb
        },
        d
    );
    // The wrong stamp misframes the record.
    assert!(LandblockData::parse(&bytes, Some(20)).is_err());

    // Stamp ≤ 33: no build bytes.
    let d = full_data(None);
    let bytes = d.to_bytes();
    assert_eq!(LandblockData::parse(&bytes, Some(22)).unwrap(), d);
    let fb = LandblockData::parse(&bytes, None).unwrap();
    assert_eq!(
        fb.ceiling
            .as_ref()
            .unwrap()
            .tree
            .as_ref()
            .unwrap()
            .build_byte,
        None
    );
    assert_eq!(fb.terrain_tree.as_ref().unwrap().build_byte, None);
    assert!(LandblockData::parse(&bytes, Some(39)).is_err());

    // Stamps 34–37 fall back and are reported.
    let u = LandblockData::parse(&bytes, Some(35)).unwrap();
    assert_eq!(u.layout_rule, LayoutRule::UnobservedStamp);
    assert!(u.first_unusual_field().unwrap().contains("unobserved"));
}

#[test]
fn fallback_without_boxes_and_without_ceiling() {
    for build in [None, Some(1)] {
        let mut d = full_data(build);
        d.ceiling = None;
        d.boxes = None;
        let bytes = d.to_bytes();
        let fb = LandblockData::parse(&bytes, None).unwrap();
        assert_eq!(fb.terrain_tree.unwrap().build_byte, build);
    }
}

#[test]
fn data_errors_are_categorised() {
    let good = full_data(Some(1)).to_bytes();
    let cat = |b: &[u8]| FailureCategory::of(&LandblockData::parse(b, Some(38)).unwrap_err());

    // Every truncation fails cleanly.
    for n in 0..good.len() {
        assert_eq!(cat(&good[..n]), FailureCategory::Truncated, "prefix {n}");
    }
    let mut extra = good.clone();
    extra.push(0);
    assert_eq!(cat(&extra), FailureCategory::TrailingBytes);

    let mut bad = good.clone();
    bad[12] = 0x40; // height count 1,088
    assert_eq!(cat(&bad), FailureCategory::BadCount);

    let mut bad = good.clone();
    bad[4..8].copy_from_slice(&(INFO + 1).to_le_bytes());
    assert_eq!(cat(&bad), FailureCategory::DidMismatch);

    let mut bad = good.clone();
    bad[0..4].copy_from_slice(&INFO.to_le_bytes()); // a landblock-info DID
    assert_eq!(cat(&bad), FailureCategory::DidMismatch);

    let mut bad = good.clone();
    bad[16 + 2 * GRID_SAMPLES] = 2; // has_ceiling
    assert_eq!(cat(&bad), FailureCategory::BadFlag);

    // A vertex number ≥ 1,089.
    let mut d = full_data(Some(1));
    d.terrain.indices[0] = 1089;
    assert_eq!(cat(&d.to_bytes()), FailureCategory::IndexOutOfRange);
    // A layer window past the index list.
    let mut d = full_data(Some(1));
    d.terrain.layers[0].prims = 3;
    assert_eq!(cat(&d.to_bytes()), FailureCategory::IndexOutOfRange);
    // A layer `whole` byte of 2.
    let mut d = LandblockData::from_heights(Did(DATA), HeightGrid::flat(0));
    d.terrain = chunk_for_quad0(false);
    let mut bytes = d.to_bytes();
    let whole_at = 16 + 2178 + 1 + 4 + 1089 + 4 + 4 * COARSE_SAMPLES + 4 + 8;
    assert_eq!(bytes[whole_at], 0);
    bytes[whole_at] = 2;
    assert_eq!(cat(&bytes), FailureCategory::BadFlag);
}

#[test]
fn quad_count_must_be_0_or_1024() {
    let d = LandblockData::from_heights(Did(DATA), HeightGrid::flat(0));
    let mut bytes = d.to_bytes();
    // Terrain chunk: u32 0 layers, VLE 0 indices, VLE 0 quads; replace the quad count.
    let quad_at = 16 + 2178 + 1 + 4 + 1089 + 4 + 4 * COARSE_SAMPLES + 4 + 1;
    assert_eq!(bytes[quad_at], 0);
    bytes[quad_at] = 5;
    let e = LandblockData::parse(&bytes, None).unwrap_err();
    assert_eq!(FailureCategory::of(&e), FailureCategory::BadCount);
}

#[test]
fn unusual_terrain_triangles_are_reported() {
    let mut d = full_data(Some(1));
    assert_eq!(d.first_unusual_field(), None);
    // Flip the diagonal flag: the stored triangles no longer use it.
    d.vertex_flags.raw_mut()[0] = 127;
    assert!(d.first_unusual_field().unwrap().contains("HM6"));
    // Clockwise terrain triangle.
    let mut d = full_data(Some(1));
    d.terrain.indices.swap(1, 2);
    assert!(d.first_unusual_field().is_some());
    // Quad grid without layers.
    let mut d = full_data(Some(1));
    d.terrain.layers.clear();
    d.terrain.indices.clear();
    assert!(d.first_unusual_field().unwrap().contains("HM5"));
}

/// landblock.md §3.7 / scenery.md SC6: coarse-grid bits 15–31 (encounter slot and above)
/// are always 0; bits 0–14 (terrain-type id, scene slot) are not unusual.
#[test]
fn coarse_grid_high_bits_are_reported() {
    let mut d = full_data(Some(1));
    d.coarse[0] = 0x0000_7FFF;
    assert_eq!(d.first_unusual_field(), None);
    for v in [0x0000_8000u32, 0x0001_0000, 0x8000_0000] {
        d.coarse[0] = v;
        assert!(
            d.first_unusual_field().unwrap().contains("coarse-grid"),
            "{v:#x}"
        );
    }
    // Word (i, j) is at 17·i + j, i along X.
    let mut d = full_data(Some(1));
    d.coarse[17 * 2 + 5] = 0x0203;
    assert_eq!(d.coarse_word(2, 5), Some(0x0203));
    assert_eq!(d.coarse_word(5, 2), Some(0));
    assert_eq!(d.coarse_word(17, 0), None);
    assert_eq!(d.coarse_word(0, 17), None);
}

#[test]
fn layer_kinds_and_windows() {
    let c = chunk_for_quad0(true);
    assert_eq!(c.layers[0].kind(), LayerKind::Masked);
    assert_eq!(c.layers[1].kind(), LayerKind::Empty);
    assert_eq!(c.layers[2].kind(), LayerKind::Whole);
    assert_eq!(layer(1, 0, 0, 1, 0, 1).kind(), LayerKind::Opaque);
    assert_eq!(c.layer_indices(&c.layers[0]).unwrap().len(), 6);
    assert_eq!(c.quad_material(31, 31), Some(5));
    assert_eq!(c.quad_material(32, 0), None);
}

#[test]
fn grid_orders() {
    // Heights are X-major, layer vertices Y-major (§3.3, §3.8.3).
    assert_eq!(vertex_to_height_index(v(1, 0)), Some(33));
    assert_eq!(vertex_to_height_index(v(0, 1)), Some(1));
    assert_eq!(vertex_to_height_index(1089), None);
    assert_eq!(vertex_xy(v(5, 7)), Some((5, 7)));
    let g = HeightGrid::from_fn(ramp);
    assert_eq!(g.get(2, 3), Some(230));
    assert_eq!(g.raw()[33 * 2 + 3], 230);
    assert_eq!(g.get(33, 0), None);
    assert_eq!(g.min_max(), (0, 3520));
    assert!(!g.is_constant());
    // landblock.md §3.4: 13,107 × 2,000 / 65,535 = 400 (the superseded / 32.768 rule
    // gave the 399.99 of coordinates.md §13).
    assert!((height_from_raw(13_107) - 400.0).abs() < 1e-4);
}

/// landblock.md §3.4: height = (f32(stored) × 2,000.0) × f32(1 / 65,535), each product
/// rounded to f32.
#[test]
fn height_rule_matches_the_client() {
    use tessera_formats::landblock::{HEIGHT_INV_MAX_RAW, HEIGHT_RANGE};
    assert_eq!(HEIGHT_INV_MAX_RAW, 1.525_902_2e-5_f32);
    assert_eq!(HEIGHT_RANGE, 2000.0);
    assert_eq!(height_from_raw(0), 0.0);
    assert_eq!(height_from_raw(65_535), 2000.0);
    assert!((height_from_raw(32_768) - 1000.0153).abs() < 1e-4);
    // Differs from the superseded rule (stored / 32.768).
    assert!((height_from_raw(3000) - 91.554_13).abs() < 1e-5);
    assert_ne!(height_from_raw(3000), 3000.0 / 32.768);
    // Within 6.2e-5 units of the exact quotient for every stored value, and monotonic.
    let mut last = -1.0f32;
    for raw in 0..=u16::MAX {
        let h = height_from_raw(raw);
        let exact = f64::from(raw) * 2000.0 / 65_535.0;
        assert!((f64::from(h) - exact).abs() <= 6.2e-5, "{raw}: {h}");
        assert!(h >= last, "{raw}");
        last = h;
    }
}

#[test]
fn quad_triangles_are_counter_clockwise() {
    for split in [false, true] {
        for t in quad_triangles(3, 4, split) {
            let c = |k: usize| (t[k].0 as i64, t[k].1 as i64);
            let ((ax, ay), (bx, by), (cx, cy)) = (c(0), c(1), c(2));
            assert!((bx - ax) * (cy - ay) - (by - ay) * (cx - ax) > 0, "{t:?}");
        }
    }
}

#[test]
fn terrain_height_follows_the_split_diagonal() {
    // Only corner (1, 1) of quad (0, 0) is raised: the diagonal decides the centre.
    let mut g = HeightGrid::flat(0);
    g.raw_mut()[33 + 1] = 32_768; // 1,000 units at (i 1, j 1)
    let mut d = LandblockData::from_heights(Did(DATA), g);
    // SE-NW split: the centre lies on the diagonal (1,0)-(0,1): height 0.
    assert!(d.terrain_height(2.5, 2.5).unwrap().abs() < 1e-3);
    // Upper triangle (fx + fy > 1) uses (1,1): x = y = 3.75 → 0.5 of 1,000.
    assert!((d.terrain_height(3.75, 3.75).unwrap() - 500.0).abs() < 0.01);
    // SW-NE split: the centre lies on the diagonal (0,0)-(1,1): half height.
    d.vertex_flags.raw_mut()[0] = 0x80;
    assert!((d.terrain_height(2.5, 2.5).unwrap() - 500.0).abs() < 0.01);
    assert!((d.terrain_height(5.0, 0.0).unwrap()).abs() < 1e-3);
    // Corners reproduce samples; clamping at the block edge.
    let d = LandblockData::from_heights(Did(DATA), HeightGrid::from_fn(ramp));
    for (i, j) in [(0, 0), (32, 32), (7, 31), (32, 0)] {
        let h = d.terrain_height(5.0 * i as f32, 5.0 * j as f32).unwrap();
        assert!((h - height_from_raw(ramp(i, j))).abs() < 1e-3, "({i}, {j})");
    }
    assert_eq!(d.terrain_height(-10.0, 400.0), d.terrain_height(0.0, 160.0));
    assert_eq!(d.terrain_height(f32::NAN, 1.0), None);
    assert!(d.quad_triangles(31, 31).is_some());
    assert!(d.quad_triangles(32, 0).is_none());
    let f = VertexFlags::filled(0x80);
    assert!(f.split_sw_ne(0, 0) && !f.split_sw_ne(32, 0));
}

/// landblock.md §3.11 evaluated with non-negative weights: a query at a grid sample returns
/// that sample exactly and never leaves the range of the triangle's corners (0–2,000), so
/// neighbouring blocks agree at shared corners.
#[test]
fn terrain_height_stays_within_the_corner_samples() {
    // Corners (31,31), (32,31), (31,32), (32,32) of quad (31, 31), SW-NE split.
    for corners in [
        [65_534u16, 23_739, 47_077, 0],
        [18_181, 61_649, 65_535, 65_535],
    ] {
        let mut g = HeightGrid::flat(0);
        let raw = g.raw_mut();
        raw[33 * 31 + 31] = corners[0];
        raw[33 * 32 + 31] = corners[1];
        raw[33 * 31 + 32] = corners[2];
        raw[33 * 32 + 32] = corners[3];
        let mut d = LandblockData::from_heights(Did(DATA), g);
        d.vertex_flags.raw_mut()[33 * 31 + 31] = 0x80;
        let h = |x: f32, y: f32| d.terrain_height(x, y).unwrap();
        // The four corners are the stored samples, exactly.
        assert_eq!(h(160.0, 160.0), height_from_raw(corners[3]), "{corners:?}");
        assert_eq!(h(155.0, 155.0), height_from_raw(corners[0]));
        assert_eq!(h(160.0, 155.0), height_from_raw(corners[1]));
        assert_eq!(h(155.0, 160.0), height_from_raw(corners[2]));
        let lo = corners
            .iter()
            .map(|&c| height_from_raw(c))
            .fold(f32::MAX, f32::min);
        let hi = corners
            .iter()
            .map(|&c| height_from_raw(c))
            .fold(f32::MIN, f32::max);
        for (x, y) in [
            (160.0, 157.3),
            (157.3, 160.0),
            (159.999, 159.998),
            (155.0001, 159.9999),
            (157.5, 157.5),
            (160.0, 155.0001),
        ] {
            let v = h(x, y);
            assert!(
                (lo..=hi).contains(&v) && (0.0..=2000.0).contains(&v),
                "({x}, {y}): {v}"
            );
        }
        // The same corners on the SE-NW split.
        d.vertex_flags.raw_mut()[33 * 31 + 31] = 0;
        assert_eq!(
            d.terrain_height(160.0, 160.0),
            Some(height_from_raw(corners[3]))
        );
        assert_eq!(
            d.terrain_height(160.0, 155.0),
            Some(height_from_raw(corners[1]))
        );
        assert_eq!(
            d.terrain_height(155.0, 160.0),
            Some(height_from_raw(corners[2]))
        );
    }
    // Two neighbouring blocks agree along their shared edge (x = 160 here, x = 0 there).
    let mut west = LandblockData::from_heights(Did(DATA), HeightGrid::from_fn(ramp));
    let mut east = LandblockData::from_heights(
        Did(DATA),
        HeightGrid::from_fn(|i, j| ramp(32, j) + 7 * i as u16),
    );
    for split in [0x80u8, 0] {
        for j in 0..32 {
            west.vertex_flags.raw_mut()[33 * 31 + j] = split;
            east.vertex_flags.raw_mut()[j] = split ^ 0x80;
        }
        for y in [0.0, 3.3, 77.7, 155.0, 160.0] {
            assert_eq!(
                west.terrain_height(160.0, y),
                east.terrain_height(0.0, y),
                "{y}"
            );
        }
    }
}

/// Checks both seams of a global grid (`global(gi, gj)` gives the raw height sample) for
/// every pairing of the two diagonals at the given offsets along the edge.
fn assert_seams_agree(global: &dyn Fn(usize, usize) -> u16, samples: &[f32], label: &str) {
    let block = |oi: usize, oj: usize| {
        LandblockData::from_heights(
            Did(DATA),
            HeightGrid::from_fn(|i, j| global(i + oi, j + oj)),
        )
    };
    for (near, far) in [(0x80u8, 0x80u8), (0x80, 0), (0, 0x80), (0, 0)] {
        // West/east: x = 160 in the west block is x = 0 in the east block.
        let (mut west, mut east) = (block(0, 0), block(32, 0));
        // South/north: y = 160 in the south block is y = 0 in the north block.
        let (mut south, mut north) = (block(0, 0), block(0, 32));
        for k in 0..32 {
            west.vertex_flags.raw_mut()[33 * 31 + k] = near;
            east.vertex_flags.raw_mut()[k] = far;
            south.vertex_flags.raw_mut()[33 * k + 31] = near;
            north.vertex_flags.raw_mut()[33 * k] = far;
        }
        for &t in samples {
            assert_eq!(
                west.terrain_height(160.0, t).map(f32::to_bits),
                east.terrain_height(0.0, t).map(f32::to_bits),
                "{label}: x seam, splits {near:#x}/{far:#x}, y = {t:e}"
            );
            assert_eq!(
                south.terrain_height(t, 160.0).map(f32::to_bits),
                north.terrain_height(t, 0.0).map(f32::to_bits),
                "{label}: y seam, splits {near:#x}/{far:#x}, x = {t:e}"
            );
        }
    }
}

/// landblock.md §3.11 and §3.3 (HM7): neighbouring blocks share their edge samples, so the
/// surface heights along the shared edge must agree bit for bit, for every pairing of the
/// two diagonals, including fractions so small that `1 + f` rounds (the first row or
/// column of quads, below 5 units).
#[test]
fn neighbouring_blocks_agree_bit_for_bit_along_the_shared_edge() {
    // A steep, irregular global grid; each block is a 33 x 33 window onto it.
    let hashed = |gi: usize, gj: usize| -> u16 {
        let k = (gi * 7_919 + gj * 104_729) as u32;
        (k.wrapping_mul(2_654_435_761) >> 16) as u16
    };
    let mut samples: Vec<f32> = (1..500).map(|k| k as f32 * 0.01).collect();
    samples.extend((0..2_000).map(|k| k as f32 * 0.0025));
    samples.extend([
        0.0,
        f32::MIN_POSITIVE,
        1.0e-30,
        1.0e-8,
        2f32.powi(-24),
        2f32.powi(-25),
        2f32.powi(-23) * 3.0,
        4.999_999_5,
        5.0,
        77.7,
        155.0,
        159.999_99,
        160.0,
    ]);
    assert_seams_agree(&hashed, &samples, "hashed");
}

/// The same seam property where the shared edge is flat (equal adjacent edge samples, as
/// is common in real terrain) and the interior samples on either side straddle the edge
/// height, one block's above it and the other's below. An edge sum rounded 1 ulp past the
/// edge height must be clamped back identically in both blocks.
#[test]
fn neighbouring_blocks_agree_bit_for_bit_along_a_flat_edge_between_straddling_interiors() {
    // Global index 32 is the shared edge; 31 lies in the first block, 33 in the second.
    let straddle = |a: u16, b: u16| {
        move |g: usize| -> u16 {
            match g {
                31 => a,
                33 => b,
                _ if g < 32 => a / 2,
                _ if g > 32 => b / 2,
                _ => 30_000,
            }
        }
    };
    let samples: Vec<f32> = (1..=20_000).map(|k| k as f32 * 0.008).collect();
    for (a, b) in [(60_000u16, 1_000u16), (1_000, 60_000)] {
        let col = straddle(a, b);
        // Columns vary across the x seam, constant along it.
        let columns = |gi: usize, _gj: usize| col(gi);
        assert_seams_agree(&columns, &samples, &format!("columns {a}/30000/{b}"));
        // Rows vary across the y seam, constant along it.
        let rows = |_gi: usize, gj: usize| col(gj);
        assert_seams_agree(&rows, &samples, &format!("rows {a}/30000/{b}"));
    }
}

/// landblock.md §3.8.2: a `whole` layer covers all 2,048 grid triangles of its surface
/// (bit 7 and counter-clockwise in a terrain chunk, bit 0 and clockwise in a ceiling chunk).
#[test]
fn whole_layers_cover_every_grid_triangle() {
    let mut d = full_data(Some(1));
    // Make the two diagonal bits differ on quad (0, 0).
    d.vertex_flags.raw_mut()[0] = 0x80;
    let whole = d.terrain.layers[2];
    assert_eq!(whole.kind(), LayerKind::Whole);
    for surface in [GridSurface::Terrain, GridSurface::Ceiling] {
        let tris = d.layer_triangles(surface, &whole).unwrap();
        assert_eq!(tris.len(), GRID_TRIANGLE_COUNT);
        for (key, t) in tris.iter().enumerate() {
            let want = d.grid_triangle(surface, key as u16).unwrap();
            let got = t.map(|v| vertex_xy(v).unwrap());
            assert_eq!(got, want, "{surface:?} key {key}");
        }
        // Quad (0, 0): bit 7 set (SW-NE) for the terrain, bit 0 clear (SE-NW) for the
        // ceiling.
        let first: Vec<[u16; 3]> = tris[..2].to_vec();
        let expect = match surface {
            GridSurface::Terrain => quad_triangles(0, 0, true),
            GridSurface::Ceiling => ceiling_quad_triangles(0, 0, false),
        };
        for t in expect {
            assert!(
                first.contains(&t.map(|(x, y)| v(x, y))),
                "{surface:?} {t:?}"
            );
        }
    }
    // Stored windows come back as triangles; empty layers have none.
    let c = &d.terrain;
    let masked = d
        .layer_triangles(GridSurface::Terrain, &c.layers[0])
        .unwrap();
    assert_eq!(masked.concat(), c.layer_indices(&c.layers[0]).unwrap());
    assert!(
        d.layer_triangles(GridSurface::Terrain, &c.layers[1])
            .unwrap()
            .is_empty()
    );
    assert!(d.surface_chunk(GridSurface::Ceiling).is_some());
    let bare = LandblockData::from_heights(Did(DATA), HeightGrid::flat(0));
    assert_eq!(bare.layer_triangles(GridSurface::Ceiling, &whole), None);
    assert!(bare.surface_chunk(GridSurface::Ceiling).is_none());
}

/// landblock.md §3.5–§3.6: ceiling quads split by bit 0, triangles clockwise from above.
#[test]
fn ceiling_quads_use_bit_0_and_face_down() {
    for split in [false, true] {
        let up = quad_triangles(3, 4, split);
        let down = ceiling_quad_triangles(3, 4, split);
        for (u, t) in up.iter().zip(&down) {
            assert_eq!([u[2], u[1], u[0]], *t);
            let c = |k: usize| (t[k].0 as i64, t[k].1 as i64);
            let ((ax, ay), (bx, by), (cx, cy)) = (c(0), c(1), c(2));
            assert!((bx - ax) * (cy - ay) - (by - ay) * (cx - ax) < 0, "{t:?}");
        }
    }
    // Bit 0 and bit 7 are independent.
    let mut f = VertexFlags::filled(0);
    f.raw_mut()[33 * 2 + 5] = 0x01;
    f.raw_mut()[33 * 4 + 1] = 0x80;
    assert!(f.ceiling_split_sw_ne(2, 5) && !f.split_sw_ne(2, 5));
    assert!(f.split_sw_ne(4, 1) && !f.ceiling_split_sw_ne(4, 1));
    assert!(f.surface_split_sw_ne(GridSurface::Ceiling, 2, 5));
    assert!(!f.ceiling_split_sw_ne(32, 0) && !f.ceiling_split_sw_ne(0, 32));
    assert_eq!(GridSurface::Terrain.diagonal_bit(), 0x80);
    assert_eq!(GridSurface::Ceiling.diagonal_bit(), 0x01);

    let mut d = full_data(Some(1));
    assert_eq!(d.first_unusual_field(), None);
    // Byte 0 = 0x80 | 127: bit 0 set, so the ceiling quad (0, 0) is split SW-NE.
    assert_eq!(
        d.ceiling_quad_triangles(0, 0),
        Some(ceiling_quad_triangles(0, 0, true))
    );
    // Bit 7 alone does not split the ceiling: the stored ceiling triangles now break HM6.
    d.vertex_flags.raw_mut()[0] = 0x80;
    assert!(
        d.first_unusual_field()
            .unwrap()
            .contains("ceiling triangle does not use the bit-0 diagonal")
    );
    // Counter-clockwise ceiling triangles are unusual.
    let mut d = full_data(Some(1));
    d.ceiling.as_mut().unwrap().surface.indices.swap(1, 2);
    assert!(d.first_unusual_field().unwrap().contains("clockwise"));
    // No ceiling: ceiling queries answer None.
    let d = LandblockData::from_heights(Did(DATA), HeightGrid::flat(7));
    assert_eq!(d.ceiling_quad_triangles(0, 0), None);
    assert_eq!(d.ceiling_height(1.0, 1.0), None);
    assert_eq!(d.surface_tree(GridSurface::Ceiling), None);
    assert_eq!(d.grid_triangle(GridSurface::Ceiling, 0), None);
}

#[test]
fn ceiling_height_follows_bit_0() {
    // Only ceiling corner (1, 1) of quad (0, 0) is raised.
    let mut g = HeightGrid::flat(0);
    g.raw_mut()[33 + 1] = 32_768;
    let mut d = LandblockData::from_heights(Did(DATA), HeightGrid::flat(0));
    d.ceiling = Some(Ceiling {
        heights: g,
        tree: None,
        surface: SurfaceChunk::default(),
    });
    // Bit 7 set, bit 0 clear: the ceiling is split SE-NW, centre on the diagonal → 0.
    d.vertex_flags.raw_mut()[0] = 0x80;
    assert!(d.ceiling_height(2.5, 2.5).unwrap().abs() < 1e-3);
    assert!((d.ceiling_height(3.75, 3.75).unwrap() - 500.0).abs() < 0.01);
    // Bit 0 set: SW-NE, half height at the centre.
    d.vertex_flags.raw_mut()[0] = 0x01;
    assert!((d.ceiling_height(2.5, 2.5).unwrap() - 500.0).abs() < 0.01);
    assert_eq!(d.ceiling_height(f32::NAN, 0.0), None);
    let c = d.ceiling.as_ref().unwrap();
    assert!(c.has_sample(1, 1) && !c.has_sample(0, 0) && !c.has_sample(33, 0));
}

/// collision.md §6.2: keys 2 · (32 · j + i) (+ 1), the key-base triangle west of the
/// diagonal; every key names one of the quad's two triangles.
#[test]
fn grid_triangle_keys() {
    assert_eq!(GRID_TRIANGLE_COUNT, 2048);
    assert_eq!(grid_triangle_key(0, 0, true), Some(0));
    assert_eq!(grid_triangle_key(0, 0, false), Some(1));
    assert_eq!(grid_triangle_key(1, 0, true), Some(2));
    assert_eq!(grid_triangle_key(0, 1, true), Some(64));
    assert_eq!(grid_triangle_key(31, 31, false), Some(2047));
    assert_eq!(grid_triangle_key(32, 0, true), None);
    assert_eq!(grid_triangle_quad(64), Some((0, 1, true)));
    assert_eq!(grid_triangle_quad(2047), Some((31, 31, false)));
    assert_eq!(grid_triangle_quad(2048), None);
    for key in 0..2048u16 {
        let (i, j, west) = grid_triangle_quad(key).unwrap();
        assert_eq!(grid_triangle_key(i, j, west), Some(key));
    }
    let mut d = LandblockData::from_heights(Did(DATA), HeightGrid::from_fn(ramp));
    // SE-NW quad (2, 3): the west triangle is (i, j), (i + 1, j), (i, j + 1).
    let base = grid_triangle_key(2, 3, true).unwrap();
    assert_eq!(
        d.grid_triangle(GridSurface::Terrain, base),
        Some([(2, 3), (3, 3), (2, 4)])
    );
    // SW-NE quad: the west triangle is (i, j), (i + 1, j + 1), (i, j + 1).
    d.vertex_flags.raw_mut()[33 * 2 + 3] = 0x80;
    assert_eq!(
        d.grid_triangle(GridSurface::Terrain, base),
        Some([(2, 3), (3, 4), (2, 4)])
    );
    assert_eq!(
        d.grid_triangle(GridSurface::Terrain, base + 1),
        Some([(2, 3), (3, 3), (3, 4)])
    );
    let p = d
        .grid_triangle_positions(GridSurface::Terrain, base)
        .unwrap();
    assert_eq!(p[0], [10.0, 15.0, height_from_raw(ramp(2, 3))]);
    assert_eq!(p[1], [15.0, 20.0, height_from_raw(ramp(3, 4))]);
    assert_eq!(d.grid_triangle(GridSurface::Terrain, 2048), None);
    // The two keys of every quad cover the quad: each corner pair (diagonal) is shared.
    for key in (0..2048u16).step_by(2) {
        let a = d.grid_triangle(GridSurface::Terrain, key).unwrap();
        let b = d.grid_triangle(GridSurface::Terrain, key + 1).unwrap();
        let shared = a.iter().filter(|c| b.contains(c)).count();
        assert_eq!(shared, 2, "{key}");
    }
    // Ceiling keys use bit 0 and the clockwise winding.
    d.ceiling = Some(Ceiling {
        heights: HeightGrid::flat(100),
        tree: Some(tree(&[0x28], None)),
        surface: SurfaceChunk::default(),
    });
    d.vertex_flags.raw_mut()[33 * 2 + 3] = 0x01;
    assert_eq!(
        d.grid_triangle(GridSurface::Ceiling, base),
        Some([(2, 4), (3, 4), (2, 3)])
    );
    assert!(d.surface_tree(GridSurface::Ceiling).is_some());
    assert_eq!(d.surface_tree(GridSurface::Terrain), None);
    let t = tree(&[0x28, 1, 2], Some(1));
    assert_eq!(t.stored_len(), 12 + 4 + 4 + 3 + 1);
    assert_eq!(t.to_bytes().len(), t.stored_len());
}

// ---------------------------------------------------------------- landblock info

const SCALE: u32 = 0x1000_0001;
const PHYS_SCALE: u32 = 0x1000_0002;
const INT: u32 = 0x1000_0003;
const GAME_MAP: u32 = 0x1000_0010;
const MAP_IMAGE: u32 = 0x1000_0011;
const BX: u32 = 0x1000_0012;
const BY: u32 = 0x1000_0013;
const PX: u32 = 0x1000_0014;
const PY: u32 = 0x1000_0015;
const MSCALE: u32 = 0x1000_0016;
const IMAGE_ARRAY: u32 = 0x1000_0017;
const IMAGE_STRUCT: u32 = 0x1000_0018;
const CONTENT_LAYER: u32 = 0x1000_0019;
const LIGHT_MAP: u32 = 0x1000_001A;
const FLOAT: u32 = 0x1000_001B;

fn schema() -> HashMap<u32, PropertyType> {
    let mut m = HashMap::new();
    m.insert(SCALE, PropertyType::Vector3);
    m.insert(PHYS_SCALE, PropertyType::Vector3);
    m.insert(INT, PropertyType::Int);
    m.insert(GAME_MAP, PropertyType::Struct);
    m.insert(MAP_IMAGE, PropertyType::DidRef);
    m.insert(BX, PropertyType::Int);
    m.insert(BY, PropertyType::Int);
    m.insert(PX, PropertyType::Int);
    m.insert(PY, PropertyType::Int);
    m.insert(MSCALE, PropertyType::Float);
    m.insert(IMAGE_ARRAY, PropertyType::Array);
    m.insert(IMAGE_STRUCT, PropertyType::Struct);
    m.insert(CONTENT_LAYER, PropertyType::Int);
    m.insert(LIGHT_MAP, PropertyType::DidRef);
    m.insert(FLOAT, PropertyType::Float);
    m
}

fn pids() -> LandblockPids {
    let names: HashMap<&str, u32> = [
        ("Render_AdjustableScale", SCALE),
        ("Physics_AdjustableScale", PHYS_SCALE),
        ("UI_Map_GameMap", GAME_MAP),
        ("UI_Map_MapImage", MAP_IMAGE),
        ("UI_Map_BlockOffsetX", BX),
        ("UI_Map_BlockOffsetY", BY),
        ("UI_Map_PixelOffsetX", PX),
        ("UI_Map_PixelOffsetY", PY),
        ("UI_Map_Scale", MSCALE),
        ("UI_Map_MapImageArray", IMAGE_ARRAY),
        ("UI_Map_ContentLayer", CONTENT_LAYER),
        ("Landblock_LightMap", LIGHT_MAP),
        ("Landblock_UseCeiling", INT),
    ]
    .into_iter()
    .collect();
    LandblockPids::from_names(|n| names.get(n).copied())
}

/// An embedded property set holding a uniform scale `s` under both scale PIDs.
fn scale_set(b: B, s: f32) -> B {
    b.tsize(0, 2)
        .u32(SCALE)
        .u32(SCALE)
        .f32(s)
        .f32(s)
        .f32(s)
        .u32(PHYS_SCALE)
        .u32(PHYS_SCALE)
        .f32(s)
        .f32(s)
        .f32(s)
}

/// A Position value: region, landblock, cell, offset (flags 0x1B).
fn position(b: B, cell: u16) -> B {
    b.u8(0x1B)
        .u8(1)
        .u8(100)
        .u8(120)
        .u16(cell)
        .f32(10.0)
        .f32(-20.0)
        .f32(300.0)
}

/// A physics SED (flags 0x1DE: type, IID, object, placement, word, properties, name).
fn physics_sed(b: B, serial: u32, rotation: [f32; 4]) -> B {
    let b = b
        .u32(0x1DE)
        .u32(7)
        .u64(InstanceId::new(1, 1, 100, 120, serial).0)
        .u32(0x4700_0D7A)
        .f32(80.0)
        .f32(40.0)
        .f32(377.1)
        .f32(rotation[0])
        .f32(rotation[1])
        .f32(rotation[2])
        .f32(rotation[3]);
    scale_set(b, 2.0).u32(1).pascal("house.entity")
}

struct InfoParts {
    entities: Vec<Vec<u8>>,
    trailer: bool,
}

fn info_bytes(p: &InfoParts) -> Vec<u8> {
    let mut b = B::new()
        .u32(INFO)
        .u32(7)
        .u32(0x1234)
        // Cross links.
        .tsize(0, 1)
        .u32(1)
        .u32(INFO)
        .u32(INFO + 1)
        .u32(16)
        .u32(2)
        .u32(0xDEAD_BEEF)
        // Header indices.
        .tsize(0, 1)
        .u16(0x100)
        .u32(2)
        .u16(0x100)
        .u16(0x101)
        // Cells.
        .tsize(0, 2);
    for (index, other) in [(0x100u16, 0x101u16), (0x101, 0x100)] {
        b = position(b.u16(index), index)
            .u32(0x2C) // properties, bit 3, echo/extra
            .u32(0x0100_0ABC)
            .u32(1)
            .u32(0)
            .u16(other)
            .u16(1)
            .u16(0)
            .u16(0)
            .u16(0)
            .u8(1)
            .u16(index) // echo
            .u32(1)
            .u16(other) // list A
            .u32(1)
            .u16(0x7777) // list B
            .u16(3); // extra
        b = scale_set(b, 1.0);
    }
    b = b.u32(0).u32(p.entities.len() as u32);
    for e in &p.entities {
        b = b.bytes(e);
    }
    // Links: one plain, one crossing.
    b = b
        .u32(2)
        .pascal("")
        .u64(1)
        .u64(2)
        .u64(3)
        .u32(16)
        .tsize(0, 0)
        .u8(0)
        .pascal("path")
        .u64(4)
        .u64(5)
        .u64(6)
        .u32(8)
        .tsize(0, 1)
        .u32(INT)
        .u32(INT)
        .u32(9)
        .u8(1)
        .u32(1)
        .u32(INFO)
        .u32(INFO + 0x100)
        .u32(2);
    // Landblock properties, reserved, weenies.
    b = b
        .tsize(0, 0)
        .u32(0)
        .tsize(0, 1)
        .u64(0x0410_0000_0000_0001)
        .tsize(0, 0);
    if p.trailer {
        b = b.u32(42);
    }
    b.build()
}

fn default_info(trailer: bool) -> Vec<u8> {
    info_bytes(&InfoParts {
        entities: vec![
            physics_sed(B::new(), 1, [FRAC_1_SQRT_2, 0.0, 0.0, FRAC_1_SQRT_2]).build(),
            // Reported-only bits 0x001, 0x020, 0x200 plus interior, no name or placement.
            scale_set(
                B::new()
                    .u32(0x6A7)
                    .u32(0x4700_0001)
                    .u32(3)
                    .u64(InstanceId::new(4, 1, 100, 120, 2).0)
                    .f32(1.0)
                    .f32(2.0)
                    .f32(3.0),
                1.0,
            )
            .vle(2)
            .u32(0x4100_0001)
            .u32(0x4100_0002)
            .build(),
        ],
        trailer,
    })
}

#[test]
fn info_decodes_every_part() {
    let bytes = default_info(true);
    let info = LandblockInfo::parse(&bytes, Some(39), &schema()).unwrap();
    assert_eq!(info.did, Did(INFO));
    assert_eq!(info.size_hint, 0x1234);
    assert_eq!(
        info.cross_links.as_ref().unwrap().items[0].mask,
        0xDEAD_BEEF
    );
    assert_eq!(
        info.header_indices.as_ref().unwrap().items[0].values,
        [0x100, 0x101]
    );
    assert_eq!(info.cells().len(), 2);
    let c = info.cell(0x101).unwrap();
    assert_eq!(c.neighbours[0].cell, 0x100);
    assert_eq!((c.index_echo, c.extra), (Some(0x101), Some(3)));
    assert_eq!(c.position.landblock(), Some((100, 120)));
    assert_eq!(c.list_b, [0x7777]);
    assert_eq!(info.entities.len(), 2);
    assert_eq!(info.links.len(), 2);
    assert_eq!(info.links[1].name, "path");
    assert_eq!(info.links[1].cross.unwrap().to_info, Did(INFO + 0x100));
    assert_eq!(info.weenies.len(), 1);
    assert_eq!(info.trailer, Some(42));
    assert_eq!(info.layout_rule, LayoutRule::Stamp);

    let e = &info.entities[0];
    assert_eq!(e.kind(), Some(StaticEntityType::Physics));
    assert_eq!(e.object, Some(Did(0x4700_0D7A)));
    assert_eq!(e.word, Some(1));
    assert_eq!(e.name.as_deref(), Some("house.entity"));
    assert_eq!(e.scale(SCALE), [2.0; 3]);
    assert_eq!(e.scale(0xFFFF), [1.0; 3]);
    assert!(!e.is_interior());
    let iid = e.iid.unwrap();
    assert_eq!(
        (iid.kind(), iid.region(), iid.landblock(), iid.serial()),
        (1, 1, (100, 120), 1)
    );
    let p = e.placement.unwrap();
    assert!(!p.renormalised && p.is_inside_block());
    // Yaw 90°: model +Y (forward) points west; scale 2 doubles the offset (§5).
    let w = p.transform([0.0, 1.0, 0.0], e.scale(SCALE));
    assert!(
        (w[0] - 78.0).abs() < 1e-4 && (w[1] - 40.0).abs() < 1e-4,
        "{w:?}"
    );
    assert!((p.rotation.heading_degrees() - 270.0).abs() < 1e-3);

    let e = &info.entities[1];
    assert_eq!(e.kind(), Some(StaticEntityType::GeneratorPoint));
    assert!(e.is_interior());
    assert_eq!(e.did, Some(Did(0x4700_0001)));
    assert_eq!(e.vector, Some([1.0, 2.0, 3.0]));
    assert_eq!(
        e.dids.as_deref(),
        Some(&[Did(0x4100_0001), Did(0x4100_0002)][..])
    );
    assert_eq!(e.placement, None);
    assert!(e.first_unusual_field().unwrap().contains("reported-only"));
    assert!(
        info.first_unusual_field()
            .unwrap()
            .contains("reported-only")
    );
    assert_eq!(
        decode_landblock_info(&bytes, None, &schema())
            .unwrap()
            .trailer,
        Some(42)
    );
}

#[test]
fn sed_encoder_round_trips() {
    let bytes = physics_sed(B::new(), 7, [1.0, 0.0, 0.0, 0.0]).build();
    let info = LandblockInfo::parse(
        &info_bytes(&InfoParts {
            entities: vec![bytes.clone()],
            trailer: false,
        }),
        Some(20),
        &schema(),
    )
    .unwrap();
    let e = &info.entities[0];
    let props = scale_set(B::new(), 2.0).build();
    assert_eq!(e.to_bytes_with(&props).unwrap(), bytes);
    assert_eq!(info.first_unusual_field(), None);
}

#[test]
fn info_trailer_follows_the_stamp() {
    let s = schema();
    let with = default_info(true);
    let without = default_info(false);
    assert_eq!(
        LandblockInfo::parse(&with, Some(31), &s).unwrap().trailer,
        Some(42)
    );
    assert_eq!(
        LandblockInfo::parse(&without, Some(29), &s)
            .unwrap()
            .trailer,
        None
    );
    assert!(matches!(
        LandblockInfo::parse(&with, Some(20), &s),
        Err(FormatError::TrailingBytes { .. })
    ));
    assert!(LandblockInfo::parse(&without, Some(38), &s).is_err());
    for (bytes, want) in [(&with, Some(42)), (&without, None)] {
        let f = LandblockInfo::parse(bytes, None, &s).unwrap();
        assert_eq!((f.trailer, f.layout_rule), (want, LayoutRule::Fallback));
        let u = LandblockInfo::parse(bytes, Some(30), &s).unwrap();
        assert_eq!(
            (u.trailer, u.layout_rule),
            (want, LayoutRule::UnobservedStamp)
        );
        assert!(u.first_unusual_field().unwrap().contains("unobserved"));
    }
}

#[test]
fn info_errors_are_categorised() {
    let s = schema();
    let good = default_info(true);
    for n in 0..good.len() {
        let e = LandblockInfo::parse(&good[..n], Some(39), &s).unwrap_err();
        assert!(
            matches!(
                FailureCategory::of(&e),
                FailureCategory::Truncated | FailureCategory::PropertySet
            ),
            "prefix {n}: {e}"
        );
    }
    let cat = |b: &[u8]| FailureCategory::of(&LandblockInfo::parse(b, Some(39), &s).unwrap_err());

    let sed = |flags: u32| {
        info_bytes(&InfoParts {
            entities: vec![B::new().u32(flags).build()],
            trailer: true,
        })
    };
    assert_eq!(cat(&sed(0x800)), FailureCategory::UnsupportedFlags);
    assert_eq!(cat(&sed(0x8000_0000)), FailureCategory::UnsupportedFlags);

    let placed = |pos: f32, rot: [f32; 4]| {
        info_bytes(&InfoParts {
            entities: vec![
                B::new()
                    .u32(0x10)
                    .f32(pos)
                    .f32(0.0)
                    .f32(0.0)
                    .f32(rot[0])
                    .f32(rot[1])
                    .f32(rot[2])
                    .f32(rot[3])
                    .build(),
            ],
            trailer: true,
        })
    };
    assert_eq!(
        cat(&placed(f32::NAN, [1.0, 0.0, 0.0, 0.0])),
        FailureCategory::Malformed
    );
    assert_eq!(
        cat(&placed(0.0, [f32::INFINITY, 0.0, 0.0, 0.0])),
        FailureCategory::Malformed
    );
    assert_eq!(cat(&placed(0.0, [0.0; 4])), FailureCategory::Malformed);
    // A rotation off unit length is renormalised and reported.
    let info = LandblockInfo::parse(&placed(0.0, [2.0, 0.0, 0.0, 0.0]), Some(39), &s).unwrap();
    let p = info.entities[0].placement.unwrap();
    assert!(p.renormalised && p.rotation.is_identity());
    assert!(info.entities[0].first_unusual_field().is_some());

    // Unknown PID in a SED property set.
    let unknown = info_bytes(&InfoParts {
        entities: vec![
            B::new()
                .u32(0x80)
                .tsize(0, 1)
                .u32(0xABCD)
                .u32(0xABCD)
                .build(),
        ],
        trailer: true,
    });
    assert_eq!(cat(&unknown), FailureCategory::PropertySet);

    // SED count over the limit.
    let mut over = B::new()
        .u32(INFO)
        .u32(0)
        .u32(0)
        .u32(0)
        .u32(MAX_STATIC_ENTITIES + 1)
        .build();
    over.resize(over.len() + 4 * (MAX_STATIC_ENTITIES as usize + 1), 0);
    assert_eq!(cat(&over), FailureCategory::Limit);
    // A SED count the remaining bytes cannot hold.
    let short = B::new().u32(INFO).u32(0).u32(0).u32(0).u32(1000).build();
    assert_eq!(cat(&short), FailureCategory::Truncated);

    // Unknown landblock-info flag bit, unknown cell flag bit, bad `cross` byte.
    let mut bad = good.clone();
    bad[4] = 0x8;
    assert_eq!(cat(&bad), FailureCategory::UnsupportedFlags);
    let cell_flags_at = good
        .windows(4)
        .position(|w| w == 0x2Cu32.to_le_bytes())
        .unwrap();
    let mut bad = good.clone();
    bad[cell_flags_at] = 0x6C;
    assert_eq!(cat(&bad), FailureCategory::UnsupportedFlags);
    let mut bad = good.clone();
    bad[0] ^= 1; // a landblock-info DID of another landblock is fine ...
    let moved = LandblockInfo::parse(&bad, Some(39), &s).unwrap();
    // ... but reported: the cell positions (checked before the IIDs) name the old one.
    assert_eq!(
        moved.first_unusual_field(),
        Some("cell position names another landblock (LI3)")
    );
    bad[2] = 0x00; // ... but a data DID is not.
    assert_eq!(cat(&bad), FailureCategory::DidMismatch);
}

#[test]
fn info_unusual_cells_are_reported() {
    let s = schema();
    let good = default_info(true);
    // Make the first cell's neighbour ordinal 5.
    let at = good
        .windows(4)
        .position(|w| w == 0x0100_0ABCu32.to_le_bytes())
        .unwrap()
        + 8;
    let mut bad = good.clone();
    bad[at] = 5;
    let info = LandblockInfo::parse(&bad, Some(39), &s).unwrap();
    assert!(info.first_unusual_field().unwrap().contains("ordinal"));
}

#[test]
fn entity_type_and_iid_helpers() {
    for code in 0..64 {
        assert_eq!(StaticEntityType::from_code(code).code(), code);
    }
    assert_eq!(StaticEntityType::from_code(7), StaticEntityType::Physics);
    assert_eq!(StaticEntityType::from_code(6), StaticEntityType::Weenie);
    let i = InstanceId::new(4, 14, 254, 3, 0xFFFF_FFFF);
    assert_eq!(
        (
            i.kind(),
            i.region(),
            i.landblock(),
            i.serial(),
            i.reserved_bits()
        ),
        (4, 14, (254, 3), 0xFFFF_FFFF, 0)
    );
}

#[test]
fn quaternion_conventions() {
    // coordinates.md §11.5 worked examples.
    let s = std::f32::consts::FRAC_1_SQRT_2;
    let cases = [
        ([1.0, 0.0, 0.0, 0.0], [0.0, 1.0, 0.0], 0.0),
        ([s, 0.0, 0.0, s], [-1.0, 0.0, 0.0], 270.0),
        ([0.0, 0.0, 0.0, 1.0], [0.0, -1.0, 0.0], 180.0),
        ([s, 0.0, 0.0, -s], [1.0, 0.0, 0.0], 90.0),
    ];
    for (q, fwd, heading) in cases {
        let q = Quaternion {
            w: q[0],
            x: q[1],
            y: q[2],
            z: q[3],
        };
        let r = q.rotate([0.0, 1.0, 0.0]);
        for k in 0..3 {
            assert!((r[k] - fwd[k]).abs() < 1e-5, "{q:?}: {r:?}");
        }
        assert!((q.heading_degrees() - heading).abs() < 1e-3, "{q:?}");
        // The matrix agrees with the rotation.
        let m = q.to_rows();
        let v = [0.3, -1.2, 2.5];
        let mv: Vec<f32> = m
            .iter()
            .map(|row| row.iter().zip(v).map(|(a, b)| a * b).sum())
            .collect();
        let qv = q.rotate(v);
        for k in 0..3 {
            assert!((mv[k] - qv[k]).abs() < 1e-5);
        }
        assert!(q.is_pure_yaw(1e-4));
    }
    // Tilt 10° about +X: model +Z leans south.
    let t = Quaternion {
        w: 0.996_19,
        x: 0.087_16,
        y: 0.0,
        z: 0.0,
    };
    let z = t.rotate([0.0, 0.0, 1.0]);
    assert!((z[1] + 0.173_65).abs() < 1e-4 && (z[2] - 0.984_81).abs() < 1e-4);
    assert!(!t.is_pure_yaw(1e-4));
    let y = Quaternion::from_yaw(1.0);
    assert!((y.yaw() - 1.0).abs() < 1e-5 && (y.length() - 1.0).abs() < 1e-6);
    // Tilted rotations take ψ from the z-y-x decomposition (coordinates.md §11.4), not from
    // the projected model +Y axis. The §11.4 example: 45° about +X, then 30° about +Y, has
    // ψ = 0 although its model +Y projects to a bearing of 26.6°.
    let tilted = Quaternion {
        w: 0.8924,
        x: 0.369_64,
        y: 0.239_12,
        z: -0.099_05,
    };
    assert!(tilted.yaw().abs() < 1e-3, "{}", tilted.yaw());
    let h = tilted.heading_degrees();
    assert!(h < 0.1 || h > 359.9, "{h}");
    // 45° yaw followed by a 60° tilt about +X: ψ = atan2(2(xy + wz), w² + x² − y² − z²).
    let q = Quaternion {
        w: 0.800_103,
        x: 0.461_94,
        y: -0.191_342,
        z: 0.331_414,
    };
    let psi = f64::atan2(
        2.0 * (0.461_94 * -0.191_342 + 0.800_103 * 0.331_414),
        0.800_103f64.powi(2) + 0.461_94f64.powi(2) - 0.191_342f64.powi(2) - 0.331_414f64.powi(2),
    );
    assert!((f64::from(q.heading_degrees()) - (360.0 - psi.to_degrees())).abs() < 1e-2);
    assert!(
        (q.heading_degrees() - 333.43).abs() < 0.01,
        "{}",
        q.heading_degrees()
    );
    // Rotated +X vertical: atan2(0, 0) = 0, heading north.
    let up = Quaternion {
        w: 0.5,
        x: -0.5,
        y: -0.5,
        z: -0.5,
    };
    assert_eq!(up.heading_degrees(), 0.0);
    assert!(!Quaternion { w: f32::NAN, ..y }.is_finite());
}

#[test]
fn region_positions_and_dids() {
    let g = region_position(100, 150, [40.0, 80.0, 399.99]);
    assert_eq!(g[..2], [16_040.0, 24_080.0]);
    assert_eq!(
        landblock_did(LandblockKind::Heightmap, 1, 126, 140),
        Did(0x8001_7E8C)
    );
    assert_eq!(
        landblock_did(LandblockKind::Info, 14, 1, 2),
        Did(0x802E_0102)
    );
    for (did, ev) in [
        (DATA, Evidence::Decoded),
        (INFO, Evidence::Decoded),
        (MAP, Evidence::Decoded),
        (PROPS, Evidence::Decoded),
        (0x0F00_0000, Evidence::Decoded),
        (0x1800_0000, Evidence::Decoded),
        (0x8051_0303, Evidence::Unknown),
    ] {
        assert_eq!(Did(did).kind().evidence(), ev, "{did:#x}");
    }
    assert_eq!(Did(MAP).kind(), DidKind::Landblock(LandblockKind::BlockMap));
}

// ---------------------------------------------------------------- block maps

fn block_map_bytes() -> Vec<u8> {
    B::new()
        .u32(MAP)
        .tsize(1, 3)
        .u32(1)
        .u16(2)
        .u32(9)
        .u16(1)
        .u32(4)
        .u16(77)
        .build()
}

fn descriptor_bytes() -> Vec<u8> {
    B::new()
        .u32(0x1800_0000)
        .u32(0)
        .tsize(1, 2)
        // key 1: an Int palette with a null element in slot 1.
        .u32(INT)
        .u32(INT)
        .u32(1)
        .u8(1)
        .u32(3)
        .u32(INT)
        .u32(10)
        .u32(0)
        .u32(INT)
        .u32(30)
        // key 9: a Float palette.
        .u32(FLOAT)
        .u32(FLOAT)
        .u32(9)
        .u8(0)
        .u32(2)
        .u32(FLOAT)
        .f32(0.0)
        .u32(FLOAT)
        .f32(1.0)
        .build()
}

#[test]
fn block_map_resolves_through_its_descriptor() {
    let bytes = block_map_bytes();
    let m = BlockMap::parse_expecting(&bytes, MAP).unwrap();
    assert_eq!(m.to_bytes(), bytes);
    assert_eq!(m.get(9), Some(1));
    assert_eq!(m.get(5), None);
    let d = BlockMapDescriptor::parse(&descriptor_bytes(), &schema()).unwrap();
    assert_eq!(d.first_unusual_field(), None);
    let r = d.resolve(&m);
    assert_eq!(r[0].value, Some(&PropertyValue::Int(30)));
    assert_eq!(r[1].value, Some(&PropertyValue::Float(1.0)));
    assert_eq!(
        (r[2].pid, r[2].value),
        (None, None),
        "key without descriptor entry"
    );
    assert_eq!(d.value_of(&m, FLOAT), Some(&PropertyValue::Float(1.0)));
    assert_eq!(d.entry(1).unwrap().value(1), None, "null palette element");
    assert_eq!(d.entry(1).unwrap().value(3), None, "out of range");
    assert_eq!(d.entry(1).unwrap().values.len(), 3);
}

#[test]
fn block_map_errors() {
    let bytes = block_map_bytes();
    for n in 0..bytes.len() {
        assert!(BlockMap::parse(&bytes[..n]).is_err());
    }
    let mut extra = bytes.clone();
    extra.push(0);
    assert!(matches!(
        BlockMap::parse(&extra),
        Err(FormatError::TrailingBytes { .. })
    ));
    let mut wrong = bytes.clone();
    wrong[2] = 0x21; // kind 2
    assert!(matches!(
        BlockMap::parse(&wrong),
        Err(FormatError::WrongRecordType { .. })
    ));
    let huge = B::new().u32(MAP).tsize(0, 1_000_000).build();
    assert!(matches!(
        BlockMap::parse(&huge),
        Err(FormatError::Truncated { .. })
    ));

    let d = descriptor_bytes();
    for n in 0..d.len() {
        assert!(BlockMapDescriptor::parse(&d[..n], &schema()).is_err());
    }
    let mut unknown = d.clone();
    unknown[4 + 4 + 2..4 + 4 + 2 + 4].copy_from_slice(&0xAAAAu32.to_le_bytes());
    // The entry PID itself is not decoded through the schema; its palette values are.
    assert!(BlockMapDescriptor::parse(&unknown, &schema()).is_ok());
    let no_schema: HashMap<u32, PropertyType> = HashMap::new();
    let e = BlockMapDescriptor::parse(&d, &no_schema).unwrap_err();
    assert_eq!(FailureCategory::of(&e), FailureCategory::PropertySet);
}

#[test]
fn bucket_order_is_checked() {
    let m = BlockMap::parse(&block_map_bytes()).unwrap();
    // Hint 1 = 23 buckets: keys 1, 9, 4 → buckets 1, 9, 4: not ordered.
    assert_eq!(m.is_bucket_ordered(), Some(false));
    assert!(m.first_unusual_field().is_some());
}

/// The 114-byte layout of every observed record (`environment.md` §3.1): fixed fields, maps
/// 1–7 with one key-0 entry each, maps 8–9 empty, then the trailing 0x19 DID.
fn region_descriptor_bytes() -> Vec<u8> {
    let mut b = B::new()
        .u32(0x0F00_0001)
        .u32(2)
        .u32(0x410D_9AE6)
        .u32(2)
        .u32(1468)
        .u32(1244)
        .f32(0.0)
        .f32(0.0)
        .f32(0.005);
    for did in [
        0x1100_0001u32,
        0x1200_0001,
        0x1400_0001,
        0x1500_0007,
        0x1600_0004,
        0x1700_0001,
        0x1800_0014,
    ] {
        b = b.tsize(1, 1).u32(0).u32(did);
    }
    b.tsize(1, 0).tsize(1, 0).u32(0x1900_0001).build()
}

#[test]
fn region_descriptor_names_its_tables() {
    let bytes = region_descriptor_bytes();
    assert_eq!(bytes.len(), REGION_DESCRIPTOR_LEN);
    let d = RegionDescriptor::parse(&bytes).unwrap();
    assert_eq!(d.region, 2);
    assert_eq!(d.map_image, Did(0x410D_9AE6));
    assert_eq!(d.value_12, 2);
    assert_eq!(d.map_offset, [0.0, 0.0]);
    // environment.md §3.2, row 0x0F000001.
    assert_eq!(d.scene_choice_table(), Some(Did(0x1100_0001)));
    assert_eq!(d.terrain_scene_table(), Some(Did(0x1200_0001)));
    assert_eq!(d.encounter_table(), Some(Did(0x1400_0001)));
    assert_eq!(d.sky_description(), Some(Did(0x1500_0007)));
    assert_eq!(d.water_table(), Some(Did(0x1600_0004)));
    assert_eq!(d.fog_zone_table(), Some(Did(0x1700_0001)));
    assert_eq!(d.block_map_descriptor(), Some(Did(0x1800_0014)));
    assert_eq!(d.terrain_type_table(), Some(Did(0x1900_0001)));
    for (t, did) in d.tables() {
        assert_eq!(did.unwrap().prefix(), t.prefix(), "{}", t.name());
        assert_eq!(d.table_with_prefix(t.prefix()), did);
    }
    let slots: Vec<usize> = RegionTable::ALL.iter().map(|t| t.slot()).collect();
    assert_eq!(slots, (1..=8).collect::<Vec<_>>());
    assert_eq!(d.table_with_prefix(0x20), None);
    assert!(d.maps[7].entries().is_empty() && d.maps[8].entries().is_empty());
    // Copy, so callers can keep it by value.
    let copy = d;
    assert_eq!(copy, d);
    assert_eq!(d.map_origin_units(), [29_360.0, 24_880.0]);
    // coordinates.md §8.4 D: the origin is (0, 0) exactly.
    assert_eq!(d.map_coordinates([183, 155], [80.0, 80.0]), [0.0, 0.0]);
    assert_eq!(d.first_unusual_field(), None);
    assert_eq!(d.to_bytes(), bytes);
    for n in 0..bytes.len() {
        assert!(RegionDescriptor::parse(&bytes[..n]).is_err());
    }
    let mut long = bytes.clone();
    long.push(0);
    assert!(RegionDescriptor::parse(&long).is_err());
    let mut wrong = bytes.clone();
    wrong[3] = 0x10;
    let e = RegionDescriptor::parse(&wrong).unwrap_err();
    assert!(matches!(e, FormatError::WrongRecordType { .. }));
}

/// The grammar, not the size, decides (`environment.md` §3.1): extra keys, empty maps and
/// filled maps 8–9 parse and are reported as unusual; the typed accessors take key 0.
#[test]
fn region_descriptor_parses_the_grammar() {
    let mut b = B::new()
        .u32(0x0F00_0009)
        .u32(6)
        .u32(0x4100_000F)
        .u32(2)
        .u32(1)
        .u32(2)
        .f32(1.5)
        .f32(-2.0)
        .f32(0.01);
    // Slot 1: two entries, key 0 second. Slot 2: empty. Slots 3–7: one entry each.
    b = b
        .tsize(1, 2)
        .u32(5)
        .u32(0x1100_0099)
        .u32(0)
        .u32(0x1100_000A);
    b = b.tsize(1, 0);
    for did in [
        0x1400_000Au32,
        0x1500_0007,
        0x1600_000E,
        0x1700_000A,
        0x1900_0000,
    ] {
        b = b.tsize(1, 1).u32(0).u32(did);
    }
    // Map 8 holds an entry; map 9 is empty; null trailing DID.
    let bytes = b
        .tsize(3, 1)
        .u32(0)
        .u32(0x2000_0001)
        .tsize(1, 0)
        .u32(0)
        .build();
    let d = RegionDescriptor::parse(&bytes).unwrap();
    assert_eq!(d.scene_choice_table(), Some(Did(0x1100_000A)));
    assert_eq!(d.maps[0].get(5), Some(Did(0x1100_0099)));
    assert_eq!(d.terrain_scene_table(), None);
    // Slot 7 names a 0x19 DID: the accessor returns it, the check reports it.
    assert_eq!(d.block_map_descriptor(), Some(Did(0x1900_0000)));
    assert_eq!(d.terrain_type_table(), None);
    assert!(d.first_unusual_field().is_some());
    // §8.2 step 3 with offsets: (Δ − (f32(X0) · 20) · s) + (X · 160 + x) · s.
    let m = d.map_coordinates([0, 0], [0.0, 0.0]);
    assert_eq!(
        m,
        [
            (1.5 - (1.0f32 * 20.0) * 0.01) + 0.0,
            (-2.0 - (2.0f32 * 20.0) * 0.01) + 0.0
        ]
    );
    assert_eq!(d.to_bytes(), bytes);
    // A huge map count is rejected before anything is allocated.
    let huge = B::new()
        .u32(0x0F00_0000)
        .bytes(&[0; 32])
        .tsize(1, 0x3FFF_FFFF)
        .build();
    let e = RegionDescriptor::parse(&huge).unwrap_err();
    assert_eq!(FailureCategory::of(&e), FailureCategory::Truncated);
    // More entries than the decoder keeps inline: a limit, not a misframe.
    let mut b = B::new().u32(0x0F00_0000).bytes(&[0; 32]);
    b = b.tsize(1, REGION_DID_MAP_CAPACITY as u32 + 1);
    for k in 0..=REGION_DID_MAP_CAPACITY as u32 {
        b = b.u32(k).u32(0x1100_0000);
    }
    let e = RegionDescriptor::parse(&b.build()).unwrap_err();
    assert_eq!(FailureCategory::of(&e), FailureCategory::Limit);
    let entries = [RegionDidMapEntry {
        key: 0,
        did: Did(0x1100_0000),
    }; REGION_DID_MAP_CAPACITY];
    let full = RegionDidMap::new(1, &entries).unwrap();
    assert_eq!(full.entries().len(), REGION_DID_MAP_CAPACITY);
    assert_eq!(full.len(), REGION_DID_MAP_CAPACITY);
    assert!(RegionDidMap::new(1, &[entries[0]; REGION_DID_MAP_CAPACITY + 1]).is_none());
}

/// coordinates.md §8.2 step 3 (every operation in f32, in the stated order) against the
/// worked examples of §8.4, with the observed constants (origin 1,468 / 1,244, offsets 0,
/// scale f32 0.005).
#[test]
fn region_map_coordinates_follow_the_f32_order() {
    let d = RegionDescriptor::parse(&region_descriptor_bytes()).unwrap();
    let lonlat = |lb: [u8; 2], local: [f32; 2]| d.map_coordinates(lb, local);
    // A: landblock (100, 150), local (40, 80).
    assert_eq!(lonlat([100, 150], [40.0, 80.0]), [-66.600_006, -4.0]);
    // C: landblock (200, 170), local (0, 0).
    assert_eq!(lonlat([200, 170], [0.0, 0.0]), [13.199_997, 11.600_006]);
    // D: the origin.
    assert_eq!(lonlat([183, 155], [80.0, 80.0]), [0.0, 0.0]);
    // "More vectors" table.
    let rows: [([u8; 2], [f32; 2], [f32; 2]); 7] = [
        ([183, 155], [90.0, 80.0], [0.049_987_8, 0.0]),
        ([183, 155], [70.0, 80.0], [-0.050_003_1, 0.0]),
        ([183, 155], [74.0, 76.0], [-0.029_998_8, -0.019_996_6]),
        ([183, 155], [70.03, 80.0], [-0.049_865_7, 0.0]),
        ([183, 155], [70.06, 80.0], [-0.049_697_9, 0.0]),
        ([254, 254], [160.0, 160.0], [57.199_997, 79.600_006]),
        ([0, 0], [0.0, 0.0], [-146.8, -124.399_994]),
    ];
    for (lb, local, want) in rows {
        let got = lonlat(lb, local);
        // The table prints six or seven significant digits; one f32 ulp of slack at
        // that precision covers the rounding of the printed value.
        for k in 0..2 {
            assert!(
                (got[k] - want[k]).abs() <= 1e-7 + want[k].abs() * 1e-6,
                "{lb:?} {local:?}: {got:?} vs {want:?}"
            );
        }
    }
    // Regressions from the f64 helper: these differ in the last place, and the
    // §8.3 display rule (|v| + 0.0502, × 10, truncate) turns them into different text.
    assert_eq!(lonlat([0, 0], [10.04, 0.0])[0], -146.749_8);
    assert_eq!(lonlat([0, 0], [110.04, 0.0])[0], -146.249_8);
    let shown = |v: f32| ((f64::from(v.abs()) + f64::from(0.0002f32) + 0.05) * 10.0).trunc();
    assert_eq!(shown(lonlat([0, 0], [10.04, 0.0])[0]), 1468.0);
    assert_eq!(shown(lonlat([0, 0], [110.04, 0.0])[0]), 1463.0);
}

/// A RegionDidMap's count cannot disagree with its entries: equality is (hint, entries).
#[test]
fn region_did_map_count_is_consistent() {
    let d = RegionDescriptor::parse(&region_descriptor_bytes()).unwrap();
    let m = d.maps[0];
    assert_eq!(m.len(), 1);
    assert_eq!(m.hint(), 1);
    assert_eq!(m.size().count, 1);
    assert!(!m.is_empty());
    assert_eq!(m, RegionDidMap::new(1, m.entries()).unwrap());
    assert_ne!(m, RegionDidMap::new(1, &[]).unwrap());
    assert!(d.maps[7].is_empty());
    assert_eq!(d.maps[7], RegionDidMap::new(1, &[]).unwrap());
}

// ---------------------------------------------------------------- properties, kind 5

fn properties_bytes() -> Vec<u8> {
    properties_bytes_with_scale(1.25)
}

fn properties_bytes_with_scale(scale: f32) -> Vec<u8> {
    B::new()
        .u32(PROPS)
        .tsize(0, 2)
        .u32(GAME_MAP)
        .u32(GAME_MAP)
        .tsize(0, 7)
        .u32(MAP_IMAGE)
        .u32(MAP_IMAGE)
        .u32(0x4100_1234)
        .u32(BX)
        .u32(BX)
        .u32(100)
        .u32(BY)
        .u32(BY)
        .u32(150)
        .u32(PX)
        .u32(PX)
        .u32(0)
        .u32(PY)
        .u32(PY)
        .u32(200)
        .u32(MSCALE)
        .u32(MSCALE)
        .f32(scale)
        .u32(IMAGE_ARRAY)
        .u32(IMAGE_ARRAY)
        .u32(1)
        .u32(IMAGE_STRUCT)
        .tsize(0, 2)
        .u32(CONTENT_LAYER)
        .u32(CONTENT_LAYER)
        .u32(1)
        .u32(MAP_IMAGE)
        .u32(MAP_IMAGE)
        .u32(0x4100_5678)
        .u32(LIGHT_MAP)
        .u32(LIGHT_MAP)
        .u32(0x4100_9999)
        .build()
}

#[test]
fn minimap_tile_and_georeference() {
    let p = LandblockProperties::parse_expecting(&properties_bytes(), PROPS, &schema()).unwrap();
    let pids = pids();
    let t = p.minimap(&pids).unwrap();
    assert_eq!(t.image, Did(0x4100_1234));
    assert_eq!(
        (t.block_offset, t.pixel_offset, t.scale),
        ([100, 150], [0, 200], 1.25)
    );
    assert_eq!(t.variants.len(), 1);
    assert_eq!(t.image_for(|l| l == 1), Did(0x4100_5678));
    assert_eq!(t.image_for(|_| false), Did(0x4100_1234));
    // coordinates.md §13: tile (100, 150), pixel (50, 100) → G (16,040, 24,080).
    assert_eq!(t.pixel_to_region(50.0, 100.0), Some([16_040.0, 24_080.0]));
    assert_eq!(t.region_to_pixel([16_040.0, 24_080.0]), [50.0, 100.0]);
    assert_eq!(p.light_map(&pids), Some(Did(0x4100_9999)));
    assert_eq!(p.ceiling_light_map(&pids), None);
    assert_eq!(p.minimap(&LandblockPids::default()), None);
    // An unusable UI_Map_Scale counts as a missing field (coordinates.md §10 divides by it).
    for bad in [0.0, -1.25, f32::NAN, f32::INFINITY, f32::NEG_INFINITY] {
        let p = LandblockProperties::parse(&properties_bytes_with_scale(bad), &schema()).unwrap();
        assert_eq!(p.minimap(&pids), None, "{bad}");
    }
    let bytes = properties_bytes();
    for n in 0..bytes.len() {
        assert!(LandblockProperties::parse(&bytes[..n], &schema()).is_err());
    }
}

#[test]
fn group_record_framing() {
    let mut b = B::new().u32(0x8051_0303).u32(GROUP_SAMPLE_COUNT);
    for k in 0..GROUP_SAMPLE_COUNT {
        b = b.u16(k as u16).u8(127).u8(255).u8(127);
    }
    let bytes = b.u32(0).u32(7).build();
    let g = LandblockGroupRecord::parse(&bytes).unwrap();
    assert_eq!(g.sample_count(), GROUP_SAMPLE_COUNT as usize);
    assert_eq!(g.sample(5).unwrap().value, 5);
    assert_eq!(g.sample(5).unwrap().vector, [127, 255, 127]);
    assert_eq!(g.sample(9801), None);
    assert_eq!(g.tail.len(), 8);
    assert_eq!(g.first_unusual_field(), None);
    let odd = B::new().u32(0x8051_0304).u32(1).bytes(&[0; 5]).build();
    assert!(
        LandblockGroupRecord::parse(&odd)
            .unwrap()
            .first_unusual_field()
            .is_some()
    );
    let huge = B::new().u32(0x8051_0303).u32(u32::MAX).build();
    assert!(LandblockGroupRecord::parse(&huge).is_err());
    assert!(LandblockGroupRecord::parse(&B::new().u32(DATA).u32(0).build()).is_err());
}

/// landblock.md §6.4, terrain-texturing.md §10: both light maps with their surface DIDs;
/// row 0 on the south edge.
#[test]
fn light_maps_and_their_orientation() {
    let bytes = B::new()
        .u32(PROPS)
        .tsize(0, 2)
        .u32(LIGHT_MAP)
        .u32(LIGHT_MAP)
        .u32(0x4100_9999)
        .u32(MAP_IMAGE)
        .u32(MAP_IMAGE)
        .u32(0x4100_8888)
        .build();
    let p = LandblockProperties::parse(&bytes, &schema()).unwrap();
    let pids = LandblockPids {
        landblock_light_map: Some(LIGHT_MAP),
        landblock_ceiling_light_map: Some(MAP_IMAGE),
        ..LandblockPids::default()
    };
    let l = p.light_maps(&pids);
    assert_eq!(
        l,
        LightMaps {
            terrain: Some(Did(0x4100_9999)),
            ceiling: Some(Did(0x4100_8888)),
        }
    );
    assert_eq!(l.for_surface(GridSurface::Ceiling), Some(Did(0x4100_8888)));
    assert!(!l.is_empty());
    assert!(p.light_maps(&LandblockPids::default()).is_empty());
    // Only the terrain map under the shared fixture's PIDs.
    let p = LandblockProperties::parse(&properties_bytes(), &schema()).unwrap();
    assert_eq!(p.light_maps(&self::pids()).ceiling, None);
    assert_eq!(light_map_uv(0.0, 0.0), [0.0, 0.0]);
    assert_eq!(light_map_uv(160.0, 40.0), [1.0, 0.25]);
}

/// water.md §9 / WA5: a (type, height) pair is found in the trailing section as its 5 bytes.
#[test]
fn sector_water_pairs_are_found_in_the_tail() {
    let pair = SectorWaterPair {
        water_type: 7,
        height: 141.17647,
    };
    assert_eq!(pair.to_bytes()[0], 7);
    assert_eq!(&pair.to_bytes()[1..], &141.17647f32.to_le_bytes());
    let mut b = B::new().u32(0x8051_0603).u32(1).bytes(&[0; 5]);
    // A tail: u32 0, some bytes, a one-pair list (VLE 1, type, height), more bytes.
    b = b.u32(0).u8(9).vle(1).u8(7).f32(141.17647).u8(1).u8(0);
    let g: DistantLandscapeSector = LandblockGroupRecord::parse(&b.build()).unwrap();
    assert!(g.tail_contains_water_pair(pair));
    assert!(!g.tail_contains_water_pair(SectorWaterPair {
        water_type: 8,
        ..pair
    }));
    assert!(!g.tail_contains_water_pair(SectorWaterPair {
        height: 141.17648,
        ..pair
    }));
    assert_eq!(g.sector_origin(), Some((6, 3)));
    let blocks = g.sector_landblocks();
    assert_eq!(blocks.len(), 9);
    assert_eq!(blocks[0], (6, 3));
    assert_eq!(blocks[8], (8, 5));
    // At the grid edge the group is clipped.
    let edge = LandblockGroupRecord::parse(&B::new().u32(0x8051_FFFE).u32(0).build()).unwrap();
    assert_eq!(edge.sector_landblocks().len(), 2);
    assert!(!edge.tail_contains_water_pair(pair));
}

// ---------------------------------------------------------------- review regressions

/// A clean record: the default cells plus one physics SED with an identity rotation.
fn clean_info() -> LandblockInfo {
    let bytes = info_bytes(&InfoParts {
        entities: vec![physics_sed(B::new(), 7, [1.0, 0.0, 0.0, 0.0]).build()],
        trailer: false,
    });
    let info = LandblockInfo::parse(&bytes, Some(20), &schema()).unwrap();
    assert_eq!(info.first_unusual_field(), None);
    info
}

/// The byte offset of the first cell's Position flag byte in `info_bytes` output.
fn first_cell_position_at(bytes: &[u8]) -> usize {
    bytes
        .windows(6)
        .position(|w| w == [0x1B, 1, 100, 120, 0x00, 0x01])
        .unwrap()
}

#[test]
fn cell_position_without_offset_decodes() {
    // Flag byte 0x4B: region, landblock, cell and the payload-less 0x40 bit, no x, y, z
    // (two observed cells, landblock.md §4.4).
    let mut bytes = info_bytes(&InfoParts {
        entities: vec![],
        trailer: false,
    });
    let at = first_cell_position_at(&bytes);
    bytes[at] = 0x4B;
    bytes.drain(at + 6..at + 18);
    let info = LandblockInfo::parse(&bytes, Some(20), &schema()).unwrap();
    let c = &info.cells()[0];
    assert_eq!(c.position.flags(), 0x4B);
    assert_eq!(c.position.offset(), None);
    assert_eq!(c.position.cell(), Some(0x100));
    assert_eq!(c.rotation(), None);
    assert_eq!(info.first_unusual_field(), None);
}

#[test]
fn cell_rotation_is_typed_wxyz() {
    // Flag byte 0x3B: offset and orientation; the four floats are (w, x, y, z).
    let mut bytes = info_bytes(&InfoParts {
        entities: vec![],
        trailer: false,
    });
    let at = first_cell_position_at(&bytes);
    bytes[at] = 0x3B;
    let q = B::new()
        .f32(FRAC_1_SQRT_2)
        .f32(0.0)
        .f32(0.0)
        .f32(FRAC_1_SQRT_2)
        .build();
    bytes.splice(at + 18..at + 18, q);
    let info = LandblockInfo::parse(&bytes, Some(20), &schema()).unwrap();
    let c = &info.cells()[0];
    let r = c.rotation().unwrap();
    assert_eq!(
        (r.w, r.x, r.y, r.z),
        (FRAC_1_SQRT_2, 0.0, 0.0, FRAC_1_SQRT_2)
    );
    // A 90° yaw: +X turns to +Y.
    let v = r.rotate([1.0, 0.0, 0.0]);
    assert!(v[0].abs() < 1e-6 && (v[1] - 1.0).abs() < 1e-6, "{v:?}");
    assert_eq!(r.to_xyzw(), [0.0, 0.0, FRAC_1_SQRT_2, FRAC_1_SQRT_2]);
    let p = Position::new(0).with_orientation([1.0, 2.0, 3.0, 4.0]);
    assert_eq!(p.rotation().unwrap().to_wxyz(), [1.0, 2.0, 3.0, 4.0]);
    assert_eq!(Position::new(0).rotation(), None);
}

#[test]
fn unusual_field_reports_every_branch() {
    let base = clean_info();
    let check = |f: &dyn Fn(&mut LandblockInfo), want: &str| {
        let mut i = base.clone();
        f(&mut i);
        assert_eq!(i.first_unusual_field(), Some(want));
    };
    fn cell(i: &mut LandblockInfo) -> &mut Cell {
        &mut cells_mut(i)[0]
    }
    check(&|i| i.reserved_a = 1, "reserved word not 0 (LI2)");
    check(&|i| i.reserved_b = 1, "reserved word not 0 (LI2)");
    check(&|i| i.flags &= !1, "cells flag (bit 0) clear");
    check(
        &|i| cell(i).position = cell(i).position.with_landblock(100, 121),
        "cell position names another landblock (LI3)",
    );
    check(
        &|i| cell(i).position = cell(i).position.with_region(2),
        "cell position names another landblock (LI3)",
    );
    check(
        &|i| cell(i).position = cell(i).position.with_cell(0x555),
        "cell position names another cell (LI3)",
    );
    check(
        &|i| cell(i).neighbours[0].cell = 0x555,
        "neighbour is not a cell of this record (LI3)",
    );
    check(
        &|i| cell(i).neighbours[0].reserved = [0, 1, 0],
        "neighbour reserved u16 not 0 (LI3)",
    );
    check(
        &|i| cell(i).index_echo = Some(0x555),
        "cell index echo differs (LI3)",
    );
    check(
        &|i| cell(i).list_a = vec![0x101, 0x555],
        "cell list A names a non-cell (LI3)",
    );
    check(
        &|i| cell(i).geometry = Did(0x0200_0001),
        "cell geometry is not a 0x01 DID (LI4)",
    );
    check(
        &|i| i.entities[0].iid = Some(InstanceId::new(1, 1, 100, 121, 7)),
        "IID names another landblock (ED3)",
    );
    check(
        &|i| i.entities[0].iid = Some(InstanceId::new(1, 2, 100, 120, 7)),
        "IID names another landblock (ED3)",
    );
    check(
        &|i| i.entities[0].iid = Some(InstanceId(InstanceId::new(1, 1, 100, 120, 7).0 | 1 << 33)),
        "IID bits 32–35 not zero (ED3)",
    );
    check(
        &|i| {
            let mut p = i.entities[0].placement.unwrap();
            p.stored_rotation = Quaternion {
                w: 1.005,
                ..Quaternion::IDENTITY
            };
            i.entities[0].placement = Some(p);
        },
        "rotation not unit length (ED5)",
    );
    check(
        &|i| i.entities[0].flags &= !0x2,
        "SED lacks type, IID or property set (ED1)",
    );

    // Membership uses every u16, including the extremes.
    let mut i = base.clone();
    let cells = &mut i.cells.as_mut().unwrap().items;
    for (c, index) in cells.iter_mut().zip([0u16, 0xFFFF]) {
        c.index = index;
        c.index_echo = Some(index);
        c.position = c.position.with_cell(index);
    }
    cells[0].neighbours[0].cell = 0xFFFF;
    cells[0].list_a = vec![0xFFFF, 0];
    cells[1].neighbours[0].cell = 0;
    cells[1].list_a = vec![0];
    assert_eq!(i.first_unusual_field(), None);
    cells_mut(&mut i)[1].list_a = vec![0xFFFE];
    assert_eq!(
        i.first_unusual_field(),
        Some("cell list A names a non-cell (LI3)")
    );
}

fn cells_mut(i: &mut LandblockInfo) -> &mut Vec<Cell> {
    &mut i.cells.as_mut().unwrap().items
}

#[test]
fn unusual_field_is_linear_in_cells() {
    // Many cells and a long list A: a per-entry scan over the cells would take seconds
    // (quadratic); the index set keeps it linear.
    let mut info = clean_info();
    let template = info.cells()[0].clone();
    let cells: Vec<Cell> = (0..20_000u16)
        .map(|k| Cell {
            index: k,
            position: template.position.with_cell(k),
            index_echo: Some(k),
            neighbours: vec![],
            list_a: vec![],
            ..template.clone()
        })
        .collect();
    *cells_mut(&mut info) = cells;
    cells_mut(&mut info)[0].list_a = vec![19_999; 250_000];
    let t = std::time::Instant::now();
    assert_eq!(info.first_unusual_field(), None);
    assert!(t.elapsed().as_secs_f32() < 5.0, "{:?}", t.elapsed());
}

#[test]
fn entity_placement_is_bounded_and_unit() {
    let s = schema();
    let one = |sed: Vec<u8>| {
        info_bytes(&InfoParts {
            entities: vec![sed],
            trailer: false,
        })
    };
    let sed = |pos: [f32; 3], rot: [f32; 4]| {
        let mut b = B::new().u32(0x10);
        for c in pos.into_iter().chain(rot) {
            b = b.f32(c);
        }
        b.build()
    };
    let placed = |pos: [f32; 3], rot: [f32; 4]| one(sed(pos, rot));
    // Positions beyond the bound are malformed; the bound itself is accepted.
    let e =
        LandblockInfo::parse(&placed([3e38; 3], [1.0, 0.0, 0.0, 0.0]), Some(20), &s).unwrap_err();
    assert_eq!(FailureCategory::of(&e), FailureCategory::Malformed);
    let edge = [MAX_PLACEMENT_COORD, -MAX_PLACEMENT_COORD, 0.0];
    assert!(LandblockInfo::parse(&placed(edge, [1.0, 0.0, 0.0, 0.0]), Some(20), &s).is_ok());

    // ED5: 1.005 is within the 0.01 renormalise threshold but outside ±0.001. The rotation
    // is normalised anyway (no 1 % scaling) and the SED is reported.
    let bytes = placed([80.0, 40.0, 0.0], [1.005, 0.0, 0.0, 0.0]);
    let info = LandblockInfo::parse(&bytes, Some(20), &s).unwrap();
    let p = info.entities[0].placement.unwrap();
    assert!(!p.renormalised && p.is_off_unit());
    assert!((p.rotation.length() - 1.0).abs() < 1e-6);
    assert!((p.stored_length() - 1.005).abs() < 1e-6);
    assert_eq!(p.transform([100.0, 0.0, 0.0], [1.0; 3]), [180.0, 40.0, 0.0]);
    assert_eq!(
        info.entities[0].first_unusual_field(),
        Some("SED lacks type, IID or property set (ED1)")
    );
    let p2 = Placement::new(
        [0.0; 3],
        Quaternion {
            w: 1.0005,
            ..Quaternion::IDENTITY
        },
    )
    .unwrap();
    assert!(!p2.is_off_unit());
    // The stored rotation is what re-encodes.
    let e = &info.entities[0];
    assert_eq!(
        e.to_bytes_with(&[]).unwrap(),
        sed([80.0, 40.0, 0.0], [1.005, 0.0, 0.0, 0.0])
    );

    assert!(Placement::new([f32::NAN, 0.0, 0.0], Quaternion::IDENTITY).is_err());
    assert!(Placement::new([2e6, 0.0, 0.0], Quaternion::IDENTITY).is_err());
    assert!(
        Placement::new(
            [0.0; 3],
            Quaternion {
                w: 0.0,
                ..Quaternion::IDENTITY
            }
        )
        .is_err()
    );
}

/// A physics SED like [`physics_sed`] but with scale vector `v` under `Render_AdjustableScale`.
fn sed_with_scale(v: [f32; 3]) -> StaticEntity {
    let props = B::new()
        .tsize(0, 1)
        .u32(SCALE)
        .u32(SCALE)
        .f32(v[0])
        .f32(v[1])
        .f32(v[2])
        .build();
    StaticEntity {
        flags: 0x92,
        entity_type: Some(7),
        properties: Some(PropertySet::parse(&props, &schema()).unwrap()),
        ..StaticEntity::default()
    }
}

#[test]
fn entity_scale_is_sane_and_normals_follow_it() {
    for bad in [
        [0.0; 3],
        [-1.0; 3],
        [1e38; 3],
        [1.0, 1e-5, 1.0],
        [2.0, 2.0, f32::NAN],
    ] {
        assert_eq!(sed_with_scale(bad).scale(SCALE), [1.0; 3], "{bad:?}");
    }
    assert_eq!(
        sed_with_scale([0.01, 0.5, 100.0]).scale(SCALE),
        [0.01, 0.5, 100.0]
    );
    assert_eq!(sed_with_scale([1.5; 3]).uniform_scale(SCALE), Some(1.5));
    assert_eq!(sed_with_scale([1.0, 2.0, 1.0]).uniform_scale(SCALE), None);

    // Non-uniform scale: normals take the inverse transpose (divide per axis), then rotate.
    let p = Placement::new([0.0; 3], Quaternion::IDENTITY).unwrap();
    let n = p.transform_normal([FRAC_1_SQRT_2, FRAC_1_SQRT_2, 0.0], [2.0, 1.0, 1.0]);
    let want = [1.0 / 5f32.sqrt(), 2.0 / 5f32.sqrt(), 0.0];
    assert!(
        n.iter().zip(want).all(|(a, b)| (a - b).abs() < 1e-6),
        "{n:?}"
    );
    // The surface x + y = 1 scaled by (2, 1, 1) becomes x/2 + y = 1: normal ∝ (1/2, 1, 0). ✓
    // Uniform scale: rotation only; a 90° yaw turns +X into +Y.
    let yaw = Placement::new([0.0; 3], Quaternion::from_yaw(std::f32::consts::FRAC_PI_2)).unwrap();
    let n = yaw.transform_normal([1.0, 0.0, 0.0], [3.0; 3]);
    assert!(n[0].abs() < 1e-6 && (n[1] - 1.0).abs() < 1e-6, "{n:?}");
    // Unusable scales fall back to (1, 1, 1); a zero normal stays zero.
    assert_eq!(
        p.transform_normal([0.0, 0.0, 2.0], [0.0, 1.0, 1.0]),
        [0.0, 0.0, 1.0]
    );
    assert_eq!(p.transform_normal([0.0; 3], [1.0; 3]), [0.0; 3]);
}

#[test]
fn sed_encoder_derives_flags_and_rejects_lossy_fields() {
    let s = schema();
    let props = scale_set(B::new(), 1.0).build();
    let decode = |bytes: &[u8]| {
        let info = LandblockInfo::parse(
            &info_bytes(&InfoParts {
                entities: vec![bytes.to_vec()],
                trailer: false,
            }),
            Some(20),
            &s,
        )
        .unwrap();
        info.entities[0].clone()
    };
    let base = StaticEntity {
        entity_type: Some(7),
        iid: Some(InstanceId::new(1, 1, 100, 120, 9)),
        properties: Some(PropertySet::parse(&props, &s).unwrap()),
        ..StaticEntity::default()
    };
    // A placement with its flag bit clear is still written (bit 0x10 derived).
    let placed = StaticEntity {
        flags: 0,
        placement: Some(Placement::new([1.0, 2.0, 3.0], Quaternion::IDENTITY).unwrap()),
        ..base.clone()
    };
    let back = decode(&placed.to_bytes_with(&props).unwrap());
    assert_eq!(back.flags, 0x96);
    assert_eq!(back.placement, placed.placement);
    // A set bit with no field writes nothing for it (no invented identity placement);
    // the payload-less interior bit is kept.
    let unplaced = StaticEntity {
        flags: 0x410 | 0x100,
        ..base.clone()
    };
    let back = decode(&unplaced.to_bytes_with(&props).unwrap());
    assert_eq!((back.flags, back.placement, back.name), (0x486, None, None));
    // Latin-1 names round-trip; others are rejected rather than rewritten with '?'.
    let named = StaticEntity {
        name: Some("caf\u{e9}".into()),
        ..base.clone()
    };
    assert_eq!(
        decode(&named.to_bytes_with(&props).unwrap()).name,
        named.name
    );
    let snowman = StaticEntity {
        name: Some("a\u{2603}b".into()),
        ..base.clone()
    };
    assert!(snowman.to_bytes_with(&props).is_err());
    // Property-set bytes must match the presence of `properties`.
    assert!(base.to_bytes_with(&[]).is_err());
    let bare = StaticEntity {
        properties: None,
        ..base.clone()
    };
    assert!(bare.to_bytes_with(&props).is_err());
    assert!(bare.to_bytes_with(&[]).is_ok());
    // Unknown flag bits are rejected.
    let odd = StaticEntity {
        flags: 0x800,
        ..base
    };
    assert!(odd.to_bytes_with(&props).is_err());
}

#[test]
fn link_and_weenie_iids_are_structured() {
    let info = LandblockInfo::parse(&default_info(true), Some(39), &schema()).unwrap();
    let w = info.weenies.items[0].iid;
    assert_eq!((w.kind(), w.region(), w.serial()), (4, 1, 1));
    let l = &info.links[1];
    assert_eq!(
        (l.iid, l.to_iid, l.from_iid),
        (InstanceId(4), InstanceId(5), InstanceId(6))
    );
    let by = info.entities_by_iid();
    assert_eq!(by[&InstanceId::new(1, 1, 100, 120, 1)], [0]);
    assert_eq!(by[&InstanceId::new(4, 1, 100, 120, 2)], [1]);
    // Duplicate IIDs are kept, in stored order.
    let mut dup = info.clone();
    dup.entities.push(dup.entities[0].clone());
    assert_eq!(
        dup.entities_by_iid()[&InstanceId::new(1, 1, 100, 120, 1)],
        [0, 2]
    );
}

#[test]
fn property_set_failures_are_categorised_stably() {
    let s = schema();
    // A record whose last bytes are one SED (no cells), so a cut SED ends the payload.
    let cat = |sed: Vec<u8>| {
        let bytes = B::new()
            .u32(INFO)
            .u32(0)
            .u32(0)
            .u32(0)
            .u32(1)
            .bytes(&sed)
            .build();
        FailureCategory::of(&LandblockInfo::parse(&bytes, Some(20), &s).unwrap_err())
    };
    // A set whose count overruns the input, a value cut short, an unknown PID: all are
    // property-set failures, however the property decoder reported them.
    let overrun = B::new().u32(0x80).tsize(0, 50).build();
    assert_eq!(cat(overrun), FailureCategory::PropertySet);
    let short = B::new()
        .u32(0x80)
        .tsize(0, 1)
        .u32(SCALE)
        .u32(SCALE)
        .f32(1.0)
        .f32(1.0) // the third float is missing
        .build();
    assert_eq!(cat(short), FailureCategory::PropertySet);
    let unknown = B::new()
        .u32(0x80)
        .tsize(0, 1)
        .u32(0xABCD)
        .u32(0xABCD)
        .build();
    assert_eq!(cat(unknown), FailureCategory::PropertySet);
    // Truncation outside any set stays `Truncated`; a whole-record property set (kind 4)
    // is a property-set failure when cut.
    let good = default_info(true);
    let e = LandblockInfo::parse(&good[..10], Some(39), &s).unwrap_err();
    assert_eq!(FailureCategory::of(&e), FailureCategory::Truncated);
    let props = B::new().u32(PROPS).tsize(0, 3).build();
    let e = LandblockProperties::parse(&props, &s).unwrap_err();
    assert_eq!(FailureCategory::of(&e), FailureCategory::PropertySet);
}

#[test]
fn block_map_resolve_is_indexed() {
    // Many unique descriptor keys and many unmatched map pairs: an entry scan per pair
    // would be quadratic.
    let entries: Vec<BlockMapProperty> = (1..=20_000u32)
        .map(|k| BlockMapProperty {
            pid: INT,
            pid_echo: INT,
            key: k,
            flag: 0,
            values: vec![PaletteValue {
                pid: INT,
                value: Some(PropertyValue::Int(k as i32)),
            }],
        })
        .collect();
    let mut d = BlockMapDescriptor {
        did: Did(0x1800_0000),
        reserved: 0,
        size: Default::default(),
        entries,
    };
    let mut map = BlockMap::parse(&block_map_bytes()).unwrap();
    map.entries = vec![BlockMapEntry { key: 0, index: 0 }; 200_000];
    map.entries[1] = BlockMapEntry { key: 777, index: 0 };
    let t = std::time::Instant::now();
    let r = d.resolve(&map);
    assert!(t.elapsed().as_secs_f32() < 2.0, "{:?}", t.elapsed());
    assert_eq!((r[0].pid, r[0].value), (None, None));
    assert_eq!(r[1].value, Some(&PropertyValue::Int(777)));
    // Duplicate keys: the first stored entry wins, as with `entry`.
    d.entries[5].key = 777;
    let r = d.resolve(&map);
    assert_eq!(r[1].value, Some(&PropertyValue::Int(6)));
    assert_eq!(d.entry(777).unwrap().key, 777);
    assert_eq!(
        d.entry(777).unwrap().values[0].value,
        Some(PropertyValue::Int(6))
    );
}

#[test]
fn group_sample_index_never_overflows() {
    let g = LandblockGroupRecord::parse(&B::new().u32(0x8051_0303).u32(0).build()).unwrap();
    for i in [usize::MAX / 5, usize::MAX / 5 - 1, usize::MAX] {
        assert_eq!(g.sample(i), None);
    }
}
