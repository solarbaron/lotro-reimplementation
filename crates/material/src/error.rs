// SPDX-License-Identifier: GPL-3.0-or-later
//! Error type of this crate.

use tessera_formats::{Did, FormatError};

/// Result alias used throughout the crate.
pub type Result<T, E = MaterialError> = std::result::Result<T, E>;

/// Boxed error returned by a caller-supplied record source.
pub type SourceFailure = Box<dyn std::error::Error + Send + Sync + 'static>;

/// Everything that can go wrong while resolving or composing an image.
///
/// Composition itself never fails: [`crate::icon::IconComposer::compose`] skips a layer
/// that produces one of these errors and reports it in [`crate::icon::LayerOutcome`]
/// (`icon-layers.md` §3.1). Every other fallible public function of the crate returns
/// it directly (for example [`crate::icon::IconComposer::layer`],
/// [`crate::icon::decode_layer`], [`crate::icon::compose_images`],
/// [`crate::icon::IconStack::from_entries`] and the drawing helpers).
#[derive(Debug, thiserror::Error)]
#[non_exhaustive]
pub enum MaterialError {
    /// The DID is not a render surface (prefix 0x41).
    #[error("{did} is not a render surface DID (prefix 0x41)")]
    NotSurface {
        /// The DID that was given.
        did: Did,
    },

    /// No archive holds the surface.
    #[error("surface {did} not found")]
    Missing {
        /// The surface DID.
        did: Did,
    },

    /// The record source failed while reading the surface.
    #[error("reading surface {did} failed: {source}")]
    Source {
        /// The surface DID.
        did: Did,
        /// The source's own error.
        #[source]
        source: SourceFailure,
    },

    /// The surface record did not decode.
    #[error("surface {did} does not decode: {source}")]
    Decode {
        /// The surface DID.
        did: Did,
        /// The decoder's error.
        #[source]
        source: FormatError,
    },

    /// The image is larger than the layer size limit on a side
    /// ([`crate::icon::MAX_LAYER_DIMENSION`] unless a caller chose another).
    #[error("image {width}×{height} exceeds the {limit} px limit per side")]
    TooLarge {
        /// Image width.
        width: u32,
        /// Image height.
        height: u32,
        /// The limit that applies.
        limit: u32,
    },

    /// The image has no pixels (width or height 0). Such a layer is taken as absent
    /// (`icon-layers.md` §3.1).
    #[error("image {width}×{height} has no pixels")]
    Empty {
        /// Image width.
        width: u32,
        /// Image height.
        height: u32,
    },

    /// An [`tessera_formats::RgbaImage`] whose pixel buffer does not hold
    /// `width × height × 4` bytes.
    #[error("RGBA image {width}×{height} has {len} bytes of pixels, expected {expected}")]
    InvalidImage {
        /// Image width.
        width: u32,
        /// Image height.
        height: u32,
        /// Bytes in the pixel buffer.
        len: usize,
        /// Bytes it should hold.
        expected: u64,
    },

    /// More layers than [`crate::icon::MAX_STACK_LAYERS`] were given.
    #[error("at least {count} layers given, more than the limit of {limit}")]
    TooManyLayers {
        /// Layers given. [`crate::icon::IconStack::from_entries`] stops reading its input
        /// after `limit + 1` non-zero entries (so an endless iterator is safe), so there it
        /// is a lower bound; [`crate::icon::compose_images`] reports the exact count.
        count: usize,
        /// The limit.
        limit: usize,
    },
}
