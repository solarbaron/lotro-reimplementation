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
//! 3. **Parts**: every visible body part decoded, flattened at its display level and
//!    prepared for the GPU, skinned to the skeleton where the mesh is skinned; materials
//!    resolved with the part's appearance layers (`material.md` §9.3) into the walker's
//!    material plans; held items' parts prepared rigid, keyed by holding location, with an
//!    attach table of the wearer's locations (`skeleton-anim.md` §8.4).
//! 4. **Clips**: the script tables choose idle, walk and run
//!    ([`ClipResolver`], `script-table.md` §11.3; every looping idle layer, walk and run
//!    on `movement`); the clips are decoded once per DID ([`ClipStore`]), baked for sampling
//!    ([`BakeCache`]) and bound to the skeleton (track count = bone count,
//!    `skeleton-anim.md` §3.4).
//!
//! A figure without a usable skeleton, or with no skinned part, is **static**: its parts
//! are drawn rigid in the setup frame, unanimated. Every failure short of "no body" is a
//! note in [`FigureAsset::notes`]; nothing panics on hostile data (every decoder below
//! bounds its input).

use std::collections::HashMap;
use std::sync::{Arc, Mutex, PoisonError};
use std::time::Instant;

use tessera_anim::{
    AttachFrame, AttachPointDesc, AttachTable, BakeCache, BoundClip, Skeleton as AnimSkeleton,
};
use tessera_dat::ArchiveSet;
use tessera_formats::script_table::resolve::{
    ClipCandidate, ClipResolver, MotionState, collect_entity_scripts,
};
use tessera_formats::setup::Anchor;
use tessera_formats::{Did, PropertySet};
use tessera_havok::Clip;
use tessera_material::{Layering, LibraryConfig, MaterialLibrary};
use tessera_mesh::assembly::{
    AssembledCharacter, CharacterAssembler, CharacterSpec, EquippedItem, GameDataSource,
    MaterialLayer, WieldState,
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
}

impl FigureRequest {
    /// The cache key.
    #[must_use]
    pub fn key(&self) -> FigureKey {
        FigureKey {
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
                    )
                })
                .collect(),
            instance: self.instance_key,
            static_only: self.static_only,
        }
    }
}

/// Identifies a figure: equal keys share one asset and one set of GPU resources.
#[derive(Debug, Clone, PartialEq, Eq, Hash)]
pub struct FigureKey {
    body: Body,
    items: Vec<(u32, u32, u32, bool)>,
    instance: Option<u64>,
    static_only: bool,
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
    /// The holding location of a held item's part (`None`: a body part).
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
    /// Whole-body scale (`appearance.md` §6.1).
    pub model_scale: f32,
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
    let started = Instant::now();
    let cat = &ctx.catalog;
    let vocab = cat
        .assembly
        .clone()
        .ok_or_else(|| FigureError::Unavailable("no assembly vocabulary".into()))?;
    let source =
        GameDataSource::new(&cat.data).map_err(|e| FigureError::Unavailable(e.to_string()))?;
    let mut asm = CharacterAssembler::new(source, vocab);
    let c = match req.body {
        Body::Character { species, sex } => {
            let mut spec = CharacterSpec::new(species, sex);
            for e in req.equipment.iter().take(super::MAX_EQUIPMENT) {
                let mut item = EquippedItem::new(Did(e.item.0)).with_state(if e.stowed {
                    WieldState::Sheathed
                } else {
                    WieldState::Wielded
                });
                item.dye = e.dye;
                item.secondary_dye = e.secondary_dye;
                spec.items.push(item);
            }
            asm.assemble_character(&spec)?
        }
        Body::Object(d) => asm.assemble_object(Did(d.0), req.instance.as_deref())?,
        _ => return Err(FigureError::UnsupportedBody),
    };
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

    let mut parts = Vec::new();
    for p in c.parts.iter().filter(|p| !p.hidden) {
        if parts.len() >= MAX_FIGURE_PARTS {
            notes.push(format!("more than {MAX_FIGURE_PARTS} parts"));
            break;
        }
        if cancelled() {
            return Err(FigureError::Cancelled);
        }
        match build_part(ctx, p.mesh, bones.filter(|_| p.skinned), &p.materials) {
            Ok(mut part) => {
                part.held = None;
                parts.push(part);
            }
            Err(e) => notes.push(format!("part {}: {e}", p.mesh)),
        }
    }
    // Held items: rigid parts at the wearer's holding locations (skeleton-anim.md §8.4).
    let mut attach_points = Vec::new();
    if skeleton.is_some() {
        for h in &c.held {
            let mut any = false;
            for &mesh in &h.parts {
                if parts.len() >= MAX_FIGURE_PARTS {
                    break;
                }
                if cancelled() {
                    return Err(FigureError::Cancelled);
                }
                match build_part(ctx, mesh, None, &[]) {
                    Ok(mut part) => {
                        part.held = Some(h.location);
                        parts.push(part);
                        any = true;
                    }
                    Err(e) => notes.push(format!("held part {mesh}: {e}")),
                }
            }
            if any {
                attach_points.push(AttachPointDesc {
                    key: h.location,
                    bone: match h.anchor {
                        Anchor::Bone(b) => i32::try_from(b).unwrap_or(-1),
                        Anchor::Origin => -1,
                    },
                    frame: AttachFrame {
                        position: h.frame.position,
                        scale: h.frame.scale,
                        rotation_wxyz: [
                            h.frame.rotation.w,
                            h.frame.rotation.x,
                            h.frame.rotation.y,
                            h.frame.rotation.z,
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
    let clips = match &skeleton {
        Some(s) => select_clips(ctx, &c, s, &mut notes),
        None => FigureClips::default(),
    };
    let model_scale = c.model_scale();
    let model_scale = if model_scale.is_finite() && model_scale > 0.0 && model_scale < 100.0 {
        model_scale
    } else {
        notes.push("unusable model scale; drawn at 1".into());
        1.0
    };
    Ok(FigureAsset {
        skeleton,
        parts,
        attach,
        clips,
        model_scale,
        notes,
        build_ms: started.elapsed().as_secs_f64() * 1e3,
    })
}

/// Decodes, flattens and prepares one part mesh; skinned when `bones` is given.
fn build_part(
    ctx: &FigureContext,
    mesh_did: Did,
    bones: Option<usize>,
    layers: &[MaterialLayer],
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
            p.surface
                .map_or(MaterialPlan::FALLBACK, |s| plan_for(ctx, s, layers))
        })
        .collect();
    let data = level_mesh_data(&geo, format!("figure part {mesh_did}"));
    let prepared = match (skinned, bones) {
        (true, Some(b)) => {
            let (skin, _rigid) = mesh_skin(&geo, b)?;
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
        MaterialPlan::FALLBACK
    })
}

/// Chooses idle, walk and run (`script-table.md` §11.3). Idle: every looping candidate,
/// the most likely per data layer (the original layers them, §6.2; a figure whose base
/// idle is a reference stance shows its stance from a higher layer); without a looping one
/// the most likely candidate of the `idle` layer, else of any layer. Walk and run: the
/// most likely candidate on the `movement` layer, else of any layer that is not an idle
/// clip.
fn select_clips(
    ctx: &FigureContext,
    c: &AssembledCharacter,
    skeleton: &Arc<AnimSkeleton>,
    notes: &mut Vec<String>,
) -> FigureClips {
    let cat = &ctx.catalog;
    let data = &cat.data;
    let mut out = FigureClips::default();
    let Some(vocab) = cat.scripts.clone() else {
        notes.push("no script vocabulary; not animated".into());
        return out;
    };
    let Ok(registry) = data.registry() else {
        return out;
    };
    let dict = registry.dictionary();
    let chain = match collect_entity_scripts(c.clip_inputs.start, dict, &vocab, |d| data.read(d)) {
        Ok(ch) => ch,
        Err(e) => {
            notes.push(format!("scripts of {}: {e}", c.clip_inputs.start));
            return out;
        }
    };
    let mut scripts = chain.scripts;
    if let Some(inst) = &c.clip_inputs.instance_properties {
        scripts.add_property_set(inst, &vocab);
    }
    let mut resolver =
        ClipResolver::new(dict, vocab, |d: Did| data.read(d)).with_dictionary_defaults(dict);
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
        let usable = |c: &&ClipCandidate| !c.clip.is_null();
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
    out
}
