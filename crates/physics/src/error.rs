// SPDX-License-Identifier: GPL-3.0-or-later
//! Error type of `tessera-physics`.

/// Result alias used throughout the crate.
pub type Result<T, E = PhysicsError> = std::result::Result<T, E>;

/// Everything that can go wrong in this crate.
#[derive(Debug, Clone, PartialEq, Eq, thiserror::Error)]
#[non_exhaustive]
pub enum PhysicsError {
    /// Input geometry or parameters that the crate refuses (hostile or inconsistent data).
    #[error("invalid physics input: {0}")]
    InvalidInput(&'static str),
    /// A count above one of the crate's limits (bounded allocation for hostile input).
    #[error("{what}: {count} exceeds the limit of {limit}")]
    TooLarge {
        /// What was counted.
        what: &'static str,
        /// The count asked for.
        count: usize,
        /// The limit.
        limit: usize,
    },
    /// A record on the way to a collision shape could not be read or decoded.
    #[error("record {did:#010x} ({what}): {message}")]
    Record {
        /// The record's DID.
        did: u32,
        /// What the record was needed for.
        what: &'static str,
        /// The reader's or decoder's message.
        message: String,
    },
    /// A property set handed in as the player movement record (`collision.md` §10.3) has no
    /// `PlayerPhysicsControl_BasePlayerParameters` struct.
    #[error("not the player movement record (no BasePlayer parameters)")]
    NotMovementRecord,
}
