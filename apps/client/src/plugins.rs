// SPDX-License-Identifier: GPL-3.0-or-later
//! Plugin host glue: a [`HostApi`] that logs, and the per-frame [`PluginManager`] driver,
//! which also owns the client's command dispatcher (decision 0020) with what its built-ins
//! act on (player music, the plugin manager).
//!
//! With a game UI attached (M5, [`crate::ui_host`]) the host also installs the `Turbine.UI`
//! classes into every new apartment ([`Plugins::set_ui_binding`]), queues chat output for
//! the chat window (the plugins' lines: [`Plugins::take_chat`]; the client's own lines,
//! command replies and player music: [`Plugins::take_local_chat`]), runs the chat window's
//! entry lines through the dispatcher ([`Plugins::submit_entry`], with the window's channel
//! mode, and [`Plugins::history_mut`] for line up and down), reports the frame clock to
//! `Turbine.Engine.GetGameTime` ([`Plugins::set_game_time`]) and leaves the plugin
//! manager's per-frame work to the UI host's frame-loop step 7 ([`Plugins::set_ui_driven`],
//! [`Plugins::pump_step7`]). The dispatcher's start-up lines and player music still run in
//! [`Plugins::pump`].
//!
//! The plugin host keeps no command table (`chat-window.md` §10 item 3, decision 0020
//! item 6): [`LogHost`] answers `Shell.AddCommand`, `RemoveCommand`, `IsCommand` and
//! `GetCommands` from the dispatcher's registry, with the calling apartment as the owner, so
//! `/help` lists the plugins' commands and the client's built-ins keep their names. The
//! install's command names are registered when the dispatcher's one read of the string
//! tables has finished or failed, and plugins load only after that (no plugin can take an
//! original name); start-up command lines wait for it too.

use std::collections::VecDeque;
use std::path::{Path, PathBuf};
use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::mpsc::{self, Receiver, TryRecvError};
use std::sync::{Arc, Mutex, PoisonError};
use std::time::Instant;

use tessera_dat::ArchiveSet;
use tessera_plugins::{
    CHAT_TYPE_STANDARD, CoreHost, DataScope, HostApi, ManagerConfig, PluginDataLocation,
    PluginManager, ScriptErrorReport,
};

use crate::commands::line::History;
use crate::commands::registry::{Handler, Help, Owner, Registry};
use crate::commands::text::CommandText;
use crate::commands::{
    Context, Dispatcher, LineKind, LocalLine, Origin, Outcome, PluginCommands, SharedRegistry,
    lock_registry,
};
use crate::panels::chat::{CHAT_ERROR, CHAT_LOG};

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
    /// Lines (chat type, text) not yet taken by [`LogHost::take_fresh`] (for the walker's
    /// chat display).
    fresh: Mutex<VecDeque<(u32, String)>>,
    /// The UI binding installed into new apartments (M5).
    ui: Mutex<Option<tessera_plugins::ui::UiBinding>>,
    /// Chat lines (type, text) waiting for the chat window, when a UI is attached.
    chat: Mutex<VecDeque<(u32, String)>>,
    /// `/plugins manager` asked for the manager window since the last take.
    manager_request: AtomicBool,
    /// The frame clock (seconds) for `Turbine.Engine.GetGameTime`, once the host sets it.
    game_time: Mutex<Option<f64>>,
    /// The dispatcher's registry: the plugins' shell commands go here (decision 0020
    /// item 6, `chat-window.md` §10 item 3).
    registry: SharedRegistry,
}

/// Most chat lines queued for the chat window between two frames.
pub const MAX_QUEUED_CHAT: usize = 1024;

impl LogHost {
    /// Host storing plugin data below `data_root`, with a registry of its own (tools and
    /// tests; the client uses [`Self::with_registry`]).
    #[must_use]
    pub fn new(data_root: PathBuf) -> Self {
        Self::with_registry(data_root, Arc::new(Mutex::new(Registry::new())))
    }

    /// Host storing plugin data below `data_root` whose shell commands go to the
    /// dispatcher's `registry`.
    #[must_use]
    pub fn with_registry(data_root: PathBuf, registry: SharedRegistry) -> Self {
        Self {
            location: Mutex::new(PluginDataLocation {
                root: data_root,
                account: LOCAL_ACCOUNT.to_owned(),
                server: Some(SANDBOX_SERVER.to_owned()),
                character: None,
            }),
            lines: Mutex::new(Vec::new()),
            fresh: Mutex::new(VecDeque::new()),
            ui: Mutex::new(None),
            chat: Mutex::new(VecDeque::new()),
            manager_request: AtomicBool::new(false),
            game_time: Mutex::new(None),
            registry,
        }
    }

    fn ui_attached(&self) -> bool {
        self.ui
            .lock()
            .unwrap_or_else(PoisonError::into_inner)
            .is_some()
    }

    fn queue_chat(&self, chat_type: u32, text: &str) {
        if !self.ui_attached() {
            return;
        }
        let mut q = self.chat.lock().unwrap_or_else(PoisonError::into_inner);
        if q.len() < MAX_QUEUED_CHAT {
            q.push_back((chat_type, text.to_owned()));
        }
    }

    /// One line of plugin output: kept for [`Self::chat_lines`], queued for the walker's
    /// chat display and, with a UI attached, for the chat window.
    fn record(&self, chat_type: u32, text: &str) {
        {
            let mut lines = self.lines.lock().unwrap_or_else(PoisonError::into_inner);
            if lines.len() >= 256 {
                lines.remove(0);
            }
            lines.push(text.to_owned());
        }
        {
            let mut fresh = self.fresh.lock().unwrap_or_else(PoisonError::into_inner);
            if fresh.len() >= MAX_PENDING_LINES {
                fresh.pop_front();
            }
            fresh.push_back((chat_type, text.to_owned()));
        }
        self.queue_chat(chat_type, text);
    }

    /// Lines (chat type, text) written since the last call (at most
    /// [`MAX_PENDING_LINES`]).
    pub fn take_fresh(&self) -> Vec<(u32, String)> {
        self.fresh
            .lock()
            .unwrap_or_else(PoisonError::into_inner)
            .drain(..)
            .collect()
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
        self.write_chat(CHAT_ERROR, &report.message);
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
        self.record(CHAT_LOG, text);
    }

    fn write_chat(&self, chat_type: u32, text: &str) {
        tracing::info!(target: "tessera_client::chat", chat_type, "{text}");
        self.record(chat_type, text);
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

    fn register_command(&self, apartment: &str, name: &str, handler: u64) -> bool {
        lock_registry(&self.registry).register(
            name,
            &Owner::Plugin(apartment.to_owned()),
            &Handler::Plugin(handler),
            &Help::Plugin,
        ) > 0
    }

    fn unregister_command(&self, apartment: &str, handler: u64) -> usize {
        lock_registry(&self.registry).unregister(
            &Owner::Plugin(apartment.to_owned()),
            &Handler::Plugin(handler),
        )
    }

    fn unregister_apartment_commands(&self, apartment: &str) -> usize {
        lock_registry(&self.registry).unregister_owner(&Owner::Plugin(apartment.to_owned()))
    }

    fn is_command(&self, name: &str) -> bool {
        lock_registry(&self.registry).is_command(name)
    }

    fn command_names(&self) -> Vec<String> {
        lock_registry(&self.registry)
            .entries()
            .map(|e| e.name.clone())
            .collect()
    }

    fn command_target(&self, word: &str) -> Option<(String, u64)> {
        match lock_registry(&self.registry).lookup(word) {
            Some(e) => match (&e.owner, &e.handler) {
                (Owner::Plugin(a), Handler::Plugin(h)) => Some((a.clone(), *h)),
                _ => None,
            },
            None => None,
        }
    }

    fn install_natives(
        &self,
        api: &tessera_plugins::api::NativeApi<'_>,
    ) -> tessera_plugins::mlua::Result<()> {
        let binding = *self.ui.lock().unwrap_or_else(PoisonError::into_inner);
        match binding {
            Some(b) => tessera_plugins::ui::install(api, b),
            None => Ok(()),
        }
    }

    fn show_plugin_manager(&self) -> bool {
        if !self.ui_attached() {
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

/// The plugin side of the command dispatcher (decision 0020 item 6) on the plugin manager:
/// `/plugins` is a client built-in that calls the manager, and the plugins' registry
/// entries run on their apartment's command object, with the word as typed and the
/// arguments of `chat-window.md` §7.4.
impl PluginCommands for PluginManager {
    fn plugins_command(&mut self, arguments: &str) {
        PluginManager::plugins_command(self, arguments);
    }

    fn run_command(&mut self, apartment: &str, handler: u64, word: &str, arguments: &str) {
        if !PluginManager::run_command(self, apartment, handler, word, arguments) {
            tracing::debug!(word, apartment, "plugin command object is gone");
        }
    }

    fn help(&mut self, apartment: &str, handler: u64, long: bool) -> Option<String> {
        self.command_help(apartment, handler, long)
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
/// words they, and the plugin loads, wait for (`chat-window.md` §10 item 3).
#[derive(Debug, Default)]
struct Startup {
    lines: Vec<String>,
    /// The archives were opened (or found missing).
    data_seen: bool,
    /// The background read of the install's command words.
    words: Option<Receiver<CommandText>>,
    /// The read has finished or failed (or there are no archives): plugins may load.
    words_settled: bool,
    /// Incremented each time the dispatcher takes a new read (the chat window follows).
    words_generation: u64,
}

/// The plugin manager plus its host, and the command dispatcher.
#[derive(Debug)]
pub struct Plugins {
    host: Arc<LogHost>,
    manager: PluginManager,
    /// The UI host runs the plugin manager's per-frame work in step 7 instead of
    /// [`Self::pump`].
    ui_driven: bool,
    /// Player music: its commands (`/play`, ...) are client built-ins of the dispatcher.
    music: Option<crate::audio::player_music::PlayerMusic>,
    dispatcher: Dispatcher,
    startup: Startup,
    /// Lines the client posted, for [`CommandSink::take_lines`].
    outbox: VecDeque<LocalLine>,
    /// With a game UI attached ([`Self::set_ui_driven`]): the client's own lines (chat-type
    /// mask, text) for the chat window, which also hands them to the plugins' `Received`
    /// ([`Self::take_local_chat`]).
    local_chat: VecDeque<(u64, String)>,
}

impl Plugins {
    /// Discover plugins in `plugins_dir` (read-only) and queue loads of the named ones; they
    /// run in [`Self::pump`] once the first plugin file index is ready.
    pub fn start(plugins_dir: &Path, data_root: PathBuf, load: &[String]) -> Self {
        // Client built-ins are registered before any plugin loads (decision 0020 item 6),
        // and the plugin host's shell commands go to the same registry.
        let dispatcher = Dispatcher::new();
        let host = Arc::new(LogHost::with_registry(
            data_root,
            dispatcher.shared_registry(),
        ));
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
        // (`plugin-runtime.md` §8.2, §8.4; decision 0015 §4.4) and the install's command
        // names are in the registry (`chat-window.md` §10 item 3), not here.
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
            music: None,
            dispatcher,
            startup: Startup::default(),
            outbox: VecDeque::new(),
            local_chat: VecDeque::new(),
        }
    }

    /// Lines to dispatch once the archives are open and the install's command words are
    /// read (`--command`, decision 0020 item 7), in order.
    pub fn set_startup_commands(&mut self, lines: Vec<String>) {
        self.startup.lines = lines;
    }

    /// The archives are open: reads the install's command words on a background thread;
    /// the start-up lines and the plugin loads run after that, in [`Self::pump`]
    /// (`chat-window.md` §10 item 3).
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
            Err(e) => {
                tracing::warn!("command words: thread: {e}; only Tessera's command names");
                self.startup.words_settled = true;
            }
        }
    }

    /// There are no archives: the start-up lines run with Tessera's own command words and
    /// the plugins may load.
    pub fn data_unavailable(&mut self) {
        self.startup.data_seen = true;
        if self.startup.words.is_none() {
            self.startup.words_settled = true;
        }
    }

    /// Whether the install's command names are in the registry, or their read failed
    /// (or there are no archives): only then do plugins load (`chat-window.md` §10 item 3).
    #[must_use]
    pub fn command_words_settled(&self) -> bool {
        self.startup.words_settled
    }

    /// How many times the dispatcher has taken the install's command words (0: never); the
    /// chat window re-reads [`Self::command_text`] when it changes (§10 item 11).
    #[must_use]
    pub fn command_text_generation(&self) -> u64 {
        self.startup.words_generation
    }

    /// The command words in use: the dispatcher's one read of the install's tables, or
    /// Tessera's defaults before (or without) it.
    #[must_use]
    pub fn command_text(&self) -> &CommandText {
        self.dispatcher.text()
    }

    /// The dispatcher.
    #[must_use]
    pub fn dispatcher(&self) -> &Dispatcher {
        &self.dispatcher
    }

    /// Runs one line through the dispatcher in the `Say` mode (the walker's overlay, the
    /// start-up lines, tools).
    pub fn submit(
        &mut self,
        line: &str,
        origin: Origin,
        session: Option<&mut dyn WalkerSession>,
        now: Instant,
    ) -> Outcome {
        self.run_line(line, "", "", origin, session, now)
    }

    /// Runs a line submitted in the M5 chat window's entry (`chat-window.md` §6.2 submit)
    /// through the dispatcher with the window's channel mode: its command word and tell
    /// target (decision 0020 items 1 and 2). Channel commands use `session`.
    pub fn submit_entry(
        &mut self,
        line: &tessera_ui::game::chat::EntryLine,
        session: Option<&mut dyn WalkerSession>,
        now: Instant,
    ) -> Outcome {
        self.run_line(
            &line.text,
            &line.word,
            &line.target,
            Origin::Chat,
            session,
            now,
        )
    }

    fn run_line(
        &mut self,
        line: &str,
        word: &str,
        target: &str,
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
        let outcome = self
            .dispatcher
            .submit_in(line, word, target, &mut ctx, &mut out);
        for l in out {
            self.post(l);
        }
        self.flush_music();
        self.flush_host();
        outcome
    }

    /// The one command history (`chat-window.md` §6.3), which the chat window's line up
    /// and down browse.
    pub fn history_mut(&mut self) -> &mut History {
        self.dispatcher.history_mut()
    }

    /// Posts one line of the client's own: the log, the walker's chat display and, with a
    /// game UI attached, the chat window's queue (type 1 `Error`, 8 `Log`, or the mask of a
    /// P4 built-in's line, `chat-window.md` §15.10 CT2).
    fn post(&mut self, line: LocalLine) {
        match line.kind {
            LineKind::Error => {
                tracing::info!(target: "tessera_client::chat", kind = "error", "{}", line.text);
            }
            LineKind::Log => {
                tracing::info!(target: "tessera_client::chat", "{}", line.text);
            }
        }
        if self.ui_driven {
            if self.local_chat.len() >= MAX_QUEUED_CHAT {
                self.local_chat.pop_front();
            }
            self.local_chat
                .push_back((line.chat_mask(), line.text.clone()));
        }
        self.push_outbox(line);
    }

    fn push_outbox(&mut self, line: LocalLine) {
        if self.outbox.len() >= MAX_PENDING_LINES {
            self.outbox.pop_front();
        }
        self.outbox.push_back(line);
    }

    fn flush_host(&mut self) {
        for (chat_type, text) in self.host.take_fresh() {
            let line = if chat_type == CHAT_ERROR {
                LocalLine::error(text)
            } else {
                LocalLine::log(text)
            };
            self.push_outbox(line);
        }
    }

    /// Takes the install's words when read, then runs the start-up lines once.
    fn pump_startup(&mut self, now: Instant) {
        if let Some(rx) = &self.startup.words {
            match rx.try_recv() {
                Ok(text) => {
                    self.startup.words = None;
                    self.startup.words_settled = true;
                    self.startup.words_generation = self.startup.words_generation.wrapping_add(1);
                    self.dispatcher.apply_text(text);
                }
                Err(TryRecvError::Empty) => return,
                Err(TryRecvError::Disconnected) => {
                    self.startup.words = None;
                    self.startup.words_settled = true;
                    tracing::warn!(
                        "command words: the read ended without a result; only Tessera's \
                         command names, prefix /"
                    );
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

    /// Deliver a chat line to plugin chat handlers as a `Standard` line.
    pub fn chat(&mut self, sender: &str, text: &str) {
        self.manager.dispatch_chat(sender, text, CHAT_TYPE_STANDARD);
    }

    /// Delivers a session chat line to plugin chat handlers without a game UI: its shown
    /// text (P4: the line in the original's form) with its chat type (`chat-window.md`
    /// §15.10 CT10: the type of its mask; 5 `Say`, 6 `Tell`, 8 `Log`, else `Standard`), as
    /// the chat window does with a UI.
    pub fn chat_line(&mut self, line: &crate::net::ChatLine) {
        let text = line.shown.as_ref().map_or(&line.text, |(_, t)| t);
        self.manager
            .dispatch_chat(&line.sender, text, line.chat_type.max(1));
    }

    /// Runs a typed line through the dispatcher without a session (decision 0020
    /// Consequences); returns whether a command ran (not unknown, not empty).
    pub fn command(&mut self, line: &str) -> bool {
        self.submit(line, Origin::Chat, None, Instant::now()) == Outcome::Ran
    }

    /// Per-frame work: the install's command words and the start-up lines, player music,
    /// then the plugins (without a game UI; with one, [`Self::pump_step7`] runs them). The
    /// plugin manager's work, and with it every plugin load, waits until the install's
    /// command names are registered or their read failed (`chat-window.md` §10 item 3).
    pub fn pump(&mut self) {
        self.pump_startup(Instant::now());
        if let Some(m) = &mut self.music {
            m.update();
        }
        self.flush_music();
        if !self.ui_driven && self.startup.words_settled {
            self.manager.pump();
        }
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

    /// Step 7: the deferred work, sliced by whole items, then the collector step (once the
    /// install's command names are settled, as [`Self::pump`]).
    pub fn pump_step7(&mut self) {
        if self.startup.words_settled {
            self.manager.pump();
        }
    }

    /// Sets the frame clock in seconds (decision 0014 D8, "Frame clock").
    pub fn set_game_time(&self, seconds: f64) {
        *self
            .host
            .game_time
            .lock()
            .unwrap_or_else(PoisonError::into_inner) = Some(seconds);
    }

    /// Every command name of the dispatcher's registry (built-in and plugin), as
    /// registered; read without running plugin code.
    #[must_use]
    pub fn command_names(&self) -> Vec<String> {
        self.dispatcher
            .registry()
            .entries()
            .map(|e| e.name.clone())
            .collect()
    }

    /// The client's own chat lines (chat-type mask, text) posted since the last call while a
    /// game UI is attached: command replies (the unknown-command line, help, refusals, the
    /// P4 built-ins' lines with their masks) and player music. The chat window shows them
    /// and hands them to the plugins' `Received` (`chat-window.md` §10 items 2 and 9).
    pub fn take_local_chat(&mut self) -> Vec<(u64, String)> {
        self.local_chat.drain(..).collect()
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
        // Nothing loads at start-up; the queued load runs in a pump once the index is ready
        // and the install's command names are settled (here: no archives).
        assert!(p.manager().loaded_plugins().is_empty());
        p.data_unavailable();
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
        p.data_unavailable();
        // The queued load runs in a pump once the plugin file index is ready.
        let deadline = std::time::Instant::now() + std::time::Duration::from_secs(30);
        while p.manager().loaded_plugins().is_empty() && std::time::Instant::now() < deadline {
            p.pump();
            std::thread::yield_now();
        }
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

    const TAKER_MAIN: &str = r#"
local cmd = Turbine.ShellCommand()
function cmd:Execute(name, args) Turbine.Shell.WriteLine("taker " .. name) end
Turbine.Shell.WriteLine("taken " .. Turbine.Shell.AddCommand("aide;Talk;mine", cmd))
Turbine.Shell.WriteLine("knows " .. tostring(Turbine.Shell.IsCommand("AIDE")))
"#;

    /// `chat-window.md` §10 item 3: plugins load only after the dispatcher's read of the
    /// install's tables has finished, so a plugin can never take an original name; the
    /// plugin's own names go to the dispatcher's registry with its apartment as the owner,
    /// and leave it when the apartment unloads.
    #[test]
    fn plugins_load_after_the_install_names() {
        let dir = tempfile::tempdir().expect("tempdir");
        let plugins = dir.path().join("Plugins");
        let pkg = plugins.join("Test").join("Cmd");
        std::fs::create_dir_all(&pkg).expect("mkdir");
        std::fs::write(plugins.join("Test").join("Cmd.plugin"), CMD_PLUGIN).expect("write");
        std::fs::write(pkg.join("Main.lua"), TAKER_MAIN).expect("write");
        let mut p = Plugins::start(&plugins, dir.path().join("data"), &["Cmd".to_owned()]);
        // The archives are open and the read is under way (a channel stands in for it).
        let (tx, rx) = mpsc::sync_channel(1);
        p.startup.data_seen = true;
        p.startup.words = Some(rx);
        for _ in 0..200 {
            p.pump();
            std::thread::yield_now();
        }
        assert!(!p.command_words_settled());
        assert!(
            p.manager().loaded_plugins().is_empty(),
            "no plugin loads before the install's names are in"
        );
        assert_eq!(p.command_text_generation(), 0);
        tx.send(CommandText {
            help_names: Some("help;aide".into()),
            default_word: Some("talk".into()),
            ..CommandText::default()
        })
        .expect("send");
        let deadline = std::time::Instant::now() + std::time::Duration::from_secs(30);
        while p.manager().loaded_plugins().is_empty() && std::time::Instant::now() < deadline {
            p.pump();
            std::thread::yield_now();
        }
        assert!(p.command_words_settled());
        assert_eq!(p.command_text_generation(), 1);
        assert_eq!(p.command_text().default_word.as_deref(), Some("talk"));
        let lines = p.host().chat_lines();
        assert!(lines.iter().any(|l| l == "taken 1"), "{lines:?}");
        assert!(lines.iter().any(|l| l == "knows true"), "{lines:?}");
        let names = p.command_names();
        assert!(names.iter().any(|n| n == "mine"), "{names:?}");
        assert!(names.iter().any(|n| n == "aide"), "{names:?}");
        {
            let reg = p.dispatcher().registry();
            assert_eq!(
                reg.lookup("AIDE").map(|e| e.owner.clone()),
                Some(Owner::Client)
            );
            assert_eq!(
                reg.lookup("talk").map(|e| e.owner.clone()),
                Some(Owner::Client)
            );
            assert!(matches!(
                reg.lookup("mine").map(|e| e.owner.clone()),
                Some(Owner::Plugin(_))
            ));
        }
        assert!(p.command("/MINE x"));
        let mut out = Vec::new();
        p.take_lines(&mut out);
        assert!(out.iter().any(|l| l.text == "taker MINE"), "{out:?}");
        p.shutdown();
        assert!(
            !p.dispatcher().registry().is_command("mine"),
            "unload removes the names"
        );
    }

    /// A failed read lets the plugins load with Tessera's names only.
    #[test]
    fn a_failed_read_settles_too() {
        let dir = tempfile::tempdir().expect("tempdir");
        let mut p = Plugins::start(&dir.path().join("nope"), dir.path().join("data"), &[]);
        let (tx, rx) = mpsc::sync_channel::<CommandText>(1);
        p.startup.data_seen = true;
        p.startup.words = Some(rx);
        p.pump();
        assert!(!p.command_words_settled());
        drop(tx);
        p.pump();
        assert!(p.command_words_settled());
        assert_eq!(p.command_text_generation(), 0);
        assert_eq!(p.dispatcher().prefix(), "/");
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
