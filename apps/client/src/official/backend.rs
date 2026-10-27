// SPDX-License-Identifier: GPL-3.0-or-later
//! The client's one construction site of the compatibility backend
//! (`docs/specs/official-entry.md` §6.2).
//!
//! [`construct_backend`] is the only function of `apps/` and `crates/` (outside
//! `crates/compat`) that names `CompatBackend`, and this file the only one that names a
//! `Connector` implementation; a source-scan test (OE-T8 (c)) enforces it. It is reached
//! only from §9 step 5 ([`super::flow::Flow`] after Log in), with a configuration that
//! carries the proof made at step 2 or 3 and the user's credentials.

use std::path::PathBuf;

use tessera_compat::{Availability, CompatBackend, CompatConfig, CompatError, Connector};
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

/// The backends of this build: the availability answer of `tessera-compat` (§4.4;
/// `Available`, the live connector is built in) and the live connector over the user's
/// install (`--game-dir`, else `TESSERA_GAME_DIR`, else discovery; found on the connector's
/// thread). The flow asks for availability only at §9 step 4, after the record check and the
/// warning, and reaches [`BackendFactory::construct`] only from the account form's Log in
/// (§9 step 5): nothing here opens a network before then.
///
/// # Tests never reach the live service
///
/// No test may drive Log in through these backends: with `TESSERA_GAME_DIR` exported the
/// live connector would find the install and contact the official service, and tests and CI
/// never connect to it (decision 0034). Two guards hold that:
///
/// - In the client's unit tests (`#[cfg(test)]`), [`BackendFactory::construct`] refuses with
///   [`CompatError::ProtocolNotBuilt`] before a connector exists; the flow shows its
///   `official.error.other` message and opens no network. A test may therefore build these
///   backends to check the screens up to the account form, and even press Log in, safely.
/// - The integration binaries in `apps/client/tests/` link this library built without
///   `cfg(test)`, so the refusal above does not reach them. They are guarded by a source
///   scan instead (`official::tests::no_test_reaches_the_live_connector`): no file there
///   names these backends, the live connector or its network factory, any `official` module
///   but `install_data`, or the `--official` and `--start-screen` options of a spawned
///   client. A scan was chosen over an environment guard that those binaries would set,
///   because a guard holds only in binaries that remember to set it, while the scan covers
///   every file added there later.
#[derive(Debug, Default)]
pub struct ProductionBackends {
    game_dir: Option<PathBuf>,
}

impl ProductionBackends {
    /// The backends for the install named by `--game-dir` (or found otherwise).
    #[must_use]
    pub fn new(game_dir: Option<PathBuf>) -> Self {
        Self { game_dir }
    }
}

impl BackendFactory for ProductionBackends {
    fn availability(&self) -> Availability {
        tessera_compat::availability()
    }

    #[cfg(test)]
    fn construct(&mut self, config: CompatConfig) -> Result<Box<dyn Backend>, CompatError> {
        // Unit tests never reach the live connector (see the type's documentation).
        let _ = (&self.game_dir, config);
        tracing::warn!("official login: the live backends refuse to construct in tests");
        Err(CompatError::ProtocolNotBuilt)
    }

    #[cfg(not(test))]
    fn construct(&mut self, config: CompatConfig) -> Result<Box<dyn Backend>, CompatError> {
        use tessera_compat::connector::LocalPaths;
        use tessera_compat::{LiveConfig, LiveConnector};

        let live = LiveConfig {
            // The install root and the original's settings directory (inside a Wine,
            // Proton or CrossOver prefix for an install there) come from the locator on
            // the connector's thread. No network version string is supplied: Tessera has
            // no setting or option for it yet (`official-login.md` §2.4.1 step 1), so the
            // client's version resource is read.
            paths: LocalPaths::default(),
            loader: super::install_data::loader(),
            locate: Some(super::install_data::locator(self.game_dir.clone())),
        };
        construct_backend(config, LiveConnector::new(live))
    }
}
