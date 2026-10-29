// SPDX-License-Identifier: GPL-3.0-or-later
//! The live connector of [`crate::CompatBackend`]: the official login and session behind
//! the neutral `Backend` model (`official-entry.md` §4.3, §8; `official-login.md`;
//! `official-session.md`; `official-messages-world.md` §3–§4; `official-messages-play.md`
//! §4–§8).
//!
//! [`LiveConnector::connect`] starts one thread per login attempt. The thread runs, in
//! order (`official-login.md` §5.1):
//!
//! 1. local discovery before any network operation (invariant `I1`; [`data`]): the launcher
//!    configuration, the language, `netver` and `didver`, the compression dictionaries;
//! 2. the HTTP exchanges ([`login_phase`]): datacenters, remote configuration,
//!    `LoginAccount` (the one use of the credentials), the world list with each world's
//!    status, then on `ChooseWorld` the status again, the billing-role check and the queue;
//! 3. the UDP session ([`session_phase`]): `SessionAuthHeader`, the version exchange, the
//!    character list (`Event::Connected`), `EnterWorld`, the referral to the world server,
//!    the play messages, the chat server and the in-game ticket refresh.
//!
//! Every network operation goes through the run's [`NetworkAccess`] ([`net::LiveNet`]),
//! which is moved into the thread (there is no copy of it); endpoints come only from the
//! install and the service's answers (decision 0042 item 3). Commands and events cross
//! between the backend and the thread as `ARCHITECTURE.md` §19.4 has it: a command queue of
//! [`COMMAND_QUEUE`] (`submit` never blocks; a full queue is `SendQueueFull`), a
//! newest-move slot of one (moves never fill the queue), a cancel flag, and at most
//! [`EVENT_QUEUE`] events waiting for `poll` (counted in events, not batches; `poll` never
//! waits).
//!
//! # Ending a session
//!
//! `Disconnect` and dropping the connector cancel the thread: the credentials, if not yet
//! used, are dropped at once; no further HTTP exchange, login step or connection is started
//! (every exchange, the authentication, each step of the world list and the session start
//! check the flag); a running UDP session sends its graceful disconnect within
//! [`worker::SHUTDOWN_LIMIT`]. A reaper thread waits for the thread for at most
//! [`REAP_LIMIT`] and then revokes the attempt's network right, so a thread still blocked
//! in an exchange can do nothing more when it returns. A withdrawal of the acceptance ends
//! every right at once ([`crate::net_access::revoke_all`]); the thread sees it at its next
//! pass (or its next exchange) and ends without trying to send, its graceful disconnect
//! included.
//!
//! The flag is looked at before and after each name lookup of the login and chat servers,
//! and the session's start looks at it again right before its local-address probe, its bind
//! and its first send: a cancel before or during a lookup probes, binds and sends nothing.
//! A command that arrives after the cancel is dropped unread. Once cancelled, the session
//! loop accepts no referral, starts no ticket refresh (the refresh thread itself looks at
//! the flag right before its request) and opens no chat session; the shutdown sends the
//! graceful disconnect only on connected connections and closes one still in its handshake
//! without a word (`session_phase` "Ending the session"). A failure caused by a revoked
//! right (the queue's wait included, which ends on the revocation) ends the session with
//! "the network access was revoked", never with a refusal of the login or the world.

mod control;
pub mod data;
pub mod diag;
pub mod entities;
pub mod login_phase;
pub mod net;
pub mod session_phase;
#[cfg(any(test, feature = "test-support"))]
pub mod test_support;
pub mod worker;

#[cfg(test)]
mod tests;

use std::sync::Arc;
use std::sync::mpsc::{self, Receiver, SyncSender, TryRecvError, TrySendError};
use std::thread::JoinHandle;
use std::time::{Duration, Instant};

use tessera_net::{Command, DisconnectReason, Event, NetError, SocialCommand};

use crate::CompatError;
use crate::backend::Connector;
use crate::credentials::Credentials;
use crate::net_access::{NetworkAccess, Revocation};
use crate::session::SessionError;

use control::{Control, Msg};

pub use data::{
    InstallData, InstallDataLoader, InstallLocator, InstallProblem, LocalPaths, LocatedInstall,
};
pub use net::{ConnectorNet, LiveNet, LiveNetFactory, NetFactory};

/// Commands waiting for the connector's thread (`ARCHITECTURE.md` §19.4: 256).
pub const COMMAND_QUEUE: usize = 256;

/// Events waiting for `poll` (`ARCHITECTURE.md` §19.4: 1,024 items). When the session
/// loop has this many waiting it holds its own events back and stops reading the socket
/// (it keeps acknowledging and sending keep-alives); the login phase waits.
pub const EVENT_QUEUE: usize = 1024;

/// Events the session loop lets wait before it stops reading the socket
/// (`ARCHITECTURE.md` §19.4: reading stops once 256 items wait): those waiting for `poll`
/// and those the session driver holds, together.
pub const READ_PAUSE: usize = 256;

/// Most events in one batch on the channel.
pub const MAX_BATCH: usize = 64;

/// Longest wait for the thread after `Disconnect` or a drop before its network right is
/// revoked: the graceful disconnect's limit plus a margin. Engineering choice, spec silent.
pub const REAP_LIMIT: Duration = Duration::from_secs(7);

/// The configuration of a live connector: where the install is and how to read its
/// records. Made by the client's official-entry module (`official-entry.md` §6.2).
pub struct LiveConfig {
    /// Paths and values from the client (the install root is replaced by what `locate`
    /// finds, when given).
    pub paths: LocalPaths,
    /// The install's records, made on the connector's thread.
    pub loader: InstallDataLoader,
    /// Finds the install on the connector's thread; `None` uses `paths`.
    pub locate: Option<InstallLocator>,
}

impl std::fmt::Debug for LiveConfig {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("LiveConfig")
            .field("paths", &self.paths)
            .finish_non_exhaustive()
    }
}

/// A running attempt. Dropping it cancels the thread and hands it to a reaper (module
/// docs).
struct Running {
    commands: Option<SyncSender<Msg>>,
    events: Option<Receiver<Vec<Event>>>,
    control: Arc<Control>,
    worker: Option<JoinHandle<()>>,
    revocation: Revocation,
}

impl Running {
    fn stop(&mut self) {
        self.control.cancel();
        // Closing both channels: the thread sees the backend gone at its next wait.
        self.commands = None;
        self.events = None;
        let Some(worker) = self.worker.take() else {
            return;
        };
        let revocation = self.revocation.clone();
        let reaper = std::thread::Builder::new()
            .name("tessera-official-reaper".into())
            .spawn(move || reap(worker, &revocation));
        if reaper.is_err() {
            // Nothing can wait for the thread: its right ends now (its graceful disconnect
            // is lost).
            self.revocation.revoke();
        }
    }
}

impl Drop for Running {
    fn drop(&mut self) {
        self.stop();
    }
}

/// Waits for the thread for at most [`REAP_LIMIT`], then revokes its network right.
fn reap(worker: JoinHandle<()>, revocation: &Revocation) {
    let until = Instant::now() + REAP_LIMIT;
    while !worker.is_finished() && Instant::now() < until {
        std::thread::sleep(Duration::from_millis(10));
    }
    revocation.revoke();
    if worker.is_finished() {
        let _ = worker.join();
    } else {
        tracing::info!("official connection thread still blocked; its network right is revoked");
    }
}

/// The live connector (module docs).
pub struct LiveConnector<F: NetFactory = LiveNetFactory> {
    config: Option<LiveConfig>,
    factory: F,
    running: Option<Running>,
    /// A `Disconnected` was handed out (nothing more is).
    ended: bool,
}

impl<F: NetFactory> std::fmt::Debug for LiveConnector<F> {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("LiveConnector")
            .field("config", &self.config)
            .field("running", &self.running.is_some())
            .field("ended", &self.ended)
            .finish_non_exhaustive()
    }
}

impl LiveConnector<LiveNetFactory> {
    /// The connector of this build, over the run's network-access value.
    #[must_use]
    pub fn new(config: LiveConfig) -> Self {
        Self::with_factory(config, LiveNetFactory)
    }
}

impl<F: NetFactory> LiveConnector<F> {
    /// A connector whose network `factory` makes (tests use one that opens nothing).
    #[must_use]
    pub fn with_factory(config: LiveConfig, factory: F) -> Self {
        Self {
            config: Some(config),
            factory,
            running: None,
            ended: false,
        }
    }

    /// Events sent by the thread and not yet taken by `poll` (diagnostics and tests).
    #[must_use]
    pub fn pending_events(&self) -> usize {
        self.running.as_ref().map_or(0, |r| r.control.pending())
    }
}

/// Whether the live session carries `command` at all (the rest is refused at once,
/// `NetError::InvalidCommand`): the Account commands, `EnterWorld`, `LeaveWorld`,
/// `WorldLoaded`, `Move`, `Chat` and the chat-server channel commands. Character creation
/// and deletion, and the gameplay groups, wait for their specs.
fn carried(command: &Command) -> bool {
    matches!(
        command,
        Command::Account(_)
            | Command::EnterWorld { .. }
            | Command::LeaveWorld
            | Command::WorldLoaded
            | Command::Move { .. }
            | Command::Chat { .. }
            | Command::Social(
                SocialCommand::JoinChannel { .. } | SocialCommand::LeaveChannel { .. }
            )
    )
}

impl<F: NetFactory> Connector for LiveConnector<F> {
    fn connect(
        &mut self,
        network: NetworkAccess,
        credentials: Credentials,
    ) -> Result<(), CompatError> {
        let Some(config) = self.config.take() else {
            drop(credentials);
            return Err(CompatError::Session(SessionError::Io(
                "the connector was already used".to_owned(),
            )));
        };
        let revocation = network.revocation();
        let net = self.factory.make(network);
        let control = Arc::new(Control::new(credentials));
        let (cmd_tx, cmd_rx) = mpsc::sync_channel(COMMAND_QUEUE);
        let (ev_tx, ev_rx) = mpsc::channel();
        let input = worker::WorkerInput {
            paths: config.paths,
            loader: config.loader,
            locate: config.locate,
            control: Arc::clone(&control),
            commands: cmd_rx,
            events: ev_tx,
        };
        // The worker logs where the starting thread logs: the process's subscriber, or the
        // one a test set for its own thread (log-capture tests of the login's lines).
        let dispatch = tracing::dispatcher::get_default(Clone::clone);
        let worker = std::thread::Builder::new()
            .name("tessera-official".into())
            .spawn(move || {
                tracing::dispatcher::with_default(&dispatch, || worker::run(net, input))
            });
        let worker = match worker {
            Ok(w) => w,
            Err(_) => {
                control.cancel();
                revocation.revoke();
                return Err(CompatError::Session(SessionError::Io(
                    "the official connection thread could not start".to_owned(),
                )));
            }
        };
        self.running = Some(Running {
            commands: Some(cmd_tx),
            events: Some(ev_rx),
            control,
            worker: Some(worker),
            revocation,
        });
        tracing::info!("official login started");
        Ok(())
    }

    fn submit(&mut self, _now: Instant, command: Command) -> Result<(), NetError> {
        if !carried(&command) {
            return Err(NetError::InvalidCommand(
                "this command is not supported on the official servers yet",
            ));
        }
        let Some(r) = self.running.as_ref() else {
            return Err(NetError::InvalidCommand(
                "the official session is not running",
            ));
        };
        let Some(tx) = r.commands.as_ref() else {
            return Ok(());
        };
        if matches!(command, Command::Move { .. }) {
            // The newest-move slot (§19.4): a move replaces the one waiting.
            if r.control.put_move(command) && tx.try_send(Msg::Wake).is_err() {
                r.control.wake_not_queued();
            }
            return Ok(());
        }
        match tx.try_send(Msg::Command(command)) {
            Ok(()) => Ok(()),
            Err(TrySendError::Full(_)) => Err(NetError::SendQueueFull),
            // The thread ended; its `Disconnected` is on the way.
            Err(TrySendError::Disconnected(_)) => Ok(()),
        }
    }

    fn poll(&mut self, _now: Instant, out: &mut Vec<Event>) {
        if self.ended {
            return;
        }
        let Some(r) = self.running.as_ref() else {
            return;
        };
        let Some(rx) = r.events.as_ref() else {
            return;
        };
        let mut closed = false;
        let mut taken = 0usize;
        while taken < EVENT_QUEUE {
            match rx.try_recv() {
                Ok(batch) => {
                    r.control.take_pending(batch.len());
                    taken += batch.len();
                    for e in batch {
                        let last = matches!(e, Event::Disconnected { .. });
                        out.push(e);
                        if last {
                            self.ended = true;
                            break;
                        }
                    }
                    if self.ended {
                        break;
                    }
                }
                Err(TryRecvError::Empty) => break,
                Err(TryRecvError::Disconnected) => {
                    closed = true;
                    break;
                }
            }
        }
        if closed && !self.ended {
            self.ended = true;
            out.push(Event::Disconnected {
                reason: DisconnectReason::TransportFailed(
                    "the official connection ended".to_owned(),
                ),
            });
        }
        if self.ended {
            // Dropping it cancels the thread (if it still runs) and reaps it.
            self.running = None;
        }
    }

    fn disconnect(&mut self, _now: Instant) {
        // Cancels the thread: no further login step; a session says Disconnect gracefully.
        self.ended = true;
        self.running = None;
    }
}
