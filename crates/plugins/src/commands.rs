// SPDX-License-Identifier: GPL-3.0-or-later
//! A command registry for hosts without a command dispatcher of their own (tools, tests,
//! fuzzing): [`CommandTable`].
//!
//! The plugin host keeps no command table (`chat-window.md` §10 item 3, decision 0020
//! item 6): `Turbine.Shell.AddCommand`, `RemoveCommand`, `IsCommand` and `GetCommands` reach
//! the embedding host's registry through [`HostApi`](crate::HostApi)'s command methods,
//! with the calling apartment as the owner. The client implements them on its dispatcher's
//! registry. A host that has no registry can embed a [`CommandTable`] and forward the
//! methods to it; it follows the same rules (`chat-window.md` §7.1, §8.2): names stored as
//! given, matched case-insensitively, the first registration wins, and the plugin-manager
//! name `plugins` reserved for the host (`lua-plugin-api.md` §6.4).

use std::collections::BTreeMap;
use std::sync::{Mutex, PoisonError};

/// Most names a [`CommandTable`] holds, all apartments and the host together (a bound
/// against runaway plugins; above the per-apartment cap of `AddCommand`, so that one
/// apartment meets its own cap first).
pub const MAX_TABLE_NAMES: usize = 16_384;

/// Longest name a [`CommandTable`] keeps, bytes.
pub const MAX_TABLE_NAME_LEN: usize = 256;

/// The names a [`CommandTable`] reserves for its host: the plugin-manager command.
pub const HOST_COMMAND_NAMES: [&str; 1] = ["plugins"];

/// Who holds a name of a [`CommandTable`].
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum CommandOwner {
    /// The host (its built-in commands).
    Host,
    /// A plugin apartment's command object: (apartment, handler).
    Plugin(String, u64),
}

#[derive(Debug, Clone)]
struct TableEntry {
    name: String,
    owner: CommandOwner,
}

/// A host-side command registry (module docs). Thread-safe; every method takes `&self`.
#[derive(Debug)]
pub struct CommandTable {
    entries: Mutex<BTreeMap<String, TableEntry>>,
}

impl Default for CommandTable {
    fn default() -> Self {
        Self::new()
    }
}

/// The lookup key of a name: Unicode lower case, no locale (`chat-window.md` §10 item 12).
fn key(name: &str) -> String {
    name.to_lowercase()
}

impl CommandTable {
    /// A table holding only the host names of [`HOST_COMMAND_NAMES`].
    #[must_use]
    pub fn new() -> Self {
        let t = Self {
            entries: Mutex::new(BTreeMap::new()),
        };
        for n in HOST_COMMAND_NAMES {
            t.register_host(n);
        }
        t
    }

    fn entries(&self) -> std::sync::MutexGuard<'_, BTreeMap<String, TableEntry>> {
        self.entries.lock().unwrap_or_else(PoisonError::into_inner)
    }

    fn insert(&self, name: &str, owner: CommandOwner) -> bool {
        if name.is_empty() || name.len() > MAX_TABLE_NAME_LEN {
            return false;
        }
        let mut e = self.entries();
        if e.len() >= MAX_TABLE_NAMES {
            return false;
        }
        let k = key(name);
        if e.contains_key(&k) {
            return false;
        }
        e.insert(
            k,
            TableEntry {
                name: name.to_owned(),
                owner,
            },
        );
        true
    }

    /// Registers a host command name; `false` when the name is taken or refused.
    pub fn register_host(&self, name: &str) -> bool {
        self.insert(name, CommandOwner::Host)
    }

    /// [`HostApi::register_command`](crate::HostApi::register_command).
    pub fn register(&self, apartment: &str, name: &str, handler: u64) -> bool {
        self.insert(name, CommandOwner::Plugin(apartment.to_owned(), handler))
    }

    /// [`HostApi::unregister_command`](crate::HostApi::unregister_command).
    pub fn unregister(&self, apartment: &str, handler: u64) -> usize {
        let mut e = self.entries();
        let before = e.len();
        e.retain(|_, t| !matches!(&t.owner, CommandOwner::Plugin(a, h) if a == apartment && *h == handler));
        before - e.len()
    }

    /// [`HostApi::unregister_apartment_commands`](crate::HostApi::unregister_apartment_commands).
    pub fn unregister_apartment(&self, apartment: &str) -> usize {
        let mut e = self.entries();
        let before = e.len();
        e.retain(|_, t| !matches!(&t.owner, CommandOwner::Plugin(a, _) if a == apartment));
        before - e.len()
    }

    /// [`HostApi::is_command`](crate::HostApi::is_command): the name exactly as given,
    /// case-insensitive; empty is never a command.
    #[must_use]
    pub fn is_command(&self, name: &str) -> bool {
        !name.is_empty() && self.entries().contains_key(&key(name))
    }

    /// [`HostApi::command_names`](crate::HostApi::command_names): every name as
    /// registered, ordered by the lower-cased name.
    #[must_use]
    pub fn names(&self) -> Vec<String> {
        self.entries().values().map(|t| t.name.clone()).collect()
    }

    /// Who holds a command word (case-insensitive).
    #[must_use]
    pub fn owner(&self, word: &str) -> Option<CommandOwner> {
        if word.is_empty() {
            return None;
        }
        self.entries().get(&key(word)).map(|t| t.owner.clone())
    }

    /// [`HostApi::command_target`](crate::HostApi::command_target): the apartment and
    /// handler of a plugin command word.
    #[must_use]
    pub fn target(&self, word: &str) -> Option<(String, u64)> {
        match self.owner(word)? {
            CommandOwner::Plugin(a, h) => Some((a, h)),
            CommandOwner::Host => None,
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn first_registration_wins_and_case_folds() {
        let t = CommandTable::new();
        assert!(t.is_command("PLUGINS"));
        assert!(!t.register("a", "Plugins", 1));
        assert!(t.register("a", "Greet", 1));
        assert!(!t.register("b", "greet", 2));
        assert!(t.register("a", "hi", 1));
        assert!(t.register("a", "other", 2));
        assert!(t.is_command("GREET"));
        assert!(!t.is_command(" greet"), "exactly as given");
        assert!(!t.is_command("/greet"), "no prefix removal");
        assert!(!t.is_command(""));
        assert_eq!(t.target("HI"), Some(("a".to_owned(), 1)));
        assert_eq!(t.target("plugins"), None);
        assert_eq!(t.names(), ["Greet", "hi", "other", "plugins"]);
        assert_eq!(t.unregister("a", 1), 2);
        assert_eq!(t.unregister("a", 1), 0);
        assert!(t.register("b", "greet", 2));
        assert_eq!(t.unregister_apartment("b"), 1);
        assert_eq!(t.unregister_apartment("a"), 1);
        assert_eq!(t.names(), ["plugins"]);
    }

    #[test]
    fn bounds() {
        let t = CommandTable::new();
        assert!(!t.register("a", &"x".repeat(MAX_TABLE_NAME_LEN + 1), 1));
        for i in 0..MAX_TABLE_NAMES {
            let _ = t.register("a", &format!("c{i}"), 1);
        }
        assert_eq!(t.names().len(), MAX_TABLE_NAMES);
        assert!(!t.register("a", "one-more", 1));
    }
}
