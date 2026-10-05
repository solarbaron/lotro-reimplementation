// SPDX-License-Identifier: GPL-3.0-or-later
//! Error type and failure categories (`docs/specs/mesh.md` §15).

use std::fmt;

use tessera_assets::AssetError;
use tessera_formats::Did;

use crate::validate::Issue;

/// Result alias used throughout the crate.
pub type Result<T, E = MeshError> = std::result::Result<T, E>;

/// The failure categories a decoder distinguishes (`mesh.md` §15).
///
/// Every [`MeshError`] maps to exactly one category through [`MeshError::category`], so a
/// census over many records can be summarised as counts per category.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, PartialOrd, Ord)]
pub enum FailureCategory {
    /// The record ended before a field or array it announces.
    Truncated,
    /// A submesh's vertex format word is outside what the spec supports (§4.2).
    UnsupportedVertexFormat,
    /// An index buffer refers past its submesh's vertices, or a draw entry past the
    /// submesh list (§7, §8.1).
    IndexOutOfRange,
    /// The record disagrees with the layout: count mismatches (MI2), bad flag bytes, a bad
    /// primitive (MI8), no levels, a DID that is not the requested one (MI1).
    Structure,
    /// Bytes remain after the last level (MI12).
    TrailingBytes,
    /// A count is larger than a fixed safety limit (hostile-input bound).
    LimitExceeded,
    /// Decoded fine, but a structural invariant failed in strict mode (§14; see
    /// [`crate::validate`] and [`crate::DecodeOptions::strict`]).
    Invalid,
    /// The record could not be read (missing, archive error, pipeline state).
    Source,
    /// The caller's request or hand-built [`crate::Mesh`] value is unusable: a level that
    /// does not exist, streams whose lengths disagree, or a mesh that cannot be encoded.
    /// Never produced by decoding a record.
    Usage,
}

impl FailureCategory {
    /// Every category, in a stable order.
    pub const ALL: [FailureCategory; 9] = [
        Self::Truncated,
        Self::UnsupportedVertexFormat,
        Self::IndexOutOfRange,
        Self::Structure,
        Self::TrailingBytes,
        Self::LimitExceeded,
        Self::Invalid,
        Self::Source,
        Self::Usage,
    ];

    /// A short lowercase name for logs and reports.
    #[must_use]
    pub fn name(self) -> &'static str {
        match self {
            Self::Truncated => "truncated",
            Self::UnsupportedVertexFormat => "unsupported-vertex-format",
            Self::IndexOutOfRange => "index-out-of-range",
            Self::Structure => "structure",
            Self::TrailingBytes => "trailing-bytes",
            Self::LimitExceeded => "limit-exceeded",
            Self::Invalid => "invalid",
            Self::Source => "source",
            Self::Usage => "usage",
        }
    }
}

impl fmt::Display for FailureCategory {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.write_str(self.name())
    }
}

/// Everything that can go wrong while decoding, flattening, encoding or loading a mesh.
#[derive(Debug, Clone, thiserror::Error)]
#[non_exhaustive]
pub enum MeshError {
    /// The record ended early.
    #[error("truncated at byte {offset}: {what} needs {needed} bytes, {available} left")]
    Truncated {
        /// What was being read.
        what: &'static str,
        /// Byte offset in the payload where it starts.
        offset: usize,
        /// Bytes it needs.
        needed: u64,
        /// Bytes left in the payload.
        available: usize,
    },

    /// A vertex format word the decoder refuses (`mesh.md` §4.2): not corrupt data, but a
    /// layout the spec does not confirm.
    #[error("submesh {submesh}: unsupported vertex format {vfmt:#010x} ({reason})")]
    UnsupportedVertexFormat {
        /// Submesh index.
        submesh: u32,
        /// The format word.
        vfmt: u32,
        /// Which rule it breaks.
        reason: &'static str,
    },

    /// An index of an index buffer is not below the vertex count of its submesh (§7).
    #[error(
        "index buffer {buffer}: index {value} at position {position} is out of range for {vertex_count} vertices"
    )]
    IndexOutOfRange {
        /// Index-buffer number.
        buffer: u32,
        /// Position of the offending index in the buffer.
        position: usize,
        /// The index value.
        value: u32,
        /// Vertices of the submesh it belongs to.
        vertex_count: u32,
    },

    /// A draw entry names a vertex block, index buffer or surface that does not exist
    /// (§8.1), or pairs an index buffer with a vertex block it does not fit (its largest
    /// index is reported as field `"index"`).
    #[error("level {level} draw {draw}: {field} {value} out of range ({count} available)")]
    DrawOutOfRange {
        /// Level number.
        level: u32,
        /// Draw number within the level.
        draw: u32,
        /// `"vertex_block"`, `"index_buffer"`, `"surface"` or `"index"`.
        field: &'static str,
        /// The value found.
        value: u32,
        /// The number of valid values.
        count: u32,
    },

    /// Two counts that must agree do not (MI2).
    #[error("{what}: expected {expected}, found {found}")]
    CountMismatch {
        /// Which counts.
        what: &'static str,
        /// The value required by the other count.
        expected: u64,
        /// The value in the record.
        found: u64,
    },

    /// A byte that must be 0 or 1 is neither (§8.1, §10.1).
    #[error("{what} at byte {offset} is {value}, expected 0 or 1")]
    BadFlagByte {
        /// Which byte.
        what: &'static str,
        /// Byte offset in the payload.
        offset: usize,
        /// The value found.
        value: u8,
    },

    /// A draw entry's primitive byte is not 4 (triangle list, §8.1).
    #[error("level {level} draw {draw}: primitive {value}, only 4 (triangle list) is known")]
    BadPrimitive {
        /// Level number.
        level: u32,
        /// Draw number within the level.
        draw: u32,
        /// The value found.
        value: u8,
    },

    /// An index buffer's length is not a multiple of 3 (§7).
    #[error("index buffer {buffer}: {count} indices is not a whole number of triangles")]
    PartialTriangle {
        /// Index-buffer number.
        buffer: u32,
        /// Its index count.
        count: u32,
    },

    /// The record has no detail level, or a level has no draw entry (§8.1: both ≥ 1).
    #[error("{what} is empty (the spec requires at least one)")]
    Empty {
        /// `"level list"` or `"draw list of level N"`.
        what: String,
    },

    /// The record starts with another DID than the one requested (MI1).
    #[error("record holds DID {found}, expected {expected}")]
    DidMismatch {
        /// The requested DID.
        expected: Did,
        /// The DID at the start of the record.
        found: Did,
    },

    /// Bytes remain after the last level (MI12).
    #[error("{count} trailing bytes after the last level (at byte {offset})")]
    TrailingBytes {
        /// Where the unread bytes start.
        offset: usize,
        /// How many there are.
        count: usize,
    },

    /// A count exceeds a fixed safety limit.
    #[error("{what} {value} exceeds the limit {limit}")]
    LimitExceeded {
        /// What was counted.
        what: &'static str,
        /// The count.
        value: u64,
        /// The limit.
        limit: u64,
    },

    /// [`crate::Mesh::level_geometry`] was asked for a level the mesh does not have.
    #[error("level {level} does not exist ({count} levels)")]
    NoSuchLevel {
        /// The level asked for.
        level: usize,
        /// The number of levels.
        count: usize,
    },

    /// A stream of a submesh does not hold one entry per vertex (`vertex_count × K` for
    /// skin slots). Decoded meshes never have this; a hand-built or edited [`crate::Mesh`]
    /// can.
    #[error("submesh {submesh}: {stream} has {found} entries, expected {expected}")]
    StreamLength {
        /// Submesh index.
        submesh: u32,
        /// `"positions"`, `"normals"`, `"scalars"`, `"colors"`, `"uv set"`, `"tangents"`,
        /// `"binormals"`, `"skin joints"` or `"skin weights"`.
        stream: &'static str,
        /// Entries found.
        found: u64,
        /// Entries required by the vertex count.
        expected: u64,
    },

    /// A skinned part uses a bone index the skeleton does not have (`setup.md` SI10,
    /// `skeleton-anim.md` §4.1). Only [`crate::Mesh::skinned_level`] with a bone count
    /// reports it.
    #[error("submesh {submesh}: bone index {joint} is not below the skeleton's {bone_count} bones")]
    JointOutOfRange {
        /// Submesh index.
        submesh: u32,
        /// The largest bone index the submesh uses.
        joint: u8,
        /// The skeleton's bone count.
        bone_count: usize,
    },

    /// Strict decoding found a failed structural invariant (§14).
    #[error("invariant failed: {0}")]
    Invalid(Issue),

    /// A mesh value cannot be written back as a record (streams that disagree with the
    /// format word, collision data that was not kept).
    #[error("cannot encode: {0}")]
    Encode(String),

    /// The record could not be read through the asset pipeline or the archive set.
    #[error("loading {did}: {source}")]
    Source {
        /// The requested DID.
        did: Did,
        /// The pipeline's error.
        #[source]
        source: AssetError,
    },
}

impl MeshError {
    /// The failure category of this error (`mesh.md` §15).
    ///
    /// A decode failure reported through the asset pipeline keeps the category of the
    /// underlying decode error.
    #[must_use]
    pub fn category(&self) -> FailureCategory {
        match self {
            Self::Truncated { .. } => FailureCategory::Truncated,
            Self::UnsupportedVertexFormat { .. } => FailureCategory::UnsupportedVertexFormat,
            Self::IndexOutOfRange { .. }
            | Self::DrawOutOfRange { .. }
            | Self::JointOutOfRange { .. } => FailureCategory::IndexOutOfRange,
            Self::CountMismatch { .. }
            | Self::BadFlagByte { .. }
            | Self::BadPrimitive { .. }
            | Self::PartialTriangle { .. }
            | Self::Empty { .. }
            | Self::DidMismatch { .. } => FailureCategory::Structure,
            Self::NoSuchLevel { .. } | Self::StreamLength { .. } | Self::Encode(_) => {
                FailureCategory::Usage
            }
            Self::TrailingBytes { .. } => FailureCategory::TrailingBytes,
            Self::LimitExceeded { .. } => FailureCategory::LimitExceeded,
            Self::Invalid(_) => FailureCategory::Invalid,
            Self::Source { source, .. } => match source {
                AssetError::Decode { source, .. } => source
                    .downcast_ref::<MeshError>()
                    .map_or(FailureCategory::Source, MeshError::category),
                _ => FailureCategory::Source,
            },
        }
    }

    /// The decode error inside a pipeline failure, if this is one.
    #[must_use]
    pub fn decode_error(&self) -> Option<&MeshError> {
        match self {
            Self::Source {
                source: AssetError::Decode { source, .. },
                ..
            } => source.downcast_ref::<MeshError>(),
            _ => None,
        }
    }
}
