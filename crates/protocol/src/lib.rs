// SPDX-License-Identifier: GPL-3.0-or-later
//! `tessera-protocol`: Tessera's own wire protocol, shared by the test-server backend in
//! `tessera-net` and by `tessera-server`.
//!
//! The protocol is designed by this project (milestone M7, `docs/specs/tessera-protocol.md`,
//! decision 0030). It is not related to any official protocol. The core of this crate
//! performs no I/O and runs no async code: it holds the message schema, framing,
//! encode/decode, version negotiation, the sans-IO connection state machines, the
//! property-value model, movement and time-sync helpers, and fingerprint and connect-string
//! handling. Every input is treated as hostile: decoders never panic and check every length
//! and count before allocating (§13).
//!
//! # Layout
//!
//! - [`limits`]: every limit, rate and timeout of the spec.
//! - [`message`]: the message catalogue (§6) as Rust types; [`frame`]: stream framing, kind
//!   bytes and datagrams (§2.3, §5.4, §5.5).
//! - [`types`]: Location, Anchor, Pose, Appearance, Destination, motion byte (§5.3, §8.1);
//!   [`value`]: property values and lists (§7); [`codes`]: error and failure codes (§14);
//!   [`text`]: text classes (§5.2).
//! - [`version`]: ALPN and minor negotiation (§3).
//! - [`client`] and [`server`]: the connection state machines (§12); [`session`]: what they
//!   share.
//! - [`movement`]: send schedule, movement validation, interest blocks (§8, §9); [`time`]:
//!   clock-offset estimation, ticks, world clock (§11); [`rate`]: rate limiters.
//! - [`fingerprint`] and [`connect`]: key fingerprints, the certificate check, connect strings
//!   and the known-servers file (§4).
//! - `transport` (feature `transport`): server key generation and key files, the rustls
//!   pinned-fingerprint verifier and the rustls/quinn configurations (decision 0030).
//!
//! # Example
//!
//! ```
//! use tessera_protocol::frame::{FrameDecoder, frame_to_vec};
//! use tessera_protocol::message::{Channel, Message, Side};
//!
//! let bytes = frame_to_vec(&Message::Ping { nonce: 7 }).unwrap();
//! let mut decoder = FrameDecoder::new(Channel::Control, Side::Client);
//! let mut input: &[u8] = &bytes;
//! assert_eq!(decoder.push(&mut input).unwrap(), Some(Message::Ping { nonce: 7 }));
//! ```

pub mod client;
pub mod codes;
pub mod connect;
pub mod error;
pub mod fingerprint;
pub mod frame;
pub mod limits;
pub mod message;
pub mod movement;
pub mod rate;
pub mod server;
pub mod session;
pub mod text;
pub mod time;
#[cfg(feature = "transport")]
pub mod transport;
pub mod types;
pub mod value;
pub mod version;
mod wire;

pub use error::ProtocolError;
pub use message::{Channel, Message, MessageType, Side};
