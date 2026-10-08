// SPDX-License-Identifier: GPL-3.0-or-later
//! Types shared by the sans-IO client and server connection state machines (§12).
//!
//! The state machines do no I/O. The network layer feeds them received bytes or decoded
//! messages and the current time (monotonic microseconds, any epoch), and drains
//! [`Transmit`]s from them: messages to send, streams to open, the control stream to finish,
//! and the connection to close.

use std::collections::VecDeque;

use crate::codes::AppErrorCode;
use crate::error::ProtocolError;
use crate::frame::StreamKind;
use crate::limits::{MAX_CLOSE_REASON_LEN, MIN_DATAGRAM_PAYLOAD, MIN_MAX_DATAGRAM_FRAME_SIZE};
use crate::message::{Message, MessageType};

/// Something the network layer must do for a state machine.
#[derive(Debug, Clone, PartialEq)]
pub enum Transmit {
    /// Send this message on its channel ([`Message::channel`]): a frame on the control,
    /// world or chat stream, or a datagram.
    Message(Message),
    /// Server only: open a unidirectional stream and write its kind byte first. Messages for
    /// that channel follow.
    OpenStream(StreamKind),
    /// Finish (FIN) the control stream (§12.3 step 1).
    FinishControl,
    /// Close the QUIC connection with this application error code and reason phrase (at most
    /// 64 bytes, §12.3). Nothing else is sent afterwards.
    Close {
        /// The application error code.
        code: AppErrorCode,
        /// The reason phrase.
        reason: String,
    },
}

/// A fatal rule broken by the peer. The state machine has already queued the close
/// (`PROTOCOL_VIOLATION` unless stated; for [`Violation::Flooding`] the server queues
/// `Disconnect(FLOODING)` instead) and moved to Closed (or Closing after a `Disconnect`).
///
/// An input that returns a `Violation` ends the session: the application gets no separate
/// `Closed` event for it.
#[derive(Debug, Clone, PartialEq, Eq, thiserror::Error)]
#[non_exhaustive]
pub enum Violation {
    /// Received bytes did not decode (§13.1).
    #[error("decode: {0}")]
    Decode(#[from] ProtocolError),
    /// A known message the sender's state does not allow (§6.1).
    #[error("{ty} not allowed in state {state}")]
    UnexpectedMessage {
        /// The message type.
        ty: MessageType,
        /// The receiver's state name.
        state: &'static str,
    },
    /// A second request while one of the same kind is outstanding (§13.1).
    #[error("{0} sent while a request is outstanding")]
    RequestOutstanding(MessageType),
    /// A message needs a capability the server did not advertise (§6.2.4, §6.2.7, §6.2.17).
    #[error("{0} needs a capability the server did not advertise")]
    CapabilityMissing(MessageType),
    /// `HelloAck.minor` above the client's `minor_max` (§6.2.2).
    #[error("negotiated minor {minor} above the client's {max}")]
    MinorTooHigh {
        /// The minor the server chose.
        minor: u16,
        /// The client's maximum.
        max: u16,
    },
    /// A world-stream rule of §6.3, §8.4 or §9 was broken.
    #[error("world stream: {0}")]
    World(&'static str),
    /// An extra or wrongly-kinded stream (§2.3 rule 1).
    #[error("unexpected stream: {0}")]
    Stream(&'static str),
    /// The peer's transport does not offer RFC 9221 datagrams (§2.1); closed with
    /// `DATAGRAMS_REQUIRED`.
    #[error("peer does not offer datagrams of at least 1200 bytes")]
    DatagramsRequired,
    /// Client control messages above 50 per second over 5 s; closed with `FLOODING`.
    #[error("control message flood")]
    Flooding,
}

impl Violation {
    /// The application error code the connection is closed with.
    #[must_use]
    pub fn app_code(&self) -> AppErrorCode {
        match self {
            Self::DatagramsRequired => AppErrorCode::DatagramsRequired,
            Self::Flooding => AppErrorCode::Flooding,
            _ => AppErrorCode::ProtocolViolation,
        }
    }
}

/// What the QUIC stack reports about the peer's RFC 9221 datagram support once the
/// handshake completed (§2.1), passed to `handshake_complete`.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum PeerDatagrams {
    /// The peer sent no `max_datagram_frame_size` transport parameter: no datagrams.
    Absent,
    /// The peer's `max_datagram_frame_size` transport parameter, for QUIC stacks that expose
    /// it. Accepted at 1,200 or more (§2.1).
    MaxFrameSize(u64),
    /// The largest datagram payload the QUIC stack can send to the peer now, for stacks that
    /// expose only that (quinn: `Connection::max_datagram_size()`, see
    /// `transport::peer_datagrams`). It is smaller than the parameter (packet and frame
    /// overhead, path MTU), so it is accepted at
    /// [`MIN_DATAGRAM_PAYLOAD`] (1,100) or more.
    MaxPayload(usize),
}

impl PeerDatagrams {
    /// Whether the peer's datagram support meets §2.1.
    #[must_use]
    pub fn acceptable(self) -> bool {
        match self {
            Self::Absent => false,
            Self::MaxFrameSize(n) => n >= MIN_MAX_DATAGRAM_FRAME_SIZE,
            Self::MaxPayload(n) => n >= MIN_DATAGRAM_PAYLOAD,
        }
    }
}

/// Which stream a peer finished, reset or stopped (§2.3 rule 4).
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum StreamRef {
    /// The control stream (either direction).
    Control,
    /// The world or chat stream.
    Uni(StreamKind),
    /// A server-initiated unidirectional stream that ended before its kind byte arrived.
    Untyped,
}

/// How a peer ended a stream (§2.3 rule 4).
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum StreamEnd {
    /// The peer finished its sending side (FIN).
    Finished,
    /// The peer reset its sending side (RESET_STREAM).
    Reset,
    /// The peer asked us to stop sending (STOP_SENDING).
    Stopped,
}

/// How the QUIC connection ended, as the network layer saw it (§12.3, Appendix A).
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum ConnectionEnd {
    /// The peer closed it with this application error code and reason phrase.
    PeerClosed {
        /// The application code.
        code: AppErrorCode,
        /// The reason phrase as received (untrusted).
        reason: String,
    },
    /// The QUIC idle timeout fired.
    IdleTimeout,
    /// A local transport or I/O error, or a QUIC-level close (transport error code).
    Transport(String),
    /// This side closed it (after a [`Transmit::Close`]).
    Local,
}

/// A local call the current state does not allow. Nothing was sent and the state is
/// unchanged; it is not fatal for the connection.
#[derive(Debug, Clone, PartialEq, Eq, thiserror::Error)]
#[non_exhaustive]
pub enum UsageError {
    /// The call is not allowed in the current state.
    #[error("not allowed in state {0}")]
    WrongState(&'static str),
    /// A request of this kind is outstanding.
    #[error("a request is outstanding")]
    Outstanding,
    /// The server did not advertise the needed capability.
    #[error("the server did not advertise the needed capability")]
    CapabilityMissing,
    /// A send-rate limit of the spec would be exceeded.
    #[error("rate limit")]
    RateLimited,
    /// The message would break a wire rule.
    #[error("invalid message: {0}")]
    Invalid(#[from] ProtocolError),
}

/// `reason` cut to at most 64 bytes on a character boundary (§12.3).
#[must_use]
pub fn close_reason(reason: &str) -> String {
    let mut end = reason.len().min(MAX_CLOSE_REASON_LEN);
    while !reason.is_char_boundary(end) {
        end -= 1;
    }
    reason[..end].to_owned()
}

/// A bounded outbox. A state machine never queues much: each input produces at most a few
/// transmits, and the network layer drains them after every call.
#[derive(Debug, Default)]
pub(crate) struct Outbox {
    queue: VecDeque<Transmit>,
}

impl Outbox {
    pub fn push(&mut self, t: Transmit) {
        self.queue.push_back(t);
    }

    pub fn send(&mut self, m: Message) {
        self.queue.push_back(Transmit::Message(m));
    }

    pub fn pop(&mut self) -> Option<Transmit> {
        self.queue.pop_front()
    }

    pub fn clear(&mut self) {
        self.queue.clear();
    }
}

pub(crate) fn micros(d: std::time::Duration) -> u64 {
    u64::try_from(d.as_micros()).unwrap_or(u64::MAX)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn reasons() {
        assert_eq!(close_reason("short"), "short");
        let long = "é".repeat(40);
        let r = close_reason(&long);
        assert!(r.len() <= 64 && r.len() == 64);
        assert_eq!(close_reason(&"x".repeat(100)).len(), 64);
    }
}
