// SPDX-License-Identifier: GPL-3.0-or-later
//! Per-install world data loaded once at start-up: the property schema, the property IDs
//! the world needs, per region its descriptor, map-coordinate constants, block-map
//! descriptor and the tables its eight slots name (`landblock.md` §5.1, `environment.md`
//! §3, `coordinates.md` §8.1), and the shared state of the M4.5 world systems
//! ([`WorldSystems`]: scene records, object properties, water, collision, sky, LOD classes).
//!
//! The eight slots of a region descriptor (`environment.md` §3.1–§3.2) are read at run
//! time, never hard-coded:
//!
//! | Slot | Table | Loaded into |
//! |---|---|---|
//! | 1, 2, 8 | scene-choice 0x11, terrain-scene 0x12, terrain type 0x19 | [`RegionInfo::scenery`] |
//! | 3 | encounter 0x14 | not loaded (the encounter pass is out of scope, `scenery.md` §7.7) |
//! | 4 | sky description 0x15 | [`WorldSystems::sky`] (one record shared by every region in this install) |
//! | 5, 6 | water 0x16, fog zones 0x17 | [`WorldSystems::water`] |
//! | 7 | block-map descriptor 0x18 | [`RegionInfo::block_map_descriptor`] |

use std::collections::{BTreeMap, HashMap};
use std::sync::Arc;

use tessera_dat::ArchiveSet;
use tessera_formats::{
    BlockMapDescriptor, DataIdMap, Did, PropertyDictionary, PropertySchema, Record,
    RegionDescriptor,
};
use tessera_physics::geometry::{PhysicsPids, StaticOptions};

use crate::areas::AreaNameSource;
use crate::bundle::BundleOptions;
use crate::collision::CollisionSystem;
use crate::coords::{LandblockKey, MAX_REGION};
use crate::draw_distance::LodClasses;
use crate::environment::{EnvironmentPids, SkyLibrary};
use crate::error::{Result, WorldError};
use crate::frills::FrillPids;
use crate::lighting::LightPids;
use crate::mapcoords::MapConstants;
use crate::objects::{ObjectResolver, WorldPids};
use crate::scenery::{ClearancePids, ObjectPropertyCache, SceneLibrary, SceneryTables};
use crate::source::WorldSource;
use crate::water::{
    DEFAULT_VOLUME_CACHE_ENTRIES, VolumeLibrary, WaterPids, WaterSystem, WaterTables,
};

/// DID of the master property dictionary (`property-system.md` §2).
pub const PROPERTY_DICTIONARY_DID: u32 = 0x3400_0000;
/// DID of the root DID map (`property-system.md` §8).
pub const ROOT_DID_MAP: u32 = 0x2800_0000;
/// Label of the REGION sub-map in the root DID map (`property-system.md` §8.1).
pub const REGION_MAP_LABEL: &str = "REGION";

/// What the world knows about one region.
#[derive(Debug, Clone)]
pub struct RegionInfo {
    /// Region number *R*.
    pub region: u8,
    /// The region descriptor (0x0F), when one was found.
    pub descriptor: Option<RegionDescriptor>,
    /// Map-coordinate constants, when the descriptor gave valid ones.
    pub map: Option<MapConstants>,
    /// The block-map descriptor (0x18) named by the region descriptor.
    pub block_map_descriptor: Option<Arc<BlockMapDescriptor>>,
    /// The scenery tables named by slots 1, 2 and 8 (`scenery.md` §4.1), when
    /// [`BundleOptions::generate_scenery`] is set and they load.
    pub scenery: Option<Arc<SceneryTables>>,
}

/// Property IDs of the M4.5 world systems, looked up by name in the property dictionary.
#[derive(Debug, Clone, Copy, Default, PartialEq, Eq)]
pub struct SystemPids {
    /// Collision flags (`collision.md` §9.1).
    pub physics: PhysicsPids,
    /// Clearance volumes of the static list (`scenery.md` §7.4 step 14a).
    pub clearance: ClearancePids,
    /// Sky keys and interior fog (`environment.md` §7).
    pub environment: EnvironmentPids,
    /// Light entities (`lighting.md` §8.3).
    pub lights: LightPids,
    /// Frill object values (`scenery.md` §8.3).
    pub frills: FrillPids,
    /// Monitors and liquid flags (`water.md` §7).
    pub water: WaterPids,
    /// `Render_LODClass` (`walker.md` §11.4.3).
    pub lod_class: Option<u32>,
}

impl SystemPids {
    /// Looks every property up with `pid_of` (name → PID).
    #[must_use]
    pub fn from_names(pid_of: impl Fn(&str) -> Option<u32> + Copy) -> Self {
        Self {
            physics: PhysicsPids::from_names(pid_of),
            clearance: ClearancePids::from_names(pid_of),
            environment: EnvironmentPids::from_names(pid_of),
            lights: LightPids::from_names(pid_of),
            frills: FrillPids::from_names(pid_of),
            water: WaterPids::from_names(pid_of),
            lod_class: pid_of("Render_LODClass"),
        }
    }
}

/// The shared state of the M4.5 world systems: read once, used by the bundle decoder on
/// every worker and by the streamer's lookups. Each part is `None` when its option is off
/// or its records could not be loaded (logged); the bundle then goes without it.
#[derive(Debug, Default)]
pub struct WorldSystems {
    /// Property IDs.
    pub pids: SystemPids,
    /// Scene records (0x02, `scenery.md` §5), cached; present when any region has scenery
    /// tables.
    pub scenes: Option<Arc<SceneLibrary>>,
    /// 0x47 properties resolved along their chains (`scenery.md` §9.2), shared by the
    /// clearance list, LOD classes and frills.
    pub object_properties: Arc<ObjectPropertyCache>,
    /// Water tables (slots 5 and 6) and liquid volumes (`water.md` §4–§7); loaded when
    /// water is built or scenery generated (which reads the water plane).
    pub water: Option<Arc<WaterSystem>>,
    /// Collision caches and limits (`collision.md` §12.1).
    pub collision: Option<Arc<CollisionSystem>>,
    /// The sky description (slot 4), its game time and day descriptions
    /// (`environment.md` §5, §6, §11).
    pub sky: Option<Arc<SkyLibrary>>,
    /// The LOD-class table (0x32, `walker.md` §11.4).
    pub lod_classes: Option<Arc<LodClasses>>,
}

/// Shared, immutable world context used by the bundle decoder on every worker.
pub struct WorldContext {
    source: Arc<dyn WorldSource>,
    schema: Arc<dyn PropertySchema + Send + Sync>,
    pids: WorldPids,
    regions: BTreeMap<u8, RegionInfo>,
    objects: ObjectResolver,
    areas: Option<Arc<dyn AreaNameSource>>,
    options: BundleOptions,
    systems: WorldSystems,
}

impl std::fmt::Debug for WorldContext {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("WorldContext")
            .field("regions", &self.regions.keys().collect::<Vec<_>>())
            .field("pids", &self.pids)
            .field("objects", &self.objects)
            .field("area_names", &self.areas.is_some())
            .field("options", &self.options)
            .field("systems", &self.systems)
            .finish_non_exhaustive()
    }
}

impl WorldContext {
    /// A context from parts (synthetic worlds and tests), without world systems (see
    /// [`WorldContext::load_systems`]).
    #[must_use]
    pub fn new(
        source: Arc<dyn WorldSource>,
        schema: Arc<dyn PropertySchema + Send + Sync>,
        pids: WorldPids,
        regions: impl IntoIterator<Item = RegionInfo>,
        options: BundleOptions,
    ) -> Self {
        let objects = ObjectResolver::new(
            Arc::clone(&source),
            Arc::clone(&schema),
            pids.clone(),
            options.object_cache_entries.max(1),
        );
        Self {
            source,
            schema,
            pids,
            regions: regions.into_iter().map(|r| (r.region, r)).collect(),
            objects,
            areas: None,
            options,
            systems: WorldSystems::default(),
        }
    }

    /// Loads the context from an install's records: the property dictionary
    /// (0x34000000), the REGION DID map named by the root map, every region descriptor it
    /// lists, and their block-map descriptors. Regions whose descriptors fail to decode are
    /// logged and left without map constants or block-map descriptor. Then the world
    /// systems are loaded as [`WorldContext::load_systems`] describes.
    ///
    /// # Errors
    ///
    /// When the property dictionary is missing or does not decode. A missing REGION map
    /// is not an error (the world then has no map coordinates or block-map values).
    pub fn load(source: Arc<dyn WorldSource>, options: BundleOptions) -> Result<Self> {
        let dict_bytes = source
            .read(Did(PROPERTY_DICTIONARY_DID))?
            .ok_or(WorldError::Missing {
                did: PROPERTY_DICTIONARY_DID,
                what: "property dictionary",
            })?;
        let dict = PropertyDictionary::parse_expecting(&dict_bytes, PROPERTY_DICTIONARY_DID)
            .map_err(|e| WorldError::format(PROPERTY_DICTIONARY_DID, "property dictionary", e))?;
        let pids = WorldPids::from_names(|n| dict.pid(n));
        let system_pids = SystemPids::from_names(|n| dict.pid(n));
        let schema: Arc<dyn PropertySchema + Send + Sync> = Arc::new(dict);
        let regions = load_regions(&*source, &*schema);
        tracing::info!(regions = regions.len(), "world context loaded");
        let mut ctx = Self::new(source, schema, pids, regions, options);
        ctx.load_systems(system_pids);
        Ok(ctx)
    }

    /// Loads the world systems with property IDs `pids`, as the options say:
    ///
    /// - [`BundleOptions::generate_scenery`]: every region's scenery tables (slots 1, 2, 8)
    ///   and the scene library;
    /// - [`BundleOptions::build_water`] or [`BundleOptions::generate_scenery`]: the water
    ///   and fog-zone tables (slots 5, 6) and the volume library (scenery generation reads
    ///   the landblock's water plane, `scenery.md` §11, whether or not water is built);
    /// - [`BundleOptions::build_collision`]: the collision caches with
    ///   [`BundleOptions::collision_limits`] and the active content layers;
    /// - [`BundleOptions::load_environment`]: the sky description (slot 4) with its game
    ///   time and day descriptions, and the LOD-class table.
    ///
    /// A part whose records are missing or broken is logged and left `None`; the rest
    /// still loads. Replaces any systems loaded before.
    pub fn load_systems(&mut self, pids: SystemPids) {
        let mut systems = WorldSystems {
            pids,
            ..WorldSystems::default()
        };
        if self.options.generate_scenery {
            let tables = SceneryTables::for_all_regions(self);
            for (region, info) in &mut self.regions {
                info.scenery = tables.get(region).cloned();
            }
            if !tables.is_empty() {
                systems.scenes = Some(Arc::new(SceneLibrary::new(Arc::clone(&self.source))));
            }
        }
        if self.options.build_water || self.options.generate_scenery {
            systems.water = Some(Arc::new(WaterSystem::new(
                WaterTables::load(self),
                VolumeLibrary::new(pids.water, DEFAULT_VOLUME_CACHE_ENTRIES),
            )));
        }
        if self.options.build_collision {
            systems.collision = Some(Arc::new(CollisionSystem::new(
                pids.physics,
                self.options.collision_limits,
                StaticOptions {
                    active_content_layers: self.options.active_content_layers.clone(),
                    instance: 0,
                },
            )));
        }
        if self.options.load_environment {
            systems.sky = match SkyLibrary::load(self) {
                Ok(s) => Some(Arc::new(s)),
                Err(e) => {
                    tracing::warn!(error = %e, "no sky: environment lookups unavailable");
                    None
                }
            };
            systems.lod_classes = match LodClasses::load(self) {
                Ok(t) => Some(Arc::new(t)),
                Err(e) => {
                    tracing::warn!(error = %e, "no LOD-class table: component defaults apply");
                    None
                }
            };
        }
        tracing::debug!(
            scenery_regions = self
                .regions
                .values()
                .filter(|r| r.scenery.is_some())
                .count(),
            water = systems.water.is_some(),
            collision = systems.collision.is_some(),
            sky = systems.sky.is_some(),
            lod_classes = systems.lod_classes.is_some(),
            "world systems loaded"
        );
        self.systems = systems;
    }

    /// Replaces the world systems (synthetic worlds and tests).
    #[must_use]
    pub fn with_systems(mut self, systems: WorldSystems) -> Self {
        self.systems = systems;
        self
    }

    /// The world systems.
    #[must_use]
    pub fn systems(&self) -> &WorldSystems {
        &self.systems
    }

    /// [`WorldContext::load`] over an opened install.
    ///
    /// # Errors
    ///
    /// As [`WorldContext::load`].
    pub fn from_install(set: Arc<ArchiveSet>, options: BundleOptions) -> Result<Self> {
        Self::load(set, options)
    }

    /// Attaches an area-name resolver (used by the bundle decoder).
    #[must_use]
    pub fn with_area_names(mut self, areas: Arc<dyn AreaNameSource>) -> Self {
        self.areas = Some(areas);
        self
    }

    /// The record source.
    #[must_use]
    pub fn source(&self) -> &Arc<dyn WorldSource> {
        &self.source
    }

    /// The property schema.
    #[must_use]
    pub fn schema(&self) -> &(dyn PropertySchema + Send + Sync) {
        &*self.schema
    }

    /// The property IDs in use.
    #[must_use]
    pub fn pids(&self) -> &WorldPids {
        &self.pids
    }

    /// Every region known.
    #[must_use]
    pub fn regions(&self) -> &BTreeMap<u8, RegionInfo> {
        &self.regions
    }

    /// One region.
    #[must_use]
    pub fn region(&self, region: u8) -> Option<&RegionInfo> {
        self.regions.get(&region)
    }

    /// The map-coordinate constants of `region`.
    #[must_use]
    pub fn map_constants(&self, region: u8) -> Option<MapConstants> {
        self.regions.get(&region).and_then(|r| r.map)
    }

    /// The object resolver.
    #[must_use]
    pub fn objects(&self) -> &ObjectResolver {
        &self.objects
    }

    /// The area-name resolver, if attached.
    #[must_use]
    pub fn area_names(&self) -> Option<&Arc<dyn AreaNameSource>> {
        self.areas.as_ref()
    }

    /// Bundle options.
    #[must_use]
    pub fn options(&self) -> &BundleOptions {
        &self.options
    }

    /// Whether landblock (`region`, X, Y) has a data record, by
    /// [`WorldSource::contains`]: a directory lookup for an install (the record is not
    /// read), on the calling thread; for tools, not the frame loop.
    ///
    /// # Errors
    ///
    /// Read errors of the source.
    pub fn has_landblock(&self, key: LandblockKey) -> Result<bool> {
        Ok(self.source.contains(key.data_did())?)
    }
}

/// Reads the REGION map and every region descriptor it lists (see [`WorldContext::load`]).
fn load_regions(source: &dyn WorldSource, schema: &dyn PropertySchema) -> Vec<RegionInfo> {
    let map = match region_map(source) {
        Ok(Some(m)) => m,
        Ok(None) => {
            tracing::warn!("no REGION DID map; map coordinates and block-map values unavailable");
            return Vec::new();
        }
        Err(e) => {
            tracing::warn!(error = %e, "REGION DID map unreadable");
            return Vec::new();
        }
    };
    let mut descriptors: HashMap<Did, Option<Arc<BlockMapDescriptor>>> = HashMap::new();
    let mut out: BTreeMap<u8, RegionInfo> = BTreeMap::new();
    // At most a few dozen entries; bounded by the map's own decoded size.
    for entry in map.iter().take(256) {
        let did = entry.value;
        let desc = match source.read(did) {
            Ok(Some(b)) => match RegionDescriptor::parse_expecting(&b, did) {
                Ok(d) => d,
                Err(e) => {
                    tracing::warn!(did = %format_args!("{:#010x}", did.0), error = %e, "region descriptor does not decode");
                    continue;
                }
            },
            Ok(None) => {
                tracing::debug!(did = %format_args!("{:#010x}", did.0), "region descriptor named by the REGION map is absent");
                continue;
            }
            Err(e) => {
                tracing::warn!(did = %format_args!("{:#010x}", did.0), error = %e, "region descriptor unreadable");
                continue;
            }
        };
        let Some(region) = u8::try_from(desc.region)
            .ok()
            .filter(|r| (1..=MAX_REGION).contains(r))
        else {
            tracing::warn!(
                region = desc.region,
                "region descriptor names an unusable region"
            );
            continue;
        };
        let map = match MapConstants::from_region_descriptor(&desc) {
            Ok(m) => Some(m),
            Err(e) => {
                tracing::warn!(region, error = %e, "region map constants rejected");
                None
            }
        };
        let block_map_descriptor = desc.block_map_descriptor().and_then(|bd| {
            descriptors
                .entry(bd)
                .or_insert_with(|| load_block_map_descriptor(source, schema, bd))
                .clone()
        });
        if out.contains_key(&region) {
            tracing::warn!(
                region,
                "two region descriptors for one region; keeping the first"
            );
            continue;
        }
        out.insert(
            region,
            RegionInfo {
                region,
                descriptor: Some(desc),
                map,
                block_map_descriptor,
                scenery: None,
            },
        );
    }
    out.into_values().collect()
}

fn region_map(source: &dyn WorldSource) -> Result<Option<DataIdMap>> {
    let Some(root) = source.read(Did(ROOT_DID_MAP))? else {
        return Ok(None);
    };
    let root = DataIdMap::parse_expecting(&root, ROOT_DID_MAP)
        .map_err(|e| WorldError::format(ROOT_DID_MAP, "root DID map", e))?;
    let Some(did) = root.by_label(REGION_MAP_LABEL) else {
        return Ok(None);
    };
    let Some(bytes) = source.read(did)? else {
        return Ok(None);
    };
    DataIdMap::parse_expecting(&bytes, did)
        .map(Some)
        .map_err(|e| WorldError::format(did.0, "REGION DID map", e))
}

fn load_block_map_descriptor(
    source: &dyn WorldSource,
    schema: &dyn PropertySchema,
    did: Did,
) -> Option<Arc<BlockMapDescriptor>> {
    match source.read(did) {
        Ok(Some(b)) => match BlockMapDescriptor::parse_expecting(&b, did, schema) {
            Ok(d) => Some(Arc::new(d)),
            Err(e) => {
                tracing::warn!(did = %format_args!("{:#010x}", did.0), error = %e, "block-map descriptor does not decode");
                None
            }
        },
        Ok(None) => {
            tracing::warn!(did = %format_args!("{:#010x}", did.0), "block-map descriptor missing");
            None
        }
        Err(e) => {
            tracing::warn!(did = %format_args!("{:#010x}", did.0), error = %e, "block-map descriptor unreadable");
            None
        }
    }
}
