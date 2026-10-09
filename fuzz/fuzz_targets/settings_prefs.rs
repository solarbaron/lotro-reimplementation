// SPDX-License-Identifier: GPL-3.0-or-later
//! `tessera-settings` preference sections: the `[UI]`, `[Input]` and `[Graphics]` readers
//! must accept any settings document without panicking, and what they store must read back
//! to the same preferences.

#![no_main]

use libfuzzer_sys::fuzz_target;
use tessera_settings::{GraphicsPrefs, InputPrefs, Settings, UiPrefs};

fuzz_target!(|data: &[u8]| {
    let Ok(text) = std::str::from_utf8(data) else {
        return;
    };
    let mut settings = Settings::parse_lenient(text).settings;
    let (input, _) = InputPrefs::from_settings(&settings);
    let _ = (input.double_click_ms(None), input.double_click_px(Some(3)));
    let (ui, _) = UiPrefs::from_settings(&settings);
    input.store(&mut settings);
    let (again, warnings) = InputPrefs::from_settings(&settings);
    assert!(
        warnings.is_empty(),
        "stored [Input] re-read with warnings: {warnings:?}"
    );
    assert_eq!(
        again, input,
        "the [Input] round trip changed the preferences"
    );
    ui.store(&mut settings);
    let _ = UiPrefs::from_settings(&settings);
    let (graphics, _) = GraphicsPrefs::from_settings(&settings);
    graphics.store(&mut settings);
    let (again, warnings) = GraphicsPrefs::from_settings(&settings);
    assert!(
        warnings.is_empty(),
        "stored [Graphics] re-read with warnings"
    );
    assert_eq!(
        again, graphics,
        "the [Graphics] round trip changed the preferences"
    );
});
