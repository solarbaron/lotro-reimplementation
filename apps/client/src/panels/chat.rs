// SPDX-License-Identifier: GPL-3.0-or-later
//! The chat window P2 (`chat-window.md`) with Tessera's sandbox content (§10): windows and
//! tabs from `GOP_ChatWindows`, the log, the entry line, and one command registry (§7.1)
//! holding Tessera's built-in commands, the channel words (local echo, §10 item 2) and,
//! through [`ShellCommands`], the plugins' `Shell` commands.
//!
//! `tessera-ui`'s [`Chat`] does the window work (tabs, filters, log, entry, history,
//! tokenizer); this module supplies its configuration ([`ChatSetup`], with the install's
//! texts and command names from [`ChatSetup::load_texts`]) and the command handlers, and
//! keeps every line the window takes (its own lines and the ones `tessera-ui` posts itself:
//! the unknown-command line of §7.6 and the reply line of §6.2) so that the client can hand
//! them to the plugins' `Turbine.Chat.Received` as deferred work (§10 item 9).

use std::collections::BTreeMap;

use tessera_formats::property::PropertySchema;
use tessera_formats::property_set::PropertySetRecord;
use tessera_formats::{DataIdMap, Did, Record, StringTable};
use tessera_ui::Ui;
use tessera_ui::game::GameVocab;
use tessera_ui::game::chat::command::{RESULT_UNKNOWN, arguments, split_names, trim_ws};
use tessera_ui::game::chat::mask::type_bit;
use tessera_ui::game::chat::window::{ERROR_MASK, REPLY_MASK};
use tessera_ui::game::chat::{
    Chat, ChatConfig, ChatTexts, CommandHost, CommandInfo, CommandOutcome, CommandRegistry,
    CommandSetup, ModeKind, OpacityTable, TabConfig, TextInputOutcome, WindowConfig,
};
use tessera_ui::game::vocab::GamePids;
use tessera_ui::{ElementKey, UiEvent};
use tessera_world::WorldSource;

use super::PanelsError;
use super::plugin_manager::{DEFAULT_APARTMENT, PluginControl};

/// Chat type 1 `Error` (`chat-window.md` §9).
pub const CHAT_ERROR: u32 = 1;
/// Chat type 5 `Say`.
pub const CHAT_SAY: u32 = 5;
/// Chat type 8 `Log`: Tessera's own system lines and `Shell.WriteLine` (§9, §10 item 2).
pub const CHAT_LOG: u32 = 8;

/// The `UNIQUEDB` key of `GameplayOptionDefaults` (`chat-window.md` §3.3, §10 item 1).
pub const GAMEPLAY_OPTION_DEFAULTS_KEY: u32 = 0x68;
/// The `UNIQUEDB` key of `UI_ChatOpacities` (`chat-window.md` §3.5).
pub const CHAT_OPACITIES_KEY: u32 = 0x1000_000C;
/// The default of `GOP_ChatOpacity` (`chat-window.md` §3.5 step 3).
pub const DEFAULT_OPACITY: i32 = 25;

/// Handler IDs of Tessera's built-in commands (a range the shell never uses).
const BUILTIN_BASE: u32 = 0x7E00_0000;
const HELP: u32 = BUILTIN_BASE + 1;
const PLUGINS: u32 = BUILTIN_BASE + 2;
/// Channel words: `CHANNEL_BASE + chat type`.
const CHANNEL_BASE: u32 = BUILTIN_BASE + 0x1000;
/// Plugin commands: `SHELL_BIT | shell id`.
const SHELL_BIT: u32 = 0x8000_0000;

/// Most lines kept for the plugins between two drains: the cap of chat lines waiting for
/// `Received` delivery of the M5 host (`lua-plugin-api.md` §11.1 implementation note).
/// Further lines are shown but not delivered to plugins, and the drop is logged.
pub const MAX_OUTGOING: usize = 256;

/// `STRINGTABLE` key of the `CommandSetup` table (`chat-window.md` §7.2).
pub const COMMAND_SETUP_TABLE_KEY: u32 = 2;
/// `STRINGTABLE` key of the `Commands` table (`chat-window.md` §6.5, §8.1).
pub const COMMANDS_TABLE_KEY: u32 = 0x1000_0007;
/// `STRINGTABLE` key of the `Chat` table (`chat-window.md` §4.6, §6.2).
pub const CHAT_TABLE_KEY: u32 = 0x1000_004D;
/// `CommandSetup` token of the command prefix (§7.2 step 4).
pub const TOKEN_PREFIX: u32 = 0x0DA2_8738;
/// `CommandSetup` token of the substitution marker (§6.5, §7.5).
pub const TOKEN_MARKER: u32 = 0x0240_9858;
/// `CommandSetup` token of the default channel word (§7.2).
pub const TOKEN_DEFAULT_WORD: u32 = 0x04DA_4989;
/// `CommandSetup` token of the unknown-command line (§7.6).
pub const TOKEN_UNKNOWN_COMMAND: u32 = 0x04FE_7F13;
/// `Chat` token of the no-reply-target line (§6.2 `ChatModeReply`).
pub const TOKEN_NO_REPLY_TARGET: u32 = 0x00CC_89CC;
/// `Commands` token of the help command's names (§8.1).
pub const TOKEN_HELP: u32 = 0x0DA3_EC70;
/// `Commands` token of the plugin-manager command's names (§8.1, LPA §6.4).
pub const TOKEN_PLUGINS: u32 = 0x073F_0613;

/// The fixed rows of the mode table (§6.5): `Commands` token, chat type, kind. The two
/// variant groups are left out (§6.5 OPEN Q12: the game-state test that picks one is not
/// traced; Tessera rule recorded there).
pub const MODE_ROWS: [(u32, u32, ModeKind); 26] = [
    (0x04DA_4989, 5, ModeKind::Plain),
    (0x0DA4_AC6C, 6, ModeKind::Plain),
    (0x048C_E78C, 6, ModeKind::Reply),
    (0x0A48_C3E9, 6, ModeKind::TellAgain),
    (0x0384_53D4, 11, ModeKind::Plain),
    (0x07B5_3A04, 23, ModeKind::Plain),
    (0x06F0_47CE, 15, ModeKind::Plain),
    (0x0253_3A8E, 46, ModeKind::Plain),
    (0x0253_3B7E, 47, ModeKind::Plain),
    (0x0AE9_AEE7, 16, ModeKind::Plain),
    (0x0AE9_AE73, 18, ModeKind::Plain),
    (0x0AE9_8903, 27, ModeKind::Plain),
    (0x040E_21EC, 19, ModeKind::Plain),
    (0x03AC_BD91, 9, ModeKind::Plain),
    (0x03AC_BD92, 10, ModeKind::Plain),
    (0x08B3_A4E1, 28, ModeKind::Plain),
    (0x08B3_A4E2, 29, ModeKind::Plain),
    (0x08B3_A4E3, 30, ModeKind::Plain),
    (0x08B3_A4E4, 31, ModeKind::Plain),
    (0x08B3_A4E5, 39, ModeKind::Plain),
    (0x08B3_A4E6, 40, ModeKind::Plain),
    (0x08B3_A4E7, 41, ModeKind::Plain),
    (0x08B3_A4E8, 42, ModeKind::Plain),
    (0x0984_23C4, 38, ModeKind::Plain),
    (0x06E8_82B4, 44, ModeKind::Plain),
    (0x0538_82B4, 45, ModeKind::Plain),
];

/// The plugins' `Shell` commands (`lua-plugin-api.md` §11.1–§11.2), as the client's plugin
/// host provides them. Command IDs must be below 0x8000_0000.
pub trait ShellCommands {
    /// The command a word names, compared case-insensitively (`chat-window.md` §8.2).
    fn lookup(&self, word: &str) -> Option<u32> {
        let _ = word;
        None
    }

    /// Runs command `id` with the command word as typed and the arguments of §7.4 (§9:
    /// trimmed, quotes kept). Returns the handler's result (1 for success).
    fn run(&mut self, id: u32, word: &str, args: &str) -> i32 {
        let _ = (id, word, args);
        0
    }

    /// Every registered name (for the help listing and `GetCommands`).
    fn names(&self) -> Vec<String> {
        Vec::new()
    }

    /// Command `id`'s short help (`ShellCommand:GetShortHelp`, LPA §11.2), shown in the
    /// help listing (`chat-window.md` §7.7). Empty when it has none.
    fn short_help(&mut self, id: u32) -> String {
        let _ = id;
        String::new()
    }

    /// Command `id`'s long help (`ShellCommand:GetHelp`, LPA §11.2), shown by the help
    /// command with the command's name as its argument (§7.7). Empty when it has none.
    fn help(&mut self, id: u32) -> String {
        let _ = id;
        String::new()
    }
}

/// No plugin commands.
#[derive(Debug, Clone, Copy, Default)]
pub struct NoShell;

impl ShellCommands for NoShell {}

/// Something a chat command asks of the panels.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ChatRequest {
    /// `/plugins manager` (`lua-plugin-api.md` §6.4): open the plugin manager.
    OpenPluginManager,
}

/// Everything the chat window needs from the install and Tessera's settings.
#[derive(Debug, Clone, PartialEq)]
pub struct ChatSetup {
    /// Windows and tabs (`GOP_ChatWindows`, §3.3).
    pub config: ChatConfig,
    /// The `CommandSetup` texts (§7.2), read from the install by the client.
    pub command_setup: CommandSetup,
    /// Texts of the unknown-command and no-reply-target lines and the channel names.
    pub texts: ChatTexts,
    /// The channel mode rows (§6.5): (`Commands` entry text, chat type, kind).
    pub modes: Vec<(String, u32, ModeKind)>,
    /// Background opacity images (§3.5).
    pub opacity: OpacityTable,
    /// `GOP_ChatOpacity` (§3.5; default 25).
    pub opacity_value: i32,
    /// The original's help command names (`Commands` entry text, §10 item 3), if read.
    pub help_entry: String,
    /// The original's plugin-manager command names (`Commands` entry text, §10 item 3).
    pub plugins_entry: String,
}

impl ChatSetup {
    /// The sandbox setup without install data (`ui-layout.md` §17.4 P2: one tab): window 1,
    /// visible, one tab showing every chat type; the syntax defaults of
    /// [`CommandSetup::default`] with its default word as the `Say` channel word.
    #[must_use]
    pub fn sandbox() -> Self {
        let command_setup = CommandSetup::default();
        let say = command_setup.default_word.clone();
        Self {
            config: ChatConfig {
                windows: vec![WindowConfig {
                    id: 1,
                    visible: true,
                    tabs: vec![TabConfig {
                        name: None,
                        mask: !tessera_ui::game::chat::mask::UNFILTERED,
                    }],
                }],
            },
            command_setup,
            texts: ChatTexts::default(),
            modes: vec![(say, CHAT_SAY, ModeKind::Plain)],
            opacity: OpacityTable::default(),
            opacity_value: DEFAULT_OPACITY,
            help_entry: String::new(),
            plugins_entry: String::new(),
        }
    }

    /// Replaces the configuration and the opacity table with the install's defaults
    /// (§10 item 1, §3.5): `GOP_ChatWindows` of `GameplayOptionDefaults` (`UNIQUEDB` key
    /// 0x68) and `UI_ChatOpacities` (key 0x1000000C). Texts, modes and command names come
    /// from string tables: [`ChatSetup::load_texts`].
    ///
    /// # Errors
    /// [`PanelsError::Record`] when a record is missing, unreadable or undecodable, or
    /// [`PanelsError::Game`] when `GOP_ChatWindows` is malformed. On error `self` keeps
    /// what it had.
    pub fn load_defaults(
        &mut self,
        source: &dyn WorldSource,
        schema: &(dyn PropertySchema + Send + Sync),
        unique: &DataIdMap,
        pids: &GamePids,
    ) -> Result<(), PanelsError> {
        let defaults = unique_set(source, schema, unique, GAMEPLAY_OPTION_DEFAULTS_KEY)?;
        let v = defaults
            .set
            .get(pids.gop_chat_windows)
            .ok_or(PanelsError::Record("GOP_ChatWindows"))?;
        let config = ChatConfig::from_value(v, pids)?;
        if let Ok(opac) = unique_set(source, schema, unique, CHAT_OPACITIES_KEY)
            && let Some(t) = opac
                .set
                .iter()
                .map(|e| OpacityTable::from_value(&e.value, pids))
                .find(|t| !t.ranges.is_empty())
        {
            self.opacity = t;
        }
        self.config = config;
        Ok(())
    }
}

impl ChatSetup {
    /// Reads the chat window's texts and command names from the install's string tables
    /// (§10 items 3, 4, 6 and 8; `stringtable` is the `STRINGTABLE` DID map):
    ///
    /// - `CommandSetup` (key 2): the command prefix (token 0x0DA28738), the substitution
    ///   marker (0x02409858), the default channel word (0x04DA4989) and the unknown-command
    ///   line (0x04FE7F13);
    /// - `Chat` (key 0x1000004D): the no-reply-target line (0x00CC89CC);
    /// - `Commands` (key 0x10000007): the help names (0x0DA3EC70), the plugin-manager names
    ///   (0x073F0613) and the fixed rows of the mode table ([`MODE_ROWS`], §6.5).
    ///
    /// Each text that is found replaces the current one; a missing table or token keeps
    /// it (so the sandbox's syntax defaults stay when the install lacks one). Returns the
    /// number of texts read. Message pieces are joined without argument values (all of
    /// these entries are one piece without arguments in the data, CW27a, CW32a).
    pub fn load_texts(&mut self, source: &dyn WorldSource, stringtable: &DataIdMap) -> usize {
        let mut tables: BTreeMap<u32, Option<StringTable>> = BTreeMap::new();
        let mut text = |key: u32, token: u32| -> Option<String> {
            let t = tables.entry(key).or_insert_with(|| {
                let did = stringtable.get(key)?.value;
                let bytes = source.read(did).ok()??;
                StringTable::parse_expecting(&bytes, did).ok()
            });
            t.as_ref()?
                .get(token)
                .map(tessera_formats::string_table::StringTableEntry::plain_text)
        };
        let mut n = 0;
        let mut set = |slot: &mut String, v: Option<String>| {
            if let Some(v) = v.filter(|v| !v.is_empty()) {
                *slot = v;
                n += 1;
            }
        };
        set(
            &mut self.command_setup.prefix,
            text(COMMAND_SETUP_TABLE_KEY, TOKEN_PREFIX),
        );
        set(
            &mut self.command_setup.marker,
            text(COMMAND_SETUP_TABLE_KEY, TOKEN_MARKER),
        );
        set(
            &mut self.command_setup.default_word,
            text(COMMAND_SETUP_TABLE_KEY, TOKEN_DEFAULT_WORD),
        );
        set(
            &mut self.texts.unknown_command,
            text(COMMAND_SETUP_TABLE_KEY, TOKEN_UNKNOWN_COMMAND),
        );
        set(
            &mut self.texts.no_reply_target,
            text(CHAT_TABLE_KEY, TOKEN_NO_REPLY_TARGET),
        );
        set(&mut self.help_entry, text(COMMANDS_TABLE_KEY, TOKEN_HELP));
        set(
            &mut self.plugins_entry,
            text(COMMANDS_TABLE_KEY, TOKEN_PLUGINS),
        );
        let modes: Vec<(String, u32, ModeKind)> = MODE_ROWS
            .iter()
            .filter_map(|(token, chat_type, kind)| {
                text(COMMANDS_TABLE_KEY, *token)
                    .filter(|t| !t.is_empty())
                    .map(|t| (t, *chat_type, *kind))
            })
            .collect();
        if !modes.is_empty() {
            n += modes.len();
            self.modes = modes;
        }
        n
    }
}

/// The property-set record a `UNIQUEDB` key names.
fn unique_set(
    source: &dyn WorldSource,
    schema: &(dyn PropertySchema + Send + Sync),
    unique: &DataIdMap,
    key: u32,
) -> Result<PropertySetRecord, PanelsError> {
    let did: Did = unique
        .get(key)
        .map(|e| e.value)
        .ok_or(PanelsError::Record("UNIQUEDB entry"))?;
    let bytes = source
        .read(did)
        .map_err(|_| PanelsError::Record("property set"))?
        .ok_or(PanelsError::Record("property set"))?;
    PropertySetRecord::parse_expecting(&bytes, did, schema)
        .map_err(|_| PanelsError::Record("property set"))
}

/// The command handlers for one call, borrowing only the registry fields of a
/// [`ChatPanel`] (so that its `chat` field stays free for the UI's chat calls).
macro_rules! dispatch {
    ($s:ident, $shell:ident, $plugins:ident) => {
        Dispatch {
            registry: &$s.registry,
            builtin_names: &$s.builtin_names,
            channel_names: &$s.setup.texts.channel_names,
            shell: $shell,
            plugins: $plugins,
            lines: Vec::new(),
            requests: Vec::new(),
        }
    };
}

/// The chat window P2 with its command registry.
#[derive(Debug, Clone)]
pub struct ChatPanel {
    /// The UI's chat system.
    pub chat: Chat,
    vocab: GameVocab,
    layout: Did,
    setup: ChatSetup,
    registry: CommandRegistry,
    builtin_names: Vec<String>,
    outgoing: Vec<(u64, String)>,
    dropped: u64,
    requests: Vec<ChatRequest>,
}

impl ChatPanel {
    /// A chat panel for the `chatwindow` layout `layout`. Nothing is created until
    /// [`ChatPanel::open`].
    #[must_use]
    pub fn new(vocab: GameVocab, layout: Did, setup: ChatSetup) -> Self {
        let chat = Self::make_chat(&vocab, layout, &setup);
        let mut p = Self {
            chat,
            vocab,
            layout,
            setup,
            registry: CommandRegistry::default(),
            builtin_names: Vec::new(),
            outgoing: Vec::new(),
            dropped: 0,
            requests: Vec::new(),
        };
        p.register_builtins();
        p
    }

    fn make_chat(vocab: &GameVocab, layout: Did, setup: &ChatSetup) -> Chat {
        let mut chat = Chat::new(vocab.clone(), layout);
        chat.command_line.setup = setup.command_setup.clone();
        chat.texts = setup.texts.clone();
        chat.modes = tessera_ui::game::chat::ChannelModes::new(&setup.modes, &setup.command_setup);
        chat.opacity = setup.opacity.clone();
        chat
    }

    /// Registers Tessera's commands (§10 item 3): `help` and `plugins` under Tessera's names
    /// and the original's names read from the install, and every channel word of the mode
    /// table (plain rows) with the default word as `Say`.
    fn register_builtins(&mut self) {
        let info = |id| CommandInfo {
            id,
            substitutes: false,
        };
        let mut names = Vec::new();
        for (entry, id) in [
            ("help".to_string(), HELP),
            (self.setup.help_entry.clone(), HELP),
            ("plugins".to_string(), PLUGINS),
            (self.setup.plugins_entry.clone(), PLUGINS),
        ] {
            names.extend(self.registry.register_list(&entry, info(id)));
        }
        let default_word = self.setup.command_setup.default_word.clone();
        if self
            .registry
            .register(&default_word, info(CHANNEL_BASE + CHAT_SAY))
        {
            names.push(default_word);
        }
        for (entry, chat_type, kind) in self.setup.modes.clone() {
            if kind != ModeKind::Plain || type_bit(chat_type).is_none() {
                continue;
            }
            names.extend(
                self.registry
                    .register_list(&entry, info(CHANNEL_BASE + chat_type)),
            );
        }
        self.builtin_names = names;
    }

    /// Creates the configured windows and tabs and applies the opacity (§3.3, §3.5).
    ///
    /// # Errors
    /// The first creation error of [`Chat::open`]; windows and tabs that could be created
    /// stay.
    pub fn open(&mut self, ui: &mut Ui) -> Result<(), PanelsError> {
        let r = self.chat.open(ui, &self.setup.config);
        self.chat.apply_opacity(ui, self.setup.opacity_value);
        r.map_err(PanelsError::from)
    }

    /// The primary window's root element (P2), once open.
    #[must_use]
    pub fn primary_root(&self) -> Option<ElementKey> {
        self.chat.primary().map(|i| self.chat.windows[i].root)
    }

    /// Whether the windows are open and the primary window still exists.
    #[must_use]
    pub fn is_open(&self, ui: &Ui) -> bool {
        self.primary_root().is_some_and(|k| ui.exists(k))
    }

    /// Destroys every chat window (primary and secondary) and forgets them, so that a
    /// later [`ChatPanel::open`] builds them again. The history and the registry are kept;
    /// lines already queued for the plugins stay queued.
    pub fn close(&mut self, ui: &mut Ui) {
        for w in &self.chat.windows {
            if ui.exists(w.root) {
                ui.destroy(w.root);
            }
        }
        let history = std::mem::take(&mut self.chat.command_line.history);
        self.chat = Self::make_chat(&self.vocab, self.layout, &self.setup);
        self.chat.command_line.history = history;
    }

    /// Every built-in name (Tessera's and the install's help and plugin-manager names and
    /// the channel words), as registered. A plugin host leaves these out of `AddCommand`
    /// and adds them to `GetCommands` (LPA §11.1).
    #[must_use]
    pub fn builtin_names(&self) -> &[String] {
        &self.builtin_names
    }

    /// Whether `name` is a built-in command name (exactly as given, case-insensitively,
    /// `chat-window.md` §8.2).
    #[must_use]
    pub fn is_builtin(&self, name: &str) -> bool {
        !name.is_empty() && self.registry.is_command(name)
    }

    /// The configuration in use.
    #[must_use]
    pub fn setup(&self) -> &ChatSetup {
        &self.setup
    }

    /// Posts a line of chat type `chat_type` (1–64) to every log that takes it (§4.3) and
    /// queues it for the plugins. Returns the number of logs that took it.
    pub fn post(&mut self, ui: &mut Ui, chat_type: u32, text: &str) -> usize {
        let Some(mask) = type_bit(chat_type) else {
            return 0;
        };
        self.post_mask(ui, mask, text)
    }

    /// Posts a Tessera system line (type 8 `Log`, §10 item 2).
    pub fn post_system(&mut self, ui: &mut Ui, text: &str) -> usize {
        self.post(ui, CHAT_LOG, text)
    }

    fn post_mask(&mut self, ui: &mut Ui, mask: u64, text: &str) -> usize {
        self.queue(mask, text.to_string());
        self.chat.post_line(ui, mask, text)
    }

    fn queue(&mut self, mask: u64, text: String) {
        if self.outgoing.len() < MAX_OUTGOING {
            self.outgoing.push((mask, text));
        } else {
            if self.dropped == 0 {
                tracing::warn!(
                    "chat: {MAX_OUTGOING} lines wait for the plugins; further lines are shown \
                     but not delivered"
                );
            }
            self.dropped = self.dropped.saturating_add(1);
        }
    }

    /// Lines dropped for the plugins since the last [`ChatPanel::take_outgoing`] because
    /// [`MAX_OUTGOING`] lines were waiting.
    #[must_use]
    pub fn dropped(&self) -> u64 {
        self.dropped
    }

    /// The lines posted since the last call, (mask, text) in order, for the plugins'
    /// `Turbine.Chat.Received` (§10 item 9; the client delivers them as deferred work).
    pub fn take_outgoing(&mut self) -> Vec<(u64, String)> {
        if self.dropped > 0 {
            tracing::warn!(
                "chat: {} line(s) not delivered to the plugins",
                self.dropped
            );
            self.dropped = 0;
        }
        std::mem::take(&mut self.outgoing)
    }

    /// The requests made by commands since the last call.
    pub fn take_requests(&mut self) -> Vec<ChatRequest> {
        std::mem::take(&mut self.requests)
    }

    /// `Shell.IsCommand(name)` (§8.2): the name exactly as given, case-insensitively, among
    /// Tessera's names and the plugins' commands.
    #[must_use]
    pub fn is_command(&self, name: &str, shell: &dyn ShellCommands) -> bool {
        !name.is_empty() && (self.registry.is_command(name) || shell.lookup(name).is_some())
    }

    /// Runs a text as if it was submitted in the entry (§7.2) without touching the entry
    /// line (Tessera's own callers, for example a key binding that runs a command).
    pub fn run_line(
        &mut self,
        ui: &mut Ui,
        text: &str,
        shell: &mut dyn ShellCommands,
        plugins: &mut dyn PluginControl,
    ) -> i32 {
        let mut d = dispatch!(self, shell, plugins);
        let word = self.chat.modes.command_word(self.chat.mode).to_string();
        let r = self.chat.command_line.run_line(text, &word, "", &mut d);
        let (lines, reqs) = (d.lines, d.requests);
        if r == RESULT_UNKNOWN {
            // Unlike `Chat::submit`, the command line posts nothing itself: the
            // unknown-command line (§7.6, §10 item 6) goes to the logs and the plugins here.
            let t = self.chat.texts.unknown_command.clone();
            self.post_mask(ui, ERROR_MASK, &t);
        }
        self.finish(ui, lines, reqs);
        r
    }

    /// The chat window's handling of an input command (`chat-window.md` §6.2; see
    /// [`Chat::command`] for when the client offers it).
    pub fn command(
        &mut self,
        ui: &mut Ui,
        name: &str,
        shift: bool,
        ctrl: bool,
        shell: &mut dyn ShellCommands,
        plugins: &mut dyn PluginControl,
    ) -> CommandOutcome {
        let mut d = dispatch!(self, shell, plugins);
        let out = self.chat.command(ui, name, shift, ctrl, &mut d);
        let (lines, reqs) = (d.lines, d.requests);
        match out {
            // `Chat::submit` posted the unknown-command line to the logs (§7.6).
            CommandOutcome::Submitted(RESULT_UNKNOWN) => self.queue_unknown(),
            // `Chat::command` posted the no-reply-target line with mask 0x21 itself (§6.2,
            // §10 item 8); the plugins receive it too (§10 item 9, §9 Q18).
            CommandOutcome::Handled if name == "ChatModeReply" => {
                let t = self.chat.texts.no_reply_target.clone();
                self.queue(REPLY_MASK, t);
            }
            _ => {}
        }
        self.finish(ui, lines, reqs);
        out
    }

    /// Submits the entry line (§6.2). Returns the result, `None` for an empty entry.
    pub fn submit(
        &mut self,
        ui: &mut Ui,
        shell: &mut dyn ShellCommands,
        plugins: &mut dyn PluginControl,
    ) -> Option<i32> {
        let mut d = dispatch!(self, shell, plugins);
        let r = self.chat.submit(ui, &mut d);
        let (lines, reqs) = (d.lines, d.requests);
        if r == Some(RESULT_UNKNOWN) {
            // `Chat::submit` posted the unknown-command line to the logs (§7.6).
            self.queue_unknown();
        }
        self.finish(ui, lines, reqs);
        r
    }

    /// A typed character (§6.5, §6.6; [`Chat::text_input`]).
    pub fn text_input(&mut self, ui: &mut Ui, ch: char, now: f64) -> TextInputOutcome {
        self.chat.text_input(ui, ch, now)
    }

    /// Routes a UI event (tab clicks).
    pub fn handle_event(&mut self, ui: &mut Ui, ev: &UiEvent) {
        self.chat.handle_event(ui, ev);
    }

    /// Queues the unknown-command line that `tessera-ui` already posted to the logs.
    fn queue_unknown(&mut self) {
        let t = self.chat.texts.unknown_command.clone();
        self.queue(ERROR_MASK, t);
    }

    /// Posts (and queues) the lines the handlers produced and keeps their requests.
    fn finish(&mut self, ui: &mut Ui, lines: Vec<(u64, String)>, reqs: Vec<ChatRequest>) {
        for (mask, text) in lines {
            self.post_mask(ui, mask, &text);
        }
        self.requests.extend(reqs);
    }
}

/// The command handlers behind one submitted line.
struct Dispatch<'a> {
    registry: &'a CommandRegistry,
    builtin_names: &'a [String],
    channel_names: &'a std::collections::BTreeMap<u32, String>,
    shell: &'a mut dyn ShellCommands,
    plugins: &'a mut dyn PluginControl,
    lines: Vec<(u64, String)>,
    requests: Vec<ChatRequest>,
}

impl Dispatch<'_> {
    fn log(&mut self, text: String) {
        if let Some(m) = type_bit(CHAT_LOG) {
            self.lines.push((m, text));
        }
    }

    fn error(&mut self, text: String) {
        if let Some(m) = type_bit(CHAT_ERROR) {
            self.lines.push((m, text));
        }
    }

    /// The help command (§7.7, Tessera's wording): without arguments the command list,
    /// with a plugin command's short help after its name; with a name, that command's long
    /// help (`GetHelp` for plugin commands).
    fn help(&mut self, args: &str) -> i32 {
        let name = trim_ws(args);
        if !name.is_empty() {
            let name = name.split(char::is_whitespace).next().unwrap_or(name);
            if let Some(id) = self.registry.lookup(name).map(|i| i.id) {
                let text = match id {
                    HELP => "help [<command>]: lists the commands, or shows one command's help."
                        .to_string(),
                    PLUGINS => {
                        "plugins load <name> | unload [<apartment>] | list | refresh | manager"
                            .to_string()
                    }
                    _ => format!("{name}: sends text to that channel."),
                };
                self.log(text);
                return 1;
            }
            if let Some(id) = self.shell.lookup(name).filter(|id| *id & SHELL_BIT == 0) {
                let h = self.shell.help(id);
                let h = if h.is_empty() {
                    self.shell.short_help(id)
                } else {
                    h
                };
                self.log(if h.is_empty() {
                    format!("{name}: no help.")
                } else {
                    format!("{name}: {h}")
                });
                return 1;
            }
            self.error(format!("No command named {name}."));
            return 0;
        }
        let mut names: Vec<(String, String)> = self
            .builtin_names
            .iter()
            .map(|n| (n.clone(), String::new()))
            .collect();
        for n in self.shell.names() {
            let short = self
                .shell
                .lookup(&n)
                .filter(|id| *id & SHELL_BIT == 0)
                .map(|id| self.shell.short_help(id))
                .unwrap_or_default();
            names.push((n, short));
        }
        names.sort_by_key(|(n, _)| n.to_lowercase());
        names.dedup_by(|a, b| a.0.eq_ignore_ascii_case(&b.0));
        let list: Vec<String> = names
            .into_iter()
            .map(|(n, s)| {
                if s.is_empty() {
                    n
                } else {
                    format!("{n} ({s})")
                }
            })
            .collect();
        self.log(format!("Commands: {}", list.join(", ")));
        1
    }

    /// `/plugins` (`lua-plugin-api.md` §6.4) with Tessera's wording.
    fn plugins(&mut self, args: &str) -> i32 {
        let t = trim_ws(args);
        let (sub, rest) = match t.find(|c: char| c.is_whitespace()) {
            Some(i) => (&t[..i], trim_ws(&t[i..])),
            None => (t, ""),
        };
        match sub.to_ascii_lowercase().as_str() {
            "load" if !rest.is_empty() => match self.plugins.load(rest) {
                Ok(()) => {
                    self.log(format!("Loading plugin {rest}."));
                    1
                }
                Err(e) => {
                    self.error(format!("Cannot load plugin {rest}: {e}"));
                    0
                }
            },
            "unload" => {
                let target = (!rest.is_empty()).then_some(rest);
                match self.plugins.unload(target) {
                    Ok(()) => {
                        self.log(match target {
                            Some(a) => format!("Unloaded {a}."),
                            None => "Unloaded every plugin.".to_string(),
                        });
                        1
                    }
                    Err(e) => {
                        self.error(format!("Cannot unload: {e}"));
                        0
                    }
                }
            }
            "list" => {
                let rows = self.plugins.rows();
                if rows.is_empty() {
                    self.log("No plugins found.".to_string());
                }
                for r in rows {
                    let state = if r.loaded { "loaded" } else { "available" };
                    // LPA §5.2, §6.1: no apartment means `Default`; names compare
                    // case-insensitively.
                    let apt = if r.apartment.is_empty()
                        || r.apartment.eq_ignore_ascii_case(DEFAULT_APARTMENT)
                    {
                        String::new()
                    } else {
                        format!(" [{}]", r.apartment)
                    };
                    self.log(format!("{} {}{apt} ({state})", r.name, r.version));
                }
                1
            }
            "refresh" => {
                self.plugins.refresh();
                self.log("Plugin list refreshed.".to_string());
                1
            }
            "manager" => {
                self.requests.push(ChatRequest::OpenPluginManager);
                1
            }
            _ => {
                self.log(
                    "Usage: plugins load <name> | unload [<apartment>] | list | refresh | manager"
                        .to_string(),
                );
                0
            }
        }
    }

    /// A channel word: the local echo of §10 item 2 (Tessera's wording).
    fn echo(&mut self, chat_type: u32, word: &str, args: &str) -> i32 {
        let Some(mask) = type_bit(chat_type) else {
            return 0;
        };
        let text = trim_ws(args);
        if text.is_empty() {
            return 1;
        }
        let name = self
            .channel_names
            .get(&chat_type)
            .cloned()
            .unwrap_or_else(|| word.to_string());
        self.lines.push((mask, format!("[{name}] {text}")));
        1
    }
}

impl CommandHost for Dispatch<'_> {
    fn lookup(&self, word: &str) -> Option<CommandInfo> {
        self.registry.lookup(word).or_else(|| {
            self.shell
                .lookup(word)
                .filter(|id| *id & SHELL_BIT == 0)
                .map(|id| CommandInfo {
                    id: id | SHELL_BIT,
                    substitutes: false,
                })
        })
    }

    fn run(&mut self, info: CommandInfo, word: &str, args: &str, _tokens: &[String]) -> i32 {
        match info.id {
            HELP => self.help(args),
            PLUGINS => self.plugins(args),
            id if id & SHELL_BIT != 0 => self.shell.run(id & !SHELL_BIT, word, args),
            id if (CHANNEL_BASE..CHANNEL_BASE + 65).contains(&id) => {
                self.echo(id - CHANNEL_BASE, word, args)
            }
            _ => 0,
        }
    }
}

/// The arguments a command word gets from a text (§7.4), for callers that run lines
/// themselves; re-exported from the UI's command line.
#[must_use]
pub fn command_arguments(text: &str, word: &str) -> String {
    arguments(trim_ws(text), word)
}

/// The names of a `Commands` entry text (§7.1 splitting rule).
#[must_use]
pub fn entry_names(entry: &str) -> Vec<String> {
    split_names(entry)
}
