// SPDX-License-Identifier: GPL-3.0-or-later
//! Output backend and mixer (`docs/specs/sound.md` §6–§8, §9.3, §12.3;
//! `docs/specs/instruments.md` §9.2–§9.4, §10.4–§10.5, §12; backend choice in
//! [decision 0019](../../../../docs/decisions/0019-audio-backend.md)).
//!
//! # Shape
//!
//! [`AudioEngine`] lives on the main thread. It takes [`SoundRequest`]s, applies the request
//! gates of `sound.md` §6.8, keeps the pending list, the payload loads and a mirror of every
//! voice, and once per frame ([`AudioEngine::update`]) computes each voice's gain (the gain
//! chain §6.1 with the distance law §6.2), panning (§6.7), the gain-0 rule (§6.1 step 5) and
//! the voice limit (§6.6). It sends the results to the mixer through a wait-free command
//! ring and receives finished voices through an event ring (ARCH §21).
//!
//! The mixer runs on the audio thread of the **device** backend (cpal, feature `device`),
//! or inside [`AudioEngine::render`] for the **offline** backend (tests, tools), or inside
//! [`AudioEngine::update`] against the wall clock for the **null** backend (`--headless`
//! runs: voices progress and complete, nothing is heard). It resamples each voice by pitch
//! × source rate / output rate, loops, starts and stops at scheduled audio-clock frames,
//! fades out linearly (§6.5) and feeds the reverb (§8). It never allocates, frees or locks.
//!
//! Streamed sources are decoded by a feeder into ring buffers: a thread for the device and
//! null backends, inline between blocks for the offline backend. The first
//! `STREAM_PREFILL_FRAMES` frames of a new stream are decoded on the main thread when its
//! voice is created. The engine never does archive I/O, and payload loads arrive through
//! [`payload_channel`] from the client's workers.
//!
//! # Back-end loss
//!
//! When the device reports that it is gone (unplugged, sound server restarted), every live
//! request completes, new requests are refused with [`RefuseReason::NoBackend`] and
//! [`AudioEngine::is_failed`] (and [`EngineStats::failed`]) turn true. The client decides
//! when to call [`AudioEngine::reopen`]; with [`Backend::Auto`] a reopen that cannot open a
//! device falls back to the null backend.
//!
//! # Per frame
//!
//! 1. [`AudioEngine::set_listener`] with the active camera.
//! 2. Requests, stops ([`AudioEngine::stop`]) and volume updates
//!    ([`AudioEngine::set_volume`]) as the scripts run.
//! 3. [`AudioEngine::update`].
//! 4. [`AudioEngine::drain_events`] to complete tracked script nodes.

mod channel;
#[cfg(feature = "device")]
mod device;
pub mod law;
mod mixer;
mod request;
mod reverb;
mod stream;
mod voice;

use std::collections::{HashMap, HashSet, VecDeque};
use std::sync::Arc;
use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::mpsc;
use std::time::{Duration, Instant};

use rtrb::{Consumer, Producer, RingBuffer};

pub use channel::{CHANNEL_COUNT, Channel, ChannelFlags, MixerSettings};
pub use request::{
    AudioEvent, AudioTime, CompletionReason, IDENTITY_ROTATION, Listener, LivePosition, LiveState,
    LoadError, Payload, PayloadReceiver, PayloadSender, PositionalClass, Positioner, RefuseReason,
    RequestOutcome, SoundFilter, SoundRequest, SoundSource, VoiceHandle, payload_channel,
    rotate_offset,
};
pub use reverb::{EnvironmentId, ReverbEnvironment, ReverbPreset};

use channel::sane_volume;
use law::{VOLUME_EPSILON, gain, pan_gains, pan_of, voice_score};
use mixer::{Command, MAX_BLOCK, MixEvent, Mixer, SharedStats};
use request::{Place, secs_to_frames};
use reverb::{EnvironmentRegistry, ReverbParams};
use stream::{Feeder, StreamJob, stream_pair};
use voice::{EndReason, Voice, VoiceInit};

/// More than this many waiting (loading or pending) requests refuse a new one (`sound.md`
/// §6.8 item 3).
pub const MAX_WAITING_REQUESTS: usize = 250;

/// The prefix byte of sound-info DIDs (`sound.md` §6.8 item 5).
pub const SOUND_INFO_DID_PREFIX: u32 = 0x2A;

/// Frames of a new stream decoded on the calling thread before it starts, so that it
/// starts without a gap.
const STREAM_PREFILL_FRAMES: usize = 4096;

/// Source sample rates the engine plays (Hz). The data holds 8,000–192,000 Hz
/// (`sound.md` §3.3, §3.4); the bound also matches the output rates the engine accepts. A
/// source outside it is refused with [`RefuseReason::InvalidSource`]: a hostile rate would
/// otherwise size a stream buffer without bound, or hold a voice for hours.
pub const SOURCE_RATES: std::ops::RangeInclusive<u32> = 8_000..=384_000;

/// Errors of the engine.
#[derive(Debug, thiserror::Error)]
pub enum EngineError {
    /// The audio device could not be opened or configured.
    #[error("audio device: {0}")]
    Device(String),
    /// The device backend was asked for but the crate was built without the `device`
    /// feature.
    #[error("built without the `device` feature: no audio device support")]
    NoDeviceSupport,
    /// A configuration value is out of range.
    #[error("invalid engine configuration: {0}")]
    Config(&'static str),
    /// [`AudioEngine::render`] on a backend other than offline.
    #[error("render() needs the offline backend")]
    NotOffline,
    /// [`AudioEngine::render`] with a buffer that is not whole stereo frames.
    #[error("render() needs interleaved stereo (an even number of samples)")]
    OddBuffer,
}

/// Options of the device backend.
#[derive(Debug, Clone, Default, PartialEq, Eq)]
pub struct DeviceOptions {
    /// Ask for this sample rate instead of the device default.
    pub sample_rate: Option<u32>,
    /// Ask for this period size in frames instead of the device default.
    pub buffer_frames: Option<u32>,
}

/// Which backend to run.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum Backend {
    /// No back end: every request is refused (`sound.md` §6.8 item 1).
    Disabled,
    /// Mixes against the wall clock in [`AudioEngine::update`] and discards the output.
    Null {
        /// Output rate.
        sample_rate: u32,
    },
    /// Mixes only in [`AudioEngine::render`], into the caller's buffer.
    Offline {
        /// Output rate.
        sample_rate: u32,
    },
    /// The default output device (feature `device`).
    Device(DeviceOptions),
    /// The device if it can be opened, else [`Backend::Null`] at 48 kHz.
    Auto(DeviceOptions),
}

/// The backend that is running.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum BackendKind {
    /// See [`Backend::Disabled`].
    Disabled,
    /// See [`Backend::Null`].
    Null,
    /// See [`Backend::Offline`].
    Offline,
    /// See [`Backend::Device`].
    Device,
}

/// Engine configuration.
#[derive(Debug, Clone, PartialEq)]
pub struct EngineConfig {
    /// The backend.
    pub backend: Backend,
    /// Voice slots: the most voices (playing, virtualised or fading) that exist at once.
    /// 1–4096; default 256.
    pub voice_slots: usize,
    /// Initial settings.
    pub settings: MixerSettings,
    /// Ring-buffer length of a streamed source, seconds (0.1–10; default 0.5).
    pub stream_buffer_secs: f32,
}

impl Default for EngineConfig {
    fn default() -> Self {
        Self {
            backend: Backend::Auto(DeviceOptions::default()),
            voice_slots: 256,
            settings: MixerSettings::default(),
            stream_buffer_secs: 0.5,
        }
    }
}

impl EngineConfig {
    /// The offline backend at `sample_rate` with default settings.
    #[must_use]
    pub fn offline(sample_rate: u32) -> Self {
        Self {
            backend: Backend::Offline { sample_rate },
            ..Self::default()
        }
    }
}

/// State of a request, for tools and tests.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum VoiceState {
    /// Waiting for its payload.
    Loading,
    /// On the pending list.
    Pending,
    /// Audible (or scheduled, or on a muted channel).
    Playing,
    /// Paused and kept: gain 0, over the voice limit, or suspended (§6.1, §6.6, §7.2).
    Virtualised,
    /// Fading out or about to end.
    Stopping,
}

/// Counters for exit criterion 4 and diagnostics.
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct EngineStats {
    /// Running backend.
    pub backend: BackendKind,
    /// Output rate.
    pub sample_rate: u32,
    /// Audio clock at the end of the last mixed block.
    pub clock: AudioTime,
    /// Voices that play (not virtualised), including fading ones.
    pub active_voices: usize,
    /// Virtualised voices.
    pub virtualised_voices: usize,
    /// Requests on the pending list.
    pub pending_requests: usize,
    /// Requests waiting for their payload.
    pub loading_requests: usize,
    /// Device underruns (xruns) reported by the backend.
    pub underruns: u64,
    /// Mixer calls that took longer than the audio they produced.
    pub overloads: u64,
    /// Voice blocks that found their stream buffer empty.
    pub starved_blocks: u64,
    /// Mixer calls.
    pub callbacks: u64,
    /// Render time / audio time of the last mixer call.
    pub mixer_load: f32,
    /// Largest such ratio so far.
    pub mixer_load_max: f32,
    /// Voice starts and stops waiting because the command ring was full. Gain updates are
    /// not queued: while the ring is full each voice keeps only its latest gains, sent once
    /// there is room.
    pub queued_commands: usize,
    /// The backend failed (device lost); see [`AudioEngine::is_failed`].
    pub failed: bool,
}

struct FeederThread {
    stop: Arc<AtomicBool>,
    handle: Option<std::thread::JoinHandle<()>>,
}

impl Drop for FeederThread {
    fn drop(&mut self) {
        self.stop.store(true, Ordering::Release);
        if let Some(h) = self.handle.take() {
            let _ = h.join();
        }
    }
}

impl std::fmt::Debug for FeederThread {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("FeederThread").finish_non_exhaustive()
    }
}

fn spawn_feeder(feeder: Feeder) -> FeederThread {
    let stop = Arc::new(AtomicBool::new(false));
    let flag = stop.clone();
    let handle = std::thread::Builder::new()
        .name("tessera-audio-feeder".into())
        .spawn(move || feeder.run(&flag, Duration::from_millis(5)))
        .map_err(|e| tracing::error!("audio feeder thread: {e}"))
        .ok();
    FeederThread { stop, handle }
}

#[derive(Debug)]
enum Host {
    Disabled,
    Offline {
        mixer: Box<Mixer>,
        feeder: Feeder,
    },
    Null {
        mixer: Box<Mixer>,
        last: Instant,
        scratch: Vec<f32>,
        _feeder: FeederThread,
    },
    #[cfg(feature = "device")]
    Device {
        // Field order: the stream (and the mixer in it) drops before the feeder stops.
        stream: device::DeviceStream,
        _feeder: FeederThread,
    },
}

/// A request's parameters without its source, sanitised.
#[derive(Debug, Clone)]
struct Params {
    sound_id: Option<u32>,
    volume: f32,
    pitch: f32,
    looping: bool,
    channel: Channel,
    class: PositionalClass,
    min_distance: f32,
    max_distance: f32,
    priority: f32,
    positioner: Positioner,
    ramp_out_secs: f32,
    tracked: bool,
    limit_exempt: bool,
    start_at: Option<AudioTime>,
    /// A scheduled stop received while the request was loading or pending: (clock frame,
    /// requested ramp seconds). Applied when the voice is created.
    stop_at: Option<(u64, f32)>,
}

fn sane(x: f32, nan: f32, lo: f32, hi: f32) -> f32 {
    if x.is_nan() { nan } else { x.clamp(lo, hi) }
}

fn split(req: SoundRequest) -> (Params, SoundSource, bool) {
    let p = Params {
        sound_id: req.sound_id,
        volume: sane(req.volume, 0.0, 0.0, 1.0),
        pitch: sane(req.pitch, 1.0, law::PITCH_FLOOR, 64.0),
        looping: req.looping,
        channel: req.channel,
        class: req.positional,
        min_distance: if req.min_distance.is_nan() {
            0.0
        } else {
            req.min_distance
        },
        max_distance: if req.max_distance.is_nan() {
            0.0
        } else {
            req.max_distance
        },
        priority: sane(req.priority, 0.0, -1e6, 1e6),
        positioner: req.positioner,
        ramp_out_secs: sane(req.ramp_out_secs, 0.0, 0.0, 3600.0),
        tracked: req.tracked,
        limit_exempt: req.limit_exempt,
        start_at: req.start_at,
        stop_at: None,
    };
    (p, req.source, req.bypass_filter)
}

/// A decoded source not yet playing.
#[derive(Debug)]
enum Ready {
    Pcm(crate::pcm::Pcm),
    Stream(Box<dyn crate::pcm::PcmSource>),
}

#[derive(Debug)]
struct Waiting {
    handle: u64,
    p: Params,
    src: Ready,
    first: bool,
}

#[derive(Debug)]
struct Loading {
    handle: u64,
    p: Params,
    rx: PayloadReceiver,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum Gate {
    Pass,
    Code(RefuseReason),
    Queued,
}

enum Attempt {
    Started,
    Pending,
    Dropped(RefuseReason),
    /// Its scheduled stop passed before it could start.
    Stopped,
}

enum CreateError {
    /// No free slot; the request is handed back.
    Capacity(Box<Waiting>),
    /// The source has no playable audio.
    Invalid,
}

#[derive(Debug)]
struct CtlVoice {
    handle: u64,
    p: Params,
    stereo: bool,
    stopping: bool,
    /// A stop scheduled at this clock frame (not yet `stopping`).
    scheduled_stop: Option<u64>,
    /// A Stop command (ramp frames, frame) that did not fit into the command ring.
    unsent_stop: Option<(u64, Option<u64>)>,
    gain_virtual: bool,
    limit_virtual: bool,
    suspend_paused: bool,
    /// Pan value while the source is beyond 1.5 m (`None`: centred).
    pan: Option<f32>,
    distance: f32,
    gain: f32,
    /// The last gains (left, right, send) and pause state put on the command ring.
    sent: Option<([f32; 3], bool)>,
    reason: Option<CompletionReason>,
}

impl CtlVoice {
    fn paused(&self) -> bool {
        self.gain_virtual || self.limit_virtual || self.suspend_paused
    }
}

/// The audio engine (main thread). See the module documentation.
#[derive(Debug)]
pub struct AudioEngine {
    host: Host,
    kind: BackendKind,
    /// The backend asked for (for [`AudioEngine::reopen`]).
    requested: Backend,
    sample_rate: u32,
    commands: Option<Producer<Command>>,
    /// Voice starts that did not fit into the command ring, in order (at most one per
    /// slot: a slot is reused only after the mixer has finished its voice). Stops, gains
    /// and reverb changes wait in per-voice and per-engine state instead.
    overflow: VecDeque<Command>,
    mix_events: Option<Consumer<MixEvent>>,
    streams: Option<mpsc::Sender<StreamJob>>,
    stats: Arc<SharedStats>,
    stream_buffer_secs: f32,
    settings: MixerSettings,
    flags: [ChannelFlags; CHANNEL_COUNT],
    suspended: bool,
    listener: Listener,
    voices: Vec<Option<CtlVoice>>,
    free_slots: Vec<u32>,
    by_handle: HashMap<u64, u32>,
    pending: VecDeque<Waiting>,
    loading: Vec<Loading>,
    next_handle: u64,
    events: VecDeque<AudioEvent>,
    filter: SoundFilter,
    suppressed: HashSet<u32>,
    envs: EnvironmentRegistry,
    reverb_sent: Option<ReverbParams>,
    rank: Vec<(f32, u64, u32)>,
    failed_reported: bool,
}

/// Bound on undelivered events (oldest dropped beyond it, with a warning).
const MAX_EVENTS: usize = 65_536;

impl AudioEngine {
    /// Starts the engine with `config`.
    ///
    /// # Errors
    /// [`EngineError::Config`] for out-of-range values; for [`Backend::Device`],
    /// [`EngineError::NoDeviceSupport`] without the `device` feature and
    /// [`EngineError::Device`] when no device can be opened.
    pub fn new(config: EngineConfig) -> Result<Self, EngineError> {
        if !(1..=4096).contains(&config.voice_slots) {
            return Err(EngineError::Config("voice_slots must be 1–4096"));
        }
        if !(0.1..=10.0).contains(&config.stream_buffer_secs) {
            return Err(EngineError::Config("stream_buffer_secs must be 0.1–10"));
        }
        let slots = config.voice_slots;
        let stats = Arc::new(SharedStats::default());
        let (cmd_tx, cmd_rx) = RingBuffer::new(4 * slots + 1024);
        let (ev_tx, ev_rx) = RingBuffer::new(slots + 64);
        let (stream_tx, feeder) = Feeder::new();
        let check_rate = |r: u32| {
            if (8_000..=384_000).contains(&r) {
                Ok(r)
            } else {
                Err(EngineError::Config("sample_rate must be 8000–384000"))
            }
        };
        let make = |rate: u32, cmd_rx, ev_tx, stats: &Arc<SharedStats>| {
            Mixer::new(rate, slots, cmd_rx, ev_tx, stats.clone())
        };
        let (host, kind, sample_rate) = match config.backend.clone() {
            Backend::Disabled => (Host::Disabled, BackendKind::Disabled, 48_000),
            Backend::Offline { sample_rate } => {
                let rate = check_rate(sample_rate)?;
                let mixer = Box::new(make(rate, cmd_rx, ev_tx, &stats));
                (Host::Offline { mixer, feeder }, BackendKind::Offline, rate)
            }
            Backend::Null { sample_rate } => {
                let rate = check_rate(sample_rate)?;
                let mixer = Box::new(make(rate, cmd_rx, ev_tx, &stats));
                let scratch = vec![0.0; MAX_BLOCK * 2];
                (
                    Host::Null {
                        mixer,
                        last: Instant::now(),
                        scratch,
                        _feeder: spawn_feeder(feeder),
                    },
                    BackendKind::Null,
                    rate,
                )
            }
            Backend::Device(opts) => {
                Self::open_device(&opts, slots, cmd_rx, ev_tx, feeder, &stats)?
            }
            Backend::Auto(opts) => {
                // The rings and the feeder move into the device attempt; on failure
                // build fresh ones for the null backend.
                match Self::open_device(&opts, slots, cmd_rx, ev_tx, feeder, &stats) {
                    Ok(h) => h,
                    Err(e) => {
                        tracing::warn!("{e}; audio falls back to the null backend");
                        let mut cfg = config.clone();
                        cfg.backend = Backend::Null {
                            sample_rate: 48_000,
                        };
                        // A later reopen tries the device again.
                        let mut engine = Self::new(cfg)?;
                        engine.requested = config.backend;
                        return Ok(engine);
                    }
                }
            }
        };
        let (commands, mix_events, streams) = if kind == BackendKind::Disabled {
            (None, None, None)
        } else {
            (Some(cmd_tx), Some(ev_rx), Some(stream_tx))
        };
        let mut settings = config.settings;
        settings.master_volume = sane_volume(settings.master_volume);
        for v in &mut settings.channel_volumes {
            *v = sane_volume(*v);
        }
        let mut engine = Self {
            host,
            kind,
            requested: config.backend,
            sample_rate,
            commands,
            overflow: VecDeque::new(),
            mix_events,
            streams,
            stats,
            stream_buffer_secs: config.stream_buffer_secs,
            settings,
            flags: Channel::ALL.map(ChannelFlags::startup),
            suspended: false,
            listener: Listener::default(),
            voices: (0..slots).map(|_| None).collect(),
            free_slots: (0..slots as u32).rev().collect(),
            by_handle: HashMap::new(),
            pending: VecDeque::new(),
            loading: Vec::new(),
            next_handle: 0,
            events: VecDeque::new(),
            filter: SoundFilter::None,
            suppressed: HashSet::new(),
            envs: EnvironmentRegistry::default(),
            reverb_sent: None,
            rank: Vec::with_capacity(slots),
            failed_reported: false,
        };
        engine.update_reverb();
        Ok(engine)
    }

    #[cfg(feature = "device")]
    fn open_device(
        opts: &DeviceOptions,
        slots: usize,
        cmd_rx: Consumer<Command>,
        ev_tx: Producer<MixEvent>,
        feeder: Feeder,
        stats: &Arc<SharedStats>,
    ) -> Result<(Host, BackendKind, u32), EngineError> {
        let mixer_stats = stats.clone();
        let stream = device::open(opts, stats.clone(), move |rate| {
            Mixer::new(rate, slots, cmd_rx, ev_tx, mixer_stats)
        })?;
        let rate = stream.sample_rate;
        Ok((
            Host::Device {
                stream,
                _feeder: spawn_feeder(feeder),
            },
            BackendKind::Device,
            rate,
        ))
    }

    #[cfg(not(feature = "device"))]
    fn open_device(
        _opts: &DeviceOptions,
        _slots: usize,
        _cmd_rx: Consumer<Command>,
        _ev_tx: Producer<MixEvent>,
        _feeder: Feeder,
        _stats: &Arc<SharedStats>,
    ) -> Result<(Host, BackendKind, u32), EngineError> {
        Err(EngineError::NoDeviceSupport)
    }

    /// The running backend.
    #[must_use]
    pub fn backend(&self) -> BackendKind {
        self.kind
    }

    /// The output device's name (device backend only).
    #[must_use]
    pub fn device_name(&self) -> Option<&str> {
        match &self.host {
            #[cfg(feature = "device")]
            Host::Device { stream, .. } => Some(&stream.name),
            _ => None,
        }
    }

    /// Output frames per second.
    #[must_use]
    pub fn sample_rate(&self) -> u32 {
        self.sample_rate
    }

    /// The audio clock at the end of the last mixed block. Schedule starts a little after
    /// it (at least one device period).
    #[must_use]
    pub fn clock(&self) -> AudioTime {
        AudioTime(self.stats.clock.load(Ordering::Acquire))
    }

    /// The current settings.
    #[must_use]
    pub fn settings(&self) -> &MixerSettings {
        &self.settings
    }

    /// Replaces all settings (volumes take effect at the next update).
    pub fn set_settings(&mut self, settings: MixerSettings) {
        let s = MixerSettings {
            master_volume: sane_volume(settings.master_volume),
            channel_volumes: settings.channel_volumes.map(sane_volume),
            ..settings
        };
        self.settings = s;
        self.update_reverb();
    }

    /// "Sound.MasterVolume".
    pub fn set_master_volume(&mut self, v: f32) {
        self.settings.master_volume = sane_volume(v);
    }

    /// `Sound.<label>Volume` of `channel`.
    pub fn set_channel_volume(&mut self, channel: Channel, v: f32) {
        self.settings.channel_volumes[channel.index()] = sane_volume(v);
    }

    /// "Sound.MaxPlayingSounds".
    pub fn set_max_playing_sounds(&mut self, n: u32) {
        self.settings.max_playing_sounds = n;
    }

    /// "Sound.EAXEnabled".
    pub fn set_reverb_enabled(&mut self, on: bool) {
        self.settings.reverb_enabled = on;
        self.update_reverb();
    }

    /// "Sound.SoundDisabled" (gate 7 for new requests; playing voices are not touched).
    pub fn set_sound_disabled(&mut self, on: bool) {
        self.settings.sound_disabled = on;
    }

    /// "Sound.MusicDisabled": mutes channel 2 (Decision, `sound.md` §7.2).
    pub fn set_music_disabled(&mut self, on: bool) {
        self.settings.music_disabled = on;
    }

    /// The flags of `channel` (start-up values of `sound.md` §7.2 by default).
    #[must_use]
    pub fn channel_flags(&self, channel: Channel) -> ChannelFlags {
        self.flags[channel.index()]
    }

    /// Replaces the flags of `channel`.
    pub fn set_channel_flags(&mut self, channel: Channel, flags: ChannelFlags) {
        self.flags[channel.index()] = flags;
    }

    /// Suspends sound (focus loss, loading; `sound.md` §7.2 flag 0x8): voices on channels
    /// with flag 0x8 are paused and kept, the others are stopped at once; new requests get
    /// the code *disabled* until resumed.
    pub fn set_suspended(&mut self, on: bool) {
        if self.suspended == on {
            return;
        }
        self.suspended = on;
        for slot in 0..self.voices.len() {
            let Some(v) = self.voices[slot].as_mut() else {
                continue;
            };
            if v.stopping {
                continue;
            }
            if !on {
                v.suspend_paused = false;
            } else if self.flags[v.p.channel.bookkeeping().index()].has(ChannelFlags::PERSIST) {
                v.suspend_paused = true;
            } else {
                self.stop_slot(slot as u32, 0, None, CompletionReason::Stopped);
                continue;
            }
            self.push_params(slot as u32);
        }
    }

    /// Sets the sound-info filter (§6.8 item 2).
    pub fn set_filter(&mut self, filter: SoundFilter) {
        self.filter = filter;
    }

    /// Adds (`true`) or removes a sound info from the suppression table (§6.8 item 4).
    pub fn set_suppressed(&mut self, sound_id: u32, suppressed: bool) {
        if suppressed {
            self.suppressed.insert(sound_id);
        } else {
            self.suppressed.remove(&sound_id);
        }
    }

    /// The listener (active camera, `sound.md` §6.7). Non-finite values are ignored.
    pub fn set_listener(&mut self, listener: Listener) {
        let ok = [
            listener.position,
            listener.right,
            listener.up,
            listener.forward,
        ]
        .iter()
        .all(|v| v.iter().all(|x| x.is_finite()));
        if ok {
            self.listener = listener;
        }
    }

    /// The listener.
    #[must_use]
    pub fn listener(&self) -> &Listener {
        &self.listener
    }

    fn waiting(&self) -> usize {
        self.pending.len() + self.loading.len()
    }

    fn live_voices(&self) -> impl Iterator<Item = &CtlVoice> {
        self.voices.iter().flatten()
    }

    fn playing_count(&self) -> usize {
        self.live_voices().filter(|v| !v.paused()).count()
    }

    fn channel_busy(&self, bookkeeping: Channel) -> bool {
        self.live_voices()
            .any(|v| v.p.channel.bookkeeping() == bookkeeping)
    }

    fn place(&self, pos: &Positioner) -> Place {
        pos.place(self.listener.position)
    }

    /// Submits a request (`sound.md` §6.8 items 1–6 here; items 7–12 now or, for a payload
    /// still loading, once it arrives).
    pub fn request(&mut self, req: SoundRequest) -> RequestOutcome {
        let (p, source, bypass) = split(req);
        // 1. Back end.
        if self.kind == BackendKind::Disabled || self.stats.failed.load(Ordering::Acquire) {
            return RequestOutcome::Refused(RefuseReason::NoBackend);
        }
        // 2. Filter.
        if let Some(id) = p.sound_id {
            let refused = match &self.filter {
                SoundFilter::None => false,
                SoundFilter::Exclude(set) => !bypass && set.contains(&id),
                SoundFilter::Include(set) => !set.contains(&id),
            };
            if refused {
                return RequestOutcome::Refused(RefuseReason::Filtered);
            }
        }
        // 3. Waiting bound.
        if self.waiting() > MAX_WAITING_REQUESTS {
            return RequestOutcome::Refused(RefuseReason::TooManyWaiting);
        }
        self.next_handle += 1;
        let handle = self.next_handle;
        if let Some(id) = p.sound_id {
            // 4. Suppression.
            if self.suppressed.contains(&id) {
                return RequestOutcome::Suppressed(VoiceHandle(handle));
            }
            // 5. DID range.
            if id >> 24 != SOUND_INFO_DID_PREFIX {
                return RequestOutcome::Refused(RefuseReason::InvalidSoundId);
            }
        }
        // 6. Voice limit.
        if !p.looping
            && !p.limit_exempt
            && self.playing_count() >= self.settings.max_playing_sounds as usize
        {
            return RequestOutcome::Skipped(VoiceHandle(handle));
        }
        let src = match source {
            SoundSource::Loading(rx) => {
                self.loading.push(Loading { handle, p, rx });
                return RequestOutcome::Loading(VoiceHandle(handle));
            }
            SoundSource::Pcm(pcm) => Ready::Pcm(pcm),
            SoundSource::Stream(s) => Ready::Stream(s),
        };
        match self.attempt(Waiting {
            handle,
            p,
            src,
            first: true,
        }) {
            Attempt::Started => RequestOutcome::Started(VoiceHandle(handle)),
            Attempt::Pending => RequestOutcome::Pending(VoiceHandle(handle)),
            Attempt::Dropped(r) => RequestOutcome::Refused(r),
            // Cannot happen: a new request has no scheduled stop.
            Attempt::Stopped => RequestOutcome::Refused(RefuseReason::Disabled),
        }
    }

    /// The gate function, `sound.md` §6.8 items 7–11.
    fn gate(&self, p: &Params, first: bool) -> Gate {
        if self.settings.sound_disabled || self.suspended {
            return Gate::Code(RefuseReason::Disabled);
        }
        if self.settings.master_volume <= VOLUME_EPSILON
            || self.settings.channel_volumes[p.channel.index()] <= VOLUME_EPSILON
        {
            return Gate::Code(RefuseReason::Silent);
        }
        let bk = p.channel.bookkeeping();
        if self.flags[bk.index()].has(ChannelFlags::EXCLUSIVE) && (first || self.channel_busy(bk)) {
            return Gate::Queued;
        }
        if p.class.is_positional() {
            match self.place(&p.positioner) {
                Place::At(pos) => {
                    let d = self.listener.distance(pos);
                    if d.is_nan() || d > p.max_distance {
                        return Gate::Code(RefuseReason::OutOfRange);
                    }
                }
                Place::Invalid | Place::Removed => return Gate::Code(RefuseReason::OutOfRange),
            }
        }
        Gate::Pass
    }

    fn attempt(&mut self, w: Waiting) -> Attempt {
        if self.stop_passed(&w.p) {
            return Attempt::Stopped;
        }
        match self.gate(&w.p, w.first) {
            Gate::Pass => match self.create_voice(w) {
                Ok(()) => Attempt::Started,
                Err(CreateError::Capacity(w)) if w.p.looping => {
                    self.pending.push_back(Waiting { first: false, ..*w });
                    Attempt::Pending
                }
                Err(CreateError::Capacity(_)) => Attempt::Dropped(RefuseReason::Capacity),
                Err(CreateError::Invalid) => Attempt::Dropped(RefuseReason::InvalidSource),
            },
            Gate::Queued => {
                self.pending.push_back(Waiting { first: false, ..w });
                Attempt::Pending
            }
            Gate::Code(_) if w.p.looping => {
                self.pending.push_back(Waiting { first: false, ..w });
                Attempt::Pending
            }
            Gate::Code(c) => Attempt::Dropped(c),
        }
    }

    fn create_voice(&mut self, w: Waiting) -> Result<(), CreateError> {
        let Some(slot) = self.free_slots.pop() else {
            return Err(CreateError::Capacity(Box::new(w)));
        };
        let Waiting { handle, p, src, .. } = w;
        let out_rate = f64::from(self.sample_rate);
        let start_at = p.start_at.map_or(0, |t| t.0);
        let init = |src_rate: u32| VoiceInit {
            id: handle,
            step: f64::from(p.pitch) * f64::from(src_rate) / out_rate,
            looping: p.looping,
            gains: [0.0; 3],
            paused: false,
            start_at,
        };
        let (voice, stereo) = match src {
            Ready::Pcm(pcm) => {
                let stereo = pcm.channels == 2;
                let rate = pcm.sample_rate;
                let voice = SOURCE_RATES
                    .contains(&rate)
                    .then(|| Voice::from_pcm(pcm, init(rate)))
                    .flatten();
                match voice {
                    Some(v) => (v, stereo),
                    None => {
                        self.free_slots.push(slot);
                        return Err(CreateError::Invalid);
                    }
                }
            }
            Ready::Stream(s) => {
                let rate = s.sample_rate();
                if !SOURCE_RATES.contains(&rate) {
                    self.free_slots.push(slot);
                    return Err(CreateError::Invalid);
                }
                let channels = usize::from(s.channels());
                // Bounded by the largest accepted rate whatever the source declares.
                let max_frames =
                    (f64::from(self.stream_buffer_secs) * f64::from(*SOURCE_RATES.end())) as usize;
                let frames = ((f64::from(self.stream_buffer_secs) * f64::from(rate)) as usize)
                    .min(max_frames);
                let Some((mut job, reader)) =
                    stream_pair(s, p.looping, frames.max(STREAM_PREFILL_FRAMES))
                else {
                    self.free_slots.push(slot);
                    return Err(CreateError::Invalid);
                };
                let alive = job.fill(STREAM_PREFILL_FRAMES);
                if alive && let Some(tx) = &self.streams {
                    let _ = tx.send(job);
                }
                (
                    Voice::from_stream(reader, channels, init(rate)),
                    channels == 2,
                )
            }
        };
        self.voices[slot as usize] = Some(CtlVoice {
            handle,
            p,
            stereo,
            stopping: false,
            scheduled_stop: None,
            unsent_stop: None,
            gain_virtual: false,
            limit_virtual: false,
            suspend_paused: false,
            pan: None,
            distance: 0.0,
            gain: 0.0,
            sent: None,
            reason: None,
        });
        self.by_handle.insert(handle, slot);
        // Initial gain and placement; "a source that starts within 1.5 m stays centred".
        self.refresh_voice(slot);
        let (gains, paused) = self.voice_output(slot);
        let mut voice = voice;
        voice.set_params(gains, paused);
        if let Some(v) = self.voices[slot as usize].as_mut() {
            v.sent = Some((gains, paused));
        }
        let start = Command::Start {
            slot,
            voice: Box::new(voice),
        };
        if let Err(cmd) = self.try_push(start) {
            self.overflow.push_back(cmd);
        }
        if let Some((at, ramp)) = self.voices[slot as usize]
            .as_ref()
            .and_then(|v| v.p.stop_at)
        {
            let handle = VoiceHandle(handle);
            self.stop_inner(handle, ramp, Some(at));
        }
        // The gain-0 rule may already apply (a positional sound exactly at max distance).
        self.apply_zero_gain_rule(slot);
        Ok(())
    }

    /// Recomputes distance, pan and gain of a playing voice (§6.1, §6.2, §6.7). Returns
    /// false when its entity was removed.
    fn refresh_voice(&mut self, slot: u32) -> bool {
        let Some(v) = self.voices[slot as usize].as_ref() else {
            return true;
        };
        let place = if v.p.class.is_positional() {
            Some(self.place(&v.p.positioner))
        } else {
            None
        };
        let muted = self.muted(v.p.channel);
        let master = self.settings.master_volume;
        let chan = self.settings.channel_volumes[v.p.channel.index()];
        let listener = self.listener;
        let Some(v) = self.voices[slot as usize].as_mut() else {
            return true;
        };
        let a = match place {
            None => 1.0,
            Some(Place::At(pos)) => {
                let d = listener.distance(pos);
                v.distance = d;
                if d > law::PAN_HOLD_DISTANCE {
                    v.pan = Some(pan_of(listener.to_local(pos)));
                }
                law::distance_factor(d, v.p.min_distance, v.p.max_distance)
            }
            Some(Place::Invalid) => 0.0,
            Some(Place::Removed) => return false,
        };
        v.gain = gain(v.p.volume, a, master, chan, muted);
        true
    }

    fn muted(&self, c: Channel) -> bool {
        self.flags[c.index()].has(ChannelFlags::MUTE)
            || (c == Channel::MUSIC && self.settings.music_disabled)
    }

    /// Per-side gains, reverb-send gain and pause state of a voice. Only flag-0x04
    /// (panned) voices feed the reverb (§6.7, §8), at the voice's own gain whatever the pan
    /// (§8 OPEN, Tessera's choice).
    fn voice_output(&self, slot: u32) -> ([f32; 3], bool) {
        let Some(v) = self.voices[slot as usize].as_ref() else {
            return ([0.0; 3], true);
        };
        let side = if v.stereo {
            (1.0, 1.0)
        } else if v.p.class == PositionalClass::Panned {
            pan_gains(v.pan.unwrap_or(0.0))
        } else {
            pan_gains(0.0)
        };
        let send = if v.p.class == PositionalClass::Panned {
            v.gain
        } else {
            0.0
        };
        ([v.gain * side.0, v.gain * side.1, send], v.paused())
    }

    /// §6.1 step 5: gain exactly 0 on a channel that is not muted stops a one-shot and
    /// virtualises a loop (and un-virtualises it when the gain is positive again).
    fn apply_zero_gain_rule(&mut self, slot: u32) {
        let Some(v) = self.voices[slot as usize].as_ref() else {
            return;
        };
        if v.stopping {
            return;
        }
        let zero = v.gain == 0.0 && !self.muted(v.p.channel);
        let looping = v.p.looping;
        if zero && !looping {
            self.stop_slot(slot, 0, None, CompletionReason::Silenced);
        } else if let Some(v) = self.voices[slot as usize].as_mut() {
            v.gain_virtual = zero;
        }
    }

    /// Sends a voice's gains when they changed. When the ring has no room (or starts are
    /// still waiting) nothing is queued: `sent` keeps the last delivered value, so the next
    /// update sends the then-current gains once (coalescing, bounded memory).
    fn push_params(&mut self, slot: u32) {
        let out = self.voice_output(slot);
        let Some(v) = self.voices[slot as usize].as_ref() else {
            return;
        };
        if v.stopping || v.sent == Some(out) || v.unsent_stop.is_some() {
            return;
        }
        let cmd = Command::Params {
            slot,
            id: v.handle,
            gains: out.0,
            paused: out.1,
        };
        if self.try_push(cmd).is_ok()
            && let Some(v) = self.voices[slot as usize].as_mut()
        {
            v.sent = Some(out);
        }
    }

    /// Stops the voice in `slot`: at once (`at` = `None`) or at clock frame `at`, fading
    /// over `ramp_frames`. Whether the voice is virtualised is decided by the mixer at the
    /// stop frame (a virtualised voice stops at once, §6.5), except that an immediate stop
    /// of a voice the controller has already virtualised skips the fade here too.
    fn stop_slot(
        &mut self,
        slot: u32,
        ramp_frames: u64,
        at: Option<u64>,
        reason: CompletionReason,
    ) {
        let Some(v) = self.voices[slot as usize].as_mut() else {
            return;
        };
        if v.stopping {
            return;
        }
        let ramp = if at.is_none() && v.paused() {
            0
        } else {
            ramp_frames
        };
        if at.is_none() {
            v.stopping = true;
            v.scheduled_stop = None;
            v.reason.get_or_insert(reason);
        } else {
            v.scheduled_stop = at;
        }
        // The newest stop replaces one still waiting (the mixer would replace it too).
        v.unsent_stop = Some((ramp, at));
        self.flush_stop(slot);
    }

    /// Puts a voice's waiting stop on the ring; false when there is no room yet.
    fn flush_stop(&mut self, slot: u32) -> bool {
        let Some(v) = self.voices[slot as usize].as_ref() else {
            return true;
        };
        let Some((ramp_frames, at)) = v.unsent_stop else {
            return true;
        };
        let cmd = Command::Stop {
            slot,
            id: v.handle,
            ramp_frames,
            at,
        };
        if self.try_push(cmd).is_err() {
            return false;
        }
        if let Some(v) = self.voices[slot as usize].as_mut() {
            v.unsent_stop = None;
        }
        true
    }

    /// Pushes `cmd` unless the ring is full or voice starts are still waiting (which must
    /// reach the mixer first); hands it back then. Without a backend the command is
    /// dropped.
    fn try_push(&mut self, cmd: Command) -> Result<(), Command> {
        let Some(tx) = self.commands.as_mut() else {
            return Ok(());
        };
        if !self.overflow.is_empty() {
            return Err(cmd);
        }
        tx.push(cmd).map_err(|rtrb::PushError::Full(cmd)| cmd)
    }

    /// Sends what waited for room: voice starts in order, then stops, then the reverb.
    fn flush_commands(&mut self) {
        let Some(tx) = self.commands.as_mut() else {
            return;
        };
        while let Some(cmd) = self.overflow.pop_front() {
            if let Err(rtrb::PushError::Full(cmd)) = tx.push(cmd) {
                self.overflow.push_front(cmd);
                return;
            }
        }
        for slot in 0..self.voices.len() as u32 {
            if !self.flush_stop(slot) {
                return;
            }
        }
        self.update_reverb();
    }

    /// Stops a request with its ramp-out (`sound.md` §6.5): a playing voice fades linearly
    /// over its `ramp_out_secs` (at once when virtualised); a pending or loading request is
    /// dropped. Returns false for an unknown or finished handle. A tracked request then
    /// completes with [`CompletionReason::Stopped`].
    pub fn stop(&mut self, handle: VoiceHandle) -> bool {
        self.stop_with_ramp(handle, 0.0)
    }

    /// [`Self::stop`] with a requested ramp: the fade time is max(`ramp_secs`, the
    /// request's ramp-out) (`instruments.md` §9.2).
    pub fn stop_with_ramp(&mut self, handle: VoiceHandle, ramp_secs: f32) -> bool {
        self.stop_inner(handle, ramp_secs, None)
    }

    /// Stops a request at an audio-clock time, sample-accurately (`instruments.md` §12),
    /// fading over max(`ramp_secs`, its ramp-out) from that frame. A time already past
    /// stops at once. A request still loading or pending keeps a stop scheduled after the
    /// current clock: it may still start, and stops at that time; once the time has passed
    /// without a voice it completes with [`CompletionReason::Stopped`] without playing.
    pub fn stop_at(&mut self, handle: VoiceHandle, at: AudioTime, ramp_secs: f32) -> bool {
        self.stop_inner(handle, ramp_secs, Some(at.0))
    }

    fn stop_inner(&mut self, handle: VoiceHandle, ramp_secs: f32, at: Option<u64>) -> bool {
        if let Some(&slot) = self.by_handle.get(&handle.0) {
            let Some(v) = self.voices[slot as usize].as_ref() else {
                return false;
            };
            if v.stopping {
                return true;
            }
            let ramp = f64::from(sane(ramp_secs, 0.0, 0.0, 3600.0).max(v.p.ramp_out_secs));
            let frames = secs_to_frames(ramp, self.sample_rate);
            self.stop_slot(slot, frames, at, CompletionReason::Stopped);
            return true;
        }
        // A stop scheduled after the current clock is kept with a request that has no voice
        // yet, so that a note whose payload arrives late still plays until its note-off.
        let later = at.filter(|&t| t > self.clock().0).map(|t| (t, ramp_secs));
        if let Some(i) = self.pending.iter().position(|w| w.handle == handle.0) {
            if later.is_some() {
                self.pending[i].p.stop_at = later;
            } else if let Some(w) = self.pending.remove(i) {
                self.complete(w.handle, w.p.tracked, CompletionReason::Stopped);
            }
            return true;
        }
        if let Some(i) = self.loading.iter().position(|l| l.handle == handle.0) {
            if later.is_some() {
                self.loading[i].p.stop_at = later;
            } else {
                let l = self.loading.swap_remove(i);
                self.complete(l.handle, l.p.tracked, CompletionReason::Stopped);
            }
            return true;
        }
        false
    }

    /// Whether a request without a voice has passed its scheduled stop.
    fn stop_passed(&self, p: &Params) -> bool {
        p.stop_at.is_some_and(|(t, _)| t <= self.clock().0)
    }

    /// Sets the volume of a request (a new intensity of its node, `sound.md` §9.3), applied
    /// at once when it differs from the current one by more than 0.0002 (§6.1 step 4).
    /// Returns false for an unknown or finished handle.
    pub fn set_volume(&mut self, handle: VoiceHandle, volume: f32) -> bool {
        let volume = sane(volume, 0.0, 0.0, 1.0);
        if let Some(&slot) = self.by_handle.get(&handle.0) {
            let Some(v) = self.voices[slot as usize].as_mut() else {
                return false;
            };
            if (volume - v.p.volume).abs() > VOLUME_EPSILON && !v.stopping {
                v.p.volume = volume;
                if !self.refresh_voice(slot) {
                    self.stop_entity_removed(slot);
                    return true;
                }
                self.apply_zero_gain_rule(slot);
                self.push_params(slot);
            }
            return true;
        }
        if let Some(w) = self.pending.iter_mut().find(|w| w.handle == handle.0) {
            w.p.volume = volume;
            return true;
        }
        if let Some(l) = self.loading.iter_mut().find(|l| l.handle == handle.0) {
            l.p.volume = volume;
            return true;
        }
        false
    }

    fn stop_entity_removed(&mut self, slot: u32) {
        let Some(v) = self.voices[slot as usize].as_ref() else {
            return;
        };
        let frames = secs_to_frames(f64::from(v.p.ramp_out_secs), self.sample_rate);
        self.stop_slot(slot, frames, None, CompletionReason::EntityRemoved);
    }

    /// The state of a request, `None` once it is finished (or was never live).
    #[must_use]
    pub fn voice_state(&self, handle: VoiceHandle) -> Option<VoiceState> {
        if let Some(&slot) = self.by_handle.get(&handle.0) {
            let v = self.voices[slot as usize].as_ref()?;
            return Some(if v.stopping {
                VoiceState::Stopping
            } else if v.paused() {
                VoiceState::Virtualised
            } else {
                VoiceState::Playing
            });
        }
        if self.pending.iter().any(|w| w.handle == handle.0) {
            return Some(VoiceState::Pending);
        }
        if self.loading.iter().any(|l| l.handle == handle.0) {
            return Some(VoiceState::Loading);
        }
        None
    }

    /// The gain last computed for a playing voice (§6.1), for tools and tests.
    #[must_use]
    pub fn voice_gain(&self, handle: VoiceHandle) -> Option<f32> {
        let slot = *self.by_handle.get(&handle.0)?;
        self.voices[slot as usize].as_ref().map(|v| v.gain)
    }

    /// Registers a reverb environment (a running SoundEnvironment node, `sound.md` §8).
    /// `None` beyond 4096 registrations.
    pub fn register_environment(&mut self, env: ReverbEnvironment) -> Option<EnvironmentId> {
        let id = self.envs.register(env);
        self.update_reverb();
        id
    }

    /// Unregisters an environment (its node stopped). Returns false for an unknown id.
    pub fn unregister_environment(&mut self, id: EnvironmentId) -> bool {
        let found = self.envs.unregister(id);
        self.update_reverb();
        found
    }

    /// Changes a registered environment's intensity.
    pub fn set_environment_intensity(&mut self, id: EnvironmentId, intensity: f32) -> bool {
        let found = self.envs.set_intensity(id, intensity);
        self.update_reverb();
        found
    }

    /// The active environment (`sound.md` §8): highest priority, ties to the newest;
    /// GENERIC at intensity 0 with none registered.
    #[must_use]
    pub fn active_environment(&self) -> ReverbEnvironment {
        self.envs.active()
    }

    fn update_reverb(&mut self) {
        let a = self.envs.active();
        let p = ReverbParams {
            enabled: self.settings.reverb_enabled,
            preset: a.preset,
            intensity: a.intensity,
        };
        if self.reverb_sent != Some(p) && self.try_push(Command::Reverb(p)).is_ok() {
            // Not sent: retried at the next flush or update.
            self.reverb_sent = Some(p);
        }
    }

    fn complete(&mut self, handle: u64, tracked: bool, reason: CompletionReason) {
        if !tracked {
            return;
        }
        if self.events.len() >= MAX_EVENTS {
            tracing::warn!("audio events not drained; dropping the oldest");
            self.events.pop_front();
        }
        self.events.push_back(AudioEvent::Completed {
            handle: VoiceHandle(handle),
            reason,
        });
    }

    /// Takes the events produced so far (completions of tracked requests).
    pub fn drain_events(&mut self) -> std::collections::vec_deque::Drain<'_, AudioEvent> {
        self.process_mix_events();
        self.events.drain(..)
    }

    fn process_mix_events(&mut self) {
        while let Some(ev) = self.mix_events.as_mut().and_then(|rx| rx.pop().ok()) {
            match ev {
                MixEvent::Finished { slot, voice } => {
                    let matches = self
                        .voices
                        .get(slot as usize)
                        .and_then(|v| v.as_ref())
                        .is_some_and(|v| v.handle == voice.id);
                    if !matches {
                        tracing::error!(
                            slot,
                            id = voice.id,
                            "audio: finished voice not in its slot"
                        );
                        continue;
                    }
                    if let Some(v) = self.voices[slot as usize].take() {
                        let reason = v.reason.unwrap_or(match voice.end {
                            Some(EndReason::Stopped) => CompletionReason::Stopped,
                            _ => CompletionReason::Ended,
                        });
                        self.by_handle.remove(&v.handle);
                        self.free_slots.push(slot);
                        self.complete(v.handle, v.p.tracked, reason);
                    }
                    drop(voice);
                }
                MixEvent::Rejected { voice } => {
                    tracing::error!(id = voice.id, "audio: voice started into a busy slot");
                }
            }
        }
    }

    /// Payload loads that arrived: their requests go through gates 7–12.
    fn poll_loading(&mut self) {
        let mut i = 0;
        while i < self.loading.len() {
            let res = self.loading[i].rx.0.try_recv();
            let payload = match res {
                Err(mpsc::TryRecvError::Empty) => {
                    i += 1;
                    continue;
                }
                Ok(Ok(p)) => Some(p),
                Ok(Err(e)) => {
                    tracing::debug!(
                        handle = self.loading[i].handle,
                        "audio payload load failed: {e}"
                    );
                    None
                }
                Err(mpsc::TryRecvError::Disconnected) => None,
            };
            let l = self.loading.swap_remove(i);
            let Some(payload) = payload else {
                self.complete(l.handle, l.p.tracked, CompletionReason::LoadFailed);
                continue;
            };
            let src = match payload {
                Payload::Pcm(p) => Ready::Pcm(p),
                Payload::Stream(s) => Ready::Stream(s),
            };
            let (handle, tracked) = (l.handle, l.p.tracked);
            match self.attempt(Waiting {
                handle,
                p: l.p,
                src,
                first: true,
            }) {
                Attempt::Dropped(r) => self.complete(handle, tracked, CompletionReason::Refused(r)),
                Attempt::Stopped => self.complete(handle, tracked, CompletionReason::Stopped),
                Attempt::Started | Attempt::Pending => {}
            }
        }
    }

    fn retry_pending(&mut self) {
        let list = std::mem::take(&mut self.pending);
        for w in list {
            let (handle, tracked) = (w.handle, w.p.tracked);
            match self.attempt(w) {
                Attempt::Dropped(r) => self.complete(handle, tracked, CompletionReason::Refused(r)),
                Attempt::Stopped => self.complete(handle, tracked, CompletionReason::Stopped),
                Attempt::Started | Attempt::Pending => {}
            }
        }
    }

    /// Once per frame: mixes (null backend), collects finished voices, takes loaded
    /// payloads, recomputes every voice's gain and placement, applies the gain-0 rule and
    /// the voice limit, retries the pending list and sends the results to the mixer
    /// (`sound.md` §12.3 step 4).
    pub fn update(&mut self) {
        self.pump_null();
        self.check_failed();
        self.process_mix_events();
        self.flush_commands();
        self.poll_loading();
        self.mark_passed_stops();
        for slot in 0..self.voices.len() as u32 {
            let Some(v) = self.voices[slot as usize].as_ref() else {
                continue;
            };
            if v.stopping {
                continue;
            }
            if !self.refresh_voice(slot) {
                self.stop_entity_removed(slot);
                continue;
            }
            self.apply_zero_gain_rule(slot);
        }
        self.rank_voices();
        for slot in 0..self.voices.len() as u32 {
            self.push_params(slot);
        }
        self.retry_pending();
        self.update_reverb();
        self.flush_commands();
    }

    /// A voice whose scheduled stop time has passed is stopping: it gets no more gain
    /// updates and is not ranked for the voice limit (its fade, if any, runs in the mixer).
    fn mark_passed_stops(&mut self) {
        let now = self.clock().0;
        for v in self.voices.iter_mut().flatten() {
            if !v.stopping && v.scheduled_stop.is_some_and(|t| t <= now) {
                v.stopping = true;
                v.scheduled_stop = None;
                v.reason.get_or_insert(CompletionReason::Stopped);
            }
        }
    }

    /// The voice limit (§6.6 and its OPEN Decision): the `MaxPlayingSounds` voices with the
    /// lowest scores stay audible; the other looping voices are virtualised; one-shots are
    /// never stopped by the ranking.
    fn rank_voices(&mut self) {
        let mut rank = std::mem::take(&mut self.rank);
        rank.clear();
        for (slot, v) in self.voices.iter().enumerate() {
            let Some(v) = v else { continue };
            if v.stopping || v.gain_virtual || v.suspend_paused || v.p.limit_exempt {
                continue;
            }
            let score = voice_score(v.p.class.is_positional(), v.p.priority, v.distance);
            rank.push((score, v.handle, slot as u32));
        }
        rank.sort_by(|a, b| a.0.total_cmp(&b.0).then(a.1.cmp(&b.1)));
        let max = self.settings.max_playing_sounds as usize;
        for (i, &(_, _, slot)) in rank.iter().enumerate() {
            if let Some(v) = self.voices[slot as usize].as_mut() {
                v.limit_virtual = i >= max && v.p.looping;
            }
        }
        self.rank = rank;
    }

    fn check_failed(&mut self) {
        if self.failed_reported || !self.stats.failed.load(Ordering::Acquire) {
            return;
        }
        self.failed_reported = true;
        tracing::error!("audio back end failed; live requests complete");
        self.complete_all();
    }

    /// Completes every live request and frees every slot (the mixer's copies are dropped
    /// with the backend).
    fn complete_all(&mut self) {
        self.overflow.clear();
        for slot in 0..self.voices.len() {
            if let Some(v) = self.voices[slot].take() {
                self.by_handle.remove(&v.handle);
                self.free_slots.push(slot as u32);
                self.complete(
                    v.handle,
                    v.p.tracked,
                    v.reason.unwrap_or(CompletionReason::Stopped),
                );
            }
        }
        for w in std::mem::take(&mut self.pending) {
            self.complete(w.handle, w.p.tracked, CompletionReason::Stopped);
        }
        for l in std::mem::take(&mut self.loading) {
            self.complete(l.handle, l.p.tracked, CompletionReason::Stopped);
        }
    }

    fn pump_null(&mut self) {
        self.flush_commands();
        if let Host::Null {
            mixer,
            last,
            scratch,
            ..
        } = &mut self.host
        {
            let now = Instant::now();
            let elapsed = now.duration_since(*last).as_secs_f64().min(0.25);
            *last = now;
            let mut frames = (elapsed * f64::from(self.sample_rate)) as usize;
            while frames > 0 {
                let n = frames.min(MAX_BLOCK);
                mixer.process(&mut scratch[..n * 2], 2);
                frames -= n;
            }
        }
    }

    /// Whether the backend failed (the device was lost). Every live request has then
    /// completed and new requests are refused with [`RefuseReason::NoBackend`] until
    /// [`Self::reopen`].
    #[must_use]
    pub fn is_failed(&self) -> bool {
        self.stats.failed.load(Ordering::Acquire)
    }

    /// Closes the backend and opens the one the engine was created with again (after a
    /// device loss, or to follow a new default device). Every live request completes
    /// first ([`CompletionReason::Stopped`] unless it was already stopping for another
    /// reason). Settings, channel flags, suspension, the filter, the suppression table, the
    /// registered environments, the listener, handle numbering and undrained events carry
    /// over. With [`Backend::Auto`], a device that cannot be opened gives the null backend.
    ///
    /// # Errors
    /// As [`Self::new`]. The engine is then [`BackendKind::Disabled`]: every request is
    /// refused until a later `reopen` succeeds.
    pub fn reopen(&mut self) -> Result<(), EngineError> {
        self.process_mix_events();
        self.complete_all();
        // Release the device (and stop the feeder) before opening it again.
        self.host = Host::Disabled;
        self.kind = BackendKind::Disabled;
        self.commands = None;
        self.mix_events = None;
        self.streams = None;
        let config = EngineConfig {
            backend: self.requested.clone(),
            voice_slots: self.voices.len(),
            settings: self.settings.clone(),
            stream_buffer_secs: self.stream_buffer_secs,
        };
        let mut fresh = Self::new(config)?;
        fresh.requested = self.requested.clone();
        fresh.flags = self.flags;
        fresh.suspended = self.suspended;
        fresh.listener = self.listener;
        fresh.next_handle = self.next_handle;
        fresh.events = std::mem::take(&mut self.events);
        fresh.filter = std::mem::take(&mut self.filter);
        fresh.suppressed = std::mem::take(&mut self.suppressed);
        fresh.envs = std::mem::take(&mut self.envs);
        fresh.update_reverb();
        *self = fresh;
        Ok(())
    }

    /// Mixes `out` (interleaved stereo) with the offline backend, advancing the audio
    /// clock by `out.len() / 2` frames. Streams are fed inline between blocks.
    ///
    /// # Errors
    /// [`EngineError::NotOffline`] on another backend, [`EngineError::OddBuffer`] for an
    /// odd length.
    pub fn render(&mut self, out: &mut [f32]) -> Result<(), EngineError> {
        if !out.len().is_multiple_of(2) {
            return Err(EngineError::OddBuffer);
        }
        self.flush_commands();
        let Host::Offline { mixer, feeder } = &mut self.host else {
            return Err(EngineError::NotOffline);
        };
        for chunk in out.chunks_mut(MAX_BLOCK * 2) {
            feeder.pump();
            mixer.process(chunk, 2);
        }
        Ok(())
    }

    /// Counters (exit criterion 4 of M6).
    #[must_use]
    pub fn stats(&self) -> EngineStats {
        let (mut active, mut virt) = (0, 0);
        for v in self.live_voices() {
            if v.paused() && !v.stopping {
                virt += 1;
            } else {
                active += 1;
            }
        }
        let s = &self.stats;
        EngineStats {
            backend: self.kind,
            sample_rate: self.sample_rate,
            clock: self.clock(),
            active_voices: active,
            virtualised_voices: virt,
            pending_requests: self.pending.len(),
            loading_requests: self.loading.len(),
            underruns: s.underruns.load(Ordering::Relaxed),
            overloads: s.overloads.load(Ordering::Relaxed),
            starved_blocks: s.starved_blocks.load(Ordering::Relaxed),
            callbacks: s.callbacks.load(Ordering::Relaxed),
            mixer_load: s.load_last(),
            mixer_load_max: s.load_max(),
            queued_commands: self.overflow.len()
                + self
                    .live_voices()
                    .filter(|v| v.unsent_stop.is_some())
                    .count(),
            failed: self.is_failed(),
        }
    }
}
