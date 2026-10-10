// SPDX-License-Identifier: GPL-3.0-or-later
//! `--scenario` with the user's install (`docs/specs/scenario-feed.md` §10.1, §10.5; P0
//! exit criterion 2): each committed scenario runs headless twice against
//! `TESSERA_GAME_DIR` and both runs write the same log, byte for byte, with the selectors
//! resolved from the install. The logs stay in a temporary directory: they hold record
//! identifiers and may hold install text, so they are never committed and this test
//! asserts counts only. Skips (passing) without `TESSERA_GAME_DIR`. A debug build plays
//! one scenario (each run builds the record index, about 40 s in debug); a release build,
//! or `TESSERA_FULL=1`, plays all of them.

use std::path::{Path, PathBuf};
use std::process::Command;

fn install() -> Option<PathBuf> {
    match std::env::var_os("TESSERA_GAME_DIR") {
        Some(d) if !d.is_empty() => Some(PathBuf::from(d)),
        _ => {
            eprintln!("TESSERA_GAME_DIR not set; skipping");
            None
        }
    }
}

fn full() -> bool {
    !cfg!(debug_assertions) || std::env::var_os("TESSERA_FULL").is_some_and(|v| v == "1")
}

fn scenarios() -> Vec<PathBuf> {
    let dir = Path::new(env!("CARGO_MANIFEST_DIR")).join("tests/scenarios");
    let mut v: Vec<PathBuf> = std::fs::read_dir(dir)
        .expect("tests/scenarios")
        .map(|e| e.expect("entry").path())
        .filter(|p| p.extension().is_some_and(|e| e == "scenario"))
        .collect();
    v.sort();
    if !full() {
        v.retain(|p| p.file_stem().is_some_and(|s| s == "combat"));
    }
    v
}

/// Plays `scenario` headless against `install`; returns the log.
fn play(install: &Path, scenario: &Path, log: &Path) -> String {
    let out = Command::new(env!("CARGO_BIN_EXE_tessera-client"))
        .env("RUST_LOG", "warn")
        .args(["--headless", "--game-dir"])
        .arg(install)
        .arg("--scenario")
        .arg(scenario)
        .arg("--scenario-log")
        .arg(log)
        .output()
        .expect("runs the client");
    assert_eq!(
        out.status.code(),
        Some(0),
        "{}: {}",
        scenario.display(),
        String::from_utf8_lossy(&out.stderr)
    );
    std::fs::read_to_string(log).expect("log written")
}

/// The `got=` count of a `resolve` line.
fn got(line: &str) -> usize {
    line.split(' ')
        .find_map(|f| f.strip_prefix("got="))
        .and_then(|n| n.parse().ok())
        .unwrap_or(0)
}

#[test]
fn committed_scenarios_play_deterministically_with_the_install() {
    let Some(install) = install() else { return };
    let tmp = tempfile::tempdir().expect("tempdir");
    assert!(
        !tessera_client::runtime::is_inside(tmp.path(), &install),
        "logs must not go into the install"
    );
    let mut resolved_total = 0usize;
    for s in scenarios() {
        let name = s.file_stem().expect("stem").to_string_lossy().into_owned();
        let a = play(&install, &s, &tmp.path().join(format!("{name}.1.log")));
        let b = play(&install, &s, &tmp.path().join(format!("{name}.2.log")));
        assert!(
            a == b,
            "{name}: two runs with the install wrote different logs"
        );
        let lines: Vec<&str> = a.lines().collect();
        assert_eq!(lines.get(1), Some(&"install present"), "{name}");
        let resolves: Vec<&str> = lines
            .iter()
            .copied()
            .filter(|l| l.starts_with("resolve "))
            .collect();
        let selectors = std::fs::read_to_string(&s)
            .expect("scenario")
            .lines()
            .filter(|l| l.starts_with("select "))
            .count();
        assert_eq!(
            resolves.len(),
            selectors,
            "{name}: one resolve line per selector"
        );
        let got_total: usize = resolves.iter().map(|l| got(l)).sum();
        resolved_total += got_total;
        let end = lines.last().copied().unwrap_or_default();
        assert!(end.contains(" end reason=complete "), "{name}: {end}");
        eprintln!(
            "{name}: {} log lines, {} selectors, {got_total} records resolved",
            lines.len(),
            selectors
        );
    }
    assert!(
        resolved_total > 0,
        "the install resolved no selector of the committed scenarios"
    );
}

/// Runs the client with `args` against `install`; returns (exit code, stdout, stderr).
fn client_run(install: &Path, args: &[&str]) -> (Option<i32>, String, String) {
    let out = Command::new(env!("CARGO_BIN_EXE_tessera-client"))
        .env("RUST_LOG", "warn")
        .arg("--game-dir")
        .arg(install)
        .args(args)
        .output()
        .expect("runs the client");
    (
        out.status.code(),
        String::from_utf8_lossy(&out.stdout).into_owned(),
        String::from_utf8_lossy(&out.stderr).into_owned(),
    )
}

/// The `at=` height of the log's `WorldEntered` line.
fn entry_height(log: &str) -> Option<f64> {
    let line = log.lines().find(|l| l.contains(" WorldEntered "))?;
    let at = line.split(' ').find_map(|f| f.strip_prefix("at="))?;
    at.rsplit(',').next()?.parse().ok()
}

/// Whether a walker run failed only for want of a GPU (the test then skips).
fn no_gpu(code: Option<i32>, err: &str) -> bool {
    if code == Some(1) && err.contains("headless renderer") {
        eprintln!("no GPU adapter; skipping: {err}");
        return true;
    }
    false
}

#[test]
fn the_world_scenario_places_its_signpost_with_a_region_1_spawn() {
    // `world.scenario`'s signpost stands at a region-1 place: with an exact spawn in
    // region 1 it spawns (with the default region-0 spawn it is a skip, §5.4).
    let Some(install) = install() else { return };
    let s = Path::new(env!("CARGO_MANIFEST_DIR")).join("tests/scenarios/world.scenario");
    let (code, log, err) = client_run(
        &install,
        &[
            "--headless",
            "--scenario",
            s.to_str().expect("utf-8"),
            "--scenario-log",
            "-",
            "--spawn",
            "1,126,140,10,20,30",
        ],
    );
    assert_eq!(code, Some(0), "{err}");
    assert!(
        log.lines()
            .any(|l| l.contains(" EntitySpawned entity=$signpost ")
                || l.contains(" EntityCreated entity=$signpost ")),
        "the signpost spawned:\n{log}"
    );
}

const TINY: &[u8] = b"tessera-scenario 1\ntick 100ms\nplayer \"Testa\"\n\
actor $post kind=object name=\"Post\" pos=3,0,0\nat 0s spawn $post\n+0.5s system \"done\"\n";

#[test]
fn a_ground_spawn_is_snapped_without_a_screenshot() {
    // `--spawn R,X,Y,x,y` (no height) in a headless run with an install: the walker's world
    // source answers the ground lookup (§3.2, §12.2 step 3), so the entry is not at z = 0.
    let Some(install) = install() else { return };
    if !full() {
        eprintln!("debug build without TESSERA_FULL=1; skipping the walker run");
        return;
    }
    let tmp = tempfile::tempdir().expect("tempdir");
    let s = tmp.path().join("tiny.scenario");
    std::fs::write(&s, TINY).expect("scenario");
    let (code, log, err) = client_run(
        &install,
        &[
            "--headless",
            "--scenario",
            s.to_str().expect("utf-8"),
            "--scenario-log",
            "-",
            "--spawn",
            "1,126,140,10,20",
        ],
    );
    if no_gpu(code, &err) {
        return;
    }
    assert_eq!(code, Some(0), "{err}");
    let z = entry_height(&log).expect("a WorldEntered line");
    assert!(z != 0.0, "ground-snapped entry, got z = {z}:\n{log}");
    assert!(!log.contains("Cannot spawn there"), "{log}");
}

#[test]
fn a_scenario_screenshot_writes_the_driver_only_log() {
    // `--scenario --screenshot` with an exact spawn: exit 0, the PNG exists, and the log is
    // the one of the same run without the walker except for the client avatar's
    // `AppearanceChanged` (§12.4 OPEN (3)).
    let Some(install) = install() else { return };
    if !full() {
        eprintln!("debug build without TESSERA_FULL=1; skipping the walker run");
        return;
    }
    let tmp = tempfile::tempdir().expect("tempdir");
    let s = tmp.path().join("tiny.scenario");
    std::fs::write(&s, TINY).expect("scenario");
    let png = tmp.path().join("shot.png");
    let common = [
        "--scenario",
        s.to_str().expect("utf-8"),
        "--scenario-log",
        "-",
        "--spawn",
        "1,126,140,10,20,436",
        "--heading",
        "0",
    ];
    let mut shot_args = vec![
        "--screenshot",
        png.to_str().expect("utf-8"),
        "--size",
        "320x240",
    ];
    shot_args.extend_from_slice(&common);
    let (code, shot_log, err) = client_run(&install, &shot_args);
    if no_gpu(code, &err) {
        return;
    }
    assert_eq!(code, Some(0), "{err}");
    assert!(png.is_file(), "the screenshot was written");
    let mut plain_args = vec!["--headless"];
    plain_args.extend_from_slice(&common);
    let (code, plain_log, err) = client_run(&install, &plain_args);
    assert_eq!(code, Some(0), "{err}");
    let strip = |log: &str| -> Vec<String> {
        log.lines()
            .filter(|l| !l.contains(" AppearanceChanged "))
            .map(str::to_owned)
            .collect()
    };
    assert_eq!(strip(&shot_log), strip(&plain_log));
}
