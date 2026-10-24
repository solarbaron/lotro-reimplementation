// SPDX-License-Identifier: GPL-3.0-or-later
//! The busiest scenario of P8 exit criterion 3 (decision 0043 §3; `docs/perf/p8-busiest.md`).
//!
//! - `busiest_scenario_has_the_decision_minimums` (no install): the committed synthetic
//!   `tests/scenarios/busiest.scenario` plays headless without data and its log shows the
//!   minimums of decision 0043 §3 item 1: at least 50 actors declared with a body, a
//!   fellowship of six, chat at 10 lines per second in every second, casts with an
//!   induction, effects, floating damage and heals, defeats, a synchronised start in every
//!   60 s window, and at least 60 s of it after any settle wait the measured run allows (the
//!   scenario lasts 180 s of virtual time).
//! - `timing_options_combine_as_the_cli_says` (no install): `--scenario-timing` needs
//!   `--scenario`, refuses `--no-data`, `--server` and `--screenshot`, and `--scenario` keeps
//!   refusing `--duration` (`scenario-feed.md` §12.1).
//! - `busiest_scenario_plays_deterministically_with_the_install` (`TESSERA_GAME_DIR`): the
//!   scenario resolves against the install and plays headless (driver only, §12.2) to its
//!   end twice at the reference location, with byte-identical logs, and the crowd's
//!   selectors resolve a body for every actor.
//! - `busiest_scenario_frame_budgets` (`TESSERA_GAME_DIR` and `TESSERA_PERF=1`): the two
//!   windowed timing runs of decision 0043 §3 item 2 (paced and unpaced, `--size 1920x1080`,
//!   `--ui-scale 1`, the bags, character and quest tracker panels open, 60 s after the
//!   settle wait) and the budgets of item 3. They need a display, an otherwise idle machine
//!   (decision 0010), a surface of at least 1024 × 768 (so that *Se* = 1) and a build with
//!   `--features audio-device` (budget (d) is measured only on the device backend); without
//!   `TESSERA_PERF=1` they do not run. Each run is waited for at most the settle timeout +
//!   the timed seconds + 240 s, then killed.
//!
//! The reference location is M3-03 of `docs/checklists/m3-reference-locations.md`: region 1,
//! landblock (109, 141), block-local (62.96, 15.07) at the terrain height 413.60, an exact
//! spawn (so a headless run needs no world lookup). Logs, frame logs and reports stay in a
//! temporary directory: with the install they hold record identifiers. Nothing is written
//! to the install.

use std::path::{Path, PathBuf};
use std::process::{Command, Output, Stdio};
use std::time::{Duration, Instant};

use tessera_client::timing::{FeedAtEnd, RunMode, TimingReport};

/// The reference location (M3-03) as an exact `--spawn`.
const SPAWN: &str = "1,109,141,62.96,15.07,413.60";
/// Seconds timed after the settle wait (decision 0043 §3 item 1: at least 60 s).
const TIMED_SECS: &str = "60";
/// The settle wait of the measured runs (`--settle-timeout`), seconds. The wait starts
/// once the player is placed; the scenario's clock starts at world entry and it lasts 180 s,
/// so the timed window ends while it still plays when placement takes under 60 s.
const SETTLE_SECS: &str = "60";
/// How long a windowed timing run may take before the test kills it: the settle wait, the
/// timed seconds and 240 s for loading (the client's own deadline is 180 s).
const RUN_LIMIT_SECS: u64 = 60 + 60 + 240;

/// Serialises the runs with the install (the test harness runs tests in parallel), so the
/// timing runs are not measured while the deterministic runs load the machine.
static INSTALL_RUNS: std::sync::Mutex<()> = std::sync::Mutex::new(());

fn one_at_a_time() -> std::sync::MutexGuard<'static, ()> {
    // A panic in another test poisons the lock; the client runs are still independent.
    INSTALL_RUNS
        .lock()
        .unwrap_or_else(std::sync::PoisonError::into_inner)
}

fn scenario() -> PathBuf {
    Path::new(env!("CARGO_MANIFEST_DIR")).join("tests/scenarios/busiest.scenario")
}

fn install() -> Option<PathBuf> {
    match std::env::var_os("TESSERA_GAME_DIR") {
        Some(d) if !d.is_empty() => Some(PathBuf::from(d)),
        _ => {
            eprintln!("TESSERA_GAME_DIR not set; skipping");
            None
        }
    }
}

fn perf() -> bool {
    std::env::var_os("TESSERA_PERF").is_some_and(|v| v == "1")
}

/// The client binary with no install in reach unless the test passes one, and its settings
/// and data in `home` (never the user's).
fn client(home: &Path) -> Command {
    let mut c = Command::new(env!("CARGO_BIN_EXE_tessera-client"));
    c.env_remove("TESSERA_GAME_DIR")
        .env_remove("TESSERA_PASSWORD")
        .env("RUST_LOG", "warn")
        .env("TESSERA_SETTINGS_FILE", home.join("settings.toml"))
        .env("XDG_CONFIG_HOME", home.join("config"))
        .env("XDG_DATA_HOME", home.join("data"));
    c
}

fn play(home: &Path, extra: &[&str], log: &Path) -> (Output, String) {
    let out = client(home)
        .args(["--headless", "--scenario"])
        .arg(scenario())
        .arg("--scenario-log")
        .arg(log)
        .args(extra)
        .output()
        .expect("runs the client");
    let text = std::fs::read_to_string(log).unwrap_or_default();
    (out, text)
}

/// Virtual seconds of a log line (`12.300 Event …`).
fn time_of(line: &str) -> Option<f64> {
    line.split(' ').next()?.parse().ok()
}

#[test]
fn busiest_scenario_has_the_decision_minimums() {
    let home = tempfile::tempdir().expect("tempdir");
    let (out, log) = play(home.path(), &["--no-data"], &home.path().join("b.log"));
    assert_eq!(
        out.status.code(),
        Some(0),
        "{}",
        String::from_utf8_lossy(&out.stderr)
    );
    let text = std::fs::read_to_string(scenario()).expect("scenario");
    let bodies = text
        .lines()
        .filter(|l| l.starts_with("actor ") && l.contains(" body=$"))
        .count();
    assert!(
        bodies >= 50,
        "{bodies} actors with a body (decision 0043: 50)"
    );
    let lines: Vec<&str> = log.lines().collect();
    let count = |what: &str| lines.iter().filter(|l| l.contains(what)).count();
    let spawned = count(" EntitySpawned ") + count(" EntityCreated ");
    assert!(spawned >= bodies, "{spawned} spawns");
    let six = lines
        .iter()
        .find(|l| l.contains(" FellowshipChanged "))
        .map(|l| l.matches("{name=").count());
    assert_eq!(
        six,
        Some(6),
        "a full fellowship: the player and five members"
    );
    assert!(count(" FellowVitalChanged ") >= 100, "fellow vitals change");
    assert!(count("cast=induction") >= 1, "an induction");
    assert!(count(" CastStarted ") >= 10, "casts");
    assert!(count(" EffectApplied ") >= 10, "effects");
    assert!(
        count("kind=damage") >= 10 && count("kind=heal") >= 1,
        "floating text"
    );
    assert!(count(" Defeated ") >= 1, "a defeat");
    assert!(count(" TargetChanged ") >= 1, "a selection");
    // Synchronised music in any 60 s window the measured run can time: the window starts
    // after the settle wait (at most 60 s after placement, `SETTLE_SECS`), so every window
    // starting in the first 120 s holds a synchronised start.
    let starts: Vec<f64> = lines
        .iter()
        .filter(|l| l.contains(" PerformanceSyncStart "))
        .filter_map(|l| time_of(l))
        .collect();
    for w in 0..=120u32 {
        let w = f64::from(w);
        assert!(
            starts.iter().any(|&t| t >= w && t <= w + 60.0),
            "no synchronised start in the window {w}-{} s: {starts:?}",
            w + 60.0
        );
    }
    assert!(count(" LootShown ") >= 1, "a window the scenario opens");
    assert!(count(" ItemAdded ") >= 1, "bag contents");
    // Chat at a steady 10 lines per second: every whole second of the run has 10.
    let mut per_second = vec![0usize; 180];
    for l in lines.iter().filter(|l| l.contains(" ChatMessage ")) {
        if let Some(t) = time_of(l) {
            let s = t.floor() as usize;
            if let Some(c) = per_second.get_mut(s) {
                *c += 1;
            }
        }
    }
    for (s, c) in per_second.iter().enumerate().take(179) {
        assert!(*c >= 10, "second {s}: {c} chat lines");
    }
    let end = lines.last().copied().unwrap_or_default();
    assert!(end.starts_with("180.000 end reason=complete"), "{end}");
}

#[test]
fn timing_options_combine_as_the_cli_says() {
    let home = tempfile::tempdir().expect("tempdir");
    let s = scenario();
    let s = s.to_str().expect("utf-8");
    let code = |args: &[&str]| {
        client(home.path())
            .args(args)
            .output()
            .expect("runs the client")
            .status
            .code()
    };
    assert_eq!(code(&["--headless", "--scenario-timing", "5"]), Some(2));
    assert_eq!(
        code(&[
            "--headless",
            "--no-data",
            "--scenario",
            s,
            "--scenario-timing",
            "5"
        ]),
        Some(2)
    );
    assert_eq!(
        code(&[
            "--scenario",
            s,
            "--scenario-timing",
            "5",
            "--screenshot",
            "x.png"
        ]),
        Some(2)
    );
    assert_eq!(
        code(&["--headless", "--scenario", s, "--duration", "5"]),
        Some(2),
        "--scenario keeps refusing --duration"
    );
    assert_eq!(
        code(&["--headless", "--scenario", s, "--scenario-timing", "0"]),
        Some(2)
    );
}

#[test]
fn busiest_scenario_plays_deterministically_with_the_install() {
    let Some(dir) = install() else { return };
    let _one = one_at_a_time();
    let home = tempfile::tempdir().expect("tempdir");
    assert!(
        !tessera_client::runtime::is_inside(home.path(), &dir),
        "outputs must not go into the install"
    );
    let game = dir.to_str().expect("utf-8 game dir");
    let extra = ["--game-dir", game, "--spawn", SPAWN];
    let (a_out, a) = play(home.path(), &extra, &home.path().join("1.log"));
    assert_eq!(
        a_out.status.code(),
        Some(0),
        "{}",
        String::from_utf8_lossy(&a_out.stderr)
    );
    let (b_out, b) = play(home.path(), &extra, &home.path().join("2.log"));
    assert_eq!(b_out.status.code(), Some(0));
    assert!(a == b, "two runs with the install wrote different logs");
    let lines: Vec<&str> = a.lines().collect();
    assert_eq!(lines.get(1), Some(&"install present"));
    let got = |binding: &str| {
        lines
            .iter()
            .find(|l| l.starts_with(&format!("resolve {binding} ")))
            .and_then(|l| l.split(' ').find_map(|f| f.strip_prefix("got=")))
            .and_then(|n| n.parse::<usize>().ok())
            .unwrap_or(0)
    };
    let (folk, beasts) = (got("$folk"), got("$beasts"));
    eprintln!(
        "busiest: {} log lines, $folk {folk}, $beasts {beasts}, $strikes {}, $wards {}, $boons {}, $wares {}, $errands {}",
        lines.len(),
        got("$strikes"),
        got("$wards"),
        got("$boons"),
        got("$wares"),
        got("$errands")
    );
    assert_eq!(folk, 36, "every NPC of the crowd has a body");
    assert_eq!(beasts, 19, "every creature of the crowd has a body");
    let created = lines
        .iter()
        .filter(|l| l.contains(" EntityCreated "))
        .count();
    assert!(created >= 50, "{created} actors spawned with a figure");
    let end = lines.last().copied().unwrap_or_default();
    assert!(end.contains(" end reason=complete "), "{end}");
}

/// One windowed timing run; returns its report.
fn timed_run(game: &str, home: &Path, paced: bool) -> TimingReport {
    let report = home.join(if paced { "paced.md" } else { "unpaced.md" });
    let frames = home.join(if paced { "paced.csv" } else { "unpaced.csv" });
    let mut c = client(home);
    c.args(["--game-dir", game, "--scenario"])
        .arg(scenario())
        .args([
            "--spawn",
            SPAWN,
            "--size",
            "1920x1080",
            "--ui-scale",
            "1",
            "--open-panel",
            "bags",
            "--open-panel",
            "character",
            "--open-panel",
            "tracker",
            "--settle-timeout",
            SETTLE_SECS,
            "--scenario-timing",
            TIMED_SECS,
            "--data-dir",
        ])
        .arg(home.join("tessera-data"))
        .arg("--ui-report")
        .arg(&report)
        .arg("--frame-log")
        .arg(&frames);
    if !paced {
        c.arg("--no-vsync");
    }
    // The client's output goes to a file (a pipe nobody reads could fill and stall it), and
    // the run is waited for with a bound, so a window that never closes fails the test.
    let err_path = home.join(if paced { "paced.err" } else { "unpaced.err" });
    let err = std::fs::File::create(&err_path).expect("stderr file");
    let mut child = c
        .stdout(Stdio::null())
        .stderr(Stdio::from(err))
        .spawn()
        .expect("runs the client");
    let limit = Instant::now() + Duration::from_secs(RUN_LIMIT_SECS);
    let status = loop {
        if let Some(s) = child.try_wait().expect("waits for the client") {
            break s;
        }
        if Instant::now() > limit {
            let _ = child.kill();
            let _ = child.wait();
            panic!(
                "the windowed timing run did not end within {RUN_LIMIT_SECS} s: {}",
                std::fs::read_to_string(&err_path).unwrap_or_default()
            );
        }
        std::thread::sleep(Duration::from_millis(200));
    };
    assert_eq!(
        status.code(),
        Some(0),
        "{}",
        std::fs::read_to_string(&err_path).unwrap_or_default()
    );
    let md = std::fs::read_to_string(&report).expect("timing report written");
    eprintln!("{md}");
    TimingReport::parse(&md).expect("timing report figures")
}

#[test]
fn busiest_scenario_frame_budgets() {
    let Some(dir) = install() else { return };
    if !perf() {
        eprintln!("TESSERA_PERF not set; the windowed timing runs are not made");
        return;
    }
    assert!(
        std::env::var_os("WAYLAND_DISPLAY").is_some() || std::env::var_os("DISPLAY").is_some(),
        "TESSERA_PERF=1: the windowed runs of decision 0043 §3 need a display"
    );
    let _one = one_at_a_time();
    let home = tempfile::tempdir().expect("tempdir");
    let game = dir.to_str().expect("utf-8 game dir");
    let mut failures = Vec::new();
    for paced in [true, false] {
        let r = timed_run(game, home.path(), paced);
        assert_eq!(r.mode, RunMode::Windowed);
        assert_eq!(r.paced, paced);
        assert_eq!(
            r.feed,
            FeedAtEnd::Running,
            "the scenario must still play when the timed window ends"
        );
        assert!(r.frames.seconds >= 59.0, "{} s timed", r.frames.seconds);
        let ui = r.ui.expect("the game UI recorded the run");
        // Decision 0043 §3 item 3 (c) judges U1–U5 at Se = 1, which needs a surface of at
        // least 1024 × 768 (`ui-scaling.md` §4.3); `budget_failures` reports (c) as not
        // judged otherwise, and this says why.
        assert!(
            r.surface.0 >= 1024 && r.surface.1 >= 768,
            "the compositor gave {} × {} (Se = {:.4}): give the window its own workspace",
            r.surface.0,
            r.surface.1,
            ui.ui_scale
        );
        failures.extend(
            r.budget_failures()
                .into_iter()
                .map(|f| format!("{}: {f}", if paced { "paced" } else { "unpaced" })),
        );
    }
    assert!(failures.is_empty(), "budgets missed: {failures:?}");
}
