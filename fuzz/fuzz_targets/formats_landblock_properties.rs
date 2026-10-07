// SPDX-License-Identifier: GPL-3.0-or-later
//! Landblock properties and kind-5 distant-landscape sector records
//! (`docs/specs/landblock.md` §6, §7) on arbitrary bytes, with a total synthetic schema and
//! synthetic PIDs for the minimap and light-map helpers.
//!
//! Nothing may panic. A decoded minimap tile has a positive finite scale, so both pixel
//! mappings are finite. Accepted group records frame their samples exactly; the sector
//! helpers (landblocks, water-pair search) work on them.
//!
//! Run with a memory cap so that allocation blow-ups count as failures, e.g.
//! `cargo fuzz run formats_landblock_properties -- -malloc_limit_mb=512 -rss_limit_mb=1024`.

#![no_main]

use libfuzzer_sys::fuzz_target;
use tessera_formats::landblock::{GridSurface, LandblockPids, SectorWaterPair};
use tessera_formats::{PropertyType, decode_landblock_group, decode_landblock_properties};

fn total(pid: u32) -> Option<PropertyType> {
    Some(PropertyType::from_code(pid % 24))
}

fuzz_target!(|data: &[u8]| {
    if let Ok(p) = decode_landblock_properties(data, &total) {
        // Small synthetic PIDs with the types the helpers expect under `total`:
        // 11 struct, 12 array, 20 DID, 15 int, 9 float, 16 colour.
        let pids = LandblockPids::from_names(|name| {
            Some(match name {
                "UI_Map_GameMap" => 11,
                "UI_Map_MapImageArray" => 12,
                "UI_Map_MapImage" | "Landblock_LightMap" | "Landblock_CeilingLightMap" => 20,
                "UI_Map_Scale" => 9,
                "UI_Map_FogOfWar_Color" => 16,
                _ => 15,
            })
        });
        if let Some(t) = p.minimap(&pids) {
            // A decoded tile always has a usable scale, so both directions are finite.
            assert!(t.scale.is_finite() && t.scale > 0.0);
            let g = t
                .pixel_to_region(50.0, 100.0)
                .expect("a decoded tile maps pixels");
            assert!(g.iter().all(|v| v.is_finite()));
            assert!(t.region_to_pixel(g).iter().all(|v| v.is_finite()));
            assert!(
                t.region_to_pixel([0.0, 40_800.0])
                    .iter()
                    .all(|v| v.is_finite())
            );
            let _ = t.image_for(|l| l == 1);
        }
        let l = p.light_maps(&pids);
        assert_eq!(l.terrain, p.light_map(&pids));
        assert_eq!(l.ceiling, p.ceiling_light_map(&pids));
        assert_eq!(l.for_surface(GridSurface::Ceiling), l.ceiling);
    }
    if let Ok(g) = decode_landblock_group(data) {
        assert_eq!(4 + 4 + g.samples.len() + g.tail.len(), data.len());
        assert_eq!(g.samples.len() % 5, 0);
        let n = g.sample_count();
        for i in [0, n.saturating_sub(1), n, usize::MAX / 5, usize::MAX] {
            assert_eq!(g.sample(i).is_some(), i < g.sample_count());
        }
        let _ = g.first_unusual_field();
        assert!(g.sector_landblocks().len() <= 9);
        // A pair taken from the tail is found; the search never reads past it.
        if let Some(b) = g.tail.get(..5) {
            let pair = SectorWaterPair {
                water_type: b[0],
                height: f32::from_le_bytes([b[1], b[2], b[3], b[4]]),
            };
            assert!(g.tail_contains_water_pair(pair));
        }
    }
});
