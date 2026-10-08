// SPDX-License-Identifier: GPL-3.0-or-later
//! The frill (grass) layer (`scenery.md` §8.2–§8.4): 10 × 10-unit sectors in their own ring
//! around the camera with the engine defaults. Frills never collide.
//!
//! | Item | Purpose | Spec |
//! |---|---|---|
//! | [`FrillSettings`], [`FrillDistances`], [`GroupDraw`], [`fade_in_opacity`] | Engine options and defaults; radius, far distance, fade, reduction, fade-in | §8.3 rules 1–5, §8.4 |
//! | [`FrillPids`], [`FrillObjectValues`] | Per-object values resolved along the 0x47 chain | §8.3 rules 6–7, §9.2 |
//! | [`FrillSectorKey`], [`sector_vertices`], [`generate_sector`], [`FrillSector`], [`FrillInstance`] | One sector's instances: the frill pass of its vertices, late records discarded, the landblock and density tests | §8.2, §8.6 |
//! | [`DensityThinner`] | The unseeded density test (frills below density 1 are random by design) | §8.2 |
//! | [`FrillRing`], [`ring_centre`], [`ring_centre_of`] | The ring centre and the square of sectors held around it | §8.2 |
//!
//! Frill sectors produce instance lists only: no entities, no collision, no selection, no
//! LOD class (§8.1). Building the vertex buffers (§8.5) is the renderer's job.

mod ring;
mod sector;
mod settings;

pub use ring::{
    FrillRing, FrillRingDiff, MAX_RING_RADIUS, far_reach_sectors, ring_centre, ring_centre_of,
};
pub use sector::{
    DensityThinner, FrillInstance, FrillSector, FrillSectorKey, generate_sector, sector_vertices,
};
pub use settings::{
    ANIMATED_THRESHOLD, FADE_IN_SECONDS, FAR_INSET_SECTORS, FAR_WITHOUT_TRANSITIONS,
    FRILL_RADIUS_BY_LEVEL, FRILL_SECTOR_SIZE, FrillDistances, FrillObjectValues, FrillPids,
    FrillSettings, GroupDraw, SECTORS_PER_BLOCK, fade_in_opacity,
};

#[cfg(test)]
mod tests;
