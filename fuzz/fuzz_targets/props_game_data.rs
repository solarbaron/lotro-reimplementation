// SPDX-License-Identifier: GPL-3.0-or-later
//! [`GameData`] (`tessera-props`, `docs/specs/property-system.md` §6.2, §7–§9,
//! `records.md` §7.1, §8) over a **damaged synthetic install**: the registry loaded from
//! the dictionary record, the root DID map and label paths, named enum mappers and their
//! base chains (the base world holds a two-mapper cycle), DID maps, the DB-object-type
//! names, property sets by the +0x09000000 object rule, and whole-set resolution with
//! labels.
//!
//! The input starts with an edit plan (`common/mutate.rs`) applied to the records in the
//! order `GameData` reads them: damaged, truncated, removed, replaced or failing once.
//! The rest of the input chooses the enum keys, bit numbers, DB-object-type codes and one
//! free label path that are looked up.
//!
//! Nothing may panic. Lookups give the same answer twice (cached or not, apart from the
//! one injected read failure), a resolved set has one entry per property, and bit labels
//! stay within their caps.

#![no_main]

#[path = "common/mutate.rs"]
mod mutate;

use std::sync::OnceLock;

use libfuzzer_sys::fuzz_target;
use mutate::{InjectedFailure, Mutator, Read};
use tessera_formats::Did;
use tessera_formats::reader::{TSIZE_BUCKET_COUNTS, write_vle};
use tessera_props::game_data::{MAX_LABELLED_BITS, RESOLVE_MAX_BIT_LABELS};
use tessera_props::{GameData, Label, MemorySource, RecordSource, ResolvedProperty, SourceError};

// Synthetic PIDs and DIDs (no game data).
const P_ENUM: u32 = 0x1000_0001;
const P_ENUM_MAP: u32 = 0x1000_0002;
const P_BITS32: u32 = 0x1000_0003;
const P_BITS64: u32 = 0x1000_0004;
const P_BITS_ANY: u32 = 0x1000_0005;
const P_DID: u32 = 0x1000_0006;
const P_PID: u32 = 0x1000_0007;
const P_STRUCT: u32 = 0x1000_0008;
const P_ARRAY: u32 = 0x1000_0009;
const P_INT: u32 = 0x1000_000A;

const DICTIONARY: u32 = 0x3400_0000;
const ROOT_MAP: u32 = 0x2800_0000;
const EMAPPER_MAP: u32 = 0x2800_0001;
const VALUE_MAP: u32 = 0x2800_0020;
const MAPPER_A: u32 = 0x2300_0010;
const MAPPER_B: u32 = 0x2300_0011;
const DBO_MAPPER: u32 = 0x2300_0012;
const OBJECT: u32 = 0x7000_0001;
const OBJECT_SET: u32 = 0x7900_0001;
const LONE_SET: u32 = 0x7800_0002;

fn u32le(out: &mut Vec<u8>, v: u32) {
    out.extend_from_slice(&v.to_le_bytes());
}

/// Smallest TSize hint whose bucket count is at least half of `n`, and that count.
fn hint_for(n: usize) -> (u8, u32) {
    for (h, &b) in TSIZE_BUCKET_COUNTS.iter().enumerate() {
        if (b as usize) * 2 >= n {
            return (u8::try_from(h).unwrap_or(12), b);
        }
    }
    (12, TSIZE_BUCKET_COUNTS[12])
}

/// Sorts `items` into bucket order for their count; writes the TSize; returns nothing.
fn tsize_sorted<T>(out: &mut Vec<u8>, items: &mut [T], key: impl Fn(&T) -> u32) {
    let (hint, b) = hint_for(items.len());
    items.sort_by_key(|i| key(i) % b);
    out.push(hint);
    write_vle(out, u32::try_from(items.len()).unwrap_or(0));
}

fn pascal(out: &mut Vec<u8>, s: &str) {
    write_vle(out, u32::try_from(s.len()).unwrap_or(0));
    out.extend_from_slice(s.as_bytes());
}

/// One property definition: PID, name, type code, data word, child PIDs.
type Def = (u32, &'static str, u32, u32, &'static [u32]);

/// A property dictionary (`property-system.md` §2.1) without defaults or ranges.
fn dictionary(defs: &[Def]) -> Vec<u8> {
    let mut out = Vec::new();
    u32le(&mut out, DICTIONARY);
    out.extend_from_slice(&[0; 8]);
    let mut names: Vec<(u32, &str)> = defs.iter().map(|d| (d.0, d.1)).collect();
    tsize_sorted(&mut out, &mut names, |n| n.0);
    for (pid, name) in &names {
        u32le(&mut out, *pid);
        pascal(&mut out, name);
    }
    out.extend_from_slice(&[0, 0]);
    let mut sorted: Vec<&Def> = defs.iter().collect();
    tsize_sorted(&mut out, &mut sorted, |d| d.0);
    for &&(pid, _, ty, data, children) in &sorted {
        u32le(&mut out, pid);
        u32le(&mut out, pid);
        u32le(&mut out, ty);
        u32le(&mut out, 0);
        u32le(&mut out, 4);
        u32le(&mut out, data);
        u32le(&mut out, 0);
        u32le(&mut out, 0);
        u32le(&mut out, 0x3FC0_0000);
        out.extend_from_slice(&[2, 2, 4, 0, 1]);
        write_vle(&mut out, u32::try_from(children.len()).unwrap_or(0));
        for c in children {
            u32le(&mut out, *c);
            u32le(&mut out, *c);
        }
        u32le(&mut out, 0);
        u32le(&mut out, 0);
    }
    out
}

/// A StringInfo literal without arguments.
fn literal(out: &mut Vec<u8>, text: &str) {
    out.push(1);
    write_vle(out, u32::try_from(text.len()).unwrap_or(0));
    for u in text.encode_utf16() {
        out.extend_from_slice(&u.to_le_bytes());
    }
    out.extend_from_slice(&[0, 1, 0]);
}

/// An enum mapper (`records.md` §7.1) with literal display texts.
fn enum_mapper(did: u32, base: u32, names: &[(u32, &str)], displays: &[(u32, &str)]) -> Vec<u8> {
    let mut out = Vec::new();
    u32le(&mut out, did);
    u32le(&mut out, base);
    let mut names = names.to_vec();
    tsize_sorted(&mut out, &mut names, |n| n.0);
    for (k, n) in &names {
        u32le(&mut out, *k);
        pascal(&mut out, n);
    }
    let mut displays = displays.to_vec();
    tsize_sorted(&mut out, &mut displays, |n| n.0);
    for (k, d) in &displays {
        u32le(&mut out, *k);
        literal(&mut out, d);
    }
    out
}

/// A DID map (`records.md` §8.1): `main` in group 0, `second` in group 1.
fn did_map(did: u32, main: &[(u32, u32, &str)], second: &[(u32, u32, &str)]) -> Vec<u8> {
    let mut out = Vec::new();
    u32le(&mut out, did);
    for group in [main, second] {
        let mut g = group.to_vec();
        tsize_sorted(&mut out, &mut g, |e| e.0);
        for (k, v, _) in &g {
            u32le(&mut out, *k);
            u32le(&mut out, *v);
        }
        let mut g = group.to_vec();
        tsize_sorted(&mut out, &mut g, |e| e.0);
        for (k, _, l) in &g {
            u32le(&mut out, *k);
            out.push(u8::try_from(l.len()).unwrap_or(0));
            out.extend_from_slice(l.as_bytes());
        }
    }
    out
}

/// Property-set entries (`property-system.md` §5.1): TSize, then (PID, echo, value).
fn entries(out: &mut Vec<u8>, items: &[(u32, Vec<u8>)]) {
    let mut sorted: Vec<&(u32, Vec<u8>)> = items.iter().collect();
    tsize_sorted(out, &mut sorted, |e| e.0);
    for (pid, value) in sorted {
        u32le(out, *pid);
        u32le(out, *pid);
        out.extend_from_slice(value);
    }
}

fn set_record(did: u32, items: &[(u32, Vec<u8>)]) -> Vec<u8> {
    let mut out = Vec::new();
    u32le(&mut out, did);
    entries(&mut out, items);
    out
}

/// The valid base install, built once.
fn base() -> MemorySource {
    static BASE: OnceLock<MemorySource> = OnceLock::new();
    BASE.get_or_init(build_base).clone()
}

fn build_base() -> MemorySource {
    let defs: &[Def] = &[
        (P_ENUM, "Test_Enum", 8, MAPPER_A, &[]),
        (P_ENUM_MAP, "Test_EnumByMap", 8, VALUE_MAP, &[]),
        (P_BITS32, "Test_Bits32", 18, MAPPER_A, &[]),
        (P_BITS64, "Test_Bits64", 14, MAPPER_B, &[]),
        (P_BITS_ANY, "Test_BitsAny", 22, MAPPER_A, &[]),
        (P_DID, "Test_Did", 20, 3, &[]),
        (P_PID, "Test_Pid", 10, 0, &[]),
        (P_STRUCT, "Test_Struct", 11, 0, &[P_ENUM, P_INT]),
        (P_ARRAY, "Test_Array", 12, 0, &[P_BITS32]),
        (P_INT, "Test_Int", 15, 0, &[]),
    ];
    let le = |v: u32| v.to_le_bytes().to_vec();
    let mut strukt = Vec::new();
    entries(&mut strukt, &[(P_ENUM, le(2)), (P_INT, le(7))]);
    let mut array = le(2);
    for bits in [0b101u32, 0x8000_0001] {
        array.extend_from_slice(&P_BITS32.to_le_bytes());
        array.extend_from_slice(&bits.to_le_bytes());
    }
    let values = [
        (P_ENUM, le(1)),
        (P_ENUM_MAP, le(1)),
        (P_BITS32, le(0b1011)),
        (P_BITS64, u64::MAX.to_le_bytes().to_vec()),
        (P_DID, le(OBJECT)),
        (P_PID, le(P_ENUM)),
        (P_STRUCT, strukt),
        (P_ARRAY, array),
        (P_INT, le(42)),
    ];
    MemorySource::new()
        .with(DICTIONARY, dictionary(defs))
        .with(
            ROOT_MAP,
            did_map(
                ROOT_MAP,
                &[
                    (1, EMAPPER_MAP, "EMAPPER"),
                    (2, VALUE_MAP, "VALUES"),
                    (3, ROOT_MAP, "ROOT"),
                    (4, 0, "NULL"),
                ],
                &[(5, MAPPER_A, "MAPPER")],
            ),
        )
        .with(
            EMAPPER_MAP,
            did_map(
                EMAPPER_MAP,
                &[(1, DBO_MAPPER, "DBOType"), (2, MAPPER_A, "Test_Enum")],
                &[(3, VALUE_MAP, "Test_EnumByMap")],
            ),
        )
        .with(
            VALUE_MAP,
            did_map(
                VALUE_MAP,
                &[(1, OBJECT, "One"), (2, 0, "Null")],
                &[(3, ROOT_MAP, "Root")],
            ),
        )
        .with(
            MAPPER_A,
            enum_mapper(
                MAPPER_A,
                MAPPER_B,
                &[(1, "one"), (2, "two"), (4, "four")],
                &[(1, "One"), (3, "Three")],
            ),
        )
        // MAPPER_B's base is MAPPER_A: a cycle the chain walk must stop on.
        .with(
            MAPPER_B,
            enum_mapper(MAPPER_B, MAPPER_A, &[(3, "three"), (64, "top")], &[]),
        )
        .with(
            DBO_MAPPER,
            enum_mapper(DBO_MAPPER, 0, &[(3, "Object"), (0x34, "Dictionary")], &[]),
        )
        .with(OBJECT_SET, set_record(OBJECT_SET, &values))
        .with(LONE_SET, set_record(LONE_SET, &values[..3]))
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

fn check_labels(r: &ResolvedProperty<'_>, bits: &mut usize) {
    if let Label::Bits { bits: b, .. } = &r.label {
        assert!(b.len() <= MAX_LABELLED_BITS);
        *bits += b.len();
    }
    for c in &r.children {
        check_labels(c, bits);
    }
}

fuzz_target!(|data: &[u8]| {
    let (plan, rest) = mutate::plan(data);
    let gd = GameData::new(Damaged {
        base: base(),
        mutator: Mutator::new(plan),
    });
    let byte = |i: usize| rest.get(i).copied().unwrap_or(0);
    let word = |i: usize| u32::from_le_bytes([byte(i), byte(i + 1), byte(i + 2), byte(i + 3)]);

    // The registry: a failure other than an injected read error is cached and repeated.
    let first = gd.registry().map(|r| r.len());
    let second = gd.registry().map(|r| r.len());
    if let (Ok(a), Ok(b)) = (&first, &second) {
        assert_eq!(a, b);
    }

    let _ = gd.root_did_map();
    let free = String::from_utf8_lossy(rest.get(16..).unwrap_or_default()).into_owned();
    let paths: [&[&str]; 6] = [
        &["EMAPPER", "DBOType"],
        &["ROOT", "ROOT", "ROOT", "VALUES"],
        &["VALUES", "Root", "EMAPPER", "Test_Enum"],
        &["NULL", "x"],
        &["EMAPPER", free.as_str()],
        &[free.as_str()],
    ];
    for path in paths {
        let a = gd.lookup_path(path);
        let b = gd.lookup_path(path);
        if let (Ok(a), Ok(b)) = (a, b) {
            assert_eq!(a, b);
        }
    }
    let _ = gd.did_map_by_label("VALUES");
    let _ = gd.did_map_by_label(&free);
    let _ = gd.enum_mapper_by_name("Test_Enum");
    let _ = gd.enum_mapper_by_name(&free);
    for mapper in [MAPPER_A, MAPPER_B, DBO_MAPPER, word(0)] {
        let _ = gd.enum_mapper(Did(mapper));
        let _ = gd.enum_label(Did(mapper), word(4));
        let _ = gd.enum_label(Did(mapper), u32::from(byte(8)));
    }
    for map in [ROOT_MAP, EMAPPER_MAP, VALUE_MAP, word(0)] {
        let _ = gd.did_map(Did(map));
        let _ = gd.did_map_entry(Did(map), u32::from(byte(9)));
    }
    let _ = gd.dbo_type_name(byte(10));
    let _ = gd.dbo_type_name(3);

    for did in [OBJECT, OBJECT_SET, LONE_SET, word(12)] {
        let _ = gd.property_set_prefix(Did(did));
        let Ok(Some(set)) = gd.properties(Did(did)) else {
            continue;
        };
        for p in set.iter() {
            let _ = gd.label(p.pid, &p.value);
        }
        if let Ok(resolved) = gd.resolve(&set) {
            assert_eq!(resolved.len(), set.len());
            let mut bits = 0usize;
            for r in &resolved {
                check_labels(r, &mut bits);
            }
            assert!(bits <= RESOLVE_MAX_BIT_LABELS);
            let _: Vec<ResolvedProperty<'static>> = resolved
                .into_iter()
                .map(ResolvedProperty::into_owned)
                .collect();
        }
    }
    let _ = gd.cached_records();
});
