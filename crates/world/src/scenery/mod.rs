// SPDX-License-Identifier: GPL-3.0-or-later
//! Scenery generation per landblock (`scenery.md` §6–§7): vertex scene pick, seeds,
//! placement nodes, height and water ranges, clearance and spacing. Deterministic; runs on
//! the asset workers.
//!
//! | Item | Purpose | Spec |
//! |---|---|---|
//! | [`SceneryTables`], [`VertexLookup`] | The region's terrain-scene (0x12), scene-choice (0x11) and terrain-type (0x19) tables, read from the region descriptor's slots 2, 1 and 8; word → scene | §4, §7.2 |
//! | [`SceneSource`], [`SceneLibrary`] | Scene records (0x02), cached | §5 |
//! | [`hash`] | The placement hashes, seeds and salts | §6 |
//! | [`generate_landblock`], [`SceneryInput`], [`GeneratedScenery`], [`SceneryObject`] | The vertex pass: (DID, position, rotation, scale) of every generated object | §7.2–§7.6 |
//! | [`frill_vertex`], [`FrillRecord`] | The frill pass of one vertex (used by [`crate::frills`]) | §8.2 |
//! | [`SceneryWater`], [`NoWater`], [`FixedWater`], [`level_plane`] | The per-landblock water surface | §11 |
//! | [`StaticClearance`], [`ClearancePids`], [`static_clearances`], [`property_dictionary`] | The static list and its clearance volumes | §7.4 step 14a |
//! | [`ObjectProperties`], [`ObjectPropertyCache`] | Properties of a 0x47 object resolved along its chain | §9.2 |
//! | [`math`] | The rotations of step 10 | §7.4 step 10 |
//!
//! # Example
//!
//! ```no_run
//! use std::sync::Arc;
//! use tessera_physics::geometry::GridMesh;
//! use tessera_world::scenery::{
//!     NoWater, SceneLibrary, SceneryInput, SceneryLimits, SceneryTables, generate_landblock,
//! };
//! use tessera_world::{LandblockKey, WorldContext};
//! # fn run(ctx: &WorldContext, data: &tessera_formats::LandblockData) -> Result<(), Box<dyn std::error::Error>> {
//! let key = LandblockKey::new(1, 48, 80)?;
//! let tables = SceneryTables::for_region(ctx, key.region())?;
//! let scenes = SceneLibrary::new(Arc::clone(ctx.source()));
//! let terrain = GridMesh::terrain(data);
//! let input = SceneryInput {
//!     key,
//!     data,
//!     terrain: &terrain,
//!     tables: &tables,
//!     scenes: &scenes,
//!     water: &NoWater,
//!     statics: &[],
//!     limits: SceneryLimits::default(),
//! };
//! let generated = generate_landblock(&input);
//! println!("{} objects", generated.objects.len());
//! # Ok(())
//! # }
//! ```
//!
//! # Hostile input
//!
//! Generation never panics and never fails. Scene references are followed with cycle
//! detection on the instantiation path and a depth cap ([`DEFAULT_MAX_SCENE_DEPTH`],
//! Tessera Decision of §13.1, never above [`MAX_SCENE_DEPTH_LIMIT`]); scenes and nodes
//! together nest at most [`MAX_RECURSION_DEPTH`] deep, which bounds the stack. Work
//! (static-list scans included), objects and footprint markers are budgeted per vertex
//! and per landblock ([`SceneryLimits`]); the static list holds at most
//! [`MAX_STATIC_VOLUMES`] volumes. Unobserved scale modes are rejected as unsupported
//! (§5.3). A scene read that fails for a reason that is not the data's is not cached and
//! marks the result incomplete ([`SceneryStats::is_complete`]), so the caller can build
//! the landblock again.

mod clearance;
mod error;
mod generate;
pub mod hash;
pub mod math;
mod props;
mod tables;

pub use clearance::{
    ClearanceBox, ClearanceCylinder, ClearancePids, MAX_STATIC_VOLUMES, MAX_VOLUMES_PER_ENTITY,
    ROTATION_UNIT_TOLERANCE, StaticClearance, property_dictionary, static_clearances,
    volumes_from_values,
};
pub use error::SceneryError;
pub use generate::{
    DEFAULT_MAX_SCENE_DEPTH, FixedWater, FrillRecord, GeneratedScenery, MAX_RECURSION_DEPTH,
    MAX_SCENE_DEPTH_LIMIT, NoWater, Rejection, SceneryInput, SceneryLimits, SceneryObject,
    SceneryStats, SceneryWater, frill_vertex, generate_landblock, level_plane, sum_stats,
};
pub use props::{
    ChainLevel, DEFAULT_PROPERTY_CACHE_ENTRIES, MAX_CHAIN_LEVELS, MAX_EXTRA_SETS, ObjectProperties,
    ObjectPropertyCache, read_set, value_bool,
};
pub use tables::{
    DEFAULT_SCENE_CACHE_ENTRIES, SceneLibrary, SceneSource, SceneryTables, VertexLookup,
};

#[cfg(test)]
mod tests;
