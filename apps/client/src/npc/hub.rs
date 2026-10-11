// SPDX-License-Identifier: GPL-3.0-or-later
//! The runtime's P3 glue: loads the NPC windows' install names ([`NpcRefs`]) and facts
//! ([`NpcData`]) off the frame, keeps the plugin wallet views' model
//! ([`SessionWallet`]) fed from each frame's player-state changes, and runs the windows
//! ([`NpcUi`]) on the game UI once it is attached.
//!
//! Frame order (windowed client, after `ClientUi::after_update`): [`NpcHub::frame`] takes
//! the UI events of the frame, syncs the windows with the player state, sends the windows'
//! commands and the input action's use through the world session, and posts the use's
//! system lines to the chat window.
//!
//! The UI events reach the hub through the UI host's event collector (the one the
//! character screens use while they are open; the hub turns it on once they are closed).
//! A dedicated hook in the UI host, beside the character panels' queue, is the merge-time
//! replacement.

use std::sync::Arc;
use std::thread::JoinHandle;
use std::time::Instant;

use tessera_dat::ArchiveSet;
use tessera_plugins::{GameplayEvent, PluginManager};
use tessera_ui::game::npc::vendor::BuyToggles;
use tessera_ui::game::npc::{FallbackArt, NpcWindowKind};

use super::commands::{
    NOTHING_NEAR_LINE, UseOutcome, WorldGeometry, WorldUse, command_of, nearest_npc, use_entity,
};
use super::data::{NpcData, NpcDataError};
use super::refs::NpcRefs;
use super::ui::{NpcKey, NpcUi};
use super::wallet::{SessionWallet, WalletPlayer, wallet_item_facts};
use crate::character::{CharacterData, PlayerChange};
use crate::ui_host::UiHost;
use crate::ui_host::services::WalkerPlayer;

/// Parses an NPC window name of `--open-panel` (`npc-windows.md` §4 Tessera rule:
/// `conversation`, `choice`, `npc-comment`, `vendor`, `vendor-quantity`, `barter`, `loot`,
/// `wallet`; ASCII case ignored).
#[must_use]
pub fn parse_npc_panel(name: &str) -> Option<NpcWindowKind> {
    NpcWindowKind::from_panel_name(&name.trim().to_ascii_lowercase())
}

/// The runtime's P3 glue (module docs).
#[derive(Default)]
pub struct NpcHub {
    refs: Option<Arc<NpcRefs>>,
    data: Option<Arc<NpcData>>,
    loading_refs: Option<JoinHandle<Result<NpcRefs, NpcDataError>>>,
    loading_data: Option<JoinHandle<Result<NpcData, NpcDataError>>>,
    data_started: bool,
    wallet: Arc<SessionWallet>,
    player: Arc<WalkerPlayer>,
    ui: Option<NpcUi>,
    pending_panels: Vec<NpcWindowKind>,
    use_requested: bool,
    wallet_toggle: bool,
    collecting: bool,
    failed: Option<String>,
    /// NPC-window keys taken from the input this frame, run in [`Self::frame`].
    keys: Vec<NpcKey>,
    /// The `VendorBuyUI` keys held (§7.4): left and right Alt, left and right Shift, Tab.
    held: [bool; 5],
    /// [`Self::player_source`], made once.
    source: std::sync::OnceLock<Arc<dyn tessera_plugins::LocalPlayerSource>>,
    /// The UI events of the last [`Self::frame`] (shared with the quest journal, P3).
    frame_events: Vec<tessera_ui::UiEvent>,
}

/// Most NPC-window keys queued between two frames (Tessera bound).
const MAX_KEYS: usize = 16;

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

    /// Starts reading the install's names from `set` on a worker thread (once).
    pub fn start_refs(&mut self, set: Arc<ArchiveSet>) {
        if self.loading_refs.is_some() || self.refs.is_some() {
            return;
        }
        match std::thread::Builder::new()
            .name("tessera-p3-refs".into())
            .spawn(move || NpcRefs::load(&set))
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
                Ok(Ok(r)) => {
                    if !r.missing.is_empty() {
                        tracing::warn!(missing = ?r.missing, "NPC windows without a layout");
                    }
                    self.refs = Some(Arc::new(r));
                }
                Ok(Err(e)) => self.failed = Some(e.to_string()),
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

    /// The input action "use nearest NPC in range" was triggered; it runs in the next
    /// [`Self::frame`].
    pub fn request_use(&mut self) {
        self.use_requested = true;
    }

    /// `TogglePaperItemPanel` (the character panel's `Player_Wallet_Button`, §9.1) was
    /// triggered; the wallet window opens or closes in the next [`Self::frame`]. Tessera's
    /// key map binds the install's default key (B) to the bags, so no key calls this yet.
    pub fn request_wallet_toggle(&mut self) {
        self.wallet_toggle = true;
    }

    /// The local player the plugins see, with the wallet views (`gameplay::LocalPlayerSource`).
    #[must_use]
    pub fn player_source(&self) -> Arc<dyn tessera_plugins::LocalPlayerSource> {
        WalletPlayer::new(Arc::clone(&self.player), Arc::clone(&self.wallet))
    }

    /// Makes [`Self::player_source`] the plugins' local player. The windowed client calls
    /// it every frame once the UI is attached, right after the UI host attaches the
    /// plugins (which installs its own player without the wallet views) and before step 7
    /// runs the plugins, so plugins never see a player without the wallet views, also
    /// after a re-attach.
    pub fn install_source(&self, plugins: &mut crate::plugins::Plugins) {
        plugins
            .manager_mut()
            .set_local_player_source(Some(Arc::clone(
                self.source.get_or_init(|| self.player_source()),
            )));
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

    /// One frame of the windows on `host` (module docs). `screens_open`: the character
    /// screens own the UI events this frame. `avatar`: where the walker placed the avatar,
    /// in the session's frame (the use range and the walk-away close measure from it; `None`
    /// before it is placed).
    pub fn frame(
        &mut self,
        host: &mut UiHost,
        world: &mut crate::net::World,
        plugins: &mut crate::plugins::Plugins,
        screens_open: bool,
        avatar: Option<tessera_net::Vec3>,
        now: Instant,
    ) {
        self.frame_events.clear();
        // The plugins' local player carries the wallet views.
        let name = world
            .player()
            .and_then(|id| world.entities().get(&id))
            .map(|e| e.name.clone());
        self.player.set(name.as_deref());
        self.install_source(plugins);
        let Some(refs) = self.refs.clone() else {
            return;
        };
        let toggles = self.buy_toggles();
        let ui = self
            .ui
            .get_or_insert_with(|| NpcUi::new(Arc::clone(&refs), FallbackArt::default()));
        let events = if screens_open {
            self.collecting = false;
            Vec::new()
        } else if self.collecting {
            host.take_screen_events()
        } else {
            host.collect_screen_events(true);
            self.collecting = true;
            Vec::new()
        };
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
        // `Usage_MinUsageDist` of the body records (interaction.md §3.4.1), only for the
        // entities a range is needed for: the open windows' (§5.7 walk-away close) and, on
        // a use request, every known one.
        let use_requested = std::mem::take(&mut self.use_requested);
        let npc = world.player_state().npc();
        let needed: Vec<tessera_net::EntityId> = if use_requested {
            npc.entities().map(|(id, _)| id).collect()
        } else {
            npc.windows().filter_map(|w| w.content.entity()).collect()
        };
        let ranges: std::collections::BTreeMap<tessera_net::EntityId, f32> = match data.as_deref() {
            Some(d) => needed
                .into_iter()
                .filter_map(|id| {
                    let body = npc.entity(id)?.body?;
                    Some((id, d.min_usage_dist(tessera_formats::Did(body.0))?))
                })
                .collect(),
            None => std::collections::BTreeMap::new(),
        };
        let range_of = |id: tessera_net::EntityId| ranges.get(&id).copied();
        let actions = {
            let geo = WorldGeometry::new(world, &range_of).with_avatar(avatar);
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
                &events,
                Some(&geo),
            ));
            ui.hide_closed_dialogs(&mut hui);
            actions
        };
        // The journal windows read the same events after this frame (P3).
        self.frame_events = events;
        for a in actions {
            if let Some(c) = command_of(a)
                && let Err(e) = world.send_command(now, c)
            {
                tracing::info!("NPC command not sent: {e}");
            }
        }
        if use_requested {
            let mut w = WorldUse::new(world, now, &range_of).with_avatar(avatar);
            match nearest_npc(&w) {
                Some(id) => {
                    let out = use_entity(&mut w, id);
                    if let UseOutcome::Refused(e) = &out {
                        tracing::info!("UseEntity not sent: {e}");
                    }
                    lines.extend(out.line().map(str::to_owned));
                }
                None => lines.push(NOTHING_NEAR_LINE.to_owned()),
            }
        }
        if !lines.is_empty() {
            host.post_notices(lines);
        }
    }

    /// The UI events the last [`Self::frame`] collected (empty while the character screens
    /// own them): the quest journal's windows take them after the NPC windows (P3,
    /// `crate::quests::hub`).
    #[must_use]
    pub fn frame_events(&self) -> &[tessera_ui::UiEvent] {
        &self.frame_events
    }

    /// Opens or closes the wallet window (`TogglePaperItemPanel`).
    pub fn toggle_wallet(&mut self, host: &mut UiHost, world: &crate::net::World) {
        let data = self.data.clone();
        if let Some(ui) = self.ui.as_mut() {
            ui.toggle_wallet(&mut host.ui_mut(), world.player_state(), data.as_deref());
        }
    }
}
