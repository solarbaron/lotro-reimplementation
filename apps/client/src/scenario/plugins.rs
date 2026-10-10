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

use std::path::{Path, PathBuf};
use std::sync::Arc;
use std::time::{Duration, Instant};

use tessera_net::Event;

use crate::character::state::NO_INSTALL_BAGS;
use crate::character::{CharacterData, CharacterHub, PlayerState};
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
        if let Some(d) = data {
            hub.set_data(d);
        }
        hub.attach_plugins(plugins.manager_mut());
        let src: Arc<dyn tessera_plugins::LocalPlayerSource> = player.clone();
        plugins.manager_mut().set_local_player_source(Some(src));
        Self {
            plugins,
            player,
            state: PlayerState::new(bags),
            hub,
            load: load.to_vec(),
            queued: false,
            timeout: PLUGIN_LOAD_WAIT,
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

    /// One driver poll's events: applied to the player's state, the plugin views' events
    /// queued, then one round of the plugin host's work.
    pub fn observe(&mut self, events: &[Event]) {
        let mut changes = Vec::new();
        for ev in events {
            self.state.apply(ev, &mut changes);
        }
        if self.player.set(self.state.name()) {
            self.plugins.set_character(self.state.name());
        }
        if !self.queued && self.state.player().is_some() {
            self.load_now();
        }
        self.hub
            .after_events(&self.state, changes, self.plugins.manager_mut());
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
