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

    /// No archive holds the render surface (prefix 0x41: an icon layer or a texture
    /// level). A missing surface record (0x31) is [`MaterialError::RecordMissing`].
    #[error("surface {did} not found")]
    Missing {
        /// The render surface DID.
        did: Did,
    },

    /// The record source failed while reading the render surface (prefix 0x41). Failures
    /// reading records of the material chain are [`MaterialError::RecordSource`].
    #[error("reading surface {did} failed: {source}")]
    Source {
        /// The render surface DID.
        did: Did,
        /// The source's own error.
        #[source]
        source: SourceFailure,
    },

    /// The render surface (prefix 0x41) did not decode, or a JPEG surface's frame header
    /// could not be read. A surface record (0x31) that does not decode is
    /// [`MaterialError::RecordDecode`].
    #[error("surface {did} does not decode: {source}")]
    Decode {
        /// The render surface DID.
        did: Did,
        /// The decoder's error.
        #[source]
        source: FormatError,
    },

    /// The image is larger than the size limit on a side: an icon layer's
    /// ([`crate::icon::MAX_LAYER_DIMENSION`] unless a caller chose another) or a
    /// [`crate::MaterialLibrary`]'s texture limit
    /// ([`crate::LibraryConfig::max_texture_dimension`]).
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

    /// A DID of the wrong record type was given where `expected` was needed (for example
    /// a surface record naming a shader DID without prefix 0x2B).
    #[error("{did} is not a {expected} DID")]
    WrongKind {
        /// The DID that was given.
        did: Did,
        /// The record type that was needed.
        expected: &'static str,
    },

    /// No archive holds a record of the material chain (surface record, material, shader
    /// record or texture list).
    #[error("{what} {did} not found")]
    RecordMissing {
        /// The DID.
        did: Did,
        /// The record type.
        what: &'static str,
    },

    /// The record source failed while reading a record of the material chain.
    #[error("reading {what} {did} failed: {source}")]
    RecordSource {
        /// The DID.
        did: Did,
        /// The record type.
        what: &'static str,
        /// The source's own error.
        #[source]
        source: SourceFailure,
    },

    /// A record of the material chain did not decode.
    #[error("{what} {did} does not decode: {source}")]
    RecordDecode {
        /// The DID.
        did: Did,
        /// The record type.
        what: &'static str,
        /// The decoder's error.
        #[source]
        source: FormatError,
    },

    /// No level of a texture list resolves (`texture-list.md` §5.2 step 4): a lookup
    /// miss, not corruption.
    #[error("texture list {did} has no level that resolves")]
    TextureUnavailable {
        /// The texture-list DID.
        did: Did,
    },

    /// A resolution was asked to apply more distinct materials than
    /// [`crate::library::MAX_RESOLVE_MATERIALS`] (observed lists have 0–3).
    #[error("{count} distinct materials given, more than the limit of {limit}")]
    TooManyMaterials {
        /// Distinct materials given.
        count: usize,
        /// The limit.
        limit: usize,
    },

    /// More layers than [`crate::icon::MAX_STACK_LAYERS`] were given.
    #[error("at least {count} layers given, more than the limit of {limit}")]
    TooManyLayers {
        /// Layers given. [`crate::icon::IconStack::from_entries`] stops reading its input
        /// after `limit + 1` kept entries or [`crate::icon::MAX_STACK_INPUT`] + 1 entries
        /// read (so an endless iterator is safe), so there it is a lower bound of kept
        /// entries or of entries read; [`crate::icon::compose_images`] reports the exact
        /// count.
        count: usize,
        /// The limit.
        limit: usize,
    },
}
