// SPDX-License-Identifier: GPL-3.0-or-later
//! Deciding a landblock's water (`water.md` §6.2) and the water plane with its "under
//! water" test (§6.3).

use tessera_formats::landblock::{HeightGrid, SectorWaterPair, height_from_raw};
use tessera_formats::water::{WaterTable, WetWater};
use tessera_formats::{BlockMap, Did};
use tessera_physics::geometry::{UNDER_WATER_TOLERANCE, WaterPlane};

/// The 32-bit float the client multiplies by after × 1000: the f32 nearest 1/255
/// (`water.md` §6.2 step 6; a client constant, decision 0012).
#[allow(clippy::excessive_precision)]
pub const INV_255: f32 = 0.003_921_568_859_368_563;

/// Scale applied to a stored level before [`INV_255`] (`water.md` §6.2 step 6).
pub const LEVEL_SCALE: f32 = 1000.0;

/// The water height *h* of stored level `level` in world units of the landblock frame
/// (`water.md` §6.2 step 6): the level as an f32, times 1000.0, times [`INV_255`], each
/// product rounded to 32 bits. Level 38 → 149.0196, 255 → 1,000.0.
#[must_use]
pub fn water_height(level: u16) -> f32 {
    let scaled = f32::from(level) * LEVEL_SCALE;
    scaled * INV_255
}

/// Why a landblock has no water (`water.md` §6.2 steps 1–7), for censuses and tools.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum DryReason {
    /// Step 1: the region has no water table.
    NoTable,
    /// Step 2: the block map lacks the table's `type_key`.
    NoTypeKey,
    /// Step 3: the type matches no listed entry and the table has no default.
    NoEntry,
    /// Step 4: the chosen entry is dry.
    DryType,
    /// Step 5: the block map lacks the table's `level_key`.
    NoLevelKey,
    /// Step 7: the height is not strictly above the block's lowest terrain sample.
    BelowTerrain,
}

/// A wet landblock's water state (`water.md` §6.2 step 8).
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct LandblockWater {
    /// The water type *t*: the block-map value of the table's `type_key`.
    pub water_type: u16,
    /// The stored level *L* (block-map value of `level_key`).
    pub level: u16,
    /// The water height *h* = [`water_height`]`(level)`, landblock frame.
    pub height: f32,
    /// The chosen entry's surface record, extra DIDs, volume entity and value.
    pub entry: WetWater,
    /// The lowest terrain sample *m* of the block (`landblock.md` §3.4).
    pub lowest_terrain: f32,
    /// The highest terrain sample (plays no part in the rule; kept for censuses).
    pub highest_terrain: f32,
}

impl LandblockWater {
    /// The surface record (prefix 0x31) drawn for this water (`water.md` §8.2).
    #[must_use]
    pub fn surface(&self) -> Did {
        self.entry.surface
    }

    /// The water-volume entity (prefix 0x47) placed at block-local (0, 0, *h*) (§7.1).
    #[must_use]
    pub fn volume_entity(&self) -> Did {
        self.entry.volume_entity
    }

    /// The water plane z = *h* over the whole block, normal +Z (§6.3). `None` only when
    /// the height is out of the physics coordinate range (never for a stored level).
    #[must_use]
    pub fn plane(&self) -> Option<WaterPlane> {
        WaterPlane::new(self.height, u32::from(self.water_type)).ok()
    }

    /// The signed distance of block-local height `z` from the plane: *z* − *h* (§6.3).
    #[must_use]
    pub fn signed_distance(&self, z: f32) -> f64 {
        f64::from(z) - f64::from(self.height)
    }

    /// Whether block-local height `z` is under the landblock water: *z* − *h* < −0.0002
    /// (`water.md` §6.3; [`UNDER_WATER_TOLERANCE`]). Points within 0.0002 of the surface are
    /// not under water. False for a non-finite `z`.
    #[must_use]
    pub fn is_under(&self, z: f32) -> bool {
        z.is_finite() && self.signed_distance(z) < -UNDER_WATER_TOLERANCE
    }

    /// Whether the whole block is under the plane: *h* at or above the highest terrain
    /// sample (the "fully submerged" column of the §6.4 census).
    #[must_use]
    pub fn is_fully_submerged(&self) -> bool {
        self.height >= self.highest_terrain
    }

    /// The (type, *h*) pair a distant-landscape sector lists for this water (§9): the type
    /// narrowed to the stored byte (its low 8 bits; every observed type is below 256) and
    /// *h*, bit-identical to §6.2 step 6.
    #[must_use]
    pub fn sector_pair(&self) -> SectorWaterPair {
        SectorWaterPair {
            water_type: self.water_type.to_le_bytes()[0],
            height: self.height,
        }
    }
}

/// The outcome of `water.md` §6.2 for one landblock.
pub type WaterDecision = Result<LandblockWater, DryReason>;

/// Decides a landblock's water exactly as `water.md` §6.2: `table` is its region's water
/// table (slot 5 of the region descriptor; `None` when the region has none), `block_map` its
/// raw block map (keys read as raw u16 values, no 0x18 palette, §6.1), `heights` its terrain
/// height grid. A pure function of its inputs (WA6). A ceiling grid plays no part.
///
/// # Errors
///
/// The [`DryReason`] of a landblock without water.
pub fn decide_water(
    table: Option<&WaterTable>,
    block_map: &BlockMap,
    heights: &HeightGrid,
) -> WaterDecision {
    let table = table.ok_or(DryReason::NoTable)?;
    let t = block_map.get(table.type_key).ok_or(DryReason::NoTypeKey)?;
    let entry = table.select(u32::from(t)).ok_or(DryReason::NoEntry)?;
    let wet = entry.wet.ok_or(DryReason::DryType)?;
    let level = block_map
        .get(table.level_key)
        .ok_or(DryReason::NoLevelKey)?;
    let height = water_height(level);
    let (lo, hi) = heights.min_max();
    // The §3.4 conversion is monotonic, so the lowest stored sample is the lowest height.
    let lowest_terrain = height_from_raw(lo);
    let highest_terrain = height_from_raw(hi);
    if height > lowest_terrain {
        Ok(LandblockWater {
            water_type: t,
            level,
            height,
            entry: wet,
            lowest_terrain,
            highest_terrain,
        })
    } else {
        Err(DryReason::BelowTerrain)
    }
}
