// SPDX-License-Identifier: GPL-3.0-or-later
//! The login's initial exchange on hostile input (`official-session.md` §9.3.2–§9.3.7,
//! decision 0046): the `0xDD` decoder, `0xEC` built from hostile iteration records, the
//! install's name-rule and language lookups over hostile DID maps, enum mappers and name
//! tables, and the top level of `0x6C`.
//!
//! The whole input is tried as a wire `0xDD` and a `0x6C` body; split into a chain of
//! length-prefixed chunks it also gives iteration records and the records of an in-memory
//! install. No panic and bounded work: `0xEC` never passes its cap, and what it writes reads
//! back through `IterationRecord::parse` as the same iterations and trailing entries.
#![no_main]

use std::collections::BTreeMap;

use libfuzzer_sys::fuzz_target;
use tessera_client::official::install_data::{
    advertised_archive, language_key_lookup, name_rules_lookup, region_keys_lookup,
};
use tessera_compat::initial::{
    AdvertisedArchive, InstallRequest, MAX_ADVERTISEMENT_BYTES, MAX_DD_TRAILING,
    PreparedAdvertisement, advertisement_order, build_install_advertisement,
    check_language_allowed, fold_language_code,
};
use tessera_compat::world::{CharacterListResponse, PropertySchemaLookup, PropertyType};
use tessera_formats::Did;

/// A schema with a type for every property, so a table reads on.
struct Everything;

impl PropertySchemaLookup for Everything {
    fn property_type(&self, pid: u32) -> Option<PropertyType> {
        PropertyType::from_code(pid % 23).ok()
    }
}

/// The DIDs the lookups read: the root, its sub-maps and their targets.
const ROOT: u32 = 0x2800_0000;
const DIDS: [u32; 8] = [
    ROOT,
    0x2800_0001,
    0x2800_0002,
    0x2800_0008,
    0x2300_00B1,
    0x2300_00B2,
    0x0E20_0000,
    0x2800_0003,
];

fn chunks(data: &[u8]) -> Vec<&[u8]> {
    let mut out = Vec::new();
    let mut rest = data;
    while let Some((&len, tail)) = rest.split_first() {
        if out.len() >= 16 {
            break;
        }
        let n = usize::from(len).min(tail.len());
        // A length byte over 200 takes the whole tail: long records stay reachable.
        let n = if len > 200 { tail.len() } else { n };
        let (chunk, tail) = tail.split_at(n);
        out.push(chunk);
        rest = tail;
    }
    out
}

/// The iteration runs merged at their joins, so two forms of one list compare equal without
/// walking a count the input chose.
fn expanded(record: &tessera_install::IterationRecord) -> Vec<(i64, u64)> {
    let mut runs: Vec<(i64, u64)> = Vec::new();
    for range in &record.ranges {
        match runs.last_mut() {
            Some((start, len))
                if start.saturating_add(i64::try_from(*len).unwrap_or(i64::MAX)) == range.start =>
            {
                *len += u64::from(range.len);
            }
            _ => runs.push((range.start, u64::from(range.len))),
        }
    }
    runs
}

fn iteration_entries(archive: &AdvertisedArchive, record: &[u8], stamp: u16) {
    let Ok(data) = build_install_advertisement(5, std::slice::from_ref(archive)) else {
        return;
    };
    assert!(data.len() <= MAX_ADVERTISEMENT_BYTES);
    assert_eq!(data.first(), Some(&0), "one flags byte");
    let Some(body) = data.get(21..data.len().saturating_sub(8)) else {
        return;
    };
    // The serializer's output is a record of the same values, whatever form the input had.
    // `advertised_archive` took this record, so both parse.
    let again = tessera_install::IterationRecord::parse(body, 2).expect("what it wrote parses");
    let first = tessera_install::IterationRecord::parse(record, stamp).expect("it parsed before");
    assert_eq!(again.count, first.count);
    assert_eq!(again.trailing, first.trailing);
    assert_eq!(expanded(&again), expanded(&first));
    let _ = advertisement_order(archive.wire_identifier());
}

/// A zlib stream of one stored deflate block: `data` as it is, with a valid Adler-32, so the input
/// reaches what follows the inflation instead of stopping at the stream's check.
fn stored_zlib(data: &[u8], len: u16) -> Vec<u8> {
    let mut out = vec![0x78, 0x01, 0x01];
    out.extend_from_slice(&len.to_le_bytes());
    out.extend_from_slice(&(!len).to_le_bytes());
    out.extend_from_slice(data);
    let (mut a, mut b) = (1u32, 0u32);
    for &byte in data {
        a = (a + u32::from(byte)) % 65_521;
        b = (b + a) % 65_521;
    }
    out.extend_from_slice(&((b << 16) | a).to_be_bytes());
    out
}

/// Flags `0x01` (§9.3.7): the layout is measured on the inflated body, so a compressed `data`
/// reads exactly as the plain one does, and a wrong stated length fails whatever `data` holds.
fn compressed_request(data: &[u8]) {
    let plain = InstallRequest::from_envelope_body(data);
    // One stored block holds this much, below every decompression bound.
    let Ok(len) = u16::try_from(data.len()) else {
        return;
    };
    let compressed = |stated: u32| {
        let mut out = vec![0x01];
        out.extend_from_slice(&stated.to_le_bytes());
        out.extend_from_slice(&stored_zlib(data, len));
        out
    };
    let stated = u32::from(len);
    assert_eq!(InstallRequest::from_data(&compressed(stated)), plain);
    assert!(InstallRequest::from_data(&compressed(stated + 1)).is_err());
}

fuzz_target!(|data: &[u8]| {
    let parts = chunks(data);

    // `0xDD`: atomic, exact, bounded.
    if let Ok(dd) = InstallRequest::from_data(data) {
        assert!(dd.trailing.len() <= MAX_DD_TRAILING);
        match data.first() {
            // Measured on the data as it is.
            Some(0) => assert_eq!(data.len(), 13 + dd.trailing.len() * 4),
            // Measured after the bounded inflation.
            Some(1) => {}
            other => panic!("flags {other:?} decoded"),
        }
    }
    compressed_request(data);

    // The top level of `0x6C`: one flags byte, then the count, and never a false list.
    let _ = CharacterListResponse::from_envelope_body(data, &Everything);
    let _ = CharacterListResponse::from_bytes(data, &Everything);

    // `0xEC` from hostile iteration bytes, with every stamp case.
    let mut archives: Vec<AdvertisedArchive> = Vec::new();
    for (i, part) in parts.iter().enumerate() {
        for stamp in [0u16, 1, 2, 3] {
            let id = [0, 0, u8::try_from(i).unwrap_or(0), 1, 0, 0, 0, 0];
            let Ok(archive) = advertised_archive(id, part, stamp) else {
                continue;
            };
            assert!(stamp == 1 || stamp == 2, "every other stamp fails");
            if stamp == 1 {
                assert!(archive.notes.is_empty(), "no trailing part below stamp 2");
            }
            assert!(archive.check_invariant().is_ok(), "invariant 7 held");
            iteration_entries(&archive, part, stamp);
            if stamp == 2 {
                archives.push(archive);
            }
        }
    }
    if let Ok(prepared) = PreparedAdvertisement::new(5, &archives) {
        assert!(prepared.data.len() <= MAX_ADVERTISEMENT_BYTES);
        assert_eq!(prepared.entries, archives.len());
        // Built twice, the same bytes: the order is a function of the identifiers.
        assert_eq!(
            build_install_advertisement(5, &archives).as_ref().ok(),
            Some(&prepared.data)
        );
    }

    // The language and rule lookups over an install made of the input's chunks.
    let records: BTreeMap<u32, &[u8]> = DIDS.iter().copied().zip(parts.iter().copied()).collect();
    let read = |did: Did| records.get(&did.0).map(|b| b.to_vec());
    if let Ok(key) = language_key_lookup(&read, Did(ROOT), "English") {
        assert_ne!(key, 0);
        assert!(fold_language_code(key) <= 0xFFFF);
    }
    if let Ok(rules) = name_rules_lookup(&read, Did(ROOT)) {
        for (_, languages) in &rules {
            let _ = check_language_allowed(languages, 5);
        }
    }
    let _ = region_keys_lookup(&read, Did(ROOT));
});
