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

mod chargen_common;

use std::path::PathBuf;

use chargen_common::{Client, SIZE, game_dir};
use tessera_client::screens::{Action, Stage};
use tessera_client::walker_host::save_png;

#[test]
fn every_body_stands_in_the_creation_viewport() {
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
