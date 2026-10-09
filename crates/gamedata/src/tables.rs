// SPDX-License-Identifier: GPL-3.0-or-later
//! The two table families of `scenario-feed.md` §8.3: entries of the places table
//! (`map-data.md` §3) and rows of the map-notes table (`map-data.md` §4), as selector
//! elements with a row number, a location and an anchor.

use tessera_formats::gameplay::map::{MapNotesTable, PlacesTable, PositionValues};
use tessera_formats::property::Position;

use crate::query::{Anchor, Candidate, Location};

/// Landblock edge in metres (`coordinates.md` §4: G = 160 · index + local).
const LANDBLOCK_SIZE: f64 = 160.0;

/// One table element with the values its attributes read.
#[derive(Debug, Clone, PartialEq)]
pub(crate) struct TableElement {
    pub candidate: Candidate,
    /// The position's region (0 when the position has none).
    pub region: u8,
    /// The `MapNote_Type` bits (map notes only).
    pub type_bits: u64,
}

/// The location and anchor of a stored position (`coordinates.md` §4, §7).
///
/// A position without a region, landblock or cell, with region 0, a landblock index of
/// 0xFF, cell 0xFFFF or a non-finite offset is invalid (§4) and gives neither. Otherwise
/// the location is region-global (160 · index + block-local offset, *z* as stored); an
/// interior position (cell ≠ 0) also gets an anchor naming its landblock and cell.
#[must_use]
pub fn position_location(p: &Position) -> (Option<Location>, Option<Anchor>) {
    let v = PositionValues::of(p);
    let (bx, by) = v.landblock;
    if p.region().is_none()
        || p.landblock().is_none()
        || v.region == 0
        || bx == 0xFF
        || by == 0xFF
        || v.cell == 0xFFFF
        || !v.offset.iter().all(|c| c.is_finite())
    {
        return (None, None);
    }
    let [ox, oy, oz] = v.offset;
    let location = Location {
        region: u16::from(v.region),
        x: LANDBLOCK_SIZE * f64::from(bx) + f64::from(ox),
        y: LANDBLOCK_SIZE * f64::from(by) + f64::from(oy),
        z: f64::from(oz),
    };
    let anchor = (v.cell != 0).then_some(Anchor {
        region: u16::from(v.region),
        x: bx,
        y: by,
        cell: v.cell,
    });
    (Some(location), anchor)
}

fn row(i: usize) -> u32 {
    u32::try_from(i).unwrap_or(u32::MAX)
}

/// The elements of the places table: DID = the entry's `area` field (may be 0), row = its
/// stored index (`scenario-feed.md` §8.3).
pub(crate) fn place_elements(table: &PlacesTable) -> Vec<TableElement> {
    table
        .places
        .iter()
        .enumerate()
        .map(|(i, p)| {
            let (location, anchor) = position_location(&p.position);
            TableElement {
                candidate: Candidate {
                    did: p.area,
                    row: row(i),
                    location,
                    anchor,
                },
                region: p.position.region().unwrap_or(0),
                type_bits: 0,
            }
        })
        .collect()
}

/// The elements of the map-notes table: DID = the row's entity, row = its stored index.
pub(crate) fn map_note_elements(table: &MapNotesTable) -> Vec<TableElement> {
    table
        .notes
        .iter()
        .enumerate()
        .map(|(i, n)| {
            let (location, anchor) = position_location(&n.base.position);
            TableElement {
                candidate: Candidate {
                    did: n.base.entity,
                    row: row(i),
                    location,
                    anchor,
                },
                region: n.base.position.region().unwrap_or(0),
                type_bits: n.note_type,
            }
        })
        .collect()
}
