// SPDX-License-Identifier: GPL-3.0-or-later
//! Synchronised start (`abc-dialect.md` §8.5, §11; ROADMAP M6): `/play <song> sync` checks
//! a performance at once and lets it wait; `/playstart` starts every waiting performance of
//! the group at one common audio-clock time.
//!
//! The group is transport-free: it holds the waiting performances and computes the start
//! time, and [`SyncGroup::start`] hands them to a [`MusicPlayer`]. A networked client (M7)
//! can carry the registrations and the [`SyncStart`] over its own protocol and apply the
//! same start on every peer.

use std::sync::Arc;

use super::InstrumentError;
use super::note::CharacterId;
use super::player::MusicPlayer;
use crate::abc::{Performance, SOLO_START_OFFSET};

/// Most performances waiting in one group (a fellowship is far smaller; the bound keeps
/// hostile input from growing the group without limit).
pub const MAX_WAITING: usize = 256;

/// A performance waiting for the synchronised start.
#[derive(Debug, Clone, PartialEq)]
pub struct WaitingPerformance {
    /// The performer.
    pub character: CharacterId,
    /// The checked performance (`abc-dialect.md` §8.5: read and checked at once).
    pub performance: Arc<Performance>,
    /// The performer's instrument type (§4.5 of `instruments.md`).
    pub instrument: u32,
}

/// The outcome of a start command: who starts, and when.
#[derive(Debug, Clone, PartialEq)]
pub struct SyncStart {
    /// The common start time on the audio clock, seconds: time 0 of every performance.
    pub start_secs: f64,
    /// The performers, in registration order.
    pub characters: Vec<CharacterId>,
}

/// A group of performers whose waiting performances start together (a fellowship).
#[derive(Debug, Clone, Default, PartialEq)]
pub struct SyncGroup {
    waiting: Vec<WaitingPerformance>,
}

impl SyncGroup {
    /// An empty group.
    #[must_use]
    pub fn new() -> Self {
        Self::default()
    }

    /// `/play <song> sync`: the character's checked performance waits. A second
    /// registration of the same character replaces the first.
    ///
    /// # Errors
    ///
    /// [`InstrumentError::Limit`] beyond [`MAX_WAITING`] performers.
    pub fn register(
        &mut self,
        character: CharacterId,
        performance: impl Into<Arc<Performance>>,
        instrument: u32,
    ) -> Result<(), InstrumentError> {
        let w = WaitingPerformance {
            character,
            performance: performance.into(),
            instrument,
        };
        if let Some(slot) = self.waiting.iter_mut().find(|x| x.character == character) {
            *slot = w;
            return Ok(());
        }
        if self.waiting.len() >= MAX_WAITING {
            return Err(InstrumentError::Limit("waiting performances"));
        }
        self.waiting.push(w);
        Ok(())
    }

    /// Withdraws the character's waiting performance (it left the group, was removed or
    /// stopped). Returns whether one was waiting.
    pub fn withdraw(&mut self, character: CharacterId) -> bool {
        let before = self.waiting.len();
        self.waiting.retain(|w| w.character != character);
        self.waiting.len() != before
    }

    /// Whether the character has a waiting performance.
    #[must_use]
    pub fn is_waiting(&self, character: CharacterId) -> bool {
        self.waiting.iter().any(|w| w.character == character)
    }

    /// The waiting performances, in registration order.
    #[must_use]
    pub fn waiting(&self) -> &[WaitingPerformance] {
        &self.waiting
    }

    /// Number of waiting performances.
    #[must_use]
    pub fn len(&self) -> usize {
        self.waiting.len()
    }

    /// Whether nothing waits.
    #[must_use]
    pub fn is_empty(&self) -> bool {
        self.waiting.is_empty()
    }

    /// The common start time for a start command processed at audio-clock time
    /// `command_secs`: the same 0.7 s delay as a solo start (Decision; where the original
    /// takes its synced start time from is open, `abc-dialect.md` §11 OPEN (3)).
    #[must_use]
    pub fn start_time(command_secs: f64) -> f64 {
        command_secs + SOLO_START_OFFSET
    }

    /// `/playstart`: queues every waiting performance on `player` with time 0 at
    /// `start_secs` and empties the group. Performers the player does not know are
    /// skipped (and leave the group). All or nothing: the room for every performance is
    /// checked first, so either every known performer starts and [`SyncStart`] names them,
    /// or nothing is queued, nothing running is cancelled and every performance still
    /// waits.
    ///
    /// # Errors
    ///
    /// [`InstrumentError::QueueFull`] or [`InstrumentError::CharacterQueueFull`] when the
    /// performances do not fit together.
    pub fn start(
        &mut self,
        player: &mut MusicPlayer,
        start_secs: f64,
    ) -> Result<SyncStart, InstrumentError> {
        let requests: Vec<(CharacterId, usize, bool)> = self
            .waiting
            .iter()
            .filter(|w| player.has_character(w.character))
            .map(|w| {
                (
                    w.character,
                    MusicPlayer::events_needed(&w.performance),
                    true,
                )
            })
            .collect();
        player.check_room(&requests)?;
        let mut started = Vec::with_capacity(requests.len());
        for w in self.waiting.drain(..) {
            if player.has_character(w.character) {
                player.play_checked(w.character, &w.performance, w.instrument, start_secs);
                started.push(w.character);
            } else {
                tracing::debug!(character = %w.character, "synced performer unknown to the player; skipped");
            }
        }
        Ok(SyncStart {
            start_secs,
            characters: started,
        })
    }
}
