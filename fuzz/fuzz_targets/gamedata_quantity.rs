// SPDX-License-Identifier: GPL-3.0-or-later
//! The crafting panel's quantity entry (`tessera_gamedata::crafting::quantity`,
//! `crafting-panels.md` §5.6, CP13): `read` of the user's typed text, `limit_text`,
//! `after_typing`, `shown`, the arrows, Make All and the quantity Make sends, for hostile
//! text and any makeable count *M*.
//!
//! Input layout: byte 0 picks how *M* is made (bit 0 clear: byte 1 as a small count; set:
//! bytes 1-8 as a little-endian u64), byte 9 (or byte 2) a value for the arrows, and the
//! rest, read as UTF-8 with replacement characters, is the entry text.
//!
//! Nothing may panic, and the documented rules hold: an empty entry reads *m*₀, a text of
//! at most 18 ASCII digits reads its decimal value, a limited text has at most
//! `QUANTITY_CHARS` characters, typed values and arrows stay within *m*₀ … *M*, the shown
//! value is the limited entry's value (1 when that and *M* are 0), Make All is at most 999
//! and Make sends 1 … 999 or nothing for 0.

#![no_main]

use libfuzzer_sys::fuzz_target;
use tessera_gamedata::crafting::quantity::{
    after_typing, clamp, decrease, floor, increase, limit_text, make_all, read, shown, to_send,
};
use tessera_gamedata::crafting::{MAKE_ALL_MAX, QUANTITY_CHARS};

fuzz_target!(|data: &[u8]| {
    let Some((&mode, rest)) = data.split_first() else {
        return;
    };
    let (m, rest) = if mode & 1 == 0 {
        let (&b, rest) = rest.split_first().unwrap_or((&0, &[]));
        (u64::from(b), rest)
    } else {
        let mut word = [0u8; 8];
        let n = rest.len().min(8);
        word[..n].copy_from_slice(&rest[..n]);
        (u64::from_le_bytes(word), &rest[n..])
    };
    let (&v, rest) = rest.split_first().unwrap_or((&0, &[]));
    let value = if mode & 2 == 0 {
        u64::from(v)
    } else {
        m.saturating_add(u64::from(v))
    };
    let text = String::from_utf8_lossy(rest);
    let lo = floor(m);
    let hi = m.max(lo);

    let r = read(&text, m);
    if text.is_empty() {
        assert_eq!(r, lo, "an empty entry reads m0");
    } else if text.len() <= 18 && text.bytes().all(|b| b.is_ascii_digit()) {
        assert_eq!(
            Ok(r),
            text.parse::<u64>(),
            "digits read as their decimal value"
        );
    }
    let limited = limit_text(&text);
    assert!(limited.chars().count() <= QUANTITY_CHARS);
    assert!(text.starts_with(&limited), "the limited text is a prefix");

    let typed = after_typing(&text, m);
    assert!(
        (lo..=hi).contains(&typed),
        "a typed value is within m0 ... M"
    );
    assert_eq!(typed, clamp(read(&limited, m), m));
    let s = shown(&text, m);
    let read_limited = read(&limited, m);
    if read_limited == 0 && m == 0 {
        assert_eq!(s, 1, "with nothing read and M = 0 the shown value is 1");
    } else {
        assert_eq!(
            s, read_limited,
            "the shown value is the limited entry's value"
        );
    }
    assert!((lo..=hi).contains(&clamp(value, m)));
    assert!(increase(value, m) <= hi, "the increase arrow stops at M");
    assert!(decrease(value, m) >= lo, "the decrease arrow stops at m0");
    assert!(make_all(m) <= u64::from(MAKE_ALL_MAX));
    match to_send(value) {
        None => assert_eq!(value, 0, "only 0 sends nothing"),
        Some(q) => assert!((1..=MAKE_ALL_MAX).contains(&u32::from(q))),
    }
    let _ = to_send(s);
});
