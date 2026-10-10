// SPDX-License-Identifier: GPL-3.0-or-later
//! The crate's error type.

use tessera_formats::Did;

/// Result alias of this crate.
pub type Result<T, E = GameDataError> = std::result::Result<T, E>;

/// Errors of `tessera-gamedata`.
///
/// Only failures of the install as a whole are errors: a record that cannot be decoded is
/// listed in the record-family index with a categorised reason
/// ([`crate::RecordFailure`]) and is never a candidate (`scenario-feed.md` §8.6).
#[derive(Debug, Clone, thiserror::Error)]
#[non_exhaustive]
pub enum GameDataError {
    /// The property layer failed (opening the install, reading the dictionary, a DID map
    /// or an enum mapper).
    #[error(transparent)]
    Props(#[from] tessera_props::PropsError),

    /// The object layer failed (the class library, the player-character records).
    #[error(transparent)]
    WState(#[from] tessera_wstate::WStateError),

    /// The install's object-record directory could not be enumerated.
    #[error("enumerating the object records failed: {0}")]
    Enumerate(String),

    /// More object records than [`crate::MAX_OBJECT_RECORDS`] (or the lower
    /// [`crate::IndexOptions::max_records`]): the index refuses to grow without bound.
    #[error("{found} object records exceed the index limit of {limit}")]
    TooManyRecords {
        /// Records listed.
        found: usize,
        /// The limit.
        limit: usize,
    },

    /// Resolution would hold more records than the total limit of `scenario-feed.md`
    /// §8.6 item 4 (1,048,576 over all bindings).
    #[error("selector {index} would raise the resolved total above {limit}")]
    ResolveLimit {
        /// Index of the offending selector in declaration order.
        index: usize,
        /// The limit.
        limit: usize,
    },

    /// The index build panicked, on a worker thread or on the calling thread (a bug, never
    /// hostile data).
    #[error("an index worker thread failed")]
    Worker,

    /// A candidate call reported the install unreadable ([`crate::CandidateStatus::Failed`],
    /// `scenario-feed.md` §8.6 step 1); its reason is repeated here.
    #[error("the record index is unavailable: {0}")]
    IndexUnavailable(String),

    /// A record exists but its view rejected it (a required property absent or
    /// mistyped, or the record did not decode).
    #[error("record {did} cannot be read as asked: {reason}")]
    View {
        /// The record.
        did: Did,
        /// Why.
        reason: String,
    },

    /// A record that the operation needs is not an object record.
    #[error("{0} is not an object record")]
    NotObject(Did),

    /// A record or element the operation needs is not in the install (named by its
    /// WEENIECONTENT, DID-map or element label).
    #[error("the install has no {0}")]
    MissingRecord(String),
}
