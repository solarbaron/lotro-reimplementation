// SPDX-License-Identifier: GPL-3.0-or-later
//! Error and warning types of `tessera-input`.

use std::path::PathBuf;

/// Result alias for this crate.
pub type Result<T, E = InputError> = std::result::Result<T, E>;

/// Everything that can go wrong in `tessera-input`.
#[derive(Debug, thiserror::Error)]
#[non_exhaustive]
pub enum InputError {
    /// A binding string (`"Ctrl+K"`, `"G G"`) could not be parsed.
    #[error("invalid binding {text:?}: {reason}")]
    Binding {
        /// The offending text (truncated to 64 bytes).
        text: String,
        /// What is wrong with it.
        reason: String,
    },
    /// The keymap document is not valid TOML.
    #[error("keymap is not valid TOML: {0}")]
    Toml(String),
    /// The keymap document is valid TOML but breaks the keymap schema
    /// (strict parsing only; lenient parsing reports [`KeymapWarning`]s).
    #[error("keymap: {0}")]
    Keymap(KeymapWarning),
    /// The keymap declares a format version newer than this build reads.
    #[error("keymap version {found} is newer than the supported version {supported}")]
    NewerVersion {
        /// Version in the file.
        found: i64,
        /// Highest version this build understands.
        supported: i64,
    },
    /// A keymap file or document exceeds the size limit.
    #[error("keymap is {size} bytes, the limit is {limit}")]
    TooLarge {
        /// Actual size.
        size: u64,
        /// Limit.
        limit: u64,
    },
    /// An action or context name is not acceptable.
    #[error("invalid name {0:?}: names are 1-64 characters of a-z, 0-9, '_' and '.'")]
    InvalidName(String),
    /// A retired `text_entry` action name (`keymap.md` §6.3): the writer
    /// never writes it, because the strict reader would refuse it and the
    /// lenient one would rename it to its successor.
    #[error("action {action:?} of context {context:?} is retired; use {successor:?}")]
    RetiredAction {
        /// The context (`text_entry`).
        context: String,
        /// The retired action name.
        action: String,
        /// The action that replaced it.
        successor: &'static str,
    },
    /// An action or context with this name is already registered.
    #[error("{kind} {name:?} is already registered")]
    Duplicate {
        /// `"action"` or `"context"`.
        kind: &'static str,
        /// The name.
        name: String,
    },
    /// A context definition is not acceptable (for example `opaque_keys` on
    /// a game-band context, `keymap.md` §3.1).
    #[error("context {name:?}: {reason}")]
    InvalidContext {
        /// The context name.
        name: String,
        /// What is wrong.
        reason: &'static str,
    },
    /// A record of the original client's data (key map, action map) could
    /// not be decoded (`keymap.md` §12.9, §12.10).
    #[error("{what}: {source}")]
    Record {
        /// Which record.
        what: String,
        /// The decoder's error.
        #[source]
        source: tessera_formats::FormatError,
    },
    /// A registry limit was reached.
    #[error("too many {kind} (limit {limit})")]
    TooMany {
        /// What overflowed.
        kind: &'static str,
        /// The limit.
        limit: usize,
    },
    /// An I/O operation on a keymap file failed.
    #[error("{op} {path}: {source}")]
    Io {
        /// What was being done.
        op: &'static str,
        /// The file.
        path: PathBuf,
        /// The cause.
        #[source]
        source: std::io::Error,
    },
    /// Writing the keymap was refused by the read-only install guard, or the
    /// destination could not be checked.
    #[error(transparent)]
    Guard(#[from] tessera_settings::SettingsError),
}

/// A problem found while reading a keymap leniently, or while compiling it
/// against an action registry. Each one names where in the document it is.
#[derive(Debug, Clone, PartialEq, Eq, thiserror::Error)]
#[non_exhaustive]
pub enum KeymapWarning {
    /// A top-level key or section has the wrong type; it was ignored.
    #[error("{key}: {reason}")]
    BadSection {
        /// Key path, e.g. `bindings.movement`.
        key: String,
        /// What is wrong.
        reason: String,
    },
    /// One binding entry could not be used; it was dropped.
    #[error("{context}.{action}[{index}]: {reason}")]
    BadEntry {
        /// Context name.
        context: String,
        /// Action name.
        action: String,
        /// Index in the action's list.
        index: usize,
        /// What is wrong.
        reason: String,
    },
    /// A limit was reached; the rest was dropped.
    #[error("{what}: more than {limit}, the rest was ignored")]
    Truncated {
        /// What was cut.
        what: String,
        /// The limit.
        limit: usize,
    },
    /// A context named in the keymap is not registered (kept in the file,
    /// ignored at run time).
    #[error("unknown context {0:?}")]
    UnknownContext(String),
    /// An action named in the keymap is not registered (kept in the file,
    /// ignored at run time).
    #[error("unknown action {context}.{action}")]
    UnknownAction {
        /// Context name.
        context: String,
        /// Action name.
        action: String,
    },
    /// A binding entry is kept in the keymap but disabled when compiled
    /// against the registry (§5.3: a non-`press` `on` on an axis action, a
    /// multi-stroke binding in a pass-through context).
    #[error("{context}.{action}: {binding:?} disabled: {reason}")]
    Disabled {
        /// Context name.
        context: String,
        /// Action name.
        action: String,
        /// The binding as written.
        binding: String,
        /// Why.
        reason: &'static str,
    },
    /// A retired action name was migrated (§6.3).
    #[error(
        "{context}.{from}: retired action migrated to {to} (old list dropped because {to} is listed: {dropped})"
    )]
    Migrated {
        /// Context name.
        context: String,
        /// Old action name.
        from: String,
        /// New action name.
        to: String,
        /// Whether the old list was dropped because the new name was
        /// already present.
        dropped: bool,
    },
    /// The same stroke is both a complete binding and the first stroke of a
    /// longer sequence in one context; the sequence was disabled.
    #[error("{context}: sequence {sequence:?} starts with {stroke:?}, which is bound on its own")]
    PrefixConflict {
        /// Context name.
        context: String,
        /// The disabled sequence.
        sequence: String,
        /// The shadowing single stroke.
        stroke: String,
    },
}
