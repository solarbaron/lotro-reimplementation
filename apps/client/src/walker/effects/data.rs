// SPDX-License-Identifier: GPL-3.0-or-later
//! The data side of particle effects in the walker, run on the asset workers: script tables
//! and scriptlets, the owners whose start-up scripts reach Particle nodes, their attachment
//! points and scales, and the effect records with the texture and blend state of their
//! surfaces (`particle-runtime.md` §5, §6.4, §9.1; `particles.md` §4.1, §7.1.1).
//!
//! A landblock scan also finds the static entities whose start-up scripts reach a SetProperty
//! of their own `Render_Opacity` but no Particle node (`particle-runtime.md` §12 rule 15,
//! answer item 2 under the anchor `s12-r15-detect`; `script-table.md` §7.7): they are owners
//! too, run while the object layer places them rather than by reach ([`RunScope::Placed`]).
//!
//! Every record is read once through [`EffectData`]'s caches. The runtime's
//! [`ScriptSource`] ([`EffectSource`]) reads from the same caches and counts the reads it
//! has to make on the main thread.

use std::collections::{HashMap, HashSet};
use std::sync::atomic::{AtomicU64, Ordering};
use std::sync::{Arc, Mutex, MutexGuard};

use tessera_anim::AttachFrame;
use tessera_dat::ArchiveSet;
use tessera_formats::PropertyValue;
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
use tessera_material::records::{
    BlendFactor as MatFactor, BlendOp as MatOp, SURFACE_RECORD_PREFIX,
};
use tessera_material::{
    BlendMode, CurveStates, MaterialLibrary, ResolvedMaterial, TextureMatrix, TextureMatrixLayout,
};
use tessera_particles::Ran2;
use tessera_particles::glam::{Mat4, Quat};
use tessera_particles::surface::{LayerCombine, ParticleSurface};
use tessera_render::{
    AlphaMode, BlendFactor, BlendOp, FixedBlend, ParticleLayers, ParticleShading, UvTransform,
};
use tessera_scriptgraph::{EntityId, ScriptSource};
use tessera_world::{EntityRef, LandblockBundle, LandblockKey};

use crate::audio::loader::fetch;
use crate::walker::figures::figure_tag;
use crate::walker::geometry::{TexRequest, VARIANT_OWN_ALPHA};
use crate::walker::objects::PlacedObjectRef;

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
/// Most effect owners ([`RunScope::Reach`]) taken from one landblock.
pub const MAX_OWNERS_PER_BLOCK: usize = 1024;
/// Most static entities run while placed ([`RunScope::Placed`]) taken from one landblock: a
/// bound of their own, so that writers never take an effect owner's place in a scan.
pub const MAX_WRITERS_PER_BLOCK: usize = 1024;
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

/// How an owner a scan found is run (`particle-runtime.md` §3.2 and §12 rule 15).
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum RunScope {
    /// An effect owner: its start-up scripts reach Particle nodes, and it runs while the
    /// viewer is within its reach (§3.2, walker application of the Q-RT1 Decision).
    Reach,
    /// A static entity that is no effect owner but whose start-up scripts reach a
    /// SetProperty of `Render_Opacity` (rule 15, answer item 2 under the anchor
    /// `s12-r15-detect`): it runs from the frame in which the object layer places its object
    /// until the object is removed, whatever the viewer's distance.
    Placed,
}

/// The scripts of an owner whose start-up scripts reach Particle nodes, or (with
/// [`RunScope::Placed`]) write the owner's `Render_Opacity`.
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
    /// How it is run. [`RunScope::Placed`] scripts reach no Particle node: their
    /// [`Self::reach`] is 0 and unused, [`Self::attaches`] false.
    pub scope: RunScope,
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
    /// The figure of a session entity (a scenario or test-server actor, the player), by its
    /// network ID ([`super::session`]).
    Session {
        /// The network ID.
        net: u64,
    },
}

/// The placed object a static owner of a landblock is drawn as: its landblock, its entity
/// index and the entity's identity in the object layer's block list
/// (`ObjectLayer::placed_entity_opacity`, `particle-runtime.md` §12 rule 15). An owner that
/// has no drawn object of its own (no visual, or not drawn) carries no parts, so it counts
/// as having no drawn object (factor 1).
#[derive(Debug, Clone, PartialEq)]
pub struct PlacedObject {
    /// The landblock.
    pub key: LandblockKey,
    /// The entity's index in its landblock's info record (`tessera_world::PlacedEntity::index`),
    /// the `ObjectInfo::entity` of its object:
    /// the object layer finds the owner's own object by it, even when another entity is
    /// placed at the same position with the same parts. `None` falls back to [`Self::entity`].
    pub index: Option<u32>,
    /// The entity: placement position, parts and rank (`PlacedObjectRef`).
    pub entity: PlacedObjectRef,
    /// The owner property `Render_Opacity` of the entity's property sets as the object layer
    /// draws it (`tessera_world::LandblockBundle::entity_render_opacities`: the per-instance
    /// set, then the 0x47 chain, clamped; `None`: none holds a usable value, factor 1). The
    /// runtime's stored value is made to agree with it (rule 15, [`super::opacity`]).
    pub property_opacity: Option<f32>,
}

impl PlacedObject {
    /// The placed object of static owner `e` of landblock `key` with its drawn `parts` and
    /// `rank` ([`EffectData::scan_block`]): its index is the entity's own
    /// (`PlacedEntity::index`, the value the object layer's block list gives the object of a
    /// placed, not generated, entity as `ObjectInfo::entity`).
    /// It carries no [`Self::property_opacity`] ([`Self::with_property_opacity`]).
    #[must_use]
    pub fn of_entity(
        key: LandblockKey,
        e: &tessera_world::PlacedEntity,
        parts: Arc<[u32]>,
        rank: u32,
    ) -> Self {
        Self {
            key,
            index: Some(e.index),
            entity: PlacedObjectRef {
                position: e.placement.position,
                parts,
                rank,
            },
            property_opacity: None,
        }
    }

    /// With [`Self::property_opacity`] `value`.
    #[must_use]
    pub fn with_property_opacity(mut self, value: Option<f32>) -> Self {
        self.property_opacity = value;
        self
    }
}

/// The part list of an entity as the object layer draws it: its visual's parts when it is
/// drawn (`draw`, a finite placement position and at least one part, the conditions under
/// which the layer's block list holds an object for it), else empty.
fn drawn_parts(e: &tessera_world::PlacedEntity) -> Vec<u32> {
    if !e.draw || !e.placement.position.iter().all(|v| v.is_finite()) {
        return Vec::new();
    }
    e.visual
        .as_deref()
        .map_or_else(Vec::new, |v| v.parts.iter().map(|p| p.0).collect())
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
    /// For a static owner of a landblock, the placed object it is drawn as (its opacity is
    /// the parent opacity of rule 15); `None` for figures and the creation viewport's
    /// owners.
    pub placed: Option<PlacedObject>,
}

/// What a landblock contributes.
#[derive(Debug, Clone, PartialEq)]
pub struct BlockScan {
    /// The landblock.
    pub key: LandblockKey,
    /// Identity of the bundle scanned (its address), to drop stale results.
    pub bundle: usize,
    /// Effect owners ([`RunScope::Reach`]) and the static entities run while placed for their
    /// `Render_Opacity` writes ([`RunScope::Placed`]), in entity order.
    pub owners: Vec<EffectOwner>,
    /// Static entities whose start-up scripts reach a SetProperty of `Render_Opacity` but no
    /// Particle node, and that have no object the object layer could place (no drawn part):
    /// they are not run, since their writes would reach no object
    /// ([`super::opacity::WriteCounts::writers_without_object`]).
    pub writers_without_object: u32,
    /// Whether the scan looked for effect owners. A scan made while the landblock is out of
    /// the effect scope (`super::effects_scope`) looks for the static entities run while
    /// placed alone ([`ScanDepth::Writers`]); the layer scans the landblock again in full
    /// when it enters the scope.
    pub effects_scanned: bool,
    /// How many of [`Self::owners`] are effect owners ([`RunScope::Reach`]), counted once at
    /// the scan so that the per-frame statistics do not walk the owners.
    pub effect_owner_count: usize,
    /// How many of [`Self::owners`] are run while placed ([`RunScope::Placed`]).
    pub writer_count: usize,
}

/// How much a landblock scan looks for ([`scan_bundle`]).
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ScanDepth {
    /// Effect owners and the static entities run while placed.
    Full,
    /// The static entities run while placed alone (a landblock out of the effect scope):
    /// figures are skipped, and an entity whose start-up scripts reach a Particle node is
    /// noted without loading its effect records ([`OwnerLookup::Unmeasured`]).
    Writers,
}

/// What an [`OwnerSource`] knows of an entity object's start-up scripts.
#[derive(Debug, Clone, PartialEq)]
pub enum OwnerLookup {
    /// Its scripts make it an owner of the given scope.
    Scripts(Arc<OwnerScripts>),
    /// An effect owner whose reach was not measured ([`ScanDepth::Writers`] only).
    Unmeasured,
    /// Neither.
    None,
}

impl OwnerLookup {
    fn scripts(self) -> Option<Arc<OwnerScripts>> {
        match self {
            Self::Scripts(s) => Some(s),
            Self::Unmeasured | Self::None => None,
        }
    }
}

/// What [`scan_bundle`] reads per entity ([`EffectData`], or a synthetic source in tests).
pub trait OwnerSource {
    /// Whether interior entities are scanned.
    fn interiors(&self) -> bool;
    /// Whether content layer `layer` is active.
    fn layer_active(&self, layer: u32) -> bool;
    /// The start-up scripts of entity object `object`: with `instance` (a figure's
    /// instance property set, added last) when given; at [`ScanDepth::Writers`] an effect
    /// owner may come back as [`OwnerLookup::Unmeasured`].
    fn lookup(&self, object: Did, instance: Option<&PropertySet>, depth: ScanDepth) -> OwnerLookup;
    /// The template scales along `chain` of `object`.
    fn model_scales(&self, object: Did, chain: &[Did]) -> ModelScales;
    /// The attachment points of setup `setup`.
    fn setup_points(&self, setup: Did) -> Option<Arc<AttachPoints>>;
}

impl BlockScan {
    /// The effect owners ([`RunScope::Reach`]).
    pub fn effect_owners(&self) -> impl Iterator<Item = &EffectOwner> {
        self.owners
            .iter()
            .filter(|o| o.scripts.scope == RunScope::Reach)
    }

    /// The static entities run while placed ([`RunScope::Placed`]).
    pub fn opacity_writers(&self) -> impl Iterator<Item = &EffectOwner> {
        self.owners
            .iter()
            .filter(|o| o.scripts.scope == RunScope::Placed)
    }
}

/// The resolved material and texture-matrix bindings of a particle surface whose values
/// follow the clock ([`ParticleSurface::is_animated`]): the alpha-test reference 1043 or
/// texture matrix 0 or 1 of a two-layer program (`particle-runtime.md` §9.1.1 answers (d),
/// (e)).
#[derive(Debug)]
pub struct SurfaceClock {
    /// The surface at time 0.
    pub look: ParticleSurface,
    /// Its resolved material.
    pub material: Arc<ResolvedMaterial>,
    /// Its shader record's texture-matrix bindings.
    pub layout: Arc<TextureMatrixLayout>,
}

impl SurfaceClock {
    /// The alpha mode and texture matrices of a draw at the material clock `t`, with the
    /// curve states of the surface record (`particle-runtime.md` §12 rule 12) and the
    /// shared stream `rng` for mode-3 curves.
    #[must_use]
    pub fn at(
        &self,
        t: f64,
        states: &mut CurveStates,
        rng: &mut Ran2,
    ) -> (AlphaMode, [UvTransform; 2]) {
        let s = self.look.at(&self.material, &self.layout, t, states, rng);
        (
            surface_alpha(&self.material, s.alpha_test),
            uv_transforms(s.matrices),
        )
    }
}

/// Texture matrices as the renderer takes them.
#[must_use]
pub fn uv_transforms(m: [TextureMatrix; 2]) -> [UvTransform; 2] {
    m.map(|m| UvTransform { rows: m.rows }.sanitized())
}

/// How a surface record draws through the particle pipeline (`particle-runtime.md` §9.1–
/// §9.1.3): the surface of a 0x39 system, or a beam ribbon's material (§11.1 step 6, drawn
/// with the same programs; §11.1 Decision (b)).
#[derive(Debug)]
pub struct SurfaceLook {
    /// The surface's colour texture (slot 3000) with its own alpha.
    pub texture: Option<TexRequest>,
    /// The second colour layer (slot 3001) of a two-layer program.
    pub second_texture: Option<TexRequest>,
    /// The surface's blend, depth write and alpha test (§9.1.3) at time 0.
    pub alpha: AlphaMode,
    /// What the surface's program computes (§9.1.1).
    pub shading: ParticleShading,
    /// Material colour 2000.
    pub material: [f32; 4],
    /// Texture matrices 0 and 1 of a two-layer program at time 0 (identity otherwise).
    pub uv: [UvTransform; 2],
    /// For a surface whose reference or matrices follow the clock: what re-evaluates
    /// `alpha` and `uv` per draw.
    pub clock: Option<Box<SurfaceClock>>,
    /// Whether the surface is drawn.
    pub drawable: bool,
}

impl SurfaceLook {
    /// The look of a system without a surface: not drawn (`particles.md` §8.11).
    #[must_use]
    pub fn none() -> Self {
        Self {
            texture: None,
            second_texture: None,
            alpha: AlphaMode::Blend,
            shading: ParticleShading::STANDARD,
            material: [1.0; 4],
            uv: [UvTransform::IDENTITY; 2],
            clock: None,
            drawable: false,
        }
    }

    /// The look of a resolved surface material `m` with its shader's texture-matrix
    /// bindings `layout`: a particle surface, or with `ribbon` a beam ribbon's material
    /// ([`ParticleSurface::of_ribbon`]).
    #[must_use]
    pub fn of(m: &Arc<ResolvedMaterial>, layout: Arc<TextureMatrixLayout>, ribbon: bool) -> Self {
        let own_alpha = |d: Did| TexRequest::new(d.0, VARIANT_OWN_ALPHA);
        let look = if ribbon {
            ParticleSurface::of_ribbon(m, &layout)
        } else {
            ParticleSurface::of(m, &layout)
        };
        Self {
            texture: look.texture.map(own_alpha),
            second_texture: look.second_layer.map(own_alpha),
            alpha: surface_alpha(m, look.alpha_test),
            shading: particle_shading(&look),
            material: look.material_colour,
            uv: uv_transforms(look.matrices),
            drawable: look.is_drawn(),
            clock: look.is_animated().then(|| {
                Box::new(SurfaceClock {
                    look,
                    material: Arc::clone(m),
                    layout,
                })
            }),
        }
    }

    /// The textures the look draws with.
    pub fn textures(&self) -> impl Iterator<Item = TexRequest> + '_ {
        self.texture.iter().chain(&self.second_texture).copied()
    }
}

/// A loaded effect record (`particles.md` §4, §6), or a beam ribbon's surface record.
#[derive(Debug)]
pub enum EffectRecord {
    /// A 0x39 system with the textures, blend state and program of its surface
    /// (`particle-runtime.md` §9.1–§9.1.3); the look's `texture` is `None` without a surface
    /// texture, `drawable` false without a surface (the client draws nothing, `particles.md`
    /// §8.11) and for the heat-distortion program, which Tessera draws as its variant
    /// without feature bit 1 (it leaves the target unchanged, §9.1.2, rule 11).
    Particles {
        /// The record.
        desc: Arc<ParticleSystem>,
        /// Its surface's look.
        look: SurfaceLook,
    },
    /// A 0x44 fork effect (not simulated, `particles.md` rule §10.2).
    Fork(Arc<ForkSystem>),
    /// A 0x31 surface record named as a beam's head, body or tail material
    /// (`particle-runtime.md` §11.1 step 6); not an effect.
    Surface(SurfaceLook),
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
/// factors and depth write (`particle-runtime.md` §9.1: the draw takes these states from
/// the surface record's material, `material.md` §7, §11.3), with the alpha test `test` of
/// the particle surface ([`ParticleSurface::alpha_test`]: §9.1.3, the effective 4004 and
/// 1043 whatever the mesh opacity-source rule says). The renderer feeds the blend the
/// particle program's output alpha (§9.1.3 item 1).
pub fn surface_alpha(m: &ResolvedMaterial, test: Option<f32>) -> AlphaMode {
    let b = m.features.blend;
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

/// What the effect scan of an owner's scripts found.
#[derive(Debug, Default)]
pub(crate) struct ScanOut {
    /// Effect DIDs with the extra reach of their nodes (offset, fork fade).
    pub effects: Vec<(Did, f32)>,
    /// Whether a Particle node attaches to a holding location or a bone.
    pub attaches: bool,
    /// Whether a SetProperty node that is not `ServerOnly` writes [`ScanPairs::opacity`]
    /// (rule 15; `script-table.md` §7.7: a `ServerOnly` one sets nothing locally).
    pub writes_opacity: bool,
}

impl ScanOut {
    /// How an entity whose start-up scripts gave this scan is run: an effect owner when they
    /// reach a Particle node, else a writer run while placed when they reach a SetProperty
    /// of `Render_Opacity` (rule 15, answer item 2 under the anchor `s12-r15-detect`), else
    /// not at all.
    #[must_use]
    pub fn scope(&self) -> Option<RunScope> {
        if !self.effects.is_empty() {
            Some(RunScope::Reach)
        } else if self.writes_opacity {
            Some(RunScope::Placed)
        } else {
            None
        }
    }
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
    /// [`OwnerLookup::Scripts`] or [`OwnerLookup::None`] per entity object (never
    /// [`OwnerLookup::Unmeasured`]).
    objects: Mutex<HashMap<Did, OwnerLookup>>,
    /// Entity objects found to be effect owners by a [`ScanDepth::Writers`] scan, whose
    /// reach is not measured yet.
    unmeasured: Mutex<HashSet<Did>>,
    /// The PID of `Render_Opacity` (rule 15, [`ScanPairs::opacity`]).
    opacity_pid: Option<u32>,
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
        let opacity_pid = dict.pid("Render_Opacity");
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
            unmeasured: Mutex::new(HashSet::new()),
            opacity_pid,
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

    /// Walks the scripts `keys` of `closure`, every script their Run nodes start and every
    /// scriptlet they invoke (bounded), collecting the effect DIDs of their Particle nodes.
    /// A Scriptlet node's `ScriptletScript_File` pair is followed ([`scan_scripts`]).
    fn scan(&self, closure: &[Arc<ScriptTable>], keys: &[u32], owner: &PropertyState) -> ScanOut {
        let dict = &*self.dict;
        let pairs = ScanPairs {
            file: self.svocab.as_ref().and_then(|v| v.scriptlet.file),
            owner,
            default: &|pid| {
                dict.definition(pid)
                    .and_then(|d| d.default.as_ref())
                    .and_then(PropertyValue::as_did)
            },
            opacity: self.opacity_pid,
        };
        scan_scripts(closure, keys, &pairs, |d| self.scriptlet(d))
    }

    /// The scripts of `object` (with `instance`, a figure's instance property set, added
    /// last), when its start-up scripts reach a Particle node ([`RunScope::Reach`]) or, failing
    /// that, a SetProperty of `Render_Opacity` ([`RunScope::Placed`]); every effect record they
    /// name is loaded on the way (the reach needs their fade distances), except at
    /// [`ScanDepth::Writers`], where an effect owner is [`OwnerLookup::Unmeasured`].
    fn owner_scan(
        &self,
        object: Did,
        instance: Option<&PropertySet>,
        depth: ScanDepth,
    ) -> OwnerLookup {
        let Some(v) = self.svocab.as_ref() else {
            return OwnerLookup::None;
        };
        let Ok(chain) =
            collect_entity_scripts(object, &*self.dict, v, |d: Did| fetch(&self.set, d.0))
        else {
            return OwnerLookup::None;
        };
        let mut s = chain.scripts;
        if let Some(set) = instance {
            s.add_property_set(set, v);
        }
        let tables = s.tables();
        if tables.is_empty() {
            return OwnerLookup::None;
        }
        let closure = self.closure(&tables);
        let mut keys: Vec<u32> = self.startup_key.into_iter().collect();
        keys.extend_from_slice(&s.startup_scripts);
        let scan = self.scan(&closure, &keys, &s.properties);
        match scan.scope() {
            None => return OwnerLookup::None,
            Some(RunScope::Placed) => {
                // Rule 15, answer item 2 under the anchor `s12-r15-detect`.
                return OwnerLookup::Scripts(Arc::new(OwnerScripts {
                    tables,
                    startup_scripts: s.startup_scripts.clone(),
                    properties: s.properties.clone(),
                    reach: 0.0,
                    attaches: false,
                    scope: RunScope::Placed,
                }));
            }
            Some(RunScope::Reach) if depth == ScanDepth::Writers => {
                return OwnerLookup::Unmeasured;
            }
            Some(RunScope::Reach) => {}
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
                Some(EffectRecord::Surface(_)) | None => continue,
            };
            reach = Some(reach.map_or(r, |x: f32| x.max(r)));
        }
        OwnerLookup::Scripts(Arc::new(OwnerScripts {
            tables,
            startup_scripts: s.startup_scripts.clone(),
            properties: s.properties.clone(),
            reach: reach.unwrap_or(DEFAULT_REACH).min(MAX_REACH),
            attaches: scan.attaches,
            scope: RunScope::Reach,
        }))
    }

    /// The effect scripts of an entity object without an instance set
    /// ([`RunScope::Reach`] only), read once.
    fn object_scripts(&self, object: Did) -> Option<Arc<OwnerScripts>> {
        self.object_scan(object, ScanDepth::Full)
            .scripts()
            .filter(|s| s.scope == RunScope::Reach)
    }

    /// [`Self::owner_scan`] of an entity object without an instance set, read once (an
    /// [`OwnerLookup::Unmeasured`] answer is remembered apart, so that a later full scan
    /// still measures the reach).
    fn object_scan(&self, object: Did, depth: ScanDepth) -> OwnerLookup {
        if let Some(o) = lock(&self.objects).get(&object) {
            return o.clone();
        }
        if depth == ScanDepth::Writers && lock(&self.unmeasured).contains(&object) {
            return OwnerLookup::Unmeasured;
        }
        let o = self.owner_scan(object, None, depth);
        if matches!(o, OwnerLookup::Unmeasured) {
            let mut u = lock(&self.unmeasured);
            if u.len() < MAX_CACHED_OBJECTS {
                u.insert(object);
            }
        } else {
            let mut m = lock(&self.objects);
            if m.len() < MAX_CACHED_OBJECTS {
                m.insert(object, o.clone());
            }
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

    /// The look of surface record `surface` through the material library (with `ribbon`, as
    /// a beam ribbon's material); `None` when it does not resolve (logged).
    fn surface_look(&self, surface: Did, ribbon: bool) -> Option<SurfaceLook> {
        let m = self
            .materials
            .resolve_surface(surface)
            .map_err(|e| tracing::debug!(%surface, "particle surface: {e}"))
            .ok()?;
        let layout = self
            .materials
            .texture_matrices(m.shader.did)
            .map_err(|e| tracing::debug!(%surface, "particle matrices: {e}"))
            .unwrap_or_default();
        Some(SurfaceLook::of(&m, layout, ribbon))
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
                let look = if surface.is_null() {
                    SurfaceLook::none()
                } else {
                    self.surface_look(surface, false)
                        .unwrap_or_else(SurfaceLook::none)
                };
                Some(EffectRecord::Particles {
                    desc: Arc::new(desc),
                    look,
                })
            }
            SURFACE_RECORD_PREFIX => self.surface_look(Did(did), true).map(EffectRecord::Surface),
            FORK_SYSTEM_PREFIX => ForkSystem::decode(&bytes, &tessera_formats::NoSchema)
                .map_err(|e| tracing::debug!(did = format!("{did:#010x}"), "fork record: {e}"))
                .ok()
                .map(|f| EffectRecord::Fork(Arc::new(f))),
            _ => None,
        }
    }

    /// What landblock `bundle` contributes ([`scan_bundle`] with these caches). Blocking
    /// (workers only).
    #[must_use]
    pub fn scan_block(
        &self,
        bundle: &LandblockBundle,
        identity: usize,
        depth: ScanDepth,
    ) -> BlockScan {
        scan_bundle(self, bundle, identity, depth)
    }
}

impl OwnerSource for EffectData {
    fn interiors(&self) -> bool {
        self.interiors
    }

    fn layer_active(&self, layer: u32) -> bool {
        self.active_layers.contains(&layer)
    }

    fn lookup(&self, object: Did, instance: Option<&PropertySet>, depth: ScanDepth) -> OwnerLookup {
        match instance {
            Some(set) => self.owner_scan(object, Some(set), depth),
            None => self.object_scan(object, depth),
        }
    }

    fn model_scales(&self, object: Did, chain: &[Did]) -> ModelScales {
        EffectData::model_scales(self, object, chain)
    }

    fn setup_points(&self, setup: Did) -> Option<Arc<AttachPoints>> {
        EffectData::setup_points(self, setup)
    }
}

/// What landblock `bundle` contributes: its static entities and figures whose start-up
/// scripts reach Particle nodes (not at [`ScanDepth::Writers`]), and its static entities
/// with a drawn object whose start-up scripts reach a SetProperty of `Render_Opacity` but no
/// Particle node ([`RunScope::Placed`]; content layers as the world's, interiors only when
/// asked, as for the effect owners). At most [`MAX_OWNERS_PER_BLOCK`] effect owners and
/// [`MAX_WRITERS_PER_BLOCK`] writers, each in entity order.
#[must_use]
pub fn scan_bundle<S: OwnerSource + ?Sized>(
    src: &S,
    bundle: &LandblockBundle,
    identity: usize,
    depth: ScanDepth,
) -> BlockScan {
    let key = bundle.key;
    let origin = key.origin();
    let mut owners = Vec::new();
    let mut writers_without_object = 0u32;
    let (mut effect_owner_count, mut writer_count) = (0usize, 0usize);
    let figure_of: HashMap<u32, (u32, Option<Arc<PropertySet>>)> = bundle
        .figures
        .iter()
        .enumerate()
        .filter_map(|(i, f)| match f.entity {
            // Start-up scripts are scanned for the info record's entities only.
            EntityRef::Entity(e) => Some((e, (u32::try_from(i).ok()?, f.instance.clone()))),
            EntityRef::Generated(_) => None,
        })
        .collect();
    // Rule 15: the rank of each drawn entity among the earlier ones with the same
    // position and parts (`ObjectLayer::placed_entity_opacity`'s fallback when the
    // object layer's objects carry no entity index), counted over every
    // entity, owner or not.
    let mut seen: HashMap<([u32; 3], Arc<[u32]>), u32> = HashMap::new();
    for (ei, e) in bundle.entities.iter().enumerate() {
        let effects_full =
            depth == ScanDepth::Writers || effect_owner_count >= MAX_OWNERS_PER_BLOCK;
        if effects_full && writer_count >= MAX_WRITERS_PER_BLOCK {
            break;
        }
        let parts: Arc<[u32]> = drawn_parts(e).into();
        let rank = if parts.is_empty() {
            0
        } else {
            let n = seen
                .entry((e.placement.position.map(f32::to_bits), Arc::clone(&parts)))
                .or_insert(0);
            let r = *n;
            *n = n.saturating_add(1);
            r
        };
        if e.interior && !src.interiors() {
            continue;
        }
        let layers_pass = match e.content_layers.as_deref() {
            None | Some([]) => true,
            Some(l) => l.iter().any(|x| src.layer_active(*x)),
        };
        if !layers_pass {
            continue;
        }
        let Some(object) = e.object else {
            continue;
        };
        let figure = u32::try_from(ei).ok().and_then(|i| figure_of.get(&i));
        // A figure is never run while placed (below), so a writers' scan skips it.
        if figure.is_some() && depth == ScanDepth::Writers {
            continue;
        }
        let lookup = match figure {
            Some((_, Some(instance))) => src.lookup(object, Some(instance), depth),
            _ if object.prefix() == ENTITY_PREFIX || figure.is_some() => {
                src.lookup(object, None, depth)
            }
            _ => OwnerLookup::None,
        };
        let Some(scripts) = lookup.scripts() else {
            continue;
        };
        match scripts.scope {
            RunScope::Reach => {
                if effects_full {
                    continue;
                }
            }
            RunScope::Placed => {
                // Rule 15 asks for the writes of static entities on their placed object: a
                // figure is drawn by the character layer, and an entity without a drawn
                // part has no object the object layer places.
                if figure.is_some() {
                    continue;
                }
                if parts.is_empty() {
                    writers_without_object = writers_without_object.saturating_add(1);
                    continue;
                }
                if writer_count >= MAX_WRITERS_PER_BLOCK {
                    continue;
                }
            }
        }
        let chain: &[Did] = e.visual.as_deref().map_or(&[], |v| v.chain.as_slice());
        let scales = src.model_scales(object, chain);
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
        let (id, kind, points, placed) = match figure {
            Some((fi, _)) => (
                EntityId(figure_tag(key, *fi)),
                OwnerKind::Figure {
                    tag: figure_tag(key, *fi),
                },
                None,
                None,
            ),
            None => {
                let points = if scripts.attaches {
                    e.visual
                        .as_ref()
                        .and_then(|v| v.setup)
                        .and_then(|s| src.setup_points(s))
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
                    Some(
                        PlacedObject::of_entity(key, e, parts, rank).with_property_opacity(
                            bundle.entity_render_opacities.get(ei).copied().flatten(),
                        ),
                    ),
                )
            }
        };
        match scripts.scope {
            RunScope::Reach => effect_owner_count += 1,
            RunScope::Placed => writer_count += 1,
        }
        owners.push(EffectOwner {
            id,
            kind,
            iid: e.iid.map(|i| i.0),
            scripts,
            points,
            scales,
            position,
            placed,
        });
    }
    BlockScan {
        key,
        bundle: identity,
        owners,
        writers_without_object,
        effects_scanned: depth == ScanDepth::Full,
        effect_owner_count,
        writer_count,
    }
}

/// First runtime ID of the creation viewport's owners ([`EffectData::scan_stage`]): above
/// the session and beam owners, below every landblock DID in the upper half.
pub const STAGE_OWNER_BASE: u64 = 3 << 32;
/// Most creation-scene elements scanned for effect owners.
pub const MAX_STAGE_ELEMENTS: usize = 4096;

/// One Physics element of a creation scene offered to the effect layer
/// (`character-screens.md` §2.1.1 rule 1: its start-up scripts run, so effect entities
/// start their particle systems).
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct StageElement {
    /// The object it places (0x47).
    pub object: Did,
    /// Where it stands (the scene's frame, metres).
    pub position: [f32; 3],
    /// Its rotation.
    pub rotation: Quat,
    /// `Render_AdjustableScale` per axis.
    pub scale: [f32; 3],
}

impl EffectData {
    /// The effect owners among a creation scene's Physics elements (`character-screens.md`
    /// §2.1.1 rule 1), as static owners in the scene's frame: the elements whose object's
    /// start-up scripts reach Particle nodes, each with its setup's attachment points and
    /// the template scales of its chain (`setup.md` §5.2). At most
    /// [`MAX_STAGE_ELEMENTS`] elements; IDs from [`STAGE_OWNER_BASE`] by element index.
    #[must_use]
    pub fn scan_stage(&self, elements: &[StageElement]) -> Vec<EffectOwner> {
        let mut owners = Vec::new();
        for (i, e) in elements.iter().enumerate().take(MAX_STAGE_ELEMENTS) {
            if owners.len() >= MAX_OWNERS_PER_BLOCK {
                break;
            }
            if e.object.prefix() != ENTITY_PREFIX {
                continue;
            }
            let Some(scripts) = self.object_scripts(e.object) else {
                continue;
            };
            let chain =
                tessera_formats::setup::resolve_setup_did(e.object, |d| fetch(&self.set, d.0))
                    .unwrap_or_default();
            let scales = self.model_scales(e.object, &chain);
            let points = if scripts.attaches {
                chain
                    .last()
                    .copied()
                    .filter(|d| d.prefix() != ENTITY_PREFIX && d.prefix() != TEMPLATE_PREFIX)
                    .and_then(|s| self.setup_points(s))
            } else {
                None
            };
            let mut scale = e.scale.map(|c| if c.is_finite() { c } else { 1.0 });
            for (a, b) in scale.iter_mut().zip(scales.model) {
                *a *= b;
            }
            let rotation = if e.rotation.is_finite() && e.rotation.length_squared() > 1e-12 {
                e.rotation.normalize()
            } else {
                Quat::IDENTITY
            };
            let position = e
                .position
                .map(|c| if c.is_finite() { f64::from(c) } else { 0.0 });
            let id = EntityId(STAGE_OWNER_BASE + u64::try_from(i).unwrap_or(u64::MAX >> 33));
            owners.push(EffectOwner {
                id,
                kind: OwnerKind::Static {
                    position,
                    rotation,
                    scale,
                },
                iid: None,
                scripts,
                points,
                scales,
                position,
                placed: None,
            });
        }
        owners
    }
}

/// Most DID-valued parameters remembered per scriptlet for the effect scan's file pairs.
pub const MAX_SCAN_PARAMETERS: usize = 64;

/// What the effect scan needs to follow a Scriptlet node's `ScriptletScript_File` pair
/// (`appearance.md` §7.4.1 rules 1–3 and the edge-case Decision on file pairs).
pub(crate) struct ScanPairs<'a> {
    /// `ScriptletScript_File` (`None`: no pair is followed).
    pub file: Option<u32>,
    /// The owner's stored property values (driven sources).
    pub owner: &'a PropertyState,
    /// The dictionary default of a property, when it is a DID.
    pub default: &'a dyn Fn(u32) -> Option<Did>,
    /// The PID of `Render_Opacity`, whose SetProperty nodes the scan notes
    /// ([`ScanOut::writes_opacity`]); `None`: not noted.
    pub opacity: Option<u32>,
}

impl ScanPairs<'_> {
    /// The DID a driven pair reads from `source`: the owner's value, else the default.
    fn driven(&self, source: u32) -> Option<Did> {
        match self.owner.get(source) {
            Some(v) => v.as_did(),
            None => (self.default)(source),
        }
    }
}

/// The DID values that a scriptlet-driven pair of `parameter` may read from the record's
/// invocations (`context`), in order, each once.
fn context_values(context: &[(u32, Did)], parameter: u32) -> Vec<Did> {
    let mut out: Vec<Did> = Vec::new();
    for &(p, d) in context {
        if p == parameter && !out.contains(&d) {
            out.push(d);
        }
    }
    out
}

/// What a Scriptlet node with stored `file` and `parameters` may invoke, and the
/// DID-valued parameters its set holds after its pairs (`appearance.md` §7.4.1 rules 1–3):
/// driven pairs first (the owner's value or the dictionary default), then scriptlet-driven
/// pairs, which read the parameters that the invocations of the node's own record hold
/// (`context`; empty for a table script, where no Scriptlet node invokes the record). A
/// pair whose field PID the set holds writes that parameter; a `ScriptletScript_File` pair
/// the set does not hold chooses the record when its value is a DID. The scan walks every
/// branch, so a scriptlet-driven pair that several invocations feed keeps every value.
fn scriptlet_choices(
    n: &tessera_formats::Node,
    file: Did,
    parameters: &PropertySet,
    context: &[(u32, Did)],
    pairs: &ScanPairs<'_>,
) -> (Vec<Did>, Vec<(u32, Did)>) {
    let held = |pid: u32| parameters.get(pid).is_some();
    // The parameters as candidates per PID: stored, then replaced by the pairs that find a
    // value.
    let mut params: Vec<(u32, Vec<Did>)> = parameters
        .entries
        .iter()
        .take(MAX_SCAN_PARAMETERS)
        .map(|e| (e.pid, e.value.as_did().into_iter().collect()))
        .collect();
    let mut files: Vec<Did> = vec![file];
    let mut write = |field: u32, values: Vec<Did>| {
        if values.is_empty() {
            return;
        }
        if held(field) {
            if let Some(slot) = params.iter_mut().find(|(p, _)| *p == field) {
                slot.1 = values;
            }
        } else if pairs.file == Some(field) {
            files = values;
        }
    };
    for &(field, source) in &n.base.driven {
        write(field, pairs.driven(source).into_iter().collect());
    }
    for &(field, parameter) in &n.base.scriptlet_driven {
        write(field, context_values(context, parameter));
    }
    files.retain(|d| !d.is_null());
    let mut filled = Vec::new();
    for (pid, values) in params {
        for d in values {
            if filled.len() < MAX_SCAN_PARAMETERS && !d.is_null() {
                filled.push((pid, d));
            }
        }
    }
    (files, filled)
}

/// Collects what one tree of a record holds: Particle effects into `out`, the scripts its
/// Run nodes start into `keys`, and the scriptlets its Scriptlet nodes may invoke, with the
/// DID parameters they pass, into `invokes`.
fn scan_node(
    root: &tessera_formats::Node,
    context: &[(u32, Did)],
    pairs: &ScanPairs<'_>,
    out: &mut ScanOut,
    keys: &mut Vec<u32>,
    invokes: &mut Vec<(Did, Vec<(u32, Did)>)>,
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
        // A ServerOnly SetProperty sets nothing locally (`script-table.md` §7.7), so it is
        // no write of the opacity rule 15 follows.
        NodeData::SetProperty {
            property,
            server_only: false,
            ..
        } if Some(*property) == pairs.opacity => {
            out.writes_opacity = true;
        }
        NodeData::Scriptlet { file, parameters } => {
            let (files, filled) = scriptlet_choices(n, *file, parameters, context, pairs);
            for f in files {
                invokes.push((f, filled.clone()));
            }
        }
        _ => {}
    });
}

/// Walks the scripts `keys` of `closure`, every script their Run nodes start and every
/// scriptlet they invoke, also those a `ScriptletScript_File` pair chooses
/// ([`scriptlet_choices`]), collecting the effect DIDs of their Particle nodes. Bounded:
/// at most [`MAX_SCAN_KEYS`] script keys and [`MAX_SCAN_SCRIPTLETS`] scriptlet walks; a
/// scriptlet is walked again only when its invocations bring parameters it had not seen
/// (at most [`MAX_SCAN_PARAMETERS`] per scriptlet).
pub(crate) fn scan_scripts(
    closure: &[Arc<ScriptTable>],
    keys: &[u32],
    pairs: &ScanPairs<'_>,
    mut load: impl FnMut(Did) -> Option<Arc<Scriptlet>>,
) -> ScanOut {
    let mut out = ScanOut::default();
    let mut todo: Vec<u32> = keys.to_vec();
    let mut seen_keys = HashSet::new();
    let mut invokes: Vec<(Did, Vec<(u32, Did)>)> = Vec::new();
    // Per scriptlet: the DID parameters its invocations hold, and how many of them it had
    // when last walked.
    let mut contexts: HashMap<Did, Vec<(u32, Did)>> = HashMap::new();
    let mut walked: HashMap<Did, usize> = HashMap::new();
    let mut walks = 0usize;
    loop {
        if let Some(k) = todo.pop() {
            if seen_keys.len() >= MAX_SCAN_KEYS || !seen_keys.insert(k) {
                continue;
            }
            for t in closure {
                if let Some(s) = t.script(k) {
                    for r in &s.roots {
                        scan_node(r, &[], pairs, &mut out, &mut todo, &mut invokes);
                    }
                }
            }
        } else if let Some((d, passed)) = invokes.pop() {
            let context = contexts.entry(d).or_default();
            for e in passed {
                if context.len() >= MAX_SCAN_PARAMETERS {
                    break;
                }
                if !context.contains(&e) {
                    context.push(e);
                }
            }
            if walked.get(&d) == Some(&context.len()) {
                continue;
            }
            if walks >= MAX_SCAN_SCRIPTLETS {
                continue;
            }
            walks += 1;
            walked.insert(d, context.len());
            let context = context.clone();
            if let Some(s) = load(d) {
                for r in &s.roots {
                    scan_node(r, &context, pairs, &mut out, &mut todo, &mut invokes);
                }
            }
        } else {
            break;
        }
    }
    out
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

/// Synthetic landblocks and a synthetic [`OwnerSource`] for tests of the scan and of the
/// layer's owner lifecycle (`super::plan_sync`, `super::plan_owner_check`).
#[cfg(test)]
pub(crate) mod testkit {
    use super::*;
    use tessera_formats::landblock::{HeightGrid, LandblockData, Placement};
    use tessera_formats::setup::Quaternion;
    use tessera_world::{DrawFlags, FigureKind, PlacedEntity, PlacedFigure, ResolvedObject};

    /// Owner scripts by entity object; records which effect owners a lookup measured.
    #[derive(Default)]
    pub(crate) struct FakeSource {
        pub objects: HashMap<Did, Arc<OwnerScripts>>,
        pub measured: Mutex<Vec<Did>>,
    }

    impl FakeSource {
        /// A source where `writer` reaches a SetProperty of `Render_Opacity` only and
        /// `effect` a Particle node.
        pub(crate) fn new(writer: Did, effect: Did) -> Self {
            let mut objects = HashMap::new();
            objects.insert(writer, scripts(RunScope::Placed));
            objects.insert(effect, scripts(RunScope::Reach));
            Self {
                objects,
                measured: Mutex::new(Vec::new()),
            }
        }
    }

    impl OwnerSource for FakeSource {
        fn interiors(&self) -> bool {
            false
        }

        fn layer_active(&self, _layer: u32) -> bool {
            false
        }

        fn lookup(
            &self,
            object: Did,
            _instance: Option<&PropertySet>,
            depth: ScanDepth,
        ) -> OwnerLookup {
            match self.objects.get(&object) {
                None => OwnerLookup::None,
                Some(s) if s.scope == RunScope::Reach && depth == ScanDepth::Writers => {
                    OwnerLookup::Unmeasured
                }
                Some(s) => {
                    if s.scope == RunScope::Reach {
                        lock(&self.measured).push(object);
                    }
                    OwnerLookup::Scripts(Arc::clone(s))
                }
            }
        }

        fn model_scales(&self, _object: Did, _chain: &[Did]) -> ModelScales {
            ModelScales::default()
        }

        fn setup_points(&self, _setup: Did) -> Option<Arc<AttachPoints>> {
            None
        }
    }

    /// Owner scripts of `scope` (reach 40 m for an effect owner).
    pub(crate) fn scripts(scope: RunScope) -> Arc<OwnerScripts> {
        Arc::new(OwnerScripts {
            tables: vec![Did(0x2000_0001)],
            startup_scripts: Vec::new(),
            properties: PropertyState::new(),
            reach: if scope == RunScope::Reach { 40.0 } else { 0.0 },
            attaches: false,
            scope,
        })
    }

    /// A drawn static entity `index` of `object` at `position` with part meshes `parts`.
    pub(crate) fn entity(
        index: u32,
        object: Did,
        position: [f32; 3],
        parts: &[u32],
    ) -> PlacedEntity {
        PlacedEntity {
            index,
            iid: None,
            kind: None,
            object: Some(object),
            placement: Placement::new(position, Quaternion::IDENTITY).expect("placement"),
            scale: [1.0; 3],
            interior: false,
            content_layers: None,
            own_draw: DrawFlags::default(),
            visual: Some(Arc::new(ResolvedObject {
                object,
                physics: Some(object),
                chain: vec![object],
                setup: None,
                parts: parts.iter().map(|p| Did(*p)).collect::<Vec<_>>().into(),
                draw: DrawFlags::default(),
                weenie_type: None,
                bones: None,
                error: None,
                transient: false,
            })),
            draw: true,
            figure: None,
        }
    }

    /// A landblock `key` with `entities`, of which those at the positions `figures` (in
    /// the entity list) are figures.
    pub(crate) fn bundle(
        key: LandblockKey,
        entities: Vec<PlacedEntity>,
        figures: &[u32],
    ) -> LandblockBundle {
        let mut b = LandblockBundle::from_data(
            key,
            LandblockData::from_heights(key.data_did(), HeightGrid::flat(1_000)),
        );
        b.entities = entities;
        b.figures = figures
            .iter()
            .map(|i| PlacedFigure {
                entity: EntityRef::Entity(*i),
                kind: FigureKind::Npc,
                instance: None,
                idle: None,
                random_appearance: None,
                weenie_type: None,
            })
            .collect();
        b
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    // --- The effect scan follows `ScriptletScript_File` pairs (`appearance.md` §7.4.1) ---

    use tessera_formats::script_table::{BaseBlock, NodeKind, ParticleScriptNode, Script};
    use tessera_formats::{PropertyEntry, TSize};

    /// `ScriptletScript_File`.
    const FILE: u32 = 0x1000_0001;
    /// An entity property holding a scriptlet DID.
    const SOURCE: u32 = 0x1000_0002;
    /// A DID scriptlet parameter.
    const PARAM: u32 = 0x1000_0003;
    const STARTUP: u32 = 5;
    const EFFECT: Did = Did(0x3900_0001);
    const NESTED_EFFECT: Did = Did(0x3900_0002);

    fn script_node(kind: NodeKind, data: NodeData, base: BaseBlock) -> tessera_formats::Node {
        tessera_formats::Node {
            kind,
            data,
            base,
            particle_flag: None,
            ports: Vec::new(),
            targets: Vec::new(),
        }
    }

    fn particle(effect: Did) -> tessera_formats::Node {
        script_node(
            NodeKind::PARTICLE,
            NodeData::Particle(Box::new(ParticleScriptNode {
                particle_id: effect,
                ..ParticleScriptNode::default()
            })),
            BaseBlock::default(),
        )
    }

    fn invoke(
        file: u32,
        parameters: &[(u32, u32)],
        driven: &[(u32, u32)],
        from_params: &[(u32, u32)],
    ) -> tessera_formats::Node {
        let entries: Vec<PropertyEntry> = parameters
            .iter()
            .map(|&(pid, d)| PropertyEntry {
                pid,
                value: PropertyValue::DidRef(Did(d)),
            })
            .collect();
        script_node(
            NodeKind::SCRIPTLET,
            NodeData::Scriptlet {
                file: Did(file),
                parameters: PropertySet {
                    size: TSize {
                        hint: 0,
                        count: u32::try_from(entries.len()).unwrap_or(0),
                    },
                    entries,
                },
            },
            BaseBlock {
                driven: driven.to_vec(),
                scriptlet_driven: from_params.to_vec(),
                probability: 1.0,
                ..BaseBlock::default()
            },
        )
    }

    fn startup(roots: Vec<tessera_formats::Node>) -> Vec<Arc<ScriptTable>> {
        vec![Arc::new(ScriptTable {
            did: Did(0x0700_0001),
            kind: 2,
            includes: Vec::new(),
            scripts_size: TSize::default(),
            scripts: vec![Script {
                key: STARTUP,
                roots,
            }],
            properties_size: TSize::default(),
            script_properties: Vec::new(),
            dids_size: TSize::default(),
            script_dids: Vec::new(),
        })]
    }

    /// The effects a scan finds, given the owner's values, with these scriptlets.
    fn scan_effects(
        closure: &[Arc<ScriptTable>],
        owner: &PropertyState,
        scriptlets: &[(u32, Vec<tessera_formats::Node>)],
    ) -> Vec<Did> {
        let pairs = ScanPairs {
            file: Some(FILE),
            owner,
            default: &|_| None,
            opacity: None,
        };
        let out = scan_scripts(closure, &[STARTUP], &pairs, |d| {
            scriptlets.iter().find(|s| s.0 == d.0).map(|s| {
                Arc::new(Scriptlet {
                    did: d,
                    roots: s.1.clone(),
                    dependencies: Vec::new(),
                })
            })
        });
        let mut found: Vec<Did> = out.effects.iter().map(|e| e.0).collect();
        found.sort_unstable();
        found.dedup();
        found
    }

    /// Rule 15 (answer item 2 under the anchor `s12-r15-detect`): the scan notes a SetProperty
    /// of `Render_Opacity` (through scriptlets too), so entities that write it without being
    /// effect owners are run while placed; another property, or no PID, notes nothing. A
    /// Particle node makes an effect owner, whether or not the scripts write the opacity.
    #[test]
    fn effect_scan_notes_opacity_writes() {
        const OPACITY: u32 = 0x1000_0020;
        let set = |property| {
            script_node(
                NodeKind::SET_PROPERTY,
                NodeData::SetProperty {
                    property,
                    value: Some(Box::new(PropertyEntry {
                        pid: property,
                        value: PropertyValue::Float(0.0),
                    })),
                    server_only: false,
                },
                BaseBlock::default(),
            )
        };
        let owner = PropertyState::new();
        let scan = |closure: &[Arc<ScriptTable>], opacity, scriptlets: &[(u32, Vec<_>)]| {
            let pairs = ScanPairs {
                file: Some(FILE),
                owner: &owner,
                default: &|_| None,
                opacity,
            };
            scan_scripts(closure, &[STARTUP], &pairs, |d| {
                scriptlets.iter().find(|s| s.0 == d.0).map(|s| {
                    Arc::new(Scriptlet {
                        did: d,
                        roots: s.1.clone(),
                        dependencies: Vec::new(),
                    })
                })
            })
        };
        let direct = startup(vec![set(OPACITY)]);
        let out = scan(&direct, Some(OPACITY), &[]);
        assert!(out.writes_opacity);
        assert!(out.effects.is_empty());
        assert_eq!(out.scope(), Some(RunScope::Placed));
        assert!(!scan(&direct, None, &[]).writes_opacity);
        assert_eq!(scan(&direct, None, &[]).scope(), None);
        let other = scan(&startup(vec![set(OPACITY + 1)]), Some(OPACITY), &[]);
        assert!(!other.writes_opacity);
        assert_eq!(other.scope(), None);
        let nested = startup(vec![invoke(0x0C00_0001, &[], &[], &[])]);
        let scriptlets = [(0x0C00_0001, vec![set(OPACITY)])];
        assert_eq!(
            scan(&nested, Some(OPACITY), &scriptlets).scope(),
            Some(RunScope::Placed)
        );
        let both = scan(
            &startup(vec![set(OPACITY), particle(EFFECT)]),
            Some(OPACITY),
            &[],
        );
        assert!(both.writes_opacity);
        assert_eq!(both.scope(), Some(RunScope::Reach), "an effect owner first");
        // A ServerOnly SetProperty sets nothing locally (`script-table.md` §7.7): no write.
        let server_only = script_node(
            NodeKind::SET_PROPERTY,
            NodeData::SetProperty {
                property: OPACITY,
                value: Some(Box::new(PropertyEntry {
                    pid: OPACITY,
                    value: PropertyValue::Float(0.0),
                })),
                server_only: true,
            },
            BaseBlock::default(),
        );
        let out = scan(&startup(vec![server_only]), Some(OPACITY), &[]);
        assert!(!out.writes_opacity);
        assert_eq!(out.scope(), None);
    }

    /// Rule 15 answer item 2 (`s12-r15-detect`) in the landblock scan: a drawn static writer
    /// is a [`RunScope::Placed`] owner with the placed object of its entity; a figure with
    /// the same scripts is not run, and one without a drawn part is counted, not run. A
    /// writers' scan ([`ScanDepth::Writers`]) finds the same writers, no effect owner, and
    /// measures no effect owner's reach.
    #[test]
    fn a_block_scan_finds_placed_writers() {
        use testkit::{FakeSource, bundle, entity};
        const WRITER: Did = Did(0x4700_0010);
        const EFFECT: Did = Did(0x4700_0020);
        let key = LandblockKey::new(1, 10, 20).expect("key");
        let src = FakeSource::new(WRITER, EFFECT);
        let b = bundle(
            key,
            vec![
                entity(3, WRITER, [1.0, 2.0, 3.0], &[5]),
                entity(4, WRITER, [9.0, 9.0, 3.0], &[5]),
                entity(5, WRITER, [7.0, 7.0, 3.0], &[]),
                entity(6, EFFECT, [4.0, 4.0, 3.0], &[6]),
                entity(7, Did(0x4700_0030), [5.0, 5.0, 3.0], &[6]),
            ],
            &[1],
        );
        let full = scan_bundle(&src, &b, 7, ScanDepth::Full);
        assert!(full.effects_scanned);
        assert_eq!(full.owners.len(), 2, "{:?}", full.owners);
        let w = &full.owners[0];
        assert_eq!(w.scripts.scope, RunScope::Placed);
        assert_eq!(w.id, static_owner_id(key, 3));
        let placed = w.placed.as_ref().expect("a placed object");
        assert_eq!((placed.key, placed.index), (key, Some(3)));
        assert_eq!(placed.entity.position, [1.0, 2.0, 3.0]);
        assert_eq!(&*placed.entity.parts, &[5]);
        assert_eq!(full.owners[1].scripts.scope, RunScope::Reach);
        assert_eq!(full.owners[1].id, static_owner_id(key, 6));
        assert_eq!(
            (full.writer_count, full.effect_owner_count),
            (1, 1),
            "the figure is not run"
        );
        assert_eq!(full.writers_without_object, 1);
        assert_eq!(full.opacity_writers().count(), full.writer_count);
        assert_eq!(full.effect_owners().count(), full.effect_owner_count);
        assert_eq!(*lock(&src.measured), vec![EFFECT]);

        let src = FakeSource::new(WRITER, EFFECT);
        let writers = scan_bundle(&src, &b, 7, ScanDepth::Writers);
        assert!(!writers.effects_scanned);
        assert_eq!(writers.owners, full.owners[..1]);
        assert_eq!((writers.writer_count, writers.effect_owner_count), (1, 0));
        assert_eq!(writers.writers_without_object, 1);
        assert!(lock(&src.measured).is_empty(), "no reach measured");
    }

    /// Review finding (scan bounds): writers and effect owners have bounds of their own per
    /// landblock, so neither takes the other's place, whichever comes first.
    #[test]
    fn writers_and_effect_owners_are_bounded_apart() {
        use testkit::{FakeSource, bundle, entity};
        const WRITER: Did = Did(0x4700_0010);
        const EFFECT: Did = Did(0x4700_0020);
        let key = LandblockKey::new(1, 10, 20).expect("key");
        let src = FakeSource::new(WRITER, EFFECT);
        let many = |first: Did, then: Did| {
            let mut v: Vec<_> = (0..1030u32)
                .map(|i| entity(i, first, [i as f32 * 0.1, 1.0, 3.0], &[5]))
                .collect();
            v.push(entity(2000, then, [1.0, 1.0, 3.0], &[6]));
            bundle(key, v, &[])
        };
        let s = scan_bundle(&src, &many(WRITER, EFFECT), 1, ScanDepth::Full);
        assert_eq!(s.writer_count, MAX_WRITERS_PER_BLOCK);
        assert_eq!(
            s.effect_owner_count, 1,
            "the effect owner after the writers is found"
        );
        let s = scan_bundle(&src, &many(EFFECT, WRITER), 1, ScanDepth::Full);
        assert_eq!(s.effect_owner_count, MAX_OWNERS_PER_BLOCK);
        assert_eq!(s.writer_count, 1);
        assert_eq!(s.owners.len(), MAX_OWNERS_PER_BLOCK + 1);
    }

    /// `appearance.md` §7.4.1, edge-case Decision: a driven `ScriptletScript_File` pair on
    /// a Scriptlet node whose set lacks that PID chooses the record (here a placeholder
    /// with file 0); the scan follows it into the chosen scriptlet. Without the owner's
    /// value the placeholder invokes nothing.
    #[test]
    fn effect_scan_follows_a_driven_file_pair() {
        let scriptlets = [(0x0C00_0001, vec![particle(EFFECT)])];
        let closure = startup(vec![invoke(0, &[], &[(FILE, SOURCE)], &[])]);
        let mut owner = PropertyState::new();
        owner.set(SOURCE, PropertyValue::DidRef(Did(0x0C00_0001)));
        assert_eq!(scan_effects(&closure, &owner, &scriptlets), [EFFECT]);
        assert!(scan_effects(&closure, &PropertyState::new(), &scriptlets).is_empty());
        // A set that holds the file PID takes the value as a parameter (rule 2): the stored
        // file (0) stays, so nothing is invoked.
        let closure = startup(vec![invoke(
            0,
            &[(FILE, 0x0C00_0009)],
            &[(FILE, SOURCE)],
            &[],
        )]);
        assert!(scan_effects(&closure, &owner, &scriptlets).is_empty());
    }

    /// `appearance.md` §7.4.1 rules 2–3: a scriptlet-driven file pair reads the parameter
    /// of the invoking Scriptlet node's own set, as that node's pairs left it (here filled
    /// from the owner by a driven pair); it overrides a stored file.
    #[test]
    fn effect_scan_follows_a_scriptlet_driven_file_pair() {
        let scriptlets = [
            (
                0x0C00_0001,
                vec![invoke(0x0C00_0003, &[], &[], &[(FILE, PARAM)])],
            ),
            (0x0C00_0002, vec![particle(NESTED_EFFECT)]),
            (0x0C00_0003, vec![particle(EFFECT)]),
        ];
        // The outer invocation holds the parameter (stored 0x0C000002).
        let closure = startup(vec![invoke(0x0C00_0001, &[(PARAM, 0x0C00_0002)], &[], &[])]);
        let owner = PropertyState::new();
        assert_eq!(scan_effects(&closure, &owner, &scriptlets), [NESTED_EFFECT]);
        // Its driven pair replaces the parameter before it is passed down.
        let closure = startup(vec![invoke(
            0x0C00_0001,
            &[(PARAM, 0x0C00_0002)],
            &[(PARAM, SOURCE)],
            &[],
        )]);
        let mut owner = PropertyState::new();
        owner.set(SOURCE, PropertyValue::DidRef(Did(0x0C00_0003)));
        assert_eq!(scan_effects(&closure, &owner, &scriptlets), [EFFECT]);
        // An invocation without the parameter: the inner node keeps its stored file.
        let closure = startup(vec![invoke(0x0C00_0001, &[], &[], &[])]);
        assert_eq!(scan_effects(&closure, &owner, &scriptlets), [EFFECT]);
    }

    /// A scriptlet reached again with a parameter it had not seen is walked again, so every
    /// file its pairs may choose is scanned.
    #[test]
    fn effect_scan_walks_a_scriptlet_again_for_new_parameters() {
        let scriptlets = [
            (0x0C00_0001, vec![invoke(0, &[], &[], &[(FILE, PARAM)])]),
            (0x0C00_0002, vec![particle(EFFECT)]),
            (0x0C00_0003, vec![particle(NESTED_EFFECT)]),
        ];
        let closure = startup(vec![
            invoke(0x0C00_0001, &[], &[], &[]),
            invoke(0x0C00_0001, &[(PARAM, 0x0C00_0002)], &[], &[]),
            invoke(0x0C00_0001, &[(PARAM, 0x0C00_0003)], &[], &[]),
        ]);
        assert_eq!(
            scan_effects(&closure, &PropertyState::new(), &scriptlets),
            [EFFECT, NESTED_EFFECT]
        );
    }

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
            alpha_test: None,
            matrices: [TextureMatrix::IDENTITY; 2],
            reference_animated: false,
            matrices_animated: false,
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

    /// A resolved unlit particle material with the given blend and no mesh alpha test
    /// (the particle test comes from the surface, `particle-runtime.md` §9.1.3).
    fn particle_material(mode: BlendMode, dst: MatFactor, depth_write: bool) -> ResolvedMaterial {
        use tessera_material::{BlendState, FeatureFlags, ParameterSet, ShaderInterface};
        let shader = Arc::new(ShaderInterface {
            did: Did(0x2B00_0001),
            declarations: Vec::new(),
            slots: Vec::new(),
            permutation_count: 1,
            lit: false,
            common_blend: None,
            cull_none: false,
            default_alpha_ref: None,
            family: Default::default(),
            unbound_defaults: Vec::new(),
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
                alpha_test: None,
                opacity_slot: Some(3000),
                tested_opacity: tessera_material::TestedOpacity::Slot(3000),
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

    /// A beam ribbon's material (`particle-runtime.md` §11.1 step 6) on a shader record the
    /// §9.1.1 table does not list keeps its `ColorAndOpacity2` layer (`material.md` §10.3);
    /// the same record as a particle surface draws one layer (rule 10).
    #[test]
    fn ribbon_materials_keep_an_opacity_second_layer() {
        use tessera_material::resolve::{ParamOrigin, TextureSlot};
        use tessera_material::vocab::{ChannelPacking, SlotKind};
        let mut m = particle_material(BlendMode::AlphaBlend, MatFactor::InvSrcAlpha, false);
        m.slots = [
            (3000, 0x4100_0001, "ColorAndOpacity"),
            (3001, 0x4100_0002, "ColorAndOpacity2"),
        ]
        .iter()
        .map(|&(id, tex, role)| TextureSlot {
            id,
            kind: SlotKind::of(id),
            texture: Some(Did(tex)),
            role: Some(role.into()),
            packing: ChannelPacking::from_role(Some(role)),
            sampler: None,
            origin: ParamOrigin::Default,
        })
        .collect();
        let m = Arc::new(m);
        let ribbon = SurfaceLook::of(&m, Arc::default(), true);
        assert_eq!(ribbon.shading.layers, Some(ParticleLayers::Product));
        assert!(ribbon.second_texture.is_some() && ribbon.texture.is_some());
        let particle = SurfaceLook::of(&m, Arc::default(), false);
        assert_eq!(particle.shading.layers, None);
        assert!(particle.second_texture.is_none());
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
            let alpha = surface_alpha(&particle_material(mode, dst, false), Some(0.5));
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

            let untested = surface_alpha(&particle_material(mode, dst, false), None);
            assert!(matches!(untested, AlphaMode::Blend | AlphaMode::Additive));
            assert_eq!(reference(untested, 1.0), Some(0.0), "{mode:?}");
        }
        // Depth write on: the reference is scaled by *A*.
        let written = surface_alpha(
            &particle_material(BlendMode::Additive, MatFactor::One, true),
            Some(0.5),
        );
        assert_eq!(
            reference(written, 0.5),
            Some((0.25_f32 * 255.0).floor() / 255.0)
        );
    }

    /// A census of the install's entity descriptors (read-only; skipped unless
    /// `TESSERA_GAME_DIR` is set; run with `cargo test --release -p tessera-client --lib
    /// census_of_static_opacity_writers -- --ignored --nocapture`): how many descriptors are
    /// effect owners and how many would be run while placed for their `Render_Opacity`
    /// writes alone (`particle-runtime.md` §12 rule 15, answer item 2 under the anchor
    /// `s12-r15-detect`, whose census is open). Only counts are printed.
    #[test]
    #[ignore = "reads every entity descriptor of the install"]
    fn census_of_static_opacity_writers() {
        let Some(dir) = std::env::var_os("TESSERA_GAME_DIR").filter(|d| !d.is_empty()) else {
            eprintln!("TESSERA_GAME_DIR not set; skipping");
            return;
        };
        let set = Arc::new(
            ArchiveSet::open(&dir, tessera_dat::SetOptions::default()).expect("archive set"),
        );
        let materials = Arc::new(MaterialLibrary::new(Arc::clone(&set)));
        let data =
            EffectData::open(Arc::clone(&set), materials, Vec::new(), true).expect("effect data");
        let lo = u32::from(ENTITY_PREFIX) << 24;
        let mut dids: Vec<u32> = Vec::new();
        for m in set.members() {
            for e in m.archive.entries_range(lo..=(lo | 0x00FF_FFFF)) {
                let Ok(e) = e else { break };
                dids.push(e.did);
            }
        }
        dids.sort_unstable();
        dids.dedup();
        let (mut reach, mut placed) = (0usize, 0usize);
        for d in &dids {
            match data
                .object_scan(Did(*d), ScanDepth::Full)
                .scripts()
                .map(|s| s.scope)
            {
                Some(RunScope::Reach) => reach += 1,
                Some(RunScope::Placed) => placed += 1,
                None => {}
            }
        }
        eprintln!(
            "entity descriptors: {}; effect owners {reach}; opacity writers without Particle \
             nodes {placed}",
            dids.len()
        );
        assert!(
            data.opacity_pid.is_some(),
            "no Render_Opacity in the dictionary"
        );
    }

    /// Review finding (`particle-runtime.md` §12 rule 15): a static owner's placed object
    /// carries the entity's own index, the value the object layer's block list gives the
    /// object of a placed entity (`ObjectInfo::entity`), so the lookup by index finds it.
    #[test]
    fn a_static_owner_names_its_object_by_the_entity_index() {
        let key = LandblockKey::new(1, 10, 20).expect("key");
        let e = tessera_world::PlacedEntity {
            index: 42,
            iid: None,
            kind: None,
            object: Some(Did(0x4700_0001)),
            placement: tessera_formats::landblock::Placement::new(
                [12.5, 30.0, 4.0],
                tessera_formats::setup::Quaternion::IDENTITY,
            )
            .expect("placement"),
            scale: [1.0; 3],
            interior: false,
            content_layers: None,
            own_draw: tessera_world::DrawFlags::default(),
            visual: None,
            draw: true,
            figure: None,
        };
        let placed = PlacedObject::of_entity(key, &e, vec![7, 8].into(), 2);
        assert_eq!(placed.key, key);
        assert_eq!(placed.index, Some(42), "the entity's own index");
        assert_eq!(placed.entity.position, [12.5, 30.0, 4.0]);
        assert_eq!(&*placed.entity.parts, &[7, 8]);
        assert_eq!(placed.entity.rank, 2);
    }
}
