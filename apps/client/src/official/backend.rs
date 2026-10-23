// SPDX-License-Identifier: GPL-3.0-or-later
//! The client's one construction site of the compatibility backend
//! (`docs/specs/official-entry.md` §6.2).
//!
//! [`construct_backend`] is the only function of `apps/` and `crates/` (outside
//! `crates/compat`) that names `CompatBackend`, and this file the only one that names a
//! `Connector` implementation; a source-scan test (OE-T8 (c)) enforces it. It is reached
//! only from §9 step 5 ([`super::flow::Flow`] after Log in), with a configuration that
//! carries the proof made at step 2 or 3 and the user's credentials.

use tessera_compat::{
    Availability, CompatBackend, CompatConfig, CompatError, Connector, PlaceholderConnector,
};
use tessera_net::Backend;

use super::flow::BackendFactory;

/// Constructs the compatibility backend over `connector` for one login attempt.
///
/// # Errors
///
/// [`CompatError::ConsentMismatch`] when the proof is not for the shipped warning text.
pub fn construct_backend<C: Connector + 'static>(
    config: CompatConfig,
    connector: C,
) -> Result<Box<dyn Backend>, CompatError> {
    Ok(Box::new(CompatBackend::with_connector(config, connector)?))
}

/// The backends of this build: the availability answer of `tessera-compat` and the
/// placeholder connector (§4.4).
#[derive(Debug, Default)]
pub struct ProductionBackends;

impl BackendFactory for ProductionBackends {
    fn availability(&self) -> Availability {
        tessera_compat::availability()
    }

    fn construct(&mut self, config: CompatConfig) -> Result<Box<dyn Backend>, CompatError> {
        construct_backend(config, PlaceholderConnector)
    }
}
