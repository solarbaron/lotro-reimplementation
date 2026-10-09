// SPDX-License-Identifier: GPL-3.0-or-later
//! The walker's far view (`far-view.md`, `--far-view on`) against the user's own install,
//! headless (read-only; skipped unless `TESSERA_GAME_DIR` is set, or when no GPU adapter
//! exists).
//!
//! `far_view_settles_at_two_places`: a `--screenshot` run with `--far-view on` and the
//! original's near field (no `--view-blocks`) at the default spawn and at a placed map
//! position. The area, the far view included, must settle; at the end the far view must
//! list sectors, have them all in, draw far terrain batches over landblocks outside the
//! near field, and have no job left. Only counts are printed; images go to a temporary
//! directory and are not compared.

use clap::Parser as _;
use tessera_client::Args;
use tessera_client::headless::{self, HeadlessReport};
use tessera_render::RenderError;

/// Serialises the walkers of this binary (the test harness runs tests in parallel).
static WALKER: std::sync::Mutex<()> = std::sync::Mutex::new(());

fn run(extra: &[&str]) -> Option<HeadlessReport> {
    let _one_at_a_time = WALKER
        .lock()
        .unwrap_or_else(std::sync::PoisonError::into_inner);
    let Some(dir) = std::env::var_os("TESSERA_GAME_DIR").filter(|d| !d.is_empty()) else {
        eprintln!("TESSERA_GAME_DIR not set; skipping");
        return None;
    };
    let data = tempfile::tempdir().expect("tempdir");
    let out = tempfile::tempdir().expect("tempdir");
    let png = out.path().join("far.png");
    let mut argv: Vec<String> = [
        "tessera-client",
        "--size",
        "320x180",
        "--game-dir",
        dir.to_str().expect("utf-8 game dir"),
        "--data-dir",
        data.path().to_str().expect("utf-8 temp path"),
        "--screenshot",
        png.to_str().expect("utf-8 temp path"),
        "--far-view",
        "on",
        "--fly",
        "--settle-timeout",
        "180",
    ]
    .iter()
    .map(|s| (*s).to_owned())
    .collect();
    argv.extend(extra.iter().map(|s| (*s).to_owned()));
    let args = Args::try_parse_from(argv).expect("valid args");
    assert!(args.far_view);
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

fn check(what: &str, report: &HeadlessReport) {
    let f = report
        .far_view
        .expect("far view counters with --far-view on");
    eprintln!(
        "{what}: {} frames, settled {}, at {:?}; far view {} / {} sectors, {} terrain batches over {} landblocks, {} sprites in {} batches, {} water quads, {} jobs",
        report.frames,
        report.settled,
        report.description,
        f.ready,
        f.listed,
        f.batches,
        f.covered,
        f.sprites,
        f.sprite_batches,
        f.water_quads,
        f.pending
    );
    assert!(
        report.settled,
        "{what}: the area (far view included) did not settle"
    );
    assert!(f.listed > 0, "{what}: no sector listed");
    assert!(f.ready <= f.listed);
    assert!(f.ready > 0, "{what}: no sector record in");
    assert!(f.batches > 0, "{what}: no far terrain batch drawn");
    // The original's near field is 3 × 3 landblocks; with F = 4 the listed square holds
    // 27 × 27, so far batches must cover much more than the near field.
    assert!(
        f.covered > 9,
        "{what}: far batches cover only {} landblocks",
        f.covered
    );
    assert_eq!(f.pending, 0, "{what}: far-view jobs left after settling");
}

#[test]
fn far_view_settles_at_two_places() {
    let Some(spawn) = run(&[]) else {
        return;
    };
    check("spawn", &spawn);
    let Some(placed) = run(&["--place", "25.0S, 70.0W"]) else {
        return;
    };
    check("25.0S, 70.0W", &placed);
}
