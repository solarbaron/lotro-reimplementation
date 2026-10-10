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
//!    resolved with the part's appearance layers (`material.md` §9.3) into the walker's
//!    material plans; held items' parts prepared rigid, keyed by holding location, with an
//!    attach table of the wearer's locations (`skeleton-anim.md` §8.4), each with its
//!    stowed or wielded placement (`appearance.md` §5.6).
//!
//! A placed figure's start-up scripts may append Appearance nodes to its look (the
//! random-appearance outfits, `appearance.md` §7.4): the Idle resolution of step 3 lists
//! them, its Random nodes picking with a generator seeded from the placement's instance ID
//! (the §7.4 Decision: a figure keeps its look), and when there are any the figure is
//! assembled again with them before its parts are built.
//!
//! The figure's height and slider values are the avatar entity's (`AC_Avatar_Height` 0 is
//! the shortest River-hobbit, `appearance.md` §6.2): the session model carries no
//! per-character values yet.
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
use tessera_formats::{Did, PropertySet};
use tessera_havok::Clip;
use tessera_material::{Layering, LibraryConfig, MaterialLibrary};
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
}

/// The look choices of a playable figure beyond its heights (decision 0025 D2: a
/// received look under the receiver rule): chosen styles and the slider string, as
/// `tessera-mesh`'s [`CharacterSpec`] takes them. Colours and face layers are not in the
/// assembler's input (`character-screens.md` §6 OPEN), so they are not drawn yet.
#[derive(Debug, Clone, Default, PartialEq)]
pub struct FigureLook {
    /// Chosen styles (`CharacterSpec::styles`).
    pub styles: Vec<StyleChoice>,
    /// The slider byte string (`AC_BoneSlider_Value_Array`).
    pub slider_values: Option<String>,
}

impl FigureLook {
    /// Whether it changes nothing.
    #[must_use]
    pub fn is_empty(&self) -> bool {
        self.styles.is_empty() && self.slider_values.is_none()
    }
}

impl FigureRequest {
    /// The cache key (no look).
    #[must_use]
    pub fn key(&self) -> FigureKey {
        self.key_with(&FigureLook::default())
    }

    /// The cache key with `look`.
    #[must_use]
    pub fn key_with(&self, look: &FigureLook) -> FigureKey {
        FigureKey {
            look: look
                .styles
                .iter()
                .map(|s| (s.key, s.modifier.to_bits()))
                .collect(),
            sliders: look.slider_values.clone(),
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
        }
    }
}

/// Identifies a figure: equal keys share one asset and one set of GPU resources.
#[derive(Debug, Clone, PartialEq, Eq, Hash)]
pub struct FigureKey {
    look: Vec<(u32, u32)>,
    sliders: Option<String>,
    body: Body,
    items: Vec<(u32, u32, u32, bool, Option<u8>)>,
    instance: Option<u64>,
    static_only: bool,
    shape: [u32; 2],
    seed: Option<u64>,
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
    /// held part, one attach point per placement (`appearance.md` §5.6).
    pub held: Option<u32>,
}

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
}

/// The clips of idle, walk and run.
#[derive(Debug, Clone, Default)]
pub struct FigureClips {
    /// The looping idle clips, one per data layer (the most likely of each), lowest layer
    /// first, at most [`MAX_IDLE_LAYERS`].
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
pub const MAX_IDLE_LAYERS: usize = 4;

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
    /// Problems, for the log.
    pub notes: Vec<String>,
    /// Worker time.
    pub build_ms: f64,
}

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

/// Decoded clips by DID, shared by every figure job, with their baked frames.
#[derive(Debug)]
pub struct ClipStore {
    clips: Mutex<HashMap<u32, Option<Arc<Clip>>>>,
    bake: BakeCache,
}

impl Default for ClipStore {
    fn default() -> Self {
        Self {
            clips: Mutex::new(HashMap::new()),
            bake: BakeCache::new(BAKE_BUDGET, MAX_BAKED_CLIP),
        }
    }
}

impl ClipStore {
    /// The decoded clip `did` (read and decoded once; failures remembered).
    fn clip(&self, set: &ArchiveSet, did: u32) -> Option<Arc<Clip>> {
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
    fn bind(
        &self,
        set: &ArchiveSet,
        did: u32,
        skeleton: &Arc<AnimSkeleton>,
    ) -> Result<Arc<BoundClip>, String> {
        let clip = self
            .clip(set, did)
            .ok_or_else(|| format!("clip {did:#010x} missing or undecodable"))?;
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
    /// Tessera's answers for the options a figure's property lookup reads first
    /// (`appearance.md` §3.3: the updated-avatar option).
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
    build_figure_with_look(ctx, req, &FigureLook::default(), cancelled)
}

/// [`build_figure`] with the look choices of a playable body (styles, slider string).
///
/// # Errors
///
/// As [`build_figure`].
pub fn build_figure_with_look(
    ctx: &FigureContext,
    req: &FigureRequest,
    look: &FigureLook,
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
            spec.styles.clone_from(&look.styles);
            spec.slider_values.clone_from(&look.slider_values);
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
    let (clips, shape, scripts) = match &skeleton {
        Some(s) => select_clips(ctx, &c, s, req.seed, &mut notes),
        None => (FigureClips::default(), FigureShape::default(), Vec::new()),
    };
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
            })
            .collect();
    }

    let mut parts = Vec::new();
    for p in c.parts.iter().filter(|p| !p.hidden) {
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
            &p.materials,
            deformation.as_ref(),
        ) {
            Ok(mut part) => {
                part.held = None;
                parts.push(part);
            }
            Err(e) => notes.push(format!("part {}: {e}", p.mesh)),
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
                match build_part(ctx, mesh, None, &[], None) {
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
    let points = crate::audio::loader::fetch(&ctx.set, c.skeleton.setup.0)
        .ok()
        .flatten()
        .and_then(|b| {
            let s = <tessera_formats::setup::Setup as tessera_formats::Record>::parse_expecting(
                &b,
                c.skeleton.setup,
            )
            .ok()?;
            Some(Arc::new(
                crate::walker::effects::data::AttachPoints::from_setup(&s, &b),
            ))
        });
    // `model_scale` applies the client's limit (§6.1); also refuse absurdly large scales.
    let model_scale = c.model_scale();
    let model_scale = if model_scale.iter().all(|s| *s < 100.0) {
        model_scale
    } else {
        notes.push("unusable model scale; drawn at 1".into());
        [1.0; 3]
    };
    let lod_class = figure_lod_class(ctx, req.instance.as_deref(), &c.chain);
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
        notes,
        build_ms: started.elapsed().as_secs_f64() * 1e3,
    })
}

/// Most 0x47 levels of a figure's entity chain read for its `Render_LODClass`.
const MAX_LOD_CLASS_LEVELS: usize = 16;

/// The `Render_LODClass` of a figure (`walker.md` §11.4.3, the Tessera Decision W22 there):
/// the first value found in the placement's instance set, then in the property sets of the
/// 0x47 levels of `chain` (nearest first). An enum value gives its key, a bit-like value its
/// low 32 bits; anything else, or nothing found, gives `None`.
fn figure_lod_class(
    ctx: &FigureContext,
    instance: Option<&PropertySet>,
    chain: &[Did],
) -> Option<u32> {
    let data = &ctx.catalog.data;
    let registry = data.registry().ok()?;
    let pid = registry.pid("Render_LODClass")?;
    let class_of = |v: &tessera_formats::PropertyValue| match v {
        tessera_formats::PropertyValue::Enum(k) => Some(*k),
        other => other.as_bits().and_then(|b| u32::try_from(b).ok()),
    };
    if let Some(v) = instance.and_then(|s| s.get(pid)) {
        return class_of(v);
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
            return class_of(v);
        }
    }
    None
}

/// Decodes, flattens and prepares one part mesh; skinned when `bones` is given, its
/// skinned vertices deformed in bind space by `deform` (`appearance.md` §6.4 step 4).
fn build_part(
    ctx: &FigureContext,
    mesh_did: Did,
    bones: Option<usize>,
    layers: &[MaterialLayer],
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
                |s| plan_for(ctx, s, layers),
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
    })
}

/// The plan of `surface` with the part's appearance layers whose binding key matches the
/// surface's (`material.md` §9.3, `appearance.md` §4.10).
fn plan_for(ctx: &FigureContext, surface: Did, layers: &[MaterialLayer]) -> MaterialPlan {
    let lib = &ctx.materials;
    let key = lib.surface_record(surface).ok().map(|r| r.binding_key);
    let matching: Vec<&MaterialLayer> = layers
        .iter()
        .filter(|l| Some(l.binding_key) == key && !l.material.is_null())
        .take(16)
        .collect();
    let resolved = if matching.is_empty() {
        lib.resolve_surface(surface).map(|m| plan_material_dyed(&m))
    } else {
        let layering = Layering {
            shader_override: matching.iter().find_map(|l| l.shader),
            materials: matching.iter().map(|l| l.material).collect(),
        };
        lib.resolve_layered(surface, &layering)
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

/// Chooses idle, walk and run (`script-table.md` §11.3). Idle: every looping candidate,
/// the most likely per data layer (the original layers them, §6.2; a figure whose base
/// idle is a reference stance shows its stance from a higher layer); without a looping one
/// the most likely candidate of the `idle` layer, else of any layer. Walk and run: the
/// most likely candidate on the `movement` layer, else of any layer that is not an idle
/// clip. Held clips (`ManualUpdate`, §6.7) are never idle, walk or run clips: they are
/// returned apart, with the figure's other shape inputs ([`figure_shape`]).
fn select_clips(
    ctx: &FigureContext,
    c: &AssembledCharacter,
    skeleton: &Arc<AnimSkeleton>,
    seed: Option<u64>,
    notes: &mut Vec<String>,
) -> (FigureClips, FigureShape, Vec<ScriptAppearance>) {
    let cat = &ctx.catalog;
    let data = &cat.data;
    let mut out = FigureClips::default();
    let mut shape = FigureShape::default();
    let mut appearances = Vec::new();
    let Some(vocab) = cat.scripts.clone() else {
        notes.push("no script vocabulary; not animated".into());
        return (out, shape, appearances);
    };
    let Ok(registry) = data.registry() else {
        return (out, shape, appearances);
    };
    let dict = registry.dictionary();
    let chain = match collect_entity_scripts(c.clip_inputs.start, dict, &vocab, |d| data.read(d)) {
        Ok(ch) => ch,
        Err(e) => {
            notes.push(format!("scripts of {}: {e}", c.clip_inputs.start));
            return (out, shape, appearances);
        }
    };
    let mut scripts = chain.scripts;
    if let Some(inst) = &c.clip_inputs.instance_properties {
        scripts.add_property_set(inst, &vocab);
    }
    let mut resolver = ClipResolver::new(dict, vocab, |d: Did| data.read(d))
        .with_dictionary_defaults(dict)
        .with_random_seed(Some(seed.unwrap_or(0)));
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
            let mut per_layer: Vec<ClipCandidate> = Vec::new();
            for c in r.candidates.iter().filter(usable).filter(|c| c.loops()) {
                match per_layer.iter_mut().find(|p| p.layer == c.layer) {
                    Some(p) if weight(c) > weight(p) => *p = c.clone(),
                    Some(_) => {}
                    None => per_layer.push(c.clone()),
                }
            }
            per_layer.sort_by_key(|c| c.layer);
            if per_layer.is_empty() {
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
    (out, shape, appearances)
}
