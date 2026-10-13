// SPDX-License-Identifier: GPL-3.0-or-later
//! Real-data test of the travel model (`travel-windows.md` §4.10, §8.1) against a user's own
//! install, read-only.
//!
//! Skipped (reported as passed) unless `TESSERA_GAME_DIR` points at an install. Runs the
//! destination census over every travel destination and station row (every destination in
//! exactly one category, no row failure) and builds a travel window over every travel
//! destination of the install (rows in entry order, names, grouping, selection display).
//! Prints counts and identifiers only, never game text. Use `--release`.

use std::path::PathBuf;
use std::sync::Arc;

use tessera_gamedata::quest::{InstallPresentationData, RecordRef};
use tessera_gamedata::travel::{
    DestinationCategory, TravelEntryIn, TravelFlags, TravelPresenter, travel_census,
};
use tessera_gamedata::{Family, GameDataStore};
use tessera_text::Strings;

fn game_dir() -> Option<PathBuf> {
    let dir = PathBuf::from(std::env::var_os("TESSERA_GAME_DIR")?);
    dir.is_dir().then_some(dir)
}

#[test]
fn travel_census_and_window_over_the_install() {
    let Some(dir) = game_dir() else {
        eprintln!("TESSERA_GAME_DIR not set; skipping the travel real-data test");
        return;
    };
    let started = std::time::Instant::now();
    let store = GameDataStore::open(&dir).expect("install opens");
    let c = travel_census(&store).expect("census runs");
    eprintln!("travel census ({:?}):", started.elapsed());
    for (cat, (swift, slow)) in &c.destinations {
        eprintln!("  {cat:?}: {} (swift {swift}, slow {slow})", swift + slow);
    }
    for (did, cat) in c.listed.iter().take(16) {
        eprintln!("  {did} {cat:?}");
    }
    let r = &c.rows;
    eprintln!(
        "  stations {}, rows {} ({}..{} per station), swift {}, slow {}, free {}, own home {}, \
         with level {} (below 2: {}), quest req {}, faction req {}, repeated {}, bad route {}, \
         bad destination {}",
        r.stations,
        r.rows,
        r.per_station.0,
        r.per_station.1,
        r.swift,
        r.slow,
        r.free,
        r.to_own_home,
        r.with_min_level,
        r.min_level_below_two,
        r.quest_requirements,
        r.faction_requirement,
        r.repeated_destinations,
        r.bad_route,
        r.bad_destination
    );
    eprintln!(
        "  routes {}, unsold {:?}, names not table {}, unreadable records {}",
        c.routes, c.unsold_routes, c.names_not_table, c.unreadable_records
    );
    let destinations = store.family(Family::TravelDestination).expect("family");
    assert_eq!(
        c.total(),
        destinations.len(),
        "every destination counted once"
    );
    assert_eq!(c.count(DestinationCategory::RecordUnreadable), 0);
    assert_eq!(r.bad_route + r.bad_destination, 0, "every row resolves");
    assert!(c.count(DestinationCategory::Listed) > 0);

    // A window over every destination (§4.10).
    let strings = Arc::new(Strings::open_install(&dir, None).expect("strings open"));
    let data = InstallPresentationData::new(&store, strings).expect("data");
    let tp = TravelPresenter::new(&data);
    let entries: Vec<TravelEntryIn> = destinations
        .iter()
        .take(1_024)
        .enumerate()
        .map(|(i, &d)| TravelEntryIn {
            entry: u16::try_from(i + 1).unwrap_or(u16::MAX),
            destination: RecordRef::Record(d),
            coin: (i as u64 % 3) * 250,
            min_level: (i % 2 == 0).then_some(40),
            flags: TravelFlags(u8::try_from(i % 8).unwrap_or(0)),
            route: None,
            area: None,
        })
        .collect();
    let w = tp.window(Some("Groom"), &entries, 35);
    let named = w.rows.iter().filter(|r| r.name.is_some()).count();
    let swift = w.rows.iter().filter(|r| r.swift).count();
    eprintln!(
        "travel window: {} rows, {named} named, {swift} swift, {} groups, title {}",
        w.rows.len(),
        w.groups.len(),
        w.title.is_some()
    );
    assert_eq!(w.rows.len(), entries.len());
    assert_eq!(named, w.rows.len(), "every destination has a name");
    let sel = w.selection(Some(0), &tp);
    assert!(sel.free != sel.cost.is_some(), "TI2");
    assert!(sel.min_level_text.is_some(), "the level text renders");
    assert!(w.title.is_some(), "the title renders");
}
