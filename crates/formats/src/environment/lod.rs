// SPDX-License-Identifier: GPL-3.0-or-later
//! The render LOD-class table (prefix 0x32, DB type 0x3B RENDER_LOD_CLASSES;
//! `environment.md` §9.1, the single owner of this layout).
//!
//! One entry per `Render_LODClass` value with eight f32 values `d0` … `d7` (`v1` … `v8` in
//! `scenery.md` §9.1). `d0` reads as the class's maximum draw distance in world units
//! (Inferred); the meaning of the others is OPEN.

use super::put;
use crate::did::{Did, DidKind};
use crate::error::Result;
use crate::reader::{PREALLOC_CAP, Reader};
use crate::record::{Record, read_prologue};

/// Type prefix of LOD-class tables.
pub const LOD_CLASS_TABLE_PREFIX: u8 = 0x32;

/// Number of f32 values per entry.
pub const LOD_CLASS_VALUES: usize = 8;

/// Stored size of one entry: class key and eight f32.
pub const LOD_CLASS_ENTRY_LEN: usize = 4 + 4 * LOD_CLASS_VALUES;

/// One LOD class (`environment.md` §9.1).
#[derive(Debug, Clone, Copy, PartialEq, Default)]
pub struct LodClass {
    /// A `Render_LODClass` value (enum mapper 0x2300008D in the observed install).
    pub class: u32,
    /// `d0` … `d7`, as stored.
    pub values: [f32; LOD_CLASS_VALUES],
}

impl LodClass {
    /// `d0`, read as the class's maximum draw distance in world units (Inferred,
    /// `environment.md` §9.1).
    #[must_use]
    pub fn max_distance(&self) -> f32 {
        self.values[0]
    }
}

/// The LOD-class table (prefix 0x32; `environment.md` §9.1).
#[derive(Debug, Clone, PartialEq, Default)]
pub struct LodClassTable {
    /// Own DID.
    pub did: Did,
    /// Entries in stored order.
    pub entries: Vec<LodClass>,
}

impl LodClassTable {
    /// The first entry for `class` (stored order).
    #[must_use]
    pub fn get(&self, class: u32) -> Option<&LodClass> {
        self.entries.iter().find(|e| e.class == class)
    }

    /// Number of entries.
    #[must_use]
    pub fn len(&self) -> usize {
        self.entries.len()
    }

    /// Whether the table has no entries.
    #[must_use]
    pub fn is_empty(&self) -> bool {
        self.entries.is_empty()
    }

    /// Re-encodes the record byte for byte.
    #[must_use]
    pub fn to_bytes(&self) -> Vec<u8> {
        let mut out = Vec::with_capacity(8 + LOD_CLASS_ENTRY_LEN * self.entries.len());
        put::did(&mut out, self.did);
        put::count(&mut out, self.entries.len());
        for e in &self.entries {
            put::u32(&mut out, e.class);
            put::f32s(&mut out, &e.values);
        }
        out
    }
}

impl Record<'_> for LodClassTable {
    const NAME: &'static str = "LOD-class table";

    fn did(&self) -> Did {
        self.did
    }

    fn parse(data: &[u8]) -> Result<Self> {
        let mut r = Reader::new(data);
        let did = read_prologue(&mut r, Self::NAME, |k| k == DidKind::LodClassTable)?;
        let count = r.u32()?;
        let mut entries = r.alloc_vec(count, LOD_CLASS_ENTRY_LEN, PREALLOC_CAP)?;
        for _ in 0..count {
            let class = r.u32()?;
            let mut values = [0.0f32; LOD_CLASS_VALUES];
            for v in &mut values {
                *v = r.f32()?;
            }
            r.push(&mut entries, LodClass { class, values }, count)?;
        }
        r.expect_end(Self::NAME)?;
        Ok(Self { did, entries })
    }
}

/// Decodes a LOD-class table payload (for fuzz targets and tools).
pub fn decode_lod_class_table(data: &[u8]) -> Result<LodClassTable> {
    LodClassTable::parse(data)
}
