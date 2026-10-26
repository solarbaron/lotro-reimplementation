// SPDX-License-Identifier: GPL-3.0-or-later
//! Fuzz target for the `.dat` records the official-session bootstrap reads from the user's
//! install (`docs/specs/official-session.md` §4.1.3, `official-login.md` §2.4.1, §2.4.3):
//! the superblock readers (`extract_didver_from_dat_header`,
//! `root_did_map_from_superblock`, the data-version descriptor), the `DataIdMap` record
//! parser and its key lookup, the 44-byte `BitTreeTable` record header, the redirect walk
//! of `resolve_bit_tree_table` and the full dictionary parser.
//!
//! Raw input rarely carries the superblock magic, a DID-map DID or the format UUID, so
//! three of the four modes place those first and fill the rest from the input. The dictionary
//! record DIDs are made up; the DID-map DID is the format value `official-session.md` §4.1.3
//! item 1 records (the code resolves the real one at run time from the install).

#![no_main]

use std::collections::BTreeMap;

use libfuzzer_sys::fuzz_target;
use tessera_compat::login::{
    BitTreeTableHeader, DAT_DIDVER_DESCRIPTOR_LEN, DAT_SUPERBLOCK_MAGIC, DAT_SUPERBLOCK_OFFSET,
    DICT_KEY_CLIENT_TO_SERVER, DICT_KEY_SERVER_TO_CLIENT, DICT_KEY_SERVER_TO_CLIENT_DEFERRED,
    DataIdMap, DataIdMapEntry, ROOT_KEY_COMPRESSION_DICTIONARY, compression_dictionary_map_did,
    extract_didver_from_dat_header, parse_dat_superblock_didver_descriptor, resolve_bit_tree_table,
    root_did_map_from_superblock,
};
use tessera_compat::session::{BIT_TREE_TABLE_FORMAT_UUID, BitTreeTable};

/// Made-up dictionary record DIDs (prefix `0x0E`, not the install's values).
const DIDS: [u32; 3] = [0x0E7F_0010, 0x0E7F_0020, 0x0E7F_0030];

/// Length of the superblock window the readers look at (`0x140 + 104`).
const SUPERBLOCK_END: usize = 0x1A8;

fn check_didver(result: Result<(String, u32), tessera_compat::login::LoginError>) {
    if let Ok((guid, _)) = result {
        assert_eq!(guid.len(), 36, "a didver GUID string has 36 characters");
        assert!(guid.bytes().all(|b| b == b'-' || b.is_ascii_hexdigit()));
    }
}

fn exercise_map(map: &DataIdMap) {
    assert_eq!(map.self_did >> 24, 0x28, "a parsed DID map has a 0x28 DID");
    for key in [
        ROOT_KEY_COMPRESSION_DICTIONARY,
        DICT_KEY_CLIENT_TO_SERVER,
        DICT_KEY_SERVER_TO_CLIENT,
        DICT_KEY_SERVER_TO_CLIENT_DEFERRED,
        0,
    ] {
        assert_ne!(
            map.resolve_enum_key(key),
            Some(0),
            "the undefined target is a miss"
        );
    }
    for e in &map.entries {
        let _ = map.resolve_enum_key(e.enum_key);
    }
}

/// Splits `data` into up to three records, each `u16 LE` length-prefixed.
fn records(mut data: &[u8]) -> Vec<&[u8]> {
    let mut out = Vec::with_capacity(3);
    while out.len() < 3 {
        let Some((len, rest)) = data.split_first_chunk::<2>() else {
            break;
        };
        let n = usize::from(u16::from_le_bytes(*len)).min(rest.len());
        let (rec, next) = rest.split_at(n);
        out.push(rec);
        data = next;
    }
    out
}

fuzz_target!(|data: &[u8]| {
    let Some((&mode, rest)) = data.split_first() else {
        return;
    };
    match mode & 3 {
        // Raw bytes through every reader.
        0 => {
            check_didver(extract_didver_from_dat_header(rest));
            check_didver(parse_dat_superblock_didver_descriptor(rest));
            if let Ok(did) = root_did_map_from_superblock(rest) {
                assert_eq!(did >> 24, 0x28);
            }
            if let Ok(map) = DataIdMap::parse(rest) {
                exercise_map(&map);
            }
            let _ = compression_dictionary_map_did(rest);
            let own = rest
                .first_chunk::<4>()
                .map_or(DIDS[0], |b| u32::from_le_bytes(*b));
            let _ = BitTreeTableHeader::parse(rest, own);
        }
        // A superblock window with the magic in place; the input fills the rest and may
        // cut it short.
        1 => {
            let mut header = vec![0u8; SUPERBLOCK_END];
            header[DAT_SUPERBLOCK_OFFSET..DAT_SUPERBLOCK_OFFSET + 4]
                .copy_from_slice(&DAT_SUPERBLOCK_MAGIC);
            let (cut, fill) = rest.split_first().map_or((0, rest), |(c, f)| (*c, f));
            let start = DAT_SUPERBLOCK_OFFSET + 4;
            for (dst, src) in header[start..].iter_mut().zip(fill) {
                *dst = *src;
            }
            header.truncate(SUPERBLOCK_END - usize::from(cut % 0x40));
            check_didver(extract_didver_from_dat_header(&header));
            if let Ok(did) = root_did_map_from_superblock(&header) {
                assert_eq!(did >> 24, 0x28);
            }
            if let Some(desc) = fill.get(..DAT_DIDVER_DESCRIPTOR_LEN) {
                check_didver(parse_dat_superblock_didver_descriptor(desc));
            }
        }
        // A DID map record: a `0x28` DID first, the input as its groups.
        2 => {
            let mut record = vec![0x10, 0x00, 0x00, 0x28];
            record.extend_from_slice(rest);
            if let Ok(map) = DataIdMap::parse(&record) {
                exercise_map(&map);
            }
            if let Ok(did) = compression_dictionary_map_did(&record) {
                assert_ne!(did, 0);
            }
        }
        // Up to three dictionary records with the format UUID in place and a redirect
        // chosen among the made-up DIDs, resolved through a synthetic map.
        _ => {
            let Some((&steer, body)) = rest.split_first() else {
                return;
            };
            let mut store: BTreeMap<u32, Vec<u8>> = BTreeMap::new();
            for (i, (rec, &did)) in records(body).iter().zip(DIDS.iter()).enumerate() {
                let mut bytes = BIT_TREE_TABLE_FORMAT_UUID.to_vec();
                bytes.extend_from_slice(rec);
                if bytes.len() >= BitTreeTableHeader::MIN_HEADER_LEN {
                    // Two bits per record pick the redirect: none, itself, or another.
                    let redirect = match (steer >> (2 * i)) & 3 {
                        0 => 0,
                        1 => did,
                        k => DIDS[(i + usize::from(k)) % DIDS.len()],
                    };
                    bytes[0x20..0x24].copy_from_slice(&redirect.to_le_bytes());
                    if steer & 0x40 != 0 {
                        // A consistent literal header: version 1, bodyLen = the rest.
                        bytes[0x24..0x28].copy_from_slice(&1u32.to_le_bytes());
                        let body_len = u32::try_from(bytes.len() - 0x2C).unwrap_or(u32::MAX);
                        bytes[0x28..0x2C].copy_from_slice(&body_len.to_le_bytes());
                    }
                }
                let _ = BitTreeTable::parse_redirect_did(&bytes, did);
                let _ = BitTreeTable::from_dat_bytes(&bytes);
                store.insert(did, bytes);
            }
            let map = DataIdMap {
                self_did: 0x2800_0010,
                entries: [
                    DICT_KEY_SERVER_TO_CLIENT_DEFERRED,
                    DICT_KEY_CLIENT_TO_SERVER,
                    DICT_KEY_SERVER_TO_CLIENT,
                ]
                .iter()
                .zip(DIDS)
                .map(|(&enum_key, target_did)| DataIdMapEntry {
                    enum_key,
                    target_did,
                })
                .collect(),
                names: BTreeMap::new(),
            };
            for e in &map.entries {
                if let Ok(resolved) = resolve_bit_tree_table(&map, e.enum_key, |did| {
                    store.get(&did).map(Vec::as_slice)
                }) {
                    assert!(
                        !resolved.header.redirects,
                        "the result is a literal dictionary"
                    );
                    assert_eq!(resolved.header.version, 1);
                    assert_eq!(resolved.header.format_uuid, BIT_TREE_TABLE_FORMAT_UUID);
                    assert!(DIDS.contains(&resolved.resolved_did));
                }
            }
        }
    }
});
