// SPDX-License-Identifier: GPL-3.0-or-later
//! What a sound request carries (`docs/specs/sound.md` §9.2 steps 5–12, §12.3 step 3), its
//! outcome (§6.8, §9.6) and the events sent back to the main thread.

use std::fmt;
use std::sync::mpsc;
use std::sync::{Arc, Mutex};

use crate::pcm::{Pcm, PcmSource};

/// A time on the audio clock: frames at the engine's output rate since the engine started.
#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash, Default)]
pub struct AudioTime(pub u64);

impl AudioTime {
    /// The time `secs` seconds after this one at `sample_rate` (negative or non-finite
    /// seconds count as 0).
    #[must_use]
    pub fn after_secs(self, secs: f64, sample_rate: u32) -> Self {
        Self(self.0.saturating_add(secs_to_frames(secs, sample_rate)))
    }

    /// Seconds since the engine started at `sample_rate`.
    #[must_use]
    pub fn as_secs(self, sample_rate: u32) -> f64 {
        if sample_rate == 0 {
            0.0
        } else {
            self.0 as f64 / f64::from(sample_rate)
        }
    }
}

/// Whole frames in `secs` seconds (rounded), 0 for negative or non-finite values, saturating.
pub(crate) fn secs_to_frames(secs: f64, sample_rate: u32) -> u64 {
    if !secs.is_finite() || secs <= 0.0 {
        return 0;
    }
    let f = (secs * f64::from(sample_rate)).round();
    if f >= u64::MAX as f64 {
        u64::MAX
    } else {
        f as u64
    }
}

/// The request number of an accepted request (`sound.md` §6.8: "reported as accepted with a
/// request number"). Never reused within one engine.
#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash)]
pub struct VoiceHandle(pub(crate) u64);

impl VoiceHandle {
    /// The raw request number (≥ 1).
    #[must_use]
    pub fn raw(self) -> u64 {
        self.0
    }
}

impl fmt::Display for VoiceHandle {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "voice#{}", self.0)
    }
}

/// How a voice is placed, from the sound info's flags (`sound.md` §4.3, §6.7).
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Default)]
pub enum PositionalClass {
    /// Neither 0x04 nor 0x40: no distance law, centred.
    #[default]
    NonPositional,
    /// 0x40 without 0x04: distance law, centred, no reverb send.
    Attenuated,
    /// 0x04: distance law, panned from the source position, reverb send.
    Panned,
}

impl PositionalClass {
    /// The class of a sound info with `flags` (`sound.md` §4.3); 0x04 wins over 0x40.
    #[must_use]
    pub fn from_sound_info_flags(flags: u32) -> Self {
        if flags & 0x04 != 0 {
            Self::Panned
        } else if flags & 0x40 != 0 {
            Self::Attenuated
        } else {
            Self::NonPositional
        }
    }

    /// Whether the distance law applies.
    #[must_use]
    pub fn is_positional(self) -> bool {
        self != Self::NonPositional
    }
}

/// State of a [`LivePosition`].
#[derive(Debug, Clone, Copy, PartialEq)]
pub enum LiveState {
    /// The source is at this point (world metres).
    At([f32; 3]),
    /// The position is unknown for now (entity hidden, landblock not loaded): a positional
    /// voice is silent (`sound.md` §9.2 step 6) and a request is out of range (§6.8 item 11).
    Invalid,
    /// The entity was removed: voices that follow it are stopped with their ramp-out at the
    /// next update (`sound.md` §9.2 OPEN on fire-and-forget loops, Decision).
    Removed,
}

/// The identity rotation, a unit quaternion (x, y, z, w).
pub const IDENTITY_ROTATION: [f32; 4] = [0.0, 0.0, 0.0, 1.0];

#[derive(Debug, Clone, Copy)]
struct LiveTransform {
    state: LiveState,
    rotation: [f32; 4],
}

/// An entity's live transform (position and orientation), shared between the main-thread
/// code that moves the entity and every voice that follows it (`MoveWithEntity` = 1 or
/// `UseCameraPosition` = 1, `sound.md` §9.2 step 6). One per entity is enough: each voice
/// carries its own offset in its [`Positioner`]. Cheap to clone; every clone names the
/// same transform. Read by the engine on the main thread only.
#[derive(Debug, Clone)]
pub struct LivePosition(Arc<Mutex<LiveTransform>>);

impl LivePosition {
    /// A live position starting at `pos` with the identity rotation.
    #[must_use]
    pub fn new(pos: [f32; 3]) -> Self {
        Self::with_rotation(pos, IDENTITY_ROTATION)
    }

    /// A live position starting at `pos` with the orientation `rotation`, a quaternion
    /// (x, y, z, w) as the entity store keeps it.
    #[must_use]
    pub fn with_rotation(pos: [f32; 3], rotation: [f32; 4]) -> Self {
        Self(Arc::new(Mutex::new(LiveTransform {
            state: LiveState::At(pos),
            rotation,
        })))
    }

    /// Moves the source (the orientation is kept).
    pub fn set(&self, pos: [f32; 3]) {
        self.update(|t| t.state = LiveState::At(pos));
    }

    /// Moves and turns the source (`rotation` as in [`Self::with_rotation`]).
    pub fn set_transform(&self, pos: [f32; 3], rotation: [f32; 4]) {
        self.update(|t| {
            t.state = LiveState::At(pos);
            t.rotation = rotation;
        });
    }

    /// Marks the position unknown (see [`LiveState::Invalid`]).
    pub fn set_invalid(&self) {
        self.update(|t| t.state = LiveState::Invalid);
    }

    /// Marks the entity removed (see [`LiveState::Removed`]).
    pub fn remove(&self) {
        self.update(|t| t.state = LiveState::Removed);
    }

    /// The current state.
    #[must_use]
    pub fn get(&self) -> LiveState {
        self.snapshot().state
    }

    /// The current orientation as stored (x, y, z, w).
    #[must_use]
    pub fn rotation(&self) -> [f32; 4] {
        self.snapshot().rotation
    }

    fn snapshot(&self) -> LiveTransform {
        match self.0.lock() {
            Ok(g) => *g,
            Err(p) => *p.into_inner(),
        }
    }

    fn update(&self, f: impl FnOnce(&mut LiveTransform)) {
        match self.0.lock() {
            Ok(mut g) => f(&mut g),
            Err(p) => f(&mut p.into_inner()),
        }
    }

    /// Whether `self` and `other` name the same position.
    #[must_use]
    pub fn same_as(&self, other: &Self) -> bool {
        Arc::ptr_eq(&self.0, &other.0)
    }
}

/// `v` rotated by the quaternion `q` (x, y, z, w), the "offset rotated by the entity's
/// orientation" of `sound.md` §9.2 step 6. `q` is normalised first; a quaternion that is
/// not finite or has a length ≤ 10⁻⁶ counts as the identity.
#[must_use]
pub fn rotate_offset(q: [f32; 4], v: [f32; 3]) -> [f32; 3] {
    let len = q.iter().map(|x| x * x).sum::<f32>().sqrt();
    if !len.is_finite() || len <= 1e-6 {
        return v;
    }
    let [x, y, z, w] = q.map(|c| c / len);
    // v + 2w (u × v) + 2 u × (u × v), u = (x, y, z).
    let u = [x, y, z];
    let t = cross(u, v).map(|c| 2.0 * c);
    let ut = cross(u, t);
    [
        v[0] + w * t[0] + ut[0],
        v[1] + w * t[1] + ut[1],
        v[2] + w * t[2] + ut[2],
    ]
}

fn cross(a: [f32; 3], b: [f32; 3]) -> [f32; 3] {
    [
        a[1] * b[2] - a[2] * b[1],
        a[2] * b[0] - a[0] * b[2],
        a[0] * b[1] - a[1] * b[0],
    ]
}

/// Where a voice's source is (`sound.md` §9.2 step 6). Ignored for non-positional voices.
///
/// The node's offset (random offset + `Offset`, §9.2 step 5) is given in the entity frame
/// and rotated by the entity's live orientation every time the position is read.
#[derive(Debug, Clone)]
pub enum Positioner {
    /// Frozen at this point (`MoveWithEntity` = 0). See [`Positioner::frozen`] to build it
    /// from the entity's transform and the node's offset.
    Fixed([f32; 3]),
    /// Follows an entity (`MoveWithEntity` = 1): the anchor's live position plus `offset`
    /// rotated by the anchor's live orientation.
    Live {
        /// The entity's live transform (one per entity, shared by its voices).
        anchor: LivePosition,
        /// The node's offset in the entity frame, metres.
        offset: [f32; 3],
    },
    /// The live camera position (`UseCameraPosition` = 1) plus `offset` rotated by the
    /// entity's live orientation (unrotated without one).
    Listener {
        /// The node's offset in the entity frame, metres.
        offset: [f32; 3],
        /// The entity whose orientation turns the offset (and whose removal stops the
        /// voice), if any.
        orientation: Option<LivePosition>,
    },
}

impl Positioner {
    /// Follows `anchor` with no offset.
    #[must_use]
    pub fn live(anchor: LivePosition) -> Self {
        Self::Live {
            anchor,
            offset: [0.0; 3],
        }
    }

    /// The camera position with no offset.
    #[must_use]
    pub fn listener() -> Self {
        Self::Listener {
            offset: [0.0; 3],
            orientation: None,
        }
    }

    /// Frozen at `position` + `offset` rotated by `rotation` (x, y, z, w): the source of a
    /// node with `MoveWithEntity` = 0, taken when it starts.
    #[must_use]
    pub fn frozen(position: [f32; 3], rotation: [f32; 4], offset: [f32; 3]) -> Self {
        let o = rotate_offset(rotation, offset);
        Self::Fixed([position[0] + o[0], position[1] + o[1], position[2] + o[2]])
    }
}

/// Where a positioner puts its source now.
pub(crate) enum Place {
    At([f32; 3]),
    Invalid,
    Removed,
}

impl Positioner {
    /// The source position now, given the listener's position.
    pub(crate) fn place(&self, listener: [f32; 3]) -> Place {
        let (base, rotation, offset) = match self {
            Self::Fixed(p) => (*p, IDENTITY_ROTATION, [0.0; 3]),
            Self::Live { anchor, offset } => {
                let t = anchor.snapshot();
                match t.state {
                    LiveState::At(p) => (p, t.rotation, *offset),
                    LiveState::Invalid => return Place::Invalid,
                    LiveState::Removed => return Place::Removed,
                }
            }
            Self::Listener {
                offset,
                orientation,
            } => {
                let rotation = match orientation.as_ref().map(LivePosition::snapshot) {
                    None => IDENTITY_ROTATION,
                    Some(t) => match t.state {
                        LiveState::At(_) => t.rotation,
                        LiveState::Invalid => return Place::Invalid,
                        LiveState::Removed => return Place::Removed,
                    },
                };
                (listener, rotation, *offset)
            }
        };
        let o = rotate_offset(rotation, offset);
        let p = [base[0] + o[0], base[1] + o[1], base[2] + o[2]];
        if p.iter().all(|x| x.is_finite()) {
            Place::At(p)
        } else {
            Place::Invalid
        }
    }
}

/// A decoded payload as supplied by the client's asset workers.
#[derive(Debug)]
pub enum Payload {
    /// Decoded whole.
    Pcm(Pcm),
    /// Decoded while playing; the engine drives it from a feeder thread, never from the
    /// audio thread.
    Stream(Box<dyn PcmSource>),
}

impl Payload {
    /// The payload of a [`crate::decode::Playable`]; `None` for non-audio data.
    #[must_use]
    pub fn from_playable(p: crate::decode::Playable) -> Option<Self> {
        match p {
            crate::decode::Playable::Buffered(pcm) => Some(Self::Pcm(pcm)),
            crate::decode::Playable::Streamed(s) => Some(Self::Stream(s)),
            crate::decode::Playable::NotAudio(_) => None,
        }
    }
}

/// Error text of a failed payload load (logged; the request completes with
/// [`CompletionReason::LoadFailed`]).
pub type LoadError = String;

/// Worker side of an asynchronous payload load (`sound.md` §12.3 step 2). Send it to the
/// asset worker that decodes the payload; dropping it without [`PayloadSender::send`]
/// fails the load.
#[derive(Debug)]
pub struct PayloadSender(mpsc::SyncSender<Result<Payload, LoadError>>);

impl PayloadSender {
    /// Delivers the decoded payload (or the reason it could not be decoded). Never blocks.
    pub fn send(self, payload: Result<Payload, LoadError>) {
        // The receiver may be gone (request stopped); nothing to do then.
        let _ = self.0.try_send(payload);
    }
}

/// Engine side of an asynchronous payload load, placed in a request as
/// [`SoundSource::Loading`]. The engine polls it without blocking on every update.
#[derive(Debug)]
pub struct PayloadReceiver(pub(crate) mpsc::Receiver<Result<Payload, LoadError>>);

/// A one-shot channel for one payload load.
#[must_use]
pub fn payload_channel() -> (PayloadSender, PayloadReceiver) {
    let (tx, rx) = mpsc::sync_channel(1);
    (PayloadSender(tx), PayloadReceiver(rx))
}

/// The audio of a request.
#[derive(Debug)]
pub enum SoundSource {
    /// A whole decoded buffer (shared, cheap to clone).
    Pcm(Pcm),
    /// A streamed source (long music, ARCH §14).
    Stream(Box<dyn PcmSource>),
    /// Still being decoded by a worker; counts as a waiting request (§6.8 item 3).
    Loading(PayloadReceiver),
}

impl From<Payload> for SoundSource {
    fn from(p: Payload) -> Self {
        match p {
            Payload::Pcm(pcm) => Self::Pcm(pcm),
            Payload::Stream(s) => Self::Stream(s),
        }
    }
}

/// One request to play a sound (`sound.md` §9.2 steps 5–12, §12.3 step 3). Build it with
/// [`SoundRequest::new`] and the builder methods, or set the public fields.
///
/// The conversion from a sound-info record (0x2A) and a Sound node is the client's: flags
/// give `looping` (0x01) and `positional` ([`PositionalClass::from_sound_info_flags`]);
/// the record gives `priority`, `min_distance`, `max_distance`; the node gives `channel`,
/// `ramp_out_secs`, `tracked` (= not fire-and-forget), the pitch multiplier
/// ([`super::law::pitch_multiplier`]) and the volume (its intensity, §9.3).
#[derive(Debug)]
pub struct SoundRequest {
    /// The sound-info DID, for the filter, suppression and DID-range gates (§6.8 items 2, 4,
    /// 5). `None` for sounds that client code makes up (test tones): those gates are skipped.
    pub sound_id: Option<u32>,
    /// The audio.
    pub source: SoundSource,
    /// Request volume *v* in [0, 1] (`sound.md` §6.1); clamped, NaN → 0.
    pub volume: f32,
    /// Playback-rate multiplier (§6.3), applied once at start; clamped to [0.01, 64], NaN → 1.
    pub pitch: f32,
    /// Loops seamlessly over the whole played range (§6.4).
    pub looping: bool,
    /// The channel (§7).
    pub channel: super::Channel,
    /// Distance law and panning class (§4.3).
    pub positional: PositionalClass,
    /// Full volume within this distance, metres (§6.2).
    pub min_distance: f32,
    /// Silent beyond this distance, metres (§6.2, §6.8 item 11).
    pub max_distance: f32,
    /// Voice-limit priority, 0–1 (§6.6).
    pub priority: f32,
    /// Where the source is (§9.2 step 6).
    pub positioner: Positioner,
    /// Linear fade time when stopped, seconds (§6.5); negative or NaN → 0.
    pub ramp_out_secs: f32,
    /// Tracked (Sound node with `FireAndForget` = 0): the engine sends a
    /// [`AudioEvent::Completed`] when the voice completes. Fire-and-forget requests send none.
    pub tracked: bool,
    /// Bypasses the voice-limit gate and ranking (client voice modes other than 0, 2 and 4,
    /// `sound.md` §6.6). Sound-node requests never set it.
    pub limit_exempt: bool,
    /// Bypasses the exclusion filter (§6.8 item 2).
    pub bypass_filter: bool,
    /// Start at this audio-clock time instead of at once (`instruments.md` §12: sample-
    /// accurate note starts). A time already past starts at once.
    pub start_at: Option<AudioTime>,
}

impl SoundRequest {
    /// A request with the constructor defaults of a sound info (`sound.md` §4.1: priority 0,
    /// min 1 m, max 50 m) and of a Sound node (channel 0, no ramp-out, pitch 1), volume 1,
    /// non-positional, not looping, fire-and-forget.
    #[must_use]
    pub fn new(source: SoundSource) -> Self {
        Self {
            sound_id: None,
            source,
            volume: 1.0,
            pitch: 1.0,
            looping: false,
            channel: super::Channel::DEFAULT,
            positional: PositionalClass::NonPositional,
            min_distance: 1.0,
            max_distance: 50.0,
            priority: 0.0,
            positioner: Positioner::listener(),
            ramp_out_secs: 0.0,
            tracked: false,
            limit_exempt: false,
            bypass_filter: false,
            start_at: None,
        }
    }

    /// A request for a whole buffer.
    #[must_use]
    pub fn pcm(pcm: Pcm) -> Self {
        Self::new(SoundSource::Pcm(pcm))
    }

    /// Sets [`Self::sound_id`].
    #[must_use]
    pub fn sound_id(mut self, did: u32) -> Self {
        self.sound_id = Some(did);
        self
    }
    /// Sets [`Self::volume`].
    #[must_use]
    pub fn volume(mut self, v: f32) -> Self {
        self.volume = v;
        self
    }
    /// Sets [`Self::pitch`].
    #[must_use]
    pub fn pitch(mut self, p: f32) -> Self {
        self.pitch = p;
        self
    }
    /// Sets [`Self::looping`].
    #[must_use]
    pub fn looping(mut self, l: bool) -> Self {
        self.looping = l;
        self
    }
    /// Sets [`Self::channel`].
    #[must_use]
    pub fn channel(mut self, c: super::Channel) -> Self {
        self.channel = c;
        self
    }
    /// Sets the positional class and distances.
    #[must_use]
    pub fn positional(mut self, class: PositionalClass, min: f32, max: f32) -> Self {
        self.positional = class;
        self.min_distance = min;
        self.max_distance = max;
        self
    }
    /// Sets [`Self::priority`].
    #[must_use]
    pub fn priority(mut self, p: f32) -> Self {
        self.priority = p;
        self
    }
    /// Sets [`Self::positioner`].
    #[must_use]
    pub fn at(mut self, p: Positioner) -> Self {
        self.positioner = p;
        self
    }
    /// Sets [`Self::ramp_out_secs`].
    #[must_use]
    pub fn ramp_out(mut self, secs: f32) -> Self {
        self.ramp_out_secs = secs;
        self
    }
    /// Sets [`Self::tracked`].
    #[must_use]
    pub fn tracked(mut self, t: bool) -> Self {
        self.tracked = t;
        self
    }
    /// Sets [`Self::limit_exempt`].
    #[must_use]
    pub fn limit_exempt(mut self, e: bool) -> Self {
        self.limit_exempt = e;
        self
    }
    /// Sets [`Self::start_at`].
    #[must_use]
    pub fn start_at(mut self, t: AudioTime) -> Self {
        self.start_at = Some(t);
        self
    }
}

/// Why a request was refused or did not play (`sound.md` §6.8).
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum RefuseReason {
    /// Item 1: no audio back end is running.
    NoBackend,
    /// Item 2: excluded (or not included) by the sound-info filter.
    Filtered,
    /// Item 3: more than 250 requests are already waiting.
    TooManyWaiting,
    /// Item 5: the sound-info DID is outside the 0x2A range.
    InvalidSoundId,
    /// Item 7 code *disabled*: sound disabled or suspended.
    Disabled,
    /// Item 8 code *silent*: the master or channel volume is ≤ 0.0002.
    Silent,
    /// Item 11 code *out of range*: positional, and its position is unknown or farther than
    /// `max_distance` from the listener.
    OutOfRange,
    /// The engine's voice slots are all in use (Tessera's allocation bound, not in the spec).
    Capacity,
    /// The payload has no playable frames, or 0 or more than 2 channels, or a sample rate
    /// of 0.
    InvalidSource,
}

impl fmt::Display for RefuseReason {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.write_str(match self {
            Self::NoBackend => "no audio back end",
            Self::Filtered => "filtered",
            Self::TooManyWaiting => "too many waiting requests",
            Self::InvalidSoundId => "not a sound-info DID",
            Self::Disabled => "sound disabled or suspended",
            Self::Silent => "master or channel volume is 0",
            Self::OutOfRange => "out of range",
            Self::Capacity => "no free voice slot",
            Self::InvalidSource => "no playable audio",
        })
    }
}

/// The result of [`super::AudioEngine::request`] (`sound.md` §6.8, §9.2 step 10, §9.6).
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum RequestOutcome {
    /// A voice was created (it may wait for its `start_at` time).
    Started(VoiceHandle),
    /// Put on the pending list (a looping request with a gate code, or a queued one); it is
    /// retried on every update.
    Pending(VoiceHandle),
    /// Waiting for its payload.
    Loading(VoiceHandle),
    /// Accepted but silently not started by the voice limit (item 6). Final: no event
    /// follows; a tracked caller completes at once (§6.6 OPEN, Decision).
    Skipped(VoiceHandle),
    /// Accepted but suppressed (item 4). Final: no event follows.
    Suppressed(VoiceHandle),
    /// Refused. Final: no event follows; the caller's node completes (§9.2 step 10).
    Refused(RefuseReason),
}

impl RequestOutcome {
    /// Whether the manager reports success (everything except [`Self::Refused`]).
    #[must_use]
    pub fn is_accepted(&self) -> bool {
        !matches!(self, Self::Refused(_))
    }

    /// Whether the request is still alive: a tracked one will send exactly one
    /// [`AudioEvent::Completed`] later.
    #[must_use]
    pub fn is_live(&self) -> bool {
        matches!(self, Self::Started(_) | Self::Pending(_) | Self::Loading(_))
    }

    /// The request number, if accepted.
    #[must_use]
    pub fn handle(&self) -> Option<VoiceHandle> {
        match *self {
            Self::Started(h)
            | Self::Pending(h)
            | Self::Loading(h)
            | Self::Skipped(h)
            | Self::Suppressed(h) => Some(h),
            Self::Refused(_) => None,
        }
    }
}

/// Why a live request completed.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum CompletionReason {
    /// Its audio ended (non-looping, `sound.md` §6.4).
    Ended,
    /// Stopped by [`super::AudioEngine::stop`] (after its ramp-out) or by suspension.
    Stopped,
    /// A non-looping voice whose gain became exactly 0 (§6.1 step 5).
    Silenced,
    /// Its entity was removed ([`LiveState::Removed`]).
    EntityRemoved,
    /// A retry or the first attempt after loading got a code that drops it (§6.8).
    Refused(RefuseReason),
    /// The payload could not be loaded.
    LoadFailed,
    /// A streamed source failed before producing any audio.
    SourceFailed,
}

/// An event for the main thread (drained with [`super::AudioEngine::drain_events`]).
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum AudioEvent {
    /// A tracked request completed (exactly once per live tracked request).
    Completed {
        /// The request.
        handle: VoiceHandle,
        /// Why.
        reason: CompletionReason,
    },
}

/// The listener: the active camera (`sound.md` §6.7). The axes are unit vectors in world
/// space; only their directions matter.
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct Listener {
    /// Camera position, world metres.
    pub position: [f32; 3],
    /// Camera right axis (panning to the right channel).
    pub right: [f32; 3],
    /// Camera up axis.
    pub up: [f32; 3],
    /// Camera forward axis.
    pub forward: [f32; 3],
}

impl Default for Listener {
    fn default() -> Self {
        Self {
            position: [0.0; 3],
            right: [1.0, 0.0, 0.0],
            up: [0.0, 0.0, 1.0],
            forward: [0.0, 1.0, 0.0],
        }
    }
}

impl Listener {
    /// `p` in the listener frame (right, up, forward).
    #[must_use]
    pub fn to_local(&self, p: [f32; 3]) -> [f32; 3] {
        let d = sub(p, self.position);
        [dot(d, self.right), dot(d, self.up), dot(d, self.forward)]
    }

    /// Distance from the listener to `p`.
    #[must_use]
    pub fn distance(&self, p: [f32; 3]) -> f32 {
        let d = sub(p, self.position);
        dot(d, d).sqrt()
    }
}

fn sub(a: [f32; 3], b: [f32; 3]) -> [f32; 3] {
    [a[0] - b[0], a[1] - b[1], a[2] - b[2]]
}

fn dot(a: [f32; 3], b: [f32; 3]) -> f32 {
    a[0] * b[0] + a[1] * b[1] + a[2] * b[2]
}

/// The sound-info filter of `sound.md` §6.8 item 2. Tessera applies none by default (OPEN
/// there, Decision); the filter exists so that a later user filter can be plugged in.
#[derive(Debug, Clone, Default, PartialEq, Eq)]
pub enum SoundFilter {
    /// No filter.
    #[default]
    None,
    /// Requests whose sound info is in the set are refused (unless they bypass the filter).
    Exclude(std::collections::HashSet<u32>),
    /// Requests whose sound info is not in the set are refused.
    Include(std::collections::HashSet<u32>),
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn classes_from_flags() {
        assert_eq!(
            PositionalClass::from_sound_info_flags(0x05),
            PositionalClass::Panned
        );
        assert_eq!(
            PositionalClass::from_sound_info_flags(0x45),
            PositionalClass::Panned
        );
        assert_eq!(
            PositionalClass::from_sound_info_flags(0x41),
            PositionalClass::Attenuated
        );
        assert_eq!(
            PositionalClass::from_sound_info_flags(0x11),
            PositionalClass::NonPositional
        );
    }

    #[test]
    fn time_conversions() {
        assert_eq!(AudioTime(10).after_secs(1.0, 48_000), AudioTime(48_010));
        assert_eq!(AudioTime(10).after_secs(-1.0, 48_000), AudioTime(10));
        assert_eq!(AudioTime(10).after_secs(f64::NAN, 48_000), AudioTime(10));
        assert_eq!(
            AudioTime(u64::MAX).after_secs(1e300, 48_000),
            AudioTime(u64::MAX)
        );
        assert!((AudioTime(24_000).as_secs(48_000) - 0.5).abs() < 1e-12);
    }

    #[test]
    fn listener_frame() {
        let l = Listener::default();
        assert_eq!(l.to_local([2.0, 3.0, 4.0]), [2.0, 4.0, 3.0]);
        assert!((l.distance([3.0, 4.0, 0.0]) - 5.0).abs() < 1e-6);
    }

    #[test]
    fn live_position() {
        let p = LivePosition::new([1.0, 2.0, 3.0]);
        let q = p.clone();
        q.set_invalid();
        assert_eq!(p.get(), LiveState::Invalid);
        assert!(p.same_as(&q));
        p.remove();
        assert_eq!(q.get(), LiveState::Removed);
    }

    fn at(p: Place) -> [f32; 3] {
        match p {
            Place::At(p) => p,
            Place::Invalid => panic!("invalid"),
            Place::Removed => panic!("removed"),
        }
    }

    fn near(a: [f32; 3], b: [f32; 3]) -> bool {
        a.iter().zip(b).all(|(x, y)| (x - y).abs() < 1e-5)
    }

    #[test]
    fn offsets_turn_with_the_entity() {
        // 90° about +Z (Z up): +X goes to +Y.
        let h = std::f32::consts::FRAC_1_SQRT_2;
        let quarter = [0.0, 0.0, h, h];
        assert!(near(
            rotate_offset(quarter, [1.0, 0.0, 0.0]),
            [0.0, 1.0, 0.0]
        ));
        assert!(near(
            rotate_offset(IDENTITY_ROTATION, [1.0, 2.0, 3.0]),
            [1.0, 2.0, 3.0]
        ));
        // Not normalised, zero and non-finite quaternions.
        assert!(near(
            rotate_offset([0.0, 0.0, 2.0, 2.0], [1.0, 0.0, 0.0]),
            [0.0, 1.0, 0.0]
        ));
        assert_eq!(rotate_offset([0.0; 4], [1.0, 2.0, 3.0]), [1.0, 2.0, 3.0]);
        assert_eq!(
            rotate_offset([f32::NAN; 4], [1.0, 2.0, 3.0]),
            [1.0, 2.0, 3.0]
        );

        // One live transform per entity, a different offset per voice.
        let e = LivePosition::with_rotation([10.0, 0.0, 0.0], quarter);
        let a = Positioner::Live {
            anchor: e.clone(),
            offset: [20.0, 0.0, 0.0],
        };
        let b = Positioner::live(e.clone());
        assert!(near(at(a.place([0.0; 3])), [10.0, 20.0, 0.0]));
        assert!(near(at(b.place([0.0; 3])), [10.0, 0.0, 0.0]));
        e.set_transform([0.0, 0.0, 5.0], IDENTITY_ROTATION);
        assert!(near(at(a.place([0.0; 3])), [20.0, 0.0, 5.0]));
        e.set([1.0, 1.0, 1.0]);
        assert_eq!(e.rotation(), IDENTITY_ROTATION);

        // Camera position plus the entity-rotated offset.
        let c = Positioner::Listener {
            offset: [0.0, 3.0, 0.0],
            orientation: Some(LivePosition::with_rotation([0.0; 3], quarter)),
        };
        assert!(near(at(c.place([5.0, 5.0, 5.0])), [2.0, 5.0, 5.0]));
        assert!(near(
            at(Positioner::listener().place([5.0, 5.0, 5.0])),
            [5.0; 3]
        ));
        let frozen = Positioner::frozen([1.0, 0.0, 0.0], quarter, [2.0, 0.0, 0.0]);
        assert!(near(at(frozen.place([0.0; 3])), [1.0, 2.0, 0.0]));

        // Invalid and removed entities; a non-finite offset is an invalid position.
        e.set_invalid();
        assert!(matches!(a.place([0.0; 3]), Place::Invalid));
        e.remove();
        assert!(matches!(a.place([0.0; 3]), Place::Removed));
        let gone = Positioner::Listener {
            offset: [0.0; 3],
            orientation: Some(e),
        };
        assert!(matches!(gone.place([0.0; 3]), Place::Removed));
        let bad = Positioner::Live {
            anchor: LivePosition::new([0.0; 3]),
            offset: [f32::INFINITY, 0.0, 0.0],
        };
        assert!(matches!(bad.place([0.0; 3]), Place::Invalid));
    }

    #[test]
    fn payload_channel_round_trip() {
        let (tx, rx) = payload_channel();
        tx.send(Err("x".into()));
        assert!(matches!(rx.0.try_recv(), Ok(Err(_))));
        let (tx, rx) = payload_channel();
        drop(tx);
        assert!(matches!(
            rx.0.try_recv(),
            Err(mpsc::TryRecvError::Disconnected)
        ));
    }
}
