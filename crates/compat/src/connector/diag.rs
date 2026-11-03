// SPDX-License-Identifier: GPL-3.0-or-later
//! Diagnostics of the session handshake (`official-session.md` §3.2, §4.2–§4.4, §6) for the
//! owner's live verification (M8 exit criterion 4): counts and labels only, never an
//! address, a port, an account, a ticket or datagram bytes (decision 0044 item 3). The one,
//! documented exception is the opt-in datagram dump of [`super::dump`] (off by default):
//! its files hold Tessera's own first datagram of each connection, the credentials replaced,
//! the server's address and port and the local address and port used; its log lines name
//! none of them.
//!
//! # What is logged
//!
//! - At the start of each connection (login and world), one `info` line,
//!   `official session: handshake started`, with the labels of [`HandshakeReport`]: the
//!   §3.2 bind case ([`BindCase`]), where the local address came from ([`LocalSource`]),
//!   the form of the remote address ([`RemoteForm`], [`RemoteName`]), the §6.2.1
//!   authentication mode ([`AuthMode`]) and the `netVersion` string in full (a version label, not a secret; `none` on a world
//!   connection, whose `ReferralAuthHeader` carries none).
//! - While a connection waits in its handshake (States `1` and `2`), every
//!   [`HANDSHAKE_REPORT_SEC`] one `debug` line, `official session: handshake waiting`, with
//!   the counts of [`HandshakeReport`].
//! - When a connection closes from its handshake (a `HandshakeTimeout`, a transport error
//!   or an unsupported codec before State `5`), one `info` line,
//!   `official session: handshake failed`, with the same counts, the reason and the state
//!   it closed from. A cancel or a revoked network right during the handshake logs the same
//!   line once, with `reason="cancelled"` or `reason="network right revoked"`.
//! - While the login connection is past its handshake and before its character list
//!   (`official-session.md` §9.3.1–§9.3.7), one `info` line,
//!   `official session: initial exchange`, at each step ([`HandshakeReport::log_initial`]),
//!   with the stages `dd_seen`, `ec_sent`, `f5_received`, `f5_sent` and `list_received` and
//!   the counts of the messages skipped; never an identifier, an iteration value, a key or a
//!   path. A login that ends before the list logs
//!   `official session: login ended before the character list` with the same fields.
//! - What refused a message or datagram of the login (`official-session.md` §9.3.7: a diagnostic
//!   with no service data) is logged as a kind ([`error_kind`]): an error's text can hold values of
//!   the server's message.
//! - The chat-server connection (`official-messages-play.md` §6.6, the same session layer)
//!   logs `official chat session: handshake failed` (`info`, [`log_chat_failed`]) when it
//!   closes from its handshake, with the connection's own counts.
//!
//! # The checksum rule
//!
//! Every datagram is sent and verified with the one per-buffer `payloadSum` of
//! `official-session.md` §4.4.2 ([`crate::session::payload_checksum`]); the earlier
//! whole-payload rule and its live-verification switch are withdrawn by the spec.
//!
//! # The login server switch
//!
//! `official-login.md` §3.3 and its invariant I3 send the session to the first non-empty
//! entry of the world status's `<loginservers>`. [`LOGIN_SERVER_ENV`]
//! (`TESSERA_OFFICIAL_LOGIN_SERVER`), read once when the connector's thread starts
//! ([`login_server_from_env`]), is a deliberate departure from that rule, for live
//! verification only and off by default: a 1-based index into
//! the non-empty entries ([`login_server_entry`]), so only addresses of the service's own
//! status answer are ever used. Unset, entry 1 is used (the spec rule, unchanged). A
//! value that is not a decimal in `1..=count` (another form, surrounding spaces, `0`, more
//! than the entries listed) is refused with a `warn` line that does not repeat it, and entry
//! 1 is used. The `official login: world status read` line says which entry is used
//! (`login_server_entry="2 of 4 (override)"` or `"1 of 4"`), never an address. Whether the
//! original client ever uses an entry after the first is a question for the spec writer
//! (`docs/m8-remediation.md` §15, L §3.3); the value forms and the refusal are engineering
//! choices.

use std::ffi::{OsStr, OsString};
use std::net::Ipv4Addr;

use crate::initial::InitialStages;
use crate::session::{
    AUTH_FLAG_HAS_EXTRA_BLOB, AUTH_FLAG_HAS_SECOND_STRING, AUTH_TYPE_CHAT, AUTH_TYPE_PRIMARY,
    BindPorts, ConnectionState, HandshakeCounts, SessionError, decode_datagram,
};

/// Every how many seconds a connection still in its handshake logs its counts (`debug`).
pub const HANDSHAKE_REPORT_SEC: f64 = 5.0;

/// The environment variable of the login server switch (module docs).
pub const LOGIN_SERVER_ENV: &str = "TESSERA_OFFICIAL_LOGIN_SERVER";

/// Which entry of the world status's `<loginservers>` the session uses (`official-login.md`
/// §3.3; module docs "The login server switch").
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct LoginServerEntry {
    /// The entry, 0-based.
    pub index: usize,
    /// The non-empty entries listed.
    pub count: usize,
    /// Chosen by [`LOGIN_SERVER_ENV`] (otherwise the spec's first entry).
    pub overridden: bool,
}

impl Default for LoginServerEntry {
    /// The first of one entry.
    fn default() -> Self {
        Self {
            index: 0,
            count: 1,
            overridden: false,
        }
    }
}

/// The kind of `error` for a log line: the name of its variant, with no field. The text and the
/// fields of an error from a received message can hold its values (a property ID, a flags byte, a
/// count).
#[must_use]
pub fn error_kind(error: &impl std::fmt::Debug) -> String {
    let form = format!("{error:?}");
    let end = form
        .find(|c: char| !c.is_ascii_alphanumeric() && c != '_')
        .unwrap_or(form.len());
    form.get(..end).unwrap_or_default().to_owned()
}

impl LoginServerEntry {
    /// The entry in a log line, 1-based: `"2 of 4 (override)"` or `"1 of 4"`.
    #[must_use]
    pub fn label(&self) -> String {
        let n = self.index.saturating_add(1);
        if self.overridden {
            format!("{n} of {} (override)", self.count)
        } else {
            format!("{n} of {}", self.count)
        }
    }
}

/// The entry of `count` non-empty `<loginservers>` entries that the variable's value `value`
/// (`None`: unset) chooses: exactly ASCII decimal digits naming `1..=count`. Any other
/// value is refused with a `warn` line, without the value, and entry 1 is used.
#[must_use]
pub fn login_server_entry(value: Option<&OsStr>, count: usize) -> LoginServerEntry {
    let first = LoginServerEntry {
        index: 0,
        count,
        overridden: false,
    };
    let Some(raw) = value else {
        return first;
    };
    let chosen = raw
        .to_str()
        .filter(|s| !s.is_empty() && s.bytes().all(|b| b.is_ascii_digit()))
        .and_then(|s| s.parse::<usize>().ok())
        .filter(|n| (1..=count).contains(n));
    match chosen {
        Some(n) => LoginServerEntry {
            index: n - 1,
            count,
            overridden: true,
        },
        None => {
            tracing::warn!(
                variable = LOGIN_SERVER_ENV,
                login_servers = count,
                used = 1,
                "official login: login server index refused (a decimal from 1 to the number \
                 of login servers); the first is used"
            );
            first
        }
    }
}

/// The raw value of [`LOGIN_SERVER_ENV`] in the process environment (`None`: unset); read
/// once, when the connector's thread starts, and judged against each world status's entries
/// ([`login_server_entry`]).
#[must_use]
pub fn login_server_from_env() -> Option<OsString> {
    std::env::var_os(LOGIN_SERVER_ENV)
}

/// Which case of `official-session.md` §3.2 item 3 chose the session's local port, as a
/// label (never the port). It is the case of the bind plan: since the bind has no fallback,
/// a socket that is bound was bound by its case.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum BindCase {
    /// Case A: the explicit port (`--outport` or `Net.UserSpecifiedPort`), one bind.
    ExplicitPort,
    /// Case B: the computed unique port, seeded from the first local IPv4 address.
    ComputedUnique,
    /// Case C: an ephemeral port (unique-port computation off, or no local IPv4 address).
    Ephemeral,
}

impl BindCase {
    /// The case of the port plan `ports`.
    #[must_use]
    pub const fn of(ports: &BindPorts) -> Self {
        match ports {
            BindPorts::Explicit(_) => Self::ExplicitPort,
            BindPorts::Unique { .. } => Self::ComputedUnique,
            BindPorts::Ephemeral => Self::Ephemeral,
        }
    }

    /// The case's name in a log line.
    #[must_use]
    pub const fn label(self) -> &'static str {
        match self {
            Self::ExplicitPort => "Case A (explicit port)",
            Self::ComputedUnique => "Case B (computed unique port, seed from the routed address)",
            Self::Ephemeral => "Case C (ephemeral port)",
        }
    }
}

/// Which local IPv4 address the session's socket binds (`official-session.md` §3.2 item 2),
/// as a label (never the address).
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum LocalSource {
    /// The `localHost` segment of the two-colon `-h` form.
    HostSegment,
    /// `Net.BindInterface` of `UserPreferences.ini`.
    BindInterface,
    /// Neither: the wildcard `0.0.0.0`.
    Wildcard,
}

impl LocalSource {
    /// The source's name in a log line.
    #[must_use]
    pub const fn label(self) -> &'static str {
        match self {
            Self::HostSegment => "host local segment",
            Self::BindInterface => "Net.BindInterface",
            Self::Wildcard => "wildcard (0.0.0.0)",
        }
    }
}

/// The form of the address a connection sends to (`official-session.md` §3.3; the `-h`
/// forms of `official-login.md` §4.2), as a label (never the host, address or port).
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum RemoteForm {
    /// `remoteHost:port`: the port inside `-h`.
    HostPort,
    /// `remoteHost` alone, the port from `-p`.
    HostWithPortOption,
    /// `localHost:remoteHost:port`.
    LocalHostPort,
    /// The world connection: the `WorldReferralHeader`'s address with an IP other than the
    /// login server's (§6.5).
    ReferralOwnIp,
    /// The world connection: the referral's IP is the login server's (the header gave
    /// `0.0.0.0`, or the same IP, §6.5).
    ReferralLoginIp,
}

impl RemoteForm {
    /// The form of the login server's `-h` value: whether it has the `localHost` segment
    /// (`two_colon`) and whether it carries the port (`port_in_host`).
    #[must_use]
    pub const fn of_host(two_colon: bool, port_in_host: bool) -> Self {
        if two_colon {
            Self::LocalHostPort
        } else if port_in_host {
            Self::HostPort
        } else {
            Self::HostWithPortOption
        }
    }

    /// The form's name in a log line.
    #[must_use]
    pub const fn label(self) -> &'static str {
        match self {
            Self::HostPort => "remoteHost:port (port in -h)",
            Self::HostWithPortOption => "remoteHost with -p port",
            Self::LocalHostPort => "localHost:remoteHost:port",
            Self::ReferralOwnIp => "referral address (IP from the header)",
            Self::ReferralLoginIp => "referral address (the login server's IP)",
        }
    }
}

/// How the remote host of a connection became an address, as a label.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum RemoteName {
    /// The host was a literal IPv4 address.
    LiteralIpv4,
    /// The host was a name; the resolver's first IPv4 address is used.
    Resolved,
    /// The address came from a referral header.
    FromReferral,
}

impl RemoteName {
    /// The name form of the host text `host`.
    #[must_use]
    pub fn of_host(host: &str) -> Self {
        if host.parse::<Ipv4Addr>().is_ok() {
            Self::LiteralIpv4
        } else {
            Self::Resolved
        }
    }

    /// The form's name in a log line.
    #[must_use]
    pub const fn label(self) -> &'static str {
        match self {
            Self::LiteralIpv4 => "literal IPv4",
            Self::Resolved => "name resolved (first IPv4 address)",
            Self::FromReferral => "from the referral header",
        }
    }
}

/// The authentication of a connection's first datagram (`official-session.md` §6.2.1
/// selection rule, §6.5), as a label.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum AuthMode {
    /// `SessionAuthHeader`, `authType 0x00000001`, `authFlags 0x04` (primary connection,
    /// ticket text in `extraBlob`).
    Ticket,
    /// `SessionAuthHeader`, `authType 0x00000001`, `authFlags 0x06` (primary connection,
    /// ticket and password).
    TicketAndPassword,
    /// `SessionAuthHeader`, `authType 0x40000002`, `authFlags 0x04` (chat connection,
    /// ticket text in `extraBlob`).
    ChatTicket,
    /// `SessionAuthHeader`, `authType 0x00000001` or `0x40000002`, `authFlags 0x00`
    /// (account only).
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
        const BOTH: u32 = AUTH_FLAG_HAS_EXTRA_BLOB | AUTH_FLAG_HAS_SECOND_STRING;
        match (auth_type, auth_flags) {
            (AUTH_TYPE_PRIMARY, AUTH_FLAG_HAS_EXTRA_BLOB) => Self::Ticket,
            (AUTH_TYPE_PRIMARY, BOTH) => Self::TicketAndPassword,
            (AUTH_TYPE_CHAT, AUTH_FLAG_HAS_EXTRA_BLOB) => Self::ChatTicket,
            (AUTH_TYPE_PRIMARY | AUTH_TYPE_CHAT, 0) => Self::AccountOnly,
            _ => Self::Other,
        }
    }

    /// The mode's name in a log line.
    #[must_use]
    pub const fn label(self) -> &'static str {
        match self {
            Self::Ticket => "authType 0x00000001 authFlags 0x04 (ticket text in extraBlob)",
            Self::TicketAndPassword => {
                "authType 0x00000001 authFlags 0x06 (ticket text and password)"
            }
            Self::ChatTicket => {
                "authType 0x40000002 authFlags 0x04 (chat, ticket text in extraBlob)"
            }
            Self::AccountOnly => "authFlags 0x00 (account only)",
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
    /// A `ConnectChallengeHeader` named a `codecUuid` that no loaded table has (§6.4 step
    /// 3, §7.3).
    Codec,
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
            E::UnsupportedCodecUuid { .. } => Self::Codec,
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
    /// [`RejectKind::Codec`].
    pub codec: u64,
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
            RejectKind::Codec => &mut self.codec,
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
            .saturating_add(self.codec)
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
    /// Refused datagrams that carried a `ConnectChallengeHeader` ([`carries_challenge`])
    /// the connection did not act on (one it acted on and then refused, an unsupported
    /// codec for example, is in [`HandshakeCounts::challenges`] only).
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
    /// Those of [`Self::other_address`] that carried a `ConnectChallengeHeader` while a
    /// connection was in its handshake ([`carries_challenge`]; the whole session): the
    /// server may answer from another address or port.
    pub challenge_from_other_address: u64,
    /// Socket reads that failed (the whole session; an ICMP port-unreachable can show up as
    /// one on some systems).
    pub recv_errors: u64,
    /// The §3.2 port case.
    pub bind: BindCase,
    /// Which local address the socket binds.
    pub local: LocalSource,
    /// The form of the remote address.
    pub remote: RemoteForm,
    /// How the remote host became an address.
    pub remote_name: RemoteName,
    /// The authentication of the connection's first datagram.
    pub auth: AuthMode,
    /// How far the initial exchange got (the login connection's; the world connection has
    /// none, its stages all stay clear).
    pub initial: InitialStages,
}

impl HandshakeReport {
    /// Whether any `ConnectChallengeHeader` arrived from the connection's server address
    /// (acted on, or in a refused datagram); one from another address is
    /// [`Self::challenge_from_other_address`].
    #[must_use]
    pub fn challenge_seen(&self) -> bool {
        self.counts.challenges > 0 || self.link.challenge_in_rejected > 0
    }

    /// Logs the start line (`info`, module docs) with the connection's `netVersion`
    /// (`None`: a `ReferralAuthHeader`, which carries none).
    pub fn log_start(&self, net_version: Option<&str>) {
        tracing::info!(
            world = self.world,
            bind = self.bind.label(),
            local_address = self.local.label(),
            remote = self.remote.label(),
            remote_name = self.remote_name.label(),
            auth = self.auth.label(),
            net_version = net_version.unwrap_or("none"),
            "official session: handshake started"
        );
    }

    /// Logs the initial-exchange line (`info`, module docs).
    pub fn log_initial(&self, what: &str) {
        tracing::info!(
            step = what,
            dd_seen = self.initial.dd_seen,
            ec_sent = self.initial.ec_sent,
            f5_received = self.initial.f5_received,
            f5_sent = self.initial.f5_sent,
            list_received = self.initial.list_received,
            skipped_149 = self.initial.skipped.op_149,
            skipped_aa = self.initial.skipped.op_aa,
            skipped_30000040 = self.initial.skipped.op_30000040,
            skipped_43 = self.initial.skipped.op_43,
            skipped_fe = self.initial.skipped.op_fe,
            skipped_54 = self.initial.skipped.op_54,
            skipped_other = self.initial.skipped.other,
            ignored = self.initial.ignored,
            refused = self.counts.refused_while_connected,
            "official session: initial exchange"
        );
    }

    /// Logs the end of a login before its character list (`info`, module docs).
    pub fn log_login_ended(&self, reason: &str) {
        tracing::info!(
            reason = %reason,
            state = ?self.state,
            elapsed_s = self.elapsed_sec,
            dd_seen = self.initial.dd_seen,
            ec_sent = self.initial.ec_sent,
            f5_received = self.initial.f5_received,
            f5_sent = self.initial.f5_sent,
            list_received = self.initial.list_received,
            skipped_149 = self.initial.skipped.op_149,
            skipped_aa = self.initial.skipped.op_aa,
            skipped_30000040 = self.initial.skipped.op_30000040,
            skipped_43 = self.initial.skipped.op_43,
            skipped_fe = self.initial.skipped.op_fe,
            skipped_54 = self.initial.skipped.op_54,
            skipped_other = self.initial.skipped.other,
            ignored = self.initial.ignored,
            refused = self.counts.refused_while_connected,
            sent = self.counts.sent,
            received = self.link.received,
            received_ok = self.counts.received_ok,
            "official session: login ended before the character list"
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
            received_ok = self.counts.received_ok,
            rejected_checksum = self.link.rejected.checksum,
            rejected_header = self.link.rejected.header,
            rejected_routing = self.link.rejected.routing,
            rejected_size = self.link.rejected.size,
            rejected_codec = self.link.rejected.codec,
            rejected_other = self.link.rejected.other,
            challenge_seen = self.challenge_seen(),
            challenges = self.counts.challenges,
            challenge_in_rejected = self.link.challenge_in_rejected,
            other_address = self.other_address,
            challenge_from_other_address = self.challenge_from_other_address,
            recv_errors = self.recv_errors,
            bind = self.bind.label(),
            remote = self.remote.label(),
            auth = self.auth.label(),
            "official session: handshake waiting"
        );
    }

    /// Logs the failure line (`info`, module docs) for a close with `reason`.
    pub fn log_failed(&self, reason: &crate::session::DisconnectReason) {
        self.log_ended(reason.label());
    }

    /// Logs the failure line (`info`, module docs) for an end the session layer did not
    /// report: a cancel (`"cancelled"`) or a revoked network right.
    pub fn log_ended(&self, reason: &str) {
        tracing::info!(
            world = self.world,
            reason = %reason,
            state = ?self.state,
            elapsed_s = self.elapsed_sec,
            auth_sent = self.counts.session_auth.saturating_add(self.counts.referral_auth),
            auth_retransmits = self.counts.auth_retransmits,
            accept_sent = self.counts.connect_accept,
            accept_retransmits = self.counts.connect_accept_retransmits,
            sent = self.counts.sent,
            send_failures = self.counts.send_failures,
            received = self.link.received,
            received_ok = self.counts.received_ok,
            rejected_checksum = self.link.rejected.checksum,
            rejected_header = self.link.rejected.header,
            rejected_routing = self.link.rejected.routing,
            rejected_size = self.link.rejected.size,
            rejected_codec = self.link.rejected.codec,
            rejected_other = self.link.rejected.other,
            challenge_seen = self.challenge_seen(),
            challenges = self.counts.challenges,
            challenge_in_rejected = self.link.challenge_in_rejected,
            other_address = self.other_address,
            challenge_from_other_address = self.challenge_from_other_address,
            recv_errors = self.recv_errors,
            bind = self.bind.label(),
            local_address = self.local.label(),
            remote = self.remote.label(),
            remote_name = self.remote_name.label(),
            auth = self.auth.label(),
            "official session: handshake failed"
        );
    }
}

/// Logs the chat-server connection's failure line (`info`, module docs): it closed from
/// `state` (State `1` or `2`) with `reason`; `counts` are its own, `refused` and
/// `recv_errors` the chat client's (cumulative over its attempts). No endpoint is named.
pub fn log_chat_failed(
    reason: &crate::session::DisconnectReason,
    state: ConnectionState,
    counts: &HandshakeCounts,
    refused: u64,
    recv_errors: u64,
) {
    tracing::info!(
        connection = "chat",
        ?reason,
        ?state,
        auth_sent = counts.session_auth,
        auth_retransmits = counts.auth_retransmits,
        accept_sent = counts.connect_accept,
        accept_retransmits = counts.connect_accept_retransmits,
        sent = counts.sent,
        send_failures = counts.send_failures,
        received_ok = counts.received_ok,
        challenges = counts.challenges,
        refused,
        recv_errors,
        "official chat session: handshake failed"
    );
}
