// SPDX-License-Identifier: GPL-3.0-or-later
//! A headless scenario run with the game UI attached drives the P3 and P5 window frames
//! (`docs/specs/scenario-feed.md` §12.2, `npc-windows.md` §5–§6, `travel-windows.md` §4.10):
//! the committed `dialogue` scenario opens an NPC dialogue window (P3) and the committed
//! `travel` scenario opens a travel window (P5). Without the per-frame window updates of
//! `tessera_client::ui_host::client::window_frames` the headless `--ui` path leaves those
//! windows closed. Only pass/fail and the window names are printed. Skipped (reported as
//! passed) unless `TESSERA_GAME_DIR` is set; the run renders offscreen, so it needs a GPU.
//!
//! Run with `TESSERA_GAME_DIR=… cargo test --release -p tessera-client --test
//! scenario_ui_windows_real_data -- --nocapture`.

use std::path::{Path, PathBuf};
use std::sync::Arc;

use clap::Parser as _;
use tessera_client::cli::Args;
use tessera_client::scenario::{locate_install, prepare};
use tessera_client::ui_host::run::run;

fn game_dir() -> Option<PathBuf> {
    match std::env::var_os("TESSERA_GAME_DIR") {
        Some(d) if !d.is_empty() => Some(PathBuf::from(d)),
        _ => {
            eprintln!("TESSERA_GAME_DIR not set; skipping");
            None
        }
    }
}

fn scenarios_dir() -> PathBuf {
    Path::new(env!("CARGO_MANIFEST_DIR")).join("tests/scenarios")
}

/// Runs the committed `name` scenario headless with the game UI on the install and returns
/// the short names of the game windows seen open during the run.
fn windows_of(install: &Path, data: &Path, name: &str) -> std::collections::BTreeSet<&'static str> {
    let s = |p: &Path| p.to_str().expect("utf-8").to_owned();
    let file = scenarios_dir().join(format!("{name}.scenario"));
    let mut args = Args::try_parse_from([
        "t".to_owned(),
        "--headless".into(),
        "--ui".into(),
        "--game-dir".into(),
        s(install),
        "--data-dir".into(),
        s(data),
        "--scenario".into(),
        s(&file),
    ])
    .expect("args parse");
    assert!(args.ui_enabled() && args.walker_headless());
    let resolved =
        prepare(&args, &locate_install(&args)).unwrap_or_else(|e| panic!("{name}: prepare: {e}"));
    args.scenario_prepared = Some(Arc::new(resolved));
    let report = run(&args).unwrap_or_else(|e| panic!("{name}: headless --ui run: {e:#}"));
    println!(
        "{name}: {} frames, windows {:?}",
        report.frames, report.game_windows
    );
    report.game_windows
}

#[test]
fn headless_ui_scenario_drives_p3_and_p5_windows() {
    let Some(install) = game_dir() else { return };
    let data = tempfile::tempdir().expect("tempdir");

    // P3: the `dialogue` scenario uses the NPC, which opens the dialogue window.
    let p3 = windows_of(install.as_path(), data.path(), "dialogue");
    assert!(
        p3.contains("npc"),
        "the dialogue scenario should open an NPC window (P3); saw {p3:?}"
    );

    // P5: the `travel` scenario uses the stable-master, which opens the travel window.
    let p5 = windows_of(install.as_path(), data.path(), "travel");
    assert!(
        p5.contains("travel"),
        "the travel scenario should open the travel window (P5); saw {p5:?}"
    );
}
