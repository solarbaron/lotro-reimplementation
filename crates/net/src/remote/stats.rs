// SPDX-License-Identifier: GPL-3.0-or-later
//! Transport statistics and failure details of the test-server backend, for the client's
//! HUD and the load tools.

use std::time::Duration;

use tessera_protocol::codes::{AppErrorCode, ClientFailure, EnterWorldFailedCode, LoginFailedCode};
use tessera_protocol::fingerprint::Fingerprint;
use tessera_protocol::message::{HelloAck, ServerCaps};

/// What the server said about itself in `HelloAck` (`tessera-protocol.md` §6.2.2).
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ServerInfo {
    /// Negotiated minor protocol version.
    pub minor: u16,
    /// Capability bits.
    pub caps: ServerCaps,
    /// Simulation rate, Hz.
    pub tick_rate: u8,
    /// `MotionBatch` rate, Hz.
    pub snapshot_rate: u8,
    /// Interest radius in landblocks.
    pub interest_radius: u8,
    /// The operator's display name (sanitised by the decoder).
    pub name: String,
    /// Message of the day (may be empty).
    pub motd: String,
}

impl From<&HelloAck> for ServerInfo {
    fn from(a: &HelloAck) -> Self {
        Self {
            minor: a.minor,
            caps: a.server_caps,
            tick_rate: a.tick_rate,
            snapshot_rate: a.snapshot_rate,
            interest_radius: a.interest_radius,
            name: a.server_name.clone(),
            motd: a.motd.clone(),
        }
    }
}

/// A snapshot of the backend's transport and session statistics. The network thread
/// refreshes it about 30 times a second; [`RemoteBackend::stats`](super::RemoteBackend::stats)
/// returns the copy taken at the latest poll.
#[derive(Debug, Clone, Default, PartialEq)]
pub struct RemoteStats {
    /// The client state machine's state name (`Handshaking`, `InWorld`, ...).
    pub state: &'static str,
    /// The server's `HelloAck`, once received.
    pub server: Option<ServerInfo>,
    /// The server key's fingerprint, once the handshake verified it.
    pub fingerprint: Option<Fingerprint>,
    /// QUIC's smoothed path round-trip time.
    pub path_rtt: Option<Duration>,
    /// Latest application round-trip time measured by `Ping`/`Pong` (§6.2.18).
    pub ping_rtt: Option<Duration>,
    /// Clock offset in use (server − client), µs (§11.2), once a sample exists.
    pub clock_offset_us: Option<i64>,
    /// Smallest round-trip delay δ among the kept time samples, µs.
    pub clock_delay_us: Option<i64>,
    /// Time samples accepted.
    pub time_samples: u64,
    /// UDP bytes sent and received (QUIC packets, encrypted).
    pub udp_bytes_sent: u64,
    /// UDP bytes received.
    pub udp_bytes_received: u64,
    /// QUIC packets the path declared lost.
    pub lost_packets: u64,
    /// Current congestion window, bytes.
    pub congestion_window: u64,
    /// Application datagrams received.
    pub datagrams_received: u64,
    /// `MoveInput`s sent.
    pub moves_sent: u64,
    /// `MotionBatch`es received and handed on.
    pub motion_batches: u64,
    /// Movement items dropped because the main thread was not keeping up (§13.3).
    pub motion_dropped: u64,
    /// Messages the protocol session dropped as stale or unexpected (§13.2).
    pub protocol_dropped: u64,
    /// `CORRECTED` location reports received.
    pub corrections: u64,
    /// Decoded items waiting on the network thread for the main thread.
    pub pending: usize,
    /// Largest `pending` seen.
    pub pending_peak: usize,
    /// Times the network thread paused reading streams because the main thread lagged.
    pub read_pauses: u64,
}

/// Why the connection failed, in more detail than
/// [`DisconnectReason`](crate::DisconnectReason): for an interactive client that wants to
/// offer trust on first use, show a login error, or retry later.
#[derive(Debug, Clone, PartialEq, Eq)]
#[non_exhaustive]
pub enum RemoteFailure {
    /// A client-local connection failure (`tessera-protocol.md` §14.2). For
    /// `KEY_MISMATCH` and `KEY_UNKNOWN`, `presented` is the key the server showed.
    Connect {
        /// The failure.
        failure: ClientFailure,
        /// The server's fingerprint, when the certificate could be read.
        presented: Option<Fingerprint>,
    },
    /// `LoginFailed` (§14.3).
    Login {
        /// The code.
        code: LoginFailedCode,
        /// Seconds to wait before trying again (`RATE_LIMITED`).
        retry_after: u16,
    },
    /// `EnterWorldFailed` (§14.5).
    EnterWorld {
        /// The code.
        code: EnterWorldFailedCode,
    },
    /// The server ended the session (`Disconnect` or CONNECTION_CLOSE, §14.1).
    Server {
        /// The application code (unknown codes as received).
        code: AppErrorCode,
    },
    /// The server broke the protocol; the client closed with `PROTOCOL_VIOLATION`.
    Violation(String),
    /// A transport or I/O failure.
    Transport(String),
}
