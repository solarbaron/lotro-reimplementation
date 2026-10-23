// SPDX-License-Identifier: GPL-3.0-or-later
//! The runtime's P3 glue: loads the NPC windows' install names ([`NpcRefs`]) and facts
//! ([`NpcData`]) off the frame, keeps the plugin wallet views' model
//! ([`SessionWallet`]) fed from each frame's player-state changes, and runs the windows
//! ([`NpcUi`]) on the game UI once it is attached.
//!
//! Frame order (windowed client, after `ClientUi::after_update`): [`NpcHub::frame`] reads
//! the UI events of the frame, syncs the windows with the player state, sends the windows'
//! commands and the uses (`USE` on the selection, the world clicks' uses) through the world
//! session, and posts the uses' system lines to the chat window.
//!
//! The use-range cylinders (`interaction.md` §3.4.1 I6a) are read on a worker
//! ([`UseShapes`]) for the bodies the frame needs: the player's, the selection's, the open
//! windows' entities' and a waiting use's target. A use whose cylinders are still being
//! read waits for them, at most [`USE_WAIT`]; then it runs with what has arrived (Decision
//! D13's cylinder for the rest).
//! An open window's walk-away close likewise waits while its entity's or the player's
//! cylinder is still to come (at most [`USE_WAIT`] from when that was first seen). The
//! player's body is its appearance's, else the player state's (`CharacterInfo`); a feed
//! session that names neither leaves the player with D13's cylinder (`interaction.md`
//! §3.4.1 implementer OPEN).
//!
//! The UI events come from the UI host's in-world event list
//! ([`UiHost::take_game_events`]): the caller takes it once per frame and hands the same
//! events to this hub and to the quest journal's; the combat widgets have their own queue.

use std::sync::Arc;
use std::thread::JoinHandle;
use std::time::Instant;

use tessera_dat::ArchiveSet;
use tessera_plugins::{GameplayEvent, PluginManager};
use tessera_ui::game::npc::vendor::BuyToggles;
use tessera_ui::game::npc::{FallbackArt, NpcWindowKind};

use super::commands::{Cylinder, UseOutcome, WorldGeometry, WorldUse, command_of, use_entity};
use super::data::{NpcData, NpcDataError};
use super::prefs::{load_window_prefs, save_window_prefs};
use super::refs::NpcRefs;
use super::shapes::UseShapes;
use super::ui::{NpcKey, NpcUi};
use super::wallet::{SessionWallet, wallet_item_facts};
use crate::character::{CharacterData, PlayerChange, PlayerState};
use crate::ui_host::UiHost;
use tessera_settings::WindowPrefs;

/// Parses an NPC window name of `--open-panel` (`npc-windows.md` §4 Tessera rule:
/// `conversation`, `choice`, `npc-comment`, `vendor`, `vendor-quantity`, `barter`, `loot`,
/// `wallet`; ASCII case ignored).
#[must_use]
pub fn parse_npc_panel(name: &str) -> Option<NpcWindowKind> {
    NpcWindowKind::from_panel_name(&name.trim().to_ascii_lowercase())
}

/// What the names loader returns: the names and Tessera's saved window choices.
type RefsLoad = (Result<NpcRefs, NpcDataError>, Option<WindowPrefs>);

/// Where the windows' changed choices are saved ([`NpcHub::save_prefs`]).
type PrefsSaver = Arc<dyn Fn(WindowPrefs) + Send + Sync>;

/// The runtime's P3 glue (module docs).
#[derive(Default)]
pub struct NpcHub {
    refs: Option<Arc<NpcRefs>>,
    data: Option<Arc<NpcData>>,
    loading_refs: Option<JoinHandle<RefsLoad>>,
    /// Tessera's saved window choices (`[GameWindows]`), read with the names off the frame
    /// ([`Self::start_refs`]); `None` when not read (tests, tools): nothing is then saved.
    saved_prefs: Option<WindowPrefs>,
    /// Where changed choices go: the settings writer once the saved choices were read
    /// ([`super::prefs::save_window_prefs`]), or a test's recorder
    /// ([`Self::set_window_prefs`]).
    saver: Option<PrefsSaver>,
    /// The dialogue font index last saved or restored.
    font_saved: Option<u32>,
    loading_data: Option<JoinHandle<Result<NpcData, NpcDataError>>>,
    data_started: bool,
    wallet: Arc<SessionWallet>,
    ui: Option<NpcUi>,
    pending_panels: Vec<NpcWindowKind>,
    use_requested: bool,
    /// The use-range cylinders of the entities' bodies.
    shapes: UseShapes,
    /// A use waiting for its cylinders: the entity and when it was asked for.
    waiting_use: Option<(tessera_net::EntityId, Instant)>,
    /// The open windows' entities whose cylinders (or the player's) are still being read,
    /// with when that was first seen: their walk-away close waits for them, at most
    /// [`USE_WAIT`] (`interaction.md` §3.4.1 I6a).
    close_waits: std::collections::BTreeMap<tessera_net::EntityId, Instant>,
    /// The outcome of the last use the hub ran (`USE` or a world click).
    last_use: Option<UseOutcome>,
    wallet_toggle: bool,
    failed: Option<String>,
    /// NPC-window keys taken from the input this frame, run in [`Self::frame`].
    keys: Vec<NpcKey>,
    /// The `VendorBuyUI` keys held (§7.4): left and right Alt, left and right Shift, Tab.
    held: [bool; 5],
}

/// Most NPC-window keys queued between two frames (Tessera bound).
const MAX_KEYS: usize = 16;

/// The longest a use waits for its cylinders to be read (Tessera bound).
pub const USE_WAIT: std::time::Duration = std::time::Duration::from_millis(500);

/// The body `id`'s use-range cylinder is built from (`interaction.md` §3.4.1 I6a: "its own
/// body model"): the body its appearance names; for the local player without one, the
/// player state's body (its latest appearance, else its `CharacterInfo`'s); else its class
/// record (`EntityCreated.class`).
fn body_of(state: &PlayerState, id: tessera_net::EntityId) -> Option<tessera_net::Body> {
    let npc = state.npc();
    npc.figure(id)
        .or_else(|| (state.player() == Some(id)).then(|| state.body()).flatten())
        .or_else(|| {
            npc.entity(id)
                .and_then(|e| e.body)
                .map(tessera_net::Body::Object)
        })
}

impl std::fmt::Debug for NpcHub {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("NpcHub")
            .field("refs", &self.refs.is_some())
            .field("data", &self.data.is_some())
            .field("ui", &self.ui)
            .finish_non_exhaustive()
    }
}

impl NpcHub {
    /// Nothing loaded; `panels` (from `--open-panel`) open once the player is in the world.
    #[must_use]
    pub fn new(panels: Vec<NpcWindowKind>) -> Self {
        Self {
            pending_panels: panels,
            ..Self::default()
        }
    }

    /// The plugin wallet views' model.
    #[must_use]
    pub fn wallet(&self) -> &Arc<SessionWallet> {
        &self.wallet
    }

    /// The install facts, once loaded.
    #[must_use]
    pub fn data(&self) -> Option<&Arc<NpcData>> {
        self.data.as_ref()
    }

    /// The install names, once loaded.
    #[must_use]
    pub fn refs(&self) -> Option<&Arc<NpcRefs>> {
        self.refs.as_ref()
    }

    /// The windows, once the UI is attached.
    #[must_use]
    pub fn ui(&self) -> Option<&NpcUi> {
        self.ui.as_ref()
    }

    /// Why the install facts are not available, when loading failed.
    #[must_use]
    pub fn failure(&self) -> Option<&str> {
        self.failed.as_deref()
    }

    /// Tessera's saved window choices, once read ([`Self::start_refs`]); `None` when they
    /// were not read (then no change is saved either).
    #[must_use]
    pub fn saved_prefs(&self) -> Option<&WindowPrefs> {
        self.saved_prefs.as_ref()
    }

    /// Uses `prefs` as Tessera's saved window choices and `saver` for the changed ones
    /// (tests, tools: the settings file is then neither read nor written).
    pub fn set_window_prefs(
        &mut self,
        prefs: WindowPrefs,
        saver: impl Fn(WindowPrefs) + Send + Sync + 'static,
    ) {
        self.font_saved = prefs.dialogue_font;
        self.saved_prefs = Some(prefs);
        self.saver = Some(Arc::new(saver));
    }

    /// Saves the keys `prefs` sets (off the frame, [`super::prefs::save_window_prefs`]),
    /// when the saved choices were read; otherwise nothing is saved.
    pub fn save_prefs(&self, prefs: WindowPrefs) {
        if self.saved_prefs.is_some()
            && let Some(save) = &self.saver
        {
            save(prefs);
        }
    }

    /// The outcome of the last use the hub ran (`USE` on the selection or a world click's
    /// use); `None` before the first.
    #[must_use]
    pub fn last_use(&self) -> Option<&UseOutcome> {
        self.last_use.as_ref()
    }

    /// Starts the use-range cylinder worker with its own loader instead of the install's
    /// (tests, tools; [`UseShapes::start_with`]).
    pub fn start_shapes_with(
        &mut self,
        load: impl FnMut(tessera_net::Body) -> Cylinder + Send + 'static,
    ) {
        self.shapes.start_with(load);
    }

    /// Starts reading the install's names from `set`, and Tessera's saved window choices
    /// (`[GameWindows]`, [`load_window_prefs`]), on a worker thread (once).
    pub fn start_refs(&mut self, set: Arc<ArchiveSet>) {
        if self.loading_refs.is_some() || self.refs.is_some() {
            return;
        }
        match std::thread::Builder::new()
            .name("tessera-p3-refs".into())
            .spawn(move || (NpcRefs::load(&set), load_window_prefs()))
        {
            Ok(h) => self.loading_refs = Some(h),
            Err(e) => self.failed = Some(format!("no loader thread: {e}")),
        }
    }

    /// Starts building the facts over `character` on a worker thread (once).
    pub fn start_data(&mut self, character: Arc<CharacterData>) {
        if self.data_started {
            return;
        }
        self.data_started = true;
        match std::thread::Builder::new()
            .name("tessera-p3-data".into())
            .spawn(move || NpcData::new(character))
        {
            Ok(h) => self.loading_data = Some(h),
            Err(e) => self.failed = Some(format!("no loader thread: {e}")),
        }
    }

    /// Takes what the workers finished (never blocks); `character` starts the facts once
    /// the P1 facts exist.
    pub fn poll(&mut self, character: Option<&Arc<CharacterData>>) {
        if let Some(c) = character {
            self.start_data(Arc::clone(c));
        }
        if self
            .loading_refs
            .as_ref()
            .is_some_and(JoinHandle::is_finished)
            && let Some(h) = self.loading_refs.take()
        {
            match h.join() {
                Ok((refs, prefs)) => {
                    if prefs.is_some() {
                        self.font_saved = prefs.and_then(|p| p.dialogue_font);
                        self.saved_prefs = prefs;
                        self.saver = Some(Arc::new(save_window_prefs));
                    }
                    match refs {
                        Ok(r) => {
                            if !r.missing.is_empty() {
                                tracing::warn!(missing = ?r.missing, "NPC windows without a layout");
                            }
                            self.refs = Some(Arc::new(r));
                        }
                        Err(e) => self.failed = Some(e.to_string()),
                    }
                }
                Err(_) => self.failed = Some("the NPC loader panicked".into()),
            }
        }
        if self
            .loading_data
            .as_ref()
            .is_some_and(JoinHandle::is_finished)
            && let Some(h) = self.loading_data.take()
        {
            match h.join() {
                Ok(Ok(d)) => self.data = Some(Arc::new(d)),
                Ok(Err(e)) => {
                    tracing::warn!("NPC window facts unavailable: {e}");
                    self.failed = Some(e.to_string());
                }
                Err(_) => self.failed = Some("the NPC loader panicked".into()),
            }
        }
    }

    /// Uses already loaded names and facts (tests, tools).
    pub fn set_loaded(&mut self, refs: Arc<NpcRefs>, data: Option<Arc<NpcData>>) {
        self.refs = Some(refs);
        self.data_started = data.is_some();
        self.data = data;
    }

    /// The wallet views' events of one frame's player-state changes, queued on `manager`
    /// for step 7 (`lua-plugin-api.md` §15.3).
    ///
    /// A full batch ([`crate::net::MAX_PLAYER_CHANGES`] changes) means the world may have
    /// dropped older changes before they were taken: the views are then rebuilt from
    /// `wallet` (the player state's wallet after the batch) without events.
    pub fn after_events(
        &mut self,
        changes: &[PlayerChange],
        wallet: &[(tessera_net::domain::Currency, u64)],
        manager: &mut PluginManager,
    ) {
        let data = self.data.clone();
        let facts = |r: &tessera_net::domain::Ref| wallet_item_facts(data.as_deref(), r);
        for ev in self.wallet.apply(changes, &facts) {
            manager.raise_gameplay_event(GameplayEvent::Wallet(ev));
        }
        if changes.len() >= crate::net::MAX_PLAYER_CHANGES {
            self.wallet.resync(wallet, &facts);
        }
    }

    /// The input action `USE` was triggered (`npc-windows.md` §3.1): the selection is used
    /// in the next [`Self::frame`] (with no selection the system line is posted).
    pub fn request_use(&mut self) {
        self.use_requested = true;
    }

    /// The use-range cylinders, as far as they have been read.
    #[must_use]
    pub fn shapes(&self) -> &UseShapes {
        &self.shapes
    }

    /// The cylinders of `ids` known so far, by entity.
    fn cylinders(
        &self,
        state: &PlayerState,
        ids: impl IntoIterator<Item = tessera_net::EntityId>,
    ) -> std::collections::BTreeMap<tessera_net::EntityId, Cylinder> {
        ids.into_iter()
            .filter_map(|id| Some((id, self.shapes.get(body_of(state, id)?)?)))
            .collect()
    }

    /// Whether `id`'s cylinder is still to come ([`UseShapes::awaits`]).
    fn awaits(&self, state: &PlayerState, id: tessera_net::EntityId) -> bool {
        body_of(state, id).is_some_and(|b| self.shapes.awaits(b))
    }

    /// Asks for the cylinders of `ids`; `true` when one of them is still to come.
    fn want_shapes(
        &mut self,
        state: &PlayerState,
        ids: impl IntoIterator<Item = tessera_net::EntityId>,
    ) -> bool {
        let mut waiting = false;
        for id in ids {
            if let Some(b) = body_of(state, id) {
                self.shapes.request(b);
                waiting |= self.shapes.awaits(b);
            }
        }
        waiting
    }

    /// The use of `entity` through the client's one entry point [`use_entity`] (use range
    /// of `interaction.md` §3.4.1 from `avatar`, with the cylinders read so far); the
    /// out-of-range line goes to the chat window.
    fn run_use(
        &mut self,
        host: &mut UiHost,
        world: &mut crate::net::World,
        entity: tessera_net::EntityId,
        avatar: Option<tessera_net::Vec3>,
        now: Instant,
    ) -> UseOutcome {
        let npc = world.player_state().npc();
        let range = npc
            .entity(entity)
            .and_then(|e| e.body)
            .zip(self.data.as_deref())
            .and_then(|(b, d)| d.min_usage_dist(tessera_formats::Did(b.0)));
        let cylinders = self.cylinders(
            world.player_state(),
            world.player().into_iter().chain([entity]),
        );
        let range_of = |id: tessera_net::EntityId| if id == entity { range } else { None };
        let cylinder_of = |id: tessera_net::EntityId| cylinders.get(&id).copied();
        let out = {
            let mut w = WorldUse::new(world, now, &range_of)
                .with_avatar(avatar)
                .with_cylinders(&cylinder_of);
            use_entity(&mut w, entity)
        };
        if let UseOutcome::Refused(e) = &out {
            tracing::info!("UseEntity not sent: {e}");
        }
        if let Some(line) = self.line_of(&out) {
            host.post_notices([line]);
        }
        self.last_use = Some(out.clone());
        out
    }

    /// Uses `entity`, the entity P2's selection just picked (`npc-windows.md` §3.1 rules 4
    /// and 5: a left double click, or a right click on a non-player entity), through the
    /// client's one entry point [`use_entity`] (use range of `interaction.md` §3.4.1 from
    /// `avatar`); the out-of-range line goes to the chat window. The local player is never
    /// used (`None`). When the player's or the entity's cylinder is still being read the
    /// use waits for it ([`UseOutcome::Waiting`]; it runs in a later [`Self::frame`], at
    /// most [`USE_WAIT`] later).
    pub fn use_selection(
        &mut self,
        host: &mut UiHost,
        world: &mut crate::net::World,
        entity: tessera_net::EntityId,
        avatar: Option<tessera_net::Vec3>,
        now: Instant,
    ) -> Option<UseOutcome> {
        if world.player() == Some(entity) {
            return None;
        }
        let ids: Vec<tessera_net::EntityId> = world.player().into_iter().chain([entity]).collect();
        if self.want_shapes(world.player_state(), ids) {
            self.waiting_use = Some((entity, now));
            self.last_use = Some(UseOutcome::Waiting);
            return Some(UseOutcome::Waiting);
        }
        self.waiting_use = None;
        Some(self.run_use(host, world, entity, avatar, now))
    }

    /// `TogglePaperItemPanel` (the character panel's `Player_Wallet_Button`, §9.1) was
    /// triggered; the wallet window opens or closes in the next [`Self::frame`]. Tessera's
    /// key map binds the install's default key (B) to the bags, so no key calls this yet.
    pub fn request_wallet_toggle(&mut self) {
        self.wallet_toggle = true;
    }

    /// One raw input event of the windowed client, before the UI and the walker see it.
    /// Returns `true` when an NPC window takes it (Escape or Enter while one is the active
    /// panel, §5.7, §7.5, §8.3): the caller then gives it to no one else. Tracks the held
    /// `VendorBuyUI` keys (§7.4; never taken).
    pub fn raw_key(&mut self, host: &UiHost, ev: &tessera_input::RawEvent) -> bool {
        use tessera_input::KeyCode as K;
        let tessera_input::RawEvent::Key {
            key,
            pressed,
            repeat,
        } = *ev
        else {
            return false;
        };
        let slot = match key {
            K::AltLeft => Some(0),
            K::AltRight => Some(1),
            K::ShiftLeft => Some(2),
            K::ShiftRight => Some(3),
            K::Tab => Some(4),
            _ => None,
        };
        if let Some(i) = slot {
            self.held[i] = pressed;
            return false;
        }
        let k = match key {
            K::Escape => NpcKey::Escape,
            K::Enter => NpcKey::Accept,
            _ => return false,
        };
        let Some(ui) = self.ui.as_ref() else {
            return false;
        };
        if !pressed || repeat || !ui.takes_key(&host.ui(), k) {
            return false;
        }
        if self.keys.len() < MAX_KEYS {
            self.keys.push(k);
        }
        true
    }

    /// The window lost the keyboard: no `VendorBuyUI` key is held any more.
    pub fn release_keys(&mut self) {
        self.held = [false; 5];
    }

    /// The held buy toggles (§7.4: Alt quantity, Shift stack, Tab fill).
    #[must_use]
    pub fn buy_toggles(&self) -> BuyToggles {
        BuyToggles {
            quantity: self.held[0] || self.held[1],
            stack: self.held[2] || self.held[3],
            fill: self.held[4],
        }
    }

    /// One frame of the windows on `host` (module docs) with the frame's UI `events`.
    /// `screens_open`: the character screens own the UI events this frame (`events` is
    /// ignored). `avatar`: where the walker placed the avatar,
    /// in the session's frame (the use range and the walk-away close measure from it; `None`
    /// before it is placed).
    pub fn frame(
        &mut self,
        host: &mut UiHost,
        world: &mut crate::net::World,
        events: &[tessera_ui::UiEvent],
        screens_open: bool,
        avatar: Option<tessera_net::Vec3>,
        now: Instant,
    ) {
        let Some(refs) = self.refs.clone() else {
            return;
        };
        let toggles = self.buy_toggles();
        if self.ui.is_none() {
            let mut ui = NpcUi::new(Arc::clone(&refs), FallbackArt::default());
            // §5.2 Tessera rule: the saved dialogue font size.
            if let Some(f) = self.font_saved {
                ui.restore_font(&mut host.ui_mut(), f);
            }
            self.ui = Some(ui);
        }
        let events = if screens_open { &[] } else { events };
        let data = self.data.clone();
        let mut lines = Vec::new();
        // The character panel's `Player_Wallet_Button` (§9.1, §9.4).
        if let Some(b) = refs.wallet_button {
            let click = host.ui().vocab().msgs.button_click;
            let clicked = events.iter().any(|e| {
                matches!(e, tessera_ui::UiEvent::Game { message }
                    if message.msg == click && message.source_id == b)
            });
            self.wallet_toggle |= clicked;
        }
        {
            let Some(ui) = self.ui.as_mut() else {
                return;
            };
            if std::mem::take(&mut self.wallet_toggle) && world.player().is_some() {
                ui.toggle_wallet(&mut host.ui_mut(), world.player_state(), data.as_deref());
            }
            if world.player().is_some() {
                for kind in std::mem::take(&mut self.pending_panels) {
                    if let Err(e) = ui.open_panel(
                        &mut host.ui_mut(),
                        kind,
                        world.player_state(),
                        data.as_deref(),
                    ) {
                        tracing::warn!("{}: {e}", kind.panel_name());
                    }
                }
            }
        }
        // `USE` (§3.1): the selection, or the system line without one.
        if std::mem::take(&mut self.use_requested) {
            match world.combat_state().selection() {
                Some(e) => self.waiting_use = Some((e, now)),
                None => {
                    lines.extend(self.line_of(&UseOutcome::NoSelection));
                    self.last_use = Some(UseOutcome::NoSelection);
                }
            }
        }
        // The cylinders this frame needs (interaction.md §3.4.1 I6a), read on the worker.
        if let Some(d) = data.as_ref()
            && !self.shapes.is_running()
        {
            self.shapes.start(Arc::clone(d));
        }
        self.shapes.poll();
        let windows: Vec<tessera_net::EntityId> = world
            .player_state()
            .npc()
            .windows()
            .filter_map(|w| w.content.entity())
            .collect();
        let selection = world.combat_state().selection();
        let wanted: Vec<tessera_net::EntityId> = world
            .player()
            .into_iter()
            .chain(selection)
            .chain(windows.iter().copied())
            .chain(self.waiting_use.map(|(e, _)| e))
            .collect();
        self.want_shapes(world.player_state(), wanted);
        // A window's walk-away close (§5.7) waits while its entity's or the player's
        // cylinder is still to come, at most `USE_WAIT` from when that was first seen.
        let player_waits = world
            .player()
            .is_some_and(|p| self.awaits(world.player_state(), p));
        let mut waits = std::mem::take(&mut self.close_waits);
        waits.retain(|id, _| windows.contains(id));
        for &id in &windows {
            if player_waits || self.awaits(world.player_state(), id) {
                waits.entry(id).or_insert(now);
            } else {
                waits.remove(&id);
            }
        }
        self.close_waits = waits;
        let unsettled: std::collections::BTreeSet<tessera_net::EntityId> = self
            .close_waits
            .iter()
            .filter(|(_, since)| now.saturating_duration_since(**since) < USE_WAIT)
            .map(|(id, _)| *id)
            .collect();
        // `Usage_MinUsageDist` of the body records (interaction.md §3.4.1), only for the
        // entities a range is needed for: the open windows' (§5.7 walk-away close).
        let npc = world.player_state().npc();
        let ranges: std::collections::BTreeMap<tessera_net::EntityId, f32> = match data.as_deref() {
            Some(d) => windows
                .iter()
                .filter_map(|&id| {
                    let body = npc.entity(id)?.body?;
                    Some((id, d.min_usage_dist(tessera_formats::Did(body.0))?))
                })
                .collect(),
            None => std::collections::BTreeMap::new(),
        };
        let range_of = |id: tessera_net::EntityId| ranges.get(&id).copied();
        let cylinders = self.cylinders(
            world.player_state(),
            world.player().into_iter().chain(windows),
        );
        let cylinder_of = |id: tessera_net::EntityId| cylinders.get(&id).copied();
        let known = |id: tessera_net::EntityId| !unsettled.contains(&id);
        let Some(ui) = self.ui.as_mut() else {
            return;
        };
        let actions = {
            let geo = WorldGeometry::new(world, &range_of)
                .with_avatar(avatar)
                .with_cylinders(&cylinder_of)
                .with_known(&known);
            let mut hui = host.ui_mut();
            ui.set_buy_toggles(&mut hui, toggles, data.as_deref());
            let mut actions = Vec::new();
            for k in std::mem::take(&mut self.keys) {
                actions.extend(ui.key(&mut hui, k, data.as_deref()));
            }
            actions.extend(ui.frame(
                &mut hui,
                world.player_state(),
                data.as_deref(),
                events,
                Some(&geo),
            ));
            ui.hide_closed_dialogs(&mut hui);
            actions
        };
        for a in actions {
            if let Some(c) = command_of(a)
                && let Err(e) = world.send_command(now, c)
            {
                tracing::info!("NPC command not sent: {e}");
            }
        }
        if !lines.is_empty() {
            host.post_notices(lines);
        }
        // A changed dialogue font size is saved off the frame (§5.2 Tessera rule).
        let font = self.ui.as_ref().and_then(NpcUi::font_index);
        if font.is_some() && font != self.font_saved {
            self.font_saved = font;
            self.save_prefs(WindowPrefs {
                dialogue_font: font,
                ..WindowPrefs::default()
            });
        }
        // A waiting use runs once its cylinders arrived, or after `USE_WAIT`.
        if let Some((entity, since)) = self.waiting_use {
            let ids: Vec<tessera_net::EntityId> =
                world.player().into_iter().chain([entity]).collect();
            let waiting = self.want_shapes(world.player_state(), ids);
            if !waiting || now.saturating_duration_since(since) >= USE_WAIT {
                self.waiting_use = None;
                self.run_use(host, world, entity, avatar, now);
            }
        }
    }

    /// The system line of a use's outcome: for no selection the install's `Usage` entry
    /// (`npc-windows.md` §3.1, [`NpcData::no_selection_line`]) when it renders, else
    /// Tessera's own text ([`UseOutcome::line`]).
    fn line_of(&self, out: &UseOutcome) -> Option<String> {
        if matches!(out, UseOutcome::NoSelection)
            && let Some(l) = self.data.as_deref().and_then(NpcData::no_selection_line)
        {
            return Some(l.to_owned());
        }
        out.line().map(str::to_owned)
    }

    /// Opens or closes the wallet window (`TogglePaperItemPanel`).
    pub fn toggle_wallet(&mut self, host: &mut UiHost, world: &crate::net::World) {
        let data = self.data.clone();
        if let Some(ui) = self.ui.as_mut() {
            ui.toggle_wallet(&mut host.ui_mut(), world.player_state(), data.as_deref());
        }
    }
}
