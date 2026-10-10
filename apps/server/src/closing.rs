// SPDX-License-Identifier: GPL-3.0-or-later
//! The cause of a connection's end, for the "connection closed" line (`test-server.md`
//! §11.1, revised 2026-10-10).
//!
//! Under the closing procedure of `tessera-protocol.md` §12.3 the `CONNECTION_CLOSE` that
//! ends a connection usually carries `NO_ERROR` and not the cause, so the connection task
//! records what happened on the way and [`CauseTracker::finish`] picks the cause in the
//! order of §11.1:
//!
//! 1. the first `Disconnect` sent or received on the connection (its code and text, from
//!    the end that sent it), never replaced by a later `CONNECTION_CLOSE`;
//! 2. else the application code and reason of the `CONNECTION_CLOSE` that ended the
//!    connection (the server's own when it closed at once, the peer's when the peer closed
//!    without a `Disconnect`); a close of the whole endpoint with an application code (the
//!    `SERVER_SHUTDOWN` of §12.1 step 6) counts as the server's own close with that code on
//!    every connection it ends, although quinn reports those only as closed locally
//!    ("Graceful shutdown" of §11.1, sixth revision);
//! 3. else the idle timeout (no end);
//! 4. else the transport's description of the end (no end).
//!
//! The fields are written as §11.1 "Fields of the cause" says: `code` (a §14.1 name, the
//! decimal number alone for a code §14.1 does not define (seventh revision), or `-`),
//! `reason`, `end` (`server`, `peer`, `none`) and, only when the cause is a `Disconnect`,
//! `close` (`<code> from <end>`, the code written as in `code`). The connection task hands
//! `reason` and `close` to `tracing` as string values, which the `text` format writes in
//! Rust's string debug form, a permitted case of the §11.1 "Quoted form" (§14 invariant 12),
//! and the `json` format as JSON strings.

use std::fmt;

use tessera_protocol::codes::AppErrorCode;

/// Which end a cause came from.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum End {
    /// This server.
    Server,
    /// The connected client.
    Peer,
}

impl End {
    /// `"server"` or `"peer"`.
    #[must_use]
    pub fn name(self) -> &'static str {
        match self {
            Self::Server => "server",
            Self::Peer => "peer",
        }
    }
}

/// How the QUIC connection ended, as far as the cause is concerned.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum QuicEnd {
    /// The peer closed with an application `CONNECTION_CLOSE` (raw code, reason).
    PeerApplication {
        /// The application code as sent (a QUIC varint).
        code: u64,
        /// The reason, decoded lossily as UTF-8.
        reason: String,
    },
    /// This end closed the connection.
    Local,
    /// The QUIC idle timeout passed.
    IdleTimeout,
    /// Anything else (a reset, a transport error, a local failure), with the transport's
    /// description.
    Other(String),
}

impl QuicEnd {
    /// The end of a quinn connection.
    #[must_use]
    pub fn of(e: &quinn::ConnectionError) -> Self {
        match e {
            quinn::ConnectionError::ApplicationClosed(c) => Self::PeerApplication {
                code: c.error_code.into_inner(),
                reason: String::from_utf8_lossy(&c.reason).into_owned(),
            },
            quinn::ConnectionError::LocallyClosed => Self::Local,
            quinn::ConnectionError::TimedOut => Self::IdleTimeout,
            other => Self::Other(other.to_string()),
        }
    }
}

impl fmt::Display for QuicEnd {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::PeerApplication { code, .. } => write!(f, "{} from peer", code_name(*code)),
            // Closed locally with no application code the server recorded: the
            // transport's description (§11.1 rule 4).
            Self::Local => write!(f, "{}", quinn::ConnectionError::LocallyClosed),
            Self::IdleTimeout => f.write_str("idle timeout"),
            Self::Other(text) => f.write_str(text),
        }
    }
}

/// The name of an application code as received (a QUIC varint): the protocol's name for a
/// value §14.1 defines, else its decimal number alone (`"65536"`; §11.1 seventh revision:
/// no prefix and no space, so the plain `code` field stays one token).
#[must_use]
pub fn code_name(code: u64) -> String {
    match u16::try_from(code) {
        Ok(v) => app_code_name(AppErrorCode::from_u16(v)),
        Err(_) => code.to_string(),
    }
}

/// The name of a decoded application code (a `Disconnect` code or one the server chose):
/// its §14.1 name, or its decimal number alone when §14.1 does not define it (§11.1).
#[must_use]
pub fn app_code_name(code: AppErrorCode) -> String {
    match code {
        AppErrorCode::Other(v) => v.to_string(),
        named => named.name().to_owned(),
    }
}

/// The cause of the end of one connection (the "code and reason" field of §11.1).
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Cause {
    /// The code's name; `None` for the idle timeout and the transport's description.
    pub code: Option<String>,
    /// The text of the `Disconnect`, the reason of the `CONNECTION_CLOSE`, `"idle timeout"`
    /// or the transport's description.
    pub reason: String,
    /// The end the cause came from; `None` for the idle timeout and the transport's
    /// description.
    pub end: Option<End>,
    /// When the cause is a `Disconnect`: the `CONNECTION_CLOSE` that then ended the
    /// connection (the separate field §11.1 allows), so a peer that did not acknowledge
    /// shows. `None` otherwise.
    pub close: Option<String>,
}

impl Cause {
    /// The code for the log line (`"-"` when there is none).
    #[must_use]
    pub fn code_text(&self) -> &str {
        self.code.as_deref().unwrap_or("-")
    }

    /// The end for the log line (`"none"` when there is none).
    #[must_use]
    pub fn end_text(&self) -> &'static str {
        self.end.map_or("none", End::name)
    }
}

/// What the connection task saw, recorded as it happens.
#[derive(Debug, Default)]
pub struct CauseTracker {
    /// The first `Disconnect` sent or received: code, text, the end that sent it.
    disconnect: Option<(AppErrorCode, String, End)>,
    /// The application code and reason of the `CONNECTION_CLOSE` this server sent.
    local_close: Option<(AppErrorCode, String)>,
}

impl CauseTracker {
    /// No events yet.
    #[must_use]
    pub fn new() -> Self {
        Self::default()
    }

    /// Whether a `Disconnect` was already recorded (later ones do not count).
    #[must_use]
    pub fn has_disconnect(&self) -> bool {
        self.disconnect.is_some()
    }

    /// The server handed `Disconnect(code, text)` to the control stream.
    pub fn disconnect_sent(&mut self, code: AppErrorCode, text: &str) {
        self.disconnect
            .get_or_insert_with(|| (code, text.to_owned(), End::Server));
    }

    /// The server received and acted on the peer's `Disconnect(code, text)`.
    pub fn disconnect_received(&mut self, code: AppErrorCode, text: &str) {
        self.disconnect
            .get_or_insert_with(|| (code, text.to_owned(), End::Peer));
    }

    /// The server closed the QUIC connection with `code` and `reason`. The first close
    /// counts (quinn ignores later ones); the caller records only a close that took effect,
    /// that is one made while the connection was still open.
    pub fn closed_by_server(&mut self, code: AppErrorCode, reason: &str) {
        self.local_close
            .get_or_insert_with(|| (code, reason.to_owned()));
    }

    /// The server closed its whole QUIC endpoint with `code` and `reason` (§12.1 step 6),
    /// and this connection ended closed locally: it counts as the server's own close with
    /// that code and reason (§11.1 "Graceful shutdown"), unless a close of the connection
    /// itself took effect first (that one counts, as quinn ignores later closes).
    pub fn closed_with_endpoint(&mut self, code: AppErrorCode, reason: &str) {
        self.closed_by_server(code, reason);
    }

    /// The cause, once the connection ended as `end` (§11.1 order).
    #[must_use]
    pub fn finish(&self, end: &QuicEnd) -> Cause {
        if let Some((code, text, from)) = &self.disconnect {
            let close = match (end, &self.local_close) {
                (QuicEnd::Local, Some((c, _))) => format!("{} from server", app_code_name(*c)),
                (other, _) => other.to_string(),
            };
            return Cause {
                code: Some(app_code_name(*code)),
                reason: text.clone(),
                end: Some(*from),
                close: Some(close),
            };
        }
        match end {
            QuicEnd::PeerApplication { code, reason } => Cause {
                code: Some(code_name(*code)),
                reason: reason.clone(),
                end: Some(End::Peer),
                close: None,
            },
            QuicEnd::Local => match &self.local_close {
                Some((code, reason)) => Cause {
                    code: Some(app_code_name(*code)),
                    reason: reason.clone(),
                    end: Some(End::Server),
                    close: None,
                },
                // Closed locally without an application code the server chose: the
                // transport's description (rule 4). The endpoint close at shutdown is not
                // this case: it is recorded by `closed_with_endpoint`.
                None => Cause {
                    code: None,
                    reason: quinn::ConnectionError::LocallyClosed.to_string(),
                    end: None,
                    close: None,
                },
            },
            QuicEnd::IdleTimeout => Cause {
                code: None,
                reason: "idle timeout".into(),
                end: None,
                close: None,
            },
            QuicEnd::Other(text) => Cause {
                code: None,
                reason: text.clone(),
                end: None,
                close: None,
            },
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn peer_close(code: u64, reason: &str) -> QuicEnd {
        QuicEnd::PeerApplication {
            code,
            reason: reason.into(),
        }
    }

    /// A server `Disconnect` acknowledged by the peer's `NO_ERROR` close (§12.3 step 2):
    /// the server's code, from the server.
    #[test]
    fn server_disconnect_acknowledged_by_the_peer() {
        let mut t = CauseTracker::new();
        t.disconnect_sent(AppErrorCode::Kicked, "operator");
        let c = t.finish(&peer_close(0, ""));
        assert_eq!(c.code.as_deref(), Some("KICKED"));
        assert_eq!(c.reason, "operator");
        assert_eq!(c.end, Some(End::Server));
        assert_eq!(c.close.as_deref(), Some("NO_ERROR from peer"));
    }

    /// A server `Disconnect` whose linger passed (step 3): still the server's code; the
    /// close the server sent afterwards is the separate field only.
    #[test]
    fn server_disconnect_after_the_linger() {
        let mut t = CauseTracker::new();
        t.disconnect_sent(AppErrorCode::ServerFull, "");
        t.closed_by_server(AppErrorCode::ServerFull, "SERVER_FULL");
        let c = t.finish(&QuicEnd::Local);
        assert_eq!(c.code.as_deref(), Some("SERVER_FULL"));
        assert_eq!(c.end, Some(End::Server));
        assert_eq!(c.close.as_deref(), Some("SERVER_FULL from server"));
    }

    /// The peer's `Disconnect` answered by the server's `NO_ERROR` close (step 2): the
    /// peer's code and text, from the peer.
    #[test]
    fn peer_disconnect_closed_by_the_server() {
        let mut t = CauseTracker::new();
        t.disconnect_received(AppErrorCode::NoError, "bye");
        t.closed_by_server(AppErrorCode::NoError, "NO_ERROR");
        let c = t.finish(&QuicEnd::Local);
        assert_eq!(c.code.as_deref(), Some("NO_ERROR"));
        assert_eq!(c.reason, "bye");
        assert_eq!(c.end, Some(End::Peer));
    }

    /// When both ends sent a `Disconnect`, the first one the server sent or received
    /// counts.
    #[test]
    fn first_disconnect_wins() {
        let mut t = CauseTracker::new();
        t.disconnect_sent(AppErrorCode::ServerShutdown, "");
        t.disconnect_received(AppErrorCode::NoError, "bye");
        assert!(t.has_disconnect());
        let c = t.finish(&peer_close(0, ""));
        assert_eq!(c.code.as_deref(), Some("SERVER_SHUTDOWN"));
        assert_eq!(c.end, Some(End::Server));

        let mut t = CauseTracker::new();
        t.disconnect_received(AppErrorCode::NoError, "bye");
        t.disconnect_sent(AppErrorCode::ServerShutdown, "");
        let c = t.finish(&QuicEnd::Local);
        assert_eq!(c.code.as_deref(), Some("NO_ERROR"));
        assert_eq!(c.end, Some(End::Peer));
    }

    /// An immediate close by the server (step 4, a protocol violation): its own code and
    /// reason, from the server.
    #[test]
    fn immediate_server_close() {
        let mut t = CauseTracker::new();
        t.closed_by_server(AppErrorCode::ProtocolViolation, "bad frame");
        t.closed_by_server(AppErrorCode::InternalError, "later");
        let c = t.finish(&QuicEnd::Local);
        assert_eq!(c.code.as_deref(), Some("PROTOCOL_VIOLATION"));
        assert_eq!(c.reason, "bad frame");
        assert_eq!(c.end, Some(End::Server));
        assert_eq!(c.close, None);
    }

    /// The peer closed without a `Disconnect`: its code and reason, from the peer; codes
    /// beyond the protocol's are named by number.
    #[test]
    fn peer_close_without_disconnect() {
        let t = CauseTracker::new();
        let c = t.finish(&peer_close(7, "gone"));
        assert_eq!(c.code.as_deref(), Some("KICKED"));
        assert_eq!(c.reason, "gone");
        assert_eq!(c.end, Some(End::Peer));
        // §11.1 (seventh revision): an undefined code is its decimal number alone.
        assert_eq!(
            t.finish(&peer_close(0x1_0000_0000, "")).code.as_deref(),
            Some("4294967296")
        );
        assert_eq!(t.finish(&peer_close(0x40, "")).code.as_deref(), Some("64"));
        assert_eq!(
            t.finish(&peer_close(0x1_0000, "")).code.as_deref(),
            Some("65536")
        );
    }

    /// §11.1 (seventh revision): an undefined code inside `close` and as a `Disconnect`
    /// code is the decimal number alone too; every §14.1 name begins with a letter, so the
    /// two forms cannot be confused, and neither holds a space.
    #[test]
    fn undefined_codes_are_bare_decimal_numbers() {
        let mut t = CauseTracker::new();
        t.disconnect_sent(AppErrorCode::Kicked, "");
        let c = t.finish(&peer_close(0x1_0000, ""));
        assert_eq!(c.close.as_deref(), Some("65536 from peer"));
        assert_eq!(
            t.finish(&peer_close(0x1_0000_0000, "")).close.as_deref(),
            Some("4294967296 from peer")
        );

        let mut t = CauseTracker::new();
        t.disconnect_received(AppErrorCode::Other(0x40), "odd");
        let c = t.finish(&peer_close(0, ""));
        assert_eq!(c.code_text(), "64");
        assert_eq!(c.close.as_deref(), Some("NO_ERROR from peer"));

        let mut t = CauseTracker::new();
        t.disconnect_received(AppErrorCode::NoError, "");
        t.closed_by_server(AppErrorCode::Other(0x41), "");
        let c = t.finish(&QuicEnd::Local);
        assert_eq!(c.close.as_deref(), Some("65 from server"));

        for v in 0..=u16::MAX {
            let name = code_name(u64::from(v));
            assert!(!name.contains(' '), "{v}: {name}");
            let defined = !matches!(AppErrorCode::from_u16(v), AppErrorCode::Other(_));
            assert_eq!(
                name.starts_with(|c: char| c.is_ascii_alphabetic()),
                defined,
                "{v}: {name}"
            );
            if !defined {
                assert_eq!(name, v.to_string());
            }
        }
    }

    /// §14 invariant 12 and the §11.1 "Quoted form": the string debug form the `text`
    /// format writes `reason` and `close` in escapes exactly what the table requires,
    /// leaves printable ASCII alone, and writes any other character either as itself or as
    /// `\u{h}` (lowercase, no leading zeros). Checked for every Unicode scalar value, so a
    /// toolchain whose debug form changed would fail here.
    #[test]
    fn debug_form_meets_the_quoted_form() {
        for c in (0u32..=0x10_FFFF).filter_map(char::from_u32) {
            let got = format!("{:?}", c.to_string());
            let inner = &got[1..got.len() - 1];
            let hex = format!("\\u{{{:x}}}", u32::from(c));
            match c {
                '"' => assert_eq!(inner, "\\\""),
                '\\' => assert_eq!(inner, "\\\\"),
                '\n' => assert_eq!(inner, "\\n"),
                '\r' => assert_eq!(inner, "\\r"),
                '\t' => assert_eq!(inner, "\\t"),
                '\0' => assert_eq!(inner, "\\0"),
                '\u{1}'..='\u{1f}' | '\u{7f}'..='\u{9f}' => assert_eq!(inner, hex, "{c:?}"),
                ' '..='~' => assert_eq!(inner, c.to_string(), "{c:?}"),
                _ => assert!(inner == c.to_string() || inner == hex, "{c:?}: {inner}"),
            }
        }
    }

    /// The idle timeout and the transport's description carry no code and no end.
    #[test]
    fn idle_timeout_and_transport() {
        let mut t = CauseTracker::new();
        let c = t.finish(&QuicEnd::IdleTimeout);
        assert_eq!((c.code_text(), c.end_text()), ("-", "none"));
        assert_eq!(c.reason, "idle timeout");
        let c = t.finish(&QuicEnd::Other("reset by peer".into()));
        assert_eq!((c.code_text(), c.end_text()), ("-", "none"));
        assert_eq!(c.reason, "reset by peer");
        // A local close without an application code (the endpoint closing).
        let c = t.finish(&QuicEnd::Local);
        assert_eq!(c.end, None);
        assert!(!c.reason.is_empty());
        // A server close recorded later turns it into rule 2.
        t.closed_by_server(AppErrorCode::InternalError, "INTERNAL_ERROR");
        assert_eq!(t.finish(&QuicEnd::Local).end, Some(End::Server));
    }

    /// §14 invariant 11: a connection ended by the endpoint close of §12.1 step 6 without
    /// a `Disconnect` gives `SERVER_SHUTDOWN` from the server (rule 2), never the
    /// transport's local-close text.
    #[test]
    fn endpoint_close_without_disconnect() {
        let mut t = CauseTracker::new();
        t.closed_with_endpoint(AppErrorCode::ServerShutdown, "SERVER_SHUTDOWN");
        let c = t.finish(&QuicEnd::Local);
        assert_eq!(c.code_text(), "SERVER_SHUTDOWN");
        assert_eq!(c.reason, "SERVER_SHUTDOWN");
        assert_eq!(c.end_text(), "server");
        assert_eq!(c.close, None);
    }

    /// The endpoint close after the server's `Disconnect(SERVER_SHUTDOWN)` (§12.1 step 5)
    /// that the peer did not acknowledge: the cause is the `Disconnect`, and `close` reads
    /// `SERVER_SHUTDOWN from server`.
    #[test]
    fn endpoint_close_after_a_disconnect() {
        let mut t = CauseTracker::new();
        t.disconnect_sent(AppErrorCode::ServerShutdown, "");
        t.closed_with_endpoint(AppErrorCode::ServerShutdown, "SERVER_SHUTDOWN");
        let c = t.finish(&QuicEnd::Local);
        assert_eq!(c.code_text(), "SERVER_SHUTDOWN");
        assert_eq!(c.reason, "");
        assert_eq!(c.end_text(), "server");
        assert_eq!(c.close.as_deref(), Some("SERVER_SHUTDOWN from server"));
    }

    /// A close of the connection that took effect before the endpoint close keeps its own
    /// code (quinn ignores the later close).
    #[test]
    fn own_close_before_the_endpoint_close_counts() {
        let mut t = CauseTracker::new();
        t.closed_by_server(AppErrorCode::ProtocolViolation, "PROTOCOL_VIOLATION");
        t.closed_with_endpoint(AppErrorCode::ServerShutdown, "SERVER_SHUTDOWN");
        let c = t.finish(&QuicEnd::Local);
        assert_eq!(c.code_text(), "PROTOCOL_VIOLATION");
        assert_eq!(c.end_text(), "server");
    }

    /// The fallback immediate close when no `Disconnect` could be queued: the code the
    /// server meant to send, from the server, no `close`.
    #[test]
    fn fallback_close_without_a_queued_disconnect() {
        let mut t = CauseTracker::new();
        t.closed_by_server(AppErrorCode::SlowConsumer, "SLOW_CONSUMER");
        let c = t.finish(&QuicEnd::Local);
        assert_eq!(c.code_text(), "SLOW_CONSUMER");
        assert_eq!(c.reason, "SLOW_CONSUMER");
        assert_eq!(c.end_text(), "server");
        assert_eq!(c.close, None);
    }

    /// A `Disconnect` whose connection then ended closed locally with no recorded close:
    /// `close` is the transport's description, as for any other unexplained end.
    #[test]
    fn disconnect_then_unexplained_local_close() {
        let mut t = CauseTracker::new();
        t.disconnect_sent(AppErrorCode::Kicked, "");
        let c = t.finish(&QuicEnd::Local);
        assert_eq!(
            c.close,
            Some(quinn::ConnectionError::LocallyClosed.to_string())
        );
        let c = t.finish(&QuicEnd::IdleTimeout);
        assert_eq!(c.close.as_deref(), Some("idle timeout"));
    }

    #[test]
    fn quinn_ends_map() {
        assert_eq!(
            QuicEnd::of(&quinn::ConnectionError::TimedOut),
            QuicEnd::IdleTimeout
        );
        assert_eq!(
            QuicEnd::of(&quinn::ConnectionError::LocallyClosed),
            QuicEnd::Local
        );
        assert!(matches!(
            QuicEnd::of(&quinn::ConnectionError::Reset),
            QuicEnd::Other(_)
        ));
    }
}
