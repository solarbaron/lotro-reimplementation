// SPDX-License-Identifier: GPL-3.0-or-later
//! Directory entry (spec §5.2) and flags (spec §5.3).

use crate::bytes::{u16_at, u32_at};
use crate::{Did, ENTRY_LEN, METADATA_DID_BASE};

/// One 32-byte directory entry: where a record lives and how it is stored (spec §5.2).
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub struct Entry {
    /// +0: storage flags. Bit 0 = zlib-compressed; bit 1 always set in observed data.
    pub flags: u16,
    /// +2: small version number of the record ("stamp"); not needed for reading.
    pub stamp: u16,
    /// +4: data identifier (the B-tree key).
    pub did: Did,
    /// +8: absolute offset of the record's first extent.
    pub offset: u32,
    /// +12: stored payload length (includes the 4-byte prefix for compressed records).
    pub length: u32,
    /// +16: Unix time (seconds, UTC) of the last write.
    pub time: u32,
    /// +20: per-record revision counter.
    pub iteration: u32,
    /// +24: size of the first extent allocation.
    pub alloc_size: u32,
    /// +28: reserved, 0 in observed data.
    pub reserved: u32,
}

impl Entry {
    /// Flag bit 0: the payload is a length-prefixed zlib stream (spec §5.3, §6.1).
    pub const FLAG_COMPRESSED: u16 = 0x0001;
    /// Flag bit 1: always set in observed data; meaning unknown.
    pub const FLAG_BIT1: u16 = 0x0002;

    /// Decodes an entry from 32 bytes (shorter input yields zero fields; never panics).
    pub fn parse(b: &[u8]) -> Self {
        Self {
            flags: u16_at(b, 0),
            stamp: u16_at(b, 2),
            did: u32_at(b, 4),
            offset: u32_at(b, 8),
            length: u32_at(b, 12),
            time: u32_at(b, 16),
            iteration: u32_at(b, 20),
            alloc_size: u32_at(b, 24),
            reserved: u32_at(b, 28),
        }
    }

    /// Encodes the entry into its 32-byte on-disk form.
    pub fn to_bytes(&self) -> [u8; ENTRY_LEN] {
        let mut out = [0u8; ENTRY_LEN];
        out[0..2].copy_from_slice(&self.flags.to_le_bytes());
        out[2..4].copy_from_slice(&self.stamp.to_le_bytes());
        for (i, v) in [
            self.did,
            self.offset,
            self.length,
            self.time,
            self.iteration,
            self.alloc_size,
            self.reserved,
        ]
        .into_iter()
        .enumerate()
        {
            let at = 4 + 4 * i;
            out[at..at + 4].copy_from_slice(&v.to_le_bytes());
        }
        out
    }

    /// Whether the stored payload is zlib-compressed (flag bit 0).
    pub fn is_compressed(&self) -> bool {
        self.flags & Self::FLAG_COMPRESSED != 0
    }

    /// Whether this is an archive metadata record (`did ≥ 0xFFFF0000`, spec §7).
    pub fn is_metadata(&self) -> bool {
        self.did >= METADATA_DID_BASE
    }

    /// High byte of the DID, which selects the record type and the routing rule (spec §8).
    pub fn did_high_byte(&self) -> u8 {
        self.did.to_be_bytes()[0]
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn roundtrip() {
        let e = Entry {
            flags: 3,
            stamp: 39,
            did: 0x4100_1234,
            offset: 0x400,
            length: 99,
            time: 1_700_000_000,
            iteration: 7,
            alloc_size: 108,
            reserved: 0,
        };
        assert_eq!(Entry::parse(&e.to_bytes()), e);
        assert!(e.is_compressed());
        assert!(!e.is_metadata());
        assert_eq!(e.did_high_byte(), 0x41);
    }
}
