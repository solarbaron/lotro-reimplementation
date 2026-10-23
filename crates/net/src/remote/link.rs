// SPDX-License-Identifier: GPL-3.0-or-later
//! The network thread of the test-server backend (decision 0030, ARCH §19.4).
//!
//! One network thread per process runs a tokio `current_thread` runtime; every session is a
//! task on it, and the sessions share one quinn client endpoint per address family. A session task owns the [`Driver`] and does all
//! socket I/O: the QUIC/TLS handshake with the pinned-key
//! check, the control stream, the server's world and chat streams, datagrams, and a 30 Hz
//! tick. It talks to the main thread only through bounded channels: commands in, decoded
//! [`Inbound`] items out, plus a `watch` slot with the newest player move and a shared
//! statistics snapshot. Nothing on the main thread ever waits for this thread.
//!
//! Back-pressure (`tessera-protocol.md` §13.3): while the outbound channel is full and items
//! are waiting here, the thread stops reading the streams (QUIC flow control then holds
//! the server back) and drops movement items, so reliable state is never lost and the
//! memory held here stays bounded.

use std::net::{IpAddr, Ipv4Addr, Ipv6Addr, SocketAddr, ToSocketAddrs};
use std::path::PathBuf;
use std::sync::{Arc, Mutex, OnceLock, PoisonError};
use std::time::{Duration, Instant};

use quinn::{Connection, ConnectionError, ReadError, RecvStream, WriteError};
use tessera_protocol::codes::{ClientFailure, QUIC_NO_APPLICATION_PROTOCOL};
use tessera_protocol::connect::{ConnectString, ExpectedKey, Host};
use tessera_protocol::fingerprint::Fingerprint;
use tessera_protocol::frame::{StreamKind, encode_datagram, frame_to_vec_minor};
use tessera_protocol::limits::HANDSHAKE_TIMEOUT;
use tessera_protocol::message::Channel;
use tessera_protocol::session::{ConnectionEnd, StreamEnd, StreamRef, Transmit};
use tessera_protocol::transport::{
    PinOutcome, PinnedServerVerifier, TofuPolicy, client_endpoint, peer_datagrams,
    quinn_client_config, server_name,
};
use tokio::sync::{mpsc, watch};

use super::config::{FirstUse, MAX_CONTROL_BACKLOG, PENDING_PAUSE, READ_CHUNK, Trust};
use super::driver::{Driver, Inbound, MoveSlot, NetCommand, TICK};
use super::known::{load_known_servers, record_known_server};
use super::stats::RemoteStats;
use tessera_protocol::codes::AppErrorCode;

/// How long the thread keeps trying to hand its last items (the end of the session) to a
/// main thread that does not poll, before it gives up.
const FINAL_DELIVERY: Duration = Duration::from_secs(30);
/// Items to keep before the network thread starts dropping movement when the main thread
/// lags.
const MOTION_KEEP: usize = PENDING_PAUSE / 4;

/// Everything the network thread needs.
pub(crate) struct LinkParams {
    pub server: ConnectString,
    pub trust: Trust,
    pub driver: Driver,
    pub base: Instant,
    pub commands: mpsc::Receiver<NetCommand>,
    pub events: mpsc::Sender<Inbound>,
    pub moves: watch::Receiver<Option<MoveSlot>>,
    pub shutdown: watch::Receiver<bool>,
    /// Set by [`RemoteBackend::drop_connection`](super::RemoteBackend::drop_connection):
    /// close at once, without `Disconnect`.
    pub abort: watch::Receiver<bool>,
    pub stats: Arc<Mutex<RemoteStats>>,
}

/// Starts one session on the network thread.
///
/// Every backend in the process shares one network thread with one tokio `current_thread`
/// runtime (decision 0030 point 4, ARCH §19.4); each session is a task on it. The thread
/// starts with the first session and lives as long as the process. Starting a thread and a
/// runtime per session made every reconnection leave memory behind in the allocator's
/// per-thread arenas, so a client (or a load test) that reconnected often grew without
/// bound (`docs/perf/m7-soak.md`).
pub(crate) fn spawn(params: LinkParams) -> std::io::Result<tokio::task::JoinHandle<()>> {
    Ok(network_runtime()?.spawn(Link::new(params).run()))
}

/// The handle of the process-wide network runtime, starting its thread on first use.
fn network_runtime() -> std::io::Result<tokio::runtime::Handle> {
    static HANDLE: OnceLock<tokio::runtime::Handle> = OnceLock::new();
    static START: Mutex<()> = Mutex::new(());
    if let Some(h) = HANDLE.get() {
        return Ok(h.clone());
    }
    let _starting = START.lock().unwrap_or_else(PoisonError::into_inner);
    if let Some(h) = HANDLE.get() {
        return Ok(h.clone());
    }
    let runtime = tokio::runtime::Builder::new_current_thread()
        .enable_all()
        .build()?;
    let handle = runtime.handle().clone();
    std::thread::Builder::new()
        .name("tessera-net".into())
        // Sessions are tasks; a panic in one is caught by the runtime, ends only that
        // task (its channels close, which the main thread reports as a failed transport)
        // and leaves the others running. A name lookup abandoned at its deadline keeps
        // running on the blocking pool without holding anything up.
        .spawn(move || runtime.block_on(std::future::pending::<()>()))?;
    let _ = HANDLE.set(handle.clone());
    Ok(handle)
}

/// The process's client endpoint for one address family, created on first use (inside the
/// network runtime) or replaced when `renew` is set.
///
/// Every session shares it: quinn gives each endpoint a receive buffer of about 3 MiB
/// (1,472-byte datagrams × 64 GRO segments × a batch of 32), and an endpoint per session
/// made the allocator keep a new one at every reconnection (`docs/perf/m7-soak.md`). QUIC
/// tells connections apart by connection ID, so one UDP socket serves them all.
fn shared_endpoint(v6: bool, renew: bool) -> Result<quinn::Endpoint, String> {
    static ENDPOINTS: Mutex<[Option<quinn::Endpoint>; 2]> = Mutex::new([None, None]);
    let mut endpoints = ENDPOINTS.lock().unwrap_or_else(PoisonError::into_inner);
    let slot = &mut endpoints[usize::from(v6)];
    if renew {
        *slot = None;
    }
    if let Some(e) = slot {
        return Ok(e.clone());
    }
    let bind = if v6 {
        SocketAddr::new(IpAddr::V6(Ipv6Addr::UNSPECIFIED), 0)
    } else {
        SocketAddr::new(IpAddr::V4(Ipv4Addr::UNSPECIFIED), 0)
    };
    let endpoint = client_endpoint(bind).map_err(|e| e.to_string())?;
    *slot = Some(endpoint.clone());
    Ok(endpoint)
}

/// A server-initiated unidirectional stream, before and after its kind byte.
struct Uni {
    recv: RecvStream,
    kind: Option<StreamKind>,
    ended: bool,
}

/// What one read produced.
enum Read {
    Data(usize),
    End(StreamEnd),
    Lost,
}

async fn read_slot(slot: Option<&mut Uni>, buf: &mut [u8]) -> Read {
    let Some(u) = slot else {
        return std::future::pending().await;
    };
    let len = if u.kind.is_none() { 1 } else { buf.len() };
    read(&mut u.recv, &mut buf[..len]).await
}

async fn read(recv: &mut RecvStream, buf: &mut [u8]) -> Read {
    match recv.read(buf).await {
        Ok(Some(n)) => Read::Data(n),
        Ok(None) => Read::End(StreamEnd::Finished),
        Err(ReadError::Reset(_)) => Read::End(StreamEnd::Reset),
        Err(_) => Read::Lost,
    }
}

struct Link {
    server: ConnectString,
    trust: Trust,
    driver: Driver,
    base: Instant,
    commands: mpsc::Receiver<NetCommand>,
    commands_open: bool,
    events: mpsc::Sender<Inbound>,
    main_alive: bool,
    moves: watch::Receiver<Option<MoveSlot>>,
    shutdown: watch::Receiver<bool>,
    shutdown_seen: bool,
    abort: watch::Receiver<bool>,
    abort_seen: bool,
    stats: Arc<Mutex<RemoteStats>>,
}

impl Link {
    fn new(p: LinkParams) -> Self {
        Self {
            server: p.server,
            trust: p.trust,
            driver: p.driver,
            base: p.base,
            commands: p.commands,
            commands_open: true,
            events: p.events,
            main_alive: true,
            moves: p.moves,
            shutdown: p.shutdown,
            shutdown_seen: false,
            abort: p.abort,
            abort_seen: false,
            stats: p.stats,
        }
    }

    fn now(&self) -> u64 {
        u64::try_from(self.base.elapsed().as_micros()).unwrap_or(u64::MAX)
    }

    async fn run(mut self) {
        self.driver.connect(self.now());
        if let Some(conn) = self.connect().await {
            self.session(&conn).await;
        }
        // The shared endpoint outlives the session and sends its CONNECTION_CLOSE.
        self.publish(None);
        self.deliver_rest().await;
        tracing::debug!("network session done");
    }

    /// Resolves the host, runs the QUIC/TLS handshake with the key check (§4.3–§4.5).
    ///
    /// Name resolution runs on a blocking-pool thread and shares the handshake's deadline
    /// (§12.1 Handshaking, 10 s) and the shutdown watch, so a slow resolver neither
    /// outlives the timeout nor delays a disconnect.
    async fn connect(&mut self) -> Option<Connection> {
        let deadline = tokio::time::Instant::now() + HANDSHAKE_TIMEOUT;
        let host = self.server.server.host.clone();
        let port = self.server.server.port;
        let lookup = tokio::task::spawn_blocking(move || resolve(&host, port));
        let resolved = tokio::select! {
            r = lookup => r,
            () = tokio::time::sleep_until(deadline) => {
                self.driver.handshake_failed(
                    ClientFailure::Timeout,
                    None,
                    "name resolution timed out",
                );
                return None;
            }
            _ = self.shutdown.changed() => {
                self.driver.disconnect(self.now());
                return None;
            }
            _ = self.abort.changed() => {
                self.driver.connection_closed(ConnectionEnd::Local);
                return None;
            }
        };
        let addr = match resolved {
            Ok(Ok(a)) => a,
            Ok(Err(detail)) => {
                self.driver
                    .connection_closed(ConnectionEnd::Transport(detail));
                return None;
            }
            Err(e) => {
                self.driver
                    .connection_closed(ConnectionEnd::Transport(format!(
                        "name resolution failed: {e}"
                    )));
                return None;
            }
        };
        let known_path: Option<PathBuf> = self.trust.known_servers.clone();
        let expected = match &known_path {
            Some(p) if self.server.pin.is_none() => {
                let path = p.clone();
                let loaded = tokio::task::spawn_blocking(move || load_known_servers(&path)).await;
                let known = match loaded {
                    Ok(Ok(known)) => known,
                    // §4.5: a found entry is authoritative. A file that exists but could not
                    // be read completely may hold this server's entry, so neither trust on
                    // first use nor any other key may be accepted: fail before the
                    // handshake, and never send the password.
                    Ok(Err(e)) => {
                        self.unreadable_known_servers(p, &e.to_string());
                        return None;
                    }
                    Err(e) => {
                        self.unreadable_known_servers(p, &e.to_string());
                        return None;
                    }
                };
                ExpectedKey::choose(&self.server, &known)
            }
            _ => ExpectedKey::choose(&self.server, &Default::default()),
        };
        let tofu: Option<TofuPolicy> = (self.trust.first_use == FirstUse::Accept
            && expected == ExpectedKey::Unknown)
            .then(|| -> TofuPolicy { Arc::new(|_: &Fingerprint| true) });
        let verifier = PinnedServerVerifier::new(expected, tofu);
        let setup = (|| {
            let cfg = quinn_client_config(verifier.clone()).map_err(|e| e.to_string())?;
            let name = server_name(&self.server.server.host);
            let endpoint = shared_endpoint(addr.is_ipv6(), false)?;
            let connecting = match endpoint.connect_with(cfg.clone(), addr, &name) {
                // The endpoint stopped (its socket failed): start a new one, once.
                Err(quinn::ConnectError::EndpointStopping) => {
                    shared_endpoint(addr.is_ipv6(), true)?.connect_with(cfg, addr, &name)
                }
                other => other,
            };
            connecting.map_err(|e| e.to_string())
        })();
        let connecting = match setup {
            Ok(x) => x,
            Err(detail) => {
                self.driver
                    .connection_closed(ConnectionEnd::Transport(detail));
                return None;
            }
        };
        let result = tokio::select! {
            r = connecting => Some(r),
            () = tokio::time::sleep_until(deadline) => None,
            _ = self.shutdown.changed() => {
                self.driver.disconnect(self.now());
                return None;
            }
            _ = self.abort.changed() => {
                self.driver.connection_closed(ConnectionEnd::Local);
                return None;
            }
        };
        let conn = match result {
            Some(Ok(c)) => c,
            Some(Err(e)) => {
                self.handshake_error(&e, verifier.outcome());
                return None;
            }
            None => {
                self.driver
                    .handshake_failed(ClientFailure::Timeout, None, "handshake timed out");
                return None;
            }
        };
        let presented = match verifier.outcome() {
            Some(PinOutcome::Matched(f)) => Some(f),
            Some(PinOutcome::Unknown {
                presented,
                accepted: true,
            }) => {
                if let Some(path) = known_path {
                    let key = self.server.server.clone();
                    let saved = tokio::task::spawn_blocking(move || {
                        record_known_server(&path, &key, &presented)
                    })
                    .await;
                    match saved {
                        Ok(Ok(())) => {}
                        Ok(Err(e)) => tracing::warn!(error = %e, "server key not recorded"),
                        Err(e) => tracing::warn!(error = %e, "server key not recorded"),
                    }
                }
                Some(presented)
            }
            _ => None,
        };
        self.with_stats(|s| s.fingerprint = presented);
        Some(conn)
    }

    /// The known-servers file exists but could not be read completely: the connection fails
    /// as `KEY_UNKNOWN` (no key can be trusted) with a detail naming the file.
    fn unreadable_known_servers(&mut self, path: &std::path::Path, error: &str) {
        tracing::warn!(path = %path.display(), %error, "cannot read the known-servers file");
        self.driver.handshake_failed(
            ClientFailure::KeyUnknown,
            None,
            &format!(
                "the known-servers file {} could not be read ({error}); fix or remove it, or connect with a pinned key",
                path.display()
            ),
        );
    }

    fn handshake_error(&mut self, e: &ConnectionError, outcome: Option<PinOutcome>) {
        let presented = match outcome {
            Some(PinOutcome::Mismatch { presented } | PinOutcome::Unknown { presented, .. }) => {
                Some(presented)
            }
            Some(PinOutcome::Matched(f)) => Some(f),
            _ => None,
        };
        if let Some(failure) = outcome.and_then(|o| o.failure()) {
            self.driver
                .handshake_failed(failure, presented, &e.to_string());
            return;
        }
        let tls = |code: u64| (0x100..=0x1ff).contains(&code);
        match e {
            ConnectionError::TimedOut => {
                self.driver
                    .handshake_failed(ClientFailure::Timeout, presented, "")
            }
            ConnectionError::ConnectionClosed(c)
                if u64::from(c.error_code) == QUIC_NO_APPLICATION_PROTOCOL =>
            {
                self.driver
                    .handshake_failed(ClientFailure::NoCommonVersion, presented, "")
            }
            ConnectionError::TransportError(t)
                if u64::from(t.code) == QUIC_NO_APPLICATION_PROTOCOL =>
            {
                self.driver
                    .handshake_failed(ClientFailure::NoCommonVersion, presented, "")
            }
            ConnectionError::ConnectionClosed(c) if tls(u64::from(c.error_code)) => self
                .driver
                .handshake_failed(ClientFailure::TlsFailure, presented, &e.to_string()),
            ConnectionError::TransportError(t) if tls(u64::from(t.code)) => self
                .driver
                .handshake_failed(ClientFailure::TlsFailure, presented, &e.to_string()),
            other => self.driver.connection_closed(connection_end(other)),
        }
    }

    /// The connected session: runs until the connection is closed.
    async fn session(&mut self, conn: &Connection) {
        let opened = tokio::time::timeout(HANDSHAKE_TIMEOUT, conn.open_bi()).await;
        let (mut send, mut recv) = match opened {
            Ok(Ok(streams)) => streams,
            Ok(Err(e)) => {
                self.driver.connection_closed(connection_end(&e));
                return;
            }
            Err(_) => {
                self.driver.handshake_failed(
                    ClientFailure::Timeout,
                    None,
                    "control stream not opened",
                );
                conn.close(0u32.into(), b"TIMEOUT");
                return;
            }
        };
        self.driver
            .handshake_complete(self.now(), peer_datagrams(conn));

        let mut ctl_out: Vec<u8> = Vec::new();
        let mut finish_pending = false;
        let mut ctl_done = false;
        let mut ctl_read_done = false;
        let mut uni: [Option<Uni>; 2] = [None, None];
        let mut buf_ctl = vec![0u8; READ_CHUNK];
        let mut buf0 = vec![0u8; READ_CHUNK];
        let mut buf1 = vec![0u8; READ_CHUNK];
        let mut stopped = Box::pin(send.stopped());
        let mut stopped_done = false;
        let mut ticker = tokio::time::interval(TICK);
        ticker.set_missed_tick_behavior(tokio::time::MissedTickBehavior::Skip);

        loop {
            // Carry out what the driver asked for.
            if let Some((code, reason)) = self.transmit(conn, &mut ctl_out, &mut finish_pending) {
                conn.close(u32::from(code.to_u16()).into(), reason.as_bytes());
                self.driver.connection_closed(ConnectionEnd::Local);
                return;
            }
            if finish_pending && ctl_out.is_empty() && !ctl_done {
                let _ = send.finish();
                finish_pending = false;
                ctl_done = true;
            }
            if ctl_out.len() > MAX_CONTROL_BACKLOG {
                self.driver.connection_closed(ConnectionEnd::Transport(
                    "the server does not read the control stream".into(),
                ));
                conn.close(
                    u32::from(AppErrorCode::InternalError.to_u16()).into(),
                    b"control backlog",
                );
                return;
            }
            self.flush_events();
            if !self.main_alive && !self.driver.closed() {
                self.driver.disconnect(self.now());
                continue;
            }

            let reading = self.driver.pending.len() < PENDING_PAUSE;
            let can_ctl = reading && !ctl_read_done;
            let can_uni = |u: &Option<Uni>, d: &Driver| match u {
                Some(Uni { ended: true, .. }) | None => false,
                Some(Uni { kind: None, .. }) => d.may_read(StreamKind::Chat),
                Some(Uni { kind: Some(k), .. }) => reading && d.may_read(*k),
            };
            let can0 = can_uni(&uni[0], &self.driver);
            let can1 = can_uni(&uni[1], &self.driver);
            let accept = uni.iter().any(Option::is_none);
            let want_send = !self.driver.pending.is_empty() && self.main_alive;
            let [slot0, slot1] = &mut uni;

            tokio::select! {
                r = self.shutdown.changed(), if !self.shutdown_seen => {
                    let _ = r;
                    self.shutdown_seen = true;
                    self.driver.disconnect(self.now());
                }
                r = self.abort.changed(), if !self.abort_seen => {
                    self.abort_seen = true;
                    if r.is_ok() && *self.abort.borrow() {
                        // Abrupt loss (test-server.md §13.1): no `Disconnect`, no wait for
                        // the streams; the peer sees a bare CONNECTION_CLOSE.
                        conn.close(u32::from(AppErrorCode::NoError.to_u16()).into(), b"");
                        self.driver.connection_closed(ConnectionEnd::Local);
                        return;
                    }
                }
                e = conn.closed() => {
                    self.driver.connection_closed(connection_end(&e));
                    return;
                }
                c = self.commands.recv(), if self.commands_open => match c {
                    Some(c) => {
                        let now = self.now();
                        self.driver.command(c, now);
                    }
                    None => self.commands_open = false,
                },
                n = send.write(&ctl_out), if !ctl_out.is_empty() && !ctl_done => match n {
                    Ok(n) => { ctl_out.drain(..n.min(ctl_out.len())); }
                    Err(WriteError::Stopped(_)) => {
                        ctl_done = true;
                        self.driver.on_stream_end(StreamRef::Control, StreamEnd::Stopped);
                    }
                    Err(_) => { ctl_done = true; }
                },
                s = &mut stopped, if !stopped_done => {
                    stopped_done = true;
                    if let Ok(Some(_)) = s {
                        self.driver.on_stream_end(StreamRef::Control, StreamEnd::Stopped);
                    }
                }
                r = read(&mut recv, &mut buf_ctl), if can_ctl => {
                    let now = self.now();
                    match r {
                        Read::Data(n) => self.driver.on_control(&buf_ctl[..n], now),
                        Read::End(end) => {
                            ctl_read_done = true;
                            self.driver.on_stream_end(StreamRef::Control, end);
                        }
                        Read::Lost => ctl_read_done = true,
                    }
                }
                r = read_slot(slot0.as_mut(), &mut buf0), if can0 => {
                    self.on_uni_read(slot0, r, &buf0);
                }
                r = read_slot(slot1.as_mut(), &mut buf1), if can1 => {
                    self.on_uni_read(slot1, r, &buf1);
                }
                a = conn.accept_uni(), if accept => {
                    if let Ok(recv) = a {
                        let slot = if slot0.is_none() { slot0 } else { slot1 };
                        if slot.is_none() {
                            *slot = Some(Uni { recv, kind: None, ended: false });
                        }
                    }
                }
                d = conn.read_datagram() => {
                    if let Ok(d) = d {
                        let now = self.now();
                        self.driver.on_datagram(&d, now);
                    }
                }
                p = self.events.clone().reserve_owned(), if want_send => match p {
                    Ok(permit) => {
                        if let Some(item) = self.driver.pending.pop_front() {
                            permit.send(item);
                        }
                    }
                    Err(_) => self.main_gone(),
                },
                _ = ticker.tick() => {
                    let now = self.now();
                    let latest = *self.moves.borrow_and_update();
                    self.driver.tick(now, latest);
                    self.publish(Some(conn));
                }
            }
        }
    }

    fn on_uni_read(&mut self, slot: &mut Option<Uni>, r: Read, buf: &[u8]) {
        let Some(u) = slot.as_mut() else {
            return;
        };
        let now = self.now();
        match (r, u.kind) {
            (Read::Data(n), None) => {
                let Some(&b) = buf.get(..n).and_then(<[u8]>::first) else {
                    return;
                };
                match self.driver.on_uni_stream(b) {
                    Some(k) => u.kind = Some(k),
                    // Ignored (closing) or refused (the driver closed the session).
                    None => u.ended = true,
                }
            }
            (Read::Data(n), Some(k)) => self.driver.on_stream(k, &buf[..n], now),
            (Read::End(end), kind) => {
                u.ended = true;
                let stream = kind.map_or(StreamRef::Untyped, StreamRef::Uni);
                self.driver.on_stream_end(stream, end);
            }
            (Read::Lost, _) => u.ended = true,
        }
    }

    /// Drains the driver's transmits. Returns the close to perform, if one was asked for.
    fn transmit(
        &mut self,
        conn: &Connection,
        ctl_out: &mut Vec<u8>,
        finish: &mut bool,
    ) -> Option<(AppErrorCode, String)> {
        while let Some(t) = self.driver.poll_transmit() {
            match t {
                Transmit::Message(m) => match m.channel() {
                    Channel::Datagram => match encode_datagram(&m) {
                        Ok(bytes) => {
                            if let Err(e) = conn.send_datagram(bytes.into()) {
                                tracing::debug!(error = %e, "datagram not sent");
                            }
                        }
                        Err(e) => tracing::warn!(error = %e, "datagram not encodable"),
                    },
                    Channel::Control => match frame_to_vec_minor(&m, self.driver.minor()) {
                        Ok(bytes) => ctl_out.extend_from_slice(&bytes),
                        Err(e) => tracing::warn!(error = %e, "frame not encodable"),
                    },
                    _ => {
                        tracing::warn!(ty = ?m.message_type(), "client cannot send on this channel")
                    }
                },
                Transmit::FinishControl => *finish = true,
                Transmit::Close { code, reason } => return Some((code, reason)),
                Transmit::OpenStream(_) => {}
            }
        }
        None
    }

    /// Hands waiting items to the main thread without waiting. When the channel is full and
    /// more than `MOTION_KEEP` items wait, every queued movement item is dropped (§13.3);
    /// reliable items are never dropped.
    fn flush_events(&mut self) {
        if !self.main_alive {
            self.driver.pending.clear();
            return;
        }
        while let Some(item) = self.driver.pending.pop_front() {
            match self.events.try_send(item) {
                Ok(()) => {}
                Err(mpsc::error::TrySendError::Full(item)) => {
                    self.driver.pending.push_front(item);
                    if self.driver.pending.len() > MOTION_KEEP {
                        let before = self.driver.pending.len();
                        self.driver.pending.retain(|i| !i.droppable());
                        let dropped = (before - self.driver.pending.len()) as u64;
                        self.driver.stats.motion_dropped += dropped;
                    }
                    if self.driver.pending.len() >= PENDING_PAUSE {
                        self.driver.stats.read_pauses += 1;
                    }
                    break;
                }
                Err(mpsc::error::TrySendError::Closed(_)) => {
                    self.main_gone();
                    break;
                }
            }
        }
        self.driver.stats.pending = self.driver.pending.len();
    }

    fn main_gone(&mut self) {
        self.main_alive = false;
        self.driver.pending.clear();
    }

    /// After the session: hands the last items (the end of the session) to the main thread.
    async fn deliver_rest(&mut self) {
        if !self.main_alive {
            return;
        }
        let deadline = tokio::time::Instant::now() + FINAL_DELIVERY;
        while let Some(item) = self.driver.pending.pop_front() {
            match tokio::time::timeout_at(deadline, self.events.send(item)).await {
                Ok(Ok(())) => {}
                _ => break,
            }
        }
    }

    fn with_stats(&self, f: impl FnOnce(&mut RemoteStats)) {
        if let Ok(mut s) = self.stats.lock() {
            f(&mut s);
        }
    }

    /// Copies the driver's and QUIC's statistics into the shared snapshot.
    fn publish(&self, conn: Option<&Connection>) {
        let mut snapshot = self.driver.stats.clone();
        if let Some(conn) = conn {
            let q = conn.stats();
            snapshot.path_rtt = Some(q.path.rtt);
            snapshot.udp_bytes_sent = q.udp_tx.bytes;
            snapshot.udp_bytes_received = q.udp_rx.bytes;
            snapshot.lost_packets = q.path.lost_packets;
            snapshot.congestion_window = q.path.cwnd;
        }
        self.with_stats(|s| {
            let fingerprint = s.fingerprint;
            let (rtt, tx, rx, lost, cwnd) = (
                s.path_rtt,
                s.udp_bytes_sent,
                s.udp_bytes_received,
                s.lost_packets,
                s.congestion_window,
            );
            *s = snapshot;
            s.fingerprint = s.fingerprint.or(fingerprint);
            if conn.is_none() {
                s.path_rtt = rtt;
                s.udp_bytes_sent = tx;
                s.udp_bytes_received = rx;
                s.lost_packets = lost;
                s.congestion_window = cwnd;
            }
        });
    }
}

/// Resolves a host. Blocking: `Link::connect` runs it on the blocking pool, under the
/// handshake deadline.
fn resolve(host: &Host, port: u16) -> Result<SocketAddr, String> {
    match host {
        Host::Ipv4(a) => Ok(SocketAddr::new(IpAddr::V4(*a), port)),
        Host::Ipv6(a) => Ok(SocketAddr::new(IpAddr::V6(*a), port)),
        Host::Dns(name) => (name.as_str(), port)
            .to_socket_addrs()
            .map_err(|e| format!("cannot resolve {name}: {e}"))?
            .next()
            .ok_or_else(|| format!("{name} has no address")),
    }
}

/// How quinn reported the end of a connection, as the protocol session wants it.
fn connection_end(e: &ConnectionError) -> ConnectionEnd {
    match e {
        ConnectionError::ApplicationClosed(c) => ConnectionEnd::PeerClosed {
            code: AppErrorCode::from_u16(
                u16::try_from(c.error_code.into_inner()).unwrap_or(u16::MAX),
            ),
            reason: String::from_utf8_lossy(&c.reason[..c.reason.len().min(256)]).into_owned(),
        },
        ConnectionError::TimedOut => ConnectionEnd::IdleTimeout,
        ConnectionError::LocallyClosed => ConnectionEnd::Local,
        other => ConnectionEnd::Transport(other.to_string()),
    }
}
