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

    /// The decoded data would exceed the reader's memory budget
    /// ([`crate::reader::MAX_DECODE_AMPLIFICATION`] × input length plus a fixed slack).
    /// Only hostile inputs built from many tiny elements reach it.
    #[error("decoded data at offset {offset} would exceed the memory budget of {limit} bytes")]
    MemoryLimit {
        /// Offset at which the budget ran out.
        offset: usize,
        /// The reader's whole budget in bytes.
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

    /// A property set names a PID the schema does not define, so its value length is
    /// unknown and decoding cannot continue (`property-system.md` §5.3 step 2.3).
    #[error("unknown property ID {pid:#x} at offset {offset}")]
    UnknownProperty {
        /// The property ID.
        pid: u32,
        /// Offset of the entry's PID.
        offset: usize,
    },

    /// Decoding the value of one property failed. Wraps the innermost failure once, so a
    /// census can attribute it to a property ID and type (`property-system.md` §11).
    #[error("property {pid:#x} (type {type_code}) at offset {offset}: {source}")]
    PropertyValue {
        /// The property whose value failed to decode.
        pid: u32,
        /// Its type code from the schema.
        type_code: u32,
        /// Offset of the value's first byte.
        offset: usize,
        /// The underlying failure.
        source: Box<FormatError>,
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
    /// The innermost error, looking through [`FormatError::PropertyValue`] wrappers.
    #[must_use]
    pub fn root_cause(&self) -> &FormatError {
        let mut e = self;
        while let Self::PropertyValue { source, .. } = e {
            e = source;
        }
        e
    }

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
