// SPDX-License-Identifier: GPL-3.0-or-later
//! Tessera's place lookup (`map-windows.md` §7.3, a Tessera rule): steps 2 and 3 of its
//! matching (place keys of the places table, `map-data.md` §3.4; names of map nodes,
//! areas, territories, regions and dungeons, normalised and ranked as the walker's
//! `--place` index, `walker.md` §6). Step 1 (map coordinates, `/loc` text) needs the
//! region descriptors and the `/loc` parser of `tessera-world`, which this crate does not
//! link: the caller tries it first.

use std::cmp::Ordering;
use std::collections::HashMap;

use tessera_formats::Did;
use tessera_formats::gameplay::map::PlacesTable;

use super::MAX_PLACE_RESULTS;
use super::regions::{AreaRef, GeoLevel, MapRegions};
use crate::query::Location;
use crate::tables::position_location;

/// Longest query text read (characters; longer text is cut).
pub const MAX_QUERY_CHARS: usize = 256;

/// What a lookup result names (`map-windows.md` §7.2), in the tie order of §7.3 step 3
/// (more specific first).
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, PartialOrd, Ord)]
pub enum PlaceKind {
    /// A dungeon (`Dungeon_Name`).
    Dungeon,
    /// An area (`Area_Name`).
    Area,
    /// A territory.
    Territory,
    /// A region record.
    Region,
    /// A map node (`UI_Map_MenuName`).
    Node,
    /// A key of the places table (step 2).
    PlaceKey,
}

impl PlaceKind {
    /// A short English word for Tessera's own chat lines.
    #[must_use]
    pub fn word(self) -> &'static str {
        match self {
            Self::Dungeon => "dungeon",
            Self::Area => "area",
            Self::Territory => "territory",
            Self::Region => "region",
            Self::Node => "map",
            Self::PlaceKey => "place",
        }
    }
}

/// One named place of the index.
#[derive(Debug, Clone, PartialEq)]
pub struct PlaceEntry {
    /// What it is.
    pub kind: PlaceKind,
    /// The rendered name.
    pub name: String,
    /// [`normalize_name`] of `name`.
    pub norm: String,
    /// The record (area, dungeon) or, for a node, its active element ID.
    pub id: u32,
    /// The region number used for the tie rule, when known.
    pub region: Option<u32>,
    /// The place's size for the tie rule (larger first; the client's landblock index can
    /// fill it for areas, 0 otherwise).
    pub size: u32,
    /// The map node to show: the node itself, the node of the area (or of the
    /// territory's or region's first listed area in tree order), the root (0) for an
    /// area, territory or region no node lists (`map-windows.md` §4.1, §4.6 step 3);
    /// `None` for dungeons, place keys and an empty tree.
    pub node: Option<usize>,
    /// A dungeon's map-data set (its dungeon map).
    pub dungeon_map: Option<Did>,
    /// Where the lookup marker goes, when the place has a position (a dungeon's position;
    /// for areas, territories and regions the client's landblock index supplies the
    /// target of `walker.md` §6).
    pub position: Option<Location>,
}

/// A lookup result.
#[derive(Debug, Clone, PartialEq)]
pub struct PlaceMatch {
    /// The entry (for a place key: an entry built from the table row).
    pub entry: PlaceEntry,
    /// Match quality: 4 a place key, 3 exact, 2 prefix at a word boundary, 1 whole-word
    /// substring.
    pub score: u8,
}

/// Normalises a name or query (`walker.md` §6): lower case, letters and digits kept (Latin-1
/// accented letters folded to their base letter), every run of other characters one space,
/// trimmed; at most [`MAX_QUERY_CHARS`] characters are read.
#[must_use]
pub fn normalize_name(s: &str) -> String {
    let mut out = String::with_capacity(s.len().min(MAX_QUERY_CHARS));
    let mut space = false;
    for c in s.chars().take(MAX_QUERY_CHARS) {
        let c = fold_latin1(c);
        if c.is_alphanumeric() {
            if space && !out.is_empty() {
                out.push(' ');
            }
            space = false;
            out.extend(c.to_lowercase().filter(|l| l.is_alphanumeric()));
        } else {
            space = true;
        }
    }
    out
}

fn fold_latin1(c: char) -> char {
    match c {
        'À'..='Å' | 'à'..='å' => 'a',
        'Ç' | 'ç' => 'c',
        'È'..='Ë' | 'è'..='ë' => 'e',
        'Ì'..='Ï' | 'ì'..='ï' => 'i',
        'Ñ' | 'ñ' => 'n',
        'Ò'..='Ö' | 'Ø' | 'ò'..='ö' | 'ø' => 'o',
        'Ù'..='Ü' | 'ù'..='ü' => 'u',
        'Ý' | 'ý' | 'ÿ' => 'y',
        _ => c,
    }
}

/// How well a normalised `name` matches a normalised `query`: exact (3), prefix at a word
/// boundary (2), whole-word substring (1), else `None` (`walker.md` §6).
#[must_use]
pub fn match_score(name: &str, query: &str) -> Option<u8> {
    if query.is_empty() || name.is_empty() {
        return None;
    }
    if name == query {
        return Some(3);
    }
    let bounded = |idx: usize| {
        let before = idx == 0 || name.as_bytes().get(idx - 1) == Some(&b' ');
        let end = idx + query.len();
        let after = end == name.len() || name.as_bytes().get(end) == Some(&b' ');
        before && after
    };
    if name.starts_with(query) && bounded(0) {
        return Some(2);
    }
    name.match_indices(query)
        .any(|(i, _)| bounded(i))
        .then_some(1)
}

/// The name index of `map-windows.md` §7.2 (without place keys, which are looked up in
/// the table directly).
#[derive(Debug, Clone, PartialEq, Default)]
pub struct PlaceIndex {
    /// Every named place.
    pub entries: Vec<PlaceEntry>,
}

impl PlaceIndex {
    /// Builds the index from the map regions (nodes, dungeons) and their area records.
    /// `render` gives the rendered name of a StringInfo (`None` for unrendered names, which
    /// are left out).
    #[must_use]
    pub fn build(
        regions: &MapRegions,
        render: &dyn Fn(&tessera_formats::StringInfo) -> Option<String>,
    ) -> Self {
        let tree = &regions.tree;
        let mut entries = Vec::new();
        for (i, n) in tree.nodes.iter().enumerate() {
            let Some(name) = n.menu_name.as_ref().and_then(render) else {
                continue;
            };
            entries.push(entry(
                PlaceKind::Node,
                name,
                n.active_element,
                n.region,
                Some(i),
            ));
        }
        // One lookup map over the area records (the first record of a repeated DID wins),
        // so every parent and region step is O(1).
        let mut by_did: HashMap<Did, &AreaRef> = HashMap::with_capacity(regions.areas.len());
        for a in &regions.areas {
            by_did.entry(a.did).or_insert(a);
        }
        let parent = |did: Did| by_did.get(&did).and_then(|a| a.parent);
        // Region numbers of region records; territories → region, areas → territory.
        let region_of_record = |did: Did| -> Option<u32> {
            by_did
                .get(&did)
                .filter(|a| a.level == GeoLevel::Region)
                .and_then(|a| a.region_id)
        };
        // The first node in tree order listing an area of each territory and region record,
        // in one pass over the listed areas.
        let mut first_node: HashMap<(GeoLevel, Did), usize> = HashMap::new();
        for (i, n) in tree.nodes.iter().enumerate() {
            for &a in &n.areas {
                if let Some(t) = parent(a) {
                    first_node.entry((GeoLevel::Territory, t)).or_insert(i);
                    if let Some(r) = parent(t) {
                        first_node.entry((GeoLevel::Region, r)).or_insert(i);
                    }
                }
            }
        }
        // An area, territory or region no node lists shows the root (`map-windows.md` §4.1,
        // §4.6 step 3: an unlisted area maps to `WorldMap`).
        let root = (!tree.nodes.is_empty()).then_some(0);
        for a in &regions.areas {
            let Some(name) = a.name.as_ref().and_then(render) else {
                continue;
            };
            let (kind, region) = match a.level {
                GeoLevel::Area => (
                    PlaceKind::Area,
                    a.parent.and_then(parent).and_then(region_of_record),
                ),
                GeoLevel::Territory => (PlaceKind::Territory, a.parent.and_then(region_of_record)),
                GeoLevel::Region => (PlaceKind::Region, a.region_id),
            };
            let node = match a.level {
                GeoLevel::Area => tree.node_for_area(a.did),
                level => first_node.get(&(level, a.did)).copied(),
            }
            .or(root);
            entries.push(entry(kind, name, a.did.0, region, node));
        }
        for d in &regions.dungeons {
            let Some(name) = d.name.as_ref().and_then(render) else {
                continue;
            };
            let position = d.position.as_ref().and_then(|p| position_location(p).0);
            let mut e = entry(
                PlaceKind::Dungeon,
                name,
                d.did.0,
                position.map(|l| u32::from(l.region)),
                None,
            );
            e.dungeon_map = d.map_data;
            e.position = position;
            entries.push(e);
        }
        Self { entries }
    }

    /// Steps 2 and 3 of `map-windows.md` §7.3: `text` equal to a place key ignoring case
    /// gives that one place; otherwise the names matching `text`, best first (exact, then
    /// prefix at a word boundary, then whole-word substring; ties: `shown_region`, the more
    /// specific kind, the larger place, the lower ID), at most [`MAX_PLACE_RESULTS`].
    #[must_use]
    pub fn lookup(
        &self,
        text: &str,
        places: Option<&PlacesTable>,
        shown_region: Option<u32>,
    ) -> Vec<PlaceMatch> {
        let trimmed: String = text.trim().chars().take(MAX_QUERY_CHARS).collect();
        if trimmed.is_empty() {
            return Vec::new();
        }
        if let Some(t) = places
            && let Some(i) = t.find(&trimmed.encode_utf16().collect::<Vec<u16>>())
            && let Some(p) = t.places.get(i)
        {
            let name = p.name.to_string_lossy();
            let location = position_location(&p.position).0;
            let mut e = entry(
                PlaceKind::PlaceKey,
                name,
                u32::try_from(i).unwrap_or(u32::MAX),
                location.map(|l| u32::from(l.region)),
                None,
            );
            e.position = location;
            return vec![PlaceMatch { entry: e, score: 4 }];
        }
        let query = normalize_name(&trimmed);
        let mut found: Vec<PlaceMatch> = self
            .entries
            .iter()
            .filter_map(|e| {
                match_score(&e.norm, &query).map(|score| PlaceMatch {
                    entry: e.clone(),
                    score,
                })
            })
            .collect();
        found.sort_by(|a, b| rank(a, b, shown_region));
        found.truncate(MAX_PLACE_RESULTS);
        found
    }
}

fn entry(
    kind: PlaceKind,
    name: String,
    id: u32,
    region: Option<u32>,
    node: Option<usize>,
) -> PlaceEntry {
    PlaceEntry {
        kind,
        norm: normalize_name(&name),
        name,
        id,
        region,
        size: 0,
        node,
        dungeon_map: None,
        position: None,
    }
}

fn rank(a: &PlaceMatch, b: &PlaceMatch, shown_region: Option<u32>) -> Ordering {
    let in_region = |m: &PlaceMatch| shown_region.is_some() && m.entry.region == shown_region;
    b.score
        .cmp(&a.score)
        .then_with(|| in_region(b).cmp(&in_region(a)))
        .then_with(|| a.entry.kind.cmp(&b.entry.kind))
        .then_with(|| b.entry.size.cmp(&a.entry.size))
        .then_with(|| a.entry.id.cmp(&b.entry.id))
}
