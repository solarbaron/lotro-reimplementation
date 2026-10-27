// SPDX-License-Identifier: GPL-3.0-or-later
//! Every body of the install on the creation screens (read-only; `docs/specs/
//! character-screens.md` §2.1, §4.1, §5.2.1, §6): for each race and each of its gender
//! entries, the race-and-class figure and the default appearance-stage figure are built,
//! drawn in the viewport and pictured; each keeps a whole-skeleton stance below layer 110
//! (its `Chargen` script, §2.1.4, CSI9) and stands wholly in the window-sized viewport on
//! the scene's marker (§2.1.2, §2.1.3).
//!
//! With `TESSERA_SHEET_DIR` set, the viewport pictures (`body-<species>-<sex>-<stage>.png`)
//! and the whole frames are written there (never committed: they show the install's art).
//! Prints counts and keys only. Skipped (reported as passed) without `TESSERA_GAME_DIR` or
//! without a GPU adapter. Writes nothing to the install and no settings.

#[path = "../../../crates/render/tests/support/gpu.rs"]
mod gpu;

mod chargen_common;

use std::path::PathBuf;

use chargen_common::{Client, SIZE, game_dir};
use tessera_client::screens::{Action, Stage};
use tessera_client::walker_host::save_png;

#[test]
fn every_body_stands_in_the_creation_viewport() {
    // One GPU test at a time in this binary (`gpu.rs`), held for the whole test.
    let _gpu = gpu::GpuLock::acquire();
    let Some(game) = game_dir() else { return };
    let dir = tempfile::tempdir().expect("tempdir");
    let Some(mut c) = Client::start(game, dir.path()) else {
        return;
    };
    let sheet = std::env::var_os("TESSERA_SHEET_DIR").map(PathBuf::from);
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
    let mut bodies = 0usize;
    let mut failures = Vec::new();
    for race in &data.chargen.races {
        let species = race.species();
        for g in &race.genders {
            let sex = g.sex;
            if c.stage() == Some(Stage::Appearance) {
                c.act(Action::Back);
            }
            if c.stage() == Some(Stage::Select) {
                c.act(Action::New);
            }
            c.act(Action::Race(species));
            c.act(Action::Gender(sex));
            assert_eq!(c.stage(), Some(Stage::RaceClass));
            for stage in ["raceclass", "appearance"] {
                if stage == "appearance" {
                    c.act(Action::Continue);
                    assert_eq!(c.stage(), Some(Stage::Appearance));
                }
                c.until("the stage figure", |c| {
                    c.figure_shown() && c.preview_pixels().0
                });
                // A few frames for the pose.
                c.run_for(std::time::Duration::from_millis(500));
                let (drawn, image) = c.preview_image(SIZE);
                assert!(drawn, "{species:#x}/{sex:#x} {stage}: the figure is drawn");
                // CSI9 on Tessera's side: the figure keeps a looping clip below layer 110
                // that drives the whole skeleton (its `Chargen` stance, §2.1.4).
                let idle = c
                    .host
                    .walker
                    .as_ref()
                    .and_then(|w| w.preview())
                    .map(|p| p.idle)
                    .unwrap_or_default();
                let stance = idle
                    .iter()
                    .any(|(layer, once, weights)| *layer < 110 && !once && !weights);
                if let Some(l) = c
                    .host
                    .walker
                    .as_ref()
                    .and_then(|w| w.characters().preview_live())
                {
                    eprintln!(
                        "body {species:#x}/{sex:#x} {stage}: live keys {:?}, {} props, variants {:?}, {:?}",
                        l.keys(),
                        l.props().len(),
                        l.stage_variants(),
                        l.stats()
                    );
                }
                // The whole figure in the window-sized viewport, standing on the marker.
                let f = c.framing(SIZE);
                eprintln!(
                    "body {species:#x}/{sex:#x} {stage}: stance {stance} ({idle:?}), present {}, in frame {}, on the marker {}",
                    f.present(),
                    f.in_frame(),
                    f.on_marker()
                );
                if !(stance && f.present() && f.in_frame() && f.on_marker()) {
                    failures.push(format!(
                        "{species:#x}/{sex:#x} {stage}: stance {stance}, {f:?}"
                    ));
                }
                let ((objects, skipped, parts, lights), settled) = c
                    .host
                    .walker
                    .as_ref()
                    .and_then(|w| w.preview_scene())
                    .expect("the race scene is shown");
                eprintln!(
                    "body {species:#x}/{sex:#x} {stage}: scene {objects} objects ({skipped} not drawn), {parts} parts, {lights} lights, settled {settled}"
                );
                assert!(objects > 0 && lights > 0, "the scene's objects and lights");
                if let Some(dir) = &sheet {
                    let name = format!("body-{species:02x}-{sex:04x}-{stage}");
                    save_png(&dir.join(format!("{name}.png")), &image).expect("png");
                    c.capture(Some(dir), &format!("{name}-frame"));
                }
            }
            bodies += 1;
        }
    }
    eprintln!(
        "{bodies} bodies drawn on both creation stages, {} not standing framed on the marker",
        failures.len()
    );
    assert!(bodies > 0);
    assert!(failures.is_empty(), "{failures:#?}");
    c.act(Action::Back);
    c.act(Action::Back);
    c.act(Action::Quit);
    c.until("the screens close", |c| c.ui.pre_world().is_none());
    c.shutdown();
}

/// `character-screens.md` §2.1.4 (CS29: the class cases of `Chargen` hold MeshFX props):
/// the race-and-class figure of the first race and gender runs its scripts live for each
/// of its classes, and the MeshFX nodes of the class branches show their props on at least
/// one class. Prints counts only.
#[test]
fn class_branch_props_attach_on_the_race_and_class_figure() {
    // One GPU test at a time in this binary (`gpu.rs`), held for the whole test.
    let _gpu = gpu::GpuLock::acquire();
    let Some(game) = game_dir() else { return };
    let dir = tempfile::tempdir().expect("tempdir");
    let Some(mut c) = Client::start(game, dir.path()) else {
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
    let race = data.chargen.races.first().expect("a race");
    let species = race.species();
    let sex = race.genders.first().expect("a gender").sex;
    c.act(Action::New);
    c.act(Action::Race(species));
    c.act(Action::Gender(sex));
    assert_eq!(c.stage(), Some(Stage::RaceClass));
    let classes = data.chargen.class_order(species);
    assert!(!classes.is_empty(), "the race offers classes");
    let (mut live, mut with_props, mut unmatched) = (0usize, 0usize, 0u64);
    for &class in &classes {
        c.act(Action::Class(class));
        c.until("the class figure", |c| {
            c.figure_shown() && c.preview_pixels().0
        });
        c.run_for(std::time::Duration::from_millis(700));
        let stats = c
            .host
            .walker
            .as_ref()
            .and_then(|w| w.characters().preview_live())
            .map(|l| {
                (
                    l.props().len(),
                    l.stats(),
                    l.prop_report().0,
                    l.prop_report().1.len(),
                )
            });
        if let Some((props, s, reached, failed)) = stats {
            live += 1;
            with_props += usize::from(s.props_shown > 0);
            // A MeshFX node that runs without a built prop is a gap of the figure job.
            unmatched += s.props_unbuilt;
            eprintln!(
                "class {class}: {reached} props reached, {props} built ({failed} not), {} shown, {} ran without a prop, {} clips started",
                s.props_shown, s.props_unbuilt, s.clips_started
            );
        }
    }
    eprintln!(
        "{species:#x}/{sex:#x}: {} classes, {live} run live, {with_props} show a prop",
        classes.len()
    );
    assert_eq!(
        live,
        classes.len(),
        "every class figure runs its scripts live"
    );
    assert!(with_props > 0, "the class branches' props attach");
    assert_eq!(unmatched, 0, "every MeshFX node that ran found its prop");
    c.act(Action::Back);
    c.act(Action::Back);
    c.act(Action::Quit);
    c.until("the screens close", |c| c.ui.pre_world().is_none());
    c.shutdown();
}
