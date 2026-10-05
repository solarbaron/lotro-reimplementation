// SPDX-License-Identifier: GPL-3.0-or-later
//! Mesh records (DID prefix 0x06): decoding into engine-neutral vertex streams.
//!
//! This crate implements `docs/specs/mesh.md` (the "spec" below).
//!
//! | Item | Purpose | Spec |
//! |---|---|---|
//! | [`VertexFormat`] | The per-submesh format word: stride and attribute offsets | §4 |
//! | [`Mesh::decode`] | A whole record: header, submeshes, index buffers, levels, collision data | §3, §5–§10, §13 |
//! | [`MeshHeader::parse`] | Region A only (DID, flags, surfaces) | §3.1 |
//! | [`Mesh::validate`] | The invariants of §14 as categorised [`Issue`]s | §14 |
//! | [`Mesh::level_geometry`] | One level flattened into shared streams with one [`Part`] per draw | §8, §13 |
//! | [`Mesh::to_bytes`] | Writes a mesh back in the record layout (fixtures, round trips) | §3 |
//! | [`synth`] | Synthetic meshes from a small description (tests, fuzz seeds, golden scenes) | §14 |
//! | [`MeshLibrary`] | Mesh records of an install, decoded on the asset pool and cached | §2, §10.4 |
//! | [`MeshSummary::parse`] | Slot, surfaces and vertex/index counts without decoding vertices (stub detection) | §3, §7 |
//! | [`Mesh::skinned_level`], [`SkinStreams`], [`SkinnedLevel::part_streams`] | Skinned streams: four influences per vertex, bones used per part, joint-range check, influence-list recomputation (GPU palettes live in `tessera-anim`) | §5.7, §6.2 |
//! | [`assembly`] | Characters and objects assembled from setups, appearance records, worn and held items | `appearance.md` §8 |
//!
//! # Conventions
//!
//! Values are kept as stored (`mesh.md` §11): object space, **Z up**, metres,
//! right-handed, characters facing +Y, counter-clockwise front faces, and UV (0, 0) at the
//! first stored texel (no V flip). Colours are converted from the stored B, G, R, A byte
//! order to R, G, B, A. Bone indices index the skeleton's bone array directly.
//!
//! # Example
//!
//! ```
//! use tessera_mesh::{GeometryLimits, Mesh};
//!
//! # fn demo(record: &[u8], stamp: u16) -> Result<(), tessera_mesh::MeshError> {
//! let mesh = Mesh::decode(record, Some(stamp), &Default::default())?;
//! let level0 = mesh.level_geometry(0, &GeometryLimits::default())?;
//! for part in &level0.parts {
//!     // Bind part.surface (a 0x31 surface record) and draw
//!     // indices[part.first_index .. part.first_index + part.index_count].
//!     let _ = (part.surface, part.first_index, part.index_count);
//! }
//! if !mesh.validate().is_clean() {
//!     // Log and still draw (§14).
//! }
//! # Ok(())
//! # }
//! ```
//!
//! # Hostile input
//!
//! Nothing panics on malformed data. Every count is checked against the bytes left
//! before anything is allocated (§13), a submesh has at most [`MAX_SUBMESH_VERTICES`]
//! vertices and a record at most [`MAX_SUBMESHES`] submeshes and [`MAX_LEVELS`] levels.
//! A decoded mesh holds at most about 1.4 × the record's bytes plus a fixed overhead of
//! a few MB, all of it counted by [`Mesh::heap_size`]. Flattening a level is bounded in
//! vertices, indices, draws, bytes and amplification by [`GeometryLimits`].
//!
//! The public types have public fields, so a [`Mesh`] can also be built or edited by
//! hand. Such a value need not be consistent: [`Mesh::validate`] reports streams whose
//! lengths disagree ([`IssueKind::StreamLength`]), [`Mesh::level_geometry`] refuses them
//! with an error and [`Mesh::to_bytes`] with another; none of them panics.

#![warn(missing_docs)]

pub mod assembly;
mod bounds;
mod encode;
mod error;
mod format;
mod geometry;
mod library;
mod read;
mod record;
mod skin;
mod summary;
pub mod synth;
pub mod validate;

pub use bounds::{Aabb, BoundingSphere, Bounds};
pub use error::{FailureCategory, MeshError, Result};
pub use format::{UnsupportedFormat, VertexFormat, VertexLayout};
pub use geometry::{GeometryLimits, Indices, LevelGeometry, Part};
pub use library::{
    MeshDecoder, MeshLibrary, MeshLibraryConfig, StampInput, StampedSource, mesh_dids,
};
pub use record::{
    BUILD_BYTE_ABSENT_MAX_STAMP, BUILD_BYTE_MIN_STAMP, BuildByteRule, CollisionCounts,
    CollisionEdge, CollisionMesh, CollisionTree, CollisionTriangle, CollisionVertex, DecodeOptions,
    Draw, IndexBuffer, Level, LevelKind, MAX_LEVELS, MAX_SUBMESH_VERTICES, MAX_SUBMESHES,
    MESH_PREFIX, Mesh, MeshHeader, PRIMITIVE_TRIANGLE_LIST, SKINNING_FLAG, SURFACE_PREFIX, Skin,
    Submesh, TreeCode, VertexStreams,
};
pub use skin::{PartSkin, PartStreams, SkinStreams, SkinnedLevel};
pub use summary::{MeshSummary, STUB_VERTEX_COUNT, SubmeshSummary};
pub use tessera_formats::Did;
pub use validate::{Attribute, Issue, IssueClass, IssueKind, ValidationReport};
