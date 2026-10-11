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

use std::collections::BTreeMap;
use std::net::{IpAddr, Ipv4Addr, Ipv6Addr, SocketAddr};
use std::sync::atomic::{AtomicBool, AtomicU64, Ordering};
use std::sync::{Arc, Mutex};
use std::thread::JoinHandle;
use std::time::Duration;

use tessera_protocol::Message;
use tessera_protocol::frame::{encode_datagram, frame_to_vec};
use tessera_protocol::message::{
    CharacterCreate, ChatChannel, ChatSend, Credentials, EnterWorld, MoveInput,
};
use tessera_protocol::types::{Location, Motion, Vec3};

use crate::LoadtestError;
use crate::raw::{self, ControlLink, Target};
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
    /// named channels), one per attack in turn, right after `Hello` as for
    /// [`Attack::MinorOneVectors`]. Each is sent in the world, so that only the 1.2 rules
    /// close the connection, by the `hostile_v12_cases` test (`tests/v12_channels.rs`).
    MinorTwoVectors,
}

impl Attack {
    /// Every attack.
    pub const ALL: [Self; 12] = [
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
}

#[derive(Debug, Default)]
struct Counters {
    attacks: Mutex<BTreeMap<&'static str, u64>>,
    connect_failures: AtomicU64,
    closed: Mutex<BTreeMap<String, u64>>,
    left_open: AtomicU64,
    udp: AtomicU64,
}

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
        let counters = Arc::new(Counters::default());
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
    /// The next vector of [`Attack::MinorOneVectors`] (§16.8) or
    /// [`Attack::MinorTwoVectors`] (§16.9).
    next_vector: usize,
}

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
    async fn settle(&self, conn: &quinn::Connection, wait: Duration) {
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
                self.counters.closed(why);
            }
            Err(_) => {
                self.counters.left_open.fetch_add(1, Ordering::Relaxed);
                conn.close(0u32.into(), b"");
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
            Attack::MinorTwoVectors => {
                let vectors = raw::negative_vectors_v12();
                let Some((v, _)) = vectors.get(self.next_vector % vectors.len().max(1)) else {
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
