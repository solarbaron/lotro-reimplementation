// SPDX-License-Identifier: GPL-3.0-or-later
//! Particle effects in the walker (`docs/specs/particle-runtime.md`, the parts it assigns
//! to the client; `docs/specs/particles.md` §8 through `tessera-particles`).
//!
//! | Part | Where | Spec |
//! |---|---|---|
//! | Owners whose start-up scripts reach Particle nodes, records, surfaces, attachment points | [`data`] | runtime §5, §6.1, §6.4, §9.1 |
//! | Particle nodes started by script tables: start gates, the never-ending instance, its stop hook and the redirect listener | `tessera-scriptgraph` | runtime §5.4–§5.8, §7.1 |
//! | The attached and dropped paths, start gating, attachment frames, owner scale vectors, water probe | [`EffectLayer`] (its sink) | runtime §5.5–§5.7, §6, `particles.md` §7.1.2 |
//! | Stop, linger, removal; the per-frame order | `tessera-particles` [`EffectManager`], [`EffectLayer::update`] | runtime §3.2, §7 |
//! | Drawing: draw order, surface texture and blend state, distance fade | [`EffectLayer::draw`] | runtime §9, `particles.md` §8.13, §8.15 |
//!
//! # Per frame ([`EffectLayer::update`], the Decision of Q-RT1)
//!
//! 1. Landblocks near the viewer are scanned on the workers for effect owners: static
//!    entities and placed figures whose start-up scripts reach Particle nodes. An owner is
//!    spawned in the layer's own script runtime when the viewer comes within its reach (the
//!    largest far-fade end of the effects it can start, plus [`SPAWN_MARGIN`]) and
//!    despawned beyond its reach plus [`DESPAWN_MARGIN`] or when its landblock leaves; a
//!    placed figure only once the character layer has its figure (its attachment points
//!    and pose come from there). Despawning stops its never-ending nodes (§7.1) and
//!    releases its effects (§7.2).
//! 2. The lingering sweep (§7.3), then placement: each owner that is drawn places its
//!    attached systems with the attachment frame of §6 (static owners are drawn while they
//!    are spawned; figures while the character layer draws them, Q-RT1), then the
//!    lingering and dropped systems; then each owner's 2 s housekeeping (§7.4). The
//!    runtime advances its timers.
//! 3. [`EffectLayer::draw`] draws the systems placed in the frame (§8.15 order), each
//!    instance through the distance fade of `particles.md` §8.13, with its surface's
//!    textures, blend state and program (§9.1, §9.1.1).
//!
//! Positions are simulated in region coordinates relative to a base point fixed per region
//! (so that `f32` values stay small near where the walker started), and moved into the
//! render frame when drawn. The clock is the material clock (`material.md` §5.3.1), and one
//! shared [`Ran2`] serves every system (§3.3, §12 rule 1). The curve states of animated
//! particle surfaces are kept per surface record DID, shared by every 0x39 record and
//! instance that draws the surface (§9.1.1, §12 rule 12); the walker's record cache keeps
//! a surface's resolved material for the session, so its states last as long.
//!
//! Session figures (scenario and test-server actors, the player) are owners too, spawned by
//! the combat layer ([`session`]): the combat scripts run on them, their animation nodes
//! play clips on the figure, their StoryHook and Execute nodes reach the combat layer, and
//! Beam nodes make beams (`particle-runtime.md` §11.1: envelope, ends, head arrival, the
//! beam and head particle systems, the ribbon drawn with its materials' surfaces before the
//! systems ([`EffectLayer::draw`]), and the head entity drawn through the object layer's
//! instanced models ([`EffectLayer::update_heads`], [`EffectLayer::draw_heads`])).
//!
//! Not done here (OPEN in the specs, or no data path in the walker): fork effects are not
//! drawn (`particles.md` rule §10.2); systems with `uses_video_post` are drawn after the
//! others in the same pass (the walker has no post-processing pass, `combat-visuals.md` §11
//! rule 9); `inherit_opacity` (owners are drawn at opacity 1, walker rule (d) of runtime
//! §5.4 and the Q-RT7 Decision); the frame-buffer distortion program and the soft-particle
//! factor of runtime §9.1.1; point lights and the ambient colour of lit systems (rule 13);
//! weather systems and the global wind (Q13); the gameplay event handler (Q-RT5); frustum
//! culling by record bounds (Q-RT3, optional).

pub mod data;
pub mod session;

use std::collections::{HashMap, HashSet};
use std::sync::Arc;
use std::sync::mpsc;
use std::time::Instant;

use tessera_assets::{CancelToken, Priority, WorkerPool};
use tessera_dat::ArchiveSet;
use tessera_formats::Did;
use tessera_formats::gameplay::particles::ParticleSystem;
use tessera_formats::script_table::ParticleScriptNode;
use tessera_material::MaterialLibrary;
use tessera_particles::beam::{RibbonPart, ribbon_colour, ribbon_mesh};
use tessera_particles::glam::{Mat4, Vec3};
use tessera_particles::runtime::{
    AttachSpec, EffectHandle, GateOptions, OwnerRole, attachment_matrix, dropped_frame,
    gate_allows, is_static_environment, local_matrix, owner_scale_vectors, placement_from,
    water_height, water_probe_segment,
};
use tessera_particles::{
    AttachmentSource, CameraView, Classification, Effect, EffectManager, Environment, ForkEffect,
    ParticleNode, Placement, Ran2, SystemSim, classify_fork, classify_system, draw_order,
};
use tessera_render::{
    Instance as GpuInstance, InstanceBatch, ParticleDraw, ParticleVertex, Pass, TextureId,
};
use tessera_scriptgraph::{
    EffectSink, EntityId, EnvironmentHandle, EnvironmentRequest, ParticleHandle, ParticleStart,
    Runtime as GraphRuntime, RuntimeVocabulary, SoundHandle, SoundStart, SoundStartOutcome,
};
use tessera_world::{LandblockKey, WorldPos, WorldStreamer};

use self::data::{
    BlockScan, EffectData, EffectOwner, EffectRecord, EffectSource, OwnerKind, SurfaceLook,
};
use super::characters::{CharacterLayer, FigureEffectView};
use super::geometry::TexRequest;
use super::objects::{ObjectLayer, TexLookup};
use super::resolver::{ObjectAnswer, Resolver};
use super::view::Anchor;

/// Landblocks scanned around the viewer (Chebyshev radius).
pub const SCAN_BLOCKS: u32 = 2;
/// Owners are spawned when the viewer is this much inside their reach…
pub const SPAWN_MARGIN: f32 = 20.0;
/// …and despawned when it is this much beyond it (hysteresis).
pub const DESPAWN_MARGIN: f32 = 60.0;
/// How often owner distances are checked, seconds.
pub const OWNER_CHECK_INTERVAL: f64 = 0.25;
/// Most owners spawned per check (nearest first).
pub const MAX_SPAWNS_PER_CHECK: usize = 16;
/// Most owners alive at once.
pub const MAX_LIVE_OWNERS: usize = 2048;
/// Most landblock scans queued at once.
pub const MAX_SCANS_IN_FLIGHT: usize = 16;
/// Most record loads queued at once.
pub const MAX_LOADS_IN_FLIGHT: usize = 64;
/// Most requests waiting for one record.
pub const MAX_WAITING_PER_RECORD: usize = 4096;
/// Animated particle surface records that keep their own curve states (`material.md`
/// §5.3.1, `particle-runtime.md` §12 rule 12); beyond it a draw of another surface
/// evaluates with fresh states.
pub const MAX_SURFACE_STATES: usize = 4096;

/// Settings of the effects layer.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct EffectConfig {
    /// Particle effects on.
    pub enabled: bool,
    /// Interior entities run their effects too.
    pub interiors: bool,
    /// Seed of the shared random generator (`particle-runtime.md` §12 rule 1: the
    /// wall-clock seconds at start-up; headless runs use a fixed one).
    pub seed: u32,
}

impl Default for EffectConfig {
    fn default() -> Self {
        Self {
            enabled: true,
            interiors: false,
            seed: 1,
        }
    }
}

/// Counters for the HUD, the frame log and headless reports.
#[derive(Debug, Clone, Copy, Default, PartialEq)]
pub struct EffectStats {
    /// Start-up finished (the runtime runs).
    pub ready: bool,
    /// Landblocks scanned and kept.
    pub blocks: usize,
    /// Effect owners found in the scanned landblocks.
    pub owners_found: usize,
    /// Owners spawned in the runtime.
    pub owners_live: usize,
    /// Running systems (attached, shared, lingering, dropped).
    pub systems: usize,
    /// Lingering and dropped systems.
    pub lingering: usize,
    /// Live particles of all running systems.
    pub live_particles: usize,
    /// Systems drawn in the last frame.
    pub drawn_systems: usize,
    /// Particle vertices drawn in the last frame.
    pub drawn_vertices: usize,
    /// The largest vertex count of one frame so far.
    pub max_drawn_vertices: usize,
    /// Frames in which particles were drawn.
    pub frames_with_particles: u64,
    /// Effects the scripts asked for (attached and dropped).
    pub requests: u64,
    /// Requests that started nothing (gating, attachment lookup, missing record).
    pub refused: u64,
    /// Records loading.
    pub loading: usize,
    /// Records that failed to load.
    pub failed_records: u64,
    /// Systems whose surface texture is still loading (not drawn yet).
    pub textures_pending: usize,
    /// Script records read on the main thread.
    pub sync_script_reads: u64,
    /// Node starts of the owners' scripts so far.
    pub node_starts: u64,
    /// Finite Particle nodes skipped by the intensity gate (§5.4 step 3).
    pub gated: u64,
    /// Distinct problems the script runtime met (unimplemented kinds, missing records).
    pub issues: usize,
    /// Main-thread time of the last update and draw, ms.
    pub update_ms: f64,
    /// Session figures spawned in the runtime (combat scripts run on them).
    pub session_owners: usize,
    /// Effects the scripts of session figures asked for.
    pub session_requests: u64,
    /// Systems of session figures and beams drawn in the last frame.
    pub session_drawn_systems: usize,
    /// Their vertices in the last frame.
    pub session_drawn_vertices: usize,
    /// The largest vertex count of session systems in one frame so far.
    pub max_session_drawn_vertices: usize,
    /// Clips the scripts started on session figures.
    pub session_clips: u64,
    /// Beams alive.
    pub beams: usize,
    /// Beams started.
    pub beams_started: u64,
    /// Beam heads that arrived (their node's port fired).
    pub beam_arrivals: u64,
    /// Beam ribbons drawn in the last frame (`particle-runtime.md` §11.1 step 6).
    pub beam_ribbons_drawn: usize,
    /// The most beam ribbons drawn in one frame so far.
    pub max_beam_ribbons_drawn: usize,
    /// Ribbon vertices drawn in the last frame.
    pub beam_ribbon_vertices: usize,
    /// Beam head entities drawn in the last frame (step 9).
    pub beam_heads_drawn: usize,
    /// The most beam head entities drawn in one frame so far.
    pub max_beam_heads_drawn: usize,
    /// StoryHook nodes that ran on any owner, and those of them whose run had a story.
    pub story_hooks: [u64; 2],
}

/// What start-up delivers.
struct Boot {
    data: Arc<EffectData>,
    vocab: RuntimeVocabulary,
}

/// One owner while it is spawned.
#[derive(Debug, Clone)]
struct LiveOwner {
    kind: OwnerKind,
    iid: Option<u64>,
    points: Option<Arc<data::AttachPoints>>,
    particle_scale: f32,
}

/// A request that waits for its record (`particle-runtime.md` §5.5 step 4).
#[derive(Debug, Clone, Copy)]
enum Waiting {
    /// The attached path: served when the record has loaded.
    Attach {
        owner: EntityId,
        handle: EffectHandle,
        node: ParticleScriptNode,
    },
    /// The dropped path: placed when the record has loaded.
    Drop {
        placement: Placement,
        class: Classification,
    },
    /// A beam's particle system (slot 0 the beam system, 1 the head system), created when
    /// its record has loaded (`particle-runtime.md` §11.1 step 1; the record is read on a
    /// worker, ARCHITECTURE §21).
    Beam { beam: u64, slot: usize },
}

/// The per-frame view of an owner for attachment frames.
struct OwnerFrame<'a> {
    /// The owner's frame (sim coordinates), with its render scale.
    world: Mat4,
    /// Render scale *A*.
    scale: Vec3,
    /// Model-space bone matrices (the reference pose of a static owner).
    model: &'a [Mat4],
    points: Option<&'a data::AttachPoints>,
    drawn: bool,
}

/// Emitter attachment points (`particles.md` §8.5.5) of an owner, in its model space.
struct OwnerAttach<'a> {
    points: Option<&'a data::AttachPoints>,
    model: &'a [Mat4],
}

impl AttachmentSource for OwnerAttach<'_> {
    fn frame(&self, holding_location: u32, bone: u32) -> Option<Mat4> {
        let p = self.points?;
        if bone != 0 {
            return p.bone(bone).and_then(|b| self.model.get(b).copied());
        }
        let (b, f) = p.holding(holding_location)?;
        match b {
            None => Some(f),
            Some(b) => self.model.get(b).map(|m| *m * f),
        }
    }
}

/// The node's run-time fields (`particle-runtime.md` §5.2).
fn node_of(p: &ParticleScriptNode, stored: Option<bool>) -> ParticleNode {
    ParticleNode {
        particle_id: p.particle_id.0,
        fork_particle_id: p.fork_particle_id.0,
        should_fade: p.should_fade,
        far_fade: [p.far_fade_min, p.far_fade_max],
        close_fade: [p.close_fade_min, p.close_fade_max],
        holding_location: p.holding_location,
        bone_type: p.bone_type,
        bone_offset: Vec3::from_array(p.bone_offset),
        drop_effect: p.drop_effect,
        remove_rotation: p.remove_rotation,
        on_water: p.on_water,
        immediate_disposal: p.immediate_disposal,
        redirect_property: p.redirect_property,
        stored_never_ending: stored,
    }
}

/// The attachment of a node on an owner with `points` (§6.1): *M*ₐ and the bone; `None`
/// when the node names a holding location or bone the owner lacks (§5.5 steps 2 and 6).
fn attach_spec(p: &ParticleScriptNode, points: Option<&data::AttachPoints>) -> Option<AttachSpec> {
    let node = node_of(p, None);
    if p.bone_type != 0 {
        let bone = points?.bone(p.bone_type)?;
        return Some(AttachSpec {
            matrix: attachment_matrix(&node, None),
            bone: u32::try_from(bone).ok(),
            remove_rotation: p.remove_rotation,
        });
    }
    if p.holding_location != 0 {
        let (bone, frame) = points?.holding(p.holding_location)?;
        // §6.1: the bone of a holding location is its anchor; its frame is *M*ₐ with the
        // offset added. A location relative to the setup origin has no bone.
        return Some(AttachSpec {
            matrix: attachment_matrix(&node, Some(frame)),
            bone: bone.and_then(|b| u32::try_from(b).ok()),
            remove_rotation: p.remove_rotation,
        });
    }
    Some(AttachSpec {
        matrix: attachment_matrix(&node, None),
        bone: None,
        remove_rotation: p.remove_rotation,
    })
}

/// The effect of a loaded record, created at `now` (`None` for a surface record, which is
/// no effect).
fn make_effect(record: &EffectRecord, now: f64, node: &ParticleScriptNode) -> Option<Effect> {
    match record {
        EffectRecord::Particles { desc, .. } => {
            Some(Effect::Particles(SystemSim::new(Arc::clone(desc), now)))
        }
        EffectRecord::Fork(f) => Some(Effect::Fork(ForkEffect::new(
            f,
            now,
            node_of(node, None).fork_fade(),
        ))),
        EffectRecord::Surface(_) => None,
    }
}

/// The classification of a record (§5.3; `None` for a surface record).
fn classify(record: &EffectRecord) -> Option<Classification> {
    match record {
        EffectRecord::Particles { desc, .. } => Some(classify_system(desc).0),
        EffectRecord::Fork(f) => Some(classify_fork(f)),
        EffectRecord::Surface(_) => None,
    }
}

/// Most distinct beam head entities the layer keeps models of.
pub const MAX_HEAD_MODELS: usize = 256;

/// A beam head entity as the layer draws it (`particle-runtime.md` §11.1 steps 1, 9).
#[derive(Debug, Clone)]
struct HeadModel {
    /// Its part meshes.
    parts: Arc<[Did]>,
    /// `Render_Color` and `Render_Opacity` along its chain (white and 1 when absent).
    tint: [f32; 4],
    /// Its own `Render_AdjustableScale` ((1, 1, 1) when absent).
    scale: Vec3,
    /// It has parts and is not suppressed (`entity-descriptor.md` §9).
    drawn: bool,
}

impl HeadModel {
    fn of(a: &ObjectAnswer) -> Self {
        let [r, g, b] = a.colour.map_or([1.0; 3], |[b, g, r, _]| {
            [r, g, b].map(|c| f32::from(c) / 255.0)
        });
        let alpha = a
            .opacity
            .filter(|v| v.is_finite())
            .map_or(1.0, |v| v.clamp(0.0, 1.0));
        let scale = a.scale.map_or(Vec3::ONE, Vec3::from_array);
        Self {
            parts: Arc::clone(&a.parts),
            tint: [r, g, b, alpha],
            scale: if scale.is_finite() && scale.min_element() > 0.0 {
                scale
            } else {
                Vec3::ONE
            },
            drawn: !a.parts.is_empty() && !a.draw.suppressed(),
        }
    }
}

/// The alpha mode and texture matrices of a draw of `look` (surface record `surface`) at
/// `now`: the alpha-test reference and the texture matrices at the material clock when
/// their curves are not constant (`particle-runtime.md` §9.1.1 (d), (e)), with the states
/// of the surface record (rule 12) and mode 3 from the shared stream.
fn look_at(
    look: &SurfaceLook,
    surface: u32,
    now: f64,
    states: &mut HashMap<u32, tessera_material::CurveStates>,
    rng: &mut Ran2,
) -> (tessera_render::AlphaMode, [tessera_render::UvTransform; 2]) {
    match &look.clock {
        None => (look.alpha, look.uv),
        Some(c) => {
            if states.len() >= MAX_SURFACE_STATES && !states.contains_key(&surface) {
                c.at(now, &mut tessera_material::CurveStates::new(), rng)
            } else {
                c.at(now, states.entry(surface).or_default(), rng)
            }
        }
    }
}

/// The GPU textures of `look`; `None` (and `pending` counted) while one is still loading.
fn look_textures(
    look: &SurfaceLook,
    objects: &ObjectLayer,
    pending: &mut usize,
) -> Option<(Option<TextureId>, Option<TextureId>)> {
    let mut waiting = false;
    let mut lookup = |t: &Option<_>| match t.map(|t| objects.texture(t)) {
        None | Some(TexLookup::Failed) => None,
        Some(TexLookup::Ready(id)) => Some(id),
        Some(TexLookup::Pending) => {
            waiting = true;
            None
        }
    };
    let tex = lookup(&look.texture);
    let tex2 = lookup(&look.second_texture);
    if waiting {
        *pending += 1;
        return None;
    }
    Some((tex, tex2))
}

/// Queues the ribbons of the beams drawn this frame (`particle-runtime.md` §11.1 step 6 and
/// the ribbon vertices, RT45): camera-facing quads seen from `eye` (sim frame) with one side
/// vector per beam, coloured by `ColorAndOpacity` with its alpha × the envelope, tail, body
/// and head parts in that order, each with the look of its material's surface record through
/// the particle pipeline (§11.1 Decision (b)) and sampled with texture-coordinate pair 1
/// (Decision (a)). A beam without a `BodyMaterial` draws no ribbon (RT45), nor one whose
/// body material's record has not loaded yet (the client draws once the materials have
/// loaded, step 8); a part whose material is 0, missing or not drawable is left out.
/// Returns (ribbons with a part drawn, vertices).
fn draw_ribbons(
    st: &mut FxState,
    pass: &mut Pass,
    objects: &ObjectLayer,
    eye: Vec3,
    shift: Vec3,
    pending: &mut usize,
) -> (usize, usize) {
    let FxState {
        data,
        session,
        surface_states,
        rng,
        now,
        ..
    } = st;
    let mut beams: Vec<(&u64, &session::BeamRun)> = session
        .beams
        .iter()
        .filter(|(_, b)| b.ribbon.is_some())
        .collect();
    // Creation order (handles grow), so that equal draws keep one order frame to frame.
    beams.sort_by_key(|(h, _)| **h);
    let (mut ribbons, mut vertices) = (0usize, 0usize);
    for (_, b) in beams {
        let Some(span) = b.ribbon else { continue };
        let node = b.node();
        if data.cached_record(node.body_material.0).is_none() {
            continue;
        }
        let mesh = ribbon_mesh(node, &span, eye);
        if mesh.is_empty() {
            continue;
        }
        let colour = ribbon_colour(node, span.intensity);
        let mut any = false;
        for (part, material) in [
            (RibbonPart::Tail, node.tail_material),
            (RibbonPart::Body, node.body_material),
            (RibbonPart::Head, node.head_material),
        ] {
            let quads = mesh.part(part);
            if quads.vertices.is_empty() || material.is_null() {
                continue;
            }
            let Some(Some(record)) = data.cached_record(material.0) else {
                continue;
            };
            let EffectRecord::Surface(look) = &*record else {
                continue;
            };
            if !look.drawable {
                continue;
            }
            let (alpha, uv) = look_at(look, material.0, *now, surface_states, rng);
            let Some((tex, tex2)) = look_textures(look, objects, pending) else {
                continue;
            };
            vertices += quads.vertices.len();
            any = true;
            pass.particles(ParticleDraw {
                vertices: quads
                    .vertices
                    .iter()
                    .map(|v| ParticleVertex {
                        position: (v.position + shift).to_array(),
                        color: colour,
                        // Pair 1 (§11.1 Decision (a)).
                        uv: v.uv,
                    })
                    .collect(),
                quad_vertices: quads.vertices.len(),
                texture: tex,
                alpha,
                tint: [1.0; 4],
                material: look.material,
                shading: look.shading,
                second_texture: tex2,
                uv,
            });
        }
        ribbons += usize::from(any);
    }
    (ribbons, vertices)
}

/// Why a session figure is not spawned in the runtime ([`EffectLayer::session_entity`]).
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum SessionRefusal {
    /// Not yet: the runtime or the figure (on the GPU, with its view) is not ready.
    NotReady,
    /// The figure has no script tables.
    NoScripts,
    /// [`session::MAX_SESSION_OWNERS`] figures run already.
    Full,
    /// The figure is in another region than the runtime's.
    OtherRegion,
    /// The runtime refused the entity.
    SpawnFailed,
}

impl SessionRefusal {
    /// Whether the refusal resolves without a change of region, cap or spawn data (the
    /// figure is still loading).
    #[must_use]
    pub fn transient(self) -> bool {
        matches!(self, Self::NotReady)
    }
}

/// Everything the sink works on (separate from the runtime it is called from).
struct FxState {
    data: Arc<EffectData>,
    manager: EffectManager,
    owners: HashMap<EntityId, LiveOwner>,
    /// Figure views of this frame, by tag.
    figures: HashMap<u64, FigureEffectView>,
    /// Water heights of the resident landblocks.
    water: HashMap<LandblockKey, f32>,
    /// The particle handle's owner (§7.1 needs it to stop).
    handles: HashMap<u64, EntityId>,
    waiting: HashMap<u32, Vec<Waiting>>,
    loads: HashSet<u32>,
    load_tx: mpsc::Sender<u32>,
    pool: WorkerPool,
    region: u8,
    base: [f64; 2],
    now: f64,
    frame: u64,
    rng: Ran2,
    gate: GateOptions,
    whitelist: HashSet<u32>,
    textures: HashSet<TexRequest>,
    new_textures: Vec<TexRequest>,
    requests: u64,
    refused: u64,
    /// Curve states of the animated particle surfaces, per surface record DID (the
    /// alpha-test reference and texture matrices evaluated per draw, `particle-runtime.md`
    /// §9.1.1, §12 rule 12: shared by every 0x39 record naming the surface).
    surface_states: HashMap<u32, tessera_material::CurveStates>,
    /// Session figures, their clips, hooks, Execute answers and beams ([`session`]).
    session: session::SessionState,
}

impl FxState {
    /// Region-global position into sim coordinates.
    fn to_sim(&self, p: [f64; 3]) -> Vec3 {
        Vec3::new(
            (p[0] - self.base[0]) as f32,
            (p[1] - self.base[1]) as f32,
            p[2] as f32,
        )
    }

    /// The owner's frame, scale, pose and points for this frame.
    fn owner_frame<'a>(&'a self, owner: &'a LiveOwner) -> Option<OwnerFrame<'a>> {
        match owner.kind {
            OwnerKind::Static {
                position,
                rotation,
                scale,
            } => {
                let s = Vec3::from_array(scale);
                let s = if s.is_finite() { s } else { Vec3::ONE };
                Some(OwnerFrame {
                    world: Mat4::from_scale_rotation_translation(
                        s,
                        rotation,
                        self.to_sim(position),
                    ),
                    scale: s,
                    model: owner
                        .points
                        .as_deref()
                        .map_or(&[], |p| p.reference.as_slice()),
                    points: owner.points.as_deref(),
                    drawn: true,
                })
            }
            OwnerKind::Figure { .. } | OwnerKind::Session { .. } => {
                let v = match owner.kind {
                    OwnerKind::Figure { tag } => self.figures.get(&tag)?,
                    OwnerKind::Session { net } => self.session.views.get(&net)?,
                    OwnerKind::Static { .. } => return None,
                };
                if v.region != self.region {
                    return None;
                }
                let s = v.scale;
                Some(OwnerFrame {
                    world: Mat4::from_scale_rotation_translation(
                        s,
                        v.rotation,
                        self.to_sim(v.position),
                    ),
                    scale: s,
                    model: &v.model,
                    points: v.points.as_deref(),
                    drawn: v.drawn,
                })
            }
        }
    }

    fn role(owner: &LiveOwner) -> OwnerRole {
        if owner.iid.is_some_and(is_static_environment) {
            OwnerRole::StaticEnvironment
        } else {
            OwnerRole::Other
        }
    }

    /// Queues the load of `did` (once; requests beyond [`MAX_LOADS_IN_FLIGHT`] are queued
    /// again by [`FxState::retry_loads`]).
    fn load(&mut self, did: u32) {
        if self.loads.contains(&did) || self.loads.len() >= MAX_LOADS_IN_FLIGHT {
            return;
        }
        let data = Arc::clone(&self.data);
        let reply = LoadReply {
            tx: Some(self.load_tx.clone()),
            did,
        };
        let job = move |_: &tessera_assets::JobContext<'_>| {
            let _ = data.record(did);
            drop(reply);
        };
        match self.pool.spawn(Priority::Visible, CancelToken::new(), job) {
            Ok(_) => {
                self.loads.insert(did);
            }
            Err(e) => tracing::debug!("effects: record load not queued: {e}"),
        }
    }

    /// Queues the loads of records that requests wait for and that no job is loading.
    fn retry_loads(&mut self) {
        let missing: Vec<u32> = self
            .waiting
            .keys()
            .filter(|d| !self.loads.contains(d))
            .copied()
            .collect();
        for did in missing {
            if self.data.cached_record(did).is_some() {
                self.loaded(did);
            } else {
                self.load(did);
            }
        }
    }

    fn wait(&mut self, did: u32, w: Waiting) {
        let list = self.waiting.entry(did).or_default();
        if list.len() < MAX_WAITING_PER_RECORD {
            list.push(w);
        }
        self.load(did);
    }

    fn note_texture(&mut self, record: &EffectRecord) {
        let look = match record {
            EffectRecord::Particles { look, .. } | EffectRecord::Surface(look) => look,
            EffectRecord::Fork(_) => return,
        };
        for t in look.textures() {
            if self.textures.insert(t) {
                self.new_textures.push(t);
            }
        }
    }

    /// §5.5 step 6 for a served request: resolve the attachment again and store it.
    fn serve(
        &mut self,
        owner: EntityId,
        handle: EffectHandle,
        node: ParticleScriptNode,
        record: &Arc<EffectRecord>,
    ) {
        let points = self.owners.get(&owner).and_then(|o| match o.kind {
            OwnerKind::Static { .. } => o.points.clone(),
            OwnerKind::Figure { tag } => self.figures.get(&tag).and_then(|v| v.points.clone()),
            OwnerKind::Session { net } => {
                self.session.views.get(&net).and_then(|v| v.points.clone())
            }
        });
        let Some(spec) = attach_spec(&node, points.as_deref()) else {
            tracing::debug!(
                owner = owner.0,
                holding = node.holding_location,
                bone = node.bone_type,
                "effects: attachment not found when the record loaded; discarded"
            );
            self.manager.stop(owner.0, handle, true, self.now);
            self.refused += 1;
            return;
        };
        let now = self.now;
        self.note_texture(record);
        if self
            .manager
            .serve(owner.0, handle, || make_effect(record, now, &node), spec)
            .is_none()
        {
            tracing::trace!(
                owner = owner.0,
                "effects: request stopped before its record loaded"
            );
        }
    }

    /// The attached path (§5.5 steps 2–4).
    fn start_attached(&mut self, req: &ParticleStart) -> Option<ParticleHandle> {
        let owner = self.owners.get(&req.target)?.clone();
        // Step 2: the pre-check on an owner with attachment points.
        let p = &req.node;
        if (p.holding_location != 0 || p.bone_type != 0)
            && let Some(f) = self.owner_frame(&owner)
            && f.points.is_some()
            && attach_spec(p, f.points).is_none()
        {
            return None;
        }
        // Step 4: gating (`particles.md` §7.1.2; the walker runs the client's defaults).
        if !gate_allows(Self::role(&owner), req.effect.0, &self.whitelist, self.gate) {
            return None;
        }
        let handle = self.manager.request(req.target.0, req.effect.0, self.now);
        self.handles.insert(handle.0, req.target);
        match self.data.cached_record(req.effect.0) {
            Some(Some(record)) => self.serve(req.target, handle, *p, &record),
            Some(None) => {
                self.manager.stop(req.target.0, handle, true, self.now);
                self.refused += 1;
            }
            None => self.wait(
                req.effect.0,
                Waiting::Attach {
                    owner: req.target,
                    handle,
                    node: *p,
                },
            ),
        }
        Some(ParticleHandle(handle.0))
    }

    /// The water probe (§5.7) at sim position `point`: the landblock water plane's height
    /// when the segment meets it.
    fn water_probe(&self, point: Vec3) -> Option<Vec3> {
        let world = WorldPos::new(
            self.region,
            f64::from(point.x) + self.base[0],
            f64::from(point.y) + self.base[1],
            f64::from(point.z),
        );
        let h = *self.water.get(&world.landblock()?)?;
        let (top, bottom) = water_probe_segment(point, point.z < h);
        let hits = (h <= top.z && h >= bottom.z).then_some(h);
        water_height(point, hits)
    }

    /// The dropped path (§5.6).
    fn start_dropped(&mut self, req: &ParticleStart) {
        let Some(owner) = self.owners.get(&req.entity).cloned() else {
            return;
        };
        let Some(f) = self.owner_frame(&owner) else {
            return;
        };
        let p = req.node;
        // Step 2: the attachment frame with the current pose.
        let Some(spec) = attach_spec(&p, f.points) else {
            return;
        };
        let bone = spec.bone.and_then(|b| f.model.get(b as usize).copied());
        let l = local_matrix(spec.matrix, bone, spec.remove_rotation);
        let attachment = f.world * l;
        let (scale_a, scale_b) = owner_scale_vectors(f.scale, Some(owner.particle_scale));
        // Steps 3–4: water, then the second offset (zero on water).
        let frame = if p.on_water {
            let point = attachment.w_axis.truncate();
            let Some(on) = self.water_probe(point) else {
                return;
            };
            let mut m = attachment;
            m.w_axis = on.extend(1.0);
            m
        } else {
            dropped_frame(attachment, Vec3::from_array(p.bone_offset))
        };
        let placement = placement_from(frame, scale_a, scale_b, 1.0);
        let class = Classification::from_stored(req.never_ending);
        match self.data.cached_record(req.effect.0) {
            Some(Some(record)) => self.drop_now(req.effect.0, &record, placement, class),
            Some(None) => self.refused += 1,
            None => self.wait(req.effect.0, Waiting::Drop { placement, class }),
        }
    }

    /// §5.6 step 5: the system without the node's fade settings (a fork effect gets the
    /// defaults, `particles.md` §6.5).
    fn drop_now(
        &mut self,
        did: u32,
        record: &Arc<EffectRecord>,
        placement: Placement,
        class: Classification,
    ) {
        self.note_texture(record);
        let Some(effect) = make_effect(record, self.now, &ParticleScriptNode::default()) else {
            self.refused += 1;
            return;
        };
        self.manager
            .drop_effect(did, effect, placement, class, self.now);
    }

    /// Serves every request waiting for `did`.
    fn loaded(&mut self, did: u32) {
        self.loads.remove(&did);
        // A job that ended without reading the record (dropped unrun) leaves the requests
        // waiting; `retry_loads` queues it again.
        let Some(record) = self.data.cached_record(did) else {
            return;
        };
        let waiting = self.waiting.remove(&did).unwrap_or_default();
        for w in waiting {
            match (w, &record) {
                (
                    Waiting::Attach {
                        owner,
                        handle,
                        node,
                    },
                    Some(r),
                ) => {
                    let r = Arc::clone(r);
                    self.serve(owner, handle, node, &r);
                }
                (Waiting::Attach { owner, handle, .. }, None) => {
                    self.manager.stop(owner.0, handle, true, self.now);
                    self.refused += 1;
                }
                (Waiting::Drop { placement, class }, Some(r)) => {
                    let r = Arc::clone(r);
                    self.drop_now(did, &r, placement, class);
                }
                (Waiting::Drop { .. }, None) => self.refused += 1,
                (Waiting::Beam { beam, slot }, Some(r)) => {
                    let r = Arc::clone(r);
                    self.beam_system_loaded(beam, slot, did, &r);
                }
                (Waiting::Beam { .. }, None) => self.refused += 1,
            }
        }
    }
}

/// The runtime's sink: Particle nodes go to [`FxState`]; sounds are refused (world audio
/// runs its own graph), environments ignored.
struct Sink<'a> {
    st: &'a mut FxState,
}

impl EffectSink for Sink<'_> {
    fn start_sound(&mut self, _request: &SoundStart) -> SoundStartOutcome {
        SoundStartOutcome::Refused
    }
    fn stop_sound(&mut self, _handle: SoundHandle) {}
    fn set_sound_volume(&mut self, _handle: SoundHandle, _volume: f32) {}
    fn register_environment(&mut self, _request: &EnvironmentRequest) -> EnvironmentHandle {
        EnvironmentHandle(0)
    }
    fn unregister_environment(&mut self, _handle: EnvironmentHandle) {}

    fn classify_particle(&mut self, effect: Did) -> Option<bool> {
        let r = self.st.data.cached_record(effect.0).flatten()?;
        Some(classify(&r)?.stored_byte())
    }

    fn start_particle(&mut self, request: &ParticleStart) -> Option<ParticleHandle> {
        self.st.requests += 1;
        if self.st.session.is_session(request.target) || self.st.session.is_session(request.entity)
        {
            self.st.session.requests += 1;
        }
        if request.node.drop_effect {
            self.st.start_dropped(request);
            return None;
        }
        let h = self.st.start_attached(request);
        if h.is_none() {
            self.st.refused += 1;
        }
        h
    }

    fn stop_particle(&mut self, handle: ParticleHandle, immediate: bool) {
        if let Some(owner) = self.st.handles.remove(&handle.0) {
            self.st
                .manager
                .stop(owner.0, EffectHandle(handle.0), immediate, self.st.now);
        }
    }

    fn object_exists(&mut self, id: u64) -> bool {
        self.st.owners.contains_key(&EntityId(id))
    }

    fn start_animation(
        &mut self,
        entity: EntityId,
        node: &tessera_formats::script_table::AnimationNode,
        intensity: f32,
    ) -> tessera_scriptgraph::AnimationOutcome {
        self.st.session_animation(entity, node, intensity)
    }

    fn stop_animation(&mut self, handle: tessera_scriptgraph::AnimationHandle) {
        self.st.session_animation_stop(handle);
    }

    fn update_animation(
        &mut self,
        handle: tessera_scriptgraph::AnimationHandle,
        node: &tessera_formats::script_table::AnimationNode,
    ) {
        self.st.session_animation_update(handle, node);
    }

    fn story_hook(&mut self, event: &tessera_scriptgraph::StoryHookEvent) {
        self.st.session_hook(event);
    }

    fn execute(
        &mut self,
        request: &tessera_scriptgraph::ExecuteRequest<'_>,
    ) -> Option<tessera_scriptgraph::ExecuteReply> {
        self.st.session_execute(request)
    }

    fn start_beam(
        &mut self,
        request: &tessera_scriptgraph::BeamStart,
    ) -> Option<tessera_scriptgraph::BeamHandle> {
        self.st.session_beam(request)
    }

    fn stop_beam(&mut self, handle: tessera_scriptgraph::BeamHandle) {
        self.st.session_beam_stop(handle);
    }
}

/// Reports a record load's end exactly once, also when the job never runs.
struct LoadReply {
    tx: Option<mpsc::Sender<u32>>,
    did: u32,
}

impl Drop for LoadReply {
    fn drop(&mut self) {
        if let Some(tx) = self.tx.take() {
            let _ = tx.send(self.did);
        }
    }
}

/// One scanned landblock.
#[derive(Debug)]
struct BlockFx {
    bundle: usize,
    scan: Option<Arc<BlockScan>>,
    token: CancelToken,
}

type ScanMsg = Result<BlockScan, (LandblockKey, usize)>;

/// Sends a scan job's reply exactly once, also when the job never runs.
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

/// What the layer needs from the walker each frame.
#[derive(Debug)]
pub struct EffectInput<'a> {
    /// Frame time, seconds.
    pub dt: f64,
    /// The material clock (`material.md` §5.3.1), seconds.
    pub now: f64,
    /// The streamer (resident landblocks, water).
    pub streamer: &'a WorldStreamer,
    /// The viewer's eye.
    pub eye: Option<WorldPos>,
    /// The character layer (placed figures).
    pub characters: &'a CharacterLayer,
}

/// The effects layer (module docs).
pub struct EffectLayer {
    config: EffectConfig,
    pool: WorkerPool,
    boot: Option<mpsc::Receiver<Result<Boot, String>>>,
    graph: Option<GraphRuntime<'static>>,
    st: Option<FxState>,
    blocks: HashMap<LandblockKey, BlockFx>,
    scans_in_flight: usize,
    scan_tx: mpsc::Sender<ScanMsg>,
    scan_rx: mpsc::Receiver<ScanMsg>,
    load_rx: Option<mpsc::Receiver<u32>>,
    live: HashMap<EntityId, LandblockKey>,
    next_check: f64,
    pending_spawns: usize,
    time: f64,
    problem: Option<String>,
    stats: EffectStats,
    /// Beam head entities by DID ([`EffectLayer::update_heads`]).
    heads: HashMap<Did, HeadModel>,
    /// Parts referenced in the object layer for head entities.
    head_parts: HashSet<u32>,
}

impl std::fmt::Debug for EffectLayer {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("EffectLayer")
            .field("config", &self.config)
            .field("stats", &self.stats)
            .finish_non_exhaustive()
    }
}

impl EffectLayer {
    /// Starts the start-up job (dictionary, vocabulary).
    #[must_use]
    pub fn new(
        config: EffectConfig,
        set: Arc<ArchiveSet>,
        materials: Arc<MaterialLibrary<Arc<ArchiveSet>>>,
        pool: WorkerPool,
        layers: Vec<u32>,
    ) -> Self {
        let (scan_tx, scan_rx) = mpsc::channel();
        let mut problem = None;
        let boot = if config.enabled {
            let (tx, rx) = mpsc::sync_channel(1);
            let interiors = config.interiors;
            let job = move |_: &tessera_assets::JobContext<'_>| {
                let r = EffectData::open(set, materials, layers, interiors).map(|data| {
                    let vocab = RuntimeVocabulary::from_data(&data.dict, |d| {
                        crate::audio::scripts::mapper(&data.set, d)
                    });
                    Boot {
                        data: Arc::new(data),
                        vocab,
                    }
                });
                let _ = tx.send(r);
            };
            match pool.spawn(Priority::Visible, CancelToken::new(), job) {
                Ok(_) => Some(rx),
                Err(e) => {
                    problem = Some(format!("start-up job: {e}"));
                    None
                }
            }
        } else {
            None
        };
        Self {
            config,
            pool,
            boot,
            graph: None,
            st: None,
            blocks: HashMap::new(),
            scans_in_flight: 0,
            scan_tx,
            scan_rx,
            load_rx: None,
            live: HashMap::new(),
            next_check: 0.0,
            pending_spawns: 0,
            time: 0.0,
            problem,
            stats: EffectStats::default(),
            heads: HashMap::new(),
            head_parts: HashSet::new(),
        }
    }

    /// Counters of the last frame.
    #[must_use]
    pub fn stats(&self) -> EffectStats {
        self.stats
    }

    /// Whether nothing the viewer should see is still loading: start-up done (or off),
    /// no landblock scan or record load in flight, no owner in range waiting to spawn and
    /// no drawn system waiting for its texture.
    #[must_use]
    pub fn settled(&self) -> bool {
        if !self.config.enabled || self.problem.is_some() {
            return true;
        }
        self.boot.is_none()
            && self.scans_in_flight == 0
            && self.pending_spawns == 0
            && self.stats.loading == 0
            && self.stats.textures_pending == 0
    }

    /// One HUD line.
    #[must_use]
    pub fn hud_line(&self) -> String {
        let s = &self.stats;
        if !self.config.enabled {
            return "Particles off".into();
        }
        if let Some(p) = &self.problem {
            return format!("Particles: {p}");
        }
        if !s.ready {
            return "Particles: starting".into();
        }
        format!(
            "Particles: {} owners ({} live), {} systems ({} lingering), {} particles, {} drawn / {} vertices",
            s.owners_found,
            s.owners_live,
            s.systems,
            s.lingering,
            s.live_particles,
            s.drawn_systems,
            s.drawn_vertices
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
                let graph = GraphRuntime::new(
                    EffectSource(Arc::clone(&b.data)),
                    Arc::clone(&b.data.dict),
                    b.vocab,
                )
                .with_seed(u64::from(self.config.seed));
                let (load_tx, load_rx) = mpsc::channel();
                self.graph = Some(graph);
                self.load_rx = Some(load_rx);
                self.st = Some(FxState {
                    data: b.data,
                    manager: EffectManager::new(),
                    owners: HashMap::new(),
                    figures: HashMap::new(),
                    water: HashMap::new(),
                    handles: HashMap::new(),
                    waiting: HashMap::new(),
                    loads: HashSet::new(),
                    load_tx,
                    pool: self.pool.clone(),
                    region: 0,
                    base: [0.0; 2],
                    now: 0.0,
                    frame: 0,
                    rng: Ran2::new(self.config.seed),
                    gate: GateOptions::default(),
                    whitelist: HashSet::new(),
                    textures: HashSet::new(),
                    new_textures: Vec::new(),
                    requests: 0,
                    refused: 0,
                    surface_states: HashMap::new(),
                    session: session::SessionState::default(),
                });
                self.stats.ready = true;
                tracing::info!("particle effects ready");
            }
            Err(e) => {
                tracing::warn!("particle effects: {e}");
                self.problem = Some(e);
            }
        }
    }

    /// Runs `f` with the runtime and a sink over the state.
    fn with_graph<R>(
        graph: &mut Option<GraphRuntime<'static>>,
        st: &mut Option<FxState>,
        f: impl FnOnce(&mut GraphRuntime<'static>, &mut Sink<'_>) -> R,
    ) -> Option<R> {
        let graph = graph.as_mut()?;
        let st = st.as_mut()?;
        let mut sink = Sink { st };
        Some(f(graph, &mut sink))
    }

    fn sync_blocks(&mut self, streamer: &WorldStreamer, here: LandblockKey) {
        let Some(data) = self.st.as_ref().map(|s| Arc::clone(&s.data)) else {
            return;
        };
        let mut wanted = HashSet::new();
        let mut gone_blocks = Vec::new();
        for (key, _ring, bundle) in streamer.resident() {
            let Some(d) = key.chebyshev(here) else {
                continue;
            };
            if d > SCAN_BLOCKS + 1 {
                continue;
            }
            wanted.insert(key);
            let identity = Arc::as_ptr(bundle) as usize;
            let fresh = self.blocks.get(&key).is_none_or(|b| b.bundle != identity);
            if !fresh || d > SCAN_BLOCKS {
                continue;
            }
            if self.scans_in_flight >= MAX_SCANS_IN_FLIGHT && key != here {
                continue;
            }
            if let Some(old) = self.blocks.remove(&key) {
                old.token.cancel();
                gone_blocks.push(key);
            }
            let token = CancelToken::new();
            self.blocks.insert(
                key,
                BlockFx {
                    bundle: identity,
                    scan: None,
                    token: token.clone(),
                },
            );
            self.scans_in_flight += 1;
            let reply = ScanReply {
                tx: Some(self.scan_tx.clone()),
                key,
                identity,
            };
            let data = Arc::clone(&data);
            let bundle = Arc::clone(bundle);
            let job = move |ctx: &tessera_assets::JobContext<'_>| {
                if ctx.is_cancelled() {
                    return;
                }
                reply.send(data.scan_block(&bundle, identity));
            };
            let priority = if key == here {
                Priority::Immediate
            } else {
                Priority::Prefetch
            };
            if let Err(e) = self.pool.spawn(priority, token, job) {
                tracing::debug!("effects: landblock scan not queued: {e}");
            }
        }
        let dropped: Vec<LandblockKey> = self
            .blocks
            .keys()
            .filter(|k| !wanted.contains(k))
            .copied()
            .collect();
        for k in dropped {
            if let Some(b) = self.blocks.remove(&k) {
                b.token.cancel();
            }
            gone_blocks.push(k);
        }
        for k in gone_blocks {
            self.despawn_block(k);
        }
        while let Ok(msg) = self.scan_rx.try_recv() {
            self.scans_in_flight = self.scans_in_flight.saturating_sub(1);
            match msg {
                Ok(scan) => {
                    if let Some(b) = self.blocks.get_mut(&scan.key)
                        && b.bundle == scan.bundle
                    {
                        b.scan = Some(Arc::new(scan));
                    }
                }
                Err((key, identity)) => {
                    if self
                        .blocks
                        .get(&key)
                        .is_some_and(|b| b.bundle == identity && b.scan.is_none())
                    {
                        self.blocks.remove(&key);
                    }
                }
            }
        }
    }

    fn despawn_block(&mut self, key: LandblockKey) {
        let ids: Vec<EntityId> = self
            .live
            .iter()
            .filter(|(_, k)| **k == key)
            .map(|(id, _)| *id)
            .collect();
        for id in ids {
            self.despawn(id);
        }
    }

    fn despawn(&mut self, id: EntityId) {
        self.live.remove(&id);
        let _ = Self::with_graph(&mut self.graph, &mut self.st, |g, sink| g.despawn(sink, id));
        if let Some(st) = self.st.as_mut() {
            let now = st.now;
            st.manager.remove_owner(id.0, now);
            st.owners.remove(&id);
            st.handles.retain(|_, o| *o != id);
        }
    }

    /// Clears everything (a region change).
    fn clear(&mut self) {
        let ids: Vec<EntityId> = self.live.keys().copied().collect();
        for id in ids {
            self.despawn(id);
        }
        for net in self.session_entities() {
            self.remove_session_entity(net);
        }
        if let Some(st) = self.st.as_mut() {
            st.manager = EffectManager::new();
            st.waiting.clear();
            st.handles.clear();
            // Beams' system keys belong to the discarded manager (a new one hands out the
            // same keys again), and their nodes were stopped with their owners.
            st.session.beams.clear();
            st.session.arrived.clear();
            st.session.finished.clear();
        }
    }

    /// The owners to spawn and despawn for a viewer at `eye` (region coordinates).
    fn check_owners(&mut self, eye: [f64; 3], characters: &CharacterLayer) {
        let dist = |p: [f64; 3]| -> f32 {
            let d = [p[0] - eye[0], p[1] - eye[1], p[2] - eye[2]];
            ((d[0] * d[0] + d[1] * d[1] + d[2] * d[2]).sqrt()) as f32
        };
        let mut despawn = Vec::new();
        let mut spawn: Vec<(f32, LandblockKey, EffectOwner)> = Vec::new();
        for (key, b) in &self.blocks {
            let Some(scan) = &b.scan else { continue };
            for o in &scan.owners {
                let pos = match o.kind {
                    OwnerKind::Figure { tag } => characters
                        .figure_effect_view(tag)
                        .map_or(o.position, |v| v.position),
                    OwnerKind::Static { .. } | OwnerKind::Session { .. } => o.position,
                };
                let d = dist(pos);
                let reach = o.scripts.reach;
                if self.live.contains_key(&o.id) {
                    // A distance that is not a number despawns too.
                    let within = d <= reach + DESPAWN_MARGIN;
                    if !within {
                        despawn.push(o.id);
                    }
                } else if d < reach + SPAWN_MARGIN {
                    spawn.push((d, *key, o.clone()));
                }
            }
        }
        for id in despawn {
            self.despawn(id);
        }
        spawn.sort_by(|a, b| a.0.total_cmp(&b.0));
        let mut pending = 0usize;
        let mut spawned = 0usize;
        for (_, key, o) in spawn {
            if spawned >= MAX_SPAWNS_PER_CHECK || self.live.len() >= MAX_LIVE_OWNERS {
                pending += 1;
                continue;
            }
            // A placed figure waits for its figure (attachment points, pose).
            if let OwnerKind::Figure { tag } = o.kind {
                match characters.figure_effect_view(tag) {
                    Some(v) => {
                        if let Some(st) = self.st.as_mut() {
                            st.figures.insert(tag, v);
                        }
                    }
                    None => continue,
                }
            }
            self.spawn(key, &o);
            spawned += 1;
        }
        self.pending_spawns = pending;
    }

    fn spawn(&mut self, key: LandblockKey, o: &EffectOwner) {
        let Some(st) = self.st.as_mut() else { return };
        st.owners.insert(
            o.id,
            LiveOwner {
                kind: o.kind,
                iid: o.iid,
                points: o.points.clone(),
                particle_scale: o.scales.particle,
            },
        );
        let init = tessera_scriptgraph::EntityInit {
            tables: o.scripts.tables.clone(),
            startup_scripts: o.scripts.startup_scripts.clone(),
            properties: o.scripts.properties.clone(),
            parent: None,
            run_startup: true,
            // Effect owners are not figures: their Random nodes keep the runtime's
            // generator (`script-runtime.md` §8 OPEN-18 and its implementer note).
            random_seed: None,
        };
        match Self::with_graph(&mut self.graph, &mut self.st, |g, sink| {
            g.spawn_entity(sink, o.id, init)
        }) {
            Some(Ok(())) => {
                self.live.insert(o.id, key);
            }
            Some(Err(e)) => {
                tracing::debug!(id = o.id.0, "effects: owner not spawned: {e}");
                if let Some(st) = self.st.as_mut() {
                    st.owners.remove(&o.id);
                }
            }
            None => {}
        }
    }

    /// One frame (module docs).
    pub fn update(&mut self, input: EffectInput<'_>, objects: &mut ObjectLayer) {
        let t0 = Instant::now();
        let dt = if input.dt.is_finite() {
            input.dt.max(0.0)
        } else {
            0.0
        };
        self.time += dt;
        self.poll_boot();
        if self.st.is_none() {
            return;
        }
        let Some(eye) = input.eye else {
            return;
        };
        let now = if input.now.is_finite() {
            input.now
        } else {
            0.0
        };
        // A region change starts over with a new base point.
        let region_changed = self.st.as_ref().is_some_and(|s| s.region != eye.region);
        if region_changed {
            self.clear();
            if let Some(st) = self.st.as_mut() {
                st.region = eye.region;
                st.base = Anchor::containing(&eye).origin();
            }
        }
        if let Some(st) = self.st.as_mut() {
            st.now = now;
            st.frame = st.frame.wrapping_add(1);
            st.water.clear();
            for (key, _, bundle) in input.streamer.resident() {
                if let Some(h) = bundle
                    .water
                    .as_ref()
                    .and_then(|w| w.water())
                    .map(|w| w.height)
                    .filter(|h| h.is_finite())
                {
                    st.water.insert(key, h);
                }
            }
            // Figure views of the live figure owners.
            st.figures.clear();
            let tags: Vec<u64> = st
                .owners
                .values()
                .filter_map(|o| match o.kind {
                    OwnerKind::Figure { tag } => Some(tag),
                    OwnerKind::Static { .. } | OwnerKind::Session { .. } => None,
                })
                .collect();
            for tag in tags {
                if let Some(v) = input.characters.figure_effect_view(tag) {
                    st.figures.insert(tag, v);
                }
            }
            // Views of the session figures.
            let nets: Vec<u64> = st.session.by_net.keys().copied().collect();
            for net in nets {
                if let Some(v) = input
                    .characters
                    .entity_effect_view(tessera_net::EntityId(net))
                {
                    st.session.views.insert(net, v);
                }
            }
            if st.session.clip_ctx.is_none() {
                st.session.clip_ctx = input.characters.figure_context();
            }
        }
        // Records that loaded.
        let loaded: Vec<u32> = self
            .load_rx
            .as_ref()
            .map(|rx| rx.try_iter().collect())
            .unwrap_or_default();
        if let Some(st) = self.st.as_mut() {
            for did in loaded {
                st.loaded(did);
            }
            st.retry_loads();
        }
        // 1. Landblocks and owners.
        if let Some(here) = eye.landblock() {
            self.sync_blocks(input.streamer, here);
        }
        if self.time >= self.next_check {
            self.next_check = self.time + OWNER_CHECK_INTERVAL;
            self.check_owners([eye.x, eye.y, eye.z], input.characters);
        }
        // The runtime's timers.
        let _ = Self::with_graph(&mut self.graph, &mut self.st, |g, sink| g.advance(sink, dt));
        // 2. Sweep, placement, housekeeping (§3.2, Q-RT1 Decision).
        if let Some(st) = self.st.as_mut() {
            let frame = st.frame;
            st.manager.begin_frame(frame, now);
            let env = Environment::default();
            let ids: Vec<EntityId> = st.owners.keys().copied().collect();
            for id in &ids {
                let Some(owner) = st.owners.get(id).cloned() else {
                    continue;
                };
                let Some(f) = st.owner_frame(&owner) else {
                    continue;
                };
                if !f.drawn {
                    continue;
                }
                let (scale_a, scale_b) = owner_scale_vectors(f.scale, Some(owner.particle_scale));
                let world = f.world;
                let model: Vec<Mat4> = f.model.to_vec();
                let points = f.points.cloned();
                let attach = OwnerAttach {
                    points: points.as_ref(),
                    model: &model,
                };
                let bone_model = |b: u32| model.get(b as usize).copied();
                let FxState { manager, rng, .. } = st;
                manager.place_owner(
                    id.0,
                    frame,
                    now,
                    world,
                    scale_a,
                    scale_b,
                    1.0,
                    &bone_model,
                    &env,
                    rng,
                    &attach,
                );
            }
            let FxState { manager, rng, .. } = st;
            manager.place_lingering(frame, now, &env, rng, &tessera_particles::NoAttachments);
            for id in &ids {
                st.manager.housekeep(id.0, now);
            }
            // Beams (`particle-runtime.md` §11.1): ends, head, systems.
            st.update_beams(frame, &env);
            // Textures of new systems.
            let new = std::mem::take(&mut st.new_textures);
            if !new.is_empty() {
                objects.acquire_textures(new);
            }
        }
        // Beam heads that arrived fire their node's port; removed beams end their node.
        self.finish_beams();
        self.refresh_stats();
        self.stats.update_ms = t0.elapsed().as_secs_f64() * 1e3;
    }

    fn finish_beams(&mut self) {
        let Some(st) = self.st.as_mut() else { return };
        let arrived = std::mem::take(&mut st.session.arrived);
        let finished = std::mem::take(&mut st.session.finished);
        let _ = Self::with_graph(&mut self.graph, &mut self.st, |g, sink| {
            for h in arrived {
                g.beam_arrived(sink, h);
            }
            for h in &finished {
                g.beam_finished(sink, *h);
            }
        });
        if let Some(st) = self.st.as_mut() {
            for h in finished {
                st.session.beams.remove(&h.0);
            }
        }
    }

    fn refresh_stats(&mut self) {
        let Some(st) = self.st.as_ref() else { return };
        self.stats.blocks = self.blocks.len();
        self.stats.owners_found = self
            .blocks
            .values()
            .filter_map(|b| b.scan.as_ref())
            .map(|s| s.owners.len())
            .sum();
        self.stats.owners_live = self.live.len();
        self.stats.systems = st.manager.running_count();
        self.stats.lingering = st.manager.lingering().len();
        self.stats.requests = st.requests;
        self.stats.session_owners = st.session.by_net.len();
        self.stats.session_requests = st.session.requests;
        self.stats.session_clips = st.session.clips_started;
        self.stats.beams = st.session.beams.len();
        self.stats.beams_started = st.session.beams_started;
        self.stats.beam_arrivals = st.session.beam_arrivals;
        self.stats.story_hooks = [st.session.story_hooks, st.session.story_hooks_with_story];
        self.stats.refused = st.refused;
        self.stats.loading = st.waiting.len().max(st.loads.len());
        self.stats.sync_script_reads = st.data.main_thread_reads();
        if let Some(g) = &self.graph {
            let rs = g.stats();
            self.stats.node_starts = rs.node_starts;
            self.stats.gated = rs.particles_gated;
            self.stats.issues = g.issues().len();
        }
    }

    /// Queues the particle draws of the systems placed this frame into `pass` (module docs,
    /// step 3), for a camera at `eye` (render frame) looking along `dir`; the ribbons of the
    /// beams drawn this frame (`particle-runtime.md` §11.1 step 6) come first.
    pub fn draw(
        &mut self,
        pass: &mut Pass,
        anchor: &Anchor,
        eye: Vec3,
        dir: Vec3,
        objects: &ObjectLayer,
    ) {
        let t0 = Instant::now();
        let Some(st) = self.st.as_mut() else {
            return;
        };
        if anchor.region != st.region {
            return;
        }
        // Render frame = sim + (base − anchor origin).
        let origin = anchor.origin();
        let shift = Vec3::new(
            (st.base[0] - origin[0]) as f32,
            (st.base[1] - origin[1]) as f32,
            0.0,
        );
        let cam_eye = eye - shift;
        let camera = CameraView::look_at(cam_eye, cam_eye + dir);
        let now = st.now;
        let frame = st.frame;
        let session_systems = st.session_systems();
        let mut pending = 0usize;
        let ribbons = draw_ribbons(st, pass, objects, cam_eye, shift, &mut pending);
        let FxState {
            manager,
            rng,
            data,
            surface_states,
            ..
        } = st;
        let mut drawn = Vec::new();
        let mut live = 0usize;
        for (key, effect) in manager.effects_mut() {
            let Some(sim) = effect.particles_mut() else {
                continue;
            };
            live += sim.live_particles();
            if sim.instance_frame() != Some(frame) {
                continue;
            }
            let desc: &Arc<ParticleSystem> = sim.desc();
            // `combat-visuals.md` §11 rule 9: the walker has no pass after post-processing,
            // so these systems are drawn after the others in the same pass (§8.15).
            let post = desc.uses_video_post;
            let Some(d) = sim.draw(now, &camera, rng) else {
                continue;
            };
            let surface = d.surface.0;
            drawn.push((key, d, surface, post));
        }
        let ranks: Vec<(i32, u64)> = drawn
            .iter()
            .map(|(k, d, _, _)| (d.sorting_priority, manager.age_rank(*k).unwrap_or(u64::MAX)))
            .collect();
        let mut order = draw_order(&ranks);
        // Stable: the post-pass systems keep their order after the others.
        order.sort_by_key(|&i| drawn.get(i).is_some_and(|d| d.3));
        let mut systems = 0usize;
        let mut vertices = 0usize;
        let mut session_drawn = 0usize;
        let mut session_vertices = 0usize;
        for i in order {
            let Some((key, d, surface, _)) = drawn.get(i) else {
                continue;
            };
            let from_session = session_systems.contains(key);
            let Some(did) = manager.did_of(*key) else {
                continue;
            };
            let Some(Some(record)) = data.cached_record(did) else {
                continue;
            };
            let EffectRecord::Particles { look, .. } = &*record else {
                continue;
            };
            if !look.drawable {
                continue;
            }
            let (alpha, uv) = look_at(look, *surface, now, surface_states, rng);
            let Some((tex, tex2)) = look_textures(look, objects, &mut pending) else {
                continue;
            };
            systems += 1;
            session_drawn += usize::from(from_session);
            for inst in &d.instances {
                if inst.vertices.is_empty() {
                    continue;
                }
                vertices += inst.vertices.len();
                if from_session {
                    session_vertices += inst.vertices.len();
                }
                pass.particles(ParticleDraw {
                    vertices: inst
                        .vertices
                        .iter()
                        .map(|v| ParticleVertex {
                            position: (Vec3::from_array(v.position) + shift).to_array(),
                            color: v.colour,
                            uv: v.uv,
                        })
                        .collect(),
                    quad_vertices: inst.quad_vertices,
                    texture: tex,
                    alpha,
                    tint: [1.0, 1.0, 1.0, inst.opacity],
                    material: look.material,
                    shading: look.shading,
                    second_texture: tex2,
                    uv,
                });
            }
        }
        self.stats.live_particles = live;
        self.stats.drawn_systems = systems;
        self.stats.drawn_vertices = vertices + ribbons.1;
        self.stats.max_drawn_vertices = self.stats.max_drawn_vertices.max(vertices + ribbons.1);
        if vertices + ribbons.1 > 0 {
            self.stats.frames_with_particles += 1;
        }
        self.stats.textures_pending = pending;
        self.stats.session_drawn_systems = session_drawn;
        self.stats.session_drawn_vertices = session_vertices;
        self.stats.max_session_drawn_vertices =
            self.stats.max_session_drawn_vertices.max(session_vertices);
        self.stats.beam_ribbons_drawn = ribbons.0;
        self.stats.max_beam_ribbons_drawn = self.stats.max_beam_ribbons_drawn.max(ribbons.0);
        self.stats.beam_ribbon_vertices = ribbons.1;
        self.stats.update_ms += t0.elapsed().as_secs_f64() * 1e3;
    }

    /// Resolves the head entities of the live beams (`particle-runtime.md` §11.1 step 1)
    /// through the walker's resolver and references their parts in the object layer, where
    /// they load like sky objects. At most [`MAX_HEAD_MODELS`] distinct head entities are
    /// kept (the data names 109); their parts stay referenced for the layer's life.
    pub(crate) fn update_heads(&mut self, resolver: &mut Resolver, objects: &mut ObjectLayer) {
        let Some(st) = self.st.as_ref() else {
            return;
        };
        let wanted: HashSet<Did> = st
            .session
            .beams
            .values()
            .map(|b| b.node().head_entity)
            .filter(|d| !d.is_null())
            .collect();
        let mut new_parts = Vec::new();
        for did in wanted {
            if self.heads.contains_key(&did) || self.heads.len() >= MAX_HEAD_MODELS {
                continue;
            }
            let Some(answer) = resolver.object(did) else {
                continue;
            };
            let model = HeadModel::of(&answer);
            if model.drawn {
                for p in model.parts.iter() {
                    if self.head_parts.insert(p.0) {
                        new_parts.push(p.0);
                    }
                }
            } else {
                tracing::debug!(%did, "beam head entity has no drawn geometry");
            }
            self.heads.insert(did, model);
        }
        if !new_parts.is_empty() {
            objects.acquire_model_parts(&new_parts);
        }
    }

    /// Queues the head entities of the beams that show one this frame (`particle-runtime.md`
    /// §11.1 step 9) on `pass`: at the head point, turned along the beam, scaled by the
    /// entity's `Render_AdjustableScale` times the vector its `HeadEntityScaleSource`
    /// selected, tinted by its `Render_Color` and `Render_Opacity` (`material.md` §8.3),
    /// through the object layer's instanced models (one batch per part). Returns the heads
    /// drawn.
    pub(crate) fn draw_heads(
        &mut self,
        pass: &mut Pass,
        anchor: &Anchor,
        objects: &mut ObjectLayer,
    ) -> usize {
        let Some(st) = self.st.as_ref() else {
            return 0;
        };
        if anchor.region != st.region {
            return 0;
        }
        let origin = anchor.origin();
        let shift = Vec3::new(
            (st.base[0] - origin[0]) as f32,
            (st.base[1] - origin[1]) as f32,
            0.0,
        );
        let mut batches: HashMap<u32, Vec<GpuInstance>> = HashMap::new();
        let mut heads = 0usize;
        for b in st.session.beams.values() {
            let Some((point, rot)) = b.head else {
                continue;
            };
            let Some(model) = self.heads.get(&b.node().head_entity).filter(|m| m.drawn) else {
                continue;
            };
            let scale = model.scale * b.head_scale;
            let m = Mat4::from_scale_rotation_translation(scale, rot, point + shift);
            if !m.is_finite() {
                continue;
            }
            let mut any = false;
            for p in model.parts.iter() {
                if objects.model(p.0, 0).is_none() {
                    continue;
                }
                let mut i = GpuInstance::new(m);
                i.tint = model.tint;
                batches.entry(p.0).or_default().push(i);
                any = true;
            }
            heads += usize::from(any);
        }
        for (part, list) in batches {
            let Some(model) = objects.model(part, 0).cloned() else {
                continue;
            };
            let mut batch = InstanceBatch::new(model, list);
            batch.uv = objects.animated_uv(part, 0);
            pass.instanced(batch);
        }
        self.stats.beam_heads_drawn = heads;
        self.stats.max_beam_heads_drawn = self.stats.max_beam_heads_drawn.max(heads);
        heads
    }

    // ----- session figures and their scripts (`session`) -----

    /// Whether the runtime runs (start-up finished).
    #[must_use]
    pub fn runtime_ready(&self) -> bool {
        self.graph.is_some() && self.st.is_some()
    }

    /// The runtime ID of the session entity `net`'s figure, spawning it in the runtime once
    /// its figure is on the GPU with its scripts: its figure's tables and `extra_tables`
    /// (for example the shared UI selection table, `combat-ui.md` TR6), its stored values,
    /// no start-up scripts. The error says why it is not (yet) spawned.
    pub fn session_entity(
        &mut self,
        net: tessera_net::EntityId,
        characters: &CharacterLayer,
        extra_tables: &[Did],
    ) -> Result<EntityId, SessionRefusal> {
        let st = self.st.as_mut().ok_or(SessionRefusal::NotReady)?;
        if let Some(&id) = st.session.by_net.get(&net.0) {
            return Ok(id);
        }
        let view = characters
            .entity_effect_view(net)
            .ok_or(SessionRefusal::NotReady)?;
        let scripts = view.scripts.clone().ok_or(SessionRefusal::NoScripts)?;
        if st.session.by_net.len() >= session::MAX_SESSION_OWNERS {
            return Err(SessionRefusal::Full);
        }
        if view.region != st.region {
            return Err(SessionRefusal::OtherRegion);
        }
        let id = EntityId(session::SESSION_OWNER_BASE + st.session.next);
        st.session.next += 1;
        st.add_session_owner(id, net.0, &view);
        let mut tables = scripts.tables.clone();
        for t in extra_tables {
            if !tables.contains(t) {
                tables.push(*t);
            }
        }
        let init = tessera_scriptgraph::EntityInit {
            tables,
            startup_scripts: Vec::new(),
            properties: scripts.properties.clone(),
            parent: None,
            run_startup: false,
            random_seed: Some(net.0),
        };
        match Self::with_graph(&mut self.graph, &mut self.st, |g, sink| {
            g.spawn_entity(sink, id, init)
        }) {
            Some(Ok(())) => Ok(id),
            _ => {
                if let Some(st) = self.st.as_mut() {
                    st.remove_session_owner(net.0);
                }
                Err(SessionRefusal::SpawnFailed)
            }
        }
    }

    /// The runtime ID of a session entity already spawned.
    #[must_use]
    pub fn session_id(&self, net: tessera_net::EntityId) -> Option<EntityId> {
        self.st.as_ref()?.session.by_net.get(&net.0).copied()
    }

    /// The network ID of a session figure's runtime ID.
    #[must_use]
    pub fn session_net(&self, id: EntityId) -> Option<tessera_net::EntityId> {
        self.st
            .as_ref()?
            .session
            .by_runtime
            .get(&id)
            .map(|n| tessera_net::EntityId(*n))
    }

    /// The session entities spawned in the runtime, by network ID.
    #[must_use]
    pub fn session_entities(&self) -> Vec<tessera_net::EntityId> {
        self.st.as_ref().map_or_else(Vec::new, |st| {
            st.session
                .by_net
                .keys()
                .map(|n| tessera_net::EntityId(*n))
                .collect()
        })
    }

    /// Despawns a session figure from the runtime (its runs stop, its effects linger).
    pub fn remove_session_entity(&mut self, net: tessera_net::EntityId) {
        let Some(id) = self
            .st
            .as_ref()
            .and_then(|s| s.session.by_net.get(&net.0).copied())
        else {
            return;
        };
        let _ = Self::with_graph(&mut self.graph, &mut self.st, |g, sink| g.despawn(sink, id));
        if let Some(st) = self.st.as_mut() {
            st.remove_session_owner(net.0);
        }
    }

    /// The local player in the runtime (`ThePlayer` targets).
    pub fn set_runtime_player(&mut self, player: Option<EntityId>) {
        if let Some(g) = self.graph.as_mut() {
            g.set_player(player);
        }
    }

    /// The Execute callbacks' vocabulary.
    pub fn set_execute_vocab(&mut self, v: session::ExecuteVocab) {
        if let Some(st) = self.st.as_mut() {
            st.session.exec = v;
        }
    }

    /// Starts `key` on `id` with run parameters (`combat-visuals.md` §3.2); the run when it
    /// stays live.
    pub fn run_script(
        &mut self,
        id: EntityId,
        key: u32,
        params: &[(u32, tessera_formats::PropertyValue)],
    ) -> Option<tessera_scriptgraph::RunId> {
        Self::with_graph(&mut self.graph, &mut self.st, |g, sink| {
            let r = g.start_script_with(sink, id, key, params).ok()?;
            tracing::debug!(entity = id.0, key, start = ?r.start, "effects: script run");
            r.run
        })
        .flatten()
    }

    /// Stops a run.
    pub fn stop_run(&mut self, run: tessera_scriptgraph::RunId) {
        let _ = Self::with_graph(&mut self.graph, &mut self.st, |g, sink| {
            g.stop_run(sink, run)
        });
    }

    /// Whether a run is live.
    #[must_use]
    pub fn run_live(&self, run: tessera_scriptgraph::RunId) -> bool {
        self.graph.as_ref().is_some_and(|g| g.run_live(run))
    }

    /// Sets an entity property in the runtime.
    pub fn set_entity_property(
        &mut self,
        id: EntityId,
        pid: u32,
        value: tessera_formats::PropertyValue,
    ) {
        let _ = Self::with_graph(&mut self.graph, &mut self.st, |g, sink| {
            g.set_property(sink, id, pid, value)
        });
    }

    /// Clears an entity property in the runtime.
    pub fn clear_entity_property(&mut self, id: EntityId, pid: u32) {
        let _ = Self::with_graph(&mut self.graph, &mut self.st, |g, sink| {
            g.clear_property(sink, id, pid)
        });
    }

    /// An entity property's value in the runtime.
    #[must_use]
    pub fn entity_property(
        &self,
        id: EntityId,
        pid: u32,
    ) -> Option<tessera_formats::PropertyValue> {
        let g = self.graph.as_ref()?;
        g.has_entity(id).then(|| g.property(id, pid)).flatten()
    }

    /// The case values of the Switch on `pid` in the definition of `key` on `id`.
    pub fn switch_cases(
        &mut self,
        id: EntityId,
        key: u32,
        pid: u32,
    ) -> Vec<tessera_formats::PropertyValue> {
        self.graph
            .as_mut()
            .map(|g| g.switch_cases(id, key, pid))
            .unwrap_or_default()
    }

    /// The story hooks the runtime produced since the last call.
    pub fn take_story_hooks(&mut self) -> Vec<tessera_scriptgraph::StoryHookEvent> {
        self.st
            .as_mut()
            .map(|s| std::mem::take(&mut s.session.hooks))
            .unwrap_or_default()
    }

    /// The open stories' strikes, for the Execute callbacks.
    pub fn set_upcoming(&mut self, view: crate::walker::combat::state::UpcomingView) {
        if let Some(st) = self.st.as_mut() {
            st.session.upcoming = view;
        }
    }

    /// The clips the scripts started or stopped since the last call, for the character
    /// layer.
    pub fn take_clip_commands(
        &mut self,
    ) -> Vec<crate::walker::characters::combat_anim::ClipCommand> {
        self.st
            .as_mut()
            .map(|s| std::mem::take(&mut s.session.clips))
            .unwrap_or_default()
    }

    /// Stops everything: every owner is despawned.
    pub fn shutdown(&mut self) {
        self.clear();
        for b in self.blocks.values() {
            b.token.cancel();
        }
        self.blocks.clear();
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    /// `particle-runtime.md` §11.1 steps 1 and 9: the head entity's own scale, tint and
    /// suppression.
    #[test]
    fn head_models_take_scale_tint_and_suppression() {
        let mut a = ObjectAnswer::failed();
        a.parts = Arc::from(vec![Did(0x0100_0001)]);
        a.colour = Some([0, 128, 255, 255]);
        a.opacity = Some(2.0);
        a.scale = Some([2.0, 2.0, 3.0]);
        let m = HeadModel::of(&a);
        assert!(m.drawn);
        assert_eq!(m.scale, Vec3::new(2.0, 2.0, 3.0));
        assert!((m.tint[0] - 1.0).abs() < 1e-6 && m.tint[2] == 0.0 && m.tint[3] == 1.0);
        a.scale = Some([f32::NAN, 1.0, 1.0]);
        a.draw.render_draw = Some(false);
        let m = HeadModel::of(&a);
        assert_eq!(m.scale, Vec3::ONE);
        assert!(!m.drawn, "a suppressed head entity is not drawn");
        assert!(!HeadModel::of(&ObjectAnswer::failed()).drawn);
    }

    fn points() -> data::AttachPoints {
        data::AttachPoints {
            holding: vec![
                (5, None, Mat4::from_translation(Vec3::new(0.0, 0.0, 2.0))),
                (6, Some(1), Mat4::from_translation(Vec3::new(1.0, 0.0, 0.0))),
            ],
            bones: vec![(9, 1)],
            reference: vec![
                Mat4::IDENTITY,
                Mat4::from_translation(Vec3::new(0.0, 3.0, 0.0)),
            ],
        }
    }

    fn node() -> ParticleScriptNode {
        ParticleScriptNode {
            particle_id: Did(0x3900_0001),
            bone_offset: [0.0, 0.0, 0.5],
            ..ParticleScriptNode::default()
        }
    }

    /// §6.1: holding location, bone type and none.
    #[test]
    fn attachment_specs_follow_the_node_fields() {
        let p = points();
        let none = attach_spec(&node(), None).expect("no attachment needs no points");
        assert_eq!(none.bone, None);
        assert!(
            none.matrix
                .w_axis
                .truncate()
                .abs_diff_eq(Vec3::new(0.0, 0.0, 0.5), 1e-6)
        );
        let held = attach_spec(
            &ParticleScriptNode {
                holding_location: 6,
                ..node()
            },
            Some(&p),
        )
        .expect("location 6");
        assert_eq!(held.bone, Some(1));
        assert!(
            held.matrix
                .w_axis
                .truncate()
                .abs_diff_eq(Vec3::new(1.0, 0.0, 0.5), 1e-6)
        );
        let bone = attach_spec(
            &ParticleScriptNode {
                bone_type: 9,
                holding_location: 5,
                ..node()
            },
            Some(&p),
        )
        .expect("bone type 9 wins");
        assert_eq!(bone.bone, Some(1));
        // A location or bone the owner lacks: nothing starts (§5.5 steps 2 and 6).
        for n in [
            ParticleScriptNode {
                holding_location: 7,
                ..node()
            },
            ParticleScriptNode {
                bone_type: 2,
                ..node()
            },
        ] {
            assert!(attach_spec(&n, Some(&p)).is_none());
            assert!(attach_spec(&n, None).is_none());
        }
    }

    /// `particles.md` §8.5.5: an emitter's own bone wins over its holding location.
    #[test]
    fn emitter_attachment_points_resolve_in_model_space() {
        let p = points();
        let a = OwnerAttach {
            points: Some(&p),
            model: &p.reference,
        };
        let at = |hl, b| a.frame(hl, b).map(|m| m.w_axis.truncate());
        assert_eq!(at(5, 0), Some(Vec3::new(0.0, 0.0, 2.0)));
        assert_eq!(at(6, 0), Some(Vec3::new(1.0, 3.0, 0.0)));
        assert_eq!(at(5, 9), Some(Vec3::new(0.0, 3.0, 0.0)));
        assert_eq!(at(7, 0), None);
        let none = OwnerAttach {
            points: None,
            model: &[],
        };
        assert!(none.frame(5, 0).is_none());
    }

    #[test]
    fn node_fields_carry_over() {
        let n = node_of(
            &ParticleScriptNode {
                drop_effect: true,
                immediate_disposal: true,
                ..node()
            },
            Some(true),
        );
        assert_eq!(n.effect_did(), Some(0x3900_0001));
        assert!(n.drop_effect && n.immediate_disposal);
        assert_eq!(n.stored_never_ending, Some(true));
        assert_eq!(n.far_fade, [40.0, 60.0]);
    }
}
