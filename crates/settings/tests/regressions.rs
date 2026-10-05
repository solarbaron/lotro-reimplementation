// SPDX-License-Identifier: GPL-3.0-or-later
//! Regression tests for audit findings against the settings crate.

use std::fs;

use tessera_settings::toml;
use tessera_settings::{
    Bookmark, LoadStatus, LoadWarning, MAX_NESTING_DEPTH, Settings, SettingsError, SettingsStore,
};

/// `x = { k.k.(parts).k = { ... 1 } }` nested `levels` inline tables deep.
/// The inline-table and dotted-key limits of the TOML parser multiply, so
/// 80 x 80 builds a table thousands of levels deep from about 13 KB.
fn nested_doc(levels: usize, parts: usize) -> String {
    let key = vec!["k"; parts].join(".");
    let mut s = String::from("x = ");
    for _ in 0..levels {
        s.push_str("{ ");
        s.push_str(&key);
        s.push_str(" = ");
    }
    s.push('1');
    for _ in 0..levels {
        s.push_str(" }");
    }
    s.push('\n');
    s
}

/// Run `f` on a thread with a 1 MiB stack (the default Windows main thread).
fn on_small_stack<T: Send + 'static>(f: impl FnOnce() -> T + Send + 'static) -> T {
    std::thread::Builder::new()
        .stack_size(1024 * 1024)
        .spawn(f)
        .unwrap()
        .join()
        .unwrap()
}

/// Regression: a small, deeply nested document used to overflow the stack in
/// parsing, cloning, serializing and dropping. It is now rejected as a whole
/// before it is turned into a tree, and the store replaces it on save.
#[test]
fn deeply_nested_document_is_rejected_on_a_small_stack() {
    let doc = nested_doc(80, 80);
    assert!(doc.len() < 16 * 1024);
    let dir = tempfile::tempdir().unwrap();
    let path = dir.path().join("settings.toml");
    fs::write(&path, &doc).unwrap();
    let p = path.clone();
    let (lenient, strict, loaded, saved) = on_small_stack(move || {
        let lenient = Settings::parse_lenient(&doc);
        let strict = Settings::from_toml_str(&doc);
        let store = SettingsStore::new(&p);
        let loaded = store.load();
        let saved = store.save(&loaded.settings);
        (lenient, strict.is_err(), loaded, saved)
    });
    assert!(lenient.defaulted);
    assert!(matches!(lenient.warnings[..], [LoadWarning::Syntax { .. }]));
    assert!(strict);
    assert_eq!(loaded.status, LoadStatus::Defaulted);
    assert!(loaded.backup.is_some());
    saved.unwrap();
    let back = SettingsStore::new(&path).load();
    assert_eq!(back.status, LoadStatus::Clean);
}

/// Each kind of nesting stays usable up to the limit and is rejected past it.
#[test]
fn nesting_limit_boundaries() {
    // Moderate nesting a hand-edited file might hold still loads.
    let ok = "[a.b.c]\nd.e = { f = [[1, 2], { g.h = 3 }] }\n";
    let p = Settings::parse_lenient(ok);
    assert!(p.warnings.is_empty(), "{:?}", p.warnings);
    assert!(p.settings.extra.contains_key("a"));
    Settings::from_toml_str(ok).unwrap();
    for doc in [
        nested_doc(2, MAX_NESTING_DEPTH),
        nested_doc(MAX_NESTING_DEPTH, 1),
        format!("[{}]\nx = 1\n", vec!["t"; MAX_NESTING_DEPTH].join(".")),
        format!(
            "a = {}1{}\n",
            "[".repeat(MAX_NESTING_DEPTH + 1),
            "]".repeat(MAX_NESTING_DEPTH + 1)
        ),
    ] {
        let p = Settings::parse_lenient(&doc);
        assert!(p.defaulted, "{doc}");
        assert!(matches!(p.warnings[..], [LoadWarning::Syntax { .. }]));
        assert!(matches!(
            Settings::from_toml_str(&doc),
            Err(SettingsError::Parse(_))
        ));
    }
    // Brackets, dots and comment markers inside strings and comments do
    // not count.
    let quoted = format!(
        "# {open}\n[\"a.b.c.d\"]\nv = \"{open}{dots}\"\nw = '''\n{open}\n'''\nz = \"\"\"{dots}\"\"\"\n",
        open = "[{".repeat(200),
        dots = ".".repeat(200),
    );
    let p = Settings::parse_lenient(&quoted);
    assert!(p.warnings.is_empty(), "{:?}", p.warnings);
}

/// A programmatically built `extra` value deeper than the limit is refused
/// by serialization instead of recursing without bound.
#[test]
fn deep_extra_values_are_not_serialized() {
    let mut v = toml::Value::Integer(1);
    for _ in 0..(MAX_NESTING_DEPTH + 4) {
        v = toml::Value::Array(vec![v]);
    }
    let mut s = Settings::default();
    s.extra.insert("deep".into(), v);
    assert!(matches!(
        s.to_toml_string(),
        Err(SettingsError::Serialize(_))
    ));
}

/// Unknown NaN values compare unequal as `toml::Value`s; their text form is
/// stable across the lenient parser, the strict parser and a round trip.
#[test]
fn nan_extras_round_trip_textually() {
    for doc in [
        "x = nan\n",
        "[game]\nfuture = nan\n",
        "[[explorer.bookmarks]]\ndid = 1\nfield = -nan\n",
    ] {
        let lenient = Settings::parse_lenient(doc);
        assert!(lenient.warnings.is_empty(), "{:?}", lenient.warnings);
        let strict = Settings::from_toml_str(doc).unwrap();
        let out = lenient.settings.to_toml_string().unwrap();
        assert_eq!(strict.to_toml_string().unwrap(), out);
        let again = Settings::parse_lenient(&out);
        assert!(again.warnings.is_empty());
        assert_eq!(again.settings.to_toml_string().unwrap(), out);
    }
}

/// Regression: a lock file planted as a symlink is never followed, so
/// `update` cannot create or open a file anywhere else (for example inside
/// a game install).
#[cfg(unix)]
#[test]
fn update_does_not_follow_a_symlinked_lock_file() {
    let dir = tempfile::tempdir().unwrap();
    let game = dir.path().join("game");
    let cfg = dir.path().join("cfg");
    fs::create_dir_all(&game).unwrap();
    fs::create_dir_all(&cfg).unwrap();
    fs::write(game.join("client_general.dat"), b"").unwrap();
    let planted = game.join("planted.lock");
    std::os::unix::fs::symlink(&planted, cfg.join(".settings.toml.lock")).unwrap();
    let store = SettingsStore::new(cfg.join("settings.toml"));
    let r = store.update(|s| s.push_recent_did(7));
    assert!(matches!(r, Err(SettingsError::Io { .. })), "{r:?}");
    assert!(
        fs::symlink_metadata(&planted).is_err(),
        "lock target created"
    );
    assert!(!cfg.join("settings.toml").exists());

    // A symlink to an existing file is refused as well, and the file is
    // left untouched.
    fs::remove_file(cfg.join(".settings.toml.lock")).unwrap();
    let archive = game.join("client_general.dat");
    std::os::unix::fs::symlink(&archive, cfg.join(".settings.toml.lock")).unwrap();
    let r = store.update(|s| s.push_recent_did(7));
    assert!(matches!(r, Err(SettingsError::Io { .. })), "{r:?}");
    assert_eq!(fs::read(&archive).unwrap(), b"");
}

/// Regression: `update` must not replace an existing settings file it
/// could not read with defaults.
#[cfg(unix)]
#[test]
fn update_refuses_to_clobber_an_unreadable_file() {
    use std::os::unix::fs::PermissionsExt;
    let dir = tempfile::tempdir().unwrap();
    let path = dir.path().join("settings.toml");
    let store = SettingsStore::new(&path);
    let mut s = Settings::default();
    s.remember_game_dir(dir.path().join("elsewhere"));
    for i in 0..10 {
        assert!(s.add_bookmark(Bookmark::new(0x0A00_0000 + i, "mark")));
    }
    store.save(&s).unwrap();
    fs::set_permissions(&path, fs::Permissions::from_mode(0o000)).unwrap();
    if fs::read(&path).is_ok() {
        // Running with privileges that ignore file modes (root).
        fs::set_permissions(&path, fs::Permissions::from_mode(0o600)).unwrap();
        eprintln!("file modes are not enforced; skipping");
        return;
    }
    let r = store.update(|s| s.push_recent_did(7));
    fs::set_permissions(&path, fs::Permissions::from_mode(0o600)).unwrap();
    assert!(
        matches!(r, Err(SettingsError::Io { op: "read", .. })),
        "{r:?}"
    );
    let back = store.load();
    assert_eq!(back.status, LoadStatus::Clean);
    assert_eq!(back.settings, s);
}

/// Regression: the load-side depth bound and the save path must agree. A
/// document that loads cleanly is written back (dotted keys and inline
/// tables become table headers) in a form the next load accepts, so a
/// load, save, load cycle never resets the settings.
#[test]
fn deep_dotted_keys_survive_a_round_trip() {
    let key = |n: usize| vec!["k"; n].join(".");
    let docs = [
        format!("{} = 1\n", key(40)),
        format!("x = {{ {} = 1 }}\n", key(40)),
        format!("{} = 1\n", key(MAX_NESTING_DEPTH - 2)),
        format!("[game]\n{} = 1\n", key(40)),
        format!("[explorer]\n{} = 1\n", key(40)),
        format!("[windows.x]\nwidth = 640\n{} = 1\n", key(40)),
        format!("[[explorer.bookmarks]]\ndid = 1\n{} = 1\n", key(40)),
        format!(
            "[[explorer.bookmarks]]\ndid = 1\n{} = [[1]]\n",
            key(MAX_NESTING_DEPTH - 7)
        ),
    ];
    for doc in docs {
        let p = Settings::parse_lenient(&doc);
        assert!(p.warnings.is_empty(), "{doc}: {:?}", p.warnings);
        assert!(!p.defaulted);
        let strict = Settings::from_toml_str(&doc).unwrap();
        let out = p.settings.to_toml_string().unwrap();
        assert_eq!(strict.to_toml_string().unwrap(), out);
        let again = Settings::parse_lenient(&out);
        assert!(again.warnings.is_empty(), "{out}: {:?}", again.warnings);
        assert_eq!(again.settings.to_toml_string().unwrap(), out);
        Settings::from_toml_str(&out).unwrap();

        // The same through the store: load, save, load keeps everything.
        let dir = tempfile::tempdir().unwrap();
        let path = dir.path().join("settings.toml");
        fs::write(&path, &doc).unwrap();
        let store = SettingsStore::new(&path);
        let first = store.load();
        assert_eq!(first.status, LoadStatus::Clean, "{doc}");
        store.save(&first.settings).unwrap();
        let second = store.load();
        assert_eq!(second.status, LoadStatus::Clean, "{doc}");
        assert_eq!(
            second.settings.to_toml_string().unwrap(),
            first.settings.to_toml_string().unwrap()
        );
    }
}

/// The real depth bound is exact: a dotted key one part longer than fits is
/// rejected, at the root and under each section.
#[test]
fn real_depth_limit_is_exact() {
    let key = |n: usize| vec!["k"; n].join(".");
    let max = MAX_NESTING_DEPTH;
    // (prefix, parts that still fit): the root table is level 1, `[game]`
    // level 2, `[windows.x]` level 3 and a bookmark level 4; a key of `n`
    // parts holding a scalar adds `n - 1` tables.
    for (prefix, fits) in [
        ("", max),
        ("[game]\n", max - 1),
        ("[windows.x]\nwidth = 640\n", max - 2),
        ("[[explorer.bookmarks]]\ndid = 1\n", max - 3),
    ] {
        let ok = format!("{prefix}{} = 1\n", key(fits));
        let p = Settings::parse_lenient(&ok);
        assert!(p.warnings.is_empty(), "{ok}: {:?}", p.warnings);
        let out = p.settings.to_toml_string().unwrap();
        assert!(Settings::parse_lenient(&out).warnings.is_empty(), "{out}");
        let bad = format!("{prefix}{} = 1\n", key(fits + 1));
        let p = Settings::parse_lenient(&bad);
        assert!(p.defaulted, "{bad}");
        assert!(matches!(p.warnings[..], [LoadWarning::Syntax { .. }]));
        assert!(matches!(
            Settings::from_toml_str(&bad),
            Err(SettingsError::Parse(_))
        ));
    }
}

/// Documents that pass the lexical pre-scan but are too deep for the real
/// bound are parsed and rejected without exhausting a small stack.
#[test]
fn prescan_bound_is_stack_safe() {
    let pre = 2 * MAX_NESTING_DEPTH;
    let docs = [
        format!("{} = 1\n", vec!["k"; pre - 1].join(".")),
        format!("a = {}1{}\n", "[".repeat(pre - 1), "]".repeat(pre - 1)),
        nested_doc(pre - 1, 1),
        nested_doc(pre / 4, 3),
        format!("[{}]\n", vec!["t"; pre / 2].join(".")),
    ];
    let results = on_small_stack(move || {
        docs.iter()
            .map(|d| {
                let p = Settings::parse_lenient(d);
                (p.defaulted, p.warnings, Settings::from_toml_str(d).is_err())
            })
            .collect::<Vec<_>>()
    });
    for (defaulted, warnings, strict_err) in results {
        assert!(defaulted);
        assert!(matches!(warnings[..], [LoadWarning::Syntax { .. }]));
        assert!(strict_err);
    }
}

/// Mixed spellings near the limit (table headers, arrays of tables, dotted
/// keys, inline tables, arrays): whatever loads cleanly re-loads cleanly
/// after a save, with the same text.
#[test]
fn mixed_nesting_round_trips_near_the_limit() {
    let mut seed = 0x9E37_79B9_7F4A_7C15_u64;
    let mut next = |n: u64| {
        seed ^= seed << 13;
        seed ^= seed >> 7;
        seed ^= seed << 17;
        usize::try_from(seed % n).unwrap()
    };
    let (mut clean, mut rejected) = (0, 0);
    for _ in 0..600 {
        let mut doc = String::new();
        let header = next(40);
        if header > 0 {
            let aot = next(2) == 0;
            let path = vec!["h"; header].join(".");
            doc.push_str(&if aot {
                format!("[[{path}]]\n")
            } else {
                format!("[{path}]\n")
            });
        }
        let dotted = 1 + next(30);
        doc.push_str(&vec!["d"; dotted].join("."));
        doc.push_str(" = ");
        let mut close = String::new();
        for _ in 0..next(20) {
            match next(3) {
                0 => {
                    doc.push_str("{ i.j = ");
                    close.insert_str(0, " }");
                }
                1 => {
                    doc.push('[');
                    close.insert(0, ']');
                }
                _ => {
                    doc.push_str("[{ t = ");
                    close.insert_str(0, " }]");
                }
            }
        }
        doc.push('1');
        doc.push_str(&close);
        doc.push('\n');
        let p = Settings::parse_lenient(&doc);
        if !p.warnings.is_empty() {
            rejected += 1;
            continue;
        }
        clean += 1;
        let out = p.settings.to_toml_string().unwrap();
        let again = Settings::parse_lenient(&out);
        assert!(
            again.warnings.is_empty(),
            "{doc}\n=> {out}\n{:?}",
            again.warnings
        );
        assert_eq!(again.settings.to_toml_string().unwrap(), out);
    }
    // Both sides of the limit are exercised.
    assert!(clean > 100 && rejected > 100, "{clean} {rejected}");
}
