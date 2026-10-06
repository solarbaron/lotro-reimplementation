// SPDX-License-Identifier: GPL-3.0-or-later
//! Errors of the scenery module.

use tessera_assets::AssetError;
use tessera_formats::FormatError;

/// What can go wrong while loading the inputs of scenery generation. Generation itself
/// never fails: hostile or missing data only makes it place less.
#[derive(Debug, thiserror::Error)]
pub enum SceneryError {
    /// The region descriptor names no table of this kind, or the region is unknown.
    #[error("region {region} has no {what}")]
    MissingTable {
        /// Region number.
        region: u8,
        /// Which table.
        what: &'static str,
    },
    /// A record named by another record does not exist.
    #[error("{what} {did:#010x} is missing")]
    MissingRecord {
        /// The record's DID.
        did: u32,
        /// What it should have been.
        what: &'static str,
    },
    /// A record exists but does not decode.
    #[error("{what} {did:#010x}: {source}")]
    Format {
        /// The record's DID.
        did: u32,
        /// What it should have been.
        what: &'static str,
        /// The decoder's error.
        #[source]
        source: FormatError,
    },
    /// A 0x47 parent chain is too long or loops.
    #[error("object chain of {did:#010x}: {reason}")]
    Chain {
        /// The object the chain starts at.
        did: u32,
        /// Why it was cut.
        reason: &'static str,
    },
    /// Reading a record failed.
    #[error(transparent)]
    Asset(#[from] AssetError),
}

impl SceneryError {
    /// Whether the failure depends on the data and so recurs on every attempt (a missing
    /// or broken record, a broken chain, an archive read failure), as opposed to the asset
    /// pipeline's state ([`AssetError::is_data_error`] false: full queue, shutdown, panic).
    /// Caches keep only data failures; the others are returned to the caller to retry.
    #[must_use]
    pub fn is_data_error(&self) -> bool {
        match self {
            Self::MissingTable { .. }
            | Self::MissingRecord { .. }
            | Self::Format { .. }
            | Self::Chain { .. } => true,
            Self::Asset(e) => e.is_data_error(),
        }
    }

    pub(crate) fn format(did: impl Into<u32>, what: &'static str, source: FormatError) -> Self {
        Self::Format {
            did: did.into(),
            what,
            source,
        }
    }
}
