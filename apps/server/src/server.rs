// SPDX-License-Identifier: GPL-3.0-or-later
//! Starting and stopping a server in-process (`test-server.md` §2.2, §8.1, §12).
//!
//! [`Server::start`] performs the start-up steps of `run` after the configuration and
//! content are known: data directory and lock, key, accounts, characters, world state,
//! then the writer, hash workers, simulation and network threads. The returned
//! [`ServerHandle`] exposes the fingerprint, the bound addresses, live counters and a clean
//! shutdown (§12.1). Tests, `apps/loadtest` and the binary all use it.

use std::net::SocketAddr;
use std::sync::Arc;
use std::sync::atomic::{AtomicBool, Ordering};
use std::thread::JoinHandle;
use std::time::{Duration, Instant};

use tessera_protocol::fingerprint::Fingerprint;
use tessera_protocol::message::ServerCaps;
use tessera_protocol::server::ServerParams;
use tessera_protocol::transport::ServerIdentity;
use tokio::sync::mpsc::UnboundedSender;

use crate::accounts::{AccountStore, HashParams};
use crate::characters::{self, WorldState};
use crate::config::Config;
use crate::content::Content;
use crate::datadir::{self, DataDir, ServerLock};
use crate::error::ServerError;
use crate::net::{self, NetControl, NetCtx};
use crate::shared::{Clock, SimControl, SimInput, Stats, StatsSnapshot, StopReport};
use crate::sim::{self, Sim, SimParts};
use crate::workers::{HashContext, HashPool, Writer};

/// How the server is started in-process.
#[derive(Debug, Clone)]
pub struct ServerOptions {
    /// The data directory (created if missing, mode 0700).
    pub data_dir: DataDir,
    /// The configuration (`[server] listen` must name at least one address; port 0 picks a
    /// free port).
    pub config: Config,
    /// The world content. [`Content::parse`] validates it; [`Server::start`] re-checks the
    /// structural rules the simulation relies on ([`Content::check_invariants`]) and refuses
    /// a hand-built one that breaks them.
    pub content: Content,
}

/// What a finished shutdown reports.
#[derive(Debug, Clone, Copy, Default, PartialEq, Eq)]
pub struct ShutdownReport {
    /// Characters saved at shutdown.
    pub saved: usize,
    /// Failed writes (exit status 1 when not zero, §12.1 step 6).
    pub failures: usize,
}

/// The server.
#[derive(Debug)]
pub struct Server;

/// A running server.
#[derive(Debug)]
pub struct ServerHandle {
    fingerprint: Fingerprint,
    addrs: Vec<SocketAddr>,
    stats: Arc<Stats>,
    clock: Clock,
    sim_tx: std::sync::mpsc::SyncSender<SimInput>,
    net_ctl: UnboundedSender<NetControl>,
    threads: Vec<JoinHandle<()>>,
    hash_pool: Option<HashPool>,
    writer: Option<Writer>,
    hurry: Arc<AtomicBool>,
    grace: Duration,
    _lock: ServerLock,
    stopped: bool,
}

/// The `HelloAck` parameters from the configuration and content.
#[must_use]
pub fn server_params(config: &Config, content: &Content) -> ServerParams {
    let mut caps = 0;
    if config.server.registration {
        caps |= ServerCaps::REGISTRATION.0;
    }
    if config.server.character_creation {
        caps |= ServerCaps::CHARACTER_CREATION.0;
    }
    if config.server.appearance_change {
        caps |= ServerCaps::APPEARANCE_CHANGE.0;
    }
    // Sent only to connections at minor ≥ 2 (the session masks it below, §4.4, §4.6).
    if config.chat.channels {
        caps |= ServerCaps::CHAT_CHANNELS.0;
    }
    ServerParams {
        caps: ServerCaps(caps),
        snapshot_rate: config.world.snapshot_rate,
        interest_radius: config.world.interest_radius,
        server_name: config.server.name.clone(),
        motd: config
            .server
            .motd
            .clone()
            .unwrap_or_else(|| content.motd.clone()),
        ..ServerParams::default()
    }
}

/// Ranges of the configuration that the threads rely on (§4), for a [`Config`] built by hand
/// rather than parsed. Returns what is wrong, in words.
fn config_problems(c: &Config) -> Vec<String> {
    let mut out = Vec::new();
    let mut check = |ok: bool, what: &str| {
        if !ok {
            out.push(format!("{what} out of range"));
        }
    };
    check((1..=16).contains(&c.world.instances), "`world.instances`");
    check(c.world.instance_capacity >= 1, "`world.instance_capacity`");
    check(
        [1, 2, 3, 5, 6, 10, 15, 30].contains(&c.world.snapshot_rate),
        "`world.snapshot_rate`",
    );
    check(c.world.interest_radius <= 8, "`world.interest_radius`");
    check(
        (1..=8).contains(&c.accounts.hash_workers),
        "`accounts.hash_workers`",
    );
    check(
        (1..=4_096).contains(&c.limits.max_connections),
        "`limits.max_connections`",
    );
    check(
        (1..=1_024).contains(&c.limits.max_handshakes),
        "`limits.max_handshakes`",
    );
    check(
        (16..=1_024).contains(&c.limits.control_queue),
        "`limits.control_queue`",
    );
    check(
        (1..=4_096).contains(&c.chat.max_channels),
        "`chat.max_channels`",
    );
    check(
        (1..=32).contains(&c.chat.max_channels_per_character),
        "`chat.max_channels_per_character`",
    );
    check(
        (2..=4_096).contains(&c.chat.max_channel_members),
        "`chat.max_channel_members`",
    );
    out
}

impl Server {
    /// Starts a server. Fails before any socket is opened when the data directory, the
    /// configuration (address guard), the key or the accounts or character files are not
    /// usable. Takes the server lock (§3.7) for the server's lifetime.
    pub fn start(options: ServerOptions) -> Result<ServerHandle, ServerError> {
        options.data_dir.create()?;
        let lock = ServerLock::acquire(&options.data_dir, true)?;
        Self::start_locked(options, lock)
    }

    /// [`start`](Self::start) with the server lock already held by the caller (`run` takes
    /// it before reading the configuration and holds it until exit, §2.2 step 1). `lock`
    /// must be the lock of `options.data_dir`, taken with the PID written.
    pub fn start_locked(
        options: ServerOptions,
        lock: ServerLock,
    ) -> Result<ServerHandle, ServerError> {
        let ServerOptions {
            data_dir,
            config,
            content,
        } = options;
        let mut problems = content.check_invariants();
        problems.extend(config_problems(&config));
        if !problems.is_empty() {
            return Err(ServerError::Usage(format!(
                "server options refused: {}",
                problems.join("; ")
            )));
        }
        data_dir.create()?;
        data_dir.remove_stale_tmp();
        if config.server.listen.is_empty() {
            return Err(ServerError::Usage(
                "no listen address: set [server] listen or pass --listen".into(),
            ));
        }
        let bad = config.registration_violations();
        if !bad.is_empty() {
            return Err(ServerError::Usage(format!(
                "registration = true needs private listen addresses only (bind the LAN address explicitly); not private: {}",
                bad.iter()
                    .map(ToString::to_string)
                    .collect::<Vec<_>>()
                    .join(", ")
            )));
        }
        if config
            .server
            .listen
            .iter()
            .any(|a| !crate::config::is_private(a.ip()))
        {
            tracing::warn!(
                "listening on a non-private address: Tessera test servers are for private or LAN testing only; no public hosting, no money, and each player needs their own legally obtained installation"
            );
        }
        let (identity, generated) = datadir::load_or_generate_key(&data_dir)?;
        if generated {
            tracing::info!("generated a new server key");
        }
        let fingerprint = identity.fingerprint();
        let accounts = Arc::new(AccountStore::load(&data_dir)?);
        let chars = characters::load_all(&data_dir)?;
        let world_state = WorldState::load(&data_dir);
        let config = Arc::new(config);
        let content = Arc::new(content);
        let params = HashParams::from_config(&config.accounts);
        let dummy = HashContext::make_dummy(params)?;

        let stats = Arc::new(Stats::default());
        let clock = Clock::new();
        let l = &config.limits;
        let capacity = (l.max_connections as usize) * (l.control_queue as usize + 8)
            + config.accounts.hash_workers as usize
            + crate::workers::HASH_QUEUE
            + 4_096;
        let (sim_tx, sim_rx) = std::sync::mpsc::sync_channel::<SimInput>(capacity);
        let writer = Writer::start(
            sim_tx.clone(),
            Some(Arc::clone(&accounts)),
            Arc::clone(&stats),
        )?;
        let hash_pool = HashPool::start(
            config.accounts.hash_workers as usize,
            HashContext {
                accounts: Arc::clone(&accounts),
                params,
                dummy,
                results: sim_tx.clone(),
                stats: Arc::clone(&stats),
            },
        )?;
        let sim = Sim::new(
            SimParts {
                config: Arc::clone(&config),
                content: Arc::clone(&content),
                data_dir: data_dir.clone(),
                accounts: Arc::clone(&accounts),
                characters: chars,
                world_state,
                hash_jobs: hash_pool.sender(),
                writer: writer.sender(),
                stats: Arc::clone(&stats),
            },
            clock.now(),
        );
        let sim_thread = std::thread::Builder::new()
            .name("tessera-sim".into())
            .spawn(move || sim::run(sim, &sim_rx, clock))
            .map_err(|e| ServerError::Runtime(format!("cannot start the simulation: {e}")))?;
        let net_ctx = Arc::new(NetCtx::new(
            clock,
            Arc::clone(&stats),
            Arc::clone(&config),
            server_params(&config, &content),
            sim_tx.clone(),
        ));
        let started = net::start(net_ctx, &identity, &config.server.listen);
        drop(identity);
        let (net_thread, addrs, net_ctl) = match started {
            Ok(x) => x,
            Err(e) => {
                let (tx, rx) = std::sync::mpsc::channel();
                let _ = sim_tx.send(SimInput::Control(SimControl::Stop(tx)));
                let _ = rx.recv_timeout(Duration::from_secs(10));
                let _ = sim_thread.join();
                hash_pool.stop();
                writer.stop();
                return Err(e);
            }
        };
        // §2.2 step 5: the fingerprint and a connect-string hint per listen address.
        tracing::info!(fingerprint = %fingerprint, "server key");
        for a in &addrs {
            let host = if a.ip().is_unspecified() {
                "<host>".to_owned()
            } else {
                match a {
                    SocketAddr::V6(v6) => format!("[{}]", v6.ip()),
                    SocketAddr::V4(v4) => v4.ip().to_string(),
                }
            };
            tracing::info!(
                "connect string: tessera://{host}:{}?pin={fingerprint}",
                a.port()
            );
        }
        tracing::info!(
            version = env!("CARGO_PKG_VERSION"),
            protocol = "1.0",
            listen = ?addrs,
            fingerprint = %fingerprint,
            content = %content.id,
            instances = config.world.instances,
            "ready"
        );
        Ok(ServerHandle {
            fingerprint,
            addrs,
            stats,
            clock,
            sim_tx,
            net_ctl,
            threads: vec![sim_thread, net_thread],
            hash_pool: Some(hash_pool),
            writer: Some(writer),
            hurry: Arc::new(AtomicBool::new(false)),
            grace: Duration::from_secs(u64::from(config.server.shutdown_grace_s)),
            _lock: lock,
            stopped: false,
        })
    }
}

impl ServerHandle {
    /// The server key's fingerprint (`tessera-protocol.md` §4.2).
    #[must_use]
    pub fn fingerprint(&self) -> Fingerprint {
        self.fingerprint
    }

    /// The bound UDP addresses (the actual port when 0 was configured).
    #[must_use]
    pub fn local_addrs(&self) -> &[SocketAddr] {
        &self.addrs
    }

    /// The first bound address.
    #[must_use]
    pub fn local_addr(&self) -> SocketAddr {
        self.addrs[0]
    }

    /// A pinned connect string (`tessera-protocol.md` §4.4) for the first bound address. An
    /// unspecified bind address (`0.0.0.0`, `[::]`) is replaced by the loopback address of
    /// its family, which is dialable on every platform; for other hosts use
    /// `tessera-server fingerprint --connect HOST`.
    #[must_use]
    pub fn connect_string(&self) -> String {
        let mut addr = self.local_addr();
        if addr.ip().is_unspecified() {
            addr.set_ip(match addr {
                SocketAddr::V4(_) => std::net::Ipv4Addr::LOCALHOST.into(),
                SocketAddr::V6(_) => std::net::Ipv6Addr::LOCALHOST.into(),
            });
        }
        format!("tessera://{addr}?pin={}", self.fingerprint)
    }

    /// Live counters.
    #[must_use]
    pub fn stats(&self) -> StatsSnapshot {
        self.stats.snapshot()
    }

    /// Server time now, µs.
    #[must_use]
    pub fn server_time(&self) -> u64 {
        self.clock.now()
    }

    /// The configured shutdown grace.
    #[must_use]
    pub fn grace(&self) -> Duration {
        self.grace
    }

    /// A flag that ends the grace period at once when set (a second signal, §12.1 step 3).
    #[must_use]
    pub fn hurry_flag(&self) -> Arc<AtomicBool> {
        Arc::clone(&self.hurry)
    }

    /// Graceful shutdown (§12.1): stop accepting, send shutdown notices (at once and at the
    /// 60, 30, 10 and 5 s marks inside the grace), wait out the grace (or until the hurry
    /// flag is set), stop the simulation (saving every character in the world and the world
    /// state), send `Disconnect(SERVER_SHUTDOWN)`, wait at most 2 s for clients, close the
    /// endpoints and join every thread.
    pub fn shutdown(mut self, grace: Duration) -> ShutdownReport {
        self.shutdown_inner(grace)
    }

    fn shutdown_inner(&mut self, grace: Duration) -> ShutdownReport {
        if self.stopped {
            return ShutdownReport::default();
        }
        self.stopped = true;
        let _ = self.net_ctl.send(NetControl::StopAccepting);
        let start = Instant::now();
        let mut marks: Vec<u64> = [60u64, 30, 10, 5]
            .into_iter()
            .filter(|m| Duration::from_secs(*m) < grace)
            .collect();
        // §12.1 step 2: a notice at once, whatever the grace (0 s: "shutting down now").
        let secs = u16::try_from(grace.as_secs()).unwrap_or(u16::MAX);
        let _ = self
            .sim_tx
            .send(SimInput::Control(SimControl::ShutdownNotice {
                seconds: secs,
            }));
        if !grace.is_zero() {
            while start.elapsed() < grace && !self.hurry.load(Ordering::Acquire) {
                let left = grace.saturating_sub(start.elapsed());
                if let Some(&m) = marks.first()
                    && left <= Duration::from_secs(m)
                {
                    marks.remove(0);
                    let _ = self
                        .sim_tx
                        .send(SimInput::Control(SimControl::ShutdownNotice {
                            seconds: u16::try_from(m).unwrap_or(u16::MAX),
                        }));
                }
                std::thread::sleep(Duration::from_millis(20).min(left));
            }
        }
        let (tx, rx) = std::sync::mpsc::channel::<StopReport>();
        let report = if self
            .sim_tx
            .send(SimInput::Control(SimControl::Stop(tx)))
            .is_ok()
        {
            rx.recv_timeout(Duration::from_secs(120))
                .unwrap_or(StopReport {
                    saved: 0,
                    failures: 1,
                })
        } else {
            StopReport {
                saved: 0,
                failures: 1,
            }
        };
        let (done_tx, done_rx) = std::sync::mpsc::channel();
        let _ = self.net_ctl.send(NetControl::Shutdown {
            linger: Duration::from_secs(2),
            done: done_tx,
        });
        let _ = done_rx.recv_timeout(Duration::from_secs(10));
        for t in self.threads.drain(..) {
            let _ = t.join();
        }
        if let Some(p) = self.hash_pool.take() {
            p.stop();
        }
        if let Some(w) = self.writer.take() {
            w.stop();
        }
        tracing::info!(saved = report.saved, failures = report.failures, "stopped");
        ShutdownReport {
            saved: report.saved,
            failures: report.failures,
        }
    }
}

impl Drop for ServerHandle {
    fn drop(&mut self) {
        if !self.stopped {
            let _ = self.shutdown_inner(Duration::ZERO);
        }
    }
}

/// Builds the server identity from the data directory without starting anything (for the
/// `fingerprint` command and tests).
pub fn identity(data_dir: &DataDir) -> Result<ServerIdentity, ServerError> {
    datadir::load_key(data_dir)
}
