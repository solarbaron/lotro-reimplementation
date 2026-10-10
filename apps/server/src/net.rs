// SPDX-License-Identifier: GPL-3.0-or-later
//! The network thread (`test-server.md` §8.1, decision 0030 item 4): a tokio
//! `current_thread` runtime owning the QUIC endpoints and one task per connection.
//!
//! Each connection task feeds a [`ConnectionCore`] with what the client sends, forwards
//! control events to the simulation over the bounded input channel (pausing the control
//! stream while `control_queue` events wait, §10.1), keeps the newest `MoveInput` in the
//! connection's one-slot movement queue, applies the simulation's requests, and writes the
//! world and chat streams through writer tasks whose unsent bytes are accounted.

use std::collections::HashMap;
use std::net::{IpAddr, SocketAddr};
use std::sync::atomic::{AtomicBool, AtomicU64, AtomicUsize, Ordering};
use std::sync::mpsc::SyncSender;
use std::sync::{Arc, Mutex};
use std::time::Duration;

use tessera_protocol::codes::AppErrorCode;
use tessera_protocol::frame::{StreamKind, encode_datagram, frame_to_vec_minor};
use tessera_protocol::limits::{CONTROL_QUEUE_GRACE, HANDSHAKE_TIMEOUT};
use tessera_protocol::message::Channel;
use tessera_protocol::server::{ServerEvent, ServerParams};
use tessera_protocol::session::{ConnectionEnd, StreamEnd, StreamRef, Transmit};
use tessera_protocol::transport::{ServerIdentity, peer_datagrams, server_endpoint};
use tokio::sync::mpsc::{UnboundedSender, unbounded_channel};

use crate::config::Config;
use crate::conn::{ConnectionCore, Inbound};
use crate::error::ServerError;
use crate::shared::{Clock, ConnShared, Outbound, SimInput, Stats};

/// Outbound queue capacity per connection (messages from the simulation).
pub const OUTBOUND_QUEUE: usize = 8_192;
/// Control-stream bytes queued unsent above which a connection is closed as a slow
/// consumer.
const CONTROL_BACKLOG_LIMIT: usize = 256 * 1024;
/// Read size for the control stream.
const READ_CHUNK: usize = 16 * 1024;
/// How often a paused control stream is re-checked.
const PAUSE_POLL: Duration = Duration::from_millis(20);
/// QUIC handshakes in progress from one source address (loopback exempt).
const MAX_HANDSHAKES_PER_ADDRESS: usize = 4;

/// Shared state of the network thread.
#[derive(Debug)]
pub(crate) struct NetCtx {
    pub clock: Clock,
    pub stats: Arc<Stats>,
    pub config: Arc<Config>,
    pub params: ServerParams,
    pub sim_tx: SyncSender<SimInput>,
    next_id: AtomicU64,
    admitted: AtomicUsize,
    per_addr: Mutex<HashMap<IpAddr, usize>>,
    handshakes_per_addr: Mutex<HashMap<IpAddr, usize>>,
    accepting: AtomicBool,
}

impl NetCtx {
    pub fn new(
        clock: Clock,
        stats: Arc<Stats>,
        config: Arc<Config>,
        params: ServerParams,
        sim_tx: SyncSender<SimInput>,
    ) -> Self {
        Self {
            clock,
            stats,
            config,
            params,
            sim_tx,
            next_id: AtomicU64::new(1),
            admitted: AtomicUsize::new(0),
            per_addr: Mutex::new(HashMap::new()),
            handshakes_per_addr: Mutex::new(HashMap::new()),
            accepting: AtomicBool::new(true),
        }
    }

    /// Admission (§10.1, §10.2): connections, connections per address (loopback exempt),
    /// and 80 % of the memory budget.
    fn admit(&self, addr: SocketAddr) -> bool {
        let l = &self.config.limits;
        let budget = u64::from(l.memory_budget_mib) * 1024 * 1024;
        let accounted = self.stats.accounted_bytes.load(Ordering::Relaxed) as u64;
        if accounted >= budget / 10 * 8 {
            return false;
        }
        if self.admitted.load(Ordering::Acquire) >= l.max_connections as usize {
            return false;
        }
        let ip = addr.ip();
        let Ok(mut map) = self.per_addr.lock() else {
            return false;
        };
        if !ip.is_loopback() {
            let n = map.get(&ip).copied().unwrap_or(0);
            if n >= l.max_connections_per_address as usize {
                return false;
            }
        }
        *map.entry(ip).or_insert(0) += 1;
        self.admitted.fetch_add(1, Ordering::AcqRel);
        true
    }

    /// Counts a handshake from `ip`; false when the address already has
    /// [`MAX_HANDSHAKES_PER_ADDRESS`] in progress (loopback exempt).
    fn start_handshake(&self, ip: IpAddr) -> bool {
        let Ok(mut map) = self.handshakes_per_addr.lock() else {
            return false;
        };
        let n = map.entry(ip).or_insert(0);
        if !ip.is_loopback() && *n >= MAX_HANDSHAKES_PER_ADDRESS {
            if *n == 0 {
                map.remove(&ip);
            }
            return false;
        }
        *n += 1;
        true
    }

    fn end_handshake(&self, ip: IpAddr) {
        if let Ok(mut map) = self.handshakes_per_addr.lock()
            && let Some(n) = map.get_mut(&ip)
        {
            *n = n.saturating_sub(1);
            if *n == 0 {
                map.remove(&ip);
            }
        }
    }

    fn release(&self, addr: SocketAddr) {
        self.admitted.fetch_sub(1, Ordering::AcqRel);
        if let Ok(mut map) = self.per_addr.lock()
            && let Some(n) = map.get_mut(&addr.ip())
        {
            *n = n.saturating_sub(1);
            if *n == 0 {
                map.remove(&addr.ip());
            }
        }
    }
}

/// Requests to the network thread.
#[derive(Debug)]
pub(crate) enum NetControl {
    /// Stop accepting connections (§12.1 step 1).
    StopAccepting,
    /// Wait up to `linger` for connections to close, then close the endpoints and stop.
    Shutdown {
        linger: Duration,
        done: std::sync::mpsc::Sender<()>,
    },
}

/// The network thread, the bound addresses and its control channel.
pub(crate) type Started = (
    std::thread::JoinHandle<()>,
    Vec<SocketAddr>,
    UnboundedSender<NetControl>,
);

/// Starts the network thread, binds every address, and returns the thread, the bound
/// addresses and the control channel.
pub(crate) fn start(
    ctx: Arc<NetCtx>,
    identity: &ServerIdentity,
    addrs: &[SocketAddr],
) -> Result<Started, ServerError> {
    let server_config = identity.quinn_server_config()?;
    let (ready_tx, ready_rx) = std::sync::mpsc::channel::<Result<Vec<SocketAddr>, ServerError>>();
    let (ctl_tx, mut ctl_rx) = unbounded_channel::<NetControl>();
    let addrs = addrs.to_vec();
    let identity_cfg = server_config;
    let pem = identity.to_pkcs8_pem();
    let thread = std::thread::Builder::new()
        .name("tessera-net".into())
        .spawn(move || {
            let rt = match tokio::runtime::Builder::new_current_thread()
                .enable_all()
                .build()
            {
                Ok(rt) => rt,
                Err(e) => {
                    let _ = ready_tx.send(Err(ServerError::Runtime(format!("tokio runtime: {e}"))));
                    return;
                }
            };
            rt.block_on(async move {
                let identity = match ServerIdentity::from_pkcs8_pem(&pem) {
                    Ok(i) => i,
                    Err(e) => {
                        let _ = ready_tx.send(Err(e.into()));
                        return;
                    }
                };
                drop(pem);
                let mut endpoints = Vec::new();
                let mut bound = Vec::new();
                for a in &addrs {
                    match server_endpoint(&identity, *a) {
                        Ok(ep) => {
                            ep.set_server_config(Some(identity_cfg.clone()));
                            match ep.local_addr() {
                                Ok(l) => bound.push(l),
                                Err(e) => {
                                    let _ = ready_tx.send(Err(ServerError::io(a.to_string(), e)));
                                    return;
                                }
                            }
                            endpoints.push(ep);
                        }
                        Err(e) => {
                            let _ = ready_tx.send(Err(ServerError::Runtime(format!(
                                "cannot listen on {a}: {e}"
                            ))));
                            return;
                        }
                    }
                }
                drop(identity);
                let stop = Arc::new(tokio::sync::Notify::new());
                for ep in &endpoints {
                    tokio::spawn(accept_loop(ep.clone(), Arc::clone(&ctx), Arc::clone(&stop)));
                }
                let _ = ready_tx.send(Ok(bound));
                while let Some(c) = ctl_rx.recv().await {
                    match c {
                        NetControl::StopAccepting => {
                            ctx.accepting.store(false, Ordering::Release);
                            stop.notify_waiters();
                            for ep in &endpoints {
                                ep.set_server_config(None);
                            }
                        }
                        NetControl::Shutdown { linger, done } => {
                            ctx.accepting.store(false, Ordering::Release);
                            stop.notify_waiters();
                            let deadline = tokio::time::Instant::now() + linger;
                            while ctx.stats.net_tasks.load(Ordering::Acquire) > 0
                                && tokio::time::Instant::now() < deadline
                            {
                                tokio::time::sleep(Duration::from_millis(10)).await;
                            }
                            for ep in &endpoints {
                                ep.close(
                                    quinn::VarInt::from_u32(u32::from(
                                        AppErrorCode::ServerShutdown.to_u16(),
                                    )),
                                    b"SERVER_SHUTDOWN",
                                );
                            }
                            for ep in &endpoints {
                                let _ =
                                    tokio::time::timeout(Duration::from_secs(2), ep.wait_idle())
                                        .await;
                            }
                            let _ = done.send(());
                            return;
                        }
                    }
                }
            });
        })
        .map_err(|e| ServerError::Runtime(format!("cannot start the network thread: {e}")))?;
    match ready_rx.recv() {
        Ok(Ok(bound)) => Ok((thread, bound, ctl_tx)),
        Ok(Err(e)) => {
            let _ = thread.join();
            Err(e)
        }
        Err(_) => {
            let _ = thread.join();
            Err(ServerError::Runtime("the network thread stopped".into()))
        }
    }
}

async fn accept_loop(ep: quinn::Endpoint, ctx: Arc<NetCtx>, stop: Arc<tokio::sync::Notify>) {
    loop {
        let incoming = tokio::select! {
            i = ep.accept() => i,
            () = stop.notified() => return,
        };
        let Some(incoming) = incoming else { return };
        if !ctx.accepting.load(Ordering::Acquire) {
            incoming.refuse();
            continue;
        }
        let l = &ctx.config.limits;
        let max = l.max_handshakes as usize;
        let handshakes = ctx.stats.handshakes.load(Ordering::Acquire);
        let addr = incoming.remote_address();
        if handshakes >= max {
            tracing::debug!(%addr, "handshake refused: too many in progress");
            incoming.refuse();
            continue;
        }
        // Connections past the handshake that admission refused still live until their
        // Hello is answered: cap every live connection, so a flood of completing but silent
        // clients cannot pile up (§10.1, §10.2).
        if ctx.stats.connections.load(Ordering::Acquire) >= l.max_connections as usize + max {
            tracing::debug!(%addr, "handshake refused: too many connections");
            incoming.refuse();
            continue;
        }
        // Under handshake pressure, make unvalidated sources prove their address with a
        // Retry first: spoofed addresses then cannot hold handshake slots.
        if handshakes >= max / 2 && !incoming.remote_address_validated() {
            if incoming.may_retry() {
                if incoming.retry().is_err() {
                    tracing::debug!(%addr, "retry failed");
                }
            } else {
                incoming.refuse();
            }
            continue;
        }
        if !ctx.start_handshake(addr.ip()) {
            tracing::debug!(%addr, "handshake refused: too many in progress from the address");
            incoming.refuse();
            continue;
        }
        ctx.stats.handshakes.fetch_add(1, Ordering::AcqRel);
        ctx.stats.net_tasks.fetch_add(1, Ordering::AcqRel);
        tokio::spawn(connection_task(Arc::clone(&ctx), incoming));
    }
}

/// What a writer task reports.
#[derive(Debug)]
enum Internal {
    StreamEnded(StreamRef, StreamEnd),
}

enum CtlWrite {
    Data(Vec<u8>),
    Finish,
}

struct Task {
    ctx: Arc<NetCtx>,
    conn: quinn::Connection,
    core: ConnectionCore,
    shared: Arc<ConnShared>,
    admitted: bool,
    reported: bool,
    ctl_tx: Option<UnboundedSender<CtlWrite>>,
    world_tx: Option<UnboundedSender<Vec<u8>>>,
    chat_tx: Option<UnboundedSender<Vec<u8>>>,
    internal_tx: UnboundedSender<Internal>,
    paused_since: Option<u64>,
    /// The control-backlog slow-consumer rule has fired (it fires once).
    slow_reported: bool,
}

impl Task {
    fn now(&self) -> u64 {
        self.ctx.clock.now()
    }

    fn report_closed(&mut self) {
        if self.reported {
            return;
        }
        self.reported = true;
        // The simulation also scans `closed` flags every tick for connections whose Closed
        // did not fit in its input channel, so a full channel loses nothing; a queued Closed
        // arrives after every event this connection forwarded.
        let queued = self.admitted
            && self
                .ctx
                .sim_tx
                .try_send(SimInput::Closed(self.shared.id))
                .is_ok();
        self.shared.close_queued.store(queued, Ordering::Release);
        self.shared.closed.store(true, Ordering::Release);
    }

    fn forward(&mut self, inbound: Vec<Inbound>) {
        for i in inbound {
            match i {
                Inbound::Greeted(h) => {
                    // The minor the session negotiated travels with the event (the
                    // simulation must not negotiate it again).
                    self.shared.set_minor(self.core.minor());
                    self.forward_event(ServerEvent::Greeted(h));
                }
                Inbound::Event(e) => self.forward_event(e),
                Inbound::Move(m) => self.shared.put_move(m),
                Inbound::Ended {
                    code,
                    client,
                    violation,
                } => {
                    if violation {
                        self.ctx.stats.violations.fetch_add(1, Ordering::Relaxed);
                    }
                    if code == AppErrorCode::ServerFull {
                        self.ctx.stats.server_full.fetch_add(1, Ordering::Relaxed);
                        tracing::debug!(conn = self.shared.id, "SERVER_FULL");
                    }
                    tracing::info!(
                        conn = self.shared.id,
                        addr = %self.shared.addr,
                        code = %code,
                        client,
                        "session ended"
                    );
                    self.report_closed();
                }
            }
        }
    }

    fn forward_event(&mut self, e: ServerEvent) {
        if !self.admitted || self.reported {
            return;
        }
        self.ctx.stats.messages_in.fetch_add(1, Ordering::Relaxed);
        self.shared.inbound_pending.fetch_add(1, Ordering::AcqRel);
        if self
            .ctx
            .sim_tx
            .try_send(SimInput::Event(self.shared.id, e))
            .is_err()
        {
            self.shared.inbound_pending.fetch_sub(1, Ordering::AcqRel);
            tracing::warn!(
                conn = self.shared.id,
                "simulation input full: connection closed"
            );
            self.shared.kill(AppErrorCode::InternalError);
        }
    }

    fn paused(&self) -> bool {
        self.paused_since.is_some()
    }

    /// Decodes buffered control bytes while fewer than `control_queue` events wait for the
    /// simulation, then updates the pause: the limit holds per message, not per read
    /// (§10.1).
    fn pump_control(&mut self, now: u64) {
        let limit = self.ctx.config.limits.control_queue as usize;
        while self.core.has_unread_control() {
            let pending = self.shared.inbound_pending.load(Ordering::Acquire);
            let budget = limit.saturating_sub(pending);
            if budget == 0 {
                break;
            }
            let inbound = self.core.decode_control(now, budget);
            self.forward(inbound);
        }
        self.update_pause(now);
    }

    fn update_pause(&mut self, now: u64) {
        let limit = self.ctx.config.limits.control_queue as usize;
        let pending = self.shared.inbound_pending.load(Ordering::Acquire);
        if pending >= limit {
            if self.paused_since.is_none() {
                self.paused_since = Some(now);
            }
        } else {
            self.paused_since = None;
        }
    }

    /// Sends everything the protocol session queued.
    fn flush(&mut self) {
        while let Some(t) = self.core.poll_transmit() {
            match t {
                Transmit::Message(m) => {
                    self.ctx.stats.messages_out.fetch_add(1, Ordering::Relaxed);
                    match m.channel() {
                        Channel::Datagram => {
                            if let Ok(b) = encode_datagram(&m) {
                                self.ctx
                                    .stats
                                    .bytes_out
                                    .fetch_add(b.len() as u64, Ordering::Relaxed);
                                let _ = self.conn.send_datagram(b.into());
                            }
                        }
                        ch => {
                            let Ok(b) = frame_to_vec_minor(&m, self.core.minor()) else {
                                continue;
                            };
                            let n = b.len();
                            let (tx, counter) = match ch {
                                Channel::Control => {
                                    let ok = self
                                        .ctl_tx
                                        .as_ref()
                                        .is_some_and(|t| t.send(CtlWrite::Data(b)).is_ok());
                                    if ok {
                                        self.shared.control_unsent.fetch_add(n, Ordering::Relaxed);
                                    }
                                    continue;
                                }
                                Channel::World => {
                                    (self.world_tx.as_ref(), &self.shared.world_unsent)
                                }
                                _ => (self.chat_tx.as_ref(), &self.shared.chat_unsent),
                            };
                            if let Some(tx) = tx {
                                counter.fetch_add(n, Ordering::Relaxed);
                                if tx.send(b).is_err() {
                                    counter.fetch_sub(n, Ordering::Relaxed);
                                }
                            }
                        }
                    }
                }
                Transmit::OpenStream(kind) => {
                    let (tx, rx) = unbounded_channel::<Vec<u8>>();
                    let counter = Arc::clone(&self.shared);
                    tokio::spawn(uni_writer(
                        self.conn.clone(),
                        kind,
                        rx,
                        counter,
                        self.internal_tx.clone(),
                        Arc::clone(&self.ctx.stats),
                    ));
                    match kind {
                        StreamKind::World => self.world_tx = Some(tx),
                        StreamKind::Chat => self.chat_tx = Some(tx),
                    }
                }
                Transmit::FinishControl => {
                    if let Some(t) = &self.ctl_tx {
                        let _ = t.send(CtlWrite::Finish);
                    }
                }
                Transmit::Close { code, reason } => {
                    self.conn.close(
                        quinn::VarInt::from_u32(u32::from(code.to_u16())),
                        reason.as_bytes(),
                    );
                }
            }
        }
    }

    fn on_outbound(&mut self, out: Outbound) {
        let now = self.now();
        match out {
            Outbound::Cmd(cmd) => {
                let inbound = self.core.apply(cmd, now);
                self.forward(inbound);
            }
            Outbound::World(b) => {
                let n = b.len();
                match &self.world_tx {
                    Some(tx) if self.core.may_send_world() => {
                        self.ctx.stats.messages_out.fetch_add(1, Ordering::Relaxed);
                        if tx.send(b).is_err() {
                            self.shared.world_unsent.fetch_sub(n, Ordering::Relaxed);
                        }
                    }
                    _ => {
                        self.shared.world_unsent.fetch_sub(n, Ordering::Relaxed);
                    }
                }
            }
            Outbound::Chat(b) => {
                let n = b.len();
                match &self.chat_tx {
                    Some(tx) if self.core.may_send_chat() => {
                        self.ctx.stats.messages_out.fetch_add(1, Ordering::Relaxed);
                        if tx.send(b).is_err() {
                            self.shared.chat_unsent.fetch_sub(n, Ordering::Relaxed);
                        }
                    }
                    _ => {
                        self.shared.chat_unsent.fetch_sub(n, Ordering::Relaxed);
                    }
                }
            }
            Outbound::Datagram(b) => {
                if self.core.may_send_world() {
                    self.ctx.stats.messages_out.fetch_add(1, Ordering::Relaxed);
                    self.ctx
                        .stats
                        .bytes_out
                        .fetch_add(b.len() as u64, Ordering::Relaxed);
                    let _ = self.conn.send_datagram(b.into());
                }
            }
        }
        self.flush();
    }
}

async fn uni_writer(
    conn: quinn::Connection,
    kind: StreamKind,
    mut rx: tokio::sync::mpsc::UnboundedReceiver<Vec<u8>>,
    shared: Arc<ConnShared>,
    internal: UnboundedSender<Internal>,
    stats: Arc<Stats>,
) {
    let counter = match kind {
        StreamKind::World => &shared.world_unsent,
        StreamKind::Chat => &shared.chat_unsent,
    };
    let result: Result<(), quinn::WriteError> = async {
        let mut s = conn
            .open_uni()
            .await
            .map_err(quinn::WriteError::ConnectionLost)?;
        s.write_all(&[kind.to_byte()]).await?;
        while let Some(b) = rx.recv().await {
            let n = b.len();
            let r = s.write_all(&b).await;
            counter.fetch_sub(n, Ordering::Relaxed);
            r?;
            stats.bytes_out.fetch_add(n as u64, Ordering::Relaxed);
        }
        Ok(())
    }
    .await;
    if let Err(quinn::WriteError::Stopped(_)) = result {
        let _ = internal.send(Internal::StreamEnded(
            StreamRef::Uni(kind),
            StreamEnd::Stopped,
        ));
    }
    // Whatever is left is never sent: release its accounting.
    rx.close();
    while let Ok(b) = rx.try_recv() {
        counter.fetch_sub(b.len(), Ordering::Relaxed);
    }
}

async fn control_writer(
    mut send: quinn::SendStream,
    mut rx: tokio::sync::mpsc::UnboundedReceiver<CtlWrite>,
    shared: Arc<ConnShared>,
    internal: UnboundedSender<Internal>,
    stats: Arc<Stats>,
) {
    let mut failed = false;
    while let Some(w) = rx.recv().await {
        match w {
            CtlWrite::Data(b) => {
                let n = b.len();
                if !failed {
                    match send.write_all(&b).await {
                        Ok(()) => {
                            stats.bytes_out.fetch_add(n as u64, Ordering::Relaxed);
                        }
                        Err(e) => {
                            failed = true;
                            if matches!(e, quinn::WriteError::Stopped(_)) {
                                let _ = internal.send(Internal::StreamEnded(
                                    StreamRef::Control,
                                    StreamEnd::Stopped,
                                ));
                            }
                        }
                    }
                }
                shared.control_unsent.fetch_sub(n, Ordering::Relaxed);
            }
            CtlWrite::Finish => {
                let _ = send.finish();
            }
        }
    }
}

fn connection_end(e: &quinn::ConnectionError) -> ConnectionEnd {
    match e {
        quinn::ConnectionError::ApplicationClosed(c) => ConnectionEnd::PeerClosed {
            code: AppErrorCode::from_u16(
                u16::try_from(c.error_code.into_inner()).unwrap_or(u16::MAX),
            ),
            reason: String::from_utf8_lossy(&c.reason).into_owned(),
        },
        quinn::ConnectionError::TimedOut => ConnectionEnd::IdleTimeout,
        quinn::ConnectionError::LocallyClosed => ConnectionEnd::Local,
        other => ConnectionEnd::Transport(other.to_string()),
    }
}

struct TaskGuard {
    ctx: Arc<NetCtx>,
}

impl Drop for TaskGuard {
    fn drop(&mut self) {
        self.ctx.stats.net_tasks.fetch_sub(1, Ordering::AcqRel);
    }
}

async fn connection_task(ctx: Arc<NetCtx>, incoming: quinn::Incoming) {
    let _guard = TaskGuard {
        ctx: Arc::clone(&ctx),
    };
    let accepted_at = ctx.clock.now();
    let addr = incoming.remote_address();
    let handshake = tokio::time::timeout(HANDSHAKE_TIMEOUT, async {
        match incoming.accept() {
            Ok(c) => c.await,
            Err(e) => Err(e),
        }
    })
    .await;
    ctx.end_handshake(addr.ip());
    let conn = match handshake {
        Ok(Ok(c)) => {
            ctx.stats.handshakes.fetch_sub(1, Ordering::AcqRel);
            c
        }
        Ok(Err(e)) => {
            ctx.stats.handshakes.fetch_sub(1, Ordering::AcqRel);
            tracing::debug!(%addr, error = %e, "handshake failed");
            return;
        }
        Err(_) => {
            ctx.stats.handshakes.fetch_sub(1, Ordering::AcqRel);
            tracing::debug!(%addr, "handshake timed out");
            return;
        }
    };
    ctx.stats.connections.fetch_add(1, Ordering::AcqRel);
    let id = ctx.next_id.fetch_add(1, Ordering::Relaxed);
    let admitted = ctx.admit(addr);
    tracing::info!(conn = id, %addr, admitted, "connection accepted");
    let (shared, mut out_rx) = ConnShared::new(id, addr, OUTBOUND_QUEUE);
    let shared = Arc::new(shared);
    let (internal_tx, mut internal_rx) = unbounded_channel::<Internal>();
    let mut core = ConnectionCore::new(ctx.params.clone(), accepted_at);
    if !admitted {
        core.refuse();
    }
    let mut task = Task {
        ctx: Arc::clone(&ctx),
        conn: conn.clone(),
        core,
        shared: Arc::clone(&shared),
        admitted,
        reported: false,
        ctl_tx: None,
        world_tx: None,
        chat_tx: None,
        internal_tx,
        paused_since: None,
        slow_reported: false,
    };
    if admitted
        && ctx
            .sim_tx
            .try_send(SimInput::Opened(Arc::clone(&shared)))
            .is_err()
    {
        task.admitted = false;
        task.core.refuse();
    }
    let now = task.now();
    let inbound = task.core.handshake_complete(now, peer_datagrams(&conn));
    task.forward(inbound);
    task.flush();

    let mut recv: Option<quinn::RecvStream> = None;
    let mut accept_open = true;
    let mut datagrams_open = true;
    // Allocated once the client opens its control stream.
    let mut buf: Vec<u8> = Vec::new();
    let end: quinn::ConnectionError = loop {
        let now = task.now();
        let mut deadline = task.core.poll_timeout();
        if task.paused() || task.core.has_unread_control() {
            let p = now + u64::try_from(PAUSE_POLL.as_micros()).unwrap_or(20_000);
            deadline = Some(deadline.map_or(p, |d| d.min(p)));
        }
        let sleep_until = deadline.map_or_else(
            || tokio::time::Instant::now() + Duration::from_secs(3600),
            |d| tokio::time::Instant::from_std(ctx.clock.instant(d)),
        );
        let reading = recv.is_some() && !task.paused() && !task.core.has_unread_control();
        tokio::select! {
            biased;
            e = conn.closed() => break e,
            () = shared.killed() => {
                let code = shared.kill_requested().unwrap_or(AppErrorCode::InternalError);
                conn.close(quinn::VarInt::from_u32(u32::from(code.to_u16())), code.name().as_bytes());
                task.report_closed();
                break conn.closed().await;
            }
            Some(i) = internal_rx.recv() => {
                let Internal::StreamEnded(s, e) = i;
                let inbound = task.core.on_stream_end(s, e);
                task.forward(inbound);
            }
            Some(out) = out_rx.recv() => task.on_outbound(out),
            r = conn.accept_bi(), if accept_open => match r {
                Ok((send, r)) => {
                    if recv.is_none() && task.ctl_tx.is_none() {
                        let (tx, rx) = unbounded_channel();
                        tokio::spawn(control_writer(
                            send,
                            rx,
                            Arc::clone(&shared),
                            task.internal_tx.clone(),
                            Arc::clone(&ctx.stats),
                        ));
                        task.ctl_tx = Some(tx);
                        recv = Some(r);
                        buf = vec![0u8; READ_CHUNK];
                    } else {
                        let inbound = task.core.on_extra_stream();
                        task.forward(inbound);
                    }
                }
                Err(_) => accept_open = false,
            },
            r = async { match recv.as_mut() { Some(s) => s.read(&mut buf).await, None => Ok(None) } }, if reading => {
                let now = task.now();
                match r {
                    Ok(Some(n)) => {
                        ctx.stats.bytes_in.fetch_add(n as u64, Ordering::Relaxed);
                        task.core.push_control(&buf[..n]);
                        task.pump_control(now);
                    }
                    Ok(None) => {
                        recv = None;
                        let inbound = task.core.on_stream_end(StreamRef::Control, StreamEnd::Finished);
                        task.forward(inbound);
                    }
                    Err(quinn::ReadError::Reset(_)) => {
                        recv = None;
                        let inbound = task.core.on_stream_end(StreamRef::Control, StreamEnd::Reset);
                        task.forward(inbound);
                    }
                    Err(_) => recv = None,
                }
            }
            d = conn.read_datagram(), if datagrams_open => match d {
                Ok(bytes) => {
                    ctx.stats.bytes_in.fetch_add(bytes.len() as u64, Ordering::Relaxed);
                    ctx.stats.messages_in.fetch_add(1, Ordering::Relaxed);
                    let now = task.now();
                    let inbound = task.core.on_datagram(&bytes, now);
                    task.forward(inbound);
                }
                Err(_) => datagrams_open = false,
            },
            () = tokio::time::sleep_until(sleep_until) => {
                let now = task.now();
                let inbound = task.core.handle_timeout(now);
                task.forward(inbound);
                let was_paused = task.paused_since;
                task.pump_control(now);
                if let Some(since) = was_paused
                    && task.paused_since.is_some()
                    && now.saturating_sub(since) >= u64::try_from(CONTROL_QUEUE_GRACE.as_micros()).unwrap_or(5_000_000)
                {
                    ctx.stats.slow_consumers.fetch_add(1, Ordering::Relaxed);
                    let inbound = task.core.disconnect(AppErrorCode::SlowConsumer, "", now);
                    task.forward(inbound);
                    task.paused_since = None;
                }
            }
        }
        if !task.slow_reported
            && shared.control_unsent.load(Ordering::Relaxed) > CONTROL_BACKLOG_LIMIT
        {
            task.slow_reported = true;
            let now = task.now();
            ctx.stats.slow_consumers.fetch_add(1, Ordering::Relaxed);
            let inbound = task.core.disconnect(AppErrorCode::SlowConsumer, "", now);
            task.forward(inbound);
        }
        task.flush();
    };
    let inbound = task.core.connection_closed(connection_end(&end));
    task.forward(inbound);
    task.report_closed();
    drop(task.ctl_tx.take());
    drop(task.world_tx.take());
    drop(task.chat_tx.take());
    out_rx.close();
    while let Ok(o) = out_rx.try_recv() {
        match o {
            Outbound::World(b) => {
                shared.world_unsent.fetch_sub(b.len(), Ordering::Relaxed);
            }
            Outbound::Chat(b) => {
                shared.chat_unsent.fetch_sub(b.len(), Ordering::Relaxed);
            }
            _ => {}
        }
    }
    if admitted {
        ctx.release(addr);
    }
    ctx.stats.connections.fetch_sub(1, Ordering::AcqRel);
    let duration_ms = ctx.clock.now().saturating_sub(accepted_at) / 1_000;
    tracing::info!(conn = id, %addr, duration_ms, reason = %end, "connection closed");
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn handshakes_per_address_are_capped() {
        let (tx, _rx) = std::sync::mpsc::sync_channel(1);
        let ctx = NetCtx::new(
            Clock::new(),
            Arc::new(Stats::default()),
            Arc::new(Config::default()),
            ServerParams::default(),
            tx,
        );
        let ip: IpAddr = "192.0.2.1".parse().unwrap();
        for _ in 0..MAX_HANDSHAKES_PER_ADDRESS {
            assert!(ctx.start_handshake(ip));
        }
        assert!(!ctx.start_handshake(ip));
        assert!(ctx.start_handshake("192.0.2.2".parse().unwrap()));
        ctx.end_handshake(ip);
        assert!(ctx.start_handshake(ip));
        // Loopback is exempt.
        let lo: IpAddr = "127.0.0.1".parse().unwrap();
        for _ in 0..100 {
            assert!(ctx.start_handshake(lo));
        }
        for _ in 0..100 {
            ctx.end_handshake(lo);
        }
        assert!(!ctx.handshakes_per_addr.lock().unwrap().contains_key(&lo));
    }
}
