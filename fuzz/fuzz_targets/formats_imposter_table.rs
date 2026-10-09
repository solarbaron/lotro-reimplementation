// SPDX-License-Identifier: GPL-3.0-or-later
//! Imposter tables (`docs/specs/misc-records.md` §5) on arbitrary bytes.
//!
//! The first input byte picks the mode: odd = the rest is a whole payload; even = the rest
//! follows a valid DID prologue (`0x27010000`), so runs reach the entry table and the tail.
//!
//! Nothing may panic. An accepted table re-encodes to bytes that decode to the same table
//! (when every float is finite and the count was a shortest VLE), and the cell helper
//! returns finite coordinates or nothing.

#![no_main]

use libfuzzer_sys::fuzz_target;
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
    let _ = t.keys_in_bucket_order();
    let bytes = t.to_bytes();
    let back = ImposterTable::parse(&bytes).expect("re-encoded table decodes");
    assert_eq!(back.entries.len(), t.entries.len());
    assert_eq!(back.to_bytes(), bytes, "re-encoding is not stable");
}

fuzz_target!(|data: &[u8]| {
    let Some((&mode, rest)) = data.split_first() else {
        return;
    };
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
