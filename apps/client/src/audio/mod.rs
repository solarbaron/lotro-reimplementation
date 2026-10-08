// SPDX-License-Identifier: GPL-3.0-or-later
//! World audio for the walker (ROADMAP M6 exit criteria 2 and 4): the engine of
//! `tessera-audio` driven by the script graphs of `tessera-scriptgraph`.
//!
//! | Part | Module | Spec |
//! |---|---|---|
//! | Backend choice, volume settings, preferences import | [`backend`] | decision 0019; `sound.md` §7; `install-layout.md` §8.3 |
//! | Sound infos and payloads on the asset workers, request conversion | [`loader`] | `sound.md` §4, §6.8, §12.1–§12.3 |
//! | Script tables, the avatar's start-up scripts, emitters and monitors per landblock | [`scripts`] | `sound.md` §10.1, §10.3, §11 |
//! | The graph's effect sink on the engine | [`sink`] | `sound.md` §6.5, §8, §9.2–§9.4 |
//! | Player properties of the music graph | [`props`] | `sound.md` §10.2–§10.3 |
//! | Game clock (`World_IsNight`) | [`clock`] | `environment.md` §6, §11.1 |
//!
//! # Per frame ([`WorldAudio::update`])
//!
//! 1. Start-up results (dictionary, vocabularies, the avatar's scripts with every table and
//!    sound info they reach, settings) arrive from a worker job.
//! 2. The listener follows the camera (`sound.md` §6.7).
//! 3. Landblocks near the viewer are scanned on the workers (area music, block-map values,
//!    emitters, monitors); static emitters are spawned in the graph when the viewer comes
//!    within their reach and despawned when it leaves or the landblock unloads (§11).
//! 4. The avatar runs `Startup` and its start-up scripts, among them
//!    `SND_music_system_startup` (§12.4 step 1); its properties follow the place on every
//!    change (§12.4 step 2, the Decisions of §10.3).
//! 5. The graph advances (§12.4 step 3), the engine updates, completions go back to the
//!    graph (§9.2 step 12).
//!
//! Nothing here reads an archive on the main thread except sound infos and script records
//! that no job prefetched (driven sound IDs); those reads are counted in [`AudioStats`].

pub mod backend;
pub mod blocks;
pub mod clock;
pub mod loader;
pub mod player_music;
pub mod props;
pub mod scripts;
pub mod sink;

use std::collections::HashMap;
use std::path::PathBuf;
use std::sync::Arc;
use std::sync::mpsc;
use std::time::Instant;

use tessera_assets::{CancelToken, Priority, WorkerPool};
use tessera_audio::engine::{
    AudioEngine, BackendKind, Channel, EngineStats, Listener, LivePosition,
};
use tessera_dat::ArchiveSet;
use tessera_formats::property::PropertyType;
use tessera_scriptgraph::{EntityId, EntityInit, Runtime as GraphRuntime, RuntimeVocabulary};
use tessera_world::{LandblockKey, WorldPos, WorldStreamer};

use self::backend::{BackendChoice, PrefsChoice, SettingsChoice};
use self::blocks::{BlockTracker, ScanMsg, plan_emitters};
pub use self::blocks::{DESPAWN_MARGIN, MAX_SCANS_IN_FLIGHT, MAX_SPAWNS_PER_CHECK, SPAWN_MARGIN};
use self::clock::{GAME_TIME_DID, GameTime};
use self::loader::SoundLoader;
use self::props::{PlaceInputs, PlayerAudioProps, diff_writes, monitor_at, player_props};
use self::scripts::{AudioData, BlockScan, DataSource};
use self::sink::{EngineSink, Entities, EntityAudio, MusicEvent, VoiceBook, prune_anchors};

/// The avatar's runtime ID (emitter IDs carry a landblock DID in their high word).
pub const AVATAR: EntityId = EntityId(1);
/// Most emitters alive at once.
pub const MAX_LIVE_EMITTERS: usize = 1024;
/// How often emitter distances are checked, seconds.
pub const EMITTER_CHECK_INTERVAL: f64 = 0.25;
/// Longest wait for the player's landblock scan before the avatar starts anyway, seconds.
pub const AVATAR_WAIT: f64 = 10.0;
/// First wait before reopening a lost audio device, seconds (decision 0019, "Device
/// loss"); it doubles after every attempt up to [`REOPEN_BACKOFF_MAX`].
pub const REOPEN_BACKOFF_MIN: f64 = 2.0;
/// Longest wait between two reopen attempts, seconds.
pub const REOPEN_BACKOFF_MAX: f64 = 30.0;
/// After this long without a failure the reopen back-off starts again at
/// [`REOPEN_BACKOFF_MIN`], seconds (a flapping device keeps the longer waits).
pub const REOPEN_STABLE_SECS: f64 = 60.0;
/// Most frames of offline-backend audio rendered per walker frame.
pub const MAX_OFFLINE_FRAMES: usize = 48_000 / 4;

/// Audio settings of a walker.
#[derive(Debug, Clone, PartialEq, Default)]
pub struct AudioConfig {
    /// The backend.
    pub backend: BackendChoice,
    /// Importing the original's `[Sound]` volumes.
    pub prefs: PrefsChoice,
    /// The install directory (for the preferences import), once known.
    pub game_dir: Option<PathBuf>,
    /// Landblocks scanned for emitters and monitors around the viewer (Chebyshev radius).
    pub scan_blocks: u8,
    /// Static emitters on (`sound.md` §11).
    pub emitters: bool,
    /// Interior entities sound too.
    pub interiors: bool,
    /// Random seed of the graph (Random nodes, timer silences; `sound.md` §9.2, §10.4).
    pub seed: u64,
    /// Where Tessera's own `[audio]` values come from.
    pub settings: SettingsChoice,
}

impl AudioConfig {
    /// A configuration with `backend`, emitters on and a two-landblock scan radius.
    #[must_use]
    pub fn with_backend(backend: BackendChoice) -> Self {
        Self {
            backend,
            scan_blocks: 2,
            emitters: true,
            ..Self::default()
        }
    }
}

/// The audio settings of the command line: `--audio` (default `null` for headless walker
/// runs, `auto` for windowed ones), `--audio-prefs`, `--audio-settings` (default: the
/// built-in defaults for headless runs, so that their results do not depend on the user's
/// settings file; the user's file for windowed ones), `--audio-seed` (default 0 for
/// headless runs, the system time for windowed ones), `--no-emitters`, `--interiors`.
#[must_use]
pub fn config_from_args(args: &crate::cli::Args) -> AudioConfig {
    let headless = args.walker_headless();
    let backend = args.audio.unwrap_or(if headless {
        BackendChoice::Null
    } else {
        BackendChoice::Auto
    });
    let settings = args.audio_settings.as_deref().map_or(
        if headless {
            SettingsChoice::Defaults
        } else {
            SettingsChoice::User
        },
        SettingsChoice::parse,
    );
    let seed = args
        .audio_seed
        .unwrap_or_else(|| if headless { 0 } else { time_seed() });
    AudioConfig {
        prefs: args
            .audio_prefs
            .as_deref()
            .map_or(PrefsChoice::IfEnabled, PrefsChoice::parse),
        game_dir: args.game_dir.clone(),
        emitters: !args.no_emitters,
        interiors: args.interiors,
        seed,
        settings,
        ..AudioConfig::with_backend(backend)
    }
}

/// A seed from the system clock (windowed runs differ from one another; `--audio-seed`
/// repeats one).
fn time_seed() -> u64 {
    std::time::SystemTime::now()
        .duration_since(std::time::UNIX_EPOCH)
        .map_or(0, |d| d.as_nanos() as u64)
}

/// What start-up delivers.
struct Boot {
    data: Arc<AudioData>,
    vocab: RuntimeVocabulary,
    avatar: Option<EntityInit>,
    clock: Option<GameTime>,
    music_labels: HashMap<u32, String>,
    music_channel: u32,
    ambient_channel: u32,
    mixer: tessera_audio::engine::MixerSettings,
    secs: f64,
    sounds: usize,
}

fn run_boot(
    set: Arc<ArchiveSet>,
    infos: Arc<loader::InfoCache>,
    layers: Vec<u32>,
    config: AudioConfig,
) -> Result<Boot, String> {
    let t0 = Instant::now();
    let data = Arc::new(AudioData::open(set, infos, layers, config.interiors)?);
    let vocab = scripts::runtime_vocabulary(&data);
    let dict = &data.dict;
    // Channel labels (sound.md §7.1) and the music enum's labels (for the HUD).
    let mut labels = backend::default_labels();
    let channel_pid = dict.pid("SoundScript_Channel");
    if let Some(m) = channel_pid.and_then(|p| scripts::property_mapper(dict, p)) {
        let names = scripts::mapper_names(&data.set, m, 64);
        for (i, l) in labels.iter_mut().enumerate() {
            if let Some(n) = names.get(&(i as u32)) {
                l.clone_from(n);
            }
        }
    }
    let find = |label: &str, fallback: Channel| {
        labels
            .iter()
            .position(|l| l == label)
            .map_or(fallback.value(), |i| i as u32)
    };
    let music_channel = find("Music", Channel::MUSIC);
    let ambient_channel = find("Ambient", Channel::AMBIENT);
    let music_labels = data
        .pids
        .area_music
        .and_then(|p| scripts::property_mapper(dict, p))
        .map(|m| scripts::mapper_names(&data.set, m, 8192))
        .unwrap_or_default();
    let clock = match loader::fetch(&data.set, GAME_TIME_DID) {
        Ok(Some(b)) => match GameTime::parse(&b, GAME_TIME_DID) {
            Ok(g) => Some(g),
            Err(e) => {
                tracing::warn!("audio: game-time record: {e}; World_IsNight unset");
                None
            }
        },
        _ => None,
    };
    let own = backend::own_settings(&config.settings);
    let settings = backend::resolve_settings(own, &config.prefs, config.game_dir.as_deref());
    let mixer = backend::mixer_settings(&settings, &labels);
    let (avatar, sounds) = match scripts::first_player_avatar(&data) {
        Some(a) => match scripts::avatar_init(&data, a) {
            Ok((init, n)) => (Some(init), n),
            Err(e) => {
                tracing::warn!("audio: {e}; no music");
                (None, 0)
            }
        },
        None => {
            tracing::warn!("audio: no player avatar in the race table; no music");
            (None, 0)
        }
    };
    Ok(Boot {
        data,
        vocab,
        avatar,
        clock,
        music_labels,
        music_channel,
        ambient_channel,
        mixer,
        secs: t0.elapsed().as_secs_f64(),
        sounds,
    })
}

/// Sends the job's reply exactly once, also when the job never runs or panics.
struct ScanReply {
    tx: Option<mpsc::Sender<ScanMsg>>,
    key: LandblockKey,
    identity: usize,
}

impl ScanReply {
    fn send(mut self, scan: BlockScan) {
        if let Some(tx) = self.tx.take() {
            let _ = tx.send(Ok(scan));
        }
    }
}

impl Drop for ScanReply {
    fn drop(&mut self) {
        if let Some(tx) = self.tx.take() {
            let _ = tx.send(Err((self.key, self.identity)));
        }
    }
}

/// Counters and current values for the HUD, the frame log and reports.
#[derive(Debug, Clone, Default, PartialEq)]
pub struct AudioStats {
    /// Running backend.
    pub backend: Option<BackendKind>,
    /// Whether start-up finished (the graph runs).
    pub ready: bool,
    /// Start-up problem, if any (also: the avatar could not be spawned).
    pub problem: Option<String>,
    /// The audio device was lost and is being reopened (decision 0019, "Device loss").
    pub device_problem: Option<String>,
    /// Reopen attempts after a device loss.
    pub reopens: u64,
    /// `Area_Music` set on the avatar.
    pub area_music: Option<u32>,
    /// Its label in the `AmbientMusicType` enum.
    pub area_label: Option<String>,
    /// Music-channel voices playing.
    pub music_voices: usize,
    /// Ambient-channel voices playing.
    pub ambient_voices: usize,
    /// The engine's counters.
    pub engine: Option<EngineStats>,
    /// Emitters alive in the graph.
    pub emitters: usize,
    /// Landblocks scanned and kept.
    pub blocks: usize,
    /// Area-music changes so far.
    pub area_changes: u64,
    /// Sound infos read on the main thread.
    pub sync_info_reads: u64,
    /// Script records read on the main thread.
    pub sync_script_reads: u64,
    /// Payload loads queued.
    pub loads: u64,
    /// Requests refused before the engine (no info, entity, slot, or out of range).
    pub refused_early: u64,
    /// Main-thread time of the last update, ms.
    pub update_ms: f64,
    /// Longest update so far, ms.
    pub max_update_ms: f64,
    /// Graph node starts so far.
    pub node_starts: u64,
}

/// When to reopen a lost audio device (decision 0019, "Device loss": the engine does not
/// retry by itself; the client reopens after a back-off). The first attempt comes
/// [`REOPEN_BACKOFF_MIN`] after the loss; the wait doubles after every attempt, up to
/// [`REOPEN_BACKOFF_MAX`], and starts again at the minimum once the backend stayed up for
/// [`REOPEN_STABLE_SECS`].
#[derive(Debug, Clone, PartialEq)]
pub struct DeviceRecovery {
    lost: bool,
    backoff: f64,
    next: f64,
    up_since: f64,
}

impl Default for DeviceRecovery {
    fn default() -> Self {
        Self {
            lost: false,
            backoff: REOPEN_BACKOFF_MIN,
            next: 0.0,
            up_since: 0.0,
        }
    }
}

impl DeviceRecovery {
    /// Called once per frame at walker time `now` with whether the engine reports a failed
    /// backend; `true` when a reopen should be attempted now.
    pub fn poll(&mut self, now: f64, failed: bool) -> bool {
        if failed && !self.lost {
            self.lost = true;
            self.next = now + self.backoff;
        }
        if !self.lost {
            if now - self.up_since >= REOPEN_STABLE_SECS {
                self.backoff = REOPEN_BACKOFF_MIN;
            }
            return false;
        }
        now >= self.next
    }

    /// Records an attempt at `now` and whether it brought a working backend back.
    pub fn attempted(&mut self, now: f64, ok: bool) {
        self.backoff = (self.backoff * 2.0).min(REOPEN_BACKOFF_MAX);
        if ok {
            self.lost = false;
            self.up_since = now;
        } else {
            self.next = now + self.backoff;
        }
    }

    /// Whether the device is lost and not yet back.
    #[must_use]
    pub fn is_lost(&self) -> bool {
        self.lost
    }

    /// Seconds until the next attempt (0 when due or not lost).
    #[must_use]
    pub fn wait(&self, now: f64) -> f64 {
        if self.lost {
            (self.next - now).max(0.0)
        } else {
            0.0
        }
    }
}

/// What a headless run reports about audio.
#[derive(Debug, Clone, Default, PartialEq)]
pub struct AudioReport {
    /// Final counters.
    pub stats: AudioStats,
    /// Music events (area changes, track requests, starts, ramp-outs, ends).
    pub music: Vec<MusicEvent>,
}

/// Logs a report: the counters and every music event (the verification of ROADMAP M6
/// criterion 2 reads these lines).
pub fn log_report(r: &AudioReport) {
    let s = &r.stats;
    let e = s.engine;
    tracing::info!(
        backend = ?s.backend,
        area_music = ?s.area_music,
        area_changes = s.area_changes,
        underruns = e.map_or(0, |e| e.underruns),
        overloads = e.map_or(0, |e| e.overloads),
        starved_blocks = e.map_or(0, |e| e.starved_blocks),
        mixer_load_max = e.map_or(0.0, |e| e.mixer_load_max),
        max_update_ms = s.max_update_ms,
        node_starts = s.node_starts,
        loads = s.loads,
        sync_info_reads = s.sync_info_reads,
        sync_script_reads = s.sync_script_reads,
        refused_early = s.refused_early,
        "audio summary"
    );
    for m in &r.music {
        tracing::info!(t = m.t, audio_t = m.audio_t, event = ?m.kind, "audio report: music");
    }
}

/// The inputs of one frame.
#[derive(Debug, Clone, Copy)]
pub struct AudioInput<'a> {
    /// Frame time, seconds.
    pub dt: f64,
    /// The landblock streamer (resident bundles).
    pub streamer: &'a WorldStreamer,
    /// The camera: eye and view direction.
    pub eye: Option<(WorldPos, [f32; 3])>,
    /// The player: position and heading (radians, 0 = east, counter-clockwise; the
    /// walker's convention). The avatar's model faces model +Y, so heading 0 turns model
    /// +Y to world +X.
    pub player: Option<(WorldPos, f32)>,
}

/// World audio (module docs).
pub struct WorldAudio {
    config: AudioConfig,
    engine: AudioEngine,
    loader: SoundLoader,
    pool: WorkerPool,
    layers: Vec<u32>,
    boot: Option<mpsc::Receiver<Result<Boot, String>>>,
    data: Option<Arc<AudioData>>,
    graph: Option<GraphRuntime<'static>>,
    avatar_init: Option<EntityInit>,
    avatar_live: Option<LivePosition>,
    avatar_since: f64,
    props: Option<PlayerAudioProps>,
    entities: Entities,
    book: VoiceBook,
    tracker: BlockTracker,
    scan_tx: mpsc::Sender<ScanMsg>,
    scan_rx: mpsc::Receiver<ScanMsg>,
    live_emitters: HashMap<EntityId, LandblockKey>,
    clock: Option<GameTime>,
    music_labels: HashMap<u32, String>,
    ambient_channel: u32,
    time: f64,
    next_emitter_check: f64,
    offline: Option<(Instant, f64, Vec<f32>)>,
    recovery: DeviceRecovery,
    stats: AudioStats,
}

impl std::fmt::Debug for WorldAudio {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("WorldAudio")
            .field("config", &self.config)
            .field("stats", &self.stats)
            .finish_non_exhaustive()
    }
}

/// The rotation (x, y, z, w) that makes a model facing model +Y (`coordinates.md`, mesh
/// frame) face `heading` (0 = world +X, counter-clockwise): a turn of heading − π/2 about
/// +Z, as `tessera_net::Quaternion::facing`.
fn heading_quaternion(heading: f32) -> [f32; 4] {
    let h = (heading - std::f32::consts::FRAC_PI_2) * 0.5;
    [0.0, 0.0, h.sin(), h.cos()]
}

fn f32x3(p: &WorldPos) -> [f32; 3] {
    [p.x as f32, p.y as f32, p.z as f32]
}

impl WorldAudio {
    /// Starts the engine and the start-up job. `layers` are the active content layers.
    #[must_use]
    pub fn new(
        config: AudioConfig,
        set: Arc<ArchiveSet>,
        pool: WorkerPool,
        layers: Vec<u32>,
    ) -> Self {
        let engine = backend::start_engine(config.backend);
        let loader = SoundLoader::new(Arc::clone(&set), pool.clone(), loader::DEFAULT_PCM_BUDGET);
        let (scan_tx, scan_rx) = mpsc::channel();
        let mut stats = AudioStats {
            backend: Some(engine.backend()),
            ..AudioStats::default()
        };
        let boot = if engine.backend() == BackendKind::Disabled {
            stats.problem = Some("audio off".into());
            None
        } else {
            let (tx, rx) = mpsc::sync_channel(1);
            let infos = Arc::clone(loader.infos());
            let cfg = config.clone();
            let l = layers.clone();
            let job = move |_: &tessera_assets::JobContext<'_>| {
                let r = run_boot(set, infos, l, cfg);
                let _ = tx.send(r);
            };
            match pool.spawn(Priority::Visible, CancelToken::new(), job) {
                Ok(_) => Some(rx),
                Err(e) => {
                    stats.problem = Some(format!("start-up job: {e}"));
                    None
                }
            }
        };
        let offline = (engine.backend() == BackendKind::Offline)
            .then(|| (Instant::now(), 0.0, vec![0.0f32; 2 * MAX_OFFLINE_FRAMES]));
        Self {
            config,
            engine,
            loader,
            pool,
            layers,
            boot,
            data: None,
            graph: None,
            avatar_init: None,
            avatar_live: None,
            avatar_since: 0.0,
            props: None,
            entities: HashMap::new(),
            book: VoiceBook {
                music_channel: Channel::MUSIC.value(),
                ..VoiceBook::default()
            },
            tracker: BlockTracker::default(),
            scan_tx,
            scan_rx,
            live_emitters: HashMap::new(),
            clock: None,
            music_labels: HashMap::new(),
            ambient_channel: Channel::AMBIENT.value(),
            time: 0.0,
            next_emitter_check: 0.0,
            offline,
            recovery: DeviceRecovery::default(),
            stats,
        }
    }

    /// The engine (tests and tools).
    #[must_use]
    pub fn engine(&self) -> &AudioEngine {
        &self.engine
    }

    /// Current counters.
    #[must_use]
    pub fn stats(&self) -> &AudioStats {
        &self.stats
    }

    /// The music events so far.
    #[must_use]
    pub fn music_events(&self) -> Vec<MusicEvent> {
        self.book.music.iter().copied().collect()
    }

    /// A report for headless runs.
    #[must_use]
    pub fn report(&self) -> AudioReport {
        AudioReport {
            stats: self.stats.clone(),
            music: self.music_events(),
        }
    }

    /// One HUD line: backend, `Area_Music` and its label, music and ambient voices, voices,
    /// underruns and mixer load.
    #[must_use]
    pub fn hud_line(&self) -> String {
        let s = &self.stats;
        let backend = s
            .backend
            .map_or("-".to_owned(), |b| format!("{b:?}").to_lowercase());
        if let Some(p) = &s.device_problem {
            return format!("Audio {backend}: {p}");
        }
        if !s.ready {
            return match &s.problem {
                Some(p) => format!("Audio {backend}: {p}"),
                None => format!("Audio {backend}: starting"),
            };
        }
        if let Some(p) = &s.problem {
            return format!("Audio {backend}: {p}");
        }
        let area = match s.area_music {
            Some(v) => format!("{v:#x} ({})", s.area_label.as_deref().unwrap_or("no label")),
            None => "none".to_owned(),
        };
        let (active, virt, under, load) = s.engine.map_or((0, 0, 0, 0.0), |e| {
            (
                e.active_voices,
                e.virtualised_voices,
                e.underruns,
                e.mixer_load_max,
            )
        });
        format!(
            "Audio {backend}: Area_Music {area}; music {} ambient {}; voices {active} (+{virt} virtual); \
             emitters {}; underruns {under}; mixer max {:.0} %",
            s.music_voices,
            s.ambient_voices,
            s.emitters,
            load * 100.0
        )
    }

    fn poll_boot(&mut self) {
        let Some(rx) = &self.boot else { return };
        let r = match rx.try_recv() {
            Ok(r) => r,
            Err(mpsc::TryRecvError::Empty) => return,
            Err(mpsc::TryRecvError::Disconnected) => Err("start-up job failed".into()),
        };
        self.boot = None;
        match r {
            Ok(b) => {
                tracing::info!(
                    secs = b.secs,
                    avatar = b.avatar.is_some(),
                    sound_nodes = b.sounds,
                    infos = b.data.infos.len(),
                    clock = b.clock.is_some(),
                    "audio start-up done"
                );
                self.engine.set_settings(b.mixer);
                self.book.music_channel = b.music_channel;
                self.ambient_channel = b.ambient_channel;
                self.clock = b.clock;
                self.music_labels = b.music_labels;
                let graph = GraphRuntime::new(
                    DataSource(Arc::clone(&b.data)),
                    Arc::clone(&b.data.dict),
                    b.vocab,
                )
                // A property absent on the player starts its Switch from the dictionary
                // default or the type's zero value (`script-runtime.md` §6.2, R27;
                // `sound.md` §10.2).
                .with_seed(self.config.seed);
                self.graph = Some(graph);
                self.avatar_init = b.avatar;
                self.avatar_since = self.time;
                self.data = Some(b.data);
                self.stats.ready = true;
            }
            Err(e) => {
                tracing::error!("audio start-up: {e}");
                self.stats.problem = Some(e);
            }
        }
    }

    /// Runs `f` with the graph and a sink over the engine.
    fn with_graph<R>(
        &mut self,
        f: impl FnOnce(&mut GraphRuntime<'static>, &mut EngineSink<'_>) -> R,
    ) -> Option<R> {
        let graph = self.graph.as_mut()?;
        let mut sink = EngineSink {
            engine: &mut self.engine,
            loader: &self.loader,
            entities: &mut self.entities,
            book: &mut self.book,
        };
        Some(f(graph, &mut sink))
    }

    fn sync_blocks(&mut self, streamer: &WorldStreamer, focus: Option<&WorldPos>) {
        let Some(data) = self.data.clone() else {
            return;
        };
        let Some(here) = focus.and_then(WorldPos::landblock) else {
            return;
        };
        let radius = u32::from(self.config.scan_blocks);
        let resident = streamer
            .resident()
            .map(|(key, _ring, bundle)| (key, Arc::as_ptr(bundle) as usize, bundle));
        let plan = self.tracker.plan(resident, here, radius);
        for key in plan.despawn {
            self.despawn_block(key);
        }
        for order in plan.scans {
            let reply = ScanReply {
                tx: Some(self.scan_tx.clone()),
                key: order.key,
                identity: order.identity,
            };
            let data = Arc::clone(&data);
            let bundle = Arc::clone(order.bundle);
            let identity = order.identity;
            let job = move |ctx: &tessera_assets::JobContext<'_>| {
                if ctx.is_cancelled() {
                    return;
                }
                reply.send(data.scan_block(&bundle, identity));
            };
            // A refused or cancelled job drops its reply, which arrives as a lost scan.
            if let Err(e) = self.pool.spawn(order.priority, order.token, job) {
                tracing::debug!("audio: landblock scan not queued: {e}");
            }
        }
        while let Ok(msg) = self.scan_rx.try_recv() {
            self.tracker.receive(msg);
        }
    }

    fn despawn_block(&mut self, key: LandblockKey) {
        let ids: Vec<EntityId> = self
            .live_emitters
            .iter()
            .filter(|(_, k)| **k == key)
            .map(|(id, _)| *id)
            .collect();
        for id in ids {
            self.despawn_emitter(id);
        }
    }

    fn despawn_emitter(&mut self, id: EntityId) {
        self.live_emitters.remove(&id);
        let _ = self.with_graph(|g, sink| g.despawn(sink, id));
        // A failed spawn left no graph entity; drop its transform anyway.
        if let Some(e) = self.entities.remove(&id) {
            e.live.remove();
        }
    }

    fn update_emitters(&mut self, listener: [f32; 3]) {
        if !self.config.emitters || self.graph.is_none() || self.time < self.next_emitter_check {
            return;
        }
        self.next_emitter_check = self.time + EMITTER_CHECK_INTERVAL;
        let plan = plan_emitters(&self.tracker.blocks, &self.live_emitters, listener);
        for id in plan.despawn {
            self.despawn_emitter(id);
        }
        for (key, e) in plan.spawn {
            if self.live_emitters.len() >= MAX_LIVE_EMITTERS {
                break;
            }
            let pos = [
                e.position[0] as f32,
                e.position[1] as f32,
                e.position[2] as f32,
            ];
            self.entities.insert(
                e.id,
                EntityAudio {
                    live: LivePosition::with_rotation(pos, e.rotation),
                    frozen: Vec::new(),
                },
            );
            let init = EntityInit {
                tables: e.scripts.tables.clone(),
                startup_scripts: e.scripts.startup_scripts.clone(),
                properties: e.scripts.properties.clone(),
                parent: None,
                run_startup: true,
            };
            match self.with_graph(|g, sink| g.spawn_entity(sink, e.id, init)) {
                Some(Ok(())) => {
                    self.live_emitters.insert(e.id, key);
                }
                Some(Err(err)) => {
                    tracing::debug!(id = e.id.0, "audio: emitter not spawned: {err}");
                    self.entities.remove(&e.id);
                }
                None => {
                    self.entities.remove(&e.id);
                }
            }
        }
    }

    /// The place inputs at `p` from the scanned blocks; `None` while the player's block is
    /// not scanned yet.
    fn place_inputs(&self, p: &WorldPos) -> Option<PlaceInputs> {
        let key = p.landblock()?;
        let scan = self.tracker.blocks.get(&key)?.scan.as_ref()?;
        let pos = [p.x, p.y, p.z];
        let near = self
            .tracker
            .blocks
            .iter()
            .filter(|(k, _)| k.chebyshev(key).is_some_and(|d| d <= 1))
            .filter_map(|(_, b)| b.scan.as_ref())
            .flat_map(|s| s.monitors.iter());
        let monitor = monitor_at(near, pos).map(|m| m.values.clone());
        let night = self.clock.and_then(|c| {
            clock::system_clock(0.0)
                .and_then(|g| c.time_of_day(g))
                .map(|t| c.is_night(t))
        });
        Some(PlaceInputs {
            block: scan.place.clone(),
            cell: None,
            monitor,
            area: scan.area,
            dungeon_music: None,
            landmark_music: None,
            night,
        })
    }

    fn apply_props(&mut self, new: PlayerAudioProps) {
        let Some(data) = self.data.clone() else {
            return;
        };
        let old = self.props.clone();
        let types = |pid: u32| -> Option<PropertyType> { props::declared_type(&data.dict, pid) };
        let writes = diff_writes(&data.pids, old.as_ref(), &new, types);
        if writes.is_empty() {
            return;
        }
        let old_area = old.as_ref().and_then(|o| o.area_music);
        if old.is_none() || old_area != new.area_music {
            if old.is_some() {
                self.stats.area_changes += 1;
            }
            self.book.log_music(sink::MusicEventKind::AreaMusic {
                old: old_area,
                new: new.area_music,
            });
        }
        self.with_graph(|g, sink| {
            for (pid, value) in writes {
                let r = match value {
                    Some(v) => g.set_property(sink, AVATAR, pid, v),
                    None => g.clear_property(sink, AVATAR, pid),
                };
                if let Err(e) = r {
                    tracing::debug!("audio: property {pid:#x}: {e}");
                }
            }
        });
        self.props = Some(new);
    }

    fn update_player(&mut self, player: Option<(WorldPos, f32)>) {
        let Some((pos, yaw)) = player else { return };
        if let Some(live) = &self.avatar_live {
            live.set_transform(f32x3(&pos), heading_quaternion(yaw));
        }
        let inputs = self.place_inputs(&pos);
        if self.avatar_live.is_none() {
            // Start the avatar once the graph runs and the player's place is known (or
            // after a while without it).
            if self.graph.is_none()
                || self.avatar_init.is_none()
                || (inputs.is_none() && self.time - self.avatar_since < AVATAR_WAIT)
            {
                return;
            }
            self.spawn_avatar(&pos, yaw, inputs.as_ref());
            return;
        }
        if let Some(inputs) = inputs {
            let previous = self.props.as_ref().and_then(|p| p.area_music);
            let new = player_props(&inputs, previous);
            self.apply_props(new);
        }
    }

    fn spawn_avatar(&mut self, pos: &WorldPos, yaw: f32, inputs: Option<&PlaceInputs>) {
        let Some(init) = self.avatar_init.take() else {
            return;
        };
        let startup: Vec<u32> = self
            .data
            .as_ref()
            .and_then(|d| d.startup_key)
            .into_iter()
            .chain(init.startup_scripts.iter().copied())
            .collect();
        let live = LivePosition::with_rotation(f32x3(pos), heading_quaternion(yaw));
        self.entities.insert(
            AVATAR,
            EntityAudio {
                live: live.clone(),
                frozen: Vec::new(),
            },
        );
        let spawned = self.with_graph(|g, sink| {
            let r = g.spawn_entity(sink, AVATAR, init);
            if r.is_ok() {
                g.set_player(Some(AVATAR));
                g.set_viewport_entity(Some(AVATAR));
            }
            r
        });
        match spawned {
            Some(Ok(())) => {}
            Some(Err(e)) => {
                // No retry: the same scripts would fail the same way. Nothing follows the
                // avatar, and the HUD and the report say why there is no music.
                tracing::error!("audio: avatar not spawned: {e}");
                self.entities.remove(&AVATAR);
                self.stats.problem = Some(format!("avatar not spawned: {e}"));
                return;
            }
            None => {
                self.entities.remove(&AVATAR);
                return;
            }
        }
        self.avatar_live = Some(live);
        // Properties first, then the start-up scripts (sound.md §12.4 steps 1–2), so the
        // music graph starts in the right case.
        let props = player_props(&inputs.cloned().unwrap_or_default(), None);
        self.apply_props(props);
        let started = self.with_graph(|g, sink| {
            let mut out = Vec::new();
            for k in startup {
                out.push((k, g.start_script(sink, AVATAR, k)));
            }
            out
        });
        for (k, r) in started.unwrap_or_default() {
            tracing::debug!(key = k, result = ?r, "audio: avatar start-up script");
        }
        tracing::info!(
            area_music = ?self.props.as_ref().and_then(|p| p.area_music),
            "audio: avatar runs its start-up scripts"
        );
    }

    fn render_offline(&mut self) {
        let Some((start, done, buf)) = &mut self.offline else {
            return;
        };
        let rate = f64::from(self.engine.sample_rate());
        let due = start.elapsed().as_secs_f64() * rate;
        let frames = ((due - *done).max(0.0) as usize).min(MAX_OFFLINE_FRAMES);
        if frames == 0 {
            return;
        }
        let out = &mut buf[..frames * 2];
        if let Err(e) = self.engine.render(out) {
            tracing::warn!("audio: offline render: {e}");
            self.offline = None;
            return;
        }
        // Real time: frames not rendered within the cap are skipped, not caught up.
        *done = due.max(*done + frames as f64);
    }

    /// One frame (module docs).
    pub fn update(&mut self, input: AudioInput<'_>) {
        let t0 = Instant::now();
        let dt = if input.dt.is_finite() {
            input.dt.max(0.0)
        } else {
            0.0
        };
        self.time += dt;
        self.book.now = self.time;
        self.book.audio_now = self.engine.clock().as_secs(self.engine.sample_rate());
        self.poll_boot();
        let mut listener_pos = None;
        if let Some((eye, dir)) = input.eye {
            let f = normalise(dir).unwrap_or([0.0, 1.0, 0.0]);
            let right = normalise([f[1], -f[0], 0.0]).unwrap_or([1.0, 0.0, 0.0]);
            let up = cross(right, f);
            let position = f32x3(&eye);
            listener_pos = Some(position);
            self.engine.set_listener(Listener {
                position,
                right,
                up,
                forward: f,
            });
        }
        let focus = input.player.map(|p| p.0).or(input.eye.map(|e| e.0));
        self.sync_blocks(input.streamer, focus.as_ref());
        if let Some(l) = listener_pos {
            self.update_emitters(l);
        }
        self.update_player(input.player.or(input.eye.map(|e| (e.0, 0.0))));
        self.with_graph(|g, sink| g.advance(sink, dt));
        self.engine.update();
        self.render_offline();
        let done = self.book.completions(&mut self.engine);
        if !done.is_empty() {
            self.with_graph(|g, sink| {
                for h in done {
                    g.sound_completed(sink, h);
                }
            });
        }
        self.book.prune(&self.engine);
        prune_anchors(&mut self.entities, &self.engine);
        self.recover_device();
        self.refresh_stats(t0);
    }

    /// Decision 0019, "Device loss": reopens a failed backend after a back-off. With
    /// `auto` a device that does not open again gives the null backend; a reopen that fails
    /// leaves the engine disabled and is tried again later.
    fn recover_device(&mut self) {
        let failed = self.engine.is_failed()
            || (self.recovery.is_lost() && self.engine.backend() == BackendKind::Disabled);
        if !self.recovery.poll(self.time, failed) {
            self.stats.device_problem = self.recovery.is_lost().then(|| {
                format!(
                    "device lost; reopening in {:.0} s",
                    self.recovery.wait(self.time).ceil()
                )
            });
            return;
        }
        self.stats.reopens += 1;
        let r = self.engine.reopen();
        let ok = r.is_ok() && !self.engine.is_failed();
        self.recovery.attempted(self.time, ok);
        match r {
            Ok(()) => tracing::info!(
                attempt = self.stats.reopens,
                backend = ?self.engine.backend(),
                device = self.engine.device_name().unwrap_or("-"),
                "audio: backend reopened after a device loss"
            ),
            Err(e) => tracing::warn!(
                attempt = self.stats.reopens,
                retry_in = self.recovery.wait(self.time),
                "audio: reopening the backend failed: {e}"
            ),
        }
        self.stats.device_problem = (!ok).then(|| "device lost; reopen failed".to_owned());
    }

    fn refresh_stats(&mut self, t0: Instant) {
        let s = &mut self.stats;
        s.backend = Some(self.engine.backend());
        s.engine = Some(self.engine.stats());
        s.area_music = self.props.as_ref().and_then(|p| p.area_music);
        s.area_label = s
            .area_music
            .and_then(|v| self.music_labels.get(&v).cloned());
        s.music_voices = self.book.playing_on(&self.engine, self.book.music_channel);
        s.ambient_voices = self.book.playing_on(&self.engine, self.ambient_channel);
        s.emitters = self.live_emitters.len();
        s.blocks = self.tracker.blocks.len();
        let ls = self.loader.stats();
        s.sync_info_reads = ls.sync_info_reads;
        s.loads = ls.loads;
        s.refused_early = self.book.refused_early;
        s.sync_script_reads = self.data.as_ref().map_or(0, |d| d.main_thread_reads());
        s.node_starts = self.graph.as_ref().map_or(0, |g| g.stats().node_starts);
        s.update_ms = t0.elapsed().as_secs_f64() * 1e3;
        s.max_update_ms = s.max_update_ms.max(s.update_ms);
    }

    /// Stops everything: every entity is despawned (tracked sounds stop), the engine stays
    /// until dropped.
    pub fn shutdown(&mut self) {
        let ids: Vec<EntityId> = self.live_emitters.keys().copied().collect();
        for id in ids {
            self.despawn_emitter(id);
        }
        if self.avatar_live.take().is_some() {
            let _ = self.with_graph(|g, sink| g.despawn(sink, AVATAR));
        }
        self.engine.update();
        for b in self.tracker.blocks.values() {
            b.token.cancel();
        }
        self.tracker.blocks.clear();
        tracing::info!(stats = ?self.stats, "audio shut down");
    }

    /// The content layers in use.
    #[must_use]
    pub fn layers(&self) -> &[u32] {
        &self.layers
    }
}

fn normalise(v: [f32; 3]) -> Option<[f32; 3]> {
    let l = (v[0] * v[0] + v[1] * v[1] + v[2] * v[2]).sqrt();
    (l.is_finite() && l > 1e-6).then(|| [v[0] / l, v[1] / l, v[2] / l])
}

fn cross(a: [f32; 3], b: [f32; 3]) -> [f32; 3] {
    [
        a[1] * b[2] - a[2] * b[1],
        a[2] * b[0] - a[0] * b[2],
        a[0] * b[1] - a[1] * b[0],
    ]
}

/// Starts only the engine, for `--smoke` (no walker, no graph): returns the running
/// backend after one update.
#[must_use]
pub fn smoke_check(choice: BackendChoice) -> BackendKind {
    let mut e = backend::start_engine(choice);
    e.update();
    e.backend()
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn listener_axes() {
        let f = normalise([0.0, 2.0, 0.0]).unwrap();
        let r = normalise([f[1], -f[0], 0.0]).unwrap();
        assert_eq!(r, [1.0, 0.0, 0.0]);
        assert_eq!(cross(r, f), [0.0, 0.0, 1.0]);
        assert!(normalise([0.0; 3]).is_none());
        assert!(normalise([f32::NAN, 0.0, 0.0]).is_none());
    }

    #[test]
    fn the_avatar_faces_its_heading_with_model_plus_y() {
        use std::f32::consts::{FRAC_PI_2, PI};
        use tessera_audio::engine::rotate_offset;
        let close = |a: [f32; 3], b: [f32; 3]| (0..3).all(|i| (a[i] - b[i]).abs() < 1e-5);
        // Heading 0 (east): 2 m ahead in the model frame is 2 m east.
        let v = rotate_offset(heading_quaternion(0.0), [0.0, 2.0, 0.0]);
        assert!(close(v, [2.0, 0.0, 0.0]), "{v:?}");
        // Heading π/2 (north): the unrotated model, ahead is north.
        let v = rotate_offset(heading_quaternion(FRAC_PI_2), [0.0, 1.0, 0.0]);
        assert!(close(v, [0.0, 1.0, 0.0]), "{v:?}");
        // Heading π (west); the model's +X (its right) points north then.
        let q = heading_quaternion(PI);
        assert!(close(rotate_offset(q, [0.0, 1.0, 0.0]), [-1.0, 0.0, 0.0]));
        assert!(close(rotate_offset(q, [1.0, 0.0, 0.0]), [0.0, 1.0, 0.0]));
    }

    #[test]
    fn a_lost_device_is_reopened_with_a_growing_back_off() {
        let mut r = DeviceRecovery::default();
        // Healthy: never due.
        assert!(!r.poll(0.0, false) && !r.is_lost());
        // Lost at t = 10: the first attempt after the minimum back-off.
        assert!(!r.poll(10.0, true));
        assert!(r.is_lost());
        assert!(!r.poll(11.9, true));
        assert!(r.poll(12.0, true));
        r.attempted(12.0, false);
        // Then 4 s, 8 s, 16 s, 30 s, 30 s.
        let mut t = 12.0;
        for wait in [4.0, 8.0, 16.0, 30.0, 30.0] {
            assert!(!r.poll(t + wait - 0.01, true), "not before {wait} s");
            t += wait;
            assert!(r.poll(t, true), "due after {wait} s");
            r.attempted(t, false);
        }
        // A reopen that works ends the loss.
        t += 30.0;
        assert!(r.poll(t, true));
        r.attempted(t, true);
        assert!(!r.is_lost() && !r.poll(t + 0.1, false));
        // A device that fails again at once waits the long back-off (no flapping)…
        assert!(!r.poll(t + 1.0, true));
        assert!(!r.poll(t + 1.0 + REOPEN_BACKOFF_MAX - 0.5, true));
        assert!(r.poll(t + 1.0 + REOPEN_BACKOFF_MAX, true));
        r.attempted(t + 31.0, true);
        // …and one that stayed up for a while starts at the minimum again.
        let later = t + 31.0 + REOPEN_STABLE_SECS;
        assert!(!r.poll(later, false));
        assert!(!r.poll(later + 1.0, true));
        assert!(r.poll(later + 1.0 + REOPEN_BACKOFF_MIN, true));
    }

    #[test]
    fn a_healthy_null_engine_is_never_reopened() {
        let mut e = backend::start_engine(BackendChoice::Null);
        let mut r = DeviceRecovery::default();
        for i in 0..100 {
            e.update();
            assert!(!r.poll(f64::from(i), e.is_failed()));
        }
        // A reopen of the null backend keeps it running (what the walker does after a loss).
        e.reopen().unwrap();
        assert_eq!(e.backend(), BackendKind::Null);
        assert!(!e.is_failed());
    }

    #[test]
    fn config_defaults() {
        let c = AudioConfig::with_backend(BackendChoice::Null);
        assert!(c.emitters && c.scan_blocks == 2);
        assert_eq!(AudioConfig::default().backend, BackendChoice::Off);
        assert_eq!(smoke_check(BackendChoice::Null), BackendKind::Null);
        assert_eq!(smoke_check(BackendChoice::Off), BackendKind::Disabled);
    }
}
