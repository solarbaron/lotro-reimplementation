// SPDX-License-Identifier: GPL-3.0-or-later
//! Error type shared by every decoder in this crate.

use std::borrow::Cow;

/// Result alias used throughout the crate.
pub type Result<T, E = FormatError> = std::result::Result<T, E>;

/// Everything that can go wrong while decoding a record payload.
///
/// Decoders never panic on malformed input; every failure is reported through this type.
#[derive(Debug, Clone, PartialEq, Eq, thiserror::Error)]
#[non_exhaustive]
pub enum FormatError {
    /// The payload ended before a field was complete.
    #[error(
        "unexpected end of data at offset {offset}: needed {needed} bytes, {available} available"
    )]
    Truncated {
        /// Offset at which the read started.
        offset: usize,
        /// Bytes the read needed.
        needed: usize,
        /// Bytes that were left.
        available: usize,
    },

    /// A `bool8` field held something other than 0 or 1.
    #[error("invalid boolean byte {value:#04x} at offset {offset}")]
    InvalidBool {
        /// Offset of the byte.
        offset: usize,
        /// The byte that was found.
        value: u8,
    },

    /// The record's own DID differs from the one that was looked up.
    #[error("record DID {found:#010x} does not match expected {expected:#010x}")]
    DidMismatch {
        /// DID the caller expected.
        expected: u32,
        /// DID stored in the record prologue.
        found: u32,
    },

    /// The record's DID belongs to a different record type than the decoder handles.
    #[error("DID {did:#010x} is not a {expected} record")]
    WrongRecordType {
        /// DID stored in the record prologue.
        did: u32,
        /// Human-readable name of the record type the decoder expected.
        expected: &'static str,
    },

    /// Bytes were left over after a record that must be consumed exactly.
    #[error("{count} trailing bytes after {what} at offset {offset}")]
    TrailingBytes {
        /// What was being decoded.
        what: &'static str,
        /// Offset of the first leftover byte.
        offset: usize,
        /// Number of leftover bytes.
        count: usize,
    },

    /// A field held a value the specification does not allow.
    #[error("malformed {what} at offset {offset}: {reason}")]
    Malformed {
        /// What was being decoded.
        what: &'static str,
        /// Offset of the offending field.
        offset: usize,
        /// Why it was rejected.
        reason: Cow<'static, str>,
    },

    /// Nested structures went deeper than the decoder's limit.
    #[error("nesting depth limit {limit} exceeded at offset {offset}")]
    DepthLimit {
        /// Offset at which the limit was hit.
        offset: usize,
        /// The configured limit.
        limit: usize,
    },

    /// A render surface uses a format code this crate does not know. Not corruption.
    #[error("unsupported surface format code {code:#x}")]
    UnsupportedSurfaceFormat {
        /// The raw format code.
        code: u32,
    },

    /// A render surface's data length disagrees with its dimensions and format.
    #[error(
        "surface data length {actual} does not match expected {expected} for {width}x{height} {format}"
    )]
    SurfaceLength {
        /// Format name.
        format: &'static str,
        /// Width from the header.
        width: u32,
        /// Height from the header.
        height: u32,
        /// Length computed from width, height and format.
        expected: u64,
        /// Length stored in the header.
        actual: u64,
    },

    /// A property value of a type whose stored encoding is not specified yet.
    #[error("property type {code} has no specified value encoding")]
    UnsupportedPropertyType {
        /// The raw property type code.
        code: u32,
    },

    /// An embedded image (JPEG) declares more pixels than the decoder will allocate for it.
    #[error("embedded image {width}x{height} exceeds the decode budget of {budget} pixels")]
    ImageTooLarge {
        /// Width from the image header.
        width: u32,
        /// Height from the image header.
        height: u32,
        /// Pixel budget that applied to this input.
        budget: u64,
    },

    /// Embedded image (JPEG) decoding failed.
    #[error("embedded image decode failed: {0}")]
    Image(String),

    /// The operation needs a cargo feature that was not enabled.
    #[error("feature `{0}` is not enabled")]
    FeatureDisabled(&'static str),
}

impl FormatError {
    pub(crate) fn malformed(
        what: &'static str,
        offset: usize,
        reason: impl Into<Cow<'static, str>>,
    ) -> Self {
        Self::Malformed {
            what,
            offset,
            reason: reason.into(),
        }
    }
}
