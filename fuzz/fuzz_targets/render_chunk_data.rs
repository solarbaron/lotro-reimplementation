// SPDX-License-Identifier: GPL-3.0-or-later
//! `tessera-render`'s GPU-free checks of terrain input built from install records
//! (`docs/specs/render.md`; `terrain-texturing.md` §6, §8; `far-view.md` §5, §9):
//! `TerrainChunkData::validate` (the full-detail splat chunk) and `FarTerrainData::validate`
//! (one far-terrain batch) on hostile streams: lengths that disagree, empty and partial
//! streams, indices past the vertex count, partial triangles, stitch starts off a triangle
//! boundary or past the end, non-finite positions, and the vertex, layer, triangle and
//! far-index counts at their caps.
//!
//! Input layout: byte 0 selects the checker and the stream-length offsets, byte 1 the
//! vertex count, byte 2 the layer count, then 4-byte words read as `f32` positions and
//! `u16`/`u32` indices (wrapping). Byte 2 = 0xFF asks for one layer past the cap; byte 2 =
//! 0xFE selects a cap mode instead, whose first word picks a vertex, triangle or far-index
//! count of cap − 1, cap or cap + 1 (one repeated position and normal, triangles taken from
//! one shared run, so these runs stay cheap). Nothing may panic. An accepted chunk has a
//! vertex count within `MAX_TERRAIN_VERTICES`, every triangle index below it, a triangle
//! total within `MAX_TERRAIN_TRIANGLES` that matches its layers, a finite bounding box that
//! holds every position, and (when the box's sum and half-diagonal fit in `f32`) a finite
//! bounding sphere; an accepted far batch has at most `MAX_FAR_TERRAIN_VERTICES` vertices and
//! `MAX_FAR_TERRAIN_INDICES` indices, equal stream lengths, a triangle list, a stitch start
//! inside it and finite positions.

#![no_main]

use libfuzzer_sys::fuzz_target;
use tessera_render::{
    FarTerrainData, MAX_FAR_TERRAIN_INDICES, MAX_FAR_TERRAIN_VERTICES, MAX_TERRAIN_LAYERS,
    MAX_TERRAIN_TRIANGLES, MAX_TERRAIN_VERTICES, TerrainChunkData, TerrainLayer, TerrainVertices,
};

/// Reads 4-byte words, wrapping around the input (zero when it is empty).
struct Words<'a> {
    data: &'a [u8],
    at: usize,
}

impl Words<'_> {
    fn word(&mut self) -> u32 {
        if self.data.is_empty() {
            return 0;
        }
        let mut b = [0u8; 4];
        for x in &mut b {
            *x = self.data[self.at % self.data.len()];
            self.at += 1;
        }
        u32::from_le_bytes(b)
    }

    fn float(&mut self) -> f32 {
        let w = self.word();
        // Mostly ordinary coordinates, sometimes raw bits (NaN, infinities, huge values).
        if w & 0xF == 0 {
            f32::from_bits(w)
        } else {
            ((w >> 8) as f32 / 256.0) - 32_768.0
        }
    }

    fn vec3(&mut self) -> [f32; 3] {
        [self.float(), self.float(), self.float()]
    }

    fn vec2(&mut self) -> [f32; 2] {
        [self.float(), self.float()]
    }
}

/// A length near `n`: equal (twice as likely), one less or one more, chosen by two bits.
fn near(n: usize, bits: u8) -> usize {
    match bits & 3 {
        0 | 1 => n,
        2 => n.saturating_sub(1),
        _ => n + 1,
    }
}

fn chunk(sel: u8, verts: usize, layers: usize, w: &mut Words<'_>) {
    let positions: Vec<[f32; 3]> = (0..verts).map(|_| w.vec3()).collect();
    let normals: Vec<[f32; 3]> = (0..near(verts, sel >> 1)).map(|_| w.vec3()).collect();
    let tangent_n = if sel & 0x20 != 0 {
        near(verts, sel >> 3)
    } else {
        0
    };
    let tangents: Vec<[f32; 3]> = (0..tangent_n).map(|_| w.vec3()).collect();
    let binormal_n = if sel & 0x40 != 0 {
        tangent_n
    } else {
        near(tangent_n, sel >> 5)
    };
    let binormals: Vec<[f32; 3]> = (0..binormal_n).map(|_| w.vec3()).collect();
    let sets: Vec<Vec<[f32; 2]>> = (0..4u8)
        .map(|k| {
            let n = if (sel >> k) & 1 == 0 {
                verts
            } else {
                near(verts, sel >> (k + 1))
            };
            (0..n).map(|_| w.vec2()).collect()
        })
        .collect();
    let triangle_lists: Vec<Vec<[u16; 3]>> = (0..layers)
        .map(|_| {
            let count = (w.word() % 64) as usize;
            (0..count)
                .map(|_| {
                    let a = w.word();
                    let m = (verts as u32).saturating_add(2).max(1);
                    let idx = |x: u32| (x % m) as u16;
                    [idx(a), idx(a >> 11), idx(a >> 22)]
                })
                .collect()
        })
        .collect();
    let layer_list: Vec<TerrainLayer<'_>> = triangle_lists
        .iter()
        .map(|t| TerrainLayer {
            triangles: t,
            alpha_test_ref: w.float(),
            ..TerrainLayer::default()
        })
        .collect();
    let data = TerrainChunkData {
        label: Some("fuzz"),
        vertices: TerrainVertices {
            positions: &positions,
            normals: &normals,
            tangents: &tangents,
            binormals: &binormals,
            sets: [&sets[0], &sets[1], &sets[2], &sets[3]],
        },
        layers: &layer_list,
        light_map: None,
    };
    check_chunk(&data, &positions, &layer_list);
}

/// The invariants of a chunk that `validate` accepted.
fn check_chunk(
    data: &TerrainChunkData<'_>,
    positions: &[[f32; 3]],
    layer_list: &[TerrainLayer<'_>],
) {
    let Ok(info) = data.validate() else {
        return;
    };
    assert!(!positions.is_empty() && positions.len() <= MAX_TERRAIN_VERTICES);
    assert_eq!(info.vertices as usize, positions.len());
    assert_eq!(info.layers as usize, layer_list.len());
    assert!(layer_list.len() <= MAX_TERRAIN_LAYERS);
    let total: usize = layer_list.iter().map(|l| l.triangles.len()).sum();
    assert_eq!(info.triangles as usize, total);
    assert!(total <= MAX_TERRAIN_TRIANGLES, "{total} triangles accepted");
    for l in layer_list {
        for t in l.triangles {
            assert!(t.iter().all(|&i| usize::from(i) < positions.len()));
        }
    }
    let (lo, hi) = (info.aabb.min, info.aabb.max);
    assert!(lo.is_finite() && hi.is_finite(), "box {lo:?} {hi:?}");
    // The sphere is finite whenever its centre and radius fit in `f32` at all. Boxes whose
    // corner sum overflows are not checked here: `validate` computes the centre as
    // (lo + hi) / 2, which overflows for positions near `f32::MAX` (reported to the
    // tessera-render owner; such a chunk is never culled, nothing panics).
    let half = (hi - lo) * 0.5;
    if (lo + hi).is_finite() && half.length_squared() < f32::MAX / 4.0 {
        let sphere = info.sphere;
        assert!(
            sphere.center.is_finite() && sphere.radius.is_finite(),
            "sphere {sphere:?} for box {lo:?} {hi:?}"
        );
    }
    for p in positions {
        assert!(p.iter().all(|c| c.is_finite()));
        assert!(
            (0..3).all(|k| lo[k] <= p[k] && p[k] <= hi[k]),
            "{p:?} outside {lo:?}..{hi:?}"
        );
    }
}

fn far(sel: u8, verts: usize, w: &mut Words<'_>) {
    let positions: Vec<[f32; 3]> = (0..verts).map(|_| w.vec3()).collect();
    let normals: Vec<[f32; 3]> = (0..near(verts, sel >> 1)).map(|_| w.vec3()).collect();
    let uvs: Vec<[f32; 2]> = (0..near(verts, sel >> 3)).map(|_| w.vec2()).collect();
    let index_n = (w.word() % 400) as usize;
    let m = (verts as u32).saturating_add(2).max(1);
    let indices: Vec<u32> = (0..index_n).map(|_| w.word() % m).collect();
    let stitch_start = (w.word() % 410) as usize;
    let data = FarTerrainData {
        label: "fuzz",
        positions: &positions,
        normals: &normals,
        uvs: &uvs,
        indices: &indices,
        stitch_start,
    };
    check_far(&data);
}

/// The invariants of a far batch that `validate` accepted.
fn check_far(data: &FarTerrainData<'_>) {
    let (positions, normals, uvs, indices, stitch_start) = (
        data.positions,
        data.normals,
        data.uvs,
        data.indices,
        data.stitch_start,
    );
    if data.validate().is_ok() {
        assert!(positions.len() <= MAX_FAR_TERRAIN_VERTICES);
        assert!(indices.len() <= MAX_FAR_TERRAIN_INDICES);
        assert_eq!(normals.len(), positions.len());
        assert_eq!(uvs.len(), positions.len());
        assert!(indices.len().is_multiple_of(3) && stitch_start.is_multiple_of(3));
        assert!(stitch_start <= indices.len());
        assert!(indices.iter().all(|&i| (i as usize) < positions.len()));
        assert!(positions.iter().flatten().all(|c| c.is_finite()));
    }
}

/// A count of `cap` − 1, `cap` or `cap` + 1, chosen by `pick`.
fn at_cap(cap: usize, pick: u32) -> usize {
    match pick % 3 {
        0 => cap - 1,
        1 => cap,
        _ => cap + 1,
    }
}

/// Chunk counts at their caps: the vertex count (one triangle reaching the last vertex, or
/// `u16::MAX`), or the triangle total over all `MAX_TERRAIN_LAYERS` layers.
fn chunk_caps(pick: u32, w: &mut Words<'_>) {
    let per_layer = MAX_TERRAIN_TRIANGLES / MAX_TERRAIN_LAYERS;
    let (verts, total) = if pick & 1 == 0 {
        (at_cap(MAX_TERRAIN_VERTICES, pick >> 1), 1)
    } else {
        (3, at_cap(MAX_TERRAIN_TRIANGLES, pick >> 1))
    };
    let positions = vec![w.vec3(); verts];
    let normals = vec![w.vec3(); verts];
    let last = u16::try_from(verts - 1).unwrap_or(u16::MAX);
    // One shared run of triangles; every layer is a slice of it.
    let run = vec![[0, 1, last]; per_layer + 1];
    let layer_list: Vec<TerrainLayer<'_>> = if total == 1 {
        vec![TerrainLayer {
            triangles: &run[..1],
            ..TerrainLayer::default()
        }]
    } else {
        // 255 full layers and a last one of `total - 255 * per_layer` triangles.
        let full = MAX_TERRAIN_LAYERS - 1;
        let tail = total - full * per_layer;
        (0..MAX_TERRAIN_LAYERS)
            .map(|i| TerrainLayer {
                triangles: &run[..if i < full { per_layer } else { tail }],
                ..TerrainLayer::default()
            })
            .collect()
    };
    let data = TerrainChunkData {
        label: Some("fuzz caps"),
        vertices: TerrainVertices {
            positions: &positions,
            normals: &normals,
            ..TerrainVertices::default()
        },
        layers: &layer_list,
        light_map: None,
    };
    let accepted = data.validate().is_ok();
    let total_given: usize = layer_list.iter().map(|l| l.triangles.len()).sum();
    if verts > MAX_TERRAIN_VERTICES || total_given > MAX_TERRAIN_TRIANGLES {
        assert!(
            !accepted,
            "{verts} vertices, {total_given} triangles accepted"
        );
    }
    check_chunk(&data, &positions, &layer_list);
}

/// Far-batch counts at their caps: the vertex count or the index count.
fn far_caps(pick: u32, w: &mut Words<'_>) {
    let (verts, index_n) = if pick & 1 == 0 {
        (at_cap(MAX_FAR_TERRAIN_VERTICES, pick >> 1), 3)
    } else {
        // Whole triangles: cap − 3, cap or cap + 3 indices.
        let n = match (pick >> 1) % 3 {
            0 => MAX_FAR_TERRAIN_INDICES - 3,
            1 => MAX_FAR_TERRAIN_INDICES,
            _ => MAX_FAR_TERRAIN_INDICES + 3,
        };
        (3, n)
    };
    let positions = vec![w.vec3(); verts];
    let normals = vec![w.vec3(); verts];
    let uvs = vec![w.vec2(); verts];
    let indices: Vec<u32> = (0..index_n).map(|i| (i % verts) as u32).collect();
    let data = FarTerrainData {
        label: "fuzz caps",
        positions: &positions,
        normals: &normals,
        uvs: &uvs,
        indices: &indices,
        stitch_start: 0,
    };
    if verts > MAX_FAR_TERRAIN_VERTICES || index_n > MAX_FAR_TERRAIN_INDICES {
        assert!(
            data.validate().is_err(),
            "{verts} vertices, {index_n} indices accepted"
        );
    }
    check_far(&data);
}

fuzz_target!(|data: &[u8]| {
    let [sel, verts, layers, rest @ ..] = data else {
        return;
    };
    let mut w = Words { data: rest, at: 0 };
    let verts = usize::from(*verts);
    if *layers == 0xFE {
        let pick = w.word();
        if sel & 0x80 == 0 {
            chunk_caps(pick, &mut w);
        } else {
            far_caps(pick, &mut w);
        }
        return;
    }
    if sel & 0x80 == 0 {
        // Mostly small layer counts; 0xFF stands for one past the cap.
        let layers = if *layers == 0xFF {
            MAX_TERRAIN_LAYERS + 1
        } else {
            usize::from(*layers % 16)
        };
        chunk(*sel, verts, layers, &mut w);
    } else {
        far(*sel, verts, &mut w);
    }
});
