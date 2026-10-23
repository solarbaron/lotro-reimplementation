// SPDX-License-Identifier: GPL-3.0-or-later
//! The runtime's P6 glue (decision 0024 P6, client half only): loads the windows' install
//! names ([`P6Refs`]) and the plugin views' DID-map keys off the frame, keeps the session's
//! plugin progression model ([`SessionProgression`]) and runs the P6 windows ([`P6Ui`]) on
//! the game UI once it is attached.
//!
//! Frame order (windowed client and headless `--ui`): [`Runtime::tick`](crate::runtime::Runtime::tick)
//! takes each frame's P6 changes from the world session ([`crate::net::World::take_p6_changes`])
//! and hands them to [`P6Hub::after_events`], which updates the plugin model and raises
//! `GameplayEvent::Progression` (`lua-plugin-api.md` §15.6), posts the level-up notice
//! (`progression-panels.md` §2.3) and keeps the changes for the windows; then
//! [`P6Hub::sync_model`] rebuilds the plugin model from the held state once, when the install
//! facts first become known (changes that arrived before them, such as the reports at world
//! entry, reached no view, because their DID-map keys were not known yet); after the NPC
//! windows, the quest journal and the P5 windows, [`P6Hub::frame`] runs the windows with the
//! frame's UI events and sends their commands
//! ([`crate::ui_host::client::window_frames`]).

use std::collections::VecDeque;
use std::sync::Arc;
use std::thread::JoinHandle;
use std::time::Instant;

use tessera_dat::ArchiveSet;
use tessera_net::EntityId;
use tessera_plugins::PluginManager;
use tessera_ui::UiEvent;
use tessera_ui::game::crafting::P6Art;

use super::commands::P6Panel;
use super::data::{ApiKeys, P6Data};
use super::plugin::SessionProgression;
use super::present::PlayerHoldings;
use super::refs::P6Refs;
use super::state::P6Change;
use super::ui::{P6Context, P6Ui, level_up_text};
use crate::character::PlayerState;
use crate::combat::CombatInstall;
use crate::npc::NpcData;
use crate::ui_host::UiHost;

/// Most P6 changes kept for the windows between two frames (Tessera bound; beyond it the
/// oldest are dropped and the windows refill from the state).
pub const MAX_UI_CHANGES: usize = 1024;

/// A crafting-panel toggle (`crafting-panels.md` §12): `None` the panel, `Some(i)` the panel
/// at 0-based profession tab `i`.
pub type CraftToggle = Option<usize>;

type Loaded = Result<(P6Refs, ApiKeys), String>;

/// The runtime's P6 glue (module docs).
#[derive(Default)]
pub struct P6Hub {
    refs: Option<Arc<P6Refs>>,
    data: Option<Arc<P6Data>>,
    currencies: Vec<u64>,
    loading: Option<(JoinHandle<Loaded>, Arc<NpcData>)>,
    failed: Option<String>,
    ui: Option<P6Ui>,
    pending_panels: Vec<P6Panel>,
    toggles: Vec<CraftToggle>,
    model: SessionProgression,
    /// The changes since the windows last ran (bounded by [`MAX_UI_CHANGES`]).
    ui_changes: VecDeque<P6Change>,
    /// The plugin model was built from the state with the known facts ([`Self::sync_model`]).
    model_synced: bool,
    /// The windows ran with the player in the world in an earlier frame.
    in_world: bool,
}

impl std::fmt::Debug for P6Hub {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("P6Hub")
            .field("refs", &self.refs.is_some())
            .field("data", &self.data.is_some())
            .field("ui", &self.ui)
            .finish_non_exhaustive()
    }
}

impl P6Hub {
    /// Nothing loaded; `panels` (from `--open-panel`) open once the player is in the world.
    #[must_use]
    pub fn new(panels: Vec<P6Panel>) -> Self {
        Self {
            pending_panels: panels,
            ..Self::default()
        }
    }

    /// The source of the plugins' progression views (`lua-plugin-api.md` §15.6): the
    /// session's model, for [`UiHost::set_plugin_progression`].
    #[must_use]
    pub fn plugin_source(&self) -> Arc<dyn tessera_plugins::ProgressionSource> {
        self.model.source()
    }

    /// The session's plugin progression model.
    #[must_use]
    pub fn model(&self) -> &SessionProgression {
        &self.model
    }

    /// The `--open-panel` windows still to open (opened at the first frame in the world).
    #[must_use]
    pub fn pending_panels(&self) -> &[P6Panel] {
        &self.pending_panels
    }

    /// The windows, once the UI is attached.
    #[must_use]
    pub fn ui(&self) -> Option<&P6Ui> {
        self.ui.as_ref()
    }

    /// The facts the windows read, once known (install or fallback).
    #[must_use]
    pub fn data(&self) -> Option<&Arc<P6Data>> {
        self.data.as_ref()
    }

    /// Why the install names are not available, when loading failed (the windows then use
    /// the fallbacks).
    #[must_use]
    pub fn failure(&self) -> Option<&str> {
        self.failed.as_deref()
    }

    /// Uses Tessera's fallback windows and the fallback facts (no install, or its facts
    /// failed).
    pub fn use_fallback(&mut self) {
        if self.refs.is_none() {
            self.refs = Some(Arc::new(P6Refs::fallback()));
        }
        if self.data.is_none() {
            self.data = Some(Arc::new(P6Data::fallback()));
        }
    }

    /// Uses already loaded names and facts (tests, tools).
    pub fn set_loaded(&mut self, refs: Arc<P6Refs>, data: Arc<P6Data>) {
        self.refs = Some(refs);
        self.data = Some(data);
    }

    /// Starts reading the install's names and the plugin keys on a worker thread once the
    /// archives and the P3 facts exist (no-op when started or loaded).
    pub fn start(&mut self, set: &Arc<ArchiveSet>, npc: &Arc<NpcData>) {
        if self.loading.is_some() || self.refs.is_some() || self.failed.is_some() {
            return;
        }
        let (set2, npc2) = (Arc::clone(set), Arc::clone(npc));
        match std::thread::Builder::new()
            .name("tessera-p6-refs".into())
            .spawn(move || {
                let refs = P6Refs::load(&set2)?;
                Ok((refs, P6Data::read_keys(&npc2)))
            }) {
            Ok(h) => self.loading = Some((h, Arc::clone(npc))),
            Err(e) => {
                self.failed = Some(format!("no loader thread: {e}"));
                self.use_fallback();
            }
        }
    }

    /// Takes what the worker finished (never blocks).
    pub fn poll(&mut self) {
        if !self
            .loading
            .as_ref()
            .is_some_and(|(h, _)| JoinHandle::is_finished(h))
        {
            return;
        }
        let Some((h, npc)) = self.loading.take() else {
            return;
        };
        match h.join() {
            Ok(Ok((refs, keys))) => {
                if !refs.missing.is_empty() {
                    tracing::warn!(missing = ?refs.missing, "P6 windows without a layout");
                }
                self.currencies = npc.coin_values();
                self.refs = Some(Arc::new(refs));
                self.data = Some(Arc::new(P6Data::install(npc, keys)));
            }
            Ok(Err(e)) => {
                tracing::warn!("P6 install names unavailable ({e}); fallback windows");
                self.failed = Some(e);
                self.use_fallback();
            }
            Err(_) => {
                self.failed = Some("the P6 loader panicked".into());
                self.use_fallback();
            }
        }
    }

    /// Whether the names and facts are known (install or fallback).
    #[must_use]
    pub fn ready(&self) -> bool {
        self.refs.is_some() && self.data.is_some()
    }

    /// `ToggleCraftingPanel` (`None`) or a profession toggle (`Some(tab)`): applied in the
    /// next [`Self::frame`] (`crafting-panels.md` §4.1, §12).
    pub fn request_toggle(&mut self, t: CraftToggle) {
        if self.toggles.len() < 16 {
            self.toggles.push(t);
        }
    }

    /// One frame's P6 changes (in event order, against the state they left): the plugin
    /// model's events are raised on `manager` (`lua-plugin-api.md` §15.6) once the facts are
    /// known and the model was built from the state ([`Self::sync_model`]; before that the
    /// model is not fed, and the rebuild covers these changes), a level-up's notice line is
    /// returned for the chat (`progression-panels.md` §2.3 item 1), and the changes are kept
    /// for the windows (the oldest dropped past [`MAX_UI_CHANGES`]).
    pub fn after_events(
        &mut self,
        changes: &[P6Change],
        state: &super::state::P6State,
        combat: Option<&CombatInstall>,
        manager: &mut PluginManager,
    ) -> Vec<String> {
        if changes.is_empty() {
            return Vec::new();
        }
        if self.model_synced
            && let Some(data) = self.data.as_deref()
        {
            self.model.apply(changes, state, data, combat, manager);
        }
        let mut notices = Vec::new();
        // Only the newest MAX_UI_CHANGES of the batch can stay.
        let skip = changes.len().saturating_sub(MAX_UI_CHANGES);
        for (i, c) in changes.iter().enumerate() {
            if let P6Change::LevelUp(up) = c {
                notices.push(level_up_text(up.to));
            }
            if i < skip {
                continue;
            }
            if self.ui_changes.len() >= MAX_UI_CHANGES {
                self.ui_changes.pop_front();
            }
            self.ui_changes.push_back(c.clone());
        }
        notices
    }

    /// Builds the plugin model from the held P6 `state` once, the first time the facts are
    /// known (install or fallback), and raises its view events on `manager` (module docs).
    /// Call every frame after [`Self::after_events`]; a no-op until the facts are known and
    /// after the first build.
    pub fn sync_model(
        &mut self,
        state: &super::state::P6State,
        combat: Option<&CombatInstall>,
        manager: &mut PluginManager,
    ) {
        if self.model_synced {
            return;
        }
        let Some(data) = self.data.clone() else {
            return;
        };
        self.model.resync(state, &data, combat, manager);
        self.model_synced = true;
    }

    /// Whether the plugin model was built from the state ([`Self::sync_model`]).
    #[must_use]
    pub fn model_synced(&self) -> bool {
        self.model_synced
    }

    /// One frame of the windows on `host` (module docs). When the player has left the world,
    /// every window goes once.
    pub fn frame(
        &mut self,
        host: &mut UiHost,
        world: &mut crate::net::World,
        events: &[UiEvent],
        now: Instant,
    ) {
        let changes: Vec<P6Change> = std::mem::take(&mut self.ui_changes).into();
        let (Some(refs), Some(data)) = (self.refs.clone(), self.data.clone()) else {
            return;
        };
        if world.player().is_none() {
            if std::mem::take(&mut self.in_world)
                && let Some(ui) = self.ui.as_mut()
            {
                ui.close_all(&mut host.ui_mut());
            }
            return;
        }
        self.in_world = true;
        let ui = self
            .ui
            .get_or_insert_with(|| P6Ui::new(Arc::clone(&refs), P6Art::default()));
        for p in std::mem::take(&mut self.pending_panels) {
            if let Err(e) = ui.open_panel(&mut host.ui_mut(), p) {
                tracing::warn!("{}: {e}", p.panel_name());
            }
        }
        for t in std::mem::take(&mut self.toggles) {
            ui.toggle_crafting(&mut host.ui_mut(), t);
        }
        // The holdings feed only the crafting panel: read them only while it exists.
        let holdings = if ui.crafting().is_some() {
            holdings_of(world.player_state())
        } else {
            PlayerHoldings::new()
        };
        let names = |id: EntityId| {
            world
                .entities()
                .get(&id)
                .map(|e| e.name.clone())
                .filter(|n| !n.is_empty())
        };
        let toolbar = host
            .panels()
            .and_then(|p| p.root(crate::panels::PanelId::Toolbar));
        let ctx = P6Context {
            data: &data,
            holdings: &holdings,
            coin: world.player_state().coin(),
            currencies: &self.currencies,
            entity_name: &names,
            toolbar,
            now: host.clock(),
        };
        let commands = ui.frame(&mut host.ui_mut(), world.p6(), &changes, &ctx, events);
        for c in commands {
            if let Err(e) = world.send_command(now, c) {
                tracing::info!("progression command not sent: {e}");
            }
        }
    }
}

/// The player's crafting holdings from the inventory model: the backpack's items by record
/// and by literal text (`crafting-panels.md` §5.4 Tessera rule: not the vault, shared storage
/// or equipped items).
#[must_use]
pub fn holdings_of(state: &PlayerState) -> PlayerHoldings {
    use tessera_gamedata::inventory::{Container, ItemRef};
    let mut h = PlayerHoldings::new();
    for item in state.inventory().in_container(Container::Backpack) {
        let q = u64::from(item.quantity);
        match &item.what {
            ItemRef::Record(d) => {
                let e = h.craft.holdings.entry(d.0).or_insert(0);
                *e = e.saturating_add(q);
            }
            ItemRef::Literal(s) => {
                let e = h.craft.literal_holdings.entry(s.clone()).or_insert(0);
                *e = e.saturating_add(q);
            }
        }
    }
    h
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::progression::state::P6State;
    use tessera_net::domain::{ProgressionEvent, Ref};
    use tessera_net::{DataId, Event, Transform};

    fn manager(dir: &std::path::Path) -> PluginManager {
        let host: Arc<dyn tessera_plugins::HostApi> =
            Arc::new(crate::plugins::LogHost::new(dir.join("data")));
        PluginManager::new(tessera_plugins::ManagerConfig::new(dir), host)
    }

    fn play(s: &mut P6State, ev: Event) -> Vec<P6Change> {
        let mut out = Vec::new();
        s.apply(&ev, &mut out);
        out
    }

    /// Reports that arrive before the install's plugin keys (world entry) reach the plugin
    /// model once the keys are known (the rebuild from the held state).
    #[test]
    fn reports_before_the_keys_reach_the_model_once_the_keys_arrive() {
        let dir = tempfile::tempdir().expect("tempdir");
        let mut m = manager(dir.path());
        let mut hub = P6Hub::new(Vec::new());
        let mut s = P6State::new();
        let rec = |d| Ref::Record(DataId(d));
        let mut changes = play(
            &mut s,
            Event::WorldEntered {
                player: EntityId(1),
                name: "Orla".into(),
                transform: Transform::default(),
            },
        );
        changes.extend(play(
            &mut s,
            Event::Progression(ProgressionEvent::ProfessionChanged {
                profession: rec(0x7000_1000),
                tier: 2,
                proficiency: 40,
                proficiency_max: 200,
                mastery: 10,
                mastery_max: 100,
                mastery_tier: None,
            }),
        ));
        changes.extend(play(
            &mut s,
            Event::Progression(ProgressionEvent::RecipeLearned {
                what: Ref::Literal("Cup".into()),
                profession: Some(rec(0x7000_1000)),
                tier: Some(2),
                category: None,
            }),
        ));
        changes.extend(play(
            &mut s,
            Event::Progression(ProgressionEvent::VocationChanged {
                vocation: Some(rec(0x7000_2000)),
                slots: Some(3),
            }),
        ));
        // No facts yet: the windows keep the changes; the model is not fed.
        hub.after_events(&changes, &s, None, &mut m);
        hub.sync_model(&s, None, &mut m);
        let src = hub.plugin_source();
        assert!(src.profession(9).is_none());
        assert!(!hub.model_synced());
        // The keys arrive (the loader finished).
        let mut keys = ApiKeys::default();
        keys.professions.insert(0x7000_1000, 9);
        keys.vocations.insert(0x7000_2000, 3);
        hub.set_loaded(
            Arc::new(P6Refs::fallback()),
            Arc::new(P6Data::fallback().with_keys(keys)),
        );
        hub.sync_model(&s, None, &mut m);
        assert!(hub.model_synced());
        let p = src.profession(9).expect("held");
        assert_eq!(
            (p.values.proficiency_level, p.values.proficiency_experience),
            (2, 40)
        );
        assert_eq!(src.recipe_count(9), 1);
        assert_eq!(src.vocation(), 3);
        // Later changes are applied directly; a repeat changes nothing.
        let again = play(
            &mut s,
            Event::Progression(ProgressionEvent::VocationChanged {
                vocation: None,
                slots: None,
            }),
        );
        hub.after_events(&again, &s, None, &mut m);
        assert_eq!(src.vocation(), 0);
        hub.sync_model(&s, None, &mut m);
        assert_eq!(src.recipe_count(9), 1, "built once");
    }

    /// The windows' queue keeps the newest changes, bounded.
    #[test]
    fn the_window_queue_is_bounded() {
        let dir = tempfile::tempdir().expect("tempdir");
        let mut m = manager(dir.path());
        let mut hub = P6Hub::new(Vec::new());
        let s = P6State::new();
        let many: Vec<P6Change> = (0..(MAX_UI_CHANGES as u32 + 10))
            .map(P6Change::Level)
            .collect();
        hub.after_events(&many, &s, None, &mut m);
        hub.after_events(&[P6Change::Level(u32::MAX)], &s, None, &mut m);
        assert_eq!(hub.ui_changes.len(), MAX_UI_CHANGES);
        assert_eq!(hub.ui_changes.back(), Some(&P6Change::Level(u32::MAX)));
        assert_eq!(hub.ui_changes.front(), Some(&P6Change::Level(11)));
    }
}
