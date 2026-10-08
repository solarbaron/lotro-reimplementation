// SPDX-License-Identifier: GPL-3.0-or-later
//! Characters in the world walker (ROADMAP M4): the avatar, the figures the landblocks
//! place (static NPCs, weenie placements) and the sandbox's simulated characters
//! (wanderers, the `--crowd`), kept in the project's entity store
//! (`tessera-entity`, decision 0008), assembled and animated from the data, and drawn with
//! the skinned pipeline (`walker.md` §10).
//!
//! # Per frame (ARCHITECTURE §20.3)
//!
//! 1. **Events** (step 2): the session's entity events become store operations:
//!    `EntityCreated` spawns a networked entity and requests its figure, `EntityMoved`
//!    moves it (and gives its ground speed), `AppearanceChanged` changes its figure (for
//!    the player it creates the avatar), `EntityDespawned` and `StaticsCleared` remove.
//! 2. **Figures**: distinct appearances ([`figure::FigureKey`]) are built once on the
//!    asset workers ([`figure::build_figure`]) and uploaded within the frame's upload
//!    budget; their textures are the object layer's (shared, reference-counted). An
//!    entity gets its render component when its figure is on the GPU, and its animation
//!    component (a scheduler instance and an [`Animator`]) when the figure is animated.
//! 3. **Animation** (step 6): first the sampling results of the previous frame are
//!    collected (never blocking), then every animated entity within the animation distance
//!    is driven by its ground speed ([`motion::drive`]) and a new sampling job is submitted
//!    to the workers ([`AnimationScheduler`]). Poses are therefore one frame old.
//! 4. **Draw**: skinning palettes of the visible characters go into the frame, then each
//!    part is drawn: skinned parts with the palette, held items at their holding location's
//!    posed transform (`skeleton-anim.md` §8.4), everything at the entity's placement and
//!    model scale.
//!
//! # Placed figures
//!
//! A figure the landblocks place is drawn either here or by the object layer as a static
//! instance, never both (`walker.md` §10.2): after each update,
//! [`CharacterLayer::shown_figures`] holds the tags of the placed entities this layer draws
//! (figure on the GPU, posed when animated, within the draw distance) plus the markers it
//! hides, and the object layer leaves out exactly those instances. A figure still building,
//! failed, released or too far therefore stays a static instance. Animated characters are
//! not drawn before their first sampled pose; each gets one sampling job for it wherever it
//! is.
//!
//! Positions are region-frame `f64` in the store; simulated characters, whose moves arrive
//! at the sandbox's tick rate, are extrapolated along their last velocity (for at most
//! [`EXTRAPOLATE`]) and put on the terrain; placed figures keep the data's height.
//! Characters of a remote session (a Tessera test server) are instead placed every frame
//! at their interpolated state ([`CharacterLayer::apply_samples`],
//! `tessera-protocol.md` §8.6), still on the terrain and still animated by ground speed.

pub mod catalog;
pub mod figure;
pub mod motion;

use std::collections::{HashMap, HashSet};
use std::sync::mpsc::{Receiver, Sender, channel};
use std::sync::{Arc, Mutex, PoisonError};
use std::time::{Duration, Instant};

use tessera_anim::{
    AnimationScheduler, Animator, InstanceId, SchedulerConfig, Submitted, skinning_matrices,
};
use tessera_assets::{CancelToken, Priority, WorkerPool};
use tessera_entity::{Changes, Column, Entity, EntityStore, NetId, StoreConfig, Transform};
use tessera_net::{Appearance, Body, EntityId, Event};
use tessera_render::glam::{Mat4, Quat, Vec3};
use tessera_render::{Frame, MeshDraw, MeshId, Renderer, SkinPalette};
use tessera_world::WorldPos;

use self::catalog::WeenieRole;
use self::figure::{FigureAsset, FigureContext, FigureKey, FigureRequest, build_figure};
use self::motion::{Locomotion, drive, make_animator};
use super::UploadBudget;
use super::geometry::{MaterialPlan, TexRequest};
use super::objects::{ObjectLayer, surface_draw};
use super::source::SourceShared;
use super::view::{Anchor, Frustum};

/// Most items of one appearance considered (equal to `tessera_net`'s limit).
pub const MAX_EQUIPMENT: usize = tessera_net::model::MAX_EQUIPMENT;
/// How far a simulated character is extrapolated along its last velocity.
pub const EXTRAPOLATE: Duration = Duration::from_millis(250);
/// Figure jobs in flight at once.
pub const MAX_JOBS: usize = 6;
/// An unreferenced figure on the GPU is kept this long for reuse before its resources are
/// freed. Unreferenced figures not on the GPU (queued, building, built) are dropped at once.
pub const FIGURE_KEEP: Duration = Duration::from_secs(30);

/// Settings of the character layer.
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct CharacterConfig {
    /// Draw weenie markers (housing hooks, distant-battle units) as static figures.
    pub markers: bool,
    /// Cap on the characters' draw distance (m): below it the class distance of
    /// [`character_draw_distance`] applies (`walker.md` §11.4.2).
    pub draw_distance: f32,
    /// Characters further than this (m) keep their last pose (no sampling job; a character
    /// gets one job for its first pose wherever it is).
    pub anim_distance: f32,
    /// Most entities in the store.
    pub max_entities: u32,
}

impl Default for CharacterConfig {
    fn default() -> Self {
        Self {
            markers: false,
            draw_distance: 400.0,
            anim_distance: 200.0,
            max_entities: 1 << 16,
        }
    }
}

/// Characters cast the sun's shadow within this distance (metres): the render
/// component's initial `d2` (`walker.md` §11.4 step 4, `mesh.md` §8.7 shadow distance at
/// the built-in settings). Tessera Decision: the class of each character's own entity is
/// not resolved for this (`walker.md` §14.11 OPEN).
pub const CHARACTER_SHADOW_DISTANCE: f32 = tessera_world::draw_distance::COMPONENT_DEFAULTS[2];

/// `d0` of the character classes (`walker.md` §11.4.2: `PlayerAvatar`, `Creature` and
/// `Creature2Deg` all have 80 m). Tessera Decision: used for every character until each
/// entity's own class is resolved (`walker.md` §14.11 OPEN).
pub const CHARACTER_D0: f32 = 80.0;

/// What the character layer's distance rules need of a frame (`walker.md` §11.4).
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct CharacterFrame {
    /// The global limit *G* (`walker.md` §11.3).
    pub g: f32,
    /// The object draw-distance multiplier *M_d* (§11.2).
    pub multiplier: f32,
    /// Frame time, seconds.
    pub dt: f32,
    /// Camera travel since the previous frame, metres (fade reset beyond 25 m).
    pub camera_moved: f32,
    /// Per-class distances and fades on; off draws every character to the cap.
    pub lod: bool,
}

impl Default for CharacterFrame {
    fn default() -> Self {
        Self {
            g: 2_080.0,
            multiplier: 1.0,
            dt: 1.0 / 60.0,
            camera_moved: 0.0,
            lod: true,
        }
    }
}

/// A character's draw distance *D* = min(*G*, `d0` × *M_d*) (`walker.md` §11.4 step 1;
/// `d0` × 3 for the player's own character, step 3), capped at `cap`; the cap alone when
/// per-class distances are off. Non-finite inputs fall back to the cap.
#[must_use]
pub fn character_draw_distance(frame: &CharacterFrame, cap: f32, player: bool) -> f32 {
    if !frame.lod {
        return cap;
    }
    let scale = if player {
        tessera_world::draw_distance::PLAYER_SCALE
    } else {
        1.0
    };
    let d = (CHARACTER_D0 * scale * frame.multiplier).min(frame.g);
    if d.is_finite() { d.min(cap) } else { cap }
}

/// Counters and timings for the HUD and the frame log.
#[derive(Debug, Clone, Copy, Default, PartialEq)]
pub struct CharacterStats {
    /// Entities in the store.
    pub entities: usize,
    /// Entities with an animation component.
    pub animated: usize,
    /// Of `animated`: entities whose animation instance has a sampled pose.
    pub posed: usize,
    /// Characters drawn last frame.
    pub drawn: usize,
    /// Sampling jobs submitted last frame.
    pub sampled: usize,
    /// Figures on the GPU.
    pub figures_ready: usize,
    /// Figures building or waiting for upload.
    pub figures_pending: usize,
    /// Figures that failed.
    pub figures_failed: usize,
    /// GPU bytes of figure meshes.
    pub gpu_bytes: u64,
    /// Main thread: collecting sampling results, ms.
    pub collect_ms: f64,
    /// Main thread: driving animators and submitting jobs, ms.
    pub submit_ms: f64,
    /// Main thread: uploading palettes and recording draws, ms.
    pub palette_ms: f64,
    /// Worker time of the jobs collected this frame, ms.
    pub worker_ms: f64,
}

/// The render component: the figure slot an entity draws.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct CharRender {
    slot: u32,
}

/// The animation component (decision 0008: scheduler instance plus animator).
#[derive(Debug)]
pub struct CharAnim {
    instance: InstanceId,
    animator: Animator,
    loco: Locomotion,
}

/// Caller-side per-entity data (a `Column`, decision 0008).
#[derive(Debug, Clone)]
struct Actor {
    net: EntityId,
    tag: Option<u64>,
    /// Put on the terrain (simulated characters).
    snap: bool,
    /// Not drawn (markers, the avatar in fly mode).
    hidden: bool,
    /// The figure requested (holds a reference).
    slot: Option<u32>,
    /// The figure the render component draws.
    attached: Option<u32>,
    region: u8,
    base: [f64; 3],
    velocity: [f32; 3],
    at: Instant,
    heading: f32,
    speed: f32,
    seed: u32,
    /// The 1.5 s fade across the draw distance (`mesh.md` §8.7).
    fade: tessera_world::draw_distance::DrawFade,
    /// In view in the previous frame (a character coming into view appears at once).
    seen: bool,
}

struct GpuPart {
    mesh: MeshId,
    plans: Vec<MaterialPlan>,
    skinned: bool,
    held: Option<u32>,
    bytes: u64,
}

/// The reference pose of a figure that has a skeleton but no animator: skinning palette
/// and holding-location transforms (`skeleton-anim.md` §8.4 with the reference pose).
#[derive(Debug, Default)]
struct RestPose {
    skin: Vec<Mat4>,
    attach: Vec<(u32, Mat4)>,
}

impl RestPose {
    fn of(asset: &FigureAsset) -> Option<Self> {
        let skel = asset.skeleton.as_ref()?;
        let model = skel.reference_model();
        let mut skin = vec![Mat4::IDENTITY; skel.bone_count()];
        if skinning_matrices(skel, skel.reference_triples(), &mut skin).is_err() {
            skin.clear();
        }
        let mut attach = Vec::new();
        if let Some(t) = &asset.attach {
            t.model_transforms(model, &mut attach);
        }
        Some(Self { skin, attach })
    }
}

/// A pose as drawn: skinning palette and holding-location transforms.
type PoseView<'a> = (&'a [Mat4], &'a [(u32, Mat4)]);

struct GpuFigure {
    asset: Arc<FigureAsset>,
    parts: Vec<GpuPart>,
    textures: Vec<TexRequest>,
    /// Object-space bounding sphere (unscaled).
    center: Vec3,
    radius: f32,
    /// The reference pose, for a skeleton without an animator.
    rest: Option<RestPose>,
}

enum SlotState {
    Building(CancelToken),
    Queued,
    Waiting(Arc<FigureAsset>, Instant),
    Ready(GpuFigure),
    Failed,
    Free,
}

struct Slot {
    key: Option<FigureKey>,
    request: Option<FigureRequest>,
    generation: u64,
    refs: u32,
    unused_since: Option<Instant>,
    state: SlotState,
}

type JobResult = (u32, u64, Result<FigureAsset, String>);

/// Sends a figure job's result exactly once, also when the job is dropped unrun.
struct JobGuard {
    slot: u32,
    generation: u64,
    tx: Sender<JobResult>,
    sent: bool,
}

impl JobGuard {
    fn send(mut self, r: Result<FigureAsset, String>) {
        self.sent = true;
        let _ = self.tx.send((self.slot, self.generation, r));
    }
}

impl Drop for JobGuard {
    fn drop(&mut self) {
        if !self.sent {
            let _ = self
                .tx
                .send((self.slot, self.generation, Err("job dropped".into())));
        }
    }
}

/// The character layer (module docs).
pub struct CharacterLayer {
    config: CharacterConfig,
    /// `None` only in tests: every figure job then fails.
    ctx: Option<Arc<FigureContext>>,
    pool: WorkerPool,
    store: EntityStore<CharRender, CharAnim>,
    changes: Changes<CharRender, CharAnim>,
    actors: Column<Actor>,
    slots: Vec<Slot>,
    free: Vec<u32>,
    by_key: HashMap<FigureKey, u32>,
    queue: std::collections::VecDeque<u32>,
    in_flight: usize,
    tx: Sender<JobResult>,
    rx: Receiver<JobResult>,
    scheduler: AnimationScheduler,
    frame: u64,
    player: Option<EntityId>,
    stats: CharacterStats,
    mesh_bytes: u64,
    /// Tags of the placed figures this layer shows (module docs, "Placed figures").
    shown: HashSet<u64>,
    /// Draw through the lit pipeline (`walker.md` §14.5).
    lit: bool,
    /// Characters cast the sun's shadow.
    shadows: bool,
    /// Leave the player's own character out (the free-fly camera, `walker.md` §14.5).
    hide_player: bool,
    /// The distance rules of the frame.
    distances: CharacterFrame,
}

impl std::fmt::Debug for CharacterLayer {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("CharacterLayer")
            .field("stats", &self.stats)
            .finish_non_exhaustive()
    }
}

fn seed_of(id: EntityId) -> u32 {
    // A small integer hash: phases differ between neighbours.
    let mut x = id.0.wrapping_mul(0x9E37_79B9_7F4A_7C15);
    x ^= x >> 29;
    (x as u32) ^ ((x >> 32) as u32)
}

impl CharacterLayer {
    /// A layer whose figure and sampling jobs run on `pool`.
    #[must_use]
    pub fn new(config: CharacterConfig, ctx: Arc<FigureContext>, pool: &WorkerPool) -> Self {
        Self::with_context(config, Some(ctx), pool)
    }

    fn with_context(
        config: CharacterConfig,
        ctx: Option<Arc<FigureContext>>,
        pool: &WorkerPool,
    ) -> Self {
        let (tx, rx) = channel();
        Self {
            config,
            ctx,
            pool: pool.clone(),
            store: EntityStore::with_config(StoreConfig {
                max_entities: config.max_entities,
            }),
            changes: Changes::new(),
            actors: Column::new(),
            slots: Vec::new(),
            free: Vec::new(),
            by_key: HashMap::new(),
            queue: std::collections::VecDeque::new(),
            in_flight: 0,
            tx,
            rx,
            scheduler: AnimationScheduler::new(pool, SchedulerConfig::default()),
            frame: 0,
            player: None,
            stats: CharacterStats::default(),
            mesh_bytes: 0,
            shown: HashSet::new(),
            lit: false,
            shadows: false,
            hide_player: false,
            distances: CharacterFrame::default(),
        }
    }

    /// Counters of the last frame.
    #[must_use]
    pub fn stats(&self) -> CharacterStats {
        self.stats
    }

    /// The entity store (read-only, for tests and diagnostics).
    #[must_use]
    pub fn store(&self) -> &EntityStore<CharRender, CharAnim> {
        &self.store
    }

    /// The region-frame position and heading of the entity `id` (tests, diagnostics).
    #[must_use]
    pub fn entity_pose(&self, id: EntityId) -> Option<(WorldPos, f32)> {
        let e = self.store.entity_for(NetId(id.0))?;
        let a = self.actors.get(e)?;
        let t = self.store.transform(e)?;
        Some((
            WorldPos::new(a.region, t.position[0], t.position[1], t.position[2]),
            a.heading,
        ))
    }

    /// The entities whose figure the data places, with their tags (tests, diagnostics).
    pub fn placed(&self) -> impl Iterator<Item = (EntityId, u64)> + '_ {
        self.actors
            .iter()
            .filter_map(|(_, a)| Some((a.net, a.tag?)))
    }

    /// Whether every figure in use is on the GPU (or failed) and every visible animated
    /// character has a pose.
    #[must_use]
    pub fn settled(&self) -> bool {
        let figures = self
            .slots
            .iter()
            .all(|s| s.refs == 0 || matches!(s.state, SlotState::Ready(_) | SlotState::Failed));
        figures
            && self.store.animated().all(|(e, a, _)| {
                self.actors.get(e).is_some_and(|x| x.hidden) || self.scheduler.has_pose(a.instance)
            })
            && self
                .actors
                .iter()
                .all(|(_, a)| a.slot.is_none() || a.slot == a.attached || a.hidden)
    }

    /// Whether entity `e` can be drawn: its figure is on the GPU and, when it is animated,
    /// it has a sampled pose (a skinned character is never drawn in its bind pose).
    fn drawable(&self, e: Entity) -> bool {
        let Some(render) = self.store.render(e) else {
            return false;
        };
        matches!(
            self.slots.get(render.slot as usize).map(|s| &s.state),
            Some(SlotState::Ready(_))
        ) && self
            .store
            .anim(e)
            .is_none_or(|a| self.scheduler.has_pose(a.instance))
    }

    /// Whether the figure of the placed entity with `tag` is drawn by this layer (on the
    /// GPU, posed and within the draw distance at the last update). The object layer leaves
    /// out the static instances of exactly these figures (`walker.md` §10.2), so a figure
    /// still building, failed, released or too far stays a static instance. Hidden markers
    /// count as shown (they are not drawn at all).
    #[must_use]
    pub fn shows_figure(&self, tag: u64) -> bool {
        self.shown.contains(&tag)
    }

    /// The tags of [`CharacterLayer::shows_figure`].
    #[must_use]
    pub fn shown_figures(&self) -> &HashSet<u64> {
        &self.shown
    }

    /// Recomputes [`CharacterLayer::shown_figures`] for a viewer at `eye`: the placed
    /// figures within the character draw distance (beyond it the object layer draws them).
    fn collect_shown(&mut self, eye: Option<WorldPos>) {
        let max_d = f64::from(character_draw_distance(
            &self.distances,
            self.config.draw_distance,
            false,
        ));
        let mut shown = std::mem::take(&mut self.shown);
        shown.clear();
        for (e, a) in self.actors.iter() {
            let Some(tag) = a.tag else { continue };
            if a.hidden {
                if a.slot.is_none() {
                    shown.insert(tag); // a marker: not drawn anywhere
                }
                continue;
            }
            if !self.drawable(e) {
                continue;
            }
            if let Some(eye) = eye
                && let Some(t) = self.store.transform(e)
            {
                let d = (t.position[0] - eye.x)
                    .hypot(t.position[1] - eye.y)
                    .hypot(t.position[2] - eye.z);
                if eye.region != a.region || d > max_d {
                    continue;
                }
            }
            shown.insert(tag);
        }
        self.shown = shown;
    }

    // ----- figures ---------------------------------------------------------------------

    /// Takes a reference to the figure of `req`, queueing its job when it is new.
    fn acquire(&mut self, req: FigureRequest) -> u32 {
        let key = req.key();
        if let Some(&slot) = self.by_key.get(&key) {
            let s = &mut self.slots[slot as usize];
            s.refs += 1;
            s.unused_since = None;
            return slot;
        }
        let slot = match self.free.pop() {
            Some(i) => i,
            None => {
                self.slots.push(Slot {
                    key: None,
                    request: None,
                    generation: 0,
                    refs: 0,
                    unused_since: None,
                    state: SlotState::Free,
                });
                u32::try_from(self.slots.len() - 1).unwrap_or(u32::MAX)
            }
        };
        let s = &mut self.slots[slot as usize];
        s.generation += 1;
        s.key = Some(key.clone());
        s.request = Some(req);
        s.refs = 1;
        s.unused_since = None;
        s.state = SlotState::Queued;
        self.by_key.insert(key, slot);
        self.queue.push_back(slot);
        slot
    }

    fn release(&mut self, slot: u32) {
        if let Some(s) = self.slots.get_mut(slot as usize) {
            s.refs = s.refs.saturating_sub(1);
            if s.refs == 0 {
                s.unused_since = Some(Instant::now());
            }
        }
    }

    fn start_jobs(&mut self) {
        while self.in_flight < MAX_JOBS {
            let Some(slot) = self.queue.pop_front() else {
                break;
            };
            let Some(s) = self.slots.get(slot as usize) else {
                continue;
            };
            if !matches!(s.state, SlotState::Queued) {
                continue;
            }
            if s.refs == 0 {
                // Released while queued: never built.
                self.forget_slot(slot as usize);
                continue;
            }
            let Some(ctx) = self.ctx.clone() else {
                self.slots[slot as usize].state = SlotState::Failed;
                continue;
            };
            let s = &mut self.slots[slot as usize];
            let Some(req) = s.request.clone() else {
                continue;
            };
            let token = CancelToken::new();
            let guard = JobGuard {
                slot,
                generation: s.generation,
                tx: self.tx.clone(),
                sent: false,
            };
            let spawned = self
                .pool
                .spawn(Priority::Visible, token.clone(), move |job| {
                    let r = std::panic::catch_unwind(std::panic::AssertUnwindSafe(|| {
                        build_figure(&ctx, &req, &|| job.is_cancelled())
                    }));
                    guard.send(match r {
                        Ok(Ok(a)) => Ok(a),
                        Ok(Err(e)) => Err(e.to_string()),
                        Err(_) => Err("the figure job panicked".into()),
                    });
                });
            match spawned {
                Ok(_) => {
                    s.state = SlotState::Building(token);
                    self.in_flight += 1;
                }
                Err(e) => {
                    // The guard reported the drop; retry on a later frame.
                    tracing::debug!("figure job not queued: {e}");
                    s.state = SlotState::Building(token);
                    self.in_flight += 1;
                    break;
                }
            }
        }
    }

    fn poll_jobs(&mut self) {
        while let Ok((slot, generation, result)) = self.rx.try_recv() {
            self.in_flight = self.in_flight.saturating_sub(1);
            let Some(s) = self.slots.get_mut(slot as usize) else {
                continue;
            };
            if s.generation != generation || !matches!(s.state, SlotState::Building(_)) {
                continue;
            }
            if s.refs == 0 {
                // Released while building: drop the result (and its prepared streams).
                self.forget_slot(slot as usize);
                continue;
            }
            s.state = match result {
                Ok(asset) => {
                    for n in asset.notes.iter().take(8) {
                        tracing::debug!(slot, "figure: {n}");
                    }
                    tracing::debug!(
                        slot,
                        body = ?s.request.as_ref().map(|r| r.body),
                        parts = asset.parts.len(),
                        skinned = asset.parts.iter().filter(|p| p.skinned).count(),
                        animated = asset.animated(),
                        idle = ?asset.clips.idle.iter().map(|c| format!("{:#010x}@{}", c.did, c.layer)).collect::<Vec<_>>(),
                        walk = ?asset.clips.walk.as_ref().map(|c| format!("{:#010x}", c.did)),
                        run = ?asset.clips.run.as_ref().map(|c| format!("{:#010x}", c.did)),
                        build_ms = asset.build_ms,
                        "figure built"
                    );
                    SlotState::Waiting(Arc::new(asset), Instant::now())
                }
                Err(e) if e == "job dropped" => {
                    self.queue.push_back(slot);
                    SlotState::Queued
                }
                Err(e) => {
                    tracing::warn!(slot, "figure failed: {e}");
                    SlotState::Failed
                }
            };
        }
    }

    fn upload(
        &mut self,
        renderer: &mut Renderer<'_>,
        budget: &mut UploadBudget,
        objects: &mut ObjectLayer,
        now: Instant,
    ) {
        for i in 0..self.slots.len() {
            let s = &mut self.slots[i];
            let SlotState::Waiting(asset, after) = &s.state else {
                continue;
            };
            if s.refs == 0 || now < *after {
                continue;
            }
            let asset = Arc::clone(asset);
            if !budget.try_take(asset.gpu_bytes()) {
                break;
            }
            let mut parts = Vec::with_capacity(asset.parts.len());
            let mut failed = None;
            for p in &asset.parts {
                let prepared = p.mesh.lock().unwrap_or_else(PoisonError::into_inner).take();
                let Some(prepared) = prepared else {
                    failed = Some("streams already taken".to_owned());
                    break;
                };
                let bytes = prepared.gpu_bytes();
                match renderer.upload_prepared(prepared) {
                    Ok(mesh) => parts.push(GpuPart {
                        mesh,
                        plans: p.plans.clone(),
                        skinned: p.skinned,
                        held: p.held,
                        bytes,
                    }),
                    Err(e) => {
                        failed = Some(e.to_string());
                        break;
                    }
                }
            }
            if let Some(why) = failed {
                for p in parts {
                    renderer.destroy_mesh(p.mesh);
                }
                tracing::warn!(slot = i, "figure upload failed: {why}");
                s.state = SlotState::Failed;
                continue;
            }
            let mut textures: Vec<TexRequest> = parts
                .iter()
                .flat_map(|p| p.plans.iter().flat_map(MaterialPlan::textures))
                .collect();
            textures.sort_unstable();
            textures.dedup();
            objects.acquire_textures(textures.iter().copied());
            let (mut lo, mut hi) = (Vec3::splat(f32::MAX), Vec3::splat(f32::MIN));
            for p in parts.iter().filter(|p| p.held.is_none()) {
                if let Some(info) = renderer.mesh_info(p.mesh) {
                    let c = info.sphere.center;
                    let r = info.sphere.radius;
                    if c.is_finite() && r.is_finite() {
                        lo = lo.min(c - Vec3::splat(r));
                        hi = hi.max(c + Vec3::splat(r));
                    }
                }
            }
            let (center, radius) = if lo.x <= hi.x {
                ((lo + hi) * 0.5, ((hi - lo) * 0.5).length() + 1.0)
            } else {
                (Vec3::new(0.0, 0.0, 1.0), 2.0)
            };
            self.mesh_bytes += parts.iter().map(|p| p.bytes).sum::<u64>();
            let rest = RestPose::of(&asset);
            s.state = SlotState::Ready(GpuFigure {
                asset,
                parts,
                textures,
                center,
                radius,
                rest,
            });
        }
    }

    fn free_slot(&mut self, i: usize, renderer: &mut Renderer<'_>, objects: &mut ObjectLayer) {
        if let SlotState::Ready(g) = std::mem::replace(&mut self.slots[i].state, SlotState::Free) {
            for p in g.parts {
                renderer.destroy_mesh(p.mesh);
                self.mesh_bytes = self.mesh_bytes.saturating_sub(p.bytes);
            }
            objects.release_textures(g.textures);
        }
        self.forget_slot(i);
    }

    /// Frees slot `i` when it holds nothing on the GPU (a `Ready` slot is left alone: use
    /// `free_slot`): cancels its job, drops its built asset and its key.
    fn forget_slot(&mut self, i: usize) {
        let Some(s) = self.slots.get_mut(i) else {
            return;
        };
        match std::mem::replace(&mut s.state, SlotState::Free) {
            SlotState::Free => return,
            SlotState::Ready(g) => {
                s.state = SlotState::Ready(g);
                return;
            }
            SlotState::Building(t) => t.cancel(),
            SlotState::Queued | SlotState::Waiting(..) | SlotState::Failed => {}
        }
        if let Some(k) = s.key.take() {
            self.by_key.remove(&k);
        }
        s.request = None;
        s.refs = 0;
        s.unused_since = None;
        s.generation += 1;
        self.free.push(u32::try_from(i).unwrap_or(u32::MAX));
    }

    /// Drops unreferenced figures: at once when they hold nothing on the GPU, after
    /// [`FIGURE_KEEP`] when they do.
    fn evict(&mut self, renderer: &mut Renderer<'_>, objects: &mut ObjectLayer, now: Instant) {
        for i in 0..self.slots.len() {
            let s = &self.slots[i];
            if s.refs != 0 || matches!(s.state, SlotState::Free) {
                continue;
            }
            if !matches!(s.state, SlotState::Ready(_)) {
                self.forget_slot(i);
            } else if s
                .unused_since
                .is_some_and(|t| now.saturating_duration_since(t) >= FIGURE_KEEP)
            {
                self.free_slot(i, renderer, objects);
            }
        }
    }

    /// Frees every figure's GPU resources and detaches every entity (a region change
    /// cleared the renderer's resources); figures in use are rebuilt.
    pub fn reset_gpu(&mut self, renderer: &mut Renderer<'_>, objects: &mut ObjectLayer) {
        let entities: Vec<Entity> = self.actors.entities().to_vec();
        for e in entities {
            self.store.remove_render(e);
            self.store.remove_anim(e);
            if let Some(a) = self.actors.get_mut(e) {
                a.attached = None;
            }
        }
        self.apply_changes();
        for i in 0..self.slots.len() {
            if matches!(self.slots[i].state, SlotState::Free) {
                continue;
            }
            if self.slots[i].refs == 0 {
                self.free_slot(i, renderer, objects);
                continue;
            }
            // Only GPU resources are lost: a Ready figure (whose prepared streams went to
            // the GPU) is rebuilt; queued, building and built figures are CPU-side and stay
            // as they are; failed ones stay failed.
            if !matches!(self.slots[i].state, SlotState::Ready(_)) {
                continue;
            }
            if let SlotState::Ready(g) =
                std::mem::replace(&mut self.slots[i].state, SlotState::Queued)
            {
                for p in g.parts {
                    renderer.destroy_mesh(p.mesh);
                    self.mesh_bytes = self.mesh_bytes.saturating_sub(p.bytes);
                }
                objects.release_textures(g.textures);
            }
            self.slots[i].generation += 1;
            self.queue.push_back(u32::try_from(i).unwrap_or(u32::MAX));
        }
        self.shown.clear();
    }

    // ----- entities --------------------------------------------------------------------

    fn spawn(
        &mut self,
        id: EntityId,
        region: u8,
        position: [f64; 3],
        heading: f32,
        tag: Option<u64>,
        now: Instant,
    ) -> Option<Entity> {
        if let Some(old) = self.store.entity_for(NetId(id.0)) {
            self.despawn(old);
        }
        let e = match self.store.spawn_networked(
            NetId(id.0),
            Transform::from_position_yaw(position, heading - std::f32::consts::FRAC_PI_2),
        ) {
            Ok(e) => e,
            Err(err) => {
                tracing::debug!(id = id.0, "character not spawned: {err}");
                return None;
            }
        };
        let actor = Actor {
            net: id,
            tag,
            snap: tag.is_none(),
            hidden: false,
            slot: None,
            attached: None,
            region,
            base: position,
            velocity: [0.0; 3],
            at: now,
            heading,
            speed: 0.0,
            seed: seed_of(id),
            fade: tessera_world::draw_distance::DrawFade::new(),
            seen: false,
        };
        if self.actors.insert(e, actor).is_err() {
            self.store.despawn(e);
            return None;
        }
        Some(e)
    }

    fn despawn(&mut self, e: Entity) {
        if let Some(a) = self.actors.remove(e)
            && let Some(slot) = a.slot
        {
            self.release(slot);
        }
        self.store.despawn(e);
    }

    /// Requests `appearance` for entity `e` (replacing its figure).
    fn set_appearance(&mut self, e: Entity, appearance: &Appearance, shared: &Mutex<SourceShared>) {
        let Some(actor) = self.actors.get(e).cloned() else {
            return;
        };
        let info = actor.tag.and_then(|t| {
            shared
                .lock()
                .unwrap_or_else(PoisonError::into_inner)
                .figure_info(t)
        });
        let role = info.as_ref().and_then(|i| i.role);
        let hidden = matches!(role, Some(WeenieRole::Marker)) && !self.config.markers;
        let req = FigureRequest {
            body: appearance.body,
            equipment: appearance
                .equipment
                .iter()
                .take(MAX_EQUIPMENT)
                .copied()
                .collect(),
            instance: info.as_ref().and_then(|i| i.instance.clone()),
            // Placements without an instance set share one figure per object (`walker.md`
            // §10.4); an instance set is identified by its placement.
            instance_key: info
                .as_ref()
                .and_then(|i| i.instance.as_ref())
                .and(actor.tag),
            static_only: matches!(role, Some(WeenieRole::Door | WeenieRole::Marker)),
            avatar_height: None,
            bone_scale: None,
        };
        let slot = (!hidden && matches!(req.body, Body::Character { .. } | Body::Object(_)))
            .then(|| self.acquire(req));
        if let Some(old) = actor.slot {
            self.release(old);
        }
        if let Some(a) = self.actors.get_mut(e) {
            a.slot = slot;
            a.hidden = hidden;
        }
        // The model scale lives in the store transform, which the draws and the culling
        // sphere read; the per-frame position update keeps it (`..t`).
        if let Some(t) = self.store.transform(e).copied() {
            let s = appearance.scale;
            let _ = self
                .store
                .set_transform(e, Transform { scale: [s; 3], ..t });
        }
        // Keep the old figure drawn until the new one is attached.
    }

    /// Applies the session's entity events (step 1 of the module docs). `region` is the
    /// player's current region (positions of moves are in it).
    pub fn apply_events(
        &mut self,
        events: &mut Vec<Event>,
        shared: &Mutex<SourceShared>,
        player: Option<EntityId>,
        region: u8,
        now: Instant,
    ) {
        if player != self.player {
            if let Some(old) = self.player.take()
                && let Some(e) = self.store.entity_for(NetId(old.0))
            {
                self.despawn(e);
            }
            self.player = player;
        }
        for ev in events.drain(..) {
            match ev {
                Event::EntityCreated {
                    id,
                    location,
                    heading,
                    appearance,
                    tag,
                    ..
                } => {
                    let Ok(r) = u8::try_from(location.region.0) else {
                        continue;
                    };
                    if let Some(e) = self.spawn(
                        id,
                        r,
                        [location.x, location.y, location.z],
                        heading,
                        tag,
                        now,
                    ) {
                        self.set_appearance(e, &appearance, shared);
                    }
                }
                Event::AppearanceChanged { id, appearance } => {
                    let e = match self.store.entity_for(NetId(id.0)) {
                        Some(e) => Some(e),
                        None if Some(id) == self.player => {
                            self.spawn(id, region, [0.0; 3], 0.0, None, now)
                        }
                        None => None,
                    };
                    if let Some(e) = e {
                        if let Some(a) = self.actors.get_mut(e)
                            && Some(id) == self.player
                        {
                            a.snap = false;
                            a.hidden = true; // until the walker places it
                        }
                        self.set_appearance(e, &appearance, shared);
                    }
                }
                Event::EntityMoved {
                    id,
                    transform,
                    velocity,
                } => {
                    if Some(id) == self.player {
                        continue; // the walker drives the avatar
                    }
                    let Some(e) = self.store.entity_for(NetId(id.0)) else {
                        continue;
                    };
                    if let Some(a) = self.actors.get_mut(e) {
                        let p = transform.position;
                        a.base = [f64::from(p.x), f64::from(p.y), f64::from(p.z)];
                        a.velocity = [velocity.x, velocity.y, velocity.z];
                        a.at = now;
                        a.heading = transform.heading;
                        a.speed = velocity.x.hypot(velocity.y);
                        a.region = region;
                    }
                }
                Event::EntityDespawned { id } => {
                    if let Some(e) = self.store.entity_for(NetId(id.0)) {
                        self.despawn(e);
                    }
                }
                Event::StaticsCleared => {
                    let placed: Vec<Entity> = self
                        .actors
                        .iter()
                        .filter(|(_, a)| a.tag.is_some())
                        .map(|(e, _)| e)
                        .collect();
                    for e in placed {
                        self.despawn(e);
                    }
                }
                Event::Disconnected { .. } => {
                    let all: Vec<Entity> = self.actors.entities().to_vec();
                    for e in all {
                        self.despawn(e);
                    }
                    self.player = None;
                }
                _ => {}
            }
        }
    }

    /// Puts every networked character (not the player, not placed figures) that `sample`
    /// knows at its interpolated state (`tessera-protocol.md` §8.6): position, heading and
    /// the velocity that drives its locomotion (zero ground speed and velocity when the
    /// motion byte says Standing, or when the sample is `SampleKind::Held`: updates late
    /// past the extrapolation limit stand the character still). Characters `sample` does not know keep the extrapolation of their
    /// last move. Positions are in `region`, the player's current region (as for moves).
    pub fn apply_samples(
        &mut self,
        sample: &dyn Fn(EntityId) -> Option<tessera_net::interp::Sample>,
        region: u8,
        now: Instant,
    ) {
        let player = self.player;
        for a in self.actors.values_mut() {
            if a.tag.is_some() || Some(a.net) == player {
                continue;
            }
            let Some(s) = sample(a.net) else { continue };
            if !(s.position.is_finite() && s.heading.is_finite() && s.velocity.is_finite()) {
                continue;
            }
            a.base = [
                f64::from(s.position.x),
                f64::from(s.position.y),
                f64::from(s.position.z),
            ];
            a.region = region;
            a.at = now;
            a.heading = s.heading;
            // Held: the entity's updates are late (past the extrapolation limit, §8.6), so
            // it stands where the sampler holds it instead of walking on the spot.
            let still = s.motion.gait == tessera_net::interp::Gait::Standing
                || s.kind == tessera_net::interp::SampleKind::Held;
            if still {
                a.velocity = [0.0; 3];
                a.speed = 0.0;
            } else {
                a.velocity = [s.velocity.x, s.velocity.y, s.velocity.z];
                a.speed = s.velocity.x.hypot(s.velocity.y);
            }
        }
    }

    /// Places the avatar (walk mode): feet, heading (0 = east, counter-clockwise) and ground
    /// speed; `None` hides it (fly mode).
    pub fn set_player_pose(&mut self, pose: Option<(WorldPos, f32, f32)>) {
        let Some(id) = self.player else { return };
        let Some(e) = self.store.entity_for(NetId(id.0)) else {
            return;
        };
        let Some(a) = self.actors.get_mut(e) else {
            return;
        };
        match pose {
            Some((p, heading, speed)) => {
                a.hidden = false;
                a.region = p.region;
                a.base = [p.x, p.y, p.z];
                a.velocity = [0.0; 3];
                a.heading = heading;
                a.speed = speed;
            }
            None => a.hidden = true,
        }
    }

    /// Whether the avatar is on the GPU (the walker draws a stand-in until it is).
    #[must_use]
    pub fn player_ready(&self) -> bool {
        self.player
            .and_then(|id| self.store.entity_for(NetId(id.0)))
            .is_some_and(|e| self.drawable(e))
    }

    fn apply_changes(&mut self) {
        self.store.take_changes(&mut self.changes);
        for (_, a) in self.changes.anim_removed.drain(..) {
            self.scheduler.remove(a.instance);
        }
        // Render payloads are figure slots whose references the actors hold.
        self.changes.render_removed.clear();
    }

    /// Per frame (module docs, steps 1–3). `ground` answers terrain heights; `eye` is the
    /// viewer; `dt` the frame time.
    #[allow(clippy::too_many_arguments)]
    pub fn update(
        &mut self,
        renderer: &mut Renderer<'_>,
        budget: &mut UploadBudget,
        objects: &mut ObjectLayer,
        ground: &dyn Fn(u8, f64, f64) -> Option<f32>,
        eye: Option<WorldPos>,
        now: Instant,
        dt: f32,
    ) {
        self.frame += 1;
        self.start_jobs();
        self.poll_jobs();
        self.start_jobs();
        self.upload(renderer, budget, objects, now);

        // Positions: extrapolate simulated characters and put them on the ground.
        for (e, a) in self.actors.iter() {
            let age = now
                .saturating_duration_since(a.at)
                .min(EXTRAPOLATE)
                .as_secs_f64();
            let mut p = [
                a.base[0] + f64::from(a.velocity[0]) * age,
                a.base[1] + f64::from(a.velocity[1]) * age,
                a.base[2] + f64::from(a.velocity[2]) * age,
            ];
            if a.snap
                && let Some(h) = ground(a.region, p[0], p[1])
            {
                p[2] = f64::from(h);
            }
            if let Some(t) = self.store.transform(e).copied() {
                let yaw = a.heading - std::f32::consts::FRAC_PI_2;
                let (s, c) = (yaw * 0.5).sin_cos();
                let _ = self.store.set_transform(
                    e,
                    Transform {
                        position: p,
                        rotation: [0.0, 0.0, s, c],
                        ..t
                    },
                );
            }
        }

        self.attach_ready();

        // Animation (ARCHITECTURE §20.3 step 6): collect the previous frame's poses, then
        // advance and submit.
        let t = Instant::now();
        let collected = self.scheduler.collect();
        self.stats.collect_ms = t.elapsed().as_secs_f64() * 1e3;
        self.stats.worker_ms = collected.worker_time.as_secs_f64() * 1e3;
        let t = Instant::now();
        let far = f64::from(self.config.anim_distance);
        let mut sampled = 0usize;
        let frame = self.frame;
        let actors = &self.actors;
        let scheduler = &mut self.scheduler;
        for (e, anim, tf) in self.store.animated_mut() {
            let Some(a) = actors.get(e) else { continue };
            if a.hidden {
                continue;
            }
            // Beyond the animation distance a character keeps its last pose, but it gets
            // one job for its first pose wherever it is (it is not drawn without one).
            let near = eye.is_none_or(|eye| {
                let dx = tf.position[0] - eye.x;
                let dy = tf.position[1] - eye.y;
                eye.region == a.region && dx.hypot(dy) <= far
            });
            if !near && scheduler.has_pose(anim.instance) {
                continue;
            }
            drive(&mut anim.animator, &mut anim.loco, a.speed, dt);
            if matches!(
                scheduler.submit(anim.instance, &anim.animator, frame),
                Ok(Submitted::Queued)
            ) {
                sampled += 1;
            }
        }
        self.stats.submit_ms = t.elapsed().as_secs_f64() * 1e3;
        self.stats.sampled = sampled;
        self.evict(renderer, objects, now);
        self.collect_shown(eye);
        self.refresh_stats();
    }

    /// Gives entities whose requested figure became ready their render (and animation)
    /// components; a failed figure removes them.
    fn attach_ready(&mut self) {
        // Attach figures that became ready.
        let pending: Vec<(Entity, u32)> = self
            .actors
            .iter()
            .filter_map(|(e, a)| {
                let slot = a.slot?;
                (a.attached != Some(slot)).then_some((e, slot))
            })
            .collect();
        for (e, slot) in pending {
            let state = self.slots.get(slot as usize).map(|s| &s.state);
            if matches!(state, Some(SlotState::Failed)) {
                // Nothing to draw: drop the previous figure too.
                self.store.remove_render(e);
                self.store.remove_anim(e);
                if let Some(a) = self.actors.get_mut(e) {
                    a.attached = Some(slot);
                }
                continue;
            }
            let Some(SlotState::Ready(g)) = state else {
                continue;
            };
            let asset = Arc::clone(&g.asset);
            if self.store.set_render(e, CharRender { slot }).is_err() {
                continue;
            }
            self.store.remove_anim(e);
            if let Some(skel) = &asset.skeleton {
                let seed = self.actors.get(e).map_or(0, |a| a.seed);
                let phase = (seed % 10_000) as f32 / 1000.0;
                if let Some((animator, loco)) = make_animator(skel, &asset.clips, phase) {
                    match self.scheduler.add(Arc::clone(skel), asset.attach.clone()) {
                        Ok(instance) => {
                            if let Err(r) = self.store.set_anim(
                                e,
                                CharAnim {
                                    instance,
                                    animator,
                                    loco,
                                },
                            ) {
                                self.scheduler.remove(r.value.instance);
                            }
                        }
                        Err(err) => tracing::debug!("animation instance refused: {err}"),
                    }
                }
            }
            if let Some(a) = self.actors.get_mut(e) {
                a.attached = Some(slot);
            }
        }
        self.apply_changes();
    }

    fn refresh_stats(&mut self) {
        let mut s = CharacterStats {
            entities: self.store.len(),
            animated: self.store.animated().count(),
            posed: self
                .store
                .animated()
                .filter(|(_, a, _)| self.scheduler.has_pose(a.instance))
                .count(),
            gpu_bytes: self.mesh_bytes,
            ..self.stats
        };
        s.figures_ready = 0;
        s.figures_pending = 0;
        s.figures_failed = 0;
        for slot in &self.slots {
            if slot.refs == 0 {
                continue;
            }
            match slot.state {
                SlotState::Ready(_) => s.figures_ready += 1,
                SlotState::Failed => s.figures_failed += 1,
                SlotState::Free => {}
                _ => s.figures_pending += 1,
            }
        }
        self.stats = s;
    }

    /// Sets how characters are shaded: through the lit pipeline (stored-value colours and
    /// the lit inputs of `lighting.md` §4.1) and whether they cast the sun's shadow.
    pub fn set_shading(&mut self, lit: bool, shadows: bool) {
        self.lit = lit;
        self.shadows = shadows;
    }

    /// Leaves the player's own character out of the draws (free-fly mode) or not.
    pub fn set_hide_player(&mut self, hide: bool) {
        self.hide_player = hide;
    }

    /// Sets the distance rules of the frame ([`character_draw_distance`], the fade).
    pub fn set_distances(&mut self, frame: CharacterFrame) {
        self.distances = frame;
    }

    /// Uploads the palettes of the visible characters into `frame` and returns their draws
    /// (module docs, step 4). `camera` is the eye in `anchor`'s frame.
    pub fn prepare_draws(
        &mut self,
        frame: &mut Frame,
        anchor: &Anchor,
        camera: Vec3,
        frustum: &Frustum,
        objects: &ObjectLayer,
        out: &mut Vec<MeshDraw>,
    ) {
        let t = Instant::now();
        let cap = self.config.draw_distance;
        let df = self.distances;
        let mut drawn = 0usize;
        for (e, render, tf) in self.store.renderables() {
            let Some(a) = self.actors.get(e) else {
                continue;
            };
            let is_player = Some(a.net) == self.player;
            if a.hidden {
                continue;
            }
            // A placed figure is either a character or a static instance, never both
            // (`walker.md` §10.2): draw exactly the ones the object layer leaves out.
            match a.tag {
                Some(tag) if !self.shown.contains(&tag) => continue,
                None if !self.drawable(e) => continue,
                _ => {}
            }
            if self.hide_player && Some(a.net) == self.player {
                continue;
            }
            let Some(SlotState::Ready(g)) = self.slots.get(render.slot as usize).map(|s| &s.state)
            else {
                continue;
            };
            let Some(pos) = anchor.to_render(&WorldPos::new(
                a.region,
                tf.position[0],
                tf.position[1],
                tf.position[2],
            )) else {
                continue;
            };
            let scale = g.asset.model_scale * tf.scale[0];
            let scale = if scale.is_finite() && scale > 0.0 {
                scale
            } else {
                1.0
            };
            let rot = Quat::from_array(tf.rotation);
            let rot = if rot.is_finite() && rot.length_squared() > 0.0 {
                rot.normalize()
            } else {
                Quat::IDENTITY
            };
            let world = Mat4::from_scale_rotation_translation(Vec3::splat(scale), rot, pos);
            let center = world.transform_point3(g.center);
            let radius = g.radius * scale;
            let distance = ((center - camera).length() - radius).max(0.0);
            let max_d = character_draw_distance(&df, cap, is_player);
            let in_view = frustum.sphere_visible(center, radius);
            let Some(a) = self.actors.get_mut(e) else {
                continue;
            };
            // `walker.md` §11.4 step 1: the 1.5 s fade across D (`mesh.md` §8.7).
            let opacity = a
                .fade
                .update(distance, max_d, df.dt, df.camera_moved, !a.seen, df.lod);
            a.seen = in_view;
            if opacity <= 0.0 || !in_view {
                continue;
            }
            // A fading character casts no shadow (as fading objects, `walker.md` §14.5).
            let casts = self.shadows && opacity >= 1.0 && distance <= CHARACTER_SHADOW_DISTANCE;
            // The sampled pose, or the reference pose of a figure without an animator.
            let pose: Option<PoseView<'_>> = match self.store.anim(e) {
                Some(anim) => self
                    .scheduler
                    .pose(anim.instance)
                    .map(|p| (p.skin.as_slice(), p.attach.as_slice())),
                None => g
                    .rest
                    .as_ref()
                    .map(|r| (r.skin.as_slice(), r.attach.as_slice())),
            };
            let palette: Option<SkinPalette> = pose
                .filter(|(skin, _)| !skin.is_empty())
                .and_then(|(skin, _)| frame.skin_palette(skin).ok());
            drawn += 1;
            for part in &g.parts {
                let transform = match part.held {
                    None => world,
                    Some(key) => {
                        let Some(m) = pose.and_then(|(_, attach)| {
                            attach.iter().find(|(k, _)| *k == key).map(|(_, m)| *m)
                        }) else {
                            continue;
                        };
                        world * m
                    }
                };
                for (i, plan) in part.plans.iter().enumerate() {
                    let Some((material, lit)) =
                        surface_draw(plan, &|t| objects.texture(t), self.lit, opacity, casts)
                    else {
                        continue;
                    };
                    let mut d = MeshDraw::new(part.mesh, transform, material)
                        .with_submesh(u32::try_from(i).unwrap_or(u32::MAX))
                        .with_lit(lit);
                    if part.skinned
                        && let Some(p) = palette
                    {
                        d = d.with_skin(p);
                    }
                    out.push(d);
                }
            }
        }
        self.stats.drawn = drawn;
        self.stats.palette_ms = t.elapsed().as_secs_f64() * 1e3;
    }

    /// Frees every GPU resource and cancels outstanding work (shutdown).
    pub fn shutdown(&mut self, renderer: &mut Renderer<'_>, objects: &mut ObjectLayer) {
        let all: Vec<Entity> = self.actors.entities().to_vec();
        for e in all {
            self.despawn(e);
        }
        self.apply_changes();
        self.scheduler.cancel_all();
        for i in 0..self.slots.len() {
            if !matches!(self.slots[i].state, SlotState::Free) {
                self.free_slot(i, renderer, objects);
            }
        }
    }
}

#[cfg(test)]
mod tests {
    use tessera_assets::PoolConfig;
    use tessera_net::{DataId, EntityKind, Location, RegionId, Transform as NetTransform};

    use super::*;

    fn layer() -> CharacterLayer {
        let pool = WorkerPool::new(PoolConfig {
            workers: 1,
            ..PoolConfig::default()
        });
        CharacterLayer::with_context(CharacterConfig::default(), None, &pool)
    }

    fn look(object: u32) -> Box<Appearance> {
        Box::new(Appearance::new(Body::Object(DataId(object))))
    }

    fn created(id: u64, object: u32, tag: Option<u64>) -> Event {
        Event::EntityCreated {
            id: EntityId(id),
            kind: EntityKind::NonPlayer,
            name: String::new(),
            class: None,
            location: Location::new(RegionId(1), 100.0 + id as f64, 200.0, 5.0),
            heading: 0.0,
            appearance: look(object),
            tag,
        }
    }

    fn apply(l: &mut CharacterLayer, events: Vec<Event>, player: Option<EntityId>) {
        let shared = Mutex::new(SourceShared::default());
        let mut events = events;
        l.apply_events(&mut events, &shared, player, 1, Instant::now());
        assert!(events.is_empty(), "every event is consumed");
        l.apply_changes();
    }

    fn refs(l: &CharacterLayer) -> Vec<u32> {
        l.slots
            .iter()
            .filter(|s| !matches!(s.state, SlotState::Free))
            .map(|s| s.refs)
            .collect()
    }

    #[test]
    fn character_distances_follow_the_class_g_and_the_multiplier() {
        let f = CharacterFrame::default();
        assert_eq!(character_draw_distance(&f, 400.0, false), 80.0);
        assert_eq!(character_draw_distance(&f, 400.0, true), 240.0);
        // `--object-draw-distance UltraHigh` (M_d 2): 160 m, not the old fixed 80 m.
        let ultra = CharacterFrame {
            multiplier: 2.0,
            ..f
        };
        assert_eq!(character_draw_distance(&ultra, 400.0, false), 160.0);
        // Fog pulls G in: G wins.
        let fog = CharacterFrame { g: 60.0, ..f };
        assert_eq!(character_draw_distance(&fog, 400.0, false), 60.0);
        // The option is a cap.
        assert_eq!(character_draw_distance(&ultra, 50.0, false), 50.0);
        // Per-class distances off: the cap alone; hostile values: the cap.
        let off = CharacterFrame { lod: false, ..f };
        assert_eq!(character_draw_distance(&off, 400.0, false), 400.0);
        let nan = CharacterFrame {
            multiplier: f32::NAN,
            ..f
        };
        assert_eq!(character_draw_distance(&nan, 400.0, false), 400.0);
    }

    #[test]
    fn created_replaces_an_entity_with_the_same_id() {
        let mut l = layer();
        apply(&mut l, vec![created(7, 0x4700_0001, None)], None);
        apply(&mut l, vec![created(7, 0x4700_0002, None)], None);
        assert_eq!(l.store.len(), 1);
        // The first figure lost its only reference, the second has one.
        assert_eq!(refs(&l), vec![0, 1]);
        let (p, _) = l.entity_pose(EntityId(7)).expect("pose");
        assert_eq!((p.region, p.x, p.y), (1, 107.0, 200.0));
    }

    #[test]
    fn appearance_scale_reaches_the_store_transform() {
        // Regression: AppearanceChanged stored its scale in an actor field nothing read,
        // so the avatar (spawned from AppearanceChanged) always drew at scale 1.
        let mut l = layer();
        let player = Some(EntityId(9));
        let mut big = look(0x4700_0001);
        big.scale = 1.5;
        apply(
            &mut l,
            vec![Event::AppearanceChanged {
                id: EntityId(9),
                appearance: big,
            }],
            player,
        );
        let e = l.store.entity_for(NetId(9)).expect("avatar spawned");
        assert_eq!(l.store.transform(e).expect("transform").scale, [1.5; 3]);
        let mut small = look(0x4700_0001);
        small.scale = 0.5;
        apply(&mut l, vec![created(3, 0x4700_0001, None)], player);
        apply(
            &mut l,
            vec![Event::AppearanceChanged {
                id: EntityId(3),
                appearance: small,
            }],
            player,
        );
        let e3 = l.store.entity_for(NetId(3)).expect("npc");
        assert_eq!(l.store.transform(e3).expect("transform").scale, [0.5; 3]);
        // EntityCreated applies its appearance scale through the same path.
        let mut made = created(4, 0x4700_0001, None);
        if let Event::EntityCreated { appearance, .. } = &mut made {
            appearance.scale = 2.0;
        }
        apply(&mut l, vec![made], player);
        let e4 = l.store.entity_for(NetId(4)).expect("npc");
        assert_eq!(l.store.transform(e4).expect("transform").scale, [2.0; 3]);
    }

    #[test]
    fn statics_cleared_removes_only_tagged_entities_and_disconnect_everything() {
        let mut l = layer();
        apply(
            &mut l,
            vec![
                created(1, 0x4700_0001, Some(10)),
                created(2, 0x4700_0001, Some(11)),
                created(3, 0x4700_0001, None),
            ],
            None,
        );
        assert_eq!(l.store.len(), 3);
        assert_eq!(l.placed().count(), 2);
        apply(&mut l, vec![Event::StaticsCleared], None);
        assert_eq!(l.store.len(), 1);
        assert!(l.entity_pose(EntityId(3)).is_some());
        apply(
            &mut l,
            vec![Event::Disconnected {
                reason: tessera_net::DisconnectReason::ClientRequested,
            }],
            None,
        );
        assert_eq!(l.store.len(), 0);
        assert_eq!(l.player, None);
    }

    #[test]
    fn moves_apply_except_the_players() {
        let mut l = layer();
        let player = Some(EntityId(1));
        apply(
            &mut l,
            vec![created(1, 0x4700_0001, None), created(2, 0x4700_0001, None)],
            player,
        );
        let moved = |id: u64| Event::EntityMoved {
            id: EntityId(id),
            transform: NetTransform {
                position: tessera_net::Vec3::new(50.0, 60.0, 7.0),
                heading: 1.0,
            },
            velocity: tessera_net::Vec3::new(3.0, 4.0, 0.0),
        };
        apply(&mut l, vec![moved(1), moved(2)], player);
        let e1 = l.store.entity_for(NetId(1)).unwrap();
        let e2 = l.store.entity_for(NetId(2)).unwrap();
        assert_eq!(
            l.actors.get(e1).unwrap().base,
            [101.0, 200.0, 5.0],
            "avatar unmoved"
        );
        let a2 = l.actors.get(e2).unwrap();
        assert_eq!(a2.base, [50.0, 60.0, 7.0]);
        assert_eq!((a2.heading, a2.speed), (1.0, 5.0));
    }

    #[test]
    fn samples_place_networked_characters_only() {
        use tessera_net::interp::{Gait, Motion, Sample, SampleKind};
        let mut l = layer();
        apply(
            &mut l,
            vec![
                created(1, 0x7900_0001, None),
                created(2, 0x7900_0002, Some(9)),
            ],
            Some(EntityId(3)),
        );
        let now = Instant::now();
        let sample = |id: EntityId| {
            (id.0 != 3).then_some(Sample {
                position: tessera_net::Vec3::new(10.0 + id.0 as f32, 20.0, 3.0),
                heading: 0.5,
                velocity: tessera_net::Vec3::new(3.0, 4.0, 0.0),
                motion: Motion::new(Gait::Walking, false),
                kind: SampleKind::Interpolated,
            })
        };
        l.apply_samples(&sample, 1, now);
        let actor = |l: &CharacterLayer, id: u64| {
            let e = l.store.entity_for(NetId(id)).expect("entity");
            l.actors.get(e).cloned().expect("actor")
        };
        let a = actor(&l, 1);
        assert_eq!(a.base, [11.0, 20.0, 3.0]);
        assert_eq!((a.heading, a.speed, a.at), (0.5, 5.0, now));
        let placed = actor(&l, 2);
        assert_ne!(
            placed.base[0], 12.0,
            "placed figures keep the data's position"
        );
        // Standing means no locomotion, whatever the interpolated velocity.
        let standing = |_: EntityId| {
            Some(Sample {
                position: tessera_net::Vec3::new(1.0, 2.0, 3.0),
                heading: 0.0,
                velocity: tessera_net::Vec3::new(0.1, 0.0, 0.0),
                motion: Motion::STANDING,
                kind: SampleKind::Held,
            })
        };
        l.apply_samples(&standing, 1, now);
        assert_eq!(actor(&l, 1).speed, 0.0);
        // Held with a walking gait (updates late, §8.6): no walk cycle on the spot and no
        // frame-to-frame extrapolation either.
        let held = |_: EntityId| {
            Some(Sample {
                position: tessera_net::Vec3::new(5.0, 6.0, 7.0),
                heading: 0.25,
                velocity: tessera_net::Vec3::new(3.0, 0.0, 0.0),
                motion: Motion::new(Gait::Walking, false),
                kind: SampleKind::Held,
            })
        };
        l.apply_samples(&sample, 1, now);
        assert_eq!(actor(&l, 1).speed, 5.0);
        l.apply_samples(&held, 1, now);
        let a = actor(&l, 1);
        assert_eq!((a.base, a.heading), ([5.0, 6.0, 7.0], 0.25));
        assert_eq!((a.speed, a.velocity), (0.0, [0.0; 3]));
        // Extrapolated keeps the locomotion.
        let extrapolated = |id: EntityId| {
            sample(id).map(|s| Sample {
                kind: SampleKind::Extrapolated,
                ..s
            })
        };
        l.apply_samples(&extrapolated, 1, now);
        assert_eq!(actor(&l, 1).speed, 5.0);
        l.apply_samples(&standing, 1, now);
        // Non-finite samples are ignored.
        let bad = |_: EntityId| {
            Some(Sample {
                position: tessera_net::Vec3::new(f32::NAN, 0.0, 0.0),
                heading: 0.0,
                velocity: tessera_net::Vec3::ZERO,
                motion: Motion::STANDING,
                kind: SampleKind::Held,
            })
        };
        l.apply_samples(&bad, 1, now);
        assert_eq!(actor(&l, 1).base, [1.0, 2.0, 3.0]);
    }

    #[test]
    fn identical_placements_without_an_instance_set_share_one_figure() {
        let mut l = layer();
        apply(
            &mut l,
            vec![
                created(1, 0x4700_0001, Some(10)),
                created(2, 0x4700_0001, Some(11)),
                created(3, 0x4700_0002, Some(12)),
            ],
            None,
        );
        assert_eq!(refs(&l), vec![2, 1]);
        apply(
            &mut l,
            vec![Event::EntityDespawned { id: EntityId(1) }],
            None,
        );
        assert_eq!(refs(&l), vec![1, 1]);
    }

    #[test]
    fn released_figures_are_never_built() {
        let mut l = layer();
        apply(
            &mut l,
            vec![
                created(1, 0x4700_0001, Some(10)),
                created(2, 0x4700_0002, Some(11)),
            ],
            None,
        );
        apply(&mut l, vec![Event::StaticsCleared], None);
        assert_eq!(refs(&l), vec![0, 0]);
        l.start_jobs();
        assert_eq!(l.in_flight, 0, "no job for an unreferenced figure");
        assert!(refs(&l).is_empty(), "both slots freed");
        assert!(l.by_key.is_empty());
        assert_eq!(l.free.len(), 2);
        // A later request reuses a freed slot and builds again.
        apply(&mut l, vec![created(3, 0x4700_0001, Some(12))], None);
        assert_eq!(refs(&l), vec![1]);
        assert_eq!(l.slots.len(), 2);
    }

    #[test]
    fn a_failed_figure_is_not_shown_so_its_static_instance_stays() {
        let mut l = layer();
        apply(&mut l, vec![created(1, 0x4700_0001, Some(10))], None);
        assert!(!l.settled(), "figure queued");
        l.start_jobs(); // no context: the figure fails
        l.poll_jobs();
        l.attach_ready();
        l.collect_shown(None);
        assert!(!l.shows_figure(10));
        assert!(l.shown_figures().is_empty());
        let e = l.store.entity_for(NetId(1)).unwrap();
        assert!(l.store.render(e).is_none());
        assert!(!l.drawable(e));
        assert!(l.settled(), "failed figures count as settled");
        l.refresh_stats();
        assert_eq!(l.stats().figures_failed, 1);
    }

    #[test]
    fn building_and_waiting_figures_survive_a_gpu_reset_bookkeeping() {
        // Without a renderer, check the slot rules reset_gpu relies on: forgetting leaves a
        // Ready slot alone and frees everything else.
        let mut l = layer();
        apply(&mut l, vec![created(1, 0x4700_0001, None)], None);
        let token = CancelToken::new();
        l.slots[0].state = SlotState::Building(token.clone());
        l.slots[0].refs = 0;
        l.forget_slot(0);
        assert!(
            token.is_cancelled(),
            "a building job is cancelled when forgotten"
        );
        assert!(matches!(l.slots[0].state, SlotState::Free));
        assert_eq!(l.slots[0].generation, 2, "a late result is ignored");
    }
}
