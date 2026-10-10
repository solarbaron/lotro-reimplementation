// SPDX-License-Identifier: GPL-3.0-or-later
//! What the network, simulation, hash-worker and writer threads share (`test-server.md`
//! §8.1): the server clock, per-connection shared state, the messages on the bounded
//! channels between the threads, and the statistics counters.

use std::net::SocketAddr;
use std::path::PathBuf;
use std::sync::Mutex;
use std::sync::atomic::{AtomicBool, AtomicU16, AtomicU64, AtomicUsize, Ordering};
use std::time::Instant;

use tessera_protocol::codes::{
    AppErrorCode, CharacterCreateFailedCode, ChatRejectedCode, EnterWorldFailedCode,
    LoginFailedCode,
};
use tessera_protocol::message::{MoveInput, WorldEnter};
use tessera_protocol::server::ServerEvent;
use tessera_protocol::types::CharacterSummary;

/// A connection number, unique for the server's lifetime.
pub type ConnId = u64;

/// Server time (`tessera-protocol.md` §11.1): microseconds since the server started,
/// monotonic, shared by every thread.
#[derive(Debug, Clone, Copy)]
pub struct Clock {
    start: Instant,
}

impl Clock {
    /// A clock starting now.
    #[must_use]
    pub fn new() -> Self {
        Self {
            start: Instant::now(),
        }
    }

    /// Server time now, µs.
    #[must_use]
    pub fn now(&self) -> u64 {
        u64::try_from(self.start.elapsed().as_micros()).unwrap_or(u64::MAX)
    }

    /// The instant of a server time.
    #[must_use]
    pub fn instant(&self, t: u64) -> Instant {
        self.start + std::time::Duration::from_micros(t)
    }
}

impl Default for Clock {
    fn default() -> Self {
        Self::new()
    }
}

/// A request from the simulation to a connection's protocol session (applied on the
/// network thread to [`tessera_protocol::server::ServerSession`]).
#[derive(Debug, Clone, PartialEq)]
pub enum SessionCmd {
    /// `LoginOk` (opens the chat stream).
    AcceptLogin {
        /// Canonical account name.
        account: String,
        /// The account's characters.
        characters: Vec<CharacterSummary>,
    },
    /// `LoginFailed`.
    RejectLogin {
        /// The code.
        code: LoginFailedCode,
        /// Seconds, for `RATE_LIMITED`.
        retry_after: u16,
    },
    /// `CharacterList` after a creation.
    CharacterCreated(Vec<CharacterSummary>),
    /// `CharacterCreateFailed`.
    RejectCharacter(CharacterCreateFailedCode),
    /// Opens the world stream with `WorldEnter`.
    EnterWorld(Box<WorldEnter>),
    /// `EnterWorldFailed`.
    RejectEnterWorld(EnterWorldFailedCode),
    /// `ChatRejected` for the oldest unanswered `ChatSend` (counts towards `FLOODING`).
    RejectChat(ChatRejectedCode),
    /// The oldest unanswered `ChatSend` was delivered (its echo is already queued). Sends
    /// nothing; it releases session-made rejections of later `ChatSend`s, which must not
    /// overtake it (`tessera-protocol.md` §6.4.3).
    ChatDelivered,
    /// `Disconnect(code, text)` and the closing procedure.
    Disconnect(AppErrorCode, String),
}

/// Outbound work for one connection, in order.
#[derive(Debug, Clone, PartialEq)]
pub enum Outbound {
    /// A protocol-session request.
    Cmd(SessionCmd),
    /// An encoded world-stream frame (already counted in `world_unsent`).
    World(Vec<u8>),
    /// An encoded chat-stream frame (already counted in `chat_unsent`).
    Chat(Vec<u8>),
    /// An encoded datagram (`MotionBatch`).
    Datagram(Vec<u8>),
}

/// The receiving end of a connection's outbound queue (the network task's end).
pub type OutboundReceiver = tokio::sync::mpsc::Receiver<Outbound>;

/// No kill requested.
const NO_KILL: u16 = u16::MAX;

/// Per-connection state shared by the network task and the simulation.
#[derive(Debug)]
pub struct ConnShared {
    /// Connection number.
    pub id: ConnId,
    /// Source address at accept time.
    pub addr: SocketAddr,
    /// Outbound queue to the network task (bounded; a full queue kills the connection).
    pub outbound: tokio::sync::mpsc::Sender<Outbound>,
    /// Control events forwarded to the simulation and not yet processed by it.
    pub inbound_pending: AtomicUsize,
    /// World-stream bytes queued but not yet accepted by the QUIC stream.
    pub world_unsent: AtomicUsize,
    /// Chat-stream bytes queued but not yet accepted by the QUIC stream.
    pub chat_unsent: AtomicUsize,
    /// Control-stream bytes queued but not yet accepted by the QUIC stream.
    pub control_unsent: AtomicUsize,
    /// The newest unprocessed `MoveInput` (a queue of one, newest wins; §10.1).
    pub latest_move: Mutex<Option<MoveInput>>,
    /// Older unprocessed movement dropped.
    pub moves_dropped: AtomicU64,
    /// Set once the network task has finished.
    pub closed: AtomicBool,
    /// Set (before `closed`) when the task's `SimInput::Closed` was queued: the simulation
    /// then waits for it instead of acting on the flag, so events forwarded before it are
    /// handled first.
    pub close_queued: AtomicBool,
    kill_code: AtomicU16,
    kill_notify: tokio::sync::Notify,
}

impl ConnShared {
    /// New shared state and the receiving end of its outbound queue.
    #[must_use]
    pub fn new(id: ConnId, addr: SocketAddr, capacity: usize) -> (Self, OutboundReceiver) {
        let (tx, rx) = tokio::sync::mpsc::channel(capacity.max(1));
        (
            Self {
                id,
                addr,
                outbound: tx,
                inbound_pending: AtomicUsize::new(0),
                world_unsent: AtomicUsize::new(0),
                chat_unsent: AtomicUsize::new(0),
                control_unsent: AtomicUsize::new(0),
                latest_move: Mutex::new(None),
                moves_dropped: AtomicU64::new(0),
                closed: AtomicBool::new(false),
                close_queued: AtomicBool::new(false),
                kill_code: AtomicU16::new(NO_KILL),
                kill_notify: tokio::sync::Notify::new(),
            },
            rx,
        )
    }

    /// Asks the network task to close the connection at once with `code` (used when even a
    /// `Disconnect` cannot be queued).
    pub fn kill(&self, code: AppErrorCode) {
        let _ = self.kill_code.compare_exchange(
            NO_KILL,
            code.to_u16(),
            Ordering::AcqRel,
            Ordering::Acquire,
        );
        self.kill_notify.notify_one();
    }

    /// The requested kill code, if any.
    #[must_use]
    pub fn kill_requested(&self) -> Option<AppErrorCode> {
        let c = self.kill_code.load(Ordering::Acquire);
        (c != NO_KILL).then(|| AppErrorCode::from_u16(c))
    }

    /// Waits for a kill request.
    pub async fn killed(&self) {
        if self.kill_requested().is_some() {
            return;
        }
        self.kill_notify.notified().await;
    }

    /// Stores a `MoveInput`, replacing (and counting) an unprocessed older one.
    pub fn put_move(&self, m: MoveInput) {
        if let Ok(mut slot) = self.latest_move.lock()
            && slot.replace(m).is_some()
        {
            self.moves_dropped.fetch_add(1, Ordering::Relaxed);
        }
    }

    /// Takes the newest unprocessed `MoveInput`.
    pub fn take_move(&self) -> Option<MoveInput> {
        self.latest_move.lock().ok().and_then(|mut s| s.take())
    }

    /// Queues outbound work. Returns false (and kills the connection) when the queue is
    /// full or closed.
    pub fn send(&self, out: Outbound) -> bool {
        match self.outbound.try_send(out) {
            Ok(()) => true,
            Err(tokio::sync::mpsc::error::TrySendError::Full(_)) => {
                self.kill(AppErrorCode::SlowConsumer);
                false
            }
            Err(tokio::sync::mpsc::error::TrySendError::Closed(_)) => false,
        }
    }

    /// Bytes this connection has queued but not sent.
    #[must_use]
    pub fn unsent(&self) -> usize {
        self.world_unsent.load(Ordering::Relaxed)
            + self.chat_unsent.load(Ordering::Relaxed)
            + self.control_unsent.load(Ordering::Relaxed)
    }
}

/// A login verification for the hash workers (§6.2 step 3). `Debug` never shows the
/// password.
pub struct HashJob {
    /// The connection to answer.
    pub conn: ConnId,
    /// Canonical account name.
    pub account: String,
    /// The password (wiped by the worker).
    pub password: String,
    /// True for `Register`.
    pub register: bool,
}

impl std::fmt::Debug for HashJob {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("HashJob")
            .field("conn", &self.conn)
            .field("account", &self.account)
            .field("register", &self.register)
            .finish_non_exhaustive()
    }
}

/// The outcome of a [`HashJob`].
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum HashOutcome {
    /// The password matches a known account (or the registration succeeded).
    Verified,
    /// Wrong password or unknown account.
    BadCredentials,
    /// `Register` for a name that exists.
    AccountExists,
    /// `Register` refused: the accounts file is full (it would exceed the size every reader
    /// accepts).
    RegistrationFull,
    /// A local failure (file or hash error).
    Error,
}

/// A finished [`HashJob`].
#[derive(Debug, Clone)]
pub struct HashResult {
    /// The connection.
    pub conn: ConnId,
    /// Canonical account name.
    pub account: String,
    /// The outcome.
    pub outcome: HashOutcome,
    /// True for `Register`.
    pub register: bool,
}

/// A file write for the writer thread (§3.7, §12.2): one pending job per file; a newer job
/// replaces an older one.
#[derive(Debug)]
pub struct WriteJob {
    /// Target file.
    pub path: PathBuf,
    /// Full new contents.
    pub contents: Vec<u8>,
    /// File mode on Unix.
    pub mode: u32,
    /// Reported back to the simulation with [`SimInput::WriteDone`].
    pub token: Option<u64>,
}

/// Messages to the writer thread.
#[derive(Debug)]
pub enum WriterMsg {
    /// Write a file.
    Write(WriteJob),
    /// Reply once every earlier job is done: the number of failed writes since the last
    /// barrier.
    Barrier(std::sync::mpsc::Sender<usize>),
    /// Finish pending jobs and stop.
    Stop,
}

/// Commands for the simulation from the server handle (shutdown, §12.1).
#[derive(Debug)]
pub enum SimControl {
    /// `ServerNotice(Shutdown, seconds)` to every logged-in connection.
    ShutdownNotice {
        /// Seconds until the server stops.
        seconds: u16,
    },
    /// Stop: save every character in the world and the world state, wait for the writer,
    /// send `Disconnect(SERVER_SHUTDOWN)` to every connection, and reply.
    Stop(std::sync::mpsc::Sender<StopReport>),
}

/// What the simulation did when it stopped.
#[derive(Debug, Clone, Copy, Default, PartialEq, Eq)]
pub struct StopReport {
    /// Characters saved.
    pub saved: usize,
    /// Failed writes (characters or world state).
    pub failures: usize,
}

/// Input to the simulation thread.
#[derive(Debug)]
pub enum SimInput {
    /// A connection completed the handshake and was admitted.
    Opened(std::sync::Arc<ConnShared>),
    /// A control event (never `Move`, which goes through [`ConnShared::put_move`]).
    Event(ConnId, ServerEvent),
    /// The connection is over.
    Closed(ConnId),
    /// A login verification finished.
    Hash(HashResult),
    /// A write with a token finished.
    WriteDone {
        /// The token.
        token: u64,
        /// Whether it succeeded.
        ok: bool,
    },
    /// A command from the server handle.
    Control(SimControl),
}

/// Live counters (`ServerHandle::stats`, `test-server.md` §11.1, §13.3).
#[derive(Debug, Default)]
pub struct Stats {
    /// Connections past the QUIC handshake (admitted or not yet refused).
    pub connections: AtomicUsize,
    /// QUIC handshakes in progress.
    pub handshakes: AtomicUsize,
    /// Open network tasks (one per connection being handled).
    pub net_tasks: AtomicUsize,
    /// Connections known to the simulation.
    pub sim_connections: AtomicUsize,
    /// Logged-in connections.
    pub logged_in: AtomicUsize,
    /// Characters in the world.
    pub in_world: AtomicUsize,
    /// Entities (players and wanderers).
    pub entities: AtomicUsize,
    /// Interest pairs (entities created on connections).
    pub interest_pairs: AtomicUsize,
    /// Accounted memory, bytes (§10.2).
    pub accounted_bytes: AtomicUsize,
    /// Bytes queued but unsent over all connections.
    pub queue_bytes: AtomicUsize,
    /// Control events waiting for the simulation.
    pub inbound_queued: AtomicUsize,
    /// Messages received (control frames and datagrams).
    pub messages_in: AtomicU64,
    /// Messages sent (frames and datagrams).
    pub messages_out: AtomicU64,
    /// Bytes received on streams and datagrams.
    pub bytes_in: AtomicU64,
    /// Bytes sent on streams and datagrams.
    pub bytes_out: AtomicU64,
    /// Simulation ticks run.
    pub ticks: AtomicU64,
    /// Tick overruns.
    pub overruns: AtomicU64,
    /// Ticks skipped after falling more than 5 behind.
    pub skipped_ticks: AtomicU64,
    /// Longest tick since the server started, µs (soak and load reports).
    pub tick_max_us: AtomicU64,
    /// Longest tick since the last statistics line, µs (§11.1); [`StatsInterval::close`]
    /// resets it.
    pub tick_max_interval_us: AtomicU64,
    /// Last tick's duration, µs.
    pub tick_last_us: AtomicU64,
    /// Movement corrections sent.
    pub corrections: AtomicU64,
    /// Movement rejected without a report (over 4 per second).
    pub rejections_dropped: AtomicU64,
    /// Accepted movement.
    pub moves_accepted: AtomicU64,
    /// Connections closed for protocol violations.
    pub violations: AtomicU64,
    /// Connections closed as slow consumers.
    pub slow_consumers: AtomicU64,
    /// Connections refused with `SERVER_FULL`.
    pub server_full: AtomicU64,
    /// Chat lines delivered (per recipient).
    pub chat_delivered: AtomicU64,
    /// Login verifications waiting or running.
    pub hash_jobs: AtomicUsize,
    /// Writer jobs queued.
    pub writer_jobs: AtomicUsize,
    /// Address-budget and lockout table entries.
    pub throttle_entries: AtomicUsize,
    /// Cached character files.
    pub character_files: AtomicUsize,
}

/// A copy of [`Stats`] at one moment.
#[derive(Debug, Clone, Copy, Default, PartialEq, Eq)]
#[allow(missing_docs)]
pub struct StatsSnapshot {
    pub connections: usize,
    pub handshakes: usize,
    pub net_tasks: usize,
    pub sim_connections: usize,
    pub logged_in: usize,
    pub in_world: usize,
    pub entities: usize,
    pub interest_pairs: usize,
    pub accounted_bytes: usize,
    pub queue_bytes: usize,
    pub inbound_queued: usize,
    pub messages_in: u64,
    pub messages_out: u64,
    pub bytes_in: u64,
    pub bytes_out: u64,
    pub ticks: u64,
    pub overruns: u64,
    pub skipped_ticks: u64,
    pub tick_max_us: u64,
    pub tick_last_us: u64,
    pub corrections: u64,
    pub rejections_dropped: u64,
    pub moves_accepted: u64,
    pub violations: u64,
    pub slow_consumers: u64,
    pub server_full: u64,
    pub chat_delivered: u64,
    pub hash_jobs: usize,
    pub writer_jobs: usize,
    pub throttle_entries: usize,
    pub character_files: usize,
}

impl Stats {
    /// Records the duration of one tick: the last, the longest since start and the longest
    /// of the current statistics interval.
    pub fn record_tick(&self, took_us: u64) {
        self.ticks.fetch_add(1, Ordering::Relaxed);
        self.tick_last_us.store(took_us, Ordering::Relaxed);
        self.tick_max_us.fetch_max(took_us, Ordering::Relaxed);
        self.tick_max_interval_us
            .fetch_max(took_us, Ordering::Relaxed);
    }

    /// A snapshot of every counter.
    #[must_use]
    pub fn snapshot(&self) -> StatsSnapshot {
        let u = |a: &AtomicUsize| a.load(Ordering::Relaxed);
        let q = |a: &AtomicU64| a.load(Ordering::Relaxed);
        StatsSnapshot {
            connections: u(&self.connections),
            handshakes: u(&self.handshakes),
            net_tasks: u(&self.net_tasks),
            sim_connections: u(&self.sim_connections),
            logged_in: u(&self.logged_in),
            in_world: u(&self.in_world),
            entities: u(&self.entities),
            interest_pairs: u(&self.interest_pairs),
            accounted_bytes: u(&self.accounted_bytes),
            queue_bytes: u(&self.queue_bytes),
            inbound_queued: u(&self.inbound_queued),
            messages_in: q(&self.messages_in),
            messages_out: q(&self.messages_out),
            bytes_in: q(&self.bytes_in),
            bytes_out: q(&self.bytes_out),
            ticks: q(&self.ticks),
            overruns: q(&self.overruns),
            skipped_ticks: q(&self.skipped_ticks),
            tick_max_us: q(&self.tick_max_us),
            tick_last_us: q(&self.tick_last_us),
            corrections: q(&self.corrections),
            rejections_dropped: q(&self.rejections_dropped),
            moves_accepted: q(&self.moves_accepted),
            violations: q(&self.violations),
            slow_consumers: q(&self.slow_consumers),
            server_full: q(&self.server_full),
            chat_delivered: q(&self.chat_delivered),
            hash_jobs: u(&self.hash_jobs),
            writer_jobs: u(&self.writer_jobs),
            throttle_entries: u(&self.throttle_entries),
            character_files: u(&self.character_files),
        }
    }
}

/// The figures of a statistics line that cover the interval since the previous line
/// (`test-server.md` §11.1).
#[derive(Debug, Clone, Copy, Default, PartialEq, Eq)]
pub struct IntervalFigures {
    /// Longest tick over the interval, µs.
    pub tick_max_us: u64,
    /// Bytes received per second over the interval.
    pub bytes_in_per_s: u64,
    /// Bytes sent per second over the interval.
    pub bytes_out_per_s: u64,
}

/// Where the current statistics interval started: server time and the byte counters then.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct StatsInterval {
    start: u64,
    bytes_in: u64,
    bytes_out: u64,
}

impl StatsInterval {
    /// An interval starting at server time `now` (µs).
    #[must_use]
    pub fn new(stats: &Stats, now: u64) -> Self {
        Self {
            start: now,
            bytes_in: stats.bytes_in.load(Ordering::Relaxed),
            bytes_out: stats.bytes_out.load(Ordering::Relaxed),
        }
    }

    /// Server time (µs) at which the interval started.
    #[must_use]
    pub fn start(&self) -> u64 {
        self.start
    }

    /// Ends the interval at server time `now` (µs), returns its figures and starts the next
    /// one: resets the interval's longest tick and remembers the byte counters.
    pub fn close(&mut self, stats: &Stats, now: u64) -> IntervalFigures {
        let tick_max_us = stats.tick_max_interval_us.swap(0, Ordering::Relaxed);
        let next = Self::new(stats, now);
        let elapsed = now.saturating_sub(self.start);
        let rate = |before: u64, after: u64| -> u64 {
            if elapsed == 0 {
                return 0;
            }
            let delta = u128::from(after.saturating_sub(before));
            u64::try_from(delta * 1_000_000 / u128::from(elapsed)).unwrap_or(u64::MAX)
        };
        let figures = IntervalFigures {
            tick_max_us,
            bytes_in_per_s: rate(self.bytes_in, next.bytes_in),
            bytes_out_per_s: rate(self.bytes_out, next.bytes_out),
        };
        *self = next;
        figures
    }
}

/// One statistics line (`test-server.md` §11.1): the counts at the time of the line and the
/// figures of the interval it closes (§14 invariant 9).
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct StatsLine {
    /// Server time (µs) at which the interval started.
    pub interval_start_us: u64,
    /// Server time (µs) of the line: the start of the tick that wrote it, and the start of
    /// the next interval.
    pub at_us: u64,
    /// Connections the simulation knows.
    pub connections: usize,
    /// Of them, those past login (`LoginOk` sent).
    pub logged_in: usize,
    /// Of those, the ones in the world.
    pub in_world: usize,
    /// Characters in the world per instance, in instance order.
    pub per_instance: Vec<u32>,
    /// Entities in all instances (characters and wanderers).
    pub entities: usize,
    /// (observer, entity) pairs in interest.
    pub interest_pairs: usize,
    /// Duration of the most recently completed tick, µs.
    pub tick_last_us: u64,
    /// The interval's figures: its longest tick and byte rates.
    pub interval: IntervalFigures,
    /// Ticks that overran, since the server started.
    pub overruns: u64,
    /// The §10.2 accounted total, bytes.
    pub accounted_bytes: usize,
    /// Resident memory, bytes, where the platform reports it.
    pub resident_bytes: Option<u64>,
}

impl StatsLine {
    /// Writes the line at info level.
    pub fn log(&self) {
        tracing::info!(
            connections = self.connections,
            logged_in = self.logged_in,
            in_world = self.in_world,
            per_instance = ?self.per_instance,
            entities = self.entities,
            interest_pairs = self.interest_pairs,
            tick_last_us = self.tick_last_us,
            tick_max_us = self.interval.tick_max_us,
            overruns = self.overruns,
            bytes_in_per_s = self.interval.bytes_in_per_s,
            bytes_out_per_s = self.interval.bytes_out_per_s,
            accounted_bytes = self.accounted_bytes,
            resident_bytes = self.resident_bytes,
            "stats"
        );
    }
}

/// This process's resident memory in bytes, where the platform reports it (`VmRSS` of
/// Linux's `/proc/self/status`); `None` elsewhere or when it cannot be read.
#[must_use]
pub fn resident_bytes() -> Option<u64> {
    #[cfg(target_os = "linux")]
    {
        let text = std::fs::read_to_string("/proc/self/status").ok()?;
        status_kib(&text, "VmRSS")
    }
    #[cfg(not(target_os = "linux"))]
    {
        None
    }
}

/// A `/proc/*/status` field given in kB, as bytes.
#[cfg_attr(not(target_os = "linux"), allow(dead_code))]
fn status_kib(text: &str, field: &str) -> Option<u64> {
    let rest = text
        .lines()
        .find_map(|l| l.strip_prefix(field)?.strip_prefix(':'))?;
    let mut parts = rest.split_whitespace();
    let n: u64 = parts.next()?.parse().ok()?;
    if parts.next() != Some("kB") {
        return None;
    }
    n.checked_mul(1024)
}

#[cfg(test)]
mod tests {
    use super::*;

    /// §11.1: the statistics line reports the longest tick over the interval (not since
    /// start) and bytes per second (not cumulative bytes).
    #[test]
    fn interval_figures_cover_only_the_interval() {
        let stats = Stats::default();
        stats.bytes_in.store(1_000, Ordering::Relaxed);
        stats.bytes_out.store(5_000, Ordering::Relaxed);
        let mut iv = StatsInterval::new(&stats, 10_000_000);
        stats.record_tick(900);
        stats.record_tick(300);
        stats.bytes_in.fetch_add(4_000, Ordering::Relaxed);
        stats.bytes_out.fetch_add(20_000, Ordering::Relaxed);
        // Two seconds later.
        let f = iv.close(&stats, 12_000_000);
        assert_eq!(
            f,
            IntervalFigures {
                tick_max_us: 900,
                bytes_in_per_s: 2_000,
                bytes_out_per_s: 10_000,
            }
        );
        // The next interval: shorter ticks only, a slower rate; the since-start maximum
        // keeps the long tick.
        stats.record_tick(200);
        stats.bytes_in.fetch_add(500, Ordering::Relaxed);
        let f = iv.close(&stats, 13_000_000);
        assert_eq!(
            f,
            IntervalFigures {
                tick_max_us: 200,
                bytes_in_per_s: 500,
                bytes_out_per_s: 0,
            }
        );
        assert_eq!(stats.snapshot().tick_max_us, 900);
        assert_eq!(stats.snapshot().tick_last_us, 200);
        assert_eq!(stats.snapshot().ticks, 3);
        // No tick and no time in between: zeros, no division by zero.
        assert_eq!(iv.close(&stats, 13_000_000), IntervalFigures::default());
    }

    /// §11.1: byte rates are rounded down.
    #[test]
    fn byte_rates_round_down() {
        let stats = Stats::default();
        let mut iv = StatsInterval::new(&stats, 0);
        stats.bytes_in.store(10, Ordering::Relaxed);
        stats.bytes_out.store(1, Ordering::Relaxed);
        // 10 bytes over 3 s: 3.33 → 3; 1 byte over 3 s: 0.33 → 0.
        let f = iv.close(&stats, 3_000_000);
        assert_eq!((f.bytes_in_per_s, f.bytes_out_per_s), (3, 0));
    }

    #[test]
    fn status_field_in_kib() {
        let t = "Name:\tx\nVmHWM:\t  2048 kB\nVmRSS:\t  1024 kB\n";
        assert_eq!(status_kib(t, "VmRSS"), Some(1024 * 1024));
        assert_eq!(status_kib(t, "VmSwap"), None);
        assert_eq!(status_kib("VmRSS:\t12 MB\n", "VmRSS"), None);
        assert_eq!(status_kib("VmRSS:\tx kB\n", "VmRSS"), None);
        #[cfg(target_os = "linux")]
        assert!(resident_bytes().is_some_and(|b| b > 0));
    }
}
