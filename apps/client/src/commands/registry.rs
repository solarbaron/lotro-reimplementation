// SPDX-License-Identifier: GPL-3.0-or-later
//! The command registry (`chat-window.md` §7.1, §8.2; decision 0020 item 6): one table of
//! names without the prefix, matched case-insensitively, each with an owner, a handler and
//! its help. Built-in and plugin commands share it; the first registration of a name wins.

use std::collections::BTreeMap;

use crate::audio::player_music::MusicWord;

use super::line::is_whitespace;

/// Most names the registry holds (plugins register at run time; a bound keeps a runaway
/// plugin from growing it without limit).
pub const MAX_NAMES: usize = 4096;
/// Longest name kept, bytes (longer pieces are skipped).
pub const MAX_NAME_LEN: usize = 256;

/// Who registered a name (decision 0020 item 6).
#[derive(Debug, Clone, PartialEq, Eq, Hash, PartialOrd, Ord)]
pub enum Owner {
    /// The client's built-ins.
    Client,
    /// A plugin apartment (by its key).
    Plugin(String),
}

/// A channel command's channel (decision 0020 item 2).
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, PartialOrd, Ord)]
pub enum Channel {
    /// Protocol channel 1.
    Say,
    /// Protocol channel 2.
    Yell,
    /// Protocol channel 3.
    Emote,
    /// Protocol channel 4: `tell <name> <text>`.
    Tell,
    /// A `Tell` word whose target comes from a game callback (the reply and tell-again
    /// words, `chat-window.md` §6.5): Tessera has no such target.
    TellReply,
    /// A channel the Tessera protocol does not carry (fellowship, raid, OOC, ...).
    Unavailable,
}

/// A client built-in command.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, PartialOrd, Ord)]
pub enum Builtin {
    /// A player-music command.
    Music(MusicWord),
    /// `help`.
    Help,
    /// `plugins` (`lua-plugin-api.md` §6.4).
    Plugins,
    /// A channel command.
    Channel(Channel),
    /// A P5 built-in (`ride`, `findplace`, `leaveinstance`; [`super::p5`]).
    P5(super::p5::P5Word),
}

/// What runs a command.
#[derive(Debug, Clone, PartialEq, Eq, Hash, PartialOrd, Ord)]
pub enum Handler {
    /// A client built-in.
    Builtin(Builtin),
    /// A plugin's command object, by a key its apartment gives (M5).
    Plugin(u64),
}

/// A command's help (decision 0020 item 5).
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum Help {
    /// Tessera's own wording: one line, and usage plus a sentence.
    Text {
        /// One line.
        short: String,
        /// Usage plus a sentence.
        long: String,
    },
    /// Asked of the plugin when `/help` runs (`GetShortHelp`, `GetHelp`,
    /// `lua-plugin-api.md` §11.2).
    Plugin,
}

/// One registered name.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Entry {
    /// The name as registered (original case, no prefix).
    pub name: String,
    /// Who registered it.
    pub owner: Owner,
    /// What runs it.
    pub handler: Handler,
    /// Its help.
    pub help: Help,
    /// Whether lines that run it may use `;` substitutions (`chat-window.md` §7.1, §7.5;
    /// no substitution words exist yet).
    pub substitutes: bool,
}

/// The registry (module docs).
#[derive(Debug, Clone, Default)]
pub struct Registry {
    /// By the lower-cased name.
    entries: BTreeMap<String, Entry>,
}

/// The lookup key of a name: its lower-case form (Unicode, `chat-window.md` §8.2).
#[must_use]
pub fn key(name: &str) -> String {
    name.to_lowercase()
}

/// Splits a name list (`chat-window.md` §7.1): at every `;`, `:` and `,`, each piece
/// trimmed of §7.2 whitespace, empty pieces ignored.
pub fn split_names(names: &str) -> impl Iterator<Item = &str> {
    names
        .split([';', ':', ','])
        .map(|p| p.trim_matches(is_whitespace))
        .filter(|p| !p.is_empty())
}

impl Registry {
    /// An empty registry.
    #[must_use]
    pub fn new() -> Self {
        Self::default()
    }

    /// Registers every name of `names` (split by [`split_names`]) for `owner` and
    /// `handler`. A name the registry already holds is skipped and not counted; so are
    /// names longer than [`MAX_NAME_LEN`] and names beyond [`MAX_NAMES`]. Returns how
    /// many names were added.
    pub fn register(
        &mut self,
        names: &str,
        owner: &Owner,
        handler: &Handler,
        help: &Help,
    ) -> usize {
        let substitutes = matches!(owner, Owner::Plugin(_));
        let mut added = 0;
        for name in split_names(names) {
            if name.len() > MAX_NAME_LEN || self.entries.len() >= MAX_NAMES {
                tracing::debug!(name, "command name not registered (bounds)");
                continue;
            }
            let k = key(name);
            if self.entries.contains_key(&k) {
                continue;
            }
            self.entries.insert(
                k,
                Entry {
                    name: name.to_owned(),
                    owner: owner.clone(),
                    handler: handler.clone(),
                    help: help.clone(),
                    substitutes,
                },
            );
            added += 1;
        }
        added
    }

    /// Removes every name `owner` registered for `handler`; returns how many.
    pub fn unregister(&mut self, owner: &Owner, handler: &Handler) -> usize {
        let before = self.entries.len();
        self.entries
            .retain(|_, e| !(e.owner == *owner && e.handler == *handler));
        before - self.entries.len()
    }

    /// Removes every name of `owner` (apartment unload, `lua-plugin-api.md` §6.3);
    /// returns how many.
    pub fn unregister_owner(&mut self, owner: &Owner) -> usize {
        let before = self.entries.len();
        self.entries.retain(|_, e| e.owner != *owner);
        before - self.entries.len()
    }

    /// `Shell.IsCommand` (`chat-window.md` §8.2): the name exactly as given (no trimming,
    /// no prefix removal, no splitting), compared case-insensitively; empty is never a
    /// command.
    #[must_use]
    pub fn is_command(&self, name: &str) -> bool {
        !name.is_empty() && self.entries.contains_key(&key(name))
    }

    /// The entry of a command word (case-insensitive).
    #[must_use]
    pub fn lookup(&self, word: &str) -> Option<&Entry> {
        if word.is_empty() {
            return None;
        }
        self.entries.get(&key(word))
    }

    /// Every entry, ordered by the lower-cased name.
    pub fn entries(&self) -> impl Iterator<Item = &Entry> {
        self.entries.values()
    }

    /// How many names are registered.
    #[must_use]
    pub fn len(&self) -> usize {
        self.entries.len()
    }

    /// Whether nothing is registered.
    #[must_use]
    pub fn is_empty(&self) -> bool {
        self.entries.is_empty()
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn help() -> Help {
        Help::Text {
            short: "s".into(),
            long: "l".into(),
        }
    }

    #[test]
    fn worked_example_is_command() {
        // chat-window.md §11.5.
        let mut r = Registry::new();
        let foo = Handler::Plugin(1);
        let p = Owner::Plugin("ap".into());
        assert_eq!(r.register("foo", &p, &foo, &Help::Plugin), 1);
        assert_eq!(
            r.register(
                "abc; ab",
                &Owner::Client,
                &Handler::Builtin(Builtin::Help),
                &help()
            ),
            2
        );
        assert!(r.is_command("FOO"));
        assert!(!r.is_command("/foo"));
        assert!(r.is_command("ab"));
        assert!(!r.is_command(" foo"));
        assert!(!r.is_command(""));
        // Case-insensitive beyond ASCII.
        r.register("Ärger", &p, &foo, &Help::Plugin);
        assert!(r.is_command("äRGER"));
    }

    #[test]
    fn first_registration_wins_and_splitting() {
        let mut r = Registry::new();
        let play = Handler::Builtin(Builtin::Music(MusicWord::Play));
        assert_eq!(r.register("play", &Owner::Client, &play, &help()), 1);
        let p = Owner::Plugin("x".into());
        // A plugin cannot take a built-in name, in any letter case.
        assert_eq!(
            r.register("PLAY, mine", &p, &Handler::Plugin(7), &Help::Plugin),
            1
        );
        assert_eq!(r.lookup("Play").map(|e| &e.owner), Some(&Owner::Client));
        assert_eq!(
            r.register(" a ;; b :\tc ,", &p, &Handler::Plugin(8), &Help::Plugin),
            3
        );
        assert!(r.is_command("c"));
        assert_eq!(r.register("", &p, &Handler::Plugin(9), &Help::Plugin), 0);
        assert_eq!(r.unregister(&p, &Handler::Plugin(8)), 3);
        assert!(!r.is_command("a"));
        assert_eq!(r.unregister_owner(&p), 1);
        assert_eq!(r.len(), 1);
        assert!(r.lookup("play").is_some_and(|e| !e.substitutes));
    }

    #[test]
    fn bounded() {
        let mut r = Registry::new();
        let p = Owner::Plugin("x".into());
        assert_eq!(
            r.register(
                &"n".repeat(MAX_NAME_LEN + 1),
                &p,
                &Handler::Plugin(1),
                &Help::Plugin
            ),
            0
        );
        let many: Vec<String> = (0..MAX_NAMES + 5).map(|i| format!("c{i}")).collect();
        let added = r.register(&many.join(";"), &p, &Handler::Plugin(1), &Help::Plugin);
        assert_eq!(added, MAX_NAMES);
        assert_eq!(r.len(), MAX_NAMES);
    }
}
