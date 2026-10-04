// SPDX-License-Identifier: GPL-3.0-or-later
//! Real-install checks (skipped unless `TESSERA_GAME_DIR` is set). The crate
//! reads no game data; this only proves that saving a keymap can never land
//! inside the install. Nothing is written there: the guard refuses before
//! any file system change.

use std::path::PathBuf;

use tessera_input::{InputError, KEYMAP_FILE_NAME, default_keymap};

fn game_dir() -> Option<PathBuf> {
    std::env::var_os("TESSERA_GAME_DIR")
        .filter(|v| !v.is_empty())
        .map(PathBuf::from)
        .filter(|p| p.is_dir())
}

#[test]
fn keymap_save_refused_inside_install() {
    let Some(dir) = game_dir() else {
        eprintln!("TESSERA_GAME_DIR not set; skipping");
        return;
    };
    let settings = tessera_settings::Settings::default();
    for target in [
        dir.join(KEYMAP_FILE_NAME),
        dir.join("no-such-subdir").join(KEYMAP_FILE_NAME),
    ] {
        assert!(!target.exists());
        let err = default_keymap().save_file(&target, &settings).unwrap_err();
        assert!(matches!(err, InputError::Guard(_)), "{err}");
        assert!(!target.exists(), "nothing may be written into the install");
    }
    assert!(!dir.join("no-such-subdir").exists());
}

/// A link outside the install that points into it: the guard checks the
/// link's target, so the save is refused and nothing appears there.
#[cfg(unix)]
#[test]
fn keymap_save_through_symlink_refused_inside_install() {
    let Some(dir) = game_dir() else {
        eprintln!("TESSERA_GAME_DIR not set; skipping");
        return;
    };
    let settings = tessera_settings::Settings::default();
    let outside = tempfile::tempdir().unwrap();
    for target in [
        dir.join("tessera-symlink-probe.toml"),
        dir.join("no-such-subdir").join(KEYMAP_FILE_NAME),
    ] {
        assert!(!target.exists());
        let link = outside.path().join("keymap.toml");
        let _ = std::fs::remove_file(&link);
        std::os::unix::fs::symlink(&target, &link).unwrap();
        let err = default_keymap().save_file(&link, &settings).unwrap_err();
        assert!(matches!(err, InputError::Guard(_)), "{err}");
        assert!(!target.exists(), "nothing may be written into the install");
        assert!(
            std::fs::symlink_metadata(&link)
                .unwrap()
                .file_type()
                .is_symlink()
        );
    }
    assert!(!dir.join("no-such-subdir").exists());
}
