// SPDX-License-Identifier: GPL-3.0-or-later
//! The plugin host as the native panels see it ([`PluginControl`], [`ShellCommands`]), and
//! the local-player view the plugins read (`plugin-api-usage.md` §8).
//!
//! Panel code runs while the host holds a mutable borrow of the [`tessera_ui::Ui`], and
//! plugin code drives the same UI through `tessera-plugins`, so no plugin code may run
//! inside a panel call. The adapters therefore answer from a snapshot (the plugin rows
//! and command names taken before the call) and **queue** loads, unloads, refreshes and
//! plugin commands; the host runs the queue right after the panel call returns, still in
//! the same frame-loop step ([`Deferred`]).

use std::cell::{OnceCell, RefCell};
use std::collections::BTreeMap;
use std::sync::{Arc, Mutex, PoisonError};

use tessera_plugins::{LocalPlayerInfo, LocalPlayerSource, PluginManager};

use crate::panels::{PluginControl, PluginRow, ShellCommands};

/// Most queued plugin requests between two runs.
pub const MAX_DEFERRED: usize = 256;

/// Plugin work queued by panel code.
#[derive(Debug, Clone, Default, PartialEq, Eq)]
pub struct Deferred {
    /// Plugins to load (`lua-plugin-api.md` §6.2).
    pub loads: Vec<String>,
    /// Apartments to unload; `None` = all (§6.3).
    pub unloads: Vec<Option<String>>,
    /// Rediscover the available plugins (§4.3).
    pub refresh: bool,
    /// Plugin command lines (`/word args`) to run.
    pub commands: Vec<String>,
}

impl Deferred {
    /// Whether nothing is queued.
    #[must_use]
    pub fn is_empty(&self) -> bool {
        self.loads.is_empty()
            && self.unloads.is_empty()
            && !self.refresh
            && self.commands.is_empty()
    }

    fn full(&self) -> bool {
        self.loads.len() + self.unloads.len() + self.commands.len() >= MAX_DEFERRED
    }
}

/// The rows of the plugin manager: every available plugin, marked loaded or not, then
/// loaded plugins no longer available, in name order.
#[must_use]
pub fn plugin_rows(manager: &PluginManager) -> Vec<PluginRow> {
    let loaded = manager.loaded_plugins();
    let mut rows: BTreeMap<String, PluginRow> = BTreeMap::new();
    for a in manager.available_plugins() {
        let d = &a.descriptor;
        let l = loaded.iter().find(|l| l.name == d.name);
        rows.insert(
            d.name.clone(),
            PluginRow {
                name: d.name.clone(),
                version: d.version.clone(),
                author: d.author.clone(),
                description: d.description.clone().unwrap_or_default(),
                apartment: l.map_or_else(|| a.apartment().to_owned(), |l| l.apartment.clone()),
                loaded: l.is_some(),
            },
        );
    }
    for l in loaded {
        rows.entry(l.name.clone()).or_insert_with(|| PluginRow {
            name: l.name.clone(),
            version: l.version.clone(),
            author: l.author.clone(),
            description: String::new(),
            apartment: l.apartment.clone(),
            loaded: true,
        });
    }
    rows.into_values().collect()
}

/// The plugins' shell command names (`lua-plugin-api.md` §11.1), without the client's
/// built-in names. They come from the first apartment's **native** `GetCommands`, which the
/// client saved in that apartment's registry before any plugin code ran
/// ([`crate::plugins::GET_COMMANDS_KEY`]); the writable `Turbine.Shell.GetCommands` entry is
/// never called, so no plugin code runs inside a panel call even when a plugin replaced it.
/// Empty when there is no apartment or the saved function is missing.
#[must_use]
pub fn plugin_command_names(manager: &PluginManager, builtins: &[String]) -> Vec<String> {
    let Some(ap) = manager.apartment_names().into_iter().next() else {
        return Vec::new();
    };
    manager
        .with_lua(&ap, |lua| -> Option<Vec<String>> {
            use tessera_plugins::mlua::{Function, Table};
            let f: Function = lua
                .named_registry_value(crate::plugins::GET_COMMANDS_KEY)
                .ok()?;
            let t: Table = f.call(()).ok()?;
            let mut out = Vec::new();
            for v in t.sequence_values::<String>().take(4096).flatten() {
                if !builtins.iter().any(|b| b.eq_ignore_ascii_case(&v)) {
                    out.push(v.to_ascii_lowercase());
                }
            }
            Some(out)
        })
        .flatten()
        .unwrap_or_default()
}

/// The adapters' shared view (module docs). Rows and command names are read from the
/// manager on first use only, so a panel call that needs neither costs nothing.
pub struct PluginView<'a> {
    /// The plugin manager (read only).
    pub manager: &'a PluginManager,
    /// The client's built-in command names.
    pub builtins: &'a [String],
    /// The queue.
    pub deferred: &'a RefCell<Deferred>,
    rows: OnceCell<Vec<PluginRow>>,
    commands: OnceCell<Vec<String>>,
}

impl std::fmt::Debug for PluginView<'_> {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("PluginView").finish_non_exhaustive()
    }
}

impl<'a> PluginView<'a> {
    /// A view over `manager`.
    #[must_use]
    pub fn new(
        manager: &'a PluginManager,
        builtins: &'a [String],
        deferred: &'a RefCell<Deferred>,
    ) -> Self {
        Self {
            manager,
            builtins,
            deferred,
            rows: OnceCell::new(),
            commands: OnceCell::new(),
        }
    }

    fn rows_ref(&self) -> &[PluginRow] {
        self.rows.get_or_init(|| plugin_rows(self.manager))
    }

    fn commands_ref(&self) -> &[String] {
        self.commands
            .get_or_init(|| plugin_command_names(self.manager, self.builtins))
    }
}

impl PluginControl for PluginView<'_> {
    fn rows(&self) -> Vec<PluginRow> {
        self.rows_ref().to_vec()
    }

    fn load(&mut self, name: &str) -> Result<(), String> {
        if !self.rows_ref().iter().any(|r| r.name == name) {
            return Err(format!("no plugin named {name}"));
        }
        let mut d = self.deferred.borrow_mut();
        if d.full() {
            return Err("too many plugin requests are waiting".into());
        }
        d.loads.push(name.to_owned());
        Ok(())
    }

    fn unload(&mut self, apartment: Option<&str>) -> Result<(), String> {
        let mut d = self.deferred.borrow_mut();
        if d.full() {
            return Err("too many plugin requests are waiting".into());
        }
        d.unloads.push(apartment.map(str::to_owned));
        Ok(())
    }

    fn refresh(&mut self) {
        self.deferred.borrow_mut().refresh = true;
    }
}

impl ShellCommands for PluginView<'_> {
    fn lookup(&self, word: &str) -> Option<u32> {
        let w = word.to_ascii_lowercase();
        self.commands_ref()
            .iter()
            .position(|c| *c == w)
            .and_then(|i| u32::try_from(i).ok())
    }

    fn run(&mut self, _id: u32, word: &str, args: &str) -> i32 {
        let mut d = self.deferred.borrow_mut();
        if d.full() {
            return 0;
        }
        let line = if args.is_empty() {
            format!("/{word}")
        } else {
            format!("/{word} {args}")
        };
        d.commands.push(line);
        1
    }

    fn names(&self) -> Vec<String> {
        self.commands_ref().to_vec()
    }
}

/// The local player the plugins see: the walker's avatar (`plugin-api-usage.md` §8, data
/// class S). Shared with the plugin thread (the main thread).
#[derive(Debug, Default)]
pub struct WalkerPlayer {
    info: Mutex<Option<LocalPlayerInfo>>,
}

impl WalkerPlayer {
    /// No character yet.
    #[must_use]
    pub fn new() -> Arc<Self> {
        Arc::new(Self::default())
    }

    /// Sets the character (or none). Returns whether the name changed.
    pub fn set(&self, name: Option<&str>) -> bool {
        let mut g = self.info.lock().unwrap_or_else(PoisonError::into_inner);
        let old = g.as_ref().map(|i| i.name.clone());
        if old.as_deref() == name {
            return false;
        }
        *g = name.map(|n| LocalPlayerInfo {
            name: n.to_owned(),
            ..LocalPlayerInfo::default()
        });
        true
    }
}

impl LocalPlayerSource for WalkerPlayer {
    fn local_player(&self) -> Option<LocalPlayerInfo> {
        self.info
            .lock()
            .unwrap_or_else(PoisonError::into_inner)
            .clone()
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn view_queues_instead_of_running() {
        let dir = tempfile::tempdir().expect("tempdir");
        let host: std::sync::Arc<dyn tessera_plugins::HostApi> =
            std::sync::Arc::new(crate::plugins::LogHost::new(dir.path().join("data")));
        let manager = PluginManager::new(tessera_plugins::ManagerConfig::new(dir.path()), host);
        let d = RefCell::new(Deferred::default());
        let builtins: Vec<String> = Vec::new();
        let mut v = PluginView::new(&manager, &builtins, &d);
        assert!(PluginControl::load(&mut v, "B").is_err(), "no such plugin");
        assert!(v.unload(None).is_ok());
        assert_eq!(v.lookup("hello"), None, "no apartment, no commands");
        assert_eq!(v.run(0, "Hello", "x y"), 1);
        v.refresh();
        let d = d.into_inner();
        assert_eq!(d.unloads, vec![None]);
        assert_eq!(d.commands, vec!["/Hello x y".to_owned()]);
        assert!(d.refresh);
    }

    #[test]
    fn walker_player_changes() {
        let p = WalkerPlayer::new();
        assert!(p.local_player().is_none());
        assert!(p.set(Some("Avatar")));
        assert!(!p.set(Some("Avatar")));
        assert_eq!(p.local_player().map(|i| i.name), Some("Avatar".into()));
        assert!(p.set(None));
    }
}
