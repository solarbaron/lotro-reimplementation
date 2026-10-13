// SPDX-License-Identifier: GPL-3.0-or-later
//! The map windows in the client (decision 0024 P5, client half only;
//! `docs/specs/map-windows.md`, decision 0036): the world-map window with its notes and
//! the scenario feed's markers, the minimap's feed markers, Tessera's place lookup
//! (`findplace`) and the census of every map region.
//!
//! The client runs no map rule of the game (discovery, world events, the player's
//! dungeon are Tessera rules of `map-windows.md` §8); the markers come from the scenario
//! feed (`MarkerAdded`, `MarkerRemoved`), the maps, notes, names and icons from the
//! install.
//!
//! | Module | What | Spec |
//! |---|---|---|
//! | [`data`] | [`MapFacts`]: the install's map regions, notes, icon tables, region menu, option defaults, map constants, landblock areas, place indexes | MW §4–§8 |
//! | [`texts`] | [`InstallMapTexts`]: coordinates, note-type labels, filter-menu items | MW §3.4, §6.9 |
//! | [`ui`] | [`MapUi`]: the map window (`tessera-ui`'s view) with notes and feed markers, the minimap's feed markers | MW §3–§6 |
//! | [`lookup`] | `findplace`: coordinates, `/loc` lines, place keys, names | MW §7.3 |
//! | [`census`] | the map-region census of P5 exit criterion 1 | MW §5.4; decision 0036 §1 |
//! | [`hub`] | [`MapHub`]: loading off the frame, the frame's work, the map key | — |
//!
//! The P3 quest markers on the minimap stay the quest journal's (`crate::quests`); the map
//! window draws every feed marker, quest markers included (§6.7).

pub mod census;
pub mod data;
pub mod hub;
pub mod lookup;
pub mod texts;
pub mod ui;

pub use census::{CensusReport, CensusRun, CensusStep};
pub use data::{MapFacts, MapSettings};
pub use hub::{MapHub, P5Key, P5Keys, parse_map_panel};
pub use lookup::{LookupOutcome, PlaceTarget, find_place};
pub use texts::InstallMapTexts;
pub use ui::{MapChange, MapUi, player_map, ui_marker};
