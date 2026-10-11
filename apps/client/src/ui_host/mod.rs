// SPDX-License-Identifier: GPL-3.0-or-later
//! The game UI in the client (ROADMAP M5): `tessera-ui`, the native panels, the plugin
//! host and the input bridge wired into the frame loop of ARCHITECTURE §20.3 as decisions
//! 0014 and 0015 amend it.
//!
//! | Step | What the host does | Entry point |
//! |---|---|---|
//! | 1 | the mapper resolves raw events and offers them to the UI through this host's [`UiSink`]: chat keys first, then [`Ui::offer`]; plugin input handlers run inside ([`PluginUi::deliver`]); band changes go back as the offer's requests; the pointer lock follows `mouselook` (0015 §2.5); the lines submitted in the chat window's entry go to the client's command dispatcher (decision 0020 item 1) | [`UiHost::handle_raw`], [`UiHost::offer_pending`], [`UiHost::dispatch_entry_lines`] |
//! | 7 | worker arrivals, the UI tick ([`Ui::tick_hover`]), `MouseHover`, the `Update` broadcast, the UI's frame work, chat lines, game-driven changes (position, character), the plugin host's deferred work and collector step, then the band changes to the mapper | [`UiHost::step7`] |
//! | 8 | layout and paint within the paint slice, the GPU operations of the frame, the composition list | [`UiHost::step8`] |
//! | 10 | the UI pass after the walker's overlays, then the software cursor | [`UiHost::draw`] |
//! | after present | the frame clock (plugins' game time, the UI's hover clock) | [`UiHost::end_frame`] |
//! | exit | every apartment unloaded after the last frame | [`UiHost::shutdown`] |
//!
//! Escape with a UI attached opens the game menu (`walker.md` §5): the walker reports a
//! `menu` press that the UI declined and the host runs [`UiHost::escape`].
//!
//! Modules: [`boot`] (UI data off the main thread), [`env`](mod@env) (the [`tessera_ui::UiEnv`]
//! over the asset workers), [`text`] (shaping), [`gpu`] (UI textures and the UI pass),
//! [`cursor`] (pointer lock), [`services`] (plugin adapters for the panels), [`stats`]
//! (U1–U5), [`script`] (the scripted run of 0015 §6).

pub mod boot;
pub mod client;
pub mod combat;
pub mod cursor;
pub mod env;
pub mod gpu;
pub mod run;
pub mod script;
pub mod services;
pub mod stats;
pub mod text;

use std::cell::{Ref, RefCell, RefMut};
use std::collections::{BTreeSet, VecDeque};
use std::rc::Rc;
use std::sync::Arc;
use std::time::{Duration, Instant};

use tessera_input::{
    BandRequests, ContextId, InputMapper, Modifiers as InputModifiers, OfferPhase, OfferedEvent,
    RawEvent, UiSink, Verdict, contexts,
};
use tessera_plugins::ui::{Modifiers, PluginUi};
use tessera_plugins::{GameplayEvent, LocalPlayerEvent};
use tessera_render::{Frame, Renderer, TextureId};
use tessera_settings::{CursorMode, UiPrefs};
use tessera_ui::BandChange;
use tessera_ui::bridge::{CommandOffer, Phase, UiChange, UiInput};
use tessera_ui::events::Command;
use tessera_ui::game::chat::{CommandOutcome, EntryLine, TextInputOutcome};
use tessera_ui::game::escape::EscapeOutcome;
use tessera_ui::game::radar::RadarInput;
use tessera_ui::game::{MapSource, NoShortcuts};
use tessera_ui::paint::{CapCounters, CompositionList, PaintLimits, TimeSlice};
use tessera_ui::skin::SkinSelection;
use tessera_ui::{ElementKey, Ui, UiEnv, UiEvent, Vocab};

use crate::panels::{NativePanels, NoMaps, PanelId, PanelsError, Services, WorldMaps};
use crate::plugins::Plugins;
use cursor::{CursorWindow, PointerLock, PointerReport, StartOutcome};
use env::{ClientEnv, EnvHandle, EnvState};
use gpu::UiTextures;
use services::{Deferred, PluginView, WalkerPlayer};
use stats::{FrameSample, Recorder, ms};

/// Most rounds of "deliver plugin events, handle client events" per call (a guard against
/// handlers that keep raising events; the rest waits for the next delivery).
pub const MAX_DELIVERY_ROUNDS: usize = 8;
/// UI asset workers (image, skin-file and font-atlas decodes).
pub const UI_WORKERS: usize = 2;
/// Most chat-window lines waiting for the command dispatcher (lines beyond it are dropped
/// with a log line; the dispatcher runs them within the same frame-loop step).
pub const MAX_ENTRY_LINES: usize = 64;
/// Longest event window in frames (U4 (e) of decision 0015 §6: a screen-sized window's
/// first job within 40 frames, plus the trigger frame).
pub const MAX_EVENT_WINDOW_FRAMES: u32 = 41;

/// Construction options of the host.
#[derive(Debug, Clone)]
pub struct UiOptions {
    /// Screen size in physical pixels.
    pub screen: (u32, u32),
    /// Preferences (`[UI]` of the settings file, with command-line overrides applied).
    pub prefs: UiPrefs,
    /// Open the HUD set (P1–P5, P11, P14) at start.
    pub hud: bool,
    /// Further panels to open at start, by name.
    pub panels: Vec<PanelId>,
    /// Record per-frame samples for U1–U5.
    pub record: bool,
}

impl Default for UiOptions {
    fn default() -> Self {
        Self {
            screen: (1280, 720),
            prefs: UiPrefs::default(),
            hud: true,
            panels: Vec::new(),
            record: false,
        }
    }
}

/// Context IDs and actions the bridge needs from the mapper's registry.
#[derive(Debug, Clone, Default)]
struct BridgeIds {
    contexts: Vec<(&'static str, ContextId)>,
    mouselook: Option<tessera_input::ActionId>,
    menu: Option<tessera_input::ActionId>,
}

impl BridgeIds {
    fn context(&self, map: &str) -> Option<ContextId> {
        let name = tessera_input::commands::context_for_input_map(map)?;
        self.contexts
            .iter()
            .find(|(n, _)| *n == name)
            .map(|(_, c)| *c)
    }
}

/// Per-frame accumulators.
#[derive(Debug, Clone, Copy, Default)]
struct FrameAccum {
    host: Duration,
    step1: Duration,
    step7: Duration,
    paint: Duration,
    pointer_events: u32,
    event: bool,
    content_event: bool,
}

/// The game UI host (module docs).
pub struct UiHost {
    ui: Rc<RefCell<Ui>>,
    plugin_ui: PluginUi,
    env: Option<EnvHandle>,
    panels: Option<NativePanels>,
    maps: Option<WorldMaps>,
    skins: Option<SkinSelection>,
    textures: UiTextures,
    composition: CompositionList,
    caps: CapCounters,
    lock: PointerLock,
    ids: BridgeIds,
    prefs: UiPrefs,
    limits: PaintLimits,
    deferred: RefCell<Deferred>,
    chat_holds: BTreeSet<u64>,
    player: Arc<WalkerPlayer>,
    /// The wallet views the plugins' local player carries (P3, [`Self::set_plugin_wallet`]).
    plugin_wallet: Option<Arc<crate::npc::SessionWallet>>,
    pending_position: Option<(RadarInput, Option<u32>)>,
    pending_notices: Vec<String>,
    pending_session: Vec<crate::net::ChatLine>,
    /// Lines submitted in the chat window (and run by quickslot aliases) waiting for the
    /// command dispatcher ([`Self::dispatch_entry_lines`]).
    entry_lines: VecDeque<EntryLine>,
    /// The generation of the dispatcher's command words the chat window last took
    /// ([`Plugins::command_text_generation`]; `chat-window.md` §10 item 11).
    command_text_generation: u64,
    /// The scale the native panels take (`[UI] ui_scale_global`, then the scripted run's
    /// scale changes), and the scale each open root was given.
    native_scale: f32,
    scaled_roots: Vec<(ElementKey, f32)>,
    /// What the OS cursor was last set to by [`Self::sync_cursor`] (`None` = never set).
    os_cursor_visible: Option<bool>,
    /// Frames left in the current event window (U4; [`stats`] module docs).
    event_window: u32,
    started: Instant,
    clock: f64,
    frame: FrameAccum,
    last: FrameSample,
    recorder: Option<Recorder>,
    phase: &'static str,
    cursor_tex: Option<TextureId>,
    frames: u64,
    command_ids: Vec<(String, u32)>,
    /// The character panel and bag windows (decision 0024 P1), with the install.
    character: Option<crate::character::panel::CharacterUi>,
    /// UI events for [`Self::character`], handled in [`Self::character_frame`] (they need
    /// the player's state).
    character_events: Vec<UiEvent>,
    /// Panel input commands (`ToggleJournalPanel`, `QUICKSLOT_BACKPACK…`,
    /// `TOOLTIP_DETACH`) taken in step 1, run in [`Self::character_frame`].
    character_commands: Vec<String>,
    /// The character screens' layouts (`--character-select`).
    chargen: Option<crate::screens::ui::ChargenRefs>,
    /// While the character screens are open: every UI event, for them
    /// ([`Self::take_screen_events`]).
    screen_events: Option<Vec<UiEvent>>,
    /// Every UI event since the last [`Self::take_game_events`] (at most
    /// [`MAX_CHARACTER_QUEUE`]; the windowed client takes them every frame), for the in-world windows the runtime drives after step 7: the NPC windows and the
    /// quest journal read the same list, so neither takes events from the other. The
    /// character screens' collector and the combat widgets' queue are filled beside it.
    game_events: Vec<UiEvent>,
    /// Render targets of the portraits (the paperdoll) by content id: the target, its
    /// registered texture and its size.
    portraits:
        std::collections::BTreeMap<u64, (tessera_render::OffscreenTarget, TextureId, (u32, u32))>,
    /// The portrait (content id, size) whose target could not be created: not tried again
    /// until its size changes (one warning, not one per frame).
    portrait_failed: Option<(u64, (u32, u32))>,
    /// The combat widgets and their queues (decision 0024 P2, [`combat`]).
    combat: combat::HostCombat,
}

/// Most UI events and panel commands queued for the character panels in one frame.
pub const MAX_CHARACTER_QUEUE: usize = 4096;

impl std::fmt::Debug for UiHost {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("UiHost")
            .field("panels", &self.panels)
            .field("textures", &self.textures.len())
            .field("lock", &self.lock.is_on())
            .field("frames", &self.frames)
            .finish_non_exhaustive()
    }
}

fn phase_of(p: OfferPhase) -> Phase {
    match p {
        OfferPhase::Start => Phase::Start,
        OfferPhase::Repeat => Phase::Repeat,
        OfferPhase::End => Phase::End,
    }
}

fn modifiers_of(m: InputModifiers) -> Modifiers {
    Modifiers {
        alt: m.contains(InputModifiers::ALT),
        control: m.contains(InputModifiers::CTRL),
        shift: m.contains(InputModifiers::SHIFT),
    }
}

fn limits_of(prefs: &UiPrefs) -> PaintLimits {
    PaintLimits {
        canvas_bytes: prefs.canvas_mib << 20,
        texture_bytes: prefs.texture_mib << 20,
        ..PaintLimits::default()
    }
}

impl UiHost {
    /// A host over any environment (synthetic runs and tests): no native panels.
    #[must_use]
    pub fn new(
        env: Box<dyn UiEnv>,
        handle: Option<EnvHandle>,
        vocab: Vocab,
        options: &UiOptions,
    ) -> Self {
        let (w, h) = options.screen;
        let mut ui = Ui::new(
            env,
            vocab,
            (
                i32::try_from(w).unwrap_or(i32::MAX),
                i32::try_from(h).unwrap_or(i32::MAX),
            ),
        );
        let limits = limits_of(&options.prefs);
        let mut textures = UiTextures::new();
        if options.prefs.scale_filter == tessera_settings::ScaleFilter::Nearest {
            // `[UI] ui_scale_filter = nearest` (decision 0015 §2.2): every entry is sampled
            // nearest instead of the composition list's bilinear default.
            textures.set_filter_override(Some(tessera_ui::paint::Filter::Nearest));
        }
        ui.set_paint_limits(limits);
        ui.set_tooltip_preferences(options.prefs.tooltip_enable, options.prefs.tooltip_delay);
        ui.set_cursor_scale(options.prefs.scale_cursor);
        let ui = Rc::new(RefCell::new(ui));
        let plugin_ui = PluginUi::new(Rc::clone(&ui));
        if let Some(h) = &handle {
            plugin_ui.set_text_pids(h.borrow().text_pids().for_plugins());
        }
        Self {
            ui,
            plugin_ui,
            env: handle,
            panels: None,
            maps: None,
            skins: None,
            textures,
            composition: CompositionList::default(),
            caps: CapCounters::default(),
            lock: PointerLock::new(),
            ids: BridgeIds::default(),
            prefs: options.prefs.clone(),
            limits,
            deferred: RefCell::new(Deferred::default()),
            chat_holds: BTreeSet::new(),
            player: WalkerPlayer::new(),
            plugin_wallet: None,
            pending_position: None,
            pending_notices: Vec::new(),
            pending_session: Vec::new(),
            entry_lines: VecDeque::new(),
            command_text_generation: 0,
            native_scale: if options.prefs.scale_global.is_finite()
                && options.prefs.scale_global > 0.0
            {
                options.prefs.scale_global
            } else {
                1.0
            },
            scaled_roots: Vec::new(),
            os_cursor_visible: None,
            event_window: 0,
            started: Instant::now(),
            clock: 0.0,
            frame: FrameAccum::default(),
            last: FrameSample::default(),
            recorder: options.record.then(Recorder::default),
            phase: "run",
            cursor_tex: None,
            frames: 0,
            command_ids: Vec::new(),
            character: None,
            character_events: Vec::new(),
            character_commands: Vec::new(),
            chargen: None,
            screen_events: None,
            game_events: Vec::new(),
            portraits: std::collections::BTreeMap::new(),
            portrait_failed: None,
            combat: combat::HostCombat::default(),
        }
    }

    /// A host without an install: the client's environment with no data (plugin files and
    /// Tessera's fallback font only).
    #[must_use]
    pub fn without_install(options: &UiOptions) -> Self {
        let state: EnvHandle = Rc::new(RefCell::new(EnvState::new(
            None,
            UI_WORKERS,
            options.prefs.image_lru_mib << 20,
            options.prefs.image_pinned_mib << 20,
        )));
        Self::new(
            Box::new(ClientEnv::new(Rc::clone(&state))),
            Some(state),
            Vocab::default(),
            options,
        )
    }

    /// The host over the install data of a finished [`boot::UiBoot`]: the client's
    /// environment, the native panels with the chat window and the map source, the skin
    /// selected by `[UI] UserSkin` (activated before any panel is built, so its geometry
    /// overrides apply, `skin-definition.md` §8.2, §9.3).
    #[must_use]
    pub fn from_boot(b: boot::UiBooted, options: &UiOptions) -> Self {
        let state: EnvHandle = Rc::new(RefCell::new(EnvState::new(
            Some(Arc::clone(&b.data)),
            UI_WORKERS,
            options.prefs.image_lru_mib << 20,
            options.prefs.image_pinned_mib << 20,
        )));
        let mut host = Self::new(
            Box::new(ClientEnv::new(Rc::clone(&state))),
            Some(state),
            b.vocab.clone(),
            options,
        );
        boot::register(&mut host.ui.borrow_mut(), &b.game_vocab);
        b.char_vocab.register_classes(&mut host.ui.borrow_mut());
        // The frames' name and level displays (P4, P5) draw text (`combat-ui.md` §4.1, §5.1).
        b.combat.vocab.register_classes(&mut host.ui.borrow_mut());
        host.character = Some(crate::character::panel::CharacterUi::new(
            b.char_vocab.clone(),
            b.char_ids.clone(),
        ));
        host.chargen = Some(b.chargen);
        host.combat.refs = Some(b.combat);
        host.combat.char_vocab = Some(b.char_vocab.clone());
        let mut panels = NativePanels::new(b.catalog, b.game_vocab.clone(), b.game_layouts);
        panels.types = b.panel_types;
        panels.set_shortcuts(Box::new(NoShortcuts));
        let mut chat = b.chat;
        if let Some(o) = options.prefs.chat.opacity {
            // `[UI.chat] opacity` (chat-window.md §10 item 1: the player's changes live in
            // Tessera's settings).
            chat.opacity_value = i32::from(o);
        }
        if let Err(e) = panels.set_chat(chat) {
            tracing::info!("chat window not configured: {e}");
        }
        host.panels = Some(panels);
        host.maps = b.maps.map(|r| {
            WorldMaps::with_pool(
                r,
                b.game_vocab.pids,
                tessera_assets::WorkerPool::new(tessera_assets::PoolConfig {
                    workers: 1,
                    max_queued: 1024,
                    thread_name: "tessera-ui-maps".to_owned(),
                }),
            )
        });
        host.command_ids = b.command_ids;
        if let Some(s) = b.skins {
            let mut sel = SkinSelection::new(s.skins, s.tables, s.env);
            let name = host.prefs.user_skin.clone();
            let sw = sel.apply_option(&name);
            if let Some(h) = &host.env {
                h.borrow_mut().set_skin(sw.current.clone());
            }
            host.ui.borrow_mut().set_skin(sw.current);
            host.skins = Some(sel);
        }
        host
    }

    /// The UI (shared with the plugin layer). Do not hold the borrow across a call into
    /// this host.
    #[must_use]
    pub fn ui(&self) -> Ref<'_, Ui> {
        self.ui.borrow()
    }

    /// The UI, mutably (tests and tools; same rule as [`Self::ui`]).
    #[must_use]
    pub fn ui_mut(&self) -> RefMut<'_, Ui> {
        self.ui.borrow_mut()
    }

    /// The plugin UI layer.
    #[must_use]
    pub fn plugin_ui(&self) -> &PluginUi {
        &self.plugin_ui
    }

    /// The native panels, when the install provides them.
    #[must_use]
    pub fn panels(&self) -> Option<&NativePanels> {
        self.panels.as_ref()
    }

    /// The environment's state, when it is the client's own.
    #[must_use]
    pub fn env(&self) -> Option<&EnvHandle> {
        self.env.as_ref()
    }

    /// The UI textures.
    #[must_use]
    pub fn textures(&self) -> &UiTextures {
        &self.textures
    }

    /// The last composition list (after [`Self::step8`]).
    #[must_use]
    pub fn composition(&self) -> &CompositionList {
        &self.composition
    }

    /// The pointer lock.
    #[must_use]
    pub fn pointer_lock(&self) -> &PointerLock {
        &self.lock
    }

    /// The last frame's figures.
    #[must_use]
    pub fn last_sample(&self) -> FrameSample {
        self.last
    }

    /// Whether the UI has settled: no position, notice or session line waits for step 7,
    /// no UI asset job is in flight and the paint scheduler is idle (every surface drawn,
    /// nothing left to paint or upload). Waiting for this is independent of how many
    /// frames the paint slices need on a loaded machine.
    #[must_use]
    pub fn settled(&self) -> bool {
        let in_flight = self
            .env
            .as_ref()
            .is_some_and(|h| h.borrow().stats().in_flight > 0);
        self.pending_position.is_none()
            && self.pending_notices.is_empty()
            && self.pending_session.is_empty()
            && !in_flight
            && self.ui.borrow().paint_settled()
    }

    /// The recorder, when recording.
    #[must_use]
    pub fn recorder(&self) -> Option<&Recorder> {
        self.recorder.as_ref()
    }

    /// Labels the frames recorded from now on (scripted runs).
    pub fn set_phase(&mut self, phase: &'static str) {
        self.phase = phase;
    }

    /// Marks the current frame as an event frame (U4).
    pub fn mark_event(&mut self) {
        self.frame.event = true;
    }

    /// Marks the current frame as a content-event frame (U4 (f)).
    pub fn mark_content_event(&mut self) {
        self.frame.content_event = true;
    }

    /// The preferences in use.
    #[must_use]
    pub fn prefs(&self) -> &UiPrefs {
        &self.prefs
    }

    /// The frame clock in seconds.
    #[must_use]
    pub fn clock(&self) -> f64 {
        self.clock
    }

    /// The skin selection, when the install has skin tables.
    #[must_use]
    pub fn skins(&self) -> Option<&SkinSelection> {
        self.skins.as_ref()
    }

    /// Activates the skin named `name` (empty = none) without rebuilding panels
    /// (`skin-definition.md` §6.6, §9.3). Returns whether a skin is active afterwards.
    pub fn set_skin(&mut self, name: &str) -> bool {
        let Some(sel) = self.skins.as_mut() else {
            return false;
        };
        let sw = sel.apply_option(name);
        let active = sw.current.is_some();
        if let Some(h) = &self.env {
            h.borrow_mut().set_skin(sw.current.clone());
        }
        self.ui.borrow_mut().set_skin(sw.current);
        self.prefs.user_skin = name.to_owned();
        active
    }

    // ----- attaching -----------------------------------------------------------------

    /// Attaches the plugin host (ARCHITECTURE §20.1 step 9: plugins load after the UI is
    /// ready): the `Turbine.UI` classes go into new apartments, step 7 runs the plugin
    /// host's per-frame work, the local player is the walker's avatar (with the wallet views
    /// of [`Self::set_plugin_wallet`], when given), and `load` is queued. This and
    /// [`Self::shutdown`] are the only writers of the plugins' local player source in the
    /// windowed client.
    pub fn attach_plugins(&mut self, plugins: &mut Plugins, load: &[String]) {
        plugins.set_ui_binding(Some(self.plugin_ui.binding()));
        plugins.set_ui_driven(true);
        if let Some(h) = &self.env {
            h.borrow_mut()
                .set_plugins_dir(Some(plugins.manager().plugins_dir().to_path_buf()));
        }
        let src: Arc<dyn tessera_plugins::LocalPlayerSource> = match &self.plugin_wallet {
            Some(w) => crate::npc::wallet::WalletPlayer::new(self.player.clone(), Arc::clone(w)),
            None => self.player.clone(),
        };
        plugins.manager_mut().set_local_player_source(Some(src));
        for name in load {
            if plugins.manager_mut().queue_load(name) {
                tracing::info!(plugin = %name, "plugin load queued");
            } else {
                tracing::warn!(plugin = %name, "plugin not loaded: no available plugin of that name");
            }
        }
    }

    /// Attaches the input mapper (decision 0014): the `pointer` context goes on, the
    /// command IDs from the install are handed over, and the UI band's contexts are
    /// looked up.
    pub fn attach_mapper(&mut self, mapper: &mut InputMapper) {
        let reg = mapper.registry();
        let mut ids = BridgeIds::default();
        for name in [
            contexts::TEXT_ENTRY,
            contexts::UI_DIALOG,
            contexts::UI_CLIPBOARD,
            contexts::UI_SCROLL_MOUSE,
            contexts::UI_SCROLL,
            contexts::UI_EAT_KEYBOARD,
            contexts::UI_CHAT_LOG,
        ] {
            if let Some(c) = reg.context(name) {
                ids.contexts.push((name, c));
            }
        }
        ids.mouselook = reg.action("mouselook");
        ids.menu = reg.action("menu");
        let pointer = reg.context(contexts::POINTER);
        if let Some(p) = pointer {
            mapper.activate(p);
        }
        if !self.command_ids.is_empty() {
            mapper.set_command_ids(self.command_ids.iter().map(|(n, i)| (n.as_str(), *i)));
        }
        self.ids = ids;
    }

    // ----- panels --------------------------------------------------------------------

    fn services_run<R>(
        &mut self,
        plugins: &mut Plugins,
        f: impl FnOnce(&mut NativePanels, &mut Ui, &mut Services<'_>) -> R,
    ) -> Option<R> {
        let panels = self.panels.as_mut()?;
        let mut view = PluginView::new(plugins.manager(), &self.deferred);
        let mut no_maps = NoMaps;
        let maps: &mut dyn MapSource = match self.maps.as_mut() {
            Some(m) => m,
            None => &mut no_maps,
        };
        let mut sv = Services {
            maps,
            plugins: &mut view,
        };
        let r = {
            let mut ui = self.ui.borrow_mut();
            f(panels, &mut ui, &mut sv)
        };
        Some(r)
    }

    /// Opens the HUD set. Returns the panels that failed, with the reason.
    pub fn open_hud(&mut self, plugins: &mut Plugins) -> Vec<(PanelId, PanelsError)> {
        self.frame.event = true;
        let r = self
            .services_run(plugins, |p, ui, sv| p.open_hud(ui, sv))
            .unwrap_or_default();
        self.after_panel_call(plugins);
        r.into_iter()
            .filter_map(|(id, r)| r.err().map(|e| (id, e)))
            .collect()
    }

    /// Opens (or shows) a panel by `ui-layout.md` §17.1.
    ///
    /// # Errors
    ///
    /// The panels' error, or [`PanelsError::NotInCatalogue`] without native panels.
    pub fn open_panel(
        &mut self,
        plugins: &mut Plugins,
        id: PanelId,
    ) -> Result<ElementKey, PanelsError> {
        self.frame.event = true;
        let r = self
            .services_run(plugins, |p, ui, sv| {
                if id == PanelId::PluginManager {
                    p.open_plugin_manager(ui, sv)
                } else {
                    p.open(ui, id, sv)
                }
            })
            .unwrap_or(Err(PanelsError::NotInCatalogue(id)));
        self.after_panel_call(plugins);
        r
    }

    /// Hides a panel.
    pub fn hide_panel(&mut self, plugins: &mut Plugins, id: PanelId) {
        self.frame.event = true;
        if let Some(p) = self.panels.as_mut() {
            p.hide(&mut self.ui.borrow_mut(), id);
        }
        self.after_panel_call(plugins);
    }

    /// The Escape rule for a `menu` press the UI declined (`game-panels.md` §16, `walker.md`
    /// §5). `None` without native panels.
    pub fn escape(
        &mut self,
        plugins: &mut Plugins,
        press: bool,
        repeat: bool,
    ) -> Option<EscapeOutcome> {
        self.frame.event = true;
        let r = self.services_run(plugins, |p, ui, sv| p.escape(ui, press, repeat, sv));
        self.after_panel_call(plugins);
        r
    }

    /// Selects or clears the target (P5 is shown only while one is selected, the Tessera
    /// choice of `ui-layout.md` §17.4).
    pub fn set_target(&mut self, plugins: &mut Plugins, selected: bool) {
        if let Some(p) = self.panels.as_mut() {
            p.set_target(&mut self.ui.borrow_mut(), selected);
        }
        self.after_panel_call(plugins);
    }

    /// A window scale-factor change (`keymap.md` §7.7 and §7.8.9, decision 0014
    /// Amendment 1 item 6): `timing` is the new *T*, *W*, *H* (scaled by the new factor),
    /// `ratio` = *s*new ÷ *s*old, `area` the client area in physical pixels after the change.
    /// During the pointer lock the frozen position becomes *F*′ (the lock stays on), the UI
    /// is told *F*′ and the mapper gets *F*′ as its cursor position; otherwise the mapper's
    /// cursor position is rescaled by `ratio`. The mapper forgets every recorded press
    /// position, so the next press is never nearby.
    pub fn rescale(
        &mut self,
        mapper: &mut InputMapper,
        timing: tessera_input::ActivationTiming,
        ratio: f64,
        area: (u32, u32),
    ) {
        let cursor = match self.lock.rescale(ratio, area) {
            Some(f) => {
                self.ui
                    .borrow_mut()
                    .set_pointer_lock(Some((f.0.floor() as i32, f.1.floor() as i32)));
                Some(f)
            }
            None => rescaled_cursor(mapper.cursor(), ratio),
        };
        mapper.rescale(timing, cursor);
    }

    /// A position update from game code; applied in step 7 (decision 0014 D8).
    pub fn update_position(&mut self, input: RadarInput, area: Option<u32>) {
        self.pending_position = Some((input, area));
    }

    /// The wallet views (`lua-plugin-api.md` §15.3) the plugins' local player carries from
    /// the next [`Self::attach_plugins`] on: the NPC windows' session wallet (P3).
    pub fn set_plugin_wallet(&mut self, wallet: Arc<crate::npc::SessionWallet>) {
        self.plugin_wallet = Some(wallet);
    }

    /// The character the plugins see (the walker's avatar). A change raises `NameChanged`
    /// for step 7.
    pub fn set_character(&mut self, plugins: &mut Plugins, name: Option<&str>) {
        if self.player.set(name) {
            plugins
                .manager_mut()
                .raise_gameplay_event(GameplayEvent::LocalPlayer(LocalPlayerEvent::NameChanged));
        }
    }

    /// Tessera's own system notices for the chat window (posted in step 7 as type 8 `Log`
    /// lines, `chat-window.md` §10 item 2).
    pub fn post_notices(&mut self, notices: impl IntoIterator<Item = String>) {
        for n in notices {
            if self.pending_notices.len() < 256 {
                self.pending_notices.push(n);
            }
        }
    }

    /// Session chat lines (with or without a sender) for the chat window, posted in step 7
    /// with their chat type. Each reaches the plugins' `Received` exactly once, through
    /// the window's intake (`chat-window.md` §10 item 9); the caller must not also hand
    /// them to the plugins.
    pub fn post_session_lines(&mut self, lines: impl IntoIterator<Item = crate::net::ChatLine>) {
        for l in lines {
            if self.pending_session.len() < 256 {
                self.pending_session.push(l);
            }
        }
    }

    /// A new screen size (applied at the start of step 7, decision 0014 Amendment 1
    /// item 2).
    pub fn resize(&mut self, width: u32, height: u32) {
        self.frame.event = true;
        self.ui.borrow_mut().request(UiChange::Screen(
            i32::try_from(width.max(1)).unwrap_or(i32::MAX),
            i32::try_from(height.max(1)).unwrap_or(i32::MAX),
        ));
    }

    /// Scales every open native panel, and every panel opened later (the scale-change
    /// event of 0015 §6, and `[UI] ui_scale_global` at attach). `tessera-ui` has no setter
    /// for `UIScale_Global` itself, so the panels' scalers are used; plugin windows'
    /// `SetScale` does not see it (recorded in `docs/perf/m5-ui-budgets.md`).
    pub fn scale_native_panels(&mut self, scale: f32) {
        self.frame.event = true;
        if scale.is_finite() && scale > 0.0 {
            self.native_scale = scale;
        }
        self.apply_native_scale();
    }

    /// The scale native panels take.
    #[must_use]
    pub fn native_scale(&self) -> f32 {
        self.native_scale
    }

    /// Gives every open native panel root [`Self::native_scale`] once (roots opened by
    /// any path: the host's calls, Escape, panel code reacting to events).
    fn apply_native_scale(&mut self) {
        let Some(p) = self.panels.as_ref() else {
            return;
        };
        let roots: Vec<ElementKey> = p.opened().map(|(_, k)| k).collect();
        self.scaled_roots.retain(|(k, _)| roots.contains(k));
        let s = self.native_scale;
        let mut todo = Vec::new();
        for k in roots {
            let current = self
                .scaled_roots
                .iter()
                .find(|(r, _)| *r == k)
                .map_or(1.0, |(_, v)| *v);
            if (current - s).abs() > f32::EPSILON {
                todo.push(k);
            }
        }
        if todo.is_empty() {
            return;
        }
        let mut ui = self.ui.borrow_mut();
        for k in todo {
            ui.scale_panel(k, s, true);
            match self.scaled_roots.iter_mut().find(|(r, _)| *r == k) {
                Some(e) => e.1 = s,
                None => self.scaled_roots.push((k, s)),
            }
        }
    }

    fn after_panel_call(&mut self, plugins: &mut Plugins) {
        if let Some(p) = self.panels.as_mut() {
            p.prune(&mut self.ui.borrow_mut());
        }
        self.run_deferred(plugins);
        self.deliver(plugins);
        self.apply_native_scale();
    }

    /// Keeps the OS cursor hidden over the window while the software cursor is in use
    /// (decision 0015 §2.5, `render.md` §6.11) and shown otherwise. Calls the window only
    /// when the wanted state changes; the pointer lock's own hide and show during a lock
    /// leave it hidden in software mode. Call once per frame with the window.
    pub fn sync_cursor(&mut self, window: &mut dyn CursorWindow) {
        if self.lock.is_on() {
            // The lock owns the cursor while it is on.
            return;
        }
        let want = self.prefs.cursor == CursorMode::Os;
        match self.os_cursor_visible {
            // The OS cursor starts visible: nothing to do in `os` mode.
            None if want => self.os_cursor_visible = Some(true),
            Some(v) if v == want => {}
            _ => {
                window.set_cursor_visible(want);
                self.os_cursor_visible = Some(want);
            }
        }
    }

    /// Shows the OS cursor again if [`Self::sync_cursor`] hid it (the UI shuts down).
    pub fn release_cursor(&mut self, window: &mut dyn CursorWindow) {
        if self.os_cursor_visible == Some(false) {
            window.set_cursor_visible(true);
        }
        self.os_cursor_visible = None;
    }

    // ----- plugin plumbing -----------------------------------------------------------

    /// Runs plugin work queued by panel code (module [`services`]).
    fn run_deferred(&mut self, plugins: &mut Plugins) {
        let d = std::mem::take(&mut *self.deferred.borrow_mut());
        if d.is_empty() {
            return;
        }
        let m = plugins.manager_mut();
        for name in d.loads {
            if !m.queue_load(&name) {
                tracing::info!(plugin = %name, "load refused: no available plugin of that name");
            }
        }
        for ap in d.unloads {
            match ap {
                None => m.unload_all(),
                Some(a) => {
                    if let Err(e) = m.unload_apartment(&a) {
                        tracing::info!(apartment = %a, "unload: {e}");
                    }
                }
            }
        }
        if d.refresh {
            m.request_refresh(true);
        }
    }

    /// Delivers plugin notifications and routes the client events to the panels, until
    /// nothing is left (at most [`MAX_DELIVERY_ROUNDS`] rounds).
    fn deliver(&mut self, plugins: &mut Plugins) {
        for _ in 0..MAX_DELIVERY_ROUNDS {
            let events = self.plugin_ui.deliver(plugins.manager());
            if events.is_empty() && self.deferred.borrow().is_empty() {
                break;
            }
            self.handle_client_events(plugins, events);
            self.run_deferred(plugins);
        }
        self.apply_native_scale();
    }

    fn handle_client_events(&mut self, plugins: &mut Plugins, events: Vec<UiEvent>) {
        if let Some(q) = self.screen_events.as_mut() {
            let room = MAX_CHARACTER_QUEUE.saturating_sub(q.len());
            q.extend(events.iter().take(room).cloned());
        }
        let room = MAX_CHARACTER_QUEUE.saturating_sub(self.game_events.len());
        self.game_events.extend(events.iter().take(room).cloned());
        if self.character.is_some() {
            for ev in &events {
                if self.character_events.len() >= MAX_CHARACTER_QUEUE {
                    break;
                }
                if matches!(
                    ev,
                    UiEvent::ExamineHover { .. } | UiEvent::DragEnded { .. } | UiEvent::Game { .. }
                ) {
                    self.character_events.push(ev.clone());
                }
            }
        }
        for ev in &events {
            self.queue_combat_event(ev);
        }
        if events.is_empty() || self.panels.is_none() {
            return;
        }
        self.services_run(plugins, |p, ui, sv| {
            for ev in &events {
                let _ = p.handle_event(ui, ev, sv);
            }
            p.prune(ui);
        });
    }

    // ----- step 1 --------------------------------------------------------------------

    /// Whether a UI element has the keyboard focus (then keys are the UI's, not the
    /// walker's own keys).
    #[must_use]
    pub fn keyboard_focused(&self) -> bool {
        self.ui.borrow().focus_element().is_some()
    }

    /// Step 1: feeds one raw event to `mapper` with this host as its UI sink. While the
    /// pointer lock is on, cursor-position and cursor-left reports are withheld (0015
    /// §2.5); a cursor-left withheld during the lock is fed right after the call that ends
    /// it.
    pub fn handle_raw(
        &mut self,
        mapper: &mut InputMapper,
        event: &RawEvent,
        now: Duration,
        plugins: &mut Plugins,
        window: &mut dyn CursorWindow,
    ) {
        let t = Instant::now();
        let is_pointer = matches!(event, RawEvent::CursorMoved { .. } | RawEvent::CursorLeft);
        if is_pointer
            && self.lock.filter(matches!(event, RawEvent::CursorLeft)) == PointerReport::Withhold
        {
            return;
        }
        {
            let mut sink = Step1 {
                host: self,
                plugins,
                window,
            };
            mapper.handle_with(event, now, &mut sink);
        }
        if self.lock.take_withheld_left() {
            let mut sink = Step1 {
                host: self,
                plugins,
                window,
            };
            mapper.handle_with(&RawEvent::CursorLeft, now, &mut sink);
        }
        let e = t.elapsed();
        self.frame.host += e;
        self.frame.step1 += e;
    }

    /// Step 1, after the last raw event of the frame: the end queue and the pending
    /// pointer move (`keymap.md` §7.8.8). Call before the mapper publishes its snapshot.
    pub fn offer_pending(
        &mut self,
        mapper: &mut InputMapper,
        plugins: &mut Plugins,
        window: &mut dyn CursorWindow,
    ) {
        let t = Instant::now();
        {
            let mut sink = Step1 {
                host: self,
                plugins,
                window,
            };
            mapper.offer_pending(&mut sink);
        }
        let e = t.elapsed();
        self.frame.host += e;
        self.frame.step1 += e;
    }

    fn offer(
        &mut self,
        event: &OfferedEvent,
        requests: &mut BandRequests,
        plugins: &mut Plugins,
        window: &mut dyn CursorWindow,
    ) -> Verdict {
        let consumed = match event {
            OfferedEvent::Action(a) => {
                self.plugin_ui.set_modifiers(modifiers_of(a.modifiers));
                self.offer_action(a, plugins, window)
            }
            OfferedEvent::Text { ch, .. } => self.offer_text(*ch, plugins),
            OfferedEvent::PointerMoved { x, y, .. } => {
                self.frame.pointer_events = self.frame.pointer_events.saturating_add(1);
                self.ui
                    .borrow_mut()
                    .offer(&UiInput::PointerMoved { x: *x, y: *y })
            }
            OfferedEvent::PointerLeft { .. } => self.ui.borrow_mut().offer(&UiInput::PointerLeft),
            OfferedEvent::FocusLost { .. } => {
                let r = self.ui.borrow_mut().offer(&UiInput::FocusLost);
                self.lock.reset(window, self.prefs.cursor == CursorMode::Os);
                self.ui.borrow_mut().set_pointer_lock(None);
                r
            }
        };
        self.deliver(plugins);
        self.band_requests(requests);
        Verdict::from_bool(consumed)
    }

    fn offer_action(
        &mut self,
        a: &tessera_input::ActionOffer,
        plugins: &mut Plugins,
        window: &mut dyn CursorWindow,
    ) -> bool {
        let command = Command {
            name: Arc::clone(&a.command),
            id: a.command_id,
        };
        let phase = phase_of(a.phase);
        let hold = a.hold.0;
        let wheel_step = is_wheel_step(&command);
        let is_pointer_cmd = command.is_mouse_button() || command.is_wheel();
        // The chat window sees key presses first while it is the keyboard target
        // (`Chat::command`); its holds are not offered to the UI again.
        if !is_pointer_cmd {
            if phase == Phase::End && self.chat_holds.remove(&hold) {
                return true;
            }
            if phase != Phase::End
                && self.panels.as_ref().is_some_and(|p| p.chat.is_some())
                && let Some(out) = self.chat_command(plugins, &command.name, a.modifiers)
                && out != CommandOutcome::NotHandled
            {
                if phase == Phase::Start {
                    self.chat_holds.insert(hold);
                }
                if matches!(out, CommandOutcome::Submitted(_)) {
                    self.frame.content_event = true;
                }
                return true;
            }
        }
        let name = Arc::clone(&command.name);
        let consumed = self.ui.borrow_mut().offer(&UiInput::Command(CommandOffer {
            phase,
            hold,
            command,
            value: a.value,
            position: a.position,
        }));
        // The P1 panel commands the UI declined (`character-panels.md` §2, §4.1,
        // `item-tooltips.md` §10): run with the player's state in step 7.
        if !consumed
            && !is_pointer_cmd
            && self.character.is_some()
            && crate::character::panel::is_panel_command(&name)
        {
            if phase == Phase::Start && self.character_commands.len() < MAX_CHARACTER_QUEUE {
                self.character_commands.push(name.to_string());
            }
            return true;
        }
        // The P2 targeting and quickslot commands and world clicks the UI declined
        // (`keymap.md` §8.2, `combat-ui.md` TR3, TR33): for the combat frame.
        if consumed {
            self.combat_taken_left(&name);
        } else if self.combat_declined(a, self.lock.is_on()) {
            return true;
        }
        if consumed && wheel_step {
            // A scroll step is a content event (U4 (f), decision 0015 §6).
            self.frame.content_event = true;
        }
        if Some(a.action) == self.ids.mouselook {
            match phase {
                Phase::Start => {
                    let drag = self.ui.borrow().item_drag_active();
                    let out = self.lock.start(hold, consumed, drag, a.position, window);
                    if out == StartOutcome::Locked {
                        let f = self
                            .lock
                            .frozen()
                            .map(|(x, y)| (x.floor() as i32, y.floor() as i32));
                        self.ui.borrow_mut().set_pointer_lock(f);
                        return false;
                    }
                    return true;
                }
                Phase::End => {
                    if self
                        .lock
                        .end(hold, window, self.prefs.cursor == CursorMode::Os)
                    {
                        self.ui.borrow_mut().set_pointer_lock(None);
                    }
                }
                Phase::Repeat => {}
            }
        }
        consumed
    }

    /// Offers a command to the chat window (`chat-window.md` §6.2). Line up and down browse
    /// the dispatcher's history; a submitted line is queued for
    /// [`Self::dispatch_entry_lines`], which runs it once the UI is no longer borrowed
    /// (plugin command handlers may drive the UI).
    fn chat_command(
        &mut self,
        plugins: &mut Plugins,
        name: &str,
        mods: InputModifiers,
    ) -> Option<CommandOutcome> {
        let shift = mods.contains(InputModifiers::SHIFT);
        let ctrl = mods.contains(InputModifiers::CTRL);
        let chat = self.panels.as_mut()?.chat.as_mut()?;
        let out = {
            let mut ui = self.ui.borrow_mut();
            chat.command(&mut ui, name, shift, ctrl, plugins.history_mut())
        };
        if let CommandOutcome::Submitted(line) = &out {
            self.queue_entry_line(line.clone());
        }
        Some(out)
    }

    fn queue_entry_line(&mut self, line: EntryLine) {
        if self.entry_lines.len() >= MAX_ENTRY_LINES {
            tracing::warn!(
                "chat: {MAX_ENTRY_LINES} lines wait for the command dispatcher; line dropped"
            );
            return;
        }
        self.entry_lines.push_back(line);
    }

    /// Whether lines submitted in the chat window wait for [`Self::dispatch_entry_lines`].
    #[must_use]
    pub fn has_entry_lines(&self) -> bool {
        !self.entry_lines.is_empty()
    }

    /// Runs the lines submitted in the chat window's entry (and the chat commands of
    /// quickslot aliases) through the client's command dispatcher, in order (decision
    /// 0020 item 1): built-ins, plugin commands, the channel commands with `session` (the
    /// test server with `--server`, the sandbox otherwise; `None`: no session), the
    /// unknown-command line. Call in frame-loop step 1 right after the raw events and
    /// pending offers, and after step 7 (aliases), with the world session. The replies
    /// reach the chat window in step 7 ([`Plugins::take_local_chat`]).
    pub fn dispatch_entry_lines(
        &mut self,
        plugins: &mut Plugins,
        mut session: Option<&mut dyn crate::walker::WalkerSession>,
        now: Instant,
    ) {
        if self.entry_lines.is_empty() {
            return;
        }
        let t = Instant::now();
        while let Some(line) = self.entry_lines.pop_front() {
            let outcome =
                plugins.submit_entry(&line, crate::commands::shorten(session.as_deref_mut()), now);
            tracing::debug!(?outcome, "chat window line dispatched");
        }
        self.frame.content_event = true;
        // Plugin command handlers may have changed the UI or raised plugin events.
        self.plugin_ui.flush_layouts();
        self.deliver(plugins);
        self.frame.host += t.elapsed();
    }

    fn offer_text(&mut self, ch: char, plugins: &mut Plugins) -> bool {
        let now = self.clock;
        let chat = self.panels.as_mut().and_then(|p| p.chat.as_mut());
        let r = match chat {
            Some(c) => !matches!(
                c.text_input(&mut self.ui.borrow_mut(), ch, now),
                TextInputOutcome::NotTaken
            ),
            None => self.ui.borrow_mut().offer(&UiInput::Char(ch)),
        };
        let _ = plugins;
        r
    }

    /// Band changes of the UI as an offer's requests (decision 0014 D5).
    fn band_requests(&mut self, requests: &mut BandRequests) {
        let changes = self.ui.borrow_mut().take_band_changes();
        for c in changes {
            match c {
                BandChange::Push {
                    map,
                    owner,
                    priority,
                } => {
                    if let Some(ctx) = self.ids.context(&map) {
                        requests.push(ctx, owner, priority);
                    }
                }
                BandChange::Pop { map, owner } => {
                    if let Some(ctx) = self.ids.context(&map) {
                        requests.pop(ctx, owner);
                    }
                }
            }
        }
    }

    /// Applies the UI's band changes made outside offers to the mapper (after step 7 and
    /// step 8).
    pub fn apply_band_changes(&mut self, mapper: &mut InputMapper) {
        let changes = self.ui.borrow_mut().take_band_changes();
        for c in changes {
            match c {
                BandChange::Push {
                    map,
                    owner,
                    priority,
                } => {
                    if let Some(ctx) = self.ids.context(&map) {
                        mapper.band_push(ctx, owner, priority);
                    }
                }
                BandChange::Pop { map, owner } => {
                    if let Some(ctx) = self.ids.context(&map) {
                        mapper.band_pop(ctx, owner);
                    }
                }
            }
        }
    }

    // ----- step 7 --------------------------------------------------------------------

    /// Step 7 (module docs).
    pub fn step7(&mut self, plugins: &mut Plugins, mapper: Option<&mut InputMapper>) {
        let t = Instant::now();
        if let Some(h) = &self.env {
            let a = h.borrow_mut().poll();
            let mut ui = self.ui.borrow_mut();
            if a.images {
                ui.images_arrived();
            }
            if a.fonts {
                ui.invalidate_text();
            }
        }
        self.ui.borrow_mut().tick_hover();
        self.deliver(plugins);
        self.plugin_ui.update_tick(plugins.manager());
        self.ui.borrow_mut().frame_work();
        self.deliver(plugins);
        if let Some((input, area)) = self.pending_position.take()
            && let Some(panels) = self.panels.as_mut()
        {
            let mut no_maps = NoMaps;
            let maps: &mut dyn MapSource = match self.maps.as_mut() {
                Some(m) => m,
                None => &mut no_maps,
            };
            panels.update_position(&mut self.ui.borrow_mut(), &input, area, maps);
        }
        self.sync_command_text(plugins);
        self.chat_io(plugins);
        if plugins.take_manager_request() {
            let _ = self.open_panel(plugins, PanelId::PluginManager);
        }
        // Quickslot aliases run their chat commands like typed lines (`plugin-api-usage.md`
        // §14.6): through the command dispatcher, in the `Say` mode, with the session the
        // caller gives to `dispatch_entry_lines` after this step.
        for text in self.plugin_ui.take_chat_commands() {
            if !text.is_empty() {
                self.queue_entry_line(EntryLine {
                    text,
                    mode: tessera_ui::game::chat::entry::SAY,
                    word: String::new(),
                    target: String::new(),
                });
            }
        }
        plugins.pump_step7();
        self.deliver(plugins);
        if let Some(m) = mapper {
            self.apply_band_changes(m);
        }
        let e = t.elapsed();
        self.frame.host += e;
        self.frame.step7 += e;
    }

    /// The character panels' frame work (after step 7): the panel commands taken in step
    /// 1, the UI events of this frame (item tooltips, drops, activations, the paperdoll's
    /// turning), the player-state `changes`, and the drag turn. Returns the commands for
    /// the session; the panels' messages go to the chat window as notices.
    pub fn character_frame(
        &mut self,
        state: &crate::character::PlayerState,
        data: Option<&crate::character::CharacterData>,
        changes: &[crate::character::PlayerChange],
        shift: bool,
        now: Instant,
    ) -> Vec<tessera_net::Command> {
        let Some(c) = self.character.as_mut() else {
            self.character_events.clear();
            self.character_commands.clear();
            return Vec::new();
        };
        let mut ui = self.ui.borrow_mut();
        c.set_split_modifier(shift);
        for name in std::mem::take(&mut self.character_commands) {
            c.command(&mut ui, &name, state, data);
        }
        c.refresh(&mut ui, state, data, changes);
        for ev in std::mem::take(&mut self.character_events) {
            c.handle_event(&mut ui, &ev, state, data, now);
        }
        c.update(&mut ui);
        c.prune(&mut ui);
        drop(ui);
        let notices = c.take_notices();
        let commands = c.take_commands();
        if !notices.is_empty() {
            self.post_notices(notices);
        }
        if !commands.is_empty() {
            self.frame.content_event = true;
        }
        commands
    }

    /// Renders the portraits (`render.md` §6.10) into their targets before the frame: the
    /// paperdoll's, when the character panel shows it, and `screens`, the character
    /// screens' viewport (`character-screens.md` §2.1). `draw` draws a portrait's figure
    /// into an offscreen frame and returns whether it drew one. The UI composes each target
    /// as its viewport's external content. A closed or hidden panel renders nothing; the
    /// screens' target is freed when they no longer ask for one. Call before the frame's
    /// `begin_frame`.
    pub fn render_portraits(
        &mut self,
        renderer: &mut Renderer<'_>,
        screens: Option<crate::character::panel::Portrait>,
        draw: &mut dyn FnMut(&mut Frame, &crate::character::panel::Portrait) -> bool,
    ) {
        let paperdoll = self
            .character
            .as_ref()
            .and_then(|c| c.portrait(&self.ui.borrow()));
        if screens.is_none()
            && let Some((_, tex, _)) = self
                .portraits
                .remove(&crate::screens::ui::VIEWPORT_CONTENT_ID)
        {
            renderer.destroy_texture(tex);
            self.textures
                .set_external(crate::screens::ui::VIEWPORT_CONTENT_ID, None);
        }
        for p in [screens, paperdoll].into_iter().flatten() {
            self.render_portrait(renderer, &p, draw);
        }
    }

    /// One portrait of [`Self::render_portraits`].
    fn render_portrait(
        &mut self,
        renderer: &mut Renderer<'_>,
        p: &crate::character::panel::Portrait,
        draw: &mut dyn FnMut(&mut Frame, &crate::character::panel::Portrait) -> bool,
    ) {
        if self.portrait_failed == Some((p.id, p.size)) {
            return;
        }
        let stale = self
            .portraits
            .get(&p.id)
            .is_some_and(|(_, _, size)| *size != p.size);
        if stale && let Some((_, tex, _)) = self.portraits.remove(&p.id) {
            renderer.destroy_texture(tex);
            self.textures.set_external(p.id, None);
        }
        if let std::collections::btree_map::Entry::Vacant(slot) = self.portraits.entry(p.id) {
            let made = renderer
                .create_offscreen_target(
                    p.size.0,
                    p.size.1,
                    tessera_render::OffscreenFormat::Rgba8Unorm,
                )
                .and_then(|t| {
                    let tex = renderer.register_target_texture(
                        &t,
                        tessera_render::Filter::Linear,
                        tessera_render::Wrap::Clamp,
                    )?;
                    Ok((t, tex))
                });
            match made {
                Ok((t, tex)) => {
                    slot.insert((t, tex, p.size));
                    self.textures.set_external(p.id, Some(tex));
                    self.portrait_failed = None;
                }
                Err(e) => {
                    tracing::warn!(
                        size = ?p.size,
                        "portrait target: {e}; not tried again at this size"
                    );
                    self.portrait_failed = Some((p.id, p.size));
                    return;
                }
            }
        }
        let Some((target, _, _)) = self.portraits.get(&p.id) else {
            return;
        };
        match renderer.begin_offscreen_frame(target) {
            Ok(mut frame) => {
                draw(&mut frame, p);
                if let Err(e) = renderer.end_frame(frame) {
                    tracing::debug!("portrait: {e}");
                }
            }
            Err(e) => tracing::debug!("portrait: {e}"),
        }
    }

    /// The character screens' layouts and IDs, when the install provides them.
    #[must_use]
    pub fn chargen(&self) -> Option<crate::screens::ui::ChargenRefs> {
        self.chargen
    }

    /// Starts (`true`) or stops collecting every UI event for the character screens.
    pub fn collect_screen_events(&mut self, on: bool) {
        self.screen_events = on.then(Vec::new);
    }

    /// The UI events collected for the character screens since the last call.
    pub fn take_screen_events(&mut self) -> Vec<UiEvent> {
        self.screen_events
            .as_mut()
            .map(std::mem::take)
            .unwrap_or_default()
    }

    /// The UI events since the last call (step 1 and step 7), for the in-world windows (the
    /// NPC windows and the quest journal read the one list). Take them once per frame; at
    /// most [`MAX_CHARACTER_QUEUE`] wait (later ones are dropped).
    pub fn take_game_events(&mut self) -> Vec<UiEvent> {
        std::mem::take(&mut self.game_events)
    }

    /// The character panels, when the install provides them.
    #[must_use]
    pub fn character(&self) -> Option<&crate::character::panel::CharacterUi> {
        self.character.as_ref()
    }

    /// The character panels, mutably (opening panels by name, sheets and tests).
    pub fn character_mut(&mut self) -> Option<&mut crate::character::panel::CharacterUi> {
        self.character.as_mut()
    }

    /// Runs a panel command by name as an input action would (`ToggleJournalPanel`,
    /// `QUICKSLOT_BACKPACKn`, …) with the player's state; `false` for other names.
    pub fn character_command(
        &mut self,
        name: &str,
        state: &crate::character::PlayerState,
        data: Option<&crate::character::CharacterData>,
    ) -> bool {
        let Some(c) = self.character.as_mut() else {
            return false;
        };
        let mut ui = self.ui.borrow_mut();
        c.command(&mut ui, name, state, data)
    }

    /// The chat window takes its command syntax and mode table from the dispatcher's one
    /// read of the install's tables whenever the dispatcher has taken a new one
    /// (`chat-window.md` §10 item 11), so it offers only mode words the registry holds.
    fn sync_command_text(&mut self, plugins: &Plugins) {
        let generation = plugins.command_text_generation();
        if generation == self.command_text_generation {
            return;
        }
        if let Some(chat) = self.panels.as_mut().and_then(|p| p.chat.as_mut()) {
            chat.set_command_text(plugins.command_text());
            self.command_text_generation = generation;
            tracing::debug!(
                generation,
                modes = chat.setup().modes.len(),
                "chat window took the dispatcher's command words"
            );
        }
    }

    /// Chat plumbing of step 7 (`chat-window.md` §10 items 2 and 9). Every line reaches
    /// the plugins' `Received` exactly once:
    ///
    /// - lines the window made itself since the last frame (the reply line) go to
    ///   `Received` with the type of their mask;
    /// - the client's own lines (the command dispatcher's replies, player music; type 1 or
    ///   8), Tessera's notices (type 8) and the session's lines (their own type and sender)
    ///   are shown in the window and queued for `Received` here, with the sender;
    /// - lines that came from the plugins are shown only: the plugin host already queued
    ///   them for `Received`.
    ///
    /// Without a chat window the client's lines, notices and session lines still go to
    /// `Received` once.
    fn chat_io(&mut self, plugins: &mut Plugins) {
        let from_plugins = plugins.take_chat();
        let local = plugins.take_local_chat();
        let notices = std::mem::take(&mut self.pending_notices);
        let session = std::mem::take(&mut self.pending_session);
        let mut received: Vec<(Option<String>, String, u32)> = Vec::new();
        let had_lines = !notices.is_empty()
            || !session.is_empty()
            || !from_plugins.is_empty()
            || !local.is_empty();
        match self.panels.as_mut().and_then(|p| p.chat.as_mut()) {
            Some(chat) => {
                let mut ui = self.ui.borrow_mut();
                for (mask, text) in chat.take_outgoing() {
                    let chat_type = (64 - mask.leading_zeros()).max(1);
                    received.push((None, text, chat_type));
                }
                for (t, text) in local {
                    chat.show(&mut ui, t, &text);
                    received.push((None, text, t));
                }
                for n in notices {
                    chat.show(&mut ui, crate::panels::chat::CHAT_LOG, &n);
                    received.push((None, n, crate::panels::chat::CHAT_LOG));
                }
                for l in session {
                    let shown = session_text(&l);
                    chat.show(&mut ui, l.chat_type, &shown);
                    received.push((
                        (!l.sender.is_empty()).then_some(l.sender),
                        l.text,
                        l.chat_type,
                    ));
                }
                for (t, text) in &from_plugins {
                    chat.show(&mut ui, *t, text);
                }
            }
            None => {
                for (t, text) in local {
                    received.push((None, text, t));
                }
                for n in notices {
                    received.push((None, n, crate::panels::chat::CHAT_LOG));
                }
                for l in session {
                    received.push((
                        (!l.sender.is_empty()).then_some(l.sender),
                        l.text,
                        l.chat_type,
                    ));
                }
            }
        }
        if had_lines || !received.is_empty() {
            self.frame.content_event = true;
        }
        let m = plugins.manager_mut();
        for (sender, text, chat_type) in received {
            m.post_chat_line(sender.as_deref(), &text, chat_type.max(1));
        }
    }

    // ----- step 8 and 10 -------------------------------------------------------------

    /// Step 8: layout and paint within the paint slice, the frame's GPU operations
    /// (applied to `renderer` when given) and the composition list.
    pub fn step8(&mut self, renderer: Option<&mut Renderer<'_>>, plugins: &mut Plugins) {
        let t = Instant::now();
        let now = Duration::from_secs_f64(self.clock.max(0.0));
        let frame = self
            .ui
            .borrow_mut()
            .paint(now, &mut TimeSlice::new(self.limits.paint_slice));
        let paint = t.elapsed();
        if let Some(r) = renderer {
            self.textures.apply(r, &frame.ops);
            if self.prefs.cursor == CursorMode::Software && self.cursor_tex.is_none() {
                let s = u32::try_from(self.ui.borrow().cursor_size().max(8)).unwrap_or(32);
                if let Ok(tex) = r.create_ui_texture(s, s, Some("ui-cursor")) {
                    let px = cursor::arrow_bgra(s);
                    let _ = r.update_texture_rect(tex, 0, 0, s, s, &px, s as usize * 4);
                    self.cursor_tex = Some(tex);
                }
            }
        }
        self.composition = frame.composition;
        self.caps = self.composition.counters;
        self.frame.paint += paint;
        self.frame.host += t.elapsed();
        let plugin = plugins.manager().take_plugin_time();
        let env = self
            .env
            .as_ref()
            .map(|h| h.borrow().stats())
            .unwrap_or_default();
        // The event window (U4): the trigger frame, then every frame until the UI is quiet
        // again (no band painted, nothing uploaded, no job in flight), at most
        // MAX_EVENT_WINDOW_FRAMES frames.
        let quiet = frame.stats.bands == 0 && frame.stats.upload_bytes == 0 && env.in_flight == 0;
        let event_window = if self.frame.event {
            self.event_window = MAX_EVENT_WINDOW_FRAMES.saturating_sub(1);
            true
        } else if self.event_window > 0 {
            if quiet {
                self.event_window = 0;
                false
            } else {
                self.event_window -= 1;
                true
            }
        } else {
            false
        };
        self.last = FrameSample {
            ui_ms: ms(self.frame.host.saturating_sub(plugin)),
            paint_ms: ms(self.frame.paint),
            step1_ms: ms(self.frame.step1),
            step7_ms: ms(self.frame.step7),
            plugin_ms: ms(plugin),
            frame_ms: 0.0,
            upload_bytes: frame.stats.upload_bytes,
            canvas_bytes: frame.stats.canvas_bytes,
            texture_bytes: frame.stats.texture_bytes,
            image_bytes: env.image_bytes,
            pinned_bytes: env.pinned_bytes,
            event: self.frame.event,
            event_window,
            content_event: self.frame.content_event,
            pointer_events: self.frame.pointer_events,
            entries: u32::try_from(self.composition.entries.len()).unwrap_or(u32::MAX),
            bands: frame.stats.bands,
        };
    }

    /// Step 10: the UI pass (after everything recorded so far, so after the walker's
    /// overlays) and, with `ui_cursor = software`, the software cursor as the last draw
    /// unless `cursor` is false (screenshots), the lock hides it or the pointer is outside.
    pub fn draw(&mut self, frame: &mut Frame, cursor: bool) {
        self.textures
            .record(frame, &self.composition, &self.caps, 0);
        if cursor
            && self.prefs.cursor == CursorMode::Software
            && !self.lock.software_hidden()
            && let (Some(tex), Some((x, y))) = (self.cursor_tex, self.ui.borrow().pointer())
        {
            frame.software_cursor(tex, [x, y]);
        }
    }

    /// After present: the frame clock advances (plugins' game time, the UI's hover clock)
    /// and the frame's sample is recorded. `frame_ms` is the frame's own time (pacing
    /// excluded).
    pub fn end_frame(&mut self, plugins: &Plugins, frame_ms: f64) {
        self.clock = self.started.elapsed().as_secs_f64();
        plugins.set_game_time(self.clock);
        self.ui.borrow_mut().set_time(self.clock);
        self.last.frame_ms = frame_ms;
        if let Some(r) = self.recorder.as_mut() {
            r.push(self.phase, self.last);
        }
        self.frame = FrameAccum::default();
        self.frames += 1;
    }

    /// At client exit, after the last frame: every apartment is unloaded (the only plugin
    /// code outside steps 1 and 7, decision 0014 D8), the skin is deactivated and the UI
    /// textures released.
    pub fn shutdown(&mut self, plugins: &mut Plugins, renderer: Option<&mut Renderer<'_>>) {
        let loaded = plugins.manager().loaded_plugins().len();
        plugins.manager_mut().unload_all();
        let _ = self.plugin_ui.deliver(plugins.manager());
        plugins.set_ui_binding(None);
        plugins.set_ui_driven(false);
        plugins.manager_mut().set_local_player_source(None);
        if let Some(sel) = self.skins.as_mut() {
            let sw = sel.deactivate();
            if let Some(h) = &self.env {
                h.borrow_mut().set_skin(sw.current.clone());
            }
            self.ui.borrow_mut().set_skin(sw.current);
        }
        if let Some(r) = renderer {
            self.textures.release(r);
            if let Some(t) = self.cursor_tex.take() {
                r.destroy_texture(t);
            }
            for (_, (_, tex, _)) in std::mem::take(&mut self.portraits) {
                r.destroy_texture(tex);
            }
        }
        tracing::info!(unloaded = loaded, frames = self.frames, "game UI shut down");
    }
}

/// The UI sink of one step-1 call.
struct Step1<'a> {
    host: &'a mut UiHost,
    plugins: &'a mut Plugins,
    window: &'a mut dyn CursorWindow,
}

impl UiSink for Step1<'_> {
    fn offer(&mut self, event: &OfferedEvent, requests: &mut BandRequests) -> Verdict {
        self.host.offer(event, requests, self.plugins, self.window)
    }
}

/// Whether an offered command is a wheel scroll step (`ScrollUp`, `ScrollDown` or the
/// analog `Scroll`, the names `tessera-input` offers for the wheel actions): when the UI
/// consumes one, the frame is a content-event frame (U4 (f)).
#[must_use]
pub fn is_wheel_step(command: &Command) -> bool {
    command.is_wheel()
}

/// The mapper's cursor position after a scale change by `ratio` (`keymap.md` §7.7:
/// unknown stays unknown; a non-finite or non-positive ratio keeps the position).
#[must_use]
pub fn rescaled_cursor(cursor: Option<(f64, f64)>, ratio: f64) -> Option<(f64, f64)> {
    let r = if ratio.is_finite() && ratio > 0.0 {
        ratio
    } else {
        1.0
    };
    cursor.map(|(x, y)| (x * r, y * r))
}

/// How a session line shows in the chat window: the text alone for a system line,
/// `Sender: text` otherwise (Tessera's own wording, `chat-window.md` §10 item 2).
#[must_use]
pub fn session_text(line: &crate::net::ChatLine) -> String {
    if line.sender.is_empty() {
        line.text.clone()
    } else {
        format!("{}: {}", line.sender, line.text)
    }
}

/// Parses a panel name for the command line: `P1`–`P15`, the panel's layout label or its
/// root label (`ui-layout.md` §17.2), ASCII case ignored.
#[must_use]
pub fn parse_panel(name: &str) -> Option<PanelId> {
    let n = name.trim();
    if let Some(num) = n
        .strip_prefix('P')
        .or_else(|| n.strip_prefix('p'))
        .and_then(|d| d.parse::<u8>().ok())
    {
        return PanelId::from_number(num);
    }
    PanelId::ALL.into_iter().find(|id| {
        id.layout_label().eq_ignore_ascii_case(n) || id.root_label().eq_ignore_ascii_case(n)
    })
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn wheel_commands_are_scroll_steps() {
        let c = |n: &str| Command {
            name: Arc::from(n),
            id: None,
        };
        // The names the mapper offers for `scroll_up` / `scroll_down` (crates/input
        // commands table); the old "Wheel" substring test matched none of them.
        assert!(is_wheel_step(&c("ScrollUp")));
        assert!(is_wheel_step(&c("ScrollDown")));
        assert!(!is_wheel_step(&c("SelectLeft")));
        let rows = tessera_input::commands::command_rows();
        for action in ["scroll_up", "scroll_down"] {
            let row = rows
                .iter()
                .find(|r| r.action == action)
                .expect("command row");
            assert!(
                is_wheel_step(&c(row.command)),
                "{action} offers {}",
                row.command
            );
        }
    }

    #[test]
    fn rescaled_cursor_keeps_unknown() {
        assert_eq!(rescaled_cursor(None, 2.0), None);
        assert_eq!(rescaled_cursor(Some((10.0, 4.0)), 1.5), Some((15.0, 6.0)));
        assert_eq!(
            rescaled_cursor(Some((10.0, 4.0)), f64::NAN),
            Some((10.0, 4.0))
        );
    }

    #[test]
    fn panel_names() {
        assert_eq!(parse_panel("P1"), PanelId::from_number(1));
        assert_eq!(parse_panel("p15"), PanelId::from_number(15));
        assert_eq!(parse_panel("P16"), None);
        let id = PanelId::from_number(7).expect("P7");
        assert_eq!(parse_panel(id.layout_label()), Some(id));
        assert_eq!(parse_panel(&id.root_label().to_ascii_uppercase()), Some(id));
        assert_eq!(parse_panel("nothing"), None);
    }
}
