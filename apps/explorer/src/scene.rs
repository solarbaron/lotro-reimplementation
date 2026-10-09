// SPDX-License-Identifier: GPL-3.0-or-later
//! Mesh-viewer scenes, built on `tessera-assets` workers by [`SceneDecoder`].
//!
//! A scene is everything the 3D view and the material inspector need for one selected
//! DID, ready to upload and draw:
//!
//! | Selected record | Parts | Spec |
//! |---|---|---|
//! | 0x06 mesh | the mesh itself | `mesh.md` |
//! | 0x04 setup, 0x1F visual template, 0x47 entity descriptor | the setup's part list, followed through the chain | `setup.md` §3.1, §6.6, §7 |
//! | 0x20 appearance | the part meshes of one entry's base block, with its material groups layered on | `setup.md` §8, `material.md` §9.3 |
//! | 0x70–0x7F object or property set | a chosen geometry link of the object: any top-level entity/template/setup DID property (`PhysObj`, …), or one `Item_WornAppearanceMapList` element | `setup.md` §3.1, §3.2 |
//! | 0x31 surface record | a preview sphere drawn with that surface's material | `material.md` §9.2 |
//! | 0x30 material | a preview sphere with the material's own base texture (no shader context) | `material.md` §6 |
//!
//! Every part is decoded (`tessera-mesh`), one detail level is flattened, converted to
//! renderer streams ([`crate::geometry`]) and prepared (`MeshData::prepare`, all CPU work
//! of an upload) here. Each draw's surface record is resolved (`tessera-material`), and
//! the textures the renderer samples are decoded and composed (base colour with the
//! opacity source in alpha, normal map, gloss and dye masks), plus small thumbnails of
//! every texture slot for the inspector. The UI thread only uploads the result.
//!
//! Parts are placed in the setup's own frame with an identity transform (`setup.md` §7:
//! the part list carries no transforms); scale-like template fields are OPEN and ignored.
//!
//! # Bounds
//!
//! A scene is bounded as a whole, not only per part: at most [`MAX_PARTS`] parts, each
//! distinct mesh built once (repeated part meshes share one prepared mesh), at most
//! [`MAX_SCENE_VERTICES`], [`MAX_SCENE_INDICES`] and [`MAX_SCENE_STREAM_BYTES`] of
//! geometry (checked by `tessera-mesh` before a level is flattened), [`TEXTURE_BUDGET`]
//! of textures at a resolution cap that is enforced, and at most [`MAX_NOTES`] notes and
//! [`MAX_CHAIN`] chain links. Whatever is left out is named in the notes.

use std::collections::{HashMap, HashSet};
use std::fmt::Write as _;
use std::sync::Arc;
use std::time::Instant;

use tessera_assets::{AssetKey, Decoder, JobContext};
use tessera_formats::appearance::MaterialGroup;
use tessera_formats::{AppearanceRecord, Did, Record, RgbaImage};
use tessera_material::records::{
    AddressMode, FilterMode, MaterialRecord, ParamValue, SamplerState,
};
use tessera_material::resolve::{DyeMask, ParamOrigin};
use tessera_material::vocab::{AlphaUse, ColorUse, SlotKind, ids};
use tessera_material::{BlendMode, Layering, ResolvedMaterial};
use tessera_mesh::{DecodeOptions, GeometryLimits, LevelKind, Mesh, MeshError};
use tessera_props::Value;
use tessera_render::{MeshConfig, MeshData, PreparedMesh};

use crate::collision_overlay::{
    CollisionOverlay, cell_overlays, landblock_overlays, template_overlays,
};
use crate::data::DataContext;
use crate::decoders::{label_text, load_property_set};
use crate::error::ExplorerError;
use crate::geometry::{
    AlphaSource, ConvertOptions, alpha_mask, base_texture, fit_image, level_mesh_data, mesh_skin,
    srgb_unit_to_linear, thumbnail, transform_aabb, y_up_to_z_up,
};

/// Most parts a scene draws (observed setups have at most 12, appearance blocks a few
/// dozen); further parts are listed in the notes and skipped.
pub const MAX_PARTS: usize = 64;
/// Most distinct GPU textures a scene uploads; further ones are left out (the
/// materials then draw without them).
pub const MAX_SCENE_TEXTURES: usize = 192;
/// Byte budget of a scene's GPU textures (RGBA8).
pub const TEXTURE_BUDGET: usize = 256 << 20;
/// Most texture thumbnails a scene keeps for the inspector.
pub const MAX_THUMBS: usize = 160;
/// Largest side of an inspector thumbnail.
pub const THUMB_SIDE: u32 = 64;
/// Most choices (object geometry links, appearance entries) a scene lists.
pub const MAX_CHOICES: usize = 255;
/// Most vertices of all distinct part meshes of a scene together (the largest observed
/// level has about 100 k, `tessera_mesh::GeometryLimits`).
pub const MAX_SCENE_VERTICES: u64 = 4 << 20;
/// Most indices of all distinct part meshes of a scene together.
pub const MAX_SCENE_INDICES: u64 = 16 << 20;
/// Most bytes of flattened vertex and index streams of all distinct part meshes of a
/// scene together (`LevelGeometry::stream_bytes`). With the vertex and index caps, the
/// prepared meshes stay well below the renderer's mesh arena (1 GiB by default).
pub const MAX_SCENE_STREAM_BYTES: u64 = 256 << 20;
/// The scene-wide geometry budget of [`build_scene_limited`] (totals over the distinct
/// part meshes of a scene).
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct SceneLimits {
    /// Most vertices.
    pub max_vertices: u64,
    /// Most indices.
    pub max_indices: u64,
    /// Most bytes of flattened vertex and index streams.
    pub max_stream_bytes: u64,
}

impl Default for SceneLimits {
    /// [`MAX_SCENE_VERTICES`], [`MAX_SCENE_INDICES`], [`MAX_SCENE_STREAM_BYTES`].
    fn default() -> Self {
        Self {
            max_vertices: MAX_SCENE_VERTICES,
            max_indices: MAX_SCENE_INDICES,
            max_stream_bytes: MAX_SCENE_STREAM_BYTES,
        }
    }
}

/// Most notes a scene keeps; further ones are counted in a last note.
pub const MAX_NOTES: usize = 64;
/// Most chain links a scene keeps; further ones are counted in a note.
pub const MAX_CHAIN: usize = 32;

/// The variant byte value meaning "level 0" for [`SceneRequest::lod`].
pub const DEFAULT_LOD: u8 = 0;

/// What a scene is built from (the selected record's type).
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum SceneSource {
    /// A 0x06 mesh.
    Mesh,
    /// A 0x04 setup.
    Setup,
    /// A 0x1F visual template.
    Template,
    /// A 0x47 entity descriptor.
    Entity,
    /// A 0x20 appearance record.
    Appearance,
    /// A 0x70–0x7F object or property set.
    Object,
    /// A 0x31 surface record (preview sphere).
    Surface,
    /// A 0x30 material (preview sphere).
    Material,
    /// An assembled character (the character view, [`crate::character`]); never the
    /// type of a selected DID.
    Figure,
    /// A 0x66 stand-alone collision mesh (drawn as a collision overlay).
    CollisionMesh,
    /// A 0x01 cell geometry: its embedded render mesh with collision overlays.
    Cell,
    /// A landblock-family record: the terrain with the landblock's collision set, portals
    /// and water as overlays.
    Landblock,
}

impl SceneSource {
    /// The source for a DID's type prefix, if the mesh view applies to it.
    #[must_use]
    pub fn of(did: u32) -> Option<Self> {
        Some(match did >> 24 {
            0x06 => Self::Mesh,
            0x04 => Self::Setup,
            0x1F => Self::Template,
            0x47 => Self::Entity,
            0x20 => Self::Appearance,
            0x70..=0x7F => Self::Object,
            0x31 => Self::Surface,
            0x30 => Self::Material,
            0x66 => Self::CollisionMesh,
            0x01 => Self::Cell,
            0x80 if crate::world::landblock_of(did).is_some() => Self::Landblock,
            _ => return None,
        })
    }

    /// Display name.
    #[must_use]
    pub fn name(self) -> &'static str {
        match self {
            Self::Mesh => "mesh",
            Self::Setup => "setup",
            Self::Template => "visual template",
            Self::Entity => "entity descriptor",
            Self::Appearance => "appearance",
            Self::Object => "object",
            Self::Surface => "surface record",
            Self::Material => "material",
            Self::Figure => "character",
            Self::CollisionMesh => "collision mesh",
            Self::Cell => "cell geometry",
            Self::Landblock => "landblock",
        }
    }
}

/// The options of a scene request, carried in the asset key's variant.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub struct SceneRequest {
    /// Level index drawn for every part (clamped to each part's last level).
    pub lod: u8,
    /// Which alternative to show (object geometry links, appearance entries).
    pub choice: u8,
    /// Whether vertex colours (RGB) modulate the base colour. Off by default: their use
    /// is OPEN (`mesh.md` §5.4, `material.md` §19).
    pub vertex_colors: bool,
}

impl Default for SceneRequest {
    fn default() -> Self {
        Self {
            lod: DEFAULT_LOD,
            choice: 0,
            vertex_colors: false,
        }
    }
}

impl SceneRequest {
    /// The asset-key variant encoding this request (0 for the default request).
    #[must_use]
    pub fn variant(self) -> u32 {
        u32::from(self.lod) | u32::from(self.choice) << 8 | u32::from(self.vertex_colors) << 16
    }

    /// Decodes [`SceneRequest::variant`].
    #[must_use]
    pub fn from_variant(v: u32) -> Self {
        Self {
            lod: (v & 0xFF) as u8,
            choice: ((v >> 8) & 0xFF) as u8,
            vertex_colors: (v >> 16) & 1 == 1,
        }
    }
}

/// One link of the chain from the selected record to its geometry.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ChainLink {
    /// What the link is.
    pub caption: String,
    /// The DID.
    pub did: u32,
}

/// One level of a part's mesh.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct LevelSummary {
    /// "detail", "proxy" or the raw kind.
    pub kind: String,
    /// Draw entries.
    pub draws: usize,
    /// Triangles.
    pub triangles: usize,
}

/// One draw of a part's level: a submesh drawn with one surface.
#[derive(Debug, Clone, PartialEq)]
pub struct SceneSubmesh {
    /// Position in the level's draw list (the renderer submesh index).
    pub draw: u32,
    /// The submesh whose vertices are drawn.
    pub submesh: u32,
    /// The surface record (0x31).
    pub surface: Option<u32>,
    /// The submesh's vertex format word.
    pub format: u32,
    /// Vertices.
    pub vertices: u32,
    /// Triangles.
    pub triangles: u32,
    /// Whether it has bone influences (drawn in bind pose).
    pub skinned: bool,
    /// Stored AABB (min, max).
    pub aabb: Option<([f32; 3], [f32; 3])>,
    /// Index into [`SceneProduct::materials`].
    pub material: Option<usize>,
}

/// A part of the scene: one mesh at one level.
#[derive(Debug, Clone)]
pub struct ScenePart {
    /// The mesh DID.
    pub mesh: u32,
    /// Where it comes from (part index, appearance slot, extra setup).
    pub caption: String,
    /// The level drawn.
    pub level: usize,
    /// Every level of the mesh.
    pub levels: Vec<LevelSummary>,
    /// The mesh's flags word.
    pub flags: u32,
    /// Local → scene transform (column-major); identity (`setup.md` §7).
    pub transform: [f32; 16],
    /// The prepared GPU streams of the drawn level.
    pub prepared: Option<Arc<PreparedMesh>>,
    /// Why the part cannot be drawn.
    pub error: Option<String>,
    /// The draws of the level.
    pub submeshes: Vec<SceneSubmesh>,
    /// Validation findings (`mesh.md` §14), as text, at most a few.
    pub issues: Vec<String>,
    /// Vertices and triangles of the drawn level.
    pub vertices: usize,
    /// Triangles of the drawn level.
    pub triangles: usize,
    /// Bounding sphere of the drawn level (centre, radius).
    pub sphere: Option<([f32; 3], f32)>,
    /// The prepared streams carry bone influences: the part is skinned with the figure's
    /// palette (character view, `skeleton-anim.md` §9.1); otherwise it is drawn rigid.
    pub skinned: bool,
    /// For a held item's part: the `HoldingLocation` key of the wearer whose model
    /// transform places it (`skeleton-anim.md` §8.4).
    pub attach: Option<u32>,
    /// Which figure of a character scene the part belongs to: 0 the figure itself, 1 the
    /// mount attached to it (`mounts.md` §10), drawn with the mount's placement and
    /// palette ([`crate::viewport::PoseDraw::mount`]).
    pub figure: u8,
}

/// How the renderer treats a material's alpha.
#[derive(Debug, Clone, Copy, PartialEq)]
pub enum AlphaKind {
    /// Opaque.
    Opaque,
    /// Alpha test with this threshold (kept when opacity ≥ threshold, `material.md` §11.2).
    Mask(f32),
    /// Alpha blending (the renderer's only blend; additive and other blend states are
    /// approximated by it).
    Blend,
}

/// One dye channel of a GPU material.
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct GpuDye {
    /// Mask texture (renderer convention: dye weight in R), index into
    /// [`SceneProduct::textures`].
    pub mask: usize,
    /// The effective dye colour (RGBA as stored, 0–1).
    pub color: [f32; 4],
}

/// What the renderer needs of one resolved material (texture indices into
/// [`SceneProduct::textures`]).
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct GpuMaterial {
    /// Base colour texture, opacity in alpha.
    pub base: Option<usize>,
    /// Tangent-space normal map.
    pub normal: Option<usize>,
    /// Gloss mask (RGB).
    pub gloss: Option<usize>,
    /// Dye channels (primary, secondary).
    pub dye: [Option<GpuDye>; 2],
    /// Alpha handling.
    pub alpha: AlphaKind,
    /// No back-face culling.
    pub two_sided: bool,
    /// Linear base colour factor (diffuse colour 2000, sRGB-decoded; the viewer applies it
    /// only when asked, `material.md` §19).
    pub base_color: [f32; 4],
    /// Specular colour (2001, converted from sRGB to linear like 2000 and the dye colour)
    /// and power (1000).
    pub specular: Option<([f32; 3], f32)>,
    /// Unlit shader record (`material.md` §11.3).
    pub unlit: bool,
}

impl Default for GpuMaterial {
    fn default() -> Self {
        Self {
            base: None,
            normal: None,
            gloss: None,
            dye: [None, None],
            alpha: AlphaKind::Opaque,
            two_sided: false,
            base_color: [1.0; 4],
            specular: None,
            unlit: false,
        }
    }
}

/// One texture-slot row of the material inspector.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct SlotRow {
    /// Parameter id.
    pub id: u32,
    /// Slot kind name.
    pub kind: String,
    /// Role (sampler name, `material.md` §10.1).
    pub role: String,
    /// Channel packing as text.
    pub packing: String,
    /// Texture list DID.
    pub texture: Option<u32>,
    /// Sampler address and filter.
    pub sampler: String,
    /// Where the value comes from.
    pub origin: String,
    /// What the viewer uses it for.
    pub used_as: String,
    /// Index into [`SceneProduct::thumbs`].
    pub thumb: Option<usize>,
}

/// One effective-parameter row of the material inspector.
#[derive(Debug, Clone, PartialEq)]
pub struct ParamRow {
    /// Parameter id.
    pub id: u32,
    /// A short name, when the id's meaning is known (`material.md` §8).
    pub name: &'static str,
    /// The value as text.
    pub value: String,
    /// The colour, for colour values.
    pub color: Option<[f32; 4]>,
    /// A DID to open (texture lists).
    pub link: Option<u32>,
    /// Where the value comes from.
    pub origin: String,
}

/// The material inspector's content for one material.
#[derive(Debug, Clone, Default, PartialEq)]
pub struct MaterialView {
    /// Records to open: (caption, DID).
    pub links: Vec<(String, u32)>,
    /// Key/value facts.
    pub facts: Vec<(String, String)>,
    /// Feature flags that are set.
    pub flags: Vec<String>,
    /// Texture slots.
    pub slots: Vec<SlotRow>,
    /// Effective parameters.
    pub params: Vec<ParamRow>,
    /// Remarks (skipped materials, ignored entries, approximations).
    pub notes: Vec<String>,
}

/// A material of the scene (one per distinct surface and appearance layering).
#[derive(Debug, Clone, PartialEq)]
pub struct SceneMaterial {
    /// Display title.
    pub title: String,
    /// Renderer description.
    pub gpu: GpuMaterial,
    /// Whether the material is dyeable (`material.md` §11.5).
    pub dyeable: bool,
    /// Inspector content.
    pub view: MaterialView,
    /// Why it did not resolve (the submesh then draws with a plain grey material).
    pub error: Option<String>,
}

/// A texture to upload.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct SceneTexture {
    /// Debug label.
    pub label: String,
    /// Width.
    pub width: u32,
    /// Height.
    pub height: u32,
    /// Straight-alpha RGBA8, top row first.
    pub rgba: Vec<u8>,
    /// Linear data (normal maps, masks) rather than sRGB colour.
    pub linear: bool,
    /// Clamp addressing instead of repeat.
    pub clamp: bool,
    /// Nearest filtering.
    pub nearest: bool,
}

/// An inspector thumbnail.
#[derive(Debug, Clone)]
pub struct Thumb {
    /// The texture list.
    pub list: u32,
    /// Whether the image shows the alpha channel (as grey) instead of RGB.
    pub alpha: bool,
    /// The image (at most [`THUMB_SIDE`] a side), opaque: the texture's RGB, or with
    /// `alpha` its alpha channel as grey.
    pub image: Arc<RgbaImage>,
    /// Size of the decoded level it was made from, or why it failed.
    pub info: String,
}

/// A scene ready to upload and draw.
#[derive(Debug, Clone)]
pub struct SceneProduct {
    /// The selected DID.
    pub did: u32,
    /// Its type.
    pub source: SceneSource,
    /// The request it was built for.
    pub request: SceneRequest,
    /// The chain from the selected record to the geometry.
    pub chain: Vec<ChainLink>,
    /// Alternatives the request's `choice` selects among.
    pub choices: Vec<String>,
    /// The alternative shown.
    pub choice: usize,
    /// Level labels by level index (over all parts).
    pub level_labels: Vec<String>,
    /// The parts.
    pub parts: Vec<ScenePart>,
    /// The materials.
    pub materials: Vec<SceneMaterial>,
    /// The textures.
    pub textures: Vec<Arc<SceneTexture>>,
    /// Inspector thumbnails.
    pub thumbs: Vec<Thumb>,
    /// Remarks for the user.
    pub notes: Vec<String>,
    /// Bounds of every drawn part (min, max).
    pub aabb: Option<([f32; 3], [f32; 3])>,
    /// Bounding sphere (centre, radius) of every drawn part.
    pub sphere: ([f32; 3], f32),
    /// Vertices drawn.
    pub vertices: usize,
    /// Triangles drawn.
    pub triangles: usize,
    /// The dye colours of the shown appearance entry's blocks (`material.md` §12.4).
    pub dyes: Vec<DyeBlock>,
    /// Time the worker spent, in milliseconds.
    pub build_ms: u64,
    /// Collision overlays (M4.5, [`crate::collision_overlay`]).
    pub collision: Vec<CollisionOverlay>,
}

impl SceneProduct {
    /// Whether any part or collision overlay can be drawn.
    #[must_use]
    pub fn drawable(&self) -> bool {
        self.parts.iter().any(|p| p.prepared.is_some())
            || self
                .collision
                .iter()
                .any(|o| o.prepared.is_some() || !o.lines.is_empty())
    }

    /// Approximate heap bytes (for the asset cache budget).
    #[must_use]
    pub fn heap_bytes(&self) -> usize {
        // Parts that repeat a mesh share its prepared streams: count those once.
        let mut seen = HashSet::new();
        let parts: usize = self
            .parts
            .iter()
            .map(|p| {
                let mesh = p
                    .prepared
                    .as_ref()
                    .filter(|m| seen.insert(Arc::as_ptr(m)))
                    .map_or(0, |m| m.gpu_bytes() as usize);
                mesh + 512
            })
            .sum();
        let tex: usize = self.textures.iter().map(|t| t.rgba.len() + 128).sum();
        let thumbs: usize = self.thumbs.iter().map(|t| t.image.pixels.len() + 64).sum();
        let mats: usize = self
            .materials
            .iter()
            .map(|m| 512 + m.view.params.len() * 96 + m.view.slots.len() * 160)
            .sum();
        let overlays: usize = self
            .collision
            .iter()
            .map(CollisionOverlay::heap_bytes)
            .sum();
        parts + tex + thumbs + mats + overlays + 1024
    }
}

/// One block of an appearance entry with the dye colours its materials set
/// (`material.md` §12.1: a dye block layers a material whose only parameters are the dye
/// colours 2003 and 2013).
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct DyeBlock {
    /// The block's `variant` (1.0 = base look, small values = dye codes, `setup.md` §8.3).
    pub variant: f32,
    /// Dye colour 2003 set by one of the block's group materials.
    pub primary: Option<[f32; 4]>,
    /// Dye colour 2013.
    pub secondary: Option<[f32; 4]>,
}

/// Most dye blocks listed per entry (observed entries have up to 53 blocks).
pub const MAX_DYE_BLOCKS: usize = 128;

/// The dye colours of every block of `entry` (blocks whose materials set no dye colour
/// are left out).
fn dye_blocks(
    ctx: &DataContext,
    entry: &tessera_formats::appearance::AppearanceEntry,
) -> Vec<DyeBlock> {
    let mut out = Vec::new();
    for b in entry.blocks.iter().take(MAX_DYE_BLOCKS) {
        let mut block = DyeBlock {
            variant: b.variant,
            primary: None,
            secondary: None,
        };
        for g in b.groups.iter().take(64) {
            if g.material.is_null() {
                continue;
            }
            if let Ok(m) = ctx.materials.material(g.material) {
                let c = tessera_material::DyeColors::of_material(&m);
                block.primary = block.primary.or(c.primary);
                block.secondary = block.secondary.or(c.secondary);
            }
        }
        if block.primary.is_some() || block.secondary.is_some() {
            out.push(block);
        }
    }
    out
}

/// Builds [`SceneProduct`]s (the request is the key's variant, [`SceneRequest`]).
#[derive(Debug, Clone)]
pub struct SceneDecoder(pub Arc<DataContext>);

impl Decoder for SceneDecoder {
    type Output = SceneProduct;
    type Error = ExplorerError;
    const NAME: &'static str = "explorer-scene";

    fn decode(
        &self,
        key: AssetKey,
        _: Vec<u8>,
        job: &JobContext<'_>,
    ) -> Result<SceneProduct, ExplorerError> {
        build_scene(
            &self.0,
            key.did,
            SceneRequest::from_variant(key.variant),
            job,
        )
    }

    fn size_of(&self, v: &SceneProduct) -> usize {
        v.heap_bytes()
    }
}

// ---------------------------------------------------------------------------------------
// Chains
// ---------------------------------------------------------------------------------------

/// A part to build: a mesh and, for appearance parts, the block's material groups.
#[derive(Debug, Clone)]
struct PartSpec {
    mesh: Did,
    caption: String,
    groups: Option<Arc<Vec<MaterialGroup>>>,
}

/// Reads a record through routing: `Ok(None)` when no archive holds it.
fn fetch(ctx: &DataContext, did: Did) -> Result<Option<Vec<u8>>, tessera_dat::Error> {
    match ctx.set.locate(did.0)? {
        Some(l) => Ok(Some(ctx.set.read_located(&l)?)),
        None => Ok(None),
    }
}

fn resolve_error(e: tessera_formats::ResolveError<tessera_dat::Error>) -> ExplorerError {
    match e {
        tessera_formats::ResolveError::Fetch { source, .. } => ExplorerError::from(source),
        tessera_formats::ResolveError::Format(f) => ExplorerError::Format(f),
    }
}

/// The chain from the selected record to its geometry: each DID once, at most
/// [`MAX_CHAIN`] links (further ones are counted).
#[derive(Debug, Default)]
struct Chain {
    links: Vec<ChainLink>,
    omitted: usize,
}

impl Chain {
    /// Adds a link unless its DID is already in the chain.
    fn push(&mut self, caption: impl Into<String>, did: u32) {
        if self.links.iter().any(|l| l.did == did) {
            return;
        }
        if self.links.len() >= MAX_CHAIN {
            self.omitted += 1;
            return;
        }
        self.links.push(ChainLink {
            caption: caption.into(),
            did,
        });
    }
}

/// Follows `start` (0x47, 0x1F or 0x04) to its setup; returns the chain links and the
/// part specs. With `collision`, the setup's collision overlays are built into it;
/// without, they are not built at all (appearance extras, whose collision is not shown).
fn setup_parts(
    ctx: &DataContext,
    start: Did,
    chain: &mut Chain,
    collision: Option<&mut Vec<CollisionOverlay>>,
) -> Result<Vec<PartSpec>, ExplorerError> {
    let resolved =
        tessera_formats::resolve_setup(start, |d| fetch(ctx, d)).map_err(resolve_error)?;
    for d in &resolved.path {
        chain.push(d.kind().name(), d.0);
    }
    if let Some(collision) = collision {
        collision.extend(setup_collision(ctx, &resolved.setup));
    }
    Ok(resolved
        .parts()
        .iter()
        .enumerate()
        .map(|(i, &mesh)| PartSpec {
            mesh,
            caption: format!("part {i}"),
            groups: None,
        })
        .collect())
}

/// The collision overlays of a setup (`collision.md` §4.3, §8): its 0x66 mesh, read with
/// its directory stamp, and its boxes, spheres and capsules. Problems become an empty
/// overlay whose caption says what went wrong.
fn setup_collision(ctx: &DataContext, setup: &tessera_formats::Setup) -> Vec<CollisionOverlay> {
    use crate::collision_overlay::{OverlayKind, line_overlay};
    let problem = |what: String| line_overlay(OverlayKind::Mesh, what, std::iter::empty());
    let mut out = Vec::new();
    let mesh = if setup.collision_mesh.is_null() {
        None
    } else {
        let d = setup.collision_mesh;
        match ctx
            .set
            .read_with_entry(d.0)
            .map_err(ExplorerError::from)
            .and_then(|(l, bytes)| {
                tessera_formats::CollisionMesh::parse(&bytes, Some(l.entry.stamp))
                    .map_err(ExplorerError::from)
            }) {
            Ok(m) => Some(m),
            Err(e) => {
                out.push(problem(format!("collision mesh {d}: {e}")));
                None
            }
        }
    };
    match tessera_physics::geometry::ShapeTemplate::from_setup(setup, mesh.as_ref()) {
        Ok(t) => out.extend(template_overlays(
            &t,
            &format!("collision mesh {}", setup.collision_mesh),
        )),
        Err(e) => out.push(problem(format!("collision shape refused: {e}"))),
    }
    out
}

/// The landblock's terrain as a drawn part (the render triangulation, which is also the
/// terrain collision mesh, `collision.md` §6.1), smooth-shaded, without a material.
fn terrain_part(bundle: &tessera_world::LandblockBundle) -> ScenePart {
    let grid = tessera_physics::geometry::GridMesh::terrain(&bundle.data);
    let mut positions = Vec::with_capacity(33 * 33);
    for j in 0..33 {
        for i in 0..33 {
            positions.push(
                grid.vertex(i, j)
                    .unwrap_or([5.0 * i as f32, 5.0 * j as f32, 0.0]),
            );
        }
    }
    let mut normals = vec![[0.0f32; 3]; positions.len()];
    let mut indices = Vec::with_capacity(tessera_physics::geometry::GRID_TRIANGLES * 3);
    for key in 0..tessera_physics::geometry::GRID_TRIANGLES {
        let Some(t) = u16::try_from(key)
            .ok()
            .and_then(|k| grid.triangle_vertices(k))
        else {
            continue;
        };
        let [a, b, c] = t.map(usize::from);
        let (Some(pa), Some(pb), Some(pc)) = (positions.get(a), positions.get(b), positions.get(c))
        else {
            continue;
        };
        let e1 = [pb[0] - pa[0], pb[1] - pa[1], pb[2] - pa[2]];
        let e2 = [pc[0] - pa[0], pc[1] - pa[1], pc[2] - pa[2]];
        let n = [
            e1[1] * e2[2] - e1[2] * e2[1],
            e1[2] * e2[0] - e1[0] * e2[2],
            e1[0] * e2[1] - e1[1] * e2[0],
        ];
        for v in [a, b, c] {
            for k in 0..3 {
                normals[v][k] += n[k];
            }
        }
        indices.extend([a as u32, b as u32, c as u32]);
    }
    let mut part = ScenePart {
        mesh: bundle.key.data_did().0,
        caption: "terrain".into(),
        level: 0,
        levels: Vec::new(),
        flags: 0,
        transform: IDENTITY,
        prepared: None,
        error: None,
        submeshes: Vec::new(),
        issues: Vec::new(),
        vertices: positions.len(),
        triangles: indices.len() / 3,
        sphere: None,
        skinned: false,
        attach: None,
        figure: 0,
    };
    let (lo, hi) = positions.iter().fold(
        ([f32::INFINITY; 3], [f32::NEG_INFINITY; 3]),
        |(mut lo, mut hi), p| {
            for k in 0..3 {
                lo[k] = lo[k].min(p[k]);
                hi[k] = hi[k].max(p[k]);
            }
            (lo, hi)
        },
    );
    let data = MeshData {
        label: Some("terrain".into()),
        positions,
        normals: Some(normals),
        tangents: None,
        binormals: None,
        uvs: None,
        colors: None,
        indices: tessera_render::Indices::U32(indices),
        submeshes: Vec::new(),
    };
    match data.prepare(&MeshConfig::default()) {
        Ok(p) => {
            let sphere = p.info().sphere;
            part.sphere = Some((sphere.center.to_array(), sphere.radius));
            part.prepared = Some(Arc::new(p));
            part.submeshes.push(SceneSubmesh {
                draw: 0,
                submesh: 0,
                surface: None,
                format: 0,
                vertices: part.vertices as u32,
                triangles: part.triangles as u32,
                skinned: false,
                aabb: (lo[0] <= hi[0]).then_some((lo, hi)),
                material: None,
            });
        }
        Err(e) => part.error = Some(e.to_string()),
    }
    part
}

/// One geometry link of an object's property set.
#[derive(Debug, Clone, PartialEq, Eq)]
enum ObjectLink {
    /// An entity, template or setup DID property.
    Entity { caption: String, did: Did },
    /// One element of the worn-appearance map list.
    Worn {
        caption: String,
        appearance: Did,
        key: u32,
    },
}

impl ObjectLink {
    fn caption(&self) -> &str {
        match self {
            Self::Entity { caption, .. } | Self::Worn { caption, .. } => caption,
        }
    }
}

/// Property names of the worn path (`setup.md` §3.2). Names come from the install's
/// property dictionary at run time; these are only looked up.
const WORN_LIST: &str = "Item_WornAppearanceMapList";
const WORN_APPEARANCE: &str = "Item_WornAppearance";
const WORN_KEY: &str = "Item_AppearanceKey";
const WORN_SPECIES: &str = "Item_SpeciesOfWearer";
const WORN_SEX: &str = "Item_SexOfWearer";

/// The geometry links of object or property set `did`: every top-level property whose
/// value is an entity (0x47), template (0x1F) or setup (0x04) DID, then every element of
/// the worn-appearance map list. Returns the links and the property-set DID.
fn object_links(ctx: &DataContext, did: u32) -> Result<(Vec<ObjectLink>, u32), ExplorerError> {
    let (set_did, set, _) = load_property_set(ctx, did)?;
    let registry = ctx.game.registry()?;
    let mut links = Vec::new();
    for p in set.iter() {
        if links.len() >= MAX_CHOICES {
            break;
        }
        if let Some(d) = p.value.did_ref()
            && matches!(d.prefix(), 0x47 | 0x1F | 0x04)
        {
            let name = registry
                .name(p.pid)
                .map_or_else(|| format!("{:#x}", p.pid), str::to_owned);
            links.push(ObjectLink::Entity {
                caption: format!("{name} → {d}"),
                did: d,
            });
        }
    }
    let pid = |n: &str| registry.pid(n);
    if let Some(list) = pid(WORN_LIST)
        .and_then(|p| set.get(p))
        .and_then(Value::as_array)
    {
        for (i, el) in list.iter().enumerate() {
            if links.len() >= MAX_CHOICES {
                break;
            }
            let Some(s) = el.value.as_struct() else {
                continue;
            };
            let field = |n: &str| pid(n).and_then(|p| s.get(p).map(|v| (p, v)));
            let Some(appearance) = field(WORN_APPEARANCE).and_then(|(_, v)| v.did_ref()) else {
                continue;
            };
            let Some(key) = field(WORN_KEY).and_then(|(_, v)| {
                v.as_enum()
                    .or_else(|| v.as_i64().and_then(|k| u32::try_from(k).ok()))
            }) else {
                continue;
            };
            let label = |n: &str| {
                field(n).map_or_else(String::new, |(p, v)| {
                    let l = label_text(&ctx.game.label(p, v), v, ctx.strings.as_ref());
                    if l.is_empty() {
                        crate::decoders::value_text(v)
                    } else {
                        l
                    }
                })
            };
            links.push(ObjectLink::Worn {
                caption: format!(
                    "worn #{i}: {} {} → {appearance} key {key:#x}",
                    label(WORN_SPECIES),
                    label(WORN_SEX)
                ),
                appearance,
                key,
            });
        }
    }
    Ok((links, set_did))
}

/// What resolving a chain gathers besides the part list.
#[derive(Debug, Default)]
struct Gathered {
    chain: Chain,
    notes: Vec<String>,
    dyes: Vec<DyeBlock>,
}

/// The parts of one appearance entry: its base block (or the first block, when none is
/// the base) with the block's material groups, plus the parts of any extra setups.
///
/// Extra setups are resolved once per DID and only while the scene has fewer than
/// [`MAX_PARTS`] parts; the rest are counted in one note, and failures are summarised in
/// one note, so a hostile entry with many extras costs neither time nor UI rows.
fn appearance_parts(
    ctx: &DataContext,
    record: &AppearanceRecord,
    key: u32,
    out: &mut Gathered,
    job: &JobContext<'_>,
) -> Result<Vec<PartSpec>, ExplorerError> {
    let Gathered { chain, notes, dyes } = out;
    let entry = record.entry(key).ok_or_else(|| {
        ExplorerError::NotApplicable(format!(
            "appearance {} has no entry for key {key:#x} (setup.md §3.2)",
            record.did
        ))
    })?;
    *dyes = dye_blocks(ctx, entry);
    let block = match entry.base_block() {
        Some(b) => b,
        None => {
            notes.push(format!(
                "entry {key:#x} has {} blocks and none is the base look (variant 1.0); \
                 showing the first block (setup.md §10.1 OPEN)",
                entry.blocks.len()
            ));
            entry.blocks.first().ok_or_else(|| {
                ExplorerError::NotApplicable(format!("entry {key:#x} has no blocks"))
            })?
        }
    };
    if entry.blocks.len() > 1 {
        notes.push(format!(
            "entry {key:#x}: {} blocks (dye variants are not applied; pick a dye colour instead)",
            entry.blocks.len()
        ));
    }
    let groups = Arc::new(block.groups.clone());
    let mut parts: Vec<PartSpec> = block
        .parts
        .iter()
        .map(|p| PartSpec {
            mesh: p.mesh,
            caption: format!("slot {:#x}", p.slot),
            groups: Some(Arc::clone(&groups)),
        })
        .collect();
    let mut seen = HashSet::new();
    let (mut repeated, mut not_resolved, mut failed) = (0usize, 0usize, 0usize);
    let mut failures = Vec::new();
    for extra in &block.extras {
        if job.is_cancelled() {
            return Err(ExplorerError::Cancelled);
        }
        if !seen.insert(extra.did) {
            repeated += 1;
            continue;
        }
        if parts.len() >= MAX_PARTS {
            not_resolved += 1;
            continue;
        }
        // Appearance extras are worn geometry: their collision is not shown.
        match setup_parts(ctx, extra.did, chain, None) {
            Ok(more) => parts.extend(more.into_iter().map(|p| PartSpec {
                caption: format!("extra {} {}", extra.did, p.caption),
                groups: Some(Arc::clone(&groups)),
                ..p
            })),
            Err(e) => {
                failed += 1;
                if failures.len() < 3 {
                    failures.push(format!("{}: {e}", extra.did));
                }
            }
        }
    }
    if failed > 0 {
        notes.push(format!(
            "{failed} extra setups could not be resolved ({}{})",
            failures.join("; "),
            if failed > failures.len() { "; …" } else { "" }
        ));
    }
    if repeated > 0 {
        notes.push(format!("{repeated} repeated extra setups were ignored"));
    }
    if not_resolved > 0 {
        notes.push(format!(
            "{not_resolved} further extra setups were not resolved (the scene already has \
             {MAX_PARTS} parts)"
        ));
    }
    if parts.is_empty() {
        notes.push(format!(
            "entry {key:#x} names no part meshes (a material-only entry, setup.md §3.2)"
        ));
    }
    Ok(parts)
}

// ---------------------------------------------------------------------------------------
// Materials and textures
// ---------------------------------------------------------------------------------------

/// Where a planned base texture's alpha comes from.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
enum AlphaPlan {
    Opaque,
    Own,
    From(Did),
}

/// A texture the scene needs, keyed so identical requests are decoded once.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
enum TexKey {
    Base {
        diffuse: Option<Did>,
        alpha: AlphaPlan,
        clamp: bool,
        nearest: bool,
    },
    Normal(Did),
    Mask {
        list: Did,
        invert: bool,
    },
}

/// A material's GPU description with texture keys not yet decoded.
#[derive(Debug, Clone, Copy)]
struct GpuPlan {
    base: Option<TexKey>,
    normal: Option<TexKey>,
    gloss: Option<TexKey>,
    dye: [Option<(TexKey, [f32; 4])>; 2],
    material: GpuMaterial,
}

fn sampler_flags(s: Option<&SamplerState>) -> (bool, bool) {
    let Some(s) = s else {
        return (false, false);
    };
    let clamp = matches!(s.address_u(), AddressMode::Clamp | AddressMode::Border)
        && matches!(s.address_v(), AddressMode::Clamp | AddressMode::Border);
    let nearest = s.mag_filter() == FilterMode::Point;
    (clamp, nearest)
}

fn finite01(v: f32, fallback: f32) -> f32 {
    if v.is_finite() {
        v.clamp(0.0, 1.0)
    } else {
        fallback
    }
}

/// Plans the renderer material of a resolved material (`material.md` §10–§12).
fn plan_material(m: &ResolvedMaterial) -> GpuPlan {
    let f = &m.features;
    let slot_tex = |id: u32| m.slot(id).and_then(|s| s.texture);
    // Opacity: the alpha-test source, else the blend's source alpha (§11.2, §11.3).
    let opacity = f
        .alpha_test
        .map(|a| a.source_slot)
        .or(if f.blend.is_transparent() {
            f.blend.opacity_slot
        } else {
            None
        });
    let alpha = match opacity {
        None => AlphaPlan::Opaque,
        Some(ids::DIFFUSE) => AlphaPlan::Own,
        // An opacity slot without a texture: opacity 1 (§18 OPEN).
        Some(other) => slot_tex(other).map_or(AlphaPlan::Opaque, AlphaPlan::From),
    };
    let diffuse = slot_tex(ids::DIFFUSE);
    let (clamp, nearest) = sampler_flags(m.slot(ids::DIFFUSE).and_then(|s| s.sampler.as_ref()));
    let base = (diffuse.is_some() || matches!(alpha, AlphaPlan::From(_))).then_some(TexKey::Base {
        diffuse,
        alpha: if diffuse.is_none() && alpha == AlphaPlan::Own {
            AlphaPlan::Opaque
        } else {
            alpha
        },
        clamp,
        nearest,
    });
    let normal = if f.normal_mapped {
        slot_tex(ids::NORMAL).map(TexKey::Normal)
    } else {
        None
    };
    let gloss = if f.specular {
        [ids::DIFFUSE, ids::NORMAL, ids::GLOSS]
            .into_iter()
            .filter(|&id| {
                m.slot(id)
                    .is_some_and(|s| s.packing.alpha == AlphaUse::Gloss)
            })
            .find_map(slot_tex)
            .map(|list| TexKey::Mask {
                list,
                invert: false,
            })
    } else {
        None
    };
    let mut dye = [None, None];
    if f.dyeable
        && let Some(setup) = m.dye
    {
        let channel = |mask: DyeMask, color: [f32; 4]| {
            mask.texture
                .map(|list| (TexKey::Mask { list, invert: true }, color))
        };
        dye[0] = channel(setup.primary.mask, setup.primary.color);
        dye[1] = setup.secondary.and_then(|s| channel(s.mask, s.color));
    }
    let alpha_kind = match f.blend.mode {
        BlendMode::Opaque => AlphaKind::Opaque,
        BlendMode::Cutout => f.alpha_test.map_or(AlphaKind::Opaque, |a| {
            AlphaKind::Mask(finite01(a.threshold, 0.5))
        }),
        BlendMode::AlphaBlend | BlendMode::Additive | BlendMode::Custom => AlphaKind::Blend,
    };
    let diffuse_color = m.params.color(ids::DIFFUSE_COLOR).unwrap_or([1.0; 4]);
    let base_color = [
        srgb_unit_to_linear(diffuse_color[0]),
        srgb_unit_to_linear(diffuse_color[1]),
        srgb_unit_to_linear(diffuse_color[2]),
        1.0,
    ];
    // Specular power 0 (the default of several common shader records, overridden by
    // materials that want a highlight) is taken as "no highlight": (N·H)^0 would light
    // every texel evenly (material.md §19 OPEN).
    let power = m
        .params
        .float(ids::SPECULAR_POWER)
        .filter(|p| p.is_finite() && *p > 0.0);
    let specular = match (f.specular, power) {
        (true, Some(power)) => {
            let c = m
                .params
                .color(ids::SPECULAR_COLOR)
                .unwrap_or([0.3, 0.3, 0.3, 1.0]);
            // Read as sRGB-encoded like 2000 and the dye colour, so the three material
            // colours share one convention (material.md §19 OPEN).
            Some((
                [0, 1, 2].map(|i| srgb_unit_to_linear(finite01(c[i], 0.3))),
                power,
            ))
        }
        _ => None,
    };
    GpuPlan {
        base,
        normal,
        gloss: gloss.filter(|_| specular.is_some()),
        dye,
        material: GpuMaterial {
            alpha: alpha_kind,
            two_sided: f.two_sided,
            base_color,
            specular,
            unlit: !f.lit,
            ..GpuMaterial::default()
        },
    }
}

/// Plans a preview of a bare material record (0x30, no shader context): its diffuse
/// texture, opaque.
fn plan_bare_material(rec: &MaterialRecord) -> GpuPlan {
    let diffuse = rec.get(ids::DIFFUSE).and_then(|e| e.value.as_texture());
    GpuPlan {
        base: diffuse.map(|d| TexKey::Base {
            diffuse: Some(d),
            alpha: AlphaPlan::Opaque,
            clamp: false,
            nearest: false,
        }),
        normal: None,
        gloss: None,
        dye: [None, None],
        material: GpuMaterial::default(),
    }
}

/// Short names of the parameter ids whose meaning `material.md` §8 gives.
#[must_use]
pub fn param_name(id: u32) -> &'static str {
    match id {
        ids::SPECULAR_POWER => "specular power",
        ids::ALPHA_THRESHOLD => "alpha-test threshold",
        ids::DEPTH_FADE => "depth fade",
        ids::DIFFUSE_COLOR => "diffuse colour",
        ids::SPECULAR_COLOR => "specular colour",
        ids::DYE_COLOR => "dye colour",
        ids::DYE_COLOR_2 => "second dye colour",
        ids::GLOW_COLOR => "glow colour",
        ids::ALPHA_TEST => "alpha test",
        ids::TWO_SIDED => "two-sided (candidate)",
        4002 => "fog (candidate)",
        1001 | 1002 => "UV0 scale",
        1003 | 1004 => "UV1 scale",
        1005 | 1006 => "UV2 scale",
        1017 | 1018 => "UV0 offset",
        1019 | 1020 => "UV1 offset",
        1021 | 1022 => "UV2 offset",
        1033 => "UV0 rotation",
        1034 => "UV1 rotation",
        1035 => "UV2 rotation",
        1070..=1075 => "rotation centre",
        3000..=3999 => SlotKind::of(id).name(),
        _ => "",
    }
}

fn origin_text(o: ParamOrigin) -> String {
    match o {
        ParamOrigin::Default => "shader default".into(),
        ParamOrigin::Material { index, did } => format!("material #{index} {did}"),
    }
}

fn param_value_text(v: &ParamValue) -> (String, Option<[f32; 4]>, Option<u32>) {
    match v {
        ParamValue::Float(c) => {
            let mut s = format!("{}", c.evaluate());
            if c.is_animated() {
                let _ = write!(s, " (curve mode {}, animated; shown as w[0])", c.mode);
            }
            (s, None, None)
        }
        ParamValue::Color(c) => (
            format!("({:.3}, {:.3}, {:.3}, {:.3})", c[0], c[1], c[2], c[3]),
            Some(*c),
            None,
        ),
        ParamValue::Texture(d) if d.is_null() => ("none".into(), None, None),
        ParamValue::Texture(d) => (d.to_string(), None, Some(d.0)),
        ParamValue::Flag(f) => (format!("{f}"), None, None),
    }
}

fn packing_text(p: tessera_material::vocab::ChannelPacking) -> String {
    let color = match p.color {
        ColorUse::BaseColor => "base colour",
        ColorUse::Normal => "normal",
        ColorUse::Emissive => "emissive",
        ColorUse::GlowMask => "glow mask",
        ColorUse::Reflection => "reflection",
        ColorUse::BakedLighting => "baked lighting",
        ColorUse::Unused => "unused",
        ColorUse::Other => "other",
    };
    let alpha = match p.alpha {
        AlphaUse::Unused => "unused",
        AlphaUse::Opacity => "opacity",
        AlphaUse::Gloss => "gloss",
        AlphaUse::DyeMask => "dye mask",
        AlphaUse::Unknown => "unknown",
    };
    format!("RGB {color} · A {alpha}")
}

fn sampler_text(s: Option<&SamplerState>) -> String {
    s.map_or_else(
        || "—".into(),
        |s| {
            format!(
                "{:?}/{:?} · {:?}/{:?}/{:?}",
                s.address_u(),
                s.address_v(),
                s.mag_filter(),
                s.min_filter(),
                s.mip_filter()
            )
        },
    )
}

fn view_of(m: &ResolvedMaterial, layered: Option<&str>) -> MaterialView {
    let f = &m.features;
    let mut v = MaterialView::default();
    if let Some(s) = m.surface {
        v.links.push(("surface".into(), s.0));
    }
    v.links.push(("shader".into(), m.shader.did.0));
    for (i, d) in m.materials.iter().enumerate() {
        v.links.push((format!("material #{i}"), d.0));
    }
    if let Some(l) = layered {
        v.facts.push(("appearance layer".into(), l.to_owned()));
    }
    if let Some(k) = m.binding_key {
        v.facts.push(("binding key".into(), format!("{k:#x}")));
    }
    v.facts.push((
        "shader".into(),
        format!(
            "{} · {} permutations{}",
            if f.lit { "lit" } else { "unlit" },
            m.shader.permutation_count,
            if m.shader.cull_none {
                " · cull none"
            } else {
                ""
            }
        ),
    ));
    v.facts.push((
        "blend".into(),
        format!(
            "{:?} ({:?} / {:?} / {:?}, depth write {}, source alpha {})",
            f.blend.mode,
            f.blend.src,
            f.blend.dst,
            f.blend.op,
            f.blend.depth_write,
            f.blend
                .opacity_slot
                .map_or_else(|| "1".to_owned(), |s| format!("slot {s}"))
        ),
    ));
    if let Some(a) = f.alpha_test {
        v.facts.push((
            "alpha test".into(),
            format!(
                "opacity ≥ {} (alpha of slot {})",
                a.threshold, a.source_slot
            ),
        ));
    }
    if let Some(r) = m.shader.default_alpha_ref {
        v.facts
            .push(("render-state alpha ref".into(), format!("{r}")));
    }
    let named = [
        (f.lit, "lit"),
        (f.two_sided, "two-sided"),
        (f.normal_mapped, "normal-mapped"),
        (f.specular, "specular"),
        (f.dyeable, "dyeable"),
        (f.dyeable_2, "second dye"),
        (f.emissive, "emissive"),
        (f.glow, "glow"),
        (f.environment, "environment"),
        (f.second_layer, "second layer"),
        (f.baked_lighting, "baked lighting"),
        (f.uv_transform, "UV transform"),
        (f.animated, "animated"),
    ];
    v.flags = named
        .iter()
        .filter(|(on, _)| *on)
        .map(|(_, n)| (*n).to_owned())
        .collect();
    if let Some(d) = &m.dye {
        let mut push = |name: &str, ch: &tessera_material::resolve::DyeChannel| {
            v.facts.push((
                name.into(),
                format!(
                    "mask alpha of slot {} ({}) · colour ({:.3}, {:.3}, {:.3})",
                    ch.mask.slot,
                    ch.mask
                        .texture
                        .map_or_else(|| "no texture".into(), |t| t.to_string()),
                    ch.color[0],
                    ch.color[1],
                    ch.color[2]
                ),
            ));
        };
        push("dye", &d.primary);
        if let Some(s) = &d.secondary {
            push("second dye", s);
        }
    }
    for p in m.params.params() {
        let (value, color, link) = param_value_text(&p.value);
        v.params.push(ParamRow {
            id: p.id,
            name: param_name(p.id),
            value,
            color,
            link,
            origin: origin_text(p.origin),
        });
    }
    for s in &m.skipped {
        v.notes
            .push(format!("material {} left out: {}", s.did, s.reason));
    }
    let ignored = m.params.ignored().len() + m.params.ignored_omitted();
    if ignored > 0 {
        v.notes.push(format!(
            "{ignored} material entries have no effect (id not declared by the shader, §9.2)"
        ));
    }
    if matches!(f.blend.mode, BlendMode::Additive | BlendMode::Custom) {
        v.notes.push(format!(
            "{:?} blending is drawn as alpha blending by the viewer",
            f.blend.mode
        ));
    }
    if f.blend.is_transparent() && f.alpha_test.is_some() {
        v.notes
            .push("blended and alpha-tested: the viewer blends without the test".into());
    }
    if let Some(c) = m.params.color(ids::DIFFUSE_COLOR)
        && c[..3].iter().any(|v| *v < 0.999)
    {
        v.notes.push(format!(
            "diffuse colour 2000 is ({:.3}, {:.3}, {:.3}, {:.3}), not white: applied only \
             with \"Diffuse colour\" on (material.md §19 OPEN)",
            c[0], c[1], c[2], c[3]
        ));
    }
    if m.dye.is_some_and(|d| d.secondary.is_some()) {
        v.notes.push(
            "two dye channels: the renderer draws one at a time (dye 1, or dye 2 while it \
             is previewed)"
                .into(),
        );
    }
    v
}

/// What the viewer samples a slot for, as text.
fn used_as(m: &ResolvedMaterial, plan: &GpuPlan, id: u32) -> String {
    let mut uses = Vec::new();
    if id == ids::DIFFUSE && plan.base.is_some() {
        uses.push("base colour");
    }
    if let Some(TexKey::Base {
        alpha: AlphaPlan::Own,
        ..
    }) = plan.base
        && id == ids::DIFFUSE
    {
        uses.push("opacity");
    }
    if let Some(TexKey::Base {
        alpha: AlphaPlan::From(d),
        ..
    }) = plan.base
        && m.slot(id).and_then(|s| s.texture) == Some(d)
        && id != ids::DIFFUSE
    {
        uses.push("opacity");
    }
    if id == ids::NORMAL && plan.normal.is_some() {
        uses.push("normal map");
    }
    let tex = m.slot(id).and_then(|s| s.texture);
    if let Some(TexKey::Mask { list, .. }) = plan.gloss
        && tex == Some(list)
        && m.slot(id)
            .is_some_and(|s| s.packing.alpha == AlphaUse::Gloss)
    {
        uses.push("gloss");
    }
    for (i, d) in plan.dye.iter().enumerate() {
        if let Some((TexKey::Mask { list, .. }, _)) = d
            && tex == Some(*list)
            && m.dye.is_some_and(|s| {
                let ch = if i == 0 { Some(s.primary) } else { s.secondary };
                ch.is_some_and(|c| c.mask.slot == id)
            })
        {
            uses.push(if i == 0 {
                "dye mask"
            } else {
                "second dye mask"
            });
        }
    }
    if uses.is_empty() {
        "—".into()
    } else {
        uses.join(", ")
    }
}

/// (surface, matching (shader override, material) layers) of an assembled part's draw.
type LayeredKey = (Did, Vec<(Option<Did>, Did)>);

/// Collects distinct textures and thumbnails while materials are added.
pub(crate) struct Collector<'a> {
    ctx: &'a DataContext,
    keys: Vec<TexKey>,
    key_index: HashMap<TexKey, usize>,
    plans: Vec<GpuPlan>,
    materials: Vec<SceneMaterial>,
    material_index: HashMap<(Did, Option<(Did, Did)>), usize>,
    /// Materials of assembled parts by (surface, matching layers).
    layered_index: HashMap<LayeredKey, usize>,
    thumb_lists: Vec<(Did, bool)>,
    thumb_index: HashMap<(Did, bool), usize>,
    notes: Vec<String>,
}

impl<'a> Collector<'a> {
    pub(crate) fn new(ctx: &'a DataContext) -> Self {
        Self {
            ctx,
            keys: Vec::new(),
            key_index: HashMap::new(),
            plans: Vec::new(),
            materials: Vec::new(),
            material_index: HashMap::new(),
            layered_index: HashMap::new(),
            thumb_lists: Vec::new(),
            thumb_index: HashMap::new(),
            notes: Vec::new(),
        }
    }

    fn key(&mut self, k: TexKey) {
        if !self.key_index.contains_key(&k) {
            self.key_index.insert(k, self.keys.len());
            self.keys.push(k);
        }
    }

    /// A thumbnail of `list`: its colour channels opaque, or with `alpha` its alpha
    /// channel as grey (for textures whose RGB is unused, `material.md` §10.2).
    fn thumb(&mut self, list: Did, alpha: bool) -> Option<usize> {
        if let Some(&i) = self.thumb_index.get(&(list, alpha)) {
            return Some(i);
        }
        if self.thumb_lists.len() >= MAX_THUMBS {
            return None;
        }
        let i = self.thumb_lists.len();
        self.thumb_lists.push((list, alpha));
        self.thumb_index.insert((list, alpha), i);
        Some(i)
    }

    fn add(&mut self, title: String, plan: GpuPlan, view: MaterialView, dyeable: bool) -> usize {
        for k in [plan.base, plan.normal, plan.gloss]
            .into_iter()
            .chain(plan.dye.iter().map(|d| d.map(|(k, _)| k)))
            .flatten()
        {
            self.key(k);
        }
        let i = self.materials.len();
        self.plans.push(plan);
        self.materials.push(SceneMaterial {
            title,
            gpu: plan.material,
            dyeable,
            view,
            error: None,
        });
        i
    }

    fn add_failed(&mut self, title: String, error: String) -> usize {
        let i = self.materials.len();
        self.plans.push(GpuPlan {
            base: None,
            normal: None,
            gloss: None,
            dye: [None, None],
            material: GpuMaterial::default(),
        });
        self.materials.push(SceneMaterial {
            title,
            gpu: GpuMaterial {
                base_color: [0.55, 0.55, 0.58, 1.0],
                ..GpuMaterial::default()
            },
            dyeable: false,
            view: MaterialView::default(),
            error: Some(error),
        });
        i
    }

    fn add_resolved(
        &mut self,
        m: &ResolvedMaterial,
        title: String,
        layered: Option<&str>,
    ) -> usize {
        let plan = plan_material(m);
        let mut view = view_of(m, layered);
        for s in &m.slots {
            let alpha = s.packing.color == ColorUse::Unused;
            let thumb = s.texture.and_then(|t| self.thumb(t, alpha));
            view.slots.push(SlotRow {
                id: s.id,
                kind: s.kind.name().to_owned(),
                role: s.role.clone().unwrap_or_else(|| "—".into()),
                packing: packing_text(s.packing),
                texture: s.texture.map(|d| d.0),
                sampler: sampler_text(s.sampler.as_ref()),
                origin: origin_text(s.origin),
                used_as: used_as(m, &plan, s.id),
                thumb,
            });
        }
        self.add(title, plan, view, m.features.dyeable)
    }

    /// The material of one draw: the surface record, layered with the appearance group
    /// whose binding key matches (`material.md` §9.3).
    pub(crate) fn surface_material(
        &mut self,
        surface: Did,
        groups: Option<&[MaterialGroup]>,
    ) -> usize {
        let lib = &self.ctx.materials;
        let group = groups.and_then(|g| {
            let key = lib.surface_record(surface).ok()?.binding_key;
            g.iter().find(|g| g.binding_key == key).copied()
        });
        let layer_key = group.map(|g| (g.shader, g.material));
        if let Some(&i) = self.material_index.get(&(surface, layer_key)) {
            return i;
        }
        let title = match group {
            Some(g) if !g.material.is_null() => format!("{surface} + {}", g.material),
            _ => surface.to_string(),
        };
        let i = match group {
            Some(g) => {
                let layering = Layering {
                    shader_override: (!g.shader.is_null()).then_some(g.shader),
                    materials: if g.material.is_null() {
                        Vec::new()
                    } else {
                        vec![g.material]
                    },
                };
                let what = format!(
                    "group material {}{}",
                    g.material,
                    if g.shader.is_null() {
                        String::new()
                    } else {
                        format!(", shader override {}", g.shader)
                    }
                );
                match lib.resolve_layered(surface, &layering) {
                    Ok(m) => self.add_resolved(&m, title, Some(&what)),
                    Err(e) => self.add_failed(title, e.to_string()),
                }
            }
            None => match lib.resolve_surface(surface) {
                Ok(m) => self.add_resolved(&m, title, None),
                Err(e) => self.add_failed(title, e.to_string()),
            },
        };
        self.material_index.insert((surface, layer_key), i);
        i
    }

    /// Decodes the textures and thumbnails; returns them with the final GPU materials.
    /// The material of one draw of an assembled part: the surface record layered with
    /// every layer of `layers` (material groups, then dye blocks, in application order,
    /// `material.md` §9.2) whose binding key matches the surface's (`material.md` §9.3,
    /// `appearance.md` §8 step 6). The first layer with a shader override sets the shader.
    pub(crate) fn surface_material_layered(
        &mut self,
        surface: Did,
        layers: &[(Option<Did>, Did, u32)],
    ) -> usize {
        let lib = &self.ctx.materials;
        let key = lib.surface_record(surface).ok().map(|r| r.binding_key);
        let matching: Vec<(Option<Did>, Did)> = layers
            .iter()
            .filter(|(_, m, k)| Some(*k) == key && !m.is_null())
            .map(|&(sh, m, _)| (sh, m))
            .take(16)
            .collect();
        if matching.is_empty() {
            return self.surface_material(surface, None);
        }
        let cache_key = (surface, matching.clone());
        if let Some(&i) = self.layered_index.get(&cache_key) {
            return i;
        }
        let shader = matching.iter().find_map(|(sh, _)| *sh);
        let materials: Vec<Did> = matching.iter().map(|(_, m)| *m).collect();
        let title = format!(
            "{surface} + {}",
            materials
                .iter()
                .map(ToString::to_string)
                .collect::<Vec<_>>()
                .join(" + ")
        );
        let what = format!(
            "assembled layers {}{}",
            materials
                .iter()
                .map(ToString::to_string)
                .collect::<Vec<_>>()
                .join(", "),
            shader.map_or_else(String::new, |s| format!(", shader override {s}"))
        );
        let layering = Layering {
            shader_override: shader,
            materials,
        };
        let i = match lib.resolve_layered(surface, &layering) {
            Ok(m) => self.add_resolved(&m, title, Some(&what)),
            Err(e) => self.add_failed(title, e.to_string()),
        };
        self.layered_index.insert(cache_key, i);
        i
    }

    pub(crate) fn finish(mut self, job: &JobContext<'_>) -> Result<Finished, ExplorerError> {
        let lib = &self.ctx.materials;
        let cap = texture_cap(self.keys.len());
        // The library picks the first level within the cap, or the chain's last level
        // when every level is larger (single-level lists): enforce the cap here.
        let load = |list: Did| -> Result<Arc<RgbaImage>, String> {
            let t = lib
                .texture(list, Some(cap))
                .map_err(|e| format!("{list}: {e}"))?;
            match fit_image(&t.image, cap) {
                Some(std::borrow::Cow::Borrowed(_)) => Ok(Arc::clone(&t.image)),
                Some(std::borrow::Cow::Owned(small)) => Ok(Arc::new(small)),
                None => Err(format!("{list}: inconsistent image")),
            }
        };
        let made = make_textures(&self.keys, TEXTURE_BUDGET, &load, &|| job.is_cancelled())?;
        self.notes.extend(made.notes);
        for (m, plan) in self.materials.iter_mut().zip(&self.plans) {
            let at = |k: Option<TexKey>| k.and_then(|k| made.slot_of.get(&k).copied());
            m.gpu.base = at(plan.base);
            m.gpu.normal = at(plan.normal);
            m.gpu.gloss = at(plan.gloss);
            for (i, d) in plan.dye.iter().enumerate() {
                m.gpu.dye[i] = d.and_then(|(k, color)| {
                    made.slot_of.get(&k).map(|&mask| GpuDye { mask, color })
                });
            }
        }
        let mut thumbs = Vec::with_capacity(self.thumb_lists.len());
        for &(list, alpha) in &self.thumb_lists {
            if job.is_cancelled() {
                return Err(ExplorerError::Cancelled);
            }
            // A list whose levels are all larger than the request decodes its last level
            // (up to the library's size limit). The cost is accepted: decoded levels are
            // cached by the library, so a thumbnail of a texture the scene also draws
            // shares that decode, and the thumbnail itself is box-filtered down.
            let (image, info) = match lib.texture(list, Some(THUMB_SIDE * 2)) {
                Ok(t) => match thumbnail(&t.image, THUMB_SIDE) {
                    Some(th) => (
                        thumb_image(&th, alpha),
                        format!(
                            "{}level {} {}×{} {}",
                            if alpha {
                                "alpha shown as grey · "
                            } else {
                                "RGB (alpha not shown) · "
                            },
                            t.level,
                            t.image.width,
                            t.image.height,
                            t.format.name()
                        ),
                    ),
                    None => (blank_thumb(), "empty".into()),
                },
                Err(e) => (blank_thumb(), e.to_string()),
            };
            thumbs.push(Thumb {
                list: list.0,
                alpha,
                image: Arc::new(image),
                info,
            });
        }
        Ok(Finished {
            materials: self.materials,
            textures: made.textures,
            thumbs,
            notes: self.notes,
        })
    }
}

/// The resolution cap of a scene's textures by how many it needs, so a large scene
/// stays within [`TEXTURE_BUDGET`].
fn texture_cap(keys: usize) -> u32 {
    match keys.min(MAX_SCENE_TEXTURES) {
        0..=24 => 1024,
        25..=96 => 512,
        _ => 256,
    }
}

/// What [`make_textures`] produces.
struct MadeTextures {
    textures: Vec<Arc<SceneTexture>>,
    slot_of: HashMap<TexKey, usize>,
    notes: Vec<String>,
}

/// Most per-texture notes [`make_textures`] writes.
const MAX_TEXTURE_NOTES: usize = 16;

/// Builds the GPU textures of `keys` (the first [`MAX_SCENE_TEXTURES`]) within `budget`
/// bytes ([`TEXTURE_BUDGET`]). `load` decodes a texture list (at the scene's resolution cap).
/// Problems are reported in the notes: decode failures, textures left out for the
/// budget (by name), and a separate opacity map that failed (its base colour is then
/// drawn opaque instead of being dropped).
fn make_textures(
    keys: &[TexKey],
    budget: usize,
    load: &dyn Fn(Did) -> Result<Arc<RgbaImage>, String>,
    cancelled: &dyn Fn() -> bool,
) -> Result<MadeTextures, ExplorerError> {
    let mut notes = Vec::new();
    let note = |notes: &mut Vec<String>, n: String| {
        if notes.len() < MAX_TEXTURE_NOTES {
            notes.push(n);
        }
    };
    if keys.len() > MAX_SCENE_TEXTURES {
        notes.push(format!(
            "{} textures needed; only the first {MAX_SCENE_TEXTURES} are loaded",
            keys.len()
        ));
    }
    let mut textures: Vec<Arc<SceneTexture>> = Vec::new();
    let mut slot_of: HashMap<TexKey, usize> = HashMap::new();
    let mut bytes = 0usize;
    let mut failures = 0usize;
    let mut over_budget: Vec<String> = Vec::new();
    let mut over_budget_count = 0usize;
    for key in keys.iter().take(MAX_SCENE_TEXTURES) {
        if cancelled() {
            return Err(ExplorerError::Cancelled);
        }
        let made: Result<Option<SceneTexture>, String> = match *key {
            TexKey::Base {
                diffuse,
                alpha,
                clamp,
                nearest,
            } => (|| {
                let base = diffuse.map(load).transpose()?;
                let other = match alpha {
                    AlphaPlan::From(d) => match load(d) {
                        Ok(o) => Some(o),
                        Err(e) => {
                            note(
                                &mut notes,
                                format!("opacity map {e}; its base colour is drawn opaque"),
                            );
                            None
                        }
                    },
                    _ => None,
                };
                let src = match (alpha, &other) {
                    (AlphaPlan::Own, _) => AlphaSource::Own,
                    (AlphaPlan::From(_), Some(o)) => AlphaSource::From(o),
                    _ => AlphaSource::Opaque,
                };
                Ok(base_texture(base.as_deref(), src).map(|img| SceneTexture {
                    label: format!(
                        "base {}",
                        diffuse.map_or_else(|| "white".into(), |d| d.to_string())
                    ),
                    width: img.width,
                    height: img.height,
                    rgba: img.pixels,
                    linear: false,
                    clamp,
                    nearest,
                }))
            })(),
            TexKey::Normal(list) => load(list).map(|img| {
                Some(SceneTexture {
                    label: format!("normal {list}"),
                    width: img.width,
                    height: img.height,
                    rgba: img.pixels.clone(),
                    linear: true,
                    clamp: false,
                    nearest: false,
                })
            }),
            TexKey::Mask { list, invert } => load(list).map(|img| {
                alpha_mask(&img, invert).map(|m| SceneTexture {
                    label: format!("{} mask {list}", if invert { "dye" } else { "gloss" }),
                    width: m.width,
                    height: m.height,
                    rgba: m.pixels,
                    linear: true,
                    clamp: false,
                    nearest: false,
                })
            }),
        };
        match made {
            Ok(Some(t)) => {
                if bytes + t.rgba.len() > budget {
                    over_budget_count += 1;
                    if over_budget.len() < 4 {
                        over_budget.push(format!("{} ({}×{})", t.label, t.width, t.height));
                    }
                    continue;
                }
                bytes += t.rgba.len();
                slot_of.insert(*key, textures.len());
                textures.push(Arc::new(t));
            }
            Ok(None) => failures += 1,
            Err(e) => {
                failures += 1;
                note(&mut notes, format!("texture {e}"));
            }
        }
    }
    if failures > 0 {
        notes.push(format!(
            "{failures} textures could not be loaded; their materials draw without them"
        ));
    }
    if over_budget_count > 0 {
        notes.push(format!(
            "{over_budget_count} textures left out for the scene's {} MiB texture budget: \
             {}{}",
            budget >> 20,
            over_budget.join(", "),
            if over_budget_count > over_budget.len() {
                ", …"
            } else {
                ""
            }
        ));
    }
    Ok(MadeTextures {
        textures,
        slot_of,
        notes,
    })
}

/// A 1×1 transparent thumbnail (for lists that could not be decoded).
fn blank_thumb() -> RgbaImage {
    RgbaImage {
        width: 1,
        height: 1,
        pixels: vec![0; 4],
    }
}

/// An opaque thumbnail: its RGB, or with `alpha` its alpha as grey.
fn thumb_image(th: &RgbaImage, alpha: bool) -> RgbaImage {
    let pixels = th
        .pixels
        .as_chunks::<4>()
        .0
        .iter()
        .flat_map(|p| {
            if alpha {
                [p[3], p[3], p[3], 255]
            } else {
                [p[0], p[1], p[2], 255]
            }
        })
        .collect();
    RgbaImage {
        width: th.width,
        height: th.height,
        pixels,
    }
}

/// What [`Collector::finish`] produces.
pub(crate) struct Finished {
    pub(crate) materials: Vec<SceneMaterial>,
    pub(crate) textures: Vec<Arc<SceneTexture>>,
    pub(crate) thumbs: Vec<Thumb>,
    pub(crate) notes: Vec<String>,
}

// ---------------------------------------------------------------------------------------
// Parts
// ---------------------------------------------------------------------------------------

pub(crate) const IDENTITY: [f32; 16] = [
    1.0, 0.0, 0.0, 0.0, 0.0, 1.0, 0.0, 0.0, 0.0, 0.0, 1.0, 0.0, 0.0, 0.0, 0.0, 1.0,
];

fn level_kind_name(k: LevelKind) -> String {
    match k {
        LevelKind::Detail => "detail".into(),
        LevelKind::Proxy => "proxy".into(),
        LevelKind::Other(v) => format!("kind {v}"),
    }
}

/// What is left of a scene's geometry budget ([`SceneLimits`]).
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(crate) struct GeometryBudget {
    vertices: u64,
    indices: u64,
    bytes: u64,
    /// Parts not built because the budget was used up or too small for them.
    pub(crate) refused: usize,
}

impl GeometryBudget {
    pub(crate) fn new(limits: &SceneLimits) -> Self {
        Self {
            vertices: limits.max_vertices,
            indices: limits.max_indices,
            bytes: limits.max_stream_bytes,
            refused: 0,
        }
    }

    /// The per-level limits for the next part: the defaults, lowered to what is left, so
    /// `tessera-mesh` refuses an over-budget level before allocating its streams.
    fn limits(&self) -> GeometryLimits {
        let d = GeometryLimits::default();
        GeometryLimits {
            max_vertices: d.max_vertices.min(self.vertices),
            max_indices: d.max_indices.min(self.indices),
            max_bytes: d.max_bytes.min(self.bytes),
            ..d
        }
    }

    /// Whether the budget is below the default per-level limits.
    fn binding(&self) -> bool {
        self.limits() != GeometryLimits::default()
    }

    fn exhausted(&self) -> bool {
        self.vertices == 0 || self.indices == 0 || self.bytes == 0
    }

    fn take(&mut self, vertices: u64, indices: u64, bytes: u64) {
        self.vertices = self.vertices.saturating_sub(vertices);
        self.indices = self.indices.saturating_sub(indices);
        self.bytes = self.bytes.saturating_sub(bytes);
    }
}

/// Decodes the mesh of a part and prepares its drawn level within `budget`. The result
/// has no caption and no materials yet ([`place_part`] adds them), so parts that repeat
/// a mesh share it.
pub(crate) fn build_mesh_part(
    ctx: &DataContext,
    did: Did,
    req: SceneRequest,
    budget: &mut GeometryBudget,
) -> ScenePart {
    build_part(ctx, did, req, budget, None, None)
}

/// [`build_mesh_part`], skinned when `skin_bones` is the bone count of the figure's
/// skeleton and the drawn level has bone influences (`skeleton-anim.md` §4.1, §9.1): the
/// prepared streams then carry four influences per vertex ([`crate::geometry::mesh_skin`])
/// and the part is marked [`ScenePart::skinned`]. A level whose bones exceed the skeleton
/// or whose weights cannot be drawn is prepared rigid (bind pose), with the reason in the
/// part's issues.
pub(crate) fn build_part(
    ctx: &DataContext,
    did: Did,
    req: SceneRequest,
    budget: &mut GeometryBudget,
    skin_bones: Option<usize>,
    deform: Option<&tessera_anim::BindDeformation>,
) -> ScenePart {
    build_part_from(ctx, did, None, req, budget, skin_bones, deform)
}

/// [`build_part`] of a mesh record already read: `record` holds its bytes and directory
/// stamp (the embedded render mesh of a cell, `collision.md` §5.7); `None` reads `did`.
pub(crate) fn build_part_from(
    ctx: &DataContext,
    did: Did,
    record: Option<(Vec<u8>, u16)>,
    req: SceneRequest,
    budget: &mut GeometryBudget,
    skin_bones: Option<usize>,
    deform: Option<&tessera_anim::BindDeformation>,
) -> ScenePart {
    let mut part = ScenePart {
        mesh: did.0,
        caption: String::new(),
        level: 0,
        levels: Vec::new(),
        flags: 0,
        transform: IDENTITY,
        prepared: None,
        error: None,
        submeshes: Vec::new(),
        issues: Vec::new(),
        vertices: 0,
        triangles: 0,
        sphere: None,
        skinned: false,
        attach: None,
        figure: 0,
    };
    if budget.exhausted() {
        budget.refused += 1;
        part.error = Some("left out: the scene's geometry budget is used up".into());
        return part;
    }
    let read = match record {
        Some(r) => Ok(r),
        None => ctx
            .set
            .read_with_entry(did.0)
            .map(|(located, bytes)| (bytes, located.entry.stamp))
            .map_err(ExplorerError::from),
    };
    let mesh = match read.and_then(|(bytes, stamp)| {
        Mesh::decode_expecting(&bytes, did, Some(stamp), &DecodeOptions::render_only())
            .map_err(ExplorerError::from)
    }) {
        Ok(m) => m,
        Err(e) => {
            part.error = Some(e.to_string());
            return part;
        }
    };
    part.flags = mesh.flags;
    part.levels = mesh
        .levels
        .iter()
        .enumerate()
        .map(|(i, l)| LevelSummary {
            kind: level_kind_name(l.kind),
            draws: l.draws.len(),
            triangles: mesh.triangle_count(i),
        })
        .collect();
    let report = mesh.validate();
    part.issues = report
        .issues
        .iter()
        .take(6)
        .map(ToString::to_string)
        .collect();
    if report.issues.len() > 6 || report.dropped > 0 {
        part.issues.push(format!(
            "… {} more",
            (report.issues.len() - part.issues.len()) as u64 + report.dropped
        ));
    }
    part.level = usize::from(req.lod).min(mesh.levels.len().saturating_sub(1));
    let geo = match mesh.level_geometry(part.level, &budget.limits()) {
        Ok(g) => g,
        Err(e) => {
            part.error = Some(
                if budget.binding() && matches!(e, MeshError::LimitExceeded { .. }) {
                    budget.refused += 1;
                    format!("left out for the scene's geometry budget: {e}")
                } else {
                    e.to_string()
                },
            );
            return part;
        }
    };
    budget.take(
        geo.vertex_count() as u64,
        geo.indices.len() as u64,
        geo.stream_bytes(),
    );
    part.vertices = geo.vertex_count();
    part.triangles = geo.triangle_count();
    part.sphere = geo
        .bounds
        .map(|b| (b.sphere.center, b.sphere.radius))
        .filter(|(c, r)| c.iter().all(|v| v.is_finite()) && r.is_finite());
    for p in &geo.parts {
        let sm = mesh.submeshes.get(p.submesh as usize);
        part.submeshes.push(SceneSubmesh {
            draw: p.draw,
            submesh: p.submesh,
            surface: p.surface.map(|d| d.0),
            format: sm.map_or(0, |s| s.format.raw()),
            vertices: p.vertex_count,
            triangles: p.index_count / 3,
            skinned: p.skinned,
            aabb: sm.map(|s| (s.stored_aabb.min, s.stored_aabb.max)),
            material: None,
        });
    }
    let options = ConvertOptions {
        vertex_colors: req.vertex_colors,
        uv_set: 0,
    };
    let mut data = level_mesh_data(&geo, &options, format!("{did} level {}", part.level));
    let skin = skin_bones.and_then(|bones| {
        if !geo.parts.iter().any(|p| p.skinned) {
            return None;
        }
        match mesh_skin(&geo, bones) {
            Ok(s) => {
                if s.fixed > 0 {
                    part.issues.push(format!(
                        "{} vertices without influences follow bone 0 (rigid in a skinned \
                         mesh)",
                        s.fixed
                    ));
                }
                Some(s.skin)
            }
            Err(e) => {
                part.issues
                    .push(format!("drawn in bind pose, not skinned: {e}"));
                None
            }
        }
    });
    let ranges: Vec<(usize, usize)> = geo
        .parts
        .iter()
        .map(|p| (p.base_vertex as usize, p.vertex_count as usize))
        .collect();
    drop(geo);
    // The figure's bind-space deformation (`appearance.md` §6.4 step 4), shared with the
    // client: skinned vertices are deformed once, then animated as usual.
    if let (Some(d), Some(sk)) = (deform, &skin) {
        d.deform(
            &mut data.positions,
            data.normals.as_deref_mut(),
            data.tangents.as_deref_mut(),
            &sk.joints,
            &sk.weights,
        );
        // The stored binormals (render.md §4.8, R-D10) turn with the tangents: the
        // deformer's direction path (the same blended transform without translation,
        // renormalised, zero kept) applied with throw-away positions.
        if let Some(b) = data.binormals.as_deref_mut() {
            let mut scratch = vec![[0.0f32; 3]; b.len()];
            d.deform(&mut scratch, Some(b), None, &sk.joints, &sk.weights);
        }
        // The stored boxes describe the undeformed mesh: the scene's bounds (framing, the
        // sheet's bind-pose top) come from the deformed vertices.
        for (s, &(first, count)) in part.submeshes.iter_mut().zip(&ranges) {
            let verts = data
                .positions
                .get(first..first.saturating_add(count))
                .unwrap_or(&[]);
            if let Some(b) = positions_aabb(verts) {
                s.aabb = Some(b);
            }
        }
        if let Some((lo, hi)) = positions_aabb(&data.positions) {
            let c = [0, 1, 2].map(|i| (lo[i] + hi[i]) * 0.5);
            let r = (0..3)
                .map(|i| (hi[i] - lo[i]) * 0.5)
                .map(|h| h * h)
                .sum::<f32>()
                .sqrt();
            part.sphere = Some((c, r));
        }
    }
    let prepared = match &skin {
        Some(sk) => data.prepare_skinned(sk, &MeshConfig::default()),
        None => data.prepare(&MeshConfig::default()),
    };
    match prepared {
        Ok(p) => {
            part.prepared = Some(Arc::new(p));
            part.skinned = skin.is_some();
        }
        Err(e) => part.error = Some(format!("renderer refused the level: {e}")),
    }
    part
}

/// The box of finite `positions` (`None` without one).
fn positions_aabb(positions: &[[f32; 3]]) -> Option<([f32; 3], [f32; 3])> {
    let mut out: Option<([f32; 3], [f32; 3])> = None;
    for p in positions.iter().filter(|p| p.iter().all(|v| v.is_finite())) {
        let b = out.get_or_insert((*p, *p));
        for (i, &v) in p.iter().enumerate() {
            b.0[i] = b.0[i].min(v);
            b.1[i] = b.1[i].max(v);
        }
    }
    out
}

/// A built mesh part placed in the scene as `spec`: its caption, and each draw's
/// material (the surface record layered with the spec's appearance groups).
fn place_part(mut part: ScenePart, spec: &PartSpec, mats: &mut Collector<'_>) -> ScenePart {
    part.caption.clone_from(&spec.caption);
    for s in &mut part.submeshes {
        s.material = s
            .surface
            .map(|d| mats.surface_material(Did(d), spec.groups.as_deref().map(Vec::as_slice)));
    }
    part
}

/// The preview sphere's streams: radius 0.5, poles on ±Z (`v` = 0 at the top), resting
/// on the ground plane (z = 0) like a prop (`mesh.md` §11.1). The renderer's sphere is
/// Y-up (`render.md` §1), so it is turned into the viewer's Z-up frame first; otherwise
/// the default camera would look straight into a pole.
fn sphere_data() -> MeshData {
    let mut data = MeshData::uv_sphere(0.5, 64, 32);
    for p in &mut data.positions {
        *p = y_up_to_z_up(*p);
        p[2] += 0.5;
    }
    if let Some(normals) = &mut data.normals {
        for n in normals {
            *n = y_up_to_z_up(*n);
        }
    }
    data.tangents = None;
    data.compute_tangents();
    data.label = Some("preview sphere".into());
    data
}

/// A preview sphere part (for surface records and materials).
fn sphere_part(material: usize, caption: &str) -> ScenePart {
    let data = sphere_data();
    let mut part = ScenePart {
        mesh: 0,
        caption: caption.to_owned(),
        level: 0,
        levels: vec![LevelSummary {
            kind: "preview".into(),
            draws: 1,
            triangles: data.indices.len() / 3,
        }],
        flags: 0,
        transform: IDENTITY,
        prepared: None,
        error: None,
        submeshes: Vec::new(),
        issues: Vec::new(),
        vertices: data.positions.len(),
        triangles: data.indices.len() / 3,
        sphere: Some(([0.0, 0.0, 0.5], 0.5)),
        skinned: false,
        attach: None,
        figure: 0,
    };
    match data.prepare(&MeshConfig::default()) {
        Ok(p) => part.prepared = Some(Arc::new(p)),
        Err(e) => part.error = Some(e.to_string()),
    }
    part.submeshes.push(SceneSubmesh {
        draw: 0,
        submesh: 0,
        surface: None,
        format: 0,
        vertices: part.vertices as u32,
        triangles: part.triangles as u32,
        skinned: false,
        aabb: Some(([-0.5, -0.5, 0.0], [0.5, 0.5, 1.0])),
        material: Some(material),
    });
    part
}

/// Builds the scene of `did` (see the module documentation) within the default
/// [`SceneLimits`].
///
/// # Errors
///
/// When the record cannot be read or decoded, has no geometry chain, or the job is
/// cancelled. Problems of single parts, materials and textures are reported inside the
/// product instead.
pub fn build_scene(
    ctx: &DataContext,
    did: u32,
    req: SceneRequest,
    job: &JobContext<'_>,
) -> Result<SceneProduct, ExplorerError> {
    build_scene_limited(ctx, did, req, job, &SceneLimits::default())
}

/// [`build_scene`] within a given geometry budget: part meshes beyond it are left out
/// (with an error on the part and a note).
///
/// # Errors
///
/// As [`build_scene`].
pub fn build_scene_limited(
    ctx: &DataContext,
    did: u32,
    req: SceneRequest,
    job: &JobContext<'_>,
    limits: &SceneLimits,
) -> Result<SceneProduct, ExplorerError> {
    let started = Instant::now();
    let source = SceneSource::of(did).ok_or_else(|| {
        ExplorerError::NotApplicable(format!("{did:#010x} has no geometry to show"))
    })?;
    let mut gathered = Gathered::default();
    let mut choices = Vec::new();
    let mut choice = 0usize;
    let mut mats = Collector::new(ctx);
    let mut extra_parts: Vec<ScenePart> = Vec::new();
    let mut collision: Vec<CollisionOverlay> = Vec::new();
    let mut cell_record: Option<(Vec<u8>, u16)> = None;
    let specs: Vec<PartSpec> = match source {
        SceneSource::Mesh => vec![PartSpec {
            mesh: Did(did),
            caption: "mesh".into(),
            groups: None,
        }],
        SceneSource::Setup | SceneSource::Template | SceneSource::Entity => {
            setup_parts(ctx, Did(did), &mut gathered.chain, Some(&mut collision))?
        }
        SceneSource::Appearance => {
            let bytes = fetch(ctx, Did(did))?.ok_or(ExplorerError::NotFound(did))?;
            let record = AppearanceRecord::parse_expecting(&bytes, Did(did))?;
            choices = record
                .entries
                .iter()
                .take(MAX_CHOICES)
                .map(|e| {
                    let parts: usize = e.base_block().map_or(0, |b| b.parts.len());
                    format!(
                        "key {:#x} · {} blocks · {parts} parts",
                        e.key,
                        e.blocks.len()
                    )
                })
                .collect();
            if record.entries.len() > MAX_CHOICES {
                gathered.notes.push(format!(
                    "{} entries; only the first {MAX_CHOICES} can be chosen",
                    record.entries.len()
                ));
            }
            choice = usize::from(req.choice).min(choices.len().saturating_sub(1));
            let key =
                record.entries.get(choice).map(|e| e.key).ok_or_else(|| {
                    ExplorerError::NotApplicable("appearance has no entries".into())
                })?;
            gathered
                .chain
                .push(format!("appearance entry {key:#x}"), did);
            appearance_parts(ctx, &record, key, &mut gathered, job)?
        }
        SceneSource::Object => {
            let (links, set_did) = object_links(ctx, did)?;
            if links.is_empty() {
                return Err(ExplorerError::NotApplicable(format!(
                    "{did:#010x} names no geometry (no entity, template or setup DID property \
                     and no worn-appearance list)"
                )));
            }
            choices = links.iter().map(|l| l.caption().to_owned()).collect();
            choice = usize::from(req.choice).min(links.len() - 1);
            gathered.chain.push("object", did);
            gathered.chain.push("property set", set_did);
            match &links[choice] {
                ObjectLink::Entity { did: start, .. } => {
                    setup_parts(ctx, *start, &mut gathered.chain, Some(&mut collision))?
                }
                ObjectLink::Worn {
                    appearance, key, ..
                } => {
                    let bytes =
                        fetch(ctx, *appearance)?.ok_or(ExplorerError::NotFound(appearance.0))?;
                    let record = AppearanceRecord::parse_expecting(&bytes, *appearance)?;
                    gathered
                        .chain
                        .push(format!("appearance entry {key:#x}"), appearance.0);
                    appearance_parts(ctx, &record, *key, &mut gathered, job)?
                }
            }
        }
        SceneSource::Surface => {
            let m = mats.surface_material(Did(did), None);
            extra_parts.push(sphere_part(m, "preview sphere"));
            gathered
                .notes
                .push("a preview sphere drawn with this surface record's material".into());
            Vec::new()
        }
        SceneSource::Figure => {
            return Err(ExplorerError::NotApplicable(
                "characters are built by the character view".into(),
            ));
        }
        SceneSource::CollisionMesh => {
            let (located, bytes) = ctx.set.read_with_entry(did)?;
            let mesh = tessera_formats::CollisionMesh::parse(&bytes, Some(located.entry.stamp))?;
            let t = tessera_physics::geometry::ShapeTemplate::from_collision_mesh(&mesh)?;
            collision.extend(template_overlays(
                &t,
                &format!("collision mesh {}", Did(did)),
            ));
            gathered.notes.push(
                "a stand-alone collision mesh in its setup's frame (collision.md §4), drawn \
                 as a collision overlay; collision tests treat its triangles as two-sided"
                    .into(),
            );
            Vec::new()
        }
        SceneSource::Cell => {
            let (located, bytes) = ctx.set.read_with_entry(did)?;
            let registry = ctx.game.registry()?;
            let cell = tessera_formats::CellGeometry::parse_expecting(
                &bytes,
                Did(did),
                Some(located.entry.stamp),
                &**registry,
            )?;
            collision.extend(cell_overlays(&cell));
            gathered.notes.push(
                "a cell in its own frame: the embedded render mesh with the physics mesh, \
                 portals, wall polygons and hull vertices as overlays (collision.md §5)"
                    .into(),
            );
            cell_record = Some((cell.render_mesh_record(), located.entry.stamp));
            Vec::new()
        }
        SceneSource::Landblock
            if crate::gameplay::is_landblock_info(did) && usize::from(req.choice) > 0 =>
        {
            // A decal choice of a landblock-info record (decals.md §9.3).
            let decals = crate::decal_preview::landblock_decals(ctx, did)?;
            choices = std::iter::once("landblock: terrain and collision".to_owned())
                .chain(
                    decals
                        .iter()
                        .enumerate()
                        .take(crate::decal_preview::MAX_DECAL_CHOICES)
                        .map(|(i, d)| d.caption(i)),
                )
                .collect();
            choice = usize::from(req.choice).min(choices.len().saturating_sub(1));
            if let Some(d) = choice.checked_sub(1).and_then(|i| decals.get(i)) {
                let (part, overlays, notes) = crate::decal_preview::preview(d, &mut mats);
                extra_parts.extend(part);
                collision.extend(overlays);
                gathered.notes.extend(notes);
                if let Some(o) = d.object {
                    gathered.chain.push("marker object", o.0);
                }
                gathered.chain.push("surface record", d.params.material.0);
            }
            Vec::new()
        }
        SceneSource::Landblock => {
            let key = crate::world::landblock_of(did).ok_or_else(|| {
                ExplorerError::NotApplicable(format!("{did:#010x} is not a landblock record"))
            })?;
            if crate::gameplay::is_landblock_info(did) {
                let decals = crate::decal_preview::landblock_decals(ctx, did)?;
                if !decals.is_empty() {
                    choices = std::iter::once("landblock: terrain and collision".to_owned())
                        .chain(
                            decals
                                .iter()
                                .enumerate()
                                .take(crate::decal_preview::MAX_DECAL_CHOICES)
                                .map(|(i, d)| d.caption(i)),
                        )
                        .collect();
                    if decals.len() > crate::decal_preview::MAX_DECAL_CHOICES {
                        gathered.notes.push(format!(
                            "only the first {} decals can be chosen",
                            crate::decal_preview::MAX_DECAL_CHOICES
                        ));
                    }
                    gathered.notes.push(format!(
                        "{} Decal SEDs: choose one in the choice list for its preview \
                         (decals.md §9.3)",
                        decals.len()
                    ));
                }
            }
            let bundle = ctx.world.bundle(key, &|| job.is_cancelled())?;
            gathered.chain.push("landblock data", key.data_did().0);
            collision.extend(landblock_overlays(&bundle));
            extra_parts.push(terrain_part(&bundle));
            gathered.notes.push(
                "the landblock frame (x, y from the south-west corner, z absolute): terrain, \
                 and the collision set the walker uses as overlays (collision.md §12.1); \
                 placed objects are drawn only as their collision"
                    .into(),
            );
            Vec::new()
        }
        SceneSource::Material => {
            let rec = ctx
                .materials
                .material(Did(did))
                .map_err(ExplorerError::from)?;
            let plan = plan_bare_material(&rec);
            let mut view = MaterialView::default();
            view.links.push(("material".into(), did));
            for e in &rec.entries {
                let (value, color, link) = param_value_text(&e.value);
                view.params.push(ParamRow {
                    id: e.id,
                    name: param_name(e.id),
                    value,
                    color,
                    link,
                    origin: "this material".into(),
                });
            }
            for e in &rec.entries {
                if let Some(list) = e.value.as_texture() {
                    let thumb = mats.thumb(list, false);
                    view.slots.push(SlotRow {
                        id: e.id,
                        kind: SlotKind::of(e.id).name().to_owned(),
                        role: "—".into(),
                        packing: "— (no shader)".into(),
                        texture: Some(list.0),
                        sampler: "—".into(),
                        origin: "this material".into(),
                        used_as: if e.id == ids::DIFFUSE {
                            "base colour".into()
                        } else {
                            "—".into()
                        },
                        thumb,
                    });
                }
            }
            view.notes.push(
                "a material alone has no shader record: entries are shown as stored and the \
                 preview uses its diffuse texture (slot 3000) only"
                    .into(),
            );
            let m = mats.add(format!("material {}", Did(did)), plan, view, false);
            extra_parts.push(sphere_part(m, "preview sphere"));
            Vec::new()
        }
    };
    let Gathered {
        chain,
        mut notes,
        dyes,
    } = gathered;
    if chain.omitted > 0 {
        notes.push(format!(
            "{} further chain links are not listed (at most {MAX_CHAIN})",
            chain.omitted
        ));
    }
    let chain = chain.links;
    if specs.len() > MAX_PARTS {
        notes.push(format!(
            "{} parts; only the first {MAX_PARTS} are drawn",
            specs.len()
        ));
    }
    // Each distinct mesh is built once, within the scene's geometry budget; parts that
    // repeat it share the prepared streams (and the uploaded mesh).
    let mut built: HashMap<Did, ScenePart> = HashMap::new();
    let mut budget = GeometryBudget::new(limits);
    let mut parts = Vec::new();
    for spec in specs.iter().take(MAX_PARTS) {
        if job.is_cancelled() {
            return Err(ExplorerError::Cancelled);
        }
        let mesh = built
            .entry(spec.mesh)
            .or_insert_with(|| build_mesh_part(ctx, spec.mesh, req, &mut budget))
            .clone();
        parts.push(place_part(mesh, spec, &mut mats));
    }
    if budget.refused > 0 {
        notes.push(format!(
            "{} part meshes left out for the scene's geometry budget ({} vertices, {} \
             indices, {} MiB of streams)",
            budget.refused,
            limits.max_vertices,
            limits.max_indices,
            limits.max_stream_bytes >> 20
        ));
    }
    if let Some(record) = cell_record {
        let spec = PartSpec {
            mesh: Did(did),
            caption: "render mesh".into(),
            groups: None,
        };
        let part = build_part_from(ctx, Did(did), Some(record), req, &mut budget, None, None);
        parts.push(place_part(part, &spec, &mut mats));
    }
    parts.extend(extra_parts);
    finish_product(
        ProductParts {
            did,
            source,
            request: req,
            chain,
            choices,
            choice,
            parts,
            notes,
            dyes,
            started,
            collision,
        },
        mats,
        job,
    )
}

/// What [`finish_product`] turns into a [`SceneProduct`].
pub(crate) struct ProductParts {
    pub(crate) did: u32,
    pub(crate) source: SceneSource,
    pub(crate) request: SceneRequest,
    pub(crate) chain: Vec<ChainLink>,
    pub(crate) choices: Vec<String>,
    pub(crate) choice: usize,
    pub(crate) parts: Vec<ScenePart>,
    pub(crate) notes: Vec<String>,
    pub(crate) dyes: Vec<DyeBlock>,
    pub(crate) started: Instant,
    pub(crate) collision: Vec<CollisionOverlay>,
}

/// Finishes a scene: decodes the materials' textures and thumbnails, computes bounds,
/// level labels and totals, and bounds the notes.
///
/// # Errors
///
/// [`ExplorerError::Cancelled`] when the job is cancelled while textures load.
pub(crate) fn finish_product(
    p: ProductParts,
    mats: Collector<'_>,
    job: &JobContext<'_>,
) -> Result<SceneProduct, ExplorerError> {
    let ProductParts {
        did,
        source,
        request: req,
        chain,
        choices,
        choice,
        parts,
        mut notes,
        dyes,
        started,
        collision,
    } = p;
    let Finished {
        materials,
        textures,
        thumbs,
        notes: mut tex_notes,
    } = mats.finish(job)?;
    notes.append(&mut tex_notes);

    // Bounds over the drawn parts, each in scene space (its transform applied).
    let mut aabb: Option<([f32; 3], [f32; 3])> = None;
    for p in &parts {
        // Held parts sit at the wearer's origin until posed: leave them out.
        if p.prepared.is_none() || p.attach.is_some() {
            continue;
        }
        for s in &p.submeshes {
            if let Some((lo, hi)) = s
                .aabb
                .and_then(|(lo, hi)| transform_aabb(&p.transform, lo, hi))
            {
                let b = aabb.get_or_insert((lo, hi));
                for i in 0..3 {
                    b.0[i] = b.0[i].min(lo[i]);
                    b.1[i] = b.1[i].max(hi[i]);
                }
            }
        }
    }
    // Collision overlays count too (a 0x66 record has nothing else), except those that
    // would not frame the scene ([`OverlayKind::frames`]: liquid boxes and ceilings), unless
    // nothing else has bounds.
    aabb = frame_overlays(aabb, &collision);
    let sphere = aabb.map_or(([0.0; 3], 1.0), |(lo, hi)| {
        let c = [0, 1, 2].map(|i| (lo[i] + hi[i]) * 0.5);
        let r = (0..3)
            .map(|i| (hi[i] - lo[i]) * 0.5)
            .map(|h| h * h)
            .sum::<f32>()
            .sqrt();
        (c, if r.is_finite() && r > 0.0 { r } else { 1.0 })
    });
    let max_levels = parts.iter().map(|p| p.levels.len()).max().unwrap_or(0);
    let level_labels = (0..max_levels)
        .map(|i| {
            let kinds: Vec<&str> = parts
                .iter()
                .filter_map(|p| p.levels.get(i).map(|l| l.kind.as_str()))
                .collect();
            let tris: usize = parts
                .iter()
                .filter_map(|p| p.levels.get(i).map(|l| l.triangles))
                .sum();
            let kind = if kinds.iter().all(|k| *k == kinds[0]) {
                kinds.first().copied().unwrap_or("?")
            } else {
                "mixed"
            };
            format!("level {i} · {kind} · {tris} triangles")
        })
        .collect();
    if parts
        .iter()
        .any(|p| !p.skinned && p.submeshes.iter().any(|s| s.skinned))
    {
        notes.push("skinned submeshes are drawn in bind pose (no skeleton yet)".into());
    }
    if parts.len() > 1 && source != SceneSource::Figure {
        notes.push(
            "parts share the setup's frame; the part list has no transforms (setup.md §7)".into(),
        );
    }
    let vertices = parts.iter().map(|p| p.vertices).sum();
    let triangles = parts.iter().map(|p| p.triangles).sum();
    if notes.len() > MAX_NOTES {
        let more = notes.len() - (MAX_NOTES - 1);
        notes.truncate(MAX_NOTES - 1);
        notes.push(format!("… {more} more notes"));
    }
    Ok(SceneProduct {
        did,
        source,
        request: req,
        chain,
        choices,
        choice,
        level_labels,
        parts,
        materials,
        textures,
        thumbs,
        notes,
        aabb,
        sphere,
        vertices,
        triangles,
        dyes,
        build_ms: started.elapsed().as_millis() as u64,
        collision,
    })
}

/// `aabb` grown by the bounds of the overlays that frame a scene
/// ([`crate::collision_overlay::OverlayKind::frames`]); when that leaves no bounds, by
/// every overlay's.
fn frame_overlays(
    mut aabb: Option<([f32; 3], [f32; 3])>,
    collision: &[CollisionOverlay],
) -> Option<([f32; 3], [f32; 3])> {
    let grow = |aabb: &mut Option<([f32; 3], [f32; 3])>, o: &CollisionOverlay| {
        if let Some((lo, hi)) = o.aabb {
            let b = aabb.get_or_insert((lo, hi));
            for i in 0..3 {
                b.0[i] = b.0[i].min(lo[i]);
                b.1[i] = b.1[i].max(hi[i]);
            }
        }
    };
    for o in collision.iter().filter(|o| o.kind.frames()) {
        grow(&mut aabb, o);
    }
    if aabb.is_none() {
        for o in collision {
            grow(&mut aabb, o);
        }
    }
    aabb
}

/// Synthetic scenes for unit tests of the viewport (no archive involved).
#[cfg(test)]
pub(crate) mod tests_support {
    use super::*;
    use tessera_mesh::synth::MeshSpec;

    /// Two synthetic parts side by side, one textured and dyeable material, one plain.
    pub(crate) fn two_part_scene() -> SceneProduct {
        let mut parts = Vec::new();
        for (i, format) in [0x3000_0152u32, 0x3002_0112].into_iter().enumerate() {
            let mesh = MeshSpec::single(format, 24).build().unwrap();
            let geo = mesh.level_geometry(0, &GeometryLimits::default()).unwrap();
            let mut data = level_mesh_data(&geo, &ConvertOptions::default(), format!("p{i}"));
            for p in &mut data.positions {
                p[0] += i as f32 * 3.0;
            }
            let prepared = data.prepare(&MeshConfig::default()).unwrap();
            let sphere = prepared.info().sphere;
            parts.push(ScenePart {
                mesh: 0x0600_0001 + i as u32,
                caption: format!("part {i}"),
                level: 0,
                levels: Vec::new(),
                flags: 1,
                transform: IDENTITY,
                prepared: Some(Arc::new(prepared)),
                error: None,
                submeshes: vec![SceneSubmesh {
                    draw: 0,
                    submesh: 0,
                    surface: None,
                    format,
                    vertices: 24,
                    triangles: 8,
                    skinned: false,
                    aabb: Some(([0.0; 3], [1.0; 3])),
                    material: Some(i),
                }],
                issues: Vec::new(),
                vertices: geo.vertex_count(),
                triangles: geo.triangle_count(),
                sphere: Some((sphere.center.to_array(), sphere.radius)),
                skinned: false,
                attach: None,
                figure: 0,
            });
        }
        let checker: Vec<u8> = (0..16 * 16)
            .flat_map(|i| {
                let on = (i % 16 / 4 + i / 64) % 2 == 0;
                [if on { 230 } else { 40 }, 120, 60, if on { 255 } else { 0 }]
            })
            .collect();
        let texture = |label: &str, linear| {
            Arc::new(SceneTexture {
                label: label.into(),
                width: 16,
                height: 16,
                rgba: checker.clone(),
                linear,
                clamp: false,
                nearest: true,
            })
        };
        let textured = SceneMaterial {
            title: "textured".into(),
            gpu: GpuMaterial {
                base: Some(0),
                dye: [
                    Some(GpuDye {
                        mask: 1,
                        color: [0.2, 0.4, 1.0, 1.0],
                    }),
                    None,
                ],
                alpha: AlphaKind::Mask(0.5),
                two_sided: true,
                specular: Some(([0.5; 3], 30.0)),
                gloss: Some(1),
                ..GpuMaterial::default()
            },
            dyeable: true,
            view: MaterialView::default(),
            error: None,
        };
        let plain = SceneMaterial {
            title: "plain".into(),
            gpu: GpuMaterial {
                alpha: AlphaKind::Blend,
                unlit: true,
                ..GpuMaterial::default()
            },
            dyeable: false,
            view: MaterialView::default(),
            error: None,
        };
        SceneProduct {
            did: 0x0400_0001,
            source: SceneSource::Setup,
            request: SceneRequest::default(),
            chain: Vec::new(),
            choices: Vec::new(),
            choice: 0,
            level_labels: vec!["level 0".into()],
            parts,
            materials: vec![textured, plain],
            textures: vec![texture("base", false), texture("mask", true)],
            thumbs: Vec::new(),
            notes: Vec::new(),
            aabb: Some(([-1.0, -1.0, -1.0], [4.0, 1.0, 1.0])),
            sphere: ([1.5, 0.0, 0.0], 3.0),
            vertices: 48,
            triangles: 16,
            dyes: Vec::new(),
            build_ms: 0,
            collision: Vec::new(),
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn request_variant_round_trip() {
        for lod in [0u8, 3, 255] {
            for choice in [0u8, 7, 255] {
                for vertex_colors in [false, true] {
                    let r = SceneRequest {
                        lod,
                        choice,
                        vertex_colors,
                    };
                    assert_eq!(SceneRequest::from_variant(r.variant()), r);
                }
            }
        }
        assert_eq!(SceneRequest::default().variant(), 0);
    }

    #[test]
    fn sources_by_prefix() {
        assert_eq!(SceneSource::of(0x0600_0001), Some(SceneSource::Mesh));
        assert_eq!(SceneSource::of(0x0400_0001), Some(SceneSource::Setup));
        assert_eq!(SceneSource::of(0x1F00_0001), Some(SceneSource::Template));
        assert_eq!(SceneSource::of(0x4700_0001), Some(SceneSource::Entity));
        assert_eq!(SceneSource::of(0x2000_0001), Some(SceneSource::Appearance));
        assert_eq!(SceneSource::of(0x7900_0001), Some(SceneSource::Object));
        assert_eq!(SceneSource::of(0x3100_0001), Some(SceneSource::Surface));
        assert_eq!(SceneSource::of(0x3000_0001), Some(SceneSource::Material));
        assert_eq!(SceneSource::of(0x4100_0001), None);
        assert_eq!(SceneSource::of(0x0100_0001), Some(SceneSource::Cell));
        assert_eq!(
            SceneSource::of(0x6600_0001),
            Some(SceneSource::CollisionMesh)
        );
        assert_eq!(SceneSource::of(0x8001_3040), Some(SceneSource::Landblock));
        assert_eq!(SceneSource::of(0x0200_0001), None);
    }

    #[test]
    fn parameter_names() {
        assert_eq!(param_name(3000), "diffuse");
        assert_eq!(param_name(1043), "alpha-test threshold");
        assert_eq!(param_name(2003), "dye colour");
        assert_eq!(param_name(1234), "");
    }

    #[test]
    fn sphere_preview_prepares() {
        let p = sphere_part(0, "x");
        assert!(p.prepared.is_some(), "{:?}", p.error);
        assert_eq!(p.submeshes.len(), 1);
        assert_eq!(p.submeshes[0].material, Some(0));
    }

    #[test]
    fn sphere_poles_are_on_z() {
        // Regression: the preview sphere kept the renderer's Y-up poles, so the default
        // camera (on the +Y side) looked into a pole.
        let d = sphere_data();
        let uvs = d.uvs.as_ref().unwrap();
        let normals = d.normals.as_ref().unwrap();
        let mut top = 0;
        for (i, p) in d.positions.iter().enumerate() {
            assert!(p[2] > -1e-4 && p[2] < 1.0 + 1e-4, "{p:?}");
            if uvs[i][1] == 0.0 {
                top += 1;
                assert!((p[2] - 1.0).abs() < 1e-4, "v = 0 at {p:?}");
                assert!(normals[i][2] > 0.99);
            }
            if uvs[i][1] == 1.0 {
                assert!(p[2].abs() < 1e-4, "v = 1 at {p:?}");
            }
        }
        assert!(top > 0);
        // The u = 0.5 meridian faces +Y (the default camera side), so the texture is
        // seen upright and unmirrored from there.
        let mid = (0..d.positions.len())
            .find(|&i| (uvs[i][0] - 0.5).abs() < 1e-6 && (uvs[i][1] - 0.5).abs() < 1e-6)
            .unwrap();
        assert!(d.positions[mid][1] > 0.49, "{:?}", d.positions[mid]);
        assert!(d.tangents.is_some());
        assert!(d.validate(&MeshConfig::default()).is_ok());
    }

    #[test]
    fn default_request_is_variant_zero_without_vertex_colours() {
        let r = SceneRequest::default();
        assert!(!r.vertex_colors);
        assert_eq!(r.variant(), 0);
        let on = SceneRequest {
            vertex_colors: true,
            ..r
        };
        assert_eq!(on.variant(), 1 << 16);
    }

    #[test]
    fn geometry_budget() {
        let mut b = GeometryBudget::new(&SceneLimits::default());
        assert!(!b.binding() && !b.exhausted());
        assert_eq!(b.limits(), GeometryLimits::default());
        b.take(MAX_SCENE_VERTICES - 10, 5, 7);
        assert!(b.binding());
        assert_eq!(b.limits().max_vertices, 10);
        b.take(100, 0, 0);
        assert!(b.exhausted());
        assert_eq!(b.limits().max_vertices, 0);
    }

    #[test]
    fn chain_links_are_unique_and_capped() {
        let mut c = Chain::default();
        for i in 0..1000u32 {
            c.push("x", 0x0400_0000 + i % 2);
        }
        assert_eq!(c.links.len(), 2);
        for i in 0..1000u32 {
            c.push("y", 0x0400_1000 + i);
        }
        assert_eq!(c.links.len(), MAX_CHAIN);
        assert_eq!(c.omitted, 1000 - (MAX_CHAIN - 2));
    }

    // ---- plan_material over hand-built resolved materials ----

    use tessera_material::interface::Declaration;
    use tessera_material::records::{BlendFactor, BlendOp, FloatCurve};
    use tessera_material::resolve::{AlphaTest, BlendState, DyeChannel, DyeSetup, FeatureFlags};
    use tessera_material::vocab::ChannelPacking;
    use tessera_material::{ParameterSet, ShaderInterface, TextureSlot};

    fn opaque_features() -> FeatureFlags {
        FeatureFlags {
            lit: true,
            alpha_test: None,
            opacity_slot: None,
            blend: BlendState {
                mode: BlendMode::Opaque,
                src: BlendFactor::One,
                dst: BlendFactor::Zero,
                op: BlendOp::Add,
                depth_write: true,
                opacity_slot: None,
            },
            two_sided: false,
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
        }
    }

    fn slot(id: u32, texture: u32, role: &str) -> TextureSlot {
        TextureSlot {
            id,
            kind: SlotKind::of(id),
            texture: Some(Did(texture)),
            role: Some(role.into()),
            packing: ChannelPacking::from_role(Some(role)),
            sampler: None,
            origin: ParamOrigin::Default,
        }
    }

    /// A resolved material whose effective parameters are the shader defaults `params`.
    fn resolved(
        params: &[(u32, ParamValue)],
        slots: Vec<TextureSlot>,
        features: FeatureFlags,
        dye: Option<DyeSetup>,
    ) -> ResolvedMaterial {
        let shader = Arc::new(ShaderInterface {
            did: Did(0x2B00_0001),
            declarations: params
                .iter()
                .map(|(id, v)| Declaration {
                    id: *id,
                    default: v.clone(),
                    binding_count: 1,
                    flag: 0,
                })
                .collect(),
            slots: Vec::new(),
            permutation_count: 1,
            lit: features.lit,
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
            slots,
            features,
            dye,
        }
    }

    const DIFFUSE_TEX: u32 = 0x4100_0001;
    const OPACITY_TEX: u32 = 0x4100_0002;
    const NORMAL_TEX: u32 = 0x4100_0003;
    const DYE_TEX: u32 = 0x4100_0004;

    #[test]
    fn plan_opacity_from_the_diffuse_alpha() {
        let mut f = opaque_features();
        f.blend.mode = BlendMode::Cutout;
        f.alpha_test = Some(AlphaTest {
            threshold: 0.8,
            source_slot: ids::DIFFUSE,
        });
        let m = resolved(
            &[],
            vec![slot(ids::DIFFUSE, DIFFUSE_TEX, "ColorAndOpacity")],
            f,
            None,
        );
        let plan = plan_material(&m);
        assert_eq!(plan.material.alpha, AlphaKind::Mask(0.8));
        assert_eq!(
            plan.base,
            Some(TexKey::Base {
                diffuse: Some(Did(DIFFUSE_TEX)),
                alpha: AlphaPlan::Own,
                clamp: false,
                nearest: false
            })
        );
        assert_eq!(used_as(&m, &plan, ids::DIFFUSE), "base colour, opacity");
        // Cutout without an alpha test draws opaque (and the base alpha is not used).
        let mut f = opaque_features();
        f.blend.mode = BlendMode::Cutout;
        let m = resolved(&[], vec![slot(ids::DIFFUSE, DIFFUSE_TEX, "Color")], f, None);
        let plan = plan_material(&m);
        assert_eq!(plan.material.alpha, AlphaKind::Opaque);
        assert!(matches!(
            plan.base,
            Some(TexKey::Base {
                alpha: AlphaPlan::Opaque,
                ..
            })
        ));
    }

    #[test]
    fn plan_opacity_from_a_separate_slot_when_blended() {
        let mut f = opaque_features();
        f.blend.mode = BlendMode::AlphaBlend;
        f.blend.opacity_slot = Some(ids::OPACITY);
        f.opacity_slot = Some(ids::OPACITY);
        let m = resolved(
            &[],
            vec![
                slot(ids::DIFFUSE, DIFFUSE_TEX, "Color"),
                slot(ids::OPACITY, OPACITY_TEX, "Opacity"),
            ],
            f,
            None,
        );
        let plan = plan_material(&m);
        assert_eq!(plan.material.alpha, AlphaKind::Blend);
        assert!(matches!(
            plan.base,
            Some(TexKey::Base { alpha: AlphaPlan::From(d), .. }) if d == Did(OPACITY_TEX)
        ));
        assert_eq!(used_as(&m, &plan, ids::OPACITY), "opacity");
        // A blend whose opacity slot has no texture is opaque in alpha.
        let mut f2 = f;
        f2.blend.opacity_slot = Some(ids::GLOSS);
        let m = resolved(
            &[],
            vec![slot(ids::DIFFUSE, DIFFUSE_TEX, "Color")],
            f2,
            None,
        );
        assert!(matches!(
            plan_material(&m).base,
            Some(TexKey::Base {
                alpha: AlphaPlan::Opaque,
                ..
            })
        ));
    }

    #[test]
    fn plan_specular_power_zero_and_linear_colour() {
        let mut f = opaque_features();
        f.specular = true;
        let grey = ParamValue::Color([0.5, 0.5, 0.5, 1.0]);
        let zero = resolved(
            &[
                (
                    ids::SPECULAR_POWER,
                    ParamValue::Float(FloatCurve::constant(0.0)),
                ),
                (ids::SPECULAR_COLOR, grey.clone()),
            ],
            vec![slot(ids::DIFFUSE, DIFFUSE_TEX, "ColorAndGloss")],
            f,
            None,
        );
        let plan = plan_material(&zero);
        assert!(plan.material.specular.is_none());
        assert!(plan.gloss.is_none(), "no gloss mask without a highlight");
        let lit = resolved(
            &[
                (
                    ids::SPECULAR_POWER,
                    ParamValue::Float(FloatCurve::constant(25.0)),
                ),
                (ids::SPECULAR_COLOR, grey),
            ],
            vec![slot(ids::DIFFUSE, DIFFUSE_TEX, "ColorAndGloss")],
            f,
            None,
        );
        let plan = plan_material(&lit);
        let (color, power) = plan.material.specular.unwrap();
        assert_eq!(power, 25.0);
        // Regression: 2001 was passed as stored while 2000 and 2003 were converted
        // from sRGB; 0.5 sRGB is about 0.214 linear.
        assert!((color[0] - srgb_unit_to_linear(0.5)).abs() < 1e-6);
        assert!(color[0] < 0.22);
        assert_eq!(
            plan.gloss,
            Some(TexKey::Mask {
                list: Did(DIFFUSE_TEX),
                invert: false
            })
        );
    }

    #[test]
    fn plan_dye_channels() {
        let mut f = opaque_features();
        f.dyeable = true;
        let ch = |slot, texture: Option<u32>, color| DyeChannel {
            mask: DyeMask {
                slot,
                texture: texture.map(Did),
            },
            color,
        };
        let red = [1.0, 0.0, 0.0, 1.0];
        let blue = [0.0, 0.0, 1.0, 1.0];
        let both = resolved(
            &[],
            vec![
                slot(ids::DIFFUSE, DIFFUSE_TEX, "ColorAndDye"),
                slot(ids::DYE_MASK, DYE_TEX, "DyeAlpha"),
            ],
            f,
            Some(DyeSetup {
                primary: ch(ids::DIFFUSE, Some(DIFFUSE_TEX), red),
                secondary: Some(ch(ids::DYE_MASK, Some(DYE_TEX), blue)),
            }),
        );
        let plan = plan_material(&both);
        assert_eq!(
            plan.dye[0],
            Some((
                TexKey::Mask {
                    list: Did(DIFFUSE_TEX),
                    invert: true
                },
                red
            ))
        );
        assert_eq!(
            plan.dye[1],
            Some((
                TexKey::Mask {
                    list: Did(DYE_TEX),
                    invert: true
                },
                blue
            ))
        );
        assert_eq!(used_as(&both, &plan, ids::DYE_MASK), "second dye mask");
        assert!(used_as(&both, &plan, ids::DIFFUSE).contains("dye mask"));
        // A dye whose mask slot has no texture plans no dye.
        let none = resolved(
            &[],
            vec![slot(ids::DIFFUSE, DIFFUSE_TEX, "ColorAndDye")],
            f,
            Some(DyeSetup {
                primary: ch(ids::DYE_MASK, None, red),
                secondary: None,
            }),
        );
        assert_eq!(plan_material(&none).dye, [None, None]);
        // Not dyeable: no dye even with a setup.
        let mut plain = none.clone();
        plain.features.dyeable = false;
        assert_eq!(plan_material(&plain).dye, [None, None]);
        // The inspector says that one channel is drawn at a time.
        assert!(
            view_of(&both, None)
                .notes
                .iter()
                .any(|n| n.contains("one at a time"))
        );
    }

    #[test]
    fn plan_diffuse_colour_and_normal_map() {
        let mut f = opaque_features();
        f.normal_mapped = true;
        let m = resolved(
            &[(ids::DIFFUSE_COLOR, ParamValue::Color([0.0, 0.0, 0.0, 0.0]))],
            vec![
                slot(ids::DIFFUSE, DIFFUSE_TEX, "Color"),
                slot(ids::NORMAL, NORMAL_TEX, "TSNormal"),
            ],
            f,
            None,
        );
        let plan = plan_material(&m);
        assert_eq!(plan.normal, Some(TexKey::Normal(Did(NORMAL_TEX))));
        assert_eq!(plan.material.base_color, [0.0, 0.0, 0.0, 1.0]);
        // A non-white 2000 is pointed out (it is applied only on request).
        assert!(
            view_of(&m, None)
                .notes
                .iter()
                .any(|n| n.contains("diffuse colour 2000"))
        );
    }

    // ---- textures ----

    fn solid(w: u32, h: u32, a: u8) -> Arc<RgbaImage> {
        Arc::new(RgbaImage {
            width: w,
            height: h,
            pixels: [10, 20, 30, a].repeat((w * h) as usize),
        })
    }

    #[test]
    fn textures_dedupe_budget_and_opacity_fallback() {
        let base = TexKey::Base {
            diffuse: Some(Did(DIFFUSE_TEX)),
            alpha: AlphaPlan::From(Did(0x4100_00FF)),
            clamp: false,
            nearest: false,
        };
        let keys = vec![
            base,
            TexKey::Normal(Did(NORMAL_TEX)),
            TexKey::Mask {
                list: Did(DYE_TEX),
                invert: true,
            },
            TexKey::Normal(Did(0x4100_0010)),
        ];
        let load = |d: Did| -> Result<Arc<RgbaImage>, String> {
            match d.0 {
                0x4100_00FF => Err(format!("{d}: broken")),
                0x4100_0010 => Ok(solid(64, 64, 255)),
                _ => Ok(solid(8, 8, 100)),
            }
        };
        // Room for the three small textures only.
        let made = make_textures(&keys, 3 * 8 * 8 * 4 + 10, &load, &|| false).unwrap();
        assert_eq!(made.textures.len(), 3);
        // Regression: a failed opacity map dropped the base texture too; now it is
        // drawn opaque.
        let b = &made.textures[made.slot_of[&base]];
        assert!(b.rgba.chunks(4).all(|p| p[3] == 255));
        assert!(made.notes.iter().any(|n| n.contains("opacity map")));
        // The texture left out for the budget is named.
        assert!(!made.slot_of.contains_key(&TexKey::Normal(Did(0x4100_0010))));
        assert!(
            made.notes
                .iter()
                .any(|n| n.contains("texture budget") && n.contains("normal 0x41000010")),
            "{:?}",
            made.notes
        );
        // Dye masks move the inverted alpha into R.
        let mask = &made.textures[made.slot_of[&keys[2]]];
        assert_eq!(&mask.rgba[..4], &[155, 155, 155, 255]);
        // Cancellation stops the work.
        assert!(matches!(
            make_textures(&keys, usize::MAX, &load, &|| true),
            Err(ExplorerError::Cancelled)
        ));
    }

    #[test]
    fn textures_beyond_the_count_cap_are_noted() {
        let keys: Vec<TexKey> = (0..MAX_SCENE_TEXTURES as u32 + 5)
            .map(|i| TexKey::Normal(Did(0x4100_1000 + i)))
            .collect();
        let made = make_textures(&keys, usize::MAX, &|_| Ok(solid(1, 1, 255)), &|| false).unwrap();
        assert_eq!(made.textures.len(), MAX_SCENE_TEXTURES);
        assert!(made.notes[0].contains("only the first"));
        assert_eq!(texture_cap(1), 1024);
        assert_eq!(texture_cap(50), 512);
        assert_eq!(texture_cap(10_000), 256);
    }

    #[test]
    fn thumbnails_are_opaque_rgba() {
        let src = RgbaImage {
            width: 2,
            height: 1,
            pixels: vec![1, 2, 3, 40, 5, 6, 7, 80],
        };
        assert_eq!(
            thumb_image(&src, false).pixels,
            vec![1, 2, 3, 255, 5, 6, 7, 255]
        );
        assert_eq!(
            thumb_image(&src, true).pixels,
            vec![40, 40, 40, 255, 80, 80, 80, 255]
        );
    }

    #[test]
    fn landblock_scenes_are_framed_without_liquid_boxes_and_ceilings() {
        // Regression: the deep monitor box of a wet landblock (1,600 units below the
        // water) and the ceiling triangles at height 0 framed the 3D view, so the terrain
        // filled a small part of it.
        use crate::collision_overlay::{OverlayKind, line_overlay, triangle_overlay};
        let terrain = triangle_overlay(
            OverlayKind::Terrain,
            "t",
            [[[0.0, 0.0, 10.0], [160.0, 0.0, 12.0], [160.0, 160.0, 20.0]]],
        );
        let deep = line_overlay(
            OverlayKind::Liquids,
            "l",
            [([0.0, 0.0, -1_600.0], [160.0, 160.0, 15.0])],
        );
        let ceiling = triangle_overlay(
            OverlayKind::Ceiling,
            "c",
            [[[-500.0, 0.0, 0.0], [160.0, 0.0, 0.0], [160.0, 900.0, 0.0]]],
        );
        let all = [terrain, deep, ceiling];
        let b = frame_overlays(None, &all).expect("bounds");
        assert_eq!(b, ([0.0, 0.0, 10.0], [160.0, 160.0, 20.0]));
        // Parts count as before.
        let b = frame_overlays(Some(([5.0, 5.0, 30.0], [6.0, 6.0, 31.0])), &all).expect("b");
        assert_eq!(b, ([0.0, 0.0, 10.0], [160.0, 160.0, 31.0]));
        // Liquid boxes alone still frame (nothing else has bounds).
        let b = frame_overlays(None, &all[1..2]).expect("liquids");
        assert_eq!(b.0[2], -1_600.0);
    }
}
