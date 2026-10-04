// SPDX-License-Identifier: GPL-3.0-or-later
//! Discovery, validation and settings tests against synthetic layouts in
//! temporary directories.

mod common;

use std::fs;
use std::path::{Path, PathBuf};

use common::{FULL_SET, app_manifest, library_folders_vdf, make_install, write_archive};
use tessera_install::{
    ArchiveKind, ArchiveStatus, Error, GameInstall, GameType, Ini, ProjectVersion, SearchEnv,
    SettingsQuery, SettingsSource, Source, SplitSetStatus, ValidationProblem, ValidationWarning,
    discover_with, validate,
};

fn canon(p: &Path) -> PathBuf {
    fs::canonicalize(p).unwrap()
}

/// Build: ~/.local/share/Steam (default library, no game) and a second
/// library elsewhere holding the game with a Proton prefix.
fn steam_layout(tmp: &Path) -> (PathBuf, PathBuf, PathBuf) {
    let home = tmp.join("home");
    let root = home.join(".local/share/Steam");
    let lib2 = tmp.join("nvme/SteamLibrary");
    fs::create_dir_all(root.join("steamapps")).unwrap();
    fs::create_dir_all(root.join("config")).unwrap();
    let vdf = library_folders_vdf(&[(&root, false), (&lib2, true)]);
    fs::write(root.join("steamapps/libraryfolders.vdf"), &vdf).unwrap();
    fs::write(root.join("config/libraryfolders.vdf"), &vdf).unwrap();
    // ~/.steam/steam -> ~/.local/share/Steam, as on a typical Linux machine.
    fs::create_dir_all(home.join(".steam")).unwrap();
    #[cfg(unix)]
    std::os::unix::fs::symlink(&root, home.join(".steam/steam")).unwrap();

    let steamapps = lib2.join("steamapps");
    fs::create_dir_all(&steamapps).unwrap();
    fs::write(
        steamapps.join("appmanifest_212500.acf"),
        app_manifest("Game Folder", 777),
    )
    .unwrap();
    let game = steamapps.join("common/Game Folder");
    make_install(&game, FULL_SET);

    let compat = steamapps.join("compatdata/212500");
    let docs = compat.join("pfx/drive_c/users/steamuser/Documents/The Lord of the Rings Online");
    fs::create_dir_all(&docs).unwrap();
    fs::create_dir_all(compat.join("pfx/drive_c/users/Public")).unwrap();
    fs::write(compat.join("version"), "GE-Proton-test\n").unwrap();
    fs::write(
        docs.join("UserPreferences.ini"),
        "\u{feff}[Render]\r\nGraphicsCore=D3D11\r\n[Display]\r\nResolution=1920x1080\r\n",
    )
    .unwrap();
    (home, lib2, game)
}

#[test]
fn finds_install_through_second_steam_library() {
    let t = tempfile::tempdir().unwrap();
    let (home, lib2, game) = steam_layout(t.path());
    let env = SearchEnv::isolated(&home);
    let d = discover_with(&env, None).unwrap();
    assert_eq!(d.installs.len(), 1, "rejected: {:?}", d.rejected);
    let inst = d.default_install().unwrap();
    assert_eq!(canon(&inst.root), canon(&game));
    assert_eq!(
        inst.sources[0],
        Source::Steam {
            library: lib2.clone()
        }
    );
    // The home scan of steamapps/common names the same install; it merges.
    assert!(inst.sources.contains(&Source::HomeScan));
    let steam = inst.steam.as_ref().unwrap();
    assert_eq!(steam.buildid, Some(777));
    assert_eq!(steam.proton_version.as_deref(), Some("GE-Proton-test"));
    assert_eq!(
        canon(inst.prefix.as_ref().unwrap()),
        canon(&lib2.join("steamapps/compatdata/212500/pfx"))
    );
    assert_eq!(inst.game_type, GameType::Compatible);
    assert!(!inst.incomplete, "{:?}", inst.warnings);
    assert_eq!(inst.languages, vec!["English".to_owned()]);
    assert_eq!(inst.client_dir.as_deref(), Some(game.join("x64").as_path()));

    // Settings folder: inside the Proton prefix, user steamuser.
    let s = inst.settings_dir(&SettingsQuery::default(), &env);
    assert!(s.has_preferences);
    assert!(matches!(&s.source, SettingsSource::Prefix { user, .. } if user == "steamuser"));
    let ini = Ini::read(&s.preferences).unwrap();
    assert_eq!(ini.get("render", "graphicscore"), Some("D3D11"));
    assert_eq!(
        ini.get_resolution("Display", "Resolution"),
        Some((1920, 1080))
    );
}

#[test]
fn manifest_missing_but_library_lists_app() {
    let t = tempfile::tempdir().unwrap();
    let home = t.path().join("home");
    let root = home.join(".local/share/Steam");
    let lib = t.path().join("lib");
    fs::create_dir_all(root.join("steamapps")).unwrap();
    fs::write(
        root.join("steamapps/libraryfolders.vdf"),
        library_folders_vdf(&[(&lib, true)]),
    )
    .unwrap();
    let game = lib.join("steamapps/common/Lord of the Rings Online");
    make_install(&game, FULL_SET);
    let d = discover_with(&SearchEnv::isolated(&home), None).unwrap();
    let inst = d.default_install().unwrap();
    assert_eq!(canon(&inst.root), canon(&game));
    assert_eq!(inst.steam.as_ref().unwrap().buildid, None);
}

#[test]
fn malformed_vdf_is_not_fatal() {
    let t = tempfile::tempdir().unwrap();
    let home = t.path().join("home");
    let root = home.join(".local/share/Steam/steamapps");
    fs::create_dir_all(&root).unwrap();
    fs::write(
        root.join("libraryfolders.vdf"),
        "\"libraryfolders\" { \"0\" {",
    )
    .unwrap();
    fs::write(root.join("appmanifest_212500.acf"), "}}}").unwrap();
    let d = discover_with(&SearchEnv::isolated(&home), None).unwrap();
    assert!(d.installs.is_empty());
}

#[test]
fn explicit_and_env_dirs() {
    let t = tempfile::tempdir().unwrap();
    let good = t.path().join("good");
    make_install(&good, FULL_SET);
    let bad = t.path().join("bad");
    make_install(&bad, &["client_general.dat"]);

    let env = SearchEnv::isolated(t.path().join("nohome"));
    let d = discover_with(&env, Some(&good)).unwrap();
    assert_eq!(d.installs[0].sources, vec![Source::Explicit]);

    match discover_with(&env, Some(&bad)) {
        Err(Error::NotUsable { problems, .. }) => {
            assert!(
                problems.contains(&ValidationProblem::MissingCoreArchive(
                    ArchiveKind::Gamelogic
                )),
                "{problems:?}"
            );
            assert!(problems.iter().any(|p| p.to_string().contains("gamelogic")));
        }
        other => panic!("expected NotUsable, got {other:?}"),
    }

    let mut env2 = env.clone();
    env2.game_dir = Some(good.clone());
    let d = discover_with(&env2, None).unwrap();
    assert_eq!(d.installs[0].sources, vec![Source::Environment]);
    env2.game_dir = Some(t.path().join("missing"));
    assert!(matches!(
        discover_with(&env2, None),
        Err(Error::NotUsable { .. })
    ));
}

#[test]
fn wine_prefixes_and_home_scan() {
    let t = tempfile::tempdir().unwrap();
    let home = t.path().join("home");
    // Standalone default path inside ~/.wine.
    let standalone = home
        .join(".wine/drive_c/Program Files (x86)/StandingStoneGames/The Lord of the Rings Online");
    make_install(&standalone, FULL_SET);
    // A Lutris-style prefix with a custom folder, found by the prefix walk.
    let lutris = home.join("Games/some-game/drive_c/Program Files/Vendor/Game");
    make_install(&lutris, FULL_SET);
    // A plain copy found only by the home scan.
    let copy = home.join("games/copy/data");
    make_install(&copy, FULL_SET);
    // A backup folder must never be treated as an install.
    make_install(
        &home.join("Games/some-game/drive_c/Program Files/Vendor/backup/old"),
        FULL_SET,
    );

    let d = discover_with(&SearchEnv::isolated(&home), None).unwrap();
    let roots: Vec<PathBuf> = d.installs.iter().map(|i| canon(&i.root)).collect();
    assert_eq!(
        roots,
        vec![canon(&standalone), canon(&lutris), canon(&copy)],
        "{:?}",
        d.rejected
    );
    assert_eq!(d.installs[0].sources[0], Source::Standalone);
    assert!(matches!(
        d.installs[1].sources[0],
        Source::WinePrefix { .. }
    ));
    assert_eq!(d.installs[2].sources, vec![Source::HomeScan]);
    assert!(d.installs[0].prefix.is_some());
    assert!(d.installs[2].prefix.is_none());
}

#[test]
fn launcher_config_with_settings_override() {
    let t = tempfile::tempdir().unwrap();
    let home = t.path().join("home");
    let game = t.path().join("elsewhere/game");
    make_install(&game, FULL_SET);
    let settings = t.path().join("my-settings");
    fs::create_dir_all(&settings).unwrap();
    let cfg_dir = home.join(".local/share/onelauncher/games/abc");
    fs::create_dir_all(&cfg_dir).unwrap();
    fs::write(
        cfg_dir.join("config.toml"),
        format!(
            "game_directory = \"{}\"\ngame_settings_directory = \"{}\"\ngame_type = \"LOTRO\"\n",
            game.display(),
            settings.display()
        ),
    )
    .unwrap();
    let env = SearchEnv::isolated(&home);
    let d = discover_with(&env, None).unwrap();
    let inst = d.default_install().unwrap();
    assert!(matches!(inst.sources[0], Source::LauncherConfig { .. }));
    let s = inst.settings_dir(&SettingsQuery::default(), &env);
    assert_eq!(s.source, SettingsSource::InstallOverride);
    assert_eq!(s.path, settings);
}

#[test]
fn unsupported_game_is_rejected() {
    let t = tempfile::tempdir().unwrap();
    let home = t.path().join("home");
    let other = home.join("Games/other");
    make_install(&other, FULL_SET);
    fs::remove_file(other.join("lotro.launcherconfig")).unwrap();
    fs::remove_dir_all(other.join("x64")).unwrap();
    fs::write(other.join("ddo.launcherconfig"), "<configuration/>").unwrap();
    let d = discover_with(&SearchEnv::isolated(&home), None).unwrap();
    assert!(d.installs.is_empty());
    assert_eq!(d.rejected.len(), 1);
    assert!(matches!(
        d.rejected[0].problems.as_slice(),
        [ValidationProblem::UnsupportedGame(_)]
    ));
    assert!(matches!(
        GameInstall::open(&other),
        Err(Error::UnsupportedGame { .. })
    ));
}

/// §3.7 step 2/3: the sibling game identified only by a legacy launcher
/// config name and an x64 client binary is not opened.
#[test]
fn unsupported_game_by_config_value_and_x64_binary() {
    let t = tempfile::tempdir().unwrap();
    let dir = t.path().join("g");
    make_install(&dir, FULL_SET);
    fs::remove_file(dir.join("lotro.launcherconfig")).unwrap();
    fs::remove_file(dir.join("x64/lotroclient64.exe")).unwrap();
    fs::write(
        dir.join("TurbineLauncher.exe.config"),
        "<configuration><appSettings>\
         <add key=\"DataCenter.GameName\" value=\"DDO\"/></appSettings></configuration>",
    )
    .unwrap();
    let r = validate(&dir);
    assert!(!r.usable);
    assert!(matches!(r.game_type, GameType::Unsupported(_)));
    // Without the config, the x64 binary alone decides.
    fs::remove_file(dir.join("TurbineLauncher.exe.config")).unwrap();
    fs::write(dir.join("x64/dndclient64.exe"), b"MZ").unwrap();
    assert!(matches!(
        GameInstall::open(&dir),
        Err(Error::UnsupportedGame { .. })
    ));
}

/// §8.1 steps 4–5: a Steam/Proton install named explicitly resolves the
/// same settings folder as when it is found by searching.
#[test]
fn explicit_steam_dir_uses_proton_prefix() {
    let t = tempfile::tempdir().unwrap();
    let (home, lib2, game) = steam_layout(t.path());
    // A competing ~/NAME folder with preferences, as on the reporter's machine.
    let direct = home.join("The Lord of the Rings Online");
    fs::create_dir_all(&direct).unwrap();
    fs::write(direct.join("UserPreferences.ini"), "[A]\nb=c\n").unwrap();
    let env = SearchEnv::isolated(&home);

    let searched = discover_with(&env, None).unwrap();
    let searched = searched.default_install().unwrap();
    let s_searched = searched.settings_dir(&SettingsQuery::default(), &env);

    for explicit in [
        discover_with(&env, Some(&game)).unwrap(),
        discover_with(
            &SearchEnv {
                game_dir: Some(game.clone()),
                ..env.clone()
            },
            None,
        )
        .unwrap(),
    ] {
        let inst = explicit.default_install().unwrap();
        assert_eq!(
            canon(inst.prefix.as_ref().unwrap()),
            canon(&lib2.join("steamapps/compatdata/212500/pfx"))
        );
        let steam = inst.steam.as_ref().unwrap();
        assert_eq!(steam.buildid, Some(777));
        assert_eq!(steam.proton_version.as_deref(), Some("GE-Proton-test"));
        let s = inst.settings_dir(&SettingsQuery::default(), &env);
        assert!(
            matches!(&s.source, SettingsSource::Prefix { user, .. } if user == "steamuser"),
            "{s:?}"
        );
        assert_eq!(s.path, s_searched.path);
    }
}

/// A directory inside a Wine prefix (an ancestor is `drive_c`) gets that
/// prefix when named explicitly or found by the home scan.
#[test]
fn explicit_dir_inside_wine_prefix() {
    let t = tempfile::tempdir().unwrap();
    let pfx = t.path().join("myprefix");
    let game = pfx.join("drive_c/Games/Custom/Game");
    make_install(&game, FULL_SET);
    let docs = pfx.join("drive_c/users/someone/Documents/The Lord of the Rings Online");
    fs::create_dir_all(&docs).unwrap();
    fs::write(docs.join("UserPreferences.ini"), "[A]\nb=c\n").unwrap();
    let env = SearchEnv::isolated(t.path().join("home"));
    let d = discover_with(&env, Some(&game)).unwrap();
    let inst = d.default_install().unwrap();
    assert_eq!(canon(inst.prefix.as_ref().unwrap()), canon(&pfx));
    assert!(inst.steam.is_none());
    let s = inst.settings_dir(&SettingsQuery::default(), &env);
    assert!(matches!(s.source, SettingsSource::Prefix { .. }), "{s:?}");
    assert_eq!(s.path, docs);
    assert_eq!(GameInstall::open(&game).unwrap().prefix, inst.prefix);

    // A folder under a Steam library's common/ that is not the manifest's
    // installdir is not treated as the Steam app.
    let (home, _lib2, game2) = steam_layout(&t.path().join("s"));
    let other = game2.parent().unwrap().join("Other Copy");
    make_install(&other, FULL_SET);
    let d = discover_with(&SearchEnv::isolated(&home), Some(&other)).unwrap();
    assert!(d.installs[0].steam.is_none());
    assert!(d.installs[0].prefix.is_none());
}

/// A broken language archive is never offered, even as the default.
#[test]
fn broken_language_archive_is_not_selected() {
    let t = tempfile::tempdir().unwrap();
    let dir = t.path().join("g");
    make_install(
        &dir,
        &[
            "client_general.dat",
            "client_gamelogic.dat",
            "client_local_English.dat",
            "client_local_FR.dat",
        ],
    );
    let mut b = fs::read(dir.join("client_local_English.dat")).unwrap();
    b[0x140..0x144].copy_from_slice(&0xDEADu32.to_le_bytes());
    fs::write(dir.join("client_local_English.dat"), &b).unwrap();
    let r = validate(&dir);
    assert!(r.usable, "{:?}", r.problems);
    assert_eq!(r.languages, vec!["FR".to_owned()]);
    assert!(r.warnings.iter().any(|w| matches!(
        w,
        ValidationWarning::UnusableLanguage { language, .. } if language == "English"
    )));
    let inst = GameInstall::from_report(r).unwrap();
    assert_eq!(inst.select_language(None), Some("FR"));
    assert_eq!(inst.select_language(Some("English")), Some("FR"));
    assert!(
        inst.set_member_paths(&ArchiveKind::Local("English".into()))
            .is_empty()
    );
}

/// The resolved archive list handed to the data layer: actual on-disk names
/// (any case), consecutive members only, broken members cut off.
#[test]
fn usable_archive_paths() {
    let t = tempfile::tempdir().unwrap();
    let dir = t.path().join("g");
    make_install(&dir, &[]);
    for a in [
        "CLIENT_GENERAL.DAT",
        "Client_Gamelogic.dat",
        "client_local_English.dat",
        "client_surface.dat",
        "client_surface_aux_1.datx",
        "client_surface_aux_2.datx",
        "client_surface_aux_4.datx",
        "client_mesh.dat",
        "client_mesh_aux_1.datx",
        "client_mesh_aux_2.datx",
    ] {
        write_archive(&dir, a, 2048);
    }
    let mut m = fs::read(dir.join("client_mesh_aux_1.datx")).unwrap();
    m[0x140] = 0;
    fs::write(dir.join("client_mesh_aux_1.datx"), &m).unwrap();
    let inst = GameInstall::open(&dir).unwrap();
    let general = inst.set_member_paths(&ArchiveKind::General);
    assert_eq!(general, vec![(0, dir.join("CLIENT_GENERAL.DAT").as_path())]);
    let surface: Vec<u32> = inst
        .set_member_paths(&ArchiveKind::Surface)
        .iter()
        .map(|(n, _)| *n)
        .collect();
    assert_eq!(surface, vec![0, 1, 2]);
    let mesh: Vec<u32> = inst
        .set_member_paths(&ArchiveKind::Mesh)
        .iter()
        .map(|(n, _)| *n)
        .collect();
    assert_eq!(mesh, vec![0], "a broken member ends the set");
    let all = inst.usable_archives();
    assert_eq!(all.len(), 1 + 1 + 1 + 3 + 1);
    assert!(all.iter().all(|(_, _, p)| p.is_file()));
}

/// FIFOs planted where metadata files are expected must not hang discovery.
#[cfg(unix)]
#[test]
fn fifos_do_not_hang_discovery() {
    fn mkfifo(p: &Path) -> bool {
        fs::create_dir_all(p.parent().unwrap()).unwrap();
        std::process::Command::new("mkfifo")
            .arg(p)
            .status()
            .is_ok_and(|s| s.success())
    }
    let t = tempfile::tempdir().unwrap();
    let (home, lib2, _game) = steam_layout(t.path());
    // compatdata version, OneLauncher config, home-scan ProjectVersion.
    let version = lib2.join("steamapps/compatdata/212500/version");
    fs::remove_file(&version).unwrap();
    let copy = home.join("Games/g");
    make_install(&copy, FULL_SET);
    fs::remove_file(copy.join("ProjectVersion")).unwrap();
    let ok = mkfifo(&version)
        && mkfifo(&home.join(".local/share/onelauncher/games/x/config.toml"))
        && mkfifo(&copy.join("ProjectVersion"));
    if !ok {
        eprintln!("mkfifo unavailable; skipping");
        return;
    }
    let env = SearchEnv::isolated(&home);
    let (tx, rx) = std::sync::mpsc::channel();
    std::thread::spawn(move || {
        let _ = tx.send(discover_with(&env, None).map(|d| d.installs));
    });
    let installs = rx
        .recv_timeout(std::time::Duration::from_secs(30))
        .expect("discovery blocked on a FIFO")
        .unwrap();
    assert_eq!(installs.len(), 2);
    assert!(installs[0].steam.as_ref().unwrap().proton_version.is_none());
    let scanned = installs
        .iter()
        .find(|i| canon(&i.root) == canon(&copy))
        .unwrap();
    assert_eq!(scanned.project_version, ProjectVersion::Malformed);
}

/// A huge library list is de-duplicated in linear time.
#[test]
fn many_steam_libraries_are_handled() {
    let t = tempfile::tempdir().unwrap();
    let home = t.path().join("home");
    let root = home.join(".local/share/Steam/steamapps");
    fs::create_dir_all(&root).unwrap();
    let mut vdf = String::from("\"libraryfolders\"\n{\n");
    for i in 0..40_000 {
        vdf.push_str(&format!("\"{i}\" \"/nx-t/{}\"\n", i % 20_000));
    }
    vdf.push_str("}\n");
    assert!(vdf.len() < tessera_install::vdf::MAX_VDF_SIZE);
    fs::write(root.join("libraryfolders.vdf"), vdf).unwrap();
    let start = std::time::Instant::now();
    let libs = tessera_install::steam::all_libraries(&SearchEnv::isolated(&home));
    // 20,000 distinct paths plus the root itself.
    assert_eq!(libs.len(), 20_001);
    let d = discover_with(&SearchEnv::isolated(&home), None).unwrap();
    assert!(d.installs.is_empty());
    eprintln!("40k-entry library list: {:?}", start.elapsed());
}

#[test]
fn validation_report_details() {
    let t = tempfile::tempdir().unwrap();
    let dir = t.path().join("g");
    make_install(
        &dir,
        &[
            "client_general.dat",
            "client_gamelogic.dat",
            "client_local_English.dat",
            "client_local_DE.dat",
            "client_surface.dat",
            "client_surface_aux_1.datx",
            "client_surface_aux_3.datx",
            "client_cell_2.dat",
            "client_unknownthing.txt",
        ],
    );
    let r = validate(&dir);
    assert!(r.usable && r.core_ok, "{:?}", r.problems);
    assert_eq!(r.languages, vec!["DE".to_owned(), "English".to_owned()]);
    assert!(r.incomplete());
    assert_eq!(r.missing_content, vec!["mesh", "sound", "anim"]);
    assert_eq!(r.unpaired_regions, vec!["cell_2"]);
    let surface = r
        .split_sets
        .iter()
        .find(|s| s.kind == ArchiveKind::Surface)
        .unwrap();
    assert_eq!(surface.members, vec![0, 1]);
    assert_eq!(surface.status, SplitSetStatus::Unverified);
    assert!(r.warnings.contains(&ValidationWarning::AuxNotConsecutive {
        file: "client_surface_aux_3.datx".into()
    }));

    let inst = GameInstall::from_report(r).unwrap();
    assert_eq!(inst.select_language(None), Some("English"));
    assert_eq!(inst.select_language(Some("de")), Some("DE"));
    assert_eq!(inst.select_language(Some("FR")), Some("English"));
    let fp = inst.schema_fingerprint();
    assert_eq!(fp.entries.len(), 8);
    assert_eq!(fp, GameInstall::open(&dir).unwrap().schema_fingerprint());
}

#[test]
fn header_problems_are_reported() {
    let t = tempfile::tempdir().unwrap();
    let dir = t.path().join("g");
    make_install(&dir, FULL_SET);
    // Wrong length in the header of gamelogic: append bytes after writing.
    let mut b = fs::read(dir.join("client_gamelogic.dat")).unwrap();
    b.extend([0u8; 16]);
    fs::write(dir.join("client_gamelogic.dat"), &b).unwrap();
    // Bad magic in mesh (content tier: only a warning).
    let mut m = fs::read(dir.join("client_mesh.dat")).unwrap();
    m[0x140] = 0;
    fs::write(dir.join("client_mesh.dat"), &m).unwrap();
    // A core archive smaller than 1024 bytes.
    let r = validate(&dir);
    assert!(!r.usable);
    let gl = r
        .archives
        .iter()
        .find(|a| a.kind == ArchiveKind::Gamelogic)
        .unwrap();
    assert!(matches!(gl.status, ArchiveStatus::SizeMismatch { .. }));
    let mesh = r
        .archives
        .iter()
        .find(|a| a.kind == ArchiveKind::Mesh)
        .unwrap();
    assert!(matches!(mesh.status, ArchiveStatus::BadMagic(_)));

    let small = t.path().join("small");
    make_install(&small, &[]);
    for a in [
        "client_general.dat",
        "client_gamelogic.dat",
        "client_local_English.dat",
    ] {
        write_archive(&small, a, 0x1A8);
    }
    let r = validate(&small);
    assert!(!r.core_ok);
    assert!(r.problems.iter().any(|p| matches!(
        p,
        ValidationProblem::CoreArchiveFailed {
            kind: ArchiveKind::General,
            ..
        }
    )));
}

#[test]
fn case_insensitive_archive_names() {
    let t = tempfile::tempdir().unwrap();
    let dir = t.path().join("g");
    make_install(&dir, &[]);
    for a in [
        "CLIENT_GENERAL.DAT",
        "Client_GameLogic.dat",
        "client_LOCAL_English.DAT",
    ] {
        write_archive(&dir, a, 2048);
    }
    let r = validate(&dir);
    assert!(r.usable, "{:?}", r.problems);
    assert_eq!(r.languages, vec!["English".to_owned()]);
}

#[test]
fn settings_fallbacks_and_prefs_argument() {
    let t = tempfile::tempdir().unwrap();
    let home = t.path().join("home");
    let game = t.path().join("game");
    make_install(&game, FULL_SET);
    let env = SearchEnv::isolated(&home);
    let inst = GameInstall::open(&game).unwrap();

    // Nothing exists yet: the first home candidate is proposed.
    let s = inst.settings_dir(&SettingsQuery::default(), &env);
    assert!(!s.exists);
    assert_eq!(s.source, SettingsSource::Home);
    assert_eq!(s.path, home.join("Documents/The Lord of the Rings Online"));

    // ~/NAME with preferences wins over an empty ~/Documents/NAME.
    fs::create_dir_all(home.join("Documents/The Lord of the Rings Online")).unwrap();
    let direct = home.join("The Lord of the Rings Online");
    fs::create_dir_all(&direct).unwrap();
    fs::write(direct.join("UserPreferences.ini"), "[A]\nb=c\n").unwrap();
    let s = inst.settings_dir(&SettingsQuery::default(), &env);
    assert_eq!(s.path, direct);
    assert!(s.has_preferences);

    // --prefs always wins, and the file need not be called UserPreferences.ini.
    let prefs = t.path().join("custom/prefs.ini");
    let q = SettingsQuery {
        prefs_file: Some(&prefs),
        document_folder: None,
    };
    let s = inst.settings_dir(&q, &env);
    assert_eq!(s.source, SettingsSource::PrefsArgument);
    assert_eq!(s.path, t.path().join("custom"));
    assert_eq!(s.preferences, prefs);
}
