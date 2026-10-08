// SPDX-License-Identifier: GPL-3.0-or-later
//! Terrain of the resident landblocks (`docs/specs/terrain.md`, `terrain-texturing.md`):
//! per block a [`TerrainBlock`] built on a worker, a level of detail chosen per frame by
//! screen-space error, meshes stitched to the neighbours' levels and normals shared across
//! seams, both built on workers. The main thread only selects levels, queues jobs and
//! uploads finished products within the frame's upload budget.
//!
//! # Texture layers (M4.5)
//!
//! With layers on, a block's surface layers are resolved against its region's terrain type
//! table (prefix 0x19) on the block job ([`tessera_terrain::resolve_landblock`]), which also
//! reads and checks the block's A8 masks and decodes its light maps. A mesh job then builds
//! the chunk's vertices (the four coordinate sets and the tangent frame, `terrain-texturing.md`
//! §6) and, for the block's level and stitched edges, every layer's triangles
//! ([`tessera_terrain::ChunkLayers::layer_indices_for`]); the main thread uploads it as a
//! [`TerrainChunk`] for the layer pipeline (`docs/specs/render.md` §5.2). The layer textures
//! (diffuse, normal, emissive, glow) are shared through the object layer's texture store;
//! masks and light maps belong to their block. A chunk waits for its textures; a block that
//! draws nothing yet uploads at once with what is ready and is rebuilt when the rest
//! arrives. A cave block's ceiling chunk is drawn at level 0 (`terrain.md` §7 Decision 3).
//! Blocks whose layers cannot be resolved, and every block with layers off, draw the M3
//! height-and-slope mesh; a terrain chunk that does not resolve keeps the block's ceiling
//! (its own layers and §4.9 flag). With layers on, a chunk hidden by
//! `terrain-texturing.md` §4.9 rule 3 is left out entirely: no mesh, no layered chunk, no
//! draw. In the far band (`walker.md` §14.4) a block keeps only its light map on the GPU:
//! its terrain masks wait on the CPU until it nears, and masks uploaded while it was near
//! are released again once its baked chunk is drawn (kept as A8 for the next upload).
//!
//! # Crack-free swaps
//!
//! Stitching (`terrain.md` §4) is crack-free only when both blocks of a seam are drawn with
//! meshes built for the same edge level. Meshes finish and upload one block at a time, so
//! a new mesh is first **staged** (uploaded, not drawn) and only replaces the drawn one in
//! a frame where every drawn neighbour agrees on the shared edge level, swapping the
//! neighbours' staged meshes in the same frame when needed ([`promotable`]).
//!
//! # Failures
//!
//! A mesh that fails to build or upload is not retried for the same signature until a
//! back-off has passed (doubling up to [`MAX_BACKOFF`]); a block job that fails (its
//! decoder panicked) leaves the block without terrain. Both count as settled, so a
//! screenshot or flight does not wait for them, and both show in [`TerrainStats::failed`].
//!
//! The same block job also turns the bundle's drawable static entities and generated
//! scenery into objects ([`ObjectInfo`], [`Instance`]) with their LOD-class values, and
//! renders a small height-shaded overview tile for the minimap fallback, so no per-entity
//! work happens on the main thread.

use std::collections::{HashMap, HashSet};
use std::panic::AssertUnwindSafe;
use std::sync::{Arc, PoisonError, RwLock};
use std::time::{Duration, Instant};

use tessera_formats::landblock::{
    GRID_SIDE, GridSurface, LandblockData, LightMaps, SurfaceChunk, SurfaceLayer,
};
use tessera_formats::terrain_type::{TerrainTypeTable, TextureProjection};
use tessera_formats::{Did, Record, RgbaImage, Surface};
use tessera_material::{CurveState, FloatCurve};
use tessera_render::glam::{Mat4, Quat, Vec3};
use tessera_render::{
    Indices, LightSelection, LitSurface, Material, MeshConfig, MeshData, MeshDraw, MeshId, Pass,
    PreparedMesh, Renderer, Submesh, TerrainChunk, TerrainChunkData, TerrainDraw, TerrainGloss,
    TerrainLayer as GpuLayer, TerrainLayerTextures, TerrainOpacity, TerrainProjection,
    TerrainSpecular, TerrainTransform, TerrainVertices, TextureId, TextureUpload, Wrap,
};
use tessera_terrain::{
    AlphaMask, ChunkLayers, CoordinateSet, EdgeLods, HeightSlopeShading, LayerVertices, Lod,
    LodSelector, Neighbours, OpacityRule, ResolveOptions, Side, TerrainBlock, TerrainMesh,
    TerrainTiles, VertexNormals, fixed_block_offset, resolve_chunk, resolve_landblock, triangulate,
};
use tessera_world::{
    LANDBLOCK_SIZE, LandblockBundle, LandblockKey, LodClasses, LodValues, WorldContext, WorldSource,
};

use super::UploadBudget;
use super::env::material_curve;
use super::farband::{BAKE_SIZE, SmallChain, bake, mip_chain, reduce_mask};
use super::geometry::{TexRequest, VARIANT_OWN_ALPHA, image_ok};
use super::interiors;
use super::jobs::JobRunner;
use super::objects::{Instance, ObjectInfo, ObjectLayer, TexLookup};
use super::view::{Anchor, Frustum};
use crate::smoke::linear_to_srgb8;

/// Edge of the overview tile in pixels (one per height sample).
pub const OVERVIEW_EDGE: u32 = 33;
/// Most mesh jobs queued per frame.
const MESH_JOBS_PER_FRAME: usize = 48;
/// Queue capacity of the terrain workers.
const QUEUE_CAPACITY: usize = 512;
/// First retry delay of a failed mesh.
pub const FIRST_BACKOFF: Duration = Duration::from_secs(1);
/// Longest retry delay of a failed mesh.
pub const MAX_BACKOFF: Duration = Duration::from_secs(30);
/// Largest mask or light-map edge read by a block job (texels).
pub const MAX_BLOCK_IMAGE_EDGE: u32 = 1024;

/// Resident terrain heights of one region, shared between the terrain layer (the only
/// writer) and its readers (the movement controllers, the sandbox's world source), so
/// there is exactly one copy.
#[derive(Clone, Default)]
pub struct SharedTiles(Arc<RwLock<RegionTiles>>);

#[derive(Default)]
struct RegionTiles {
    region: Option<u8>,
    tiles: TerrainTiles,
}

impl std::fmt::Debug for SharedTiles {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        let t = self.read();
        f.debug_struct("SharedTiles")
            .field("region", &t.region)
            .field("blocks", &t.tiles.len())
            .finish()
    }
}

impl SharedTiles {
    fn read(&self) -> std::sync::RwLockReadGuard<'_, RegionTiles> {
        self.0.read().unwrap_or_else(PoisonError::into_inner)
    }

    fn write(&self) -> std::sync::RwLockWriteGuard<'_, RegionTiles> {
        self.0.write().unwrap_or_else(PoisonError::into_inner)
    }

    /// Terrain height at region-global (`gx`, `gy`) of the region held, `None` where no
    /// resident block covers the point.
    #[must_use]
    pub fn height_at(&self, gx: f64, gy: f64) -> Option<f32> {
        self.read().tiles.height_at(gx, gy)
    }

    /// As [`SharedTiles::height_at`], `None` unless the tiles hold `region`.
    #[must_use]
    pub fn height_in(&self, region: u8, gx: f64, gy: f64) -> Option<f32> {
        let t = self.read();
        if t.region == Some(region) {
            t.tiles.height_at(gx, gy)
        } else {
            None
        }
    }

    /// The region held.
    #[must_use]
    pub fn region(&self) -> Option<u8> {
        self.read().region
    }

    /// Blocks held.
    #[must_use]
    pub fn len(&self) -> usize {
        self.read().tiles.len()
    }

    /// Whether no block is held.
    #[must_use]
    pub fn is_empty(&self) -> bool {
        self.len() == 0
    }

    /// Switches to `region`, dropping every block of another one.
    pub fn set_region(&self, region: u8) {
        let mut t = self.write();
        if t.region != Some(region) {
            t.tiles = TerrainTiles::new();
            t.region = Some(region);
        }
    }

    /// Adds `key`'s block when the tiles hold `key`'s region (see
    /// [`SharedTiles::set_region`]). Returns whether it was added.
    pub fn insert(&self, key: LandblockKey, block: Arc<TerrainBlock>) -> bool {
        let mut t = self.write();
        t.region == Some(key.region()) && t.tiles.insert(key.x(), key.y(), block).is_ok()
    }

    /// Drops `key`'s block.
    pub fn remove(&self, key: LandblockKey) {
        let mut t = self.write();
        if t.region == Some(key.region()) {
            t.tiles.remove(key.x(), key.y());
        }
    }

    /// Drops every block.
    pub fn clear(&self) {
        let mut t = self.write();
        t.tiles = TerrainTiles::new();
    }
}

/// What the block jobs read besides the bundle: the region terrain type tables (0x19),
/// the LOD-class table and the record source for masks and light maps.
pub struct BlockInputs {
    /// Record source of masks and light maps (`None`: blocks get no images).
    pub source: Option<Arc<dyn WorldSource>>,
    /// Terrain type table per region.
    pub tables: HashMap<u8, Arc<TerrainTypeTable>>,
    /// The LOD-class table (`walker.md` §11.4).
    pub lod_classes: Option<Arc<LodClasses>>,
}

impl std::fmt::Debug for BlockInputs {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("BlockInputs")
            .field("tables", &self.tables.keys().collect::<Vec<_>>())
            .field("lod_classes", &self.lod_classes.is_some())
            .finish_non_exhaustive()
    }
}

impl BlockInputs {
    /// The inputs of `ctx`: the tables its regions' scenery loaded, else read from the
    /// region descriptors (`environment.md` §3, slot 8); the LOD-class table of its world
    /// systems.
    #[must_use]
    pub fn from_context(ctx: &WorldContext) -> Self {
        let mut tables = HashMap::new();
        for (r, info) in ctx.regions() {
            if let Some(t) = info.scenery.as_ref().and_then(|s| s.terrain_types.clone()) {
                tables.insert(*r, t);
                continue;
            }
            let Some(did) = info
                .descriptor
                .as_ref()
                .and_then(|d| d.terrain_type_table())
            else {
                continue;
            };
            match ctx.source().read(did) {
                Ok(Some(bytes)) => match TerrainTypeTable::parse_expecting(&bytes, did) {
                    Ok(t) => {
                        tables.insert(*r, Arc::new(t));
                    }
                    Err(e) => tracing::warn!(region = r, %did, "terrain type table: {e}"),
                },
                Ok(None) => tracing::warn!(region = r, %did, "terrain type table missing"),
                Err(e) => tracing::warn!(region = r, %did, "terrain type table unreadable: {e}"),
            }
        }
        let lod_classes = ctx.systems().lod_classes.clone();
        Self {
            source: Some(Arc::clone(ctx.source())),
            tables,
            lod_classes,
        }
    }
}

/// The shared texture store the terrain's layer textures live in (the object layer's,
/// [`ObjectLayer`]), behind a trait so the layer can be tested without archives.
pub trait TextureStore {
    /// Adds a reference to each texture of `reqs` (loaded and kept until released).
    fn acquire(&mut self, reqs: &[TexRequest]);
    /// The small levels of `t` kept on the CPU, once on the GPU.
    fn small(&self, t: TexRequest) -> Option<Arc<[RgbaImage]>>;
    /// Drops the references taken with [`TextureStore::acquire`].
    fn release(&mut self, reqs: &[TexRequest]);
    /// Where texture `t` stands.
    fn lookup(&self, t: TexRequest) -> TexLookup;
    /// The mean colour of `t`'s smallest level, once on the GPU.
    fn mean(&self, t: TexRequest) -> Option<[f32; 3]>;
}

impl TextureStore for ObjectLayer {
    fn acquire(&mut self, reqs: &[TexRequest]) {
        self.acquire_textures(reqs.iter().copied());
    }

    fn release(&mut self, reqs: &[TexRequest]) {
        self.release_textures(reqs.iter().copied());
    }

    fn lookup(&self, t: TexRequest) -> TexLookup {
        self.texture(t)
    }

    fn mean(&self, t: TexRequest) -> Option<[f32; 3]> {
        self.texture_mean(t)
    }

    fn small(&self, t: TexRequest) -> Option<Arc<[RgbaImage]>> {
        self.texture_small(t)
    }
}

/// A block image waiting for its upload: a light map (RGBA8) or a layer mask, kept as its
/// A8 texels (a quarter of the RGBA size, since a far block keeps its masks until it
/// comes near) and expanded to (255, 255, 255, *a*) at upload, as the surface decoder
/// expands A8.
#[derive(Debug, Clone, PartialEq)]
pub enum BlockImage {
    /// RGBA8 texels.
    Rgba(RgbaImage),
    /// A8 texels, width × height bytes.
    Mask {
        /// Width.
        width: u32,
        /// Height.
        height: u32,
        /// Alpha per texel.
        alpha: Vec<u8>,
    },
}

impl BlockImage {
    /// The image as RGBA8 (masks expanded to white with their alpha); `None` for a mask
    /// whose texel count is not width × height.
    #[must_use]
    pub fn to_rgba(&self) -> Option<std::borrow::Cow<'_, RgbaImage>> {
        match self {
            Self::Rgba(img) => Some(std::borrow::Cow::Borrowed(img)),
            Self::Mask {
                width,
                height,
                alpha,
            } => {
                let n = (*width as usize).checked_mul(*height as usize)?;
                (n == alpha.len() && n > 0).then(|| {
                    std::borrow::Cow::Owned(RgbaImage {
                        width: *width,
                        height: *height,
                        pixels: alpha.iter().flat_map(|&a| [255, 255, 255, a]).collect(),
                    })
                })
            }
        }
    }

    /// Bytes of the uploaded RGBA8 texture.
    #[must_use]
    pub fn upload_bytes(&self) -> u64 {
        match self {
            Self::Rgba(img) => img.pixels.len() as u64,
            Self::Mask { width, height, .. } => u64::from(*width) * u64::from(*height) * 4,
        }
    }
}

/// The resolved texture layers of a block (`terrain-texturing.md` §4).
#[derive(Debug, Clone)]
pub struct BlockLayers {
    /// The terrain chunk's layers.
    pub terrain: Arc<ChunkLayers>,
    /// The ceiling chunk's layers and surface, for a cave block.
    pub ceiling: Option<(Arc<ChunkLayers>, Arc<TerrainBlock>)>,
    /// The terrain chunk stores no layers and the block is not open: the original draws
    /// nothing for it (`terrain-texturing.md` §4.9 rule 3).
    pub terrain_hidden: bool,
    /// The same for the ceiling chunk.
    pub ceiling_hidden: bool,
}

/// Whether a block is open (`terrain-texturing.md` §4.9 rule 1): it has no ceiling, or at
/// some grid sample (*i*, *j*) with 0 ≤ *i*, *j* ≤ 31 the stored terrain height is strictly
/// below the stored ceiling height.
#[must_use]
pub fn block_open(data: &LandblockData) -> bool {
    let Some(c) = &data.ceiling else {
        return true;
    };
    (0..GRID_SIDE - 1).any(|i| {
        (0..GRID_SIDE - 1).any(|j| match (data.heights.get(i, j), c.heights.get(i, j)) {
            (Some(t), Some(h)) => t < h,
            _ => false,
        })
    })
}

/// The terrain-type key a chunk without layers is drawn with (`terrain-texturing.md` §4.9
/// rule 2): the table entry with the smallest `draw_order`, the one met last in stored
/// order on a tie; `None` for an empty table.
#[must_use]
pub fn layerless_key(table: &TerrainTypeTable) -> Option<u32> {
    let mut best: Option<&tessera_formats::terrain_type::TerrainType> = None;
    for e in table.entries() {
        if best.is_none_or(|b| e.draw_order <= b.draw_order) {
            best = Some(e);
        }
    }
    best.map(|e| e.key)
}

/// The layers of a surface chunk with §4.9 applied: a chunk that stores layers resolves
/// them; one without layers in an open block resolves to one opaque `whole` layer of
/// [`layerless_key`] without a mask (so it covers the full grid of its surface and takes
/// the surface's light map); one without layers in a block that is not open resolves to no
/// layer. Returns the layers and whether the chunk is layerless in a block that is not
/// open (§4.9 rule 3: not drawn). A layerless chunk of an open block whose table has no
/// entry resolves to no layer but is not rule 3: it is drawn like a region without a table
/// (`walker.md` §14.4 height-shaded fallback; Tessera Decision, OPEN there).
fn resolve_surface(
    data: &LandblockData,
    surface: GridSurface,
    table: &TerrainTypeTable,
    light_maps: &LightMaps,
    options: &ResolveOptions,
    open: bool,
) -> tessera_terrain::Result<(ChunkLayers, bool)> {
    let stored = data
        .surface_chunk(surface)
        .is_some_and(|c| !c.layers.is_empty());
    if stored {
        return resolve_landblock(data, surface, table, light_maps, options).map(|l| (l, false));
    }
    let key = layerless_key(table).filter(|_| open);
    let chunk = SurfaceChunk {
        layers: key
            .map(|material| SurfaceLayer {
                material,
                whole: true,
                ..SurfaceLayer::default()
            })
            .into_iter()
            .collect(),
        ..SurfaceChunk::default()
    };
    let layers = resolve_chunk(
        &chunk,
        &data.vertex_flags,
        surface,
        table,
        light_maps.for_surface(surface),
        options,
    )?;
    Ok((layers, !open))
}

/// The layers of a terrain chunk whose stored layers do not resolve: none, so that the
/// chunk is drawn like one none of whose layers resolves (`walker.md` §14.4 height-shaded
/// fallback) while the block's ceiling is still resolved on its own.
fn unresolved_terrain(data: &LandblockData, table: &TerrainTypeTable) -> Option<ChunkLayers> {
    resolve_chunk(
        &SurfaceChunk::default(),
        &data.vertex_flags,
        GridSurface::Terrain,
        table,
        None,
        &ResolveOptions::default(),
    )
    .ok()
}

/// The walker's §4.9 path for one surface of a block (`terrain-texturing.md` §4.9,
/// `walker.md` §14.4): [`block_open`], then the surface resolved as [`resolve_surface`]
/// does. Returns the layers and whether the chunk is not drawn (rule 3).
///
/// # Errors
///
/// When a stored layer does not resolve ([`tessera_terrain::resolve_landblock`]).
pub fn resolve_block_surface(
    data: &LandblockData,
    surface: GridSurface,
    table: &TerrainTypeTable,
    light_maps: &LightMaps,
    options: &ResolveOptions,
) -> tessera_terrain::Result<(ChunkLayers, bool)> {
    resolve_surface(data, surface, table, light_maps, options, block_open(data))
}

/// What the block job produces.
#[derive(Debug)]
pub struct BlockProduct {
    /// Terrain heights and LOD error bounds (computed here).
    pub block: Arc<TerrainBlock>,
    /// Height-shaded overview tile, RGBA8 sRGB, north row first.
    pub overview: Vec<u8>,
    /// Part instances of the drawable, outdoor objects.
    pub instances: Arc<[Instance]>,
    /// The drawable, outdoor objects (statics and generated scenery).
    pub objects: Arc<[ObjectInfo]>,
    /// The texture layers, when the region's table resolves them.
    pub layers: Option<BlockLayers>,
    /// The block's masks (kept A8) and light maps, decoded.
    pub images: Vec<(Did, BlockImage)>,
    /// The terrain chunk's masks reduced for the far-band bake ([`super::farband`]).
    pub bake_masks: HashMap<Did, AlphaMask>,
}

/// The signature of a built terrain mesh: own level, effective edge levels, which edge
/// neighbours the normals saw, and whether it is a layered chunk.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
struct MeshSig {
    lod: Lod,
    edges: [Lod; 4],
    present: u8,
    layered: bool,
    /// The far band: one baked layer (`walker.md` §14.4).
    far: bool,
}

/// A layered chunk built on a worker: its vertices and every drawn layer's triangles.
#[derive(Debug)]
pub struct ChunkPrep {
    /// The vertex streams (`terrain-texturing.md` §6).
    pub vertices: LayerVertices,
    /// Triangles of each drawn layer, in the order of [`ChunkLayers::layers`].
    pub triangles: Vec<Vec<[u16; 3]>>,
    /// Which surface it is.
    pub surface: GridSurface,
    /// A far-band chunk: one layer of every covered triangle.
    pub far: bool,
}

impl ChunkPrep {
    fn bytes(&self) -> u64 {
        let tris: usize = self.triangles.iter().map(Vec::len).sum();
        (self.vertices.positions.len() * 80 + tris * 6) as u64
    }
}

enum Job {
    Block {
        key: LandblockKey,
        generation: u64,
        bundle: Arc<LandblockBundle>,
        shading: Arc<HeightSlopeShading>,
        inputs: Arc<BlockInputs>,
    },
    Mesh {
        key: LandblockKey,
        generation: u64,
        sig: MeshSig,
        block: Arc<TerrainBlock>,
        neighbours: [Option<Arc<TerrainBlock>>; 4],
        shading: Arc<HeightSlopeShading>,
        config: MeshConfig,
        layers: Option<Arc<ChunkLayers>>,
        far: bool,
    },
    Ceiling {
        key: LandblockKey,
        generation: u64,
        ceiling: Arc<TerrainBlock>,
        layers: Arc<ChunkLayers>,
    },
    Bake {
        key: LandblockKey,
        generation: u64,
        block: Arc<TerrainBlock>,
        layers: Arc<ChunkLayers>,
        diffuse: HashMap<Did, SmallChain>,
        masks: Arc<HashMap<Did, AlphaMask>>,
    },
    /// The height-shaded mesh of a ceiling (`walker.md` §14.4 fallback, ceilings).
    ShadedCeiling {
        key: LandblockKey,
        generation: u64,
        bundle: Arc<LandblockBundle>,
        shading: Arc<HeightSlopeShading>,
        config: MeshConfig,
    },
}

/// A mesh job's product.
enum Built {
    Mesh(PreparedMesh),
    Chunk(Arc<ChunkPrep>),
}

enum Output {
    Block {
        key: LandblockKey,
        generation: u64,
        product: Box<BlockProduct>,
    },
    /// The block job failed (it panicked); the block gets no terrain.
    BlockFailed {
        key: LandblockKey,
        generation: u64,
        reason: String,
    },
    Mesh {
        key: LandblockKey,
        generation: u64,
        sig: MeshSig,
        mesh: Result<(Built, f32, f32), String>,
    },
    Ceiling {
        key: LandblockKey,
        generation: u64,
        prep: Option<Arc<ChunkPrep>>,
    },
    Bake {
        key: LandblockKey,
        generation: u64,
        chain: Result<Vec<RgbaImage>, String>,
    },
    ShadedCeiling {
        key: LandblockKey,
        generation: u64,
        mesh: Result<PreparedMesh, String>,
    },
}

/// The height-shaded mesh of a block's ceiling, drawn where its layered chunk is not
/// (`walker.md` §14.4; Tessera Decision, OPEN there): layers or lighting switched off, no
/// 0x19 table, or ceiling layers that do not resolve.
enum ShadedCeiling {
    /// Not asked for.
    None,
    /// Building on a worker.
    Pending,
    /// Built, waiting for upload budget.
    Waiting(PreparedMesh),
    /// On the GPU: mesh and bytes.
    Ready(MeshId, u64),
    /// The block has no ceiling, or the build or upload failed.
    Failed,
}

/// The far-band bake of a block.
enum Bake {
    /// Not asked for yet.
    None,
    /// Baking on a worker.
    Pending,
    /// Baked image waiting for upload budget.
    Waiting(Vec<RgbaImage>),
    /// On the GPU: texture and bytes.
    Ready(TextureId, u64),
    /// Failed (the block stays in the near band's form).
    Failed,
}

/// The last mesh attempt of a block failed.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
struct Failure {
    sig: MeshSig,
    retry_at: Instant,
    attempts: u32,
}

/// A drawn (or staged) terrain surface.
enum GpuKind {
    Mesh(MeshId),
    Chunk {
        chunk: TerrainChunk,
        /// Kept while some texture was missing at upload, to rebuild it.
        prep: Option<Arc<ChunkPrep>>,
    },
}

struct GpuTerrain {
    kind: GpuKind,
    sig: MeshSig,
    min_h: f32,
    max_h: f32,
    bytes: u64,
}

impl GpuTerrain {
    fn complete(&self) -> bool {
        !matches!(self.kind, GpuKind::Chunk { prep: Some(_), .. })
    }
}

/// A ceiling chunk (level 0 only).
struct Ceiling {
    prep: Option<Arc<ChunkPrep>>,
    gpu: Option<(TerrainChunk, bool)>,
    pending: bool,
}

struct Slot {
    bundle: Arc<LandblockBundle>,
    generation: u64,
    block: Option<Arc<TerrainBlock>>,
    /// The block job failed: no terrain for this bundle.
    block_failed: bool,
    lod: Option<Lod>,
    /// The mesh drawn.
    mesh: Option<GpuTerrain>,
    /// An uploaded mesh waiting for its neighbours to agree on the shared edges.
    staged: Option<GpuTerrain>,
    pending: Option<MeshSig>,
    /// A finished mesh waiting for upload budget (or textures).
    ready: Option<(MeshSig, Built, f32, f32)>,
    failed: Option<Failure>,
    layers: Option<BlockLayers>,
    /// Masks and light maps waiting for upload.
    images: Vec<(Did, BlockImage)>,
    /// Of `images`, how many at the end are needed only near (terrain masks, which a
    /// baked block does not draw with).
    near_only: usize,
    /// Masks and light maps on the GPU (owned by the block).
    own: HashMap<Did, (TextureId, u64)>,
    /// The near-only images (terrain masks) of `own`, kept on the CPU (A8) so that they
    /// can be released when the block goes back into the far band and uploaded again when
    /// it nears ([`Slot::park_masks`]).
    parked: Vec<(Did, BlockImage)>,
    /// Masks and light maps that could not be uploaded.
    own_failed: HashSet<Did>,
    /// Shared layer textures referenced in the object layer's store.
    shared: Vec<TexRequest>,
    ceiling: Option<Ceiling>,
    /// The ceiling's height-shaded fallback ([`Slot::wants_shaded_ceiling`]).
    shaded_ceiling: ShadedCeiling,
    /// Masks for the far-band bake, dropped once baked.
    bake_masks: Option<Arc<HashMap<Did, AlphaMask>>>,
    bake: Bake,
    /// The block is in the far band (with hysteresis).
    far: bool,
    /// Layers whose `u_offset` / `v_offset` vary with time (`terrain-texturing.md` §6.3):
    /// (layer index, the two curves with their states).
    scrolling: Vec<(usize, [(FloatCurve, CurveState); 2])>,
}

impl Slot {
    fn new(bundle: Arc<LandblockBundle>, generation: u64) -> Self {
        Self {
            bundle,
            generation,
            block: None,
            block_failed: false,
            lod: None,
            mesh: None,
            staged: None,
            pending: None,
            ready: None,
            failed: None,
            layers: None,
            images: Vec::new(),
            near_only: 0,
            own: HashMap::new(),
            parked: Vec::new(),
            own_failed: HashSet::new(),
            shared: Vec::new(),
            ceiling: None,
            shaded_ceiling: ShadedCeiling::None,
            bake_masks: None,
            bake: Bake::None,
            far: false,
            scrolling: Vec::new(),
        }
    }

    /// Whether a mesh job for `sig` should be queued at `now`.
    fn wants_mesh(&self, sig: MeshSig, now: Instant) -> bool {
        !(self.pending.is_some()
            || self.mesh.as_ref().is_some_and(|m| m.sig == sig)
            || self.staged.as_ref().is_some_and(|m| m.sig == sig)
            || self.ready.as_ref().is_some_and(|r| r.0 == sig)
            || self
                .failed
                .is_some_and(|f| f.sig == sig && now < f.retry_at))
    }

    /// Records a failed attempt for `sig`; returns the attempt number.
    fn fail(&mut self, sig: MeshSig, now: Instant) -> u32 {
        let attempts = self.failed.map_or(0, |f| f.attempts).saturating_add(1);
        let shift = attempts.saturating_sub(1).min(16);
        let backoff = FIRST_BACKOFF.saturating_mul(1u32 << shift).min(MAX_BACKOFF);
        self.failed = Some(Failure {
            sig,
            retry_at: now.checked_add(backoff).unwrap_or(now),
            attempts,
        });
        attempts
    }

    fn release(&mut self, renderer: &mut Renderer<'_>, objects: Option<&mut dyn TextureStore>) {
        for m in [self.mesh.take(), self.staged.take()].into_iter().flatten() {
            if let GpuKind::Mesh(id) = m.kind {
                renderer.destroy_mesh(id);
            }
        }
        self.ceiling = None;
        if let ShadedCeiling::Ready(id, _) =
            std::mem::replace(&mut self.shaded_ceiling, ShadedCeiling::None)
        {
            renderer.destroy_mesh(id);
        }
        for (_, (id, _)) in self.own.drain() {
            renderer.destroy_texture(id);
        }
        self.parked.clear();
        if let Bake::Ready(id, _) = std::mem::replace(&mut self.bake, Bake::None) {
            renderer.destroy_texture(id);
        }
        if let Some(objects) = objects {
            objects.release(&self.shared);
            self.shared.clear();
        }
    }

    /// Frees the terrain chunk's mesh, staged mesh and waiting product (a chunk that
    /// [`Self::terrain_out_of_scene`] leaves out).
    fn drop_terrain(&mut self, renderer: &mut Renderer<'_>) {
        for m in [self.mesh.take(), self.staged.take()].into_iter().flatten() {
            if let GpuKind::Mesh(id) = m.kind {
                renderer.destroy_mesh(id);
            }
        }
        self.ready = None;
        self.failed = None;
    }

    /// Frees the ceiling's height-shaded mesh, built or uploaded (a ceiling that
    /// [`Self::ceiling_out_of_scene`] leaves out). A build still running is left to finish;
    /// it is freed in the next frame.
    fn drop_shaded_ceiling(&mut self, renderer: &mut Renderer<'_>) {
        match std::mem::replace(&mut self.shaded_ceiling, ShadedCeiling::None) {
            ShadedCeiling::Ready(id, _) => {
                renderer.destroy_mesh(id);
            }
            ShadedCeiling::Waiting(_) | ShadedCeiling::None => {}
            other @ (ShadedCeiling::Pending | ShadedCeiling::Failed) => {
                self.shaded_ceiling = other;
            }
        }
    }

    /// Whether a full-layer terrain chunk is drawn, staged or waiting for its upload (it
    /// samples the terrain masks).
    fn uses_masks(&self) -> bool {
        let near = |sig: &MeshSig| sig.layered && !sig.far;
        self.mesh.as_ref().is_some_and(|m| near(&m.sig))
            || self.staged.as_ref().is_some_and(|m| near(&m.sig))
            || self.ready.as_ref().is_some_and(|r| near(&r.0))
    }

    /// Releases the terrain masks of a block that went back into the far band
    /// (`walker.md` §14.4: "a block in the band keeps only its light map on the GPU"):
    /// once its masks wait again ([`defers_masks`]), its baked chunk is what is drawn and
    /// no full-layer chunk is left, the uploaded masks are freed and queued again (at the
    /// end of `images`, as near-only images) for when the block nears. Returns how many
    /// were released.
    fn park_masks(&mut self, frame: &TerrainFrame, renderer: &mut Renderer<'_>) -> usize {
        if self.parked.is_empty()
            || !defers_masks(frame, self)
            || !matches!(self.bake, Bake::Ready(..))
            || !self.mesh.as_ref().is_some_and(|m| m.sig.far)
            || self.uses_masks()
        {
            return 0;
        }
        let n = self.parked.len();
        for (did, img) in std::mem::take(&mut self.parked) {
            if let Some((id, _)) = self.own.remove(&did) {
                renderer.destroy_texture(id);
            }
            self.images.push((did, img));
            self.near_only += 1;
        }
        n
    }

    /// Whether every image the block draws with in `frame` is uploaded.
    fn images_done(&self, frame: &TerrainFrame) -> bool {
        self.images.len()
            == if defers_masks(frame, self) {
                self.near_only
            } else {
                0
            }
    }

    /// Whether the terrain chunk resolved to at least one drawn layer (a chunk without
    /// stored layers in an open block resolves to the one layer of
    /// `terrain-texturing.md` §4.9). One that does not is drawn like a chunk that does not
    /// resolve (`walker.md` §14.4: the M3 height-and-slope mesh), unless it is hidden
    /// ([`Self::terrain_hidden`]).
    fn has_terrain_layers(&self) -> bool {
        self.layers.as_ref().is_some_and(|l| !l.terrain.is_empty())
    }

    /// Whether the original draws nothing for the terrain chunk: it stores no layers and
    /// the block is not open (`terrain-texturing.md` §4.9 rule 3).
    fn terrain_hidden(&self) -> bool {
        self.layers.as_ref().is_some_and(|l| l.terrain_hidden)
    }

    /// Whether the terrain chunk is left out of the scene in `frame`: §4.9 rule 3 in the
    /// normal (layered, lit) view. No mesh is built, uploaded or drawn for it
    /// (`terrain-texturing.md` §4.9: "not added to the client's scene at all").
    fn terrain_out_of_scene(&self, frame: &TerrainFrame) -> bool {
        frame.layered && frame.lit && self.terrain_hidden()
    }

    /// Whether the ceiling is left out of the scene in `frame`: §4.9 rule 3 hides it and
    /// the view is the normal (layered, lit) one. Nothing is kept on the GPU for it, not
    /// even a height-shaded mesh built in another view (`terrain-texturing.md` §4.9: "not
    /// added to the client's scene at all").
    fn ceiling_out_of_scene(&self, frame: &TerrainFrame) -> bool {
        frame.layered && frame.lit && self.layers.as_ref().is_some_and(|l| l.ceiling_hidden)
    }

    /// The ceiling's layers when it is drawn with them: it has a layer to draw and is not
    /// hidden by §4.9 rule 3. A ceiling without one gets no layered chunk at all (it would
    /// be a zero-layer draw that still receives lights); it is drawn by its height-shaded
    /// fallback or not at all ([`Self::wants_shaded_ceiling`]).
    fn layered_ceiling(&self) -> Option<(Arc<ChunkLayers>, Arc<TerrainBlock>)> {
        let l = self.layers.as_ref()?;
        if l.ceiling_hidden {
            return None;
        }
        l.ceiling.clone().filter(|(c, _)| !c.is_empty())
    }

    /// Whether the block's ceiling is drawn as its height-shaded mesh in `frame`
    /// (`walker.md` §14.4 fallback, extended to ceilings; Tessera Decision): the block has
    /// a ceiling grid and its block job is done, and the ceiling is not drawn with layers:
    /// layers or lighting switched off (then also a ceiling that §4.9 rule 3 hides, the M3
    /// view), no 0x19 table, or ceiling layers that do not resolve. In the normal view a
    /// ceiling hidden by rule 3 draws nothing.
    fn wants_shaded_ceiling(&self, frame: &TerrainFrame) -> bool {
        if self.block.is_none() || self.bundle.data.ceiling.is_none() {
            return false;
        }
        if !(frame.layered && frame.lit) {
            return true;
        }
        let layers = self.layers.as_ref();
        let hidden = layers.is_some_and(|l| l.ceiling_hidden);
        let layered = layers
            .and_then(|l| l.ceiling.as_ref())
            .is_some_and(|(c, _)| !c.is_empty());
        !(hidden || layered)
    }

    /// Whether image `did` is needed while the block is drawn baked: its light map, and
    /// everything of a cave ceiling (drawn with its layers at every distance).
    fn needed_when_far(&self, did: Did) -> bool {
        self.layers.as_ref().is_none_or(|l| {
            l.terrain
                .layers()
                .iter()
                .any(|x| x.textures().light_map == Some(did))
                || l.ceiling.as_ref().is_some_and(|(c, _)| {
                    c.layers().iter().any(|x| {
                        let t = x.textures();
                        t.light_map == Some(did) || t.mask == Some(did)
                    })
                })
        })
    }
}

/// Whether a block's masks wait (`walker.md` §14.4): it is in the far band, which is on,
/// and its bake has not failed.
fn defers_masks(frame: &TerrainFrame, slot: &Slot) -> bool {
    frame.layered
        && frame.lit
        && frame.far_band > 0.0
        && slot.far
        && !matches!(slot.bake, Bake::Failed)
}

/// Counters for the HUD and the frame log.
#[derive(Debug, Clone, Copy, Default, PartialEq, Eq)]
pub struct TerrainStats {
    /// Landblocks tracked.
    pub blocks: usize,
    /// Blocks with a terrain block built.
    pub built: usize,
    /// Blocks with a mesh on the GPU.
    pub meshed: usize,
    /// Blocks drawn with texture layers.
    pub layered: usize,
    /// Layered chunks waiting for a texture.
    pub incomplete: usize,
    /// Mesh jobs in flight, waiting for upload or staged for a neighbour.
    pub pending: usize,
    /// Of `pending`: mesh jobs in flight.
    pub in_flight: usize,
    /// Of `pending`: meshes built and waiting for the upload budget.
    pub awaiting_upload: usize,
    /// Of `pending`: meshes uploaded and staged until the neighbours' seams agree.
    pub staged: usize,
    /// Blocks with masks or light maps waiting for the upload budget.
    pub images_waiting: usize,
    /// Blocks whose block job or latest mesh attempt failed.
    pub failed: usize,
    /// Terrain jobs that panicked so far.
    pub panics: u64,
    /// GPU bytes of terrain meshes, chunks, masks and light maps (drawn and staged).
    pub gpu_bytes: u64,
    /// Meshes and chunks drawn last frame.
    pub drawn: usize,
    /// Ceiling chunks drawn last frame.
    pub ceilings: usize,
    /// Blocks drawn in the far band (baked).
    pub far: usize,
    /// Far-band bakes on the GPU.
    pub baked: usize,
    /// Triangles drawn last frame (meshes; chunks count their layers' triangles).
    pub triangles: u64,
}

/// How the terrain of one frame is drawn.
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct TerrainFrame {
    /// Draw texture layers (needs a lit pass); off draws the M3 height-shaded meshes.
    pub layered: bool,
    /// Draw through the lit pipeline.
    pub lit: bool,
    /// Chunks within this distance (metres) cast the sun's shadow; 0 for none.
    pub shadow_radius: f32,
    /// Only blocks within this distance (metres, to the block square) are drawn.
    pub extent: f32,
    /// Blocks farther than this (metres) are drawn with their baked layer stack
    /// (`walker.md` §14.4); 0 draws full layers at every distance.
    pub far_band: f32,
    /// The material clock in seconds (`material.md` §5.3.1), for scrolling layers.
    pub time: f64,
}

impl Default for TerrainFrame {
    fn default() -> Self {
        Self {
            layered: true,
            lit: true,
            shadow_radius: 0.0,
            extent: f32::INFINITY,
            far_band: 0.0,
            time: 0.0,
        }
    }
}

/// Bake jobs queued per frame at most.
const BAKES_PER_FRAME: usize = 8;

/// Blocks waiting for a bake examined per frame (their layer textures looked up).
const BAKE_CANDIDATES_PER_FRAME: usize = 48;
/// The far band's hysteresis: a far block returns to full layers this much (metres)
/// inside the band.
const FAR_HYSTERESIS: f32 = 40.0;
/// Whether a block at distance `d` is in the far band starting at `band` (0 = off), with
/// hysteresis: it enters beyond `band` and leaves at `band` − [`FAR_HYSTERESIS`], the
/// hysteresis limited to half the band so that a band of 40 m or less still has a
/// positive exit distance (`walker.md` §14.4; Tessera Decision, OPEN there).
#[must_use]
pub fn in_far_band(was: bool, d: f32, band: f32) -> bool {
    if !(band.is_finite() && band > 0.0) {
        return false;
    }
    let exit = band - FAR_HYSTERESIS.min(0.5 * band);
    if was { d > exit } else { d > band }
}

/// Blocks are baked once they come this much (metres) inside the band, ahead of need.
const BAKE_AHEAD: f32 = 320.0;

/// The terrain layer (module docs).
pub struct TerrainLayer {
    runner: JobRunner<Job, Output>,
    slots: HashMap<LandblockKey, Slot>,
    tiles: SharedTiles,
    selector: LodSelector,
    shading: Arc<HeightSlopeShading>,
    mesh_config: MeshConfig,
    next_gen: u64,
    inputs: Arc<BlockInputs>,
    stats: TerrainStats,
    panics: u64,
    /// Finished block products not yet taken by the walker.
    finished: Vec<(LandblockKey, BlockProduct)>,
    order: Vec<(f32, LandblockKey)>,
    frame: TerrainFrame,
}

impl std::fmt::Debug for TerrainLayer {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("TerrainLayer")
            .field("stats", &self.stats)
            .finish_non_exhaustive()
    }
}

/// What the edge-consistency check needs of a block: the edge levels of the drawn and the
/// staged mesh, and whether its neighbours should ignore it (its meshing failed, so it
/// may never change).
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
struct EdgeView {
    shown: Option<[Lod; 4]>,
    staged: Option<[Lod; 4]>,
    frozen: bool,
}

/// The blocks whose staged mesh can be drawn this frame: the largest set such that, after
/// swapping them in, every pair of drawn neighbours meshed their shared edge at the same
/// level (`terrain.md` §4). Blocks without a drawn mesh constrain nothing; frozen blocks
/// are ignored by their neighbours.
fn promotable(views: &HashMap<LandblockKey, EdgeView>) -> HashSet<LandblockKey> {
    let mut set: HashSet<LandblockKey> = views
        .iter()
        .filter(|(_, v)| v.staged.is_some())
        .map(|(k, _)| *k)
        .collect();
    let mut work: Vec<LandblockKey> = set.iter().copied().collect();
    while let Some(key) = work.pop() {
        if !set.contains(&key) {
            continue;
        }
        let Some(edges) = views.get(&key).and_then(|v| v.staged) else {
            set.remove(&key);
            continue;
        };
        let agrees = Side::ALL.iter().all(|&side| {
            let (dx, dy) = side.offset();
            let Some(n) = key.offset(dx, dy) else {
                return true;
            };
            let Some(v) = views.get(&n).filter(|v| !v.frozen) else {
                return true;
            };
            let drawn = if set.contains(&n) { v.staged } else { v.shown };
            drawn.is_none_or(|e| e[side.opposite().index()] == edges[side.index()])
        });
        if !agrees {
            set.remove(&key);
            for side in Side::ALL {
                let (dx, dy) = side.offset();
                if let Some(n) = key.offset(dx, dy)
                    && set.contains(&n)
                {
                    work.push(n);
                }
            }
        }
    }
    set
}

/// Runs `f`, turning a panic into an error message.
fn guarded<T>(f: impl FnOnce() -> T) -> Result<T, String> {
    std::panic::catch_unwind(AssertUnwindSafe(f)).map_err(|p| {
        p.downcast_ref::<&str>()
            .map(|s| (*s).to_owned())
            .or_else(|| p.downcast_ref::<String>().cloned())
            .unwrap_or_else(|| "panicked".to_owned())
    })
}

/// The shared textures a layered block needs: every drawn layer's diffuse, normal,
/// emissive and glow lists of its terrain and ceiling chunks (`terrain-texturing.md` §8.2).
#[must_use]
pub fn layer_textures(layers: &BlockLayers) -> Vec<TexRequest> {
    let mut out: Vec<TexRequest> = Vec::new();
    let chunks = std::iter::once(&*layers.terrain).chain(layers.ceiling.iter().map(|c| &*c.0));
    for chunk in chunks {
        for l in chunk.layers() {
            let t = l.textures();
            for did in [t.diffuse, t.normal, t.emissive, t.glow]
                .into_iter()
                .flatten()
            {
                let r = TexRequest {
                    did: did.0,
                    variant: VARIANT_OWN_ALPHA,
                };
                if !out.contains(&r) {
                    out.push(r);
                }
            }
        }
    }
    out
}

impl TerrainLayer {
    /// A layer with `threads` workers.
    ///
    /// # Errors
    ///
    /// When the worker threads cannot be started.
    pub fn new(
        threads: usize,
        selector: LodSelector,
        mesh_config: MeshConfig,
        inputs: Arc<BlockInputs>,
    ) -> std::io::Result<Self> {
        Ok(Self {
            runner: JobRunner::new("tessera-terrain", threads, QUEUE_CAPACITY, run_job)?,
            slots: HashMap::new(),
            tiles: SharedTiles::default(),
            selector,
            shading: Arc::new(HeightSlopeShading::default()),
            mesh_config,
            next_gen: 1,
            inputs,
            stats: TerrainStats::default(),
            panics: 0,
            finished: Vec::new(),
            order: Vec::new(),
            frame: TerrainFrame::default(),
        })
    }

    /// Updates the selector for a new viewport height or field of view.
    pub fn set_selector(&mut self, selector: LodSelector) {
        self.selector = selector;
    }

    /// The resident terrain blocks of the current region (region-global height queries),
    /// shared with other readers.
    #[must_use]
    pub fn tiles(&self) -> &SharedTiles {
        &self.tiles
    }

    /// Counters.
    #[must_use]
    pub fn stats(&self) -> TerrainStats {
        self.stats
    }

    /// Whether every tracked block is built and meshed with no job outstanding, its
    /// layered chunk complete and its ceiling drawn (blocks whose block job or current
    /// mesh failed count as settled).
    #[must_use]
    pub fn settled(&self) -> bool {
        self.slots.iter().all(|(key, s)| {
            if s.block_failed {
                return true;
            }
            if s.block.is_none() || s.pending.is_some() || s.ready.is_some() {
                return false;
            }
            if s.failed
                .is_some_and(|f| Some(f.sig) == self.signature(*key))
            {
                return true;
            }
            let ceiling_done = s.ceiling.as_ref().is_none_or(|c| {
                !c.pending
                    && c.gpu.as_ref().is_none_or(|g| g.1)
                    && (c.gpu.is_some() || c.prep.is_none())
            });
            let shaded_done = if s.ceiling_out_of_scene(&self.frame) {
                // A build from another view still has to land and be freed.
                matches!(
                    s.shaded_ceiling,
                    ShadedCeiling::None | ShadedCeiling::Failed
                )
            } else {
                !s.wants_shaded_ceiling(&self.frame)
                    || matches!(
                        s.shaded_ceiling,
                        ShadedCeiling::Ready(..) | ShadedCeiling::Failed
                    )
            };
            let terrain_done = if s.terrain_out_of_scene(&self.frame) {
                s.mesh.is_none()
            } else {
                s.mesh.as_ref().is_some_and(GpuTerrain::complete)
            };
            terrain_done
                && s.staged.is_none()
                && s.images_done(&self.frame)
                && ceiling_done
                && shaded_done
        })
    }

    /// Puts the queued jobs nearest first and drops those of blocks no longer tracked (or
    /// tracked with a newer bundle): under a fast camera the queue would otherwise work
    /// through blocks long passed while the near ones wait.
    fn rank_jobs(&self, anchor: &Anchor, camera: Vec3) {
        let size = LANDBLOCK_SIZE as f32;
        let dropped = self.runner.reorder(|job| {
            let (key, generation) = job.target();
            let slot = self.slots.get(&key)?;
            if slot.generation != generation {
                return None;
            }
            let off = anchor.offset_of(key)?;
            let near = Vec3::new(
                camera.x.clamp(off.x, off.x + size),
                camera.y.clamp(off.y, off.y + size),
                camera.z,
            );
            Some((near - camera).length())
        });
        if dropped > 0 {
            tracing::trace!(dropped, "stale terrain jobs dropped");
        }
    }

    /// Block products finished since the last call (objects and overview tiles for the
    /// other layers).
    pub fn take_finished(&mut self) -> Vec<(LandblockKey, BlockProduct)> {
        std::mem::take(&mut self.finished)
    }

    /// Starts tracking `bundle` (new, or replacing the bundle tracked for `key`, whose
    /// meshes are freed). Returns false when the block job could not be queued (call
    /// again next frame; a replaced slot is then kept as it was).
    pub fn insert(
        &mut self,
        key: LandblockKey,
        bundle: &Arc<LandblockBundle>,
        renderer: &mut Renderer<'_>,
        objects: &mut dyn TextureStore,
    ) -> bool {
        let generation = self.next_gen;
        let job = Job::Block {
            key,
            generation,
            bundle: Arc::clone(bundle),
            shading: Arc::clone(&self.shading),
            inputs: Arc::clone(&self.inputs),
        };
        if self.runner.submit(job).is_err() {
            return false;
        }
        self.tiles.set_region(key.region());
        self.next_gen += 1;
        if let Some(mut old) = self
            .slots
            .insert(key, Slot::new(Arc::clone(bundle), generation))
        {
            old.release(renderer, Some(objects));
            self.tiles.remove(key);
        }
        true
    }

    /// Whether `key` is tracked with exactly this bundle.
    #[must_use]
    pub fn tracks(&self, key: LandblockKey, bundle: &Arc<LandblockBundle>) -> bool {
        self.slots
            .get(&key)
            .is_some_and(|s| Arc::ptr_eq(&s.bundle, bundle))
    }

    /// Keys tracked.
    pub fn keys(&self) -> impl Iterator<Item = LandblockKey> + '_ {
        self.slots.keys().copied()
    }

    /// Stops tracking `key` and frees its meshes and textures.
    pub fn remove(
        &mut self,
        key: LandblockKey,
        renderer: &mut Renderer<'_>,
        objects: &mut dyn TextureStore,
    ) {
        if let Some(mut slot) = self.slots.remove(&key) {
            slot.release(renderer, Some(objects));
            self.tiles.remove(key);
        }
    }

    /// Frees everything.
    pub fn clear(&mut self, renderer: &mut Renderer<'_>, objects: &mut dyn TextureStore) {
        for (_, mut slot) in self.slots.drain() {
            slot.release(renderer, Some(objects));
        }
        self.tiles.clear();
    }

    /// The colour of the terrain at block-local (`x`, `y`) of `key` for frills
    /// (`scenery.md` §8.3 rule 7, Tessera Decision): the mean colour of the top layer's
    /// diffuse texture covering the point, times its `diffuse_scale`. `None` without
    /// layers or before the texture is on the GPU.
    #[must_use]
    pub fn colour_at(
        &self,
        key: LandblockKey,
        x: f32,
        y: f32,
        objects: &dyn TextureStore,
    ) -> Option<[f32; 3]> {
        let layers = &self.slots.get(&key)?.layers.as_ref()?.terrain;
        let tri = layers.triangle_at(x, y)?;
        let top = layers.layers_at(tri).last()?;
        let diffuse = top.textures().diffuse?;
        let mean = objects.mean(TexRequest {
            did: diffuse.0,
            variant: VARIANT_OWN_ALPHA,
        })?;
        let s = top.params().diffuse_colour;
        Some([mean[0] * s[0], mean[1] * s[1], mean[2] * s[2]])
    }

    /// Per frame: collect finished jobs, choose levels, queue meshes, upload within
    /// `budget`, swap in staged meshes whose seams agree. `camera` is in the render frame
    /// of `anchor`.
    pub fn update(
        &mut self,
        renderer: &mut Renderer<'_>,
        anchor: &Anchor,
        camera: Vec3,
        frame: &TerrainFrame,
        objects: &mut dyn TextureStore,
        budget: &mut UploadBudget,
    ) {
        let now = Instant::now();
        self.frame = *frame;
        self.collect(now, objects);

        // Levels, nearest first.
        self.order.clear();
        for (key, slot) in &mut self.slots {
            let Some(block) = &slot.block else { continue };
            let Some(off) = anchor.offset_of(*key) else {
                continue;
            };
            let local = camera - off;
            let d = block.distance_to(local.to_array());
            slot.lod = Some(self.selector.select(block, d, slot.lod));
            slot.far = in_far_band(slot.far, d, frame.far_band);
            self.order.push((d, *key));
        }
        self.order
            .sort_unstable_by(|a, b| a.0.total_cmp(&b.0).then(a.1.cmp(&b.1)));
        self.rank_jobs(anchor, camera);

        // Block textures (masks, light maps), nearest first.
        let order = std::mem::take(&mut self.order);
        for &(_, key) in &order {
            let Some(slot) = self.slots.get_mut(&key) else {
                continue;
            };
            // A block drawn baked needs only its light map; its masks wait until it nears.
            let deferring = defers_masks(frame, slot);
            loop {
                // The images needed far (light maps) first, then the masks (at the end)
                // unless they wait.
                let i = if slot.images.len() > slot.near_only {
                    slot.images.len() - slot.near_only - 1
                } else if !deferring && slot.near_only > 0 {
                    slot.images.len() - 1
                } else {
                    break;
                };
                let Some((_, img)) = slot.images.get(i) else {
                    break;
                };
                let bytes = img.upload_bytes();
                if !budget.try_take(bytes) {
                    break;
                }
                let near_only = i + slot.near_only >= slot.images.len() && slot.near_only > 0;
                if near_only {
                    slot.near_only -= 1;
                }
                let (did, img) = slot.images.remove(i);
                let uploaded = match img.to_rgba() {
                    Some(img) => renderer
                        .upload_texture(&TextureUpload {
                            label: Some(&format!("terrain {did}")),
                            wrap: Wrap::Clamp,
                            ..TextureUpload::rgba8(img.width, img.height, &img.pixels)
                        })
                        .map_err(|e| e.to_string()),
                    None => Err("malformed mask".to_owned()),
                };
                match uploaded {
                    Ok(id) => {
                        slot.own.insert(did, (id, bytes));
                        if near_only {
                            slot.parked.push((did, img));
                        }
                    }
                    Err(e) => {
                        tracing::debug!(%key, %did, "terrain image upload failed: {e}");
                        slot.own_failed.insert(did);
                    }
                }
            }
        }

        // Far-band bakes, nearest first, ahead of need.
        let mut bakes = 0usize;
        let mut examined = 0usize;
        for &(d, key) in &order {
            if bakes >= BAKES_PER_FRAME
                || examined >= BAKE_CANDIDATES_PER_FRAME
                || frame.far_band <= 0.0
                || !frame.layered
            {
                break;
            }
            if d < frame.far_band - BAKE_AHEAD {
                continue;
            }
            let Some(slot) = self.slots.get_mut(&key) else {
                continue;
            };
            // Only a terrain chunk drawn with its layers has a stack to bake: not a closed
            // layerless chunk (§4.9 rule 3) nor one none of whose layers resolves.
            if !matches!(slot.bake, Bake::None)
                || !slot.has_terrain_layers()
                || slot.terrain_hidden()
            {
                continue;
            }
            let (Some(layers), Some(block), Some(masks)) =
                (&slot.layers, &slot.block, &slot.bake_masks)
            else {
                continue;
            };
            examined += 1;
            // Every layer texture's small levels (a texture that failed bakes as white).
            let mut diffuse: HashMap<Did, SmallChain> = HashMap::new();
            let mut ready = true;
            for l in layers.terrain.layers() {
                let Some(did) = l.textures().diffuse else {
                    continue;
                };
                let t = TexRequest {
                    did: did.0,
                    variant: VARIANT_OWN_ALPHA,
                };
                match (objects.small(t), objects.lookup(t)) {
                    (Some(c), _) => {
                        diffuse.insert(did, c);
                    }
                    (None, TexLookup::Pending) => ready = false,
                    (None, _) => {}
                }
            }
            if !ready {
                continue;
            }
            let job = Job::Bake {
                key,
                generation: slot.generation,
                block: Arc::clone(block),
                layers: Arc::clone(&layers.terrain),
                diffuse,
                masks: Arc::clone(masks),
            };
            if self.runner.submit(job).is_err() {
                break;
            }
            slot.bake = Bake::Pending;
            bakes += 1;
        }
        // Baked images to the GPU.
        for &(_, key) in &order {
            let Some(slot) = self.slots.get_mut(&key) else {
                continue;
            };
            let Bake::Waiting(chain) = &slot.bake else {
                continue;
            };
            let bytes: u64 = chain.iter().map(|l| l.pixels.len() as u64).sum();
            if !budget.try_take(bytes) {
                break;
            }
            let levels: Vec<tessera_render::MipLevel<'_>> = chain
                .iter()
                .map(|l| tessera_render::MipLevel {
                    width: l.width,
                    height: l.height,
                    rgba: &l.pixels,
                })
                .collect();
            let label = format!("terrain bake {key}");
            slot.bake = match renderer.upload_texture_mips(&tessera_render::MipChainUpload {
                label: Some(&label),
                wrap: Wrap::Clamp,
                ..tessera_render::MipChainUpload::new(&levels)
            }) {
                Ok(id) => {
                    slot.bake_masks = None;
                    Bake::Ready(id, bytes)
                }
                Err(e) => {
                    tracing::debug!(%key, "terrain bake upload failed: {e}");
                    Bake::Failed
                }
            };
        }

        // What the frame leaves out: the meshes of terrain chunks and ceilings hidden by
        // §4.9 rule 3 (built in another view), and the masks of blocks back in the far
        // band.
        for slot in self.slots.values_mut() {
            if slot.terrain_out_of_scene(frame) {
                slot.drop_terrain(renderer);
            }
            if slot.ceiling_out_of_scene(frame) {
                slot.drop_shaded_ceiling(renderer);
            }
            slot.park_masks(frame, renderer);
        }

        // Mesh jobs for blocks whose signature changed.
        let mut queued = 0usize;
        for &(_, key) in &order {
            if queued >= MESH_JOBS_PER_FRAME {
                break;
            }
            let Some(sig) = self.signature(key) else {
                continue;
            };
            let Some(slot) = self.slots.get_mut(&key) else {
                continue;
            };
            // A staged mesh made stale by a level that flipped back is not needed.
            if slot.mesh.as_ref().is_some_and(|m| m.sig == sig)
                && let Some(old) = slot.staged.take()
                && let GpuKind::Mesh(id) = old.kind
            {
                renderer.destroy_mesh(id);
            }
            if !slot.wants_mesh(sig, now) {
                continue;
            }
            let Some(block) = slot.block.clone() else {
                continue;
            };
            let generation = slot.generation;
            let layers = if sig.layered {
                slot.layers.as_ref().map(|l| Arc::clone(&l.terrain))
            } else {
                None
            };
            let far = sig.far;
            let neighbours = Side::ALL.map(|side| self.neighbour_block(key, side));
            let job = Job::Mesh {
                key,
                generation,
                sig,
                block,
                neighbours,
                shading: Arc::clone(&self.shading),
                config: self.mesh_config,
                layers,
                far,
            };
            if self.runner.submit(job).is_err() {
                break;
            }
            if let Some(slot) = self.slots.get_mut(&key) {
                slot.pending = Some(sig);
            }
            queued += 1;
        }

        // Ceiling jobs.
        for &(_, key) in &order {
            if queued >= MESH_JOBS_PER_FRAME || !frame.layered {
                break;
            }
            let Some(slot) = self.slots.get_mut(&key) else {
                continue;
            };
            let Some((layers, ceiling)) = slot.layered_ceiling() else {
                continue;
            };
            if slot.ceiling.is_some() {
                continue;
            }
            let job = Job::Ceiling {
                key,
                generation: slot.generation,
                ceiling,
                layers,
            };
            if self.runner.submit(job).is_err() {
                break;
            }
            slot.ceiling = Some(Ceiling {
                prep: None,
                gpu: None,
                pending: true,
            });
            queued += 1;
        }

        // Height-shaded ceilings where the layered ceiling is not drawn.
        for &(_, key) in &order {
            if queued >= MESH_JOBS_PER_FRAME {
                break;
            }
            let Some(slot) = self.slots.get_mut(&key) else {
                continue;
            };
            if !matches!(slot.shaded_ceiling, ShadedCeiling::None)
                || !slot.wants_shaded_ceiling(frame)
            {
                continue;
            }
            let job = Job::ShadedCeiling {
                key,
                generation: slot.generation,
                bundle: Arc::clone(&slot.bundle),
                shading: Arc::clone(&self.shading),
                config: self.mesh_config,
            };
            if self.runner.submit(job).is_err() {
                break;
            }
            slot.shaded_ceiling = ShadedCeiling::Pending;
            queued += 1;
        }

        // Uploads, nearest first, into the staging slot.
        for &(_, key) in &order {
            let Some(slot) = self.slots.get_mut(&key) else {
                continue;
            };
            // A drawn chunk built while a texture was missing: rebuilt once they are all
            // there.
            if let Some(m) = &slot.mesh
                && let GpuKind::Chunk {
                    prep: Some(prep), ..
                } = &m.kind
                && let Some(layers) = &slot.layers
            {
                let prep = Arc::clone(prep);
                if let Some((tex, lm, true)) =
                    chunk_textures(&layers.terrain, slot, objects, GridSurface::Terrain)
                    && budget.try_take(prep.bytes())
                    && let Ok(chunk) = upload_chunk(renderer, key, &prep, &layers.terrain, &tex, lm)
                    && let Some(m) = slot.mesh.as_mut()
                {
                    m.kind = GpuKind::Chunk { chunk, prep: None };
                }
            }
            // Ceilings.
            if let Some(layers) = slot.layers.clone()
                && let Some((clayers, _)) = &layers.ceiling
                && let Some(c) = &slot.ceiling
                && let Some(prep) = c.prep.clone()
                && c.gpu.as_ref().is_none_or(|g| !g.1)
                && let Some((tex, lm, complete)) =
                    chunk_textures(clayers, slot, objects, GridSurface::Ceiling)
                && (complete || c.gpu.is_none())
                && budget.try_take(prep.bytes())
            {
                match upload_chunk(renderer, key, &prep, clayers, &tex, lm) {
                    Ok(chunk) => {
                        if let Some(c) = slot.ceiling.as_mut() {
                            c.gpu = Some((chunk, complete));
                            if complete {
                                c.prep = None;
                            }
                        }
                    }
                    Err(e) => {
                        tracing::debug!(%key, "ceiling chunk upload failed: {e}");
                        if let Some(c) = slot.ceiling.as_mut() {
                            c.prep = None;
                        }
                    }
                }
            }
            if matches!(slot.shaded_ceiling, ShadedCeiling::Waiting(_)) {
                let ShadedCeiling::Waiting(prepared) =
                    std::mem::replace(&mut slot.shaded_ceiling, ShadedCeiling::Pending)
                else {
                    continue;
                };
                let bytes = prepared.gpu_bytes();
                slot.shaded_ceiling = if budget.try_take(bytes) {
                    match renderer.upload_prepared(prepared) {
                        Ok(id) => ShadedCeiling::Ready(id, bytes),
                        Err(e) => {
                            tracing::debug!(%key, "shaded ceiling upload failed: {e}");
                            ShadedCeiling::Failed
                        }
                    }
                } else {
                    ShadedCeiling::Waiting(prepared)
                };
            }
            let Some((sig, built, min_h, max_h)) = slot.ready.take() else {
                continue;
            };
            match built {
                Built::Mesh(prepared) => {
                    let bytes = prepared.gpu_bytes();
                    if !budget.try_take(bytes) {
                        slot.ready = Some((sig, Built::Mesh(prepared), min_h, max_h));
                        break;
                    }
                    match renderer.upload_prepared(prepared) {
                        Ok(id) => {
                            slot.failed = None;
                            stage(
                                renderer,
                                slot,
                                GpuTerrain {
                                    kind: GpuKind::Mesh(id),
                                    sig,
                                    min_h,
                                    max_h,
                                    bytes,
                                },
                            );
                        }
                        Err(e) => {
                            let attempt = slot.fail(sig, now);
                            if attempt == 1 {
                                tracing::warn!(%key, "terrain upload failed (retrying later): {e}");
                            } else {
                                tracing::debug!(%key, attempt, "terrain upload failed again: {e}");
                            }
                        }
                    }
                }
                Built::Chunk(prep) if prep.far => {
                    let Some(layers) = slot.layers.clone() else {
                        continue;
                    };
                    let Bake::Ready(baked, _) = slot.bake else {
                        // The bake went away: build the signature again.
                        continue;
                    };
                    let light_map = layers
                        .terrain
                        .layers()
                        .iter()
                        .find_map(|l| l.textures().light_map)
                        .and_then(|d| slot.own.get(&d).map(|v| v.0));
                    let bytes = prep.bytes();
                    if !budget.try_take(bytes) {
                        slot.ready = Some((sig, Built::Chunk(prep), min_h, max_h));
                        break;
                    }
                    match upload_far_chunk(renderer, key, &prep, baked, light_map) {
                        Ok(chunk) => {
                            slot.failed = None;
                            stage(
                                renderer,
                                slot,
                                GpuTerrain {
                                    kind: GpuKind::Chunk { chunk, prep: None },
                                    sig,
                                    min_h,
                                    max_h,
                                    bytes,
                                },
                            );
                        }
                        Err(e) => {
                            slot.fail(sig, now);
                            tracing::debug!(%key, "far chunk upload failed: {e}");
                        }
                    }
                }
                Built::Chunk(prep) => {
                    let Some(layers) = slot.layers.clone() else {
                        continue;
                    };
                    let textures =
                        chunk_textures(&layers.terrain, slot, objects, GridSurface::Terrain);
                    let first = slot.mesh.is_none() && slot.staged.is_none();
                    let Some((tex, lm, complete)) = textures.filter(|t| t.2 || first) else {
                        // Waiting for textures.
                        slot.ready = Some((sig, Built::Chunk(prep), min_h, max_h));
                        continue;
                    };
                    let bytes = prep.bytes();
                    if !budget.try_take(bytes) {
                        slot.ready = Some((sig, Built::Chunk(prep), min_h, max_h));
                        break;
                    }
                    match upload_chunk(renderer, key, &prep, &layers.terrain, &tex, lm) {
                        Ok(chunk) => {
                            slot.failed = None;
                            stage(
                                renderer,
                                slot,
                                GpuTerrain {
                                    kind: GpuKind::Chunk {
                                        chunk,
                                        prep: (!complete).then_some(prep),
                                    },
                                    sig,
                                    min_h,
                                    max_h,
                                    bytes,
                                },
                            );
                        }
                        Err(e) => {
                            let attempt = slot.fail(sig, now);
                            if attempt == 1 {
                                tracing::warn!(%key, "terrain chunk upload failed (retrying later): {e}");
                            } else {
                                tracing::debug!(%key, attempt, "terrain chunk upload failed again: {e}");
                            }
                        }
                    }
                }
            }
        }
        self.order = order;
        self.promote(renderer);
        self.refresh_stats();
    }

    /// Swaps in the staged meshes whose seams agree with what their neighbours draw.
    fn promote(&mut self, renderer: &mut Renderer<'_>) {
        if self.slots.values().all(|s| s.staged.is_none()) {
            return;
        }
        let views: HashMap<LandblockKey, EdgeView> = self
            .slots
            .iter()
            .map(|(k, s)| {
                (
                    *k,
                    EdgeView {
                        shown: s.mesh.as_ref().map(|m| m.sig.edges),
                        staged: s.staged.as_ref().map(|m| m.sig.edges),
                        frozen: s.block_failed || s.failed.is_some(),
                    },
                )
            })
            .collect();
        for key in promotable(&views) {
            if let Some(slot) = self.slots.get_mut(&key)
                && let Some(new) = slot.staged.take()
                && let Some(old) = slot.mesh.replace(new)
                && let GpuKind::Mesh(id) = old.kind
            {
                renderer.destroy_mesh(id);
            }
        }
    }

    fn neighbour_block(&self, key: LandblockKey, side: Side) -> Option<Arc<TerrainBlock>> {
        let (dx, dy) = side.offset();
        let n = key.offset(dx, dy)?;
        self.slots.get(&n)?.block.clone()
    }

    fn signature(&self, key: LandblockKey) -> Option<MeshSig> {
        let slot = self.slots.get(&key)?;
        if slot.terrain_out_of_scene(&self.frame) {
            return None;
        }
        let lod = slot.lod?;
        let mut edges = EdgeLods::NONE;
        let mut present = 0u8;
        for side in Side::ALL {
            let (dx, dy) = side.offset();
            let n = key.offset(dx, dy).and_then(|n| self.slots.get(&n));
            if let Some(n) = n
                && n.block.is_some()
            {
                present |= 1 << side.index();
                edges = edges.with(side, n.lod);
            }
        }
        let layered = self.frame.layered && self.frame.lit && slot.has_terrain_layers();
        let baked = matches!(slot.bake, Bake::Ready(..));
        // A far block whose bake is not there yet, and that has no full-layer chunk drawn,
        // is drawn height-shaded meanwhile (`walker.md` §14.4): a full-layer chunk would be
        // built and uploaded only to be replaced by the baked one.
        let placeholder = layered
            && slot.far
            && !baked
            && !matches!(slot.bake, Bake::Failed)
            && !slot
                .mesh
                .as_ref()
                .is_some_and(|m| m.sig.layered && !m.sig.far);
        Some(MeshSig {
            lod,
            edges: edges.effective(lod),
            present,
            layered: layered && !placeholder,
            far: layered && slot.far && baked,
        })
    }

    fn collect(&mut self, now: Instant, objects: &mut dyn TextureStore) {
        while let Some(out) = self.runner.try_next() {
            self.apply(out, now, objects);
        }
        let panics = self.runner.panics();
        if panics > self.panics {
            tracing::error!(
                total = panics,
                "terrain jobs panicked outside the job guard"
            );
            self.panics = panics;
        }
    }

    /// Applies one worker result.
    fn apply(&mut self, out: Output, now: Instant, objects: &mut dyn TextureStore) {
        match out {
            Output::Block {
                key,
                generation,
                product,
            } => {
                let Some(slot) = self.slots.get_mut(&key) else {
                    return;
                };
                if slot.generation != generation {
                    return;
                }
                let mut product = *product;
                slot.block = Some(Arc::clone(&product.block));
                if let Some(layers) = product.layers.take() {
                    slot.scrolling = layers
                        .terrain
                        .layers()
                        .iter()
                        .enumerate()
                        .filter_map(|(i, l)| {
                            let o = l.params().offsets;
                            o.constant().is_none().then(|| {
                                (
                                    i,
                                    [
                                        (material_curve(&o.u), CurveState::new()),
                                        (material_curve(&o.v), CurveState::new()),
                                    ],
                                )
                            })
                        })
                        .collect();
                    slot.shared = layer_textures(&layers);
                    objects.acquire(&slot.shared);
                    slot.layers = Some(layers);
                }
                slot.images = std::mem::take(&mut product.images);
                // The images needed far (light maps, the ceiling's) first, masks last.
                let far: Vec<bool> = slot
                    .images
                    .iter()
                    .map(|(d, _)| slot.needed_when_far(*d))
                    .collect();
                let mut tagged: Vec<(bool, (Did, BlockImage))> =
                    far.into_iter().zip(slot.images.drain(..)).collect();
                tagged.sort_by_key(|(f, _)| !*f);
                slot.near_only = tagged.iter().filter(|(f, _)| !*f).count();
                slot.images = tagged.into_iter().map(|(_, i)| i).collect();
                slot.bake_masks = (slot.has_terrain_layers() && !slot.terrain_hidden())
                    .then(|| Arc::new(std::mem::take(&mut product.bake_masks)));
                self.tiles.insert(key, Arc::clone(&product.block));
                self.finished.push((key, product));
            }
            Output::BlockFailed {
                key,
                generation,
                reason,
            } => {
                self.panics += 1;
                let Some(slot) = self.slots.get_mut(&key) else {
                    return;
                };
                if slot.generation != generation {
                    return;
                }
                tracing::error!(%key, "terrain block failed; the block stays without terrain: {reason}");
                slot.block_failed = true;
            }
            Output::Mesh {
                key,
                generation,
                sig,
                mesh,
            } => {
                let Some(slot) = self.slots.get_mut(&key) else {
                    return;
                };
                if slot.generation != generation {
                    return;
                }
                if slot.pending == Some(sig) {
                    slot.pending = None;
                }
                match mesh {
                    Ok((p, lo, hi)) => slot.ready = Some((sig, p, lo, hi)),
                    Err(e) => {
                        let attempt = slot.fail(sig, now);
                        if attempt == 1 {
                            tracing::warn!(%key, "terrain mesh failed (retrying later): {e}");
                        } else {
                            tracing::debug!(%key, attempt, "terrain mesh failed again: {e}");
                        }
                    }
                }
            }
            Output::Bake {
                key,
                generation,
                chain,
            } => {
                let Some(slot) = self.slots.get_mut(&key) else {
                    return;
                };
                if slot.generation != generation {
                    return;
                }
                slot.bake = match chain {
                    Ok(c) if !c.is_empty() => Bake::Waiting(c),
                    Ok(_) => Bake::Failed,
                    Err(e) => {
                        tracing::warn!(%key, "terrain bake failed: {e}");
                        Bake::Failed
                    }
                };
            }
            Output::Ceiling {
                key,
                generation,
                prep,
            } => {
                let Some(slot) = self.slots.get_mut(&key) else {
                    return;
                };
                if slot.generation != generation {
                    return;
                }
                if let Some(c) = slot.ceiling.as_mut() {
                    c.pending = false;
                    c.prep = prep;
                }
            }
            Output::ShadedCeiling {
                key,
                generation,
                mesh,
            } => {
                let Some(slot) = self.slots.get_mut(&key) else {
                    return;
                };
                if slot.generation != generation {
                    return;
                }
                slot.shaded_ceiling = match mesh {
                    Ok(m) => ShadedCeiling::Waiting(m),
                    Err(e) => {
                        tracing::debug!(%key, "shaded ceiling failed: {e}");
                        ShadedCeiling::Failed
                    }
                };
            }
        }
    }

    fn refresh_stats(&mut self) {
        let mut s = TerrainStats {
            blocks: self.slots.len(),
            panics: self.panics,
            drawn: self.stats.drawn,
            ceilings: self.stats.ceilings,
            triangles: self.stats.triangles,
            ..TerrainStats::default()
        };
        for slot in self.slots.values() {
            s.built += usize::from(slot.block.is_some());
            if let Some(m) = &slot.mesh {
                s.meshed += 1;
                s.gpu_bytes += m.bytes;
                if let GpuKind::Chunk { prep, .. } = &m.kind {
                    s.layered += 1;
                    s.incomplete += usize::from(prep.is_some());
                }
            }
            if let Some(m) = &slot.staged {
                s.gpu_bytes += m.bytes;
            }
            s.gpu_bytes += slot.own.values().map(|v| v.1).sum::<u64>();
            if let ShadedCeiling::Ready(_, b) = slot.shaded_ceiling {
                s.gpu_bytes += b;
            }
            if let Bake::Ready(_, b) = slot.bake {
                s.gpu_bytes += b;
                s.baked += 1;
            }
            if slot.mesh.as_ref().is_some_and(|m| m.sig.far) {
                s.far += 1;
            }
            s.pending += usize::from(
                slot.pending.is_some() || slot.ready.is_some() || slot.staged.is_some(),
            );
            s.in_flight += usize::from(slot.pending.is_some());
            s.awaiting_upload += usize::from(slot.ready.is_some());
            s.staged += usize::from(slot.staged.is_some());
            s.images_waiting += usize::from(!slot.images.is_empty());
            s.failed += usize::from(slot.block_failed || slot.failed.is_some());
        }
        self.stats = s;
    }

    /// Records the visible terrain into `pass`: layered chunks (lit passes) or meshes.
    /// `camera` is the eye in the render frame. Every chunk takes the renderer's automatic
    /// light selection, which follows the pass's reach entries (`lighting.md` §8.5: ρ, key
    /// lights first, baked-landscape lights never on a terrain or ceiling chunk).
    pub fn draw(
        &mut self,
        renderer: &Renderer<'_>,
        pass: &mut Pass,
        anchor: &Anchor,
        frustum: &Frustum,
        camera: Vec3,
    ) {
        let frame = self.frame;
        let mut drawn = 0usize;
        let mut ceilings = 0usize;
        let mut triangles = 0u64;
        let material = Material::color([1.0, 1.0, 1.0, 1.0]);
        let size = LANDBLOCK_SIZE as f32;
        // Nearest first, so the depth test rejects hidden terrain before it is shaded.
        let mut order: Vec<(f32, LandblockKey)> = Vec::with_capacity(self.slots.len());
        for key in self.slots.keys() {
            if let Some(off) = anchor.offset_of(*key) {
                let near = Vec3::new(
                    camera.x.clamp(off.x, off.x + size),
                    camera.y.clamp(off.y, off.y + size),
                    camera.z,
                );
                order.push(((near - camera).length(), *key));
            }
        }
        order.sort_unstable_by(|a, b| a.0.total_cmp(&b.0).then(a.1.cmp(&b.1)));
        for (horizontal, key) in order {
            let Some(slot) = self.slots.get_mut(&key) else {
                continue;
            };
            let Some(off) = anchor.offset_of(key) else {
                continue;
            };
            // The landscape extent (`walker.md` §11.3): blocks whose square is farther.
            if horizontal > frame.extent {
                continue;
            }
            let tf = Mat4::from_translation(off);
            // §4.9 rule 3: a closed block's layerless terrain chunk is not in the scene.
            let hidden = slot.terrain_out_of_scene(&frame);
            if let Some(m) = slot.mesh.as_ref().filter(|_| !hidden) {
                let min = off + Vec3::new(0.0, 0.0, m.min_h);
                let max = off + Vec3::new(size, size, m.max_h);
                let visible = frustum.aabb_visible(min, max);
                let mid = off + Vec3::new(size * 0.5, size * 0.5, 0.5 * (m.min_h + m.max_h));
                let radius = 0.5 * (min - max).length();
                let casts = frame.shadow_radius > 0.0
                    && (mid - camera).length() - radius <= frame.shadow_radius;
                match &m.kind {
                    GpuKind::Chunk { chunk, .. } if frame.lit && (visible || casts) => {
                        let mut d = TerrainDraw::new(chunk.clone(), tf);
                        d.cast_shadow = casts;
                        d.lights = LightSelection::Auto;
                        // Scrolling layers (`terrain-texturing.md` §6.3) at the material clock.
                        if !m.sig.far
                            && !slot.scrolling.is_empty()
                            && let Some(layers) = &slot.layers
                        {
                            let mut offsets: Vec<[f32; 2]> = layers
                                .terrain
                                .layers()
                                .iter()
                                .map(|l| l.params().offsets.constant().unwrap_or([0.0; 2]))
                                .collect();
                            for (i, curves) in &mut slot.scrolling {
                                let [(cu, su), (cv, sv)] = curves;
                                if let Some(o) = offsets.get_mut(*i) {
                                    *o = [
                                        cu.evaluate_finite(frame.time, su),
                                        cv.evaluate_finite(frame.time, sv),
                                    ];
                                }
                            }
                            d.layer_offsets = Some(offsets.into());
                        }
                        pass.terrain(d);
                        drawn += usize::from(visible);
                        triangles += u64::from(chunk.info().triangles);
                    }
                    GpuKind::Mesh(id) if visible => {
                        let lit = LitSurface {
                            vertex_colors: true,
                            cast_shadow: casts,
                            ..LitSurface::default()
                        };
                        pass.mesh(MeshDraw::new(*id, tf, material).with_lit(lit));
                        drawn += 1;
                        triangles += renderer
                            .mesh_info(*id)
                            .map_or(0, |i| u64::from(i.index_count / 3));
                    }
                    _ => {}
                }
            }
            if slot.wants_shaded_ceiling(&frame) {
                if let ShadedCeiling::Ready(id, _) = slot.shaded_ceiling
                    && let Some(info) = renderer.mesh_info(id)
                {
                    let c = tf.transform_point3(info.sphere.center);
                    if frustum.sphere_visible(c, info.sphere.radius) {
                        let lit = LitSurface {
                            vertex_colors: true,
                            cast_shadow: false,
                            ..LitSurface::default()
                        };
                        pass.mesh(MeshDraw::new(id, tf, material).with_lit(lit));
                        ceilings += 1;
                        triangles += u64::from(info.index_count / 3);
                    }
                }
            } else if frame.lit
                && frame.layered
                && slot.layered_ceiling().is_some()
                && let Some(Ceiling {
                    gpu: Some((chunk, _)),
                    ..
                }) = &slot.ceiling
            {
                let info = chunk.info();
                let c = tf.transform_point3(info.sphere.center);
                if frustum.sphere_visible(c, info.sphere.radius) {
                    let mut d = TerrainDraw::new(chunk.clone(), tf);
                    d.cast_shadow = false;
                    d.lights = LightSelection::Auto;
                    pass.terrain(d);
                    ceilings += 1;
                }
            }
        }
        self.stats.drawn = drawn;
        self.stats.ceilings = ceilings;
        self.stats.triangles = triangles;
    }

    /// Stops the workers.
    pub fn shutdown(&mut self) {
        self.runner.shutdown();
    }
}

/// Puts `new` into `slot`'s staging place (freeing a mesh it replaces).
fn stage(renderer: &mut Renderer<'_>, slot: &mut Slot, new: GpuTerrain) {
    if let Some(old) = slot.staged.replace(new)
        && let GpuKind::Mesh(id) = old.kind
    {
        renderer.destroy_mesh(id);
    }
}

/// The texture ids of every drawn layer of `layers` and its light map, and whether all of
/// them have settled (on the GPU or failed). `None` while the block's own images are still
/// being uploaded.
fn chunk_textures(
    layers: &ChunkLayers,
    slot: &Slot,
    objects: &dyn TextureStore,
    _surface: GridSurface,
) -> Option<(Vec<TerrainLayerTextures>, Option<TextureId>, bool)> {
    let mut complete = true;
    let mut shared = |did: Option<Did>| -> Option<TextureId> {
        let did = did?;
        match objects.lookup(TexRequest {
            did: did.0,
            variant: VARIANT_OWN_ALPHA,
        }) {
            TexLookup::Ready(id) => Some(id),
            TexLookup::Failed => None,
            TexLookup::Pending => {
                complete = false;
                None
            }
        }
    };
    let mut out = Vec::with_capacity(layers.layers().len());
    let mut light_map = None;
    for l in layers.layers() {
        let t = l.textures();
        let tex = TerrainLayerTextures {
            diffuse: shared(t.diffuse),
            normal: shared(t.normal),
            emissive: shared(t.emissive),
            glow: shared(t.glow),
            mask: t.mask.and_then(|d| slot.own.get(&d).map(|v| v.0)),
        };
        if let Some(lm) = t.light_map {
            light_map = light_map.or_else(|| slot.own.get(&lm).map(|v| v.0));
        }
        out.push(tex);
    }
    // Incomplete while one of the chunk's own images still waits for its upload. Images
    // of the block that this chunk does not use (a far block's deferred terrain masks,
    // for its ceiling) do not count, or the ceiling of a far cave block never completes.
    if own_waiting(layers, slot) {
        complete = false;
    }
    Some((out, light_map, complete))
}

/// The render input of a layer (`docs/specs/render.md` §5.2, "the terrain crate's
/// `DrawLayer` … map onto these one to one").
fn gpu_layer<'a>(
    l: &tessera_terrain::DrawLayer,
    triangles: &'a [[u16; 3]],
    textures: TerrainLayerTextures,
) -> GpuLayer<'a> {
    let p = l.params();
    let f = l.features();
    let s = l.splat();
    let opacity = match s.opacity {
        OpacityRule::Opaque => TerrainOpacity::Opaque,
        OpacityRule::MaskedDiffuse { alpha_blend_max } => {
            TerrainOpacity::MaskedDiffuse { alpha_blend_max }
        }
        OpacityRule::Mask => TerrainOpacity::Mask,
        OpacityRule::Diffuse => TerrainOpacity::Diffuse,
        OpacityRule::One => TerrainOpacity::One,
    };
    let projection = match l.projection() {
        TextureProjection::Xz => TerrainProjection::Xz,
        TextureProjection::Yz => TerrainProjection::Yz,
        _ => TerrainProjection::Xy,
    };
    let _ = l.coordinate_set() == CoordinateSet::Xy;
    GpuLayer {
        triangles,
        projection,
        transform: TerrainTransform {
            u_scale: p.transform.u_scale,
            v_scale: p.transform.v_scale,
            rotation_degrees: p.transform.rotation_degrees,
            offset: p.offsets.constant().unwrap_or([0.0; 2]),
        },
        opacity,
        alpha_test_ref: p.alpha_test_ref,
        diffuse_color: [
            p.diffuse_colour[0],
            p.diffuse_colour[1],
            p.diffuse_colour[2],
        ],
        specular: f.specular.then_some(TerrainSpecular {
            color: [
                p.specular_colour[0],
                p.specular_colour[1],
                p.specular_colour[2],
            ],
            power: p.specular_power,
            gloss: if f.gloss_from_normal {
                TerrainGloss::NormalAlpha
            } else {
                TerrainGloss::DiffuseAlpha
            },
        }),
        glow_color: [p.glow_colour[0], p.glow_colour[1], p.glow_colour[2]],
        textures,
    }
}

/// Whether one of the masks or light maps `layers` draws with is still waiting in the
/// block's image queue.
fn own_waiting(layers: &ChunkLayers, slot: &Slot) -> bool {
    layers.layers().iter().any(|l| {
        let t = l.textures();
        [t.mask, t.light_map]
            .into_iter()
            .flatten()
            .any(|d| slot.images.iter().any(|(w, _)| *w == d))
    })
}

/// Uploads a layered chunk.
fn upload_chunk(
    renderer: &mut Renderer<'_>,
    key: LandblockKey,
    prep: &ChunkPrep,
    layers: &ChunkLayers,
    textures: &[TerrainLayerTextures],
    light_map: Option<TextureId>,
) -> Result<TerrainChunk, tessera_render::RenderError> {
    let v = &prep.vertices;
    let gpu: Vec<GpuLayer<'_>> = layers
        .layers()
        .iter()
        .zip(&prep.triangles)
        .zip(textures)
        .map(|((l, tris), tex)| gpu_layer(l, tris, *tex))
        .collect();
    let label = format!("terrain {key} {:?}", prep.surface);
    renderer.upload_terrain_chunk(&TerrainChunkData {
        label: Some(&label),
        vertices: TerrainVertices {
            positions: &v.positions,
            normals: &v.normals,
            tangents: &v.tangents,
            binormals: &v.binormals,
            sets: [&v.sets[0], &v.sets[1], &v.sets[2], &v.sets[3]],
        },
        layers: &gpu,
        light_map,
    })
}

/// Builds the layered chunk of a surface for a triangulation (`None`: every layer's own
/// level-0 triangles, the ceiling rule).
fn chunk_prep(
    block: &TerrainBlock,
    normals: &VertexNormals,
    key: LandblockKey,
    layers: &ChunkLayers,
    triangulation: Option<&[u16]>,
) -> ChunkPrep {
    let vertices = LayerVertices::build(block, normals, fixed_block_offset(key.x(), key.y()));
    let triangles = (0..layers.layers().len())
        .map(|i| match triangulation {
            Some(t) => layers.layer_indices_for(i, t).as_chunks::<3>().0.to_vec(),
            None => layers
                .layers()
                .get(i)
                .map(|l| l.triangles().to_vec())
                .unwrap_or_default(),
        })
        .collect();
    ChunkPrep {
        vertices,
        triangles,
        surface: layers.surface(),
        far: false,
    }
}

/// The far-band chunk of a surface for a triangulation: its vertices and one triangle
/// list, every triangle that some layer draws (holes stay open).
fn far_prep(
    block: &TerrainBlock,
    normals: &VertexNormals,
    key: LandblockKey,
    layers: &ChunkLayers,
    triangulation: &[u16],
) -> ChunkPrep {
    let vertices = LayerVertices::build(block, normals, fixed_block_offset(key.x(), key.y()));
    let mut seen: HashSet<[u16; 3]> = HashSet::new();
    let mut tris: Vec<[u16; 3]> = Vec::new();
    for i in 0..layers.layers().len() {
        for t in layers
            .layer_indices_for(i, triangulation)
            .as_chunks::<3>()
            .0
        {
            let mut k = *t;
            k.sort_unstable();
            if seen.insert(k) {
                tris.push(*t);
            }
        }
    }
    ChunkPrep {
        vertices,
        triangles: vec![tris],
        surface: GridSurface::Terrain,
        far: true,
    }
}

/// Uploads a far-band chunk: one opaque layer sampling the baked image at
/// (x / 160, y / 160), with the block's light map (`walker.md` §14.4).
fn upload_far_chunk(
    renderer: &mut Renderer<'_>,
    key: LandblockKey,
    prep: &ChunkPrep,
    baked: TextureId,
    light_map: Option<TextureId>,
) -> Result<TerrainChunk, tessera_render::RenderError> {
    let v = &prep.vertices;
    let empty: Vec<[u16; 3]> = Vec::new();
    let tris = prep.triangles.first().unwrap_or(&empty);
    let layer = GpuLayer {
        triangles: tris,
        projection: TerrainProjection::Xy,
        // Set 0 is (x + 160 X, y + 160 Y): scaled by 1 / 160 and moved back by (X, Y).
        transform: TerrainTransform {
            u_scale: 1.0 / 160.0,
            v_scale: 1.0 / 160.0,
            rotation_degrees: 0.0,
            offset: [-f32::from(key.x()), -f32::from(key.y())],
        },
        opacity: TerrainOpacity::Opaque,
        alpha_test_ref: 0.0,
        diffuse_color: [1.0; 3],
        specular: None,
        glow_color: [0.0; 3],
        textures: TerrainLayerTextures {
            diffuse: Some(baked),
            ..TerrainLayerTextures::default()
        },
    };
    let label = format!("terrain far {key}");
    renderer.upload_terrain_chunk(&TerrainChunkData {
        label: Some(&label),
        vertices: TerrainVertices {
            positions: &v.positions,
            normals: &v.normals,
            tangents: &[],
            binormals: &[],
            sets: [&v.sets[0], &[], &[], &v.sets[3]],
        },
        layers: &[layer],
        light_map,
    })
}

impl Job {
    /// The block and slot generation the job is for.
    fn target(&self) -> (LandblockKey, u64) {
        match self {
            Job::Block {
                key, generation, ..
            }
            | Job::Mesh {
                key, generation, ..
            }
            | Job::Ceiling {
                key, generation, ..
            }
            | Job::Bake {
                key, generation, ..
            }
            | Job::ShadedCeiling {
                key, generation, ..
            } => (*key, *generation),
        }
    }
}

fn run_job(job: Job) -> Output {
    match job {
        Job::Block {
            key,
            generation,
            bundle,
            shading,
            inputs,
        } => match guarded(|| build_block(&bundle, &shading, &inputs)) {
            Ok(product) => Output::Block {
                key,
                generation,
                product: Box::new(product),
            },
            Err(reason) => Output::BlockFailed {
                key,
                generation,
                reason,
            },
        },
        Job::Mesh {
            key,
            generation,
            sig,
            block,
            neighbours,
            shading,
            config,
            layers,
            far,
        } => {
            let mesh = guarded(|| {
                let n = Neighbours(std::array::from_fn(|i| neighbours[i].as_deref()));
                let normals = VertexNormals::compute(&block, &n);
                let edges = EdgeLods(sig.edges.map(Some));
                if let Some(layers) = layers.as_ref().filter(|_| far) {
                    let tri = triangulate(&block, sig.lod, edges);
                    let prep = far_prep(&block, &normals, key, layers, &tri);
                    let (lo, hi) = height_range(&block);
                    return Ok((Built::Chunk(Arc::new(prep)), lo, hi));
                }
                if let Some(layers) = layers {
                    let tri = triangulate(&block, sig.lod, edges);
                    let prep = chunk_prep(&block, &normals, key, &layers, Some(&tri));
                    let (lo, hi) = height_range(&block);
                    return Ok((Built::Chunk(Arc::new(prep)), lo, hi));
                }
                let mesh = TerrainMesh::build(&block, &normals, &shading, sig.lod, edges);
                let (lo, hi) = (mesh.min_height, mesh.max_height);
                terrain_mesh_data(mesh, key)
                    .prepare(&config)
                    .map(|p| (Built::Mesh(p), lo, hi))
                    .map_err(|e| e.to_string())
            })
            .and_then(|r| r);
            Output::Mesh {
                key,
                generation,
                sig,
                mesh,
            }
        }
        Job::Bake {
            key,
            generation,
            block,
            layers,
            diffuse,
            masks,
        } => {
            let chain = guarded(|| {
                let img = bake(
                    &layers,
                    &block,
                    fixed_block_offset(key.x(), key.y()),
                    &diffuse,
                    &masks,
                    BAKE_SIZE,
                );
                mip_chain(img)
            });
            Output::Bake {
                key,
                generation,
                chain,
            }
        }
        Job::ShadedCeiling {
            key,
            generation,
            bundle,
            shading,
            config,
        } => {
            let mesh = guarded(|| {
                let ceiling = TerrainBlock::from_ceiling(&bundle.data)
                    .ok_or_else(|| "no ceiling".to_owned())?;
                shaded_ceiling_mesh(&ceiling, &shading, key)
                    .prepare(&config)
                    .map_err(|e| e.to_string())
            })
            .and_then(|r| r);
            Output::ShadedCeiling {
                key,
                generation,
                mesh,
            }
        }
        Job::Ceiling {
            key,
            generation,
            ceiling,
            layers,
        } => {
            let prep = guarded(|| {
                let normals = VertexNormals::ceiling(&ceiling, &Neighbours::NONE);
                chunk_prep(&ceiling, &normals, key, &layers, None)
            })
            .map_err(|e| tracing::warn!(%key, "ceiling chunk failed: {e}"))
            .ok()
            .map(Arc::new);
            Output::Ceiling {
                key,
                generation,
                prep,
            }
        }
    }
}

/// The lowest and highest height sample of a block.
fn height_range(block: &TerrainBlock) -> (f32, f32) {
    let mut lo = f32::INFINITY;
    let mut hi = f32::NEG_INFINITY;
    for j in 0..33 {
        for i in 0..33 {
            if let Some(h) = block.height(i, j) {
                lo = lo.min(h);
                hi = hi.max(h);
            }
        }
    }
    if lo.is_finite() && hi.is_finite() {
        (lo, hi)
    } else {
        (0.0, 0.0)
    }
}

/// The renderer streams of a terrain mesh (one submesh).
#[must_use]
pub fn terrain_mesh_data(mesh: TerrainMesh, key: LandblockKey) -> MeshData {
    let count = u32::try_from(mesh.indices.len()).unwrap_or(u32::MAX);
    MeshData {
        label: Some(format!("terrain {key}")),
        positions: mesh.positions,
        normals: Some(mesh.normals),
        tangents: None,
        uvs: Some(mesh.uvs),
        colors: Some(mesh.colors),
        indices: Indices::U16(mesh.indices),
        submeshes: vec![Submesh {
            first_index: 0,
            index_count: count,
        }],
    }
}

/// The height-shaded mesh of a ceiling grid (`walker.md` §14.4 fallback): the M3 mesh of
/// the ceiling heights at the finest level with the downward ceiling normals
/// (`terrain-texturing.md` §4.8), its triangles turned to face down.
#[must_use]
pub fn shaded_ceiling_mesh(
    ceiling: &TerrainBlock,
    shading: &HeightSlopeShading,
    key: LandblockKey,
) -> MeshData {
    let normals = VertexNormals::ceiling(ceiling, &Neighbours::NONE);
    let mut mesh = TerrainMesh::build(ceiling, &normals, shading, Lod::FINEST, EdgeLods::NONE);
    for tri in mesh.indices.as_chunks_mut::<3>().0 {
        tri.swap(1, 2);
    }
    let mut data = terrain_mesh_data(mesh, key);
    data.label = Some(format!("ceiling {key}"));
    data
}

/// Reads and decodes a block image (an A8 mask or a DXT1 light map); `None` (logged) when
/// it is missing, does not decode or is too large.
fn read_image(source: &dyn WorldSource, did: Did) -> Option<RgbaImage> {
    let bytes = match source.read(did) {
        Ok(Some(b)) => b,
        Ok(None) => {
            tracing::debug!(%did, "terrain image missing");
            return None;
        }
        Err(e) => {
            tracing::debug!(%did, "terrain image unreadable: {e}");
            return None;
        }
    };
    let s = match Surface::parse_expecting(&bytes, did) {
        Ok(s) => s,
        Err(e) => {
            tracing::debug!(%did, "terrain image does not decode: {e}");
            return None;
        }
    };
    if s.width.max(s.height) > MAX_BLOCK_IMAGE_EDGE {
        tracing::debug!(%did, width = s.width, height = s.height, "terrain image too large");
        return None;
    }
    s.to_rgba8().ok().filter(image_ok)
}

/// What [`build_layers`] gives: the layers, the decoded masks and light maps, and the
/// masks reduced for the far-band bake.
type LayerBuild = (
    Option<BlockLayers>,
    Vec<(Did, BlockImage)>,
    HashMap<Did, AlphaMask>,
);

/// Resolves the texture layers of a bundle and reads its masks and light maps
/// (`terrain-texturing.md` §4, §5, §10).
fn build_layers(bundle: &LandblockBundle, inputs: &BlockInputs) -> LayerBuild {
    let Some(table) = inputs.tables.get(&bundle.key.region()) else {
        return (None, Vec::new(), HashMap::new());
    };
    let light_maps = LightMaps {
        terrain: bundle.light_map,
        ceiling: bundle.ceiling_light_map,
    };
    let opts = ResolveOptions::default();
    let open = block_open(&bundle.data);
    // A terrain chunk whose stored layers do not resolve keeps the block's ceiling: it
    // resolves to no layer (drawn height-shaded, `walker.md` §14.4) and is not §4.9
    // rule 3, while the ceiling keeps its own layers and its rule-3 flag.
    let (mut terrain, terrain_hidden) = match resolve_surface(
        &bundle.data,
        GridSurface::Terrain,
        table,
        &light_maps,
        &opts,
        open,
    ) {
        Ok(l) => l,
        Err(e) => {
            tracing::warn!(key = %bundle.key, "terrain layers not resolved: {e}");
            match unresolved_terrain(&bundle.data, table) {
                Some(empty) => (empty, false),
                None => return (None, Vec::new(), HashMap::new()),
            }
        }
    };
    let mut ceiling_hidden = false;
    let mut ceiling = TerrainBlock::from_ceiling(&bundle.data).and_then(|c| {
        resolve_surface(
            &bundle.data,
            GridSurface::Ceiling,
            table,
            &light_maps,
            &opts,
            open,
        )
        .map_err(|e| tracing::warn!(key = %bundle.key, "ceiling layers not resolved: {e}"))
        .ok()
        .map(|(l, hidden)| {
            ceiling_hidden = hidden;
            (l, c)
        })
    });
    let mut images: Vec<(Did, BlockImage)> = Vec::new();
    let mut seen: HashSet<Did> = HashSet::new();
    let Some(source) = inputs.source.as_deref() else {
        // No record source: every layer drawn without its mask.
        for i in 0..terrain.layers().len() {
            if let Some(l) = terrain.layer_mut(i) {
                l.drop_mask();
            }
        }
        if let Some((c, _)) = ceiling.as_mut() {
            for i in 0..c.layers().len() {
                if let Some(l) = c.layer_mut(i) {
                    l.drop_mask();
                }
            }
        }
        return (
            Some(BlockLayers {
                terrain: Arc::new(terrain),
                ceiling: ceiling.map(|(l, c)| (Arc::new(l), Arc::new(c))),
                terrain_hidden,
                ceiling_hidden,
            }),
            images,
            HashMap::new(),
        );
    };
    let mut bake_masks: HashMap<Did, AlphaMask> = HashMap::new();
    // Masks: checked (A8) before use, dropped (the layer drawn without one) otherwise.
    let check = |chunk: &mut ChunkLayers,
                 images: &mut Vec<(Did, BlockImage)>,
                 seen: &mut HashSet<Did>,
                 mut bake_masks: Option<&mut HashMap<Did, AlphaMask>>| {
        for i in 0..chunk.layers().len() {
            let Some(layer) = chunk.layer_mut(i) else {
                continue;
            };
            let Some(did) = layer.mask() else { continue };
            let Ok(Some(bytes)) = source.read(did) else {
                tracing::debug!(%did, "terrain mask missing; layer drawn without one");
                layer.drop_mask();
                continue;
            };
            let Ok(surface) = Surface::parse_expecting(&bytes, did) else {
                tracing::debug!(%did, "terrain mask does not decode; layer drawn without one");
                layer.drop_mask();
                continue;
            };
            if layer.check_mask(&surface).is_err() {
                continue;
            }
            // Masks are at most 512 square in the data (`terrain-texturing.md` T14); one
            // above the block-image cap is rejected like any unusable mask (§5.1).
            if surface.width.max(surface.height) > MAX_BLOCK_IMAGE_EDGE {
                tracing::debug!(%did, "terrain mask too large; layer drawn without one");
                layer.drop_mask();
                continue;
            }
            if let Some(bm) = bake_masks.as_deref_mut()
                && !bm.contains_key(&did)
                && let Ok(m) = AlphaMask::from_surface(&surface)
            {
                bm.insert(did, reduce_mask(&m));
            }
            if seen.insert(did) {
                // `check_mask` accepted it: A8 with width × height texels.
                images.push((
                    did,
                    BlockImage::Mask {
                        width: surface.width,
                        height: surface.height,
                        alpha: surface.data.to_vec(),
                    },
                ));
            }
        }
    };
    check(&mut terrain, &mut images, &mut seen, Some(&mut bake_masks));
    if let Some((c, _)) = ceiling.as_mut() {
        check(c, &mut images, &mut seen, None);
    }
    for did in [bundle.light_map, bundle.ceiling_light_map]
        .into_iter()
        .flatten()
    {
        if seen.insert(did)
            && let Some(img) = read_image(source, did)
        {
            images.push((did, BlockImage::Rgba(img)));
        }
    }
    (
        Some(BlockLayers {
            terrain: Arc::new(terrain),
            ceiling: ceiling.map(|(l, c)| (Arc::new(l), Arc::new(c))),
            terrain_hidden,
            ceiling_hidden,
        }),
        images,
        bake_masks,
    )
}

/// Builds the terrain block, the overview tile, the objects and the texture layers of a
/// bundle.
#[must_use]
pub fn build_block(
    bundle: &LandblockBundle,
    shading: &HeightSlopeShading,
    inputs: &BlockInputs,
) -> BlockProduct {
    let block = TerrainBlock::from_landblock(&bundle.data);
    let _ = block.lod_errors();
    let overview = overview_tile(&block, shading);
    let (instances, objects) = block_objects(bundle, inputs.lod_classes.as_deref());
    let (layers, images, bake_masks) = build_layers(bundle, inputs);
    BlockProduct {
        block: Arc::new(block),
        overview,
        instances: instances.into(),
        objects: objects.into(),
        layers,
        images,
        bake_masks,
    }
}

/// The drawable objects of a bundle with their part instances (`entity-descriptor.md`
/// §9, `scenery.md` §7.6) and LOD-class values (`walker.md` §11.4.3), then the render
/// meshes of its interior cells (`landblock.md` §4.14). Each object names the cell it is
/// drawn through (`walker.md` §14.12 (e)–(h)): outdoor objects cell 0; interior entities the cell
/// of the block's collision set whose volume contains their origin, or
/// [`interiors::UNATTACHED`]; a cell mesh its own cell. Which interior objects are loaded
/// and drawn is the object layer's decision ([`ObjectFrame::interior`]).
///
/// [`ObjectFrame::interior`]: super::objects::ObjectFrame::interior
#[must_use]
pub fn block_objects(
    bundle: &LandblockBundle,
    classes: Option<&LodClasses>,
) -> (Vec<Instance>, Vec<ObjectInfo>) {
    let mut instances = Vec::new();
    let mut objects = Vec::new();
    let generated_start = bundle.entities.len();
    let lists = bundle
        .entities
        .iter()
        .enumerate()
        .map(|(i, e)| (i, e, false))
        .chain(
            bundle
                .generated
                .iter()
                .enumerate()
                .map(|(i, e)| (generated_start + i, e, true)),
        );
    let default = LodClasses::default();
    let classes = classes.unwrap_or(&default);
    for (_, e, generated) in lists {
        if !e.draw {
            continue;
        }
        let Some(visual) = &e.visual else { continue };
        let q = e.placement.rotation;
        let rot = Quat::from_xyzw(q.x, q.y, q.z, q.w);
        let rot = if rot.is_finite() && rot.length_squared() > 0.0 {
            rot.normalize()
        } else {
            Quat::IDENTITY
        };
        let scale = Vec3::from_array(e.scale);
        let scale = if scale.is_finite() && scale.min_element() > 0.0 {
            scale
        } else {
            Vec3::ONE
        };
        let pos = Vec3::from_array(e.placement.position);
        if !pos.is_finite() || visual.parts.is_empty() {
            continue;
        }
        let model = Mat4::from_scale_rotation_translation(scale, rot, pos);
        let Ok(object) = u32::try_from(objects.len()) else {
            break;
        };
        let first = u32::try_from(instances.len()).unwrap_or(u32::MAX);
        for &part in visual.parts.iter() {
            instances.push(Instance {
                part: part.0,
                model,
                scale: scale.max_element(),
                figure: e.figure,
                object,
            });
        }
        let class = bundle.lod_class_of(e);
        let cell = if e.interior {
            entity_cell(bundle, pos)
        } else {
            interiors::OUTDOORS
        };
        objects.push(ObjectInfo {
            position: pos,
            lod: classes.values(class, false),
            generated,
            first,
            count: u32::try_from(visual.parts.len()).unwrap_or(u32::MAX),
            cell,
        });
    }
    cell_objects(bundle, &mut instances, &mut objects);
    (instances, objects)
}

/// The LOD values of a cell mesh (`landblock.md` §4.14 OPEN on cell detail levels and
/// draw distances; Tessera Decision of `walker.md` §14.12 (g)): drawn out to the global limit
/// *G* and the distance cap, always at the finest level, casting shadows within the
/// component's default shadow distance.
#[must_use]
pub fn cell_lod() -> LodValues {
    let mut v = LodValues::default();
    v.0[0] = CELL_DRAW_DISTANCE;
    v.0[1] = CELL_DRAW_DISTANCE;
    v
}

/// `d0` and `d1` of a cell mesh: farther than any global limit.
const CELL_DRAW_DISTANCE: f32 = 1.0e6;

/// The cell of the block's collision set whose volume contains an interior entity's
/// origin (`entity-descriptor.md` §4.8: containment is the fallback attachment, the
/// stored rule is OPEN), tried at the origin and 0.05 m above and below it;
/// [`interiors::UNATTACHED`] when none does or the block has no collision set.
fn entity_cell(bundle: &LandblockBundle, pos: Vec3) -> u16 {
    let Some(col) = bundle.collision.as_deref() else {
        return interiors::UNATTACHED;
    };
    [0.0f32, 0.05, -0.05]
        .into_iter()
        .find_map(|dz| col.cell_at([pos.x, pos.y, pos.z + dz]).map(|c| c.index))
        .filter(|&c| c != interiors::OUTDOORS && c != interiors::UNATTACHED)
        .unwrap_or(interiors::UNATTACHED)
}

/// Appends one object per placed interior cell of the bundle's landblock info: its 0x01
/// geometry as the part (the object layer draws its embedded render mesh,
/// `landblock.md` §4.14), placed by the cell's position and rotation without its
/// adjustable scale (§4.14 Tessera Decision). A cell whose position has no x, y, z offset
/// (or no orientation) is placed at the landblock origin (with the identity), as §4.4 gives
/// (L42); cells numbered 0 or 0xFFFF are left out (LI8).
fn cell_objects(
    bundle: &LandblockBundle,
    instances: &mut Vec<Instance>,
    objects: &mut Vec<ObjectInfo>,
) {
    let Some(info) = bundle.info.as_ref() else {
        return;
    };
    let lod = cell_lod();
    for c in info.cells() {
        if c.index == interiors::OUTDOORS || c.index == interiors::UNATTACHED {
            continue;
        }
        // An absent offset is (0, 0, 0) and an absent orientation the identity
        // (`landblock.md` §4.4, L42): such a cell lies at the landblock origin.
        let Ok(placement) = tessera_physics::geometry::CellPlacement::new(
            c.position.offset().unwrap_or([0.0; 3]),
            c.rotation()
                .unwrap_or(tessera_formats::setup::Quaternion::IDENTITY),
        ) else {
            continue;
        };
        let q = placement.rotation;
        let rot = Quat::from_xyzw(q.x, q.y, q.z, q.w);
        let pos = Vec3::from_array(placement.position);
        if !rot.is_finite() || !pos.is_finite() || c.geometry.is_null() {
            continue;
        }
        let (Ok(object), Ok(first)) =
            (u32::try_from(objects.len()), u32::try_from(instances.len()))
        else {
            break;
        };
        instances.push(Instance {
            part: c.geometry.0,
            model: Mat4::from_rotation_translation(rot, pos),
            scale: 1.0,
            figure: None,
            object,
        });
        objects.push(ObjectInfo {
            position: pos,
            lod,
            generated: false,
            first,
            count: 1,
            cell: c.index,
        });
    }
}

/// A 33 × 33 height/slope-shaded image of a block, north row first (the minimap
/// convention, `coordinates.md` §10), with a little directional shading so relief shows.
#[must_use]
pub fn overview_tile(block: &TerrainBlock, shading: &HeightSlopeShading) -> Vec<u8> {
    let normals = VertexNormals::compute(block, &Neighbours::NONE);
    let edge = OVERVIEW_EDGE as usize;
    let mut out = Vec::with_capacity(edge * edge * 4);
    let light = Vec3::new(-0.5, 0.5, 0.7).normalize();
    for row in 0..edge {
        let j = edge - 1 - row;
        for i in 0..edge {
            let h = block.height(i, j).unwrap_or(0.0);
            let n = normals.get(i, j).unwrap_or([0.0, 0.0, 1.0]);
            let c = shading.color(h, n[2]);
            let lit = (Vec3::from_array(n).dot(light) * 0.6 + 0.55).clamp(0.3, 1.2);
            out.extend_from_slice(&[
                linear_to_srgb8(c[0] * lit),
                linear_to_srgb8(c[1] * lit),
                linear_to_srgb8(c[2] * lit),
                255,
            ]);
        }
    }
    out
}

#[cfg(test)]
mod tests {
    use super::*;
    use tessera_formats::landblock::{HeightGrid, VertexFlags};

    #[test]
    fn overview_is_north_up_and_sized() {
        let block = TerrainBlock::new(
            &HeightGrid::from_fn(|_, j| (1000 + 2000 * j) as u16),
            &VertexFlags::default(),
        );
        let img = overview_tile(&block, &HeightSlopeShading::default());
        assert_eq!(img.len(), 33 * 33 * 4);
        assert!(img.as_chunks::<4>().0.iter().all(|p| p[3] == 255));
    }

    #[test]
    fn terrain_mesh_streams() {
        let block = TerrainBlock::flat(10_000);
        let normals = VertexNormals::compute(&block, &Neighbours::NONE);
        let mesh = TerrainMesh::build(
            &block,
            &normals,
            &HeightSlopeShading::default(),
            Lod::new(3).unwrap(),
            EdgeLods::NONE,
        );
        let key = LandblockKey::new(1, 2, 3).unwrap();
        let data = terrain_mesh_data(mesh, key);
        let p = data.prepare(&MeshConfig::default()).unwrap();
        assert_eq!(p.info().index_count as usize % 3, 0);
        assert!(p.info().index_count > 0);
    }

    fn lod(n: u8) -> Lod {
        Lod::new(n).unwrap()
    }

    fn key(x: u8, y: u8) -> LandblockKey {
        LandblockKey::new(1, x, y).unwrap()
    }

    fn view(shown: Option<[u8; 4]>, staged: Option<[u8; 4]>) -> EdgeView {
        EdgeView {
            shown: shown.map(|e| e.map(lod)),
            staged: staged.map(|e| e.map(lod)),
            frozen: false,
        }
    }

    // Edge order: south, east, north, west. (10, 10) and (11, 10) share A's east / B's west.
    #[test]
    fn staged_meshes_wait_for_their_neighbours() {
        let (a, b) = (key(10, 10), key(11, 10));
        // A's new mesh meshes the seam at level 2, B still draws a mesh built for level 1.
        let mut views = HashMap::from([
            (a, view(Some([1; 4]), Some([2, 2, 2, 2]))),
            (b, view(Some([1; 4]), None)),
        ]);
        assert!(promotable(&views).is_empty(), "A must wait for B");
        // B's matching mesh arrives: both swap in the same frame.
        views.insert(b, view(Some([1; 4]), Some([1, 1, 1, 2])));
        let p = promotable(&views);
        assert!(p.contains(&a) && p.contains(&b));
        // B's staged mesh disagrees with A's: neither may swap.
        views.insert(b, view(Some([1; 4]), Some([1, 1, 1, 3])));
        assert!(promotable(&views).is_empty());
        // A block that draws nothing does not hold anyone back.
        views.insert(b, view(None, None));
        assert_eq!(promotable(&views), HashSet::from([a]));
        // Nor does a frozen (failed) one.
        views.insert(
            b,
            EdgeView {
                frozen: true,
                ..view(Some([1; 4]), None)
            },
        );
        assert_eq!(promotable(&views), HashSet::from([a]));
    }

    #[test]
    fn promotion_chains_are_resolved() {
        // A row of three: C's staged mesh disagrees with D's drawn one, so C stays, and B
        // (which agrees only with C's staged mesh) must stay too; A only borders B's drawn
        // mesh, which it agrees with.
        let (a, b, c, d) = (key(1, 1), key(2, 1), key(3, 1), key(4, 1));
        let views = HashMap::from([
            (a, view(Some([0; 4]), Some([0, 0, 0, 0]))),
            (b, view(Some([0; 4]), Some([0, 1, 0, 0]))),
            (c, view(Some([0; 4]), Some([0, 1, 0, 1]))),
            (d, view(Some([0; 4]), None)),
        ]);
        assert_eq!(promotable(&views), HashSet::from([a]));
    }

    #[test]
    fn failures_back_off() {
        let bundle = Arc::new(bundle_at(key(5, 5), 0));
        let mut slot = Slot::new(bundle, 1);
        let sig = MeshSig {
            lod: lod(0),
            edges: [lod(0); 4],
            present: 0,
            layered: false,
            far: false,
        };
        let now = Instant::now();
        assert!(slot.wants_mesh(sig, now));
        assert_eq!(slot.fail(sig, now), 1);
        assert!(!slot.wants_mesh(sig, now), "same signature: wait");
        assert!(
            slot.wants_mesh(sig, now + FIRST_BACKOFF),
            "retried after the back-off"
        );
        let other = MeshSig { lod: lod(1), ..sig };
        assert!(
            slot.wants_mesh(other, now),
            "a new signature is tried at once"
        );
        let layered = MeshSig {
            layered: true,
            ..sig
        };
        assert!(slot.wants_mesh(layered, now), "layers on: a new signature");
        let mut last = now;
        for _ in 0..40 {
            slot.fail(sig, now);
            let at = slot.failed.unwrap().retry_at;
            assert!(at >= last);
            last = at;
        }
        assert!(last <= now + MAX_BACKOFF);
    }

    #[test]
    fn panics_become_errors() {
        assert_eq!(guarded(|| 3), Ok(3));
        let e = guarded(|| -> u32 { panic!("bad block") }).unwrap_err();
        assert!(e.contains("bad block"), "{e}");
    }

    /// `walker.md` §14.4 far band: a band of 40 m or less still lets a block leave it
    /// (review finding: the exit distance was ≤ 0).
    #[test]
    fn far_band_hysteresis_keeps_a_positive_exit() {
        assert!(!in_far_band(false, 470.0, 480.0));
        assert!(in_far_band(false, 490.0, 480.0));
        assert!(
            in_far_band(true, 450.0, 480.0),
            "inside the 40 m hysteresis"
        );
        assert!(!in_far_band(true, 430.0, 480.0));
        for band in [40.0, 20.0, 1.0] {
            assert!(in_far_band(true, 0.6 * band, band));
            assert!(
                !in_far_band(true, 0.0, band),
                "the block under the camera leaves"
            );
            assert!(!in_far_band(true, 0.4 * band, band));
        }
        assert!(!in_far_band(true, 1.0e6, 0.0), "off");
        assert!(!in_far_band(true, 1.0e6, f32::NAN));
    }

    /// The ceiling's height-shaded fallback faces down and uses the ceiling heights.
    #[test]
    fn shaded_ceilings_face_down() {
        use tessera_formats::landblock::{Ceiling, LandblockData};
        let k = key(5, 5);
        let mut data = LandblockData::from_heights(k.data_did(), HeightGrid::flat(1_000));
        data.ceiling = Some(Ceiling {
            heights: HeightGrid::flat(2_000),
            ..Ceiling::default()
        });
        let ceiling = TerrainBlock::from_ceiling(&data).expect("ceiling");
        let shading = HeightSlopeShading::default();
        let mesh = shaded_ceiling_mesh(&ceiling, &shading, k);
        let Indices::U16(idx) = &mesh.indices else {
            panic!("u16 indices");
        };
        assert!(!idx.is_empty());
        let p = |i: u16| Vec3::from_array(mesh.positions[usize::from(i)]);
        for t in idx.as_chunks::<3>().0 {
            let n = (p(t[1]) - p(t[0])).cross(p(t[2]) - p(t[0]));
            assert!(n.z < 0.0, "counter-clockwise seen from below");
        }
        let z = ceiling.height(0, 0).expect("height");
        assert!(mesh.positions.iter().all(|q| (q[2] - z).abs() < 1e-3));
        let normals = mesh.normals.as_ref().expect("normals");
        assert!(normals.iter().all(|n| n[2] < 0.0), "downward normals");
    }

    #[test]
    fn a_chunk_without_layers_is_drawn_height_shaded() {
        use tessera_formats::landblock::{SurfaceChunk, SurfaceLayer};
        use tessera_formats::terrain_type::{TerrainType, TerrainTypeTable};
        let table = TerrainTypeTable::new(
            Did(0x1900_0000),
            vec![TerrainType {
                key: 0,
                ..TerrainType::default()
            }],
        )
        .expect("table");
        let resolve = |chunk: &SurfaceChunk| {
            tessera_terrain::resolve_chunk(
                chunk,
                &VertexFlags::default(),
                GridSurface::Terrain,
                &table,
                None,
                &ResolveOptions::default(),
            )
            .expect("resolves")
        };
        let k = key(5, 5);
        let mut slot = Slot::new(Arc::new(bundle_at(k, 0)), 1);
        assert!(!slot.has_terrain_layers(), "no table: not layered");
        slot.layers = Some(BlockLayers {
            terrain: Arc::new(resolve(&SurfaceChunk::default())),
            ceiling: None,
            terrain_hidden: false,
            ceiling_hidden: false,
        });
        assert!(
            !slot.has_terrain_layers(),
            "an empty stack is drawn like one that does not resolve (review finding)"
        );
        assert!(!slot.terrain_hidden());
        let mut chunk = SurfaceChunk::default();
        chunk.layers.push(SurfaceLayer {
            material: 0,
            whole: true,
            ..SurfaceLayer::default()
        });
        slot.layers = Some(BlockLayers {
            terrain: Arc::new(resolve(&chunk)),
            ceiling: None,
            terrain_hidden: false,
            ceiling_hidden: false,
        });
        assert!(slot.has_terrain_layers());
    }

    /// `terrain-texturing.md` §4.9: the open flag, the lowest-`draw_order` entry and the
    /// one-layer draw of a chunk without layers.
    #[test]
    fn chunks_without_layers_follow_the_open_flag() {
        use tessera_formats::landblock::{Ceiling, LandblockData};
        use tessera_formats::terrain_type::{TerrainType, TerrainTypeTable};
        let entry = |key, draw_order| TerrainType {
            key,
            draw_order,
            ..TerrainType::default()
        };
        let table = TerrainTypeTable::new(
            Did(0x1900_0000),
            vec![entry(0, 5), entry(1, 2), entry(2, 9), entry(3, 2)],
        )
        .expect("table");
        assert_eq!(layerless_key(&table), Some(3), "a tie goes to the last met");
        let empty = TerrainTypeTable::new(Did(0x1900_0000), Vec::new()).expect("table");
        assert_eq!(layerless_key(&empty), None);

        let k = key(5, 5);
        let mut data = LandblockData::from_heights(k.data_did(), HeightGrid::flat(1_000));
        assert!(block_open(&data), "no ceiling");
        // A ceiling at or below the terrain everywhere it is tested: closed. The last row
        // and column (index 32) are not tested.
        data.ceiling = Some(Ceiling {
            heights: HeightGrid::from_fn(|i, j| if i == 32 || j == 32 { 2_000 } else { 1_000 }),
            ..Ceiling::default()
        });
        assert!(!block_open(&data));
        if let Some(c) = data.ceiling.as_mut() {
            c.heights = HeightGrid::from_fn(|i, j| if (i, j) == (31, 0) { 1_001 } else { 0 });
        }
        assert!(block_open(&data), "one sample of open cave");

        let lm = LightMaps {
            terrain: Some(Did(0x4100_0001)),
            ceiling: None,
        };
        let opts = ResolveOptions::default();
        let (open, hidden) =
            resolve_surface(&data, GridSurface::Terrain, &table, &lm, &opts, true).unwrap();
        assert!(!hidden);
        assert_eq!(open.layers().len(), 1);
        let l = &open.layers()[0];
        assert_eq!(l.key(), 3);
        assert!(l.whole() && l.first_drawn() && l.mask().is_none());
        assert_eq!(l.light_map(), Some(Did(0x4100_0001)));
        assert!(open.uncovered().is_empty(), "the full grid");
        let (ceiling, _) =
            resolve_surface(&data, GridSurface::Ceiling, &table, &lm, &opts, true).unwrap();
        assert_eq!(
            ceiling.layers().len(),
            1,
            "a layerless ceiling of an open block"
        );
        let (closed, hidden) =
            resolve_surface(&data, GridSurface::Terrain, &table, &lm, &opts, false).unwrap();
        assert!(hidden && closed.is_empty());
        // An open block whose table has no entry: nothing to draw with, but not rule 3
        // (drawn height-shaded like a region without a table, review finding).
        let (none, hidden) =
            resolve_surface(&data, GridSurface::Terrain, &empty, &lm, &opts, true).unwrap();
        assert!(!hidden && none.is_empty());
        let (_, hidden) =
            resolve_block_surface(&data, GridSurface::Terrain, &empty, &lm, &opts).unwrap();
        assert!(!hidden, "the block is open");
        // A chunk that stores layers resolves them, whatever the flag.
        data.terrain.layers.push(SurfaceLayer {
            material: 0,
            whole: true,
            ..SurfaceLayer::default()
        });
        let (stored, hidden) =
            resolve_surface(&data, GridSurface::Terrain, &table, &lm, &opts, false).unwrap();
        assert!(!hidden);
        assert_eq!(stored.layers()[0].key(), 0);

        let mut slot = Slot::new(Arc::new(bundle_at(k, 0)), 1);
        slot.layers = Some(BlockLayers {
            terrain: Arc::new(closed),
            ceiling: None,
            terrain_hidden: true,
            ceiling_hidden: false,
        });
        assert!(slot.terrain_hidden() && !slot.has_terrain_layers());
    }

    #[test]
    fn a_chunk_waits_only_for_its_own_images() {
        use tessera_formats::landblock::{SurfaceChunk, SurfaceLayer};
        use tessera_formats::terrain_type::{TerrainType, TerrainTypeTable};
        let table = TerrainTypeTable::new(
            Did(0x1900_0000),
            vec![TerrainType {
                key: 0,
                ..TerrainType::default()
            }],
        )
        .expect("table");
        let own = Did(0x4100_0002);
        let mut chunk = SurfaceChunk::default();
        for mask in [Did(0), own] {
            chunk.layers.push(SurfaceLayer {
                material: 0,
                whole: true,
                mask,
                ..SurfaceLayer::default()
            });
        }
        let ceiling = tessera_terrain::resolve_chunk(
            &chunk,
            &VertexFlags::default(),
            GridSurface::Ceiling,
            &table,
            None,
            &ResolveOptions::default(),
        )
        .expect("resolves");
        assert_eq!(ceiling.layers()[1].textures().mask, Some(own));
        let k = key(5, 5);
        let mut slot = Slot::new(Arc::new(bundle_at(k, 0)), 1);
        let mask = |did| {
            (
                did,
                BlockImage::Mask {
                    width: 1,
                    height: 1,
                    alpha: vec![0],
                },
            )
        };
        // A far block's deferred terrain mask does not hold the ceiling back (review: the
        // ceiling of a far cave block never completed, so the walker never settled).
        slot.images = vec![mask(Did(0x4100_0001))];
        assert!(!own_waiting(&ceiling, &slot));
        slot.images.push(mask(own));
        assert!(own_waiting(&ceiling, &slot));
    }

    #[test]
    fn masks_wait_as_a8_and_expand_like_the_surface_decoder() {
        let m = BlockImage::Mask {
            width: 2,
            height: 1,
            alpha: vec![7, 200],
        };
        assert_eq!(m.upload_bytes(), 8);
        let rgba = m.to_rgba().expect("well-formed");
        assert_eq!(rgba.pixels, vec![255, 255, 255, 7, 255, 255, 255, 200]);
        assert_eq!((rgba.width, rgba.height), (2, 1));
        // The expansion matches `Surface::to_rgba8` for A8.
        let surface = tessera_formats::Surface {
            did: Did(0x4100_0001),
            category: 0,
            format: tessera_formats::SurfaceFormat::A8,
            width: 2,
            height: 1,
            data: &[7, 200],
        };
        assert_eq!(*rgba, surface.to_rgba8().unwrap());
        let bad = BlockImage::Mask {
            width: 3,
            height: 3,
            alpha: vec![0; 4],
        };
        assert!(bad.to_rgba().is_none());
        let empty = BlockImage::Mask {
            width: 0,
            height: 0,
            alpha: Vec::new(),
        };
        assert!(empty.to_rgba().is_none());
    }

    #[test]
    fn layered_chunks_cover_the_grid_once() {
        use tessera_formats::landblock::{SurfaceChunk, SurfaceLayer};
        use tessera_formats::terrain_type::{TerrainType, TerrainTypeTable};
        let block = TerrainBlock::flat(10_000);
        let normals = VertexNormals::compute(&block, &Neighbours::NONE);
        let mut chunk = SurfaceChunk::default();
        chunk.layers.push(SurfaceLayer {
            material: 0,
            whole: true,
            ..SurfaceLayer::default()
        });
        let table = TerrainTypeTable::new(
            Did(0x1900_0000),
            vec![TerrainType {
                key: 0,
                ..TerrainType::default()
            }],
        )
        .expect("table");
        let layers = tessera_terrain::resolve_chunk(
            &chunk,
            &VertexFlags::default(),
            GridSurface::Terrain,
            &table,
            None,
            &ResolveOptions::default(),
        )
        .expect("one whole layer");
        let k = key(3, 4);
        for level in 0..=5u8 {
            let tri = triangulate(&block, lod(level), EdgeLods::NONE);
            let prep = chunk_prep(&block, &normals, k, &layers, Some(&tri));
            assert_eq!(prep.triangles.len(), 1);
            assert_eq!(prep.triangles[0].len() * 3, tri.len(), "level {level}");
            assert_eq!(prep.vertices.positions.len(), 33 * 33);
        }
        let ceiling = chunk_prep(&block, &normals, k, &layers, None);
        assert_eq!(ceiling.triangles[0].len(), 2048);
    }

    fn bundle_at(key: LandblockKey, base: u16) -> LandblockBundle {
        LandblockBundle::from_data(
            key,
            tessera_formats::landblock::LandblockData::from_heights(
                key.data_did(),
                HeightGrid::from_fn(|i, j| base + (i * 37 + j * 11) as u16 * 20),
            ),
        )
    }

    fn headless() -> Option<Renderer<'static>> {
        match Renderer::new_headless(&tessera_render::RendererConfig::default()) {
            Ok(r) => Some(r),
            Err(e) => {
                eprintln!("skipping: no GPU: {e}");
                None
            }
        }
    }

    fn inputs() -> Arc<BlockInputs> {
        Arc::new(BlockInputs {
            source: None,
            tables: HashMap::new(),
            lod_classes: None,
        })
    }

    /// A texture store whose textures never load.
    #[derive(Default)]
    struct NoTextures {
        refs: i64,
    }

    impl TextureStore for NoTextures {
        fn acquire(&mut self, reqs: &[TexRequest]) {
            self.refs += reqs.len() as i64;
        }
        fn release(&mut self, reqs: &[TexRequest]) {
            self.refs -= reqs.len() as i64;
        }
        fn lookup(&self, _t: TexRequest) -> TexLookup {
            TexLookup::Failed
        }
        fn mean(&self, _t: TexRequest) -> Option<[f32; 3]> {
            None
        }
        fn small(&self, _t: TexRequest) -> Option<Arc<[RgbaImage]>> {
            None
        }
    }

    fn objects(renderer: &Renderer<'_>) -> NoTextures {
        let _ = renderer;
        NoTextures::default()
    }

    /// The surface chunks and table of the §4.9 and unresolved-layer tests: a table with
    /// key 0, a chunk with one whole layer of it, and a hostile chunk whose index window
    /// lies outside its index list (it does not resolve).
    fn layer_fixture() -> (
        Arc<TerrainTypeTable>,
        tessera_formats::landblock::SurfaceChunk,
        tessera_formats::landblock::SurfaceChunk,
    ) {
        use tessera_formats::landblock::{SurfaceChunk, SurfaceLayer};
        use tessera_formats::terrain_type::TerrainType;
        let table = TerrainTypeTable::new(
            Did(0x1900_0000),
            vec![TerrainType {
                key: 0,
                ..TerrainType::default()
            }],
        )
        .expect("table");
        let whole = SurfaceChunk {
            layers: vec![SurfaceLayer {
                material: 0,
                whole: true,
                ..SurfaceLayer::default()
            }],
            ..SurfaceChunk::default()
        };
        let hostile = SurfaceChunk {
            layers: vec![SurfaceLayer {
                material: 0,
                prims: 1,
                vertex_span: 3,
                ..SurfaceLayer::default()
            }],
            ..SurfaceChunk::default()
        };
        (Arc::new(table), whole, hostile)
    }

    /// A bundle at `k` with terrain heights 1,000, terrain chunk `terrain` and a ceiling at
    /// `ceiling_raw` with chunk `ceiling` (above the terrain: open; below: closed).
    fn cave_bundle(
        k: LandblockKey,
        terrain: tessera_formats::landblock::SurfaceChunk,
        ceiling: tessera_formats::landblock::SurfaceChunk,
        ceiling_raw: u16,
    ) -> LandblockBundle {
        use tessera_formats::landblock::{Ceiling, LandblockData};
        let mut data = LandblockData::from_heights(k.data_did(), HeightGrid::flat(1_000));
        data.terrain = terrain;
        data.ceiling = Some(Ceiling {
            heights: HeightGrid::flat(ceiling_raw),
            surface: ceiling,
            ..Ceiling::default()
        });
        LandblockBundle::from_data(k, data)
    }

    fn table_inputs(region: u8, table: &Arc<TerrainTypeTable>) -> BlockInputs {
        BlockInputs {
            source: None,
            tables: HashMap::from([(region, Arc::clone(table))]),
            lod_classes: None,
        }
    }

    /// Review finding (terrain.rs `build_layers`): a terrain chunk whose stored layers do
    /// not resolve no longer takes the ceiling's layers and rule-3 flag with it. The
    /// ceiling's height-shaded fallback is drawn only where `walker.md` §14.4 OPEN (b)
    /// allows it, and a rule-3 ceiling draws nothing in the normal view.
    #[test]
    fn an_unresolved_terrain_chunk_keeps_the_ceiling() {
        use tessera_formats::landblock::SurfaceChunk;
        let (table, whole, hostile) = layer_fixture();
        let k = key(5, 5);
        let inputs = table_inputs(k.region(), &table);
        let frame = TerrainFrame::default();
        let slot_of = |bundle: LandblockBundle| {
            let (layers, _, _) = build_layers(&bundle, &inputs);
            let block = Arc::new(TerrainBlock::from_landblock(&bundle.data));
            let mut slot = Slot::new(Arc::new(bundle), 1);
            slot.block = Some(block);
            slot.layers = layers;
            slot
        };

        // Open block, the ceiling's own layer resolves: drawn with it, not height-shaded.
        let slot = slot_of(cave_bundle(k, hostile.clone(), whole.clone(), 2_000));
        let l = slot.layers.as_ref().expect("layers kept");
        assert!(l.terrain.is_empty() && !l.terrain_hidden);
        assert!(!l.ceiling_hidden);
        assert_eq!(l.ceiling.as_ref().map(|(c, _)| c.layers().len()), Some(1));
        assert!(!slot.has_terrain_layers() && !slot.terrain_out_of_scene(&frame));
        assert!(!slot.wants_shaded_ceiling(&frame));
        assert!(slot.layered_ceiling().is_some());

        // Closed block, layerless ceiling: §4.9 rule 3 hides it in the normal view.
        let slot = slot_of(cave_bundle(
            k,
            hostile.clone(),
            SurfaceChunk::default(),
            500,
        ));
        let l = slot.layers.as_ref().expect("layers kept");
        assert!(l.ceiling_hidden && !l.terrain_hidden);
        assert!(!slot.wants_shaded_ceiling(&frame), "draws nothing");
        assert!(slot.layered_ceiling().is_none(), "no zero-layer chunk");
        let off = TerrainFrame {
            layered: false,
            ..frame
        };
        assert!(slot.wants_shaded_ceiling(&off), "the M3 view draws it");

        // Ceiling layers that do not resolve: the fallback.
        let slot = slot_of(cave_bundle(k, whole, hostile, 2_000));
        assert!(slot.has_terrain_layers());
        assert!(slot.layered_ceiling().is_none());
        assert!(slot.wants_shaded_ceiling(&frame));
    }

    /// Review finding (terrain.rs ceiling jobs, `signature`): a ceiling and a terrain
    /// chunk hidden by §4.9 rule 3 get no chunk, no mesh and no draw in the normal view
    /// (`terrain-texturing.md` §4.9 rule 3); with layers off both are drawn height-shaded,
    /// and switching layers on again frees both meshes.
    #[test]
    fn rule_three_chunks_are_left_out_of_the_scene() {
        use tessera_formats::landblock::SurfaceChunk;
        let Some(mut renderer) = headless() else {
            return;
        };
        let (table, _, _) = layer_fixture();
        let k = key(10, 10);
        let mut objects = objects(&renderer);
        let mut layer = TerrainLayer::new(
            1,
            LodSelector::from_fov(720.0, 1.0, 1.5),
            MeshConfig::default(),
            Arc::new(table_inputs(k.region(), &table)),
        )
        .unwrap();
        let anchor = Anchor {
            region: 1,
            x: 10,
            y: 10,
        };
        let bundle = cave_bundle(k, SurfaceChunk::default(), SurfaceChunk::default(), 500);
        assert!(!block_open(&bundle.data));
        assert!(layer.insert(k, &Arc::new(bundle), &mut renderer, &mut objects));
        settle(&mut layer, &mut renderer, &anchor, &mut objects);
        let slot = &layer.slots[&k];
        assert!(slot.terrain_hidden());
        assert!(
            slot.mesh.is_none() && slot.staged.is_none(),
            "no terrain mesh"
        );
        assert!(slot.ceiling.is_none(), "no ceiling chunk");
        assert!(matches!(slot.shaded_ceiling, ShadedCeiling::None));
        assert_eq!(renderer.mesh_stats().meshes, 0);

        let off = TerrainFrame {
            layered: false,
            ..TerrainFrame::default()
        };
        settle_in(&mut layer, &mut renderer, &anchor, &mut objects, &off);
        let slot = &layer.slots[&k];
        assert!(slot.mesh.is_some(), "the M3 view draws the terrain");
        assert!(matches!(slot.shaded_ceiling, ShadedCeiling::Ready(..)));

        settle(&mut layer, &mut renderer, &anchor, &mut objects);
        let slot = &layer.slots[&k];
        assert!(slot.mesh.is_none(), "freed again in the normal view");
        assert!(slot.ceiling.is_none());
        assert!(
            matches!(slot.shaded_ceiling, ShadedCeiling::None),
            "the shaded ceiling is freed too"
        );
        assert_eq!(renderer.mesh_stats().meshes, 0, "nothing stays on the GPU");
        layer.clear(&mut renderer, &mut objects);
        layer.shutdown();
    }

    /// Review finding (terrain.rs far band): a block back in the far band releases the
    /// masks uploaded while it was near and queues them again (`walker.md` §14.4: a block
    /// in the band keeps only its light map on the GPU).
    #[test]
    fn far_blocks_release_their_masks() {
        let Some(mut renderer) = headless() else {
            return;
        };
        let k = key(5, 5);
        let mut slot = Slot::new(Arc::new(bundle_at(k, 0)), 1);
        let mask = Did(0x4100_0001);
        let img = BlockImage::Mask {
            width: 1,
            height: 1,
            alpha: vec![7],
        };
        let texel = TextureUpload::rgba8(1, 1, &[255, 255, 255, 7]);
        let mask_id = renderer.upload_texture(&texel).expect("mask");
        let bake_id = renderer.upload_texture(&texel).expect("bake");
        slot.own.insert(mask, (mask_id, 4));
        slot.parked.push((mask, img.clone()));
        slot.bake = Bake::Ready(bake_id, 4);
        let mesh = renderer
            .upload_mesh(&shaded_ceiling_mesh(
                &TerrainBlock::flat(1_000),
                &HeightSlopeShading::default(),
                k,
            ))
            .expect("mesh");
        let sig = |far: bool| MeshSig {
            lod: Lod::FINEST,
            edges: [Lod::FINEST; 4],
            present: 0,
            layered: true,
            far,
        };
        slot.mesh = Some(GpuTerrain {
            kind: GpuKind::Mesh(mesh),
            sig: sig(true),
            min_h: 0.0,
            max_h: 0.0,
            bytes: 0,
        });
        let band = TerrainFrame {
            far_band: 480.0,
            ..TerrainFrame::default()
        };
        let textures = renderer.texture_count();
        // Near, or the band off: kept.
        assert_eq!(slot.park_masks(&band, &mut renderer), 0);
        slot.far = true;
        assert_eq!(slot.park_masks(&TerrainFrame::default(), &mut renderer), 0);
        // A full-layer chunk still waits for its upload: kept.
        slot.ready = Some((sig(false), Built::Mesh(prepared(k)), 0.0, 0.0));
        assert_eq!(slot.park_masks(&band, &mut renderer), 0);
        slot.ready = None;
        // Far, baked, drawn baked: released and queued again as near-only.
        assert_eq!(slot.park_masks(&band, &mut renderer), 1);
        assert!(slot.own.is_empty() && slot.parked.is_empty());
        assert_eq!(slot.images, vec![(mask, img)]);
        assert_eq!(slot.near_only, 1);
        assert_eq!(renderer.texture_count(), textures - 1);
        assert!(slot.images_done(&band), "masks wait while far");
        slot.far = false;
        assert!(!slot.images_done(&band), "and are uploaded when it nears");
        slot.release(&mut renderer, None);
    }

    fn prepared(k: LandblockKey) -> PreparedMesh {
        shaded_ceiling_mesh(
            &TerrainBlock::flat(1_000),
            &HeightSlopeShading::default(),
            k,
        )
        .prepare(&MeshConfig::default())
        .expect("prepared")
    }

    /// Runs `update` until settled (bounded).
    fn settle(
        layer: &mut TerrainLayer,
        renderer: &mut Renderer<'_>,
        anchor: &Anchor,
        objects: &mut dyn TextureStore,
    ) {
        settle_in(layer, renderer, anchor, objects, &TerrainFrame::default());
    }

    /// Runs `update` with `frame` until settled (bounded).
    fn settle_in(
        layer: &mut TerrainLayer,
        renderer: &mut Renderer<'_>,
        anchor: &Anchor,
        objects: &mut dyn TextureStore,
        frame: &TerrainFrame,
    ) {
        let t = Instant::now();
        loop {
            let mut budget = UploadBudget::new(u64::MAX, Duration::from_secs(1), Instant::now());
            layer.update(
                renderer,
                anchor,
                Vec3::new(80.0, 80.0, 300.0),
                frame,
                objects,
                &mut budget,
            );
            if layer.settled() {
                return;
            }
            assert!(t.elapsed() < Duration::from_secs(60), "never settled");
            std::thread::sleep(Duration::from_millis(2));
        }
    }

    fn seams_agree(layer: &TerrainLayer) -> bool {
        layer.slots.iter().all(|(k, s)| {
            let Some(m) = &s.mesh else { return true };
            Side::ALL.iter().all(|&side| {
                let (dx, dy) = side.offset();
                let n = k.offset(dx, dy).and_then(|n| layer.slots.get(&n));
                n.and_then(|n| n.mesh.as_ref()).is_none_or(|nm| {
                    nm.sig.edges[side.opposite().index()] == m.sig.edges[side.index()]
                })
            })
        })
    }

    /// Insert, settle, replace, remove and switch regions with synthetic bundles on a real
    /// (headless) renderer: meshes are freed, heights follow, seams agree.
    #[test]
    fn layer_tracks_bundles() {
        let Some(mut renderer) = headless() else {
            return;
        };
        let mut objects = objects(&renderer);
        let mut layer = TerrainLayer::new(
            2,
            LodSelector::from_fov(720.0, 1.0, 1.5),
            MeshConfig::default(),
            inputs(),
        )
        .unwrap();
        let anchor = Anchor {
            region: 1,
            x: 10,
            y: 10,
        };
        let keys: Vec<LandblockKey> = (8..13)
            .flat_map(|x| (8..13).map(move |y| key(x, y)))
            .collect();
        for k in &keys {
            assert!(layer.insert(
                *k,
                &Arc::new(bundle_at(*k, 4_000)),
                &mut renderer,
                &mut objects
            ));
        }
        settle(&mut layer, &mut renderer, &anchor, &mut objects);
        assert_eq!(layer.stats().meshed, keys.len());
        assert_eq!(layer.take_finished().len(), keys.len());
        assert_eq!(renderer.mesh_stats().meshes as usize, keys.len());
        assert!(seams_agree(&layer));
        let tiles = layer.tiles().clone();
        assert_eq!(tiles.len(), keys.len());
        assert!(
            tiles
                .height_in(1, 10.0 * 160.0 + 5.0, 10.0 * 160.0 + 5.0)
                .is_some()
        );

        // Move the camera far away: levels change, seams still agree once settled.
        let far = Anchor {
            region: 1,
            x: 0,
            y: 0,
        };
        settle(&mut layer, &mut renderer, &far, &mut objects);
        assert!(seams_agree(&layer));
        assert_eq!(renderer.mesh_stats().meshes as usize, keys.len());

        // Replacing a bundle frees the old mesh at once.
        let k = key(10, 10);
        assert!(layer.insert(
            k,
            &Arc::new(bundle_at(k, 9_000)),
            &mut renderer,
            &mut objects
        ));
        assert_eq!(renderer.mesh_stats().meshes as usize, keys.len() - 1);
        settle(&mut layer, &mut renderer, &anchor, &mut objects);
        assert_eq!(renderer.mesh_stats().meshes as usize, keys.len());

        // Removing frees the mesh and the heights.
        layer.remove(k, &mut renderer, &mut objects);
        assert_eq!(renderer.mesh_stats().meshes as usize, keys.len() - 1);
        assert!(
            tiles
                .height_in(1, 10.0 * 160.0 + 5.0, 10.0 * 160.0 + 5.0)
                .is_none()
        );
        assert_eq!(tiles.len(), keys.len() - 1);

        // A bundle of another region switches the shared heights to it.
        let other = LandblockKey::new(2, 10, 10).unwrap();
        assert!(layer.insert(
            other,
            &Arc::new(bundle_at(other, 4_000)),
            &mut renderer,
            &mut objects
        ));
        assert_eq!(tiles.region(), Some(2));
        assert!(tiles.is_empty());
        layer.clear(&mut renderer, &mut objects);
        assert_eq!(renderer.mesh_stats().meshes, 0);
        layer.shutdown();
    }

    /// A failed block job leaves the block settled, without terrain.
    #[test]
    fn failed_block_counts_as_settled() {
        let Some(mut renderer) = headless() else {
            return;
        };
        let mut objects = objects(&renderer);
        let mut layer = TerrainLayer::new(
            1,
            LodSelector::from_fov(720.0, 1.0, 1.5),
            MeshConfig::default(),
            inputs(),
        )
        .unwrap();
        let k = key(3, 3);
        assert!(layer.insert(k, &Arc::new(bundle_at(k, 100)), &mut renderer, &mut objects));
        let generation = layer.slots[&k].generation;
        // The worker's real result may arrive later; it is applied after this one and
        // would build the block, so stop the workers first.
        layer.shutdown();
        layer.apply(
            Output::BlockFailed {
                key: k,
                generation,
                reason: "synthetic".into(),
            },
            Instant::now(),
            &mut objects,
        );
        assert!(layer.settled());
        layer.refresh_stats();
        assert_eq!(layer.stats().failed, 1);
        assert_eq!(layer.stats().panics, 1);
        layer.clear(&mut renderer, &mut objects);
    }
}
