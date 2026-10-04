// SPDX-License-Identifier: GPL-3.0-or-later
//! `tessera_dat::RoutingTable::parse` on arbitrary text: must never panic, and every
//! parsed rule must be well-formed and usable for routing.

#![no_main]

use libfuzzer_sys::fuzz_target;
use tessera_dat::{KindTag, RouteTarget, RoutingTable};

fuzz_target!(|data: &[u8]| {
    let Ok(text) = std::str::from_utf8(data) else {
        return;
    };
    let _ = text.parse::<KindTag>();
    let Ok(table) = RoutingTable::parse(text) else {
        return;
    };
    for rule in table.rules() {
        assert!(rule.lo <= rule.hi, "ranges are ordered");
        assert!(rule.class.is_none_or(|c| c <= 0xF), "class is a nibble");
        assert!(!rule.targets.is_empty(), "rules have targets");
        for t in &rule.targets {
            if let RouteTarget::Region(k) = t {
                assert!(matches!(k, KindTag::Cell | KindTag::Map));
            }
        }
    }
    for did in [0, 0x0100_0001, 0x8013_0000, 0x7A00_0000, u32::MAX] {
        let _ = table.targets(did);
    }
});
