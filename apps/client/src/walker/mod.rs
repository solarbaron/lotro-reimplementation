// SPDX-License-Identifier: GPL-3.0-or-later
//! The offline world walker (ROADMAP M3): terrain and static objects of the user's world
//! streamed around the viewer, free-fly and walk modes, area name, map coordinates,
//! landblock, frame time and residency on screen, and a minimap.
//!
//! # Threads
//!
//! The main thread never reads archives or decodes. It runs, once per frame:
//!
//! 1. input → controllers (`controls`: free-fly and terrain walking on a fixed 30 Hz step,
//!    walk mode on collision on its own fixed 1/60 s step, `movement`);
//! 2. the landblock streamer of `tessera-world` (bundles decoded on the asset workers) and
//!    its frill ring;
//! 3. new or released bundles → the terrain layer (`terrain`: blocks, texture layers and
//!    meshes built on its own workers), the object layer (`objects`: parts and textures
//!    decoded on the asset workers) and the minimap (`minimap`);
//! 4. the environment (`env`: game clock, sky, sun, ambient, fog, lights; `walker.md`
//!    §14.3);
//! 5. GPU uploads of finished products within a per-frame byte and time budget
//!    ([`UploadBudget`]);
//! 6. culling and draw recording into one lit world pass: terrain layers, objects (by LOD
//!    class, faded, instanced; interior cells with their appearance lists, `looks`),
//!    characters (culled by the cell they belong to, [`character_cell`]), water, sky and
//!    frills (`walker.md` §14).
//!
//! Start-up work that reads the archives (world context, area names: `boot`; the place
//! index: `places`) runs on its own threads.
//!
//! # Avatar and characters
//!
//! The sandbox backend of `tessera-net` owns the avatar: it resolves `--spawn` and
//! `--place` through the walker's `source::WalkerSource`, reports where the player
//! entered the world and how the player looks, and receives the walker's movement. It
//! also serves the figures the landblocks place (static NPCs, weenie placements; the
//! source hands it the resident bundles' figures) and its simulated characters
//! (wanderers, `--crowd`). The character layer (`characters`) keeps them all in the
//! entity store, assembles and animates them, and draws them skinned (`walker.md` §10).

pub(crate) mod boot;
pub mod characters;
pub mod chat;
pub(crate) mod controls;
pub mod effects;
pub mod env;
pub mod fallback;
pub mod farband;
pub mod farview;
pub mod figures;
pub(crate) mod framelog;
pub mod frills;
pub mod fxparse;
pub(crate) mod geometry;
pub(crate) mod hud;
pub mod interiors;
pub(crate) mod jobs;
pub mod liquid;
pub mod looks;
pub(crate) mod minimap;
pub mod movement;
pub(crate) mod objects;
pub mod parse;
pub(crate) mod places;
pub(crate) mod resolver;
pub(crate) mod source;
pub mod stage;
pub(crate) mod terrain;
pub(crate) mod view;
pub mod water;

/// The walker's material plan of a resolved surface, with its fallback diagnostic, for the
/// real-data fallback probe.
pub use self::geometry::{
    MaterialPlan, OwnPlan, PlanDiag, TexRequest, plan_material, plan_material_with,
};
/// The fallback reason of a failed texture request of the object layer.
pub use self::objects::texture_failure_reason;
/// The walker's `terrain-texturing.md` §4.9 path, for the real-data row checks.
pub use self::terrain::{block_open, layerless_key, resolve_block_surface};

use std::collections::{HashMap, HashSet};
use std::path::PathBuf;
use std::sync::{Arc, Mutex};
use std::time::{Duration, Instant};

use tessera_assets::{PoolConfig, WorkerPool};
use tessera_dat::ArchiveSet;
use tessera_formats::{PropertySchema, PropertyType};
use tessera_input::RawEvent;
use tessera_net::{EntityId, Event, LocationCause, RegionId};
use tessera_render::glam::{Mat4, Vec3};
use tessera_render::{
    Camera, Frame, FrameStats, Light, Lighting, Material, MeshConfig, MeshData, MeshDraw, MeshId,
    PassSpace, Perspective, Renderer, Rgba,
};
use tessera_terrain::LodSelector;
use tessera_world::{
    DistanceSettings, LANDBLOCK_SIZE, LandblockBundle, LandblockKey, StreamConfig, WorldContext,
    WorldPos, WorldStreamer,
};

use self::characters::catalog::CharacterCatalog;
use self::characters::figure::{ClipStore, FigureContext};
use self::characters::{CharacterConfig, CharacterLayer};
use self::controls::{Controls, MoveMode};
use self::env::{ClockConfig, FrameEnv, IndoorInputs, IndoorPids, WorldEnv};
use self::framelog::{FrameLog, FrameRow, Rolling, rss_bytes};
use self::frills::FrillLayer;
use self::fxparse::{CameraLoop, System, Systems};
use self::hud::TextPanel;
use self::interiors::{BlockVisibility, InteriorView, OUTDOORS, SceneFlags};
use self::minimap::{MinimapLayer, MinimapView};
use self::objects::{ObjectConfig, ObjectFrame, ObjectLayer};
use self::parse::FlyPath;
use self::places::PlaceBuild;
use self::resolver::Resolver;
use self::source::{FigureBlock, Places, SourceShared, describe, loc_text, lock};
use self::terrain::{BlockInputs, TerrainFrame, TerrainLayer};
use self::view::{Anchor, Frustum};
use self::water::WaterLayer;
use crate::net::PlayerLocation;

/// What the walker needs of a world session: the player's authoritative position and a
/// way to report movement. The client's [`crate::net::World`] implements it; a remote
/// session (M7) or a test double can too.
pub trait WalkerSession {
    /// The latest location report of the player.
    fn player_location(&self) -> Option<PlayerLocation>;
    /// Reports the player's movement (`heading` in radians, 0 = east, counter-clockwise).
    fn send_move(
        &mut self,
        now: Instant,
        location: tessera_net::Location,
        heading: f32,
        velocity: tessera_net::Vec3,
    );
    /// The player's entity, once in the world.
    fn player_id(&self) -> Option<EntityId> {
        None
    }
    /// Appends at most `max` of the entity events (created, moved, appearance, despawned,
    /// cleared) received since the last call, oldest first; the rest stay for the next
    /// call. The default has none.
    fn drain_entity_events(&mut self, out: &mut Vec<Event>, max: usize) {
        let _ = (out, max);
    }
    /// The interpolated state of a networked entity at `now` (a remote session,
    /// `tessera-protocol.md` §8.6); `None` keeps the walker's extrapolation of its moves
    /// (the sandbox, the default).
    fn sample_entity(&self, id: EntityId, now: Instant) -> Option<tessera_net::interp::Sample> {
        let _ = (id, now);
        None
    }
    /// Connection state for the HUD. The default has none.
    fn net_status(&self) -> Option<crate::net::NetStatus> {
        None
    }
    /// Appends the chat lines received since the last call. The default has none.
    fn drain_chat(&mut self, out: &mut Vec<crate::net::ChatLine>) {
        let _ = out;
    }
    /// Sends `text` on `channel` (a `Tell` carries its peer); `false` when it was not sent
    /// (not in the world). Only the dispatcher's channel commands call it (decision 0020
    /// item 2). The default sends nothing.
    fn send_chat(&mut self, now: Instant, channel: &tessera_net::ChatChannel, text: &str) -> bool {
        let _ = (now, channel, text);
        false
    }
    /// The HUD's scenario line (`--scenario`, `scenario-feed.md` §12.4): the scenario's
    /// state and, while it lags behind the frame time, the lag. The default has none.
    fn scenario_line(&self, now: Instant) -> Option<String> {
        let _ = now;
        None
    }
    /// The install's creation controls received looks are read with (decision 0025 D2),
    /// once loaded. The default has none (figures then show no look).
    fn look_data(&self) -> Option<Arc<crate::screens::ScreenData>> {
        None
    }
    /// The player's social state (decision 0024 P4: the chat model of `chat-window.md`
    /// §15.10, the fellowship, lists, invitation and trade), which the command dispatcher's
    /// P4 built-ins act on. The default has none (the built-ins then say they are not
    /// connected).
    fn social_mut(&mut self) -> Option<&mut crate::social::SocialState> {
        None
    }
    /// Sends a command for the player (the P4 built-ins' channel and list commands, the
    /// panels' actions); `Err` with the reason when it was not sent. The default sends
    /// nothing.
    ///
    /// # Errors
    ///
    /// Why the command was not sent (not in the world, refused by the backend).
    fn send_command(&mut self, now: Instant, command: tessera_net::Command) -> Result<(), String> {
        let _ = (now, command);
        Err("no world session".to_owned())
    }
}

/// Most session entity events applied in one frame (decision 0008: the store's change
/// lists are unbounded, so the client bounds what it applies); the rest wait, in order, for
/// the next frames.
pub const MAX_ENTITY_EVENTS_PER_FRAME: usize = 4096;

/// Vertical field of view of the paperdoll's portrait, degrees (Tessera choice; the UI
/// scene's camera, `character-panels.md` §3.2, is not read yet).
pub const PORTRAIT_FOV_DEGREES: f32 = 30.0;
/// The portrait camera's distance as a multiple of the one at which the figure's bounding
/// sphere just fills the view's height (below 1: closer; Tessera choice).
pub const PORTRAIT_FILL: f32 = 0.6;
/// The creation viewport's zoom (`character-screens.md` §2.3): the camera's distance as a
/// fraction of the unzoomed one (Tessera framing while the UI scene's zoom cameras are not
/// read, §2.1 OPEN).
pub const PREVIEW_ZOOM_FILL: f32 = 0.5;
/// The zoomed camera's aim above the figure's centre, in bounding-sphere radii (the sphere
/// is the parts' box with a metre of margin, so a quarter radius lands on the upper body).
pub const PREVIEW_ZOOM_RAISE: f32 = 0.25;
/// How far the screens' figure box reaches beyond its joints, metres (Tessera rule of
/// `character-screens.md` §2.3 for "under the pointer": the joints' box widened by the
/// body around them and the head above its joint).
pub const PREVIEW_PICK_PAD: f32 = 0.15;

/// Sky colour (linear) when no environment is drawn.
pub const SKY: Rgba = env::NO_SKY;
/// Vertical field of view: 45° (`walker.md` §11.8, camera-state record 0x08000002).
pub const FOV_Y: f32 = std::f32::consts::FRAC_PI_4;
/// Near clip plane (`walker.md` §11.8: 0.1 m).
pub const NEAR: f32 = 0.1;
/// Smallest mesh-arena share of the terrain.
pub const MIN_TERRAIN_ARENA: u64 = 64 << 20;
/// Mesh-arena bytes allowed per resident terrain block (a measured average of about
/// 34 KiB at the default tolerance, doubled for staged meshes and page fragmentation).
const TERRAIN_BYTES_PER_BLOCK: f64 = 64.0 * 1024.0;

/// The mesh-arena share kept for terrain meshes when the far ring has radius `ring_far`
/// (world units): the blocks of that disc plus the hysteresis ring, at
/// `TERRAIN_BYTES_PER_BLOCK`, never below [`MIN_TERRAIN_ARENA`]. Object parts never
/// fill the arena beyond its cap less this.
#[must_use]
pub fn terrain_arena_reserve(ring_far: f64) -> u64 {
    let r = (ring_far / LANDBLOCK_SIZE).clamp(0.0, 512.0) + 2.0;
    let bytes = std::f64::consts::PI * r * r * TERRAIN_BYTES_PER_BLOCK;
    (bytes as u64).max(MIN_TERRAIN_ARENA)
}

/// Total mesh-arena bytes of a walker with `config`: the object budget plus the
/// terrain's share ([`terrain_arena_reserve`]). The host creates its renderer with
/// `mesh_config(arena_bytes(..))`.
#[must_use]
pub fn arena_bytes(config: &WalkerConfig) -> u64 {
    config
        .objects
        .gpu_budget
        .saturating_add(terrain_arena_reserve(config.stream.rings.far))
}
/// Smallest far clip plane.
pub const MIN_FAR: f32 = 4_000.0;
/// How long a streaming request of the sandbox (a teleport target whose ground is not
/// loaded) keeps the streamer away from the viewer after it was last repeated.
const FOCUS_REQUEST_TTL: Duration = Duration::from_secs(3);

/// The far clip plane for terrain streamed up to `ring_far` world units around the viewer:
/// the far ring's outer block corners plus a margin, never below [`MIN_FAR`].
#[must_use]
pub fn far_plane(ring_far: f64) -> f32 {
    let reach = ring_far + LANDBLOCK_SIZE * std::f64::consts::SQRT_2 + 500.0;
    if reach.is_finite() {
        (reach as f32).max(MIN_FAR)
    } else {
        MIN_FAR
    }
}

/// What a location report of the session does to the viewer.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Placement {
    /// Put the viewer there with the configured view (spawn, teleport, first report).
    Place,
    /// Move the viewer there keeping the view (the session refused a move).
    Correct,
    /// Nothing (an answer to a location query, or a scripted flight owns the camera).
    Ignore,
}

/// How the walker treats a location report with `cause`.
#[must_use]
pub fn placement(cause: LocationCause, placed: bool, scripted: bool) -> Placement {
    if scripted {
        return Placement::Ignore;
    }
    match cause {
        _ if !placed => Placement::Place,
        LocationCause::Entered | LocationCause::Teleported => Placement::Place,
        LocationCause::Corrected => Placement::Correct,
        _ => Placement::Ignore,
    }
}

/// Where the streamer centres: the viewer once placed, except while the sandbox needs
/// data elsewhere (a teleport target, asked about within `FOCUS_REQUEST_TTL`); before
/// placement, the sandbox's request.
#[must_use]
pub fn stream_focus(
    viewer: Option<WorldPos>,
    request: Option<(WorldPos, Instant)>,
    now: Instant,
) -> Option<WorldPos> {
    let fresh = request
        .filter(|(_, at)| now.saturating_duration_since(*at) < FOCUS_REQUEST_TTL)
        .map(|(p, _)| p);
    match (viewer, fresh) {
        (Some(v), Some(r)) if v.landblock() != r.landblock() => Some(r),
        (Some(v), _) => Some(v),
        (None, _) => request.map(|(p, _)| p),
    }
}
/// How often the HUD text is rebuilt.
const HUD_INTERVAL: Duration = Duration::from_millis(250);
/// How often the avatar's movement is reported to the sandbox.
const MOVE_INTERVAL: Duration = Duration::from_millis(100);
/// How often the process RSS is sampled.
const RSS_INTERVAL: Duration = Duration::from_millis(500);

/// The loading state behind [`Walker::settled`] (see [`Walker::load_progress`]): equal
/// snapshots mean no loading work finished or arrived in between.
#[derive(Debug, Clone, Copy, Default, PartialEq, Eq)]
pub struct LoadProgress {
    /// The viewer is placed.
    pub placed: bool,
    /// Streamer: completed bundle jobs, integrated bundles, failed and retried requests.
    pub stream: [u64; 4],
    /// Streamer, last update: landblocks loading, near-ring blocks missing, deferred.
    pub report: [usize; 3],
    /// Terrain: built, meshed, layered, incomplete, pending, images waiting, failed.
    pub terrain: [usize; 7],
    /// Objects: parts ready, pending, failed; models; textures ready, pending.
    pub objects: [usize; 6],
    /// Characters: entities, animated, posed; figures ready, pending, failed.
    pub characters: [usize; 6],
    /// Water surfaces still loading.
    pub water_pending: usize,
    /// Frill sectors waiting.
    pub frills_waiting: usize,
    /// Sky parts all on the GPU or failed.
    pub sky_settled: bool,
    /// Resolver lookups outstanding.
    pub resolver_pending: usize,
}

/// Per-frame GPU upload allowance (ARCH §8: an upload budget keeps frame times stable).
#[derive(Debug, Clone, Copy)]
pub struct UploadBudget {
    bytes_left: u64,
    deadline: Instant,
    took_any: bool,
    /// Bytes taken this frame.
    pub taken: u64,
}

impl UploadBudget {
    /// A budget of `bytes`, valid until `now + time`.
    #[must_use]
    pub fn new(bytes: u64, time: Duration, now: Instant) -> Self {
        Self {
            bytes_left: bytes,
            deadline: now + time,
            took_any: false,
            taken: 0,
        }
    }

    /// Takes `bytes` if they fit and time is left. The first upload of a frame always
    /// fits, so a product larger than the whole budget is not starved.
    pub fn try_take(&mut self, bytes: u64) -> bool {
        if self.took_any && (bytes > self.bytes_left || Instant::now() >= self.deadline) {
            return false;
        }
        self.took_any = true;
        self.bytes_left = self.bytes_left.saturating_sub(bytes);
        self.taken += bytes;
        true
    }
}

/// A scripted straight flight (`--fly-path`).
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct FlightPlan {
    /// The path, region resolved.
    pub path: FlyPath,
    /// Region of the path.
    pub region: u8,
    /// Speed in world units per second.
    pub speed: f64,
    /// Height above the terrain.
    pub height: f64,
    /// Longest wait for the start area to load before flying anyway.
    pub settle_timeout: Duration,
}

/// A timing run (`--duration`): waits for the spawn area to settle (at most
/// `settle_timeout`), then logs every frame for `duration`.
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct BenchPlan {
    /// How long to log.
    pub duration: Duration,
    /// Longest wait for the spawn area to settle.
    pub settle_timeout: Duration,
}

#[derive(Debug, Clone, Copy)]
struct Bench {
    plan: BenchPlan,
    wait_until: Option<Instant>,
    started: Option<Instant>,
    done: bool,
}

#[derive(Debug, Clone, Copy)]
struct Flight {
    plan: FlightPlan,
    wait_until: Instant,
    started: Option<Instant>,
    z: f64,
    done: bool,
}

/// Walker settings.
#[derive(Debug, Clone)]
pub struct WalkerConfig {
    /// Starting movement mode.
    pub mode: MoveMode,
    /// Base fly speed (units per second).
    pub fly_speed: f32,
    /// Region for map coordinates and place names without one.
    pub region: u8,
    /// Landblock streaming (rings, residency budget).
    pub stream: StreamConfig,
    /// Static objects.
    pub objects: ObjectConfig,
    /// Terrain screen-space error tolerance in pixels.
    pub pixel_tolerance: f32,
    /// Terrain worker threads.
    pub terrain_threads: usize,
    /// Load and draw every interior cell and interior entity whatever the camera's cell
    /// (a debugging view; `walker.md` §14.12 (k)). Without it interiors are drawn through the
    /// portal system of `landblock.md` §4.11 from inside only.
    pub interiors: bool,
    /// Keymap file.
    pub keymap: Option<PathBuf>,
    /// Scripted flight.
    pub flight: Option<FlightPlan>,
    /// GPU upload bytes per frame.
    pub upload_bytes: u64,
    /// GPU upload time per frame.
    pub upload_time: Duration,
    /// Minimap: world units from centre to edge.
    pub minimap_radius: f64,
    /// Keep a per-frame log.
    pub frame_log: bool,
    /// Facing at the spawn (radians, 0 = east, counter-clockwise) instead of the
    /// sandbox's.
    pub heading: Option<f32>,
    /// Camera pitch at the spawn (radians, negative = down).
    pub pitch: f32,
    /// Fly camera height above the spawn.
    pub eye_height: f32,
    /// Characters (avatar, NPCs, crowd).
    pub characters: CharacterConfig,
    /// Timing run.
    pub bench: Option<BenchPlan>,
    /// Move forward on its own from the start: `Some(true)` walking, `Some(false)` running.
    pub autorun: Option<bool>,
    /// World audio (ROADMAP M6); off by default.
    pub audio: crate::audio::AudioConfig,
    /// Particle effects (`particle-runtime.md`).
    pub effects: effects::EffectConfig,
    /// Walk on collision (`tessera-physics`, ROADMAP M4.5) when the world builds collision
    /// sets; otherwise, or when false, walk mode follows terrain heights.
    pub collision_walk: bool,
    /// The avatar whose capsule and float height the collision walk mode uses
    /// (`collision.md` §10.6, §11.2).
    pub avatar: crate::avatar::AvatarSpec,
    /// The M4.5 systems switched on (`walker.md` §14.2).
    pub systems: Systems,
    /// The game clock's start and rate (`walker.md` §14.3).
    pub clock: ClockConfig,
    /// The distance preferences (`walker.md` §11.2; the built-in defaults of §11.8).
    pub distances: DistanceSettings,
    /// A scripted camera for timing runs (`m4-5-visual.md` §3).
    pub camera: Option<CameraPlan>,
    /// Terrain blocks farther than this (metres) draw their baked layer stack
    /// (`walker.md` §14.4); 0 draws full layers at every distance.
    pub far_band: f32,
    /// The far view of `far-view.md` (`--far-view on`).
    pub far_view: farview::FarViewConfig,
    /// Draw the HUD and the minimap.
    pub hud: bool,
}

/// A scripted free-fly camera of a timing run (`m4-5-visual.md` §3): a loop flown at a
/// constant speed and height above the higher of terrain and water, or a turn on the spot.
#[derive(Debug, Clone, PartialEq)]
pub enum CameraPlan {
    /// Fly the loop.
    Loop {
        /// The path.
        path: CameraLoop,
        /// Speed, m/s.
        speed: f64,
        /// Height above the terrain or water surface, m.
        height: f64,
        /// Pitch, radians (negative looks down).
        pitch: f32,
    },
    /// Turn on the spot.
    Turn {
        /// Turn rate, radians per second (positive: clockwise seen from above).
        rate: f32,
    },
}

impl Default for WalkerConfig {
    fn default() -> Self {
        Self {
            mode: MoveMode::Walk,
            fly_speed: 60.0,
            region: 1,
            stream: StreamConfig::default(),
            objects: ObjectConfig::default(),
            pixel_tolerance: 1.5,
            // Block jobs resolve texture layers and read masks since M4.5, and far
            // blocks are baked: a quarter of the hardware threads, at least 2.
            terrain_threads: std::thread::available_parallelism()
                .map_or(2, |n| n.get() / 4)
                .clamp(2, 6),
            interiors: false,
            keymap: None,
            flight: None,
            upload_bytes: 24 << 20,
            upload_time: Duration::from_millis(4),
            minimap_radius: 320.0,
            frame_log: false,
            heading: None,
            pitch: -0.25,
            eye_height: 30.0,
            characters: CharacterConfig::default(),
            bench: None,
            autorun: None,
            audio: crate::audio::AudioConfig::default(),
            effects: effects::EffectConfig::default(),
            collision_walk: true,
            avatar: crate::avatar::AvatarSpec::default(),
            systems: Systems::all(),
            clock: ClockConfig::default(),
            distances: DistanceSettings::default(),
            camera: None,
            far_band: DEFAULT_FAR_BAND,
            far_view: farview::FarViewConfig::default(),
            hud: true,
        }
    }
}

/// Where the terrain's far band starts by default, metres (`walker.md` §14.4, Tessera
/// Decision).
pub const DEFAULT_FAR_BAND: f32 = 480.0;

/// What the frame asks of the host.
#[derive(Debug, Clone, Copy, Default, PartialEq, Eq)]
pub struct WalkerEvents {
    /// Close the walker.
    pub quit: bool,
    /// The run duration (`--duration`) is over.
    pub duration_done: bool,
    /// Save a screenshot.
    pub screenshot: bool,
    /// The scripted flight reached its end.
    pub flight_done: bool,
    /// A `menu` press the game UI declined (only while a UI is attached).
    pub menu: bool,
}

/// The walker's own keys, read as raw key codes (`walker.md` §14.2–§14.3): `[`, `]`,
/// `\`, `-`, `=` and F5–F11.
pub const WORLD_KEYS: [tessera_input::KeyCode; 12] = {
    use tessera_input::KeyCode as K;
    [
        K::BracketLeft,
        K::BracketRight,
        K::Backslash,
        K::Minus,
        K::Equal,
        K::F5,
        K::F6,
        K::F7,
        K::F8,
        K::F9,
        K::F10,
        K::F11,
    ]
};

/// The [`WORLD_KEYS`] that some binding of `keymap` uses (any context, alone or in a
/// chord): the walker leaves them to the keymap, so a user's binding never fires twice.
#[must_use]
pub fn keys_taken_by(keymap: &tessera_input::Keymap) -> HashSet<tessera_input::KeyCode> {
    let mut taken = HashSet::new();
    for (_, _, bindings) in keymap.iter() {
        for b in bindings {
            let tessera_input::BindingSpec::Digital { chord, .. } = b else {
                continue;
            };
            for stroke in chord.strokes() {
                for button in stroke.buttons() {
                    if let tessera_input::Button::Key(k) = button
                        && WORLD_KEYS.contains(k)
                    {
                        taken.insert(*k);
                    }
                }
            }
        }
    }
    taken
}

/// The camera's interior cell (`walker.md` §14.12 (a), `landblock.md` §4.12 Tessera
/// Decision): the cell lookup of the resident landblocks at `eye` (the eye as the camera
/// controller placed it, before any terrain clamp), else `mover` (the walking character's
/// cell, in walk mode on collision); `None` outdoors. Owner landblock and cell number.
#[must_use]
pub fn camera_cell_at(
    streamer: &WorldStreamer,
    eye: &WorldPos,
    mover: Option<(LandblockKey, u16)>,
) -> Option<(LandblockKey, u16)> {
    streamer
        .cell_at(eye)
        .map(|(k, c)| (k, c.index))
        .filter(|(_, c)| *c != OUTDOORS)
        .or(mover)
}

/// How far above a character's feet its cell is looked up (`walker.md` §13.4, the CELL
/// rule the walker's own mover follows).
pub const CHARACTER_CELL_RAISE: f64 = 0.05;

/// The cell a character belongs to for visibility (`landblock.md` §4.11 "Moving entities",
/// Tessera Decision: the cell its movement code assigns, cell 0 outdoors), with its owner
/// landblock; `None` outdoors:
/// - the user's character in walk mode on collision: the mover's cell (`player_cell`,
///   `Some(None)` when the mover is outdoors);
/// - a placed figure of the landblock data: the cell its static instance is drawn through
///   (`walker.md` §14.12 (h): containment of its origin), so the figure and its static
///   instance agree;
/// - any other character (no collision-based movement code assigns it a cell): the cell
///   lookup of the resident landblocks at its feet raised by [`CHARACTER_CELL_RAISE`], the
///   lookup that movement code would run (`collision.md` §5.8.1).
///
/// `figure_cell` is `ObjectLayer::figure_cell`; `lookup` the cell lookup at a position
/// (the walker passes [`WorldStreamer::cell_at`]'s owner landblock and cell number).
#[must_use]
pub fn character_cell(
    site: &characters::CharacterSite,
    player_cell: Option<Option<(LandblockKey, u16)>>,
    figure_cell: impl Fn(u64) -> Option<(LandblockKey, u16)>,
    lookup: impl Fn(&WorldPos) -> Option<(LandblockKey, u16)>,
) -> Option<(LandblockKey, u16)> {
    if site.player
        && let Some(cell) = player_cell
    {
        return cell;
    }
    if let Some(tag) = site.tag
        && let Some(placed) = figure_cell(tag)
    {
        return (placed.1 != OUTDOORS).then_some(placed);
    }
    let feet = WorldPos::new(
        site.position.region,
        site.position.x,
        site.position.y,
        site.position.z + CHARACTER_CELL_RAISE,
    );
    lookup(&feet).filter(|(_, c)| *c != OUTDOORS)
}

/// The walker (module docs).
pub struct Walker {
    config: WalkerConfig,
    ctx: Arc<WorldContext>,
    set: Arc<ArchiveSet>,
    pool: WorkerPool,
    streamer: WorldStreamer,
    /// The owners of cells reaching the walking character's square from further away
    /// (`walker.md` §13.3, [`movement::CellOwnerIndex`]).
    cell_owners: movement::CellOwnerIndex,
    /// The landblocks the collision walk pins (`walker.md` §13.3: the owner of the
    /// mover's cell), streamed wherever they lie.
    pins: movement::PinStreamers,
    terrain: TerrainLayer,
    objects: ObjectLayer,
    minimap: MinimapLayer,
    shared: Arc<Mutex<SourceShared>>,
    places: Option<PlaceBuild>,
    controls: Controls,
    anchor: Option<Anchor>,
    hud: TextPanel,
    detail: bool,
    hud_at: Option<Instant>,
    rolling: Rolling,
    log: Option<FrameLog>,
    log_start: Instant,
    frame_start: Option<Instant>,
    frame_ms: f64,
    stream_ms: f64,
    uploaded: u64,
    rss: (Option<Instant>, u64),
    location_seq: u64,
    flight: Option<Flight>,
    far: f32,
    help: String,
    /// Walker keys ([`WORLD_KEYS`]) a keymap binding uses: left to the keymap.
    taken_keys: HashSet<tessera_input::KeyCode>,
    avatar: Option<MeshId>,
    move_sent: Option<Instant>,
    viewport: (u32, u32),
    last_report: tessera_world::FrameReport,
    notice: Option<(String, Instant)>,
    characters: CharacterLayer,
    bench: Option<Bench>,
    entity_events: Vec<Event>,
    char_draws: Vec<MeshDraw>,
    player_region: u8,
    /// Main-thread time of the character layer this frame (events, figures, animation).
    chars_ms: f64,
    /// World audio (`None` with audio off).
    audio: Option<crate::audio::WorldAudio>,
    /// Particle effects.
    effects: effects::EffectLayer,
    /// The collision walk mode's mover, loading on its own thread.
    mover_job: Option<std::thread::JoinHandle<movement::setup::MoverSetup>>,
    /// The M4.5 systems in effect (`walker.md` §14.2).
    systems: Systems,
    /// Game clock, sky, sun, fog and lights.
    env: WorldEnv,
    /// Background lookups of sky and frill objects and water surfaces.
    resolver: Resolver,
    /// The creation viewport's scene (`character-screens.md` §2.1.1).
    stage: stage::StageLayer,
    water: WaterLayer,
    frills: FrillLayer,
    /// The environment of this frame.
    frame_env: Option<FrameEnv>,
    /// The frame's light table (render frame).
    lights: Vec<Light>,
    /// The reach of each light of [`Self::lights`] for the renderer's selection
    /// (`lighting.md` §8.5).
    light_reach: Vec<tessera_render::LightReach>,
    /// Start of the material and light clock (`material.md` §5.3.1).
    clock_start: Instant,
    /// The eye of the previous frame (camera travel for fades).
    last_eye: Option<WorldPos>,
    /// Main-thread time of the environment and lights this frame.
    env_ms: f64,
    /// Main-thread time of the frill ring update this frame.
    frills_ms: f64,
    /// The renderer's statistics of the previous frame.
    render_stats: FrameStats,
    /// The scripted camera's start time.
    camera_started: Option<Instant>,
    /// Main-thread phases of this frame, ms: terrain, object and minimap updates; terrain,
    /// object, water, sky and frill recording.
    phase_ms: [f64; 8],
    /// Interior visibility per landblock (`landblock.md` §4.11), built when the camera
    /// enters one of its cells, with the bundle it was built from.
    visibility: HashMap<LandblockKey, (Arc<LandblockBundle>, Arc<BlockVisibility>)>,
    /// Resident landblocks with an outdoor-walk entry cell (§4.12 step 1).
    outdoor_entries: HashSet<LandblockKey>,
    /// The camera's interior cell this frame (owner landblock, cell), `None` outdoors.
    camera_cell: Option<(LandblockKey, u16)>,
    /// The scene flags of this frame (§4.12).
    scene: SceneFlags,
    /// Property IDs of the indoor environment record (§4.13).
    indoor_pids: IndoorPids,
    /// The indoor record's inputs, the landblock they were read for and when (§4.13:
    /// re-read on a landblock change, else at most every 2.5 s).
    indoor: Option<(LandblockKey, Instant, IndoorInputs)>,
    /// Visibility cells drawn in the last frame from inside (HUD).
    interior_drawn: Option<usize>,
    /// Chat overlay and input (M7).
    chat: chat::ChatOverlay,
    /// The HUD's network line (M7).
    net_line: Option<String>,
    /// The far view (`--far-view on`).
    far_view: Option<farview::FarViewLayer>,
    /// The HUD's scenario line (`--scenario`, P0).
    scenario_line: Option<String>,
    /// Draw the HUD text (`walker_hud`, `walker.md` §6).
    show_hud: bool,
    /// Draw the walker minimap (`walker_minimap`, `walker.md` §6).
    show_minimap: bool,
}

/// How often the indoor record's interior overrides are re-read while the camera stays in
/// one landblock's cells (`landblock.md` §4.13).
const INDOOR_REREAD: Duration = Duration::from_millis(2_500);

/// The world context's property schema as an owned [`PropertySchema`] (the object layer
/// decodes cell geometry records with it on its workers).
struct ContextSchema(Arc<WorldContext>);

impl PropertySchema for ContextSchema {
    fn property_type(&self, pid: u32) -> Option<PropertyType> {
        self.0.schema().property_type(pid)
    }
}

impl std::fmt::Debug for Walker {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("Walker")
            .field("anchor", &self.anchor)
            .field("terrain", &self.terrain)
            .field("objects", &self.objects)
            .finish_non_exhaustive()
    }
}

/// The renderer mesh settings the walker prepares streams for (no wireframe edges).
#[must_use]
pub fn mesh_config(gpu_budget: u64) -> MeshConfig {
    MeshConfig {
        wireframe_edges: false,
        max_total_bytes: gpu_budget.max(64 << 20),
        ..MeshConfig::default()
    }
}

impl Walker {
    /// A walker over an opened install and a loaded world context. `shared` is the state
    /// shared with the sandbox's `source::WalkerSource`.
    ///
    /// # Errors
    ///
    /// Invalid streaming settings, or worker threads that cannot be started.
    pub fn new(
        config: WalkerConfig,
        set: Arc<ArchiveSet>,
        ctx: Arc<WorldContext>,
        shared: Arc<Mutex<SourceShared>>,
        catalog: Arc<CharacterCatalog>,
        renderer: &mut Renderer<'_>,
    ) -> anyhow::Result<Self> {
        let pool = WorkerPool::new(PoolConfig {
            thread_name: "tessera-asset".into(),
            ..PoolConfig::default()
        });
        if let Some(f) = &config.flight
            && !ctx.regions().contains_key(&f.region)
        {
            anyhow::bail!("region {} has no landblock data in this install", f.region);
        }
        if !ctx.regions().contains_key(&config.region) {
            tracing::warn!(
                region = config.region,
                "--region names a region without data; names and map coordinates will not resolve"
            );
        }
        let streamer = WorldStreamer::new(Arc::clone(&ctx), &pool, config.stream.clone())?;
        let mesh_cfg = mesh_config(arena_bytes(&config));
        let object_arena = mesh_cfg
            .max_total_bytes
            .saturating_sub(terrain_arena_reserve(config.stream.rings.far));
        // Interior cells drawn with their appearance lists (`landblock.md` §4.14).
        let cell_looks = match catalog.data.registry() {
            Ok(r) => looks::CellLooks::new(r.dictionary(), Arc::clone(ctx.source())).map(Arc::new),
            Err(e) => {
                tracing::warn!("property registry: {e}; cells without their appearance");
                None
            }
        };
        let terrain = TerrainLayer::new(
            config.terrain_threads.max(1),
            LodSelector::from_fov(720.0, FOV_Y, config.pixel_tolerance),
            mesh_cfg,
            Arc::new(BlockInputs {
                looks: cell_looks.clone(),
                ..BlockInputs::from_context(&ctx)
            }),
        )?;
        let objects = ObjectLayer::new(
            Arc::clone(&set),
            &pool,
            config.objects,
            mesh_cfg,
            object_arena,
            Some(Arc::new(ContextSchema(Arc::clone(&ctx)))),
            cell_looks,
        );
        let indoor_pids = match catalog.data.registry() {
            Ok(r) => IndoorPids::from_names(|n| r.dictionary().pid(n)),
            Err(e) => {
                tracing::warn!("property registry: {e}; interiors without their overrides");
                IndoorPids::default()
            }
        };
        let minimap = MinimapLayer::new(Arc::clone(&set), &pool);
        let mut characters = CharacterLayer::new(
            config.characters,
            Arc::new(FigureContext {
                set: Arc::clone(&set),
                catalog: Arc::clone(&catalog),
                materials: objects.materials(),
                mesh_config: mesh_cfg,
                clips: ClipStore::default(),
                looks: tessera_mesh::assembly::look::LookCache::default(),
                assembly: tessera_mesh::assembly::AssemblyOptions {
                    avatar_update_visible: config.characters.updated_avatars,
                    avatar_face_compositing: config.characters.face_compositing,
                },
            }),
            &pool,
        );
        characters.set_lod_classes(ctx.systems().lod_classes.clone());
        {
            let mut s = lock(&shared);
            s.catalog = Some(Arc::clone(&catalog));
            s.interiors = config.interiors;
            s.default_region = config.region;
            s.maps = ctx
                .regions()
                .iter()
                .filter_map(|(r, i)| i.map.map(|m| (*r, m)))
                .collect();
            s.terrain = terrain.tiles().clone();
            s.ready = true;
        }
        let keymap = controls::load_keymap(config.keymap.as_deref());
        let mut controls = Controls::new(&keymap, config.mode, config.fly_speed);
        if let Some(walk) = config.autorun {
            controls.set_autorun(true, walk);
        }
        let taken_keys = keys_taken_by(&keymap);
        let help = format!(
            "{}; [ ] time, \\ clock, -/= rate, F5-F11 layers/shadows/water/sky/scenery/frills/lod",
            controls::help_text(&keymap)
        );
        tracing::info!("walker controls: {help}");
        if !taken_keys.is_empty() {
            tracing::info!(keys = ?taken_keys, "walker keys bound by the keymap; the keymap wins");
        }
        let avatar = match renderer.upload_mesh(&MeshData::uv_sphere(0.5, 16, 8)) {
            Ok(m) => Some(m),
            Err(e) => {
                tracing::warn!("avatar mesh: {e}");
                None
            }
        };
        let now = Instant::now();
        let flight = config.flight.map(|plan| Flight {
            plan,
            wait_until: crate::runtime::deadline_after(now, plan.settle_timeout),
            started: None,
            z: 0.0,
            done: false,
        });
        tracing::info!(
            workers = pool.workers(),
            budget_mib = config.stream.residency_budget >> 20,
            "world walker ready"
        );
        let audio =
            (config.audio.backend != crate::audio::backend::BackendChoice::Off).then(|| {
                crate::audio::WorldAudio::new(
                    config.audio.clone(),
                    Arc::clone(&set),
                    pool.clone(),
                    ctx.options().active_content_layers.clone(),
                )
            });
        let effects = effects::EffectLayer::new(
            config.effects,
            Arc::clone(&set),
            objects.materials(),
            pool.clone(),
            ctx.options().active_content_layers.clone(),
        );
        // The collision walk mode: the mover (capsule, movement record, conditions) loads on
        // its own thread (locating the movement record reads the 0x79 sets).
        let mover_job = if config.collision_walk && ctx.options().build_collision {
            let catalog = Arc::clone(&catalog);
            let avatar = config.avatar.clone();
            std::thread::Builder::new()
                .name("tessera-walk-mover".into())
                .spawn(move || movement::setup::MoverSetup::load(&catalog, &avatar))
                .map_err(|e| tracing::warn!("walk mover thread: {e}; walking on terrain heights"))
                .ok()
        } else {
            None
        };
        // With the far view the sectors reach the landscape extent E beyond the ring.
        let far = far_plane(if config.far_view.enabled {
            config
                .stream
                .rings
                .far
                .max(f64::from(config.far_view.settings.extent()))
        } else {
            config.stream.rings.far
        });
        let config_region = config.region;
        let config_bench = config.bench;
        let env = WorldEnv::new(ctx.systems().sky.clone(), &config.clock);
        let tint = catalog
            .data
            .registry()
            .map(|r| resolver::TintPids {
                colour: r.pid("Render_Color"),
                opacity: r.pid("Render_Opacity"),
            })
            .unwrap_or_default();
        let resolver = Resolver::new(Arc::clone(&ctx), objects.materials(), tint)?;
        let systems = config.systems;
        let far_view = (config.far_view.enabled)
            .then(|| farview::FarViewLayer::new(Arc::clone(&ctx), &config.far_view));
        let cell_owners = movement::CellOwnerIndex::from_context(&ctx);
        Ok(Self {
            far,
            help,
            taken_keys,
            log: config.frame_log.then(FrameLog::default),
            config,
            ctx,
            set,
            pins: movement::PinStreamers::new(Some(pool.clone())),
            cell_owners,
            pool,
            streamer,
            terrain,
            objects,
            minimap,
            shared,
            places: None,
            controls,
            anchor: None,
            hud: TextPanel::default(),
            detail: true,
            hud_at: None,
            rolling: Rolling::default(),
            log_start: now,
            frame_start: None,
            frame_ms: 0.0,
            stream_ms: 0.0,
            uploaded: 0,
            rss: (None, 0),
            location_seq: 0,
            flight,
            avatar,
            move_sent: None,
            viewport: (1280, 720),
            last_report: tessera_world::FrameReport::default(),
            notice: None,
            characters,
            bench: config_bench.map(|plan| Bench {
                plan,
                wait_until: None,
                started: None,
                done: false,
            }),
            entity_events: Vec::new(),
            char_draws: Vec::new(),
            player_region: config_region,
            chars_ms: 0.0,
            audio,
            effects,
            mover_job,
            systems,
            env,
            resolver,
            stage: stage::StageLayer::default(),
            water: WaterLayer::default(),
            frills: FrillLayer::default(),
            frame_env: None,
            lights: Vec::new(),
            light_reach: Vec::new(),
            clock_start: now,
            last_eye: None,
            env_ms: 0.0,
            frills_ms: 0.0,
            render_stats: FrameStats::default(),
            camera_started: None,
            phase_ms: [0.0; 8],
            visibility: HashMap::new(),
            outdoor_entries: HashSet::new(),
            camera_cell: None,
            scene: SceneFlags::OUTDOORS,
            indoor_pids,
            indoor: None,
            interior_drawn: None,
            chat: chat::ChatOverlay::default(),
            net_line: None,
            far_view,
            scenario_line: None,
            show_hud: true,
            show_minimap: true,
        })
    }

    /// The input mapper and its clock (the game UI's step 1, `crate::ui_host`).
    pub fn mapper_mut(&mut self) -> (&mut tessera_input::InputMapper, std::time::Duration) {
        self.controls.mapper_mut()
    }

    /// Attaches or detaches a game UI: Escape then gives `menu` to the UI and game code
    /// instead of quitting, and the overlays follow `hud` and `minimap` (`walker.md`
    /// §5–§6; they draw under the UI pass, which is recorded after [`Walker::draw`]).
    pub fn set_ui_attached(&mut self, on: bool, hud: bool, minimap: bool) {
        self.controls.set_ui_attached(on);
        // The help line names `menu` "menu" while a UI is attached (`walker.md` §5).
        self.help = if on {
            match self.help.strip_suffix(" quit") {
                Some(rest) => format!("{rest} menu"),
                None => self.help.clone(),
            }
        } else {
            match self.help.strip_suffix(" menu") {
                Some(rest) => format!("{rest} quit"),
                None => self.help.clone(),
            }
        };
        self.show_hud = hud;
        self.show_minimap = minimap;
    }

    /// The viewer's minimap input for the game UI (`game-panels.md` §8.3): region,
    /// landblock, global coordinates and compass heading; area and coordinate labels are
    /// not filled yet. `None` before the viewer is placed.
    #[must_use]
    pub fn radar_input(&self) -> Option<tessera_ui::game::radar::RadarInput> {
        let pos = self.focus()?;
        let key = pos.landblock()?;
        let compass = (90.0 - self.controls.heading().to_degrees()).rem_euclid(360.0);
        Some(tessera_ui::game::radar::RadarInput {
            region: pos.region,
            x: key.x(),
            y: key.y(),
            cell: 0,
            global: (pos.x as f32, pos.y as f32),
            heading: compass,
            has_player: true,
            ..tessera_ui::game::radar::RadarInput::default()
        })
    }

    /// Forwards a raw input event. The walker's own keys ([`WORLD_KEYS`], `walker.md`
    /// §14.2–§14.3): `[` and `]` move the time of day by a 24th of a day, `\` stops and
    /// restarts the clock, `-` and `=` halve and double its rate; F5–F11 switch the
    /// terrain layers, shadows, water, sky, generated scenery, frills and per-class
    /// distances. A key that the keymap binds to an action is left to the keymap. The
    /// chat (M7) sees every event first; while its input is open the walker keys are off.
    pub fn input(&mut self, event: &RawEvent) {
        // While the chat input is open, keys are typed text, not walker keys.
        if !self.chat.is_open() {
            self.world_key(event);
        }
        chat::route(&mut self.chat, &mut self.controls, event);
    }

    /// The walker's own keys ([`Self::input`]) without the chat overlay: what a game UI
    /// (M5) forwards when no UI element has the keyboard focus.
    pub fn world_key(&mut self, event: &RawEvent) {
        use tessera_input::KeyCode as K;
        if let RawEvent::Key {
            key,
            pressed: true,
            repeat: false,
        } = event
            && !self.taken_keys.contains(key)
        {
            match key {
                K::BracketLeft => self.env.step_time(-1.0),
                K::BracketRight => self.env.step_time(1.0),
                K::Backslash => {
                    let paused = self.env.toggle_pause();
                    self.notice(if paused {
                        "clock stopped"
                    } else {
                        "clock running"
                    });
                }
                K::Minus => self.env.scale_rate(0.5),
                K::Equal => self.env.scale_rate(2.0),
                K::F5 => self.toggle_system(System::TerrainLayers),
                K::F6 => self.toggle_system(System::Shadows),
                K::F7 => self.toggle_system(System::Water),
                K::F8 => self.toggle_system(System::Sky),
                K::F9 => self.toggle_system(System::Scenery),
                K::F10 => self.toggle_system(System::Frills),
                K::F11 => self.toggle_system(System::Lod),
                _ => {}
            }
        }
    }

    /// Switches system `s` on or off at run time.
    pub fn toggle_system(&mut self, s: System) {
        let on = self.systems.toggle(s);
        self.apply_frill_switch();
        self.notice(format!("{} {}", s.name(), if on { "on" } else { "off" }));
    }

    /// The systems in effect.
    #[must_use]
    pub fn systems(&self) -> Systems {
        self.systems
    }

    /// Hands the frill switch to the streamer's frill ring.
    fn apply_frill_switch(&mut self) {
        let want = self.systems.on(System::Frills);
        let mut s = self.streamer.config().frills;
        if s.draw_frills != want {
            s.draw_frills = want;
            self.streamer.set_frill_settings(s);
            if !want {
                self.frills.clear(&mut self.objects);
            }
        }
    }

    /// Records how long the frame's submission (`end_frame`) and the wait for the GPU took
    /// (frame-log columns; headless runs).
    pub fn record_frame_timing(&mut self, end_frame_ms: f64, gpu_wait_ms: f64) {
        if let Some(log) = &mut self.log {
            log.set_frame_timing(end_frame_ms, gpu_wait_ms);
        }
    }

    /// Records the renderer's statistics of the frame just ended (frame-log columns).
    pub fn record_render_stats(&mut self, stats: &FrameStats) {
        self.render_stats = *stats;
        if let Some(log) = &mut self.log {
            log.set_render_stats(stats);
        }
    }

    /// Sets the viewport size (LOD selection, projected-size culling, HUD layout).
    pub fn set_viewport(&mut self, width: u32, height: u32) {
        self.viewport = (width.max(1), height.max(1));
        let h = self.viewport.1 as f32;
        self.terrain
            .set_selector(LodSelector::from_fov(h, FOV_Y, self.config.pixel_tolerance));
        self.objects
            .set_projection_scale(h / (2.0 * (FOV_Y / 2.0).tan()));
    }

    /// Shows `text` on the HUD for a few seconds.
    pub fn notice(&mut self, text: impl Into<String>) {
        self.notice = Some((text.into(), Instant::now()));
    }

    /// The frame log, when enabled.
    #[must_use]
    pub fn frame_log(&self) -> Option<&FrameLog> {
        self.log.as_ref()
    }

    /// Whether the viewer has been placed in the world.
    #[must_use]
    pub fn placed(&self) -> bool {
        self.controls.placed()
    }

    /// Whether everything wanted around the viewer is loaded and on the GPU.
    #[must_use]
    pub fn settled(&self) -> bool {
        self.controls.placed()
            && self.last_report.loading == 0
            && self.last_report.near_missing == 0
            && self.last_report.deferred == 0
            && self.terrain.settled()
            && self.objects.settled()
            && self.characters.settled()
            && (!self.water_drawn() || self.water.settled())
            && (!self.frills_drawn() || self.frills.settled())
            && (!self.sky_drawn() || self.env.sky_settled(&self.objects))
            && self.resolver.pending() == 0
            && self.far_view_settled()
            && self.effects.settled()
    }

    /// Whether landscape water is drawn: switched on and outdoor-visible (§4.12).
    fn water_drawn(&self) -> bool {
        self.systems.on(System::Water) && self.scene.outdoor_visible
    }

    /// Whether frills are drawn: switched on and outdoor-visible.
    fn frills_drawn(&self) -> bool {
        self.systems.on(System::Frills) && self.scene.outdoor_visible
    }

    /// Whether the sky is drawn: switched on, sky-visible (§4.12) and not hidden by fog
    /// (`environment.md` §11.7 step 10).
    fn sky_drawn(&self) -> bool {
        self.systems.on(System::Sky)
            && self.scene.sky_visible
            && !self.frame_env.as_ref().is_some_and(|f| f.sky_fogged)
    }

    /// What keeps [`Walker::settled`] false (diagnostics), empty when settled.
    #[must_use]
    pub fn unsettled(&self) -> Vec<&'static str> {
        let mut out = Vec::new();
        if !self.controls.placed() {
            out.push("not placed");
        }
        if self.last_report.loading > 0 {
            out.push("landblocks loading");
        }
        if self.last_report.near_missing > 0 {
            out.push("near ring missing");
        }
        if self.last_report.deferred > 0 {
            out.push("landblocks deferred");
        }
        if !self.terrain.settled() {
            out.push("terrain");
        }
        if !self.objects.settled() {
            out.push("objects");
        }
        if !self.characters.settled() {
            out.push("characters");
        }
        if self.water_drawn() && !self.water.settled() {
            out.push("water");
        }
        if self.frills_drawn() && !self.frills.settled() {
            out.push("frills");
        }
        if self.sky_drawn() && !self.env.sky_settled(&self.objects) {
            out.push("sky");
        }
        if self.resolver.pending() > 0 {
            out.push("resolver");
        }
        if !self.far_view_settled() {
            out.push("far view");
        }
        if !self.effects.settled() {
            out.push("particle effects");
        }
        out
    }

    /// A snapshot of the loading work behind [`Walker::settled`]: completion counters and
    /// outstanding-work counts of every layer it checks. Two snapshots differ when some
    /// loading work finished or new work arrived; per-frame draw and timing counters are
    /// left out, so a scene that only renders gives equal snapshots. Headless runs that
    /// wait for the settled state as long as loading progresses compare them
    /// ([`crate::headless::SettleWait`]).
    #[must_use]
    pub fn load_progress(&self) -> LoadProgress {
        let st = self.streamer.stats();
        let t = self.terrain.stats();
        let o = self.objects.stats();
        let c = self.characters.stats();
        LoadProgress {
            placed: self.controls.placed(),
            stream: [st.jobs, st.integrated, st.failed, st.retried],
            report: [
                self.last_report.loading,
                self.last_report.near_missing,
                self.last_report.deferred,
            ],
            terrain: [
                t.built,
                t.meshed,
                t.layered,
                t.incomplete,
                t.pending,
                t.images_waiting,
                t.failed,
            ],
            objects: [
                o.parts_ready,
                o.parts_pending,
                o.parts_failed,
                o.models,
                o.textures_ready,
                o.textures_pending,
            ],
            characters: [
                c.entities,
                c.animated,
                c.posed,
                c.figures_ready,
                c.figures_pending,
                c.figures_failed,
            ],
            water_pending: self.water.stats().pending,
            frills_waiting: self.frills.stats().waiting,
            sky_settled: self.env.sky_settled(&self.objects),
            resolver_pending: self.resolver.pending(),
        }
    }

    /// The character layer (tests and diagnostics).
    #[must_use]
    pub fn characters(&self) -> &CharacterLayer {
        &self.characters
    }

    /// The viewer's position.
    #[must_use]
    pub fn focus(&self) -> Option<WorldPos> {
        self.controls.placed().then(|| self.controls.focus())
    }

    /// Per frame (module docs, steps 1–4). `dt` is the frame time in seconds. Lines typed
    /// in the chat overlay go to `commands`, the client's command dispatcher (decision
    /// 0020), which also gives the lines the client posts.
    pub fn update(
        &mut self,
        renderer: &mut Renderer<'_>,
        world: Option<&mut dyn WalkerSession>,
        commands: &mut dyn crate::plugins::CommandSink,
        now: Instant,
        dt: f32,
    ) -> WalkerEvents {
        if let Some(prev) = self.frame_start {
            self.frame_ms = now.saturating_duration_since(prev).as_secs_f64() * 1e3;
            self.rolling.push(now, self.frame_ms);
        }
        self.frame_start = Some(now);
        let mut events = WalkerEvents::default();

        // Spawn and teleports from the sandbox.
        let mut player_id = None;
        if let Some(w) = world.as_deref() {
            player_id = w.player_id();
            if let Some(loc) = w.player_location() {
                self.player_region = u8::try_from(loc.location.region.0).unwrap_or(0);
            }
        }
        if let Some(w) = world.as_deref()
            && let Some(loc) = w.player_location()
            && loc.seq != self.location_seq
        {
            self.location_seq = loc.seq;
            let r = u8::try_from(loc.location.region.0).unwrap_or(0);
            let pos = WorldPos::new(r, loc.location.x, loc.location.y, loc.location.z);
            if matches!(
                loc.cause,
                LocationCause::Entered | LocationCause::Teleported
            ) {
                // The sandbox has placed the player: its data request is answered.
                lock(&self.shared).focus_request = None;
            }
            match placement(loc.cause, self.controls.placed(), self.flight.is_some()) {
                Placement::Place => {
                    tracing::info!(%r, x = pos.x, y = pos.y, z = pos.z, cause = ?loc.cause, "viewer placed");
                    // A placement is a jump: no cross-fade or sun-change fade runs across it
                    // (`landblock.md` §4.13 Tessera Decision, `environment.md` §11.6 step 2).
                    self.env.mark_immediate();
                    // Placed for the first time: the configured heading; later teleports
                    // keep the session's.
                    let yaw = match (self.controls.placed(), self.config.heading) {
                        (false, Some(h)) => h,
                        _ => loc.heading,
                    };
                    self.controls
                        .place(&pos, yaw, self.config.eye_height, self.config.pitch);
                }
                Placement::Correct => {
                    tracing::debug!(%r, x = pos.x, y = pos.y, z = pos.z, "viewer corrected by the session");
                    self.controls.correct(&pos);
                }
                Placement::Ignore => {}
            }
        }

        // Entity events (ARCHITECTURE §20.3 step 2).
        let mut world = world;
        let t_chars = Instant::now();
        if let Some(w) = world.as_deref_mut() {
            w.drain_entity_events(&mut self.entity_events, MAX_ENTITY_EVENTS_PER_FRAME);
            self.characters.set_look_data(w.look_data());
        }
        self.characters.apply_events(
            &mut self.entity_events,
            &self.shared,
            player_id,
            self.player_region,
            now,
        );
        if let Some(w) = world.as_deref() {
            self.characters
                .apply_samples(&|id| w.sample_entity(id, now), self.player_region, now);
            // Remote sessions only: the sandbox's HUD (and its screenshots) stay as they were.
            self.net_line = w
                .net_status()
                .filter(|s| s.link.is_some())
                .map(|s| hud::net_line(&s));
            self.scenario_line = w.scenario_line(now);
        }
        let mut chars_ms = t_chars.elapsed().as_secs_f64() * 1e3;

        // Input and movement: walk mode on collision once the mover has loaded.
        if self
            .mover_job
            .as_ref()
            .is_some_and(std::thread::JoinHandle::is_finished)
            && let Some(job) = self.mover_job.take()
        {
            match job.join() {
                Ok(mover) => {
                    self.controls.enable_physics(mover);
                    self.notice("walking on collision");
                }
                Err(_) => tracing::error!("walk mover thread panicked; walking on terrain heights"),
            }
        }
        let tiles = self.terrain.tiles().clone();
        let ground = |gx: f64, gy: f64| tiles.height_at(gx, gy);
        // The owners of cells reaching the walking character's square from further away
        // (`walker.md` §13.3): asked for around its feet before the step needs them.
        if self.controls.mode() == MoveMode::Walk
            && let Some(feet) = self.controls.physics().and_then(|p| p.feet().landblock())
        {
            self.cell_owners.update(feet);
        }
        let collision = movement::PinnedCollision {
            main: &self.streamer,
            pins: &self.pins,
            owners: Some(&self.cell_owners),
        };
        let ce = self.controls.frame(dt, &ground, Some(&collision));
        events.quit = ce.quit;
        events.menu = ce.menu;
        events.screenshot = ce.screenshot;
        if ce.toggle_debug {
            self.detail = !self.detail;
        }
        if ce.mode_changed {
            self.notice(format!("{} mode", self.controls.mode().name()));
        }
        if let Some(done) = self.fly(now) {
            events.flight_done = done;
        }
        self.script_camera(now, dt);
        self.update_camera_cell();

        // Streaming focus: the viewer, or where the sandbox needs data (module docs of
        // `source`).
        let request = {
            let s = lock(&self.shared);
            s.focus_request.zip(s.focus_at)
        };
        let focus = stream_focus(
            self.controls.placed().then(|| self.controls.focus()),
            request,
            now,
        );
        if let Some(focus) = focus {
            let t = Instant::now();
            self.last_report = self.streamer.update(focus);
            // The pinned landblocks a step needs wherever they lie (`walker.md` §13.3): the
            // walker's own, and the owners the cell-owner index names.
            let pins = self
                .controls
                .physics()
                .filter(|_| self.controls.mode() == MoveMode::Walk)
                .map(|p| {
                    let collision = movement::PinnedCollision {
                        main: &self.streamer,
                        pins: &self.pins,
                        owners: Some(&self.cell_owners),
                    };
                    p.step_pins(&collision)
                })
                .unwrap_or_default();
            self.pins.update(self.streamer.context(), &pins);
            self.stream_ms = t.elapsed().as_secs_f64() * 1e3;
        }
        // The frill ring follows the camera and the player (`scenery.md` §8.2).
        let t_frills = Instant::now();
        if self.frills_drawn() && self.controls.placed() {
            let (eye, _) = self.view();
            let player = self.controls.focus();
            let _ = self.streamer.update_frills(&eye, &player);
        }
        self.frills_ms = t_frills.elapsed().as_secs_f64() * 1e3;
        self.sync_bundles(renderer);

        // Render anchor follows the eye.
        let eye = if self.controls.placed() {
            Some(self.view().0)
        } else {
            focus
        };
        if let Some(eye) = eye {
            let a = Anchor::containing(&eye);
            if self.anchor.is_some_and(|old| old.region != a.region) {
                self.clear_all(renderer);
            }
            self.anchor = Some(a);
        }

        // The environment of the frame: clock, sky, sun, fog, lights (`walker.md` §14.3).
        let t_env = Instant::now();
        self.resolver.poll();
        if let (Some(anchor), Some(eye_w)) = (self.anchor, eye) {
            let (_, dir) = if self.controls.placed() {
                self.view()
            } else {
                (eye_w, Vec3::X)
            };
            let cam = anchor.to_render(&eye_w).unwrap_or(Vec3::ZERO);
            // Key A at the viewer's position (`environment.md` §7.1.1): the camera's cell
            // with its owner landblock, else the eye's landblock.
            let sky_key = Some(env::camera_sky_key(
                &self.streamer,
                self.camera_cell,
                &eye_w,
            ));
            let indoor = if self.scene.outdoor_visible {
                None
            } else {
                Some(self.indoor_inputs(now, &eye_w))
            };
            let time = self.material_time();
            let fe = self.env.update(
                f64::from(dt),
                sky_key,
                indoor.as_ref(),
                &self.systems,
                self.extent(),
                cam,
                dir,
                time as f32,
            );
            if self.systems.on(System::Lights) {
                let (lights, reach) = self.env.lights(
                    &self.streamer,
                    &anchor,
                    &eye_w,
                    fe.night,
                    time,
                    if self.camera_cell.is_some() {
                        (false, true)
                    } else {
                        (true, self.config.interiors)
                    },
                );
                self.lights = lights;
                self.light_reach = reach;
            } else {
                self.lights.clear();
                self.light_reach.clear();
            }
            self.frame_env = Some(fe);
        }
        self.env_ms = t_env.elapsed().as_secs_f64() * 1e3;

        // The upload time budget runs from here: the work above (streaming, environment)
        // would otherwise leave the uploads one item a frame on a busy frame.
        let mut budget = UploadBudget::new(
            self.config.upload_bytes,
            self.config.upload_time,
            Instant::now(),
        );
        if let (Some(anchor), Some(eye)) = (self.anchor, eye) {
            let cam = anchor.to_render(&eye).unwrap_or(Vec3::ZERO);
            let tframe = self.terrain_frame();
            let t0 = Instant::now();
            self.terrain.update(
                renderer,
                &anchor,
                cam,
                &tframe,
                &mut self.objects,
                &mut budget,
            );
            for (key, product) in self.terrain.take_finished() {
                self.objects.insert(key, product.instances, product.objects);
                self.minimap.set_overview(renderer, key, &product.overview);
            }
            let t1 = Instant::now();
            let oframe = self.object_frame(&eye, dt);
            self.objects
                .update(renderer, [eye.x, eye.y, eye.z], &oframe, &mut budget);
            self.characters.set_distances(characters::CharacterFrame {
                g: oframe.g,
                multiplier: oframe.settings.object_multiplier(),
                shadow_multiplier: oframe.settings.shadow_multiplier(),
                dt: oframe.dt,
                camera_moved: oframe.camera_moved,
                lod: oframe.lod,
            });
            self.last_eye = Some(eye);
            if let Some(fv) = self.far_view.as_mut() {
                let ring = self.config.stream.rings.far;
                let terrain = &self.terrain;
                let near = farview::near_field_of(
                    fv.near_mode(),
                    terrain.keys(),
                    |k| terrain.drawable(k),
                    &eye,
                    ring,
                    fv.near_radius(),
                );
                let view = farview::sprite_view(self.viewport.1, FOV_Y, &self.config.distances);
                fv.update(renderer, &mut self.objects, Some(&eye), near, view);
                // The terrain draws this frame and meshes next frame by the new hand-off.
                self.terrain.set_far_view(Some(fv.hand_off()));
            }
            let t2 = Instant::now();
            if let Some(k) = eye.landblock() {
                let radius = (self.config.minimap_radius / LANDBLOCK_SIZE).ceil() as u32 + 1;
                self.minimap.update(renderer, k, radius, &mut budget);
            }
            let ms = |a: Instant, b: Instant| b.saturating_duration_since(a).as_secs_f64() * 1e3;
            self.phase_ms[0] = ms(t0, t1);
            self.phase_ms[1] = ms(t1, t2);
            self.phase_ms[2] = ms(t2, Instant::now());
        } else if self.stage.scene().is_some() {
            // No viewer yet, with the creation viewport's scene (`character-screens.md`
            // §2.1.1): its parts and their textures load through the object layer, which
            // holds no landblock before world entry.
            let time = self.material_time();
            self.stage
                .update(&mut self.resolver, &mut self.objects, time);
            let oframe = self.unviewed_frame(dt);
            self.objects
                .update(renderer, [0.0; 3], &oframe, &mut budget);
        } else {
            // No viewer yet (the character screens before world entry): the textures of
            // the screens' figure still load.
            self.objects.update_textures(renderer, &mut budget);
        }
        // Characters: the avatar follows the walk controller; figures, animation.
        let pose = self.controls.avatar().map(|(feet, yaw)| {
            let v = self.controls.velocity();
            (feet, yaw, v.x.hypot(v.y))
        });
        self.characters.set_player_pose(pose);
        let tiles = self.terrain.tiles().clone();
        let ground = |r: u8, gx: f64, gy: f64| tiles.height_in(r, gx, gy);
        let t_chars = Instant::now();
        self.characters.update(
            renderer,
            &mut budget,
            &mut self.objects,
            &ground,
            eye,
            now,
            dt,
        );
        chars_ms += t_chars.elapsed().as_secs_f64() * 1e3;
        self.chars_ms = chars_ms;
        self.uploaded = budget.taken;

        // Particle effects (`particle-runtime.md` §3.2): owners, sweep, placement,
        // housekeeping.
        let fx_eye = self.controls.placed().then(|| self.view().0);
        let time = self.material_time();
        self.effects.update(
            effects::EffectInput {
                dt: f64::from(dt),
                now: time,
                streamer: &self.streamer,
                eye: fx_eye,
                characters: &self.characters,
            },
            &mut self.objects,
        );

        // World audio: listener at the camera, the avatar at the player (ROADMAP M6).
        if self.audio.is_some() {
            let placed = self.controls.placed();
            let eye = placed.then(|| {
                let (e, d) = self.view();
                (e, [d.x, d.y, d.z])
            });
            let player = self
                .controls
                .avatar()
                .or_else(|| placed.then(|| (self.controls.focus(), self.controls.heading())));
            // `sound.md` §10.8.6 step 1: the walking character's own cell (physics), never
            // the camera's; in free flight the camera eye's cell lookup.
            let player_cell = if self.controls.mode() == MoveMode::Walk {
                self.controls
                    .physics()
                    .and_then(movement::PhysicsWalker::cell)
            } else {
                camera_cell_at(&self.streamer, &self.controls.eye().0, None)
            };
            if let Some(audio) = self.audio.as_mut() {
                audio.update(crate::audio::AudioInput {
                    dt: f64::from(dt),
                    streamer: &self.streamer,
                    eye,
                    player,
                    player_cell,
                });
            }
        }

        let mut chat_in = Vec::new();
        if let Some(w) = world.as_deref_mut() {
            w.drain_chat(&mut chat_in);
        }
        let notice = self.notice.as_ref().map(|(t, _)| t.as_str());
        if self.chat.receive(
            &chat_in,
            // A scenario owns the chat: its system lines go to the overlay too.
            self.net_line.is_some() || self.scenario_line.is_some(),
            notice,
            now,
        ) {
            self.notice = None;
        }
        chat::dispatch(&mut self.chat, commands, &mut world, now);
        let scale = if self.viewport.1 >= 1000 { 2 } else { 1 };
        self.chat.refresh(renderer, now, scale, self.viewport.0);
        if let Some(w) = world {
            self.report_move(w, now);
        }
        events.duration_done = self.bench_step(now);
        self.pump_places();
        self.refresh_hud(renderer, now);
        events
    }

    /// The timing run: starts logging once settled (or after the settle timeout), returns
    /// true on the frame its duration is over.
    fn bench_step(&mut self, now: Instant) -> bool {
        let placed = self.controls.placed();
        let settled = self.settled();
        let Some(b) = self.bench.as_mut() else {
            return false;
        };
        if b.done || !placed {
            return false;
        }
        let wait = *b
            .wait_until
            .get_or_insert_with(|| crate::runtime::deadline_after(now, b.plan.settle_timeout));
        match b.started {
            None => {
                if settled || now >= wait {
                    tracing::info!(settled, "timing run starts");
                    b.started = Some(now);
                    self.log_start = now;
                    if let Some(log) = &mut self.log {
                        *log = FrameLog::default();
                    }
                    self.rolling = Rolling::default();
                }
                false
            }
            Some(start) => {
                if now.saturating_duration_since(start) >= b.plan.duration {
                    b.done = true;
                    true
                } else {
                    false
                }
            }
        }
    }

    /// The scripted flight: waits for the start area, then moves along the path.
    /// Returns `Some(true)` on the frame it ends.
    fn fly(&mut self, now: Instant) -> Option<bool> {
        let settled = self.settled();
        let tiles = self.terrain.tiles().clone();
        let tiles_height = |g: [f64; 2]| tiles.height_at(g[0], g[1]);
        let f = self.flight.as_mut()?;
        if f.done {
            return Some(false);
        }
        let p = f.plan;
        let yaw = p.path.heading() as f32;
        let started = match f.started {
            Some(s) => s,
            None => {
                let g = p.path.from;
                let h = tiles_height(g).map_or(f.z.max(200.0), f64::from);
                f.z = h + p.height;
                let pos = WorldPos::new(p.region, g[0], g[1], f.z);
                self.controls.set_fly_pose(&pos, yaw, -0.15);
                if settled || now >= f.wait_until {
                    tracing::info!(
                        settled,
                        length = p.path.length(),
                        speed = p.speed,
                        "scripted flight starts"
                    );
                    f.started = Some(now);
                    self.log_start = now;
                    if let Some(log) = &mut self.log {
                        *log = FrameLog::default();
                    }
                    self.rolling = Rolling::default();
                }
                return Some(false);
            }
        };
        let d = now.saturating_duration_since(started).as_secs_f64() * p.speed;
        let g = p.path.at(d);
        if let Some(h) = tiles_height(g) {
            // Follow the terrain, rising quickly and sinking slowly.
            let target = f64::from(h) + p.height;
            f.z = if target > f.z {
                target
            } else {
                f.z + (target - f.z) * 0.02
            };
        }
        let pos = WorldPos::new(p.region, g[0], g[1], f.z);
        self.controls.set_fly_pose(&pos, yaw, -0.15);
        if d >= p.path.length() {
            f.done = true;
            return Some(true);
        }
        Some(false)
    }

    /// The landscape extent in force (`walker.md` §11.3): the far ring's radius, which the
    /// host sets to *E* of the distance preferences unless `--view-blocks` overrides it; with
    /// the far view on, *E* of its settings (`far-view.md` §7.1), whatever the full-detail
    /// ring.
    fn extent(&self) -> f32 {
        match &self.far_view {
            Some(fv) => fv.extent(),
            None => self.ring_extent(),
        }
    }

    /// The radius of the full-detail ring (terrain and landblock water are drawn within it).
    fn ring_extent(&self) -> f32 {
        self.config.stream.rings.far as f32
    }

    /// The fallback diagnostic of the last frame: object submesh draws without their
    /// base-colour texture, by reason and surface, and the draws of records with programs of
    /// their own drawn through the generic path ([`fallback`]).
    #[must_use]
    pub fn fallback_stats(&self) -> fallback::FallbackStats {
        self.objects.fallback_stats().clone()
    }

    /// The landscape-water drawings of the last frame, with their parameters at the frame's
    /// material clock (`water.md` §8.7).
    #[must_use]
    pub fn water_drawings(&self) -> &[water::DrawnWater] {
        self.water.drawn()
    }

    /// The material and light clock now, seconds since the walker started
    /// (`material.md` §5.3.1).
    #[must_use]
    pub fn material_clock(&self) -> f64 {
        self.material_time()
    }

    /// The far view's counters of the last frame (`None` with the far view off).
    #[must_use]
    pub fn far_view_stats(&self) -> Option<farview::FarViewStats> {
        self.far_view.as_ref().map(farview::FarViewLayer::stats)
    }

    /// Whether the far view is in (true with it off or while the outdoors is not visible).
    fn far_view_settled(&self) -> bool {
        !self.scene.outdoor_visible
            || self
                .far_view
                .as_ref()
                .is_none_or(|fv| fv.settled(&self.objects))
    }

    /// Seconds on the material and light clock (`material.md` §5.3.1: since the start).
    fn material_time(&self) -> f64 {
        self.clock_start.elapsed().as_secs_f64()
    }

    /// How the terrain is drawn this frame.
    fn terrain_frame(&self) -> TerrainFrame {
        TerrainFrame {
            layered: self.systems.on(System::TerrainLayers),
            lit: self.systems.on(System::Lit),
            shadow_radius: if self.systems.on(System::Shadows) {
                3.0 * env::SHADOW_RADIUS + LANDBLOCK_SIZE as f32
            } else {
                0.0
            },
            extent: self.ring_extent(),
            far_band: self.config.far_band,
            time: self.material_time(),
        }
    }

    /// How objects are chosen and drawn this frame (`walker.md` §11.4, §14.5).
    /// The object layer's frame before world entry (no viewer): the walker's settings with
    /// a still camera and no interior.
    fn unviewed_frame(&self, dt: f32) -> ObjectFrame {
        let (fps, _, _) = self.rolling.stats();
        ObjectFrame {
            g: self.extent(),
            settings: self.config.distances,
            lod: self.systems.on(System::Lod),
            instancing: self.systems.on(System::Instancing),
            scenery: self.systems.on(System::Scenery),
            lit: self.systems.on(System::Lit),
            shadows: self.systems.on(System::Shadows),
            dt,
            fps: if fps > 0.0 { fps as f32 } else { 60.0 },
            camera_moved: 0.0,
            interior: None,
            all_interiors: self.config.interiors,
            time: self.material_time(),
        }
    }

    fn object_frame(&self, eye: &WorldPos, dt: f32) -> ObjectFrame {
        let (fps, _, _) = self.rolling.stats();
        let moved = self.last_eye.map_or(f32::INFINITY, |l| {
            if l.region == eye.region {
                ((l.x - eye.x).powi(2) + (l.y - eye.y).powi(2) + (l.z - eye.z).powi(2)).sqrt()
                    as f32
            } else {
                f32::INFINITY
            }
        });
        ObjectFrame {
            g: self.frame_env.as_ref().map_or(self.extent(), |f| f.g),
            settings: self.config.distances,
            lod: self.systems.on(System::Lod),
            instancing: self.systems.on(System::Instancing),
            scenery: self.systems.on(System::Scenery),
            lit: self.systems.on(System::Lit),
            shadows: self.systems.on(System::Shadows),
            dt,
            fps: if fps > 0.0 { fps as f32 } else { 60.0 },
            camera_moved: moved,
            interior: self.camera_cell.map(|(k, _)| k),
            all_interiors: self.config.interiors,
            time: self.material_time(),
        }
    }

    /// Finds the camera's interior cell (`landblock.md` §4.12 Tessera Decision): the cell
    /// lookup at the eye; when it finds none but the walking character's feet are in a
    /// cell, the character's cell. Then the frame's scene flags by the pass of
    /// `landblock.md` §4.12: inside, the cell visited with its PVS; outdoors, both set and
    /// the outdoor-walk entry cells of the resident landblocks visited with theirs.
    fn update_camera_cell(&mut self) {
        let (eye, _) = self.controls.eye();
        let mover = if self.controls.mode() == MoveMode::Walk {
            self.controls
                .physics()
                .and_then(movement::PhysicsWalker::cell)
        } else {
            None
        };
        let cell = camera_cell_at(&self.streamer, &eye, mover);
        self.camera_cell = cell;
        self.scene = match cell {
            Some((k, c)) => self
                .visibility_of(k)
                .map_or(SceneFlags::CLOSED, |v| v.scene_flags(c)),
            None => {
                // The outdoor branch visits the outdoor-walk entry cells of every loaded
                // landblock (none in this install, LI8), in landblock order.
                let mut keys: Vec<LandblockKey> = self.outdoor_entries.iter().copied().collect();
                keys.sort_unstable();
                let blocks: Vec<Arc<BlockVisibility>> = keys
                    .into_iter()
                    .filter_map(|k| self.visibility_of(k))
                    .collect();
                interiors::outdoor_flags(blocks.iter().map(|b| &**b))
            }
        };
    }

    /// The visibility of landblock `key`'s cells (`landblock.md` §4.11), built from its
    /// resident bundle (portal polygons from its collision set) and kept while that
    /// bundle stays resident.
    fn visibility_of(&mut self, key: LandblockKey) -> Option<Arc<BlockVisibility>> {
        let bundle = self.streamer.bundle(key)?;
        if let Some((b, v)) = self.visibility.get(&key)
            && Arc::ptr_eq(b, bundle)
        {
            return Some(Arc::clone(v));
        }
        let info = bundle.info.as_ref()?;
        let col = bundle.collision.as_deref();
        let vis = Arc::new(BlockVisibility::build(info, |cell, k| {
            col.and_then(|c| c.cell(cell))
                .and_then(|c| c.portals.get(k))
                .map(|p| p.corners.clone())
                .unwrap_or_default()
        }));
        tracing::debug!(
            landblock = %key,
            visibility_cells = vis.visibility_cells(),
            portals = vis.portals().len(),
            "interior visibility built"
        );
        let bundle = Arc::clone(bundle);
        self.visibility
            .retain(|k, _| self.streamer.bundle(*k).is_some());
        self.visibility.insert(key, (bundle, Arc::clone(&vis)));
        Some(vis)
    }

    /// The indoor record's inputs at the camera position (`landblock.md` §4.13): looked up
    /// at [`env::lookup_target`] (in a cell: its owner's landblock info and block map, the
    /// cell's own set not read, L32; otherwise the landblock info and block map under the
    /// eye); re-read when that landblock changes and otherwise at most every
    /// [`INDOOR_REREAD`].
    fn indoor_inputs(&mut self, now: Instant, eye: &WorldPos) -> IndoorInputs {
        let (key, _cell) = env::lookup_target(self.camera_cell, eye);
        let Some(key) = key else {
            return IndoorInputs::read(&self.indoor_pids, |_| None);
        };
        if let Some((k, at, inputs)) = &self.indoor
            && *k == key
            && now.saturating_duration_since(*at) < INDOOR_REREAD
        {
            return *inputs;
        }
        let inputs =
            env::indoor_inputs_at(self.streamer.bundle(key).map(|b| &**b), &self.indoor_pids);
        self.indoor = Some((key, now, inputs));
        inputs
    }

    /// The scripted camera of a timing run (`m4-5-visual.md` §3): it waits at its start
    /// until the timing run starts (or at once without one), then flies the loop at its
    /// speed and height over the higher of terrain and water, or turns on the spot.
    fn script_camera(&mut self, now: Instant, dt: f32) {
        let Some(plan) = self.config.camera.clone() else {
            return;
        };
        if !self.controls.placed() {
            return;
        }
        let running = self.bench.is_none_or(|b| b.started.is_some());
        let t = if running {
            let start = *self.camera_started.get_or_insert(now);
            now.saturating_duration_since(start).as_secs_f64()
        } else {
            0.0
        };
        match plan {
            CameraPlan::Loop {
                path,
                speed,
                height,
                pitch,
            } => {
                let (p, heading) = path.at(t * speed);
                let g = path.global(p);
                let ground = self.terrain.tiles().height_in(path.region, g[0], g[1]);
                let water = self
                    .streamer
                    .landblock_water(&WorldPos::new(path.region, g[0], g[1], 0.0))
                    .map(|w| w.height);
                let base = match (ground, water) {
                    (Some(a), Some(b)) => Some(a.max(b)),
                    (a, b) => a.or(b),
                };
                let z = base.map_or_else(|| self.controls.focus().z, |h| f64::from(h) + height);
                let pos = WorldPos::new(path.region, g[0], g[1], z);
                self.controls.set_fly_pose(&pos, heading as f32, pitch);
            }
            CameraPlan::Turn { rate } => {
                let (eye, dir) = self.controls.eye();
                let yaw = dir.y.atan2(dir.x);
                let pitch = dir.z.clamp(-1.0, 1.0).asin();
                let step = if running && dt.is_finite() { dt } else { 0.0 };
                self.controls.set_fly_pose(&eye, yaw - rate * step, pitch);
            }
        }
    }

    /// Tracks new and released bundles.
    fn sync_bundles(&mut self, renderer: &mut Renderer<'_>) {
        let layers = self.ctx.options().active_content_layers.clone();
        let active = |l: u32| layers.contains(&l);
        let mut resident: HashSet<LandblockKey> = HashSet::new();
        let mut fresh = Vec::new();
        for (key, _ring, bundle) in self.streamer.resident() {
            resident.insert(key);
            if !self.terrain.tracks(key, bundle) {
                fresh.push((key, Arc::clone(bundle)));
            }
        }
        {
            let mut s = lock(&self.shared);
            for (key, _ring, bundle) in self.streamer.resident() {
                let stale = !matches!(s.figures.get(&key), Some(FigureBlock::Ready(b)) if Arc::ptr_eq(b, bundle));
                if stale {
                    s.figures
                        .insert(key, FigureBlock::Ready(Arc::clone(bundle)));
                }
            }
            s.figures.retain(|k, f| match f {
                FigureBlock::Ready(_) => resident.contains(k),
                FigureBlock::Missing => true,
            });
            // Landblocks around the viewer that failed (or have no data) have no figures.
            if let Some(k) = self
                .controls
                .placed()
                .then(|| self.controls.focus().landblock())
                .flatten()
            {
                for dy in -3i32..=3 {
                    for dx in -3i32..=3 {
                        let Some(n) = k.offset(dx, dy) else { continue };
                        if matches!(
                            self.streamer.state_of(n),
                            Some(tessera_world::BlockState::Failed(_))
                        ) {
                            s.figures.insert(n, FigureBlock::Missing);
                        } else if matches!(s.figures.get(&n), Some(FigureBlock::Missing))
                            && resident.contains(&n)
                        {
                            s.figures.remove(&n);
                        }
                    }
                }
                s.figures.retain(|n, _| {
                    n.region() == k.region()
                        && n.x().abs_diff(k.x()) <= 8
                        && n.y().abs_diff(k.y()) <= 8
                        || resident.contains(n)
                });
            }
        }
        for (key, bundle) in fresh {
            if bundle
                .info
                .as_ref()
                .is_some_and(interiors::has_outdoor_entry)
            {
                self.outdoor_entries.insert(key);
            } else {
                self.outdoor_entries.remove(&key);
            }
            if self
                .terrain
                .insert(key, &bundle, renderer, &mut self.objects)
            {
                self.minimap
                    .set_image(key, bundle.minimap_image(active).map(|d| d.0));
                if let Some(a) = &bundle.area {
                    lock(&self.shared).areas.insert(key, Arc::clone(a));
                }
            }
        }
        let gone: Vec<LandblockKey> = self
            .terrain
            .keys()
            .filter(|k| !resident.contains(k))
            .collect();
        for key in gone {
            self.outdoor_entries.remove(&key);
            self.terrain.remove(key, renderer, &mut self.objects);
            self.objects.remove(key);
            self.minimap.remove(renderer, key);
            lock(&self.shared).areas.remove(&key);
        }
    }

    /// Drops everything drawn (a region change, shutdown). Every user of the object layer
    /// releases its references first, so it acquires them again on the next frame (the
    /// water surfaces and sky parts would otherwise stay pending for ever).
    fn clear_all(&mut self, renderer: &mut Renderer<'_>) {
        self.pins.clear();
        self.terrain.clear(renderer, &mut self.objects);
        self.frills.clear(&mut self.objects);
        self.water.release(&mut self.objects);
        self.env.release(&mut self.objects);
        if let Some(fv) = self.far_view.as_mut() {
            fv.clear(&mut self.objects);
        }
        let keys: Vec<LandblockKey> = self.objects.keys().collect();
        for k in keys {
            self.objects.remove(k);
        }
        self.characters.reset_gpu(renderer, &mut self.objects);
        self.objects.clear(renderer);
        self.minimap.clear(renderer);
        lock(&self.shared).areas.clear();
    }

    fn report_move(&mut self, world: &mut dyn WalkerSession, now: Instant) {
        if !self.controls.placed()
            || self
                .move_sent
                .is_some_and(|t| now.saturating_duration_since(t) < MOVE_INTERVAL)
        {
            return;
        }
        let p = self.controls.focus();
        let v = self.controls.velocity();
        world.send_move(
            now,
            tessera_net::Location::new(RegionId(u16::from(p.region)), p.x, p.y, p.z),
            self.controls.heading(),
            tessera_net::Vec3::new(v.x, v.y, v.z),
        );
        self.move_sent = Some(now);
    }

    fn pump_places(&mut self) {
        let requested = matches!(lock(&self.shared).places, Places::Requested);
        if requested && self.places.is_none() {
            match PlaceBuild::spawn(Arc::clone(&self.set), Arc::clone(&self.ctx)) {
                Ok(b) => {
                    self.notice("building the place index…");
                    self.places = Some(b);
                }
                Err(e) => {
                    tracing::error!("place index: {e}");
                    lock(&self.shared).places =
                        Places::Ready(Arc::new(places::PlaceIndex::default()));
                }
            }
        }
        if let Some(b) = &mut self.places
            && let Some(index) = b.try_take()
        {
            lock(&self.shared).places = Places::Ready(Arc::new(index));
            self.places = None;
        }
    }

    fn refresh_hud(&mut self, renderer: &mut Renderer<'_>, now: Instant) {
        if self
            .hud_at
            .is_some_and(|t| now.saturating_duration_since(t) < HUD_INTERVAL)
        {
            return;
        }
        self.hud_at = Some(now);
        if self
            .rss
            .0
            .is_none_or(|t| now.saturating_duration_since(t) >= RSS_INTERVAL)
        {
            self.rss = (Some(now), rss_bytes());
        }
        let lines = self.hud_lines(now);
        let scale = if self.viewport.1 >= 1000 { 2 } else { 1 };
        self.hud.set(renderer, lines, scale);
    }

    fn hud_lines(&self, now: Instant) -> Vec<(String, [u8; 3])> {
        const WHITE: [u8; 3] = [240, 240, 235];
        const GOLD: [u8; 3] = [255, 214, 120];
        const GREY: [u8; 3] = [180, 190, 200];
        const RED: [u8; 3] = [255, 120, 100];
        let mut out = Vec::new();
        let (fps, mean, max) = self.rolling.stats();
        let mode = if self.flight.is_some() {
            "scripted flight"
        } else {
            self.controls.mode().name()
        };
        out.push((
            format!(
                "Tessera world walker [{mode}]  {fps:.0} fps  {mean:.1} ms avg  {max:.1} ms max(1s)"
            ),
            WHITE,
        ));
        if let Some(net) = &self.net_line {
            out.push((net.clone(), GREY));
        }
        if let Some(line) = &self.scenario_line {
            out.push((line.clone(), GOLD));
        }
        let Some(pos) = self.focus() else {
            let s = lock(&self.shared);
            let what = match (&s.places, s.focus_request) {
                (Places::Requested, _) => "building the place index",
                (_, Some(_)) => "loading the spawn area",
                _ => "waiting for the world",
            };
            out.push((format!("Spawning: {what}..."), GOLD));
            return out;
        };
        let s = lock(&self.shared);
        let area = pos.landblock().and_then(|k| s.areas.get(&k));
        let area_text = area.map_or_else(
            || "(area unknown)".to_owned(),
            |a| {
                let mut t = a.display().unwrap_or("(unnamed area)").to_owned();
                if let Some(terr) = a.territory.as_deref()
                    && Some(terr) != a.name.as_deref()
                {
                    t.push_str(" - ");
                    t.push_str(terr);
                }
                if let Some(reg) = a.region.as_deref() {
                    t.push_str(", ");
                    t.push_str(reg);
                }
                t
            },
        );
        out.push((area_text, GOLD));
        let map = s
            .maps
            .get(&pos.region)
            .map_or_else(|| "-".to_owned(), |c| c.to_map(pos.xy()).to_string());
        drop(s);
        out.push((format!("Map {map}"), WHITE));
        let heading = self.controls.heading();
        out.push((loc_text(&pos, heading), GREY));
        if let Some(fv) = &self.far_view {
            out.push((fv.hud_line(), GREY));
        }
        if let Some((k, c)) = self.camera_cell {
            out.push((
                format!(
                    "Inside: cell {c} of {k}, {} visibility cells drawn{}{}",
                    self.interior_drawn.unwrap_or(0),
                    if self.scene.sky_visible { ", sky" } else { "" },
                    if self.scene.outdoor_visible {
                        ", landscape"
                    } else {
                        ""
                    },
                ),
                GREY,
            ));
        }
        if self.controls.mode() == MoveMode::Walk
            && let Some(w) = self.controls.physics()
        {
            let st = w.state();
            let cell = w.cell().map_or_else(
                || "outdoors".to_owned(),
                |(k, c)| format!("cell {c} of {k}"),
            );
            let water = st
                .water
                .map_or_else(String::new, |w| format!(", water {:.2} m deep", w.depth));
            out.push((
                format!(
                    "Walk on collision: state {} ({:?}), {cell}{water}",
                    st.element.key(),
                    st.element
                ),
                GREY,
            ));
        }
        if let Some(f) = &self.frame_env {
            let hours = f.time_of_day * 24.0;
            out.push((
                format!(
                    "Day {} {:02}:{:02} (t {:.3}, rate {:.2}){}  sky key {}  day {}  G {:.0} m{}",
                    f.day,
                    hours.floor() as u32 % 24,
                    ((hours.fract()) * 60.0).floor() as u32,
                    f.time_of_day,
                    self.env.rate(),
                    if f.night { " night" } else { "" },
                    f.sky_key
                        .map_or_else(|| "-".to_owned(), |k| format!("{k:#x}")),
                    f.day_description
                        .map_or_else(|| "-".to_owned(), |d| format!("{d}")),
                    f.g,
                    f.fog_far
                        .map_or_else(String::new, |far| format!(", fog to {far:.0} m")),
                ),
                GREY,
            ));
        }
        if let Some(k) = pos.landblock() {
            let l = pos.to_local().map_or([0.0; 3], |l| l.local);
            out.push((
                format!(
                    "Region {}  landblock ({}, {}) {:#010x}  local ({:.1}, {:.1}, {:.1})",
                    k.region(),
                    k.x(),
                    k.y(),
                    k.data_did().0,
                    l[0],
                    l[1],
                    l[2]
                ),
                GREY,
            ));
        }
        let r = &self.last_report;
        let mib = |b: u64| b as f64 / f64::from(1 << 20);
        let ts = self.terrain.stats();
        let os = self.objects.stats();
        let ms = self.minimap.stats();
        out.push((
            format!(
                "Resident {} landblocks, {:.1}/{} MiB; loading {}{}",
                r.resident_blocks,
                mib(r.resident_bytes as u64),
                self.config.stream.residency_budget >> 20,
                r.loading,
                if r.near_missing > 0 {
                    format!(", {} near missing", r.near_missing)
                } else {
                    String::new()
                }
            ),
            if r.near_missing > 0 { RED } else { WHITE },
        ));
        out.push((
            format!(
                "GPU {:.0} MiB (terrain {:.0}, meshes {:.0}, textures {:.0}); RSS {:.0} MiB",
                mib(ts.gpu_bytes + os.mesh_bytes + os.texture_bytes),
                mib(ts.gpu_bytes),
                mib(os.mesh_bytes),
                mib(os.texture_bytes),
                mib(self.rss.1)
            ),
            WHITE,
        ));
        if self.detail {
            out.push((
                format!(
                    "Terrain {}/{} meshed, {} pending, {} drawn ({:.2} M tris){}",
                    ts.meshed,
                    ts.blocks,
                    ts.pending,
                    ts.drawn,
                    ts.triangles as f64 / 1e6,
                    if ts.failed > 0 || ts.panics > 0 {
                        format!(", {} failed, {} job panics", ts.failed, ts.panics)
                    } else {
                        String::new()
                    }
                ),
                if ts.failed > 0 || ts.panics > 0 {
                    RED
                } else {
                    GREY
                },
            ));
            out.push((
                format!(
                    "Objects {} blocks ({:.0} m cap), {} active of {}, parts {} ready / {} pending / {} failed, \
                     textures {} / {} pending",
                    os.active_blocks,
                    os.distance,
                    os.active_objects,
                    os.objects,
                    os.parts_ready,
                    os.parts_pending,
                    os.parts_failed,
                    os.textures_ready,
                    os.textures_pending
                ),
                GREY,
            ));
            out.push((
                format!(
                    "Draws {}{}, culled {}; minimap {}+{}; streamer {:.2} ms; upload {:.1} MiB; workers {} (queue {})",
                    os.draws,
                    if os.capped { " (capped)" } else { "" },
                    os.culled,
                    ms.drawn_tiles,
                    ms.drawn_overviews,
                    self.stream_ms,
                    mib(self.uploaded),
                    self.pool.workers(),
                    self.pool.queue_len()
                ),
                GREY,
            ));
            let ws = self.water.stats();
            let fs = self.frills.stats();
            out.push((
                format!(
                    "Layers {} blocks ({} waiting for textures), {} ceilings; objects {} drawn ({} fading, \
                     {} instanced in {} batches, {} models); water {} quads; frills {} groups / {} \
                     instances; lights {}; sky {}; off: {}",
                    ts.layered,
                    ts.incomplete,
                    ts.ceilings,
                    os.objects_drawn,
                    os.fading,
                    os.instanced,
                    os.batches,
                    os.models,
                    ws.quads,
                    fs.groups,
                    fs.instances,
                    self.lights.len(),
                    self.frame_env.as_ref().map_or(0, |f| f.sky_objects),
                    self.systems.disabled_text(),
                ),
                GREY,
            ));
            let fb = self.objects.fallback_stats();
            out.push((fb.hud_line(), if fb.draws > 0 { GOLD } else { GREY }));
            let cs = self.characters.stats();
            out.push((
                format!(
                    "Characters {} ({} animated, {} drawn, {} sampled); figures {} ready / {} \
                     pending / {} failed, {:.0} MiB; anim main {:.2} ms, workers {:.2} ms",
                    cs.entities,
                    cs.animated,
                    cs.drawn,
                    cs.sampled,
                    cs.figures_ready,
                    cs.figures_pending,
                    cs.figures_failed,
                    mib(cs.gpu_bytes),
                    cs.collect_ms + cs.submit_ms + cs.palette_ms,
                    cs.worker_ms
                ),
                GREY,
            ));
            if let Some(a) = &self.audio {
                out.push((a.hud_line(), GREY));
            }
            out.push((self.effects.hud_line(), GREY));
            if self.flight.is_none() {
                out.push((
                    format!("{} (fly {:.0} m/s)", self.help, self.controls.fly_speed()),
                    GREY,
                ));
            }
        }
        if let Some((text, at)) = &self.notice
            && now.saturating_duration_since(*at) < Duration::from_secs(6)
        {
            out.push((text.clone(), GOLD));
        }
        out
    }

    /// Draws the player's figure alone into `frame` as the paperdoll's portrait
    /// (`character-panels.md` §3.2, `render.md` §6.10): a premultiplied-alpha pass
    /// (`Pass::premultiplied_alpha`, the external-content target's contract) cleared to
    /// transparent black, the
    /// figure at the origin turned by `yaw_degrees` (the paperdoll's turn), seen from its
    /// front at the height of its bounding sphere's centre, unlit with the walker's fixed
    /// light. Returns whether a figure was drawn (`false` while it is not on the GPU).
    pub fn draw_portrait(&mut self, frame: &mut Frame, yaw_degrees: f32) -> bool {
        let player = self.characters.player_id();
        self.draw_alone(frame, player, yaw_degrees, false)
    }

    /// Shows the character screens' figure (`character-screens.md` §2.1, §6) or removes it
    /// (`None`, which also removes the scene); see [`CharacterLayer::set_preview`]. Call
    /// every frame the screens are open with their current figure
    /// ([`crate::screens::viewport::figure_job`]).
    pub fn set_preview(
        &mut self,
        job: Option<(
            characters::figure::FigureRequest,
            characters::figure::LookChoices,
        )>,
    ) {
        if job.is_none() {
            self.stage.set_scene(None, &mut self.objects);
        }
        self.characters.set_preview(job, Instant::now());
    }

    /// Shows the race scene around the character screens' figure (`character-screens.md`
    /// §2.1.1), or none. Call every frame the screens are open (an unchanged scene costs a
    /// comparison).
    pub fn set_preview_scene(&mut self, scene: Option<Arc<crate::screens::scene::StageScene>>) {
        self.stage.set_scene(scene, &mut self.objects);
    }

    /// The scene around the screens' figure: (objects with geometry, objects skipped,
    /// parts referenced, lights) and whether everything has loaded.
    #[must_use]
    pub fn preview_scene(&self) -> Option<((usize, usize, usize, usize), bool)> {
        self.stage.scene()?;
        Some((self.stage.counts(), self.stage.settled(&self.objects)))
    }

    /// The character screens' figure: what was requested and how far it has come.
    #[must_use]
    pub fn preview(&self) -> Option<characters::FigureStatus> {
        self.characters.preview()
    }

    /// The box of the screens' figure as posed now, standing at `view`'s figure frame
    /// ([`characters::CharacterLayer::posed_bounds`] with [`PREVIEW_PICK_PAD`]): what the
    /// screens test the pointer's ray against (`character-screens.md` §2.3 Tessera rule).
    #[must_use]
    pub fn preview_bounds(&self, view: &stage::StageView) -> Option<(Vec3, Vec3)> {
        self.characters.posed_bounds(
            characters::PREVIEW_ID,
            &|s| view.figure_transform(s),
            PREVIEW_PICK_PAD,
        )
    }

    /// Whether the screens' figure for the current request is still loading (§4.3 loading
    /// notice and race blocker).
    #[must_use]
    pub fn preview_loading(&self) -> bool {
        self.characters.preview_loading()
    }

    /// Whether the screens' figure and scene have loaded: every figure on the GPU and
    /// posed, every part and texture the object layer holds on the GPU (or failed for
    /// good), every scene object resolved (tests and screenshots).
    #[must_use]
    pub fn preview_settled(&self) -> bool {
        self.characters.settled()
            && self.objects.settled()
            && self.stage.settled(&self.objects)
            && self.resolver.pending() == 0
    }

    /// Draws the creation viewport (`character-screens.md` §2.1.1–§2.1.3) into `frame`:
    /// with a `view`, the scene set by [`Walker::set_preview_scene`] lit by its own lights
    /// and the viewport's ambient, the screens' figure standing at the view's figure frame
    /// and lit like the scene, through the view's camera, on opaque black; without one
    /// (no camera frame for the stage, §2.1.3 OPEN) the figure alone as the paperdoll's
    /// portrait. Returns whether the figure was drawn.
    pub fn draw_preview(&mut self, frame: &mut Frame, view: Option<&stage::StageView>) -> bool {
        let Some((view, camera)) = view.and_then(|v| v.camera().map(|c| (*v, c))) else {
            return self.draw_alone(frame, Some(characters::PREVIEW_ID), 0.0, false);
        };
        let mut draws = Vec::new();
        let place = |scale: Vec3| view.figure_transform(scale);
        let bounds = self.characters.placed_draws(
            frame,
            characters::PREVIEW_ID,
            &place,
            true,
            &self.objects,
            &mut draws,
        );
        let time = self.material_time() as f32;
        let (lighting, reach) = self.stage.lighting(view.ambient, time);
        let pass = frame.world_pass("chargen-viewport", &camera);
        pass.premultiplied_alpha();
        pass.clear([0.0, 0.0, 0.0, 1.0]);
        pass.frame_lighting(lighting);
        pass.light_reach(reach);
        if view.scene.is_some() {
            self.stage.draw(pass, &mut self.objects);
        }
        for d in draws {
            pass.mesh(d);
        }
        bounds.is_some()
    }

    /// One figure alone into `frame` ([`Walker::draw_portrait`]).
    fn draw_alone(
        &mut self,
        frame: &mut Frame,
        id: Option<EntityId>,
        yaw_degrees: f32,
        zoomed: bool,
    ) -> bool {
        let Some(id) = id else {
            frame
                .world_pass("portrait", &Camera::default())
                .premultiplied_alpha();
            return false;
        };
        let mut draws = Vec::new();
        let yaw = yaw_degrees.to_radians();
        let bounds = self
            .characters
            .portrait_draws(frame, id, yaw, &self.objects, &mut draws);
        let (centre, radius) = bounds.unwrap_or((Vec3::new(0.0, 0.0, 1.0), 1.0));
        let radius = if radius.is_finite() && radius > 0.01 {
            radius
        } else {
            1.0
        };
        // Zoomed: half the distance, aimed a quarter radius above the centre (the head and
        // upper body of a standing figure).
        let (centre, fill) = if zoomed {
            (
                centre + Vec3::Z * (radius * PREVIEW_ZOOM_RAISE),
                PORTRAIT_FILL * PREVIEW_ZOOM_FILL,
            )
        } else {
            (centre, PORTRAIT_FILL)
        };
        // The figure faces its local +Y at yaw 0: the camera looks back at it along −Y
        // from in front, at a distance where the bounding sphere's diameter fills about the
        // view's height.
        let fov = PORTRAIT_FOV_DEGREES.to_radians();
        let distance = radius / (fov * 0.5).tan() * fill;
        let camera = Camera {
            eye: centre + Vec3::Y * distance,
            target: centre,
            up: Vec3::Z,
            projection: Perspective {
                fov_y: fov,
                near: (distance - radius * 2.0).max(0.05),
                far: distance + radius * 2.0,
            },
        };
        let pass = frame.world_pass("portrait", &camera);
        // `render.md` §6.10: fixed blends write the target's alpha by the premultiplied
        // rule (an opaque "destination ZERO" surface covers what lies behind the viewport).
        pass.premultiplied_alpha();
        pass.lighting(Lighting {
            direction: Vec3::new(0.6, -0.3, 0.75),
            color: [1.0, 0.97, 0.9],
            ambient: [0.45, 0.46, 0.5],
        });
        let drawn = !draws.is_empty();
        for d in draws {
            pass.mesh(d);
        }
        drawn
    }

    /// Records the frame: the world pass (terrain, objects, avatar) and the overlay (HUD,
    /// minimap). Ends the frame's main-thread timing and logs it.
    pub fn draw(&mut self, renderer: &Renderer<'_>, frame: &mut Frame) {
        let (w, h) = frame.size();
        if (w.max(1), h.max(1)) != self.viewport {
            // Level selection and the projected-size cull follow the frame actually drawn.
            self.set_viewport(w, h);
        }
        let aspect = frame.aspect();
        if let (Some(anchor), true) = (self.anchor, self.controls.placed()) {
            let (eye_w, dir) = self.view();
            let eye = anchor.to_render(&eye_w).unwrap_or(Vec3::ZERO);
            let camera = Camera {
                eye,
                target: eye + dir,
                up: Vec3::Z,
                projection: Perspective {
                    fov_y: FOV_Y,
                    near: NEAR,
                    far: self.far,
                },
            };
            let view_proj = camera.view_proj(aspect);
            let frustum = Frustum::from_view_proj(view_proj);
            // Interiors (`landblock.md` §4.11–§4.12): from inside, the portal walk from the
            // camera's visibility cell decides which cells and interior objects are drawn,
            // and the scene flags whether the landscape and the sky are.
            let interior = self.camera_cell.and_then(|(k, c)| {
                let vis = self.visibility.get(&k).map(|(_, v)| Arc::clone(v))?;
                let off = anchor.offset_of(k)?;
                let to_clip = view_proj * Mat4::from_translation(off);
                Some(InteriorView::new(k, c, vis, eye - off, &to_clip))
            });
            self.interior_drawn = interior.as_ref().map(InteriorView::visible_count);
            let scene = self.scene;
            let mut char_draws = std::mem::take(&mut self.char_draws);
            char_draws.clear();
            self.characters.set_shading(
                self.systems.on(System::Lit),
                self.systems.on(System::Shadows),
            );
            // The free-fly camera hides the avatar (`m4-5-visual.md` §4.0).
            self.characters
                .set_hide_player(self.controls.mode() == MoveMode::Fly);
            // Which objects the camera's cell lets through (`landblock.md` §4.11–§4.12): an
            // outdoor object only from outdoors; an interior one through a visibility cell
            // the portal walk reaches.
            let all_interiors = self.config.interiors;
            let inside = self.camera_cell.is_some();
            let drawn = |key: LandblockKey, cell: u16| {
                if cell == OUTDOORS {
                    !inside
                } else {
                    all_interiors || interior.as_ref().is_some_and(|v| v.draws(key, cell))
                }
            };
            // Characters are culled by the cell they belong to like every drawn object
            // (`landblock.md` §4.11 "Moving entities", Tessera Decision there).
            let player_cell = (self.controls.mode() == MoveMode::Walk)
                .then(|| self.controls.physics().map(movement::PhysicsWalker::cell))
                .flatten();
            let streamer = &self.streamer;
            let objects = &self.objects;
            let visible = |site: &characters::CharacterSite| {
                let cell = character_cell(
                    site,
                    player_cell,
                    |tag| objects.figure_cell(tag),
                    |p| streamer.cell_at(p).map(|(k, c)| (k, c.index)),
                );
                match cell {
                    Some((k, c)) => drawn(k, c),
                    None => !inside,
                }
            };
            self.characters.prepare_draws(
                frame,
                &anchor,
                eye,
                &frustum,
                &self.objects,
                &visible,
                &mut char_draws,
            );
            let lit = self.systems.on(System::Lit);
            let fe = self.frame_env.clone();
            let clear = fe.as_ref().map_or(SKY, |f| f.clear);
            let pass = frame.world_pass("walker-world", &camera);
            pass.clear(clear);
            if lit {
                let mut fl = fe
                    .as_ref()
                    .map_or_else(tessera_render::FrameLighting::default, |f| {
                        f.lighting.clone()
                    });
                fl.lights.clone_from(&self.lights);
                pass.frame_lighting(fl);
                pass.light_reach(self.light_reach.clone());
            } else {
                pass.lighting(Lighting {
                    direction: Vec3::new(0.35, -0.45, 0.82),
                    color: [1.0, 0.97, 0.9],
                    ambient: [0.36, 0.38, 0.42],
                });
            }
            let t0 = Instant::now();
            if scene.outdoor_visible {
                self.terrain.draw(renderer, pass, &anchor, &frustum, eye);
            }
            let water_on = self.water_drawn();
            let material_time = self.material_time();
            let hand_off = self.far_view.as_ref().map(farview::FarViewLayer::hand_off);
            if let (Some(fv), true) = (self.far_view.as_mut(), scene.outdoor_visible) {
                let c = self.config.distances.object_multiplier();
                fv.draw(
                    pass,
                    &anchor,
                    eye,
                    c,
                    &mut self.resolver,
                    &mut self.objects,
                    water_on,
                    material_time,
                );
            }
            let t1 = Instant::now();
            // Figures the character layer draws are left out of the static instances.
            self.objects.draw(
                pass,
                &anchor,
                eye,
                &frustum,
                self.characters.shown_figures(),
                &drawn,
            );
            let t2 = Instant::now();
            for d in char_draws.drain(..) {
                pass.mesh(d);
            }
            self.char_draws = char_draws;
            // The avatar placeholder is a moving entity in the visibility cell of the cell
            // it stands in (`landblock.md` §4.11): the mover's cell, else outdoors.
            let placeholder_cell = self
                .controls
                .physics()
                .and_then(movement::PhysicsWalker::cell);
            let placeholder_drawn = match placeholder_cell {
                Some((k, c)) => drawn(k, c),
                None => !inside,
            };
            if let (Some(mesh), Some((feet, yaw))) = (self.avatar, self.controls.avatar())
                && !self.characters.player_ready()
                && self.controls.mode() == MoveMode::Walk
                && placeholder_drawn
                && let Some(p) = anchor.to_render(&feet)
            {
                let tf = Mat4::from_translation(p + Vec3::Z * 0.9)
                    * Mat4::from_rotation_z(yaw)
                    * Mat4::from_scale(Vec3::new(0.7, 0.5, 1.8));
                let color = if lit {
                    [0.98, 0.76, 0.38, 1.0]
                } else {
                    [0.95, 0.55, 0.12, 1.0]
                };
                pass.mesh(MeshDraw::new(mesh, tf, Material::color(color)));
            }
            let t3 = Instant::now();
            if self.water_drawn() {
                self.water.draw(
                    pass,
                    &self.streamer,
                    &anchor,
                    eye,
                    self.ring_extent(),
                    hand_off.as_deref(),
                    material_time,
                    &mut self.resolver,
                    &mut self.objects,
                );
            }
            let t4 = Instant::now();
            if self.sky_drawn()
                && let Some(sky) = self.env.sky(&mut self.resolver, &mut self.objects)
            {
                pass.sky(sky);
            }
            let t5 = Instant::now();
            if self.frills_drawn() {
                let settings = self.streamer.config().frills;
                self.frills.draw(
                    pass,
                    &self.streamer,
                    &anchor,
                    &settings,
                    &mut self.resolver,
                    &self.terrain,
                    &mut self.objects,
                    &frustum,
                    eye,
                    Instant::now(),
                );
            }
            // Particle effects after the other translucent surfaces of the scene
            // (`particle-runtime.md` §9; their order is `particles.md` §8.15).
            self.effects.draw(pass, &anchor, eye, dir, &self.objects);
            let ms = |a: Instant, b: Instant| b.saturating_duration_since(a).as_secs_f64() * 1e3;
            self.phase_ms[3] = ms(t0, t1);
            self.phase_ms[4] = ms(t1, t2);
            self.phase_ms[5] = ms(t3, t4);
            self.phase_ms[6] = ms(t4, t5);
            self.phase_ms[7] = ms(t5, Instant::now());
        } else {
            frame
                .world_pass("walker-world", &Camera::default())
                .clear(SKY);
        }
        let overlay = frame.pass("walker-hud", PassSpace::Screen);
        let hud = self.config.hud && self.show_hud;
        if hud {
            self.hud.draw(overlay, 10.0, 10.0);
        }
        // The chat stays visible while its input is open, even without the HUD. With a game
        // UI attached its chat window shows the chat instead (no second display).
        if (hud && !self.controls.ui_attached()) || self.chat.is_open() {
            self.chat.draw(overlay, 10.0, h as f32 - 10.0);
        }
        if let (true, Some(pos), Some(anchor)) = (
            self.config.hud && self.show_minimap,
            self.focus(),
            self.anchor,
        ) {
            let size = (h as f32 * 0.3).clamp(120.0, 320.0);
            let view = MinimapView {
                x: w as f32 - size - 14.0,
                y: 14.0,
                size,
                radius: self.config.minimap_radius,
            };
            self.minimap.draw(
                overlay,
                anchor.region,
                pos.xy(),
                self.controls.heading(),
                &view,
            );
        }
        let cpu_ms = self
            .frame_start
            .map_or(0.0, |s| s.elapsed().as_secs_f64() * 1e3);
        self.log_row(cpu_ms);
    }

    fn log_row(&mut self, cpu_ms: f64) {
        let Some(start) = self.frame_start else {
            return;
        };
        let p = self.focus().unwrap_or(WorldPos::new(0, 0.0, 0.0, 0.0));
        let ts = self.terrain.stats();
        let os = self.objects.stats();
        let cs = self.characters.stats();
        let r = &self.last_report;
        let audio = self.audio.as_ref().map(|a| a.stats());
        let engine = audio.and_then(|a| a.engine);
        let ws = self.water.stats();
        let fs = self.frills.stats();
        let fx_row = framelog::FxRow {
            env_ms: self.env_ms,
            frills_ms: self.frills_ms,
            time_of_day: self.frame_env.as_ref().map_or(0.0, |f| f.time_of_day),
            g: self.frame_env.as_ref().map_or(0.0, |f| f.g),
            fog_far: self
                .frame_env
                .as_ref()
                .and_then(|f| f.fog_far)
                .unwrap_or(0.0),
            terrain_layered: ts.layered,
            ceilings: ts.ceilings,
            objects_drawn: os.objects_drawn,
            fading: os.fading,
            instanced: os.instanced,
            batches: os.batches,
            water_quads: ws.quads,
            frill_groups: fs.groups,
            frill_instances: fs.instances,
            lights: self.lights.len(),
            phase_ms: self.phase_ms,
            ..framelog::FxRow::default()
        };
        let Some(log) = &mut self.log else { return };
        log.push(FrameRow {
            t: start
                .saturating_duration_since(self.log_start)
                .as_secs_f64(),
            frame_ms: self.frame_ms,
            cpu_ms,
            stream_ms: self.stream_ms,
            uploaded: self.uploaded,
            resident_blocks: r.resident_blocks,
            resident_bytes: r.resident_bytes,
            loading: r.loading,
            near_missing: r.near_missing,
            gpu_bytes: ts.gpu_bytes + os.mesh_bytes + os.texture_bytes,
            rss: self.rss.1,
            draws: os.draws,
            terrain: ts.drawn,
            characters: cs.animated,
            character_draws: cs.drawn,
            characters_ms: self.chars_ms,
            anim_collect_ms: cs.collect_ms,
            anim_submit_ms: cs.submit_ms,
            anim_upload_ms: cs.palette_ms,
            anim_worker_ms: cs.worker_ms,
            audio_ms: audio.map_or(0.0, |a| a.update_ms),
            audio_voices: engine.map_or(0, |e| e.active_voices),
            audio_underruns: engine.map_or(0, |e| e.underruns),
            mixer_load: engine.map_or(0.0, |e| f64::from(e.mixer_load)),
            x: p.x,
            y: p.y,
            z: p.z,
            fx: fx_row,
        });
    }

    /// The eye and view direction. The walk-mode third-person camera is kept above the
    /// terrain (`walker.md` §5; it must not dip under a slope), except inside a cell
    /// (§14.12 (a): the terrain is not drawn there); free-fly and scripted cameras are not.
    fn view(&self) -> (WorldPos, Vec3) {
        let (eye, dir) = self.controls.eye();
        if self.camera_cell.is_some() || self.controls.mode() != MoveMode::Walk {
            return (eye, dir);
        }
        let ground = self.terrain.tiles().height_at(eye.x, eye.y);
        let feet = self.controls.avatar().map(|(feet, _)| feet);
        controls::keep_above_terrain(eye, dir, ground, feet)
    }

    /// Whether the scripted flight has finished (always false without one).
    #[must_use]
    pub fn flight_done(&self) -> bool {
        self.flight.is_some_and(|f| f.done)
    }

    /// Streamer statistics.
    #[must_use]
    pub fn stream_stats(&self) -> &tessera_world::StreamStats {
        self.streamer.stats()
    }

    /// The description of the viewer's position, as the sandbox would give it.
    #[must_use]
    pub fn describe(&self) -> Option<String> {
        let pos = self.focus()?;
        Some(describe(&lock(&self.shared), &pos, self.controls.heading()))
    }

    /// Particle effects (counters for tests and reports).
    #[must_use]
    pub fn effects(&self) -> &effects::EffectLayer {
        &self.effects
    }

    /// World audio, when on.
    #[must_use]
    pub fn audio(&self) -> Option<&crate::audio::WorldAudio> {
        self.audio.as_ref()
    }

    /// Frees GPU resources and stops the workers.
    pub fn shutdown(&mut self, renderer: &mut Renderer<'_>) {
        if let Some(a) = self.audio.as_mut() {
            a.shutdown();
        }
        self.effects.shutdown();
        self.characters.shutdown(renderer, &mut self.objects);
        self.stage.release(&mut self.objects);
        self.water.release(&mut self.objects);
        self.env.release(&mut self.objects);
        self.resolver.shutdown();
        self.terrain.shutdown();
        if let Some(fv) = self.far_view.as_mut() {
            fv.shutdown();
        }
        self.clear_all(renderer);
        self.hud.release(renderer);
        self.chat.release(renderer);
        if let Some(m) = self.avatar.take() {
            renderer.destroy_mesh(m);
        }
        self.streamer.clear();
        self.places = None;
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn characters_belong_to_the_cell_their_movement_assigns() {
        // `landblock.md` §4.11 "Moving entities", Tessera Decision.
        let k = LandblockKey::new(1, 0x91, 0xA6).expect("key");
        let far = LandblockKey::new(1, 0x93, 0xA6).expect("key");
        let site = |tag: Option<u64>, player: bool| characters::CharacterSite {
            tag,
            player,
            position: WorldPos::new(1, 100.0, 200.0, -10.0),
        };
        // The lookup sees the feet raised by 0.05 and finds cell 9 of `far`.
        let lookup = |p: &WorldPos| {
            assert!((p.z - (-10.0 + CHARACTER_CELL_RAISE)).abs() < 1e-9);
            Some((far, 9))
        };
        let outdoors = |_: &WorldPos| None;
        let no_figure = |_: u64| None;
        // The player in walk mode on collision: the mover's cell, even outdoors.
        assert_eq!(
            character_cell(&site(None, true), Some(Some((k, 5))), no_figure, lookup),
            Some((k, 5))
        );
        assert_eq!(
            character_cell(&site(None, true), Some(None), no_figure, lookup),
            None,
            "the mover is outdoors"
        );
        // Without a mover the player's cell is looked up like any character's.
        assert_eq!(
            character_cell(&site(None, true), None, no_figure, lookup),
            Some((far, 9))
        );
        // A placed figure takes its static instance's cell, outdoors included.
        let tag = figures::figure_tag(k, 3);
        let figure = |t: u64| (t == tag).then_some((k, 7));
        assert_eq!(
            character_cell(&site(Some(tag), false), None, figure, outdoors),
            Some((k, 7))
        );
        let outside = |t: u64| (t == tag).then_some((k, OUTDOORS));
        assert_eq!(
            character_cell(&site(Some(tag), false), None, outside, lookup),
            None
        );
        // A placed figure whose landblock's objects are not in the layer: looked up.
        assert_eq!(
            character_cell(&site(Some(tag), false), None, no_figure, lookup),
            Some((far, 9))
        );
        // Any other character: the lookup at its feet; outdoors when none contains it.
        assert_eq!(
            character_cell(&site(None, false), None, no_figure, lookup),
            Some((far, 9))
        );
        assert_eq!(
            character_cell(&site(None, false), None, no_figure, outdoors),
            None
        );
        assert_eq!(
            character_cell(&site(None, false), None, no_figure, |_: &WorldPos| Some((
                k, OUTDOORS
            ))),
            None,
            "cell 0 is outdoors"
        );
    }

    #[test]
    fn keys_the_keymap_binds_are_left_to_it() {
        use tessera_input::{BindingSpec, KeyCode, Keymap, default_keymap};
        assert!(
            keys_taken_by(&default_keymap()).is_empty(),
            "the built-in bindings leave the walker keys free"
        );
        let mut user = Keymap::new();
        user.set(
            tessera_input::contexts::GLOBAL,
            "toggle_debug",
            vec![BindingSpec::parse("F9").unwrap()],
        )
        .unwrap();
        user.set(
            tessera_input::contexts::CAMERA,
            "zoom",
            vec![BindingSpec::parse("Ctrl+Equal").unwrap()],
        )
        .unwrap();
        let taken = keys_taken_by(&default_keymap().overlay(&user));
        assert_eq!(taken, HashSet::from([KeyCode::F9, KeyCode::Equal]));
    }

    #[test]
    fn upload_budget_admits_first_and_stops_when_spent() {
        let now = Instant::now();
        let mut b = UploadBudget::new(100, Duration::from_secs(60), now);
        assert!(b.try_take(1_000), "first upload always fits");
        assert!(!b.try_take(1));
        let mut b = UploadBudget::new(100, Duration::from_secs(60), now);
        assert!(b.try_take(40) && b.try_take(60) && !b.try_take(1));
        assert_eq!(b.taken, 100);
        let mut b = UploadBudget::new(100, Duration::ZERO, now);
        assert!(b.try_take(1));
        assert!(!b.try_take(1), "time is up");
    }

    #[test]
    fn far_plane_covers_the_far_ring() {
        assert_eq!(far_plane(1_600.0), MIN_FAR);
        for blocks in 2..=30u8 {
            let ring = f64::from(blocks) * LANDBLOCK_SIZE;
            let far = f64::from(far_plane(ring));
            assert!(
                far >= ring + LANDBLOCK_SIZE * std::f64::consts::SQRT_2,
                "{blocks}"
            );
        }
        assert_eq!(far_plane(f64::NAN), MIN_FAR);
    }

    #[test]
    fn location_reports_place_correct_or_are_ignored() {
        use LocationCause::{Corrected, Entered, Requested, Teleported};
        assert_eq!(placement(Entered, false, false), Placement::Place);
        assert_eq!(placement(Corrected, false, false), Placement::Place);
        assert_eq!(placement(Teleported, true, false), Placement::Place);
        assert_eq!(placement(Entered, true, false), Placement::Place);
        assert_eq!(placement(Corrected, true, false), Placement::Correct);
        assert_eq!(placement(Requested, true, false), Placement::Ignore);
        assert_eq!(placement(Corrected, true, true), Placement::Ignore);
        assert_eq!(placement(Entered, false, true), Placement::Ignore);
    }

    #[test]
    fn streaming_follows_fresh_requests() {
        let now = Instant::now();
        let viewer = WorldPos::new(1, 16_040.0, 24_080.0, 0.0);
        let target = WorldPos::new(1, 32_000.0, 8_000.0, 0.0);
        let near = WorldPos::new(1, 16_050.0, 24_090.0, 0.0);
        assert_eq!(stream_focus(None, None, now), None);
        assert_eq!(stream_focus(None, Some((target, now)), now), Some(target));
        // Before placement even an old request counts.
        let old = now.checked_sub(Duration::from_secs(60)).unwrap_or(now);
        assert_eq!(stream_focus(None, Some((target, old)), now), Some(target));
        assert_eq!(stream_focus(Some(viewer), None, now), Some(viewer));
        // A pending teleport elsewhere pulls the streamer away from the viewer...
        assert_eq!(
            stream_focus(Some(viewer), Some((target, now)), now),
            Some(target)
        );
        // ...but not once the request is stale, nor for the viewer's own block.
        if old < now {
            assert_eq!(
                stream_focus(Some(viewer), Some((target, old)), now),
                Some(viewer)
            );
        }
        assert_eq!(
            stream_focus(Some(viewer), Some((near, now)), now),
            Some(viewer)
        );
    }

    #[test]
    fn terrain_share_grows_with_the_view() {
        assert_eq!(terrain_arena_reserve(0.0), MIN_TERRAIN_ARENA);
        assert_eq!(
            terrain_arena_reserve(10.0 * LANDBLOCK_SIZE),
            MIN_TERRAIN_ARENA
        );
        let far = terrain_arena_reserve(30.0 * LANDBLOCK_SIZE);
        // About 3,200 blocks at 64 KiB.
        assert!((190 << 20..=210 << 20).contains(&far), "{far}");
        assert!(terrain_arena_reserve(f64::NAN) >= MIN_TERRAIN_ARENA);
        assert!(terrain_arena_reserve(f64::INFINITY) < u64::MAX);
    }
}
