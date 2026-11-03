// SPDX-License-Identifier: GPL-3.0-or-later
//! The login's initial exchange on hostile input (`official-session.md` §9.3.2–§9.3.5,
//! decision 0046): the `0xDD` decoder, `0xEC` built from hostile iteration records, the
//! install's name-rule and language lookups over hostile DID maps, enum mappers and name
//! tables, and the top level of `0x6C`.
//!
//! The input is a chain of length-prefixed chunks: the first selects a wire `0xDD` and `0x6C`
//! body, the next ones are records served by an in-memory install. No panic and bounded work:
//! `0xEC` never passes its cap, and what it writes reads back through `IterationRecord::parse`.
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
    let again = tessera_install::IterationRecord::parse(body, 2);
    let first = tessera_install::IterationRecord::parse(record, stamp);
    if let (Ok(again), Ok(first)) = (again, first) {
        assert_eq!(again.count, first.count);
        assert_eq!(again.trailing, first.trailing);
    }
    let _ = advertisement_order(archive.wire_identifier());
}

fuzz_target!(|data: &[u8]| {
    let parts = chunks(data);

    // `0xDD`: atomic, exact, bounded.
    if let Ok(dd) = InstallRequest::from_data(data) {
        assert!(dd.trailing.len() <= MAX_DD_TRAILING);
        assert_eq!(data.len(), 13 + dd.trailing.len() * 4);
        assert_eq!(data.first(), Some(&0));
    }
    let _ = InstallRequest::from_envelope_body(data);

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
            assert!(stamp < 3, "stamp 3 or more is malformed");
            if stamp < 2 {
                assert!(archive.notes.is_empty(), "no trailing part below stamp 2");
            }
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
