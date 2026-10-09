// SPDX-License-Identifier: GPL-3.0-or-later
//! The explorer's DID search box and hex view (`apps/explorer/src/query.rs` and
//! `hex.rs`, included by path): `parse_query` on arbitrary text, and `hex_line` on
//! arbitrary record bytes at every line, the last partial one and positions far past the
//! end; `human_bytes` and `thousands` on arbitrary counts.
//!
//! Nothing may panic. Hex queries give ordered ranges and round-trip through their
//! canonical forms; every line of the dump up to `line_count` has the fixed layout, and
//! lines past it are empty.

#![no_main]

#[path = "../../apps/explorer/src/hex.rs"]
#[allow(dead_code)]
mod hex;
#[path = "../../apps/explorer/src/query.rs"]
#[allow(dead_code)]
mod query;

use hex::{BYTES_PER_LINE, hex_line, human_bytes, line_count, thousands};
use libfuzzer_sys::fuzz_target;
use query::{DidQuery, parse_query};

fuzz_target!(|data: &[u8]| {
    // The search box: the input as text.
    let text = String::from_utf8_lossy(data);
    match parse_query(&text) {
        DidQuery::Exact(d) => {
            assert_eq!(parse_query(&format!("0x{d:08x}")), DidQuery::Exact(d));
            assert_eq!(query_range(&text), Some((d, d)));
        }
        DidQuery::Range { lo, hi } => {
            // A prefix of k hex digits: the span is 16^(8 - k) - 1 and lo is aligned to it.
            assert!(lo <= hi);
            let span = hi - lo;
            assert!((u64::from(span) + 1).is_power_of_two());
            assert_eq!(span.count_ones() % 4, 0);
            assert_eq!(lo & span, 0);
            let digits = 8 - span.count_ones() as usize / 4;
            let canonical = if digits == 0 {
                "*".to_owned()
            } else {
                format!("0x{:0width$x}*", lo >> (32 - 4 * digits), width = digits)
            };
            assert_eq!(parse_query(&canonical), DidQuery::Range { lo, hi });
        }
        DidQuery::Name(n) => assert!(!n.is_empty()),
        DidQuery::Empty => assert!(text.trim().is_empty()),
    }

    // The hex view: the input as record bytes.
    let lines = line_count(data.len());
    for line in 0..lines.min(64) {
        let s = hex_line(data, line);
        assert!(s.starts_with(&format!("{:08x}", line * BYTES_PER_LINE)));
        assert!(s.ends_with('|'));
    }
    if lines > 0 {
        let last = hex_line(data, lines - 1);
        assert!(!last.is_empty());
    }
    assert!(hex_line(data, lines).is_empty());
    assert!(hex_line(data, usize::MAX).is_empty());
    assert!(hex_line(data, usize::MAX / BYTES_PER_LINE + 1).is_empty());

    let mut n = [0u8; 8];
    for (d, s) in n.iter_mut().zip(data) {
        *d = *s;
    }
    let n = u64::from_le_bytes(n);
    let _ = human_bytes(n);
    let t = thousands(n);
    assert_eq!(t.replace(',', ""), n.to_string());
});

/// The range of a hex query, as (lo, hi).
fn query_range(text: &str) -> Option<(u32, u32)> {
    parse_query(text).range().map(|r| (*r.start(), *r.end()))
}
