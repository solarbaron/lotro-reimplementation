// SPDX-License-Identifier: GPL-3.0-or-later
//! The playback module (`instruments.md` §8–§10, §12): queue processing on the audio clock,
//! the registry of sounding notes, cancel, the instance limit, starting a note's sounds
//! and stopping them with their release ramp.

use std::collections::HashMap;
use std::sync::Arc;

use rand::SeedableRng;
use rand::rngs::SmallRng;
use tessera_formats::Did;

use super::InstrumentError;
use super::note::{CharacterId, LIVE_VELOCITY, MusicEvent, MusicEventKind, note_volume, pack_note};
use super::queue::{Due, NoteQueue};
use super::table::{InstrumentTable, MusicScript, NoteDraws, NoteSound};
use crate::abc::{NoteEventKind, Performance, SOLO_START_OFFSET};
use crate::engine::law::{FIRE_AND_FORGET_FLOOR, pitch_multiplier};
use crate::engine::{
    AudioEngine, AudioTime, Channel, LivePosition, LiveState, PositionalClass, Positioner,
    RequestOutcome, SoundRequest, SoundSource, VoiceHandle,
};
use crate::pcm::Pcm;

/// Supplies the audio of a sample record (0x0A) when a note starts.
///
/// The player never reads archives or decodes: the client's asset workers decode payloads
/// (`crate::decode`) and hand them over, either decoded ([`SoundSource::Pcm`]) or still
/// loading ([`SoundSource::Loading`] with a [`crate::engine::payload_channel`]). `None`
/// means the sample is not available: the note's sound does not play and
/// [`PlayerStats::missing_sample`] counts it.
pub trait SampleProvider {
    /// The audio of `payload` for a sound that starts now.
    fn source(&mut self, payload: Did) -> Option<SoundSource>;
}

impl<F: FnMut(Did) -> Option<SoundSource>> SampleProvider for F {
    fn source(&mut self, payload: Did) -> Option<SoundSource> {
        self(payload)
    }
}

/// A [`SampleProvider`] over decoded buffers kept in memory (shared, cheap to hand out).
#[derive(Debug, Clone, Default)]
pub struct SampleBank {
    samples: HashMap<Did, Pcm>,
}

impl SampleBank {
    /// An empty bank.
    #[must_use]
    pub fn new() -> Self {
        Self::default()
    }

    /// Adds or replaces the samples of `payload`.
    pub fn insert(&mut self, payload: Did, pcm: Pcm) {
        self.samples.insert(payload, pcm);
    }

    /// Removes the samples of `payload`.
    pub fn remove(&mut self, payload: Did) -> Option<Pcm> {
        self.samples.remove(&payload)
    }

    /// Whether `payload` is held.
    #[must_use]
    pub fn contains(&self, payload: Did) -> bool {
        self.samples.contains_key(&payload)
    }

    /// Number of payloads held.
    #[must_use]
    pub fn len(&self) -> usize {
        self.samples.len()
    }

    /// Whether the bank is empty.
    #[must_use]
    pub fn is_empty(&self) -> bool {
        self.samples.is_empty()
    }

    /// Drops every payload.
    pub fn clear(&mut self) {
        self.samples.clear();
    }
}

impl SampleProvider for SampleBank {
    fn source(&mut self, payload: Did) -> Option<SoundSource> {
        self.samples.get(&payload).cloned().map(SoundSource::Pcm)
    }
}

/// Player configuration.
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct PlayerConfig {
    /// Events due up to this many seconds after the audio clock are handed to the engine
    /// at each [`MusicPlayer::update`] with their exact start or stop frame, so that notes
    /// start and stop sample-accurately (§12 Decision 2). It should exceed the time
    /// between two updates. Default 0.1 s.
    pub lookahead_secs: f64,
    /// Most events in the queue (default 262,144). A performance that does not fit is
    /// refused as a whole. The note-offs of a cancel (§8.5) are always accepted, even beyond
    /// this and [`Self::max_events_per_character`]: there is at most one per registered note
    /// key (256 per character), a cancel first removes the character's queued events
    /// (earlier cancel note-offs included), and they are due at once.
    pub max_queued_events: usize,
    /// Most queued events of one character (default 65,536, more than three performances of
    /// the longest kind), so that one character, a remote peer's events included, cannot
    /// take the whole queue from the others.
    pub max_events_per_character: usize,
    /// Most events one [`MusicPlayer::update`] processes (default 8192). The rest wait for
    /// the next call, where the late-drop rule (§8.3 step 2) applies to them as usual; this
    /// bounds the work of one frame however many events fall due at once.
    pub max_events_per_update: usize,
    /// Most characters known at once (default 4096).
    pub max_characters: usize,
    /// Seed of the random draws (pitch variance, intensity variance, probability, random
    /// offsets).
    pub seed: u64,
}

impl Default for PlayerConfig {
    fn default() -> Self {
        Self {
            lookahead_secs: 0.1,
            max_queued_events: 262_144,
            max_events_per_character: 65_536,
            max_events_per_update: 8192,
            max_characters: 4096,
            seed: 0x7E55_E2A0,
        }
    }
}

/// Counters, for tools, tests and the M6 exit checks.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub struct PlayerStats {
    /// Note-ons processed (not dropped).
    pub note_ons: u64,
    /// Note-offs processed.
    pub note_offs: u64,
    /// Note-ons discarded as 0.5 s or more late (§8.3 step 2).
    pub late_dropped: u64,
    /// Events for characters the player does not know (§8.3 step 3).
    pub no_character: u64,
    /// Note-ons whose key was already registered: they play but no note-off can stop them
    /// (§8.4).
    pub unregistered: u64,
    /// Script runs refused by the instance limit (§8.6).
    pub limit_refused: u64,
    /// Note-ons whose script reaches no Sound node.
    pub silent_notes: u64,
    /// Sound nodes with the null sound info (§9.1 step 1).
    pub null_sound: u64,
    /// Sound nodes whose sound info is missing or undecodable.
    pub missing_sound_info: u64,
    /// Sound paths stopped by a probability draw.
    pub probability_skipped: u64,
    /// Fire-and-forget sounds at or below the audibility floor (§10.3).
    pub below_floor: u64,
    /// Sounds whose sample the [`SampleProvider`] did not have.
    pub missing_sample: u64,
    /// Requests the engine accepted and will play (started, pending or loading).
    pub voices_accepted: u64,
    /// Requests silently skipped by the voice limit (§9.3).
    pub voices_skipped: u64,
    /// Requests the engine suppressed.
    pub voices_suppressed: u64,
    /// Requests the engine refused (volume, distance, back end; §9.3).
    pub voices_refused: u64,
    /// Cancels (§8.5).
    pub cancels: u64,
    /// Performances queued.
    pub performances: u64,
}

#[derive(Debug)]
struct Character {
    position: LivePosition,
    /// Note key → the instance its note-on started (§8.4; instance ids are unique, so the
    /// script key of the original's entry is not needed).
    registry: HashMap<u8, u64>,
    /// Running instances of this character.
    instances: Vec<u64>,
    /// Every accepted voice that may still play (fire-and-forget ones included).
    voices: Vec<VoiceHandle>,
    /// Voices handed to the engine ahead of their event time (look-ahead), with that time:
    /// a cancel before the time withdraws them as if their note-on were still queued.
    ahead: Vec<(VoiceHandle, f64)>,
    /// A song performance is queued or playing.
    performing: bool,
}

#[derive(Debug)]
struct Instance {
    script: MusicScript,
    /// The tracked (non-fire-and-forget) voices the instance keeps (§9.1 step 8).
    voices: Vec<VoiceHandle>,
}

fn alive(engine: &AudioEngine, voices: &[VoiceHandle]) -> bool {
    voices.iter().any(|&h| engine.voice_state(h).is_some())
}

/// The playback module: one queue of note events for every character, the registry of
/// sounding notes, script instances and the instance limit, and the requests to the
/// engine (`instruments.md` §8–§10).
///
/// Per frame, after the client has moved the characters' [`LivePosition`]s and before
/// [`AudioEngine::update`]: call [`MusicPlayer::update`]. The player does not drain the
/// engine's events; it watches its voices with [`AudioEngine::voice_state`].
///
/// An instance stays active while one of its tracked (non-fire-and-forget) voices plays.
/// An instance whose sounds are all fire-and-forget completes at once and does not count
/// toward the instance limit (Decision for `instruments.md` Q2, from the node-completion
/// rules of `sound.md` §9.6; the Particle node is visual and not run here).
#[derive(Debug)]
pub struct MusicPlayer {
    table: Arc<InstrumentTable>,
    config: PlayerConfig,
    queue: NoteQueue,
    next_seq: u64,
    characters: HashMap<CharacterId, Character>,
    instances: HashMap<u64, Instance>,
    next_instance: u64,
    rng: SmallRng,
    stats: PlayerStats,
    /// Voices handed ahead whose note-on a cancel withdrew, with their start time: stopped
    /// at the next update (or at once where the engine is at hand) unless already started.
    revoked: Vec<(VoiceHandle, f64)>,
}

impl MusicPlayer {
    /// A player for `table`.
    #[must_use]
    pub fn new(table: Arc<InstrumentTable>, config: PlayerConfig) -> Self {
        Self {
            table,
            config,
            queue: NoteQueue::new(),
            next_seq: 0,
            characters: HashMap::new(),
            instances: HashMap::new(),
            next_instance: 1,
            rng: SmallRng::seed_from_u64(config.seed),
            stats: PlayerStats::default(),
            revoked: Vec::new(),
        }
    }

    /// The instrument table.
    #[must_use]
    pub fn table(&self) -> &Arc<InstrumentTable> {
        &self.table
    }

    /// The configuration.
    #[must_use]
    pub fn config(&self) -> &PlayerConfig {
        &self.config
    }

    /// The counters.
    #[must_use]
    pub fn stats(&self) -> PlayerStats {
        self.stats
    }

    /// The engine's audio clock in seconds: the time base of every event.
    #[must_use]
    pub fn now(engine: &AudioEngine) -> f64 {
        engine.clock().as_secs(engine.sample_rate())
    }

    /// Adds a character that plays at `position` (its live transform, shared with the
    /// engine's voices; §9.1 step 3, §10.5). Re-adding a character replaces its position.
    ///
    /// # Errors
    ///
    /// [`InstrumentError::Limit`] beyond [`PlayerConfig::max_characters`].
    pub fn add_character(
        &mut self,
        id: CharacterId,
        position: LivePosition,
    ) -> Result<(), InstrumentError> {
        if let Some(c) = self.characters.get_mut(&id) {
            c.position = position;
            return Ok(());
        }
        if self.characters.len() >= self.config.max_characters {
            return Err(InstrumentError::Limit("characters"));
        }
        self.characters.insert(
            id,
            Character {
                position,
                registry: HashMap::new(),
                instances: Vec::new(),
                voices: Vec::new(),
                ahead: Vec::new(),
                performing: false,
            },
        );
        Ok(())
    }

    /// Whether the character is known.
    #[must_use]
    pub fn has_character(&self, id: CharacterId) -> bool {
        self.characters.contains_key(&id)
    }

    /// Removes a character: its queued events are dropped and its tracked sounds stop now
    /// with their release ramp (§8.5 Decision: cancel when the character is removed). Its
    /// fire-and-forget sounds that have started play on, as in the original; mark its
    /// [`LivePosition`] removed to stop those too (`sound.md` §9.2 Decision). Sounds handed
    /// to the engine ahead of their note-on time that have not started are withdrawn, as
    /// their queued note-on would have been.
    pub fn remove_character(&mut self, engine: &mut AudioEngine, id: CharacterId) {
        self.queue.remove_character(id);
        if let Some(c) = self.characters.remove(&id) {
            self.stats.cancels += 1;
            self.revoked.extend(c.ahead);
            for i in c.instances {
                if let Some(inst) = self.instances.remove(&i) {
                    for h in inst.voices {
                        engine.stop(h);
                    }
                }
            }
        }
        self.flush_revoked(engine, Self::now(engine));
    }

    /// Whether the character has a song performance queued or playing.
    #[must_use]
    pub fn is_performing(&self, id: CharacterId) -> bool {
        self.characters.get(&id).is_some_and(|c| c.performing)
    }

    /// Whether note key `key` is registered for the character (§8.4).
    #[must_use]
    pub fn is_registered(&self, id: CharacterId, key: u8) -> bool {
        self.characters
            .get(&id)
            .is_some_and(|c| c.registry.contains_key(&key))
    }

    /// The registered note keys of the character, ascending.
    #[must_use]
    pub fn registered_keys(&self, id: CharacterId) -> Vec<u8> {
        let mut k: Vec<u8> = self
            .characters
            .get(&id)
            .map(|c| c.registry.keys().copied().collect())
            .unwrap_or_default();
        k.sort_unstable();
        k
    }

    /// The voices of the character that were accepted and may still play, in start order
    /// (pruned at each update).
    #[must_use]
    pub fn voices(&self, id: CharacterId) -> Vec<VoiceHandle> {
        self.characters
            .get(&id)
            .map(|c| c.voices.clone())
            .unwrap_or_default()
    }

    /// The active instances of `script` on the character (§8.6).
    #[must_use]
    pub fn active_instances(
        &self,
        engine: &AudioEngine,
        id: CharacterId,
        script: MusicScript,
    ) -> usize {
        let Some(c) = self.characters.get(&id) else {
            return 0;
        };
        c.instances
            .iter()
            .filter_map(|i| self.instances.get(i))
            .filter(|i| i.script == script && alive(engine, &i.voices))
            .count()
    }

    /// Number of queued events.
    #[must_use]
    pub fn queued(&self) -> usize {
        self.queue.len()
    }

    /// The queue (for tools).
    #[must_use]
    pub fn queue(&self) -> &NoteQueue {
        &self.queue
    }

    fn fresh_seq(&mut self) -> u64 {
        let s = self.next_seq;
        self.next_seq = self.next_seq.wrapping_add(1);
        s
    }

    /// Checks that every `(character, events, replaces)` request fits the queue together:
    /// `replaces` means the request cancels the character's running performance first, which
    /// frees its queued events (the cancel's note-offs are headroom, see
    /// [`PlayerConfig::max_queued_events`]). Nothing changes.
    pub(super) fn check_room(
        &self,
        requests: &[(CharacterId, usize, bool)],
    ) -> Result<(), InstrumentError> {
        let mut queued = self.queue.len();
        let mut adding = 0usize;
        for &(id, n, replaces) in requests {
            let own = self.queue.count(id);
            let freed = if replaces && self.is_performing(id) {
                own
            } else {
                0
            };
            queued = queued.saturating_sub(freed);
            adding = adding.saturating_add(n);
            let kept = own - freed;
            if kept.saturating_add(n) > self.config.max_events_per_character {
                return Err(InstrumentError::CharacterQueueFull {
                    character: id,
                    queued: kept,
                    adding: n,
                });
            }
        }
        if queued.saturating_add(adding) > self.config.max_queued_events {
            return Err(InstrumentError::QueueFull { queued, adding });
        }
        Ok(())
    }

    /// Number of events [`Self::play`] queues for `performance`: its events, plus an end
    /// event when it has none.
    pub(super) fn events_needed(performance: &Performance) -> usize {
        let has_end = performance
            .events
            .iter()
            .any(|e| e.kind == NoteEventKind::End);
        performance
            .events
            .len()
            .saturating_add(usize::from(!has_end))
    }

    /// Queues one event with a fresh sequence number (the given `seq` is replaced) and
    /// returns that number. For events that arrive from elsewhere (another player's notes
    /// carried by a transport, tools, tests).
    ///
    /// # Errors
    ///
    /// [`InstrumentError::QueueFull`] when the queue is full;
    /// [`InstrumentError::CharacterQueueFull`] when the character's share is used up.
    pub fn queue_event(&mut self, mut ev: MusicEvent) -> Result<u64, InstrumentError> {
        self.check_room(&[(ev.character, 1, false)])?;
        ev.seq = self.fresh_seq();
        self.queue.push(ev);
        Ok(ev.seq)
    }

    /// Queues a song performance of `id` on `instrument` whose time 0 is `start_secs` on
    /// the audio clock (`abc-dialect.md` §8.5, §9.4; `instruments.md` §8.1). Notes carry the
    /// from-song flag and not the quantized flag (§5.1), their pitch is turned into a note
    /// key by the label rule (§7.1) and their volume value is the velocity (§10.2).
    /// A performance already queued or playing for the character is cancelled first
    /// (Decision: one performance per character). A performance without an end event (one
    /// built by hand or carried by a transport) gets one at its last event time, so that the
    /// character does not stay performing. Returns the number of events queued.
    ///
    /// # Errors
    ///
    /// [`InstrumentError::UnknownCharacter`]; [`InstrumentError::QueueFull`] or
    /// [`InstrumentError::CharacterQueueFull`] when the performance does not fit (counting
    /// the events the cancel of the running one frees). Nothing changes then: a running
    /// performance goes on.
    pub fn play(
        &mut self,
        id: CharacterId,
        performance: &Performance,
        instrument: u32,
        start_secs: f64,
    ) -> Result<usize, InstrumentError> {
        if !self.characters.contains_key(&id) {
            return Err(InstrumentError::UnknownCharacter(id));
        }
        self.check_room(&[(id, Self::events_needed(performance), true)])?;
        Ok(self.play_checked(id, performance, instrument, start_secs))
    }

    /// [`Self::play`] for a known character after [`Self::check_room`].
    pub(super) fn play_checked(
        &mut self,
        id: CharacterId,
        performance: &Performance,
        instrument: u32,
        start_secs: f64,
    ) -> usize {
        if self.is_performing(id) {
            self.cancel(id, 0.0, f64::NEG_INFINITY);
        }
        let start = if start_secs.is_finite() {
            start_secs
        } else {
            0.0
        };
        let mut queued = 0usize;
        let mut last = 0.0f64;
        let mut has_end = false;
        for e in &performance.events {
            has_end |= e.kind == NoteEventKind::End;
            if e.time.is_finite() {
                last = last.max(e.time);
            }
            let (kind, packed) = match e.kind {
                NoteEventKind::On => (
                    MusicEventKind::NoteOn,
                    pack_note(self.table.pitch_key(e.pitch), u32::from(e.volume)),
                ),
                NoteEventKind::Off => (
                    MusicEventKind::NoteOff,
                    pack_note(self.table.pitch_key(e.pitch), u32::from(e.volume)),
                ),
                NoteEventKind::End => (MusicEventKind::End, 0),
            };
            let seq = self.fresh_seq();
            self.queue.push(MusicEvent {
                time: start + e.time,
                seq,
                kind,
                character: id,
                instrument,
                packed,
                quantized: false,
                from_song: true,
            });
            queued += 1;
        }
        if !has_end {
            let seq = self.fresh_seq();
            self.queue.push(MusicEvent {
                time: start + last,
                seq,
                kind: MusicEventKind::End,
                character: id,
                instrument,
                packed: 0,
                quantized: false,
                from_song: true,
            });
            queued += 1;
        }
        if let Some(c) = self.characters.get_mut(&id) {
            c.performing = true;
        }
        self.stats.performances += 1;
        queued
    }

    /// `/play` without `sync`: [`Self::play`] starting [`SOLO_START_OFFSET`] (0.7 s) after
    /// the engine's clock (`abc-dialect.md` §8.5). Returns the start time.
    ///
    /// # Errors
    ///
    /// As [`Self::play`].
    pub fn play_solo(
        &mut self,
        engine: &AudioEngine,
        id: CharacterId,
        performance: &Performance,
        instrument: u32,
    ) -> Result<f64, InstrumentError> {
        let start = Self::now(engine) + SOLO_START_OFFSET;
        self.play(id, performance, instrument, start)?;
        Ok(start)
    }

    /// Cancels the character's music (§8.5): drops its queued events, then queues a
    /// note-off due immediately for every registered note key, so sustained notes end with
    /// their release ramp. The stop command calls this (Decision, Q1).
    ///
    /// Notes the player has already handed to the engine ahead of their time (look-ahead,
    /// [`PlayerConfig::lookahead_secs`]) count as still queued: the next
    /// [`Self::update`] withdraws every one that has not started by then, fire-and-forget
    /// ones included, so nothing due after that update sounds. Use [`Self::stop_now`] to
    /// withdraw them at once.
    pub fn stop(&mut self, id: CharacterId) {
        self.cancel(id, 0.0, f64::NEG_INFINITY);
    }

    /// [`Self::stop`], withdrawing the notes handed ahead that have not started at once.
    pub fn stop_now(&mut self, engine: &mut AudioEngine, id: CharacterId) {
        self.cancel(id, 0.0, f64::NEG_INFINITY);
        self.flush_revoked(engine, Self::now(engine));
    }

    /// §8.5 with the note-offs due at `time`; voices handed ahead with an event time after
    /// `withdraw_after` are withdrawn (their note-ons count as still queued).
    fn cancel(&mut self, id: CharacterId, time: f64, withdraw_after: f64) {
        self.queue.remove_character(id);
        let Some(c) = self.characters.get_mut(&id) else {
            return;
        };
        c.performing = false;
        let revoked = &mut self.revoked;
        c.ahead.retain(|&(h, t)| {
            if t > withdraw_after {
                revoked.push((h, t));
                false
            } else {
                true
            }
        });
        let mut keys: Vec<u8> = c.registry.keys().copied().collect();
        keys.sort_unstable();
        self.stats.cancels += 1;
        for k in keys {
            let seq = self.fresh_seq();
            self.queue.push(MusicEvent {
                time,
                seq,
                kind: MusicEventKind::NoteOff,
                character: id,
                instrument: 0,
                packed: u32::from(k),
                quantized: false,
                from_song: false,
            });
        }
    }

    /// A live note (`instruments.md` §5.1 I41, §12 sandbox Decision): a note-on (`on`) or
    /// note-off of note key `key` at audio-clock time `time`, velocity 90, from-song flag 0,
    /// the quantized flag as given (the quantize option, with no delay to a grid). Ignored
    /// (returns `Ok(false)`) while the character has a song performance.
    ///
    /// # Errors
    ///
    /// [`InstrumentError::UnknownCharacter`]; [`InstrumentError::QueueFull`].
    pub fn live_note(
        &mut self,
        id: CharacterId,
        instrument: u32,
        key: u32,
        on: bool,
        quantized: bool,
        time: f64,
    ) -> Result<bool, InstrumentError> {
        let Some(c) = self.characters.get(&id) else {
            return Err(InstrumentError::UnknownCharacter(id));
        };
        if c.performing {
            return Ok(false);
        }
        self.queue_event(MusicEvent {
            time,
            seq: 0,
            kind: if on {
                MusicEventKind::NoteOn
            } else {
                MusicEventKind::NoteOff
            },
            character: id,
            instrument,
            packed: pack_note(key, LIVE_VELOCITY),
            quantized,
            from_song: false,
        })?;
        Ok(true)
    }

    /// Processes the queue (§8.3) against the engine's audio clock: every event due up to
    /// clock + [`PlayerConfig::lookahead_secs`] is processed in queue order and handed to
    /// the engine with its exact start or stop frame (§12 Decision 2); note-ons 0.5 s or
    /// more late are dropped. At most [`PlayerConfig::max_events_per_update`] events are
    /// taken per call. Notes handed ahead whose note-on a cancel withdrew are stopped first
    /// unless they have started. Call once per frame, before [`AudioEngine::update`].
    pub fn update(&mut self, engine: &mut AudioEngine, samples: &mut dyn SampleProvider) {
        let rate = engine.sample_rate();
        let now = Self::now(engine);
        let ahead = if self.config.lookahead_secs.is_finite() {
            self.config.lookahead_secs.max(0.0)
        } else {
            0.0
        };
        self.flush_revoked(engine, now);
        self.prune(engine, now);
        let mut budget = self.config.max_events_per_update;
        while budget > 0
            && let Some(due) = self.queue.pop_due(now, now + ahead)
        {
            budget -= 1;
            let ev = match due {
                Due::Late(_) => {
                    self.stats.late_dropped += 1;
                    continue;
                }
                Due::Ready(ev) => ev,
            };
            let at = (ev.time > now).then(|| AudioTime(0).after_secs(ev.time, rate));
            match ev.kind {
                MusicEventKind::NoteOn => self.note_on(engine, samples, &ev, at),
                MusicEventKind::NoteOff => self.note_off(engine, &ev, at),
                MusicEventKind::End => {
                    if self.is_performing(ev.character) {
                        // The performance ended (§8.5 Decision): cancel, with the
                        // note-offs due at the end.
                        self.cancel(ev.character, ev.time, ev.time);
                    }
                }
            }
        }
        self.flush_revoked(engine, now);
    }

    /// Stops the withdrawn voices that have not started by `now` (they make no sound).
    fn flush_revoked(&mut self, engine: &mut AudioEngine, now: f64) {
        for (h, t) in self.revoked.drain(..) {
            if t > now {
                engine.stop(h);
            }
        }
    }

    /// Forgets finished instances and voices, and voices handed ahead that have started.
    fn prune(&mut self, engine: &AudioEngine, now: f64) {
        self.instances.retain(|_, i| alive(engine, &i.voices));
        let instances = &self.instances;
        for c in self.characters.values_mut() {
            c.instances.retain(|i| instances.contains_key(i));
            c.voices.retain(|&h| engine.voice_state(h).is_some());
            c.ahead
                .retain(|&(h, t)| t > now && engine.voice_state(h).is_some());
        }
    }

    /// §8.3 step 3, §8.4, §8.6, §5.2–§5.4.
    fn note_on(
        &mut self,
        engine: &mut AudioEngine,
        samples: &mut dyn SampleProvider,
        ev: &MusicEvent,
        at: Option<AudioTime>,
    ) {
        let script = MusicScript::for_note(ev.quantized);
        let key = ev.key();
        let instance = self.next_instance;
        self.next_instance = self.next_instance.wrapping_add(1);
        let limit = self.table.max_instances(script) as usize;
        let Some(c) = self.characters.get_mut(&ev.character) else {
            self.stats.no_character += 1;
            return;
        };
        self.stats.note_ons += 1;
        // The entry is added before the run (so a refused run leaves an entry naming no
        // instance); an existing entry is kept (§8.4).
        match c.registry.entry(key) {
            std::collections::hash_map::Entry::Occupied(_) => self.stats.unregistered += 1,
            std::collections::hash_map::Entry::Vacant(v) => {
                v.insert(instance);
            }
        }
        let instances = &mut self.instances;
        c.instances.retain(|i| match instances.get(i) {
            Some(inst) if alive(engine, &inst.voices) => true,
            Some(_) => {
                instances.remove(i);
                false
            }
            None => false,
        });
        let active = c
            .instances
            .iter()
            .filter(|i| instances.get(i).is_some_and(|x| x.script == script))
            .count();
        if active >= limit {
            self.stats.limit_refused += 1;
            tracing::debug!(character = %ev.character, limit, "music script run refused by the instance limit");
            return;
        }
        let position = c.position.clone();
        let table = Arc::clone(&self.table);
        let volume = note_volume(ev.packed);
        let mut tracked = Vec::new();
        let mut started = Vec::new();
        let mut any = false;
        // One set of draws per note-on: shared ancestors run or not, and have one
        // intensity, for every Sound node below them (§10.1 step 3).
        let mut draws = NoteDraws::new();
        for s in table.lookup(script, ev.from_song, ev.quantized, ev.instrument, key) {
            any = true;
            if let Some((h, is_tracked)) = self.start_sound(
                engine,
                samples,
                (&table, &position, s),
                volume,
                at,
                &mut draws,
            ) {
                started.push(h);
                if is_tracked {
                    tracked.push(h);
                }
            }
        }
        if !any {
            self.stats.silent_notes += 1;
        }
        let Some(c) = self.characters.get_mut(&ev.character) else {
            return;
        };
        if at.is_some() {
            c.ahead.extend(started.iter().map(|&h| (h, ev.time)));
        }
        c.voices.extend(started);
        if !tracked.is_empty() {
            c.instances.push(instance);
            self.instances.insert(
                instance,
                Instance {
                    script,
                    voices: tracked,
                },
            );
        }
    }

    /// §8.3 step 4: remove the registry entry and stop its instance with the release ramp
    /// (§9.2: requested ramp 0, so the fade lasts `RampOutTime`).
    fn note_off(&mut self, engine: &mut AudioEngine, ev: &MusicEvent, at: Option<AudioTime>) {
        let Some(c) = self.characters.get_mut(&ev.character) else {
            self.stats.no_character += 1;
            return;
        };
        self.stats.note_offs += 1;
        let Some(instance) = c.registry.remove(&ev.key()) else {
            return;
        };
        c.instances.retain(|&i| i != instance);
        if let Some(inst) = self.instances.remove(&instance) {
            for h in inst.voices {
                match at {
                    Some(t) => engine.stop_at(h, t, 0.0),
                    None => engine.stop(h),
                };
            }
        }
    }

    /// Starts one Sound node (§9.1, §7.3, §10). Returns the accepted handle and whether
    /// it is tracked.
    fn start_sound(
        &mut self,
        engine: &mut AudioEngine,
        samples: &mut dyn SampleProvider,
        (table, position, s): (&InstrumentTable, &LivePosition, &NoteSound),
        note_volume: f32,
        at: Option<AudioTime>,
        draws: &mut NoteDraws,
    ) -> Option<(VoiceHandle, bool)> {
        if s.sound_info.is_null() {
            self.stats.null_sound += 1;
            return None;
        }
        let Some(info) = table.sound_info(s.sound_info) else {
            self.stats.missing_sound_info += 1;
            return None;
        };
        let Some(volume) = s.volume_shared(note_volume, &mut self.rng, draws) else {
            self.stats.probability_skipped += 1;
            return None;
        };
        if s.fire_and_forget && volume <= FIRE_AND_FORGET_FLOOR {
            self.stats.below_floor += 1;
            return None;
        }
        let pitch = pitch_multiplier(s.pitch, s.pitch_variance, &mut self.rng);
        let offset = self.offset(s);
        let Some(source) = samples.source(info.payload) else {
            self.stats.missing_sample += 1;
            tracing::debug!(payload = %info.payload, "instrument sample not available");
            return None;
        };
        let positioner = if s.use_camera_position {
            Positioner::Listener {
                offset,
                orientation: Some(position.clone()),
            }
        } else if s.move_with_entity {
            Positioner::Live {
                anchor: position.clone(),
                offset,
            }
        } else {
            match position.get() {
                LiveState::At(p) => Positioner::frozen(p, position.rotation(), offset),
                _ => Positioner::Live {
                    anchor: position.clone(),
                    offset,
                },
            }
        };
        let mut req = SoundRequest::new(source)
            .sound_id(info.did.0)
            .volume(volume)
            .pitch(pitch)
            .looping(info.is_loop())
            .channel(Channel::new_lossy(s.channel))
            .positional(
                PositionalClass::from_sound_info_flags(info.flags.bits()),
                info.min_distance,
                info.max_distance,
            )
            .priority(info.priority)
            .at(positioner)
            .ramp_out(s.ramp_out)
            .tracked(!s.fire_and_forget);
        if let Some(t) = at {
            req = req.start_at(t);
        }
        match engine.request(req) {
            RequestOutcome::Started(h)
            | RequestOutcome::Pending(h)
            | RequestOutcome::Loading(h) => {
                self.stats.voices_accepted += 1;
                Some((h, !s.fire_and_forget))
            }
            RequestOutcome::Skipped(_) => {
                self.stats.voices_skipped += 1;
                None
            }
            RequestOutcome::Suppressed(_) => {
                self.stats.voices_suppressed += 1;
                None
            }
            RequestOutcome::Refused(r) => {
                self.stats.voices_refused += 1;
                tracing::trace!(reason = %r, "instrument note refused");
                None
            }
        }
    }

    /// The random offset of `sound.md` §9.2 step 5 plus `Offset` (instrument nodes have
    /// both random distances 0, §6.1).
    fn offset(&mut self, s: &NoteSound) -> [f32; 3] {
        use rand::Rng;
        let (min, max) = s.random_distance;
        if max <= 0.0 {
            return s.offset;
        }
        let d = [
            self.rng.random_range(-1.0f32..=1.0),
            self.rng.random_range(-1.0f32..=1.0),
            self.rng.random_range(-1.0f32..=1.0),
        ];
        let len = (d[0] * d[0] + d[1] * d[1] + d[2] * d[2]).sqrt();
        let dir = if len > 1e-6 {
            d.map(|c| c / len)
        } else {
            [0.0, 0.0, 1.0]
        };
        let dist = if max > min {
            self.rng.random_range(min..=max)
        } else {
            max
        };
        [
            dir[0] * dist + s.offset[0],
            dir[1] * dist + s.offset[1],
            dir[2] * dist + s.offset[2],
        ]
    }
}
