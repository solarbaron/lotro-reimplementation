// SPDX-License-Identifier: GPL-3.0-or-later
//! The `.plugindata` reader and writer (`docs/specs/lua-plugin-api.md` §10.4, §10.5) on
//! arbitrary bytes.
//!
//! - `plugindata::parse` with small limits must never panic and must respect them.
//! - Anything it accepts must survive `serialize` -> `parse` -> `serialize` unchanged
//!   (compared as bytes, so NaN and `-0` need no special casing).
//! - `format_number` must round-trip every finite double through the reader.
//! - `EngineDate::from_unix_utc` and `DataScope::from_number` must not panic on any
//!   input and must return in-range fields.

#![no_main]

use libfuzzer_sys::fuzz_target;
use tessera_script::plugindata::{format_number, parse, serialize};
use tessera_script::{DataScope, DataValue, EngineDate, ParseLimits};

const LIMITS: ParseLimits = ParseLimits {
    max_bytes: 64 * 1024,
    max_depth: 32,
    max_entries: 4096,
};

/// Limits for re-reading our own output, which may be longer than the input
/// (indentation, bracketed keys, escapes) but never deeper or larger in entry count.
const RELAXED: ParseLimits = ParseLimits {
    max_bytes: 64 * 1024 * 1024,
    max_depth: 32,
    max_entries: 4096,
};

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

fn number_round_trip(n: f64) {
    if !n.is_finite() {
        return;
    }
    let text = format_number(n);
    let src = format!("return {text}");
    match parse(src.as_bytes(), &RELAXED) {
        Ok(DataValue::Number(back)) => {
            assert!(back == n, "{n:e} -> {text:?} -> {back:e}");
        }
        other => panic!("{n:e} -> {text:?} does not read back: {other:?}"),
    }
}

fn date(secs: i64) {
    let d = EngineDate::from_unix_utc(secs);
    assert!((1..=12).contains(&d.month), "{secs}: {d:?}");
    assert!((1..=31).contains(&d.day), "{secs}: {d:?}");
    assert!(
        d.hour < 24 && d.minute < 60 && d.second < 60,
        "{secs}: {d:?}"
    );
    assert!((1..=7).contains(&d.day_of_week), "{secs}: {d:?}");
    assert!((1..=366).contains(&d.day_of_year), "{secs}: {d:?}");
}

fuzz_target!(|data: &[u8]| {
    if let Some(w) = data.first_chunk::<8>() {
        let bits = u64::from_le_bytes(*w);
        number_round_trip(f64::from_bits(bits));
        let _ = DataScope::from_number(f64::from_bits(bits));
        date(i64::from_le_bytes(*w));
    }

    let Ok(v) = parse(data, &LIMITS) else {
        return;
    };
    assert!(depth(&v) <= LIMITS.max_depth, "depth limit");
    assert!(entries(&v) <= LIMITS.max_entries, "entry limit");
    let once = serialize(&v);
    let back = match parse(&once, &RELAXED) {
        Ok(b) => b,
        Err(e) => panic!(
            "own output does not parse: {e}\n{}",
            String::from_utf8_lossy(&once)
        ),
    };
    let twice = serialize(&back);
    assert!(
        once == twice,
        "serialize/parse is not stable:\n{}\n---\n{}",
        String::from_utf8_lossy(&once),
        String::from_utf8_lossy(&twice)
    );
});
