// SPDX-License-Identifier: GPL-3.0-or-later
//! Error type of the crate.

use std::borrow::Cow;
use std::sync::Arc;

use tessera_formats::{Did, FormatError};
use tessera_props::PropsError;

/// Result alias using the crate's [`WStateError`].
pub type Result<T, E = WStateError> = std::result::Result<T, E>;

/// Everything that can go wrong while decoding a class library or an object-state record,
/// or while resolving objects through a record source.
///
/// Decoders never panic on malformed input; every failure is reported through this type.
/// The error is cheap to clone, so a failed load can be cached and reported again.
#[derive(Debug, Clone, thiserror::Error)]
#[non_exhaustive]
pub enum WStateError {
    /// A primitive read failed: truncation, an invalid boolean, the memory budget, a
    /// property or StringInfo value that did not decode, ...
    #[error("{what}: {source}")]
    Format {
        /// What was being decoded.
        what: &'static str,
        /// The underlying failure.
        #[source]
        source: FormatError,
    },

    /// A field held a value the specification does not allow.
    #[error("malformed {what} at offset {offset}: {reason}")]
    Malformed {
        /// What was being decoded.
        what: &'static str,
        /// Offset of the offending field (within the record, or within the bundle for
        /// bundle contents).
        offset: usize,
        /// Why it was rejected.
        reason: Cow<'static, str>,
    },

    /// The class library's container version is not one this reader supports
    /// (`wlib.md` §3.1: 30, 36 and 38 are accepted).
    #[error("unsupported class-library version {version}")]
    UnsupportedVersion {
        /// The stored version.
        version: u32,
    },

    /// A chunk the class library cannot be built without is absent (`wlib.md` §10 step 1).
    #[error("class library has no chunk of type {chunk_type}")]
    MissingChunk {
        /// The chunk type (16, 64, 512 or 1024).
        chunk_type: u32,
    },

    /// An item of an object-state bundle starts with an unknown marker (`wstate.md` §5.1).
    #[error("unknown item marker {marker:#010x} at bundle offset {offset}")]
    BadMarker {
        /// The marker found.
        marker: u32,
        /// Offset of the marker within the bundle.
        offset: usize,
    },

    /// A reference value names a label that does not exist in its bundle
    /// (`wstate.md` §8.1).
    #[error("reference to absent label {label} in item {item}")]
    DanglingReference {
        /// The label referenced.
        label: u32,
        /// Index of the item holding the reference.
        item: usize,
    },

    /// Bytes were left over after a structure that must be consumed exactly.
    #[error("{count} trailing bytes after {what} at offset {offset}")]
    TrailingBytes {
        /// What was being decoded.
        what: &'static str,
        /// Offset of the first leftover byte.
        offset: usize,
        /// Number of leftover bytes.
        count: usize,
    },

    /// The record's own DID differs from the DID that was looked up.
    #[error("record DID {found} does not match expected {expected}")]
    DidMismatch {
        /// The DID looked up.
        expected: Did,
        /// The DID stored in the record.
        found: Did,
    },

    /// The DID is not an object-state DID (prefix 0x70–0x77).
    #[error("{did} is not an object-state DID")]
    NotObjectDid {
        /// The DID given.
        did: Did,
    },

    /// A record that the operation needs is absent from the source.
    #[error("required {what} record {did} is missing")]
    Missing {
        /// The DID looked up.
        did: Did,
        /// Record kind, for messages.
        what: &'static str,
    },

    /// A record exists but is not of the kind the operation expected (for example an area
    /// DID whose property set lacks the area properties).
    #[error("{did} is not a {expected} record")]
    WrongKind {
        /// The DID given.
        did: Did,
        /// The expected kind.
        expected: &'static str,
    },

    /// An object's root attribute that the operation needs is missing or does not lead to
    /// the expected item (for example a dungeon's position attribute, `wstate.md` §11.4).
    #[error("{did}: root attribute {hash:#010x} {problem}")]
    BadAttribute {
        /// The object.
        did: Did,
        /// Name hash of the attribute.
        hash: u32,
        /// What is wrong.
        problem: &'static str,
    },

    /// Reading or decoding through the property layer failed.
    #[error(transparent)]
    Props(#[from] PropsError),

    /// Decoding record `did` failed.
    #[error("record {did}: {source}")]
    InRecord {
        /// The record that failed.
        did: Did,
        /// The underlying failure.
        #[source]
        source: Arc<WStateError>,
    },
}

impl WStateError {
    pub(crate) fn malformed(
        what: &'static str,
        offset: usize,
        reason: impl Into<Cow<'static, str>>,
    ) -> Self {
        Self::Malformed {
            what,
            offset,
            reason: reason.into(),
        }
    }

    /// Wraps the error with the record it came from (unless it already carries one).
    pub(crate) fn in_record(self, did: Did) -> Self {
        match self {
            e @ (Self::InRecord { .. } | Self::Props(_)) => e,
            e => Self::InRecord {
                did,
                source: Arc::new(e),
            },
        }
    }

    /// The innermost error, looking through [`WStateError::InRecord`] wrappers.
    #[must_use]
    pub fn root_cause(&self) -> &WStateError {
        let mut e = self;
        while let Self::InRecord { source, .. } = e {
            e = source;
        }
        e
    }
}

/// Converts a [`FormatError`] with a description of what was being read.
pub(crate) trait FormatContext<T> {
    fn ctx(self, what: &'static str) -> Result<T>;
}

impl<T> FormatContext<T> for std::result::Result<T, FormatError> {
    fn ctx(self, what: &'static str) -> Result<T> {
        self.map_err(|source| WStateError::Format { what, source })
    }
}
