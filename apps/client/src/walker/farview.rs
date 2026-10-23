// SPDX-License-Identifier: GPL-3.0-or-later
//! The walker's far view (`docs/specs/far-view.md`, `--far-view on`): distant-landscape
//! sectors around the viewer drawn beyond the walker's full-detail terrain, with their
//! colour lists and the detail texture, the sectors' landscape water outside the
//! full-detail landblocks, and the distant sprites from the imposter atlases.
//!
//! - **Near field** (§2, §3.2, Tessera Decisions FD9 and FD15): by default the original's
//!   3 × 3 landblocks around the viewer's ([`NearFieldMode::Original`]; the host streams
//!   terrain within [`NEAR_FIELD_RING`]); with `--view-blocks` the walker's own full-detail
//!   ring ([`NearFieldMode::Ring`]), whose far neighbours stitch to it by FD11 and FD12. Only landblocks the terrain layer can draw (meshed, or
//!   deliberately out of the scene) enter the near field ([`near_field_of`]); the far
//!   batches cover the rest of the listed sectors, so an unmeshed near landblock is filled
//!   by the far mesh until its full-detail mesh arrives.
//! - **Hand-off** ([`FarHandOff`], Tessera Decision FD13): the terrain and water layers
//!   skip exactly the landblocks the far batches **on the GPU** cover, so while new
//!   batches build on the workers a landblock is drawn by the old far batch or by the full-detail terrain,
//!   never by both and never by neither (as long as the terrain still holds it). Terrain
//!   blocks next to the far field mesh at the finest level and ignore the far neighbour
//!   for their seam, so their edge has every sample the far stitch fans to (§5.2).
//! - **Work** (FD3): sector reads, imposter-table reads, batch builds and sprite placement
//!   run on the layer's own worker threads (`super::jobs::JobRunner`); results are swapped
//!   in per sector and uploaded within a per-frame cap ([`MAX_UPLOADS_PER_FRAME`]).
//! - **Textures** come from the object layer's texture store (colour lists, the detail
//!   texture of UNIQUEDB key 0x7A, the two imposter atlases), reference-counted per sector;
//!   the sector water's flow-map copies hold theirs while they are drawn (and
//!   `FLOW_COPY_GRACE_FRAMES` after).
//! - **Far entities** (§10): the distant set is walked every frame; an entry is created
//!   when its squared distance is below `view_distance_sq` × *s*, removed beyond it, and
//!   every entity is removed on a change of the viewer's instance. Each entry's object is
//!   resolved on the layer's workers (chain, parts, `Render_LODClass`,
//!   `Render_DepthBias_Type`; an object that cannot be read now is tried
//!   [`FAR_MAX_ATTEMPTS`] times, then left undrawn) and the entities are handed to the
//!   object layer per landblock ([`ObjectLayer::insert_far`]) once none of the landblock's
//!   entries is still being resolved, so a far block is rebuilt once per batch of changes,
//!   not once per resolved entry. The layer activates, fades and draws them by their own
//!   draw distances like every static object (`walker.md` §11.4). Where the layer holds the
//!   landblock's own objects too, it leaves each far entity to them within their distance
//!   cap and draws it beyond ([`super::objects::handed_to_main`]): §10's "exists through
//!   the main set" by distance, since the layer's static objects carry no instance IDs
//!   (Tessera choice, `far-view.md` §12 OPEN). Entries of type Weenie and setups that are
//!   skinned rigs are created but not drawn (the walker draws figures through its
//!   character layer, which does not take far entities; Tessera choice, `far-view.md`
//!   §12).
//! - **Templates** (§6.1, §6.2, §6.4): the active template follows
//!   [`FarViewSettings::template`]; its defaults (colour and detail repeats, the
//!   normal-mapped template's normal texture 3011) are read from the shader record. With
//!   the seam overlay on ([`FarViewSettings::seam_overlay`], §6.5) the hand-off carries the
//!   sector materials to the terrain layer, which draws its near chunks once more with them
//!   ([`SeamMaterial`]).
//! - **Sector water** (§8, `distant-landscape.md` §5.5): a sector whose `flow_map` is not 0
//!   draws its water with a copy of the water material whose slot 3075 is that map
//!   ([`super::water::WaterSurfaces::surface_with_flow`]).
//! - **Horizon fill** (§7.3): the mesh of UNIQUEDB key 0x95 is placed as §7.3 says and drawn
//!   through the instanced path, only when its horizontal radius is at least 1.25 × *E*, so
//!   it can never hide listed terrain (Tessera Decision under the §12 OPEN on the horizon
//!   mesh: placed from the eye's height; its depth state is not traced).
//! - **Draw order** (Tessera Decision FD14): the renderer draws the far view after the sky
//!   and before the water, so the far-draw fade blends with the sky.

use std::collections::{BTreeMap, HashMap, HashSet, VecDeque};
use std::sync::Arc;

use tessera_formats::{Did, ImposterTable, Record as _, SectorRecord};
use tessera_material::ShaderRecord;
use tessera_render::glam::{Mat4, Quat, Vec3};
use tessera_render::{
    DistantSprite, DrawDepthBias, FarFadeParams, FarTerrainData, FarTerrainDraw, FarTerrainMesh,
    Instance, InstanceBatch, Pass, Renderer, SpriteBatch, SpriteDraw, TextureId, WaterDraw,
    WaterQuad,
};
use tessera_world::far_view::{
    BuildJob, Cell, FarMesh, FarSprite, FarViewRecords, FarViewSettings, FarViewState, MeshBounds,
    NearField, SectorKey, SectorMeshes, SpriteJob, SpriteView, Viewer, horizon,
    read_imposter_table, read_sector, sector_water,
};
use tessera_world::far_view::{DistantChanges, DistantEntry, DistantGroups, Template};
use tessera_world::{
    DistanceSettings, LANDBLOCK_SIZE, LandblockKey, LodClasses, LodValues, WorldContext,
    WorldError, WorldPos,
};

use super::geometry::{TexRequest, VARIANT_OPAQUE, VARIANT_OWN_ALPHA};
use super::interiors::OUTDOORS;
use super::jobs::JobRunner;
use super::objects::{Instance as ObjectInstance, ObjectInfo, ObjectLayer, TexLookup};
use super::resolver::Resolver;
use super::terrain::TextureStore;
use super::view::Anchor;

/// Most batch and sprite uploads per frame.
pub const MAX_UPLOADS_PER_FRAME: usize = 12;

/// Streaming ring (metres) of the full-detail terrain with the far view on and no
/// `--view-blocks`: two landblocks hold the 3 × 3 near field (§3.2) from any eye position in
/// the viewer's landblock (its farthest square is 160 √2 ≈ 226 m away) with room to spare.
pub const NEAR_FIELD_RING: f64 = 2.0 * LANDBLOCK_SIZE;

/// Which landblocks the walker draws at full detail with the far view on.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub enum NearFieldMode {
    /// The original's near field (§2, §3.2): Chebyshev distance ≤ *N* (1) from the viewer's
    /// landblock.
    #[default]
    Original,
    /// The walker's full-detail ring (`--view-blocks`, FD9 enhancement): every landblock
    /// whose square lies within the ring's radius.
    Ring,
}

/// What the far view hands to the full-detail terrain and water layers each frame (module
/// docs).
#[derive(Debug, Clone, Default, PartialEq)]
pub struct FarHandOff {
    /// Landblocks the far terrain batches on the GPU draw (and whose sector water the far
    /// view draws): the terrain and water layers skip them.
    pub covered: HashSet<LandblockKey>,
    /// `covered` plus the landblocks the current grid puts in the far field (batches still
    /// building): terrain blocks next to one mesh at the finest level and ignore it as a
    /// seam neighbour.
    pub far: HashSet<LandblockKey>,
    /// The sector materials of the seam overlay (§6.5): `Some` only while
    /// `NearFarSeamBlending` is on and the normal-mapped template is active.
    pub seam: Option<Arc<SeamMaterial>>,
}

/// What the near-terrain builder gets for the seam overlay (`far-view.md` §6.5,
/// `terrain-texturing.md` §12): the normal-mapped template's inputs shared by every sector
/// and each listed sector's colour list once it is on the GPU.
#[derive(Debug, Clone, Default, PartialEq)]
pub struct SeamMaterial {
    /// The viewer's landblock: its south-west corner is `c_NearViewOrigin` (§6.4, Inferred).
    pub viewer: Option<LandblockKey>,
    /// The detail texture (3001).
    pub detail: Option<TextureId>,
    /// The normal texture (3011, the template's default).
    pub normal: Option<TextureId>,
    /// Colour-list repeats per sector (transform set 0, §6.2).
    pub colour_repeats: [f32; 2],
    /// Detail and normal repeats per sector (transform set 1, §6.2).
    pub detail_repeats: [f32; 2],
    /// The far-draw fade (FD1).
    pub fade: FarFadeParams,
    /// The colour list of each listed sector on the GPU.
    pub colours: HashMap<SectorKey, TextureId>,
}

impl SeamMaterial {
    /// The colour list of the sector that holds landblock `key`, once on the GPU.
    #[must_use]
    pub fn colour_of(&self, key: LandblockKey) -> Option<TextureId> {
        self.colours.get(&SectorKey::of_landblock(key)).copied()
    }
}

impl FarHandOff {
    /// Whether a far batch on the GPU draws `key`.
    #[must_use]
    pub fn is_covered(&self, key: LandblockKey) -> bool {
        self.covered.contains(&key)
    }

    /// Whether `key` is (or is about to be) drawn by the far view.
    #[must_use]
    pub fn is_far(&self, key: LandblockKey) -> bool {
        self.far.contains(&key)
    }

    /// Whether a full-detail block at `key` borders the far field across an edge, so its
    /// edge must carry every sample the far stitch joins to (§5.2: the near field's
    /// full-resolution edge).
    #[must_use]
    pub fn borders_far(&self, key: LandblockKey) -> bool {
        !self.is_far(key)
            && [(-1, 0), (0, -1), (1, 0), (0, 1)]
                .iter()
                .any(|&(dx, dy)| key.offset(dx, dy).is_some_and(|n| self.is_far(n)))
    }
}

/// Most queued jobs.
const JOB_CAPACITY: usize = 256;

/// Frames a flow-map copy of the sector water survives without being drawn (a sector
/// whose water is out of view or beyond the fade for a moment keeps it; one that left the
/// list releases its flow map, `super::water::WaterSurfaces::release_unused_flows`).
const FLOW_COPY_GRACE_FRAMES: u64 = 300;

/// The far view's switches.
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct FarViewConfig {
    /// Draw the far view (`--far-view on`).
    pub enabled: bool,
    /// Which landblocks stay at full detail.
    pub near_field: NearFieldMode,
    /// Radii, template, sprites.
    pub settings: FarViewSettings,
    /// Worker threads.
    pub threads: usize,
}

impl Default for FarViewConfig {
    fn default() -> Self {
        Self {
            enabled: false,
            near_field: NearFieldMode::default(),
            settings: FarViewSettings::default(),
            threads: 2,
        }
    }
}

/// Counters of the last frame.
#[derive(Debug, Clone, Copy, Default, PartialEq, Eq)]
pub struct FarViewStats {
    /// Sectors listed.
    pub listed: usize,
    /// Listed sectors with a decoded record.
    pub ready: usize,
    /// Far-terrain batches drawn (terrain and ceiling).
    pub batches: usize,
    /// Sprite batches drawn.
    pub sprite_batches: usize,
    /// Sprites in the drawn batches.
    pub sprites: usize,
    /// Water quads drawn from the sectors.
    pub water_quads: usize,
    /// Landblocks the far terrain batches on the GPU cover.
    pub covered: usize,
    /// Live far entities of the distant set (§10).
    pub distant_live: usize,
    /// Far entities handed to the object layer (resolved and drawable).
    pub far_entities: usize,
    /// Far entities the object layer drew in the last frame it drew.
    pub far_drawn: usize,
    /// Jobs queued or running.
    pub pending: usize,
}

enum Job {
    Records,
    Template(Did, Template),
    Entity(Box<DistantEntry>),
    Sector(SectorKey),
    Table(Did),
    Build(BuildJob),
    Sprites(SpriteJob),
}

enum Done {
    Records(Result<FarViewRecords, WorldError>),
    Template(Did, Template, Option<TemplateDefaults>),
    Entity(u64, FarVisualResult),
    Sector(SectorKey, Box<Result<Option<SectorRecord>, WorldError>>),
    Table(Did, Result<Option<ImposterTable>, WorldError>),
    Built(SectorKey, u64, Box<Result<SectorMeshes, WorldError>>),
    Sprites(SectorKey, u64, [f32; 2], Arc<[FarSprite]>),
}

#[derive(Default)]
struct SectorGpu {
    generation: u64,
    terrain: Option<FarTerrainMesh>,
    ceiling: Option<FarTerrainMesh>,
    /// The landblocks the uploaded terrain batch covers.
    covers: Vec<(u8, u8)>,
    sprite_epoch: u64,
    /// Sprites with the sector offset they were placed for (sector corner relative to the
    /// viewer landblock's).
    sprites: Option<(SpriteBatch, [f32; 2])>,
    textures: Vec<TexRequest>,
    textured: bool,
    atlased: bool,
}

/// The far-view layer (module docs).
pub struct FarViewLayer {
    ctx: Arc<WorldContext>,
    state: FarViewState,
    runner: Option<JobRunner<Job, Done>>,
    backlog: VecDeque<Job>,
    in_flight: usize,
    records: Option<FarViewRecords>,
    records_asked: bool,
    /// The active template's defaults (§6.2): `None` until read (the draws use the plain
    /// template's 4 × 4 meanwhile).
    template: Option<TemplateDefaults>,
    template_asked: bool,
    template_done: bool,
    /// The normal-mapped template's normal texture (3011), referenced in the store.
    normal: Option<TexRequest>,
    /// The far entities (§10).
    far: FarEntities,
    near_mode: NearFieldMode,
    hand_off: Arc<FarHandOff>,
    gpu: HashMap<SectorKey, SectorGpu>,
    detail: Option<TexRequest>,
    horizon: Option<u32>,
    horizon_warned: bool,
    water: super::water::WaterSurfaces,
    /// Sprite epoch and placement offset of the sprites swapped in last, per sector.
    sprite_offsets: HashMap<SectorKey, (u64, [f32; 2])>,
    stats: FarViewStats,
}

impl std::fmt::Debug for FarViewLayer {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("FarViewLayer")
            .field("stats", &self.stats)
            .finish_non_exhaustive()
    }
}

/// The near field of the walker (§2, §3.2, FD9) for `mode`: with
/// [`NearFieldMode::Original`] the landblocks within Chebyshev distance `radius` of `eye`'s
/// landblock; with [`NearFieldMode::Ring`] the landblocks of `keys` (the terrain layer's) in
/// `eye`'s region whose square lies within `extent` of `eye` horizontally (the test the
/// terrain layer draws by, `walker.md` §11.3). Either way only landblocks for which
/// `drawable` holds (the terrain layer can draw them) are kept; the far batches cover the
/// others.
#[must_use]
pub fn near_field_of(
    mode: NearFieldMode,
    keys: impl Iterator<Item = LandblockKey>,
    drawable: impl Fn(LandblockKey) -> bool,
    eye: &WorldPos,
    extent: f64,
    radius: u32,
) -> NearField {
    let set = match (mode, eye.landblock()) {
        (NearFieldMode::Original, Some(v)) => {
            let r = i32::try_from(radius.min(40)).unwrap_or(1);
            (-r..=r)
                .flat_map(|dx| (-r..=r).map(move |dy| (dx, dy)))
                .filter_map(|(dx, dy)| v.offset(dx, dy))
                .filter(|k| drawable(*k))
                .map(|k| (k.x(), k.y()))
                .collect()
        }
        (NearFieldMode::Original, None) => std::collections::BTreeSet::new(),
        (NearFieldMode::Ring, _) => keys
            .filter(|k| {
                k.region() == eye.region && k.distance_to(eye.xy()) <= extent && drawable(*k)
            })
            .map(|k| (k.x(), k.y()))
            .collect(),
    };
    NearField::Set(Arc::new(set))
}

/// A far batch as renderer input.
#[must_use]
pub fn far_terrain_data<'a>(label: &'a str, m: &'a FarMesh) -> FarTerrainData<'a> {
    FarTerrainData {
        label,
        positions: &m.positions,
        normals: &m.normals,
        uvs: &m.uvs,
        indices: &m.indices,
        stitch_start: m.stitch_start,
    }
}

/// Placed sprites as renderer input.
#[must_use]
pub fn distant_sprites(sprites: &[FarSprite]) -> Vec<DistantSprite> {
    sprites
        .iter()
        .map(|s| DistantSprite {
            position: s.position,
            size: s.size,
            base: s.base,
            u: s.u,
            v: s.v,
            distance: s.distance,
            mirror: s.mirror,
        })
        .collect()
}

/// The sprite camera values of §9.3 (FD6): the render target's height, the vertical field
/// of view and *M_d*.
#[must_use]
pub fn sprite_view(height: u32, fov_y: f32, distances: &DistanceSettings) -> SpriteView {
    #[allow(clippy::cast_precision_loss)]
    SpriteView {
        target_height: height.max(1) as f32,
        fov_y,
        object_scale: distances.object_multiplier(),
    }
}

/// The far-draw fade of the settings as renderer input.
#[must_use]
pub fn fade_params(settings: &FarViewSettings) -> FarFadeParams {
    let f = settings.fade();
    FarFadeParams {
        start: f.start,
        inv_range: f.inv_range,
    }
}

/// Whether the square of side `size` with south-west corner `origin` (render frame) lies
/// wholly beyond the end of the far-draw fade `fade` seen from `eye`: its nearest point is
/// farther than *s* + 1 / *r* horizontally, so every point of it is at least that far and
/// fully faded (*a* = 0). Never with no fade (*r* = 0) or non-finite values.
#[must_use]
pub fn beyond_fade(origin: [f32; 2], size: f32, eye: Vec3, fade: FarFadeParams) -> bool {
    if !(fade.inv_range.is_finite() && fade.inv_range > 0.0 && fade.start.is_finite()) {
        return false;
    }
    let end = fade.start + 1.0 / fade.inv_range;
    let nx = eye.x.clamp(origin[0], origin[0] + size);
    let ny = eye.y.clamp(origin[1], origin[1] + size);
    let d = (eye.x - nx).hypot(eye.y - ny);
    d.is_finite() && d > end
}

/// The defaults of far-terrain template `did` used as `template` (§6.2,
/// [`template_defaults`]); `None` when the record is missing or does not decode.
fn template_defaults_of(
    ctx: &WorldContext,
    did: Did,
    template: Template,
) -> Option<TemplateDefaults> {
    let bytes = match ctx.source().read(did) {
        Ok(Some(b)) => b,
        Ok(None) => {
            tracing::warn!(did = %did, "far-terrain template missing");
            return None;
        }
        Err(e) => {
            tracing::warn!(did = %did, error = %e, "far-terrain template unreadable");
            return None;
        }
    };
    let rec = match ShaderRecord::parse_expecting(&bytes, did) {
        Ok(r) => r,
        Err(e) => {
            tracing::warn!(did = %did, error = %e, "far-terrain template does not decode");
            return None;
        }
    };
    Some(template_defaults(&rec, template))
}

/// Whether the far view hands the sector materials to the near terrain for the seam
/// overlay (§6.5): `NearFarSeamBlending` on and the normal-mapped template wanted
/// ([`FarViewSettings::seam_overlay`]) **and** the template that was read is the
/// normal-mapped one (UNIQUEDB may name only the plain one, [`active_template`]).
#[must_use]
pub fn seam_enabled(settings: &FarViewSettings, template: &TemplateDefaults) -> bool {
    settings.seam_overlay() && template.template == Template::NormalMapped
}

/// Parameter of transform set 0's *u* scale (`material.md` §8.4).
pub const PARAM_SCALE_U0: u32 = 1001;
/// Parameter of transform set 0's *v* scale (`material.md` §8.4).
pub const PARAM_SCALE_V0: u32 = 1002;
/// Parameter of transform set 1's *u* scale (`material.md` §8.4).
pub const PARAM_SCALE_U1: u32 = 1003;
/// Parameter of transform set 1's *v* scale (`material.md` §8.4).
pub const PARAM_SCALE_V1: u32 = 1004;
/// The normal texture slot of the normal-mapped template (§6.2).
pub const PARAM_NORMAL_TEXTURE: u32 = 3011;

/// What a far-terrain draw takes from its template's declared defaults (§6.2).
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct TemplateDefaults {
    /// Which template.
    pub template: Template,
    /// Colour-list repeats per sector: (1, 1) for the plain template, which samples the
    /// colour list at (*u*, *v*); transform set 0 for the normal-mapped one.
    pub colour_repeats: [f32; 2],
    /// Detail (and normal) repeats per sector: transform set 0 of the plain template, set 1
    /// of the normal-mapped one.
    pub detail_repeats: [f32; 2],
    /// The normal texture (3011) of the normal-mapped template.
    pub normal_texture: Option<Did>,
}

impl TemplateDefaults {
    /// The values §6.2 observed, for a template whose record is missing: colour (1, 1),
    /// detail 4 × 4, no normal texture.
    #[must_use]
    pub fn fallback(template: Template) -> Self {
        Self {
            template,
            colour_repeats: [1.0, 1.0],
            detail_repeats: [4.0, 4.0],
            normal_texture: None,
        }
    }
}

/// The defaults of a far-terrain template record used as `template` (§6.2): the scales of
/// the transform sets it samples with and, for the normal-mapped template, the default of
/// its normal texture 3011. A pair that is not declared as positive finite floats keeps the
/// value of [`TemplateDefaults::fallback`].
#[must_use]
pub fn template_defaults(rec: &ShaderRecord, template: Template) -> TemplateDefaults {
    let mut d = TemplateDefaults::fallback(template);
    match template {
        Template::Plain => {
            if let Some(r) = template_scale(rec, PARAM_SCALE_U0, PARAM_SCALE_V0) {
                d.detail_repeats = r;
            }
        }
        Template::NormalMapped => {
            if let Some(r) = template_scale(rec, PARAM_SCALE_U0, PARAM_SCALE_V0) {
                d.colour_repeats = r;
            }
            if let Some(r) = template_scale(rec, PARAM_SCALE_U1, PARAM_SCALE_V1) {
                d.detail_repeats = r;
            }
            d.normal_texture = match rec.declaration(PARAM_NORMAL_TEXTURE).map(|e| &e.value) {
                Some(tessera_material::records::ParamValue::Texture(t)) if !t.is_null() => Some(*t),
                _ => None,
            };
        }
    }
    d
}

/// The (`u`, `v`) default pair of a template's declarations; `None` unless both are
/// declared as positive finite floats.
#[must_use]
pub fn template_scale(rec: &ShaderRecord, u: u32, v: u32) -> Option<[f32; 2]> {
    let get = |id: u32| {
        rec.declaration(id)
            .and_then(|e| e.value.as_float())
            .filter(|x| x.is_finite() && *x > 0.0)
    };
    Some([get(u)?, get(v)?])
}

fn run(ctx: &WorldContext, job: Job) -> Done {
    match job {
        Job::Records => Done::Records(FarViewRecords::load(ctx)),
        Job::Template(d, t) => Done::Template(d, t, template_defaults_of(ctx, d, t)),
        Job::Entity(e) => Done::Entity(e.iid, far_visual(ctx, &e)),
        Job::Sector(k) => Done::Sector(k, Box::new(read_sector(&**ctx.source(), k, ctx.schema()))),
        Job::Table(d) => Done::Table(d, read_imposter_table(&**ctx.source(), d)),
        Job::Build(j) => Done::Built(j.key, j.generation, Box::new(j.run())),
        Job::Sprites(j) => Done::Sprites(j.key, j.epoch, j.offset, j.run()),
    }
}

impl FarViewLayer {
    /// A layer over `ctx` with `config`.
    #[must_use]
    pub fn new(ctx: Arc<WorldContext>, config: &FarViewConfig) -> Self {
        let worker_ctx = Arc::clone(&ctx);
        let runner = JobRunner::new(
            "far-view",
            config.threads.max(1),
            JOB_CAPACITY,
            move |job: Job| run(&worker_ctx, job),
        )
        .inspect_err(|e| tracing::warn!(error = %e, "far-view workers did not start"))
        .ok();
        let pid = ctx.pids().landblock.entity_content_layers;
        Self {
            ctx,
            state: FarViewState::new(config.settings, pid),
            runner,
            backlog: VecDeque::new(),
            in_flight: 0,
            records: None,
            records_asked: false,
            template: None,
            template_asked: false,
            template_done: false,
            normal: None,
            far: FarEntities::default(),
            near_mode: config.near_field,
            hand_off: Arc::new(FarHandOff::default()),
            gpu: HashMap::new(),
            detail: None,
            horizon: None,
            horizon_warned: false,
            water: super::water::WaterSurfaces::default(),
            sprite_offsets: HashMap::new(),
            stats: FarViewStats::default(),
        }
    }

    /// Counters of the last frame.
    #[must_use]
    pub fn stats(&self) -> FarViewStats {
        self.stats
    }

    /// Which landblocks stay at full detail.
    #[must_use]
    pub fn near_mode(&self) -> NearFieldMode {
        self.near_mode
    }

    /// The near radius *N* of the settings.
    #[must_use]
    pub fn near_radius(&self) -> u32 {
        self.state.settings().near_radius
    }

    /// The landscape extent *E* of the settings (§7.1).
    #[must_use]
    pub fn extent(&self) -> f32 {
        self.state.settings().extent()
    }

    /// The hand-off of the last update (module docs).
    #[must_use]
    pub fn hand_off(&self) -> Arc<FarHandOff> {
        Arc::clone(&self.hand_off)
    }

    /// Whether everything is in: the UNIQUEDB entries and the template read, every listed
    /// sector's record, batches and sprites built **and on the GPU**, their textures loaded
    /// (or failed), and nothing queued. False before the first update with a viewer.
    #[must_use]
    pub fn settled(&self, store: &ObjectLayer) -> bool {
        if self.records.is_none()
            || !self.template_done
            || self.state.viewer().is_none()
            || !self.state.settled()
            || !self.backlog.is_empty()
            || self.in_flight > 0
        {
            return false;
        }
        let pending = |t: TexRequest| matches!(store.lookup(t), TexLookup::Pending);
        if self.detail.is_some_and(pending) || self.normal.is_some_and(pending) {
            return false;
        }
        if self.far.pending() > 0 {
            return false;
        }
        self.state.listed().iter().all(|k| {
            let Some(slot) = self.state.slot(*k) else {
                return true;
            };
            if slot.ready().is_none() {
                // Missing or failed records are settled (the state checked loading ones).
                return true;
            }
            let Some(g) = self.gpu.get(k) else {
                return false;
            };
            let meshes_up = slot.build_failed == Some(slot.generation)
                || slot
                    .meshes
                    .as_ref()
                    .is_some_and(|m| m.generation == g.generation);
            let sprites_up = slot
                .sprites
                .as_ref()
                .is_none_or(|(e, _)| *e == g.sprite_epoch);
            meshes_up && sprites_up && g.textured && !g.textures.iter().copied().any(pending)
        })
    }

    /// A HUD line.
    #[must_use]
    pub fn hud_line(&self) -> String {
        let s = self.stats;
        format!(
            "far view: {} / {} sectors, {} batches over {} landblocks, {} sprites in {} batches, {} water, {} far entities ({} placed, {} drawn), {} jobs",
            s.ready,
            s.listed,
            s.batches,
            s.covered,
            s.sprites,
            s.sprite_batches,
            s.water_quads,
            s.distant_live,
            s.far_entities,
            s.far_drawn,
            s.pending
        )
    }

    fn release(gpu: SectorGpu, store: &mut dyn TextureStore) {
        if !gpu.textures.is_empty() {
            store.release(&gpu.textures);
        }
    }

    fn take_results(&mut self) {
        let Some(runner) = &self.runner else {
            return;
        };
        let mut done = Vec::new();
        while let Some(d) = runner.try_next() {
            done.push(d);
        }
        for d in done {
            self.in_flight = self.in_flight.saturating_sub(1);
            match d {
                Done::Records(r) => {
                    self.records = Some(r.unwrap_or_else(|e| {
                        tracing::warn!(error = %e, "far-view UNIQUEDB entries unavailable");
                        FarViewRecords::default()
                    }));
                }
                Done::Template(d, t, r) => {
                    if r.is_none() {
                        tracing::info!(did = %d, "far-terrain template unreadable; its observed defaults are used");
                    }
                    self.template = Some(r.unwrap_or_else(|| TemplateDefaults::fallback(t)));
                    self.template_done = true;
                }
                Done::Entity(iid, v) => self.far.resolved(iid, v),
                Done::Sector(k, r) => self.state.sector_loaded(k, *r),
                Done::Table(d, r) => self.state.table_loaded(d, r),
                Done::Built(k, g, r) => {
                    self.state.built(k, g, *r);
                }
                Done::Sprites(k, e, offset, s) => {
                    if self.state.sprites_placed(k, e, s) {
                        self.sprite_offsets.insert(k, (e, offset));
                    }
                }
            }
        }
    }

    fn submit(&mut self) {
        let Some(runner) = &self.runner else {
            self.backlog.clear();
            return;
        };
        while let Some(job) = self.backlog.pop_front() {
            match runner.submit(job) {
                Ok(()) => self.in_flight += 1,
                Err(job) => {
                    self.backlog.push_front(job);
                    break;
                }
            }
        }
    }

    /// One frame: results in, the viewer's sector list and grid, jobs out, uploads.
    /// `focus` is the viewer's position, `near` the landblocks the walker draws at full
    /// detail, `view` the sprite camera values (§9.3).
    pub fn update(
        &mut self,
        renderer: &mut Renderer<'_>,
        store: &mut ObjectLayer,
        focus: Option<&WorldPos>,
        near: NearField,
        view: SpriteView,
    ) {
        self.take_results();
        if !self.records_asked {
            self.records_asked = true;
            self.backlog.push_back(Job::Records);
        }
        if let (Some(r), None) = (&self.records, &self.detail)
            && let Some(d) = r.detail_texture
        {
            let t = TexRequest::new(d.0, VARIANT_OPAQUE);
            store.acquire(&[t]);
            self.detail = Some(t);
        }
        if let (Some(r), None) = (&self.records, self.horizon)
            && let Some(d) = r.horizon_mesh
        {
            store.acquire_model_parts(&[d.0]);
            self.horizon = Some(d.0);
        }
        // §6.1: the active template and its defaults (§6.2).
        if let Some(r) = &self.records
            && !self.template_asked
        {
            self.template_asked = true;
            match active_template(r, self.state.settings().template()) {
                Some((d, t)) => self.backlog.push_back(Job::Template(d, t)),
                None => {
                    tracing::info!(
                        "no far-terrain template in UNIQUEDB; the plain template's observed defaults"
                    );
                    self.template = Some(TemplateDefaults::fallback(Template::Plain));
                    self.template_done = true;
                }
            }
        }
        if let (Some(t), None) = (&self.template, &self.normal)
            && let Some(n) = t.normal_texture
        {
            let req = TexRequest::new(n.0, VARIANT_OPAQUE);
            store.acquire(&[req]);
            self.normal = Some(req);
        }
        let viewer = focus.and_then(WorldPos::landblock).map(|landblock| Viewer {
            landblock,
            instance: 0,
        });
        let work = self.state.update(viewer, near, view);
        if work.discarded {
            for (_, g) in self.gpu.drain() {
                Self::release(g, store);
            }
            self.sprite_offsets.clear();
        }
        for k in &work.freed {
            if let Some(g) = self.gpu.remove(k) {
                Self::release(g, store);
            }
            self.sprite_offsets.remove(k);
        }
        self.backlog
            .extend(work.load_sectors.iter().map(|k| Job::Sector(*k)));
        self.backlog
            .extend(work.load_tables.iter().map(|d| Job::Table(*d)));
        self.backlog.extend(work.build.into_iter().map(Job::Build));
        self.backlog
            .extend(work.sprites.into_iter().map(Job::Sprites));
        self.submit();
        self.upload(renderer, store);
        self.hand_off = Arc::new(self.compute_hand_off(store));
        self.stats.covered = self.hand_off.covered.len();
        // Far entities (§10): walk the distant set, resolve what was created, hand them to
        // the object layer per landblock.
        if let Some(p) = focus {
            let active = self.ctx.options().active_content_layers.clone();
            let changes = self.state.distant_mut().walk(
                p,
                VIEWER_INSTANCE,
                DISTANT_FACTOR,
                // The main-set test is made by distance in the object layer, which leaves a
                // far entity to its landblock's own objects within their cap
                // (`super::objects::handed_to_main`; Tessera choice, §12 OPEN).
                |_| false,
                |e| {
                    tessera_physics::geometry::content_layers_pass(
                        e.content_layers.as_deref(),
                        &active,
                        0,
                    )
                },
            );
            let jobs = self.far.apply(&changes);
            self.backlog
                .extend(jobs.into_iter().map(|e| Job::Entity(Box::new(e))));
            self.submit();
        }
        self.stats.far_entities = self.far.sync(store);
        self.stats.far_drawn = store.stats().far_objects_drawn;
        let listed = self.state.listed();
        self.stats.listed = listed.len();
        self.stats.ready = listed
            .iter()
            .filter(|k| self.state.slot(**k).and_then(|s| s.ready()).is_some())
            .count();
        self.stats.distant_live = self.state.distant().live().count();
        self.stats.pending = self.backlog.len() + self.in_flight;
    }

    fn upload(&mut self, renderer: &mut Renderer<'_>, store: &mut ObjectLayer) {
        let mut budget = MAX_UPLOADS_PER_FRAME;
        let keys: Vec<SectorKey> = self.state.listed().to_vec();
        for k in keys {
            let Some(slot) = self.state.slot(k) else {
                continue;
            };
            let Some(rec) = slot.ready().cloned() else {
                continue;
            };
            let g = self.gpu.entry(k).or_default();
            if !g.textured {
                g.textured = true;
                let mut reqs = Vec::with_capacity(2);
                if rec.colour_list.0 != 0 {
                    reqs.push(TexRequest::new(rec.colour_list.0, VARIANT_OPAQUE));
                }
                if rec.ceiling_colour_list.0 != 0 {
                    reqs.push(TexRequest::new(rec.ceiling_colour_list.0, VARIANT_OPAQUE));
                }
                store.acquire(&reqs);
                g.textures.extend(reqs);
            }
            if !g.atlased
                && let Some(t) = self.state.table(rec.imposter_table)
            {
                g.atlased = true;
                let reqs: Vec<TexRequest> = [t.colour_atlas, t.second_atlas]
                    .iter()
                    .filter(|d| d.0 != 0)
                    .map(|d| TexRequest::new(d.0, VARIANT_OWN_ALPHA))
                    .collect();
                store.acquire(&reqs);
                g.textures.extend(reqs);
            }
            if budget == 0 {
                continue;
            }
            if let Some(m) = &slot.meshes
                && m.generation != g.generation
            {
                budget -= 1;
                let label = format!("far terrain {:#010x}", rec.did.0);
                let up = |mesh: &FarMesh, r: &mut Renderer<'_>| {
                    if mesh.is_empty() {
                        return None;
                    }
                    r.upload_far_terrain(&far_terrain_data(&label, mesh))
                        .inspect_err(|e| tracing::warn!(error = %e, "far terrain upload failed"))
                        .ok()
                };
                g.terrain = up(&m.terrain, renderer);
                g.ceiling = m.ceiling.as_ref().and_then(|c| up(c, renderer));
                g.generation = m.generation;
                g.covers = if g.terrain.is_some() {
                    m.far_landblocks().collect()
                } else {
                    Vec::new()
                };
            }
            if let Some((epoch, sprites)) = &slot.sprites
                && *epoch != g.sprite_epoch
                && let Some((e, offset)) = self.sprite_offsets.get(&k)
                && e == epoch
                && budget > 0
            {
                budget -= 1;
                g.sprite_epoch = *epoch;
                g.sprites = renderer
                    .upload_sprite_batch(
                        &format!("sprites {:#010x}", rec.did.0),
                        &distant_sprites(sprites),
                    )
                    .inspect_err(|e| tracing::warn!(error = %e, "sprite upload failed"))
                    .ok()
                    .map(|b| (b, *offset));
            }
        }
    }

    /// Whether a sector's terrain batch is drawn: its colour list is on the GPU or failed
    /// (a white batch while it loads would flash).
    fn colour_settled(store: &ObjectLayer, rec: &SectorRecord) -> bool {
        rec.colour_list.0 == 0
            || !matches!(
                store.lookup(TexRequest::new(rec.colour_list.0, VARIANT_OPAQUE)),
                TexLookup::Pending
            )
    }

    /// The hand-off of this frame: the landblocks of the far terrain batches that
    /// [`FarViewLayer::draw`] draws, and those the current grid puts in the far field.
    fn compute_hand_off(&self, store: &ObjectLayer) -> FarHandOff {
        let mut out = FarHandOff::default();
        let region = self.state.viewer().map(|v| v.landblock.region());
        for k in self.state.listed() {
            let (Some(slot), Some(g)) = (self.state.slot(*k), self.gpu.get(k)) else {
                continue;
            };
            let Some(rec) = slot.ready() else {
                continue;
            };
            if g.terrain.is_none() || !Self::colour_settled(store, rec) {
                continue;
            }
            out.covered.extend(
                g.covers
                    .iter()
                    .filter_map(|&(x, y)| LandblockKey::new(k.region(), x, y).ok()),
            );
        }
        out.far.clone_from(&out.covered);
        if let (Some(grid), Some(region)) = (self.state.grid(), region) {
            out.far.extend(grid.cells().filter_map(|((x, y), c)| {
                matches!(c, Cell::Far(_))
                    .then(|| LandblockKey::new(region, x, y).ok())
                    .flatten()
            }));
        }
        out.seam = self.seam_material(store).map(Arc::new);
        out
    }

    /// The seam overlay's sector materials (§6.5): only with `NearFarSeamBlending` on and
    /// the normal-mapped template active, its normal texture on the GPU.
    fn seam_material(&self, store: &ObjectLayer) -> Option<SeamMaterial> {
        let t = self.template?;
        if !seam_enabled(self.state.settings(), &t) {
            return None;
        }
        let ready = |r: Option<TexRequest>| match r.map(|r| store.lookup(r)) {
            Some(TexLookup::Ready(id)) => Some(id),
            _ => None,
        };
        let normal = ready(self.normal)?;
        let mut colours = HashMap::new();
        for k in self.state.listed() {
            let Some(rec) = self.state.slot(*k).and_then(|s| s.ready()) else {
                continue;
            };
            if let Some(id) = ready(
                (rec.colour_list.0 != 0)
                    .then(|| TexRequest::new(rec.colour_list.0, VARIANT_OPAQUE)),
            ) {
                colours.insert(*k, id);
            }
        }
        Some(SeamMaterial {
            viewer: self.state.viewer().map(|v| v.landblock),
            detail: ready(self.detail),
            normal: Some(normal),
            colour_repeats: t.colour_repeats,
            detail_repeats: t.detail_repeats,
            fade: fade_params(self.state.settings()),
            colours,
        })
    }

    /// Queues the far terrain, the sector water outside the near field, the sprites and
    /// the horizon fill into `pass`. `eye` is the camera in the render frame, `distance_scale`
    /// *c* of §9.3 (crowd factor × *M_d*).
    #[allow(clippy::too_many_arguments)]
    pub fn draw(
        &mut self,
        pass: &mut Pass,
        anchor: &Anchor,
        eye: Vec3,
        distance_scale: f32,
        resolver: &mut Resolver,
        store: &mut ObjectLayer,
        water: bool,
        time: f64,
    ) {
        let settings = *self.state.settings();
        let fade = fade_params(&settings);
        let lookup = |store: &ObjectLayer, t: Option<TexRequest>| -> Option<TextureId> {
            match t.map(|t| store.lookup(t)) {
                Some(TexLookup::Ready(id)) => Some(id),
                _ => None,
            }
        };
        let detail = lookup(store, self.detail);
        let mut batches = 0usize;
        let mut sprite_draws: Vec<(f32, SpriteDraw)> = Vec::new();
        let mut sprites = 0usize;
        // Per water surface and the sector's flow map (0: the material as it is).
        let mut quads: HashMap<(Did, Did), Vec<WaterQuad>> = HashMap::new();
        let viewer = self.state.viewer();
        let hand_off = Arc::clone(&self.hand_off);
        let ctx = Arc::clone(&self.ctx);
        let tables = ctx.systems().water.as_ref().map(|w| w.tables());
        for k in self.state.listed() {
            let (Some(slot), Some(g)) = (self.state.slot(*k), self.gpu.get(k)) else {
                continue;
            };
            let Some(rec) = slot.ready() else {
                continue;
            };
            let Some(origin) = k.corner_landblock().and_then(|c| anchor.offset_of(c)) else {
                continue;
            };
            let tf = Mat4::from_translation(origin);
            let colour = lookup(
                store,
                Some(TexRequest::new(rec.colour_list.0, VARIANT_OPAQUE)),
            );
            let near_view = viewer
                .and_then(|v| anchor.offset_of(v.landblock))
                .map(|o| [o.x, o.y]);
            let t = self
                .template
                .unwrap_or_else(|| TemplateDefaults::fallback(Template::Plain));
            // §6.4: the normal-mapped template draws with its normal texture once loaded.
            let normal_map = (t.template == Template::NormalMapped)
                .then(|| lookup(store, self.normal))
                .flatten();
            let far_draw = |m: &FarTerrainMesh, colour: Option<TextureId>| {
                let mut d = FarTerrainDraw::new(m.clone(), tf);
                d.colour = colour;
                d.detail = detail;
                d.colour_repeats = t.colour_repeats;
                d.detail_repeats = t.detail_repeats;
                d.fade = fade;
                d.normal_map = normal_map;
                d.near_view_origin = near_view;
                d
            };
            if let Some(m) = &g.terrain
                && Self::colour_settled(store, rec)
            {
                pass.far_terrain(far_draw(m, colour));
                batches += 1;
            }
            if let Some(m) = &g.ceiling {
                let c = lookup(
                    store,
                    Some(TexRequest::new(rec.ceiling_colour_list.0, VARIANT_OPAQUE)),
                );
                pass.far_terrain(far_draw(m, c));
                batches += 1;
            }
            if let (Some((b, offset)), Some(t)) = (&g.sprites, self.state.table(rec.imposter_table))
            {
                let atlas = lookup(
                    store,
                    Some(TexRequest::new(t.colour_atlas.0, VARIANT_OWN_ALPHA)),
                );
                let normals = (t.second_atlas.0 != 0)
                    .then(|| {
                        lookup(
                            store,
                            Some(TexRequest::new(t.second_atlas.0, VARIANT_OWN_ALPHA)),
                        )
                    })
                    .flatten();
                if atlas.is_some() && !b.is_empty() {
                    // Sprite positions are relative to the viewer landblock they were placed
                    // for: the sector corner minus the placement offset.
                    let base = origin - Vec3::new(offset[0], offset[1], 0.0);
                    let mut d = SpriteDraw::new(b.clone(), Mat4::from_translation(base), atlas);
                    d.normal_atlas = normals;
                    d.distance_scale = distance_scale;
                    d.fade = fade;
                    let centre = origin + Vec3::new(240.0, 240.0, 0.0);
                    let dist = (centre - eye).truncate().length();
                    sprites += b.len() as usize;
                    sprite_draws.push((dist, d));
                }
            }
            // Sector water of the landblocks the far batches cover (§8; the walker's water
            // layer draws the others, skipping these: the same hand-off as the terrain).
            if water && let Some(tables) = &tables {
                for (x, y, pair) in sector_water(rec) {
                    let (cx, cy) = k.corner();
                    let covered = LandblockKey::new(k.region(), cx + x, cy + y)
                        .is_ok_and(|lb| hand_off.is_covered(lb));
                    if !covered {
                        continue;
                    }
                    let Some(surface) = tables
                        .water_table(k.region())
                        .and_then(|t| t.select(u32::from(pair.water_type)))
                        .and_then(|t| t.wet.as_ref())
                        .map(|w| w.surface)
                    else {
                        continue;
                    };
                    let size = LANDBLOCK_SIZE as f32;
                    let o = origin + Vec3::new(f32::from(x) * size, f32::from(y) * size, 0.0);
                    // Limited like the far terrain (§8 OPEN, Tessera): a quad wholly beyond
                    // the end of the far-draw fade is not drawn.
                    if beyond_fade([o.x, o.y], size, eye, fade) {
                        continue;
                    }
                    quads.entry((surface, rec.flow_map)).or_default().push(
                        // §8.1: the landblock's place in its 3 × 3 sector.
                        WaterQuad::landblock([o.x, o.y], pair.height)
                            .with_sector_uv(u32::from(x), u32::from(y)),
                    );
                }
            }
        }
        // FD4: far to near.
        sprite_draws.sort_by(|a, b| b.0.total_cmp(&a.0));
        let sprite_batches = sprite_draws.len();
        for (_, d) in sprite_draws {
            pass.distant_sprites(d);
        }
        let mut water_quads = 0usize;
        let region_offset = {
            let o = anchor.origin();
            [-(o[0] as f32), -(o[1] as f32)]
        };
        self.water.begin(time);
        for ((surface, flow), list) in quads {
            // `distant-landscape.md` §5.5: a copy of the material with 3075 = `flow_map`.
            let super::water::SurfaceState::Ready {
                params,
                animated_scales,
                normal_map,
                textures,
                ..
            } = self.water.surface_with_flow(surface, flow, resolver, store)
            else {
                continue;
            };
            water_quads += list.len();
            let mut d = WaterDraw::new(list, normal_map, params);
            d.block_offset =
                super::liquid::block_offset(animated_scales, region_offset, [eye.x, eye.y]);
            d.textures = textures;
            // Faded like the far terrain (§7.2, FD1; §8 OPEN, Tessera choice).
            d.fade = fade;
            pass.water(d);
        }
        // The flow-map copies of sectors no longer drawn give their textures back.
        self.water
            .release_unused_flows(store, FLOW_COPY_GRACE_FRAMES);
        self.draw_horizon(pass, eye, store, &settings);
        self.stats.batches = batches;
        self.stats.sprite_batches = sprite_batches;
        self.stats.sprites = sprites;
        self.stats.water_quads = water_quads;
    }

    /// The horizon fill (§7.3), drawn only when its horizontal radius reaches 1.25 × *E*.
    fn draw_horizon(
        &mut self,
        pass: &mut Pass,
        eye: Vec3,
        store: &ObjectLayer,
        settings: &FarViewSettings,
    ) {
        let Some(part) = self.horizon else {
            return;
        };
        let Some(model) = store.model(part, 0) else {
            return;
        };
        let aabb = model.info().aabb;
        let bounds = MeshBounds {
            min: aabb.min.to_array(),
            max: aabb.max.to_array(),
        };
        let extent = settings.extent();
        let r = bounds.max[0].max(bounds.max[1]);
        if r < horizon::EXTENT_FACTOR * extent {
            if !self.horizon_warned {
                self.horizon_warned = true;
                tracing::info!(
                    radius = r,
                    extent,
                    "horizon fill smaller than 1.25 E; not drawn (its depth state is not traced)"
                );
            }
            return;
        }
        let floor = horizon::floor(self.state.ring_floor(), self.state.near_floor());
        let Some(floor) = floor else {
            return;
        };
        let Some(p) = horizon::placement(eye.z, floor, extent, &bounds) else {
            return;
        };
        let tf = Mat4::from_translation(Vec3::new(eye.x, eye.y, eye.z - p.drop))
            * Mat4::from_scale(Vec3::new(1.0, 1.0, p.scale_z));
        let mut batch = InstanceBatch::new(model.clone(), vec![Instance::new(tf)]);
        batch.cast_shadow = false;
        pass.instanced(batch);
    }

    /// Frees every GPU object and texture reference (shutdown, region change of the walker,
    /// and before the object layer is cleared).
    pub fn clear(&mut self, store: &mut ObjectLayer) {
        for (_, g) in self.gpu.drain() {
            Self::release(g, store);
        }
        if let Some(t) = self.detail.take() {
            store.release(&[t]);
        }
        if let Some(t) = self.normal.take() {
            store.release(&[t]);
        }
        self.far.clear(store);
        if let Some(p) = self.horizon.take() {
            store.release_model_parts(&[p]);
        }
        self.water.release(store);
        self.sprite_offsets.clear();
        self.hand_off = Arc::new(FarHandOff::default());
        self.state.reset();
    }

    /// Stops the workers.
    pub fn shutdown(&mut self) {
        if let Some(r) = &mut self.runner {
            r.shutdown();
        }
    }
}

// ---------------------------------------------------------------------------------------
// Far entities (§10)
// ---------------------------------------------------------------------------------------

/// The viewer's instance in the walker (Tessera Decision FD15: always 0, so the instance
/// rule of §10 step 2 never fires there; the distant set implements it).
pub const VIEWER_INSTANCE: u16 = 0;

/// The manager factor *s* of `entity-descriptor.md` §9.3: 1.0 at construction (§10 step 3;
/// what else sets it is OPEN, §15).
pub const DISTANT_FACTOR: f32 = 1.0;

/// Entity type Weenie of `entity-descriptor.md` §4.1 (the 0x70 entries of the distant
/// battles, `distant-landscape.md` §3.8).
pub const ENTITY_TYPE_WEENIE: u32 = 6;

/// Delay before a far entity whose object could not be read now is resolved again.
const FAR_RETRY: std::time::Duration = std::time::Duration::from_millis(500);

/// Most attempts at resolving a far entity whose object cannot be read now (not the data's
/// fault, [`FarVisualResult::Retry`]): after these it is created but not drawn, so a lasting
/// read error does not keep the far view unsettled (about 2.5 s of retries; Tessera
/// choice).
pub const FAR_MAX_ATTEMPTS: u32 = 5;

/// The template record that draws `wanted` (§6.1): the normal-mapped record when that
/// template is wanted and the UNIQUEDB map names it, else the plain one.
fn active_template(r: &FarViewRecords, wanted: Template) -> Option<(Did, Template)> {
    match (wanted, r.normal_template, r.plain_template) {
        (Template::NormalMapped, Some(d), _) => Some((d, Template::NormalMapped)),
        (Template::NormalMapped, None, plain) => {
            tracing::info!("no normal-mapped far-terrain template in UNIQUEDB; plain template");
            plain.map(|d| (d, Template::Plain))
        }
        (Template::Plain, _, plain) => plain.map(|d| (d, Template::Plain)),
    }
}

/// What a far entity is drawn with (the normal path, `walker.md` §11.4).
#[derive(Debug, Clone, PartialEq)]
pub struct FarVisual {
    /// The setup's part meshes (0x06).
    pub parts: Arc<[Did]>,
    /// The LOD-class values of its `Render_LODClass` (`walker.md` §11.4.3; the component's
    /// initial values without a class).
    pub lod: LodValues,
    /// Its `Render_DepthBias_Type` entry (`decals.md` §6.5).
    pub depth_bias: DrawDepthBias,
}

/// The result of resolving a far entity's object on a worker.
#[derive(Debug, Clone, PartialEq)]
pub enum FarVisualResult {
    /// Drawn with this.
    Drawn(Arc<FarVisual>),
    /// Created but not drawn: no geometry, suppressed (`entity-descriptor.md` §9), a Weenie
    /// entry or a skinned rig (module docs), or its chain does not resolve.
    NotDrawn,
    /// A record could not be read now (not the data's fault): resolve again later.
    Retry,
}

/// Resolves a distant entry's object (module docs).
fn far_visual(ctx: &WorldContext, e: &DistantEntry) -> FarVisualResult {
    if e.entity_type == ENTITY_TYPE_WEENIE {
        return FarVisualResult::NotDrawn;
    }
    let r = ctx.objects().resolve(e.object);
    if r.transient {
        return FarVisualResult::Retry;
    }
    if r.error.is_some() || !r.has_geometry() || r.draw.suppressed() {
        return FarVisualResult::NotDrawn;
    }
    let min_rig = ctx.options().min_rig_bones;
    if min_rig > 0 && r.bones.is_some_and(|b| b >= min_rig) {
        return FarVisualResult::NotDrawn;
    }
    let chain = match r.physics {
        Some(p) => match ctx
            .systems()
            .object_properties
            .get(&**ctx.source(), ctx.schema(), p)
        {
            Ok(c) => c,
            Err(err) if !err.is_data_error() => return FarVisualResult::Retry,
            Err(_) => None,
        },
        None => None,
    };
    let pids = &ctx.systems().pids;
    let enum_of = |pid: Option<u32>| {
        pid.and_then(|pid| {
            tessera_world::draw_distance::placement_lod_class(pid, None, None, chain.as_deref())
        })
    };
    let default = LodClasses::default();
    let classes = ctx.systems().lod_classes.as_deref().unwrap_or(&default);
    let lod = classes.values(enum_of(pids.lod_class), false);
    let depth_bias = super::terrain::entity_depth_bias(
        ctx.systems().depth_bias.as_deref(),
        enum_of(pids.depth_bias_type),
    );
    FarVisualResult::Drawn(Arc::new(FarVisual {
        parts: Arc::clone(&r.parts),
        lod,
        depth_bias,
    }))
}

/// The objects of far landblock `key` for the object layer: one [`ObjectInfo`] per entry
/// with its part instances in the landblock's frame, placed by the entry's Position
/// (orientation stored as (w, x, y, z), `coordinates.md` §11.1) and uniform scale. Entries
/// with a non-finite position or no parts are left out; a non-finite or non-positive scale
/// counts as 1.
#[must_use]
pub fn far_block(
    key: LandblockKey,
    members: &[(&DistantEntry, &FarVisual)],
) -> (Vec<ObjectInstance>, Vec<ObjectInfo>) {
    let mut instances = Vec::new();
    let mut objects = Vec::new();
    let o = key.origin();
    for (e, v) in members {
        #[allow(clippy::cast_possible_truncation)]
        let pos = Vec3::new(
            (e.position.x - o[0]) as f32,
            (e.position.y - o[1]) as f32,
            e.position.z as f32,
        );
        if !pos.is_finite() || v.parts.is_empty() {
            continue;
        }
        let rot = e
            .orientation
            .map(|[w, x, y, z]| Quat::from_xyzw(x, y, z, w))
            .filter(|q| q.is_finite() && q.length_squared() > 0.0)
            .map_or(Quat::IDENTITY, Quat::normalize);
        let scale = if e.scale.is_finite() && e.scale > 0.0 {
            e.scale
        } else {
            1.0
        };
        let model = Mat4::from_scale_rotation_translation(Vec3::splat(scale), rot, pos);
        let (Ok(object), Ok(first), Ok(count)) = (
            u32::try_from(objects.len()),
            u32::try_from(instances.len()),
            u32::try_from(v.parts.len()),
        ) else {
            break;
        };
        instances.extend(v.parts.iter().map(|p| ObjectInstance {
            part: p.0,
            model,
            scale,
            figure: None,
            object,
        }));
        objects.push(ObjectInfo {
            position: pos,
            lod: v.lod,
            generated: false,
            first,
            count,
            cell: OUTDOORS,
            depth_bias: v.depth_bias,
        });
    }
    (instances, objects)
}

/// The far landblocks to hand to the object layer and the entities of each: every grouped
/// landblock with its members whose object resolved to something drawn (`drawn`);
/// landblocks left with none are omitted.
#[must_use]
pub fn wanted_far_blocks(
    groups: &DistantGroups,
    drawn: impl Fn(u64) -> bool,
) -> BTreeMap<LandblockKey, Vec<u64>> {
    groups
        .landblocks()
        .filter_map(|k| {
            let ids: Vec<u64> = groups.members(k).filter(|i| drawn(*i)).collect();
            (!ids.is_empty()).then_some((k, ids))
        })
        .collect()
}

/// The changes that turn `inserted` into `wanted`: landblocks to (re)insert with their
/// entities, and landblocks to remove. A landblock for which `settling` holds (one of its
/// entries is still being resolved) is not (re)inserted yet: what is in the layer stays
/// until the batch is complete, so the layer rebuilds it (and resets its fades) once.
#[must_use]
pub fn far_block_changes(
    wanted: &BTreeMap<LandblockKey, Vec<u64>>,
    inserted: &HashMap<LandblockKey, Vec<u64>>,
    settling: impl Fn(LandblockKey) -> bool,
) -> (Vec<(LandblockKey, Vec<u64>)>, Vec<LandblockKey>) {
    let insert = wanted
        .iter()
        .filter(|(k, ids)| inserted.get(*k) != Some(*ids) && !settling(**k))
        .map(|(k, ids)| (*k, ids.clone()))
        .collect();
    let mut remove: Vec<LandblockKey> = inserted
        .keys()
        .filter(|k| !wanted.contains_key(*k))
        .copied()
        .collect();
    remove.sort_unstable();
    (insert, remove)
}

#[derive(Debug, Clone)]
enum VisualState {
    /// Being resolved; the attempts made before this one.
    Pending(u32),
    Ready(FarVisualResult),
    /// Resolved again at the instant; the attempts made.
    RetryAt(std::time::Instant, u32),
}

/// The far entities of the layer: the live distant entries grouped per landblock, what
/// each resolved to, and what is in the object layer.
#[derive(Debug, Default)]
struct FarEntities {
    groups: DistantGroups,
    entries: HashMap<u64, DistantEntry>,
    visuals: HashMap<u64, VisualState>,
    inserted: HashMap<LandblockKey, Vec<u64>>,
    /// Whether the warning about an entity given up after [`FAR_MAX_ATTEMPTS`] was logged.
    gave_up_warned: bool,
}

impl FarEntities {
    /// Applies a walk; returns the entries to resolve (new ones and due retries).
    fn apply(&mut self, changes: &DistantChanges) -> Vec<DistantEntry> {
        self.groups.apply(changes);
        for iid in &changes.destroy {
            self.entries.remove(iid);
            self.visuals.remove(iid);
        }
        let mut jobs = Vec::new();
        for e in &changes.create {
            self.entries.insert(e.iid, e.clone());
            if let std::collections::hash_map::Entry::Vacant(v) = self.visuals.entry(e.iid) {
                v.insert(VisualState::Pending(0));
                jobs.push(e.clone());
            }
        }
        let now = std::time::Instant::now();
        for (iid, v) in &mut self.visuals {
            if let VisualState::RetryAt(at, tries) = *v
                && now >= at
                && let Some(e) = self.entries.get(iid)
            {
                *v = VisualState::Pending(tries);
                jobs.push(e.clone());
            }
        }
        jobs
    }

    /// A worker's answer for `iid` (ignored unless the entity waits for one: it was removed
    /// meanwhile, or the answer is stale). After [`FAR_MAX_ATTEMPTS`] answers of
    /// [`FarVisualResult::Retry`] the entity is given up (created, not drawn).
    fn resolved(&mut self, iid: u64, v: FarVisualResult) {
        let Some(slot) = self.visuals.get_mut(&iid) else {
            return;
        };
        let VisualState::Pending(tries) = *slot else {
            return;
        };
        *slot = match v {
            FarVisualResult::Retry if tries + 1 >= FAR_MAX_ATTEMPTS => {
                if self.gave_up_warned {
                    tracing::debug!(iid, "far entity unreadable; not drawn");
                } else {
                    self.gave_up_warned = true;
                    tracing::warn!(
                        iid,
                        attempts = FAR_MAX_ATTEMPTS,
                        "far entity's object could not be read; it is not drawn (further cases at debug level)"
                    );
                }
                VisualState::Ready(FarVisualResult::NotDrawn)
            }
            FarVisualResult::Retry => {
                VisualState::RetryAt(std::time::Instant::now() + FAR_RETRY, tries + 1)
            }
            other => VisualState::Ready(other),
        };
    }

    /// Whether entity `iid` is still being resolved (or waits for a retry).
    fn is_pending(&self, iid: u64) -> bool {
        self.visuals
            .get(&iid)
            .is_some_and(|v| !matches!(v, VisualState::Ready(_)))
    }

    /// Entities still being resolved.
    fn pending(&self) -> usize {
        self.visuals
            .values()
            .filter(|v| !matches!(v, VisualState::Ready(_)))
            .count()
    }

    fn drawn(&self, iid: u64) -> Option<&Arc<FarVisual>> {
        match self.visuals.get(&iid) {
            Some(VisualState::Ready(FarVisualResult::Drawn(v))) => Some(v),
            _ => None,
        }
    }

    /// Hands the far landblocks to `store` (module docs); returns the far entities in it.
    fn sync(&mut self, store: &mut ObjectLayer) -> usize {
        let wanted = wanted_far_blocks(&self.groups, |i| self.drawn(i).is_some());
        let (insert, remove) = far_block_changes(&wanted, &self.inserted, |k| {
            self.groups.members(k).any(|i| self.is_pending(i))
        });
        for k in remove {
            store.remove_far(k);
            self.inserted.remove(&k);
        }
        for (k, ids) in insert {
            let members: Vec<(&DistantEntry, &FarVisual)> = ids
                .iter()
                .filter_map(|i| Some((self.entries.get(i)?, &**self.drawn(*i)?)))
                .collect();
            let (instances, objects) = far_block(k, &members);
            store.insert_far(k, instances.into(), objects.into());
            self.inserted.insert(k, ids);
        }
        self.inserted.values().map(Vec::len).sum()
    }

    /// Removes every far entity from `store` and forgets them.
    fn clear(&mut self, store: &mut ObjectLayer) {
        for (k, _) in self.inserted.drain() {
            store.remove_far(k);
        }
        self.groups.clear();
        self.entries.clear();
        self.visuals.clear();
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use tessera_material::FloatCurve;
    use tessera_material::records::{ParamEntry, ParamValue};
    use tessera_world::far_view::Template;

    fn lb(x: u8, y: u8) -> LandblockKey {
        LandblockKey::new(1, x, y).unwrap()
    }

    fn eye_in(x: u8, y: u8, fx: f64, fy: f64) -> WorldPos {
        WorldPos::new(
            1,
            (f64::from(x) + fx) * LANDBLOCK_SIZE,
            (f64::from(y) + fy) * LANDBLOCK_SIZE,
            50.0,
        )
    }

    fn set(n: &NearField) -> Vec<(u8, u8)> {
        match n {
            NearField::Set(s) => s.iter().copied().collect(),
            NearField::Radius(_) => panic!("expected a set"),
        }
    }

    #[test]
    fn far_water_ends_with_the_far_terrain_fade() {
        // FD1 at High: s = E − 160, r = 1/160, so nothing is visible beyond E = 2,080.
        let fade = FarFadeParams {
            start: 1_920.0,
            inv_range: 1.0 / 160.0,
        };
        let eye = Vec3::new(0.0, 0.0, 500.0);
        // Nearest point 1,920 or 2,080 away: inside the fade band, drawn.
        assert!(!beyond_fade([1_920.0, -80.0], 160.0, eye, fade));
        assert!(!beyond_fade([2_080.0, 0.0], 160.0, eye, fade));
        // Nearest point just past E: wholly faded, not drawn (whatever the eye height).
        assert!(beyond_fade([2_081.0, 0.0], 160.0, eye, fade));
        assert!(beyond_fade([1_500.0, 1_500.0], 160.0, eye, fade));
        // A quad around the eye is never beyond.
        assert!(!beyond_fade([-80.0, -80.0], 160.0, eye, fade));
        // No fade, or hostile values: never limited.
        assert!(!beyond_fade(
            [1.0e6, 0.0],
            160.0,
            eye,
            FarFadeParams::default()
        ));
        let nan = FarFadeParams {
            start: f32::NAN,
            inv_range: 1.0,
        };
        assert!(!beyond_fade([1.0e6, 0.0], 160.0, eye, nan));
        // The settings' fade is the one the far terrain uses.
        let settings = FarViewSettings::default();
        let f = fade_params(&settings);
        assert_eq!(
            (f.start, f.inv_range),
            (settings.fade().start, settings.fade().inv_range)
        );
    }

    #[test]
    fn original_near_field_is_the_drawable_3_by_3() {
        let eye = eye_in(100, 50, 0.9, 0.1);
        assert_eq!(eye.landblock(), Some(lb(100, 50)));
        // Every landblock drawable: exactly the 3 × 3 around the viewer's, whatever the
        // eye's place in it and whatever other keys the terrain holds.
        let keys = [lb(97, 50), lb(102, 52), lb(100, 50)];
        let n = near_field_of(
            NearFieldMode::Original,
            keys.into_iter(),
            |_| true,
            &eye,
            NEAR_FIELD_RING,
            1,
        );
        let s = set(&n);
        assert_eq!(s.len(), 9);
        for x in 99..=101 {
            for y in 49..=51 {
                assert!(s.contains(&(x, y)), "{x},{y}");
            }
        }
        // Landblocks the terrain cannot draw yet stay with the far view.
        let n = near_field_of(
            NearFieldMode::Original,
            std::iter::empty(),
            |k| k != lb(101, 51),
            &eye,
            NEAR_FIELD_RING,
            1,
        );
        assert_eq!(set(&n).len(), 8);
        assert!(!set(&n).contains(&(101, 51)));
        // At the map edge the square is clipped.
        let n = near_field_of(
            NearFieldMode::Original,
            std::iter::empty(),
            |_| true,
            &eye_in(0, 0, 0.5, 0.5),
            NEAR_FIELD_RING,
            1,
        );
        assert_eq!(set(&n), vec![(0, 0), (0, 1), (1, 0), (1, 1)]);
    }

    #[test]
    fn ring_near_field_follows_the_terrain_keys() {
        let eye = eye_in(100, 50, 0.5, 0.5);
        let keys = [lb(100, 50), lb(103, 50), lb(105, 50), lb(100, 47)];
        let other_region = LandblockKey::new(2, 100, 50).unwrap();
        let n = near_field_of(
            NearFieldMode::Ring,
            keys.into_iter().chain([other_region]),
            |k| k != lb(103, 50),
            &eye,
            3.0 * LANDBLOCK_SIZE,
            1,
        );
        // (103, 50) is not drawable, (105, 50) lies beyond the ring, region 2 is skipped.
        assert_eq!(set(&n), vec![(100, 47), (100, 50)]);
    }

    #[test]
    fn near_field_ring_holds_the_3_by_3_from_any_eye() {
        // The farthest square of the 3 × 3 lies 160 √2 from a corner of the viewer's
        // landblock; the ring must cover it.
        let eye = eye_in(100, 50, 0.0, 0.0);
        for x in 99..=101 {
            for y in 49..=51 {
                assert!(lb(x, y).distance_to(eye.xy()) <= NEAR_FIELD_RING);
            }
        }
    }

    #[test]
    fn hand_off_borders() {
        let mut h = FarHandOff::default();
        h.far.insert(lb(10, 10));
        h.covered.insert(lb(10, 10));
        assert!(h.is_far(lb(10, 10)) && h.is_covered(lb(10, 10)));
        // Edge neighbours border the far field; diagonal ones and the far block do not.
        assert!(h.borders_far(lb(9, 10)));
        assert!(h.borders_far(lb(10, 11)));
        assert!(!h.borders_far(lb(9, 9)));
        assert!(!h.borders_far(lb(10, 10)));
        // A block about to be far (batch still building) counts for the borders, not for
        // the cover.
        h.far.insert(lb(20, 20));
        assert!(h.borders_far(lb(21, 20)));
        assert!(!h.is_covered(lb(20, 20)));
    }

    fn declaration(id: u32, v: f32) -> ParamEntry {
        ParamEntry {
            id,
            group: 1000,
            value: ParamValue::Float(FloatCurve::constant(v)),
            flag: 0,
            bindings: Vec::new(),
        }
    }

    #[test]
    fn template_scale_reads_transform_set_0() {
        let mut rec = ShaderRecord {
            did: Did(0x2B00_0001),
            reserved: 0,
            params: vec![
                declaration(PARAM_SCALE_U0, 4.0),
                declaration(PARAM_SCALE_V0, 4.0),
            ],
            permutations: Vec::new(),
        };
        assert_eq!(
            template_scale(&rec, PARAM_SCALE_U0, PARAM_SCALE_V0),
            Some([4.0, 4.0])
        );
        rec.params[1] = declaration(PARAM_SCALE_V0, f32::NAN);
        assert_eq!(template_scale(&rec, PARAM_SCALE_U0, PARAM_SCALE_V0), None);
        rec.params[1] = declaration(PARAM_SCALE_V0, -1.0);
        assert_eq!(template_scale(&rec, PARAM_SCALE_U0, PARAM_SCALE_V0), None);
        rec.params.truncate(1);
        assert_eq!(template_scale(&rec, PARAM_SCALE_U0, PARAM_SCALE_V0), None);
    }

    fn texture_decl(id: u32, did: u32) -> ParamEntry {
        ParamEntry {
            id,
            group: 3000,
            value: ParamValue::Texture(Did(did)),
            flag: 0,
            bindings: Vec::new(),
        }
    }

    /// §6.2: the plain template's detail repeats are set 0; the normal-mapped one samples
    /// the colour list at set 0 and detail and normal at set 1, with its own 3011.
    #[test]
    fn template_defaults_follow_the_template() {
        let rec = ShaderRecord {
            did: Did(0x2B00_0002),
            reserved: 0,
            params: vec![
                declaration(PARAM_SCALE_U0, 1.0),
                declaration(PARAM_SCALE_V0, 1.0),
                declaration(PARAM_SCALE_U1, 4.0),
                declaration(PARAM_SCALE_V1, 4.0),
                texture_decl(PARAM_NORMAL_TEXTURE, 0x4001_7690),
            ],
            permutations: Vec::new(),
        };
        let nm = template_defaults(&rec, Template::NormalMapped);
        assert_eq!(nm.colour_repeats, [1.0, 1.0]);
        assert_eq!(nm.detail_repeats, [4.0, 4.0]);
        assert_eq!(nm.normal_texture, Some(Did(0x4001_7690)));
        let plain = template_defaults(&rec, Template::Plain);
        assert_eq!(plain.detail_repeats, [1.0, 1.0], "set 0 of this record");
        assert_eq!(plain.colour_repeats, [1.0, 1.0]);
        assert_eq!(plain.normal_texture, None);
        let empty = ShaderRecord {
            params: Vec::new(),
            ..rec
        };
        assert_eq!(
            template_defaults(&empty, Template::NormalMapped),
            TemplateDefaults::fallback(Template::NormalMapped)
        );
        // §6.1: the normal-mapped record when wanted and named, else the plain one.
        let r = FarViewRecords {
            plain_template: Some(Did(1)),
            normal_template: Some(Did(2)),
            ..FarViewRecords::default()
        };
        assert_eq!(
            active_template(&r, Template::NormalMapped),
            Some((Did(2), Template::NormalMapped))
        );
        assert_eq!(
            active_template(&r, Template::Plain),
            Some((Did(1), Template::Plain))
        );
        let r = FarViewRecords {
            normal_template: None,
            ..r
        };
        assert_eq!(
            active_template(&r, Template::NormalMapped),
            Some((Did(1), Template::Plain))
        );
    }

    fn far_entry(iid: u64, x: f64, y: f64) -> DistantEntry {
        DistantEntry {
            iid,
            entity_type: 7,
            object: Did(0x4700_0001),
            scale: 2.0,
            position: WorldPos::new(1, x, y, 30.0),
            // 90° about z, stored (w, x, y, z).
            orientation: Some([
                std::f32::consts::FRAC_1_SQRT_2,
                0.0,
                0.0,
                std::f32::consts::FRAC_1_SQRT_2,
            ]),
            view_distance_sq: 1.0e6,
            sector: tessera_world::far_view::SectorKey::new(1, 33, 16).unwrap(),
            content_layers: None,
        }
    }

    fn visual(parts: &[u32]) -> FarVisual {
        FarVisual {
            parts: parts.iter().map(|&p| Did(p)).collect(),
            lod: LodValues::default(),
            depth_bias: DrawDepthBias::NONE,
        }
    }

    /// §10: far entities become objects of their landblock in its frame, one instance per
    /// part, placed by Position, orientation and scale.
    #[test]
    fn far_entities_become_landblock_objects() {
        let key = lb(100, 50);
        let o = key.origin();
        let a = far_entry(1, o[0] + 10.0, o[1] + 20.0);
        let mut b = far_entry(2, o[0] + 150.0, o[1] + 5.0);
        b.scale = f32::NAN;
        b.orientation = None;
        let mut bad = far_entry(3, f64::NAN, 0.0);
        bad.scale = 1.0;
        let (va, vb) = (visual(&[0x0600_0001, 0x0600_0002]), visual(&[0x0600_0003]));
        let (instances, objects) = far_block(key, &[(&a, &va), (&b, &vb), (&bad, &vb)]);
        assert_eq!(objects.len(), 2, "the non-finite entry is left out");
        assert_eq!(instances.len(), 3);
        assert_eq!((objects[0].first, objects[0].count), (0, 2));
        assert_eq!((objects[1].first, objects[1].count), (2, 1));
        assert_eq!(objects[0].position, Vec3::new(10.0, 20.0, 30.0));
        assert_eq!(objects[0].cell, OUTDOORS);
        assert!(!objects[0].generated);
        // Part space x maps to block y (90° about z), scaled 2, moved to the position.
        let p = instances[0].model.transform_point3(Vec3::X);
        assert!((p - Vec3::new(10.0, 22.0, 30.0)).length() < 1e-4, "{p}");
        assert_eq!(instances[1].object, 0);
        assert_eq!(instances[2].object, 1);
        assert_eq!(instances[2].scale, 1.0, "a non-finite scale counts as 1");
        assert!(instances.iter().all(|i| i.figure.is_none()));
    }

    /// §10 step 3 through the object layer's far blocks: created entities are inserted per
    /// landblock, removed ones taken out, whether the layer holds the landblock's own
    /// objects or not (it hands over by distance), and only changed landblocks are touched.
    #[test]
    fn far_blocks_follow_create_and_remove() {
        use tessera_world::far_view::DistantSet;
        let k0 = lb(100, 50);
        let k1 = lb(104, 50);
        let o0 = k0.origin();
        let o1 = k1.origin();
        let mut set = DistantSet::default();
        set.add_sector([
            far_entry(1, o0[0] + 10.0, o0[1] + 10.0),
            far_entry(2, o0[0] + 20.0, o0[1] + 10.0),
            far_entry(3, o1[0] + 10.0, o1[1] + 10.0),
        ]);
        let mut groups = DistantGroups::default();
        let viewer = WorldPos::new(1, o0[0], o0[1], 30.0);
        groups.apply(&set.walk(
            &viewer,
            VIEWER_INSTANCE,
            DISTANT_FACTOR,
            |_| false,
            |_| true,
        ));
        let drawn = |i: u64| i != 2;
        let settled = |_: LandblockKey| false;
        let mut inserted: HashMap<LandblockKey, Vec<u64>> = HashMap::new();
        let wanted = wanted_far_blocks(&groups, drawn);
        let (insert, remove) = far_block_changes(&wanted, &inserted, settled);
        assert_eq!(insert, vec![(k0, vec![1]), (k1, vec![3])], "2 is not drawn");
        assert!(remove.is_empty());
        inserted.extend(insert);
        // Nothing changed: nothing to do.
        let (insert, remove) =
            far_block_changes(&wanted_far_blocks(&groups, drawn), &inserted, settled);
        assert!(insert.is_empty() && remove.is_empty());
        // While a landblock's entry is being resolved, its far block is left as it is
        // (inserted once the batch is complete, not once per answer).
        let all = |_: u64| true;
        let (insert, remove) =
            far_block_changes(&wanted_far_blocks(&groups, all), &inserted, |k| k == k0);
        assert!(insert.is_empty() && remove.is_empty());
        let (insert, _) = far_block_changes(&wanted_far_blocks(&groups, all), &inserted, settled);
        assert_eq!(insert, vec![(k0, vec![1, 2])]);
        // The viewer leaves (s small): everything is removed, settling or not.
        groups.apply(&set.walk(&viewer, VIEWER_INSTANCE, 1.0e-9, |_| false, |_| true));
        let (insert, remove) =
            far_block_changes(&wanted_far_blocks(&groups, drawn), &inserted, |_| true);
        assert!(insert.is_empty());
        assert_eq!(remove, vec![k0, k1]);
        // An instance change recreates (and so re-inserts) what is in range.
        groups.apply(&set.walk(
            &viewer,
            VIEWER_INSTANCE,
            DISTANT_FACTOR,
            |_| false,
            |_| true,
        ));
        let c = set.walk(
            &viewer,
            VIEWER_INSTANCE + 1,
            DISTANT_FACTOR,
            |_| false,
            |_| true,
        );
        assert_eq!(c.destroy.len(), 3);
        assert_eq!(c.create.len(), 3);
        // The far-entity state resolves each created entry once, until it is destroyed.
        let mut far = FarEntities::default();
        let jobs = far.apply(&c);
        assert_eq!(jobs.len(), 3);
        assert_eq!(far.pending(), 3);
        far.resolved(1, FarVisualResult::Drawn(Arc::new(visual(&[0x0600_0001]))));
        far.resolved(2, FarVisualResult::NotDrawn);
        far.resolved(3, FarVisualResult::Retry);
        assert_eq!(far.pending(), 1, "3 waits for its retry");
        assert!(far.is_pending(3) && !far.is_pending(1));
        assert!(far.drawn(1).is_some() && far.drawn(2).is_none());
        far.resolved(99, FarVisualResult::NotDrawn);
        assert!(
            !far.visuals.contains_key(&99),
            "an answer for a removed entity is ignored"
        );
    }

    /// A far entity whose object cannot be read now is retried at most
    /// [`FAR_MAX_ATTEMPTS`] times, then created but not drawn, so a lasting read error does
    /// not keep the far view unsettled.
    #[test]
    fn far_entity_retries_are_capped() {
        use tessera_world::far_view::DistantSet;
        let o = lb(100, 50).origin();
        let mut set = DistantSet::default();
        set.add_sector([far_entry(7, o[0] + 10.0, o[1] + 10.0)]);
        let viewer = WorldPos::new(1, o[0], o[1], 30.0);
        let mut far = FarEntities::default();
        let c = set.walk(
            &viewer,
            VIEWER_INSTANCE,
            DISTANT_FACTOR,
            |_| false,
            |_| true,
        );
        assert_eq!(far.apply(&c).len(), 1);
        for attempt in 1..FAR_MAX_ATTEMPTS {
            far.resolved(7, FarVisualResult::Retry);
            assert_eq!(far.pending(), 1, "attempt {attempt}");
            // The retry falls due: the entity is asked for again.
            if let Some(VisualState::RetryAt(at, _)) = far.visuals.get_mut(&7) {
                *at = std::time::Instant::now();
            }
            assert_eq!(far.apply(&DistantChanges::default()).len(), 1);
        }
        far.resolved(7, FarVisualResult::Retry);
        assert_eq!(
            far.pending(),
            0,
            "given up after {FAR_MAX_ATTEMPTS} attempts"
        );
        assert!(far.drawn(7).is_none());
        assert!(
            far.apply(&DistantChanges::default()).is_empty(),
            "no more retries"
        );
        // Stale answers past the cap leave it settled.
        far.resolved(7, FarVisualResult::Retry);
        far.resolved(7, FarVisualResult::Retry);
        assert_eq!(far.pending(), 0);
    }

    /// §6.5: the seam overlay needs `NearFarSeamBlending`, the normal-mapped template wanted
    /// (`FarLandscapeNormalMaps` and graphics core 2 or 5) and read.
    #[test]
    fn seam_overlay_gate() {
        let normal = TemplateDefaults::fallback(Template::NormalMapped);
        let plain = TemplateDefaults::fallback(Template::Plain);
        let mut s = FarViewSettings::default();
        assert!(!seam_enabled(&s, &normal), "built-in settings: off");
        s.normal_maps = true;
        s.seam_blending = true;
        s.graphics_core = 0;
        assert!(!seam_enabled(&s, &normal), "graphics core not 2 or 5");
        for core in [2, 5] {
            s.graphics_core = core;
            assert!(seam_enabled(&s, &normal));
            assert!(
                !seam_enabled(&s, &plain),
                "only the plain template was read"
            );
        }
        s.seam_blending = false;
        assert!(!seam_enabled(&s, &normal), "the switch off");
        s.seam_blending = true;
        s.normal_maps = false;
        assert!(!seam_enabled(&s, &normal), "the plain template wanted");
    }

    /// Real data (`far-view.md` §10; read-only, skipped unless `TESSERA_GAME_DIR` is set):
    /// at the walker's default spawn with the built-in settings, the far view's sectors give
    /// a distant set whose walk creates far entities, their objects resolve to drawable
    /// parts on the far view's own path ([`far_visual`], [`wanted_far_blocks`],
    /// [`far_block`]), and at least one long-range landmark lies beyond the 3 × 3 near field
    /// and beyond the 320-unit cap of the landblocks' own objects (so the object layer
    /// draws it as a far entity whether or not it holds its landblock) while inside its
    /// draw distance min(*G*, `d0` × *M_d*) under the far-entity cap. Only counts are
    /// printed. The GPU side (the object layer drawing them) is covered by the headless
    /// walker runs, not here.
    #[test]
    fn far_entities_at_the_default_spawn_real_data() {
        use tessera_world::BundleOptions;
        let Some(dir) = std::env::var_os("TESSERA_GAME_DIR").filter(|d| !d.is_empty()) else {
            eprintln!("TESSERA_GAME_DIR not set; skipping");
            return;
        };
        let set = Arc::new(
            tessera_dat::ArchiveSet::open(
                std::path::Path::new(&dir),
                tessera_dat::SetOptions::default(),
            )
            .expect("open the archive set"),
        );
        let ctx = WorldContext::from_install(set, BundleOptions::default()).expect("world");
        let settings = FarViewSettings::default();
        let mut state = FarViewState::new(settings, ctx.pids().landblock.entity_content_layers);
        // `walker_host::DEFAULT_SPAWN`: region 1, landblock (126, 140), its centre.
        let spawn = LandblockKey::new(1, 126, 140).expect("spawn landblock");
        let viewer = Viewer {
            landblock: spawn,
            instance: VIEWER_INSTANCE,
        };
        let distances = DistanceSettings::default();
        let view = sprite_view(720, 0.8, &distances);
        for _ in 0..3 {
            let w = state.update(Some(viewer), NearField::Radius(1), view);
            for k in &w.load_sectors {
                state.sector_loaded(*k, read_sector(&**ctx.source(), *k, ctx.schema()));
            }
        }
        let o = spawn.origin();
        let eye = WorldPos::new(
            1,
            o[0] + 80.0,
            o[1] + 80.0,
            f64::from(state.near_floor().unwrap_or(0.0)) + 2.0,
        );
        let active = ctx.options().active_content_layers.clone();
        let changes = state.distant_mut().walk(
            &eye,
            VIEWER_INSTANCE,
            DISTANT_FACTOR,
            |_| false,
            |e| {
                tessera_physics::geometry::content_layers_pass(
                    e.content_layers.as_deref(),
                    &active,
                    0,
                )
            },
        );
        let mut far = FarEntities::default();
        for e in far.apply(&changes) {
            far.resolved(e.iid, far_visual(&ctx, &e));
        }
        let wanted = wanted_far_blocks(&far.groups, |i| far.drawn(i).is_some());
        let drawn: usize = wanted.values().map(Vec::len).sum();
        let mut objects = 0usize;
        for (k, ids) in &wanted {
            let members: Vec<(&DistantEntry, &FarVisual)> = ids
                .iter()
                .filter_map(|i| Some((far.entries.get(i)?, &**far.drawn(*i)?)))
                .collect();
            objects += far_block(*k, &members).1.len();
        }
        // Landmarks beyond the 3 × 3 near field and the 320-unit main cap, inside their draw
        // distance under the far-entity cap (G = E, no GPU-budget shrink).
        let g = settings.extent();
        let cap = super::super::objects::far_entity_cap(g, 320.0, 320.0);
        let landmarks = far
            .entries
            .values()
            .filter_map(|e| Some((e, far.drawn(e.iid)?)))
            .filter(|(e, v)| {
                let beyond_near = e.position.landblock().is_some_and(|k| {
                    k.region() != spawn.region()
                        || k.x().abs_diff(spawn.x()) > 1
                        || k.y().abs_diff(spawn.y()) > 1
                });
                #[allow(clippy::cast_possible_truncation)]
                let d = e.distance_sq(&eye).sqrt() as f32;
                let reach = v.lod.draw_distance(g, &distances).min(cap);
                beyond_near && d > 320.0 && d <= reach
            })
            .count();
        eprintln!(
            "default spawn: {} distant entries, {} created, {} drawable in {} landblocks ({} objects), {} landmarks beyond the near field within their draw distance",
            state.distant().len(),
            changes.create.len(),
            drawn,
            wanted.len(),
            objects,
            landmarks
        );
        assert!(!changes.create.is_empty(), "no far entity created");
        assert!(drawn > 0, "no far entity resolves to something drawn");
        assert!(
            objects > 0 && objects <= drawn,
            "drawable far entities become objects"
        );
        assert!(
            landmarks > 0,
            "no long-range landmark beyond the 3 × 3 is within reach"
        );
        assert_eq!(far.pending(), 0);
    }

    #[test]
    fn renderer_inputs() {
        let d = DistanceSettings::default();
        let v = sprite_view(0, 0.8, &d);
        assert_eq!(v.target_height, 1.0);
        assert_eq!(v.fov_y, 0.8);
        assert_eq!(v.object_scale, d.object_multiplier());
        let s = FarViewSettings::default();
        let f = fade_params(&s);
        // FD1 at the built-in level: s = E − 160, r = 1/160.
        assert_eq!(f.start, s.extent() - 160.0);
        assert_eq!(f.inv_range, 1.0 / 160.0);
        let sprite = FarSprite {
            position: [1.0, 2.0, 3.0],
            size: 4.0,
            base: 0.5,
            u: [0.1, 0.2],
            v: [0.3, 0.4],
            distance: 140.0,
            mirror: true,
        };
        let out = distant_sprites(&[sprite]);
        assert_eq!(out.len(), 1);
        let o = &out[0];
        assert_eq!(
            (o.position, o.size, o.base, o.u, o.v, o.distance, o.mirror),
            (
                sprite.position,
                sprite.size,
                sprite.base,
                sprite.u,
                sprite.v,
                sprite.distance,
                sprite.mirror
            )
        );
    }
}
