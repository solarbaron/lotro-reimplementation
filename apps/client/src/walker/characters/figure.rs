// SPDX-License-Identifier: GPL-3.0-or-later
//! Figures built on the asset workers: an appearance (and, for a figure the landblocks
//! place, its instance property set) in, everything the main thread uploads and animates
//! out.
//!
//! One job per distinct figure ([`FigureKey`]) runs, in order (`appearance.md` §8,
//! `skeleton-anim.md` §10, `script-table.md` §11.3):
//!
//! 1. **Assembly** with `tessera-mesh` ([`CharacterAssembler`]): a playable body with its
//!    worn and held items, or a placed object (weenie game object or 0x47 entity) with its
//!    instance set.
//! 2. **Skeleton** from the setup's tagfile (`skeleton-anim.md` §2.1).
//! 3. **Clips and shape**: the script tables choose idle, walk and run
//!    ([`ClipResolver`], `script-table.md` §11.3; every looping idle layer, walk and run
//!    on `movement`), the held proportion clips (`script-table.md` §6.7, §6.8) and the
//!    species table's body-shape deformer and bone sliders (`appearance.md` §6.4,
//!    [`figure_shape`]); the clips are decoded once per DID ([`ClipStore`]), baked for
//!    sampling ([`BakeCache`]) and bound to the skeleton (track count = bone count,
//!    `skeleton-anim.md` §3.4). The body shape and sliders become a [`BindDeformation`].
//! 4. **Parts**: every visible body part decoded, flattened at its display level and
//!    prepared for the GPU, skinned to the skeleton where the mesh is skinned, its skinned
//!    vertices deformed once in bind space by the figure's deformation; materials
//!    resolved with the part's appearance layers (`material.md` §9.3), then the species
//!    table's composited face texture and material edits (`appearance.md` §3.5, §8 step 6;
//!    `figure_look`, shared with the explorer), into the walker's material plans; held items' parts prepared rigid, keyed by holding location, with an
//!    attach table of the wearer's locations (`skeleton-anim.md` §8.4), each with its
//!    stowed or wielded placement (`appearance.md` §5.6).
//!
//! A placed figure's start-up scripts may append Appearance nodes to its look (the
//! random-appearance outfits, `appearance.md` §7.4): the Idle resolution of step 3 lists
//! them, its Random nodes picking with a generator seeded from the placement's instance ID
//! (the §7.4 Decision: a figure keeps its look), and when there are any the figure is
//! assembled again with them before its parts are built.
//!
//! The figure's heights, styles, slider values and appearance values are the avatar
//! entity's (`AC_Avatar_Height` 0 is the shortest River-hobbit, `appearance.md` §6.2)
//! unless the request's [`LookChoices`] carry the character's own (a received look under
//! the receiver rule, decision 0025 D2, or the creation screen's choices): the appearance
//! values go into the figure's script state, so the species tables compose the face and
//! tint hair and skin with them (`appearance.md` §3.5).
//!
//! A figure on a character screen ([`FigureRequest::chargen_stage`]) carries its
//! `CharGen_Stage`, and its `CharGen_Action`, `Agent_Class` and `Render_AlwaysSelectable`
//! when the request names them (`character-screens.md` §2.1.4), from the start and is not
//! placed in a world (`appearance.md` §3.5 Decision); every other figure gets
//! `Physics_InWorld` after its start-up. A character-screen figure also starts the script
//! `Chargen` after `Startup` and its class table's start-up names (§2.1.4, CS28), which
//! gives it its stage stance; it loops only its `LoopForever` clips and plays its certain
//! one-shot clips once ([`OneShot`], `script-table.md` §6.10).
//!
//! A figure without a usable skeleton, or with no skinned part, is **static**: its parts
//! are drawn rigid in the setup frame, unanimated. Every failure short of "no body" is a
//! note in [`FigureAsset::notes`]; nothing panics on hostile data (every decoder below
//! bounds its input).

use std::collections::HashMap;
use std::sync::{Arc, Mutex, PoisonError};
use std::time::Instant;

use tessera_anim::{
    AttachFrame, AttachPointDesc, AttachTable, BakeCache, BindDeformation, BoundClip,
    Skeleton as AnimSkeleton, bind_shape,
};
use tessera_dat::ArchiveSet;
use tessera_formats::script_table::resolve::{
    ClipCandidate, ClipResolver, MotionState, collect_entity_scripts,
};
use tessera_formats::setup::Anchor;
use tessera_formats::{Did, PropertySet, PropertyValue};
use tessera_havok::Clip;
use tessera_material::{Layering, LibraryConfig, MaterialLibrary, SurfaceLook};
use tessera_material::{ParameterEdit, composed_edit};
use tessera_mesh::assembly::look::{
    FigureLook, LookCache, LookLimits, apply_figure_properties, figure_after_startup,
    figure_look_from_data, figure_on_screen,
};
use tessera_mesh::assembly::shape::{FigureShape, figure_shape};
use tessera_mesh::assembly::{
    AssembledCharacter, AssemblyOptions, CharacterAssembler, CharacterSpec, EquippedItem,
    GameDataSource, MaterialLayer, ScriptAppearance, StyleChoice, WieldState,
};
use tessera_mesh::{DecodeOptions, GeometryLimits, Mesh};
use tessera_net::{Body, Equipped};
use tessera_render::{MeshConfig, PreparedMesh};

use super::catalog::CharacterCatalog;
use crate::walker::geometry::{
    MaterialPlan, display_level, level_mesh_data, mesh_skin, plan_material_dyed,
};

/// Most parts (body and held) a figure keeps.
pub const MAX_FIGURE_PARTS: usize = 64;
/// Most decoded clips the [`ClipStore`] keeps.
pub const MAX_CACHED_CLIPS: usize = 1024;
/// Byte budget of baked clips (every frame sampled once).
pub const BAKE_BUDGET: usize = 256 << 20;
/// Largest clip baked (larger clips are sampled from their curves).
pub const MAX_BAKED_CLIP: usize = 16 << 20;
/// Fallback natural walking speed (m/s) of a walk clip without displacement: the walk
/// controller's walking speed (`script-table.md` §8.5 Decision: such clips move at the
/// controller's speed).
pub const FALLBACK_WALK_SPEED: f32 = 2.0;
/// Fallback natural running speed (m/s), as [`FALLBACK_WALK_SPEED`].
pub const FALLBACK_RUN_SPEED: f32 = 6.0;

/// Why a figure could not be built at all.
#[derive(Debug, thiserror::Error)]
pub enum FigureError {
    /// The install lacks what assembly needs (registry, vocabulary).
    #[error("character data unavailable: {0}")]
    Unavailable(String),
    /// The body does not resolve.
    #[error(transparent)]
    Assembly(#[from] tessera_mesh::assembly::AssemblyError),
    /// The body kind is not supported.
    #[error("unsupported body")]
    UnsupportedBody,
    /// The job was cancelled.
    #[error("cancelled")]
    Cancelled,
}

/// What to build.
#[derive(Debug, Clone, PartialEq)]
pub struct FigureRequest {
    /// The body.
    pub body: Body,
    /// Worn and held items (playable bodies).
    pub equipment: Vec<Equipped>,
    /// The instance property set of a placed figure.
    pub instance: Option<Arc<PropertySet>>,
    /// Identity of `instance` (a placed figure's tag), part of the key.
    pub instance_key: Option<u64>,
    /// Draw rigid and unanimated even when the body could be animated.
    pub static_only: bool,
    /// `AC_Avatar_Height` of a playable body (`appearance.md` §6.2); `None` keeps the
    /// avatar entity's (0 for the River-hobbit: the shortest). The walker takes it from a
    /// received look (decision 0025 D2).
    pub avatar_height: Option<f32>,
    /// `AC_BoneScale_Test` of a playable body (`appearance.md` §6.2, §6.4); `None` keeps the
    /// avatar entity's.
    pub bone_scale: Option<f32>,
    /// Seed of the figure's script Random picks (`appearance.md` §7.4 Decision: the placed
    /// figure's instance ID, so it keeps its look); `None` uses 0.
    pub seed: Option<u64>,
    /// The `CharGen_Stage` key of a figure shown on a character screen
    /// (`character-screens.md` §2.2; `appearance.md` §3.5 Decision: such a figure carries
    /// its stage and is not placed in a world); `None` for a figure in the world.
    pub chargen_stage: Option<u32>,
    /// The `CharGen_Action` key of a character-screen figure (`character-screens.md`
    /// §2.1.4: 2 `Idle`, 3 `Hover`), carried from the start like the stage.
    pub chargen_action: Option<u32>,
    /// The `Agent_Class` a character-screen figure is created with (the chosen class on
    /// the creation stages, §2.1.4).
    pub class: Option<u32>,
    /// `Render_AlwaysSelectable` true (the race-and-class figure, `character-screens.md`
    /// §2.1.4, CS28).
    pub always_selectable: bool,
}

/// The look choices of a playable figure beyond its heights (decision 0025 D2: a
/// received look under the receiver rule; the creation screen's choices): chosen styles,
/// the slider string and the character's own appearance values, as `tessera-mesh`'s
/// [`CharacterSpec`] takes them. The appearance values (colours, face layers, the head
/// value, …) reach the figure's script state, where the species tables' driven nodes read
/// them for the composited face and the hair and skin tints (`appearance.md` §3.5,
/// `tessera_mesh::assembly::look`).
#[derive(Debug, Clone, Default, PartialEq)]
pub struct LookChoices {
    /// Chosen styles (`CharacterSpec::styles`).
    pub styles: Vec<StyleChoice>,
    /// The slider byte string (`AC_BoneSlider_Value_Array`).
    pub slider_values: Option<String>,
    /// The character's own property values (`CharacterSpec::appearance_values`), data
    /// order (colours as stored: B, G, R, A).
    pub appearance_values: Vec<(u32, PropertyValue)>,
}

impl LookChoices {
    /// Whether it changes nothing.
    #[must_use]
    pub fn is_empty(&self) -> bool {
        self.styles.is_empty() && self.slider_values.is_none() && self.appearance_values.is_empty()
    }
}

/// [`value_key`]'s form: type code, bits, text.
type ValueKey = (u32, u64, Option<String>);

/// A hashable form of an appearance value: its type code and its value's bits (the
/// debug form for values without fixed bits; a look holds only scalars, short strings and
/// colours).
fn value_key(v: &PropertyValue) -> ValueKey {
    let t = v.property_type().code();
    match v {
        PropertyValue::Bool(b) => (t, u64::from(*b), None),
        PropertyValue::Int(i) => (t, u64::from(i.cast_unsigned()), None),
        PropertyValue::Int64(i) => (t, i.cast_unsigned(), None),
        PropertyValue::Float(f) => (t, u64::from(f.to_bits()), None),
        PropertyValue::Enum(k) | PropertyValue::Bitfield32(k) => (t, u64::from(*k), None),
        PropertyValue::Bitfield64(k) => (t, *k, None),
        PropertyValue::DidRef(d) => (t, u64::from(d.0), None),
        PropertyValue::Colour(c) => (t, u64::from(u32::from_le_bytes(*c)), None),
        PropertyValue::String(s) => (t, 0, Some(s.clone())),
        other => (t, 0, Some(format!("{other:?}"))),
    }
}

impl FigureRequest {
    /// The cache key (no look).
    #[must_use]
    pub fn key(&self) -> FigureKey {
        self.key_with(&LookChoices::default())
    }

    /// The cache key with `look`.
    #[must_use]
    pub fn key_with(&self, look: &LookChoices) -> FigureKey {
        FigureKey {
            look: look
                .styles
                .iter()
                .map(|s| {
                    let e = s.element.unwrap_or_default();
                    (
                        s.key,
                        s.modifier.to_bits(),
                        if s.element.is_some() { e.file.0 } else { 0 },
                        e.toggle_file.map_or(0, |d| d.0),
                        e.toggle_property.unwrap_or(0),
                    )
                })
                .collect(),
            sliders: look.slider_values.clone(),
            values: look
                .appearance_values
                .iter()
                .map(|(p, v)| (*p, value_key(v)))
                .collect(),
            body: self.body,
            items: self
                .equipment
                .iter()
                .map(|e| {
                    (
                        e.item.0,
                        e.dye.map_or(u32::MAX, f32::to_bits),
                        e.secondary_dye.map_or(u32::MAX, f32::to_bits),
                        e.stowed,
                        e.slot,
                    )
                })
                .collect(),
            instance: self.instance_key,
            static_only: self.static_only,
            shape: [
                self.avatar_height.map_or(u32::MAX, f32::to_bits),
                self.bone_scale.map_or(u32::MAX, f32::to_bits),
            ],
            seed: self.seed,
            stage: self.chargen_stage,
            screen: (self.chargen_action, self.class, self.always_selectable),
        }
    }
}

/// Identifies a figure: equal keys share one asset and one set of GPU resources.
#[derive(Debug, Clone, PartialEq, Eq, Hash)]
pub struct FigureKey {
    look: Vec<(u32, u32, u32, u32, u32)>,
    sliders: Option<String>,
    values: Vec<(u32, ValueKey)>,
    body: Body,
    items: Vec<(u32, u32, u32, bool, Option<u8>)>,
    instance: Option<u64>,
    static_only: bool,
    shape: [u32; 2],
    seed: Option<u64>,
    stage: Option<u32>,
    screen: (Option<u32>, Option<u32>, bool),
}

/// One part ready for upload.
#[derive(Debug)]
pub struct PartAsset {
    /// The prepared streams (taken on upload).
    pub mesh: Mutex<Option<PreparedMesh>>,
    /// GPU bytes of the streams.
    pub gpu_bytes: u64,
    /// One plan per renderer submesh.
    pub plans: Vec<MaterialPlan>,
    /// Drawn with the figure's skinning palette.
    pub skinned: bool,
    /// The attach-table key of a held item's part (`None`: a body part): the index of its
    /// held part, one attach point per placement (`appearance.md` §5.6); a screen figure's
    /// prop parts have keys from [`super::live::PROP_KEY_BASE`].
    pub held: Option<u32>,
    /// The appearance variants the part belongs to (bit *i*: variant *i*,
    /// [`super::live`]); [`ALL_VARIANTS`] for every figure but a live screen figure's
    /// stage pieces.
    pub variants: u8,
}

/// The variant mask of a part drawn in every variant.
pub const ALL_VARIANTS: u8 = u8::MAX;

/// One clip a figure plays.
#[derive(Debug, Clone)]
pub struct ClipPlay {
    /// The clip DID.
    pub did: u32,
    /// The clip bound to the figure's skeleton.
    pub clip: Arc<BoundClip>,
    /// Playback rate from the script node (`script-table.md` §8.5).
    pub rate: f32,
    /// Natural ground speed (m/s) for locomotion clips.
    pub speed: f32,
    /// The data's animation layer (`AnimationScript_Layer` key).
    pub layer: u32,
    /// The node asks for per-track weights (`UseBoneOpacity`, `script-table.md` §6.5).
    pub track_weights: bool,
    /// For a held clip (`ManualUpdate`, `script-table.md` §6.7): the fraction of the clip's
    /// whole duration it shows (the held fraction × the played length, over the duration,
    /// `tessera_anim::held_clip_fraction`); it never advances.
    pub held: Option<f32>,
    /// A one-shot clip of a character-screen figure ([`OneShot`]): played once and then
    /// removed; `None` for looping and held clips.
    pub once: Option<OneShot>,
}

/// How a one-shot clip plays (`LoopForever` 0, `ManualUpdate` 0, rate at least 0.0002;
/// `script-table.md` §6.10 steps 3–5 and 8, `character-screens.md` §2.1.4 "One-shot stage
/// clips"): from its start time for `length` seconds, faded out over `ramp_out` seconds
/// that end at its end, then removed. Never held at its last frame, never looped beyond
/// its `LoopCount`.
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct OneShot {
    /// The clip time it starts at, seconds (§6.10 step 5: min(*f*₀ × duration ÷ rate,
    /// duration)).
    pub start: f32,
    /// Wall-clock seconds from its start to its end *τ*_end (§6.10 step 4).
    pub length: f32,
    /// `RampOut`, seconds (below [`MIN_RATE`] it is removed at once at its end).
    pub ramp_out: f32,
    /// It plays its clip more than once (`LoopCount` above 1): the clip time wraps.
    pub wraps: bool,
}

/// The smallest rate (and ramp) the client treats as non-zero (`script-table.md` §6.10).
pub const MIN_RATE: f32 = 0.0002;

/// What `script-table.md` §6.10 makes of a played (not held) clip.
#[derive(Debug, Clone, Copy, PartialEq)]
pub enum PlayPlan {
    /// No end: a looping clip (`LoopForever` 1), or a rate below [`MIN_RATE`].
    Endless,
    /// A one-shot clip.
    Once(OneShot),
    /// Its end has passed before it starts (step 4): not started.
    Skip,
}

impl PlayPlan {
    /// The plan of a clip with `loop_forever`, `loop_count`, the start fraction
    /// `start_fraction` (*f*₀), `end_time` (`EndTime`), `ramp_out`, its `duration` and its
    /// playback `rate`. Non-finite inputs give [`PlayPlan::Skip`] for a one-shot clip.
    #[must_use]
    pub fn of(
        loop_forever: bool,
        loop_count: i32,
        start_fraction: f32,
        end_time: f32,
        ramp_out: f32,
        duration: f32,
        rate: f32,
    ) -> Self {
        if loop_forever || !(rate.is_finite() && rate >= MIN_RATE) {
            return Self::Endless;
        }
        if !(duration.is_finite() && duration > 0.0 && end_time.is_finite()) {
            return Self::Skip;
        }
        let f0 = if start_fraction.is_finite() {
            start_fraction.clamp(0.0, 1.0)
        } else {
            0.0
        };
        // τ₀ = now − f₀ × duration ÷ rate (step 3); τ_end = τ₀ + (LoopCount × duration −
        // (1 − EndTime) × duration) ÷ rate (step 4).
        let started = f0 * duration / rate;
        let played = (loop_count as f32 * duration - (1.0 - end_time) * duration) / rate;
        let length = played - started;
        if !(length.is_finite() && length > 0.0) {
            return Self::Skip;
        }
        Self::Once(OneShot {
            start: started.min(duration),
            length,
            ramp_out: if ramp_out.is_finite() {
                ramp_out.max(0.0)
            } else {
                0.0
            },
            wraps: loop_count > 1,
        })
    }
}

/// The clips of idle, walk and run.
#[derive(Debug, Clone, Default)]
pub struct FigureClips {
    /// The looping idle clips, one per data layer (the most likely of each), lowest layer
    /// first, at most [`MAX_IDLE_LAYERS`]; for a character-screen figure also its one-shot
    /// stage clips ([`ClipPlay::once`]).
    pub idle: Vec<ClipPlay>,
    /// Walk forward.
    pub walk: Option<ClipPlay>,
    /// Run forward.
    pub run: Option<ClipPlay>,
    /// Held proportion clips (`script-table.md` §6.8), all of them, lowest data layer
    /// first (several may share a layer, §6.2): shown in every motion state.
    pub held: Vec<ClipPlay>,
}

/// Most idle layers a figure plays.
pub const MAX_IDLE_LAYERS: usize = 6;

/// A built figure.
#[derive(Debug)]
pub struct FigureAsset {
    /// The skeleton of an animated figure (`None`: static).
    pub skeleton: Option<Arc<AnimSkeleton>>,
    /// Body parts, then held parts.
    pub parts: Vec<PartAsset>,
    /// Holding locations of the held items, on the skeleton.
    pub attach: Option<Arc<AttachTable>>,
    /// Clips (empty for a static figure).
    pub clips: FigureClips,
    /// The bind-space deformation the skinned parts were built with (`appearance.md`
    /// §6.4); `None` when the figure has none or it changes nothing.
    pub deformation: Option<Arc<BindDeformation>>,
    /// The shape inputs it was built from (held clips, body shape, slider values).
    pub shape: FigureShape,
    /// Whole-body scale per axis (`appearance.md` §6.1).
    pub model_scale: [f32; 3],
    /// Every holding location and `attach_map` bone of the figure's setup, for the particle
    /// effects attached to it (`particle-runtime.md` §6.1); `None` when the setup does not
    /// read.
    pub points: Option<Arc<crate::walker::effects::data::AttachPoints>>,
    /// The figure's `Render_LODClass` (`walker.md` §11.4.3, W22): the first found in the
    /// placement's instance set, then along the 0x47 entity chain from the figure's entity;
    /// `None` when no level holds it (the component's initial values apply, `mesh.md` §8.5).
    pub lod_class: Option<u32>,
    /// The figure's `Entity_Camera_Lookat_Height` *H* in metres, before any scale
    /// (`walker.md` §11.7 step 1): the first value found in the placement's instance set,
    /// then along the 0x47 entity chain; `None` when no level holds it.
    pub look_at_height: Option<f32>,
    /// What the species tables' look gave the figure (`appearance.md` §3.5): composed
    /// faces and material edits, for tests and diagnostics.
    pub face: FaceSummary,
    /// A character-screen figure's live scripts (`character-screens.md` §2.1.4,
    /// [`super::live`]); `None` for every other figure, and when the install lacks what
    /// the runtime needs (the static resolution then applies).
    pub live: Option<Arc<super::live::LivePackage>>,
    /// The figure's script tables and stored values, as its clips were chosen with
    /// (`script-table.md` §8.1): what the combat scripts of `combat-visuals.md` run on.
    pub scripts: Option<Arc<FigureScripts>>,
    /// Problems, for the log.
    pub notes: Vec<String>,
    /// Worker time.
    pub build_ms: f64,
}

/// The script state of a built figure: its tables (class, instance, additional; their
/// closures are searched in this order) and its stored property values.
#[derive(Debug, Clone, Default)]
pub struct FigureScripts {
    /// The tables.
    pub tables: Vec<Did>,
    /// The stored values.
    pub properties: tessera_formats::script_table::resolve::PropertyState,
    /// The start-up script keys after `Startup` (`ScriptTable_StartupScriptNames`, with
    /// `Chargen` on a character-screen figure; `script-table.md` §8.1).
    pub startup_scripts: Vec<u32>,
}

/// The figure's look in numbers (`appearance.md` §3.5): which compositors were attached,
/// which of them composed an image (with the image's identity, a digest of record,
/// layer selection and colours), and the material edits registered.
#[derive(Debug, Clone, Default, PartialEq, Eq)]
pub struct FaceSummary {
    /// (compositor name, identity of its composed image; `None` when it composed none),
    /// attach order.
    pub compositors: Vec<(u32, Option<u64>)>,
    /// Material edits registered at the end (every parameter group).
    pub edits: usize,
    /// The colour values of the edits of the dye-colour parameter 2003 (stored order
    /// B, G, R, A), start order: the hair and skin tints (`appearance.md` §3.5).
    pub dye_colours: Vec<[u8; 4]>,
}

impl FaceSummary {
    /// The summary of `look`.
    #[must_use]
    pub fn of(look: &FigureLook) -> Self {
        Self {
            compositors: look
                .compositors
                .iter()
                .map(|c| (c.name, c.image.as_ref().map(|_| c.identity)))
                .collect(),
            edits: look.edits.len(),
            dye_colours: look
                .edits
                .iter()
                .filter(|e| e.material_property == DYE_COLOUR_PARAMETER)
                .map(|e| e.color_value)
                .collect(),
        }
    }

    /// Whether a face was composed.
    #[must_use]
    pub fn composed(&self) -> bool {
        self.compositors.iter().any(|(_, i)| i.is_some())
    }
}

/// The dye-colour material parameter the hair and skin edits set (`appearance.md` §3.5,
/// `material.md` §12).
pub const DYE_COLOUR_PARAMETER: u32 = 2003;

impl FigureAsset {
    /// Whether the figure is animated.
    #[must_use]
    pub fn animated(&self) -> bool {
        self.skeleton.is_some()
    }

    /// GPU bytes of every part.
    #[must_use]
    pub fn gpu_bytes(&self) -> u64 {
        self.parts.iter().map(|p| p.gpu_bytes).sum()
    }
}

/// Most clip DIDs a frame-thread miss queues for the workers at once ([`ClipStore::take_pending`]).
pub const MAX_PENDING_CLIPS: usize = 256;

/// Decoded clips by DID, shared by every figure job, with their baked frames.
///
/// **Frame thread** (ARCHITECTURE §21: no blocking I/O on the main thread). Once
/// [`ClipStore::mark_frame_thread`] has named the walker's frame thread, a clip that is not
/// decoded yet is never read there: [`ClipStore::bind`] fails for that call (the asking
/// node plays nothing, a placeholder for the frame), the miss is counted
/// ([`ClipStore::frame_thread_misses`]) and the DID queued for the workers
/// ([`ClipStore::take_pending`], [`ClipStore::prefetch`]). Worker threads read and decode
/// as before.
#[derive(Debug)]
pub struct ClipStore {
    clips: Mutex<HashMap<u32, Option<Arc<Clip>>>>,
    bake: BakeCache,
    frame_thread: std::sync::OnceLock<std::thread::ThreadId>,
    misses: std::sync::atomic::AtomicU64,
    pending: Mutex<Vec<u32>>,
    /// The first clip DIDs frame-thread lookups missed (diagnostics, at most
    /// [`MAX_MISSED_CLIPS`]).
    missed: Mutex<Vec<u32>>,
}

/// Most missed clip DIDs remembered ([`ClipStore::frame_thread_missed`]).
pub const MAX_MISSED_CLIPS: usize = 32;

impl Default for ClipStore {
    fn default() -> Self {
        Self {
            clips: Mutex::new(HashMap::new()),
            bake: BakeCache::new(BAKE_BUDGET, MAX_BAKED_CLIP),
            frame_thread: std::sync::OnceLock::new(),
            misses: std::sync::atomic::AtomicU64::new(0),
            pending: Mutex::new(Vec::new()),
            missed: Mutex::new(Vec::new()),
        }
    }
}

impl ClipStore {
    /// Names the calling thread the frame thread (once; later calls change nothing): clips
    /// are no longer read there (type docs).
    pub fn mark_frame_thread(&self) {
        let _ = self.frame_thread.set(std::thread::current().id());
    }

    /// Whether the calling thread is the frame thread named by [`Self::mark_frame_thread`].
    #[must_use]
    pub fn on_frame_thread(&self) -> bool {
        self.frame_thread
            .get()
            .is_some_and(|t| *t == std::thread::current().id())
    }

    /// Clips asked for on the frame thread before they were decoded (each was then not read
    /// there, type docs).
    #[must_use]
    pub fn frame_thread_misses(&self) -> u64 {
        self.misses.load(std::sync::atomic::Ordering::Relaxed)
    }

    /// The first distinct clip DIDs frame-thread lookups missed (at most
    /// [`MAX_MISSED_CLIPS`]; diagnostics).
    #[must_use]
    pub fn frame_thread_missed(&self) -> Vec<u32> {
        self.missed
            .lock()
            .unwrap_or_else(PoisonError::into_inner)
            .clone()
    }

    /// The clip DIDs frame-thread misses queued since the last call (for
    /// [`Self::prefetch`] on the workers).
    pub fn take_pending(&self) -> Vec<u32> {
        std::mem::take(&mut *self.pending.lock().unwrap_or_else(PoisonError::into_inner))
    }

    /// Whether clip `did` has been decoded (or failed for good) already.
    #[must_use]
    pub fn has(&self, did: u32) -> bool {
        self.clips
            .lock()
            .unwrap_or_else(PoisonError::into_inner)
            .contains_key(&did)
    }

    /// Reads, decodes and bakes clip `did` when it is not in the store yet (blocking: a
    /// worker's job). Returns whether the clip decoded.
    pub fn prefetch(&self, set: &ArchiveSet, did: u32) -> bool {
        let Some(clip) = self.read_clip(set, did) else {
            return false;
        };
        let _ = self.bake.get_or_bake(u64::from(did), &clip);
        true
    }

    /// The decoded clip `did` (read and decoded once; failures remembered). On the frame
    /// thread a clip not decoded yet is not read (type docs).
    fn clip(&self, set: &ArchiveSet, did: u32) -> Option<Arc<Clip>> {
        if let Some(c) = self
            .clips
            .lock()
            .unwrap_or_else(PoisonError::into_inner)
            .get(&did)
        {
            return c.clone();
        }
        if self.on_frame_thread() {
            self.misses
                .fetch_add(1, std::sync::atomic::Ordering::Relaxed);
            let mut p = self.pending.lock().unwrap_or_else(PoisonError::into_inner);
            if p.len() < MAX_PENDING_CLIPS && !p.contains(&did) {
                p.push(did);
            }
            drop(p);
            let mut m = self.missed.lock().unwrap_or_else(PoisonError::into_inner);
            if m.len() < MAX_MISSED_CLIPS && !m.contains(&did) {
                m.push(did);
            }
            tracing::debug!(
                clip = format_args!("{did:#010x}"),
                "clip not decoded yet; not read on the frame thread"
            );
            return None;
        }
        self.read_clip(set, did)
    }

    /// Reads and decodes `did` into the store (any thread).
    fn read_clip(&self, set: &ArchiveSet, did: u32) -> Option<Arc<Clip>> {
        if let Some(c) = self
            .clips
            .lock()
            .unwrap_or_else(PoisonError::into_inner)
            .get(&did)
        {
            return c.clone();
        }
        let decoded = set
            .read(did)
            .ok()
            .and_then(|b| Clip::parse(&b).ok())
            .map(Arc::new);
        let mut m = self.clips.lock().unwrap_or_else(PoisonError::into_inner);
        if m.len() >= MAX_CACHED_CLIPS {
            m.clear();
        }
        m.insert(did, decoded.clone());
        decoded
    }

    /// `did` bound to `skeleton`, with baked frames when they fit the bake budget.
    pub(crate) fn bind(
        &self,
        set: &ArchiveSet,
        did: u32,
        skeleton: &Arc<AnimSkeleton>,
    ) -> Result<Arc<BoundClip>, String> {
        let clip = self
            .clip(set, did)
            .ok_or_else(|| format!("clip {did:#010x} missing, undecodable or not loaded yet"))?;
        let bound = BoundClip::bind(Arc::clone(&clip), Arc::clone(skeleton))
            .map_err(|e| format!("clip {did:#010x}: {e}"))?;
        let bound = match self.bake.get_or_bake(u64::from(did), &clip) {
            Some(baked) => bound.clone().with_baked(baked).unwrap_or(bound),
            None => bound,
        };
        Ok(Arc::new(bound))
    }
}

/// Everything a figure job reads (shared, immutable).
#[derive(Debug)]
pub struct FigureContext {
    /// The archives.
    pub set: Arc<ArchiveSet>,
    /// The character vocabulary (holds the game data).
    pub catalog: Arc<CharacterCatalog>,
    /// Materials and textures (the object layer's library).
    pub materials: Arc<MaterialLibrary<Arc<ArchiveSet>>>,
    /// Renderer mesh settings.
    pub mesh_config: MeshConfig,
    /// Decoded clips.
    pub clips: ClipStore,
    /// Composed faces and their decoded sources, shared by every figure (`appearance.md`
    /// §3.5; figures with the same face compose it once).
    pub looks: LookCache,
    /// Tessera's answers for the options a figure's property lookup reads first
    /// (`appearance.md` §3.3: the updated-avatar option; §3.5: the face-compositing
    /// option).
    pub assembly: AssemblyOptions,
}

impl FigureContext {
    /// A context over `set` with its own material library (tests and tools; the walker
    /// shares the object layer's).
    #[must_use]
    pub fn standalone(
        set: Arc<ArchiveSet>,
        catalog: Arc<CharacterCatalog>,
        mesh_config: MeshConfig,
    ) -> Self {
        let materials = Arc::new(MaterialLibrary::with_config(
            Arc::clone(&set),
            LibraryConfig {
                max_texture_dimension: 4096,
                ..LibraryConfig::default()
            },
        ));
        Self {
            set,
            catalog,
            materials,
            mesh_config,
            clips: ClipStore::default(),
            looks: LookCache::default(),
            assembly: AssemblyOptions::default(),
        }
    }
}

/// The skeleton embedded in a setup's tagfile.
fn skeleton_of(tagfile: &[u8]) -> Result<Arc<AnimSkeleton>, String> {
    if tagfile.is_empty() {
        return Err("the setup has no skeleton".into());
    }
    let h = tessera_havok::Skeleton::parse(tagfile).map_err(|e| e.to_string())?;
    AnimSkeleton::from_havok(&h)
        .map(Arc::new)
        .map_err(|e| e.to_string())
}

/// The `ContainerSlot` bit of equipment slot key `key` (key *k* is bit *k* − 1,
/// `property-system.md` §7.3); `None` outside 1–64.
#[must_use]
pub fn slot_key_bits(key: u8) -> Option<u64> {
    (1..=64).contains(&key).then(|| 1u64 << (key - 1))
}

/// Builds the figure of `req` (module docs). `cancelled` is polled between parts.
///
/// # Errors
///
/// [`FigureError`] when the body does not resolve or the job is cancelled.
pub fn build_figure(
    ctx: &FigureContext,
    req: &FigureRequest,
    cancelled: &dyn Fn() -> bool,
) -> Result<FigureAsset, FigureError> {
    build_figure_with_look(ctx, req, &LookChoices::default(), cancelled)
}

/// [`build_figure`] with the look choices of a playable body (styles, slider string).
///
/// # Errors
///
/// As [`build_figure`].
pub fn build_figure_with_look(
    ctx: &FigureContext,
    req: &FigureRequest,
    choices: &LookChoices,
    cancelled: &dyn Fn() -> bool,
) -> Result<FigureAsset, FigureError> {
    let started = Instant::now();
    let cat = &ctx.catalog;
    let vocab = cat
        .assembly
        .clone()
        .ok_or_else(|| FigureError::Unavailable("no assembly vocabulary".into()))?;
    let source =
        GameDataSource::new(&cat.data).map_err(|e| FigureError::Unavailable(e.to_string()))?;
    let mut asm = CharacterAssembler::new(source, vocab).with_options(ctx.assembly);
    let spec = match req.body {
        Body::Character { species, sex } => {
            let mut spec = CharacterSpec::new(species, sex);
            spec.avatar_height = req.avatar_height;
            spec.bone_scale = req.bone_scale;
            spec.styles.clone_from(&choices.styles);
            spec.slider_values.clone_from(&choices.slider_values);
            spec.appearance_values
                .clone_from(&choices.appearance_values);
            for e in req.equipment.iter().take(super::MAX_EQUIPMENT) {
                let mut item = EquippedItem::new(Did(e.item.0)).with_state(if e.stowed {
                    WieldState::Sheathed
                } else {
                    WieldState::Wielded
                });
                item.dye = e.dye;
                item.secondary_dye = e.secondary_dye;
                // The equipment slot key that holds the item (decision 0025 D3) as its
                // `ContainerSlot` bit (key *k* is bit *k* − 1); without one the assembler
                // applies the hand rule of `character-panels.md` §7.
                if let Some(bits) = e.slot.and_then(slot_key_bits) {
                    item = item.in_slot(bits);
                }
                spec.items.push(item);
            }
            Some(spec)
        }
        Body::Object(_) => None,
        _ => return Err(FigureError::UnsupportedBody),
    };
    let mut assemble = |scripts: &[ScriptAppearance]| match (&spec, req.body) {
        (Some(spec), _) => asm.assemble_character_with(spec, scripts),
        (None, Body::Object(d)) => {
            asm.assemble_object_with(Did(d.0), req.instance.as_deref(), scripts)
        }
        (None, _) => Err(tessera_mesh::assembly::AssemblyError::Body {
            start: Did(0),
            reason: "unsupported body".into(),
        }),
    };
    let mut c = assemble(&[])?;
    if cancelled() {
        return Err(FigureError::Cancelled);
    }
    let mut notes = Vec::new();
    if !c.diagnostics.is_empty() {
        notes.push(format!(
            "{} assembly diagnostics (first: {})",
            c.diagnostics.len() + c.dropped_diagnostics,
            c.diagnostics[0].category.name()
        ));
    }
    let skinned_body = c.parts.iter().any(|p| !p.hidden && p.skinned);
    let skeleton = if req.static_only || !skinned_body {
        None
    } else {
        match skeleton_of(&c.skeleton.tagfile) {
            Ok(s) if s.bone_count() <= tessera_render::MAX_PALETTE_BONES => Some(s),
            Ok(s) => {
                notes.push(format!("{} bones exceed the palette", s.bone_count()));
                None
            }
            Err(e) => {
                notes.push(format!("no skeleton: {e}"));
                None
            }
        }
    };
    let bones = skeleton.as_ref().map(|s| s.bone_count());
    let (clips, shape, scripts, look, figure_scripts, live) = match &skeleton {
        Some(s) => select_clips(ctx, &c, s, req, &mut notes),
        None => (
            FigureClips::default(),
            FigureShape::default(),
            Vec::new(),
            FigureLook::default(),
            None,
            None,
        ),
    };
    notes.extend(look.notes.iter().take(4).map(|n| format!("look: {n}")));
    let face = FaceSummary::of(&look);
    // The Appearance nodes the start-up scripts ran (`appearance.md` §7.4): assembled
    // after the lists, on the same body and setup.
    if !scripts.is_empty() {
        c = assemble(&scripts)?;
        if cancelled() {
            return Err(FigureError::Cancelled);
        }
    }
    notes.extend(shape.notes.iter().take(4).cloned());
    // Held clips and the bind-space deformation (`appearance.md` §6.4, `script-table.md`
    // §6.7, §6.8), by the path the explorer shares.
    let mut clips = clips;
    let mut deformation = None;
    if let Some(s) = &skeleton {
        let bound = bind_shape(&shape, s, |d| ctx.clips.bind(&ctx.set, d.0, s));
        notes.extend(bound.notes);
        deformation = bound.deformation;
        // Kept in every motion state, never faded (script-table.md §6.8).
        clips.held = bound
            .held
            .into_iter()
            .map(|h| ClipPlay {
                did: h.held.clip.0,
                clip: h.clip,
                rate: 0.0,
                speed: 0.0,
                layer: h.held.layer,
                track_weights: h.held.use_bone_opacity,
                held: Some(h.clip_fraction),
                once: None,
            })
            .collect();
    }

    // A live screen figure's appearance variants (`super::live`): variant 0 holds the
    // pieces of the stage it is built with; every other stage value whose pieces differ
    // gets its own variant, and each part is tagged with the variants it belongs to.
    let mut variant_sets: Vec<Vec<ScriptAppearance>> = vec![scripts.clone()];
    let mut stage_variants: Vec<(u32, u8)> = Vec::new();
    if let Some(cap) = &live {
        for (stage, set) in &cap.stage_pieces {
            let index = match variant_sets.iter().position(|v| v == set) {
                Some(i) => i,
                None if variant_sets.len() < super::live::MAX_VARIANTS => {
                    variant_sets.push(set.clone());
                    variant_sets.len() - 1
                }
                None => {
                    notes.push(format!(
                        "stage {stage}: more than {} piece sets",
                        super::live::MAX_VARIANTS
                    ));
                    0
                }
            };
            stage_variants.push((*stage, u8::try_from(index).unwrap_or(0)));
        }
    }
    let first_mask = if live.is_some() { 1 } else { ALL_VARIANTS };
    let mut parts = Vec::new();
    // (mesh, slot, materials) of the body parts built, for the variants' sharing.
    let mut built: Vec<(Did, u32, Vec<MaterialLayer>)> = Vec::new();
    let mut variant_parts: Vec<(u8, Vec<tessera_mesh::assembly::AssembledPart>)> =
        vec![(0, c.parts.clone())];
    for (i, set) in variant_sets.iter().enumerate().skip(1) {
        if cancelled() {
            return Err(FigureError::Cancelled);
        }
        match assemble(set) {
            Ok(v) => variant_parts.push((u8::try_from(i).unwrap_or(0), v.parts)),
            Err(e) => notes.push(format!("variant {i}: {e}")),
        }
    }
    for (variant, list) in &variant_parts {
        let bit = if live.is_some() {
            1u8 << (*variant).min(7)
        } else {
            ALL_VARIANTS
        };
        for p in list.iter().filter(|p| !p.hidden) {
            if let Some(j) = built
                .iter()
                .position(|(m, slot, mats)| *m == p.mesh && *slot == p.slot && *mats == p.materials)
            {
                if let Some(part) = parts.get_mut(j) {
                    let part: &mut PartAsset = part;
                    part.variants |= bit;
                }
                continue;
            }
            if parts.len() >= MAX_FIGURE_PARTS {
                notes.push(format!("more than {MAX_FIGURE_PARTS} parts"));
                break;
            }
            if cancelled() {
                return Err(FigureError::Cancelled);
            }
            match build_part(
                ctx,
                p.mesh,
                bones.filter(|_| p.skinned),
                (&p.materials, Some((p.slot, &look))),
                deformation.as_ref(),
            ) {
                Ok(mut part) => {
                    part.held = None;
                    part.variants = if *variant == 0 { first_mask } else { bit };
                    built.push((p.mesh, p.slot, p.materials.clone()));
                    parts.push(part);
                }
                Err(e) => notes.push(format!("part {}: {e}", p.mesh)),
            }
        }
    }
    // Held items: rigid parts at the wearer's holding locations (skeleton-anim.md §8.4),
    // each at its placement (`appearance.md` §5.6) and scaled relative to the figure
    // (§6.1). One attach point per held part, keyed by its index.
    let mut attach_points = Vec::new();
    if skeleton.is_some() {
        for (index, h) in c.held.iter().enumerate() {
            let key = u32::try_from(index).unwrap_or(u32::MAX);
            let mut any = false;
            for &mesh in &h.parts {
                if parts.len() >= MAX_FIGURE_PARTS {
                    break;
                }
                if cancelled() {
                    return Err(FigureError::Cancelled);
                }
                match build_part(ctx, mesh, None, (&[], None), None) {
                    Ok(mut part) => {
                        part.held = Some(key);
                        parts.push(part);
                        any = true;
                    }
                    Err(e) => notes.push(format!("held part {mesh}: {e}")),
                }
            }
            if any {
                let mut frame = h.placement.applied_to(&h.frame);
                let rel = c.held_scale(h);
                for (s, r) in frame.scale.iter_mut().zip(rel) {
                    *s *= r;
                }
                attach_points.push(AttachPointDesc {
                    key,
                    bone: match h.anchor {
                        Anchor::Bone(b) => i32::try_from(b).unwrap_or(-1),
                        Anchor::Origin => -1,
                    },
                    frame: AttachFrame {
                        position: frame.position,
                        scale: frame.scale,
                        rotation_wxyz: [
                            frame.rotation.w,
                            frame.rotation.x,
                            frame.rotation.y,
                            frame.rotation.z,
                        ],
                    },
                });
            }
        }
    } else if !c.held.is_empty() {
        notes.push("held items need an animated wearer; not drawn".into());
    }
    // The setup: the figure's attachment points, and the holding locations of a screen
    // figure's props.
    let setup_record = crate::audio::loader::fetch(&ctx.set, c.skeleton.setup.0)
        .ok()
        .flatten();
    let setup = setup_record.as_ref().and_then(|b| {
        <tessera_formats::setup::Setup as tessera_formats::Record>::parse_expecting(
            b,
            c.skeleton.setup,
        )
        .ok()
    });
    let points = setup
        .as_ref()
        .zip(setup_record.as_ref())
        .map(|(s, b)| Arc::new(crate::walker::effects::data::AttachPoints::from_setup(s, b)));
    // A live screen figure's props (`super::live`): each prop entity's parts at its holding
    // location, hidden until its MeshFX node runs.
    let mut props = Vec::new();
    let mut prop_notes: Vec<String> = Vec::new();
    if let (Some(cap), Some(setup), Some(_)) = (&live, &setup, &skeleton) {
        for prop in &cap.props {
            if parts.len() >= MAX_FIGURE_PARTS {
                prop_notes.push("no room for the props".into());
                break;
            }
            if cancelled() {
                return Err(FigureError::Cancelled);
            }
            let Some(h) = prop_anchor(setup, prop) else {
                prop_notes.push(format!(
                    "prop {}: no holding location {} or bone type {}",
                    prop.entity, prop.location, prop.bone_type
                ));
                continue;
            };
            let held = match asm.assemble_object_with(prop.entity, None, &[]) {
                Ok(p) => p,
                Err(e) => {
                    prop_notes.push(format!("prop {}: {e}", prop.entity));
                    continue;
                }
            };
            let key = super::live::PROP_KEY_BASE + u32::try_from(props.len()).unwrap_or(0);
            let mut any = false;
            for p in held.parts.iter().filter(|p| !p.hidden) {
                if parts.len() >= MAX_FIGURE_PARTS {
                    break;
                }
                match build_part(ctx, p.mesh, None, (&p.materials, None), None) {
                    Ok(mut part) => {
                        part.held = Some(key);
                        parts.push(part);
                        any = true;
                    }
                    Err(e) => prop_notes.push(format!("prop part {}: {e}", p.mesh)),
                }
            }
            if !any {
                prop_notes.push(format!("prop {}: no part built", prop.entity));
                continue;
            }
            let placement = tessera_mesh::assembly::HeldPlacement {
                offset: prop.offset,
                rotation: prop.rotation,
                scale: prop.scale,
            };
            let mut frame = placement.applied_to(&h.frame);
            // The prop's own model scale relative to the holder's (`appearance.md` §6.1).
            let (own, holder) = (held.model_scale(), c.model_scale());
            for ((s, o), w) in frame.scale.iter_mut().zip(own).zip(holder) {
                let r = o / w;
                if r.is_finite() && r > 0.0 {
                    *s *= r;
                }
            }
            attach_points.push(AttachPointDesc {
                key,
                bone: match h.anchor {
                    Anchor::Bone(b) => i32::try_from(b).unwrap_or(-1),
                    Anchor::Origin => -1,
                },
                frame: AttachFrame {
                    position: frame.position,
                    scale: frame.scale,
                    rotation_wxyz: [
                        frame.rotation.w,
                        frame.rotation.x,
                        frame.rotation.y,
                        frame.rotation.z,
                    ],
                },
            });
            props.push(*prop);
        }
    }
    let attach = match &skeleton {
        Some(s) if !attach_points.is_empty() => {
            let (t, errors) = AttachTable::new_skipping_invalid(&attach_points, s.bone_count());
            if let Some(e) = errors.first() {
                notes.push(format!("holding location skipped: {e}"));
            }
            Some(Arc::new(t))
        }
        _ => None,
    };
    // `model_scale` applies the client's limit (§6.1); also refuse absurdly large scales.
    let model_scale = c.model_scale();
    let model_scale = if model_scale.iter().all(|s| *s < 100.0) {
        model_scale
    } else {
        notes.push("unusable model scale; drawn at 1".into());
        [1.0; 3]
    };
    let lod_class = figure_lod_class(ctx, req.instance.as_deref(), &c.chain);
    let look_at_height = figure_look_at_height(ctx, req.instance.as_deref(), &c.chain);
    let live = live.map(|cap| {
        let mut package = cap.package;
        package.props = props;
        package.stage_variants = stage_variants;
        package.held_clips = clips.held.iter().map(|h| h.did).collect();
        prop_notes.truncate(super::live::MAX_PROPS);
        notes.extend(prop_notes.iter().take(4).cloned());
        package.prop_notes = prop_notes;
        Arc::new(package)
    });
    Ok(FigureAsset {
        skeleton,
        parts,
        attach,
        clips,
        deformation: deformation.map(Arc::new),
        shape,
        model_scale,
        points,
        lod_class,
        look_at_height,
        face,
        scripts: figure_scripts,
        live,
        notes,
        build_ms: started.elapsed().as_secs_f64() * 1e3,
    })
}

/// The holding-location key an attached MeshFX node with neither `HoldingLocation` nor
/// `BoneType` uses (`script-table.md` §5.2: key 6, `origin` in the base mapper).
pub const ORIGIN_LOCATION_KEY: u32 = 6;

/// How a prop's anchor is chosen (`script-table.md` §5.2 "Where a MeshFX node attaches its
/// entity", T47).
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum PropAnchorRule {
    /// That holding location of the holder (list A).
    Location(u32),
    /// The holder's bone of that bone type.
    Bone(u32),
    /// The holding location with key [`ORIGIN_LOCATION_KEY`]; the model origin when list A
    /// has none (the Tessera rule of the §5.2 OPEN).
    OriginKey,
    /// The holder's model origin.
    Origin,
}

/// The anchor rule of a MeshFX node (`script-table.md` §5.2): an attached node
/// (`AttachToParent` 1) takes its `HoldingLocation` when not 0, else its `BoneType` when not
/// 0, else the location with key 6; an unattached one its `BoneType` when not 0, else its
/// `HoldingLocation` when not 0, else the model origin.
#[must_use]
pub fn prop_anchor_rule(attached: bool, location: u32, bone_type: u32) -> PropAnchorRule {
    match (attached, location, bone_type) {
        (true, l, _) if l != 0 => PropAnchorRule::Location(l),
        (true, 0, b) if b != 0 => PropAnchorRule::Bone(b),
        (true, _, _) => PropAnchorRule::OriginKey,
        (false, _, b) if b != 0 => PropAnchorRule::Bone(b),
        (false, l, _) if l != 0 => PropAnchorRule::Location(l),
        (false, _, _) => PropAnchorRule::Origin,
    }
}

/// Where a screen figure's prop is attached ([`prop_anchor_rule`], `script-table.md` §5.2,
/// `character-screens.md` §2.1.4 answer): a bone at an identity frame, a holding location
/// at its frame, or the holder's model origin. `None` when the setup lacks the named
/// location or bone (the node shows nothing). The placement frame and scale are applied by
/// the caller (`appearance.md` §5.7). An unattached node's anchor is read once at placement
/// (`script-table.md` §5.2 answer: its prop's world transform is the one at placement and
/// stays fixed; the Tessera rule lets it be a part of the figure on that condition). The
/// prop parts built here are attach-table entries, so they follow the anchor every frame;
/// freezing the transform of an unattached prop at its placement is the draw path's part
/// ([`super::live::ScreenProp::attached`] tells the two apart) and is not done yet.
fn prop_anchor(
    setup: &tessera_formats::setup::Setup,
    prop: &super::live::ScreenProp,
) -> Option<tessera_formats::setup::HoldingLocation> {
    use tessera_formats::setup::{Anchor as SetupAnchor, Frame, HoldingLocation, Quaternion};
    let identity = Frame {
        position: [0.0; 3],
        scale: [1.0; 3],
        rotation: Quaternion {
            w: 1.0,
            x: 0.0,
            y: 0.0,
            z: 0.0,
        },
    };
    let origin = |key: u32| HoldingLocation {
        key,
        anchor: SetupAnchor::Origin,
        frame: identity,
    };
    match prop_anchor_rule(prop.attached, prop.location, prop.bone_type) {
        PropAnchorRule::Location(l) => setup.holding_locations().find(|h| h.key == l),
        PropAnchorRule::Bone(b) => {
            let a = setup.attachments().find(|a| a.bone_type == b)?;
            Some(HoldingLocation {
                key: prop.location,
                anchor: SetupAnchor::Bone(a.bone),
                frame: identity,
            })
        }
        PropAnchorRule::OriginKey => Some(
            setup
                .holding_locations()
                .find(|h| h.key == ORIGIN_LOCATION_KEY)
                .unwrap_or_else(|| origin(ORIGIN_LOCATION_KEY)),
        ),
        PropAnchorRule::Origin => Some(origin(0)),
    }
}

/// Most 0x47 levels of a figure's entity chain read for one of its properties.
const MAX_LOD_CLASS_LEVELS: usize = 16;

/// The value of property `name` for a figure: the first value found in the placement's
/// instance set, then in the property sets of the 0x47 levels of `chain` (nearest first);
/// `None` when the dictionary lacks the name or no level holds it.
fn figure_property(
    ctx: &FigureContext,
    instance: Option<&PropertySet>,
    chain: &[Did],
    name: &str,
) -> Option<PropertyValue> {
    let data = &ctx.catalog.data;
    let registry = data.registry().ok()?;
    let pid = registry.pid(name)?;
    if let Some(v) = instance.and_then(|s| s.get(pid)) {
        return Some(v.clone());
    }
    for did in chain
        .iter()
        .filter(|d| d.prefix() == 0x47)
        .take(MAX_LOD_CLASS_LEVELS)
    {
        let Ok(Some(bytes)) = data.read(*did) else {
            continue;
        };
        let Ok(e) = tessera_formats::setup::EntityDescriptor::parse(&bytes, registry.dictionary())
        else {
            continue;
        };
        if let Some(v) = e.properties.get(pid) {
            return Some(v.clone());
        }
    }
    None
}

/// The `Render_LODClass` of a figure (`walker.md` §11.4.3, the Tessera Decision W22 there):
/// the first value found in the placement's instance set, then in the property sets of the
/// 0x47 levels of `chain` (nearest first). An enum value gives its key, a bit-like value its
/// low 32 bits; anything else, or nothing found, gives `None`.
fn figure_lod_class(
    ctx: &FigureContext,
    instance: Option<&PropertySet>,
    chain: &[Did],
) -> Option<u32> {
    match figure_property(ctx, instance, chain, "Render_LODClass")? {
        PropertyValue::Enum(k) => Some(k),
        other => other.as_bits().and_then(|b| u32::try_from(b).ok()),
    }
}

/// The `Entity_Camera_Lookat_Height` of a figure (`walker.md` §11.7 step 1), found like
/// [`figure_lod_class`]; a value that is not a finite number of at least 0 gives `None`.
fn figure_look_at_height(
    ctx: &FigureContext,
    instance: Option<&PropertySet>,
    chain: &[Did],
) -> Option<f32> {
    let v = figure_property(ctx, instance, chain, "Entity_Camera_Lookat_Height")?;
    #[allow(clippy::cast_possible_truncation)]
    let h = v.as_f64()? as f32;
    (h.is_finite() && h >= 0.0).then_some(h.min(1_000.0))
}

/// Decodes, flattens and prepares one part mesh; skinned when `bones` is given, its
/// skinned vertices deformed in bind space by `deform` (`appearance.md` §6.4 step 4). The
/// materials take the part's appearance layers and, for a body part, its slot and the
/// figure's look (`appearance.md` §3.5).
fn build_part(
    ctx: &FigureContext,
    mesh_did: Did,
    bones: Option<usize>,
    (layers, look): (&[MaterialLayer], Option<(u32, &FigureLook)>),
    deform: Option<&BindDeformation>,
) -> Result<PartAsset, String> {
    let (located, bytes) = ctx
        .set
        .read_with_entry(mesh_did.0)
        .map_err(|e| e.to_string())?;
    let mesh = Mesh::decode_expecting(
        &bytes,
        mesh_did,
        Some(located.entry.stamp),
        &DecodeOptions::render_only(),
    )
    .map_err(|e| e.to_string())?;
    drop(bytes);
    let level = display_level(&mesh).ok_or("the mesh has no levels")?;
    let limits = GeometryLimits::default();
    let (geo, skinned) = match bones {
        Some(b) => {
            let s = mesh
                .skinned_level(level, &limits, Some(b))
                .map_err(|e| e.to_string())?;
            let skinned = s.is_skinned();
            (s.geometry, skinned)
        }
        None => (
            mesh.level_geometry(level, &limits)
                .map_err(|e| e.to_string())?,
            false,
        ),
    };
    drop(mesh);
    let plans = geo
        .parts
        .iter()
        .map(|p| {
            p.surface.map_or(
                MaterialPlan::unresolved(None, tessera_material::FallbackReason::NoSurface),
                |s| plan_for(ctx, s, layers, look),
            )
        })
        .collect();
    let mut data = level_mesh_data(&geo, format!("figure part {mesh_did}"));
    let prepared = match (skinned, bones) {
        (true, Some(b)) => {
            let (skin, _rigid) = mesh_skin(&geo, b)?;
            if let Some(d) = deform {
                d.deform(
                    &mut data.positions,
                    data.normals.as_deref_mut(),
                    data.tangents.as_deref_mut(),
                    &skin.joints,
                    &skin.weights,
                );
            }
            data.prepare_skinned(&skin, &ctx.mesh_config)
        }
        _ => data.prepare(&ctx.mesh_config),
    }
    .map_err(|e| e.to_string())?;
    Ok(PartAsset {
        gpu_bytes: prepared.gpu_bytes(),
        mesh: Mutex::new(Some(prepared)),
        plans,
        skinned,
        held: None,
        variants: ALL_VARIANTS,
    })
}

/// Most material groups layered onto one surface (a hostile group list stays bounded;
/// on overflow the last ones in group-list order are kept, `material.md` §18).
pub const MAX_SURFACE_GROUPS: usize = 16;

/// The plan of `surface` with the figure's look (`material.md` §9.3 rules 1–4,
/// `appearance.md` §3.5, §4.10): the part's groups whose binding key matches the surface's
/// ([`tessera_material::group_binding_matches`]) in list order, a shader-only group
/// included and the last override winning ([`Layering::from_groups`]); after the surface's
/// own materials, the composed image of the part's slot, then the group materials, then the
/// look edits ([`MaterialLibrary::resolve_look`]).
fn plan_for(
    ctx: &FigureContext,
    surface: Did,
    layers: &[MaterialLayer],
    look: Option<(u32, &FigureLook)>,
) -> MaterialPlan {
    let lib = &ctx.materials;
    let key = lib.surface_record(surface).ok().map(|r| r.binding_key);
    let layering = key.and_then(|k| {
        Layering::from_groups(
            layers
                .iter()
                .filter(|l| tessera_material::group_binding_matches(l.binding_key, k))
                .map(|l| (l.shader, l.material)),
            MAX_SURFACE_GROUPS,
        )
    });
    let (composed, edits) = match (look, key) {
        (Some((slot, look)), Some(key)) => look_inputs(lib, look, slot, key),
        _ => (None, Vec::new()),
    };
    let resolved = if layering.is_none() && composed.is_none() && edits.is_empty() {
        lib.resolve_surface(surface).map(|m| plan_material_dyed(&m))
    } else {
        lib.resolve_look(
            surface,
            &SurfaceLook {
                composed: composed.as_slice(),
                layer: layering.as_ref(),
                edits: &edits,
            },
        )
        .map(|m| plan_material_dyed(&m))
    };
    resolved.unwrap_or_else(|e| {
        tracing::debug!(%surface, "figure material unresolved: {e}");
        MaterialPlan::unresolved(
            Some(surface),
            tessera_material::diagnose::reason_of_resolve_error(&e),
        )
    })
}

/// What the figure's look adds to a surface with `key` of a part in `slot`
/// (`appearance.md` §3.5): the composed image of the last attached compositor that matches
/// (a run-time texture of the shared library, so the object layer's texture loader finds
/// it), and the material edits by descending priority.
fn look_inputs<S: tessera_material::RecordSource>(
    lib: &MaterialLibrary<S>,
    look: &FigureLook,
    slot: u32,
    key: u32,
) -> (Option<ParameterEdit>, Vec<ParameterEdit>) {
    let composed = look.compositor_for(slot, key).and_then(|f| {
        f.image
            .as_ref()
            .and_then(|i| composed_edit(lib.runtime_textures(), f.identity, i))
    });
    let edits = look
        .edits_for(slot, key)
        .into_iter()
        .filter_map(ParameterEdit::from_material_edit)
        .collect();
    (composed, edits)
}

/// A key-valued property of a character-screen figure (`CharGen_Action`, `Agent_Class`;
/// `character-screens.md` §2.1.4) as a value of the property's dictionary type: an enum
/// key, or an integer for an integer property; `None` when the dictionary lacks the name
/// or types it otherwise.
pub(crate) fn screen_property(
    registry: &tessera_props::PropertyRegistry,
    name: &str,
    key: u32,
) -> Option<(u32, PropertyValue)> {
    let pid = registry.pid(name)?;
    let v = match registry.property_type(pid)? {
        tessera_formats::PropertyType::Enum => PropertyValue::Enum(key),
        tessera_formats::PropertyType::Int => PropertyValue::Int(i32::try_from(key).ok()?),
        tessera_formats::PropertyType::Int64 => PropertyValue::Int64(i64::from(key)),
        _ => return None,
    };
    Some((pid, v))
}

/// A Random share or node probability at least this large counts as certain.
const CERTAIN: f32 = 0.999;

/// The one-shot clips a character-screen figure plays once at its start
/// (`character-screens.md` §2.1.4 "One-shot stage clips"): the static resolution lists
/// every clip the start-up scripts would start; of the played clips that do not loop
/// (`LoopForever` 0, rate at least [`MIN_RATE`], not held), those started for certain
/// (no Random share below [`CERTAIN`]), the first per data layer in start order. Gestures
/// chosen by Random nodes, and later steps of Sequences and Timers, need the running graph
/// and are not played (`character-screens.md` §2.1.4 implementer OPEN).
fn screen_one_shots(candidates: &[ClipCandidate]) -> Vec<ClipCandidate> {
    let mut out: Vec<ClipCandidate> = Vec::new();
    for c in candidates {
        if c.clip.is_null()
            || c.is_held()
            || c.loop_forever
            || c.playback_rate() < MIN_RATE
            || c.chance.is_nan()
            || c.chance < CERTAIN
            || out.iter().any(|o| o.layer == c.layer)
        {
            continue;
        }
        out.push(c.clone());
    }
    out
}

/// The start fraction *f*₀ of a played clip (`script-table.md` §6.10 step 1): `StartTime`,
/// or with `RandomizeStartTime` a fraction in [`StartTime`, `EndTime`] drawn from the
/// figure's seed and the clip (deterministic, so a figure keeps its look).
fn start_fraction(c: &ClipCandidate, seed: u64) -> f32 {
    if !c.randomize_start_time {
        return c.start_time;
    }
    let mut x = seed ^ (u64::from(c.clip.0) << 17) ^ u64::from(c.node_id);
    // SplitMix64 step.
    x = x.wrapping_add(0x9E37_79B9_7F4A_7C15);
    let mut z = x;
    z = (z ^ (z >> 30)).wrapping_mul(0xBF58_476D_1CE4_E5B9);
    z = (z ^ (z >> 27)).wrapping_mul(0x94D0_49BB_1331_11EB);
    z ^= z >> 31;
    let u = (z >> 40) as f32 / (1u64 << 24) as f32;
    let (a, b) = (c.start_time, c.end_time);
    if a.is_finite() && b.is_finite() {
        a + (b - a) * u
    } else {
        c.start_time
    }
}

/// The script vocabulary, the entity's scripts and the values set after its start-up
/// ([`prepare_figure_scripts`]).
pub(crate) type PreparedScripts = (
    tessera_formats::script_table::resolve::ScriptVocabulary,
    tessera_formats::script_table::resolve::EntityScripts,
    Vec<(u32, PropertyValue)>,
);

/// The script state of an assembled figure before clip selection (shared by the figure's
/// clip choice and the walker's mounted evaluation, `mounts.md` §14.5): the vocabulary, the
/// entity's scripts with its instance set, the options and appearance values of its lookup
/// (`appearance.md` §3.3, §3.5), the character-screen stage when it has one, and the values
/// set after its start-up. `None` (with a note) when the install lacks the script vocabulary
/// or the scripts cannot be collected.
pub(crate) fn prepare_figure_scripts(
    cat: &CharacterCatalog,
    options: AssemblyOptions,
    c: &AssembledCharacter,
    req: &FigureRequest,
    notes: &mut Vec<String>,
) -> Option<PreparedScripts> {
    let data = &cat.data;
    let Some(vocab) = cat.scripts.clone() else {
        notes.push("no script vocabulary; not animated".into());
        return None;
    };
    let Ok(registry) = data.registry() else {
        return None;
    };
    let dict = registry.dictionary();
    let chain = match collect_entity_scripts(c.clip_inputs.start, dict, &vocab, |d| data.read(d)) {
        Ok(ch) => ch,
        Err(e) => {
            notes.push(format!("scripts of {}: {e}", c.clip_inputs.start));
            return None;
        }
    };
    let mut scripts = chain.scripts;
    if let Some(inst) = &c.clip_inputs.instance_properties {
        scripts.add_property_set(inst, &vocab);
    }
    // The options the figure's lookup answers first and the character's own appearance
    // values (`appearance.md` §3.3, §3.5).
    if let Some(asm) = &cat.assembly {
        apply_figure_properties(&mut scripts, c, asm, options);
    }
    // A figure in the walker is placed in the world: `Physics_InWorld` turns true after its
    // start-up; a figure on a character screen carries its `CharGen_Stage` from the start
    // and is not placed (`appearance.md` §3.5 Decision).
    let after_startup = match (req.chargen_stage, cat.assembly.as_ref()) {
        (Some(stage), Some(asm)) => {
            for (pid, v) in figure_on_screen(asm, stage) {
                scripts.properties.set(pid, v);
            }
            for (name, key) in [
                ("CharGen_Action", req.chargen_action),
                ("Agent_Class", req.class),
            ] {
                if let Some((pid, v)) = key.and_then(|k| screen_property(registry, name, k)) {
                    scripts.properties.set(pid, v);
                }
            }
            if req.always_selectable
                && let Some(pid) = registry.pid("Render_AlwaysSelectable")
            {
                scripts.properties.set(pid, PropertyValue::Bool(true));
            }
            // The screens append `Chargen` to the start-up names of the set they create
            // the figure with (`character-screens.md` §2.1.4, CS28): it starts after
            // `Startup` and the class table's start-up names.
            match cat.chargen_script {
                Some(k) if !scripts.startup_scripts.contains(&k) => {
                    scripts.startup_scripts.push(k);
                }
                Some(_) => {}
                None => notes.push("no Chargen script key; the stage stance may be missing".into()),
            }
            Vec::new()
        }
        (Some(_), None) => Vec::new(),
        (None, asm) => asm.map(figure_after_startup).unwrap_or_default(),
    };
    Some((vocab, scripts, after_startup))
}

/// What [`select_clips`] gives a figure: its clips, shape inputs, script appearances, look,
/// script state and (a character-screen figure) its live-script capture.
type SelectedClips = (
    FigureClips,
    FigureShape,
    Vec<ScriptAppearance>,
    FigureLook,
    Option<Arc<FigureScripts>>,
    Option<super::live::Capture>,
);

/// Chooses idle, walk and run (`script-table.md` §11.3). Idle: every looping candidate,
/// the most likely per data layer (the original layers them, §6.2; a figure whose base
/// idle is a reference stance shows its stance from a higher layer); without a looping one
/// the most likely candidate of the `idle` layer, else of any layer. Walk and run: the
/// most likely candidate on the `movement` layer, else of any layer that is not an idle
/// clip. Held clips (`ManualUpdate`, §6.7) are never idle, walk or run clips: they are
/// returned apart, with the figure's other shape inputs ([`figure_shape`]). The figure's
/// script state ([`FigureScripts`]) is returned too, for the script-started clips
/// ([`super::combat_anim`]).
fn select_clips(
    ctx: &FigureContext,
    c: &AssembledCharacter,
    skeleton: &Arc<AnimSkeleton>,
    req: &FigureRequest,
    notes: &mut Vec<String>,
) -> SelectedClips {
    let seed = req.seed;
    let cat = &ctx.catalog;
    let data = &cat.data;
    let mut out = FigureClips::default();
    let mut shape = FigureShape::default();
    let mut appearances = Vec::new();
    let mut look = FigureLook::default();
    let Some((vocab, scripts, after_startup)) =
        prepare_figure_scripts(cat, ctx.assembly, c, req, notes)
    else {
        return (out, shape, appearances, look, None, None);
    };
    let Ok(registry) = data.registry() else {
        return (out, shape, appearances, look, None, None);
    };
    // A character-screen figure runs its scripts live (`character-screens.md` §2.1.4).
    let live_vocab = req.chargen_stage.is_some().then(|| vocab.clone());
    let dict = registry.dictionary();
    let figure_scripts = {
        let mut properties = scripts.properties.clone();
        for (pid, v) in &after_startup {
            properties.set(*pid, v.clone());
        }
        Arc::new(FigureScripts {
            tables: scripts.tables(),
            properties,
            startup_scripts: scripts.startup_scripts.clone(),
        })
    };
    let mut resolver = ClipResolver::new(dict, vocab, |d: Did| data.read(d))
        .with_dictionary_defaults(dict)
        .with_random_seed(Some(seed.unwrap_or(0)))
        .with_after_startup(after_startup);
    let mut idle_clips: Vec<Did> = Vec::new();
    let weight = |c: &ClipCandidate| c.blend_weight * c.chance;
    for state in [MotionState::Idle, MotionState::Walk, MotionState::Run] {
        let r = match resolver.resolve(&scripts, state) {
            Ok(r) => r,
            Err(e) => {
                notes.push(format!("{state:?}: {e}"));
                continue;
            }
        };
        let usable = |c: &&ClipCandidate| !c.clip.is_null() && !c.is_held();
        let best = |it: &mut dyn Iterator<Item = &ClipCandidate>| -> Option<ClipCandidate> {
            let mut best: Option<&ClipCandidate> = None;
            for c in it.filter(usable) {
                if best.is_none_or(|b| weight(c) > weight(b)) {
                    best = Some(c);
                }
            }
            best.cloned()
        };
        let chosen: Vec<ClipCandidate> = if state == MotionState::Idle {
            idle_clips = r.clips();
            if let Some(asm) = &cat.assembly {
                shape = figure_shape(data, asm, c, &r);
            }
            look = figure_look_from_data(data, &r, &LookLimits::default(), Some(&ctx.looks));
            appearances = r
                .appearances
                .iter()
                .filter(|a| a.picked)
                .map(|a| ScriptAppearance {
                    record: a.appearance,
                    key: a.key,
                    modifier: a.modifier,
                })
                .collect();
            if tracing::enabled!(tracing::Level::TRACE) {
                for c in r.candidates.iter().take(16) {
                    tracing::trace!(
                        table = %c.table,
                        clip = %c.clip,
                        layer = c.layer,
                        loops = c.loops(),
                        bone_opacity = c.use_bone_opacity,
                        weight = weight(c),
                        "idle candidate"
                    );
                }
            }
            // Every looping candidate, the most likely one per layer (the data layers
            // them, `script-table.md` §6.2); one-shot fidgets are left out (§8.6 OPEN).
            // A character-screen figure loops only its `LoopForever` clips and plays its
            // certain one-shot clips once ([`screen_one_shots`]).
            let screen = req.chargen_stage.is_some();
            let looping = |c: &&ClipCandidate| {
                if screen {
                    c.loop_forever || c.playback_rate() < MIN_RATE
                } else {
                    c.loops()
                }
            };
            let mut per_layer: Vec<ClipCandidate> = Vec::new();
            for c in r.candidates.iter().filter(usable).filter(looping) {
                match per_layer.iter_mut().find(|p| p.layer == c.layer) {
                    Some(p) if weight(c) > weight(p) => *p = c.clone(),
                    Some(_) => {}
                    None => per_layer.push(c.clone()),
                }
            }
            per_layer.sort_by_key(|c| c.layer);
            if screen {
                per_layer.extend(screen_one_shots(&r.candidates));
                per_layer.sort_by_key(|c| (c.layer, !c.loop_forever));
            } else if per_layer.is_empty() {
                // Nothing loops: the most likely candidate of the idle layer, else of any.
                per_layer.extend(
                    cat.idle_layer
                        .and_then(|l| best(&mut r.on_layer(l)))
                        .or_else(|| best(&mut r.candidates.iter())),
                );
            }
            per_layer.truncate(MAX_IDLE_LAYERS);
            per_layer
        } else {
            cat.movement_layer
                .and_then(|l| best(&mut r.on_layer(l)))
                .or_else(|| {
                    best(
                        &mut r
                            .candidates
                            .iter()
                            .filter(|c| !idle_clips.contains(&c.clip)),
                    )
                })
                .into_iter()
                .collect()
        };
        if chosen.is_empty() {
            notes.push(format!(
                "no {state:?} clip ({} candidates, {} issues)",
                r.candidates.len(),
                r.issues.len() + r.closure.issues.len()
            ));
        }
        for cand in chosen {
            let bound = match ctx.clips.bind(&ctx.set, cand.clip.0, skeleton) {
                Ok(b) => b,
                Err(e) => {
                    notes.push(e);
                    continue;
                }
            };
            let rate = cand.playback_rate();
            let rate = if rate.is_finite() && rate > 0.0 && rate < 100.0 {
                rate
            } else {
                1.0
            };
            let natural = cand.natural_speed(bound.duration());
            // A character-screen figure's one-shot clip (`script-table.md` §6.10).
            let once = if req.chargen_stage.is_some() && state == MotionState::Idle {
                let f0 = start_fraction(&cand, seed.unwrap_or(0));
                match PlayPlan::of(
                    cand.loop_forever,
                    cand.loop_count,
                    f0,
                    cand.end_time,
                    cand.ramp_out,
                    bound.duration(),
                    cand.playback_rate(),
                ) {
                    PlayPlan::Endless => None,
                    PlayPlan::Once(o) => Some(o),
                    PlayPlan::Skip => continue,
                }
            } else {
                None
            };
            let play = |fallback: f32| ClipPlay {
                did: cand.clip.0,
                clip: Arc::clone(&bound),
                rate,
                speed: natural
                    .map(|s| s * rate)
                    .filter(|s| s.is_finite() && *s > 0.1 && *s < 100.0)
                    .unwrap_or(fallback),
                layer: cand.layer,
                track_weights: cand.use_bone_opacity,
                held: None,
                once,
            };
            match state {
                MotionState::Idle => out.idle.push(play(0.0)),
                MotionState::Walk => out.walk = Some(play(FALLBACK_WALK_SPEED)),
                _ => out.run = Some(play(FALLBACK_RUN_SPEED)),
            }
        }
    }
    // A walk and a run that are the same clip, or a run no faster than the walk: one
    // locomotion clip.
    if let (Some(w), Some(r)) = (&out.walk, &out.run)
        && (w.did == r.did || r.speed <= w.speed)
    {
        out.run = None;
    }
    let live = live_vocab.and_then(|v| {
        super::live::capture(
            ctx,
            v,
            &scripts,
            req.chargen_stage,
            req.chargen_action,
            seed.unwrap_or(0),
        )
    });
    (out, shape, appearances, look, Some(figure_scripts), live)
}

#[cfg(test)]
mod tests {
    use super::*;

    /// `script-table.md` §5.2 (T47): for attached nodes the holding location wins over the
    /// bone type, for unattached nodes the bone type wins; both 0 give key 6 (attached) or
    /// the model origin (unattached).
    #[test]
    fn prop_anchors_follow_the_attachment_table() {
        use PropAnchorRule::{Bone, Location, Origin, OriginKey};
        assert_eq!(prop_anchor_rule(true, 12, 3), Location(12));
        assert_eq!(prop_anchor_rule(true, 12, 0), Location(12));
        assert_eq!(prop_anchor_rule(true, 0, 3), Bone(3));
        assert_eq!(prop_anchor_rule(true, 0, 0), OriginKey);
        assert_eq!(prop_anchor_rule(false, 12, 3), Bone(3));
        assert_eq!(prop_anchor_rule(false, 12, 0), Location(12));
        assert_eq!(prop_anchor_rule(false, 0, 3), Bone(3));
        assert_eq!(prop_anchor_rule(false, 0, 0), Origin);
    }

    /// `script-table.md` §6.10 steps 3–5: start time, end and the cases without one.
    #[test]
    fn play_plans_follow_the_start_and_end_rules() {
        assert_eq!(
            PlayPlan::of(true, 1, 0.0, 1.0, 0.2, 2.0, 1.0),
            PlayPlan::Endless
        );
        assert_eq!(
            PlayPlan::of(false, 1, 0.0, 1.0, 0.2, 2.0, 0.0),
            PlayPlan::Endless
        );
        // Once from the start: the whole clip.
        assert_eq!(
            PlayPlan::of(false, 1, 0.0, 1.0, 0.2, 2.0, 1.0),
            PlayPlan::Once(OneShot {
                start: 0.0,
                length: 2.0,
                ramp_out: 0.2,
                wraps: false,
            })
        );
        // Start fraction 0.25 and end time 0.75 at rate 2: starts 0.25 s in, ends after
        // (2 − 0.5) ÷ 2 − 0.25 = 0.5 s.
        let PlayPlan::Once(o) = PlayPlan::of(false, 1, 0.25, 0.75, 0.0, 2.0, 2.0) else {
            panic!("one-shot");
        };
        assert!((o.start - 0.25).abs() < 1e-6 && (o.length - 0.5).abs() < 1e-6);
        // Twice: wraps, twice as long.
        let PlayPlan::Once(o) = PlayPlan::of(false, 2, 0.0, 1.0, 0.0, 1.0, 1.0) else {
            panic!("one-shot");
        };
        assert!(o.wraps && (o.length - 2.0).abs() < 1e-6);
        // An end before the start is not started; hostile values do not panic.
        assert_eq!(
            PlayPlan::of(false, 1, 0.9, 0.5, 0.0, 1.0, 1.0),
            PlayPlan::Skip
        );
        assert_eq!(
            PlayPlan::of(false, 0, 0.0, 1.0, 0.0, 1.0, 1.0),
            PlayPlan::Skip
        );
        assert_eq!(
            PlayPlan::of(false, 1, f32::NAN, f32::NAN, f32::NAN, f32::NAN, 1.0),
            PlayPlan::Skip
        );
    }
}
