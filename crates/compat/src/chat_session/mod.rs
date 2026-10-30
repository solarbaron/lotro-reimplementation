// SPDX-License-Identifier: GPL-3.0-or-later
//! The separate chat-server session (`ChatSession`,
//! `docs/specs/official-messages-play.md` §6.1, §6.6).
//!
//! The chat server is a second UDP session with the transport of
//! `official-session.md` (its own multiplexer and socket, §3.4 there) and its own opcode
//! space `0x15C..=0x195` plus `0x1A2`. Parts:
//!
//! - this file: the endpoint rule of §6.1.1 and the `SessionAuthHeader` of §6.6.2 (ticket
//!   authentication with the undecoded ASCII Base64 GLS ticket, `netVersion` `"180804"`);
//! - [`wire`]: `ChatRoomDescriptor`, the container encodings and `ChatMessagePayload`
//!   (§6.6.4);
//! - [`events`]: all 59 events of the catalogue (§6.6.5) as typed requests and events;
//! - [`state`]: the login gate (`0x165`), heartbeat (`0x17E` / `0x174`) and retry policies
//!   (§6.6.2–§6.6.3), sans I/O;
//! - [`client`]: [`ChatClient`], which drives a session connection over a socket made by
//!   [`crate::net_access::NetworkAccess`], keeps the room directory and maps to
//!   `tessera_net`'s neutral events and commands (§8).
//!
//! Endpoints come only from the user's launch parameters at run time; nothing here names
//! a host. The admin request `0x17D` is never encoded; its response `0x15F` is ignored and
//! counted (decision 0042 item 2).

pub mod client;
pub mod events;
pub mod state;
pub mod wire;

use std::net::SocketAddrV4;

/// The `authType` of the chat connection (§6.6.2, `official-session.md` §6.2.1:
/// `0x40000002`) and of the primary connection (`0x00000001`).
pub use crate::session::{AUTH_TYPE_CHAT, AUTH_TYPE_PRIMARY};
pub use client::{ChatClient, ChatCodecs, ChatConfig};

use crate::play::PlayError;
use crate::session::{AuthConnection, SessionAuthHeader, SessionError};

/// `recipientId` stamped into every outgoing chat-server fragment (§6.6.1).
pub const CHAT_RECIPIENT_ID: u64 = 0x0700_0000_0000_FFF9;

/// The effective chat-server endpoint string of §6.1.1: `None` when the chat subsystem is
/// off (neither `--usechat` nor a `--chatserver` value); `--host` when `--chatserver` is
/// empty.
#[must_use]
pub fn effective_endpoint(chatserver: &str, usechat: bool, host: &str) -> Option<String> {
    let chatserver = chatserver.trim();
    let use_chat = usechat || !chatserver.is_empty();
    if !use_chat {
        return None;
    }
    let s = if chatserver.is_empty() {
        host.trim()
    } else {
        chatserver
    };
    (!s.is_empty()).then(|| s.to_owned())
}

/// Parses an endpoint of the form `a.b.c.d:port` (an IPv4 literal with a non-zero port).
///
/// Host names are refused: resolving one is a network operation that would have to go
/// through [`crate::net_access::NetworkAccess`], which offers no resolver yet.
///
/// # Errors
///
/// [`PlayError::InvalidEndpoint`] for anything else.
pub fn parse_endpoint(s: &str) -> Result<SocketAddrV4, PlayError> {
    let bad = || PlayError::InvalidEndpoint(s.chars().take(64).collect());
    let addr: SocketAddrV4 = s.trim().parse().map_err(|_| bad())?;
    if addr.port() == 0 || addr.ip().is_unspecified() {
        return Err(bad());
    }
    Ok(addr)
}

/// Checks a GLS ticket for the chat connection: it is sent as its ASCII Base64 text, never
/// decoded (§6.6.2; `official-session.md` §6.2.1), so this is
/// [`crate::session::check_ticket_text`].
///
/// # Errors
///
/// [`PlayError::InvalidTicket`] for an empty, over-long or non-printable string.
pub fn check_ticket(s: &str) -> Result<(), PlayError> {
    crate::session::check_ticket_text(s).map_err(|_| PlayError::InvalidTicket)
}

/// Builds the chat-server `SessionAuthHeader` (§6.6.2 item 1, `official-session.md`
/// §6.2–§6.2.1): `netVersion` `"180804"`, `authType = 0x40000002`, and with a ticket
/// `authFlags = 0x04` and the ticket's ASCII Base64 text (undecoded) in `extraBlob`;
/// without one, `authFlags = 0` (account only). `client_timestamp` is the Unix time in
/// seconds when the transport was initialised. An empty ticket counts as none.
///
/// # Errors
///
/// [`PlayError::InvalidTicket`] for a ticket that fails [`check_ticket`].
pub fn chat_auth_header(
    account: &str,
    ticket_base64: Option<&str>,
    client_timestamp: u32,
) -> Result<SessionAuthHeader, PlayError> {
    SessionAuthHeader::for_connection(
        AuthConnection::Chat,
        "",
        account,
        ticket_base64,
        None,
        client_timestamp,
    )
    .map_err(|e| match e {
        SessionError::InvalidTicketEncoding => PlayError::InvalidTicket,
        other => PlayError::from(other),
    })
}
