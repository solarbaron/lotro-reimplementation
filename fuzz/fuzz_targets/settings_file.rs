// SPDX-License-Identifier: GPL-3.0-or-later
//! `tessera-settings` file parsing: the lenient loader used at start-up must accept any
//! text without panicking, agree with the strict parser on documents both accept, and
//! produce settings that serialize and parse back to the same value.
//!
//! Settings are compared through their serialized text, not with `PartialEq`:
//! unknown keys are kept as `toml::Value`, and a NaN float (`x = nan`) never
//! compares equal to itself although nothing is wrong.

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
        let lenient = Settings::parse_lenient(text).settings;
        assert_eq!(lenient.to_toml_string().ok(), strict.to_toml_string().ok());
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
        assert_eq!(
            again.settings.to_toml_string().ok().as_deref(),
            Some(out.as_str()),
            "round trip changed the settings"
        );
    }
});
