// SPDX-License-Identifier: GPL-3.0-or-later
//! Day descriptions (prefix 0x1A, `docs/specs/environment.md` §5.2–§5.4) on arbitrary bytes.
//! The first byte selects whether the rest is the payload as is, or follows a valid
//! prologue (u32 0 and own DID) so that short inputs reach the keyframes, their float curves
//! and the sky objects.
//!
//! Nothing may panic. An accepted record re-encodes byte for byte (it has no variable-length
//! integers), and the helpers work on it.
//!
//! Run with a memory cap so that allocation blow-ups count as failures, e.g.
//! `cargo fuzz run formats_day_desc -- -malloc_limit_mb=512 -rss_limit_mb=1024`.

#![no_main]

use libfuzzer_sys::fuzz_target;
use tessera_formats::Record;
use tessera_formats::environment::{DayDescription, decode_day_description};

fn payload(mode: u8, rest: &[u8]) -> Vec<u8> {
    if mode.is_multiple_of(2) {
        return rest.to_vec();
    }
    let mut out = Vec::with_capacity(8 + rest.len());
    out.extend_from_slice(&0u32.to_le_bytes());
    out.extend_from_slice(&0x1A00_0001u32.to_le_bytes());
    out.extend_from_slice(rest);
    out
}

fuzz_target!(|input: &[u8]| {
    let [mode, rest @ ..] = input else {
        return;
    };
    let data = &payload(*mode, rest)[..];
    let Ok(d) = decode_day_description(data) else {
        return;
    };
    let bytes = d.to_bytes();
    assert_eq!(bytes, data, "exact re-encoding");
    let again = DayDescription::parse(&bytes).expect("re-encoded record decodes");
    assert_eq!(again.did(), d.did());
    let _ = d.keyframes_ascending();
    let _ = d.object_indices_unique();
    for k in &d.keyframes {
        // EV4: a stored effect has a non-zero object (the extra value follows only then).
        assert!(k.effect.is_none_or(|e| !e.object.is_null()));
        for e in &k.object_entries {
            let _ = d.object(e.object_index);
            let _ = k.object_entry(e.object_index).expect("entry found");
            let _ = e.tint.to_rgba_f32();
        }
        for c in &k.curves {
            assert!((1..=10).contains(&c.mode()));
        }
    }
});
