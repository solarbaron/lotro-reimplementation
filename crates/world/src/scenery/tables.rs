// SPDX-License-Identifier: GPL-3.0-or-later
//! The per-region scene tables (`scenery.md` §4: slot 1 scene-choice table 0x11, slot 2
//! terrain-scene table 0x12, slot 8 terrain-type table 0x19) and the scene records (0x02)
//! that generation reads.

use std::collections::{BTreeMap, BTreeSet, HashMap};
use std::sync::{Arc, Mutex, PoisonError};

use tessera_formats::scenery::{
    Scene, SceneChoiceEntry, SceneChoiceTable, TerrainSceneTable, WORD_SLOT_MASK, WORD_SLOT_SHIFT,
    WORD_TERRAIN_MASK,
};
use tessera_formats::terrain_type::TerrainTypeTable;
use tessera_formats::{Did, DidKind, Record, RegionDescriptor};

use super::error::SceneryError;
use super::hash;
use crate::context::{RegionInfo, WorldContext};
use crate::source::WorldSource;

/// Default bound of a [`SceneLibrary`] (the install has 3,402 scene records).
pub const DEFAULT_SCENE_CACHE_ENTRIES: usize = 16_384;

/// The three region tables scenery generation reads (`scenery.md` §4.1).
#[derive(Debug, Clone)]
pub struct SceneryTables {
    /// Region number *R*.
    pub region: u8,
    /// Slot 2: terrain-scene table (0x12).
    pub terrain_scenes: Arc<TerrainSceneTable>,
    /// Slot 1: scene-choice table (0x11).
    pub scene_choices: Arc<SceneChoiceTable>,
    /// Slot 8: terrain-type table (0x19). Without it the terrain-type test of §7.4 step 7
    /// finds no key and passes.
    pub terrain_types: Option<Arc<TerrainTypeTable>>,
}

/// The result of [`SceneryTables::load_all_regions`].
#[derive(Debug, Clone, Default)]
pub struct RegionSceneryTables {
    /// The tables of every region that has them.
    pub tables: BTreeMap<u8, Arc<SceneryTables>>,
    /// Regions whose tables could not be read for a reason that is not the data's (full
    /// queue, shutdown, worker panic); load them again later.
    pub retry: BTreeSet<u8>,
}

/// How a coarse-grid word resolves to a scene (`scenery.md` §4.2–§4.3, §7.2 steps 1–2).
#[derive(Debug, Clone, Copy, PartialEq)]
pub enum VertexLookup<'t> {
    /// The terrain entry has no pair for the slot and no default pair (the client logs a
    /// diagnostic; §12 counts 7,295 such vertices).
    NoPair,
    /// The pair's scene key is 0.
    KeyZero,
    /// The key is not in the scene-choice table.
    MissingKey(u32),
    /// The scene-choice entry of the key.
    Entry(&'t SceneChoiceEntry),
}

impl SceneryTables {
    /// Tables from parts (tests and tools).
    #[must_use]
    pub fn new(
        region: u8,
        terrain_scenes: Arc<TerrainSceneTable>,
        scene_choices: Arc<SceneChoiceTable>,
        terrain_types: Option<Arc<TerrainTypeTable>>,
    ) -> Self {
        Self {
            region,
            terrain_scenes,
            scene_choices,
            terrain_types,
        }
    }

    /// Reads the tables a region descriptor names in slots 1, 2 and 8 (`scenery.md`
    /// §4.1; the DIDs are read at run time, never hard-coded). A missing or undecodable
    /// terrain-type table is logged and left out.
    ///
    /// # Errors
    ///
    /// When slot 1 or 2 is empty, or its record is missing or does not decode; when any
    /// of the three reads fails for a reason that is not the data's
    /// ([`SceneryError::is_data_error`] false).
    pub fn load(
        source: &dyn WorldSource,
        region: u8,
        descriptor: &RegionDescriptor,
    ) -> Result<Self, SceneryError> {
        let terrain_did = descriptor
            .terrain_scene_table()
            .ok_or(SceneryError::MissingTable {
                region,
                what: "terrain-scene table (slot 2)",
            })?;
        let choice_did = descriptor
            .scene_choice_table()
            .ok_or(SceneryError::MissingTable {
                region,
                what: "scene-choice table (slot 1)",
            })?;
        let terrain_scenes: TerrainSceneTable =
            read_record(source, terrain_did, "terrain-scene table")?;
        let scene_choices: SceneChoiceTable =
            read_record(source, choice_did, "scene-choice table")?;
        let terrain_types = match descriptor.terrain_type_table() {
            Some(did) => match read_record::<TerrainTypeTable>(source, did, "terrain-type table") {
                Ok(t) => Some(Arc::new(t)),
                // A pipeline failure is not a property of the data: the caller retries.
                Err(e) if !e.is_data_error() => return Err(e),
                Err(e) => {
                    tracing::warn!(region, error = %e, "terrain-type table unusable; procedural flags ignored");
                    None
                }
            },
            None => {
                tracing::warn!(
                    region,
                    "no terrain-type table (slot 8); procedural flags ignored"
                );
                None
            }
        };
        Ok(Self::new(
            region,
            Arc::new(terrain_scenes),
            Arc::new(scene_choices),
            terrain_types,
        ))
    }

    /// [`SceneryTables::load`] for a region of the world context.
    ///
    /// # Errors
    ///
    /// As [`SceneryTables::load`]; [`SceneryError::MissingTable`] when the region has no
    /// descriptor.
    pub fn for_region(ctx: &WorldContext, region: u8) -> Result<Self, SceneryError> {
        // The region as loaded: `WorldContext::region` itself calls this function for its
        // retry.
        let info: &RegionInfo = ctx
            .regions()
            .get(&region)
            .ok_or(SceneryError::MissingTable {
                region,
                what: "region descriptor",
            })?;
        let desc = info.descriptor.as_ref().ok_or(SceneryError::MissingTable {
            region,
            what: "region descriptor",
        })?;
        Self::load(&**ctx.source(), region, desc)
    }

    /// The tables of every region of the context that has them, with the regions whose
    /// tables could not be read for a reason outside the data reported for a retry.
    ///
    /// - A region whose slot-1 or slot-2 table is missing, empty or broken is a property of
    ///   the data (`scenery.md` §4.1 Tessera Decision): it is logged and left out of both
    ///   sets, and gets no scenery or frills.
    /// - A region whose slot-1, slot-2 or slot-8 read fails for a reason that is not the
    ///   data's ([`SceneryError::is_data_error`] false: the asset pipeline's queue is full,
    ///   it shut down, a worker panicked) is left out of [`RegionSceneryTables::tables`]
    ///   and listed in [`RegionSceneryTables::retry`]. Nothing about the region is
    ///   remembered; the caller loads it again later ([`SceneryTables::for_region`]).
    ///   [`WorldContext::load_systems`] does so on request ([`WorldContext::region`]).
    ///   `scenery.md` is silent on table reads; this follows its §7.4 step 15 Tessera
    ///   Decision for scene reads (implementer note in `scenery.md` §4.1).
    #[must_use]
    pub fn load_all_regions(ctx: &WorldContext) -> RegionSceneryTables {
        let mut out = RegionSceneryTables::default();
        for &region in ctx.regions().keys() {
            match Self::for_region(ctx, region) {
                Ok(t) => {
                    out.tables.insert(region, Arc::new(t));
                }
                Err(e) if e.is_data_error() => {
                    tracing::warn!(region, error = %e, "no scenery tables");
                }
                Err(e) => {
                    tracing::warn!(region, error = %e, "scenery tables unavailable; retry later");
                    out.retry.insert(region);
                }
            }
        }
        out
    }

    /// The tables of every region of the context that has them
    /// ([`SceneryTables::load_all_regions`] without the retry list). A region whose tables
    /// are missing or broken is logged and left out; so is a region whose read failed for
    /// a reason outside the data, which is logged as retryable but not retried. Callers
    /// that live longer than one load should use [`SceneryTables::load_all_regions`] and
    /// retry those, as [`WorldContext::load_systems`] does.
    #[must_use]
    pub fn for_all_regions(ctx: &WorldContext) -> BTreeMap<u8, Arc<Self>> {
        Self::load_all_regions(ctx).tables
    }

    /// Resolves a coarse-grid word to its scene-choice entry (`scenery.md` §4.2 lookup,
    /// §7.2 steps 1–2).
    #[must_use]
    pub fn lookup(&self, word: u32) -> VertexLookup<'_> {
        let terrain = word & WORD_TERRAIN_MASK;
        let slot = (word >> WORD_SLOT_SHIFT) & WORD_SLOT_MASK;
        let Some(pair) = self.terrain_scenes.pair_for(terrain, slot) else {
            return VertexLookup::NoPair;
        };
        if pair.scene_key == 0 {
            return VertexLookup::KeyZero;
        }
        match self.scene_choices.get(pair.scene_key) {
            Some(e) => VertexLookup::Entry(e),
            None => VertexLookup::MissingKey(pair.scene_key),
        }
    }

    /// The scene chosen for coarse vertex (i, j) of landblock (X, Y) with scenery word
    /// `word` (`scenery.md` §6.1, §7.2): `None` when the word resolves to no entry.
    #[must_use]
    pub fn choose_scene(&self, x: u8, y: u8, i: u32, j: u32, word: u32) -> Option<Did> {
        let VertexLookup::Entry(entry) = self.lookup(word) else {
            return None;
        };
        let gx = 16 * u32::from(x) + i;
        let gy = 16 * u32::from(y) + j;
        entry.choose(hash::vertex_fraction(gx, gy)).map(|c| c.scene)
    }

    /// Whether procedural content may be placed on surface-layer material `key`
    /// (`scenery.md` §4.4): `None` when the key is not in the terrain-type table or there
    /// is no table.
    #[must_use]
    pub fn procedural_allowed(&self, key: u32) -> Option<bool> {
        self.terrain_types.as_ref()?.allows_procedural(key)
    }
}

fn read_record<T: for<'a> Record<'a>>(
    source: &dyn WorldSource,
    did: Did,
    what: &'static str,
) -> Result<T, SceneryError> {
    let bytes = source
        .read(did)?
        .ok_or(SceneryError::MissingRecord { did: did.0, what })?;
    T::parse_expecting(&bytes, did).map_err(|e| SceneryError::format(did.0, what, e))
}

/// Where generation gets scene records (0x02) from. Implementations are shared by every
/// worker.
pub trait SceneSource: Send + Sync {
    /// The scene `did`: `Ok(None)` when it is missing, unreadable or does not decode (a
    /// property of the data, the same every time). Either way the child that references it
    /// **fails** (`scenery.md` §7.4 step 15, S41): its node stops and the failure climbs to
    /// the vertex, whose remaining scene is abandoned (in the frill pass it stops at the
    /// innermost frill layer).
    ///
    /// # Errors
    ///
    /// When the scene could not be read for a reason that is not the data's (the asset
    /// pipeline's queue is full, it shut down, a worker panicked;
    /// [`SceneryError::is_data_error`] false). The child fails in the same way, and
    /// generation also counts
    /// [`SceneryStats::scenes_unavailable`](super::SceneryStats::scenes_unavailable): its
    /// result is incomplete and the caller should build the landblock again later.
    fn scene(&self, did: Did) -> Result<Option<Arc<Scene>>, SceneryError>;
}

impl SceneSource for HashMap<Did, Arc<Scene>> {
    fn scene(&self, did: Did) -> Result<Option<Arc<Scene>>, SceneryError> {
        Ok(self.get(&did).cloned())
    }
}

impl SceneSource for BTreeMap<Did, Arc<Scene>> {
    fn scene(&self, did: Did) -> Result<Option<Arc<Scene>>, SceneryError> {
        Ok(self.get(&did).cloned())
    }
}

/// Scene records read on demand from a [`WorldSource`] and cached (missing and broken
/// records too, as `None`; failures that are not the data's are returned uncached, see
/// [`SceneSource::scene`]). The cache is bounded by entry count; when full it is emptied.
pub struct SceneLibrary {
    source: Arc<dyn WorldSource>,
    cache: Mutex<HashMap<Did, Option<Arc<Scene>>>>,
    max_entries: usize,
}

impl std::fmt::Debug for SceneLibrary {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("SceneLibrary")
            .field("cached", &self.len())
            .field("max_entries", &self.max_entries)
            .finish_non_exhaustive()
    }
}

impl SceneLibrary {
    /// A library over `source` with [`DEFAULT_SCENE_CACHE_ENTRIES`].
    #[must_use]
    pub fn new(source: Arc<dyn WorldSource>) -> Self {
        Self::with_capacity(source, DEFAULT_SCENE_CACHE_ENTRIES)
    }

    /// A library whose cache holds at most `max_entries` (at least 1) scenes.
    #[must_use]
    pub fn with_capacity(source: Arc<dyn WorldSource>, max_entries: usize) -> Self {
        Self {
            source,
            cache: Mutex::new(HashMap::new()),
            max_entries: max_entries.max(1),
        }
    }

    /// Reads and decodes scene `did`, bypassing the cache.
    ///
    /// # Errors
    ///
    /// When `did` is not a scene DID, the record is missing, unreadable or does not
    /// decode.
    pub fn load(&self, did: Did) -> Result<Scene, SceneryError> {
        if did.kind() != DidKind::Scene {
            return Err(SceneryError::MissingRecord {
                did: did.0,
                what: "scene",
            });
        }
        read_record(&*self.source, did, "scene")
    }

    /// Number of cached entries.
    #[must_use]
    pub fn len(&self) -> usize {
        self.cache
            .lock()
            .unwrap_or_else(PoisonError::into_inner)
            .len()
    }

    /// Whether the cache is empty.
    #[must_use]
    pub fn is_empty(&self) -> bool {
        self.len() == 0
    }
}

impl SceneSource for SceneLibrary {
    fn scene(&self, did: Did) -> Result<Option<Arc<Scene>>, SceneryError> {
        if let Some(hit) = self
            .cache
            .lock()
            .unwrap_or_else(PoisonError::into_inner)
            .get(&did)
        {
            return Ok(hit.clone());
        }
        let loaded = match self.load(did) {
            Ok(s) => Some(Arc::new(s)),
            Err(e) if e.is_data_error() => {
                tracing::debug!(did = %format_args!("{:#010x}", did.0), error = %e, "scene unusable");
                None
            }
            Err(e) => {
                tracing::warn!(did = %format_args!("{:#010x}", did.0), error = %e, "scene unavailable (not cached)");
                return Err(e);
            }
        };
        let mut cache = self.cache.lock().unwrap_or_else(PoisonError::into_inner);
        if cache.len() >= self.max_entries {
            cache.clear();
        }
        cache.insert(did, loaded.clone());
        Ok(loaded)
    }
}
