// SPDX-License-Identifier: GPL-3.0-or-later
//! The game UI attached to the walker, for both the windowed client and the headless runs:
//! the UI start-up thread, then the [`UiHost`] with the walker's mapper, the runtime's
//! plugin host and the frame-loop steps in the order of ARCHITECTURE §20.3.
//!
//! The caller's frame:
//!
//! 0. once, right after creating it: [`ClientUi::prepare`] (session chat lines wait for the
//!    chat window from then on);
//! 1. raw events → [`ClientUi::raw_event`] (instead of `Walker::input`);
//! 2. [`ClientUi::before_update`] (pending offers, the OS cursor), then `Walker::update`
//!    (steps 2–6 and the mapper's snapshot);
//! 3. [`ClientUi::after_update`] with the walker's events (Escape, positions, the session's
//!    chat lines, step 7);
//! 4. [`ClientUi::step8`];
//! 5. `Walker::draw`, then [`ClientUi::draw`] (the UI pass after the walker's overlays);
//! 6. present, then [`ClientUi::end_frame`];
//! 7. at exit, after the last frame, [`ClientUi::shutdown`].
//!
//! If the UI cannot start, the run goes on without it: the plugins named on the command
//! line are queued on the plugin host as without a UI, and held chat lines go to the
//! plugins ([`ClientUi::poll`]).

use std::path::PathBuf;
use std::time::Instant;

use tessera_input::RawEvent;
use tessera_render::{Frame, Renderer};
use tessera_settings::UiPrefs;

use super::boot::UiBoot;
use super::cursor::CursorWindow;
use super::script::{Action, UiScript};
use super::{UiHost, UiOptions};
use crate::cli::Args;
use crate::panels::PanelId;
use crate::runtime::{DataStatus, Runtime};
use crate::walker::{Walker, WalkerEvents};

/// The UI's start state.
#[derive(Debug)]
enum State {
    /// Waiting for the archives.
    Idle,
    /// The start-up thread runs.
    Booting(UiBoot),
    /// Attached.
    Ready(Box<UiHost>),
    /// Start-up failed; the walker runs without a UI.
    Failed(String),
}

/// The walker's game UI (module docs).
#[derive(Debug)]
pub struct ClientUi {
    state: State,
    options: UiOptions,
    load: Vec<String>,
    attached_walker: bool,
    frame_start: Option<Instant>,
    /// The scripted run of decision 0015 §6 (`--ui-script`).
    script: Option<UiScript>,
    /// Frames waited for the world to settle before the script starts.
    script_wait: u64,
    /// The drag of the current resize-bar phase: its start point.
    drag: Option<(f64, f64)>,
    /// A screen size the scripted run asked for, for the caller to apply to the render
    /// target, the walker's viewport or the window ([`Self::take_screen_request`]).
    screen_request: Option<(u32, u32)>,
    /// Whether the fallback of a failed start-up ran.
    fell_back: bool,
}

/// Frames the script waits for the walker to settle before it starts anyway (60 s).
pub const SCRIPT_SETTLE_FRAMES: u64 = 3600;

/// The game-UI preferences of a run: the settings file's `[UI]` section with the command
/// line's overrides.
#[must_use]
pub fn prefs_from(args: &Args) -> UiPrefs {
    let mut prefs = match tessera_settings::SettingsStore::open_default() {
        Ok(store) => {
            let loaded = store.load();
            let (p, warnings) = UiPrefs::from_settings(&loaded.settings);
            for w in warnings {
                tracing::warn!("settings: {w}");
            }
            p
        }
        Err(e) => {
            tracing::debug!("settings unavailable ({e}); UI defaults");
            UiPrefs::default()
        }
    };
    if let Some(s) = &args.skin {
        prefs.user_skin.clone_from(s);
    }
    if let Some(r) = &args.skins_root {
        prefs.skins_root = Some(r.clone());
    }
    if let Some(h) = args.walker_hud {
        prefs.walker_hud = h;
    }
    if let Some(m) = args.walker_minimap {
        prefs.walker_minimap = m;
    }
    if let Some(c) = args.ui_cursor {
        prefs.cursor = c;
    }
    prefs
}

impl ClientUi {
    /// The UI of a run configured by `args` (nothing starts until the archives are open).
    #[must_use]
    pub fn new(args: &Args) -> Self {
        let panels: Vec<PanelId> = args
            .open_panel
            .iter()
            .filter_map(|n| {
                let p = super::parse_panel(n);
                if p.is_none() {
                    tracing::warn!(panel = %n, "unknown panel name (use P1–P15 or a layout label)");
                }
                p
            })
            .collect();
        Self {
            state: State::Idle,
            options: UiOptions {
                screen: args.size,
                prefs: prefs_from(args),
                hud: !args.no_ui_hud,
                panels,
                record: args.ui_script || args.ui_report.is_some(),
            },
            load: args.load_plugins.clone(),
            attached_walker: false,
            frame_start: None,
            script: args.ui_script.then(|| UiScript::new(args.size)),
            script_wait: 0,
            drag: None,
            screen_request: None,
            fell_back: false,
        }
    }

    /// Right after creating the UI: session chat lines wait for the chat window from now on
    /// (`chat-window.md` §10 item 9: each reaches the plugins once, through the window).
    pub fn prepare(&mut self, rt: &mut Runtime) {
        rt.hold_chat_for_ui(true);
    }

    /// The UI could not start: the run goes on without it, so the plugins named on the
    /// command line are queued on the plugin host as without a UI (the runtime's
    /// per-frame pump loads them) and held chat lines go to the plugins. Runs once.
    fn fall_back(&mut self, rt: &mut Runtime) {
        if self.fell_back {
            return;
        }
        self.fell_back = true;
        rt.hold_chat_for_ui(false);
        if self.load.is_empty() {
            return;
        }
        tracing::warn!(
            plugins = self.load.len(),
            "game UI unavailable; plugins load without it"
        );
        for name in &self.load {
            if rt.plugins.manager_mut().queue_load(name) {
                tracing::info!(plugin = %name, "plugin load queued");
            } else {
                tracing::warn!(plugin = %name, "plugin not loaded: no available plugin of that name");
            }
        }
    }

    /// A screen size the scripted run asked for since the last call (its screen phase):
    /// the caller resizes the render target and the walker's viewport, or asks the window
    /// for it.
    pub fn take_screen_request(&mut self) -> Option<(u32, u32)> {
        self.screen_request.take()
    }

    /// Whether the scripted run finished (always `false` without one).
    #[must_use]
    pub fn script_done(&self) -> bool {
        self.script.as_ref().is_some_and(UiScript::done)
    }

    /// Whether the scripted run has started its phases.
    #[must_use]
    pub fn script_running(&self) -> bool {
        self.script.is_some() && self.script_wait == u64::MAX
    }

    /// The host, once attached.
    #[must_use]
    pub fn host(&self) -> Option<&UiHost> {
        match &self.state {
            State::Ready(h) => Some(h),
            _ => None,
        }
    }

    /// The host, mutably.
    pub fn host_mut(&mut self) -> Option<&mut UiHost> {
        match &mut self.state {
            State::Ready(h) => Some(h),
            _ => None,
        }
    }

    /// Why the UI is not available, if it failed.
    #[must_use]
    pub fn failed(&self) -> Option<&str> {
        match &self.state {
            State::Failed(e) => Some(e),
            _ => None,
        }
    }

    /// Whether the UI is attached to the walker.
    #[must_use]
    pub fn attached(&self) -> bool {
        self.attached_walker
    }

    /// Starts the UI start-up once the archives are open and attaches the UI when both it
    /// and the walker exist. Never blocks.
    pub fn poll(&mut self, rt: &mut Runtime, walker: Option<&mut Walker>) {
        if matches!(self.state, State::Idle) {
            match &rt.status {
                DataStatus::Ready { root, .. } => {
                    let Some(set) = rt.archives.clone() else {
                        return;
                    };
                    match UiBoot::spawn(set, root.clone(), self.options.prefs.skins_root.clone()) {
                        Ok(b) => self.state = State::Booting(b),
                        Err(e) => self.state = State::Failed(format!("UI start-up thread: {e}")),
                    }
                }
                DataStatus::NoData(reason) => {
                    self.state = State::Failed(format!("no game data: {reason}"));
                }
                DataStatus::Loading => {}
            }
        }
        if let State::Booting(b) = &mut self.state
            && let Some(r) = b.try_take()
        {
            self.state = match r {
                Ok(booted) => State::Ready(Box::new(UiHost::from_boot(booted, &self.options))),
                Err(e) => {
                    tracing::error!("game UI unavailable: {e}");
                    State::Failed(e)
                }
            };
        }
        if matches!(self.state, State::Failed(_)) {
            self.fall_back(rt);
            return;
        }
        if self.attached_walker {
            return;
        }
        let (State::Ready(host), Some(walker)) = (&mut self.state, walker) else {
            return;
        };
        let prefs = host.prefs().clone();
        walker.set_ui_attached(true, prefs.walker_hud, prefs.walker_minimap.shown(true));
        let (mapper, _) = walker.mapper_mut();
        host.attach_mapper(mapper);
        host.attach_plugins(&mut rt.plugins, &self.load);
        if self.options.hud {
            for (id, e) in host.open_hud(&mut rt.plugins) {
                tracing::warn!("{id} did not open: {e}");
            }
        }
        for id in self.options.panels.clone() {
            if let Err(e) = host.open_panel(&mut rt.plugins, id) {
                tracing::warn!("{id} did not open: {e}");
            }
            if id == PanelId::TargetVitals {
                // Asked for by name: select a target so that P5 shows (§17.4).
                host.set_target(&mut rt.plugins, true);
            }
        }
        // `[UI] ui_scale_global` reaches every native panel through the panels' scalers as
        // it opens (UiHost::native_scale; `tessera-ui` has no `UIScale_Global` setter).
        self.attached_walker = true;
        if let Some(p) = host.panels() {
            let ui = host.ui();
            for (id, k) in p.opened() {
                if let Some(b) = ui.layout_box(k) {
                    tracing::info!(
                        panel = %id,
                        x = b.l,
                        y = b.t,
                        w = b.w(),
                        h = b.h(),
                        shown = p.is_shown(&ui, id),
                        "panel root rectangle (ui-layout.md §8.4)"
                    );
                }
            }
        }
        tracing::info!(
            panels = host.panels().map_or(0, |p| p.opened().count()),
            "game UI attached"
        );
    }

    /// Step 1: a raw event. Returns `false` when the UI is not attached (the caller feeds
    /// the walker as before).
    pub fn raw_event(
        &mut self,
        walker: &mut Walker,
        rt: &mut Runtime,
        event: &RawEvent,
        window: &mut dyn CursorWindow,
    ) -> bool {
        if !self.attached_walker {
            return false;
        }
        let State::Ready(host) = &mut self.state else {
            return false;
        };
        let (mapper, now) = walker.mapper_mut();
        host.handle_raw(mapper, event, now, &mut rt.plugins, window);
        // The walker's own keys (time of day, system switches) when no UI element has the
        // keyboard focus.
        if !host.keyboard_focused() {
            walker.world_key(event);
        }
        true
    }

    /// Step 1 after the last raw event: the pending offers (before `Walker::update`).
    pub fn before_update(
        &mut self,
        walker: &mut Walker,
        rt: &mut Runtime,
        window: &mut dyn CursorWindow,
    ) {
        self.frame_start = Some(Instant::now());
        if !self.attached_walker {
            return;
        }
        self.run_script(walker, rt, window);
        if let State::Ready(host) = &mut self.state {
            host.sync_cursor(window);
            let (mapper, _) = walker.mapper_mut();
            host.offer_pending(mapper, &mut rt.plugins, window);
        }
    }

    /// One frame of the scripted run: its actions as raw events (step 1) and host actions.
    fn run_script(&mut self, walker: &mut Walker, rt: &mut Runtime, window: &mut dyn CursorWindow) {
        let State::Ready(host) = &mut self.state else {
            return;
        };
        let Some(script) = self.script.as_mut() else {
            return;
        };
        if self.script_wait != u64::MAX {
            let budget_loaded = rt
                .plugins
                .manager()
                .loaded_plugins()
                .iter()
                .any(|p| p.name == "Budget");
            if (walker.settled() && budget_loaded) || self.script_wait >= SCRIPT_SETTLE_FRAMES {
                // The phases work from the UI's current screen (the window size the
                // compositor gave in a windowed run).
                script.set_screen(self.options.screen);
                tracing::info!(
                    waited_frames = self.script_wait,
                    settled = walker.settled(),
                    budget_loaded,
                    screen = ?self.options.screen,
                    "UI script starts"
                );
                self.script_wait = u64::MAX;
            } else {
                self.script_wait += 1;
                host.set_phase("startup");
                return;
            }
        }
        host.set_phase(script.phase());
        for a in script.tick() {
            match a {
                Action::Raw(ev) => {
                    let (mapper, now) = walker.mapper_mut();
                    host.handle_raw(mapper, &ev, now, &mut rt.plugins, window);
                }
                Action::Open(id) => {
                    if let Err(e) = host.open_panel(&mut rt.plugins, id) {
                        tracing::debug!("script: {id}: {e}");
                    }
                }
                Action::Hide(id) => host.hide_panel(&mut rt.plugins, id),
                Action::Command(line) => {
                    let _ = rt.plugins.command(&line);
                }
                Action::Chat(text) => host.post_notices([text]),
                Action::Scale(s) => host.scale_native_panels(s),
                Action::Resize(w, h) => {
                    host.resize(w, h);
                    self.options.screen = (w, h);
                    self.screen_request = Some((w, h));
                }
                Action::Event => host.mark_event(),
                Action::Drag(id, step) => {
                    let (mapper, now) = walker.mapper_mut();
                    match (step, self.drag) {
                        (Some(0), _) | (Some(_), None) => {
                            let root = host
                                .panels()
                                .and_then(|p| p.root(id))
                                .map(|k| host.ui().displayed_box(k));
                            if let Some(b) = root {
                                let start = (f64::from(b.r - 2), f64::from(b.b - 2));
                                self.drag = Some(start);
                                for ev in [
                                    RawEvent::CursorMoved {
                                        x: start.0,
                                        y: start.1,
                                    },
                                    RawEvent::MouseButton {
                                        button: tessera_input::MouseButton::Left,
                                        pressed: true,
                                    },
                                ] {
                                    host.handle_raw(mapper, &ev, now, &mut rt.plugins, window);
                                }
                            }
                        }
                        (Some(d), Some(start)) => {
                            let d = f64::from(d);
                            let ev = RawEvent::CursorMoved {
                                x: start.0 + d,
                                y: start.1 + d * 0.5,
                            };
                            host.handle_raw(mapper, &ev, now, &mut rt.plugins, window);
                        }
                        (None, _) => {
                            self.drag = None;
                            let ev = RawEvent::MouseButton {
                                button: tessera_input::MouseButton::Left,
                                pressed: false,
                            };
                            host.handle_raw(mapper, &ev, now, &mut rt.plugins, window);
                        }
                    }
                }
            }
        }
    }

    /// After `Walker::update`: Escape, the position and the character (game code), the
    /// session's chat lines for the chat window, then step 7. Lines held before the UI
    /// attached are posted in the first call after attachment.
    pub fn after_update(&mut self, walker: &mut Walker, rt: &mut Runtime, events: &WalkerEvents) {
        if !self.attached_walker {
            return;
        }
        let State::Ready(host) = &mut self.state else {
            return;
        };
        if events.menu
            && let Some(o) = host.escape(&mut rt.plugins, true, false)
        {
            tracing::debug!(?o, "Escape");
        }
        if let Some(input) = walker.radar_input() {
            host.update_position(input, None);
        }
        let name = rt.world.as_ref().and_then(|w| {
            w.player()
                .and_then(|id| w.entities().get(&id))
                .map(|e| e.name.clone())
        });
        host.set_character(&mut rt.plugins, name.as_deref());
        host.post_session_lines(rt.take_ui_chat());
        let (mapper, _) = walker.mapper_mut();
        host.step7(&mut rt.plugins, Some(mapper));
    }

    /// Step 8.
    pub fn step8(&mut self, walker: &mut Walker, renderer: &mut Renderer<'_>, rt: &mut Runtime) {
        if !self.attached_walker {
            return;
        }
        if let State::Ready(host) = &mut self.state {
            host.step8(Some(renderer), &mut rt.plugins);
            let (mapper, _) = walker.mapper_mut();
            host.apply_band_changes(mapper);
        }
    }

    /// Step 10: the UI pass (call after `Walker::draw`). `cursor` is false for screenshots.
    pub fn draw(&mut self, frame: &mut Frame, cursor: bool) {
        if !self.attached_walker {
            return;
        }
        if let State::Ready(host) = &mut self.state {
            host.draw(frame, cursor);
        }
    }

    /// After present: the frame clock.
    pub fn end_frame(&mut self, rt: &Runtime) {
        let frame_ms = self
            .frame_start
            .take()
            .map_or(0.0, |t| t.elapsed().as_secs_f64() * 1e3);
        if let State::Ready(host) = &mut self.state
            && self.attached_walker
        {
            host.end_frame(&rt.plugins, frame_ms);
        }
    }

    /// A window scale-factor change (`keymap.md` §7.7, §7.8.9): see [`UiHost::rescale`].
    /// Returns `false` when the UI is not attached (the caller rescales the mapper itself).
    pub fn rescale(
        &mut self,
        walker: &mut Walker,
        timing: tessera_input::ActivationTiming,
        ratio: f64,
        area: (u32, u32),
    ) -> bool {
        if !self.attached_walker {
            return false;
        }
        let State::Ready(host) = &mut self.state else {
            return false;
        };
        let (mapper, _) = walker.mapper_mut();
        host.rescale(mapper, timing, ratio, area);
        true
    }

    /// A new window size.
    pub fn resize(&mut self, width: u32, height: u32) {
        self.options.screen = (width, height);
        if let State::Ready(host) = &mut self.state {
            host.resize(width, height);
        }
    }

    /// At exit, after the last frame (decision 0014 D8). The OS cursor is shown again
    /// when the software cursor had hidden it.
    pub fn shutdown(
        &mut self,
        rt: &mut Runtime,
        renderer: Option<&mut Renderer<'_>>,
        window: &mut dyn CursorWindow,
    ) {
        if let State::Ready(host) = &mut self.state {
            host.shutdown(&mut rt.plugins, renderer);
            host.release_cursor(window);
        }
        rt.hold_chat_for_ui(false);
        self.fell_back = true;
        self.attached_walker = false;
        self.state = State::Failed("shut down".into());
    }

    /// The runtime's arguments for a run with the game UI: plugin loads wait for the UI
    /// (ARCHITECTURE §20.1 step 9), and the Plugins folder of `[UI] plugins_dir` is used
    /// when `--plugins` is not given.
    #[must_use]
    pub fn runtime_args(args: &Args) -> Args {
        let mut a = args.clone();
        if args.ui_enabled() {
            a.load_plugins.clear();
            if a.plugins.is_none() {
                a.plugins = prefs_from(args).plugins_dir;
            }
        }
        a
    }

    /// The U1–U5 report of the recorded frames (scripted runs and `--ui-report`).
    #[must_use]
    pub fn report(&self, title: &str) -> Option<String> {
        let rec = self.host()?.recorder()?;
        Some(super::stats::report_markdown(rec, title, &[]))
    }

    /// The Plugins folder the run uses (for logs).
    #[must_use]
    pub fn plugins_dir(rt: &Runtime) -> PathBuf {
        rt.plugins.manager().plugins_dir().to_path_buf()
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use clap::Parser as _;

    #[test]
    fn a_failed_start_up_loads_the_plugins_without_the_ui() {
        let dir = tempfile::tempdir().expect("tempdir");
        let plugins = dir.path().join("Plugins");
        let pkg = plugins.join("Test").join("Hello");
        std::fs::create_dir_all(&pkg).expect("mkdir");
        std::fs::write(
            plugins.join("Test").join("Hello.plugin"),
            "<?xml version=\"1.0\"?><Plugin><Information><Name>Hello</Name>\
             <Author>Test</Author><Version>1.0</Version></Information>\
             <Package>Test.Hello.Main</Package></Plugin>",
        )
        .expect("write");
        std::fs::write(
            pkg.join("Main.lua"),
            "Turbine.Shell.WriteLine('hello: loaded')",
        )
        .expect("write");
        let s = |p: &std::path::Path| p.to_str().expect("utf-8").to_owned();
        let args = Args::try_parse_from([
            "t".to_owned(),
            "--ui".into(),
            "--game-dir".into(),
            s(&dir.path().join("no-install")),
            "--data-dir".into(),
            s(&dir.path().join("data")),
            "--plugins".into(),
            s(&plugins),
            "--load-plugin".into(),
            "Hello".into(),
        ])
        .expect("args");
        assert!(args.ui_enabled());
        let mut rt = Runtime::start(&ClientUi::runtime_args(&args), Instant::now()).expect("rt");
        let mut ui = ClientUi::new(&args);
        ui.prepare(&mut rt);
        assert!(rt.holds_chat_for_ui());
        let deadline = Instant::now() + std::time::Duration::from_secs(30);
        while matches!(rt.status, DataStatus::Loading) && Instant::now() < deadline {
            let _ = rt.wait_loader(std::time::Duration::from_millis(50));
        }
        assert!(
            matches!(rt.status, DataStatus::NoData(_)),
            "{:?}",
            rt.status
        );
        ui.poll(&mut rt, None);
        assert!(ui.failed().is_some());
        assert!(
            !rt.holds_chat_for_ui(),
            "chat goes to the plugins directly again"
        );
        // The plugin loads through the runtime's pump, as without a UI.
        while rt.plugins.manager().loaded_plugins().is_empty() && Instant::now() < deadline {
            rt.tick(Instant::now());
            std::thread::sleep(std::time::Duration::from_millis(2));
        }
        assert_eq!(rt.plugins.manager().loaded_plugins().len(), 1);
        assert!(
            rt.plugins
                .host()
                .chat_lines()
                .iter()
                .any(|l| l == "hello: loaded"),
            "{:?}",
            rt.plugins.host().chat_lines()
        );
        // Polling again does not queue the load twice.
        ui.poll(&mut rt, None);
        rt.shutdown();
    }
}
