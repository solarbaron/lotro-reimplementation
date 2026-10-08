// SPDX-License-Identifier: GPL-3.0-or-later
//! Plugin host glue: a [`HostApi`] that logs, and the per-frame [`PluginManager`] driver.
//!
//! With a game UI attached (M5, [`crate::ui_host`]) the host also installs the `Turbine.UI`
//! classes into every new apartment ([`Plugins::set_ui_binding`]), queues chat output for
//! the chat window ([`Plugins::take_chat`]), reports the frame clock to
//! `Turbine.Engine.GetGameTime` ([`Plugins::set_game_time`]) and leaves the per-frame work
//! to the UI host's frame-loop step 7 ([`Plugins::set_ui_driven`], [`Plugins::pump_step7`]).

use std::collections::VecDeque;
use std::path::{Path, PathBuf};
use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::{Arc, Mutex, PoisonError};

use tessera_plugins::{
    CHAT_TYPE_STANDARD, CoreHost, DataScope, HostApi, ManagerConfig, PluginDataLocation,
    PluginManager, ScriptErrorReport,
};

/// Account name used for plugin-data folders until real accounts exist.
pub const LOCAL_ACCOUNT: &str = "local";
/// Server name used for plugin-data folders with the sandbox backend.
pub const SANDBOX_SERVER: &str = "sandbox";

/// [`HostApi`] for the client: chat output goes to the log; plugin data lives under
/// Tessera's own data folder, never inside the game install.
#[derive(Debug)]
pub struct LogHost {
    location: Mutex<PluginDataLocation>,
    lines: Mutex<Vec<String>>,
    /// The UI binding installed into new apartments (M5).
    ui: Mutex<Option<tessera_plugins::ui::UiBinding>>,
    /// Chat lines (type, text) waiting for the chat window, when a UI is attached.
    chat: Mutex<VecDeque<(u32, String)>>,
    /// `/plugins manager` asked for the manager window since the last take.
    manager_request: AtomicBool,
    /// The frame clock (seconds) for `Turbine.Engine.GetGameTime`, once the host sets it.
    game_time: Mutex<Option<f64>>,
    /// The client's built-in chat command names (lower case).
    builtins: Mutex<Vec<String>>,
}

/// Most chat lines queued for the chat window between two frames.
pub const MAX_QUEUED_CHAT: usize = 1024;

/// Lua registry key under which each apartment keeps the client's copy of its native
/// `Turbine.Shell.GetCommands`, saved before any plugin code runs. Plugins can replace
/// the global table entry but cannot reach the registry.
pub const GET_COMMANDS_KEY: &str = "tessera.client.GetCommands";

impl LogHost {
    /// Host storing plugin data below `data_root`.
    #[must_use]
    pub fn new(data_root: PathBuf) -> Self {
        Self {
            location: Mutex::new(PluginDataLocation {
                root: data_root,
                account: LOCAL_ACCOUNT.to_owned(),
                server: Some(SANDBOX_SERVER.to_owned()),
                character: None,
            }),
            lines: Mutex::new(Vec::new()),
            ui: Mutex::new(None),
            chat: Mutex::new(VecDeque::new()),
            manager_request: AtomicBool::new(false),
            game_time: Mutex::new(None),
            builtins: Mutex::new(Vec::new()),
        }
    }

    fn queue_chat(&self, chat_type: u32, text: &str) {
        if self
            .ui
            .lock()
            .unwrap_or_else(PoisonError::into_inner)
            .is_none()
        {
            return;
        }
        let mut q = self.chat.lock().unwrap_or_else(PoisonError::into_inner);
        if q.len() < MAX_QUEUED_CHAT {
            q.push_back((chat_type, text.to_owned()));
        }
    }

    /// Set (or clear) the character in the world, which enables character-scope data.
    pub fn set_character(&self, name: Option<&str>) {
        let mut loc = self.location.lock().unwrap_or_else(PoisonError::into_inner);
        loc.character = name.map(str::to_owned);
    }

    /// Lines written to the chat window so far (bounded to the last 256).
    #[must_use]
    pub fn chat_lines(&self) -> Vec<String> {
        self.lines
            .lock()
            .unwrap_or_else(PoisonError::into_inner)
            .clone()
    }
}

impl CoreHost for LogHost {
    fn script_log(&self, apartment: &str, message: &str) {
        tracing::info!(target: "tessera_client::script", apartment, "{message}");
    }

    fn report_error(&self, report: &ScriptErrorReport) {
        // Error lines are chat type 1 `Error`, without a prefix (`lua-plugin-api.md` §6.6,
        // P59), the type the plugin host also gives them for `Received`.
        self.write_chat(crate::panels::chat::CHAT_ERROR, &report.message);
        tracing::warn!(
            target: "tessera_client::script",
            plugin = report.source.as_deref().unwrap_or("-"),
            apartment = %report.state,
            context = %report.context,
            traceback = report.traceback.as_deref().unwrap_or(""),
            "plugin error: {}",
            report.message
        );
    }
}

impl HostApi for LogHost {
    fn write_line(&self, text: &str) {
        tracing::info!(target: "tessera_client::chat", "{text}");
        {
            let mut lines = self.lines.lock().unwrap_or_else(PoisonError::into_inner);
            if lines.len() >= 256 {
                lines.remove(0);
            }
            lines.push(text.to_owned());
        }
        self.queue_chat(crate::panels::chat::CHAT_LOG, text);
    }

    fn write_chat(&self, chat_type: u32, text: &str) {
        tracing::info!(target: "tessera_client::chat", chat_type, "{text}");
        {
            let mut lines = self.lines.lock().unwrap_or_else(PoisonError::into_inner);
            if lines.len() >= 256 {
                lines.remove(0);
            }
            lines.push(text.to_owned());
        }
        self.queue_chat(chat_type, text);
    }

    fn game_time(&self) -> f64 {
        match *self
            .game_time
            .lock()
            .unwrap_or_else(PoisonError::into_inner)
        {
            Some(t) => t,
            None => {
                static START: std::sync::OnceLock<std::time::Instant> = std::sync::OnceLock::new();
                START
                    .get_or_init(std::time::Instant::now)
                    .elapsed()
                    .as_secs_f64()
            }
        }
    }

    fn is_builtin_command(&self, name: &str) -> bool {
        self.builtins
            .lock()
            .unwrap_or_else(PoisonError::into_inner)
            .iter()
            .any(|b| b.eq_ignore_ascii_case(name))
    }

    fn builtin_commands(&self) -> Vec<String> {
        self.builtins
            .lock()
            .unwrap_or_else(PoisonError::into_inner)
            .clone()
    }

    fn install_natives(
        &self,
        api: &tessera_plugins::api::NativeApi<'_>,
    ) -> tessera_plugins::mlua::Result<()> {
        // Keep the apartment's own `Turbine.Shell.GetCommands` where plugin code cannot
        // replace it (the registry; plugins have no `debug` library), so that the panels'
        // command list never runs plugin code ([`GET_COMMANDS_KEY`]).
        if let Ok(shell) = api
            .ns_root()
            .raw_get::<tessera_plugins::mlua::Table>("Shell")
            && let Ok(f) = shell.raw_get::<tessera_plugins::mlua::Function>("GetCommands")
        {
            api.lua().set_named_registry_value(GET_COMMANDS_KEY, f)?;
        }
        let binding = *self.ui.lock().unwrap_or_else(PoisonError::into_inner);
        match binding {
            Some(b) => tessera_plugins::ui::install(api, b),
            None => Ok(()),
        }
    }

    fn show_plugin_manager(&self) -> bool {
        if self
            .ui
            .lock()
            .unwrap_or_else(PoisonError::into_inner)
            .is_none()
        {
            return false;
        }
        self.manager_request.store(true, Ordering::Relaxed);
        true
    }

    fn show_plugin_options(&self, _plugin: &str) {
        let _ = self.show_plugin_manager();
    }

    fn plugin_data_dir(&self, scope: DataScope) -> Option<PathBuf> {
        self.location
            .lock()
            .unwrap_or_else(PoisonError::into_inner)
            .dir_for(scope)
    }
}

/// The plugin manager plus its host.
#[derive(Debug)]
pub struct Plugins {
    host: Arc<LogHost>,
    manager: PluginManager,
    /// The UI host runs the per-frame work in step 7 instead of [`Self::pump`].
    ui_driven: bool,
}

impl Plugins {
    /// Discover plugins in `plugins_dir` (read-only) and queue loads of the named ones; they
    /// run in [`Self::pump`] once the first plugin file index is ready.
    pub fn start(plugins_dir: &Path, data_root: PathBuf, load: &[String]) -> Self {
        let host = Arc::new(LogHost::new(data_root));
        let host_dyn: Arc<dyn HostApi> = Arc::clone(&host) as Arc<dyn HostApi>;
        let mut config = ManagerConfig::new(plugins_dir);
        // The clean-room class helper (decision 0016), used only when the user's Plugins
        // folder has no Turbine package of its own (`plugin-class-helper.md` §5.1).
        config.builtins = Some(tessera_plugins::class_helper::ClassHelper::provider());
        let mut manager = PluginManager::new(config, host_dyn);
        // A missing folder is normal (no plugins installed); only scan one that exists.
        let report = if plugins_dir.is_dir() {
            manager.refresh()
        } else {
            tessera_plugins::RefreshReport::default()
        };
        tracing::info!(
            dir = %plugins_dir.display(),
            found = report.found,
            skipped = report.skipped.len(),
            "plugins discovered"
        );
        for (path, why) in &report.skipped {
            tracing::warn!(path = %path.display(), "plugin descriptor skipped: {why}");
        }
        for p in manager.available_plugins() {
            tracing::debug!(
                name = %p.descriptor.name,
                apartment = p.apartment(),
                path = %p.path.display(),
                "plugin available"
            );
        }
        // Each load is a step-7 item that runs once the first plugin file index is ready
        // (`plugin-runtime.md` §8.2, §8.4; decision 0015 §4.4), not here.
        for name in load {
            if manager.queue_load(name) {
                tracing::info!(plugin = %name, "plugin load queued");
            } else {
                tracing::warn!(plugin = %name, "plugin not loaded: no available plugin of that name");
            }
        }
        Self {
            host,
            manager,
            ui_driven: false,
        }
    }

    /// The host (for tests and the UI).
    #[must_use]
    pub fn host(&self) -> &LogHost {
        &self.host
    }

    /// The manager.
    #[must_use]
    pub fn manager(&self) -> &PluginManager {
        &self.manager
    }

    /// Character entered or left the world.
    pub fn set_character(&self, name: Option<&str>) {
        self.host.set_character(name);
    }

    /// Deliver a chat line to plugin chat handlers.
    pub fn chat(&mut self, sender: &str, text: &str) {
        self.manager.dispatch_chat(sender, text, CHAT_TYPE_STANDARD);
    }

    /// Run a `/command` line; returns whether a plugin or the host handled it.
    pub fn command(&mut self, line: &str) -> bool {
        self.manager.execute_command(line)
    }

    /// Per-frame work (without a game UI; with one, [`Self::pump_step7`] runs it).
    pub fn pump(&mut self) {
        if !self.ui_driven {
            self.manager.pump();
        }
    }

    /// The manager, mutably (the UI host's step 1 and step 7).
    pub fn manager_mut(&mut self) -> &mut PluginManager {
        &mut self.manager
    }

    /// Installs the `Turbine.UI` classes into apartments created from now on (`None`
    /// stops it), and switches chat output to the chat-window queue.
    pub fn set_ui_binding(&self, binding: Option<tessera_plugins::ui::UiBinding>) {
        *self.host.ui.lock().unwrap_or_else(PoisonError::into_inner) = binding;
    }

    /// Whether the UI host drives the per-frame work (frame-loop step 7).
    pub fn set_ui_driven(&mut self, on: bool) {
        self.ui_driven = on;
    }

    /// Whether a game UI drives the per-frame work (it is attached).
    #[must_use]
    pub fn ui_driven(&self) -> bool {
        self.ui_driven
    }

    /// Step 7: the deferred work, sliced by whole items, then the collector step.
    pub fn pump_step7(&mut self) {
        self.manager.pump();
    }

    /// Sets the frame clock in seconds (decision 0014 D8, "Frame clock").
    pub fn set_game_time(&self, seconds: f64) {
        *self
            .host
            .game_time
            .lock()
            .unwrap_or_else(PoisonError::into_inner) = Some(seconds);
    }

    /// Sets the client's built-in chat command names (`Shell.IsCommand`, LPA §11.1).
    pub fn set_builtin_commands(&self, names: Vec<String>) {
        *self
            .host
            .builtins
            .lock()
            .unwrap_or_else(PoisonError::into_inner) = names;
    }

    /// The client's built-in chat command names.
    #[must_use]
    pub fn builtin_commands(&self) -> Vec<String> {
        self.host
            .builtins
            .lock()
            .unwrap_or_else(PoisonError::into_inner)
            .clone()
    }

    /// Chat lines (type, text) the plugins wrote since the last call.
    pub fn take_chat(&self) -> Vec<(u32, String)> {
        self.host
            .chat
            .lock()
            .unwrap_or_else(PoisonError::into_inner)
            .drain(..)
            .collect()
    }

    /// Whether `/plugins manager` (or `ShowOptions`) asked for the manager window since
    /// the last call.
    pub fn take_manager_request(&self) -> bool {
        self.host.manager_request.swap(false, Ordering::Relaxed)
    }

    /// Unload every apartment.
    pub fn shutdown(&mut self) {
        let loaded = self.manager.loaded_plugins().len();
        self.manager.unload_all();
        tracing::info!(unloaded = loaded, "plugins shut down");
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    const DESCRIPTOR: &str = r#"<?xml version="1.0"?>
<Plugin>
  <Information>
    <Name>Hello</Name>
    <Author>Tessera</Author>
    <Version>1.0</Version>
  </Information>
  <Package>Test.Hello.Main</Package>
</Plugin>
"#;

    const MAIN: &str = r#"Turbine.Shell.WriteLine("hello from plugin")"#;

    #[test]
    fn loads_plugin_and_logs_output() {
        let dir = tempfile::tempdir().expect("tempdir");
        let plugins = dir.path().join("Plugins");
        let pkg = plugins.join("Test").join("Hello");
        std::fs::create_dir_all(&pkg).expect("mkdir");
        std::fs::write(plugins.join("Test").join("Hello.plugin"), DESCRIPTOR).expect("write");
        std::fs::write(pkg.join("Main.lua"), MAIN).expect("write");

        let mut p = Plugins::start(&plugins, dir.path().join("data"), &["Hello".to_owned()]);
        // Nothing loads at start-up; the queued load runs in a pump once the index is ready.
        assert!(p.manager().loaded_plugins().is_empty());
        let deadline = std::time::Instant::now() + std::time::Duration::from_secs(30);
        while p.manager().loaded_plugins().is_empty() && std::time::Instant::now() < deadline {
            p.pump();
            std::thread::yield_now();
        }
        assert_eq!(p.manager().loaded_plugins().len(), 1);
        assert!(
            p.host()
                .chat_lines()
                .iter()
                .any(|l| l == "hello from plugin"),
            "{:?}",
            p.host().chat_lines()
        );
        p.shutdown();
        assert!(p.manager().loaded_plugins().is_empty());
    }

    #[test]
    fn missing_folder_is_harmless() {
        let dir = tempfile::tempdir().expect("tempdir");
        let mut p = Plugins::start(&dir.path().join("nope"), dir.path().join("data"), &[]);
        p.pump();
        assert!(p.manager().available_plugins().is_empty());
    }

    #[test]
    fn data_dirs_follow_character() {
        let host = LogHost::new(PathBuf::from("/x"));
        assert!(host.plugin_data_dir(DataScope::Character).is_none());
        host.set_character(Some("Wanderer"));
        assert_eq!(
            host.plugin_data_dir(DataScope::Character),
            Some(PathBuf::from("/x/local/sandbox/Wanderer"))
        );
    }
}
