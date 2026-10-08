// SPDX-License-Identifier: GPL-3.0-or-later
//! Collision records: the quantised tree reader shared by meshes, terrain, ceilings, 0x66
//! records and cells (`collision.md` §3), stand-alone collision meshes (0x66, §4) and cell
//! geometry (0x01, §5).
//!
//! | Item | Spec |
//! |---|---|
//! | [`TreeCode`]: integer coordinates, [`Instruction`] set, point, box and structural walks | §3.3–§3.7 |
//! | [`TreeBlock`]: the stored container of 0x66 records and 0x01 physics meshes | §3.1 |
//! | [`CollisionMesh`]: 0x66 records ([`crate::DidKind::CollisionMesh`]) and cell physics meshes | §4, §5.3 |
//! | [`CellGeometry`]: 0x01 records ([`crate::DidKind::CollisionGeometry`]) | §5 |
//!
//! The terrain and ceiling trees of landblock data are framed by
//! [`crate::landblock::CollisionTree`]; `TreeCode::from(&tree)` walks them. The trees of
//! render-mesh detail levels are framed by the mesh crate and walked the same way with
//! [`TreeCode::new`].
//!
//! Tessera's own collision does not need the trees (§3.8): it builds its own structure from
//! the triangles. Point-in-cell tests and cell placement belong to the physics layer.
//!
//! Neither record kind fits [`crate::Record`]: 0x66 records carry no DID and both depend on
//! the directory `stamp` (§3.1), and cell geometry also needs the property dictionary.

mod cell;
mod mesh;
mod tree;

pub use cell::{
    CELL_GEOMETRY_PREFIX, CELL_MESH_COUNT, CELL_MESH_FLAG, CellGeometry, CellPlane, CellPortal,
    PLANE_ENTRY_LEN, PLANE_TAG_BPFL, PLANE_TAG_LEAF, PLANE_TAG_NEXT, PlaneListEnd, WallPolygon,
    is_plane_tag,
};
pub use mesh::{COLLISION_MESH_PREFIX, CollisionMesh};
pub use tree::{
    Axis, Diagonal, Instruction, InstructionCounts, MAX_BRANCH_STACK, MAX_QUERY_STEPS,
    MAX_WALK_STATES, MIN_STEP_BUDGET, QUANTUM, STEPS_PER_CODE_BYTE, TREE_HEADER_LEN, TreeBlock,
    TreeCode, TreeWalk, WALK_STATES_PER_CODE_BYTE, quantisation_for_bounds,
};
