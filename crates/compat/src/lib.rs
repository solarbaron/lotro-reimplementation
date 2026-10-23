// SPDX-License-Identifier: GPL-3.0-or-later

//! Official-server compatibility crate (`docs/ROADMAP.md` M8).
//!
//! `tessera-compat` is the single crate where official-server protocol code
//! lives (`docs/decisions/0034-official-server-compatibility.md`). Removing it
//! from the workspace withdraws the feature without restructuring the client.
//!
//! Current status: **partly done and paused** (incident 0041; decision 0042).
//! The code is kept and will be checked against the M8 specifications once
//! they pass review (decision 0042 item 6). Until then only cleanup,
//! hardening and the neutral renames of decision 0042 item 5 land here; no
//! new protocol behaviour. The crate contains:
//!
//! 1. [`backend`]: the [`CompatBackend`] shell implementing
//!    [`tessera_net::Backend`].
//! 2. [`login`]: launcher configuration discovery, the SOAP authentication,
//!    world status and login queue exchanges, ticket refresh and the client
//!    network version discovery of decision 0042 item 1.
//! 3. [`version_resource`]: a hostile-input reader for the version resource
//!    of a PE image (public PE/COFF layout), used by the version discovery.
//! 4. [`session`]: the UDP transport, datagram framing, checksums and ISAAC
//!    masks, blob fragmentation and reassembly, and message envelopes.
//! 5. [`world`]: wire value encodings, schema-driven encoded property tables,
//!    character list and selection, world entry and object lifecycle codecs.
//!
//! # Rules this crate keeps
//!
//! - No service URL, host or path fragment is built in; every endpoint comes
//!   from the user's install or a server response (decision 0042 item 3).
//! - The client executable is never scanned; only its version resource is
//!   read (decision 0042 item 1).
//! - No admin or game-master message class is sent or acted on (decision
//!   0042 item 2).
//! - All input is hostile: lengths and counts are checked before use,
//!   allocations, recursion and decompression are bounded, and parsers return
//!   errors instead of panicking. Logging goes through `tracing`.

#![forbid(unsafe_code)]
#![warn(missing_docs)]

pub mod backend;
pub mod login;
pub mod session;
pub mod version_resource;
pub mod world;

pub use backend::{CompatBackend, CompatConfig, Connector, PlaceholderConnector};

/// Errors returned by [`CompatBackend`] and the compatibility modules.
#[derive(Debug, thiserror::Error)]
pub enum CompatError {
    /// The end-to-end official-server session wiring is not yet connected in
    /// [`PlaceholderConnector`].
    #[error("official-server session connector is not wired into the client yet")]
    ProtocolNotBuilt,

    /// An error occurred in the official login / launcher / queue flow.
    #[error("official login error: {0}")]
    Login(#[from] login::LoginError),

    /// An error occurred in the official UDP session / transport layer.
    #[error("official session error: {0}")]
    Session(#[from] session::SessionError),

    /// An error occurred in the official world-messages codec.
    #[error("official world message error: {0}")]
    World(#[from] world::WorldError),
}
