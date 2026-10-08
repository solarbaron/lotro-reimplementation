// SPDX-License-Identifier: GPL-3.0-or-later
//! Draw distances of `tessera-world` against a user's own install (read-only).
//!
//! `lod_class_table_from_uniquedb`: finds the LOD-class table through the root DID map's
//! `UNIQUEDB` sub-map (key 0x3F, `mesh.md` §8.5) and checks W-I4 (`walker.md` §11.11: every
//! entry has `d1` ≤ `d0` and `d0` > 0) and W-I5 (*E* increases with the landscape level,
//! *G* ≥ 60 m, *D* ≤ *G*) for every class at every preference level. The classes' `d0` and
//! `d1` are printed for comparison with `walker.md` §11.4.1.

use std::path::PathBuf;
use std::sync::Arc;

use tessera_dat::{ArchiveSet, SetOptions};
use tessera_world::draw_distance::{
    DetailLevel, DistanceSettings, FogClip, LandscapeLevel, LodClasses, MIN_GLOBAL_LIMIT,
    global_limit,
};
use tessera_world::{BundleOptions, WorldContext};

fn install() -> Option<PathBuf> {
    match std::env::var_os("TESSERA_GAME_DIR") {
        Some(d) if !d.is_empty() => Some(PathBuf::from(d)),
        _ => {
            eprintln!("TESSERA_GAME_DIR not set; skipping");
            None
        }
    }
}

#[test]
fn lod_class_table_from_uniquedb() {
    let Some(dir) = install() else { return };
    let set = Arc::new(ArchiveSet::open(&dir, SetOptions::default()).expect("open archive set"));
    let ctx = WorldContext::from_install(set, BundleOptions::default()).expect("world context");
    let classes = LodClasses::load(&ctx).expect("LOD-class table");
    let table = classes.table();
    assert!(!table.is_empty());
    for e in &table.entries {
        let [d0, d1, ..] = e.values;
        assert!(
            d0 > 0.0 && d1 <= d0,
            "W-I4: class {:#x}: d0 {d0}, d1 {d1}",
            e.class
        );
    }
    let levels = [
        DetailLevel::Low,
        DetailLevel::Medium,
        DetailLevel::High,
        DetailLevel::VeryHigh,
        DetailLevel::UltraHigh,
    ];
    let landscapes = [
        LandscapeLevel::VeryLow,
        LandscapeLevel::Low,
        LandscapeLevel::Medium,
        LandscapeLevel::High,
        LandscapeLevel::VeryHigh,
    ];
    assert!(
        landscapes.windows(2).all(|w| w[0].extent() < w[1].extent()),
        "W-I5: E"
    );
    for land in landscapes {
        for fog in [None, Some(10.0_f32), Some(900.0), Some(5_000.0)] {
            let g = global_limit(
                Some(land.extent()),
                fog.map(|far| FogClip { enabled: true, far }),
            );
            assert!(g >= MIN_GLOBAL_LIMIT, "W-I5: G");
            for object in levels {
                let s = DistanceSettings {
                    object_draw_distance: object,
                    landscape: land,
                    ..DistanceSettings::default()
                };
                for e in &table.entries {
                    for player in [false, true] {
                        let d = classes.values(Some(e.class), player).draw_distance(g, &s);
                        assert!(d <= g, "W-I5: D");
                    }
                }
            }
        }
    }
    let rows: Vec<String> = table
        .entries
        .iter()
        .map(|e| format!("{:#x}: {} / {}", e.class, e.values[0], e.values[1]))
        .collect();
    eprintln!(
        "LOD-class table {}: {} classes (class: d0 / d1) {}",
        table.did,
        table.len(),
        rows.join(", ")
    );
}
