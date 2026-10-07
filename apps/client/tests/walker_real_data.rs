// SPDX-License-Identifier: GPL-3.0-or-later
//! The world walker against the user's own install, headless (read-only; skipped unless
//! `TESSERA_GAME_DIR` is set, or when no GPU adapter exists).
//!
//! - `screenshot_at_spawn`: `--screenshot` at the default spawn; the area must settle and
//!   the image must show more than one colour.
//! - `place_by_map_coordinates`: `--place` with map coordinates (`coordinates.md` §8.4 B)
//!   must put the viewer exactly there.
//! - `flight_across_a_region`: `--fly-path` along a landblock row at 1,000 m/s with a
//!   frame log. A release build flies the full region (`cargo test --release -p
//!   tessera-client --test walker_real_data -- --nocapture`) and must have no near-ring
//!   holes; a debug build flies 20 landblocks and only checks that the flight completes and
//!   residency stays bounded. The frame-time criterion (no frame over 50 ms, ROADMAP M3
//!   exit criterion 2) is asserted only in release builds with `TESSERA_PERF=1`, because
//!   it fails whenever other work loads the machine; the summary with the stall count is
//!   printed either way.
//!
//! Only counts, timings and sizes are printed; images go to a temporary directory. The
//! three walkers run one at a time (a lock in `run`), so the flight's frame times are not
//! measured while the other tests stream and render in parallel.

use clap::Parser as _;
use tessera_client::Args;
use tessera_client::headless::{self, HeadlessReport};
use tessera_render::RenderError;

/// `TESSERA_PERF=1`: also assert frame-time budgets (needs an otherwise idle machine).
fn perf() -> bool {
    std::env::var_os("TESSERA_PERF").is_some_and(|v| v == "1")
}

/// Serialises the walkers of this binary (the test harness runs tests in parallel).
static WALKER: std::sync::Mutex<()> = std::sync::Mutex::new(());

fn run(extra: &[&str]) -> Option<HeadlessReport> {
    // A panic in another test poisons the lock; the walker itself is still usable.
    let _one_at_a_time = WALKER
        .lock()
        .unwrap_or_else(std::sync::PoisonError::into_inner);
    let Some(dir) = std::env::var_os("TESSERA_GAME_DIR").filter(|d| !d.is_empty()) else {
        eprintln!("TESSERA_GAME_DIR not set; skipping");
        return None;
    };
    let data = tempfile::tempdir().expect("tempdir");
    // Default start-up (no `--no-entry-count`): walker runs skip the showcase work.
    let mut argv: Vec<String> = [
        "tessera-client",
        "--size",
        "320x180",
        "--game-dir",
        dir.to_str().expect("utf-8 game dir"),
        "--data-dir",
        data.path().to_str().expect("utf-8 temp path"),
    ]
    .iter()
    .map(|s| (*s).to_owned())
    .collect();
    argv.extend(extra.iter().map(|s| (*s).to_owned()));
    let args = Args::try_parse_from(argv).expect("valid args");
    match headless::run(&args) {
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
fn screenshot_at_spawn() {
    let out = tempfile::tempdir().expect("tempdir");
    let png = out.path().join("spawn.png");
    let Some(report) = run(&["--screenshot", png.to_str().unwrap(), "--fly"]) else {
        return;
    };
    eprintln!(
        "{} frames, settled {}, at {:?}",
        report.frames, report.settled, report.description
    );
    assert!(report.settled, "the spawn area did not settle");
    let decoder = png::Decoder::new(std::io::BufReader::new(std::fs::File::open(&png).unwrap()));
    let mut reader = decoder.read_info().expect("png header");
    let mut buf = vec![0; reader.output_buffer_size().expect("size")];
    let info = reader.next_frame(&mut buf).expect("png data");
    assert_eq!((info.width, info.height), (320, 180));
    let px = &buf[..info.buffer_size()];
    let first = &px[..4];
    let distinct = px.as_chunks::<4>().0.iter().filter(|p| *p != first).count();
    assert!(distinct > 1000, "image looks blank");
}

#[test]
fn place_by_map_coordinates() {
    let out = tempfile::tempdir().expect("tempdir");
    let png = out.path().join("place.png");
    let Some(report) = run(&[
        "--screenshot",
        png.to_str().unwrap(),
        "--place",
        "12.3S, 45.6W",
        "--settle-timeout",
        "60",
    ]) else {
        return;
    };
    let d = report.description.expect("placed");
    eprintln!(
        "{} frames, settled {}, at {d}",
        report.frames, report.settled
    );
    assert!(d.contains("12.3S, 45.6W"), "{d}");
    assert!(d.contains("ox80.00 oy20.00"), "{d}");
    assert!(png.exists());
}

#[test]
fn flight_across_a_region() {
    let release = !cfg!(debug_assertions);
    let path = if release {
        "1:0:118,1:254:118"
    } else {
        "1:100:118,1:120:118"
    };
    let log_dir = tempfile::tempdir().expect("tempdir");
    let log = log_dir.path().join("frames.csv");
    let Some(report) = run(&[
        "--fly-path",
        path,
        "--speed",
        "1000",
        "--frame-log",
        log.to_str().unwrap(),
        "--residency-mib",
        "512",
    ]) else {
        return;
    };
    let s = report.summary.expect("flight summary");
    eprintln!("flight {path}: {s}");
    assert!(s.frames > 10);
    assert!(log.exists());
    // Bundles: the budget plus the always-admitted near ring.
    assert!(s.peak_resident_bytes < (512 + 128) << 20, "{s}");
    if release {
        assert_eq!(s.near_missing_frames, 0, "{s}");
        if perf() {
            assert_eq!(s.stalls, 0, "{s}");
        } else if s.stalls > 0 {
            eprintln!(
                "{} frames over 50 ms (not asserted; set TESSERA_PERF=1)",
                s.stalls
            );
        }
    }
}
