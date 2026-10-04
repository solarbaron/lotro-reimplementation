// SPDX-License-Identifier: GPL-3.0-or-later
//! String-table message model (`docs/specs/records.md` §3): decoding plus the piece/slot
//! view and the invariant report, on arbitrary bytes.
//!
//! Nothing may panic. For an accepted table, every entry's parts list holds each piece and
//! each argument ID exactly once, in stored order.

#![no_main]

use libfuzzer_sys::fuzz_target;
use tessera_formats::{MessagePart, decode_string_table};

fuzz_target!(|data: &[u8]| {
    let Ok(table) = decode_string_table(data) else {
        return;
    };
    assert_eq!(table.len() as u64, u64::from(table.size.count));
    for e in &table {
        let parts: Vec<MessagePart<'_>> = e.parts().collect();
        let pieces: Vec<_> = parts
            .iter()
            .filter_map(|p| match p {
                MessagePart::Piece(s) => Some(*s),
                MessagePart::Slot(_) => None,
            })
            .collect();
        let slots: Vec<u32> = parts
            .iter()
            .filter_map(|p| match p {
                MessagePart::Slot(id) => Some(*id),
                MessagePart::Piece(_) => None,
            })
            .collect();
        assert_eq!(pieces.len(), e.pieces.len());
        assert!(pieces.iter().zip(&e.pieces).all(|(a, b)| *a == b));
        assert_eq!(slots, e.arg_ids);
        let _ = e.issues().count();
        let _ = e.is_empty_text();
    }
    let _ = table.issues();
    let _ = table.is_bucket_ordered();
});
