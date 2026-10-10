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
//! - the walker draws the figure alone into the viewport's external content (pixels of the
//!   preview pass), and the full frame with the UI is captured.
//!
//! With `TESSERA_SHEET_DIR` set, the frames are written there as PNG (never committed:
//! they show the install's art). Prints counts and keys only. Skipped (reported as passed)
//! without `TESSERA_GAME_DIR` or without a GPU adapter. Writes nothing to the install and
//! no settings (no character is created).

use std::path::PathBuf;
use std::time::{Duration, Instant};

use clap::Parser as _;
use tessera_client::Args;
use tessera_client::runtime::Runtime;
use tessera_client::screens::controls::ControlChoices;
use tessera_client::screens::{Action, ControlValue, Stage};
use tessera_client::ui_host::client::ClientUi;
use tessera_client::ui_host::cursor::NoWindow;
use tessera_client::walker::characters::FigureState;
use tessera_client::walker_host::{WalkerHost, save_png};
use tessera_render::{OffscreenFormat, Renderer, RendererConfig};

fn game_dir() -> Option<PathBuf> {
    match std::env::var_os("TESSERA_GAME_DIR") {
        Some(d) if !d.is_empty() => Some(PathBuf::from(d)),
        _ => {
            eprintln!("TESSERA_GAME_DIR not set; skipping");
            None
        }
    }
}

const SIZE: (u32, u32) = (1280, 720);
const FRAME: Duration = Duration::from_millis(16);
/// Frames a stage may take to show its figure (figure jobs, uploads, first pose).
const MAX_FRAMES: u32 = 3600;

/// The client's headless loop with the game UI (as `ui_host::run`), driven by the test.
struct Client<'r> {
    rt: Runtime,
    host: WalkerHost,
    ui: ClientUi,
    renderer: Renderer<'r>,
    last: Instant,
    viewport_set: bool,
}

impl Client<'_> {
    /// One frame: loader, walker, UI steps, the portraits and the frame itself.
    fn frame(&mut self) {
        let now = Instant::now();
        let dt = now.saturating_duration_since(self.last).as_secs_f32();
        self.last = now;
        let _ = self.rt.poll_loader();
        self.rt.tick(now);
        self.host.poll(&mut self.rt, &mut self.renderer);
        assert!(self.host.failed.is_none(), "walker: {:?}", self.host.failed);
        self.ui.poll(&mut self.rt, self.host.walker.as_mut());
        assert!(self.ui.failed().is_none(), "UI: {:?}", self.ui.failed());
        let Some(walker) = self.host.walker.as_mut() else {
            std::thread::sleep(Duration::from_millis(20));
            return;
        };
        if !self.viewport_set {
            walker.set_viewport(SIZE.0, SIZE.1);
            self.viewport_set = true;
        }
        self.ui.before_update(walker, &mut self.rt, &mut NoWindow);
        let ev = walker.update(
            &mut self.renderer,
            self.rt
                .world
                .as_mut()
                .map(|w| w as &mut dyn tessera_client::walker::WalkerSession),
            &mut self.rt.plugins,
            now,
            dt,
        );
        self.ui.after_update(walker, &mut self.rt, &ev);
        self.ui.step8(walker, &mut self.renderer, &mut self.rt);
        self.ui.render_portraits(walker, &mut self.renderer);
        self.ui.end_frame(&self.rt);
        let spent = now.elapsed();
        if spent < FRAME {
            std::thread::sleep(FRAME - spent);
        }
    }

    /// Frames until `done`, at most [`MAX_FRAMES`].
    fn until(&mut self, what: &str, mut done: impl FnMut(&mut Self) -> bool) {
        for _ in 0..MAX_FRAMES {
            self.frame();
            if done(self) {
                return;
            }
        }
        let preview = self.host.walker.as_ref().and_then(|w| w.preview());
        panic!(
            "{what}: not reached in {MAX_FRAMES} frames; preview {:?}, wanted {:?}, settled {:?}",
            preview.as_ref().map(|p| (&p.request, p.state, &p.face)),
            self.ui
                .pre_world()
                .and_then(|p| p.figure_job())
                .map(|j| j.0),
            self.host.walker.as_ref().map(|w| w.characters().settled()),
        );
    }

    fn act(&mut self, a: Action) {
        assert!(self.ui.screens_act(a), "the screens are open");
        self.frame();
    }

    fn stage(&self) -> Option<Stage> {
        self.ui.pre_world().map(|p| p.screens().model().stage())
    }

    /// Whether the screens' figure is built for the request the screens ask for now and
    /// the character layer has settled (on the GPU, posed; its textures may still load,
    /// which [`Client::preview_pixels`] waits for).
    fn figure_shown(&self) -> bool {
        let (Some(walker), Some(pre)) = (self.host.walker.as_ref(), self.ui.pre_world()) else {
            return false;
        };
        let Some(want) = pre.figure_job() else {
            return false;
        };
        walker.preview().is_some_and(|p| {
            p.request == want.0
                && p.look == want.1
                && matches!(p.state, FigureState::Built { parts, .. } if parts > 0)
        }) && walker.characters().settled()
    }

    /// The screens' figure drawn alone, as the viewport receives it: (drawn, covered
    /// pixels).
    fn preview_pixels(&mut self) -> (bool, usize) {
        let walker = self.host.walker.as_mut().expect("walker");
        let target = self
            .renderer
            .create_offscreen_target(320, 360, OffscreenFormat::Rgba8Unorm)
            .expect("target");
        let mut frame = self.renderer.begin_offscreen_frame(&target).expect("frame");
        let drawn = walker.draw_preview(&mut frame, 0.0, false);
        self.renderer.end_frame(frame).expect("end frame");
        let image = self.renderer.read_pixels(&target).expect("read back");
        let covered = image
            .data
            .as_chunks::<4>()
            .0
            .iter()
            .filter(|p| p[3] > 0)
            .count();
        (drawn, covered)
    }

    fn capture(&mut self, sheet: Option<&PathBuf>, name: &str) {
        let walker = self.host.walker.as_mut().expect("walker");
        let image = tessera_client::ui_host::run::capture_with_ui(
            &mut self.renderer,
            walker,
            &mut self.ui,
            SIZE,
        )
        .expect("capture");
        if let Some(dir) = sheet {
            let path = dir.join(format!("{name}.png"));
            save_png(&path, &image).expect("png");
            eprintln!("picture written: {}", path.display());
        }
    }
}

#[test]
fn the_creation_viewport_draws_the_figure_of_the_choices() {
    let Some(game) = game_dir() else { return };
    let renderer = match Renderer::new_headless(&RendererConfig {
        meshes: tessera_client::walker::mesh_config(tessera_client::walker::arena_bytes(
            &tessera_client::walker_host::walker_config(&Args::parse_from(["tessera-client"])),
        )),
        ..RendererConfig::default()
    }) {
        Ok(r) => r,
        Err(e) => {
            eprintln!("SKIPPED: no GPU adapter ({e})");
            return;
        }
    };
    let dir = tempfile::tempdir().expect("tempdir");
    let mut args = Args::parse_from([
        "tessera-client",
        "--character-select",
        "--size",
        "1280x720",
        // The screens alone in the pictures: no walker HUD, no game HUD set.
        "--no-hud",
        "--no-ui-hud",
    ]);
    args.game_dir = Some(game);
    args.data_dir = Some(dir.path().join("data"));
    let sheet = std::env::var_os("TESSERA_SHEET_DIR").map(PathBuf::from);
    let rargs = ClientUi::runtime_args(&args);
    let mut rt = Runtime::start(&rargs, Instant::now()).expect("runtime");
    let shared = rt.walker_shared.clone().expect("walker session");
    let host = WalkerHost::new(&args, shared);
    let mut ui = ClientUi::new(&rargs);
    ui.prepare(&mut rt);
    let mut c = Client {
        rt,
        host,
        ui,
        renderer,
        last: Instant::now(),
        viewport_set: false,
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
        // The race-and-class stage: the race-select entity carries stage RaceClass.
        if let Some((req, _)) = c.ui.pre_world().and_then(|p| p.figure_job()) {
            assert_eq!(req.chargen_stage, Some(keys.race_class));
            c.until("the race-select figure", |c| {
                c.figure_shown() && c.preview_pixels().0
            });
            let (drawn, covered) = c.preview_pixels();
            eprintln!(
                "race {species:#x}/{sex:#x}: race-select figure drawn {drawn}, {covered} pixels"
            );
            assert!(drawn && covered > 0, "the race-select figure is drawn");
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
        // Turn the figure a little (turntable, §2.3) so the pictures show it from aside.
        c.act(Action::TurnPress(tessera_client::screens::Turn::Left));
        let held = Instant::now();
        while held.elapsed() < Duration::from_millis(300) {
            c.frame();
        }
        c.act(Action::TurnRelease(tessera_client::screens::Turn::Left));
        let yaw =
            c.ui.pre_world()
                .map_or(0.0, |p| p.screens().viewport().yaw_degrees);
        assert!(yaw.abs() > 1.0, "the turntable turned the figure ({yaw}°)");
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
        let (drawn, covered) = c.preview_pixels();
        eprintln!(
            "race {species:#x}/{sex:#x}: creation figure drawn {drawn}, {covered} pixels, {} compositors, {} edits",
            face.compositors.len(),
            face.edits
        );
        assert!(drawn && covered > 0, "the creation figure is drawn");
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
            // The turntable's zoom (§2.3): the same figure, framed closer.
            c.act(Action::ZoomIn);
            for _ in 0..5 {
                c.frame();
            }
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
    let Client {
        mut rt,
        mut host,
        mut ui,
        mut renderer,
        ..
    } = c;
    ui.shutdown(&mut rt, Some(&mut renderer), &mut NoWindow);
    if let Some(w) = host.walker.as_mut() {
        w.shutdown(&mut renderer);
    }
    rt.shutdown();
}
