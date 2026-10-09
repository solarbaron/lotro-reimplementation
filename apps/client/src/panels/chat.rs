// SPDX-License-Identifier: GPL-3.0-or-later
//! The chat window P2 (`chat-window.md`) with Tessera's sandbox content (§10): windows and
//! tabs from `GOP_ChatWindows`, the log and the entry line.
//!
//! `tessera-ui`'s [`Chat`] does the window work (tabs, filters, log, entry, channel modes);
//! this module supplies its configuration ([`ChatSetup`], with the install's texts and mode
//! words from [`ChatSetup::load_texts`]) and keeps every line the window takes (its own
//! lines and the reply line of §6.2 that `tessera-ui` posts itself) so that the client can
//! hand them to the plugins' `Turbine.Chat.Received` as deferred work (§10 item 9).
//!
//! The window keeps no command registry and parses no command line: a submitted entry line
//! comes back as an [`EntryLine`] that the client runs through its one command dispatcher
//! (`crate::commands`, decision 0020 item 1), whose registry holds the built-ins, the
//! channel words and the plugins' commands; line up and down browse the dispatcher's
//! history ([`EntryHistory`]).

use std::collections::BTreeMap;

use tessera_formats::property::PropertySchema;
use tessera_formats::property_set::PropertySetRecord;
use tessera_formats::{DataIdMap, Did, Record, StringTable};
use tessera_ui::Ui;
use tessera_ui::game::GameVocab;
use tessera_ui::game::chat::mask::type_bit;
use tessera_ui::game::chat::window::REPLY_MASK;
use tessera_ui::game::chat::{
    Chat, ChatConfig, ChatTexts, CommandOutcome, CommandSetup, EntryHistory, EntryLine, ModeKind,
    OpacityTable, TabConfig, TextInputOutcome, WindowConfig,
};
use tessera_ui::game::vocab::GamePids;
use tessera_ui::{ElementKey, UiEvent};
use tessera_world::WorldSource;

use super::PanelsError;

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
/// `Chat` token of the no-reply-target line (§6.2 `ChatModeReply`).
pub const TOKEN_NO_REPLY_TARGET: u32 = 0x00CC_89CC;

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

/// Everything the chat window needs from the install and Tessera's settings.
#[derive(Debug, Clone, PartialEq)]
pub struct ChatSetup {
    /// Windows and tabs (`GOP_ChatWindows`, §3.3).
    pub config: ChatConfig,
    /// The `CommandSetup` texts the window uses (§6.2 `START_COMMAND`, §6.5 mode words),
    /// read from the install by the client.
    pub command_setup: CommandSetup,
    /// The text of the no-reply-target line and the channel names.
    pub texts: ChatTexts,
    /// The channel mode rows (§6.5): (`Commands` entry text, chat type, kind).
    pub modes: Vec<(String, u32, ModeKind)>,
    /// Background opacity images (§3.5).
    pub opacity: OpacityTable,
    /// `GOP_ChatOpacity` (§3.5; default 25).
    pub opacity_value: i32,
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
    /// Reads the chat window's texts and mode words from the install's string tables
    /// (§10 items 4 and 8; `stringtable` is the `STRINGTABLE` DID map):
    ///
    /// - `CommandSetup` (key 2): the command prefix (token 0x0DA28738), the substitution
    ///   marker (0x02409858) and the default channel word (0x04DA4989);
    /// - `Chat` (key 0x1000004D): the no-reply-target line (0x00CC89CC);
    /// - `Commands` (key 0x10000007): the fixed rows of the mode table ([`MODE_ROWS`],
    ///   §6.5).
    ///
    /// The command names (§10 item 3) and the unknown-command line (§7.6) are the command
    /// dispatcher's, which reads them itself (`crate::commands::text`).
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
            &mut self.texts.no_reply_target,
            text(CHAT_TABLE_KEY, TOKEN_NO_REPLY_TARGET),
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

/// The chat window P2.
#[derive(Debug, Clone)]
pub struct ChatPanel {
    /// The UI's chat system.
    pub chat: Chat,
    vocab: GameVocab,
    layout: Did,
    setup: ChatSetup,
    outgoing: Vec<(u64, String)>,
    dropped: u64,
}

impl ChatPanel {
    /// A chat panel for the `chatwindow` layout `layout`. Nothing is created until
    /// [`ChatPanel::open`].
    #[must_use]
    pub fn new(vocab: GameVocab, layout: Did, setup: ChatSetup) -> Self {
        let chat = Self::make_chat(&vocab, layout, &setup);
        Self {
            chat,
            vocab,
            layout,
            setup,
            outgoing: Vec::new(),
            dropped: 0,
        }
    }

    fn make_chat(vocab: &GameVocab, layout: Did, setup: &ChatSetup) -> Chat {
        let mut chat = Chat::new(vocab.clone(), layout);
        chat.setup = setup.command_setup.clone();
        chat.texts = setup.texts.clone();
        chat.modes = tessera_ui::game::chat::ChannelModes::new(&setup.modes, &setup.command_setup);
        chat.opacity = setup.opacity.clone();
        chat
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
    /// later [`ChatPanel::open`] builds them again. Lines already queued for the plugins
    /// stay queued (the history is the dispatcher's and is not touched).
    pub fn close(&mut self, ui: &mut Ui) {
        for w in &self.chat.windows {
            if ui.exists(w.root) {
                ui.destroy(w.root);
            }
        }
        self.chat = Self::make_chat(&self.vocab, self.layout, &self.setup);
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

    /// Posts a line of chat type `chat_type` to the logs **without** queueing it for the
    /// plugins (lines the plugin host already hands to `Received` itself). Returns the
    /// number of logs that took it.
    pub fn show(&mut self, ui: &mut Ui, chat_type: u32, text: &str) -> usize {
        let Some(mask) = type_bit(chat_type) else {
            return 0;
        };
        self.chat.post_line(ui, mask, text)
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

    /// The chat window's handling of an input command (`chat-window.md` §6.2; see
    /// [`Chat::command`] for when the client offers it). Line up and down browse `history`
    /// (the dispatcher's, §6.3). A submitted line comes back as
    /// [`CommandOutcome::Submitted`]: the caller runs it through the command dispatcher
    /// (decision 0020 item 1) once it no longer holds the UI.
    pub fn command(
        &mut self,
        ui: &mut Ui,
        name: &str,
        shift: bool,
        ctrl: bool,
        history: &mut dyn EntryHistory,
    ) -> CommandOutcome {
        let out = self.chat.command(ui, name, shift, ctrl, history);
        if out == CommandOutcome::Handled && name == "ChatModeReply" {
            // `Chat::command` posted the no-reply-target line with mask 0x21 itself (§6.2,
            // §10 item 8); the plugins receive it too (§10 item 9, §9 Q18).
            let t = self.chat.texts.no_reply_target.clone();
            self.queue(REPLY_MASK, t);
        }
        out
    }

    /// Submits the entry line (§6.2): the line for the command dispatcher, `None` for an
    /// empty entry.
    pub fn submit(&mut self, ui: &mut Ui) -> Option<EntryLine> {
        self.chat.submit(ui)
    }

    /// A typed character (§6.5, §6.6; [`Chat::text_input`]).
    pub fn text_input(&mut self, ui: &mut Ui, ch: char, now: f64) -> TextInputOutcome {
        self.chat.text_input(ui, ch, now)
    }

    /// Routes a UI event (tab clicks).
    pub fn handle_event(&mut self, ui: &mut Ui, ev: &UiEvent) {
        self.chat.handle_event(ui, ev);
    }
}
