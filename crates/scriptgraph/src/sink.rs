// SPDX-License-Identifier: GPL-3.0-or-later
//! Effects: what running graphs ask the rest of the client to do.
//!
//! The runtime never plays audio itself. Sound and SoundEnvironment nodes call an
//! [`EffectSink`] (the client wires it to `tessera-audio`), and the caller feeds sound
//! completions back with [`crate::Runtime::sound_completed`] so that tracked Sound nodes
//! complete (`sound.md` §9.2 step 12).

use std::collections::BTreeMap;

use tessera_formats::script_table::{
    AnimationNode, BeamNode, ParticleScriptNode, PhysicsImpulseNode,
};
use tessera_formats::{Did, PropertyEntry, PropertySet, PropertyValue};

/// An entity, identified by the caller.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, PartialOrd, Ord)]
pub struct EntityId(pub u64);

/// A sound voice, numbered by the sink.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, PartialOrd, Ord)]
pub struct SoundHandle(pub u64);

/// A registered sound environment, numbered by the sink.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, PartialOrd, Ord)]
pub struct EnvironmentHandle(pub u64);

/// A started animation, numbered by the sink.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, PartialOrd, Ord)]
pub struct AnimationHandle(pub u64);

/// A started particle effect, numbered by the sink (`particle-runtime.md` §4: the request
/// handle).
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, PartialOrd, Ord)]
pub struct ParticleHandle(pub u64);

/// A Particle node that passed the start gates of `particle-runtime.md` §5.4 (an effect
/// DID, and for a finite effect an intensity above 0.0002): the sink takes the attached
/// path (§5.5 steps 2–6) or, with `node.drop_effect`, the dropped path (§5.6).
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct ParticleStart {
    /// The entity whose script runs the node.
    pub entity: EntityId,
    /// The target after `ParticleScript_RedirectProperty` (§5.5 step 1): `entity` unless
    /// the property names another entity. Always `entity` on the dropped path (§5.6 step 1).
    pub target: EntityId,
    /// The effect DID (`particles.md` §7.1.1).
    pub effect: Did,
    /// The node's fields.
    pub node: ParticleScriptNode,
    /// The node's classification (§5.3): the stored byte, else the sink's answer.
    pub never_ending: bool,
    /// The node's editor ID (for logs).
    pub node_id: u32,
}

/// A started beam, numbered by the sink (`particle-runtime.md` §11.1).
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, PartialOrd, Ord)]
pub struct BeamHandle(pub u64);

/// A Beam node asking for a beam (`particle-runtime.md` §11.1 step 1): the node resolved its
/// target (the first entity) or uses `TargetPosition`. The sink moves the beam's head and
/// calls [`crate::Runtime::beam_arrived`] once it has arrived (step 7) and
/// [`crate::Runtime::beam_finished`] when the beam is removed (step 10, rule 14).
#[derive(Debug, Clone, PartialEq)]
pub struct BeamStart {
    /// The entity whose script runs the node (the beam's source).
    pub entity: EntityId,
    /// The target entity (`None` with `UseTargetPosition` = 1 and no resolved target).
    pub target: Option<EntityId>,
    /// The node's fields.
    pub node: BeamNode,
    /// The node's intensity.
    pub intensity: f32,
    /// The node's editor ID (for logs).
    pub node_id: u32,
}

/// A StoryHook node ran (`script-table.md` §9.1): its type and the story of its run.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct StoryHookEvent {
    /// The entity whose script runs the node (the performer).
    pub entity: EntityId,
    /// `StoryHookScript_Type` (1 `Defend`, 2 `Attack` in the data).
    pub hook_type: u32,
    /// The run's `StorySystem_StoryID` parameter; `None` when the run belongs to no story
    /// (the hook then releases nothing).
    pub story: Option<i64>,
}

/// An Execute node calling its native callback (`script-table.md` §9.1).
#[derive(Debug, Clone, Copy)]
pub struct ExecuteRequest<'a> {
    /// The entity whose script runs the node.
    pub entity: EntityId,
    /// `ExecuteScript_Callback`: a key of the callback DID map (0x28000007).
    pub callback: u32,
    /// The node's `DefaultValue`.
    pub default_value: Option<&'a PropertyEntry>,
    /// The node's `CallProperties` (for example `SCP_GetUpcomingTargets_CombatResultType_Filter`).
    pub call_properties: &'a PropertySet,
    /// The run's `StorySystem_StoryID` parameter, when the run belongs to a story.
    pub story: Option<i64>,
    /// The node's editor ID (for logs).
    pub node_id: u32,
}

/// The answer to an [`ExecuteRequest`]: values written into the run's parameter set (the
/// callback's outputs and its result property). The runtime then fires the result ports
/// (type 5) whose payload (result PID, value) matches the written value, otherwise the
/// Default ports (`combat-visuals.md` §11 rule 8).
#[derive(Debug, Clone, Default, PartialEq)]
pub struct ExecuteReply {
    /// (PID, value) pairs written into the run parameters, in order (at most
    /// [`MAX_EXECUTE_OUTPUTS`] are kept).
    pub outputs: Vec<(u32, PropertyValue)>,
}

/// Most output values one Execute reply writes (hostile-input bound).
pub const MAX_EXECUTE_OUTPUTS: usize = 64;

/// The constant the client adds to an `IsJump` node's `AirTime` before multiplying by
/// `ExpectedGravity`, seconds (`collision.md` §11.10 item 2; BIN, decision 0012).
pub const JUMP_AIR_TIME_OFFSET: f32 = 0.2;

/// A PhysicsImpulse node's request to one target (`script-runtime.md` §30.6 step 3).
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct ImpulseRequest {
    /// The entity whose script runs the node.
    pub entity: EntityId,
    /// The entity the impulse acts on (the running entity unless a target names another).
    pub target: EntityId,
    /// The node's fields after its driven values (`script-runtime.md` §5).
    pub node: PhysicsImpulseNode,
    /// The vector: `Impulse` × the node's intensity, or for an `IsJump` node the vertical
    /// impulse (0, 0, (`AirTime` + [`JUMP_AIR_TIME_OFFSET`]) × `ExpectedGravity`), which the
    /// intensity does not scale. Non-finite components are 0.
    pub vector: [f32; 3],
    /// The scheduler time of the request, seconds (Tessera keeps no synchronisation
    /// offset, so this is the runtime's current time).
    pub time: f64,
    /// The node's editor ID (for logs).
    pub node_id: u32,
}

/// Where a sound's source is (`sound.md` §9.2 step 6). The offset (random offset plus
/// `SoundScript_Offset`, step 5) is in the entity frame, metres; the client rotates it by
/// the entity's orientation.
#[derive(Debug, Clone, Copy, PartialEq)]
pub enum SoundPositioner {
    /// `MoveWithEntity` = 1: follows the entity every frame.
    FollowEntity {
        /// The entity.
        entity: EntityId,
        /// Offset in the entity frame.
        offset: [f32; 3],
    },
    /// `MoveWithEntity` = 0: the entity's transform at the moment the sound starts, frozen.
    Frozen {
        /// The entity.
        entity: EntityId,
        /// Offset in the entity frame.
        offset: [f32; 3],
    },
    /// `UseCameraPosition` = 1: the live camera position, the offset turned by the entity's
    /// orientation.
    Camera {
        /// The entity whose orientation turns the offset.
        entity: EntityId,
        /// Offset in the entity frame.
        offset: [f32; 3],
    },
}

/// A sound request of a Sound node (`sound.md` §9.2 steps 5–10).
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct SoundStart {
    /// The entity whose script runs the node.
    pub entity: EntityId,
    /// The sound info (prefix 0x2A; never the null DID).
    pub sound: Did,
    /// Volume in [0, 1] (§9.2 step 7).
    pub volume: f32,
    /// Playback-rate multiplier (§6.3), at least 0.01.
    pub pitch: f32,
    /// `SoundScript_Channel`: the `SoundChannel` enum value (§7.1).
    pub channel: u32,
    /// `SoundScript_RampOutTime`, seconds, as stored (the sink treats negatives as 0).
    pub ramp_out: f32,
    /// `FireAndForget` = 0: the node waits for [`crate::Runtime::sound_completed`] and stops
    /// the voice (with its ramp-out) when it is stopped first.
    pub tracked: bool,
    /// Where the source is.
    pub positioner: SoundPositioner,
    /// The node's editor ID (for logs).
    pub node_id: u32,
}

/// What the sink did with a [`SoundStart`].
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum SoundStartOutcome {
    /// Accepted: a voice plays (or waits on the sink's pending list) under this handle. A
    /// tracked node stays running until the handle completes or the node stops.
    Accepted(SoundHandle),
    /// Accepted, but nothing will play and no completion will come (a request skipped by
    /// the voice limit or suppressed, `sound.md` §6.8 items 4 and 6). A tracked node
    /// completes at once (`sound.md` §6.6 OPEN Decision).
    AcceptedSilent,
    /// Refused (`sound.md` §6.8 items 1–3, 5): the node completes at once.
    Refused,
}

/// A SoundEnvironment node registering its environment (`sound.md` §8, §9.4).
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct EnvironmentRequest {
    /// The entity whose script runs the node.
    pub entity: EntityId,
    /// `SoundEnvironmentScript_Name`: the preset (enum labelled "SoundEnv").
    pub name: u32,
    /// `SoundEnvironmentScript_Priority`.
    pub priority: i32,
    /// The node's intensity.
    pub intensity: f32,
}

/// What the sink did with an animation node (`script-runtime.md` §18).
#[derive(Debug, Clone, Copy, PartialEq)]
pub enum AnimationOutcome {
    /// Nothing to wait for: the node completes at once (also the default for clients that
    /// do not animate, and for a missing clip).
    CompleteAtOnce,
    /// The node runs for this many seconds (the clip's played length ÷ rate) and then
    /// completes; `stop_animation` is called if it is stopped first.
    RunFor(AnimationHandle, f64),
    /// A looping clip: the node runs until it is stopped.
    RunUntilStopped(AnimationHandle),
}

/// The client side of a running graph. Every method is called synchronously from inside a
/// runtime call; none may call back into the runtime.
pub trait EffectSink {
    /// A Sound node asks for a sound (`sound.md` §9.2 step 10).
    fn start_sound(&mut self, request: &SoundStart) -> SoundStartOutcome;
    /// A tracked Sound node stopped before its voice completed: stop the voice with its
    /// ramp-out (`sound.md` §6.5).
    fn stop_sound(&mut self, handle: SoundHandle);
    /// A tracked Sound node's intensity changed (`script-runtime.md` §4.3): the new volume,
    /// in [0, 1].
    fn set_sound_volume(&mut self, handle: SoundHandle, volume: f32);
    /// A SoundEnvironment node started (`sound.md` §8).
    fn register_environment(&mut self, request: &EnvironmentRequest) -> EnvironmentHandle;
    /// A SoundEnvironment node stopped.
    fn unregister_environment(&mut self, handle: EnvironmentHandle);
    /// A running SoundEnvironment node's intensity changed. Default: ignored.
    fn set_environment_intensity(&mut self, handle: EnvironmentHandle, intensity: f32) {
        let _ = (handle, intensity);
    }
    /// An animation node starts (`script-runtime.md` §18). Default: completes at once.
    fn start_animation(
        &mut self,
        entity: EntityId,
        node: &AnimationNode,
        intensity: f32,
    ) -> AnimationOutcome {
        let _ = (entity, node, intensity);
        AnimationOutcome::CompleteAtOnce
    }
    /// A running animation node stopped. Default: ignored.
    fn stop_animation(&mut self, handle: AnimationHandle) {
        let _ = handle;
    }
    /// A driven field of a running animation node changed (`script-table.md` §6.10 step 7):
    /// `node` carries the node's fields with the current driven values; the clip takes the
    /// new rate (`Speed` × `SpeedMultiplier`) at once and blends to the new
    /// `UseBoneOpacity` (`skeleton-anim.md` §6.6). Default: ignored.
    fn update_animation(&mut self, handle: AnimationHandle, node: &AnimationNode) {
        let _ = (handle, node);
    }
    /// The never-ending classification of `effect` for a Particle node without a stored
    /// byte (`particle-runtime.md` §5.3, rule §12.2: only synthetic nodes). Default: `None`,
    /// treated as finite.
    fn classify_particle(&mut self, effect: Did) -> Option<bool> {
        let _ = effect;
        None
    }
    /// A Particle node starts an effect (`particle-runtime.md` §5.5, §5.6). The handle (or
    /// `None` when nothing started) is kept by a never-ending node's instance and given to
    /// [`EffectSink::stop_particle`] when the node stops; finite nodes complete at once and
    /// never stop their effect. Default: nothing starts.
    fn start_particle(&mut self, request: &ParticleStart) -> Option<ParticleHandle> {
        let _ = request;
        None
    }
    /// A never-ending Particle node stopped (`particle-runtime.md` §7.1): stop its effect,
    /// marking it for immediate disposal when `immediate` (`ParticleScript_ImmediateDisposal`).
    /// Default: ignored.
    fn stop_particle(&mut self, handle: ParticleHandle, immediate: bool) {
        let _ = (handle, immediate);
    }
    /// Whether `id` names an existing object outside the runtime's entities (the redirect
    /// of `particle-runtime.md` §5.5 step 1). Default: no.
    fn object_exists(&mut self, id: u64) -> bool {
        let _ = id;
        false
    }
    /// An entity was despawned after all its runs stopped: end the fire-and-forget voices
    /// it owns (`script-runtime.md` §19 OPEN-12 Decision). Default: ignored.
    fn entity_removed(&mut self, entity: EntityId) {
        let _ = entity;
    }
    /// A PhysicsImpulse node sends its impulse to a target (`script-runtime.md` §30.6 step
    /// 3); the node completes at once (§30.6 OPEN-20 Decision). Default: ignored.
    fn physics_impulse(&mut self, request: &ImpulseRequest) {
        let _ = request;
    }
    /// A StoryHook node ran (`script-table.md` §9.1; `combat-visuals.md` §4.2): the node
    /// completes at once. Default: ignored.
    fn story_hook(&mut self, event: &StoryHookEvent) {
        let _ = event;
    }
    /// An Execute node calls its callback (`script-table.md` §9.1). `None`: the callback is
    /// not implemented, and the node fires the result port matching its `DefaultValue`
    /// (`script-runtime.md` §18), or completes at once. Default: `None`.
    fn execute(&mut self, request: &ExecuteRequest<'_>) -> Option<ExecuteReply> {
        let _ = request;
        None
    }
    /// A Beam node asks for a beam (`particle-runtime.md` §11.1 step 1). `None`: no beam is
    /// made and the node completes at once (rule 14). Default: `None`.
    fn start_beam(&mut self, request: &BeamStart) -> Option<BeamHandle> {
        let _ = request;
        None
    }
    /// A Beam node with a beam stopped (§11.1 step 2: the node's stop stops its beam).
    /// Default: ignored.
    fn stop_beam(&mut self, handle: BeamHandle) {
        let _ = handle;
    }
}

/// A sink that does nothing: every sound is refused, environments get handle 0.
#[derive(Debug, Clone, Copy, Default)]
pub struct NullSink;

impl EffectSink for NullSink {
    fn start_sound(&mut self, _request: &SoundStart) -> SoundStartOutcome {
        SoundStartOutcome::Refused
    }
    fn stop_sound(&mut self, _handle: SoundHandle) {}
    fn set_sound_volume(&mut self, _handle: SoundHandle, _volume: f32) {}
    fn register_environment(&mut self, _request: &EnvironmentRequest) -> EnvironmentHandle {
        EnvironmentHandle(0)
    }
    fn unregister_environment(&mut self, _handle: EnvironmentHandle) {}
}

/// One call a [`RecordingSink`] received.
#[derive(Debug, Clone, PartialEq)]
pub enum SinkEvent {
    /// `start_sound`, with the outcome's handle (`None` when refused).
    SoundStarted(Option<SoundHandle>, SoundStart),
    /// `stop_sound`.
    SoundStopped(SoundHandle),
    /// `set_sound_volume`.
    SoundVolume(SoundHandle, f32),
    /// `register_environment`.
    EnvironmentRegistered(EnvironmentHandle, EnvironmentRequest),
    /// `unregister_environment`.
    EnvironmentUnregistered(EnvironmentHandle),
    /// `set_environment_intensity`.
    EnvironmentIntensity(EnvironmentHandle, f32),
    /// `entity_removed`.
    EntityRemoved(EntityId),
    /// `start_particle`, with the handle the sink gave.
    ParticleStarted(ParticleHandle, ParticleStart),
    /// `stop_particle`, with the immediate-disposal flag.
    ParticleStopped(ParticleHandle, bool),
    /// `physics_impulse`.
    Impulse(ImpulseRequest),
    /// `story_hook`.
    StoryHook(StoryHookEvent),
    /// `start_beam`, with the handle the sink gave.
    BeamStarted(BeamHandle, Box<BeamStart>),
    /// `stop_beam`.
    BeamStopped(BeamHandle),
}

/// A sink that accepts everything and records what it was asked, for tests and tools.
///
/// It keeps the tracked voices that are playing ([`RecordingSink::playing`]); events are
/// kept only up to [`RecordingSink::max_events`] (counters always count).
#[derive(Debug, Clone)]
pub struct RecordingSink {
    /// The calls, oldest first (bounded by `max_events`).
    pub events: Vec<SinkEvent>,
    /// Most events kept.
    pub max_events: usize,
    /// When set, every sound request is refused.
    pub refuse_sounds: bool,
    /// Tracked voices started and neither stopped nor completed.
    pub playing: BTreeMap<SoundHandle, SoundStart>,
    /// Environments registered and not unregistered.
    pub environments: BTreeMap<EnvironmentHandle, EnvironmentRequest>,
    /// Particle effects started and not stopped (finite ones are never stopped).
    pub particles: BTreeMap<ParticleHandle, ParticleStart>,
    /// Beams started and not stopped.
    pub beams: BTreeMap<BeamHandle, BeamStart>,
    /// Number of sound requests accepted.
    pub sounds_started: u64,
    /// Number of sound requests refused.
    pub sounds_refused: u64,
    /// Number of `stop_sound` calls.
    pub sounds_stopped: u64,
    next: u64,
}

impl Default for RecordingSink {
    fn default() -> Self {
        Self {
            events: Vec::new(),
            max_events: 100_000,
            refuse_sounds: false,
            playing: BTreeMap::new(),
            environments: BTreeMap::new(),
            particles: BTreeMap::new(),
            beams: BTreeMap::new(),
            sounds_started: 0,
            sounds_refused: 0,
            sounds_stopped: 0,
            next: 1,
        }
    }
}

impl RecordingSink {
    /// An empty sink.
    #[must_use]
    pub fn new() -> Self {
        Self::default()
    }

    /// Forgets a voice that completed (call it next to
    /// [`crate::Runtime::sound_completed`]).
    pub fn complete(&mut self, handle: SoundHandle) {
        self.playing.remove(&handle);
    }

    fn push(&mut self, e: SinkEvent) {
        if self.events.len() < self.max_events {
            self.events.push(e);
        }
    }

    fn handle(&mut self) -> u64 {
        let h = self.next;
        self.next = self.next.wrapping_add(1);
        h
    }

    /// The sound requests recorded (accepted or refused), oldest first.
    pub fn sound_starts(&self) -> impl Iterator<Item = (Option<SoundHandle>, &SoundStart)> {
        self.events.iter().filter_map(|e| match e {
            SinkEvent::SoundStarted(h, s) => Some((*h, s)),
            _ => None,
        })
    }

    /// The handles stopped, oldest first.
    pub fn sound_stops(&self) -> impl Iterator<Item = SoundHandle> + '_ {
        self.events.iter().filter_map(|e| match e {
            SinkEvent::SoundStopped(h) => Some(*h),
            _ => None,
        })
    }
}

impl EffectSink for RecordingSink {
    fn start_sound(&mut self, request: &SoundStart) -> SoundStartOutcome {
        if self.refuse_sounds {
            self.sounds_refused += 1;
            self.push(SinkEvent::SoundStarted(None, *request));
            return SoundStartOutcome::Refused;
        }
        let h = SoundHandle(self.handle());
        self.sounds_started += 1;
        if request.tracked {
            self.playing.insert(h, *request);
        }
        self.push(SinkEvent::SoundStarted(Some(h), *request));
        SoundStartOutcome::Accepted(h)
    }

    fn stop_sound(&mut self, handle: SoundHandle) {
        self.sounds_stopped += 1;
        self.playing.remove(&handle);
        self.push(SinkEvent::SoundStopped(handle));
    }

    fn set_sound_volume(&mut self, handle: SoundHandle, volume: f32) {
        if let Some(s) = self.playing.get_mut(&handle) {
            s.volume = volume;
        }
        self.push(SinkEvent::SoundVolume(handle, volume));
    }

    fn register_environment(&mut self, request: &EnvironmentRequest) -> EnvironmentHandle {
        let h = EnvironmentHandle(self.handle());
        self.environments.insert(h, *request);
        self.push(SinkEvent::EnvironmentRegistered(h, *request));
        h
    }

    fn unregister_environment(&mut self, handle: EnvironmentHandle) {
        self.environments.remove(&handle);
        self.push(SinkEvent::EnvironmentUnregistered(handle));
    }

    fn set_environment_intensity(&mut self, handle: EnvironmentHandle, intensity: f32) {
        if let Some(e) = self.environments.get_mut(&handle) {
            e.intensity = intensity;
        }
        self.push(SinkEvent::EnvironmentIntensity(handle, intensity));
    }

    fn entity_removed(&mut self, entity: EntityId) {
        self.push(SinkEvent::EntityRemoved(entity));
    }

    fn start_particle(&mut self, request: &ParticleStart) -> Option<ParticleHandle> {
        let h = ParticleHandle(self.handle());
        self.particles.insert(h, *request);
        self.push(SinkEvent::ParticleStarted(h, *request));
        Some(h)
    }

    fn stop_particle(&mut self, handle: ParticleHandle, immediate: bool) {
        self.particles.remove(&handle);
        self.push(SinkEvent::ParticleStopped(handle, immediate));
    }

    fn physics_impulse(&mut self, request: &ImpulseRequest) {
        self.push(SinkEvent::Impulse(*request));
    }
    fn story_hook(&mut self, event: &StoryHookEvent) {
        self.push(SinkEvent::StoryHook(*event));
    }

    fn start_beam(&mut self, request: &BeamStart) -> Option<BeamHandle> {
        let h = BeamHandle(self.handle());
        self.beams.insert(h, request.clone());
        self.push(SinkEvent::BeamStarted(h, Box::new(request.clone())));
        Some(h)
    }

    fn stop_beam(&mut self, handle: BeamHandle) {
        self.beams.remove(&handle);
        self.push(SinkEvent::BeamStopped(handle));
    }
}
