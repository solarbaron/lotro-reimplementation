// SPDX-License-Identifier: GPL-3.0-or-later
//! LOD-class tables (prefix 0x32, `docs/specs/environment.md` §9.1) on arbitrary bytes. The
//! first byte selects whether the rest is the payload as is, or follows a valid own DID so
//! that short inputs reach the entries.
//!
//! Nothing may panic. An accepted record re-encodes byte for byte and the look-up finds every
//! class.
//!
//! Run with a memory cap so that allocation blow-ups count as failures, e.g.
//! `cargo fuzz run formats_lod_classes -- -malloc_limit_mb=512 -rss_limit_mb=1024`.

#![no_main]

use libfuzzer_sys::fuzz_target;
use tessera_formats::Record;
use tessera_formats::environment::{LodClassTable, decode_lod_class_table};

fn payload(mode: u8, rest: &[u8]) -> Vec<u8> {
    if mode.is_multiple_of(2) {
        return rest.to_vec();
    }
    let mut out = Vec::with_capacity(4 + rest.len());
    out.extend_from_slice(&0x3200_0000u32.to_le_bytes());
    out.extend_from_slice(rest);
    out
}

fuzz_target!(|input: &[u8]| {
    let [mode, rest @ ..] = input else {
        return;
    };
    let data = &payload(*mode, rest)[..];
    let Ok(t) = decode_lod_class_table(data) else {
        return;
    };
    let bytes = t.to_bytes();
    assert_eq!(bytes, data, "exact re-encoding");
    let again = LodClassTable::parse(&bytes).expect("re-encoded record decodes");
    assert_eq!(again.len(), t.len());
    for e in &t.entries {
        assert_eq!(t.get(e.class).map(|f| f.class), Some(e.class));
        let _ = e.max_distance();
    }
});
