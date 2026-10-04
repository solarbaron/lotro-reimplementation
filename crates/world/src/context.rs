// SPDX-License-Identifier: GPL-3.0-or-later
//! Per-install world data loaded once at start-up: the property schema, the property IDs
//! the world needs, and per region its descriptor, map-coordinate constants and block-map
//! descriptor (`landblock.md` §5.1, `environment.md` §3, `coordinates.md` §8.1).

use std::collections::{BTreeMap, HashMap};
use std::sync::Arc;

use tessera_dat::ArchiveSet;
use tessera_formats::{
    BlockMapDescriptor, DataIdMap, Did, PropertyDictionary, PropertySchema, Record,
    RegionDescriptor,
};

use crate::areas::AreaNameSource;
use crate::bundle::BundleOptions;
use crate::coords::{LandblockKey, MAX_REGION};
use crate::error::{Result, WorldError};
use crate::mapcoords::MapConstants;
use crate::objects::{ObjectResolver, WorldPids};
use crate::source::WorldSource;

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
}

impl std::fmt::Debug for WorldContext {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("WorldContext")
            .field("regions", &self.regions.keys().collect::<Vec<_>>())
            .field("pids", &self.pids)
            .field("objects", &self.objects)
            .field("area_names", &self.areas.is_some())
            .field("options", &self.options)
            .finish_non_exhaustive()
    }
}

impl WorldContext {
    /// A context from parts (synthetic worlds and tests).
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
        }
    }

    /// Loads the context from an install's records: the property dictionary
    /// (0x34000000), the REGION DID map named by the root map, every region descriptor it
    /// lists, and their block-map descriptors. Regions whose descriptors fail to decode are
    /// logged and left without map constants or block-map descriptor.
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
        let schema: Arc<dyn PropertySchema + Send + Sync> = Arc::new(dict);
        let regions = load_regions(&*source, &*schema);
        tracing::info!(regions = regions.len(), "world context loaded");
        Ok(Self::new(source, schema, pids, regions, options))
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
