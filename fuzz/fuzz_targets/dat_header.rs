// SPDX-License-Identifier: GPL-3.0-or-later
//! The fixed-size structures of `docs/specs/dat-container.md` on arbitrary bytes:
//! `Header::parse` (§3), `Entry::parse` (§5.2) and `Archive::from_bytes` header checks.
//!
//! None of them may panic. A 32-byte entry must survive a parse/encode round trip, and an
//! accepted header must agree with the bytes it was parsed from.

#![no_main]

use libfuzzer_sys::fuzz_target;
use tessera_dat::{Archive, ArchiveOptions, ENTRY_LEN, Entry, HEADER_LEN, Header, MAGIC};

fn u32_at(b: &[u8], off: usize) -> u32 {
    let mut w = [0u8; 4];
    w.copy_from_slice(&b[off..off + 4]);
    u32::from_le_bytes(w)
}

fuzz_target!(|data: &[u8]| {
    if let Ok(h) = Header::parse(data) {
        assert!(data.len() >= Header::MIN_LEN);
        assert_eq!(h.magic, MAGIC);
        assert_eq!(h.root_offset, u32_at(data, 0x160));
        assert_eq!(h.root_size, u32_at(data, 0x1A4));
        assert_eq!(
            h.padding_nonzero.is_some(),
            data.len() >= HEADER_LEN as usize
        );
        let _ = (h.dataset_type(), h.archive_kind_code());
        let _ = (h.journal_is_empty(), h.layout_sizes_are_standard());
    }

    if let Some(raw) = data.get(..ENTRY_LEN) {
        let e = Entry::parse(raw);
        assert_eq!(&e.to_bytes()[..], raw, "entry round trip");
        let _ = (e.is_compressed(), e.is_metadata(), e.did_high_byte());
    } else {
        // Short input must not panic either.
        let _ = Entry::parse(data);
    }

    // Header-only archives: every check in the constructor runs, nothing is walked.
    if let Ok(a) = Archive::from_bytes(data.to_vec(), ArchiveOptions::default()) {
        assert!(a.file_len() >= HEADER_LEN);
        let root = a.root();
        assert!(u64::from(root.offset) + u64::from(root.size) <= a.file_len());
    }
});
