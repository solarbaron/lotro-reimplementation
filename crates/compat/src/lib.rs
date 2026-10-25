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
//! 1. [`consent`]: the ban-risk warning of decision 0034 §1 item 4
//!    (`docs/specs/official-entry.md` §5, §6.1): its texts, digest, the acceptance record
//!    and the consent proof. No network code.
//! 2. [`net_access`]: the network-access type (`official-entry.md` §6.1): every network
//!    operation of the crate goes through a [`net_access::NetworkAccess`] value, which only
//!    a consent proof makes.
//! 3. [`backend`]: the [`CompatBackend`] shell implementing [`tessera_net::Backend`], built
//!    only from a [`CompatConfig`] carrying a consent proof and the user's
//!    [`Credentials`]; [`backend::availability`] says whether a live connector exists (the
//!    backend uses the [`PlaceholderConnector`] only).
//! 4. [`login`]: launcher configuration discovery, the SOAP authentication,
//!    world status and login queue exchanges, ticket refresh and the client
//!    network version discovery of decision 0042 item 1.
//! 5. [`version_resource`]: a hostile-input reader for the version resource
//!    of a PE image (public PE/COFF layout), used by the version discovery.
//! 6. [`session`]: the UDP transport (made only through [`net_access`]), datagram framing,
//!    checksums and ISAAC masks, blob fragmentation and reassembly, and message envelopes.
//! 7. [`world`]: wire value encodings, schema-driven encoded property tables,
//!    character list and selection, world entry and object lifecycle codecs.
//!
//! # Rules this crate keeps
//!
//! - No network operation without a consent proof: sockets, name resolution and requests
//!   of any protocol are reachable only through [`net_access::NetworkAccess`]
//!   (`official-entry.md` §6.1; source scan OE-T8 (b)).
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
pub mod chat_session;
pub mod consent;
pub mod credentials;
pub mod login;
pub mod net_access;
pub mod play;
pub mod session;
pub mod version_resource;
pub mod world;

pub use backend::{
    Availability, CompatBackend, CompatConfig, Connector, PlaceholderConnector, availability,
};
pub use credentials::{Credentials, CredentialsError};

/// Errors returned by [`CompatBackend`] and the compatibility modules.
#[derive(Debug, thiserror::Error)]
pub enum CompatError {
    /// The end-to-end official-server session wiring is not yet connected in
    /// [`PlaceholderConnector`].
    #[error("official-server session connector is not wired into the client yet")]
    ProtocolNotBuilt,

    /// The consent proof is not for the shipped warning text (`official-entry.md` §6.1).
    #[error("the consent proof does not match the current warning text")]
    ConsentMismatch,

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
