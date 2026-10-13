// SPDX-License-Identifier: GPL-3.0-or-later
//! The map windows' presentation data (decision 0024 P5): the map tree, every map region
//! with its census category and test point, the table notes of one map, the note icon
//! tables and Tessera's place lookup.
//!
//! Spec: `docs/specs/map-windows.md` §4.1, §5, §6.1, §6.3, §6.9, §7.3, §8 (with
//! `map-data.md` §3.4, §4.6–§4.8, `game-panels.md` §8.6.1, §9.1 and `coordinates.md` §4).
//! Widgets, navigation and drawing belong to `tessera-ui` and the client; nothing here
//! decides a note's discovery, a world event or a player's dungeon (`map-windows.md` §8).
//!
//! | Item | Purpose | Spec |
//! |---|---|---|
//! | [`MapPids`], [`MapTree`], [`MapNode`], [`GameMap`], [`Georef`], [`game_map_of`] | The map tree, map images and the georeference | `map-windows.md` §4.1; `game-panels.md` §8.6.1, §9.1 |
//! | [`MapData`], [`InstallMapData`], [`MapLayout`], [`ImageInfo`], [`DungeonRef`], [`AreaRef`], [`GeoLevel`] | What the models read, and its implementation over an install | — |
//! | [`MapRegions`], [`MapRegion`], [`RegionKind`], [`RegionClass`], [`ExcludedSet`], [`classify_frame`], [`marker_in_frame`], [`MapRegionCensus`] | Every map region, its category C1–C6 and test point; the census | §5.1–§5.4 |
//! | [`select_notes`], [`ShownMap`], [`NoteSettings`], [`SelectedNote`], [`passes_tests`], [`read_filter`], [`NoteIcons`], [`NoteIcon`], [`NoteImages`], [`region_map_list`] | Table notes of one map; icon tables; the map menu's regions | §4.5, §6.3, §6.9; `map-data.md` §4.8 |
//! | [`PlaceIndex`], [`PlaceEntry`], [`PlaceMatch`], [`PlaceKind`], [`normalize_name`], [`match_score`] | Place lookup steps 2–3 | §7.3; `walker.md` §6 |
//!
//! # Hostile input
//!
//! Nothing here parses bytes (records, layouts and image headers are decoded by
//! `tessera-formats`). Work is bounded: [`MAX_NODES`] nodes, depth [`MAX_DEPTH`],
//! [`MAX_AREAS`] area DIDs per tree, [`MAX_NOTES_SHOWN`] notes per selection,
//! [`MAX_PLACE_RESULTS`] lookup results.

mod install;
mod notes;
mod places;
mod regions;
mod tree;

#[cfg(test)]
mod tests;

pub use install::{InstallMapData, MAP_ICONS_LABEL, RADAR_ICONS_LABEL, REGION_CONTROL_LABEL};
pub use notes::{
    LAYER_ALL, LAYER_INSTANCE_ZERO, NoteIcon, NoteIcons, NoteImages, NoteSettings, SelectedNote,
    ShownMap, location_region, passes_tests, read_filter, region_map_list, select_notes,
};
pub use places::{
    MAX_QUERY_CHARS, PlaceEntry, PlaceIndex, PlaceKind, PlaceMatch, match_score, normalize_name,
};
pub use regions::{
    AreaRef, DungeonRef, ExcludedSet, GeoLevel, ImageInfo, MapData, MapLayout, MapRegion,
    MapRegionCensus, MapRegions, RegionClass, RegionKind, classify_frame, marker_in_frame,
};
pub use tree::{GameMap, Georef, MapNode, MapPids, MapTree, game_map_of};

/// The `UILAYOUT` label of the map layout (`map-windows.md` §3).
pub const MAP_LAYOUT: &str = "map";
/// The element that carries the map tree (`game-panels.md` §9.1).
pub const MAP_WINDOW: &str = "MapWindow";
/// The DID prefix of stand-alone property sets, which map-data sets are (`map-data.md`
/// §2.1, MD46).
pub const MAP_SET_PREFIX: u8 = 0x78;
/// Most nodes of a map tree (a Tessera bound; the data has 227).
pub const MAX_NODES: usize = 4_096;
/// Deepest node (a Tessera bound; the data reaches depth 5).
pub const MAX_DEPTH: usize = 32;
/// Most area DIDs over a tree (a Tessera bound; the data has 541).
pub const MAX_AREAS: usize = 65_536;
/// Most notes one selection returns (a Tessera bound).
pub const MAX_NOTES_SHOWN: usize = 16_384;
/// Most results of a place lookup (`map-windows.md` §7.3).
pub const MAX_PLACE_RESULTS: usize = 10;
/// Most regions a census lists by name (a Tessera bound).
pub const MAX_LISTED: usize = 256;
/// Smallest usable scale magnitude (`map-windows.md` §5.3 C4).
pub const MIN_SCALE: f32 = 0.000_2;
/// The far edge of the valid block grid: 255 · 160 (`coordinates.md` §4).
pub const VALID_EXTENT: f64 = 255.0 * 160.0;
/// The common note-type mask ORed into a node's display mask when
/// `UI_Map_IncludeCommonMapNoteTypes` is true (`map-windows.md` MW3, BIN).
pub const COMMON_NOTE_MASK: u64 = 0x1B7C_76E7_5CF0_0000;
/// The mask the window ANDs into a filter option when it reads it (`map-windows.md` MW12,
/// §6.9, BIN).
pub const FILTER_READ_MASK: u64 = 0x9B7C_7FFF_FEFF_0002;

/// Why map data could not be read.
#[derive(Debug, Clone, PartialEq, Eq, thiserror::Error)]
#[non_exhaustive]
pub enum MapError {
    /// A value has the wrong shape.
    #[error("malformed map data: {0}")]
    Malformed(&'static str),
    /// A Tessera bound was passed.
    #[error("{what} exceeds the bound of {cap}")]
    TooLarge {
        /// What.
        what: &'static str,
        /// The bound.
        cap: usize,
    },
    /// The install's dictionary or labels cannot be read.
    #[error("map data unavailable: {0}")]
    Unavailable(String),
}
