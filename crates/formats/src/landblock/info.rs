// SPDX-License-Identifier: GPL-3.0-or-later
//! Landblock info `0x802RXXYY`: cross links, header indices, cells, static entities, links,
//! the landblock property set and weenies.
//!
//! Spec: `docs/specs/landblock.md` §4; the static entities follow
//! `docs/specs/entity-descriptor.md`.

use std::collections::HashMap;

use super::entity::{InstanceId, MAX_STATIC_ENTITIES, StaticEntity};
use super::{LayoutRule, info_trailer_rule, read_embedded_set, read_landblock_did};
use crate::did::{Did, LandblockId, LandblockKind};
use crate::error::{FormatError, Result};
use crate::property::{Position, PropertySchema};
use crate::property_set::PropertySet;
use crate::reader::{Reader, TSize};
use crate::setup::Quaternion;

/// Landblock-info flag: cells present (set in every observed record).
const FLAG_CELLS: u32 = 0x1;
/// Landblock-info flag: cross links present.
const FLAG_CROSS_LINKS: u32 = 0x2;
/// Landblock-info flag: header indices present.
const FLAG_HEADER_INDICES: u32 = 0x4;
/// Every landblock-info flag bit with a known meaning.
const INFO_FLAGS_KNOWN: u32 = FLAG_CELLS | FLAG_CROSS_LINKS | FLAG_HEADER_INDICES;

/// Cell flag bits observed (0x04 always; 0x08, 0x10, 0x20 sometimes). Other bits are
/// rejected: their payloads are unknown.
const CELL_FLAGS_KNOWN: u32 = 0x3C;
/// Cell flag: property set present (always set).
const CELL_PROPERTIES: u32 = 0x04;
/// Cell flag: `index_echo` and `extra` present.
const CELL_ECHO: u32 = 0x20;

/// Smallest stored sizes, for bounding counts by the remaining bytes (§10).
const CROSS_LINK_SIZE: usize = 24;
const NEIGHBOUR_SIZE: usize = 15;
/// u16 index + u32 count.
const HEADER_INDEX_MIN: usize = 6;
/// u16 index + position flag byte + u32 flags + DID + u32 count + two u32 list counts.
const CELL_MIN: usize = 2 + 1 + 4 + 4 + 4 + 4 + 4;
/// Pascal string length byte + 3 × u64 + u32 + TSize (2) + u8 cross.
const LINK_MIN: usize = 1 + 24 + 4 + 2 + 1;
/// u64 + TSize (2).
const WEENIE_MIN: usize = 8 + 2;
/// SED flags word.
const SED_MIN: usize = 4;

/// A TSize-counted collection: the stored header (kept so the bucket hint survives) and the
/// decoded elements.
#[derive(Debug, Clone, PartialEq, Default)]
pub struct TSizeList<T> {
    /// Stored header.
    pub size: TSize,
    /// Elements in stored order.
    pub items: Vec<T>,
}

impl<T> TSizeList<T> {
    /// Number of elements.
    #[must_use]
    pub fn len(&self) -> usize {
        self.items.len()
    }

    /// Whether there are no elements.
    #[must_use]
    pub fn is_empty(&self) -> bool {
        self.items.is_empty()
    }

    /// Iterates the elements.
    pub fn iter(&self) -> std::slice::Iter<'_, T> {
        self.items.iter()
    }

    fn read(
        r: &mut Reader<'_>,
        min_size: usize,
        mut f: impl FnMut(&mut Reader<'_>) -> Result<T>,
    ) -> Result<Self> {
        let size = r.tsize()?;
        let mut items = r.alloc_vec(size.count, min_size, usize::MAX)?;
        for _ in 0..size.count {
            let item = f(r)?;
            r.push(&mut items, item, size.count)?;
        }
        Ok(Self { size, items })
    }
}

impl<'t, T> IntoIterator for &'t TSizeList<T> {
    type Item = &'t T;
    type IntoIter = std::slice::Iter<'t, T>;

    fn into_iter(self) -> Self::IntoIter {
        self.items.iter()
    }
}

/// A cross link (24 bytes; §4.2).
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Default)]
pub struct CrossLink {
    /// Source index.
    pub from_index: u32,
    /// Source landblock-info DID.
    pub from_info: Did,
    /// Target landblock-info DID.
    pub to_info: Did,
    /// Link type (same code space as [`Link::link_type`]).
    pub link_type: u32,
    /// Target index.
    pub to_index: u32,
    /// Meaning OPEN.
    pub mask: u32,
}

/// A header-index entry (§4.3; meaning OPEN, probably a grouping over cells).
#[derive(Debug, Clone, PartialEq, Eq, Default)]
pub struct HeaderIndex {
    /// Leading index (usually a cell index).
    pub index: u16,
    /// Listed values (cell indices of the same record).
    pub values: Vec<u16>,
}

/// One neighbour entry of a cell (15 bytes; §4.4).
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Default)]
pub struct CellNeighbour {
    /// Ordinal; equals the entry's position in the list.
    pub ordinal: u32,
    /// Neighbour cell index (a cell of the same record).
    pub cell: u16,
    /// Back-reference (meaning OPEN).
    pub back_reference: u16,
    /// Three u16, 0 in every observed entry.
    pub reserved: [u16; 3],
    /// Flag byte (1 or 0 observed; meaning OPEN).
    pub flag: u8,
}

/// An interior cell (room, cave, dungeon piece) of a landblock (§4.4).
#[derive(Debug, Clone, PartialEq, Default)]
pub struct Cell {
    /// Cell number.
    pub index: u16,
    /// Position of the cell in the landblock frame (not confined to 0–160).
    ///
    /// Region, landblock and cell are present in every observed cell, but the x, y, z
    /// offset is **not**: two observed cells (flag byte 0x4B) carry none, so
    /// [`Position::offset`] may be `None` and a loader must handle that rather than unwrap
    /// it or default to the landblock origin (`landblock.md` §4.4, OPEN). Use
    /// [`Cell::rotation`] for the orientation.
    pub position: Position,
    /// Cell flags (bits 2–5 only; meaning of 3, 4, 5 OPEN).
    pub flags: u32,
    /// The cell geometry, a 0x01 record (`collision.md` §5.1; decoded by
    /// [`CellGeometry::parse`](crate::collision::CellGeometry::parse)).
    pub geometry: Did,
    /// Neighbour cells.
    pub neighbours: Vec<CellNeighbour>,
    /// Flag 0x20: echo of `index`.
    pub index_echo: Option<u16>,
    /// List A (cell indices of this record; meaning OPEN).
    pub list_a: Vec<u16>,
    /// List B (meaning OPEN).
    pub list_b: Vec<u16>,
    /// Flag 0x20: a u16 (1–11 or 0xFFFF observed; meaning OPEN).
    pub extra: Option<u16>,
    /// Flag 0x04: the cell's property set (scale, interior fog and light, ...).
    pub properties: Option<PropertySet>,
}

impl Cell {
    /// The cell's orientation as a typed (w, x, y, z) rotation (`coordinates.md` §11.1,
    /// active from cell space into the landblock frame), or `None` when the position has
    /// no orientation (flag 0x20 clear). See [`Position::rotation`].
    #[must_use]
    pub fn rotation(&self) -> Option<Quaternion> {
        self.position.rotation()
    }
}

/// The coda of a cross-landblock link (§4.6).
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Default)]
pub struct LinkCoda {
    /// First index.
    pub from_index: u32,
    /// First landblock-info DID.
    pub from_info: Did,
    /// Second landblock-info DID.
    pub to_info: Did,
    /// Second index.
    pub to_index: u32,
}

/// A link between entities (paths, patrols, generators, scripts, ...; §4.6). Its IIDs use
/// the same structured form as [`StaticEntity::iid`], so they join directly with
/// [`LandblockInfo::entities_by_iid`].
#[derive(Debug, Clone, PartialEq, Default)]
pub struct Link {
    /// Label (often empty).
    pub name: String,
    /// The link's own instance ID.
    pub iid: InstanceId,
    /// Target entity.
    pub to_iid: InstanceId,
    /// Source entity.
    pub from_iid: InstanceId,
    /// Link type code (§4.6 table).
    pub link_type: u32,
    /// The link's property set.
    pub properties: PropertySet,
    /// Present when the link crosses landblocks (`cross` = 1).
    pub cross: Option<LinkCoda>,
}

/// A server-side object template (spawn generators and the like; §4.8).
#[derive(Debug, Clone, PartialEq, Default)]
pub struct Weenie {
    /// Instance ID (structured like [`StaticEntity::iid`]).
    pub iid: InstanceId,
    /// Its property set.
    pub properties: PropertySet,
}

/// A decoded landblock-info record `0x802RXXYY` (§4.1), consumed to the last byte.
#[derive(Debug, Clone, PartialEq)]
pub struct LandblockInfo {
    /// Own DID.
    pub did: Did,
    /// Flags: bit 0 cells, bit 1 cross links, bit 2 header indices.
    pub flags: u32,
    /// Meaning OPEN; ignore.
    pub size_hint: u32,
    /// Cross links (flag bit 1).
    pub cross_links: Option<TSizeList<CrossLink>>,
    /// Header indices (flag bit 2).
    pub header_indices: Option<TSizeList<HeaderIndex>>,
    /// Cells (flag bit 0).
    pub cells: Option<TSizeList<Cell>>,
    /// u32 after the cells; 0 in every observed record.
    pub reserved_a: u32,
    /// Static entity descriptors (`entity-descriptor.md`).
    pub entities: Vec<StaticEntity>,
    /// Links.
    pub links: Vec<Link>,
    /// The landblock's own property set (dungeon and interior settings; usually empty).
    pub properties: PropertySet,
    /// u32 after the property set; 0 in every observed record.
    pub reserved_b: u32,
    /// Weenies.
    pub weenies: TSizeList<Weenie>,
    /// Final u32 of records with stamp ≥ 31 (meaning OPEN).
    pub trailer: Option<u32>,
    /// How the trailer's presence was decided.
    pub layout_rule: LayoutRule,
}

impl LandblockInfo {
    /// Human-readable record type name, used in errors.
    pub const NAME: &'static str = "landblock info";

    /// Decodes a complete payload with the record's directory `stamp` (`None` uses the
    /// §4.9 fallback) and `schema` (normally the property dictionary) for the embedded
    /// property sets. The payload must be consumed exactly.
    pub fn parse<S: PropertySchema + ?Sized>(
        data: &[u8],
        stamp: Option<u16>,
        schema: &S,
    ) -> Result<Self> {
        let mut r = Reader::new(data);
        let rec = Self::read(&mut r, stamp, schema)?;
        r.expect_end(Self::NAME)?;
        r.log_unknown_hints(Self::NAME);
        Ok(rec)
    }

    /// [`LandblockInfo::parse`], also checking the record's DID against the one looked up.
    pub fn parse_expecting<S: PropertySchema + ?Sized>(
        data: &[u8],
        expected: impl Into<Did>,
        stamp: Option<u16>,
        schema: &S,
    ) -> Result<Self> {
        let expected = expected.into();
        let rec = Self::parse(data, stamp, schema)?;
        if rec.did != expected {
            return Err(FormatError::DidMismatch {
                expected: expected.0,
                found: rec.did.0,
            });
        }
        Ok(rec)
    }

    fn read<S: PropertySchema + ?Sized>(
        r: &mut Reader<'_>,
        stamp: Option<u16>,
        schema: &S,
    ) -> Result<Self> {
        let did = read_landblock_did(r, LandblockKind::Info, Self::NAME)?;
        let flags_at = r.position();
        let flags = r.u32()?;
        if flags & !INFO_FLAGS_KNOWN != 0 {
            return Err(FormatError::Unsupported {
                what: "landblock-info flags",
                offset: flags_at,
                value: flags,
            });
        }
        let size_hint = r.u32()?;
        let cross_links = if flags & FLAG_CROSS_LINKS != 0 {
            Some(TSizeList::read(r, CROSS_LINK_SIZE, read_cross_link)?)
        } else {
            None
        };
        let header_indices = if flags & FLAG_HEADER_INDICES != 0 {
            Some(TSizeList::read(r, HEADER_INDEX_MIN, |r| {
                let index = r.u16()?;
                let values = read_u16_list(r)?;
                Ok(HeaderIndex { index, values })
            })?)
        } else {
            None
        };
        let cells = if flags & FLAG_CELLS != 0 {
            Some(TSizeList::read(r, CELL_MIN, |r| read_cell(r, schema))?)
        } else {
            None
        };
        let reserved_a = r.u32()?;

        let count_at = r.position();
        let count = r.u32()?;
        if count > MAX_STATIC_ENTITIES {
            return Err(FormatError::CountLimit {
                what: "static entity count",
                offset: count_at,
                count: u64::from(count),
                limit: u64::from(MAX_STATIC_ENTITIES),
            });
        }
        let mut entities = r.alloc_vec(count, SED_MIN, usize::MAX)?;
        let mut renormalised = 0u32;
        for _ in 0..count {
            let e = StaticEntity::read(r, schema)?;
            if e.placement.is_some_and(|p| p.renormalised) {
                renormalised += 1;
            }
            r.push(&mut entities, e, count)?;
        }
        if renormalised != 0 {
            // One event per record; each entity carries its own `Placement::renormalised`.
            tracing::debug!(%did, count = renormalised, "static entity rotations renormalised");
        }

        let count = r.u32()?;
        let mut links = r.alloc_vec(count, LINK_MIN, usize::MAX)?;
        for _ in 0..count {
            let l = read_link(r, schema)?;
            r.push(&mut links, l, count)?;
        }

        let properties = read_embedded_set(r, schema)?;
        let reserved_b = r.u32()?;
        let weenies = TSizeList::read(r, WEENIE_MIN, |r| {
            Ok(Weenie {
                iid: InstanceId(r.u64()?),
                properties: read_embedded_set(r, schema)?,
            })
        })?;

        let (rule, layout_rule) = info_trailer_rule(stamp);
        let present = rule.unwrap_or(r.remaining() == 4);
        let trailer = if present { Some(r.u32()?) } else { None };
        Ok(Self {
            did,
            flags,
            size_hint,
            cross_links,
            header_indices,
            cells,
            reserved_a,
            entities,
            links,
            properties,
            reserved_b,
            weenies,
            trailer,
            layout_rule,
        })
    }

    /// The landblock this record describes.
    #[must_use]
    pub fn landblock(&self) -> Option<LandblockId> {
        LandblockId::from_did(self.did)
    }

    /// The cells, or an empty slice.
    #[must_use]
    pub fn cells(&self) -> &[Cell] {
        self.cells.as_ref().map_or(&[], |c| &c.items)
    }

    /// The cell with number `index` (the first, if several share it). A linear search:
    /// to look up many cells, build a map from [`LandblockInfo::cells`] once.
    #[must_use]
    pub fn cell(&self, index: u16) -> Option<&Cell> {
        self.cells().iter().find(|c| c.index == index)
    }

    /// Positions in [`LandblockInfo::entities`] of the static entities with each IID, in
    /// stored order. A list can hold more than one entity: IIDs are not guaranteed unique
    /// (§4.2). Entities without an IID are left out.
    #[must_use]
    pub fn entities_by_iid(&self) -> HashMap<InstanceId, Vec<usize>> {
        let mut map: HashMap<InstanceId, Vec<usize>> = HashMap::new();
        for (i, e) in self.entities.iter().enumerate() {
            if let Some(iid) = e.iid {
                map.entry(iid).or_default().push(i);
            }
        }
        map
    }

    /// The first deviation from invariants LI2–LI4 (§4.10) and ED1/ED3/ED5
    /// (`entity-descriptor.md` §11) that can be checked within the record, or `None`. Such
    /// records still decode.
    #[must_use]
    pub fn first_unusual_field(&self) -> Option<&'static str> {
        if self.layout_rule == LayoutRule::UnobservedStamp {
            return Some("trailer decided by fallback for an unobserved stamp");
        }
        if self.flags & FLAG_CELLS == 0 {
            return Some("cells flag (bit 0) clear");
        }
        if self.reserved_a != 0 || self.reserved_b != 0 {
            return Some("reserved word not 0 (LI2)");
        }
        let lb = self.landblock();
        let cells = self.cells();
        // Cell indices are u16: a 65,536-bit set makes each membership test O(1), so the
        // check stays linear in the record size (a per-test scan would be quadratic).
        let mut present = vec![0u64; 1 << 10];
        for c in cells {
            present[usize::from(c.index >> 6)] |= 1 << (c.index & 63);
        }
        let is_cell = |i: u16| present[usize::from(i >> 6)] & (1 << (i & 63)) != 0;
        for c in cells {
            let here = lb.map(|l| (l.region, (l.x, l.y)));
            if c.position
                .landblock()
                .is_some_and(|p| Some(p) != here.map(|h| h.1))
                || c.position
                    .region()
                    .is_some_and(|r| Some(r) != here.map(|h| h.0))
            {
                return Some("cell position names another landblock (LI3)");
            }
            if c.position.cell().is_some_and(|i| i != c.index) {
                return Some("cell position names another cell (LI3)");
            }
            for (k, n) in c.neighbours.iter().enumerate() {
                if n.ordinal as usize != k {
                    return Some("neighbour ordinal differs from its position (LI3)");
                }
                if !is_cell(n.cell) {
                    return Some("neighbour is not a cell of this record (LI3)");
                }
                if n.reserved != [0; 3] {
                    return Some("neighbour reserved u16 not 0 (LI3)");
                }
            }
            if c.index_echo.is_some_and(|e| e != c.index) {
                return Some("cell index echo differs (LI3)");
            }
            if c.list_a.iter().any(|&i| !is_cell(i)) {
                return Some("cell list A names a non-cell (LI3)");
            }
            if c.geometry.prefix() != 0x01 {
                return Some("cell geometry is not a 0x01 DID (LI4)");
            }
        }
        for e in &self.entities {
            if let Some(why) = e.first_unusual_field() {
                return Some(why);
            }
            if let (Some(iid), Some(l)) = (e.iid, lb)
                && (iid.region() != l.region || iid.landblock() != (l.x, l.y))
            {
                return Some("IID names another landblock (ED3)");
            }
        }
        None
    }
}

fn read_cross_link(r: &mut Reader<'_>) -> Result<CrossLink> {
    Ok(CrossLink {
        from_index: r.u32()?,
        from_info: r.did()?,
        to_info: r.did()?,
        link_type: r.u32()?,
        to_index: r.u32()?,
        mask: r.u32()?,
    })
}

/// A u32 count followed by that many u16.
fn read_u16_list(r: &mut Reader<'_>) -> Result<Vec<u16>> {
    let count = r.u32()?;
    let mut out = r.alloc_vec(count, 2, usize::MAX)?;
    for _ in 0..count {
        let v = r.u16()?;
        r.push(&mut out, v, count)?;
    }
    Ok(out)
}

fn read_cell<S: PropertySchema + ?Sized>(r: &mut Reader<'_>, schema: &S) -> Result<Cell> {
    let index = r.u16()?;
    let position = Position::read(r)?;
    let flags_at = r.position();
    let flags = r.u32()?;
    if flags & !CELL_FLAGS_KNOWN != 0 {
        return Err(FormatError::Unsupported {
            what: "cell flags",
            offset: flags_at,
            value: flags,
        });
    }
    let geometry = r.did()?;
    let count = r.u32()?;
    let mut neighbours = r.alloc_vec(count, NEIGHBOUR_SIZE, usize::MAX)?;
    for _ in 0..count {
        let n = CellNeighbour {
            ordinal: r.u32()?,
            cell: r.u16()?,
            back_reference: r.u16()?,
            reserved: [r.u16()?, r.u16()?, r.u16()?],
            flag: r.u8()?,
        };
        r.push(&mut neighbours, n, count)?;
    }
    let echo = flags & CELL_ECHO != 0;
    let index_echo = if echo { Some(r.u16()?) } else { None };
    let list_a = read_u16_list(r)?;
    let list_b = read_u16_list(r)?;
    let extra = if echo { Some(r.u16()?) } else { None };
    let properties = if flags & CELL_PROPERTIES != 0 {
        Some(read_embedded_set(r, schema)?)
    } else {
        None
    };
    Ok(Cell {
        index,
        position,
        flags,
        geometry,
        neighbours,
        index_echo,
        list_a,
        list_b,
        extra,
        properties,
    })
}

fn read_link<S: PropertySchema + ?Sized>(r: &mut Reader<'_>, schema: &S) -> Result<Link> {
    let name = r.pascal_string()?;
    let iid = InstanceId(r.u64()?);
    let to_iid = InstanceId(r.u64()?);
    let from_iid = InstanceId(r.u64()?);
    let link_type = r.u32()?;
    let properties = read_embedded_set(r, schema)?;
    let cross = if r.bool8()? {
        Some(LinkCoda {
            from_index: r.u32()?,
            from_info: r.did()?,
            to_info: r.did()?,
            to_index: r.u32()?,
        })
    } else {
        None
    };
    Ok(Link {
        name,
        iid,
        to_iid,
        from_iid,
        link_type,
        properties,
        cross,
    })
}
