// SPDX-License-Identifier: GPL-3.0-or-later
//! The client's bridge from `[UI.panels]` of the settings file to the UI's saved-placement
//! store (`apps/client/src/ui_host/placements.rs`, included by path so this crate does not
//! pull in the window and GPU stack; `ui-scaling.md` §5.8): the panel-key reader
//! `parse_panel_key` and the field copies `store_from_prefs` / `prefs_from_store`. The file
//! is the user's, so its keys and values are hostile.
//!
//! Byte 0 selects the mode: even, the rest is a settings document (lossy UTF-8) whose
//! `[UI.panels]` table goes through the bridge; odd, each line of the rest is a panel key.
//!
//! Nothing may panic. A key that reads gives a key that is written back and reads to the
//! same (layout, element) pair; the store holds exactly the entries whose keys read; a
//! placement copied to the store and back once is stable under a second copy.

#![no_main]

// The included file logs with `tracing::debug!`; this crate has no `tracing` dependency,
// so the name points at this crate and its `debug!` swallows the arguments.
extern crate self as tracing;

/// Stand-in for `tracing::debug!` (see above).
#[macro_export]
macro_rules! debug {
    ($($t:tt)*) => {
        ()
    };
}

#[allow(dead_code, clippy::single_match)]
#[path = "../../apps/client/src/ui_host/placements.rs"]
mod placements;

use libfuzzer_sys::fuzz_target;
use tessera_settings::{Settings, UiPrefs};

use placements::{panel_key, parse_panel_key, prefs_from_store, store_from_prefs};

fn key(text: &str) {
    if let Some((l, e)) = parse_panel_key(text) {
        assert_eq!(parse_panel_key(&panel_key(l, e)), Some((l, e)), "{text:?}");
    }
}

fuzz_target!(|data: &[u8]| {
    let Some((&mode, rest)) = data.split_first() else {
        return;
    };
    let text = String::from_utf8_lossy(rest);
    if mode & 1 == 1 {
        for line in text.lines().take(64) {
            key(line);
        }
        return;
    }
    let settings = Settings::parse_lenient(&text).settings;
    let (prefs, _) = UiPrefs::from_settings(&settings);
    for k in prefs.panels.keys() {
        key(k);
    }
    let store = store_from_prefs(&prefs.panels);
    let readable = prefs
        .panels
        .keys()
        .filter_map(|k| parse_panel_key(k))
        .collect::<std::collections::BTreeSet<_>>();
    assert_eq!(
        store.placements.len(),
        readable.len(),
        "store lost or invented entries"
    );
    let once = prefs_from_store(&store);
    assert_eq!(once.len(), store.placements.len());
    for k in once.keys() {
        assert!(
            parse_panel_key(k).is_some(),
            "written key {k:?} does not read"
        );
    }
    let twice = prefs_from_store(&store_from_prefs(&once));
    for (k, a) in &once {
        let Some(b) = twice.get(k) else {
            panic!("key {k:?} lost on the second copy");
        };
        // The offsets are rounded to two decimals in `f32`; within ordinary panel offsets
        // a second rounding changes nothing.
        if a.dx.abs() < 1.0e5 && a.dy.abs() < 1.0e5 {
            assert_eq!(a, b, "placement {k:?} changed on the second copy");
        }
    }
});
