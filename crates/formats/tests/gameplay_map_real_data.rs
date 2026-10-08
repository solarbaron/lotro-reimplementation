// SPDX-License-Identifier: GPL-3.0-or-later
//! Real-data checks of the map-data decoders and views (`docs/specs/map-data.md`) against a
//! user's own install, read-only.
//!
//! Skipped unless `TESSERA_GAME_DIR` points at an install. Every record of the family is
//! read: the three singleton tables in full (§3–§5) and every stand-alone and object
//! property set (0x78 and 0x79), classified by the kinds of §2.3. The invariants of §12
//! (MI1–MI15) are asserted; the SegmentNode half of MI11 and the landblock-info edges of §2.4
//! walk every landblock-info record.
//!
//! MI14 coverage: every §2.4 edge whose target is a kind of §2.3 (or a 0x78 set holding
//! `UI_Map_GameMap`) is asserted by the target's properties. The edges whose target §2.4 names
//! only by class index (scene records, effects, trait records and the entity slot of map-note
//! rows) are checked for existence, and their root-class histograms are census counts,
//! because the tests never hard-code a class index of one install. The SegmentNode edge is
//! MI11.
//!
//! The census counts of §3.5, §4.5, §5.3 and §7–§11 are observations of one install: they
//! are compared and printed, and asserted only when `TESSERA_STRICT_CENSUS=1`. No game text
//! is printed.

use std::collections::{BTreeMap, BTreeSet, HashMap};
use std::path::PathBuf;
use std::sync::Mutex;

use tessera_dat::{ArchiveKind, Entry};
use tessera_formats::gameplay::common::{
    ObjectRoot, Pid, PropertyAccess, UNIQUEDB_LABEL, ValueExt,
};
use tessera_formats::gameplay::map::{
    DUNGEON_QUEST_NOTES_LABEL, House, HouseList, HousingHook, MAP_NOTES_LABEL, MapKind,
    MapNotesTable, MapPids, NEIGHBORHOOD_DIRECTORY_LABEL, Neighbourhood, NeighbourhoodGate,
    NeighbourhoodTemplate, NoteFiling, PLACES_TABLE_LABEL, PlaceIssue, PlacesTable, PositionValues,
    QuestNoteInfo, QuestNotesTable, RIPCORDS_LABEL, RouteAction, StableMaster,
    TRAVEL_WEB_DIRECTORY_LABEL, TravelDestination, TravelHop, TravelMode, TravelPad, TravelRoute,
    TravelSegment, TravelVersioning, TravelWeb, classify_set, for_each_place_reference,
    is_null_text, neighbourhood_directory, position_flags_consistent, position_is_valid,
    route_pattern, text_is_valid, travel_directory,
};
use tessera_formats::landblock::{LandblockInfo, StaticEntityType};
use tessera_formats::property_set::{EntryContext, object_property_set_did};
use tessera_formats::{
    Did, PropertyDictionary, PropertySet, PropertySetRecord, Record, StringSource, StringTable,
};
use tessera_wstate::ObjectStore;

fn game_dir() -> Option<PathBuf> {
    let dir = PathBuf::from(std::env::var_os("TESSERA_GAME_DIR")?);
    dir.is_dir().then_some(dir)
}

fn strict() -> bool {
    std::env::var_os("TESSERA_STRICT_CENSUS").is_some_and(|v| v == "1")
}

/// Census counts of one install: compared, printed, asserted only in strict mode.
#[derive(Default)]
struct Census {
    name: &'static str,
    matched: usize,
    differ: Vec<String>,
}

impl Census {
    fn new(name: &'static str) -> Self {
        Self {
            name,
            ..Self::default()
        }
    }

    fn eq<T: PartialEq + std::fmt::Debug>(&mut self, what: &str, spec: T, found: T) {
        if spec == found {
            self.matched += 1;
        } else {
            self.differ
                .push(format!("{what}: spec {spec:?}, found {found:?}"));
        }
    }

    fn finish(self) {
        eprintln!(
            "{} census: {} of {} counts match the spec",
            self.name,
            self.matched,
            self.matched + self.differ.len()
        );
        for d in &self.differ {
            eprintln!("  differs: {d}");
        }
        if strict() {
            assert!(self.differ.is_empty(), "{} census differs", self.name);
        }
    }
}

fn hist<K: Ord>(it: impl IntoIterator<Item = K>) -> BTreeMap<K, usize> {
    let mut m = BTreeMap::new();
    for k in it {
        *m.entry(k).or_default() += 1;
    }
    m
}

fn sorted_desc(h: &BTreeMap<impl Copy + Ord, usize>) -> Vec<usize> {
    let mut v: Vec<usize> = h.values().copied().collect();
    v.sort_unstable_by(|a, b| b.cmp(a));
    v
}

struct Env {
    store: ObjectStore,
    dict: PropertyDictionary,
    pids: MapPids,
}

impl Env {
    fn open() -> Option<Self> {
        let dir = game_dir()?;
        let store = ObjectStore::open(&dir).expect("open install");
        let dict = store
            .game_data()
            .registry()
            .expect("dictionary")
            .dictionary()
            .clone();
        let pids = MapPids::resolve(&dict);
        assert!(
            pids.missing().is_empty(),
            "undefined labels: {:?}",
            pids.missing()
        );
        Some(Self { store, dict, pids })
    }

    fn read(&self, did: Did) -> Option<Vec<u8>> {
        self.store.game_data().read(did).expect("read")
    }

    fn unique(&self, label: &str) -> Did {
        self.store
            .game_data()
            .lookup_path(&[UNIQUEDB_LABEL, label])
            .expect("UNIQUEDB")
            .unwrap_or_else(|| panic!("UNIQUEDB has no {label}"))
    }

    fn object_set(&self, object: Did) -> Option<PropertySet> {
        let did = object_property_set_did(object)?;
        let payload = self.read(did)?;
        Some(
            PropertySetRecord::parse(&payload, &self.dict)
                .expect("object property set")
                .set,
        )
    }

    /// The root class of 0x70 record `did` (`wstate.md` §3.1), `None` when it does not exist.
    fn root_class(&self, did: Did) -> Option<u32> {
        let payload = self.read(did)?;
        Some(
            ObjectRoot::read_expecting(&payload, did)
                .expect("object record")
                .root_class,
        )
    }

    /// Root-class histogram of `targets` (MI14 edges named by class); every target exists.
    fn classes_of(&self, what: &str, targets: &[Did]) -> Vec<(u32, usize)> {
        hist(targets.iter().map(|&d| {
            self.root_class(d)
                .unwrap_or_else(|| panic!("MI14: {what} target {d} does not exist"))
        }))
        .into_iter()
        .collect()
    }

    fn zone_kind(&self, cache: &mut HashMap<Did, Option<MapKind>>, did: Did) -> Option<MapKind> {
        *cache.entry(did).or_insert_with(|| {
            self.object_set(did)
                .and_then(|s| classify_set(&s, &self.pids))
                .filter(|k| matches!(k, MapKind::Area | MapKind::Dungeon))
        })
    }
}

/// String-table lookups for the text test of §4.4.2, cached per table.
struct Strings<'e> {
    env: &'e Env,
    tables: Mutex<HashMap<Did, Option<StringTable>>>,
}

impl Strings<'_> {
    fn with<T>(&self, table: Did, f: impl FnOnce(Option<&StringTable>) -> T) -> T {
        let mut tables = self.tables.lock().unwrap();
        let t = tables.entry(table).or_insert_with(|| {
            self.env
                .read(table)
                .and_then(|p| StringTable::parse(&p).ok())
        });
        f(t.as_ref())
    }

    fn has(&self, table: Did, token: u32) -> bool {
        self.with(table, |t| t.is_some_and(|t| t.get(token).is_some()))
    }

    fn is_empty(&self, table: Did, token: u32) -> bool {
        self.with(table, |t| {
            t.and_then(|t| t.get(token))
                .is_some_and(|e| e.is_empty_text())
        })
    }
}

// ---------------------------------------------------------------------------------------------
// §3–§5 The three singleton tables
// ---------------------------------------------------------------------------------------------

#[test]
fn singleton_tables() {
    let Some(env) = Env::open() else {
        eprintln!("TESSERA_GAME_DIR not set; skipping");
        return;
    };
    let mut zones = HashMap::new();
    let places = places_table(&env, &mut zones);
    map_notes(&env, &mut zones);
    quest_notes(&env, &mut zones);
    eprintln!("places table: {} entries", places.len());
}

fn places_table(env: &Env, zones: &mut HashMap<Did, Option<MapKind>>) -> PlacesTable {
    let did = env.unique(PLACES_TABLE_LABEL);
    let payload = env.read(did).expect("places table exists");
    // MI1: decodes to its last byte.
    let t = PlacesTable::parse_expecting(&payload, did).expect("places table decodes");

    // MI2, MI3 (equality), MI4, MI15.
    let issues = t.issues();
    let ascii_only = t.iter().all(|p| p.name.units().iter().all(|&u| u < 0x80));
    for i in &issues {
        match i {
            PlaceIssue::OutOfBucketOrder { non_ascii, .. } => {
                assert!(*non_ascii, "MI2: ASCII key out of bucket order: {i:?}");
            }
            other => panic!("places table: {other:?}"),
        }
    }
    if ascii_only {
        assert_eq!(t.is_bucket_ordered(), Some(true), "MI2");
    }
    // Every key finds its own entry.
    for (i, p) in t.iter().enumerate() {
        assert_eq!(t.find(p.name.units()), Some(i), "MI2: lookup of entry {i}");
    }
    // MI3: zone slots are 0 or a zone record.
    let mut by_slot: BTreeMap<(bool, &str), usize> = BTreeMap::new();
    for p in t.iter() {
        let kind = if p.area.is_null() {
            "none"
        } else {
            match env.zone_kind(zones, p.area) {
                Some(MapKind::Area) => "area",
                Some(MapKind::Dungeon) => "dungeon",
                other => panic!("MI3: place zone {} is {other:?}", p.area),
            }
        };
        let indoor = PositionValues::of(&p.position).is_interior();
        *by_slot.entry((indoor, kind)).or_default() += 1;
    }

    let mut c = Census::new("places (§3.5)");
    c.eq("entries", 9_025, t.len());
    c.eq("hint", 10, t.size.hint);
    let flags = hist(t.iter().map(|p| p.position.flags()));
    c.eq(
        "flag bytes",
        vec![
            (0x1B, 312),
            (0x3B, 1_099),
            (0x5B, 256),
            (0x7B, 663),
            (0x9B, 758),
            (0xBB, 3_574),
            (0xDB, 348),
            (0xFB, 2_015),
        ],
        flags.into_iter().collect::<Vec<_>>(),
    );
    let regions = hist(t.iter().map(|p| PositionValues::of(&p.position).region));
    c.eq(
        "regions",
        vec![
            (1, 3_282),
            (2, 2_917),
            (3, 668),
            (4, 944),
            (5, 1_039),
            (6, 109),
            (14, 63),
            (15, 3),
        ],
        regions.into_iter().collect::<Vec<_>>(),
    );
    c.eq(
        "with orientation",
        7_351,
        t.iter()
            .filter(|p| p.position.orientation().is_some())
            .count(),
    );
    c.eq(
        "interior",
        2_330,
        t.iter()
            .filter(|p| PositionValues::of(&p.position).is_interior())
            .count(),
    );
    let units: usize = t.iter().map(|p| p.name.len()).sum();
    c.eq("code units", 296_009, units);
    c.eq(
        "names with 0x1F",
        5,
        t.iter().filter(|p| p.name.units().contains(&0x1F)).count(),
    );
    c.eq(
        "names with upper case",
        168,
        t.iter()
            .filter(|p| p.name.units().iter().any(|u| (0x41..=0x5A).contains(u)))
            .count(),
    );
    c.eq(
        "zone slots (indoor, kind)",
        vec![
            ((false, "area"), 40),
            ((false, "none"), 6_655),
            ((true, "area"), 78),
            ((true, "dungeon"), 2_071),
            ((true, "none"), 181),
        ],
        by_slot.into_iter().collect::<Vec<_>>(),
    );
    c.finish();
    t
}

fn map_notes(env: &Env, zones: &mut HashMap<Did, Option<MapKind>>) {
    let did = env.unique(MAP_NOTES_LABEL);
    let payload = env.read(did).expect("map-note table exists");
    // MI1, MI5.
    let t = MapNotesTable::parse_expecting(&payload, did, &env.dict).expect("map notes decode");
    let strings = Strings {
        env,
        tables: Mutex::new(HashMap::new()),
    };
    let layers_pid = env.pids.content_layers.id();

    let mut empty_texts = 0usize;
    for (i, n) in t.iter().enumerate() {
        // MI4, MI15.
        assert!(position_flags_consistent(&n.base.position), "MI4: row {i}");
        assert!(position_is_valid(&n.base.position), "MI15: row {i}");
        // MI6 by construction; MI7.
        assert_eq!(n.link.is_some(), n.is_link(), "MI6: row {i}");
        assert!(
            n.type_bit().is_some(),
            "MI7: row {i} type {:#x}",
            n.note_type
        );
        if let Some(link) = &n.link {
            assert_eq!(n.note_type, 1, "MI7: row {i}");
            assert!(is_null_text(&n.text), "MI7: row {i}");
            assert_eq!(
                (n.level, n.min_range, n.max_range, n.discovery),
                (0, 0.0, 0.0, 0),
                "MI7: row {i}"
            );
            assert!(position_flags_consistent(&link.position), "MI4: link {i}");
            assert!(position_is_valid(&link.position), "MI15: link {i}");
        } else {
            // §4.5: references only, no arguments.
            assert!(
                matches!(n.text.source, StringSource::Table { .. }) && n.text.args.is_none(),
                "row {i}: text form"
            );
            if let StringSource::Table { token, table } = n.text.source
                && strings.is_empty(table, token)
            {
                empty_texts += 1;
            }
        }
        // MI8.
        assert_eq!(
            n.validity(&|t, k| strings.has(t, k)),
            Ok(()),
            "MI8: row {i}"
        );
        assert!(!n.base.area.is_null(), "MI8: row {i} area");
        let v = n.base.position_values();
        if v.cell == 0 {
            assert!(
                n.base.dungeon.is_null(),
                "MI8: outdoor row {i} names a dungeon"
            );
        }
        // MI9.
        assert_eq!(n.base.layers_pid(), layers_pid, "MI9: row {i}");
        assert!(!n.base.layer_keys().contains(&0), "MI9: row {i}");
        assert!(!n.flag_b, "row {i}: flag_b");
    }
    // With every lookup failing only literals pass the text test, and there are none.
    let literals = t
        .iter()
        .filter(|n| text_is_valid(&n.text, &|_: Did, _: u32| false))
        .count();
    assert_eq!(literals, 0, "§4.5: no literal texts");
    let links = t.iter().filter(|n| n.is_link()).count();

    let ix = t.index();
    let filed = ix.by_dungeon.values().map(Vec::len).sum::<usize>()
        + ix.by_landblock.values().map(Vec::len).sum::<usize>()
        + ix.ranged.len();
    assert_eq!(filed, t.len(), "§4.6: every row filed once");

    let mut c = Census::new("map notes (§4.5)");
    c.eq("rows", 13_485, t.len());
    c.eq("link rows", 2_754, links);
    c.eq(
        "flag bytes",
        vec![
            (0x1B, 247),
            (0x3B, 1_218),
            (0x5B, 200),
            (0x7B, 1_018),
            (0x9B, 2_657),
            (0xBB, 4_552),
            (0xDB, 1_342),
            (0xFB, 2_251),
        ],
        hist(t.iter().map(|n| n.base.position.flags()))
            .into_iter()
            .collect::<Vec<_>>(),
    );
    c.eq(
        "regions",
        vec![
            (1, 4_811),
            (2, 4_119),
            (3, 1_436),
            (4, 1_731),
            (5, 1_314),
            (6, 6),
            (14, 68),
        ],
        hist(t.iter().map(|n| n.base.position_values().region))
            .into_iter()
            .collect::<Vec<_>>(),
    );
    let mut area_kinds = BTreeMap::new();
    for n in t.iter() {
        *area_kinds
            .entry(format!("{:?}", env.zone_kind(zones, n.base.area)))
            .or_insert(0usize) += 1;
    }
    c.eq(
        "area kinds",
        vec![
            ("Some(Area)".to_owned(), 13_411),
            ("Some(Dungeon)".to_owned(), 74),
        ],
        area_kinds.into_iter().collect::<Vec<_>>(),
    );
    let indoor: Vec<_> = t
        .iter()
        .filter(|n| n.base.position_values().cell != 0)
        .collect();
    c.eq("indoor rows", 2_683, indoor.len());
    c.eq(
        "indoor rows naming a dungeon",
        2_581,
        indoor.iter().filter(|n| !n.base.dungeon.is_null()).count(),
    );
    let entities = hist(t.iter().map(|n| n.base.entity));
    c.eq("distinct entities", 5_698, entities.len());
    c.eq(
        "entities in more than one row",
        1_031,
        entities.values().filter(|&&k| k > 1).count(),
    );
    // MI14: the entity slot names an existing record; its root classes are a census.
    let mut entity_classes: BTreeMap<u32, usize> = BTreeMap::new();
    for (&e, &rows) in entities.iter().filter(|(e, _)| !e.is_null()) {
        let class = env
            .root_class(e)
            .unwrap_or_else(|| panic!("MI14: entity {e} does not exist"));
        *entity_classes.entry(class).or_default() += rows;
    }
    c.eq(
        "entity classes (class, rows)",
        vec![
            (796, 1_234),
            (804, 473),
            (815, 1_674),
            (1170, 320),
            (1210, 3_217),
            (1722, 1_080),
            (1724, 5_487),
        ],
        entity_classes.into_iter().collect::<Vec<_>>(),
    );
    c.eq(
        "layer list lengths",
        vec![(0, 10_844), (1, 2_515), (2, 117), (3, 7), (5, 2)],
        hist(t.iter().map(|n| n.base.layer_keys().len()))
            .into_iter()
            .collect::<Vec<_>>(),
    );
    let elements: Vec<u32> = t.iter().flat_map(|n| n.base.layer_keys()).collect();
    c.eq(
        "layer elements (1, 2, named)",
        (1_574, 202, 1_004),
        (
            elements.iter().filter(|&&k| k == 1).count(),
            elements.iter().filter(|&&k| k == 2).count(),
            elements.iter().filter(|&&k| k > 2).count(),
        ),
    );
    c.eq(
        "non-zero icons",
        11,
        t.iter().filter(|n| !n.icon.is_null()).count(),
    );
    c.eq(
        "levels",
        vec![
            (0, 2_754),
            (0x10000, 7_501),
            (0x20000, 14),
            (0xA0000, 4),
            (0xE0000, 9),
            (0xF0000, 3_203),
        ],
        hist(t.iter().map(|n| n.level))
            .into_iter()
            .collect::<Vec<_>>(),
    );
    let types = hist(t.iter().filter_map(|n| n.type_bit()));
    c.eq(
        "type bits 0, 1, 42, 37, 57, 44",
        vec![2_754, 4, 2_458, 1_697, 1_123, 1_049],
        [0u32, 1, 42, 37, 57, 44]
            .iter()
            .map(|b| types.get(b).copied().unwrap_or(0))
            .collect::<Vec<_>>(),
    );
    let ranges = hist(
        t.iter()
            .map(|n| (n.min_range.to_bits(), n.max_range.to_bits())),
    );
    let pair = |a: f32, b: f32| {
        ranges
            .get(&(a.to_bits(), b.to_bits()))
            .copied()
            .unwrap_or(0)
    };
    c.eq(
        "range pairs (0,0) (80,800) (80,640), others",
        (12_611, 811, 23, 8, 40),
        (
            pair(0.0, 0.0),
            pair(80.0, 800.0),
            pair(80.0, 640.0),
            ranges.len().saturating_sub(3),
            t.len()
                .saturating_sub(pair(0.0, 0.0))
                .saturating_sub(pair(80.0, 800.0))
                .saturating_sub(pair(80.0, 640.0)),
        ),
    );
    assert!(t.iter().all(|n| n.min_range <= n.max_range), "§4.5 ranges");
    c.eq(
        "non-zero discovery",
        2_379,
        t.iter().filter(|n| n.discovery != 0).count(),
    );
    c.eq(
        "extra: empty, one MapNote_GameSpecific",
        (12_814, 671),
        (
            t.iter().filter(|n| n.extra.size.count == 0).count(),
            t.iter()
                .filter(|n| n.extra.len() == 1 && n.extra.has(env.pids.note_game_specific))
                .count(),
        ),
    );
    c.eq(
        "consecutive ascending (region, landblock) pairs",
        12_532,
        t.notes
            .windows(2)
            .filter(|w| {
                let a = w[0].base.position_values();
                let b = w[1].base.position_values();
                (a.region, a.landblock_key()) <= (b.region, b.landblock_key())
            })
            .count(),
    );
    c.eq("non-link texts resolving to empty text", 87, empty_texts);
    // §4.3 link rows.
    let link_rows: Vec<_> = t.iter().filter(|n| n.is_link()).collect();
    let mut target_kinds = BTreeMap::new();
    for n in &link_rows {
        let target = n.link.map(|l| l.target).unwrap_or_default();
        *target_kinds
            .entry(format!("{:?}", env.zone_kind(zones, target)))
            .or_insert(0usize) += 1;
    }
    c.eq(
        "link targets",
        vec![
            ("Some(Area)".to_owned(), 69),
            ("Some(Dungeon)".to_owned(), 2_685),
        ],
        target_kinds.into_iter().collect::<Vec<_>>(),
    );
    c.eq(
        "link position flag bytes",
        vec![
            (0x1B, 425),
            (0x3B, 1_300),
            (0x5B, 324),
            (0x7B, 699),
            (0xBB, 2),
            (0xFB, 4),
        ],
        hist(
            link_rows
                .iter()
                .filter_map(|n| n.link.map(|l| l.position.flags())),
        )
        .into_iter()
        .collect::<Vec<_>>(),
    );
    c.eq(
        "links within the row's landblock",
        412,
        link_rows
            .iter()
            .filter(|n| {
                let a = n.base.position_values();
                n.link.is_some_and(|l| {
                    let b = PositionValues::of(&l.position);
                    (a.region, a.landblock) == (b.region, b.landblock)
                })
            })
            .count(),
    );
    // §4.6.
    c.eq(
        "link rows with a zone entry",
        2_590,
        ix.zone_links.values().map(Vec::len).sum::<usize>(),
    );
    // §4.6 step 3, written out from the stored fields: the index's ranged list holds exactly
    // the rows with no dungeon and both ranges above 0.0002 in magnitude, in stored order.
    let ranged: Vec<usize> = t
        .iter()
        .enumerate()
        .filter(|(_, n)| {
            n.base.dungeon.is_null() && n.min_range.abs() > 0.0002 && n.max_range.abs() > 0.0002
        })
        .map(|(i, _)| i)
        .collect();
    assert_eq!(ix.ranged, ranged, "§4.6: ranged list");
    eprintln!("map notes: {} ranged rows", ranged.len());
    let filings = hist(t.iter().map(|n| match n.filing() {
        NoteFiling::Dungeon(_) => 0u8,
        NoteFiling::Landblock { .. } => 1,
        NoteFiling::Ranged => 2,
    }));
    eprintln!("map notes: filings (dungeon, landblock, ranged) {filings:?}");
    c.finish();
}

fn quest_notes(env: &Env, zones: &mut HashMap<Did, Option<MapKind>>) {
    let did = env.unique(DUNGEON_QUEST_NOTES_LABEL);
    let payload = env.read(did).expect("quest-note table exists");
    // MI1.
    let t = QuestNotesTable::parse_expecting(&payload, did, &env.dict).expect("quest notes decode");
    let layers_pid = env.pids.content_layers.id();
    for (i, n) in t.notes.iter().enumerate() {
        // MI4, MI9, MI10, MI15.
        assert!(position_flags_consistent(&n.base.position), "MI4: row {i}");
        assert!(position_is_valid(&n.base.position), "MI15: row {i}");
        assert_eq!(n.base.layers_pid(), layers_pid, "MI9: row {i}");
        assert!(!n.base.layer_keys().contains(&0), "MI9: row {i}");
        assert!(n.base.position_values().is_interior(), "MI10: row {i}");
        assert_eq!(
            env.zone_kind(zones, n.base.dungeon),
            Some(MapKind::Dungeon),
            "MI10: row {i}"
        );
    }
    let infos: Vec<QuestNoteInfo> = t
        .notes
        .iter()
        .map(|n| QuestNoteInfo::read(&n.quest_info, &env.pids))
        .collect();

    let mut c = Census::new("dungeon quest notes (§5)");
    c.eq("rows", 5_638, t.len());
    c.eq(
        "flag bytes",
        vec![(0x1B, 1_818), (0x3B, 2_163), (0x5B, 738), (0x7B, 919)],
        hist(t.notes.iter().map(|n| n.base.position.flags()))
            .into_iter()
            .collect::<Vec<_>>(),
    );
    c.eq(
        "regions",
        vec![
            (1, 1_657),
            (2, 1_958),
            (3, 381),
            (4, 1_074),
            (5, 561),
            (14, 7),
        ],
        hist(t.notes.iter().map(|n| n.base.position_values().region))
            .into_iter()
            .collect::<Vec<_>>(),
    );
    let mut area_kinds = BTreeMap::new();
    for n in &t.notes {
        let k = if n.base.area.is_null() {
            "none".to_owned()
        } else {
            format!("{:?}", env.zone_kind(zones, n.base.area))
        };
        *area_kinds.entry(k).or_insert(0usize) += 1;
    }
    c.eq(
        "area slots",
        vec![
            ("Some(Area)".to_owned(), 354),
            ("Some(Dungeon)".to_owned(), 36),
            ("none".to_owned(), 5_248),
        ],
        area_kinds.into_iter().collect::<Vec<_>>(),
    );
    let entities = hist(t.notes.iter().map(|n| n.base.entity));
    c.eq(
        "distinct entities, in more than one row",
        (2_612, 471),
        (
            entities.len(),
            entities.values().filter(|&&k| k > 1).count(),
        ),
    );
    c.eq(
        "layer list lengths",
        vec![(0, 3_099), (1, 2_520), (2, 10), (4, 1), (5, 8)],
        hist(t.notes.iter().map(|n| n.base.layer_keys().len()))
            .into_iter()
            .collect::<Vec<_>>(),
    );
    c.eq(
        "quest_info entry counts",
        vec![(1, 5_460), (2, 178)],
        hist(t.notes.iter().map(|n| n.quest_info.len()))
            .into_iter()
            .collect::<Vec<_>>(),
    );
    c.eq(
        "role constraints, active-quest arrays",
        (3_363, 2_453),
        (
            infos.iter().filter(|i| i.role_constraint.is_some()).count(),
            t.notes
                .iter()
                .filter(|n| n.quest_info.has(env.pids.active_quests))
                .count(),
        ),
    );
    let roles: Vec<_> = infos.iter().flat_map(|i| &i.active_quests).collect();
    c.eq(
        "quest values, role names",
        (16_124, 16_121),
        (
            roles.iter().filter(|r| r.quest.is_some()).count(),
            roles.iter().filter(|r| r.role_name.is_some()).count(),
        ),
    );
    let lens: Vec<usize> = infos
        .iter()
        .map(|i| i.active_quests.len())
        .filter(|&l| l > 0)
        .collect();
    c.eq(
        "active-quest array lengths (min, max, ones)",
        (1, 146, 720),
        (
            lens.iter().copied().min().unwrap_or(0),
            lens.iter().copied().max().unwrap_or(0),
            lens.iter().filter(|&&l| l == 1).count(),
        ),
    );
    assert_eq!(
        t.by_dungeon().values().map(Vec::len).sum::<usize>(),
        t.len(),
        "§5.3: every row filed under its dungeon"
    );
    c.finish();
}

// ---------------------------------------------------------------------------------------------
// §2.3, §7–§11 Every property set
// ---------------------------------------------------------------------------------------------

fn set_dids(env: &Env, lo: u32, hi: u32) -> Vec<Did> {
    let mut dids = Vec::new();
    for member in env.store.game_data().source().members() {
        for entry in member.archive.entries_range(lo..hi) {
            dids.push(Did(entry.expect("directory entry").did));
        }
    }
    dids.sort_unstable();
    dids.dedup();
    dids
}

#[derive(Default)]
struct Scan {
    kinds: HashMap<Did, MapKind>,
    sets: HashMap<Did, PropertySet>,
    npc_stations: Vec<(Did, StableMaster)>,
    pad_refs: (usize, usize),
    game_maps: usize,
    standalone: usize,
    objects: usize,
    landmark_music: usize,
    music_without_type: usize,
    /// Objects carrying `MapNote_Type` but not `Landmark_Music`.
    note_type_only: Vec<Did>,
    /// §2.4 edges found in object sets, by property: (source object, target).
    edges: BTreeMap<&'static str, Vec<(Did, Did)>>,
    /// Stand-alone (0x78) sets holding `UI_Map_GameMap` (§7).
    game_map_sets: BTreeSet<Did>,
}

/// The §2.4 edges collected from every object set: the property (or array) label and the
/// PID it was resolved to. DID values are collected at any depth, and for an array label
/// the DID elements of the array.
fn edge_pids(p: &MapPids) -> [(&'static str, Pid); 14] {
    [
        ("HousingSystem_PurchaseTemplateDID", p.purchase_template),
        ("HouseKeeper_HouseDID", p.house_keeper_house),
        ("HousingSystem_NeighborhoodGateDID", p.gate_from_weenie),
        ("QuestEvent_LandmarkDID", p.quest_landmark),
        ("Travel_PlayerHomeLocationArray", p.player_home_locations),
        ("Dungeon_MapData", p.dungeon_map_data),
        ("Dungeon_MiniMapData", p.dungeon_minimap_data),
        ("Area_MapData", p.area_map_data),
        ("Area_Territory", p.area_territory),
        ("Area_Region", p.area_region),
        ("NeighborhoodTemplate_SceneID", p.template_scene),
        ("TravelHop_SceneID", p.hop_scene),
        ("TravelMode_TravelEffectDID", p.mode_effect),
        ("House_CharacteristicArray", p.house_characteristics),
    ]
}

fn collect_edges(
    set: &PropertySet,
    object: Did,
    pids: &[(&'static str, Pid)],
    out: &mut BTreeMap<&'static str, Vec<(Did, Did)>>,
) {
    set.walk(|v| {
        let Some(target) = v.entry.did_nonnull() else {
            return;
        };
        let parent = match v.context {
            EntryContext::ArrayElement { parent } => Some(parent),
            _ => None,
        };
        for &(label, pid) in pids {
            if pid.is(v.entry.pid) || parent.is_some_and(|q| pid.is(q)) {
                out.entry(label).or_default().push((object, target));
            }
        }
    });
}

#[test]
fn all_property_sets() {
    let Some(env) = Env::open() else {
        eprintln!("TESSERA_GAME_DIR not set; skipping");
        return;
    };
    let started = std::time::Instant::now();
    let places = {
        let did = env.unique(PLACES_TABLE_LABEL);
        PlacesTable::parse(&env.read(did).expect("places")).expect("places decode")
    };
    let p = &env.pids;
    let mut scan = Scan::default();

    // §7: stand-alone sets.
    for did in set_dids(&env, 0x7800_0000, 0x7900_0000) {
        let payload = env.read(did).expect("listed");
        let rec = PropertySetRecord::parse(&payload, &env.dict).expect("0x78 set");
        scan.standalone += 1;
        if rec.set.has(p.game_map) {
            scan.game_maps += 1;
            scan.game_map_sets.insert(did);
        }
    }

    // Every object property set, on a few threads.
    let dids = set_dids(&env, 0x7900_0000, 0x7A00_0000);
    let threads = std::thread::available_parallelism().map_or(4, |n| n.get().min(8));
    let chunk = dids.len().div_ceil(threads).max(1);
    let parts: Vec<Scan> = std::thread::scope(|sc| {
        let handles: Vec<_> = dids
            .chunks(chunk)
            .map(|part| {
                let (env, places) = (&env, &places);
                sc.spawn(move || scan_objects(env, places, part))
            })
            .collect();
        handles
            .into_iter()
            .map(|h| h.join().expect("scan thread"))
            .collect()
    });
    for part in parts {
        scan.objects += part.objects;
        scan.kinds.extend(part.kinds);
        scan.sets.extend(part.sets);
        scan.npc_stations.extend(part.npc_stations);
        scan.pad_refs.0 += part.pad_refs.0;
        scan.pad_refs.1 += part.pad_refs.1;
        scan.landmark_music += part.landmark_music;
        scan.music_without_type += part.music_without_type;
        scan.note_type_only.extend(part.note_type_only);
        for (label, edges) in part.edges {
            scan.edges.entry(label).or_default().extend(edges);
        }
    }
    scan.npc_stations.sort_by_key(|&(d, _)| d);
    scan.note_type_only.sort_unstable();
    for edges in scan.edges.values_mut() {
        edges.sort_unstable();
    }

    // §8.5.1: `Segment_HeadName` values of SegmentNode SEDs in every landblock-info record.
    let heads = segment_node_heads(&env);
    eprintln!(
        "map kinds: scanned {} object sets and {} stand-alone sets in {:.1?}",
        scan.objects,
        scan.standalone,
        started.elapsed()
    );
    check_kinds(&env, &scan, &places, &heads);
    check_edges(&env, &scan, &heads);
}

/// SegmentNode SEDs of all landblock-info records (`landblock.md`; ED type 19), and the
/// landblock-info edges of §2.4.
#[derive(Default)]
struct SegmentHeads {
    nodes: usize,
    with_head: usize,
    values: Vec<String>,
    records: usize,
    /// `HouseKeeper_HouseDID` values on weenies and on SEDs.
    house_keepers: (Vec<Did>, Vec<Did>),
    /// `HousingSystem_NeighborhoodGateDID` values on weenies.
    gates: Vec<Did>,
    /// Object DIDs of weenie SEDs (type 6).
    weenie_objects: Vec<Did>,
}

fn segment_node_heads(env: &Env) -> SegmentHeads {
    let set = env.store.game_data().source();
    let mut work: Vec<(usize, Entry)> = Vec::new();
    for (i, m) in set.members().iter().enumerate() {
        if !matches!(m.id.kind, ArchiveKind::Cell(_)) {
            continue;
        }
        let lo = 0x8000_0000u32 | (2 << 20);
        for e in m.archive.entries_range(lo..=lo | 0x000F_FFFF) {
            work.push((i, e.expect("directory walk")));
        }
    }
    let head_pid = env.pids.segment_node_head;
    let (keeper_pid, gate_pid) = (env.pids.house_keeper_house, env.pids.gate_from_weenie);
    let threads = std::thread::available_parallelism().map_or(4, |n| n.get().min(8));
    let chunk = work.len().div_ceil(threads).max(1);
    let parts: Vec<SegmentHeads> = std::thread::scope(|sc| {
        let handles: Vec<_> = work
            .chunks(chunk)
            .map(|part| {
                sc.spawn(move || {
                    let mut out = SegmentHeads::default();
                    for (i, e) in part {
                        let payload = set.member(*i).archive.read_entry(e).expect("read info");
                        let info = LandblockInfo::parse(&payload, Some(e.stamp), &env.dict)
                            .unwrap_or_else(|err| panic!("{:#x}: {err}", e.did));
                        out.records += 1;
                        for w in &info.weenies.items {
                            out.house_keepers
                                .0
                                .extend(w.properties.did_nonnull(keeper_pid));
                            out.gates.extend(w.properties.did_nonnull(gate_pid));
                        }
                        for sed in &info.entities {
                            if let Some(props) = &sed.properties {
                                out.house_keepers.1.extend(props.did_nonnull(keeper_pid));
                            }
                            if sed.kind() == Some(StaticEntityType::Weenie) {
                                out.weenie_objects.extend(sed.object);
                            }
                            if sed.kind() != Some(StaticEntityType::SegmentNode) {
                                continue;
                            }
                            out.nodes += 1;
                            if let Some(h) =
                                sed.properties.as_ref().and_then(|s| s.string(head_pid))
                            {
                                out.with_head += 1;
                                out.values.push(h.to_owned());
                            }
                        }
                    }
                    out
                })
            })
            .collect();
        handles
            .into_iter()
            .map(|h| h.join().expect("SED thread"))
            .collect()
    });
    let mut all = SegmentHeads::default();
    for p in parts {
        all.nodes += p.nodes;
        all.with_head += p.with_head;
        all.records += p.records;
        all.values.extend(p.values);
        all.house_keepers.0.extend(p.house_keepers.0);
        all.house_keepers.1.extend(p.house_keepers.1);
        all.gates.extend(p.gates);
        all.weenie_objects.extend(p.weenie_objects);
    }
    all
}

fn scan_objects(env: &Env, places: &PlacesTable, dids: &[Did]) -> Scan {
    let p = &env.pids;
    let edge_pids = edge_pids(p);
    let mut scan = Scan::default();
    for &did in dids {
        let payload = env.read(did).expect("listed");
        let rec =
            PropertySetRecord::parse(&payload, &env.dict).unwrap_or_else(|e| panic!("{did}: {e}"));
        scan.objects += 1;
        let object = Did(did.0 - 0x0900_0000);
        if let Some(sm) = StableMaster::read(&rec.set, p)
            && !rec.set.has(p.home_location)
        {
            scan.npc_stations.push((object, sm));
        }
        // §10: place keys found by walking every string value.
        for_each_place_reference(&rec.set, places, |r| {
            scan.pad_refs.0 += usize::from(p.pad_location.is(r.pid));
        });
        scan.pad_refs.1 += usize::from(rec.set.string(p.pad_location).is_some());
        if rec.set.has(p.landmark_music) {
            scan.landmark_music += 1;
            scan.music_without_type += usize::from(!rec.set.has(p.note_type));
        } else if rec.set.has(p.note_type) {
            scan.note_type_only.push(object);
        }
        collect_edges(&rec.set, object, &edge_pids, &mut scan.edges);
        if let Some(kind) = classify_set(&rec.set, p) {
            scan.kinds.insert(object, kind);
            if !matches!(kind, MapKind::Landmark | MapKind::Area | MapKind::Dungeon) {
                scan.sets.insert(object, rec.set);
            }
        }
    }
    scan
}

fn of_kind(scan: &Scan, kind: MapKind) -> Vec<(Did, &PropertySet)> {
    let mut v: Vec<(Did, &PropertySet)> = scan
        .kinds
        .iter()
        .filter(|&(_, &k)| k == kind)
        .filter_map(|(&d, _)| Some((d, scan.sets.get(&d)?)))
        .collect();
    v.sort_by_key(|&(d, _)| d);
    v
}

#[allow(clippy::too_many_lines)]
fn check_kinds(env: &Env, scan: &Scan, places: &PlacesTable, heads: &SegmentHeads) {
    let p = &env.pids;
    let kind_of = |d: Did| scan.kinds.get(&d).copied();
    let counts = hist(scan.kinds.values().copied());
    let mut c = Census::new("kinds (§2.3)");
    for (kind, n) in [
        (MapKind::TravelWeb, 421),
        (MapKind::TravelDestination, 768),
        (MapKind::TravelRoute, 5_008),
        (MapKind::TravelSegment, 2_223),
        (MapKind::TravelHop, 11),
        (MapKind::TravelPad, 73),
        (MapKind::TravelMode, 8),
        (MapKind::TravelVersioning, 1),
        (MapKind::Landmark, 2_733),
        (MapKind::Neighbourhood, 3_248),
        (MapKind::NeighbourhoodTemplate, 14),
        (MapKind::HouseList, 14),
        (MapKind::House, 227),
        (MapKind::NeighbourhoodGate, 13),
        (MapKind::HousingHook, 744),
        (MapKind::Ripcord, 348),
        (MapKind::Area, 675),
        (MapKind::Territory, 86),
        (MapKind::Region, 5),
        (MapKind::Dungeon, 1_303),
    ] {
        c.eq(
            &format!("{kind:?}"),
            n,
            counts.get(&kind).copied().unwrap_or(0),
        );
    }
    c.eq("UI_Map_GameMap sets (§7)", 1_300, scan.game_maps);
    c.eq("stand-alone sets (§7)", 1_616, scan.standalone);
    c.eq(
        "sets with Landmark_Music, of them without MapNote_Type",
        (2_733, 0),
        (scan.landmark_music, scan.music_without_type),
    );
    c.finish();

    // §8 travel.
    let mut c = Census::new("travel (§8)");
    let webs: Vec<(Did, TravelWeb)> = of_kind(scan, MapKind::TravelWeb)
        .into_iter()
        .map(|(d, s)| (d, TravelWeb::read(s, p).expect("travel-web view")))
        .collect();
    let routes: BTreeMap<Did, TravelRoute> = of_kind(scan, MapKind::TravelRoute)
        .into_iter()
        .map(|(d, s)| (d, TravelRoute::read(s, p).expect("route view")))
        .collect();
    let dests: BTreeMap<Did, TravelDestination> = of_kind(scan, MapKind::TravelDestination)
        .into_iter()
        .map(|(d, s)| (d, TravelDestination::read(s, p).expect("destination view")))
        .collect();
    let segments: BTreeMap<Did, TravelSegment> = of_kind(scan, MapKind::TravelSegment)
        .into_iter()
        .map(|(d, s)| (d, TravelSegment::read(s, p)))
        .collect();
    // MI14: station edges.
    let mut offers = 0usize;
    let mut sold: HashMap<Did, usize> = HashMap::new();
    let mut alias_kinds = BTreeMap::new();
    let mut zero_cost = 0usize;
    let mut own_home = 0usize;
    for (d, w) in &webs {
        assert_eq!(
            kind_of(w.home),
            Some(MapKind::TravelDestination),
            "MI14: {d} home"
        );
        for a in &w.home_aliases {
            *alias_kinds
                .entry(format!("{:?}", kind_of(*a)))
                .or_insert(0usize) += 1;
        }
        for o in &w.offers {
            offers += 1;
            assert_eq!(
                kind_of(o.route),
                Some(MapKind::TravelRoute),
                "MI14: {d} offer"
            );
            *sold.entry(o.route).or_default() += 1;
            zero_cost += usize::from(o.cost == Some(0));
            own_home += usize::from(
                routes
                    .get(&o.route)
                    .is_some_and(|r| r.destination == w.home),
            );
        }
    }
    c.eq("offers", 5_199, offers);
    c.eq("offers with cost 0", 29, zero_cost);
    c.eq("offers leading to the station's home", 12, own_home);
    c.eq("routes sold", 5_005, sold.len());
    c.eq(
        "most stations selling one route",
        13,
        sold.values().copied().max().unwrap_or(0),
    );
    c.eq(
        "home aliases (destination, route)",
        vec![
            ("Some(TravelDestination)".to_owned(), 775),
            ("Some(TravelRoute)".to_owned(), 26),
        ],
        alias_kinds.into_iter().collect::<Vec<_>>(),
    );
    c.eq(
        "distinct homes",
        399,
        webs.iter()
            .map(|(_, w)| w.home)
            .collect::<BTreeSet<_>>()
            .len(),
    );
    // MI11 and MI14: routes.
    let mut patterns: BTreeMap<(String, bool), usize> = BTreeMap::new();
    let mut by_mode: BTreeMap<Did, usize> = BTreeMap::new();
    let mut action_kinds = BTreeMap::new();
    for (d, r) in &routes {
        assert_eq!(
            kind_of(r.destination),
            Some(MapKind::TravelDestination),
            "MI11: {d} destination"
        );
        let mode = r.mode.unwrap_or_else(|| panic!("MI11: {d} has no mode"));
        assert_eq!(kind_of(mode), Some(MapKind::TravelMode), "MI14: {d} mode");
        *by_mode.entry(mode).or_default() += 1;
        assert!(!r.actions.is_empty(), "MI11: {d} has no action");
        let acts: Vec<RouteAction> = r
            .actions
            .iter()
            .map(|a| {
                kind_of(*a)
                    .and_then(RouteAction::of)
                    .unwrap_or_else(|| panic!("MI11: {d} action {a} is {:?}", kind_of(*a)))
            })
            .collect();
        for a in &acts {
            *action_kinds.entry(format!("{a:?}")).or_insert(0usize) += 1;
        }
        let swift = dests.get(&r.destination).is_some_and(|x| x.is_swift);
        *patterns.entry((route_pattern(&acts), swift)).or_default() += 1;
    }
    c.eq(
        "route actions (hop, pad, segment)",
        vec![
            ("Hop".to_owned(), 3_058),
            ("Pad".to_owned(), 234),
            ("Segment".to_owned(), 18_878),
        ],
        action_kinds.into_iter().collect::<Vec<_>>(),
    );
    let pat = |s: &str, swift: bool| patterns.get(&(s.to_owned(), swift)).copied().unwrap_or(0);
    c.eq(
        "patterns S H S, S+, H P, S+ H S+, H S, S (swift, slow)",
        vec![(2_661, 24), (0, 1_946), (170, 63), (0, 66), (61, 0), (0, 6)],
        ["S H S", "S+", "H P", "S+ H S+", "H S", "S"]
            .iter()
            .map(|s| (pat(s, true), pat(s, false)))
            .collect::<Vec<_>>(),
    );
    c.eq(
        "routes per mode",
        vec![2_665, 669, 488, 477, 366, 202, 109, 32],
        sorted_desc(&by_mode),
    );
    c.eq(
        "swift destinations, allowing paper items",
        (395, 763),
        (
            dests.values().filter(|d| d.is_swift).count(),
            dests
                .values()
                .filter(|d| d.allow_paper_items == Some(true))
                .count(),
        ),
    );
    c.eq(
        "routes with a minimum level, quest requirements, faction",
        (4_516, 178, 41),
        (
            routes.values().filter(|r| r.min_level.is_some()).count(),
            routes
                .values()
                .filter(|r| !r.quest_requirements.is_empty())
                .count(),
            routes
                .values()
                .filter(|r| r.required_faction.is_some())
                .count(),
        ),
    );
    // MI11: segments.
    for (d, s) in &segments {
        assert!(s.is_well_formed(), "MI11: segment {d}");
        for sub in &s.sub_segments {
            assert_eq!(kind_of(*sub), Some(MapKind::TravelSegment), "MI14: {d}");
        }
    }
    c.eq(
        "segments with head names, sub-segments",
        (2_191, 32),
        (
            segments.values().filter(|s| s.head_name.is_some()).count(),
            segments
                .values()
                .filter(|s| !s.sub_segments.is_empty())
                .count(),
        ),
    );
    c.eq(
        "distinct head names",
        2_185,
        segments
            .values()
            .filter_map(|s| s.head_name.as_deref())
            .collect::<BTreeSet<_>>()
            .len(),
    );
    // MI11: every head name names a SegmentNode SED's `Segment_HeadName`.
    let node_heads: BTreeSet<&str> = heads.values.iter().map(String::as_str).collect();
    for (d, seg) in &segments {
        if let Some(h) = seg.head_name.as_deref() {
            assert!(
                node_heads.contains(h),
                "MI11: segment {d} head name has no SegmentNode"
            );
        }
    }
    let used: BTreeSet<&str> = segments
        .values()
        .filter_map(|s| s.head_name.as_deref())
        .collect();
    c.eq(
        "landblock-info records, SegmentNode SEDs, with a head name, using a segment's value",
        (49_636, 55_645, 2_260, 2_193),
        (
            heads.records,
            heads.nodes,
            heads.with_head,
            heads
                .values
                .iter()
                .filter(|v| used.contains(v.as_str()))
                .count(),
        ),
    );
    // Hops, pads (MI12), modes.
    let hops: Vec<TravelHop> = of_kind(scan, MapKind::TravelHop)
        .into_iter()
        .map(|(_, s)| TravelHop::read(s, p))
        .collect();
    c.eq(
        "hops with a scene",
        9,
        hops.iter().filter(|h| h.scene.is_some()).count(),
    );
    for (d, s) in of_kind(scan, MapKind::TravelPad) {
        let pad = TravelPad::read(s, p).expect("pad view");
        assert!(places.get_str(&pad.location).is_some(), "MI12: pad {d}");
    }
    let modes: Vec<TravelMode> = of_kind(scan, MapKind::TravelMode)
        .into_iter()
        .map(|(_, s)| TravelMode::read(s, p))
        .collect();
    c.eq(
        "modes with an effect, distinct script values",
        (7, 7),
        (
            modes.iter().filter(|m| m.effect.is_some()).count(),
            modes
                .iter()
                .filter_map(|m| m.script_pants)
                .collect::<BTreeSet<_>>()
                .len(),
        ),
    );
    // §8.7.
    let web_dir = env
        .store
        .weenie(TRAVEL_WEB_DIRECTORY_LABEL)
        .expect("WEENIECONTENT")
        .expect("travel directory");
    let dir_set = env.object_set(web_dir).expect("directory set");
    let listed = travel_directory(&dir_set, p);
    for s in &listed {
        assert_eq!(kind_of(*s), Some(MapKind::TravelWeb), "MI14: directory {s}");
    }
    let npc_named: BTreeSet<Did> = scan
        .npc_stations
        .iter()
        .filter_map(|(_, s)| s.station)
        .collect();
    for s in &npc_named {
        assert_eq!(
            kind_of(*s),
            Some(MapKind::TravelWeb),
            "MI14: NPC station {s}"
        );
    }
    let listed_set: BTreeSet<Did> = listed.iter().copied().collect();
    for (d, _) in &webs {
        assert!(
            listed_set.contains(d) || npc_named.contains(d),
            "§8.7: station {d} unreachable"
        );
    }
    c.eq("directory stations", 375, listed.len());
    c.eq(
        "NPCs naming a station, distinct stations",
        (437, 411),
        (scan.npc_stations.len(), npc_named.len()),
    );
    // §8.7: the pad keys outside pad records are those of NPCs.
    let pads = counts.get(&MapKind::TravelPad).copied().unwrap_or(0);
    c.eq(
        "NPC pads resolving / with a pad",
        (421, 432),
        (
            scan.pad_refs.0.saturating_sub(pads),
            scan.pad_refs.1.saturating_sub(pads),
        ),
    );
    c.eq(
        "TravelPad_Location values resolving / total (§10)",
        (494, 505),
        scan.pad_refs,
    );
    let versioning: Vec<TravelVersioning> = of_kind(scan, MapKind::TravelVersioning)
        .into_iter()
        .map(|(_, s)| TravelVersioning::read(s, p))
        .collect();
    c.eq(
        "versioning pairs",
        11,
        versioning.iter().map(|v| v.pairs.len()).sum::<usize>(),
    );
    for v in &versioning {
        for (slow, swift) in &v.pairs {
            for d in [slow, swift].into_iter().flatten() {
                assert_eq!(kind_of(*d), Some(MapKind::TravelDestination), "MI14");
            }
        }
    }
    c.finish();

    // §9 housing.
    let mut c = Census::new("housing (§9)");
    let hood_dir = env
        .store
        .weenie(NEIGHBORHOOD_DIRECTORY_LABEL)
        .expect("WEENIECONTENT")
        .expect("neighbourhood directory");
    let hoods = neighbourhood_directory(&env.object_set(hood_dir).expect("set"), p);
    c.eq("directory neighbourhoods", 3_248, hoods.len());
    let mut per_template: BTreeMap<Did, usize> = BTreeMap::new();
    for (d, s) in of_kind(scan, MapKind::Neighbourhood) {
        let n = Neighbourhood::read(s, p).expect("neighbourhood view");
        assert_eq!(
            kind_of(n.template),
            Some(MapKind::NeighbourhoodTemplate),
            "MI14: {d}"
        );
        *per_template.entry(n.template).or_default() += 1;
    }
    c.eq(
        "copies per template",
        vec![
            450, 400, 400, 350, 300, 275, 275, 275, 275, 225, 20, 1, 1, 1,
        ],
        sorted_desc(&per_template),
    );
    let mut list_of_house: HashMap<Did, usize> = HashMap::new();
    let lists: BTreeMap<Did, HouseList> = of_kind(scan, MapKind::HouseList)
        .into_iter()
        .map(|(d, s)| (d, HouseList::read(s, p)))
        .collect();
    for l in lists.values() {
        for h in &l.houses {
            assert_eq!(kind_of(*h), Some(MapKind::House), "MI14: house list entry");
            *list_of_house.entry(*h).or_default() += 1;
        }
    }
    let mut boundary_blocks = Vec::new();
    for (d, s) in of_kind(scan, MapKind::NeighbourhoodTemplate) {
        let t = NeighbourhoodTemplate::read(s, p).expect("template view");
        // MI13.
        assert_eq!(kind_of(t.house_list), Some(MapKind::HouseList), "MI13: {d}");
        assert!(t.scene.is_some(), "MI13: {d} scene");
        for key in [&t.telepad, &t.boot_position] {
            let key = key.as_deref().unwrap_or_else(|| panic!("MI13: {d} key"));
            assert!(places.get_str(key).is_some(), "MI13: {d} key resolves");
        }
        for b in &t.boundaries {
            assert!(matches!(b.flags(), 0x03 | 0x43), "§9.3: boundary flags");
        }
        boundary_blocks.push(t.boundaries.len());
    }
    boundary_blocks.sort_unstable();
    c.eq(
        "boundary sizes (min, max)",
        (0, 173),
        (
            boundary_blocks.first().copied().unwrap_or(0),
            boundary_blocks.last().copied().unwrap_or(0),
        ),
    );
    let houses: Vec<(Did, House)> = of_kind(scan, MapKind::House)
        .into_iter()
        .map(|(d, s)| (d, House::read(s, p).expect("house view")))
        .collect();
    for (d, h) in &houses {
        // MI13: in exactly one list.
        assert_eq!(list_of_house.get(d).copied(), Some(1), "MI13: house {d}");
        assert_eq!(
            kind_of(h.template),
            Some(MapKind::NeighbourhoodTemplate),
            "MI14"
        );
        let key = h
            .telepad
            .as_deref()
            .unwrap_or_else(|| panic!("house {d} telepad"));
        assert!(places.get_str(key).is_some(), "§9.4: house {d} telepad");
    }
    c.eq(
        "houses: kinship, premium, distinct types",
        (25, 88, 37),
        (
            houses
                .iter()
                .filter(|(_, h)| h.is_kinship == Some(true))
                .count(),
            houses
                .iter()
                .filter(|(_, h)| h.is_premium == Some(true))
                .count(),
            houses
                .iter()
                .filter_map(|(_, h)| h.house_type)
                .collect::<BTreeSet<_>>()
                .len(),
        ),
    );
    for (d, s) in of_kind(scan, MapKind::NeighbourhoodGate) {
        let g = NeighbourhoodGate::read(s, p).expect("gate view");
        assert_eq!(
            kind_of(g.template),
            Some(MapKind::NeighbourhoodTemplate),
            "MI14: {d}"
        );
        let key = g
            .telepad
            .as_deref()
            .unwrap_or_else(|| panic!("gate {d} telepad"));
        assert!(places.get_str(key).is_some(), "§9.5: gate {d} telepad");
    }
    let hooks: Vec<HousingHook> = of_kind(scan, MapKind::HousingHook)
        .into_iter()
        .map(|(_, s)| HousingHook::read(s, p))
        .collect();
    c.eq(
        "property hooks",
        153,
        hooks
            .iter()
            .filter(|h| h.is_property_hook == Some(true))
            .count(),
    );
    c.finish();

    // §11 ripcords.
    let mut c = Census::new("ripcords (§11)");
    let map = env
        .store
        .game_data()
        .did_map_by_label(RIPCORDS_LABEL)
        .expect("root map")
        .expect("RIPCORDS map");
    let mut named = 0usize;
    for e in map.iter().filter(|e| !e.value.is_null()) {
        assert_eq!(kind_of(e.value), Some(MapKind::Ripcord), "§11: {}", e.value);
        named += 1;
    }
    c.eq("entries, records", (292, 291), (map.len(), named));
    let with_property = of_kind(scan, MapKind::Ripcord)
        .into_iter()
        .filter(|(_, s)| {
            tessera_formats::gameplay::map::Ripcord::read(*s, p)
                .world_property
                .is_some()
        })
        .count();
    c.eq("ripcords naming a property", 348, with_property);
    c.finish();
}

/// MI14 for the §2.4 edges not checked with the views above, and MI13's scene check.
fn check_edges(env: &Env, scan: &Scan, heads: &SegmentHeads) {
    let kind_of = |d: Did| scan.kinds.get(&d).copied();
    let edges = |label: &str| -> Vec<Did> {
        scan.edges
            .get(label)
            .map(|v| v.iter().map(|&(_, t)| t).collect())
            .unwrap_or_default()
    };
    let mut c = Census::new("reference graph (§2.4)");

    // Edges whose target is a kind of §2.3. A value naming no record at all is counted as
    // dangling (a census count: §2.4 reports none); every value naming a record must name one
    // of the listed kind. Returns the number of values naming a record.
    let dangling: std::cell::RefCell<Vec<(String, usize)>> = std::cell::RefCell::default();
    let by_kind = |what: &str, targets: &[Did], kind: MapKind| {
        let (missing, present): (Vec<Did>, Vec<Did>) = targets
            .iter()
            .partition(|&&d| kind_of(d).is_none() && env.root_class(d).is_none());
        let wrong: Vec<String> = present
            .iter()
            .filter(|&&d| kind_of(d) != Some(kind))
            .map(|&d| format!("{d}: {:?}", kind_of(d)))
            .collect();
        assert!(
            wrong.is_empty(),
            "MI14: {} of {} {what} values are not {kind:?}: {:?}",
            wrong.len(),
            present.len(),
            &wrong[..wrong.len().min(5)]
        );
        if !missing.is_empty() {
            let distinct = missing.iter().collect::<BTreeSet<_>>().len();
            eprintln!(
                "MI14: {} {what} values ({distinct} distinct DIDs) name no record",
                missing.len()
            );
        }
        dangling.borrow_mut().push((what.to_owned(), missing.len()));
        present.len()
    };
    let purchase = by_kind(
        "HousingSystem_PurchaseTemplateDID",
        &edges("HousingSystem_PurchaseTemplateDID"),
        MapKind::NeighbourhoodTemplate,
    );
    c.eq("NPC purchase templates", 21, purchase);
    let keepers = (
        by_kind(
            "HouseKeeper_HouseDID (weenies)",
            &heads.house_keepers.0,
            MapKind::House,
        ),
        by_kind(
            "HouseKeeper_HouseDID (SEDs)",
            &heads.house_keepers.1,
            MapKind::House,
        ),
    );
    c.eq("house keepers on weenies, on SEDs", (454, 3), keepers);
    let gates = by_kind(
        "HousingSystem_NeighborhoodGateDID",
        &heads.gates,
        MapKind::NeighbourhoodGate,
    );
    c.eq("gates on weenies", 43, gates);
    // The same properties on object sets, if any (§2.4 lists none).
    let object_side = (
        by_kind(
            "HouseKeeper_HouseDID (object sets)",
            &edges("HouseKeeper_HouseDID"),
            MapKind::House,
        ),
        by_kind(
            "HousingSystem_NeighborhoodGateDID (object sets)",
            &edges("HousingSystem_NeighborhoodGateDID"),
            MapKind::NeighbourhoodGate,
        ),
    );
    eprintln!("§2.4: house-keeper and gate values on object sets {object_side:?}");
    let landmarks = by_kind(
        "QuestEvent_LandmarkDID",
        &edges("QuestEvent_LandmarkDID"),
        MapKind::Landmark,
    );
    c.eq("quest landmark references", 3_419, landmarks);
    let homes = by_kind(
        "Travel_PlayerHomeLocationArray",
        &edges("Travel_PlayerHomeLocationArray"),
        MapKind::TravelDestination,
    );
    c.eq("player home locations (25 + 162)", 187, homes);
    let zones = (
        by_kind(
            "Area_Territory",
            &edges("Area_Territory"),
            MapKind::Territory,
        ),
        by_kind("Area_Region", &edges("Area_Region"), MapKind::Region),
    );
    c.eq("area → territory, territory → region", (675, 86), zones);
    let hooks = heads
        .weenie_objects
        .iter()
        .filter(|&&d| kind_of(d) == Some(MapKind::HousingHook))
        .count();
    c.eq("weenie SEDs placing a housing hook", 32_338, hooks);
    for (what, n) in dangling.take() {
        c.eq(&format!("{what}: values naming no record"), 0, n);
    }

    // Map links of zones (§7): 0x78 sets holding `UI_Map_GameMap`.
    let game_map = |what: &str| {
        let targets = edges(what);
        for d in &targets {
            assert!(
                scan.game_map_sets.contains(d),
                "MI14: {what} {d} is not a set holding UI_Map_GameMap"
            );
        }
        targets.len()
    };
    let maps = (
        game_map("Dungeon_MapData"),
        game_map("Dungeon_MiniMapData"),
        game_map("Area_MapData"),
    );
    c.eq(
        "dungeon map, minimap, area map links",
        (1_303, 1_303, 1),
        maps,
    );

    // Edges whose target §2.4 names by class: the targets exist; their classes are counts.
    let mut scenes = edges("NeighborhoodTemplate_SceneID");
    let template_scenes = scenes.len();
    scenes.extend(edges("TravelHop_SceneID"));
    let scene_classes = env.classes_of("scene", &scenes);
    // MI13 and MI14: template and hop scenes are records of one class.
    assert_eq!(
        scene_classes.len(),
        1,
        "MI13: scene classes {scene_classes:?}"
    );
    c.eq(
        "scene links (templates, hops)",
        (14, 9),
        (template_scenes, scenes.len() - template_scenes),
    );
    c.eq("scene class", vec![(1468, 23)], scene_classes);
    c.eq(
        "travel effect classes",
        vec![(734, 7)],
        env.classes_of(
            "TravelMode_TravelEffectDID",
            &edges("TravelMode_TravelEffectDID"),
        ),
    );
    let traits = env.classes_of(
        "House_CharacteristicArray",
        &edges("House_CharacteristicArray"),
    );
    c.eq(
        "house characteristic classes, values",
        (vec![1477, 1483], 471),
        (
            traits.iter().map(|&(k, _)| k).collect::<Vec<_>>(),
            traits.iter().map(|&(_, n)| n).sum::<usize>(),
        ),
    );

    // §2.3 landmarks: the class of the recognised ones, and how many sets with
    // `MapNote_Type` but no `Landmark_Music` share it (§14.4 OPEN).
    let landmarks: Vec<Did> = scan
        .kinds
        .iter()
        .filter(|&(_, &k)| k == MapKind::Landmark)
        .map(|(&d, _)| d)
        .collect();
    let landmark_classes = env.classes_of("landmark", &landmarks);
    let same_class = if let [(class, _)] = landmark_classes.as_slice() {
        scan.note_type_only
            .iter()
            .filter(|&&d| env.root_class(d) == Some(*class))
            .count()
    } else {
        0
    };
    eprintln!(
        "§2.3 landmarks: classes of recognised ones {landmark_classes:?}; {} sets carry \
         MapNote_Type without Landmark_Music, {same_class} of them in that class",
        scan.note_type_only.len()
    );
    c.finish();
}
