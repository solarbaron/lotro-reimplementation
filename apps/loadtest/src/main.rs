// SPDX-License-Identifier: GPL-3.0-or-later
//! `tessera-loadtest` binary: see the library documentation and `docs/specs/test-server.md`
//! §13. Exit status: 0 the run passed, 1 it failed (or could not run), 2 usage error.

use std::path::{Path, PathBuf};
use std::process::ExitCode;
use std::time::Duration;

use clap::Parser;
use tessera_loadtest::runner::{
    DEFAULT_AREA_BLOCKS, HostileOptions, RunOptions, Scenario, TargetOptions,
};
use tessera_loadtest::{LoadtestError, run};
use tessera_net::remote::Password;
use tracing_subscriber::EnvFilter;

/// Parses `250ms`, `30s`, `10m`, `1h` or a plain number of seconds.
fn duration(s: &str) -> Result<Duration, String> {
    let s = s.trim();
    let (num, unit) = match s.find(|c: char| c.is_ascii_alphabetic()) {
        Some(i) => s.split_at(i),
        None => (s, "s"),
    };
    let v: f64 = num
        .trim()
        .parse()
        .map_err(|_| format!("bad duration '{s}'"))?;
    let secs = match unit {
        "ms" => v / 1e3,
        "s" => v,
        "m" | "min" => v * 60.0,
        "h" => v * 3_600.0,
        _ => return Err(format!("bad duration unit in '{s}' (ms, s, m, h)")),
    };
    if !secs.is_finite() || !(0.0..=1.0e7).contains(&secs) {
        return Err(format!("duration out of range: '{s}'"));
    }
    Ok(Duration::from_secs_f64(secs))
}

/// An interval; `None` means "never".
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
struct Interval(Option<Duration>);

/// Like [`duration`], with `0` or `off` meaning "never".
fn interval(s: &str) -> Result<Interval, String> {
    if s.trim() == "off" {
        return Ok(Interval(None));
    }
    duration(s).map(|d| Interval((!d.is_zero()).then_some(d)))
}

/// Load and soak tests for Tessera's own protocol and test server.
///
/// Headless bots use the client's own network backend to connect to an in-process
/// tessera-server on loopback (the default; accounts and characters are created) or to a
/// remote one (--server with the server's fingerprint and --password-file). They walk and
/// run across landblock borders, teleport, change appearance, chat and reconnect, while
/// the tool checks desync, chat delivery, latency, traffic and memory growth. --hostile adds
/// clients that send malformed and flooding traffic at the server at the same time.
///
/// Use it only against your own test servers. Timing numbers are indicative unless
/// --reference-machine is given on the reference machine with no other load.
#[derive(Debug, Parser)]
#[command(name = "tessera-loadtest", version)]
struct Cli {
    /// Scenario: soak, walk, chat, churn, connect-storm or cheat.
    #[arg(long, default_value = "soak")]
    scenario: String,
    /// Number of bots (1-1024). The tool checks every observer-entity pair 10 times a
    /// second on one thread; the report says when it could not keep up (see
    /// docs/perf/m7-soak.md for the measured limit).
    #[arg(long, default_value_t = 16)]
    bots: usize,
    /// Run length, e.g. 30s, 10m, 1h (default: the scenario's).
    #[arg(long, value_parser = duration)]
    duration: Option<Duration>,
    /// Remote server connect string, tessera://host:port[?pin=sha256:...].
    #[arg(long, conflicts_with_all = ["in_process", "data_dir"])]
    server: Option<String>,
    /// The remote server's key fingerprint (when the connect string has no pin).
    #[arg(long, requires = "server")]
    fingerprint: Option<String>,
    /// File whose first line is every bot account's password (remote servers).
    #[arg(long, requires = "server")]
    password_file: Option<PathBuf>,
    /// Create the bot accounts with Register (the remote server must allow it).
    #[arg(long, requires = "server")]
    register: bool,
    /// PID of a tessera-server process on this machine: its resident memory is sampled
    /// from /proc and judged (with --server); the tool's own memory is then only reported.
    #[arg(long, requires = "server")]
    server_pid: Option<u32>,
    /// The remote server's Say range in metres ([chat] say_range_m). Without it, Say lines
    /// are judged only for the speaker's echo.
    #[arg(long, requires = "server")]
    say_range: Option<f64>,
    /// Run an in-process server on loopback (the default without --server).
    #[arg(long)]
    in_process: bool,
    /// Data directory of the in-process server (default: a temporary one). A directory
    /// that already has an accounts file is refused unless --reuse-data-dir is given.
    #[arg(long)]
    data_dir: Option<PathBuf>,
    /// Allow a --data-dir that already has accounts: the bot accounts in it
    /// (--account-prefix plus a number) get new random passwords.
    #[arg(long, requires = "data_dir")]
    reuse_data_dir: bool,
    /// Account names: this prefix plus a four-digit number.
    #[arg(long, default_value = "bot-")]
    account_prefix: String,
    /// Character names: this prefix (letters) plus three letters.
    #[arg(long, default_value = "Bot")]
    name_prefix: String,
    /// Side of the walking area in landblocks (1-16). At least 2R + 4 (8 for the default
    /// interest radius 2) lets every far teleport leave interest range; below R + 3 the
    /// leave side of interest management is not tested at all.
    #[arg(long, default_value_t = DEFAULT_AREA_BLOCKS)]
    area_blocks: u32,
    /// Fraction of bots that keep walking (the rest idle with short walks).
    #[arg(long)]
    walk_fraction: Option<f64>,
    /// Fraction of walking legs at run speed.
    #[arg(long)]
    run_fraction: Option<f64>,
    /// One chat line per bot per interval (0 or off: none).
    #[arg(long, value_parser = interval)]
    chat_interval: Option<Interval>,
    /// One teleport per bot per interval (0 or off: none).
    #[arg(long, value_parser = interval)]
    teleport_interval: Option<Interval>,
    /// Every interval a fraction of the bots reconnects (0 or off: never).
    #[arg(long, value_parser = interval)]
    reconnect_interval: Option<Interval>,
    /// The fraction of bots that reconnect each reconnect interval.
    #[arg(long)]
    reconnect_fraction: Option<f64>,
    /// One appearance change per bot per interval (0 or off: none).
    #[arg(long, value_parser = interval)]
    appearance_interval: Option<Interval>,
    /// Add hostile clients during the run.
    #[arg(long)]
    hostile: bool,
    /// Number of concurrent hostile clients.
    #[arg(long, default_value_t = 8)]
    hostile_clients: usize,
    /// Pause between two attacks of one hostile client (at most 10 s).
    #[arg(long, value_parser = duration, default_value = "200ms")]
    hostile_pace: Duration,
    /// Leak sampling interval.
    #[arg(long, value_parser = duration, default_value = "10s")]
    sample_interval: Duration,
    /// Shortest memory warm-up (the warm-up is the larger of this and 10 % of the run, at
    /// most half the run).
    #[arg(long, value_parser = duration, default_value = "5m")]
    warmup_min: Duration,
    /// Seed of the bots' random choices.
    #[arg(long)]
    seed: Option<u64>,
    /// Wait for the server's throttle tables to empty after the run (default: for runs of
    /// 5 minutes or more).
    #[arg(long)]
    check_throttle: Option<bool>,
    /// Declare this the reference machine under quiet conditions (decision 0010).
    #[arg(long)]
    reference_machine: bool,
    /// Write the samples as CSV to this file.
    #[arg(long)]
    csv: Option<PathBuf>,
    /// Write the summary as JSON to this file.
    #[arg(long)]
    json: Option<PathBuf>,
    /// More log output (repeatable); RUST_LOG overrides.
    #[arg(short, long, action = clap::ArgAction::Count)]
    verbose: u8,
}

fn options(cli: &Cli) -> Result<RunOptions, LoadtestError> {
    let scenario = Scenario::parse(&cli.scenario).ok_or_else(|| {
        LoadtestError::Usage(format!(
            "unknown scenario '{}' (soak, walk, chat, churn, connect-storm, cheat)",
            cli.scenario
        ))
    })?;
    let mut o = RunOptions::new(scenario, cli.bots);
    if let Some(d) = cli.duration {
        o.duration = d;
        o.check_throttle = d >= Duration::from_secs(300);
    }
    if let Some(server) = &cli.server {
        let mut connect = server.clone();
        if let Some(fp) = &cli.fingerprint {
            if connect.contains('?') {
                return Err(LoadtestError::Usage(
                    "give the pin either in the connect string or with --fingerprint".into(),
                ));
            }
            tessera_protocol::fingerprint::Fingerprint::parse(fp)
                .map_err(|_| LoadtestError::Usage("--fingerprint is not a fingerprint".into()))?;
            connect = format!("{connect}?pin={fp}");
        }
        let Some(pf) = &cli.password_file else {
            return Err(LoadtestError::Usage(
                "--password-file is required with --server".into(),
            ));
        };
        let text = std::fs::read_to_string(pf).map_err(|e| LoadtestError::io(pf, e))?;
        let password = text.lines().next().unwrap_or("").to_owned();
        if password.is_empty() {
            return Err(LoadtestError::Usage(
                "the password file's first line is empty".into(),
            ));
        }
        o.target = TargetOptions::Remote {
            connect,
            password: Password::new(password),
            register: cli.register,
        };
        o.check_throttle = false;
        o.say_range = cli.say_range;
    } else {
        o.target = TargetOptions::InProcess {
            data_dir: cli.data_dir.clone(),
        };
        o.reuse_data_dir = cli.reuse_data_dir;
    }
    o.account_prefix.clone_from(&cli.account_prefix);
    o.name_prefix.clone_from(&cli.name_prefix);
    o.area_blocks = cli.area_blocks;
    let b = &mut o.behaviour;
    if let Some(v) = cli.walk_fraction {
        b.walk_fraction = v;
    }
    if let Some(v) = cli.run_fraction {
        b.run_fraction = v;
    }
    if let Some(Interval(v)) = cli.chat_interval {
        b.chat_interval = v;
    }
    if let Some(Interval(v)) = cli.teleport_interval {
        b.teleport_interval = v;
    }
    if let Some(Interval(v)) = cli.reconnect_interval {
        b.reconnect_interval = v;
    }
    if let Some(v) = cli.reconnect_fraction {
        b.reconnect_fraction = v;
    }
    if let Some(Interval(v)) = cli.appearance_interval {
        b.appearance_interval = v;
    }
    if cli.hostile {
        o.hostile = Some(HostileOptions {
            clients: cli.hostile_clients,
            pace: cli.hostile_pace,
        });
    }
    o.sample_interval = cli.sample_interval;
    o.warmup_min = cli.warmup_min;
    if let Some(s) = cli.seed {
        o.seed = s;
    }
    if let Some(c) = cli.check_throttle {
        o.check_throttle = c;
    }
    o.reference_machine = cli.reference_machine;
    o.server_pid = cli.server_pid;
    Ok(o)
}

fn write(path: &Path, text: &str) -> Result<(), LoadtestError> {
    std::fs::write(path, text).map_err(|e| LoadtestError::io(path, e))
}

/// Creates (or truncates) every output file before the run, so a bad path fails at once
/// instead of after an hour's run.
fn preflight(paths: &[&Path]) -> Result<(), LoadtestError> {
    for p in paths {
        std::fs::OpenOptions::new()
            .write(true)
            .create(true)
            .truncate(true)
            .open(p)
            .map_err(|e| LoadtestError::io(*p, e))?;
    }
    Ok(())
}

/// Writes the output files after the run; returns the errors (the verdict is already
/// printed).
fn write_outputs(cli: &Cli, report: &tessera_loadtest::Report) -> Vec<LoadtestError> {
    let mut errors = Vec::new();
    if let Some(p) = &cli.csv
        && let Err(e) = write(p, &report.csv())
    {
        errors.push(e);
    }
    if let Some(p) = &cli.json
        && let Err(e) = write(p, &report.json())
    {
        errors.push(e);
    }
    errors
}

fn main() -> ExitCode {
    let cli = Cli::parse();
    let level = match cli.verbose {
        0 => "warn",
        1 => "info",
        2 => "debug",
        _ => "trace",
    };
    tracing_subscriber::fmt()
        .with_writer(std::io::stderr)
        .with_env_filter(
            EnvFilter::try_from_default_env().unwrap_or_else(|_| EnvFilter::new(level)),
        )
        .init();
    let outcome = options(&cli).and_then(|o| {
        let outputs: Vec<&Path> = cli
            .csv
            .iter()
            .chain(&cli.json)
            .map(PathBuf::as_path)
            .collect();
        preflight(&outputs)?;
        run(&o)
    });
    match outcome {
        Ok(report) => {
            // The verdict first: a file that cannot be written must not hide it.
            print!("{}", report.text());
            let errors = write_outputs(&cli, &report);
            for e in &errors {
                eprintln!("tessera-loadtest: {e}");
            }
            if report.passed() && errors.is_empty() {
                ExitCode::SUCCESS
            } else {
                ExitCode::from(1)
            }
        }
        Err(LoadtestError::Usage(m)) => {
            eprintln!("tessera-loadtest: {m}");
            ExitCode::from(2)
        }
        Err(e) => {
            eprintln!("tessera-loadtest: {e}");
            ExitCode::from(1)
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn durations() {
        assert_eq!(duration("250ms"), Ok(Duration::from_millis(250)));
        assert_eq!(duration("30"), Ok(Duration::from_secs(30)));
        assert_eq!(duration("10m"), Ok(Duration::from_secs(600)));
        assert_eq!(duration("1h"), Ok(Duration::from_secs(3_600)));
        assert!(duration("-1s").is_err());
        assert!(duration("1y").is_err());
        assert!(duration("x").is_err());
        assert_eq!(interval("off"), Ok(Interval(None)));
        assert_eq!(interval("0"), Ok(Interval(None)));
        assert_eq!(interval("5s"), Ok(Interval(Some(Duration::from_secs(5)))));
    }

    #[test]
    fn cli_parses() {
        let cli = Cli::try_parse_from([
            "t",
            "--scenario",
            "walk",
            "--bots",
            "2",
            "--duration",
            "5s",
            "--chat-interval",
            "off",
            "--teleport-interval",
            "7s",
        ])
        .unwrap();
        let o = options(&cli).unwrap();
        assert_eq!(o.bots, 2);
        assert_eq!(o.scenario, Scenario::Walk);
        assert_eq!(o.behaviour.chat_interval, None);
        assert_eq!(o.behaviour.teleport_interval, Some(Duration::from_secs(7)));
        let cli = Cli::try_parse_from(["t", "--scenario", "nope"]).unwrap();
        assert!(matches!(options(&cli), Err(LoadtestError::Usage(_))));
        let cli = Cli::try_parse_from(["t", "--server", "tessera://127.0.0.1:1"]).unwrap();
        assert!(matches!(options(&cli), Err(LoadtestError::Usage(_))));
        assert!(Cli::try_parse_from(["t", "--fingerprint", "x"]).is_err());
        let cli = Cli::try_parse_from(["t"]).unwrap();
        assert_eq!(options(&cli).unwrap().area_blocks, 8);
    }

    /// Regression: an unwritable --json path was found only after the run, and its error
    /// replaced the whole outcome. It now fails before the run starts.
    #[test]
    fn output_paths_are_checked_first() {
        let missing = Path::new("/nonexistent-tessera-dir/r.json");
        assert!(matches!(
            preflight(&[missing]),
            Err(LoadtestError::Io { .. })
        ));
        let dir = std::env::temp_dir().join(format!("tessera-lt-main-{}", std::process::id()));
        std::fs::create_dir_all(&dir).unwrap();
        let ok = dir.join("r.json");
        std::fs::write(&ok, "old").unwrap();
        preflight(&[ok.as_path()]).unwrap();
        assert_eq!(std::fs::read_to_string(&ok).unwrap(), "");
        let _ = std::fs::remove_dir_all(&dir);
    }

    #[test]
    fn remote_and_data_dir_options() {
        let cli = Cli::try_parse_from(["t", "--data-dir", "/tmp/x", "--reuse-data-dir"]).unwrap();
        assert!(options(&cli).unwrap().reuse_data_dir);
        assert!(Cli::try_parse_from(["t", "--reuse-data-dir"]).is_err());
        assert!(Cli::try_parse_from(["t", "--say-range", "20"]).is_err());
    }
}
