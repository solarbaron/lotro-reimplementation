// SPDX-License-Identifier: GPL-3.0-or-later
//! Terrain type tables (prefix 0x19, `docs/specs/terrain-texturing.md` §3) on arbitrary
//! bytes. The first byte selects whether the rest is the payload as is, or follows a valid
//! prologue (own DID and an entry count taken from the low bits of the selector) so that
//! short inputs reach the entries and their float curves.
//!
//! Nothing may panic. An accepted table re-encodes canonically (a fixpoint, never longer
//! than the input), its keys are exactly 0 … n − 1, and the lookups work on it.
//!
//! Run with a memory cap so that allocation blow-ups count as failures, e.g.
//! `cargo fuzz run formats_terrain_type -- -malloc_limit_mb=512 -rss_limit_mb=1024`.

#![no_main]

use libfuzzer_sys::fuzz_target;
use tessera_formats::Record;
use tessera_formats::terrain_type::{TerrainTypeTable, decode_terrain_type_table};

fn payload(mode: u8, rest: &[u8]) -> Vec<u8> {
    if mode.is_multiple_of(2) {
        return rest.to_vec();
    }
    let mut out = Vec::with_capacity(8 + rest.len());
    out.extend_from_slice(&0x1900_0001u32.to_le_bytes());
    out.extend_from_slice(&u32::from((mode >> 1) & 0x3).to_le_bytes());
    out.extend_from_slice(rest);
    out
}

fuzz_target!(|input: &[u8]| {
    let [mode, rest @ ..] = input else {
        return;
    };
    let data = &payload(*mode, rest)[..];
    let Ok(t) = decode_terrain_type_table(data) else {
        return;
    };
    let bytes = t.to_bytes();
    assert!(bytes.len() <= data.len());
    let again = TerrainTypeTable::parse(&bytes).expect("re-encoded table decodes");
    // Compared as bytes: the f32 fields may be NaN.
    assert_eq!(again.to_bytes(), bytes);
    assert_eq!(t.did(), again.did());
    let n = t.len();
    for k in 0..n as u32 {
        let e = t.get(k).expect("every key below n has an entry");
        assert_eq!(e.key, k);
        let _ = e.u_offset.constant();
        let _ = e.v_offset.encoded_len();
        let _ = e.diffuse_scale.to_rgba_f32();
        let _ = e.specular_switches();
    }
    assert!(t.get(n as u32).is_none());
    assert_eq!(t.iter_by_key().count(), n);
    let _ = t.allows_procedural(u32::MAX);
});
