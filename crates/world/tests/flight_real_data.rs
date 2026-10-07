// SPDX-License-Identifier: GPL-3.0-or-later
//! The streamer alone on a fast diagonal flight against a user's own install (read-only):
//! the near-ring holes of ROADMAP M4.5 criterion 5 (flight M3-b, `walker.md` §14.11)
//! without the renderer, in alternating modes (so that all see a similar machine load):
//! lookahead off, a fixed 1.5 s urgent path, a 1.5 s urgent path that follows slow bundle
//! jobs, and the default lookahead, which adds an early part out to 4 s
//! (`tessera_world::streamer` module docs, "Lookahead").
//!
//! Skipped unless `TESSERA_GAME_DIR` points at an install; ignored by default because it
//! flies for about 20 s per run. Run it with
//! `cargo test --release -p tessera-world --test flight_real_data -- --ignored --nocapture`.
//! `TESSERA_FLIGHT_RUNS` (rounds of every mode, default 1), `TESSERA_FLIGHT_MODES` (a
//! comma-separated subset of `off,fixed,adaptive,default`), `TESSERA_FLIGHT_SPEED` (units
//! per second, default 3,000) and `TESSERA_FLIGHT_REGION` (default 2) change the flight.
//! Results depend on the machine and its load: the test prints counts and asserts only
//! that the flight completes and that no bundle job failed (the synthetic regression test
//! is `streamer::tests::early_part_closes_the_hole_of_a_slow_bundle`). No game data
//! leaves the install.

use std::path::PathBuf;
use std::sync::Arc;
use std::time::{Duration, Instant};

use tessera_assets::{PoolConfig, WorkerPool};
use tessera_dat::{ArchiveSet, SetOptions};
use tessera_world::{
    BlockState, BundleOptions, FrillSettings, LANDBLOCK_SIZE, LandblockKey, Ring, RingConfig,
    StreamConfig, WorldContext, WorldPos, WorldStreamer,
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
    std::env::var(name).ok().and_then(|s| s.parse().ok())
}

/// What one flight saw.
#[derive(Debug, Default)]
struct Flight {
    frames: usize,
    hole_frames: usize,
    max_holes: usize,
    max_ahead: usize,
    requested: u64,
    ahead_requested: u64,
    ahead_released: u64,
    max_latency: Duration,
    max_lookahead: Duration,
    slow_job: Duration,
    mean_latency: Duration,
    peak_mib: usize,
    mean_update: Duration,
    max_update: Duration,
    mean_corridor: Duration,
    max_corridor: Duration,
    /// Near-ring landblocks that were missing, with the frames they were missing in.
    missing: std::collections::BTreeMap<LandblockKey, usize>,
}

/// Flies from the centre of block (0, 0) to that of (254, 254) of `region` at `speed`,
/// paced at 60 updates per second, as the walker's `--fly-path` does.
fn fly(set: &Arc<ArchiveSet>, region: u8, speed: f64, mode: Mode) -> Flight {
    let ctx = Arc::new(
        WorldContext::from_install(Arc::clone(set), BundleOptions::default())
            .expect("world context"),
    );
    let pool = WorkerPool::new(PoolConfig::default());
    // The walker's rings with the default settings (`walker.md` §11.3: E = 2,080).
    let config = StreamConfig {
        rings: RingConfig {
            near: 3.0 * LANDBLOCK_SIZE,
            mid: 6.0 * LANDBLOCK_SIZE,
            far: 13.0 * LANDBLOCK_SIZE,
            hysteresis: LANDBLOCK_SIZE,
        },
        residency_budget: 4_096 << 20,
        frills: FrillSettings {
            distance_level: 2,
            ..FrillSettings::default()
        },
        lookahead: match mode {
            Mode::Off => Duration::ZERO,
            Mode::Fixed | Mode::Adaptive | Mode::Default => StreamConfig::default().lookahead,
        },
        lookahead_job_margin: match mode {
            Mode::Adaptive | Mode::Default => StreamConfig::default().lookahead_job_margin,
            Mode::Off | Mode::Fixed => 0.0,
        },
        // The modes before the early part: urgent path only, at most 6,400 units.
        lookahead_early: match mode {
            Mode::Default => StreamConfig::default().lookahead_early,
            Mode::Off | Mode::Fixed | Mode::Adaptive => Duration::ZERO,
        },
        lookahead_max: match mode {
            Mode::Default => StreamConfig::default().lookahead_max,
            Mode::Off | Mode::Fixed | Mode::Adaptive => 6_400.0,
        },
        ..StreamConfig::default()
    };
    let mut w = WorldStreamer::new(ctx, &pool, config).expect("streamer");
    let a = [80.0, 80.0];
    let b = [254.0 * LANDBLOCK_SIZE + 80.0; 2];
    let len = (b[0] - a[0]).hypot(b[1] - a[1]);
    let at = |s: f64| {
        let t = (s / len).clamp(0.0, 1.0);
        WorldPos::new(
            region,
            a[0] + t * (b[0] - a[0]),
            a[1] + t * (b[1] - a[1]),
            0.0,
        )
    };
    assert!(w.settle(at(0.0), Duration::from_secs(300)), "start area");
    let mut f = Flight::default();
    let dt = Duration::from_micros(16_667);
    let start = Instant::now();
    let mut next = start;
    loop {
        let s = speed * start.elapsed().as_secs_f64();
        let p = at(s);
        let r = w.update(p);
        let eye = WorldPos::new(region, p.x, p.y, 120.0);
        let _ = w.update_frills(&eye, &p);
        f.frames += 1;
        f.hole_frames += usize::from(r.near_missing > 0);
        f.max_holes = f.max_holes.max(r.near_missing);
        f.max_ahead = f.max_ahead.max(r.ahead);
        if r.near_missing > 0 {
            for (k, ring) in w.rings().members() {
                if *ring == Ring::Near && !matches!(w.state_of(*k), Some(BlockState::Ready(_))) {
                    *f.missing.entry(*k).or_default() += 1;
                }
            }
        }
        if s >= len {
            break;
        }
        next += dt;
        if let Some(wait) = next.checked_duration_since(Instant::now()) {
            std::thread::sleep(wait);
        }
    }
    let st = w.stats();
    f.requested = st.requested;
    f.ahead_requested = st.ahead_requested;
    f.ahead_released = st.ahead_released;
    f.max_latency = st.max_load_latency;
    f.max_lookahead = st.max_lookahead;
    f.slow_job = st.max_job;
    f.mean_latency = st.mean_load_latency();
    f.peak_mib = st.peak_resident_bytes >> 20;
    f.mean_update = st.mean_frame();
    f.max_update = st.max_frame;
    f.mean_corridor = st
        .corridor_time
        .checked_div(u32::try_from(f.frames).unwrap_or(u32::MAX).max(1))
        .unwrap_or_default();
    f.max_corridor = st.max_corridor_time;
    assert_eq!(st.failed, 0, "{st:?}");
    f
}

/// The lookahead of a flight.
#[derive(Debug, Clone, Copy)]
enum Mode {
    /// None.
    Off,
    /// A 1.5 s urgent path, not following job times, no early part.
    Fixed,
    /// A 1.5 s urgent path or 1.5 times the recent slow-job time, no early part.
    Adaptive,
    /// The default: [`Mode::Adaptive`] plus the early part out to 4 s.
    Default,
}

impl Mode {
    fn parse(s: &str) -> Option<Self> {
        match s.trim() {
            "off" => Some(Self::Off),
            "fixed" => Some(Self::Fixed),
            "adaptive" => Some(Self::Adaptive),
            "default" => Some(Self::Default),
            _ => None,
        }
    }
}

#[test]
#[ignore = "flies for about 20 s per run; run with --ignored"]
fn fast_flight_near_ring_holes() {
    let Some(dir) = install() else { return };
    // `RUST_LOG=tessera_world::streamer=debug` shows slow bundle jobs and late near-ring
    // landblocks.
    let _ = tracing_subscriber::fmt()
        .with_env_filter(tracing_subscriber::EnvFilter::from_default_env())
        .with_test_writer()
        .try_init();
    let set = Arc::new(ArchiveSet::open(&dir, SetOptions::default()).expect("open archive set"));
    let runs: usize = env_num("TESSERA_FLIGHT_RUNS").unwrap_or(1);
    let speed: f64 = env_num("TESSERA_FLIGHT_SPEED").unwrap_or(3_000.0);
    let region: u8 = env_num("TESSERA_FLIGHT_REGION").unwrap_or(2);
    let modes: Vec<Mode> = std::env::var("TESSERA_FLIGHT_MODES").map_or_else(
        |_| vec![Mode::Off, Mode::Fixed, Mode::Adaptive, Mode::Default],
        |s| s.split(',').filter_map(Mode::parse).collect(),
    );
    for run in 0..runs {
        for &mode in &modes {
            let f = fly(&set, region, speed, mode);
            eprintln!(
                "run {run}, lookahead {mode:?}: {} frames, {} with near-ring holes (at most \
                 {}), {} requested ({} ahead, {} released ahead, at most {} ahead at once), \
                 load latency mean {:?} max {:?}, longest job {:?}, longest horizon {:?}, \
                 peak {} MiB, update mean {:?} max {:?} (path mean {:?} max {:?})",
                f.frames,
                f.hole_frames,
                f.max_holes,
                f.requested,
                f.ahead_requested,
                f.ahead_released,
                f.max_ahead,
                f.mean_latency,
                f.max_latency,
                f.slow_job,
                f.max_lookahead,
                f.peak_mib,
                f.mean_update,
                f.max_update,
                f.mean_corridor,
                f.max_corridor
            );
            let mut worst: Vec<(usize, LandblockKey)> =
                f.missing.iter().map(|(k, n)| (*n, *k)).collect();
            worst.sort_unstable_by(|a, b| b.cmp(a));
            worst.truncate(12);
            eprintln!(
                "  {} landblocks missing; most frames: {}",
                f.missing.len(),
                worst
                    .iter()
                    .map(|(n, k)| format!("{k} {n}"))
                    .collect::<Vec<_>>()
                    .join(", ")
            );
        }
    }
}
