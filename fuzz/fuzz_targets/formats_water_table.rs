// SPDX-License-Identifier: GPL-3.0-or-later
//! Water tables (prefix 0x16, `docs/specs/water.md` §4) on arbitrary bytes. The first byte
//! selects whether the rest is the payload as is, or follows a valid header (own DID,
//! reserved, level key 4, type key 7) so that short inputs reach the entries.
//!
//! Nothing may panic. An accepted table re-encodes canonically (a fixpoint, never longer
//! than the input), and the key look-up returns the first entry of each key, else the
//! default.
//!
//! Run with a memory cap so that allocation blow-ups count as failures, e.g.
//! `cargo fuzz run formats_water_table -- -malloc_limit_mb=512 -rss_limit_mb=1024`.

#![no_main]

use libfuzzer_sys::fuzz_target;
use tessera_formats::Record;
use tessera_formats::water::{WaterTable, decode_water_table};

fn payload(mode: u8, rest: &[u8]) -> Vec<u8> {
    if mode.is_multiple_of(2) {
        return rest.to_vec();
    }
    let mut out = Vec::with_capacity(16 + rest.len());
    for v in [0x1600_0000u32, 0, 4, 7] {
        out.extend_from_slice(&v.to_le_bytes());
    }
    out.extend_from_slice(rest);
    out
}

fuzz_target!(|input: &[u8]| {
    let [mode, rest @ ..] = input else {
        return;
    };
    let data = &payload(*mode, rest)[..];
    let Ok(t) = decode_water_table(data) else {
        return;
    };
    let bytes = t.to_bytes();
    assert!(bytes.len() <= data.len());
    let again = WaterTable::parse(&bytes).expect("re-encoded table decodes");
    assert_eq!(again.to_bytes(), bytes);
    assert_eq!(again, t);
    for (i, e) in t.entries().iter().enumerate() {
        let first = t
            .entries()
            .iter()
            .position(|x| x.key == e.key)
            .expect("present");
        assert!(first <= i);
        assert_eq!(t.get(e.key), t.entries().get(first));
        if let Some(w) = &e.wet {
            assert!(w.value.is_finite());
        }
    }
    if t.get(u32::MAX).is_none() {
        assert_eq!(t.select(u32::MAX), t.default.as_ref());
    }
});
