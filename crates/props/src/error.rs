// SPDX-License-Identifier: GPL-3.0-or-later
//! Error type of the crate.

use std::sync::Arc;

use tessera_formats::{Did, FormatError, PropertyType};

/// Result alias using the crate's [`PropsError`].
pub type Result<T, E = PropsError> = std::result::Result<T, E>;

/// Boxed error returned by a [`crate::RecordSource`].
pub type SourceError = Box<dyn std::error::Error + Send + Sync + 'static>;

/// Everything that can go wrong while loading or resolving properties.
///
/// The error is cheap to clone (sources are shared behind [`Arc`]), so a failed lookup can
/// be cached and reported again without repeating the work.
#[derive(Debug, Clone, thiserror::Error)]
#[non_exhaustive]
pub enum PropsError {
    /// Opening the archive set of an install failed.
    #[error("opening archives in {path} failed: {source}")]
    Open {
        /// The install directory.
        path: std::path::PathBuf,
        /// Underlying error.
        #[source]
        source: Arc<tessera_dat::Error>,
    },

    /// The record source failed to read a record (I/O error, corrupt archive, ...).
    #[error("reading record {did} failed: {source}")]
    Source {
        /// The DID being read.
        did: Did,
        /// Underlying error.
        #[source]
        source: Arc<dyn std::error::Error + Send + Sync + 'static>,
    },

    /// A record was read but did not decode.
    #[error("decoding {what} {did} failed: {source}")]
    Decode {
        /// The DID of the record.
        did: Did,
        /// Record kind, for messages.
        what: &'static str,
        /// Underlying error.
        #[source]
        source: Arc<FormatError>,
    },

    /// A record the operation cannot do without is absent.
    #[error("required {what} record {did} is missing")]
    Missing {
        /// The DID looked up.
        did: Did,
        /// Record kind, for messages.
        what: &'static str,
    },

    /// A raw value of a type this crate does not model (a newer `tessera-formats` value
    /// variant).
    #[error("property {pid:#x}: value of type {ty:?} is not supported")]
    UnsupportedValue {
        /// The property ID (0 when not known).
        pid: u32,
        /// The value's type.
        ty: PropertyType,
    },

    /// The DID is neither a game object (0x70–0x77) nor a property set (0x78–0x7F).
    #[error("DID {did} has no property set")]
    NotPropertySource {
        /// The DID given.
        did: Did,
    },
}

impl PropsError {
    pub(crate) fn decode(did: Did, what: &'static str, source: FormatError) -> Self {
        Self::Decode {
            did,
            what,
            source: Arc::new(source),
        }
    }

    pub(crate) fn source(did: Did, source: SourceError) -> Self {
        Self::Source {
            did,
            source: Arc::from(source),
        }
    }
}
