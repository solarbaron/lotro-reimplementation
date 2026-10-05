// SPDX-License-Identifier: GPL-3.0-or-later
//! `tessera-terrain` — landblock terrain for the world walker.
//!
//! Input is the decoded height grid and vertex flags of a landblock-data record
//! (`tessera_formats::landblock`, spec `docs/specs/landblock.md` §3); Tessera's own design
//! choices (LOD scheme, stitching, normals, shading) are written up in
//! `docs/specs/terrain.md`.
//!
//! | Piece | Type | What it does |
//! |---|---|---|
//! | Block | [`TerrainBlock`] | 33 × 33 stored heights, split diagonals, per-LOD error bounds; exact CPU height queries ([`TerrainBlock::height_at`], `landblock.md` §3.11) |
//! | Levels | [`Lod`], [`EdgeLods`], [`LodSelector`] | Geomipmapping levels 0–5 (5- to 160-unit quads), neighbour levels per side, screen-space-error selection with hysteresis |
//! | Meshing | [`triangulate`], [`TerrainMesh`] | Triangle lists with stitched edges (crack-free at any level difference), engine-neutral vertex streams |
//! | Normals | [`VertexNormals`], [`Neighbours`] | Central differences on stored integers, using neighbour rows at edges: bit-identical across blocks and levels |
//! | Shading | [`HeightSlopeShading`] | Original height-band and slope palette as vertex colours (until the layer material table is decoded) |
//! | Splatting | [`splat_layers`], [`SplatLayer`] | Validated surface-layer triangle runs (material id, mask DID) over the grid |
//! | Region | [`TerrainTiles`] | Resident blocks of one region: region-global height/normal queries, neighbour lookup |
//!
//! # Conventions
//!
//! Z-up, +X east, +Y north, right-handed (`coordinates.md` §3). Mesh positions are
//! block-local (x, y ∈ [0, 160] from the south-west corner, z absolute); triangles are
//! counter-clockwise from above. Every vertex stream is in grid order, vertex number
//! **33 · j + i** with i along X ([`vertex_index`]), which is also the order of
//! surface-layer vertex numbers.
//!
//! [`TerrainMesh`] mirrors `tessera_render::MeshData` field by field (positions, normals,
//! uvs, colors, u16 indices) without depending on the renderer, which sits in a higher
//! layer (ARCH §3).
//!
//! ```
//! use tessera_formats::landblock::{HeightGrid, VertexFlags};
//! use tessera_terrain::*;
//!
//! let block = TerrainBlock::new(
//!     &HeightGrid::from_fn(|i, j| (10_000 + 40 * i + 25 * j) as u16),
//!     &VertexFlags::default(),
//! );
//! let normals = VertexNormals::compute(&block, &Neighbours::NONE);
//! let mesh = TerrainMesh::build(
//!     &block,
//!     &normals,
//!     &HeightSlopeShading::default(),
//!     Lod::new(2)?,
//!     EdgeLods::NONE.with(Side::East, Some(Lod::new(4)?)),
//! );
//! assert!(mesh.indices.iter().all(|&i| usize::from(i) < mesh.positions.len()));
//! let z = block.height_at(80.0, 80.0).unwrap();
//! assert!((z - (10_000.0 + 40.0 * 16.0 + 25.0 * 16.0) * 2_000.0 / 65_535.0).abs() < 1e-3);
//! # Ok::<(), TerrainError>(())
//! ```

pub mod block;
pub mod error;
pub mod lod;
pub mod mesh;
pub mod normals;
pub mod shading;
pub mod splat;
pub mod tiles;

pub use block::{GroundSample, TerrainBlock, vertex_index};
pub use error::{Result, TerrainError};
pub use lod::{EdgeLods, Lod, LodSelector, Side};
pub use mesh::{TerrainMesh, triangulate};
pub use normals::{Neighbours, VertexNormals};
pub use shading::{ColorStop, HeightSlopeShading, MAX_COLOR_STOPS, srgb8_to_linear};
pub use splat::{MAX_SPLAT_LAYERS, SplatLayer, splat_layers};
pub use tiles::TerrainTiles;
