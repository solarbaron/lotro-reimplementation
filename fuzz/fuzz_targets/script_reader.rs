// SPDX-License-Identifier: GPL-3.0-or-later
//! The core's neutral, no-execution Lua-table reader (`tessera_script::data`,
//! `docs/specs/lua-plugin-api.md` §10.4 "Reading (Tessera)", decision 0023) on arbitrary
//! bytes, with no numeral hook and with a hook that claims arbitrary bytes after a numeral.
//!
//! Nothing may panic; the limits must hold; a hook that claims bytes may only make the
//! reader accept or reject, never panic or read past the input.

#![no_main]

use libfuzzer_sys::fuzz_target;
use tessera_script::data::{DataValue, NumeralHook, NumeralSite, ParseLimits, parse, parse_with};

const LIMITS: ParseLimits = ParseLimits {
    max_bytes: 64 * 1024,
    max_depth: 24,
    max_entries: 2048,
};

struct Greedy(usize);

impl NumeralHook for Greedy {
    fn extend(
        &self,
        site: NumeralSite,
        text: &[u8],
        value: f64,
        rest: &[u8],
    ) -> Option<(f64, usize)> {
        let _ = (site, text);
        self.0
            .is_multiple_of(3)
            .then(|| (value + 1.0, self.0.min(rest.len() + 5)))
    }
}

fn depth(v: &DataValue) -> usize {
    match v {
        DataValue::Table(t) => 1 + t.values().map(depth).max().unwrap_or(0),
        _ => 0,
    }
}

fn entries(v: &DataValue) -> usize {
    match v {
        DataValue::Table(t) => t.len() + t.values().map(entries).sum::<usize>(),
        _ => 0,
    }
}

fuzz_target!(|data: &[u8]| {
    if let Ok(v) = parse(data, &LIMITS) {
        assert!(depth(&v) <= LIMITS.max_depth);
        assert!(entries(&v) <= LIMITS.max_entries);
    }
    let hook = Greedy(data.first().copied().unwrap_or(0) as usize);
    if let Ok(v) = parse_with(data, &LIMITS, &hook) {
        assert!(depth(&v) <= LIMITS.max_depth);
    }
});
