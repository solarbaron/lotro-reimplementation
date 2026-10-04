// SPDX-License-Identifier: GPL-3.0-or-later
//! Directory-node parsing (`docs/specs/dat-container.md` §5) with a valid file header.
//!
//! `dat_archive` fuzzes whole files, but a random file almost never gets past the header
//! checks. This target writes a well-formed header itself (§3.3) and lets the fuzzer
//! control only the root node payload (§5.1) and whatever follows it in the file, so the
//! node, lookup, traversal, extent and free-table code is reached on every run.
//!
//! Input layout: byte 0 selects options (bit 0: legacy layout, bit 1: inflate while
//! validating, bits 2-3: minimum allocation size), then up to 2452 bytes of root payload
//! (zero-padded), then the bytes that follow the root node in the file.

#![no_main]

use libfuzzer_sys::fuzz_target;
use tessera_dat::{
    Archive, ArchiveOptions, CURRENT_ENGINE_VERSION, ENTRY_LEN, HEADER_LEN, MAGIC, MIN_NODE_ALLOC,
    NODE_PAYLOAD_LEN, ValidateOptions,
};

const CAP: u64 = 1 << 20;

fn put(buf: &mut [u8], off: usize, v: u32) {
    buf[off..off + 4].copy_from_slice(&v.to_le_bytes());
}

fuzz_target!(|data: &[u8]| {
    let Some((&flags, rest)) = data.split_first() else {
        return;
    };
    let (payload, tail) = rest.split_at(rest.len().min(NODE_PAYLOAD_LEN));

    let header_len = HEADER_LEN as usize;
    let root_alloc = MIN_NODE_ALLOC as usize;
    let mut file = vec![0u8; header_len + root_alloc];
    // Root node allocation: 8-byte header {0, 0} (§4.1), then the fuzzed payload.
    file[header_len + 8..header_len + 8 + payload.len()].copy_from_slice(payload);
    file.extend_from_slice(tail);
    let len = u32::try_from(file.len()).unwrap_or(u32::MAX);

    // Superblock (§3.3). Free-space table is empty: offset 0, size 0, count 0.
    let min_alloc = [0x100, 0x400, 0x1000, 0x4400][usize::from((flags >> 2) & 3)];
    // Empty journal pattern (§3.2).
    file[0x100..0x104].copy_from_slice(&[0x00, 0x50, 0x4C, 0x00]);
    put(&mut file, 0x140, MAGIC);
    put(&mut file, 0x144, min_alloc);
    put(&mut file, 0x148, len);
    // Dataset descriptor (§3.4): type 1 (portal) in byte 0x14F, kind 3 in byte 0x14E.
    put(&mut file, 0x14C, 0x0103_0000);
    put(&mut file, 0x160, HEADER_LEN as u32);
    put(&mut file, 0x174, CURRENT_ENGINE_VERSION);
    put(&mut file, 0x178, 3);
    put(&mut file, 0x190, 0x1A8 - 0x140);
    put(&mut file, 0x194, ENTRY_LEN as u32);
    put(&mut file, 0x198, 8);
    put(&mut file, 0x1A4, MIN_NODE_ALLOC);

    let options = ArchiveOptions {
        max_record_len: CAP,
        max_inflated_len: CAP,
        legacy_layout: flags & 1 != 0,
        node_cache_capacity: 16,
        ..ArchiveOptions::default()
    };
    let a = match Archive::from_bytes(file, options) {
        Ok(a) => a,
        Err(e) => panic!("synthetic header must be accepted: {e}"),
    };

    let mut prev = None;
    for e in a.entries().take(4096) {
        let Ok(e) = e else { break };
        if let Some(p) = prev {
            assert!(e.did > p, "in-order traversal is strictly ascending");
        }
        prev = Some(e.did);
        // Whatever traversal yields, lookup must find again.
        if let Ok(found) = a.lookup(e.did) {
            assert_eq!(found.map(|f| f.did), Some(e.did));
        }
        let _ = a.layout(&e);
        let _ = a.read_entry(&e);
    }
    for did in [0, 1, 0x0100_0000, 0x7FFF_FFFF, 0xFFFF_0005, u32::MAX] {
        let _ = a.lookup(did);
    }
    let _ = a.free_table();
    let _ = a.membership();
    let _ = a.validate_with(&ValidateOptions {
        inflate_compressed: flags & 2 != 0,
        max_tracked: 1 << 16,
        ..ValidateOptions::default()
    });
});
