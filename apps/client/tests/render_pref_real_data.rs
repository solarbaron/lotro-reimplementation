// SPDX-License-Identifier: GPL-3.0-or-later
//! The headless real-data check of `far-view.md` FD17 with the `--render-pref` overrides of
//! FD22 (`walker.md` §14.10), against the user's own install (read-only; skipped unless
//! `TESSERA_GAME_DIR` is set, or when no GPU adapter exists).
//!
//! `normal_mapped_far_view_settles_with_render_prefs`: a `--screenshot` run with
//! `--far-view on` and `FarLandscapeNormalMaps=True`, `MultiPassLighting=True` and
//! `LandscapeLightingQuality=High`. The walker's configuration must select the
//! normal-mapped template and the seam overlay (the user's preferences file is never read
//! by a headless run, FD21), and the area, the far view included, must settle with far
//! terrain batches drawn over landblocks outside the near field. Only counts are printed;
//! the image goes to a temporary directory and is not compared.
//!
//! **What it proves:** the template and the overlay are checked at the configuration
//! level only (`walker_config`); the run itself is checked with the far view's general
//! counters (`ready`, `batches`, `covered`, `pending`). The far view's counters have no
//! template or seam-overlay entry, so a run that fell back to the plain template or skipped
//! the overlay would still pass here.

use clap::Parser as _;
use tessera_client::Args;
use tessera_client::headless::{self, SettleWait};
use tessera_render::RenderError;
use tessera_world::far_view::Template;

/// Frames per second of the paced headless loop.
const FPS: u64 = 60;

/// How the screenshot waits for the area to settle, counted in frames (as
/// `far_view_real_data.rs`).
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

#[test]
fn normal_mapped_far_view_settles_with_render_prefs() {
    let Some(dir) = std::env::var_os("TESSERA_GAME_DIR").filter(|d| !d.is_empty()) else {
        eprintln!("TESSERA_GAME_DIR not set; skipping");
        return;
    };
    let data = tempfile::tempdir().expect("tempdir");
    let out = tempfile::tempdir().expect("tempdir");
    let png = out.path().join("far-normal.png");
    let argv = [
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
        "--render-pref",
        "FarLandscapeNormalMaps=True",
        "--render-pref",
        "MultiPassLighting=True",
        "--render-pref",
        "LandscapeLightingQuality=High",
    ];
    let args = Args::try_parse_from(argv).expect("valid args");
    assert!(args.walker_headless());
    let config = tessera_client::walker_host::walker_config(&args);
    let s = &config.far_view.settings;
    assert_eq!(s.template(), Template::NormalMapped, "FD22 overrides");
    assert!(s.seam_overlay(), "FD18 inputs from the overrides");
    let report = match headless::run_with(&args, settle_wait()) {
        Ok(r) => r,
        Err(e) => {
            if e.chain().any(|c| {
                matches!(
                    c.downcast_ref::<RenderError>(),
                    Some(RenderError::NoAdapter { .. })
                )
            }) {
                eprintln!("skipping: no GPU adapter: {e:#}");
                return;
            }
            panic!("walker run failed: {e:#}");
        }
    };
    let f = report
        .far_view
        .expect("far view counters with --far-view on");
    eprintln!(
        "normal-mapped far view: {} frames, settled {}; {} / {} sectors, {} terrain batches over {} landblocks, {} jobs",
        report.frames, report.settled, f.ready, f.listed, f.batches, f.covered, f.pending
    );
    assert!(
        report.settled,
        "the area (far view included) did not settle"
    );
    assert!(f.ready > 0, "no sector record in");
    assert!(f.batches > 0, "no far terrain batch drawn");
    assert!(
        f.covered > 9,
        "far batches cover only {} landblocks",
        f.covered
    );
    assert_eq!(f.pending, 0, "far-view jobs left after settling");
}
