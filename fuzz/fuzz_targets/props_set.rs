// SPDX-License-Identifier: GPL-3.0-or-later
//! `tessera-props` decoding (`docs/specs/property-system.md` §3-§6): the property
//! registry from a dictionary record, and property sets (records and embedded) decoded
//! with that registry, including the index built over each set. Nothing may panic.
//!
//! Input layout: a little-endian u16 length `n`, then `n` bytes of dictionary payload,
//! then the property-set bytes. When the dictionary does not decode, the set bytes are
//! still decoded with an empty registry (every PID unknown).

#![no_main]

use libfuzzer_sys::fuzz_target;
use tessera_formats::Did;
use tessera_props::{PropertyRegistry, PropertySet};

/// A dictionary record with no names and no definitions (`property-system.md` §2.1):
/// DID 0x34000000, eight reserved bytes, an empty TSize list, the two-byte separator and
/// another empty TSize list.
const EMPTY_DICTIONARY: [u8; 18] = [
    0x00, 0x00, 0x00, 0x34, 0, 0, 0, 0, 0, 0, 0, 0, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00,
];

fn exercise(set: &PropertySet) {
    let _ = format!("{set:?}");
}

fuzz_target!(|data: &[u8]| {
    if data.len() < 2 {
        return;
    }
    let n = usize::from(u16::from_le_bytes([data[0], data[1]])).min(data.len() - 2);
    let (dict, set_bytes) = data[2..].split_at(n);

    let registry = PropertyRegistry::parse(dict).unwrap_or_else(|_| {
        PropertyRegistry::parse(&EMPTY_DICTIONARY).expect("the empty dictionary decodes")
    });

    if let Some(d) = set_bytes.get(..4) {
        let did = Did(u32::from_le_bytes([d[0], d[1], d[2], d[3]]));
        if let Ok(set) = PropertySet::parse_record(set_bytes, did, &registry) {
            exercise(&set);
        }
        if let Ok((set, trailing)) = PropertySet::parse_record_prefix(set_bytes, did, &registry) {
            assert!(trailing <= set_bytes.len());
            exercise(&set);
        }
    }
    let did = Did(0x7900_0001);
    if let Ok(set) = PropertySet::parse_embedded(set_bytes, did, &registry) {
        exercise(&set);
    }
    if let Ok((set, used)) = PropertySet::parse_embedded_prefix(set_bytes, did, &registry) {
        assert!(used <= set_bytes.len());
        exercise(&set);
    }
});
