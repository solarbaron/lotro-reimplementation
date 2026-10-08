// SPDX-License-Identifier: GPL-3.0-or-later
//! The world model of the offline walker: coordinates, landblocks and their streaming
//! (`docs/ARCHITECTURE.md` §9).
//!
//! | Item | Purpose | Spec |
//! |---|---|---|
//! | [`LandblockKey`], [`WorldPos`], [`LocalPos`], [`Landcell`], [`RenderAnchor`] | Frames and conversions; landblock record DIDs; yaw and compass heading ([`yaw`], [`heading_degrees`]) | `coordinates.md` §2–§7, §11, §12 |
//! | [`MapConstants`], [`MapCoords`], [`LocReport`] | Map coordinates ("12.3S, 45.6W") and `/loc` tokens, formatting and parsing; minimap pixels | §8–§10 |
//! | [`RingTracker`], [`RingConfig`], [`Ring`] | Desired landblock set around a focus, rings with hysteresis | ARCH §9.2 |
//! | [`WorldContext`] | Per-install data: property schema and PIDs, region descriptors, map constants, block-map descriptors, object resolver, area names | `landblock.md` §5.1, `environment.md` §3 |
//! | [`LandblockBundle`], [`build_bundle`], [`BundleDecoder`] | One landblock: data, info, resolved block map ([`BlockProps`]), properties (minimap, light maps), [`PlacedEntity`] list with objects resolved, figures ([`PlacedFigure`]: NPC-like entities and weenie placements with their instance property sets) | `landblock.md` §8, `entity-descriptor.md` §9, `appearance.md` §7.2, `walker.md` §10 |
//! | [`ObjectResolver`], [`ResolvedObject`] | 0x47 / 0x70 → setup → parts, draw-suppression flags, cached | `setup.md` §3.1 |
//! | [`WorldStreamer`] | Per-frame streaming on `tessera-assets` workers: priorities by ring and distance, cancellation, residency budget with eviction, lookups (area, music, ambient, sky, height, minimap) | ARCH §9.2, §20.3 |
//! | [`AreaNameSource`], [`WStateAreaNames`] | Area → territory → region names via `tessera-wstate` and `tessera-text` | `wstate.md` §11 |
//! | [`WorldSource`], [`BundleSource`], [`BundleFrame`] | Records with directory stamps; the internal frame that carries a landblock's records from read to decode | `landblock.md` §2.4 |
//! | [`testkit`] | Synthetic records and worlds | |
//!
//! # Example
//!
//! ```no_run
//! use std::sync::Arc;
//! use std::time::Duration;
//! use tessera_assets::{PoolConfig, WorkerPool};
//! use tessera_dat::{ArchiveSet, SetOptions};
//! use tessera_world::{BundleOptions, StreamConfig, WorldContext, WorldPos, WorldStreamer};
//!
//! # fn main() -> Result<(), Box<dyn std::error::Error>> {
//! let set = Arc::new(ArchiveSet::open("/path/to/install", SetOptions::default())?);
//! let ctx = Arc::new(WorldContext::from_install(set, BundleOptions::default())?);
//! let pool = WorkerPool::new(PoolConfig::default());
//! let mut world = WorldStreamer::new(ctx, &pool, StreamConfig::default())?;
//! let here = WorldPos::new(1, 20_240.0, 22_420.0, 0.0);
//! world.settle(here, Duration::from_secs(10));
//! if let (Some(map), Some(area)) = (world.map_coords(&here), world.area_at(&here)) {
//!     println!("{map} {:?}", area.display());
//! }
//! # Ok(())
//! # }
//! ```
//!
//! # Hostile input
//!
//! Record decoding is done by `tessera-formats`, which bounds every allocation. This
//! crate's own parsers (map coordinates, `/loc` lines, bundle frames) cap their input,
//! check every length against the bytes present and never panic. Object chains are bounded
//! by the resolver's hop limit and cycle detection; caches are bounded by entry count; the
//! streamer by its ring radii and residency budget.

#![warn(missing_docs)]

pub mod areas;
pub mod bundle;
pub mod context;
pub mod coords;
pub mod error;
pub mod mapcoords;
pub mod objects;
pub mod ring;
pub mod source;
pub mod streamer;
pub mod testkit;
pub mod ui_maps;

pub use areas::{AreaName, AreaNameSource, FixedAreaNames, WStateAreaNames};
pub use bundle::{
    BlockProps, BlockValue, BundleDecoder, BundleOptions, BundleProblem, FigureKind,
    LandblockBundle, PlacedEntity, PlacedFigure, build_bundle,
};
pub use context::{RegionInfo, WorldContext};
pub use coords::{
    LANDBLOCK_SIZE, LandblockKey, Landcell, LocalPos, RenderAnchor, WorldPos, heading_degrees,
    heading_from_yaw, yaw, yaw_from_heading, yaw_rotation,
};
pub use error::{Result, WorldError};
pub use mapcoords::{LocReport, MapConstants, MapCoords};
pub use objects::{DrawFlags, ObjectResolver, ObjectStats, ResolvedObject, WorldPids};
pub use ring::{Entered, Ring, RingConfig, RingDiff, RingTracker};
pub use source::{
    BundleFrame, BundleRead, BundleRecord, BundleSource, FramedFailure, FramedRecord,
    MemoryWorldSource, StampedRecord, WorldSource,
};
pub use streamer::{BlockState, FrameReport, MinimapRef, StreamConfig, StreamStats, WorldStreamer};

#[cfg(test)]
mod tests;
