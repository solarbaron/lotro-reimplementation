// SPDX-License-Identifier: GPL-3.0-or-later
//! `ui-layout.md` against a user's own install (read-only): ROADMAP M5 exit criterion 1.
//!
//! Decodes **every** record with prefix 0x22 and reports the counts per §13 failure
//! category, the §11 census, the §7.1 base-chain census, the §10 reference census and the
//! §3.3 label resolution, and checks the invariants of §12 that need no run-time layout
//! (UL1–UL14) and UL21. The §3.2 / §11 / §10 / §13 numbers are checked against the local
//! baseline of the install version (decision 0045, `tessera-testkit`), with the spec's
//! figures beside them (drift reports, `TESSERA_CENSUS_STRICT=1`).
//!
//! Not here: UL15 (override pairs), UL16–UL20 and UL22 need the §7 merge, the §8–§9
//! arithmetic or the §17 panels, so they are tested by `tessera-ui`.
//!
//! Skipped unless `TESSERA_GAME_DIR` points at an install. Run with
//! `cargo test --release -p tessera-formats --test ui_layout_real_data -- --nocapture`.
//! Only counts, DIDs and error kinds are printed; no game data leaves the install.

use std::collections::{BTreeMap, BTreeSet, HashMap};
use std::path::PathBuf;

use tessera_dat::{ArchiveSet, SetOptions};
use tessera_formats::ui_layout::{
    FailureCategory, LayoutRef, MediaItem, MediaKind, UiEnum, UiLabels, UiLayoutCensus, base_chain,
};
use tessera_formats::{
    DataIdMap, Did, EnumMapper, PropertyDictionary, Record, StringSource, StringTable, UiLayout,
};

const PROPERTY_DICTIONARY: u32 = 0x3400_0000;
const ROOT_DID_MAP: u32 = 0x2800_0000;
/// Record count and total payload size of the install the spec's census was taken on (§11),
/// shown beside the observed ones.
const SPEC_RECORDS: u64 = 302;
const SPEC_BYTES: u64 = 2_610_590;

fn open_set() -> Option<ArchiveSet> {
    let dir = match std::env::var_os("TESSERA_GAME_DIR") {
        Some(d) if !d.is_empty() => PathBuf::from(d),
        _ => {
            eprintln!("TESSERA_GAME_DIR not set; skipping");
            return None;
        }
    };
    Some(ArchiveSet::open(&dir, SetOptions::default()).expect("open archive set"))
}

fn exists(set: &ArchiveSet, did: Did) -> bool {
    matches!(set.locate(did.0), Ok(Some(_)))
}

fn did_map(set: &ArchiveSet, did: Did) -> Option<DataIdMap> {
    let payload = set.read(did.0).ok()?;
    DataIdMap::parse_expecting(&payload, did).ok()
}

fn labels(set: &ArchiveSet) -> UiLabels {
    let root = did_map(set, Did(ROOT_DID_MAP)).expect("root DID map");
    let emapper = root
        .by_label("EMAPPER")
        .and_then(|d| did_map(set, d))
        .expect("EMAPPER map");
    UiLabels::load(
        |label| emapper.by_label(label),
        |did| {
            let payload = set.read(did.0).ok()?;
            EnumMapper::parse_expecting(&payload, did).ok()
        },
    )
}

#[derive(Default)]
struct Report {
    /// Per §13 category: (count, example DIDs).
    failures: BTreeMap<FailureCategory, (u64, BTreeSet<u32>)>,
    /// Records with a based element whose base element is missing.
    element_missing_records: BTreeSet<u32>,
}

impl Report {
    fn add(&mut self, c: FailureCategory, did: u32) {
        let e = self.failures.entry(c).or_default();
        e.0 += 1;
        e.1.insert(did);
    }
    fn count(&self, c: FailureCategory) -> u64 {
        self.failures.get(&c).map_or(0, |e| e.0)
    }
}

#[test]
fn every_ui_layout_decodes_with_the_census() {
    let Some(set) = open_set() else { return };
    let dict = {
        let payload = set.read(PROPERTY_DICTIONARY).expect("read the dictionary");
        PropertyDictionary::parse_expecting(&payload, PROPERTY_DICTIONARY).expect("dictionary")
    };

    // §3.1: where the records are.
    let mut per_archive: BTreeMap<String, u64> = BTreeMap::new();
    let mut dids = BTreeSet::new();
    for m in set.members() {
        for e in m.archive.entries_range(0x2200_0000..=0x22FF_FFFF) {
            let e = e.expect("directory walk");
            *per_archive.entry(m.id.file_name()).or_default() += 1;
            dids.insert(e.did);
        }
    }
    println!("UI layout records per archive: {per_archive:?}");

    let mut report = Report::default();
    let mut layouts: HashMap<Did, UiLayout> = HashMap::new();
    let mut census = UiLayoutCensus::default();
    let mut issues: BTreeMap<&'static str, (u64, BTreeSet<u32>)> = BTreeMap::new();
    for &did in &dids {
        let payload = set.read(did).expect("read layout");
        census.bytes += payload.len() as u64;
        match UiLayout::parse_expecting(&payload, did, &dict) {
            Ok(l) => {
                census.add(&l.census());
                for i in l.issues(usize::MAX) {
                    let e = issues.entry(i.invariant()).or_default();
                    e.0 += 1;
                    e.1.insert(did);
                }
                layouts.insert(Did(did), l);
            }
            Err(e) => {
                let c = FailureCategory::of_decode_error(&e);
                println!("  {did:#010x}: {c:?}: {e}");
                report.add(c, did);
            }
        }
    }
    let decoded = layouts.len() as u64;
    let records = dids.len() as u64;

    // §7.1: base chains of every stored based element; §13 resolution categories.
    let mut depths: BTreeMap<usize, u64> = BTreeMap::new();
    let lookup = |d: Did| layouts.get(&d);
    let mut sorted: Vec<&UiLayout> = layouts.values().collect();
    sorted.sort_by_key(|l| l.did);
    for l in &sorted {
        l.walk(|e, _| {
            if e.base().is_none() {
                return;
            }
            match base_chain(e, lookup) {
                Ok(chain) => *depths.entry(chain.depth()).or_default() += 1,
                Err(err) => {
                    let c = err.category();
                    report.add(c, l.did.0);
                    if c == FailureCategory::BaseElementMissing {
                        report.element_missing_records.insert(l.did.0);
                    }
                }
            }
        });
    }

    // §10: references. Media and property DIDs, StringInfo values.
    let mut by_prefix: BTreeMap<u8, u64> = BTreeMap::new();
    let mut zero_refs: BTreeMap<&'static str, u64> = BTreeMap::new();
    let mut nonzero = 0u64;
    let mut checked: HashMap<Did, bool> = HashMap::new();
    let mut string_values = 0u64;
    let mut literals = 0u64;
    let mut pairs: BTreeSet<(u32, u32)> = BTreeSet::new();
    let mut font_dids: BTreeSet<Did> = BTreeSet::new();
    let mut base_layouts: BTreeSet<Did> = BTreeSet::new();
    let mut property_layouts: BTreeSet<Did> = BTreeSet::new();
    for l in &sorted {
        l.for_each_reference(|r| {
            let did = match r {
                LayoutRef::Media { kind, did } => {
                    if did.is_null() {
                        *zero_refs.entry(kind.name()).or_default() += 1;
                        return;
                    }
                    did
                }
                LayoutRef::Property { did, .. } => {
                    if did.is_null() {
                        *zero_refs.entry("DID property").or_default() += 1;
                        return;
                    }
                    if did.prefix() == 0x22 {
                        property_layouts.insert(did);
                    }
                    did
                }
                LayoutRef::String { source, .. } => {
                    string_values += 1;
                    match source {
                        StringSource::Literal(_) => literals += 1,
                        StringSource::Table { token, table } => {
                            pairs.insert((table.0, *token));
                            if table.is_null() {
                                if *token == 0 {
                                    *zero_refs.entry("StringInfo (0, 0)").or_default() += 1;
                                }
                                return;
                            }
                            *by_prefix.entry(table.prefix()).or_default() += 1;
                            nonzero += 1;
                            if !*checked
                                .entry(*table)
                                .or_insert_with(|| exists(&set, *table))
                            {
                                report.add(FailureCategory::MissingReferencedRecord, l.did.0);
                            }
                        }
                    }
                    return;
                }
                LayoutRef::Base(b) => {
                    base_layouts.insert(b.layout);
                    return;
                }
            };
            nonzero += 1;
            *by_prefix.entry(did.prefix()).or_default() += 1;
            if did.prefix() == 0x42 {
                font_dids.insert(did);
            }
            if !*checked.entry(did).or_insert_with(|| exists(&set, did)) {
                report.add(FailureCategory::MissingReferencedRecord, l.did.0);
            }
        });
    }
    // String-table tokens (§10): every distinct (table, token) pair with a non-zero table.
    let mut tables: HashMap<u32, Option<StringTable>> = HashMap::new();
    let mut resolved_pairs = 0u64;
    let mut missing_pairs = 0u64;
    for &(table, token) in &pairs {
        if table == 0 {
            continue;
        }
        let t = tables.entry(table).or_insert_with(|| {
            let payload = set.read(table).ok()?;
            StringTable::parse_expecting(&payload, table).ok()
        });
        if t.as_ref().is_some_and(|t| t.get(token).is_some()) {
            resolved_pairs += 1;
        } else {
            missing_pairs += 1;
            report.add(FailureCategory::MissingStringToken, table);
        }
    }

    // §3.3 / UL11: labels of every ID field.
    let labels = labels(&set);
    let mut unlabelled: BTreeMap<&'static str, u64> = BTreeMap::new();
    let mut engine_types: BTreeMap<String, u64> = BTreeMap::new();
    let mut label_checks = 0u64;
    {
        let mut check = |what: &'static str, which: UiEnum, key: u32| {
            label_checks += 1;
            if labels.label(which, key).is_none() {
                *unlabelled.entry(what).or_default() += 1;
            }
        };
        for l in &sorted {
            l.walk(|e, _| {
                check("element ID", UiEnum::ElementId, e.element_id);
                if e.element_type != 0 {
                    check("type", UiEnum::ElementType, e.element_type);
                }
                if e.default_state != 0 {
                    check("default state", UiEnum::StateId, e.default_state);
                }
                if e.base_element != 0 {
                    check("base element", UiEnum::ElementId, e.base_element);
                }
                for s in &e.states {
                    check("state ID", UiEnum::StateId, s.state_id);
                }
                for item in e.blocks().flat_map(|b| b.media.iter()) {
                    if let Some(m) = item.draw_mode() {
                        check("draw mode", UiEnum::DrawMode, m.0);
                    }
                    match item {
                        MediaItem::State { state, .. } => {
                            check("State item", UiEnum::StateId, *state);
                        }
                        MediaItem::Message { message, .. } => {
                            check("message", UiEnum::Message, *message);
                        }
                        _ => {}
                    }
                }
            });
        }
    }
    for l in &sorted {
        l.walk(|e, _| {
            if e.element_type != 0 && e.element_type < 0x1000_0000 {
                let name = labels
                    .element_type(e.element_type)
                    .map_or_else(|| format!("{:#x}", e.element_type), str::to_owned);
                *engine_types.entry(name).or_default() += 1;
            }
        });
    }

    // Report.
    println!(
        "UI layouts: {records} records, {} bytes, {decoded} decode",
        census.bytes
    );
    println!("§13 failure categories:");
    for c in FailureCategory::ALL {
        let (n, dids) = report.failures.get(&c).cloned().unwrap_or_default();
        let ex: Vec<String> = dids.iter().take(4).map(|d| format!("{d:#010x}")).collect();
        let note = if c == FailureCategory::TypeWithoutWidget {
            " (decided by tessera-ui)"
        } else {
            ""
        };
        println!("  {:<58} {n:>6}{note} {ex:?}", c.name());
    }
    println!(
        "  base-element-missing records: {}",
        report.element_missing_records.len()
    );
    println!("§11 census: {census:#?}");
    let kinds: Vec<String> = MediaKind::ALL
        .iter()
        .map(|k| format!("{} {}", k.name(), census.media_by_kind[k.code() as usize]))
        .collect();
    println!("media by kind: {}", kinds.join(", "));
    println!("engine widget types: {engine_types:?}");
    println!("§7.1 chain depths: {depths:?}");
    println!(
        "§10: {nonzero} non-zero references by prefix {:?}",
        by_prefix
            .iter()
            .map(|(p, n)| format!("{p:#04x}: {n}"))
            .collect::<Vec<_>>()
    );
    println!("§10: zero references {zero_refs:?}");
    println!(
        "§10: {string_values} StringInfo values ({literals} literal), {} distinct pairs: {resolved_pairs} resolve, {missing_pairs} missing token, {} (0, 0)",
        pairs.len(),
        pairs.iter().filter(|p| p.0 == 0).count()
    );
    println!(
        "FN12: {} distinct font DIDs named by layouts",
        font_dids.len()
    );
    println!("§3.3: {label_checks} ID fields checked, unlabelled {unlabelled:?}");
    println!("§12 issues (UL5-UL10, UL14): {issues:?}");

    // Invariants (any install).
    assert_eq!(
        per_archive.keys().filter(|k| k.contains("general")).count(),
        0,
        "U21"
    );
    assert_eq!(
        report.count(FailureCategory::RecordDoesNotDecode)
            + report.count(FailureCategory::UnknownMediaKind),
        0,
        "every record decodes (UL1-UL4)"
    );
    assert!(issues.is_empty(), "UL5-UL10, UL14: {issues:?}");
    assert!(unlabelled.is_empty(), "UL11: {unlabelled:?}");
    assert_eq!(
        report.count(FailureCategory::MissingReferencedRecord),
        0,
        "UL12"
    );
    assert_eq!(report.count(FailureCategory::BaseLayoutMissing), 0, "UL13");
    assert_eq!(report.count(FailureCategory::BaseChainCycle), 0);
    let based = census.based;
    let failed = report.count(FailureCategory::BaseElementMissing);
    assert!(
        failed * 1000 <= based,
        "UL13: {failed} of {based} base references fail"
    );

    // The counts of §3.2, §7.1, §10, §11 and §13, against the local baseline of the install
    // version (decision 0045), with the spec's figures beside them.
    let dir = PathBuf::from(std::env::var_os("TESSERA_GAME_DIR").expect("install"));
    let mut c =
        tessera_testkit::census!("every_ui_layout_decodes_with_the_census", &dir).expect("census");
    c.count_vs_spec("§11 records", records, SPEC_RECORDS);
    c.count_vs_spec("§11 bytes", census.bytes, SPEC_BYTES);
    census.records = records;
    for (what, got, spec) in [
        ("top-level elements", census.top_level, 3_144),
        (
            "most top-level elements in a record",
            census.max_top_level,
            450,
        ),
        (
            "records with one top-level element",
            census.single_top_level,
            111,
        ),
        ("elements", census.elements, 16_421),
        (
            "most elements in a record",
            census.max_elements_per_record,
            1_533,
        ),
        ("deepest element", census.max_depth, 7),
        ("most children", census.max_children, 339),
        ("typed elements", census.typed, 7_804),
        ("based elements", census.based, 8_617),
        ("states", census.states, 12_977),
        ("most states", census.max_states, 139),
        (
            "elements without states",
            census.elements_without_states,
            13_970,
        ),
        ("media in elements", census.media_in_elements, 3_280),
        ("media in states", census.media_in_states, 9_718),
        ("media", census.media(), 12_998),
        ("element properties", census.element_properties, 35_615),
        ("state properties", census.state_properties, 2_083),
        ("default states", census.default_states, 1_212),
        ("elements without a position", census.without_position, 222),
        ("media paths", census.media_paths, 11_367),
        ("longest media path", census.max_media_path, 48),
    ] {
        c.count_vs_spec(&format!("§11 {what}"), got, spec);
    }
    let k = |kind: MediaKind| census.media_by_kind[kind.code() as usize];
    for (kind, spec) in [
        (MediaKind::Movie, 2),
        (MediaKind::Jump, 0),
        (MediaKind::Image, 11_365),
        (MediaKind::Message, 31),
        (MediaKind::Fade, 36),
        (MediaKind::HitMask, 0),
        (MediaKind::Animation, 133),
        (MediaKind::ColouredImage, 421),
        (MediaKind::Cursor, 180),
        (MediaKind::Sound, 692),
        (MediaKind::State, 86),
        (MediaKind::Pause, 52),
    ] {
        c.count_vs_spec(
            &format!("§6.1 media of kind {}", kind.name()),
            k(kind),
            spec,
        );
    }
    let expected_types = [
        ("Field", 4_796),
        ("Button", 700),
        ("Text", 346),
        ("ListBox", 193),
        ("Dragbar", 135),
        ("Meter", 88),
        ("Panel", 73),
        ("Tree", 59),
        ("ResizeBar", 52),
        ("GroupBox", 13),
        ("Scrollbar", 13),
        ("Browser", 7),
        ("ColorPicker", 4),
        ("Menu", 4),
        ("Viewport", 1),
    ];
    for (name, n) in expected_types {
        c.count_vs_spec(
            &format!("§11 elements of type {name}"),
            engine_types.get(name).copied().unwrap_or(0),
            n,
        );
    }
    for (name, &n) in &engine_types {
        if !expected_types.iter().any(|(t, _)| t == name) {
            c.count(&format!("§11 elements of type {name}"), n);
        }
    }
    // §7.1 and §13.
    let spec_depths = BTreeMap::from([(1, 7_076), (2, 1_494), (3, 44)]);
    let all_depths: BTreeSet<usize> = spec_depths.keys().chain(depths.keys()).copied().collect();
    for d in all_depths {
        let name = format!("§7.1 base chains of depth {d}");
        let got = depths.get(&d).copied().unwrap_or(0);
        match spec_depths.get(&d) {
            Some(&spec) => c.count_vs_spec(&name, got, spec),
            None => c.count(&name, got),
        }
    }
    c.count_vs_spec("§13 base element missing", failed, 3);
    c.count_vs_spec(
        "§13 records with a missing base element",
        report.element_missing_records.len(),
        2,
    );
    c.count_vs_spec("§13 missing string-table tokens", missing_pairs, 2);
    // §10.
    c.count_vs_spec("§10 non-zero references", nonzero, 27_512);
    let spec_prefix = BTreeMap::from([
        (0x21, 20),
        (0x22, 4_043),
        (0x25, 5_367),
        (0x2A, 694),
        (0x40, 2),
        (0x41, 15_160),
        (0x42, 1_230),
        (0x70, 996),
    ]);
    let prefixes: BTreeSet<u8> = spec_prefix
        .keys()
        .chain(by_prefix.keys())
        .copied()
        .collect();
    for p in prefixes {
        let name = format!("§10 references to prefix {p:#04x}");
        let got = by_prefix.get(&p).copied().unwrap_or(0);
        match spec_prefix.get(&p) {
            Some(&spec) => c.count_vs_spec(&name, got, spec),
            None => c.count(&name, got),
        }
    }
    let spec_zero = BTreeMap::from([
        ("ColouredImage", 392),
        ("DID property", 10),
        ("Image", 805),
        ("Movie", 2),
        ("StringInfo (0, 0)", 115),
    ]);
    let zero_kinds: BTreeSet<&str> = spec_zero.keys().chain(zero_refs.keys()).copied().collect();
    for z in zero_kinds {
        let name = format!("§10 zero references of {z}");
        let got = zero_refs.get(z).copied().unwrap_or(0);
        match spec_zero.get(z) {
            Some(&spec) => c.count_vs_spec(&name, got, spec),
            None => c.count(&name, got),
        }
    }
    c.count_vs_spec("§10 StringInfo values", string_values, 5_478);
    c.count_vs_spec("§10 literal strings", literals, 1);
    c.count_vs_spec("§10 distinct pairs", pairs.len(), 3_329);
    c.count_vs_spec("§10 resolved pairs", resolved_pairs, 3_326);
    c.count_vs_spec("FN12 distinct font DIDs", font_dids.len(), 37);
    // §3.3: 16,421 element IDs + 12,977 state IDs + 7,804 types + 1,212 default states +
    // 8,617 base element IDs + 11,919 draw modes + 86 State-item states + 31 messages.
    c.count_vs_spec("§3.3 ID fields", label_checks, 59_067);

    // §3.2: the UILAYOUT map and how the layouts are reached.
    let root = did_map(&set, Did(ROOT_DID_MAP)).expect("root DID map");
    let uilayout = root
        .by_label("UILAYOUT")
        .and_then(|d| did_map(&set, d))
        .expect("UILAYOUT map");
    let mapped: BTreeSet<Did> = uilayout
        .iter()
        .map(|e| e.value)
        .filter(|d| d.prefix() == 0x22)
        .collect();
    let mapped_entries = uilayout.iter().filter(|e| e.value.prefix() == 0x22).count();
    println!(
        "§3.2: UILAYOUT {} entries, {mapped_entries} naming 0x22 ({} distinct); {} base layouts, {} property-named layouts",
        uilayout.len(),
        mapped.len(),
        base_layouts.len(),
        property_layouts.len()
    );
    c.count_vs_spec("§3.2 UILAYOUT entries", uilayout.len(), 236);
    c.count_vs_spec("§3.2 UILAYOUT entries naming 0x22", mapped_entries, 232);
    c.count_vs_spec("§3.2 distinct 0x22 layouts in UILAYOUT", mapped.len(), 232);
    c.count_vs_spec("§3.2 base layouts", base_layouts.len(), 177);
    c.count_vs_spec("§3.2 property-named layouts", property_layouts.len(), 72);
    let reached_by_none = dids
        .iter()
        .map(|&d| Did(d))
        .filter(|d| {
            !mapped.contains(d) && !base_layouts.contains(d) && !property_layouts.contains(d)
        })
        .count();
    c.count_vs_spec("§3.2 layouts reached by none", reached_by_none, 1);
    c.finish();

    // UL21: `containerdisplay` holds PlayerBag1..6_Template, each resolving to UIScaler and
    // none named by a base reference.
    let container = uilayout
        .iter()
        .find(|e| &*e.label == "containerdisplay")
        .map(|e| e.value)
        .expect("UL21: containerdisplay entry");
    let container_layout = layouts.get(&container).expect("UL21: layout decodes");
    let lookup = |d: Did| layouts.get(&d);
    for n in 1..=6 {
        let name = format!("PlayerBag{n}_Template");
        let id = labels
            .key(UiEnum::ElementId, &name)
            .unwrap_or_else(|| panic!("UL21: {name} label"));
        let e = container_layout
            .top_level(id)
            .unwrap_or_else(|| panic!("UL21: {name} is top-level"));
        let chain = base_chain(e, lookup).unwrap_or_else(|err| panic!("UL21: {name}: {err}"));
        let end = chain.end().unwrap_or(e);
        assert_eq!(
            labels.element_type(end.element_type),
            Some("UIScaler"),
            "UL21: {name} type"
        );
        let target = tessera_formats::ui_layout::BaseRef {
            element: id,
            layout: container,
        };
        let mut named = 0;
        for l in &sorted {
            l.walk(|x, _| named += usize::from(x.base() == Some(target)));
        }
        assert_eq!(named, 0, "UL21: {name} is not a base");
    }
}
