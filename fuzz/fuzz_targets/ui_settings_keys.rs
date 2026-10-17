// SPDX-License-Identifier: GPL-3.0-or-later
//! `tessera-settings` `[UI]` UI-scaling keys (`ui-scaling.md` §4.4, §7.1): the reader must
//! accept any settings document without panicking, keep every value inside its documented
//! range and every collection inside its bound, and what it stores must read back to the same
//! preferences without warnings.

#![no_main]

use libfuzzer_sys::fuzz_target;
use tessera_settings::{
    GROUP_SCALE_RANGE, MAX_GROUP_SCALES, MAX_PANEL_DIM, MAX_PANEL_KEY_BYTES, MAX_PANEL_OFFSET,
    MAX_UI_PANELS, SAFE_ASPECT_RANGE, SafeAspect, Settings, TEXT_SCALE_RANGE, UI_SCALE_RANGE,
    UiPrefs, UiScale,
};

/// Checks the bounds and ranges of `ui-scaling.md` §4.1, §4.3, §5.1, §5.8, §7.4 and §7.5.
fn check_bounds(p: &UiPrefs) {
    if let UiScale::Fixed(s) = p.ui_scale {
        assert!(
            (UI_SCALE_RANGE.0..=UI_SCALE_RANGE.1).contains(&s),
            "ui_scale {s} out of range"
        );
    }
    if let SafeAspect::Custom(r) = p.safe_aspect {
        assert!(
            (SAFE_ASPECT_RANGE.0..=SAFE_ASPECT_RANGE.1).contains(&r),
            "safe aspect {r} out of range"
        );
    }
    assert!(
        (TEXT_SCALE_RANGE.0..=TEXT_SCALE_RANGE.1).contains(&p.text_scale),
        "text_scale"
    );
    // The cursor factor must stay finite so the stored value round-trips (ui-scaling.md §6.7,
    // ui-behaviour.md §9.3): a non-finite factor cannot be written back and read as itself.
    assert!(p.scale_cursor.is_finite(), "scale_cursor not finite");

    assert!(p.panels.len() <= MAX_UI_PANELS, "too many panels");
    for (key, placement) in &p.panels {
        assert!(
            !key.is_empty() && key.len() <= MAX_PANEL_KEY_BYTES,
            "panel key"
        );
        assert!(
            placement.dx.is_finite() && placement.dx.abs() <= MAX_PANEL_OFFSET,
            "dx"
        );
        assert!(
            placement.dy.is_finite() && placement.dy.abs() <= MAX_PANEL_OFFSET,
            "dy"
        );
        if let Some(w) = placement.w {
            assert!((1..=MAX_PANEL_DIM).contains(&w), "panel w");
        }
        if let Some(h) = placement.h {
            assert!((1..=MAX_PANEL_DIM).contains(&h), "panel h");
        }
    }

    assert!(p.group_scale.len() <= MAX_GROUP_SCALES, "too many groups");
    for factor in p.group_scale.values() {
        assert!(
            (GROUP_SCALE_RANGE.0..=GROUP_SCALE_RANGE.1).contains(factor),
            "group factor {factor} out of range"
        );
    }
}

/// Reads the `[UI]` keys, checks the bounds, then stores and re-reads: the second read must
/// equal the first and carry no warning (storing writes only canonical, in-range values).
fn exercise(settings: &Settings) {
    let (prefs, _) = UiPrefs::from_settings(settings);
    check_bounds(&prefs);

    let mut stored = settings.clone();
    prefs.store(&mut stored);
    let (again, warnings) = UiPrefs::from_settings(&stored);
    check_bounds(&again);
    assert!(
        warnings.is_empty(),
        "stored [UI] re-read with warnings: {warnings:?}"
    );
    assert_eq!(again, prefs, "the [UI] round trip changed the preferences");
}

fuzz_target!(|data: &[u8]| {
    let Ok(text) = std::str::from_utf8(data) else {
        return;
    };

    // The document as given.
    exercise(&Settings::parse_lenient(text).settings);

    // The bytes treated as the body of the `[UI]` section, so the fuzzer reaches the keys
    // directly (`ui-scaling.md` §4.4, §7.1) rather than mostly producing other sections.
    let mut wrapped = String::with_capacity(text.len() + 5);
    wrapped.push_str("[UI]\n");
    wrapped.push_str(text);
    exercise(&Settings::parse_lenient(&wrapped).settings);
});
