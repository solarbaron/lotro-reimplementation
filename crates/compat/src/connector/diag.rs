// SPDX-License-Identifier: GPL-3.0-or-later
//! Diagnostics of the session handshake (`official-session.md` §3.2, §4.2–§4.4, §6) for the
//! owner's live verification (M8 exit criterion 4): counts and labels only, never an
//! address, a port, an account, a ticket or datagram bytes (decision 0044 item 3).
//!
//! # What is logged
//!
//! - At the start of each connection (login and world), one `info` line,
//!   `official session: handshake started`, with the labels of [`HandshakeReport`]: the
//!   §3.2 bind case ([`BindCase`]), where the local address came from ([`LocalSource`]),
//!   the §6.2.1 authentication mode ([`AuthMode`]) and the send checksum rule
//!   ([`SendChecksumRule`]).
//! - While a connection waits in its handshake (States `1` and `2`), every
//!   [`HANDSHAKE_REPORT_SEC`] one `debug` line, `official session: handshake waiting`, with
//!   the counts of [`HandshakeReport`].
//! - When a connection closes from its handshake (a `HandshakeTimeout`, a transport error
//!   or an unsupported codec before State `5`), one `info` line,
//!   `official session: handshake failed`, with the same counts, the reason and the state
//!   it closed from.
//!
//! # The send checksum switch
//!
//! [`SEND_CHECKSUM_ENV`] (`TESSERA_OFFICIAL_SEND_CHECKSUM`) is read once when the
//! connector's thread starts ([`send_checksum_from_env`]): `per-buffer` (the default, the
//! §4.4.2 sender rule) or `whole-payload` (the §4.4.2 receiver rule applied on send). An
//! unknown value is refused with a `warn` line and the default is used. The receive rule
//! never changes. This is a live-verification aid for the contested rule
//! (`docs/m8-remediation.md` §15, S §4.4.2), off by default (`docs/DEVELOPMENT.md`).
//! Engineering choices, spec silent.

use std::net::{Ipv4Addr, SocketAddrV4};

use crate::session::{
    AUTH_FLAG_HAS_EXTRA_BLOB, AUTH_FLAG_HAS_SECOND_STRING, AUTH_TYPE_ACCOUNT, AUTH_TYPE_TICKET,
    ConnectionState, HandshakeCounts, SendChecksumRule, SessionError, candidate_bind_ports,
    decode_datagram,
};

/// The environment variable of the send checksum switch (module docs).
pub const SEND_CHECKSUM_ENV: &str = "TESSERA_OFFICIAL_SEND_CHECKSUM";

/// Every how many seconds a connection still in its handshake logs its counts (`debug`).
pub const HANDSHAKE_REPORT_SEC: f64 = 5.0;

/// The send checksum rule for the variable's value `value` (`None`: unset). An unknown
/// value (or one that is not Unicode) is refused with a `warn` line, without the value,
/// and the default rule is used.
#[must_use]
pub fn send_checksum_rule(value: Option<&std::ffi::OsStr>) -> SendChecksumRule {
    let Some(raw) = value else {
        return SendChecksumRule::default();
    };
    match raw.to_str().and_then(SendChecksumRule::parse) {
        Some(rule) => rule,
        None => {
            tracing::warn!(
                variable = SEND_CHECKSUM_ENV,
                used = SendChecksumRule::default().label(),
                "official session: unknown send checksum rule refused (per-buffer or \
                 whole-payload); the default is used"
            );
            SendChecksumRule::default()
        }
    }
}

/// [`send_checksum_rule`] of the process environment's [`SEND_CHECKSUM_ENV`]; read once,
/// when the connector's thread starts.
#[must_use]
pub fn send_checksum_from_env() -> SendChecksumRule {
    send_checksum_rule(std::env::var_os(SEND_CHECKSUM_ENV).as_deref())
}

/// Which case of `official-session.md` §3.2 item 2 gave the session's local port, as a
/// label (never the port).
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum BindCase {
    /// Case A: the explicit base port plus `k`.
    ExplicitBase,
    /// Case B: the computed unique port.
    ComputedUnique,
    /// Case C: an ephemeral port on the chosen local address.
    Ephemeral,
    /// Case C's last fallback: an ephemeral port on any address (`0.0.0.0`).
    EphemeralAnyAddress,
}

impl BindCase {
    /// The case of a socket bound at `bound` for the local address `local_ip`, the explicit
    /// base port `explicit_base_port` and the `Net.ComputeUniquePort` value `compute_unique`
    /// (the inputs of [`candidate_bind_ports`]).
    #[must_use]
    pub fn of(
        local_ip: Ipv4Addr,
        explicit_base_port: u16,
        compute_unique: bool,
        bound: SocketAddrV4,
    ) -> Self {
        if *bound.ip() != local_ip {
            return Self::EphemeralAnyAddress;
        }
        let port = bound.port();
        let listed = port != 0
            && candidate_bind_ports(local_ip, explicit_base_port, compute_unique).contains(&port);
        if listed && explicit_base_port != 0 {
            Self::ExplicitBase
        } else if listed && compute_unique && !local_ip.is_unspecified() {
            Self::ComputedUnique
        } else {
            Self::Ephemeral
        }
    }

    /// The case's name in a log line.
    #[must_use]
    pub const fn label(self) -> &'static str {
        match self {
            Self::ExplicitBase => "Case A (explicit base port)",
            Self::ComputedUnique => "Case B (computed unique port)",
            Self::Ephemeral => "Case C (ephemeral port)",
            Self::EphemeralAnyAddress => "Case C (ephemeral port, any address)",
        }
    }
}

/// Where the session's local IPv4 address came from (`official-session.md` §3.2 item 1),
/// as a label (never the address).
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum LocalSource {
    /// The `localHost` segment of the two-colon `-h` form.
    HostSegment,
    /// The address the system routes towards the server.
    Routed,
    /// None: `0.0.0.0`.
    Unspecified,
}

impl LocalSource {
    /// The source's name in a log line.
    #[must_use]
    pub const fn label(self) -> &'static str {
        match self {
            Self::HostSegment => "host local segment",
            Self::Routed => "routed toward the server",
            Self::Unspecified => "unspecified",
        }
    }
}

/// The authentication of a connection's first datagram (`official-session.md` §6.2.1
/// selection rule, §6.5), as a label.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum AuthMode {
    /// `SessionAuthHeader`, `authType 0x40000002`, `authFlags 0x04` (`extraBlob` ticket).
    Ticket,
    /// `SessionAuthHeader`, `authType 0x00000001`, `authFlags 0x02` (ticket in `secondStr`).
    TicketInSecondString,
    /// `SessionAuthHeader`, `authType 0x00000001`, `authFlags 0x00` (account only).
    AccountOnly,
    /// Another `(authType, authFlags)` pair (never sent: the header refuses it).
    Other,
    /// `ReferralAuthHeader` with the referral cookie (§6.5).
    Referral,
}

impl AuthMode {
    /// The mode of a `SessionAuthHeader` with `auth_type` and `auth_flags`.
    #[must_use]
    pub const fn of(auth_type: u32, auth_flags: u32) -> Self {
        match (auth_type, auth_flags) {
            (AUTH_TYPE_TICKET, AUTH_FLAG_HAS_EXTRA_BLOB) => Self::Ticket,
            (AUTH_TYPE_ACCOUNT, AUTH_FLAG_HAS_SECOND_STRING) => Self::TicketInSecondString,
            (AUTH_TYPE_ACCOUNT, 0) => Self::AccountOnly,
            _ => Self::Other,
        }
    }

    /// The mode's name in a log line.
    #[must_use]
    pub const fn label(self) -> &'static str {
        match self {
            Self::Ticket => "authType 0x40000002 authFlags 0x04 (ticket in extraBlob)",
            Self::TicketInSecondString => {
                "authType 0x00000001 authFlags 0x02 (ticket in secondStr)"
            }
            Self::AccountOnly => "authType 0x00000001 authFlags 0x00 (account only)",
            Self::Other => "other",
            Self::Referral => "ReferralAuthHeader (referral cookie)",
        }
    }
}

/// Why a received datagram was refused, by group.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum RejectKind {
    /// The checksum did not verify (§4.4.2, §10.1 rule 6).
    Checksum,
    /// The datagram header, the wire prefix or an optional header did not parse (§4.1–§4.3).
    Header,
    /// No connection takes it: an unknown recipient ID or a source address that does not
    /// match (§3.4, §10.1 rule 4).
    Routing,
    /// A length is out of bounds (§3.5).
    Size,
    /// Anything else (sequence window, state, fragments, messages).
    Other,
}

impl RejectKind {
    /// The group of a session-layer refusal.
    #[must_use]
    pub fn of(e: &SessionError) -> Self {
        use SessionError as E;
        match e {
            E::ChecksumMismatch { .. } | E::MissingChecksumMask => Self::Checksum,
            E::DatagramTooShort { .. }
            | E::DatagramTooLarge { .. }
            | E::InvalidPakSize { .. }
            | E::TooManyFragments { .. }
            | E::InvalidFragmentDataLen { .. }
            | E::BlobTooLarge { .. } => Self::Size,
            E::SourceAddressMismatch { .. }
            | E::UnknownRecipient { .. }
            | E::RecipientIdCollision { .. } => Self::Routing,
            E::MissingWireCodec { .. }
            | E::WireDecompressionFailed
            | E::InvalidHeaderFlags { .. }
            | E::OptionalHeaderOverrun { .. }
            | E::TrailingPayloadBytes { .. }
            | E::InvalidSequenceNakCount { .. }
            | E::InvalidOutOfOrderAckCount { .. }
            | E::InvalidSessionAuthVersion
            | E::InvalidAuthBlock
            | E::UnsupportedAuthMode { .. }
            | E::AuthCredentialsMismatch { .. } => Self::Header,
            _ => Self::Other,
        }
    }
}

/// Refused datagrams by [`RejectKind`].
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub struct RejectCounts {
    /// [`RejectKind::Checksum`].
    pub checksum: u64,
    /// [`RejectKind::Header`].
    pub header: u64,
    /// [`RejectKind::Routing`].
    pub routing: u64,
    /// [`RejectKind::Size`].
    pub size: u64,
    /// [`RejectKind::Other`].
    pub other: u64,
}

impl RejectCounts {
    /// Counts one refusal of `kind`.
    pub fn add(&mut self, kind: RejectKind) {
        let n = match kind {
            RejectKind::Checksum => &mut self.checksum,
            RejectKind::Header => &mut self.header,
            RejectKind::Routing => &mut self.routing,
            RejectKind::Size => &mut self.size,
            RejectKind::Other => &mut self.other,
        };
        *n = n.saturating_add(1);
    }

    /// Every refusal.
    #[must_use]
    pub fn total(&self) -> u64 {
        self.checksum
            .saturating_add(self.header)
            .saturating_add(self.routing)
            .saturating_add(self.size)
            .saturating_add(self.other)
    }
}

/// Whether a refused datagram still carries a `ConnectChallengeHeader` (read without the
/// checksum check and without a codec; a diagnostic only, nothing acts on it).
#[must_use]
pub fn carries_challenge(datagram: &[u8]) -> bool {
    decode_datagram(datagram, true, None, false)
        .is_ok_and(|p| p.opt_headers.connect_challenge.is_some())
}

/// What one connection's driver side counted (the connection's own counts are
/// [`HandshakeCounts`]).
#[derive(Debug, Clone, Copy, PartialEq, Default)]
pub struct LinkDiag {
    /// When the connection started.
    pub started: f64,
    /// When the next `debug` report is due.
    pub next_report: f64,
    /// Datagrams received from the server's address and handed to the connection.
    pub received: u64,
    /// Those refused, by group.
    pub rejected: RejectCounts,
    /// Refused datagrams that carried a `ConnectChallengeHeader` ([`carries_challenge`]).
    pub challenge_in_rejected: u64,
}

impl LinkDiag {
    /// A connection started at `now`.
    #[must_use]
    pub fn new(now: f64) -> Self {
        Self {
            started: now,
            next_report: now + HANDSHAKE_REPORT_SEC,
            ..Self::default()
        }
    }
}

/// One connection's handshake diagnostics: counts and labels only.
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct HandshakeReport {
    /// The world connection (otherwise the login connection).
    pub world: bool,
    /// The connection's state, or the state it closed from.
    pub state: Option<ConnectionState>,
    /// Seconds since the connection started.
    pub elapsed_sec: f64,
    /// The connection's own counts (sent by kind, challenges, accepted).
    pub counts: HandshakeCounts,
    /// The driver's counts of this connection.
    pub link: LinkDiag,
    /// Datagrams from an address no connection has, or naming a connection's ID from
    /// another address (the whole session).
    pub other_address: u64,
    /// The §3.2 bind case.
    pub bind: BindCase,
    /// Where the local address came from.
    pub local: LocalSource,
    /// The authentication of the connection's first datagram.
    pub auth: AuthMode,
    /// The send checksum rule.
    pub send_checksum: SendChecksumRule,
}

impl HandshakeReport {
    /// Whether any `ConnectChallengeHeader` arrived (acted on, or in a refused datagram).
    #[must_use]
    pub fn challenge_seen(&self) -> bool {
        self.counts.challenges > 0 || self.link.challenge_in_rejected > 0
    }

    /// Logs the start line (`info`, module docs).
    pub fn log_start(&self) {
        tracing::info!(
            world = self.world,
            bind = self.bind.label(),
            local_address = self.local.label(),
            auth = self.auth.label(),
            send_checksum = self.send_checksum.label(),
            "official session: handshake started"
        );
    }

    /// Logs the waiting line (`debug`, module docs).
    pub fn log_waiting(&self) {
        tracing::debug!(
            world = self.world,
            state = ?self.state,
            elapsed_s = self.elapsed_sec,
            auth_sent = self.counts.session_auth.saturating_add(self.counts.referral_auth),
            auth_retransmits = self.counts.auth_retransmits,
            accept_sent = self.counts.connect_accept,
            accept_retransmits = self.counts.connect_accept_retransmits,
            sent = self.counts.sent,
            send_failures = self.counts.send_failures,
            received = self.link.received,
            accepted = self.counts.accepted,
            rejected_checksum = self.link.rejected.checksum,
            rejected_header = self.link.rejected.header,
            rejected_routing = self.link.rejected.routing,
            rejected_size = self.link.rejected.size,
            rejected_other = self.link.rejected.other,
            challenge_seen = self.challenge_seen(),
            challenges = self.counts.challenges,
            challenge_in_rejected = self.link.challenge_in_rejected,
            other_address = self.other_address,
            bind = self.bind.label(),
            auth = self.auth.label(),
            send_checksum = self.send_checksum.label(),
            "official session: handshake waiting"
        );
    }

    /// Logs the failure line (`info`, module docs) for a close with `reason`.
    pub fn log_failed(&self, reason: &crate::session::DisconnectReason) {
        tracing::info!(
            world = self.world,
            ?reason,
            state = ?self.state,
            elapsed_s = self.elapsed_sec,
            auth_sent = self.counts.session_auth.saturating_add(self.counts.referral_auth),
            auth_retransmits = self.counts.auth_retransmits,
            accept_sent = self.counts.connect_accept,
            accept_retransmits = self.counts.connect_accept_retransmits,
            sent = self.counts.sent,
            send_failures = self.counts.send_failures,
            received = self.link.received,
            accepted = self.counts.accepted,
            rejected_checksum = self.link.rejected.checksum,
            rejected_header = self.link.rejected.header,
            rejected_routing = self.link.rejected.routing,
            rejected_size = self.link.rejected.size,
            rejected_other = self.link.rejected.other,
            challenge_seen = self.challenge_seen(),
            challenges = self.counts.challenges,
            challenge_in_rejected = self.link.challenge_in_rejected,
            other_address = self.other_address,
            bind = self.bind.label(),
            local_address = self.local.label(),
            auth = self.auth.label(),
            send_checksum = self.send_checksum.label(),
            "official session: handshake failed"
        );
    }
}
