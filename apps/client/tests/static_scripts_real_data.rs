// SPDX-License-Identifier: GPL-3.0-or-later
//! Start-up scripts of static entities that are no effect owners, in the world walker,
//! headless, against the user's own install (read-only; skipped unless `TESSERA_GAME_DIR` is
//! set, or when no GPU adapter exists). Run with `cargo test --release -p tessera-client
//! --test static_scripts_real_data -- --nocapture`.
//!
//! `particle-runtime.md` §12 rule 15, answer item 2 under the anchor `s12-r15-detect`: a
//! static entity whose start-up scripts reach a SetProperty of its `Render_Opacity` and no
//! Particle node runs those scripts while the object layer places it, inside the spawn cap of
//! §3.2. No reference location is recorded for such entities (the census test
//! `census_of_static_opacity_writers` in `walker::effects::data` finds one entity descriptor of
//! the kind on the owner's install), so the walker stands at M3-03 outdoors and with
//! `--interiors`, and at M3-09 (`docs/checklists/m3-reference-locations.md`), and each run checks
//! that once the area settles writers found in the landblocks the object layer holds are
//! running, that the write counts add up, and (at M3-03) that the effect owners still spawn and
//! draw although the layer now scans every landblock the object layer holds. Only counts are
//! printed; the screenshots go to temporary directories.

use clap::Parser as _;
use tessera_client::Args;
use tessera_client::headless::{self, HeadlessReport, SettleWait};
use tessera_render::RenderError;

/// Frames per second of the paced headless loop.
const FPS: u64 = 60;

/// How the screenshot waits for the area to settle (`walker.md` §7), counted in frames.
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

fn run(dir: &std::ffi::OsStr, extra: &[&str]) -> Option<HeadlessReport> {
    let data = tempfile::tempdir().expect("tempdir");
    let png = tempfile::tempdir().expect("tempdir");
    let shot = png.path().join("static-scripts.png");
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
    ];
    argv.extend(extra.iter().map(|s| (*s).to_owned()));
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

/// Checks one settled run at `name`; `effects` whether its location must draw particles.
fn check(name: &str, report: &HeadlessReport, effects: bool) -> usize {
    let p = report.particles;
    let o = p.opacity;
    eprintln!(
        "{name}: settled {}; {} landblocks tracked; {} effect owners found ({} live); \
         {} opacity writers found ({} running), {} without object; opacity writes {} \
         (applied {}, replaced {}, lost {}, waiting {}); {} node starts, {} runtime issues",
        report.settled,
        p.blocks,
        p.owners_found,
        p.owners_live,
        o.writers,
        o.writers_running,
        o.writers_without_object,
        o.writes,
        o.applied,
        o.replaced,
        o.lost,
        o.waiting,
        p.node_starts,
        p.issues
    );
    assert!(report.settled, "{name}: the location did not settle");
    if effects {
        assert!(p.owners_live > 0, "{name}: no effect owner was spawned");
        assert!(
            p.drawn_vertices > 0,
            "{name}: no particles drawn in the settled frame"
        );
    }
    assert!(o.writers_running + o.writers_capped <= o.writers, "{name}");
    // Settled means no owner waits to spawn: writers whose object the object layer holds
    // run (a writer whose object is not in the layer's list is not placed).
    if o.writers > 0 {
        assert!(o.writers_running > 0, "{name}: no opacity writer was run");
    }
    assert_eq!(
        o.writes,
        o.applied + o.replaced + o.lost + o.waiting,
        "{name}: write counts do not add up"
    );
    o.writers
}

#[test]
fn static_opacity_writers_run_while_placed() {
    let Some(dir) = std::env::var_os("TESSERA_GAME_DIR").filter(|d| !d.is_empty()) else {
        eprintln!("TESSERA_GAME_DIR not set; skipping");
        return;
    };
    // M3-03 (region 1, landblock 0x6D8D) outdoors and with interiors, M3-09 (region 2,
    // landblock 0x7B68).
    let m3_03 = ["--spawn", "1,109,141,62.96,15.07", "--heading", "270"];
    let cases: [(&str, Vec<&str>, bool); 3] = [
        ("M3-03", m3_03.to_vec(), true),
        (
            "M3-03 interiors",
            [&m3_03[..], &["--interiors"]].concat(),
            true,
        ),
        ("M3-09", vec!["--spawn", "2,123,104,131.54,30.72"], false),
    ];
    let mut writers = 0usize;
    for (name, args, effects) in cases {
        let Some(report) = run(&dir, &args) else {
            return;
        };
        writers += check(name, &report, effects);
    }
    eprintln!("opacity writers found over all runs: {writers}");
}
