// SPDX-License-Identifier: GPL-3.0-or-later

//! Official-server compatibility crate (`docs/ROADMAP.md` M8).
//!
//! `tessera-compat` is the single crate where official-server protocol code
//! lives (`docs/decisions/0034-official-server-compatibility.md`). Removing it
//! from the workspace withdraws the feature without restructuring the client.
//!
//! Current status: **partly done and needs more work**. This crate implements:
//!
//! 1. [`backend`]: the [`CompatBackend`] shell implementing
//!    [`tessera_net::Backend`].
//! 2. [`login`]: the GLS launcher discovery, SOAP authentication, world status,
//!    login queue, and `RefreshTicket` protocol (`docs/specs/official-login.md`).
//! 3. [`session`]: the UDP transport, packet framing, checksum/ISAAC, `NetBlob`
//!    fragmentation/reassembly, and `NetMessage` envelope (`docs/specs/official-session.md`).
//! 4. [`world`]: common wire value encodings, dictionary-driven property sets,
//!    character-list/selection, world entry, and object lifecycle codecs
//!    (`docs/specs/official-messages-world.md`).
//!
//! Further work is needed to implement the in-world play/chat/WLib RPC codecs
//! (`docs/specs/official-messages-play.md`, `docs/specs/official-messages-gameplay.md`)
//! and wire them end-to-end into [`CompatBackend`] and `apps/client`.

#![forbid(unsafe_code)]
#![warn(missing_docs)]

pub mod backend;
pub mod login;
pub mod session;
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
