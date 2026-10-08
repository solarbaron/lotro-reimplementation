// SPDX-License-Identifier: GPL-3.0-or-later
//! ROADMAP M4.5 exit criterion 6, walker side: the "expected" facts of the visual rows of
//! `docs/checklists/m4-5-visual.md` §4, checked with the world model the walker draws from
//! (read-only; skipped unless `TESSERA_GAME_DIR` is set). The look of each row is checked
//! by viewing the walker's screenshot (`walker.md` §14.10) and, against the original, by
//! the owner (items O20–O27).
//!
//! | Rows | Checked |
//! |---|---|
//! | T1–T16 | the layer stack (terrain-type keys, bottom first) over the triangle under the target, terrain or ceiling chunk, resolved by the walker's `terrain-texturing.md` §4.9 path against the region's table; the light map where the row names one |
//! | S1–S11 | the landblock's total of generated objects; the two nearest: DID, position (0.01 m), heading (0.1°), scale (0.0001) |
//! | F1–F6 | frill records of the block (all 256 sectors at density 1); F1: the block stores no terrain layers, is open and is drawn with one whole layer of the table's smallest-`draw_order` entry, key 0 (`walker.md` §14.10a) |
//! | W1–W8 | landblock water: height and surface (W8: none) |
//! | Y1–Y4 | sky key A by the walker's lookup (`camera_sky_key`: landblock-info set, block map, else 2) → day description at day 12,240 and the row's time (day type 2); fog near and far, sun and ambient brightness, toward-sun vector |
//! | Y5 | the indoor record's inputs at the IN1 start (cell 9 of 0x802191A6) by the walker's lookup (cell, landblock info, block map): key B 1, ambient *X* = *Y*, linear fog 0.059–90 in R 11, G 14, B 2 (`landblock.md` §4.13 worked example) |
//! | L2 | the Light entities: positions, range, falloff, colour, baked flag |
//!
//! Run with `cargo test --release -p tessera-client --test visual_rows_real_data --
//! --nocapture`. Only DIDs, keys, numbers and pass/fail are printed.

use std::path::PathBuf;
use std::sync::Arc;
use std::time::Duration;

use tessera_assets::{PoolConfig, WorkerPool};
use tessera_client::walker::env::{
    IndoorPids, camera_sky_key, compose, fog_distances, indoor_inputs_of,
};
use tessera_client::walker::fxparse::{Systems, clock_seconds};
use tessera_client::walker::{block_open, layerless_key, resolve_block_surface};
use tessera_dat::{ArchiveSet, SetOptions};
use tessera_formats::landblock::{GridSurface, LightMaps};
use tessera_formats::{Did, PropertyDictionary, Record};
use tessera_terrain::ResolveOptions;
use tessera_world::frills::DensityThinner;
use tessera_world::{
    BundleOptions, LandblockKey, RingConfig, StreamConfig, WorldContext, WorldPos, WorldStreamer,
    build_frill_sector, heading_degrees,
};

fn install() -> Option<PathBuf> {
    match std::env::var_os("TESSERA_GAME_DIR") {
        Some(d) if !d.is_empty() => Some(PathBuf::from(d)),
        _ => {
            eprintln!("TESSERA_GAME_DIR not set; skipping");
            None
        }
    }
}

fn key(region: u8, block: u16) -> LandblockKey {
    LandblockKey::new(region, (block >> 8) as u8, (block & 0xFF) as u8).expect("row landblock")
}

/// A world with small rings: only the row's landblock and its neighbours are needed.
fn world() -> Option<(Arc<WorldContext>, WorldStreamer)> {
    let dir = install()?;
    let set = Arc::new(ArchiveSet::open(&dir, SetOptions::default()).expect("open archive set"));
    let ctx = Arc::new(
        WorldContext::from_install(Arc::clone(&set), BundleOptions::default())
            .expect("world context"),
    );
    let pool = WorkerPool::new(PoolConfig::default());
    let config = StreamConfig {
        rings: RingConfig {
            near: 160.0,
            mid: 160.0,
            far: 160.0,
            hysteresis: 160.0,
        },
        ..StreamConfig::default()
    };
    let w = WorldStreamer::new(Arc::clone(&ctx), &pool, config).expect("streamer");
    Some((ctx, w))
}

fn settle(w: &mut WorldStreamer, region: u8, block: u16, x: f64, y: f64) -> WorldPos {
    let o = key(region, block).origin();
    let pos = WorldPos::new(region, o[0] + x, o[1] + y, 0.0);
    assert!(w.settle(pos, Duration::from_secs(120)), "never settled");
    pos
}

/// One terrain row: target, expected stack (bottom first) and ceiling flag.
struct TRow {
    id: &'static str,
    region: u8,
    block: u16,
    target: [f32; 2],
    stack: &'static [u32],
    ceiling: bool,
    light_map: Option<u32>,
}

#[rustfmt::skip]
const T_ROWS: [TRow; 16] = [
    TRow { id: "T1", region: 1, block: 0x7574, target: [128.90, 98.63], stack: &[2, 6, 7, 675, 10], ceiling: false, light_map: None },
    TRow { id: "T2", region: 1, block: 0x54A0, target: [37.34, 111.30], stack: &[391], ceiling: false, light_map: None },
    TRow { id: "T3", region: 2, block: 0x65B1, target: [139.41, 106.00], stack: &[850], ceiling: false, light_map: None },
    TRow { id: "T4", region: 2, block: 0x4F32, target: [139.42, 64.30], stack: &[195], ceiling: false, light_map: None },
    TRow { id: "T5", region: 3, block: 0x9FE8, target: [44.36, 27.70], stack: &[0, 297, 306, 57], ceiling: false, light_map: Some(0x4116_792D) },
    TRow { id: "T6", region: 3, block: 0x7FBD, target: [52.91, 49.90], stack: &[67], ceiling: false, light_map: None },
    TRow { id: "T7", region: 3, block: 0x49E7, target: [24.31, 121.73], stack: &[290, 274], ceiling: false, light_map: None },
    TRow { id: "T8", region: 4, block: 0x25BC, target: [34.19, 44.50], stack: &[385, 257, 232], ceiling: false, light_map: None },
    TRow { id: "T9", region: 4, block: 0xAC6E, target: [67.77, 93.62], stack: &[609, 613], ceiling: false, light_map: Some(0x411A_D5A6) },
    TRow { id: "T10", region: 5, block: 0x5C6C, target: [97.57, 49.58], stack: &[1089, 1256], ceiling: false, light_map: None },
    TRow { id: "T11", region: 5, block: 0x3B85, target: [89.66, 104.89], stack: &[691, 884, 936], ceiling: false, light_map: None },
    TRow { id: "T12", region: 5, block: 0x4F62, target: [124.98, 105.99], stack: &[1103, 1245, 1256, 1257, 1258, 1259, 1268, 1220, 1222], ceiling: false, light_map: Some(0x4125_EE9F) },
    TRow { id: "T13", region: 14, block: 0x07CA, target: [74.78, 120.78], stack: &[33], ceiling: false, light_map: Some(0x4120_56D3) },
    TRow { id: "T14", region: 14, block: 0x0DCA, target: [133.79, 82.41], stack: &[29, 78], ceiling: false, light_map: Some(0x4120_54EE) },
    TRow { id: "T15", region: 1, block: 0xE13A, target: [111.36, 133.71], stack: &[639, 640, 641, 774, 775, 776], ceiling: true, light_map: Some(0x4111_307C) },
    TRow { id: "T16", region: 4, block: 0xC9AE, target: [17.17, 56.20], stack: &[426], ceiling: true, light_map: Some(0x411A_F104) },
];

#[test]
fn terrain_rows_have_their_layer_stacks() {
    let Some((ctx, mut w)) = world() else { return };
    let mut failed = Vec::new();
    for row in &T_ROWS {
        settle(&mut w, row.region, row.block, 80.0, 80.0);
        let k = key(row.region, row.block);
        let bundle = w.bundle(k).expect("resident");
        let table = ctx
            .region(row.region)
            .and_then(|r| r.scenery.as_ref())
            .and_then(|s| s.terrain_types.clone())
            .expect("terrain type table");
        let surface = if row.ceiling {
            GridSurface::Ceiling
        } else {
            GridSurface::Terrain
        };
        let maps = LightMaps {
            terrain: bundle.light_map,
            ceiling: bundle.ceiling_light_map,
        };
        // The walker's path (`walker.md` §14.4): the open flag, then the chunk's layers.
        let (layers, hidden) = resolve_block_surface(
            &bundle.data,
            surface,
            &table,
            &maps,
            &ResolveOptions::default(),
        )
        .expect("layers resolve");
        let keys: Vec<u32> = layers
            .triangle_at(row.target[0], row.target[1])
            .map(|t| layers.layers_at(t).map(|l| l.key()).collect())
            .unwrap_or_default();
        let lm = maps.for_surface(surface).map(|d| d.0);
        let mut fails = Vec::new();
        if hidden {
            fails.push("chunk hidden (§4.9 rule 3)".to_owned());
        }
        if keys != row.stack {
            fails.push(format!("stack {keys:?} ≠ {:?}", row.stack));
        }
        // The row's light map is the surface's own, or the block's terrain map for a
        // ceiling row that names only that one (T16).
        let block_maps = [maps.terrain, maps.ceiling].map(|m| m.map(|d| d.0));
        if let Some(want) = row.light_map
            && lm != Some(want)
            && !(row.ceiling && block_maps.contains(&Some(want)))
        {
            fails.push(format!("light map {lm:x?} ≠ {want:#x}"));
        }
        report(row.id, &fails, &mut failed, &format!("stack {keys:?}"));
    }
    assert!(failed.is_empty(), "rows failing: {failed:?}");
}

/// One generated object of a scenery row.
struct Obj {
    did: u32,
    pos: [f32; 3],
    heading: f64,
    scale: f32,
}

/// The scenery rows of `m4-5-visual.md` §4.2 as re-derived on 2026-10-07 (`scenery.md`
/// §7.4 step 14b, S36): id, region, landblock, the landblock's total of generated
/// objects, and the two nearest objects.
#[rustfmt::skip]
const S_ROWS: [(&str, u8, u16, usize, [Obj; 2]); 11] = [
    ("S1", 1, 0x669A, 25, [Obj { did: 0x4700_35F6, pos: [134.757, 76.422, 555.839], heading: 222.29, scale: 1.2196 }, Obj { did: 0x4700_35F4, pos: [130.612, 83.519, 559.263], heading: 338.75, scale: 1.1701 }]),
    ("S2", 1, 0xA197, 315, [Obj { did: 0x4700_92DC, pos: [101.636, 43.146, 404.978], heading: 170.70, scale: 0.7830 }, Obj { did: 0x4700_08C5, pos: [102.067, 41.334, 405.268], heading: 200.52, scale: 1.1077 }]),
    ("S3", 2, 0xAEBC, 165, [Obj { did: 0x4700_6181, pos: [33.282, 36.418, 673.242], heading: 84.89, scale: 0.7148 }, Obj { did: 0x4700_6181, pos: [35.800, 43.432, 669.749], heading: 156.23, scale: 1.1352 }]),
    ("S4", 2, 0x7744, 309, [Obj { did: 0x4700_5A62, pos: [54.338, 122.030, 158.063], heading: 64.84, scale: 1.0634 }, Obj { did: 0x4700_5A5C, pos: [50.475, 128.619, 159.243], heading: 11.21, scale: 0.9836 }]),
    ("S5", 3, 0x79AD, 151, [Obj { did: 0x4700_7213, pos: [61.890, 74.593, 311.986], heading: 18.84, scale: 0.8633 }, Obj { did: 0x4700_5FEC, pos: [55.744, 77.442, 311.986], heading: 344.47, scale: 0.6526 }]),
    ("S6", 3, 0x7A69, 111, [Obj { did: 0x4700_6BEA, pos: [83.824, 18.210, 189.609], heading: 100.84, scale: 1.0760 }, Obj { did: 0x4700_6BEB, pos: [68.317, 13.195, 189.385], heading: 158.61, scale: 1.2235 }]),
    ("S7", 4, 0x1A23, 445, [Obj { did: 0x4700_8203, pos: [85.939, 86.901, 187.329], heading: 225.05, scale: 0.9025 }, Obj { did: 0x4700_7F5A, pos: [92.493, 91.001, 187.369], heading: 311.70, scale: 1.0031 }]),
    ("S8", 4, 0x7ADD, 166, [Obj { did: 0x4700_6A36, pos: [122.779, 107.565, 252.148], heading: 122.83, scale: 1.1231 }, Obj { did: 0x4700_0620, pos: [126.422, 101.922, 254.398], heading: 163.79, scale: 1.1500 }]),
    ("S9", 5, 0x655A, 221, [Obj { did: 0x4700_D780, pos: [123.961, 37.660, 440.486], heading: 356.54, scale: 1.0304 }, Obj { did: 0x4700_8481, pos: [130.043, 34.487, 444.051], heading: 81.28, scale: 0.4816 }]),
    ("S10", 5, 0x6FD9, 80, [Obj { did: 0x4700_7CB0, pos: [59.346, 42.853, 174.406], heading: 128.72, scale: 1.2184 }, Obj { did: 0x4700_7AA0, pos: [58.741, 41.622, 174.458], heading: 17.71, scale: 1.0178 }]),
    ("S11", 14, 0x1FE2, 186, [Obj { did: 0x4700_09A6, pos: [109.528, 66.155, 450.852], heading: 291.40, scale: 0.5993 }, Obj { did: 0x4700_5B1E, pos: [106.341, 65.737, 450.953], heading: 88.19, scale: 0.7630 }]),
];

/// The smallest difference of two compass headings in degrees.
fn heading_diff(a: f64, b: f64) -> f64 {
    let d = (a - b).rem_euclid(360.0);
    d.min(360.0 - d)
}

#[test]
fn scenery_rows_place_their_objects() {
    let Some((_ctx, mut w)) = world() else { return };
    let mut failed = Vec::new();
    for (id, region, block, total, objs) in &S_ROWS {
        settle(&mut w, *region, *block, 80.0, 80.0);
        let bundle = w.bundle(key(*region, *block)).expect("resident");
        let mut fails = Vec::new();
        if bundle.generated.len() != *total {
            fails.push(format!(
                "{} generated objects ≠ {total}",
                bundle.generated.len()
            ));
        }
        for o in objs {
            let found = bundle.generated.iter().find(|e| {
                e.object == Some(Did(o.did))
                    && (0..3).all(|i| (e.placement.position[i] - o.pos[i]).abs() <= 0.01)
            });
            match found {
                None => {
                    // The nearest generated object, for the report.
                    let near = bundle
                        .generated
                        .iter()
                        .map(|e| {
                            let d: f32 = (0..3)
                                .map(|i| (e.placement.position[i] - o.pos[i]).powi(2))
                                .sum::<f32>()
                                .sqrt();
                            (d, e)
                        })
                        .min_by(|a, b| a.0.total_cmp(&b.0));
                    fails.push(format!(
                        "{:#010x} not at {:?} (nearest: {:?} at {:?}, {:.3} m away)",
                        o.did,
                        o.pos,
                        near.and_then(|n| n.1.object),
                        near.map(|n| n.1.placement.position),
                        near.map_or(f32::NAN, |n| n.0)
                    ));
                }
                Some(e) => {
                    let h = heading_degrees(e.placement.rotation);
                    if heading_diff(h, o.heading) > 0.1 {
                        fails.push(format!("{:#010x} heading {h:.2} ≠ {}", o.did, o.heading));
                    }
                    if (e.scale[0] - o.scale).abs() > 0.0001 {
                        fails.push(format!(
                            "{:#010x} scale {} ≠ {}",
                            o.did, e.scale[0], o.scale
                        ));
                    }
                }
            }
        }
        report(
            id,
            &fails,
            &mut failed,
            &format!("{} generated objects", bundle.generated.len()),
        );
    }
    assert!(failed.is_empty(), "rows failing: {failed:?}");
}

#[test]
fn frill_rows_count_their_records() {
    let Some((ctx, mut w)) = world() else { return };
    let rows: [(&str, u8, u16, usize); 6] = [
        ("F1", 1, 0x4C66, 4_075),
        ("F2", 3, 0x7A69, 22_945),
        ("F3", 5, 0x6FD9, 15_584),
        ("F4", 2, 0x7744, 23_172),
        ("F5", 4, 0x7ADD, 15_066),
        ("F6", 14, 0x1FE2, 8_347),
    ];
    let mut failed = Vec::new();
    for (id, region, block, count) in rows {
        settle(&mut w, region, block, 80.0, 80.0);
        let bundle = w.bundle(key(region, block)).expect("resident");
        let mut thinner = DensityThinner::with_seed(1);
        let mut total = 0usize;
        for sx in 0..16 {
            for sy in 0..16 {
                total += build_frill_sector(&ctx, bundle, sx, sy, 1.0, &mut thinner)
                    .instances
                    .len();
            }
        }
        let mut fails = if total == count {
            Vec::new()
        } else {
            vec![format!("{total} frill records ≠ {count}")]
        };
        if id == "F1" {
            // The F1 block stores no terrain layers and has no ceiling: §4.9 draws it with
            // one whole layer of the entry with the smallest `draw_order`, key 0 here.
            let table = ctx
                .region(region)
                .and_then(|r| r.scenery.as_ref())
                .and_then(|s| s.terrain_types.clone())
                .expect("terrain type table");
            let maps = LightMaps {
                terrain: bundle.light_map,
                ceiling: bundle.ceiling_light_map,
            };
            let (layers, hidden) = resolve_block_surface(
                &bundle.data,
                GridSurface::Terrain,
                &table,
                &maps,
                &ResolveOptions::default(),
            )
            .expect("layers resolve");
            let stored = bundle.data.terrain.layers.len();
            let drawn: Vec<(u32, bool)> = layers
                .layers()
                .iter()
                .map(|l| (l.key(), l.whole()))
                .collect();
            if stored != 0 || !block_open(&bundle.data) || hidden {
                fails.push(format!(
                    "stored layers {stored}, open {}, hidden {hidden}",
                    block_open(&bundle.data)
                ));
            }
            if layerless_key(&table) != Some(0) || drawn != [(0, true)] {
                fails.push(format!(
                    "drawn {drawn:?}, layerless key {:?}",
                    layerless_key(&table)
                ));
            }
        }
        report(id, &fails, &mut failed, &format!("{total} frill records"));
    }
    assert!(failed.is_empty(), "rows failing: {failed:?}");
}

#[test]
fn water_rows_have_their_water() {
    let Some((_ctx, mut w)) = world() else { return };
    #[rustfmt::skip]
    #[allow(clippy::type_complexity)]
    let rows: [(&str, u8, u16, Option<(f32, u32)>); 8] = [
        ("W1", 5, 0xEFB1, Some((149.020, 0x3100_8DB5))),
        ("W2", 3, 0x7E2C, Some((149.020, 0x3100_6653))),
        ("W3", 2, 0x5BC7, Some((941.177, 0x3100_4359))),
        ("W4", 1, 0x9E7F, Some((337.255, 0x3100_4359))),
        ("W5", 2, 0x81F8, Some((439.216, 0))),
        ("W6", 14, 0x3DEB, Some((380.392, 0x3100_4359))),
        ("W7", 4, 0x38A8, Some((149.020, 0x3100_6FAE))),
        ("W8", 3, 0x4DE9, None),
    ];
    let mut failed = Vec::new();
    for (id, region, block, want) in rows {
        let pos = settle(&mut w, region, block, 80.0, 80.0);
        let got = w.landblock_water(&pos);
        let mut fails = Vec::new();
        match (got.as_ref(), want) {
            (None, None) => {}
            (Some(lw), Some((h, surface))) => {
                if (lw.height - h).abs() > 0.001 {
                    fails.push(format!("height {} ≠ {h}", lw.height));
                }
                if surface != 0 && lw.surface() != Did(surface) {
                    fails.push(format!("surface {} ≠ {surface:#010x}", lw.surface()));
                }
            }
            (g, wnt) => fails.push(format!("water {:?} ≠ {wnt:?}", g.map(|l| l.height))),
        }
        report(
            id,
            &fails,
            &mut failed,
            &format!("water {:?}", got.map(|l| (l.height, l.surface()))),
        );
    }
    assert!(failed.is_empty(), "rows failing: {failed:?}");
}

#[test]
fn sky_rows_blend_their_keyframes() {
    let Some((ctx, mut w)) = world() else { return };
    let lib = ctx.systems().sky.clone().expect("sky library");
    let cal = lib.calendar().clone();
    #[rustfmt::skip]
    #[allow(clippy::type_complexity)]
    let rows: [(&str, u8, u16, [f64; 2], f64, u32, u32, [f32; 2], f32, [f32; 3], f32); 4] = [
        ("Y1", 1, 0x5378, [56.36, 56.59], 0.5, 0x1000_023B, 0x1A00_0359, [300.0, 2_058.8], 0.55, [-0.224, -0.837, 0.500], 0.20),
        ("Y2", 4, 0xC755, [33.74, 131.37], 0.25, 0x1000_0106, 0x1A00_0269, [363.6, 2_500.0], 0.50, [0.078, 0.638, 0.766], 0.40),
        ("Y3", 3, 0x693E, [19.47, 44.71], 0.78, 0x1000_0084, 0x1A00_0213, [200.0, 1_725.0], 0.55, [-0.224, -0.837, 0.500], 0.20),
        ("Y4", 1, 0x7574, [128.90, 98.63], 0.0, 0x1000_0070, 0x1A00_0000, [1_000.0, 2_500.0], 0.55, [-0.224, -0.837, 0.500], 0.20),
    ];
    let mut failed = Vec::new();
    for (id, region, block, xy, t, sky, day, fog, sun, toward, ambient) in rows {
        let pos = settle(&mut w, region, block, xy[0], xy[1]);
        let g = clock_seconds(
            cal.days_per_year,
            cal.day_length,
            cal.zero_offset,
            cal.zero_year,
            12_240,
            t,
        )
        .expect("clock");
        let mut fails = Vec::new();
        // Key A by the walker's own lookup (`walker.md` §14.3 OPEN (d), `environment.md`
        // §7.1.1): landblock-info set, then block map, else 2; outdoors, no camera cell.
        let k = camera_sky_key(&w, None, &pos);
        if k != sky {
            fails.push(format!("sky key {k:#x} ≠ {sky:#x}"));
        }
        let e = lib.evaluate(Some(k), 2, g);
        let v = &e.values;
        if e.choice.map(|c| c.day) != Some(Did(day)) {
            fails.push(format!("day {:?} ≠ {day:#010x}", e.choice.map(|c| c.day)));
        }
        if (v.fog_near - fog[0]).abs() > 0.2 || (v.fog_far - fog[1]).abs() > 0.2 {
            fails.push(format!("fog {} / {} ≠ {fog:?}", v.fog_near, v.fog_far));
        }
        if (v.sun_brightness - sun).abs() > 0.005 {
            fails.push(format!("sun brightness {} ≠ {sun}", v.sun_brightness));
        }
        if (0..3).any(|i| (v.sun_toward[i] - toward[i]).abs() > 0.002) {
            fails.push(format!("toward sun {:?} ≠ {toward:?}", v.sun_toward));
        }
        if (v.ambient_brightness - ambient).abs() > 0.005 {
            fails.push(format!("ambient {} ≠ {ambient}", v.ambient_brightness));
        }
        report(
            id,
            &fails,
            &mut failed,
            &format!(
                "day {:?}, fog {:.1} / {:.1}",
                e.choice.map(|c| c.day),
                v.fog_near,
                v.fog_far
            ),
        );
    }
    // Y5: the indoor record at the IN1 start, cell 9 of 0x802191A6, by the walker's
    // lookup (`landblock.md` §4.13 worked example).
    settle(&mut w, 1, 0x91A6, 80.0, 80.0);
    let bundle = w.bundle(key(1, 0x91A6)).expect("IN1 landblock");
    let dict_bytes = ctx
        .source()
        .read(Did(0x3400_0000))
        .expect("read")
        .expect("property dictionary");
    let dict = PropertyDictionary::parse_expecting(&dict_bytes, 0x3400_0000_u32)
        .expect("property dictionary");
    let pids = IndoorPids::from_names(|n| dict.pid(n));
    let ind = indoor_inputs_of(Some(bundle), &pids);
    let byte = |v: f32| v * 255.0;
    let near_rgb = |c: [f32; 3], want: [f32; 3]| (0..3).all(|i| (byte(c[i]) - want[i]).abs() < 0.6);
    let mut fails = Vec::new();
    if ind.sky_key != 1 {
        fails.push(format!("key B {}", ind.sky_key));
    }
    match ind.ambient_xy() {
        Some((x, y)) if x == y && near_rgb(x, [8.0, 25.0, 8.0]) => {}
        other => fails.push(format!("ambient X, Y {other:?}")),
    }
    if !ind.fog_on(true) || ind.fog_on(false) {
        fails.push(format!("fog type {:?}", ind.fog_type));
    }
    let (near, far) = fog_distances(0.0, 10_000.0, ind.fog_near, ind.fog_far);
    if (near - 0.059).abs() > 0.001 || (far - 90.0).abs() > 0.001 {
        fails.push(format!("interior fog {near} / {far}"));
    }
    if ind
        .fog_colour
        .is_none_or(|c| !near_rgb(c, [11.0, 14.0, 2.0]))
    {
        fails.push(format!("interior fog colour {:?}", ind.fog_colour));
    }
    // The composed indoor frame: no sun, the fog of the overrides.
    let values = tessera_world::environment::SkyValues::defaults();
    let lit = compose(
        Some(&values),
        Some(&ind),
        &Systems::all(),
        tessera_render::glam::Vec3::ZERO,
        tessera_render::glam::Vec3::X,
        0.0,
    );
    if lit.lighting.sun.is_some()
        || lit
            .lighting
            .fog
            .is_none_or(|f| (f.near - near).abs() > 1e-6 || (f.far - far).abs() > 1e-6)
    {
        fails.push("composed indoor frame".to_owned());
    }
    report("Y5", &fails, &mut failed, "indoor record");
    assert!(failed.is_empty(), "rows failing: {failed:?}");
}

#[test]
fn light_row_places_its_lights() {
    let Some((_ctx, mut w)) = world() else { return };
    settle(&mut w, 1, 0x8FF3, 80.0, 80.0);
    let bundle = w.bundle(key(1, 0x8FF3)).expect("resident");
    let want: [(u32, [f32; 3]); 5] = [
        (460, [90.30, 42.41, 621.09]),
        (462, [125.62, 48.18, 621.09]),
        (483, [74.68, 34.44, 621.09]),
        (464, [119.10, 80.34, 621.09]),
        (474, [71.64, 72.15, 621.09]),
    ];
    let mut fails = Vec::new();
    for (index, pos) in want {
        match bundle.lights.iter().find(|l| l.index == index) {
            None => fails.push(format!("light #{index} missing")),
            Some(l) => {
                if (0..3).any(|i| (l.position[i] - pos[i]).abs() > 0.01) {
                    fails.push(format!("#{index} at {:?}", l.position));
                }
                let p = &l.params;
                if (p.range - 10.0).abs() > 1e-4
                    || (p.falloff - 0.5).abs() > 1e-4
                    || !p.baked_landscape
                {
                    fails.push(format!("#{index} range {} falloff {}", p.range, p.falloff));
                }
                let c = p.diffuse.map(|v| (v * 255.0).round());
                if c != [251.0, 97.0, 3.0] {
                    fails.push(format!("#{index} colour {c:?}"));
                }
            }
        }
    }
    let mut failed = Vec::new();
    report(
        "L2",
        &fails,
        &mut failed,
        &format!("{} lights in the block", bundle.lights.len()),
    );
    assert!(failed.is_empty(), "rows failing: {failed:?}");
}

fn report(id: &str, fails: &[String], failed: &mut Vec<String>, ok: &str) {
    if fails.is_empty() {
        eprintln!("{id}: pass ({ok})");
    } else {
        eprintln!("{id}: FAIL: {}", fails.join("; "));
        failed.push(id.to_owned());
    }
}
