// SPDX-License-Identifier: GPL-3.0-or-later
//! Plugins in headless scenario runs (`--scenario … --headless --load-plugin NAME`): the
//! plugin host with the inventory views of `lua-plugin-api.md` §15.1, fed from the
//! scenario's events on the scenario's virtual clock (decision 0024 P1 exit criterion 4:
//! the conformance checks 66–69 run through the plugin host on scenarios).
//!
//! After each driver poll the events go to the player's state ([`PlayerState`]), the
//! plugin inventory views' mirror and events ([`CharacterHub`]), and the plugin host's
//! step-7 work runs once ([`Plugins::pump`]), so each poll's events are delivered before
//! the next poll, as one frame would deliver them. The plugins load when the player enters
//! the world (plugins read `LocalPlayer.GetInstance()` at load time, `lua-plugin-api.md`
//! §15), before that poll's events are queued, so they see every inventory event; the
//! plugins' chat output is logged and kept ([`ScenarioPlugins::lines`]).
//!
//! The combat views of §15.2 (decision 0024 P2) are fed the same way
//! ([`crate::combat::PluginCombat`]), with `Turbine.Engine.GetGameTime` on the scenario's
//! virtual clock ([`ScenarioPlugins::observe_at`]: seconds since world entry), so the
//! conformance checks 70–76 run on scenarios.
//!
//! The wallet and money views (`lua-plugin-api.md` §15.3, decision 0024 P3) are fed the
//! same way: the local player carries the session's wallet model
//! ([`crate::npc::SessionWallet`]), and each poll's wallet changes become its events.
//! So are the mount views (`lua-plugin-api.md` §15.5, decision 0024 P5): the local player
//! carries the session's mount model ([`crate::travel::SessionMount`]), fed by each poll's
//! `MountChanged` and `MountVitalChanged`.

use std::path::{Path, PathBuf};
use std::sync::Arc;
use std::time::{Duration, Instant};

use tessera_net::Event;

use crate::character::state::NO_INSTALL_BAGS;
use crate::character::{CharacterData, CharacterHub, PlayerState};
use crate::npc::NpcData;
use crate::npc::wallet::{SessionWallet, WalletPlayer, wallet_item_facts};
use crate::plugins::Plugins;
use crate::ui_host::services::WalkerPlayer;

/// How long the plugin loads at world entry may take.
pub const PLUGIN_LOAD_WAIT: Duration = Duration::from_secs(30);

/// The plugin host of a headless scenario run (module docs).
#[derive(Debug)]
pub struct ScenarioPlugins {
    plugins: Plugins,
    player: Arc<WalkerPlayer>,
    state: PlayerState,
    hub: CharacterHub,
    load: Vec<String>,
    /// The loads were queued (at world entry).
    queued: bool,
    timeout: Duration,
    /// The plugin combat views' source (§15.2).
    combat: crate::combat::PluginCombat,
    /// The install's combat data, with an install.
    combat_install: Option<Arc<crate::combat::CombatInstall>>,
    /// World entry on the driver's clock (the game time's origin).
    base: Option<Instant>,
    /// The wallet views' model (P3).
    wallet: Arc<SessionWallet>,
    /// The NPC facts of the install (the wallet items' values), when the run has one.
    npc_data: Option<Arc<NpcData>>,
    /// The P5 state the mount views follow.
    p5: crate::travel::P5State,
    /// The mount views' model (P5).
    mount: crate::travel::SessionMount,
}

impl ScenarioPlugins {
    /// The plugin host over `plugins_dir` (plugin data under `data_root`) with the plugins
    /// `load` queued, and the install facts `data` when the run has an install. The
    /// install's command words are not read: plugin commands use Tessera's own names.
    #[must_use]
    pub fn start(
        plugins_dir: &Path,
        data_root: PathBuf,
        load: &[String],
        data: Option<Arc<CharacterData>>,
    ) -> Self {
        let mut plugins = Plugins::start(plugins_dir, data_root, &[]);
        plugins.data_unavailable();
        let player = WalkerPlayer::new();
        let mut hub = CharacterHub::new();
        let bags = data.as_ref().map_or(NO_INSTALL_BAGS, |d| d.bags());
        let combat_install =
            data.as_ref()
                .and_then(|d| match crate::combat::CombatInstall::new(Arc::clone(d)) {
                    Ok(i) => Some(Arc::new(i)),
                    Err(e) => {
                        tracing::warn!("combat data unavailable: {e}");
                        None
                    }
                });
        let npc_data = data
            .as_ref()
            .and_then(|d| match NpcData::new(Arc::clone(d)) {
                Ok(n) => Some(Arc::new(n)),
                Err(e) => {
                    tracing::warn!("NPC facts unavailable: {e}");
                    None
                }
            });
        if let Some(d) = data {
            hub.set_data(d);
        }
        hub.attach_plugins(plugins.manager_mut());
        let combat = crate::combat::PluginCombat::new();
        combat.attach(plugins.manager_mut());
        let wallet = SessionWallet::new();
        let mount = crate::travel::SessionMount::new();
        let src: Arc<dyn tessera_plugins::LocalPlayerSource> =
            WalletPlayer::new(player.clone(), Arc::clone(&wallet));
        let src: Arc<dyn tessera_plugins::LocalPlayerSource> =
            crate::travel::MountPlayer::new(src, mount.source());
        plugins.manager_mut().set_local_player_source(Some(src));
        Self {
            plugins,
            player,
            state: PlayerState::new(bags),
            hub,
            load: load.to_vec(),
            queued: false,
            timeout: PLUGIN_LOAD_WAIT,
            combat,
            combat_install,
            base: None,
            wallet,
            npc_data,
            p5: crate::travel::P5State::new(),
            mount,
        }
    }

    /// How long a load at world entry may take (default [`PLUGIN_LOAD_WAIT`]).
    pub fn set_load_timeout(&mut self, timeout: Duration) {
        self.timeout = timeout;
    }

    /// Whether every plugin asked for is loaded.
    #[must_use]
    pub fn all_loaded(&self) -> bool {
        let loaded = self.plugins.manager().loaded_plugins();
        self.load
            .iter()
            .all(|n| loaded.iter().any(|l| l.name.eq_ignore_ascii_case(n)))
    }

    /// Queues the loads and pumps the plugin host until they have run (or the timeout
    /// passes).
    fn load_now(&mut self) {
        self.queued = true;
        for name in &self.load {
            if !self.plugins.manager_mut().queue_load(name) {
                tracing::warn!(plugin = %name, "plugin not loaded: no available plugin of that name");
            }
        }
        let start = Instant::now();
        loop {
            self.plugins.pump();
            let m = self.plugins.manager();
            if m.index_ready() && m.deferred_len() == 0 {
                break;
            }
            if start.elapsed() > self.timeout {
                tracing::warn!("plugins still loading after {:?}", self.timeout);
                break;
            }
            std::thread::sleep(Duration::from_millis(1));
        }
    }

    /// One driver poll's events at the wall clock ([`Self::observe_at`]).
    pub fn observe(&mut self, events: &[Event]) {
        self.observe_at(Instant::now(), events);
    }

    /// One driver poll's events at `now` (the driver's virtual clock): the game time
    /// (seconds since world entry) is set, the events applied to the player's state, the
    /// plugin views' events (inventory and combat) queued, then one round of the plugin
    /// host's work.
    pub fn observe_at(&mut self, now: Instant, events: &[Event]) {
        if events
            .iter()
            .any(|e| matches!(e, Event::WorldEntered { .. }))
        {
            self.base = Some(now);
        }
        let game_time = self
            .base
            .map_or(0.0, |b| crate::combat::seconds_since(b, now));
        self.plugins.set_game_time(game_time);
        let mut changes = Vec::new();
        let mut p5 = Vec::new();
        for ev in events {
            self.state.apply(ev, &mut changes);
            self.p5.apply(ev, &mut p5);
        }
        if self.player.set(self.state.name()) {
            self.plugins.set_character(self.state.name());
        }
        let install = self.combat_install.clone();
        // The combat views learn of the world entry (the player's entity and level)
        // before the plugins load, so `LocalPlayer` reads them at load time; the rest of
        // the poll's events are queued after the load, so the plugins see them.
        let split = if self.queued {
            0
        } else {
            events
                .iter()
                .position(|e| matches!(e, Event::WorldEntered { .. }))
                .map_or(0, |i| i + 1)
        };
        self.combat.observe(
            &events[..split],
            game_time,
            install.as_deref(),
            self.plugins.manager_mut(),
        );
        if !self.queued && self.state.player().is_some() {
            self.load_now();
        }
        let data = self.npc_data.clone();
        let facts = |r: &tessera_net::domain::Ref| wallet_item_facts(data.as_deref(), r);
        for ev in self.wallet.apply(&changes, &facts) {
            self.plugins
                .manager_mut()
                .raise_gameplay_event(tessera_plugins::GameplayEvent::Wallet(ev));
        }
        let player = self.state.player();
        for ev in self.mount.apply(&p5, player, self.npc_data.as_deref()) {
            self.plugins
                .manager_mut()
                .raise_gameplay_event(tessera_plugins::GameplayEvent::Mount(ev));
        }
        self.hub
            .after_events(&self.state, changes, self.plugins.manager_mut());
        self.combat.observe(
            &events[split..],
            game_time,
            install.as_deref(),
            self.plugins.manager_mut(),
        );
        self.plugins.pump();
        let _ = self.hub.take_ui_changes();
        for line in self.plugins.host().take_fresh() {
            tracing::info!(chat_type = line.0, "plugin: {}", line.1);
        }
    }

    /// The player's state as the plugins see it.
    #[must_use]
    pub fn state(&self) -> &PlayerState {
        &self.state
    }

    /// The plugin host.
    #[must_use]
    pub fn plugins(&self) -> &Plugins {
        &self.plugins
    }

    /// The plugin host, mutably (tests evaluate in apartments).
    pub fn plugins_mut(&mut self) -> &mut Plugins {
        &mut self.plugins
    }

    /// Every chat line the plugins wrote so far.
    #[must_use]
    pub fn lines(&self) -> Vec<String> {
        self.plugins.host().chat_lines()
    }

    /// Unloads every apartment.
    pub fn shutdown(&mut self) {
        self.plugins.shutdown();
    }
}
