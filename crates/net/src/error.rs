// SPDX-License-Identifier: GPL-3.0-or-later
//! The crate-wide error type.

use crate::session::SessionState;

/// Every fallible operation in `tessera-net` returns this error.
#[derive(Debug, thiserror::Error)]
#[non_exhaustive]
pub enum NetError {
    /// A command was submitted in a session state that does not allow it.
    #[error("command `{command}` is not valid in session state {state:?}")]
    InvalidState {
        /// Name of the rejected command.
        command: &'static str,
        /// The state the session was in.
        state: SessionState,
    },

    /// A command carried invalid arguments (empty text, non-finite position, ...).
    #[error("invalid command: {0}")]
    InvalidCommand(&'static str),

    /// A configuration value is out of its permitted range.
    #[error("invalid configuration: {0}")]
    InvalidConfig(&'static str),

    /// A message exceeds the configured maximum size.
    #[error("message of {len} bytes exceeds the limit of {max} bytes")]
    MessageTooLarge {
        /// Size of the rejected message.
        len: usize,
        /// The applicable limit.
        max: usize,
    },

    /// The outbound queue is full; retry after the peer has acknowledged data.
    #[error("send queue is full")]
    SendQueueFull,

    /// A datagram failed to decode. Only returned by the packet codec; the
    /// endpoint drops such datagrams silently and counts them in its stats.
    #[error("malformed packet: {0}")]
    Malformed(&'static str),

    /// The peer sent well-formed packets whose content breaks the protocol
    /// (inconsistent fragments, oversized reassembly, ...). Fatal.
    #[error("protocol violation by peer: {0}")]
    ProtocolViolation(&'static str),

    /// A reliable segment exceeded its retransmission budget. Fatal.
    #[error("peer stopped acknowledging data")]
    PeerUnresponsive,

    /// Nothing was received from the peer within the idle timeout. Fatal.
    #[error("connection timed out")]
    TimedOut,

    /// The connection is closed (locally or by the peer).
    #[error("connection is closed")]
    Closed,

    /// An I/O error from the underlying datagram socket.
    #[error("datagram I/O error: {0}")]
    Io(#[from] std::io::Error),
}
