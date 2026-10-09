// SPDX-License-Identifier: GPL-3.0-or-later
//! `--scenario FILE`: play a scenario file (`docs/specs/scenario-feed.md`) with the sandbox
//! backend (decision 0024 §4, P0).
//!
//! The pipeline of §3.2: read the file (at most [`MAX_FILE_BYTES`]), parse it, resolve its
//! selectors against the install with the client's [`resolver`] (every selector resolves
//! empty without one, §8.7), then
//!
//! - **headless** (`--headless` or `--screenshot`, §12.2): run
//!   [`HeadlessDriver`] over the sandbox on the scenario's virtual clock to its end, write
//!   the canonical log (`--scenario-log`, §10) and exit with the codes of §12.2
//!   ([`EXIT_PARSE`], [`EXIT_RESOLVE`], [`EXIT_RUNTIME`]). With an install, the walker
//!   renders offscreen from the driver's events ([`screenshot`]) for `--screenshot` and
//!   whenever the spawn needs a world lookup (`--place`, `--spawn` without a height, §3.2);
//!   otherwise only the driver runs (no GPU needed; `scenario-feed.md` §12.4 OPEN (2));
//! - **windowed** (§12.4): the walker (or, without an install, the test scene) runs with the
//!   sandbox playing the scenario in real time; scenario events reach the existing handlers
//!   (chat lines in the chat window and overlay, actors with a body as figures), the HUD
//!   shows the scenario's state, the window caption its title, and `--scenario-log`
//!   writes the log with wall-clock times ([`live_log`]).
//!
//! Parse and resolve errors exit 3 and 4 in both modes, before the world opens. An install
//! the user named (`--game-dir`, `TESSERA_GAME_DIR`) that cannot be used is a resolve error
//! (`InstallUnreadable`, exit 4), never a quiet no-install run.

pub mod live_log;

pub mod resolver;
pub mod screenshot;

use std::io::{self, Read as _, Write};
use std::path::{Path, PathBuf};
use std::sync::Arc;
use std::time::Duration;

use tessera_net::sandbox::world::SpawnPoint;
use tessera_net::sandbox::{MAX_RESOLVE_TIMEOUT, SandboxBackend, SandboxConfig};
use tessera_net::scenario::{HeadlessDriver, MAX_FILE_BYTES, ResolvedScenario, Scenario};
use tessera_net::{Destination, RegionId, Session, Transform, Vec3};

use crate::cli::Args;
use resolver::ClientResolver;

/// Exit code: the scenario finished (`complete` or `horizon`).
pub const EXIT_OK: u8 = 0;
/// Exit code: another client failure (unreadable file, no session, ...).
pub const EXIT_FAILURE: u8 = 1;
/// Exit code: a command-line usage error (as clap's).
pub const EXIT_USAGE: u8 = 2;
/// Exit code: the scenario file did not parse (§11.2).
pub const EXIT_PARSE: u8 = 3;
/// Exit code: resolution failed (`InstallUnreadable`, `InstallRequired`, `ResolveLimit`).
pub const EXIT_RESOLVE: u8 = 4;
/// Exit code: a run-time error (feed failure, log write failure, unexpected disconnect).
pub const EXIT_RUNTIME: u8 = 5;

/// The resolve timeout of a scenario run with an install: the walker's (§3.2).
pub const INSTALL_RESOLVE_TIMEOUT: Duration = crate::walker_host::WALKER_RESOLVE_TIMEOUT;

/// Why a scenario run stopped before playing.
#[derive(Debug, Clone, PartialEq, Eq, thiserror::Error)]
pub enum PrepareError {
    /// A command-line usage error (exit 2; the message is shown with the usage).
    #[error("{0}")]
    Usage(String),
    /// The file could not be read (exit 1).
    #[error("{0}")]
    Read(String),
    /// The file did not parse: `FILE:LINE:COLUMN: KIND` (exit 3).
    #[error("{0}")]
    Parse(String),
    /// Resolution failed: `FILE:LINE: KIND` (exit 4).
    #[error("{0}")]
    Resolve(String),
}

impl PrepareError {
    /// The process exit code of §12.2.
    #[must_use]
    pub fn exit_code(&self) -> u8 {
        match self {
            Self::Usage(_) => EXIT_USAGE,
            Self::Read(_) => EXIT_FAILURE,
            Self::Parse(_) => EXIT_PARSE,
            Self::Resolve(_) => EXIT_RESOLVE,
        }
    }
}

/// Reads a scenario file: at most [`MAX_FILE_BYTES`] + 1 bytes, so an oversized file
/// reaches the parser's `FileTooLarge` check without being read whole.
///
/// # Errors
///
/// When the file cannot be opened or read.
pub fn read_file(path: &Path) -> io::Result<Vec<u8>> {
    let file = std::fs::File::open(path)?;
    let cap = MAX_FILE_BYTES.saturating_add(1);
    let mut bytes = Vec::new();
    file.take(u64::try_from(cap).unwrap_or(u64::MAX))
        .read_to_end(&mut bytes)?;
    Ok(bytes)
}

/// The install a scenario run resolves against ([`locate_install`]).
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum InstallChoice {
    /// None: `--no-data`, or automatic discovery found nothing (a no-install run, §8.7).
    None,
    /// The install directory.
    Found(PathBuf),
    /// The user named an install (`--game-dir` or `TESSERA_GAME_DIR`) that cannot be used:
    /// resolution fails with `InstallUnreadable` (exit 4, §8.6) instead of quietly running
    /// without data.
    Unusable(String),
}

impl InstallChoice {
    /// The install directory, when one was found.
    #[must_use]
    pub fn root(&self) -> Option<&Path> {
        match self {
            Self::Found(p) => Some(p),
            _ => None,
        }
    }
}

/// The install a scenario run resolves against: none with `--no-data`, else `--game-dir`,
/// `TESSERA_GAME_DIR` or automatic discovery (the walker's rules). Automatic discovery that
/// finds nothing gives a no-install run (§8.7); a named directory that is not usable gives
/// [`InstallChoice::Unusable`].
#[must_use]
pub fn locate_install(args: &Args) -> InstallChoice {
    if args.no_data {
        return InstallChoice::None;
    }
    let named = args.game_dir.is_some()
        || tessera_install::SearchEnv::from_process()
            .game_dir
            .is_some();
    match crate::data::find_install(args.game_dir.as_deref()) {
        Ok(install) => InstallChoice::Found(install.root),
        Err(reason) if named => {
            tracing::error!("scenario: the named install is not usable: {reason}");
            InstallChoice::Unusable(reason)
        }
        Err(reason) => {
            tracing::warn!("scenario: {reason}; selectors resolve empty (no-install run)");
            InstallChoice::None
        }
    }
}

/// Reads, parses and resolves the scenario of `args` (`--scenario`) against `install`
/// (§3.2 steps 1–2). Windowed runs play the input script only with `--scenario-input`
/// (§12.4); headless runs always do.
///
/// # Errors
///
/// A [`PrepareError`] with the message to show and the exit code to use.
pub fn prepare(args: &Args, install: &InstallChoice) -> Result<ResolvedScenario, PrepareError> {
    let Some(path) = &args.scenario else {
        return Err(PrepareError::Usage("--scenario is missing".into()));
    };
    let shown = path.display().to_string();
    let bytes = read_file(path).map_err(|e| PrepareError::Read(format!("{shown}: {e}")))?;
    let scenario = Scenario::parse(&bytes)
        .map_err(|e| PrepareError::Parse(e.with_file(&shown).to_string()))?;
    drop(bytes);
    if let InstallChoice::Unusable(reason) = install {
        // The run was asked to use this install: fail as an unreadable one does (§8.6),
        // even when no selector would have reached it.
        return Err(PrepareError::Resolve(format!(
            "{shown}: InstallUnreadable ({reason})"
        )));
    }
    let mut resolver = ClientResolver::for_install(install.root());
    let mut resolved = scenario
        .resolve(resolver.as_dyn())
        .map_err(|e| PrepareError::Resolve(format!("{shown}:{e}")))?;
    let headless = is_headless(args);
    if !headless {
        check_windowed_enter(&resolved)?;
    }
    resolved.set_plays_input(headless || args.scenario_input);
    tracing::info!(
        file = %shown,
        title = resolved.title().unwrap_or(""),
        install = resolved.has_install(),
        notes = resolved.notes().len(),
        "scenario resolved"
    );
    Ok(resolved)
}

/// Refuses a windowed run whose `enter` character only an `accept-character` step adds
/// (§4.3): the step fires on the input script's pre-world `create-character`, which a
/// windowed run does not play yet (§12.4 OPEN (1)), so the run could never enter.
///
/// # Errors
///
/// [`PrepareError::Usage`] naming the character.
fn check_windowed_enter(scenario: &ResolvedScenario) -> Result<(), PrepareError> {
    let Some(enter) = scenario.enter_character() else {
        return Ok(());
    };
    let key = crate::net::name_key(enter);
    if scenario
        .characters()
        .iter()
        .any(|c| crate::net::name_key(c) == key)
    {
        return Ok(());
    }
    Err(PrepareError::Usage(format!(
        "the scenario enters with \"{}\", a character only its input script creates; \
         windowed runs do not play the input script yet, so play it with --headless",
        enter.escape_debug()
    )))
}

/// Whether the scenario run of `args` is headless (`--headless`, or `--screenshot`, which
/// implies it).
#[must_use]
pub fn is_headless(args: &Args) -> bool {
    args.headless || args.screenshot.is_some()
}

/// The spawn point the command line asks for (`--spawn`, `--place`, with `--heading`);
/// `None` without either (§3.2: a headless run then spawns at the exact default).
///
/// # Errors
///
/// When the spawn text or place name is invalid (clap already checked `--spawn`).
pub fn spawn_point(args: &Args) -> anyhow::Result<Option<SpawnPoint>> {
    let point = if let Some(text) = &args.spawn {
        SpawnPoint::parse(text).map_err(|e| anyhow::anyhow!("--spawn {text}: {e}"))?
    } else if let Some(place) = &args.place {
        SpawnPoint::named(place.clone()).map_err(|e| anyhow::anyhow!("--place: {e}"))?
    } else {
        return Ok(None);
    };
    Ok(Some(point.with_heading(heading(args))))
}

/// The `--heading` as a sandbox heading (radians, 0 = east), 0 without one.
fn heading(args: &Args) -> f32 {
    args.heading
        .map(tessera_net::model::heading_from_compass_degrees)
        .unwrap_or(0.0)
}

/// The sandbox configuration of a headless scenario run (§3.2), also used by a windowed
/// run without the walker (`--no-data`): the default spawn (region 0, the origin, heading
/// 0 or `--heading`, exact) unless `--spawn` or `--place` gives one, the plain sandbox's
/// resolve timeout (10 s) without an install and the walker's (300 s) with one, and the
/// scenario. The sandbox itself turns off wanderers, crowd, latency and the welcome line
/// while a scenario plays.
///
/// # Errors
///
/// As [`spawn_point`].
pub fn headless_config(
    args: &Args,
    scenario: Arc<ResolvedScenario>,
) -> anyhow::Result<SandboxConfig> {
    let defaults = SandboxConfig::default();
    let spawn_point = spawn_point(args)?;
    let region = match spawn_point.as_ref().map(|p| &p.destination) {
        Some(
            Destination::Location { location, .. }
            | Destination::Ground { location, .. }
            | Destination::Anchored { location, .. },
        ) => location.region,
        Some(_) => RegionId(u16::from(args.region)),
        None => defaults.region,
    };
    let resolve_timeout = if scenario.has_install() {
        MAX_RESOLVE_TIMEOUT.min(INSTALL_RESOLVE_TIMEOUT)
    } else {
        defaults.resolve_timeout
    };
    Ok(SandboxConfig {
        region,
        spawn: Transform::new(Vec3::ZERO, heading(args)),
        spawn_point,
        resolve_timeout,
        scenario: Some(scenario),
        ..defaults
    })
}

/// Whether the spawn of `args` needs a world lookup (`--place`, or `--spawn` without a
/// height): only the walker's world source answers it (§3.2, §12.2 step 3).
///
/// # Errors
///
/// As [`spawn_point`].
pub fn spawn_needs_lookup(args: &Args) -> anyhow::Result<bool> {
    Ok(spawn_point(args)?.is_some_and(|p| {
        matches!(
            p.destination,
            Destination::Ground { .. } | Destination::Named(_)
        )
    }))
}

/// Where the canonical log goes.
fn open_log(args: &Args, install: Option<&Path>) -> Result<Option<Box<dyn Write + Send>>, String> {
    let Some(path) = &args.scenario_log else {
        return Ok(None);
    };
    if path.as_os_str() == "-" {
        return Ok(Some(Box::new(io::BufWriter::new(io::stdout()))));
    }
    let status = install.map(|root| crate::runtime::DataStatus::Ready {
        root: root.to_path_buf(),
        stats: crate::data::ArchiveStats::default(),
    });
    crate::runtime::refuse_install_path(args, status.as_ref(), path).map_err(|e| e.to_string())?;
    let file = std::fs::File::create(path)
        .map_err(|e| format!("cannot write the scenario log {}: {e}", path.display()))?;
    Ok(Some(Box::new(io::BufWriter::new(file))))
}

/// Runs `--scenario` as `args` says and returns the process exit code (§12.2): headless
/// to the scenario's end, or the windowed client.
#[must_use]
pub fn run(mut args: Args) -> u8 {
    if let Err(e) = args.check() {
        let _ = e.print();
        return EXIT_USAGE;
    }
    let headless = is_headless(&args);
    let install = locate_install(&args);
    if headless && args.screenshot.is_some() && install == InstallChoice::None {
        // §12.2: a scenario screenshot needs the walker, which needs an install.
        let e = usage_error("--screenshot with --scenario needs an install (none was found)");
        let _ = e.print();
        return EXIT_USAGE;
    }
    let resolved = match prepare(&args, &install) {
        Ok(r) => Arc::new(r),
        Err(e) => {
            eprintln!("tessera-client: {e}");
            return e.exit_code();
        }
    };
    let install = install.root();
    if headless {
        return run_headless(&args, resolved, install);
    }
    if args.scenario_input {
        // The input script's only player is the headless driver, which owns the session
        // (`scenario-feed.md` §12.4 OPEN): the windowed client cannot send its lines yet.
        tracing::warn!(
            "--scenario-input: the input script is not played in windowed runs yet; play \
             the scenario with --headless to run it"
        );
    }
    // `--scenario-log` (§12.4): the world session writes it, with wall-clock times.
    let slot = match open_log(&args, install) {
        Ok(l) => l.map(live_log::LiveLogSlot::new),
        Err(e) => {
            eprintln!("tessera-client: {e}");
            return EXIT_RUNTIME;
        }
    };
    args.scenario_prepared = Some(resolved);
    args.scenario_log_slot.clone_from(&slot);
    let code = match crate::app::run(args) {
        Ok(()) => EXIT_OK,
        Err(e) => {
            tracing::error!("{e:#}");
            EXIT_FAILURE
        }
    };
    windowed_exit_code(code, slot.as_ref())
}

/// A windowed run's exit code: a log that could not be written is a run-time error (5).
fn windowed_exit_code(code: u8, slot: Option<&live_log::LiveLogSlot>) -> u8 {
    if code == EXIT_OK && slot.is_some_and(live_log::LiveLogSlot::failed) {
        eprintln!("tessera-client: writing the scenario log failed");
        return EXIT_RUNTIME;
    }
    code
}

/// A clap usage error of the client's command (printed with the usage line, exit 2).
#[must_use]
pub fn usage_error(message: &str) -> clap::Error {
    use clap::CommandFactory as _;
    Args::command().error(clap::error::ErrorKind::ArgumentConflict, message)
}

/// Runs a resolved scenario headless (§12.2 steps 3–5) and returns the exit code. With an
/// install and `--screenshot`, or a spawn that needs a world lookup ([`spawn_needs_lookup`]),
/// the walker renders offscreen from the driver's events ([`screenshot::run`]; with
/// `--screenshot` it captures the frame after the last tick). Otherwise only the driver
/// runs: the walker would send no command and draw nothing anyone sees, and with an exact
/// (or default) spawn no lookup is pending, so the log is the same apart from the client
/// avatar's appearance (§12.4 OPEN (2), (3)).
#[must_use]
pub fn run_headless(args: &Args, scenario: Arc<ResolvedScenario>, install: Option<&Path>) -> u8 {
    let log = match open_log(args, install) {
        Ok(l) => l,
        Err(e) => {
            eprintln!("tessera-client: {e}");
            return EXIT_RUNTIME;
        }
    };
    let needs_lookup = match spawn_needs_lookup(args) {
        Ok(n) => n,
        Err(e) => {
            eprintln!("tessera-client: {e:#}");
            return EXIT_FAILURE;
        }
    };
    if args.screenshot.is_some() || (install.is_some() && needs_lookup) {
        return match screenshot::run(args, scenario, log) {
            Ok(code) => code,
            Err(e) => {
                tracing::error!("{e:#}");
                EXIT_FAILURE
            }
        };
    }
    let config = match headless_config(args, Arc::clone(&scenario)) {
        Ok(c) => c,
        Err(e) => {
            eprintln!("tessera-client: {e:#}");
            return EXIT_FAILURE;
        }
    };
    let backend = match SandboxBackend::new(config) {
        Ok(b) => b,
        Err(e) => {
            eprintln!("tessera-client: the sandbox refused the scenario run: {e}");
            return EXIT_FAILURE;
        }
    };
    let mut driver = HeadlessDriver::new(Session::new(backend), scenario, log);
    let outcome = driver.run();
    finish(driver.into_log(), outcome.exit_code(), &outcome)
}

/// Flushes the log and reports the outcome; a log that cannot be flushed is a run-time
/// error (exit 5).
fn finish(
    log: Option<Box<dyn Write + Send>>,
    code: u8,
    outcome: &tessera_net::scenario::DriverOutcome,
) -> u8 {
    if let Some(mut w) = log
        && let Err(e) = w.flush()
    {
        eprintln!("tessera-client: writing the scenario log failed: {e}");
        return EXIT_RUNTIME;
    }
    tracing::info!(
        reason = ?outcome.reason,
        time = %outcome.time,
        skipped = outcome.counters.skipped,
        input_skipped = outcome.counters.input_skipped,
        dropped = outcome.counters.dropped,
        spilled = outcome.counters.spilled,
        "scenario run ended"
    );
    code
}

#[cfg(test)]
mod tests {
    use super::*;
    use clap::Parser as _;

    fn args(extra: &[&str]) -> Args {
        let mut v = vec!["tessera-client"];
        v.extend_from_slice(extra);
        Args::try_parse_from(v).expect("args")
    }

    fn scenario_file(text: &str) -> tempfile::NamedTempFile {
        let mut f = tempfile::NamedTempFile::new().expect("temp file");
        f.write_all(text.as_bytes()).expect("write");
        f
    }

    #[test]
    fn prepare_maps_errors_to_exit_codes() {
        let bad = scenario_file("tessera-scenario 2\n");
        let a = args(&[
            "--no-data",
            "--headless",
            "--scenario",
            bad.path().to_str().unwrap(),
        ]);
        let e = prepare(&a, &InstallChoice::None).expect_err("version");
        assert_eq!(e.exit_code(), EXIT_PARSE);
        assert!(e.to_string().contains(":1:"), "{e}");
        let req = scenario_file("tessera-scenario 1\nrequires install\n");
        let a = args(&[
            "--no-data",
            "--headless",
            "--scenario",
            req.path().to_str().unwrap(),
        ]);
        let e = prepare(&a, &InstallChoice::None).expect_err("install required");
        assert_eq!(e.exit_code(), EXIT_RESOLVE);
        assert!(e.to_string().contains("InstallRequired"), "{e}");
        let ok = scenario_file("tessera-scenario 1\nplayer \"Testa\"\nat 0s system \"Hi\"\n");
        let a = args(&["--no-data", "--scenario", ok.path().to_str().unwrap()]);
        let r = prepare(&a, &InstallChoice::None).expect("prepares");
        assert!(
            !r.plays_input(),
            "windowed runs play input only with --scenario-input"
        );
        let a = args(&[
            "--no-data",
            "--scenario",
            ok.path().to_str().unwrap(),
            "--scenario-input",
        ]);
        assert!(
            prepare(&a, &InstallChoice::None)
                .expect("prepares")
                .plays_input()
        );
    }

    #[test]
    fn oversized_files_are_parse_errors_without_reading_them_whole() {
        let mut f = tempfile::NamedTempFile::new().expect("temp file");
        let chunk = vec![b'#'; 64 * 1024];
        for _ in 0..20 {
            f.write_all(&chunk).expect("write");
        }
        assert_eq!(
            read_file(f.path()).expect("reads").len(),
            MAX_FILE_BYTES + 1
        );
        let a = args(&[
            "--no-data",
            "--headless",
            "--scenario",
            f.path().to_str().unwrap(),
        ]);
        let e = prepare(&a, &InstallChoice::None).expect_err("too large");
        assert_eq!(e.exit_code(), EXIT_PARSE);
        assert!(e.to_string().contains("FileTooLarge"), "{e}");
    }

    #[test]
    fn headless_config_uses_the_exact_default_spawn() {
        let f = scenario_file("tessera-scenario 1\n");
        let a = args(&[
            "--no-data",
            "--headless",
            "--scenario",
            f.path().to_str().unwrap(),
        ]);
        let r = Arc::new(prepare(&a, &InstallChoice::None).expect("prepares"));
        let c = headless_config(&a, Arc::clone(&r)).expect("config");
        assert!(c.spawn_point.is_none());
        assert_eq!(c.region, RegionId(0));
        assert_eq!(c.spawn.position, Vec3::ZERO);
        assert_eq!(c.resolve_timeout, SandboxConfig::default().resolve_timeout);
        assert!(c.scenario.is_some());
        let a = args(&[
            "--no-data",
            "--headless",
            "--scenario",
            f.path().to_str().unwrap(),
            "--spawn",
            "1,126,140,10,20,30",
            "--heading",
            "0",
        ]);
        let c = headless_config(&a, r).expect("config");
        assert_eq!(c.region, RegionId(1));
        assert!(matches!(
            c.spawn_point.map(|p| p.destination),
            Some(Destination::Location { .. })
        ));
    }

    #[test]
    fn windowed_runs_refuse_an_enter_character_only_the_input_creates() {
        let f = scenario_file(
            "tessera-scenario 1\nplayer \"Tamsin\"\nenter \"Wren\"\n\
             on create-character name=\"Wren\"\n+0s accept-character \"Wren\"\nend\n\
             at 0s system \"hi\"\ninput\nat pre create-character \"Wren\"\nend\n",
        );
        let path = f.path().to_str().unwrap();
        for extra in [&[][..], &["--scenario-input"][..]] {
            let mut v = vec!["--no-data", "--scenario", path];
            v.extend_from_slice(extra);
            let e = prepare(&args(&v), &InstallChoice::None).expect_err("refused");
            assert_eq!(e.exit_code(), EXIT_USAGE);
            assert!(e.to_string().contains("Wren"), "{e}");
        }
        // Headless runs play the input script, so they enter.
        let a = args(&["--no-data", "--headless", "--scenario", path]);
        assert!(prepare(&a, &InstallChoice::None).is_ok());
        // A declared character is fine windowed.
        let g = scenario_file(
            "tessera-scenario 1\nplayer \"Tamsin\"\ncharacter \"Orrin\"\nenter \"Orrin\"\n",
        );
        let a = args(&["--no-data", "--scenario", g.path().to_str().unwrap()]);
        let r = prepare(&a, &InstallChoice::None);
        assert!(r.is_ok(), "{r:?}");
    }

    #[test]
    fn a_named_install_that_cannot_be_used_is_a_resolve_error() {
        let empty = tempfile::tempdir().expect("tempdir");
        let f = scenario_file("tessera-scenario 1\nplayer \"Testa\"\n");
        let a = args(&[
            "--headless",
            "--game-dir",
            empty.path().to_str().unwrap(),
            "--scenario",
            f.path().to_str().unwrap(),
        ]);
        let choice = locate_install(&a);
        assert!(matches!(choice, InstallChoice::Unusable(_)), "{choice:?}");
        assert!(choice.root().is_none());
        let e = prepare(&a, &choice).expect_err("unusable");
        assert_eq!(e.exit_code(), EXIT_RESOLVE);
        assert!(e.to_string().contains("InstallUnreadable"), "{e}");
        // Parse errors still come first (exit 3).
        let bad = scenario_file("nonsense\n");
        let a = args(&[
            "--headless",
            "--game-dir",
            empty.path().to_str().unwrap(),
            "--scenario",
            bad.path().to_str().unwrap(),
        ]);
        assert_eq!(
            prepare(&a, &choice).expect_err("parse").exit_code(),
            EXIT_PARSE
        );
        // `--no-data` never looks.
        let a = args(&["--no-data", "--scenario", f.path().to_str().unwrap()]);
        assert_eq!(locate_install(&a), InstallChoice::None);
    }

    #[test]
    fn only_ground_and_named_spawns_need_a_world_lookup() {
        let f = scenario_file("tessera-scenario 1\n");
        let p = f.path().to_str().unwrap();
        let needs = |extra: &[&str]| {
            let mut v = vec!["--headless", "--scenario", p];
            v.extend_from_slice(extra);
            spawn_needs_lookup(&args(&v)).expect("spawn")
        };
        assert!(!needs(&[]));
        assert!(!needs(&["--spawn", "1,126,140,10,20,30"]));
        assert!(needs(&["--spawn", "1,126,140,10,20"]));
        assert!(needs(&["--spawn", "1,126,140"]));
        assert!(needs(&["--place", "12.3S, 45.6W"]));
    }

    #[test]
    fn a_failed_windowed_log_exits_5() {
        let slot = live_log::LiveLogSlot::new(Box::new(io::sink()));
        assert_eq!(windowed_exit_code(EXIT_OK, Some(&slot)), EXIT_OK);
        assert_eq!(windowed_exit_code(EXIT_OK, None), EXIT_OK);
        assert_eq!(windowed_exit_code(EXIT_FAILURE, Some(&slot)), EXIT_FAILURE);
        struct Broken;
        impl Write for Broken {
            fn write(&mut self, _: &[u8]) -> io::Result<usize> {
                Err(io::Error::other("full"))
            }
            fn flush(&mut self) -> io::Result<()> {
                Ok(())
            }
        }
        let broken = live_log::LiveLogSlot::new(Box::new(Broken));
        let sc = prepare(
            &args(&[
                "--no-data",
                "--scenario",
                scenario_file("tessera-scenario 1\n")
                    .path()
                    .to_str()
                    .unwrap(),
            ]),
            &InstallChoice::None,
        )
        .expect("prepares");
        let _log = broken.take(&sc);
        assert_eq!(windowed_exit_code(EXIT_OK, Some(&broken)), EXIT_RUNTIME);
    }
}
