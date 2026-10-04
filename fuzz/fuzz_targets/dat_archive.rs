// SPDX-License-Identifier: GPL-3.0-or-later
//! `tessera_dat::Archive` on arbitrary bytes: opening, lookup, in-order and range
//! iteration, record reads, the free-space table, the membership record and the full
//! validator must never panic and must stay within bounded memory.
//!
//! The first input byte selects options (bit 0: legacy chained layout, bit 1: inflate
//! compressed records while validating); the rest is the archive file. Run with a memory
//! limit so that amplification bugs show up, e.g.
//! `cargo fuzz run dat_archive -- -rss_limit_mb=512 -malloc_limit_mb=256`.

#![no_main]

use libfuzzer_sys::fuzz_target;
use tessera_dat::{Archive, ArchiveOptions, ValidateOptions};

/// Small caps: real limits are far higher, but a fuzzer must not spend memory on them.
const CAP: u64 = 1 << 20;

fuzz_target!(|data: &[u8]| {
    let Some((&flags, file)) = data.split_first() else {
        return;
    };
    let options = ArchiveOptions {
        max_record_len: CAP,
        max_inflated_len: CAP,
        legacy_layout: flags & 1 != 0,
        node_cache_capacity: 64,
        ..ArchiveOptions::default()
    };
    let Ok(a) = Archive::from_bytes(file.to_vec(), options) else {
        return;
    };
    let mut first = None;
    for e in a.entries().take(4096) {
        let Ok(e) = e else { break };
        first.get_or_insert(e.did);
        let _ = a.layout(&e);
        let _ = a.read_entry(&e);
    }
    for did in [0, 0x0100_0000, 0xFFFF_0005, u32::MAX] {
        let _ = a.lookup(did);
    }
    if let Some(did) = first {
        let _ = a.read(did);
        let _ = a.entries_from(did).take(256).count();
        let _ = a
            .entries_range(did..did.saturating_add(0x0100_0000))
            .take(256)
            .count();
    }
    let _ = a.free_table();
    let _ = a.membership();
    let _ = a.validate_with(&ValidateOptions {
        inflate_compressed: flags & 2 != 0,
        max_tracked: 1 << 16,
        ..ValidateOptions::default()
    });
});
