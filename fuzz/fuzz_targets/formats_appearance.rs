// SPDX-License-Identifier: GPL-3.0-or-later
//! Appearance-record decoding (prefix 0x20, `docs/specs/setup.md` §8) on arbitrary bytes.
//!
//! Nothing may panic. An accepted record must survive an encode → decode → encode round
//! trip byte for byte, keep unique entry keys and a key table as long as its entry list.
//!
//! Run with a memory cap so that allocation blow-ups count as failures, e.g.
//! `cargo fuzz run formats_appearance -- -malloc_limit_mb=512 -rss_limit_mb=1024`.

#![no_main]

use libfuzzer_sys::fuzz_target;
use tessera_formats::{AppearanceRecord, Record};

fuzz_target!(|data: &[u8]| {
    if let Ok(a) = AppearanceRecord::parse(data) {
        assert_eq!(a.entries.len(), a.key_table.len());
        let _ = a.first_unusual_field();
        for e in &a.entries {
            assert_eq!(a.entry(e.key).map(|x| x.key), Some(e.key));
            if let Some(b) = e.base_block() {
                let _ = b.meshes().count();
            }
        }
        let bytes = a.to_bytes();
        let again = AppearanceRecord::parse(&bytes).expect("re-encoded record decodes");
        assert_eq!(again.to_bytes(), bytes);
    }
});
