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
//!
//! The same run reports the weather and water systems (`particle-runtime.md` §11.2) and the
//! placed decals of the resident landblocks (`decals.md` §2.3,
//! §9.1): Decal SEDs realised as decal objects and built; at M3-09 some are drawn. A
//! further test walks the creation
//! viewport's race scenes (`character-screens.md` §2.1.1, through the `--character-select`
//! start-up of `chargen_common`): every scene's Decal elements are built and drawn (drawn
//! count = the scene's decal count) and the effect entities' particle systems give the
//! viewport pass vertices.

mod chargen_common;

use clap::Parser as _;
use tessera_client::Args;
use tessera_client::headless::{self, HeadlessReport, SettleWait};
use tessera_render::RenderError;

/// Frames per second of the paced headless loop.
const FPS: u64 = 60;

/// How the screenshot waits for the area to settle (`walker.md` §7): as long as loading
/// progresses, counted in frames rather than wall-clock seconds, so a loaded machine makes
/// the run slower, not different. It gives up after this many frames without any loading
/// progress, or after this many frames in all.
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
    let shot = png.path().join("particles.png");
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
    // The weather and water systems of UNIQUEDB (`particle-runtime.md` §11.2): created
    // from the install's records; the three water systems are placed in every world frame.
    eprintln!(
        "M3-03 weather: {} systems running, {} placed in the last frame",
        p.weather_systems, p.weather_placed
    );
    assert!(p.weather_systems > 0, "no weather system was created");
    assert!(p.weather_placed > 0, "no weather system was placed");
    // Placed decals around the location (`decals.md` §2.3, §4, §9.1).
    let d = p.decals;
    eprintln!(
        "M3-03 decals: {} landblocks with decals, {} realised, {} built ({} triangles), \
         {} drawn in the last frame in {} batches (nearest {} m, its range {} m), {} \
         surfaces unresolved, {} batches waiting for a texture",
        d.blocks,
        d.decals,
        d.built,
        d.triangles,
        d.drawn,
        d.batches,
        d.nearest.0,
        d.nearest.1,
        d.looks_failed,
        d.textures_pending
    );
    assert!(d.decals > 0, "no Decal SED was realised around M3-03");
    assert!(d.built > 0, "no decal mesh was built");
    assert_eq!(d.pending, 0, "decal builds still running when settled");
}

/// Placed decals drawn in the world (`decals.md` §6–§7, §9.1): at reference location M3-09
/// (region 2, landblock 0x7B68) decals lie within their draw range of the stand point, so
/// the settled frame queues decal batches.
#[test]
fn placed_decals_are_drawn_near_a_reference_location() {
    let Some(dir) = std::env::var_os("TESSERA_GAME_DIR").filter(|d| !d.is_empty()) else {
        eprintln!("TESSERA_GAME_DIR not set; skipping");
        return;
    };
    let Some(report) = run(&dir, &["--spawn", "2,123,104,131.54,30.72"]) else {
        return;
    };
    let d = report.particles.decals;
    eprintln!(
        "M3-09 decals: {} realised, {} built, {} drawn ({} triangles) in {} batches, nearest \
         {} m (range {} m), {} batches waiting for a texture",
        d.decals,
        d.built,
        d.drawn,
        d.drawn_triangles,
        d.batches,
        d.nearest.0,
        d.nearest.1,
        d.textures_pending
    );
    assert!(report.settled, "the location did not settle");
    assert!(d.drawn > 0, "no placed decal drawn near M3-09");
    assert!(d.batches > 0 && d.drawn_triangles > 0);
}

/// `character-screens.md` §2.1.1 rules 1 and 3 on every race scene: the scene's Decal
/// elements are built against its objects and all drawn, and the effect entities' systems
/// reach the viewport pass.
#[test]
fn creation_scenes_draw_their_decals_and_effects() {
    use tessera_client::screens::{Action, Stage};
    let Some(game) = chargen_common::game_dir() else {
        return;
    };
    let dir = tempfile::tempdir().expect("tempdir");
    let Some(mut c) = chargen_common::Client::start(game, dir.path()) else {
        return;
    };
    c.until("the character screens open", |c| {
        c.ui.pre_world().is_some_and(|p| {
            p.screens().model().stage() == Stage::Select && !p.screens().model().rows().is_empty()
        })
    });
    let data =
        c.ui.pre_world()
            .expect("screens")
            .screens()
            .model()
            .data()
            .clone();
    let mut seen = std::collections::HashSet::new();
    let (mut scenes, mut with_decals, mut decals_drawn, mut with_vertices) = (0, 0, 0, 0);
    let mut most_vertices = 0usize;
    for race in &data.chargen.races {
        let species = race.species();
        let Some(scene) = data.scene(species).cloned() else {
            continue;
        };
        if !seen.insert(scene.did) {
            continue;
        }
        let Some(sex) = race.genders.last().map(|g| g.sex) else {
            continue;
        };
        if c.stage() == Some(Stage::RaceClass) {
            c.act(Action::Back);
        }
        if c.stage() == Some(Stage::Select) {
            c.act(Action::New);
        }
        c.act(Action::Race(species));
        c.act(Action::Gender(sex));
        assert_eq!(c.stage(), Some(Stage::RaceClass));
        let want = scene.decals;
        let later = scenes > 0;
        c.until("the scene, its decals and its effects", |c| {
            let ready = c.host.walker.as_ref().is_some_and(|w| {
                let fx = w.effects();
                // A new scene is a new frame: while its owners are not yet spawned, none of
                // the previous scene's systems linger on.
                if later && !fx.stage_ready() {
                    assert_eq!(
                        fx.stats().lingering,
                        0,
                        "the previous scene's systems linger in scene {}",
                        scene.did
                    );
                }
                w.preview_settled() && fx.stage_ready() && (want == 0 || fx.decals.stage_ready())
            });
            // Draw the viewport (the stage's pass also counts what it drew).
            ready && c.preview_pixels().0
        });
        // One more frame so the systems have been placed and drawn after loading.
        c.run_for(std::time::Duration::from_millis(500));
        let _ = c.preview_pixels();
        let fx = c.host.walker.as_ref().expect("walker").effects().stats();
        eprintln!(
            "scene {}: {} decal elements, {} realised, {} built, {} drawn ({} triangles); \
             {} effect owners, {} systems / {} vertices in the viewport pass",
            scene.did,
            want,
            fx.decals.stage_decals,
            fx.decals.stage_built,
            fx.decals.stage_drawn,
            fx.decals.stage_triangles,
            fx.stage_owners,
            fx.stage_drawn_systems,
            fx.stage_drawn_vertices
        );
        scenes += 1;
        assert_eq!(fx.decals.stage_decals, want, "every Decal element realised");
        if want > 0 {
            with_decals += 1;
            assert_eq!(fx.decals.stage_drawn, want, "every Decal element drawn");
            decals_drawn += fx.decals.stage_drawn;
        }
        if fx.stage_drawn_vertices > 0 {
            with_vertices += 1;
        }
        most_vertices = most_vertices.max(fx.max_stage_drawn_vertices);
    }
    eprintln!(
        "{scenes} race scenes: {with_decals} with decals ({decals_drawn} drawn), \
         {with_vertices} with particle vertices in the viewport (most {most_vertices})"
    );
    assert!(scenes > 0, "no race scene");
    assert!(with_decals > 0, "no race scene has decals to draw");
    assert!(
        most_vertices > 0,
        "the effect entities of the race scenes drew no particle vertices"
    );
}
