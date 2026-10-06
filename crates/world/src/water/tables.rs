// SPDX-License-Identifier: GPL-3.0-or-later
//! The region water tables (slot 5, prefix 0x16) and fog-zone tables (slot 6, prefix
//! 0x17), read from the region descriptors (`water.md` §3), and the fog-zone corner lookup
//! (§5.3).

use std::collections::{BTreeMap, HashMap};
use std::sync::Arc;

use tessera_formats::landblock::HeightGrid;
use tessera_formats::water::{
    FOG_ZONE_TABLE_PREFIX, FogZone, FogZoneTable, WATER_TABLE_PREFIX, WaterTable,
    decode_fog_zone_table, decode_water_table,
};
use tessera_formats::{BlockMap, Did};

use super::decide::{DryReason, WaterDecision, decide_water};
use crate::context::WorldContext;
use crate::coords::LandblockKey;
use crate::source::WorldSource;

/// The water and fog-zone tables of every region (`water.md` §3): the DIDs come from each
/// region descriptor's slots 5 and 6 at run time. A table that is missing or does not
/// decode is logged and left out, so its region has no water (§6.2 step 1) or no fog zones.
#[derive(Debug, Clone, Default)]
pub struct WaterTables {
    water: BTreeMap<u8, Arc<WaterTable>>,
    fog: BTreeMap<u8, Arc<FogZoneTable>>,
}

impl WaterTables {
    /// Tables from parts (synthetic worlds and tests).
    #[must_use]
    pub fn new(
        water: impl IntoIterator<Item = (u8, Arc<WaterTable>)>,
        fog: impl IntoIterator<Item = (u8, Arc<FogZoneTable>)>,
    ) -> Self {
        Self {
            water: water.into_iter().collect(),
            fog: fog.into_iter().collect(),
        }
    }

    /// Reads the tables every region descriptor of `ctx` names. Regions sharing a table
    /// share one decoded copy.
    #[must_use]
    pub fn load(ctx: &WorldContext) -> Self {
        let source = &**ctx.source();
        let mut water_cache: HashMap<Did, Option<Arc<WaterTable>>> = HashMap::new();
        let mut fog_cache: HashMap<Did, Option<Arc<FogZoneTable>>> = HashMap::new();
        let mut out = Self::default();
        for (region, info) in ctx.regions() {
            let Some(desc) = &info.descriptor else {
                continue;
            };
            if let Some(did) = desc.water_table() {
                let t = water_cache
                    .entry(did)
                    .or_insert_with(|| {
                        read(
                            source,
                            did,
                            WATER_TABLE_PREFIX,
                            "water table",
                            decode_water_table,
                        )
                    })
                    .clone();
                if let Some(t) = t {
                    out.water.insert(*region, t);
                }
            }
            if let Some(did) = desc.fog_zone_table() {
                let t = fog_cache
                    .entry(did)
                    .or_insert_with(|| {
                        read(
                            source,
                            did,
                            FOG_ZONE_TABLE_PREFIX,
                            "fog-zone table",
                            decode_fog_zone_table,
                        )
                    })
                    .clone();
                if let Some(t) = t {
                    out.fog.insert(*region, t);
                }
            }
        }
        tracing::debug!(
            water = out.water.len(),
            fog = out.fog.len(),
            "water tables loaded"
        );
        out
    }

    /// The water table of `region`.
    #[must_use]
    pub fn water_table(&self, region: u8) -> Option<&Arc<WaterTable>> {
        self.water.get(&region)
    }

    /// The fog-zone table of `region`.
    #[must_use]
    pub fn fog_zone_table(&self, region: u8) -> Option<&Arc<FogZoneTable>> {
        self.fog.get(&region)
    }

    /// Regions with a water table.
    pub fn water_regions(&self) -> impl Iterator<Item = u8> + '_ {
        self.water.keys().copied()
    }

    /// [`decide_water`] for landblock `key` with its region's table. A missing block map
    /// counts as one without the type key (§6.2 step 2).
    ///
    /// # Errors
    ///
    /// The [`DryReason`] of a landblock without water.
    pub fn decide(
        &self,
        key: LandblockKey,
        block_map: Option<&BlockMap>,
        heights: &HeightGrid,
    ) -> WaterDecision {
        let table = self.water.get(&key.region()).map(|t| &**t);
        match block_map {
            Some(map) => decide_water(table, map, heights),
            None if table.is_none() => Err(DryReason::NoTable),
            None => Err(DryReason::NoTypeKey),
        }
    }
}

fn read<T>(
    source: &dyn WorldSource,
    did: Did,
    prefix: u8,
    what: &'static str,
    decode: fn(&[u8]) -> tessera_formats::Result<T>,
) -> Option<Arc<T>> {
    if did.prefix() != prefix {
        tracing::warn!(did = %format_args!("{:#010x}", did.0), what, "region slot names a record of another kind");
        return None;
    }
    match source.read(did) {
        Ok(Some(bytes)) => match decode(&bytes) {
            Ok(t) => Some(Arc::new(t)),
            Err(e) => {
                tracing::warn!(did = %format_args!("{:#010x}", did.0), what, error = %e, "does not decode");
                None
            }
        },
        Ok(None) => {
            tracing::warn!(did = %format_args!("{:#010x}", did.0), what, "missing");
            None
        }
        Err(e) => {
            tracing::warn!(did = %format_args!("{:#010x}", did.0), what, error = %e, "unreadable");
            None
        }
    }
}

/// The fog-zone corners of an outdoor landblock (X, Y) (`water.md` §5.3): `values` are the
/// block-map values of the table's `zone_key` at (X, Y), (X, Y + 1), (X + 1, Y + 1) and
/// (X + 1, Y), in that order (`None` where the block map is missing or lacks the key). Each
/// value selects its first entry, else the default. Returns the four entries when at least
/// one of them is active, else `None`: no fog zone, and the sky's fog applies unchanged
/// (`environment.md` §11.3). A corner without an entry (no value, or no match and no
/// default) is `None` in the result.
///
/// How the corners are blended across the block and with the sky's fog is OPEN (§5.3); the
/// Tessera rule of §5.4 is to decode the table and ignore it while no zone is active.
#[must_use]
pub fn fog_zone_corners(
    table: &FogZoneTable,
    values: [Option<u16>; 4],
) -> Option<[Option<&FogZone>; 4]> {
    let entries = values.map(|v| v.and_then(|v| table.select(u32::from(v))));
    entries
        .iter()
        .flatten()
        .any(|z| z.is_active())
        .then_some(entries)
}
