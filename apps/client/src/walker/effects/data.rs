// SPDX-License-Identifier: GPL-3.0-or-later
//! The data side of particle effects in the walker, run on the asset workers: script tables
//! and scriptlets, the owners whose start-up scripts reach Particle nodes, their attachment
//! points and scales, and the effect records with the texture and blend state of their
//! surfaces (`particle-runtime.md` §5, §6.4, §9.1; `particles.md` §4.1, §7.1.1).
//!
//! Every record is read once through [`EffectData`]'s caches. The runtime's
//! [`ScriptSource`] ([`EffectSource`]) reads from the same caches and counts the reads it
//! has to make on the main thread.

use std::collections::{HashMap, HashSet};
use std::sync::atomic::{AtomicU64, Ordering};
use std::sync::{Arc, Mutex, MutexGuard};

use tessera_anim::AttachFrame;
use tessera_dat::ArchiveSet;
use tessera_formats::gameplay::particles::{
    FORK_SYSTEM_PREFIX, ForkSystem, PARTICLE_SYSTEM_PREFIX, ParticleSystem,
};
use tessera_formats::property_set::PropertySet;
use tessera_formats::script_table::resolve::{
    PropertyState, ScriptVocabulary, collect_entity_scripts,
};
use tessera_formats::setup::{
    Anchor as SetupAnchor, ENTITY_PREFIX, Setup, TEMPLATE_PREFIX, VisualTemplate,
};
use tessera_formats::{Did, NodeData, PropertyDictionary, Record as _, ScriptTable, Scriptlet};
use tessera_material::records::{BlendFactor as MatFactor, BlendOp as MatOp};
use tessera_material::{BlendMode, MaterialLibrary, ResolvedMaterial};
use tessera_particles::glam::{Mat4, Quat};
use tessera_particles::surface::{LayerCombine, ParticleSurface};
use tessera_render::{
    AlphaMode, BlendFactor, BlendOp, FixedBlend, ParticleLayers, ParticleShading,
};
use tessera_scriptgraph::{EntityId, ScriptSource};
use tessera_world::{LandblockBundle, LandblockKey};

use crate::audio::loader::fetch;
use crate::walker::figures::figure_tag;
use crate::walker::geometry::{TexRequest, VARIANT_OWN_ALPHA};

/// The property dictionary's DID (`property-system.md`).
pub const PROPERTY_DICTIONARY: u32 = tessera_world::context::PROPERTY_DICTIONARY_DID;
/// Most tables in one closure.
pub const MAX_CLOSURE_TABLES: usize = 512;
/// Most script keys followed through Run nodes from one owner's start-up scripts.
pub const MAX_SCAN_KEYS: usize = 4096;
/// Most scriptlets walked for one owner.
pub const MAX_SCAN_SCRIPTLETS: usize = 1024;
/// Most script tables (and as many scriptlets) cached.
pub const MAX_CACHED_SCRIPTS: usize = 8192;
/// Most owner objects, setups and effect records cached (each).
pub const MAX_CACHED_OBJECTS: usize = 32_768;
/// Most effect owners taken from one landblock.
pub const MAX_OWNERS_PER_BLOCK: usize = 1024;
/// The reach of an owner whose effect records give none (metres): the default far fade of
/// a new Particle node (`particle-runtime.md` §5.2).
pub const DEFAULT_REACH: f32 = 60.0;
/// Largest reach accepted from data (metres): hostile fade distances cannot keep owners
/// alive across a region.
pub const MAX_REACH: f32 = 500.0;
/// Low bit pattern of a static owner's runtime ID (figure owners use the figure tag, whose
/// low word is a figure index below this bit).
pub const STATIC_OWNER_BIT: u64 = 0x8000_0000;

pub(crate) fn lock<T>(m: &Mutex<T>) -> MutexGuard<'_, T> {
    m.lock().unwrap_or_else(std::sync::PoisonError::into_inner)
}

/// The runtime ID of the static entity `index` of landblock `key`.
#[must_use]
pub fn static_owner_id(key: LandblockKey, index: u32) -> EntityId {
    EntityId((u64::from(key.data_did().0) << 32) | STATIC_OWNER_BIT | u64::from(index))
}

/// The attachment points of a setup (`skeleton-anim.md` §8.1, §8.2): its holding locations
/// (frame list A) with their bones, its `attach_map` and the skeleton's reference pose in
/// model space (the pose of an owner that is not animated).
#[derive(Debug, Clone, Default, PartialEq)]
pub struct AttachPoints {
    /// (`HoldingLocation` key, bone or `None` for the setup origin, frame matrix).
    pub holding: Vec<(u32, Option<usize>, Mat4)>,
    /// (`BoneType` key, bone index).
    pub bones: Vec<(u32, usize)>,
    /// Model-space reference matrices per bone (empty without a skeleton).
    pub reference: Vec<Mat4>,
}

impl AttachPoints {
    /// The points of a decoded setup and the reference pose of its skeleton (empty when
    /// the setup has none or it does not decode). Locations whose frame is unusable, or
    /// whose bone is not below the bone count, are left out.
    #[must_use]
    pub fn from_setup(setup: &Setup, record: &[u8]) -> Self {
        let reference = tessera_anim::Skeleton::from_setup_record(record)
            .map(|s| s.reference_model().to_vec())
            .unwrap_or_default();
        let bones_n = reference.len();
        let holding = setup
            .holding_locations()
            .filter_map(|h| {
                let f = h.frame;
                let m = AttachFrame {
                    position: f.position,
                    scale: f.scale,
                    rotation_wxyz: [f.rotation.w, f.rotation.x, f.rotation.y, f.rotation.z],
                }
                .matrix()?;
                let bone = match h.anchor {
                    SetupAnchor::Origin => None,
                    SetupAnchor::Bone(b) => {
                        let b = usize::try_from(b).ok().filter(|&b| b < bones_n)?;
                        Some(b)
                    }
                };
                Some((h.key, bone, m))
            })
            .collect();
        let bones = setup
            .attachments()
            .filter_map(|a| {
                let b = usize::try_from(a.bone).ok().filter(|&b| b < bones_n)?;
                Some((a.bone_type, b))
            })
            .collect();
        Self {
            holding,
            bones,
            reference,
        }
    }

    /// Holding location `key`: its bone (or `None`: the setup origin) and frame.
    #[must_use]
    pub fn holding(&self, key: u32) -> Option<(Option<usize>, Mat4)> {
        self.holding
            .iter()
            .find(|(k, _, _)| *k == key)
            .map(|(_, b, m)| (*b, *m))
    }

    /// The bone of `BoneType` key `bone_type`.
    #[must_use]
    pub fn bone(&self, bone_type: u32) -> Option<usize> {
        self.bones
            .iter()
            .find(|(k, _)| *k == bone_type)
            .map(|(_, b)| *b)
    }
}

/// The template values of an object's visual chain (`setup.md` §5.2): `Model_Scale` (the
/// `vec3` product) and `Model_ParticleScale` (the `value_b` product), 1 when absent.
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct ModelScales {
    /// `Model_Scale`.
    pub model: [f32; 3],
    /// `Model_ParticleScale` (vector *B*'s *p*, `particle-runtime.md` §6.4).
    pub particle: f32,
}

impl Default for ModelScales {
    fn default() -> Self {
        Self {
            model: [1.0; 3],
            particle: 1.0,
        }
    }
}

/// The scripts of an owner whose start-up scripts reach Particle nodes.
#[derive(Debug, Clone, PartialEq)]
pub struct OwnerScripts {
    /// Its tables (class, instance, additional).
    pub tables: Vec<Did>,
    /// Its start-up script keys.
    pub startup_scripts: Vec<u32>,
    /// Its stored property values.
    pub properties: PropertyState,
    /// The farthest its effects are drawn from the owner (metres): the largest far-fade end
    /// of the 0x39 records and far-fade maximum of the fork nodes it reaches, plus the node
    /// offsets, at most [`MAX_REACH`].
    pub reach: f32,
    /// Whether one of its nodes attaches to a holding location or a bone.
    pub attaches: bool,
}

/// What an owner is.
#[derive(Debug, Clone, Copy, PartialEq)]
pub enum OwnerKind {
    /// A static entity of the landblock.
    Static {
        /// Region-global position.
        position: [f64; 3],
        /// Rotation.
        rotation: Quat,
        /// `Render_AdjustableScale` × `Model_Scale` (the render scale, `setup.md` §5.2).
        scale: [f32; 3],
    },
    /// A placed figure, posed by the character layer (its tag, `walker.md` §10.2).
    Figure {
        /// The figure tag.
        tag: u64,
    },
}

/// An effect owner a landblock scan found.
#[derive(Debug, Clone, PartialEq)]
pub struct EffectOwner {
    /// Its runtime ID.
    pub id: EntityId,
    /// Static entity or figure.
    pub kind: OwnerKind,
    /// The 64-bit instance ID, when the entity has one (the start gating of `particles.md`
    /// §7.1.2 tells static-environment objects by it).
    pub iid: Option<u64>,
    /// Its scripts.
    pub scripts: Arc<OwnerScripts>,
    /// The attachment points of its setup (static owners; figures bring their own).
    pub points: Option<Arc<AttachPoints>>,
    /// Its template scales.
    pub scales: ModelScales,
    /// Region-global position (for figures, the placement in the data; the character layer
    /// gives the live one).
    pub position: [f64; 3],
}

/// What a landblock contributes.
#[derive(Debug, Clone, PartialEq)]
pub struct BlockScan {
    /// The landblock.
    pub key: LandblockKey,
    /// Identity of the bundle scanned (its address), to drop stale results.
    pub bundle: usize,
    /// Effect owners.
    pub owners: Vec<EffectOwner>,
}

/// A loaded effect record (`particles.md` §4, §6).
#[derive(Debug)]
pub enum EffectRecord {
    /// A 0x39 system with the textures, blend state and program of its surface
    /// (`particle-runtime.md` §9.1, §9.1.1); `texture` is `None` without a surface texture,
    /// `drawable` false without a surface (the client draws nothing, `particles.md` §8.11)
    /// and for the frame-buffer distortion program, which Tessera draws as its variant
    /// without a frame-buffer input (it leaves the target unchanged).
    Particles {
        /// The record.
        desc: Arc<ParticleSystem>,
        /// The surface's colour texture (slot 3000) with its own alpha.
        texture: Option<TexRequest>,
        /// The second colour layer (slot 3001) of a two-layer program.
        second_texture: Option<TexRequest>,
        /// The surface's blend, alpha test and depth write.
        alpha: AlphaMode,
        /// What the surface's program computes (§9.1.1).
        shading: ParticleShading,
        /// Material colour 2000.
        material: [f32; 4],
        /// Whether the system is drawn.
        drawable: bool,
    },
    /// A 0x44 fork effect (not simulated, `particles.md` rule §10.2).
    Fork(Arc<ForkSystem>),
}

fn factor(f: MatFactor) -> Option<BlendFactor> {
    Some(match f {
        MatFactor::Zero => BlendFactor::Zero,
        MatFactor::One => BlendFactor::One,
        MatFactor::SrcColor => BlendFactor::SrcColor,
        MatFactor::InvSrcColor => BlendFactor::OneMinusSrcColor,
        MatFactor::SrcAlpha => BlendFactor::SrcAlpha,
        MatFactor::InvSrcAlpha => BlendFactor::OneMinusSrcAlpha,
        MatFactor::DestAlpha => BlendFactor::DstAlpha,
        MatFactor::DestColor => BlendFactor::DstColor,
        MatFactor::Other(_) => return None,
    })
}

/// The renderer's program description for a particle surface (`particle-runtime.md`
/// §9.1.1).
#[must_use]
pub fn particle_shading(s: &ParticleSurface) -> ParticleShading {
    let p = s.program;
    ParticleShading {
        tests_texture: p.tests_texture,
        texture_alpha: p.texture_alpha,
        vertex_colour: p.vertex_colour,
        material_colour: p.material_colour,
        doubled: p.doubled,
        fogged: p.fogged,
        layers: p.layers.map(|l| match l {
            LayerCombine::Product => ParticleLayers::Product,
            LayerCombine::Sum => ParticleLayers::Sum,
        }),
    }
}

/// The renderer's alpha mode for a particle system's surface material: its blend mode,
/// factors, depth write and alpha test (`particle-runtime.md` §9.1: the draw takes these
/// states from the surface record's material, `material.md` §7, §11).
#[must_use]
pub fn surface_alpha(m: &ResolvedMaterial) -> AlphaMode {
    let b = m.features.blend;
    let test = m.features.alpha_test.map(|a| a.threshold);
    match b.mode {
        BlendMode::Opaque => test.map_or(AlphaMode::Opaque, |c| AlphaMode::Mask { cutoff: c }),
        BlendMode::Cutout => AlphaMode::Mask {
            cutoff: test.unwrap_or(0.5),
        },
        BlendMode::AlphaBlend if b.depth_write => AlphaMode::TestBlend {
            cutoff: test.unwrap_or(-1.0),
        },
        // `particle-runtime.md` §9.1.1 rule 1: alpha test on with depth write off keeps
        // the material's reference (the renderer switches the test off while *A* < 0.9),
        // so the draw carries the cut-off without depth write.
        BlendMode::AlphaBlend => test.map_or(AlphaMode::Blend, |c| {
            AlphaMode::Fixed(FixedBlend {
                src: BlendFactor::SrcAlpha,
                dst: BlendFactor::OneMinusSrcAlpha,
                op: BlendOp::Add,
                depth_write: false,
                alpha_test: Some(tessera_render::AlphaThreshold(c)),
            })
        }),
        BlendMode::Additive if b.depth_write => AlphaMode::Fixed(FixedBlend {
            depth_write: true,
            alpha_test: test.map(tessera_render::AlphaThreshold),
            ..FixedBlend::ADDITIVE
        }),
        BlendMode::Additive => test.map_or(AlphaMode::Additive, |c| {
            AlphaMode::Fixed(FixedBlend {
                alpha_test: Some(tessera_render::AlphaThreshold(c)),
                ..FixedBlend::ADDITIVE
            })
        }),
        BlendMode::Custom => {
            let op = match b.op {
                MatOp::Add => Some(BlendOp::Add),
                MatOp::RevSubtract => Some(BlendOp::ReverseSubtract),
                MatOp::Other(_) => None,
            };
            match (factor(b.src), factor(b.dst), op) {
                (Some(src), Some(dst), Some(op)) => AlphaMode::Fixed(FixedBlend {
                    src,
                    dst,
                    op,
                    depth_write: b.depth_write,
                    alpha_test: test.map(tessera_render::AlphaThreshold),
                }),
                _ => AlphaMode::Blend,
            }
        }
    }
}

#[derive(Debug, Default)]
struct ScanOut {
    effects: Vec<(Did, f32)>,
    attaches: bool,
}

/// Shared data and caches (module docs).
pub struct EffectData {
    /// The archives.
    pub set: Arc<ArchiveSet>,
    /// The property dictionary.
    pub dict: Arc<PropertyDictionary>,
    /// Entity script vocabulary (`None` when a required name is missing).
    pub svocab: Option<ScriptVocabulary>,
    /// The `Startup` script key.
    pub startup_key: Option<u32>,
    /// Content layers treated as active.
    pub active_layers: Vec<u32>,
    /// Whether interior entities run their effects.
    pub interiors: bool,
    materials: Arc<MaterialLibrary<Arc<ArchiveSet>>>,
    tables: Mutex<HashMap<Did, Option<Arc<ScriptTable>>>>,
    scriptlets: Mutex<HashMap<Did, Option<Arc<Scriptlet>>>>,
    objects: Mutex<HashMap<Did, Option<Arc<OwnerScripts>>>>,
    points: Mutex<HashMap<Did, Option<Arc<AttachPoints>>>>,
    scales: Mutex<HashMap<Did, ModelScales>>,
    records: Mutex<HashMap<u32, Option<Arc<EffectRecord>>>>,
    main_thread_reads: AtomicU64,
}

impl std::fmt::Debug for EffectData {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("EffectData")
            .field("startup_key", &self.startup_key)
            .finish_non_exhaustive()
    }
}

impl EffectData {
    /// Loads the dictionary and the script vocabulary.
    ///
    /// # Errors
    ///
    /// When the dictionary is missing or does not decode.
    pub fn open(
        set: Arc<ArchiveSet>,
        materials: Arc<MaterialLibrary<Arc<ArchiveSet>>>,
        active_layers: Vec<u32>,
        interiors: bool,
    ) -> Result<Self, String> {
        let bytes = fetch(&set, PROPERTY_DICTIONARY)
            .map_err(|e| format!("property dictionary: {e}"))?
            .ok_or("property dictionary missing")?;
        let dict = PropertyDictionary::parse_expecting(&bytes, PROPERTY_DICTIONARY)
            .map_err(|e| format!("property dictionary: {e}"))?;
        let svocab =
            match ScriptVocabulary::from_data(&dict, |d| crate::audio::scripts::mapper(&set, d)) {
                Ok(v) => Some(v),
                Err(e) => {
                    tracing::warn!("effects: entity script vocabulary incomplete: {e}");
                    None
                }
            };
        let startup_key = svocab.as_ref().map(|v| v.startup_key);
        Ok(Self {
            set,
            dict: Arc::new(dict),
            svocab,
            startup_key,
            active_layers,
            interiors,
            materials,
            tables: Mutex::new(HashMap::new()),
            scriptlets: Mutex::new(HashMap::new()),
            objects: Mutex::new(HashMap::new()),
            points: Mutex::new(HashMap::new()),
            scales: Mutex::new(HashMap::new()),
            records: Mutex::new(HashMap::new()),
            main_thread_reads: AtomicU64::new(0),
        })
    }

    /// Script records the runtime had to read on the main thread (cache misses).
    #[must_use]
    pub fn main_thread_reads(&self) -> u64 {
        self.main_thread_reads.load(Ordering::Relaxed)
    }

    /// A script table, read once.
    #[must_use]
    pub fn table(&self, did: Did) -> Option<Arc<ScriptTable>> {
        if did.prefix() != tessera_formats::script_table::SCRIPT_TABLE_PREFIX {
            return None;
        }
        if let Some(t) = lock(&self.tables).get(&did) {
            return t.clone();
        }
        let t = fetch(&self.set, did.0).ok().flatten().and_then(|b| {
            ScriptTable::parse_expecting(&b, did, &*self.dict)
                .map_err(|e| tracing::debug!(%did, error = %e, "script table does not decode"))
                .ok()
                .map(Arc::new)
        });
        let mut m = lock(&self.tables);
        if m.len() < MAX_CACHED_SCRIPTS {
            m.insert(did, t.clone());
        }
        t
    }

    /// A scriptlet, read once.
    #[must_use]
    pub fn scriptlet(&self, did: Did) -> Option<Arc<Scriptlet>> {
        if did.prefix() != tessera_formats::script_table::SCRIPTLET_PREFIX {
            return None;
        }
        if let Some(t) = lock(&self.scriptlets).get(&did) {
            return t.clone();
        }
        let s = fetch(&self.set, did.0)
            .ok()
            .flatten()
            .and_then(|b| Scriptlet::parse_expecting(&b, did, &*self.dict).ok())
            .map(Arc::new);
        let mut m = lock(&self.scriptlets);
        if m.len() < MAX_CACHED_SCRIPTS {
            m.insert(did, s.clone());
        }
        s
    }

    fn cached_table(&self, did: Did) -> Option<Option<Arc<ScriptTable>>> {
        lock(&self.tables).get(&did).cloned()
    }

    fn cached_scriptlet(&self, did: Did) -> Option<Option<Arc<Scriptlet>>> {
        lock(&self.scriptlets).get(&did).cloned()
    }

    /// The tables `roots` and everything they include, pre-order, each once, at most
    /// [`MAX_CLOSURE_TABLES`].
    #[must_use]
    pub fn closure(&self, roots: &[Did]) -> Vec<Arc<ScriptTable>> {
        let mut out = Vec::new();
        let mut seen = HashSet::new();
        let mut stack: Vec<Did> = roots.iter().rev().copied().collect();
        while let Some(d) = stack.pop() {
            if out.len() >= MAX_CLOSURE_TABLES {
                break;
            }
            if !seen.insert(d) {
                continue;
            }
            if let Some(t) = self.table(d) {
                stack.extend(t.includes.iter().rev().copied());
                out.push(t);
            }
        }
        out
    }

    fn scan_node(
        root: &tessera_formats::Node,
        out: &mut ScanOut,
        keys: &mut Vec<u32>,
        scriptlets: &mut Vec<Did>,
    ) {
        root.walk(&mut |n, _| match &n.data {
            NodeData::Particle(p) => {
                if let Some(d) = p.effect_did() {
                    let o = p.bone_offset;
                    let off = (o[0] * o[0] + o[1] * o[1] + o[2] * o[2]).sqrt();
                    let off = if off.is_finite() {
                        off.min(MAX_REACH)
                    } else {
                        0.0
                    };
                    let fork_reach = if p.far_fade_max.is_finite() {
                        p.far_fade_max.clamp(0.0, MAX_REACH)
                    } else {
                        0.0
                    };
                    out.effects.push((
                        d,
                        if d.prefix() == FORK_SYSTEM_PREFIX {
                            fork_reach + off
                        } else {
                            off
                        },
                    ));
                }
                out.attaches |= p.holding_location != 0 || p.bone_type != 0;
            }
            NodeData::Run(r) if !r.stop => keys.push(r.script),
            NodeData::Scriptlet { file, .. } if !file.is_null() => scriptlets.push(*file),
            _ => {}
        });
    }

    /// Walks the scripts `keys` of `closure`, every script their Run nodes start and every
    /// scriptlet they invoke (bounded), collecting the effect DIDs of their Particle nodes.
    fn scan(&self, closure: &[Arc<ScriptTable>], keys: &[u32]) -> ScanOut {
        let mut out = ScanOut::default();
        let mut todo: Vec<u32> = keys.to_vec();
        let mut seen_keys = HashSet::new();
        let mut scriptlets: Vec<Did> = Vec::new();
        let mut seen_scriptlets = HashSet::new();
        loop {
            if let Some(k) = todo.pop() {
                if seen_keys.len() >= MAX_SCAN_KEYS || !seen_keys.insert(k) {
                    continue;
                }
                for t in closure {
                    if let Some(s) = t.script(k) {
                        for r in &s.roots {
                            Self::scan_node(r, &mut out, &mut todo, &mut scriptlets);
                        }
                    }
                }
            } else if let Some(d) = scriptlets.pop() {
                if seen_scriptlets.len() >= MAX_SCAN_SCRIPTLETS || !seen_scriptlets.insert(d) {
                    continue;
                }
                if let Some(s) = self.scriptlet(d) {
                    for r in &s.roots {
                        Self::scan_node(r, &mut out, &mut todo, &mut scriptlets);
                    }
                }
            } else {
                break;
            }
        }
        out
    }

    /// The scripts of `object` (with `instance`, a figure's instance property set, added
    /// last), when its start-up scripts reach a Particle node; every effect record they
    /// name is loaded on the way (the reach needs their fade distances).
    fn owner_scripts(&self, object: Did, instance: Option<&PropertySet>) -> Option<OwnerScripts> {
        let v = self.svocab.as_ref()?;
        let chain =
            collect_entity_scripts(object, &*self.dict, v, |d: Did| fetch(&self.set, d.0)).ok()?;
        let mut s = chain.scripts;
        if let Some(set) = instance {
            s.add_property_set(set, v);
        }
        let tables = s.tables();
        if tables.is_empty() {
            return None;
        }
        let closure = self.closure(&tables);
        let mut keys: Vec<u32> = self.startup_key.into_iter().collect();
        keys.extend_from_slice(&s.startup_scripts);
        let scan = self.scan(&closure, &keys);
        if scan.effects.is_empty() {
            return None;
        }
        let mut reach: Option<f32> = None;
        for (did, extra) in &scan.effects {
            let r = match self.record(did.0).as_deref() {
                Some(EffectRecord::Particles { desc, .. }) => {
                    let f = desc.far_fade_end;
                    if f.is_finite() {
                        f.clamp(0.0, MAX_REACH) + extra
                    } else {
                        DEFAULT_REACH
                    }
                }
                Some(EffectRecord::Fork(_)) => *extra,
                _ => continue,
            };
            reach = Some(reach.map_or(r, |x: f32| x.max(r)));
        }
        Some(OwnerScripts {
            tables,
            startup_scripts: s.startup_scripts.clone(),
            properties: s.properties.clone(),
            reach: reach.unwrap_or(DEFAULT_REACH).min(MAX_REACH),
            attaches: scan.attaches,
        })
    }

    /// The effect scripts of an entity object without an instance set, read once.
    fn object_scripts(&self, object: Did) -> Option<Arc<OwnerScripts>> {
        if let Some(o) = lock(&self.objects).get(&object) {
            return o.clone();
        }
        let o = self.owner_scripts(object, None).map(Arc::new);
        let mut m = lock(&self.objects);
        if m.len() < MAX_CACHED_OBJECTS {
            m.insert(object, o.clone());
        }
        o
    }

    /// The attachment points of setup `setup`, read once.
    #[must_use]
    pub fn setup_points(&self, setup: Did) -> Option<Arc<AttachPoints>> {
        if let Some(p) = lock(&self.points).get(&setup) {
            return p.clone();
        }
        let p = fetch(&self.set, setup.0).ok().flatten().and_then(|b| {
            let s = Setup::parse_expecting(&b, setup).ok()?;
            Some(Arc::new(AttachPoints::from_setup(&s, &b)))
        });
        let mut m = lock(&self.points);
        if m.len() < MAX_CACHED_OBJECTS {
            m.insert(setup, p.clone());
        }
        p
    }

    /// The template values along `chain` (the visual chain of a resolved object,
    /// `setup.md` §5.2: they multiply), read once per object.
    fn model_scales(&self, object: Did, chain: &[Did]) -> ModelScales {
        if let Some(s) = lock(&self.scales).get(&object) {
            return *s;
        }
        let mut s = ModelScales::default();
        for d in chain.iter().filter(|d| d.prefix() == TEMPLATE_PREFIX) {
            let Some(t) = fetch(&self.set, d.0)
                .ok()
                .flatten()
                .and_then(|b| VisualTemplate::parse_expecting(&b, *d).ok())
            else {
                continue;
            };
            if let Some(v) = t.vec3 {
                for (a, b) in s.model.iter_mut().zip(v) {
                    *a *= b;
                }
            }
            if let Some(b) = t.value_b {
                s.particle *= b;
            }
        }
        let fix = |v: f32| if v.is_finite() && v > 0.0 { v } else { 1.0 };
        let s = ModelScales {
            model: s.model.map(fix),
            particle: fix(s.particle),
        };
        let mut m = lock(&self.scales);
        if m.len() < MAX_CACHED_OBJECTS {
            m.insert(object, s);
        }
        s
    }

    /// An effect record (0x39 with its surface, or 0x44), read once; `None` when missing
    /// or malformed (logged).
    #[must_use]
    pub fn record(&self, did: u32) -> Option<Arc<EffectRecord>> {
        if let Some(r) = lock(&self.records).get(&did) {
            return r.clone();
        }
        let r = self.read_record(did).map(Arc::new);
        let mut m = lock(&self.records);
        if m.len() < MAX_CACHED_OBJECTS {
            m.insert(did, r.clone());
        }
        r
    }

    /// A record already in the cache: `Some(None)` for one that failed.
    #[must_use]
    pub fn cached_record(&self, did: u32) -> Option<Option<Arc<EffectRecord>>> {
        lock(&self.records).get(&did).cloned()
    }

    fn read_record(&self, did: u32) -> Option<EffectRecord> {
        let bytes = fetch(&self.set, did).ok().flatten()?;
        match (did >> 24) as u8 {
            PARTICLE_SYSTEM_PREFIX => {
                let desc = match ParticleSystem::decode(&bytes) {
                    Ok(d) => d,
                    Err(e) => {
                        tracing::debug!(did = format!("{did:#010x}"), "particle record: {e}");
                        return None;
                    }
                };
                let surface = if desc.surface.is_null() {
                    desc.extra_surface
                } else {
                    desc.surface
                };
                let own_alpha = |d: Did| TexRequest {
                    did: d.0,
                    variant: VARIANT_OWN_ALPHA,
                };
                let resolved = if surface.is_null() {
                    None
                } else {
                    self.materials
                        .resolve_surface(surface)
                        .map_err(|e| tracing::debug!(%surface, "particle surface: {e}"))
                        .ok()
                };
                Some(match resolved {
                    Some(m) => {
                        let look = ParticleSurface::of(&m);
                        EffectRecord::Particles {
                            desc: Arc::new(desc),
                            texture: look.texture.map(own_alpha),
                            second_texture: look.second_layer.map(own_alpha),
                            alpha: surface_alpha(&m),
                            shading: particle_shading(&look),
                            material: look.material_colour,
                            drawable: look.is_drawn(),
                        }
                    }
                    None => EffectRecord::Particles {
                        desc: Arc::new(desc),
                        texture: None,
                        second_texture: None,
                        alpha: AlphaMode::Blend,
                        shading: ParticleShading::STANDARD,
                        material: [1.0; 4],
                        drawable: false,
                    },
                })
            }
            FORK_SYSTEM_PREFIX => ForkSystem::decode(&bytes, &tessera_formats::NoSchema)
                .map_err(|e| tracing::debug!(did = format!("{did:#010x}"), "fork record: {e}"))
                .ok()
                .map(|f| EffectRecord::Fork(Arc::new(f))),
            _ => None,
        }
    }

    /// What landblock `bundle` contributes: its static entities and figures whose start-up
    /// scripts reach Particle nodes (content layers as the world's, interiors only when
    /// asked). Blocking (workers only).
    #[must_use]
    pub fn scan_block(&self, bundle: &LandblockBundle, identity: usize) -> BlockScan {
        let key = bundle.key;
        let origin = key.origin();
        let mut owners = Vec::new();
        let figure_of: HashMap<u32, (u32, Option<Arc<PropertySet>>)> = bundle
            .figures
            .iter()
            .enumerate()
            .filter_map(|(i, f)| Some((f.entity, (u32::try_from(i).ok()?, f.instance.clone()))))
            .collect();
        for (ei, e) in bundle.entities.iter().enumerate() {
            if owners.len() >= MAX_OWNERS_PER_BLOCK {
                break;
            }
            if e.interior && !self.interiors {
                continue;
            }
            let layers_pass = match e.content_layers.as_deref() {
                None | Some([]) => true,
                Some(l) => l.iter().any(|x| self.active_layers.contains(x)),
            };
            if !layers_pass {
                continue;
            }
            let Some(object) = e.object else {
                continue;
            };
            let figure = u32::try_from(ei).ok().and_then(|i| figure_of.get(&i));
            let scripts = match figure {
                Some((_, Some(instance))) => {
                    self.owner_scripts(object, Some(instance)).map(Arc::new)
                }
                _ if object.prefix() == ENTITY_PREFIX || figure.is_some() => {
                    self.object_scripts(object)
                }
                _ => None,
            };
            let Some(scripts) = scripts else {
                continue;
            };
            let chain: &[Did] = e.visual.as_deref().map_or(&[], |v| v.chain.as_slice());
            let scales = self.model_scales(object, chain);
            let pl = &e.placement;
            let position = [
                origin[0] + f64::from(pl.position[0]),
                origin[1] + f64::from(pl.position[1]),
                f64::from(pl.position[2]),
            ];
            let q = pl.rotation;
            let rotation = Quat::from_xyzw(q.x, q.y, q.z, q.w);
            let rotation = if rotation.is_finite() && rotation.length_squared() > 1e-12 {
                rotation.normalize()
            } else {
                Quat::IDENTITY
            };
            let (id, kind, points) = match figure {
                Some((fi, _)) => (
                    EntityId(figure_tag(key, *fi)),
                    OwnerKind::Figure {
                        tag: figure_tag(key, *fi),
                    },
                    None,
                ),
                None => {
                    let points = if scripts.attaches {
                        e.visual
                            .as_ref()
                            .and_then(|v| v.setup)
                            .and_then(|s| self.setup_points(s))
                    } else {
                        None
                    };
                    let mut scale = e.scale;
                    for (a, b) in scale.iter_mut().zip(scales.model) {
                        *a *= b;
                    }
                    (
                        static_owner_id(key, e.index),
                        OwnerKind::Static {
                            position,
                            rotation,
                            scale,
                        },
                        points,
                    )
                }
            };
            owners.push(EffectOwner {
                id,
                kind,
                iid: e.iid.map(|i| i.0),
                scripts,
                points,
                scales,
                position,
            });
        }
        BlockScan {
            key,
            bundle: identity,
            owners,
        }
    }
}

/// The runtime's script source: [`EffectData`]'s caches, reading on a miss (counted).
#[derive(Debug)]
pub struct EffectSource(pub Arc<EffectData>);

impl ScriptSource for EffectSource {
    fn table(&mut self, did: Did) -> Option<Arc<ScriptTable>> {
        if let Some(t) = self.0.cached_table(did) {
            return t;
        }
        self.0.main_thread_reads.fetch_add(1, Ordering::Relaxed);
        self.0.table(did)
    }

    fn scriptlet(&mut self, did: Did) -> Option<Arc<Scriptlet>> {
        if let Some(s) = self.0.cached_scriptlet(did) {
            return s;
        }
        self.0.main_thread_reads.fetch_add(1, Ordering::Relaxed);
        self.0.scriptlet(did)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn owner_ids_do_not_collide_with_figure_tags() {
        let key = LandblockKey::new(1, 10, 20).expect("key");
        let s = static_owner_id(key, 5);
        assert_ne!(s.0, figure_tag(key, 5));
        assert_eq!(s.0 & STATIC_OWNER_BIT, STATIC_OWNER_BIT);
        assert_eq!(figure_tag(key, 5) & STATIC_OWNER_BIT, 0);
    }

    #[test]
    fn attach_points_find_locations_and_bones() {
        let p = AttachPoints {
            holding: vec![(7, None, Mat4::from_translation([0.0, 0.0, 2.0].into()))],
            bones: vec![(3, 0)],
            reference: vec![Mat4::IDENTITY],
        };
        assert_eq!(p.holding(7).map(|(b, _)| b), Some(None));
        assert!(p.holding(8).is_none());
        assert_eq!(p.bone(3), Some(0));
        assert_eq!(p.bone(4), None);
    }

    /// `particle-runtime.md` §9.1.1: the renderer receives the program of the surface's
    /// shader record.
    #[test]
    fn particle_shading_follows_the_surface_program() {
        use tessera_particles::surface::SurfaceProgram;
        let surface = |shader: u32| ParticleSurface {
            program: SurfaceProgram::of(shader).expect("listed"),
            material_colour: [1.0; 4],
            texture: None,
            second_layer: None,
        };
        let doubled = particle_shading(&surface(0x2B00_0703));
        assert!(doubled.doubled && doubled.fogged);
        assert_eq!(doubled.layers, Some(ParticleLayers::Product));
        let additive = particle_shading(&surface(0x2B00_0717));
        assert!(!additive.fogged);
        assert_eq!(additive.layers, Some(ParticleLayers::Sum));
        let colour_only = particle_shading(&surface(0x2B00_07F2));
        assert!(!colour_only.texture_alpha && !colour_only.tests_texture);
        let no_vertex = particle_shading(&surface(0x2B00_0755));
        assert!(!no_vertex.vertex_colour && no_vertex.material_colour);
        assert!(!surface(0x2B00_0084).is_drawn());
    }

    /// A resolved unlit particle material with the given blend and alpha test.
    fn particle_material(
        mode: BlendMode,
        dst: MatFactor,
        depth_write: bool,
        test: Option<f32>,
    ) -> ResolvedMaterial {
        use tessera_material::{
            AlphaTest, BlendState, FeatureFlags, ParameterSet, ShaderInterface,
        };
        let shader = Arc::new(ShaderInterface {
            did: Did(0x2B00_0001),
            declarations: Vec::new(),
            slots: Vec::new(),
            permutation_count: 1,
            lit: false,
            common_blend: None,
            cull_none: false,
            default_alpha_ref: None,
        });
        let params = ParameterSet::build(&shader, &[]);
        ResolvedMaterial {
            surface: Some(Did(0x3100_0001)),
            binding_key: Some(1),
            shader,
            materials: Vec::new(),
            skipped: Vec::new(),
            params,
            slots: Vec::new(),
            features: FeatureFlags {
                lit: false,
                alpha_test: test.map(|threshold| AlphaTest {
                    threshold,
                    source_slot: 3000,
                }),
                opacity_slot: Some(3000),
                blend: BlendState {
                    mode,
                    src: MatFactor::SrcAlpha,
                    dst,
                    op: MatOp::Add,
                    depth_write,
                    opacity_slot: Some(3000),
                },
                two_sided: true,
                normal_mapped: false,
                specular: false,
                dyeable: false,
                dyeable_2: false,
                emissive: false,
                glow: false,
                environment: false,
                second_layer: false,
                baked_lighting: false,
                uv_transform: false,
                animated: false,
            },
            dye: None,
        }
    }

    /// The alpha-test reference the renderer applies to a draw of `mode` at *A* = `a`
    /// (`particle-runtime.md` §9.1.1 rule 1 through `material.md` §11.2).
    fn reference(mode: AlphaMode, a: f32) -> Option<f32> {
        let (test, depth_write) = match mode {
            AlphaMode::Fixed(f) => (f.alpha_test.map(|t| t.value()), f.depth_write),
            AlphaMode::Blend | AlphaMode::Additive => (None, false),
            other => panic!("unexpected particle mode {other:?}"),
        };
        tessera_render::draw_reference(test, depth_write, true, a)
    }

    /// `particle-runtime.md` §9.1.1 rule 1: a surface with alpha test on and depth write
    /// off keeps its material's reference while *A* ≥ 0.9 and tests nothing but opacity 0
    /// below; without alpha test the reference is 0.
    #[test]
    fn alpha_tested_surfaces_without_depth_write_keep_their_reference() {
        let r = (0.5_f32 * 255.0).floor() / 255.0;
        for (mode, dst, factor) in [
            (
                BlendMode::AlphaBlend,
                MatFactor::InvSrcAlpha,
                BlendFactor::OneMinusSrcAlpha,
            ),
            (BlendMode::Additive, MatFactor::One, BlendFactor::One),
        ] {
            let alpha = surface_alpha(&particle_material(mode, dst, false, Some(0.5)));
            let AlphaMode::Fixed(f) = alpha else {
                panic!("{mode:?}: {alpha:?}");
            };
            assert_eq!(f.src, BlendFactor::SrcAlpha);
            assert_eq!(f.dst, factor);
            assert_eq!(f.op, BlendOp::Add);
            assert!(!f.depth_write);
            assert_eq!(reference(alpha, 1.0), Some(r), "{mode:?}");
            assert_eq!(reference(alpha, 0.9), Some(r), "{mode:?}");
            assert_eq!(reference(alpha, 0.85), Some(0.0), "{mode:?}");

            let untested = surface_alpha(&particle_material(mode, dst, false, None));
            assert!(matches!(untested, AlphaMode::Blend | AlphaMode::Additive));
            assert_eq!(reference(untested, 1.0), Some(0.0), "{mode:?}");
        }
        // Depth write on: the reference is scaled by *A*.
        let written = surface_alpha(&particle_material(
            BlendMode::Additive,
            MatFactor::One,
            true,
            Some(0.5),
        ));
        assert_eq!(
            reference(written, 0.5),
            Some((0.25_f32 * 255.0).floor() / 255.0)
        );
    }
}
