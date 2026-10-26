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
//! Every network operation goes through the run's [`NetworkAccess`] ([`net::LiveNet`]);
//! endpoints come only from the install and the service's answers (decision 0042 item 3).
//! Commands and events cross between the backend and the thread over two bounded
//! channels (`ARCHITECTURE.md` §19.4): `submit` never blocks (a full queue is
//! `SendQueueFull`), `poll` never waits.

pub mod data;
pub mod login_phase;
pub mod net;
pub mod session_phase;
mod worker;

#[cfg(test)]
mod tests;

use std::sync::mpsc::{self, Receiver, SyncSender, TryRecvError, TrySendError};
use std::time::Instant;

use tessera_net::{Command, DisconnectReason, Event, NetError, SocialCommand};

use crate::CompatError;
use crate::backend::Connector;
use crate::credentials::Credentials;
use crate::net_access::NetworkAccess;
use crate::session::SessionError;

pub use data::{InstallData, InstallDataLoader, InstallLocator, LocalPaths};
pub use net::{ConnectorNet, LiveNet, LiveNetFactory, NetFactory};

/// Commands waiting for the connector's thread (`ARCHITECTURE.md` §19.4: 256).
pub const COMMAND_QUEUE: usize = 256;

/// Event batches waiting for `poll` (`ARCHITECTURE.md` §19.4: 1,024 items; a batch is one
/// pass of the thread). When full the thread waits, and the session layer's flow control
/// holds the server back.
pub const EVENT_QUEUE: usize = 1024;

/// Most event batches taken in one `poll`.
pub const MAX_BATCHES_PER_POLL: usize = 1024;

/// The configuration of a live connector: where the install is and how to read its
/// records. Made by the client's official-entry module (`official-entry.md` §6.2).
pub struct LiveConfig {
    /// Paths on this machine.
    pub paths: LocalPaths,
    /// The install's records, made on the connector's thread.
    pub loader: InstallDataLoader,
    /// Finds the install root on the connector's thread; `None` uses `paths.install_root`.
    pub locate: Option<InstallLocator>,
}

impl std::fmt::Debug for LiveConfig {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("LiveConfig")
            .field("paths", &self.paths)
            .finish_non_exhaustive()
    }
}

struct Running {
    commands: SyncSender<Command>,
    events: Receiver<Vec<Event>>,
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
}

/// Whether the live session carries `command` at all (the rest is refused at once,
/// `NetError::InvalidCommand`): the Account commands, `EnterWorld`, `Move`, `Chat` and the
/// chat-server channel commands. Character creation and deletion, and the gameplay groups,
/// wait for their specs.
fn carried(command: &Command) -> bool {
    matches!(
        command,
        Command::Account(_)
            | Command::EnterWorld { .. }
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
        network: &NetworkAccess,
        credentials: Credentials,
    ) -> Result<(), CompatError> {
        let Some(config) = self.config.take() else {
            drop(credentials);
            return Err(CompatError::Session(SessionError::Io(
                "the connector was already used".to_owned(),
            )));
        };
        let net = self.factory.make(network.duplicate());
        let (cmd_tx, cmd_rx) = mpsc::sync_channel(COMMAND_QUEUE);
        let (ev_tx, ev_rx) = mpsc::sync_channel(EVENT_QUEUE);
        let input = worker::WorkerInput {
            paths: config.paths,
            loader: config.loader,
            locate: config.locate,
            credentials,
            commands: cmd_rx,
            events: ev_tx,
        };
        std::thread::Builder::new()
            .name("tessera-official".into())
            .spawn(move || worker::run(net, input))
            .map_err(|_| {
                CompatError::Session(SessionError::Io(
                    "the official connection thread could not start".to_owned(),
                ))
            })?;
        self.running = Some(Running {
            commands: cmd_tx,
            events: ev_rx,
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
        match r.commands.try_send(command) {
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
        let mut closed = false;
        for _ in 0..MAX_BATCHES_PER_POLL {
            match r.events.try_recv() {
                Ok(batch) => {
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
            self.running = None;
        }
    }

    fn disconnect(&mut self, _now: Instant) {
        // Closing the command channel asks the thread to disconnect gracefully and end.
        self.ended = true;
        self.running = None;
    }
}
