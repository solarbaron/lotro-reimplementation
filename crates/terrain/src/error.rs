// SPDX-License-Identifier: GPL-3.0-or-later
//! Error type of `tessera-terrain`.

/// Result alias used throughout the crate.
pub type Result<T, E = TerrainError> = std::result::Result<T, E>;

/// Everything that can go wrong in this crate.
///
/// Meshing and height queries are total over their (already validated) inputs and never
/// fail; errors come from caller-supplied parameters and from surface-layer data that does
/// not fit the 33 × 33 grid.
#[derive(Debug, Clone, PartialEq, Eq, thiserror::Error)]
#[non_exhaustive]
pub enum TerrainError {
    /// A level of detail outside `0..=Lod::COARSEST`.
    #[error("terrain LOD {0} out of range (0..=5)")]
    LodOutOfRange(u8),

    /// A surface layer's index window lies outside its chunk's index list.
    #[error("surface layer {layer}: index window {start}..{end} outside the index list ({len})")]
    LayerWindow {
        /// Layer position in the chunk.
        layer: usize,
        /// First index of the window.
        start: usize,
        /// One past the last index of the window.
        end: usize,
        /// Length of the chunk's index list.
        len: usize,
    },

    /// A surface layer references a vertex number outside the 33 × 33 grid.
    #[error("surface layer {layer}: vertex number {vertex} outside the grid (< 1089)")]
    VertexOutOfRange {
        /// Layer position in the chunk.
        layer: usize,
        /// The offending vertex number.
        vertex: u16,
    },

    /// The layers' triangle windows together cover more indices than the chunk has
    /// (overlapping windows; real chunks partition their list).
    #[error("surface layer {layer}: triangle windows overlap (more indices than the list holds)")]
    LayerOverlap {
        /// Layer position in the chunk.
        layer: usize,
    },

    /// A surface chunk holds more layers than [`crate::splat::MAX_SPLAT_LAYERS`].
    #[error("surface chunk has {0} layers (limit {limit})", limit = crate::splat::MAX_SPLAT_LAYERS)]
    TooManyLayers(usize),
}
