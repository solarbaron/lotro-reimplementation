// SPDX-License-Identifier: GPL-3.0-or-later
//! `--scenario` without game data (`docs/specs/scenario-feed.md` §10.5, §12, §13; P0 exit
//! criterion 2): every committed synthetic scenario under `tests/scenarios/` plays headless
//! with no install, twice, and both runs write the committed log byte for byte; the
//! committed scenarios follow the content rules of §13; the command-line errors exit with
//! the codes of §12.2. Runs the real binary with `TESSERA_GAME_DIR` unset and `--no-data`
//! (the CI replay of §10.5).

use std::path::{Path, PathBuf};
use std::process::{Command, Output};

use clap::Parser as _;
use tessera_net::scenario::Scenario;

/// The system groups of `scenario-feed.md` §6.1–§6.6, each with its committed scenario.
const GROUPS: [(&str, &str); 6] = [
    ("§6.1 characters and inventory", "inventory"),
    ("§6.2 targeting, skills and combat", "combat"),
    ("§6.3 NPC windows", "npc-windows"),
    ("§6.4 social", "social"),
    ("§6.5 map, travel, mounts and instances", "world"),
    ("§6.6 progression and crafting", "progression"),
];

/// The NPC-window and quest-journal scenarios of decision 0024 P3 (`npc-windows.md`,
/// `quest-journal.md`; ROADMAP P3 deliverables), each with its committed log.
const P3_SCENARIOS: [&str; 4] = ["dialogue", "vendor", "loot", "quest-log"];

fn scenarios_dir() -> PathBuf {
    Path::new(env!("CARGO_MANIFEST_DIR")).join("tests/scenarios")
}

/// Every committed scenario, sorted by name.
fn committed() -> Vec<PathBuf> {
    let mut v: Vec<PathBuf> = std::fs::read_dir(scenarios_dir())
        .expect("tests/scenarios")
        .map(|e| e.expect("entry").path())
        .filter(|p| p.extension().is_some_and(|e| e == "scenario"))
        .collect();
    v.sort();
    v
}

/// The client binary with no install in reach of the run.
fn client() -> Command {
    let mut c = Command::new(env!("CARGO_BIN_EXE_tessera-client"));
    c.env_remove("TESSERA_GAME_DIR")
        .env("RUST_LOG", "warn")
        .env_remove("TESSERA_PASSWORD");
    c
}

fn run(args: &[&str]) -> Output {
    client().args(args).output().expect("runs the client")
}

/// Plays `scenario` headless without data and returns the log.
fn play(scenario: &Path, log: &Path) -> Vec<u8> {
    let out = run(&[
        "--headless",
        "--no-data",
        "--scenario",
        scenario.to_str().expect("utf-8"),
        "--scenario-log",
        log.to_str().expect("utf-8"),
    ]);
    assert_eq!(
        out.status.code(),
        Some(0),
        "{}: {}",
        scenario.display(),
        String::from_utf8_lossy(&out.stderr)
    );
    std::fs::read(log).expect("log written")
}

fn text(b: &[u8]) -> String {
    String::from_utf8_lossy(b).into_owned()
}

#[test]
fn every_system_group_has_a_committed_scenario_with_a_log() {
    for (group, name) in GROUPS {
        let s = scenarios_dir().join(format!("{name}.scenario"));
        assert!(s.is_file(), "{group}: {} missing", s.display());
    }
    for name in P3_SCENARIOS {
        let s = scenarios_dir().join(format!("{name}.scenario"));
        assert!(s.is_file(), "P3: {} missing", s.display());
    }
    for s in committed() {
        // All committed scenarios are synthetic (they play without an install), so each
        // has its expected log (§10.5).
        let log = s.with_extension("log");
        assert!(log.is_file(), "{} has no committed log", s.display());
        let body = std::fs::read_to_string(&log).expect("log");
        assert!(
            body.starts_with("tessera-scenario-log 1\ninstall none\n"),
            "{}: only logs of runs without an install may be committed (§10.5)",
            log.display()
        );
    }
}

#[test]
fn committed_scenarios_follow_the_content_rules() {
    let list = committed();
    assert!(list.len() >= GROUPS.len());
    for path in list {
        let bytes = std::fs::read(&path).expect("read");
        let sc = Scenario::parse(&bytes)
            .unwrap_or_else(|e| panic!("{}", e.with_file(&path.display().to_string())));
        // §13 rule 2: selectors only, no record identifiers, enum values as labels.
        assert!(!sc.uses_did_family(), "{}: `did` family", path.display());
        assert!(
            !sc.numeric_enum_values(),
            "{}: numeric enum value",
            path.display()
        );
        assert!(
            !sc.requires_install(),
            "{}: committed scenarios here are synthetic",
            path.display()
        );
        let text = std::str::from_utf8(&bytes).expect("utf-8");
        for (n, line) in text.lines().enumerate() {
            let code = line.split('#').next().unwrap_or("");
            assert!(
                !code.contains("0x") && !code.contains("0X"),
                "{}:{}: a hexadecimal number (a record identifier?)",
                path.display(),
                n + 1
            );
        }
        // §13 rule 1: GPL-3.0-or-later or CC0-1.0, stated by the SPDX header comment.
        assert!(
            text.lines().next().is_some_and(|l| {
                l.starts_with('#')
                    && ["GPL-3.0-or-later", "CC0-1.0"]
                        .iter()
                        .any(|id| l.trim_end() == format!("# SPDX-License-Identifier: {id}"))
            }),
            "{}: SPDX header (GPL-3.0-or-later or CC0-1.0)",
            path.display()
        );
    }
}

#[test]
fn committed_scenarios_replay_their_logs_byte_for_byte() {
    let dir = tempfile::tempdir().expect("tempdir");
    for s in committed() {
        let name = s.file_stem().expect("stem").to_string_lossy().into_owned();
        let first = play(&s, &dir.path().join(format!("{name}.1.log")));
        let second = play(&s, &dir.path().join(format!("{name}.2.log")));
        assert_eq!(text(&first), text(&second), "{name}: two runs differ");
        let expected = std::fs::read(s.with_extension("log")).expect("committed log");
        assert_eq!(
            text(&first),
            text(&expected),
            "{name}: the log differs from the committed one"
        );
        let last = text(&first)
            .lines()
            .last()
            .map(str::to_owned)
            .unwrap_or_default();
        assert!(last.contains(" end reason=complete "), "{name}: {last}");
    }
}

#[test]
fn the_log_can_go_to_standard_output_alone() {
    let s = scenarios_dir().join("social.scenario");
    let out = run(&[
        "--headless",
        "--no-data",
        "--scenario",
        s.to_str().unwrap(),
        "--scenario-log",
        "-",
    ]);
    assert_eq!(
        out.status.code(),
        Some(0),
        "{}",
        String::from_utf8_lossy(&out.stderr)
    );
    let expected = std::fs::read(s.with_extension("log")).expect("committed log");
    assert_eq!(text(&out.stdout), text(&expected));
}

#[test]
fn a_run_without_a_log_still_finishes() {
    let s = scenarios_dir().join("combat.scenario");
    let out = run(&["--headless", "--no-data", "--scenario", s.to_str().unwrap()]);
    assert_eq!(out.status.code(), Some(0));
    assert!(out.stdout.is_empty() || !text(&out.stdout).contains("tessera-scenario-log"));
}

/// Writes `body` to a temporary scenario file.
fn file(dir: &Path, name: &str, body: &[u8]) -> PathBuf {
    let p = dir.join(name);
    std::fs::write(&p, body).expect("write");
    p
}

fn code(args: &[&str]) -> (Option<i32>, String) {
    let out = run(args);
    (out.status.code(), text(&out.stderr))
}

#[test]
fn a_missing_file_is_a_usage_error() {
    let dir = tempfile::tempdir().expect("tempdir");
    let missing = dir.path().join("nowhere.scenario");
    let (c, err) = code(&[
        "--headless",
        "--no-data",
        "--scenario",
        missing.to_str().unwrap(),
    ]);
    assert_eq!(c, Some(2), "{err}");
    assert!(err.contains("no such scenario file"), "{err}");
    let (c, err) = code(&[
        "--headless",
        "--no-data",
        "--scenario",
        dir.path().to_str().unwrap(),
    ]);
    assert_eq!(c, Some(2), "a directory: {err}");
}

#[test]
fn hostile_files_are_parse_errors_with_positions() {
    let dir = tempfile::tempdir().expect("tempdir");
    let mut huge = b"tessera-scenario 1\n".to_vec();
    huge.resize(tessera_net::scenario::MAX_FILE_BYTES + 100, b'#');
    let mut long_line = b"tessera-scenario 1\nsystem \"".to_vec();
    long_line.resize(10_000, b'a');
    long_line.extend_from_slice(b"\"\n");
    let cases: Vec<(&str, Vec<u8>, &str)> = vec![
        ("binary", vec![0xFF, 0xFE, 0x00, 0x01, 0x80, 0x9C], ":1:1:"),
        ("bom", b"\xEF\xBB\xBFtessera-scenario 1\n".to_vec(), ":1:1:"),
        ("empty", Vec::new(), ":1:1:"),
        ("version", b"tessera-scenario 9\n".to_vec(), ":1:"),
        (
            "control",
            b"tessera-scenario 1\nat 0s system \"a\x07b\"\n".to_vec(),
            ":2:",
        ),
        (
            "unclosed",
            b"tessera-scenario 1\non use player\n".to_vec(),
            ":2:",
        ),
        (
            "nesting",
            b"tessera-scenario 1\nat 0s repeat 2 every 1s\nat 0s repeat 2 every 1s\nend\nend\n"
                .to_vec(),
            ":3:",
        ),
        ("huge", huge, "FileTooLarge"),
        ("long-line", long_line, ":2:1:"),
        (
            "did-overflow",
            b"tessera-scenario 1\nselect $x = did 0x1FFFFFFFFFFFFFFFFF all\n".to_vec(),
            ":2:",
        ),
        (
            "horizon",
            b"tessera-scenario 1\nat 61min system \"late\"\n".to_vec(),
            ":2:",
        ),
    ];
    for (name, body, expect) in cases {
        let p = file(dir.path(), &format!("{name}.scenario"), &body);
        let log = dir.path().join(format!("{name}.log"));
        let (c, err) = code(&[
            "--headless",
            "--no-data",
            "--scenario",
            p.to_str().unwrap(),
            "--scenario-log",
            log.to_str().unwrap(),
        ]);
        assert_eq!(c, Some(3), "{name}: {err}");
        assert!(err.contains(expect), "{name}: {err}");
        assert!(
            err.contains(&format!("{name}.scenario:")),
            "{name}: FILE:LINE:COLUMN: {err}"
        );
        assert!(!log.exists(), "{name}: no log before parsing succeeds");
    }
}

#[test]
fn resolve_errors_exit_4() {
    let dir = tempfile::tempdir().expect("tempdir");
    let p = file(
        dir.path(),
        "needs.scenario",
        b"tessera-scenario 1\nrequires install\nat 0s system \"hi\"\n",
    );
    let (c, err) = code(&["--headless", "--no-data", "--scenario", p.to_str().unwrap()]);
    assert_eq!(c, Some(4), "{err}");
    assert!(err.contains("InstallRequired"), "{err}");
}

#[test]
fn a_named_install_that_cannot_be_used_exits_4() {
    // `--game-dir` (or `TESSERA_GAME_DIR`) naming a directory without archives fails the
    // run instead of quietly playing without data (§8.6 `InstallUnreadable`).
    let dir = tempfile::tempdir().expect("tempdir");
    let empty = dir.path().join("not-an-install");
    std::fs::create_dir(&empty).expect("dir");
    let s = scenarios_dir().join("combat.scenario");
    let log = dir.path().join("c.log");
    let (c, err) = code(&[
        "--headless",
        "--game-dir",
        empty.to_str().unwrap(),
        "--scenario",
        s.to_str().unwrap(),
        "--scenario-log",
        log.to_str().unwrap(),
    ]);
    assert_eq!(c, Some(4), "{err}");
    assert!(err.contains("InstallUnreadable"), "{err}");
    assert!(!log.exists(), "no log before resolution succeeds");
    let out = client()
        .env("TESSERA_GAME_DIR", &empty)
        .args(["--headless", "--scenario", s.to_str().unwrap()])
        .output()
        .expect("runs the client");
    assert_eq!(
        out.status.code(),
        Some(4),
        "{}",
        String::from_utf8_lossy(&out.stderr)
    );
}

#[test]
fn a_feed_failure_exits_5_with_an_error_end_line() {
    // 3,000 vendor windows of 1,000 entries each in two ticks: the held-over events pass
    // the queue's weight limit (§7.4), a run-time `QueueOverflow` (§11.3).
    let dir = tempfile::tempdir().expect("tempdir");
    let entries = vec!["\"a\""; 1000].join(",");
    let body = format!(
        "tessera-scenario 1\ntick 1000ms\nat 0s repeat 3000 every 1ms\n+0s vendor items={entries}\nend\n"
    );
    let p = file(dir.path(), "flood.scenario", body.as_bytes());
    let log = dir.path().join("flood.log");
    let (c, err) = code(&[
        "--headless",
        "--no-data",
        "--scenario",
        p.to_str().unwrap(),
        "--scenario-log",
        log.to_str().unwrap(),
    ]);
    assert_eq!(c, Some(5), "{err}");
    let body = std::fs::read_to_string(&log).expect("log");
    let last = body.lines().last().unwrap_or_default();
    assert!(
        last.contains(" end reason=error ") && last.contains("error=QueueOverflow"),
        "{last}"
    );
}

#[test]
fn scenario_conflicts_with_other_headless_tasks() {
    let dir = tempfile::tempdir().expect("tempdir");
    let s = scenarios_dir().join("combat.scenario");
    let s = s.to_str().unwrap();
    let walks = file(dir.path(), "walks.txt", b"");
    let png = dir.path().join("x.png");
    let own_path = dir.path().join("own.scenario");
    std::fs::copy(scenarios_dir().join("combat.scenario"), &own_path).expect("copy");
    let own = own_path.to_str().unwrap();
    let cases: Vec<Vec<&str>> = vec![
        vec![
            "--headless",
            "--scenario",
            s,
            "--walk-path",
            walks.to_str().unwrap(),
        ],
        vec!["--headless", "--scenario", s, "--fly-path", "1:1:1,1:2:1"],
        vec!["--headless", "--scenario", s, "--duration", "5"],
        vec!["--headless", "--scenario", s, "--ui-script"],
        vec!["--scenario", s, "--smoke"],
        vec!["--scenario", s, "--showcase"],
        // A scenario screenshot needs an install (§12.2).
        vec![
            "--no-data",
            "--scenario",
            s,
            "--screenshot",
            png.to_str().unwrap(),
        ],
        // --scenario-log and --scenario-input need --scenario, also next to a task that
        // conflicts with --scenario (clap alone would accept these and write no log).
        vec!["--headless", "--no-data", "--scenario-log", "x.log"],
        vec!["--no-data", "--scenario-input"],
        vec![
            "--headless",
            "--no-data",
            "--duration",
            "1",
            "--scenario-log",
            "x.log",
        ],
        vec!["--no-data", "--smoke", "--scenario-log", "x.log"],
        vec!["--no-data", "--smoke", "--scenario-input"],
        // The log must not overwrite the scenario file.
        vec![
            "--headless",
            "--no-data",
            "--scenario",
            own,
            "--scenario-log",
            own,
        ],
    ];
    for args in cases {
        let (c, err) = code(&args);
        assert_eq!(c, Some(2), "{args:?}: {err}");
        assert!(
            err.contains("Usage") || err.contains("usage"),
            "{args:?}: {err}"
        );
    }
    // The refused run left the scenario file as it was.
    assert_eq!(
        std::fs::read(&own_path).expect("read"),
        std::fs::read(scenarios_dir().join("combat.scenario")).expect("read")
    );
    assert!(!png.exists());
    // `--headless --scenario` is a complete headless task on its own.
    assert!(tessera_client::Args::try_parse_from(["t", "--headless", "--scenario", s]).is_ok());
}

/// `--scenario` with `--server` is an overlay run (`scenario-feed.md` §12.5): a file
/// outside the overlay subset fails with `NotAllowedInOverlay` before anything connects
/// (exit 3, §12.5.1).
#[test]
fn an_overlay_run_refuses_statements_outside_its_subset() {
    let s = scenarios_dir().join("combat.scenario");
    let (c, err) = code(&[
        "--headless",
        "--no-data",
        "--scenario",
        s.to_str().unwrap(),
        "--server",
        "127.0.0.1:9",
        "--account",
        "a",
    ]);
    assert_eq!(c, Some(3), "{err}");
    assert!(err.contains("NotAllowedInOverlay"), "{err}");
}

#[test]
fn the_log_is_never_written_into_the_install() {
    let install = tempfile::tempdir().expect("tempdir");
    let s = scenarios_dir().join("combat.scenario");
    let log = install.path().join("out.log");
    let (c, err) = code(&[
        "--headless",
        "--game-dir",
        install.path().to_str().unwrap(),
        "--scenario",
        s.to_str().unwrap(),
        "--scenario-log",
        log.to_str().unwrap(),
    ]);
    assert_ne!(c, Some(0), "{err}");
    assert_eq!(std::fs::read_dir(install.path()).unwrap().count(), 0);
}
