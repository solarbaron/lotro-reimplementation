// SPDX-License-Identifier: GPL-3.0-or-later
//! Placed decals in the walker and the creation viewport (`docs/specs/decals.md` §2.3,
//! §4–§7, §9.1, §9.2; `character-screens.md` §2.1.1 rule 3).
//!
//! | Part | Where | Spec |
//! |---|---|---|
//! | Decal SEDs of a resident landblock realised as decal objects (content-layer test, parameters of §3.2, box of §4.2, LOD distance) | [`DecalLayer::update`], on the workers | §2.3, §3, §4.2, §7 step 1 |
//! | Receivers: placed objects of the landblock and its eight neighbours (drawn Physics entities and generated objects without skinned parts that accept decals, §5.1), then the full-resolution terrain (outdoor decals), the landblock water plane for `StickToWater` | the build job | §4.3, §5 |
//! | The mesh (back-face test, clipping, coordinates, lift, centre and mean normal) | `tessera_world::decals::build_mesh` | §4.4–§4.6 |
//! | Per frame: draw distance and its 1.5 s fade, opacity, the light colour and the vertex colour, batches by phase, depth bias and material | [`DecalLayer::draw`] | §6.2–§6.6, §7 |
//! | The race scene's Decal elements on the scene's Physics elements | [`DecalLayer::update_stage`], [`DecalLayer::draw_stage`] | `character-screens.md` §2.1.1 |
//!
//! **Tessera Decisions and choices** (recorded in `decals.md` implementer notes):
//!
//! - Rebuild (§4.8 Decision): a landblock's decals are built once the landblock is
//!   resident, and built again when it or one of its eight neighbours enters or leaves
//!   residency (a new bundle), since those bring the terrain and the objects in its boxes.
//! - Interior-viewer rule (`entity-descriptor.md` §4.8): decals with the interior flag are
//!   drawn while the camera is in an interior cell, the others while it is outdoors.
//!   Interior cell geometry is not collected as a receiver (no CPU cell meshes in this
//!   layer), so interior decals land on placed objects only.
//! - Receivers within [`RECEIVER_SEARCH_MARGIN`] plus their mesh radius of the box centre
//!   are tested triangle by triangle; their level-0 mesh is used (§4.3 OPEN (b) default).
//! - Lighting (§6.3): *A* = *A₀* and *T* = 1 (the OPEN Q5 defaults), the main light as the
//!   directional light, the table's point lights whose selection radius reaches the mesh
//!   centre (key lights first, nearest first, at most eight, `lighting.md` §8.5).
//! - Drawing: unlit, two-sided, with the surface record's colour texture, blend, depth
//!   write and alpha test (§6.1), clamped to the texture's edge where its sampler says so.
//!   The creation viewport has no environment: its decals have no draw distance.

use std::collections::{HashMap, HashSet};
use std::sync::{Arc, Mutex, MutexGuard, mpsc};

use tessera_assets::{CancelToken, Priority, WorkerPool};
use tessera_dat::ArchiveSet;
use tessera_formats::landblock::StaticEntityType;
use tessera_formats::{Did, EntityDescriptor, LandblockData, PropertyDictionary, PropertySet};
use tessera_formats::{PropertyValue, Record as _};
use tessera_material::MaterialLibrary;
use tessera_material::vocab::ids;
use tessera_mesh::{DecodeOptions, GeometryLimits, Mesh};
use tessera_particles::glam::{Quat, Vec3};
use tessera_particles::surface::ParticleSurface;
use tessera_render::{AlphaMode, DecalDraw, DecalVertex as GpuDecalVertex, Pass, TextureId};
use tessera_world::decals::realise::{
    LANDBLOCK_UNITS, MAX_RECEIVER_TRIANGLES, PlacedDecal, ReceiverBudget, TerrainGrid, decal_seds,
    default_material, mesh_receivers, terrain_receivers,
};
use tessera_world::decals::shading::{
    ATTENUATION_SCALE, DecalLight, DecalLightKind, DepthBiasTable, decal_colour, decal_light,
    depth_opacity, draw_range, height_alpha, vertex_colour_applies,
};
use tessera_world::decals::{DecalMesh, DecalPids, MAX_MARKER_CHAIN, build_mesh};
use tessera_world::draw_distance::{DrawFade, aabb_distance};
use tessera_world::{LandblockBundle, LandblockKey, WorldContext, WorldStreamer};

use super::EffectLighting;
use crate::audio::loader::fetch;
use crate::walker::geometry::{TexRequest, VARIANT_OWN_ALPHA, slot_wrap};
use crate::walker::objects::{ObjectLayer, TexLookup};
use crate::walker::terrain::TextureStore;
use crate::walker::view::Anchor;

/// Landblock builds queued at once.
pub const MAX_BUILDS_IN_FLIGHT: usize = 8;
/// Most decals realised per landblock (the data has at most a few hundred).
pub const MAX_DECALS_PER_BLOCK: usize = 8192;
/// Most decals of a creation scene (the data has 2 and 5).
pub const MAX_STAGE_DECALS: usize = 256;
/// Receiving objects are tested when their origin lies within the box's terrain
/// half-width plus this many metres plus their mesh radius of the box centre.
pub const RECEIVER_SEARCH_MARGIN: f32 = 4.0;
/// Most receiving objects (whose meshes reach the box) collected per decal.
pub const MAX_RECEIVER_OBJECTS: usize = 1024;
/// Most part meshes kept decoded for receivers.
pub const MAX_CACHED_MESHES: usize = 4096;
/// Most marker chains and receiver property chains kept.
pub const MAX_CACHED_CHAINS: usize = 8192;
/// Most decal surface looks kept.
pub const MAX_CACHED_LOOKS: usize = 1024;
/// Most point lights a decal sums (`lighting.md` §8.5: at most eight per-vertex lights).
pub const MAX_DECAL_LIGHTS: usize = 8;
/// Most decal vertices queued per pass (the renderer's bound, `tessera_render`).
pub const MAX_DRAWN_VERTICES: usize = tessera_render::MAX_DECAL_VERTICES_PER_PASS;
/// Most decal batches (draw calls) queued per pass (the renderer's bound); a decal that
/// would open a batch beyond it is left out of the frame.
pub const MAX_DRAWN_BATCHES: usize = tessera_render::MAX_DECAL_DRAWS_PER_PASS;
/// Decals whose box reaches farther than this many metres from its centre (the terrain
/// search half-width |*e*|) are not built: the data's boxes are at most 20 m on a side
/// (about 35 m), and a hostile `OrthoScale` must not pull in every receiver of the window.
pub const MAX_DECAL_HALF_WIDTH: f32 = 160.0;
/// Receiving objects whose origin lies farther than the box's half-width plus
/// [`RECEIVER_SEARCH_MARGIN`] plus this many metres from the box centre are not looked at
/// (before their meshes are decoded; the radius test follows for the rest).
pub const MAX_RECEIVER_RADIUS: f32 = 200.0;
/// Most receiving triangles collected for all decals of one landblock build together.
pub const MAX_BLOCK_RECEIVER_TRIANGLES: usize = 1 << 20;
/// Most decal vertices one landblock build keeps (32 MiB); later decals of the block get
/// no mesh.
pub const MAX_BLOCK_DECAL_VERTICES: usize = 1 << 20;
/// Most decal vertices the layer keeps for all resident landblocks together (128 MiB); a
/// build that arrives beyond it keeps no mesh for the decals that do not fit.
pub const MAX_LAYER_DECAL_VERTICES: usize = 1 << 22;
/// Most vertices of the decoded receiver meshes kept in the cache together; beyond it a
/// mesh is used for the build at hand and not kept.
pub const MAX_CACHED_MESH_VERTICES: usize = 1 << 22;

fn lock<T>(m: &Mutex<T>) -> MutexGuard<'_, T> {
    m.lock().unwrap_or_else(std::sync::PoisonError::into_inner)
}

/// Counters of the layer (HUD, headless reports, real-data tests).
#[derive(Debug, Clone, Copy, Default, PartialEq, Eq)]
pub struct DecalStats {
    /// Landblocks with decal objects.
    pub blocks: usize,
    /// Decal objects realised (world).
    pub decals: usize,
    /// Of them, decals whose build produced triangles.
    pub built: usize,
    /// Triangles of every built world decal.
    pub triangles: usize,
    /// World decals in the batches queued in the last frame (a batch waiting for its
    /// texture is not counted).
    pub drawn: usize,
    /// Their triangles.
    pub drawn_triangles: usize,
    /// Decal batches (draw calls) queued in the last world frame (`decals.md` §6.6).
    pub batches: usize,
    /// Landblocks with decals not yet built for their current neighbours (queued,
    /// running, or waiting for a build slot).
    pub pending: usize,
    /// Decal objects of the creation viewport's scene.
    pub stage_decals: usize,
    /// Of them, decals with triangles.
    pub stage_built: usize,
    /// Creation-scene decals in the batches queued in the last viewport pass.
    pub stage_drawn: usize,
    /// Their triangles.
    pub stage_triangles: usize,
    /// Surface records that did not resolve (their decals draw nothing, §6.1).
    pub looks_failed: usize,
    /// Batches left out of the last frame while their texture loads.
    pub textures_pending: usize,
    /// Decals with triangles left out of the last pass by the batch or vertex bound.
    pub left_out: usize,
    /// Distance from the eye to the nearest box of a built world decal in the last frame
    /// (whole metres, rounded down; `u32::MAX` without one), and that decal's draw range
    /// *D* (§7 step 2; whole metres).
    pub nearest: (u32, u32),
}

/// How a decal's surface record draws (`decals.md` §6.1).
#[derive(Debug, Clone, PartialEq)]
pub struct DecalLook {
    /// The colour texture (slot 3000) with its own alpha and sampler address modes.
    pub texture: Option<TexRequest>,
    /// Blend, depth write and alpha test.
    pub alpha: AlphaMode,
    /// Material colour 2000.
    pub material: [f32; 4],
    /// The vertex colour reaches the output (§6.1: not for the two records without a
    /// colour input).
    pub vertex_colour: bool,
    /// Fogged.
    pub fogged: bool,
}

/// A part mesh as a receiver: level 0, object space.
#[derive(Debug)]
struct PartMesh {
    positions: Vec<[f32; 3]>,
    normals: Vec<[f32; 3]>,
    triangles: Vec<[u32; 3]>,
    /// The part has skinned vertices (§4.3 step 1: such a model receives nothing).
    skinned: bool,
    /// Largest distance of a vertex from the part's origin.
    radius: f32,
}

/// Decoded receiver meshes by part DID, and the vertices they hold together.
type MeshCache = (HashMap<Did, Option<Arc<PartMesh>>>, usize);

/// What the build jobs share.
struct DecalData {
    set: Arc<ArchiveSet>,
    dict: PropertyDictionary,
    pids: DecalPids,
    allow: Option<u32>,
    force: Option<u32>,
    materials: Arc<MaterialLibrary<Arc<ArchiveSet>>>,
    default_material: Mutex<Option<Did>>,
    /// 0x47 chains, nearest first.
    chains: Mutex<HashMap<Did, Arc<Vec<PropertySet>>>>,
    /// Decoded receiver meshes and the vertices they hold together.
    meshes: Mutex<MeshCache>,
    looks: Mutex<HashMap<Did, Option<Arc<DecalLook>>>>,
}

impl std::fmt::Debug for DecalData {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("DecalData").finish_non_exhaustive()
    }
}

impl DecalData {
    fn open(
        set: Arc<ArchiveSet>,
        materials: Arc<MaterialLibrary<Arc<ArchiveSet>>>,
    ) -> Result<Self, String> {
        let bytes = fetch(&set, super::data::PROPERTY_DICTIONARY)
            .map_err(|e| format!("property dictionary: {e}"))?
            .ok_or("property dictionary missing")?;
        let dict = PropertyDictionary::parse_expecting(&bytes, super::data::PROPERTY_DICTIONARY)
            .map_err(|e| format!("property dictionary: {e}"))?;
        let pids = DecalPids::resolve(&dict);
        Ok(Self {
            allow: pids.allow_decals.id(),
            force: pids.force_decals.id(),
            pids,
            dict,
            set,
            materials,
            default_material: Mutex::new(None),
            chains: Mutex::new(HashMap::new()),
            meshes: Mutex::new((HashMap::new(), 0)),
            looks: Mutex::new(HashMap::new()),
        })
    }

    /// The 0x47 chain of `object`, nearest first (`decals.md` §3.2 reads it root first).
    fn chain(&self, object: Did) -> Arc<Vec<PropertySet>> {
        if let Some(c) = lock(&self.chains).get(&object) {
            return Arc::clone(c);
        }
        let mut levels = tessera_world::decals::marker_chain(object, |d| {
            let bytes = fetch(&self.set, d.0).ok().flatten()?;
            let ed = EntityDescriptor::parse(&bytes, &self.dict).ok()?;
            Some((ed.properties, Some(ed.header.template)))
        });
        levels.reverse();
        levels.truncate(MAX_MARKER_CHAIN);
        let c = Arc::new(levels);
        let mut m = lock(&self.chains);
        if m.len() < MAX_CACHED_CHAINS {
            m.insert(object, Arc::clone(&c));
        }
        c
    }

    fn default_material(&self, ctx: Option<&WorldContext>) -> Did {
        let mut d = lock(&self.default_material);
        if d.is_none() {
            *d = ctx.and_then(default_material);
        }
        d.unwrap_or(Did(0))
    }

    /// A set's own `Render_AllowDecals` and `Render_ForceDecals` values.
    fn own_flags(&self, own: Option<&PropertySet>) -> (Option<bool>, Option<bool>) {
        let find = |pid: Option<u32>| own.and_then(|s| flag(s, pid?));
        (find(self.allow), find(self.force))
    }

    /// Whether a receiver accepts decals (§5.1 with the Q4 Decision): each property on its
    /// own, the receiver's own value (`own`: allow, force) first, then its chain nearest
    /// first, first value found ([`receiver_accepts`]).
    fn accepts(&self, own: (Option<bool>, Option<bool>), object: Option<Did>) -> bool {
        let chain = object.map(|o| self.chain(o));
        let find = |pid: Option<u32>| -> Option<bool> {
            let pid = pid?;
            chain.as_ref()?.iter().find_map(|s| flag(s, pid))
        };
        receiver_accepts(own, (find(self.allow), find(self.force)))
    }

    fn mesh(&self, part: Did) -> Option<Arc<PartMesh>> {
        if let Some(m) = lock(&self.meshes).0.get(&part) {
            return m.clone();
        }
        let m = fetch(&self.set, part.0).ok().flatten().and_then(|bytes| {
            let mesh = Mesh::decode(&bytes, None, &DecodeOptions::render_only()).ok()?;
            let geo = mesh.level_geometry(0, &GeometryLimits::default()).ok()?;
            let triangles: Vec<[u32; 3]> = (0..geo.indices.len() / 3)
                .filter_map(|t| {
                    let i = |k: usize| geo.indices.get(3 * t + k);
                    Some([i(0)?, i(1)?, i(2)?])
                })
                .collect();
            let radius = geo
                .positions
                .iter()
                .map(|p| (p[0] * p[0] + p[1] * p[1] + p[2] * p[2]).sqrt())
                .filter(|r| r.is_finite())
                .fold(0.0f32, f32::max);
            Some(Arc::new(PartMesh {
                skinned: geo.joints.is_some(),
                positions: geo.positions,
                normals: geo.normals,
                triangles,
                radius,
            }))
        });
        let n = m.as_ref().map_or(0, |m| m.positions.len());
        let mut c = lock(&self.meshes);
        let (cache, held) = &mut *c;
        if cache.len() < MAX_CACHED_MESHES && *held + n <= MAX_CACHED_MESH_VERTICES {
            *held += n;
            cache.insert(part, m.clone());
        }
        m
    }

    /// The look of surface record `surface` (§6.1); `None` when it does not resolve.
    fn look(&self, surface: Did) -> Option<Arc<DecalLook>> {
        if let Some(l) = lock(&self.looks).get(&surface) {
            return l.clone();
        }
        let l = self
            .materials
            .resolve_surface(surface)
            .map_err(|e| tracing::debug!(%surface, "decal surface: {e}"))
            .ok()
            .map(|m| {
                let layout = self
                    .materials
                    .texture_matrices(m.shader.did)
                    .unwrap_or_default();
                let p = ParticleSurface::of(&m, &layout);
                let wrap = slot_wrap(&m, ids::DIFFUSE);
                Arc::new(DecalLook {
                    texture: p
                        .texture
                        .map(|d| TexRequest::new(d.0, VARIANT_OWN_ALPHA).with_wrap(wrap)),
                    alpha: super::data::surface_alpha(&m, p.alpha_test),
                    material: p.material_colour,
                    vertex_colour: vertex_colour_applies(m.shader.did.0),
                    fogged: p.program.fogged,
                })
            });
        let mut c = lock(&self.looks);
        if c.len() < MAX_CACHED_LOOKS {
            c.insert(surface, l.clone());
        }
        l
    }

    /// Appends the triangles of a receiving object (`parts` in the setup's frame,
    /// placed by `position`, `rotation` and `scale`) to `out` when it may reach the box;
    /// `true` when one of its part meshes reaches the box's search sphere.
    #[allow(clippy::too_many_arguments)]
    fn object_receivers(
        &self,
        b: &tessera_world::decals::DecalBox,
        parts: &[Did],
        position: Vec3,
        rotation: Quat,
        scale: Vec3,
        budget: &mut ReceiverBudget,
        out: &mut Vec<tessera_world::decals::ReceiverTriangle>,
    ) -> bool {
        let c = Vec3::from_array(b.center);
        let reach = b.terrain_half_width() + RECEIVER_SEARCH_MARGIN;
        let s_max = scale.abs().max_element();
        let meshes: Vec<Arc<PartMesh>> = parts.iter().filter_map(|p| self.mesh(*p)).collect();
        // §4.3 step 1: a model with a skinned part receives nothing.
        if meshes.iter().any(|m| m.skinned) {
            return false;
        }
        let mut reached = false;
        for m in meshes {
            let within = (position - c).length() <= reach + m.radius * s_max;
            if !within {
                continue;
            }
            reached = true;
            mesh_receivers(
                b,
                &m.positions,
                &m.normals,
                m.triangles.iter().copied(),
                |p| (position + rotation * (Vec3::from_array(p) * scale)).to_array(),
                |n| (rotation * Vec3::from_array(n)).to_array(),
                budget,
                out,
            );
        }
        reached
    }
}

/// A property's value as a flag (a boolean, or a number that is not 0).
fn flag(s: &PropertySet, pid: u32) -> Option<bool> {
    match s.get(pid)? {
        PropertyValue::Bool(b) => Some(*b),
        v => v.as_f64().map(|x| x != 0.0),
    }
}

/// §5.1 with the Q4 Decision: `Render_AllowDecals` and `Render_ForceDecals` are each looked
/// up on their own, the receiver's own value (`own`) before its chain's (`chain`); then
/// [`tessera_world::decals::entity_accepts_decals`].
#[must_use]
pub fn receiver_accepts(
    own: (Option<bool>, Option<bool>),
    chain: (Option<bool>, Option<bool>),
) -> bool {
    tessera_world::decals::entity_accepts_decals(own.0.or(chain.0), own.1.or(chain.1))
}

/// Whether a decal's box is small enough to be built ([`MAX_DECAL_HALF_WIDTH`]).
fn buildable(b: &tessera_world::decals::DecalBox) -> bool {
    let hw = b.terrain_half_width();
    hw.is_finite() && hw <= MAX_DECAL_HALF_WIDTH
}

/// A built decal: the object, its mesh and look, its draw-distance fade.
#[derive(Debug)]
pub struct BuiltDecal {
    /// The decal object (landblock frame, or the scene's frame on the stage).
    pub decal: PlacedDecal,
    /// Its mesh (same frame).
    pub mesh: DecalMesh,
    /// Its surface's look (`None`: does not resolve, nothing drawn).
    pub look: Option<Arc<DecalLook>>,
    fade: DrawFade,
}

/// The 3 × 3 window of bundles around a landblock, [dy + 1][dx + 1].
struct Window([[Option<Arc<LandblockBundle>>; 3]; 3]);

impl Window {
    fn get(&self, dx: i32, dy: i32) -> Option<&Arc<LandblockBundle>> {
        let row = usize::try_from(dy + 1).ok()?;
        let col = usize::try_from(dx + 1).ok()?;
        self.0.get(row)?.get(col)?.as_ref()
    }
}

impl TerrainGrid for Window {
    fn data(&self, dx: i32, dy: i32) -> Option<&LandblockData> {
        self.get(dx, dy).map(|b| &b.data)
    }
}

/// The water plane height of the bundle for `StickToWater` (§4.3 step 3, OPEN (c)
/// Tessera Decision: the landblock water plane).
fn water_height(bundle: &LandblockBundle) -> Option<f32> {
    bundle
        .water
        .as_ref()
        .and_then(|w| w.water())
        .map(|w| w.height)
        .filter(|h| h.is_finite())
}

fn quat(q: tessera_formats::setup::Quaternion) -> Quat {
    let r = Quat::from_xyzw(q.x, q.y, q.z, q.w);
    if r.is_finite() && r.length_squared() > 1e-12 {
        r.normalize()
    } else {
        Quat::IDENTITY
    }
}

/// The shared budgets of one build (several decals): receiving triangles collected and
/// output vertices kept.
struct BuildBudget {
    receivers: usize,
    vertices: usize,
    warned: bool,
}

impl BuildBudget {
    fn new() -> Self {
        Self {
            receivers: MAX_BLOCK_RECEIVER_TRIANGLES,
            vertices: MAX_BLOCK_DECAL_VERTICES,
            warned: false,
        }
    }

    /// The receiver budget of the next decal.
    fn receivers(&self) -> ReceiverBudget {
        ReceiverBudget::with_limit(self.receivers)
    }

    /// Accounts the decal's receivers and returns its mesh, emptied when its vertices do
    /// not fit what is left.
    fn finish(&mut self, used: &ReceiverBudget, mesh: DecalMesh) -> DecalMesh {
        self.receivers = self.receivers.saturating_sub(used.used());
        if mesh.vertices.len() <= self.vertices {
            self.vertices -= mesh.vertices.len();
            return mesh;
        }
        if !self.warned {
            self.warned = true;
            tracing::debug!(
                limit = MAX_BLOCK_DECAL_VERTICES,
                "decal meshes beyond the build's vertex limit left out"
            );
        }
        DecalMesh::default()
    }
}

/// Builds the decals of the landblock at the centre of `w` (module docs).
fn build_block(data: &DecalData, ctx: &WorldContext, w: &Window) -> Vec<BuiltDecal> {
    let Some(centre) = w.get(0, 0) else {
        return Vec::new();
    };
    let active = &ctx.options().active_content_layers;
    let seds = decal_seds(centre, active);
    if seds.is_empty() {
        return Vec::new();
    }
    let default = data.default_material(Some(ctx));
    let lod = ctx.systems().lod_classes.as_deref();
    let mut out = Vec::new();
    let mut shared = BuildBudget::new();
    for sed in seds.into_iter().take(MAX_DECALS_PER_BLOCK) {
        let e = sed.entity;
        let chain = e.object.map(|o| data.chain(o)).unwrap_or_default();
        let root_first: Vec<&PropertySet> = chain.iter().rev().collect();
        let decal = PlacedDecal::realise(
            &data.pids,
            default,
            &root_first,
            sed.own,
            e.placement.position,
            e.placement.rotation,
            e.iid,
            e.interior,
            lod,
        );
        let p = decal.params;
        let b = decal.volume;
        if !buildable(&b) {
            tracing::debug!(
                half_width = b.terrain_half_width(),
                max = MAX_DECAL_HALF_WIDTH,
                "decal box too large; not built"
            );
            out.push(BuiltDecal {
                decal,
                mesh: DecalMesh::default(),
                look: data.look(p.material),
                fade: DrawFade::new(),
            });
            continue;
        }
        let mut budget = shared.receivers();
        let mut tris = Vec::new();
        // §4.3 step 1: placed objects first.
        if p.stick_to_world_entities {
            let mut tested = 0usize;
            let reach = b.terrain_half_width() + RECEIVER_SEARCH_MARGIN + MAX_RECEIVER_RADIUS;
            'blocks: for dy in -1..=1 {
                for dx in -1..=1 {
                    let Some(bundle) = w.get(dx, dy) else {
                        continue;
                    };
                    #[allow(clippy::cast_precision_loss)]
                    let off = Vec3::new(
                        dx as f32 * LANDBLOCK_UNITS,
                        dy as f32 * LANDBLOCK_UNITS,
                        0.0,
                    );
                    let info = bundle.info.as_ref();
                    let placed = bundle
                        .entities
                        .iter()
                        .map(|e| {
                            let own = info.and_then(|i| {
                                i.entities
                                    .get(usize::try_from(e.index).ok()?)?
                                    .properties
                                    .as_ref()
                            });
                            (e, own)
                        })
                        .chain(bundle.generated.iter().map(|e| (e, None)));
                    for (r, own) in placed {
                        if !r.draw || r.figure.is_some() {
                            continue;
                        }
                        let Some(visual) = r.visual.as_ref().filter(|v| v.has_geometry()) else {
                            continue;
                        };
                        let pos = off + Vec3::from_array(r.placement.position);
                        let near = (pos - Vec3::from_array(b.center)).length() <= reach;
                        if !near {
                            continue;
                        }
                        if !data.accepts(data.own_flags(own), r.object) {
                            continue;
                        }
                        let scale = Vec3::from_array(r.scale);
                        let scale = if scale.is_finite() { scale } else { Vec3::ONE };
                        // Only objects whose meshes reach the box count against the cap.
                        if data.object_receivers(
                            &b,
                            &visual.parts,
                            pos,
                            quat(r.placement.rotation),
                            scale,
                            &mut budget,
                            &mut tris,
                        ) {
                            tested += 1;
                            if tested >= MAX_RECEIVER_OBJECTS {
                                break 'blocks;
                            }
                        }
                    }
                }
            }
        }
        // §4.3 step 2: terrain, outdoor decals only.
        if p.stick_to_landscape && !decal.interior {
            terrain_receivers(&b, w, &mut budget, &mut tris);
        }
        if budget.overflowed() {
            tracing::debug!(
                limit = MAX_RECEIVER_TRIANGLES,
                "decal receivers beyond the limit left out"
            );
        }
        let water = p.stick_to_water.then(|| water_height(centre)).flatten();
        let mesh = shared.finish(&budget, build_mesh(&p, &b, tris, water));
        let look = data.look(p.material);
        out.push(BuiltDecal {
            decal,
            mesh,
            look,
            fade: DrawFade::new(),
        });
    }
    out
}

/// A Decal element of a creation scene and the scene's receiving objects.
#[derive(Debug, Clone)]
pub struct StageDecals {
    /// The scene.
    pub scene: Did,
    /// Its decal objects (scene frame).
    pub decals: Vec<PlacedDecal>,
    /// The scene's drawn Physics elements as receivers: (object, its parts, model
    /// matrix, the element's own `Render_AllowDecals` and `Render_ForceDecals`).
    pub receivers: Vec<StageReceiver>,
}

/// One receiving object of a creation scene.
#[derive(Debug, Clone)]
pub struct StageReceiver {
    /// The 0x47 object (its chain gives the receiving properties the element lacks).
    pub object: Did,
    /// The parts drawn (setup frame).
    pub parts: Arc<[Did]>,
    /// Position in the scene.
    pub position: Vec3,
    /// Rotation.
    pub rotation: Quat,
    /// Scale.
    pub scale: Vec3,
    /// The element's own receiving values (§5.1): `Render_AllowDecals`,
    /// `Render_ForceDecals`.
    pub own: (Option<bool>, Option<bool>),
}

fn build_stage(data: &DecalData, s: &StageDecals) -> Vec<BuiltDecal> {
    let mut out = Vec::new();
    let mut shared = BuildBudget::new();
    for decal in s.decals.iter().take(MAX_STAGE_DECALS) {
        let p = decal.params;
        let b = decal.volume;
        let mut budget = shared.receivers();
        let mut tris = Vec::new();
        if p.stick_to_world_entities && buildable(&b) {
            for r in s.receivers.iter().take(MAX_RECEIVER_OBJECTS * 4) {
                // §5.1: each property on its own, the element's value before the chain's.
                if !data.accepts(r.own, Some(r.object)) {
                    continue;
                }
                let _ = data.object_receivers(
                    &b,
                    &r.parts,
                    r.position,
                    r.rotation,
                    r.scale,
                    &mut budget,
                    &mut tris,
                );
            }
        }
        let mesh = if buildable(&b) {
            shared.finish(&budget, build_mesh(&p, &b, tris, None))
        } else {
            DecalMesh::default()
        };
        out.push(BuiltDecal {
            decal: decal.clone(),
            mesh,
            look: data.look(p.material),
            fade: DrawFade::new(),
        });
    }
    out
}

struct BlockDecals {
    /// Identities of the 3 × 3 bundles the decals are wanted for.
    identity: [[usize; 3]; 3],
    /// The decals drawn: the last finished build (kept while a rebuild runs, so that a
    /// neighbour entering or leaving residency does not make them blink).
    decals: Vec<BuiltDecal>,
    /// The build running for `identity`: its job number and token.
    job: Option<(u64, CancelToken)>,
    /// The decals are built for `identity`.
    done: bool,
}

impl BlockDecals {
    fn cancel(&mut self) {
        if let Some((_, t)) = self.job.take() {
            t.cancel();
        }
    }
}

/// A finished landblock build: the landblock, the job's number, the decals.
type BuildMsg = (LandblockKey, u64, Vec<BuiltDecal>);

/// The creation scene's decals: the scene, the build in flight, the built decals.
type StageBuild = (
    Did,
    Option<mpsc::Receiver<Vec<BuiltDecal>>>,
    Vec<BuiltDecal>,
);

/// The distinct textures of `decals`.
fn textures_of(decals: &[BuiltDecal]) -> HashSet<TexRequest> {
    decals
        .iter()
        .filter_map(|d| d.look.as_ref().and_then(|l| l.texture))
        .collect()
}

/// Vertices of `decals` together.
fn vertices_of(decals: &[BuiltDecal]) -> usize {
    decals.iter().map(|d| d.mesh.vertices.len()).sum()
}

/// One more holder (a landblock's or the stage's decals) of each texture of `decals`; the
/// first acquires it in the object layer.
fn hold(
    held: &mut HashMap<TexRequest, usize>,
    decals: &[BuiltDecal],
    objects: &mut dyn TextureStore,
) {
    let mut new = Vec::new();
    for t in textures_of(decals) {
        let n = held.entry(t).or_insert(0);
        if *n == 0 {
            new.push(t);
        }
        *n += 1;
    }
    if !new.is_empty() {
        objects.acquire(&new);
    }
}

/// One holder fewer of each texture of `decals`; the last releases it.
fn unhold(
    held: &mut HashMap<TexRequest, usize>,
    decals: &[BuiltDecal],
    objects: &mut dyn TextureStore,
) {
    let mut gone = Vec::new();
    for t in textures_of(decals) {
        if let Some(n) = held.get_mut(&t) {
            *n = n.saturating_sub(1);
            if *n == 0 {
                held.remove(&t);
                gone.push(t);
            }
        }
    }
    if !gone.is_empty() {
        objects.release(&gone);
    }
}

/// Placed decals (module docs).
pub struct DecalLayer {
    pool: Option<WorkerPool>,
    boot: Option<mpsc::Receiver<Result<Arc<DecalData>, String>>>,
    data: Option<Arc<DecalData>>,
    blocks: HashMap<LandblockKey, BlockDecals>,
    next_job: u64,
    tx: mpsc::Sender<BuildMsg>,
    rx: mpsc::Receiver<BuildMsg>,
    stage: Option<StageBuild>,
    /// Textures acquired in the object layer, with the number of holders naming each.
    textures: HashMap<TexRequest, usize>,
    /// Vertices of every landblock's decals together ([`MAX_LAYER_DECAL_VERTICES`]).
    held_vertices: usize,
    /// The batch bound was reported.
    batches_warned: bool,
    problem: Option<String>,
    stats: DecalStats,
}

impl std::fmt::Debug for DecalLayer {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("DecalLayer")
            .field("stats", &self.stats)
            .finish_non_exhaustive()
    }
}

impl Default for DecalLayer {
    /// A layer that does nothing (no archives).
    fn default() -> Self {
        let (tx, rx) = mpsc::channel();
        Self {
            pool: None,
            boot: None,
            data: None,
            blocks: HashMap::new(),
            next_job: 0,
            tx,
            rx,
            stage: None,
            textures: HashMap::new(),
            held_vertices: 0,
            batches_warned: false,
            problem: None,
            stats: DecalStats::default(),
        }
    }
}

/// What [`DecalLayer::draw`] needs from the walker's frame.
#[derive(Debug, Clone, Copy)]
pub struct DecalView<'a> {
    /// The render anchor.
    pub anchor: &'a Anchor,
    /// The camera (render frame).
    pub eye: Vec3,
    /// The global limit *G* (§7 step 2).
    pub g: f32,
    /// The `ObjectDrawDistance` multiplier *M_d*.
    pub multiplier: f32,
    /// Frame time, seconds (the 1.5 s fade, §7 step 3).
    pub dt: f32,
    /// The camera's travel since the previous frame (a jump shows or hides at once).
    pub camera_moved: f32,
    /// The camera is in an interior cell (`entity-descriptor.md` §4.8 module rule).
    pub inside: bool,
    /// The frame's lighting (§6.3); `None`: *L* = (1, 1, 1, 1) (§9.2).
    pub lighting: Option<EffectLighting<'a>>,
    /// The depth-bias table (§6.5).
    pub depth_bias: Option<&'a DepthBiasTable>,
}

/// The decal's light colour *L* (§6.3) for `lighting`, at the mesh centre `centre`
/// (render frame) with mean normal `normal`.
fn light_of(lighting: &EffectLighting<'_>, centre: Vec3, normal: [f32; 3]) -> [f32; 4] {
    let mut lights: Vec<DecalLight> = Vec::new();
    if let Some((colour, toward)) = lighting.main {
        lights.push(DecalLight {
            kind: DecalLightKind::Directional {
                direction: (-toward).to_array(),
            },
            colour,
            intensity: 1.0,
        });
    }
    // The table's point lights whose selection radius reaches the centre, key first,
    // nearest first (`lighting.md` §8.5), at most eight.
    let mut reached: Vec<(bool, f32, DecalLight)> = Vec::new();
    for (i, l) in lighting.lights.iter().enumerate() {
        let tessera_render::LightKind::Point {
            position,
            range,
            attenuation,
        } = l.kind
        else {
            continue;
        };
        let r = lighting.reach.get(i);
        if r.is_some_and(|r| r.only_influence_parent) {
            continue;
        }
        let rho = r
            .and_then(|r| r.radius)
            .filter(|v| v.is_finite() && *v >= 0.0)
            .unwrap_or(range);
        let d2 = (position - centre).length_squared();
        let reaches = d2 < rho * rho;
        if !reaches {
            continue;
        }
        let falloff = attenuation * range * range / ATTENUATION_SCALE;
        reached.push((
            !r.is_some_and(|r| r.key_light),
            d2,
            DecalLight {
                kind: DecalLightKind::Point {
                    position: position.to_array(),
                    range,
                    falloff: if falloff.is_finite() { falloff } else { 0.0 },
                },
                colour: l.color,
                intensity: 1.0,
            },
        ));
    }
    reached.sort_by(|a, b| a.0.cmp(&b.0).then(a.1.total_cmp(&b.1)));
    lights.extend(
        reached
            .into_iter()
            .take(MAX_DECAL_LIGHTS)
            .map(|(_, _, l)| l),
    );
    decal_light(
        lighting.ambient,
        &lights,
        normal,
        centre.to_array(),
        [1.0; 4],
    )
}

/// The batch key of §6.6: render phase, depth-bias entry (an index of 16 or more is entry
/// 0, §6.5, so hostile indices do not split batches), material.
type BatchKey = (u32, u32, Did);

/// One batch of §6.6 being filled: its key, vertices and look, and the decals and
/// triangles in it.
struct Batch {
    key: BatchKey,
    vertices: Vec<GpuDecalVertex>,
    look: Arc<DecalLook>,
    decals: usize,
    triangles: usize,
}

/// The batch key of a decal's parameters.
fn batch_key(p: &tessera_world::decals::DecalParams) -> BatchKey {
    let bias = usize::try_from(p.depth_bias)
        .ok()
        .filter(|&i| i < tessera_world::decals::shading::DEPTH_BIAS_ENTRIES)
        .map_or(0, |_| p.depth_bias);
    (p.render_phase, bias, p.material)
}

/// Appends one decal's triangles to its batch (§6.2–§6.4): the per-decal colour with
/// light `light` and opacity `opacity`, the height fade per vertex, positions moved by
/// `offset`. `false` when it adds nothing: no triangle, the vertex `budget` spent, or a
/// new batch beyond [`MAX_DRAWN_BATCHES`].
#[allow(clippy::too_many_arguments)]
fn push_decal(
    batches: &mut Vec<Batch>,
    index: &mut HashMap<BatchKey, usize>,
    d: &BuiltDecal,
    look: &Arc<DecalLook>,
    light: [f32; 4],
    opacity: f32,
    offset: Vec3,
    budget: &mut usize,
) -> bool {
    let p = &d.decal.params;
    let n = d.mesh.vertices.len() / 3 * 3;
    if n == 0 || *budget < n {
        return false;
    }
    let key = batch_key(p);
    let slot = match index.get(&key) {
        Some(&i) => i,
        None if batches.len() >= MAX_DRAWN_BATCHES => return false,
        None => {
            batches.push(Batch {
                key,
                vertices: Vec::new(),
                look: Arc::clone(look),
                decals: 0,
                triangles: 0,
            });
            index.insert(key, batches.len() - 1);
            batches.len() - 1
        }
    };
    let Some(batch) = batches.get_mut(slot) else {
        return false;
    };
    *budget -= n;
    let packed = decal_colour(p.colour, light, opacity, p.lighting_blend);
    let colour = packed.map(|c| f32::from(c) / 255.0);
    let zc = d.decal.volume.center[2];
    for v in d.mesh.vertices.iter().take(n) {
        let mut c = colour;
        if p.use_height_based_opacity {
            c[3] = height_alpha(colour[3], v.position[2], zc, p.height_based_opacity_range);
        }
        batch.vertices.push(GpuDecalVertex {
            position: (Vec3::from_array(v.position) + offset).to_array(),
            color: c,
            uv: v.uv,
        });
    }
    batch.decals += 1;
    batch.triangles += n / 3;
    true
}

/// What [`queue_batches`] queued.
#[derive(Debug, Clone, Copy, Default, PartialEq, Eq)]
struct Queued {
    /// Batches queued.
    batches: usize,
    /// Batches left out while their texture loads.
    waiting: usize,
    /// Decals in the batches queued.
    decals: usize,
    /// Their triangles.
    triangles: usize,
}

/// Queues `batches` on `pass` (a batch whose texture is still loading is left out).
fn queue_batches(
    pass: &mut Pass,
    batches: Vec<Batch>,
    objects: &ObjectLayer,
    depth_bias: Option<&DepthBiasTable>,
) -> Queued {
    let mut q = Queued::default();
    for b in batches {
        let texture: Option<TextureId> = match b.look.texture.map(|t| objects.texture(t)) {
            None | Some(TexLookup::Failed) => None,
            Some(TexLookup::Ready(id)) => Some(id),
            Some(TexLookup::Pending) => {
                q.waiting += 1;
                continue;
            }
        };
        pass.decal(DecalDraw {
            vertices: b.vertices,
            texture,
            alpha: b.look.alpha,
            material: b.look.material,
            vertex_colour: b.look.vertex_colour,
            fogged: b.look.fogged,
            two_sided: true,
            depth_bias: crate::walker::terrain::entity_depth_bias(depth_bias, Some(b.key.1)),
        });
        q.batches += 1;
        q.decals += b.decals;
        q.triangles += b.triangles;
    }
    q
}

impl DecalLayer {
    /// A layer reading `set` with the walker's material library, building on `pool`.
    #[must_use]
    pub fn new(
        set: Arc<ArchiveSet>,
        materials: Arc<MaterialLibrary<Arc<ArchiveSet>>>,
        pool: WorkerPool,
    ) -> Self {
        let (tx, rx) = mpsc::sync_channel(1);
        let job = move |_: &tessera_assets::JobContext<'_>| {
            let _ = tx.send(DecalData::open(set, materials).map(Arc::new));
        };
        let mut out = Self::default();
        match pool.spawn(Priority::Visible, CancelToken::new(), job) {
            Ok(_) => out.boot = Some(rx),
            Err(e) => out.problem = Some(format!("decal start-up job: {e}")),
        }
        out.pool = Some(pool);
        out
    }

    /// Counters.
    #[must_use]
    pub fn stats(&self) -> DecalStats {
        self.stats
    }

    /// Whether nothing is loading: start-up done, every landblock with decals built for
    /// its current neighbours (also those still waiting for a build slot), the creation
    /// scene's decals built, no texture of a drawn decal loading.
    #[must_use]
    pub fn settled(&self) -> bool {
        if self.pool.is_none() || self.problem.is_some() {
            return true;
        }
        self.boot.is_none()
            && self.blocks.values().all(|b| b.done)
            && self.stage.as_ref().is_none_or(|s| s.1.is_none())
            && self.stats.textures_pending == 0
    }

    /// Releases every texture the layer holds in `objects` and drops its decals (the
    /// object layer is cleared: a region change). Landblocks and the creation scene are
    /// built again, and their textures acquired again, by the next updates.
    pub fn release(&mut self, objects: &mut dyn TextureStore) {
        for (_, mut b) in self.blocks.drain() {
            b.cancel();
        }
        self.stage = None;
        let all: Vec<TexRequest> = self.textures.drain().map(|(t, _)| t).collect();
        if !all.is_empty() {
            objects.release(&all);
        }
        self.held_vertices = 0;
        self.stats.textures_pending = 0;
        self.stats.drawn = 0;
        self.stats.drawn_triangles = 0;
        self.stats.batches = 0;
        self.stats.stage_drawn = 0;
        self.stats.stage_triangles = 0;
        self.refresh_stats();
    }

    fn poll_boot(&mut self) {
        let Some(rx) = &self.boot else { return };
        match rx.try_recv() {
            Ok(Ok(d)) => {
                self.data = Some(d);
                self.boot = None;
            }
            Ok(Err(e)) => {
                tracing::warn!("decals: {e}");
                self.problem = Some(e);
                self.boot = None;
            }
            Err(mpsc::TryRecvError::Empty) => {}
            Err(mpsc::TryRecvError::Disconnected) => {
                self.problem = Some("decal start-up job lost".into());
                self.boot = None;
            }
        }
    }

    /// Takes a finished build of `key` by job `job` (a build that is no longer wanted is
    /// dropped): the layer's vertex bound, the textures, the block's decals.
    fn arrive(
        &mut self,
        key: LandblockKey,
        job: u64,
        mut built: Vec<BuiltDecal>,
        objects: &mut dyn TextureStore,
    ) {
        let Some(b) = self.blocks.get_mut(&key) else {
            return;
        };
        if b.job.as_ref().map(|j| j.0) != Some(job) {
            return;
        }
        let old = vertices_of(&b.decals);
        let mut room =
            MAX_LAYER_DECAL_VERTICES.saturating_sub(self.held_vertices.saturating_sub(old));
        let mut cut = false;
        for d in &mut built {
            let n = d.mesh.vertices.len();
            if n <= room {
                room -= n;
            } else {
                d.mesh = DecalMesh::default();
                cut = true;
            }
        }
        if cut {
            tracing::debug!(
                limit = MAX_LAYER_DECAL_VERTICES,
                "decal meshes beyond the layer's vertex limit left out"
            );
        }
        let new = vertices_of(&built);
        hold(&mut self.textures, &built, objects);
        let previous = std::mem::replace(&mut b.decals, built);
        b.job = None;
        b.done = true;
        unhold(&mut self.textures, &previous, objects);
        self.held_vertices = self.held_vertices.saturating_sub(old) + new;
    }

    /// Drops the decals of a landblock entry (it left residency or holds no decal now).
    fn forget(&mut self, mut b: BlockDecals, objects: &mut dyn TextureStore) {
        b.cancel();
        self.held_vertices = self.held_vertices.saturating_sub(vertices_of(&b.decals));
        unhold(&mut self.textures, &b.decals, objects);
    }

    /// Per frame: realises and builds the decals of the resident landblocks on the
    /// workers (module docs) and takes the finished builds.
    pub fn update(&mut self, streamer: &WorldStreamer, objects: &mut ObjectLayer) {
        self.poll_boot();
        let Some(data) = self.data.clone() else {
            return;
        };
        let Some(pool) = self.pool.clone() else {
            return;
        };
        let resident: HashMap<LandblockKey, &Arc<LandblockBundle>> =
            streamer.resident().map(|(k, _, b)| (k, b)).collect();
        // Landblocks that left.
        let gone: Vec<LandblockKey> = self
            .blocks
            .keys()
            .filter(|k| !resident.contains_key(k))
            .copied()
            .collect();
        for k in gone {
            if let Some(b) = self.blocks.remove(&k) {
                self.forget(b, objects);
            }
        }
        let ctx = Arc::clone(streamer.context());
        let mut in_flight = self.blocks.values().filter(|b| b.job.is_some()).count();
        for (&key, bundle) in &resident {
            let mut window: [[Option<Arc<LandblockBundle>>; 3]; 3] = Default::default();
            let mut identity = [[0usize; 3]; 3];
            for (dy, (wrow, irow)) in window.iter_mut().zip(identity.iter_mut()).enumerate() {
                for (dx, (w, id)) in wrow.iter_mut().zip(irow.iter_mut()).enumerate() {
                    #[allow(clippy::cast_possible_wrap, clippy::cast_possible_truncation)]
                    let k = key.offset(dx as i32 - 1, dy as i32 - 1);
                    if let Some(b) = k.and_then(|k| resident.get(&k)) {
                        *id = Arc::as_ptr(b) as usize;
                        *w = Some(Arc::clone(b));
                    }
                }
            }
            if self
                .blocks
                .get(&key)
                .is_some_and(|b| b.identity == identity && (b.done || b.job.is_some()))
            {
                continue;
            }
            // Only landblocks with Decal SEDs need a build.
            let has = bundle
                .entities
                .iter()
                .any(|e| e.kind == Some(StaticEntityType::Decal));
            if !has {
                let none = BlockDecals {
                    identity,
                    decals: Vec::new(),
                    job: None,
                    done: true,
                };
                if let Some(old) = self.blocks.insert(key, none) {
                    self.forget(old, objects);
                }
                continue;
            }
            // Wanted for these neighbours: an entry that is not done (also while it waits
            // for a build slot, so that the layer is not settled meanwhile).
            let entry = self.blocks.entry(key).or_insert_with(|| BlockDecals {
                identity,
                decals: Vec::new(),
                job: None,
                done: false,
            });
            if entry.identity != identity {
                entry.cancel();
                entry.identity = identity;
                entry.done = false;
            }
            if in_flight >= MAX_BUILDS_IN_FLIGHT {
                continue;
            }
            let token = CancelToken::new();
            let job_id = self.next_job;
            self.next_job = self.next_job.wrapping_add(1);
            let tx = self.tx.clone();
            let (data, ctx) = (Arc::clone(&data), Arc::clone(&ctx));
            let w = Window(window);
            let job = move |jc: &tessera_assets::JobContext<'_>| {
                let built = if jc.is_cancelled() {
                    Vec::new()
                } else {
                    build_block(&data, &ctx, &w)
                };
                let _ = tx.send((key, job_id, built));
            };
            match pool.spawn(Priority::Prefetch, token.clone(), job) {
                Ok(_) => {
                    in_flight += 1;
                    entry.job = Some((job_id, token));
                }
                Err(e) => tracing::debug!("decals: build not queued: {e}"),
            }
        }
        while let Ok((key, job, built)) = self.rx.try_recv() {
            self.arrive(key, job, built, objects);
        }
        self.refresh_stats();
    }

    fn refresh_stats(&mut self) {
        let mut s = DecalStats {
            drawn: self.stats.drawn,
            drawn_triangles: self.stats.drawn_triangles,
            batches: self.stats.batches,
            left_out: self.stats.left_out,
            textures_pending: self.stats.textures_pending,
            nearest: self.stats.nearest,
            stage_drawn: self.stats.stage_drawn,
            stage_triangles: self.stats.stage_triangles,
            pending: self.blocks.values().filter(|b| !b.done).count(),
            ..DecalStats::default()
        };
        for b in self.blocks.values() {
            if !b.decals.is_empty() {
                s.blocks += 1;
            }
            for d in &b.decals {
                s.decals += 1;
                let t = d.mesh.triangle_count();
                if t > 0 {
                    s.built += 1;
                }
                s.triangles += t;
                s.looks_failed += usize::from(d.look.is_none());
            }
        }
        if let Some((_, _, built)) = &self.stage {
            s.stage_decals = built.len();
            s.stage_built = built.iter().filter(|d| d.mesh.triangle_count() > 0).count();
            s.looks_failed += built.iter().filter(|d| d.look.is_none()).count();
        }
        self.stats = s;
    }

    /// Reports decals left out by the batch bound, once.
    fn note_left_out(&mut self, left_out: usize) {
        self.stats.left_out = left_out;
        if left_out > 0 && !self.batches_warned {
            self.batches_warned = true;
            tracing::warn!(
                left_out,
                max = MAX_DRAWN_BATCHES,
                "decals beyond the pass's batch or vertex bound left out"
            );
        }
    }

    /// Queues the decals of the frame into the world `pass` (§6.6, §7; module docs).
    pub fn draw(&mut self, pass: &mut Pass, view: &DecalView<'_>, objects: &ObjectLayer) {
        let mut keys: Vec<LandblockKey> = self.blocks.keys().copied().collect();
        keys.sort();
        let mut batches = Vec::new();
        let mut index = HashMap::new();
        let mut budget = MAX_DRAWN_VERTICES;
        let mut left_out = 0usize;
        let mut nearest = (f32::INFINITY, 0.0f32);
        let eye = view.eye.to_array();
        for key in keys {
            let Some(offset) = view.anchor.offset_of(key) else {
                continue;
            };
            let Some(block) = self.blocks.get_mut(&key) else {
                continue;
            };
            for d in &mut block.decals {
                // Outdoor decals for an outdoor camera, interior ones for an interior one.
                if d.decal.interior != view.inside || d.mesh.vertices.is_empty() {
                    continue;
                }
                let Some(look) = d.look.clone() else { continue };
                let (lo, hi) = d.decal.volume.scene_bounds();
                let lo = (Vec3::from_array(lo) + offset).to_array();
                let hi = (Vec3::from_array(hi) + offset).to_array();
                let dist = aabb_distance(eye, lo, hi);
                let range = draw_range(view.g, d.decal.lod_distance, view.multiplier);
                if d.mesh.triangle_count() > 0 && dist.is_finite() && dist < nearest.0 {
                    nearest = (dist, range);
                }
                let fade = d
                    .fade
                    .update(dist, range, view.dt, view.camera_moved, false, true);
                if fade <= 0.0 {
                    continue;
                }
                let p = &d.decal.params;
                let mut opacity = fade;
                if p.use_depth_based_opacity {
                    opacity *=
                        depth_opacity(p.depth_based_opacity_min, p.depth_based_opacity_range, dist);
                }
                let light = match (&view.lighting, p.is_lit()) {
                    (Some(l), true) => {
                        let c = d.mesh.center.unwrap_or(d.decal.volume.center);
                        light_of(
                            l,
                            Vec3::from_array(c) + offset,
                            d.mesh.mean_normal.unwrap_or([0.0; 3]),
                        )
                    }
                    _ => [1.0; 4],
                };
                if !push_decal(
                    &mut batches,
                    &mut index,
                    d,
                    &look,
                    light,
                    opacity,
                    offset,
                    &mut budget,
                ) && d.mesh.triangle_count() > 0
                {
                    left_out += 1;
                }
            }
        }
        let q = queue_batches(pass, batches, objects, view.depth_bias);
        self.stats.drawn = q.decals;
        self.stats.drawn_triangles = q.triangles;
        self.stats.batches = q.batches;
        self.note_left_out(left_out);
        let whole = |v: f32| {
            if v.is_finite() {
                // Clamped into u32 first.
                #[allow(clippy::cast_possible_truncation, clippy::cast_sign_loss)]
                let w = v.clamp(0.0, u32::MAX as f32) as u32;
                w
            } else {
                u32::MAX
            }
        };
        self.stats.nearest = (whole(nearest.0), whole(nearest.1));
        self.stats.textures_pending = q.waiting;
    }

    /// The creation viewport's decals (`character-screens.md` §2.1.1 rule 3): builds the
    /// decals of `scene` against its receivers on a worker, once per scene; `None`
    /// removes them.
    pub fn update_stage(&mut self, scene: Option<&StageDecals>, objects: &mut ObjectLayer) {
        self.poll_boot();
        let want = scene.map(|s| s.scene);
        if self.stage.as_ref().map(|s| s.0) != want {
            if let Some((_, _, old)) = self.stage.take() {
                unhold(&mut self.textures, &old, objects);
            }
            if let (Some(s), Some(data), Some(pool)) = (scene, self.data.clone(), self.pool.clone())
            {
                let (tx, rx) = mpsc::sync_channel(1);
                let did = s.scene;
                let s = s.clone();
                let job = move |_: &tessera_assets::JobContext<'_>| {
                    let _ = tx.send(build_stage(&data, &s));
                };
                match pool.spawn(Priority::Visible, CancelToken::new(), job) {
                    Ok(_) => self.stage = Some((did, Some(rx), Vec::new())),
                    Err(e) => tracing::debug!("decals: creation-scene build not queued: {e}"),
                }
            }
        }
        let arrived = match self.stage.as_ref().and_then(|s| s.1.as_ref()) {
            Some(rx) => match rx.try_recv() {
                Ok(b) => Some(b),
                Err(mpsc::TryRecvError::Disconnected) => Some(Vec::new()),
                Err(mpsc::TryRecvError::Empty) => None,
            },
            None => None,
        };
        if let Some(built) = arrived {
            hold(&mut self.textures, &built, objects);
            if let Some(s) = self.stage.as_mut() {
                s.1 = None;
                s.2 = built;
            }
        }
        self.refresh_stats();
    }

    /// Whether the creation scene's decals are built.
    #[must_use]
    pub fn stage_ready(&self) -> bool {
        self.stage.as_ref().is_some_and(|s| s.1.is_none())
    }

    /// Queues the creation scene's decals into the viewport's `pass` (scene frame = render
    /// frame) with `lighting` (the scene's lights and ambient, §6.3; `None`: *L* = 1) and
    /// the depth-bias table `depth_bias` (§6.5). Returns the decals queued.
    pub fn draw_stage(
        &mut self,
        pass: &mut Pass,
        lighting: Option<EffectLighting<'_>>,
        objects: &ObjectLayer,
        depth_bias: Option<&DepthBiasTable>,
    ) -> usize {
        let Some((_, _, built)) = self.stage.as_ref() else {
            return 0;
        };
        let mut batches = Vec::new();
        let mut index = HashMap::new();
        let mut budget = MAX_DRAWN_VERTICES;
        let mut left_out = 0usize;
        for d in built {
            let Some(look) = d.look.clone() else { continue };
            let p = &d.decal.params;
            let light = match (&lighting, p.is_lit()) {
                (Some(l), true) => light_of(
                    l,
                    Vec3::from_array(d.mesh.center.unwrap_or(d.decal.volume.center)),
                    d.mesh.mean_normal.unwrap_or([0.0; 3]),
                ),
                _ => [1.0; 4],
            };
            if !push_decal(
                &mut batches,
                &mut index,
                d,
                &look,
                light,
                1.0,
                Vec3::ZERO,
                &mut budget,
            ) && d.mesh.triangle_count() > 0
            {
                left_out += 1;
            }
        }
        let q = queue_batches(pass, batches, objects, depth_bias);
        self.stats.stage_drawn = q.decals;
        self.stats.stage_triangles = q.triangles;
        self.stats.textures_pending = q.waiting;
        self.note_left_out(left_out);
        q.decals
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use tessera_world::decals::shading::DepthBias;
    use tessera_world::decals::{DecalBox, DecalParams, DecalVertex};

    fn built(params: DecalParams, vertices: Vec<DecalVertex>) -> BuiltDecal {
        BuiltDecal {
            decal: PlacedDecal {
                iid: None,
                interior: false,
                params,
                volume: DecalBox::new(
                    [0.0; 3],
                    tessera_formats::setup::Quaternion::IDENTITY,
                    [1.0; 3],
                ),
                lod_distance: 40.0,
            },
            mesh: DecalMesh {
                vertices,
                ..DecalMesh::default()
            },
            look: None,
            fade: DrawFade::new(),
        }
    }

    fn look() -> Arc<DecalLook> {
        Arc::new(DecalLook {
            texture: None,
            alpha: AlphaMode::Blend,
            material: [1.0; 4],
            vertex_colour: true,
            fogged: false,
        })
    }

    fn tri() -> Vec<DecalVertex> {
        (0..3)
            .map(|i| DecalVertex {
                position: [i as f32, 0.0, 1.0],
                normal: [0.0, 0.0, 1.0],
                uv: [0.0, 0.0],
            })
            .collect()
    }

    /// §6.6: decals that share phase, depth-bias entry and material go into one batch in
    /// registration order; another material starts a batch; the vertex colour is the
    /// packed per-decal colour (§6.2) / 255 and positions move by the offset.
    #[test]
    fn batches_follow_phase_bias_and_material() {
        let base = DecalParams {
            colour: [0, 128, 255, 204],
            ..DecalParams::initial(Did(0x3100_0001))
        };
        let other = DecalParams {
            material: Did(0x3100_0002),
            ..base
        };
        let l = look();
        let mut batches = Vec::new();
        let mut index = HashMap::new();
        let mut budget = 100;
        let off = Vec3::new(10.0, 0.0, 0.0);
        for p in [base, other, base] {
            assert!(push_decal(
                &mut batches,
                &mut index,
                &built(p, tri()),
                &l,
                [1.0; 4],
                1.0,
                off,
                &mut budget,
            ));
        }
        assert_eq!(batches.len(), 2);
        assert_eq!(batches[0].vertices.len(), 6);
        assert_eq!(batches[1].vertices.len(), 3);
        assert_eq!((batches[0].decals, batches[0].triangles), (2, 2));
        assert_eq!((batches[1].decals, batches[1].triangles), (1, 1));
        let v = batches[0].vertices[1];
        assert_eq!(v.position, [11.0, 0.0, 1.0]);
        // Example 3, mode 0: bytes (255, 128, 0, 204).
        let want = [255.0, 128.0, 0.0, 204.0].map(|c: f32| c / 255.0);
        for (a, b) in v.color.iter().zip(want) {
            assert!((a - b).abs() < 1e-6, "{:?}", v.color);
        }
        // The vertex budget leaves out a decal that does not fit.
        let mut small = 2;
        assert!(!push_decal(
            &mut batches,
            &mut index,
            &built(base, tri()),
            &l,
            [1.0; 4],
            1.0,
            off,
            &mut small,
        ));
        // A mesh without triangles adds nothing.
        assert!(!push_decal(
            &mut batches,
            &mut index,
            &built(base, Vec::new()),
            &l,
            [1.0; 4],
            1.0,
            off,
            &mut budget,
        ));
    }

    /// §6.4: the height fade replaces each vertex alpha.
    #[test]
    fn height_fade_per_vertex() {
        let p = DecalParams {
            use_height_based_opacity: true,
            height_based_opacity_range: 2.0,
            ..DecalParams::initial(Did(1))
        };
        let mut batches = Vec::new();
        let mut index = HashMap::new();
        let mut budget = 100;
        push_decal(
            &mut batches,
            &mut index,
            &built(p, tri()),
            &look(),
            [1.0; 4],
            1.0,
            Vec3::ZERO,
            &mut budget,
        );
        // z_v = 1 and z_c = 0: the test box is centred on the origin with the identity
        // rotation, so its centre lies at height 0. (1 − |1 − 0| / 2) × 1 = 0.5.
        let a = batches[0].vertices[0].color[3];
        assert!((a - 0.5).abs() < 1e-6, "{a}");
    }

    /// §6.3 with example 4 of the spec: ambient (0.2, 0.2, 0.25), one point light at
    /// (80, 84, 107) with colour (1, 0.8, 0.5) × 0.9, range 10, falloff 0.5 (k = 0.05),
    /// mean normal +Z at (80, 80, 104): *L* = (0.44, 0.392, 0.37, 1).
    #[test]
    fn decal_light_of_example_4() {
        let light = tessera_render::Light {
            kind: tessera_render::LightKind::Point {
                position: Vec3::new(80.0, 84.0, 107.0),
                range: 10.0,
                attenuation: 0.05,
            },
            color: [0.9, 0.72, 0.45],
            specular: [0.0; 3],
        };
        let lighting = EffectLighting {
            ambient: [0.2, 0.2, 0.25],
            main: None,
            lights: std::slice::from_ref(&light),
            reach: &[],
            anchor: None,
        };
        let l = light_of(&lighting, Vec3::new(80.0, 80.0, 104.0), [0.0, 0.0, 1.0]);
        let want = [0.44, 0.392, 0.37, 1.0];
        for (a, b) in l.iter().zip(want) {
            assert!((a - b).abs() < 1e-4, "{l:?}");
        }
        // Beyond the selection radius the light is not in the list.
        let far = light_of(&lighting, Vec3::new(80.0, 80.0, 50.0), [0.0, 0.0, 1.0]);
        assert!((far[0] - 0.2).abs() < 1e-6, "{far:?}");
        // The main light: (0.5, 0.5, 0.5) from straight above.
        let sun = EffectLighting {
            main: Some(([0.5; 3], Vec3::Z)),
            lights: &[],
            ..lighting
        };
        let l = light_of(&sun, Vec3::ZERO, [0.0, 0.0, 1.0]);
        assert!((l[0] - 0.7).abs() < 1e-5, "{l:?}");
    }

    /// §6.5 / invariant 13: a batch draws with the entry its `DecalVolume_DepthBias`
    /// selects (slope-scale and maximum of that entry); an index of 16 or more draws with
    /// entry 0.
    #[test]
    fn depth_bias_entries() {
        let mut t = DepthBiasTable::default();
        t.entries[1] = DepthBias {
            absolute: 0.0,
            slope_scale: -1.0,
            max: 0.0,
        };
        let b = crate::walker::terrain::entity_depth_bias(Some(&t), Some(1));
        assert_eq!(b, tessera_render::DrawDepthBias::new(-1.0, 0.0));
        let b = crate::walker::terrain::entity_depth_bias(Some(&t), Some(20));
        assert_eq!(b, tessera_render::DrawDepthBias::new(0.0, 0.0));
    }

    /// §6.5: a depth-bias index of 16 or more is entry 0, so it shares the batch of entry
    /// 0; no more than [`MAX_DRAWN_BATCHES`] batches are opened, the decal that would open
    /// one more is left out.
    #[test]
    fn hostile_bias_indices_share_a_batch_and_batches_are_bounded() {
        let l = look();
        let mut batches = Vec::new();
        let mut index = HashMap::new();
        let mut budget = usize::MAX;
        for bias in [0u32, 16, u32::MAX] {
            let p = DecalParams {
                depth_bias: bias,
                ..DecalParams::initial(Did(0x3100_0001))
            };
            assert!(push_decal(
                &mut batches,
                &mut index,
                &built(p, tri()),
                &l,
                [1.0; 4],
                1.0,
                Vec3::ZERO,
                &mut budget,
            ));
        }
        assert_eq!(batches.len(), 1);
        assert_eq!(batches[0].key.1, 0);
        // Fill the batch bound with distinct render phases.
        for phase in 1..MAX_DRAWN_BATCHES {
            let p = DecalParams {
                render_phase: u32::try_from(phase).unwrap_or(u32::MAX),
                ..DecalParams::initial(Did(0x3100_0001))
            };
            assert!(push_decal(
                &mut batches,
                &mut index,
                &built(p, tri()),
                &l,
                [1.0; 4],
                1.0,
                Vec3::ZERO,
                &mut budget,
            ));
        }
        assert_eq!(batches.len(), MAX_DRAWN_BATCHES);
        let beyond = DecalParams {
            render_phase: u32::MAX,
            ..DecalParams::initial(Did(0x3100_0001))
        };
        let before = budget;
        assert!(!push_decal(
            &mut batches,
            &mut index,
            &built(beyond, tri()),
            &l,
            [1.0; 4],
            1.0,
            Vec3::ZERO,
            &mut budget,
        ));
        assert_eq!(batches.len(), MAX_DRAWN_BATCHES);
        assert_eq!(budget, before, "a decal left out spends no vertex budget");
        // An existing batch still takes decals.
        assert!(push_decal(
            &mut batches,
            &mut index,
            &built(DecalParams::initial(Did(0x3100_0001)), tri()),
            &l,
            [1.0; 4],
            1.0,
            Vec3::ZERO,
            &mut budget,
        ));
    }

    /// §5.1 with the Q4 Decision: each property is looked up on its own, the receiver's
    /// own value before its chain's.
    #[test]
    fn receiver_flags_resolve_each_property_on_its_own() {
        // Own allow = false, the chain forces: accepted (force from the chain).
        assert!(receiver_accepts((Some(false), None), (None, Some(true))));
        // Own force = false is found before the chain's force; the chain's allow = false
        // then rejects.
        assert!(!receiver_accepts(
            (None, Some(false)),
            (Some(false), Some(true))
        ));
        // Own force wins over everything.
        assert!(receiver_accepts(
            (Some(false), Some(true)),
            (Some(false), None)
        ));
        // Nothing set: allowed by default; the chain's allow = false rejects.
        assert!(receiver_accepts((None, None), (None, None)));
        assert!(!receiver_accepts((None, None), (Some(false), None)));
        // Own allow = true overrides the chain's false.
        assert!(receiver_accepts(
            (Some(true), None),
            (Some(false), Some(false))
        ));
    }

    /// A box beyond [`MAX_DECAL_HALF_WIDTH`] (or whose half-width overflows) is not built;
    /// an infinite scale component is already 1 m (`DecalBox::new`).
    #[test]
    fn huge_boxes_are_not_built() {
        let q = tessera_formats::setup::Quaternion::IDENTITY;
        assert!(buildable(&DecalBox::new([0.0; 3], q, [20.0; 3])));
        assert!(!buildable(&DecalBox::new([0.0; 3], q, [1.0e6; 3])));
        assert!(!buildable(&DecalBox::new(
            [0.0; 3],
            q,
            [f32::MAX, 1.0, 1.0]
        )));
    }

    /// The build's shared budgets: receivers handed out shrink with use, and a mesh beyond
    /// the vertex budget is emptied.
    #[test]
    fn build_budgets_are_shared() {
        let mut b = BuildBudget::new();
        let mut r = b.receivers();
        assert!(r.take() && r.take());
        let mesh = DecalMesh {
            vertices: tri(),
            ..DecalMesh::default()
        };
        assert_eq!(b.finish(&r, mesh).vertices.len(), 3);
        assert_eq!(b.receivers, MAX_BLOCK_RECEIVER_TRIANGLES - 2);
        assert_eq!(b.vertices, MAX_BLOCK_DECAL_VERTICES - 3);
        b.vertices = 2;
        let mesh = DecalMesh {
            vertices: tri(),
            ..DecalMesh::default()
        };
        assert!(b.finish(&r, mesh).vertices.is_empty());
    }

    /// Counts references; every texture loads forever.
    #[derive(Default)]
    struct Refs(HashMap<TexRequest, i64>);

    impl TextureStore for Refs {
        fn acquire(&mut self, reqs: &[TexRequest]) {
            for t in reqs {
                *self.0.entry(*t).or_default() += 1;
            }
        }
        fn release(&mut self, reqs: &[TexRequest]) {
            for t in reqs {
                *self.0.entry(*t).or_default() -= 1;
            }
        }
        fn lookup(&self, _t: TexRequest) -> TexLookup {
            TexLookup::Pending
        }
        fn mean(&self, _t: TexRequest) -> Option<[f32; 3]> {
            None
        }
        fn small(&self, _t: TexRequest) -> Option<Arc<[tessera_formats::RgbaImage]>> {
            None
        }
    }

    fn textured(t: u32) -> BuiltDecal {
        let mut d = built(DecalParams::initial(Did(1)), tri());
        d.look = Some(Arc::new(DecalLook {
            texture: Some(TexRequest::new(t, VARIANT_OWN_ALPHA)),
            ..(*look()).clone()
        }));
        d
    }

    fn key(x: u8) -> LandblockKey {
        LandblockKey::new(1, x, 1).expect("a valid key")
    }

    /// A layer that counts as running (so that `settled` looks at its tables).
    fn running_layer() -> DecalLayer {
        DecalLayer {
            pool: Some(WorkerPool::new(tessera_assets::PoolConfig {
                workers: 1,
                ..tessera_assets::PoolConfig::default()
            })),
            ..DecalLayer::default()
        }
    }

    /// Settling follows the block table: a block waiting for a build slot or a build is not
    /// settled; a stale job's result (an older job number) neither finishes the block nor
    /// counts against a live one.
    #[test]
    fn settled_follows_the_block_table() {
        let mut layer = running_layer();
        let mut store = Refs::default();
        assert!(layer.settled());
        // A block waiting for a build slot.
        layer.blocks.insert(
            key(1),
            BlockDecals {
                identity: [[0; 3]; 3],
                decals: Vec::new(),
                job: None,
                done: false,
            },
        );
        layer.refresh_stats();
        assert!(!layer.settled());
        assert_eq!(layer.stats().pending, 1);
        // Its build (job 7) runs; a stale job 6 arrives first and is dropped.
        if let Some(b) = layer.blocks.get_mut(&key(1)) {
            b.job = Some((7, CancelToken::new()));
        }
        layer.arrive(key(1), 6, vec![textured(0x4100_0001)], &mut store);
        assert!(!layer.settled());
        assert!(store.0.is_empty(), "a stale build acquires nothing");
        layer.arrive(key(1), 7, vec![textured(0x4100_0001)], &mut store);
        layer.refresh_stats();
        assert!(layer.settled());
        assert_eq!(layer.stats().pending, 0);
        assert_eq!(
            store
                .0
                .get(&TexRequest::new(0x4100_0001, VARIANT_OWN_ALPHA)),
            Some(&1)
        );
    }

    /// Textures are counted per holder: a rebuild keeps a shared texture, a block that
    /// leaves releases its own, and `release` (the object layer is cleared) gives back
    /// every texture and forgets it, so the next build acquires it again.
    #[test]
    fn textures_follow_their_holders() {
        let mut layer = running_layer();
        let mut store = Refs::default();
        let (a, b) = (
            TexRequest::new(0x4100_0001, VARIANT_OWN_ALPHA),
            TexRequest::new(0x4100_0002, VARIANT_OWN_ALPHA),
        );
        for (k, job, tex) in [(1u8, 1u64, 0x4100_0001u32), (2, 2, 0x4100_0001)] {
            layer.blocks.insert(
                key(k),
                BlockDecals {
                    identity: [[0; 3]; 3],
                    decals: Vec::new(),
                    job: Some((job, CancelToken::new())),
                    done: false,
                },
            );
            layer.arrive(key(k), job, vec![textured(tex), textured(tex)], &mut store);
        }
        assert_eq!(store.0.get(&a), Some(&1), "acquired once for both holders");
        // Block 1 rebuilt with another texture: `a` stays (block 2), `b` is acquired.
        if let Some(e) = layer.blocks.get_mut(&key(1)) {
            e.job = Some((3, CancelToken::new()));
        }
        layer.arrive(key(1), 3, vec![textured(0x4100_0002)], &mut store);
        assert_eq!((store.0.get(&a), store.0.get(&b)), (Some(&1), Some(&1)));
        // Block 2 leaves: `a` is released.
        if let Some(e) = layer.blocks.remove(&key(2)) {
            layer.forget(e, &mut store);
        }
        assert_eq!(store.0.get(&a), Some(&0));
        assert_eq!(layer.held_vertices, 3);
        // The object layer is cleared: everything goes back, nothing is remembered.
        layer.release(&mut store);
        assert_eq!(store.0.get(&b), Some(&0));
        assert!(layer.textures.is_empty() && layer.blocks.is_empty());
        assert_eq!(layer.held_vertices, 0);
        layer.blocks.insert(
            key(1),
            BlockDecals {
                identity: [[0; 3]; 3],
                decals: Vec::new(),
                job: Some((9, CancelToken::new())),
                done: false,
            },
        );
        layer.arrive(key(1), 9, vec![textured(0x4100_0002)], &mut store);
        assert_eq!(store.0.get(&b), Some(&1), "acquired again after the clear");
    }
}
