// SPDX-License-Identifier: GPL-3.0-or-later
//! Error type of the crate.

use std::path::PathBuf;

use crate::Did;

/// Result alias using the crate's [`Error`].
pub type Result<T, E = Error> = std::result::Result<T, E>;

/// Everything that can go wrong while reading an archive or an archive set.
///
/// Errors carry the absolute file offset of the offending structure where one exists.
#[derive(Debug, thiserror::Error)]
#[non_exhaustive]
pub enum Error {
    /// Opening or listing a file system path failed.
    #[error("I/O error on {path}: {source}")]
    Io {
        /// The path being opened or listed.
        path: PathBuf,
        /// Underlying error.
        #[source]
        source: std::io::Error,
    },

    /// A positional read inside an archive failed.
    #[error("read of {len} bytes at offset {offset:#x} failed: {source}")]
    Read {
        /// Absolute file offset.
        offset: u64,
        /// Requested length.
        len: u64,
        /// Underlying error.
        #[source]
        source: std::io::Error,
    },

    /// The file is shorter than the fixed header area (invariant I-1).
    #[error("file is too small to be an archive ({len} bytes)")]
    TooSmall {
        /// Actual file length.
        len: u64,
    },

    /// The magic at 0x140 is wrong (invariant I-2).
    #[error("bad magic {found:#010x} at 0x140 (expected 0x00005442)")]
    BadMagic {
        /// The value found.
        found: u32,
    },

    /// The header describes something this reader cannot use (invariants I-4, I-6, I-8).
    #[error("unusable header (I-{invariant}): {reason}")]
    BadHeader {
        /// Number `n` of the violated invariant "I-n" (spec §10).
        invariant: u8,
        /// Human-readable reason.
        reason: String,
    },

    /// An offset/size pair points outside the file or into the header area.
    #[error(
        "{what} at {offset:#x} (+{len:#x}) lies outside the data area of a {file_len:#x}-byte file"
    )]
    OutOfBounds {
        /// What was being addressed.
        what: &'static str,
        /// Absolute offset.
        offset: u64,
        /// Length of the range.
        len: u64,
        /// File length.
        file_len: u64,
    },

    /// A structure is internally inconsistent.
    #[error("corrupt data at {offset:#x}: {reason}")]
    Corrupt {
        /// Absolute offset of the structure.
        offset: u64,
        /// Human-readable reason.
        reason: String,
    },

    /// A length exceeds a configured cap ([`crate::ArchiveOptions`]).
    #[error("{what} of {len} bytes exceeds the limit of {limit} bytes")]
    TooLarge {
        /// What was being sized.
        what: &'static str,
        /// Requested length.
        len: u64,
        /// Configured limit.
        limit: u64,
    },

    /// A directory node was reached twice during a walk (invariant I-13).
    #[error("directory node at {offset:#x} is reachable twice (cycle or shared subtree)")]
    TreeCycle {
        /// Offset of the node seen twice.
        offset: u64,
    },

    /// The directory tree is deeper than [`crate::MAX_TREE_DEPTH`] (invariant I-13).
    #[error("directory tree exceeds the depth limit of {limit}")]
    TreeTooDeep {
        /// The depth limit.
        limit: usize,
    },

    /// An allocation uses the legacy chained layout (spec §4.3), which is disabled by default.
    #[error("allocation at {offset:#x} uses the unsupported legacy chained layout")]
    UnsupportedLegacyLayout {
        /// Offset of the allocation.
        offset: u64,
    },

    /// The DID is not present in the archive (or set).
    #[error("DID {did:#010x} not found")]
    NotFound {
        /// The DID looked up.
        did: Did,
    },

    /// A compressed record could not be inflated or did not match its length prefix (I-23).
    #[error("cannot decompress record {did:#010x}: {reason}")]
    Decompress {
        /// The record's DID (0 if unknown).
        did: Did,
        /// Human-readable reason.
        reason: String,
    },

    /// The routing table text could not be parsed.
    #[error("routing table line {line}: {reason}")]
    Routing {
        /// 1-based line number.
        line: usize,
        /// Human-readable reason.
        reason: String,
    },

    /// An archive-kind name is not one of the known kinds (see [`crate::KindTag`]).
    #[error("unknown archive kind `{name}`")]
    UnknownKind {
        /// The name that was given.
        name: String,
    },

    /// A [`crate::Located`] refers to a member index this set does not have (it came
    /// from another [`crate::ArchiveSet`]).
    #[error("archive set has no member {index}")]
    NoSuchMember {
        /// The member index.
        index: usize,
    },

    /// No archive could be opened in the given directory.
    #[error("no client_*.dat archives found in {path}")]
    NoArchives {
        /// The directory searched.
        path: PathBuf,
    },
}

impl Error {
    pub(crate) fn corrupt(offset: u64, reason: impl Into<String>) -> Self {
        Self::Corrupt {
            offset,
            reason: reason.into(),
        }
    }
}
