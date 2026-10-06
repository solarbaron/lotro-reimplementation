// SPDX-License-Identifier: GPL-3.0-or-later
//! Landblock records (DID prefix 0x80, `0x80KRXXYY`) and the per-region tables they use.
//!
//! | Kind `K` | Record | Archive | Type | Spec |
//! |---|---|---|---|---|
//! | 0 | landblock data: heights, vertex flags, surface layers, collision trees | `client_cell_R` | [`LandblockData`] | `landblock.md` §3 |
//! | 1 | block map: (key, palette index) pairs | `client_map_R` | [`BlockMap`] | §5.3 |
//! | 2 | landblock info: cells, static entities, links, weenies | `client_cell_R` | [`LandblockInfo`] | §4, `entity-descriptor.md` |
//! | 4 | landblock properties: minimap tile, light maps | `client_cell_R` | [`LandblockProperties`] | §6 |
//! | 5 | distant-landscape sector, one per 3 × 3 group (framing; water pairs searchable) | `client_cell_R` | [`LandblockGroupRecord`] ([`DistantLandscapeSector`]) | §7, `water.md` §9 |
//!
//! Related per-region records: the block-map descriptor (prefix 0x18,
//! [`BlockMapDescriptor`], §5.2) and the region descriptor (prefix 0x0F,
//! [`RegionDescriptor`], `environment.md` §3.1–§3.2) that names it and the region's other
//! seven tables ([`RegionTable`]).
//!
//! # Terrain and ceiling grids
//!
//! A landblock's terrain and (in cave blocks) ceiling are 33 × 33 grids meshed as two
//! triangles per 5 × 5 quad. The terrain quad's diagonal is bit 7 of the vertex-flag byte at
//! its south-west corner and its triangles are counter-clockwise from above; the ceiling
//! quad's diagonal is **bit 0** and its triangles are clockwise from above (§3.5, §3.6).
//! [`GridSurface`] selects the surface in the per-quad queries
//! ([`LandblockData::surface_quad_triangles`], [`LandblockData::grid_triangle_positions`],
//! [`LandblockData::terrain_height`], [`LandblockData::ceiling_height`]), and
//! [`grid_triangle_key`] gives the collision-tree triangle numbering (`collision.md` §6.2).
//!
//! # Directory stamps
//!
//! Two layout details depend on the record's directory-entry `stamp`
//! (`dat-container.md` §5.2), so the landblock-data and landblock-info decoders take it as
//! an input next to the payload (`Option<u16>`):
//!
//! - a collision tree's trailing `build_byte` exists iff `stamp` ≥ 38 (§3.9);
//! - landblock info ends with a trailer `u32` iff `stamp` ≥ 31 (§4.9).
//!
//! Without a stamp, or with a stamp the rules were never observed for (30 for the trailer,
//! 34–37 for the tree byte), the decoders use the spec's fallbacks and report
//! [`LayoutRule::Fallback`] / [`LayoutRule::UnobservedStamp`] in the decoded record, which
//! [`FailureCategory::UnknownStampRule`] census code can count.
//!
//! # Coordinates
//!
//! Landblock-local x, y run from the south-west corner (0–160 units); z is absolute.
//! Heights are stored as u16 with **height = stored × 2,000 / 65,535**, evaluated in f32 as
//! the original client does ([`height_from_raw`], `landblock.md` §3.4).
//! The height grid is X-major (sample (i, j) at index 33·i + j, i along X), while the
//! vertex numbers of surface-layer triangles are **Y-major** (33·y + x); see
//! [`vertex_to_height_index`]. Rotations are (w, x, y, z) unit quaternions applied as active
//! rotations ([`crate::setup::Quaternion`], `coordinates.md` §11).
//!
//! # Limits
//!
//! Every count is checked against the remaining bytes before anything is allocated, and the
//! static-entity count is additionally capped at [`MAX_STATIC_ENTITIES`]. Decoded data is
//! charged against the [`crate::Reader`] memory budget like every other decoder of this
//! crate.

mod block_map;
mod data;
mod entity;
mod info;
mod properties;
mod region;

pub use block_map::{
    BLOCK_MAP_DESCRIPTOR_PREFIX, BlockMap, BlockMapDescriptor, BlockMapEntry, BlockMapProperty,
    PaletteValue, ResolvedBlockMapValue,
};
pub use data::{
    Ceiling, CollisionTree, GridSurface, HeightGrid, LandblockBox, LandblockData, LayerKind,
    SurfaceChunk, SurfaceLayer, VertexFlags, ceiling_quad_triangles, grid_triangle_key,
    grid_triangle_quad, quad_triangles, surface_quad_triangles,
};
pub use entity::{
    InstanceId, MAX_PLACEMENT_COORD, MAX_SCALE, MAX_STATIC_ENTITIES, MIN_SCALE, Placement,
    SED_FLAGS_KNOWN, StaticEntity, StaticEntityType,
};
pub use info::{
    Cell, CellNeighbour, CrossLink, HeaderIndex, LandblockInfo, Link, LinkCoda, TSizeList, Weenie,
};
pub use properties::{
    DistantLandscapeSector, GROUP_SAMPLE_COUNT, GroupSample, LandblockGroupRecord, LandblockPids,
    LandblockProperties, LightMaps, MinimapTile, MinimapVariant, SECTOR_SIDE, SectorWaterPair,
    light_map_uv,
};
pub use region::{
    REGION_DESCRIPTOR_LEN, REGION_DESCRIPTOR_PREFIX, REGION_DID_MAP_CAPACITY, REGION_DID_MAPS,
    RegionDescriptor, RegionDidMap, RegionDidMapEntry, RegionTable,
};

use crate::did::{Did, LandblockId, LandblockKind};
use crate::error::FormatError;

/// Side length of a landblock in world units.
pub const LANDBLOCK_SIZE: f32 = 160.0;
/// Samples per side of the height grid (33).
pub const GRID_SIDE: usize = 33;
/// Samples in the height grid (33 × 33 = 1,089).
pub const GRID_SAMPLES: usize = GRID_SIDE * GRID_SIDE;
/// Spacing of height samples in world units.
pub const GRID_SPACING: f32 = 5.0;
/// Quads per side of the height grid (32).
pub const QUAD_SIDE: usize = GRID_SIDE - 1;
/// Quads in the height grid (1,024), and the length of a surface chunk's quad grid.
pub const QUAD_COUNT: usize = QUAD_SIDE * QUAD_SIDE;
/// Triangles of a terrain or ceiling grid (2 per quad, 2,048), the key range of their
/// collision trees (`collision.md` §6.2).
pub const GRID_TRIANGLE_COUNT: usize = 2 * QUAD_COUNT;
/// Entries per side of the coarse grid (17).
pub const COARSE_SIDE: usize = 17;
/// Entries in the coarse grid (17 × 17 = 289).
pub const COARSE_SAMPLES: usize = COARSE_SIDE * COARSE_SIDE;
/// Height in world units of the largest stored sample, 65,535 (`landblock.md` §3.4).
pub const HEIGHT_RANGE: f32 = 2000.0;
/// The f32 nearest 1 / 65,535 (1.5259022 × 10⁻⁵), the second factor of the height rule
/// (`landblock.md` §3.4). IEEE division is correctly rounded, so this is that nearest value.
pub const HEIGHT_INV_MAX_RAW: f32 = 1.0 / 65_535.0;
/// Stored height steps per world unit, 65,535 / 2,000 (`landblock.md` §3.4), as the nearest
/// f32. For continuous (interpolated) conversions only: one divisor cannot reproduce the
/// rule's two f32 roundings, so [`height_from_raw`] is the bit-exact conversion of a stored
/// sample. (The superseded rule of the spec divided by 32.768.)
pub const HEIGHT_STEPS_PER_UNIT: f32 = 32.7675;
/// Largest landblock X or Y observed (0–254; `coordinates.md` §4).
pub const MAX_LANDBLOCK_COORD: u8 = 254;
/// Offset from a landblock-data DID to the landblock-info DID of the same landblock.
pub const INFO_DID_OFFSET: u32 = 0x0020_0000;

/// Converts a stored height sample to world units (`landblock.md` §3.4): the sample as an
/// f32, times 2,000.0 rounded to f32, times the f32 nearest 1 / 65,535 rounded to f32, the
/// order the original client uses. Stored 65,535 is exactly 2,000.0.
#[must_use]
pub fn height_from_raw(raw: u16) -> f32 {
    let scaled = f32::from(raw) * HEIGHT_RANGE;
    scaled * HEIGHT_INV_MAX_RAW
}

/// Height-grid index (33·i + j, i along X) of surface-layer vertex number `v`, which is
/// Y-major (v = 33·y + x; `landblock.md` §3.8.3). `None` for `v` ≥ 1,089.
#[must_use]
pub fn vertex_to_height_index(v: u16) -> Option<usize> {
    let v = usize::from(v);
    (v < GRID_SAMPLES).then(|| GRID_SIDE * (v % GRID_SIDE) + v / GRID_SIDE)
}

/// Grid coordinates (x index, y index) of surface-layer vertex number `v` (Y-major).
/// `None` for `v` ≥ 1,089.
#[must_use]
pub fn vertex_xy(v: u16) -> Option<(usize, usize)> {
    let v = usize::from(v);
    (v < GRID_SAMPLES).then_some((v % GRID_SIDE, v / GRID_SIDE))
}

/// Region-global position (G_x, G_y, z) of a block-local position in landblock (X, Y):
/// (160·X + x, 160·Y + y, z) in f64, since region coordinates reach about 40,800 units
/// where f32 keeps only millimetres (`coordinates.md` §2, §12).
#[must_use]
pub fn region_position(landblock_x: u8, landblock_y: u8, local: [f32; 3]) -> [f64; 3] {
    let size = f64::from(LANDBLOCK_SIZE);
    [
        size * f64::from(landblock_x) + f64::from(local[0]),
        size * f64::from(landblock_y) + f64::from(local[1]),
        f64::from(local[2]),
    ]
}

/// Builds the DID `0x80KRXXYY` of landblock record kind `kind` for (R, X, Y).
#[must_use]
pub fn landblock_did(kind: LandblockKind, region: u8, x: u8, y: u8) -> Did {
    LandblockId { kind, region, x, y }.to_did()
}

/// How a stamp-dependent layout detail was decided (module docs).
///
/// A record with no stamp-dependent field (landblock data without a collision tree) reports
/// [`LayoutRule::Stamp`] when a stamp was given, whatever its value, and
/// [`LayoutRule::Fallback`] when none was: the rule then says whether a stamp was supplied,
/// and a census of stampless decodes counts every record.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Default)]
pub enum LayoutRule {
    /// A stamp was given, and either it lies in a range the rule was observed for or the
    /// record has no field that depends on it.
    #[default]
    Stamp,
    /// No stamp was given: the spec's fallback decided (or nothing needed deciding).
    Fallback,
    /// By the fallback, because the stamp lies in a range the rule was never observed for
    /// (`landblock.md` §9 `UnknownStampRule`).
    UnobservedStamp,
}

/// Decision for a collision tree's `build_byte`: `Some(present)` from the stamp, or `None`
/// to use the fallback (`landblock.md` §3.9, `mesh.md` §10.4).
pub(crate) fn tree_build_byte_rule(stamp: Option<u16>) -> (Option<bool>, LayoutRule) {
    match stamp {
        None => (None, LayoutRule::Fallback),
        Some(s) if s >= 38 => (Some(true), LayoutRule::Stamp),
        Some(s) if s <= 33 => (Some(false), LayoutRule::Stamp),
        Some(_) => (None, LayoutRule::UnobservedStamp),
    }
}

/// Decision for the landblock-info trailer: `Some(present)` from the stamp, or `None` to use
/// the fallback (`landblock.md` §4.9).
pub(crate) fn info_trailer_rule(stamp: Option<u16>) -> (Option<bool>, LayoutRule) {
    match stamp {
        None => (None, LayoutRule::Fallback),
        Some(s) if s >= 31 => (Some(true), LayoutRule::Stamp),
        Some(s) if s <= 29 => (Some(false), LayoutRule::Stamp),
        Some(_) => (None, LayoutRule::UnobservedStamp),
    }
}

/// The failure categories of `landblock.md` §9 (and `entity-descriptor.md` §12), for
/// censuses that compare installs. See [`FailureCategory::of`].
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, PartialOrd, Ord)]
pub enum FailureCategory {
    /// The payload ended inside a field.
    Truncated,
    /// Bytes were left after the layout.
    TrailingBytes,
    /// A grid count ≠ 1,089 / 289, or a quad count ∉ {0, 1,024}.
    BadCount,
    /// A `has_*`, `whole` or `cross` byte not 0/1.
    BadFlag,
    /// A layer window outside the index list, or a vertex ≥ 1,089.
    IndexOutOfRange,
    /// Own DID or header info DID inconsistent, or a DID of the wrong record type.
    DidMismatch,
    /// A stamp in a range the layout rules were never observed for. Decoders fall back and
    /// report this through [`LayoutRule::UnobservedStamp`], not as an error.
    UnknownStampRule,
    /// An embedded property set failed (`property-system.md` §5.3).
    PropertySet,
    /// An unsupported flag bit (static entity, cell or landblock-info flags).
    UnsupportedFlags,
    /// A count above a decoder limit, or the memory budget was exhausted.
    Limit,
    /// A non-finite or zero-length placement transform, or any other malformed field.
    Malformed,
}

impl FailureCategory {
    /// Classifies a decoder error by its structure (never by message text):
    ///
    /// - a resource cap ([`FormatError::MemoryLimit`], [`FormatError::DepthLimit`],
    ///   [`FormatError::CountLimit`]) is [`FailureCategory::Limit`] wherever it occurred;
    /// - any other failure inside an embedded property set ([`FormatError::InPropertySet`],
    ///   [`FormatError::PropertyValue`], [`FormatError::UnknownProperty`]) is
    ///   [`FailureCategory::PropertySet`], whether the set was truncated, malformed or named
    ///   an unknown PID;
    /// - everything else is classified by its innermost error.
    #[must_use]
    pub fn of(e: &FormatError) -> Self {
        let mut in_set = false;
        let mut cur = e;
        loop {
            match cur {
                FormatError::InPropertySet { source, .. }
                | FormatError::PropertyValue { source, .. } => {
                    in_set = true;
                    cur = source;
                }
                FormatError::InRecord { source, .. } => cur = source,
                _ => break,
            }
        }
        match cur {
            FormatError::MemoryLimit { .. }
            | FormatError::DepthLimit { .. }
            | FormatError::CountLimit { .. } => Self::Limit,
            FormatError::UnknownProperty { .. } => Self::PropertySet,
            _ if in_set => Self::PropertySet,
            FormatError::Truncated { .. } => Self::Truncated,
            FormatError::TrailingBytes { .. } => Self::TrailingBytes,
            FormatError::CountMismatch { .. } => Self::BadCount,
            FormatError::InvalidBool { .. } => Self::BadFlag,
            FormatError::IndexOutOfRange { .. } => Self::IndexOutOfRange,
            FormatError::DidMismatch { .. } | FormatError::WrongRecordType { .. } => {
                Self::DidMismatch
            }
            FormatError::Unsupported { .. } => Self::UnsupportedFlags,
            _ => Self::Malformed,
        }
    }

    /// Short name as used in `landblock.md` §9.
    #[must_use]
    pub fn name(self) -> &'static str {
        match self {
            Self::Truncated => "Truncated",
            Self::TrailingBytes => "TrailingBytes",
            Self::BadCount => "BadCount",
            Self::BadFlag => "BadFlag",
            Self::IndexOutOfRange => "IndexOutOfRange",
            Self::DidMismatch => "DidMismatch",
            Self::UnknownStampRule => "UnknownStampRule",
            Self::PropertySet => "PropertySet",
            Self::UnsupportedFlags => "UnsupportedFlags",
            Self::Limit => "Limit",
            Self::Malformed => "Malformed",
        }
    }
}

/// Reads a landblock-family DID and checks its kind.
pub(crate) fn read_landblock_did(
    r: &mut crate::reader::Reader<'_>,
    kind: LandblockKind,
    what: &'static str,
) -> crate::error::Result<Did> {
    let did = r.did()?;
    match LandblockId::from_did(did) {
        Some(id) if id.kind == kind => Ok(did),
        _ => Err(FormatError::WrongRecordType {
            did: did.0,
            expected: what,
        }),
    }
}

/// Reads a property set embedded in a landblock-family record, wrapping any failure in
/// [`FormatError::InPropertySet`] so that [`FailureCategory::of`] classifies it as
/// [`FailureCategory::PropertySet`] (or `Limit` for a resource cap) however it failed.
pub(crate) fn read_embedded_set<S: crate::property::PropertySchema + ?Sized>(
    r: &mut crate::reader::Reader<'_>,
    schema: &S,
) -> crate::error::Result<crate::property_set::PropertySet> {
    let offset = r.position();
    crate::property_set::PropertySet::read(r, schema).map_err(|e| FormatError::InPropertySet {
        offset,
        source: Box::new(e),
    })
}

/// Little-endian writers shared by the encoders of this module.
pub(crate) mod put {
    pub(crate) fn u16(out: &mut Vec<u8>, v: u16) {
        out.extend_from_slice(&v.to_le_bytes());
    }
    pub(crate) fn u32(out: &mut Vec<u8>, v: u32) {
        out.extend_from_slice(&v.to_le_bytes());
    }
    pub(crate) fn f32s(out: &mut Vec<u8>, v: &[f32]) {
        for x in v {
            out.extend_from_slice(&x.to_le_bytes());
        }
    }
    pub(crate) fn len_u32(len: usize) -> u32 {
        u32::try_from(len).unwrap_or(u32::MAX)
    }
}

#[cfg(test)]
mod tests;
