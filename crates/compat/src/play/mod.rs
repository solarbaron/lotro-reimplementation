// SPDX-License-Identifier: GPL-3.0-or-later
//! In-world play messages of the official world session
//! (`docs/specs/official-messages-play.md`).
//!
//! This module is a sans-I/O layer: it decodes the inner event payloads the session layer
//! hands over (`official-messages-world.md` §1.1.2: the bytes after `envelopeFlags`, starting
//! with `u8 base_flags = 0x00`) and builds the inner payloads of the play messages the client
//! sends. It never opens a socket; the connector sends what it builds through the session
//! layer, which runs only behind [`crate::net_access::NetworkAccess`].
//!
//! Parts:
//!
//! - [`rpc`]: the script RPC envelope `0x30` and `ScriptArgFrame` (§5), with typed codecs of
//!   the chat (`0x04EC`), emote (`0x04F9`) and logout (`0x051E`) methods of §6.3, §6.4, §7.1.
//! - [`chat`]: text limits (§6.2), chat categories (§6.7), the world-server system and
//!   property-bag chat events (§6.5).
//! - [`movement`]: the movement step window and `0x24` (§4.1–§4.2), reconciliation against
//!   `0xCC` (§4.3.2), the teleport stamp state machine (§4.5), `0xBF` (§4.4.2), and the
//!   remote motion events `0x59`, `0xED`, `0xBD` (§4.6).
//! - [`logout`]: the timed logout state machine and the world-teardown opcodes (§7).
//! - [`inbound`]: one dispatcher from `(opcode, recipient, inner payload)` to a decoded
//!   [`inbound::PlayMessage`], counting the out-of-scope admin messages it ignores.
//! - [`adapter`]: the mapping to `tessera_net`'s neutral events and commands (§8).
//!
//! Rules kept here (decision 0042 item 2, play spec §6.1.3, §6.5): admin and game-master
//! messages (`0x30000045`, RPC `0x84EC002C`) are never encoded; when received they are
//! ignored and counted. All input is hostile: counts are checked against the remaining bytes
//! before allocating, `bool8` values must be `0` or `1`, and every decoder returns an error
//! instead of panicking.

pub mod adapter;
pub mod chat;
pub mod inbound;
pub mod logout;
pub mod movement;
pub mod rpc;

use crate::session::SessionError;
use crate::world::WorldError;

/// Errors of the play and chat-server layers (`official-messages-play.md`).
#[derive(Debug, Clone, PartialEq, Eq, thiserror::Error)]
#[non_exhaustive]
pub enum PlayError {
    /// A wire value or world message failed to decode or encode.
    #[error("wire value error: {0}")]
    World(#[from] WorldError),
    /// The UDP session layer reported an error (chat-server session, §6.6).
    #[error("session error: {0}")]
    Session(#[from] SessionError),
    /// A script RPC `function_id` without bit 31 set (§5.1).
    #[error("script RPC function id 0x{0:08X} lacks bit 31")]
    InvalidFunctionId(u32),
    /// A `ScriptStateObject` node without bit 28 in `node_flags` (§9.1 rule 3).
    #[error("script state node flags 0x{0:08X} lack the binary state class bit 0x10000000")]
    InvalidStateNodeFlags(u32),
    /// A `ScriptStateObject` node of a class whose payload this layer cannot size (§5.3).
    #[error("script state class 0x{0:04X} has no known payload layout")]
    UnsupportedStateClass(u16),
    /// The argument frame of a known RPC did not match its signature (§5.3).
    #[error("script RPC 0x{function_id:08X} argument {index} is not of the expected kind")]
    ArgumentMismatch {
        /// The RPC.
        function_id: u32,
        /// Zero-based argument index.
        index: usize,
    },
    /// A text is longer than its UTF-16 code-unit limit (§6.2).
    #[error("text of {units} UTF-16 code units exceeds the limit of {max}")]
    TextTooLong {
        /// Code units of the text.
        units: usize,
        /// The limit.
        max: usize,
    },
    /// An outgoing text is empty.
    #[error("text is empty")]
    EmptyText,
    /// `0x84EC001B` refuses chat categories `1` and `2` locally (§6.3.1, §6.7).
    #[error("chat category {0} is refused locally")]
    ChatTypeRefused(u32),
    /// No chat category number is known for this channel (§6.7: read from the install).
    #[error("no chat category is known for the {0} channel")]
    UnmappedChannel(&'static str),
    /// The command has no world-server mapping in this layer.
    #[error("command {0} is not mapped by the play layer")]
    UnsupportedCommand(&'static str),
    /// The command needs the player in the world.
    #[error("the player is not in the world")]
    NotInWorld,
    /// A `ChatRoomDescriptor` `presence_tag` other than `0` or `0x18` (§9.1 rule 2).
    #[error("chat descriptor presence tag 0x{0:08X} is invalid")]
    InvalidDescriptorTag(u32),
    /// A `ChatRoomDescriptor` `desc_type` outside `1..=3` (§9.1 rule 2).
    #[error("chat descriptor type {0} is outside 1..=3")]
    InvalidDescriptorType(u32),
    /// A chat-server opcode outside `0x15C..=0x195` and not `0x1A2` (§9.1 rule 2).
    #[error("chat-server opcode 0x{0:08X} is outside the chat opcode space")]
    ChatOpcodeOutOfRange(u32),
    /// A chat-server opcode received in the wrong direction (a request from the server).
    #[error("chat-server opcode 0x{0:08X} is not a server-to-client event")]
    UnexpectedChatDirection(u32),
    /// A chat-server opcode that is not a client-to-server request.
    #[error("chat-server opcode 0x{0:08X} is not a client-to-server request")]
    NotAChatRequest(u32),
    /// A `ChatMessagePayload` `extra_type` outside `0..=2` (§6.6.4).
    #[error("chat message extra type {0} is unknown")]
    UnknownExtraType(u32),
    /// A chat-server endpoint string that is not `IPv4:port` (§6.1.1).
    #[error("chat server endpoint {0:?} is not an IPv4 address with a port")]
    InvalidEndpoint(String),
    /// A GLS ticket that is not valid Base64 (§6.6.2).
    #[error("the session ticket is not valid Base64")]
    InvalidTicket,
    /// The decoded ticket is longer than the auth block allows.
    #[error("the decoded session ticket of {0} bytes is too long")]
    TicketTooLong(usize),
    /// A collection is larger than a fixed engineering cap of this layer.
    #[error("{what} holds more than {max} entries")]
    TooMany {
        /// What overflowed.
        what: &'static str,
        /// The cap.
        max: usize,
    },
    /// An `0x59` waypoint, `0xBF` or `0xCE` coordinate failed the bounds of §9.1 rule 4.
    #[error("non-finite or out-of-range {0} in a movement message")]
    BadCoordinate(&'static str),
    /// The chat-server session is not ready (no successful `0x165` yet, §6.6.2).
    #[error("the chat-server session is not ready")]
    ChatNotReady,
    /// No known chat room has this name.
    #[error("no joined chat room is named {0:?}")]
    UnknownRoom(String),
}

/// Number of UTF-16 code units of `text`, the unit of the limits of §6.2.
#[must_use]
pub fn utf16_units(text: &str) -> usize {
    text.encode_utf16().count()
}

/// Checks `text` against a UTF-16 code-unit limit of §6.2 and §9.1 rule 6.
///
/// # Errors
///
/// [`PlayError::TextTooLong`] when the text has more than `max` code units.
pub fn check_units(text: &str, max: usize) -> Result<(), PlayError> {
    let units = utf16_units(text);
    if units > max {
        return Err(PlayError::TextTooLong { units, max });
    }
    Ok(())
}

/// One inner event payload to send on the world-server connection: the `blobHeaderWord`
/// opcode and the inner payload starting with `u8 base_flags = 0x00`
/// (`official-messages-world.md` §1.1.1–§1.1.2). The connector wraps it with
/// `envelopeFlags = 0x00` (`crate::world::encode_blob_payload`) and sends it as a
/// fragmented blob.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct OutgoingEvent {
    /// The event ID (`blobHeaderWord`).
    pub opcode: u32,
    /// The inner payload.
    pub payload: Vec<u8>,
}
