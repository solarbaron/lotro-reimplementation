// SPDX-License-Identifier: GPL-3.0-or-later
//! Tessera's place lookup (`map-windows.md` §7.3, a Tessera rule): the client built-in
//! `findplace TEXT` of the one dispatcher. The matching steps, in order, the first with a
//! result winning:
//!
//! 1. TEXT parses as map coordinates ("12.3S, 45.6W", `coordinates.md` §8.3 inverse, in
//!    the shown map's region, else the player's) or as a pasted `/loc` line (§9.8): one
//!    result, that position;
//! 2. TEXT equals a key of the places table, ignoring case (`map-data.md` §3.4);
//! 3. the names of map nodes, areas, territories, regions and dungeons, matched and ranked
//!    as the walker's `--place` index ranks them (`tessera-gamedata`'s [`PlaceIndex`](tessera_gamedata::map::PlaceIndex)).
//!
//! At most [`MAX_PLACE_RESULTS`] results are listed as one chat line each (Tessera's own
//! wording); the first is shown on the map with the lookup marker. Nothing here writes to
//! the install.

use std::str::FromStr;

use tessera_formats::Did;
use tessera_gamedata::map::{MAX_PLACE_RESULTS, PlaceKind, PlaceMatch};
use tessera_ui::game::map::{BLOCK_UNITS, GameMap, MapPoint};
use tessera_world::{LocReport, MapCoords};

use super::data::MapFacts;
use super::texts::coordinates_text;

/// Tessera's line when nothing matches (`map-windows.md` §7.3).
pub const NO_PLACE: &str = "No place found.";

/// What the map shows for a lookup result (§7.3 "Result").
#[derive(Debug, Clone, PartialEq)]
pub enum PlaceTarget {
    /// A map node, by its active element ID.
    Node(u32),
    /// A dungeon map, with the dungeon whose notes it shows (the zone of its note query).
    Dungeon {
        /// The dungeon map.
        map: GameMap,
        /// The dungeon record.
        zone: Did,
    },
    /// The player's-map choice of §4.6 step 3 for a position's area (`None`: unknown, the
    /// root).
    Area(Option<u32>),
}

/// The outcome of one lookup.
#[derive(Debug, Clone, PartialEq, Default)]
pub struct LookupOutcome {
    /// The chat lines (results, or [`NO_PLACE`]).
    pub lines: Vec<String>,
    /// The first result's map and the lookup marker (position and label), when there is a
    /// result.
    pub show: Option<(PlaceTarget, Option<(MapPoint, String)>)>,
}

/// A `tessera-gamedata` map-data set's region as a `tessera-ui` game map (the dungeon
/// map of `map-windows.md` §4.7).
#[must_use]
pub fn dungeon_game_map(facts: &MapFacts, map_data: Did) -> Option<GameMap> {
    let regions = facts.regions.as_ref()?;
    let r = regions.find(map_data, 0)?;
    Some(game_map_of(r))
}

/// The `tessera-ui` game map of a map region (image and georeference).
#[must_use]
pub fn game_map_of(r: &tessera_gamedata::map::MapRegion) -> GameMap {
    let g = r.georef;
    GameMap {
        image: r.image.map_or(0, |d| d.0),
        layers: Vec::new(),
        block_x: g.map(|g| g.block_x),
        block_y: g.map(|g| g.block_y),
        pixel_x: g.map_or(0, |g| g.pixel_x),
        pixel_y: g.map_or(0, |g| g.pixel_y),
        scale: g.map(|g| g.scale),
        fog: false,
    }
}

/// The area of a position (its landblock's `Area_DID`).
fn area_of(facts: &MapFacts, p: &MapPoint) -> Option<u32> {
    let bx = (f64::from(p.global.0) / f64::from(BLOCK_UNITS)).floor();
    let by = (f64::from(p.global.1) / f64::from(BLOCK_UNITS)).floor();
    if !(0.0..255.0).contains(&bx) || !(0.0..255.0).contains(&by) {
        return None;
    }
    #[allow(clippy::cast_possible_truncation, clippy::cast_sign_loss)]
    facts.area_at(p.region, bx as u8, by as u8)
}

/// Step 1: map coordinates or a `/loc` line.
fn parse_position(facts: &MapFacts, text: &str, region: Option<u8>) -> Option<MapPoint> {
    if let Ok(m) = MapCoords::from_str(text) {
        let region = region.unwrap_or(1);
        let c = facts.constants.get(&region)?;
        let [x, y] = c.to_region(m);
        #[allow(clippy::cast_possible_truncation)]
        return Some(MapPoint {
            global: (x as f32, y as f32),
            region,
            instance: 0,
        });
    }
    let t = text.trim();
    let t = t.strip_prefix("/loc").map_or(t, str::trim);
    let r = LocReport::from_str(t).ok()?;
    let w = r.to_world();
    #[allow(clippy::cast_possible_truncation)]
    Some(MapPoint {
        global: (w.x as f32, w.y as f32),
        region: w.region,
        instance: 0,
    })
}

fn location_point(l: &tessera_gamedata::Location) -> Option<MapPoint> {
    #[allow(clippy::cast_possible_truncation)]
    Some(MapPoint {
        global: (l.x as f32, l.y as f32),
        region: u8::try_from(l.region).ok()?,
        instance: 0,
    })
}

/// One result line (Tessera's wording: kind, name, map coordinates when known).
fn line(facts: &MapFacts, m: &PlaceMatch) -> String {
    let coords = m
        .entry
        .position
        .as_ref()
        .and_then(location_point)
        .and_then(|p| coordinates_text(&facts.constants, &p));
    match coords {
        Some(c) => format!("{}: {} ({c})", m.entry.kind.word(), m.entry.name),
        None => format!("{}: {}", m.entry.kind.word(), m.entry.name),
    }
}

/// Runs a lookup of `text` (§7.3) with the shown map's region and the player's region.
#[must_use]
pub fn find_place(
    facts: &MapFacts,
    text: &str,
    shown_region: Option<u8>,
    player_region: Option<u8>,
) -> LookupOutcome {
    let text = text.trim();
    if text.is_empty() {
        return LookupOutcome::default();
    }
    if let Some(p) = parse_position(facts, text, shown_region.or(player_region)) {
        let label = coordinates_text(&facts.constants, &p).unwrap_or_else(|| text.to_owned());
        return LookupOutcome {
            lines: vec![format!("place: {label}")],
            show: Some((PlaceTarget::Area(area_of(facts, &p)), Some((p, label)))),
        };
    }
    let matches =
        facts
            .place_index
            .lookup(text, facts.places.as_deref(), shown_region.map(u32::from));
    let Some(first) = matches.first() else {
        return LookupOutcome {
            lines: vec![NO_PLACE.to_owned()],
            show: None,
        };
    };
    let lines = matches
        .iter()
        .take(MAX_PLACE_RESULTS)
        .map(|m| line(facts, m))
        .collect();
    let e = &first.entry;
    let node_element = |n: Option<usize>| {
        let regions = facts.regions.as_ref()?;
        Some(regions.tree.nodes.get(n?)?.active_element)
    };
    let show = match e.kind {
        PlaceKind::Node => node_element(e.node).map(|el| (PlaceTarget::Node(el), None)),
        PlaceKind::Area | PlaceKind::Territory | PlaceKind::Region => {
            // The target of this record (not of another record with the same name).
            let marker = facts.place_targets.get(&e.id).map(|(region, t)| {
                #[allow(clippy::cast_possible_truncation)]
                let point = MapPoint {
                    global: (t[0] as f32, t[1] as f32),
                    region: *region,
                    instance: 0,
                };
                (point, e.name.clone())
            });
            let target = match node_element(e.node) {
                Some(el) => PlaceTarget::Node(el),
                None => PlaceTarget::Area(None),
            };
            Some((target, marker))
        }
        PlaceKind::Dungeon => {
            let marker = e
                .position
                .as_ref()
                .and_then(location_point)
                .map(|p| (p, e.name.clone()));
            match e.dungeon_map.and_then(|d| dungeon_game_map(facts, d)) {
                Some(map) => Some((
                    PlaceTarget::Dungeon {
                        map,
                        zone: Did(e.id),
                    },
                    marker,
                )),
                None => marker
                    .as_ref()
                    .map(|(p, _)| PlaceTarget::Area(area_of(facts, p)))
                    .map(|t| (t, marker.clone())),
            }
        }
        PlaceKind::PlaceKey => e.position.as_ref().and_then(location_point).map(|p| {
            (
                PlaceTarget::Area(area_of(facts, &p)),
                Some((p, e.name.clone())),
            )
        }),
    };
    LookupOutcome { lines, show }
}
