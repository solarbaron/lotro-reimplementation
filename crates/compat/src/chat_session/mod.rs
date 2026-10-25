// SPDX-License-Identifier: GPL-3.0-or-later
//! The separate chat-server session (`ChatSession`,
//! `docs/specs/official-messages-play.md` §6.1, §6.6).
//!
//! The chat server is a second UDP session with the transport of
//! `official-session.md` (its own multiplexer and socket, §3.4 there) and its own opcode
//! space `0x15C..=0x195` plus `0x1A2`. Parts:
//!
//! - this file: the endpoint rule of §6.1.1 and the `SessionAuthHeader` of §6.6.2 (ticket
//!   authentication with the Base64-decoded GLS ticket);
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

/// The `authType` values of §6.6.2 (`official-session.md` §6.2.1).
pub use crate::session::{AUTH_TYPE_ACCOUNT, AUTH_TYPE_TICKET};
pub use client::{ChatClient, ChatCodecs, ChatConfig};

use crate::play::PlayError;
use crate::session::{AUTH_FLAG_HAS_EXTRA_BLOB, SessionAuthCredentials, SessionAuthHeader};

/// `recipientId` stamped into every outgoing chat-server fragment (§6.6.1).
pub const CHAT_RECIPIENT_ID: u64 = 0x0700_0000_0000_FFF9;
/// Longest decoded ticket accepted. Engineering choice, spec silent (its examples reach
/// 192 bytes).
pub const MAX_TICKET_BYTES: usize = 4096;

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

/// Decodes standard Base64 (RFC 4648 §4) strictly: the alphabet of §4 only, no
/// whitespace, `=` padding either absent or complete (to a multiple of 4 characters), and
/// the unused low bits of a final partial group zero (RFC 4648 §3.5, canonical encoding).
/// The decoded length is the ticket length of §6.6.2. The bytes decoded before an error
/// are wiped (`official-login.md` §6.3).
///
/// # Errors
///
/// [`PlayError::InvalidTicket`] on a character outside the alphabet, misplaced or partial
/// padding, non-zero pad bits or an impossible length; [`PlayError::TicketTooLong`] above
/// [`MAX_TICKET_BYTES`].
pub fn decode_base64(s: &str) -> Result<Vec<u8>, PlayError> {
    let bytes = s.as_bytes();
    let unpadded = bytes.iter().rposition(|&b| b != b'=').map_or(0, |i| i + 1);
    let padding = bytes.len() - unpadded;
    if padding > 2 || (padding > 0 && !bytes.len().is_multiple_of(4)) {
        return Err(PlayError::InvalidTicket);
    }
    let body = bytes.get(..unpadded).ok_or(PlayError::InvalidTicket)?;
    if body.len() % 4 == 1 {
        return Err(PlayError::InvalidTicket);
    }
    let out_len = body.len() / 4 * 3 + (body.len() % 4).saturating_sub(1);
    if out_len > MAX_TICKET_BYTES {
        return Err(PlayError::TicketTooLong(out_len));
    }
    let value = |c: u8| -> Result<u32, PlayError> {
        Ok(u32::from(match c {
            b'A'..=b'Z' => c - b'A',
            b'a'..=b'z' => c - b'a' + 26,
            b'0'..=b'9' => c - b'0' + 52,
            b'+' => 62,
            b'/' => 63,
            _ => return Err(PlayError::InvalidTicket),
        }))
    };
    let mut out = Vec::with_capacity(out_len);
    let decoded = (|| {
        for chunk in body.chunks(4) {
            let mut acc = 0u32;
            for &c in chunk {
                acc = (acc << 6) | value(c)?;
            }
            acc <<= 6 * (4 - chunk.len() as u32);
            let produced = chunk.len().saturating_sub(1);
            // Canonical form: the bits below the produced bytes are zero.
            let unused_bits = 24 - 8 * produced as u32;
            if acc & ((1u32 << unused_bits) - 1) != 0 {
                return Err(PlayError::InvalidTicket);
            }
            for i in 0..produced {
                out.push((acc >> (16 - 8 * i as u32)) as u8);
            }
        }
        Ok(())
    })();
    match decoded {
        Ok(()) => Ok(out),
        Err(e) => {
            crate::login::zeroize_vec(&mut out);
            Err(e)
        }
    }
}

/// Builds the chat-server `SessionAuthHeader` (§6.6.2 item 1, `official-session.md`
/// §6.2.1): with a ticket, `authType = 0x40000002`, `authFlags = 0x04` and the decoded
/// ticket as `extraBlob` (its length is the decoded length); without one,
/// `authType = 0x00000001` and `authFlags = 0`. `client_timestamp` is the Unix time in
/// seconds when the transport was initialised. The modes are the selection rule of
/// `SessionAuthHeader::for_login`; this builder decodes the ticket with the strict
/// [`decode_base64`] and treats an empty ticket as none.
///
/// # Errors
///
/// [`PlayError::InvalidTicket`] or [`PlayError::TicketTooLong`] for a bad ticket.
pub fn chat_auth_header(
    account: &str,
    ticket_base64: Option<&str>,
    client_timestamp: u32,
) -> Result<SessionAuthHeader, PlayError> {
    let (auth_type, auth_flags, extra_blob) = match ticket_base64 {
        Some(t) if !t.is_empty() => (
            AUTH_TYPE_TICKET,
            AUTH_FLAG_HAS_EXTRA_BLOB,
            Some(decode_base64(t)?),
        ),
        _ => (AUTH_TYPE_ACCOUNT, 0, None),
    };
    Ok(SessionAuthHeader {
        auth_type,
        auth_flags,
        client_timestamp,
        credentials: SessionAuthCredentials {
            account_or_ticket: account.to_owned(),
            second_string: None,
            extra_blob,
        },
    })
}
