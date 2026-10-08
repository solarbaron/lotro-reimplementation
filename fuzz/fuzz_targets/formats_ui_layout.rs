// SPDX-License-Identifier: GPL-3.0-or-later
//! UI layout (0x22) decoding (`docs/specs/ui-layout.md` §4, §6, §12.1, §13) on arbitrary
//! bytes, with a total synthetic schema (every PID typed from its value, so the embedded
//! property sets reach every value decoder) and with no schema, both raw and behind a valid
//! 0x22 DID prologue.
//!
//! Nothing may panic; nesting stays within `MAX_ELEMENT_DEPTH`. An accepted record must agree
//! with its own census and invariant checks, and every failure maps to a §13 category.
//! Base chains are followed within the decoded record (its own DID resolves to itself).
//!
//! Run with a memory cap so that allocation blow-ups count as failures, e.g.
//! `cargo fuzz run formats_ui_layout -- -malloc_limit_mb=512 -rss_limit_mb=1024`.

#![no_main]

use libfuzzer_sys::fuzz_target;
use tessera_formats::ui_layout::{FailureCategory, LayoutIssue, MAX_ELEMENT_DEPTH, base_chain};
use tessera_formats::{AnyRecord, NoSchema, PropertyType, UiLayout, decode_ui_layout};

/// Every PID has a type: codes 0 and 23 map to `Unknown`, the rest to types 1-22.
fn total(pid: u32) -> Option<PropertyType> {
    Some(PropertyType::from_code(pid % 24))
}

fn check(l: &UiLayout) {
    let c = l.census();
    assert_eq!(c.elements as usize, l.element_count());
    assert!(c.typed + c.based <= c.elements);
    let issues = l.issues(usize::MAX);
    if !issues
        .iter()
        .any(|i| matches!(i, LayoutIssue::TypeAndBase { .. }))
    {
        // UL5 intact: every element is exactly one of typed and based.
        assert_eq!(c.typed + c.based, c.elements);
    }
    assert!(c.max_depth as usize <= MAX_ELEMENT_DEPTH);
    let by_kind: u64 = c.media_by_kind.iter().sum();
    assert_eq!(by_kind, c.media());
    let mut refs = 0u64;
    l.for_each_reference(|_| refs += 1);
    let mut walked = 0;
    l.walk(|e, depth| {
        walked += 1;
        assert!(depth <= MAX_ELEMENT_DEPTH);
        assert_eq!(e.block.state_id, 0);
        for s in &e.states {
            assert!(e.state(s.state_id).is_some());
        }
        if walked <= 256 {
            let lookup = |d| (d == l.did).then_some(l);
            if let Err(err) = base_chain(e, lookup) {
                let _ = err.category();
            }
        }
    });
}

fn decode(data: &[u8]) {
    for r in [
        decode_ui_layout(data, &total),
        decode_ui_layout(data, &NoSchema),
    ] {
        match r {
            Ok(l) => check(&l),
            Err(e) => {
                let c = FailureCategory::of_decode_error(&e);
                assert!(matches!(
                    c,
                    FailureCategory::RecordDoesNotDecode | FailureCategory::UnknownMediaKind
                ));
            }
        }
    }
}

fuzz_target!(|data: &[u8]| {
    decode(data);
    let mut prefixed = 0x2200_0001u32.to_le_bytes().to_vec();
    prefixed.extend_from_slice(data);
    decode(&prefixed);
    if let Ok(Some(r)) = AnyRecord::decode_with_schema(0x2200_0001u32, &prefixed, &total) {
        assert_eq!(r.did().0, 0x2200_0001);
    }
});
