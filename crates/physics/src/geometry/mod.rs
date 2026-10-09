// SPDX-License-Identifier: GPL-3.0-or-later
//! Collision geometry: terrain and ceiling triangles (`collision.md` §6.1–§6.4), the
//! terrain radius and plane queries (§6.6), cell volumes and point-in-cell (§5.4, §5.8.1),
//! per-landblock collision sets (§9.3, §9.4, §12.1), the bounding-volume hierarchy and
//! capsule sweeps.
//!
//! | Item | Spec |
//! |---|---|
//! | [`GridMesh`]: terrain and ceiling meshes, key numbering, [`GridMesh::client_keys_near`] (radius query), [`GridMesh::client_plane_at`] (plane query), [`GridMesh::keys_in_disc`] | §6.1–§6.6, `scenery.md` §7.4 steps 8, 13 |
//! | [`CellVolume`], [`CellPlacement`], [`PlacedCell`], [`CellSet::cell_at`] | §5.4, §5.8, §5.8.1 |
//! | [`ShapeTemplate`], [`BoxVolume`]: 0x66 meshes, setup volumes (solid boxes), cell physics meshes, mesh-level collision | §4, §5.3, §7, §8 |
//! | [`CollisionFlags`], [`EntityLookups`], [`PhysicsPids`], [`static_blocks_walker`], [`static_has_walker_collision`], [`generated_blocks_walker`], [`content_layers_pass`] | §9.1–§9.4 |
//! | [`resolve_object_collision`], [`ObjectCollisionCache`] | §8, §9.1, §9.4 |
//! | [`CollisionSetBuilder`], [`CollisionSet`], [`CollisionView`], [`CollisionQuery`] | §12.1; decision 0013 (positions) |
//! | [`WaterPlane`], [`LiquidBox`], [`LiquidFlags`] | `water.md` §6.2–§7.4; decision 0013 (water) |
//! | [`Bvh`] | §3.8, §12.1 (Tessera's own structure) |
//!
//! Frames: a set is in its landblock's frame (x, y from the south-west corner, z absolute;
//! `landblock.md` §3.3). Every type here is `Send + Sync`; building runs on
//! `tessera-world`'s asset workers and queries may run concurrently.

mod bvh;
mod cell;
mod flags;
mod liquid;
mod math;
mod narrow;
mod primitives;
mod resolve;
mod set;
mod shape;
mod terrain;

pub use bvh::{Bvh, MAX_DEPTH, MAX_LEAF_ITEMS};
pub use cell::{
    CELL_BROAD_PHASE_HALF, CELL_LOOKUP_MAX_DISTANCE_SQ, CELL_PLANE_TOLERANCE, CellPlacement,
    CellSet, CellVolume, MAX_CELL_HULL, MAX_CELL_PLANES, MAX_SET_CELL_HULL, MAX_SET_CELL_PLANES,
    PlacedCell,
};
pub use flags::{
    CREATION_TYPE_INVALID, CREATION_TYPE_SHARED_STATIC_IMMOBILE,
    CREATION_TYPE_UNIQUE_DYNAMIC_IMMOBILE, CREATION_TYPE_UNIQUE_DYNAMIC_MOBILE, CollisionFlags,
    ETHEREAL_CAMERA, ETHEREAL_DEFAULT, ETHEREAL_PLAYER, EntityLookups, MAX_PHYSICS_SCALE,
    PhysicsPids, SCALE_TOLERANCE, SetupCollision, content_layer_list, content_layers_pass,
    ethereality_bit, generated_blocks_walker, physics_scale, static_blocks_walker,
    static_has_walker_collision,
};
pub use liquid::{
    LiquidBox, LiquidFlags, LiquidPids, LiquidSpan, MAX_LIQUID_BOXES, UNDER_WATER_TOLERANCE,
    WaterPlane,
};
pub use math::MAX_COORDINATE;
pub use primitives::{Aabb, Capsule, Plane, Sphere, Transform};
pub use resolve::{
    DEFAULT_COLLISION_CACHE_BYTES, DEFAULT_COLLISION_CACHE_ENTRIES, FetchedRecord, ObjectCollision,
    ObjectCollisionCache, resolve_object_collision,
};
pub use set::{
    Body, BodyId, BodyPurpose, BodySource, BuildReport, CellHit, CollisionLimits, CollisionQuery,
    CollisionSet, CollisionSetBuilder, CollisionView, Contact, FeatureKind, GeneratedObject,
    LANDBLOCK_SIDE, MAX_QUERY_ITEMS, QueryFilter, SKIN, StaticOptions, SweepHit, landblock_origin,
};
pub use shape::{BoxVolume, MAX_TEMPLATE_PRIMITIVES, MAX_TEMPLATE_TRIANGLES, ShapeTemplate};
pub use terrain::{GRID_TRIANGLES, GridMesh, RADIUS_QUERY_EDGE, RADIUS_QUERY_SIDE_TOLERANCE};

#[cfg(test)]
mod tests;
