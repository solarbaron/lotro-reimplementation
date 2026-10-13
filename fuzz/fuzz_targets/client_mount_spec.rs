// SPDX-License-Identifier: GPL-3.0-or-later
//! The walker's `--mount SPEC` and `/ride` grammar (`apps/client/src/walker/mount/spec.rs`,
//! included by path so this crate does not pull in the window and GPU stack; `walker.md`
//! §16.2, §16.3).
//!
//! The input is (lossy) UTF-8 text. Nothing may panic. A parsed spec has an index of at most
//! 9,999, a lock gait of 4–7, and its printed form parses back to the same spec; a parsed
//! `/ride list` page lies in 1–999.

#![no_main]

#[allow(dead_code)]
#[path = "../../apps/client/src/walker/mount/spec.rs"]
mod spec;

use libfuzzer_sys::fuzz_target;
use spec::{
    MAX_INDEX, MAX_PAGE, MountSource, RideCommand, parse_did_text, parse_mount, parse_ride,
};

fuzz_target!(|data: &[u8]| {
    let text = String::from_utf8_lossy(data);
    let _ = parse_did_text(&text);
    if let Ok(RideCommand::List(page)) = parse_ride(&text) {
        assert!((1..=MAX_PAGE).contains(&page));
    }
    let Ok(s) = parse_mount(&text) else {
        return;
    };
    if let MountSource::Dir(i) = s.source {
        assert!(i <= MAX_INDEX);
    }
    if let Some(g) = s.lock {
        assert!((4..=7).contains(&g));
    }
    let printed = s.to_string();
    let again = parse_mount(&printed).expect("the printed form parses");
    assert_eq!(again, s, "{printed}");
});
