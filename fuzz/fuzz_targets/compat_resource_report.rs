// SPDX-License-Identifier: GPL-3.0-or-later
//! Decision 0046 item 13's resource-set report on hostile input (`official-session.md` §9.3.12):
//! the strict `0x13D` decoder, the pure resolver (chain, names, dictionary, configuration, mount
//! check) over records the input describes, and the client's route through hostile DID maps and enum
//! mappers.
//!
//! The whole input is tried as a `0x13D`; split into length-prefixed chunks it also gives mapper
//! records, a configuration string, archive variants and the records of an in-memory install. No
//! panic and bounded work: a decoded report re-encodes to the same bytes, and every candidate the
//! resolver builds is a strict singleton, the same in both fields.
#![no_main]

use std::collections::BTreeMap;

use libfuzzer_sys::fuzz_target;
use tessera_client::official::install_data::resource_set_lookup;
use tessera_compat::initial::AdvertisedArchive;
use tessera_compat::resource_report::{
    DictionaryEntry, MapperNames, PID_AVAILABLE_RESOURCE_SETS, PID_LOADED_RESOURCE_SETS,
    RESOURCE_SET_CHAIN_LIMIT, ReportOmission, TYPE_BITFIELD, candidate_data, check_mounted,
    resolve_resource_sets, resource_set_config, selected_key,
};
use tessera_compat::world::{RESOURCE_SET_MAX_BITS, ResourceSetMask, ResourceSetReport};
use tessera_formats::Did;

/// The DIDs the mapper chain uses: index `i` of the chunks.
const MAPPER_BASE: u32 = 0x2300_0400;
/// The DIDs the client route reads.
const ROOT: u32 = 0x2800_0000;
const ROUTE_DIDS: [u32; 6] = [
    ROOT,
    0x2800_0001,
    0x2300_0400,
    0x2300_0401,
    0x2300_0402,
    0x2800_0002,
];

fn chunks(data: &[u8]) -> Vec<&[u8]> {
    let mut out = Vec::new();
    let mut rest = data;
    while let Some((&len, tail)) = rest.split_first() {
        if out.len() >= 24 {
            break;
        }
        let n = usize::from(len).min(tail.len());
        let n = if len > 200 { tail.len() } else { n };
        let (chunk, tail) = tail.split_at(n);
        out.push(chunk);
        rest = tail;
    }
    out
}

/// A mapper record described by a chunk: a base index byte (`0xFF`: none, else a chunk index),
/// then entries of a `u16` key, a name length and the name bytes.
fn mapper_of(chunk: &[u8], count: usize) -> MapperNames {
    let Some((&base, mut rest)) = chunk.split_first() else {
        return MapperNames::default();
    };
    let base = if base == 0xFF || usize::from(base) >= count {
        0
    } else {
        MAPPER_BASE + u32::from(base)
    };
    let mut names = Vec::new();
    while let [a, b, len, tail @ ..] = rest {
        let n = usize::from(*len % 16).min(tail.len());
        let (name, tail) = tail.split_at(n);
        names.push((
            u32::from(u16::from_le_bytes([*a, *b])),
            String::from_utf8_lossy(name).into_owned(),
        ));
        rest = tail;
    }
    MapperNames { base, names }
}

fn archive(variant: u16) -> AdvertisedArchive {
    let [a, b] = variant.to_le_bytes();
    AdvertisedArchive {
        resource_id: [a, b, 3, 1, 0, 0, 0, 0],
        iteration_count: 0,
        spans: Vec::new(),
        notes: Vec::new(),
    }
}

fn check_candidate(data: &[u8], key: u32) {
    let report = ResourceSetReport::from_bytes(data).expect("the candidate is strict");
    assert_eq!(report.available, report.loaded);
    assert_eq!(report.available.keys(), [key]);
    assert_eq!(report.available.bit_count(), key);
}

fuzz_target!(|data: &[u8]| {
    // The strict codec: what decodes re-encodes to the same bytes and is minimal.
    if let Ok(report) = ResourceSetReport::from_bytes(data) {
        assert_eq!(report.to_bytes().ok().as_deref(), Some(data));
        for field in [&report.available, &report.loaded] {
            assert!(field.bit_count() <= RESOURCE_SET_MAX_BITS);
            assert_eq!(field.keys().last().copied().unwrap_or(0), field.bit_count());
        }
    }
    // Keys from the input: in range they round-trip, out of range nothing is built.
    let keys: Vec<u32> = data
        .as_chunks::<2>()
        .0
        .iter()
        .take(64)
        .map(|p| u32::from(u16::from_le_bytes(*p)))
        .collect();
    match ResourceSetMask::from_keys(&keys) {
        Ok(mask) => {
            let mut want = keys.clone();
            want.sort_unstable();
            want.dedup();
            assert_eq!(mask.keys(), want);
        }
        Err(_) => assert!(keys.iter().any(|k| *k == 0 || *k > RESOURCE_SET_MAX_BITS)),
    }

    // The pure resolver over a chain the chunks describe.
    let parts = chunks(data);
    let records: BTreeMap<u32, MapperNames> = parts
        .iter()
        .enumerate()
        .take(RESOURCE_SET_CHAIN_LIMIT + 2)
        .map(|(i, c)| (MAPPER_BASE + i as u32, mapper_of(c, parts.len())))
        .collect();
    let target = MAPPER_BASE;
    let flavour = data.first().copied().unwrap_or(0);
    let dictionary = move |pid: u32| match flavour % 4 {
        0 => None,
        1 => Some(DictionaryEntry {
            type_code: 18,
            data: target,
        }),
        2 if pid == PID_LOADED_RESOURCE_SETS => Some(DictionaryEntry {
            type_code: TYPE_BITFIELD,
            data: target + 1,
        }),
        _ => (pid == PID_AVAILABLE_RESOURCE_SETS || pid == PID_LOADED_RESOURCE_SETS).then_some(
            DictionaryEntry {
                type_code: TYPE_BITFIELD,
                data: target,
            },
        ),
    };
    let names = resolve_resource_sets(
        target,
        &mut |did| {
            records
                .get(&did)
                .cloned()
                .ok_or(ReportOmission::RouteMissing)
        },
        &dictionary,
    );
    let option = parts
        .last()
        .map(|c| String::from_utf8_lossy(c).into_owned());
    let option = (flavour & 0x10 != 0).then_some(option).flatten();
    let variants: Vec<AdvertisedArchive> = parts
        .get(1)
        .map(|c| {
            c.as_chunks::<2>()
                .0
                .iter()
                .take(32)
                .map(|p| archive(u16::from_le_bytes(*p)))
                .collect()
        })
        .unwrap_or_default();
    if let Ok(names) = &names {
        let key = names.default_key();
        assert_ne!(key, 0);
        if let Ok(config) = resource_set_config(option.as_deref())
            && let Ok(picked) = selected_key(names, &config)
        {
            assert_eq!(picked, key, "only the default is ever selected");
        }
        if let Ok(bytes) = candidate_data(Ok(names.clone()), option.as_deref(), &variants) {
            assert!(check_mounted(key, &variants).is_ok());
            check_candidate(&bytes, key);
        }
    }

    // The client's route over an install made of the chunks.
    let install: BTreeMap<u32, &[u8]> = ROUTE_DIDS
        .iter()
        .copied()
        .zip(parts.iter().copied())
        .collect();
    let read = |did: Did| install.get(&did.0).map(|b| b.to_vec());
    if let Ok(names) = resource_set_lookup(&read, Did(ROOT), &dictionary) {
        assert_ne!(names.default_key(), 0);
        if let Ok(bytes) = candidate_data(Ok(names.clone()), None, &variants) {
            check_candidate(&bytes, names.default_key());
        }
    }
});
