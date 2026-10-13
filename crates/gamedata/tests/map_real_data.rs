// SPDX-License-Identifier: GPL-3.0-or-later
//! Real-data test of the map models (`map-windows.md` §5, §6.3, §7.3; `scenario-feed.md`
//! §8.3 `map-region`) against a user's own install, read-only.
//!
//! Skipped (reported as passed) unless `TESSERA_GAME_DIR` points at an install. Runs the
//! map-region census over every node and every dungeon map (no failure, every region in
//! one category), resolves the `map-region` family, selects the table notes of every node
//! and of a seeded sample of dungeon maps, and looks up the names of a seeded sample of
//! places. Prints counts and identifiers only, never game text. Use `--release`.

use std::collections::BTreeMap;
use std::path::PathBuf;
use std::sync::Arc;

use tessera_gamedata::map::{
    InstallMapData, MAP_ICONS_LABEL, MapRegions, NoteSettings, PlaceIndex, RegionClass, RegionKind,
    ShownMap, select_notes,
};
use tessera_gamedata::quest::InstallPresentationData;
use tessera_gamedata::tooltip::{TextRef, TooltipData};
use tessera_gamedata::{Attribute, Family, FilterValue, GameDataStore, SelectorQuery};
use tessera_text::{Strings, Values};

fn game_dir() -> Option<PathBuf> {
    let dir = PathBuf::from(std::env::var_os("TESSERA_GAME_DIR")?);
    dir.is_dir().then_some(dir)
}

#[test]
fn map_regions_notes_and_places_over_the_install() {
    let Some(dir) = game_dir() else {
        eprintln!("TESSERA_GAME_DIR not set; skipping the map real-data test");
        return;
    };
    let strings = Arc::new(Strings::open_install(&dir, None).expect("strings open"));
    let store = GameDataStore::open(&dir)
        .expect("install opens")
        .with_strings(Arc::clone(&strings));
    let started = std::time::Instant::now();
    let map_data = InstallMapData::new(&store).expect("map data");
    let regions = MapRegions::load(&map_data);
    let census = regions.census();
    eprintln!(
        "map regions: {} nodes, {} dungeon maps, {} drawn, {} marked, excluded sets {}, \
         layout error {:?}, dungeon error {:?} ({:?})",
        census.nodes,
        census.dungeons,
        census.drawn,
        census.marked,
        census.excluded,
        census.layout_error,
        regions.dungeon_error,
        started.elapsed()
    );
    for (class, n) in &census.by_class {
        eprintln!("  {}: {n}", class.code());
    }
    for (kind, class) in census
        .listed
        .iter()
        .filter(|(_, c)| *c == RegionClass::FrameOutside)
    {
        eprintln!("  C5 {kind:?} {}", class.code());
    }
    for r in regions
        .regions
        .iter()
        .filter(|r| r.detail.is_some())
        .take(8)
    {
        eprintln!("  detail {:?}: {:?}", r.kind, r.detail);
    }
    assert!(census.layout_error.is_none(), "the map tree reads");
    assert!(census.nodes > 0, "the tree has nodes");
    assert_eq!(census.total(), regions.regions.len());
    assert_eq!(
        census.by_class.values().sum::<usize>(),
        census.total(),
        "every region in exactly one class"
    );
    assert!(
        census.failures.is_empty(),
        "failures: {:?}",
        census.failures
    );
    // §5.4 step 5 structurally (the install's counts are 1,525 drawn, 1,523 marked, C5 = 2):
    // no region is undrawable (C1–C4, MWI1–MWI3), every drawn region but C5 is marked, and
    // the root (`WorldMap`) is C5.
    assert!(regions.dungeon_error.is_none(), "the dungeon list reads");
    for class in [
        RegionClass::NoElement,
        RegionClass::NoImage,
        RegionClass::BadImage,
        RegionClass::NoGeoreference,
    ] {
        assert_eq!(census.count(class), 0, "regions in {}", class.code());
    }
    assert!(census.drawn > 0, "maps are drawn");
    assert_eq!(census.drawn, census.total(), "every region is drawn");
    assert!(census.marked > 0, "markers are placed");
    assert_eq!(
        census.marked,
        census.drawn - census.count(RegionClass::FrameOutside),
        "every drawn region but C5 is marked"
    );
    assert_eq!(
        regions.regions.first().map(|r| r.class),
        Some(RegionClass::FrameOutside),
        "the root map's frame lies outside the world"
    );
    // MWI10: each region exactly once.
    let mut seen = std::collections::BTreeSet::new();
    for r in &regions.regions {
        assert!(seen.insert(r.kind), "region listed twice: {:?}", r.kind);
    }

    // The map-region family through the store (scenario-feed.md §8.3).
    let all = store
        .resolve(&SelectorQuery::all(Family::MapRegion), &[])
        .expect("resolves");
    let dungeons = store
        .resolve(
            &SelectorQuery::all(Family::MapRegion).filter(
                Attribute::Kind,
                FilterValue::Keywords(vec!["dungeon".into()]),
            ),
            &[],
        )
        .expect("resolves");
    let region1 = store
        .resolve(
            &SelectorQuery::all(Family::MapRegion)
                .filter(Attribute::Region, FilterValue::Integers(vec![1])),
            &[],
        )
        .expect("resolves");
    eprintln!(
        "map-region selector: all {}, dungeon {}, region 1 {}",
        all.records.len(),
        dungeons.records.len(),
        region1.records.len()
    );
    assert_eq!(all.records.len(), regions.regions.len());
    assert_eq!(dungeons.records.len(), census.dungeons);
    let named = all
        .records
        .iter()
        .filter(|c| store.display_name(c.did, Some(c.row)).is_some())
        .count();
    eprintln!("map-region display names: {named} of {}", all.records.len());

    // Table notes of every node and of the dungeon maps (map-windows.md §6.3).
    let data = InstallPresentationData::new(&store, Arc::clone(&strings)).expect("data");
    let render = |info: &tessera_formats::StringInfo| -> Option<String> {
        data.render(TextRef::Info(info), &Values::new())
            .ok()
            .flatten()
            .map(|t| t.plain_text())
    };
    let icons = map_data.icon_table(MAP_ICONS_LABEL);
    eprintln!(
        "map icon table: {} entries",
        icons.as_ref().map_or(0, |i| i.entries.len())
    );
    let notes = store.map_notes().expect("map notes table");
    let index = notes.index();
    let settings = NoteSettings {
        layer: 0,
        discovery_all: true,
        filter: u64::MAX,
    };
    let pids = *tessera_gamedata::map::MapData::pids(&map_data);
    let mut per_node = 0usize;
    let mut visible = 0usize;
    let mut texts = 0usize;
    let mut no_icon = 0usize;
    for (i, r) in regions.regions.iter().enumerate() {
        let shown = match r.kind {
            RegionKind::Node { index, .. } => ShownMap::Node(index),
            RegionKind::Dungeon { .. } if i % 10 == 0 => ShownMap::Dungeon {
                zone: r.dungeons.first().copied().unwrap_or_default(),
                georef: r.georef,
                frame: r.frame.unwrap_or((0, 0)),
            },
            RegionKind::Dungeon { .. } => continue,
        };
        let sel = select_notes(
            &notes,
            &index,
            &regions.tree,
            shown,
            r.frame.unwrap_or((0, 0)),
            &settings,
            &pids,
            &render,
        );
        per_node += sel.len();
        for n in &sel {
            if n.visible && !n.world_event_hidden {
                visible += 1;
            }
            if n.text.is_some() {
                texts += 1;
            }
            if !n.link && icons.as_ref().is_some_and(|t| t.get(n.type_bits).is_none()) {
                no_icon += 1;
            }
        }
    }
    eprintln!(
        "notes selected: {per_node} ({visible} visible, {texts} with text, {no_icon} without icon)"
    );

    // Place lookup (map-windows.md §7.3).
    let places = PlaceIndex::build(&regions, &render);
    let mut by_kind: BTreeMap<String, usize> = BTreeMap::new();
    for e in &places.entries {
        *by_kind.entry(e.kind.word().to_owned()).or_default() += 1;
    }
    eprintln!("place index: {by_kind:?}");
    let table = store.places();
    let mut found = 0usize;
    let probes: Vec<_> = places.entries.iter().step_by(37).take(64).collect();
    for e in &probes {
        let hits = places.lookup(&e.name, table.as_deref(), e.region);
        assert!(hits.len() <= 10);
        if hits.iter().any(|h| h.entry.norm == e.norm) {
            found += 1;
        }
    }
    eprintln!("place lookups: {found} of {} names found", probes.len());
    assert_eq!(found, probes.len(), "every indexed name finds itself");
    if let Some(t) = &table
        && let Some(p) = t.places.get(t.places.len() / 2)
    {
        let key = p.name.to_string_lossy().to_uppercase();
        let hit = places.lookup(&key, Some(t), None);
        assert_eq!(hit.len(), 1, "a place key matches ignoring case");
    }
    eprintln!("map real-data test took {:?}", started.elapsed());
}
