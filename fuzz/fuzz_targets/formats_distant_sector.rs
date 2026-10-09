// SPDX-License-Identifier: GPL-3.0-or-later
//! Distant-landscape sector records (`docs/specs/distant-landscape.md` §3) on arbitrary
//! bytes, with a total synthetic schema.
//!
//! The first input byte picks the mode: odd = the rest is a whole payload; even = the rest
//! follows a valid synthetic DID and terrain mesh block (49,635 bytes), so runs reach the
//! mode byte, the ceiling, the water list, sprites, entities and the DID tail.
//!
//! Nothing may panic. An accepted record follows the decoder rules of §3.11 (sample counts,
//! chunk windows, water indices, mode and mask); the accessors and the invariant report run
//! on it, as do the sprite rules of §4.8 and §5.6, the far-placement registration of §4.9
//! and the normal check of §4.3 (without landblock records); re-encoding (when every
//! entity property is null) is stable.
//!
//! Run with a memory cap so that allocation blow-ups count as failures, e.g.
//! `cargo fuzz run formats_distant_sector -- -malloc_limit_mb=512 -rss_limit_mb=1024`.

#![no_main]

use std::sync::OnceLock;

use libfuzzer_sys::fuzz_target;
use tessera_formats::landblock::{
    ALL_LANDBLOCKS_MASK, FarPlacements, SECTOR_BLOCK_SAMPLES, SECTOR_TERRAIN_SAMPLES, SectorChunk,
    SectorMesh, SectorRecord, SectorSample, chunk_index, sprite_fade_parameter, sprite_is_mirrored,
    vertex_block,
};
use tessera_formats::{Did, PropertyType};

fn total(pid: u32) -> Option<PropertyType> {
    Some(PropertyType::from_code(pid % 24))
}

/// DID and terrain mesh block of a valid synthetic sector (region 1, X 3, Y 6).
fn prefix() -> &'static [u8] {
    static P: OnceLock<Vec<u8>> = OnceLock::new();
    P.get_or_init(|| {
        let mut terrain = SectorMesh::default();
        for k in 0..9u16 {
            for s in 0..1089u16 {
                terrain.samples.push(SectorSample {
                    height: k * 1000 + s,
                    normal: [127, 255, 127],
                });
            }
        }
        for x in 0..3 {
            for y in 0..3 {
                let k = vertex_block(x, y) as u32;
                terrain.chunks[chunk_index(x, y)] = SectorChunk::expected(k, x, y, 0.0, 1.0);
            }
        }
        let rec = SectorRecord {
            did: Did(0x8051_0306),
            terrain,
            mode: 0,
            ceiling_mask: 0,
            ceiling: None,
            block_size: 160.0,
            water: Vec::new(),
            sprites: Vec::new(),
            entities: Vec::new(),
            colour_list: Did(0),
            ceiling_colour_list: Did(0),
            tile: Did(0),
            ceiling_tile: Did(0),
            flow_map: Did(0),
            imposter_table: Did(0),
        };
        let mut bytes = rec.to_bytes().expect("synthetic sector encodes");
        // Keep the DID and the terrain mesh block only: everything up to the mode byte.
        bytes.truncate(49_635);
        bytes
    })
}

fn check(rec: &SectorRecord) {
    assert_eq!(rec.terrain.samples.len(), SECTOR_TERRAIN_SAMPLES as usize);
    assert!(rec.mode <= 2);
    assert_eq!(rec.ceiling.is_some(), rec.mode != 0);
    assert_eq!(rec.ceiling_mask & !ALL_LANDBLOCKS_MASK, 0);
    if rec.mode == 1 {
        assert_eq!(rec.ceiling_mask, ALL_LANDBLOCKS_MASK);
    }
    for mesh in std::iter::once(&rec.terrain).chain(&rec.ceiling) {
        for c in &mesh.chunks {
            assert!(c.vertex_count == 0 || c.vertex_count == SECTOR_BLOCK_SAMPLES);
            if c.is_present() {
                assert!(mesh.chunk_samples(c).is_some());
            }
            if c.wet {
                assert!(usize::from(c.water_index) < rec.water.len());
            }
        }
        for x in 0..4 {
            for y in 0..4 {
                let _ = mesh.block(x, y);
                let _ = mesh.sample(x, y, 32, 32);
            }
        }
    }
    if let Some(c) = &rec.ceiling {
        assert_eq!(
            c.samples.len(),
            SECTOR_BLOCK_SAMPLES as usize * rec.ceiling_mask.count_ones() as usize
        );
    }
    for x in 0..4 {
        for y in 0..4 {
            let _ = rec.water_at(x, y);
            let _ = rec.has_ceiling(x, y);
        }
    }
    for s in &rec.terrain.samples {
        let _ = (s.height_units(), s.normal_xyz());
    }
    for e in &rec.entities {
        let _ = (e.kind(), e.view_distance());
    }
    for sp in rec.sprites.iter().take(64) {
        let h = sp.heading_degrees();
        assert!(h.is_nan() || (0.0..360.0).contains(&h));
        let (size, _) = sp.billboard_size(1.0, 0.1);
        let at = sp.viewer_frame(rec.origin().unwrap_or((0, 0)), (1, 1));
        let _ = sprite_fade_parameter(sp, at, size, 1080.0, 1.0, 1.0);
        let _ = sprite_is_mirrored(sp, at);
    }
    let mut placements = FarPlacements::new();
    let added = placements.register(rec);
    assert_eq!(
        placements.register(rec),
        0,
        "a second registration adds nothing"
    );
    assert_eq!(placements.len(), added);
    let check = rec.check_normals(|_, _| None);
    assert_eq!(check.terrain.samples, check.terrain.uncomputable);
    assert_eq!(check.ceiling.samples, check.ceiling.uncomputable);
    let _ = (rec.key(), rec.region(), rec.deviations());
    let _ = rec.water_list_follows_rules();
    if rec.entities.iter().all(|e| e.property.is_none()) {
        let a = rec.to_bytes().expect("null properties encode");
        let back = SectorRecord::parse(&a, &total).expect("re-encoded record decodes");
        let b = back.to_bytes().expect("null properties encode");
        assert_eq!(a, b, "re-encoding is not stable");
    }
}

fuzz_target!(|data: &[u8]| {
    let Some((&mode, rest)) = data.split_first() else {
        return;
    };
    if mode & 1 == 1 {
        if let Ok(rec) = SectorRecord::parse(rest, &total) {
            check(&rec);
        }
    } else {
        let mut payload = prefix().to_vec();
        payload.extend_from_slice(rest);
        if let Ok(rec) = SectorRecord::parse(&payload, &total) {
            check(&rec);
        }
    }
});
