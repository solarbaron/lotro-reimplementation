// SPDX-License-Identifier: GPL-3.0-or-later
//! `tessera-terrain` — landblock terrain for the world walker.
//!
//! Input is the decoded height grid, vertex flags and surface layers of a landblock-data
//! record (`tessera_formats::landblock`, spec `docs/specs/landblock.md` §3) and the region's
//! terrain type table (prefix 0x19, `docs/specs/terrain-texturing.md` §3). Tessera's own
//! design choices (LOD scheme, stitching, normals, fallback shading) are written up in
//! `docs/specs/terrain.md`; the texture layers follow `terrain-texturing.md` §4–§7, §10.
//!
//! | Piece | Type | What it does |
//! |---|---|---|
//! | Block | [`TerrainBlock`] | 33 × 33 stored heights, split diagonals, per-LOD error bounds; exact CPU height queries ([`TerrainBlock::height_at`], `landblock.md` §3.11); the ceiling surface ([`TerrainBlock::from_ceiling`]) |
//! | Levels | [`Lod`], [`EdgeLods`], [`LodSelector`] | Geomipmapping levels 0–5 (5- to 160-unit quads), neighbour levels per side, screen-space-error selection with hysteresis |
//! | Meshing | [`triangulate`], [`TerrainMesh`] | Triangle lists with stitched edges (crack-free at any level difference), engine-neutral vertex streams |
//! | Normals | [`VertexNormals`], [`Neighbours`] | Central differences on stored integers, using neighbour rows at edges: bit-identical across blocks and levels; downward ceiling normals |
//! | Layers | [`resolve_chunk`], [`ChunkLayers`], [`DrawLayer`] | **Default path.** Surface layers resolved against the 0x19 table: draw order, first-drawn opaque layer, `whole` layers, holes, feature switches, material parameters, opacity rule and alpha test, textures by DID, level-of-detail index lists |
//! | Texture coordinates | [`LayerVertices`], [`TextureFrame`], [`LayerTransform`], [`tangent_frame`] | The four coordinate sets of every grid vertex, the per-block offset, the layer texture transform, the normal-map tangent frame, light-map coordinates |
//! | Masks | [`AlphaMask`], [`mask_texel`] | A8 check, south-row-first addressing, bilinear clamp sampling (CPU reference) |
//! | Shading | [`HeightSlopeShading`] | **Debug fallback only**: height-band and slope palette as vertex colours, for blocks whose layers cannot be resolved |
//! | Splatting | [`splat_layers`], [`SplatLayer`] | Validated surface-layer triangle runs (the hostile-chunk checks under [`resolve_chunk`]) |
//! | Region | [`TerrainTiles`] | Resident blocks of one region: region-global height/normal queries, neighbour lookup |
//!
//! # Drawing a block with layers
//!
//! 1. Resolve the terrain chunk (and the ceiling chunk of a cave block) with
//!    [`resolve_landblock`] against the region's table, passing the block's light maps.
//! 2. Build [`LayerVertices`] for the surface (the terrain [`TerrainBlock`] or
//!    [`TerrainBlock::from_ceiling`], with [`VertexNormals::compute`] or
//!    [`VertexNormals::ceiling`]) and the block offset of the [`TextureFrame`].
//! 3. Draw [`ChunkLayers::layers`] in order over the full-grid vertex buffer: each
//!    layer's [`DrawLayer::triangles`], its textures, feature switches, parameters and
//!    splat inputs. Triangles in no layer are not drawn. Check each layer's mask surface
//!    with [`DrawLayer::check_mask`] (it drops and logs a non-A8 mask), and skip a layer
//!    that cannot be set up with [`ChunkLayers::skip_layer`].
//! 4. For the **terrain** at a coarser or stitched level, draw each layer with
//!    [`ChunkLayers::layer_indices_for`] that level's list from [`triangulate`].
//!    **Ceilings** are drawn at level 0 from [`DrawLayer::triangles`] only (`terrain.md`
//!    §7, Tessera Decision (3)); their triangles face down (clockwise from above).
//!
//! # Conventions
//!
//! Z-up, +X east, +Y north, right-handed (`coordinates.md` §3). Mesh positions are
//! block-local (x, y ∈ [0, 160] from the south-west corner, z absolute); terrain triangles
//! are counter-clockwise from above, ceiling triangles clockwise. Every vertex stream is
//! in grid order, vertex number **33 · j + i** with i along X ([`vertex_index`]), which is
//! also the order of surface-layer vertex numbers.
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
//! let streams = LayerVertices::build(&block, &normals, fixed_block_offset(3, 7));
//! assert_eq!(streams.set(CoordinateSet::Xy)[0], [480.0, 1120.0]);
//! # Ok::<(), TerrainError>(())
//! ```

pub mod block;
pub mod error;
pub mod layers;
pub mod lod;
pub mod mask;
pub mod mesh;
pub mod normals;
pub mod shading;
pub mod splat;
pub mod texcoords;
pub mod tiles;

pub use block::{GroundSample, TerrainBlock, vertex_index};
pub use error::{Result, TerrainError};
pub use layers::{
    ALPHA_TEST_MIN, Availability, ChunkLayers, ColourAlphaRole, DrawLayer, LayerFeatures,
    LayerParams, LayerTextures, OpacityRule, ResolveOptions, SplatInputs, TriangleSet,
    UnresolvedLayer, quad_terrain_type, resolve_chunk, resolve_landblock,
};
pub use lod::{EdgeLods, Lod, LodSelector, Side};
pub use mask::{AlphaMask, MAX_MASK_SIDE, mask_texel};
pub use mesh::{TerrainMesh, triangulate};
pub use normals::{Neighbours, VertexNormals};
pub use shading::{ColorStop, HeightSlopeShading, MAX_COLOR_STOPS, srgb8_to_linear};
pub use splat::{MAX_SPLAT_LAYERS, SplatLayer, splat_layers};
pub use texcoords::{
    BLOCK_UNITS, CoordinateSet, LayerOffsets, LayerTransform, LayerVertices, OFFSET_JUMP_UNITS,
    OFFSET_RESET_UNITS, TANGENT_EPSILON, TangentFrame, TextureFrame, fixed_block_offset,
    tangent_frame,
};
pub use tiles::TerrainTiles;
