// SPDX-License-Identifier: GPL-3.0-or-later
//! `tessera-settings` `[GameWindows]` section: the reader must accept any settings document
//! without panicking, every value it keeps must be in range, and what it stores must read
//! back to the same preferences.

#![no_main]

use libfuzzer_sys::fuzz_target;
use tessera_settings::{MAX_FONT_INDEX, Settings, WindowPrefs};

fuzz_target!(|data: &[u8]| {
    let Ok(text) = std::str::from_utf8(data) else {
        return;
    };
    let mut settings = Settings::parse_lenient(text).settings;
    let (prefs, _) = WindowPrefs::from_settings(&settings);
    for i in [prefs.dialogue_font, prefs.quest_log_font]
        .into_iter()
        .flatten()
    {
        assert!(i <= MAX_FONT_INDEX, "font index {i} kept out of range");
    }
    prefs.store(&mut settings);
    // Set keys are written valid; an unset key keeps whatever the file had, which reads as
    // unset again (with its warning).
    let (again, _) = WindowPrefs::from_settings(&settings);
    assert_eq!(
        again, prefs,
        "the [GameWindows] round trip changed the preferences"
    );
});
