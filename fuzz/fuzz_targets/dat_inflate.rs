// SPDX-License-Identifier: GPL-3.0-or-later
//! `tessera_dat::inflate_record` on arbitrary bytes: must never panic, must respect the
//! inflated-size cap, and anything it accepts must have exactly the prefixed length.

#![no_main]

use libfuzzer_sys::fuzz_target;
use tessera_dat::inflate_record;

const CAP: u64 = 1 << 20;

fuzz_target!(|data: &[u8]| {
    if let Ok(out) = inflate_record(data, CAP) {
        let prefix = u32::from_le_bytes([data[0], data[1], data[2], data[3]]);
        assert_eq!(
            out.len() as u64,
            u64::from(prefix),
            "length matches the prefix"
        );
        assert!(out.len() as u64 <= CAP, "cap respected");
    }
});
