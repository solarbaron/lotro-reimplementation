// SPDX-License-Identifier: GPL-3.0-or-later
//! The walker's interior visibility (`landblock.md` §4.11–§4.12, `walker.md` §14.12) against
//! a user's own install (read-only; skipped unless `TESSERA_GAME_DIR` is set).
//!
//! - `visibility_census`: every landblock-info record's cells partitioned into visibility
//!   cells and portals by [`BlockVisibility`], with the totals printed (§4.11 OBS2 gives
//!   104,271 cells, 18,390 visibility cells and 39,692 portals for the spec's install; the
//!   install patched on 2026-10-27 has 105,424, 19,543 and 43,946, so they are not pinned),
//!   and on every record LI7 (every neighbour in the PVS) and LI8 (no outdoor-walk entry, so
//!   the outdoor scene flags stay set).
//! - `in1_example`: the worked example of §4.11–§4.12 (landblock info 0x802191A6): group 1
//!   is one visibility cell whose only portals lead from cell 6 into cell 5 and from cell 8
//!   into cell 24; the PVS of cell 9 has 25 cells, none with the sky bit; neither the
//!   landscape nor the sky is drawn from cell 9. With the world's collision set the
//!   landblock's 26 portals all have their polygons (`walker.md` §14.12). The Y5 eye (the
//!   IN1 start + 1.8 m) lies in cell 9 by the walker's camera-cell lookup
//!   ([`camera_cell_at`], `walker.md` §14.12 (a)), so from it neither landscape nor sky is
//!   drawn; looking east, the portal walk draws the camera's own visibility cell.
//!
//! Run with `cargo test --release -p tessera-client --test interiors_real_data --
//! --nocapture`. Only counts and cell numbers are printed.

use std::collections::BTreeSet;
use std::path::PathBuf;
use std::sync::Arc;
use std::sync::atomic::{AtomicUsize, Ordering};

use tessera_assets::{PoolConfig, WorkerPool};
use tessera_client::walker::camera_cell_at;
use tessera_client::walker::interiors::{
    BlockVisibility, SceneFlags, has_outdoor_entry, outdoor_flags,
};
use tessera_dat::{ArchiveKind, ArchiveSet, SetOptions};
use tessera_formats::{Did, LandblockInfo};
use tessera_render::glam::Vec3;
use tessera_world::{
    BundleOptions, LandblockKey, RingConfig, StreamConfig, WorldContext, WorldPos, WorldSource,
    WorldStreamer,
};

/// Settling the streamer waits as long as loading progresses: it gives up after this many
/// updates (about 1 ms apart, longer on a loaded machine) in a row without any loading
/// work starting or finishing, or after this many updates in all
/// ([`tessera_world::WorldStreamer::settle_on_progress`]; `walker.md` §7, implementer note
/// on progress waits).
const SETTLE_STALL: u64 = if cfg!(debug_assertions) {
    900_000
} else {
    120_000
};
/// See [`SETTLE_STALL`].
const SETTLE_MAX: u64 = if cfg!(debug_assertions) {
    14_400_000
} else {
    3_600_000
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

fn open() -> Option<(Arc<ArchiveSet>, Arc<WorldContext>)> {
    let dir = install()?;
    let set = Arc::new(ArchiveSet::open(&dir, SetOptions::default()).expect("open archive set"));
    let ctx = Arc::new(
        WorldContext::from_install(Arc::clone(&set), BundleOptions::default())
            .expect("world context"),
    );
    Some((set, ctx))
}

fn read_info(ctx: &WorldContext, did: u32) -> Option<LandblockInfo> {
    let (bytes, stamp) = ctx.source().read_stamped(Did(did)).ok()??;
    LandblockInfo::parse(&bytes, stamp, ctx.schema()).ok()
}

#[derive(Default)]
struct Tally {
    records: usize,
    cells: usize,
    vcells: usize,
    portals: usize,
    li7: usize,
    entries: usize,
    flags: usize,
}

#[test]
fn visibility_census() {
    let Some((set, ctx)) = open() else { return };
    let mut infos = BTreeSet::new();
    for m in set.members() {
        let ArchiveKind::Cell(r) = m.id.kind else {
            continue;
        };
        let lo = 0x8020_0000 | (r << 16);
        for e in m.archive.entries_range(lo..=lo | 0xFFFF) {
            infos.insert(e.expect("directory walk").did);
        }
    }
    // Every record, also in debug builds (`walker.md` §14.12: LI7 and LI8 on every record).
    let infos: Vec<u32> = infos.into_iter().collect();
    let next = AtomicUsize::new(0);
    let tallies = std::sync::Mutex::new(Vec::new());
    let threads = std::thread::available_parallelism().map_or(4, |n| n.get());
    std::thread::scope(|s| {
        for _ in 0..threads {
            s.spawn(|| {
                let mut t = Tally::default();
                loop {
                    let i = next.fetch_add(1, Ordering::Relaxed);
                    let Some(&did) = infos.get(i) else { break };
                    let Some(info) = read_info(&ctx, did) else {
                        continue;
                    };
                    t.records += 1;
                    if info.cells().is_empty() {
                        continue;
                    }
                    let v = BlockVisibility::build(&info, |_, _| Vec::new());
                    t.cells += info.cells().len();
                    t.vcells += v.visibility_cells() as usize;
                    t.portals += v.portals().len();
                    t.entries += usize::from(has_outdoor_entry(&info));
                    if outdoor_flags([&v]) != SceneFlags::OUTDOORS {
                        t.flags += 1;
                    }
                    for c in info.cells() {
                        let pvs = v.pvs(c.index);
                        if c.neighbours.iter().any(|n| !pvs.contains(&n.cell)) {
                            t.li7 += 1;
                        }
                    }
                }
                tallies.lock().unwrap().push(t);
            });
        }
    });
    let mut t = Tally::default();
    for x in tallies.into_inner().unwrap() {
        t.records += x.records;
        t.cells += x.cells;
        t.vcells += x.vcells;
        t.portals += x.portals;
        t.li7 += x.li7;
        t.entries += x.entries;
        t.flags += x.flags;
    }
    eprintln!(
        "{} of {} landblock infos: {} cells, {} visibility cells, {} portals; LI7 failures {}, \
         records with an outdoor-walk entry {}",
        t.records,
        infos.len(),
        t.cells,
        t.vcells,
        t.portals,
        t.li7,
        t.entries
    );
    assert_eq!(t.records, infos.len(), "every record decodes");
    assert_eq!(t.li7, 0, "LI7: every neighbour lies in the PVS");
    assert_eq!(t.entries, 0, "LI8: no cell has the outdoor-walk bit");
    assert_eq!(
        t.flags, 0,
        "outdoors both scene flags stay set (§4.12 step 1)"
    );
    // The totals move with every content patch (printed above); what holds on any install:
    assert!(t.cells > 0, "cells (OBS2)");
    assert!(
        t.vcells > 0 && t.vcells <= t.cells,
        "visibility cells partition the cells (§4.11)"
    );
    assert!(t.portals > 0, "portals (§4.11)");
}

#[test]
fn in1_example() {
    let Some((_, ctx)) = open() else { return };
    let info = read_info(&ctx, 0x8021_91A6).expect("landblock info 0x802191A6");
    let v = BlockVisibility::build(&info, |_, _| Vec::new());
    let group = v.vcell_of(6).expect("cell 6");
    for c in 6..=23 {
        assert_eq!(v.vcell_of(c), Some(group), "cell {c} is in group 1");
    }
    let from_group: Vec<(u32, u32)> = v
        .portals()
        .iter()
        .filter(|p| p.from == group)
        .map(|p| (p.from, p.to))
        .collect();
    let expect: BTreeSet<(u32, u32)> = [
        (group, v.vcell_of(5).expect("cell 5")),
        (group, v.vcell_of(24).expect("cell 24")),
    ]
    .into();
    assert_eq!(
        from_group.iter().copied().collect::<BTreeSet<_>>(),
        expect,
        "only 6→5 and 8→24 leave the group"
    );
    assert_eq!(from_group.len(), 2);
    let pvs = v.pvs(9);
    let want: Vec<u16> = (2..=25).chain([31]).collect();
    assert_eq!(pvs, want.as_slice(), "PVS of cell 9");
    assert_eq!(v.scene_flags(9), SceneFlags::CLOSED);

    // With the streamer's collision set the portal polygons are known.
    let pool = WorkerPool::new(PoolConfig::default());
    let mut streamer = WorldStreamer::new(
        Arc::clone(&ctx),
        &pool,
        StreamConfig {
            rings: RingConfig {
                near: 160.0,
                mid: 160.0,
                far: 160.0,
                hysteresis: 160.0,
            },
            ..StreamConfig::default()
        },
    )
    .expect("streamer");
    let key = LandblockKey::new(1, 0x91, 0xA6).expect("key");
    let start = WorldPos::new(
        1,
        key.origin()[0] + 40.86,
        key.origin()[1] - 108.12,
        -212.90,
    );
    assert!(
        streamer.settle_on_progress(start, SETTLE_STALL, SETTLE_MAX),
        "never settled"
    );
    let bundle = Arc::clone(streamer.bundle(key).expect("IN1 bundle"));
    let col = bundle.collision.as_deref().expect("collision set");
    let (_, cell) = streamer
        .cell_at(&WorldPos::new(1, start.x, start.y, start.z + 0.05))
        .expect("the IN1 start is in a cell");
    assert_eq!(cell.index, 9, "IN1 starts in cell 9");
    let v = BlockVisibility::build(&info, |c, k| {
        col.cell(c)
            .and_then(|c| c.portals.get(k))
            .map(|p| p.corners.clone())
            .unwrap_or_default()
    });
    let with_corners = v.portals().iter().filter(|p| p.corners.len() >= 3).count();
    eprintln!(
        "IN1: {} visibility cells, {} portals ({} with polygons)",
        v.visibility_cells(),
        v.portals().len(),
        with_corners
    );
    assert_eq!(
        v.portals().len(),
        26,
        "the IN1 landblock's portals (§14.12)"
    );
    assert_eq!(
        with_corners,
        v.portals().len(),
        "every portal has its polygon"
    );
    // The Y5 eye: the IN1 start + 1.8 m, heading 90 (east, the IN1 walk's way), pitch 0,
    // given its cell by the walker's camera-cell lookup (no mover: the eye alone).
    let eye_w = WorldPos::new(1, start.x, start.y, start.z + 1.8);
    let camera_cell = camera_cell_at(&streamer, &eye_w, None);
    eprintln!("Y5 eye: camera cell {camera_cell:?}");
    assert_eq!(
        camera_cell,
        Some((key, 9)),
        "the Y5 eye is in cell 9 (`walker.md` §14.12)"
    );
    assert_eq!(
        v.scene_flags(9),
        SceneFlags::CLOSED,
        "no landscape, no sky from the Y5 eye"
    );
    let eye = Vec3::new(40.86, -108.12, -212.90 + 1.8);
    let camera = tessera_render::Camera {
        eye,
        target: eye + Vec3::X,
        up: Vec3::Z,
        projection: tessera_render::Perspective {
            fov_y: std::f32::consts::FRAC_PI_4,
            near: 0.1,
            far: 4_000.0,
        },
    };
    let seen = v.visible_cells(9, eye, &camera.view_proj(16.0 / 9.0));
    let n = seen.iter().filter(|x| **x).count();
    eprintln!("Y5 eye: {n} visibility cells drawn");
    assert!(seen[group as usize], "the camera's own visibility cell");
}
