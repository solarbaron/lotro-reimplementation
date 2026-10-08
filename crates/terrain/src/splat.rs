// SPDX-License-Identifier: GPL-3.0-or-later
//! Texture-splatting inputs from a landblock's surface layers (`landblock.md` §3.8).
//!
//! Each terrain surface layer names a material id, an optional alpha-mask surface (0x41,
//! A8) and a run of triangles over the 33 × 33 grid. Their vertex numbers are Y-major
//! (33·y + x, §3.8.3), which is this crate's vertex order, so a layer's triangles index
//! the level-0 [`crate::TerrainMesh`] directly (and other meshes through
//! [`crate::TerrainMesh::remap_grid_indices`]).
//!
//! This module only validates and exposes the stored layers. Their meaning (the material id
//! is a key of the region's terrain type table, stored order is the draw order, masks have
//! row 0 on the south edge, `whole` covers all 2,048 grid triangles) is applied by
//! [`crate::layers::resolve_chunk`], which builds on this check (`terrain-texturing.md` §4).
//!
//! The chunk is treated as hostile: windows and vertex numbers are checked, and a layer
//! list larger than [`MAX_SPLAT_LAYERS`] is rejected before anything is copied.

use tessera_formats::Did;
use tessera_formats::landblock::{GRID_SAMPLES, LayerKind, SurfaceChunk};

use crate::error::{Result, TerrainError};

/// Largest layer count accepted (the decoder census saw at most 80 per chunk).
pub const MAX_SPLAT_LAYERS: usize = 256;

/// One drawable surface layer.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct SplatLayer {
    /// Position of the layer in the chunk (stored order = draw order,
    /// `terrain-texturing.md` §4.2).
    pub ordinal: usize,
    /// Terrain type key of the region's 0x19 table (`terrain-texturing.md` §4.1).
    pub material: u32,
    /// The 0x41 alpha-mask surface DID; `None` when the stored DID is 0 (draw opaque).
    pub mask: Option<Did>,
    /// How the layer is drawn ([`LayerKind::Masked`], [`LayerKind::Opaque`] or
    /// [`LayerKind::Whole`]; empty layers are not returned).
    pub kind: LayerKind,
    /// Triangles as grid vertex numbers (33·y + x), three per triangle, in stored order
    /// and winding (counter-clockwise from above for terrain chunks). Empty for
    /// [`LayerKind::Whole`] layers, which cover all 2,048 grid triangles
    /// (`terrain-texturing.md` §4.3; [`crate::layers::DrawLayer::triangles`] lists them).
    pub indices: Vec<u16>,
}

/// The drawable layers of `chunk` in stored order, skipping layers without geometry
/// ([`LayerKind::Empty`]).
///
/// # Errors
///
/// [`TerrainError::TooManyLayers`] above [`MAX_SPLAT_LAYERS`], [`TerrainError::LayerWindow`]
/// when a layer's triangle window leaves the index list,
/// [`TerrainError::VertexOutOfRange`] for a vertex number ≥ 1,089 and
/// [`TerrainError::LayerOverlap`] when the windows together exceed the index list.
///
/// Chunks from the `tessera-formats` decoder always pass the window and vertex-number
/// checks (the decoder enforces both), but a corrupt or hostile record that still decodes
/// can fail the other two: the decoder bounds the layer count only by the record size and
/// does not check that windows are disjoint. Callers must handle the error, for example by
/// skipping the layers of that block and using the height/slope fallback shading.
pub fn splat_layers(chunk: &SurfaceChunk) -> Result<Vec<SplatLayer>> {
    if chunk.layers.len() > MAX_SPLAT_LAYERS {
        return Err(TerrainError::TooManyLayers(chunk.layers.len()));
    }
    // Real chunks partition their index list among the layers (`m = 3 · Σ prims`,
    // §3.8.1); accepting at most that many copied indices keeps the output linear in the
    // input even for overlapping windows.
    let mut budget = chunk.indices.len();
    let mut out = Vec::with_capacity(chunk.layers.len());
    for (ordinal, layer) in chunk.layers.iter().enumerate() {
        let kind = layer.kind();
        let indices = match kind {
            LayerKind::Empty => continue,
            LayerKind::Whole => Vec::new(),
            LayerKind::Masked | LayerKind::Opaque => {
                let range = layer.index_range();
                let window = chunk
                    .layer_indices(layer)
                    .ok_or(TerrainError::LayerWindow {
                        layer: ordinal,
                        start: range.start,
                        end: range.end,
                        len: chunk.indices.len(),
                    })?;
                if let Some(&vertex) = window.iter().find(|&&v| usize::from(v) >= GRID_SAMPLES) {
                    return Err(TerrainError::VertexOutOfRange {
                        layer: ordinal,
                        vertex,
                    });
                }
                budget = budget
                    .checked_sub(window.len())
                    .ok_or(TerrainError::LayerOverlap { layer: ordinal })?;
                window.to_vec()
            }
        };
        out.push(SplatLayer {
            ordinal,
            material: layer.material,
            mask: (!layer.mask.is_null()).then_some(layer.mask),
            kind,
            indices,
        });
    }
    Ok(out)
}

#[cfg(test)]
mod tests {
    use super::*;
    use tessera_formats::landblock::SurfaceLayer;

    fn layer(material: u32, mask: u32, whole: bool, start: u16, prims: u16) -> SurfaceLayer {
        SurfaceLayer {
            material,
            mask: Did(mask),
            whole,
            start,
            prims,
            min_vertex: 0,
            vertex_span: 0,
        }
    }

    #[test]
    fn layers_are_validated_and_exposed() {
        let chunk = SurfaceChunk {
            layers: vec![
                layer(7, 0x4100_0001, false, 0, 2),
                layer(8, 0, false, 6, 1),
                layer(9, 0x4100_0002, false, 0, 0),
                layer(10, 0, true, 0, 0),
            ],
            indices: vec![0, 1, 34, 0, 34, 33, 1, 2, 35],
            quads: None,
        };
        let layers = splat_layers(&chunk).unwrap();
        assert_eq!(layers.len(), 3);
        assert_eq!(layers[0].kind, LayerKind::Masked);
        assert_eq!(layers[0].mask, Some(Did(0x4100_0001)));
        assert_eq!(layers[1].mask, None);
        assert_eq!(layers[0].indices, [0, 1, 34, 0, 34, 33]);
        assert_eq!(layers[1].kind, LayerKind::Opaque);
        assert_eq!(layers[1].indices, [1, 2, 35]);
        assert_eq!(layers[2].kind, LayerKind::Whole);
        assert_eq!(layers[2].ordinal, 3);
        assert!(layers[2].indices.is_empty());
    }

    #[test]
    fn hostile_chunks_are_rejected() {
        let bad_window = SurfaceChunk {
            layers: vec![layer(1, 0, false, 3, 5)],
            indices: vec![0; 9],
            quads: None,
        };
        assert!(matches!(
            splat_layers(&bad_window),
            Err(TerrainError::LayerWindow { layer: 0, .. })
        ));
        let bad_vertex = SurfaceChunk {
            layers: vec![layer(1, 0, false, 0, 1)],
            indices: vec![0, 1, 1089],
            quads: None,
        };
        assert_eq!(
            splat_layers(&bad_vertex),
            Err(TerrainError::VertexOutOfRange {
                layer: 0,
                vertex: 1089
            })
        );
        let many = SurfaceChunk {
            layers: vec![layer(0, 0, false, 0, 0); MAX_SPLAT_LAYERS + 1],
            ..SurfaceChunk::default()
        };
        let overlap = SurfaceChunk {
            layers: vec![layer(1, 0, false, 0, 2), layer(2, 0, false, 3, 1)],
            indices: vec![0, 1, 34, 0, 34, 33],
            quads: None,
        };
        assert_eq!(
            splat_layers(&overlap),
            Err(TerrainError::LayerOverlap { layer: 1 })
        );
        assert!(matches!(
            splat_layers(&many),
            Err(TerrainError::TooManyLayers(_))
        ));
        assert_eq!(splat_layers(&SurfaceChunk::default()), Ok(Vec::new()));
    }

    #[test]
    fn decoder_accepted_chunks_can_still_fail() {
        // Regression for the old doc claim that decoded chunks always pass: the decoder
        // checks windows and vertex numbers but neither overlap nor the layer count.
        use tessera_formats::landblock::{HeightGrid, LandblockData};
        let mut data = LandblockData::from_heights(Did(0x8001_0000), HeightGrid::flat(1000));
        data.terrain = SurfaceChunk {
            layers: vec![layer(1, 0, false, 0, 1), layer(2, 0, false, 0, 1)],
            indices: vec![0, 1, 34],
            quads: None,
        };
        let decoded = LandblockData::parse(&data.to_bytes(), None).expect("decoder accepts");
        assert_eq!(
            splat_layers(&decoded.terrain),
            Err(TerrainError::LayerOverlap { layer: 1 })
        );
        data.terrain = SurfaceChunk {
            layers: vec![layer(0, 0, false, 0, 0); MAX_SPLAT_LAYERS + 1],
            ..SurfaceChunk::default()
        };
        let decoded = LandblockData::parse(&data.to_bytes(), None).expect("decoder accepts");
        assert_eq!(
            splat_layers(&decoded.terrain),
            Err(TerrainError::TooManyLayers(MAX_SPLAT_LAYERS + 1))
        );
    }
}
