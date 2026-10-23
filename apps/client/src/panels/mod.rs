// SPDX-License-Identifier: GPL-3.0-or-later
//! The walker's native panels (ROADMAP M5 exit criterion 2): the fifteen panels of
//! `ui-layout.md` §17 opened from their layout records with Tessera's sandbox data, the chat
//! window, the plugin manager and the Escape rule of `game-panels.md` §16.
//!
//! This module depends only on `tessera-ui`, `tessera-formats`, `tessera-world` and
//! `tessera-assets` (the map lookups' worker pool); the client host owns the [`Ui`], its
//! environment (layouts, images, text) and the input bridge, and mounts [`NativePanels`] on
//! it.
//!
//! **Where it is mounted:** the client's UI host (`ui_host`, used with `--ui` and by the
//! headless combat screens) constructs [`NativePanels`] only when it is built from the
//! install (`UiHost::from_boot`); the host built without an install
//! (`UiHost::without_install`) has no panels. Most panel calls get [`Services`] made for
//! that call: [`WorldMaps`] over the install on a one-worker pool when map data is
//! available ([`NoMaps`] otherwise), and a view of the plugin host's manager as
//! [`PluginControl`]. Opening the combat HUD's panels (`ui_host` `open_combat_panels`)
//! passes [`NoMaps`] and [`NoPlugins`] instead.
//!
//! **Gaps recorded as implementation OPENs** (`ui-layout.md` §17.5, items (d) and (e)): P7's
//! option pages stay empty (the specs do not name the slider and check-box templates or the
//! pages by element-ID label), and P12's info tabs keep their layout state (the about tab
//! and the options pane are not named either; `ui-widgets.md` §13 OPEN (P12)).
//!
//! | Module | What | Spec |
//! |---|---|---|
//! | [`catalog`] | P1–P15, their layout and root labels, lookup by label | UL §17.1 step 1, §17.2 |
//! | [`census`] | element and art-surface counts of an instantiated panel | UL §17.2 |
//! | [`maps`] | the minimap and world-map data source over the install (on the asset workers) | GP §8.4.2, §8.5.1 |
//! | [`chat`] | the chat window P2: setup, windows, the entry line handed to the command dispatcher | `chat-window.md` §3, §6, §10; decision 0020 |
//! | [`plugin_manager`] | the plugin manager P12 (game layout or Tessera's own window) | UL §17.4 P12, LPA §6.4–§6.7 |
//!
//! # How the host drives it
//!
//! 1. Build the [`PanelCatalog`] ([`PanelCatalog::resolve`]) and the game layouts
//!    ([`catalog::game_layouts`]) from the `UILAYOUT` map and the `UIElementID` labels;
//!    create [`NativePanels::new`]; resolve [`NativePanels::types`] by label; optionally
//!    [`NativePanels::set_chat`] with a [`chat::ChatSetup`] (before P2 opens; later:
//!    [`NativePanels::reset_chat`]).
//! 2. [`NativePanels::open_hud`] once the UI exists; on-demand panels with
//!    [`NativePanels::open`] / [`NativePanels::toggle`].
//! 3. Every [`UiEvent`] taken from the UI: [`NativePanels::handle_event`].
//! 4. Every position update: [`NativePanels::update_position`].
//! 5. A `menu` press the UI declined: [`NativePanels::escape`] (never quits).
//! 6. After closing or destroying elements: [`NativePanels::prune`].

pub mod catalog;
pub mod census;
pub mod chat;
pub mod maps;
pub mod plugin_manager;

use std::collections::{BTreeMap, BTreeSet};

pub use catalog::{MissingPart, PanelCatalog, PanelId, PanelRef};
pub use census::{PanelCensus, panel_census};
pub use chat::{ChatPanel, ChatSetup};
pub use maps::{NoMaps, WorldMaps};
pub use plugin_manager::{NoPlugins, PluginControl, PluginManagerPanel, PluginRow};

use tessera_ui::game::escape::{EscapeOutcome, EscapePanels, declined_escape};
use tessera_ui::game::radar::RadarInput;
use tessera_ui::game::{
    GameAction, GameError, GameLayouts, GamePanels, GameVocab, MapSource, NoShortcuts, SandboxData,
    Shortcuts,
};
use tessera_ui::{ElementKey, Ui, UiError, UiEvent};

/// Errors of the native panels.
#[derive(Debug, Clone, PartialEq, Eq, thiserror::Error)]
pub enum PanelsError {
    /// The panel was not found in the install (`ui-layout.md` §17.1 step 1).
    #[error("panel {0} is not in the catalogue")]
    NotInCatalogue(PanelId),
    /// A layout record is missing or does not decode.
    #[error("layout {0:#010x} is not available")]
    NoLayout(u32),
    /// The root element is not a top-level element of its layout (UL16).
    #[error("element {0:#010x} is not a top-level element of the layout")]
    NoRoot(u32),
    /// The root element's base chain does not resolve (UL §13).
    #[error("element {0:#010x} does not resolve")]
    Unresolved(u32),
    /// The chat window was asked for but no chat setup was given.
    #[error("no chat window configured")]
    NoChat,
    /// [`NativePanels::set_chat`] while the chat window is open (use
    /// [`NativePanels::reset_chat`]).
    #[error("the chat window is open")]
    ChatOpen,
    /// A record the panels read is missing or does not decode.
    #[error("{0} is missing or malformed")]
    Record(&'static str),
    /// The UI core refused an operation.
    #[error(transparent)]
    Ui(#[from] UiError),
    /// A game widget failed.
    #[error(transparent)]
    Game(#[from] GameError),
}

/// The services the host lends to the panels for one call.
pub struct Services<'a> {
    /// Minimap and world-map records.
    pub maps: &'a mut dyn MapSource,
    /// The plugin host.
    pub plugins: &'a mut dyn PluginControl,
}

impl std::fmt::Debug for Services<'_> {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("Services").finish_non_exhaustive()
    }
}

/// Something the host should know about after an event.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum PanelAction {
    /// A panel was opened or shown.
    Opened(PanelId),
    /// A game widget asked for something (already handled by the panels).
    Game(GameAction),
}

/// Element types the panels' own code looks up by label (`UIElementType` mapper labels; the
/// defaults are placeholders in a range no data uses, like `tessera-ui`'s game types).
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct PanelTypes {
    /// `BuyNowButton`: P15's store button, hidden in the sandbox (`ui-layout.md` §17.4 P15,
    /// Tessera choice).
    pub store_button: u32,
}

impl Default for PanelTypes {
    fn default() -> Self {
        Self {
            store_button: 0xFFFC_0100,
        }
    }
}

impl PanelTypes {
    /// Every `(label, value)` pair.
    #[must_use]
    pub fn entries(&self) -> Vec<(&'static str, u32)> {
        vec![("BuyNowButton", self.store_button)]
    }

    /// Replaces each value whose label `lookup` resolves (the `UIElementType` chain).
    pub fn resolve_with(&mut self, lookup: &mut dyn FnMut(&str) -> Option<u32>) {
        if let Some(v) = lookup("BuyNowButton") {
            self.store_button = v;
        }
    }
}

/// The native panels of the walker.
pub struct NativePanels {
    catalog: PanelCatalog,
    /// The game widgets of the opened panels.
    pub game: GamePanels,
    /// The chat window, when configured.
    pub chat: Option<ChatPanel>,
    /// The plugin manager.
    pub manager: PluginManagerPanel,
    /// The sandbox player and account (`game-panels.md` §2: everything absent).
    pub data: SandboxData,
    /// Element types of the panels' own rules.
    pub types: PanelTypes,
    shortcuts: Box<dyn Shortcuts>,
    roots: BTreeMap<PanelId, ElementKey>,
    last_input: RadarInput,
    last_area: Option<u32>,
    target_selected: bool,
    buttons: BTreeMap<u32, PanelId>,
    /// Panels hidden by the panels' own code; with an auto-fade (`ui-behaviour.md` §11.1)
    /// such a panel keeps its visible flag until the fade ends, yet counts as hidden.
    hiding: BTreeSet<PanelId>,
    /// The P5 map window replaces P10 for `MapZoom` (decision 0024 P5,
    /// [`Self::set_world_map_external`]).
    world_map_external: std::cell::Cell<bool>,
    /// `MapZoom` requests not yet taken by the P5 map window.
    world_map_requests: std::cell::Cell<u32>,
}

impl std::fmt::Debug for NativePanels {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("NativePanels")
            .field("catalogue", &self.catalog.len())
            .field("open", &self.roots.keys().collect::<Vec<_>>())
            .field("chat", &self.chat.is_some())
            .finish_non_exhaustive()
    }
}

impl NativePanels {
    /// Panels for `catalog`, with the game vocabulary and the layouts game code creates
    /// elements from. The sandbox shortcuts are all empty.
    #[must_use]
    pub fn new(catalog: PanelCatalog, vocab: GameVocab, layouts: GameLayouts) -> Self {
        Self {
            catalog,
            game: GamePanels::new(vocab, layouts),
            chat: None,
            manager: PluginManagerPanel::default(),
            data: SandboxData::default(),
            types: PanelTypes::default(),
            shortcuts: Box::new(NoShortcuts),
            roots: BTreeMap::new(),
            last_input: RadarInput::default(),
            last_area: None,
            target_selected: false,
            buttons: BTreeMap::new(),
            hiding: BTreeSet::new(),
            world_map_external: std::cell::Cell::new(false),
            world_map_requests: std::cell::Cell::new(0),
        }
    }

    /// Hands the minimap's `MapZoom` (`game-panels.md` §8.7.5) to the P5 map window
    /// (`map-windows.md` MW28) instead of opening P10: each request is counted for
    /// [`Self::take_world_map_requests`].
    pub fn set_world_map_external(&self, on: bool) {
        self.world_map_external.set(on);
    }

    /// The `MapZoom` requests since the last call (P5 map window).
    #[must_use]
    pub fn take_world_map_requests(&self) -> u32 {
        self.world_map_requests.replace(0)
    }

    /// The catalogue.
    #[must_use]
    pub fn catalog(&self) -> &PanelCatalog {
        &self.catalog
    }

    /// Configures the chat window P2 for the `chatwindow` layout of the catalogue. Call
    /// before P2 opens; once it is open use [`NativePanels::reset_chat`].
    ///
    /// # Errors
    /// [`PanelsError::NotInCatalogue`] when P2 was not found, [`PanelsError::ChatOpen`]
    /// when the chat window is open (nothing changes).
    pub fn set_chat(&mut self, setup: ChatSetup) -> Result<(), PanelsError> {
        let r = self
            .catalog
            .get(PanelId::Chat)
            .ok_or(PanelsError::NotInCatalogue(PanelId::Chat))?;
        if self.roots.contains_key(&PanelId::Chat)
            || self
                .chat
                .as_ref()
                .is_some_and(|c| c.primary_root().is_some())
        {
            return Err(PanelsError::ChatOpen);
        }
        self.chat = Some(ChatPanel::new(self.game.vocab.clone(), r.layout, setup));
        Ok(())
    }

    /// Replaces the chat setup while the UI exists: destroys the old chat windows (every
    /// window, not only the primary), configures the new setup, and opens the new windows
    /// when P2 was shown. Returns the new root when it was reopened. Lines the old panel
    /// queued for the plugins are dropped with it; drain them first.
    ///
    /// # Errors
    /// [`PanelsError::NotInCatalogue`] when P2 was not found, or the error of
    /// [`NativePanels::open`] when reopening fails (the new setup stays configured).
    pub fn reset_chat(
        &mut self,
        ui: &mut Ui,
        setup: ChatSetup,
        sv: &mut Services<'_>,
    ) -> Result<Option<ElementKey>, PanelsError> {
        if self.catalog.get(PanelId::Chat).is_none() {
            return Err(PanelsError::NotInCatalogue(PanelId::Chat));
        }
        let was_shown = self.is_shown(ui, PanelId::Chat);
        self.close_chat(ui);
        self.set_chat(setup)?;
        if was_shown {
            return self.open(ui, PanelId::Chat, sv).map(Some);
        }
        Ok(None)
    }

    /// Destroys every chat window and drops P2's bookkeeping.
    fn close_chat(&mut self, ui: &mut Ui) {
        if let Some(c) = self.chat.as_mut() {
            c.close(ui);
        }
        self.roots.remove(&PanelId::Chat);
        self.hiding.remove(&PanelId::Chat);
        self.prune(ui);
    }

    /// Replaces the quickslot shortcuts (default: every slot empty).
    pub fn set_shortcuts(&mut self, s: Box<dyn Shortcuts>) {
        self.shortcuts = s;
    }

    /// The root of an opened panel (for P12, Tessera's own window when that is in use).
    #[must_use]
    pub fn root(&self, id: PanelId) -> Option<ElementKey> {
        match self.roots.get(&id) {
            Some(k) => Some(*k),
            None if id == PanelId::PluginManager && self.manager.is_own() => self.manager.root(),
            None => None,
        }
    }

    /// The opened panels and their roots.
    pub fn opened(&self) -> impl Iterator<Item = (PanelId, ElementKey)> + '_ {
        self.roots.iter().map(|(i, k)| (*i, *k))
    }

    /// Whether a panel is open and shown. A panel the panels hid that is still fading out
    /// (`ui-behaviour.md` §11.1) counts as hidden.
    #[must_use]
    pub fn is_shown(&self, ui: &Ui, id: PanelId) -> bool {
        if id == PanelId::PluginManager && self.manager.is_own() {
            return self.manager.is_shown(ui);
        }
        self.root(id).is_some_and(|k| {
            ui.exists(k) && ui.is_visible(k) && !(self.hiding.contains(&id) && ui.is_fading(k))
        })
    }

    fn id_of(&self, k: ElementKey) -> Option<PanelId> {
        self.roots.iter().find(|(_, r)| **r == k).map(|(i, _)| *i)
    }

    fn mark_hidden(&mut self, id: PanelId, hidden: bool) {
        if hidden {
            self.hiding.insert(id);
        } else {
            self.hiding.remove(&id);
        }
    }

    /// Opens a panel by `ui-layout.md` §17.1, or shows it again when it is open:
    ///
    /// 1. the root from the catalogue (P2 through the chat window's configuration,
    ///    `chat-window.md` §3.3);
    /// 2. created as a root panel (§9.1; the UI resolves, creates and lays it out);
    /// 3. Tessera's saved location applied (§9.9);
    /// 4. the game widgets attached with the sandbox data (`game-panels.md` §3);
    /// 5. per panel: P5 hidden while nothing is selected (§17.4 Tessera choice), P10 shows
    ///    the map for the last position (`game-panels.md` §9.4), P12 lists the plugins, P15's
    ///    store button (`BuyNowButton`) is hidden (§17.4 Tessera choice).
    ///
    /// An on-demand panel becomes the active panel. P12 without a layout in the install
    /// opens Tessera's own plugin manager window. P2 whose windows were destroyed is built
    /// again.
    ///
    /// # Errors
    /// [`PanelsError::NotInCatalogue`], [`PanelsError::NoChat`], or the UI's error (for P2
    /// the chat window's creation error).
    pub fn open(
        &mut self,
        ui: &mut Ui,
        id: PanelId,
        sv: &mut Services<'_>,
    ) -> Result<ElementKey, PanelsError> {
        if id == PanelId::PluginManager && (self.catalog.get(id).is_none() || self.manager.is_own())
        {
            // No P12 layout (or it failed before): Tessera's own window.
            return self.manager.open_own(ui, &*sv.plugins);
        }
        if id == PanelId::Chat
            && let Some(c) = self.chat.as_ref()
            && c.primary_root().is_some()
            && !c.is_open(ui)
        {
            // The windows were destroyed: forget them and build them again.
            self.close_chat(ui);
        }
        if let Some(k) = self.roots.get(&id).copied().filter(|k| ui.exists(*k)) {
            let show = id != PanelId::TargetVitals || self.target_selected;
            ui.set_shown(k, show);
            self.mark_hidden(id, !show);
            if id == PanelId::PluginManager {
                self.manager.rebuild(ui, &*sv.plugins);
            }
            if id.is_on_demand() {
                ui.request_activation(k);
            }
            return Ok(k);
        }
        let k = if id == PanelId::Chat {
            if self.catalog.get(PanelId::Chat).is_none() {
                return Err(PanelsError::NotInCatalogue(PanelId::Chat));
            }
            let chat = self.chat.as_mut().ok_or(PanelsError::NoChat)?;
            if chat.primary_root().is_none()
                && let Err(e) = chat.open(ui)
            {
                // Windows that could be created stay; without a primary window P2 failed.
                if chat.primary_root().is_none() {
                    return Err(e);
                }
                tracing::debug!("chat window: {e}");
            }
            chat.primary_root().ok_or(PanelsError::NoChat)?
        } else {
            let r = self
                .catalog
                .get(id)
                .ok_or(PanelsError::NotInCatalogue(id))?;
            ui.open_panel(r.layout, r.element_id)?
        };
        ui.apply_saved(k);
        self.game.attach(ui, k, &self.data, &*self.shortcuts);
        match id {
            PanelId::TargetVitals => ui.set_shown(k, self.target_selected),
            PanelId::WorldMap => {
                if let Err(e) =
                    self.game
                        .attach_world_map(ui, k, self.last_area, &self.last_input, sv.maps)
                {
                    tracing::debug!("world map: {e}");
                }
            }
            PanelId::PluginManager => self.manager.attach_layout(ui, k, &*sv.plugins),
            PanelId::Bag => {
                let store = self.types.store_button;
                for e in tessera_ui::game::subtree(ui, k) {
                    if ui.element_type(e) == Some(store) {
                        ui.set_shown(e, false);
                    }
                }
            }
            _ => {}
        }
        if id.is_on_demand() {
            // Activation requested by code (`ui-behaviour.md` §5.1 step 2).
            ui.request_activation(k);
        }
        self.roots.insert(id, k);
        self.mark_hidden(id, id == PanelId::TargetVitals && !self.target_selected);
        Ok(k)
    }

    /// Attaches the game widgets (`game-panels.md` §3: drag boxes, conditional fields,
    /// quickslot bars, ...) to a root opened outside the catalogue (the combat panels of
    /// `gameplayactivatables`, decision 0024 P2).
    pub fn attach_game_widgets(&mut self, ui: &mut Ui, k: ElementKey) {
        self.game.attach(ui, k, &self.data, &*self.shortcuts);
    }

    /// Opens the HUD panels (P1–P5, P11, P14; P2 only with a chat setup). Returns each
    /// panel's result.
    pub fn open_hud(
        &mut self,
        ui: &mut Ui,
        sv: &mut Services<'_>,
    ) -> Vec<(PanelId, Result<ElementKey, PanelsError>)> {
        let has_chat = self.chat.is_some();
        let mut out = Vec::new();
        for id in PanelId::HUD {
            if id != PanelId::Chat || has_chat {
                out.push((id, self.open(ui, id, sv)));
            }
        }
        out
    }

    /// Opens the plugin manager: P12 from the game layout when the install has it,
    /// otherwise Tessera's own window.
    ///
    /// # Errors
    /// As [`NativePanels::open`] and [`PluginManagerPanel::open_own`].
    pub fn open_plugin_manager(
        &mut self,
        ui: &mut Ui,
        sv: &mut Services<'_>,
    ) -> Result<ElementKey, PanelsError> {
        if self.catalog.get(PanelId::PluginManager).is_some() {
            match self.open(ui, PanelId::PluginManager, sv) {
                Ok(k) => return Ok(k),
                Err(e) => tracing::debug!("plugin manager layout: {e}; Tessera's own window"),
            }
        }
        self.manager.open_own(ui, &*sv.plugins)
    }

    /// Hides a panel (it stays built).
    pub fn hide(&mut self, ui: &mut Ui, id: PanelId) {
        if id == PanelId::PluginManager && self.manager.is_own() {
            self.manager.hide(ui);
            return;
        }
        if let Some(k) = self.roots.get(&id).copied() {
            ui.set_shown(k, false);
            self.mark_hidden(id, true);
        }
    }

    /// Shows a hidden panel (opening it first if needed) or hides a shown one. Returns
    /// whether it is shown afterwards.
    ///
    /// # Errors
    /// As [`NativePanels::open`].
    pub fn toggle(
        &mut self,
        ui: &mut Ui,
        id: PanelId,
        sv: &mut Services<'_>,
    ) -> Result<bool, PanelsError> {
        if self.is_shown(ui, id) {
            self.hide(ui, id);
            Ok(false)
        } else {
            self.open(ui, id, sv)?;
            Ok(self.is_shown(ui, id))
        }
    }

    /// Destroys a panel's elements (for P2 every chat window).
    pub fn destroy(&mut self, ui: &mut Ui, id: PanelId) {
        if id == PanelId::Chat {
            self.close_chat(ui);
            return;
        }
        if let Some(k) = self.roots.remove(&id) {
            ui.destroy(k);
        }
        self.prune(ui);
    }

    /// Drops the bookkeeping of destroyed elements (call after closing or destroying).
    pub fn prune(&mut self, ui: &mut Ui) {
        self.game.prune(ui);
        self.roots.retain(|_, k| ui.exists(*k));
        let roots = &self.roots;
        self.hiding.retain(|id| roots.contains_key(id));
    }

    /// Selects or clears the target (P5 is shown only while a target is selected, the
    /// Tessera choice of `ui-layout.md` §17.4).
    pub fn set_target(&mut self, ui: &mut Ui, selected: bool) {
        self.target_selected = selected;
        if let Some(k) = self.root(PanelId::TargetVitals) {
            ui.set_shown(k, selected);
            self.mark_hidden(PanelId::TargetVitals, !selected);
        }
    }

    /// Binds a button (by element ID) to a panel: a click toggles the panel. For the
    /// toolbar's menu buttons, whose panel assignment the specs do not name yet
    /// (`ui-layout.md` §17.5 OPEN).
    pub fn bind_button(&mut self, ui: &mut Ui, element_id: u32, id: PanelId) {
        if self.buttons.insert(element_id, id).is_none() {
            ui.register_handler(element_id, ui.vocab().msgs.button_click);
        }
    }

    /// A position update (`game-panels.md` §8.3, §9.3): the minimap and the world map
    /// follow; `area` is the `Area_DID` of the player's block, used when P10 opens.
    pub fn update_position(
        &mut self,
        ui: &mut Ui,
        input: &RadarInput,
        area: Option<u32>,
        maps: &mut dyn MapSource,
    ) {
        self.last_input = input.clone();
        self.last_area = area;
        self.game.update_radar(ui, input, maps);
        self.game.update_world_map(ui, input, maps);
    }

    /// The input action `ToggleHiddenDragBoxes` (`game-panels.md` §4.3).
    pub fn toggle_drag_boxes(&mut self, ui: &mut Ui) {
        self.game.toggle_drag_boxes(ui);
    }

    /// Routes one UI event to the game widgets, the chat window and the plugin manager,
    /// and runs what they ask for (`MapZoom` opens P10, `/plugins manager` opens the
    /// manager, bound buttons toggle their panel).
    pub fn handle_event(
        &mut self,
        ui: &mut Ui,
        ev: &UiEvent,
        sv: &mut Services<'_>,
    ) -> Vec<PanelAction> {
        let mut out = Vec::new();
        for a in self.game.handle_event(ui, ev) {
            out.push(PanelAction::Game(a));
            if a == GameAction::OpenWorldMap && self.world_map_external.get() {
                self.world_map_requests
                    .set(self.world_map_requests.get().saturating_add(1));
            } else if a == GameAction::OpenWorldMap {
                match self.open(ui, PanelId::WorldMap, sv) {
                    Ok(_) => out.push(PanelAction::Opened(PanelId::WorldMap)),
                    Err(e) => tracing::debug!("world map: {e}"),
                }
            }
        }
        if let Some(c) = self.chat.as_mut() {
            c.handle_event(ui, ev);
        }
        self.manager.handle_event(ui, ev, sv.plugins);
        if let UiEvent::Game { message } = ev
            && message.msg == ui.vocab().msgs.button_click
            && let Some(id) = self.buttons.get(&message.source_id).copied()
        {
            match self.toggle(ui, id, sv) {
                Ok(true) => out.push(PanelAction::Opened(id)),
                Ok(false) => {}
                Err(e) => tracing::debug!("{id}: {e}"),
            }
        }
        out
    }

    /// The panels the Escape rule may hide: the open on-demand panels (the own
    /// plugin-manager window included) and the main menu.
    fn escape_panels(&self) -> EscapePanels {
        let mut on_demand: Vec<ElementKey> = self
            .roots
            .iter()
            .filter(|(id, _)| id.is_on_demand() && **id != PanelId::MainMenu)
            .map(|(_, k)| *k)
            .collect();
        if let Some(w) = self.manager.root().filter(|_| self.manager.is_own()) {
            on_demand.push(w);
        }
        EscapePanels {
            on_demand,
            main_menu: self.root(PanelId::MainMenu),
        }
    }

    /// Whether step 1 of the Escape rule (`game-panels.md` §16) finds a panel to hide: the
    /// active panel is a visible on-demand panel, the main menu P6 and the own
    /// plugin-manager window included (`combat-ui.md` TR5 runs its combat step only
    /// otherwise).
    #[must_use]
    pub fn active_on_demand(&self, ui: &Ui) -> bool {
        let p = self.escape_panels();
        ui.active_panel()
            .filter(|a| ui.exists(*a) && ui.is_visible(*a))
            .is_some_and(|a| p.on_demand.contains(&a) || p.main_menu == Some(a))
    }

    /// The Escape rule (`game-panels.md` §16) for a `menu` command the UI declined: a press
    /// that is not an auto-repeat hides the active on-demand panel, or else shows or hides
    /// the main menu P6 (opening it the first time). It never quits.
    pub fn escape(
        &mut self,
        ui: &mut Ui,
        press: bool,
        repeat: bool,
        sv: &mut Services<'_>,
    ) -> EscapeOutcome {
        let panels = self.escape_panels();
        let out = declined_escape(ui, &panels, press, repeat);
        match out {
            EscapeOutcome::HidActive(k) | EscapeOutcome::HidMainMenu(k) => {
                if let Some(id) = self.id_of(k) {
                    self.mark_hidden(id, true);
                }
            }
            EscapeOutcome::ShowedMainMenu(_) => self.mark_hidden(PanelId::MainMenu, false),
            _ => {}
        }
        if out == EscapeOutcome::OpenMainMenu {
            match self.open(ui, PanelId::MainMenu, sv) {
                Ok(k) => {
                    ui.activate(k);
                    return EscapeOutcome::ShowedMainMenu(k);
                }
                Err(e) => tracing::debug!("main menu: {e}"),
            }
        }
        out
    }
}
