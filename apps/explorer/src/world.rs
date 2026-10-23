// SPDX-License-Identifier: GPL-3.0-or-later
//! The M4.5 world views' data, built on the asset workers: a landblock's generated
//! scenery, water and collision (`scenery.md` §6–§7, `water.md` §6–§7, `collision.md`
//! §5, §6, §9.3–§9.4, §12.1), and the sky and day-description timeline
//! (`environment.md` §5, §11).
//!
//! | Item | Purpose |
//! |---|---|
//! | [`WorldState`] | The install's `tessera-world` context (loaded once, on a worker) and a small cache of built landblock bundles |
//! | [`LandblockDecoder`], [`LandblockProduct`] | One landblock: terrain heights, generated objects, statics, water, collision summary |
//! | [`SkyDecoder`], [`SkyProduct`] | A sky description (0x15) with its preloaded day descriptions, or one day description (0x1A) with its timeline, computed on the worker |
//! | [`Timeline`], [`day_timeline`], [`sky_timeline`], [`clock_value`] | Blended keyframes over one game day (pure, bounded; re-exported from [`crate::timeline`]) |
//!
//! Scenery, water and collision are produced by `tessera-world` and `tessera-physics`
//! exactly as the walker builds them ([`tessera_world::build_bundle`]); this module only
//! summarises the result. Every list is bounded ([`MAX_LISTED`]).

use std::collections::{BTreeMap, BTreeSet, VecDeque};
use std::sync::{Arc, Mutex, OnceLock, PoisonError};
use std::time::Instant;

use tessera_assets::{AssetError, AssetKey, Decoder, JobContext, RecordSource};
use tessera_dat::ArchiveSet;
use tessera_formats::{DayDescription, Did, Record};
use tessera_physics::geometry::{BodyId, BodySource, CollisionSet, QueryFilter};
use tessera_world::environment::{Calendar, SkyLibrary};
use tessera_world::{
    BundleOptions, BundleSource, LandblockBundle, LandblockKey, WorldContext, WorldSource,
    build_bundle, heading_degrees,
};

use crate::data::DataContext;
use crate::error::ExplorerError;
pub use crate::timeline::{
    MAX_TIMELINE_SAMPLES, MAX_TRACKS, ObjectTrack, START_DAY_TYPE, TIMELINE_SAMPLES, Timeline,
    day_heap_bytes, day_timeline, day_timeline_with, defaults_timeline, initial_sky_key,
    library_heap_bytes, sky_choice, sky_timeline,
};

/// Most objects, statics, cells and liquid boxes a landblock product lists.
pub const MAX_LISTED: usize = 65_536;
/// Built landblock bundles kept for the landblock and 3D views (both read the same one).
pub const BUNDLE_CACHE: usize = 4;

/// The install's world model for the explorer: the `tessera-world` context, loaded on
/// first use (on a worker: it reads the property dictionary, the region descriptors and
/// the tables of their eight slots, and preloads the sky), and the last few landblock
/// bundles built.
pub struct WorldState {
    set: Arc<ArchiveSet>,
    ctx: OnceLock<Result<Arc<WorldContext>, String>>,
    bundles: Mutex<VecDeque<(u32, Arc<LandblockBundle>)>>,
}

impl std::fmt::Debug for WorldState {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("WorldState")
            .field("loaded", &self.ctx.get().map(Result::is_ok))
            .finish_non_exhaustive()
    }
}

impl WorldState {
    /// The world state of `set`; nothing is read until first use.
    #[must_use]
    pub fn new(set: Arc<ArchiveSet>) -> Self {
        Self {
            set,
            ctx: OnceLock::new(),
            bundles: Mutex::new(VecDeque::new()),
        }
    }

    /// The world context, loaded on the first call (blocking; call it on a worker).
    /// Concurrent first callers wait for one load. A failed load is kept.
    ///
    /// # Errors
    ///
    /// When the install's property dictionary is missing or does not decode.
    pub fn context(&self) -> Result<Arc<WorldContext>, ExplorerError> {
        self.ctx
            .get_or_init(|| {
                let started = Instant::now();
                let r = WorldContext::from_install(Arc::clone(&self.set), BundleOptions::default())
                    .map(Arc::new)
                    .map_err(|e| e.to_string());
                tracing::info!(
                    ms = started.elapsed().as_millis() as u64,
                    ok = r.is_ok(),
                    "explorer world context"
                );
                r
            })
            .clone()
            .map_err(ExplorerError::Worker)
    }

    /// The record source of the world (the archive set).
    #[must_use]
    pub fn source(&self) -> Arc<dyn WorldSource> {
        Arc::clone(&self.set) as Arc<dyn WorldSource>
    }

    /// The bundle of landblock `key`, built as the walker builds it (scenery, water,
    /// collision, lights), or a cached one.
    ///
    /// # Errors
    ///
    /// When the context cannot load, the landblock has no data record, a record cannot
    /// be read, or `cancelled` turns true.
    pub fn bundle(
        &self,
        key: LandblockKey,
        cancelled: &dyn Fn() -> bool,
    ) -> Result<Arc<LandblockBundle>, ExplorerError> {
        let did = key.data_did().0;
        if let Some(b) = self
            .bundles
            .lock()
            .unwrap_or_else(PoisonError::into_inner)
            .iter()
            .find(|(d, _)| *d == did)
            .map(|(_, b)| Arc::clone(b))
        {
            return Ok(b);
        }
        let ctx = self.context()?;
        let frame = BundleSource::new(Arc::clone(ctx.source()))
            .read_record(did)
            .map_err(asset_error)?;
        if cancelled() {
            return Err(ExplorerError::Cancelled);
        }
        let bundle = match build_bundle(&ctx, Did(did), &frame, cancelled) {
            Ok(b) => Arc::new(b),
            Err(_) if cancelled() => return Err(ExplorerError::Cancelled),
            Err(e) => return Err(e.into()),
        };
        if bundle.is_complete() {
            let mut cache = self.bundles.lock().unwrap_or_else(PoisonError::into_inner);
            cache.retain(|(d, _)| *d != did);
            if cache.len() >= BUNDLE_CACHE {
                cache.pop_front();
            }
            cache.push_back((did, Arc::clone(&bundle)));
        }
        Ok(bundle)
    }
}

/// An asset-layer error as an explorer error.
pub(crate) fn asset_error(e: AssetError) -> ExplorerError {
    match e {
        AssetError::NotFound { did } => ExplorerError::NotFound(did),
        other => ExplorerError::Worker(other.to_string()),
    }
}

/// The landblock of a landblock-family DID (any record kind).
#[must_use]
pub fn landblock_of(did: u32) -> Option<LandblockKey> {
    LandblockKey::from_did(Did(did))
}

// ---------------------------------------------------------------------------------------
// Landblocks
// ---------------------------------------------------------------------------------------

/// An object placed in a landblock, as the landblock view lists it.
#[derive(Debug, Clone, PartialEq)]
pub struct ObjectRow {
    /// Position in its list (info entities or generated objects).
    pub index: u32,
    /// The object (0x47, or 0x70 for weenies).
    pub object: Option<u32>,
    /// Position in the landblock frame (x, y from the south-west corner, z absolute).
    pub position: [f32; 3],
    /// Compass heading in degrees (`coordinates.md` §11).
    pub heading: f32,
    /// Largest scale factor.
    pub scale: f32,
    /// `Render_LODClass`, when known.
    pub lod_class: Option<u32>,
    /// Whether the object got a collision body (`collision.md` §9.3–§9.4). An object
    /// ethereal to Player gets one too, so this does not mean it blocks a walker.
    pub collides: bool,
    /// Whether it blocks a walker: it got a movement body that the walker's filter
    /// ([`QueryFilter::player`]) accepts, so its effective `Physics_EtherealToType` does
    /// not contain Player (§9.2).
    pub blocks: bool,
    /// Whether it is drawn (the M3 draw rule; generated objects with geometry).
    pub drawn: bool,
}

/// A placed cell of landblock info.
#[derive(Debug, Clone, PartialEq)]
pub struct CellRow {
    /// Cell number.
    pub index: u16,
    /// Its 0x01 geometry.
    pub geometry: u32,
    /// Its position in the landblock frame, when it could be placed.
    pub position: Option<[f32; 3]>,
    /// Portal count.
    pub portals: usize,
    /// Portal polygons in the landblock frame.
    pub outlines: Vec<Vec<[f32; 3]>>,
}

/// A liquid box in the landblock frame: its bounds and flags.
#[derive(Debug, Clone, PartialEq)]
pub struct LiquidRow {
    /// Bounds (min, max).
    pub bounds: ([f32; 3], [f32; 3]),
    /// Flags, as text.
    pub flags: String,
}

/// One landblock for the landblock view.
#[derive(Debug, Clone)]
pub struct LandblockProduct {
    /// The selected DID (any record of the landblock).
    pub did: u32,
    /// The landblock.
    pub key: LandblockKey,
    /// Terrain heights by sample, `heights[33 · j + i]` at (5i, 5j).
    pub heights: Vec<f32>,
    /// Ceiling heights, when the landblock has a ceiling.
    pub ceiling: Option<Vec<f32>>,
    /// Lowest and highest terrain height.
    pub range: (f32, f32),
    /// The water height, when the landblock is wet.
    pub water: Option<f32>,
    /// General facts.
    pub summary: Vec<(String, String)>,
    /// Scenery generation counters.
    pub scenery: Vec<(String, String)>,
    /// Water decision and volumes.
    pub water_rows: Vec<(String, String)>,
    /// Collision set summary.
    pub collision: Vec<(String, String)>,
    /// Generated scenery objects.
    pub generated: Vec<ObjectRow>,
    /// Static entities of landblock info.
    pub statics: Vec<ObjectRow>,
    /// Cells.
    pub cells: Vec<CellRow>,
    /// Liquid boxes.
    pub liquids: Vec<LiquidRow>,
    /// Bundle problems.
    pub problems: Vec<String>,
    /// Build time in milliseconds.
    pub build_ms: u64,
}

impl LandblockProduct {
    /// Approximate heap bytes.
    #[must_use]
    pub fn heap_bytes(&self) -> usize {
        let rows =
            |v: &[(String, String)]| v.iter().map(|(a, b)| a.len() + b.len() + 48).sum::<usize>();
        self.heights.len() * 4
            + self.ceiling.as_ref().map_or(0, |c| c.len() * 4)
            + rows(&self.summary)
            + rows(&self.scenery)
            + rows(&self.water_rows)
            + rows(&self.collision)
            + (self.generated.len() + self.statics.len()) * std::mem::size_of::<ObjectRow>()
            + self
                .cells
                .iter()
                .map(|c| 64 + c.outlines.iter().map(|o| o.len() * 12 + 24).sum::<usize>())
                .sum::<usize>()
            + self.liquids.len() * 64
            + self.problems.iter().map(|p| p.len() + 24).sum::<usize>()
            + 512
    }
}

fn kv(k: &str, v: impl Into<String>) -> (String, String) {
    (k.to_owned(), v.into())
}

fn max_scale(s: [f32; 3]) -> f32 {
    let m = s[0].abs().max(s[1].abs()).max(s[2].abs());
    if m.is_finite() { m } else { 1.0 }
}

/// Which static entities and generated objects of a landblock got a collision body, and
/// whether a walker is blocked by it (`collision.md` §9.2–§9.4).
#[derive(Debug, Clone, Default, PartialEq, Eq)]
pub struct BodyIndex {
    /// Static entity index (in the info record's entity list) → whether one of its bodies
    /// blocks a walker. Statics without a body are absent.
    pub statics: BTreeMap<u32, bool>,
    /// Per generated object (the bundle's order): `None` without a body, else whether it
    /// blocks a walker. At most [`MAX_LISTED`] entries.
    pub generated: Vec<Option<bool>>,
}

impl BodyIndex {
    /// The body index of `set`, with `generated_bodies` the body of each generated object
    /// ([`tessera_world::BlockCollision::generated_bodies`]). "Blocks" is the walker's rule:
    /// [`QueryFilter::player`] accepts the body.
    #[must_use]
    pub fn new(set: &CollisionSet, generated_bodies: &[Option<BodyId>]) -> Self {
        let walker = QueryFilter::player();
        let mut statics = BTreeMap::new();
        for b in set.bodies() {
            if let BodySource::Static { index, .. } = b.source {
                *statics.entry(index).or_insert(false) |= walker.accepts(b);
            }
        }
        let generated = generated_bodies
            .iter()
            .take(MAX_LISTED)
            .map(|id| id.and_then(|id| set.body(id)).map(|b| walker.accepts(b)))
            .collect();
        Self { statics, generated }
    }
}

/// Summarises a built bundle (pure; used by [`LandblockDecoder`] and tests).
#[must_use]
pub fn landblock_product(did: u32, bundle: &LandblockBundle, build_ms: u64) -> LandblockProduct {
    let key = bundle.key;
    let heights_mesh = bundle
        .scenery
        .as_ref()
        .map(|s| Arc::clone(&s.terrain))
        .or_else(|| {
            bundle
                .collision
                .as_ref()
                .and_then(|c| c.set.terrain().cloned())
        })
        .unwrap_or_else(|| Arc::new(tessera_physics::geometry::GridMesh::terrain(&bundle.data)));
    let grid = |m: &tessera_physics::geometry::GridMesh| -> Vec<f32> {
        (0..33)
            .flat_map(|j| (0..33).map(move |i| (i, j)))
            .map(|(i, j)| m.vertex(i, j).map_or(0.0, |v| v[2]))
            .collect()
    };
    let heights = grid(&heights_mesh);
    let ceiling = tessera_physics::geometry::GridMesh::ceiling(&bundle.data).map(|m| grid(&m));
    let (lo, hi) = heights
        .iter()
        .filter(|h| h.is_finite())
        .fold((f32::INFINITY, f32::NEG_INFINITY), |(lo, hi), h| {
            (lo.min(*h), hi.max(*h))
        });
    let range = if lo <= hi { (lo, hi) } else { (0.0, 0.0) };

    let mut summary = vec![
        kv(
            "landblock",
            format!("region {} · x {} · y {}", key.region(), key.x(), key.y()),
        ),
        kv("data", format!("{:#010x}", key.data_did().0)),
        kv("terrain", format!("{:.2} … {:.2} m", range.0, range.1)),
        kv("ceiling", if ceiling.is_some() { "yes" } else { "no" }),
        kv("entities", bundle.entities.len().to_string()),
        kv("generated", bundle.generated.len().to_string()),
        kv("lights", bundle.lights.len().to_string()),
        kv("complete", if bundle.is_complete() { "yes" } else { "no" }),
    ];
    if let Some(a) = &bundle.area {
        summary.push(kv("area", format!("{a:?}")));
    }

    let mut scenery = Vec::new();
    match &bundle.scenery {
        None => scenery.push(kv("scenery", "not generated (no tables for this region)")),
        Some(s) => {
            let st = &s.stats;
            scenery.push(kv("objects", st.objects.to_string()));
            scenery.push(kv(
                "vertices with a scene",
                st.vertices_with_scene.to_string(),
            ));
            scenery.push(kv(
                "vertices without a pair",
                st.vertices_without_pair.to_string(),
            ));
            scenery.push(kv("missing scene keys", st.missing_keys.to_string()));
            scenery.push(kv("missing scenes", st.missing_scenes.to_string()));
            scenery.push(kv(
                "nodes placed / tested",
                format!("{} / {}", st.nodes_placed, st.nodes_tested),
            ));
            for r in tessera_world::scenery::Rejection::ALL {
                let n = st.rejections(r);
                if n > 0 {
                    scenery.push(kv(&format!("rejected: {r:?}"), n.to_string()));
                }
            }
            scenery.push(kv(
                "cycles / depth capped / budget cut",
                format!("{} / {} / {}", st.cycles, st.depth_capped, st.budget_cut),
            ));
            scenery.push(kv("statics with clearance", s.statics.len().to_string()));
            scenery.push(kv(
                "water plane (scenery)",
                s.water
                    .map_or_else(|| "none".into(), |p| format!("{:?} d {}", p.normal, p.d)),
            ));
            scenery.push(kv("complete", if s.complete { "yes" } else { "no" }));
        }
    }

    let mut water_rows = Vec::new();
    let mut liquids = Vec::new();
    let mut water = None;
    match &bundle.water {
        None => water_rows.push(kv("water", "not built")),
        Some(w) => {
            match &w.decision {
                Ok(lw) => {
                    water = Some(lw.height);
                    water_rows.push(kv("state", "wet"));
                    water_rows.push(kv("type", lw.water_type.to_string()));
                    water_rows.push(kv("level", lw.level.to_string()));
                    water_rows.push(kv("height", format!("{:.3} m", lw.height)));
                    water_rows.push(kv("surface", format!("{:#010x}", lw.surface().0)));
                    water_rows.push(kv(
                        "volume entity",
                        format!("{:#010x}", lw.volume_entity().0),
                    ));
                    water_rows.push(kv(
                        "terrain",
                        format!("{:.2} … {:.2} m", lw.lowest_terrain, lw.highest_terrain),
                    ));
                    water_rows.push(kv(
                        "fully submerged",
                        if lw.is_fully_submerged() { "yes" } else { "no" },
                    ));
                }
                Err(reason) => {
                    water_rows.push(kv("state", "dry"));
                    water_rows.push(kv("reason", format!("{reason:?}")));
                }
            }
            water_rows.push(kv("liquid boxes", w.boxes.len().to_string()));
            water_rows.push(kv(
                "placed volume entities",
                w.placed_volume_entities.to_string(),
            ));
            if w.boxes_left_out > 0 {
                water_rows.push(kv("boxes left out", w.boxes_left_out.to_string()));
            }
            for b in w.boxes.iter().take(MAX_LISTED) {
                if let Some(a) = b.aabb() {
                    let f = b.flags;
                    let names: Vec<&str> = [
                        (f.liquid, "liquid"),
                        (f.lava, "lava"),
                        (f.surface, "surface"),
                        (f.wading, "wading"),
                        (f.deep, "deep"),
                    ]
                    .iter()
                    .filter(|(on, _)| *on)
                    .map(|(_, n)| *n)
                    .collect();
                    liquids.push(LiquidRow {
                        bounds: (a.min, a.max),
                        flags: names.join(" "),
                    });
                }
            }
        }
    }

    let mut collision = Vec::new();
    let mut cells = Vec::new();
    let mut bodies = BodyIndex::default();
    match &bundle.collision {
        None => collision.push(kv("collision", "not built")),
        Some(c) => {
            let r = c.report();
            let set = &c.set;
            collision.push(kv("bodies", set.bodies().len().to_string()));
            collision.push(kv("triangles", set.triangle_count().to_string()));
            collision.push(kv("spheres and capsules", set.round_count().to_string()));
            collision.push(kv("solid boxes", set.solid_box_count().to_string()));
            bodies = BodyIndex::new(set, &c.generated_bodies);
            let walker = QueryFilter::player();
            let passed = set
                .bodies()
                .iter()
                .filter(|b| {
                    b.purpose == tessera_physics::geometry::BodyPurpose::Movement
                        && !walker.accepts(b)
                })
                .count();
            collision.push(kv(
                "statics with a body / seen",
                format!("{} / {}", r.statics_blocking, r.statics_seen),
            ));
            collision.push(kv(
                "statics blocking a walker",
                bodies.statics.values().filter(|b| **b).count().to_string(),
            ));
            collision.push(kv(
                "generated with a body / seen",
                format!("{} / {}", r.generated_blocking, r.generated_seen),
            ));
            collision.push(kv(
                "generated blocking a walker",
                bodies
                    .generated
                    .iter()
                    .filter(|b| **b == Some(true))
                    .count()
                    .to_string(),
            ));
            collision.push(kv(
                "movement bodies a walker passes through (ethereal to Player)",
                passed.to_string(),
            ));
            collision.push(kv(
                "cells placed / refused",
                format!("{} / {}", r.cells, r.cells_refused),
            ));
            collision.push(kv(
                "unresolved / incomplete / over limit / dropped",
                format!(
                    "{} / {} / {} / {}",
                    r.unresolved, r.incomplete, r.over_limit, r.dropped_elements
                ),
            ));
            collision.push(kv(
                "ceiling mesh",
                if set.ceiling().is_some() { "yes" } else { "no" },
            ));
            if c.portals_left_out > 0 {
                collision.push(kv("portals left out", c.portals_left_out.to_string()));
            }
            collision.push(kv("complete", if c.complete { "yes" } else { "no" }));
            for cell in c.cells.iter().take(MAX_LISTED) {
                cells.push(CellRow {
                    index: cell.index,
                    geometry: cell.geometry.0,
                    position: cell.placement.map(|p| p.position),
                    portals: cell.portals.len(),
                    outlines: cell
                        .portals
                        .iter()
                        .filter(|p| p.corners.len() >= 2)
                        .map(|p| p.corners.clone())
                        .collect(),
                });
            }
        }
    }

    // `body`: None without a collision body, else whether it blocks a walker.
    let row = |i: usize, e: &tessera_world::PlacedEntity, lod: Option<u32>, body: Option<bool>| {
        ObjectRow {
            index: u32::try_from(i).unwrap_or(u32::MAX),
            object: e.object.map(|d| d.0),
            position: e.placement.position,
            heading: heading_degrees(e.placement.rotation) as f32,
            scale: max_scale(e.scale),
            lod_class: lod,
            collides: body.is_some(),
            blocks: body == Some(true),
            drawn: e.draw,
        }
    };
    let generated = bundle
        .generated
        .iter()
        .enumerate()
        .take(MAX_LISTED)
        .map(|(i, e)| {
            row(
                i,
                e,
                bundle.generated_lod_classes.get(i).copied().flatten(),
                bodies.generated.get(i).copied().flatten(),
            )
        })
        .collect();
    let statics = bundle
        .entities
        .iter()
        .enumerate()
        .take(MAX_LISTED)
        .map(|(i, e)| {
            row(
                i,
                e,
                bundle.entity_lod_classes.get(i).copied().flatten(),
                bodies.statics.get(&e.index).copied(),
            )
        })
        .collect();
    let problems = bundle
        .problems
        .iter()
        .take(256)
        .map(|p| format!("{:#010x} {}: {}", p.did.0, p.what, p.message))
        .collect();
    LandblockProduct {
        did,
        key,
        heights,
        ceiling,
        range,
        water,
        summary,
        scenery,
        water_rows,
        collision,
        generated,
        statics,
        cells,
        liquids,
        problems,
        build_ms,
    }
}

/// Builds [`LandblockProduct`]s for landblock-family DIDs (any record kind of the
/// landblock).
#[derive(Debug, Clone)]
pub struct LandblockDecoder(pub Arc<DataContext>);

impl Decoder for LandblockDecoder {
    type Output = LandblockProduct;
    type Error = ExplorerError;
    const NAME: &'static str = "explorer-landblock";

    fn decode(
        &self,
        key: AssetKey,
        _: Vec<u8>,
        job: &JobContext<'_>,
    ) -> Result<LandblockProduct, ExplorerError> {
        let lb = landblock_of(key.did).ok_or_else(|| {
            ExplorerError::NotApplicable(format!("{:#010x} is not a landblock record", key.did))
        })?;
        let started = Instant::now();
        let bundle = self.0.world.bundle(lb, &|| job.is_cancelled())?;
        Ok(landblock_product(
            key.did,
            &bundle,
            started.elapsed().as_millis() as u64,
        ))
    }

    fn size_of(&self, v: &LandblockProduct) -> usize {
        v.heap_bytes()
    }
}

// ---------------------------------------------------------------------------------------
// Sky and day descriptions
// ---------------------------------------------------------------------------------------

/// The data of the sky view.
#[derive(Debug, Clone)]
pub enum SkyProduct {
    /// A sky description (0x15) with its game time and preloaded day descriptions.
    Sky {
        /// The library (records read; evaluation reads nothing more).
        library: Arc<SkyLibrary>,
        /// Every sky key the description lists (day groups and local defaults), sorted.
        keys: Vec<u32>,
        /// Every day type of its day groups, sorted.
        day_types: Vec<u32>,
        /// Usable day descriptions preloaded.
        days: usize,
    },
    /// One day description (0x1A) with its timeline.
    Day {
        /// The record.
        day: Arc<DayDescription>,
        /// Its timeline over [`TIMELINE_SAMPLES`] samples ([`day_timeline_with`]).
        timeline: Arc<Timeline>,
    },
}

impl SkyProduct {
    /// Heap bytes, measured from the decoded data: for a sky description its record and
    /// every cached day description of its library ([`library_heap_bytes`]); for a day
    /// description the record ([`day_heap_bytes`]) and its timeline.
    #[must_use]
    pub fn heap_bytes(&self) -> usize {
        match self {
            Self::Sky {
                library,
                keys,
                day_types,
                ..
            } => library_heap_bytes(library) + (keys.len() + day_types.len()) * 4 + 256,
            Self::Day { day, timeline } => day_heap_bytes(day) + timeline.heap_bytes() + 256,
        }
    }
}

/// Builds [`SkyProduct`]s for 0x15 and 0x1A records.
#[derive(Debug, Clone)]
pub struct SkyDecoder(pub Arc<DataContext>);

impl Decoder for SkyDecoder {
    type Output = SkyProduct;
    type Error = ExplorerError;
    const NAME: &'static str = "explorer-sky";

    fn decode(
        &self,
        key: AssetKey,
        _: Vec<u8>,
        job: &JobContext<'_>,
    ) -> Result<SkyProduct, ExplorerError> {
        let did = Did(key.did);
        match key.did >> 24 {
            0x15 => {
                let library = SkyLibrary::load_from(self.0.world.source(), did)
                    .map_err(|e| ExplorerError::Worker(e.to_string()))?;
                let sky = library.sky();
                let keys: BTreeSet<u32> = sky
                    .groups
                    .iter()
                    .flat_map(|g| g.entries.iter().map(|e| e.sky_key))
                    .chain(sky.local_defaults.iter().map(|e| e.sky_key))
                    .collect();
                let day_types: BTreeSet<u32> = sky.groups.iter().map(|g| g.day_type).collect();
                let days = sky
                    .day_descriptions()
                    .collect::<BTreeSet<_>>()
                    .into_iter()
                    .filter(|d| library.cached_day(*d).flatten().is_some())
                    .count();
                Ok(SkyProduct::Sky {
                    keys: keys.into_iter().take(MAX_LISTED).collect(),
                    day_types: day_types.into_iter().take(MAX_LISTED).collect(),
                    days,
                    library: Arc::new(library),
                })
            }
            0x1A => {
                let bytes = self
                    .0
                    .world
                    .source()
                    .read(did)
                    .map_err(asset_error)?
                    .ok_or(ExplorerError::NotFound(did.0))?;
                let day = DayDescription::parse_expecting(&bytes, did)?;
                // The timeline is computed here, on the worker: its cost grows with the
                // record (bounded by `day_timeline_with`), so the view only draws it.
                let timeline = day_timeline_with(&day, TIMELINE_SAMPLES, &|| job.is_cancelled())
                    .ok_or(ExplorerError::Cancelled)?;
                Ok(SkyProduct::Day {
                    day: Arc::new(day),
                    timeline: Arc::new(timeline),
                })
            }
            _ => Err(ExplorerError::NotApplicable(format!(
                "{:#010x} is not a sky or day description",
                key.did
            ))),
        }
    }

    fn size_of(&self, v: &SkyProduct) -> usize {
        v.heap_bytes()
    }
}

/// The clock value *G* of time of day `t` (fraction) on game day `day_number`
/// (`environment.md` §11.1, inverted): the reading of the result has that day number and
/// time of day, within rounding.
#[must_use]
pub fn clock_value(cal: &Calendar, day_number: i64, t: f64) -> f64 {
    let r0 = cal.read(0.0);
    let days = day_number.saturating_sub(r0.day_number) as f64;
    (days - r0.time_of_day + t.clamp(0.0, 1.0)) * cal.day_length
}

#[cfg(test)]
mod tests {
    use super::*;
    use tessera_formats::LandblockData;
    use tessera_formats::landblock::{HeightGrid, Placement};
    use tessera_formats::setup::Quaternion;
    use tessera_physics::geometry::{
        BodyPurpose, CollisionFlags, CollisionLimits, CollisionSetBuilder, ETHEREAL_PLAYER,
        GridMesh, ShapeTemplate,
    };
    use tessera_world::water::DryReason;
    use tessera_world::{BlockCollision, BlockProps, BlockWater, DrawFlags, PlacedEntity};

    use crate::collision_overlay::{OverlayKind, landblock_overlays};

    #[test]
    fn clock_values_invert_the_reading() {
        let cal = Calendar {
            day_length: 100.0,
            days_per_year: 10,
            zero_offset: 1234.5,
            zero_year: 7,
            month_starts: vec![0, 5],
            night_start: 0.8,
            night_end: 0.25,
        };
        for day_number in [70_i64, 71, 79, 80, 123] {
            for t in [0.01, 0.5, 0.99] {
                let r = cal.read(clock_value(&cal, day_number, t));
                assert_eq!(r.day_number, day_number, "day {day_number} t {t}");
                assert!((r.time_of_day - t).abs() < 1e-9);
            }
        }
    }

    fn key() -> LandblockKey {
        LandblockKey::from_did(Did(0x8001_2AAB)).expect("key")
    }

    fn entity(index: u32, x: f32) -> PlacedEntity {
        PlacedEntity {
            index,
            iid: None,
            kind: None,
            object: Some(Did(0x4700_0100 + index)),
            placement: Placement::new([x, 20.0, 5.0], Quaternion::IDENTITY).expect("placement"),
            scale: [1.0; 3],
            interior: false,
            content_layers: None,
            own_draw: DrawFlags::default(),
            visual: None,
            draw: true,
            figure: None,
        }
    }

    fn triangle() -> ShapeTemplate {
        ShapeTemplate::from_triangle_list(
            &[[0.0, 0.0, 0.0], [1.0, 0.0, 0.0], [0.0, 1.0, 1.0]],
            &[[0, 1, 2]],
        )
        .expect("template")
    }

    /// Statics 0 (blocks), 1 (no body), 2 (ethereal to Player); generated objects 0
    /// (blocks), 1 (no body), 2 (ethereal to Player); flat terrain at height 0; dry.
    fn bundle(generated_count: usize) -> LandblockBundle {
        let key = key();
        let data = LandblockData::from_heights(key.data_did(), HeightGrid::flat(0));
        let ethereal = CollisionFlags {
            ethereal_to: Some(ETHEREAL_PLAYER),
            ..CollisionFlags::default()
        };
        let solid = CollisionFlags::default();
        let mut b = CollisionSetBuilder::new(CollisionLimits::default());
        b.add_grid(Arc::new(GridMesh::terrain(&data)))
            .expect("terrain");
        let t = triangle();
        let object = Did(0x4700_0001);
        for (index, flags) in [(0, solid), (2, ethereal)] {
            b.add_shape(
                BodySource::Static { index, object },
                flags,
                BodyPurpose::Movement,
                &t,
            )
            .expect("static");
        }
        let g0 = b
            .add_shape(
                BodySource::Generated { index: 0, object },
                solid,
                BodyPurpose::Movement,
                &t,
            )
            .expect("generated");
        let g2 = b
            .add_shape(
                BodySource::Generated { index: 2, object },
                ethereal,
                BodyPurpose::Movement,
                &t,
            )
            .expect("generated");
        let set = b.build();
        let mut generated_bodies = vec![Some(g0), None, Some(g2)];
        generated_bodies.resize(generated_count, None);
        LandblockBundle {
            key,
            data,
            data_stamp: None,
            info: None,
            block_map: None,
            block_props: BlockProps::default(),
            properties: None,
            minimap: None,
            light_map: None,
            ceiling_light_map: None,
            entities: (0..3).map(|i| entity(i, 10.0 * i as f32)).collect(),
            unplaced_entities: 0,
            figures: Vec::new(),
            figures_dropped: 0,
            area: None,
            generated: (0..generated_count)
                .map(|i| entity(u32::try_from(i).unwrap_or(u32::MAX), 50.0))
                .collect(),
            scenery: None,
            water: Some(Arc::new(BlockWater::dry(key, DryReason::NoTable))),
            collision: Some(Arc::new(BlockCollision {
                key,
                set: Arc::new(set),
                cells: Vec::new(),
                portals_left_out: 0,
                cell_bounds: None,
                generated_bodies,
                complete: true,
                heap_bytes: 0,
            })),
            lights: Vec::new(),
            entity_lod_classes: vec![Some(3), None, None],
            generated_lod_classes: Vec::new(),
            entity_depth_bias_types: Vec::new(),
            generated_depth_bias_types: Vec::new(),
            entity_render_opacities: Vec::new(),
            generated_render_opacities: Vec::new(),
            problems: Vec::new(),
            heap_bytes: 0,
        }
    }

    fn walker(rows: &[ObjectRow]) -> Vec<(bool, bool)> {
        rows.iter().map(|o| (o.collides, o.blocks)).collect()
    }

    #[test]
    fn objects_ethereal_to_player_do_not_block_a_walker() {
        // Regression: every object with a collision body was listed as blocking, although
        // the walker's filter passes through bodies ethereal to Player (collision.md §9.2).
        let b = bundle(3);
        let p = landblock_product(0x8001_2AAB, &b, 7);
        let expected = vec![(true, true), (false, false), (true, false)];
        assert_eq!(walker(&p.statics), expected);
        assert_eq!(walker(&p.generated), expected);
        assert_eq!(p.statics[0].lod_class, Some(3));
        assert_eq!(p.statics[1].object, Some(0x4700_0101));
        assert_eq!(p.heights.len(), 33 * 33);
        assert_eq!(p.range, (0.0, 0.0));
        assert!(p.ceiling.is_none() && p.water.is_none());
        assert_eq!(p.build_ms, 7);
        let row = |rows: &[(String, String)], k: &str| {
            rows.iter()
                .find(|(a, _)| a == k)
                .map(|(_, v)| v.clone())
                .unwrap_or_else(|| panic!("no row {k}"))
        };
        assert_eq!(row(&p.water_rows, "state"), "dry");
        assert_eq!(row(&p.water_rows, "reason"), "NoTable");
        assert_eq!(row(&p.collision, "statics blocking a walker"), "1");
        assert_eq!(row(&p.collision, "generated blocking a walker"), "1");
        assert_eq!(
            row(
                &p.collision,
                "movement bodies a walker passes through (ethereal to Player)"
            ),
            "2"
        );
        assert!(p.heap_bytes() > 33 * 33 * 4);

        // The 3D overlays put the pass-through bodies in their own group, hidden by
        // default, and count only blocking bodies as statics and generated objects.
        let o = landblock_overlays(&b);
        let caption = |k: OverlayKind| {
            o.iter()
                .find(|o| o.kind == k)
                .map(|o| o.caption.clone())
                .unwrap_or_else(|| panic!("no {k:?} overlay"))
        };
        assert_eq!(
            caption(OverlayKind::Statics),
            "statics: 1 bodies, 1 triangles"
        );
        assert_eq!(
            caption(OverlayKind::Generated),
            "generated objects: 1 bodies, 1 triangles"
        );
        assert!(caption(OverlayKind::PassThrough).contains("2 bodies, 2 triangles"));
        assert!(!OverlayKind::PassThrough.shown_by_default());
        assert!(caption(OverlayKind::Terrain).starts_with("terrain collision: 1 bodies"));
    }

    #[test]
    fn listed_objects_are_bounded() {
        let b = bundle(MAX_LISTED + 3);
        let p = landblock_product(0x8001_2AAB, &b, 0);
        assert_eq!(p.generated.len(), MAX_LISTED);
        assert_eq!(
            walker(&p.generated[..3]),
            vec![(true, true), (false, false), (true, false)]
        );
        assert!(p.generated[3..].iter().all(|o| !o.collides));
        let index = BodyIndex::new(&b.collision.as_ref().expect("c").set, &[None; 3]);
        assert_eq!(index.statics.len(), 2);
        assert_eq!(index.generated, vec![None; 3]);
    }
}
