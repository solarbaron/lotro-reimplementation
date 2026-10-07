// SPDX-License-Identifier: GPL-3.0-or-later
//! The offline world walker (ROADMAP M3): terrain and static objects of the user's world
//! streamed around the viewer, free-fly and walk modes, area name, map coordinates,
//! landblock, frame time and residency on screen, and a minimap.
//!
//! # Threads
//!
//! The main thread never reads archives or decodes. It runs, once per frame:
//!
//! 1. input → controllers (fixed 30 Hz step, `controls`);
//! 2. the landblock streamer of `tessera-world` (bundles decoded on the asset workers);
//! 3. new or released bundles → the terrain layer (`terrain`: blocks and meshes built on
//!    its own workers), the object layer (`objects`: parts and textures decoded on the
//!    asset workers) and the minimap (`minimap`);
//! 4. GPU uploads of finished products within a per-frame byte and time budget
//!    ([`UploadBudget`]);
//! 5. culling and draw recording.
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
pub(crate) mod controls;
pub mod figures;
pub(crate) mod framelog;
pub(crate) mod geometry;
pub(crate) mod hud;
pub(crate) mod jobs;
pub(crate) mod minimap;
pub(crate) mod objects;
pub mod parse;
pub(crate) mod places;
pub(crate) mod source;
pub(crate) mod terrain;
pub(crate) mod view;

use std::collections::HashSet;
use std::path::PathBuf;
use std::sync::{Arc, Mutex};
use std::time::{Duration, Instant};

use tessera_assets::{PoolConfig, WorkerPool};
use tessera_dat::ArchiveSet;
use tessera_input::RawEvent;
use tessera_net::{EntityId, Event, LocationCause, RegionId};
use tessera_render::glam::{Mat4, Vec3};
use tessera_render::{
    Camera, Frame, Lighting, Material, MeshConfig, MeshData, MeshDraw, MeshId, PassSpace,
    Perspective, Renderer, Rgba,
};
use tessera_terrain::LodSelector;
use tessera_world::{
    LANDBLOCK_SIZE, LandblockKey, StreamConfig, WorldContext, WorldPos, WorldStreamer,
};

use self::characters::catalog::CharacterCatalog;
use self::characters::figure::{ClipStore, FigureContext};
use self::characters::{CharacterConfig, CharacterLayer};
use self::controls::{Controls, MoveMode};
use self::framelog::{FrameLog, FrameRow, Rolling, rss_bytes};
use self::hud::TextPanel;
use self::minimap::{MinimapLayer, MinimapView};
use self::objects::{ObjectConfig, ObjectLayer};
use self::parse::FlyPath;
use self::places::PlaceBuild;
use self::source::{FigureBlock, Places, SourceShared, describe, loc_text, lock};
use self::terrain::TerrainLayer;
use self::view::{Anchor, Frustum};
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
}

/// Most session entity events applied in one frame (decision 0008: the store's change
/// lists are unbounded, so the client bounds what it applies); the rest wait, in order, for
/// the next frames.
pub const MAX_ENTITY_EVENTS_PER_FRAME: usize = 4096;

/// Sky colour (linear).
pub const SKY: Rgba = [0.42, 0.58, 0.80, 1.0];
/// Vertical field of view.
pub const FOV_Y: f32 = 1.0;
/// Near clip plane.
pub const NEAR: f32 = 0.5;
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
    /// Draw interior (cell) entities too.
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
            terrain_threads: 2,
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
        }
    }
}

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
}

/// The walker (module docs).
pub struct Walker {
    config: WalkerConfig,
    ctx: Arc<WorldContext>,
    set: Arc<ArchiveSet>,
    pool: WorkerPool,
    streamer: WorldStreamer,
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
        let terrain = TerrainLayer::new(
            config.terrain_threads.max(1),
            LodSelector::from_fov(720.0, FOV_Y, config.pixel_tolerance),
            mesh_cfg,
            config.interiors,
        )?;
        let objects = ObjectLayer::new(
            Arc::clone(&set),
            &pool,
            config.objects,
            mesh_cfg,
            object_arena,
        );
        let minimap = MinimapLayer::new(Arc::clone(&set), &pool);
        let characters = CharacterLayer::new(
            config.characters,
            Arc::new(FigureContext {
                set: Arc::clone(&set),
                catalog: Arc::clone(&catalog),
                materials: objects.materials(),
                mesh_config: mesh_cfg,
                clips: ClipStore::default(),
            }),
            &pool,
        );
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
        let help = controls::help_text(&keymap);
        tracing::info!("walker controls: {help}");
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
        let far = far_plane(config.stream.rings.far);
        let config_region = config.region;
        let config_bench = config.bench;
        Ok(Self {
            far,
            help,
            log: config.frame_log.then(FrameLog::default),
            config,
            ctx,
            set,
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
        })
    }

    /// Forwards a raw input event.
    pub fn input(&mut self, event: &RawEvent) {
        self.controls.handle(event);
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

    /// Per frame (module docs, steps 1–4). `dt` is the frame time in seconds.
    pub fn update(
        &mut self,
        renderer: &mut Renderer<'_>,
        world: Option<&mut dyn WalkerSession>,
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
        }
        self.characters.apply_events(
            &mut self.entity_events,
            &self.shared,
            player_id,
            self.player_region,
            now,
        );
        let mut chars_ms = t_chars.elapsed().as_secs_f64() * 1e3;

        // Input and movement.
        let tiles = self.terrain.tiles().clone();
        let ground = |gx: f64, gy: f64| tiles.height_at(gx, gy);
        let ce = self.controls.frame(dt, &ground);
        events.quit = ce.quit;
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
            self.stream_ms = t.elapsed().as_secs_f64() * 1e3;
        }
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

        let mut budget = UploadBudget::new(self.config.upload_bytes, self.config.upload_time, now);
        if let (Some(anchor), Some(eye)) = (self.anchor, eye) {
            let cam = anchor.to_render(&eye).unwrap_or(Vec3::ZERO);
            self.terrain.update(renderer, &anchor, cam, &mut budget);
            for (key, product) in self.terrain.take_finished() {
                self.objects.insert(key, product.instances);
                self.minimap.set_overview(renderer, key, &product.overview);
            }
            self.objects.update(renderer, eye.xy(), &mut budget);
            if let Some(k) = eye.landblock() {
                let radius = (self.config.minimap_radius / LANDBLOCK_SIZE).ceil() as u32 + 1;
                self.minimap.update(renderer, k, radius, &mut budget);
            }
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
            if let Some(audio) = self.audio.as_mut() {
                audio.update(crate::audio::AudioInput {
                    dt: f64::from(dt),
                    streamer: &self.streamer,
                    eye,
                    player,
                });
            }
        }

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
            if self.terrain.insert(key, &bundle, renderer) {
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
            self.terrain.remove(key, renderer);
            self.objects.remove(key);
            self.minimap.remove(renderer, key);
            lock(&self.shared).areas.remove(&key);
        }
    }

    fn clear_all(&mut self, renderer: &mut Renderer<'_>) {
        self.terrain.clear(renderer);
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
                    "Objects {} blocks ({:.0} m), {} instances, parts {} ready / {} pending / {} failed, \
                     textures {} / {} pending",
                    os.active_blocks,
                    os.distance,
                    os.instances,
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
            let frustum = Frustum::from_view_proj(camera.view_proj(aspect));
            let mut char_draws = std::mem::take(&mut self.char_draws);
            char_draws.clear();
            self.characters.prepare_draws(
                frame,
                &anchor,
                eye,
                &frustum,
                &self.objects,
                &mut char_draws,
            );
            let pass = frame.world_pass("walker-world", &camera);
            pass.clear(SKY).lighting(Lighting {
                direction: Vec3::new(0.35, -0.45, 0.82),
                color: [1.0, 0.97, 0.9],
                ambient: [0.36, 0.38, 0.42],
            });
            self.terrain.draw(renderer, pass, &anchor, &frustum);
            // Figures the character layer draws are left out of the static instances.
            self.objects.draw(
                pass,
                &anchor,
                eye,
                &frustum,
                self.characters.shown_figures(),
            );
            for d in char_draws.drain(..) {
                pass.mesh(d);
            }
            self.char_draws = char_draws;
            if let (Some(mesh), Some((feet, yaw))) = (self.avatar, self.controls.avatar())
                && !self.characters.player_ready()
                && let Some(p) = anchor.to_render(&feet)
            {
                let tf = Mat4::from_translation(p + Vec3::Z * 0.9)
                    * Mat4::from_rotation_z(yaw)
                    * Mat4::from_scale(Vec3::new(0.7, 0.5, 1.8));
                pass.mesh(MeshDraw::new(
                    mesh,
                    tf,
                    Material::color([0.95, 0.55, 0.12, 1.0]),
                ));
            }
        } else {
            frame
                .world_pass("walker-world", &Camera::default())
                .clear(SKY);
        }
        let overlay = frame.pass("walker-hud", PassSpace::Screen);
        self.hud.draw(overlay, 10.0, 10.0);
        if let (Some(pos), Some(anchor)) = (self.focus(), self.anchor) {
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
        });
    }

    /// The eye and view direction, with the eye kept above the terrain (a third-person
    /// camera behind the avatar must not dip under a slope).
    fn view(&self) -> (WorldPos, Vec3) {
        let (mut eye, mut dir) = self.controls.eye();
        if let Some(h) = self.terrain.tiles().height_at(eye.x, eye.y) {
            let min = f64::from(h) + 0.8;
            if eye.z < min {
                eye.z = min;
                // Keep looking at the avatar's head.
                if let Some((feet, _)) = self.controls.avatar() {
                    let head = Vec3::new(
                        (feet.x - eye.x) as f32,
                        (feet.y - eye.y) as f32,
                        (feet.z + f64::from(controls::EYE_HEIGHT) - eye.z) as f32,
                    );
                    if let Some(d) = head.try_normalize() {
                        dir = d;
                    }
                }
            }
        }
        (eye, dir)
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
        self.characters.shutdown(renderer, &mut self.objects);
        self.terrain.shutdown();
        self.clear_all(renderer);
        self.hud.release(renderer);
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
