// SPDX-License-Identifier: GPL-3.0-or-later
//! Plugin host glue: a [`HostApi`] that logs, and the per-frame [`PluginManager`] driver.

use std::path::{Path, PathBuf};
use std::sync::{Arc, Mutex, PoisonError};

use tessera_script::{
    CHAT_TYPE_STANDARD, DataScope, HostApi, ManagerConfig, PluginDataLocation, PluginManager,
    ScriptErrorReport,
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
}

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

impl HostApi for LogHost {
    fn write_line(&self, text: &str) {
        tracing::info!(target: "tessera_client::chat", "{text}");
        let mut lines = self.lines.lock().unwrap_or_else(PoisonError::into_inner);
        if lines.len() >= 256 {
            lines.remove(0);
        }
        lines.push(text.to_owned());
    }

    fn plugin_data_dir(&self, scope: DataScope) -> Option<PathBuf> {
        self.location
            .lock()
            .unwrap_or_else(PoisonError::into_inner)
            .dir_for(scope)
    }

    fn script_log(&self, apartment: &str, message: &str) {
        tracing::info!(target: "tessera_client::script", apartment, "{message}");
    }

    fn report_error(&self, report: &ScriptErrorReport) {
        self.write_line(&report.chat_line());
        tracing::warn!(
            target: "tessera_client::script",
            plugin = report.plugin.as_deref().unwrap_or("-"),
            apartment = %report.apartment,
            context = %report.context,
            traceback = report.traceback.as_deref().unwrap_or(""),
            "plugin error: {}",
            report.message
        );
    }
}

/// The plugin manager plus its host.
#[derive(Debug)]
pub struct Plugins {
    host: Arc<LogHost>,
    manager: PluginManager,
    /// Player music: its commands (`/play`, ...) are host commands, tried before plugins.
    music: Option<crate::audio::player_music::PlayerMusic>,
}

impl Plugins {
    /// Discover plugins in `plugins_dir` (read-only) and load the named ones.
    pub fn start(plugins_dir: &Path, data_root: PathBuf, load: &[String]) -> Self {
        let host = Arc::new(LogHost::new(data_root));
        let host_dyn: Arc<dyn HostApi> = Arc::clone(&host) as Arc<dyn HostApi>;
        let mut manager = PluginManager::new(ManagerConfig::new(plugins_dir), host_dyn);
        // A missing folder is normal (no plugins installed); only scan one that exists.
        let report = if plugins_dir.is_dir() {
            manager.refresh()
        } else {
            tessera_script::RefreshReport::default()
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
        for name in load {
            match manager.load_plugin(name) {
                Ok(()) => tracing::info!(plugin = %name, "plugin loaded"),
                Err(e) => tracing::warn!(plugin = %name, "plugin not loaded: {e}"),
            }
        }
        Self {
            host,
            manager,
            music: None,
        }
    }

    /// Routes the player-music commands (`/play`, `/playstart`, `/playlist`, `/playstop`,
    /// `/instrument`) to `music`; its chat lines go to the host's chat output.
    pub fn set_music(&mut self, music: crate::audio::player_music::PlayerMusic) {
        self.music = Some(music);
    }

    /// The player music, if set.
    pub fn music_mut(&mut self) -> Option<&mut crate::audio::player_music::PlayerMusic> {
        self.music.as_mut()
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
        if let Some(m) = &mut self.music
            && m.command(line)
        {
            self.flush_music();
            return true;
        }
        self.manager.execute_command(line)
    }

    /// Per-frame work.
    pub fn pump(&mut self) {
        if let Some(m) = &mut self.music {
            m.update();
            self.flush_music();
        }
        self.manager.pump();
    }

    fn flush_music(&mut self) {
        if let Some(m) = &mut self.music {
            for line in m.take_lines() {
                self.host.write_line(&line);
            }
        }
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
        p.pump();
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
