// SPDX-License-Identifier: GPL-3.0-or-later
//! Landblock properties and kind-5 group records (`docs/specs/landblock.md` §6, §7) on
//! arbitrary bytes, with a total synthetic schema and synthetic PIDs for the minimap
//! helpers.
//!
//! Nothing may panic. Accepted group records frame their samples exactly.
//!
//! Run with a memory cap so that allocation blow-ups count as failures, e.g.
//! `cargo fuzz run formats_landblock_properties -- -malloc_limit_mb=512 -rss_limit_mb=1024`.

#![no_main]

use libfuzzer_sys::fuzz_target;
use tessera_formats::landblock::LandblockPids;
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
            if let Some(g) = t.pixel_to_region(50.0, 100.0) {
                let _ = t.region_to_pixel(g);
            }
            let _ = t.image_for(|l| l == 1);
        }
        let _ = (p.light_map(&pids), p.ceiling_light_map(&pids));
    }
    if let Ok(g) = decode_landblock_group(data) {
        assert_eq!(4 + 4 + g.samples.len() + g.tail.len(), data.len());
        assert_eq!(g.samples.len() % 5, 0);
        let n = g.sample_count();
        for i in [0, n.saturating_sub(1), n, usize::MAX / 5, usize::MAX] {
            assert_eq!(g.sample(i).is_some(), i < g.sample_count());
        }
        let _ = g.first_unusual_field();
    }
});
