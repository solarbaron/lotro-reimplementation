// SPDX-License-Identifier: GPL-3.0-or-later
//! The command words and lines that come from the user's install (`chat-window.md` §10
//! items 3, 4, 6 and 11; decision 0020 items 1, 2, 4 and 5): the command prefix, the
//! default channel word, the substitution marker and the unknown-command line of the
//! `CommandSetup` table, and the help, plugin-manager and mode-table name lists of the
//! `Commands` table. Read once, at run time, off the frame thread, by the dispatcher; the
//! chat window takes its mode table from the same read (§10 item 11), so it offers only mode
//! words the registry holds. When a table or entry is missing Tessera's own defaults apply.
//! Nothing of the tables' text is built in.

use std::sync::Arc;

use tessera_dat::ArchiveSet;
use tessera_text::{RenderOptions, Strings, Values};

use tessera_ui::game::chat::ModeKind;

use super::line::{self, DEFAULT_PREFIX};
use super::registry::{Channel, SocialWord};

/// STRINGTABLE key of the `CommandSetup` table (`chat-window.md` §7.2).
pub const COMMAND_SETUP_KEY: u32 = 2;
/// STRINGTABLE key of the `Commands` table (`chat-window.md` §6.5, §8.1).
pub const COMMANDS_KEY: u32 = 0x1000_0007;
/// `CommandSetup` token of the command prefix (§7.2 step 4).
pub const TOKEN_PREFIX: u32 = 0x0DA2_8738;
/// `CommandSetup` token of the default channel word, the `Say` word (§7.2).
pub const TOKEN_DEFAULT_WORD: u32 = 0x04DA_4989;
/// `CommandSetup` token of the substitution marker (§6.5, §7.5).
pub const TOKEN_MARKER: u32 = 0x0240_9858;
/// `CommandSetup` token of the unknown-command line (§7.6).
pub const TOKEN_UNKNOWN: u32 = 0x04FE_7F13;
/// `Commands` token of the help command's names (§8.1).
pub const TOKEN_HELP: u32 = 0x0DA3_EC70;
/// `Commands` token of the plugin-manager command's names (§8.1).
pub const TOKEN_PLUGINS: u32 = 0x073F_0613;

/// The 26 fixed rows of the channel-mode table (`chat-window.md` §6.5, §10 item 3): a
/// `Commands` token, the row's chat type and word kind (what the chat window's modes use)
/// and what its words do in Tessera (revision 4, §15.10 CT2: `Say`, `Tell`, reply, retell,
/// fellowship and the eight user-channel slots run their handlers; every other channel is
/// not carried by the Tessera protocol). The reply and retell rows follow the correction
/// CW43 (0x0A48C3E9 is the reply word, 0x048CE78C the retell word). Neither variant group
/// is read while Q12 (the game-state test that picks one) is open, so `IsCommand` answers
/// false for their words (§10 item 3).
pub const MODE_TABLE: [(u32, u32, ModeKind, Channel); 26] = [
    (0x04DA_4989, 5, ModeKind::Plain, Channel::Say),
    (0x0DA4_AC6C, 6, ModeKind::Plain, Channel::Tell),
    (0x048C_E78C, 6, ModeKind::TellAgain, Channel::Retell),
    (0x0A48_C3E9, 6, ModeKind::Reply, Channel::Reply),
    (0x0384_53D4, 11, ModeKind::Plain, Channel::Fellowship),
    (0x07B5_3A04, 23, ModeKind::Plain, Channel::Unavailable),
    (0x06F0_47CE, 15, ModeKind::Plain, Channel::Unavailable),
    (0x0253_3A8E, 46, ModeKind::Plain, Channel::Unavailable),
    (0x0253_3B7E, 47, ModeKind::Plain, Channel::Unavailable),
    (0x0AE9_AEE7, 16, ModeKind::Plain, Channel::Unavailable),
    (0x0AE9_AE73, 18, ModeKind::Plain, Channel::Unavailable),
    (0x0AE9_8903, 27, ModeKind::Plain, Channel::Unavailable),
    (0x040E_21EC, 19, ModeKind::Plain, Channel::Unavailable),
    (0x03AC_BD91, 9, ModeKind::Plain, Channel::Unavailable),
    (0x03AC_BD92, 10, ModeKind::Plain, Channel::Unavailable),
    (0x08B3_A4E1, 28, ModeKind::Plain, Channel::Slot(1)),
    (0x08B3_A4E2, 29, ModeKind::Plain, Channel::Slot(2)),
    (0x08B3_A4E3, 30, ModeKind::Plain, Channel::Slot(3)),
    (0x08B3_A4E4, 31, ModeKind::Plain, Channel::Slot(4)),
    (0x08B3_A4E5, 39, ModeKind::Plain, Channel::Slot(5)),
    (0x08B3_A4E6, 40, ModeKind::Plain, Channel::Slot(6)),
    (0x08B3_A4E7, 41, ModeKind::Plain, Channel::Slot(7)),
    (0x08B3_A4E8, 42, ModeKind::Plain, Channel::Slot(8)),
    (0x0984_23C4, 38, ModeKind::Plain, Channel::Unavailable),
    (0x06E8_82B4, 44, ModeKind::Plain, Channel::Unavailable),
    (0x0538_82B4, 45, ModeKind::Plain, Channel::Unavailable),
];

/// The `Commands` entries of the P4 built-ins that are not mode-table rows
/// (`chat-window.md` §15.10 CT2; `social-panels.md` §9 TR11): a token and its built-in.
pub const SOCIAL_COMMANDS: [(u32, SocialWord); 11] = [
    (0x03A7_284C, SocialWord::Join),
    (0x0DCD_958C, SocialWord::Part),
    (0x0A55_AF03, SocialWord::Channels),
    (0x098B_3A44, SocialWord::ChSay),
    (0x03D8_B7E4, SocialWord::Friend),
    (0x03FE_1B25, SocialWord::Ignore),
    (0x0F31_08C5, SocialWord::Invite),
    (0x05A6_3F75, SocialWord::Leave),
    (0x0F8F_9533, SocialWord::Dismiss),
    (0x094A_D2D5, SocialWord::Promote),
    (0x0313_9345, SocialWord::Promote),
];

/// The disband names (`social-panels.md` §7.1, TR11).
pub const DISBAND_COMMANDS: [u32; 2] = [0x0F8E_33D4, 0x05B5_7444];

/// The sub-word entries of `friend` and `ignore` (`social-panels.md` §7.1, TR11).
pub const SUB_ADD: [u32; 3] = [0x07EB_EF14, 0x0B2A_C9B4, 0x0B53_0F84];
/// The "remove" sub-word entries.
pub const SUB_REMOVE: [u32; 3] = [0x070A_93F5, 0x0901_5875, 0x0B6B_9465];
/// The "list" sub-word entries.
pub const SUB_LIST: [u32; 5] = [
    0x0531_B214,
    0x02AD_D114,
    0x06A8_8324,
    0x0845_C0E4,
    0x0EBF_BBD4,
];

/// One mode-table row read from the install: its `Commands` entry (a name list), chat
/// type, word kind and channel.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ModeText {
    /// The entry text (names split by `chat-window.md` §7.1).
    pub names: String,
    /// The row's chat type (§6.5).
    pub chat_type: u32,
    /// The word kind (§6.5).
    pub kind: ModeKind,
    /// What the row's words do in Tessera.
    pub channel: Channel,
}

/// Longest entry text used, bytes (longer entries are ignored).
pub const MAX_ENTRY_LEN: usize = 1024;

/// The install's command words (module docs).
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct CommandText {
    /// The command prefix *P* (default `/`).
    pub prefix: String,
    /// The default channel word (the `Say` word) of `CommandSetup`, if read.
    pub default_word: Option<String>,
    /// The substitution marker of `CommandSetup`, if read (the chat window's mode words,
    /// §6.5).
    pub marker: Option<String>,
    /// The unknown-command line, if read.
    pub unknown_line: Option<String>,
    /// The help command's name list, if read.
    pub help_names: Option<String>,
    /// The plugin-manager command's name list, if read.
    pub plugins_names: Option<String>,
    /// The rows of the mode table that were read ([`MODE_TABLE`], in table order): the
    /// dispatcher registers their names as channel commands and the chat window takes its
    /// modes from them (`chat-window.md` §10 items 3 and 11).
    pub modes: Vec<ModeText>,
    /// The name lists of the P4 built-ins that were read ([`SOCIAL_COMMANDS`] and the
    /// disband entries), in table order.
    pub social: Vec<(SocialWord, String)>,
    /// The install's sub-words of `friend` and `ignore` (TR11): every name of the "add",
    /// "remove" and "list" entries that were read.
    pub sub_add: Vec<String>,
    /// The "remove" sub-words.
    pub sub_remove: Vec<String>,
    /// The "list" sub-words.
    pub sub_list: Vec<String>,
}

impl Default for CommandText {
    fn default() -> Self {
        Self {
            prefix: DEFAULT_PREFIX.to_owned(),
            default_word: None,
            marker: None,
            unknown_line: None,
            help_names: None,
            plugins_names: None,
            modes: Vec::new(),
            social: Vec::new(),
            sub_add: Vec::new(),
            sub_remove: Vec::new(),
            sub_list: Vec::new(),
        }
    }
}

/// One entry rendered as plain text without arguments, trimmed of §7.2 whitespace;
/// `None` when the entry is missing, empty or too long.
fn entry(strings: &Strings, table: Option<u32>, token: u32) -> Option<String> {
    let message = strings.message(table?, token).ok()?;
    let text =
        tessera_text::render(&message, &Values::new(), &RenderOptions::default()).plain_text();
    let text = line::trim(&text);
    (!text.is_empty() && text.len() <= MAX_ENTRY_LEN).then(|| text.to_owned())
}

impl CommandText {
    /// The words of `strings` (their named-table map finds the two tables).
    #[must_use]
    pub fn from_strings(strings: &Strings) -> Self {
        let named = strings.named_tables();
        let setup = named.by_key(COMMAND_SETUP_KEY);
        let commands = named.by_key(COMMANDS_KEY);
        let modes = MODE_TABLE
            .iter()
            .filter_map(|&(token, chat_type, kind, channel)| {
                entry(strings, commands, token).map(|names| ModeText {
                    names,
                    chat_type,
                    kind,
                    channel,
                })
            })
            .collect();
        let mut social: Vec<(SocialWord, String)> = SOCIAL_COMMANDS
            .iter()
            .filter_map(|&(token, w)| entry(strings, commands, token).map(|n| (w, n)))
            .collect();
        social.extend(
            DISBAND_COMMANDS
                .iter()
                .filter_map(|&t| entry(strings, commands, t).map(|n| (SocialWord::Disband, n))),
        );
        let words = |tokens: &[u32]| -> Vec<String> {
            tokens
                .iter()
                .filter_map(|&t| entry(strings, commands, t))
                .flat_map(|e| {
                    super::registry::split_names(&e)
                        .map(str::to_owned)
                        .collect::<Vec<_>>()
                })
                .collect()
        };
        let text = Self {
            prefix: entry(strings, setup, TOKEN_PREFIX)
                .unwrap_or_else(|| DEFAULT_PREFIX.to_owned()),
            default_word: entry(strings, setup, TOKEN_DEFAULT_WORD),
            marker: entry(strings, setup, TOKEN_MARKER),
            unknown_line: entry(strings, setup, TOKEN_UNKNOWN),
            help_names: entry(strings, commands, TOKEN_HELP),
            plugins_names: entry(strings, commands, TOKEN_PLUGINS),
            modes,
            social,
            sub_add: words(&SUB_ADD),
            sub_remove: words(&SUB_REMOVE),
            sub_list: words(&SUB_LIST),
        };
        if setup.is_some() && commands.is_some() {
            tracing::info!(
                prefix_from_install = text.prefix != DEFAULT_PREFIX,
                unknown_line = text.unknown_line.is_some(),
                mode_rows = text.modes.len(),
                social = text.social.len(),
                "command words read"
            );
        } else {
            // chat-window.md §10 item 3: a table that cannot be read is a warning.
            tracing::warn!(
                setup = setup.is_some(),
                commands = commands.is_some(),
                prefix_from_install = text.prefix != DEFAULT_PREFIX,
                mode_rows = text.modes.len(),
                "command words: a command table cannot be read; only Tessera's names"
            );
        }
        text
    }

    /// The words of the selected language archive of `set` (`language`, else English).
    /// Blocking (reads the archives): not for the frame thread. Defaults when the set has
    /// no language archive.
    #[must_use]
    pub fn load(set: &Arc<ArchiveSet>, language: Option<&str>) -> Self {
        match crate::audio::player_music::local_strings(set, language) {
            Some(strings) => Self::from_strings(&strings),
            None => {
                // chat-window.md §10 item 3: no table can be read.
                tracing::warn!("command words: no language archive; only Tessera's names");
                Self::default()
            }
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn defaults() {
        let t = CommandText::default();
        assert_eq!(t.prefix, "/");
        assert!(t.default_word.is_none() && t.marker.is_none() && t.modes.is_empty());
        // The mode table's fixed rows are distinct tokens.
        let mut tokens: Vec<u32> = MODE_TABLE.iter().map(|r| r.0).collect();
        tokens.sort_unstable();
        tokens.dedup();
        assert_eq!(tokens.len(), MODE_TABLE.len());
    }

    /// A `MakeWriter` that collects formatted log lines.
    #[derive(Clone, Default)]
    struct Captured(Arc<std::sync::Mutex<Vec<u8>>>);

    impl std::io::Write for Captured {
        fn write(&mut self, buf: &[u8]) -> std::io::Result<usize> {
            if let Ok(mut v) = self.0.lock() {
                v.extend_from_slice(buf);
            }
            Ok(buf.len())
        }

        fn flush(&mut self) -> std::io::Result<()> {
            Ok(())
        }
    }

    impl<'a> tracing_subscriber::fmt::MakeWriter<'a> for Captured {
        type Writer = Self;

        fn make_writer(&'a self) -> Self::Writer {
            self.clone()
        }
    }

    /// Finding: chat-window.md §10 item 3 asks for a warning when a command table cannot be
    /// read; it was logged at info.
    #[test]
    fn unreadable_command_tables_log_a_warning() {
        let out = Captured::default();
        let subscriber = tracing_subscriber::fmt()
            .with_writer(out.clone())
            .with_ansi(false)
            .with_max_level(tracing::Level::TRACE)
            .finish();
        let text = tracing::subscriber::with_default(subscriber, || {
            CommandText::from_strings(&Strings::new(tessera_text::MemorySource::new()))
        });
        assert_eq!(text.prefix, "/");
        let log = String::from_utf8(out.0.lock().map(|v| v.clone()).unwrap_or_default())
            .unwrap_or_default();
        let line = log
            .lines()
            .find(|l| l.contains("command words"))
            .unwrap_or_else(|| panic!("no command-words line: {log}"));
        assert!(line.contains("WARN"), "{line}");
        assert!(line.contains("cannot be read"), "{line}");
    }
}
