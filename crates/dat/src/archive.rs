// SPDX-License-Identifier: GPL-3.0-or-later
//! A single opened archive file.

use std::collections::HashMap;
use std::fmt;
use std::fs::File;
use std::path::{Path, PathBuf};
use std::sync::{Arc, RwLock};

use crate::btree::{ChildPtr, Node};
use crate::bytes::{array_at, u32_at};
use crate::compress::inflate_record;
use crate::entry::Entry;
use crate::error::{Error, Result};
use crate::extent::{ALLOC_HEADER_LEN, RecordLayout};
use crate::header::Header;
use crate::source::Source;
use crate::{
    CURRENT_ENGINE_VERSION, Did, ENTRY_LEN, HEADER_LEN, LEGACY_ENGINE_VERSION, MIN_NODE_ALLOC,
    SET_MEMBERSHIP_DID,
};

/// Length of the set-membership record 0xFFFF0005 (spec §7.2).
const MEMBERSHIP_LEN: usize = 24;

/// Largest stored or inflated set-membership record accepted (implementation choice,
/// spec §14 implementer notes). The record is 24 bytes in every observed archive.
const MEMBERSHIP_MAX_LEN: u64 = 1024;

/// Limits and switches for reading an archive.
///
/// The defaults are safe for hostile input and comfortably above everything observed in a
/// real install (spec §9).
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ArchiveOptions {
    /// Largest stored (on-disk) record payload accepted, in bytes. Default 256 MiB
    /// (observed maximum ≈ 39 MiB).
    pub max_record_len: u64,
    /// Largest inflated record accepted, in bytes. Default 128 MiB (observed maximum
    /// ≈ 17 MiB; the spec asks for "comfortably above 32 MiB").
    pub max_inflated_len: u64,
    /// Enable the legacy chained allocation layout (spec §4.3). It was never observed in
    /// a current install and its byte accounting is unconfirmed, so it is off by default and
    /// such allocations fail with [`Error::UnsupportedLegacyLayout`].
    pub legacy_layout: bool,
    /// Maximum number of interior directory nodes kept in the per-archive cache. Leaves
    /// are never cached. 0 disables the cache.
    pub node_cache_capacity: usize,
    /// Largest fragment count `F` accepted in a modern-layout allocation (spec §4.2).
    /// Larger tables fail with [`Error::TooLarge`] before anything is allocated. Default
    /// 4096 (observed maximum 31).
    pub max_fragments: u64,
    /// Largest free-extent count accepted in the free-space table (spec §4.5). Default
    /// 65,536 (observed maximum 84).
    pub max_free_extents: u64,
}

impl Default for ArchiveOptions {
    fn default() -> Self {
        Self {
            max_record_len: 256 << 20,
            max_inflated_len: 128 << 20,
            legacy_layout: false,
            node_cache_capacity: 16_384,
            max_fragments: 4096,
            max_free_extents: 1 << 16,
        }
    }
}

/// One free extent listed in the free-space table (spec §4.5).
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct FreeExtent {
    /// Size of the free extent in bytes.
    pub size: u32,
    /// Absolute offset of the free extent.
    pub offset: u32,
}

/// The free-space table (spec §4.5).
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct FreeTable {
    /// The table allocation's 8-byte header words (`{0, 0}` in observed data).
    pub header: [u32; 2],
    /// The first `count` pairs, in table order.
    pub extents: Vec<FreeExtent>,
    /// The first `u32` of the terminating pair after the last listed pair (0 observed).
    /// Always `Some` from [`Archive::free_table`], which requires the pair to fit (§4.5).
    pub terminator: Option<u32>,
}

/// Contents of the set-membership metadata record 0xFFFF0005 (spec §7.2).
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub struct SetMembership {
    /// Identifier shared by all parts of one archive kind.
    pub set_id: [u8; 16],
    /// Part index: 0 for the `.dat` base, N for `_aux_N.datx`.
    pub part_index: u32,
    /// Number of parts in the set.
    pub part_count: u32,
}

/// One opened archive (`.dat` or `.datx`), read-only and thread-safe.
///
/// The file is opened read-only without locks and accessed with positional reads, so one
/// `Archive` can be shared by any number of threads.
pub struct Archive {
    source: Source,
    path: Option<PathBuf>,
    file_len: u64,
    header: Header,
    options: ArchiveOptions,
    pub(crate) node_cache: RwLock<HashMap<(u32, u32), Arc<Node>>>,
}

impl fmt::Debug for Archive {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.debug_struct("Archive")
            .field("path", &self.path)
            .field("file_len", &self.file_len)
            .field("header", &self.header)
            .field("options", &self.options)
            .finish_non_exhaustive()
    }
}

impl Archive {
    /// Opens an archive file read-only with default options.
    pub fn open(path: impl AsRef<Path>) -> Result<Self> {
        Self::open_with(path, ArchiveOptions::default())
    }

    /// Opens an archive file read-only.
    ///
    /// Fails if the header is unusable: file shorter than 0x400 bytes (I-1), wrong magic
    /// (I-2), zero minimum allocation size (I-4), entry size ≠ 32 (I-6) or an out-of-range
    /// root node (I-8). Other header oddities are logged as warnings.
    pub fn open_with(path: impl AsRef<Path>, options: ArchiveOptions) -> Result<Self> {
        let path = path.as_ref();
        let file = File::open(path).map_err(|source| Error::Io {
            path: path.to_owned(),
            source,
        })?;
        Self::from_source(Source::File(file), Some(path.to_owned()), options)
    }

    /// Opens an archive held in memory (useful for tests and fuzzing).
    pub fn from_bytes(bytes: impl Into<Arc<[u8]>>, options: ArchiveOptions) -> Result<Self> {
        Self::from_source(Source::Memory(bytes.into()), None, options)
    }

    fn from_source(source: Source, path: Option<PathBuf>, options: ArchiveOptions) -> Result<Self> {
        let file_len = source.len().map_err(|source| Error::Read {
            offset: 0,
            len: 0,
            source,
        })?;
        if file_len < HEADER_LEN {
            return Err(Error::TooSmall { len: file_len });
        }
        let mut buf = vec![0u8; HEADER_LEN as usize];
        source
            .read_exact_at(&mut buf, 0)
            .map_err(|source| Error::Read {
                offset: 0,
                len: HEADER_LEN,
                source,
            })?;
        let header = Header::parse(&buf)?;
        if header.min_alloc == 0 {
            return Err(Error::BadHeader {
                invariant: 4,
                reason: "minimum allocation size (0x144) is 0".into(),
            });
        }
        if header.entry_size != ENTRY_LEN as u32 {
            return Err(Error::BadHeader {
                invariant: 6,
                reason: format!(
                    "directory entry size (0x194) is {}, only 32 is supported",
                    header.entry_size
                ),
            });
        }
        let root_end = u64::from(header.root_offset) + u64::from(header.root_size);
        if u64::from(header.root_offset) < HEADER_LEN
            || header.root_size < MIN_NODE_ALLOC
            || root_end > file_len
        {
            return Err(Error::BadHeader {
                invariant: 8,
                reason: format!(
                    "root node {:#x}+{:#x} is out of range for a {file_len:#x}-byte file",
                    header.root_offset, header.root_size
                ),
            });
        }
        let shown = path.as_deref().map(Path::display);
        if u64::from(header.file_size) != file_len {
            tracing::warn!(
                path = ?shown,
                header = header.file_size,
                actual = file_len,
                "header file size differs from the real length (concurrent patching?)"
            );
        }
        if header.engine_version != CURRENT_ENGINE_VERSION {
            tracing::warn!(path = ?shown, version = header.engine_version, "unexpected engine version");
            if header.engine_version == LEGACY_ENGINE_VERSION && !options.legacy_layout {
                tracing::warn!(path = ?shown, "engine version 111 suggests the legacy layout, which is disabled");
            }
        }
        if !header.layout_sizes_are_standard() {
            tracing::warn!(path = ?shown, "header layout sizes are not 104/32/8");
        }
        if !header.journal_is_empty() {
            tracing::warn!(path = ?shown, "journal is not empty (interrupted patch?)");
        }
        Ok(Self {
            source,
            path,
            file_len,
            header,
            options,
            node_cache: RwLock::new(HashMap::new()),
        })
    }

    /// The parsed file header.
    pub fn header(&self) -> &Header {
        &self.header
    }

    /// The file path, if the archive was opened from a file.
    pub fn path(&self) -> Option<&Path> {
        self.path.as_deref()
    }

    /// The file length in bytes, as measured when the archive was opened.
    pub fn file_len(&self) -> u64 {
        self.file_len
    }

    /// The options this archive was opened with.
    pub fn options(&self) -> &ArchiveOptions {
        &self.options
    }

    /// The root node pointer from the header (0x160 / 0x1A4).
    pub fn root(&self) -> ChildPtr {
        ChildPtr {
            size: self.header.root_size,
            offset: self.header.root_offset,
        }
    }

    /// Whether the archive contains `did`.
    pub fn contains(&self, did: Did) -> Result<bool> {
        Ok(self.lookup(did)?.is_some())
    }

    /// Reads and, if needed, inflates the record `did`. Fails with [`Error::NotFound`] if
    /// it is absent.
    pub fn read(&self, did: Did) -> Result<Vec<u8>> {
        let entry = self.lookup(did)?.ok_or(Error::NotFound { did })?;
        self.read_entry(&entry)
    }

    /// Reads the record described by `entry` and inflates it if its flag bit 0 is set.
    ///
    /// Only per-record caps apply ([`ArchiveOptions::max_record_len`],
    /// [`ArchiveOptions::max_inflated_len`], spec §9). The reader does not check that
    /// records occupy disjoint bytes: in a hostile archive many entries can name the same
    /// stored bytes, so a caller that reads every record (a snapshot, an export) should
    /// budget the aggregate it reads, or run the validator first, which reports such
    /// overlap (I-20) and inflates shared bytes only once.
    pub fn read_entry(&self, entry: &Entry) -> Result<Vec<u8>> {
        let stored = self.read_raw(entry)?;
        if !entry.is_compressed() {
            return Ok(stored);
        }
        inflate_record(&stored, self.options.max_inflated_len).map_err(|e| match e {
            Error::Decompress { reason, .. } => Error::Decompress {
                did: entry.did,
                reason,
            },
            other => other,
        })
    }

    /// Reads the stored (still compressed, if applicable) payload of `entry`.
    pub fn read_raw(&self, entry: &Entry) -> Result<Vec<u8>> {
        let layout = self.layout(entry)?;
        self.read_payload(&layout, u64::from(entry.length))
    }

    /// Describes where the bytes of `entry` live (first extent, fragments).
    pub fn layout(&self, entry: &Entry) -> Result<RecordLayout> {
        self.layout_at(entry.offset, entry.alloc_size)
    }

    /// Reads the free-space table (spec §4.5).
    pub fn free_table(&self) -> Result<FreeTable> {
        let h = &self.header;
        let (off, size, count) = (
            u64::from(h.free_table_offset),
            u64::from(h.free_table_size),
            u64::from(h.free_count),
        );
        // No shortcut for a zero-sized table: every version-112 archive has one (§4.5,
        // §4.6), so a missing table fails the size check below and the validator reports
        // it as I-21.
        self.check_range("free-space table", off, size)?;
        let max = self.options.max_free_extents;
        if count > max {
            return Err(Error::TooLarge {
                what: "free-space table",
                len: count.saturating_mul(8),
                limit: max.saturating_mul(8),
            });
        }
        let need = ALLOC_HEADER_LEN + 8 * count;
        // Spec §4.5: the 8-byte header, `count` pairs and the terminating pair must fit,
        // `16 + 8 × count ≤ size` (I-21).
        if need + 8 > size {
            return Err(Error::corrupt(
                off,
                format!(
                    "{count} free extents and the terminating pair do not fit a {size}-byte table"
                ),
            ));
        }
        let with_term = need + 4;
        let mut buf = vec![0u8; with_term as usize];
        self.read_at(off, &mut buf)?;
        let extents = buf
            .get(ALLOC_HEADER_LEN as usize..need as usize)
            .unwrap_or(&[])
            .as_chunks::<8>()
            .0
            .iter()
            .map(|p| FreeExtent {
                size: u32_at(p, 0),
                offset: u32_at(p, 4),
            })
            .collect();
        Ok(FreeTable {
            header: [u32_at(&buf, 0), u32_at(&buf, 4)],
            extents,
            terminator: Some(u32_at(&buf, need as usize)),
        })
    }

    /// Reads the set-membership record 0xFFFF0005, if present (spec §7.2).
    pub fn membership(&self) -> Result<Option<SetMembership>> {
        let Some(entry) = self.lookup(SET_MEMBERSHIP_DID)? else {
            return Ok(None);
        };
        let at = u64::from(entry.offset);
        // The record is 24 bytes (spec §7.2) and stored uncompressed in every observed
        // archive (§7.1). A compressed one is still decoded, but both the stored and the
        // inflated size are capped far below the general record caps, so a hostile
        // 0xFFFF0005 cannot make every set open read or inflate megabytes.
        if u64::from(entry.length) > MEMBERSHIP_MAX_LEN {
            return Err(Error::TooLarge {
                what: "set-membership record",
                len: u64::from(entry.length),
                limit: MEMBERSHIP_MAX_LEN,
            });
        }
        let stored = self.read_raw(&entry)?;
        let b = if entry.is_compressed() {
            inflate_record(&stored, MEMBERSHIP_MAX_LEN).map_err(|e| match e {
                Error::Decompress { reason, .. } => Error::Decompress {
                    did: entry.did,
                    reason,
                },
                other => other,
            })?
        } else {
            stored
        };
        // Check the decoded length, not the stored one: for a compressed record the stored
        // length includes the prefix and the zlib stream.
        if b.len() < MEMBERSHIP_LEN {
            return Err(Error::corrupt(
                at,
                format!("set-membership record is only {} bytes", b.len()),
            ));
        }
        Ok(Some(SetMembership {
            set_id: array_at(&b, 0),
            part_index: u32_at(&b, 16),
            part_count: u32_at(&b, 20),
        }))
    }

    /// Positional read mapped to [`Error::Read`].
    pub(crate) fn read_at(&self, offset: u64, buf: &mut [u8]) -> Result<()> {
        self.source
            .read_exact_at(buf, offset)
            .map_err(|source| Error::Read {
                offset,
                len: buf.len() as u64,
                source,
            })
    }
}
