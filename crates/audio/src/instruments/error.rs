// SPDX-License-Identifier: GPL-3.0-or-later
//! Errors of the instrument module.

use tessera_formats::FormatError;

use super::CharacterId;

/// Errors of [`super`]: building the instrument table and queueing music.
#[derive(Debug, thiserror::Error)]
#[non_exhaustive]
pub enum InstrumentError {
    /// Reading a record failed (I/O, decompression, archive corruption). The fetch
    /// function's error is kept as text so that the type is not generic.
    #[error("reading record {did:#010x} failed: {message}")]
    Fetch {
        /// The record that could not be read.
        did: u32,
        /// The fetch function's error message.
        message: String,
    },
    /// A record is missing or does not decode.
    #[error(transparent)]
    Format(#[from] FormatError),
    /// A property name or enum label the music module needs is not in the data
    /// (`instruments.md` §3).
    #[error("{what} `{name}` not found in the data")]
    UnknownName {
        /// What was looked up (property, enum label, …).
        what: &'static str,
        /// The name.
        name: &'static str,
    },
    /// No table of the avatar's closure defines a music script (`instruments.md` §4.1).
    #[error("no script table in the closure defines music script {key:#010x}")]
    ScriptNotFound {
        /// The script key.
        key: u32,
    },
    /// A bound of [`super::TableLimits`] or of the player was reached.
    #[error("limit reached: {0}")]
    Limit(&'static str),
    /// The playback queue is full ([`super::PlayerConfig::max_queued_events`]).
    #[error("the playback queue is full ({queued} events, {adding} more requested)")]
    QueueFull {
        /// Events already queued (not counting those the request would replace).
        queued: usize,
        /// Events that were to be added.
        adding: usize,
    },
    /// The character's share of the queue is used up
    /// ([`super::PlayerConfig::max_events_per_character`]).
    #[error("{character} has too many queued events ({queued} queued, {adding} more requested)")]
    CharacterQueueFull {
        /// The character.
        character: CharacterId,
        /// Its events already queued (not counting those the request would replace).
        queued: usize,
        /// Events that were to be added.
        adding: usize,
    },
    /// A time that must be finite is not (`instruments.md` §8.8.3: the start time of a
    /// synchronised start).
    #[error("invalid time: {0}")]
    InvalidTime(&'static str),
    /// The character is not known to the player ([`super::MusicPlayer::add_character`]).
    #[error("unknown {0}")]
    UnknownCharacter(CharacterId),
}

impl InstrumentError {
    /// A fetch failure for `did`.
    pub(crate) fn fetch(did: u32, e: impl std::fmt::Display) -> Self {
        Self::Fetch {
            did,
            message: e.to_string(),
        }
    }
}
