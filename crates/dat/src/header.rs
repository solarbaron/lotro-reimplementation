// SPDX-License-Identifier: GPL-3.0-or-later
//! File header: version string, journal and superblock (spec §3).

use crate::bytes::{array_at, u32_at};
use crate::error::{Error, Result};
use crate::{ENTRY_LEN, HEADER_LEN, MAGIC};

/// End of the superblock; a header buffer must be at least this long (spec §3.3).
pub(crate) const SUPERBLOCK_END: usize = 0x1A8;

/// The 64-byte "empty journal" pattern starts with these four bytes; the rest is zero (spec §3.2).
pub(crate) const EMPTY_JOURNAL_PREFIX: [u8; 4] = [0x00, 0x50, 0x4C, 0x00];

/// Parsed archive file header (spec §3).
///
/// Field names follow the spec's tables. Values are reported as read; [`Header::parse`]
/// only rejects a wrong magic. Further sanity checks happen in [`crate::Archive::open`]
/// and [`crate::Archive::validate`].
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Header {
    /// 0x000: version string, up to the first NUL (lossy UTF-8). Empty in observed installs.
    pub version: String,
    /// 0x100: 64-byte transaction journal area.
    pub journal: [u8; 64],
    /// 0x140: magic, always [`MAGIC`] after a successful parse.
    pub magic: u32,
    /// 0x144: minimum allocation size ("block size" in older sources), spec §4.4.
    pub min_alloc: u32,
    /// 0x148: file size recorded in the header.
    pub file_size: u32,
    /// 0x14C: dataset descriptor; see [`Header::dataset_type`] and [`Header::archive_kind_code`].
    pub dataset: u32,
    /// 0x150: subset (region number for cell/map archives, 2 for local, 0 otherwise).
    pub subset: u32,
    /// 0x154: offset of the free-space table allocation.
    pub free_table_offset: u32,
    /// 0x158: unused (0 in observed data).
    pub unused_158: u32,
    /// 0x15C: number of free extents listed in the free-space table.
    pub free_count: u32,
    /// 0x160: offset of the root directory node allocation.
    pub root_offset: u32,
    /// 0x164: three unused words (0 in observed data).
    pub unused_164: [u32; 3],
    /// 0x170: "master map" DID (non-zero only in the general archive).
    pub master_map_did: u32,
    /// 0x174: engine/container format version (112 observed; 111 = legacy).
    pub engine_version: u32,
    /// 0x178: game data version.
    pub game_data_version: u32,
    /// 0x17C: 16-byte identifier.
    pub identifier: [u8; 16],
    /// 0x18C: minor version counter.
    pub minor_version: u32,
    /// 0x190: superblock length (104 observed).
    pub superblock_len: u32,
    /// 0x194: directory entry size (32 observed; anything else is refused).
    pub entry_size: u32,
    /// 0x198: child-pointer / allocation-header size (8 observed).
    pub pointer_size: u32,
    /// 0x19C: total free payload bytes (sum of free extent size − 8).
    pub free_bytes: u32,
    /// 0x1A0: size of the free-space table allocation.
    pub free_table_size: u32,
    /// 0x1A4: size of the root directory node allocation.
    pub root_size: u32,
    /// Whether any byte of the padding 0x1A8..0x400 is non-zero (`None` if fewer than
    /// 0x400 bytes were supplied).
    pub padding_nonzero: Option<bool>,
}

impl Header {
    /// Number of bytes [`Header::parse`] needs at minimum.
    pub const MIN_LEN: usize = SUPERBLOCK_END;

    /// Parses a header from the first bytes of an archive.
    ///
    /// `bytes` must hold at least [`Header::MIN_LEN`] bytes; [`HEADER_LEN`] bytes are
    /// needed to inspect the padding. Fails with [`Error::TooSmall`] or [`Error::BadMagic`].
    pub fn parse(bytes: &[u8]) -> Result<Self> {
        if bytes.len() < SUPERBLOCK_END {
            return Err(Error::TooSmall {
                len: bytes.len() as u64,
            });
        }
        let magic = u32_at(bytes, 0x140);
        if magic != MAGIC {
            return Err(Error::BadMagic { found: magic });
        }
        let version_area: [u8; 256] = array_at(bytes, 0);
        let end = version_area.iter().position(|&b| b == 0).unwrap_or(256);
        let version = String::from_utf8_lossy(&version_area[..end]).into_owned();
        let padding_nonzero = bytes
            .get(SUPERBLOCK_END..HEADER_LEN as usize)
            .map(|p| p.iter().any(|&b| b != 0));
        Ok(Self {
            version,
            journal: array_at(bytes, 0x100),
            magic,
            min_alloc: u32_at(bytes, 0x144),
            file_size: u32_at(bytes, 0x148),
            dataset: u32_at(bytes, 0x14C),
            subset: u32_at(bytes, 0x150),
            free_table_offset: u32_at(bytes, 0x154),
            unused_158: u32_at(bytes, 0x158),
            free_count: u32_at(bytes, 0x15C),
            root_offset: u32_at(bytes, 0x160),
            unused_164: [
                u32_at(bytes, 0x164),
                u32_at(bytes, 0x168),
                u32_at(bytes, 0x16C),
            ],
            master_map_did: u32_at(bytes, 0x170),
            engine_version: u32_at(bytes, 0x174),
            game_data_version: u32_at(bytes, 0x178),
            identifier: array_at(bytes, 0x17C),
            minor_version: u32_at(bytes, 0x18C),
            superblock_len: u32_at(bytes, 0x190),
            entry_size: u32_at(bytes, 0x194),
            pointer_size: u32_at(bytes, 0x198),
            free_bytes: u32_at(bytes, 0x19C),
            free_table_size: u32_at(bytes, 0x1A0),
            root_size: u32_at(bytes, 0x1A4),
            padding_nonzero,
        })
    }

    /// Dataset type, byte 0x14F: 1 = portal-type, 2 = cell/map, 3 = local (spec §3.4).
    pub fn dataset_type(&self) -> u8 {
        self.dataset.to_le_bytes()[3]
    }

    /// Archive kind within the dataset type, byte 0x14E (spec §3.4).
    pub fn archive_kind_code(&self) -> u8 {
        self.dataset.to_le_bytes()[2]
    }

    /// Whether the journal area holds the "empty journal" pattern (spec §3.2, I-7).
    pub fn journal_is_empty(&self) -> bool {
        self.journal[..4] == EMPTY_JOURNAL_PREFIX && self.journal[4..].iter().all(|&b| b == 0)
    }

    /// Whether 0x190/0x194/0x198 hold the observed layout sizes 104/32/8 (I-6).
    pub fn layout_sizes_are_standard(&self) -> bool {
        self.superblock_len == 104 && self.entry_size == ENTRY_LEN as u32 && self.pointer_size == 8
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn minimal() -> Vec<u8> {
        let mut b = vec![0u8; HEADER_LEN as usize];
        b[0x100..0x104].copy_from_slice(&EMPTY_JOURNAL_PREFIX);
        b[0x140..0x144].copy_from_slice(&MAGIC.to_le_bytes());
        b[0x14C..0x150].copy_from_slice(&0x0103_0000u32.to_le_bytes());
        b[0x190..0x194].copy_from_slice(&104u32.to_le_bytes());
        b[0x194..0x198].copy_from_slice(&32u32.to_le_bytes());
        b[0x198..0x19C].copy_from_slice(&8u32.to_le_bytes());
        b
    }

    #[test]
    fn parses_descriptor_bytes() {
        let h = Header::parse(&minimal()).unwrap();
        assert_eq!(h.dataset_type(), 1);
        assert_eq!(h.archive_kind_code(), 3);
        assert!(h.journal_is_empty());
        assert!(h.layout_sizes_are_standard());
        assert_eq!(h.padding_nonzero, Some(false));
        assert_eq!(h.version, "");
    }

    #[test]
    fn rejects_short_and_bad_magic() {
        assert!(matches!(
            Header::parse(&[0; 16]),
            Err(Error::TooSmall { .. })
        ));
        let mut b = minimal();
        b[0x140] = 0;
        assert!(matches!(Header::parse(&b), Err(Error::BadMagic { .. })));
    }
}
