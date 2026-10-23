// SPDX-License-Identifier: GPL-3.0-or-later
//! [`CompatBackend`]: the [`Backend`] shell for `tessera-compat`
//! (`docs/specs/official-entry.md` §4.3, §4.4, §6.1).
//!
//! Every constructor takes a [`CompatConfig`], which can only be made from a
//! [`ConsentProof`] (by value) and the user's [`Credentials`]; the backend refuses to be built
//! when the proof is not for the shipped warning text. While [`PlaceholderConnector`] is the
//! only connector built into this binary, [`availability`] answers
//! [`Availability::NotBuilt`] and a placeholder backend answers `Connect` with
//! `LoginRefused { reason: NotAvailable }` and one `Disconnected` in the same poll, opening
//! no socket.
//!
//! A backend cannot be built without a proof (OE-T8 (d)). Each `compile_fail` example below
//! differs from the compiling one only in the step it is about (the error code is checked by
//! nightly rustdoc):
//!
//! ```
//! use tessera_compat::consent::ConsentProof;
//! use tessera_compat::{CompatBackend, CompatConfig, CompatError, Credentials};
//! fn built(p: ConsentProof, c: Credentials) -> Result<CompatBackend, CompatError> {
//!     CompatBackend::new(CompatConfig::new(p, c))
//! }
//! ```
//!
//! Without the proof the configuration cannot be made (wrong number of arguments):
//!
//! ```compile_fail,E0061
//! use tessera_compat::{CompatBackend, CompatConfig, CompatError, Credentials};
//! fn built(c: Credentials) -> Result<CompatBackend, CompatError> {
//!     CompatBackend::new(CompatConfig::new(c))
//! }
//! ```
//!
//! No default configuration (`CompatConfig: Default` is not satisfied):
//!
//! ```compile_fail,E0277
//! use tessera_compat::{CompatBackend, CompatConfig, CompatError};
//! fn built() -> Result<CompatBackend, CompatError> {
//!     CompatBackend::new(Default::default())
//! }
//! ```
//!
//! No copy of a configuration (`CompatConfig: Clone` is not satisfied):
//!
//! ```compile_fail,E0277
//! use tessera_compat::CompatConfig;
//! fn copy(c: &CompatConfig) -> CompatConfig {
//!     Clone::clone(c)
//! }
//! ```

use std::time::Instant;

use tessera_net::account::{AccountEvent, LoginRefusal};
use tessera_net::{Backend, Command, DisconnectReason, Event, NetError};

use crate::CompatError;
use crate::consent::ConsentProof;
use crate::credentials::Credentials;
use crate::net_access::NetworkAccess;

/// Whether a live official-server connector is built into this binary (§4.4). Answered
/// without a network and without a consent proof.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Availability {
    /// A live connector is built into this binary.
    Available,
    /// Only the placeholder connector exists.
    NotBuilt,
}

/// The availability of official play in this binary (§4.4): [`Availability::NotBuilt`] while
/// [`PlaceholderConnector`] is the only connector.
#[must_use]
pub fn availability() -> Availability {
    Availability::NotBuilt
}

/// Pluggable connection driver behind [`CompatBackend`].
pub trait Connector: Send + std::fmt::Debug {
    /// Opens the connection through `network` with `credentials` (taken on their first use,
    /// §7.3; the connector drops them as soon as the login reports success or failure).
    ///
    /// # Errors
    ///
    /// Returns [`CompatError`] if the connection refuses to open.
    fn connect(
        &mut self,
        network: &NetworkAccess,
        credentials: Credentials,
    ) -> Result<(), CompatError>;
}

/// The connector of this build: always fails with [`CompatError::ProtocolNotBuilt`] and never
/// opens a socket (it drops the credentials at once).
#[derive(Debug, Default, Clone, Copy)]
pub struct PlaceholderConnector;

impl Connector for PlaceholderConnector {
    fn connect(
        &mut self,
        _network: &NetworkAccess,
        credentials: Credentials,
    ) -> Result<(), CompatError> {
        drop(credentials);
        Err(CompatError::ProtocolNotBuilt)
    }
}

/// Configuration for [`CompatBackend`]: the consent proof and the credentials (§4.3, §7.3).
/// No `Clone`, no `Default`; its `Debug` form prints neither.
pub struct CompatConfig {
    proof: ConsentProof,
    credentials: Credentials,
}

impl std::fmt::Debug for CompatConfig {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.write_str("CompatConfig { .. }")
    }
}

impl CompatConfig {
    /// The configuration of one login attempt.
    #[must_use]
    pub fn new(proof: ConsentProof, credentials: Credentials) -> Self {
        Self { proof, credentials }
    }

    /// The consent proof it carries.
    #[must_use]
    pub fn proof(&self) -> &ConsentProof {
        &self.proof
    }
}

/// Official-server compatibility backend (`docs/ROADMAP.md` M8).
#[derive(Debug)]
pub struct CompatBackend<C: Connector = PlaceholderConnector> {
    proof: ConsentProof,
    credentials: Option<Credentials>,
    connector: C,
    pending_events: Vec<Event>,
}

impl CompatBackend<PlaceholderConnector> {
    /// A backend over the [`PlaceholderConnector`].
    ///
    /// # Errors
    ///
    /// [`CompatError::ConsentMismatch`] when the proof is not for the shipped warning text.
    pub fn new(config: CompatConfig) -> Result<Self, CompatError> {
        Self::with_connector(config, PlaceholderConnector)
    }
}

impl<C: Connector> CompatBackend<C> {
    /// A backend over a custom [`Connector`] (tests and future live connectors).
    ///
    /// # Errors
    ///
    /// [`CompatError::ConsentMismatch`] when the proof is not for the shipped warning text.
    pub fn with_connector(config: CompatConfig, connector: C) -> Result<Self, CompatError> {
        let CompatConfig { proof, credentials } = config;
        if !proof.is_current() {
            return Err(CompatError::ConsentMismatch);
        }
        Ok(Self {
            proof,
            credentials: Some(credentials),
            connector,
            pending_events: Vec::new(),
        })
    }

    /// Whether the backend still holds credentials (they are dropped on their first use and
    /// when the session ends, §7.3).
    #[must_use]
    pub fn holds_credentials(&self) -> bool {
        self.credentials.is_some()
    }

    fn connect(&mut self) {
        let Some(credentials) = self.credentials.take() else {
            // One login attempt per backend (§8.6): a second Connect cannot reuse anything.
            self.pending_events.push(Event::Disconnected {
                reason: DisconnectReason::TransportFailed(
                    "official login already attempted".to_owned(),
                ),
            });
            return;
        };
        let result = NetworkAccess::new(&self.proof)
            .and_then(|network| self.connector.connect(&network, credentials));
        match result {
            Ok(()) => {}
            Err(CompatError::ProtocolNotBuilt) => {
                self.pending_events
                    .push(Event::Account(AccountEvent::LoginRefused {
                        reason: LoginRefusal::NotAvailable,
                        retry_after_seconds: None,
                        detail: None,
                    }));
                self.pending_events.push(Event::Disconnected {
                    reason: DisconnectReason::TransportFailed(
                        CompatError::ProtocolNotBuilt.to_string(),
                    ),
                });
            }
            Err(err) => self.pending_events.push(Event::Disconnected {
                reason: DisconnectReason::TransportFailed(err.to_string()),
            }),
        }
    }
}

impl<C: Connector> Backend for CompatBackend<C> {
    fn name(&self) -> &'static str {
        "compat"
    }

    fn as_any(&self) -> Option<&dyn std::any::Any> {
        None
    }

    fn submit(&mut self, _now: Instant, command: Command) -> Result<(), NetError> {
        match command {
            Command::Connect => self.connect(),
            Command::Disconnect => {
                self.credentials = None;
                self.pending_events.push(Event::Disconnected {
                    reason: DisconnectReason::ClientRequested,
                });
            }
            Command::Account(_) => {
                return Err(NetError::InvalidCommand(
                    "the official login is not available in this version of Tessera",
                ));
            }
            _ => {}
        }
        Ok(())
    }

    fn poll(&mut self, _now: Instant, out: &mut Vec<Event>) {
        if self
            .pending_events
            .iter()
            .any(|e| matches!(e, Event::Disconnected { .. }))
        {
            // The session ends: nothing of the login is kept.
            self.credentials = None;
        }
        out.append(&mut self.pending_events);
    }
}
