// SPDX-License-Identifier: GPL-3.0-or-later
//! Imposter tables: records of prefix 0x27 (DB type 0x30), one per square of 27 × 27
//! landblocks of a region (`docs/specs/misc-records.md` §5). The one decoder of these
//! records: the far view, the gameplay views and the tools all use it.
//!
//! A table maps imposter name keys to cells of two texture atlases (a colour atlas and a
//! tangent-space normal atlas) and holds two floats per entry that size the distant sprites
//! of the distant-landscape sectors (`distant-landscape.md` §4.8, `far-view.md` §9):
//! *S* = `value_a` × `scale` is the quad's width and height, *B* = `value_b` × `scale` the
//! height of its lower edge above the sprite position (§5.2).
//!
//! | Item | Spec |
//! |---|---|
//! | [`ImposterSquare`], [`ImposterTable::did_for`] | the table DID of a landblock's square (§5.1) |
//! | [`ImposterTable`], [`ImposterEntry`], [`decode_imposter_table`] | layout (§5.2) |
//! | [`ImposterTable::dependencies`] | the two atlases (§5.3) |
//! | [`imposter_atlas_grid`], [`ImposterTable::cell`], [`AtlasCell::u_pair`] | atlas geometry, cell texture coordinates (I7), mirrored sprites (§5.4) |
//! | [`ImposterTable::first_unusual_field`] | the observed invariant MR5 (§16) |
//!
//! # Layout (§5.2)
//!
//! Own DID; a hashed table (TSize: hint byte and VLE count, `records.md` §1.3) of 14-byte
//! entries (u32 `name_key`, u8 `column`, u8 `row`, f32 `value_a`, f32 `value_b`); the colour
//! atlas DID; the second (normal) atlas DID; f32 `cell_u`; f32 `cell_v`. No bytes remain.
//!
//! # Hostile input
//!
//! A bucket hint above [`MAX_TABLE_HINT`] is rejected (MR12). The entry count is checked
//! against the remaining bytes (14 per entry) before anything is allocated, and kept
//! entries are charged against the [`Reader`] budget. Floats are kept as stored (the cell
//! and size helpers return `None` for non-finite results); a table whose keys repeat keeps
//! every entry and [`ImposterTable::get`] returns the first.

use crate::did::{Did, DidKind};
use crate::environment::put;
use crate::error::{FormatError, Result};
use crate::gameplay::misc::MAX_TABLE_HINT;
use crate::reader::{PREALLOC_CAP, Reader, TSize, keys_in_bucket_order};
use crate::record::{Record, read_prologue};

/// Bytes of one stored entry (§5.2).
pub const IMPOSTER_ENTRY_SIZE: usize = 14;

/// Side of the landblock square one table covers (§5.1).
pub const IMPOSTER_TABLE_SQUARE: u8 = 27;

/// DID prefix of imposter tables (§5.1).
pub const IMPOSTER_TABLE_PREFIX: u8 = 0x27;

/// Atlas columns are at most this many (§5.4).
pub const IMPOSTER_MAX_COLUMNS: u32 = 16;

/// Top-level atlas texels per cell (§5.4).
pub const IMPOSTER_CELL_TEXELS: u32 = 128;

/// The square of 27 × 27 landblocks one imposter table covers (§5.1).
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, PartialOrd, Ord)]
pub struct ImposterSquare {
    /// Region.
    pub region: u8,
    /// Landblock x of the lower corner (a multiple of 27 for tables of this install).
    pub x: u8,
    /// Landblock y of the lower corner.
    pub y: u8,
}

impl ImposterSquare {
    /// The square that contains landblock (`x`, `y`) of `region`: *X* = 27 × ⌊*x* / 27⌋,
    /// likewise for *Y*.
    #[must_use]
    pub fn containing(region: u8, x: u8, y: u8) -> Self {
        let snap = |v: u8| v - v % IMPOSTER_TABLE_SQUARE;
        Self {
            region,
            x: snap(x),
            y: snap(y),
        }
    }

    /// The table DID: 0x27000000 | region << 16 | x << 8 | y.
    #[must_use]
    pub fn did(self) -> Did {
        Did(u32::from_be_bytes([
            IMPOSTER_TABLE_PREFIX,
            self.region,
            self.x,
            self.y,
        ]))
    }

    /// The square a table DID names, or `None` for another prefix.
    #[must_use]
    pub fn from_did(did: Did) -> Option<Self> {
        let [p, region, x, y] = did.0.to_be_bytes();
        (p == IMPOSTER_TABLE_PREFIX).then_some(Self { region, x, y })
    }

    /// Whether `x` and `y` are multiples of 27 (MR5).
    #[must_use]
    pub fn is_aligned(self) -> bool {
        self.x.is_multiple_of(IMPOSTER_TABLE_SQUARE) && self.y.is_multiple_of(IMPOSTER_TABLE_SQUARE)
    }
}

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
    /// Stored TSize header of the entry table (the hint is kept for byte-exact writing).
    pub size: TSize,
    /// Entries in stored (bucket) order.
    pub entries: Vec<ImposterEntry>,
    /// Colour atlas (a texture list, 0x40; texture parameter 3000), or 0.
    pub colour_atlas: Did,
    /// Normal atlas (a texture list, 0x40; texture parameter 3011), or 0.
    pub second_atlas: Did,
    /// Width of one atlas cell in texture coordinates.
    pub cell_u: f32,
    /// Height of one atlas cell in texture coordinates.
    pub cell_v: f32,
}

/// Atlas columns and rows for `n` entries (§5.4): columns = the smallest power of two ≥
/// *n*, at most 16; rows = the smallest power of two ≥ ⌈*n* / 16⌉. `None` for *n* = 0.
#[must_use]
pub fn imposter_atlas_grid(n: u32) -> Option<(u32, u32)> {
    if n == 0 {
        return None;
    }
    let columns = n
        .checked_next_power_of_two()
        .map_or(IMPOSTER_MAX_COLUMNS, |c| c.min(IMPOSTER_MAX_COLUMNS));
    let rows = n
        .div_ceil(IMPOSTER_MAX_COLUMNS)
        .checked_next_power_of_two()?;
    Some((columns, rows))
}

fn len_u32(n: usize) -> u32 {
    u32::try_from(n).unwrap_or(u32::MAX)
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

    /// The square this table covers.
    #[must_use]
    pub fn square(&self) -> Option<ImposterSquare> {
        ImposterSquare::from_did(self.did)
    }

    /// The table DID of the square that holds landblock (`x`, `y`) of `region` (§5.1).
    #[must_use]
    pub fn did_for(region: u8, x: u8, y: u8) -> Did {
        ImposterSquare::containing(region, x, y).did()
    }

    /// The texture coordinates of `entry`'s cell (§5.4, I7): step = cell + cell / 31,
    /// *u*₀ = `column` × step_u, *u*₁ = *u*₀ + `cell_u`, likewise for *v*. `None` when a
    /// value is not finite.
    #[must_use]
    pub fn cell(&self, entry: &ImposterEntry) -> Option<AtlasCell> {
        self.cell_at(entry.column, entry.row)
    }

    /// The texture coordinates of cell (`column`, `row`) by the rule of
    /// [`ImposterTable::cell`] (I7); `None` when a value is not finite.
    #[must_use]
    pub fn cell_at(&self, column: u8, row: u8) -> Option<AtlasCell> {
        let step_u = self.cell_u + self.cell_u / 31.0;
        let step_v = self.cell_v + self.cell_v / 31.0;
        let u0 = f32::from(column) * step_u;
        let v0 = f32::from(row) * step_v;
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

    /// Preview rectangle (u0, v0, u1, v1) of cell (`column`, `row`) for tools: the cell of
    /// [`ImposterTable::cell_at`] (I7; for the stored cell size (31/32) / grid of every
    /// table in the data that is (*c* / *C*, *r* / *R*) extended by (`cell_u`, `cell_v`),
    /// §17.3). `None` for an empty table or a non-finite value.
    #[must_use]
    pub fn cell_uv(&self, column: u8, row: u8) -> Option<[f32; 4]> {
        self.grid()?;
        self.cell_at(column, row).map(|c| [c.u0, c.v0, c.u1, c.v1])
    }

    /// Columns and rows of the atlas ([`imposter_atlas_grid`]).
    #[must_use]
    pub fn grid(&self) -> Option<(u32, u32)> {
        imposter_atlas_grid(len_u32(self.entries.len()))
    }

    /// Expected top-level atlas size in texels: 128·*C* × 128·*R* (§5.4).
    #[must_use]
    pub fn atlas_texels(&self) -> Option<(u32, u32)> {
        let (c, r) = self.grid()?;
        Some((
            c.checked_mul(IMPOSTER_CELL_TEXELS)?,
            r.checked_mul(IMPOSTER_CELL_TEXELS)?,
        ))
    }

    /// The cell size the spec's geometry predicts: ((31/32) / *C*, (31/32) / *R*) (§5.4).
    #[must_use]
    #[allow(clippy::cast_precision_loss)]
    pub fn expected_cell_size(&self) -> Option<(f32, f32)> {
        let (c, r) = self.grid()?;
        let margin = 31.0_f32 / 32.0;
        Some((margin / c as f32, margin / r as f32))
    }

    /// Whether the stored keys follow the bucket order of their TSize hint (`records.md`
    /// §1.3.1); `None` for a hint without a known bucket count.
    #[must_use]
    pub fn keys_in_bucket_order(&self) -> Option<bool> {
        keys_in_bucket_order(self.size.hint, self.entries.iter().map(|e| e.name_key))
    }

    /// Same as [`ImposterTable::keys_in_bucket_order`] (the name the other record types of
    /// `misc-records.md` use).
    #[must_use]
    pub fn is_bucket_ordered(&self) -> Option<bool> {
        self.keys_in_bucket_order()
    }

    /// The texture lists the client loads with the table (the non-null atlases, §5.2).
    #[must_use]
    pub fn dependencies(&self) -> Vec<Did> {
        [self.colour_atlas, self.second_atlas]
            .into_iter()
            .filter(|d| !d.is_null())
            .collect()
    }

    /// Re-encodes the table (§5.2); byte-exact for a decoded table when the stored count
    /// is a shortest VLE (the count is written from the entry list; the hint is kept).
    #[must_use]
    pub fn to_bytes(&self) -> Vec<u8> {
        let mut out = Vec::with_capacity(
            24usize.saturating_add(self.entries.len().saturating_mul(IMPOSTER_ENTRY_SIZE)),
        );
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

    /// The first deviation from the observed invariant MR5 (§16), or `None`.
    #[must_use]
    #[allow(clippy::cast_precision_loss)]
    pub fn first_unusual_field(&self) -> Option<&'static str> {
        if !self.square().is_some_and(ImposterSquare::is_aligned) {
            return Some("square coordinates not multiples of 27 (MR5)");
        }
        if self.size.hint != 1 {
            return Some("bucket hint not 1 (MR5)");
        }
        if self.is_bucket_ordered() == Some(false) {
            return Some("entries not in bucket order (MR5)");
        }
        let cells_ok = self
            .entries
            .iter()
            .enumerate()
            .all(|(i, e)| usize::from(e.column) == i % 16 && usize::from(e.row) == i / 16);
        if !cells_ok {
            return Some("entry cell is not (i mod 16, i / 16) (MR5)");
        }
        if let Some((c, r)) = self.grid() {
            if self.colour_atlas.is_null() || self.second_atlas.is_null() {
                return Some("non-empty table without both atlases (MR5)");
            }
            let margin = 31.0_f32 / 32.0;
            let close = |a: f32, b: f32| (a - b).abs() <= 1.0e-6;
            if !close(self.cell_u * c as f32, margin) || !close(self.cell_v * r as f32, margin) {
                return Some("cell size is not (31/32) / grid (MR5)");
            }
        }
        None
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
        let at = r.position();
        let size = r.tsize()?;
        if size.hint > MAX_TABLE_HINT {
            return Err(FormatError::Unsupported {
                what: "imposter table bucket hint",
                offset: at,
                value: u32::from(size.hint),
            });
        }
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
/// When the payload is not a complete imposter table (§5.2) or its bucket hint is above
/// [`MAX_TABLE_HINT`] (MR12).
pub fn decode_imposter_table(data: &[u8]) -> Result<ImposterTable> {
    ImposterTable::parse(data)
}
