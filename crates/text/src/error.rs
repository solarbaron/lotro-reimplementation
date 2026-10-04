// SPDX-License-Identifier: GPL-3.0-or-later
//! Error type of `tessera-text`.
//!
//! Rendering itself never fails (`docs/specs/string-table.md` §7): malformed grammar falls
//! back to literal text or an absent value. Errors only come from loading data: opening the
//! language archive, reading a record, or decoding a record.

use std::path::PathBuf;

/// Result alias for this crate.
pub type Result<T, E = TextError> = std::result::Result<T, E>;

/// Errors raised while locating, reading or decoding string data.
#[derive(Debug, thiserror::Error)]
#[non_exhaustive]
pub enum TextError {
    /// The archive container layer failed (I/O, corrupt directory, ...).
    #[error("archive error: {0}")]
    Dat(#[from] tessera_dat::Error),

    /// A record was read but does not decode.
    #[error("record {did:#010x} does not decode: {source}")]
    Decode {
        /// The DID that was read.
        did: u32,
        /// The decoder's error.
        #[source]
        source: tessera_formats::FormatError,
    },

    /// Listing an install directory failed.
    #[error("I/O error on {path}: {source}")]
    Io {
        /// The directory or file concerned.
        path: PathBuf,
        /// The underlying error.
        #[source]
        source: std::io::Error,
    },

    /// The install directory holds no language archive (`client_local_<Lang>.dat`).
    #[error("no language archive (client_local_<Lang>.dat) in {0}")]
    NoLocale(PathBuf),

    /// Loading this record failed earlier and the failure is cached (see
    /// [`crate::TableCache`]); [`crate::Strings::clear_cache`] allows a retry.
    #[error("record {did:#010x} failed to load earlier: {reason}")]
    CachedFailure {
        /// The DID that failed.
        did: u32,
        /// The original error, as text.
        reason: String,
    },

    /// A DID map needed to resolve named string tables is missing or has no such label.
    #[error("named string tables unavailable: {0}")]
    NamedTables(String),
}
