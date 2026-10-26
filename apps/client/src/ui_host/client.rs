// SPDX-License-Identifier: GPL-3.0-or-later
//! The game UI attached to the walker, for both the windowed client and the headless runs:
//! the UI start-up thread, then the [`UiHost`] with the walker's mapper, the runtime's
//! plugin host and the frame-loop steps in the order of ARCHITECTURE §20.3.
//!
//! The caller's frame:
//!
//! 0. once, right after creating it: [`ClientUi::prepare`] (session chat lines wait for the
//!    chat window from then on);
//! 1. raw events → [`ClientUi::raw_event`] (instead of `Walker::input`); lines submitted in
//!    the chat window go to the command dispatcher with the world session right after each
//!    event (decision 0020 item 1);
//! 2. [`ClientUi::before_update`] (pending offers, the OS cursor), then `Walker::update`
//!    (steps 2–6 and the mapper's snapshot);
//! 3. [`ClientUi::after_update`] with the walker's events (Escape, positions, the session's
//!    chat lines, step 7);
//! 4. [`ClientUi::render_combat_portraits`] (the combat frames' portraits, after the window
//!    frames of `window_frames`), then [`ClientUi::step8`];
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
    /// A scenario timing run (`--scenario-timing`): the recorder's frames are labelled
    /// [`crate::timing::SETTLE_PHASE`] until the walker's timing starts and
    /// [`crate::timing::TIMED_PHASE`] from then on.
    timing: bool,
    /// Frames waited for the world to settle before the script starts.
    script_wait: u64,
    /// The drag of the current resize-bar phase: its start point.
    drag: Option<(f64, f64)>,
    /// A screen size the scripted run asked for, for the caller to apply to the render
    /// target, the walker's viewport or the window ([`Self::take_screen_request`]).
    screen_request: Option<(u32, u32)>,
    /// Whether the fallback of a failed start-up ran.
    fell_back: bool,
    /// P1 panels asked for on the command line, opened once the player is in the world.
    p1_open: Vec<P1Panel>,
    /// P4 social panels asked for on the command line (`--open-panel social|friends|
    /// fellowship|trade`), handed to the social panels once the UI is attached.
    social_open: Vec<crate::social::ui::SocialOpen>,
    /// `--character-select`: the character screens are still to open.
    want_screens: bool,
    /// The character screens before world entry, while open.
    pre_world: Option<crate::pre_world::PreWorld>,
    /// The player quit from the character screens (or their session ended).
    quit: bool,
    /// Where creation choices are kept (`character-screens.md` §7.1): the test server's
    /// connect string and account.
    server_key: Option<String>,
    /// The P1 panels' settings (comparison switch, bag columns), read once.
    p1_settings: crate::character::settings::P1Settings,
    /// The game UI's one settings writer (ARCHITECTURE §21): the settled UI scale, the
    /// Interface window's rows, the saved panel placements and the bag columns are written
    /// on its thread, never on the frame thread.
    settings: super::settings_writer::SettingsWriter,
    /// Whether user moves of saved panels are written to `[UI.panels]` (off in headless and
    /// scripted runs, whose moves are not the user's). Those runs also start without the
    /// user's saved placements, so their captures (local goldens, decision 0031) never
    /// depend on where an earlier windowed session put a panel.
    persist_placements: bool,
    /// The walker HUD and the game HUD wait for the character screens to close
    /// (`character-screens.md` §2: the screens own the display while open).
    hud: HudGate,
    /// The portraits of the player and target frames (P4, P5; `combat-ui.md` §5.1,
    /// `ui-widgets.md` §12): made when the UI attaches, over the install's records
    /// ([`crate::portrait::CombatPortraits::for_install`]); released at shutdown.
    combat_portraits:
        Option<crate::portrait::CombatPortraits<crate::walker::characters::FigureStatus>>,
}

/// Whether the HUDs (the walker's overlays and the game HUD panels) are held back while the
/// character screens are open, and when they come back (`--character-select`).
///
/// The screens are "active" from the start of a `--character-select` run until they are
/// abandoned, the player enters the world or quits. The HUDs open at attachment when no
/// screens are active, else once they stop being active.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub struct HudGate {
    /// The HUDs are held back (attached while the screens were active).
    held: bool,
    /// The HUDs have been shown.
    shown: bool,
}

impl HudGate {
    /// At attachment: whether the HUDs show now (`false` while `screens_active`).
    pub fn attach(&mut self, screens_active: bool) -> bool {
        self.held = screens_active;
        self.shown = !screens_active;
        self.shown
    }

    /// Each frame after attachment: `true` once, in the frame the held HUDs are to show
    /// (the screens are no longer active).
    pub fn release(&mut self, screens_active: bool) -> bool {
        if self.held && !screens_active {
            self.held = false;
            self.shown = true;
            return true;
        }
        false
    }

    /// Whether the HUDs are held back now.
    #[must_use]
    pub fn held(&self) -> bool {
        self.held
    }

    /// Whether the HUDs have been shown.
    #[must_use]
    pub fn shown(&self) -> bool {
        self.shown
    }
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
    // `--ui-scale` / `--ui-safe-aspect` override the settings for the run only (not saved,
    // `ui-scaling.md` §7.3).
    if let Some(s) = args.ui_scale {
        prefs.ui_scale = s;
    }
    if let Some(a) = args.ui_safe_aspect {
        prefs.safe_aspect = a;
    }
    prefs
}

/// Whether a run's user moves of saved panels are written to `[UI.panels]`: not in
/// headless, scripted, screenshot or scenario runs.
#[must_use]
pub fn persists_placements(args: &Args) -> bool {
    !(args.headless
        || args.ui_script
        || args.screenshot.is_some()
        || args.scenario_prepared.is_some())
}

impl ClientUi {
    /// The UI of a run configured by `args` (nothing starts until the archives are open).
    #[must_use]
    pub fn new(args: &Args) -> Self {
        let p1_open: Vec<P1Panel> = args
            .open_panel
            .iter()
            .filter_map(|n| parse_p1_panel(n))
            .collect();
        let social_open: Vec<crate::social::ui::SocialOpen> = args
            .open_panel
            .iter()
            .filter_map(|n| crate::social::ui::parse_open(n))
            .collect();
        let panels: Vec<PanelId> = args
            .open_panel
            .iter()
            .filter(|n| {
                parse_p1_panel(n).is_none()
                    && crate::social::ui::parse_open(n).is_none()
                    && crate::progression::parse_p6_panel(n).is_none()
            })
            .filter_map(|n| {
                let p = super::parse_panel(n);
                if p.is_none() {
                    tracing::warn!(panel = %n, "unknown panel name (use P1–P15 or a layout label)");
                }
                p
            })
            .collect();
        let persist_placements = persists_placements(args);
        let mut prefs = prefs_from(args);
        if !persist_placements {
            prefs.panels.clear();
        }
        Self {
            state: State::Idle,
            options: UiOptions {
                screen: args.size,
                prefs,
                hud: !args.no_ui_hud,
                panels,
                record: args.ui_script
                    || args.ui_report.is_some()
                    || args.scenario_timing.is_some(),
            },
            load: args.load_plugins.clone(),
            attached_walker: false,
            frame_start: None,
            script: args.ui_script.then(|| UiScript::new(args.size)),
            timing: args.scenario_timing.is_some() && !args.ui_script,
            script_wait: 0,
            drag: None,
            screen_request: None,
            fell_back: false,
            p1_open,
            social_open,
            want_screens: args.character_select,
            pre_world: None,
            quit: false,
            server_key: args
                .server
                .as_ref()
                .map(|s| format!("{s}#{}", args.account.as_deref().unwrap_or(""))),
            p1_settings: crate::character::settings::P1Settings::load_default(),
            settings: super::settings_writer::SettingsWriter::open_default(),
            persist_placements,
            hud: HudGate::default(),
            combat_portraits: None,
        }
    }

    /// The HUD gate (tests and tools).
    #[must_use]
    pub fn hud_gate(&self) -> HudGate {
        self.hud
    }

    /// The options the game UI starts with (tests and tools).
    #[must_use]
    pub fn options(&self) -> &UiOptions {
        &self.options
    }

    /// Whether the character screens are active: wanted and not yet opened, or open.
    fn screens_active(&self) -> bool {
        self.want_screens || self.pre_world.is_some()
    }

    /// Whether the client should end (the player quit on the character screens).
    #[must_use]
    pub fn quit_requested(&self) -> bool {
        self.quit
    }

    /// Whether the character screens are open.
    #[must_use]
    pub fn screens_open(&self) -> bool {
        self.pre_world.is_some()
    }

    /// Why the character screens can never open, once that is known: the facts failed to
    /// load, they loaded without the screens' data, or the UI has no creation layout.
    fn screens_unavailable(&self, rt: &Runtime) -> Option<String> {
        if let Some(e) = rt.p1.failure() {
            return Some(format!("install facts: {e}"));
        }
        if rt.p1.data().is_some() && rt.p1.screens().is_none() {
            return Some("the creation data could not be read".into());
        }
        match &self.state {
            State::Ready(host) if host.chargen().is_none() => {
                Some("the UI has no character-creation layout".into())
            }
            State::Failed(e) => Some(format!("game UI: {e}")),
            _ => None,
        }
    }

    /// `--character-select` cannot be served: the session enters on its own
    /// ([`Runtime::abandon_character_select`]).
    fn abandon_screens(&mut self, rt: &mut Runtime, reason: &str) {
        if !self.want_screens {
            return;
        }
        self.want_screens = false;
        rt.abandon_character_select(reason);
    }

    /// Opens the character screens once the UI, the world session and their data exist
    /// (`--character-select`); when one of them can never exist, the session enters the
    /// first character instead.
    fn open_screens(&mut self, rt: &mut Runtime) {
        if !self.want_screens || self.pre_world.is_some() {
            return;
        }
        if let Some(reason) = self.screens_unavailable(rt) {
            self.abandon_screens(rt, &reason);
            return;
        }
        let State::Ready(host) = &mut self.state else {
            return;
        };
        let (Some(screens), Some(data), Some(refs)) = (
            rt.p1.screens().cloned(),
            rt.p1.data().cloned(),
            host.chargen(),
        ) else {
            return;
        };
        let Some(world) = rt.world.as_mut() else {
            return;
        };
        self.want_screens = false;
        let options = crate::screens::ScreenOptions {
            server_key: self.server_key.clone(),
            settings: tessera_settings::SettingsStore::open_default().ok(),
        };
        host.collect_screen_events(true);
        let pre = {
            let mut ui = host.ui_mut();
            crate::pre_world::PreWorld::open(world, screens, data, options, Some((&mut ui, refs)))
        };
        tracing::info!("character screens open");
        self.pre_world = Some(pre);
    }

    /// The character screens' frame (after step 7): the screens pump the session, and the
    /// walker gets the viewport's figure (`character-screens.md` §2.1, §6).
    fn screens_frame(&mut self, walker: &mut Walker, rt: &mut Runtime, menu: bool) {
        let Some(pre) = self.pre_world.as_mut() else {
            return;
        };
        let State::Ready(host) = &mut self.state else {
            return;
        };
        let Some(world) = rt.world.as_mut() else {
            return;
        };
        let now = Instant::now();
        if menu {
            pre.act(crate::screens::Action::Quit, now);
        }
        let events = host.take_screen_events();
        let state = {
            let mut ui = host.ui_mut();
            pre.frame(world, Some(&mut ui), &events, now)
        };
        match state {
            crate::pre_world::PreWorldState::Open => {
                {
                    let ui = host.ui();
                    pre.hover_frame(&ui, now, &|v| walker.preview_bounds(v));
                }
                walker.set_preview(pre.figure_job());
                walker.set_preview_scene(pre.scene());
                pre.set_figure_loading(walker.preview_loading());
            }
            crate::pre_world::PreWorldState::Entered(name) => {
                tracing::info!(character = %name, "entered the world from the character screens");
                host.collect_screen_events(false);
                self.pre_world = None;
                walker.set_preview(None);
            }
            crate::pre_world::PreWorldState::Ended(reason) => {
                tracing::info!("character screens ended: {reason}");
                host.collect_screen_events(false);
                self.pre_world = None;
                self.quit = true;
                walker.set_preview(None);
            }
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
                Ok(booted) => {
                    let mut host = UiHost::from_boot(booted, &self.options);
                    host.set_persist_placements(self.persist_placements);
                    if let Some(c) = host.character_mut() {
                        c.apply_settings(&self.p1_settings);
                    }
                    State::Ready(Box::new(host))
                }
                Err(e) => {
                    tracing::error!("game UI unavailable: {e}");
                    State::Failed(e)
                }
            };
        }
        if let State::Failed(e) = &self.state {
            let reason = format!("game UI: {e}");
            self.abandon_screens(rt, &reason);
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
        // While the character screens are active the walker's HUD and minimap and the game
        // HUD stay hidden; they come with the world ([`HudGate`]).
        let show_hud = self
            .hud
            .attach(self.want_screens || self.pre_world.is_some());
        walker.set_ui_attached(
            true,
            prefs.walker_hud && show_hud,
            prefs.walker_minimap.shown(true) && show_hud,
        );
        let (mapper, _) = walker.mapper_mut();
        host.attach_mapper(mapper);
        // The plugins' local player carries the NPC windows' wallet views (P3).
        host.set_plugin_wallet(std::sync::Arc::clone(rt.npc.wallet()));
        // The plugins' party views read their members' values from the runtime's party
        // bridge (P4).
        host.set_plugin_party(rt.party_actor_source());
        // The plugins' local player carries the travel hub's mount views (P5).
        host.set_plugin_mount(rt.p5.mount().source());
        // The plugins' local player carries the P6 hub's progression views (P6,
        // `lua-plugin-api.md` §15.6).
        host.set_plugin_progression(rt.p6.plugin_source());
        host.attach_plugins(&mut rt.plugins, &self.load);
        if self.options.hud && show_hud {
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
        // The P4 social panels asked for by name.
        if let Some(s) = host.social_mut() {
            for o in std::mem::take(&mut self.social_open) {
                s.request_open(o);
            }
        }
        // The combat widgets over the opened HUD (decision 0024 P2, `combat-ui.md`), built
        // from the session's state; what was queued before is dropped, and the install's
        // data, if already here, is in them (no rebuild for its arrival).
        let _ = rt.p2.take_arrived();
        let install = rt.p2.install().cloned();
        match rt.world.as_mut() {
            Some(w) => {
                if host.attach_combat_to_world(w, install.as_ref(), Instant::now()) {
                    walker.combat_mut().set_selection(None);
                }
            }
            None => host.attach_combat(crate::combat::hud::floaty_settings(install.as_ref()), 0.0),
        }
        // The combat frames' portraits read their masks and scenes from the install (on
        // worker threads, ARCH §21).
        if self.combat_portraits.is_none()
            && let Some(set) = rt.archives.clone()
        {
            let source: std::sync::Arc<dyn tessera_props::RecordSource> = set;
            self.combat_portraits = Some(crate::portrait::CombatPortraits::for_install(source));
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
        dispatch_lines(host, rt);
        // The walker's own keys (time of day, system switches) when no UI element has the
        // keyboard focus and no modal dialog is shown (`ui-widgets.md` §18.4).
        if !host.keyboard_focused() && !host.modal_dialog_shown() {
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
            if self.timing {
                host.set_phase(match walker.timing_phase() {
                    crate::walker::TimingPhase::Running { .. } => crate::timing::TIMED_PHASE,
                    _ => crate::timing::SETTLE_PHASE,
                });
            }
            host.sync_cursor(window);
            let (mapper, _) = walker.mapper_mut();
            host.offer_pending(mapper, &mut rt.plugins, window);
            dispatch_lines(host, rt);
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
        dispatch_lines(host, rt);
    }

    /// After `Walker::update`: Escape, the position and the character (game code), the
    /// session's chat lines for the chat window, then step 7. Lines held before the UI
    /// attached are posted in the first call after attachment.
    pub fn after_update(&mut self, walker: &mut Walker, rt: &mut Runtime, events: &WalkerEvents) {
        if !self.attached_walker {
            return;
        }
        if self.pre_world.is_none()
            && rt.waits_for_character_screens()
            && rt
                .world
                .as_mut()
                .is_some_and(crate::net::World::take_returned)
        {
            // Back from the world on the same session (`Event::ReturnedToCharacterSelect`,
            // `official-messages-play.md` §7.2): the character screens open again.
            self.want_screens = true;
        }
        self.open_screens(rt);
        let screens = self.pre_world.is_some();
        // The HUDs held back for the character screens show once they closed.
        let release = self.hud.release(self.screens_active());
        let State::Ready(host) = &mut self.state else {
            return;
        };
        if release {
            let prefs = host.prefs().clone();
            walker.set_ui_attached(true, prefs.walker_hud, prefs.walker_minimap.shown(true));
            if self.options.hud {
                for (id, e) in host.open_hud(&mut rt.plugins) {
                    tracing::warn!("{id} did not open: {e}");
                }
            }
        }
        if events.menu && !screens {
            // TR5: a cancellable cast or a selection before the main menu.
            match host.combat_escape() {
                Some(a) => send_combat(host, rt, &[(crate::combat::CombatInput::Widget(a), None)]),
                None => {
                    if let Some(o) = host.escape(&mut rt.plugins, true, false) {
                        tracing::debug!(?o, "Escape");
                    }
                }
            }
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
        let shift = mapper.mods().contains(tessera_input::Modifiers::SHIFT);
        // The four UI-scaling actions pressed this frame (`ui-scaling.md` §7.3; unbound by
        // default, so a no-op in normal runs).
        host.poll_scale_actions(mapper, Instant::now());
        // The crafting-panel toggles (`crafting-panels.md` §4.1, §12: T and the three
        // profession toggles), acted on in the P6 window pass, never while the character
        // screens are open.
        if !screens {
            for t in host.poll_p6_actions(mapper) {
                rt.p6.request_toggle(t);
            }
        }
        host.step7(&mut rt.plugins, Some(mapper));
        // The character panels (decision 0024 P1): panels asked for on the command line,
        // the player-state changes of this frame, item interactions as session commands.
        character_frame(host, rt, &mut self.p1_open, shift, &mut self.settings);
        // The combat widgets (decision 0024 P2): changes, clicks, keys, anchors.
        combat_frame(host, rt, walker);
        // Chat commands that quickslot aliases ran in step 7.
        dispatch_lines(host, rt);
        // The UI-scaling requests and per-frame tick (decision 0040, `ui-scaling.md` §4).
        apply_uiscale(host, rt, Instant::now(), &mut self.settings);
        // The character screens before world entry (`--character-select`).
        self.screens_frame(walker, rt, events.menu && screens);
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

    /// The portraits of the player and target frames (P4, P5; `combat-ui.md` §5.1,
    /// `ui-widgets.md` §12), drawn with the walker's figures into their targets
    /// ([`crate::portrait::render_combat_portraits`]). Call after [`Self::after_update`]
    /// (and the frame's window updates) and **before** [`Self::step8`], so that showing or
    /// hiding a portrait reaches the same frame's composition list (`crate::portrait`
    /// module docs, "Driving it"). Does nothing before the UI is attached.
    pub fn render_combat_portraits(&mut self, walker: &mut Walker, renderer: &mut Renderer<'_>) {
        if !self.attached_walker {
            return;
        }
        if let (State::Ready(host), Some(p)) = (&self.state, self.combat_portraits.as_mut()) {
            crate::portrait::render_combat_portraits(p, host, walker, renderer);
        }
    }

    /// The combat frames' portraits, once the UI is attached (tests and tools).
    #[must_use]
    pub fn combat_portraits(
        &self,
    ) -> Option<&crate::portrait::CombatPortraits<crate::walker::characters::FigureStatus>> {
        self.combat_portraits.as_ref()
    }

    /// The portraits the UI shows (the paperdoll's figure and the character screens'
    /// figure, `render.md` §6.10), rendered with the walker into their targets. Call before
    /// the frame's `begin_frame`.
    pub fn render_portraits(&mut self, walker: &mut Walker, renderer: &mut Renderer<'_>) {
        if !self.attached_walker {
            return;
        }
        if let State::Ready(host) = &mut self.state {
            let screens = self.pre_world.as_ref().and_then(|p| p.portrait(&host.ui()));
            host.render_portraits(renderer, screens, &mut |frame, p| match p.figure {
                crate::character::panel::PortraitFigure::Player => {
                    walker.draw_portrait(frame, p.yaw)
                }
                crate::character::panel::PortraitFigure::Screens(view) => {
                    walker.draw_preview(frame, view.as_ref())
                }
            });
        }
    }

    /// The character screens while they are open (tests and headless drivers).
    #[must_use]
    pub fn pre_world(&self) -> Option<&crate::pre_world::PreWorld> {
        self.pre_world.as_ref()
    }

    /// Takes one action of the player on the open character screens (tests and headless
    /// drivers; the windowed client feeds them through the UI's events). Returns whether
    /// the screens were open.
    pub fn screens_act(&mut self, action: crate::screens::Action) -> bool {
        match self.pre_world.as_mut() {
            Some(p) => {
                p.act(action, Instant::now());
                true
            }
            None => false,
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

    /// The renderer's figures of the frame just submitted (after `Renderer::end_frame`,
    /// before [`Self::end_frame`]): the UI pass's GPU time for U2 (render.md §6.16,
    /// [`UiHost::record_render_stats`]).
    pub fn record_render_stats(&mut self, stats: &tessera_render::FrameStats) {
        if let State::Ready(host) = &mut self.state {
            host.record_render_stats(stats);
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

    /// A change of the operating system's scale factor σ for the window's monitor
    /// (`ui-scaling.md` §4.2, §4.6): with the automatic scale the UI screen is re-evaluated.
    pub fn set_os_scale(&mut self, sigma: f64) {
        if let State::Ready(host) = &mut self.state {
            host.set_os_scale(sigma);
        }
    }

    /// At exit, after the last frame (decision 0014 D8). The OS cursor is shown again
    /// when the software cursor had hidden it.
    pub fn shutdown(
        &mut self,
        rt: &mut Runtime,
        mut renderer: Option<&mut Renderer<'_>>,
        window: &mut dyn CursorWindow,
    ) {
        if let State::Ready(host) = &mut self.state {
            // The combat portraits' targets and masks go with the UI (their contents are
            // unregistered from the host's external table).
            if let (Some(p), Some(r)) = (self.combat_portraits.as_mut(), renderer.as_deref_mut()) {
                p.release(r, &host.textures().external_textures());
            }
            // The last user move of a saved panel is written before the exit.
            if let Some(map) = host.take_placement_persist() {
                self.settings
                    .submit(super::settings_writer::SettingsJob::Panels(map));
            }
            if let Some(map) = host.take_group_scale_persist() {
                self.settings
                    .submit(super::settings_writer::SettingsJob::GroupScales(map));
            }
            host.shutdown(&mut rt.plugins, renderer);
            host.release_cursor(window);
        }
        self.combat_portraits = None;
        self.settings.flush();
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

/// A P1 panel asked for on the command line (`--open-panel character`, `bag1` … `bag6`,
/// `bags`).
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum P1Panel {
    /// The character panel (`Journal_Window`).
    Character,
    /// Bag window *n* (1–6).
    Bag(usize),
    /// Every offered bag.
    Bags,
}

/// Parses a P1 panel name of `--open-panel`: `character`, `journal`, `Journal_Window`,
/// `bag1` … `bag6`, `PlayerBag1_Template` … `6`, `bags`; ASCII case ignored.
#[must_use]
pub fn parse_p1_panel(name: &str) -> Option<P1Panel> {
    let n = name.trim().to_ascii_lowercase();
    match n.as_str() {
        "character" | "journal" | "journal_window" => return Some(P1Panel::Character),
        "bags" => return Some(P1Panel::Bags),
        _ => {}
    }
    let digits = n.strip_prefix("bag").or_else(|| {
        n.strip_prefix("playerbag")
            .and_then(|r| r.strip_suffix("_template"))
    })?;
    let b: usize = digits.parse().ok()?;
    (1..=crate::character::panel::BAG_WINDOWS)
        .contains(&b)
        .then_some(P1Panel::Bag(b))
}

/// The character panels' frame (after step 7): panels still to open once the player is in
/// the world, the player-state changes, and the item commands sent to the world session.
fn character_frame(
    host: &mut UiHost,
    rt: &mut Runtime,
    pending: &mut Vec<P1Panel>,
    shift: bool,
    settings: &mut super::settings_writer::SettingsWriter,
) {
    let changes = rt.p1.take_ui_changes();
    let data = rt.p1.data().cloned();
    let Some(world) = rt.world.as_mut() else {
        return;
    };
    let now = Instant::now();
    if !pending.is_empty() && world.player().is_some() {
        for p in std::mem::take(pending) {
            let name = match p {
                P1Panel::Character => crate::character::panel::labels::TOGGLE_JOURNAL.to_owned(),
                P1Panel::Bags => crate::character::panel::labels::TOGGLE_BAGS.to_owned(),
                P1Panel::Bag(n) => {
                    format!("{}{n}", crate::character::panel::labels::TOGGLE_BAG_PREFIX)
                }
            };
            host.character_command(&name, world.player_state(), data.as_deref());
        }
    }
    let commands =
        host.character_frame(world.player_state(), data.as_deref(), &changes, shift, now);
    // The bag windows' column settings (`character-panels.md` §4.1) are written by the
    // settings writer's thread (ARCHITECTURE §21), never here.
    if let Some(c) = host.character_mut() {
        let changes = c.take_column_changes();
        if !changes.is_empty() {
            settings.submit(super::settings_writer::SettingsJob::Columns(changes));
        }
    }
    for c in commands {
        if let Err(e) = world.send_command(now, c) {
            tracing::info!("item command not sent: {e}");
        }
    }
}

/// The social panels' frame (decision 0024 P4; after step 7, beside the NPC windows and
/// the quest journal): the world session's social state drawn, the frame's UI events
/// (`events`: the one list [`UiHost::take_game_events`] returns, empty while the character
/// screens own them), the panels' actions sent to the session (`social-panels.md` §8.1).
pub(crate) fn social_frame(
    host: &mut UiHost,
    world: &mut crate::net::World,
    data: Option<&crate::character::CharacterData>,
    events: &[tessera_ui::UiEvent],
) {
    let (social, player) = world.social_and_player();
    let commands = host.social_frame(social, data, Some(player), events);
    let refused = world.send_panel_commands(Instant::now(), commands);
    host.post_session_lines(refused);
}

/// The per-frame window updates shared by the windowed client (`crate::app`) and the
/// headless UI runs (`super::run`): the Interface window, the NPC windows (decision 0024 P3),
/// the quest journal and its indicators (P3), the social panels (P4), the P5 travel,
/// instance-finder and map windows and the P6 progression and crafting windows, all reading
/// one list of in-world UI events taken once (the combat widgets keep their own queue,
/// handled in [`ClientUi::after_update`]). Call after [`ClientUi::after_update`] and before
/// [`ClientUi::step8`]; both paths call this one function.
pub(crate) fn window_frames(
    ui: &mut ClientUi,
    rt: &mut Runtime,
    walker: &mut Walker,
    renderer: &mut Renderer<'_>,
    now: Instant,
) {
    // The frame's UI events, taken once every frame: the NPC windows, the quest journal,
    // the social panels and the P5 windows read the same list; the character screens own
    // them while they are open.
    let screens = ui.screens_open();
    let taken = ui
        .host_mut()
        .map(UiHost::take_game_events)
        .unwrap_or_default();
    let events: &[tessera_ui::UiEvent] = if screens { &[] } else { &taken };
    // The Interface window (decision 0040, `ui-scaling.md` §7.2) reads the same per-frame
    // event list; it works in any mode, so it runs before the attached/world gates below.
    if let Some(host) = ui.host_mut() {
        host.interface_frame(events, now);
    }
    if !ui.attached() {
        return;
    }
    let (Some(host), Some(world)) = (ui.host_mut(), rt.world.as_mut()) else {
        return;
    };
    // The use range measures from the avatar (session frame, as `Move`).
    #[allow(clippy::cast_possible_truncation)]
    let avatar = walker
        .focus()
        .map(|p| tessera_net::Vec3::new(p.x as f32, p.y as f32, p.z as f32));
    let npc_before = npc_window_ids(&rt.npc);
    rt.npc.frame(host, world, events, screens, avatar, now);
    // An NPC window that opened this frame (a dialogue, vendor or loot window the session
    // showed) is a panel open: an event frame of decision 0015 §6 U4 (a), (b).
    if npc_window_ids(&rt.npc)
        .iter()
        .any(|id| !npc_before.contains(id))
    {
        host.mark_event();
    }
    // The quest journal (P3), after the NPC windows.
    let radar = walker.radar_input();
    rt.quests
        .frame(host, world, &rt.npc, events, radar.as_ref(), now);
    // The social panels (P4) read the same list.
    let data = rt.p1.data().cloned();
    social_frame(host, world, data.as_deref(), events);
    // Travel and the instance finder (P5), then the map window, read the same list.
    rt.p5.frame(host, world, &rt.npc, events, now);
    rt.map
        .frame(host, world, rt.p5.refs(), &rt.npc, events, radar.as_ref());
    // The P6 windows (progression and crafting) read the same list.
    rt.p6.frame(host, world, events, now);
    // The indicators over ringed NPCs (§8.2), at the walker's positions.
    let position = |id| walker.entity_render_pos(id);
    rt.quests
        .update_indicators(Some(renderer), world, &rt.npc, &position);
}

/// The ids of the NPC windows open now (P3), at most a handful.
fn npc_window_ids(npc: &crate::npc::NpcHub) -> Vec<u32> {
    npc.ui()
        .map(|u| u.open_windows().map(|(id, _, _)| id).collect())
        .unwrap_or_default()
}

/// The combat frame (decision 0024 P2, after the character panels): the combat state's
/// changes to the widgets, the world clicks the UI declined picked through the walker
/// (`combat-ui.md` TR3), the commands of keys, clicks and widgets sent to the world
/// session, the walker's selection cleared when the client cleared it locally (decision
/// 0032 D2), and the floating elements anchored above the walker's figures (§6.4).
fn combat_frame(host: &mut UiHost, rt: &mut Runtime, walker: &mut Walker) {
    use crate::combat::{CombatChange, CombatInput};
    use crate::ui_host::combat::WorldClick;
    let Some(world) = rt.world.as_mut() else {
        return;
    };
    let instant = Instant::now();
    let now = world.combat_clock(instant);
    // The install's data came after the widgets attached: rebuild them with it (CUI10,
    // TR41).
    if rt.p2.take_arrived()
        && let Some(i) = rt.p2.install().cloned()
    {
        host.combat_install_arrived(world.combat_state(), &i, now);
    }
    // Changes dropped from the session's full queue: rebuild from the state.
    if world.take_combat_overflow() {
        let install = rt.p2.install().cloned();
        host.combat_resync(world.combat_state(), install.as_deref(), now);
        if world.combat_state().selection().is_none() {
            walker.combat_mut().set_selection(None);
        }
    }
    let changes = world.take_combat_changes();
    if changes
        .iter()
        .any(|c| matches!(c, CombatChange::Selection { local: true, .. }))
    {
        walker.combat_mut().set_selection(None);
    }
    #[allow(clippy::cast_possible_truncation)]
    let px = |x: f64, y: f64| [x as f32, y as f32];
    let mut extra = Vec::new();
    // Entities to use after the selection was sent (`npc-windows.md` §3.1 rules 4 and 5).
    let mut uses = Vec::new();
    for click in host.take_world_clicks() {
        match click {
            WorldClick::Select(x, y) => {
                extra.push(CombatInput::PickEntity(walker.pick_entity(px(x, y))));
            }
            WorldClick::Use(x, y) => {
                if let Some(e) = walker.pick_entity(px(x, y)) {
                    extra.push(CombatInput::PickEntity(Some(e)));
                    uses.push(e);
                }
            }
            WorldClick::Right(x, y) => {
                let picked = walker.pick_entity(px(x, y));
                extra.push(CombatInput::PickEntity(picked));
                if let Some(e) = picked
                    && world.entities().get(&e).map(|v| v.kind)
                        != Some(tessera_net::EntityKind::Player)
                {
                    uses.push(e);
                }
            }
        }
    }
    let install = rt.p2.install().cloned();
    let anchors = |e: u64| {
        walker
            .entity_anchor(tessera_net::EntityId(e))
            .map(crate::combat::anchors::from_walker)
    };
    // TR4/TR1: the targeting keys read body records the way mouse picking does.
    let object_selectable = |e: tessera_net::EntityId| walker.entity_selectable(e);
    let commands = host.combat_frame_with(
        world.combat_state(),
        &changes,
        extra,
        install.as_deref(),
        now,
        &anchors,
        &object_selectable,
    );
    for (c, slot) in commands {
        if let Err(e) = world.send_command(instant, c) {
            tracing::info!("combat command not sent: {e}");
            if let Some(i) = slot {
                host.combat_slot_error(i, now);
            }
        }
    }
    // Using goes through the client's one entry point (use range, `npc::use_entity`).
    if !uses.is_empty() {
        #[allow(clippy::cast_possible_truncation)]
        let avatar = walker
            .focus()
            .map(|p| tessera_net::Vec3::new(p.x as f32, p.y as f32, p.z as f32));
        for e in uses {
            rt.npc.use_selection(host, world, e, avatar, instant);
        }
    }
}

/// Sends the commands of combat inputs outside the combat frame (Escape, TR5).
fn send_combat(
    host: &mut UiHost,
    rt: &mut Runtime,
    inputs: &[(crate::combat::CombatInput, Option<i32>)],
) {
    let Some(world) = rt.world.as_mut() else {
        return;
    };
    let instant = Instant::now();
    // TR5 sends only cast-cancel and selection-clear, never a targeting pick, so no object
    // body-record reading is needed here.
    for (i, slot) in inputs {
        for c in crate::combat::commands::commands(*i, world.combat_state(), &|_| true) {
            if let Err(e) = world.send_command(instant, c) {
                tracing::info!("combat command not sent: {e}");
                if let Some(n) = slot {
                    let now = world.combat_clock(instant);
                    host.combat_slot_error(*n, now);
                }
            }
        }
    }
}

/// Applies the UI-scaling work of a frame (decision 0040, `ui-scaling.md` §4, §7.3): the
/// `/uiscale` requests and the `ui_interface_window` action open the Interface window or
/// change the scale live; the per-frame tick applies the automatic default once the window
/// size settles. A settled scale change, the Interface window's optional rows and the saved
/// panel placements of the user's moves (§5.8 "Saving") go to the settings writer, which
/// writes them off the frame thread in order (§4.4: "through the settings writer off the
/// main thread").
fn apply_uiscale(
    host: &mut UiHost,
    rt: &mut Runtime,
    now: Instant,
    settings: &mut super::settings_writer::SettingsWriter,
) {
    use super::settings_writer::SettingsJob;
    for req in rt.plugins.take_uiscale_requests() {
        host.apply_uiscale_request(&req, now);
    }
    host.scale_tick(now);
    if host.take_interface_request() {
        // Tessera's own Interface window (`ui-scaling.md` §7.2): opened (or toggled closed)
        // here; its events are handled in the per-frame window pass ([`window_frames`]).
        host.open_interface();
    }
    if let Some((scale, aspect)) = host.take_scale_persist(now) {
        settings.submit(SettingsJob::UiScale { scale, aspect });
    }
    if let Some(extra) = host.take_interface_persist() {
        settings.submit(SettingsJob::UiExtra(extra));
    }
    if let Some(map) = host.take_placement_persist() {
        settings.submit(SettingsJob::Panels(map));
    }
    // `[UI.group_scale]` with the placements (`ui-scaling.md` §7.4).
    if let Some(map) = host.take_group_scale_persist() {
        settings.submit(SettingsJob::GroupScales(map));
    }
}

/// Runs the lines submitted in the chat window through the client's command dispatcher
/// with the world session (decision 0020 items 1 and 3: `--server` or the sandbox).
fn dispatch_lines(host: &mut UiHost, rt: &mut Runtime) {
    if !host.has_entry_lines() {
        return;
    }
    let session = rt
        .world
        .as_mut()
        .map(|w| w as &mut dyn crate::walker::WalkerSession);
    host.dispatch_entry_lines(&mut rt.plugins, session, Instant::now());
}

#[cfg(test)]
mod tests {
    use super::*;
    use clap::Parser as _;

    fn select_args(dir: &std::path::Path) -> Args {
        let s = |p: &std::path::Path| p.to_str().expect("utf-8").to_owned();
        Args::try_parse_from([
            "t".to_owned(),
            "--character-select".into(),
            "--game-dir".into(),
            s(&dir.join("no-install")),
            "--data-dir".into(),
            s(&dir.join("data")),
        ])
        .expect("args")
    }

    /// The HUDs wait for the character screens: held at attachment while they are active,
    /// shown once (and only once) when they close; without screens they show at once.
    #[test]
    fn huds_wait_for_the_character_screens() {
        let mut g = HudGate::default();
        assert!(!g.attach(true), "screens active: no HUD at attachment");
        assert!(g.held() && !g.shown());
        assert!(!g.release(true), "still on the screens");
        assert!(g.release(false), "the screens closed: the HUDs show");
        assert!(g.shown() && !g.held());
        assert!(!g.release(false), "only once");
        let mut g = HudGate::default();
        assert!(g.attach(false), "no screens: the HUDs show at attachment");
        assert!(!g.release(false), "nothing held");
        assert!(g.shown());
    }

    #[test]
    fn a_character_select_run_holds_the_huds() {
        let dir = tempfile::tempdir().expect("tempdir");
        let args = select_args(dir.path());
        let ui = ClientUi::new(&args);
        assert!(ui.screens_active(), "--character-select: screens wanted");
        let args = Args::try_parse_from(["t", "--ui"]).expect("args");
        let ui = ClientUi::new(&args);
        assert!(!ui.screens_active());
        assert!(!ui.hud_gate().held());
    }

    /// Headless and scripted runs neither write nor read the user's `[UI.panels]`.
    #[test]
    fn scripted_runs_start_without_the_users_saved_placements() {
        let args =
            Args::try_parse_from(["t", "--ui", "--headless", "--duration", "1"]).expect("args");
        assert!(!persists_placements(&args));
        let ui = ClientUi::new(&args);
        assert!(ui.options().prefs.panels.is_empty());
        assert!(!ui.persist_placements);
        let args = Args::try_parse_from(["t", "--ui"]).expect("args");
        assert!(persists_placements(&args));
    }

    #[test]
    fn character_select_without_its_data_enters_on_its_own() {
        let dir = tempfile::tempdir().expect("tempdir");
        let args = select_args(dir.path());
        let mut rt = Runtime::start(&ClientUi::runtime_args(&args), Instant::now()).expect("rt");
        assert!(rt.waits_for_character_screens());
        let mut ui = ClientUi::new(&args);
        // The facts failed to load: the screens can never open.
        rt.p1.set_failed("strings: unreadable");
        ui.open_screens(&mut rt);
        assert!(!rt.waits_for_character_screens());
        assert!(!ui.want_screens);
        let notices = rt.take_notices();
        assert!(
            notices
                .iter()
                .any(|n| n.contains("character screens are unavailable")),
            "{notices:?}"
        );
    }

    #[test]
    fn character_select_without_a_ui_enters_on_its_own() {
        let dir = tempfile::tempdir().expect("tempdir");
        let args = select_args(dir.path());
        let mut rt = Runtime::start(&ClientUi::runtime_args(&args), Instant::now()).expect("rt");
        let mut ui = ClientUi::new(&args);
        let deadline = Instant::now() + std::time::Duration::from_secs(30);
        while matches!(rt.status, DataStatus::Loading) && Instant::now() < deadline {
            let _ = rt.wait_loader(std::time::Duration::from_millis(50));
        }
        ui.poll(&mut rt, None);
        assert!(ui.failed().is_some());
        assert!(!rt.waits_for_character_screens());
    }

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
