// SPDX-License-Identifier: GPL-3.0-or-later
//! Particle effects in the world walker, headless, against the user's own install
//! (read-only; skipped unless `TESSERA_GAME_DIR` is set, or when no GPU adapter exists).
//! Run with `cargo test --release -p tessera-client --test particles_real_data --
//! --nocapture`.
//!
//! The walker stands at reference location M3-03 (`docs/checklists/m3-reference-locations.md`),
//! whose landblock holds static entities whose start-up scripts start Particle nodes
//! (`particle-runtime.md` §5). It must settle (the effects layer counts in the settled
//! test: scans, record loads, owners in range and surface textures) and, by then, the
//! scripts of those owners must have started effects that are drawn: particles in the
//! last frame. Only counts are printed; the screenshot goes to a temporary directory.

use clap::Parser as _;
use tessera_client::Args;
use tessera_client::headless::{self, HeadlessReport};
use tessera_render::RenderError;

fn run(dir: &std::ffi::OsStr, extra: &[&str]) -> Option<HeadlessReport> {
    let data = tempfile::tempdir().expect("tempdir");
    let png = tempfile::tempdir().expect("tempdir");
    let shot = png.path().join("particles.png");
    let mut argv: Vec<String> = vec![
        "tessera-client".into(),
        "--size".into(),
        "320x180".into(),
        "--game-dir".into(),
        dir.to_str().expect("utf-8 game dir").into(),
        "--data-dir".into(),
        data.path().to_str().expect("utf-8 temp path").into(),
        "--settle-timeout".into(),
        if cfg!(debug_assertions) { "900" } else { "90" }.into(),
        "--screenshot".into(),
        shot.to_str().expect("utf-8 temp path").into(),
    ];
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
fn an_effect_bearing_location_draws_particles_and_settles() {
    let Some(dir) = std::env::var_os("TESSERA_GAME_DIR").filter(|d| !d.is_empty()) else {
        eprintln!("TESSERA_GAME_DIR not set; skipping");
        return;
    };
    // M3-03: region 1, landblock 0x6D8D, stand point (62.96, 15.07), facing west (compass
    // 270°; the nearest effect owner stands about 30 m west).
    let Some(report) = run(
        &dir,
        &["--spawn", "1,109,141,62.96,15.07", "--heading", "270"],
    ) else {
        return;
    };
    let p = report.particles;
    eprintln!(
        "M3-03: settled {}; {} owners found ({} live), {} requests ({} refused), {} systems \
         ({} lingering), {} live particles; last frame {} systems / {} vertices; most {} \
         vertices; {} frames with particles; {} script reads on the main thread; {} node \
         starts, {} gated, {} runtime issues",
        report.settled,
        p.owners_found,
        p.owners_live,
        p.requests,
        p.refused,
        p.systems,
        p.lingering,
        p.live_particles,
        p.drawn_systems,
        p.drawn_vertices,
        p.max_drawn_vertices,
        p.frames_with_particles,
        p.sync_script_reads,
        p.node_starts,
        p.gated,
        p.issues
    );
    assert!(report.settled, "the location did not settle");
    assert!(p.owners_live > 0, "no effect owner was spawned");
    assert!(p.requests > 0, "no Particle node started");
    assert!(p.systems > 0, "no system is running");
    assert!(
        p.drawn_vertices > 0,
        "no particles drawn in the settled frame"
    );
}
