// SPDX-License-Identifier: GPL-3.0-or-later
//! `tessera-settings` `[[CreatedCharacters]]` (the creation choices of
//! `character-screens.md` §7.1): the reader must accept any settings document without
//! panicking, keep its bounds, and what it stores must read back to the same entries
//! without warnings.

#![no_main]

use libfuzzer_sys::fuzz_target;
use tessera_settings::{
    CreatedCharacters, MAX_CREATED_CHARACTERS, MAX_STORED_LOOK_ENTRIES, Settings,
};

fuzz_target!(|data: &[u8]| {
    let Ok(text) = std::str::from_utf8(data) else {
        return;
    };
    let mut settings = Settings::parse_lenient(text).settings;
    let (created, _) = CreatedCharacters::from_settings(&settings);
    assert!(created.entries.len() <= MAX_CREATED_CHARACTERS);
    for c in &created.entries {
        assert!(c.key_is_valid());
        assert!(c.look.len() <= MAX_STORED_LOOK_ENTRIES);
        assert!(c.look.windows(2).all(|w| w[0].property < w[1].property));
        assert!(c.look.iter().all(|e| e.property != 0 && e.value.is_valid()));
    }
    created.store(&mut settings);
    let (again, warnings) = CreatedCharacters::from_settings(&settings);
    assert!(
        warnings.is_empty(),
        "stored entries re-read with warnings: {warnings:?}"
    );
    assert_eq!(again, created, "the round trip changed the entries");
});
