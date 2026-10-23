// SPDX-License-Identifier: GPL-3.0-or-later
//! One load run (`test-server.md` §13): start or reach the server, create the bots'
//! characters, drive the bots, sample desync, chat, traffic and memory, optionally run the
//! hostile clients, then let everyone leave and check that the server returns to its
//! baseline.

use std::collections::HashMap;
use std::net::{SocketAddr, ToSocketAddrs};
use std::path::PathBuf;
use std::sync::Once;
use std::sync::atomic::{AtomicU64, Ordering};
use std::time::{Duration, Instant};

use tessera_net::ChatChannel;
use tessera_net::remote::{
    EVENT_CHANNEL_CAPACITY, PENDING_PAUSE, Password, READ_CHUNK, ServerInfo,
};
use tessera_protocol::connect::{ConnectString, Host};
use tessera_server::accounts::{self, HashParams};
use tessera_server::{
    Config, Content, DataDir, Server, ServerHandle, ServerOptions, StatsSnapshot,
};

use crate::LoadtestError;
use crate::bot::{
    Area, Behaviour, Bot, BotSpec, Ctx, Endpoint, MAX_ENTITIES, MIN_CHANNEL_CYCLE, TimeBase,
};
use crate::chat::{ChatLedger, Kind as ChatKind};
use crate::desync::{DesyncConfig, DesyncTracker, Kind, SAMPLE_INTERVAL};
use crate::hostile::{HostileConfig, HostileRun, MAX_PACE};
use crate::leak::{Bounds, LeakMonitor, LeakSample, baseline_differences};
use crate::raw::{self, Target};
use crate::report::{BaselineResult, Report, RunParams, ToolLoad};
use crate::rng::Rng;
use crate::sysinfo::{self, Machine};

/// The content the in-process server serves: one spawn in the middle of the grid of a
/// world without data (`test-server.md` §5.7 `void`), no wanderers, no opaque references.
pub const LOADTEST_CONTENT: &str = "\
# SPDX-License-Identifier: GPL-3.0-or-later
#
# Tessera load-test world (apps/loadtest): original project content. One spawn in the middle
# of the grid of a world without data; no game text and no opaque references.
format = 1

[world]
id = \"loadtest\"
title = \"Tessera load-test world\"
motd = \"Tessera load test. This server sends no game content.\"
default_spawn = \"void\"

[[spawn]]
name = \"void\"
region = 0
block = [127, 127]
offset = [80.0, 80.0, 0.0]
ground = false

[clock]
start = 0.0
rate = 1.0
";

/// The area's centre block and region (the `void` spawn's block).
const AREA_BLOCK: (u8, u8) = (127, 127);

/// The load scenarios of `test-server.md` §13.1.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Scenario {
    /// Everyone connects at once, then walks.
    ConnectStorm,
    /// Walk and run on random paths with stops and turns.
    Walk,
    /// Walk and chat often.
    Chat,
    /// Walk, chat, and reconnect often.
    Churn,
    /// Some bots send moves far above the limits; each must be corrected or dropped.
    Cheat,
    /// The long mix (default 1 h).
    Soak,
}

impl Scenario {
    /// Every scenario.
    pub const ALL: [Self; 6] = [
        Self::ConnectStorm,
        Self::Walk,
        Self::Chat,
        Self::Churn,
        Self::Cheat,
        Self::Soak,
    ];

    /// Its name on the command line.
    #[must_use]
    pub fn name(self) -> &'static str {
        match self {
            Self::ConnectStorm => "connect-storm",
            Self::Walk => "walk",
            Self::Chat => "chat",
            Self::Churn => "churn",
            Self::Cheat => "cheat",
            Self::Soak => "soak",
        }
    }

    /// The scenario named `s`.
    #[must_use]
    pub fn parse(s: &str) -> Option<Self> {
        Self::ALL.into_iter().find(|x| x.name() == s)
    }

    /// The default run length.
    #[must_use]
    pub fn default_duration(self) -> Duration {
        match self {
            Self::Soak => Duration::from_secs(3_600),
            Self::ConnectStorm => Duration::from_secs(30),
            _ => Duration::from_secs(60),
        }
    }

    /// The bots' behaviour.
    #[must_use]
    pub fn behaviour(self) -> Behaviour {
        let quiet = Behaviour {
            walk_fraction: 1.0,
            chat_interval: None,
            teleport_interval: None,
            reconnect_interval: None,
            appearance_interval: None,
            ..Behaviour::default()
        };
        let s = Duration::from_secs;
        match self {
            Self::Soak => Behaviour::default(),
            Self::Walk | Self::ConnectStorm => quiet,
            // Bots at minor 2 also join one of three shared channels, leave and re-join it
            // now and then, and speak on it (test-server.md §13.1 `chat`).
            Self::Chat => Behaviour {
                walk_fraction: 0.5,
                chat_interval: Some(s(3)),
                channels: 3,
                channel_cycle: Some(s(20)),
                ..quiet
            },
            Self::Churn => Behaviour {
                chat_interval: Some(s(10)),
                reconnect_interval: Some(s(20)),
                reconnect_fraction: 0.25,
                ..quiet
            },
            Self::Cheat => Behaviour {
                cheat_interval: Some(s(5)),
                cheat_fraction: 0.25,
                ..quiet
            },
        }
    }
}

/// Where the bots connect.
#[derive(Debug, Clone)]
pub enum TargetOptions {
    /// An in-process server on loopback, with a temporary data directory unless one is
    /// given (accounts and characters are created).
    InProcess {
        /// Data directory to use instead of a temporary one.
        data_dir: Option<PathBuf>,
    },
    /// A remote `tessera-server`.
    Remote {
        /// `tessera://host:port?pin=…` (the pin is required).
        connect: String,
        /// Every bot account's password (its `Debug` is redacted).
        password: Password,
        /// Create the accounts with `Register` (the server must allow it).
        register: bool,
    },
}

/// Hostile-mode options.
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct HostileOptions {
    /// Concurrent attackers.
    pub clients: usize,
    /// Pause between two attacks of one attacker.
    pub pace: Duration,
}

/// Everything a run needs.
#[derive(Debug, Clone)]
pub struct RunOptions {
    /// Scenario (for the report; the behaviour is in `behaviour`).
    pub scenario: Scenario,
    /// Number of bots (1–1,024).
    pub bots: usize,
    /// Length of the measured run.
    pub duration: Duration,
    /// Server.
    pub target: TargetOptions,
    /// Bot behaviour.
    pub behaviour: Behaviour,
    /// Side of the walking area in landblocks (1–16). At least *R* + 3 lets interest leave
    /// happen at all, 2*R* + 4 (the default, 8 for *R* = 2) lets every far teleport leave
    /// *R* + 1 blocks.
    pub area_blocks: u32,
    /// Account names are this plus a four-digit number.
    pub account_prefix: String,
    /// Character names are this plus three letters (letters only).
    pub name_prefix: String,
    /// Hostile clients, if any.
    pub hostile: Option<HostileOptions>,
    /// Leak sampling interval.
    pub sample_interval: Duration,
    /// Smallest memory warm-up.
    pub warmup_min: Duration,
    /// Time between two bots' first connections.
    pub stagger: Duration,
    /// Seed.
    pub seed: u64,
    /// Wait for the server's throttle tables to empty after the run (`lockout_max_s` +
    /// 60 s; in-process only).
    pub check_throttle: bool,
    /// The operator states this is the reference machine under quiet conditions.
    pub reference_machine: bool,
    /// PID of a separate server process on this machine whose resident memory is sampled
    /// and judged (remote runs against a local `tessera-server`); the tool's own memory,
    /// which then holds only the bots, is reported but not judged.
    pub server_pid: Option<u32>,
    /// The remote server's `[chat] say_range_m` (in-process: taken from its
    /// configuration). Unknown: Say lines are judged only for the speaker's echo.
    pub say_range: Option<f64>,
    /// Use an in-process `data_dir` that already holds an accounts file. The bot accounts
    /// in it (`account_prefix` + number) get new random passwords.
    pub reuse_data_dir: bool,
}

/// The largest `[chat] max_channels_per_character` a server may hold (`test-server.md`
/// §4.6 range), which bounds the memberships of every character.
const MAX_CHANNELS_PER_CHARACTER: usize = 32;
/// Most shared channels a run uses.
pub const MAX_CHANNELS: u8 = 16;
/// Characters the hostile clients log in with for the cases that must be sent in the world
/// (`test-server.md` §13.1 "Hostile 1.2 cases"): accounts and characters numbered after the
/// bots' (`account_prefix` + number, `name_prefix` + three letters).
pub const HOSTILE_CHARACTERS: usize = 4;

/// Longest duration any option may hold (the command line's limit, about 116 days).
pub const MAX_OPTION_SECS: f64 = 1.0e7;
/// The largest number of bots a run accepts.
pub const MAX_BOTS: usize = 1_024;
/// Default side of the walking area, blocks (2*R* + 4 for the default *R* = 2).
pub const DEFAULT_AREA_BLOCKS: u32 = 8;

impl RunOptions {
    /// Defaults for a scenario against an in-process server.
    #[must_use]
    pub fn new(scenario: Scenario, bots: usize) -> Self {
        let duration = scenario.default_duration();
        Self {
            scenario,
            bots,
            duration,
            target: TargetOptions::InProcess { data_dir: None },
            behaviour: scenario.behaviour(),
            area_blocks: DEFAULT_AREA_BLOCKS,
            account_prefix: "bot-".into(),
            name_prefix: "Bot".into(),
            hostile: None,
            sample_interval: Duration::from_secs(10),
            warmup_min: Duration::from_secs(300),
            stagger: if scenario == Scenario::ConnectStorm {
                Duration::ZERO
            } else {
                Duration::from_millis(50)
            },
            seed: 0x7E55_E2A0,
            check_throttle: duration >= Duration::from_secs(300),
            reference_machine: false,
            server_pid: None,
            say_range: None,
            reuse_data_dir: false,
        }
    }

    fn validate(&self) -> Result<(), LoadtestError> {
        let bad = |m: &str| Err(LoadtestError::Usage(m.into()));
        if !(1..=MAX_BOTS).contains(&self.bots) {
            return bad("--bots must be 1-1024");
        }
        if self.duration.is_zero() {
            return bad("--duration must be positive");
        }
        let long = |d: Duration| d.as_secs_f64() > MAX_OPTION_SECS;
        if long(self.duration)
            || long(self.sample_interval)
            || long(self.warmup_min)
            || long(self.stagger)
            || [
                self.behaviour.chat_interval,
                self.behaviour.teleport_interval,
                self.behaviour.reconnect_interval,
                self.behaviour.appearance_interval,
                self.behaviour.cheat_interval,
            ]
            .into_iter()
            .flatten()
            .any(long)
        {
            return bad("durations and intervals must be at most 1e7 s");
        }
        // The last bot starts at `stagger × (bots − 1)`; the run must still end in range.
        if self
            .stagger
            .checked_mul(u32::try_from(self.bots).unwrap_or(u32::MAX))
            .is_none_or(long)
        {
            return bad("--stagger × --bots must be at most 1e7 s");
        }
        if !(1..=16).contains(&self.area_blocks) {
            return bad("--area-blocks must be 1-16");
        }
        if self.name_prefix.is_empty()
            || self.name_prefix.len() > 20
            || !self.name_prefix.chars().all(|c| c.is_ascii_alphabetic())
        {
            return bad("--name-prefix must be 1-20 ASCII letters");
        }
        let account = format!("{}0001", self.account_prefix);
        if account.len() > 32
            || !account
                .chars()
                .all(|c| c.is_ascii_alphanumeric() || matches!(c, '.' | '_' | '-'))
        {
            return bad("--account-prefix must be at most 28 of A-Z a-z 0-9 . _ -");
        }
        if self.sample_interval < Duration::from_millis(100) {
            return bad("--sample-interval must be at least 0.1 s");
        }
        let b = &self.behaviour;
        if !(0.0..=1.0).contains(&b.walk_fraction)
            || !(0.0..=1.0).contains(&b.run_fraction)
            || !(0.0..=1.0).contains(&b.reconnect_fraction)
            || !(0.0..=1.0).contains(&b.cheat_fraction)
            || !(0.0..=1.0).contains(&b.far_teleport_fraction)
            || !(0.0..=1.0).contains(&b.channel_fraction)
        {
            return bad("fractions must be within 0-1");
        }
        let speed_ok = |v: f64| v.is_finite() && v > 0.0 && v <= 100.0;
        if !speed_ok(b.walk_speed) || !speed_ok(b.run_speed) || b.walk_speed > b.run_speed {
            return bad("walk and run speeds must be within (0, 100] m/s, walk ≤ run");
        }
        if !b.truth_bias_m.is_finite() || b.truth_bias_m.abs() > 1_000.0 {
            return bad("the truth bias must be within ±1000 m");
        }
        if b.channels > MAX_CHANNELS {
            return bad("--channels must be 0-16");
        }
        if b.channel_cycle
            .is_some_and(|c| c < MIN_CHANNEL_CYCLE || long(c))
        {
            return bad("--channel-cycle must be at least 15 s");
        }
        if let Some(h) = self.hostile {
            if !(1..=256).contains(&h.clients) {
                return bad("--hostile-clients must be 1-256");
            }
            if h.pace > MAX_PACE {
                return bad("--hostile-pace must be at most 10 s");
            }
        }
        if let Some(r) = self.say_range
            && !(r.is_finite() && (1.0..=1_000.0).contains(&r))
        {
            return bad("--say-range must be 1-1000 m");
        }
        Ok(())
    }
}

static PANICS: AtomicU64 = AtomicU64::new(0);
static HOOK: Once = Once::new();

/// Installs a panic hook (once per process) that counts every panic in any thread (the
/// in-process server's threads included) before the previous hook runs.
pub fn install_panic_counter() {
    HOOK.call_once(|| {
        let previous = std::panic::take_hook();
        std::panic::set_hook(Box::new(move |info| {
            PANICS.fetch_add(1, Ordering::SeqCst);
            previous(info);
        }));
    });
}

/// Panics counted so far in this process.
#[must_use]
pub fn panics() -> u64 {
    PANICS.load(Ordering::SeqCst)
}

/// The character name of bot `i`: the prefix plus three letters.
#[must_use]
pub fn character_name(prefix: &str, i: usize) -> String {
    let mut s = prefix.to_owned();
    let mut k = i;
    let mut letters = [b'a'; 3];
    for l in letters.iter_mut().rev() {
        *l = b'a' + (k % 26) as u8;
        k /= 26;
    }
    s.extend(letters.iter().map(|b| char::from(*b)));
    s
}

/// The account name of bot `i`.
#[must_use]
pub fn account_name(prefix: &str, i: usize) -> String {
    format!("{prefix}{:04}", i + 1)
}

/// A temporary directory removed on drop.
#[derive(Debug)]
struct TempDir(PathBuf);

impl TempDir {
    fn create() -> Result<Self, LoadtestError> {
        let nanos = std::time::SystemTime::now()
            .duration_since(std::time::UNIX_EPOCH)
            .map_or(0, |d| d.as_nanos());
        let p =
            std::env::temp_dir().join(format!("tessera-loadtest-{}-{nanos:x}", std::process::id()));
        std::fs::create_dir_all(&p).map_err(|e| LoadtestError::io(&p, e))?;
        Ok(Self(p))
    }
}

impl Drop for TempDir {
    fn drop(&mut self) {
        let _ = std::fs::remove_dir_all(&self.0);
    }
}

/// The in-process server's configuration for `bots` bots.
#[must_use]
pub fn in_process_config(bots: usize, hostile: Option<HostileOptions>) -> Config {
    let mut c = Config::default();
    c.server.listen = vec![SocketAddr::from(([127, 0, 0, 1], 0))];
    c.server.name = "Tessera load test".into();
    // §13.1: Argon2id at the configuration floor.
    c.use_hash_floor();
    c.accounts.hash_workers = 2;
    c.accounts.attempts_per_address_per_min = 10_000;
    c.accounts.lockout_base_s = 1;
    c.accounts.lockout_max_s = 30;
    let hostile_chars = hostile.map_or(0, |_| HOSTILE_CHARACTERS);
    c.world.instance_capacity =
        u32::try_from((bots + hostile_chars).clamp(64, 1_024)).unwrap_or(1_024);
    let hostile = hostile.map_or(0, |h| h.clients * 8);
    c.limits.max_connections = u32::try_from((bots + hostile + 64).min(4_096)).unwrap_or(4_096);
    c.limits.max_handshakes = 128;
    c.log.stats_interval_s = 0;
    c
}

fn resolve(cs: &ConnectString) -> Result<(SocketAddr, String), LoadtestError> {
    let port = cs.server.port;
    let name = tessera_protocol::transport::server_name(&cs.server.host);
    let addr = match &cs.server.host {
        Host::Ipv4(a) => SocketAddr::from((*a, port)),
        Host::Ipv6(a) => SocketAddr::from((*a, port)),
        Host::Dns(n) => (n.as_str(), port)
            .to_socket_addrs()
            .map_err(|e| LoadtestError::Usage(format!("cannot resolve {n}: {e}")))?
            .next()
            .ok_or_else(|| LoadtestError::Usage(format!("{n} has no address")))?,
    };
    Ok((addr, name))
}

/// Accounts provisioned at the same time. `tessera-server` refuses a fifth QUIC handshake in
/// progress from one source address that is not loopback (`test-server.md` §15.1), so a
/// larger batch fails against a server on a LAN address.
const PROVISION_PARALLEL: usize = 4;

fn provision_all(
    target: &Target,
    specs: &[BotSpec],
    register: bool,
) -> Result<(usize, usize), LoadtestError> {
    let rt = tokio::runtime::Builder::new_current_thread()
        .enable_all()
        .build()
        .map_err(|e| LoadtestError::Runtime(e.to_string()))?;
    rt.block_on(async {
        let bind = if target.addr.is_ipv4() {
            SocketAddr::from(([0, 0, 0, 0], 0))
        } else {
            SocketAddr::from(([0u16; 8], 0))
        };
        let endpoint = raw::endpoint(bind).map_err(LoadtestError::Runtime)?;
        let (mut created, mut existing) = (0, 0);
        for chunk in specs.chunks(PROVISION_PARALLEL) {
            let mut tasks = Vec::new();
            for s in chunk {
                let (ep, t, s) = (endpoint.clone(), target.clone(), s.clone());
                tasks.push(tokio::spawn(async move {
                    let r = raw::provision(
                        &ep,
                        &t,
                        &s.account,
                        s.password.expose(),
                        &s.character,
                        register,
                        (raw::bot_appearance(s.index), raw::bot_look(s.index)),
                    )
                    .await;
                    (s.account, r)
                }));
            }
            for t in tasks {
                let (account, r) = t.await.map_err(|e| LoadtestError::Runtime(e.to_string()))?;
                match r {
                    Ok(raw::Provisioned::Created) => created += 1,
                    Ok(raw::Provisioned::Existing) => existing += 1,
                    Err(reason) => return Err(LoadtestError::Provision { account, reason }),
                }
            }
        }
        endpoint.close(0u32.into(), b"");
        let _ = tokio::time::timeout(Duration::from_secs(2), endpoint.wait_idle()).await;
        Ok((created, existing))
    })
}

/// Waits (at most `limit`) until `pred` holds for the server's counters; returns the last
/// snapshot and whether it held.
fn wait_stats(
    h: &ServerHandle,
    limit: Duration,
    pred: impl Fn(&StatsSnapshot) -> bool,
) -> (StatsSnapshot, bool) {
    let deadline = Instant::now() + limit;
    loop {
        let s = h.stats();
        if pred(&s) {
            return (s, true);
        }
        if Instant::now() >= deadline {
            return (s, false);
        }
        std::thread::sleep(Duration::from_millis(50));
    }
}

/// Runs the load test and returns its report (whose [`Report::passed`] decides the exit
/// status).
///
/// # Errors
///
/// [`LoadtestError`] when the run cannot start: bad options, a server that does not start,
/// characters that cannot be created.
pub fn run(opts: &RunOptions) -> Result<Report, LoadtestError> {
    opts.validate()?;
    install_panic_counter();
    let panics_before = panics();
    let machine = Machine::detect();
    let started_wall = std::time::SystemTime::now();
    let mut rng = Rng::new(opts.seed);

    // The server.
    let mut temp = None;
    let mut handle: Option<ServerHandle> = None;
    let (connect, target, password, register, mut params) = match &opts.target {
        TargetOptions::InProcess { data_dir } => {
            let dir = match data_dir {
                Some(d) => {
                    let accounts = DataDir::new(d.clone()).accounts_path();
                    if accounts.exists() && !opts.reuse_data_dir {
                        return Err(LoadtestError::Usage(format!(
                            "{} already has an accounts file; the run would give the bot \
                             accounts in it new passwords (pass --reuse-data-dir to allow \
                             that)",
                            d.display()
                        )));
                    }
                    d.clone()
                }
                None => {
                    let t = TempDir::create()?;
                    let p = t.0.join("data");
                    temp = Some(t);
                    p
                }
            };
            let mut config = in_process_config(opts.bots, opts.hostile);
            if let Some(r) = opts.say_range {
                config.chat.say_range_m = r;
            }
            let content = Content::parse(LOADTEST_CONTENT)
                .map_err(|f| LoadtestError::Runtime(format!("built-in content rejected: {f:?}")))?;
            let data = DataDir::new(dir);
            data.create()?;
            let password =
                Password::new(format!("lt-{:016x}{:016x}", rng.next_u64(), rng.next_u64()));
            let names: Vec<String> = (0..opts.bots + hostile_characters(opts))
                .map(|i| account_name(&opts.account_prefix, i))
                .collect();
            let entries: Vec<(&str, &str)> = names
                .iter()
                .map(|n| (n.as_str(), password.expose()))
                .collect();
            accounts::add_accounts(&data, HashParams::from_config(&config.accounts), &entries)?;
            let params = RunParams {
                server_version: format!(
                    "tessera-server {} (in-process, same build)",
                    env!("CARGO_PKG_VERSION")
                ),
                interest_radius: u32::from(config.world.interest_radius),
                tick_rate: 30,
                snapshot_rate: u32::from(config.world.snapshot_rate),
                creates_per_tick: config.world.creates_per_tick,
                instance_capacity: Some(config.world.instance_capacity),
                say_range: Some(config.chat.say_range_m),
                walk_speed: Some(f64::from(config.movement.walk_speed)),
                run_speed: Some(f64::from(config.movement.run_speed)),
                wanderers: Some(0),
            };
            let h = Server::start(ServerOptions {
                data_dir: data,
                config,
                content,
            })?;
            let addr = h.local_addr();
            let target = Target {
                addr,
                server_name: addr.ip().to_string(),
                fingerprint: h.fingerprint(),
            };
            let connect = h.connect_string();
            handle = Some(h);
            (connect, target, password, false, params)
        }
        TargetOptions::Remote {
            connect,
            password,
            register,
        } => {
            let cs = ConnectString::parse(connect)
                .map_err(|e| LoadtestError::Usage(format!("connect string: {e:?}")))?;
            let Some(fingerprint) = cs.pin else {
                return Err(LoadtestError::Usage(
                    "the server's fingerprint is required (?pin= or --fingerprint)".into(),
                ));
            };
            let (addr, server_name) = resolve(&cs)?;
            let target = Target {
                addr,
                server_name,
                fingerprint,
            };
            // Assumed until the first HelloAck (§17 defaults); replaced then.
            let params = RunParams {
                server_version: "remote (not yet known)".into(),
                interest_radius: 2,
                tick_rate: 30,
                snapshot_rate: 15,
                creates_per_tick: 16,
                instance_capacity: None,
                say_range: opts.say_range,
                walk_speed: None,
                run_speed: None,
                wanderers: None,
            };
            (connect.clone(), target, password.clone(), *register, params)
        }
    };

    // Characters.
    let specs: Vec<BotSpec> = (0..opts.bots)
        .map(|i| BotSpec {
            index: i,
            account: account_name(&opts.account_prefix, i),
            password: password.clone(),
            character: character_name(&opts.name_prefix, i),
        })
        .collect();
    let provision_start = Instant::now();
    let (created, existing) = provision_all(&target, &specs, register)?;
    let provision_s = provision_start.elapsed().as_secs_f64();
    // The hostile clients' own characters, numbered after the bots'. A remote server
    // without these accounts (and without registration) leaves the in-world cases unsent;
    // the report counts them as skipped.
    let hostile_specs: Vec<BotSpec> = (opts.bots..opts.bots + hostile_characters(opts))
        .map(|i| BotSpec {
            index: i,
            account: account_name(&opts.account_prefix, i),
            password: password.clone(),
            character: character_name(&opts.name_prefix, i),
        })
        .collect();
    let hostile_characters = if hostile_specs.is_empty() {
        Vec::new()
    } else {
        match provision_all(&target, &hostile_specs, register) {
            Ok(_) => hostile_specs,
            Err(e) => {
                tracing::warn!(error = %e, "hostile characters not provisioned");
                Vec::new()
            }
        }
    };

    let baseline = handle.as_ref().map(|h| {
        wait_stats(h, Duration::from_secs(10), |s| {
            s.logged_in == 0 && s.connections == 0 && s.handshakes == 0
        })
        .0
    });
    let bounds = Bounds {
        bots: opts.bots,
        hostile_characters: hostile_characters.len(),
        max_memberships: (opts.bots + hostile_characters.len()) * MAX_CHANNELS_PER_CHARACTER,
        wanderers: params.wanderers,
        max_connections: opts.bots + opts.hostile.map_or(0, |h| h.clients * 8) + 64,
        max_handshakes: 128,
        hash_queue: 2 + 64,
        base_net_tasks: baseline.map_or(0, |b| b.net_tasks),
        max_entities: MAX_ENTITIES,
        bot_pending: PENDING_PAUSE + READ_CHUNK / 6 + EVENT_CHANNEL_CAPACITY,
    };
    let time = match &handle {
        Some(h) => {
            let base = Instant::now();
            TimeBase::Shared {
                base,
                server_at_base: h.server_time(),
            }
        }
        None => TimeBase::Estimated,
    };

    // Bots.
    let area = Area::around(0, AREA_BLOCK.0, AREA_BLOCK.1, opts.area_blocks, 0.0);
    let mut behaviour = opts.behaviour.clone();
    if let (Some(w), Some(r)) = (params.walk_speed, params.run_speed) {
        behaviour.walk_speed = w;
        behaviour.run_speed = r;
    }
    let names: HashMap<String, usize> = specs
        .iter()
        .map(|s| (s.character.clone(), s.index))
        .collect();
    let start = Instant::now();
    let end = start + opts.duration;
    let mut bots: Vec<Bot> = specs
        .iter()
        .map(|s| {
            Bot::new(
                s.clone(),
                Endpoint {
                    connect: connect.clone(),
                },
                behaviour.clone(),
                rng.next_u64(),
                start + opts.stagger.mul_f64(s.index as f64),
            )
        })
        .collect();
    let mut ledger = ChatLedger::new((rng.next_u64() & 0xFFFF_FFFF) as u32, start);
    let mut desync = DesyncTracker::new(
        opts.bots,
        DesyncConfig {
            radius: params.interest_radius,
            max_entities: MAX_ENTITIES,
            creates_per_tick: params.creates_per_tick,
            tick_rate: params.tick_rate,
        },
        start,
    );
    let mut leak = LeakMonitor::new();
    let stats_start = handle.as_ref().map(ServerHandle::stats);
    let cpu_start = sysinfo::process_cpu_seconds();

    let hostile_window = opts.hostile.map(|_| {
        let lead = opts.duration.mul_f64(0.1).min(Duration::from_secs(10));
        let tail = opts.duration.mul_f64(0.1).min(Duration::from_secs(5));
        (start + lead, end.checked_sub(tail).unwrap_or(end))
    });
    let mut hostile: Option<HostileRun> = None;

    let tick = Duration::from_millis(33);
    let quiet_tail = Duration::from_secs(5).min(opts.duration / 4);
    let mut last = Instant::now();
    let mut next_desync = start;
    let mut next_judge = start;
    let mut next_sample = start;
    let mut next_reconnect = opts.behaviour.reconnect_interval.map(|iv| start + iv);
    let mut quiet = false;
    let mut delay_in_use = None;
    let mut server_info: Option<ServerInfo> = None;
    let mut tool = ToolLoad {
        tick_ms: tick.as_secs_f64() * 1e3,
        ..ToolLoad::default()
    };
    // In-process: when the server last fell behind its tick schedule.
    let mut server_behind: Option<Instant> = None;
    let mut last_lag_counts = handle.as_ref().map(|h| {
        let s = h.stats();
        (s.overruns, s.skipped_ticks)
    });

    loop {
        let now = Instant::now();
        if now >= end {
            break;
        }
        let dt = now.saturating_duration_since(last);
        last = now;
        if !quiet && now + quiet_tail >= end {
            quiet = true;
        }
        // Hostile window.
        if let (Some((from, to)), Some(h)) = (hostile_window, opts.hostile) {
            if hostile.is_none() && now >= from && now < to {
                hostile = Some(HostileRun::start(HostileConfig {
                    target: target.clone(),
                    clients: h.clients,
                    pace: h.pace,
                    seed: rng.next_u64(),
                    characters: hostile_characters.clone(),
                })?);
            }
            if now >= to
                && let Some(run) = &hostile
            {
                // Do not block the bots: the attackers finish their current attack on
                // their own thread and are joined after the run.
                run.signal_stop();
            }
        }
        {
            let mut ctx = Ctx {
                now,
                names: &names,
                ledger: &mut ledger,
                time,
                area: &area,
                radius: desync.config().radius,
            };
            for b in &mut bots {
                b.poll(&mut ctx);
            }
            if !quiet {
                for b in &mut bots {
                    b.step(now, dt, &mut ctx);
                }
            } else {
                // The quiet tail: keep moving, but no new chat, teleports or reconnects, so
                // every line and teleport is answered before the bots leave.
                for b in &mut bots {
                    b.quiet();
                    b.step(now, dt, &mut ctx);
                }
            }
        }
        // Corrections (a cheater's corrections of its cheats are judged by the bot).
        for (i, b) in bots.iter_mut().enumerate() {
            for (t, d) in std::mem::take(&mut b.corrections) {
                desync.record(
                    Kind::Correction,
                    i,
                    None,
                    t,
                    d,
                    "corrected while within the limits".into(),
                );
            }
        }
        // Chat.
        if !quiet {
            send_chat(now, &mut bots, &mut ledger, &mut rng, params.say_range);
        }
        // Reconnects.
        if let (Some(at), Some(iv)) = (next_reconnect, opts.behaviour.reconnect_interval)
            && now >= at
        {
            next_reconnect = Some(at + iv);
            if !quiet {
                let ready: Vec<usize> = (0..bots.len()).filter(|&i| bots[i].in_world()).collect();
                let k = ((opts.behaviour.reconnect_fraction * bots.len() as f64).ceil() as usize)
                    .min(ready.len());
                let mut pool = ready;
                for _ in 0..k {
                    let j = rng.below(pool.len() as u64) as usize;
                    let i = pool.swap_remove(j);
                    bots[i].disconnect(now, true);
                }
            }
        }
        if now >= next_desync {
            // A fixed 10 Hz schedule (§13.2); rounds the loop could not make are skipped
            // and show in the achieved rate.
            next_desync += SAMPLE_INTERVAL;
            if next_desync + SAMPLE_INTERVAL <= now {
                tool.desync_rounds_skipped += 1;
                next_desync = now + SAMPLE_INTERVAL;
            }
            if delay_in_use.is_none() {
                delay_in_use = bots.iter().find_map(|b| {
                    b.backend()
                        .filter(|_| b.in_world())
                        .map(|x| x.interpolation_delay())
                });
            }
            if server_info.is_none()
                && let Some(info) = bots
                    .iter()
                    .find_map(|b| b.backend().and_then(|x| x.server_info()))
            {
                server_info = Some(info.clone());
                if handle.is_none() {
                    params.interest_radius = u32::from(info.interest_radius);
                    params.tick_rate = u32::from(info.tick_rate);
                    params.snapshot_rate = u32::from(info.snapshot_rate);
                    params.server_version =
                        format!("remote: {} (protocol 1.{})", info.name, info.minor);
                    desync.set_radius(params.interest_radius);
                    desync.set_tick_rate(params.tick_rate);
                }
            }
            let lagging = match (&handle, last_lag_counts.as_mut()) {
                (Some(h), Some(last)) => {
                    let st = h.stats();
                    let counts = (st.overruns, st.skipped_ticks);
                    if counts != *last {
                        *last = counts;
                        server_behind = Some(now);
                    }
                    server_behind.is_some_and(|t| now.saturating_duration_since(t) < LAG_HOLD)
                }
                _ => false,
            };
            desync.sample(now, &bots, lagging);
        }
        if now >= next_judge {
            next_judge = now + Duration::from_secs(1);
            ledger.judge_due(now, &|r, from, to, kind| stable(&bots, r, from, to, kind));
        }
        if now >= next_sample {
            next_sample = now + opts.sample_interval;
            let s = leak_sample(now, start, &bots, handle.as_ref(), opts.server_pid);
            desync.set_load(s.load);
            leak.push(s, &bounds);
        }
        let next = last + tick;
        let after = Instant::now();
        let work = after.saturating_duration_since(now);
        tool.iterations += 1;
        tool.longest_ms = tool.longest_ms.max(work.as_secs_f64() * 1e3);
        if work > tick {
            tool.overruns += 1;
        }
        if next > after {
            std::thread::sleep(next - after);
        }
    }
    let measured_s = start.elapsed().as_secs_f64();
    if let Some(run) = &hostile {
        run.signal_stop();
    }
    // A last sample at full load, then everyone leaves.
    let s = leak_sample(
        Instant::now(),
        start,
        &bots,
        handle.as_ref(),
        opts.server_pid,
    );
    leak.push(s, &bounds);
    let stats_end = handle.as_ref().map(ServerHandle::stats);
    let cpu_end = sysinfo::process_cpu_seconds();
    for b in &mut bots {
        b.flush_traffic();
    }
    let now = Instant::now();
    for b in &mut bots {
        b.finish(now);
    }
    let leave_deadline = Instant::now() + Duration::from_secs(15);
    while !bots.iter().all(Bot::done) && Instant::now() < leave_deadline {
        let now = Instant::now();
        let mut ctx = Ctx {
            now,
            names: &names,
            ledger: &mut ledger,
            time,
            area: &area,
            radius: desync.config().radius,
        };
        for b in &mut bots {
            b.poll(&mut ctx);
            b.step(now, Duration::ZERO, &mut ctx);
        }
        std::thread::sleep(Duration::from_millis(20));
    }
    ledger.judge_all(&|r, from, to, kind| stable(&bots, r, from, to, kind));
    let left_s = Instant::now().duration_since(now).as_secs_f64();
    let hostile_snapshot = hostile.take().map(HostileRun::stop);

    // Return to the baseline and server shutdown (in-process).
    let mut baseline_result = None;
    let mut shutdown = None;
    if let (Some(h), Some(base)) = (handle.take(), baseline) {
        let (after, ok) = wait_stats(&h, Duration::from_secs(10), |s| {
            baseline_differences(&base, s).is_empty()
        });
        let differences = if ok {
            Vec::new()
        } else {
            baseline_differences(&base, &after)
        };
        let throttle = if opts.check_throttle {
            let (t, ok) = wait_stats(&h, Duration::from_secs(30 + 60), |s| {
                s.throttle_entries <= base.throttle_entries
            });
            Some((ok, t.throttle_entries))
        } else {
            None
        };
        baseline_result = Some(BaselineResult {
            differences,
            throttle,
            after,
        });
        shutdown = Some(h.shutdown(Duration::ZERO));
    }
    drop(temp);
    let panics_during = panics() - panics_before;
    let bot_stats = bots
        .iter()
        .map(|b| (b.spec.index, b.cheater, b.stats.clone()))
        .collect();
    Ok(Report::build(crate::report::Inputs {
        opts: opts.clone(),
        machine,
        started_wall,
        measured_s,
        left_s,
        provision: (created, existing, provision_s),
        in_process: matches!(opts.target, TargetOptions::InProcess { .. }),
        bots: bot_stats,
        desync,
        ledger,
        leak,
        stats: stats_start.zip(stats_end),
        cpu: cpu_start.zip(cpu_end).map(|(a, b)| b - a),
        hostile: hostile_snapshot,
        baseline: baseline_result,
        shutdown,
        panics: panics_during,
        interpolation_delay: delay_in_use,
        params,
        tool,
        area_blocks: area.blocks(),
    }))
}

/// Missing is not judged for this long after the in-process server fell behind its tick
/// schedule (overruns or skipped ticks slow the §9.4 create budget).
const LAG_HOLD: Duration = Duration::from_secs(2);

/// Whether recipient `r`'s situation held still during [`from`, `to`] for a line of `kind`
/// (see [`ChatLedger::judge_due`]). For a channel line, a bot of another channel is never a
/// member, so it must not receive the line wherever it is; a bot of the line's channel is
/// judged only when it was in the world and its membership held still.
fn stable(bots: &[Bot], r: usize, from: Instant, to: Instant, kind: ChatKind) -> bool {
    bots.get(r).is_some_and(|b| match kind {
        ChatKind::Channel(k) if b.channel_slot() != Some(k) => true,
        ChatKind::Channel(_) => b.online_during(from, to) && b.channel_settled_during(from, to),
        _ => b.online_during(from, to) && (!kind.positional() || !b.jumped_during(from, to)),
    })
}

/// Hostile characters the run provisions: [`HOSTILE_CHARACTERS`] with hostile clients.
fn hostile_characters(opts: &RunOptions) -> usize {
    opts.hostile.map_or(0, |_| HOSTILE_CHARACTERS)
}

fn leak_sample(
    now: Instant,
    start: Instant,
    bots: &[Bot],
    h: Option<&ServerHandle>,
    server_pid: Option<u32>,
) -> LeakSample {
    LeakSample {
        t_s: now.saturating_duration_since(start).as_secs_f64(),
        rss: sysinfo::process_rss(),
        server_rss: server_pid.and_then(sysinfo::pid_rss),
        server: h.map(ServerHandle::stats),
        bots_in_world: bots.iter().filter(|b| b.in_world()).count(),
        bot_entities: bots.iter().map(|b| b.view.len()).sum(),
        bot_entities_max: bots.iter().map(|b| b.view.len()).max().unwrap_or(0),
        bot_bots_max: bots.iter().map(|b| b.view_bot.len()).max().unwrap_or(0),
        bot_pending: bots
            .iter()
            .filter_map(|b| b.backend().map(|x| x.stats().pending))
            .sum(),
        load: sysinfo::load_average(),
        cpu_s: sysinfo::process_cpu_seconds(),
    }
}

/// Margin around the say range in which delivery is not judged (one second at run speed).
const SAY_MARGIN: f64 = 6.0;

/// How a Say line's possible recipient is judged.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum SayRecipient {
    /// Must receive it.
    Certain,
    /// May receive it (near the range edge, teleporting, or the range is unknown).
    Optional,
    /// Must not receive it.
    Excluded,
}

/// The rule for a recipient `d` metres from the speaker (`test-server.md` §15.2).
fn say_recipient(range: Option<f64>, d: f64, teleporting: bool) -> SayRecipient {
    match range {
        _ if teleporting => SayRecipient::Optional,
        None => SayRecipient::Optional,
        Some(r) if d < r - SAY_MARGIN => SayRecipient::Certain,
        Some(r) if d <= r + SAY_MARGIN => SayRecipient::Optional,
        Some(_) => SayRecipient::Excluded,
    }
}

/// Sends the chat lines that are due. `say_range`: the server's Say range; `None` (a
/// remote server whose range is not given) judges Say only for the speaker's echo.
fn send_chat(
    now: Instant,
    bots: &mut [Bot],
    ledger: &mut ChatLedger,
    rng: &mut Rng,
    say_range: Option<f64>,
) {
    for s in 0..bots.len() {
        let Some(seq) = bots[s].chat_due(now) else {
            continue;
        };
        let others: Vec<usize> = (0..bots.len())
            .filter(|&r| r != s && bots[r].in_world())
            .collect();
        let member = bots[s].channel_member();
        let roll = rng.unit();
        let kind = if let Some(k) = member
            && rng.unit() < bots[s].channel_fraction()
        {
            ChatKind::Channel(k)
        } else if roll < 0.4 {
            ChatKind::Say
        } else if roll < 0.8 || others.is_empty() {
            ChatKind::Yell
        } else {
            ChatKind::Tell
        };
        let mut certain = vec![s];
        let mut optional = Vec::new();
        let channel = match kind {
            ChatKind::Say => {
                let me = bots[s].pos;
                for (r, b) in bots.iter().enumerate() {
                    if r == s {
                        continue;
                    }
                    if !b.in_world() {
                        optional.push(r);
                        continue;
                    }
                    if b.pos.region != me.region {
                        continue;
                    }
                    let d = b.pos.distance_xy(&me);
                    match say_recipient(say_range, d, b.teleport_pending.is_some()) {
                        SayRecipient::Certain => certain.push(r),
                        SayRecipient::Optional => optional.push(r),
                        SayRecipient::Excluded => {}
                    }
                }
                ChatChannel::Say
            }
            ChatKind::Yell => {
                // Instance-wide (`tessera-protocol.md` §10.1): bots in another instance
                // (`WorldEnter.instance`) are excluded.
                let mine = bots[s].instance();
                for (r, b) in bots.iter().enumerate() {
                    if r == s {
                        continue;
                    }
                    if b.in_world() && !crate::desync::same_instance(mine, b.instance()) {
                        continue;
                    }
                    if b.in_world() {
                        certain.push(r);
                    } else {
                        optional.push(r);
                    }
                }
                ChatChannel::Yell
            }
            ChatKind::Tell => {
                let r = others[rng.below(others.len() as u64) as usize];
                certain.push(r);
                ChatChannel::Tell {
                    peer: bots[r].spec.character.clone(),
                }
            }
            // Exactly the members (§14 invariant 15): those whose join was answered must
            // receive it, those with a join or leave in flight may.
            ChatKind::Channel(k) => {
                for (r, b) in bots.iter().enumerate() {
                    if r == s {
                        continue;
                    }
                    if b.channel_member() == Some(k) {
                        certain.push(r);
                    } else if b.channel_pending() == Some(k) {
                        optional.push(r);
                    }
                }
                ChatChannel::Named {
                    name: ledger.channel_name(k),
                }
            }
        };
        let text = ledger.text(s, seq, kind);
        if bots[s].chat(now, channel, text) {
            ledger.sent(s, seq, kind, now, certain, optional);
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn names() {
        assert_eq!(character_name("Bot", 0), "Botaaa");
        assert_eq!(character_name("Bot", 27), "Botabb");
        assert_eq!(account_name("bot-", 0), "bot-0001");
        for s in Scenario::ALL {
            assert_eq!(Scenario::parse(s.name()), Some(s));
        }
        assert_eq!(Scenario::parse("nope"), None);
    }

    #[test]
    fn content_and_config_are_valid() {
        let c = Content::parse(LOADTEST_CONTENT).expect("content parses");
        assert!(c.check_invariants().is_empty());
        let cfg = in_process_config(32, None);
        assert!(cfg.world.instance_capacity >= 32);
        let cfg = in_process_config(
            1_024,
            Some(HostileOptions {
                clients: 256,
                pace: Duration::ZERO,
            }),
        );
        assert!(cfg.limits.max_connections <= 4_096);
    }

    #[test]
    fn options_are_validated() {
        let ok = RunOptions::new(Scenario::Walk, 2);
        assert!(ok.validate().is_ok());
        let mut bad = ok.clone();
        bad.bots = 0;
        assert!(bad.validate().is_err());
        let mut bad = ok.clone();
        bad.name_prefix = "Bot1".into();
        assert!(bad.validate().is_err());
        let mut bad = ok.clone();
        bad.account_prefix = "a b".into();
        assert!(bad.validate().is_err());
        let mut bad = ok.clone();
        bad.behaviour.walk_fraction = 2.0;
        assert!(bad.validate().is_err());
    }

    /// Regression: `start + duration` and `stagger × index` panicked on overflow for
    /// library callers (`Duration::MAX` for "run until stopped").
    #[test]
    fn extreme_durations_are_usage_errors() {
        let ok = RunOptions::new(Scenario::Walk, 2);
        for f in [
            (|o: &mut RunOptions| o.duration = Duration::MAX) as fn(&mut RunOptions),
            |o| o.stagger = Duration::MAX,
            |o| o.sample_interval = Duration::MAX,
            |o| o.warmup_min = Duration::MAX,
            |o| o.behaviour.chat_interval = Some(Duration::MAX),
            |o| {
                o.bots = 1_024;
                o.stagger = Duration::from_secs(1_000_000);
            },
            |o| {
                o.hostile = Some(HostileOptions {
                    clients: 1,
                    pace: Duration::from_secs(11),
                });
            },
            |o| o.say_range = Some(f64::NAN),
            |o| o.behaviour.run_speed = f64::INFINITY,
            |o| o.behaviour.truth_bias_m = f64::NAN,
        ] {
            let mut bad = ok.clone();
            f(&mut bad);
            assert!(
                matches!(bad.validate(), Err(LoadtestError::Usage(_))),
                "{bad:?}"
            );
            assert!(matches!(run(&bad), Err(LoadtestError::Usage(_))));
        }
    }

    /// Regression: `--data-dir` silently reset the passwords of existing accounts.
    #[test]
    fn existing_accounts_file_needs_reuse() {
        let dir = TempDir::create().expect("temp dir");
        let data = DataDir::new(dir.0.join("data"));
        data.create().expect("data dir");
        std::fs::write(data.accounts_path(), "").expect("accounts file");
        let mut o = RunOptions::new(Scenario::Walk, 1);
        o.target = TargetOptions::InProcess {
            data_dir: Some(data.path().to_path_buf()),
        };
        let e = run(&o).expect_err("refused");
        assert!(
            matches!(&e, LoadtestError::Usage(m) if m.contains("--reuse-data-dir")),
            "{e}"
        );
    }

    #[test]
    fn remote_password_is_redacted() {
        let mut o = RunOptions::new(Scenario::Walk, 1);
        o.target = TargetOptions::Remote {
            connect: "tessera://127.0.0.1:1".into(),
            password: Password::new("hunter2-secret"),
            register: false,
        };
        assert!(!format!("{o:?}").contains("hunter2-secret"));
    }

    /// Regression: the Say range was fixed at 40 m, so on a server with a 20 m range bots
    /// 20–34 m away were certain recipients and showed as "lost". The range now comes from
    /// the server's configuration or `--say-range`; unknown, only the echo is judged.
    #[test]
    fn say_recipients_follow_the_range() {
        use SayRecipient::*;
        assert_eq!(say_recipient(Some(20.0), 25.0, false), Optional);
        assert_eq!(say_recipient(Some(20.0), 30.0, false), Excluded);
        assert_eq!(say_recipient(Some(20.0), 13.0, false), Certain);
        assert_eq!(say_recipient(Some(20.0), 13.0, true), Optional);
        assert_eq!(say_recipient(Some(40.0), 30.0, false), Certain);
        assert_eq!(say_recipient(None, 1.0, false), Optional);
    }
}
