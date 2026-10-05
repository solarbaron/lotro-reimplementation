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

    /// A field holds a value this decoder does not support: an unknown flag bit or section
    /// type, or a constant whose other values might change the layout. Not necessarily
    /// corruption; the data may come from a newer client.
    #[error("unsupported {what} {value:#x} at offset {offset}")]
    Unsupported {
        /// What was being decoded.
        what: &'static str,
        /// Offset of the field.
        offset: usize,
        /// The value found.
        value: u32,
    },

    /// A count that the format stores twice disagrees with its other copy.
    #[error("count mismatch in {what} at offset {offset}: expected {expected}, found {found}")]
    CountMismatch {
        /// What was being decoded.
        what: &'static str,
        /// Offset of the second copy.
        offset: usize,
        /// The first copy.
        expected: u32,
        /// The second copy.
        found: u32,
    },

    /// An index or window stored in the record points outside the structure it indexes
    /// (for example a terrain layer's triangle window outside its chunk's index list, or a
    /// grid vertex number ≥ 1,089; `landblock.md` §9 `IndexOutOfRange`).
    #[error("{what} {index} at offset {offset} is out of range (limit {limit})")]
    IndexOutOfRange {
        /// What was being decoded.
        what: &'static str,
        /// Offset of the offending field.
        offset: usize,
        /// The index (or window end) found.
        index: u64,
        /// The exclusive upper bound it had to stay below (or at, for window ends).
        limit: u64,
    },

    /// A stored count exceeds an absolute decoder cap (not the remaining-bytes bound, which
    /// is [`FormatError::Truncated`]); for example the static-entity count above
    /// [`crate::landblock::MAX_STATIC_ENTITIES`] (`landblock.md` §9 `Limit`).
    #[error("{what} {count} at offset {offset} exceeds the limit of {limit}")]
    CountLimit {
        /// What was being counted.
        what: &'static str,
        /// Offset of the count field.
        offset: usize,
        /// The stored count.
        count: u64,
        /// The decoder's cap.
        limit: u64,
    },

    /// Decoding a property set embedded in a larger record failed (`landblock.md` §9
    /// `PropertySet`). Wraps every failure inside the set once, so census code classifies
    /// it by structure rather than by the innermost error.
    #[error("embedded property set at offset {offset}: {source}")]
    InPropertySet {
        /// Offset of the set's first byte (its TSize header).
        offset: usize,
        /// The underlying failure.
        source: Box<FormatError>,
    },

    /// A record named by another record does not exist (reference resolvers only).
    #[error("record {from:#010x} names {to:#010x}, which does not exist")]
    DanglingReference {
        /// The referring record.
        from: u32,
        /// The missing record.
        to: u32,
    },

    /// A chain of references cannot be followed: a reference to an unexpected record type,
    /// a cycle, a chain longer than the resolver's limit, or a link with no continuation.
    #[error("reference chain broken at {did:#010x}: {reason}")]
    ReferenceChain {
        /// The record where the chain broke.
        did: u32,
        /// Why.
        reason: Cow<'static, str>,
    },

    /// Decoding record `did` failed while following references.
    #[error("record {did:#010x}: {source}")]
    InRecord {
        /// The record that failed to decode.
        did: u32,
        /// The underlying failure.
        source: Box<FormatError>,
    },

    /// A property name or enum label that code looks up in the data is not defined there
    /// (for example a property missing from the dictionary).
    #[error("{what} `{name}` is not defined in the data")]
    UnknownName {
        /// What kind of name: "property", "enum label", ...
        what: &'static str,
        /// The name that was looked up.
        name: &'static str,
    },

    /// The operation needs a cargo feature that was not enabled.
    #[error("feature `{0}` is not enabled")]
    FeatureDisabled(&'static str),
}

impl FormatError {
    /// The innermost error, looking through [`FormatError::PropertyValue`],
    /// [`FormatError::InPropertySet`] and [`FormatError::InRecord`] wrappers.
    #[must_use]
    pub fn root_cause(&self) -> &FormatError {
        let mut e = self;
        while let Self::PropertyValue { source, .. }
        | Self::InPropertySet { source, .. }
        | Self::InRecord { source, .. } = e
        {
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
