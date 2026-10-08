// SPDX-License-Identifier: GPL-3.0-or-later
//! Plugin host glue: a [`HostApi`] that logs, and the per-frame [`PluginManager`] driver,
//! which also owns the client's command dispatcher (decision 0020) with what its built-ins
//! act on (player music, the plugin manager).

use std::collections::{BTreeSet, VecDeque};
use std::path::{Path, PathBuf};
use std::sync::mpsc::{self, Receiver, TryRecvError};
use std::sync::{Arc, Mutex, PoisonError};
use std::time::Instant;

use tessera_dat::ArchiveSet;

use tessera_script::{
    CHAT_TYPE_STANDARD, DataScope, HostApi, ManagerConfig, PluginDataLocation, PluginManager,
    ScriptErrorReport,
};

use crate::commands::text::CommandText;
use crate::commands::{Context, Dispatcher, LineKind, LocalLine, Origin, Outcome, PluginCommands};
use crate::walker::WalkerSession;

/// Account name used for plugin-data folders until real accounts exist.
pub const LOCAL_ACCOUNT: &str = "local";
/// Server name used for plugin-data folders with the sandbox backend.
pub const SANDBOX_SERVER: &str = "sandbox";
/// Most lines kept for [`LogHost::take_fresh`] and [`Plugins::take_lines`] (the oldest
/// go first).
pub const MAX_PENDING_LINES: usize = 256;

/// [`HostApi`] for the client: chat output goes to the log; plugin data lives under
/// Tessera's own data folder, never inside the game install.
#[derive(Debug)]
pub struct LogHost {
    location: Mutex<PluginDataLocation>,
    lines: Mutex<Vec<String>>,
    /// Lines not yet taken by [`LogHost::take_fresh`] (for the chat display).
    fresh: Mutex<VecDeque<String>>,
    /// The client's command names (lower case), for `Shell.IsCommand` and
    /// `Shell.GetCommands` (decision 0020 item 6, until M5 connects the registry).
    builtins: Mutex<BTreeSet<String>>,
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
            fresh: Mutex::new(VecDeque::new()),
            builtins: Mutex::new(BTreeSet::new()),
        }
    }

    /// Lines written since the last call (at most [`MAX_PENDING_LINES`]).
    pub fn take_fresh(&self) -> Vec<String> {
        self.fresh
            .lock()
            .unwrap_or_else(PoisonError::into_inner)
            .drain(..)
            .collect()
    }

    /// Sets the client's command names (lower case).
    pub fn set_builtin_commands(&self, names: impl IntoIterator<Item = String>) {
        let mut b = self.builtins.lock().unwrap_or_else(PoisonError::into_inner);
        b.clear();
        b.extend(names);
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
        drop(lines);
        let mut fresh = self.fresh.lock().unwrap_or_else(PoisonError::into_inner);
        if fresh.len() >= MAX_PENDING_LINES {
            fresh.pop_front();
        }
        fresh.push_back(text.to_owned());
    }

    fn is_builtin_command(&self, name: &str) -> bool {
        !name.is_empty()
            && self
                .builtins
                .lock()
                .unwrap_or_else(PoisonError::into_inner)
                .contains(&name.to_lowercase())
    }

    fn builtin_commands(&self) -> Vec<String> {
        self.builtins
            .lock()
            .unwrap_or_else(PoisonError::into_inner)
            .iter()
            .cloned()
            .collect()
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

/// The plugin side of the command dispatcher (decision 0020 item 6) on the current plugin
/// manager: `/plugins` and plugin commands still go through the manager's own line entry
/// point until M5 replaces it with `run_command(apartment, word, args)`.
impl PluginCommands for PluginManager {
    fn plugins_command(&mut self, arguments: &str) {
        self.execute_command(&format!("/plugins {arguments}"));
    }

    fn run_unregistered(&mut self, word: &str, arguments: &str) -> bool {
        if word.is_empty() || word.contains(char::is_whitespace) {
            return false;
        }
        self.execute_command(&format!("/{word} {arguments}"))
    }
}

/// Where the walker's chat overlay sends its lines and takes the client's own lines from
/// (decision 0020 Consequences: the overlay feeds the dispatcher).
pub trait CommandSink {
    /// Dispatches one line typed in the chat; channel commands use `session`.
    fn submit_chat(
        &mut self,
        line: &str,
        session: Option<&mut dyn WalkerSession>,
        now: Instant,
    ) -> Outcome;
    /// Appends the lines the client posted since the last call (command replies, player
    /// music, plugin output).
    fn take_lines(&mut self, out: &mut Vec<LocalLine>);
}

/// Start-up command lines (`--command`, decision 0020 item 7) and the install's command
/// words they wait for.
#[derive(Debug, Default)]
struct Startup {
    lines: Vec<String>,
    /// The archives were opened (or found missing).
    data_seen: bool,
    /// The background read of the install's command words.
    words: Option<Receiver<CommandText>>,
}

/// The plugin manager plus its host, and the command dispatcher.
#[derive(Debug)]
pub struct Plugins {
    host: Arc<LogHost>,
    manager: PluginManager,
    /// Player music: its commands (`/play`, ...) are client built-ins of the dispatcher.
    music: Option<crate::audio::player_music::PlayerMusic>,
    dispatcher: Dispatcher,
    startup: Startup,
    /// Lines the client posted, for [`CommandSink::take_lines`].
    outbox: VecDeque<LocalLine>,
}

impl Plugins {
    /// Discover plugins in `plugins_dir` (read-only) and load the named ones.
    pub fn start(plugins_dir: &Path, data_root: PathBuf, load: &[String]) -> Self {
        let host = Arc::new(LogHost::new(data_root));
        // Client built-ins are registered before any plugin loads (decision 0020 item 6).
        let dispatcher = Dispatcher::new();
        host.set_builtin_commands(dispatcher.client_names());
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
            dispatcher,
            startup: Startup::default(),
            outbox: VecDeque::new(),
        }
    }

    /// Lines to dispatch once the archives are open and the install's command words are
    /// read (`--command`, decision 0020 item 7), in order.
    pub fn set_startup_commands(&mut self, lines: Vec<String>) {
        self.startup.lines = lines;
    }

    /// The archives are open: reads the install's command words on a background thread;
    /// the start-up lines run after that, in [`Self::pump`].
    pub fn attach_data(&mut self, set: Arc<ArchiveSet>) {
        self.startup.data_seen = true;
        let (tx, rx) = mpsc::sync_channel(1);
        let spawned = std::thread::Builder::new()
            .name("command-words".into())
            .spawn(move || {
                let _ = tx.send(CommandText::load(&set, None));
            });
        match spawned {
            Ok(_) => self.startup.words = Some(rx),
            Err(e) => tracing::warn!("command words: thread: {e}; Tessera's defaults"),
        }
    }

    /// There are no archives: the start-up lines run with Tessera's own command words.
    pub fn data_unavailable(&mut self) {
        self.startup.data_seen = true;
    }

    /// The dispatcher.
    #[must_use]
    pub fn dispatcher(&self) -> &Dispatcher {
        &self.dispatcher
    }

    /// Runs one line through the dispatcher.
    pub fn submit(
        &mut self,
        line: &str,
        origin: Origin,
        session: Option<&mut dyn WalkerSession>,
        now: Instant,
    ) -> Outcome {
        let mut out = Vec::new();
        let mut ctx = Context {
            now,
            origin,
            session: crate::commands::shorten(session),
            music: self.music.as_mut(),
            plugins: Some(&mut self.manager),
        };
        let outcome = self.dispatcher.submit(line, &mut ctx, &mut out);
        for l in out {
            self.post(l);
        }
        self.flush_music();
        self.flush_host();
        outcome
    }

    fn post(&mut self, line: LocalLine) {
        match line.kind {
            LineKind::Error => {
                tracing::info!(target: "tessera_client::chat", kind = "error", "{}", line.text);
            }
            LineKind::Log => tracing::info!(target: "tessera_client::chat", "{}", line.text),
        }
        if self.outbox.len() >= MAX_PENDING_LINES {
            self.outbox.pop_front();
        }
        self.outbox.push_back(line);
    }

    fn flush_host(&mut self) {
        for text in self.host.take_fresh() {
            if self.outbox.len() >= MAX_PENDING_LINES {
                self.outbox.pop_front();
            }
            self.outbox.push_back(LocalLine::log(text));
        }
    }

    /// Takes the install's words when read, then runs the start-up lines once.
    fn pump_startup(&mut self, now: Instant) {
        if let Some(rx) = &self.startup.words {
            match rx.try_recv() {
                Ok(text) => {
                    self.startup.words = None;
                    self.dispatcher.apply_text(text);
                    self.host
                        .set_builtin_commands(self.dispatcher.client_names());
                }
                Err(TryRecvError::Empty) => return,
                Err(TryRecvError::Disconnected) => {
                    self.startup.words = None;
                    tracing::warn!("command words: the read ended without a result");
                }
            }
        }
        if !self.startup.data_seen || self.startup.lines.is_empty() {
            return;
        }
        for line in std::mem::take(&mut self.startup.lines) {
            tracing::info!(line = %line, "start-up command");
            if self.submit(&line, Origin::Startup, None, now) == Outcome::Unknown {
                tracing::warn!(line = %line, "start-up command unknown");
            }
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

    /// Runs a typed line through the dispatcher without a session (decision 0020
    /// Consequences); returns whether a command ran (not unknown, not empty).
    pub fn command(&mut self, line: &str) -> bool {
        self.submit(line, Origin::Chat, None, Instant::now()) == Outcome::Ran
    }

    /// Per-frame work: the install's command words and the start-up lines, player
    /// music, then the plugins.
    pub fn pump(&mut self) {
        self.pump_startup(Instant::now());
        if let Some(m) = &mut self.music {
            m.update();
        }
        self.flush_music();
        self.manager.pump();
        self.flush_host();
    }

    fn flush_music(&mut self) {
        let lines = self
            .music
            .as_mut()
            .map(|m| m.take_lines())
            .unwrap_or_default();
        for line in lines {
            self.post(LocalLine::log(line));
        }
    }

    /// Unload every apartment.
    pub fn shutdown(&mut self) {
        let loaded = self.manager.loaded_plugins().len();
        self.manager.unload_all();
        tracing::info!(unloaded = loaded, "plugins shut down");
    }
}

impl CommandSink for Plugins {
    fn submit_chat(
        &mut self,
        line: &str,
        session: Option<&mut dyn WalkerSession>,
        now: Instant,
    ) -> Outcome {
        self.submit(line, Origin::Chat, session, now)
    }

    fn take_lines(&mut self, out: &mut Vec<LocalLine>) {
        self.flush_music();
        self.flush_host();
        out.extend(self.outbox.drain(..));
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

    const CMD_PLUGIN: &str = r#"<?xml version="1.0"?>
<Plugin>
  <Information>
    <Name>Cmd</Name>
    <Author>Tessera</Author>
    <Version>1.0</Version>
  </Information>
  <Package>Test.Cmd.Main</Package>
</Plugin>
"#;

    const CMD_MAIN: &str = r#"
local cmd = Turbine.ShellCommand()
function cmd:Execute(name, args)
    Turbine.Shell.WriteLine("exec " .. name .. " [" .. tostring(args) .. "]")
end
Turbine.Shell.WriteLine("added " .. Turbine.Shell.AddCommand("greet;PLAY;say", cmd))
Turbine.Shell.WriteLine("is " .. tostring(Turbine.Shell.IsCommand("Playlist")))
"#;

    #[test]
    fn plugin_commands_go_through_the_dispatcher() {
        let dir = tempfile::tempdir().expect("tempdir");
        let plugins = dir.path().join("Plugins");
        let pkg = plugins.join("Test").join("Cmd");
        std::fs::create_dir_all(&pkg).expect("mkdir");
        std::fs::write(plugins.join("Test").join("Cmd.plugin"), CMD_PLUGIN).expect("write");
        std::fs::write(pkg.join("Main.lua"), CMD_MAIN).expect("write");
        let mut p = Plugins::start(&plugins, dir.path().join("data"), &["Cmd".to_owned()]);
        p.pump();
        // Client built-ins cannot be taken, and IsCommand knows them.
        let lines = p.host().chat_lines();
        assert!(lines.iter().any(|l| l == "added 1"), "{lines:?}");
        assert!(lines.iter().any(|l| l == "is true"), "{lines:?}");
        let mut out = Vec::new();
        p.take_lines(&mut out);
        assert!(p.command("/GREET  a  b "));
        assert!(!p.command("/nothing"));
        assert!(p.command("/plugins list"));
        p.take_lines(&mut out);
        let texts: Vec<&str> = out.iter().map(|l| l.text.as_str()).collect();
        assert!(texts.contains(&"exec GREET [a  b]"), "{texts:?}");
        assert!(
            texts.contains(&"Unknown command. Type /help for a list."),
            "{texts:?}"
        );
        assert!(texts.contains(&"Available plugins:"), "{texts:?}");
        p.shutdown();
    }

    #[test]
    fn startup_lines_wait_for_the_data() {
        let dir = tempfile::tempdir().expect("tempdir");
        let mut p = Plugins::start(&dir.path().join("nope"), dir.path().join("data"), &[]);
        p.set_startup_commands(vec!["/help play".into(), "hello".into()]);
        p.pump();
        let mut out = Vec::new();
        p.take_lines(&mut out);
        assert!(out.is_empty(), "{out:?}");
        p.data_unavailable();
        p.pump();
        p.take_lines(&mut out);
        assert_eq!(out.len(), 2, "{out:?}");
        assert!(out[0].text.starts_with("/play <name>"));
        assert!(out[1].text.starts_with("Start-up line not sent"));
        p.pump();
        out.clear();
        p.take_lines(&mut out);
        assert!(out.is_empty(), "once");
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
