// SPDX-License-Identifier: GPL-3.0-or-later
//! Synchronised group start (`instruments.md` §8.8–§8.9, §12 P4 Decision;
//! `abc-dialect.md` §8.5, §11, §11.1): `/play <song> sync` checks a part at once and lets it
//! wait; one start gives every waiting part of the group — the local player's and each
//! fellowship member's — the same start time *S*.
//!
//! The group is transport-free. It holds one waiting part per character, and
//! [`SyncGroup::start_at`] queues them on a [`MusicPlayer`] at the common *S* that the caller
//! supplies (from a scenario or session start event, or [`SyncGroup::start_time`] for a
//! start processed locally). A part whose start is processed less than
//! [`CATCH_UP_SECS`] before *S* loses its head by the catch-up rule of §8.7 step 4 instead
//! of starting late: every event that is scheduled keeps the time *S* + its offset.
//!
//! The start tolerance ([`START_TOLERANCE`], §8.9) is checked from the player's schedule log
//! ([`MusicPlayer::set_schedule_log`]) with [`SyncStart::measure`] and
//! [`SyncStart::check_tolerance`]: each performance's actual time-0 on the audio clock is
//! the scheduled time of each of its note events minus the event's offset. The measurement
//! holds only if [`MusicPlayer::update`] runs at least every
//! [`super::PlayerConfig::lookahead_secs`] of audio (or [`MusicPlayer::update_for`] with the
//! block's length): an event whose time passes between two updates is handed on late. The
//! pass of the criterion is [`ToleranceCheck::passes`].
//!
//! # Withdrawal and cancel (§8.8.7, §12 P4 Decision 6)
//!
//! | Event | Here |
//! |---|---|
//! | `/play … sync` accepted | [`SyncGroup::register`] / [`SyncGroup::register_member`] replace the part |
//! | `/play … sync` refused | nothing to call: the earlier part stays |
//! | Solo `/play` accepted | [`MusicPlayer::play_solo`] only: the waiting part stays |
//! | Start | [`SyncGroup::start_at`]: started parts leave the group |
//! | Music-end key | the group is not touched: the part keeps waiting |
//! | Leaving music mode, unequipping (Q11) | [`SyncGroup::withdraw`] |
//! | Member left the fellowship, fellowship gone | [`SyncGroup::withdraw_absent_members`] |
//! | `/playstop`, the feed stops a member's performance | [`SyncGroup::cancel`] |
//! | Teardown of the music module | [`SyncGroup::clear`] |

use std::collections::HashMap;
use std::sync::Arc;

use super::InstrumentError;
use super::note::{CharacterId, MusicEventKind};
use super::player::{MusicPlayer, QueuedRange, ScheduledEvent};
use crate::abc::{NoteEventKind, Performance, SOLO_START_OFFSET};

/// Most performances waiting in one group (a fellowship is far smaller; the bound keeps
/// hostile input from growing the group without limit).
pub const MAX_WAITING: usize = 256;

/// The start tolerance, seconds (`instruments.md` §8.9, Decision): on one listener's audio
/// clock, the time-0 of any two performances of one synchronised start differ by at most
/// this, and each differs from the intended common start *S* by at most this.
pub const START_TOLERANCE: f64 = 0.0001;

/// The catch-up threshold of the performance stepper, seconds (`instruments.md` §8.7 step
/// 4): a part whose start is scheduled less than this before *S* + its position skips its
/// head.
pub const CATCH_UP_SECS: f64 = 0.5;

/// The lead of a start processed by Tessera, seconds (`instruments.md` §12 P4 Decision 3:
/// *A* = 0.7, the solo start's delay).
pub const SYNC_LEAD_SECS: f64 = SOLO_START_OFFSET;

/// Whose part waits: the local player's own, or a fellowship member's part received from a
/// scenario or session feed (`instruments.md` §12 P4 Decision 1, 5).
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Default)]
pub enum PartOrigin {
    /// The local player's `/play … sync`.
    #[default]
    Local,
    /// A fellowship member's part made ready by the feed.
    Member,
}

/// A performance waiting for the synchronised start.
#[derive(Debug, Clone, PartialEq)]
pub struct WaitingPerformance {
    /// The performer.
    pub character: CharacterId,
    /// The checked performance (`abc-dialect.md` §8.5: read and checked at once).
    pub performance: Arc<Performance>,
    /// The performer's instrument type (§4.5 of `instruments.md`).
    pub instrument: u32,
    /// Local or received.
    pub origin: PartOrigin,
}

/// One part a start queued (or skipped whole), for the report and the time-0 measurement.
#[derive(Debug, Clone, PartialEq)]
pub struct StartedPart {
    /// The performer.
    pub character: CharacterId,
    /// Local or received.
    pub origin: PartOrigin,
    /// The performance that started.
    pub performance: Arc<Performance>,
    /// Events at the head of the event list skipped by the catch-up rule (§8.7 step 4);
    /// 0 for a start processed at least [`CATCH_UP_SECS`] before *S*.
    pub skipped: usize,
    /// The first queue sequence number of the part's events (they are consecutive).
    pub first_seq: u64,
    /// Events queued (the synthetic end of a performance without one included); 0 when the
    /// whole part was skipped.
    pub queued: usize,
}

impl StartedPart {
    /// The time-line offset (`abc-dialect.md` §8.4) of the part's `j`-th queued event and its
    /// kind, or `None` past the queued events.
    fn queued_event(&self, j: usize) -> Option<(f64, NoteEventKind)> {
        if j >= self.queued {
            return None;
        }
        let i = self.skipped.checked_add(j)?;
        match self.performance.events.get(i) {
            Some(e) => Some((e.time, e.kind)),
            None => Some((
                MusicPlayer::synthetic_end_time(&self.performance),
                NoteEventKind::End,
            )),
        }
    }

    /// Note-ons and note-offs among the queued events: what the tolerance measures.
    #[must_use]
    pub fn queued_notes(&self) -> usize {
        (0..self.queued)
            .filter_map(|j| self.queued_event(j))
            .filter(|(_, k)| *k != NoteEventKind::End)
            .count()
    }
}

/// Which queue limit refused a part at a start.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum QueueLimit {
    /// [`super::PlayerConfig::max_events_per_character`]
    /// ([`InstrumentError::CharacterQueueFull`]).
    Character,
    /// [`super::PlayerConfig::max_queued_events`] ([`InstrumentError::QueueFull`]).
    Total,
}

/// A waiting part a start could not queue because it did not fit the player's queue. It
/// left the group like every other part (§12 P4 Decision 3: the start empties the waiting
/// slots); its performer's running performance was not touched. The caller shows Tessera's
/// own line for it.
#[derive(Debug, Clone, PartialEq)]
pub struct RefusedPart {
    /// The performer.
    pub character: CharacterId,
    /// Local or received.
    pub origin: PartOrigin,
    /// The limit that refused it.
    pub limit: QueueLimit,
    /// Events queued (of the performer, or in all) that the part would have joined.
    pub queued: usize,
    /// Events the part needed (after its head skip).
    pub adding: usize,
}

/// The outcome of a start: who starts, and when.
#[derive(Debug, Clone, PartialEq)]
pub struct SyncStart {
    /// The common start time on the audio clock, seconds: time 0 of every performance.
    pub start_secs: f64,
    /// The performers whose parts started, in registration order.
    pub characters: Vec<CharacterId>,
    /// What each performer's part queued, in the same order.
    pub parts: Vec<StartedPart>,
    /// The parts the queue could not take, in registration order (normally empty).
    pub refused: Vec<RefusedPart>,
}

/// The measured time-0 of one started part (`instruments.md` §8.9).
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct PartTiming {
    /// The performer.
    pub character: CharacterId,
    /// Note-ons and note-offs of the part found in the log.
    pub measured: usize,
    /// Measured events whose time-0 is not finite (a non-finite offset or log time); each
    /// fails the tolerance, and none enters `earliest` or `latest`.
    pub non_finite: usize,
    /// Note-ons and note-offs the part queued.
    pub expected: usize,
    /// The earliest finite time-0 (scheduled time minus offset) over the measured events,
    /// seconds on the audio clock; NaN when none was measured.
    pub earliest: f64,
    /// The latest finite time-0 over the measured events; NaN when none was measured.
    pub latest: f64,
}

/// The start tolerance checked over every part of one start (`instruments.md` §8.9).
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct ToleranceCheck {
    /// The largest difference between two measured time-0s (any parts), seconds.
    pub spread: f64,
    /// The largest |time-0 − *S*| over the measured events, seconds.
    pub worst_from_start: f64,
    /// Note events measured.
    pub measured: usize,
    /// Measured note events whose time-0 is not finite: each fails the tolerance.
    pub non_finite: usize,
    /// Note events queued but not found in the log (not processed yet, dropped as late, or
    /// beyond the log's capacity, [`MusicPlayer::schedule_log_dropped`]).
    pub missing: usize,
}

impl ToleranceCheck {
    /// Whether both rules of §8.9 hold for every measured event: no non-finite time-0, and
    /// `spread` and `worst_from_start` within [`START_TOLERANCE`]. Events not measured are
    /// not judged, so this is true when nothing was measured; the criterion check uses
    /// [`Self::passes`].
    #[must_use]
    pub fn within(&self) -> bool {
        self.non_finite == 0
            && self.spread <= START_TOLERANCE
            && self.worst_from_start <= START_TOLERANCE
    }

    /// The start tolerance of §8.9 as P4 exit criterion 3 checks it: something was
    /// measured, every queued note event was found in the log (`missing` 0: none
    /// unprocessed, late-dropped or beyond the log), and [`Self::within`] holds.
    #[must_use]
    pub fn passes(&self) -> bool {
        self.measured > 0 && self.missing == 0 && self.within()
    }
}

impl SyncStart {
    /// Each part's time-0 measured from a schedule log (`instruments.md` §8.9): for every
    /// note-on and note-off of the part found in `log` (matched by character and sequence
    /// number), its scheduled time on the audio clock ([`ScheduledEvent::secs`]) minus its
    /// offset on the part's time line. Log entries of other events are ignored.
    #[must_use]
    pub fn measure(&self, log: &[ScheduledEvent]) -> Vec<PartTiming> {
        let mut timings: Vec<PartTiming> = self
            .parts
            .iter()
            .map(|p| PartTiming {
                character: p.character,
                measured: 0,
                non_finite: 0,
                expected: p.queued_notes(),
                earliest: f64::NAN,
                latest: f64::NAN,
            })
            .collect();
        let mut by_character: HashMap<CharacterId, Vec<usize>> = HashMap::new();
        for (i, p) in self.parts.iter().enumerate() {
            by_character.entry(p.character).or_default().push(i);
        }
        for ev in log {
            if !matches!(ev.kind, MusicEventKind::NoteOn | MusicEventKind::NoteOff) {
                continue;
            }
            let Some(parts) = by_character.get(&ev.character) else {
                continue;
            };
            for &i in parts {
                let part = &self.parts[i];
                let Ok(j) = usize::try_from(ev.seq.wrapping_sub(part.first_seq)) else {
                    continue;
                };
                let Some((offset, kind)) = part.queued_event(j) else {
                    continue;
                };
                if kind == NoteEventKind::End {
                    continue;
                }
                let t0 = ev.secs - offset;
                let t = &mut timings[i];
                t.measured += 1;
                if !t0.is_finite() {
                    t.non_finite += 1;
                } else if t.measured - t.non_finite == 1 {
                    t.earliest = t0;
                    t.latest = t0;
                } else {
                    t.earliest = t.earliest.min(t0);
                    t.latest = t.latest.max(t0);
                }
            }
        }
        timings
    }

    /// [`Self::measure`] summarised against both rules of the start tolerance.
    #[must_use]
    pub fn check_tolerance(&self, log: &[ScheduledEvent]) -> ToleranceCheck {
        let timings = self.measure(log);
        let mut lo = f64::INFINITY;
        let mut hi = f64::NEG_INFINITY;
        let mut measured = 0usize;
        let mut non_finite = 0usize;
        let mut missing = 0usize;
        for t in &timings {
            measured += t.measured;
            non_finite += t.non_finite;
            missing += t.expected.saturating_sub(t.measured);
            if t.measured > t.non_finite {
                lo = lo.min(t.earliest);
                hi = hi.max(t.latest);
            }
        }
        if measured == non_finite {
            return ToleranceCheck {
                spread: 0.0,
                worst_from_start: 0.0,
                measured,
                non_finite,
                missing,
            };
        }
        ToleranceCheck {
            spread: hi - lo,
            worst_from_start: (hi - self.start_secs)
                .abs()
                .max((lo - self.start_secs).abs()),
            measured,
            non_finite,
            missing,
        }
    }
}

/// The head a fresh stepper skips (`instruments.md` §8.7 step 4) when its performance with
/// time-0 at `start_secs` is first scheduled at `now_secs`: the number of events taken at the
/// head of the event list without sounding, and whether the end was among them (then
/// nothing of the part plays).
///
/// With *ahead* = `start_secs` − `now_secs` (the position is 0 before any event is taken):
/// nothing is skipped when *ahead* ≥ [`CATCH_UP_SECS`]. Otherwise *k* = 0.5 − *ahead*;
/// while *k* > 0 and the end has not been taken, the next event is taken and its delta
/// subtracted from *k*, then every following event whose delta is 0. So the part before
/// `now_secs` + 0.5 — and the first event at or after it, with its zero-delta followers —
/// never sounds; the rest keeps its times. A non-finite `start_secs` or `now_secs` (a
/// scheduling time that is not known, `instruments.md` §12 implementation OPEN (d)) skips
/// nothing.
#[must_use]
pub fn head_skip(performance: &Performance, start_secs: f64, now_secs: f64) -> (usize, bool) {
    let events = &performance.events;
    let ahead = start_secs - now_secs;
    if !start_secs.is_finite() || !now_secs.is_finite() || ahead >= CATCH_UP_SECS {
        return (0, false);
    }
    let mut k = CATCH_UP_SECS - ahead;
    let mut i = 0usize;
    let mut end = false;
    while k > 0.0 && !end && i < events.len() {
        let d = events[i].delta;
        if d.is_finite() {
            k -= d;
        }
        end |= events[i].kind == NoteEventKind::End;
        i += 1;
        while !end && i < events.len() && events[i].delta == 0.0 {
            end |= events[i].kind == NoteEventKind::End;
            i += 1;
        }
    }
    (i, end || i >= events.len())
}

/// A group of performers whose waiting performances start together: the local player and
/// the members of its fellowship (`instruments.md` §8.8, §12 P4 Decision 2).
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

    /// `/play <song> sync` accepted: the character's checked performance waits as a
    /// [`PartOrigin::Local`] part. A second registration of the same character replaces the
    /// first (one waiting slot, §8.8.1); nothing running is touched. The caller checks the
    /// part's size at once with [`MusicPlayer::check_part_size`] (§8.8.1: a part that can
    /// never fit the queue is refused then); a part that does not fit at the start is
    /// refused there ([`SyncStart::refused`]).
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
        self.register_part(WaitingPerformance {
            character,
            performance: performance.into(),
            instrument,
            origin: PartOrigin::Local,
        })
    }

    /// A fellowship member's part made ready by the feed (`PerformanceReady`, §12 P4
    /// Decision 5): it waits as a [`PartOrigin::Member`] part, replacing the member's earlier
    /// one. Check its size as for [`Self::register`].
    ///
    /// # Errors
    ///
    /// [`InstrumentError::Limit`] beyond [`MAX_WAITING`] performers.
    pub fn register_member(
        &mut self,
        character: CharacterId,
        performance: impl Into<Arc<Performance>>,
        instrument: u32,
    ) -> Result<(), InstrumentError> {
        self.register_part(WaitingPerformance {
            character,
            performance: performance.into(),
            instrument,
            origin: PartOrigin::Member,
        })
    }

    /// Lets `part` wait, replacing the character's earlier part (whatever its origin; the
    /// replacement keeps the earlier part's place in registration order).
    ///
    /// # Errors
    ///
    /// [`InstrumentError::Limit`] beyond [`MAX_WAITING`] performers.
    pub fn register_part(&mut self, part: WaitingPerformance) -> Result<(), InstrumentError> {
        if let Some(slot) = self
            .waiting
            .iter_mut()
            .find(|x| x.character == part.character)
        {
            *slot = part;
            return Ok(());
        }
        if self.waiting.len() >= MAX_WAITING {
            return Err(InstrumentError::Limit("waiting performances"));
        }
        self.waiting.push(part);
        Ok(())
    }

    /// Withdraws the character's waiting part (§8.8.7: leaving music mode or unequipping,
    /// Q11; a member's part the feed withdraws). The running performance is not touched.
    /// Returns whether one was waiting.
    pub fn withdraw(&mut self, character: CharacterId) -> bool {
        let before = self.waiting.len();
        self.waiting.retain(|w| w.character != character);
        self.waiting.len() != before
    }

    /// The fellowship changed (`FellowshipChanged`, §12 P4 Decision 6): every
    /// [`PartOrigin::Member`] part whose character is not in `members` is withdrawn (an
    /// empty list: the fellowship is gone). Local parts stay. Returns the withdrawn
    /// characters, in registration order.
    pub fn withdraw_absent_members(&mut self, members: &[CharacterId]) -> Vec<CharacterId> {
        let mut gone = Vec::new();
        self.waiting.retain(|w| {
            let keep = w.origin != PartOrigin::Member || members.contains(&w.character);
            if !keep {
                gone.push(w.character);
            }
            keep
        });
        gone
    }

    /// `/playstop` (Tessera's own command, `abc-dialect.md` §11 (a)) or the feed's
    /// `PerformanceStopped` for a member (§12 P4 Decision 6): cancels the character's
    /// running performance on `player` (§8.5) and withdraws its waiting part. Returns
    /// whether a part was waiting.
    pub fn cancel(&mut self, player: &mut MusicPlayer, character: CharacterId) -> bool {
        if player.is_performing(character) {
            player.stop(character);
        }
        self.withdraw(character)
    }

    /// Teardown of the music module (§8.8.7): every waiting part is removed.
    pub fn clear(&mut self) {
        self.waiting.clear();
    }

    /// Whether the character has a waiting part.
    #[must_use]
    pub fn is_waiting(&self, character: CharacterId) -> bool {
        self.waiting.iter().any(|w| w.character == character)
    }

    /// The waiting parts, in registration order.
    #[must_use]
    pub fn waiting(&self) -> &[WaitingPerformance] {
        &self.waiting
    }

    /// Number of waiting parts.
    #[must_use]
    pub fn len(&self) -> usize {
        self.waiting.len()
    }

    /// Whether nothing waits.
    #[must_use]
    pub fn is_empty(&self) -> bool {
        self.waiting.is_empty()
    }

    /// The common start time for a start Tessera processes at audio-clock time
    /// `command_secs`: *S* = *t* + 0.7 ([`SYNC_LEAD_SECS`], `instruments.md` §12 P4
    /// Decision 3).
    #[must_use]
    pub fn start_time(command_secs: f64) -> f64 {
        Self::start_time_after(command_secs, SYNC_LEAD_SECS)
    }

    /// *S* = `command_secs` + `lead_secs`: the start time of a start event that carries its
    /// own lead (a scenario or session start, §8.8.3). A negative or non-finite lead counts
    /// as 0.
    #[must_use]
    pub fn start_time_after(command_secs: f64, lead_secs: f64) -> f64 {
        let lead = if lead_secs.is_finite() {
            lead_secs.max(0.0)
        } else {
            0.0
        };
        command_secs + lead
    }

    /// A start with no catch-up: [`Self::start_at`] as if scheduled long before `start_secs`
    /// (no head is skipped, whatever the clock says).
    ///
    /// # Errors
    ///
    /// As [`Self::start_at`].
    pub fn start(
        &mut self,
        player: &mut MusicPlayer,
        start_secs: f64,
    ) -> Result<SyncStart, InstrumentError> {
        self.start_at(player, start_secs, f64::NEG_INFINITY)
    }

    /// One synchronised start (`instruments.md` §8.8.3–§8.8.4, §12 P4 Decision 3–4): every
    /// waiting part — local and received — is queued on `player` with time-0 at the common
    /// `start_secs` (*S*, from the caller), and the group is emptied. `now_secs` is the
    /// audio-clock time of the scheduling: a part that is scheduled less than
    /// [`CATCH_UP_SECS`] before *S* skips its head by [`head_skip`] instead of starting late,
    /// and the rest keeps *S* + offset. A performer's running performance is cancelled
    /// first only if its own part waits (§8.8.7); performers without a waiting part are not
    /// touched, and a start with nothing waiting does nothing (the caller still shows the
    /// start notice once). Leadership plays no part.
    ///
    /// The parts are taken in registration order, each with the room the earlier ones left:
    /// a part that does not fit the player's queue limits is not queued, its performer's
    /// running performance is not touched, and it is reported in [`SyncStart::refused`].
    /// Performers the player does not know are skipped. Either way the part leaves the
    /// group: a start always empties every waiting slot (§8.8.3 step 2, §12 P4 Decision 3),
    /// so one part that can never fit cannot block the others.
    ///
    /// `now_secs` must be the audio clock at the call (or a non-finite value, which skips
    /// nothing); the tolerance of §8.9 then needs [`MusicPlayer::update`] at least every
    /// [`super::PlayerConfig::lookahead_secs`] of audio, or [`MusicPlayer::update_for`].
    ///
    /// # Errors
    ///
    /// [`InstrumentError::InvalidTime`] for a non-finite `start_secs`; nothing changes then.
    pub fn start_at(
        &mut self,
        player: &mut MusicPlayer,
        start_secs: f64,
        now_secs: f64,
    ) -> Result<SyncStart, InstrumentError> {
        if !start_secs.is_finite() {
            return Err(InstrumentError::InvalidTime("synchronised start"));
        }
        let mut characters = Vec::with_capacity(self.waiting.len());
        let mut parts = Vec::with_capacity(self.waiting.len());
        let mut refused = Vec::new();
        for w in std::mem::take(&mut self.waiting) {
            if !player.has_character(w.character) {
                tracing::debug!(character = %w.character, "synced performer unknown to the player; skipped");
                continue;
            }
            let (skip, whole) = head_skip(&w.performance, start_secs, now_secs);
            let range = if whole {
                // The whole part lies before the catch-up horizon: the start still stops
                // the running performance (§8.7 start step 1), and nothing sounds.
                if player.is_performing(w.character) {
                    player.stop(w.character);
                }
                tracing::debug!(character = %w.character, "synced part started too late; every event skipped");
                QueuedRange {
                    first_seq: 0,
                    count: 0,
                }
            } else {
                let n = MusicPlayer::events_needed(&w.performance).saturating_sub(skip);
                if let Err(e) = player.check_room(&[(w.character, n, true)]) {
                    let (limit, queued, adding) = match e {
                        InstrumentError::CharacterQueueFull { queued, adding, .. } => {
                            (QueueLimit::Character, queued, adding)
                        }
                        InstrumentError::QueueFull { queued, adding } => {
                            (QueueLimit::Total, queued, adding)
                        }
                        _ => (QueueLimit::Total, player.queued(), n),
                    };
                    tracing::debug!(character = %w.character, queued, adding, "synced part does not fit the queue; refused");
                    refused.push(RefusedPart {
                        character: w.character,
                        origin: w.origin,
                        limit,
                        queued,
                        adding,
                    });
                    continue;
                }
                player.play_from(w.character, &w.performance, w.instrument, start_secs, skip)
            };
            characters.push(w.character);
            parts.push(StartedPart {
                character: w.character,
                origin: w.origin,
                performance: w.performance,
                skipped: skip,
                first_seq: range.first_seq,
                queued: range.count,
            });
        }
        Ok(SyncStart {
            start_secs,
            characters,
            parts,
            refused,
        })
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::abc::{self, PartRequest};

    /// `instruments.md` §14: the part **CDEF** (unit 0.25 s).
    fn cdef() -> Performance {
        abc::parse(
            b"X:1\nT:CDEF\nL:1/8\nQ:1/4=120\nK:C\nCDEF|\n",
            PartRequest::Unspecified,
        )
        .unwrap()
    }

    #[test]
    fn head_skip_on_time_skips_nothing() {
        assert_eq!(head_skip(&cdef(), 10.0, 7.0), (0, false));
        assert_eq!(head_skip(&cdef(), 10.0, 9.5), (0, false));
        assert_eq!(head_skip(&cdef(), f64::NAN, 9.5), (0, false));
    }

    /// A scheduling or start time that is not known (non-finite) skips nothing, whatever
    /// its sign (§12 implementation OPEN (d)).
    #[test]
    fn head_skip_non_finite_times_skip_nothing() {
        let p = cdef();
        for (start, now) in [
            (0.0, f64::INFINITY),
            (0.0, f64::NEG_INFINITY),
            (0.0, f64::NAN),
            (f64::INFINITY, 0.0),
            (f64::NEG_INFINITY, 0.0),
            (f64::NAN, 0.0),
        ] {
            assert_eq!(head_skip(&p, start, now), (0, false), "{start} {now}");
        }
    }

    /// §14.10: *S* = 10.0, first tick at 9.8: on C, off C, on D, off D, on E are taken.
    #[test]
    fn head_skip_14_10() {
        let p = cdef();
        let (skip, whole) = head_skip(&p, 10.0, 9.8);
        assert_eq!(skip, 5);
        assert!(!whole);
        assert_eq!(p.events[skip].kind, NoteEventKind::Off);
        assert!((p.events[skip].time - 0.75).abs() < 1e-12);
    }

    #[test]
    fn head_skip_past_the_end_takes_everything() {
        let p = cdef();
        assert_eq!(head_skip(&p, 10.0, 12.0), (p.events.len(), true));
        assert_eq!(head_skip(&p, 0.0, 1.0e9), (p.events.len(), true));
        let empty = Performance {
            events: Vec::new(),
            ..p
        };
        assert_eq!(head_skip(&empty, 0.0, 1.0), (0, true));
    }
}
