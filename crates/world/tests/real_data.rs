// SPDX-License-Identifier: GPL-3.0-or-later
//! `tessera-world` against a user's own install (read-only). Skipped unless
//! `TESSERA_GAME_DIR` points at an install.
//!
//! - `context_and_bundles`: loads the world context (dictionary, REGION map, region and
//!   block-map descriptors), checks CO4 (`coordinates.md` §13), builds bundles around a
//!   few landblocks and resolves an area name through `tessera-wstate`.
//! - `stream_straight_line_across_region`: flies a straight line across a full region at
//!   high speed with the streamer, headless, paced at 60 frames per second, and reports
//!   load times, peak residency and the longest main-thread update (ROADMAP M3 exit
//!   criterion 2: no frame over 50 ms, residency under the configured budget).
//!
//! Tunables (environment): `TESSERA_WALK_REGION` (default 1), `TESSERA_WALK_Y` (landblock
//! row; default: the row with the most landblock-info records), `TESSERA_WALK_SPEED`
//! (units per second, default 1,000), `TESSERA_WALK_BUDGET_MB` (residency budget, default
//! 512). A debug build flies 40 landblocks unless `TESSERA_FULL=1`; run
//! `cargo test --release -p tessera-world --test real_data -- --nocapture` for the full
//! line.
//!
//! Only counts, DIDs, timings and sizes are printed; no game data leaves the install.

use std::path::PathBuf;
use std::sync::Arc;
use std::time::{Duration, Instant};

use tessera_assets::{PoolConfig, WorkerPool};
use tessera_dat::{ArchiveKind, ArchiveSet, SetOptions};
use tessera_world::{
    BundleOptions, BundleSource, LandblockKey, RingConfig, StreamConfig, WStateAreaNames,
    WorldContext, WorldPos, WorldStreamer, build_bundle,
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

fn env_num<T: std::str::FromStr>(name: &str) -> Option<T> {
    std::env::var(name).ok()?.parse().ok()
}

fn full() -> bool {
    !cfg!(debug_assertions) || std::env::var_os("TESSERA_FULL").is_some_and(|v| v == "1")
}

/// Resident set size of this process in bytes (Linux; `None` elsewhere).
fn rss() -> Option<usize> {
    let s = std::fs::read_to_string("/proc/self/status").ok()?;
    let line = s.lines().find(|l| l.starts_with("VmRSS:"))?;
    let kb: usize = line.split_whitespace().nth(1)?.parse().ok()?;
    Some(kb * 1024)
}

fn open(dir: &PathBuf) -> Arc<ArchiveSet> {
    Arc::new(ArchiveSet::open(dir, SetOptions::default()).expect("open archive set"))
}

/// Landblock-info counts per row Y of `region`, from the directory (no record reads).
fn info_rows(set: &ArchiveSet, region: u8) -> [u32; 255] {
    let mut rows = [0u32; 255];
    for m in set.members() {
        if m.id.kind != ArchiveKind::Cell(u32::from(region)) {
            continue;
        }
        let lo = 0x8020_0000 | (u32::from(region) << 16);
        for e in m.archive.entries_range(lo..=lo | 0xFFFF) {
            let e = e.expect("directory walk");
            let y = (e.did & 0xFF) as usize;
            if y < 255 {
                rows[y] += 1;
            }
        }
    }
    rows
}

#[test]
fn context_and_bundles() {
    let Some(dir) = install() else { return };
    let set = open(&dir);
    let t = Instant::now();
    let names =
        WStateAreaNames::open(Arc::clone(&set), &dir, Some("English")).expect("language archive");
    let ctx = WorldContext::from_install(Arc::clone(&set), BundleOptions::default())
        .expect("world context")
        .with_area_names(Arc::new(names));
    eprintln!(
        "context: {} regions in {:?}",
        ctx.regions().len(),
        t.elapsed()
    );
    for (r, info) in ctx.regions() {
        let m = info.map.expect("map constants");
        // CO4: every region descriptor stores origin (1,468, 1,244) landcells, scale 0.005.
        assert_eq!(m.origin, [29_360.0, 24_880.0], "region {r}");
        // The f32 0.005 widens through its shortest decimal to exactly 0.005.
        assert_eq!(m.scale, 0.005, "region {r}");
        // Centre of the §9 example landcell: lat -3.95, lon -66.55 round away from zero.
        assert_eq!(
            m.to_map([16_050.0, 24_090.0]).to_string(),
            "4.0S, 66.6W",
            "region {r}"
        );
        eprintln!(
            "  region {r}: descriptor {:#010x}, block-map descriptor {}",
            info.descriptor.map_or(0, |d| d.did.0),
            info.block_map_descriptor
                .as_ref()
                .map_or("none".into(), |d| format!(
                    "{:#010x} ({} keys)",
                    d.did.0,
                    d.entries.len()
                ))
        );
    }
    for r in [1u8, 2, 3, 4, 5, 14] {
        assert!(
            ctx.region(r)
                .is_some_and(|i| i.block_map_descriptor.is_some()),
            "region {r}"
        );
    }

    // Bundles along the busiest row of region 1.
    let rows = info_rows(&set, 1);
    let y = (0..255).max_by_key(|&y| rows[y]).unwrap() as u8;
    let src = BundleSource::new(Arc::clone(ctx.source()));
    let (mut n, mut entities, mut drawn, mut areas, mut minimaps, mut problems) =
        (0, 0, 0, 0, 0, 0);
    let mut max_t = Duration::ZERO;
    let mut bytes = 0usize;
    for x in (0..=254u8).step_by(3) {
        let k = LandblockKey::new(1, x, y).unwrap();
        let frame = tessera_assets::RecordSource::read_record(&src, k.data_did().0).expect("frame");
        let t = Instant::now();
        let b = build_bundle(&ctx, k.data_did(), &frame, &|| false).expect("bundle");
        max_t = max_t.max(t.elapsed());
        n += 1;
        entities += b.entities.len();
        drawn += b.drawable().count();
        areas += usize::from(b.area.as_ref().and_then(|a| a.display()).is_some());
        minimaps += usize::from(b.minimap.is_some());
        problems += b.problems.len();
        bytes += b.heap_bytes;
        if b.block_props.area.is_some() {
            assert!(b.area.is_some(), "{k}: area DID did not resolve");
        }
    }
    let os = ctx.objects().stats();
    eprintln!(
        "row y={y}: {n} bundles, {entities} entities ({drawn} drawn), {areas} named areas, \
         {minimaps} minimap tiles, {problems} problems, {:.1} KiB mean, slowest build {max_t:?}; \
         objects resolved {} failed {} cache hits {}",
        bytes as f64 / n as f64 / 1024.0,
        os.resolved,
        os.failed,
        os.hits
    );
    assert_eq!(problems, 0);
    assert!(areas > 0 && minimaps > 0 && drawn > 0);
}

#[test]
fn stream_straight_line_across_region() {
    let Some(dir) = install() else { return };
    let set = open(&dir);
    let region: u8 = env_num("TESSERA_WALK_REGION").unwrap_or(1);
    let rows = info_rows(&set, region);
    let y: u8 = env_num("TESSERA_WALK_Y")
        .unwrap_or_else(|| (0..255).max_by_key(|&y| rows[y]).unwrap() as u8);
    let speed: f64 = env_num("TESSERA_WALK_SPEED").unwrap_or(1_000.0);
    let budget_mb: usize = env_num("TESSERA_WALK_BUDGET_MB").unwrap_or(512);
    let names =
        WStateAreaNames::open(Arc::clone(&set), &dir, Some("English")).expect("language archive");
    let ctx = Arc::new(
        WorldContext::from_install(Arc::clone(&set), BundleOptions::default())
            .expect("world context")
            .with_area_names(Arc::new(names)),
    );
    let pool = WorkerPool::new(PoolConfig::default());
    let config = StreamConfig {
        rings: RingConfig::default(),
        residency_budget: budget_mb << 20,
        ..StreamConfig::default()
    };
    let mut w = WorldStreamer::new(Arc::clone(&ctx), &pool, config.clone()).expect("streamer");

    let gy = f64::from(y) * 160.0 + 80.0;
    let (x0, x1) = if full() {
        (80.0, 254.0 * 160.0 + 80.0)
    } else {
        (100.0 * 160.0, 140.0 * 160.0)
    };
    let dt = Duration::from_micros(16_667);
    let step = speed * dt.as_secs_f64();
    eprintln!(
        "region {region}, row y={y} ({} info records), x {x0}..{x1}, {speed} u/s, {} workers, \
         budget {budget_mb} MiB",
        rows[usize::from(y)],
        pool.workers()
    );
    let start = WorldPos::new(region, x0, gy, 0.0);
    let rss0 = rss();
    let mut peak_rss = rss0.unwrap_or(0);
    let t = Instant::now();
    assert!(w.settle(start, Duration::from_secs(120)), "initial load");
    eprintln!(
        "initial load: {:?}, {} resident",
        t.elapsed(),
        w.resident().count()
    );

    let mut frame_times: Vec<Duration> = Vec::new();
    let mut max_near_missing = 0usize;
    let mut near_missing_frames = 0usize;
    let mut peak_total = 0usize;
    let mut x = x0;
    let wall = Instant::now();
    let mut next = Instant::now();
    while x <= x1 {
        let r = w.update(WorldPos::new(region, x, gy, 0.0));
        frame_times.push(r.main_thread);
        max_near_missing = max_near_missing.max(r.near_missing);
        near_missing_frames += usize::from(r.near_missing > 0);
        if frame_times.len().is_multiple_of(60) {
            peak_total = peak_total.max(w.sample_memory());
            peak_rss = peak_rss.max(rss().unwrap_or(0));
        }
        x += step;
        next += dt;
        let now = Instant::now();
        if next > now {
            std::thread::sleep(next - now);
        }
    }
    let wall = wall.elapsed();
    let end = WorldPos::new(region, x1, gy, 0.0);
    w.settle(end, Duration::from_secs(60));
    peak_total = peak_total.max(w.sample_memory());

    frame_times.sort_unstable();
    let p = |q: f64| frame_times[((frame_times.len() - 1) as f64 * q) as usize];
    let st = w.stats().clone();
    let store = w.store_stats();
    let os = ctx.objects().stats();
    let mib = |b: usize| b as f64 / f64::from(1 << 20);
    eprintln!(
        "frames {} in {wall:?}; update mean {:?} p50 {:?} p99 {:?} max {:?}",
        frame_times.len(),
        st.mean_frame(),
        p(0.5),
        p(0.99),
        st.max_frame
    );
    eprintln!(
        "bundles: requested {} integrated {} released {} evicted {} cancelled {} failed {}",
        st.requested, st.integrated, st.released, st.evicted, st.cancelled, st.failed
    );
    eprintln!(
        "jobs {}: mean {:?} max {:?} (read {:?} decode {:?} total); request→resident mean {:?} max {:?}",
        st.jobs,
        st.mean_job(),
        st.max_job,
        st.read_time,
        st.decode_time,
        st.mean_load_latency(),
        st.max_load_latency
    );
    eprintln!(
        "residency: peak {:.1} MiB in {} blocks (budget {budget_mb} MiB), peak with store cache \
         {:.1} MiB, mean bundle {:.1} KiB",
        mib(st.peak_resident_bytes),
        st.peak_resident_blocks,
        mib(peak_total),
        w.size_estimate() as f64 / 1024.0
    );
    if let Some(r0) = rss0 {
        eprintln!(
            "process RSS: {:.1} MiB before streaming, peak {:.1} MiB (+{:.1} MiB; includes \
             mapped archive pages and allocator slack)",
            mib(r0),
            mib(peak_rss),
            mib(peak_rss.saturating_sub(r0))
        );
    }
    eprintln!(
        "near-ring holes: {near_missing_frames} frames, at most {max_near_missing} blocks; \
         store hit rate {:.2}; objects resolved {} failed {} hits {}",
        store.hit_rate(),
        os.resolved,
        os.failed,
        os.hits
    );
    assert_eq!(st.failed, 0, "no bundle may fail");
    assert!(
        st.max_frame < Duration::from_millis(50),
        "an update took {:?}",
        st.max_frame
    );
    // The near ring may exceed the budget only by itself (always admitted).
    let near_allowance = 64 * w.size_estimate();
    assert!(
        st.peak_resident_bytes <= config.residency_budget + near_allowance,
        "peak residency {} over budget",
        st.peak_resident_bytes
    );
}
