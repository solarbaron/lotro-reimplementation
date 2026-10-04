// SPDX-License-Identifier: GPL-3.0-or-later
//! End-to-end `--smoke` runs, in process. Skipped when no GPU adapter (not even a software
//! one) is available. The real-data run needs `TESSERA_GAME_DIR` and only reads the install.

use clap::Parser as _;
use tessera_client::Args;
use tessera_client::runtime::DataStatus;
use tessera_client::smoke::{self, SmokeReport};
use tessera_render::RenderError;

fn run(extra: &[&str]) -> Option<SmokeReport> {
    let data = tempfile::tempdir().expect("tempdir");
    let data_dir = data.path().to_str().expect("utf-8 temp path").to_owned();
    let mut argv = vec![
        "tessera-client",
        "--smoke",
        "--frames",
        "3",
        "--size",
        "160x90",
    ];
    argv.extend_from_slice(&["--data-dir", &data_dir]);
    argv.extend_from_slice(extra);
    let args = Args::try_parse_from(argv).expect("valid args");
    match smoke::run(&args) {
        Ok(r) => Some(r),
        Err(e) => {
            if matches!(
                e.downcast_ref::<RenderError>(),
                Some(RenderError::NoAdapter { .. })
            ) {
                eprintln!("skipping: no GPU adapter: {e:#}");
                return None;
            }
            panic!("smoke run failed: {e:#}");
        }
    }
}

#[test]
fn smoke_without_game_data() {
    let Some(report) = run(&["--no-data"]) else {
        return;
    };
    assert_eq!(report.frames, 3);
    assert!(report.quad_visible);
    assert_eq!(report.surface, None, "procedural texture expected");
    assert!(matches!(report.data, DataStatus::NoData(_)));
    assert!(report.net_events > 0, "sandbox produced no events");
}

#[test]
fn smoke_with_real_data() {
    let Some(dir) = std::env::var_os("TESSERA_GAME_DIR") else {
        eprintln!("skipping: TESSERA_GAME_DIR is not set");
        return;
    };
    let dir = dir.to_str().expect("utf-8 game dir").to_owned();
    let Some(report) = run(&["--game-dir", &dir, "--no-entry-count"]) else {
        return;
    };
    assert!(
        matches!(report.data, DataStatus::Ready { .. }),
        "{:?}",
        report.data
    );
    let did = report.surface.expect("a surface from the archives");
    assert_eq!(did >> 24, 0x41);
    assert!(report.quad_visible);
}
