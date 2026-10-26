// SPDX-License-Identifier: GPL-3.0-or-later
//! The live connector's thread: the login of `official-login.md` §5.1 from local discovery
//! to the hand-off, then the UDP session ([`super::session_phase`]), reporting through the
//! Account group and the neutral events (`official-entry.md` §8).

use std::sync::mpsc::{Receiver, RecvTimeoutError, SyncSender};
use std::time::{Duration, Instant};

use tessera_net::account::{
    AccountCommand, AccountEvent, LoginRefusal, LoginStage, WorldEntry, WorldId, WorldRefusal,
};
use tessera_net::{Command, DisconnectReason, Event};

use crate::credentials::Credentials;
use crate::login::{QueueEvaluation, SessionHandoff, TicketRefresher, WorldDescriptor};

use super::data::{InstallDataLoader, LocalPaths, SchemaView, discover_local};
use super::login_phase::{
    Authenticated, LoginStop, WORLD_LIST_STATUS_TIMEOUT_SEC, authenticate, discover_service,
    join_status, leave_queue, queue_poll_interval, take_a_number, world_list, world_refusal,
    world_status,
};
use super::net::{ConnectorNet, NetHttp};
use super::session_phase::{SessionDriver, SessionStart, StartFailure};

/// How long the session loop waits for a command before its next pass (the session layer's
/// shortest timer is 0.6 s; engineering choice).
pub const LOOP_WAIT: Duration = Duration::from_millis(5);

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
    /// Finds the install root, when the paths do not name it.
    pub locate: Option<super::data::InstallLocator>,
    /// The one use of the user's credentials.
    pub credentials: Credentials,
    /// Commands from the backend; closed when the backend lets go (`Disconnect`, drop).
    pub commands: Receiver<Command>,
    /// Events to the backend.
    pub events: SyncSender<Vec<Event>>,
}

struct Out {
    tx: SyncSender<Vec<Event>>,
    gone: bool,
}

impl Out {
    fn send(&mut self, events: Vec<Event>) {
        if events.is_empty() || self.gone {
            return;
        }
        if self.tx.send(events).is_err() {
            self.gone = true;
        }
    }

    fn one(&mut self, e: Event) {
        self.send(vec![e]);
    }

    fn progress(&mut self, stage: LoginStage) {
        self.one(Event::Account(AccountEvent::LoginProgress { stage }));
    }

    /// `LoginRefused` and its `Disconnected` in one batch (§8.1: "in the same poll").
    fn refuse(&mut self, reason: LoginRefusal) {
        self.send(vec![
            Event::Account(AccountEvent::LoginRefused {
                reason,
                retry_after_seconds: None,
                detail: None,
            }),
            Event::Disconnected {
                reason: DisconnectReason::TransportFailed("official login refused".to_owned()),
            },
        ]);
    }
}

/// The joined world: what the UDP phase needs from the HTTP phase.
struct Joined {
    handoff: SessionHandoff,
    refresher: TicketRefresher,
}

enum Choice {
    Joined(Box<Joined>),
    Refused(WorldRefusal),
    Left,
    Shutdown,
}

/// Runs the session to its end.
pub fn run<N: ConnectorNet>(mut net: N, input: WorkerInput) {
    let WorkerInput {
        mut paths,
        loader,
        locate,
        credentials,
        commands,
        events,
    } = input;
    let mut out = Out {
        tx: events,
        gone: false,
    };
    let clock = Instant::now();
    if let Some(locate) = locate {
        match locate() {
            Ok(root) => paths.install_root = root,
            Err(why) => {
                drop(credentials);
                tracing::warn!("official login: no install ({why})");
                out.refuse(LoginRefusal::Other);
                return;
            }
        }
    }
    // S1: before any network operation (invariant I1).
    let local = match discover_local(&paths, loader) {
        Ok(l) => l,
        Err(e) => {
            drop(credentials);
            tracing::info!(class = %e.failure_class(), "official login: local discovery failed");
            tracing::debug!(error = %e, "official login: local discovery");
            out.refuse(LoginRefusal::Other);
            return;
        }
    };
    // S2, S3.
    let (dc, remote) = match discover_service(&mut NetHttp(&mut net), &local) {
        Ok(v) => v,
        Err(e) => {
            drop(credentials);
            let LoginStop::Refused(r) = LoginStop::from(&e);
            out.refuse(r);
            return;
        }
    };
    // S5 (the one use of the credentials).
    let auth = match authenticate(&mut NetHttp(&mut net), &local, dc, remote, credentials) {
        Ok(a) => a,
        Err(LoginStop::Refused(r)) => {
            out.refuse(r);
            return;
        }
    };
    // The ticket's lifetime counts from here (`official-login.md` §4.3, `t0`).
    let ticket_installed = clock.elapsed().as_secs_f64();
    tracing::info!("official login: authenticated");
    // The world list (§3.3 sequential status queries, 10 s each).
    out.progress(LoginStage::ListingWorlds);
    let list = {
        let http = &mut NetHttp(&mut net);
        let o = &mut out;
        world_list(&auth.datacenter.worlds, &auth.subscription, |w| {
            let s = world_status(http, w, WORLD_LIST_STATUS_TIMEOUT_SEC);
            // Each answer keeps the progress screen's watchdog alive (§8.3).
            o.progress(LoginStage::ListingWorlds);
            s
        })
    };
    if list.is_empty() {
        out.refuse(LoginRefusal::ServiceUnavailable);
        return;
    }
    let entries: Vec<WorldEntry> = list.iter().map(|(_, e)| e.clone()).collect();
    let send_list = |out: &mut Out| {
        out.one(Event::Account(AccountEvent::WorldList {
            worlds: entries.clone(),
        }));
    };
    send_list(&mut out);
    let joined = loop {
        if out.gone {
            return;
        }
        let cmd = match commands.recv() {
            Ok(c) => c,
            Err(_) => return,
        };
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
                match choose(
                    &mut net,
                    &mut out,
                    &commands,
                    &auth,
                    &local,
                    desc,
                    ticket_installed,
                ) {
                    Choice::Joined(j) => break *j,
                    Choice::Refused(reason) => {
                        out.one(Event::Account(AccountEvent::WorldRefused {
                            world,
                            reason,
                            detail: None,
                        }))
                    }
                    Choice::Left => send_list(&mut out),
                    Choice::Shutdown => return,
                }
            }
            Command::Account(AccountCommand::LeaveLoginQueue) => send_list(&mut out),
            other => tracing::debug!(command = other.name(), "official login: command ignored"),
        }
    };
    drop(auth);
    let start = SessionStart {
        handoff: joined.handoff,
        refresher: joined.refresher,
        version: local.version.clone(),
        codecs: local.codecs.clone(),
        codec_uuids: local.codec_uuids.clone(),
        schema: SchemaView(local.data.clone()),
    };
    let now = || clock.elapsed().as_secs_f64();
    let mut driver = match SessionDriver::start(&mut net, start, now()) {
        Ok(d) => d,
        Err(StartFailure::Address) => {
            out.refuse(LoginRefusal::ServiceUnavailable);
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
            out.refuse(LoginRefusal::Other);
            return;
        }
    };
    session_loop(&mut net, &mut driver, &mut out, &commands, now);
}

fn descriptor<'a>(
    list: &[(usize, WorldEntry)],
    worlds: &'a [WorldDescriptor],
    id: WorldId,
) -> Option<&'a WorldDescriptor> {
    let (index, _) = list.iter().find(|(_, e)| e.id == id)?;
    worlds.get(*index)
}

/// After `ChooseWorld`: the world's status, the billing-role check, the queue and the
/// hand-off values (§3.3–§3.6, §4.2).
fn choose<N: ConnectorNet>(
    net: &mut N,
    out: &mut Out,
    commands: &Receiver<Command>,
    auth: &Authenticated,
    local: &super::data::LocalInputs,
    world: &WorldDescriptor,
    t0: f64,
) -> Choice {
    out.progress(LoginStage::JoiningWorld);
    let status = match join_status(&mut NetHttp(net), world, &auth.subscription) {
        Ok(s) => s,
        Err(e) => {
            tracing::info!(class = %e.failure_class(), "official login: world refused");
            return Choice::Refused(world_refusal(&e));
        }
    };
    if let Some(queue_url) = status.queue_url() {
        let wait = queue_poll_interval(&auth.remote);
        loop {
            match take_a_number(&mut NetHttp(net), auth, queue_url) {
                Ok(QueueEvaluation::Ready) => break,
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
                        match commands.recv_timeout(left) {
                            Ok(Command::Account(AccountCommand::LeaveLoginQueue)) => {
                                leave_queue(
                                    &mut NetHttp(net),
                                    auth,
                                    queue_url,
                                    context_number.as_deref(),
                                );
                                return Choice::Left;
                            }
                            Ok(other) => tracing::debug!(
                                command = other.name(),
                                "official login: command ignored while queued"
                            ),
                            Err(RecvTimeoutError::Timeout) => break,
                            Err(RecvTimeoutError::Disconnected) => {
                                leave_queue(
                                    &mut NetHttp(net),
                                    auth,
                                    queue_url,
                                    context_number.as_deref(),
                                );
                                return Choice::Shutdown;
                            }
                        }
                    }
                }
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
        local.version.clone(),
        local.language.clone(),
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
    let refresher = TicketRefresher::new(
        auth_url,
        auth.subscription.name(),
        auth.account.ticket(),
        lifetime,
        t0,
    );
    Choice::Joined(Box::new(Joined { handoff, refresher }))
}

fn session_loop<N: ConnectorNet>(
    net: &mut N,
    driver: &mut SessionDriver<N>,
    out: &mut Out,
    commands: &Receiver<Command>,
    now: impl Fn() -> f64,
) {
    loop {
        driver.step(now());
        if let Some((player, name)) = driver.wants_chat()
            && let Some(binder) = net.chat_binder()
        {
            driver.attach_chat(binder, player, &name, now());
        }
        out.send(driver.take_events());
        if driver.ended() {
            // The server ended it (or the session failed): nothing to send any more.
            return;
        }
        if out.gone {
            shutdown(driver, &now);
            return;
        }
        match commands.recv_timeout(LOOP_WAIT) {
            Ok(cmd) => {
                if let Err(e) = driver.command(&cmd, now()) {
                    tracing::debug!(command = cmd.name(), error = %e, "official session: command not carried");
                }
            }
            Err(RecvTimeoutError::Timeout) => {}
            Err(RecvTimeoutError::Disconnected) => {
                shutdown(driver, &now);
                return;
            }
        }
    }
}

fn shutdown<N: ConnectorNet>(driver: &mut SessionDriver<N>, now: &impl Fn() -> f64) {
    driver.begin_shutdown(now());
    let until = Instant::now() + SHUTDOWN_LIMIT;
    while Instant::now() < until && driver.shutdown_step(now()) {
        std::thread::sleep(LOOP_WAIT);
    }
    tracing::info!("official session closed");
}
