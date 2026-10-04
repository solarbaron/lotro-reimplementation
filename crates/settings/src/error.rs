// SPDX-License-Identifier: GPL-3.0-or-later
//! Error and warning types.

use std::io;
use std::path::PathBuf;

/// Errors returned by the strict API ([`crate::Settings::from_toml_str`],
/// [`crate::SettingsStore::save`], ...).
///
/// The tolerant [`crate::SettingsStore::load`] never returns an error; it
/// reports problems as [`LoadWarning`]s instead.
#[derive(Debug, thiserror::Error)]
#[non_exhaustive]
pub enum SettingsError {
    /// The platform has no per-user configuration directory (for example
    /// `$HOME` is unset) and no override was given.
    #[error("no per-user configuration directory is available on this platform")]
    NoConfigDir,

    /// A filesystem operation failed.
    #[error("{op} {}: {source}", path.display())]
    Io {
        /// What was being done (`"read"`, `"write"`, `"rename"`, ...).
        op: &'static str,
        /// The path involved.
        path: PathBuf,
        /// The underlying error.
        #[source]
        source: io::Error,
    },

    /// The file (or the serialized document) exceeds [`crate::MAX_FILE_SIZE`].
    #[error("settings document is {size} bytes, above the {limit}-byte limit")]
    TooLarge {
        /// Observed size in bytes (a lower bound when reading stopped early).
        size: u64,
        /// The limit in bytes.
        limit: u64,
    },

    /// The file is not valid UTF-8.
    #[error("settings file is not valid UTF-8")]
    NotUtf8,

    /// The document is not valid TOML, or a section has the wrong shape.
    #[error("invalid settings document: {0}")]
    Parse(String),

    /// Serializing the settings to TOML failed.
    #[error("cannot serialize settings: {0}")]
    Serialize(String),

    /// `schema_version` is present but is not a positive integer.
    #[error("invalid schema_version: {0}")]
    BadSchemaVersion(String),

    /// The document was written by a newer Tessera with a schema this build
    /// does not know (strict parsing only).
    #[error("settings schema version {found} is newer than supported version {supported}")]
    NewerSchema {
        /// Version found in the document.
        found: u32,
        /// [`crate::SCHEMA_VERSION`].
        supported: u32,
    },

    /// A schema migration step failed or no step exists for a version.
    #[error("cannot migrate settings from schema version {from}: {message}")]
    Migration {
        /// Version the failing step started from.
        from: u32,
        /// Human-readable reason.
        message: String,
    },

    /// Refused to write the settings file because its path lies inside one of
    /// the game directories recorded in the settings. The game install is
    /// strictly read-only.
    #[error(
        "refusing to write settings to {} because it is inside game directory {}",
        path.display(),
        game_dir.display()
    )]
    InsideGameDir {
        /// The settings file path.
        path: PathBuf,
        /// The game directory that contains it.
        game_dir: PathBuf,
    },

    /// Refused to replace an existing file at the settings path because it
    /// cannot be a settings file Tessera wrote (it is not a regular file, or
    /// it is larger than [`crate::MAX_FILE_SIZE`]). Move or delete it by
    /// hand.
    #[error("refusing to replace {}: {reason}; move or delete it", path.display())]
    WontReplace {
        /// The path that holds the foreign file.
        path: PathBuf,
        /// Why it was not replaced.
        reason: String,
    },
}

/// Result alias for this crate.
pub type Result<T, E = SettingsError> = std::result::Result<T, E>;

/// A problem found by the tolerant loader. Every warning is also logged with
/// `tracing::warn!`. At most [`crate::MAX_WARNINGS_PER_SECTION`] detailed
/// warnings are produced per section; the rest are summarized by one
/// [`LoadWarning::Suppressed`].
#[derive(Debug, Clone, PartialEq, Eq, thiserror::Error)]
#[non_exhaustive]
pub enum LoadWarning {
    /// The file exists but could not be read; defaults were used.
    #[error("cannot read settings file: {message}")]
    Unreadable {
        /// The I/O error text.
        message: String,
    },
    /// The file exceeds [`crate::MAX_FILE_SIZE`]; defaults were used.
    #[error("settings file is larger than {limit} bytes; using defaults")]
    TooLarge {
        /// The limit in bytes.
        limit: u64,
    },
    /// The file is not UTF-8; defaults were used.
    #[error("settings file is not valid UTF-8; using defaults")]
    NotUtf8,
    /// The file is not valid TOML; defaults were used.
    #[error("settings file is not valid TOML ({message}); using defaults")]
    Syntax {
        /// Parser message.
        message: String,
    },
    /// `schema_version` is malformed; the current version was assumed.
    #[error("invalid schema_version ({message}); assuming version {assumed}")]
    BadSchemaVersion {
        /// What was wrong.
        message: String,
        /// The version assumed instead.
        assumed: u32,
    },
    /// The file comes from a newer Tessera. Known sections were loaded;
    /// unknown top-level keys are kept, but saving writes the current version.
    #[error("settings file has schema version {found}, newer than supported {supported}")]
    NewerSchema {
        /// Version found.
        found: u32,
        /// [`crate::SCHEMA_VERSION`].
        supported: u32,
    },
    /// Migration from an older schema failed; defaults were used.
    #[error("cannot migrate settings from schema version {from}: {message}; using defaults")]
    MigrationFailed {
        /// Version the failing step started from.
        from: u32,
        /// Reason.
        message: String,
    },
    /// One section was not a table and was reset to its defaults.
    #[error("settings section [{section}] is invalid ({message}); reset to defaults")]
    Section {
        /// Section name.
        section: String,
        /// Deserializer message.
        message: String,
    },
    /// One value or list entry inside a section was invalid. A bad value is
    /// reset to its default, a bad list entry (or a bookmark without a valid
    /// DID, or a window entry with an invalid key) is dropped; the rest of
    /// the section is kept.
    #[error("invalid entry in settings section [{section}] ({message}); ignored")]
    Entry {
        /// Section name (`game`, `explorer` or `windows`).
        section: String,
        /// What was wrong, including the key path (for example
        /// `bookmarks[3].did: ...`).
        message: String,
    },
    /// More problems were found in a section than are reported one by one.
    #[error("{count} more problems in settings section [{section}] not listed")]
    Suppressed {
        /// Section name.
        section: String,
        /// How many problems were not reported individually.
        count: usize,
    },
    /// The backup copy of a damaged or migrated file could not be written,
    /// or was deliberately skipped because it would land inside a game
    /// install.
    #[error("cannot back up settings file: {message}")]
    BackupFailed {
        /// The I/O error text.
        message: String,
    },
}
