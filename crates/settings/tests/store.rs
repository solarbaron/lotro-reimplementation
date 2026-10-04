// SPDX-License-Identifier: GPL-3.0-or-later
//! Filesystem behaviour of `SettingsStore`, using temporary directories.

use std::fs;
use std::path::{Path, PathBuf};

use tessera_settings::{
    Bookmark, LoadStatus, LoadWarning, MAX_BACKUPS, MAX_FILE_SIZE, SCHEMA_VERSION, Settings,
    SettingsError, SettingsStore, WindowGeometry, check_not_inside_game_dirs,
};

fn sample() -> Settings {
    let mut s = Settings::default();
    s.remember_game_dir("/games/install");
    s.set_last_opened(Some("client_surface.dat".into()), Some(0x4100_0123));
    assert!(s.add_bookmark(Bookmark::new(0x0A00_0001, "start")));
    assert!(s.set_window("explorer", WindowGeometry::default()));
    s
}

fn backups(dir: &Path) -> Vec<PathBuf> {
    let mut v: Vec<PathBuf> = fs::read_dir(dir)
        .unwrap()
        .map(|e| e.unwrap().path())
        .filter(|p| p.extension().is_some_and(|e| e == "bak"))
        .collect();
    v.sort();
    v
}

fn names(dir: &Path) -> Vec<String> {
    let mut v: Vec<String> = fs::read_dir(dir)
        .unwrap()
        .map(|e| e.unwrap().file_name().to_string_lossy().into_owned())
        .collect();
    v.sort();
    v
}

#[test]
fn missing_file_gives_defaults_and_writes_nothing() {
    let dir = tempfile::tempdir().unwrap();
    let store = SettingsStore::new(dir.path().join("tessera/settings.toml"));
    let l = store.load();
    assert_eq!(l.status, LoadStatus::Missing);
    assert_eq!(l.settings, Settings::default());
    assert!(l.warnings.is_empty() && l.backup.is_none());
    assert!(store.try_load().unwrap().is_none());
    assert!(names(dir.path()).is_empty());
}

#[test]
fn save_creates_dirs_and_round_trips() {
    let dir = tempfile::tempdir().unwrap();
    let store = SettingsStore::new(dir.path().join("a/b/settings.toml"));
    let s = sample();
    store.save(&s).unwrap();
    let l = store.load();
    assert_eq!(l.status, LoadStatus::Clean);
    assert_eq!(l.settings, s);
    assert_eq!(l.file_schema_version, Some(SCHEMA_VERSION));
    assert_eq!(store.try_load().unwrap(), Some(s.clone()));
    // Overwrite and confirm no temp files are left behind.
    let mut s2 = s;
    s2.push_recent_did(7);
    store.save(&s2).unwrap();
    assert_eq!(store.load().settings, s2);
    assert_eq!(names(&dir.path().join("a/b")), vec!["settings.toml"]);
}

#[test]
fn corrupt_file_is_copied_not_moved() {
    let dir = tempfile::tempdir().unwrap();
    let path = dir.path().join("settings.toml");
    fs::write(&path, "[game\nthis is not toml").unwrap();
    let store = SettingsStore::new(&path);
    assert!(matches!(store.try_load(), Err(SettingsError::Parse(_))));
    assert!(path.exists(), "try_load must not touch the file");
    let l = store.load();
    assert_eq!(l.status, LoadStatus::Defaulted);
    assert_eq!(l.settings, Settings::default());
    assert!(matches!(l.warnings[..], [LoadWarning::Syntax { .. }]));
    let b = l.backup.expect("backup");
    assert_eq!(
        fs::read_to_string(&path).unwrap(),
        "[game\nthis is not toml",
        "load never moves or modifies the file"
    );
    assert!(
        b.file_name()
            .unwrap()
            .to_string_lossy()
            .starts_with("settings.toml.corrupt-")
    );
    assert_eq!(fs::read_to_string(&b).unwrap(), "[game\nthis is not toml");
    // Loading the same damaged file again reuses the identical backup.
    assert_eq!(store.load().backup.as_deref(), Some(b.as_path()));
    assert_eq!(backups(dir.path()).len(), 1);
    // The next save replaces the damaged file.
    store.save(&sample()).unwrap();
    assert_eq!(store.load().status, LoadStatus::Clean);
    assert!(b.exists());
}

#[test]
fn not_utf8_is_copied_and_oversized_is_left_alone() {
    let dir = tempfile::tempdir().unwrap();
    let path = dir.path().join("settings.toml");
    let store = SettingsStore::new(&path);

    fs::write(&path, b"schema_version = 1\n\xff\xfe").unwrap();
    assert!(matches!(store.try_load(), Err(SettingsError::NotUtf8)));
    let l = store.load();
    assert_eq!(l.status, LoadStatus::Defaulted);
    assert!(matches!(l.warnings[..], [LoadWarning::NotUtf8]));
    let b = l.backup.expect("backup");
    assert_eq!(fs::read(&b).unwrap(), b"schema_version = 1\n\xff\xfe");
    assert!(path.exists());

    let big = vec![b'#'; usize::try_from(MAX_FILE_SIZE).unwrap() + 1];
    fs::write(&path, &big).unwrap();
    assert!(matches!(
        store.try_load(),
        Err(SettingsError::TooLarge { .. })
    ));
    let l = store.load();
    assert_eq!(l.status, LoadStatus::Defaulted);
    assert!(matches!(l.warnings[..], [LoadWarning::TooLarge { .. }]));
    assert!(l.backup.is_none());
    assert_eq!(fs::metadata(&path).unwrap().len(), big.len() as u64);
    // Save refuses to replace a file that cannot be ours.
    assert!(matches!(
        store.save(&sample()),
        Err(SettingsError::WontReplace { .. })
    ));
    assert_eq!(fs::metadata(&path).unwrap().len(), big.len() as u64);
}

/// Regression: a wrong settings path pointing at some other file is never
/// renamed away by `load`.
#[test]
fn foreign_files_are_never_moved_by_load() {
    let dir = tempfile::tempdir().unwrap();
    let readme = dir.path().join("readme.txt");
    fs::write(&readme, "just some text\n").unwrap();
    let dat = dir.path().join("archive.dat");
    fs::write(&dat, vec![0u8; usize::try_from(MAX_FILE_SIZE).unwrap() * 2]).unwrap();
    for p in [&readme, &dat] {
        let l = SettingsStore::new(p).load();
        assert_eq!(l.status, LoadStatus::Defaulted);
        assert!(p.exists(), "{} was moved", p.display());
    }
    assert_eq!(fs::read_to_string(&readme).unwrap(), "just some text\n");
    assert_eq!(
        fs::metadata(&dat).unwrap().len(),
        MAX_FILE_SIZE * 2,
        "oversized file untouched"
    );
    assert!(matches!(
        SettingsStore::new(&dat).save(&Settings::default()),
        Err(SettingsError::WontReplace { .. })
    ));
}

#[test]
fn partial_damage_keeps_file_and_copies_backup() {
    let dir = tempfile::tempdir().unwrap();
    let path = dir.path().join("settings.toml");
    let text = "[game]\nactive_dir = 5\n[explorer]\nlast_did = \"0x20\"\n";
    fs::write(&path, text).unwrap();
    let l = SettingsStore::new(&path).load();
    assert_eq!(l.status, LoadStatus::Recovered);
    assert_eq!(l.settings.explorer.last_did, Some(0x20));
    let b = l.backup.expect("backup");
    assert!(b.to_string_lossy().contains(".partial-"));
    assert_eq!(fs::read_to_string(&b).unwrap(), text);
    assert_eq!(
        fs::read_to_string(&path).unwrap(),
        text,
        "original untouched"
    );
}

#[test]
fn newer_schema_is_backed_up_and_extras_survive_save() {
    let dir = tempfile::tempdir().unwrap();
    let path = dir.path().join("settings.toml");
    let text = "schema_version = 9\nnew_thing = 1\n[game]\nactive_dir = \"/g\"\n";
    fs::write(&path, text).unwrap();
    let store = SettingsStore::new(&path);
    let l = store.load();
    assert_eq!(l.status, LoadStatus::Recovered);
    assert_eq!(l.file_schema_version, Some(9));
    assert!(l.backup.unwrap().to_string_lossy().contains(".newer-v9-"));
    store.save(&l.settings).unwrap();
    let saved = fs::read_to_string(&path).unwrap();
    assert!(saved.contains("new_thing = 1"));
    assert!(saved.contains("schema_version = 1"));
}

#[test]
fn backups_are_pruned() {
    let dir = tempfile::tempdir().unwrap();
    let path = dir.path().join("settings.toml");
    let store = SettingsStore::new(&path);
    let mut last = None;
    for i in 0..(MAX_BACKUPS + 5) {
        fs::write(&path, format!("not = = toml {i}")).unwrap();
        last = store.load().backup;
        assert!(last.as_ref().is_some_and(|b| b.exists()));
    }
    assert_eq!(backups(dir.path()).len(), MAX_BACKUPS);
    assert_eq!(
        fs::read_to_string(last.unwrap()).unwrap(),
        format!("not = = toml {}", MAX_BACKUPS + 4)
    );
}

#[test]
fn unreadable_path_is_reported_not_moved() {
    let dir = tempfile::tempdir().unwrap();
    // A directory where the file should be: reading fails, nothing is moved.
    let path = dir.path().join("settings.toml");
    fs::create_dir(&path).unwrap();
    let store = SettingsStore::new(&path);
    let l = store.load();
    assert_eq!(l.status, LoadStatus::Defaulted);
    assert!(matches!(l.warnings[..], [LoadWarning::Unreadable { .. }]));
    assert!(l.backup.is_none() && path.is_dir());
    assert!(matches!(store.try_load(), Err(SettingsError::Io { .. })));
    // Saving over a directory fails cleanly and leaves no temp file.
    assert!(matches!(
        store.save(&sample()),
        Err(SettingsError::Io { .. })
    ));
    assert_eq!(names(dir.path()), vec!["settings.toml"]);
}

#[test]
fn refuses_to_write_inside_a_game_dir() {
    let dir = tempfile::tempdir().unwrap();
    let game = dir.path().join("game");
    fs::create_dir(&game).unwrap();
    let mut s = Settings::default();
    s.remember_game_dir(&game);

    for path in [
        game.join("settings.toml"),
        game.join("sub/dir/settings.toml"),
    ] {
        let store = SettingsStore::new(&path);
        assert!(matches!(
            store.save(&s),
            Err(SettingsError::InsideGameDir { .. })
        ));
    }
    assert!(names(&game).is_empty(), "nothing written in the game dir");

    // Through a `..` detour, caught by canonicalization.
    let detour = dir.path().join("other/../game/settings.toml");
    fs::create_dir(dir.path().join("other")).unwrap();
    assert!(matches!(
        check_not_inside_game_dirs(&detour, &s),
        Err(SettingsError::InsideGameDir { .. })
    ));

    // Regression: a `..` after a directory that does not exist yet.
    let missing = dir.path().join("missing/../game/settings.toml");
    assert!(matches!(
        check_not_inside_game_dirs(&missing, &s),
        Err(SettingsError::InsideGameDir { .. })
    ));
    assert!(matches!(
        SettingsStore::new(&missing).save(&s),
        Err(SettingsError::InsideGameDir { .. })
    ));
    let deeper = dir.path().join("m1/m2/../../game/x/../settings.toml");
    assert!(matches!(
        SettingsStore::new(&deeper).save(&s),
        Err(SettingsError::InsideGameDir { .. })
    ));
    assert!(names(&game).is_empty(), "nothing written in the game dir");

    // Regression: leaving the game dir through `..` is not inside it.
    let out = game.join("../cfg/settings.toml");
    SettingsStore::new(&out).save(&s).unwrap();
    assert!(dir.path().join("cfg/settings.toml").exists());

    // A sibling directory with a common name prefix is fine.
    let sibling = dir.path().join("game2/settings.toml");
    SettingsStore::new(&sibling).save(&s).unwrap();
    assert!(sibling.exists());
}

#[cfg(unix)]
#[test]
fn refuses_to_write_through_a_symlink_into_a_game_dir() {
    let dir = tempfile::tempdir().unwrap();
    let game = dir.path().join("game");
    fs::create_dir(&game).unwrap();
    let link = dir.path().join("cfg");
    std::os::unix::fs::symlink(&game, &link).unwrap();
    let mut s = Settings::default();
    s.remember_game_dir(&game);
    assert!(matches!(
        SettingsStore::new(link.join("settings.toml")).save(&s),
        Err(SettingsError::InsideGameDir { .. })
    ));
    assert!(names(&game).is_empty());
}

#[test]
fn old_file_without_version_loads_clean() {
    let dir = tempfile::tempdir().unwrap();
    let path = dir.path().join("settings.toml");
    fs::write(&path, "[explorer]\nrecent_dids = [1, 2]\n").unwrap();
    let l = SettingsStore::new(&path).load();
    assert_eq!(l.status, LoadStatus::Clean);
    assert!(l.backup.is_none());
    assert_eq!(l.settings.explorer.recent_dids, vec![1, 2]);
}

/// Regression: a damaged list entry is reported, gets a `.partial` backup
/// and a `Recovered` status instead of vanishing silently.
#[test]
fn bad_entries_are_reported_and_backed_up() {
    let dir = tempfile::tempdir().unwrap();
    let path = dir.path().join("settings.toml");
    let text = "[explorer]\nbookmarks = [{ did = \"0x0A00000G\", label = \"my important one\" }, { did = 5 }]\nrecent_dids = [\"0x1\", \"0xZZ\"]\n";
    fs::write(&path, text).unwrap();
    let l = SettingsStore::new(&path).load();
    assert_eq!(l.status, LoadStatus::Recovered);
    assert_eq!(
        l.warnings
            .iter()
            .filter(|w| matches!(w, LoadWarning::Entry { .. }))
            .count(),
        2
    );
    let b = l.backup.expect("backup");
    assert!(b.to_string_lossy().contains(".partial-"));
    assert_eq!(fs::read_to_string(&b).unwrap(), text);
}

fn set_old_mtime(p: &Path) {
    let old = std::time::UNIX_EPOCH + std::time::Duration::from_secs(1_000_000_000);
    fs::File::options()
        .write(true)
        .open(p)
        .unwrap()
        .set_modified(old)
        .unwrap();
}

/// Regression: the backup just made is never pruned, even when its stamp or
/// mtime is the oldest in the directory.
#[test]
fn prune_never_deletes_the_new_backup() {
    let dir = tempfile::tempdir().unwrap();
    let path = dir.path().join("settings.toml");
    for i in 0..MAX_BACKUPS {
        fs::write(
            dir.path()
                .join(format!("settings.toml.partial-999999999999-{}.bak", i + 1)),
            format!("{i}"),
        )
        .unwrap();
    }
    fs::write(&path, "definitely not toml = = =").unwrap();
    set_old_mtime(&path);
    let l = SettingsStore::new(&path).load();
    assert_eq!(l.status, LoadStatus::Defaulted);
    let b = l.backup.expect("backup");
    assert!(b.exists(), "the new backup was pruned");
    assert_eq!(fs::read_to_string(&b).unwrap(), "definitely not toml = = =");
    assert!(path.exists());
    assert_eq!(backups(dir.path()).len(), MAX_BACKUPS);
}

/// Regression: only files matching the crate's exact backup pattern are
/// counted and pruned; the user's own `.bak` files are never touched.
#[test]
fn handmade_backups_are_never_pruned() {
    let dir = tempfile::tempdir().unwrap();
    let path = dir.path().join("settings.toml");
    let mine = [
        "settings.toml.mine.bak",
        "settings.toml.my-handmade.bak",
        "settings.toml.corrupt.bak",
        "settings.toml.corrupt-123.bak",
        "s.toml.bak",
    ];
    for m in mine {
        let p = dir.path().join(m);
        fs::write(&p, "keep me").unwrap();
        set_old_mtime(&p);
    }
    let store = SettingsStore::new(&path);
    for i in 0..(MAX_BACKUPS * 2) {
        fs::write(&path, format!("schema_version = 7\nn = {i}\n")).unwrap();
        assert!(store.load().backup.is_some());
    }
    for m in mine {
        assert!(dir.path().join(m).exists(), "{m} was deleted");
    }
    let ours = backups(dir.path())
        .into_iter()
        .filter(|p| p.to_string_lossy().contains(".newer-v7-"))
        .count();
    assert_eq!(ours, MAX_BACKUPS);
}

/// Regression: repeated loads of another kind of file cannot evict the only
/// backup of a corrupted original.
#[test]
fn newest_backup_of_each_kind_survives() {
    let dir = tempfile::tempdir().unwrap();
    let path = dir.path().join("settings.toml");
    let store = SettingsStore::new(&path);
    fs::write(&path, "corrupt = = =").unwrap();
    let corrupt = store.load().backup.expect("corrupt backup");
    for i in 0..(MAX_BACKUPS * 2) {
        fs::write(&path, format!("schema_version = 2\nn = {i}\n")).unwrap();
        store.load();
    }
    assert!(corrupt.exists());
    assert_eq!(fs::read_to_string(&corrupt).unwrap(), "corrupt = = =");
    assert_eq!(backups(dir.path()).len(), MAX_BACKUPS);
}

/// Regression: loading a settings file that lies inside a recorded game
/// directory writes, renames and deletes nothing there.
#[test]
fn load_inside_a_game_dir_writes_nothing() {
    let dir = tempfile::tempdir().unwrap();
    let game = dir.path().join("game");
    fs::create_dir(&game).unwrap();
    let path = game.join("settings.toml");
    let text = format!(
        "schema_version = 7\n[game]\nactive_dir = {:?}\n",
        game.to_str().unwrap()
    );
    fs::write(&path, &text).unwrap();
    for i in 0..(MAX_BACKUPS + 2) {
        fs::write(
            game.join(format!("settings.toml.partial-00000000000{}.bak", i % 10)),
            "x",
        )
        .unwrap();
    }
    let before = names(&game);
    let l = SettingsStore::new(&path).load();
    assert_eq!(l.status, LoadStatus::Recovered);
    assert!(l.backup.is_none());
    assert!(l.warnings.iter().any(
        |w| matches!(w, LoadWarning::BackupFailed { message } if message.contains("skipped"))
    ));
    assert_eq!(names(&game), before);
    assert_eq!(fs::read_to_string(&path).unwrap(), text);
}

/// A directory that holds `client_general.dat` is an install: nothing is
/// written there even when no game directory is recorded.
#[test]
fn install_marker_makes_a_directory_read_only() {
    let dir = tempfile::tempdir().unwrap();
    for marker in ["client_general.dat", "CLIENT_General.DAT"] {
        let game = dir.path().join(format!(
            "install-{}",
            marker.len() + marker.as_bytes()[0] as usize
        ));
        fs::create_dir_all(game.join("sub")).unwrap();
        fs::write(game.join(marker), b"").unwrap();
        for path in [
            game.join("settings.toml"),
            game.join("sub/new/settings.toml"),
        ] {
            assert!(matches!(
                SettingsStore::new(&path).save(&Settings::default()),
                Err(SettingsError::InsideGameDir { .. })
            ));
        }
        // A damaged file found there is not backed up in place.
        let path = game.join("settings.toml");
        fs::write(&path, "= = =").unwrap();
        let l = SettingsStore::new(&path).load();
        assert!(l.backup.is_none());
        assert_eq!(
            names(&game),
            vec![marker.to_owned(), "settings.toml".into(), "sub".into()]
        );
        assert!(
            SettingsStore::new(&path)
                .update(|s| s.push_recent_did(1))
                .is_err()
        );
        assert_eq!(
            names(&game),
            vec![marker.to_owned(), "settings.toml".into(), "sub".into()]
        );
    }
}

/// Regression: a FIFO at the settings path never blocks loading or saving.
#[cfg(unix)]
#[test]
fn fifo_at_settings_path_does_not_block() {
    use std::sync::mpsc;
    use std::time::Duration;
    let dir = tempfile::tempdir().unwrap();
    let path = dir.path().join("settings.toml");
    let status = std::process::Command::new("mkfifo")
        .arg(&path)
        .status()
        .expect("mkfifo");
    assert!(status.success());
    let (tx, rx) = mpsc::channel();
    let p = path.clone();
    std::thread::spawn(move || {
        let store = SettingsStore::new(&p);
        let l = store.load();
        let strict = store.try_load();
        let saved = store.save(&Settings::default());
        let _ = tx.send((l, strict.is_err(), saved));
    });
    let (l, strict_err, saved) = rx
        .recv_timeout(Duration::from_secs(10))
        .expect("load blocked on a FIFO");
    assert_eq!(l.status, LoadStatus::Defaulted);
    assert!(matches!(l.warnings[..], [LoadWarning::Unreadable { .. }]));
    assert!(l.backup.is_none());
    assert!(strict_err);
    assert!(matches!(saved, Err(SettingsError::WontReplace { .. })));
}

/// Regression: saving through a symlinked settings file updates the link
/// target and keeps the link.
#[cfg(unix)]
#[test]
fn save_writes_through_a_symlink() {
    let dir = tempfile::tempdir().unwrap();
    let real_dir = dir.path().join("dotfiles");
    fs::create_dir(&real_dir).unwrap();
    let real = real_dir.join("tessera.toml");
    fs::write(&real, "").unwrap();
    let link = dir.path().join("settings.toml");
    std::os::unix::fs::symlink(&real, &link).unwrap();
    let store = SettingsStore::new(&link);
    let s = sample();
    store.save(&s).unwrap();
    assert!(
        fs::symlink_metadata(&link)
            .unwrap()
            .file_type()
            .is_symlink()
    );
    assert_eq!(
        Settings::from_toml_str(&fs::read_to_string(&real).unwrap()).unwrap(),
        s
    );
    assert_eq!(store.load().settings, s);
    assert_eq!(names(&real_dir), vec!["tessera.toml"], "no temp file left");

    // A dangling relative link creates its target.
    let link2 = dir.path().join("other.toml");
    std::os::unix::fs::symlink("dotfiles/new/other-real.toml", &link2).unwrap();
    SettingsStore::new(&link2).save(&s).unwrap();
    assert!(
        fs::symlink_metadata(&link2)
            .unwrap()
            .file_type()
            .is_symlink()
    );
    assert!(real_dir.join("new/other-real.toml").is_file());

    // Backups of a symlinked file land next to the real file.
    fs::write(&real, "= = =").unwrap();
    let b = store.load().backup.expect("backup");
    assert_eq!(b.parent().unwrap(), fs::canonicalize(&real_dir).unwrap());
}

/// Regression: two programs sharing one file through `update` keep each
/// other's changes.
#[test]
fn update_merges_concurrent_changes() {
    let dir = tempfile::tempdir().unwrap();
    let path = dir.path().join("cfg/settings.toml");
    let explorer = SettingsStore::new(&path);
    let client = SettingsStore::new(&path);
    explorer.update(|s| s.push_recent_did(42)).unwrap();
    let after = client
        .update(|s| {
            assert!(s.set_window("client", WindowGeometry::default()));
        })
        .unwrap();
    assert_eq!(after.explorer.recent_dids, vec![42]);
    let l = explorer.load();
    assert_eq!(l.settings.explorer.recent_dids, vec![42]);
    assert!(l.settings.window("client").is_some());

    let threads: Vec<_> = (0..8u32)
        .map(|i| {
            let p = path.clone();
            std::thread::spawn(move || {
                SettingsStore::new(&p)
                    .update(|s| s.push_recent_did(1000 + i))
                    .unwrap();
            })
        })
        .collect();
    for t in threads {
        t.join().unwrap();
    }
    let recent = explorer.load().settings.explorer.recent_dids;
    for i in 0..8u32 {
        assert!(recent.contains(&(1000 + i)), "lost update {i}: {recent:?}");
    }
}

/// Real-data check: remember the user's install, round-trip it through a
/// temporary settings file, and confirm the guard rejects a settings path
/// inside the install. Never writes to the install.
#[test]
fn real_game_dir_round_trip() {
    let Some(game) = std::env::var_os("TESSERA_GAME_DIR").filter(|v| !v.is_empty()) else {
        eprintln!("TESSERA_GAME_DIR unset; skipping");
        return;
    };
    let game = PathBuf::from(game);
    assert!(game.is_dir(), "TESSERA_GAME_DIR is not a directory");
    let dir = tempfile::tempdir().unwrap();
    let store = SettingsStore::new(dir.path().join("settings.toml"));
    let mut s = Settings::default();
    s.remember_game_dir(&game);
    s.set_last_opened(Some("client_general.dat".into()), Some(0x0A00_0001));
    store.save(&s).unwrap();
    let l = store.load();
    assert_eq!(l.status, LoadStatus::Clean);
    assert_eq!(l.settings.game.active_dir.as_deref(), Some(game.as_path()));
    // Pure check only: no save is attempted against the install.
    assert!(matches!(
        check_not_inside_game_dirs(&game.join("tessera-settings.toml"), &s),
        Err(SettingsError::InsideGameDir { .. })
    ));
    assert!(!game.join("tessera-settings.toml").exists());
}
