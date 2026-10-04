// SPDX-License-Identifier: GPL-3.0-or-later
//! ROADMAP M3 exit criteria 4 and 5, walker side: the 20 reference locations of
//! `docs/checklists/m3-reference-locations.md` §2.2–§2.3, checked with the world model
//! the walker draws from (read-only; skipped unless `TESSERA_GAME_DIR` is set).
//!
//! For every row the streamer is settled at the stand point (as `--spawn R,X,Y,x,y` or
//! `--place "<loc line>"` would put the walker there) and the checklist's §4.1 checks run:
//!
//! 1. terrain height at the stand point within 0.05 m of the expected value (and at the
//!    anchor corner within 0.05 m);
//! 2. the anchor SED (landblock info, index) is present with the listed object, instance
//!    ID, scale and yaw, passes the draw rule, and the bottom corner of its part bounds
//!    (union of the part meshes' level-0 boxes, `mesh.md` §6.1), placed with the walker's
//!    scale · rotation · translation (`entity-descriptor.md` §5), lies within 0.1 m of the
//!    listed corner; its bottom and top within 0.1 m;
//! 3. the map-coordinate string equals either listed form, and the `/loc` tokens equal the
//!    listed ones;
//! 4. the block map's `Area_DID` is the listed area and its name resolves (the name itself
//!    is game text and is never printed).
//!
//! The owner-observation columns (what the original client shows) are not checked here.
//! Run with `cargo test --release -p tessera-client --test reference_locations --
//! --nocapture`. Only DIDs, coordinates, heights and pass/fail are printed.

use std::path::PathBuf;
use std::sync::Arc;
use std::time::Duration;

use tessera_assets::{PoolConfig, WorkerPool};
use tessera_dat::{ArchiveSet, SetOptions};
use tessera_formats::Did;
use tessera_mesh::Mesh;
use tessera_render::glam::{Mat4, Quat, Vec3};
use tessera_world::{
    BundleOptions, LandblockKey, LocReport, LocalPos, StreamConfig, WStateAreaNames, WorldContext,
    WorldPos, WorldStreamer, yaw,
};

/// One row of the checklist (§2.2 joined with §2.3).
struct Row {
    id: &'static str,
    region: u8,
    block: u16,
    stand: [f64; 2],
    loc: &'static str,
    map: &'static str,
    map_alt: Option<&'static str>,
    height: f64,
    area: u32,
    anchor: u32,
    sed: u32,
    iid: u64,
    scale: f32,
    yaw_deg: f64,
    sed_pos: [f64; 3],
    corner: [f64; 2],
    corner_height: f64,
    bottom_top: [f64; 2],
}

#[rustfmt::skip]
const ROWS: [Row; 20] = [
    Row { id: "M3-01", region: 1, block: 0x5378, stand: [56.36, 56.59], loc: "r1 lx666 ly962 ox56.36 oy56.59 oz455.05", map: "28.1S, 80.1W", map_alt: None, height: 455.05, area: 0x7006_05C5, anchor: 0x4700_0E92, sed: 0, iid: 0x0115_3780_0000_0001, scale: 1.00, yaw_deg: 139.5, sed_pos: [38.08, 55.79, 455.12], corner: [54.87, 56.48], corner_height: 455.05, bottom_top: [453.87, 467.36] },
    Row { id: "M3-02", region: 1, block: 0xA197, stand: [137.35, 111.58], loc: "r1 lx1294 ly1213 ox137.35 oy111.58 oz389.25", map: "3.0S, 17.3W", map_alt: None, height: 389.25, area: 0x7006_93CE, anchor: 0x4700_03D3, sed: 23, iid: 0x011A_1970_0000_0018, scale: 1.00, yaw_deg: 0.0, sed_pos: [127.81, 102.47, 389.26], corner: [136.29, 110.52], corner_height: 389.25, bottom_top: [389.16, 407.78] },
    Row { id: "M3-03", region: 1, block: 0x6D8D, stand: [62.96, 15.07], loc: "r1 lx875 ly1128 ox62.96 oy15.07 oz413.60", map: "11.5S, 59.3W", map_alt: Some("11.5S, 59.2W"), height: 413.60, area: 0x7000_3DA6, anchor: 0x4700_03F2, sed: 70, iid: 0x0116_D8D0_0000_00FA, scale: 1.00, yaw_deg: 45.0, sed_pos: [71.88, 15.08, 413.76], corner: [64.46, 15.07], corner_height: 413.60, bottom_top: [413.41, 423.93] },
    Row { id: "M3-04", region: 1, block: 0x9B40, stand: [127.57, 145.41], loc: "r1 lx1246 ly519 ox127.57 oy145.41 oz580.57", map: "72.5S, 22.2W", map_alt: Some("72.4S, 22.1W"), height: 580.57, area: 0x7002_0A43, anchor: 0x4700_1411, sed: 19, iid: 0x0119_B400_0000_0018, scale: 1.20, yaw_deg: 25.5, sed_pos: [120.58, 146.04, 580.62], corner: [126.16, 145.92], corner_height: 580.57, bottom_top: [579.79, 589.15] },
    Row { id: "M3-05", region: 1, block: 0xA68E, stand: [123.99, 113.58], loc: "r1 lx1334 ly1141 ox123.99 oy113.58 oz526.81", map: "10.2S, 13.4W", map_alt: Some("10.2S, 13.3W"), height: 526.81, area: 0x7006_DD10, anchor: 0x4700_9706, sed: 26, iid: 0x011A_68E0_0000_001B, scale: 1.00, yaw_deg: 106.0, sed_pos: [130.07, 110.31, 526.54], corner: [125.30, 112.85], corner_height: 526.79, bottom_top: [526.00, 539.46] },
    Row { id: "M3-06", region: 2, block: 0x6EA5, stand: [116.64, 69.58], loc: "r2 lx885 ly1323 ox116.64 oy69.58 oz517.46", map: "7.9N, 58.2W", map_alt: None, height: 517.46, area: 0x7005_2188, anchor: 0x4700_93DF, sed: 20, iid: 0x0126_EA50_0000_0038, scale: 1.00, yaw_deg: 307.3, sed_pos: [139.60, 72.83, 517.46], corner: [118.12, 69.38], corner_height: 517.46, bottom_top: [516.60, 531.31] },
    Row { id: "M3-07", region: 2, block: 0x5C58, stand: [155.67, 112.37], loc: "r2 lx743 ly709 ox155.67 oy112.37 oz311.55", map: "53.4S, 72.4W", map_alt: None, height: 311.55, area: 0x7003_2F44, anchor: 0x4700_618F, sed: 46, iid: 0x0125_C580_0000_0074, scale: 1.00, yaw_deg: 165.0, sed_pos: [145.92, 109.01, 311.84], corner: [154.37, 111.62], corner_height: 311.55, bottom_top: [311.16, 321.37] },
    Row { id: "M3-08", region: 2, block: 0x81CC, stand: [7.34, 132.21], loc: "r2 lx1032 ly1638 ox7.34 oy132.21 oz692.96", map: "39.5N, 43.6W", map_alt: Some("39.4N, 43.5W"), height: 692.96, area: 0x7004_E2AE, anchor: 0x4700_91C8, sed: 128, iid: 0x0128_1CC0_0000_0108, scale: 1.00, yaw_deg: 180.0, sed_pos: [18.61, 140.29, 693.06], corner: [8.40, 133.27], corner_height: 692.96, bottom_top: [692.96, 722.09] },
    Row { id: "M3-09", region: 2, block: 0x7B68, stand: [131.54, 30.72], loc: "r2 lx990 ly833 ox131.54 oy30.72 oz432.50", map: "41.0S, 47.7W", map_alt: None, height: 432.50, area: 0x7002_CA68, anchor: 0x4700_5EEE, sed: 72, iid: 0x0127_B680_0000_006D, scale: 1.00, yaw_deg: 45.0, sed_pos: [121.88, 30.72, 432.63], corner: [130.04, 30.72], corner_height: 432.50, bottom_top: [432.19, 465.32] },
    // Mesh bottom: the checklist's 224.83 is the union over every submesh of the part,
    // including lower detail levels; `mesh.md` §6.1 bounds a mesh by level 0 only, which
    // gives 225.09 (checklist §3 erratum).
    Row { id: "M3-10", region: 3, block: 0x693E, stand: [19.47, 44.71], loc: "r3 lx840 ly498 ox19.47 oy44.71 oz225.37", map: "74.6S, 62.7W", map_alt: Some("74.5S, 62.7W"), height: 225.37, area: 0x7003_EE0B, anchor: 0x4700_6DFD, sed: 15, iid: 0x0136_93E0_0000_0012, scale: 1.00, yaw_deg: 75.0, sed_pos: [37.79, 50.11, 225.19], corner: [20.77, 45.46], corner_height: 225.37, bottom_top: [225.09, 257.05] },
    Row { id: "M3-11", region: 3, block: 0xA73F, stand: [126.67, 41.80], loc: "r3 lx1342 ly506 ox126.67 oy41.80 oz311.98", map: "73.8S, 12.6W", map_alt: Some("73.7S, 12.5W"), height: 311.98, area: 0x7004_290A, anchor: 0x4700_7788, sed: 523, iid: 0x013A_73F0_0000_02B1, scale: 1.00, yaw_deg: 90.0, sed_pos: [107.94, 26.65, 311.99], corner: [125.61, 40.74], corner_height: 311.98, bottom_top: [311.92, 348.32] },
    Row { id: "M3-12", region: 3, block: 0x973B, stand: [118.38, 132.54], loc: "r3 lx1213 ly478 ox118.38 oy132.54 oz175.87", map: "76.5S, 25.4W", map_alt: None, height: 175.87, area: 0x7004_290B, anchor: 0x4700_4B6E, sed: 81, iid: 0x0139_73B0_0000_0074, scale: 1.00, yaw_deg: 345.0, sed_pos: [130.07, 118.32, 175.69], corner: [119.13, 131.25], corner_height: 175.87, bottom_top: [175.51, 253.18] },
    Row { id: "M3-13", region: 3, block: 0x622F, stand: [147.76, 107.10], loc: "r3 lx791 ly381 ox147.76 oy107.10 oz217.66", map: "86.3S, 67.7W", map_alt: Some("86.2S, 67.6W"), height: 217.66, area: 0x7004_6C64, anchor: 0x4700_711B, sed: 38, iid: 0x0136_22F0_0000_0030, scale: 0.70, yaw_deg: 206.7, sed_pos: [146.57, 103.02, 217.61], corner: [147.29, 105.68], corner_height: 217.64, bottom_top: [216.25, 231.41] },
    Row { id: "M3-14", region: 4, block: 0xC755, stand: [33.74, 131.37], loc: "r4 lx1593 ly686 ox33.74 oy131.37 oz155.40", map: "55.7S, 12.6E", map_alt: Some("55.7S, 12.5E"), height: 155.40, area: 0x7004_7BAC, anchor: 0x4700_85DA, sed: 40, iid: 0x014C_7550_0000_00BD, scale: 1.00, yaw_deg: 225.0, sed_pos: [16.94, 135.56, 155.45], corner: [32.24, 131.37], corner_height: 155.40, bottom_top: [155.31, 169.98] },
    Row { id: "M3-15", region: 4, block: 0xBE69, stand: [129.59, 126.29], loc: "r4 lx1526 ly846 ox129.59 oy126.29 oz298.49", map: "39.8S, 5.8E", map_alt: Some("39.7S, 5.8E"), height: 298.49, area: 0x7004_6CA4, anchor: 0x4700_7FBA, sed: 61, iid: 0x014B_E690_0000_007E, scale: 1.00, yaw_deg: 90.0, sed_pos: [121.83, 135.15, 300.54], corner: [128.53, 127.35], corner_height: 298.49, bottom_top: [298.37, 332.19] },
    Row { id: "M3-16", region: 4, block: 0x5659, stand: [47.58, 123.44], loc: "r4 lx690 ly718 ox47.58 oy123.44 oz229.98", map: "52.6S, 77.8W", map_alt: Some("52.5S, 77.7W"), height: 229.98, area: 0x7006_6142, anchor: 0x4700_175A, sed: 156, iid: 0x0145_6590_0000_009D, scale: 1.75, yaw_deg: 279.8, sed_pos: [37.79, 130.36, 229.98], corner: [46.35, 124.31], corner_height: 229.98, bottom_top: [229.50, 243.72] },
    Row { id: "M3-17", region: 5, block: 0x267B, stand: [116.31, 34.90], loc: "r5 lx309 ly985 ox116.31 oy34.90 oz192.26", map: "25.8S, 115.8W", map_alt: None, height: 192.26, area: 0x7006_71C5, anchor: 0x4700_D0B5, sed: 6, iid: 0x0152_67B0_0000_000F, scale: 1.00, yaw_deg: 15.0, sed_pos: [95.08, 42.65, 192.16], corner: [115.01, 35.65], corner_height: 192.26, bottom_top: [192.06, 229.93] },
    Row { id: "M3-18", region: 5, block: 0x757A, stand: [79.02, 66.36], loc: "r5 lx939 ly979 ox79.02 oy66.36 oz228.76", map: "26.5S, 52.8W", map_alt: Some("26.4S, 52.8W"), height: 228.76, area: 0x7006_B251, anchor: 0x4700_E31C, sed: 6, iid: 0x0157_57A0_0000_000A, scale: 1.00, yaw_deg: 0.0, sed_pos: [66.81, 54.55, 228.49], corner: [77.96, 65.30], corner_height: 228.76, bottom_top: [228.41, 252.75] },
    Row { id: "M3-19", region: 5, block: 0x536C, stand: [81.41, 98.01], loc: "r5 lx668 ly868 ox81.41 oy98.01 oz194.52", map: "37.5S, 80.0W", map_alt: Some("37.5S, 79.9W"), height: 194.52, area: 0x7006_B247, anchor: 0x4700_E7C3, sed: 224, iid: 0x0155_36C0_0000_0122, scale: 1.00, yaw_deg: 90.0, sed_pos: [76.32, 89.51, 194.55], corner: [80.35, 96.95], corner_height: 194.52, bottom_top: [194.53, 213.76] },
    Row { id: "M3-20", region: 5, block: 0x4280, stand: [118.65, 50.38], loc: "r5 lx533 ly1026 ox118.65 oy50.38 oz158.94", map: "21.7S, 93.4W", map_alt: None, height: 158.94, area: 0x7006_71C4, anchor: 0x4700_D3B8, sed: 37, iid: 0x0154_2800_0000_005A, scale: 1.00, yaw_deg: 0.0, sed_pos: [110.28, 45.15, 158.94], corner: [117.59, 49.32], corner_height: 158.94, bottom_top: [158.55, 164.48] },
];

/// Tolerances of the checklist's §4.1.
const HEIGHT_TOL: f64 = 0.05;
const CORNER_TOL: f64 = 0.1;
/// The checklist rounds SED positions, yaw and scale to the printed digits.
const POS_TOL: f64 = 0.006;
const YAW_TOL_DEG: f64 = 0.06;
const SCALE_TOL: f32 = 0.006;

fn install() -> Option<PathBuf> {
    match std::env::var_os("TESSERA_GAME_DIR") {
        Some(d) if !d.is_empty() => Some(PathBuf::from(d)),
        _ => {
            eprintln!("TESSERA_GAME_DIR not set; skipping");
            None
        }
    }
}

/// Union of the level-0 boxes of the part meshes, in the setup's frame.
fn part_bounds(set: &ArchiveSet, parts: &[Did]) -> Result<([f32; 3], [f32; 3]), String> {
    let mut lo = [f32::INFINITY; 3];
    let mut hi = [f32::NEG_INFINITY; 3];
    for p in parts {
        let bytes = set
            .read(p.0)
            .map_err(|e| format!("part {:#010x}: {e}", p.0))?;
        let mesh = Mesh::parse(&bytes).map_err(|e| format!("part {:#010x}: {e}", p.0))?;
        let Some(b) = mesh.bounds else { continue };
        for i in 0..3 {
            lo[i] = lo[i].min(b.aabb.min[i]);
            hi[i] = hi[i].max(b.aabb.max[i]);
        }
    }
    if lo.iter().chain(&hi).all(|v| v.is_finite()) {
        Ok((lo, hi))
    } else {
        Err("no part has bounds".into())
    }
}

/// Checks one row against the settled streamer; returns the failures (empty = pass).
fn check_row(set: &ArchiveSet, w: &WorldStreamer, row: &Row) -> Vec<String> {
    let mut fails = Vec::new();
    let (bx, by) = ((row.block >> 8) as u8, (row.block & 0xFF) as u8);
    let key = LandblockKey::new(row.region, bx, by).expect("row landblock");
    let o = key.origin();
    let pos = WorldPos::new(
        row.region,
        o[0] + row.stand[0],
        o[1] + row.stand[1],
        row.height,
    );
    let Some(bundle) = w.bundle(key) else {
        return vec!["landblock not resident after settling".into()];
    };

    // 1. Terrain height at the stand point and at the anchor corner.
    match w.terrain_height_at(&pos) {
        Some(h) if (f64::from(h) - row.height).abs() <= HEIGHT_TOL => {}
        Some(h) => fails.push(format!("terrain height {h:.3} ≠ {:.2}", row.height)),
        None => fails.push("no terrain height".into()),
    }
    match bundle.terrain_height(row.corner[0] as f32, row.corner[1] as f32) {
        Some(h) if (f64::from(h) - row.corner_height).abs() <= HEIGHT_TOL => {}
        Some(h) => fails.push(format!(
            "terrain at corner {h:.3} ≠ {:.2}",
            row.corner_height
        )),
        None => fails.push("no terrain height at corner".into()),
    }

    // 3. Map coordinates and `/loc` tokens, as the HUD forms them.
    match w.map_coords(&pos) {
        Some(m) => {
            let s = m.to_string();
            if s != row.map && Some(s.as_str()) != row.map_alt {
                fails.push(format!("map coordinates {s} ≠ {}", row.map));
            }
        }
        None => fails.push("no map coordinates".into()),
    }
    let local = LocalPos {
        block: key,
        local: [row.stand[0] as f32, row.stand[1] as f32, row.height as f32],
    };
    let loc = LocReport::from_local(&local, None).to_string();
    if loc != row.loc {
        fails.push(format!("/loc {loc} ≠ {}", row.loc));
    }
    // The walker's `--place "<loc line>"` must land on the stand point.
    match row.loc.parse::<LocReport>() {
        Ok(r) => {
            let p = r.to_world();
            if p.region != row.region || p.distance_xy(&pos) > 0.01 {
                fails.push(format!("/loc parses to {p:?}"));
            }
        }
        Err(e) => fails.push(format!("/loc does not parse: {e}")),
    }

    // 4. Area.
    if bundle.block_props.area != Some(Did(row.area)) {
        fails.push(format!(
            "Area_DID {:?} ≠ {:#010x}",
            bundle.block_props.area, row.area
        ));
    }
    match w.area_at(&pos) {
        Some(a) if a.area == Did(row.area) && a.display().is_some_and(|n| !n.is_empty()) => {}
        Some(a) => fails.push(format!("area name of {:#010x} did not resolve", a.area.0)),
        None => fails.push("no area name".into()),
    }

    // 2. Anchor placement.
    let Some(e) = bundle.entities.iter().find(|e| e.index == row.sed) else {
        fails.push(format!("SED #{} not placed", row.sed));
        return fails;
    };
    if e.object != Some(Did(row.anchor)) {
        fails.push(format!("SED object {:?} ≠ {:#010x}", e.object, row.anchor));
    }
    if e.iid.map(|i| i.0) != Some(row.iid) {
        fails.push(format!("SED IID {:?} ≠ {:#018x}", e.iid, row.iid));
    }
    if !e.draw || e.interior {
        fails.push(format!(
            "anchor not drawn (draw {}, interior {})",
            e.draw, e.interior
        ));
    }
    if e.scale.iter().any(|s| (s - row.scale).abs() > SCALE_TOL) {
        fails.push(format!("scale {:?} ≠ {}", e.scale, row.scale));
    }
    let psi = yaw(e.placement.rotation).to_degrees().rem_euclid(360.0);
    let dpsi = (psi - row.yaw_deg).rem_euclid(360.0);
    if dpsi.min(360.0 - dpsi) > YAW_TOL_DEG {
        fails.push(format!("yaw {psi:.2}° ≠ {}°", row.yaw_deg));
    }
    let p = e.placement.position;
    if (0..3).any(|i| (f64::from(p[i]) - row.sed_pos[i]).abs() > POS_TOL) {
        fails.push(format!("SED position {p:?} ≠ {:?}", row.sed_pos));
    }
    let Some(visual) = e.visual.as_ref().filter(|v| v.has_geometry()) else {
        fails.push("anchor has no geometry".into());
        return fails;
    };
    let (lo, hi) = match part_bounds(set, &visual.parts) {
        Ok(b) => b,
        Err(err) => {
            fails.push(err);
            return fails;
        }
    };
    // The walker's instance transform (scale · rotation · translation, `walker.md` §4).
    let q = e.placement.rotation;
    let model = Mat4::from_scale_rotation_translation(
        Vec3::from_array(e.scale),
        Quat::from_xyzw(q.x, q.y, q.z, q.w).normalize(),
        Vec3::from_array(p),
    );
    let corners: Vec<Vec3> = (0..8)
        .map(|i| {
            model.transform_point3(Vec3::new(
                if i & 1 == 0 { lo[0] } else { hi[0] },
                if i & 2 == 0 { lo[1] } else { hi[1] },
                if i & 4 == 0 { lo[2] } else { hi[2] },
            ))
        })
        .collect();
    let bottom = corners.iter().map(|c| c.z).fold(f32::INFINITY, f32::min);
    let top = corners
        .iter()
        .map(|c| c.z)
        .fold(f32::NEG_INFINITY, f32::max);
    if (f64::from(bottom) - row.bottom_top[0]).abs() > CORNER_TOL
        || (f64::from(top) - row.bottom_top[1]).abs() > CORNER_TOL
    {
        fails.push(format!(
            "mesh bottom/top {bottom:.3}/{top:.3} ≠ {:.2}/{:.2}",
            row.bottom_top[0], row.bottom_top[1]
        ));
    }
    let nearest = corners[..4]
        .iter()
        .map(|c| {
            let (dx, dy) = (
                f64::from(c.x) - row.corner[0],
                f64::from(c.y) - row.corner[1],
            );
            (dx * dx + dy * dy).sqrt()
        })
        .fold(f64::INFINITY, f64::min);
    if nearest > CORNER_TOL {
        fails.push(format!(
            "nearest bottom corner {nearest:.3} m from ({:.2}, {:.2})",
            row.corner[0], row.corner[1]
        ));
    }
    fails
}

#[test]
fn reference_locations_match_the_data() {
    let Some(dir) = install() else { return };
    let set = Arc::new(ArchiveSet::open(&dir, SetOptions::default()).expect("open archive set"));
    let names =
        WStateAreaNames::open(Arc::clone(&set), &dir, Some("English")).expect("language archive");
    let ctx = Arc::new(
        WorldContext::from_install(Arc::clone(&set), BundleOptions::default())
            .expect("world context")
            .with_area_names(Arc::new(names)),
    );
    let pool = WorkerPool::new(PoolConfig::default());
    let mut w = WorldStreamer::new(ctx, &pool, StreamConfig::default()).expect("streamer");

    let mut failed = Vec::new();
    for row in &ROWS {
        let key = LandblockKey::new(row.region, (row.block >> 8) as u8, (row.block & 0xFF) as u8)
            .expect("row landblock");
        let o = key.origin();
        let pos = WorldPos::new(
            row.region,
            o[0] + row.stand[0],
            o[1] + row.stand[1],
            row.height,
        );
        assert!(
            w.settle(pos, Duration::from_secs(120)),
            "{}: never settled",
            row.id
        );
        let fails = check_row(&set, &w, row);
        if fails.is_empty() {
            let map = w
                .map_coords(&pos)
                .map(|m| m.to_string())
                .unwrap_or_default();
            let form = if map == row.map {
                "rounded"
            } else {
                "truncated"
            };
            eprintln!(
                "{} r{} {:#06x}: pass (map {map}, {form} form)",
                row.id, row.region, row.block
            );
        } else {
            eprintln!(
                "{} r{} {:#06x}: FAIL: {}",
                row.id,
                row.region,
                row.block,
                fails.join("; ")
            );
            failed.push(row.id);
        }
    }
    assert!(failed.is_empty(), "rows failing: {failed:?}");
}
