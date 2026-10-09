// SPDX-License-Identifier: GPL-3.0-or-later
//! Reading the far view's records: the UNIQUEDB entries of `far-view.md` §6.1 and §7.3,
//! sector records and imposter tables.

use tessera_formats::{DataIdMap, Did, ImposterTable, PropertySchema, Record, SectorRecord};

use super::grid::SectorKey;
use crate::context::{ROOT_DID_MAP, WorldContext};
use crate::draw_distance::UNIQUEDB_LABEL;
use crate::error::{Result, WorldError};
use crate::source::WorldSource;

/// UNIQUEDB key of the plain far-terrain template (§6.1, BIN).
pub const KEY_PLAIN_TEMPLATE: u32 = 0x39;
/// UNIQUEDB key of the normal-mapped far-terrain template (§6.1, BIN).
pub const KEY_NORMAL_TEMPLATE: u32 = 0xB6;
/// UNIQUEDB key of the far-terrain detail texture, parameter 3001 (§6.1, BIN).
pub const KEY_DETAIL_TEXTURE: u32 = 0x7A;
/// UNIQUEDB key of the horizon fill mesh (§7.3, BIN).
pub const KEY_HORIZON_MESH: u32 = 0x95;

/// The DIDs the landscape manager looks up in the UNIQUEDB map (§6.1, §7.3). A missing
/// entry or a zero target is `None`.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub struct FarViewRecords {
    /// Plain template (shader record).
    pub plain_template: Option<Did>,
    /// Normal-mapped template (shader record).
    pub normal_template: Option<Did>,
    /// Detail texture (texture list).
    pub detail_texture: Option<Did>,
    /// Horizon fill (render mesh).
    pub horizon_mesh: Option<Did>,
}

impl FarViewRecords {
    /// Reads the root DID map's UNIQUEDB sub-map and takes the four keys.
    ///
    /// # Errors
    ///
    /// When the root map or the sub-map is missing or does not decode.
    pub fn load(ctx: &WorldContext) -> Result<Self> {
        let source = ctx.source();
        let read = |did: Did, what: &'static str| -> Result<Vec<u8>> {
            source
                .read(did)?
                .ok_or(WorldError::Missing { did: did.0, what })
        };
        let root =
            DataIdMap::parse_expecting(&read(Did(ROOT_DID_MAP), "root DID map")?, ROOT_DID_MAP)
                .map_err(|e| WorldError::format(ROOT_DID_MAP, "root DID map", e))?;
        let unique = root.by_label(UNIQUEDB_LABEL).ok_or(WorldError::Missing {
            did: ROOT_DID_MAP,
            what: "UNIQUEDB sub-map",
        })?;
        let map = DataIdMap::parse_expecting(&read(unique, "UNIQUEDB map")?, unique.0)
            .map_err(|e| WorldError::format(unique.0, "UNIQUEDB map", e))?;
        let get = |key: u32| map.get(key).map(|e| e.value).filter(|d| d.0 != 0);
        Ok(Self {
            plain_template: get(KEY_PLAIN_TEMPLATE),
            normal_template: get(KEY_NORMAL_TEMPLATE),
            detail_texture: get(KEY_DETAIL_TEXTURE),
            horizon_mesh: get(KEY_HORIZON_MESH),
        })
    }
}

/// Reads and decodes the sector record of `key`. `Ok(None)` when it does not exist (DL-E1).
///
/// # Errors
///
/// When the record exists but cannot be read or decoded (`distant-landscape.md` §5.8: the
/// caller logs it once and skips the sector).
pub fn read_sector(
    source: &dyn WorldSource,
    key: SectorKey,
    schema: &dyn PropertySchema,
) -> Result<Option<SectorRecord>> {
    let did = key.did();
    let Some(bytes) = source.read(did)? else {
        return Ok(None);
    };
    SectorRecord::parse_expecting(&bytes, did, schema)
        .map(Some)
        .map_err(|e| WorldError::format(did.0, "distant-landscape sector", e))
}

/// Reads and decodes imposter table `did`. `Ok(None)` when it does not exist.
///
/// # Errors
///
/// When the record exists but cannot be read or decoded.
pub fn read_imposter_table(source: &dyn WorldSource, did: Did) -> Result<Option<ImposterTable>> {
    let Some(bytes) = source.read(did)? else {
        return Ok(None);
    };
    ImposterTable::parse_expecting(&bytes, did)
        .map(Some)
        .map_err(|e| WorldError::format(did.0, "imposter table", e))
}
