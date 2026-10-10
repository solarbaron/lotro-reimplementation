// SPDX-License-Identifier: GPL-3.0-or-later
//! The runtime's P1 glue: loads [`CharacterData`] off the frame once the archives are
//! open, feeds the plugin inventory views from the player-state changes of each frame
//! (`lua-plugin-api.md` §15.1: events queued for step 7), and keeps the changes for the
//! game UI's panels until they take them.

use std::path::PathBuf;
use std::sync::Arc;
use std::thread::JoinHandle;

use tessera_plugins::{GameplayEvent, PluginManager};

use super::data::{CharacterData, CharacterDataError};
use super::plugin::PluginInventory;
use super::state::{PlayerChange, PlayerState};

/// Most changes kept for the game UI between two takes (the oldest are dropped; the panels
/// then refresh from the state).
pub const MAX_UI_CHANGES: usize = 1 << 16;

/// What the loader thread returns: the facts and, when asked for, the character screens'
/// data.
type Loaded = (
    CharacterData,
    Option<Result<crate::screens::ScreenData, crate::screens::ScreenDataError>>,
);

/// The runtime's P1 glue (module docs).
#[derive(Debug, Default)]
pub struct CharacterHub {
    data: Option<Arc<CharacterData>>,
    screens: Option<Arc<crate::screens::ScreenData>>,
    want_screens: bool,
    loading: Option<JoinHandle<Result<Loaded, CharacterDataError>>>,
    failed: Option<String>,
    plugin: PluginInventory,
    ui_changes: Vec<PlayerChange>,
    ui_resync: bool,
}

impl CharacterHub {
    /// Nothing loaded yet.
    #[must_use]
    pub fn new() -> Self {
        Self::default()
    }

    /// Also loads the character screens' data (`--character-select`).
    pub fn want_screens(&mut self, on: bool) {
        self.want_screens = on;
    }

    /// The character screens' data (also the creation controls the walker reads received
    /// looks with), once loaded.
    #[must_use]
    pub fn screens(&self) -> Option<&Arc<crate::screens::ScreenData>> {
        self.screens.as_ref()
    }

    /// Starts loading the install facts of `root` on a worker thread (once).
    pub fn start_loading(&mut self, root: PathBuf) {
        if self.loading.is_some() || self.data.is_some() {
            return;
        }
        let screens = self.want_screens;
        let spawned = std::thread::Builder::new()
            .name("tessera-p1-data".into())
            .spawn(move || {
                let d = CharacterData::open(&root)?;
                // The screens' data also serves the walker's figures (the creation
                // controls that received looks are read with, decision 0025 D2); only
                // the random-name generator is left out without the screens.
                let strings = screens.then(|| d.strings().as_ref());
                let s = Some(crate::screens::ScreenData::load(d.store(), strings));
                Ok((d, s))
            });
        match spawned {
            Ok(h) => self.loading = Some(h),
            Err(e) => self.failed = Some(format!("no loader thread: {e}")),
        }
    }

    /// Takes the loaded facts when the worker has finished; returns them once, when they
    /// arrive.
    pub fn poll(&mut self) -> Option<Arc<CharacterData>> {
        if !self.loading.as_ref().is_some_and(JoinHandle::is_finished) {
            return None;
        }
        let h = self.loading.take()?;
        match h.join() {
            Ok(Ok((d, s))) => {
                tracing::info!(?d, "character data ready");
                self.take_screens(s);
                let d = Arc::new(d);
                self.data = Some(Arc::clone(&d));
                Some(d)
            }
            Ok(Err(e)) => {
                tracing::warn!("character data unavailable: {e}");
                self.failed = Some(e.to_string());
                None
            }
            Err(_) => {
                self.failed = Some("the loader panicked".into());
                None
            }
        }
    }

    /// After the facts arrived later than the session's first items: the plugin views
    /// re-read every held item of `state` with the facts, and the panels refresh
    /// everything at their next take ([`Self::take_ui_changes`] leads with a
    /// [`PlayerChange::Header`]).
    pub fn resync(&mut self, state: &PlayerState) {
        let data = self.data.clone();
        self.plugin.rebuild(state, data.as_deref());
        self.ui_resync = true;
    }

    /// Waits for the loader (tests and headless runs).
    pub fn wait(&mut self) -> Option<Arc<CharacterData>> {
        if let Some(h) = self.loading.take() {
            match h.join() {
                Ok(Ok((d, s))) => {
                    self.take_screens(s);
                    self.data = Some(Arc::new(d));
                }
                Ok(Err(e)) => self.failed = Some(e.to_string()),
                Err(_) => self.failed = Some("the loader panicked".into()),
            }
        }
        self.data.clone()
    }

    fn take_screens(
        &mut self,
        s: Option<Result<crate::screens::ScreenData, crate::screens::ScreenDataError>>,
    ) {
        match s {
            Some(Ok(s)) => self.screens = Some(Arc::new(s)),
            Some(Err(e)) => tracing::warn!("character screens unavailable: {e}"),
            None => {}
        }
    }

    /// Uses already loaded facts (tests, tools).
    pub fn set_data(&mut self, data: Arc<CharacterData>) {
        self.data = Some(data);
    }

    /// The install facts, once loaded.
    #[must_use]
    pub fn data(&self) -> Option<&Arc<CharacterData>> {
        self.data.as_ref()
    }

    /// Records that the facts cannot be loaded (tests, tools; the loader sets it itself).
    pub fn set_failed(&mut self, why: impl Into<String>) {
        self.failed = Some(why.into());
    }

    /// Why the facts are not available, when loading failed.
    #[must_use]
    pub fn failure(&self) -> Option<&str> {
        self.failed.as_deref()
    }

    /// The plugin inventory views' client side.
    #[must_use]
    pub fn plugin(&self) -> &PluginInventory {
        &self.plugin
    }

    /// Hands the plugin manager the inventory views' source (`lua-plugin-api.md` §15.1).
    pub fn attach_plugins(&self, manager: &mut PluginManager) {
        manager.set_inventory_source(Some(self.plugin.source()));
    }

    /// Takes one frame's player-state changes (already applied to `state`): the plugin
    /// events are queued on `manager` for step 7, and the changes are kept for the UI.
    pub fn after_events(
        &mut self,
        state: &PlayerState,
        changes: Vec<PlayerChange>,
        manager: &mut PluginManager,
    ) {
        if changes.is_empty() {
            return;
        }
        let data = self.data.clone();
        for ev in self.plugin.update(state, data.as_deref(), &changes) {
            manager.raise_gameplay_event(GameplayEvent::Inventory(ev));
        }
        if self.ui_changes.len().saturating_add(changes.len()) > MAX_UI_CHANGES {
            // Nobody took them (no UI): keep a marker that makes the panels refresh.
            self.ui_changes.clear();
            self.ui_resync = true;
        }
        self.ui_changes.extend(changes);
    }

    /// The changes since the last call, for the game UI; when changes were dropped, a
    /// [`PlayerChange::Header`] leads them so the panels refresh everything.
    pub fn take_ui_changes(&mut self) -> Vec<PlayerChange> {
        let mut out = std::mem::take(&mut self.ui_changes);
        if std::mem::take(&mut self.ui_resync) {
            out.insert(0, PlayerChange::Header);
        }
        out
    }
}
