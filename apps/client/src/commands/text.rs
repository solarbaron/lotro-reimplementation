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
use super::registry::Channel;

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
/// and what its words do in Tessera (`Say` and `Tell` send, the reply and tell-again words
/// need a target Tessera does not have, every other channel is not carried by the Tessera
/// protocol). Neither variant group is read while Q12 (the game-state test that picks one)
/// is open, so `IsCommand` answers false for their words (§10 item 3).
pub const MODE_TABLE: [(u32, u32, ModeKind, Channel); 26] = [
    (0x04DA_4989, 5, ModeKind::Plain, Channel::Say),
    (0x0DA4_AC6C, 6, ModeKind::Plain, Channel::Tell),
    (0x048C_E78C, 6, ModeKind::Reply, Channel::TellReply),
    (0x0A48_C3E9, 6, ModeKind::TellAgain, Channel::TellReply),
    (0x0384_53D4, 11, ModeKind::Plain, Channel::Unavailable),
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
    (0x08B3_A4E1, 28, ModeKind::Plain, Channel::Unavailable),
    (0x08B3_A4E2, 29, ModeKind::Plain, Channel::Unavailable),
    (0x08B3_A4E3, 30, ModeKind::Plain, Channel::Unavailable),
    (0x08B3_A4E4, 31, ModeKind::Plain, Channel::Unavailable),
    (0x08B3_A4E5, 39, ModeKind::Plain, Channel::Unavailable),
    (0x08B3_A4E6, 40, ModeKind::Plain, Channel::Unavailable),
    (0x08B3_A4E7, 41, ModeKind::Plain, Channel::Unavailable),
    (0x08B3_A4E8, 42, ModeKind::Plain, Channel::Unavailable),
    (0x0984_23C4, 38, ModeKind::Plain, Channel::Unavailable),
    (0x06E8_82B4, 44, ModeKind::Plain, Channel::Unavailable),
    (0x0538_82B4, 45, ModeKind::Plain, Channel::Unavailable),
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
        let text = Self {
            prefix: entry(strings, setup, TOKEN_PREFIX)
                .unwrap_or_else(|| DEFAULT_PREFIX.to_owned()),
            default_word: entry(strings, setup, TOKEN_DEFAULT_WORD),
            marker: entry(strings, setup, TOKEN_MARKER),
            unknown_line: entry(strings, setup, TOKEN_UNKNOWN),
            help_names: entry(strings, commands, TOKEN_HELP),
            plugins_names: entry(strings, commands, TOKEN_PLUGINS),
            modes,
        };
        tracing::info!(
            setup = setup.is_some(),
            commands = commands.is_some(),
            prefix_from_install = text.prefix != DEFAULT_PREFIX,
            unknown_line = text.unknown_line.is_some(),
            mode_rows = text.modes.len(),
            "command words read"
        );
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
                tracing::info!("command words: no language archive; Tessera's defaults");
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
}
