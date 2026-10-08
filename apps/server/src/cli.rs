// SPDX-License-Identifier: GPL-3.0-or-later
//! The command line (`test-server.md` §2): `run`, `keygen`, `fingerprint`, `account …`,
//! `character …` and `check-content`, with the exit statuses of §2.8.

use std::io::{BufRead, IsTerminal, Read, Write};
use std::path::{Path, PathBuf};
use std::sync::atomic::Ordering;

use clap::{Args, Parser, Subcommand, ValueEnum};

use crate::accounts::{self, HashParams};
use crate::characters::{self, StoredPosition};
use crate::config::{Config, LogFormat};
use crate::content::{self, Content};
use crate::datadir::{self, DataDir, ServerLock};
use crate::error::{ServerError, exit};
use crate::names;
use crate::server::{Server, ServerOptions};
use crate::tomlx::{Finding, Rule};

const OPERATOR_RULES: &str = "\
Operator rules (docs/specs/server-content.md §8): run a Tessera test server only for private \
or LAN testing, for yourself and people you know. Do not list, advertise or host it publicly, \
and do not open registration to strangers. Never charge money or take payments, donations, \
ads or sales connected with it. The server holds and sends no game data; each player needs \
their own legally obtained installation of the game. The server never connects to any \
official service.";

/// `tessera-server`: Tessera's own headless test server.
#[derive(Debug, Parser)]
#[command(name = "tessera-server", version, about, long_about = None, after_help = OPERATOR_RULES)]
pub struct Cli {
    /// The server's data directory.
    #[arg(long, global = true, env = datadir::DATA_DIR_ENV, value_name = "DIR")]
    pub data_dir: Option<PathBuf>,
    /// Configuration file (default: `<data-dir>/server.toml`).
    #[arg(long, global = true, value_name = "FILE")]
    pub config: Option<PathBuf>,
    /// More log output (repeatable).
    #[arg(short, long, global = true, action = clap::ArgAction::Count)]
    pub verbose: u8,
    /// Less log output (repeatable).
    #[arg(short, long, global = true, action = clap::ArgAction::Count)]
    pub quiet: u8,
    /// The command.
    #[command(subcommand)]
    pub command: Command,
}

/// Log line format on the command line.
#[derive(Debug, Clone, Copy, ValueEnum)]
pub enum LogFormatArg {
    /// Human-readable text.
    Text,
    /// One JSON object per line.
    Json,
}

/// Commands.
#[derive(Debug, Subcommand)]
pub enum Command {
    /// Run the server in the foreground until SIGINT or SIGTERM.
    Run {
        /// UDP address to listen on (repeatable; replaces `[server] listen`).
        #[arg(long, value_name = "ADDR:PORT")]
        listen: Vec<String>,
        /// Content file (replaces `[server] content`).
        #[arg(long, value_name = "FILE")]
        content: Option<PathBuf>,
        /// Log line format (replaces `[log] format`).
        #[arg(long, value_enum)]
        log_format: Option<LogFormatArg>,
    },
    /// Create the server key and print its fingerprint.
    Keygen {
        /// Replace an existing key (clients that pinned it will fail with KEY_MISMATCH).
        #[arg(long)]
        force: bool,
    },
    /// Print the server key's fingerprint, or connect strings with --connect.
    Fingerprint {
        /// Print one connect string per configured listen port with this host.
        #[arg(long, value_name = "HOST")]
        connect: Option<String>,
    },
    /// Manage accounts.
    #[command(subcommand)]
    Account(AccountCmd),
    /// Manage characters.
    #[command(subcommand)]
    Character(CharacterCmd),
    /// Check content files (with no FILE: the content `run` would use and the configuration).
    CheckContent {
        /// Content files.
        files: Vec<PathBuf>,
    },
}

/// How a new password is given.
#[derive(Debug, Args)]
#[group(multiple = false)]
pub struct PasswordSource {
    /// Read the password from the first line of standard input.
    #[arg(long)]
    pub password_stdin: bool,
    /// Generate a random password and print it once.
    #[arg(long)]
    pub generate: bool,
}

/// Account commands.
#[derive(Debug, Subcommand)]
pub enum AccountCmd {
    /// Create an account.
    Add {
        /// Account name (ASCII letters, digits, '.', '_', '-'; stored lowercase).
        name: String,
        #[command(flatten)]
        password: PasswordSource,
        /// Create it disabled.
        #[arg(long)]
        disabled: bool,
    },
    /// List accounts.
    List,
    /// Replace an account's password.
    Passwd {
        /// Account name.
        name: String,
        #[command(flatten)]
        password: PasswordSource,
    },
    /// Disable an account (a running server ends its session).
    Disable {
        /// Account name.
        name: String,
    },
    /// Enable an account.
    Enable {
        /// Account name.
        name: String,
    },
    /// Mute an account (chat refused).
    Mute {
        /// Account name.
        name: String,
    },
    /// Unmute an account.
    Unmute {
        /// Account name.
        name: String,
    },
    /// Remove an account and its characters (refused while a server runs).
    Remove {
        /// Account name.
        name: String,
        /// Do not ask for confirmation.
        #[arg(long)]
        yes: bool,
    },
}

/// Character commands.
#[derive(Debug, Subcommand)]
pub enum CharacterCmd {
    /// List characters (all, or one account's).
    List {
        /// Account name.
        account: Option<String>,
    },
    /// Remove a character (refused while a server runs).
    Remove {
        /// Character name.
        name: String,
        /// Do not ask for confirmation.
        #[arg(long)]
        yes: bool,
    },
    /// Reset a character's stored position to a content spawn point (refused while a server
    /// runs).
    Move {
        /// Character name.
        name: String,
        /// Spawn point name.
        #[arg(long)]
        spawn: String,
    },
}

/// Runs the command line with the process arguments; returns the exit status.
#[must_use]
pub fn main() -> u8 {
    let cli = Cli::parse();
    run_cli(cli)
}

/// Runs a parsed command line; returns the exit status (§2.8).
#[must_use]
pub fn run_cli(cli: Cli) -> u8 {
    let quiet_default = !matches!(cli.command, Command::Run { .. });
    if quiet_default {
        init_simple_logging(cli.verbose, cli.quiet);
    }
    match dispatch(&cli) {
        Ok(status) => status,
        Err(e) => {
            eprintln!("tessera-server: {e}");
            e.exit_status()
        }
    }
}

fn data_dir(cli: &Cli) -> Result<DataDir, ServerError> {
    match &cli.data_dir {
        Some(d) => Ok(DataDir::new(d)),
        None => datadir::default_data_dir()
            .map(DataDir::new)
            .ok_or_else(|| {
                ServerError::Usage(
                    "no data directory: pass --data-dir or set TESSERA_SERVER_DIR".into(),
                )
            }),
    }
}

fn config_path(cli: &Cli, dir: &DataDir) -> (PathBuf, bool) {
    match &cli.config {
        Some(p) => (p.clone(), true),
        None => (dir.config_path(), false),
    }
}

/// Loads the configuration; findings are a [`ServerError::Config`].
fn load_config(cli: &Cli, dir: &DataDir) -> Result<Config, ServerError> {
    let (path, explicit) = config_path(cli, dir);
    let (cfg, findings) = Config::load(&path, explicit).map_err(|e| ServerError::io(&path, e))?;
    if !findings.is_empty() {
        return Err(ServerError::Config(
            findings.into_iter().map(|f| (path.clone(), f)).collect(),
        ));
    }
    Ok(cfg)
}

/// The content `run` uses: the configured file or the built-in default world, with the
/// checks of §5.6 minus SPDX. Findings are a [`ServerError::Content`].
fn load_content(dir: &DataDir, configured: Option<&Path>) -> Result<Content, ServerError> {
    match configured {
        None => Ok(Content::default_world()),
        Some(p) => {
            let path = dir.resolve(p);
            match content::check_file(&path, false) {
                (Some(c), f) if f.is_empty() => Ok(c),
                (_, f) => Err(ServerError::Content(
                    f.into_iter().map(|f| (path.clone(), f)).collect(),
                )),
            }
        }
    }
}

fn dispatch(cli: &Cli) -> Result<u8, ServerError> {
    match &cli.command {
        Command::Run {
            listen,
            content,
            log_format,
        } => cmd_run(cli, listen, content.as_deref(), *log_format),
        Command::Keygen { force } => cmd_keygen(cli, *force),
        Command::Fingerprint { connect } => cmd_fingerprint(cli, connect.as_deref()),
        Command::Account(a) => cmd_account(cli, a),
        Command::Character(c) => cmd_character(cli, c),
        Command::CheckContent { files } => cmd_check_content(cli, files),
    }
}

// ---------------------------------------------------------------------------- logging

fn level_name(base: &str, verbose: u8, quiet: u8) -> &'static str {
    const LEVELS: [&str; 5] = ["error", "warn", "info", "debug", "trace"];
    let idx = LEVELS.iter().position(|l| *l == base).unwrap_or(2) as i32;
    let idx = (idx + i32::from(verbose) - i32::from(quiet)).clamp(0, 4);
    LEVELS[idx as usize]
}

fn init_simple_logging(verbose: u8, quiet: u8) {
    let filter = tracing_subscriber::EnvFilter::try_from_default_env()
        .unwrap_or_else(|_| tracing_subscriber::EnvFilter::new(level_name("warn", verbose, quiet)));
    let _ = tracing_subscriber::fmt()
        .with_env_filter(filter)
        .with_writer(std::io::stderr)
        .try_init();
}

fn init_run_logging(cfg: &Config, verbose: u8, quiet: u8) -> Result<(), ServerError> {
    let filter = tracing_subscriber::EnvFilter::try_from_default_env().unwrap_or_else(|_| {
        tracing_subscriber::EnvFilter::new(level_name(&cfg.log.level, verbose, quiet))
    });
    let builder = tracing_subscriber::fmt().with_env_filter(filter);
    let json = cfg.log.format == LogFormat::Json;
    match &cfg.log.file {
        Some(path) => {
            let mut o = std::fs::OpenOptions::new();
            o.create(true).append(true);
            #[cfg(unix)]
            {
                use std::os::unix::fs::OpenOptionsExt;
                o.mode(0o600);
            }
            let file = o.open(path).map_err(|e| ServerError::io(path, e))?;
            let writer = std::sync::Mutex::new(file);
            if json {
                let _ = builder.json().with_writer(writer).try_init();
            } else {
                let _ = builder.with_ansi(false).with_writer(writer).try_init();
            }
        }
        None => {
            if json {
                let _ = builder.json().with_writer(std::io::stderr).try_init();
            } else {
                let _ = builder.with_writer(std::io::stderr).try_init();
            }
        }
    }
    Ok(())
}

// ---------------------------------------------------------------------------- run

fn cmd_run(
    cli: &Cli,
    listen: &[String],
    content: Option<&Path>,
    log_format: Option<LogFormatArg>,
) -> Result<u8, ServerError> {
    let dir = data_dir(cli)?;
    dir.create()?;
    // Take the lock before reading anything else and hold it until exit (§2.2 step 1,
    // §3.7): the server keeps this one.
    let lock = ServerLock::acquire(&dir, true)?;
    let mut cfg = load_config(cli, &dir)?;
    if !listen.is_empty() {
        let mut addrs = Vec::new();
        for l in listen {
            addrs.push(crate::config::parse_listen(l).ok_or_else(|| {
                ServerError::Usage(format!(
                    "--listen {l}: not an address host:port with an IP address and a port 1–65535"
                ))
            })?);
        }
        cfg.server.listen = addrs;
    }
    if let Some(c) = content {
        // A command-line path is relative to the working directory, like `--config` and
        // `check-content` (only `[server] content` in server.toml is relative to the data
        // directory, §4.2).
        let c = std::path::absolute(c).map_err(|e| ServerError::io(c, e))?;
        cfg.server.content = Some(c);
    }
    if let Some(f) = log_format {
        cfg.log.format = match f {
            LogFormatArg::Text => LogFormat::Text,
            LogFormatArg::Json => LogFormat::Json,
        };
    }
    init_run_logging(&cfg, cli.verbose, cli.quiet)?;
    let content = load_content(&dir, cfg.server.content.as_deref())?;
    tracing::info!(id = %content.id, title = %content.title, "content loaded");
    let handle = Server::start_locked(
        ServerOptions {
            data_dir: dir,
            config: cfg,
            content,
        },
        lock,
    )?;
    wait_for_signals(handle)
}

fn wait_for_signals(handle: crate::ServerHandle) -> Result<u8, ServerError> {
    let rt = tokio::runtime::Builder::new_current_thread()
        .enable_all()
        .build()
        .map_err(|e| ServerError::Runtime(format!("tokio runtime: {e}")))?;
    let grace = handle.grace();
    let hurry = handle.hurry_flag();
    rt.block_on(async move {
        #[cfg(unix)]
        let mut term = tokio::signal::unix::signal(tokio::signal::unix::SignalKind::terminate())
            .map_err(|e| ServerError::Runtime(format!("signal handler: {e}")))?;
        let (done_tx, mut done_rx) = tokio::sync::oneshot::channel::<crate::ShutdownReport>();
        let mut handle = Some(handle);
        let mut done_tx = Some(done_tx);
        let mut signals = 0u32;
        loop {
            #[cfg(unix)]
            let signal = async {
                tokio::select! {
                    _ = tokio::signal::ctrl_c() => {},
                    _ = term.recv() => {},
                }
            };
            #[cfg(not(unix))]
            let signal = async {
                let _ = tokio::signal::ctrl_c().await;
            };
            tokio::select! {
                () = signal => {}
                r = &mut done_rx, if handle.is_none() => {
                    let r = r.unwrap_or(crate::ShutdownReport { saved: 0, failures: 1 });
                    return Ok(if r.failures == 0 { exit::OK } else { exit::RUNTIME });
                }
            }
            signals += 1;
            match signals {
                1 => {
                    tracing::info!(grace_s = grace.as_secs(), "shutting down");
                    if let (Some(h), Some(tx)) = (handle.take(), done_tx.take()) {
                        std::thread::spawn(move || {
                            let r = h.shutdown(grace);
                            let _ = tx.send(r);
                        });
                    }
                }
                2 => {
                    tracing::info!("second signal: ending the grace period");
                    hurry.store(true, Ordering::Release);
                }
                _ => {
                    tracing::error!("third signal: exiting at once; characters in the world since their last save are not saved");
                    std::process::exit(i32::from(exit::RUNTIME));
                }
            }
        }
    })
}

// ---------------------------------------------------------------------------- keys

fn cmd_keygen(cli: &Cli, force: bool) -> Result<u8, ServerError> {
    let dir = data_dir(cli)?;
    dir.create()?;
    let _lock = ServerLock::acquire(&dir, false)?;
    let (identity, old) = datadir::keygen(&dir, force)?;
    match old {
        Some(old) => {
            println!("old fingerprint: {old}");
            println!("new fingerprint: {}", identity.fingerprint());
            eprintln!(
                "warning: every client that pinned the old fingerprint will now fail with KEY_MISMATCH"
            );
        }
        None => println!("{}", identity.fingerprint()),
    }
    Ok(exit::OK)
}

fn cmd_fingerprint(cli: &Cli, connect: Option<&str>) -> Result<u8, ServerError> {
    let dir = data_dir(cli)?;
    dir.require_exists()?;
    let identity = datadir::load_key(&dir)?;
    let fp = identity.fingerprint();
    match connect {
        None => println!("{fp}"),
        Some(host) => {
            let cfg = load_config(cli, &dir)?;
            if cfg.server.listen.is_empty() {
                return Err(ServerError::Usage(
                    "no [server] listen addresses configured".into(),
                ));
            }
            let host = if host.contains(':') && !host.starts_with('[') {
                format!("[{host}]")
            } else {
                host.to_owned()
            };
            // One connect string per configured port, in configuration order.
            let mut ports: Vec<u16> = Vec::new();
            for a in &cfg.server.listen {
                if !ports.contains(&a.port()) {
                    ports.push(a.port());
                }
            }
            for p in ports {
                println!("tessera://{host}:{p}?pin={fp}");
            }
        }
    }
    Ok(exit::OK)
}

// ---------------------------------------------------------------------------- passwords

/// Reads a line from standard input without echo (Unix terminals).
fn read_hidden(prompt: &str) -> Result<String, ServerError> {
    eprint!("{prompt}");
    let _ = std::io::stderr().flush();
    #[cfg(unix)]
    let restore = {
        let stdin = std::io::stdin();
        match rustix::termios::tcgetattr(&stdin) {
            Ok(old) => {
                let mut t = old.clone();
                t.local_modes.remove(rustix::termios::LocalModes::ECHO);
                let _ =
                    rustix::termios::tcsetattr(&stdin, rustix::termios::OptionalActions::Now, &t);
                Some(old)
            }
            Err(_) => None,
        }
    };
    let mut line = String::new();
    let r = std::io::stdin().lock().read_line(&mut line);
    #[cfg(unix)]
    if let Some(old) = restore {
        let _ = rustix::termios::tcsetattr(
            std::io::stdin(),
            rustix::termios::OptionalActions::Now,
            &old,
        );
    }
    eprintln!();
    r.map_err(|e| ServerError::Runtime(format!("reading the password: {e}")))?;
    strip_newline(&mut line);
    Ok(line)
}

/// Removes one trailing LF or CRLF (§2.7); nothing else is trimmed.
fn strip_newline(s: &mut String) {
    if s.ends_with('\n') {
        s.pop();
        if s.ends_with('\r') {
            s.pop();
        }
    }
}

/// Gets a new password per §2.7; returns it and whether it was generated (to print once).
fn new_password(src: &PasswordSource) -> Result<(String, bool), ServerError> {
    if src.generate {
        return Ok((accounts::generate_password()?, true));
    }
    if src.password_stdin {
        let mut line = String::new();
        std::io::stdin()
            .lock()
            .take(1024 * 1024)
            .read_line(&mut line)
            .map_err(|e| ServerError::Runtime(format!("reading standard input: {e}")))?;
        strip_newline(&mut line);
        return Ok((line, false));
    }
    if !std::io::stdin().is_terminal() {
        return Err(ServerError::Usage(
            "no terminal: use --password-stdin or --generate (passwords are never taken from the command line)".into(),
        ));
    }
    let a = read_hidden("New password: ")?;
    let b = read_hidden("Repeat the password: ")?;
    if a != b {
        accounts::wipe(b);
        accounts::wipe(a);
        return Err(ServerError::Usage("the two entries differ".into()));
    }
    accounts::wipe(b);
    Ok((a, false))
}

fn confirm(question: &str, yes: bool) -> Result<bool, ServerError> {
    if yes {
        return Ok(true);
    }
    if !std::io::stdin().is_terminal() {
        return Err(ServerError::Usage(
            "no terminal to confirm on: pass --yes".into(),
        ));
    }
    eprint!("{question} [y/N] ");
    let _ = std::io::stderr().flush();
    let mut line = String::new();
    let _ = std::io::stdin().lock().read_line(&mut line);
    Ok(matches!(line.trim(), "y" | "Y" | "yes" | "Yes"))
}

// ---------------------------------------------------------------------------- accounts

fn canonical(name: &str) -> Result<String, ServerError> {
    names::canonical_account(name).ok_or_else(|| {
        ServerError::Usage(format!(
            "\"{name}\" is not an account name (1–32 ASCII letters, digits, '.', '_', '-')"
        ))
    })
}

fn set_flag(
    dir: &DataDir,
    name: &str,
    f: impl FnOnce(&mut accounts::Account),
) -> Result<u8, ServerError> {
    let name = canonical(name)?;
    accounts::modify_accounts(dir, |file| {
        let a = file
            .accounts
            .get_mut(&name)
            .ok_or_else(|| ServerError::Usage(format!("no account \"{name}\"")))?;
        f(a);
        Ok(())
    })?;
    Ok(exit::OK)
}

fn cmd_account(cli: &Cli, cmd: &AccountCmd) -> Result<u8, ServerError> {
    let dir = data_dir(cli)?;
    match cmd {
        AccountCmd::Add {
            name,
            password,
            disabled,
        } => {
            let name = canonical(name)?;
            dir.create()?;
            let cfg = load_config(cli, &dir)?;
            if accounts::read_accounts(&dir)?.accounts.contains_key(&name) {
                return Err(ServerError::Usage(format!("account \"{name}\" exists")));
            }
            let (pw, generated) = new_password(password)?;
            if !accounts::password_acceptable(&pw, &name, cfg.accounts.password_min_bytes) {
                accounts::wipe(pw);
                return Err(ServerError::Usage(format!(
                    "password refused: it must be {}–256 bytes and differ from the account name",
                    cfg.accounts.password_min_bytes
                )));
            }
            let hash =
                accounts::hash_password(pw.as_bytes(), HashParams::from_config(&cfg.accounts))?;
            let disabled = *disabled;
            let n = name.clone();
            accounts::modify_accounts(&dir, move |f| {
                if f.accounts.contains_key(&n) {
                    return Err(ServerError::Usage(format!("account \"{n}\" exists")));
                }
                let mut a = accounts::new_account(n.clone(), hash);
                a.disabled = disabled;
                f.accounts.insert(n, a);
                Ok(())
            })?;
            if generated {
                println!("{pw}");
            }
            accounts::wipe(pw);
            eprintln!("account \"{name}\" created");
            Ok(exit::OK)
        }
        AccountCmd::List => {
            dir.require_exists()?;
            let file = accounts::read_accounts(&dir)?;
            for a in file.accounts.values() {
                let chars = characters::read_file(&dir, &a.name)
                    .map(|f| f.characters.len())
                    .unwrap_or(0);
                let mut flags = Vec::new();
                if a.disabled {
                    flags.push("disabled");
                }
                if a.muted {
                    flags.push("muted");
                }
                println!(
                    "{}\t{}\t{}\t{} character{}",
                    a.name,
                    a.created,
                    if flags.is_empty() {
                        "-".to_owned()
                    } else {
                        flags.join(",")
                    },
                    chars,
                    if chars == 1 { "" } else { "s" }
                );
            }
            Ok(exit::OK)
        }
        AccountCmd::Passwd { name, password } => {
            dir.require_exists()?;
            let name = canonical(name)?;
            let cfg = load_config(cli, &dir)?;
            if !accounts::read_accounts(&dir)?.accounts.contains_key(&name) {
                return Err(ServerError::Usage(format!("no account \"{name}\"")));
            }
            let (pw, generated) = new_password(password)?;
            if !accounts::password_acceptable(&pw, &name, cfg.accounts.password_min_bytes) {
                accounts::wipe(pw);
                return Err(ServerError::Usage(format!(
                    "password refused: it must be {}–256 bytes and differ from the account name",
                    cfg.accounts.password_min_bytes
                )));
            }
            let hash =
                accounts::hash_password(pw.as_bytes(), HashParams::from_config(&cfg.accounts))?;
            set_flag(&dir, &name, move |a| a.hash = hash)?;
            if generated {
                println!("{pw}");
            }
            accounts::wipe(pw);
            Ok(exit::OK)
        }
        AccountCmd::Disable { name } => {
            dir.require_exists()?;
            set_flag(&dir, name, |a| a.disabled = true)
        }
        AccountCmd::Enable { name } => {
            dir.require_exists()?;
            set_flag(&dir, name, |a| a.disabled = false)
        }
        AccountCmd::Mute { name } => {
            dir.require_exists()?;
            set_flag(&dir, name, |a| a.muted = true)
        }
        AccountCmd::Unmute { name } => {
            dir.require_exists()?;
            set_flag(&dir, name, |a| a.muted = false)
        }
        AccountCmd::Remove { name, yes } => {
            dir.require_exists()?;
            let name = canonical(name)?;
            let _lock = ServerLock::acquire(&dir, false)?;
            if !accounts::read_accounts(&dir)?.accounts.contains_key(&name) {
                return Err(ServerError::Usage(format!("no account \"{name}\"")));
            }
            if !confirm(
                &format!("Remove account \"{name}\" and its characters?"),
                *yes,
            )? {
                eprintln!("not removed");
                return Ok(exit::OK);
            }
            accounts::modify_accounts(&dir, |f| {
                f.accounts.remove(&name);
                Ok(())
            })?;
            let cpath = dir.character_path(&name);
            match std::fs::remove_file(&cpath) {
                Ok(()) => {}
                Err(e) if e.kind() == std::io::ErrorKind::NotFound => {}
                Err(e) => return Err(ServerError::io(cpath, e)),
            }
            eprintln!("account \"{name}\" removed");
            Ok(exit::OK)
        }
    }
}

// ---------------------------------------------------------------------------- characters

fn find_character(
    dir: &DataDir,
    name: &str,
) -> Result<(characters::CharacterFile, usize), ServerError> {
    let key = names::name_key(name);
    for (_, file) in characters::load_all(dir)? {
        if let Some(i) = file
            .characters
            .iter()
            .position(|c| names::name_key(&c.name) == key)
        {
            return Ok((file, i));
        }
    }
    Err(ServerError::Usage(format!("no character \"{name}\"")))
}

fn write_character_file(
    dir: &DataDir,
    file: &characters::CharacterFile,
) -> Result<(), ServerError> {
    let path = dir.character_path(&file.account);
    datadir::create_private_dir(&dir.characters_dir())?;
    datadir::atomic_write(&path, file.to_toml().as_bytes(), 0o600)
        .map_err(|e| ServerError::io(path, e))
}

fn cmd_character(cli: &Cli, cmd: &CharacterCmd) -> Result<u8, ServerError> {
    let dir = data_dir(cli)?;
    dir.require_exists()?;
    match cmd {
        CharacterCmd::List { account } => {
            let filter = account.as_deref().map(canonical).transpose()?;
            for (acct, file) in characters::load_all(&dir)? {
                if filter.as_ref().is_some_and(|f| *f != acct) {
                    continue;
                }
                for c in &file.characters {
                    let (region, block) = match &c.position {
                        Some(p) => {
                            let (bx, by) = tessera_protocol::movement::interest_block(
                                &p.location,
                                p.anchor.as_ref(),
                            );
                            (p.location.region.to_string(), format!("({bx}, {by})"))
                        }
                        None => ("-".into(), "-".into()),
                    };
                    println!(
                        "{}\t{}\t{}\t{}\t{}",
                        c.name, acct, c.instance, region, block
                    );
                }
            }
            Ok(exit::OK)
        }
        CharacterCmd::Remove { name, yes } => {
            let _lock = ServerLock::acquire(&dir, false)?;
            let (mut file, i) = find_character(&dir, name)?;
            let shown = file.characters[i].name.clone();
            if !confirm(&format!("Remove character \"{shown}\"?"), *yes)? {
                eprintln!("not removed");
                return Ok(exit::OK);
            }
            file.characters.remove(i);
            write_character_file(&dir, &file)?;
            eprintln!("character \"{shown}\" removed");
            Ok(exit::OK)
        }
        CharacterCmd::Move { name, spawn } => {
            let _lock = ServerLock::acquire(&dir, false)?;
            let cfg = load_config(cli, &dir)?;
            let content = load_content(&dir, cfg.server.content.as_deref())?;
            let s = content
                .spawn_named(spawn)
                .ok_or_else(|| ServerError::Usage(format!("no spawn point \"{spawn}\"")))?;
            let (mut file, i) = find_character(&dir, name)?;
            file.characters[i].position = Some(StoredPosition {
                location: s.location,
                heading: s.heading,
                anchor: s.anchor,
            });
            write_character_file(&dir, &file)?;
            eprintln!(
                "character \"{}\" moved to \"{}\"",
                file.characters[i].name, s.name
            );
            Ok(exit::OK)
        }
    }
}

// ---------------------------------------------------------------------------- check-content

/// Runs `check-content` and returns the output lines and the exit status (0 or 3).
#[must_use]
pub fn check_content_report(
    files: &[PathBuf],
    config: Option<(PathBuf, bool)>,
    data_dir: Option<&DataDir>,
) -> (Vec<String>, u8) {
    let mut findings: Vec<(String, Finding)> = Vec::new();
    let mut checked = 0usize;
    if files.is_empty() {
        let mut configured = None;
        if let Some((path, explicit)) = &config {
            match Config::load(path, *explicit) {
                Ok((cfg, f)) => {
                    let shown = path.display().to_string();
                    for x in f {
                        findings.push((shown.clone(), x));
                    }
                    configured = cfg.server.content.clone();
                }
                Err(e) => findings.push((
                    path.display().to_string(),
                    Finding::new(
                        0,
                        Rule::Config,
                        format!("cannot read the configuration: {e}"),
                    ),
                )),
            }
            checked += 1;
        }
        match configured {
            None => {
                let (_, f) = content::check_text(content::DEFAULT_WORLD, true);
                for x in f {
                    findings.push((content::DEFAULT_WORLD_NAME.to_owned(), x));
                }
            }
            Some(p) => {
                let path = data_dir.map_or_else(|| p.clone(), |d| d.resolve(&p));
                let (_, f) = content::check_file(&path, true);
                for x in f {
                    findings.push((path.display().to_string(), x));
                }
            }
        }
        checked += 1;
    } else {
        for p in files {
            let (_, f) = content::check_file(p, true);
            for x in f {
                findings.push((p.display().to_string(), x));
            }
            checked += 1;
        }
    }
    findings.sort_by(|a, b| {
        a.0.cmp(&b.0)
            .then(a.1.line.cmp(&b.1.line))
            .then(a.1.rule.cmp(&b.1.rule))
    });
    let mut lines: Vec<String> = findings
        .iter()
        .map(|(file, f)| format!("{file}:{}: {}: {}", f.line, f.rule, f.message))
        .collect();
    lines.push(format!(
        "{} finding{} in {checked} file{}",
        findings.len(),
        if findings.len() == 1 { "" } else { "s" },
        if checked == 1 { "" } else { "s" }
    ));
    (
        lines,
        if findings.is_empty() {
            exit::OK
        } else {
            exit::CONTENT
        },
    )
}

fn cmd_check_content(cli: &Cli, files: &[PathBuf]) -> Result<u8, ServerError> {
    let (config, dir) = if files.is_empty() {
        let dir = data_dir(cli)?;
        let c = config_path(cli, &dir);
        (Some(c), Some(dir))
    } else {
        (None, None)
    };
    let (lines, status) = check_content_report(files, config, dir.as_ref());
    for l in lines {
        println!("{l}");
    }
    Ok(status)
}

#[cfg(test)]
mod tests {
    use super::*;
    use clap::CommandFactory;

    #[test]
    fn cli_definition() {
        Cli::command().debug_assert();
        let help = Cli::command().render_long_help().to_string();
        assert!(help.contains("private"), "{help}");
        assert!(help.contains("legally obtained installation"));
    }

    #[test]
    fn levels_and_newlines() {
        assert_eq!(level_name("info", 1, 0), "debug");
        assert_eq!(level_name("info", 0, 5), "error");
        let mut s = "pw\r\n".to_owned();
        strip_newline(&mut s);
        assert_eq!(s, "pw");
        let mut s = "pw \n\n".to_owned();
        strip_newline(&mut s);
        assert_eq!(s, "pw \n");
    }

    #[test]
    fn built_in_check() {
        let (lines, status) = check_content_report(&[], None, None);
        assert_eq!(status, 0, "{lines:?}");
        assert_eq!(lines.last().unwrap(), "0 findings in 1 file");
    }
}
