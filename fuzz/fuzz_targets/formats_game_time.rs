// SPDX-License-Identifier: GPL-3.0-or-later
//! Game-time records (prefix 0x35, `docs/specs/environment.md` §6) on arbitrary bytes. The
//! first byte selects whether the rest is the payload as is, or follows a valid own DID so
//! that short inputs reach the calendar tables.
//!
//! Nothing may panic. An accepted record re-encodes byte for byte.
//!
//! Run with a memory cap so that allocation blow-ups count as failures, e.g.
//! `cargo fuzz run formats_game_time -- -malloc_limit_mb=512 -rss_limit_mb=1024`.

#![no_main]

use libfuzzer_sys::fuzz_target;
use tessera_formats::Record;
use tessera_formats::environment::{GameTime, decode_game_time};

fn payload(mode: u8, rest: &[u8]) -> Vec<u8> {
    if mode.is_multiple_of(2) {
        return rest.to_vec();
    }
    let mut out = Vec::with_capacity(4 + rest.len());
    out.extend_from_slice(&0x3500_0000u32.to_le_bytes());
    out.extend_from_slice(rest);
    out
}

fuzz_target!(|input: &[u8]| {
    let [mode, rest @ ..] = input else {
        return;
    };
    let data = &payload(*mode, rest)[..];
    let Ok(g) = decode_game_time(data) else {
        return;
    };
    let bytes = g.to_bytes();
    assert_eq!(bytes, data, "exact re-encoding");
    let again = GameTime::parse(&bytes).expect("re-encoded record decodes");
    assert_eq!(again.did(), g.did());
    let _ = g.calendar_is_ordered();
});
