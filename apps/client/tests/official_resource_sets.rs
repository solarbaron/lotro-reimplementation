// SPDX-License-Identifier: GPL-3.0-or-later
//! Decision 0046 item 13's reading of the ResourceSets enum (`official-session.md` §9.3.12 steps
//! 1–3 and 6) over synthetic DID maps and enum mappers: the route through root key 7 and sub-key
//! `0x8C` in either map group, the family checks before any read, the chain and names rules, and the
//! dictionary check. Every byte and key is generated here; no game data is involved.

use std::collections::BTreeMap;

use tessera_client::official::install_data::resource_set_lookup;
use tessera_compat::resource_report::{
    DictionaryEntry, PID_AVAILABLE_RESOURCE_SETS, PID_LOADED_RESOURCE_SETS, ReportOmission,
    ResourceSetNames, TYPE_BITFIELD,
};
use tessera_formats::Did;

/// A child's names, its base's names and the omission they give.
type NameCase<'a> = (&'a [(u32, &'a str)], &'a [(u32, &'a str)], ReportOmission);

const MASTER: u32 = 0x2800_0000;
const NAMED: u32 = 0x2800_0001;
const SETS: u32 = 0x2300_0300;
const SETS_BASE: u32 = 0x2300_0301;

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

/// A healthy install: the sub-key in the second group of the named map, a child over its base.
fn records() -> BTreeMap<u32, Vec<u8>> {
    BTreeMap::from([
        (MASTER, did_map(MASTER, &[&[(0x07, NAMED, "EnumMappers")]])),
        (
            NAMED,
            did_map(
                NAMED,
                &[&[(0x4A, 0x2300_0001, "Languages")], &[(0x8C, SETS, "Sets")]],
            ),
        ),
        (SETS, enum_mapper(SETS, SETS_BASE, &[(9, "SynthExtra")])),
        (SETS_BASE, enum_mapper(SETS_BASE, 0, &[(5, "SynthDefault")])),
    ])
}

fn dictionary_of(target: u32) -> impl Fn(u32) -> Option<DictionaryEntry> {
    move |pid| {
        (pid == PID_AVAILABLE_RESOURCE_SETS || pid == PID_LOADED_RESOURCE_SETS).then_some(
            DictionaryEntry {
                type_code: TYPE_BITFIELD,
                data: target,
            },
        )
    }
}

fn lookup(m: &BTreeMap<u32, Vec<u8>>) -> Result<ResourceSetNames, ReportOmission> {
    let read = |did: Did| m.get(&did.0).cloned();
    resource_set_lookup(&read, Did(MASTER), &dictionary_of(SETS))
}

#[test]
fn the_default_resolves_through_the_route_in_either_group() {
    let names = lookup(&records()).expect("resolves");
    assert_eq!(names.default_key(), 5);
    assert_eq!(names.name_count(), 2);
    assert_eq!(names.key_of("synthdefault"), Ok(5));
}

#[test]
fn a_missing_route_omits() {
    for missing in [MASTER, NAMED, SETS, SETS_BASE] {
        let mut m = records();
        m.remove(&missing);
        assert_eq!(
            lookup(&m),
            Err(ReportOmission::RouteMissing),
            "{missing:#x}"
        );
    }
    // No sub-key 0x8C.
    let mut m = records();
    m.insert(
        NAMED,
        did_map(NAMED, &[&[(0x4A, 0x2300_0001, "Languages")]]),
    );
    assert_eq!(lookup(&m), Err(ReportOmission::RouteMissing));
    // A master DID without the DID-map prefix.
    let m = records();
    let read = |did: Did| m.get(&did.0).cloned();
    assert_eq!(
        resource_set_lookup(&read, Did(0x2300_0000), &dictionary_of(SETS)),
        Err(ReportOmission::RouteMissing)
    );
}

#[test]
fn a_target_or_base_of_another_family_is_never_read_as_a_mapper() {
    // The sub-key names a DID map.
    let mut m = records();
    m.insert(NAMED, did_map(NAMED, &[&[(0x8C, 0x2800_0005, "Sets")]]));
    m.insert(0x2800_0005, did_map(0x2800_0005, &[&[(1, 2, "x")]]));
    let read = |did: Did| m.get(&did.0).cloned();
    assert_eq!(
        resource_set_lookup(&read, Did(MASTER), &dictionary_of(0x2800_0005)),
        Err(ReportOmission::WrongFamily)
    );
    // A base link to a DID map.
    let mut m = records();
    m.insert(SETS, enum_mapper(SETS, NAMED, &[(9, "SynthExtra")]));
    assert_eq!(lookup(&m), Err(ReportOmission::WrongFamily));
}

#[test]
fn a_malformed_record_omits() {
    let mut m = records();
    // One trailing byte: the record is not consumed exactly.
    let mut long = enum_mapper(SETS_BASE, 0, &[(5, "SynthDefault")]);
    long.push(0);
    m.insert(SETS_BASE, long);
    assert_eq!(lookup(&m), Err(ReportOmission::DecodeError));
    // A record whose own DID is another.
    let mut m = records();
    m.insert(
        SETS,
        enum_mapper(SETS_BASE, SETS_BASE, &[(9, "SynthExtra")]),
    );
    assert_eq!(lookup(&m), Err(ReportOmission::DecodeError));
    let mut m = records();
    m.insert(NAMED, vec![1, 2, 3]);
    assert_eq!(lookup(&m), Err(ReportOmission::DecodeError));
}

#[test]
fn the_chain_and_name_rules_hold_on_real_records() {
    let cases: [NameCase<'_>; 5] = [
        (
            &[(9, "A"), (9, "B")],
            &[(5, "SynthDefault")],
            ReportOmission::DuplicateKey,
        ),
        (
            &[(9, "A")],
            &[(5, "SynthDefault"), (6, "Two")],
            ReportOmission::BaseDefaultNotUnique,
        ),
        (&[(9, "A")], &[(0, "SynthDefault")], ReportOmission::ZeroKey),
        (
            &[(5, "Renamed")],
            &[(5, "SynthDefault")],
            ReportOmission::LabelChanged,
        ),
        (
            &[(9, "SYNTHDEFAULT")],
            &[(5, "SynthDefault")],
            ReportOmission::AmbiguousLabel,
        ),
    ];
    for (child, base, want) in cases {
        let mut m = records();
        m.insert(SETS, enum_mapper(SETS, SETS_BASE, child));
        m.insert(SETS_BASE, enum_mapper(SETS_BASE, 0, base));
        assert_eq!(lookup(&m), Err(want), "{want:?}");
    }
    // A cycle back to the target.
    let mut m = records();
    m.insert(
        SETS_BASE,
        enum_mapper(SETS_BASE, SETS, &[(5, "SynthDefault")]),
    );
    assert_eq!(lookup(&m), Err(ReportOmission::CycleOrDepth));
}

#[test]
fn the_dictionary_must_name_the_routed_mapper() {
    let m = records();
    let read = |did: Did| m.get(&did.0).cloned();
    // The printed DID of a world table is never a fallback: another mapper is a mismatch.
    assert_eq!(
        resource_set_lookup(&read, Did(MASTER), &dictionary_of(SETS_BASE)),
        Err(ReportOmission::DictionaryMismatch)
    );
    assert_eq!(
        resource_set_lookup(&read, Did(MASTER), &|_| None),
        Err(ReportOmission::DictionaryMissing)
    );
    let wrong_type =
        |pid: u32| dictionary_of(SETS)(pid).map(|e| DictionaryEntry { type_code: 18, ..e });
    assert_eq!(
        resource_set_lookup(&read, Did(MASTER), &wrong_type),
        Err(ReportOmission::DictionaryType)
    );
}
