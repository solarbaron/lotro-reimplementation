// SPDX-License-Identifier: GPL-3.0-or-later
//! The day and night lamp lights of placed light-glow objects (`instance-scripts.md` §6,
//! §9.4) in the walker's frame light table, against the user's own install, headless
//! (read-only; skipped unless `TESSERA_GAME_DIR` is set, or when no GPU adapter exists).
//!
//! `lamp_lights_reach_the_light_table_at_night`: a `--screenshot` run at landblock
//! (1, 127, 146), the landblock near the default spawn with the most light-glow lamps
//! (`tessera-world`'s `lamps_real_data` prints it; the default spawn has none within the
//! 200 m light radius), with the clock stopped at midnight; once the area has settled the
//! light table must have
//! held lamp lights in some frame (the table lit particle systems and decals take their
//! lights from, `particle-runtime.md` §12 rule 13, `decals.md` §9.1 (d)). Only counts are
//! printed; the image goes to a temporary directory and is not compared.

use clap::Parser as _;
use tessera_client::Args;
use tessera_client::headless::{self, HeadlessReport, SettleWait};
use tessera_render::RenderError;

/// The spawn landblock (module docs).
const LAMP_SPAWN: &str = "1,127,146";

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

fn run(extra: &[&str]) -> Option<HeadlessReport> {
    let Some(dir) = std::env::var_os("TESSERA_GAME_DIR").filter(|d| !d.is_empty()) else {
        eprintln!("TESSERA_GAME_DIR not set; skipping");
        return None;
    };
    let data = tempfile::tempdir().expect("tempdir");
    let out = tempfile::tempdir().expect("tempdir");
    let png = out.path().join("lamps.png");
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
        "--time-of-day",
        "0",
        "--time-rate",
        "0",
    ]
    .iter()
    .map(|s| (*s).to_owned())
    .collect();
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
                eprintln!("no GPU adapter; skipping: {e:#}");
                return None;
            }
            panic!("headless lamp run failed: {e:#}");
        }
    }
}

#[test]
fn lamp_lights_reach_the_light_table_at_night() {
    let Some(r) = run(&["--spawn", LAMP_SPAWN]) else {
        return;
    };
    let p = &r.particles;
    eprintln!(
        "settled {} after {} frames; lamp lights in the table: {} last frame, {} at most; lit owners {}",
        r.settled, r.frames, p.table_lamp_lights, p.max_table_lamp_lights, p.lit_owners
    );
    assert!(r.settled, "the area did not settle: {:?}", r.unsettled);
    assert!(
        p.max_table_lamp_lights > 0,
        "no lamp light reached the light table at midnight at {LAMP_SPAWN}"
    );
}
