// SPDX-License-Identifier: GPL-3.0-or-later
//! The small text-config readers of `tessera-install` on arbitrary bytes
//! (`docs/specs/install-layout.md` §5.6.2, §6, §8.3):
//!
//! - `Ini::from_bytes` (`UserPreferences.ini`) and every typed getter, with sections and
//!   keys taken from the parsed document;
//! - `parse_launcher_config` (the launcher's XML-ish `appSettings` reader, which must
//!   reject declarations and stay within its size and depth caps);
//! - `onelauncher::parse_config` (third-party launcher TOML subset);
//! - `translate_windows_path` (Windows paths from configs mapped into a Wine prefix).
//!
//! The first byte selects the reader so each gets the whole remaining input. Nothing may
//! panic.

#![no_main]

use std::path::{Component, Path};

use libfuzzer_sys::fuzz_target;
use tessera_install::launcherconfig::{LauncherConfigError, parse_launcher_config};
use tessera_install::onelauncher::parse_config;
use tessera_install::{Ini, translate_windows_path};

fn ini(data: &[u8]) {
    let doc = Ini::from_bytes(data);
    for s in &doc.sections {
        for e in &s.entries {
            assert!(!e.key.is_empty(), "keys are non-empty");
            assert_eq!(e.key.trim(), e.key, "keys are trimmed");
            assert_eq!(e.value.trim(), e.value, "values are trimmed");
            assert!(doc.get(&s.name, &e.key).is_some(), "lookup finds the key");
            let _ = (
                doc.get_bool(&s.name, &e.key),
                doc.get_i64(&s.name, &e.key),
                doc.get_f64(&s.name, &e.key)
                    .inspect(|f| assert!(f.is_finite())),
                doc.get_resolution(&s.name, &e.key),
            );
        }
    }
    let names = doc.section_names();
    assert!(names.len() <= doc.sections.len());
}

fn launcher(data: &[u8]) {
    match parse_launcher_config(data) {
        Ok(cfg) => {
            for (k, _) in &cfg.settings {
                assert!(cfg.get(k).is_some(), "lookup finds the key");
            }
            let _ = (cfg.game_name(), cfg.document_folder());
        }
        Err(LauncherConfigError::TooLarge) => assert!(data.len() > 64 * 1024),
        Err(_) => {}
    }
}

fn onelauncher(data: &[u8]) {
    let cfg = parse_config(&String::from_utf8_lossy(data));
    let _ = (
        cfg.game_directory,
        cfg.game_settings_directory,
        cfg.game_type,
        cfg.wine_prefix,
    );
}

fn windows_path(data: &[u8]) {
    // A prefix that does not exist: `C:` and other drive letters resolve inside it (or not
    // at all); `Z:` maps to the host root, which is only listed, never written.
    let prefix = Path::new("/nonexistent-tessera-fuzz-prefix");
    if let Some(p) = translate_windows_path(prefix, &String::from_utf8_lossy(data)) {
        assert!(
            !p.components().any(|c| matches!(c, Component::ParentDir)),
            "no '..' in a translated path: {p:?}"
        );
    }
}

fuzz_target!(|data: &[u8]| {
    let Some((&sel, rest)) = data.split_first() else {
        return;
    };
    match sel % 4 {
        0 => ini(rest),
        1 => launcher(rest),
        2 => onelauncher(rest),
        _ => windows_path(rest),
    }
});
