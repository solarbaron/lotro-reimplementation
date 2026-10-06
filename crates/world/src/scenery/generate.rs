// SPDX-License-Identifier: GPL-3.0-or-later
//! Generating the procedural scenery of one landblock (`scenery.md` §7) and the frill pass
//! of one vertex (§8.2).
//!
//! Generation is a pure function of the landblock's data, its region's tables, the scene
//! records, its terrain collision mesh, its water surface and its static list: the same
//! inputs give bit-identical output (SC8). It never fails; hostile tables are cut by the
//! limits of [`SceneryLimits`] (scene-reference depth and cycles, work and object budgets),
//! which the statistics report.

use tessera_formats::LandblockData;
use tessera_formats::scenery::{PlacementNode, SCALE_MODE_UNIFORM, SceneChild};
use tessera_formats::setup::Quaternion;
use tessera_formats::{Did, landblock::COARSE_SIDE};
use tessera_physics::geometry::{GeneratedObject, GridMesh, Plane, Transform};

use super::clearance::StaticClearance;
use super::hash::{self, FRILL_LAYER_SALT};
use super::math::{self, EPS};
use super::tables::{SceneSource, SceneryTables};
use crate::coords::LandblockKey;

/// Landblock edge length in units.
const BLOCK: f32 = 160.0;
/// Scenery vertex spacing minus the 0.0002 that keeps vertex 16 inside the block (§7.3).
const VERTEX_STEP: f32 = 10.0 - 0.0002;
/// Surface quads per side.
const QUADS: i32 = 32;
/// Largest scene-reference path (Tessera Decision of §13.1: 16; the install's maximum is 5).
pub const DEFAULT_MAX_SCENE_DEPTH: usize = 16;
/// Hard upper bound of [`SceneryLimits::max_scene_depth`]: larger values are clamped to it.
pub const MAX_SCENE_DEPTH_LIMIT: usize = 32;
/// Most nested calls (scene instantiations and placement nodes together) on one
/// generation path, whatever the limits say. Generation recurses once per scene and once
/// per nested node; this bound keeps the stack use of one landblock finite on hostile
/// scenes (decoded nodes nest at most 16 deep, but [`SceneSource`] implementations may hand
/// out deeper ones). The default limits need at most 16 × (16 + 1) = 272. A path cut here
/// is counted in [`SceneryStats::depth_capped`].
///
/// Stack: a path of this depth fits in the 2 MiB that `std` gives a spawned thread, in
/// debug builds too (`scenery::tests::deep_recursion_fits_a_small_stack`).
pub const MAX_RECURSION_DEPTH: usize = 17 * MAX_SCENE_DEPTH_LIMIT;
/// Work units charged per this many static-list items (entities plus volumes) scanned by
/// one step-14a test.
const STATIC_ITEMS_PER_WORK_UNIT: u32 = 16;

/// The per-landblock water surface generation reads (`scenery.md` §11, `water.md`
/// §6.2–§6.3). The world's water module provides the real one; tests use [`NoWater`] or a
/// fixed plane.
pub trait SceneryWater: Send + Sync {
    /// The landblock's water plane (unit normal, offset; n · p + d = 0), or `None` when the
    /// landblock has no water.
    fn water_plane(&self, key: LandblockKey) -> Option<Plane>;
}

/// No water anywhere.
#[derive(Debug, Clone, Copy, Default)]
pub struct NoWater;

impl SceneryWater for NoWater {
    fn water_plane(&self, _key: LandblockKey) -> Option<Plane> {
        None
    }
}

/// The same water plane (or none) for every landblock.
#[derive(Debug, Clone, Copy, Default)]
pub struct FixedWater(pub Option<Plane>);

impl SceneryWater for FixedWater {
    fn water_plane(&self, _key: LandblockKey) -> Option<Plane> {
        self.0
    }
}

impl<F: Fn(LandblockKey) -> Option<Plane> + Send + Sync> SceneryWater for F {
    fn water_plane(&self, key: LandblockKey) -> Option<Plane> {
        self(key)
    }
}

/// The horizontal water plane at height `h` (`water.md` §6.3: normal (0, 0, 1), offset
/// −h).
#[must_use]
pub fn level_plane(h: f32) -> Plane {
    Plane {
        normal: [0.0, 0.0, 1.0],
        d: -h,
    }
}

/// Bounds that keep generation finite on hostile tables (`scenery.md` §13.1). The
/// defaults are far above anything the install needs; a run that hits one reports it in
/// [`SceneryStats`].
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct SceneryLimits {
    /// Most scenes on one instantiation path, the vertex scene included. Values above
    /// [`MAX_SCENE_DEPTH_LIMIT`] are clamped to it; independently, a path never holds more
    /// than [`MAX_RECURSION_DEPTH`] scenes and nodes together, so the stack stays bounded.
    pub max_scene_depth: usize,
    /// Work units per vertex (one per child or node visited, plus terrain-triangle work).
    pub max_work_per_vertex: u32,
    /// Work units per landblock.
    pub max_work_per_landblock: u32,
    /// Objects (or frill records) per vertex.
    pub max_objects_per_vertex: u32,
    /// Objects per landblock.
    pub max_objects_per_landblock: u32,
    /// Footprint markers per landblock.
    pub max_markers: u32,
}

impl Default for SceneryLimits {
    fn default() -> Self {
        Self {
            max_scene_depth: DEFAULT_MAX_SCENE_DEPTH,
            max_work_per_vertex: 1 << 18,
            max_work_per_landblock: 1 << 23,
            max_objects_per_vertex: 1 << 14,
            max_objects_per_landblock: 1 << 18,
            max_markers: 1 << 18,
        }
    }
}

/// Everything one landblock's generation reads.
#[derive(Clone, Copy)]
pub struct SceneryInput<'a> {
    /// The landblock (seed A, global vertex coordinates).
    pub key: LandblockKey,
    /// Its data record: coarse grid (§3), heights (§7.3), vertex flags (§7.4 step 8) and
    /// quad grid (§7.4 step 7).
    pub data: &'a LandblockData,
    /// Its terrain collision mesh (`collision.md` §6.1), built from `data`.
    pub terrain: &'a GridMesh,
    /// Its region's tables.
    pub tables: &'a SceneryTables,
    /// Scene records.
    pub scenes: &'a dyn SceneSource,
    /// Water surfaces.
    pub water: &'a dyn SceneryWater,
    /// The static list (§7.4 step 14a).
    pub statics: &'a [StaticClearance],
    /// Bounds.
    pub limits: SceneryLimits,
}

impl std::fmt::Debug for SceneryInput<'_> {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("SceneryInput")
            .field("key", &self.key)
            .field("region", &self.tables.region)
            .field("statics", &self.statics.len())
            .field("limits", &self.limits)
            .finish_non_exhaustive()
    }
}

/// One generated object (`scenery.md` §7.6): a 0x47 entity descriptor and its frame in
/// the landblock frame.
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct SceneryObject {
    /// The 0x47 object.
    pub object: Did,
    /// Position (block-local x, y; absolute z).
    pub position: [f32; 3],
    /// Unit rotation, (w, x, y, z), active from model to landblock frame.
    pub rotation: Quaternion,
    /// Per-axis scale (uniform in the install).
    pub scale: [f32; 3],
    /// The coarse vertex (i, j) that generated it.
    pub vertex: [u8; 2],
}

impl SceneryObject {
    /// The object as `tessera-physics` takes it for collision (`collision.md` §9.4).
    #[must_use]
    pub fn to_generated(&self) -> GeneratedObject {
        GeneratedObject {
            object: self.object,
            transform: Transform {
                position: self.position,
                rotation: self.rotation,
                scale: self.scale,
            },
        }
    }
}

/// One frill record of the frill pass (§8.1–§8.2), before the landblock and density tests.
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct FrillRecord {
    /// The object, its frame and scale, as for [`SceneryObject`].
    pub object: SceneryObject,
    /// Created after a frill layer finished rather than inside one (§8.1 source (b));
    /// Tessera discards these (§8.6).
    pub late: bool,
}

/// Why a placement node placed nothing. The discriminants index
/// [`SceneryStats::rejected`]; [`Rejection::ALL`] lists every variant in that order.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
#[non_exhaustive]
pub enum Rejection {
    /// Step 1: the frequency draw (not an error).
    Frequency,
    /// A `scale_mode` other than 4 (unobserved; unsupported).
    ScaleMode,
    /// Step 6: outside the block.
    Outside,
    /// Step 7: a surface quad forbids procedural content.
    TerrainType,
    /// Step 9: slope.
    Slope,
    /// Step 11: `height_mode` 1 without water.
    NoWater,
    /// Step 12 (§7.5): height ranges.
    HeightRange,
    /// Step 13: terrain clearance.
    Clearance,
    /// Step 14a: a static entity's clearance volume.
    StaticSpacing,
    /// Step 14b: a placed node's footprint.
    Footprint,
}

impl Rejection {
    /// Every reason, in the order of [`SceneryStats::rejected`].
    pub const ALL: [Self; 10] = [
        Self::Frequency,
        Self::ScaleMode,
        Self::Outside,
        Self::TerrainType,
        Self::Slope,
        Self::NoWater,
        Self::HeightRange,
        Self::Clearance,
        Self::StaticSpacing,
        Self::Footprint,
    ];
    /// Number of reasons (the length of [`SceneryStats::rejected`]).
    pub const COUNT: usize = Self::ALL.len();

    /// The reason's index in [`SceneryStats::rejected`].
    #[must_use]
    pub const fn index(self) -> usize {
        self as usize
    }
}

/// Counters of one generation run.
#[derive(Debug, Clone, Default, PartialEq, Eq)]
pub struct SceneryStats {
    /// Coarse vertices whose word resolved to a scene.
    pub vertices_with_scene: u32,
    /// Coarse vertices whose terrain entry had no pair for the slot (§4.2 step 2).
    pub vertices_without_pair: u32,
    /// Scene keys missing from the scene-choice table.
    pub missing_keys: u32,
    /// Scene DIDs that could not be read.
    pub missing_scenes: u32,
    /// Placement nodes tested.
    pub nodes_tested: u32,
    /// Placement nodes placed.
    pub nodes_placed: u32,
    /// Rejections by reason, indexed by [`Rejection::index`] (the order of
    /// [`Rejection::ALL`]).
    pub rejected: [u32; Rejection::COUNT],
    /// Objects created (static pass) or frill records made (frill pass).
    pub objects: u32,
    /// Objects skipped by the frill pass because no frill layer had been met.
    pub frill_skipped: u32,
    /// Scene references refused because the scene was already on the path (a cycle).
    pub cycles: u32,
    /// Scene references refused at the depth cap.
    pub depth_capped: u32,
    /// Vertices cut short by the work or object budget.
    pub budget_cut: u32,
    /// Scene reads that failed for a reason that is not the data's (see
    /// [`SceneSource::scene`]); the scene placed nothing in this run only. Non-zero means
    /// the result is incomplete ([`SceneryStats::is_complete`]) and should be rebuilt.
    pub scenes_unavailable: u32,
}

impl SceneryStats {
    /// Rejections for `reason`.
    #[must_use]
    pub fn rejections(&self, reason: Rejection) -> u32 {
        self.rejected.get(reason.index()).copied().unwrap_or(0)
    }

    /// Whether any hostile-input bound was hit.
    #[must_use]
    pub fn truncated(&self) -> bool {
        self.cycles > 0 || self.depth_capped > 0 || self.budget_cut > 0
    }

    /// Whether every scene could be read (no [`SceneryStats::scenes_unavailable`]): the
    /// result is then the deterministic one (SC8).
    #[must_use]
    pub fn is_complete(&self) -> bool {
        self.scenes_unavailable == 0
    }

    /// Adds `o` to these counters, saturating at `u32::MAX`.
    fn add(&mut self, o: &Self) {
        fn sat(a: &mut u32, b: u32) {
            *a = a.saturating_add(b);
        }
        sat(&mut self.vertices_with_scene, o.vertices_with_scene);
        sat(&mut self.vertices_without_pair, o.vertices_without_pair);
        sat(&mut self.missing_keys, o.missing_keys);
        sat(&mut self.missing_scenes, o.missing_scenes);
        sat(&mut self.nodes_tested, o.nodes_tested);
        sat(&mut self.nodes_placed, o.nodes_placed);
        for (a, b) in self.rejected.iter_mut().zip(o.rejected) {
            sat(a, b);
        }
        sat(&mut self.objects, o.objects);
        sat(&mut self.frill_skipped, o.frill_skipped);
        sat(&mut self.cycles, o.cycles);
        sat(&mut self.depth_capped, o.depth_capped);
        sat(&mut self.budget_cut, o.budget_cut);
        sat(&mut self.scenes_unavailable, o.scenes_unavailable);
    }
}

/// The generated scenery of one landblock.
#[derive(Debug, Clone, Default, PartialEq)]
pub struct GeneratedScenery {
    /// Objects in generation order (the order of the client's generated list).
    pub objects: Vec<SceneryObject>,
    /// Counters.
    pub stats: SceneryStats,
}

/// Generates the scenery of one landblock: the vertex pass of §7.2–§7.6 over all 17 × 17
/// coarse vertices, i outer and j inner. The encounter pass (§7.7) is not run: no
/// landblock of the install uses it.
#[must_use]
pub fn generate_landblock(input: &SceneryInput<'_>) -> GeneratedScenery {
    let mut out = GeneratedScenery::default();
    let mut g = Gen::new(input, Pass::Static);
    let mut work_left = input.limits.max_work_per_landblock;
    for i in 0..COARSE_SIDE {
        for j in 0..COARSE_SIDE {
            if work_left == 0 {
                g.stats.budget_cut += 1;
                continue;
            }
            g.begin_vertex(i, j, work_left);
            g.run_vertex(i, j);
            work_left = work_left.saturating_sub(g.work);
        }
    }
    out.stats = g.stats;
    out.objects = g.objects;
    out
}

/// The frill pass of coarse vertex (i, j), 0 ≤ i, j ≤ 16 (§8.2): every object met while
/// the vertex's *under a frill layer* state is set becomes a [`FrillRecord`]. Spacing is
/// tested against the static list and the markers of this vertex's frill pass only, which
/// are discarded afterwards (Tessera Decision of the §8.2 OPEN). Records are returned
/// before the landblock and density tests, late ones (§8.6) included.
#[must_use]
pub fn frill_vertex(
    input: &SceneryInput<'_>,
    i: usize,
    j: usize,
) -> (Vec<FrillRecord>, SceneryStats) {
    if i >= COARSE_SIDE || j >= COARSE_SIDE {
        return (Vec::new(), SceneryStats::default());
    }
    let mut g = Gen::new(input, Pass::Frill);
    g.begin_vertex(i, j, input.limits.max_work_per_vertex);
    g.run_vertex(i, j);
    (g.frills, g.stats)
}

/// Statistics of several runs added up (each counter saturates at `u32::MAX`).
#[must_use]
pub fn sum_stats<'s>(stats: impl IntoIterator<Item = &'s SceneryStats>) -> SceneryStats {
    let mut out = SceneryStats::default();
    for s in stats {
        out.add(s);
    }
    out
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum Pass {
    Static,
    Frill,
}

#[derive(Debug, Clone, Copy)]
struct Frame {
    pos: [f32; 3],
    rot: Quaternion,
}

#[derive(Debug, Clone, Copy, Default)]
struct Switches {
    no_spacing: bool,
    no_checks: bool,
}

#[derive(Debug, Clone, Copy)]
struct Marker {
    pos: [f32; 3],
    radius: f32,
    spacing_3d: bool,
    open: bool,
}

struct Gen<'a, 'i> {
    input: &'a SceneryInput<'i>,
    pass: Pass,
    water: Option<Plane>,
    a: u32,
    b: u32,
    vertex: [u8; 2],
    under_frill: bool,
    frill_depth: u32,
    markers: Vec<Marker>,
    path: Vec<Did>,
    /// Nested scenes and nodes on the current path ([`MAX_RECURSION_DEPTH`]).
    depth: usize,
    /// Scene-depth limit, clamped to [`MAX_SCENE_DEPTH_LIMIT`].
    max_scene_depth: usize,
    /// Work units of one step-14a scan of the static list.
    static_cost: u32,
    work: u32,
    work_cap: u32,
    vertex_objects: u32,
    cut: bool,
    objects: Vec<SceneryObject>,
    frills: Vec<FrillRecord>,
    stats: SceneryStats,
}

impl<'a, 'i> Gen<'a, 'i> {
    fn new(input: &'a SceneryInput<'i>, pass: Pass) -> Self {
        let key = input.key;
        Self {
            input,
            pass,
            water: input.water.water_plane(key),
            a: hash::seed_a(key.region(), key.x(), key.y()),
            b: 0,
            vertex: [0; 2],
            under_frill: false,
            frill_depth: 0,
            markers: Vec::new(),
            path: Vec::new(),
            depth: 0,
            max_scene_depth: input.limits.max_scene_depth.min(MAX_SCENE_DEPTH_LIMIT),
            static_cost: static_cost(input.statics),
            work: 0,
            work_cap: 0,
            vertex_objects: 0,
            cut: false,
            objects: Vec::new(),
            frills: Vec::new(),
            stats: SceneryStats::default(),
        }
    }

    fn begin_vertex(&mut self, i: usize, j: usize, work_left: u32) {
        #[allow(clippy::cast_possible_truncation)]
        let (iu, ju) = (i as u32, j as u32);
        self.b = hash::seed_b(iu, ju);
        #[allow(clippy::cast_possible_truncation)]
        {
            self.vertex = [i as u8, j as u8];
        }
        self.under_frill = false;
        self.frill_depth = 0;
        self.path.clear();
        self.depth = 0;
        self.work = 0;
        self.work_cap = self.input.limits.max_work_per_vertex.min(work_left);
        self.vertex_objects = 0;
        self.cut = false;
        if self.pass == Pass::Frill {
            self.markers.clear();
        }
    }

    /// §7.2 and §7.3 for one vertex.
    fn run_vertex(&mut self, i: usize, j: usize) {
        let input = self.input;
        let Some(word) = input.data.coarse_word(i, j) else {
            return;
        };
        let scene = match input.tables.lookup(word) {
            super::tables::VertexLookup::NoPair => {
                self.stats.vertices_without_pair += 1;
                return;
            }
            super::tables::VertexLookup::KeyZero => return,
            super::tables::VertexLookup::MissingKey(_) => {
                self.stats.missing_keys += 1;
                return;
            }
            super::tables::VertexLookup::Entry(entry) => {
                let gx = 16 * u32::from(input.key.x()) + self.b_i();
                let gy = 16 * u32::from(input.key.y()) + self.b_j();
                match entry.choose(hash::vertex_fraction(gx, gy)) {
                    Some(c) => c.scene,
                    None => return,
                }
            }
        };
        self.stats.vertices_with_scene += 1;
        #[allow(clippy::cast_precision_loss)]
        let base = Frame {
            pos: [
                i as f32 * VERTEX_STEP,
                j as f32 * VERTEX_STEP,
                input.data.heights.height(2 * i, 2 * j).unwrap_or(0.0),
            ],
            rot: Quaternion::IDENTITY,
        };
        self.instantiate(scene, base, [1.0; 3], hash::SCENE_SEED, Switches::default());
        if self.cut {
            self.stats.budget_cut += 1;
        }
    }

    fn b_i(&self) -> u32 {
        u32::from(self.vertex[0])
    }

    fn b_j(&self) -> u32 {
        u32::from(self.vertex[1])
    }

    /// Charges `units` of work; false once the budget is spent.
    fn charge(&mut self, units: u32) -> bool {
        if self.cut {
            return false;
        }
        self.work = self.work.saturating_add(units);
        if self.work > self.work_cap {
            self.cut = true;
            return false;
        }
        true
    }

    /// Enters one level of recursion; false (counted as depth-capped) at
    /// [`MAX_RECURSION_DEPTH`].
    fn enter(&mut self) -> bool {
        if self.depth >= MAX_RECURSION_DEPTH {
            self.stats.depth_capped += 1;
            return false;
        }
        self.depth += 1;
        true
    }

    /// Instantiates a scene (§7.3, and §8.2 items 1–3 in the frill pass). Returns the
    /// number of objects created directly in its group (for `max_objects`, §7.4 step 15).
    fn instantiate(
        &mut self,
        did: Did,
        frame: Frame,
        scale: [f32; 3],
        c: u32,
        sw: Switches,
    ) -> u32 {
        if !self.enter() {
            return 0;
        }
        let created = self.instantiate_inner(did, frame, scale, c, sw);
        self.depth -= 1;
        created
    }

    fn instantiate_inner(
        &mut self,
        did: Did,
        frame: Frame,
        scale: [f32; 3],
        c: u32,
        sw: Switches,
    ) -> u32 {
        if !self.charge(1) {
            return 0;
        }
        if self.path.contains(&did) {
            self.stats.cycles += 1;
            return 0;
        }
        if self.path.len() >= self.max_scene_depth {
            self.stats.depth_capped += 1;
            return 0;
        }
        let scene = match self.input.scenes.scene(did) {
            Ok(Some(scene)) => scene,
            Ok(None) => {
                self.stats.missing_scenes += 1;
                return 0;
            }
            Err(e) => {
                tracing::debug!(scene = %format_args!("{:#010x}", did.0), error = %e, "scene unavailable; result incomplete");
                self.stats.scenes_unavailable += 1;
                return 0;
            }
        };
        self.path.push(did);
        let s = hash::scene_seed(c);
        let mut created = 0;
        if let Some(group) = scene.placement_group() {
            for (k, child) in group.children.iter().enumerate() {
                if !self.charge(1) {
                    break;
                }
                #[allow(clippy::cast_possible_truncation)]
                let ck = hash::child_seed(s, k as u32);
                match child {
                    SceneChild::Node(n) => self.node(n, frame, scale, ck, sw),
                    SceneChild::Scene(d) => {
                        self.instantiate(*d, frame, scale, ck, sw);
                    }
                    SceneChild::Object(d) => {
                        if self.create(*d, frame, scale) {
                            created += 1;
                        }
                    }
                }
            }
        }
        if self.pass == Pass::Frill
            && let Some(layer) = scene.frill_layer.as_ref()
            && !self.cut
        {
            self.under_frill = true;
            self.frill_depth += 1;
            self.node(layer, frame, scale, s.wrapping_add(FRILL_LAYER_SALT), sw);
            self.frill_depth -= 1;
        }
        self.path.pop();
        created
    }

    /// Creates an object (§7.6), or a frill record in the frill pass (§8.2). Returns
    /// whether something was created.
    fn create(&mut self, did: Did, frame: Frame, scale: [f32; 3]) -> bool {
        let limits = self.input.limits;
        if self.vertex_objects >= limits.max_objects_per_vertex {
            self.cut = true;
            return false;
        }
        let object = SceneryObject {
            object: did,
            position: frame.pos,
            rotation: frame.rot,
            scale,
            vertex: self.vertex,
        };
        match self.pass {
            Pass::Static => {
                if self.objects.len() >= limits.max_objects_per_landblock as usize {
                    self.cut = true;
                    return false;
                }
                self.objects.push(object);
            }
            Pass::Frill => {
                if !self.under_frill {
                    self.stats.frill_skipped += 1;
                    return false;
                }
                self.frills.push(FrillRecord {
                    object,
                    late: self.frill_depth == 0,
                });
            }
        }
        self.vertex_objects += 1;
        self.stats.objects += 1;
        true
    }

    fn reject(&mut self, why: Rejection) {
        if let Some(r) = self.stats.rejected.get_mut(why.index()) {
            *r = r.saturating_add(1);
        }
    }

    /// Processes a placement node (§7.4).
    fn node(&mut self, n: &PlacementNode, parent: Frame, sigma: [f32; 3], c: u32, sw: Switches) {
        if !self.enter() {
            return;
        }
        self.node_inner(n, parent, sigma, c, sw);
        self.depth -= 1;
    }

    #[allow(clippy::too_many_lines)]
    fn node_inner(
        &mut self,
        n: &PlacementNode,
        parent: Frame,
        sigma: [f32; 3],
        c: u32,
        sw: Switches,
    ) {
        if !self.charge(1) {
            return;
        }
        self.stats.nodes_tested += 1;
        let (a, b) = (self.a, self.b);
        let r = |salt: u32| hash::node_random(a, b, c.wrapping_add(salt));

        // 1. Frequency.
        if r(hash::SALT_FREQUENCY) >= n.frequency || n.frequency.is_nan() {
            self.reject(Rejection::Frequency);
            return;
        }
        // 2. Switches.
        let sw = Switches {
            no_spacing: sw.no_spacing || n.no_spacing,
            no_checks: sw.no_checks || n.no_checks,
        };
        // 3. Scale.
        if n.scale_mode != SCALE_MODE_UNIFORM {
            self.reject(Rejection::ScaleMode);
            return;
        }
        let start = if n.inherit_scale { sigma } else { [1.0; 3] };
        let varies = (0..3).any(|k| (n.scale_max[k] - n.scale_min[k]).abs() > EPS);
        let t = if varies { r(hash::SALT_SCALE) } else { 0.0 };
        let scale: [f32; 3] = std::array::from_fn(|k| {
            let f = if varies {
                n.scale_min[k] + t * (n.scale_max[k] - n.scale_min[k])
            } else {
                n.scale_min[k]
            };
            start[k] * f
        });
        // 4. Position.
        let height_mode = n.height_mode;
        let along = |k: usize, salt: u32| {
            let rv = r(salt);
            let d = n.disp_min[k] + rv * (n.disp_max[k] - n.disp_min[k]);
            if n.disp_scaled { d * sigma[k] } else { d }
        };
        let mut d = [
            along(0, hash::SALT_DISP_X),
            along(1, hash::SALT_DISP_Y),
            if height_mode == 9 {
                along(2, hash::SALT_DISP_Z)
            } else {
                0.0
            },
        ];
        if n.disp_relative {
            d = parent.rot.rotate(d);
        }
        let mut p = [
            parent.pos[0] + d[0],
            parent.pos[1] + d[1],
            parent.pos[2] + d[2],
        ];
        // 5. Spacing radius.
        let largest = if n.spacing_3d {
            scale[0].max(scale[1]).max(scale[2])
        } else {
            scale[0].max(scale[1])
        };
        let rho_node = if n.spacing_scaled {
            n.spacing * largest
        } else {
            n.spacing
        };
        let rho = if sw.no_spacing { 0.0 } else { rho_node };
        // 6. Inside the block. The extra 0 ≤ p < 160 bound changes nothing for ρ ≥ 0; it
        //    keeps a hostile negative `spacing` from placing objects outside the block
        //    (`scenery.md` §13.1 OPEN).
        let inside = |v: f32| rho <= v && v < BLOCK - rho && (0.0..BLOCK).contains(&v);
        if !(inside(p[0]) && inside(p[1])) {
            self.reject(Rejection::Outside);
            return;
        }
        // 7. Terrain type.
        if !n.no_checks && !self.terrain_type_ok(p, rho) {
            self.reject(Rejection::TerrainType);
            return;
        }
        // 8. Terrain plane.
        let plane = self
            .input
            .terrain
            .client_plane_at(p[0], p[1], &self.input.data.vertex_flags);
        let nrm = plane.normal;
        // 9. Slope.
        if !(n.slope_min_cos <= nrm[2] && nrm[2] <= n.slope_max_cos) {
            self.reject(Rejection::Slope);
            return;
        }
        // 10. Orientation.
        let mut h = n.heading.min + r(hash::SALT_HEADING) * (n.heading.max - n.heading.min);
        let a2 = n.angle2.min + r(hash::SALT_ANGLE2) * (n.angle2.max - n.angle2.min);
        let a3 = n.angle3.min + r(hash::SALT_ANGLE3) * (n.angle3.max - n.angle3.min);
        let rot = if n.face_direction {
            let horiz = (nrm[0] * nrm[0] + nrm[1] * nrm[1]).sqrt();
            let bearing = if horiz <= 1e-6 {
                0.0
            } else {
                (-nrm[0]).atan2(nrm[1]).to_degrees()
            };
            h = h + 180.0 + bearing;
            if n.align_to_slope {
                math::mul(math::align_to_normal(nrm), math::rot_z(-h))
            } else {
                math::rot_z(-h)
            }
        } else if n.align_to_slope {
            math::mul(math::align_to_normal(nrm), math::rot_z(-h))
        } else {
            let mut local = math::rot_z(-h);
            if a3 != 0.0 {
                local = math::mul(local, math::rot_y(a3));
            }
            if a2 != 0.0 {
                local = math::mul(local, math::rot_x(a2));
            }
            if n.orient_relative {
                math::mul(parent.rot, local)
            } else {
                local
            }
        };
        // 11. Height.
        match height_mode {
            0 => {
                if let Some(z) = plane.height_at(p[0], p[1]) {
                    p[2] = z;
                }
            }
            1 => match self.water {
                Some(w) => {
                    if let Some(z) = w.height_at(p[0], p[1]) {
                        p[2] = z;
                    }
                }
                None => {
                    self.reject(Rejection::NoWater);
                    return;
                }
            },
            2..=8 => p[2] = 0.0,
            _ => {}
        }
        if height_mode != 9 {
            let rv = r(hash::SALT_DISP_X);
            let off = n.disp_min[2] + rv * (n.disp_max[2] - n.disp_min[2]);
            p[2] += if n.disp_scaled { off * sigma[2] } else { off };
        }
        // 12. Height ranges.
        if !n.no_checks && !self.height_ranges_ok(n, p, &plane) {
            self.reject(Rejection::HeightRange);
            return;
        }
        // 13. Terrain clearance.
        if n.clearance > 0.0 {
            let radius = if n.clearance_scaled {
                n.clearance * scale[0].max(scale[1])
            } else {
                n.clearance
            };
            if !self.clearance_ok(n, p, radius, rot) {
                self.reject(Rejection::Clearance);
                return;
            }
        }
        // 14. Spacing.
        if !n.no_checks {
            if self.static_overlaps(p, rho, n.spacing_3d) {
                self.reject(Rejection::StaticSpacing);
                return;
            }
            if !sw.no_spacing && self.footprint_overlaps(p, rho, n.spacing_3d) {
                self.reject(Rejection::Footprint);
                return;
            }
        }
        // 15. Children.
        self.stats.nodes_placed += 1;
        let marker = if self.pass == Pass::Frill && self.under_frill {
            None
        } else if self.markers.len() < self.input.limits.max_markers as usize {
            self.markers.push(Marker {
                pos: p,
                radius: rho_node,
                spacing_3d: n.spacing_3d,
                open: true,
            });
            Some(self.markers.len() - 1)
        } else {
            self.cut = true;
            None
        };
        let frame = Frame { pos: p, rot };
        let mut created = 0u32;
        for (k, child) in n.children.iter().enumerate() {
            if n.max_objects > 0 && created >= n.max_objects {
                break;
            }
            if !self.charge(1) {
                break;
            }
            #[allow(clippy::cast_possible_truncation)]
            let ck = hash::child_seed(c, k as u32);
            match child {
                SceneChild::Node(m) => self.node(m, frame, scale, ck, sw),
                SceneChild::Scene(dd) => {
                    created += self.instantiate(*dd, frame, scale, ck, sw);
                }
                SceneChild::Object(dd) => {
                    if self.create(*dd, frame, scale) {
                        created += 1;
                    }
                }
            }
        }
        if let Some(m) = marker
            && let Some(m) = self.markers.get_mut(m)
        {
            m.open = false;
        }
    }

    /// §7.4 step 7.
    fn terrain_type_ok(&mut self, p: [f32; 3], rho: f32) -> bool {
        let data = self.input.data;
        if data.terrain.quads.is_none() {
            return true;
        }
        let qx = p[0] * 32.0 / BLOCK;
        let qy = p[1] * 32.0 / BLOCK;
        let rq = rho * 32.0 / BLOCK;
        let (lx, hx) = (qx - rq - 1.0, qx + rq + 1.0);
        let (ly, hy) = (qy - rq - 1.0, qy + rq + 1.0);
        #[allow(clippy::cast_precision_loss)]
        let side = QUADS as f32;
        if lx > side || ly > side || hx < 0.0 || hy < 0.0 {
            return true;
        }
        // `as` truncates toward zero (and saturates), then clamp to 0..31.
        #[allow(clippy::cast_possible_truncation)]
        let bound = |v: f32| (v as i32).clamp(0, QUADS - 1);
        let (x0, x1, y0, y1) = (bound(lx), bound(hx), bound(ly), bound(hy));
        let span = u32::try_from((x1 - x0 + 1) * (y1 - y0 + 1)).unwrap_or(u32::MAX);
        if !self.charge(span / 16) {
            return false;
        }
        let rq2 = rq * rq;
        for gy in y0..=y1 {
            for gx in x0..=x1 {
                #[allow(clippy::cast_precision_loss)]
                let dx = (gx as f32 + 0.5 - qx).abs();
                #[allow(clippy::cast_precision_loss)]
                let dy = (gy as f32 + 0.5 - qy).abs();
                let counted = dx < 0.5 || dy < 0.5 || {
                    let (ex, ey) = (dx - 0.5, dy - 0.5);
                    ex * ex + ey * ey <= rq2
                };
                if !counted {
                    continue;
                }
                #[allow(clippy::cast_sign_loss)]
                let Some(material) = data.terrain.quad_material(gx as usize, gy as usize) else {
                    continue;
                };
                if self.input.tables.procedural_allowed(u32::from(material)) == Some(false) {
                    return false;
                }
            }
        }
        true
    }

    /// §7.5.
    fn height_ranges_ok(&self, n: &PlacementNode, p: [f32; 3], plane: &Plane) -> bool {
        let z = p[2];
        if !(n.z_range.min <= z && z <= n.z_range.max) {
            return false;
        }
        let g = plane.height_at(p[0], p[1]).unwrap_or(z);
        let check = |mode: u8, a: f32, b: f32, q: f32| match mode {
            0 => false,
            1 => q >= a,
            2 => a <= q && q <= b,
            _ => true,
        };
        if let Some(wp) = self.water {
            let w = wp.height_at(p[0], p[1]).unwrap_or(z);
            if w - g > EPS {
                if !check(
                    n.water_depth_mode,
                    n.water_depth.min,
                    n.water_depth.max,
                    w - g,
                ) {
                    return false;
                }
                return if z - w >= -EPS {
                    check(
                        n.above_water_mode,
                        n.above_water.min,
                        n.above_water.max,
                        z - w,
                    )
                } else {
                    check(
                        n.below_water_mode,
                        n.below_water.min,
                        n.below_water.max,
                        w - z,
                    )
                };
            }
        }
        if z - g >= -EPS {
            check(
                n.above_ground_mode,
                n.above_ground.min,
                n.above_ground.max,
                z - g,
            )
        } else {
            check(
                n.below_ground_mode,
                n.below_ground_min,
                n.below_ground_max,
                g - z,
            )
        }
    }

    /// §7.4 step 13 with the radius query of `collision.md` §6.6.
    fn clearance_ok(
        &mut self,
        n: &PlacementNode,
        p: [f32; 3],
        radius: f32,
        rot: Quaternion,
    ) -> bool {
        let tris = self.input.terrain.client_triangles_near(p[0], p[1], radius);
        let count = u32::try_from(tris.len()).unwrap_or(u32::MAX);
        if !self.charge(count / 4) {
            return false;
        }
        // a. Slope bounds of every triangle.
        if tris
            .iter()
            .any(|(_, pl)| !(n.clear_min_cos <= pl.normal[2] && pl.normal[2] <= n.clear_max_cos))
        {
            return false;
        }
        // b. Aspect (never exercised: the tolerance is 360 in every node).
        if n.clear_aspect_tol < 180.0 {
            let heading = rot.heading_degrees();
            for (_, pl) in &tris {
                let nn = pl.normal;
                if nn[0].abs() <= EPS && nn[1].abs() <= EPS {
                    continue;
                }
                if math::bearing_difference(math::aspect_degrees(nn), heading) >= n.clear_aspect_tol
                {
                    return false;
                }
            }
        }
        // c. Pairwise normal agreement. Triangles with bit-identical normals give the same
        //    dot product with every other, so each distinct normal is tested once against
        //    every other; a normal shared by two or more triangles is also tested against
        //    itself, since those triangles form a pair too.
        if n.clear_min_dot > -0.9998 {
            let mut normals: Vec<[u32; 3]> = tris
                .iter()
                .map(|(_, pl)| pl.normal.map(f32::to_bits))
                .collect();
            normals.sort_unstable();
            let mut distinct: Vec<([f32; 3], bool)> = Vec::with_capacity(normals.len());
            for (k, v) in normals.iter().enumerate() {
                if k > 0 && normals[k - 1] == *v {
                    if let Some(last) = distinct.last_mut() {
                        last.1 = true;
                    }
                } else {
                    distinct.push((v.map(f32::from_bits), false));
                }
            }
            let pairs =
                u32::try_from(distinct.len().saturating_mul(distinct.len())).unwrap_or(u32::MAX);
            if !self.charge(pairs / 64) {
                return false;
            }
            let fails = |a: &[f32; 3], b: &[f32; 3]| {
                let dot = a[0] * b[0] + a[1] * b[1] + a[2] * b[2];
                // NaN fails the test like a dot below the bound.
                dot.partial_cmp(&n.clear_min_dot)
                    .is_none_or(std::cmp::Ordering::is_lt)
            };
            for (k, (a, shared)) in distinct.iter().enumerate() {
                if *shared && fails(a, a) {
                    return false;
                }
                if distinct[k + 1..].iter().any(|(b, _)| fails(a, b)) {
                    return false;
                }
            }
        }
        true
    }

    /// §7.4 step 14a. The scan is charged to the work budget; a scan the budget cannot pay
    /// for counts as an overlap (as in [`Gen::footprint_overlaps`]).
    fn static_overlaps(&mut self, p: [f32; 3], rho: f32, spacing_3d: bool) -> bool {
        if self.input.statics.is_empty() {
            return false;
        }
        if !self.charge(self.static_cost) {
            return true;
        }
        self.input
            .statics
            .iter()
            .any(|s| s.overlaps(p, rho, spacing_3d))
    }

    /// §7.4 step 14b: closed markers only.
    fn footprint_overlaps(&mut self, p: [f32; 3], rho: f32, spacing_3d: bool) -> bool {
        let count = u32::try_from(self.markers.len()).unwrap_or(u32::MAX);
        if !self.charge(count / 64) {
            return true;
        }
        self.markers.iter().any(|m| {
            if m.open || m.radius.is_nan() || m.radius <= 0.0 {
                return false;
            }
            let dx = p[0] - m.pos[0];
            let dy = p[1] - m.pos[1];
            let dz = if spacing_3d && m.spacing_3d {
                p[2] - m.pos[2]
            } else {
                0.0
            };
            let r = rho + m.radius;
            r * r >= dx * dx + dy * dy + dz * dz
        })
    }
}

/// Work units of one step-14a scan: one per [`STATIC_ITEMS_PER_WORK_UNIT`] entities and
/// volumes, at least 1 for a non-empty list.
fn static_cost(statics: &[StaticClearance]) -> u32 {
    let items = statics.iter().fold(0usize, |n, s| {
        n.saturating_add(1).saturating_add(s.volume_count())
    });
    let units = items.div_ceil(STATIC_ITEMS_PER_WORK_UNIT as usize);
    u32::try_from(units).unwrap_or(u32::MAX)
}
