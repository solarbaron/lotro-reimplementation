// SPDX-License-Identifier: GPL-3.0-or-later
//! Landblock water (`water.md` §6.2–§6.3), water volumes from the volume entity and placed
//! volumes (§7), distant-sector water (§9), fog-zone corners (§5.3) and the "under water"
//! query (§6.3, §10).
//!
//! | Item | Purpose | Spec |
//! |---|---|---|
//! | [`water_height`], [`decide_water`], [`LandblockWater`], [`DryReason`] | The wet decision, the water height in f32 and the water plane with its "under water" test | §6.2, §6.3 |
//! | [`WaterTables`], [`fog_zone_corners`] | The region water tables (slot 5) and fog-zone tables (slot 6) read from the region descriptors | §3, §4, §5.3 |
//! | [`MonitorPids`], [`WaterPids`], [`Monitor`], [`monitors_from_value`], [`VolumeTemplate`], [`VolumeLibrary`], [`placed_volumes`], [`PlacedVolumes`] | Monitor boxes of 0x47 objects with their §7.2 flags; the landblock volume entity at (0, 0, *h*) and placed volumes | §7.1–§7.3 |
//! | [`BlockWater`], [`WaterSystem`] | One landblock's water: plane and liquid boxes in the landblock frame; implements the physics [`WaterQuery`](tessera_physics::WaterQuery) and fills a collision set | §6–§7, §10 |
//! | [`WaterMap`] | Resident landblocks' water in world coordinates; implements [`SceneryWater`](crate::scenery::SceneryWater) | §6.3, §10 |
//! | [`SectorWater`], [`sector_origin`] | The (type, *h*) pairs of a 3 × 3-landblock distant sector, derived from the block maps | §9 |
//!
//! Everything here is a pure function of the records (WA6) or a bounded cache of such
//! results; the water state at a given game time does not depend on time at all (the
//! drawing inputs of §8 that scroll with time belong to `tessera-material`).
//!
//! # Hostile input
//!
//! The decoders of `tessera-formats` bound every count. Here: monitors per object are
//! capped ([`MAX_MONITORS_PER_OBJECT`]), boxes with non-finite or inverted corners and
//! frames that the physics layer rejects are skipped, 0x47 chains are followed with the
//! cycle and depth limits of [`crate::scenery::ObjectProperties`], caches are bounded by
//! entry count ([`DEFAULT_VOLUME_CACHE_ENTRIES`], [`MAX_WATER_MAP_ENTRIES`]), a landblock
//! holds at most [`MAX_LIQUID_BOXES`](tessera_physics::geometry::MAX_LIQUID_BOXES) liquid
//! boxes, the limit of its collision set (the rest are counted in
//! [`BlockWater::boxes_left_out`]), and the surface search sorts the column intervals once
//! and sweeps them (O(*n* log *n*) in the boxes it sees). [`WaterMap`] queries hold the
//! map's lock only to clone the nine landblocks they read. Nothing panics.

mod decide;
mod query;
mod sector;
mod tables;
mod volumes;

pub use decide::{
    DryReason, INV_255, LEVEL_SCALE, LandblockWater, WaterDecision, decide_water, water_height,
};
pub use query::{BlockWater, MAX_WATER_MAP_ENTRIES, WaterMap, WaterSystem, column_water};
pub use sector::{SectorWater, sector_origin};
pub use tables::{WaterTables, fog_zone_corners};
pub use volumes::{
    DEFAULT_VOLUME_CACHE_ENTRIES, MAX_MONITORS_PER_OBJECT, Monitor, MonitorPids, PlacedVolumes,
    VolumeBox, VolumeLibrary, VolumeTemplate, WaterPids, monitors_from_value, placed_volumes,
};

use crate::error::WorldError;
use crate::scenery::SceneryError;

/// What can go wrong while loading water inputs. Deciding water itself never fails: a
/// landblock without usable inputs is dry.
#[derive(Debug, thiserror::Error)]
pub enum WaterError {
    /// Reading a 0x47 chain or a 0x78 property set failed.
    #[error(transparent)]
    Records(#[from] SceneryError),
    /// Reading or decoding a world record failed.
    #[error(transparent)]
    World(#[from] WorldError),
}

impl WaterError {
    /// Whether the failure depends on the data and so recurs on every attempt (a missing
    /// or broken record), as opposed to the asset pipeline's state.
    #[must_use]
    pub fn is_data_error(&self) -> bool {
        match self {
            Self::Records(e) => e.is_data_error(),
            Self::World(WorldError::Asset(e)) => e.is_data_error(),
            Self::World(WorldError::Cancelled) => false,
            Self::World(_) => true,
        }
    }
}

#[cfg(test)]
mod tests;
