// SPDX-License-Identifier: GPL-3.0-or-later
//! The world walker's `--avatar` grammar (`apps/client/src/avatar.rs`, included by path so
//! this crate does not pull in the window and GPU stack; `walker.md` §10).
//!
//! The input is (lossy) UTF-8 text. Nothing may panic. A parsed spec has at most
//! `MAX_AVATAR_ITEMS` items, every item DID has an item prefix (0x70–0x76, 0x79), every
//! dye code is finite and in range, an object body has a figure prefix, and the spec's
//! printed form parses back to the same spec.

#![no_main]

#[allow(dead_code)]
#[path = "../../apps/client/src/avatar.rs"]
mod avatar;

use avatar::{AvatarBody, MAX_AVATAR_ITEMS, MAX_DYE, parse_avatar, parse_did};
use libfuzzer_sys::fuzz_target;

fuzz_target!(|data: &[u8]| {
    let text = String::from_utf8_lossy(data);
    let _ = parse_did(&text);
    let Ok(spec) = parse_avatar(&text) else {
        return;
    };
    assert!(spec.items.len() <= MAX_AVATAR_ITEMS);
    for i in &spec.items {
        assert!(matches!(i.did >> 24, 0x70..=0x76 | 0x79));
        for d in [i.dye, i.dye2].into_iter().flatten() {
            assert!(d.is_finite() && (0.0..=MAX_DYE).contains(&d));
        }
    }
    if let AvatarBody::Object(d) = spec.body {
        assert!(matches!(d >> 24, 0x47 | 0x70..=0x76));
    }
    let printed = spec.to_string();
    let again = parse_avatar(&printed).expect("the printed form parses");
    assert_eq!(again, spec, "{printed}");
});
