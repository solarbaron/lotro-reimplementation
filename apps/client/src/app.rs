// SPDX-License-Identifier: GPL-3.0-or-later
//! The windowed client: a winit [`ApplicationHandler`] driving the frame loop.

use std::sync::Arc;
use std::time::{Duration, Instant};

use anyhow::Context as _;
use tessera_render::{RenderError, Renderer, RendererConfig, wgpu};
use winit::application::ApplicationHandler;
use winit::dpi::PhysicalSize;
use winit::event::{
    DeviceEvent, DeviceId, ElementState, KeyEvent, MouseButton, MouseScrollDelta, WindowEvent,
};
use winit::event_loop::{ActiveEventLoop, ControlFlow, EventLoop};
use winit::keyboard::{KeyCode, PhysicalKey};
use winit::window::{Window, WindowId};

use crate::cli::Args;
use crate::input::{CameraAction, CameraInput};
use crate::runtime::{DataStatus, Runtime};
use crate::scene::{CLEAR_COLOR, Scene, default_camera};
use crate::walker::hud::TextPanel;
use crate::walker_host::{WalkerHost, save_png};

/// Pixels of trackpad scrolling that count as one wheel notch.
const PIXELS_PER_NOTCH: f64 = 40.0;
/// How often the window title (frame rate, status) is refreshed.
const TITLE_INTERVAL: Duration = Duration::from_secs(1);

/// Run the windowed client until the window is closed.
pub fn run(args: Args) -> anyhow::Result<()> {
    // `--start-screen` and `--official` open Tessera's start screen first
    // (`official-entry.md` §4); "Explore offline" comes back to `App` below.
    if args.opens_start_screen() {
        return crate::official::window::run(args);
    }
    let event_loop = EventLoop::new().context("creating the event loop")?;
    event_loop.set_control_flow(ControlFlow::Poll);
    let mut app = App::new(args)?;
    event_loop.run_app(&mut app).context("event loop")?;
    app.shutdown();
    match app.error.take() {
        Some(e) => Err(e),
        None => Ok(()),
    }
}

/// Window, renderer and scene; created on `resumed`.
#[derive(Debug)]
struct Gfx {
    // Field order matters: the renderer (and its surface) drops before the window.
    renderer: Renderer<'static>,
    scene: Scene,
    window: Arc<Window>,
}

/// Longest scenario title shown in the window caption, in bytes (`scenario-feed.md` §4.3).
pub const MAX_CAPTION_TITLE: usize = 128;

/// A scenario title for the window caption: control characters replaced by spaces, runs of
/// white space collapsed, at most [`MAX_CAPTION_TITLE`] bytes (cut at a character
/// boundary); `None` without a title or when nothing printable remains.
#[must_use]
pub fn scenario_caption(title: Option<&str>) -> Option<String> {
    let cleaned: String = title?
        .chars()
        .map(|c| if c.is_control() { ' ' } else { c })
        .collect::<String>()
        .split_whitespace()
        .collect::<Vec<_>>()
        .join(" ");
    let mut end = cleaned.len().min(MAX_CAPTION_TITLE);
    while !cleaned.is_char_boundary(end) {
        end -= 1;
    }
    let t = cleaned[..end].trim_end();
    (!t.is_empty()).then(|| t.to_owned())
}

/// The overall deadline of a windowed scenario timing run (`--scenario-timing` without
/// `--ui-script`), counted from `now`: `--settle-timeout` + the timed seconds + 180 s, as
/// the headless UI run allows (`crate::ui_host::run`).
fn timing_deadline(args: &Args, now: Instant) -> Option<Instant> {
    let secs = args.scenario_timing.filter(|_| !args.ui_script)?;
    Some(crate::runtime::deadline_after(
        now,
        Duration::from_secs(
            args.settle_timeout
                .saturating_add(secs)
                .saturating_add(TIMING_LOAD_SECS),
        ),
    ))
}

/// Seconds a scenario timing run allows for loading the world, on top of the settle
/// timeout and the timed seconds (as the headless UI run, `crate::ui_host::run`).
const TIMING_LOAD_SECS: u64 = 180;

#[derive(Debug)]
pub(crate) struct App {
    args: Args,
    runtime: Option<Runtime>,
    gfx: Option<Gfx>,
    input: CameraInput,
    last_frame: Instant,
    title_at: Instant,
    frames_since_title: u32,
    frames_total: u64,
    error: Option<anyhow::Error>,
    /// The world walker (walker mode only).
    host: Option<WalkerHost>,
    /// Loading/status text while the walker is not running.
    status_panel: TextPanel,
    /// The game UI attached to the walker (`--ui`, M5).
    ui: Option<crate::ui_host::client::ClientUi>,
    /// The settings' `[Input]` section (double-click time and rectangle, `keymap.md` §7.7).
    input_prefs: tessera_settings::InputPrefs,
    /// `"<scenario title> - "` for the window caption of a scenario run with a title
    /// (`scenario-feed.md` §4.3 `title`), see [`scenario_caption`].
    scenario_caption: Option<String>,
    /// The window's scale factor the mapper's timing was last computed for.
    scale_factor: f64,
    /// A scenario timing run's overall deadline (`--scenario-timing`): the settle timeout,
    /// the timed seconds and 180 s for loading, as the headless UI run allows. A windowed
    /// run that has not written its report by then fails, so it never stays open for ever
    /// (a walker that failed, or a player never placed).
    timing_deadline: Option<Instant>,
    /// The world session was handed over by the official login (`official-entry.md` §8.7):
    /// quitting the character screens, closing the window and the session's end do not end
    /// the client; they end this part of the run ([`App::handed_over_ended`]) and the start
    /// screen's window takes over again (the account form, §8.7).
    handed_over: bool,
    /// The handed-over part of the run is over (see `handed_over`).
    ended: bool,
}

/// The pointer-lock window operations over the winit window (decision 0015 §2.5).
struct WinitCursor<'a>(&'a Window);

impl crate::ui_host::cursor::CursorWindow for WinitCursor<'_> {
    fn set_cursor_visible(&mut self, visible: bool) {
        self.0.set_cursor_visible(visible);
    }

    fn set_grab(&mut self, grab: bool) -> bool {
        use winit::window::CursorGrabMode;
        if !grab {
            return self.0.set_cursor_grab(CursorGrabMode::None).is_err();
        }
        self.0.set_cursor_grab(CursorGrabMode::Locked).is_ok()
            || self.0.set_cursor_grab(CursorGrabMode::Confined).is_ok()
    }

    fn warp(&mut self, x: f64, y: f64) -> bool {
        self.0
            .set_cursor_position(winit::dpi::PhysicalPosition::new(x, y))
            .is_ok()
    }
}

fn key_action(code: KeyCode) -> Option<CameraAction> {
    Some(match code {
        KeyCode::KeyA | KeyCode::ArrowLeft => CameraAction::YawLeft,
        KeyCode::KeyD | KeyCode::ArrowRight => CameraAction::YawRight,
        KeyCode::KeyW | KeyCode::ArrowUp => CameraAction::PitchUp,
        KeyCode::KeyS | KeyCode::ArrowDown => CameraAction::PitchDown,
        KeyCode::KeyE | KeyCode::Equal | KeyCode::NumpadAdd | KeyCode::PageUp => {
            CameraAction::ZoomIn
        }
        KeyCode::KeyQ | KeyCode::Minus | KeyCode::NumpadSubtract | KeyCode::PageDown => {
            CameraAction::ZoomOut
        }
        _ => return None,
    })
}

impl App {
    pub(crate) fn new(args: Args) -> anyhow::Result<Self> {
        Self::with_session(args, None)
    }

    /// The windowed client over `handed`, the session the official login brought to
    /// character selection (`official-entry.md` §8.7), when given: the character screens
    /// open over it (`args.character_select`), and the end of this part of the run is
    /// reported by [`App::handed_over_ended`] instead of ending the event loop.
    pub(crate) fn with_session(
        args: Args,
        handed: Option<crate::net::HandOver>,
    ) -> anyhow::Result<Self> {
        let handed_over = handed.is_some();
        if let Some(path) = &args.frame_log {
            crate::runtime::refuse_install_path(&args, None, path)?;
        }
        let now = Instant::now();
        let ui_args = if args.ui_script {
            crate::ui_host::run::script_args(&args)?
        } else {
            args.clone()
        };
        let mut runtime = Runtime::start_with(
            &crate::ui_host::client::ClientUi::runtime_args(&ui_args),
            now,
            handed,
        )?;
        let host = runtime
            .walker_shared
            .clone()
            .map(|shared| WalkerHost::new(&args, shared));
        if host.is_none() {
            tracing::info!(
                "controls: drag with left/right mouse or WASD/arrows to orbit, wheel or Q/E to zoom, \
                 R reset, T say hello, P list plugins, Esc quit"
            );
        }
        // The NPC windows' `--open-panel` names are the NPC hub's (decision 0024 P3).
        let mut client_args = ui_args.clone();
        client_args
            .open_panel
            .retain(|n| crate::npc::parse_npc_panel(n).is_none());
        // The quest journal's names are the quest hub's (P3).
        client_args
            .open_panel
            .retain(|n| crate::quests::parse_journal_panel(n).is_none());
        // The map window's and the travel windows' names are the P5 hubs'.
        client_args.open_panel.retain(|n| {
            !crate::map::parse_map_panel(n) && crate::travel::parse_travel_panel(n).is_none()
        });
        let mut ui = (host.is_some() && args.ui_enabled())
            .then(|| crate::ui_host::client::ClientUi::new(&client_args));
        if let Some(u) = ui.as_mut() {
            u.prepare(&mut runtime);
        }
        let args_caption = args
            .scenario_prepared
            .as_deref()
            .and_then(|sc| scenario_caption(sc.title()))
            .map(|t| format!("{t} - "));
        let timing_deadline = timing_deadline(&args, now);
        Ok(Self {
            args,
            runtime: Some(runtime),
            gfx: None,
            input: CameraInput::default(),
            last_frame: now,
            title_at: now,
            frames_since_title: 0,
            frames_total: 0,
            error: None,
            ui,
            host,
            status_panel: TextPanel::default(),
            input_prefs: crate::ui_host::cursor::input_prefs(),
            scale_factor: 1.0,
            scenario_caption: args_caption,
            timing_deadline,
            handed_over,
            ended: false,
        })
    }

    /// Whether the handed-over part of an official run is over (the player left the
    /// character screens, closed the window, or the session ended): the start screen's
    /// window shuts this client down and shows the account form (`official-entry.md` §8.7).
    pub(crate) fn handed_over_ended(&self) -> bool {
        self.ended
    }

    /// Ends the event loop, or, for a handed-over session, only this part of the run.
    fn exit(&mut self, event_loop: &ActiveEventLoop) {
        if self.handed_over {
            if !self.ended {
                tracing::info!("official session: leaving the character screens and the world");
            }
            self.ended = true;
        } else {
            event_loop.exit();
        }
    }

    /// Ends a windowed scenario timing run that has not finished by its deadline, or whose
    /// walker failed: the run's error (exit non-zero) instead of a window that stays open.
    fn check_timing_run(&mut self, event_loop: &ActiveEventLoop, now: Instant) -> bool {
        let Some(deadline) = self.timing_deadline else {
            return false;
        };
        let failed = self.host.as_ref().and_then(|h| h.failed.clone());
        let err = match failed {
            Some(reason) => anyhow::anyhow!("scenario timing run: world walker failed: {reason}"),
            None if now > deadline => anyhow::anyhow!(
                "scenario timing run: the timed window did not end in time (settle timeout + timing + 180 s)"
            ),
            None => return false,
        };
        self.fail(event_loop, err);
        true
    }

    fn fail(&mut self, event_loop: &ActiveEventLoop, err: anyhow::Error) {
        tracing::error!("{err:#}");
        self.error.get_or_insert(err);
        event_loop.exit();
    }

    fn create_gfx(&self, event_loop: &ActiveEventLoop) -> anyhow::Result<Gfx> {
        let (w, h) = self.args.size;
        let title = match &self.scenario_caption {
            Some(c) => format!("Tessera - {}", c.trim_end_matches(" - ")),
            None => "Tessera".to_owned(),
        };
        let attrs = Window::default_attributes()
            .with_title(title)
            .with_inner_size(PhysicalSize::new(w, h))
            .with_min_inner_size(PhysicalSize::new(64u32, 64u32));
        let window = Arc::new(
            event_loop
                .create_window(attrs)
                .context("creating the window")?,
        );
        let size = window.inner_size();
        let walker = self.host.is_some();
        let config = RendererConfig {
            // No vsync: Immediate where the surface offers it (may tear), else Mailbox, else
            // Fifo (the renderer resolves the alias against the surface's modes).
            present_mode: if self.args.no_vsync {
                wgpu::PresentMode::AutoNoVsync
            } else {
                wgpu::PresentMode::Fifo
            },
            clear_color: if walker {
                crate::walker::SKY
            } else {
                CLEAR_COLOR
            },
            meshes: if walker {
                crate::walker::mesh_config(crate::walker_host::walker_arena_bytes(&self.args))
            } else {
                tessera_render::MeshConfig::default()
            },
            ..RendererConfig::default()
        };
        let mut renderer = Renderer::new_windowed(
            Arc::clone(&window),
            size.width.max(1),
            size.height.max(1),
            &config,
        )
        .context("creating the renderer")?;
        let info = renderer.adapter_info();
        tracing::info!(adapter = %info.name, backend = ?info.backend, w = size.width, h = size.height, "window renderer ready");
        let scene = Scene::new(&mut renderer).context("creating the scene")?;
        Ok(Gfx {
            renderer,
            scene,
            window,
        })
    }

    fn redraw(&mut self, event_loop: &ActiveEventLoop) {
        let now = Instant::now();
        let dt = now.saturating_duration_since(self.last_frame).as_secs_f32();
        self.last_frame = now;
        if self.host.is_some() {
            self.redraw_walker(event_loop, now, dt);
            return;
        }
        let (Some(rt), Some(gfx)) = (self.runtime.as_mut(), self.gfx.as_mut()) else {
            return;
        };
        if let Some(tex) = rt.poll_loader()
            && let Err(e) = gfx.scene.set_surface(&mut gfx.renderer, &tex)
        {
            tracing::warn!(
                did = format_args!("{:#010x}", tex.did),
                "texture upload failed: {e}"
            );
        }
        rt.tick(now);
        self.input.apply(&mut gfx.scene.camera, dt);

        let result = match gfx.renderer.begin_frame() {
            Ok(Some(mut frame)) => {
                rt.draw(&gfx.scene, &mut frame);
                gfx.renderer.end_frame(frame).map(|_| true)
            }
            Ok(None) => Ok(false),
            Err(RenderError::SurfaceLost) => {
                tracing::warn!("surface lost; recreating it");
                let size = gfx.window.inner_size();
                gfx.renderer
                    .replace_surface(
                        Arc::clone(&gfx.window),
                        size.width.max(1),
                        size.height.max(1),
                    )
                    .map(|()| false)
            }
            Err(e) => Err(e),
        };
        let presented = match result {
            Ok(p) => p,
            Err(e) => {
                self.fail(
                    event_loop,
                    anyhow::Error::new(e).context("rendering a frame"),
                );
                return;
            }
        };

        self.frames_since_title += 1;
        self.frames_total += u64::from(presented);
        if self
            .args
            .exit_after_frames
            .is_some_and(|n| self.frames_total >= n)
        {
            tracing::info!(frames = self.frames_total, "frame limit reached; closing");
            event_loop.exit();
            return;
        }
        let elapsed = now.saturating_duration_since(self.title_at);
        if elapsed >= TITLE_INTERVAL {
            let fps = f64::from(self.frames_since_title) / elapsed.as_secs_f64();
            let shown = gfx.scene.shown();
            let what = shown.did.map_or_else(
                || "test pattern".to_owned(),
                |d| format!("surface {d:#010x}"),
            );
            let data = match &rt.status {
                DataStatus::Loading => "loading data",
                DataStatus::NoData(_) => "no data",
                DataStatus::Ready { .. } => "data ready",
            };
            let caption = self.scenario_caption.as_deref().unwrap_or("");
            gfx.window.set_title(&format!(
                "Tessera - {caption}{what} - {data} - {fps:.0} fps"
            ));
            self.title_at = now;
            self.frames_since_title = 0;
        }
        gfx.window.request_redraw();
    }

    /// One frame of the world walker (or its loading screen).
    fn redraw_walker(&mut self, event_loop: &ActiveEventLoop, now: Instant, dt: f32) {
        if self.check_timing_run(event_loop, now) {
            return;
        }
        let (Some(rt), Some(gfx), Some(host)) =
            (self.runtime.as_mut(), self.gfx.as_mut(), self.host.as_mut())
        else {
            return;
        };
        let _ = rt.poll_loader();
        rt.tick(now);
        let had_walker = host.walker.is_some();
        host.poll(rt, &mut gfx.renderer);
        if let Some(ui) = self.ui.as_mut() {
            ui.poll(rt, host.walker.as_mut());
        }
        let size = gfx.window.inner_size();
        let notices = rt.take_notices();
        let mut quit = false;
        let mut shot = false;
        let mut flight_done = false;
        let mut timing_done = false;
        if let Some(walker) = host.walker.as_mut() {
            if !had_walker {
                walker.set_viewport(size.width, size.height);
                self.status_panel.release(&mut gfx.renderer);
                // T, W and H from the settings (or defaults), W and H scaled by the window's
                // scale factor (`keymap.md` §7.7, decision 0014 D3).
                self.scale_factor = gfx.window.scale_factor();
                let timing = crate::ui_host::cursor::activation_timing(
                    &self.input_prefs,
                    None,
                    self.scale_factor,
                );
                walker.mapper_mut().0.set_timing(timing);
            }
            for n in notices {
                tracing::info!("world: {n}");
                walker.notice(n);
            }
            if let Some(ui) = self.ui.as_mut() {
                ui.before_update(walker, rt, &mut WinitCursor(&gfx.window));
            }
            let ev = walker.update(
                &mut gfx.renderer,
                rt.world
                    .as_mut()
                    .map(|w| w as &mut dyn crate::walker::WalkerSession),
                &mut rt.plugins,
                now,
                dt,
            );
            quit = ev.quit || ev.duration_done;
            timing_done = ev.duration_done;
            shot = ev.screenshot;
            flight_done = ev.flight_done;
            // The walker's first settled frame in the world ends the client's world loading
            // for this entry (`Command::WorldLoaded`, `official-messages-play.md` §4.5 item
            // 4); the official session's portal exit waits for it.
            if walker.settled()
                && let Some(w) = rt.world.as_mut()
            {
                w.world_loaded(now);
            }
            // P5: the dispatcher's `ride`, `findplace` and `leaveinstance`, the feed's
            // mounts for the walker, and the walker's mount state for the plugin views.
            crate::travel::hub::run_p5_requests(rt, walker, now);
            if let Some(ui) = self.ui.as_mut() {
                ui.after_update(walker, rt, &ev);
                // The per-frame window pass (decision 0024 P3–P6, decision 0040): the
                // Interface window, the NPC windows, the quest journal and its indicators,
                // the social panels, the P5 travel, finder and map windows and the P6
                // progression and crafting windows, all reading the frame's one list of UI
                // events (the character screens own it while they are open). The headless
                // `--ui` runs call the same function.
                crate::ui_host::client::window_frames(ui, rt, walker, &mut gfx.renderer, now);
                // The player and target frames' portraits (P4, P5), before step 8 paints
                // the frame's composition list (`crate::portrait` module docs).
                ui.render_combat_portraits(walker, &mut gfx.renderer);
                ui.step8(walker, &mut gfx.renderer, rt);
            }
        } else {
            let text = match (&host.failed, &rt.status) {
                (Some(reason), _) => format!("World walker unavailable: {reason}"),
                (None, DataStatus::Loading) => "Opening the archives...".to_owned(),
                (None, _) => "Loading the world...".to_owned(),
            };
            self.status_panel
                .set(&mut gfx.renderer, vec![(text, [240, 240, 235])], 2);
        }
        if let (Some(walker), Some(ui)) = (host.walker.as_mut(), self.ui.as_mut()) {
            // The paperdoll's portrait, before the frame that composes it.
            ui.render_portraits(walker, &mut gfx.renderer);
        }
        let result = match gfx.renderer.begin_frame() {
            Ok(Some(mut frame)) => {
                match host.walker.as_mut() {
                    Some(walker) => {
                        walker.draw(&gfx.renderer, &mut frame);
                        // Quest indicators over NPCs (P3), under the game UI.
                        let view_proj = walker.view_proj(frame.aspect());
                        rt.quests.draw_indicators(&mut frame, view_proj);
                        if let Some(ui) = self.ui.as_mut() {
                            ui.draw(&mut frame, true);
                        }
                    }
                    None => {
                        frame
                            .world_pass("loading", &tessera_render::Camera::default())
                            .clear(crate::walker::SKY);
                        let pass = frame.pass("loading", tessera_render::PassSpace::Screen);
                        self.status_panel.draw(pass, 20.0, 20.0);
                    }
                }
                gfx.renderer.end_frame(frame).map(|stats| {
                    if let Some(walker) = host.walker.as_mut() {
                        walker.record_render_stats(&stats);
                    }
                    // The UI pass's GPU time for U2 (render.md §6.16), as the headless
                    // `--ui` runs report it.
                    if let Some(ui) = self.ui.as_mut() {
                        ui.record_render_stats(&stats);
                    }
                    true
                })
            }
            Ok(None) => Ok(false),
            Err(RenderError::SurfaceLost) => {
                tracing::warn!("surface lost; recreating it");
                gfx.renderer
                    .replace_surface(
                        Arc::clone(&gfx.window),
                        size.width.max(1),
                        size.height.max(1),
                    )
                    .map(|()| false)
            }
            Err(e) => Err(e),
        };
        if let Some(ui) = self.ui.as_mut() {
            ui.end_frame(rt);
            if let Some((w, h)) = ui.take_screen_request() {
                // The scripted run's resize: ask the window for it; the Resized event then
                // resizes the renderer, the walker's viewport and the UI.
                let _ = gfx.window.request_inner_size(PhysicalSize::new(w, h));
            }
            if ui.script_done() {
                // The scripted run of decision 0015 §6 at the window size the compositor gave.
                let title = format!(
                    "{}x{} windowed, {}",
                    size.width,
                    size.height,
                    gfx.renderer.adapter_info().name
                );
                if let Some(md) = ui.report(&title) {
                    tracing::info!("UI budgets:\n{md}");
                    if let Some(path) = &self.args.ui_report {
                        let r =
                            crate::runtime::refuse_install_path(&self.args, Some(&rt.status), path)
                                .and_then(|()| {
                                    std::fs::write(path, &md).map_err(anyhow::Error::from)
                                });
                        if let Err(e) = r {
                            tracing::warn!(path = %path.display(), "UI report not written: {e:#}");
                        }
                    }
                }
                event_loop.exit();
                return;
            }
        }
        match result {
            Ok(p) => self.frames_total += u64::from(p),
            Err(e) => {
                self.fail(
                    event_loop,
                    anyhow::Error::new(e).context("rendering a frame"),
                );
                return;
            }
        }
        if timing_done
            && self.args.scenario_timing.is_some()
            && !self.args.ui_script
            && let Some(walker) = host.walker.as_ref()
        {
            // The scenario timing run's report (decision 0043 §3), at the surface the
            // compositor gave.
            let t = crate::timing::build(
                walker,
                self.ui.as_ref(),
                rt,
                crate::timing::RunMode::Windowed,
                (size.width, size.height),
                !self.args.no_vsync,
                &gfx.renderer.adapter_info().name,
            );
            if let Err(e) = crate::timing::emit(&t, &self.args, Some(&rt.status)) {
                // As the headless run: a report that could not be written fails the run
                // (exit non-zero); the window closes below (`quit`).
                let e = e.context("timing report not written");
                tracing::error!("{e:#}");
                self.error.get_or_insert(e);
            }
            // The report is written (or failed): the run is over.
            self.timing_deadline = None;
        }
        if shot && let Some(walker) = host.walker.as_mut() {
            let dir = crate::runtime::data_root(&self.args).join("screenshots");
            let stamp = std::time::SystemTime::now()
                .duration_since(std::time::UNIX_EPOCH)
                .map_or(0, |d| d.as_secs());
            let path = dir.join(format!("walker-{stamp}.png"));
            // The read-back waits for the GPU; encoding and writing go to a thread.
            if let Err(e) = crate::runtime::refuse_install_path(&self.args, Some(&rt.status), &path)
            {
                tracing::warn!("screenshot not saved: {e:#}");
                walker.notice("screenshot not saved: the data folder is inside the install");
            } else {
                // With a game UI the image holds the UI pass too, but never the software
                // cursor (`render.md` §6.11).
                match crate::ui_host::run::capture_frame(
                    &mut gfx.renderer,
                    walker,
                    self.ui.as_mut(),
                    (size.width.max(1), size.height.max(1)),
                ) {
                    Ok(img) => {
                        walker.notice(format!("saving screenshot {}", path.display()));
                        let spawned = std::thread::Builder::new()
                            .name("tessera-screenshot".into())
                            .spawn(move || match save_png(&path, &img) {
                                Ok(()) => {
                                    tracing::info!(path = %path.display(), "screenshot saved")
                                }
                                Err(e) => tracing::warn!("screenshot failed: {e:#}"),
                            });
                        if let Err(e) = spawned {
                            tracing::warn!("screenshot thread: {e}");
                        }
                    }
                    Err(e) => tracing::warn!("screenshot failed: {e:#}"),
                }
            }
        }
        if quit
            || flight_done
            || self.ui.as_ref().is_some_and(|u| u.quit_requested())
            || (self.handed_over
                && rt
                    .world
                    .as_ref()
                    .is_some_and(|w| w.state() == tessera_net::SessionState::Closed))
            || self
                .args
                .exit_after_frames
                .is_some_and(|n| self.frames_total >= n)
        {
            self.exit(event_loop);
            return;
        }
        let elapsed = now.saturating_duration_since(self.title_at);
        self.frames_since_title += 1;
        if elapsed >= TITLE_INTERVAL {
            let fps = f64::from(self.frames_since_title) / elapsed.as_secs_f64();
            let place = host
                .walker
                .as_ref()
                .and_then(|w| w.describe())
                .unwrap_or_else(|| "loading".into());
            let caption = self.scenario_caption.as_deref().unwrap_or("");
            gfx.window
                .set_title(&format!("Tessera walker - {caption}{place} - {fps:.0} fps"));
            self.title_at = now;
            self.frames_since_title = 0;
        }
        gfx.window.request_redraw();
    }

    fn on_key(&mut self, event_loop: &ActiveEventLoop, event: &KeyEvent) {
        let PhysicalKey::Code(code) = event.physical_key else {
            return;
        };
        let pressed = event.state == ElementState::Pressed;
        if let Some(action) = key_action(code) {
            self.input.set_action(action, pressed);
            return;
        }
        if !pressed || event.repeat {
            return;
        }
        match code {
            KeyCode::Escape => event_loop.exit(),
            KeyCode::KeyR => {
                if let Some(gfx) = &mut self.gfx {
                    gfx.scene.camera = default_camera();
                }
            }
            KeyCode::KeyT => {
                if let Some(world) = self.runtime.as_mut().and_then(|r| r.world.as_mut()) {
                    world.say(Instant::now(), "Hello from Tessera");
                }
            }
            KeyCode::KeyP => {
                if let Some(rt) = &mut self.runtime {
                    rt.plugins.command("/plugins list");
                }
            }
            _ => {}
        }
    }

    /// The first error of the run, if one ended it.
    pub(crate) fn take_error(&mut self) -> Option<anyhow::Error> {
        self.error.take()
    }

    /// Idempotent orderly shutdown.
    pub(crate) fn shutdown(&mut self) {
        if let (Some(ui), Some(rt)) = (self.ui.as_mut(), self.runtime.as_mut()) {
            // After the last frame: every apartment unloads (decision 0014 D8).
            match self.gfx.as_mut() {
                Some(g) => {
                    let mut cursor = WinitCursor(&g.window);
                    ui.shutdown(rt, Some(&mut g.renderer), &mut cursor);
                }
                None => ui.shutdown(rt, None, &mut crate::ui_host::cursor::NoWindow),
            }
        }
        if let (Some(host), Some(gfx)) = (self.host.as_mut(), self.gfx.as_mut())
            && let Some(mut walker) = host.walker.take()
        {
            if let Some(log) = walker.frame_log() {
                tracing::info!("frame summary: {}", log.summary());
                let status = self.runtime.as_ref().map(|rt| &rt.status);
                if let Some(path) = &self.args.frame_log {
                    match crate::runtime::refuse_install_path(&self.args, status, path)
                        .and_then(|()| log.write_csv(path).map_err(anyhow::Error::from))
                    {
                        Ok(()) => tracing::info!(path = %path.display(), "frame log written"),
                        Err(e) => tracing::warn!(path = %path.display(), "frame log: {e:#}"),
                    }
                }
            }
            walker.shutdown(&mut gfx.renderer);
        }
        // The quest indicators' glyphs (P3).
        if let (Some(rt), Some(gfx)) = (self.runtime.as_mut(), self.gfx.as_mut()) {
            rt.quests.release(&mut gfx.renderer);
        }
        if let Some(mut rt) = self.runtime.take() {
            rt.shutdown();
        }
        if let Some(gfx) = self.gfx.take() {
            if let Err(e) = gfx.renderer.context().wait_idle() {
                tracing::warn!("GPU did not go idle cleanly: {e}");
            }
            drop(gfx);
            tracing::info!("window closed");
        }
    }
}

impl ApplicationHandler for App {
    fn resumed(&mut self, event_loop: &ActiveEventLoop) {
        if self.gfx.is_some() {
            return;
        }
        match self.create_gfx(event_loop) {
            Ok(gfx) => {
                gfx.window.request_redraw();
                self.last_frame = Instant::now();
                self.gfx = Some(gfx);
            }
            Err(e) => self.fail(event_loop, e),
        }
    }

    fn window_event(&mut self, event_loop: &ActiveEventLoop, _id: WindowId, event: WindowEvent) {
        if let Some(host) = &mut self.host {
            if let Some(walker) = host.walker.as_mut() {
                match (self.ui.as_mut(), self.runtime.as_mut(), self.gfx.as_ref()) {
                    (Some(ui), Some(rt), Some(gfx)) if ui.attached() => {
                        let mut cursor = WinitCursor(&gfx.window);
                        if matches!(event, WindowEvent::Focused(false)) {
                            rt.npc.release_keys();
                            rt.map.release_keys();
                        }
                        tessera_input::winit_adapter::window_event(&event, |e| {
                            // A modal engine dialog takes every key (`ui-widgets.md`
                            // §18.4): the keys below are game code's, so they do not act
                            // while one is shown (their modifier and release tracking still
                            // sees every event).
                            let modal = ui.host().is_some_and(|h| h.modal_dialog_shown());
                            let npc_key = matches!(
                                e,
                                tessera_input::RawEvent::Key {
                                    key: tessera_input::KeyCode::Escape
                                        | tessera_input::KeyCode::Enter,
                                    pressed: true,
                                    ..
                                }
                            );
                            // Escape and Enter on an active NPC window (P3, npc-windows.md
                            // §5.7, §7.5, §8.3) go to that window only.
                            if !(modal && npc_key)
                                && ui.host().is_some_and(|h| rt.npc.raw_key(h, &e))
                            {
                                return;
                            }
                            // U (`use_selection`) and L (`toggle_quest_log`) are keymap
                            // actions (`keymap.md` §8.3); their routed commands reach the
                            // NPC and quest hubs, so no raw-key test is made here.
                            // P5: M (map), Ctrl+J (finder). Every event updates the
                            // modifiers; the keys act only without focus. `toggle_mount`
                            // (Ctrl+M) is a `tessera-input` action the walker's controls
                            // take (`walker.md` §16.4).
                            let focused = modal || ui.host().is_some_and(|h| h.keyboard_focused());
                            if let Some(crate::map::P5Key::Finder) = rt.map.key(&e, focused) {
                                rt.p5.request_finder();
                            }
                            ui.raw_event(walker, rt, &e, &mut cursor);
                        });
                    }
                    // Without the game UI the walker's controls take every key,
                    // `toggle_mount` (Ctrl+M, `walker.md` §16.4) among them.
                    _ => tessera_input::winit_adapter::window_event(&event, |e| walker.input(&e)),
                }
            }
            match event {
                WindowEvent::CloseRequested => self.exit(event_loop),
                WindowEvent::Resized(size) => {
                    if let Some(gfx) = &mut self.gfx {
                        gfx.renderer.resize(size.width, size.height);
                        gfx.window.request_redraw();
                    }
                    if let Some(w) = host.walker.as_mut() {
                        w.set_viewport(size.width, size.height);
                    }
                    if let Some(ui) = self.ui.as_mut() {
                        ui.resize(size.width, size.height);
                    }
                }
                WindowEvent::RedrawRequested => self.redraw(event_loop),
                WindowEvent::ScaleFactorChanged { scale_factor, .. } => {
                    // `keymap.md` §7.7 and §7.8.9: new W and H, recorded press positions
                    // forgotten, the cursor position (or the lock's frozen position)
                    // rescaled by s_new ÷ s_old.
                    let old = self.scale_factor;
                    let ratio = if old > 0.0 { scale_factor / old } else { 1.0 };
                    self.scale_factor = scale_factor;
                    // The automatic UI scale follows the monitor's scale factor σ
                    // (`ui-scaling.md` §4.2, §4.6).
                    if let Some(ui) = self.ui.as_mut() {
                        ui.set_os_scale(scale_factor);
                    }
                    if let Some(walker) = host.walker.as_mut() {
                        let timing = crate::ui_host::cursor::activation_timing(
                            &self.input_prefs,
                            None,
                            scale_factor,
                        );
                        let area = self.gfx.as_ref().map_or((1, 1), |g| {
                            let s = g.window.inner_size();
                            (
                                (f64::from(s.width) * ratio).round().max(1.0) as u32,
                                (f64::from(s.height) * ratio).round().max(1.0) as u32,
                            )
                        });
                        let handled = self
                            .ui
                            .as_mut()
                            .is_some_and(|ui| ui.rescale(walker, timing, ratio, area));
                        if !handled {
                            let (mapper, _) = walker.mapper_mut();
                            let cursor = crate::ui_host::rescaled_cursor(mapper.cursor(), ratio);
                            mapper.rescale(timing, cursor);
                        }
                    }
                }
                WindowEvent::KeyboardInput { event, .. }
                    if host.walker.is_none()
                        && event.state == ElementState::Pressed
                        && event.physical_key == PhysicalKey::Code(KeyCode::Escape) =>
                {
                    self.exit(event_loop);
                }
                _ => {}
            }
            return;
        }
        match event {
            WindowEvent::CloseRequested => event_loop.exit(),
            WindowEvent::Resized(size) => {
                if let Some(gfx) = &mut self.gfx {
                    gfx.renderer.resize(size.width, size.height);
                    gfx.window.request_redraw();
                }
            }
            WindowEvent::RedrawRequested => self.redraw(event_loop),
            WindowEvent::KeyboardInput { event, .. } => self.on_key(event_loop, &event),
            WindowEvent::Focused(false) => self.input.release_all(),
            WindowEvent::MouseInput { state, button, .. } => {
                if matches!(button, MouseButton::Left | MouseButton::Right) {
                    self.input.set_dragging(state == ElementState::Pressed);
                }
            }
            WindowEvent::CursorMoved { position, .. } => {
                self.input.cursor_moved(position.x, position.y);
            }
            WindowEvent::CursorLeft { .. } => self.input.cursor_left(),
            WindowEvent::MouseWheel { delta, .. } => {
                let notches = match delta {
                    MouseScrollDelta::LineDelta(_, y) => y,
                    MouseScrollDelta::PixelDelta(p) => (p.y / PIXELS_PER_NOTCH) as f32,
                };
                self.input.wheel(notches);
            }
            _ => {}
        }
    }

    fn device_event(&mut self, _event_loop: &ActiveEventLoop, _id: DeviceId, event: DeviceEvent) {
        if let Some(walker) = self.host.as_mut().and_then(|h| h.walker.as_mut()) {
            match (self.ui.as_mut(), self.runtime.as_mut(), self.gfx.as_ref()) {
                (Some(ui), Some(rt), Some(gfx)) if ui.attached() => {
                    let mut cursor = WinitCursor(&gfx.window);
                    tessera_input::winit_adapter::device_event(&event, |e| {
                        ui.raw_event(walker, rt, &e, &mut cursor);
                    });
                }
                _ => tessera_input::winit_adapter::device_event(&event, |e| walker.input(&e)),
            }
        }
    }

    fn exiting(&mut self, _event_loop: &ActiveEventLoop) {
        self.shutdown();
    }
}

#[cfg(test)]
mod tests {
    #[test]
    fn scenario_titles_are_cleaned_for_the_caption() {
        assert_eq!(super::scenario_caption(None), None);
        assert_eq!(super::scenario_caption(Some(" \t\n")), None);
        assert_eq!(
            super::scenario_caption(Some("Evening\u{7}at  the\ninn ")).as_deref(),
            Some("Evening at the inn")
        );
        let long = "é".repeat(100);
        let c = super::scenario_caption(Some(&long)).expect("caption");
        assert!(c.len() <= super::MAX_CAPTION_TITLE && c.chars().all(|ch| ch == 'é'));
    }

    use super::*;

    #[test]
    fn keymap_covers_every_action() {
        let codes = [
            KeyCode::KeyA,
            KeyCode::KeyD,
            KeyCode::KeyW,
            KeyCode::KeyS,
            KeyCode::KeyE,
            KeyCode::KeyQ,
        ];
        let mapped: Vec<CameraAction> = codes.iter().filter_map(|c| key_action(*c)).collect();
        for a in CameraInput::actions() {
            assert!(mapped.contains(a), "{a:?} unbound");
        }
        assert_eq!(key_action(KeyCode::KeyZ), None);
    }
}
