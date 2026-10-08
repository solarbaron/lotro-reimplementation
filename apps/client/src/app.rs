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
use crate::walker_host::{WalkerHost, capture, save_png};

/// Pixels of trackpad scrolling that count as one wheel notch.
const PIXELS_PER_NOTCH: f64 = 40.0;
/// How often the window title (frame rate, status) is refreshed.
const TITLE_INTERVAL: Duration = Duration::from_secs(1);

/// Run the windowed client until the window is closed.
pub fn run(args: Args) -> anyhow::Result<()> {
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

#[derive(Debug)]
struct App {
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
    fn new(args: Args) -> anyhow::Result<Self> {
        if let Some(path) = &args.frame_log {
            crate::runtime::refuse_install_path(&args, None, path)?;
        }
        let now = Instant::now();
        let runtime = Runtime::start(&args, now)?;
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
            host,
            status_panel: TextPanel::default(),
        })
    }

    fn fail(&mut self, event_loop: &ActiveEventLoop, err: anyhow::Error) {
        tracing::error!("{err:#}");
        self.error.get_or_insert(err);
        event_loop.exit();
    }

    fn create_gfx(&self, event_loop: &ActiveEventLoop) -> anyhow::Result<Gfx> {
        let (w, h) = self.args.size;
        let attrs = Window::default_attributes()
            .with_title("Tessera")
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
                crate::walker::mesh_config(crate::walker::arena_bytes(
                    &crate::walker_host::walker_config(&self.args),
                ))
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
            gfx.window
                .set_title(&format!("Tessera - {what} - {data} - {fps:.0} fps"));
            self.title_at = now;
            self.frames_since_title = 0;
        }
        gfx.window.request_redraw();
    }

    /// One frame of the world walker (or its loading screen).
    fn redraw_walker(&mut self, event_loop: &ActiveEventLoop, now: Instant, dt: f32) {
        let (Some(rt), Some(gfx), Some(host)) =
            (self.runtime.as_mut(), self.gfx.as_mut(), self.host.as_mut())
        else {
            return;
        };
        let _ = rt.poll_loader();
        rt.tick(now);
        let had_walker = host.walker.is_some();
        host.poll(rt, &mut gfx.renderer);
        let size = gfx.window.inner_size();
        let notices = rt.take_notices();
        let mut quit = false;
        let mut shot = false;
        let mut flight_done = false;
        if let Some(walker) = host.walker.as_mut() {
            if !had_walker {
                walker.set_viewport(size.width, size.height);
                self.status_panel.release(&mut gfx.renderer);
            }
            for n in notices {
                tracing::info!("world: {n}");
                walker.notice(n);
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
            shot = ev.screenshot;
            flight_done = ev.flight_done;
        } else {
            let text = match (&host.failed, &rt.status) {
                (Some(reason), _) => format!("World walker unavailable: {reason}"),
                (None, DataStatus::Loading) => "Opening the archives...".to_owned(),
                (None, _) => "Loading the world...".to_owned(),
            };
            self.status_panel
                .set(&mut gfx.renderer, vec![(text, [240, 240, 235])], 2);
        }
        let result = match gfx.renderer.begin_frame() {
            Ok(Some(mut frame)) => {
                match host.walker.as_mut() {
                    Some(walker) => walker.draw(&gfx.renderer, &mut frame),
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
                match capture(
                    &mut gfx.renderer,
                    walker,
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
            || self
                .args
                .exit_after_frames
                .is_some_and(|n| self.frames_total >= n)
        {
            event_loop.exit();
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
            gfx.window
                .set_title(&format!("Tessera walker - {place} - {fps:.0} fps"));
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

    /// Idempotent orderly shutdown.
    fn shutdown(&mut self) {
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
                tessera_input::winit_adapter::window_event(&event, |e| walker.input(&e));
            }
            match event {
                WindowEvent::CloseRequested => event_loop.exit(),
                WindowEvent::Resized(size) => {
                    if let Some(gfx) = &mut self.gfx {
                        gfx.renderer.resize(size.width, size.height);
                        gfx.window.request_redraw();
                    }
                    if let Some(w) = host.walker.as_mut() {
                        w.set_viewport(size.width, size.height);
                    }
                }
                WindowEvent::RedrawRequested => self.redraw(event_loop),
                WindowEvent::KeyboardInput { event, .. }
                    if host.walker.is_none()
                        && event.state == ElementState::Pressed
                        && event.physical_key == PhysicalKey::Code(KeyCode::Escape) =>
                {
                    event_loop.exit();
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
            tessera_input::winit_adapter::device_event(&event, |e| walker.input(&e));
        }
    }

    fn exiting(&mut self, _event_loop: &ActiveEventLoop) {
        self.shutdown();
    }
}

#[cfg(test)]
mod tests {
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
