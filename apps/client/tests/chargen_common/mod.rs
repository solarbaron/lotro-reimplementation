// SPDX-License-Identifier: GPL-3.0-or-later
//! The client's headless loop with the game UI and the character screens on the user's
//! install, shared by the creation-viewport tests (`chargen_viewport_real_data.rs`,
//! `chargen_bodies_real_data.rs`): the `--character-select` start-up (walker, game UI and
//! character screens over the plain sandbox) driven frame by frame through the screens'
//! actions, with pictures of the viewport and of the whole frame.
#![allow(dead_code)]

use std::path::{Path, PathBuf};
use std::time::{Duration, Instant};

use clap::Parser as _;
use tessera_client::Args;
use tessera_client::runtime::Runtime;
use tessera_client::screens::{Action, Stage};
use tessera_client::ui_host::client::ClientUi;
use tessera_client::ui_host::cursor::NoWindow;
use tessera_client::walker::characters::FigureState;
use tessera_client::walker_host::{WalkerHost, save_png};
use tessera_render::{OffscreenFormat, Renderer, RendererConfig};

/// The install, or `None` (reported) when `TESSERA_GAME_DIR` is unset.
pub fn game_dir() -> Option<PathBuf> {
    match std::env::var_os("TESSERA_GAME_DIR") {
        Some(d) if !d.is_empty() => Some(PathBuf::from(d)),
        _ => {
            eprintln!("TESSERA_GAME_DIR not set; skipping");
            None
        }
    }
}

/// The window size of the runs.
pub const SIZE: (u32, u32) = (1280, 720);
/// One frame.
pub const FRAME: Duration = Duration::from_millis(16);
/// Frames a stage may take to show its figure (figure jobs, uploads, first pose).
pub const MAX_FRAMES: u32 = 3600;

/// The client's headless loop with the game UI (as `ui_host::run`), driven by a test.
pub struct Client<'r> {
    pub rt: Runtime,
    pub host: WalkerHost,
    pub ui: ClientUi,
    pub renderer: Renderer<'r>,
    pub last: Instant,
    pub viewport_set: bool,
}

impl Client<'static> {
    /// Starts the client with `--character-select` on `game` with a fresh data directory in
    /// `data`; `None` (reported) without a GPU adapter.
    pub fn start(game: PathBuf, data: &Path) -> Option<Self> {
        let renderer = match Renderer::new_headless(&RendererConfig {
            meshes: tessera_client::walker::mesh_config(tessera_client::walker::arena_bytes(
                &tessera_client::walker_host::walker_config(&Args::parse_from(["tessera-client"])),
            )),
            ..RendererConfig::default()
        }) {
            Ok(r) => r,
            Err(e) => {
                eprintln!("SKIPPED: no GPU adapter ({e})");
                return None;
            }
        };
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
        args.data_dir = Some(data.join("data"));
        let rargs = ClientUi::runtime_args(&args);
        let mut rt = Runtime::start(&rargs, Instant::now()).expect("runtime");
        let shared = rt.walker_shared.clone().expect("walker session");
        let host = WalkerHost::new(&args, shared);
        let mut ui = ClientUi::new(&rargs);
        ui.prepare(&mut rt);
        Some(Self {
            rt,
            host,
            ui,
            renderer,
            last: Instant::now(),
            viewport_set: false,
        })
    }
}

impl Client<'_> {
    /// One frame: loader, walker, UI steps, the portraits and the frame itself.
    pub fn frame(&mut self) {
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
    pub fn until(&mut self, what: &str, mut done: impl FnMut(&mut Self) -> bool) {
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

    /// Frames for `d` of wall-clock time.
    pub fn run_for(&mut self, d: Duration) {
        let t = Instant::now();
        while t.elapsed() < d {
            self.frame();
        }
    }

    /// One action, then one frame.
    pub fn act(&mut self, a: Action) {
        assert!(self.ui.screens_act(a), "the screens are open");
        self.frame();
    }

    /// The stage shown.
    pub fn stage(&self) -> Option<Stage> {
        self.ui.pre_world().map(|p| p.screens().model().stage())
    }

    /// Whether the screens' figure is built for the request the screens ask for now and
    /// it and the scene have loaded (on the GPU, posed, textures and scene parts loaded).
    pub fn figure_shown(&self) -> bool {
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
        }) && walker.preview_settled()
    }

    /// The viewport drawn as it receives it (the race scene, the figure at its marker, the
    /// stage camera; the recorded ambient), at `size`: (figure drawn, image).
    pub fn preview_image(&mut self, size: (u32, u32)) -> (bool, tessera_render::RgbaImage) {
        let view = self
            .ui
            .pre_world()
            .map(|p| p.screens().viewport())
            .expect("the screens are open");
        let stage = tessera_client::screens::viewport::stage_view(
            &view,
            tessera_client::screens::ui::RECORDED_AMBIENT,
        );
        let walker = self.host.walker.as_mut().expect("walker");
        let target = self
            .renderer
            .create_offscreen_target(size.0, size.1, OffscreenFormat::Rgba8Unorm)
            .expect("target");
        let mut frame = self.renderer.begin_offscreen_frame(&target).expect("frame");
        let drawn = walker.draw_preview(&mut frame, stage.as_ref());
        self.renderer.end_frame(frame).expect("end frame");
        let image = self.renderer.read_pixels(&target).expect("read back");
        (drawn, image)
    }

    /// Whether the viewport's figure is drawn (a small picture of the viewport).
    pub fn preview_pixels(&mut self) -> (bool, usize) {
        let (drawn, image) = self.preview_image((320, 360));
        (drawn, image.data.len() / 4)
    }

    /// Where the figure stands in the viewport drawn at `size` (the race scene left out,
    /// so only the figure covers the black background; `character-screens.md` §2.1.2–
    /// §2.1.4): the figure's pixel box and the row of the marker the figure stands on.
    pub fn framing(&mut self, size: (u32, u32)) -> Framing {
        let view = self
            .ui
            .pre_world()
            .map(|p| p.screens().viewport())
            .expect("the screens are open");
        let stage = tessera_client::screens::viewport::stage_view(
            &view,
            tessera_client::screens::ui::RECORDED_AMBIENT,
        );
        let alone = stage.map(|s| tessera_client::walker::stage::StageView { scene: None, ..s });
        let walker = self.host.walker.as_mut().expect("walker");
        // The lowest joint's height above the marker.
        let lowest = stage.and_then(|s| {
            walker.preview_bounds(&s).map(|(lo, _)| {
                lo.z + tessera_client::walker::PREVIEW_PICK_PAD - s.figure_position.z
            })
        });
        let target = self
            .renderer
            .create_offscreen_target(size.0, size.1, OffscreenFormat::Rgba8Unorm)
            .expect("target");
        let mut frame = self.renderer.begin_offscreen_frame(&target).expect("frame");
        let drawn = walker.draw_preview(&mut frame, alone.as_ref());
        self.renderer.end_frame(frame).expect("end frame");
        let image = self.renderer.read_pixels(&target).expect("read back");
        let (w, h) = (size.0 as usize, size.1 as usize);
        let (mut x0, mut y0, mut x1, mut y1, mut n) = (usize::MAX, usize::MAX, 0, 0, 0usize);
        for (i, p) in image.data.as_chunks::<4>().0.iter().enumerate() {
            if p[0].max(p[1]).max(p[2]) > 6 {
                let (x, y) = (i % w, i / w);
                x0 = x0.min(x);
                y0 = y0.min(y);
                x1 = x1.max(x);
                y1 = y1.max(y);
                n += 1;
            }
        }
        let marker = stage.and_then(|s| s.project(size, s.figure_position));
        Framing {
            drawn,
            pixels: n,
            bbox: (n > 0).then_some((x0, y0, x1, y1)),
            size: (w, h),
            marker,
            lowest,
        }
    }

    /// The whole frame with the UI, written to `sheet` as `name`.png when given.
    pub fn capture(&mut self, sheet: Option<&PathBuf>, name: &str) {
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

    /// Ends the run: quits the screens and frees everything.
    pub fn shutdown(self) {
        let Client {
            mut rt,
            mut host,
            mut ui,
            mut renderer,
            ..
        } = self;
        ui.shutdown(&mut rt, Some(&mut renderer), &mut NoWindow);
        if let Some(w) = host.walker.as_mut() {
            w.shutdown(&mut renderer);
        }
        rt.shutdown();
    }
}

/// Where the figure stands in the viewport ([`Client::framing`]).
#[derive(Debug, Clone, Copy)]
pub struct Framing {
    /// The walker drew a figure.
    pub drawn: bool,
    /// Pixels the figure covers.
    pub pixels: usize,
    /// The figure's pixel box (left, top, right, bottom).
    pub bbox: Option<(usize, usize, usize, usize)>,
    /// The picture's size.
    pub size: (usize, usize),
    /// Where the marker the figure stands on appears (column, row).
    pub marker: Option<(f32, f32)>,
    /// The height of the figure's lowest joint above the marker, metres.
    pub lowest: Option<f32>,
}

impl Framing {
    /// The figure is drawn and covers a visible area (at least 0.5 % of the picture).
    pub fn present(&self) -> bool {
        self.drawn && self.pixels * 200 >= self.size.0 * self.size.1
    }

    /// The whole figure is inside the picture: its box touches no edge.
    pub fn in_frame(&self) -> bool {
        self.bbox.is_some_and(|(x0, y0, x1, y1)| {
            x0 > 0 && y0 > 0 && x1 + 1 < self.size.0 && y1 + 1 < self.size.1
        })
    }

    /// The figure stands on the marker: its lowest joint (a foot) lies between 0.12 m
    /// below and 0.25 m above the marker, so it is neither sunk into the ground nor
    /// floating (a figure standing waist-deep has its feet about half a metre below).
    pub fn on_marker(&self) -> bool {
        self.lowest.is_some_and(|z| (-0.12..=0.25).contains(&z))
    }
}
