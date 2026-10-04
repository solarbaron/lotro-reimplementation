// SPDX-License-Identifier: GPL-3.0-or-later
//! A generic reliable-datagram transport of Tessera's own design.
//!
//! This is *not* a reimplementation of any third-party protocol. It exists so
//! that Tessera's own components (a local test server, tooling, future
//! protocol adapters that want a well-tested reliability layer) can share
//! one implementation. The wire format and algorithms are documented in
//! `docs/specs/transport-design.md` and in [`packet`] and [`endpoint`].
//! Whether Tessera's own M7 protocol uses it (versus, say, QUIC) is left to
//! that milestone's decision record.
//!
//! Layering:
//!
//! ```text
//!   application messages (Vec<u8>)
//!          |  send / send_unreliable / recv
//!   ReliableEndpoint   -- seq/ack, RTO retransmit, fragmentation, reassembly
//!          |  Packet::encode / Packet::decode
//!   Datagram           -- LoopbackDatagram | UdpDatagram | LossyDatagram<D>
//! ```

pub mod datagram;
pub mod endpoint;
pub mod packet;

pub use datagram::{Datagram, LoopbackDatagram, LossConfig, LossyDatagram, UdpDatagram};
pub use endpoint::{EndpointState, Received, ReliableEndpoint, TransportConfig, TransportStats};
