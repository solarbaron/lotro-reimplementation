// SPDX-License-Identifier: GPL-3.0-or-later
//! The live connector's thread: the login of `official-login.md` §5.1 from local discovery
//! to the hand-off, then the UDP session ([`super::session_phase`]), reporting through the
//! Account group and the neutral events (`official-entry.md` §8).
//!
//! The progress screen's watchdog (`official-entry.md` §8.3: 60 s without an event) is kept
//! alive by a `LoginProgress` at the start and after each of `S1`–`S3`, after each world's
//! status, when the queue lets the user in and when the login connection is established.

use std::sync::Arc;
use std::sync::mpsc::{Receiver, RecvTimeoutError, Sender};
use std::time::{Duration, Instant};

use tessera_net::account::{
    AccountCommand, AccountEvent, LoginRefusal, LoginStage, WorldEntry, WorldId, WorldRefusal,
};
use tessera_net::{Command, DisconnectReason, Event};

use crate::login::{QueueEvaluation, SessionHandoff, TicketRefresher, WorldDescriptor};

use super::control::{Control, Msg};
use super::data::{InstallDataLoader, LocalInputs, LocalPaths, SchemaView, discover_local};
use super::login_phase::{
    Authenticated, LoginStop, PlainSteps, WORLD_LIST_STATUS_TIMEOUT_SEC, authenticate,
    get_datacenters, get_remote_config, join_status, leave_queue, local_refusal,
    queue_poll_interval, take_a_number, world_list, world_refusal, world_status,
};
use super::net::{ConnectorNet, NetHttp};
use super::session_phase::{
    SessionDriver, SessionStart, StartFailure, refusal_cut, resolve_chat_endpoint,
};
use super::{EVENT_QUEUE, MAX_BATCH, READ_PAUSE};

/// Shortest wait of the session loop for a command: after a pass in which something
/// happened (more datagrams may follow at once) and when a timer is already due.
pub const LOOP_WAIT_MIN: Duration = Duration::from_millis(1);

/// Longest wait of the session loop. The session's socket cannot wake the thread (it is
/// read without blocking), so an idle loop still looks at it this often; the chat client
/// and a running ticket refresh, which have no deadline query, are looked at as often.
/// Engineering choice, spec silent.
pub const LOOP_WAIT_MAX: Duration = Duration::from_millis(50);

/// How often the login phase looks at the cancel flag while it waits for a command.
pub const COMMAND_WAIT: Duration = Duration::from_millis(100);

/// Longest time a graceful disconnect is waited for once the backend let go of the session
/// (the session layer's own flush can take 40 s; the thread does not linger that long).
/// Engineering choice, spec silent.
pub const SHUTDOWN_LIMIT: Duration = Duration::from_secs(5);

/// What the thread started with.
pub struct WorkerInput {
    /// Where to look on this machine.
    pub paths: LocalPaths,
    /// The install's records.
    pub loader: InstallDataLoader,
    /// Finds the install, when the paths do not name it.
    pub locate: Option<super::data::InstallLocator>,
    /// Shared with the connector: the cancel flag, the credentials, the newest move and
    /// the count of waiting events.
    pub(super) control: Arc<Control>,
    /// Commands from the backend; closed when the backend lets go (`Disconnect`, drop).
    pub(super) commands: Receiver<Msg>,
    /// Events to the backend.
    pub(super) events: Sender<Vec<Event>>,
}

impl std::fmt::Debug for WorkerInput {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("WorkerInput")
            .field("paths", &self.paths)
            .finish_non_exhaustive()
    }
}

/// The events to the backend, at most [`EVENT_QUEUE`] waiting (counted in events).
struct Out {
    tx: Sender<Vec<Event>>,
    control: Arc<Control>,
    gone: bool,
}

impl Out {
    fn pending(&self) -> usize {
        self.control.pending()
    }

    fn room(&self) -> usize {
        EVENT_QUEUE.saturating_sub(self.pending())
    }

    /// Sends `events` now, in batches of at most [`MAX_BATCH`] (one more when that keeps a
    /// `LoginRefused` and its `Disconnected` together, [`refusal_cut`]: a batch is taken by
    /// one poll); the caller keeps to [`Self::room`].
    fn push(&mut self, events: Vec<Event>) {
        if self.gone {
            return;
        }
        let mut events = events;
        while !events.is_empty() {
            let rest = events.split_off(refusal_cut(&events, MAX_BATCH));
            let batch = std::mem::replace(&mut events, rest);
            let n = batch.len();
            self.control.add_pending(n);
            if self.tx.send(batch).is_err() {
                self.control.take_pending(n);
                self.gone = true;
                return;
            }
        }
    }

    /// The login phase's send: waits (looking at the cancel flag) until the events fit.
    fn send(&mut self, events: Vec<Event>) {
        if events.is_empty() {
            return;
        }
        while !self.gone && !self.control.cancelled() && self.room() < events.len() {
            std::thread::sleep(LOOP_WAIT_MAX);
        }
        if self.control.cancelled() {
            return;
        }
        self.push(events);
    }

    fn one(&mut self, e: Event) {
        self.send(vec![e]);
    }

    fn progress(&mut self, stage: LoginStage) {
        self.one(Event::Account(AccountEvent::LoginProgress { stage }));
    }

    /// `LoginRefused` and its `Disconnected` in one batch (§8.1: "in the same poll").
    fn refuse(&mut self, reason: LoginRefusal, detail: Option<String>) {
        self.send(vec![
            Event::Account(AccountEvent::LoginRefused {
                reason,
                retry_after_seconds: None,
                detail,
            }),
            Event::Disconnected {
                reason: DisconnectReason::TransportFailed("official login refused".to_owned()),
            },
        ]);
    }

    /// A login step failed: its refusal, unless the attempt was cancelled (nothing more
    /// is said) or the network right was revoked (`live` false: the failure is the
    /// revocation's, not the service's, so it ends with [`revoked`], never a refusal).
    fn stopped(&mut self, stop: LoginStop, live: bool) {
        if self.control.cancelled() {
            return;
        }
        if !live {
            self.one(revoked());
            return;
        }
        if let LoginStop::Refused(reason, detail) = stop {
            self.refuse(reason, detail);
        }
    }

    /// The end of an attempt that cannot go on: [`revoked`] when the network right was
    /// revoked and the backend still listens; nothing after a cancel.
    fn end_unless_cancelled(&mut self, live: bool) {
        if !live && !self.control.cancelled() {
            self.one(revoked());
        }
    }
}

/// The joined world: what the UDP phase needs from the HTTP phase.
struct Joined {
    handoff: SessionHandoff,
    refresher: TicketRefresher,
    peer: std::net::SocketAddrV4,
    chat_endpoint: Option<std::net::SocketAddrV4>,
}

/// The end of a session whose network right was revoked (Withdraw, `official-entry.md`
/// §5.5): nothing more can be sent.
fn revoked() -> Event {
    Event::Disconnected {
        reason: DisconnectReason::TransportFailed("the network access was revoked".to_owned()),
    }
}

enum Choice {
    Joined(Box<Joined>),
    Refused(WorldRefusal),
    Left,
    /// The attempt was cancelled: nothing more is said or sent.
    Shutdown,
    /// The network right was revoked (Withdraw): the session ends with [`revoked`], and no
    /// `WorldRefused` is reported for a failure the revocation caused.
    Revoked,
}

/// Runs the session to its end.
pub fn run<N: ConnectorNet>(mut net: N, input: WorkerInput) {
    let WorkerInput {
        mut paths,
        loader,
        locate,
        control,
        commands,
        events,
    } = input;
    let mut out = Out {
        tx: events,
        control: Arc::clone(&control),
        gone: false,
    };
    let stop = || control.cancelled();
    let clock = Instant::now();
    out.progress(LoginStage::Authenticating);
    if let Some(locate) = locate {
        match locate() {
            Ok(found) => {
                paths.install_root = found.root;
                if paths.user_settings_dir.is_none() {
                    paths.user_settings_dir = found.user_settings_dir;
                }
            }
            Err(problem) => {
                // A kind only, never a path (`official-entry.md` §7.3).
                tracing::warn!(%problem, "official login: no install");
                out.refuse(local_refusal(), None);
                return;
            }
        }
    }
    if stop() {
        return;
    }
    // S1: before any network operation (invariant I1).
    let local = match discover_local(&paths, loader) {
        Ok(l) => l,
        Err(e) => {
            tracing::info!(class = %e.failure_class(), "official login: local discovery failed");
            out.refuse(local_refusal(), None);
            return;
        }
    };
    out.progress(LoginStage::Authenticating);
    // S2 (`login_phase` "HTTPS first, then the address as given").
    let plain = PlainSteps::default();
    let dc = match get_datacenters(&mut NetHttp::new(&mut net, &stop), &local, &plain) {
        Ok(v) => v,
        Err(e) => return out.stopped(e, net.is_live()),
    };
    out.progress(LoginStage::Authenticating);
    // S3.
    let remote = match get_remote_config(&mut NetHttp::new(&mut net, &stop), &dc, &plain) {
        Ok(v) => v,
        Err(e) => return out.stopped(e, net.is_live()),
    };
    out.progress(LoginStage::Authenticating);
    // S5 (the one use of the credentials); a cancel has dropped them already.
    if stop() {
        return;
    }
    let Some(credentials) = control.take_credentials() else {
        return;
    };
    let auth = match authenticate(
        &mut NetHttp::new(&mut net, &stop),
        &local,
        dc,
        remote,
        credentials,
        &plain,
    ) {
        Ok(a) => a,
        Err(e) => return out.stopped(e, net.is_live()),
    };
    // The ticket's lifetime counts from here (`official-login.md` §4.3, `t0`).
    let ticket_installed = clock.elapsed().as_secs_f64();
    tracing::info!("official login: authenticated");
    let Some(mut list) = list_worlds(&mut net, &mut out, &stop, &auth) else {
        out.end_unless_cancelled(net.is_live());
        return;
    };
    if list.is_empty() {
        out.refuse(LoginRefusal::ServiceUnavailable, None);
        return;
    }
    send_list(&mut out, &list);
    let joined = loop {
        if out.gone || stop() {
            return;
        }
        if !net.is_live() {
            out.one(revoked());
            return;
        }
        let cmd = match commands.recv_timeout(COMMAND_WAIT) {
            Ok(Msg::Command(c)) => c,
            Ok(Msg::Wake) | Err(RecvTimeoutError::Timeout) => continue,
            Err(RecvTimeoutError::Disconnected) => return,
        };
        if stop() {
            // A command that arrives after the cancel is dropped unread.
            return;
        }
        match cmd {
            Command::Account(AccountCommand::ChooseWorld { world }) => {
                let Some(desc) = descriptor(&list, &auth.datacenter.worlds, world) else {
                    out.one(Event::Account(AccountEvent::WorldRefused {
                        world,
                        reason: WorldRefusal::Other,
                        detail: None,
                    }));
                    continue;
                };
                let ctx = Ctx {
                    commands: &commands,
                    stop: &stop,
                    auth: &auth,
                    local: &local,
                    t0: ticket_installed,
                };
                match choose(&mut net, &mut out, &ctx, desc) {
                    Choice::Joined(j) => break *j,
                    Choice::Refused(_) if stop() => return,
                    Choice::Refused(_) | Choice::Revoked if !net.is_live() => {
                        out.end_unless_cancelled(false);
                        return;
                    }
                    Choice::Revoked => return,
                    Choice::Refused(reason) => {
                        out.one(Event::Account(AccountEvent::WorldRefused {
                            world,
                            reason,
                            detail: None,
                        }));
                    }
                    Choice::Left => {
                        // The statuses may have changed while queued: asked again.
                        match list_worlds(&mut net, &mut out, &stop, &auth) {
                            Some(l) if !l.is_empty() => {
                                list = l;
                                send_list(&mut out, &list);
                            }
                            Some(_) => {
                                out.refuse(LoginRefusal::ServiceUnavailable, None);
                                return;
                            }
                            None => {
                                out.end_unless_cancelled(net.is_live());
                                return;
                            }
                        }
                    }
                    Choice::Shutdown => return,
                }
            }
            Command::Account(AccountCommand::LeaveLoginQueue) => send_list(&mut out, &list),
            other => tracing::debug!(command = other.name(), "official login: command ignored"),
        }
    };
    let plain = auth.plain.clone();
    drop(auth);
    if stop() {
        return;
    }
    if !net.is_live() {
        out.one(revoked());
        return;
    }
    let start = SessionStart {
        handoff: joined.handoff,
        login_peer: joined.peer,
        plain,
        refresher: joined.refresher,
        chat_endpoint: joined.chat_endpoint,
        version: local.version.clone(),
        codecs: local.codecs.clone(),
        codec_uuids: local.codec_uuids.clone(),
        schema: SchemaView(local.data.clone()),
        cancel: control.cancel_flag(),
    };
    let now = || clock.elapsed().as_secs_f64();
    let mut driver = match SessionDriver::start(&mut net, start, now(), &stop) {
        Ok(d) => d,
        Err(StartFailure::Cancelled) => return,
        Err(_) if !net.is_live() => {
            out.end_unless_cancelled(false);
            return;
        }
        Err(StartFailure::Address) => {
            out.refuse(LoginRefusal::ServiceUnavailable, None);
            return;
        }
        Err(StartFailure::Bind) => {
            out.one(Event::Disconnected {
                reason: DisconnectReason::TransportFailed(
                    "the official connection could not be opened".to_owned(),
                ),
            });
            return;
        }
        Err(StartFailure::Ticket) => {
            out.refuse(LoginRefusal::Other, None);
            return;
        }
    };
    session_loop(&mut net, &mut driver, &mut out, &commands, &control, now);
}

/// The world list with each world's status (§3.3 sequential status queries, 10 s each),
/// keeping the watchdog alive; `None` when the attempt was cancelled or the network right
/// was revoked (a status the revocation failed is not reported as the world's).
fn list_worlds<N: ConnectorNet>(
    net: &mut N,
    out: &mut Out,
    stop: &dyn Fn() -> bool,
    auth: &Authenticated,
) -> Option<Vec<(usize, WorldEntry)>> {
    out.progress(LoginStage::ListingWorlds);
    let mut http = NetHttp::new(net, stop);
    world_list(&auth.datacenter.worlds, &auth.subscription, |w| {
        if stop() || !http.is_live() {
            return None;
        }
        let s = world_status(
            &mut http,
            &w.status_server_url,
            WORLD_LIST_STATUS_TIMEOUT_SEC,
            &auth.plain,
        );
        if stop() || !http.is_live() {
            return None;
        }
        out.progress(LoginStage::ListingWorlds);
        Some(s)
    })
}

fn send_list(out: &mut Out, list: &[(usize, WorldEntry)]) {
    out.one(Event::Account(AccountEvent::WorldList {
        worlds: list.iter().map(|(_, e)| e.clone()).collect(),
    }));
}

fn descriptor<'a>(
    list: &[(usize, WorldEntry)],
    worlds: &'a [WorldDescriptor],
    id: WorldId,
) -> Option<&'a WorldDescriptor> {
    let (index, _) = list.iter().find(|(_, e)| e.id == id)?;
    worlds.get(*index)
}

/// What [`choose`] reads.
struct Ctx<'a> {
    commands: &'a Receiver<Msg>,
    stop: &'a dyn Fn() -> bool,
    auth: &'a Authenticated,
    local: &'a LocalInputs,
    t0: f64,
}

/// After `ChooseWorld`: the world's status, the billing-role check, the queue and the
/// hand-off values (§3.3–§3.6, §4.2), with the login server's and the chat server's
/// addresses resolved (an unusable login server refuses the world, §5.1 `S7`). The cancel
/// flag is looked at before each name lookup and after the last, so a cancel before or
/// during either binds and sends nothing. A failure while the network right is revoked
/// (Withdraw) is [`Choice::Revoked`], never a refusal of the world; the queue's wait ends
/// on a revocation too.
fn choose<N: ConnectorNet>(
    net: &mut N,
    out: &mut Out,
    ctx: &Ctx<'_>,
    world: &WorldDescriptor,
) -> Choice {
    let stop = ctx.stop;
    let auth = ctx.auth;
    // Why the attempt cannot go on, if it cannot.
    let halted = |net: &N| {
        if stop() {
            Some(Choice::Shutdown)
        } else if !net.is_live() {
            Some(Choice::Revoked)
        } else {
            None
        }
    };
    out.progress(LoginStage::JoiningWorld);
    let status = match join_status(
        &mut NetHttp::new(net, stop),
        world,
        &auth.subscription,
        &auth.plain,
    ) {
        Ok(s) => s,
        Err(_) if halted(net).is_some() => return halted(net).unwrap_or(Choice::Shutdown),
        Err(e) => {
            tracing::info!(class = %e.failure_class(), "official login: world refused");
            return Choice::Refused(world_refusal(&e));
        }
    };
    if let Some(queue_url) = status.queue_url() {
        let wait = queue_poll_interval(&auth.remote);
        loop {
            if let Some(h) = halted(net) {
                return h;
            }
            match take_a_number(&mut NetHttp::new(net, stop), auth, queue_url) {
                Ok(QueueEvaluation::Ready) => {
                    out.progress(LoginStage::JoiningWorld);
                    break;
                }
                Ok(QueueEvaluation::Waiting {
                    ahead,
                    context_number,
                }) => {
                    out.one(Event::Account(AccountEvent::LoginQueueChanged {
                        position: Some(ahead),
                        length: None,
                        wait_seconds: None,
                    }));
                    let deadline = Instant::now() + wait;
                    loop {
                        let left = deadline.saturating_duration_since(Instant::now());
                        if left.is_zero() {
                            break;
                        }
                        match ctx.commands.recv_timeout(left.min(COMMAND_WAIT)) {
                            Ok(Msg::Command(_)) if halted(net).is_some() => {
                                // Dropped unread: the attempt ended.
                                return halted(net).unwrap_or(Choice::Shutdown);
                            }
                            Ok(Msg::Command(Command::Account(AccountCommand::LeaveLoginQueue))) => {
                                leave_queue(
                                    &mut NetHttp::new(net, stop),
                                    auth,
                                    queue_url,
                                    context_number.as_deref(),
                                );
                                return Choice::Left;
                            }
                            Ok(Msg::Command(other)) => tracing::debug!(
                                command = other.name(),
                                "official login: command ignored while queued"
                            ),
                            Ok(Msg::Wake) | Err(RecvTimeoutError::Timeout) => {
                                if let Some(h) = halted(net) {
                                    return h;
                                }
                            }
                            // The backend let go: nothing more goes on the network, not even
                            // `LeaveQueue` (the place lapses on the service's side).
                            // Engineering choice, spec silent (question with the spec
                            // writers).
                            Err(RecvTimeoutError::Disconnected) => return Choice::Shutdown,
                        }
                    }
                }
                Err(_) if halted(net).is_some() => return halted(net).unwrap_or(Choice::Shutdown),
                Err(e) => {
                    tracing::info!(class = %e.failure_class(), "official login: queue failed");
                    return Choice::Refused(WorldRefusal::Other);
                }
            }
        }
    }
    let auth_url = auth
        .remote
        .effective_auth_server_url(&auth.datacenter.auth_server)
        .to_owned();
    let lifetime = auth.remote.gls_ticket_lifetime_sec;
    let handoff = match SessionHandoff::new(
        status.login_server(),
        0,
        auth.subscription.name(),
        auth.account.ticket(),
        ctx.local.version.clone(),
        ctx.local.language.clone(),
        world.chat_server_url.clone(),
        auth_url.clone(),
        lifetime,
    ) {
        Ok(h) => h,
        Err(e) => {
            tracing::info!(class = %e.failure_class(), "official login: no usable login server");
            return Choice::Refused(world_refusal(&e));
        }
    };
    if let Some(h) = halted(net) {
        return h;
    }
    let Some(peer) = login_peer(net, &handoff) else {
        if let Some(h) = halted(net) {
            return h;
        }
        tracing::info!("official login: the login server's address is not usable");
        return Choice::Refused(WorldRefusal::Other);
    };
    if let Some(h) = halted(net) {
        return h;
    }
    let chat_endpoint = resolve_chat_endpoint(net, &handoff);
    if let Some(h) = halted(net) {
        return h;
    }
    let refresher = TicketRefresher::new(
        auth_url,
        auth.subscription.name(),
        auth.account.ticket(),
        lifetime,
        ctx.t0,
    );
    Choice::Joined(Box::new(Joined {
        handoff,
        refresher,
        peer,
        chat_endpoint,
    }))
}

/// The login server's address (`official-session.md` §3.3: the `-h` value the hand-off
/// carries, resolved at run time).
fn login_peer<N: ConnectorNet>(
    net: &mut N,
    handoff: &SessionHandoff,
) -> Option<std::net::SocketAddrV4> {
    let host = handoff.login_host().ok()?;
    let port = host.remote_port(None).ok()?;
    net.resolve(&host.remote_host, port).ok()
}

/// How long the session loop waits for a command before its next pass: until `deadline`
/// (the driver's [`SessionDriver::next_deadline`], on the clock of `now`), at least
/// [`LOOP_WAIT_MIN`] and at most [`LOOP_WAIT_MAX`]; the longest wait without a deadline. A
/// command ends the wait at once either way.
#[must_use]
pub fn loop_wait(now: f64, deadline: Option<f64>) -> Duration {
    let Some(deadline) = deadline else {
        return LOOP_WAIT_MAX;
    };
    let left = deadline - now;
    if left.is_nan() || left <= LOOP_WAIT_MIN.as_secs_f64() {
        return LOOP_WAIT_MIN;
    }
    if left >= LOOP_WAIT_MAX.as_secs_f64() {
        return LOOP_WAIT_MAX;
    }
    // Finite and between the two bounds here: the conversion cannot panic. One microsecond
    // more, so that rounding never brings the pass just before the timer.
    Duration::from_secs_f64(left + 1e-6).clamp(LOOP_WAIT_MIN, LOOP_WAIT_MAX)
}

/// The session loop: a pass ([`SessionDriver::step`]), then a wait for a command until
/// the driver's next deadline ([`loop_wait`]); after a pass in which something happened
/// the wait is the shortest.
fn session_loop<N: ConnectorNet>(
    net: &mut N,
    driver: &mut SessionDriver<N>,
    out: &mut Out,
    commands: &Receiver<Msg>,
    control: &Control,
    now: impl Fn() -> f64,
) {
    let stop = || control.cancelled();
    loop {
        if out.gone || control.cancelled() {
            shutdown(net, driver, &now);
            return;
        }
        if !net.is_live() {
            // Revoked without a cancel: nothing can be sent, not even the disconnect.
            out.push(vec![revoked()]);
            tracing::info!("official session ended: the network access was revoked");
            return;
        }
        let t = now();
        // A full event queue: stop reading the socket (§19.4); ticks and acknowledgements
        // go on. The driver's own events count too.
        let read = out.pending().saturating_add(driver.event_count()) < READ_PAUSE;
        let mut busy = driver.step(t, read, &stop);
        if let Some((player, name)) = driver.wants_chat(t)
            && !control.cancelled()
        {
            match net.chat_binder() {
                Some(binder) => driver.attach_chat(binder, player, &name, t, &stop),
                None => driver.chat_failed(t),
            }
        }
        if let Some(m) = control.take_move()
            && !control.cancelled()
        {
            if let Err(e) = driver.command(&m, t) {
                tracing::debug!(error = %e, "official session: move not carried");
            }
            busy = true;
        }
        let room = out.room();
        if room > 0 && driver.has_events() {
            out.push(driver.take_events_up_to(room));
            busy = true;
        }
        if driver.ended() && !driver.has_events() {
            // The server ended it (or the session failed): nothing to send any more.
            return;
        }
        if out.gone || control.cancelled() {
            shutdown(net, driver, &now);
            return;
        }
        let t = now();
        // Datagrams still queued after the pass's flush are deliberately not "due now": the
        // flush stops at a failed send, and a send error that persists (a revoked right
        // fails every send) would otherwise wake this loop every `LOOP_WAIT_MIN`. The next
        // timer or `LOOP_WAIT_MAX` tries the queue again (`SessionMultiplexer::next_deadline`).
        let deadline = if busy {
            Some(t)
        } else {
            driver.next_deadline(t)
        };
        match commands.recv_timeout(loop_wait(t, deadline)) {
            Ok(Msg::Command(_)) if control.cancelled() || out.gone => {
                // Dropped unread: after the cancel nothing new goes out (no `0x6F`, no chat
                // line for the shutdown to flush).
                shutdown(net, driver, &now);
                return;
            }
            Ok(Msg::Command(cmd)) => {
                if let Err(e) = driver.command(&cmd, now()) {
                    tracing::debug!(command = cmd.name(), error = %e, "official session: command not carried");
                }
            }
            // The next pass takes the move; a timeout is the deadline (or the longest wait).
            Ok(Msg::Wake) | Err(RecvTimeoutError::Timeout) => {}
            Err(RecvTimeoutError::Disconnected) => {
                shutdown(net, driver, &now);
                return;
            }
        }
    }
}

/// The graceful end after the backend let go: the transport disconnect on every connected
/// connection, for at most [`SHUTDOWN_LIMIT`] (`official-session.md` §8.2). It is the only
/// traffic after a cancel; nothing new is started, and a connection still in its handshake
/// is closed without a word ([`SessionDriver::begin_shutdown`]). When the network right is
/// already revoked (Withdraw) nothing is tried at all ([`SessionDriver::abandon`]); a
/// revocation during the shutdown ends it at once.
fn shutdown<N: ConnectorNet>(net: &N, driver: &mut SessionDriver<N>, now: &impl Fn() -> f64) {
    if !net.is_live() {
        driver.abandon();
        tracing::info!("official session closed: the network access was revoked");
        return;
    }
    driver.begin_shutdown(now());
    let until = Instant::now() + SHUTDOWN_LIMIT;
    while Instant::now() < until && net.is_live() && driver.shutdown_step(now()) {
        let t = now();
        let left = until.saturating_duration_since(Instant::now());
        std::thread::sleep(loop_wait(t, driver.next_deadline(t)).min(left));
    }
    tracing::info!("official session closed");
}

#[cfg(test)]
mod tests {
    use super::*;
    use tessera_net::account::LoginRefusal;

    #[test]
    fn a_batch_never_parts_a_refusal_from_its_disconnect() {
        let control = Arc::new(Control::default());
        let (tx, rx) = std::sync::mpsc::channel();
        let mut out = Out {
            tx,
            control: Arc::clone(&control),
            gone: false,
        };
        let mut events: Vec<Event> = (0..MAX_BATCH - 1)
            .map(|_| {
                Event::Account(AccountEvent::LoginProgress {
                    stage: LoginStage::JoiningWorld,
                })
            })
            .collect();
        events.push(Event::Account(AccountEvent::LoginRefused {
            reason: LoginRefusal::Other,
            retry_after_seconds: None,
            detail: None,
        }));
        events.push(revoked());
        out.push(events);
        let first = rx.try_recv().expect("one batch");
        assert_eq!(first.len(), MAX_BATCH + 1);
        assert!(matches!(first.last(), Some(Event::Disconnected { .. })));
        assert!(rx.try_recv().is_err(), "nothing split off");
        assert_eq!(control.pending(), MAX_BATCH + 1);
    }

    #[test]
    fn batches_are_cut_at_the_batch_size_otherwise() {
        let control = Arc::new(Control::default());
        let (tx, rx) = std::sync::mpsc::channel();
        let mut out = Out {
            tx,
            control,
            gone: false,
        };
        let events: Vec<Event> = (0..2 * MAX_BATCH + 1).map(|_| revoked()).collect();
        out.push(events);
        let sizes: Vec<usize> = rx.try_iter().map(|b| b.len()).collect();
        assert_eq!(sizes, vec![MAX_BATCH, MAX_BATCH, 1]);
    }
}
