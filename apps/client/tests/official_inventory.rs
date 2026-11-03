// SPDX-License-Identifier: GPL-3.0-or-later
//! The install's answers to the login's initial exchange (`official-session.md` §9.3.2, §9.3.4)
//! over synthetic archives and records: the full language key through the Languages enum and its
//! base chain, the name rules in both map groups, the identifier's two halves and every refusal
//! of the archive inventory. Every byte is generated here; no game data is involved.

use std::path::Path;

use tessera_client::official::install_data::{
    advertised_archive, archive_inventory, initial_inputs, language_key_lookup, name_rules_lookup,
    region_keys_lookup,
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

/// `N = 3`, one run from 7, `K = 1` (`install-layout.md` §9.3).
fn iteration_record() -> Vec<u8> {
    let mut out = Vec::new();
    for v in [3i32, -3, 7, 1, 8] {
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
    let mut general = base(0x0103_0000, 0, 1);
    for (did, bytes) in records() {
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
    write(dir, "client_local_DE.dat", &base(0x0303_0000, 0x1004, 1));
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
    let inputs = initial_inputs(&set, "DE");
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
        initial_inputs(&set, "DE").language_key,
        Err(InstallUnsupported::LanguageSubset)
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
    // Stamp 3 and more are malformed; so is a list with no iterations.
    assert_eq!(
        advertised_archive(id, &iteration_record(), 3),
        Err(InstallUnsupported::ArchiveIteration)
    );
    assert_eq!(
        advertised_archive(id, &[0u8; 4], 1),
        Err(InstallUnsupported::ArchiveIteration)
    );
}

// ---------------------------------------------------------------------------
// Refusals of the inventory: nothing is silently omitted
// ---------------------------------------------------------------------------

/// Each change of a healthy install makes the inventory unsupported, with its kind.
#[track_caller]
fn refused(change: impl FnOnce(&Path), want: InstallUnsupported) {
    let dir = fresh();
    change(dir.path());
    assert_eq!(inventory_of(dir.path()), Err(want));
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
    // A part past the base's count.
    refused(
        |d| write(d, "client_cell_1_aux_1.datx", &aux(0x0203_0000, 1, 1, 1)),
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
    // A name with no canonical part number, which the original counts as a part.
    refused(
        |d| std::fs::write(d.join("client_mesh_aux_01.datx"), b"x").expect("written"),
        failed,
    );
    refused(
        |d| std::fs::write(d.join("client_mesh_aux_x.datx"), b"x").expect("written"),
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
    // A region archive outside the map's keys is not one the policy knows.
    refused(
        |d| {
            write(d, "client_cell_9.dat", &base(0x0203_0000, 9, 1));
            write(d, "client_map_9.dat", &base(0x020A_0000, 9, 1));
        },
        InstallUnsupported::ArchiveUnknown,
    );
}

#[test]
fn the_other_inventory_refusals_keep_their_kinds() {
    refused(
        |d| std::fs::remove_file(d.join("client_highres.dat")).expect("removed"),
        InstallUnsupported::ArchiveMissing,
    );
    refused(
        |d| std::fs::remove_file(d.join("client_local_DE.dat")).expect("removed"),
        InstallUnsupported::ArchiveFailed,
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
