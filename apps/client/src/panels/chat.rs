// SPDX-License-Identifier: GPL-3.0-or-later
//! The chat window P2 (`chat-window.md`) with Tessera's sandbox content (§10): windows and
//! tabs from `GOP_ChatWindows`, the log and the entry line.
//!
//! `tessera-ui`'s [`Chat`] does the window work (tabs, filters, log, entry, channel modes);
//! this module supplies its configuration ([`ChatSetup`]: the install's defaults, the
//! no-reply-target text from [`ChatSetup::load_texts`], and the command syntax and mode
//! table from the command dispatcher's one read of the `CommandSetup` and `Commands`
//! tables, [`ChatSetup::apply_command_text`] and [`ChatPanel::set_command_text`], §10
//! item 11) and keeps every line the window takes (its own lines and the reply line of
//! §6.2 that `tessera-ui` posts itself) so that the client can hand them to the plugins'
//! `Turbine.Chat.Received` as deferred work (§10 item 9).
//!
//! The window keeps no command registry and parses no command line: a submitted entry line
//! comes back as an [`EntryLine`] that the client runs through its one command dispatcher
//! (`crate::commands`, decision 0020 item 1), whose registry holds the built-ins, the
//! channel words and the plugins' commands; line up and down browse the dispatcher's
//! history ([`EntryHistory`]).

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
use crate::commands::text::CommandText;

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

/// `STRINGTABLE` key of the `Chat` table (`chat-window.md` §4.6, §6.2).
pub const CHAT_TABLE_KEY: u32 = 0x1000_004D;
/// `Chat` token of the no-reply-target line (§6.2 `ChatModeReply`).
pub const TOKEN_NO_REPLY_TARGET: u32 = 0x00CC_89CC;

/// Everything the chat window needs from the install and Tessera's settings.
#[derive(Debug, Clone, PartialEq)]
pub struct ChatSetup {
    /// Windows and tabs (`GOP_ChatWindows`, §3.3).
    pub config: ChatConfig,
    /// The `CommandSetup` texts the window uses (§6.2 `START_COMMAND`, §6.5 mode words),
    /// from the command dispatcher's read of the install ([`Self::apply_command_text`]).
    pub command_setup: CommandSetup,
    /// The text of the no-reply-target line and the channel names.
    pub texts: ChatTexts,
    /// The channel mode rows (§6.5): (`Commands` entry text, chat type, kind), from the
    /// command dispatcher's read of the install ([`Self::apply_command_text`]).
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
    /// 0x68) and `UI_ChatOpacities` (key 0x1000000C). The no-reply-target text comes from
    /// [`ChatSetup::load_texts`], the command syntax and the modes from the dispatcher's
    /// read ([`ChatSetup::apply_command_text`]).
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
    /// Reads the chat window's own text from the install's string tables (§10 item 8;
    /// `stringtable` is the `STRINGTABLE` DID map): the no-reply-target line (token
    /// 0x00CC89CC of the `Chat` table, key 0x1000004D). A missing table or token keeps the
    /// current text. Returns the number of texts read (0 or 1).
    ///
    /// The command prefix, the substitution marker, the default channel word and the mode
    /// table are **not** read here: the window takes them from the command dispatcher's one
    /// read of the `CommandSetup` and `Commands` tables (§10 item 11,
    /// [`Self::apply_command_text`]), so it offers only mode words the registry holds.
    pub fn load_texts(&mut self, source: &dyn WorldSource, stringtable: &DataIdMap) -> usize {
        let text = (|| {
            let did = stringtable.get(CHAT_TABLE_KEY)?.value;
            let bytes = source.read(did).ok()??;
            let table = StringTable::parse_expecting(&bytes, did).ok()?;
            table
                .get(TOKEN_NO_REPLY_TARGET)
                .map(tessera_formats::string_table::StringTableEntry::plain_text)
        })();
        match text.filter(|t| !t.is_empty()) {
            Some(t) => {
                self.texts.no_reply_target = t;
                1
            }
            None => 0,
        }
    }

    /// Takes the command syntax and the mode table from the command dispatcher's read of
    /// the install (§10 item 11): the prefix, the substitution marker and the default
    /// channel word that were read (an entry that was not keeps the current text), and the
    /// rows of the 26 fixed mode-table rows that were read (none read: one `Say` row on the
    /// default word). Returns the number of mode rows.
    pub fn apply_command_text(&mut self, text: &CommandText) -> usize {
        self.command_setup.prefix.clone_from(&text.prefix);
        if let Some(m) = &text.marker {
            self.command_setup.marker.clone_from(m);
        }
        if let Some(w) = &text.default_word {
            self.command_setup.default_word.clone_from(w);
        }
        self.modes = if text.modes.is_empty() {
            // Without the install's mode table the window has one `Say` row on the default
            // word, which the registry holds (a Tessera name, or the install's default word
            // registered as a `Say` word, §10 item 3).
            vec![(
                self.command_setup.default_word.clone(),
                CHAT_SAY,
                ModeKind::Plain,
            )]
        } else {
            text.modes
                .iter()
                .map(|m| (m.names.clone(), m.chat_type, m.kind))
                .collect()
        };
        self.modes.len()
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

    /// Takes the command syntax and the mode table of the command dispatcher's read of the
    /// install (§10 item 11, [`ChatSetup::apply_command_text`]) without closing the
    /// windows: the open entry uses them from its next key on. The current mode of each
    /// tab is kept.
    pub fn set_command_text(&mut self, text: &CommandText) {
        self.setup.apply_command_text(text);
        self.chat.setup = self.setup.command_setup.clone();
        self.chat.modes =
            tessera_ui::game::chat::ChannelModes::new(&self.setup.modes, &self.setup.command_setup);
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

#[cfg(test)]
mod tests {
    use super::*;
    use crate::commands::Dispatcher;
    use crate::commands::registry::Channel;
    use crate::commands::text::ModeText;
    use tessera_ui::game::chat::command::split_names;

    fn text() -> CommandText {
        CommandText {
            prefix: "#".into(),
            default_word: Some("talk".into()),
            marker: Some("%".into()),
            modes: vec![
                ModeText {
                    names: "talk;t".into(),
                    chat_type: 5,
                    kind: ModeKind::Plain,
                    channel: Channel::Say,
                },
                ModeText {
                    names: "whisper".into(),
                    chat_type: 6,
                    kind: ModeKind::Plain,
                    channel: Channel::Tell,
                },
                ModeText {
                    names: "again".into(),
                    chat_type: 6,
                    kind: ModeKind::TellAgain,
                    channel: Channel::TellReply,
                },
                ModeText {
                    names: "band;b".into(),
                    chat_type: 11,
                    kind: ModeKind::Plain,
                    channel: Channel::Unavailable,
                },
            ],
            ..CommandText::default()
        }
    }

    /// `chat-window.md` §10 item 11: the window takes its syntax and mode table from the
    /// dispatcher's one read, and every mode word it offers is a name the registry holds.
    #[test]
    fn modes_come_from_the_dispatchers_read() {
        let mut d = Dispatcher::new();
        d.apply_text(text());
        let mut setup = ChatSetup::sandbox();
        assert_eq!(setup.apply_command_text(d.text()), 4);
        assert_eq!(setup.command_setup.prefix, "#");
        assert_eq!(setup.command_setup.marker, "%");
        assert_eq!(setup.command_setup.default_word, "talk");
        assert_eq!(
            setup.modes,
            [
                ("talk;t".to_owned(), 5, ModeKind::Plain),
                ("whisper".to_owned(), 6, ModeKind::Plain),
                ("again".to_owned(), 6, ModeKind::TellAgain),
                ("band;b".to_owned(), 11, ModeKind::Plain),
            ]
        );
        let reg = d.registry();
        for (names, _, _) in &setup.modes {
            for n in split_names(names) {
                assert!(reg.is_command(&n), "{n}: a mode word the registry holds");
            }
        }
        // The window's switch words use the read prefix and marker.
        let modes = tessera_ui::game::chat::ChannelModes::new(&setup.modes, &setup.command_setup);
        assert_eq!(modes.switch_word("#b").map(|r| r.chat_type), Some(11));
        assert_eq!(modes.switch_word("%whisper").map(|r| r.chat_type), Some(6));
        assert!(modes.switch_word("/b").is_none());
        assert_eq!(modes.command_word(5), "talk");
    }

    /// Without the install's words (the read failed): Tessera's syntax and one `Say` row
    /// on the default word, which the registry holds as a Tessera name.
    #[test]
    fn defaults_without_the_install() {
        let d = Dispatcher::new();
        let mut setup = ChatSetup::sandbox();
        assert_eq!(setup.apply_command_text(d.text()), 1);
        assert_eq!(setup.command_setup.prefix, "/");
        assert_eq!(setup.modes, [("say".to_owned(), CHAT_SAY, ModeKind::Plain)]);
        assert!(d.registry().is_command("say"));
        // Only a default word read: one `Say` row on it.
        let mut setup = ChatSetup::sandbox();
        let t = CommandText {
            default_word: Some("talk".into()),
            ..CommandText::default()
        };
        assert_eq!(setup.apply_command_text(&t), 1);
        assert_eq!(
            setup.modes,
            [("talk".to_owned(), CHAT_SAY, ModeKind::Plain)]
        );
    }
}
