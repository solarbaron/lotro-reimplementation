// SPDX-License-Identifier: GPL-3.0-or-later
//! Real-data smoke test of the character viewer's worker side: one playable character
//! and one clip, opened headlessly against the owner's install, then one pose sampled on
//! the animation scheduler.
//!
//! Skipped (reported as passed) unless `TESSERA_GAME_DIR` is set. The install is only
//! read. Output is counts and DIDs, never game content. Run in release mode:
//! `cargo test --release -p tessera-explorer --test character_real_data -- --nocapture`.
//! `TESSERA_EXPLORER_FULL=1` also assembles every playable body and scans the whole clip
//! catalogue.

use std::path::PathBuf;
use std::sync::Arc;
use std::time::{Duration, Instant};

use tessera_anim::{AnimationScheduler, Animator, Playback, SchedulerConfig, Wrap};
use tessera_assets::{JobContext, PoolConfig, WorkerPool};
use tessera_explorer::character::{
    CharacterRequest, Figure, build_catalog, build_character, build_clip_catalog, load_clip,
};
use tessera_explorer::charspec::{StateArg, parse_figure, parse_item};
use tessera_explorer::data::DataContext;

fn game_dir() -> Option<PathBuf> {
    let dir = std::env::var_os("TESSERA_GAME_DIR").map(PathBuf::from);
    if dir.is_none() {
        eprintln!("TESSERA_GAME_DIR not set; skipping real-data test");
    }
    dir
}

fn full() -> bool {
    std::env::var_os("TESSERA_EXPLORER_FULL").is_some_and(|v| v == "1")
}

#[test]
fn one_character_and_one_clip() {
    let Some(dir) = game_dir() else { return };
    let ctx = DataContext::open(&dir, None).expect("open the install read-only");
    let job = JobContext::detached();
    let t = Instant::now();
    let catalog = build_catalog(&ctx).expect("catalog");
    eprintln!(
        "catalog: {} bodies, idle layer {:?}, movement layer {:?}, slots {:x?}, problems {:?} \
         ({} ms)",
        catalog.bodies.len(),
        catalog.idle_layer,
        catalog.movement_layer,
        catalog.slots,
        catalog.problems,
        t.elapsed().as_millis()
    );
    assert!(
        catalog.bodies.len() >= 14,
        "appearance.md §2.2 lists 14 bodies"
    );
    let figure = catalog
        .resolve_figure(&parse_figure("human,m").unwrap())
        .expect("Human M");
    let mut req = CharacterRequest::new(figure);
    // A tunic (W7) and a one-handed sword in the hand (H1): one worn and one held item.
    req.items.push(parse_item("0x79000420").unwrap());
    req.items.push(parse_item("0x7900066F").unwrap());
    let t = Instant::now();
    let c = build_character(&ctx, &catalog, &req, &job).expect("character");
    eprintln!(
        "{}: aabb {:?} {} parts ({} skinned, {} held), {} bones, scale {}, {} materials, {} textures, \
         {} diagnostics, notes {:?} ({} ms)",
        c.title,
        c.scene.aabb,
        c.scene.parts.len(),
        c.scene.parts.iter().filter(|p| p.skinned).count(),
        c.held.len(),
        c.bones(),
        c.model_scale,
        c.scene.materials.len(),
        c.scene.textures.len(),
        c.diagnostics.len(),
        c.scene.notes,
        t.elapsed().as_millis()
    );
    assert_eq!(
        c.bones(),
        89,
        "the 89-bone male skeleton (appearance.md §2.2)"
    );
    assert!(c.scene.drawable());
    assert!(
        c.scene.parts.iter().any(|p| p.skinned),
        "skinned body parts"
    );
    assert_eq!(c.held.len(), 1, "the sword is held");
    assert!(c.attach.is_some());
    for s in &c.clips.states {
        eprintln!(
            "  {}: {:?} ({} candidates, {} issues)",
            s.state.name(),
            s.chosen
                .as_ref()
                .map(|r| (r.clip, r.layer.1.clone(), r.rate)),
            s.candidates.len(),
            s.issues
        );
    }
    let idle = c
        .clips
        .chosen(StateArg::Idle)
        .expect("an idle clip (script-table.md §8.3)")
        .clip;
    let walk = c.clips.chosen(StateArg::Walk).expect("a walk clip").clip;
    assert_ne!(idle, walk);

    let t = Instant::now();
    let clip = load_clip(&ctx, walk.0, c.skeleton.clone(), &job).expect("walk clip");
    eprintln!(
        "clip {walk}: {} s, {} tracks, {} frames, {}, {} annotation tracks, bound {}, \
         root bone {:?} travel {:.3} m ({} ms)",
        clip.info.duration,
        clip.info.tracks,
        clip.info.frames,
        clip.info.kind,
        clip.info.annotation_tracks,
        clip.bound.is_some(),
        clip.root_bone,
        clip.root_travel,
        t.elapsed().as_millis()
    );
    let bound = clip.bound.clone().expect("the walk clip fits the skeleton");

    // One pose on the scheduler (sampling on workers).
    let pool = WorkerPool::new(PoolConfig::default());
    let mut sched = AnimationScheduler::new(&pool, SchedulerConfig::default());
    let skeleton = c.skeleton.clone().unwrap();
    let id = sched.add(Arc::clone(&skeleton), c.attach.clone()).unwrap();
    let mut anim = Animator::new(Arc::clone(&skeleton));
    let mut p = Playback::new(bound, Wrap::Loop);
    p.time = 0.4;
    anim.play(0, p, 0.0, false).unwrap();
    sched.submit(id, &anim, 1).unwrap();
    sched.collect_all(Duration::from_secs(10));
    let pose = sched.pose(id).unwrap();
    assert_eq!(pose.frame, 1);
    assert!(pose.error.is_none(), "{:?}", pose.error);
    assert_eq!(pose.skin.len(), 89);
    assert!(pose.skin.iter().all(|m| m.is_finite()));
    assert_eq!(pose.attach.len(), 1, "the sword's location");
}

/// The headless sheet (`--sheet`): one figure, two times, two angles, written to a
/// temporary file outside the install and removed again. Skipped without a GPU adapter.
#[test]
fn a_sheet_renders_headless() {
    let Some(dir) = game_dir() else { return };
    let out =
        std::env::temp_dir().join(format!("tessera-explorer-sheet-{}.png", std::process::id()));
    let plan = tessera_explorer::sheet::SheetPlan {
        game_dir: dir,
        out: out.clone(),
        args: tessera_explorer::charspec::CharacterArgs::parse_words([
            "--character",
            "dwarf,m",
            "--wear",
            "0x7901D692",
            "--clip",
            "walk",
            "--bones",
            "--trail",
        ])
        .unwrap(),
        times: vec![0.0, 0.5],
        angles: vec![[160.0, 10.0], [70.0, 5.0]],
        tile: [96, 128],
        wireframe: false,
        focus: None,
    };
    match tessera_explorer::sheet::run_sheet(&plan) {
        Ok(r) => {
            for l in &r.lines {
                eprintln!("{l}");
            }
            assert!(r.title.contains("Dwarf"));
            assert!(r.clip.is_some(), "the walk clip");
            assert!(
                r.lines.iter().any(|l| l.contains("held ")),
                "the shield is held"
            );
            let written = r.written.expect("written");
            let bytes = std::fs::read(&written).unwrap();
            assert!(bytes.starts_with(b"\x89PNG"));
            let _ = std::fs::remove_file(written);
        }
        Err(e) if e.to_string().contains("no GPU renderer") => {
            eprintln!("no GPU renderer in this environment, skipping: {e}");
        }
        Err(e) => panic!("{e}"),
    }
}

#[test]
fn every_body_and_the_clip_catalogue() {
    let Some(dir) = game_dir() else { return };
    if !full() {
        eprintln!("set TESSERA_EXPLORER_FULL=1 for every body and the clip catalogue");
        return;
    }
    let ctx = DataContext::open(&dir, None).expect("open the install read-only");
    let job = JobContext::detached();
    let catalog = build_catalog(&ctx).expect("catalog");
    for b in &catalog.bodies {
        let t = Instant::now();
        let c = build_character(
            &ctx,
            &catalog,
            &CharacterRequest::new(Figure::Playable {
                species: b.species,
                sex: b.sex,
            }),
            &job,
        )
        .expect("body");
        let chosen: Vec<String> = c
            .clips
            .states
            .iter()
            .map(|s| {
                format!(
                    "{} {}",
                    s.state.name(),
                    s.chosen
                        .as_ref()
                        .map_or_else(|| "-".into(), |r| r.clip.to_string())
                )
            })
            .collect();
        eprintln!(
            "{} (avatar {}): did {:#010x} bones {} scale {:.4} parts {} sliders {}/{} clips [{}] \
             ({} ms)",
            b.title(),
            b.avatar,
            c.did(),
            c.bones(),
            c.model_scale,
            c.scene.parts.len(),
            c.sliders.visible(),
            c.sliders.defs.len(),
            chosen.join(", "),
            t.elapsed().as_millis()
        );
        assert!(c.scene.drawable(), "{}", b.title());
    }
    let t = Instant::now();
    let cat = build_clip_catalog(&ctx, &job).expect("clip catalogue");
    let spline = cat.clips.iter().filter(|c| c.spline == Some(true)).count();
    let inter = cat.clips.iter().filter(|c| c.spline == Some(false)).count();
    eprintln!(
        "clip catalogue: {} clips ({spline} spline, {inter} interleaved, {} unknown), {} failed, \
         {} ms ({} ms measured here); {} fit 89 bones, {} fit 62",
        cat.clips.len(),
        cat.clips.len() - spline - inter,
        cat.failed,
        cat.ms,
        t.elapsed().as_millis(),
        cat.fitting(89).count(),
        cat.fitting(62).count()
    );
    assert!(cat.clips.len() > 1000);
}
