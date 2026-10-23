// SPDX-License-Identifier: GPL-3.0-or-later
//! Hostile-input reader for the version resource of a Windows PE image.
//!
//! Decision 0042 item 1 makes the executable's version resource the second
//! source of the client version string (after a value the user supplies). This
//! module reads that resource and nothing else: it never looks at code
//! sections, data-section contents or strings outside the version resource.
//!
//! # Format
//!
//! Every structure read here is the documented, public PE/COFF and Win32
//! version-resource layout:
//!
//! - the DOS header (`MZ`, `e_lfanew` at `0x3C`), the `PE\0\0` signature, the
//!   20-byte COFF file header and the PE32 (`0x10B`) or PE32+ (`0x20B`)
//!   optional header with its data directories (index 2 is the resource
//!   directory);
//! - the section table, used to map relative virtual addresses to file offsets;
//! - the three-level resource directory tree (type, name, language) with
//!   `RT_VERSION` (`16`) at the type level and a data entry at the leaf;
//! - the `VS_VERSIONINFO` block tree: `wLength`, `wValueLength`, `wType`, a
//!   NUL-terminated UTF-16 key, 32-bit alignment, the value, 32-bit alignment
//!   and the children. The root value is a `VS_FIXEDFILEINFO` (signature
//!   `0xFEEF04BD`); `StringFileInfo` holds `StringTable` blocks whose children
//!   are the `(key, value)` strings; `VarFileInfo` is skipped.
//!
//! # Hostile input
//!
//! Every count and length is checked against the bytes that remain before it
//! is used, every allocation is capped ([`MAX_VERSION_RESOURCE_BYTES`],
//! [`MAX_VERSION_STRINGS`], [`MAX_PE_SECTIONS`],
//! [`MAX_RESOURCE_DIRECTORY_ENTRIES`]), the block tree is walked to a fixed
//! depth (root, `StringFileInfo`, `StringTable`, `String`; deeper children
//! are never read) and the directory walk has a fixed three levels, so no
//! input can recurse,
//! loop or allocate without bound. The file reader loads only the headers and
//! the version block, never the whole image.

use std::fmt;
use std::fs::File;
use std::io::{Read, Seek, SeekFrom};
use std::path::Path;

/// Maximum number of section headers accepted in a PE image.
pub const MAX_PE_SECTIONS: usize = 96;

/// Maximum number of entries accepted in one resource directory.
pub const MAX_RESOURCE_DIRECTORY_ENTRIES: usize = 4096;

/// Maximum size of the version resource data entry (1 MiB).
pub const MAX_VERSION_RESOURCE_BYTES: usize = 1024 * 1024;

/// Maximum number of `(key, value)` strings collected from the string tables.
pub const MAX_VERSION_STRINGS: usize = 1024;

/// Resource type identifier of version resources (`RT_VERSION`).
const RT_VERSION: u32 = 16;

/// Data directory index of the resource table.
const RESOURCE_DIRECTORY_INDEX: usize = 2;

/// Signature of `VS_FIXEDFILEINFO`.
const FIXED_FILE_INFO_SIGNATURE: u32 = 0xFEEF_04BD;

/// Byte length of `VS_FIXEDFILEINFO`.
const FIXED_FILE_INFO_LEN: usize = 52;

/// Byte length of a version block header (`wLength`, `wValueLength`, `wType`).
const BLOCK_HEADER_LEN: usize = 6;

/// Errors produced while reading a PE version resource.
#[derive(Debug, Clone, PartialEq, Eq, thiserror::Error)]
#[non_exhaustive]
pub enum VersionResourceError {
    /// The file could not be opened, measured or read.
    #[error("cannot read '{path}': {reason}")]
    Io {
        /// Path of the file.
        path: String,
        /// Operating-system error text.
        reason: String,
    },
    /// The bytes are not a PE image (bad `MZ` or `PE\0\0` signature).
    #[error("not a PE image: {detail}")]
    NotPeImage {
        /// What was wrong.
        detail: &'static str,
    },
    /// A structure ran past the end of the image.
    #[error("PE image truncated in {what} at offset {offset:#x}")]
    Truncated {
        /// Structure being read.
        what: &'static str,
        /// File offset of the structure.
        offset: u64,
    },
    /// The optional header magic is neither PE32 (`0x10B`) nor PE32+ (`0x20B`).
    #[error("unsupported PE optional header magic {magic:#06x}")]
    UnsupportedOptionalHeader {
        /// Magic value found.
        magic: u16,
    },
    /// The section count is zero or above [`MAX_PE_SECTIONS`].
    #[error("unsupported PE section count {count} (limit {MAX_PE_SECTIONS})")]
    BadSectionCount {
        /// Section count found.
        count: usize,
    },
    /// The image has no resource directory.
    #[error("PE image has no resource directory")]
    NoResourceDirectory,
    /// A relative virtual address range is not backed by section file data.
    #[error("RVA range {rva:#x}+{len:#x} is not mapped to file data")]
    RvaNotMapped {
        /// Start of the range.
        rva: u32,
        /// Length of the range.
        len: usize,
    },
    /// The resource tree has no version resource.
    #[error("PE image has no version resource")]
    NoVersionResource,
    /// A resource directory lists more than [`MAX_RESOURCE_DIRECTORY_ENTRIES`] entries.
    #[error("resource directory lists {count} entries (limit {MAX_RESOURCE_DIRECTORY_ENTRIES})")]
    TooManyDirectoryEntries {
        /// Entry count found.
        count: usize,
    },
    /// The resource tree has the wrong shape (a leaf where a directory belongs or the reverse).
    #[error("malformed resource directory: {detail}")]
    MalformedResourceDirectory {
        /// What was wrong.
        detail: &'static str,
    },
    /// The version resource is larger than [`MAX_VERSION_RESOURCE_BYTES`].
    #[error("version resource of {size} bytes exceeds the {MAX_VERSION_RESOURCE_BYTES}-byte limit")]
    ResourceTooLarge {
        /// Declared size.
        size: usize,
    },
    /// The `VS_VERSIONINFO` block tree is malformed.
    #[error("malformed version information at offset {offset:#x}: {detail}")]
    MalformedVersionInfo {
        /// Offset inside the version resource.
        offset: usize,
        /// What was wrong.
        detail: &'static str,
    },
    /// The string tables hold more than [`MAX_VERSION_STRINGS`] strings.
    #[error("version resource holds more than {MAX_VERSION_STRINGS} strings")]
    TooManyStrings,
}

/// A four-part version number from `VS_FIXEDFILEINFO`.
#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash)]
pub struct FixedVersion {
    /// High word of the most significant double word.
    pub major: u16,
    /// Low word of the most significant double word.
    pub minor: u16,
    /// High word of the least significant double word.
    pub build: u16,
    /// Low word of the least significant double word.
    pub revision: u16,
}

impl FixedVersion {
    /// Builds a version from its most and least significant double words.
    #[must_use]
    pub const fn from_words(ms: u32, ls: u32) -> Self {
        Self {
            major: (ms >> 16) as u16,
            minor: (ms & 0xFFFF) as u16,
            build: (ls >> 16) as u16,
            revision: (ls & 0xFFFF) as u16,
        }
    }
}

impl fmt::Display for FixedVersion {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(
            f,
            "{}.{}.{}.{}",
            self.major, self.minor, self.build, self.revision
        )
    }
}

/// One `(key, value)` string of a version resource string table.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct VersionString {
    /// Key of the enclosing `StringTable` (language and code page, usually
    /// eight hexadecimal digits).
    pub table: String,
    /// String key.
    pub key: String,
    /// String value, cut at its first NUL.
    pub value: String,
}

/// The parsed version resource of a PE image.
#[derive(Debug, Clone, PartialEq, Eq, Default)]
pub struct VersionResource {
    /// File version from `VS_FIXEDFILEINFO`, when the fixed information is present.
    pub file_version: Option<FixedVersion>,
    /// Product version from `VS_FIXEDFILEINFO`, when the fixed information is present.
    pub product_version: Option<FixedVersion>,
    /// All strings of all string tables, in file order.
    pub strings: Vec<VersionString>,
}

impl VersionResource {
    /// Iterates over the `(key, value)` pairs of all string tables, in file order.
    pub fn string_pairs(&self) -> impl Iterator<Item = (&str, &str)> {
        self.strings
            .iter()
            .map(|s| (s.key.as_str(), s.value.as_str()))
    }

    /// Returns the value of the first string whose key is `key`, comparing
    /// exactly first and then ignoring ASCII case.
    #[must_use]
    pub fn lookup(&self, key: &str) -> Option<&str> {
        self.strings
            .iter()
            .find(|s| s.key == key)
            .or_else(|| {
                self.strings
                    .iter()
                    .find(|s| s.key.eq_ignore_ascii_case(key))
            })
            .map(|s| s.value.as_str())
    }
}

// ---------------------------------------------------------------------------
// Byte sources
// ---------------------------------------------------------------------------

/// Random-access source of image bytes. Every read is bounded by its caller.
trait ImageSource {
    /// Total length of the image in bytes.
    fn image_len(&self) -> u64;

    /// Reads exactly `len` bytes at `offset`.
    fn read_range(
        &mut self,
        offset: u64,
        len: usize,
        what: &'static str,
    ) -> Result<Vec<u8>, VersionResourceError>;
}

struct SliceSource<'a>(&'a [u8]);

impl ImageSource for SliceSource<'_> {
    fn image_len(&self) -> u64 {
        self.0.len() as u64
    }

    fn read_range(
        &mut self,
        offset: u64,
        len: usize,
        what: &'static str,
    ) -> Result<Vec<u8>, VersionResourceError> {
        let start = usize::try_from(offset)
            .map_err(|_| VersionResourceError::Truncated { what, offset })?;
        let end = start
            .checked_add(len)
            .ok_or(VersionResourceError::Truncated { what, offset })?;
        self.0
            .get(start..end)
            .map(<[u8]>::to_vec)
            .ok_or(VersionResourceError::Truncated { what, offset })
    }
}

struct FileSource {
    file: File,
    len: u64,
    path: String,
}

impl ImageSource for FileSource {
    fn image_len(&self) -> u64 {
        self.len
    }

    fn read_range(
        &mut self,
        offset: u64,
        len: usize,
        what: &'static str,
    ) -> Result<Vec<u8>, VersionResourceError> {
        let end = offset
            .checked_add(len as u64)
            .ok_or(VersionResourceError::Truncated { what, offset })?;
        if end > self.len {
            return Err(VersionResourceError::Truncated { what, offset });
        }
        let io_err = |e: std::io::Error, path: &str| VersionResourceError::Io {
            path: path.to_owned(),
            reason: e.to_string(),
        };
        self.file
            .seek(SeekFrom::Start(offset))
            .map_err(|e| io_err(e, &self.path))?;
        let mut buf = vec![0u8; len];
        self.file
            .read_exact(&mut buf)
            .map_err(|e| io_err(e, &self.path))?;
        Ok(buf)
    }
}

// ---------------------------------------------------------------------------
// Little-endian helpers on bounded slices
// ---------------------------------------------------------------------------

fn le_u16(bytes: &[u8], at: usize) -> Option<u16> {
    let b = bytes.get(at..at.checked_add(2)?)?;
    Some(u16::from_le_bytes([*b.first()?, *b.get(1)?]))
}

fn le_u32(bytes: &[u8], at: usize) -> Option<u32> {
    let b = bytes.get(at..at.checked_add(4)?)?;
    Some(u32::from_le_bytes([
        *b.first()?,
        *b.get(1)?,
        *b.get(2)?,
        *b.get(3)?,
    ]))
}

// ---------------------------------------------------------------------------
// PE headers and resource directory
// ---------------------------------------------------------------------------

#[derive(Debug, Clone, Copy)]
struct Section {
    virtual_address: u32,
    raw_size: u32,
    raw_offset: u32,
}

impl Section {
    /// Maps `[rva, rva + len)` to a file offset when the whole range lies in
    /// this section's file data.
    fn map(&self, rva: u32, len: usize) -> Option<u64> {
        let delta = rva.checked_sub(self.virtual_address)?;
        let len_u32 = u32::try_from(len).ok()?;
        let end = delta.checked_add(len_u32)?;
        // Only the raw part of a section is backed by file data.
        if end > self.raw_size {
            return None;
        }
        Some(u64::from(self.raw_offset) + u64::from(delta))
    }
}

struct PeLayout {
    sections: Vec<Section>,
    resource_rva: u32,
    resource_size: u32,
}

impl PeLayout {
    fn map(&self, rva: u32, len: usize) -> Result<u64, VersionResourceError> {
        self.sections
            .iter()
            .find_map(|s| s.map(rva, len))
            .ok_or(VersionResourceError::RvaNotMapped { rva, len })
    }

    /// Reads `len` bytes at offset `rel` from the start of the resource directory.
    fn read_resource_rel(
        &self,
        src: &mut dyn ImageSource,
        rel: u32,
        len: usize,
        what: &'static str,
    ) -> Result<Vec<u8>, VersionResourceError> {
        let len_u32 = u32::try_from(len).map_err(|_| VersionResourceError::RvaNotMapped {
            rva: self.resource_rva,
            len,
        })?;
        // Directory structures must lie inside the declared resource directory.
        if rel
            .checked_add(len_u32)
            .is_none_or(|end| end > self.resource_size)
        {
            return Err(VersionResourceError::MalformedResourceDirectory {
                detail: "directory structure outside the resource directory",
            });
        }
        let rva = self
            .resource_rva
            .checked_add(rel)
            .ok_or(VersionResourceError::RvaNotMapped { rva: u32::MAX, len })?;
        let offset = self.map(rva, len)?;
        src.read_range(offset, len, what)
    }
}

fn read_pe_layout(src: &mut dyn ImageSource) -> Result<PeLayout, VersionResourceError> {
    let dos = src.read_range(0, 0x40, "DOS header")?;
    if dos.get(0..2) != Some(b"MZ".as_slice()) {
        return Err(VersionResourceError::NotPeImage {
            detail: "missing MZ signature",
        });
    }
    let e_lfanew = u64::from(le_u32(&dos, 0x3C).ok_or(VersionResourceError::Truncated {
        what: "DOS header",
        offset: 0,
    })?);

    // Signature (4) + COFF file header (20).
    let nt = src.read_range(e_lfanew, 24, "COFF header")?;
    if nt.get(0..4) != Some(b"PE\0\0".as_slice()) {
        return Err(VersionResourceError::NotPeImage {
            detail: "missing PE\\0\\0 signature",
        });
    }
    let truncated_coff = VersionResourceError::Truncated {
        what: "COFF header",
        offset: e_lfanew,
    };
    let section_count = usize::from(le_u16(&nt, 4 + 2).ok_or(truncated_coff.clone())?);
    let optional_len = usize::from(le_u16(&nt, 4 + 16).ok_or(truncated_coff)?);
    if section_count == 0 || section_count > MAX_PE_SECTIONS {
        return Err(VersionResourceError::BadSectionCount {
            count: section_count,
        });
    }

    let optional_offset = e_lfanew + 24;
    let optional = src.read_range(optional_offset, optional_len, "optional header")?;
    let truncated_optional = VersionResourceError::Truncated {
        what: "optional header",
        offset: optional_offset,
    };
    let magic = le_u16(&optional, 0).ok_or(truncated_optional.clone())?;
    // Offsets of NumberOfRvaAndSizes and of the first data directory.
    let (count_at, dirs_at) = match magic {
        0x010B => (92usize, 96usize),
        0x020B => (108usize, 112usize),
        other => return Err(VersionResourceError::UnsupportedOptionalHeader { magic: other }),
    };
    let dir_count = le_u32(&optional, count_at).ok_or(truncated_optional)?;
    if dir_count as usize <= RESOURCE_DIRECTORY_INDEX {
        return Err(VersionResourceError::NoResourceDirectory);
    }
    let entry_at = dirs_at + RESOURCE_DIRECTORY_INDEX * 8;
    let (Some(resource_rva), Some(resource_size)) =
        (le_u32(&optional, entry_at), le_u32(&optional, entry_at + 4))
    else {
        return Err(VersionResourceError::NoResourceDirectory);
    };
    if resource_rva == 0 || resource_size == 0 {
        return Err(VersionResourceError::NoResourceDirectory);
    }

    let table_offset = optional_offset + optional_len as u64;
    let table = src.read_range(table_offset, section_count * 40, "section table")?;
    let mut sections = Vec::with_capacity(section_count);
    for chunk in table.as_chunks::<40>().0 {
        let field = |at: usize| le_u32(chunk, at).unwrap_or(0);
        sections.push(Section {
            virtual_address: field(12),
            raw_size: field(16),
            raw_offset: field(20),
        });
    }

    Ok(PeLayout {
        sections,
        resource_rva,
        resource_size,
    })
}

/// One entry of a resource directory.
#[derive(Debug, Clone, Copy)]
struct DirEntry {
    /// Numeric identifier, `None` for a named entry.
    id: Option<u32>,
    /// Offset relative to the resource directory start.
    target: u32,
    /// `true` when `target` is a subdirectory.
    is_directory: bool,
}

fn read_directory(
    layout: &PeLayout,
    src: &mut dyn ImageSource,
    rel: u32,
) -> Result<Vec<DirEntry>, VersionResourceError> {
    let header = layout.read_resource_rel(src, rel, 16, "resource directory")?;
    let named = usize::from(le_u16(&header, 12).unwrap_or(0));
    let ids = usize::from(le_u16(&header, 14).unwrap_or(0));
    let count = named + ids;
    if count > MAX_RESOURCE_DIRECTORY_ENTRIES {
        return Err(VersionResourceError::TooManyDirectoryEntries { count });
    }
    let entries_rel =
        rel.checked_add(16)
            .ok_or(VersionResourceError::MalformedResourceDirectory {
                detail: "directory offset overflow",
            })?;
    let raw =
        layout.read_resource_rel(src, entries_rel, count * 8, "resource directory entries")?;
    let mut out = Vec::with_capacity(count);
    for chunk in raw.as_chunks::<8>().0 {
        let name = le_u32(chunk, 0).unwrap_or(0);
        let data = le_u32(chunk, 4).unwrap_or(0);
        out.push(DirEntry {
            id: (name & 0x8000_0000 == 0).then_some(name),
            target: data & 0x7FFF_FFFF,
            is_directory: data & 0x8000_0000 != 0,
        });
    }
    Ok(out)
}

/// Reads the version resource bytes of the image in `src`.
fn read_version_block(src: &mut dyn ImageSource) -> Result<Vec<u8>, VersionResourceError> {
    let layout = read_pe_layout(src)?;

    // Level 1: resource type.
    let types = read_directory(&layout, src, 0)?;
    let version_type = types
        .iter()
        .find(|e| e.id == Some(RT_VERSION))
        .ok_or(VersionResourceError::NoVersionResource)?;
    if !version_type.is_directory {
        return Err(VersionResourceError::MalformedResourceDirectory {
            detail: "version type entry is not a directory",
        });
    }

    // Level 2: resource name; the version resource is normally ID 1.
    let names = read_directory(&layout, src, version_type.target)?;
    let name = names
        .iter()
        .find(|e| e.id == Some(1))
        .or_else(|| names.first())
        .ok_or(VersionResourceError::NoVersionResource)?;
    if !name.is_directory {
        return Err(VersionResourceError::MalformedResourceDirectory {
            detail: "version name entry is not a directory",
        });
    }

    // Level 3: language; take the first.
    let languages = read_directory(&layout, src, name.target)?;
    let language = languages
        .first()
        .ok_or(VersionResourceError::NoVersionResource)?;
    if language.is_directory {
        return Err(VersionResourceError::MalformedResourceDirectory {
            detail: "version language entry is not a data entry",
        });
    }

    // Leaf: IMAGE_RESOURCE_DATA_ENTRY { OffsetToData (RVA), Size, CodePage, Reserved }.
    let leaf = layout.read_resource_rel(src, language.target, 16, "resource data entry")?;
    let data_rva = le_u32(&leaf, 0).unwrap_or(0);
    let data_size = le_u32(&leaf, 4).unwrap_or(0) as usize;
    if data_size > MAX_VERSION_RESOURCE_BYTES {
        return Err(VersionResourceError::ResourceTooLarge { size: data_size });
    }
    if data_size < BLOCK_HEADER_LEN {
        return Err(VersionResourceError::MalformedVersionInfo {
            offset: 0,
            detail: "version resource shorter than one block header",
        });
    }
    let offset = layout.map(data_rva, data_size)?;
    src.read_range(offset, data_size, "version resource")
}

// ---------------------------------------------------------------------------
// VS_VERSIONINFO block tree
// ---------------------------------------------------------------------------

/// A version block whose bounds were checked against its parent.
struct Block<'a> {
    data: &'a [u8],
    start: usize,
    end: usize,
    value_len: usize,
    key: String,
    value_start: usize,
    children_start: usize,
}

const fn align4(offset: usize) -> usize {
    (offset + 3) & !3
}

fn malformed(offset: usize, detail: &'static str) -> VersionResourceError {
    VersionResourceError::MalformedVersionInfo { offset, detail }
}

/// Reads the block at `start`, which must end at or before `limit`.
fn read_block(data: &[u8], start: usize, limit: usize) -> Result<Block<'_>, VersionResourceError> {
    let length =
        usize::from(le_u16(data, start).ok_or(malformed(start, "truncated block header"))?);
    let value_len =
        usize::from(le_u16(data, start + 2).ok_or(malformed(start, "truncated block header"))?);
    if length < BLOCK_HEADER_LEN {
        return Err(malformed(start, "block length shorter than its header"));
    }
    let end = start
        .checked_add(length)
        .ok_or(malformed(start, "block length overflow"))?;
    if end > limit {
        return Err(malformed(start, "block runs past its parent"));
    }

    // NUL-terminated UTF-16 key.
    let mut units = Vec::new();
    let mut cursor = start + BLOCK_HEADER_LEN;
    loop {
        let unit = le_u16(data, cursor)
            .filter(|_| cursor + 2 <= end)
            .ok_or(malformed(cursor, "unterminated block key"))?;
        cursor += 2;
        if unit == 0 {
            break;
        }
        units.push(unit);
    }
    let key = String::from_utf16_lossy(&units);
    let value_start = align4(cursor).min(end);
    Ok(Block {
        data,
        start,
        end,
        value_len,
        key,
        value_start,
        children_start: value_start,
    })
}

/// Iterates over the children of `block` that start at `first`.
fn for_each_child<'a>(
    block: &Block<'a>,
    first: usize,
    mut f: impl FnMut(Block<'a>) -> Result<(), VersionResourceError>,
) -> Result<(), VersionResourceError> {
    let mut cursor = align4(first);
    while cursor.saturating_add(BLOCK_HEADER_LEN) <= block.end {
        // A zero `wLength` is trailing zero padding, not a block: stop here
        // and keep what was read so far.
        if le_u16(block.data, cursor) == Some(0) {
            break;
        }
        let child = read_block(block.data, cursor, block.end)?;
        let next = align4(child.end);
        f(child)?;
        if next <= cursor {
            return Err(malformed(cursor, "block does not advance"));
        }
        cursor = next;
    }
    Ok(())
}

/// Reads the text value of a `String` block: UTF-16 from the value start to the
/// end of the block, cut at the first NUL.
fn string_value(block: &Block<'_>) -> String {
    if block.value_len == 0 {
        return String::new();
    }
    let raw = block.data.get(block.value_start..block.end).unwrap_or(&[]);
    let units: Vec<u16> = raw
        .as_chunks::<2>()
        .0
        .iter()
        .map(|&pair| u16::from_le_bytes(pair))
        .take_while(|&u| u != 0)
        .collect();
    String::from_utf16_lossy(&units)
}

/// Parses a `VS_VERSIONINFO` block tree (the bytes of a version resource).
///
/// # Errors
///
/// Returns [`VersionResourceError::MalformedVersionInfo`] when a block is
/// truncated, overruns its parent or has the wrong root key, and
/// [`VersionResourceError::TooManyStrings`] past [`MAX_VERSION_STRINGS`].
pub fn parse_version_info(data: &[u8]) -> Result<VersionResource, VersionResourceError> {
    if data.len() > MAX_VERSION_RESOURCE_BYTES {
        return Err(VersionResourceError::ResourceTooLarge { size: data.len() });
    }
    let root = read_block(data, 0, data.len())?;
    if root.key != "VS_VERSION_INFO" {
        return Err(malformed(0, "root block key is not VS_VERSION_INFO"));
    }

    let mut resource = VersionResource::default();
    let mut children_start = root.children_start;
    if root.value_len > 0 {
        let fixed_end = root
            .value_start
            .checked_add(root.value_len)
            .ok_or(malformed(root.start, "fixed information overflow"))?;
        if fixed_end > root.end {
            return Err(malformed(
                root.value_start,
                "fixed information runs past its block",
            ));
        }
        if root.value_len >= FIXED_FILE_INFO_LEN
            && le_u32(data, root.value_start) == Some(FIXED_FILE_INFO_SIGNATURE)
        {
            let word = |n: usize| le_u32(data, root.value_start + 4 * n).unwrap_or(0);
            resource.file_version = Some(FixedVersion::from_words(word(2), word(3)));
            resource.product_version = Some(FixedVersion::from_words(word(4), word(5)));
        }
        children_start = fixed_end;
    }

    // Depth 1: StringFileInfo / VarFileInfo.
    for_each_child(&root, children_start, |info| {
        if info.key != "StringFileInfo" {
            return Ok(());
        }
        // Depth 2: StringTable.
        for_each_child(&info, info.children_start, |table| {
            let table_key = table.key.clone();
            // Depth 3: String.
            for_each_child(&table, table.children_start, |string| {
                if resource.strings.len() >= MAX_VERSION_STRINGS {
                    return Err(VersionResourceError::TooManyStrings);
                }
                let value = string_value(&string);
                resource.strings.push(VersionString {
                    table: table_key.clone(),
                    key: string.key,
                    value,
                });
                Ok(())
            })
        })
    })?;
    Ok(resource)
}

/// Reads the version resource of a PE image held in memory.
///
/// # Errors
///
/// Returns a [`VersionResourceError`] when the bytes are not a PE image, have
/// no version resource, or any structure on the way is truncated or malformed.
pub fn read_version_resource(image: &[u8]) -> Result<VersionResource, VersionResourceError> {
    let block = read_version_block(&mut SliceSource(image))?;
    parse_version_info(&block)
}

/// Reads the version resource of the PE image at `path`, read-only. Only the
/// headers, the resource directory entries on the way and the version block
/// are read from the file.
///
/// # Errors
///
/// Returns [`VersionResourceError::Io`] when the file cannot be read, and the
/// errors of [`read_version_resource`] otherwise.
pub fn read_version_resource_from_file(
    path: &Path,
) -> Result<VersionResource, VersionResourceError> {
    let display = path.display().to_string();
    let file = File::open(path).map_err(|e| VersionResourceError::Io {
        path: display.clone(),
        reason: e.to_string(),
    })?;
    let len = file
        .metadata()
        .map_err(|e| VersionResourceError::Io {
            path: display.clone(),
            reason: e.to_string(),
        })?
        .len();
    let mut src = FileSource {
        file,
        len,
        path: display,
    };
    let block = read_version_block(&mut src)?;
    tracing::debug!(
        bytes = block.len(),
        image_len = src.image_len(),
        "read PE version resource"
    );
    parse_version_info(&block)
}
