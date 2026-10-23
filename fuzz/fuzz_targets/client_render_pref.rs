// SPDX-License-Identifier: GPL-3.0-or-later
//! The `--render-pref KEY=VALUE` parser of `tessera-client` (`apps/client/src/render_pref.rs`,
//! included by path so this crate does not pull in the window and GPU stack; `walker.md`
//! §14.10, `far-view.md` FD22).
//!
//! Input layout: the whole input as (lossy) UTF-8 text is parsed as one use of the option;
//! then the text is split at `\n` (at most 32 lines) and every accepted line joins a list
//! the look-up [`render_pref`] runs over with each accepted key in another case.
//!
//! Nothing may panic. What the parser accepts has a non-empty key of at most
//! `MAX_RENDER_PREF_KEY` ASCII letters, digits and `_`, a value of at most
//! `MAX_RENDER_PREF_VALUE` characters without control characters, both trimmed, and parses
//! back to itself from `KEY=VALUE`; the look-up returns the value of the last use of a key.
//! The level-number reading of each accepted value ([`render_pref::capped_level_number`])
//! gives a number only for a value of ASCII digits, never above the cap.

#![no_main]

#[allow(dead_code)]
#[path = "../../apps/client/src/render_pref.rs"]
mod render_pref;

use libfuzzer_sys::fuzz_target;
use render_pref::{
    MAX_RENDER_PREF_KEY, MAX_RENDER_PREF_VALUE, RenderPref, capped_level_number, parse_render_pref,
    render_pref,
};

fn check(p: &RenderPref) {
    assert!(!p.key.is_empty() && p.key.len() <= MAX_RENDER_PREF_KEY);
    assert!(
        p.key
            .bytes()
            .all(|b| b.is_ascii_alphanumeric() || b == b'_')
    );
    assert!(p.value.chars().count() <= MAX_RENDER_PREF_VALUE);
    assert!(!p.value.chars().any(char::is_control));
    assert_eq!(p.key.trim(), p.key);
    assert_eq!(p.value.trim(), p.value);
    let again = parse_render_pref(&format!("{}={}", p.key, p.value));
    assert_eq!(again.as_ref(), Ok(p), "round trip");
    if let Some(n) = capped_level_number(&p.value, 4) {
        assert!(n <= 4);
        assert!(!p.value.is_empty() && p.value.bytes().all(|b| b.is_ascii_digit()));
    }
}

fuzz_target!(|data: &[u8]| {
    let text = String::from_utf8_lossy(data);
    if let Ok(p) = parse_render_pref(&text) {
        check(&p);
    }
    let prefs: Vec<RenderPref> = text
        .split('\n')
        .take(32)
        .filter_map(|line| parse_render_pref(line).ok())
        .collect();
    for (i, p) in prefs.iter().enumerate() {
        check(p);
        let last = prefs
            .iter()
            .skip(i)
            .rev()
            .find(|q| q.key.eq_ignore_ascii_case(&p.key))
            .map(|q| q.value.as_str());
        let flipped: String = p
            .key
            .chars()
            .map(|c| {
                if c.is_ascii_lowercase() {
                    c.to_ascii_uppercase()
                } else {
                    c.to_ascii_lowercase()
                }
            })
            .collect();
        assert_eq!(render_pref(&prefs, &flipped), last);
        assert!(last.is_some());
    }
});
