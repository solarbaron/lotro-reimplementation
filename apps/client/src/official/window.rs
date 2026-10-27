// SPDX-License-Identifier: GPL-3.0-or-later
//! The window of a start-screen run (`--start-screen`, `--official`;
//! `docs/specs/official-entry.md` §4).
//!
//! [`EntryApp`] owns the window while the start, warning, account-form, login and character
//! screens show. Its `window_event` is the client's window-event handler of such a run: the
//! one place that makes [`DeviceInput`] values (§5.4 rule 1), handed directly to the flow.
//! "Explore offline" closes this window and starts the offline walker as the client does
//! without a mode option (`crate::app::App`, which starts the plugin host with the
//! `--load-plugin` queue, §4.5); from then on every event goes to it.
//!
//! The hand-over of §8.7 does the same with the official session: this window closes and a
//! client over that session (`crate::app::App::with_session`, the character screens of
//! `--character-select`, with its own plugin host, §4.5) takes every event. When its part of
//! the run ends ([`crate::app::App::handed_over_ended`]: the player left the character
//! screens, closed the window, the session ended, or the character screens or the walker
//! could not start) it is shut down (its session ends and its plugin host stops) and this
//! window opens again on the account form, both fields empty, with the end's message. The
//! handed-over client gets an allowlist of this run's options ([`handed_over_args`]), never
//! a copy of them.

use std::sync::Arc;
use std::sync::mpsc;
use std::time::{Duration, Instant};

use anyhow::Context as _;
use tessera_compat::consent::{self, WarningText};
use tessera_render::{
    Camera, Filter, PassSpace, Quad, RenderError, Renderer, RendererConfig, TextureId,
    TextureUpload, Wrap, wgpu,
};
use winit::application::ApplicationHandler;
use winit::dpi::PhysicalSize;
use winit::event::{DeviceEvent, DeviceId, WindowEvent};
use winit::event_loop::{ActiveEventLoop, ControlFlow, EventLoop};
use winit::window::{Window, WindowId};

use super::backend::ProductionBackends;
use super::flow::{Flow, FlowConfig, FlowRequest};
use super::input::{DeviceInput, Modifiers};
use super::plugin_host::PluginSlot;
use super::view::{self, Fonts, Scene};
use crate::cli::Args;

/// How often the screens are redrawn while nothing happens (the warning's delay, the
/// queue's updates, the watchdog).
const FRAME_INTERVAL: Duration = Duration::from_millis(50);

/// Runs a start-screen run until the client ends.
///
/// # Errors
///
/// The event loop, the window or the renderer failed, or the offline walker did.
pub fn run(args: Args) -> anyhow::Result<()> {
    let event_loop = EventLoop::new().context("creating the event loop")?;
    event_loop.set_control_flow(ControlFlow::Poll);
    let mut app = EntryApp::new(args);
    event_loop.run_app(&mut app).context("event loop")?;
    app.finish()
}

/// The screens' window and renderer.
#[derive(Debug)]
struct Gfx {
    // Field order: the renderer (and its surface) drops before the window.
    renderer: Renderer<'static>,
    window: Arc<Window>,
    texture: Option<(TextureId, u32, u32)>,
    painted: Option<u64>,
}

/// The fonts, rasterised on a worker.
#[derive(Debug)]
enum FontSlot {
    Loading(mpsc::Receiver<Option<Arc<Fonts>>>),
    Ready(Option<Arc<Fonts>>),
}

/// The handler of a start-screen run (module docs).
#[derive(Debug)]
pub struct EntryApp {
    args: Args,
    flow: Flow,
    gfx: Option<Gfx>,
    fonts: Option<FontSlot>,
    scene: Scene,
    modifiers: Modifiers,
    offline: Option<crate::app::App>,
    /// The client over the handed-over official session (§8.7), while it runs.
    official: Option<crate::app::App>,
    error: Option<anyhow::Error>,
}

/// The options of the client that takes the handed-over official session (§8.7): built
/// from an allowlist of the start-screen run's options (the install, Tessera's folders, the
/// window, rendering, audio and UI scale), never cloned, so no scripted or automation
/// option (`--command`, `--duration`, `--exit-after-frames`, `--fly-path`, `--open-panel`,
/// `--mount`, a scenario, `--load-plugin`, ...) and no test-server option reaches official
/// play (`official-entry.md` §4.2: no synthetic input; decision 0034: automation stays
/// offline). The `--command` lines dropped are counted in a warning line, as §4.5 does
/// for `--load-plugin`. The character screens open (`--character-select`).
///
/// # Errors
///
/// The client's default options did not parse (not expected).
pub fn handed_over_args(args: &Args) -> anyhow::Result<Args> {
    use clap::Parser as _;
    if !args.commands.is_empty() {
        tracing::warn!(
            dropped = args.commands.len(),
            "official play: {} --command line(s) dropped (no synthetic input in official play)",
            args.commands.len()
        );
    }
    // Every option at its default (no environment variable feeds an option).
    let mut a = Args::try_parse_from(["tessera-client"]).context("the default options")?;
    // The install and Tessera's own folders.
    a.game_dir.clone_from(&args.game_dir);
    a.plugins.clone_from(&args.plugins);
    a.data_dir.clone_from(&args.data_dir);
    a.no_entry_count = args.no_entry_count;
    a.keymap.clone_from(&args.keymap);
    // The window and rendering.
    a.size = args.size;
    a.no_vsync = args.no_vsync;
    a.residency_mib = args.residency_mib;
    a.view_blocks = args.view_blocks;
    a.object_distance = args.object_distance;
    a.gpu_mib = args.gpu_mib;
    a.landscape_distance = args.landscape_distance;
    a.object_draw_distance = args.object_draw_distance;
    a.model_detail = args.model_detail;
    a.frill_distance = args.frill_distance;
    a.far_band = args.far_band;
    a.far_view = args.far_view;
    a.render_prefs.clone_from(&args.render_prefs);
    a.texture_cap = args.texture_cap;
    // Audio.
    a.audio = args.audio;
    a.audio_prefs.clone_from(&args.audio_prefs);
    a.audio_settings.clone_from(&args.audio_settings);
    // The UI and its scale.
    a.ui_scale = args.ui_scale;
    a.ui_safe_aspect = args.ui_safe_aspect;
    a.ui_cursor = args.ui_cursor;
    a.skin.clone_from(&args.skin);
    a.skins_root.clone_from(&args.skins_root);
    a.walker_hud = args.walker_hud;
    a.walker_minimap = args.walker_minimap;
    a.character_select = true;
    Ok(a)
}

/// Reads `[Official] last_world` from Tessera's settings file (§8.5).
fn last_world(path: &std::path::Path) -> Option<String> {
    let settings = tessera_settings::SettingsStore::new(path)
        .try_load()
        .ok()
        .flatten()?;
    let v = settings
        .extra
        .get("Official")?
        .as_table()?
        .get("last_world")?;
    let s = v.as_str()?;
    (!s.is_empty()).then(|| tessera_net::model::sanitize_text(s, 64).into_owned())
}

/// The flow's configuration for `args`: the record in `record_dir`, `[Official]
/// last_world` from `settings_file`.
#[must_use]
pub fn flow_config(
    args: &Args,
    record_dir: Option<std::path::PathBuf>,
    settings_file: Option<std::path::PathBuf>,
) -> FlowConfig {
    FlowConfig {
        record_dir,
        text: Arc::new(WarningText::shipped()),
        last_world: settings_file.as_deref().and_then(last_world),
        settings_file,
        queued_plugin_loads: args.load_plugins.len(),
    }
}

/// The flow of a start-screen run: on the start screen, or (`--official`) at §9 step 1.
#[must_use]
pub fn start_flow(
    args: &Args,
    config: FlowConfig,
    factory: Box<dyn super::flow::BackendFactory>,
    plugins: Box<dyn super::flow::PluginHostControl>,
) -> Flow {
    let mut flow = Flow::new(config, factory, plugins);
    if args.official {
        // `--official` only selects (§4.2): §9 from step 1.
        flow.choose_official();
    }
    flow
}

impl EntryApp {
    fn new(args: Args) -> Self {
        let root = crate::runtime::data_root(&args);
        let plugins_dir = args.plugins.clone().unwrap_or_else(|| root.join("Plugins"));
        let config = flow_config(
            &args,
            consent::default_record_dir(),
            tessera_settings::default_path().ok(),
        );
        let flow = start_flow(
            &args,
            config,
            Box::new(ProductionBackends::new(args.game_dir.clone())),
            Box::new(PluginSlot::new(plugins_dir, root.join("plugindata"))),
        );
        Self {
            args,
            flow,
            gfx: None,
            fonts: None,
            scene: Scene::default(),
            modifiers: Modifiers::default(),
            offline: None,
            official: None,
            error: None,
        }
    }

    fn finish(mut self) -> anyhow::Result<()> {
        if let Some(mut app) = self.official.take() {
            app.shutdown();
        }
        self.flow.finish_writes();
        if let Some(mut app) = self.offline.take() {
            app.shutdown();
            if let Some(e) = app.take_error() {
                return Err(e);
            }
        }
        self.release();
        match self.error.take() {
            Some(e) => Err(e),
            None => Ok(()),
        }
    }

    fn fail(&mut self, event_loop: &ActiveEventLoop, err: anyhow::Error) {
        tracing::error!("{err:#}");
        self.error.get_or_insert(err);
        event_loop.exit();
    }

    fn create_gfx(&mut self, event_loop: &ActiveEventLoop) -> anyhow::Result<()> {
        let (w, h) = self.args.size;
        let attrs = Window::default_attributes()
            .with_title("Tessera")
            .with_inner_size(PhysicalSize::new(w, h))
            .with_min_inner_size(PhysicalSize::new(320u32, 240u32));
        let window = Arc::new(
            event_loop
                .create_window(attrs)
                .context("creating the window")?,
        );
        // §7.2: no input-method composition on these screens.
        window.set_ime_allowed(false);
        let size = window.inner_size();
        let config = RendererConfig {
            present_mode: if self.args.no_vsync {
                wgpu::PresentMode::AutoNoVsync
            } else {
                wgpu::PresentMode::Fifo
            },
            ..RendererConfig::default()
        };
        let renderer = Renderer::new_windowed(
            Arc::clone(&window),
            size.width.max(1),
            size.height.max(1),
            &config,
        )
        .context("creating the renderer")?;
        let scale = window.scale_factor();
        let (tx, rx) = mpsc::sync_channel(1);
        let spawned = std::thread::Builder::new()
            .name("tessera-official-fonts".into())
            .spawn(move || {
                let fonts = match Fonts::load(scale) {
                    Ok(f) => Some(Arc::new(f)),
                    Err(e) => {
                        tracing::error!("the bundled font cannot be drawn: {e}");
                        None
                    }
                };
                let _ = tx.send(fonts);
            });
        self.fonts = Some(match spawned {
            Ok(_) => FontSlot::Loading(rx),
            Err(_) => FontSlot::Ready(None),
        });
        window.request_redraw();
        self.gfx = Some(Gfx {
            renderer,
            window,
            texture: None,
            painted: None,
        });
        Ok(())
    }

    fn fonts(&mut self) -> Option<Arc<Fonts>> {
        if let Some(FontSlot::Loading(rx)) = &self.fonts {
            match rx.try_recv() {
                Ok(f) => self.fonts = Some(FontSlot::Ready(f)),
                Err(mpsc::TryRecvError::Disconnected) => self.fonts = Some(FontSlot::Ready(None)),
                Err(mpsc::TryRecvError::Empty) => {}
            }
        }
        match &self.fonts {
            Some(FontSlot::Ready(f)) => f.clone(),
            _ => None,
        }
    }

    fn release(&mut self) {
        if let Some(mut gfx) = self.gfx.take() {
            if let Some((id, _, _)) = gfx.texture.take() {
                gfx.renderer.destroy_texture(id);
            }
            if let Err(e) = gfx.renderer.context().wait_idle() {
                tracing::warn!("GPU did not go idle cleanly: {e}");
            }
        }
    }

    fn handle_request(&mut self, event_loop: &ActiveEventLoop) {
        match self.flow.take_request() {
            Some(FlowRequest::Quit) => event_loop.exit(),
            Some(FlowRequest::ExploreOffline) => self.explore_offline(event_loop),
            Some(FlowRequest::HandOver) => self.hand_over(event_loop),
            None => {}
        }
    }

    /// §8.7: the character screens and the world over the official session, in a client
    /// window of their own (module docs). If that client cannot start, the session it was
    /// given ends with it and this window shows the account form again.
    fn hand_over(&mut self, event_loop: &ActiveEventLoop) {
        let Some(handed) = self.flow.take_hand_over() else {
            return;
        };
        self.release();
        event_loop.set_control_flow(ControlFlow::Poll);
        let app = handed_over_args(&self.args)
            .and_then(|args| crate::app::App::with_session(args, Some(handed)));
        match app {
            Ok(mut app) => {
                app.resumed(event_loop);
                self.official = Some(app);
            }
            Err(e) => {
                tracing::error!("the character screens cannot open: {e:#}");
                self.flow
                    .characters_ended(super::flow::HandOverEnd::Failed(format!("{e:#}")));
                if let Err(e) = self.create_gfx(event_loop) {
                    self.fail(event_loop, e);
                }
            }
        }
    }

    /// The handed-over client's part of the run ended: it is shut down (its session ends
    /// and its plugin host stops before anything else shows, §4.5) and this window opens
    /// again on the account form.
    fn official_ended(&mut self, event_loop: &ActiveEventLoop) {
        if !self
            .official
            .as_ref()
            .is_some_and(crate::app::App::handed_over_ended)
        {
            return;
        }
        let mut end = super::flow::HandOverEnd::Left;
        if let Some(mut app) = self.official.take() {
            end = app
                .take_handed_end()
                .unwrap_or(super::flow::HandOverEnd::Left);
            app.shutdown();
            if let Some(e) = app.take_error() {
                tracing::error!("official session: {e:#}");
            }
        }
        self.flow.characters_ended(end);
        if self.gfx.is_none()
            && let Err(e) = self.create_gfx(event_loop)
        {
            self.fail(event_loop, e);
        }
    }

    /// "Explore offline": the offline walker as without a mode option (§4.1, §4.5).
    fn explore_offline(&mut self, event_loop: &ActiveEventLoop) {
        let mut args = self.args.clone();
        args.start_screen = false;
        args.official = false;
        if self.flow.plugin_loads_dropped() {
            args.load_plugins.clear();
        }
        self.release();
        event_loop.set_control_flow(ControlFlow::Poll);
        match crate::app::App::new(args) {
            Ok(mut app) => {
                app.resumed(event_loop);
                self.offline = Some(app);
            }
            Err(e) => self.fail(event_loop, e),
        }
    }

    fn frame(&mut self, event_loop: &ActiveEventLoop) {
        let now = Instant::now();
        self.flow.tick(now);
        self.handle_request(event_loop);
        if self.offline.is_some() || self.official.is_some() {
            return;
        }
        let fonts = self.fonts();
        let Some(gfx) = self.gfx.as_mut() else {
            return;
        };
        let size = gfx.window.inner_size();
        let scene = view::build(&self.flow, fonts.as_deref(), (size.width, size.height), now);
        if scene.warning_text_ready
            && let Some(w) = self.flow.warning_mut()
        {
            w.set_text_ready(now);
            w.clamp_scroll(scene.warning_scroll_max);
        }
        let key = scene.key();
        if gfx.painted != Some(key) || gfx.texture.is_none() {
            let (w, h, rgba) = view::paint(&scene, fonts.as_deref(), (size.width, size.height));
            let reuse = gfx.texture.is_some_and(|(_, tw, th)| tw == w && th == h);
            let updated = reuse
                && gfx
                    .texture
                    .is_some_and(|(id, _, _)| gfx.renderer.update_texture(id, &rgba).is_ok());
            if !updated {
                if let Some((id, _, _)) = gfx.texture.take() {
                    gfx.renderer.destroy_texture(id);
                }
                match gfx.renderer.upload_texture(&TextureUpload {
                    label: Some("official-screens"),
                    filter: Filter::Linear,
                    wrap: Wrap::Clamp,
                    ..TextureUpload::rgba8(w, h, &rgba)
                }) {
                    Ok(id) => gfx.texture = Some((id, w, h)),
                    Err(e) => tracing::warn!("official screens: upload failed: {e}"),
                }
            }
            gfx.painted = Some(key);
        }
        self.scene = scene;
        let result = match gfx.renderer.begin_frame() {
            Ok(Some(mut frame)) => {
                frame
                    .world_pass("official", &Camera::default())
                    .clear([0.01, 0.01, 0.012, 1.0]);
                if let Some((id, _, _)) = gfx.texture {
                    frame.pass("official", PassSpace::Screen).quad(
                        Quad::screen_rect(0.0, 0.0, size.width as f32, size.height as f32)
                            .with_texture(id),
                    );
                }
                gfx.renderer.end_frame(frame).map(|_| ())
            }
            Ok(None) => Ok(()),
            Err(RenderError::SurfaceLost) => {
                tracing::warn!("surface lost; recreating it");
                gfx.renderer.replace_surface(
                    Arc::clone(&gfx.window),
                    size.width.max(1),
                    size.height.max(1),
                )
            }
            Err(e) => Err(e),
        };
        if let Err(e) = result {
            self.fail(
                event_loop,
                anyhow::Error::new(e).context("rendering a frame"),
            );
        }
    }
}

impl ApplicationHandler for EntryApp {
    fn resumed(&mut self, event_loop: &ActiveEventLoop) {
        if let Some(app) = self.offline.as_mut() {
            app.resumed(event_loop);
            return;
        }
        if let Some(app) = self.official.as_mut() {
            app.resumed(event_loop);
            return;
        }
        if self.gfx.is_none()
            && let Err(e) = self.create_gfx(event_loop)
        {
            self.fail(event_loop, e);
        }
    }

    /// The client's window-event handler of a start-screen run: the only place that makes
    /// device-input values (§5.4 rule 1).
    fn window_event(&mut self, event_loop: &ActiveEventLoop, id: WindowId, event: WindowEvent) {
        if let Some(app) = self.offline.as_mut() {
            app.window_event(event_loop, id, event);
            return;
        }
        if let Some(app) = self.official.as_mut() {
            app.window_event(event_loop, id, event);
            self.official_ended(event_loop);
            return;
        }
        match &event {
            WindowEvent::ModifiersChanged(m) => {
                let s = m.state();
                self.modifiers = Modifiers {
                    shift: s.shift_key(),
                    ctrl: s.control_key(),
                    alt: s.alt_key(),
                    logo: s.super_key(),
                };
            }
            WindowEvent::Resized(size) => {
                if let Some(gfx) = self.gfx.as_mut() {
                    gfx.renderer.resize(size.width.max(1), size.height.max(1));
                    gfx.window.request_redraw();
                }
            }
            WindowEvent::RedrawRequested => self.frame(event_loop),
            _ => {}
        }
        if let Some(input) = DeviceInput::from_window_event(&event, self.modifiers, Instant::now())
        {
            let scene = &self.scene;
            self.flow.device_input(&input, &|x, y| scene.hit(x, y));
            self.handle_request(event_loop);
            if let Some(gfx) = self.gfx.as_ref() {
                gfx.window.request_redraw();
            }
        }
    }

    fn device_event(&mut self, event_loop: &ActiveEventLoop, id: DeviceId, event: DeviceEvent) {
        if let Some(app) = self.offline.as_mut() {
            app.device_event(event_loop, id, event);
        } else if let Some(app) = self.official.as_mut() {
            app.device_event(event_loop, id, event);
        }
    }

    fn about_to_wait(&mut self, event_loop: &ActiveEventLoop) {
        if let Some(app) = self.offline.as_mut() {
            app.about_to_wait(event_loop);
            return;
        }
        if let Some(app) = self.official.as_mut() {
            app.about_to_wait(event_loop);
            self.official_ended(event_loop);
            return;
        }
        event_loop.set_control_flow(ControlFlow::WaitUntil(Instant::now() + FRAME_INTERVAL));
        if let Some(gfx) = self.gfx.as_ref() {
            gfx.window.request_redraw();
        }
    }

    fn exiting(&mut self, event_loop: &ActiveEventLoop) {
        if let Some(app) = self.offline.as_mut() {
            app.exiting(event_loop);
        }
        if let Some(app) = self.official.as_mut() {
            app.exiting(event_loop);
        }
        self.release();
    }
}
