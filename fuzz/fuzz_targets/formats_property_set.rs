// SPDX-License-Identifier: GPL-3.0-or-later
//! Property-set decoding (`docs/specs/property-system.md` §4-§5) on arbitrary bytes, with
//! two synthetic schemas: a total one (every PID has a type, chosen from the PID) so that
//! random PIDs reach every value decoder and the struct/array recursion, and a small fixed
//! one so that unknown-PID paths are hit too.
//!
//! Nothing may panic, and an accepted set must agree with its own header: stored entries
//! plus null entries equal the count, the walk visits `value_count` values, and no value
//! sits deeper than the depth limit.
//!
//! Run with a memory cap so that allocation blow-ups count as failures, e.g.
//! `cargo fuzz run formats_property_set -- -malloc_limit_mb=512 -rss_limit_mb=1024`.

#![no_main]

use std::collections::HashMap;

use libfuzzer_sys::fuzz_target;
use tessera_formats::property::PROPERTY_MAX_DEPTH;
use tessera_formats::{
    PropertySet, PropertySetRecord, PropertyType, PropertyValue, decode_embedded_property_set,
    decode_property_set, decode_property_value, object_property_set_did,
};

/// Every PID has a type: codes 0 and 23 map to `Unknown`, the rest to types 1-22.
fn total(pid: u32) -> Option<PropertyType> {
    Some(PropertyType::from_code(pid % 24))
}

fn check_set(set: &PropertySet) {
    assert_eq!(
        set.entries.len() + set.null_entries(),
        set.size.count as usize
    );
    let mut n = 0usize;
    set.walk(|v| {
        n += 1;
        assert!(v.depth < PROPERTY_MAX_DEPTH);
        assert_ne!(v.entry.pid, 0);
        match &v.entry.value {
            PropertyValue::Struct(s) => {
                assert_eq!(s.fields.len() + s.null_entries(), s.size.count as usize);
                let _ = s.is_bucket_ordered();
            }
            PropertyValue::Array(a) => {
                assert_eq!(a.elements.len() + a.null_entries(), a.count as usize);
            }
            PropertyValue::BitfieldAny(b) => {
                assert_eq!(b.bytes.len() as u64, u64::from(b.bits.div_ceil(8)));
                let _ = b.set_bits().count();
            }
            _ => {}
        }
    });
    assert_eq!(n, set.value_count());
    let _ = set.is_bucket_ordered();
    let _ = set.structs_bucket_ordered();
}

fuzz_target!(|data: &[u8]| {
    let mut small: HashMap<u32, PropertyType> = HashMap::new();
    for code in 1..=22u32 {
        small.insert(code, PropertyType::from_code(code));
        small.insert(0x1000_0000 | code, PropertyType::from_code(code));
    }

    if let Ok(rec) = decode_property_set(data, &total) {
        check_set(&rec.set);
        let _ = object_property_set_did(rec.did);
    }
    if let Ok(rec) = PropertySetRecord::parse(data, &small) {
        check_set(&rec.set);
    }
    if let Ok((rec, left)) = PropertySetRecord::parse_prefix(data, &total) {
        assert!(left <= data.len());
        check_set(&rec.set);
    }
    if let Ok(set) = decode_embedded_property_set(data, &total) {
        check_set(&set);
    }
    // A single value whose type comes from the first byte.
    if let Some((&code, rest)) = data.split_first() {
        let ty = PropertyType::from_code(u32::from(code % 24));
        if let Ok(v) = decode_property_value(rest, ty, &total) {
            assert_eq!(v.property_type(), ty);
            let _ = v.children().len();
        }
    }
});
