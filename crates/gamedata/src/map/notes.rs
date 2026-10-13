// SPDX-License-Identifier: GPL-3.0-or-later
//! The table notes the map window shows on one map (`map-data.md` §4.8.1–§4.8.2, §4.8.4,
//! §4.8.6; `map-windows.md` §6.1, §6.3, §8) and the note icon tables (`map-data.md` §4.8.5).

use std::collections::{BTreeMap, BTreeSet};

use tessera_formats::gameplay::common::PropertyAccess;
use tessera_formats::gameplay::map::{MapNote, MapNoteIndex, MapNotesTable};
use tessera_formats::property::PropertyValue;
use tessera_formats::{Did, PropertySet, StringInfo};

use super::tree::{Georef, MapPids, MapTree, key_of};
use super::{FILTER_READ_MASK, MAX_NOTES_SHOWN, VALID_EXTENT};
use crate::query::Location;
use crate::tables::position_location;

/// Content-layer key `InstanceZero` (`map-data.md` §4.8.2).
pub const LAYER_INSTANCE_ZERO: u32 = 1;
/// Content-layer key `All` (`map-data.md` §4.8.2).
pub const LAYER_ALL: u32 = 2;

/// The map a note selection is for.
#[derive(Debug, Clone, Copy, PartialEq)]
pub enum ShownMap {
    /// An outdoor map node (index into [`MapTree::nodes`]).
    Node(usize),
    /// A dungeon map: the dungeon (zone) and the map's georeference and pixel frame.
    Dungeon {
        /// The dungeon record (the zone query's zone).
        zone: Did,
        /// The dungeon map's georeference.
        georef: Option<Georef>,
        /// Its pixel frame.
        frame: (u32, u32),
    },
}

/// The client-half settings a selection uses (`map-windows.md` §8).
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct NoteSettings {
    /// Content layer *L* (`map-windows.md` §8: the walker's content layer).
    pub layer: u32,
    /// Tessera setting `map_discovery`: `true` = every discovery key counts as discovered.
    pub discovery_all: bool,
    /// The filter value as stored in the option (it is ANDed with
    /// [`FILTER_READ_MASK`] here, `map-windows.md` §6.9).
    pub filter: u64,
}

/// One table note selected for a map.
#[derive(Debug, Clone, PartialEq)]
pub struct SelectedNote {
    /// Row of the map-note table.
    pub row: usize,
    /// The row's `type` bits.
    pub type_bits: u64,
    /// Whether it is a link row.
    pub link: bool,
    /// Where it is, when its position is valid: the row's `position`, or for the zone-list
    /// entry of a link row leading into a zone, its `link_position` (`map-data.md` §4.6).
    pub location: Option<Location>,
    /// Its map pixel, when its location maps into the shown map's frame by
    /// `map-windows.md` §6.1 (0 ≤ *px* < width, 0 ≤ *py* < height).
    pub pixel: Option<(i32, i32)>,
    /// Visible under the filter value (`map-windows.md` §6.3 item 3).
    pub visible: bool,
    /// Hidden because no world-event state holds (`map-data.md` §4.8.6 item 3). Tessera
    /// does not evaluate the states yet: no session supplies world properties
    /// (`map-windows.md` §8), so every row with a state array (empty or not) is hidden and no
    /// state's icon or text replacement applies.
    pub world_event_hidden: bool,
    /// The note's text (`text`), rendered; `None` for null texts and render misses.
    pub text: Option<String>,
    /// The row's stored text.
    pub text_info: StringInfo,
}

/// The filter value the window uses after reading an option (`map-windows.md` §6.9).
#[must_use]
pub fn read_filter(option: u64) -> u64 {
    option & FILTER_READ_MASK
}

/// Whether a row passes the discovery and content-layer tests (`map-data.md` §4.8.2).
#[must_use]
pub fn passes_tests(n: &MapNote, s: &NoteSettings) -> bool {
    if n.discovery != 0 && !s.discovery_all {
        return false;
    }
    let Some(entry) = &n.base.layers else {
        return false;
    };
    let Some(arr) = entry.value.as_array() else {
        return false;
    };
    if arr.elements.is_empty() {
        return true;
    }
    arr.elements.iter().any(|e| {
        let key = match &e.value {
            PropertyValue::Enum(k) | PropertyValue::PropertyId(k) => *k,
            _ => return false,
        };
        key == s.layer || key == LAYER_ALL || (key == LAYER_INSTANCE_ZERO && s.layer == 0)
    })
}

/// One entry of a query result: a row and whether it is the zone-list entry of a link row
/// leading into the zone (placed at the row's `link_position`, `map-data.md` §4.6) rather
/// than the row itself (placed at its `position`).
type QueryEntry = (usize, bool);

/// The zone list of `zone` (`map-data.md` §4.6): rows filed under it and the entries of
/// link rows leading into it, ascending row order (a row is never in both: a link row gets
/// an entry only in a zone other than its own dungeon).
fn zone_rows(index: &MapNoteIndex, zone: Did) -> Vec<QueryEntry> {
    let mut rows: Vec<QueryEntry> = index
        .in_dungeon(zone)
        .iter()
        .map(|&r| (r, false))
        .chain(index.links_into(zone).iter().map(|&r| (r, true)))
        .collect();
    rows.sort_unstable();
    rows
}

/// Appends the zone lists of the distinct link targets among `rows` (other than `skip`),
/// one level deep, in first-seen order (`map-data.md` §4.8.1).
fn follow_links(
    table: &MapNotesTable,
    index: &MapNoteIndex,
    rows: &[QueryEntry],
    skip: Option<Did>,
    out: &mut Vec<QueryEntry>,
) {
    let mut seen = BTreeSet::new();
    let mut targets = Vec::new();
    for &(r, _) in rows {
        if let Some(l) = table.notes.get(r).and_then(|n| n.link.as_ref())
            && !l.target.is_null()
            && Some(l.target) != skip
            && seen.insert(l.target)
        {
            targets.push(l.target);
        }
    }
    for t in targets {
        out.extend(zone_rows(index, t));
    }
}

/// The landblock rectangle a node's frame covers, clamped to 0 … 254.
#[allow(clippy::cast_possible_truncation, clippy::cast_sign_loss)]
fn frame_blocks(g: &Georef, frame: (u32, u32)) -> Option<((u8, u8), (u8, u8))> {
    let a = g.to_world(0.0, 0.0)?;
    let b = g.to_world(f64::from(frame.0), f64::from(frame.1))?;
    let block = |v: f64| (v / 160.0).floor().clamp(0.0, 254.0) as u8;
    let (x0, x1) = (a.0.min(b.0), a.0.max(b.0));
    let (y0, y1) = (a.1.min(b.1), a.1.max(b.1));
    if x1 < 0.0 || y1 < 0.0 || x0 >= VALID_EXTENT || y0 >= VALID_EXTENT {
        return None;
    }
    Some(((block(x0), block(x1)), (block(y0), block(y1))))
}

/// Whether a row's `extra` holds a world-event state array (`map-data.md` §4.8.6).
fn has_world_event_states(extra: &PropertySet, p: &MapPids) -> bool {
    if extra.has(p.note_states) {
        return true;
    }
    extra
        .structure(p.note_game_specific)
        .is_some_and(|s| s.has(p.note_states))
}

/// The table notes the map window creates on `shown` (`map-data.md` §4.8.1, §4.8.2,
/// §4.8.4; `map-windows.md` §6.3 items 1–3), in query order without repeats, with their
/// visibility under the filter, their pixel and their text through `render`. At most
/// [`MAX_NOTES_SHOWN`] notes.
///
/// On an outdoor node the area query uses the node's region and the landblocks its pixel
/// frame covers (`frame` is the node's frame); a node without a region, georeference or
/// usable scale gets no table note. A note is created when its `type` shares a bit with
/// the node's display mask (every type on a dungeon map) and, outdoors, the node of its
/// area is the shown node or a descendant (an unlisted area belongs to the root).
#[must_use]
#[allow(clippy::too_many_arguments)]
pub fn select_notes(
    table: &MapNotesTable,
    index: &MapNoteIndex,
    tree: &MapTree,
    shown: ShownMap,
    frame: (u32, u32),
    settings: &NoteSettings,
    pids: &MapPids,
    render: &dyn Fn(&StringInfo) -> Option<String>,
) -> Vec<SelectedNote> {
    let filter = read_filter(settings.filter);
    let (rows, mask, georef, origin_region, shown_node) = match shown {
        ShownMap::Node(i) => {
            let Some(node) = tree.nodes.get(i) else {
                return Vec::new();
            };
            let (Some(region), Some(g)) = (node.region, node.map.georef()) else {
                return Vec::new();
            };
            if !g.is_usable() {
                return Vec::new();
            }
            let Some(((x0, x1), (y0, y1))) = frame_blocks(&g, frame) else {
                return Vec::new();
            };
            let Ok(r8) = u8::try_from(region) else {
                return Vec::new();
            };
            let mut rows: Vec<QueryEntry> = index
                .ranged
                .iter()
                .copied()
                .filter(|&r| {
                    table
                        .notes
                        .get(r)
                        .and_then(|n| n.base.position.region())
                        .is_some_and(|nr| nr == r8)
                })
                .map(|r| (r, false))
                .collect();
            let mut block_rows = Vec::new();
            for x in x0..=x1 {
                for y in y0..=y1 {
                    block_rows.extend(index.in_landblock(r8, x, y).iter().map(|&r| (r, false)));
                }
            }
            rows.extend_from_slice(&block_rows);
            follow_links(table, index, &block_rows, None, &mut rows);
            (rows, node.display_mask(), Some(g), Some(region), Some(i))
        }
        ShownMap::Dungeon { zone, georef, .. } => {
            let first = zone_rows(index, zone);
            let mut rows = first.clone();
            follow_links(table, index, &first, Some(zone), &mut rows);
            (rows, u64::MAX, georef, None, None)
        }
    };
    let mut seen = BTreeSet::new();
    let mut out = Vec::new();
    for (r, from_link) in rows {
        if out.len() >= MAX_NOTES_SHOWN {
            tracing::warn!(cap = MAX_NOTES_SHOWN, "note selection cut");
            break;
        }
        if !seen.insert(r) {
            continue;
        }
        let Some(n) = table.notes.get(r) else {
            continue;
        };
        if !passes_tests(n, settings) || n.note_type & mask == 0 {
            continue;
        }
        if let Some(sn) = shown_node {
            let owner = tree.node_for_area(n.base.area).unwrap_or(0);
            if !tree.is_within(owner, sn) {
                continue;
            }
        }
        // A zone-list entry of a link row stands at the link's `link_position`, inside the
        // zone (`map-data.md` §4.6).
        let position = match (from_link, &n.link) {
            (true, Some(l)) => &l.position,
            _ => &n.base.position,
        };
        let (location, _) = position_location(position);
        let pixel = match (location, georef) {
            (Some(l), Some(g)) => {
                let origin = origin_region.unwrap_or(u32::from(l.region));
                #[allow(clippy::cast_possible_truncation)]
                let (px, py) = g.to_pixel(l.x as f32, l.y as f32);
                (u32::from(l.region) == origin
                    && px >= 0
                    && py >= 0
                    && i64::from(px) < i64::from(frame.0)
                    && i64::from(py) < i64::from(frame.1))
                .then_some((px, py))
            }
            _ => None,
        };
        let link = n.is_link();
        let visible = if link {
            filter & 0b10 != 0
        } else {
            n.note_type & filter != 0
        };
        // No session supplies world properties yet, so no state's condition holds and every
        // row with a state array is hidden (`map-windows.md` §8; `map-data.md` §4.8.6 item 3).
        let world_event_hidden = has_world_event_states(&n.extra, pids);
        out.push(SelectedNote {
            row: r,
            type_bits: n.note_type,
            link,
            location,
            pixel,
            visible,
            world_event_hidden,
            text: render(&n.text),
            text_info: n.text.clone(),
        });
    }
    out
}

/// The three images of a note state (`map-data.md` §4.8.5).
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub struct NoteImages {
    /// `UI_Map_MapNoteImage`.
    pub normal: Option<Did>,
    /// `UI_Map_MapNoteSelectedImage`.
    pub selected: Option<Did>,
    /// `UI_Map_MapNoteHighlightImage`.
    pub highlight: Option<Did>,
}

/// One icon-table entry (`map-data.md` §4.8.5).
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub struct NoteIcon {
    /// On-screen images.
    pub onscreen: NoteImages,
    /// Off-screen images, when the entry has them.
    pub offscreen: Option<NoteImages>,
    /// `UI_Map_MapNoteMenuImage`.
    pub menu: Option<Did>,
    /// `UI_Map_MapNoteRelativeZLevel` (absent = 0).
    pub relative_z: i32,
}

/// A note icon table: `type` value → entry, the first entry winning for a repeated value
/// (`map-data.md` §4.8.5).
#[derive(Debug, Clone, PartialEq, Eq, Default)]
pub struct NoteIcons {
    /// Entries by their whole 64-bit `UI_Map_MapNoteType` value.
    pub entries: BTreeMap<u64, NoteIcon>,
}

impl NoteIcons {
    /// Reads `UI_Map_MapNoteTypeArray` of an icon-table set.
    #[must_use]
    pub fn read(set: &PropertySet, p: &MapPids) -> Self {
        let images = |s: &tessera_formats::property::PropertyStruct| NoteImages {
            normal: s.did_nonnull(p.note_image),
            selected: s.did_nonnull(p.note_selected),
            highlight: s.did_nonnull(p.note_highlight),
        };
        let mut entries = BTreeMap::new();
        for s in set.structs(p.note_type_array) {
            let Some(ty) = s.bitfield(p.note_type_value) else {
                continue;
            };
            entries.entry(ty).or_insert(NoteIcon {
                onscreen: s.structure(p.onscreen).map(images).unwrap_or_default(),
                offscreen: s.structure(p.offscreen).map(images),
                menu: s.did_nonnull(p.note_menu_image),
                relative_z: s.int(p.note_rel_z).unwrap_or(0),
            });
        }
        Self { entries }
    }

    /// The entry for a note's whole `type` value; `None` gives no icon.
    #[must_use]
    pub fn get(&self, type_bits: u64) -> Option<&NoteIcon> {
        self.entries.get(&type_bits)
    }

    /// The entry for the single bit *b* (the feed-marker icons of `map-windows.md` §6.7
    /// item 2: bit 1 `Quest`, bit 47 `Waypoint`).
    #[must_use]
    pub fn for_bit(&self, bit: u32) -> Option<&NoteIcon> {
        (bit < 64).then(|| self.get(1u64 << bit)).flatten()
    }
}

/// The map-region selector element's `region` attribute value: the region of the
/// element's location (`scenario-feed.md` §8.3).
#[must_use]
pub fn location_region(l: Option<&Location>) -> Option<u32> {
    l.map(|l| u32::from(l.region))
}

/// The regions of `Region_Map_List` in stored order: (`Region_ID`, `Region_Name`)
/// (`map-windows.md` §4.5; `wstate.md` §11.3).
#[must_use]
pub fn region_map_list(set: &PropertySet, p: &MapPids) -> Vec<(u32, Option<StringInfo>)> {
    set.structs(p.region_map_list)
        .filter_map(|s| {
            let id = s.get_pid(p.region_id).and_then(key_of)?;
            Some((id, s.string_info(p.region_name).cloned()))
        })
        .collect()
}
