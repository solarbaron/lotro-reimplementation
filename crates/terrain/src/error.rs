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

    /// A layer mask surface is not format 28 (A8). The layer is then drawn as if it had no
    /// mask (`terrain-texturing.md` §5.1, Tessera Decision).
    #[error("layer mask {did}: format {format} is not A8 (28)")]
    MaskFormat {
        /// The mask surface.
        did: tessera_formats::Did,
        /// The surface's format name.
        format: &'static str,
    },

    /// A layer mask surface whose size is 0 or above [`crate::mask::MAX_MASK_SIDE`], or
    /// whose data does not hold width × height bytes.
    #[error("layer mask {did}: unusable size {width} × {height} ({len} bytes)")]
    MaskSize {
        /// The mask surface.
        did: tessera_formats::Did,
        /// Width in texels.
        width: u32,
        /// Height in texels.
        height: u32,
        /// Stored data length.
        len: usize,
    },

    /// The ceiling chunk was asked for on a landblock without a ceiling.
    #[error("the landblock has no ceiling")]
    NoCeiling,
}
