// SPDX-License-Identifier: GPL-3.0-or-later
//! The install's answers to the login's initial exchange (`official-session.md` §9.3.2, §9.3.7)
//! over synthetic archives and records: the full language key through the Languages enum and its
//! base chain, the name rules in both map groups, the identifier's two halves and every outcome
//! of the preflight inventory. Every byte is generated here; no game data is involved.

use std::path::Path;

use tessera_client::official::install_data::{
    LANGUAGE_CHAIN_LIMIT, advertised_archive, archive_inventory, initial_inputs,
    language_key_lookup, name_rules_lookup, region_keys_lookup, uses_older_identifier,
};
use tessera_compat::initial::{
    InstallUnsupported, UnsupportedNegotiation, build_install_advertisement,
    check_language_allowed, fold_language_code,
};
use tessera_dat::{ArchiveSet, SetOptions};
use tessera_formats::Did;

#[path = "../../../crates/dat/tests/common/mod.rs"]
mod dat_common;

use dat_common::{Builder, Layout, Rec};

const ITERATION_DID: u32 = 0xFFFF_0001;
const MASTER_MAP: u32 = 0x2800_0000;
const ENUM_MAPS: u32 = 0x2800_0001;
const NAME_MAPS: u32 = 0x2800_0002;
const REGION_MAP: u32 = 0x2800_0008;
const LANGUAGES: u32 = 0x2300_0001;
const LANGUAGES_BASE: u32 = 0x2300_0002;
const NAME_TABLE: u32 = 0x0E00_0001;

const DE_KEY: u32 = 0x1000_0004;
const SET_ID: [u8; 16] = [0x5A; 16];

/// `N = 3`, one run from 7, `K = 1`: the highest listed (`install-layout.md` §9.3, §9.6).
fn iteration_record() -> Vec<u8> {
    let mut out = Vec::new();
    for v in [3i32, -3, 7, 1, 9] {
        out.extend_from_slice(&v.to_le_bytes());
    }
    out.extend_from_slice(&[0xAB; 8]);
    out
}

fn iteration(stamp: u16, stored: Vec<u8>) -> Rec {
    Rec {
        did: ITERATION_DID,
        flags: 2,
        stored,
        layout: Layout::Plain,
        stamp,
        time: 1_700_000_000,
        iteration: 0,
    }
}

/// A base archive with a usable iteration record.
fn base(dataset: u32, subset: u32, parts: u32) -> Builder {
    let mut b = Builder::new();
    b.dataset = dataset;
    b.subset = subset;
    b.membership = Some((SET_ID, 0, parts));
    b.records.push(iteration(2, iteration_record()));
    b
}

fn aux(dataset: u32, subset: u32, part: u32, parts: u32) -> Builder {
    let mut b = Builder::new();
    b.dataset = dataset;
    b.subset = subset;
    b.membership = Some((SET_ID, part, parts));
    b
}

fn write(dir: &Path, name: &str, b: &Builder) {
    std::fs::write(dir.join(name), &b.build().bytes).expect("written");
}

/// One group of a DID map: `(key, target, label)`.
fn group(out: &mut Vec<u8>, entries: &[(u32, u32, &str)]) {
    let n = u8::try_from(entries.len()).expect("small");
    out.extend_from_slice(&[0, n]);
    for (k, t, _) in entries {
        out.extend_from_slice(&k.to_le_bytes());
        out.extend_from_slice(&t.to_le_bytes());
    }
    out.extend_from_slice(&[0, n]);
    for (k, _, label) in entries {
        out.extend_from_slice(&k.to_le_bytes());
        out.push(u8::try_from(label.len()).expect("small"));
        out.extend_from_slice(label.as_bytes());
    }
}

fn did_map(did: u32, groups: &[&[(u32, u32, &str)]]) -> Vec<u8> {
    let mut out = did.to_le_bytes().to_vec();
    for g in groups {
        group(&mut out, g);
    }
    out
}

fn enum_mapper(did: u32, base: u32, names: &[(u32, &str)]) -> Vec<u8> {
    let mut out = did.to_le_bytes().to_vec();
    out.extend_from_slice(&base.to_le_bytes());
    out.extend_from_slice(&[0, u8::try_from(names.len()).expect("small")]);
    for (k, n) in names {
        out.extend_from_slice(&k.to_le_bytes());
        out.push(u8::try_from(n.len()).expect("small"));
        out.extend_from_slice(n.as_bytes());
    }
    out.extend_from_slice(&[0, 0]);
    out
}

/// A name-filter table of `rules` (`misc-records.md` §9): the fields this path does not read are
/// empty.
fn name_table(rules: &[(u32, &[u32])]) -> Vec<u8> {
    let mut out = NAME_TABLE.to_le_bytes().to_vec();
    out.extend_from_slice(&[0, u8::try_from(rules.len()).expect("small")]);
    for (key, languages) in rules {
        out.extend_from_slice(&key.to_le_bytes());
        out.extend_from_slice(&[0u8; 7 * 4]);
        out.extend_from_slice(&0u32.to_le_bytes());
        out.extend_from_slice(&[0, 0]);
        out.extend_from_slice(&[0, 0]);
        out.push(0);
        out.extend_from_slice(&u32::try_from(languages.len()).expect("small").to_le_bytes());
        for l in *languages {
            out.extend_from_slice(&l.to_le_bytes());
        }
        out.push(0);
    }
    out
}

/// The in-memory records of a healthy install: the Languages entry sits in the base of the
/// chain and the name table's sub-key in the second map group.
fn records() -> std::collections::BTreeMap<u32, Vec<u8>> {
    let mut m = std::collections::BTreeMap::new();
    m.insert(
        MASTER_MAP,
        did_map(
            MASTER_MAP,
            &[
                &[(0x07, ENUM_MAPS, "EnumMappers")],
                &[(0x0F, NAME_MAPS, "NameFilter")],
            ],
        ),
    );
    m.insert(
        ENUM_MAPS,
        did_map(
            ENUM_MAPS,
            &[&[
                (0x4A, LANGUAGES, "Languages"),
                (0x99, REGION_MAP, "RegionID"),
            ]],
        ),
    );
    m.insert(
        NAME_MAPS,
        did_map(
            NAME_MAPS,
            &[&[(9, 0x0E00_0009, "x")], &[(1, NAME_TABLE, "y")]],
        ),
    );
    m.insert(
        REGION_MAP,
        did_map(
            REGION_MAP,
            &[&[(0, 0x1, "none"), (1, 0x1, "one"), (2, 0x1, "two")]],
        ),
    );
    m.insert(
        LANGUAGES,
        enum_mapper(LANGUAGES, LANGUAGES_BASE, &[(2, "English")]),
    );
    m.insert(
        LANGUAGES_BASE,
        enum_mapper(LANGUAGES_BASE, 0, &[(DE_KEY, "DE"), (0x1000_0003, "FR")]),
    );
    m.insert(
        NAME_TABLE,
        name_table(&[(7, &[2, DE_KEY]), (8, &[]), (9, &[0x1004])]),
    );
    m
}

fn reader(m: std::collections::BTreeMap<u32, Vec<u8>>) -> impl Fn(Did) -> Option<Vec<u8>> {
    move |did: Did| m.get(&did.0).cloned()
}

/// Writes a healthy install: every portal set, region 1 and the selected language `DE`, with the
/// install's records in the general archive. The mesh set has one auxiliary part.
fn healthy(dir: &Path) {
    install(dir, records(), "client_local_DE.dat", 0x1004);
}

/// [`healthy`] with its own records and language archive.
fn install(
    dir: &Path,
    records: std::collections::BTreeMap<u32, Vec<u8>>,
    language_file: &str,
    subset: u32,
) {
    let mut general = base(0x0103_0000, 0, 1);
    for (did, bytes) in records {
        general.records.push(Rec {
            did,
            flags: 2,
            stored: bytes,
            layout: Layout::Plain,
            stamp: 1,
            time: 1_700_000_000,
            iteration: 0,
        });
    }
    let mut built = general.build();
    built.put_u32(0x170, MASTER_MAP);
    std::fs::write(dir.join("client_general.dat"), &built.bytes).expect("written");
    for (name, dataset) in [
        ("client_gamelogic.dat", 0x0101_0000u32),
        ("client_surface.dat", 0x0109_0000),
        ("client_sound.dat", 0x0107_0000),
        ("client_anim.dat", 0x0104_0000),
        ("client_highres.dat", 0x0108_0000),
    ] {
        write(dir, name, &base(dataset, 0, 1));
    }
    write(dir, "client_mesh.dat", &base(0x0102_0000, 0, 2));
    write(dir, "client_mesh_aux_1.datx", &aux(0x0102_0000, 0, 1, 2));
    write(dir, "client_cell_1.dat", &base(0x0203_0000, 1, 1));
    write(dir, "client_map_1.dat", &base(0x020A_0000, 1, 1));
    write(dir, language_file, &base(0x0303_0000, subset, 1));
}

fn open(dir: &Path) -> ArchiveSet {
    ArchiveSet::open(
        dir,
        SetOptions {
            language: Some("DE".to_owned()),
            ..SetOptions::default()
        },
    )
    .expect("the set opens")
}

fn inventory_of(dir: &Path) -> Result<usize, InstallUnsupported> {
    archive_inventory(&open(dir), "DE", &[1, 2]).map(|a| a.len())
}

fn fresh() -> tempfile::TempDir {
    let dir = tempfile::tempdir().expect("a directory");
    healthy(dir.path());
    dir
}

// ---------------------------------------------------------------------------
// The whole answer
// ---------------------------------------------------------------------------

/// A language whose key folds to something else: the full key reaches `0xEC`, the fold only the
/// archive check, and the identifier's halves come out in wire order.
#[test]
fn a_healthy_install_answers_with_the_full_key_and_its_real_entries() {
    let dir = fresh();
    let set = open(dir.path());
    assert_eq!(set.problems(), [], "a healthy set has no problem");
    let inputs = initial_inputs(&set);
    assert_eq!(inputs.language_key, Ok(DE_KEY), "not the folded key");
    assert_eq!(fold_language_code(DE_KEY), 0x1004);

    let rules = inputs.name_rule(7).expect("the rule");
    assert!(rules.contains(&DE_KEY));
    assert_eq!(check_language_allowed(&rules, DE_KEY), Ok(()));
    // A folded key does not pass a rule that lists the full one, and a rule that lists the fold
    // does not pass the full key.
    assert_eq!(
        check_language_allowed(&rules, 0x1004),
        Err(UnsupportedNegotiation::LanguageNotAllowed)
    );
    let folded_rule = inputs.name_rule(9).expect("the rule");
    assert_eq!(
        check_language_allowed(&folded_rule, DE_KEY),
        Err(UnsupportedNegotiation::LanguageNotAllowed)
    );
    assert_eq!(
        inputs.name_rule(99),
        Err(InstallUnsupported::NameRuleMissing)
    );
    assert_eq!(
        check_language_allowed(&inputs.name_rule(8).expect("empty"), DE_KEY),
        Err(UnsupportedNegotiation::NameRuleEmpty)
    );

    // Seven portal sets, region 1's two and the one language; the auxiliary part has no entry.
    let inventory = inputs.inventory.expect("the inventory");
    assert_eq!(inventory.len(), 10);
    let cell = inventory
        .iter()
        .find(|a| a.resource_id == [0x00, 0x00, 0x03, 0x02, 0x01, 0x00, 0x00, 0x00])
        .expect("region 1's cell: bytes 0x14C..0x153 as stored");
    assert_eq!(
        cell.wire_identifier(),
        [0x01, 0x00, 0x00, 0x00, 0x00, 0x00, 0x03, 0x02],
        "bytes 0x150..0x153, then 0x14C..0x14F"
    );
    assert_eq!(cell.iteration_count, 3);
    assert_eq!(cell.notes.len(), 1);

    let data = build_install_advertisement(DE_KEY, &inventory).expect("builds");
    assert_eq!(
        &data[1..5],
        &[0x04, 0x00, 0x00, 0x10],
        "the key on the wire"
    );
}

#[test]
fn another_language_stays_out_of_the_inventory() {
    let dir = fresh();
    write(
        dir.path(),
        "client_local_FR.dat",
        &base(0x0303_0000, 0x1003, 1),
    );
    // Its side files and its own stale parts are not this login's.
    std::fs::write(dir.path().join("client_local_FR.dat.ovl"), b"x").expect("written");
    std::fs::write(dir.path().join("client_local_FR_aux_01.datx"), b"x").expect("written");
    let set = open(dir.path());
    let inventory = archive_inventory(&set, "DE", &[1]).expect("the inventory");
    assert_eq!(inventory.len(), 10);
    assert!(
        inventory
            .iter()
            .all(|a| a.resource_id != [0x00, 0x00, 0x03, 0x03, 0x03, 0x10, 0x00, 0x00])
    );
}

#[test]
fn the_selected_archive_must_carry_the_fold_of_the_key() {
    let dir = fresh();
    write(
        dir.path(),
        "client_local_DE.dat",
        &base(0x0303_0000, 0x1003, 1),
    );
    let set = open(dir.path());
    assert_eq!(
        initial_inputs(&set).language_key,
        Err(InstallUnsupported::LanguageSubset)
    );
}

/// §9.3.7's resolver cases: the requested label is absent and the fallback archive's label is the
/// one actually mounted; it resolves to the full key and requires its fold. Neither the fold nor
/// the low half stands in for the key.
#[test]
fn the_lookup_uses_the_label_actually_mounted() {
    const FIXTURE_KEY: u32 = 0x1000_0005;
    let fixture_records = || {
        let mut m = records();
        m.insert(
            LANGUAGES_BASE,
            enum_mapper(LANGUAGES_BASE, 0, &[(FIXTURE_KEY, "fixturelang")]),
        );
        m.insert(NAME_TABLE, name_table(&[(7, &[FIXTURE_KEY])]));
        m
    };
    let open_as = |dir: &Path| {
        ArchiveSet::open(
            dir,
            SetOptions {
                language: Some("Requested".to_owned()),
                ..SetOptions::default()
            },
        )
        .expect("the set opens")
    };
    let dir = tempfile::tempdir().expect("a directory");
    install(
        dir.path(),
        fixture_records(),
        "client_local_FixtureLang.dat",
        0x1005,
    );
    let set = open_as(dir.path());
    assert_eq!(set.language(), Some("FixtureLang"), "the fallback");
    let inputs = initial_inputs(&set);
    assert_eq!(inputs.language_key, Ok(FIXTURE_KEY));
    assert_eq!(inputs.name_rule(7), Ok(vec![FIXTURE_KEY]));
    let inventory = inputs.inventory.expect("the inventory");
    let data = build_install_advertisement(FIXTURE_KEY, &inventory).expect("builds");
    assert_eq!(&data[1..5], &[0x05, 0x00, 0x00, 0x10], "the full key");

    // A low-half subset is not the fold.
    let dir = tempfile::tempdir().expect("a directory");
    install(
        dir.path(),
        fixture_records(),
        "client_local_FixtureLang.dat",
        0x0005,
    );
    assert_eq!(
        initial_inputs(&open_as(dir.path())).language_key,
        Err(InstallUnsupported::LanguageSubset)
    );
    // The requested label is never substituted for the mounted one: an enum without the mounted
    // label fails, even when it has the requested one.
    let mut m = fixture_records();
    m.insert(
        LANGUAGES_BASE,
        enum_mapper(LANGUAGES_BASE, 0, &[(FIXTURE_KEY, "Requested")]),
    );
    let dir = tempfile::tempdir().expect("a directory");
    install(dir.path(), m, "client_local_FixtureLang.dat", 0x1005);
    assert_eq!(
        initial_inputs(&open_as(dir.path())).language_key,
        Err(InstallUnsupported::LanguageAbsent)
    );
}

// ---------------------------------------------------------------------------
// Lookups
// ---------------------------------------------------------------------------

#[test]
fn the_language_lookup_walks_the_base_chain_and_takes_no_guess() {
    let root = Did(MASTER_MAP);
    let read = reader(records());
    assert_eq!(language_key_lookup(&read, root, "DE"), Ok(DE_KEY));
    assert_eq!(language_key_lookup(&read, root, "de"), Ok(DE_KEY));
    assert_eq!(language_key_lookup(&read, root, "English"), Ok(2));
    assert_eq!(
        language_key_lookup(&read, root, "D"),
        Err(InstallUnsupported::LanguageAbsent)
    );

    // The same label under a second key, in the chain.
    let mut m = records();
    m.insert(
        LANGUAGES,
        enum_mapper(LANGUAGES, LANGUAGES_BASE, &[(77, "DE")]),
    );
    assert_eq!(
        language_key_lookup(&reader(m), root, "DE"),
        Err(InstallUnsupported::LanguageAmbiguous)
    );

    // The nearest child's name for a key wins, and an overridden name matches nothing.
    let mut m = records();
    m.insert(
        LANGUAGES,
        enum_mapper(LANGUAGES, LANGUAGES_BASE, &[(DE_KEY, "Deutsch")]),
    );
    let read = reader(m);
    assert_eq!(language_key_lookup(&read, root, "Deutsch"), Ok(DE_KEY));
    assert_eq!(
        language_key_lookup(&read, root, "DE"),
        Err(InstallUnsupported::LanguageAbsent)
    );
    // A label the child takes from its base under another key is on two effective keys only if
    // the base's key keeps it; a child's override of that key clears the tie.
    let mut m = records();
    m.insert(
        LANGUAGES,
        enum_mapper(LANGUAGES, LANGUAGES_BASE, &[(77, "DE"), (DE_KEY, "Other")]),
    );
    assert_eq!(language_key_lookup(&reader(m), root, "DE"), Ok(77));

    // Key zero.
    let mut m = records();
    m.insert(LANGUAGES, enum_mapper(LANGUAGES, 0, &[(0, "DE")]));
    assert_eq!(
        language_key_lookup(&reader(m), root, "DE"),
        Err(InstallUnsupported::LanguageKeyZero)
    );

    // A chain that cannot rule out a second key: a cycle, or a mapper that does not read.
    let mut m = records();
    m.insert(
        LANGUAGES_BASE,
        enum_mapper(LANGUAGES_BASE, LANGUAGES, &[(DE_KEY, "DE")]),
    );
    assert_eq!(
        language_key_lookup(&reader(m), root, "DE"),
        Err(InstallUnsupported::LanguageMapper)
    );
    let mut m = records();
    m.remove(&LANGUAGES_BASE);
    assert_eq!(
        language_key_lookup(&reader(m), root, "DE"),
        Err(InstallUnsupported::LanguageMapper)
    );
}

/// §9.3.7: a chain of 64 records is followed, and one of 65 fails.
#[test]
fn the_language_chain_is_bounded() {
    assert_eq!(LANGUAGE_CHAIN_LIMIT, 64);
    let chain = |records_in_chain: u32| {
        let mut m = records();
        let did = |i: u32| {
            if i == 0 { LANGUAGES } else { 0x2300_1000 + i }
        };
        for i in 0..records_in_chain {
            let next = if i + 1 == records_in_chain {
                0
            } else {
                did(i + 1)
            };
            let names: &[(u32, &str)] = if i + 1 == records_in_chain {
                &[(DE_KEY, "DE")]
            } else {
                &[]
            };
            m.insert(did(i), enum_mapper(did(i), next, names));
        }
        language_key_lookup(&reader(m), Did(MASTER_MAP), "DE")
    };
    assert_eq!(chain(64), Ok(DE_KEY));
    assert_eq!(chain(65), Err(InstallUnsupported::LanguageMapper));
}

#[test]
fn the_name_rules_are_found_in_the_second_map_group() {
    let root = Did(MASTER_MAP);
    let rules = name_rules_lookup(&reader(records()), root).expect("the table");
    assert_eq!(rules.len(), 3);
    assert_eq!(rules[0], (7, vec![2, DE_KEY]));

    let mut m = records();
    m.remove(&NAME_TABLE);
    assert_eq!(
        name_rules_lookup(&reader(m), root),
        Err(InstallUnsupported::NameTable)
    );
    let mut m = records();
    m.insert(NAME_TABLE, vec![0xFF]);
    assert_eq!(
        name_rules_lookup(&reader(m), root),
        Err(InstallUnsupported::NameTable)
    );
}

#[test]
fn the_region_keys_leave_out_zero() {
    assert_eq!(
        region_keys_lookup(&reader(records()), Did(MASTER_MAP)),
        Ok(vec![1, 2])
    );
}

#[test]
fn the_iteration_stamp_decides_the_trailing_part() {
    let id = [0, 0, 3, 2, 1, 0, 0, 0];
    // Stamp 1 ends at the run list: `K = 0` in the advertisement.
    let mut short = Vec::new();
    for v in [3i32, -3, 7] {
        short.extend_from_slice(&v.to_le_bytes());
    }
    let archive = advertised_archive(id, &short, 1).expect("stamp 1");
    assert!(archive.notes.is_empty());
    let data = build_install_advertisement(DE_KEY, std::slice::from_ref(&archive)).expect("builds");
    assert_eq!(&data[33..37], &0u32.to_le_bytes(), "K");
    // Its bytes are not a stamp-2 record, stamp 2's bytes are not a stamp-1 record.
    assert_eq!(
        advertised_archive(id, &short, 2),
        Err(InstallUnsupported::ArchiveIteration)
    );
    assert_eq!(
        advertised_archive(id, &iteration_record(), 1),
        Err(InstallUnsupported::ArchiveIteration)
    );
    // §9.3.7: every stamp but 1 and 2 fails, stamp 3 and more as malformed and stamp 0 though the
    // general reader takes it as having no trailing part.
    assert_eq!(
        advertised_archive(id, &iteration_record(), 3),
        Err(InstallUnsupported::ArchiveIteration)
    );
    for (bytes, stamp) in [(&short, 0), (&[0u8; 4].to_vec(), 0), (&short, u16::MAX)] {
        assert_eq!(
            advertised_archive(id, bytes, stamp),
            Err(InstallUnsupported::ArchiveIteration),
            "stamp {stamp}"
        );
    }
    // An empty list passes with exact consumption and `K = 0`, in both stamps, and writes the
    // real empty list.
    let empty = advertised_archive(id, &[0u8; 4], 1).expect("stamp 1, no iterations");
    assert_eq!(empty.iteration_count, 0);
    let stored = advertised_archive(id, &[0u8; 8], 2).expect("stamp 2, no iterations");
    assert_eq!(stored, empty);
    let data = build_install_advertisement(DE_KEY, &[empty]).expect("builds");
    assert_eq!(&data[21..29], &[0u8; 8], "N and K");
    assert_eq!(
        advertised_archive(id, &[0u8; 5], 1),
        Err(InstallUnsupported::ArchiveIteration),
        "exact consumption"
    );
    // `K = 1` over an empty list has nothing to name.
    let mut one_note = [0u8; 8].to_vec();
    one_note[4] = 1;
    one_note.extend_from_slice(&[7, 0, 0, 0]);
    one_note.extend_from_slice(&[0xAB; 8]);
    assert_eq!(
        advertised_archive(id, &one_note, 2),
        Err(InstallUnsupported::ArchiveIteration)
    );
}

/// `install-layout.md` §9.6 invariant 7, with no repair: the trailing list is the `K` highest
/// listed iterations in increasing order. Trailing bytes are kept, runs are written again.
#[test]
fn the_iteration_record_must_keep_the_invariant() {
    let id = [0, 0, 3, 2, 1, 0, 0, 0];
    let record = |values: &[i32], opaque: &[u8]| {
        let mut out = Vec::new();
        for v in values {
            out.extend_from_slice(&v.to_le_bytes());
        }
        out.extend_from_slice(opaque);
        out
    };
    // The middle of the list is not the highest.
    assert_eq!(
        advertised_archive(id, &record(&[3, -3, 7, 1, 8], &[0xAB; 8]), 2),
        Err(InstallUnsupported::ArchiveIteration)
    );
    // Two values, ascending: the two highest.
    let mut two = record(&[3, -3, 7, 2, 8], &[1; 8]);
    two.extend_from_slice(&9i32.to_le_bytes());
    two.extend_from_slice(&[2; 8]);
    let archive = advertised_archive(id, &two, 2).expect("the two highest");
    let data = build_install_advertisement(DE_KEY, &[archive]).expect("builds");
    assert_eq!(&data[21..data.len() - 8], two.as_slice());
    // In decreasing order it fails.
    let mut swapped = record(&[3, -3, 7, 2, 9], &[1; 8]);
    swapped.extend_from_slice(&8i32.to_le_bytes());
    swapped.extend_from_slice(&[2; 8]);
    assert_eq!(
        advertised_archive(id, &swapped, 2),
        Err(InstallUnsupported::ArchiveIteration)
    );
    // A stored two-value run is written again as two singles, its values kept.
    let stored = record(&[2, -2, 13, 1, 14], &[3; 8]);
    let archive = advertised_archive(id, &stored, 2).expect("a stored pair");
    let data = build_install_advertisement(DE_KEY, &[archive]).expect("builds");
    assert_eq!(
        &data[21..data.len() - 8],
        record(&[2, 13, 14, 1, 14], &[3; 8]).as_slice()
    );
}

// ---------------------------------------------------------------------------
// The preflight's outcomes
// ---------------------------------------------------------------------------

/// Each change of a healthy install makes the inventory unsupported, with its kind.
#[track_caller]
fn refused(change: impl FnOnce(&Path), want: InstallUnsupported) {
    let dir = fresh();
    change(dir.path());
    assert_eq!(inventory_of(dir.path()), Err(want));
}

/// §9.3.7: `highres` and `anim` are optional: absent, they contribute no entry.
#[test]
fn the_optional_portal_sets_may_be_absent() {
    let dir = fresh();
    std::fs::remove_file(dir.path().join("client_anim.dat")).expect("removed");
    assert_eq!(inventory_of(dir.path()), Ok(9));
    std::fs::remove_file(dir.path().join("client_highres.dat")).expect("removed");
    assert_eq!(inventory_of(dir.path()), Ok(8));
    // The non-optional ones may not.
    for file in [
        "client_gamelogic.dat",
        "client_mesh.dat",
        "client_surface.dat",
        "client_sound.dat",
    ] {
        refused(
            |d| {
                std::fs::remove_file(d.join(file)).expect("removed");
                if file == "client_mesh.dat" {
                    std::fs::remove_file(d.join("client_mesh_aux_1.datx")).expect("removed");
                }
            },
            InstallUnsupported::ArchiveMissing,
        );
    }
}

/// §9.3.7, `install-layout.md` §3.4: a stale or stray auxiliary file is left out when its family's
/// own membership is verified, and is never touched.
#[test]
fn a_stale_or_stray_auxiliary_file_is_left_out() {
    let check = |change: &dyn Fn(&Path), file: &str| {
        let dir = fresh();
        change(dir.path());
        let before = std::fs::read(dir.path().join(file)).expect("there");
        assert_eq!(inventory_of(dir.path()), Ok(10), "{file}");
        assert_eq!(
            std::fs::read(dir.path().join(file)).expect("still there"),
            before,
            "{file}"
        );
    };
    // At or past the base's count, whatever its own record says.
    check(
        &|d| write(d, "client_cell_1_aux_1.datx", &aux(0x0203_0000, 1, 1, 1)),
        "client_cell_1_aux_1.datx",
    );
    check(
        &|d| write(d, "client_mesh_aux_2.datx", &aux(0x0102_0000, 0, 2, 3)),
        "client_mesh_aux_2.datx",
    );
    check(
        &|d| {
            let mut a = aux(0x0102_0000, 0, 2, 2);
            a.membership = Some(([0x77; 16], 2, 2));
            write(d, "client_mesh_aux_2.datx", &a);
        },
        "client_mesh_aux_2.datx",
    );
    // A name with no canonical part number.
    check(
        &|d| std::fs::write(d.join("client_mesh_aux_01.datx"), b"x").expect("written"),
        "client_mesh_aux_01.datx",
    );
    check(
        &|d| std::fs::write(d.join("client_mesh_aux_x.datx"), b"x").expect("written"),
        "client_mesh_aux_x.datx",
    );
    // A stray with no verified family to be left out of.
    refused(
        |d| std::fs::write(d.join("client_cell_2_aux_x.datx"), b"x").expect("written"),
        InstallUnsupported::ArchiveFailed,
    );
    refused(
        |d| std::fs::write(d.join("client_custom_aux_1x.datx"), b"x").expect("written"),
        InstallUnsupported::ArchiveFailed,
    );
}

/// §9.3.7's row is by file name: a part at or past its family's verified count is stale also when
/// it does not open, and it is left alone. A part below the count that does not open is a missing
/// active member.
#[test]
fn a_stale_auxiliary_file_that_does_not_open_is_left_out() {
    let garbage = [0u8; 4096];
    let dir = fresh();
    std::fs::write(dir.path().join("client_mesh_aux_2.datx"), garbage).expect("written");
    assert_eq!(inventory_of(dir.path()), Ok(10));
    assert_eq!(
        std::fs::read(dir.path().join("client_mesh_aux_2.datx")).expect("still there"),
        garbage
    );
    // Too short to be an archive, and an archive cut in the middle.
    let dir = fresh();
    std::fs::write(dir.path().join("client_mesh_aux_2.datx"), b"x").expect("written");
    let cut = aux(0x0102_0000, 0, 3, 2).build().bytes;
    std::fs::write(
        dir.path().join("client_mesh_aux_3.datx"),
        cut.get(..cut.len() / 2).expect("half"),
    )
    .expect("written");
    assert_eq!(inventory_of(dir.path()), Ok(10));
    // The same file as an active part is not stale.
    refused(
        |d| std::fs::write(d.join("client_mesh_aux_1.datx"), garbage).expect("written"),
        InstallUnsupported::ArchiveFailed,
    );
}

/// §9.3.7's first row: a requested language that is present but unreadable is unsupported, with no
/// fallback to another language's key. `install-layout.md` §3.5 falls back for a label with no
/// file. Beside it, the same corrupt file of a language nobody requested stays out, and a label
/// with no file still falls back.
#[test]
fn a_requested_language_that_is_present_but_unreadable_is_refused() {
    let open_for = |dir: &Path, label: &str| {
        ArchiveSet::open(
            dir,
            SetOptions {
                language: Some(label.to_owned()),
                ..SetOptions::default()
            },
        )
        .expect("the set opens")
    };
    let dir = fresh();
    std::fs::write(dir.path().join("client_local_FR.dat"), [0u8; 4096]).expect("written");
    // The set falls back to the language that opened, and the login does not.
    let set = open_for(dir.path(), "FR");
    assert_eq!(set.language(), Some("DE"), "the set's own fallback");
    assert_eq!(set.requested_language(), Some("FR"));
    assert_eq!(
        initial_inputs(&set).inventory.map(|a| a.len()),
        Err(InstallUnsupported::ArchiveFailed)
    );
    // ASCII case-insensitive, as the names of the set are.
    assert_eq!(
        initial_inputs(&open_for(dir.path(), "fr"))
            .inventory
            .map(|a| a.len()),
        Err(InstallUnsupported::ArchiveFailed)
    );
    // Requesting the language that opens: the corrupt one is another language's.
    assert_eq!(
        initial_inputs(&open_for(dir.path(), "DE"))
            .inventory
            .map(|a| a.len()),
        Ok(10)
    );
    // No file: the fallback of §3.5, on the label actually mounted.
    assert_eq!(
        initial_inputs(&open_for(dir.path(), "ES"))
            .inventory
            .map(|a| a.len()),
        Ok(10)
    );
}

/// §9.3.7's row leaves other languages out of the inventory, and a part another language is missing
/// is no problem of the listed sets.
#[test]
fn another_languages_missing_part_refuses_nothing() {
    let dir = fresh();
    write(
        dir.path(),
        "client_local_FR.dat",
        &base(0x0303_0000, 0x1003, 2),
    );
    let set = open(dir.path());
    assert!(
        set.problems()
            .iter()
            .any(|p| p.path.is_none() && p.severity == tessera_dat::Severity::Error),
        "the set reports the missing part"
    );
    assert_eq!(archive_inventory(&set, "DE", &[1]).map(|a| a.len()), Ok(10));
}

/// A local archive fault on the official path logs the file's name, never the directory it lies
/// in, which can name the user: not when it fails to open, not for a name that differs only in
/// case and not for a file that opens with something odd about it.
#[test]
fn a_local_archive_fault_logs_no_install_path() {
    use std::sync::{Arc, Mutex, PoisonError};

    #[derive(Clone, Default)]
    struct Buf(Arc<Mutex<Vec<u8>>>);
    impl std::io::Write for Buf {
        fn write(&mut self, b: &[u8]) -> std::io::Result<usize> {
            self.0
                .lock()
                .unwrap_or_else(PoisonError::into_inner)
                .extend_from_slice(b);
            Ok(b.len())
        }
        fn flush(&mut self) -> std::io::Result<()> {
            Ok(())
        }
    }

    let dir = fresh();
    std::fs::write(dir.path().join("client_sound_aux_7.datx"), [0u8; 4096]).expect("written");
    let cut = aux(0x0102_0000, 0, 3, 2).build().bytes;
    std::fs::write(
        dir.path().join("client_mesh_aux_3.datx"),
        cut.get(..cut.len() / 2).expect("half"),
    )
    .expect("written");
    write(
        dir.path(),
        "CLIENT_MESH_AUX_4.DATX",
        &aux(0x0102_0000, 0, 4, 2),
    );
    write(
        dir.path(),
        "client_mesh_aux_4.datx",
        &aux(0x0102_0000, 0, 4, 2),
    );
    let mut odd = aux(0x0102_0000, 0, 5, 2);
    odd.engine_version = 1;
    write(dir.path(), "client_mesh_aux_5.datx", &odd);

    let buf = Buf::default();
    let out = buf.clone();
    let sub = tracing_subscriber::fmt()
        .with_writer(move || out.clone())
        .with_max_level(tracing::Level::TRACE)
        .with_ansi(false)
        .finish();
    // With one registered subscriber `tracing` asks only the current thread's about a new callsite.
    let _second = tracing::Dispatch::new(tracing_subscriber::fmt().finish());
    tracing::subscriber::with_default(sub, || {
        tracing::callsite::rebuild_interest_cache();
        // The dictionary is what this fixture has not got: the opening is what is under test.
        let _ = tessera_client::official::install_data::install_data_at(dir.path(), "DE");
    });
    let logs =
        String::from_utf8_lossy(&buf.0.lock().unwrap_or_else(PoisonError::into_inner)).into_owned();
    assert!(logs.contains("client_sound_aux_7.datx"), "{logs}");
    assert!(logs.contains("skipping archive"), "{logs}");
    assert!(logs.contains("client_mesh_aux_5.datx"), "{logs}");
    let shown = dir.path().to_string_lossy();
    assert!(!logs.contains(shown.as_ref()), "the directory:\n{logs}");
}

/// §9.3.7, `dat-container.md` §3.4.1: neither the raw older encoding nor a conversion is sent.
#[test]
fn an_older_identifier_encoding_is_refused() {
    assert!(uses_older_identifier(0x0000_0001));
    assert!(!uses_older_identifier(0));
    assert!(!uses_older_identifier(0x0107_0000));
    refused(
        |d| write(d, "client_sound.dat", &base(0x0000_0001, 7, 1)),
        InstallUnsupported::ArchiveOlderIdentifier,
    );
}

/// §9.3.7: the requested label's absence alone refuses nothing (a warning naming no file), but a
/// problem of the listed sets does.
#[test]
fn a_missing_part_is_refused_and_a_language_fallback_is_not() {
    let dir = fresh();
    let set = ArchiveSet::open(
        dir.path(),
        SetOptions {
            language: Some("Requested".to_owned()),
            ..SetOptions::default()
        },
    )
    .expect("the set opens");
    assert_eq!(set.language(), Some("DE"));
    assert!(!set.problems().is_empty());
    assert_eq!(archive_inventory(&set, "DE", &[1]).map(|a| a.len()), Ok(10));
}

#[test]
fn an_auxiliary_part_that_is_not_associated_with_its_base_is_refused() {
    let failed = InstallUnsupported::ArchiveFailed;
    // Another set's identifier: a leftover of a patch.
    refused(
        |d| {
            let mut a = aux(0x0102_0000, 0, 1, 2);
            a.membership = Some(([0x77; 16], 1, 2));
            write(d, "client_mesh_aux_1.datx", &a);
        },
        failed,
    );
    // A wrong part index.
    refused(
        |d| write(d, "client_mesh_aux_1.datx", &aux(0x0102_0000, 0, 7, 2)),
        failed,
    );
    // An all-zero identifier on the part.
    refused(
        |d| {
            let mut a = aux(0x0102_0000, 0, 1, 2);
            a.membership = Some(([0; 16], 1, 2));
            write(d, "client_mesh_aux_1.datx", &a);
        },
        failed,
    );
    // No membership record.
    refused(
        |d| {
            let mut a = aux(0x0102_0000, 0, 1, 2);
            a.membership = None;
            write(d, "client_mesh_aux_1.datx", &a);
        },
        failed,
    );
    // A part with no base.
    refused(
        |d| write(d, "client_cell_2_aux_1.datx", &aux(0x0203_0000, 2, 1, 1)),
        failed,
    );
    // A part the base counts that is not there.
    refused(
        |d| std::fs::remove_file(d.join("client_mesh_aux_1.datx")).expect("removed"),
        failed,
    );
    // Another identifier than its base's.
    refused(
        |d| write(d, "client_mesh_aux_1.datx", &aux(0x0102_0000, 3, 1, 2)),
        InstallUnsupported::ArchiveIdentifier,
    );
}

#[test]
fn a_base_with_no_membership_record_or_an_ambiguous_name_is_refused() {
    refused(
        |d| {
            let mut b = base(0x0101_0000, 0, 1);
            b.membership = None;
            write(d, "client_gamelogic.dat", &b);
        },
        InstallUnsupported::ArchiveFailed,
    );
    // Two files naming one archive, on a file system that tells them apart.
    let dir = fresh();
    write(dir.path(), "CLIENT_GAMELOGIC.dat", &base(0x0101_0000, 0, 1));
    let names = std::fs::read_dir(dir.path()).expect("listed").count();
    if names == 12 {
        assert_eq!(
            inventory_of(dir.path()),
            Err(InstallUnsupported::ArchiveFailed)
        );
    }
    // A file named as an archive that does not open.
    refused(
        |d| std::fs::write(d.join("client_anim.dat"), b"short").expect("written"),
        InstallUnsupported::ArchiveFailed,
    );
}

#[test]
fn an_overlay_next_to_a_listed_base_is_refused() {
    for name in [
        "client_general.dat.ovl",
        "client_cell_1.dat.ovl",
        "client_local_DE.dat.ovl",
    ] {
        refused(
            |d| std::fs::write(d.join(name), b"x").expect("written"),
            InstallUnsupported::ArchiveOverlay,
        );
    }
    // Next to an archive that is not listed it means nothing: an overlay's name without a base.
    let dir = fresh();
    std::fs::write(dir.path().join("client_cell_9.dat.ovl"), b"x").expect("written");
    assert_eq!(inventory_of(dir.path()), Ok(10));
}

/// Region 1 is required, and half of another pair is unsupported (§9.3.8). A region archive outside
/// the map's keys is not a candidate.
#[test]
fn the_regions_are_complete_or_unsupported() {
    let missing = InstallUnsupported::ArchiveMissing;
    // Region 1 is required, each half of it.
    refused(
        |d| std::fs::remove_file(d.join("client_cell_1.dat")).expect("removed"),
        missing,
    );
    refused(
        |d| std::fs::remove_file(d.join("client_map_1.dat")).expect("removed"),
        missing,
    );
    // Half of another pair is a partial inventory.
    refused(
        |d| write(d, "client_cell_2.dat", &base(0x0203_0000, 2, 1)),
        missing,
    );
    refused(
        |d| write(d, "client_map_2.dat", &base(0x020A_0000, 2, 1)),
        missing,
    );
    // A key with neither file does not count as missing; a whole pair counts.
    let dir = fresh();
    assert_eq!(inventory_of(dir.path()), Ok(10));
    write(dir.path(), "client_cell_2.dat", &base(0x0203_0000, 2, 1));
    write(dir.path(), "client_map_2.dat", &base(0x020A_0000, 2, 1));
    assert_eq!(inventory_of(dir.path()), Ok(12));
    // A region archive outside the map's keys is not a candidate: ignored, not advertised.
    write(dir.path(), "client_cell_9.dat", &base(0x0203_0000, 9, 1));
    write(dir.path(), "client_map_9.dat", &base(0x020A_0000, 9, 1));
    assert_eq!(inventory_of(dir.path()), Ok(12));
}

#[test]
fn the_other_inventory_refusals_keep_their_kinds() {
    refused(
        |d| std::fs::remove_file(d.join("client_local_DE.dat")).expect("removed"),
        InstallUnsupported::ArchiveMissing,
    );
    refused(
        |d| write(d, "client_custom.dat", &base(0x0103_0000, 0, 1)),
        InstallUnsupported::ArchiveUnknown,
    );
    // The identifier disagrees with the file name.
    refused(
        |d| write(d, "client_sound.dat", &base(0x0104_0000, 0, 1)),
        InstallUnsupported::ArchiveIdentifier,
    );
    // No iteration record, or one of a stamp that is malformed.
    refused(
        |d| {
            let mut b = base(0x0104_0000, 0, 1);
            b.records.clear();
            write(d, "client_anim.dat", &b);
        },
        InstallUnsupported::ArchiveIteration,
    );
    refused(
        |d| {
            let mut b = base(0x0104_0000, 0, 1);
            b.records = vec![iteration(3, iteration_record())];
            write(d, "client_anim.dat", &b);
        },
        InstallUnsupported::ArchiveIteration,
    );
}
