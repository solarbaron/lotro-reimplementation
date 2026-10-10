// SPDX-License-Identifier: GPL-3.0-or-later
//! The creation viewport's figure on the user's install (read-only), without a window
//! (`docs/specs/character-screens.md` §2.1, §6; `appearance.md` §3.5): the client's
//! `--character-select` start-up (walker, game UI and character screens, the plain sandbox)
//! driven through the screens' actions, for several races:
//!
//! - on the race-and-class stage the gender entry's race-select entity, and on the
//!   appearance stage the figure of the creation choices, are built through the shared
//!   figure path and carry the stage's `CharGen_Stage` key;
//! - the appearance-stage figure takes its face from its stage (a composed face) and
//!   follows a colour control at once (a new figure with another look);
//! - the walker draws the figure into the viewport's race scene: drawn alone at the
//!   window's size it covers a visible area, lies wholly inside the picture and (unturned)
//!   stands on the scene's marker (§2.1.2–§2.1.4), the ray through the pointer on the figure
//!   finds it (§2.3), and the full frame with the UI is captured.
//!
//! With `TESSERA_SHEET_DIR` set, the frames are written there as PNG (never committed:
//! they show the install's art). Prints counts and keys only. Skipped (reported as passed)
//! without `TESSERA_GAME_DIR` or without a GPU adapter. Writes nothing to the install and
//! no settings (no character is created).

mod chargen_common;

use std::path::PathBuf;
use std::time::Duration;

use chargen_common::{Client, SIZE, game_dir};
use tessera_client::screens::controls::ControlChoices;
use tessera_client::screens::{Action, ControlValue, Stage};

#[test]
fn the_creation_viewport_draws_the_figure_of_the_choices() {
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
    let keys = data.stage_keys;
    // Several races: the first, a middle and the last of the list (three different bodies).
    let n = data.chargen.races.len();
    assert!(n > 0, "races");
    let mut picks = vec![0, n / 2, n - 1];
    picks.dedup();
    let (mut composed, mut followed, mut pictures) = (0usize, 0usize, 0usize);
    for (k, &i) in picks.iter().enumerate() {
        let race = &data.chargen.races[i];
        let species = race.species();
        let sex = race.genders.last().expect("CSI2").sex;
        if c.stage() == Some(Stage::Appearance) {
            c.act(Action::Back);
        }
        if c.stage() == Some(Stage::Select) {
            c.act(Action::New);
        }
        c.act(Action::Race(species));
        c.act(Action::Gender(sex));
        assert_eq!(c.stage(), Some(Stage::RaceClass));
        // The race-and-class stage: the figure of the race and gender carries
        // `CharGen_Stage` 1 (`ClassSelect`, §2.1.4).
        if let Some((req, _)) = c.ui.pre_world().and_then(|p| p.figure_job()) {
            assert_eq!(req.chargen_stage, Some(keys.class_select));
            c.until("the race-and-class figure", |c| {
                c.figure_shown() && c.preview_pixels().0
            });
            let f = c.framing(SIZE);
            eprintln!(
                "race {species:#x}/{sex:#x}: race-and-class figure present {}, in frame {}, on the marker {}",
                f.present(),
                f.in_frame(),
                f.on_marker()
            );
            assert!(
                f.present() && f.in_frame() && f.on_marker(),
                "the race-and-class figure stands framed on its marker: {f:?}"
            );
            // The figure under the pointer (§2.3 Tessera rule, as the selection screen
            // finds it): the ray through the middle of its posed box meets it, the one
            // through the viewport's corner does not.
            let view =
                c.ui.pre_world()
                    .map(|p| p.screens().viewport())
                    .expect("screens");
            let stage = tessera_client::screens::viewport::stage_view(
                &view,
                tessera_client::screens::ui::RECORDED_AMBIENT,
            )
            .expect("a stage camera");
            let (lo, hi) = c
                .host
                .walker
                .as_ref()
                .and_then(|w| w.preview_bounds(&stage))
                .expect("the figure's box");
            let (x, y) = stage.project(SIZE, (lo + hi) * 0.5).expect("in view");
            let hit = |x: f32, y: f32| {
                stage
                    .ray(SIZE, x, y)
                    .is_some_and(|(o, d)| tessera_client::walker::stage::ray_hits_box(o, d, lo, hi))
            };
            assert!(hit(x, y), "the pointer on the figure finds it");
            assert!(!hit(1.0, 1.0), "the pointer in the corner does not");
            c.capture(sheet.as_ref(), &format!("chargen-raceclass-{species:02x}"));
            pictures += 1;
        }
        c.act(Action::Continue);
        assert_eq!(c.stage(), Some(Stage::Appearance));
        // The avatar's own values first (§6 step 3 defaults).
        c.until("the default creation figure", |c| {
            c.figure_shown() && c.preview_pixels().0
        });
        c.capture(
            sheet.as_ref(),
            &format!("chargen-appearance-{species:02x}-default"),
        );
        pictures += 1;
        c.act(Action::Randomise(0x5EED_0000 + i as u64));
        // Turn the camera a little (turntable, §2.3) so the pictures show the figure from
        // aside: two hot clicks of 20°.
        c.act(Action::TurnClick(tessera_client::screens::Turn::Left));
        c.act(Action::TurnClick(tessera_client::screens::Turn::Left));
        c.run_for(Duration::from_millis(1500));
        let turn =
            c.ui.pre_world()
                .map_or(0.0, |p| p.screens().viewport().turn);
        assert!(turn < -20.0, "the turntable turned the camera ({turn}°)");
        let (req, _) =
            c.ui.pre_world()
                .and_then(|p| p.figure_job())
                .expect("the creation figure");
        assert_eq!(req.chargen_stage, Some(keys.appearance));
        c.until("the creation figure", |c| {
            c.figure_shown() && c.preview_pixels().0
        });
        let face = c
            .host
            .walker
            .as_ref()
            .and_then(|w| w.preview())
            .and_then(|p| p.face)
            .expect("built");
        if face.composed() {
            composed += 1;
        }
        let f = c.framing(SIZE);
        eprintln!(
            "race {species:#x}/{sex:#x}: creation figure present {}, in frame {}, {} compositors, {} edits",
            f.present(),
            f.in_frame(),
            face.compositors.len(),
            face.edits
        );
        // Turned by the turntable: whole and in frame (the marker row is checked unturned).
        assert!(
            f.present() && f.in_frame(),
            "the creation figure is drawn whole in the viewport: {f:?}"
        );
        c.capture(sheet.as_ref(), &format!("chargen-appearance-{species:02x}"));
        pictures += 1;
        // A colour control: the drawn figure follows at once (a new figure, another look).
        let colour =
            c.ui.pre_world()
                .and_then(|p| p.screens().model().current_avatar().cloned())
                .and_then(|a| {
                    a.controls
                        .iter()
                        .find(|ct| {
                            ct.writes.is_some() && matches!(ct.choices, ControlChoices::Palette(_))
                        })
                        .map(|ct| ct.control.index)
                });
        if let Some(ci) = colour {
            c.act(Action::Control(
                ci,
                ControlValue::Colour([40, 70, 200, 255]),
            ));
            c.until("the recoloured figure", |c| {
                c.figure_shown() && c.preview_pixels().0
            });
            let after = c
                .host
                .walker
                .as_ref()
                .and_then(|w| w.preview())
                .and_then(|p| p.face)
                .expect("built");
            if after != face {
                followed += 1;
            }
            c.capture(
                sheet.as_ref(),
                &format!("chargen-appearance-{species:02x}-recoloured"),
            );
            pictures += 1;
        }
        if k == 0 {
            // The turntable's zoom (§2.3): the timed move to the zoom frame.
            c.act(Action::ZoomIn);
            c.run_for(Duration::from_millis(2500));
            let zoom =
                c.ui.pre_world()
                    .map_or(0.0, |p| p.screens().viewport().zoom);
            assert_eq!(zoom, 1.0, "zoomed in");
            c.capture(
                sheet.as_ref(),
                &format!("chargen-appearance-{species:02x}-zoomed"),
            );
            pictures += 1;
            c.act(Action::ZoomOut);
        }
    }
    eprintln!(
        "{} races: {composed} creation figures with a composed face, {followed} followed a colour control, {pictures} pictures",
        picks.len()
    );
    assert_eq!(composed, picks.len(), "faces from the stage");
    assert_eq!(followed, picks.len(), "colour controls applied live");
    // Leaving the screens removes the figure.
    c.act(Action::Back);
    c.act(Action::Back);
    assert_eq!(c.stage(), Some(Stage::Select));
    c.act(Action::Quit);
    c.until("the screens close", |c| c.ui.pre_world().is_none());
    assert!(
        c.host
            .walker
            .as_ref()
            .is_some_and(|w| w.preview().is_none())
    );
    c.shutdown();
}
