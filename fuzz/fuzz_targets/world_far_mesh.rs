// SPDX-License-Identifier: GPL-3.0-or-later
//! The far-view mesh builder of `tessera-world` (`docs/specs/far-view.md` §4–§5,
//! `distant-landscape.md` §5.3) and the sprite placement (§9) on arbitrary input.
//!
//! The first byte picks the mode:
//!
//! - odd: the rest is a sector record payload; when it decodes, the per-landblock grid is
//!   built around a viewer taken from the next bytes and every batch (far and near,
//!   terrain and ceiling) is built from the grid's descriptors;
//! - even: a synthetic mesh block whose samples, chunk windows, `block_size` and nine
//!   descriptors (presence, near/far, ceiling, LOD 0–7, edge stitch flags and neighbour
//!   LODs) all come from the input, so the builder sees descriptors no grid produces.
//!
//! Nothing may panic. A built batch has every index below its vertex count, a whole number
//! of triangles and its stitching triangles after the grid ones.

#![no_main]

use libfuzzer_sys::fuzz_target;
use tessera_formats::landblock::{SectorChunk, SectorMesh, SectorSample, SectorSprite};
use tessera_formats::{Did, ImposterEntry, ImposterTable, PropertyType, SectorRecord, TSize};
use tessera_world::LandblockKey;
use tessera_world::far_view::{
    Batch, Edge, FarMesh, LandblockDescriptor, LandblockGrid, MeshKind, NearField, SectorKey,
    SpriteView, build_batch, place_sprites, sector_list,
};

fn total(pid: u32) -> Option<PropertyType> {
    Some(PropertyType::from_code(pid % 24))
}

fn check(m: &FarMesh) {
    assert_eq!(m.indices.len() % 3, 0);
    assert_eq!(m.stitch_start % 3, 0);
    assert!(m.stitch_start <= m.indices.len());
    let n = m.positions.len();
    assert_eq!(m.normals.len(), n);
    assert_eq!(m.uvs.len(), n);
    assert!(m.indices.iter().all(|i| (*i as usize) < n));
}

struct Bytes<'a>(&'a [u8]);

impl Bytes<'_> {
    fn u8(&mut self) -> u8 {
        match self.0.split_first() {
            Some((b, rest)) => {
                self.0 = rest;
                *b
            }
            None => 0,
        }
    }
    fn u16(&mut self) -> u16 {
        u16::from_le_bytes([self.u8(), self.u8()])
    }
    fn f32(&mut self) -> f32 {
        f32::from_le_bytes([self.u8(), self.u8(), self.u8(), self.u8()])
    }
}

fn record_mode(rest: &[u8]) {
    let Some((viewer, payload)) = rest.split_at_checked(3) else {
        return;
    };
    let Ok(rec) = SectorRecord::parse(payload, &total) else {
        return;
    };
    let Some(key) = rec
        .key()
        .and_then(|(sx, sy)| SectorKey::new(rec.region().unwrap_or(1).max(1), sx, sy))
    else {
        return;
    };
    let Ok(v) = LandblockKey::new(key.region(), viewer[0] % 255, viewer[1] % 255) else {
        return;
    };
    let list = sector_list(v, u32::from(viewer[2] % 4) + 1);
    let near = NearField::Radius(u32::from(viewer[2] >> 4) % 4);
    let grid = LandblockGrid::build(v, &list, |k| k == key, &near);
    let d = grid.descriptors(key, rec.ceiling_mask);
    for batch in [Batch::Far, Batch::Near] {
        if let Ok(m) = build_batch(
            rec.did,
            &rec.terrain,
            rec.block_size,
            &d,
            MeshKind::Terrain,
            batch,
        ) {
            check(&m);
        }
        if let Some(c) = &rec.ceiling
            && let Ok(m) = build_batch(rec.did, c, rec.block_size, &d, MeshKind::Ceiling, batch)
        {
            check(&m);
        }
    }
    // Sprites against a small table built from the record's own keys.
    let table = ImposterTable {
        did: rec.imposter_table,
        size: TSize { hint: 1, count: 1 },
        entries: rec
            .sprites
            .iter()
            .take(4)
            .enumerate()
            .map(|(i, s)| ImposterEntry {
                name_key: u32::from(s.name_key),
                column: u8::try_from(i).unwrap_or(0),
                row: 0,
                value_a: 1.0 + i as f32,
                value_b: -0.1,
            })
            .collect(),
        colour_atlas: Did(0),
        second_atlas: Did(0),
        cell_u: 0.25,
        cell_v: 0.5,
    };
    let mut out = Vec::new();
    let _ = place_sprites(
        &rec.sprites,
        &table,
        [-160.0, 320.0],
        &SpriteView::default(),
        &mut out,
    );
    for s in &out {
        assert!(s.size.is_finite() && s.size > 0.0);
    }
}

fn synthetic_mode(rest: &[u8]) {
    let mut b = Bytes(rest);
    let block_size = match b.u8() % 4 {
        0 => 160.0,
        1 => b.f32(),
        2 => 1.0 + f32::from(b.u8()),
        _ => -1.0,
    };
    let blocks = usize::from(b.u8() % 10);
    let mut mesh = SectorMesh::default();
    for _ in 0..blocks * 1089 {
        let h = b.u16();
        mesh.samples.push(SectorSample {
            height: h,
            normal: [h as u8, (h >> 8) as u8, b.u8()],
        });
    }
    for c in 0..9 {
        let (x, y) = (c % 3, c / 3);
        let rank = u32::from(b.u8() % 10);
        let mut chunk = SectorChunk::expected(rank, x, y, 0.0, 1.0);
        if b.u8() % 7 == 0 {
            chunk.vertex_count = u32::from(b.u16());
            chunk.vertex_start = u32::from(b.u16());
        }
        mesh.chunks[c] = chunk;
    }
    let mut d = [LandblockDescriptor::default(); 9];
    for desc in &mut d {
        let flags = b.u8();
        let mut edges = [Edge::default(); 4];
        for e in &mut edges {
            let v = b.u8();
            *e = Edge {
                stitch: v & 1 != 0,
                neighbour_lod: v >> 1,
            };
        }
        *desc = LandblockDescriptor {
            present: flags & 1 != 0,
            far: flags & 2 != 0,
            ceiling: flags & 4 != 0,
            lod: flags >> 4,
            edges,
        };
    }
    for kind in [MeshKind::Terrain, MeshKind::Ceiling] {
        for batch in [Batch::Far, Batch::Near] {
            if let Ok(m) = build_batch(Did(0x8051_0000), &mesh, block_size, &d, kind, batch) {
                check(&m);
            }
        }
    }
    // Sprites with hostile floats.
    let sprites: Vec<SectorSprite> = (0..usize::from(b.u8() % 8))
        .map(|_| SectorSprite {
            name_key: b.u16() % 4,
            east: b.f32(),
            height: b.f32(),
            north: b.f32(),
            scale: b.f32(),
            distance: b.f32(),
            byte: b.u8(),
        })
        .collect();
    let table = ImposterTable {
        did: Did(0x2701_0000),
        size: TSize { hint: 1, count: 2 },
        entries: vec![
            ImposterEntry {
                name_key: 1,
                column: b.u8(),
                row: b.u8(),
                value_a: b.f32(),
                value_b: b.f32(),
            },
            ImposterEntry {
                name_key: 2,
                column: 1,
                row: 0,
                value_a: 3.0,
                value_b: 0.0,
            },
        ],
        colour_atlas: Did(0),
        second_atlas: Did(0),
        cell_u: b.f32(),
        cell_v: 0.25,
    };
    let view = SpriteView {
        target_height: b.f32(),
        fov_y: b.f32(),
        object_scale: b.f32(),
    };
    let mut out = Vec::new();
    let _ = place_sprites(&sprites, &table, [b.f32(), b.f32()], &view, &mut out);
    for s in &out {
        assert!(s.size.is_finite() && s.size > 0.0);
        assert!(s.position.iter().all(|v| v.is_finite()));
    }
}

fuzz_target!(|data: &[u8]| {
    let Some((&mode, rest)) = data.split_first() else {
        return;
    };
    if mode & 1 == 1 {
        record_mode(rest);
    } else {
        synthetic_mode(rest);
    }
});
