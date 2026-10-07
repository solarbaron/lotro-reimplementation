// SPDX-License-Identifier: GPL-3.0-or-later
//! Configuration of the test-server backend: where to connect, which server key to trust,
//! and the account to log in with (`tessera-protocol.md` §4.4–§4.7).

use std::fmt;
use std::path::PathBuf;

use tessera_protocol::client::ClientConfig;
use tessera_protocol::connect::ConnectString;
use tessera_protocol::fingerprint::Fingerprint;
use tessera_protocol::limits::{MAX_PASSWORD_LEN, MIN_MAX_ENTITIES};
use tessera_protocol::text::{is_valid_account, is_valid_password};

use crate::NetError;
use crate::interp::InterpConfig;
use crate::sandbox::world::StaticConfig;

/// Capacity of the channel that carries decoded network input from the network thread to
/// the main thread (ARCH §19.4). When it is full the network thread stops reading every
/// reliable stream, the control stream included, once [`PENDING_PAUSE`] items wait (QUIC
/// flow control then pushes back on the server), and drops movement items, so no reliable
/// state is lost (`tessera-protocol.md` §13.3).
pub const EVENT_CHANNEL_CAPACITY: usize = 1024;
/// Capacity of the command channel from the main thread to the network thread. A full
/// channel makes [`Backend::submit`](crate::Backend::submit) fail with
/// [`NetError::SendQueueFull`]; it never blocks.
pub const COMMAND_CHANNEL_CAPACITY: usize = 256;
/// The network thread stops reading reliable streams while this many decoded items wait
/// for the main thread (on top of a full [`EVENT_CHANNEL_CAPACITY`]). One read that starts
/// below the limit is decoded in full, so up to `PENDING_PAUSE + READ_CHUNK / 6` items
/// (about 1,620; 6 bytes is the smallest world-stream frame) can wait at the peak.
pub const PENDING_PAUSE: usize = 256;
/// Bytes read from one stream per read call: bounds the items one read can decode.
pub const READ_CHUNK: usize = 8 * 1024;
/// Most bytes queued for the control stream before the connection is given up (the
/// server is not reading).
pub const MAX_CONTROL_BACKLOG: usize = 256 * 1024;

/// A password supplied by the caller. It is never logged: `Debug` prints
/// `<redacted>`, and its bytes are overwritten (best effort) when it is dropped.
///
/// The wiping covers this wrapper and the backend's own copy of the credentials (wiped
/// when they are dropped unused). The copies made for the wire, the `Login`/`Register`
/// message and its encoded frame inside the protocol and QUIC layers, are freed without
/// being overwritten.
#[derive(Clone, PartialEq, Eq)]
pub struct Password(String);

impl Password {
    /// Wraps a password, exactly as the user typed it (no normalisation in protocol v1,
    /// `tessera-protocol.md` §4.7).
    #[must_use]
    pub fn new(password: impl Into<String>) -> Self {
        Self(password.into())
    }

    /// The password text. Callers must not log it.
    #[must_use]
    pub fn expose(&self) -> &str {
        &self.0
    }
}

impl fmt::Debug for Password {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.write_str("Password(<redacted>)")
    }
}

impl Drop for Password {
    fn drop(&mut self) {
        let mut bytes = std::mem::take(&mut self.0).into_bytes();
        bytes.fill(0);
        std::hint::black_box(&bytes);
    }
}

/// What to do with a server key that is neither pinned nor in the known-servers file
/// (`tessera-protocol.md` §4.5).
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub enum FirstUse {
    /// Refuse it: the connection fails with `KEY_UNKNOWN`, and
    /// [`RemoteFailure::Connect`](super::RemoteFailure::Connect) carries the presented
    /// fingerprint so an interactive client can show it, ask the user and reconnect with
    /// a pin (or record the key with [`record_known_server`](super::record_known_server)).
    /// The default, and the only correct choice for a non-interactive client unless it
    /// was explicitly told otherwise.
    #[default]
    Refuse,
    /// Accept it and append it to the known-servers file (when one is configured). Only
    /// for an explicit user decision or an explicitly configured tool: trust on first use
    /// cannot detect an attacker present on that first connection.
    Accept,
}

/// Which server key the client trusts (`tessera-protocol.md` §4.5).
#[derive(Debug, Clone, PartialEq, Eq, Default)]
pub struct Trust {
    /// The known-servers file (`known_servers` in Tessera's own settings directory, never
    /// in the game install). `None`: no file, so only a pin in the connect string (or
    /// [`FirstUse::Accept`]) lets a connection through.
    pub known_servers: Option<PathBuf>,
    /// What to do with an unknown key.
    pub first_use: FirstUse,
}

/// How to reach and log in to a Tessera test server. Build it with
/// [`RemoteConfig::new`] and adjust the public fields.
#[derive(Clone)]
#[non_exhaustive]
pub struct RemoteConfig {
    /// `tessera://<host>:<port>[?pin=sha256:…]`. A pin in it is authoritative (§4.5).
    pub server: ConnectString,
    /// Known-servers file and first-use policy, used when the connect string has no pin.
    pub trust: Trust,
    /// The account name (`ACCOUNT`: 1–32 of `A–Z a–z 0–9 . _ -`).
    pub account: String,
    /// The password, sent only inside the encrypted connection after the server key was
    /// verified.
    pub password: Password,
    /// Send `Register` instead of `Login` (the server must advertise `REGISTRATION`).
    pub register: bool,
    /// The `Hello` parameters: entity limit (≥ 64) and the software name for the
    /// server's logs.
    pub client: ClientConfig,
    /// Streaming of statics from the client's own data around the player (only with a
    /// [`WorldSource`](crate::sandbox::WorldSource), as in the sandbox).
    pub statics: StaticConfig,
    /// Interpolation parameters for remote entities. Once the server's rates are known the
    /// delay is raised to [`recommended_delay`](crate::interp::recommended_delay) of them,
    /// and the history with it ([`InterpConfig::for_server_rates`]).
    pub interp: InterpConfig,
}

impl fmt::Debug for RemoteConfig {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.debug_struct("RemoteConfig")
            .field("server", &self.server.to_string())
            .field("trust", &self.trust)
            .field("account", &self.account)
            .field("password", &self.password)
            .field("register", &self.register)
            .field("client", &self.client)
            .field("statics", &self.statics)
            .field("interp", &self.interp)
            .finish()
    }
}

impl RemoteConfig {
    /// A configuration for `server` with the given account, the default trust (no
    /// known-servers file, unknown keys refused), and default parameters.
    ///
    /// # Errors
    ///
    /// [`NetError::InvalidConfig`] when the connect string does not parse.
    pub fn new(
        server: &str,
        account: impl Into<String>,
        password: Password,
    ) -> Result<Self, NetError> {
        let server = ConnectString::parse(server)
            .map_err(|_| NetError::InvalidConfig("connect string is not valid"))?;
        Ok(Self {
            server,
            trust: Trust::default(),
            account: account.into(),
            password,
            register: false,
            client: ClientConfig {
                client_name: format!("tessera-net {}", env!("CARGO_PKG_VERSION")),
                ..ClientConfig::default()
            },
            statics: StaticConfig::default(),
            interp: InterpConfig::default(),
        })
    }

    /// The same configuration trusting exactly `fingerprint` (as if the connect string
    /// carried it as `?pin=`).
    #[must_use]
    pub fn pinned(mut self, fingerprint: Fingerprint) -> Self {
        self.server.pin = Some(fingerprint);
        self
    }

    /// The same configuration with a known-servers file and first-use policy.
    #[must_use]
    pub fn with_trust(mut self, trust: Trust) -> Self {
        self.trust = trust;
        self
    }

    /// Checks every field against the protocol's rules, so nothing invalid is ever sent.
    ///
    /// # Errors
    ///
    /// [`NetError::InvalidConfig`] naming the bad field.
    pub fn validate(&self) -> Result<(), NetError> {
        if !is_valid_account(&self.account) {
            return Err(NetError::InvalidConfig(
                "account name must be 1-32 of A-Z a-z 0-9 . _ -",
            ));
        }
        if !is_valid_password(self.password.expose())
            || self.password.expose().len() > MAX_PASSWORD_LEN
        {
            return Err(NetError::InvalidConfig("password must be 1-256 bytes"));
        }
        if self.client.max_entities < MIN_MAX_ENTITIES {
            return Err(NetError::InvalidConfig("max_entities below 64"));
        }
        if self.client.client_name.len() > tessera_protocol::limits::MAX_SOFTWARE_NAME_LEN
            || tessera_protocol::text::has_control(&self.client.client_name)
        {
            return Err(NetError::InvalidConfig(
                "client name above 64 bytes or with control characters",
            ));
        }
        self.statics.validate()?;
        self.interp.validate()?;
        Ok(())
    }
}
