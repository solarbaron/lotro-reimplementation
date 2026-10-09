// SPDX-License-Identifier: GPL-3.0-or-later
//! Imposter tables (`docs/specs/misc-records.md` §5) on arbitrary bytes.
//!
//! The first input byte picks the mode: odd = the rest is a whole payload; even = the rest
//! follows a valid DID prologue (`0x27010000`), so runs reach the entry table and the tail.
//!
//! Nothing may panic. An accepted table re-encodes to bytes that decode to the same table
//! (when every float is finite and the count was a shortest VLE), and the cell helper
//! returns finite coordinates or nothing. The geometry and invariant helpers (atlas grid,
//! texel size, preview rectangle, square, MR5 check) run on every accepted table, and the
//! atlas-grid rule on a count taken from the input. This is the one fuzz target of the one
//! imposter-table decoder (`tessera_formats::imposter`).

#![no_main]

use libfuzzer_sys::fuzz_target;
use tessera_formats::imposter::{ImposterSquare, imposter_atlas_grid};
use tessera_formats::{ImposterTable, Record};

fn check(t: &ImposterTable) {
    for e in &t.entries {
        if let Some(c) = t.cell(e) {
            assert!([c.u0, c.v0, c.u1, c.v1].iter().all(|v| v.is_finite()));
            let (l, r) = c.u_pair(true);
            assert!(l.is_finite() && r.is_finite());
        }
        assert_eq!(t.get(e.name_key).map(|x| x.name_key), Some(e.name_key));
    }
    assert_eq!(t.keys_in_bucket_order(), t.is_bucket_ordered());
    let _ = t.first_unusual_field();
    let _ = (t.atlas_texels(), t.expected_cell_size());
    if let Some(sq) = t.square() {
        assert_eq!(sq.did(), t.did);
        assert_eq!(ImposterSquare::from_did(sq.did()), Some(sq));
    }
    for e in &t.entries {
        if let Some(uv) = t.cell_uv(e.column, e.row) {
            assert!(uv.iter().all(|v| v.is_finite()));
        }
    }
    assert!(t.dependencies().iter().all(|d| !d.is_null()));
    let bytes = t.to_bytes();
    let back = ImposterTable::parse(&bytes).expect("re-encoded table decodes");
    assert_eq!(back.entries.len(), t.entries.len());
    assert_eq!(back.to_bytes(), bytes, "re-encoding is not stable");
}

fuzz_target!(|data: &[u8]| {
    let Some((&mode, rest)) = data.split_first() else {
        return;
    };
    if let Some(n) = rest.get(..4).and_then(|b| <[u8; 4]>::try_from(b).ok())
        && let Some((c, r)) = imposter_atlas_grid(u32::from_le_bytes(n))
    {
        assert!(c.is_power_of_two() && c <= 16 && r.is_power_of_two());
    }
    if mode & 1 == 1 {
        if let Ok(t) = ImposterTable::parse(rest) {
            check(&t);
        }
    } else {
        let mut payload = 0x2701_0000u32.to_le_bytes().to_vec();
        payload.extend_from_slice(rest);
        if let Ok(t) = ImposterTable::parse(&payload) {
            check(&t);
        }
    }
});
