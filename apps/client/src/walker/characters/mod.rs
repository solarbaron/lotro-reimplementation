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
pub mod combat_anim;
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
use tessera_render::{DrawDepthBias, Frame, MeshDraw, MeshId, Renderer, SkinPalette};
use tessera_world::WorldPos;
use tessera_world::draw_distance::{LodClasses, LodValues};

use self::catalog::WeenieRole;
use self::figure::{
    FigureAsset, FigureContext, FigureKey, FigureRequest, LookChoices, build_figure_with_look,
};
use self::motion::{Locomotion, drive, make_animator_with_keys};
use super::UploadBudget;
use super::geometry::{MaterialPlan, TexRequest};
use super::objects::{ObjectLayer, surface_draw};
use super::source::SourceShared;
use super::view::{Anchor, Frustum};

/// The entity ID of the character screens' figure ([`CharacterLayer::set_preview`]): the
/// highest ID, which no session gives (the sandbox counts up from small IDs; the test-server
/// backend's local IDs count up from 2⁶³).
pub const PREVIEW_ID: EntityId = EntityId(u64::MAX);

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
    /// The updated-avatar graphics option (`appearance.md` §3.3, default on): figure
    /// appearance elements with a toggle file use it. Kept in Tessera's settings
    /// (`[Graphics]`); read when the walker starts.
    pub updated_avatars: bool,
    /// The face-compositing graphics option (`appearance.md` §3.5, default on): with it
    /// off the species tables attach no face compositor. Kept in Tessera's settings
    /// (`[Graphics]`); read when the walker starts.
    pub face_compositing: bool,
}

impl Default for CharacterConfig {
    fn default() -> Self {
        Self {
            markers: false,
            draw_distance: 400.0,
            anim_distance: 200.0,
            max_entities: 1 << 16,
            updated_avatars: true,
            face_compositing: true,
        }
    }
}

/// `d0` of the character classes when the LOD-class table is missing (`walker.md` §14.5
/// OPEN: "80 m only as the fallback for `d0` when the table is missing"; §11.4.2:
/// `PlayerAvatar`, `Creature` and `Creature2Deg` all have 80 m in this install).
pub const CHARACTER_D0: f32 = 80.0;

/// Opacity above which a character casts the sun's shadow (`mesh.md` §8.8, row W-O1), as
/// objects do.
pub const SHADOW_OPACITY: f32 = 0.8;

/// What the character layer's distance rules need of a frame (`walker.md` §11.4).
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct CharacterFrame {
    /// The global limit *G* (`walker.md` §11.3).
    pub g: f32,
    /// The object draw-distance multiplier *M_d* (§11.2).
    pub multiplier: f32,
    /// The shadow-distance multiplier *M_s* (`mesh.md` §8.7).
    pub shadow_multiplier: f32,
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
            shadow_multiplier: 1.0,
            dt: 1.0 / 60.0,
            camera_moved: 0.0,
            lod: true,
        }
    }
}

/// A character's LOD values (`walker.md` §11.4, §11.4.3; `mesh.md` §8.5): its class's
/// entry of the table (`classes`), `d0` … `d5` × 3 for the player's own character
/// ([`LodClasses::values`]: a character without a class keeps the component's initial
/// values without the ×3). Without the table, the component's initial values with `d0` =
/// [`CHARACTER_D0`], × 3 for the player's own character.
#[must_use]
pub fn character_lod(classes: Option<&LodClasses>, class: Option<u32>, player: bool) -> LodValues {
    match classes {
        Some(t) => t.values(class, player),
        None => {
            let mut v = LodValues::default();
            v.0[0] = CHARACTER_D0;
            if player { v.for_player() } else { v }
        }
    }
}

/// A character's draw distance *D* = min(*G*, `d0` × *M_d*) of its LOD values `lod`
/// (`walker.md` §11.4 step 1; [`character_lod`]), capped at `cap`; the cap alone when
/// per-class distances are off. Non-finite inputs fall back to the cap.
#[must_use]
pub fn character_draw_distance(frame: &CharacterFrame, cap: f32, lod: &LodValues) -> f32 {
    if !frame.lod {
        return cap;
    }
    let d = (lod.d0() * frame.multiplier).min(frame.g);
    if d.is_finite() { d.min(cap) } else { cap }
}

/// A character's shadow distance min(*G*, `d2` × *M_d* × *M_s*) of its LOD values (`mesh.md`
/// §8.7, row W-O2), infinite while per-class distances are off. A product that is not a
/// number gives *G*.
#[must_use]
pub fn character_shadow_distance(frame: &CharacterFrame, lod: &LodValues) -> f32 {
    if !frame.lod {
        return f32::INFINITY;
    }
    let s = lod.0[2] * frame.multiplier * frame.shadow_multiplier;
    if s.is_nan() { frame.g } else { s.min(frame.g) }
}

/// Whether a character casts the sun's shadow (`mesh.md` §8.7–§8.8, rows W-O1, W-O2): while
/// its opacity is above [`SHADOW_OPACITY`] and its distance is strictly below its shadow
/// distance ([`character_shadow_distance`]), as objects do.
#[must_use]
pub fn character_casts(
    frame: &CharacterFrame,
    lod: &LodValues,
    opacity: f32,
    distance: f32,
) -> bool {
    opacity > SHADOW_OPACITY && distance < character_shadow_distance(frame, lod)
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
    /// Clips the scripts started ([`combat_anim`]).
    clips: combat_anim::ScriptClips,
}

/// Where a character stands, for the visibility test of `landblock.md` §4.11 ("Moving
/// entities"): the caller decides from it the cell the character belongs to and whether
/// the camera's portal walk reaches that cell's visibility cell.
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct CharacterSite {
    /// The placed figure's tag ([`super::figures::figure_tag`]), for a figure of the
    /// landblock data.
    pub tag: Option<u64>,
    /// The user's own character.
    pub player: bool,
    /// Its position (feet).
    pub position: WorldPos,
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
    /// Its fade was advanced in the current frame (cleared at the start of each draw pass).
    seen: bool,
    /// Its fade was advanced in the previous frame: otherwise the fade restarts (the first
    /// frame of its state; `walker.md` §14.5 row W-O3).
    seen_before: bool,
    /// The character screens' figure ([`CharacterLayer::set_preview`]): never drawn in
    /// the world, always animated, drawn alone into the screens' viewport.
    preview: bool,
    /// The session's classification (`EntityCreated`), for selection (`combat-ui.md` TR1).
    kind: Option<tessera_net::EntityKind>,
}

struct GpuPart {
    mesh: MeshId,
    plans: Vec<MaterialPlan>,
    skinned: bool,
    held: Option<u32>,
    bytes: u64,
}

/// What the particle effects of a placed figure need of it (`particle-runtime.md` §6):
/// its frame, render scale, whether it is drawn, its current model-space pose and its
/// attachment points.
#[derive(Debug, Clone)]
pub struct FigureEffectView {
    /// Region.
    pub region: u8,
    /// Region-global position.
    pub position: [f64; 3],
    /// Rotation.
    pub rotation: Quat,
    /// Render scale per axis (model scale × placement scale, as drawn; `appearance.md`
    /// §6.1).
    pub scale: Vec3,
    /// Drawn by this layer at the last update (`shown_figures`, `walker.md` §10.2).
    pub drawn: bool,
    /// Model-space bone matrices of the current pose (the sampled pose, else the reference
    /// pose; empty without a skeleton).
    pub model: Vec<Mat4>,
    /// The setup's holding locations and bones.
    pub points: Option<Arc<crate::walker::effects::data::AttachPoints>>,
    /// The figure's skeleton (clips that scripts start are bound to it).
    pub skeleton: Option<Arc<tessera_anim::Skeleton>>,
    /// The figure's script tables and stored values (`None` when not resolved).
    pub scripts: Option<Arc<figure::FigureScripts>>,
}

/// Margin around a figure's joints for its picking box and anchor, metres (Tessera choice:
/// the joints lie inside the skin).
pub const BOX_MARGIN: f32 = 0.15;
/// Half-width and height of the box of a figure without a skeleton, metres (Tessera choice).
pub const DEFAULT_BOX: [f32; 2] = [0.35, 1.8];
/// Most script clips tracked at once (hostile-input bound).
pub const MAX_SCRIPT_CLIPS: usize = 4096;

/// A figure's box in region-global coordinates ([`CharacterLayer::entity_box`]).
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct EntityBox {
    /// Region.
    pub region: u8,
    /// Lower corner.
    pub min: [f64; 3],
    /// Upper corner.
    pub max: [f64; 3],
    /// Whether this layer draws the figure.
    pub drawn: bool,
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
    /// The look choices built with the request.
    look: LookChoices,
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
    /// The LOD-class table (`walker.md` §11.4.3; `None`: missing).
    lod_classes: Option<Arc<LodClasses>>,
    /// The install's creation controls per player avatar: the receiver rule of received
    /// looks and their translation into figure choices (decision 0025 D2).
    looks: Option<Arc<crate::screens::ScreenData>>,
    /// The last appearance with a look of each entity whose figure is a playable body,
    /// applied again when [`Self::looks`] changes.
    look_appearances: HashMap<Entity, Appearance>,
    /// [`Self::looks`] changed: the look appearances are applied again at the next events.
    looks_changed: bool,
    /// Look entries ignored by the receiver rule (for the rate-limited log).
    look_ignored: u64,
    /// The figure and ramp-out of each script clip playing ([`combat_anim`]).
    clip_owners: HashMap<u64, (EntityId, f32)>,
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

/// How far the figure of an entity has come ([`CharacterLayer::figure_of`]).
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum FigureState {
    /// Queued or building on the workers.
    Pending,
    /// Built: its parts (held parts included) and how many of them are held items.
    Built {
        /// Parts of the figure.
        parts: usize,
        /// Parts that are held items (`appearance.md` §8 step 9).
        held: usize,
    },
    /// The build failed.
    Failed,
}

/// The figure an entity has: what was requested and how far it has come.
#[derive(Debug, Clone, PartialEq)]
pub struct FigureStatus {
    /// The request (body, worn and held items with their slots).
    pub request: FigureRequest,
    /// The look choices of a received look or of the creation screen (styles, slider
    /// string, appearance values).
    pub look: LookChoices,
    /// Its state.
    pub state: FigureState,
    /// What the species tables' look gave the built figure (`None` before it is built).
    pub face: Option<figure::FaceSummary>,
}

impl CharacterLayer {
    /// The figure entity `id` has (tests and tools; `None` without one).
    #[must_use]
    pub fn figure_of(&self, id: EntityId) -> Option<FigureStatus> {
        let e = self.store.entity_for(NetId(id.0))?;
        let slot = self.actors.get(e)?.slot?;
        let s = self.slots.get(slot as usize)?;
        let count = |a: &FigureAsset| FigureState::Built {
            parts: a.parts.len(),
            held: a.parts.iter().filter(|p| p.held.is_some()).count(),
        };
        let state = match &s.state {
            SlotState::Building(_) | SlotState::Queued => FigureState::Pending,
            SlotState::Waiting(a, _) => count(a),
            SlotState::Ready(g) => count(&g.asset),
            SlotState::Failed | SlotState::Free => FigureState::Failed,
        };
        let face = match &s.state {
            SlotState::Waiting(a, _) => Some(a.face.clone()),
            SlotState::Ready(g) => Some(g.asset.face.clone()),
            _ => None,
        };
        Some(FigureStatus {
            request: s.request.clone()?,
            look: s.look.clone(),
            state,
            face,
        })
    }

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
            lod_classes: None,
            looks: None,
            look_appearances: HashMap::new(),
            looks_changed: false,
            look_ignored: 0,
            clip_owners: HashMap::new(),
        }
    }

    /// The install's creation controls the received looks are read with (decision 0025
    /// D2); figures already shown with a look are requested again when they change.
    pub fn set_look_data(&mut self, data: Option<Arc<crate::screens::ScreenData>>) {
        let same = match (&self.looks, &data) {
            (Some(a), Some(b)) => Arc::ptr_eq(a, b),
            (None, None) => true,
            _ => false,
        };
        if same {
            return;
        }
        self.looks = data;
        self.looks_changed = !self.look_appearances.is_empty();
    }

    /// The figure choices of `appearance`'s look (decision 0025 D2): its entries filtered
    /// by the receiver rule (a property a creation control of the body's player avatar
    /// writes in this install, with a matching value kind), then turned into heights,
    /// styles, the slider string and the appearance values (colours, face layers, …) as
    /// the creation screen's controls turn them, so the figure composes its face and tints
    /// from them (`appearance.md` §3.5).
    fn look_choices(&mut self, appearance: &Appearance) -> (Option<f32>, Option<f32>, LookChoices) {
        let (Some(look), Some(data), Body::Character { species, sex }) =
            (&appearance.look, &self.looks, appearance.body)
        else {
            return (None, None, LookChoices::default());
        };
        let Some(detail) = data.avatar(species, sex) else {
            return (None, None, LookChoices::default());
        };
        let (accepted, ignored) = crate::screens::controls::accept_look(look, detail);
        if ignored > 0 {
            let before = self.look_ignored;
            self.look_ignored = self.look_ignored.saturating_add(ignored as u64);
            // Rate-limited: once per power of two of the running count.
            if (before + 1..=self.look_ignored).any(u64::is_power_of_two) {
                tracing::debug!(
                    ignored,
                    total = self.look_ignored,
                    "look entries ignored by the receiver rule"
                );
            }
        }
        let mut choices = crate::screens::AppearanceChoices::default();
        choices.apply_entries(detail, &accepted);
        let spec = choices.character_spec(detail, &data.pids, species, sex);
        (
            spec.avatar_height,
            spec.bone_scale,
            LookChoices {
                styles: spec.styles,
                slider_values: spec.slider_values,
                appearance_values: spec.appearance_values,
            },
        )
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

    /// The placed figure with `tag` as the particle effects see it, once its figure is on
    /// the GPU (`None` before, or when no such figure is placed).
    #[must_use]
    pub fn figure_effect_view(&self, tag: u64) -> Option<FigureEffectView> {
        let (e, a) = self.actors.iter().find(|(_, a)| a.tag == Some(tag))?;
        let mut v = self.effect_view(e, a)?;
        v.drawn = !a.hidden && self.shown.contains(&tag);
        Some(v)
    }

    /// The figure of the session entity `id` (a scenario actor, a test-server actor, the
    /// player) as the particle effects and combat scripts see it, once it is on the GPU.
    /// Its `drawn` is whether this layer draws it (not hidden, on the GPU, posed when
    /// animated).
    #[must_use]
    pub fn entity_effect_view(&self, id: EntityId) -> Option<FigureEffectView> {
        let e = self.store.entity_for(NetId(id.0))?;
        let a = self.actors.get(e)?;
        if a.preview {
            return None;
        }
        let mut v = self.effect_view(e, a)?;
        v.drawn = !a.hidden && self.drawable(e);
        Some(v)
    }

    /// The figure context (decoded clips, shared with the figure jobs); `None` in tests
    /// without one.
    #[must_use]
    pub fn figure_context(&self) -> Option<Arc<FigureContext>> {
        self.ctx.clone()
    }

    /// The session's classification of entity `id` (from `EntityCreated`).
    #[must_use]
    pub fn entity_kind(&self, id: EntityId) -> Option<tessera_net::EntityKind> {
        let e = self.store.entity_for(NetId(id.0))?;
        self.actors.get(e)?.kind
    }

    /// The session entities (not placed by the landblock data, not the screens' figure).
    #[must_use]
    pub fn session_entities(&self) -> Vec<EntityId> {
        self.actors
            .iter()
            .filter(|(_, a)| a.tag.is_none() && !a.preview)
            .map(|(_, a)| a.net)
            .collect()
    }

    /// Whether entity `id` is moving (its locomotion layer is in, else its ground speed is
    /// above the start speed).
    #[must_use]
    pub fn entity_moving(&self, id: EntityId) -> Option<bool> {
        let e = self.store.entity_for(NetId(id.0))?;
        if let Some(anim) = self.store.anim(e) {
            return Some(anim.loco.moving);
        }
        Some(self.actors.get(e)?.speed > motion::START_SPEED)
    }

    /// The region and the region-global axis-aligned box of entity `id`'s figure: the
    /// joints of its current pose (else of its reference pose) with [`BOX_MARGIN`] around
    /// them, or a standing [`DEFAULT_BOX`] for a figure without a skeleton. `None` before
    /// its figure is on the GPU.
    #[must_use]
    pub fn entity_box(&self, id: EntityId) -> Option<EntityBox> {
        let e = self.store.entity_for(NetId(id.0))?;
        let a = self.actors.get(e)?;
        let v = self.effect_view(e, a)?;
        let tf = Mat4::from_scale_rotation_translation(v.scale, v.rotation, Vec3::ZERO);
        let mut lo = Vec3::splat(f32::INFINITY);
        let mut hi = Vec3::splat(f32::NEG_INFINITY);
        for m in &v.model {
            let p = tf.transform_point3(m.w_axis.truncate());
            if p.is_finite() {
                lo = lo.min(p);
                hi = hi.max(p);
            }
        }
        if !(lo.is_finite() && hi.is_finite()) {
            let [w, h] = DEFAULT_BOX;
            lo = Vec3::new(-w, -w, 0.0) * v.scale;
            hi = Vec3::new(w, w, h) * v.scale;
        }
        lo -= Vec3::splat(BOX_MARGIN);
        hi += Vec3::splat(BOX_MARGIN);
        let at = |p: Vec3| {
            [
                v.position[0] + f64::from(p.x),
                v.position[1] + f64::from(p.y),
                v.position[2] + f64::from(p.z),
            ]
        };
        Some(EntityBox {
            region: v.region,
            min: at(lo),
            max: at(hi),
            drawn: !a.hidden && self.drawable(e),
        })
    }

    /// Applies the clips the scripts started or stopped ([`combat_anim`]): a figure with a
    /// skeleton but no animator gets one for them.
    pub fn apply_clip_commands(&mut self, commands: Vec<combat_anim::ClipCommand>) {
        for c in commands {
            match c {
                combat_anim::ClipCommand::Play { entity, request } => {
                    let Some(e) = self.store.entity_for(NetId(entity.0)) else {
                        continue;
                    };
                    if self.store.anim(e).is_none() {
                        self.add_bare_animator(e);
                    }
                    if self.clip_owners.len() >= MAX_SCRIPT_CLIPS {
                        // Forget the clips whose layer is gone (one-shot clips end on their
                        // own); with no room left the new clip is refused, so that the stops
                        // of live clips are never lost.
                        let store = &self.store;
                        self.clip_owners.retain(|h, (owner, _)| {
                            store
                                .entity_for(NetId(owner.0))
                                .and_then(|e| store.anim(e))
                                .is_some_and(|a| a.clips.holds(*h))
                        });
                        if self.clip_owners.len() >= MAX_SCRIPT_CLIPS {
                            continue;
                        }
                    }
                    let Some(anim) = self.store.anim_mut(e) else {
                        continue;
                    };
                    let CharAnim {
                        animator,
                        loco,
                        clips,
                        ..
                    } = anim;
                    if clips.play(animator, loco, &request) {
                        self.clip_owners
                            .insert(request.handle, (entity, request.node.ramp_out));
                    }
                }
                combat_anim::ClipCommand::Update { handle, node } => {
                    let Some(&(entity, _)) = self.clip_owners.get(&handle) else {
                        continue;
                    };
                    if let Some(e) = self.store.entity_for(NetId(entity.0))
                        && let Some(anim) = self.store.anim_mut(e)
                    {
                        anim.clips.update(&mut anim.animator, handle, &node);
                    }
                    if let Some(o) = self.clip_owners.get_mut(&handle) {
                        o.1 = node.ramp_out;
                    }
                }
                combat_anim::ClipCommand::Stop { handle } => {
                    let Some((entity, ramp_out)) = self.clip_owners.remove(&handle) else {
                        continue;
                    };
                    let Some(e) = self.store.entity_for(NetId(entity.0)) else {
                        continue;
                    };
                    if let Some(anim) = self.store.anim_mut(e) {
                        let CharAnim {
                            animator,
                            loco,
                            clips,
                            ..
                        } = anim;
                        clips.stop(animator, loco, handle, ramp_out);
                    }
                }
            }
        }
    }

    /// An empty animator (reference pose) for a figure that has a skeleton but no clips,
    /// so that script clips can play on it.
    fn add_bare_animator(&mut self, e: Entity) {
        let Some(render) = self.store.render(e) else {
            return;
        };
        let Some(SlotState::Ready(g)) = self.slots.get(render.slot as usize).map(|s| &s.state)
        else {
            return;
        };
        let Some(skel) = g.asset.skeleton.clone() else {
            return;
        };
        let attach = g.asset.attach.clone();
        let Ok(instance) = self.scheduler.add(Arc::clone(&skel), attach) else {
            return;
        };
        let anim = CharAnim {
            instance,
            animator: Animator::new(skel),
            loco: Locomotion {
                walk_speed: 0.0,
                run_speed: 0.0,
                base_rate: 1.0,
                moving: false,
                has_move: false,
                move_index: 0,
                above: Vec::new(),
            },
            clips: combat_anim::ScriptClips::default(),
        };
        if let Err(r) = self.store.set_anim(e, anim) {
            self.scheduler.remove(r.value.instance);
        }
        self.apply_changes();
    }

    fn effect_view(&self, e: Entity, a: &Actor) -> Option<FigureEffectView> {
        let render = self.store.render(e)?;
        let Some(SlotState::Ready(g)) = self.slots.get(render.slot as usize).map(|s| &s.state)
        else {
            return None;
        };
        let tf = self.store.transform(e)?;
        let scale = Vec3::from_array(g.asset.model_scale) * tf.scale[0];
        let rot = Quat::from_array(tf.rotation);
        let model = match self.store.anim(e) {
            Some(anim) => self
                .scheduler
                .pose(anim.instance)
                .map(|p| p.model.clone())
                .unwrap_or_default(),
            None => g
                .asset
                .skeleton
                .as_ref()
                .map(|s| s.reference_model().to_vec())
                .unwrap_or_default(),
        };
        Some(FigureEffectView {
            region: a.region,
            position: tf.position,
            rotation: if rot.is_finite() && rot.length_squared() > 0.0 {
                rot.normalize()
            } else {
                Quat::IDENTITY
            },
            scale: if scale.is_finite() && scale.min_element() > 0.0 {
                scale
            } else {
                Vec3::ONE
            },
            drawn: false,
            model,
            points: g.asset.points.clone(),
            skeleton: g.asset.skeleton.clone(),
            scripts: g.asset.scripts.clone(),
        })
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
                let lod = character_lod(self.lod_classes.as_deref(), self.class_of(a.slot), false);
                let max_d = f64::from(character_draw_distance(
                    &self.distances,
                    self.config.draw_distance,
                    &lod,
                ));
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
    fn acquire(&mut self, req: FigureRequest, look: LookChoices) -> u32 {
        let key = req.key_with(&look);
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
                    look: LookChoices::default(),
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
        s.look = look;
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
            let look = s.look.clone();
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
                        build_figure_with_look(&ctx, &req, &look, &|| job.is_cancelled())
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
            seen_before: false,
            preview: false,
            kind: None,
        };
        if self.actors.insert(e, actor).is_err() {
            self.store.despawn(e);
            return None;
        }
        Some(e)
    }

    fn despawn(&mut self, e: Entity) {
        self.look_appearances.remove(&e);
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
        if appearance.look.is_some() && matches!(appearance.body, Body::Character { .. }) {
            self.look_appearances.insert(e, appearance.clone());
        } else {
            self.look_appearances.remove(&e);
        }
        let (avatar_height, bone_scale, look) = self.look_choices(appearance);
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
            avatar_height,
            bone_scale,
            // `appearance.md` §7.4 Decision: a placed figure's script picks are seeded
            // from its instance ID.
            seed: info.as_ref().and_then(|i| i.iid),
            chargen_stage: None,
        };
        let slot = (!hidden && matches!(req.body, Body::Character { .. } | Body::Object(_)))
            .then(|| self.acquire(req, look));
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
        if std::mem::take(&mut self.looks_changed) {
            // The look data arrived (or changed): figures with a look are requested again.
            let pending: Vec<(Entity, Appearance)> = self
                .look_appearances
                .iter()
                .map(|(e, a)| (*e, a.clone()))
                .collect();
            for (e, a) in pending {
                self.set_appearance(e, &a, shared);
            }
        }
        for ev in events.drain(..) {
            match ev {
                Event::EntityCreated {
                    id,
                    kind,
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
                        if let Some(a) = self.actors.get_mut(e) {
                            a.kind = Some(kind);
                        }
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
            // one job for its first pose wherever it is (it is not drawn without one). The
            // screens' figure is always animated.
            let near = a.preview
                || eye.is_none_or(|eye| {
                    let dx = tf.position[0] - eye.x;
                    let dy = tf.position[1] - eye.y;
                    eye.region == a.region && dx.hypot(dy) <= far
                });
            if !near && scheduler.has_pose(anim.instance) {
                continue;
            }
            drive(&mut anim.animator, &mut anim.loco, a.speed, dt);
            anim.clips.tick(&mut anim.animator, &mut anim.loco, dt);
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
                if let Some((animator, loco, keys)) =
                    make_animator_with_keys(skel, &asset.clips, phase)
                {
                    match self.scheduler.add(Arc::clone(skel), asset.attach.clone()) {
                        Ok(instance) => {
                            if let Err(r) = self.store.set_anim(
                                e,
                                CharAnim {
                                    instance,
                                    animator,
                                    loco,
                                    clips: combat_anim::ScriptClips::new(keys),
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

    /// Sets the LOD-class table the characters' distances come from (`walker.md` §11.4.3;
    /// `None`: the table is missing, [`character_lod`]).
    pub fn set_lod_classes(&mut self, classes: Option<Arc<LodClasses>>) {
        self.lod_classes = classes;
    }

    /// The LOD class of the figure in slot `slot` once it is built (`None` before, or
    /// without a class).
    fn class_of(&self, slot: Option<u32>) -> Option<u32> {
        match slot
            .and_then(|s| self.slots.get(s as usize))
            .map(|s| &s.state)
        {
            Some(SlotState::Ready(g)) => g.asset.lod_class,
            _ => None,
        }
    }

    /// Uploads the palettes of the visible characters into `frame` and returns their draws
    /// (module docs, step 4). `camera` is the eye in `anchor`'s frame; `visible` says
    /// whether a character is in a visibility cell the camera sees (`landblock.md` §4.11
    /// "Moving entities"; tested after its fade advanced).
    #[allow(clippy::too_many_arguments)]
    pub fn prepare_draws(
        &mut self,
        frame: &mut Frame,
        anchor: &Anchor,
        camera: Vec3,
        frustum: &Frustum,
        objects: &ObjectLayer,
        visible: &dyn Fn(&CharacterSite) -> bool,
        out: &mut Vec<MeshDraw>,
    ) {
        let t = Instant::now();
        let cap = self.config.draw_distance;
        let df = self.distances;
        let mut drawn = 0usize;
        // A fade not advanced in the previous frame restarts (`walker.md` §14.5 W-O3).
        for (_, a) in self.actors.iter_mut() {
            a.seen_before = a.seen;
            a.seen = false;
        }
        for (e, render, tf) in self.store.renderables() {
            let Some(a) = self.actors.get(e) else {
                continue;
            };
            let is_player = Some(a.net) == self.player;
            if a.hidden || a.preview {
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
            let scale = Vec3::from_array(g.asset.model_scale) * tf.scale[0];
            let scale = if scale.is_finite() && scale.min_element() > 0.0 {
                scale
            } else {
                Vec3::ONE
            };
            let rot = Quat::from_array(tf.rotation);
            let rot = if rot.is_finite() && rot.length_squared() > 0.0 {
                rot.normalize()
            } else {
                Quat::IDENTITY
            };
            let world = Mat4::from_scale_rotation_translation(scale, rot, pos);
            let center = world.transform_point3(g.center);
            let radius = g.radius * scale.max_element();
            let distance = ((center - camera).length() - radius).max(0.0);
            let lod = character_lod(self.lod_classes.as_deref(), g.asset.lod_class, is_player);
            let max_d = character_draw_distance(&df, cap, &lod);
            let in_view = frustum.sphere_visible(center, radius);
            let Some(a) = self.actors.get_mut(e) else {
                continue;
            };
            // `walker.md` §11.4 step 1: the 1.5 s fade across D (`mesh.md` §8.7), advanced
            // for every character of the scene before the view test (§14.5 W-O3).
            let opacity = a.fade.update(
                distance,
                max_d,
                df.dt,
                df.camera_moved,
                !a.seen_before,
                df.lod,
            );
            a.seen = true;
            if opacity <= 0.0 || !in_view {
                continue;
            }
            // `landblock.md` §4.11 "Moving entities": culled by the visibility cell of its
            // cell like any drawn object, after its fade advanced (W-O3).
            let site = CharacterSite {
                tag: a.tag,
                player: is_player,
                position: WorldPos::new(a.region, tf.position[0], tf.position[1], tf.position[2]),
            };
            if !visible(&site) {
                continue;
            }
            // `mesh.md` §8.7–§8.8: opacity above 0.8, strictly within the class's shadow
            // distance (rows W-O1, W-O2).
            let casts = self.shadows && character_casts(&df, &lod, opacity, distance);
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
            // A placed figure's `Render_DepthBias_Type` entry on every draw of it
            // (`decals.md` §6.5 OPEN (c), Tessera default); other characters have none.
            let depth_bias = a
                .tag
                .map_or(DrawDepthBias::NONE, |tag| objects.figure_depth_bias(tag));
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
                        .with_lit(lit)
                        .with_depth_bias(depth_bias);
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

    /// The player's entity, once known.
    #[must_use]
    pub fn player_id(&self) -> Option<EntityId> {
        self.player
    }

    /// Shows `job`'s figure as the character screens' figure (`character-screens.md` §2.1,
    /// §6: the viewport follows the selection at once), or removes it (`None`). The figure
    /// is the entity [`PREVIEW_ID`]: built on the asset workers like every other figure,
    /// always animated, never drawn in the world; [`CharacterLayer::portrait_draws`] with
    /// [`PREVIEW_ID`] draws it alone. A new request keeps the previous figure drawn until
    /// the new one is on the GPU; an unchanged one costs a key comparison.
    pub fn set_preview(&mut self, job: Option<(FigureRequest, LookChoices)>, now: Instant) {
        let existing = self.store.entity_for(NetId(PREVIEW_ID.0));
        let Some((req, look)) = job else {
            if let Some(e) = existing {
                self.despawn(e);
                self.apply_changes();
            }
            return;
        };
        let e = match existing {
            Some(e) => e,
            None => {
                let Some(e) = self.spawn(
                    PREVIEW_ID,
                    0,
                    [0.0; 3],
                    std::f32::consts::FRAC_PI_2,
                    None,
                    now,
                ) else {
                    return;
                };
                if let Some(a) = self.actors.get_mut(e) {
                    a.snap = false;
                    a.preview = true;
                }
                e
            }
        };
        let key = req.key_with(&look);
        let Some(old) = self.actors.get(e).map(|a| a.slot) else {
            return;
        };
        let same = old
            .and_then(|s| self.slots.get(s as usize))
            .and_then(|s| s.key.as_ref())
            .is_some_and(|k| *k == key);
        if same {
            return;
        }
        let slot = self.acquire(req, look);
        if let Some(old) = old {
            self.release(old);
        }
        if let Some(a) = self.actors.get_mut(e) {
            a.slot = Some(slot);
        }
    }

    /// The figure the character screens show ([`CharacterLayer::set_preview`]): what was
    /// requested and how far it has come.
    #[must_use]
    pub fn preview(&self) -> Option<FigureStatus> {
        self.figure_of(PREVIEW_ID)
    }

    /// The draws of entity `id`'s figure alone, as a portrait (the paperdoll viewport,
    /// `character-panels.md` §3.2, `render.md` §6.10): at the origin, turned by `yaw`
    /// radians about the vertical axis, with its current pose, unlit and opaque. Returns
    /// the figure's bounding sphere (centre, radius) in that frame; `None` while the figure
    /// is not on the GPU.
    pub fn portrait_draws(
        &self,
        frame: &mut Frame,
        id: EntityId,
        yaw: f32,
        objects: &ObjectLayer,
        out: &mut Vec<MeshDraw>,
    ) -> Option<(Vec3, f32)> {
        let e = self.store.entity_for(NetId(id.0))?;
        let render = self.store.render(e)?;
        let Some(SlotState::Ready(g)) = self.slots.get(render.slot as usize).map(|s| &s.state)
        else {
            return None;
        };
        let scale = Vec3::from_array(g.asset.model_scale);
        let scale = if scale.is_finite() && scale.min_element() > 0.0 {
            scale
        } else {
            Vec3::ONE
        };
        let yaw = if yaw.is_finite() { yaw } else { 0.0 };
        let world =
            Mat4::from_scale_rotation_translation(scale, Quat::from_rotation_z(yaw), Vec3::ZERO);
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
                    surface_draw(plan, &|t| objects.texture(t), false, 1.0, false)
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
        let centre = world.transform_point3(g.center);
        Some((centre, g.radius * scale.max_element()))
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

    /// Look data for one playable body (species 0x0A01, male): a head style (property
    /// 0x100, int; variants 0, 0.01, 0.02) and a body scale (0x102, float).
    fn look_data() -> Arc<crate::screens::ScreenData> {
        use crate::screens::controls::{
            AvatarDetail, ControlChoices, ControlDetail, ControlValue, ScreenPids,
        };
        use tessera_formats::Did;
        use tessera_gamedata::chargen::{
            AppearanceControl, ChargenData, ControlKind, GenderEntry, RaceChoice, RaceRecord,
        };
        use tessera_net::PropertyKind;
        let avatar = Did(0x7000_0A01);
        let record = RaceRecord {
            race: 0x0A01,
            genders: vec![GenderEntry {
                gender: tessera_gamedata::GENDER_MALE,
                player_avatar: Some(avatar),
                ..GenderEntry::default()
            }],
            ..RaceRecord::default()
        };
        let chargen = ChargenData {
            races: vec![RaceChoice::from_record(Did(0x7000_00A0), record)],
            ..ChargenData::default()
        };
        let control = |index, kind| AppearanceControl {
            index,
            availability: 1,
            group: None,
            kind,
        };
        let detail = AvatarDetail {
            entity: Some(Did(0x4700_1000)),
            controls: vec![
                ControlDetail {
                    control: control(
                        0,
                        ControlKind::Style {
                            file: None,
                            key: Some(0x1000_0009),
                            script_property: Some(0x100),
                        },
                    ),
                    writes: Some((0x100, PropertyKind::Int)),
                    choices: ControlChoices::Styles {
                        key: Some(0x1000_0009),
                        variants: vec![0.0, 0.01, 0.02],
                    },
                    default: Some(ControlValue::Style(0)),
                },
                ControlDetail {
                    control: control(
                        1,
                        ControlKind::BodyScale {
                            min: Some(0.0),
                            max: Some(1.0),
                        },
                    ),
                    writes: Some((0x102, PropertyKind::Float)),
                    choices: ControlChoices::Range { min: 0.0, max: 1.0 },
                    default: Some(ControlValue::Float(0.5)),
                },
            ],
            notes: Vec::new(),
        };
        Arc::new(crate::screens::ScreenData {
            chargen,
            names: None,
            avatars: std::iter::once((avatar, detail)).collect(),
            pids: ScreenPids {
                bone_scale: Some(0x102),
                ..ScreenPids::default()
            },
            stage_keys: crate::screens::data::StageKeys::default(),
        })
    }

    /// `character-screens.md` §2.1, §6: the screens' figure is one entity of its own; an
    /// unchanged request keeps its figure, a changed one takes a new figure and releases
    /// the old, `None` removes it; it never enters the world's draws.
    #[test]
    fn the_screens_figure_follows_its_request() {
        let mut l = layer();
        let req = |stage: u32, height: Option<f32>| FigureRequest {
            body: Body::Character {
                species: 0x0A01,
                sex: tessera_gamedata::GENDER_MALE,
            },
            equipment: Vec::new(),
            instance: None,
            instance_key: None,
            static_only: false,
            avatar_height: height,
            bone_scale: None,
            seed: None,
            chargen_stage: Some(stage),
        };
        let look = LookChoices {
            appearance_values: vec![(
                0x101,
                tessera_formats::PropertyValue::Colour([1, 2, 3, 255]),
            )],
            ..LookChoices::default()
        };
        assert!(l.preview().is_none());
        l.set_preview(Some((req(6, None), look.clone())), Instant::now());
        let p = l.preview().expect("the screens' figure");
        assert_eq!(p.request.chargen_stage, Some(6));
        assert_eq!(p.look, look);
        assert_eq!(refs(&l), vec![1]);
        // The same request again: the same figure, no new reference.
        l.set_preview(Some((req(6, None), look.clone())), Instant::now());
        assert_eq!(refs(&l), vec![1]);
        // Another choice: a new figure; the old one is released.
        l.set_preview(Some((req(6, Some(0.5)), look.clone())), Instant::now());
        assert_eq!(refs(&l), vec![0, 1]);
        assert_eq!(l.preview().unwrap().request.avatar_height, Some(0.5));
        // Never a world figure: no tag, not a session entity, not the player.
        assert!(l.placed().next().is_none());
        assert!(l.player_id().is_none());
        l.set_preview(None, Instant::now());
        assert!(l.preview().is_none());
        assert!(l.store().is_empty());
    }

    #[test]
    fn received_looks_reach_the_figure_under_the_receiver_rule() {
        use tessera_net::{Look, LookEntry, PropertyValue};
        let mut l = layer();
        let look = Look {
            entries: vec![
                LookEntry {
                    property: 0x100,
                    value: PropertyValue::Int(2),
                },
                // Not written by any control here: ignored.
                LookEntry {
                    property: 0x101,
                    value: PropertyValue::Int(1),
                },
                LookEntry {
                    property: 0x102,
                    value: PropertyValue::Float(0.75),
                },
                // Not written either (a hostile peer's string): ignored.
                LookEntry {
                    property: 0x103,
                    value: PropertyValue::String("x".into()),
                },
            ],
        };
        let appearance = Appearance::new(Body::Character {
            species: 0x0A01,
            sex: tessera_gamedata::GENDER_MALE,
        })
        .with_look(Some(look));
        let event = |id: u64| Event::EntityCreated {
            id: EntityId(id),
            kind: EntityKind::Player,
            name: String::new(),
            class: None,
            location: Location::new(RegionId(1), 100.0, 200.0, 5.0),
            heading: 0.0,
            appearance: Box::new(appearance.clone()),
            tag: None,
        };
        // Before the look data: the race's default figure.
        apply(&mut l, vec![event(5)], None);
        let f = l.figure_of(EntityId(5)).expect("figure");
        assert!(f.look.is_empty());
        assert_eq!(f.request.bone_scale, None);
        // The data arrives: the figure is requested again with the look.
        l.set_look_data(Some(look_data()));
        apply(&mut l, Vec::new(), None);
        let f = l.figure_of(EntityId(5)).expect("figure");
        assert_eq!(
            f.look.styles,
            vec![tessera_mesh::assembly::StyleChoice {
                key: 0x1000_0009,
                modifier: 0.02
            }]
        );
        assert_eq!(f.request.bone_scale, Some(0.75));
        assert_eq!(f.request.avatar_height, None);
        // Every accepted value reaches the figure's script state (colours and face layers
        // the same way), so the species tables compose and tint with it.
        assert_eq!(
            f.look.appearance_values,
            vec![
                (0x100, tessera_formats::PropertyValue::Int(2)),
                (0x102, tessera_formats::PropertyValue::Float(0.75)),
            ]
        );
        assert_eq!(f.request.chargen_stage, None, "a figure in the world");
        // A second entity with the same look shares the figure.
        apply(&mut l, vec![event(6)], None);
        let a = l.figure_of(EntityId(6)).expect("figure");
        assert_eq!(a.look, f.look);
        assert_eq!(refs(&l).iter().filter(|r| **r > 0).count(), 1);
        // Two ignored entries per look applied with the data (entities 5 and 6).
        assert_eq!(l.look_ignored, 4);
    }

    #[test]
    fn character_distances_follow_the_class_g_and_the_multiplier() {
        let f = CharacterFrame::default();
        // Without the table: d0 80 (× 3 for the player's own character).
        let npc = character_lod(None, None, false);
        let player = character_lod(None, None, true);
        assert_eq!(character_draw_distance(&f, 400.0, &npc), 80.0);
        assert_eq!(character_draw_distance(&f, 400.0, &player), 240.0);
        // `--object-draw-distance UltraHigh` (M_d 2): 160 m, not the old fixed 80 m.
        let ultra = CharacterFrame {
            multiplier: 2.0,
            ..f
        };
        assert_eq!(character_draw_distance(&ultra, 400.0, &npc), 160.0);
        // Fog pulls G in: G wins.
        let fog = CharacterFrame { g: 60.0, ..f };
        assert_eq!(character_draw_distance(&fog, 400.0, &npc), 60.0);
        // The option is a cap.
        assert_eq!(character_draw_distance(&ultra, 50.0, &npc), 50.0);
        // Per-class distances off: the cap alone; hostile values: the cap.
        let off = CharacterFrame { lod: false, ..f };
        assert_eq!(character_draw_distance(&off, 400.0, &npc), 400.0);
        let nan = CharacterFrame {
            multiplier: f32::NAN,
            ..f
        };
        assert_eq!(character_draw_distance(&nan, 400.0, &npc), 400.0);
    }

    /// `walker.md` §14.5 (rows W-O1, W-O2, W-K1): a character's shadow distance is its
    /// class's min(G, d2 × M_d × M_s), × 3 for the player's own; it casts while its opacity
    /// is above 0.8 and its distance strictly below that.
    #[test]
    fn character_shadows_follow_the_class() {
        let f = CharacterFrame::default();
        // A class with d2 = 26 (the three character classes of this install, W-K1).
        let mut class = LodValues::default();
        class.0[0] = 80.0;
        class.0[2] = 26.0;
        assert_eq!(character_shadow_distance(&f, &class), 26.0);
        assert_eq!(character_shadow_distance(&f, &class.for_player()), 78.0);
        assert!(character_casts(&f, &class, 0.81, 25.9));
        assert!(!character_casts(&f, &class, 0.8, 10.0), "opacity above 0.8");
        assert!(!character_casts(&f, &class, 1.0, 26.0), "strictly below");
        // M_s and G.
        let half = CharacterFrame {
            shadow_multiplier: 0.5,
            ..f
        };
        assert_eq!(character_shadow_distance(&half, &class), 13.0);
        let fog = CharacterFrame { g: 20.0, ..f };
        assert_eq!(character_shadow_distance(&fog, &class), 20.0);
        // Per-class distances off: at any distance.
        let off = CharacterFrame { lod: false, ..f };
        assert!(character_casts(&off, &class, 1.0, 1.0e6));
        // No class: the component's initial d2 (16), scaled and capped all the same.
        let none = character_lod(None, None, false);
        assert_eq!(character_shadow_distance(&f, &none), 16.0);
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
