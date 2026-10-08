// SPDX-License-Identifier: GPL-3.0-or-later
//! The object store and the geography layer of `tessera-wstate` (`docs/specs/wstate.md`
//! §2, §11; `wlib.md` §2) over a **damaged synthetic install**: the class library found
//! through the WLIB map, object headers, imports and states with the cache, game objects
//! with their property sets, well-known objects, singletons; then area / territory /
//! region / dungeon classification, area chains, dungeon positions, the region table
//! (from the control record's properties and from its state) and the geography index.
//!
//! The base install holds a property dictionary, a class library with one class per
//! geographic kind, one object of each kind (a territory list, a dungeon position and a
//! region table as native objects), their property sets and the DID maps that name the
//! library and the region-control object. The input starts with an edit plan
//! (`common/mutate.rs`) applied to the records in the order they are read; the rest picks
//! cache limits, extra DIDs, a singleton class and a name to search.
//!
//! Nothing may panic, and the index build report must add up.

#![no_main]

#[path = "common/mutate.rs"]
mod mutate;

use std::sync::OnceLock;

use libfuzzer_sys::fuzz_target;
use mutate::{InjectedFailure, Mutator, Read};
use tessera_props::{MemorySource, RecordSource, SourceError};
use tessera_text::Strings;
use tessera_wstate::testkit::{
    LibraryBuilder, RecordBuilder, did_map, property_dictionary, property_set_record, put_position,
    put_string_info_literal, put_tsize,
};
use tessera_wstate::{
    CacheLimits, Did, Geo, GeoIndex, GeoIndexBuilder, GeoKind, LIBRARY_DID, ObjectStore, Ref,
    Value, ValueType, geo,
};

// Synthetic PIDs and DIDs (no game data). The property names are the ones the
// geography layer recognises kinds by (`wstate.md` §11.1).
const P_AREA_NAME: u32 = 0x1000_0001;
const P_AREA_TERRITORY: u32 = 0x1000_0002;
const P_AREA_REGION: u32 = 0x1000_0003;
const P_AREA_REGION_ID: u32 = 0x1000_0004;
const P_DUNGEON_NAME: u32 = 0x1000_0005;
const P_DUNGEON_MAP: u32 = 0x1000_0006;
const P_DUNGEON_PARENT: u32 = 0x1000_0007;
const P_REGION_LIST: u32 = 0x1000_0008;
const P_REGION_ENTRY: u32 = 0x1000_0009;
const P_REGION_ID: u32 = 0x1000_000A;
const P_REGION_NAME: u32 = 0x1000_000B;

const AREA: u32 = 0x7000_0010;
const TERRITORY: u32 = 0x7000_0011;
const REGION: u32 = 0x7000_0012;
const DUNGEON: u32 = 0x7000_0020;
const SUB_DUNGEON: u32 = 0x7000_0021;
const CONTROL: u32 = 0x7000_0030;
const OTHER: u32 = 0x7000_0040;
const ALL: [u32; 7] = [
    AREA,
    TERRITORY,
    REGION,
    DUNGEON,
    SUB_DUNGEON,
    CONTROL,
    OTHER,
];

fn le(words: &[u32]) -> Vec<u8> {
    words.iter().flat_map(|w| w.to_le_bytes()).collect()
}

fn si(text: &str) -> Vec<u8> {
    let mut b = Vec::new();
    put_string_info_literal(&mut b, text);
    b
}

/// The valid base install, built once.
fn base() -> MemorySource {
    static BASE: OnceLock<MemorySource> = OnceLock::new();
    BASE.get_or_init(build_base).clone()
}

fn build_base() -> MemorySource {
    let dict = property_dictionary(&[
        (P_AREA_NAME, "Area_Name", 13),
        (P_AREA_TERRITORY, "Area_Territory", 20),
        (P_AREA_REGION, "Area_Region", 20),
        (P_AREA_REGION_ID, "Area_RegionID", 15),
        (P_DUNGEON_NAME, "Dungeon_Name", 13),
        (P_DUNGEON_MAP, "Dungeon_MapData", 20),
        (P_DUNGEON_PARENT, "Dungeon_ParentDungeon", 20),
        (P_REGION_LIST, "Region_Map_List", 12),
        (P_REGION_ENTRY, "Region_Map_Entry", 11),
        (P_REGION_ID, "Region_ID", 15),
        (P_REGION_NAME, "Region_Name", 13),
    ]);
    // Classes: 10 area, 11 territory, 12 region, 13 dungeon, 14 region control, 15 other.
    let lib = LibraryBuilder::new(20)
        .define(10, &[])
        .define(11, &[(geo::TERRITORY_LIST_ATTR, ValueType::Reference)])
        .define(12, &[])
        .define(13, &[(geo::DUNGEON_POSITION_ATTR, ValueType::Reference)])
        .define(14, &[(geo::REGION_TABLE_ATTR, ValueType::Reference)])
        .define(15, &[])
        .parent(13, 10)
        .singleton(14)
        .build();
    let mut pos = Vec::new();
    put_position(&mut pos, 2, (100, 50), 0, [10.0, 20.0, 30.0]);
    let mut table = Vec::new();
    put_tsize(&mut table, 0, 1);
    table.extend(le(&[3, 2]));
    let entry = {
        let mut s = Vec::new();
        put_tsize(&mut s, 0, 2);
        s.extend(le(&[P_REGION_ID, P_REGION_ID, 3]));
        s.extend(le(&[P_REGION_NAME, P_REGION_NAME]));
        s.extend(si("North"));
        s
    };
    let mut list = le(&[1, P_REGION_ENTRY]);
    list.extend(entry);

    let mut src = MemorySource::new()
        .with(0x3400_0000, dict)
        .with(LIBRARY_DID.0, lib)
        .with(
            0x2800_0000,
            did_map(
                0x2800_0000,
                &[
                    (0xA, 0x2800_000E, "WEENIECONTENT"),
                    (0xB, 0x2800_000F, "WLIB"),
                ],
            ),
        )
        .with(
            0x2800_000E,
            did_map(0x2800_000E, &[(1, CONTROL, "RegionControl")]),
        )
        .with(
            0x2800_000F,
            did_map(0x2800_000F, &[(1, LIBRARY_DID.0, "Client")]),
        );
    let records = [
        (
            AREA,
            RecordBuilder::new(AREA, 10)
                .class(10, &[])
                .script(0, 10, &[]),
        ),
        (
            TERRITORY,
            RecordBuilder::new(TERRITORY, 11)
                .class(11, &[(geo::TERRITORY_LIST_ATTR, ValueType::Reference)])
                .script(0, 11, &[Value::Ref(Ref(1))])
                .object(1, 25, &le(&[1, AREA])),
        ),
        (
            REGION,
            RecordBuilder::new(REGION, 12)
                .class(12, &[])
                .script(0, 12, &[])
                .import(3, AREA, 0),
        ),
        (
            DUNGEON,
            RecordBuilder::new(DUNGEON, 13)
                .class(13, &[(geo::DUNGEON_POSITION_ATTR, ValueType::Reference)])
                .script(0, 13, &[Value::Ref(Ref(1))])
                .object(1, 160, &pos),
        ),
        (
            SUB_DUNGEON,
            RecordBuilder::new(SUB_DUNGEON, 13)
                .class(13, &[(geo::DUNGEON_POSITION_ATTR, ValueType::Reference)])
                .script(0, 13, &[Value::Ref(Ref::NULL)])
                .did_item(2, DUNGEON),
        ),
        (
            CONTROL,
            RecordBuilder::new(CONTROL, 14)
                .class(14, &[(geo::REGION_TABLE_ATTR, ValueType::Reference)])
                .script(0, 14, &[Value::Ref(Ref(1))])
                .object(1, 35, &table)
                .object(2, 199, &si("North")),
        ),
        (
            OTHER,
            RecordBuilder::new(OTHER, 15)
                .class(15, &[])
                .script(0, 15, &[]),
        ),
    ];
    for (did, b) in records {
        src.insert(did, b.build());
    }
    let set = |did: u32, entries: &[(u32, Vec<u8>)]| {
        let d = did + 0x0900_0000;
        (d, property_set_record(d, entries))
    };
    for (did, bytes) in [
        set(
            AREA,
            &[
                (P_AREA_NAME, si("Ashdale")),
                (P_AREA_TERRITORY, le(&[TERRITORY])),
            ],
        ),
        set(
            TERRITORY,
            &[
                (P_AREA_NAME, si("Eastfold")),
                (P_AREA_REGION, le(&[REGION])),
            ],
        ),
        set(
            REGION,
            &[(P_AREA_NAME, si("North")), (P_AREA_REGION_ID, le(&[3]))],
        ),
        set(
            DUNGEON,
            &[
                (P_DUNGEON_NAME, si("Deep")),
                (P_DUNGEON_MAP, le(&[0x7800_0001])),
                (P_DUNGEON_PARENT, le(&[SUB_DUNGEON])),
            ],
        ),
        // The two dungeons name each other as parents: a cycle.
        set(
            SUB_DUNGEON,
            &[
                (P_DUNGEON_NAME, si("Lower  DEEP")),
                (P_DUNGEON_PARENT, le(&[DUNGEON])),
            ],
        ),
        set(CONTROL, &[(P_REGION_LIST, list)]),
        set(OTHER, &[]),
    ] {
        src.insert(did, bytes);
    }
    src
}

/// [`MemorySource`] behind an edit plan.
struct Damaged {
    base: MemorySource,
    mutator: Mutator,
}

impl RecordSource for Damaged {
    fn read_record(&self, did: Did) -> Result<Option<Vec<u8>>, SourceError> {
        let base = || self.base.read_record(did).ok().flatten().map(|b| (b, None));
        match self.mutator.read(did.0, base) {
            Read::Ok(r) => Ok(r.map(|(b, _)| b)),
            Read::Fail => Err(Box::new(InjectedFailure)),
        }
    }
}

fuzz_target!(|data: &[u8]| {
    let (plan, rest) = mutate::plan(data);
    let byte = |i: usize| rest.get(i).copied().unwrap_or(0);
    let word = |i: usize| u32::from_le_bytes([byte(i), byte(i + 1), byte(i + 2), byte(i + 3)]);
    let mut store = ObjectStore::new(Damaged {
        base: base(),
        mutator: Mutator::new(plan),
    });
    if byte(0) & 1 == 1 {
        // A tiny cache: eviction on every insert.
        store = store.with_cache_limits(CacheLimits {
            records: usize::from(byte(1) % 4),
            bytes: usize::from(byte(2)) * 64,
            max_payload: usize::from(byte(3)) * 16,
        });
    }
    let extra = 0x7000_0000 | (word(4) & 0x07FF_FFFF);
    let name = String::from_utf8_lossy(rest.get(8..).unwrap_or_default()).into_owned();

    let _ = store.library_did();
    let _ = store.library();
    for did in ALL.into_iter().chain([extra, word(4)]) {
        let did = Did(did);
        let _ = store.header(did);
        let _ = store.imports(did);
        let _ = store.state(did);
        let _ = store.object(did);
        let _ = store.properties(did);
        let _ = store.follow_did(did);
    }
    let _ = store.weenie("RegionControl");
    let _ = store.weenie(&name);
    let _ = store.singleton(14);
    let _ = store.singleton(u32::from(byte(1)));
    let _ = store.cached();
    let _ = store.cached_bytes();

    let Ok(geo) = Geo::new(&store) else {
        return;
    };
    for did in ALL.into_iter().chain([extra]) {
        let did = Did(did);
        let _ = geo.kind(did);
        let _ = geo.area(did);
        let _ = geo.territory(did);
        let _ = geo.region(did);
        let _ = geo.dungeon(did);
        let _ = geo.dungeon_position(did);
        let _ = geo.area_chain(did);
    }
    let _ = geo.region_control();
    let _ = geo.region_table();
    let _ = geo.region_table_from_state();

    let strings = Strings::new(tessera_text::MemorySource::new());
    let dids = ALL.into_iter().chain([extra]).map(Did);
    let (index, report) = GeoIndex::build(&geo, Some(&strings), dids.clone());
    assert_eq!(report.scanned, ALL.len() + 1);
    assert_eq!(
        report.scanned,
        report.absent
            + report.failed
            + report.no_properties
            + report.skipped
            + report.properties_read
    );
    assert!(index.len() <= report.scanned);
    // The incremental builder in two chunks, without strings.
    let mut b = GeoIndexBuilder::new();
    let all: Vec<Did> = dids.collect();
    let (first, second) = all.split_at(usize::from(byte(5)) % (all.len() + 1));
    b.scan_all(&geo, None, first.iter().copied());
    b.scan_all(&geo, None, second.iter().copied());
    let (index2, report2) = b.finish();
    assert_eq!(report2.scanned, report.scanned);
    for kind in GeoKind::ALL {
        let _ = index.of_kind(kind).count();
        let _ = index.classes(kind);
        let _ = index.find_kind(&name, kind);
    }
    let _ = index.find(&name);
    let _ = index.search(&name).take(64).count();
    let _ = index2.search(&geo::normalize_name(&name)).take(64).count();
    for e in index.iter() {
        assert_eq!(index.get(e.did).map(|g| g.did), Some(e.did));
        let _ = index.name(e.did);
    }
});
