// SPDX-License-Identifier: GPL-3.0-or-later
//! The drops of the weather code in a headless walker run against the user's own install
//! (read-only; skipped, passing, unless `TESSERA_GAME_DIR` is set, or when no GPU adapter
//! exists). Run with `cargo test --release -p tessera-client --test
//! weather_headless_real_data -- --nocapture`.
//!
//! `particle-runtime.md` §11.2.6 and §12 rule 17: while rain falls, the walker keeps a rain
//! drop set of ⌊3,000 × *r* × *q*⌋ drops (*q* = 0.6 at the built-in `AtmosphericsDetail`
//! `Medium`, headless runs read no preferences file) and draws one streak quad per drop in
//! use. The test first finds, with the walker's own environment code ([`WorldEnv`]), a game
//! day and time of day on which the outdoor sky key at reference location M3-03
//! (`docs/checklists/m3-reference-locations.md`) gives rain, then runs the headless walker
//! there with a stopped clock and asserts the headless report's weather counters: the
//! precipitation of the state in use, rain drops in use and streak vertices drawn. Only
//! counts and the chosen clock are printed; no record content.

use std::sync::Arc;
use std::time::Duration;

use clap::Parser as _;
use tessera_assets::{PoolConfig, WorkerPool};
use tessera_client::Args;
use tessera_client::headless::{self, HeadlessReport, SettleWait};
use tessera_client::walker::env::{ClockConfig, DEFAULT_DAY, WorldEnv, camera_sky_key};
use tessera_client::walker::fxparse::Systems;
use tessera_dat::{ArchiveSet, SetOptions};
use tessera_render::RenderError;
use tessera_render::glam::Vec3;
use tessera_world::environment::INITIAL_DAY_TYPE;
use tessera_world::{
    BundleOptions, LandblockKey, LocalPos, RingConfig, StreamConfig, WorldContext, WorldStreamer,
};

/// Frames per second of the paced headless loop.
const FPS: u64 = 60;

/// Game days searched for rain, from [`DEFAULT_DAY`] on.
const DAYS_SEARCHED: i64 = 720;

/// Times of day tried on each searched day.
const TIMES: [f64; 8] = [
    0.0625, 0.1875, 0.3125, 0.4375, 0.5625, 0.6875, 0.8125, 0.9375,
];

/// The least rain value searched for: at `Medium` it gives ⌊3,000 × 0.25 × 0.6⌋ = 450 drops.
const MIN_RAIN: f32 = 0.25;

/// The M3-03 stand point: region 1, landblock (109, 141), (62.96, 15.07) at 413.60 m.
const SPAWN: &str = "1,109,141,62.96,15.07";

/// A clock found to give rain: day type, day, time of day and the rain value.
#[derive(Debug, Clone, Copy)]
struct RainyClock {
    day_type: u32,
    day: i64,
    time_of_day: f64,
    rain: f32,
}

/// A day type, day and time of day on which the outdoor sky key at M3-03 gives rain of at
/// least [`MIN_RAIN`], with that rain value; `None` (skip) only when the landblock or the
/// sky library is missing. With both present a search that finds no rain fails the test:
/// the install is known to give rain there, so no rain is a regression of the walker's
/// environment code. The day types tried are the walker's built-in one and those
/// of the sky description's day groups (`environment.md` §11.2: the day description of a
/// key depends on the day type, which offline stays at the built-in value unless
/// `--day-type` sets it).
fn rainy_clock(dir: &std::ffi::OsStr) -> Option<RainyClock> {
    let set = Arc::new(ArchiveSet::open(dir, SetOptions::default()).expect("open archives"));
    let ctx = Arc::new(
        WorldContext::from_install(Arc::clone(&set), BundleOptions::default())
            .expect("world context"),
    );
    let key = LandblockKey::new(1, 109, 141).expect("key");
    if !ctx.has_landblock(key).expect("directory") {
        eprintln!("{key} not in this install; skipping");
        return None;
    }
    let Some(library) = ctx.systems().sky.clone() else {
        eprintln!("no sky library in this install; skipping");
        return None;
    };
    let pool = WorkerPool::new(PoolConfig {
        workers: std::thread::available_parallelism().map_or(4, |n| n.get()),
        ..PoolConfig::default()
    });
    let mut w = WorldStreamer::new(
        Arc::clone(&ctx),
        &pool,
        StreamConfig {
            rings: RingConfig {
                near: 200.0,
                mid: 240.0,
                far: 280.0,
                hysteresis: 40.0,
            },
            ..StreamConfig::default()
        },
    )
    .expect("streamer");
    let feet = LocalPos::new(key, [62.96, 15.07, 413.60]).to_world();
    assert!(w.settle(feet, Duration::from_secs(900)), "settles");
    let sky_key = camera_sky_key(&w, None, &feet);
    let systems = Systems::all();
    let mut most = 0.0f32;
    let mut descriptions = std::collections::HashSet::new();
    let mut day_types = vec![INITIAL_DAY_TYPE];
    for g in &library.sky().groups {
        if !day_types.contains(&g.day_type) {
            day_types.push(g.day_type);
        }
    }
    for &day_type in &day_types {
        for day in DEFAULT_DAY..DEFAULT_DAY + DAYS_SEARCHED {
            for time_of_day in TIMES {
                let mut env = WorldEnv::new(
                    Some(Arc::clone(&library)),
                    &ClockConfig {
                        day,
                        time_of_day,
                        rate: 0.0,
                        day_type,
                    },
                );
                let fe = env.update(
                    1.0 / 60.0,
                    Some(sky_key),
                    None,
                    &systems,
                    2_080.0,
                    Vec3::ZERO,
                    Vec3::X,
                    0.0,
                );
                most = most.max(fe.precipitation.rain);
                if let Some(d) = fe.day_description {
                    descriptions.insert(d);
                }
                if fe.precipitation.rain >= MIN_RAIN {
                    return Some(RainyClock {
                        day_type,
                        day,
                        time_of_day,
                        rain: fe.precipitation.rain,
                    });
                }
            }
        }
    }
    // The landblock and the sky library are there: a search that finds no rain is a
    // failure of the environment code under test, not a reason to skip.
    panic!(
        "no rain of at least {MIN_RAIN} at M3-03 ({} day types, {} day descriptions, most \
         rain {most:.3}) on {DAYS_SEARCHED} days from day {DEFAULT_DAY}",
        day_types.len(),
        descriptions.len()
    );
}

/// How the screenshot waits for the area to settle: as long as loading progresses, counted
/// in frames (`walker.md` §7).
fn settle_wait() -> SettleWait {
    let (stall_s, max_s) = if cfg!(debug_assertions) {
        (900, 4 * 3600)
    } else {
        (60, 3600)
    };
    SettleWait::Progress {
        stall_frames: stall_s * FPS,
        max_frames: max_s * FPS,
    }
}

/// A headless screenshot run at M3-03 facing west with `extra` arguments; `None` without a
/// GPU adapter.
fn run(dir: &std::ffi::OsStr, extra: &[String]) -> Option<HeadlessReport> {
    let data = tempfile::tempdir().expect("tempdir");
    let png = tempfile::tempdir().expect("tempdir");
    let shot = png.path().join("weather.png");
    let mut argv: Vec<String> = vec![
        "tessera-client".into(),
        "--size".into(),
        "320x180".into(),
        "--game-dir".into(),
        dir.to_str().expect("utf-8 game dir").into(),
        "--data-dir".into(),
        data.path().to_str().expect("utf-8 temp path").into(),
        "--screenshot".into(),
        shot.to_str().expect("utf-8 temp path").into(),
        "--spawn".into(),
        SPAWN.into(),
        "--heading".into(),
        "270".into(),
    ];
    argv.extend(extra.iter().cloned());
    let args = Args::try_parse_from(argv).expect("valid args");
    match headless::run_with(&args, settle_wait()) {
        Ok(r) => Some(r),
        Err(e) => {
            if e.chain().any(|c| {
                matches!(
                    c.downcast_ref::<RenderError>(),
                    Some(RenderError::NoAdapter { .. })
                )
            }) {
                eprintln!("skipping: no GPU adapter: {e:#}");
                return None;
            }
            panic!("walker run failed: {e:#}");
        }
    }
}

#[test]
fn rain_falls_in_a_headless_walk_at_m3_03() {
    let Some(dir) = std::env::var_os("TESSERA_GAME_DIR").filter(|d| !d.is_empty()) else {
        eprintln!("TESSERA_GAME_DIR not set; skipping");
        return;
    };
    let Some(RainyClock {
        day_type,
        day,
        time_of_day,
        rain,
    }) = rainy_clock(&dir)
    else {
        return;
    };
    println!(
        "M3-03: rain {rain:.3} on day {day} at time of day {time_of_day} (day type {day_type})"
    );
    let extra = [
        "--day-type".to_owned(),
        day_type.to_string(),
        "--day".to_owned(),
        day.to_string(),
        "--time-of-day".to_owned(),
        time_of_day.to_string(),
        "--time-rate".to_owned(),
        "0".to_owned(),
    ];
    let Some(report) = run(&dir, &extra) else {
        return;
    };
    let w = *report.weather();
    let p = report.precipitation.unwrap_or_default();
    println!(
        "M3-03 weather at the end: settled {}; rain {:.3}, snow {:.3}; {} rain drops, {} snow \
         drops, {} placed and {} drop spawns in the last frame; {} streak vertices in the last \
         frame, most {}",
        report.settled,
        p.rain,
        p.snow,
        w.rain_drops,
        w.snow_drops,
        w.drops_placed,
        w.drop_spawns,
        w.streak_vertices,
        w.max_streak_vertices,
    );
    // The walker's curves run on its material clock, so the value may differ a little from
    // the search's; rain must still fall.
    assert!(p.rain > 0.0, "the walker's state in use gives rain");
    // ⌊3,000 × r × 0.6⌋ at the built-in `Medium` (§11.2.6 step 1).
    #[allow(clippy::cast_possible_truncation, clippy::cast_sign_loss)]
    let expected = (3_000.0 * p.rain.clamp(0.0, 1.0) * 0.6).floor() as usize;
    assert_eq!(w.rain_drops, expected, "the rain set uses its count");
    assert!(w.rain_drops > 0, "rain drops are in use");
    assert!(
        w.streak_vertices > 0 && w.max_streak_vertices >= w.streak_vertices,
        "rain streaks are drawn"
    );
}
