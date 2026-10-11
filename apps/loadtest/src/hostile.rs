// SPDX-License-Identifier: GPL-3.0-or-later
//! Hostile clients (ROADMAP M7 exit criterion 3, `tessera-protocol.md` §13): over real UDP
//! they send random datagrams, malformed, truncated and oversized frames, messages out of
//! handshake order, control and datagram floods, half-open handshakes and failed logins,
//! while the honest bots keep playing. The run then checks that the server did not panic,
//! that the honest bots saw nothing of it, and that memory stayed bounded.
//!
//! The attackers run on their own thread with a tokio `current_thread` runtime. Against a
//! loopback server they bind `127.0.0.2` when they can, so the server's per-address login
//! budget (`test-server.md` §6.2) they exhaust is not the honest bots' one.
//!
//! The protocol 1.2 cases (`test-server.md` §13.1 "Hostile 1.2 cases") are sent after login
//! in the world, with characters the run provisions for the attackers
//! ([`HostileConfig::characters`], one attacker per character at a time).

use std::collections::BTreeMap;
use std::net::{IpAddr, Ipv4Addr, Ipv6Addr, SocketAddr};
use std::sync::atomic::{AtomicBool, AtomicU64, Ordering};
use std::sync::{Arc, Mutex};
use std::thread::JoinHandle;
use std::time::Duration;

use tessera_protocol::codes::{AppErrorCode, ChannelRefusedCode, ChatRejectedCode};
use tessera_protocol::frame::{encode_datagram, frame_to_vec};
use tessera_protocol::message::{
    CharacterCreate, ChatChannel, ChatSend, Credentials, EnterWorld, MoveInput, ServerCaps,
};
use tessera_protocol::types::{Location, Motion, Vec3};
use tessera_protocol::{Message, MessageType};

use crate::LoadtestError;
use crate::bot::BotSpec;
use crate::raw::{self, ChannelsNeeded, ControlLink, Target, WorldClient};
use crate::rng::Rng;

/// The attacks, in the order the report lists them.
#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord)]
pub enum Attack {
    /// Random UDP datagrams (some shaped like QUIC long headers, some oversized).
    UdpGarbage,
    /// Random bytes on the control stream.
    StreamGarbage,
    /// A frame cut short: the stream ends or stalls mid-frame.
    TruncatedFrame,
    /// A frame whose length field exceeds 65,536 (or is below 2).
    OversizedFrame,
    /// Valid messages in an order the handshake forbids.
    OutOfOrder,
    /// Hundreds of control messages at once.
    ControlFlood,
    /// Thousands of datagrams at once, random or `MoveInput` before login.
    DatagramFlood,
    /// QUIC connections that never say `Hello`.
    HandshakeHold,
    /// Repeated logins to unknown accounts with wrong passwords.
    BadLogin,
    /// A client-opened unidirectional stream with junk.
    UniStream,
    /// The client-to-server negative vectors of `tessera-protocol.md` §16.8 (protocol 1.1:
    /// looks and slots), one per attack in turn (`test-server.md` §13.1).
    ///
    /// The attackers hold no account, so each vector follows `Hello` directly, before any
    /// login: the server must survive it and close with `PROTOCOL_VIOLATION`, but a
    /// `CharacterCreate` or `AppearanceSet` is already out of its state there. Whether the
    /// server's v1.1 decoding refuses each vector in the message's own state is checked by
    /// the `hostile_v11_vectors_are_protocol_violations` test (`tests/v11_looks.rs`), which
    /// logs in first.
    MinorOneVectors,
    /// The client-to-server negative vectors of `tessera-protocol.md` §16.9 (protocol 1.2:
    /// named channels), one per attack in turn, each sent **after login in the world**
    /// (`test-server.md` §13.1) with one of the run's hostile characters; each must end
    /// the connection with `PROTOCOL_VIOLATION`. The two vectors that need a server
    /// without `CHAT_CHANNELS` are skipped against a server that advertises it (and the
    /// others against one that does not); the `hostile_v12_cases` test
    /// (`tests/v12_channels.rs`) sends all of them against two servers.
    MinorTwoVectors,
    /// The non-fatal 1.2 cases of `test-server.md` §13.1, in the world at minor 2: an empty
    /// name, a control character, a name that breaks the channel-name rule, a leave of a
    /// channel not joined and a line on it (one variant), or a join, a second join of the
    /// same key and a leave twice (the other variant), each with its `ChannelRefused` or
    /// `ChatRejected(NOT_JOINED)` code and the connection left open; then 30 join requests
    /// in a burst, refused `RATE_LIMITED` until the flood rule ends the session with
    /// `FLOODING`. The joins beyond `max_channels_per_character`, `max_channel_members` and
    /// `max_channels` need limits the honest run must not have and more requests than the
    /// request bucket gives one attack; the `hostile_v12_cases` test sends them.
    MinorTwoRefusals,
}

impl Attack {
    /// Every attack.
    pub const ALL: [Self; 13] = [
        Self::UdpGarbage,
        Self::StreamGarbage,
        Self::TruncatedFrame,
        Self::OversizedFrame,
        Self::OutOfOrder,
        Self::ControlFlood,
        Self::DatagramFlood,
        Self::HandshakeHold,
        Self::BadLogin,
        Self::UniStream,
        Self::MinorOneVectors,
        Self::MinorTwoVectors,
        Self::MinorTwoRefusals,
    ];

    /// Short name for the report.
    #[must_use]
    pub fn name(self) -> &'static str {
        match self {
            Self::UdpGarbage => "udp-garbage",
            Self::StreamGarbage => "stream-garbage",
            Self::TruncatedFrame => "truncated-frame",
            Self::OversizedFrame => "oversized-frame",
            Self::OutOfOrder => "out-of-order",
            Self::ControlFlood => "control-flood",
            Self::DatagramFlood => "datagram-flood",
            Self::HandshakeHold => "handshake-hold",
            Self::BadLogin => "bad-login",
            Self::UniStream => "uni-stream",
            Self::MinorOneVectors => "v11-vectors",
            Self::MinorTwoVectors => "v12-vectors",
            Self::MinorTwoRefusals => "v12-refusals",
        }
    }
}

/// Hostile-mode parameters.
#[derive(Debug, Clone)]
pub struct HostileConfig {
    /// The server.
    pub target: Target,
    /// Concurrent attackers.
    pub clients: usize,
    /// Pause between two attacks of one attacker.
    pub pace: Duration,
    /// Seed.
    pub seed: u64,
    /// Characters the attackers log in with for the 1.2 cases (empty: those cases are
    /// skipped and counted).
    pub characters: Vec<BotSpec>,
}

/// What the attackers did, counted.
#[derive(Debug, Clone, Default, PartialEq, Eq)]
pub struct HostileSnapshot {
    /// Attacks run, by name.
    pub attacks: BTreeMap<&'static str, u64>,
    /// QUIC connections that could not be established (refused or timed out: expected
    /// under load, the server may refuse handshakes).
    pub connect_failures: u64,
    /// Connections the server closed, by application code name (or the transport error).
    pub closed_by_server: BTreeMap<String, u64>,
    /// Connections still open when the attacker gave up waiting (it then closed them).
    pub left_open: u64,
    /// UDP datagrams sent outside QUIC.
    pub udp_datagrams: u64,
    /// The address the attackers bound.
    pub bind: Option<IpAddr>,
    /// 1.2 cases sent in the world.
    pub v12_in_world: u64,
    /// 1.2 cases not sent: no free hostile character, a world entry that failed, or a
    /// vector the server's channel setting cannot take.
    pub v12_skipped: u64,
    /// 1.2 cases the server answered otherwise than the spec says (a run failure).
    pub v12_wrong: u64,
    /// The first of those, in words.
    pub v12_wrong_examples: Vec<String>,
}

#[derive(Debug, Default)]
struct Counters {
    attacks: Mutex<BTreeMap<&'static str, u64>>,
    connect_failures: AtomicU64,
    closed: Mutex<BTreeMap<String, u64>>,
    left_open: AtomicU64,
    udp: AtomicU64,
    /// Indexes of the hostile characters not in use.
    free: Mutex<Vec<usize>>,
    v12_in_world: AtomicU64,
    v12_skipped: AtomicU64,
    v12_wrong: AtomicU64,
    v12_wrong_examples: Mutex<Vec<String>>,
}

/// Most wrong 1.2 answers described in the snapshot.
const MAX_WRONG_EXAMPLES: usize = 16;

impl Counters {
    fn bump_attack(&self, a: Attack) {
        if let Ok(mut m) = self.attacks.lock() {
            *m.entry(a.name()).or_insert(0) += 1;
        }
    }

    fn closed(&self, why: String) {
        if let Ok(mut m) = self.closed.lock()
            && (m.len() < 64 || m.contains_key(&why))
        {
            *m.entry(why).or_insert(0) += 1;
        }
    }

    fn wrong(&self, what: String) {
        tracing::warn!(%what, "hostile 1.2 case answered wrongly");
        self.v12_wrong.fetch_add(1, Ordering::Relaxed);
        if let Ok(mut v) = self.v12_wrong_examples.lock()
            && v.len() < MAX_WRONG_EXAMPLES
        {
            v.push(what);
        }
    }

    fn skipped(&self) {
        self.v12_skipped.fetch_add(1, Ordering::Relaxed);
    }
}

/// A hostile character in use by one attack; given back when dropped (also when the
/// attack is cut short by [`ATTACK_LIMIT`]).
struct Borrowed {
    counters: Arc<Counters>,
    index: usize,
}

impl Drop for Borrowed {
    fn drop(&mut self) {
        if let Ok(mut f) = self.counters.free.lock() {
            f.push(self.index);
        }
    }
}

/// Running attackers; stop them with [`stop`](Self::stop).
#[derive(Debug)]
pub struct HostileRun {
    stop: Arc<AtomicBool>,
    counters: Arc<Counters>,
    bind: IpAddr,
    thread: Option<JoinHandle<()>>,
}

/// The local address the attackers bind for `server`.
fn bind_ip(server: SocketAddr) -> IpAddr {
    match server.ip() {
        IpAddr::V4(a) if a.is_loopback() => {
            let alt = Ipv4Addr::new(127, 0, 0, 2);
            if std::net::UdpSocket::bind((alt, 0)).is_ok() {
                IpAddr::V4(alt)
            } else {
                IpAddr::V4(Ipv4Addr::LOCALHOST)
            }
        }
        IpAddr::V4(_) => IpAddr::V4(Ipv4Addr::UNSPECIFIED),
        IpAddr::V6(a) if a.is_loopback() => IpAddr::V6(Ipv6Addr::LOCALHOST),
        IpAddr::V6(_) => IpAddr::V6(Ipv6Addr::UNSPECIFIED),
    }
}

impl HostileRun {
    /// Starts `config.clients` attackers on a new thread.
    ///
    /// # Errors
    ///
    /// [`LoadtestError::Runtime`] when the thread or runtime cannot start.
    pub fn start(config: HostileConfig) -> Result<Self, LoadtestError> {
        let stop = Arc::new(AtomicBool::new(false));
        let counters = Arc::new(Counters {
            free: Mutex::new((0..config.characters.len()).rev().collect()),
            ..Counters::default()
        });
        let characters = Arc::new(config.characters.clone());
        let bind = bind_ip(config.target.addr);
        let rt = tokio::runtime::Builder::new_current_thread()
            .enable_all()
            .build()
            .map_err(|e| LoadtestError::Runtime(e.to_string()))?;
        let (s, c) = (Arc::clone(&stop), Arc::clone(&counters));
        let thread = std::thread::Builder::new()
            .name("loadtest-hostile".into())
            .spawn(move || {
                rt.block_on(async move {
                    let mut seed = Rng::new(config.seed);
                    let mut tasks = Vec::new();
                    for _ in 0..config.clients.max(1) {
                        let ctx = Attacker {
                            target: config.target.clone(),
                            bind,
                            stop: Arc::clone(&s),
                            counters: Arc::clone(&c),
                            rng: seed.fork(),
                            pace: config.pace,
                            next_vector: 0,
                            next_v12: 0,
                            refusal_variant: 0,
                            characters: Arc::clone(&characters),
                            server_channels: None,
                        };
                        tasks.push(tokio::spawn(ctx.run()));
                    }
                    for t in tasks {
                        let _ = t.await;
                    }
                });
            })
            .map_err(|e| LoadtestError::Runtime(e.to_string()))?;
        Ok(Self {
            stop,
            counters,
            bind,
            thread: Some(thread),
        })
    }

    /// The counts so far.
    #[must_use]
    pub fn snapshot(&self) -> HostileSnapshot {
        let c = &self.counters;
        HostileSnapshot {
            attacks: c.attacks.lock().map(|m| m.clone()).unwrap_or_default(),
            connect_failures: c.connect_failures.load(Ordering::Relaxed),
            closed_by_server: c.closed.lock().map(|m| m.clone()).unwrap_or_default(),
            left_open: c.left_open.load(Ordering::Relaxed),
            udp_datagrams: c.udp.load(Ordering::Relaxed),
            bind: Some(self.bind),
            v12_in_world: c.v12_in_world.load(Ordering::Relaxed),
            v12_skipped: c.v12_skipped.load(Ordering::Relaxed),
            v12_wrong: c.v12_wrong.load(Ordering::Relaxed),
            v12_wrong_examples: c
                .v12_wrong_examples
                .lock()
                .map(|v| v.clone())
                .unwrap_or_default(),
        }
    }

    /// Tells the attackers to stop after their current attack, without waiting.
    pub fn signal_stop(&self) {
        self.stop.store(true, Ordering::Release);
    }

    /// Stops the attackers (each finishes its current attack, at most [`ATTACK_LIMIT`];
    /// the pause between attacks is cut short) and returns the final counts.
    #[must_use]
    pub fn stop(mut self) -> HostileSnapshot {
        self.stop.store(true, Ordering::Release);
        if let Some(t) = self.thread.take() {
            let _ = t.join();
        }
        self.snapshot()
    }
}

impl Drop for HostileRun {
    fn drop(&mut self) {
        self.stop.store(true, Ordering::Release);
        if let Some(t) = self.thread.take() {
            let _ = t.join();
        }
    }
}

struct Attacker {
    target: Target,
    bind: IpAddr,
    stop: Arc<AtomicBool>,
    counters: Arc<Counters>,
    rng: Rng,
    pace: Duration,
    /// The next vector of [`Attack::MinorOneVectors`] (§16.8).
    next_vector: usize,
    /// The next vector of [`Attack::MinorTwoVectors`] (§16.9).
    next_v12: usize,
    /// The next variant of [`Attack::MinorTwoRefusals`].
    refusal_variant: usize,
    /// The run's hostile characters.
    characters: Arc<Vec<BotSpec>>,
    /// Whether the server advertises `CHAT_CHANNELS` at minor 2, once seen.
    server_channels: Option<bool>,
}

/// One non-fatal channel request of [`Attack::MinorTwoRefusals`]: message type, name bytes
/// and the expected refusal code (`None`: answered `ChannelJoined` or `ChannelLeft`).
type ChannelCase = (u16, Vec<u8>, Option<ChannelRefusedCode>);

/// A world entry of a hostile character must complete within this time.
const ENTER_WAIT: Duration = Duration::from_secs(5);
/// How long a 1.2 case waits for each answer.
const ANSWER_WAIT: Duration = Duration::from_secs(3);

/// Longest any one attack may take.
pub const ATTACK_LIMIT: Duration = Duration::from_secs(15);
/// The pause between attacks is slept in slices of this length, rechecking the stop flag.
const PAUSE_SLICE: Duration = Duration::from_millis(100);
/// Longest pause between two attacks of one attacker ([`HostileConfig::pace`] is clamped).
pub const MAX_PACE: Duration = Duration::from_secs(10);
/// How long an attacker waits for the server to close a connection it offended.
const CLOSE_WAIT: Duration = Duration::from_secs(3);

impl Attacker {
    async fn run(mut self) {
        let endpoint = match raw::endpoint(SocketAddr::new(self.bind, 0)) {
            Ok(e) => e,
            Err(e) => {
                tracing::warn!(error = %e, "hostile: no endpoint");
                return;
            }
        };
        while !self.stop.load(Ordering::Acquire) {
            let attack = Attack::ALL[self.rng.below(Attack::ALL.len() as u64) as usize];
            self.counters.bump_attack(attack);
            let _ = tokio::time::timeout(ATTACK_LIMIT, self.attack(&endpoint, attack)).await;
            let pause = self.pace.min(MAX_PACE).mul_f64(self.rng.range(0.5, 1.5));
            self.pause(pause).await;
        }
        endpoint.close(0u32.into(), b"");
        let _ = tokio::time::timeout(Duration::from_secs(2), endpoint.wait_idle()).await;
    }

    /// Sleeps for `d`, or less once the attackers are told to stop.
    async fn pause(&self, d: Duration) {
        let end = tokio::time::Instant::now() + d;
        while !self.stop.load(Ordering::Acquire) {
            let now = tokio::time::Instant::now();
            if now >= end {
                break;
            }
            tokio::time::sleep((end - now).min(PAUSE_SLICE)).await;
        }
    }

    async fn link(&mut self, endpoint: &quinn::Endpoint) -> Option<ControlLink> {
        match raw::connect(endpoint, &self.target, Duration::from_secs(5)).await {
            Ok(c) => match ControlLink::open(c).await {
                Ok(l) => Some(l),
                Err(_) => {
                    self.counters
                        .connect_failures
                        .fetch_add(1, Ordering::Relaxed);
                    None
                }
            },
            Err(_) => {
                self.counters
                    .connect_failures
                    .fetch_add(1, Ordering::Relaxed);
                None
            }
        }
    }

    /// Waits for the server to close `conn`, records how, and closes it ourselves if it
    /// did not.
    async fn settle(&self, conn: &quinn::Connection, wait: Duration) -> Option<String> {
        match tokio::time::timeout(wait, conn.closed()).await {
            Ok(e) => {
                let why = match e {
                    quinn::ConnectionError::ApplicationClosed(a) => {
                        let code = u16::try_from(a.error_code.into_inner())
                            .map(tessera_protocol::codes::AppErrorCode::from_u16)
                            .map_or("OTHER", |c| c.name());
                        code.to_owned()
                    }
                    quinn::ConnectionError::ConnectionClosed(_) => "transport-close".into(),
                    quinn::ConnectionError::TimedOut => "idle-timeout".into(),
                    quinn::ConnectionError::Reset => "reset".into(),
                    quinn::ConnectionError::LocallyClosed => "local".into(),
                    _ => "other".into(),
                };
                self.counters.closed(why.clone());
                Some(why)
            }
            Err(_) => {
                self.counters.left_open.fetch_add(1, Ordering::Relaxed);
                conn.close(0u32.into(), b"");
                None
            }
        }
    }

    async fn attack(&mut self, endpoint: &quinn::Endpoint, attack: Attack) {
        match attack {
            Attack::UdpGarbage => self.udp_garbage().await,
            Attack::StreamGarbage => {
                let Some(mut l) = self.link(endpoint).await else {
                    return;
                };
                let n = 1 + self.rng.below(4096) as usize;
                let junk = self.rng.bytes(n);
                let _ = l.send_raw(&junk).await;
                self.settle(&l.conn, CLOSE_WAIT).await;
            }
            Attack::TruncatedFrame => {
                let Some(mut l) = self.link(endpoint).await else {
                    return;
                };
                let Ok(full) = frame_to_vec(&raw::hello()) else {
                    return;
                };
                let cut = 1 + self.rng.below(full.len().saturating_sub(1).max(1) as u64) as usize;
                let _ = l.send_raw(&full[..cut.min(full.len() - 1)]).await;
                if self.rng.chance(0.5) {
                    // End the stream mid-frame.
                    l.finish();
                    self.settle(&l.conn, CLOSE_WAIT).await;
                } else {
                    // Stall mid-frame; now and then long enough for the Hello timeout.
                    let hold = if self.rng.chance(0.2) {
                        11.5
                    } else {
                        self.rng.range(0.2, 2.0)
                    };
                    self.settle(&l.conn, Duration::from_secs_f64(hold)).await;
                }
            }
            Attack::OversizedFrame => {
                let Some(mut l) = self.link(endpoint).await else {
                    return;
                };
                let len: u32 = match self.rng.below(4) {
                    0 => 65_537,
                    1 => u32::MAX,
                    2 => 1,
                    _ => 0,
                };
                let mut b = len.to_le_bytes().to_vec();
                let n = self.rng.below(2048) as usize;
                b.extend(self.rng.bytes(n));
                let _ = l.send_raw(&b).await;
                self.settle(&l.conn, CLOSE_WAIT).await;
            }
            Attack::OutOfOrder => self.out_of_order(endpoint).await,
            Attack::ControlFlood => {
                let Some(mut l) = self.link(endpoint).await else {
                    return;
                };
                let _ = l.send(&raw::hello()).await;
                let mut burst = Vec::new();
                for i in 0..600u64 {
                    let m = if i % 2 == 0 {
                        Message::Ping { nonce: i }
                    } else {
                        Message::ChatSend(ChatSend {
                            channel: ChatChannel::Say,
                            text: "flood".into(),
                        })
                    };
                    if let Ok(b) = frame_to_vec(&m) {
                        burst.extend(b);
                    }
                }
                let _ = l.send_raw(&burst).await;
                self.settle(&l.conn, CLOSE_WAIT).await;
            }
            Attack::DatagramFlood => {
                let Some(mut l) = self.link(endpoint).await else {
                    return;
                };
                let _ = l.send(&raw::hello()).await;
                let mv = encode_datagram(&Message::MoveInput(MoveInput {
                    seq: 1,
                    epoch: 1,
                    location: Location::new(0, 20_400.0, 20_400.0, 0.0),
                    heading: 0.0,
                    velocity: Vec3 {
                        x: 0.0,
                        y: 0.0,
                        z: 0.0,
                    },
                    motion: Motion::STANDING,
                }))
                .unwrap_or_default();
                for i in 0..2_000u32 {
                    let d = if i % 3 == 0 && !mv.is_empty() {
                        mv.clone()
                    } else {
                        let n = 1 + self.rng.below(1_150) as usize;
                        self.rng.bytes(n)
                    };
                    let _ = l.conn.send_datagram(d.into());
                    if i % 200 == 0 {
                        tokio::task::yield_now().await;
                    }
                }
                self.settle(&l.conn, Duration::from_millis(500)).await;
            }
            Attack::HandshakeHold => {
                let n = 1 + self.rng.below(4);
                let mut conns = Vec::new();
                for _ in 0..n {
                    match raw::connect(endpoint, &self.target, Duration::from_secs(5)).await {
                        Ok(c) => conns.push(c),
                        Err(_) => {
                            self.counters
                                .connect_failures
                                .fetch_add(1, Ordering::Relaxed);
                        }
                    }
                }
                let hold = if self.rng.chance(0.2) {
                    11.5
                } else {
                    self.rng.range(0.5, 3.0)
                };
                tokio::time::sleep(Duration::from_secs_f64(hold)).await;
                for c in conns {
                    self.settle(&c, Duration::from_millis(10)).await;
                }
            }
            Attack::BadLogin => {
                let Some(mut l) = self.link(endpoint).await else {
                    return;
                };
                let _ = l.send(&raw::hello()).await;
                for _ in 0..4 {
                    let account = format!("hx-{:012x}", self.rng.next_u64() & 0xFFFF_FFFF_FFFF);
                    let password = format!("{:016x}", self.rng.next_u64());
                    if l.send(&Message::Login(Credentials { account, password }))
                        .await
                        .is_err()
                    {
                        break;
                    }
                    let _ = l.recv(Duration::from_millis(1_500)).await;
                }
                self.settle(&l.conn, CLOSE_WAIT).await;
            }
            Attack::MinorOneVectors => {
                let vectors = raw::negative_vectors_v11();
                let Some(v) = vectors.get(self.next_vector % vectors.len().max(1)) else {
                    return;
                };
                self.next_vector = self.next_vector.wrapping_add(1);
                let Some(mut l) = self.link(endpoint).await else {
                    return;
                };
                let _ = l.send(&raw::hello_minor(v.minor_max)).await;
                let _ = l.send_raw(&v.frame).await;
                self.settle(&l.conn, CLOSE_WAIT).await;
            }
            Attack::MinorTwoVectors => self.v12_vector(endpoint).await,
            Attack::MinorTwoRefusals => self.v12_refusals(endpoint).await,
            Attack::UniStream => {
                let Ok(conn) = raw::connect(endpoint, &self.target, Duration::from_secs(5)).await
                else {
                    self.counters
                        .connect_failures
                        .fetch_add(1, Ordering::Relaxed);
                    return;
                };
                if let Ok(Ok(mut s)) =
                    tokio::time::timeout(Duration::from_secs(1), conn.open_uni()).await
                {
                    let n = 1 + self.rng.below(512) as usize;
                    let junk = self.rng.bytes(n);
                    let _ = tokio::time::timeout(Duration::from_secs(1), s.write_all(&junk)).await;
                }
                self.settle(&conn, CLOSE_WAIT).await;
            }
        }
    }

    /// Takes a free hostile character, if any.
    fn borrow(&self) -> Option<(Borrowed, BotSpec)> {
        let index = self.counters.free.lock().ok()?.pop()?;
        let guard = Borrowed {
            counters: Arc::clone(&self.counters),
            index,
        };
        let spec = self.characters.get(index)?.clone();
        Some((guard, spec))
    }

    /// Logs `spec` in at `minor_max` and enters the world; a failure is counted as skipped.
    async fn enter(
        &self,
        endpoint: &quinn::Endpoint,
        spec: &BotSpec,
        minor_max: u16,
    ) -> Option<WorldClient> {
        match WorldClient::enter(
            endpoint,
            &self.target,
            (&spec.account, spec.password.expose()),
            &spec.character,
            minor_max,
            ENTER_WAIT,
        )
        .await
        {
            Ok(c) => Some(c),
            Err(e) => {
                tracing::debug!(error = %e, "hostile: world entry failed");
                self.counters.skipped();
                None
            }
        }
    }

    /// Learns the server's channel setting from a minor-2 `HelloAck`.
    fn learn_channels(&mut self, c: &WorldClient) -> Option<bool> {
        (c.minor() >= 2).then(|| {
            let on = c.server_caps().contains(ServerCaps::CHAT_CHANNELS);
            self.server_channels = Some(on);
            on
        })
    }

    /// [`Attack::MinorTwoVectors`]: the next §16.9 vector this server can take, in the
    /// world; the server must close with `PROTOCOL_VIOLATION`.
    async fn v12_vector(&mut self, endpoint: &quinn::Endpoint) {
        let vectors = raw::negative_vectors_v12();
        let n = vectors.len();
        let mut pick = None;
        for _ in 0..n {
            let i = self.next_v12 % n.max(1);
            self.next_v12 = self.next_v12.wrapping_add(1);
            let Some((v, needs)) = vectors.get(i) else {
                break;
            };
            // Below minor 2 the server's channel setting does not matter.
            let fits = v.minor_max < 2
                || self
                    .server_channels
                    .is_none_or(|on| on == (*needs == ChannelsNeeded::On));
            if fits {
                pick = Some((v.clone(), *needs));
                break;
            }
        }
        let Some((v, needs)) = pick else {
            self.counters.skipped();
            return;
        };
        let Some((_guard, spec)) = self.borrow() else {
            self.counters.skipped();
            return;
        };
        let Some(mut client) = self.enter(endpoint, &spec, v.minor_max).await else {
            return;
        };
        if let Some(on) = self.learn_channels(&client)
            && on != (needs == ChannelsNeeded::On)
        {
            self.counters.skipped();
            client.close().await;
            return;
        }
        self.counters.v12_in_world.fetch_add(1, Ordering::Relaxed);
        let _ = client.link.send_raw(&v.frame).await;
        let why = self.settle(&client.link.conn, CLOSE_WAIT).await;
        if why.as_deref() != Some(AppErrorCode::ProtocolViolation.name()) {
            self.counters.wrong(format!(
                "{}: closed with {}",
                v.name,
                why.as_deref().unwrap_or("nothing (left open)")
            ));
        }
        client.close().await;
    }

    /// [`Attack::MinorTwoRefusals`]: non-fatal cases with their codes, then a burst of
    /// joins until the flood rule closes the session.
    async fn v12_refusals(&mut self, endpoint: &quinn::Endpoint) {
        if self.server_channels == Some(false) {
            self.counters.skipped();
            return;
        }
        let Some((_guard, spec)) = self.borrow() else {
            self.counters.skipped();
            return;
        };
        let Some(mut c) = self.enter(endpoint, &spec, 2).await else {
            return;
        };
        if self.learn_channels(&c) != Some(true) {
            self.counters.skipped();
            c.close().await;
            return;
        }
        self.counters.v12_in_world.fetch_add(1, Ordering::Relaxed);
        let tag = format!("hx{:08x}", self.rng.next_u64() & 0xFFFF_FFFF);
        let join = MessageType::ChannelJoin.code();
        let leave = MessageType::ChannelLeave.code();
        use ChannelRefusedCode as R;
        // At most five requests: the request bucket's capacity (tessera-protocol.md §10.6).
        let (cases, line): (Vec<ChannelCase>, bool) = if self.refusal_variant.is_multiple_of(2) {
            (
                vec![
                    (join, Vec::new(), Some(R::InvalidName)),
                    (join, b"Hxq".to_vec(), Some(R::InvalidName)),
                    (join, b"Hx!".to_vec(), Some(R::InvalidName)),
                    (leave, format!("{tag} out").into_bytes(), Some(R::NotJoined)),
                ],
                true,
            )
        } else {
            let name = format!("{tag} in");
            (
                vec![
                    (join, name.clone().into_bytes(), None),
                    (
                        join,
                        name.to_uppercase().into_bytes(),
                        Some(R::AlreadyJoined),
                    ),
                    (leave, name.clone().into_bytes(), None),
                    (leave, name.into_bytes(), Some(R::NotJoined)),
                ],
                false,
            )
        };
        self.refusal_variant = self.refusal_variant.wrapping_add(1);
        for (ty, name, want) in cases {
            let what = format!(
                "{} {:?}",
                if ty == join { "join" } else { "leave" },
                String::from_utf8_lossy(&name)
            );
            if c.link
                .send_raw(&raw::channel_request(ty, &name))
                .await
                .is_err()
            {
                self.counters.wrong(format!("{what}: connection closed"));
                c.close().await;
                return;
            }
            let got = c
                .expect_chat(ANSWER_WAIT, &mut Vec::new(), |m| match m {
                    Message::ChannelJoined { .. } | Message::ChannelLeft(_) => Some(None),
                    Message::ChannelRefused(r) => Some(Some(r.code)),
                    _ => None,
                })
                .await;
            match got {
                Ok(code) if code == want => {}
                Ok(code) => self
                    .counters
                    .wrong(format!("{what}: answered {code:?}, expected {want:?}")),
                Err(e) => {
                    self.counters.wrong(format!("{what}: {e}"));
                    c.close().await;
                    return;
                }
            }
        }
        if line {
            // A line on a channel not joined (§14.7 code 6); the chat bucket is separate.
            let send = c
                .link
                .send(&Message::ChatSend(ChatSend {
                    channel: ChatChannel::Named {
                        name: format!("{tag} out"),
                    },
                    text: "hi".into(),
                }))
                .await;
            let got = match send {
                Ok(()) => {
                    c.expect_chat(ANSWER_WAIT, &mut Vec::new(), |m| match m {
                        Message::ChatRejected(f) => Some(f.code),
                        _ => None,
                    })
                    .await
                }
                Err(e) => Err(e),
            };
            if got != Ok(ChatRejectedCode::NotJoined) {
                self.counters
                    .wrong(format!("line on a channel not joined: {got:?}"));
            }
        }
        if c.link.conn.close_reason().is_some() {
            self.counters
                .wrong("the connection closed before the burst".into());
            c.close().await;
            return;
        }
        // 30 join requests in a burst: RATE_LIMITED (or an answer while a token is left)
        // until the flood rule ends the session.
        let mut burst = Vec::new();
        for i in 0..30 {
            burst.extend(raw::channel_request(join, format!("{tag} b{i}").as_bytes()));
        }
        let _ = c.link.send_raw(&burst).await;
        let mut other = 0u32;
        while let Ok(Some(m)) = c.next_chat(CLOSE_WAIT).await {
            if let Message::ChannelRefused(r) = &m
                && r.code != R::RateLimited
            {
                other += 1;
            }
        }
        if other > 0 {
            self.counters
                .wrong(format!("burst: {other} refusals other than RATE_LIMITED"));
        }
        let why = self.settle(&c.link.conn, CLOSE_WAIT).await;
        if why.as_deref() != Some(AppErrorCode::Flooding.name()) {
            self.counters.wrong(format!(
                "burst: closed with {}",
                why.as_deref().unwrap_or("nothing (left open)")
            ));
        }
        c.close().await;
    }

    async fn udp_garbage(&mut self) {
        let Ok(sock) = tokio::net::UdpSocket::bind(SocketAddr::new(self.bind, 0)).await else {
            return;
        };
        for i in 0..64u32 {
            let d = match i % 8 {
                // A QUIC long header (Initial, version 1) followed by junk, padded.
                0 => {
                    let mut b = vec![0xC0 | (self.rng.below(16) as u8), 0, 0, 0, 1];
                    let n = 1_195 + self.rng.below(100) as usize;
                    b.extend(self.rng.bytes(n));
                    b
                }
                // A short header with junk.
                1 => {
                    let mut b = vec![0x40 | (self.rng.below(64) as u8)];
                    let n = 20 + self.rng.below(1_300) as usize;
                    b.extend(self.rng.bytes(n));
                    b
                }
                // Oversized for any QUIC path.
                2 => self.rng.bytes(9_000),
                _ => {
                    let n = 1 + self.rng.below(1_400) as usize;
                    self.rng.bytes(n)
                }
            };
            if sock.send_to(&d, self.target.addr).await.is_ok() {
                self.counters.udp.fetch_add(1, Ordering::Relaxed);
            }
        }
    }

    async fn out_of_order(&mut self, endpoint: &quinn::Endpoint) {
        let Some(mut l) = self.link(endpoint).await else {
            return;
        };
        let creds = Credentials {
            account: format!("hx-{:08x}", self.rng.next_u64() & 0xFFFF_FFFF),
            password: "not a password".into(),
        };
        let seq: Vec<Message> = match self.rng.below(7) {
            0 => vec![Message::Login(creds)],
            1 => vec![Message::EnterWorld(EnterWorld {
                name: "Nobody".into(),
            })],
            2 => vec![raw::hello(), raw::hello()],
            3 => vec![
                raw::hello(),
                Message::EnterWorld(EnterWorld {
                    name: "Nobody".into(),
                }),
            ],
            4 => vec![
                raw::hello(),
                Message::ChatSend(ChatSend {
                    channel: ChatChannel::Yell,
                    text: "before login".into(),
                }),
            ],
            5 => vec![
                raw::hello(),
                Message::CharacterCreate(CharacterCreate {
                    name: "Nobody".into(),
                    appearance: raw::bot_appearance(0),
                    look: raw::bot_look(0),
                }),
            ],
            _ => vec![Message::Pong { nonce: 1 }, raw::hello()],
        };
        for m in &seq {
            if l.send(m).await.is_err() {
                break;
            }
        }
        if self.rng.chance(0.3) {
            // A datagram before the handshake is complete.
            if let Ok(d) = encode_datagram(&Message::TimeRequest { client_time: 1 }) {
                let _ = l.conn.send_datagram(d.into());
            }
        }
        self.settle(&l.conn, CLOSE_WAIT).await;
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn names_are_unique() {
        let mut names: Vec<_> = Attack::ALL.iter().map(|a| a.name()).collect();
        names.sort_unstable();
        names.dedup();
        assert_eq!(names.len(), Attack::ALL.len());
    }

    #[test]
    fn bind_addresses() {
        let v6 = bind_ip("[2001:db8::1]:4000".parse().unwrap());
        assert_eq!(v6, IpAddr::V6(Ipv6Addr::UNSPECIFIED));
        let v4 = bind_ip("192.0.2.1:4000".parse().unwrap());
        assert_eq!(v4, IpAddr::V4(Ipv4Addr::UNSPECIFIED));
        assert!(bind_ip("127.0.0.1:4000".parse().unwrap()).is_loopback());
    }

    /// Regression: the pause between attacks was slept in one piece, so `stop()` blocked
    /// for up to 1.5 × pace (37.5 s measured with a 60 s pace) after the run.
    #[test]
    fn stop_cuts_the_pause_short() {
        // Nothing listens on this port: every attack fails fast or times out.
        let sock = std::net::UdpSocket::bind("127.0.0.1:0").expect("a free port");
        let addr = sock.local_addr().expect("its address");
        drop(sock);
        let run = HostileRun::start(HostileConfig {
            target: Target {
                addr,
                server_name: "127.0.0.1".into(),
                fingerprint: tessera_protocol::fingerprint::Fingerprint([7; 32]),
            },
            clients: 1,
            pace: Duration::from_secs(60),
            seed: 1,
            characters: Vec::new(),
        })
        .expect("attackers start");
        std::thread::sleep(Duration::from_millis(300));
        let t = std::time::Instant::now();
        let snap = run.stop();
        let took = t.elapsed();
        assert!(snap.attacks.values().sum::<u64>() >= 1);
        // At most the attack in progress plus one pause slice.
        assert!(
            took < ATTACK_LIMIT + Duration::from_secs(1),
            "stop took {took:?}"
        );
    }
}
