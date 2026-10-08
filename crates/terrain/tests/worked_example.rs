// SPDX-License-Identifier: GPL-3.0-or-later
//! The synthetic worked example of `docs/specs/terrain-texturing.md` §14, reproduced
//! exactly: the two-entry terrain type table (written byte by byte from §3.3), the block at
//! landblock (3, 7) with a `whole` base layer and a masked layer of 10 triangles, and the
//! point (12.5, 3.0): per-block offset, texture coordinates of both layers, the feature
//! switches, the mask texel, the opacity and the blended result.

use tessera_formats::landblock::{
    GRID_SIDE, GridSurface, HeightGrid, LandblockData, LightMaps, SurfaceChunk, SurfaceLayer,
};
use tessera_formats::terrain_type::TerrainTypeTable;
use tessera_formats::{Did, Record};
use tessera_terrain::{
    AlphaMask, ColourAlphaRole, CoordinateSet, LayerVertices, Neighbours, OpacityRule,
    ResolveOptions, TerrainBlock, TextureFrame, VertexNormals, fixed_block_offset, mask_texel,
    resolve_landblock,
};

const TABLE: u32 = 0x1900_0042;

/// One §14 entry, field by field in the order of §3.3.
#[allow(clippy::too_many_arguments)]
fn entry(
    out: &mut Vec<u8>,
    key: u32,
    draw_order: u32,
    diffuse: u32,
    scale: f32,
    rotation: f32,
    use_opacity_mask: bool,
    threshold: f32,
    blend_max: f32,
    diffuse_scale: [u8; 4],
) {
    let start = out.len();
    out.extend_from_slice(&key.to_le_bytes());
    out.push(6);
    out.extend_from_slice(b"type_k");
    out.extend_from_slice(&draw_order.to_le_bytes());
    out.extend_from_slice(&diffuse.to_le_bytes());
    out.extend_from_slice(&scale.to_le_bytes());
    out.extend_from_slice(&scale.to_le_bytes());
    out.extend_from_slice(&rotation.to_le_bytes());
    for _ in 0..2 {
        out.extend_from_slice(&1u32.to_le_bytes()); // curve mode 1
        out.extend_from_slice(&0f32.to_le_bytes());
    }
    out.extend_from_slice(&3u32.to_le_bytes()); // projection XY
    out.extend_from_slice(&[u8::from(use_opacity_mask), 0, 0, 0, 0]);
    out.extend_from_slice(&threshold.to_le_bytes());
    out.extend_from_slice(&blend_max.to_le_bytes());
    out.extend_from_slice(&128u32.to_le_bytes()); // mask resolution
    out.extend_from_slice(&0u32.to_le_bytes()); // physics material
    out.push(1); // procedural
    out.extend_from_slice(&diffuse_scale);
    for _ in 0..3 {
        out.extend_from_slice(&0u32.to_le_bytes()); // normal, emissive, glow
    }
    out.extend_from_slice(&[0, 0, 0, 0xFF]); // glow colour
    out.extend_from_slice(&[0xFF; 4]); // specular colour
    out.extend_from_slice(&8f32.to_le_bytes());
    assert_eq!(
        out.len() - start,
        101,
        "§14: each entry is 4 + 1 + 6 + 90 bytes"
    );
}

fn table(opacity_on_key1: bool) -> TerrainTypeTable {
    let mut b = Vec::new();
    b.extend_from_slice(&TABLE.to_le_bytes());
    b.extend_from_slice(&2u32.to_le_bytes());
    entry(
        &mut b,
        0,
        10,
        0x4000_0001,
        0.1,
        0.0,
        false,
        0.0,
        0.0,
        [0xFF; 4],
    );
    entry(
        &mut b,
        1,
        20,
        0x4000_0002,
        0.2,
        90.0,
        opacity_on_key1,
        0.1,
        0.25,
        [0x20, 0x40, 0x80, 0xFF],
    );
    assert_eq!(b.len(), 210, "§14: the record is 8 + 2 · 101 bytes");
    TerrainTypeTable::parse_expecting(&b, TABLE).expect("§14 table decodes")
}

fn v(i: usize, j: usize) -> u16 {
    (GRID_SIDE * j + i) as u16
}

/// Landblock (3, 7) of region 1: key 0 `whole` without mask; key 1 with 10 triangles
/// (both triangles of quads (0, 0) … (4, 0)) and a 128 × 128 mask.
fn block() -> LandblockData {
    let mut data = LandblockData::from_heights(Did(0x8001_0307), HeightGrid::flat(1000));
    let mut indices = Vec::new();
    for i in 0..5 {
        // Every quad split south-east to north-west (vertex flags 0).
        indices.extend([v(i, 0), v(i + 1, 0), v(i, 1)]);
        indices.extend([v(i + 1, 0), v(i + 1, 1), v(i, 1)]);
    }
    data.terrain = SurfaceChunk {
        layers: vec![
            SurfaceLayer {
                material: 0,
                whole: true,
                ..SurfaceLayer::default()
            },
            SurfaceLayer {
                material: 1,
                mask: Did(0x4100_0123),
                whole: false,
                start: 0,
                prims: 10,
                min_vertex: 0,
                vertex_span: 39,
            },
        ],
        indices,
        quads: None,
    };
    data
}

#[test]
fn section_14_table() {
    let t = table(true);
    assert_eq!(t.len(), 2);
    let e1 = t.get(1).unwrap();
    let [r, g, b, a] = e1.diffuse_scale.to_rgba_f32();
    assert!((r - 0.502).abs() < 5e-4 && (g - 0.251).abs() < 5e-4 && (b - 0.125).abs() < 5e-4);
    assert_eq!(a, 1.0);
}

#[test]
fn section_14_point() {
    let data = block();
    let t = table(true);
    let layers = resolve_landblock(
        &data,
        GridSurface::Terrain,
        &t,
        &LightMaps::default(),
        &ResolveOptions::default(),
    )
    .unwrap();
    assert_eq!(layers.layers().len(), 2);
    let (base, top) = (&layers.layers()[0], &layers.layers()[1]);

    // Offset (§6.2), fixed-region-coordinate simplification: (480 + 12.5, 1,120 + 3.0).
    let offset = fixed_block_offset(3, 7);
    let (x, y) = (12.5f32, 3.0f32);
    let pq = [x + offset[0], y + offset[1]];
    assert_eq!(pq, [492.5, 1123.0]);
    // The vertex streams give the same set-0 values at the grid samples.
    let tb = TerrainBlock::from_landblock(&data);
    let streams =
        LayerVertices::build(&tb, &VertexNormals::compute(&tb, &Neighbours::NONE), offset);
    assert_eq!(
        streams.set(CoordinateSet::Xy)[usize::from(v(2, 0))],
        [490.0, 1120.0]
    );
    // With the anchor at (2, 7) the point is (172.5, 3.0).
    let mut frame = TextureFrame::new();
    frame.set_reference(1, 2, 7);
    let o2 = frame.block_offset(1, 3, 7).unwrap();
    assert_eq!([x + o2[0], y + o2[1]], [172.5, 3.0]);

    // Key 0: first drawn, record XY, colour role Color, no overlay, opaque.
    assert!(base.first_drawn() && base.whole());
    assert_eq!(base.coordinate_set(), CoordinateSet::Xy);
    let f0 = base.features();
    assert!(!f0.overlay && !f0.opacity);
    assert_eq!(f0.colour_alpha(), ColourAlphaRole::Unused);
    assert_eq!(base.splat().opacity, OpacityRule::Opaque);
    let p0 = base.params();
    let uv0 = p0.transform.apply(pq, p0.offsets.constant().unwrap());
    assert_eq!(uv0, [49.25, 112.3]);

    // Key 1: overlay, opacity on, alpha test on (0.1 > 0.0002).
    let f1 = top.features();
    assert!(f1.overlay && f1.opacity);
    assert_eq!(f1.colour_alpha(), ColourAlphaRole::Opacity);
    let s1 = top.splat();
    assert!(s1.alpha_test());
    assert_eq!(s1.alpha_test_ref, 0.1);
    assert_eq!(top.textures().mask, Some(Did(0x4100_0123)));
    let p1 = top.params();
    let uv1 = p1.transform.apply(pq, p1.offsets.constant().unwrap());
    assert_eq!(uv1, [-224.6, 98.5]);

    // Mask texel: column 10, row 2 from the south edge. Suppose m = 0.8 there and the
    // diffuse alpha is 0.5.
    assert_eq!(mask_texel(x, y, 128, 128), Some((10, 2)));
    let mut texels = vec![0u8; 128 * 128];
    texels[2 * 128 + 10] = 204;
    let mask = AlphaMask::from_texels(Did(0x4100_0123), 128, 128, texels).unwrap();
    let m = mask.value_at(x, y).unwrap();
    assert!((m - 0.8).abs() < 1e-7);
    let alpha = s1.alpha(m, 0.5).expect("kept: 0.56 > 0.1");
    assert!((alpha - 0.56).abs() < 1e-6);

    // Result: 0.56 · C₁ + 0.44 · C₀.
    let (c0, c1) = ([0.2f32, 0.4, 0.6], [1.0f32, 0.5, 0.0]);
    let key = layers.triangle_at(x, y).unwrap();
    assert!(top.coverage().contains(key) && base.coverage().contains(key));
    let out = layers
        .composite(x, y, |l| {
            if l.key() == 0 {
                (c0, 0.0, 1.0)
            } else {
                (c1, mask.value_at(x, y).unwrap(), 0.5)
            }
        })
        .unwrap();
    for k in 0..3 {
        assert!(
            (out[k] - (0.56 * c1[k] + 0.44 * c0[k])).abs() < 1e-6,
            "{out:?}"
        );
    }
    // Outside key 1's triangles only the base shows.
    let base_only = layers.composite(100.0, 100.0, |l| {
        if l.key() == 0 {
            (c0, 0.0, 1.0)
        } else {
            (c1, 1.0, 1.0)
        }
    });
    assert_eq!(base_only, Some(c0));

    // Had key 1 `use_opacity_mask` = 0, α would be m = 0.8 and `alpha_blend_max` would have
    // no effect.
    let t0 = table(false);
    let alt = resolve_landblock(
        &data,
        GridSurface::Terrain,
        &t0,
        &LightMaps::default(),
        &ResolveOptions::default(),
    )
    .unwrap();
    let s = alt.layers()[1].splat();
    assert_eq!(s.opacity, OpacityRule::Mask);
    assert_eq!(s.alpha(m, 0.5), Some(m));
    assert!(!alt.layers()[1].features().opacity);
}
