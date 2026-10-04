// SPDX-License-Identifier: GPL-3.0-or-later
//! Read-only reader for the `.dat` / `.datx` archive container.
//!
//! This crate implements `docs/specs/dat-container.md` (the "spec" below). It covers the
//! container only: the file header, the allocation (extent) layer, the directory B-tree,
//! zlib-compressed records, the archive set of an install and DID → archive routing. What
//! is *inside* a record is the business of `tessera-formats`.
//!
//! # Overview
//!
//! * [`Archive`] opens one archive file read-only (positional reads, no locking, no
//!   memory map, so a launcher rewriting the file cannot crash the reader with `SIGBUS`).
//!   It offers [`Archive::lookup`], [`Archive::entries`] (sorted in-order iteration;
//!   [`Archive::entries_range`] for one DID range),
//!   [`Archive::read`] (reassembled and inflated payload) and [`Archive::validate`].
//! * [`ArchiveSet`] opens every `client_*.dat` / `client_*.datx` archive in an install
//!   directory and routes a DID to the archives likely to hold it ([`RoutingTable`]),
//!   falling back to searching all of them.
//! * All input is treated as hostile: every offset and size is bounds-checked, tree walks
//!   carry depth and cycle guards, and allocations are capped ([`ArchiveOptions`],
//!   [`ValidateOptions::max_tracked`], [`MAX_SET_PARTS`]).
//!
//! ```no_run
//! use tessera_dat::{ArchiveSet, SetOptions};
//!
//! # fn main() -> Result<(), tessera_dat::Error> {
//! let set = ArchiveSet::open("/path/to/install", SetOptions::default())?;
//! let (found, bytes) = set.read_with_entry(0x0100_0001)?;
//! if let Some(member) = set.archive_of(&found) {
//!     println!("{} bytes from {}", bytes.len(), member.id);
//! }
//! // Every record of one DID class, without walking the whole tree:
//! if let Some(general) = set.members().first() {
//!     for entry in general.archive.entries_range(0x2500_0000..0x2600_0000) {
//!         let entry = entry?;
//!         println!("{:#010x}: {} bytes", entry.did, entry.length);
//!     }
//! }
//! # Ok(())
//! # }
//! ```
//!
//! [`Archive`] is `Send + Sync`; share it between threads behind an `Arc` or a reference.

#![warn(missing_docs)]

mod archive;
mod btree;
mod bytes;
mod compress;
mod entry;
mod error;
mod extent;
mod header;
mod routing;
mod set;
mod source;
mod validate;

pub use archive::{Archive, ArchiveOptions, FreeExtent, FreeTable, SetMembership};
pub use btree::{ChildPtr, Entries};
pub use compress::inflate_record;
pub use entry::Entry;
pub use error::{Error, Result};
pub use extent::{Fragment, LegacyBlock, RecordLayout};
pub use header::Header;
pub use routing::{DEFAULT_ROUTING, KindTag, RouteRule, RouteTarget, RoutingTable};
pub use set::{
    ArchiveId, ArchiveKind, ArchiveSet, Located, MAX_SET_PARTS, SetMember, SetOptions, SetProblem,
    SetValidationReport,
};
pub use validate::{Issue, Severity, ValidateOptions, ValidationReport, ValidationStats};

/// A 32-bit data identifier, the key of the directory B-tree (spec §5.2).
///
/// The container layer keeps it a plain `u32` so that integer literals work at every call
/// site. Record-level crates may wrap it in a newtype; such a type converts with
/// `u32::from(did)` / `did.into()` at this boundary.
pub type Did = u32;

/// Length of the file header area; the first allocation starts here (spec §3).
pub const HEADER_LEN: u64 = 0x400;

/// Magic number stored at offset 0x140 (spec §3.3).
pub const MAGIC: u32 = 0x0000_5442;

/// Size of the fixed directory-node payload that follows the 8-byte allocation header (spec §5.1).
pub const NODE_PAYLOAD_LEN: usize = 2452;

/// Smallest valid directory-node allocation: 8-byte header + [`NODE_PAYLOAD_LEN`] (spec §5.6).
pub const MIN_NODE_ALLOC: u32 = 2460;

/// Maximum number of entries in one directory node (spec §5.1).
pub const MAX_NODE_ENTRIES: usize = 61;

/// Number of child-pointer slots in one directory node (spec §5.1).
pub const NODE_CHILD_SLOTS: usize = 62;

/// Size of one directory entry in bytes (spec §5.2).
pub const ENTRY_LEN: usize = 32;

/// Depth bound for B-tree descent; deeper trees are rejected as corrupt (spec §5.5, I-13).
pub const MAX_TREE_DEPTH: usize = 16;

/// Step limit for walking a legacy chained allocation (spec §4.3).
pub const MAX_LEGACY_STEPS: usize = 1000;

/// DIDs at or above this value are archive metadata records, not game content (spec §7).
pub const METADATA_DID_BASE: Did = 0xFFFF_0000;

/// DID of the set-membership metadata record (spec §7.2).
pub const SET_MEMBERSHIP_DID: Did = 0xFFFF_0005;

/// Engine/container format version observed in current installs (header 0x174, spec §3.3).
pub const CURRENT_ENGINE_VERSION: u32 = 112;

/// Engine version that sources associate with the legacy chained layout (spec §4.3).
pub const LEGACY_ENGINE_VERSION: u32 = 111;
