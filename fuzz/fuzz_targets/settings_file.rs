// SPDX-License-Identifier: GPL-3.0-or-later
//! `tessera-settings` file parsing: the lenient loader used at start-up must accept any
//! text without panicking, agree with the strict parser on documents both accept, and
//! produce settings that serialize and parse back to the same value.

#![no_main]

use libfuzzer_sys::fuzz_target;
use tessera_settings::{Settings, parse_did};

fuzz_target!(|data: &[u8]| {
    let Ok(text) = std::str::from_utf8(data) else {
        return;
    };
    let _ = parse_did(text);
    if let Ok(strict) = Settings::from_toml_str(text) {
        // A document the strict parser accepts is clean for the lenient one too.
        assert_eq!(Settings::parse_lenient(text).settings, strict);
    }
    let parsed = Settings::parse_lenient(text);
    let s = &parsed.settings;
    let _ = format!("{s:?}");
    if let Ok(out) = s.to_toml_string() {
        let again = Settings::parse_lenient(&out);
        assert!(
            again.warnings.is_empty(),
            "own output re-parsed with warnings: {:?}",
            again.warnings
        );
        assert_eq!(&again.settings, s, "round trip changed the settings");
    }
});
