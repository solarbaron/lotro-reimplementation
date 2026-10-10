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
//!    without a `Disconnect`);
//! 3. else the idle timeout (no end);
//! 4. else the transport's description of the end (no end).

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
            Self::Local => f.write_str("closed by server"),
            Self::IdleTimeout => f.write_str("idle timeout"),
            Self::Other(text) => f.write_str(text),
        }
    }
}

/// The name of an application code: the protocol's name for a defined value, else
/// `"code N"`.
#[must_use]
pub fn code_name(code: u64) -> String {
    match u16::try_from(code) {
        Ok(v) => AppErrorCode::from_u16(v).to_string(),
        Err(_) => format!("code {code}"),
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
    /// counts (quinn ignores later ones).
    pub fn closed_by_server(&mut self, code: AppErrorCode, reason: &str) {
        self.local_close
            .get_or_insert_with(|| (code, reason.to_owned()));
    }

    /// The cause, once the connection ended as `end` (§11.1 order).
    #[must_use]
    pub fn finish(&self, end: &QuicEnd) -> Cause {
        if let Some((code, text, from)) = &self.disconnect {
            let close = match (end, &self.local_close) {
                (QuicEnd::Local, Some((c, _))) => format!("{c} from server"),
                (other, _) => other.to_string(),
            };
            return Cause {
                code: Some(code.to_string()),
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
                    code: Some(code.to_string()),
                    reason: reason.clone(),
                    end: Some(End::Server),
                    close: None,
                },
                // Closed locally without an application code (for example the endpoint
                // closing): the transport's description.
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
        assert_eq!(
            t.finish(&peer_close(0x1_0000_0000, "")).code.as_deref(),
            Some("code 4294967296")
        );
        assert_eq!(
            t.finish(&peer_close(0x40, "")).code.as_deref(),
            Some("code 64")
        );
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
