// SPDX-License-Identifier: GPL-3.0-or-later
//! The playback queue (`instruments.md` §8.1–§8.3): one time-ordered queue of note events
//! for all characters.

use std::cmp::Ordering;
use std::collections::{BTreeMap, HashMap};
use std::ops::Bound;

use super::note::{CharacterId, MusicEvent, MusicEventKind};

/// Two event times closer than this count as equal; equal times are ordered by sequence
/// number (§8.2).
pub const TIE_TOLERANCE: f64 = 0.0001;

/// A note-on due this long ago or longer is discarded without playing (§8.3 step 2).
pub const LATE_DROP_SECS: f64 = 0.5;

/// Most distinct event times looked at when choosing among events within [`TIE_TOLERANCE`]
/// of the earliest one. Among events with one exact time the first in the map already has
/// the lowest sequence number, so only the first event of each distinct time is a
/// candidate: a pop costs at most this many map lookups however many events share a time.
/// Beyond it (only hostile input puts more than 16 distinct times within 0.1 ms) the lowest
/// sequence number among the first 16 times wins.
const TIE_WINDOW: usize = 16;

/// An event time with a total order (`f64::total_cmp`).
#[derive(Debug, Clone, Copy)]
struct Time(f64);

impl PartialEq for Time {
    fn eq(&self, o: &Self) -> bool {
        self.cmp(o) == Ordering::Equal
    }
}
impl Eq for Time {}
impl PartialOrd for Time {
    fn partial_cmp(&self, o: &Self) -> Option<Ordering> {
        Some(self.cmp(o))
    }
}
impl Ord for Time {
    fn cmp(&self, o: &Self) -> Ordering {
        self.0.total_cmp(&o.0)
    }
}

/// What [`NoteQueue::pop_due`] returns.
#[derive(Debug, Clone, Copy, PartialEq)]
pub enum Due {
    /// Process this event (§8.3 steps 3–4).
    Ready(MusicEvent),
    /// A note-on 0.5 s or more late: discarded without playing (§8.3 step 2).
    Late(MusicEvent),
}

/// The queue of note events, ordered by time with the tolerance of §8.2, then by sequence
/// number.
#[derive(Debug, Clone, Default)]
pub struct NoteQueue {
    events: BTreeMap<(Time, u64), MusicEvent>,
    /// Queued events per character (entries are removed when they reach 0).
    counts: HashMap<CharacterId, usize>,
}

impl NoteQueue {
    /// An empty queue.
    #[must_use]
    pub fn new() -> Self {
        Self::default()
    }

    /// Adds an event. A non-finite time counts as 0 (due at once). An event with the same
    /// time and sequence number as a queued one replaces it.
    pub fn push(&mut self, mut ev: MusicEvent) {
        if !ev.time.is_finite() {
            ev.time = 0.0;
        }
        if let Some(old) = self.events.insert((Time(ev.time), ev.seq), ev) {
            self.uncount(old.character);
        }
        *self.counts.entry(ev.character).or_insert(0) += 1;
    }

    fn uncount(&mut self, character: CharacterId) {
        if let Some(n) = self.counts.get_mut(&character) {
            *n = n.saturating_sub(1);
            if *n == 0 {
                self.counts.remove(&character);
            }
        }
    }

    /// Number of queued events of `character`.
    #[must_use]
    pub fn count(&self, character: CharacterId) -> usize {
        self.counts.get(&character).copied().unwrap_or(0)
    }

    /// Number of queued events.
    #[must_use]
    pub fn len(&self) -> usize {
        self.events.len()
    }

    /// Whether the queue is empty.
    #[must_use]
    pub fn is_empty(&self) -> bool {
        self.events.is_empty()
    }

    /// The time of the earliest event.
    #[must_use]
    pub fn next_time(&self) -> Option<f64> {
        self.events.keys().next().map(|(t, _)| t.0)
    }

    /// Takes the next event to process at audio-clock time `now`, considering events due
    /// up to `until` (≥ `now` to schedule ahead, sample-accurately; `instruments.md` §12):
    ///
    /// 1. the earliest event; if it is later than `until`, nothing;
    /// 2. among the events within [`TIE_TOLERANCE`] of it, the one with the lowest sequence
    ///    number (§8.2);
    /// 3. a note-on at or before `now` − [`LATE_DROP_SECS`] comes back as [`Due::Late`]
    ///    (§8.3 step 2); note-offs are never discarded.
    pub fn pop_due(&mut self, now: f64, until: f64) -> Option<Due> {
        let (&first, _) = self.events.first_key_value()?;
        if first.0.0 > until {
            return None;
        }
        let limit = first.0.0 + TIE_TOLERANCE;
        let mut best = first;
        let mut cur = first;
        // The first event of each later distinct time within the tolerance.
        for _ in 1..TIE_WINDOW {
            let next = self
                .events
                .range((Bound::Excluded((cur.0, u64::MAX)), Bound::Unbounded))
                .next();
            let Some((&k, _)) = next else { break };
            if k.0.0 >= limit {
                break;
            }
            if k.1 < best.1 {
                best = k;
            }
            cur = k;
        }
        let ev = self.events.remove(&best)?;
        self.uncount(ev.character);
        if ev.kind == MusicEventKind::NoteOn && ev.time <= now - LATE_DROP_SECS {
            return Some(Due::Late(ev));
        }
        Some(Due::Ready(ev))
    }

    /// Removes every queued event of `character` (§8.5) and returns how many.
    pub fn remove_character(&mut self, character: CharacterId) -> usize {
        if self.counts.remove(&character).is_none() {
            return 0;
        }
        let before = self.events.len();
        self.events.retain(|_, e| e.character != character);
        before - self.events.len()
    }

    /// Whether `character` has queued events.
    #[must_use]
    pub fn has_character(&self, character: CharacterId) -> bool {
        self.counts.contains_key(&character)
    }

    /// The queued events in processing order of their exact times (for tools).
    pub fn iter(&self) -> impl Iterator<Item = &MusicEvent> {
        self.events.values()
    }
}
