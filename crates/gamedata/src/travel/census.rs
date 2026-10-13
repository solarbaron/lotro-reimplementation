// SPDX-License-Identifier: GPL-3.0-or-later
//! The census of `travel-windows.md` §8.1: every travel destination in one category, and
//! the counts of the stations' rows.

use std::collections::{BTreeMap, BTreeSet};

use tessera_formats::gameplay::common::PropertyAccess;
use tessera_formats::gameplay::map::{
    MapKind, MapPids, MapWeenieKeys, TravelDestination, TravelRoute, TravelWeb, classify_set,
};
use tessera_formats::property::PropertyValue;
use tessera_formats::{Did, PropertySet};
use tessera_props::RecordSource;

use crate::error::Result;
use crate::store::GameDataStore;

/// Deepest nesting searched for DIDs in a reference list (a Tessera bound).
const MAX_DID_DEPTH: usize = 4;

/// The category of a travel destination (`travel-windows.md` §8.1), tried in this order.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, PartialOrd, Ord)]
pub enum DestinationCategory {
    /// The destination of a route some station sells: a row of some travel window.
    Listed,
    /// No station sells a route to it; it is some station's `TravelHomeLocation`.
    HomeOnly,
    /// Only routes no station sells lead to it.
    UnsoldRouteOnly,
    /// Named only by `TravelVersioning`, `Travel_PlayerHomeLocationArray` or
    /// `TravelHomeLocationsArray`.
    OtherReference,
    /// Named by none of the above.
    Unreferenced,
    /// Not a destination, or it fails to decode.
    RecordUnreadable,
}

/// Counts of the stations' rows (§8.1 "Station rows").
#[derive(Debug, Clone, Default, PartialEq, Eq)]
pub struct StationRows {
    /// Stations (travel-web records).
    pub stations: usize,
    /// Rows over all stations.
    pub rows: usize,
    /// Fewest and most rows of one station.
    pub per_station: (usize, usize),
    /// Rows whose route leads to a swift destination.
    pub swift: usize,
    /// Rows whose route leads to a slow destination.
    pub slow: usize,
    /// Rows whose route's destination is no destination record (none expected).
    pub bad_destination: usize,
    /// Rows naming a route that is no route record (none expected).
    pub bad_route: usize,
    /// Rows costing 0 (shown as free).
    pub free: usize,
    /// Rows leading to the station's own home.
    pub to_own_home: usize,
    /// Rows whose route has `Usage_MinLevel`.
    pub with_min_level: usize,
    /// Rows whose route has `Usage_MinLevel` below 2.
    pub min_level_below_two: usize,
    /// Rows whose route carries quest requirements.
    pub quest_requirements: usize,
    /// Rows whose route carries a faction requirement.
    pub faction_requirement: usize,
    /// Stations listing one destination twice.
    pub repeated_destinations: usize,
}

/// The census of §8.1.
#[derive(Debug, Clone, Default, PartialEq, Eq)]
pub struct TravelCensus {
    /// Destinations per category, with (swift, slow) counts.
    pub destinations: BTreeMap<DestinationCategory, (usize, usize)>,
    /// The destinations of the categories other than `Listed` (at most 256).
    pub listed: Vec<(Did, DestinationCategory)>,
    /// Station rows.
    pub rows: StationRows,
    /// Route records.
    pub routes: usize,
    /// Routes no station sells.
    pub unsold_routes: Vec<Did>,
    /// Destinations whose `TravelDisplayName` is not a table reference.
    pub names_not_table: usize,
    /// Object records the scan could not read.
    pub unreadable_records: usize,
}

impl TravelCensus {
    /// Destinations of `category`.
    #[must_use]
    pub fn count(&self, category: DestinationCategory) -> usize {
        self.destinations.get(&category).map_or(0, |(a, b)| a + b)
    }

    /// Destinations in all.
    #[must_use]
    pub fn total(&self) -> usize {
        self.destinations.values().map(|(a, b)| a + b).sum()
    }
}

/// What the scan keeps of one record.
#[derive(Debug, Clone)]
enum Rec {
    Web(TravelWeb),
    Destination(Option<TravelDestination>),
    Route(Option<TravelRoute>),
    References(Vec<Did>),
}

fn dids_in(v: &PropertyValue, depth: usize, out: &mut Vec<Did>) {
    if depth > MAX_DID_DEPTH {
        return;
    }
    match v {
        PropertyValue::DidRef(d) if !d.is_null() => out.push(*d),
        PropertyValue::Array(a) => {
            for e in &a.elements {
                dids_in(&e.value, depth + 1, out);
            }
        }
        PropertyValue::Struct(s) => {
            for e in &s.fields {
                dids_in(&e.value, depth + 1, out);
            }
        }
        _ => {}
    }
}

fn scan_record(set: &PropertySet, p: &MapPids) -> Option<Rec> {
    match classify_set(set, p, &MapWeenieKeys::default()) {
        Some(MapKind::TravelWeb) => TravelWeb::read(set, p).ok().map(Rec::Web),
        Some(MapKind::TravelDestination) => {
            Some(Rec::Destination(TravelDestination::read(set, p).ok()))
        }
        Some(MapKind::TravelRoute) => Some(Rec::Route(TravelRoute::read(set, p).ok())),
        Some(MapKind::TravelVersioning) => {
            let mut out = Vec::new();
            if let Some(v) = set.get_pid(p.versioning_pairs) {
                dids_in(v, 0, &mut out);
            }
            Some(Rec::References(out))
        }
        _ => {
            let v = set.get_pid(p.player_home_locations)?;
            let mut out = Vec::new();
            dids_in(v, 0, &mut out);
            Some(Rec::References(out))
        }
    }
}

/// The census of every travel destination and station row (`travel-windows.md` §8.1),
/// from one scan of every object record (seconds in a release build).
///
/// # Errors
///
/// When the dictionary cannot be read or a scan worker fails.
pub fn travel_census<S: RecordSource>(store: &GameDataStore<S>) -> Result<TravelCensus> {
    let registry = store.object_store().game_data().registry()?;
    let p = MapPids::resolve(registry.dictionary());
    let scan = store.scan_objects(|_, set| scan_record(set, &p))?;
    let mut webs: Vec<(Did, TravelWeb)> = Vec::new();
    let mut dests: BTreeMap<Did, Option<TravelDestination>> = BTreeMap::new();
    let mut routes: BTreeMap<Did, Option<TravelRoute>> = BTreeMap::new();
    let mut references: BTreeSet<Did> = BTreeSet::new();
    for (did, rec) in scan.found {
        match rec {
            Rec::Web(w) => webs.push((did, w)),
            Rec::Destination(d) => {
                dests.insert(did, d);
            }
            Rec::Route(r) => {
                routes.insert(did, r);
            }
            Rec::References(v) => references.extend(v),
        }
    }
    let mut c = TravelCensus {
        routes: routes.len(),
        unreadable_records: scan.failed,
        ..TravelCensus::default()
    };
    let route_dest = |r: Did| {
        routes
            .get(&r)
            .and_then(|x| x.as_ref())
            .map(|x| x.destination)
    };
    let mut sold_routes = BTreeSet::new();
    let mut listed = BTreeSet::new();
    let mut homes = BTreeSet::new();
    let mut per = (usize::MAX, 0usize);
    for (_, w) in &webs {
        homes.insert(w.home);
        references.extend(w.home_aliases.iter().copied());
        per = (per.0.min(w.offers.len()), per.1.max(w.offers.len()));
        let mut seen = BTreeSet::new();
        let mut repeated = false;
        for o in &w.offers {
            c.rows.rows += 1;
            sold_routes.insert(o.route);
            let Some(route) = routes.get(&o.route).and_then(Option::as_ref) else {
                c.rows.bad_route += 1;
                continue;
            };
            let d = route.destination;
            listed.insert(d);
            if !seen.insert(d) {
                repeated = true;
            }
            match dests.get(&d).and_then(Option::as_ref) {
                Some(x) if x.is_swift => c.rows.swift += 1,
                Some(_) => c.rows.slow += 1,
                None => c.rows.bad_destination += 1,
            }
            if o.cost == Some(0) {
                c.rows.free += 1;
            }
            if d == w.home {
                c.rows.to_own_home += 1;
            }
            if let Some(l) = route.min_level {
                c.rows.with_min_level += 1;
                if l < 2 {
                    c.rows.min_level_below_two += 1;
                }
            }
            if !route.quest_requirements.is_empty() {
                c.rows.quest_requirements += 1;
            }
            if route.required_faction.is_some() {
                c.rows.faction_requirement += 1;
            }
        }
        if repeated {
            c.rows.repeated_destinations += 1;
        }
    }
    c.rows.stations = webs.len();
    c.rows.per_station = if webs.is_empty() { (0, 0) } else { per };
    let unsold: BTreeSet<Did> = routes
        .keys()
        .copied()
        .filter(|r| !sold_routes.contains(r))
        .collect();
    c.unsold_routes = unsold.iter().copied().collect();
    let unsold_dests: BTreeSet<Did> = unsold.iter().filter_map(|&r| route_dest(r)).collect();
    // Only the destination's own DID counts as a reference (a route named there does not
    // name its destination).
    let reference_dests = &references;
    for (&did, d) in &dests {
        let category = match d {
            None => DestinationCategory::RecordUnreadable,
            Some(_) if listed.contains(&did) => DestinationCategory::Listed,
            Some(_) if homes.contains(&did) => DestinationCategory::HomeOnly,
            Some(_) if unsold_dests.contains(&did) => DestinationCategory::UnsoldRouteOnly,
            Some(_) if reference_dests.contains(&did) => DestinationCategory::OtherReference,
            Some(_) => DestinationCategory::Unreferenced,
        };
        let e = c.destinations.entry(category).or_default();
        if d.as_ref().is_some_and(|x| x.is_swift) {
            e.0 += 1;
        } else {
            e.1 += 1;
        }
        if d.as_ref().is_some_and(|x| {
            x.display_name
                .as_ref()
                .is_none_or(|n| !matches!(n.source, tessera_formats::StringSource::Table { .. }))
        }) {
            c.names_not_table += 1;
        }
        if category != DestinationCategory::Listed && c.listed.len() < 256 {
            c.listed.push((did, category));
        }
    }
    Ok(c)
}
