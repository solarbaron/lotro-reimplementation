// SPDX-License-Identifier: GPL-3.0-or-later

//! [`CompatBackend`]: the [`Backend`] shell for `tessera-compat`.
//!
//! This module implements [`Backend`] for [`CompatBackend`]. The underlying
//! login (`crate::login`), session (`crate::session`), and world
//! (`crate::world`) protocol codecs are implemented in this crate, while
//! [`PlaceholderConnector`] stands in until the end-to-end live client session
//! driver is wired up.

use std::time::Instant;

use tessera_net::{Backend, Command, DisconnectReason, Event, NetError};

use crate::CompatError;

/// Pluggable connection driver behind [`CompatBackend`].
pub trait Connector: Send + std::fmt::Debug {
    /// Open or advance the connection.
    ///
    /// # Errors
    ///
    /// Returns [`CompatError`] if the connection refuses to open.
    fn connect(&mut self) -> Result<(), CompatError>;
}

/// Default connector used while the end-to-end official-server client session
/// driver is still being wired up: always fails with
/// [`CompatError::ProtocolNotBuilt`] and never opens a socket.
#[derive(Debug, Default, Clone, Copy)]
pub struct PlaceholderConnector;

impl Connector for PlaceholderConnector {
    fn connect(&mut self) -> Result<(), CompatError> {
        Err(CompatError::ProtocolNotBuilt)
    }
}

/// Configuration for [`CompatBackend`].
#[derive(Debug, Default, Clone)]
pub struct CompatConfig;

impl CompatConfig {
    /// Create a [`CompatConfig`].
    #[must_use]
    pub fn new() -> Self {
        Self
    }
}

/// Official-server compatibility backend (`docs/ROADMAP.md` M8).
#[derive(Debug)]
pub struct CompatBackend<C: Connector = PlaceholderConnector> {
    config: CompatConfig,
    connector: C,
    pending_events: Vec<Event>,
}

impl CompatBackend<PlaceholderConnector> {
    /// Construct a [`CompatBackend`] with the default [`PlaceholderConnector`].
    #[must_use]
    pub fn new(config: CompatConfig) -> Self {
        Self::with_connector(config, PlaceholderConnector)
    }
}

impl<C: Connector> CompatBackend<C> {
    /// Construct a [`CompatBackend`] with a custom [`Connector`] (used by
    /// tests and future live session drivers).
    #[must_use]
    pub fn with_connector(config: CompatConfig, connector: C) -> Self {
        Self {
            config,
            connector,
            pending_events: Vec::new(),
        }
    }

    /// Borrow the backend configuration.
    #[must_use]
    pub fn config(&self) -> &CompatConfig {
        &self.config
    }

    /// Attempt to open the underlying connection via [`Connector::connect`].
    ///
    /// # Errors
    ///
    /// Propagates any [`CompatError`] returned by the connector.
    pub fn connect(&mut self) -> Result<(), CompatError> {
        self.connector.connect()
    }
}

impl<C: Connector> Backend for CompatBackend<C> {
    fn name(&self) -> &'static str {
        "compat"
    }

    fn submit(&mut self, _now: Instant, command: Command) -> Result<(), NetError> {
        if matches!(command, Command::Connect) {
            if let Err(err) = self.connector.connect() {
                self.pending_events.push(Event::Disconnected {
                    reason: DisconnectReason::TransportFailed(err.to_string()),
                });
            }
        } else if matches!(command, Command::Disconnect) {
            self.pending_events.push(Event::Disconnected {
                reason: DisconnectReason::ClientRequested,
            });
        }
        Ok(())
    }

    fn poll(&mut self, _now: Instant, out: &mut Vec<Event>) {
        out.append(&mut self.pending_events);
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn placeholder_connector_reports_protocol_not_built() {
        let mut backend = CompatBackend::new(CompatConfig::new());
        assert!(matches!(
            backend.connect(),
            Err(CompatError::ProtocolNotBuilt)
        ));
        let now = Instant::now();
        backend.submit(now, Command::Connect).unwrap();
        let mut events = Vec::new();
        backend.poll(now, &mut events);
        assert_eq!(
            events,
            vec![Event::Disconnected {
                reason: DisconnectReason::TransportFailed(
                    CompatError::ProtocolNotBuilt.to_string()
                ),
            }]
        );
    }
}
