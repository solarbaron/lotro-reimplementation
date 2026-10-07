// SPDX-License-Identifier: GPL-3.0-or-later
//! The script graph's effect sink on the audio engine: Sound nodes become engine requests
//! (`sound.md` §9.2 steps 5–12 via [`super::loader::request_from`]), stops ramp out (§6.5),
//! intensity changes become volume changes (§9.3), SoundEnvironment nodes register reverb
//! environments (§8, §9.4) and a removed entity's voices end (`script-runtime.md` §19
//! OPEN-12 Decision). Completions travel back through [`VoiceBook::completions`].

use std::collections::{HashMap, VecDeque};

use tessera_assets::CancelToken;
use tessera_audio::engine::{
    AudioEngine, Channel, ChannelFlags, CompletionReason, EnvironmentId, IDENTITY_ROTATION,
    LivePosition, LiveState, PositionalClass, Positioner, RequestOutcome, ReverbEnvironment,
    ReverbPreset, VoiceHandle, VoiceState, rotate_offset,
};
use tessera_formats::Did;
use tessera_scriptgraph::{
    EffectSink, EntityId, EnvironmentHandle, EnvironmentRequest, SoundHandle, SoundPositioner,
    SoundStart, SoundStartOutcome,
};

use super::loader::{SoundProvider, request_from};

/// Most frozen anchors (looping voices fixed where they started) kept per entity. Anchors
/// of voices the engine no longer knows are dropped first ([`prune_anchors`]); past the
/// bound the oldest anchor is removed, which ends its voice, so that no voice outlives
/// its entity.
pub const MAX_FROZEN_ANCHORS: usize = 64;
/// Most music events kept for diagnostics.
pub const MAX_MUSIC_EVENTS: usize = 4096;

/// The anchor of a looping voice frozen where it started (`MoveWithEntity` = 0): it never
/// moves, and is removed with its entity so that the voice ends then (`sound.md` §9.2
/// OPEN, Decision).
#[derive(Debug, Clone)]
pub struct FrozenAnchor {
    /// The voice placed at it.
    pub voice: VoiceHandle,
    /// The anchor.
    pub anchor: LivePosition,
}

/// An entity's live transform as the audio sees it.
#[derive(Debug, Clone)]
pub struct EntityAudio {
    /// Position and orientation, shared with every voice that follows the entity.
    pub live: LivePosition,
    /// Anchors of its live looping voices frozen where they started (removed with the
    /// entity).
    pub frozen: Vec<FrozenAnchor>,
}

/// Drops the frozen anchors whose voice the engine no longer knows (completed, refused
/// after loading, stopped).
pub fn prune_anchors(entities: &mut Entities, engine: &AudioEngine) {
    for e in entities.values_mut() {
        if !e.frozen.is_empty() {
            e.frozen.retain(|f| engine.voice_state(f.voice).is_some());
        }
    }
}

/// Keeps `anchor` of `voice` on `entity`: dead anchors are dropped first; at
/// [`MAX_FROZEN_ANCHORS`] the oldest is removed, ending its voice.
fn keep_anchor(
    entity: &mut EntityAudio,
    engine: &AudioEngine,
    voice: VoiceHandle,
    anchor: LivePosition,
) {
    entity
        .frozen
        .retain(|f| engine.voice_state(f.voice).is_some());
    if entity.frozen.len() >= MAX_FROZEN_ANCHORS {
        let oldest = entity.frozen.remove(0);
        tracing::debug!(voice = %oldest.voice, "audio: frozen-anchor bound; the oldest voice ends");
        oldest.anchor.remove();
    }
    entity.frozen.push(FrozenAnchor { voice, anchor });
}

/// The entity a request is placed by: the positioner's (`sound.md` §9.2 step 3, the
/// viewport entity of a `Script_RunInViewport` entity), not necessarily the one whose
/// script runs the node.
#[must_use]
pub fn placing_entity(p: &SoundPositioner) -> EntityId {
    match *p {
        SoundPositioner::FollowEntity { entity, .. }
        | SoundPositioner::Frozen { entity, .. }
        | SoundPositioner::Camera { entity, .. } => entity,
    }
}

/// `sound.md` §6.8 item 11 ahead of the payload load: whether a request is sure to be
/// refused as out of range, so that its decode is saved. Only a positional one-shot whose
/// place is known qualifies, and only when its channel is not exclusive: an exclusive
/// channel queues a first attempt before the range test (item 9). "Farther than" the
/// maximum distance, or a distance that is not a number, as the engine's gate.
#[must_use]
pub fn surely_out_of_range(
    class: PositionalClass,
    looping: bool,
    exclusive: bool,
    at: Option<[f32; 3]>,
    listener: [f32; 3],
    max_distance: f32,
) -> bool {
    if !class.is_positional() || looping || exclusive {
        return false;
    }
    at.is_some_and(|p| {
        let d = distance(p, listener);
        d.is_nan() || d > max_distance
    })
}

/// Entities by runtime ID.
pub type Entities = HashMap<EntityId, EntityAudio>;

/// One request the sink made.
#[derive(Debug, Clone)]
pub struct VoiceEntry {
    /// The engine's request number.
    pub voice: VoiceHandle,
    /// `SoundScript_Channel`.
    pub channel: u32,
    /// The sound info.
    pub sound: Did,
    /// Tracked (`FireAndForget` = 0).
    pub tracked: bool,
    /// Ramp-out time, seconds.
    pub ramp_out: f32,
    /// Cancels the payload load.
    pub token: CancelToken,
    /// Walker time of the request.
    pub requested_at: f64,
    /// Walker time it was first seen playing.
    pub playing_at: Option<f64>,
    /// Walker time its stop was asked.
    pub stopped_at: Option<f64>,
}

/// What happened to music, for the diagnostics of exit criterion 2.
#[derive(Debug, Clone, Copy, PartialEq)]
pub enum MusicEventKind {
    /// `Area_Music` changed on the avatar.
    AreaMusic {
        /// Old value.
        old: Option<u32>,
        /// New value.
        new: Option<u32>,
    },
    /// A music-channel request was made.
    TrackRequested {
        /// Sound info.
        sound: Did,
    },
    /// A music voice started playing (its payload arrived and it passed the gates).
    TrackPlaying {
        /// Sound info.
        sound: Did,
    },
    /// A playing music voice was stopped by its node: its ramp-out begins.
    RampOut {
        /// Sound info.
        sound: Did,
        /// Ramp-out seconds.
        secs: f32,
    },
    /// A music voice completed.
    TrackEnded {
        /// Sound info.
        sound: Did,
        /// Why.
        reason: CompletionReason,
    },
}

/// One music event: walker time, audio-clock time and what happened.
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct MusicEvent {
    /// Seconds since the audio started (the walker's frame times).
    pub t: f64,
    /// The engine's audio clock, seconds.
    pub audio_t: f64,
    /// What.
    pub kind: MusicEventKind,
}

/// The voices and environments the sink created, and the music log.
#[derive(Debug, Default)]
pub struct VoiceBook {
    /// Live requests by graph handle (= the engine's request number).
    pub voices: HashMap<SoundHandle, VoiceEntry>,
    /// Registered environments.
    pub environments: HashMap<EnvironmentHandle, EnvironmentId>,
    /// Last environment handle given out.
    pub next_environment: u64,
    /// Music log (bounded).
    pub music: VecDeque<MusicEvent>,
    /// The music channel's value (`SoundChannel` label "Music").
    pub music_channel: u32,
    /// Walker time now.
    pub now: f64,
    /// Audio clock now, seconds.
    pub audio_now: f64,
    /// Sound requests refused before reaching the engine (no info, no entity, no payload
    /// slot, out of range).
    pub refused_early: u64,
}

impl VoiceBook {
    /// Records a music event (oldest dropped past [`MAX_MUSIC_EVENTS`]) and logs it.
    pub fn log_music(&mut self, kind: MusicEventKind) {
        let e = MusicEvent {
            t: self.now,
            audio_t: self.audio_now,
            kind,
        };
        tracing::info!(t = e.t, audio_t = e.audio_t, event = ?e.kind, "audio: music");
        if self.music.len() >= MAX_MUSIC_EVENTS {
            self.music.pop_front();
        }
        self.music.push_back(e);
    }

    /// Takes the engine's completion events: the tracked graph handles to complete, and
    /// logs music completions. Fire-and-forget voices that ended are forgotten by
    /// [`VoiceBook::prune`].
    pub fn completions(&mut self, engine: &mut AudioEngine) -> Vec<SoundHandle> {
        let events: Vec<_> = engine.drain_events().collect();
        let mut out = Vec::new();
        for ev in events {
            let tessera_audio::engine::AudioEvent::Completed { handle, reason } = ev;
            let h = SoundHandle(handle.raw());
            if let Some(v) = self.voices.remove(&h) {
                if v.channel == self.music_channel {
                    self.log_music(MusicEventKind::TrackEnded {
                        sound: v.sound,
                        reason,
                    });
                }
                if v.tracked {
                    out.push(h);
                }
            }
        }
        out
    }

    /// Forgets voices the engine no longer knows and notes music voices that started
    /// playing.
    pub fn prune(&mut self, engine: &AudioEngine) {
        let mut playing = Vec::new();
        self.voices
            .retain(|_, v| match engine.voice_state(v.voice) {
                None => false,
                Some(VoiceState::Playing) => {
                    if v.playing_at.is_none() {
                        v.playing_at = Some(self.now);
                        if v.channel == self.music_channel {
                            playing.push(v.sound);
                        }
                    }
                    true
                }
                Some(_) => true,
            });
        for sound in playing {
            self.log_music(MusicEventKind::TrackPlaying { sound });
        }
    }

    /// Live voices on `channel` that play (not loading, pending or virtualised).
    #[must_use]
    pub fn playing_on(&self, engine: &AudioEngine, channel: u32) -> usize {
        self.voices
            .values()
            .filter(|v| {
                v.channel == channel
                    && matches!(
                        engine.voice_state(v.voice),
                        Some(VoiceState::Playing | VoiceState::Stopping)
                    )
            })
            .count()
    }
}

/// The sink (module docs). Built per runtime call from disjoint parts of the world audio.
#[allow(missing_debug_implementations)]
pub struct EngineSink<'a> {
    /// The engine.
    pub engine: &'a mut AudioEngine,
    /// Sound infos and payloads.
    pub loader: &'a dyn SoundProvider,
    /// Entities' live transforms.
    pub entities: &'a mut Entities,
    /// Voices, environments, music log.
    pub book: &'a mut VoiceBook,
}

fn add(a: [f32; 3], b: [f32; 3]) -> [f32; 3] {
    [a[0] + b[0], a[1] + b[1], a[2] + b[2]]
}

fn distance(a: [f32; 3], b: [f32; 3]) -> f32 {
    let d = [a[0] - b[0], a[1] - b[1], a[2] - b[2]];
    (d[0] * d[0] + d[1] * d[1] + d[2] * d[2]).sqrt()
}

impl EngineSink<'_> {
    /// The positioner of a request, where its source is when known now, and the anchor of
    /// a looping frozen voice (kept only once the engine accepts the request).
    fn positioner(
        entity: &EntityAudio,
        p: &SoundPositioner,
        looping: bool,
        listener: [f32; 3],
    ) -> (Positioner, Option<[f32; 3]>, Option<LivePosition>) {
        let state = entity.live.get();
        let rot = entity.live.rotation();
        match *p {
            SoundPositioner::FollowEntity { offset, .. } => {
                let at = match state {
                    LiveState::At(pos) => Some(add(pos, rotate_offset(rot, offset))),
                    _ => None,
                };
                (
                    Positioner::Live {
                        anchor: entity.live.clone(),
                        offset,
                    },
                    at,
                    None,
                )
            }
            SoundPositioner::Frozen { offset, .. } => match state {
                LiveState::At(pos) => {
                    let at = add(pos, rotate_offset(rot, offset));
                    if looping {
                        // A looping voice must still end with its entity (sound.md §9.2
                        // OPEN, Decision): it follows an anchor that never moves.
                        let anchor = LivePosition::with_rotation(at, IDENTITY_ROTATION);
                        (Positioner::live(anchor.clone()), Some(at), Some(anchor))
                    } else {
                        (Positioner::Fixed(at), Some(at), None)
                    }
                }
                // Unknown position: silent until it is known.
                _ => (Positioner::live(entity.live.clone()), None, None),
            },
            SoundPositioner::Camera { offset, .. } => (
                Positioner::Listener {
                    offset,
                    orientation: Some(entity.live.clone()),
                },
                Some(listener),
                None,
            ),
        }
    }
}

impl EffectSink for EngineSink<'_> {
    fn start_sound(&mut self, request: &SoundStart) -> SoundStartOutcome {
        // §9.2 step 1: no sound info, or the entity is gone: the start fails (completes).
        let Some(info) = self.loader.info(request.sound) else {
            self.book.refused_early += 1;
            return SoundStartOutcome::Refused;
        };
        let listener = self.engine.listener().position;
        // §9.2 step 3: the positioner names the entity the sound belongs to from here on
        // (the viewport entity for a Script_RunInViewport entity).
        let target = placing_entity(&request.positioner);
        let Some(entity) = self.entities.get(&target) else {
            self.book.refused_early += 1;
            return SoundStartOutcome::Refused;
        };
        let looping = info.is_loop();
        let (positioner, at, anchor) =
            Self::positioner(entity, &request.positioner, looping, listener);
        let class = PositionalClass::from_sound_info_flags(info.flags.bits());
        let exclusive = self
            .engine
            .channel_flags(Channel::new_lossy(request.channel).bookkeeping())
            .has(ChannelFlags::EXCLUSIVE);
        if surely_out_of_range(class, looping, exclusive, at, listener, info.max_distance) {
            self.book.refused_early += 1;
            return SoundStartOutcome::Refused;
        }
        let token = CancelToken::new();
        let source = match self.loader.source(info.payload, token.clone()) {
            Ok(s) => s,
            Err(e) => {
                tracing::debug!(sound = %request.sound, error = %e, "sound not loaded");
                self.book.refused_early += 1;
                return SoundStartOutcome::Refused;
            }
        };
        let req = request_from(&info, request, positioner, source);
        let outcome = self.engine.request(req);
        match outcome {
            RequestOutcome::Started(v)
            | RequestOutcome::Pending(v)
            | RequestOutcome::Loading(v) => {
                let h = SoundHandle(v.raw());
                if let Some(anchor) = anchor
                    && let Some(entity) = self.entities.get_mut(&target)
                {
                    keep_anchor(entity, self.engine, v, anchor);
                }
                if request.channel == self.book.music_channel {
                    self.book.log_music(MusicEventKind::TrackRequested {
                        sound: request.sound,
                    });
                }
                self.book.voices.insert(
                    h,
                    VoiceEntry {
                        voice: v,
                        channel: request.channel,
                        sound: request.sound,
                        tracked: request.tracked,
                        ramp_out: request.ramp_out,
                        token,
                        requested_at: self.book.now,
                        playing_at: None,
                        stopped_at: None,
                    },
                );
                SoundStartOutcome::Accepted(h)
            }
            RequestOutcome::Skipped(_) | RequestOutcome::Suppressed(_) => {
                token.cancel();
                SoundStartOutcome::AcceptedSilent
            }
            RequestOutcome::Refused(_) => {
                token.cancel();
                SoundStartOutcome::Refused
            }
        }
    }

    fn stop_sound(&mut self, handle: SoundHandle) {
        let now = self.book.now;
        let music = self.book.music_channel;
        let Some(v) = self.book.voices.get_mut(&handle) else {
            return;
        };
        v.token.cancel();
        if v.stopped_at.is_none() {
            v.stopped_at = Some(now);
            let (sound, secs, channel) = (v.sound, v.ramp_out, v.channel);
            let playing = v.playing_at.is_some()
                || self.engine.voice_state(v.voice) == Some(VoiceState::Playing);
            self.engine.stop(v.voice);
            if channel == music && playing {
                self.book.log_music(MusicEventKind::RampOut {
                    sound,
                    secs: secs.max(0.0),
                });
            }
        }
    }

    fn set_sound_volume(&mut self, handle: SoundHandle, volume: f32) {
        if let Some(v) = self.book.voices.get(&handle) {
            self.engine.set_volume(v.voice, volume);
        }
    }

    fn register_environment(&mut self, request: &EnvironmentRequest) -> EnvironmentHandle {
        self.book.next_environment = self.book.next_environment.wrapping_add(1);
        let h = EnvironmentHandle(self.book.next_environment);
        if let Some(id) = self.engine.register_environment(ReverbEnvironment {
            preset: ReverbPreset::from_value(request.name),
            priority: request.priority,
            intensity: request.intensity,
        }) {
            self.book.environments.insert(h, id);
        }
        h
    }

    fn unregister_environment(&mut self, handle: EnvironmentHandle) {
        if let Some(id) = self.book.environments.remove(&handle) {
            self.engine.unregister_environment(id);
        }
    }

    fn set_environment_intensity(&mut self, handle: EnvironmentHandle, intensity: f32) {
        if let Some(id) = self.book.environments.get(&handle) {
            self.engine.set_environment_intensity(*id, intensity);
        }
    }

    fn entity_removed(&mut self, entity: EntityId) {
        if let Some(e) = self.entities.remove(&entity) {
            e.live.remove();
            for f in e.frozen {
                f.anchor.remove();
            }
        }
    }
}

#[cfg(test)]
mod tests {
    use std::sync::Arc;

    use tessera_audio::engine::{Backend, EngineConfig, Listener, SoundSource};
    use tessera_audio::pcm::Pcm;
    use tessera_formats::{SoundInfo, SoundInfoFlags};

    use super::*;

    const AVATAR: EntityId = EntityId(1);
    const EMITTER: EntityId = EntityId(0x0101_0000_0000_0007);

    struct Fake {
        flags: u32,
        fail_source: bool,
    }

    impl SoundProvider for Fake {
        fn info(&self, did: Did) -> Option<Arc<SoundInfo>> {
            Some(Arc::new(SoundInfo {
                did,
                flags: SoundInfoFlags(self.flags),
                unused_a: -1.0,
                unused_b: -1.0,
                priority: 0.5,
                min_distance: 2.0,
                max_distance: 40.0,
                payload: Did(0x0A00_0001),
                name: String::new(),
            }))
        }

        fn source(&self, _: Did, _: CancelToken) -> Result<SoundSource, String> {
            if self.fail_source {
                return Err("251 loads in flight".into());
            }
            Ok(SoundSource::Pcm(Pcm {
                sample_rate: 8000,
                channels: 1,
                samples: Arc::from(vec![0.0f32; 8000]),
            }))
        }
    }

    fn engine() -> AudioEngine {
        let mut e = AudioEngine::new(EngineConfig {
            backend: Backend::Null {
                sample_rate: 48_000,
            },
            ..EngineConfig::default()
        })
        .unwrap();
        e.set_listener(Listener {
            position: [0.0; 3],
            right: [1.0, 0.0, 0.0],
            up: [0.0, 0.0, 1.0],
            forward: [0.0, 1.0, 0.0],
        });
        e
    }

    fn entity(pos: [f32; 3]) -> EntityAudio {
        EntityAudio {
            live: LivePosition::with_rotation(pos, IDENTITY_ROTATION),
            frozen: Vec::new(),
        }
    }

    fn start(channel: u32, positioner: SoundPositioner) -> SoundStart {
        SoundStart {
            entity: EMITTER,
            sound: Did(0x2A00_0001),
            volume: 1.0,
            pitch: 1.0,
            channel,
            ramp_out: 0.0,
            tracked: false,
            positioner,
            node_id: 1,
        }
    }

    fn frozen(entity: EntityId) -> SoundPositioner {
        SoundPositioner::Frozen {
            entity,
            offset: [0.0; 3],
        }
    }

    struct Rig {
        engine: AudioEngine,
        entities: Entities,
        book: VoiceBook,
    }

    impl Rig {
        fn new() -> Self {
            Self {
                engine: engine(),
                entities: Entities::new(),
                book: VoiceBook::default(),
            }
        }

        fn start(&mut self, provider: &Fake, request: &SoundStart) -> SoundStartOutcome {
            let mut sink = EngineSink {
                engine: &mut self.engine,
                loader: provider,
                entities: &mut self.entities,
                book: &mut self.book,
            };
            sink.start_sound(request)
        }
    }

    #[test]
    fn the_positioners_entity_places_the_sound() {
        // A Script_RunInViewport emitter: the graph names the avatar in the positioner.
        let mut r = Rig::new();
        r.entities.insert(AVATAR, entity([5.0, 0.0, 0.0]));
        let fake = Fake {
            flags: 0x04,
            fail_source: false,
        };
        let follow = SoundPositioner::FollowEntity {
            entity: AVATAR,
            offset: [0.0; 3],
        };
        assert!(matches!(
            r.start(&fake, &start(8, follow)),
            SoundStartOutcome::Accepted(_)
        ));
        // The running entity (absent here) does not matter; a missing placing entity does.
        let gone = SoundPositioner::FollowEntity {
            entity: EMITTER,
            offset: [0.0; 3],
        };
        assert_eq!(r.start(&fake, &start(8, gone)), SoundStartOutcome::Refused);
        assert_eq!(r.book.refused_early, 1);
        // A looping frozen sound of the viewport entity is anchored on it.
        let looping = Fake {
            flags: 0x05,
            fail_source: false,
        };
        assert!(matches!(
            r.start(&looping, &start(7, frozen(AVATAR))),
            SoundStartOutcome::Accepted(_)
        ));
        assert_eq!(r.entities[&AVATAR].frozen.len(), 1);
    }

    #[test]
    fn the_early_range_refusal_follows_the_gates() {
        let mut r = Rig::new();
        r.entities.insert(AVATAR, entity([100.0, 0.0, 0.0]));
        let one_shot = Fake {
            flags: 0x04,
            fail_source: false,
        };
        // SFX, 100 m > 40 m: refused before the load.
        assert_eq!(
            r.start(&one_shot, &start(8, frozen(AVATAR))),
            SoundStartOutcome::Refused
        );
        assert_eq!(r.book.refused_early, 1);
        // VO is exclusive: the first attempt queues before the range test (§6.8 item 9).
        let vo = Channel::VO.value();
        match r.start(&one_shot, &start(vo, frozen(AVATAR))) {
            SoundStartOutcome::Accepted(h) => {
                let v = r.book.voices[&h].voice;
                assert_eq!(r.engine.voice_state(v), Some(VoiceState::Pending));
            }
            other => panic!("VO one-shot: {other:?}"),
        }
        assert_eq!(r.book.refused_early, 1);
        // Exactly at the maximum distance is not "farther than" it.
        r.entities.insert(AVATAR, entity([40.0, 0.0, 0.0]));
        assert!(matches!(
            r.start(&one_shot, &start(8, frozen(AVATAR))),
            SoundStartOutcome::Accepted(_)
        ));
        assert_eq!(r.book.refused_early, 1);
        // Attenuated (0x40) counts as positional; a non-positional one never is refused.
        let p = PositionalClass::from_sound_info_flags(0x40);
        assert!(surely_out_of_range(
            p,
            false,
            false,
            Some([50.0, 0.0, 0.0]),
            [0.0; 3],
            40.0
        ));
        let n = PositionalClass::from_sound_info_flags(0x00);
        assert!(!surely_out_of_range(
            n,
            false,
            false,
            Some([50.0, 0.0, 0.0]),
            [0.0; 3],
            40.0
        ));
        assert!(!surely_out_of_range(
            p,
            true,
            false,
            Some([50.0, 0.0, 0.0]),
            [0.0; 3],
            40.0
        ));
        assert!(!surely_out_of_range(p, false, false, None, [0.0; 3], 40.0));
        assert!(surely_out_of_range(
            p,
            false,
            false,
            Some([f32::NAN; 3]),
            [0.0; 3],
            40.0
        ));
    }

    #[test]
    fn frozen_anchors_are_kept_only_for_accepted_voices_and_never_dropped_silently() {
        let mut r = Rig::new();
        r.entities.insert(EMITTER, entity([3.0, 0.0, 0.0]));
        let looping = Fake {
            flags: 0x05,
            fail_source: false,
        };
        let refused = Fake {
            flags: 0x05,
            fail_source: true,
        };
        // A request that never reaches the engine leaves no anchor.
        assert_eq!(
            r.start(&refused, &start(7, frozen(EMITTER))),
            SoundStartOutcome::Refused
        );
        assert!(r.entities[&EMITTER].frozen.is_empty());
        // Accepted loops are anchored, up to the bound; past it the oldest is removed so
        // that its voice ends.
        for _ in 0..MAX_FROZEN_ANCHORS {
            assert!(matches!(
                r.start(&looping, &start(7, frozen(EMITTER))),
                SoundStartOutcome::Accepted(_)
            ));
        }
        let first = r.entities[&EMITTER].frozen[0].anchor.clone();
        assert_eq!(r.entities[&EMITTER].frozen.len(), MAX_FROZEN_ANCHORS);
        assert!(matches!(
            r.start(&looping, &start(7, frozen(EMITTER))),
            SoundStartOutcome::Accepted(_)
        ));
        assert_eq!(r.entities[&EMITTER].frozen.len(), MAX_FROZEN_ANCHORS);
        assert_eq!(first.get(), LiveState::Removed);
        // Voices the engine no longer knows (here: every voice, after a reopen) lose their
        // anchors.
        r.engine.reopen().unwrap();
        prune_anchors(&mut r.entities, &r.engine);
        assert!(r.entities[&EMITTER].frozen.is_empty());
    }

    #[test]
    fn a_removed_entity_removes_its_anchors() {
        let mut r = Rig::new();
        r.entities.insert(EMITTER, entity([3.0, 0.0, 0.0]));
        let looping = Fake {
            flags: 0x05,
            fail_source: false,
        };
        r.start(&looping, &start(7, frozen(EMITTER)));
        let a = r.entities[&EMITTER].frozen[0].anchor.clone();
        let fake = Fake {
            flags: 0,
            fail_source: false,
        };
        let mut sink = EngineSink {
            engine: &mut r.engine,
            loader: &fake,
            entities: &mut r.entities,
            book: &mut r.book,
        };
        sink.entity_removed(EMITTER);
        assert_eq!(a.get(), LiveState::Removed);
        assert!(!r.entities.contains_key(&EMITTER));
    }
}
