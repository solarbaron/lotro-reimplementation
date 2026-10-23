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
//! over the asset workers), [`text`] (shaping), [`prefs`] (the `[UI]` keys of
//! `ui-scaling.md` §7.1 applied to `tessera-ui`), [`fonts`] (the §6.4.2 font ladders),
//! [`gpu`] (UI textures and the UI pass),
//! [`cursor`] (pointer lock), [`services`] (plugin adapters for the panels), [`stats`]
//! (U1–U5), [`script`] (the scripted run of 0015 §6).

pub mod boot;
pub mod client;
pub mod combat;
pub mod cursor;
pub mod env;
pub mod fonts;
pub mod gpu;
pub mod interface;
pub mod placements;
pub mod prefs;
pub mod run;
pub mod scaling;
pub mod script;
pub mod services;
pub mod settings_writer;
pub mod stats;
pub mod text;

use std::cell::{Ref, RefCell, RefMut};
use std::collections::{BTreeMap, BTreeSet, VecDeque};
use std::rc::Rc;
use std::sync::Arc;
use std::time::{Duration, Instant};

use tessera_input::{
    BandRequests, Button, ContextId, InputMapper, Modifiers as InputModifiers, OfferPhase,
    OfferedEvent, RawEvent, UiSink, Verdict, contexts,
};
use tessera_plugins::ui::{Modifiers, PluginUi};
use tessera_plugins::{GameplayEvent, LocalPlayerEvent};
use tessera_render::{Frame, Renderer, TextureId};
use tessera_settings::{CursorMode, UiPrefs};
use tessera_ui::BandChange;
use tessera_ui::bridge::{CommandOffer, Phase, UiInput};
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
use interface::InterfaceController;
use scaling::ScaleState;
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
/// Most key holds pressed while a modal engine dialog was shown whose releases are still to
/// be consumed (`ui-widgets.md` §18.4; Tessera bound: beyond it the oldest is forgotten and
/// its release reaches game code).
pub const MAX_MODAL_HOLDS: usize = 256;
/// Most keys and pad buttons pressed while a modal engine dialog was shown that are
/// remembered until their release (`ui-widgets.md` §18.4; Tessera bound: beyond it a new
/// press is not remembered and its release-class binding may reach game code).
pub const MAX_MODAL_KEYS: usize = 64;
/// The band owner of the host's own entry of the `text_entry` context while a modal engine
/// dialog is shown ([`UiHost::modal_dialog_shown`]); not an [`ElementKey::owner`] in
/// practice (generation and index both `u32::MAX`), and not the code owner 0, which
/// `InputMapper::activate` uses.
const MODAL_BAND_OWNER: u64 = u64::MAX;

/// The step of the per-window scale (`ui-scaling.md` §7.4: steps of 0.05).
pub const GROUP_SCALE_STEP: f32 = 0.05;

/// The Tessera limits of every per-window factor (`ui-scaling.md` §7.4: the group's data range
/// intersected with 0.5–2.0). The client does not read the data ranges yet
/// (`UIScaleDefinition_Array`, `ui-behaviour.md` §9.1), so only these limits apply unless a
/// caller hands ranges to [`UiHost::set_group_scale_ranges`].
pub const GROUP_SCALE_LIMITS: (f32, f32) = (0.5, 2.0);

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
    /// The four UI-scaling actions (`ui-scaling.md` §7.3), unbound by default, polled each
    /// frame ([`UiHost::poll_scale_actions`]).
    ui_scale_up: Option<tessera_input::ActionId>,
    ui_scale_down: Option<tessera_input::ActionId>,
    ui_scale_auto: Option<tessera_input::ActionId>,
    ui_interface_window: Option<tessera_input::ActionId>,
    /// The per-window scale gesture (`ui-scaling.md` §7.4): unbound by default, polled each
    /// frame ([`UiHost::poll_scale_actions`]); it scales the panel under the pointer.
    ui_window_scale_up: Option<tessera_input::ActionId>,
    ui_window_scale_down: Option<tessera_input::ActionId>,
    /// The crafting-panel toggles (`crafting-panels.md` §12): `toggle_crafting` (T) and
    /// `toggle_crafting_1` … `3`, polled each frame ([`UiHost::poll_p6_actions`]).
    toggle_crafting: Option<tessera_input::ActionId>,
    toggle_crafting_tabs: [Option<tessera_input::ActionId>; 3],
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
    /// Key holds pressed while a modal engine dialog was shown (`ui-widgets.md` §18.4):
    /// their repeats and releases are consumed too, even after the dialog closed (at most
    /// [`MAX_MODAL_HOLDS`]). The value says whether the hold's Start was offered to the UI
    /// (the keyboard target was inside the shown dialog), so its Repeats and End are too.
    modal_holds: BTreeMap<u64, bool>,
    /// Keys and pad buttons pressed (raw) while a modal engine dialog was shown, until
    /// their raw release (at most [`MAX_MODAL_KEYS`]): a hold such a button makes, also a
    /// release-class one made after the dialog closed, is consumed (§18.4).
    modal_keys: Vec<Button>,
    /// Whether the host's modal entry of the `text_entry` context is in the mapper's band
    /// ([`Self::sync_modal_band`]).
    modal_band: bool,
    player: Arc<WalkerPlayer>,
    /// The wallet views the plugins' local player carries (P3, [`Self::set_plugin_wallet`]).
    plugin_wallet: Option<Arc<crate::npc::SessionWallet>>,
    /// The party members' actor values for the plugins' party views (P4,
    /// [`Self::set_plugin_party`]).
    plugin_party: Option<Arc<dyn tessera_plugins::PartyActorSource>>,
    /// The mount views the plugins' local player carries (P5, [`Self::set_plugin_mount`]).
    plugin_mount: Option<Arc<dyn tessera_plugins::MountSource>>,
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
    /// Where [`Self::end_frame`] takes the frame clock from ([`FrameClock`]).
    frame_clock: FrameClock,
    frame: FrameAccum,
    last: FrameSample,
    recorder: Option<Recorder>,
    phase: &'static str,
    cursor_tex: Option<TextureId>,
    /// The side of [`Self::cursor_tex`] in physical pixels (`ui-scaling.md` §6.7): the
    /// texture is built again when `Ui::cursor_physical_size` no longer matches it (a change
    /// of *Se* or of `ui_scale_cursor`).
    cursor_side: u32,
    /// The hot spot of [`Self::cursor_tex`], physical pixels (§6.7).
    cursor_hotspot: (i32, i32),
    /// The pointer's last physical position offered to the UI (`None` after it left), where
    /// the software cursor is drawn (render.md §6.11).
    pointer_physical: Option<(f64, f64)>,
    /// Builds the font ladders of `ui-scaling.md` §6.4.2 once the enum's records have loaded
    /// on the asset workers ([`fonts`]); `None` without an install or once registered.
    ladders: Option<fonts::LadderLoader>,
    frames: u64,
    command_ids: Vec<(String, u32)>,
    /// The command IDs of toggle type 3 (AutoRepeat) in table A of the default action map
    /// (`keymap.md` §7.8.6, §12.10), handed to the mapper in [`Self::attach_mapper`] so key
    /// repeat follows the install's action map. `None` without action-map data (an empty
    /// table A from boot): the actions' own `repeat` flags decide then.
    repeat_commands: Option<Vec<u32>>,
    /// The scale definitions' ranges by scaling group (`ui-scaling.md` §7.4,
    /// [`Self::set_group_scale_ranges`]); empty = 0.5–2.0 for every group.
    group_ranges: BTreeMap<u32, (f32, f32)>,
    /// Whether `[UI.group_scale]` changed since [`Self::take_group_scale_persist`].
    group_scales_dirty: bool,
    /// The root panels [`Self::apply_group_scales`] has seen; a root not in it was opened since
    /// and gets its saved placement again once its factor is set (`ui-scaling.md` §5.8).
    group_seen: BTreeSet<ElementKey>,
    /// Group values not yet told to the plugin layer (`ui-scaling.md` §7.7 "Group values",
    /// `lua-plugin-api.md` §13.21 rule 8), sent at the next delivery.
    plugin_group_scales: Vec<(u32, f32)>,
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
    /// The social panels (decision 0024 P4), with the install.
    social: Option<crate::social::SocialUi>,
    /// Instant-message lines of session tells waiting for [`Self::social_frame`] (CT6).
    pending_im: Vec<tessera_ui::game::chat::social::ImLine>,
    /// The tell-target generation the chat window last copied (CT3).
    tells_generation: Option<u64>,
    /// Every UI event since the last [`Self::take_game_events`] (at most
    /// [`MAX_CHARACTER_QUEUE`]; the windowed client takes them every frame), for the
    /// in-world windows the runtime drives after step 7: the NPC windows, the quest journal
    /// and the social panels read the same list, so none takes events from another. The
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
    /// The UI-scaling state (decision 0040, `ui-scaling.md` §4): resolves the effective
    /// scale, the UI screen *F* and the safe area *A*, and converts pointers ([`scaling`]).
    /// The `tessera-ui` screen root is sized in UI units (*F*), never physical pixels (§3.2).
    scale: ScaleState,
    /// Whether `/uiscale` or the `ui_interface_window` action asked for the Interface window
    /// (`ui-scaling.md` §7.2): the frame loop takes it with [`Self::take_interface_request`].
    want_interface: bool,
    /// Tessera's own Interface window while it is open (`ui-scaling.md` §7.2).
    interface: Option<InterfaceController>,
    /// The optional-row settings a window action changed and that wait to be written
    /// (`ui-scaling.md` §4.4, §7.1): art filter, text size and the two resets. `None` when
    /// nothing is pending. Scale and safe aspect persist through [`Self::take_scale_persist`].
    interface_persist: Option<interface::ExtraPersist>,
    /// The progression views the plugins' local player carries (P6,
    /// [`Self::set_plugin_progression`]).
    plugin_progression: Option<Arc<dyn tessera_plugins::ProgressionSource>>,
    /// A user move or resize changed the saved placements since the last
    /// [`Self::take_placement_persist`] (`ui-scaling.md` §5.8 "Saving").
    placements_dirty: bool,
    /// Whether saved placements are handed to the settings writer (off in scripted runs,
    /// whose drags are not the user's).
    persist_placements: bool,
    /// The saved roots whose placement was applied since they were attached
    /// (`ui-scaling.md` §5.8 "Applying": when the panel is attached), by the host or by the
    /// code that opened them.
    placed: std::collections::HashSet<ElementKey>,
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
        // Decision 0040 (`ui-scaling.md` §3.2, §4): the UI screen root is sized in UI units
        // (*F*), derived from the window's physical size and the UI scale, not in physical
        // pixels. σ = 1.0 here; the windowed client updates it from the monitor
        // ([`Self::set_os_scale`]). At *Se* = 1 (1024 × 768, 1920 × 1080 …) *F* equals the
        // physical size, so nothing changes (invariant I2).
        let scale = ScaleState::new(
            options.prefs.ui_scale,
            options.prefs.safe_aspect,
            (
                i32::try_from(w).unwrap_or(i32::MAX),
                i32::try_from(h).unwrap_or(i32::MAX),
            ),
            1.0,
        );
        let mut ui = Ui::new(env, vocab, scale.ui_size());
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
        // `ui-scaling.md` §7.1: the art filter, text factor and text source of `[UI]`, the
        // derived-glyph cache's S6 share, and the shaper's layout parameters for the crisp
        // text path (§6.4.5; without them the §7.5 text factor is not applied). At *Se* = 1
        // with the defaults nothing is laid out or drawn differently (invariant I7).
        prefs::apply_text_and_art(&mut ui, &options.prefs);
        // §8 S6: one unpinned pool for the decoded-image LRU and the glyph cache.
        let s6 = prefs::s6_split(
            options.prefs.image_lru_mib.saturating_mul(1 << 20),
            scale.effective_scale(),
        );
        ui.set_glyph_cache_budget(s6.glyph);
        if let Some(h) = &handle {
            h.borrow_mut().set_lru_budget(s6.image_lru);
            let st = h.borrow();
            let leading = st.data().map_or(0.0, |d| d.locale_leading);
            ui.set_text_layout_params(Some(prefs::text_params_reader(st.text_pids(), leading)));
        }
        // Decision 0040 (`ui-scaling.md` §3.2, §4.5 step 1): make the UI screen *F* and the
        // safe area *A* live from the first frame, so surfaces map to their physical box
        // (§3.6, §6.6) and roots fit by the §5 rules. The screen root is already *F*-sized
        // (given to [`Ui::new`] above), so this stores *F*/*A* without changing its size. At
        // *Se* = 1 with *A* = *F* every mapping is the identity, so nothing changes (I2).
        ui.set_ui_screen(*scale.screen(), scale.safe_area());
        // The saved placements of `[UI.panels]` (`ui-scaling.md` §5.8): every panel opened
        // from now on is placed by them ([`placements`]).
        ui.set_saved_store(placements::store_from_prefs(&options.prefs.panels));
        let ui = Rc::new(RefCell::new(ui));
        // §6.4.2 ladders need the install's font records.
        let ladders = handle
            .as_ref()
            .filter(|h| h.borrow().data().is_some())
            .map(|_| fonts::LadderLoader::default());
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
            modal_holds: BTreeMap::new(),
            modal_keys: Vec::new(),
            modal_band: false,
            player: WalkerPlayer::new(),
            plugin_wallet: None,
            plugin_party: None,
            plugin_mount: None,
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
            frame_clock: FrameClock::Wall,
            frame: FrameAccum::default(),
            last: FrameSample::default(),
            recorder: options.record.then(Recorder::default),
            phase: "run",
            cursor_tex: None,
            cursor_side: 0,
            cursor_hotspot: cursor::ARROW_HOTSPOT,
            pointer_physical: None,
            ladders,
            frames: 0,
            command_ids: Vec::new(),
            repeat_commands: None,
            group_ranges: BTreeMap::new(),
            group_scales_dirty: false,
            group_seen: BTreeSet::new(),
            plugin_group_scales: Vec::new(),
            character: None,
            character_events: Vec::new(),
            character_commands: Vec::new(),
            chargen: None,
            screen_events: None,
            game_events: Vec::new(),
            portraits: std::collections::BTreeMap::new(),
            portrait_failed: None,
            social: None,
            pending_im: Vec::new(),
            tells_generation: None,
            combat: combat::HostCombat::default(),
            scale,
            want_interface: false,
            interface: None,
            interface_persist: None,
            plugin_progression: None,
            placements_dirty: false,
            persist_placements: true,
            placed: std::collections::HashSet::new(),
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
        // The UI's Impulse test reads the action map's toggle types by command ID
        // (`lua-plugin-api.md` §13.12.1, decision 0014 D9). Without that table every type
        // is 0, so the tap pulses then give plugin button events (D9, Known difference 12).
        host.ui
            .borrow_mut()
            .set_toggle_types(b.toggle_types.clone());
        b.char_vocab.register_classes(&mut host.ui.borrow_mut());
        // The frames' name and level displays (P4, P5) draw text (`combat-ui.md` §4.1, §5.1).
        b.combat.vocab.register_classes(&mut host.ui.borrow_mut());
        host.character = Some(crate::character::panel::CharacterUi::new(
            b.char_vocab.clone(),
            b.char_ids.clone(),
        ));
        host.chargen = Some(b.chargen);
        host.social = Some(crate::social::SocialUi::new(
            b.social_vocab.clone(),
            b.char_vocab.clone(),
        ));
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
        // `keymap.md` §7.8.6: with action-map data a hold repeats exactly when its command has
        // toggle type 3 in table A. Boot leaves the table empty when the record is missing or
        // does not decode; then no set is handed over and the actions' flags decide.
        host.set_toggle_types(&b.toggle_types);
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

    /// Chooses where the frame clock comes from from the next [`Self::end_frame`] on
    /// (tests and captures, decision 0031: a picture of an animated part, such as the
    /// loading screen's ring, is stable only at a fixed animation time). The clock goes on
    /// from its current value; a stepped clock's step is held to 0–1 s (a non-finite step
    /// holds it).
    pub fn set_frame_clock(&mut self, clock: FrameClock) {
        self.frame_clock = match clock {
            FrameClock::Stepped { step } if step.is_finite() => FrameClock::Stepped {
                step: step.clamp(0.0, 1.0),
            },
            FrameClock::Stepped { .. } => FrameClock::Held,
            other => other,
        };
    }

    /// Where the frame clock comes from.
    #[must_use]
    pub fn frame_clock(&self) -> FrameClock {
        self.frame_clock
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
    /// of [`Self::set_plugin_wallet`], when given), the party views read their members'
    /// values from [`Self::set_plugin_party`]'s source, and `load` is queued. This and
    /// [`Self::shutdown`] are the only writers of the plugins' local player source (and of
    /// the party views' actor source) in the windowed client.
    pub fn attach_plugins(&mut self, plugins: &mut Plugins, load: &[String]) {
        plugins.set_ui_binding(Some(self.plugin_ui.binding()));
        // `ui-scaling.md` §7.7: the plugin layer starts with every saved group value.
        let groups: Vec<u32> = self.prefs.group_scale.keys().copied().collect();
        for g in groups {
            let p = self.group_scale_of(g);
            self.queue_plugin_group_scale(g, p);
        }
        plugins.set_ui_driven(true);
        if let Some(h) = &self.env {
            h.borrow_mut()
                .set_plugins_dir(Some(plugins.manager().plugins_dir().to_path_buf()));
        }
        let src: Arc<dyn tessera_plugins::LocalPlayerSource> = match &self.plugin_wallet {
            Some(w) => crate::npc::wallet::WalletPlayer::new(self.player.clone(), Arc::clone(w)),
            None => self.player.clone(),
        };
        // The mount views (P5) wrap that source, keeping its other views
        // ([`crate::travel::mount::MountPlayer`]).
        let src = match &self.plugin_mount {
            Some(m) => crate::travel::mount::MountPlayer::new(src, Arc::clone(m))
                as Arc<dyn tessera_plugins::LocalPlayerSource>,
            None => src,
        };
        // The progression views (P6) wrap that source in turn, keeping its other views
        // ([`crate::progression::plugin::ProgressionPlayer`]).
        let src = match &self.plugin_progression {
            Some(p) => crate::progression::plugin::ProgressionPlayer::new(src, Arc::clone(p))
                as Arc<dyn tessera_plugins::LocalPlayerSource>,
            None => src,
        };
        plugins.manager_mut().set_local_player_source(Some(src));
        if let Some(p) = &self.plugin_party {
            plugins
                .manager_mut()
                .set_party_actor_source(Some(Arc::clone(p)));
        }
        for name in load {
            if plugins.manager_mut().queue_load(name) {
                tracing::info!(plugin = %name, "plugin load queued");
            } else {
                tracing::warn!(plugin = %name, "plugin not loaded: no available plugin of that name");
            }
        }
    }

    /// Takes table A of the default action map (command ID → toggle type, `keymap.md`
    /// §12.10) for the mapper's repeat rule (§7.8.6): the command IDs of type 3 (AutoRepeat)
    /// are handed to the mapper at the next [`Self::attach_mapper`]. An empty table means no
    /// action-map data, and no set is handed over (the actions' `repeat` flags decide).
    pub fn set_toggle_types(&mut self, types: &BTreeMap<u32, u8>) {
        self.repeat_commands = (!types.is_empty()).then(|| {
            types
                .iter()
                .filter(|(_, t)| **t == 3)
                .map(|(c, _)| *c)
                .collect()
        });
    }

    /// Attaches the input mapper (decision 0014): the `pointer` context goes on, the
    /// command IDs from the install are handed over, and the UI band's contexts are
    /// looked up. With action-map data and command IDs the type-3 command set of
    /// [`Self::set_toggle_types`] goes to [`InputMapper::set_repeat_commands`] (`keymap.md`
    /// §7.8.6); otherwise the mapper gets no set.
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
        // The four UI-scaling actions (`ui-scaling.md` §7.3), registered unbound in the
        // input crate's default registry.
        ids.ui_scale_up = reg.action("ui_scale_up");
        ids.ui_scale_down = reg.action("ui_scale_down");
        ids.ui_scale_auto = reg.action("ui_scale_auto");
        ids.ui_interface_window = reg.action("ui_interface_window");
        // The per-window scale actions (`ui-scaling.md` §7.4), registered unbound too.
        ids.ui_window_scale_up = reg.action("ui_window_scale_up");
        ids.ui_window_scale_down = reg.action("ui_window_scale_down");
        ids.toggle_crafting = reg.action("toggle_crafting");
        ids.toggle_crafting_tabs = [
            reg.action("toggle_crafting_1"),
            reg.action("toggle_crafting_2"),
            reg.action("toggle_crafting_3"),
        ];
        let pointer = reg.context(contexts::POINTER);
        if let Some(p) = pointer {
            mapper.activate(p);
        }
        if !self.command_ids.is_empty() {
            mapper.set_command_ids(self.command_ids.iter().map(|(n, i)| (n.as_str(), *i)));
        }
        // `keymap.md` §7.8.6: the type-3 set of the action map decides repeat per hold. It
        // names commands by their IDs, so it is handed over only together with the command-ID
        // table; without either, the actions' `repeat` flags decide (and a set an earlier
        // attach handed over is withdrawn).
        match &self.repeat_commands {
            Some(ids) if !self.command_ids.is_empty() => {
                mapper.set_repeat_commands(Some(ids.iter().copied()));
            }
            other => {
                if other.is_some() {
                    tracing::info!(
                        "action map has repeat data but no command IDs; key repeat follows the actions' flags"
                    );
                }
                mapper.set_repeat_commands(None::<Vec<u32>>);
            }
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
        if id == PanelId::Social
            && let Some(s) = self.social.as_mut()
        {
            // P8 is the social panel of P4 (`social-panels.md` §3): built and driven by
            // the social panels, so it is never opened twice.
            s.request_open(crate::social::ui::SocialOpen::Social);
            let mut ui = self.ui.borrow_mut();
            s.ensure_open(&mut ui);
            return s
                .panel()
                .map(|p| p.root)
                .ok_or(PanelsError::NotInCatalogue(id));
        }
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
        // No placement is recorded at a close: `ui-scaling.md` §5.8 saves at the end of a
        // user move or resize only.
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
        let social = if press && !repeat {
            match self.social.as_mut() {
                Some(s) => s.escape(&mut self.ui.borrow_mut()),
                None => None,
            }
        } else {
            None
        };
        if let Some(root) = social {
            return Some(EscapeOutcome::HidActive(root));
        }
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
                // The frozen position is a physical pixel; the UI's lock point is in UI units
                // (`ui-scaling.md` §3.6). At Se = 1 this is the identity.
                let (ux, uy) = self
                    .scale
                    .pointer_to_ui(f.0.floor() as i32, f.1.floor() as i32);
                self.ui.borrow_mut().set_pointer_lock(Some((ux, uy)));
                if self.pointer_physical.is_some() {
                    self.pointer_physical = Some(f);
                }
                Some(f)
            }
            None => {
                // The software cursor is drawn at the physical pointer (render.md §6.11): it
                // moves with the mapper's cursor, not only at the next pointer move.
                self.pointer_physical = rescaled_cursor(self.pointer_physical, ratio);
                rescaled_cursor(mapper.cursor(), ratio)
            }
        };
        mapper.rescale(timing, cursor);
    }

    /// The pointer's last physical position offered to the UI, rescaled by
    /// [`Self::rescale`] (`None` after it left): where the software cursor is drawn.
    #[must_use]
    pub fn pointer_physical(&self) -> Option<(f64, f64)> {
        self.pointer_physical
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

    /// The source of the party members' `Actor` values (`lua-plugin-api.md` §15.4) the
    /// plugins' party views read from the next [`Self::attach_plugins`] on: the runtime's
    /// party bridge (P4, [`crate::social::party::PartyBridge::actor_source`]).
    pub fn set_plugin_party(&mut self, source: Arc<dyn tessera_plugins::PartyActorSource>) {
        self.plugin_party = Some(source);
    }

    /// The source of the mount views (`lua-plugin-api.md` §15.5) the plugins' local player
    /// carries from the next [`Self::attach_plugins`] on: the travel hub's session mount
    /// (P5, [`crate::travel::mount::SessionMount::source`]).
    pub fn set_plugin_mount(&mut self, source: Arc<dyn tessera_plugins::MountSource>) {
        self.plugin_mount = Some(source);
    }

    /// The source of the progression and crafting views (`lua-plugin-api.md` §15.6) the
    /// plugins' local player carries from the next [`Self::attach_plugins`] on: the P6
    /// hub's session model (P6, [`crate::progression::P6Hub::plugin_source`]).
    pub fn set_plugin_progression(&mut self, source: Arc<dyn tessera_plugins::ProgressionSource>) {
        self.plugin_progression = Some(source);
    }

    // ----- saved placements (`ui-scaling.md` §5.8) -------------------------------------

    /// Records the saved placement of `element` after a user move or resize
    /// (`ui-scaling.md` §5.8 "Saving"; the host calls it on `UiEvent::LayoutChanged`, never
    /// at a close). An element without save properties is ignored.
    pub fn record_placement(&mut self, element: ElementKey) {
        let mut ui = self.ui.borrow_mut();
        if !ui.is_saved_element(element) {
            return;
        }
        ui.record_saved(element);
        self.placements_dirty = true;
    }

    /// Whether user moves are saved (off for scripted runs).
    pub fn set_persist_placements(&mut self, on: bool) {
        self.persist_placements = on;
    }

    /// The saved placements as `[UI.panels]`, once after they changed (the frame loop hands
    /// them to the settings writer, off the frame thread).
    pub fn take_placement_persist(
        &mut self,
    ) -> Option<std::collections::BTreeMap<String, tessera_settings::PanelPlacement>> {
        if !std::mem::take(&mut self.placements_dirty) || !self.persist_placements {
            return None;
        }
        let map = placements::prefs_from_store(self.ui.borrow().saved_store());
        self.prefs.panels.clone_from(&map);
        Some(map)
    }

    /// "Reset panel positions" (`ui-scaling.md` §7.1, §7.2 row 7): every saved placement
    /// goes, from the in-memory store as well as `[UI.panels]` (written by the next
    /// [`Self::take_interface_persist`]), and the roots are laid out again.
    pub fn reset_panel_positions(&mut self) {
        self.prefs.panels.clear();
        {
            let mut ui = self.ui.borrow_mut();
            ui.set_saved_store(tessera_ui::SavedStore::default());
            // "Lays the roots out again" (§7.1): every saved root goes back to the position
            // its layout gives it at the current screen (its design position, then the
            // ordinary run-time layout against the screen, `ui-layout.md` §8.4).
            let roots = ui.children(ui.screen_root());
            for k in roots {
                if !ui.is_saved_element(k) {
                    continue;
                }
                if let Some(d) = ui.design_box(k) {
                    ui.move_to(k, d.l, d.t);
                }
                ui.set_reference_box(k, None);
            }
        }
        self.placements_dirty = false;
        let p = self.note_interface_persist();
        p.clear_panels = true;
        self.frame.event = true;
        self.feed_screen();
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

    /// A new window size in **physical** pixels (applied at the start of step 7, decision
    /// 0014 Amendment 1 item 2). The UI scale recomputes the UI screen *F* and the safe area
    /// (`ui-scaling.md` §4.6); the `tessera-ui` screen root is sized in UI units (§3.2). A
    /// size with a zero side changes nothing (§3.2).
    pub fn resize(&mut self, width: u32, height: u32) {
        self.frame.event = true;
        let phys = (
            i32::try_from(width).unwrap_or(i32::MAX),
            i32::try_from(height).unwrap_or(i32::MAX),
        );
        if self.scale.set_physical(phys, Instant::now()).screen_changed {
            self.feed_screen();
        }
    }

    /// A change of the operating system's scale factor σ for the window's monitor (the window
    /// moved to another monitor, `ui-scaling.md` §4.2, §4.6): with the automatic scale *F*
    /// is re-evaluated. The windowed client calls this on `ScaleFactorChanged`.
    pub fn set_os_scale(&mut self, sigma: f64) {
        if self.scale.set_os_scale(sigma).screen_changed {
            self.frame.event = true;
            self.feed_screen();
        }
    }

    /// Feeds the current UI screen *F* and safe area *A* into `tessera-ui` live
    /// (`ui-scaling.md` §4.5). [`Ui::set_ui_screen`] stores *F*/*A* for the §3.6 physical
    /// mapping and resizes the screen root to *F*'s UI-unit size, so every root is laid out
    /// again by the §5 fit rules (step 2); [`Ui::invalidate_paint`] then asks for a full
    /// repaint so every surface re-paints crisply at the new content scale (steps 2-4). At
    /// *Se* = 1 with *A* = *F* the mappings are the identity and the root size is unchanged, so
    /// the result stays byte-identical (invariant I2); only the repaint is extra work.
    fn feed_screen(&mut self) {
        let screen = *self.scale.screen();
        let safe = self.scale.safe_area();
        {
            let mut ui = self.ui.borrow_mut();
            ui.set_ui_screen(screen, safe);
            // §8 S6: the pool of the image LRU and the glyph cache grows with Se².
            let s6 = prefs::s6_split(
                self.prefs.image_lru_mib.saturating_mul(1 << 20),
                self.scale.effective_scale(),
            );
            ui.set_glyph_cache_budget(s6.glyph);
            if let Some(h) = &self.env {
                h.borrow_mut().set_lru_budget(s6.image_lru);
            }
            ui.invalidate_paint();
        }
        // §5.8 "Applying": again after every change of F or A.
        self.apply_saved_placements(true);
    }

    /// Applies the saved placements of `[UI.panels]` (`ui-scaling.md` §5.8 "Applying") to
    /// the screen's saved roots: those attached since the last call (`all` false), or every
    /// one (`all` true, after a change of *F* or *A*). `tessera-ui` applies none on its own
    /// (no caller of `Ui::apply_saved` in the crate), so the host does it once per frame and
    /// after each screen change. Cheap: the roots are the screen root's children.
    ///
    /// Roots whose opener already applied their placement when it created them (the native
    /// panels, the combat roots, the character panel and bag windows) are not applied again
    /// on attachment, so each attachment posts UI message 0x2E once. The re-application
    /// after a change of *F* or *A* keeps each root's current visibility (Tessera reading:
    /// the user's open or closed panel stays so; only the position and size follow the
    /// saved placement).
    pub fn apply_saved_placements(&mut self, all: bool) {
        let self_placed = self.self_placed_roots();
        let mut ui = self.ui.borrow_mut();
        let roots = ui.children(ui.screen_root());
        self.placed.retain(|k| ui.exists(*k));
        for k in roots {
            if !ui.is_saved_element(k) {
                continue;
            }
            if !all {
                if self.placed.contains(&k) {
                    continue;
                }
                self.placed.insert(k);
                if !self_placed.contains(&k) {
                    ui.apply_saved(k);
                }
                continue;
            }
            let shown = ui.is_visible(k);
            ui.apply_saved(k);
            if ui.is_visible(k) != shown {
                ui.set_shown(k, shown);
            }
            self.placed.insert(k);
        }
    }

    /// The roots whose opener applies the saved placement itself when it creates them
    /// (`Ui::apply_saved` right after `Ui::open_panel`): the native panels' roots and the
    /// character panel and bag windows. (The combat roots are marked placed when the host
    /// opens them, `open_combat_panels`.)
    fn self_placed_roots(&self) -> std::collections::HashSet<ElementKey> {
        let mut out = std::collections::HashSet::new();
        if let Some(p) = self.panels.as_ref() {
            out.extend(PanelId::ALL.iter().filter_map(|id| p.root(*id)));
        }
        if let Some(c) = self.character.as_ref() {
            out.extend(c.placed_roots());
        }
        out
    }

    /// The UI-scaling state (decision 0040, `ui-scaling.md` §4): the effective scale, the UI
    /// screen *F*, the safe area *A* and the pointer conversion.
    #[must_use]
    pub fn scale(&self) -> &ScaleState {
        &self.scale
    }

    /// Applies one `/uiscale` request (`ui-scaling.md` §7.3; [`crate::commands::uiscale`]):
    /// opens the Interface window, or changes the scale or the safe aspect live (§4.5). Call
    /// from the frame loop with the frame time.
    pub fn apply_uiscale_request(
        &mut self,
        req: &crate::commands::uiscale::UiScaleRequest,
        now: Instant,
    ) {
        use crate::commands::uiscale::UiScaleRequest as R;
        let applied = match req {
            R::Open => {
                self.want_interface = true;
                return;
            }
            R::Auto => self.scale.set_auto(now),
            R::Set(v) => self.scale.set_scale(*v, now),
            R::Step(d) => self.scale.step_scale(*d, now),
            R::Aspect(a) => self.scale.set_safe_aspect(*a, now),
        };
        if applied.screen_changed {
            self.frame.event = true;
            self.feed_screen();
        }
    }

    /// The per-frame UI-scaling tick: applies the automatic default once the window size has
    /// settled (`ui-scaling.md` §4.2). Call once per frame with the frame time.
    pub fn scale_tick(&mut self, now: Instant) {
        if self.scale.tick(now).screen_changed {
            self.frame.event = true;
            self.feed_screen();
        }
    }

    /// Takes the settled UI-scale change for the settings writer, once the settle elapsed
    /// (`ui-scaling.md` §4.5 step 5). The frame loop writes `ui_scale` and `ui_safe_aspect`.
    pub fn take_scale_persist(
        &mut self,
        now: Instant,
    ) -> Option<(tessera_settings::UiScale, tessera_settings::SafeAspect)> {
        self.scale.take_persist(now)
    }

    /// Takes a pending request to open the Interface window (`ui-scaling.md` §7.2), set by
    /// `/uiscale` with no argument or the `ui_interface_window` action.
    pub fn take_interface_request(&mut self) -> bool {
        std::mem::take(&mut self.want_interface)
    }

    /// Opens Tessera's own Interface window (`ui-scaling.md` §7.2), or closes it when it is
    /// already open (the command and the action toggle it). Built from the current scale and
    /// settings with neutral fallback art.
    pub fn open_interface(&mut self) {
        self.frame.event = true;
        if let Some(mut w) = self.interface.take() {
            // Toggle closed.
            let mut ui = self.ui.borrow_mut();
            w.close(&mut ui);
            return;
        }
        let mut ui = self.ui.borrow_mut();
        match InterfaceController::open(&mut ui, &self.scale, &self.prefs) {
            Ok(w) => self.interface = Some(w),
            Err(e) => tracing::warn!("Interface window not opened: {e}"),
        }
    }

    /// Whether the Interface window is open.
    #[must_use]
    pub fn interface_open(&self) -> bool {
        self.interface.is_some()
    }

    /// The open Interface window's controller (tests drive its sliders).
    #[must_use]
    pub fn interface_window(&self) -> Option<&InterfaceController> {
        self.interface.as_ref()
    }

    /// Feeds this frame's UI events to the open Interface window and applies its actions live
    /// (`ui-scaling.md` §4.5, §7.1-§7.6): the scale slider and Auto set *S*, the safe-aspect
    /// buttons set `ui_safe_aspect`, the optional controls set the art filter and text size
    /// and clear the saved placements or window scales. The close box closes the window. The
    /// events are the one per-frame list the in-world windows also read; the window owns only
    /// its own elements' events, so the others ignore them.
    pub fn interface_frame(&mut self, events: &[UiEvent], now: Instant) {
        let Some(mut w) = self.interface.take() else {
            return;
        };
        let mut touched_scale = false;
        let mut closed = false;
        for ev in events {
            let action = {
                let ui = self.ui.borrow();
                w.handle_event(&ui, ev)
            };
            let Some(action) = action else { continue };
            match self.apply_interface_action(action, now) {
                InterfaceApplied::Close => {
                    closed = true;
                    break;
                }
                InterfaceApplied::Scale => touched_scale = true,
                InterfaceApplied::Other => {}
            }
        }
        if touched_scale {
            self.frame.event = true;
            self.feed_screen();
        }
        if closed {
            let mut ui = self.ui.borrow_mut();
            w.close(&mut ui);
            return;
        }
        {
            let mut ui = self.ui.borrow_mut();
            w.refresh(&mut ui, &self.scale, &self.prefs);
        }
        self.interface = Some(w);
    }

    /// Applies one Interface-window action live (`ui-scaling.md` §7.1–§7.6), as
    /// [`Self::interface_frame`] does for the window's events: a scale or safe-aspect change
    /// is stored in the scaling state (the caller feeds the new screen), the text factor and
    /// art filter reach the UI at once, the resets clear what they name. Public for tests that
    /// drive an action without the window's widgets.
    pub fn apply_interface_action(
        &mut self,
        action: tessera_ui::game::interface::InterfaceAction,
        now: Instant,
    ) -> InterfaceApplied {
        use interface::{choice_to_art, choice_to_aspect, scale_of_stop, text_of_stop};
        use tessera_ui::game::interface::InterfaceAction as A;
        match action {
            A::SetScaleStop(i) => {
                self.scale.set_scale(scale_of_stop(i), now);
                InterfaceApplied::Scale
            }
            A::SetAuto(true) => {
                self.scale.set_auto(now);
                InterfaceApplied::Scale
            }
            A::SetAuto(false) => {
                // Leave auto for the value the slider currently shows (§7.2 row 3).
                let s = self.scale.resolved_scale() as f32;
                self.scale.set_scale(s, now);
                InterfaceApplied::Scale
            }
            A::SetAspect(c) => {
                self.scale.set_safe_aspect(choice_to_aspect(c), now);
                InterfaceApplied::Scale
            }
            A::SetTextStop(i) => {
                self.prefs.text_scale = text_of_stop(i);
                self.note_interface_persist();
                // §7.5 live: the text factor lays the reflowing texts out again.
                prefs::apply_text_and_art(&mut self.ui.borrow_mut(), &self.prefs);
                InterfaceApplied::Other
            }
            A::SetArt(c) => {
                self.prefs.art_filter = choice_to_art(c);
                self.note_interface_persist();
                // §7.6 live: every surface repaints with the new filter.
                prefs::apply_text_and_art(&mut self.ui.borrow_mut(), &self.prefs);
                InterfaceApplied::Other
            }
            A::ResetPanels => {
                self.reset_panel_positions();
                InterfaceApplied::Other
            }
            A::ResetWindowScales => {
                let cleared: Vec<u32> = self.prefs.group_scale.keys().copied().collect();
                for g in cleared {
                    self.queue_plugin_group_scale(g, 1.0);
                }
                self.prefs.group_scale.clear();
                // Every panel of a group goes back to 1.0 at once (§7.2 row 10, §7.4).
                self.apply_group_scales();
                let p = self.note_interface_persist();
                p.clear_groups = true;
                InterfaceApplied::Other
            }
            A::Close => InterfaceApplied::Close,
        }
    }

    /// Seeds (if needed) and returns the pending optional-row persist snapshot from the
    /// current preferences (`ui-scaling.md` §7.1).
    fn note_interface_persist(&mut self) -> &mut interface::ExtraPersist {
        self.interface_persist
            .get_or_insert(interface::ExtraPersist {
                art: self.prefs.art_filter,
                text_scale: self.prefs.text_scale,
                clear_panels: false,
                clear_groups: false,
            });
        // Always reflect the latest art/text in the snapshot.
        let p = self.interface_persist.as_mut().expect("just inserted");
        p.art = self.prefs.art_filter;
        p.text_scale = self.prefs.text_scale;
        p
    }

    /// Takes the pending optional-row persist (art filter, text size, the two resets), once
    /// (`ui-scaling.md` §4.4, §7.1). The frame loop writes them off the main thread.
    pub fn take_interface_persist(&mut self) -> Option<interface::ExtraPersist> {
        self.interface_persist.take()
    }

    /// Applies the four UI-scaling actions pressed this frame (`ui-scaling.md` §7.3):
    /// `ui_scale_up`/`_down` step by 0.05, `ui_scale_auto` sets auto, `ui_interface_window`
    /// opens the Interface window. The actions are unbound by default (the Interface window
    /// and `/uiscale` are the usual way in), so this does nothing until the user binds them.
    pub fn poll_scale_actions(&mut self, mapper: &InputMapper, now: Instant) {
        let pressed =
            |id: Option<tessera_input::ActionId>| id.is_some_and(|i| mapper.just_pressed(i));
        if pressed(self.ids.ui_scale_up) && self.scale.step_scale(0.05, now).screen_changed {
            self.frame.event = true;
            self.feed_screen();
        }
        if pressed(self.ids.ui_scale_down) && self.scale.step_scale(-0.05, now).screen_changed {
            self.frame.event = true;
            self.feed_screen();
        }
        if pressed(self.ids.ui_scale_auto) && self.scale.set_auto(now).screen_changed {
            self.frame.event = true;
            self.feed_screen();
        }
        if pressed(self.ids.ui_interface_window) {
            self.want_interface = true;
        }
        if pressed(self.ids.ui_window_scale_up) {
            self.step_window_scale(GROUP_SCALE_STEP);
        }
        if pressed(self.ids.ui_window_scale_down) {
            self.step_window_scale(-GROUP_SCALE_STEP);
        }
    }

    /// The per-window scale gesture (`ui-scaling.md` §7.4) over the panel under the pointer:
    /// the panel's scaling group *g* (`UIComponent_UIScaler_Group`) takes the factor *p* ±
    /// 0.05, on the 0.05 grid, within the group's range; every open panel of the group follows
    /// ([`Self::apply_group_scales`]) and `[UI.group_scale]` is written off the frame thread
    /// ([`Self::take_group_scale_persist`]), and the plugin layer is told the group's new value
    /// at the next delivery (§7.7). Nothing happens, and nothing is saved, over a panel without
    /// a group, a plugin window, a panel that takes no per-window factor
    /// ([`tessera_ui::Ui::accepts_window_scale`]) or no panel at all. Returns whether a factor
    /// changed.
    pub fn step_window_scale(&mut self, delta: f32) -> bool {
        let root = {
            let ui = self.ui.borrow();
            ui.hovered().and_then(|h| ui.screen_root_of(h))
        };
        root.is_some_and(|r| self.step_panel_scale(r, delta))
    }

    /// The step of [`Self::step_window_scale`] for the root panel `root` (`ui-scaling.md`
    /// §7.4): its group's factor changes by `delta` on the 0.05 grid. Returns whether it
    /// changed.
    pub fn step_panel_scale(&mut self, root: ElementKey, delta: f32) -> bool {
        let group = {
            let ui = self.ui.borrow();
            // A panel that takes no per-window factor (a plugin window, a fit root, one the
            // panel-scale rule owns) is not scaled, and nothing is saved for it.
            if ui.is_control(root) || !ui.accepts_window_scale(root) {
                return false;
            }
            let g = ui.prop(root, ui.vocab().pids.scaler_group);
            if !g.is_set() {
                return false;
            }
            u32::try_from(g.int()).ok()
        };
        let Some(g) = group else { return false };
        let cur = self.group_scale_of(g);
        let (lo, hi) = self.group_scale_range(g);
        let p = (((cur + delta) / GROUP_SCALE_STEP).round() * GROUP_SCALE_STEP).clamp(lo, hi);
        if (p - cur).abs() <= f32::EPSILON {
            return false;
        }
        if (p - 1.0).abs() <= f32::EPSILON {
            self.prefs.group_scale.remove(&g);
        } else {
            self.prefs.group_scale.insert(g, p);
        }
        self.group_scales_dirty = true;
        self.frame.event = true;
        self.queue_plugin_group_scale(g, p);
        self.apply_group_scales();
        true
    }

    /// Notes a changed group value for the plugin layer (`ui-scaling.md` §7.7 "Group values"):
    /// sent at the next delivery, where marked plugin windows of the group take it and get
    /// `UIScaleChanged` (`lua-plugin-api.md` §13.21 rule 8). A later value of the same group
    /// replaces a pending one.
    fn queue_plugin_group_scale(&mut self, g: u32, p: f32) {
        self.plugin_group_scales.retain(|(q, _)| *q != g);
        self.plugin_group_scales.push((g, p));
    }

    /// Sends the pending group values to the plugin layer and handles the delivery's events.
    fn flush_plugin_group_scales(&mut self, plugins: &mut Plugins) {
        let pending = std::mem::take(&mut self.plugin_group_scales);
        for (g, p) in pending {
            let events = self.plugin_ui.scale_changed(plugins.manager(), g, p);
            self.handle_client_events(plugins, events);
        }
    }

    /// The factor of scaling group `g` (`ui-scaling.md` §7.4): its `[UI.group_scale]` entry,
    /// 1.0 when absent, held to the group's range.
    #[must_use]
    pub fn group_scale_of(&self, g: u32) -> f32 {
        let (lo, hi) = self.group_scale_range(g);
        self.prefs
            .group_scale
            .get(&g)
            .copied()
            .filter(|v| v.is_finite())
            .unwrap_or(1.0)
            .clamp(lo, hi)
    }

    /// The range of scaling group `g` (§7.4): the group's `UIScaleDefinition_Min`/`Max` handed
    /// to [`Self::set_group_scale_ranges`] intersected with 0.5–2.0, or 0.5–2.0 when the host
    /// has no range for the group. The client does not read `UIScaleDefinition_Array` yet, so
    /// in the client every group has 0.5–2.0 today. A range that does not meet 0.5–2.0 (or is
    /// not finite) leaves only 1.0: the group is not scaled (a Tessera choice; the spec does
    /// not say, proposed as an OPEN).
    #[must_use]
    pub fn group_scale_range(&self, g: u32) -> (f32, f32) {
        let (min, max) = GROUP_SCALE_LIMITS;
        match self.group_ranges.get(&g) {
            Some(&(lo, hi)) if lo.is_finite() && hi.is_finite() && lo.max(min) <= hi.min(max) => {
                (lo.max(min), hi.min(max))
            }
            Some(_) => (1.0, 1.0),
            None => (min, max),
        }
    }

    /// The scale definitions' ranges by group (`ui-behaviour.md` §9.1: `UIScaleDefinition_Min`
    /// and `_Max` of each `UIScaleDefinition_Array` entry), for [`Self::group_scale_range`].
    /// Nothing in the client calls this yet (the array is not read at UI boot).
    pub fn set_group_scale_ranges(&mut self, ranges: BTreeMap<u32, (f32, f32)>) {
        self.group_ranges = ranges;
        self.apply_group_scales();
    }

    /// Gives every native root panel with a scaling group its group's factor (`ui-scaling.md`
    /// §7.4): the root panels of the screen and the modal dialogs ([`Ui::screen_roots`]), the
    /// panels opened since included. A panel whose factor is already right, or that takes no
    /// factor ([`Ui::accepts_window_scale`]), is left alone. A panel opened since the last call
    /// that gets a factor other than 1 has its saved placement applied again afterwards, so the
    /// placement uses its real displayed size (§5.8 "Applying"). Runs after panel calls and
    /// deliveries, like [`Self::apply_native_scale`].
    fn apply_group_scales(&mut self) {
        let (todo, roots): (Vec<(ElementKey, f32, bool)>, Vec<ElementKey>) = {
            let ui = self.ui.borrow();
            let pid = ui.vocab().pids.scaler_group;
            let roots: Vec<ElementKey> = ui
                .screen_roots()
                .into_iter()
                .filter(|k| !ui.is_control(*k))
                .collect();
            let todo = roots
                .iter()
                .copied()
                .filter(|k| ui.accepts_window_scale(*k))
                .filter_map(|k| {
                    let g = ui.prop(k, pid);
                    if !g.is_set() {
                        return None;
                    }
                    let g = u32::try_from(g.int()).ok()?;
                    let want = self.group_scale_of(g);
                    ((ui.window_scale(k) - want).abs() > f32::EPSILON).then_some((
                        k,
                        want,
                        !self.group_seen.contains(&k),
                    ))
                })
                .collect();
            (todo, roots)
        };
        self.group_seen.retain(|k| roots.contains(k));
        self.group_seen.extend(roots.iter().copied());
        if todo.is_empty() {
            return;
        }
        let mut ui = self.ui.borrow_mut();
        for (k, p, new) in todo {
            if ui.set_window_scale(k, p) && new {
                ui.reapply_saved_placement(k);
            }
        }
    }

    /// `[UI.group_scale]`, once after a gesture changed it (the frame loop hands it to the
    /// settings writer, off the frame thread; `ui-scaling.md` §4.4, §7.4). Not saved in
    /// scripted runs, like the placements ([`Self::set_persist_placements`]).
    pub fn take_group_scale_persist(&mut self) -> Option<BTreeMap<u32, f32>> {
        if !std::mem::take(&mut self.group_scales_dirty) || !self.persist_placements {
            return None;
        }
        Some(self.prefs.group_scale.clone())
    }

    /// The crafting-panel toggles pressed this frame (`crafting-panels.md` §4.1, §12):
    /// `None` for `toggle_crafting`, `Some(i)` (0-based tab) for `toggle_crafting_<i + 1>`.
    /// Nothing while a UI element has the keyboard focus (a key typed into an entry is text).
    #[must_use]
    pub fn poll_p6_actions(&self, mapper: &InputMapper) -> Vec<Option<usize>> {
        if self.keyboard_focused() {
            return Vec::new();
        }
        let pressed =
            |id: Option<tessera_input::ActionId>| id.is_some_and(|i| mapper.just_pressed(i));
        let mut out = Vec::new();
        if pressed(self.ids.toggle_crafting) {
            out.push(None);
        }
        for (i, id) in self.ids.toggle_crafting_tabs.iter().enumerate() {
            if pressed(*id) {
                out.push(Some(i));
            }
        }
        out
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
        self.apply_group_scales();
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
        self.flush_plugin_group_scales(plugins);
        for _ in 0..MAX_DELIVERY_ROUNDS {
            let events = self.plugin_ui.deliver(plugins.manager());
            if events.is_empty() && self.deferred.borrow().is_empty() {
                break;
            }
            self.handle_client_events(plugins, events);
            self.run_deferred(plugins);
        }
        self.apply_native_scale();
        self.apply_group_scales();
    }

    fn handle_client_events(&mut self, plugins: &mut Plugins, events: Vec<UiEvent>) {
        // The end of a user move or resize of a saved element (`ui-behaviour.md` §8.4 step
        // 2): its placement is recorded for `[UI.panels]` (`ui-scaling.md` §5.8).
        for ev in &events {
            if let UiEvent::LayoutChanged { element } = ev {
                self.record_placement(*element);
            }
        }
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

    /// Whether a modal engine dialog is shown (`ui-widgets.md` §18.4): then no key reaches
    /// game code (the host consumes the actions the dialog does not take; callers with keys
    /// of their own, such as the walker's world keys and the NPC windows' Escape and Enter,
    /// leave them alone).
    #[must_use]
    pub fn modal_dialog_shown(&self) -> bool {
        tessera_ui::widgets::dialog::modal_dialog_shown(&self.ui.borrow())
    }

    /// Whether the UI's keyboard target (the focus element, else the active panel) lies in
    /// a shown engine dialog: only then do keys pressed under a modal dialog reach the UI
    /// (§18.4: the dialog's own keys), so a dialog shown with `StartInactive`, which leaves
    /// the keyboard where it was (for example in the chat entry), takes no key from it.
    fn keyboard_in_dialog(&self) -> bool {
        use tessera_ui::widgets::dialog::{MARK_PID, MARK_SHOWN, MARK_TAKEN_DOWN};
        let ui = self.ui.borrow();
        let mut cur = ui.keyboard_target();
        // Tessera bound on the walk up (element trees are far shallower).
        for _ in 0..256 {
            let Some(k) = cur else {
                return false;
            };
            let m = ui.prop(k, MARK_PID).int();
            if m & MARK_SHOWN != 0 && m & MARK_TAKEN_DOWN == 0 && ui.is_displayed(k) {
                return true;
            }
            cur = ui.parent(k);
        }
        false
    }

    /// §18.4 for bindings that are never offered to the UI (holds without a command name,
    /// such as `toggle_mount`, are the game's at once, `keymap.md` §7.8): while a modal
    /// engine dialog is shown, and until every key pressed under it is released, the host
    /// keeps its own entry of the `text_entry` context in the UI band at the lowest
    /// priority. Its `opaque_keys` keeps keys and pad buttons from the game band (§3.3),
    /// while its bindings still give the dialog `AcceptInput` (Enter) and `EscapeKey`
    /// (Escape), and the UI band's other contexts (the dialog's own map) come first.
    /// Tessera reading: as when a text entry takes the keyboard, the game-band key holds
    /// held from before the dialog end when it appears (their Ends reach game code).
    fn sync_modal_band(&mut self, mapper: &mut InputMapper) {
        let want = self.modal_dialog_shown() || !self.modal_keys.is_empty();
        if want == self.modal_band {
            return;
        }
        let Some(ctx) = self
            .ids
            .contexts
            .iter()
            .find(|(n, _)| *n == contexts::TEXT_ENTRY)
            .map(|(_, c)| *c)
        else {
            return;
        };
        if want {
            mapper.band_push(ctx, MODAL_BAND_OWNER, i32::MIN);
        } else {
            mapper.band_pop(ctx, MODAL_BAND_OWNER);
        }
        self.modal_band = want;
    }

    /// The raw keys of §18.4: a key or pad button pressed while a modal dialog is shown is
    /// remembered until its release (`pressed` = the press side of the event).
    fn modal_key_of(event: &RawEvent) -> Option<(Button, bool)> {
        match *event {
            RawEvent::Key {
                key,
                pressed,
                repeat: false,
            } => Some((Button::Key(key), pressed)),
            RawEvent::PadButton { button, pressed } => Some((Button::Pad(button), pressed)),
            _ => None,
        }
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
        // §18.4: remember the keys pressed under a modal dialog, and keep the game band
        // from keys while one is shown (`sync_modal_band`).
        let modal_key = Self::modal_key_of(event);
        if let Some((b, true)) = modal_key
            && self.modal_dialog_shown()
            && !self.modal_keys.contains(&b)
            && self.modal_keys.len() < MAX_MODAL_KEYS
        {
            self.modal_keys.push(b);
        }
        self.sync_modal_band(mapper);
        {
            let mut sink = Step1 {
                host: self,
                plugins,
                window,
            };
            mapper.handle_with(event, now, &mut sink);
        }
        match (modal_key, event) {
            (Some((b, false)), _) => self.modal_keys.retain(|k| *k != b),
            (_, RawEvent::FocusLost) => self.modal_keys.clear(),
            _ => {}
        }
        self.sync_modal_band(mapper);
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
        self.sync_modal_band(mapper);
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
                // The mapper reports the pointer as a continuous physical position; the UI
                // works in UI units (`ui-scaling.md` §3.6). At Se = 1 this is the identity.
                let (ux, uy) = self.scale.physical_to_ui(*x, *y);
                self.pointer_physical = Some((*x, *y));
                self.ui
                    .borrow_mut()
                    .offer(&UiInput::PointerMoved { x: ux, y: uy })
            }
            OfferedEvent::PointerLeft { .. } => {
                self.pointer_physical = None;
                self.ui.borrow_mut().offer(&UiInput::PointerLeft)
            }
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
        // `ui-widgets.md` §18.4: while a modal engine dialog is shown no key reaches game
        // code below it (the chat entry, the actions); the dialog's own keys reach it
        // through the UI below. A hold made by a key pressed under the dialog counts too
        // (a release-class binding fires at the release, maybe after the dialog closed).
        let modal =
            !is_pointer_cmd && (self.modal_dialog_shown() || self.modal_keys.contains(&a.source));
        // The chat window sees key presses first while it is the keyboard target
        // (`Chat::command`); its holds are not offered to the UI again.
        if !is_pointer_cmd {
            if phase == Phase::End && self.chat_holds.remove(&hold) {
                return true;
            }
            if phase != Phase::End
                && !modal
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
        // §18.4: every other key press while the modal dialog is shown, and the repeats and
        // the release of each key pressed while it was shown, are consumed. The UI sees
        // them only while its keyboard target lies in the shown dialog (its own keys);
        // otherwise (a `StartInactive` dialog left the keyboard in the chat entry) nobody
        // does. Keys held from before the dialog are not consumed, so their releases are
        // not lost (game-band key holds end when the dialog appears, `sync_modal_band`).
        if !is_pointer_cmd {
            let under_modal = match phase {
                Phase::Start if modal => {
                    if self.modal_holds.len() >= MAX_MODAL_HOLDS
                        && let Some(oldest) = self.modal_holds.keys().next().copied()
                    {
                        self.modal_holds.remove(&oldest);
                    }
                    let offered = self.keyboard_in_dialog();
                    self.modal_holds.insert(hold, offered);
                    Some(offered)
                }
                Phase::Start => None,
                // Offered on only while the keyboard is still in the dialog: the release of
                // the key that closed it goes nowhere (no key broadcast either).
                Phase::Repeat => self
                    .modal_holds
                    .get(&hold)
                    .map(|o| *o && self.keyboard_in_dialog()),
                Phase::End => self
                    .modal_holds
                    .remove(&hold)
                    .map(|o| o && self.keyboard_in_dialog()),
            };
            if let Some(offered) = under_modal {
                if offered {
                    let _ = self.ui.borrow_mut().offer(&UiInput::Command(CommandOffer {
                        phase,
                        hold,
                        command,
                        value: a.value,
                        position: a.position,
                    }));
                }
                return true;
            }
        }
        // Every command goes through `Ui::offer`: the UI decides Impulse from the action
        // map's toggle types by command ID (`lua-plugin-api.md` §13.12.1, decision 0014 D9);
        // the host hard-codes none.
        let consumed = self.ui.borrow_mut().offer(&UiInput::Command(CommandOffer {
            phase,
            hold,
            command,
            value: a.value,
            position: a.position,
        }));
        // The P4 social commands the UI declined (`social-panels.md` §7.2, TR12): run with
        // the social state after step 7.
        if !consumed
            && !is_pointer_cmd
            && let Some(s) = self.social.as_mut()
            && crate::social::ui::is_social_command(&name)
        {
            if phase == Phase::Start {
                s.queue_command(&name);
            }
            return true;
        }
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
                        // The frozen cursor is a physical pixel; the UI lock point is in UI
                        // units (`ui-scaling.md` §3.6; identity at Se = 1).
                        let f = self.lock.frozen().map(|(x, y)| {
                            self.scale.pointer_to_ui(x.floor() as i32, y.floor() as i32)
                        });
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
        // `ui-widgets.md` §18.4: under a modal dialog typed text goes to the UI's focus only
        // when that lies in the shown dialog (its entry); never to the chat window's entry
        // or another element below the dialog, and none reaches game code.
        if self.modal_dialog_shown() {
            let _ = plugins;
            if self.keyboard_in_dialog() {
                let _ = self.ui.borrow_mut().offer(&UiInput::Char(ch));
            }
            return true;
        }
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
        self.poll_ladders();
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

    /// The social panels' frame (after step 7; decision 0024 P4): the panel commands and
    /// the frame's UI events (`events`: the one list of [`Self::take_game_events`] the NPC
    /// windows and the quest journal read too) with the world session's social state, the
    /// panels refreshed from it, the chat window's tell targets (CT3) and instant-message lines (CT6), and the
    /// instant-message tab created once the chat window is open (CT6). Returns the commands
    /// for the session; the panels' lines go to the chat window, a tell the panels started
    /// opens the chat entry in the `Tell` mode.
    pub fn social_frame(
        &mut self,
        state: &mut crate::social::SocialState,
        data: Option<&crate::character::CharacterData>,
        player: Option<&crate::character::PlayerState>,
        events: &[UiEvent],
    ) -> Vec<tessera_net::Command> {
        let Some(s) = self.social.as_mut() else {
            self.pending_im.clear();
            return Vec::new();
        };
        for ev in events {
            // The core's `DialogResult` of a dialog key (`ui-widgets.md` §18.4) reaches the
            // social panels' dialogs too, so a dialog answered by key gets its result.
            if matches!(
                ev,
                UiEvent::ExamineHover { .. }
                    | UiEvent::DragEnded { .. }
                    | UiEvent::Game { .. }
                    | UiEvent::DialogResult { .. }
            ) {
                s.queue_event(ev);
            }
        }
        let mut ui = self.ui.borrow_mut();
        if let Some(root) = self
            .panels
            .as_ref()
            .and_then(|p| p.root(PanelId::PlayerVitals))
        {
            s.attach_player_vitals(&mut ui, root, state);
        }
        let out = s.frame(&mut ui, state, data, player);
        // The fellowship rows' effect lists go to the combat widgets (`social-panels.md`
        // TR16), which bind them in their next frame.
        let lists = s.member_effect_lists();
        if lists != self.combat.member_lists.as_slice() {
            self.combat.member_lists = lists.to_vec();
            self.combat.members_dirty = true;
        }
        if let Some(chat) = self.panels.as_mut().and_then(|p| p.chat.as_mut()) {
            chat.ensure_im_tab(&mut ui);
            if self.tells_generation != Some(state.targets_generation()) {
                chat.chat.set_tell_targets(state.chat().targets());
                self.tells_generation = Some(state.targets_generation());
            }
            let time = clock_text(std::time::SystemTime::now());
            for im in std::mem::take(&mut self.pending_im) {
                let _ = chat
                    .chat
                    .deliver_im(&mut ui, &im, &time, state.text_source());
            }
            for (mask, text) in &out.lines {
                chat.post_mask_line(&mut ui, *mask, text);
            }
            if let Some(name) = &out.tell {
                chat.start_tell(&mut ui, name);
            }
        } else {
            self.pending_im.clear();
        }
        drop(ui);
        if !out.commands.is_empty() || !out.lines.is_empty() {
            self.frame.content_event = true;
        }
        out.commands
    }

    /// The social panels, when the install provides them.
    #[must_use]
    pub fn social(&self) -> Option<&crate::social::SocialUi> {
        self.social.as_ref()
    }

    /// The social panels, mutably (opening panels by name, tests).
    pub fn social_mut(&mut self) -> Option<&mut crate::social::SocialUi> {
        self.social.as_mut()
    }

    /// Gives the host social panels (tests and tools: a host made without an install,
    /// [`Self::without_install`], has none; one booted from the install has them already).
    pub fn set_social(&mut self, social: crate::social::SocialUi) {
        self.social = Some(social);
    }

    /// Queues a line for the command dispatcher as the chat window's entry would
    /// (decision 0020 item 1): it runs in [`Self::dispatch_entry_lines`].
    pub fn submit_entry_line(&mut self, line: EntryLine) {
        self.queue_entry_line(line);
    }

    /// The UI events since the last call (step 1 and step 7), for the in-world windows (the
    /// NPC windows, the quest journal and the social panels read the one list). Take them once per frame; at
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
                for (mask, text) in local {
                    chat.show_mask(&mut ui, mask, &text);
                    received.push((None, text, mask_type(mask)));
                }
                for n in notices {
                    chat.show(&mut ui, crate::panels::chat::CHAT_LOG, &n);
                    received.push((None, n, crate::panels::chat::CHAT_LOG));
                }
                for l in session {
                    // P4 line forms (`chat-window.md` §15.10 CT1, CT10): shown and delivered
                    // with their masks and rendered text; the tell's instant-message line
                    // goes to its session in the social frame (CT6).
                    if let Some((mask, shown)) = &l.shown {
                        chat.show_mask(&mut ui, *mask, shown);
                        if let Some(im) = l.im.clone()
                            && self.pending_im.len() < 256
                        {
                            self.pending_im.push(im);
                        }
                        received.push((
                            (!l.sender.is_empty()).then_some(l.sender),
                            shown.clone(),
                            mask_type(*mask),
                        ));
                        continue;
                    }
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
                for (mask, text) in local {
                    received.push((None, text, mask_type(mask)));
                }
                for n in notices {
                    received.push((None, n, crate::panels::chat::CHAT_LOG));
                }
                for l in session {
                    let text = match &l.shown {
                        Some((_, t)) => t.clone(),
                        None => l.text,
                    };
                    received.push((
                        (!l.sender.is_empty()).then_some(l.sender),
                        text,
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
        // Panels attached this frame take their saved placements before they are painted.
        self.apply_saved_placements(false);
        let t = Instant::now();
        let now = Duration::from_secs_f64(self.clock.max(0.0));
        let frame = self
            .ui
            .borrow_mut()
            .paint(now, &mut TimeSlice::new(self.limits.paint_slice));
        let paint = t.elapsed();
        if let Some(r) = renderer {
            self.textures.apply(r, &frame.ops);
            if self.prefs.cursor == CursorMode::Software {
                self.sync_cursor_texture(r);
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
            ui_gpu_ms: None,
        };
    }

    /// The software cursor's texture at the size of `ui-scaling.md` §6.7: built at
    /// `Ui::cursor_physical_size` (trunc(32 × *Se* × `ui_scale_cursor`)) with the hot spot
    /// scaled by the same factor, and built again whenever that size changes (a change of
    /// *Se* or of the cursor scale). Tessera's arrow is drawn at the size directly; it has no
    /// native image to resample. The tooltip offset keeps the UI-unit `Ui::cursor_size`.
    fn sync_cursor_texture(&mut self, r: &mut Renderer<'_>) {
        let (side, hotspot) = {
            let ui = self.ui.borrow();
            let side = cursor::cursor_image_side(ui.cursor_physical_size());
            let (hx, hy) = cursor::ARROW_HOTSPOT;
            (side, ui.cursor_hotspot(hx, hy))
        };
        if self.cursor_tex.is_some() && self.cursor_side == side {
            return;
        }
        if let Some(old) = self.cursor_tex.take() {
            r.destroy_texture(old);
        }
        match r.create_ui_texture(side, side, Some("ui-cursor")) {
            Ok(tex) => {
                let px = cursor::arrow_bgra(side);
                let _ = r.update_texture_rect(tex, 0, 0, side, side, &px, side as usize * 4);
                self.cursor_tex = Some(tex);
                self.cursor_side = side;
                self.cursor_hotspot = hotspot;
            }
            Err(e) => {
                // Tried again next frame; the OS cursor stays hidden meanwhile.
                tracing::debug!("software cursor texture: {e}");
                self.cursor_side = 0;
            }
        }
    }

    /// The software cursor's image side and hot spot in physical pixels (`ui-scaling.md`
    /// §6.7), as last built; (0, …) before the first build.
    #[must_use]
    pub fn cursor_image(&self) -> (u32, (i32, i32)) {
        (self.cursor_side, self.cursor_hotspot)
    }

    /// Takes the renderer's figures of the frame just submitted (call after
    /// `Renderer::end_frame`, before [`Self::end_frame`]): the UI pass's GPU time
    /// (`UiStats::gpu_ns`, render.md §6.16 U2) goes into the frame's sample. It is the most
    /// recent resolved figure, so it lags by a few frames, and `None` without
    /// `TIMESTAMP_QUERY` (U2 is then not a gate).
    pub fn record_render_stats(&mut self, stats: &tessera_render::FrameStats) {
        self.last.ui_gpu_ms = stats.ui.gpu_ns.map(|ns| ns as f64 / 1e6);
        if let Some(r) = self.recorder.as_mut() {
            r.render_stats_fed = true;
        }
    }

    /// Step 7: once the enum's fonts have loaded, registers the font ladders
    /// (`ui-scaling.md` §6.4.2, [`fonts`]). The loads start only when no other UI asset job is
    /// in flight, so they never hold back the first visible panels.
    fn poll_ladders(&mut self) {
        let (Some(loader), Some(h)) = (self.ladders.as_mut(), self.env.as_ref()) else {
            return;
        };
        let ladders = {
            let mut st = h.borrow_mut();
            if !loader.started() && st.busy() {
                return;
            }
            loader.poll_env(&mut st)
        };
        if let Some(ladders) = ladders {
            tracing::debug!(ladders = ladders.len(), "font ladders registered");
            self.ui.borrow_mut().set_font_ladders(ladders);
            self.ladders = None;
        }
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
            && let (Some(tex), Some(ui_pointer)) = (self.cursor_tex, self.ui.borrow().pointer())
        {
            let at = cursor::cursor_position(
                self.pointer_physical,
                ui_pointer,
                self.scale.screen(),
                self.cursor_hotspot,
            );
            frame.software_cursor(tex, at);
        }
    }

    /// After present: the frame clock advances (plugins' game time, the UI's hover clock)
    /// and the frame's sample is recorded. `frame_ms` is the frame's own time (pacing
    /// excluded).
    pub fn end_frame(&mut self, plugins: &Plugins, frame_ms: f64) {
        self.clock = match self.frame_clock {
            FrameClock::Wall => self.started.elapsed().as_secs_f64(),
            FrameClock::Stepped { step } => self.clock + step,
            FrameClock::Held => self.clock,
        };
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
        plugins.manager_mut().set_party_actor_source(None);
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

/// What [`UiHost::apply_interface_action`] did.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum InterfaceApplied {
    /// The UI scale or safe aspect changed: the screen is fed again.
    Scale,
    /// The window is to close.
    Close,
    /// Anything else (applied in place).
    Other,
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

/// How a session line shows in the chat window: the P4 line form when it has one
/// (`chat-window.md` §15.10 CT1), else the text alone for a system line and `Sender: text`
/// otherwise (Tessera's own wording, `chat-window.md` §10 item 2).
#[must_use]
pub fn session_text(line: &crate::net::ChatLine) -> String {
    line.display_text()
}

/// The chat type of a line's mask for the plugins' `Received` (`chat-window.md` §9 Q18
/// rule, §15.10 CT10): its highest set type, 1 for an empty mask.
#[must_use]
pub fn mask_type(mask: u64) -> u32 {
    (64 - mask.leading_zeros()).max(1)
}

/// The time-of-day text of an instant-message line (`chat-window.md` §15.10 CT6, §5.4
/// Tessera rule: 24-hour `HH:MM:SS`). The platform gives the client no local offset (no
/// time-zone database), so this is the UTC time of day, the known difference accepted in
/// the §15.10 answered callout (b); §5.4 timestamps, when built, must use this clock too.
#[must_use]
pub fn clock_text(now: std::time::SystemTime) -> String {
    let secs = now
        .duration_since(std::time::UNIX_EPOCH)
        .map_or(0, |d| d.as_secs())
        % 86_400;
    format!("{:02}:{:02}:{:02}", secs / 3600, secs / 60 % 60, secs % 60)
}

/// Where the UI host's frame clock (the UI's animation and hover clock, the plugins' game
/// time) comes from ([`UiHost::set_frame_clock`]).
#[derive(Debug, Clone, Copy, PartialEq, Default)]
pub enum FrameClock {
    /// Seconds since the host started (the client).
    #[default]
    Wall,
    /// Advances by `step` seconds at each frame end, whatever the wall clock does (tests:
    /// animation time follows the frames).
    Stepped {
        /// Seconds per frame (0–1).
        step: f64,
    },
    /// Stays where it is (a capture waiting for images and paint: animations do not
    /// move).
    Held,
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

    /// `keymap.md` §7.8.6: the mapper gets the action map's type-3 command set at attach only
    /// with both action-map data and command IDs; a later attach without them withdraws it from
    /// the same mapper.
    #[test]
    fn attach_hands_over_the_repeat_set() {
        use tessera_input::{InputMapper, default_keymap, default_registry};
        let options = UiOptions::default();
        let mut host = UiHost::new(
            Box::new(tessera_ui::NullEnv),
            None,
            Vocab::default(),
            &options,
        );
        let (mut mapper, _) = InputMapper::new(default_registry(), &default_keymap());
        host.attach_mapper(&mut mapper);
        assert!(mapper.repeat_commands().is_none(), "no action map, no set");
        let types: BTreeMap<u32, u8> = [(10, 3), (11, 2), (12, 3), (13, 1)].into();
        host.set_toggle_types(&types);
        // Table A without command IDs: no hold carries an ID, so no set (the flags decide).
        host.attach_mapper(&mut mapper);
        assert!(mapper.repeat_commands().is_none(), "no command IDs, no set");
        host.command_ids = vec![("backspace".to_owned(), 10)];
        host.attach_mapper(&mut mapper);
        let got: BTreeSet<u32> = mapper
            .repeat_commands()
            .expect("set handed over")
            .iter()
            .copied()
            .collect();
        assert_eq!(got, [10, 12].into());
        // An empty table A is no action-map data: the same mapper loses the old set.
        host.set_toggle_types(&BTreeMap::new());
        host.attach_mapper(&mut mapper);
        assert!(mapper.repeat_commands().is_none(), "withdrawn on re-attach");
    }

    /// One layout with three 400 × 300 root panels of scaling group 9 that save their location
    /// (`ROOT`, `SECOND`, and the modal dialog `DIALOG`) and one without a group (`PLAIN`).
    fn group_host() -> UiHost {
        use tessera_formats::Did;
        use tessera_formats::property::{PropertyEntry, PropertyValue};
        use tessera_formats::ui_layout::{ElementDesc, UiLayout};
        let v = Vocab::default();
        let el = |id: u32, x: i32, y: i32, props: Vec<(u32, PropertyValue)>| {
            let mut d = ElementDesc {
                element_id: id,
                element_type: v.types.field,
                ..ElementDesc::default()
            };
            d.block.rect.x = x;
            d.block.rect.y = y;
            d.block.rect.width = 400;
            d.block.rect.height = 300;
            d.block.has_position = true;
            for (pid, value) in props {
                d.block
                    .properties
                    .entries
                    .push(PropertyEntry { pid, value });
            }
            d.block.properties.size.count = d.block.properties.entries.len() as u32;
            d
        };
        let g = |extra: Vec<(u32, PropertyValue)>| {
            let mut p = vec![
                (v.pids.scaler_group, PropertyValue::Int(9)),
                (v.pids.save_loc, PropertyValue::Bool(true)),
            ];
            p.extend(extra);
            p
        };
        let mut l = UiLayout {
            did: Did(GROUP_LAYOUT),
            design_width: 1920,
            design_height: 1080,
            ..UiLayout::default()
        };
        l.elements = vec![
            el(GROUP_ROOT, 300, 200, g(vec![])),
            el(GROUP_SECOND, 900, 200, g(vec![])),
            el(
                GROUP_DIALOG,
                700,
                600,
                g(vec![(v.pids.dialog_modal, PropertyValue::Bool(true))]),
            ),
            el(GROUP_PLAIN, 100, 700, vec![]),
        ];
        l.elements_size.count = l.elements.len() as u32;
        let mut env = tessera_ui::MemoryEnv::default();
        env.add_layout(l);
        let options = UiOptions {
            screen: (1920, 1080),
            hud: false,
            ..UiOptions::default()
        };
        let mut host = UiHost::new(Box::new(env), None, v, &options);
        host.set_persist_placements(true);
        host
    }

    const GROUP_LAYOUT: u32 = 0x22FF_6100;
    const GROUP_ROOT: u32 = 0x1000_6101;
    const GROUP_SECOND: u32 = 0x1000_6102;
    const GROUP_DIALOG: u32 = 0x1000_6103;
    const GROUP_PLAIN: u32 = 0x1000_6104;

    fn open_group(host: &mut UiHost, id: u32) -> ElementKey {
        let k = host
            .ui_mut()
            .open_panel(tessera_formats::Did(GROUP_LAYOUT), id)
            .expect("panel");
        // As the native panels do when a panel opens (`panels/mod.rs`): the saved placement.
        host.ui_mut().apply_saved(k);
        host.apply_group_scales();
        k
    }

    /// `ui-scaling.md` §7.4: the gesture steps the panel's group by 0.05 within 0.5–2.0, the
    /// panel takes it as a uniform node scale, a panel of the group opened afterwards takes it
    /// too, the change is persisted once and queued for the plugin layer, and "Reset window
    /// scales" puts every panel back to 1.
    #[test]
    fn per_window_scale_follows_the_group() {
        let mut host = group_host();
        let k = open_group(&mut host, GROUP_ROOT);
        assert_eq!(host.group_scale_range(9), GROUP_SCALE_LIMITS);
        assert!(host.step_panel_scale(k, GROUP_SCALE_STEP));
        assert!(host.step_panel_scale(k, GROUP_SCALE_STEP));
        assert!((host.group_scale_of(9) - 1.1).abs() < 1e-6);
        assert!((host.ui().window_scale(k) - 1.1).abs() < 1e-6);
        let persisted = host.take_group_scale_persist().expect("persisted");
        assert!((persisted[&9] - 1.1).abs() < 1e-6);
        assert!(host.take_group_scale_persist().is_none(), "once");
        assert_eq!(
            host.plugin_group_scales.len(),
            1,
            "one pending value per group"
        );
        assert!((host.plugin_group_scales[0].1 - 1.1).abs() < 1e-6);
        // A panel of the group opened after the step takes the factor.
        let second = open_group(&mut host, GROUP_SECOND);
        assert!((host.ui().window_scale(second) - 1.1).abs() < 1e-6);
        // Held to the range: 2.0 at most.
        for _ in 0..40 {
            host.step_panel_scale(k, GROUP_SCALE_STEP);
        }
        assert_eq!(host.group_scale_of(9), 2.0);
        assert!(
            !host.step_panel_scale(k, GROUP_SCALE_STEP),
            "no change past the top"
        );
        // A data range narrows it (intersected with 0.5–2.0) ...
        host.set_group_scale_ranges([(9, (0.4, 1.5))].into());
        assert_eq!(host.group_scale_range(9), (0.5, 1.5));
        assert_eq!(
            host.ui().window_scale(k),
            1.5,
            "re-applied within the range"
        );
        // ... and one that does not meet 0.5–2.0 leaves only 1.0.
        host.set_group_scale_ranges([(9, (2.5, 3.0))].into());
        assert_eq!(host.group_scale_range(9), (1.0, 1.0));
        assert_eq!(host.ui().window_scale(k), 1.0);
        host.set_group_scale_ranges(BTreeMap::new());
        // Reset window scales: every panel of the group back to 1, and the plugins told.
        let _ = host.apply_interface_action(
            tessera_ui::game::interface::InterfaceAction::ResetWindowScales,
            std::time::Instant::now(),
        );
        assert_eq!(host.ui().window_scale(k), 1.0);
        assert_eq!(host.ui().window_scale(second), 1.0);
        assert_eq!(host.plugin_group_scales.last(), Some(&(9, 1.0)));
        // An element without a group is not scaled.
        let plain = open_group(&mut host, GROUP_PLAIN);
        assert!(!host.step_panel_scale(plain, GROUP_SCALE_STEP));
        let screen = host.ui().screen_root();
        assert!(!host.step_panel_scale(screen, GROUP_SCALE_STEP));
    }

    /// §7.4 and §5.8: a panel whose placement was saved centred at p = 1.5 comes back at the
    /// same displayed box when it is opened again (the placement is applied with its scaled
    /// size), and stays there after a change of F.
    #[test]
    fn a_reopened_scaled_panel_keeps_its_saved_box() {
        let mut host = group_host();
        host.prefs.group_scale.insert(9, 1.5);
        let k = open_group(&mut host, GROUP_ROOT);
        assert_eq!(host.ui().window_scale(k), 1.5);
        // The user leaves it centred: displayed 660, 315, 600 × 450 at C2.
        let d = host.ui().displayed_box(k);
        let b = host.ui().layout_box(k).expect("box");
        host.ui_mut()
            .move_to(k, b.l + (660 - d.l), b.t + (315 - d.t));
        host.ui_mut().record_saved(k);
        let saved = host.ui().displayed_box(k);
        assert_eq!(
            (saved.l, saved.t, saved.w(), saved.h()),
            (660, 315, 600, 450)
        );
        host.ui_mut().destroy(k);
        host.apply_group_scales();
        let k = open_group(&mut host, GROUP_ROOT);
        assert_eq!(
            host.ui().displayed_box(k),
            saved,
            "same box after reopening"
        );
    }

    /// §7.4: a modal dialog of the group follows the gesture too (it is a screen root in the
    /// dialog container), and the gesture over it scales the group.
    #[test]
    fn a_modal_dialog_follows_its_group() {
        let mut host = group_host();
        let k = open_group(&mut host, GROUP_ROOT);
        let dlg = open_group(&mut host, GROUP_DIALOG);
        assert_ne!(
            host.ui().parent(dlg),
            Some(host.ui().screen_root()),
            "modal"
        );
        assert_eq!(host.ui().screen_root_of(dlg), Some(dlg));
        assert!(host.step_panel_scale(dlg, GROUP_SCALE_STEP));
        assert!((host.ui().window_scale(dlg) - 1.05).abs() < 1e-6);
        assert!((host.ui().window_scale(k) - 1.05).abs() < 1e-6);
    }

    /// §7.4: over a panel the panel-scale rule owns (`Ui::scale_panel` at 1.5) the gesture
    /// changes and saves nothing; once that rule is back at 1 the panel follows the group.
    #[test]
    fn the_gesture_skips_a_panel_it_cannot_scale() {
        let mut host = group_host();
        let k = open_group(&mut host, GROUP_ROOT);
        host.ui_mut().scale_panel(k, 1.5, true);
        assert!(!host.step_panel_scale(k, GROUP_SCALE_STEP));
        assert!(host.take_group_scale_persist().is_none(), "nothing saved");
        assert!(host.prefs.group_scale.is_empty());
        host.ui_mut().scale_panel(k, 1.0, true);
        assert!(host.step_panel_scale(k, GROUP_SCALE_STEP));
        assert!((host.ui().window_scale(k) - 1.05).abs() < 1e-6);
    }

    /// `ui-scaling.md` §7.7 "Group values": a marked plugin window of group 9 takes the group's
    /// new value after a gesture on a native panel of the group, `GetUIGroupScale(9)` returns
    /// it and `UIScaleChanged` fires once.
    #[test]
    fn plugin_windows_follow_the_group_gesture() {
        let dir = tempfile::tempdir().expect("tempdir");
        let plugins_dir = dir.path().join("Plugins");
        let pkg = plugins_dir.join("T");
        std::fs::create_dir_all(&pkg).expect("mkdir");
        std::fs::write(
            pkg.join("Gs.plugin"),
            "<Plugin><Information><Name>Gs</Name><Author>Tessera</Author>\
             <Version>1</Version></Information><Package>T.Gs</Package>\
             <Configuration Apartment=\"Default\"/></Plugin>",
        )
        .expect("descriptor");
        std::fs::write(
            pkg.join("Gs.lua"),
            "W = Turbine.UI.Window()\n\
             W:SetSize(100, 50)\n\
             W:SetUIScalingGroup(9)\n\
             W:SetVisible(true)\n\
             Changes = {}\n\
             W.UIScaleChanged = function(_, a) Changes[#Changes + 1] = a.Index .. '=' .. string.format('%.2f', a.Value) end\n",
        )
        .expect("plugin");
        let mut plugins = Plugins::start(&plugins_dir, dir.path().join("data"), &[]);
        plugins.data_unavailable();
        let mut host = group_host();
        host.attach_plugins(&mut plugins, &["Gs".to_owned()]);
        let start = std::time::Instant::now();
        while plugins.manager().loaded_plugins().is_empty() {
            plugins.pump();
            plugins.pump_step7();
            host.deliver(&mut plugins);
            assert!(start.elapsed() < Duration::from_secs(30), "plugin load");
            std::thread::sleep(Duration::from_millis(1));
        }
        host.deliver(&mut plugins);
        let eval = |plugins: &Plugins, src: &str| -> String {
            plugins
                .manager()
                .with_lua("Default", |lua| lua.load(src).eval::<String>())
                .expect("apartment")
                .expect("evaluates")
        };
        let k = open_group(&mut host, GROUP_ROOT);
        assert!(host.step_panel_scale(k, GROUP_SCALE_STEP));
        host.deliver(&mut plugins);
        assert_eq!(
            eval(
                &plugins,
                "return tostring(#T.Changes) .. ' ' .. tostring(T.Changes[1])"
            ),
            "1 9=1.05",
            "UIScaleChanged once"
        );
        let got: f64 = eval(
            &plugins,
            "return string.format('%.4f', Turbine.UI.Display.GetUIGroupScale(9))",
        )
        .parse()
        .expect("number");
        assert!((got - 1.05).abs() < 1e-4, "GetUIGroupScale(9) = {got}");
        let w = host.plugin_ui().windows("Default");
        assert_eq!(w.len(), 1, "one window");
        let s = host.ui().node_scale_at_or_above(w[0]).0;
        assert!((s - 1.05).abs() < 1e-4, "the window's node scale {s}");
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
