// SPDX-License-Identifier: GPL-3.0-or-later
//! Static objects of the resident landblocks: part meshes (0x06) with their materials and
//! textures, loaded on the asset workers and drawn per object with the original's
//! distances (`walker.md` §11.4, §14).
//!
//! - **Objects** are the drawable entities of a landblock and its generated scenery
//!   (`scenery.md` §7), one [`ObjectInfo`] each with its part instances ([`Instance`]) and
//!   its LOD-class values (`walker.md` §11.4.3). An object is *activated* (its parts are
//!   requested and reference-counted) while it lies within its draw distance *D* =
//!   min(*G*, `d0` × *M_d*) plus a margin, and released beyond it with a hysteresis.
//! - **Parts** are requested per unique mesh DID. A worker reads the record, decodes the
//!   mesh (`mesh.md`), flattens **every detail level** (`mesh.md` §8.3), resolves every
//!   submesh's surface to a [`MaterialPlan`] (`material.md`) and prepares the GPU streams;
//!   the main thread only uploads, within the frame's [`UploadBudget`]. Parts that
//!   instanced draws use also keep their streams until a [`WorldModel`] per level is built
//!   from them once their textures are on the GPU (`docs/specs/render.md` §5.5).
//! - **Textures** are requested when a part is uploaded, keyed by [`TexRequest`] (base
//!   texture and alpha source), decoded with their stored mip chain (`texture-list.md`
//!   §5.5) and composed on a worker, capped in size, and reference-counted by uploaded
//!   parts and by the other users of the layer (characters, terrain, frills, sky, water).
//! - GPU memory has a budget: unreferenced parts and textures are kept for reuse until
//!   the budget is exceeded or they have been unused for a while, then freed least
//!   recently used first. Part meshes share the renderer's mesh arena with the terrain, so
//!   they never fill it beyond the terrain's share ([`super::terrain_arena_reserve`]) below
//!   its cap; a part that does not fit waits, and the distance cap shrinks until what is
//!   in range fits.
//! - **Drawing** (`mesh.md` §8.4–§8.7): per object the distance *d* to the nearest point of
//!   its box, the 1.5 s fade when *d* crosses *D* ([`DrawFade`]), the detail level from the
//!   smoothed distance and *R_d* = `d1` × *M* ([`detail_level`]). Fully faded-in generated
//!   objects are drawn instanced, one batch per landblock, part and level; everything else
//!   (statics, fading objects, surfaces with emissive, glow or dye) as single mesh draws,
//!   through the lit pipeline when the pass is lit. Landblocks are culled by a box around
//!   the spheres of their uploaded instances ([`instance_bounds`]).

use std::collections::{HashMap, HashSet};
use std::sync::{Arc, Mutex, PoisonError};
use std::time::{Duration, Instant};

use tessera_assets::{
    AssetError, AssetKey, AssetState, AssetStore, Decoder, Handle, JobContext, Outcome, Priority,
    RecordSource, StoreConfig, WorkerPool,
};
use tessera_dat::ArchiveSet;
use tessera_formats::collision::{CELL_GEOMETRY_PREFIX, CellGeometry};
use tessera_formats::{Did, PropertySchema, RgbaImage};
use tessera_material::diagnose::{reason_of_resolve_error, reason_of_texture_error};
use tessera_material::{FallbackReason, LibraryConfig, MaterialLibrary};
use tessera_mesh::{DecodeOptions, GeometryLimits, Mesh, StampedSource};
use tessera_render::glam::{Mat4, Vec3};
use tessera_render::{
    AlphaMode, Dye, DyeMask, Gloss, Instance as GpuInstance, InstanceBatch, LiquidMeshDraw,
    LitSurface, Material, MeshConfig, MeshData, MeshDraw, MeshId, MipChainUpload, MipLevel,
    Opacity, Pass, PreparedMesh, Renderer, SecondLayer, Specular, TextureId, TextureUpload,
    UvTransform, WorldModel, WorldSurface, Wrap,
};
use tessera_world::draw_distance::{DistanceSmoother, DrawFade, aabb_distance, detail_level};
use tessera_world::{DistanceSettings, LANDBLOCK_SIZE, LandblockKey, LodValues};

use super::UploadBudget;
use super::fallback::{FallbackStats, FallbackTally};
use super::figures::figure_tag;
use super::geometry::{
    AlphaFrom, AlphaKind, GlossFrom, MaterialPlan, OWN_TEXTURES, TexRequest, VARIANT_ALPHA_ONLY,
    VARIANT_DYE_MASK, VARIANT_OPAQUE, VARIANT_OWN_ALPHA, compose_texture, downscale_to, dye_mask,
    image_ok, level_mesh_data, plan_material_with,
};
use super::interiors::OUTDOORS;
use super::liquid::{AnimClock, SurfaceAnim};
use super::view::{Anchor, Frustum};

/// Frames an unreferenced GPU resource is kept before it may be freed even under budget.
const IDLE_KEEP: Duration = Duration::from_secs(30);
/// Retry delay after a full queue.
const RETRY_AFTER: Duration = Duration::from_millis(250);
/// Retry delay of a part whose upload found the mesh arena full.
const ARENA_RETRY: Duration = Duration::from_secs(2);
/// How long a full mesh arena keeps the distance cap shrinking.
const ARENA_PRESSURE: Duration = Duration::from_secs(1);
/// Texture levels up to this edge (texels) are kept on the CPU as well
/// ([`ObjectLayer::texture_small`]).
pub const SMALL_LEVEL: u32 = 64;
/// Most detail levels kept per part (`mesh.md` §8.3: the data has at most six).
pub const MAX_DETAIL_LEVELS: usize = 8;
/// An object is activated (its parts requested) within its draw distance plus this margin
/// (metres), so its parts are on the GPU before it fades in.
pub const ACTIVATE_MARGIN: f32 = 60.0;
/// An active object is released beyond its draw distance plus this margin (metres).
pub const RELEASE_MARGIN: f32 = 140.0;
/// The activation pass of a landblock runs again once the camera has moved this far
/// (metres) since its last pass (or when anything else it depends on changed,
/// [`ActivationKey`]).
const ACTIVATION_STEP: f32 = 12.0;

/// What an activation pass depends on besides the camera: *G*, the distance cap in force,
/// the switches and the distance settings. A landblock's pass reruns when any of them
/// changes (beyond 1 m for *G*), as well as after [`ACTIVATION_STEP`] of camera travel.
#[derive(Debug, Clone, Copy, PartialEq)]
struct ActivationKey {
    g: f32,
    cap: f32,
    lod: bool,
    scenery: bool,
    instancing: bool,
    settings: DistanceSettings,
    interior: Option<LandblockKey>,
    all_interiors: bool,
}

impl ActivationKey {
    fn new(frame: &ObjectFrame, cap: f32) -> Self {
        Self {
            g: frame.g,
            cap,
            lod: frame.lod,
            scenery: frame.scenery,
            instancing: frame.instancing,
            settings: frame.settings,
            interior: frame.interior,
            all_interiors: frame.all_interiors,
        }
    }
}

/// Whether a landblock last activated at `eval_at` (camera, key) needs a new pass with the
/// camera at `camera` under `key`.
fn activation_stale(
    eval_at: Option<([f64; 3], ActivationKey)>,
    camera: [f64; 3],
    key: &ActivationKey,
) -> bool {
    eval_at.is_none_or(|(at, k)| {
        let dx = at[0] - camera[0];
        let dy = at[1] - camera[1];
        let dz = at[2] - camera[2];
        (dx * dx + dy * dy + dz * dz).sqrt() > f64::from(ACTIVATION_STEP)
            || (k.g - key.g).abs() > 1.0
            || k.cap != key.cap
            || k.lod != key.lod
            || k.scenery != key.scenery
            || k.instancing != key.instancing
            || k.settings != key.settings
            || k.interior != key.interior
            || k.all_interiors != key.all_interiors
    })
}

/// Errors of the part and texture decoders.
#[derive(Debug, thiserror::Error)]
pub enum ObjectError {
    /// The record is shorter than the directory stamp appended by the source.
    #[error("record too short for its stamp")]
    NoStamp,
    /// The mesh record did not decode or flatten.
    #[error(transparent)]
    Mesh(#[from] tessera_mesh::MeshError),
    /// The mesh has no levels.
    #[error("mesh has no levels")]
    NoLevels,
    /// A cell geometry record (0x01) did not decode, or no property schema was given to
    /// decode it with.
    #[error("cell geometry: {0}")]
    Cell(String),
    /// The renderer refused the streams.
    #[error(transparent)]
    Render(#[from] tessera_render::RenderError),
    /// A texture could not be decoded.
    #[error(transparent)]
    Material(#[from] tessera_material::MaterialError),
    /// Nothing to compose (inconsistent images).
    #[error("texture composition failed")]
    Compose,
    /// The job was cancelled.
    #[error("cancelled")]
    Cancelled,
}

/// One decoded detail level of a part.
#[derive(Debug)]
pub struct LevelAsset {
    mesh: Mutex<Option<PreparedMesh>>,
    /// The level's streams, for a [`WorldModel`] (instanced draws).
    data: Arc<MeshData>,
    gpu_bytes: u64,
    /// One plan per renderer submesh.
    pub plans: Vec<MaterialPlan>,
    /// Per submesh, what the per-frame evaluation needs when the plan animates its texture
    /// matrices or is drawn by its own program ([`SurfaceAnim`]).
    pub anims: Vec<Option<Arc<SurfaceAnim>>>,
}

/// A decoded part, ready for upload: its detail levels, finest first (`mesh.md` §8.3).
#[derive(Debug)]
pub struct PartAsset {
    levels: Vec<LevelAsset>,
    gpu_bytes: u64,
}

/// The detail levels of `mesh` the walker draws: the kind-1 levels in stored order
/// (`mesh.md` §8.3; proxy levels are ignored), at most [`MAX_DETAIL_LEVELS`], else level 0.
#[must_use]
pub fn drawn_levels(mesh: &Mesh) -> Vec<usize> {
    let mut v: Vec<usize> = mesh.detail_levels().take(MAX_DETAIL_LEVELS).collect();
    if v.is_empty() && !mesh.levels.is_empty() {
        v.push(0);
    }
    v
}

/// The property schema a [`PartDecoder`] decodes the embedded property set of a cell
/// geometry record (0x01) with.
pub type CellSchema = Arc<dyn PropertySchema + Send + Sync>;

/// Decodes part meshes (module docs). Holds the material library shared with textures.
///
/// A part DID with prefix 0x01 is an interior cell's geometry (`collision.md` §5): the
/// record is decoded and its embedded render mesh (§5.7, `landblock.md` §4.14) is what the
/// part draws, decoded with the record's directory stamp. A key whose variant is not 0
/// draws the cell with the look of that index ([`super::looks::CellLooks::asset_key`],
/// `landblock.md` §4.14).
pub struct PartDecoder {
    materials: Arc<MaterialLibrary<Arc<ArchiveSet>>>,
    mesh_config: MeshConfig,
    limits: GeometryLimits,
    cell_schema: Option<CellSchema>,
    looks: Option<Arc<super::looks::CellLooks>>,
}

impl PartDecoder {
    /// The plan of `surface`, with `look`'s matching groups layered on it, and what its
    /// per-frame evaluation needs (texture matrices that animate, an own-program drawing).
    fn plan(
        &self,
        surface: Did,
        look: Option<&super::looks::CellLook>,
    ) -> (MaterialPlan, Option<Arc<SurfaceAnim>>) {
        let layering = look.and_then(|l| {
            let key = self.materials.surface_record(surface).ok()?.binding_key;
            l.layering(key, None)
        });
        let resolved = match &layering {
            Some(layering) => self
                .materials
                .resolve_layered(surface, layering)
                .map(Arc::new),
            None => self.materials.resolve_surface(surface),
        };
        match resolved {
            Ok(m) => {
                // Without the record's bindings the matrices stay still (the plan's
                // diagnostic says so).
                let layout = self.materials.texture_matrices(m.shader.did).ok();
                let plan = plan_material_with(&m, layout.as_deref());
                let anim = (plan.animated_uv || plan.own.is_some()).then(|| {
                    Arc::new(SurfaceAnim {
                        material: m,
                        layout: layout.unwrap_or_default(),
                    })
                });
                (plan, anim)
            }
            Err(e) => {
                tracing::debug!(surface = %format_args!("{:#010x}", surface.0), "material unresolved: {e}");
                (
                    MaterialPlan::unresolved(Some(surface), reason_of_resolve_error(&e)),
                    None,
                )
            }
        }
    }
}

impl std::fmt::Debug for PartDecoder {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("PartDecoder").finish_non_exhaustive()
    }
}

impl Decoder for PartDecoder {
    type Output = PartAsset;
    type Error = ObjectError;
    const NAME: &'static str = "walker-part";

    fn decode(
        &self,
        key: AssetKey,
        mut bytes: Vec<u8>,
        ctx: &JobContext<'_>,
    ) -> Result<PartAsset, ObjectError> {
        // `StampedSource` appends the directory stamp as a little-endian u32.
        let n = bytes.len().checked_sub(4).ok_or(ObjectError::NoStamp)?;
        let stamp = u32::from_le_bytes([bytes[n], bytes[n + 1], bytes[n + 2], bytes[n + 3]]);
        bytes.truncate(n);
        let did = Did(key.did);
        let stamp = u16::try_from(stamp).ok();
        // A cell drawn with its appearance list (`landblock.md` §4.14).
        let look = (key.variant != 0)
            .then(|| self.looks.as_ref()?.look(key.variant))
            .flatten();
        let mesh = if did.prefix() == CELL_GEOMETRY_PREFIX {
            let schema = self
                .cell_schema
                .as_deref()
                .ok_or_else(|| ObjectError::Cell("no property schema".into()))?;
            let cell = CellGeometry::parse_expecting(&bytes, did, stamp, schema)
                .map_err(|e| ObjectError::Cell(e.to_string()))?;
            drop(bytes);
            Mesh::decode(
                &cell.render_mesh_record(),
                stamp,
                &DecodeOptions::render_only(),
            )?
        } else {
            Mesh::decode_expecting(&bytes, did, stamp, &DecodeOptions::render_only())?
        };
        let levels = drawn_levels(&mesh);
        if levels.is_empty() {
            return Err(ObjectError::NoLevels);
        }
        let mut out = Vec::with_capacity(levels.len());
        let mut total = 0u64;
        for (k, level) in levels.into_iter().enumerate() {
            if ctx.is_cancelled() {
                return Err(ObjectError::Cancelled);
            }
            let geo = match mesh.level_geometry(level, &self.limits) {
                Ok(g) => g,
                // A coarser level that does not flatten: draw the finer ones only.
                Err(e) if k > 0 => {
                    tracing::debug!(part = %format_args!("{:#010x}", key.did), level, "detail level skipped: {e}");
                    break;
                }
                Err(e) => return Err(e.into()),
            };
            let data = level_mesh_data(&geo, format!("part {:#010x}/{k}", key.did));
            let (plans, anims): (Vec<_>, Vec<_>) = geo
                .parts
                .iter()
                .map(|p| {
                    p.surface.map_or(
                        (
                            MaterialPlan::unresolved(None, FallbackReason::NoSurface),
                            None,
                        ),
                        |s| self.plan(s, look.as_deref()),
                    )
                })
                .unzip();
            drop(geo);
            let prepared = data.prepare(&self.mesh_config)?;
            let bytes = prepared.gpu_bytes();
            total += bytes;
            out.push(LevelAsset {
                mesh: Mutex::new(Some(prepared)),
                data: Arc::new(data),
                gpu_bytes: bytes,
                plans,
                anims,
            });
        }
        Ok(PartAsset {
            levels: out,
            gpu_bytes: total,
        })
    }

    fn size_of(&self, value: &PartAsset) -> usize {
        usize::try_from(value.gpu_bytes)
            .unwrap_or(usize::MAX)
            .saturating_mul(2)
            .max(1)
    }
}

/// A record source that reads nothing: the texture decoder reads through the material
/// library itself (texture lists, highres fallback).
#[derive(Debug, Clone, Copy)]
pub struct NoRecord;

impl RecordSource for NoRecord {
    fn read_record(&self, _did: u32) -> Result<Vec<u8>, AssetError> {
        Ok(Vec::new())
    }
}

/// A decoded texture: its mip levels, largest first (one level for composed and derived
/// images), and the mean colour of its smallest level (used as the terrain colour under
/// frills, `scenery.md` §8.3 rule 7).
#[derive(Debug, Clone, PartialEq)]
pub struct TexImage {
    /// Levels, largest first; every level is consistent ([`image_ok`]).
    pub levels: Vec<RgbaImage>,
    /// Mean RGB of the smallest level, stored values in 0–1.
    pub mean: [f32; 3],
}

impl TexImage {
    /// Bytes of every level.
    #[must_use]
    pub fn bytes(&self) -> usize {
        self.levels.iter().map(|l| l.pixels.len()).sum()
    }
}

/// The mean RGB of an image in 0–1 (white for an inconsistent or empty one).
#[must_use]
pub fn mean_rgb(img: &RgbaImage) -> [f32; 3] {
    if !image_ok(img) {
        return [1.0; 3];
    }
    let mut sum = [0u64; 3];
    let mut n = 0u64;
    for px in img.pixels.as_chunks::<4>().0 {
        for c in 0..3 {
            sum[c] += u64::from(px[c]);
        }
        n += 1;
    }
    if n == 0 {
        return [1.0; 3];
    }
    sum.map(|s| (s as f64 / n as f64 / 255.0) as f32)
}

/// Decodes and composes textures for [`TexRequest`]s (`AssetKey { did, variant }`).
pub struct TexDecoder {
    materials: Arc<MaterialLibrary<Arc<ArchiveSet>>>,
    cap: u32,
}

impl std::fmt::Debug for TexDecoder {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("TexDecoder")
            .field("cap", &self.cap)
            .finish_non_exhaustive()
    }
}

impl TexDecoder {
    /// The stored mip chain of texture list `list` within the cap (`texture-list.md`
    /// §5.5): the levels of the upload plan, each decoded by the library; a single level
    /// when the list has no usable chain.
    fn chain(&self, list: u32) -> Result<Vec<RgbaImage>, ObjectError> {
        let did = Did(list);
        if let Ok(chain) = self.materials.texture_chain(did, Some(self.cap))
            && let Some(plan) = chain.plan
        {
            let first = plan.first as usize;
            let count = (plan.uploadable as usize).max(1);
            let mut out: Vec<RgbaImage> = Vec::with_capacity(count);
            for lv in chain.levels.iter().skip(first).take(count) {
                let (w, h) = (lv.level.width, lv.level.height);
                let Ok(img) = self.materials.texture(did, Some(w.max(h).max(1))) else {
                    break;
                };
                if img.image.width != w || img.image.height != h || !image_ok(&img.image) {
                    break;
                }
                out.push((*img.image).clone());
            }
            if !out.is_empty() {
                return Ok(out);
            }
        }
        let img = self.materials.texture(did, Some(self.cap))?;
        Ok(vec![downscale_to((*img.image).clone(), self.cap)])
    }
}

impl Decoder for TexDecoder {
    type Output = TexImage;
    type Error = ObjectError;
    const NAME: &'static str = "walker-texture";

    fn decode(
        &self,
        key: AssetKey,
        _bytes: Vec<u8>,
        _ctx: &JobContext<'_>,
    ) -> Result<TexImage, ObjectError> {
        let load = |did: u32| self.materials.texture(Did(did), Some(self.cap));
        let levels = match key.variant {
            VARIANT_ALPHA_ONLY => {
                let a = load(key.did)?;
                vec![
                    compose_texture(None, AlphaFrom::Image(&a.image))
                        .ok_or(ObjectError::Compose)?,
                ]
            }
            VARIANT_DYE_MASK => {
                let a = load(key.did)?;
                vec![dye_mask(&a.image).ok_or(ObjectError::Compose)?]
            }
            VARIANT_OPAQUE | VARIANT_OWN_ALPHA => {
                let alpha = if key.variant == VARIANT_OPAQUE {
                    AlphaFrom::Opaque
                } else {
                    AlphaFrom::Own
                };
                self.chain(key.did)?
                    .iter()
                    .map(|l| compose_texture(Some(l), alpha))
                    .collect::<Option<Vec<_>>>()
                    .ok_or(ObjectError::Compose)?
            }
            other => {
                let chain = self.chain(key.did)?;
                let a = load(other)?;
                chain
                    .iter()
                    .map(|b| compose_texture(Some(b), AlphaFrom::Image(&a.image)))
                    .collect::<Option<Vec<_>>>()
                    .ok_or(ObjectError::Compose)?
            }
        };
        let levels: Vec<RgbaImage> = levels
            .into_iter()
            .map(|l| downscale_to(l, self.cap))
            .filter(image_ok)
            .collect();
        let mean = levels.last().map_or([1.0; 3], mean_rgb);
        if levels.is_empty() {
            return Err(ObjectError::Compose);
        }
        Ok(TexImage { levels, mean })
    }

    fn size_of(&self, value: &TexImage) -> usize {
        value.bytes().max(1)
    }
}

/// Settings of the object layer.
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct ObjectConfig {
    /// Cap on every object's draw distance (world units). The class distances of
    /// `walker.md` §11.4 apply below it; the cap shrinks while the GPU budget is exceeded.
    pub distance: f64,
    /// Objects nearer than this load their parts at `Visible` priority, the others at
    /// `Prefetch` (parts never outrank the near ring's landblocks, which load at
    /// `Immediate`).
    pub near: f64,
    /// Smallest projected radius drawn, in pixels.
    pub min_pixels: f32,
    /// Most object draws per frame (single mesh draws; instanced batches count once).
    pub max_draws: usize,
    /// GPU bytes for part meshes and textures together.
    pub gpu_budget: u64,
    /// Largest texture edge.
    pub texture_cap: u32,
}

impl Default for ObjectConfig {
    fn default() -> Self {
        Self {
            distance: 2_080.0,
            near: 200.0,
            min_pixels: 1.5,
            max_draws: 20_000,
            gpu_budget: 1024 << 20,
            texture_cap: 512,
        }
    }
}

/// How the objects of one frame are chosen and drawn (`walker.md` §11.4, §14).
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct ObjectFrame {
    /// The global limit *G* (`walker.md` §11.3).
    pub g: f32,
    /// The distance preferences (multipliers).
    pub settings: DistanceSettings,
    /// Per-class distances, fades and detail levels on; off draws every object to the
    /// distance cap at its finest level, without fades.
    pub lod: bool,
    /// Draw faded-in generated objects instanced.
    pub instancing: bool,
    /// Draw generated scenery at all.
    pub scenery: bool,
    /// Draw through the lit pipeline (stored-value colours, lit inputs).
    pub lit: bool,
    /// Objects cast sun shadows.
    pub shadows: bool,
    /// Frame time, seconds.
    pub dt: f32,
    /// Running frame rate (distance smoothing, `mesh.md` §8.6).
    pub fps: f32,
    /// Camera travel since the previous frame, metres (fade reset beyond 25 m).
    pub camera_moved: f32,
    /// The landblock owning the camera's interior cell (`landblock.md` §4.12), `None`
    /// outdoors: only that landblock's interior objects are loaded and drawn.
    pub interior: Option<LandblockKey>,
    /// Load and draw every interior object whatever the camera's cell (`--interiors`,
    /// a debugging view).
    pub all_interiors: bool,
    /// Seconds on the material clock (`material.md` §5.3.1): animated texture matrices
    /// and own-program parameters are evaluated at it every frame (`water.md` §8.7).
    pub time: f64,
}

/// Whether an object attached to `cell` of block `key` may be loaded in `frame`: outdoor
/// objects always; interior ones (cells and interior entities) only while the camera is
/// in a cell of the same landblock, or with [`ObjectFrame::all_interiors`].
#[must_use]
pub fn interior_eligible(key: LandblockKey, cell: u16, frame: &ObjectFrame) -> bool {
    cell == OUTDOORS || frame.all_interiors || frame.interior == Some(key)
}

impl Default for ObjectFrame {
    fn default() -> Self {
        Self {
            g: 2_080.0,
            settings: DistanceSettings::default(),
            lod: true,
            instancing: true,
            scenery: true,
            lit: true,
            shadows: true,
            dt: 1.0 / 60.0,
            fps: 60.0,
            camera_moved: 0.0,
            interior: None,
            all_interiors: false,
            time: 0.0,
        }
    }
}

enum PartState {
    Loading(Handle<PartAsset>),
    Retry(Instant),
    Waiting(Arc<PartAsset>),
    Ready(GpuPart),
    Failed,
}

/// One uploaded detail level.
struct GpuLevel {
    mesh: MeshId,
    plans: Vec<MaterialPlan>,
    /// Per plan, what its per-frame evaluation needs ([`LevelAsset::anims`]).
    anims: Vec<Option<Arc<SurfaceAnim>>>,
    /// The instanced model, once built.
    model: Option<WorldModel>,
    /// Level 0 of a frill part with more than one draw entry: the model of its first draw
    /// entry only (`scenery.md` §8.5 step 3), once built.
    frill_model: Option<WorldModel>,
    /// That model could not be built (the frill draws the whole level instead).
    frill_failed: bool,
    /// The streams kept to build the model (dropped once it is built).
    data: Option<Arc<MeshData>>,
    /// The model could not be built (its draws stay single mesh draws).
    model_failed: bool,
    /// Every surface can be drawn instanced ([`MaterialPlan::instanceable`]).
    instanceable: bool,
    bytes: u64,
}

struct GpuPart {
    levels: Vec<GpuLevel>,
    /// Bounding sphere of level 0 (object space).
    center: Vec3,
    radius: f32,
    bytes: u64,
}

impl GpuLevel {
    /// More than one draw entry, and no first-entry frill model yet (nor a failed one).
    fn needs_frill_model(&self) -> bool {
        self.plans.len() > 1 && self.frill_model.is_none() && !self.frill_failed
    }
}

impl GpuPart {
    fn textures(&self) -> impl Iterator<Item = TexRequest> + '_ {
        self.levels
            .iter()
            .flat_map(|l| l.plans.iter().flat_map(MaterialPlan::textures))
    }
}

struct PartEntry {
    refs: u32,
    state: PartState,
    priority: Priority,
    unused_since: Option<Instant>,
    /// Instanced draws or models are wanted: keep the streams to build models.
    want_model: bool,
    /// Used as a frill ([`ObjectLayer::frill_model`]).
    want_frill: bool,
    /// A second decode for models of a part uploaded without its streams.
    model_job: Option<Handle<PartAsset>>,
}

enum TexState {
    Loading(Handle<TexImage>),
    Retry(Instant),
    Waiting(Arc<TexImage>),
    Ready(TextureId, u64, [f32; 3], Option<Arc<[RgbaImage]>>),
    /// Failed for good, with the reason for the fallback diagnostic.
    Failed(FallbackReason),
}

/// The fallback reason of a texture request that failed (`tessera_material::diagnose`).
#[must_use]
pub fn texture_failure_reason(err: &AssetError) -> FallbackReason {
    match err {
        AssetError::Decode { source, .. } => match source.downcast_ref::<ObjectError>() {
            Some(ObjectError::Material(e)) => reason_of_texture_error(e),
            Some(ObjectError::Compose) => FallbackReason::TextureComposeFailed,
            _ => FallbackReason::TextureUndecodable,
        },
        _ => FallbackReason::TextureReadFailed,
    }
}

struct TexEntry {
    refs: u32,
    state: TexState,
    unused_since: Option<Instant>,
}

/// One static part placed in its landblock's frame (an [`ObjectInfo`] has one per part
/// of its setup).
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct Instance {
    /// The 0x06 part mesh.
    pub part: u32,
    /// Part (object) space → block-local frame: translation · rotation · scale
    /// (`entity-descriptor.md` §5).
    pub model: Mat4,
    /// Largest axis scale (for bounding spheres).
    pub scale: f32,
    /// The bundle's figure index when the part belongs to a placed figure (`walker.md`
    /// §10.2): drawn as a static instance unless the character layer draws that figure.
    pub figure: Option<u32>,
    /// Index of the instance's object in [`ObjectInfo`]s of its block.
    pub object: u32,
}

/// One drawable object of a landblock (an entity or a generated object).
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct ObjectInfo {
    /// Block-local position of its origin.
    pub position: Vec3,
    /// Its LOD-class values (`walker.md` §11.4.3; the component's initial values without a
    /// class).
    pub lod: LodValues,
    /// A generated scenery object (`scenery.md` §7.6).
    pub generated: bool,
    /// Its part instances: `first .. first + count` of the block's instances.
    pub first: u32,
    /// Number of part instances.
    pub count: u32,
    /// The interior cell it is drawn through (`landblock.md` §4.11): [`OUTDOORS`] (0) for
    /// the outdoor visibility cell, a cell number of the block's landblock info, or
    /// [`UNATTACHED`](super::interiors::UNATTACHED) for an interior entity no cell
    /// contains (`walker.md` §14.12 (h)).
    pub cell: u16,
}

/// Per-object state of the layer.
#[derive(Debug, Clone, Copy, Default)]
struct ObjState {
    /// Its parts are referenced.
    active: bool,
    fade: DrawFade,
    smooth: DistanceSmoother,
    /// Block-local bounding sphere over its uploaded parts.
    sphere: Option<(Vec3, f32)>,
    /// The draw frame ([`ObjectLayer::draw`] count, from 1) in which its fade was last
    /// updated; 0: never.
    faded_at: u64,
}

/// Whether an object's fade restarts in draw frame `frame`: its fade was not updated in the
/// frame before. The fade of every object of the walker's scene advances in every frame,
/// drawn or not (its block out of view, after the draw cap; `walker.md` §14.5 row W-O3: the
/// client's per-object step runs for every object of the scene before the culls), so this
/// holds only in the first frame of an object's state (it became active) and when it
/// re-enters the scene (an outdoor object after the camera leaves a cell, a generated object
/// after `scenery` is switched on), the walker's counterpart of the client's first frame of
/// a per-view state. A camera jump of more than 25 m restarts a fade too ([`DrawFade`]).
#[must_use]
pub fn fade_resets(faded_at: u64, frame: u64) -> bool {
    faded_at == 0 || faded_at.checked_add(1) != Some(frame)
}

struct BlockObjects {
    instances: Arc<[Instance]>,
    objects: Arc<[ObjectInfo]>,
    state: Vec<ObjState>,
    /// Block-local box around the bounding spheres of the instances whose part is on the
    /// GPU (`None`: none is).
    bounds: Option<(Vec3, Vec3)>,
    /// `bounds` needs recomputing (a part of the block was uploaded, or objects activated).
    dirty: bool,
    /// Camera position (region frame) and what else the last activation pass depended on.
    eval_at: Option<([f64; 3], ActivationKey)>,
    /// Active objects.
    active: usize,
    /// Block-local rectangle around the outdoor objects' origins (x, y).
    rect: ([f32; 2], [f32; 2]),
    /// The largest `d0` of the block's outdoor objects.
    max_d0: f32,
    /// The distinct parts of the block's instances, sorted.
    parts: Vec<u32>,
    /// Part instances of the active objects.
    active_instances: usize,
    /// The cell each placed figure of the block is drawn through: the [`ObjectInfo::cell`]
    /// of the object of its first instance, by figure index ([`Instance::figure`]).
    figure_cells: HashMap<u32, u16>,
}

impl BlockObjects {
    /// Updates the smoothed distance and the draw-distance fade of every object of the
    /// block from index `from` on that is in the scene (active, not a generated object with
    /// `scenery` off, drawn through a cell the camera sees) but not processed by the draw
    /// loop this frame: because the block's box is out of view (`from` 0), or because the
    /// draw cap stopped the frame (the objects after the one that reached the cap, and
    /// every object of the blocks after it). Both follow every object of the scene each
    /// frame, drawn or not, as the client's per-object step does before its culls
    /// (`mesh.md` §8.6–§8.7; `walker.md` §14.5 row W-O3), so an object culled while the
    /// camera travels comes back with a current distance and fade. `local_cam` is the eye in
    /// the block's frame, `draw_frame` the frame number ([`fade_resets`]), `cap` the
    /// distance cap.
    fn follow_distances(
        &mut self,
        key: LandblockKey,
        local_cam: Vec3,
        frame: &ObjectFrame,
        drawn: &dyn Fn(LandblockKey, u16) -> bool,
        from: usize,
        (draw_frame, cap): (u64, f32),
    ) {
        for (obj, st) in self.objects.iter().zip(self.state.iter_mut()).skip(from) {
            if !st.active || (obj.generated && !frame.scenery) || !drawn(key, obj.cell) {
                continue;
            }
            if let Some((c, r)) = st.sphere {
                let d = aabb_distance(
                    local_cam.to_array(),
                    (c - Vec3::splat(r)).to_array(),
                    (c + Vec3::splat(r)).to_array(),
                );
                let reset = fade_resets(st.faded_at, draw_frame);
                st.faded_at = draw_frame;
                st.fade.update(
                    d,
                    object_draw_distance(&obj.lod, frame, cap),
                    frame.dt,
                    frame.camera_moved,
                    reset,
                    frame.lod,
                );
                st.smooth.update(d, frame.fps, false);
            }
        }
    }

    /// A block's objects, none active.
    fn new(instances: Arc<[Instance]>, objects: Arc<[ObjectInfo]>) -> Self {
        let state = vec![ObjState::default(); objects.len()];
        let mut lo = [f32::INFINITY; 2];
        let mut hi = [f32::NEG_INFINITY; 2];
        let mut max_d0 = 0.0f32;
        // Interior objects (cells may lie landblocks away from their owner, and cell
        // meshes reach to *G*) are loaded only from inside, where the block is never
        // skipped ([`activate_blocks`]), so they do not widen the outdoor skip test.
        for o in objects.iter().filter(|o| o.cell == OUTDOORS) {
            if o.position.is_finite() {
                lo = [lo[0].min(o.position.x), lo[1].min(o.position.y)];
                hi = [hi[0].max(o.position.x), hi[1].max(o.position.y)];
            }
            if o.lod.d0().is_finite() {
                max_d0 = max_d0.max(o.lod.d0());
            }
        }
        if lo[0] > hi[0] {
            (lo, hi) = ([0.0; 2], [0.0; 2]);
        }
        let mut parts: Vec<u32> = instances.iter().map(|i| i.part).collect();
        parts.sort_unstable();
        parts.dedup();
        let mut figure_cells: HashMap<u32, u16> = HashMap::new();
        for inst in instances.iter() {
            if let Some(f) = inst.figure
                && let Some(o) = objects.get(inst.object as usize)
            {
                figure_cells.entry(f).or_insert(o.cell);
            }
        }
        BlockObjects {
            instances,
            objects,
            state,
            bounds: None,
            dirty: true,
            eval_at: None,
            active: 0,
            rect: (lo, hi),
            max_d0,
            parts,
            active_instances: 0,
            figure_cells,
        }
    }
}

/// The asset key part id `part` is requested by: a cell look's ([`super::looks`]), else
/// the DID itself.
fn part_key(looks: Option<&super::looks::CellLooks>, part: u32) -> AssetKey {
    looks.map_or_else(|| AssetKey::new(part, 0), |l| l.asset_key(part))
}

/// Slack (metres) for an object's extent around its origin when a whole landblock is
/// skipped by the activation pass (larger objects have larger `d0` anyway).
const BLOCK_SKIP_SLACK: f32 = 100.0;

/// The block-local box around the bounding spheres of `instances` whose part `sphere`
/// knows (object-space centre and radius), exactly the spheres the per-instance cull
/// tests, so the block box is visible whenever one of them is. `None` when no instance
/// has a sphere. Non-finite spheres are skipped.
#[must_use]
pub fn instance_bounds(
    instances: &[Instance],
    sphere: impl Fn(u32) -> Option<(Vec3, f32)>,
) -> Option<(Vec3, Vec3)> {
    let mut out: Option<(Vec3, Vec3)> = None;
    for inst in instances {
        let Some((center, radius)) = sphere(inst.part) else {
            continue;
        };
        let c = inst.model.transform_point3(center);
        let r = radius * inst.scale;
        if !c.is_finite() || !r.is_finite() {
            continue;
        }
        let (lo, hi) = (c - Vec3::splat(r.abs()), c + Vec3::splat(r.abs()));
        out = Some(match out {
            Some((a, b)) => (a.min(lo), b.max(hi)),
            None => (lo, hi),
        });
    }
    out
}

/// The sphere around a set of spheres (centre, radius): the centre of their box and the
/// largest distance from it to a sphere's surface. `None` for none.
#[must_use]
pub fn enclosing_sphere(spheres: impl Iterator<Item = (Vec3, f32)>) -> Option<(Vec3, f32)> {
    let list: Vec<(Vec3, f32)> = spheres
        .filter(|(c, r)| c.is_finite() && r.is_finite())
        .collect();
    let (mut lo, mut hi) = (Vec3::splat(f32::INFINITY), Vec3::splat(f32::NEG_INFINITY));
    for (c, r) in &list {
        lo = lo.min(*c - Vec3::splat(r.abs()));
        hi = hi.max(*c + Vec3::splat(r.abs()));
    }
    if list.is_empty() {
        return None;
    }
    let center = (lo + hi) * 0.5;
    let radius = list
        .iter()
        .map(|(c, r)| (*c - center).length() + r.abs())
        .fold(0.0, f32::max);
    Some((center, radius))
}

/// The draw distance of an object (`walker.md` §11.4 step 1) under the frame's settings,
/// capped at `cap`; the cap alone when distances are off.
#[must_use]
pub fn object_draw_distance(lod: &LodValues, frame: &ObjectFrame, cap: f32) -> f32 {
    if frame.lod {
        lod.draw_distance(frame.g, &frame.settings).min(cap)
    } else {
        cap
    }
}

/// The alpha mode of a draw that fades with opacity `fade` < 1: opaque and alpha-tested
/// surfaces blend by their output alpha with depth write (`docs/specs/render.md` §4.3);
/// blended ones keep their mode.
#[must_use]
pub fn faded_alpha(mode: AlphaMode) -> AlphaMode {
    match mode {
        AlphaMode::Opaque => AlphaMode::BLEND_DEPTH_WRITE,
        AlphaMode::Mask { cutoff } => AlphaMode::TestBlend { cutoff },
        other => other,
    }
}

/// Linear-light unit value to its 8-bit sRGB encoding in 0–1 (clamped; non-finite → 0):
/// the stored value of a colour the M2 plans keep linear.
#[must_use]
pub fn linear_to_stored(c: f32) -> f32 {
    let c = if c.is_finite() {
        c.clamp(0.0, 1.0)
    } else {
        0.0
    };
    if c <= 0.003_130_8 {
        c * 12.92
    } else {
        1.055 * c.powf(1.0 / 2.4) - 0.055
    }
}

/// An object casts into the sun's map only while its opacity is above this (`mesh.md`
/// §8.8; the walker's opacity is the fade's, without the render component's factors).
pub const SHADOW_MIN_OPACITY: f32 = 0.8;

/// The material and lit inputs of one submesh drawn with `plan` (`lighting.md` §4.1,
/// `docs/specs/render.md` §4.3), or `None` while its base texture or second colour layer
/// is still loading. `lit` selects the lit pipeline's inputs (stored-value colours); `fade`
/// the draw-distance opacity (`mesh.md` §8.7), which turns the surface translucent below 1;
/// `shadow` whether it casts the sun's shadow. The lit inputs carry the plan's texture
/// matrices at clock 0 (`material.md` §8.4; the caller replaces them every frame when they
/// animate) and its second colour layer (`material.md` §10.3).
#[must_use]
pub fn surface_draw(
    plan: &MaterialPlan,
    texture: &impl Fn(TexRequest) -> TexLookup,
    lit: bool,
    fade: f32,
    shadow: bool,
) -> Option<(Material, LitSurface)> {
    let base = match plan.texture {
        None => None,
        Some(t) => match texture(t) {
            TexLookup::Ready(id) => Some(id),
            // Not there yet: skip rather than draw a solid placeholder (alpha-tested
            // foliage would show as cards).
            TexLookup::Failed if plan.alpha == AlphaKind::Opaque => None,
            _ => return None,
        },
    };
    let ready = |t: Option<TexRequest>| match t.map(texture) {
        Some(TexLookup::Ready(id)) => Some(id),
        _ => None,
    };
    // §10.3: the second colour layer; wait for it like the base texture, draw without it
    // when it failed.
    let layer2 = match plan.layer2 {
        None => None,
        Some((t, add)) => match texture(t) {
            TexLookup::Ready(id) => Some(SecondLayer { texture: id, add }),
            TexLookup::Failed => None,
            TexLookup::Pending if lit => return None,
            TexLookup::Pending => None,
        },
    };
    let fade = if fade.is_finite() {
        fade.clamp(0.0, 1.0)
    } else {
        1.0
    };
    if !lit {
        let dye = plan.dye.and_then(|(mask, color)| {
            ready(Some(mask)).map(|id| Dye {
                color,
                mask: DyeMask::Texture(id),
            })
        });
        let mut color = plan.base_color;
        color[3] *= fade;
        let alpha = match plan.alpha {
            AlphaKind::Opaque if fade < 1.0 => AlphaMode::Blend,
            AlphaKind::Opaque => AlphaMode::Opaque,
            AlphaKind::Mask(c) => AlphaMode::Mask { cutoff: c },
            AlphaKind::Blend => AlphaMode::Blend,
        };
        let m = Material {
            base_color: color,
            base_texture: base,
            dye,
            alpha,
            two_sided: plan.two_sided,
            unlit: plan.unlit,
            ..Material::color(color)
        };
        return Some((m, LitSurface::default()));
    }
    let l = &plan.lit;
    let dye = plan.dye.and_then(|(mask, color)| {
        ready(Some(mask)).map(|id| Dye {
            color: color.map(linear_to_stored),
            mask: DyeMask::Texture(id),
        })
    });
    let mut color = l.color;
    color[3] *= fade;
    let normal = ready(l.normal);
    let alpha = if fade < 1.0 {
        faded_alpha(l.alpha)
    } else {
        l.alpha
    };
    let m = Material {
        base_color: color,
        base_texture: base,
        alpha,
        two_sided: plan.two_sided,
        normal_texture: normal,
        dye,
        specular: l.specular.map(|(c, p)| Specular {
            color: c,
            shininess: p,
            texture: None,
        }),
        unlit: plan.unlit,
        ..Material::color(color)
    };
    let gloss = match l.gloss {
        GlossFrom::One => Gloss::One,
        GlossFrom::BaseAlpha if base.is_some() => Gloss::BaseAlpha,
        GlossFrom::NormalAlpha if normal.is_some() => Gloss::NormalAlpha,
        _ => Gloss::One,
    };
    let surface = LitSurface {
        glow: l.glow,
        emissive: ready(l.emissive),
        glow_mask: ready(l.glow_mask),
        gloss,
        opacity: if l.opacity_from_base && base.is_some() {
            Opacity::BaseAlpha
        } else {
            Opacity::One
        },
        vertex_colors: false,
        fog: true,
        cast_shadow: shadow && fade > SHADOW_MIN_OPACITY,
        lights: tessera_render::LightSelection::Auto,
        uv: plan.uv,
        second_layer: layer2,
        emissive_uv1: plan.emissive_uv1,
    };
    Some((m, surface))
}

/// The instanced surface of a plan, once its textures have settled.
#[derive(Debug, Clone, Copy, PartialEq)]
enum ModelSurface {
    /// The surface.
    Ready(WorldSurface),
    /// A texture is still loading.
    Pending,
    /// The plan cannot be drawn instanced (a cut-out whose texture failed would draw as a
    /// card; single mesh draws leave it out).
    Unusable,
}

/// The instanced surface of a plan (`docs/specs/render.md` §5.5), with its textures
/// looked up.
fn world_surface(plan: &MaterialPlan, texture: &impl Fn(TexRequest) -> TexLookup) -> ModelSurface {
    let look = |t: Option<TexRequest>| -> Option<Option<TextureId>> {
        match t.map(texture) {
            None | Some(TexLookup::Failed) => Some(None),
            Some(TexLookup::Ready(id)) => Some(Some(id)),
            Some(TexLookup::Pending) => None,
        }
    };
    let (Some(base), Some(normal), Some(layer2)) = (
        look(plan.texture),
        look(plan.lit.normal),
        look(plan.layer2.map(|l| l.0)),
    ) else {
        return ModelSurface::Pending;
    };
    if plan.texture.is_some() && base.is_none() && plan.alpha != AlphaKind::Opaque {
        return ModelSurface::Unusable;
    }
    let l = &plan.lit;
    let alpha = if l.opacity_from_base && base.is_some() {
        l.alpha
    } else {
        match l.alpha {
            AlphaMode::Mask { .. } => AlphaMode::Opaque,
            other => other,
        }
    };
    ModelSurface::Ready(WorldSurface {
        base_color: l.color,
        base_texture: base,
        normal_texture: normal,
        alpha,
        two_sided: plan.two_sided,
        specular: l.specular.map(|(c, p)| Specular {
            color: c,
            shininess: p,
            texture: None,
        }),
        gloss_from_base_alpha: l.gloss == GlossFrom::BaseAlpha && base.is_some(),
        gloss_from_normal_alpha: l.gloss == GlossFrom::NormalAlpha && normal.is_some(),
        vertex_colors: false,
        unlit: plan.unlit,
        second_layer: layer2
            .zip(plan.layer2)
            .map(|(texture, (_, add))| SecondLayer { texture, add }),
        uv: plan.uv,
    })
}

/// This frame's texture matrices of every surface of `level` (`material.md` §8.4, §5.3.1),
/// for an instanced batch ([`InstanceBatch::uv`]); `None` when none of them animates.
fn level_uv(level: &GpuLevel, clock: &mut AnimClock) -> Option<Arc<[[UvTransform; 2]]>> {
    if !level.plans.iter().any(|p| p.animated_uv) {
        return None;
    }
    let list: Vec<[UvTransform; 2]> = level
        .plans
        .iter()
        .zip(level.anims.iter().chain(std::iter::repeat(&None)))
        .map(|(p, a)| match a {
            Some(a) if p.animated_uv => clock.uv(a),
            _ => p.uv,
        })
        .collect();
    Some(list.into())
}

/// Counters for the HUD and frame log.
#[derive(Debug, Clone, Copy, Default, PartialEq)]
pub struct ObjectStats {
    /// Landblocks with objects tracked.
    pub blocks: usize,
    /// Landblocks with active objects.
    pub active_blocks: usize,
    /// Objects tracked.
    pub objects: usize,
    /// Objects whose parts are referenced (within their draw distance plus a margin).
    pub active_objects: usize,
    /// Part instances of the active objects.
    pub instances: usize,
    /// Unique parts referenced.
    pub parts: usize,
    /// Parts on the GPU.
    pub parts_ready: usize,
    /// Parts loading or waiting for upload.
    pub parts_pending: usize,
    /// Parts that failed.
    pub parts_failed: usize,
    /// Instanced models on the GPU.
    pub models: usize,
    /// Textures on the GPU.
    pub textures_ready: usize,
    /// Textures loading or waiting for upload.
    pub textures_pending: usize,
    /// GPU bytes of part meshes and models.
    pub mesh_bytes: u64,
    /// GPU bytes of textures.
    pub texture_bytes: u64,
    /// Single mesh draws recorded last frame.
    pub draws: usize,
    /// Objects drawn last frame.
    pub objects_drawn: usize,
    /// Objects drawn while fading in or out last frame.
    pub fading: usize,
    /// Part instances drawn instanced last frame.
    pub instanced: usize,
    /// Instanced batches recorded last frame.
    pub batches: usize,
    /// Objects culled last frame (frustum, size, distance).
    pub culled: usize,
    /// Whether the draw cap cut the last frame short.
    pub capped: bool,
    /// Distance cap in force (shrunk below the configured one while the GPU budget is
    /// exceeded by what is in range).
    pub distance: f64,
}

/// Where a texture of the object layer stands ([`ObjectLayer::texture`]).
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum TexLookup {
    /// On the GPU.
    Ready(TextureId),
    /// Failed for good.
    Failed,
    /// Loading, waiting for upload, or not referenced.
    Pending,
}

/// The object layer (module docs).
pub struct ObjectLayer {
    config: ObjectConfig,
    materials: Arc<MaterialLibrary<Arc<ArchiveSet>>>,
    parts: AssetStore<StampedSource, PartDecoder>,
    /// The looks of interior cells: part ids of cells drawn with a look are requested
    /// through [`ObjectLayer::part_key`].
    looks: Option<Arc<super::looks::CellLooks>>,
    textures: AssetStore<NoRecord, TexDecoder>,
    blocks: HashMap<LandblockKey, BlockObjects>,
    gpu_parts: HashMap<u32, PartEntry>,
    gpu_tex: HashMap<TexRequest, TexEntry>,
    mesh_bytes: u64,
    texture_bytes: u64,
    stats: ObjectStats,
    projection_scale: f32,
    /// Distance cap in force (see [`ObjectStats::distance`]).
    distance: f64,
    /// When the cap was last adjusted.
    adjusted: Option<Instant>,
    order: Vec<(f64, LandblockKey)>,
    /// Arena bytes part meshes may fill together (the arena cap less the terrain's share).
    arena_limit: u64,
    /// When a part last had to wait for arena space.
    arena_full_at: Option<Instant>,
    /// Instanced batches being collected this frame: (landblock, part, level, casts
    /// shadows) → instances.
    batches: HashMap<(LandblockKey, u32, u8, bool), Vec<GpuInstance>>,
    /// The frame settings of the last update.
    frame: ObjectFrame,
    /// Draw frames recorded so far ([`fade_resets`]).
    draw_frame: u64,
    /// The fallback diagnostic of the frame being drawn ([`super::fallback`]).
    fallback: FallbackTally,
    /// The fallback diagnostic of the last frame drawn.
    fallback_stats: FallbackStats,
    /// Per-frame evaluation of animated texture matrices and own-program materials
    /// (`water.md` §8.7).
    anim: AnimClock,
}

/// Whether instance `inst` of block `key` is a placed figure the character layer draws
/// (its tag is in `characters`), so the object layer leaves it out.
#[must_use]
pub fn drawn_as_character(key: LandblockKey, inst: &Instance, characters: &HashSet<u64>) -> bool {
    inst.figure
        .is_some_and(|i| characters.contains(&figure_tag(key, i)))
}

impl std::fmt::Debug for ObjectLayer {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("ObjectLayer")
            .field("stats", &self.stats)
            .finish_non_exhaustive()
    }
}

impl ObjectLayer {
    /// A layer whose decode jobs run on `pool`; part meshes fill at most `arena_limit`
    /// bytes of the renderer's mesh arena.
    #[must_use]
    pub fn new(
        set: Arc<ArchiveSet>,
        pool: &WorkerPool,
        config: ObjectConfig,
        mesh_config: MeshConfig,
        arena_limit: u64,
        cell_schema: Option<CellSchema>,
        looks: Option<Arc<super::looks::CellLooks>>,
    ) -> Self {
        let materials = Arc::new(MaterialLibrary::with_config(
            Arc::clone(&set),
            LibraryConfig {
                max_texture_dimension: 4096,
                ..LibraryConfig::default()
            },
        ));
        let library = Arc::clone(&materials);
        let parts = AssetStore::new(
            pool,
            StampedSource::new(Arc::clone(&set)),
            PartDecoder {
                materials: Arc::clone(&materials),
                mesh_config,
                limits: GeometryLimits::default(),
                cell_schema,
                looks: looks.clone(),
            },
            StoreConfig {
                // The prepared streams are moved out on upload: never serve a product
                // twice from the cache.
                cache_budget: 0,
                max_completions: 16_384,
                max_completion_bytes: 512 << 20,
                ..StoreConfig::default()
            },
        );
        let textures = AssetStore::new(
            pool,
            NoRecord,
            TexDecoder {
                materials,
                cap: config.texture_cap.max(1),
            },
            StoreConfig {
                cache_budget: 64 << 20,
                max_completions: 16_384,
                max_completion_bytes: 512 << 20,
                ..StoreConfig::default()
            },
        );
        Self {
            config,
            materials: library,
            looks,
            parts,
            textures,
            blocks: HashMap::new(),
            gpu_parts: HashMap::new(),
            gpu_tex: HashMap::new(),
            mesh_bytes: 0,
            texture_bytes: 0,
            stats: ObjectStats::default(),
            projection_scale: 900.0,
            distance: config.distance,
            adjusted: None,
            order: Vec::new(),
            arena_limit,
            arena_full_at: None,
            batches: HashMap::new(),
            frame: ObjectFrame::default(),
            draw_frame: 0,
            fallback: FallbackTally::default(),
            fallback_stats: FallbackStats::default(),
            anim: AnimClock::default(),
        }
    }

    /// The material library the layer decodes with (shared with the character figures).
    #[must_use]
    pub fn materials(&self) -> Arc<MaterialLibrary<Arc<ArchiveSet>>> {
        Arc::clone(&self.materials)
    }

    /// Adds a reference to each texture of `reqs` for a user outside the layer (character
    /// figures, terrain layers, water, frills, sky): referenced textures are loaded,
    /// uploaded within the frame budget and kept until released
    /// ([`ObjectLayer::release_textures`]).
    pub fn acquire_textures(&mut self, reqs: impl IntoIterator<Item = TexRequest>) {
        let now = Instant::now();
        for t in reqs {
            let te = self.gpu_tex.entry(t).or_insert_with(|| TexEntry {
                refs: 0,
                state: TexState::Retry(now - RETRY_AFTER),
                unused_since: None,
            });
            te.refs += 1;
            te.unused_since = None;
        }
    }

    /// Drops one reference to each texture of `reqs` (taken with
    /// [`ObjectLayer::acquire_textures`]); unreferenced textures are evicted like the
    /// layer's own.
    pub fn release_textures(&mut self, reqs: impl IntoIterator<Item = TexRequest>) {
        let now = Instant::now();
        for t in reqs {
            if let Some(te) = self.gpu_tex.get_mut(&t) {
                te.refs = te.refs.saturating_sub(1);
                if te.refs == 0 {
                    te.unused_since = Some(now);
                }
            }
        }
    }

    /// The state of texture `t`.
    #[must_use]
    pub fn texture(&self, t: TexRequest) -> TexLookup {
        match self.gpu_tex.get(&t).map(|e| &e.state) {
            Some(TexState::Ready(id, ..)) => TexLookup::Ready(*id),
            Some(TexState::Failed(_)) => TexLookup::Failed,
            _ => TexLookup::Pending,
        }
    }

    /// The levels of texture `t` no larger than [`SMALL_LEVEL`] texels, largest first,
    /// kept on the CPU once it is on the GPU (for the terrain's far-band bake,
    /// `walker.md` §14.4). `None` while loading or for a texture without such levels.
    #[must_use]
    pub fn texture_small(&self, t: TexRequest) -> Option<Arc<[RgbaImage]>> {
        match self.gpu_tex.get(&t).map(|e| &e.state) {
            Some(TexState::Ready(_, _, _, small)) => small.clone(),
            _ => None,
        }
    }

    /// The mean colour of texture `t`'s smallest level (stored values), once on the GPU.
    #[must_use]
    pub fn texture_mean(&self, t: TexRequest) -> Option<[f32; 3]> {
        match self.gpu_tex.get(&t).map(|e| &e.state) {
            Some(TexState::Ready(_, _, mean, _)) => Some(*mean),
            _ => None,
        }
    }

    /// Adds a reference to each part of `parts` for a user of its instanced models (frills,
    /// sky objects): the part is loaded and its [`WorldModel`]s are built once its
    /// textures are on the GPU ([`ObjectLayer::model`]).
    pub fn acquire_model_parts(&mut self, parts: &[u32]) {
        self.acquire_parts(parts, Priority::Visible, true);
    }

    /// Drops the references taken with [`ObjectLayer::acquire_model_parts`] or
    /// [`ObjectLayer::acquire_frill_parts`].
    pub fn release_model_parts(&mut self, parts: &[u32]) {
        self.release_parts(parts);
    }

    /// Like [`ObjectLayer::acquire_model_parts`], for parts drawn as frills: their
    /// [`ObjectLayer::frill_model`] is built too.
    pub fn acquire_frill_parts(&mut self, parts: &[u32]) {
        self.acquire_parts(parts, Priority::Visible, true);
        for p in parts {
            let Some(e) = self.gpu_parts.get_mut(p) else {
                continue;
            };
            if e.want_frill {
                continue;
            }
            e.want_frill = true;
            // Level 0 already built without its first-entry model: decode it again.
            if let PartState::Ready(g) = &e.state
                && g.levels
                    .first()
                    .is_some_and(|l| l.needs_frill_model() && l.data.is_none() && l.instanceable)
                && e.model_job.is_none()
            {
                let key = part_key(self.looks.as_deref(), *p);
                e.model_job = Some(self.parts.request(key, Priority::Visible));
            }
        }
    }

    /// This frame's texture matrices of the surfaces of `part`'s level `level` when one
    /// of them animates (`material.md` §8.4; for instanced batches drawn outside the object
    /// layer, after [`ObjectLayer::draw`] started the frame's clock).
    pub fn animated_uv(&mut self, part: u32, level: usize) -> Option<Arc<[[UvTransform; 2]]>> {
        let Some(PartState::Ready(g)) = self.gpu_parts.get(&part).map(|e| &e.state) else {
            return None;
        };
        level_uv(g.levels.get(level)?, &mut self.anim)
    }

    /// The frill model of `part` (`scenery.md` §8.5 step 3): the first draw entry of its
    /// detail level 0, once built. For a level with one draw entry that is its instanced
    /// model; should the first-entry model fail, the whole level.
    #[must_use]
    pub fn frill_model(&self, part: u32) -> Option<&WorldModel> {
        match &self.gpu_parts.get(&part)?.state {
            PartState::Ready(g) => {
                let l = g.levels.first()?;
                if l.plans.len() > 1 && !l.frill_failed {
                    l.frill_model.as_ref()
                } else {
                    l.model.as_ref()
                }
            }
            _ => None,
        }
    }

    /// The instanced model of detail level `level` of `part` (clamped to its last level),
    /// once built.
    #[must_use]
    pub fn model(&self, part: u32, level: usize) -> Option<&WorldModel> {
        match &self.gpu_parts.get(&part)?.state {
            PartState::Ready(g) => g
                .levels
                .get(level.min(g.levels.len().saturating_sub(1)))?
                .model
                .as_ref(),
            _ => None,
        }
    }

    /// The cell the placed figure `tag` ([`figure_tag`]) is drawn through as a static
    /// instance (`landblock.md` §4.11: [`OUTDOORS`] for the outdoor visibility cell, a cell
    /// number of its landblock, or [`UNATTACHED`](super::interiors::UNATTACHED)), with
    /// its landblock; `None` when its landblock's objects are not in the layer or the
    /// figure has no instance.
    #[must_use]
    pub fn figure_cell(&self, tag: u64) -> Option<(LandblockKey, u16)> {
        let (key, index) = super::figures::tag_parts(tag)?;
        let cell = *self.blocks.get(&key)?.figure_cells.get(&index)?;
        Some((key, cell))
    }

    /// Whether `part` is on the GPU (or failed for good, so waiting for it is pointless).
    #[must_use]
    pub fn part_settled(&self, part: u32) -> bool {
        match self.gpu_parts.get(&part).map(|e| &e.state) {
            Some(PartState::Ready(g)) => g
                .levels
                .iter()
                .all(|l| l.model.is_some() || l.model_failed || l.data.is_none()),
            Some(PartState::Failed) => true,
            _ => false,
        }
    }

    /// Pixels per world unit at distance 1 (for the projected-size cull).
    pub fn set_projection_scale(&mut self, s: f32) {
        if s.is_finite() && s > 0.0 {
            self.projection_scale = s;
        }
    }

    /// Counters.
    #[must_use]
    pub fn stats(&self) -> ObjectStats {
        self.stats
    }

    /// The fallback diagnostic of the last frame drawn: submesh draws without their
    /// base-colour texture, by reason and surface, and the draws of records with programs of
    /// their own drawn through the generic path ([`super::fallback`]).
    #[must_use]
    pub fn fallback_stats(&self) -> &FallbackStats {
        &self.fallback_stats
    }

    /// Whether every part and texture in use is on the GPU or failed, and every instanced
    /// model wanted is built.
    #[must_use]
    pub fn settled(&self) -> bool {
        self.gpu_parts
            .values()
            .filter(|e| e.refs > 0)
            .all(|e| match &e.state {
                PartState::Ready(g) => {
                    !e.want_model
                        || g.levels
                            .iter()
                            .all(|l| l.model.is_some() || l.model_failed || l.data.is_none())
                }
                PartState::Failed => true,
                _ => false,
            })
            && self
                .gpu_tex
                .values()
                .filter(|e| e.refs > 0)
                .all(|e| matches!(e.state, TexState::Ready(..) | TexState::Failed(_)))
    }

    /// Sets the objects of `key` (replacing earlier ones).
    pub fn insert(
        &mut self,
        key: LandblockKey,
        instances: Arc<[Instance]>,
        objects: Arc<[ObjectInfo]>,
    ) {
        self.remove(key);
        self.blocks
            .insert(key, BlockObjects::new(instances, objects));
    }

    /// Forgets `key`'s objects (their parts lose a reference).
    pub fn remove(&mut self, key: LandblockKey) {
        if let Some(b) = self.blocks.remove(&key) {
            let mut parts = Vec::new();
            for (i, o) in b.objects.iter().enumerate() {
                if b.state.get(i).is_some_and(|s| s.active) {
                    parts.extend(object_parts(&b.instances, o));
                }
            }
            self.release_parts(&parts);
        }
    }

    /// Keys tracked.
    pub fn keys(&self) -> impl Iterator<Item = LandblockKey> + '_ {
        self.blocks.keys().copied()
    }

    fn release_parts(&mut self, parts: &[u32]) {
        let now = Instant::now();
        for p in parts {
            let Some(e) = self.gpu_parts.get_mut(p) else {
                continue;
            };
            e.refs = e.refs.saturating_sub(1);
            if e.refs == 0 {
                match &e.state {
                    PartState::Loading(h) => {
                        h.cancel();
                        self.gpu_parts.remove(p);
                    }
                    PartState::Retry(_) | PartState::Failed | PartState::Waiting(_) => {
                        self.gpu_parts.remove(p);
                    }
                    PartState::Ready(_) => e.unused_since = Some(now),
                }
            }
        }
    }

    fn acquire_parts(&mut self, parts: &[u32], priority: Priority, want_model: bool) {
        for &p in parts {
            let e = self.gpu_parts.entry(p).or_insert_with(|| PartEntry {
                refs: 0,
                state: PartState::Retry(Instant::now() - RETRY_AFTER),
                priority,
                unused_since: None,
                want_model,
                want_frill: false,
                model_job: None,
            });
            e.refs += 1;
            e.unused_since = None;
            if priority > e.priority {
                e.priority = priority;
                if let PartState::Loading(h) = &e.state {
                    h.set_priority(priority);
                }
            }
            if want_model {
                self.want_model(p);
            }
        }
    }

    /// Marks part `p` as wanted with its instanced models; one uploaded without its
    /// streams is decoded again for them.
    fn want_model(&mut self, p: u32) {
        let Some(e) = self.gpu_parts.get_mut(&p) else {
            return;
        };
        if e.want_model {
            return;
        }
        e.want_model = true;
        if let PartState::Ready(g) = &e.state
            && g.levels
                .iter()
                .all(|l| l.model.is_none() && l.data.is_none())
            && e.model_job.is_none()
        {
            let key = part_key(self.looks.as_deref(), p);
            e.model_job = Some(self.parts.request(key, Priority::Visible));
        }
    }

    /// Per frame: activates and releases objects by their draw distances, requests,
    /// collects and uploads within `budget`, builds instanced models, evicts over budget.
    /// `camera` is region-global.
    pub fn update(
        &mut self,
        renderer: &mut Renderer<'_>,
        camera: [f64; 3],
        frame: &ObjectFrame,
        budget: &mut UploadBudget,
    ) {
        let now = Instant::now();
        // Instancing switched on: the parts in use want their models now.
        let instancing_on = frame.instancing && !self.frame.instancing;
        self.frame = *frame;
        if instancing_on {
            let parts: Vec<u32> = self
                .gpu_parts
                .iter()
                .filter(|(_, e)| e.refs > 0 && !e.want_model)
                .map(|(p, _)| *p)
                .collect();
            for p in parts {
                self.want_model(p);
            }
        }
        self.adapt_distance(now);
        self.activate(camera, frame);
        self.request_parts(now);
        self.poll_parts();
        let uploaded = self.upload_parts(renderer, budget, now);
        self.request_textures(now);
        self.poll_textures();
        self.upload_textures(renderer, budget);
        self.build_models(renderer, budget);
        self.evict(renderer, now);
        self.refresh_bounds(&uploaded);
        self.refresh_stats();
    }

    /// The activation pass (module docs, [`activate_blocks`]), then the references it
    /// takes and drops.
    fn activate(&mut self, camera: [f64; 3], frame: &ObjectFrame) {
        let cap = self.distance as f32;
        let (acquire, release) = activate_blocks(
            &mut self.blocks,
            &mut self.order,
            camera,
            frame,
            cap,
            self.config.near,
        );
        for (p, prio, model) in acquire {
            self.acquire_parts(&[p], prio, model);
        }
        self.release_parts(&release);
    }

    /// Whether part meshes had to wait for arena space within [`ARENA_PRESSURE`].
    fn arena_pressed(&self, now: Instant) -> bool {
        self.arena_full_at
            .is_some_and(|t| now.saturating_duration_since(t) < ARENA_PRESSURE)
    }

    /// Recomputes the boxes and object spheres of blocks that use one of `uploaded`
    /// (sorted) or were marked dirty.
    fn refresh_bounds(&mut self, uploaded: &[u32]) {
        let parts = &self.gpu_parts;
        let sphere = |p: u32| match parts.get(&p).map(|e| &e.state) {
            Some(PartState::Ready(g)) => Some((g.center, g.radius)),
            _ => None,
        };
        for b in self.blocks.values_mut() {
            if b.active == 0 {
                if b.dirty {
                    b.bounds = None;
                    b.dirty = false;
                }
                continue;
            }
            let touched = b.dirty || sorted_intersect(&b.parts, uploaded);
            if !touched {
                continue;
            }
            for (i, obj) in b.objects.iter().enumerate() {
                let range = obj.first as usize..(obj.first as usize + obj.count as usize);
                let Some(insts) = b.instances.get(range) else {
                    continue;
                };
                let s = enclosing_sphere(insts.iter().filter_map(|inst| {
                    let (c, r) = sphere(inst.part)?;
                    Some((inst.model.transform_point3(c), r * inst.scale))
                }));
                if let Some(st) = b.state.get_mut(i) {
                    st.sphere = s;
                }
            }
            let active: Vec<Instance> = b
                .instances
                .iter()
                .filter(|i| b.state.get(i.object as usize).is_some_and(|s| s.active))
                .copied()
                .collect();
            b.bounds = instance_bounds(&active, sphere);
            b.dirty = false;
        }
    }

    fn request_parts(&mut self, now: Instant) {
        for (&did, e) in &mut self.gpu_parts {
            if let PartState::Retry(at) = e.state
                && e.refs > 0
                && now >= at
            {
                let h = self
                    .parts
                    .request(part_key(self.looks.as_deref(), did), e.priority);
                e.state = match h.state() {
                    AssetState::Ready(a) => PartState::Waiting(a),
                    AssetState::Failed(err) if err.is_data_error() => {
                        tracing::debug!(part = %format_args!("{did:#010x}"), "part failed: {err}");
                        PartState::Failed
                    }
                    AssetState::Failed(_) | AssetState::Cancelled => {
                        PartState::Retry(now + RETRY_AFTER)
                    }
                    AssetState::Queued | AssetState::Loading => PartState::Loading(h),
                };
            }
        }
    }

    fn poll_parts(&mut self) {
        let mut models: Vec<(u32, Arc<PartAsset>)> = Vec::new();
        for c in self.parts.poll_completions(4096) {
            let part = super::looks::part_of(c.key);
            let Some(e) = self.gpu_parts.get_mut(&part) else {
                continue;
            };
            if e.model_job.is_some() && matches!(e.state, PartState::Ready(_)) {
                if let Outcome::Ready(a) = c.outcome {
                    models.push((part, a));
                }
                e.model_job = None;
                continue;
            }
            if !matches!(e.state, PartState::Loading(_)) {
                continue;
            }
            e.state = match c.outcome {
                Outcome::Ready(a) => PartState::Waiting(a),
                Outcome::Failed(err) => {
                    if err.is_data_error() {
                        tracing::debug!(part = %c.key, "part failed: {err}");
                        PartState::Failed
                    } else {
                        PartState::Retry(Instant::now() + RETRY_AFTER)
                    }
                }
                Outcome::Cancelled => PartState::Retry(Instant::now()),
            };
        }
        // Handles that finished without a completion (dropped completions).
        for (&did, e) in &mut self.gpu_parts {
            if let PartState::Loading(h) = &e.state
                && h.is_finished()
            {
                e.state = match h.state() {
                    AssetState::Ready(a) => PartState::Waiting(a),
                    AssetState::Failed(err) if err.is_data_error() => PartState::Failed,
                    _ => PartState::Retry(Instant::now() + RETRY_AFTER),
                };
            }
            if let Some(h) = &e.model_job
                && h.is_finished()
            {
                if let AssetState::Ready(a) = h.state() {
                    models.push((did, a));
                }
                e.model_job = None;
            }
        }
        // Model streams of a part uploaded without them.
        for (did, asset) in models {
            if let Some(PartEntry {
                state: PartState::Ready(g),
                want_frill,
                ..
            }) = self.gpu_parts.get_mut(&did)
            {
                for (k, (gl, la)) in g.levels.iter_mut().zip(&asset.levels).enumerate() {
                    let frill = k == 0 && *want_frill && gl.needs_frill_model();
                    if (gl.model.is_none() || frill) && gl.data.is_none() && gl.instanceable {
                        gl.data = Some(Arc::clone(&la.data));
                        gl.model_failed = false;
                    }
                }
            }
        }
    }

    /// Uploads waiting parts, nearest landblocks first; returns the parts uploaded
    /// (sorted).
    fn upload_parts(
        &mut self,
        renderer: &mut Renderer<'_>,
        budget: &mut UploadBudget,
        now: Instant,
    ) -> Vec<u32> {
        let mut uploaded = Vec::new();
        // Nearest landblocks' parts first, then parts used outside the blocks.
        let mut wanted: Vec<u32> = Vec::new();
        let mut seen: HashSet<u32> = HashSet::new();
        let waiting = self
            .gpu_parts
            .values()
            .any(|e| matches!(e.state, PartState::Waiting(_)));
        for &(_, key) in &self.order {
            if !waiting {
                break;
            }
            let Some(b) = self.blocks.get(&key) else {
                continue;
            };
            if b.active == 0 {
                continue;
            }
            for &p in &b.parts {
                if matches!(
                    self.gpu_parts.get(&p).map(|e| &e.state),
                    Some(PartState::Waiting(_))
                ) && seen.insert(p)
                {
                    wanted.push(p);
                }
            }
        }
        for (&p, e) in &self.gpu_parts {
            if matches!(e.state, PartState::Waiting(_)) && !seen.contains(&p) {
                wanted.push(p);
            }
        }
        for p in wanted {
            let Some(e) = self.gpu_parts.get_mut(&p) else {
                continue;
            };
            let PartState::Waiting(asset) = &e.state else {
                continue;
            };
            // Leave the terrain its share of the shared mesh arena.
            if self.mesh_bytes.saturating_add(asset.gpu_bytes) > self.arena_limit {
                self.arena_full_at = Some(now);
                break;
            }
            if !budget.try_take(asset.gpu_bytes) {
                break;
            }
            let asset = Arc::clone(asset);
            let want_model = e.want_model;
            let mut levels: Vec<GpuLevel> = Vec::with_capacity(asset.levels.len());
            let mut failure: Option<tessera_render::RenderError> = None;
            for la in &asset.levels {
                let prepared = la
                    .mesh
                    .lock()
                    .unwrap_or_else(PoisonError::into_inner)
                    .take();
                let Some(prepared) = prepared else {
                    failure = Some(tessera_render::RenderError::InvalidMesh(
                        "part streams already taken".into(),
                    ));
                    break;
                };
                match renderer.upload_prepared(prepared) {
                    Ok(id) => {
                        let instanceable = la.plans.iter().all(MaterialPlan::instanceable);
                        levels.push(GpuLevel {
                            mesh: id,
                            plans: la.plans.clone(),
                            anims: la.anims.clone(),
                            model: None,
                            frill_model: None,
                            frill_failed: false,
                            data: (want_model && instanceable).then(|| Arc::clone(&la.data)),
                            model_failed: false,
                            instanceable,
                            bytes: la.gpu_bytes,
                        });
                    }
                    Err(err) => {
                        failure = Some(err);
                        break;
                    }
                }
            }
            match failure {
                None => {
                    let (center, radius) = levels
                        .first()
                        .and_then(|l| renderer.mesh_info(l.mesh))
                        .map_or((Vec3::ZERO, 1.0), |i| (i.sphere.center, i.sphere.radius));
                    let bytes: u64 = levels.iter().map(|l| l.bytes).sum();
                    self.mesh_bytes += bytes;
                    let part = GpuPart {
                        levels,
                        center,
                        radius,
                        bytes,
                    };
                    for t in part.textures() {
                        let te = self.gpu_tex.entry(t).or_insert_with(|| TexEntry {
                            refs: 0,
                            state: TexState::Retry(now - RETRY_AFTER),
                            unused_since: None,
                        });
                        te.refs += 1;
                        te.unused_since = None;
                    }
                    e.state = PartState::Ready(part);
                    uploaded.push(p);
                }
                Some(err) => {
                    for l in levels {
                        renderer.destroy_mesh(l.mesh);
                    }
                    match err {
                        tessera_render::RenderError::LimitExceeded {
                            what: "mesh arena bytes",
                            ..
                        } => {
                            // Transient: space comes back as parts are evicted. The
                            // streams were moved out, so the part is decoded again.
                            tracing::debug!(part = %format_args!("{p:#010x}"), "mesh arena full; part retried later");
                            self.arena_full_at = Some(now);
                            e.state = PartState::Retry(now + ARENA_RETRY);
                        }
                        err => {
                            tracing::debug!(part = %format_args!("{p:#010x}"), "part upload failed: {err}");
                            e.state = PartState::Failed;
                        }
                    }
                }
            }
        }
        uploaded.sort_unstable();
        uploaded
    }

    /// Builds the instanced models of uploaded parts whose textures are settled.
    fn build_models(&mut self, renderer: &mut Renderer<'_>, budget: &mut UploadBudget) {
        let gpu_tex = &self.gpu_tex;
        let lookup = |t: TexRequest| match gpu_tex.get(&t).map(|e| &e.state) {
            Some(TexState::Ready(id, ..)) => TexLookup::Ready(*id),
            Some(TexState::Failed(_)) => TexLookup::Failed,
            _ => TexLookup::Pending,
        };
        let mut added = 0u64;
        'parts: for (&p, e) in &mut self.gpu_parts {
            if e.refs == 0 || !e.want_model {
                continue;
            }
            let want_frill = e.want_frill;
            let PartState::Ready(g) = &mut e.state else {
                continue;
            };
            for (k, l) in g.levels.iter_mut().enumerate() {
                let Some(data) = l.data.clone() else { continue };
                let frill = k == 0 && want_frill && l.needs_frill_model();
                if l.model.is_some() && !frill {
                    l.data = None;
                    continue;
                }
                let mut surfaces = Vec::with_capacity(l.plans.len());
                let mut state = ModelSurface::Pending;
                for plan in &l.plans {
                    state = world_surface(plan, &lookup);
                    match state {
                        ModelSurface::Ready(s) => surfaces.push(s),
                        _ => break,
                    }
                }
                match state {
                    ModelSurface::Pending if surfaces.len() < l.plans.len() => continue,
                    ModelSurface::Unusable => {
                        l.model_failed = true;
                        l.data = None;
                        continue;
                    }
                    _ => {}
                }
                if !budget.try_take(l.bytes) {
                    // Out of budget: the models built so far still count.
                    break 'parts;
                }
                if l.model.is_none() {
                    match renderer.upload_world_model(&data, &surfaces) {
                        Ok(m) => {
                            added += m.gpu_bytes();
                            l.model = Some(m);
                        }
                        Err(err) => {
                            tracing::debug!(part = %format_args!("{p:#010x}"), level = k, "instanced model failed: {err}");
                            l.model_failed = true;
                        }
                    }
                }
                if frill {
                    // The first draw entry only (`scenery.md` §8.5 step 3).
                    let mut first = (*data).clone();
                    first.submeshes.truncate(1);
                    match renderer.upload_world_model(&first, surfaces.get(..1).unwrap_or(&[])) {
                        Ok(m) => {
                            added += m.gpu_bytes();
                            l.frill_model = Some(m);
                        }
                        Err(err) => {
                            tracing::debug!(part = %format_args!("{p:#010x}"), "frill model failed: {err}");
                            l.frill_failed = true;
                        }
                    }
                }
                l.data = None;
            }
        }
        self.mesh_bytes += added;
    }

    fn request_textures(&mut self, now: Instant) {
        for (t, e) in &mut self.gpu_tex {
            if let TexState::Retry(at) = e.state
                && e.refs > 0
                && now >= at
            {
                let h = self
                    .textures
                    .request(AssetKey::new(t.did, t.variant), Priority::Visible);
                e.state = match h.state() {
                    AssetState::Ready(img) => TexState::Waiting(img),
                    AssetState::Failed(err) if err.is_data_error() => {
                        tracing::debug!(texture = %format_args!("{:#010x}/{}", t.did, t.variant), "texture failed: {err}");
                        TexState::Failed(texture_failure_reason(&err))
                    }
                    AssetState::Failed(_) | AssetState::Cancelled => {
                        TexState::Retry(now + RETRY_AFTER)
                    }
                    AssetState::Queued | AssetState::Loading => TexState::Loading(h),
                };
            }
        }
    }

    fn poll_textures(&mut self) {
        for c in self.textures.poll_completions(4096) {
            let t = TexRequest {
                did: c.key.did,
                variant: c.key.variant,
            };
            let Some(e) = self.gpu_tex.get_mut(&t) else {
                continue;
            };
            if !matches!(e.state, TexState::Loading(_)) {
                continue;
            }
            e.state = match c.outcome {
                Outcome::Ready(img) => TexState::Waiting(img),
                Outcome::Failed(err) if err.is_data_error() => {
                    tracing::debug!(texture = %c.key, "texture failed: {err}");
                    TexState::Failed(texture_failure_reason(&err))
                }
                Outcome::Failed(_) | Outcome::Cancelled => {
                    TexState::Retry(Instant::now() + RETRY_AFTER)
                }
            };
        }
        for e in self.gpu_tex.values_mut() {
            if let TexState::Loading(h) = &e.state
                && h.is_finished()
            {
                e.state = match h.state() {
                    AssetState::Ready(img) => TexState::Waiting(img),
                    AssetState::Failed(err) if err.is_data_error() => {
                        TexState::Failed(texture_failure_reason(&err))
                    }
                    _ => TexState::Retry(Instant::now() + RETRY_AFTER),
                };
            }
        }
    }

    fn upload_textures(&mut self, renderer: &mut Renderer<'_>, budget: &mut UploadBudget) {
        for (t, e) in &mut self.gpu_tex {
            let TexState::Waiting(img) = &e.state else {
                continue;
            };
            let bytes = img.bytes() as u64;
            if !budget.try_take(bytes) {
                break;
            }
            let label = format!("tex {:#010x}/{}", t.did, t.variant);
            let color_space = if t.variant == VARIANT_DYE_MASK {
                tessera_render::ColorSpace::Linear
            } else {
                tessera_render::ColorSpace::Srgb
            };
            let result = match img.levels.as_slice() {
                [one] => renderer.upload_texture(&TextureUpload {
                    label: Some(&label),
                    wrap: Wrap::Repeat,
                    color_space,
                    ..TextureUpload::rgba8(one.width, one.height, &one.pixels)
                }),
                levels => {
                    let mips: Vec<MipLevel<'_>> = levels
                        .iter()
                        .map(|l| MipLevel {
                            width: l.width,
                            height: l.height,
                            rgba: &l.pixels,
                        })
                        .collect();
                    renderer.upload_texture_mips(&MipChainUpload {
                        label: Some(&label),
                        color_space,
                        ..MipChainUpload::new(&mips)
                    })
                }
            };
            match result {
                Ok(id) => {
                    self.texture_bytes += bytes;
                    let small: Vec<RgbaImage> = img
                        .levels
                        .iter()
                        .filter(|l| l.width.max(l.height) <= SMALL_LEVEL)
                        .cloned()
                        .collect();
                    e.state = TexState::Ready(
                        id,
                        bytes,
                        img.mean,
                        (!small.is_empty()).then(|| small.into()),
                    );
                }
                Err(err) => {
                    tracing::debug!(texture = %label, "texture upload failed: {err}");
                    e.state = TexState::Failed(FallbackReason::TextureComposeFailed);
                }
            }
        }
    }

    /// Frees unreferenced parts and textures: those idle longer than [`IDLE_KEEP`], and
    /// more (least recently used first) while over the GPU budget.
    fn evict(&mut self, renderer: &mut Renderer<'_>, now: Instant) {
        let pressed = self.arena_pressed(now);
        let over = |s: &Self| pressed || s.mesh_bytes + s.texture_bytes > s.config.gpu_budget;
        let mut idle: Vec<(Instant, u32)> = self
            .gpu_parts
            .iter()
            .filter(|(_, e)| e.refs == 0)
            .filter_map(|(d, e)| e.unused_since.map(|t| (t, *d)))
            .collect();
        idle.sort_unstable();
        for (since, did) in idle {
            if now.saturating_duration_since(since) < IDLE_KEEP && !over(self) {
                break;
            }
            if let Some(e) = self.gpu_parts.remove(&did)
                && let PartState::Ready(g) = e.state
            {
                let textures: Vec<TexRequest> = g.textures().collect();
                for l in g.levels {
                    renderer.destroy_mesh(l.mesh);
                    for m in [l.model, l.frill_model].into_iter().flatten() {
                        self.mesh_bytes = self.mesh_bytes.saturating_sub(m.gpu_bytes());
                    }
                }
                self.mesh_bytes = self.mesh_bytes.saturating_sub(g.bytes);
                for t in textures {
                    if let Some(te) = self.gpu_tex.get_mut(&t) {
                        te.refs = te.refs.saturating_sub(1);
                        if te.refs == 0 {
                            te.unused_since = Some(now);
                        }
                    }
                }
            }
        }
        // Textures nobody references.
        let mut idle: Vec<(Instant, TexRequest)> = self
            .gpu_tex
            .iter()
            .filter(|(_, e)| e.refs == 0)
            .map(|(t, e)| (e.unused_since.unwrap_or(now), *t))
            .collect();
        idle.sort_unstable();
        for (since, t) in idle {
            let ready = matches!(
                self.gpu_tex.get(&t).map(|e| &e.state),
                Some(TexState::Ready(..))
            );
            if ready && now.saturating_duration_since(since) < IDLE_KEEP && !over(self) {
                continue;
            }
            if let Some(e) = self.gpu_tex.remove(&t) {
                match e.state {
                    TexState::Ready(id, bytes, ..) => {
                        renderer.destroy_texture(id);
                        self.texture_bytes = self.texture_bytes.saturating_sub(bytes);
                    }
                    TexState::Loading(h) => h.cancel(),
                    _ => {}
                }
            }
        }
    }

    /// Keeps GPU memory near the budget: resources still referenced by objects in range
    /// cannot be evicted, so while they alone exceed the budget (or parts in range wait
    /// for mesh-arena space) the distance cap shrinks (10 % per second, down to one
    /// landblock), and it grows back while under 80 % with no arena pressure.
    fn adapt_distance(&mut self, now: Instant) {
        if self
            .adjusted
            .is_some_and(|t| now.saturating_duration_since(t) < Duration::from_secs(1))
        {
            return;
        }
        // Only what is referenced counts: the rest is a cache that the eviction trims to
        // the budget anyway.
        let mut textures: HashSet<TexRequest> = HashSet::new();
        let mut used = 0u64;
        for e in self.gpu_parts.values().filter(|e| e.refs > 0) {
            if let PartState::Ready(g) = &e.state {
                used += g.bytes;
                textures.extend(g.textures());
            }
        }
        for t in &textures {
            if let Some(TexEntry {
                state: TexState::Ready(_, b, ..),
                ..
            }) = self.gpu_tex.get(t)
            {
                used += b;
            }
        }
        let budget = self.config.gpu_budget;
        let before = self.distance;
        let pressed = self.arena_pressed(now);
        if used > budget || pressed {
            self.distance = (self.distance * 0.9).max(LANDBLOCK_SIZE);
        } else if used < budget / 10 * 8 {
            self.distance = (self.distance * 1.05).min(self.config.distance);
        }
        if (self.distance - before).abs() > f64::EPSILON {
            tracing::debug!(
                distance = self.distance,
                used,
                budget,
                "object distance cap adjusted"
            );
        }
        self.adjusted = Some(now);
    }

    fn refresh_stats(&mut self) {
        let mut s = ObjectStats {
            blocks: self.blocks.len(),
            draws: self.stats.draws,
            objects_drawn: self.stats.objects_drawn,
            fading: self.stats.fading,
            instanced: self.stats.instanced,
            batches: self.stats.batches,
            culled: self.stats.culled,
            capped: self.stats.capped,
            mesh_bytes: self.mesh_bytes,
            texture_bytes: self.texture_bytes,
            distance: self.distance,
            ..ObjectStats::default()
        };
        for b in self.blocks.values() {
            s.objects += b.objects.len();
            if b.active > 0 {
                s.active_blocks += 1;
                s.active_objects += b.active;
                s.instances += b.active_instances;
            }
        }
        for e in self.gpu_parts.values() {
            if e.refs > 0 {
                s.parts += 1;
            }
            match &e.state {
                PartState::Ready(g) => {
                    s.parts_ready += 1;
                    s.models += g.levels.iter().filter(|l| l.model.is_some()).count();
                }
                PartState::Failed => s.parts_failed += 1,
                _ => s.parts_pending += 1,
            }
        }
        for e in self.gpu_tex.values() {
            match e.state {
                TexState::Ready(..) => s.textures_ready += 1,
                TexState::Failed(_) => {}
                _ => s.textures_pending += 1,
            }
        }
        self.stats = s;
    }

    /// Records the visible objects into `pass`. `camera` is the eye in `anchor`'s frame.
    /// `characters` holds the tags ([`figure_tag`]) of the placed figures the character
    /// layer draws: their static instances are left out (`walker.md` §10.2).
    pub fn draw(
        &mut self,
        pass: &mut Pass,
        anchor: &Anchor,
        camera: Vec3,
        frustum: &Frustum,
        characters: &HashSet<u64>,
        drawn: &dyn Fn(LandblockKey, u16) -> bool,
    ) {
        let frame = self.frame;
        self.draw_frame = self.draw_frame.saturating_add(1);
        let draw_frame = self.draw_frame;
        let cap = self.distance as f32;
        let mut draws = 0usize;
        let mut objects_drawn = 0usize;
        let mut fading = 0usize;
        let mut instanced = 0usize;
        let mut culled = 0usize;
        let mut capped = false;
        let min_px = self.config.min_pixels;
        let scale = self.projection_scale;
        for v in self.batches.values_mut() {
            v.clear();
        }
        let gpu_tex = &self.gpu_tex;
        let lookup = |t: TexRequest| match gpu_tex.get(&t).map(|e| &e.state) {
            Some(TexState::Ready(id, ..)) => TexLookup::Ready(*id),
            Some(TexState::Failed(_)) => TexLookup::Failed,
            _ => TexLookup::Pending,
        };
        let failed = |t: TexRequest| match gpu_tex.get(&t).map(|e| &e.state) {
            Some(TexState::Failed(r)) => Some(*r),
            _ => None,
        };
        self.fallback.clear();
        self.anim.begin(frame.time);
        // *O* of water drawings on placed meshes: the region origin (`water.md` §8.3
        // Tessera rule); the camera's landblock for curve-valued scales (§8.7.1).
        let region_offset = {
            let o = anchor.origin();
            [-(o[0] as f32), -(o[1] as f32)]
        };
        let order = std::mem::take(&mut self.order);
        for &(_, key) in &order {
            let Some(b) = self.blocks.get_mut(&key) else {
                continue;
            };
            if b.active == 0 {
                continue;
            }
            let Some(off) = anchor.offset_of(key) else {
                continue;
            };
            // Nothing of the block on the GPU yet, or its box is out of view.
            let Some((lo, hi)) = b.bounds else {
                continue;
            };
            let local_cam = camera - off;
            // The draw cap stopped the frame: the block is not reached, but the smoothed
            // distances of its objects still follow the camera.
            if capped {
                b.follow_distances(key, local_cam, &frame, drawn, 0, (draw_frame, cap));
                continue;
            }
            if !frustum.aabb_visible(off + lo, off + hi) {
                culled += b.active;
                b.follow_distances(key, local_cam, &frame, drawn, 0, (draw_frame, cap));
                continue;
            }
            let block_tf = Mat4::from_translation(off);
            // Index of the object whose draw reached the cap.
            let mut capped_at = None;
            'objects: for (oi, obj) in b.objects.iter().enumerate() {
                let Some(st) = b.state.get_mut(oi) else { break };
                if !st.active || (obj.generated && !frame.scenery) || !drawn(key, obj.cell) {
                    continue;
                }
                let Some((c, r)) = st.sphere else { continue };
                let d = aabb_distance(
                    local_cam.to_array(),
                    (c - Vec3::splat(r)).to_array(),
                    (c + Vec3::splat(r)).to_array(),
                );
                let draw_d = object_draw_distance(&obj.lod, &frame, cap);
                let reset = fade_resets(st.faded_at, draw_frame);
                st.faded_at = draw_frame;
                let opacity =
                    st.fade
                        .update(d, draw_d, frame.dt, frame.camera_moved, reset, frame.lod);
                // Before the culls: see the culled-block branch above.
                let ds = st.smooth.update(d, frame.fps, false);
                if opacity <= 0.0 {
                    continue;
                }
                let center = off + c;
                let dc = (center - camera).length();
                if r * scale < min_px * (dc - r).max(1.0) || !frustum.sphere_visible(center, r) {
                    culled += 1;
                    continue;
                }
                let detail = obj.lod.detail_range(&frame.settings);
                // `mesh.md` §8.7 `d2`: objects cast shadows within their shadow distance.
                // Strictly within the shadow distance (`mesh.md` §8.7).
                let casts = frame.shadows
                    && (!frame.lod || d < obj.lod.shadow_distance(frame.g, &frame.settings));
                objects_drawn += 1;
                if opacity < 1.0 {
                    fading += 1;
                }
                let range = obj.first as usize..(obj.first as usize + obj.count as usize);
                let Some(insts) = b.instances.get(range) else {
                    continue;
                };
                for inst in insts {
                    if drawn_as_character(key, inst, characters) {
                        continue;
                    }
                    let Some(PartState::Ready(g)) =
                        self.gpu_parts.get(&inst.part).map(|e| &e.state)
                    else {
                        continue;
                    };
                    let n = g.levels.len();
                    let k = if frame.lod {
                        detail_level(n, detail, ds, true)
                    } else {
                        0
                    }
                    .min(n.saturating_sub(1));
                    let Some(level) = g.levels.get(k) else {
                        continue;
                    };
                    let tf = block_tf * inst.model;
                    if frame.instancing
                        && frame.lit
                        && opacity >= 1.0
                        && level.instanceable
                        && level.model.is_some()
                    {
                        self.batches
                            .entry((key, inst.part, u8::try_from(k).unwrap_or(u8::MAX), casts))
                            .or_default()
                            .push(GpuInstance::new(tf));
                        instanced += 1;
                        continue;
                    }
                    for (i, plan) in level.plans.iter().enumerate() {
                        if draws >= self.config.max_draws {
                            capped = true;
                            capped_at = Some(oi);
                            break 'objects;
                        }
                        let anim = level.anims.get(i).and_then(Option::as_ref);
                        // `water.md` §8.5 rule: the records of `material.md` §11.6 are
                        // drawn by their own programs at the mesh's own triangles.
                        if let (Some(own), Some(anim), true) = (plan.own, anim, frame.lit) {
                            let mut ids = [None; OWN_TEXTURES];
                            let mut pending = false;
                            for (slot, t) in ids.iter_mut().zip(own.textures) {
                                match t.map(lookup) {
                                    Some(TexLookup::Ready(id)) => *slot = Some(id),
                                    Some(TexLookup::Pending) => pending = true,
                                    _ => {}
                                }
                            }
                            if pending {
                                continue;
                            }
                            let Some(lf) = self.anim.liquid(anim) else {
                                continue;
                            };
                            let offset = super::liquid::block_offset(
                                lf.animated_scales,
                                region_offset,
                                [camera.x, camera.y],
                            );
                            let (normal_map, textures) = super::liquid::draw_textures(ids);
                            pass.liquid_mesh(LiquidMeshDraw {
                                mesh: level.mesh,
                                submesh: Some(u32::try_from(i).unwrap_or(u32::MAX)),
                                transform: tf,
                                shading: super::liquid::placed(lf.shading, offset, opacity),
                                normal_map,
                                textures,
                                light_object: None,
                            });
                            self.fallback.count(&plan.diag, None, 1);
                            draws += 1;
                            continue;
                        }
                        let Some((material, mut lit)) =
                            surface_draw(plan, &lookup, frame.lit, opacity, casts)
                        else {
                            continue;
                        };
                        // `material.md` §8.4, §5.3.1: animated matrices at this frame's clock.
                        let mut diag = plan.diag;
                        match anim {
                            Some(a) if plan.animated_uv => lit.uv = self.anim.uv(a),
                            None if plan.animated_uv => diag.scrolls = true,
                            _ => {}
                        }
                        if plan.own.is_some() {
                            // An own-program record on the generic path (no lit pass).
                            diag.routed = false;
                        }
                        let reason = plan.diag.fallback.or_else(|| {
                            material.base_texture.is_none().then(|| {
                                plan.texture
                                    .and_then(failed)
                                    .unwrap_or(FallbackReason::TextureComposeFailed)
                            })
                        });
                        self.fallback.count(&diag, reason, 1);
                        pass.mesh(
                            MeshDraw::new(level.mesh, tf, material)
                                .with_submesh(u32::try_from(i).unwrap_or(u32::MAX))
                                .with_lit(lit),
                        );
                        draws += 1;
                    }
                }
            }
            if let Some(oi) = capped_at {
                b.follow_distances(
                    key,
                    local_cam,
                    &frame,
                    drawn,
                    oi.saturating_add(1),
                    (draw_frame, cap),
                );
            }
        }
        self.order = order;
        let mut batches = 0usize;
        for ((_, part, k, casts), list) in &self.batches {
            if list.is_empty() {
                continue;
            }
            let Some(model) = self.model(*part, usize::from(*k)).cloned() else {
                continue;
            };
            let mut uv = None;
            if let Some(PartState::Ready(g)) = self.gpu_parts.get(part).map(|e| &e.state)
                && let Some(level) = g.levels.get(usize::from(*k))
            {
                for plan in &level.plans {
                    let reason = plan.diag.fallback.or_else(|| plan.texture.and_then(failed));
                    self.fallback.count(&plan.diag, reason, list.len());
                }
                uv = level_uv(level, &mut self.anim);
            }
            let mut batch = InstanceBatch::new(model, list.as_slice());
            batch.cast_shadow = *casts;
            batch.uv = uv;
            pass.instanced(batch);
            batches += 1;
        }
        self.batches.retain(|_, v| !v.is_empty());
        self.fallback_stats = self.fallback.stats();
        self.stats.draws = draws;
        self.stats.objects_drawn = objects_drawn;
        self.stats.fading = fading;
        self.stats.instanced = instanced;
        self.stats.batches = batches;
        self.stats.culled = culled;
        self.stats.capped = capped;
    }

    /// Frees every GPU resource and cancels outstanding requests.
    pub fn clear(&mut self, renderer: &mut Renderer<'_>) {
        for (_, e) in self.gpu_parts.drain() {
            match e.state {
                PartState::Ready(g) => {
                    for l in g.levels {
                        renderer.destroy_mesh(l.mesh);
                    }
                }
                PartState::Loading(h) => h.cancel(),
                _ => {}
            }
        }
        for (_, e) in self.gpu_tex.drain() {
            match e.state {
                TexState::Ready(id, ..) => {
                    renderer.destroy_texture(id);
                }
                TexState::Loading(h) => h.cancel(),
                _ => {}
            }
        }
        for b in self.blocks.values_mut() {
            for st in &mut b.state {
                *st = ObjState::default();
            }
            b.active = 0;
            b.active_instances = 0;
            b.bounds = None;
            b.dirty = true;
            b.eval_at = None;
        }
        self.batches.clear();
        self.mesh_bytes = 0;
        self.texture_bytes = 0;
    }
}

/// The activation pass (module docs) over `blocks`: per landblock, when the camera moved
/// [`ACTIVATION_STEP`] since its last pass or anything else it depends on changed
/// ([`ActivationKey`]: *G*, the distance cap `cap`, the switches, the settings), objects
/// within their draw distance plus [`ACTIVATE_MARGIN`] are activated and those beyond it
/// plus [`RELEASE_MARGIN`] (or generated ones with scenery off) released. Fills `order`
/// with the blocks nearest first and returns the part references to take (part,
/// priority: `Visible` within `near`, else `Prefetch`; models wanted) and to drop.
fn activate_blocks(
    blocks: &mut HashMap<LandblockKey, BlockObjects>,
    order: &mut Vec<(f64, LandblockKey)>,
    camera: [f64; 3],
    frame: &ObjectFrame,
    cap: f32,
    near: f64,
) -> (Vec<(u32, Priority, bool)>, Vec<u32>) {
    let akey = ActivationKey::new(frame, cap);
    let keys: Vec<LandblockKey> = blocks.keys().copied().collect();
    order.clear();
    let mut acquire: Vec<(u32, Priority, bool)> = Vec::new();
    let mut release: Vec<u32> = Vec::new();
    for key in keys {
        let d_block = key.distance_to([camera[0], camera[1]]);
        order.push((d_block, key));
        let Some(b) = blocks.get_mut(&key) else {
            continue;
        };
        if !activation_stale(b.eval_at, camera, &akey) {
            continue;
        }
        let o = key.origin();
        // A landblock whose objects are all out of reach is skipped whole (the pass is
        // per object otherwise, and a fast camera reruns it every frame). The landblock
        // whose interior the camera is in is never skipped.
        let interiors_wanted = frame.all_interiors || frame.interior == Some(key);
        if b.active == 0 && !interiors_wanted {
            let reach = if frame.lod {
                (b.max_d0 * frame.settings.object_multiplier())
                    .min(frame.g)
                    .min(cap)
            } else {
                cap
            };
            let lx = (camera[0] - o[0]) as f32;
            let ly = (camera[1] - o[1]) as f32;
            let dx = (b.rect.0[0] - lx).max(lx - b.rect.1[0]).max(0.0);
            let dy = (b.rect.0[1] - ly).max(ly - b.rect.1[1]).max(0.0);
            if dx.hypot(dy) > reach + ACTIVATE_MARGIN + BLOCK_SKIP_SLACK {
                continue;
            }
        }
        b.eval_at = Some((camera, akey));
        for (i, obj) in b.objects.iter().enumerate() {
            let Some(st) = b.state.get_mut(i) else { break };
            if (obj.generated && !frame.scenery) || !interior_eligible(key, obj.cell, frame) {
                if st.active {
                    st.active = false;
                    b.active -= 1;
                    b.active_instances = b.active_instances.saturating_sub(obj.count as usize);
                    release.extend(object_parts(&b.instances, obj));
                }
                continue;
            }
            let dx = o[0] + f64::from(obj.position.x) - camera[0];
            let dy = o[1] + f64::from(obj.position.y) - camera[1];
            let dz = f64::from(obj.position.z) - camera[2];
            let radius = st.sphere.map_or(0.0, |(_, r)| r);
            let d = ((dx * dx + dy * dy + dz * dz).sqrt() as f32 - radius).max(0.0);
            let draw = object_draw_distance(&obj.lod, frame, cap);
            let want = if st.active {
                d <= draw + RELEASE_MARGIN
            } else {
                d <= draw + ACTIVATE_MARGIN
            };
            if want == st.active {
                continue;
            }
            st.active = want;
            if want {
                b.active += 1;
                b.active_instances += obj.count as usize;
                // Parts never outrank the near ring's landblocks (`Immediate`).
                let prio = if f64::from(d) <= near {
                    Priority::Visible
                } else {
                    Priority::Prefetch
                };
                for p in object_parts(&b.instances, obj) {
                    acquire.push((p, prio, frame.instancing));
                }
            } else {
                b.active -= 1;
                b.active_instances = b.active_instances.saturating_sub(obj.count as usize);
                st.fade = DrawFade::new();
                st.smooth.reset();
                release.extend(object_parts(&b.instances, obj));
            }
            b.dirty = true;
        }
    }
    order.sort_unstable_by(|a, b| a.0.total_cmp(&b.0).then(a.1.cmp(&b.1)));
    (acquire, release)
}

/// Whether two ascending lists share an element.
fn sorted_intersect(a: &[u32], b: &[u32]) -> bool {
    let (mut i, mut j) = (0, 0);
    while i < a.len() && j < b.len() {
        match a[i].cmp(&b[j]) {
            std::cmp::Ordering::Less => i += 1,
            std::cmp::Ordering::Greater => j += 1,
            std::cmp::Ordering::Equal => return true,
        }
    }
    false
}

/// The parts of object `obj` (its instances' part DIDs).
fn object_parts<'a>(instances: &'a [Instance], obj: &ObjectInfo) -> impl Iterator<Item = u32> + 'a {
    let start = (obj.first as usize).min(instances.len());
    let end = (obj.first as usize)
        .saturating_add(obj.count as usize)
        .min(instances.len());
    instances[start..end].iter().map(|i| i.part)
}

#[cfg(test)]
mod tests {
    use super::*;
    use tessera_render::{Camera, Perspective};

    fn inst(part: u32, at: Vec3, scale: f32) -> Instance {
        Instance {
            part,
            model: Mat4::from_scale_rotation_translation(
                Vec3::splat(scale),
                tessera_render::glam::Quat::IDENTITY,
                at,
            ),
            scale,
            figure: None,
            object: 0,
        }
    }

    #[test]
    fn figure_instances_are_left_out_only_while_drawn_as_characters() {
        let key = LandblockKey::new(1, 10, 20).unwrap();
        let other = LandblockKey::new(1, 13, 20).unwrap();
        let mut fig = inst(1, Vec3::ZERO, 1.0);
        fig.figure = Some(3);
        let plain = inst(2, Vec3::ZERO, 1.0);
        let mut shown = HashSet::new();
        // Pending, failed or released figures stay static instances.
        assert!(!drawn_as_character(key, &fig, &shown));
        shown.insert(figure_tag(key, 3));
        assert!(drawn_as_character(key, &fig, &shown));
        // Whatever the distance (the sandbox's hysteresis ring included): only the tag
        // counts, and only for that block's figure.
        assert!(!drawn_as_character(other, &fig, &shown));
        shown.insert(figure_tag(other, 2));
        assert!(!drawn_as_character(other, &fig, &shown));
        assert!(!drawn_as_character(key, &plain, &shown));
    }

    #[test]
    fn figures_take_the_cell_of_their_first_instance() {
        // `landblock.md` §4.11 "Moving entities": a placed figure the character layer draws
        // is culled by the cell its static instance is drawn through.
        let key = LandblockKey::new(1, 10, 20).unwrap();
        let o = |cell| ObjectInfo {
            position: Vec3::ZERO,
            lod: LodValues([300.0, 100.0, 16.0, 80.0, 1000.0, 1000.0, 1100.0, -1.0]),
            generated: false,
            first: 0,
            count: 1,
            cell,
        };
        let with = |figure, object| Instance {
            figure: Some(figure),
            object,
            ..inst(1, Vec3::ZERO, 1.0)
        };
        let instances: Arc<[Instance]> = vec![
            inst(1, Vec3::ZERO, 1.0),
            with(4, 1),
            with(4, 2),
            with(6, 0),
            with(8, 9),
        ]
        .into();
        let objects: Arc<[ObjectInfo]> = vec![o(OUTDOORS), o(12), o(13)].into();
        let b = BlockObjects::new(instances, objects);
        assert_eq!(
            b.figure_cells.get(&4),
            Some(&12),
            "the first instance's object"
        );
        assert_eq!(b.figure_cells.get(&6), Some(&OUTDOORS));
        assert_eq!(b.figure_cells.get(&8), None, "an object index out of range");
        let mut layer_blocks = HashMap::new();
        layer_blocks.insert(key, b);
        let lookup = |tag: u64| -> Option<(LandblockKey, u16)> {
            let (k, i) = super::super::figures::tag_parts(tag)?;
            Some((k, *layer_blocks.get(&k)?.figure_cells.get(&i)?))
        };
        assert_eq!(lookup(figure_tag(key, 4)), Some((key, 12)));
        assert_eq!(lookup(figure_tag(key, 5)), None);
    }

    #[test]
    fn bounds_cover_the_spheres_of_uploaded_parts() {
        let instances = [
            inst(1, Vec3::new(10.0, 20.0, 5.0), 2.0),
            inst(2, Vec3::new(-300.0, 0.0, 0.0), 1.0),
            inst(3, Vec3::new(1.0e6, 0.0, 0.0), 1.0),
        ];
        // Part 1: sphere at (0, 0, 10) radius 3 (scaled ×2); part 2 not uploaded; part 3
        // has a non-finite sphere.
        let sphere = |p: u32| match p {
            1 => Some((Vec3::new(0.0, 0.0, 10.0), 3.0)),
            3 => Some((Vec3::NAN, 1.0)),
            _ => None,
        };
        let (lo, hi) = instance_bounds(&instances, sphere).expect("one sphere");
        assert_eq!(lo, Vec3::new(4.0, 14.0, 19.0));
        assert_eq!(hi, Vec3::new(16.0, 26.0, 31.0));
        assert_eq!(instance_bounds(&instances, |_| None), None);
    }

    /// The review case: a tall part whose instance origin lies outside its block (local
    /// y = 409) and whose top is far above the origin. The old fixed box (x/y −80..240,
    /// z up to origin + 200) was out of view while the part's top was in view.
    #[test]
    fn tall_parts_outside_their_block_stay_visible() {
        let instances = [inst(7, Vec3::new(66.96, 409.17, 400.0), 1.0)];
        let sphere = |_| Some((Vec3::new(0.0, 0.0, 400.0), 30.0));
        let (lo, hi) = instance_bounds(&instances, sphere).unwrap();
        let cam = Camera {
            eye: Vec3::new(66.96, 300.0, 800.0),
            target: Vec3::new(66.96, 409.0, 800.0),
            up: Vec3::Z,
            projection: Perspective {
                fov_y: 1.0,
                near: 0.5,
                far: 4_000.0,
            },
        };
        let f = Frustum::from_view_proj(cam.view_proj(16.0 / 9.0));
        let top = Vec3::new(66.96, 409.17, 800.0);
        assert!(f.sphere_visible(top, 1.0));
        let old = (
            Vec3::new(-80.0, -80.0, 400.0 - 80.0),
            Vec3::new(240.0, 240.0, 400.0 + 200.0),
        );
        assert!(!f.aabb_visible(old.0, old.1), "the old box hid it");
        assert!(f.aabb_visible(lo, hi));
    }

    #[test]
    fn enclosing_sphere_covers_its_members() {
        assert_eq!(enclosing_sphere(std::iter::empty()), None);
        let s = [
            (Vec3::new(0.0, 0.0, 0.0), 1.0),
            (Vec3::new(10.0, 0.0, 0.0), 2.0),
            (Vec3::NAN, 5.0),
        ];
        let (c, r) = enclosing_sphere(s.into_iter()).unwrap();
        for (sc, sr) in &s[..2] {
            assert!((*sc - c).length() + sr <= r + 1e-4);
        }
        assert!(c.is_finite() && r.is_finite());
    }

    #[test]
    fn draw_distance_follows_the_class_and_the_cap() {
        let lod = LodValues([300.0, 100.0, 16.0, 80.0, 1000.0, 1000.0, 1100.0, -1.0]);
        let mut f = ObjectFrame {
            g: 2_080.0,
            ..ObjectFrame::default()
        };
        assert_eq!(object_draw_distance(&lod, &f, 5_000.0), 300.0);
        f.g = 120.0;
        assert_eq!(object_draw_distance(&lod, &f, 5_000.0), 120.0);
        assert_eq!(object_draw_distance(&lod, &f, 50.0), 50.0);
        f.lod = false;
        assert_eq!(object_draw_distance(&lod, &f, 900.0), 900.0);
        // No class anywhere: the component's initial values (walker.md §11.4 step 4).
        f.lod = true;
        f.g = 2_080.0;
        assert_eq!(
            object_draw_distance(&LodValues::default(), &f, 5_000.0),
            200.0
        );
    }

    #[test]
    fn faded_surfaces_blend_with_depth_write() {
        assert_eq!(faded_alpha(AlphaMode::Opaque), AlphaMode::BLEND_DEPTH_WRITE);
        assert_eq!(
            faded_alpha(AlphaMode::Mask { cutoff: 0.3 }),
            AlphaMode::TestBlend { cutoff: 0.3 }
        );
        assert_eq!(faded_alpha(AlphaMode::Additive), AlphaMode::Additive);
        assert_eq!(faded_alpha(AlphaMode::Blend), AlphaMode::Blend);
    }

    #[test]
    fn stored_colours_invert_the_linear_conversion() {
        for v in [0.0_f32, 0.001, 0.02, 0.2, 0.5, 0.9, 1.0] {
            let lin = super::super::geometry::srgb_unit_to_linear(v);
            assert!((linear_to_stored(lin) - v).abs() < 1e-4, "{v}");
        }
        assert_eq!(linear_to_stored(f32::NAN), 0.0);
        assert!((linear_to_stored(9.0) - 1.0).abs() < 1e-6);
    }

    #[test]
    fn fades_restart_after_a_frame_without_an_update() {
        assert!(fade_resets(0, 1), "never updated");
        assert!(!fade_resets(4, 5), "updated in the frame before");
        assert!(
            fade_resets(4, 6),
            "skipped a frame (culled, not reached, not drawn)"
        );
        assert!(fade_resets(u64::MAX, 1));
    }

    #[test]
    fn surface_draws_wait_for_their_base_texture() {
        let tex = TexRequest {
            did: 0x4000_0001,
            variant: VARIANT_OWN_ALPHA,
        };
        let mut plan = MaterialPlan::FALLBACK;
        plan.texture = Some(tex);
        plan.alpha = AlphaKind::Mask(0.5);
        plan.lit.alpha = AlphaMode::Mask { cutoff: 0.5 };
        plan.lit.opacity_from_base = true;
        let pending = |_| TexLookup::Pending;
        assert!(surface_draw(&plan, &pending, true, 1.0, true).is_none());
        let failed = |_| TexLookup::Failed;
        assert!(
            surface_draw(&plan, &failed, true, 1.0, true).is_none(),
            "cut-out without texture"
        );
        // A real texture id needs a renderer.
        let Ok(mut r) =
            tessera_render::Renderer::new_headless(&tessera_render::RendererConfig::default())
        else {
            eprintln!("skipping: no GPU");
            return;
        };
        let id = r
            .upload_texture(&TextureUpload::rgba8(1, 1, &[255, 255, 255, 255]))
            .expect("texture");
        let ready = |_| TexLookup::Ready(id);
        let (m, l) = surface_draw(&plan, &ready, true, 1.0, true).unwrap();
        assert_eq!(m.alpha, AlphaMode::Mask { cutoff: 0.5 });
        assert_eq!(l.opacity, Opacity::BaseAlpha);
        assert!(l.cast_shadow);
        // Fading: translucent, no shadow at or below opacity 0.8 (`mesh.md` §8.8), alpha
        // scaled.
        let (m, l) = surface_draw(&plan, &ready, true, 0.25, true).unwrap();
        assert_eq!(m.alpha, AlphaMode::TestBlend { cutoff: 0.5 });
        assert!((m.base_color[3] - 0.25 * plan.lit.color[3]).abs() < 1e-6);
        assert!(!l.cast_shadow);
        let (_, l) = surface_draw(&plan, &ready, true, SHADOW_MIN_OPACITY, true).unwrap();
        assert!(!l.cast_shadow, "0.8 itself does not cast");
        let (m, l) = surface_draw(&plan, &ready, true, 0.85, true).unwrap();
        assert_eq!(m.alpha, AlphaMode::TestBlend { cutoff: 0.5 });
        assert!(l.cast_shadow, "above 0.8 a fading object casts");
        // The M2 path keeps the linear plan colour.
        let (m, _) = surface_draw(&plan, &ready, false, 1.0, true).unwrap();
        assert_eq!(m.base_color, plan.base_color);
        // An opaque surface with a failed texture draws untextured.
        let mut opaque = plan;
        opaque.alpha = AlphaKind::Opaque;
        opaque.lit.alpha = AlphaMode::Opaque;
        let (m, l) = surface_draw(&opaque, &failed, true, 1.0, false).unwrap();
        assert_eq!(m.base_texture, None);
        assert_eq!(l.opacity, Opacity::One);
        assert!(!l.cast_shadow);
    }

    #[test]
    fn mean_colour_of_images() {
        let img = RgbaImage {
            width: 2,
            height: 1,
            pixels: vec![255, 0, 0, 255, 0, 0, 255, 255],
        };
        let m = mean_rgb(&img);
        assert!((m[0] - 0.5).abs() < 1e-6 && m[1] == 0.0 && (m[2] - 0.5).abs() < 1e-6);
        let bad = RgbaImage {
            width: 3,
            height: 3,
            pixels: vec![1; 4],
        };
        assert_eq!(mean_rgb(&bad), [1.0; 3]);
    }

    #[test]
    fn sorted_lists_intersect() {
        assert!(sorted_intersect(&[1, 4, 9], &[2, 3, 9]));
        assert!(!sorted_intersect(&[1, 4, 9], &[2, 3, 10]));
        assert!(!sorted_intersect(&[], &[1]));
    }

    #[test]
    fn object_parts_stay_in_range() {
        let instances = [inst(1, Vec3::ZERO, 1.0), inst(2, Vec3::ZERO, 1.0)];
        let o = |first, count| ObjectInfo {
            position: Vec3::ZERO,
            lod: LodValues::default(),
            generated: false,
            first,
            count,
            cell: OUTDOORS,
        };
        assert_eq!(
            object_parts(&instances, &o(0, 2)).collect::<Vec<_>>(),
            [1, 2]
        );
        assert_eq!(object_parts(&instances, &o(1, 9)).collect::<Vec<_>>(), [2]);
        assert_eq!(object_parts(&instances, &o(5, 1)).count(), 0);
        assert_eq!(object_parts(&instances, &o(u32::MAX, u32::MAX)).count(), 0);
    }

    /// One landblock (region 1, block 10, 10) with a static object (part 1) and a generated
    /// one (part 2), both 100 m east of the block's south-west corner, `d0` 300.
    fn one_block() -> HashMap<LandblockKey, BlockObjects> {
        let instances: Arc<[Instance]> = vec![
            inst(1, Vec3::new(100.0, 0.0, 0.0), 1.0),
            inst(2, Vec3::new(100.0, 0.0, 0.0), 1.0),
        ]
        .into();
        let lod = LodValues([300.0, 100.0, 16.0, 80.0, 1000.0, 1000.0, 1100.0, -1.0]);
        let o = |first, generated| ObjectInfo {
            position: Vec3::new(100.0, 0.0, 0.0),
            lod,
            generated,
            first,
            count: 1,
            cell: OUTDOORS,
        };
        let objects: Arc<[ObjectInfo]> = vec![o(0, false), o(1, true)].into();
        let key = LandblockKey::new(1, 10, 10).unwrap();
        HashMap::from([(key, BlockObjects::new(instances, objects))])
    }

    fn pass(
        blocks: &mut HashMap<LandblockKey, BlockObjects>,
        camera: [f64; 3],
        frame: &ObjectFrame,
        cap: f32,
    ) -> (Vec<u32>, Vec<u32>) {
        let mut order = Vec::new();
        let (a, r) = activate_blocks(blocks, &mut order, camera, frame, cap, 200.0);
        (a.into_iter().map(|x| x.0).collect(), r)
    }

    #[test]
    fn activation_reruns_when_the_cap_or_a_switch_changes_with_the_camera_still() {
        let mut blocks = one_block();
        // 250 m from both objects (draw distance 300 m, released beyond D + 140 m).
        let camera = [1_450.0, 1_600.0, 0.0];
        let frame = ObjectFrame::default();
        let (acq, rel) = pass(&mut blocks, camera, &frame, 2_080.0);
        assert_eq!((acq, rel), (vec![1, 2], vec![]));
        // Nothing changed: no work.
        assert_eq!(pass(&mut blocks, camera, &frame, 2_080.0), (vec![], vec![]));
        // The GPU cap shrinks with the camera still: the objects are released once they
        // lie beyond the cap + 140 m (review finding: the cap did not mark the block
        // stale).
        let (acq, mut rel) = pass(&mut blocks, camera, &frame, 160.0);
        assert!(acq.is_empty());
        assert_eq!(rel, Vec::<u32>::new(), "250 m is still within 160 + 140 m");
        (_, rel) = pass(&mut blocks, camera, &frame, 100.0);
        rel.sort_unstable();
        assert_eq!(rel, vec![1, 2]);
        // It grows back: both return.
        let (mut acq, _) = pass(&mut blocks, camera, &frame, 2_080.0);
        acq.sort_unstable();
        assert_eq!(acq, vec![1, 2]);
        // Scenery off (F9) releases the generated object at once; on brings it back.
        let off = ObjectFrame {
            scenery: false,
            ..frame
        };
        assert_eq!(pass(&mut blocks, camera, &off, 2_080.0), (vec![], vec![2]));
        assert_eq!(
            pass(&mut blocks, camera, &frame, 2_080.0),
            (vec![2], vec![])
        );
        // LOD off (F11) and other settings rerun the pass too.
        let mut far = ObjectFrame {
            lod: false,
            ..frame
        };
        let k0 = ActivationKey::new(&frame, 2_080.0);
        assert!(activation_stale(
            Some((camera, k0)),
            camera,
            &ActivationKey::new(&far, 2_080.0)
        ));
        far.lod = true;
        far.settings.dial_back = !far.settings.dial_back;
        assert!(activation_stale(
            Some((camera, k0)),
            camera,
            &ActivationKey::new(&far, 2_080.0)
        ));
        assert!(!activation_stale(
            Some((camera, k0)),
            [1_455.0, 1_600.0, 0.0],
            &k0
        ));
        assert!(activation_stale(
            Some((camera, k0)),
            [1_463.0, 1_600.0, 0.0],
            &k0
        ));
        assert!(activation_stale(None, camera, &k0));
    }

    /// Review finding (objects.rs, `walker.md` §14.5 OPEN (b)): the smoothed distance of
    /// an object in a culled block follows the camera, so it does not come back stale; an
    /// object not in the scene (a generated one with `scenery` off) is left alone.
    #[test]
    fn culled_objects_keep_their_smoothed_distance_current() {
        let mut blocks = one_block();
        let key = LandblockKey::new(1, 10, 10).unwrap();
        let b = blocks.get_mut(&key).unwrap();
        for st in &mut b.state {
            st.active = true;
            st.sphere = Some((Vec3::new(100.0, 0.0, 0.0), 1.0));
        }
        let frame = ObjectFrame {
            scenery: false,
            fps: 60.0,
            ..ObjectFrame::default()
        };
        let all = |_: LandblockKey, _: u16| true;
        b.follow_distances(
            key,
            Vec3::new(1_101.0, 0.0, 0.0),
            &frame,
            &all,
            0,
            (1, 5_000.0),
        );
        assert_eq!(b.state[0].smooth.value(), Some(1_000.0), "first frame: d");
        assert_eq!(b.state[1].smooth.value(), None, "not in the scene");
        for f in 0..600 {
            b.follow_distances(
                key,
                Vec3::new(111.0, 0.0, 0.0),
                &frame,
                &all,
                0,
                (2 + f, 5_000.0),
            );
        }
        let ds = b.state[0].smooth.value().unwrap();
        assert!(
            (ds - 10.0).abs() < 0.01,
            "followed the camera to 10 m: {ds}"
        );
        let none = |_: LandblockKey, _: u16| false;
        b.follow_distances(
            key,
            Vec3::new(1_101.0, 0.0, 0.0),
            &frame,
            &none,
            0,
            (602, 5_000.0),
        );
        assert_eq!(
            b.state[0].smooth.value(),
            Some(ds),
            "a cell the camera does not see"
        );
    }

    /// Review finding (objects.rs, `walker.md` §14.5 OPEN (b)): when the draw cap stops
    /// the frame at an object, the objects after it in its block still have their smoothed
    /// distance updated; the ones before it (already processed by the draw loop) are not
    /// updated twice.
    #[test]
    fn objects_after_the_draw_cap_keep_their_smoothed_distance_current() {
        let mut blocks = one_block();
        let key = LandblockKey::new(1, 10, 10).unwrap();
        let b = blocks.get_mut(&key).unwrap();
        for st in &mut b.state {
            st.active = true;
            st.sphere = Some((Vec3::new(100.0, 0.0, 0.0), 1.0));
        }
        let frame = ObjectFrame {
            fps: 60.0,
            ..ObjectFrame::default()
        };
        let all = |_: LandblockKey, _: u16| true;
        // The cap was reached at object 0: only object 1 is followed here.
        b.follow_distances(
            key,
            Vec3::new(1_101.0, 0.0, 0.0),
            &frame,
            &all,
            1,
            (1, 5_000.0),
        );
        assert_eq!(
            b.state[0].smooth.value(),
            None,
            "processed by the draw loop"
        );
        assert_eq!(b.state[1].smooth.value(), Some(1_000.0), "after the cap");
        // An index past the end is harmless.
        b.follow_distances(
            key,
            Vec3::new(1_101.0, 0.0, 0.0),
            &frame,
            &all,
            usize::MAX,
            (2, 5_000.0),
        );
        assert_eq!(b.state[0].smooth.value(), None);
    }

    /// `walker.md` §14.5 row W-O3: the fade of an object in the scene advances in every
    /// frame, also while its block is out of view, so it neither pauses nor restarts there;
    /// it restarts only in an object's first frame and after a camera jump of more than
    /// 25 m.
    #[test]
    fn fades_advance_for_objects_out_of_view() {
        let mut blocks = one_block();
        let key = LandblockKey::new(1, 10, 10).unwrap();
        let b = blocks.get_mut(&key).unwrap();
        for st in &mut b.state {
            st.active = true;
            st.sphere = Some((Vec3::new(100.0, 0.0, 0.0), 1.0));
        }
        let mut frame = ObjectFrame {
            fps: 60.0,
            dt: 0.1,
            lod: true,
            g: 2_080.0,
            ..ObjectFrame::default()
        };
        let all = |_: LandblockKey, _: u16| true;
        let cap = 50.0;
        // First frame, within the draw distance: shown at once.
        b.follow_distances(key, Vec3::new(110.0, 0.0, 0.0), &frame, &all, 0, (1, cap));
        assert_eq!(b.state[0].fade.opacity(), 1.0);
        // The camera walks away (out of reach): the fade runs on frame by frame while the
        // block is out of view, 0.1 s / 1.5 s per frame.
        for f in 0..5 {
            b.follow_distances(
                key,
                Vec3::new(300.0, 0.0, 0.0),
                &frame,
                &all,
                0,
                (2 + f, cap),
            );
        }
        let o = b.state[0].fade.opacity();
        assert!((o - (1.0 - 5.0 * 0.1 / 1.5)).abs() < 1e-4, "{o}");
        // A camera jump of more than 25 m restarts it.
        frame.camera_moved = 30.0;
        b.follow_distances(key, Vec3::new(300.0, 0.0, 0.0), &frame, &all, 0, (7, cap));
        assert_eq!(b.state[0].fade.opacity(), 0.0);
        // A frame without the step (the object out of the scene) restarts it on return.
        frame.camera_moved = 0.0;
        b.follow_distances(key, Vec3::new(110.0, 0.0, 0.0), &frame, &all, 0, (9, cap));
        assert_eq!(b.state[0].fade.opacity(), 1.0);
    }

    #[test]
    fn interior_objects_load_only_from_inside_their_landblock() {
        let instances: Arc<[Instance]> = vec![
            inst(1, Vec3::new(100.0, 0.0, 0.0), 1.0),
            inst(3, Vec3::new(-300.0, 0.0, -200.0), 1.0),
        ]
        .into();
        let o = |first, position, cell| ObjectInfo {
            position,
            lod: LodValues::default(),
            generated: false,
            first,
            count: 1,
            cell,
        };
        // An outdoor static and a cell mesh 300 m west of the block, 200 m down.
        let objects: Arc<[ObjectInfo]> = vec![
            o(0, Vec3::new(100.0, 0.0, 0.0), OUTDOORS),
            o(1, Vec3::new(-300.0, 0.0, -200.0), 9),
        ]
        .into();
        let key = LandblockKey::new(1, 10, 10).unwrap();
        let mut blocks = HashMap::from([(key, BlockObjects::new(instances, objects))]);
        let b = blocks.get(&key).unwrap();
        assert_eq!(b.rect, ([100.0, 0.0], [100.0, 0.0]), "outdoor objects only");
        let o = key.origin();
        let camera = [o[0] - 300.0, o[1], -198.0];
        let outside = ObjectFrame::default();
        assert_eq!(
            pass(&mut blocks, camera, &outside, 2_080.0).0,
            Vec::<u32>::new()
        );
        let inside = ObjectFrame {
            interior: Some(key),
            ..outside
        };
        assert_eq!(
            pass(&mut blocks, camera, &inside, 2_080.0),
            (vec![3], vec![])
        );
        let elsewhere = ObjectFrame {
            interior: LandblockKey::new(1, 10, 11).ok(),
            ..outside
        };
        assert_eq!(
            pass(&mut blocks, camera, &elsewhere, 2_080.0),
            (vec![], vec![3])
        );
        let debug = ObjectFrame {
            all_interiors: true,
            ..outside
        };
        assert_eq!(
            pass(&mut blocks, camera, &debug, 2_080.0),
            (vec![3], vec![])
        );
        assert!(interior_eligible(key, OUTDOORS, &outside));
        assert!(!interior_eligible(key, 9, &outside));
    }
}
