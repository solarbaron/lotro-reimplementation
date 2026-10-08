// SPDX-License-Identifier: GPL-3.0-or-later
//! Sky descriptions (prefix 0x15, `docs/specs/environment.md` §5.1) on arbitrary bytes. The
//! first byte selects whether the rest is the payload as is, or follows a valid header
//! (version 1, own DID, four intervals, game-time DID) so that short inputs reach the day
//! groups, named lists and sky entries.
//!
//! Nothing may panic. An accepted record re-encodes canonically (a fixpoint, never longer
//! than the input), and the look-ups work on it.
//!
//! Run with a memory cap so that allocation blow-ups count as failures, e.g.
//! `cargo fuzz run formats_sky -- -malloc_limit_mb=512 -rss_limit_mb=1024`.

#![no_main]

use libfuzzer_sys::fuzz_target;
use tessera_formats::Record;
use tessera_formats::environment::{SkyDescription, decode_sky_description};

fn payload(mode: u8, rest: &[u8]) -> Vec<u8> {
    if mode.is_multiple_of(2) {
        return rest.to_vec();
    }
    let mut out = Vec::with_capacity(44 + rest.len());
    out.extend_from_slice(&1u32.to_le_bytes());
    out.extend_from_slice(&0x1500_0007u32.to_le_bytes());
    for v in [1.0f64, 1.0, 180.0, 180.0] {
        out.extend_from_slice(&v.to_le_bytes());
    }
    out.extend_from_slice(&0x3500_0000u32.to_le_bytes());
    out.extend_from_slice(rest);
    out
}

fuzz_target!(|input: &[u8]| {
    let [mode, rest @ ..] = input else {
        return;
    };
    let data = &payload(*mode, rest)[..];
    let Ok(s) = decode_sky_description(data) else {
        return;
    };
    let bytes = s.to_bytes();
    assert!(bytes.len() <= data.len());
    let again = SkyDescription::parse(&bytes).expect("re-encoded record decodes");
    // Compared as bytes: weights and intervals may be NaN.
    assert_eq!(again.to_bytes(), bytes);
    assert_eq!(again.did(), s.did());
    let _ = s.day_descriptions().count();
    for g in &s.groups {
        let _ = s.resolve_follow_list(&g.follow).map(|l| l.total_weight());
        // Only the first group of a day type is consulted (§11.2 step 1).
        let first = s.group(g.day_type).is_some_and(|f| std::ptr::eq(f, g));
        for e in &g.entries {
            let found = s.sky_entry(g.day_type, e.sky_key);
            assert!(!first || found.is_some(), "entry of the first group found");
            if let Some(found) = found {
                let _ = s.resolve_sky_list(&found.list);
            }
        }
    }
    for e in &s.local_defaults {
        assert!(s.local_default(e.sky_key).is_some());
    }
});
