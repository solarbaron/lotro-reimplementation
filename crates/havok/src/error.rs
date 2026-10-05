// SPDX-License-Identifier: GPL-3.0-or-later
//! The crate's error type and the failure categories of `havok-data.md` §9.3.

use std::borrow::Cow;
use std::fmt;

/// Result alias used throughout the crate.
pub type Result<T, E = HavokError> = std::result::Result<T, E>;

/// Which of the three channels of a spline track an error refers to (§5.4).
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum Channel {
    /// Translation.
    Position,
    /// Rotation quaternion.
    Rotation,
    /// Scale.
    Scale,
}

impl fmt::Display for Channel {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.write_str(match self {
            Channel::Position => "position",
            Channel::Rotation => "rotation",
            Channel::Scale => "scale",
        })
    }
}

/// Everything that can go wrong while decoding Havok-format data.
///
/// Every variant maps to exactly one failure category of `havok-data.md` §9.3 through
/// [`HavokError::category`]. Decoders never panic; every failure comes back as one of these.
#[derive(Debug, Clone, PartialEq, thiserror::Error)]
#[non_exhaustive]
pub enum HavokError {
    /// The 0x05 envelope ended inside a field, or a length field points past the record end.
    #[error("0x05 envelope truncated at offset {offset} while reading {what}")]
    EnvelopeTruncated {
        /// Record offset of the field.
        offset: usize,
        /// The field being read.
        what: &'static str,
    },
    /// `has_reference_frame` was not 0 or 1 (§2.3).
    #[error("0x05 envelope reference-frame flag is {value}, not 0 or 1")]
    EnvelopeBadFlag {
        /// The byte found.
        value: u8,
    },
    /// Bytes follow the weight array (§2.3).
    #[error("{count} bytes after the 0x05 weight array at offset {offset}")]
    EnvelopeTrailingBytes {
        /// Record offset of the first leftover byte.
        offset: usize,
        /// Leftover bytes.
        count: usize,
    },
    /// The weight count is neither 0 nor the track count (§2.3).
    #[error("0x05 weight count {count} is neither 0 nor the track count {tracks}")]
    EnvelopeWeightCount {
        /// The weight count found.
        count: u32,
        /// The envelope's track count.
        tracks: u32,
    },
    /// The tagfile magic is wrong (§3.1).
    #[error("not a binary tagfile (bad magic{})", if *swapped { ", byte-swapped" } else { "" })]
    BadMagic {
        /// The byte-swapped (big-endian) magic was found.
        swapped: bool,
    },
    /// The file-info entry is missing, repeated, or names a version other than 3 (§3.3).
    #[error("tagfile file info: {reason}")]
    BadVersion {
        /// What was wrong.
        reason: Cow<'static, str>,
    },
    /// The input ended inside an entry, varint or string, or a count exceeds the bytes left.
    #[error("tagfile truncated at offset {offset} while reading {what}")]
    Truncated {
        /// Tagfile offset of the read.
        offset: usize,
        /// What was being read.
        what: &'static str,
    },
    /// A varint is longer than 5 bytes or out of `i32` range, or a count is negative (§3.2).
    #[error("bad varint at offset {offset}: {reason}")]
    BadVarint {
        /// Tagfile offset of the varint.
        offset: usize,
        /// What was wrong.
        reason: &'static str,
    },
    /// A tag other than 1, 2, 4, 7 (§3.3).
    #[error("unknown tag {tag} at offset {offset}")]
    UnknownTag {
        /// Tagfile offset of the tag.
        offset: usize,
        /// The tag value.
        tag: i32,
    },
    /// A class or parent index (or a struct class name) names no declared class (§3.5, §3.7).
    #[error("bad class index {index} for {what} at offset {offset}")]
    BadClassIndex {
        /// Tagfile offset.
        offset: usize,
        /// The index found (−1 for a class name that resolves to no class).
        index: i64,
        /// What referred to the class.
        what: &'static str,
    },
    /// A string reference beyond the pool's current end (§3.4).
    #[error("string reference {index} beyond the pool (size {pool}) at offset {offset}")]
    BadStringRef {
        /// Tagfile offset.
        offset: usize,
        /// The referenced pool index.
        index: u32,
        /// Pool size at that point.
        pool: usize,
    },
    /// A member type code with an unknown kind or flags (§3.6).
    #[error("bad member type code {code} at offset {offset}")]
    BadMemberType {
        /// Tagfile offset.
        offset: usize,
        /// The code found.
        code: i32,
    },
    /// A form of §3.12 that this reader does not implement, or a resource limit.
    #[error("unsupported tagfile form at offset {offset}: {what}")]
    UnsupportedForm {
        /// Tagfile offset.
        offset: usize,
        /// The form.
        what: &'static str,
    },
    /// An object reference that resolves to no object (§3.9).
    #[error("object reference {index} resolves to no object")]
    UnresolvedRef {
        /// The reference found.
        index: i32,
    },
    /// Bytes after the end tag (§3.3).
    #[error("{count} bytes after the tagfile end tag at offset {offset}")]
    TagfileTrailingBytes {
        /// Tagfile offset of the first leftover byte.
        offset: usize,
        /// Leftover bytes.
        count: usize,
    },
    /// Object 1 is not the expected kind of object, or there is no object (§9.3).
    #[error("expected {expected} as object 1, found {}", found.as_deref().unwrap_or("no object"))]
    ObjectMissing {
        /// The class family expected.
        expected: &'static str,
        /// The class name of object 1, if there is one.
        found: Option<String>,
    },
    /// Object 1 has a class `havok-data.md` does not describe.
    #[error("object 1 has unknown class {name:?}")]
    UnknownClass {
        /// Class name (Latin-1 decoded).
        name: String,
    },
    /// A needed member is declared with a different type code (§3.11).
    #[error("{class}.{member} declared with type {found}, expected {expected}")]
    TypeMismatch {
        /// Class name.
        class: &'static str,
        /// Member name.
        member: &'static str,
        /// The type code this reader needs.
        expected: u8,
        /// The type code declared in the file.
        found: u8,
    },
    /// Counts disagree, or an index is out of range (§9.3 `object.inconsistent`).
    #[error("inconsistent object: {what}")]
    Inconsistent {
        /// What disagreed.
        what: Cow<'static, str>,
    },
    /// A quantisation code this reader does not implement (§5.3).
    #[error("block {block} track {track} {channel}: unsupported quantisation code {code}")]
    UnsupportedQuantisation {
        /// Block index.
        block: usize,
        /// Track index.
        track: usize,
        /// Channel.
        channel: Channel,
        /// The code found.
        code: u8,
    },
    /// The clip has float tracks (§5.8).
    #[error("clip has {count} float tracks")]
    FloatTracks {
        /// `numberOfFloatTracks`.
        count: i32,
    },
    /// A component both static and dynamic, or unused mask bits set (§5.3).
    #[error("block {block} track {track} {channel}: bad mask {mask:#04x}")]
    BadMask {
        /// Block index.
        block: usize,
        /// Track index.
        track: usize,
        /// Channel.
        channel: Channel,
        /// The mask byte.
        mask: u8,
    },
    /// A curve whose degree, control-point count or knot vector violates §5.4.
    #[error("block {block} track {track} {channel}: bad curve ({reason})")]
    BadCurve {
        /// Block index.
        block: usize,
        /// Track index.
        track: usize,
        /// Channel.
        channel: Channel,
        /// What was wrong.
        reason: &'static str,
    },
    /// A channel reads past the block end (§7.3).
    #[error("block {block}: read past the block end at block offset {offset} (track {track})")]
    BlockOverrun {
        /// Block index.
        block: usize,
        /// Track index (the track count for the mask section).
        track: usize,
        /// Offset from the block start.
        offset: usize,
    },
    /// Parsing a block ended elsewhere than `floatBlockOffsets[b]` (§5.2).
    #[error("block {block}: parsing ended at {found}, declared length {expected}")]
    BlockLength {
        /// Block index.
        block: usize,
        /// `floatBlockOffsets[b]`.
        expected: usize,
        /// Where parsing ended.
        found: usize,
    },
    /// The transform count is not a positive multiple of the track count (§6).
    #[error("interleaved clip: {transforms} transforms for {tracks} tracks")]
    InterleavedBadShape {
        /// Elements of `transforms`.
        transforms: usize,
        /// `numberOfTransformTracks`.
        tracks: i64,
    },
    /// A decoded or sampled value is NaN or infinite.
    #[error("non-finite {what} (track {track})")]
    NonFinite {
        /// What was not finite.
        what: &'static str,
        /// Track (or bone) index.
        track: usize,
    },
}

impl HavokError {
    /// The failure category of `havok-data.md` §9.3 this error belongs to.
    pub fn category(&self) -> FailureCategory {
        use FailureCategory as C;
        match self {
            HavokError::EnvelopeTruncated { .. } => C::EnvelopeTruncated,
            HavokError::EnvelopeBadFlag { .. } => C::EnvelopeBadFlag,
            HavokError::EnvelopeTrailingBytes { .. } => C::EnvelopeTrailingBytes,
            HavokError::EnvelopeWeightCount { .. } => C::EnvelopeWeightCount,
            HavokError::BadMagic { .. } => C::TagfileBadMagic,
            HavokError::BadVersion { .. } => C::TagfileBadVersion,
            HavokError::Truncated { .. } => C::TagfileTruncated,
            HavokError::BadVarint { .. } => C::TagfileBadVarint,
            HavokError::UnknownTag { .. } => C::TagfileUnknownTag,
            HavokError::BadClassIndex { .. } => C::TagfileBadClassIndex,
            HavokError::BadStringRef { .. } => C::TagfileBadStringRef,
            HavokError::BadMemberType { .. } => C::TagfileBadMemberType,
            HavokError::UnsupportedForm { .. } => C::TagfileUnsupportedForm,
            HavokError::UnresolvedRef { .. } => C::TagfileUnresolvedRef,
            HavokError::TagfileTrailingBytes { .. } => C::TagfileTrailingBytes,
            HavokError::ObjectMissing { .. } => C::ObjectMissing,
            HavokError::UnknownClass { .. } => C::ObjectUnknownClass,
            HavokError::TypeMismatch { .. } => C::ObjectTypeMismatch,
            HavokError::Inconsistent { .. } => C::ObjectInconsistent,
            HavokError::UnsupportedQuantisation { .. } => C::SplineUnsupportedQuantisation,
            HavokError::FloatTracks { .. } => C::SplineFloatTracks,
            HavokError::BadMask { .. } => C::SplineBadMask,
            HavokError::BadCurve { .. } => C::SplineBadCurve,
            HavokError::BlockOverrun { .. } => C::SplineBlockOverrun,
            HavokError::BlockLength { .. } => C::SplineBlockLength,
            HavokError::InterleavedBadShape { .. } => C::InterleavedBadShape,
            HavokError::NonFinite { .. } => C::SampleNonFinite,
        }
    }

    pub(crate) fn inconsistent(what: impl Into<Cow<'static, str>>) -> Self {
        HavokError::Inconsistent { what: what.into() }
    }
}

/// The failure categories of `havok-data.md` §9.3, one per failed record.
///
/// [`FailureCategory::name`] gives the spec's dotted name (for example `tagfile.truncated`).
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, PartialOrd, Ord)]
#[allow(missing_docs)] // Each variant is documented by its `name()` and the spec table.
pub enum FailureCategory {
    EnvelopeTruncated,
    EnvelopeBadFlag,
    EnvelopeTrailingBytes,
    EnvelopeWeightCount,
    TagfileBadMagic,
    TagfileBadVersion,
    TagfileTruncated,
    TagfileBadVarint,
    TagfileUnknownTag,
    TagfileBadClassIndex,
    TagfileBadStringRef,
    TagfileBadMemberType,
    TagfileUnsupportedForm,
    TagfileUnresolvedRef,
    TagfileTrailingBytes,
    ObjectMissing,
    ObjectUnknownClass,
    ObjectTypeMismatch,
    ObjectInconsistent,
    SplineUnsupportedQuantisation,
    SplineFloatTracks,
    SplineBadMask,
    SplineBadCurve,
    SplineBlockOverrun,
    SplineBlockLength,
    InterleavedBadShape,
    SampleNonFinite,
}

impl FailureCategory {
    /// Every category, in the order of the spec table.
    pub const ALL: [FailureCategory; 27] = [
        FailureCategory::EnvelopeTruncated,
        FailureCategory::EnvelopeBadFlag,
        FailureCategory::EnvelopeTrailingBytes,
        FailureCategory::EnvelopeWeightCount,
        FailureCategory::TagfileBadMagic,
        FailureCategory::TagfileBadVersion,
        FailureCategory::TagfileTruncated,
        FailureCategory::TagfileBadVarint,
        FailureCategory::TagfileUnknownTag,
        FailureCategory::TagfileBadClassIndex,
        FailureCategory::TagfileBadStringRef,
        FailureCategory::TagfileBadMemberType,
        FailureCategory::TagfileUnsupportedForm,
        FailureCategory::TagfileUnresolvedRef,
        FailureCategory::TagfileTrailingBytes,
        FailureCategory::ObjectMissing,
        FailureCategory::ObjectUnknownClass,
        FailureCategory::ObjectTypeMismatch,
        FailureCategory::ObjectInconsistent,
        FailureCategory::SplineUnsupportedQuantisation,
        FailureCategory::SplineFloatTracks,
        FailureCategory::SplineBadMask,
        FailureCategory::SplineBadCurve,
        FailureCategory::SplineBlockOverrun,
        FailureCategory::SplineBlockLength,
        FailureCategory::InterleavedBadShape,
        FailureCategory::SampleNonFinite,
    ];

    /// The spec's dotted name of the category.
    pub fn name(self) -> &'static str {
        use FailureCategory as C;
        match self {
            C::EnvelopeTruncated => "envelope.truncated",
            C::EnvelopeBadFlag => "envelope.bad_flag",
            C::EnvelopeTrailingBytes => "envelope.trailing_bytes",
            C::EnvelopeWeightCount => "envelope.weight_count",
            C::TagfileBadMagic => "tagfile.bad_magic",
            C::TagfileBadVersion => "tagfile.bad_version",
            C::TagfileTruncated => "tagfile.truncated",
            C::TagfileBadVarint => "tagfile.bad_varint",
            C::TagfileUnknownTag => "tagfile.unknown_tag",
            C::TagfileBadClassIndex => "tagfile.bad_class_index",
            C::TagfileBadStringRef => "tagfile.bad_string_ref",
            C::TagfileBadMemberType => "tagfile.bad_member_type",
            C::TagfileUnsupportedForm => "tagfile.unsupported_form",
            C::TagfileUnresolvedRef => "tagfile.unresolved_ref",
            C::TagfileTrailingBytes => "tagfile.trailing_bytes",
            C::ObjectMissing => "object.missing",
            C::ObjectUnknownClass => "object.unknown_class",
            C::ObjectTypeMismatch => "object.type_mismatch",
            C::ObjectInconsistent => "object.inconsistent",
            C::SplineUnsupportedQuantisation => "spline.unsupported_quantisation",
            C::SplineFloatTracks => "spline.float_tracks",
            C::SplineBadMask => "spline.bad_mask",
            C::SplineBadCurve => "spline.bad_curve",
            C::SplineBlockOverrun => "spline.block_overrun",
            C::SplineBlockLength => "spline.block_length",
            C::InterleavedBadShape => "interleaved.bad_shape",
            C::SampleNonFinite => "sample.non_finite",
        }
    }

    /// Looks a category up by its dotted name.
    pub fn from_name(name: &str) -> Option<Self> {
        Self::ALL.into_iter().find(|c| c.name() == name)
    }
}

impl fmt::Display for FailureCategory {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.write_str(self.name())
    }
}
