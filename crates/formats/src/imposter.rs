// SPDX-License-Identifier: GPL-3.0-or-later
//! Imposter tables: records of prefix 0x27 (DB type 0x30), one per square of 27 × 27
//! landblocks of a region (`docs/specs/misc-records.md` §5).
//!
//! A table maps imposter name keys to cells of two texture atlases (a colour atlas and a
//! tangent-space normal atlas) and holds two floats per entry that size the distant sprites
//! of the distant-landscape sectors (`distant-landscape.md` §4.8, `far-view.md` §9):
//! *S* = `value_a` × `scale` is the quad's width and height, *B* = `value_b` × `scale` the
//! height of its lower edge above the sprite position (§5.2).
//!
//! # Layout (§5.2)
//!
//! Own DID; a hashed table (TSize: hint byte and VLE count, `records.md` §1.3) of 14-byte
//! entries (u32 `name_key`, u8 `column`, u8 `row`, f32 `value_a`, f32 `value_b`); the colour
//! atlas DID; the second (normal) atlas DID; f32 `cell_u`; f32 `cell_v`. No bytes remain.
//!
//! # Hostile input
//!
//! The entry count is checked against the remaining bytes (14 per entry) before anything is
//! allocated, and kept entries are charged against the [`Reader`] budget. Floats are kept
//! as stored (the cell and size helpers return `None` for non-finite results); a table whose
//! keys repeat keeps every entry and [`ImposterTable::get`] returns the first.

use crate::did::{Did, DidKind};
use crate::environment::put;
use crate::error::Result;
use crate::reader::{PREALLOC_CAP, Reader, TSize, keys_in_bucket_order};
use crate::record::{Record, read_prologue};

/// Bytes of one stored entry (§5.2).
pub const IMPOSTER_ENTRY_SIZE: usize = 14;

/// Side of the landblock square one table covers (§5.1).
pub const IMPOSTER_TABLE_SQUARE: u8 = 27;

/// DID prefix of imposter tables (§5.1).
pub const IMPOSTER_TABLE_PREFIX: u8 = 0x27;

/// One entry of an imposter table (§5.2).
#[derive(Debug, Clone, Copy, PartialEq, Default)]
pub struct ImposterEntry {
    /// Imposter name key (a key of the RenderImposterNames enum mapper).
    pub name_key: u32,
    /// Atlas column.
    pub column: u8,
    /// Atlas row.
    pub row: u8,
    /// Quad size factor (client default 1.0).
    pub value_a: f32,
    /// Lower-edge height factor (client default 0.0).
    pub value_b: f32,
}

/// Texture coordinates of an atlas cell (§5.4): *u*₀, *v*₀ (top left) and *u*₁, *v*₁
/// (bottom right).
#[derive(Debug, Clone, Copy, PartialEq, Default)]
pub struct AtlasCell {
    /// Left *u*.
    pub u0: f32,
    /// Top *v*.
    pub v0: f32,
    /// Right *u*.
    pub u1: f32,
    /// Bottom *v*.
    pub v1: f32,
}

impl AtlasCell {
    /// The cell's (left, right) *u* values of a sprite quad: (*u*₀, *u*₁), or for a mirrored
    /// sprite (*u*₁ − 1, *u*₀ − 1) (§5.4 "Mirrored sprites").
    #[must_use]
    pub fn u_pair(&self, mirrored: bool) -> (f32, f32) {
        if mirrored {
            (self.u1 - 1.0, self.u0 - 1.0)
        } else {
            (self.u0, self.u1)
        }
    }
}

/// A decoded imposter table (§5.2).
#[derive(Debug, Clone, PartialEq, Default)]
pub struct ImposterTable {
    /// Own DID.
    pub did: Did,
    /// Stored TSize header of the entry table.
    pub size: TSize,
    /// Entries in stored order.
    pub entries: Vec<ImposterEntry>,
    /// Colour atlas (texture list, parameter 3000), or 0.
    pub colour_atlas: Did,
    /// Normal atlas (texture list, parameter 3011), or 0.
    pub second_atlas: Did,
    /// Width of one atlas cell in texture coordinates.
    pub cell_u: f32,
    /// Height of one atlas cell in texture coordinates.
    pub cell_v: f32,
}

impl ImposterTable {
    /// The first entry with `name_key`.
    #[must_use]
    pub fn get(&self, name_key: u32) -> Option<&ImposterEntry> {
        self.entries.iter().find(|e| e.name_key == name_key)
    }

    /// Whether the table has no entries.
    #[must_use]
    pub fn is_empty(&self) -> bool {
        self.entries.is_empty()
    }

    /// The texture coordinates of `entry`'s cell (§5.4, I7): step = cell + cell / 31,
    /// *u*₀ = `column` × step_u, *u*₁ = *u*₀ + `cell_u`, likewise for *v*. `None` when a
    /// value is not finite.
    #[must_use]
    pub fn cell(&self, entry: &ImposterEntry) -> Option<AtlasCell> {
        let step_u = self.cell_u + self.cell_u / 31.0;
        let step_v = self.cell_v + self.cell_v / 31.0;
        let u0 = f32::from(entry.column) * step_u;
        let v0 = f32::from(entry.row) * step_v;
        let c = AtlasCell {
            u0,
            v0,
            u1: u0 + self.cell_u,
            v1: v0 + self.cell_v,
        };
        [c.u0, c.v0, c.u1, c.v1]
            .iter()
            .all(|v| v.is_finite())
            .then_some(c)
    }

    /// Whether the stored keys follow the bucket order of their TSize hint (`records.md`
    /// §1.3.1); `None` for a hint without a known bucket count.
    #[must_use]
    pub fn keys_in_bucket_order(&self) -> Option<bool> {
        keys_in_bucket_order(self.size.hint, self.entries.iter().map(|e| e.name_key))
    }

    /// The table DID of the square that holds landblock (`x`, `y`) of `region` (§5.1).
    #[must_use]
    pub fn did_for(region: u8, x: u8, y: u8) -> Did {
        let sx = x - x % IMPOSTER_TABLE_SQUARE;
        let sy = y - y % IMPOSTER_TABLE_SQUARE;
        Did(u32::from(IMPOSTER_TABLE_PREFIX) << 24
            | u32::from(region) << 16
            | u32::from(sx) << 8
            | u32::from(sy))
    }

    /// Re-encodes the table (§5.2); byte-exact for a decoded table when the stored count
    /// is a shortest VLE (the count is written from the entry list).
    #[must_use]
    pub fn to_bytes(&self) -> Vec<u8> {
        let mut out = Vec::with_capacity(24 + self.entries.len() * IMPOSTER_ENTRY_SIZE);
        put::did(&mut out, self.did);
        put::u8(&mut out, self.size.hint);
        put::vle_count(&mut out, self.entries.len());
        for e in &self.entries {
            put::u32(&mut out, e.name_key);
            put::u8(&mut out, e.column);
            put::u8(&mut out, e.row);
            put::f32(&mut out, e.value_a);
            put::f32(&mut out, e.value_b);
        }
        put::did(&mut out, self.colour_atlas);
        put::did(&mut out, self.second_atlas);
        put::f32(&mut out, self.cell_u);
        put::f32(&mut out, self.cell_v);
        out
    }
}

impl Record<'_> for ImposterTable {
    const NAME: &'static str = "imposter table";

    fn did(&self) -> Did {
        self.did
    }

    fn parse(data: &[u8]) -> Result<Self> {
        let mut r = Reader::new(data);
        let did = read_prologue(&mut r, Self::NAME, |k| k == DidKind::ImposterTable)?;
        let size = r.tsize()?;
        let mut entries = r.alloc_vec(size.count, IMPOSTER_ENTRY_SIZE, PREALLOC_CAP)?;
        for _ in 0..size.count {
            let e = ImposterEntry {
                name_key: r.u32()?,
                column: r.u8()?,
                row: r.u8()?,
                value_a: r.f32()?,
                value_b: r.f32()?,
            };
            r.push(&mut entries, e, size.count)?;
        }
        let colour_atlas = r.did()?;
        let second_atlas = r.did()?;
        let cell_u = r.f32()?;
        let cell_v = r.f32()?;
        r.expect_end(Self::NAME)?;
        r.log_unknown_hints(Self::NAME);
        tracing::trace!(did = %did, entries = entries.len(), "imposter table decoded");
        Ok(Self {
            did,
            size,
            entries,
            colour_atlas,
            second_atlas,
            cell_u,
            cell_v,
        })
    }
}

/// Decodes an imposter table payload (for fuzz targets and tools).
///
/// # Errors
///
/// When the payload is not a complete imposter table (§5.2).
pub fn decode_imposter_table(data: &[u8]) -> Result<ImposterTable> {
    ImposterTable::parse(data)
}
