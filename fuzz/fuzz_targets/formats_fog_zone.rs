// SPDX-License-Identifier: GPL-3.0-or-later
//! Fog-zone tables (prefix 0x17, `docs/specs/water.md` §5) on arbitrary bytes. The first
//! byte selects whether the rest is the payload as is, or follows a valid header (own DID,
//! reserved, zone key 3) so that short inputs reach the entries.
//!
//! Nothing may panic. An accepted table re-encodes canonically (a fixpoint, never longer
//! than the input), all six floats of every entry are finite (`water.md` §11), `value_e`
//! follows only a non-zero `did_e`, and the key look-up returns the first entry of each
//! key, else the default.
//!
//! Run with a memory cap so that allocation blow-ups count as failures, e.g.
//! `cargo fuzz run formats_fog_zone -- -malloc_limit_mb=512 -rss_limit_mb=1024`.

#![no_main]

use libfuzzer_sys::fuzz_target;
use tessera_formats::Record;
use tessera_formats::water::{FogZoneTable, decode_fog_zone_table};

fn payload(mode: u8, rest: &[u8]) -> Vec<u8> {
    if mode.is_multiple_of(2) {
        return rest.to_vec();
    }
    let mut out = Vec::with_capacity(12 + rest.len());
    for v in [0x1700_0000u32, 0, 3] {
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
    let Ok(t) = decode_fog_zone_table(data) else {
        return;
    };
    let bytes = t.to_bytes();
    assert!(bytes.len() <= data.len());
    let again = FogZoneTable::parse(&bytes).expect("re-encoded table decodes");
    assert_eq!(again.to_bytes(), bytes);
    assert_eq!(again, t);
    let _ = t.any_active();
    for (i, z) in t.entries().iter().chain(t.default.as_ref()).enumerate() {
        // water.md §11: every fog-zone float of a decoded entry is finite.
        for v in [z.near, z.far, z.amount, z.value_60, z.light_a, z.light_b] {
            assert!(v.is_finite());
        }
        assert!(z.is_encodable());
        // §5.2 field 18: `value_e` is stored only after a non-zero `did_e`.
        assert!(z.e.is_none_or(|(d, _)| !d.is_null()));
        if i < t.entries().len() {
            let first = t
                .entries()
                .iter()
                .position(|x| x.key == z.key)
                .expect("present");
            assert_eq!(t.get(z.key), t.entries().get(first));
        }
    }
});
