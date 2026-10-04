// SPDX-License-Identifier: GPL-3.0-or-later
//! Real-data tests. They read the user's own install, strictly read-only, and
//! are skipped when `TESSERA_GAME_DIR` is unset.

use std::path::PathBuf;

use tessera_install::{
    ArchiveKind, GameInstall, SearchEnv, SettingsQuery, Source, SplitSetStatus, discover_with,
    validate,
};

fn game_dir() -> Option<PathBuf> {
    std::env::var_os("TESSERA_GAME_DIR")
        .filter(|v| !v.is_empty())
        .map(PathBuf::from)
}

#[test]
fn real_install_validates() {
    let Some(dir) = game_dir() else {
        eprintln!("TESSERA_GAME_DIR not set; skipping real-data test");
        return;
    };
    let report = validate(&dir);
    assert!(report.usable, "problems: {:?}", report.problems);
    for w in &report.warnings {
        eprintln!("warning: {w}");
    }
    // §9.6 invariants that need only headers.
    for a in &report.archives {
        let name = a.file_name();
        assert!(a.status.is_ok(), "{name}: {:?}", a.status);
        assert!(a.size < 1u64 << 32, "{name}");
        let h = a.header.as_ref().unwrap();
        assert_eq!(h.field_174, 112, "{name}");
        assert_eq!(h.field_178 == 0, h.family() == 2, "{name}");
        if let ArchiveKind::Cell(r) | ArchiveKind::Map(r) = a.kind {
            assert_eq!(h.sub_code, r, "{name}");
        }
        if a.aux_index > 0 {
            let base = report
                .archives
                .iter()
                .find(|b| b.kind == a.kind && b.aux_index == 0)
                .and_then(|b| b.header.as_ref())
                .unwrap();
            assert_eq!(
                (base.kind_word, base.sub_code, base.block_size),
                (h.kind_word, h.sub_code, h.block_size),
                "{name}"
            );
        }
    }
    assert!(report.unpaired_regions.is_empty());
    for s in &report.split_sets {
        assert_eq!(s.status, SplitSetStatus::Unverified);
    }

    assert_eq!(
        GameInstall::from_report(report).unwrap().languages,
        vec!["English".to_owned()]
    );
    let inst = GameInstall::open(&dir).unwrap();
    let fp = inst.schema_fingerprint();
    eprintln!(
        "archives: {}, languages: {:?}, split sets: {:?}",
        inst.archives.len(),
        inst.languages,
        inst.split_sets
            .iter()
            .filter(|s| s.members.len() > 1)
            .map(|s| (s.kind.key(), s.members.len()))
            .collect::<Vec<_>>()
    );
    eprintln!("project version: {:?}", inst.project_version);
    eprintln!("schema fingerprint (level 0): {}", fp.short());
    for (kind, n, p) in inst.usable_archives() {
        assert!(p.is_file(), "{kind} {n}");
    }
    assert_eq!(inst.usable_archives().len(), inst.archives.len());
    let env = SearchEnv::from_process();
    let s = inst.settings_dir(&SettingsQuery::default(), &env);
    eprintln!("settings dir: {} ({:?})", s.path.display(), s.source);

    // Named explicitly, the install must resolve the same settings folder as
    // the search does (§8.1 steps 4–5).
    let named = discover_with(&env, Some(&dir)).unwrap();
    let named = named.default_install().unwrap();
    let opened = GameInstall::open(&dir).unwrap();
    assert_eq!(named.prefix, opened.prefix);
    assert_eq!(named.steam, opened.steam);
    let s_named = named.settings_dir(&SettingsQuery::default(), &env);
    eprintln!(
        "explicit: prefix {:?}, steam {:?}, settings {} ({:?})",
        named.prefix,
        named.steam,
        s_named.path.display(),
        s_named.source
    );
}

/// Discovery through the Steam libraries under `$HOME`. Machine-specific, so
/// ignored by default. Run with:
/// `cargo test -p tessera-install --test real_data -- --ignored`
/// Optionally set `TESSERA_EXPECT_STEAM_LIBRARY` to the library that should
/// hold the game.
#[test]
#[ignore = "machine-specific: needs a Steam install of the game under $HOME"]
fn discovers_via_steam_under_home() {
    let mut env = SearchEnv::from_process();
    env.game_dir = None; // exercise the search, not the override
    let d = discover_with(&env, None).unwrap();
    for r in &d.rejected {
        eprintln!("rejected: {} {:?}", r.path.display(), r.problems);
    }
    let inst = d
        .installs
        .iter()
        .find(|i| i.sources.iter().any(|s| matches!(s, Source::Steam { .. })))
        .expect("no install found through Steam");
    eprintln!(
        "found {} via {:?}; prefix {:?}; steam {:?}",
        inst.root.display(),
        inst.sources,
        inst.prefix,
        inst.steam
    );
    if let Some(lib) = std::env::var_os("TESSERA_EXPECT_STEAM_LIBRARY") {
        let lib = std::fs::canonicalize(lib).unwrap();
        assert!(
            std::fs::canonicalize(&inst.root).unwrap().starts_with(&lib),
            "{} is not under {}",
            inst.root.display(),
            lib.display()
        );
    }
    let s = inst.settings_dir(&SettingsQuery::default(), &env);
    eprintln!(
        "settings dir: {} ({:?}, preferences present: {})",
        s.path.display(),
        s.source,
        s.has_preferences
    );
}
