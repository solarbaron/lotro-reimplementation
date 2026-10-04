// SPDX-License-Identifier: GPL-3.0-or-later
//! `tessera-net`: the boundary between the Tessera client and whatever
//! provides its world.
//!
//! - [`session`]: the [`Backend`] trait and the [`Session`] state machine.
//!   Poll-based, async-free: call [`Session::poll_into`] once per frame.
//! - [`model`]: Tessera's neutral world data model (entities, transforms,
//!   chat), independent of any wire format.
//! - [`sandbox`]: an in-process fake world for offline development.
//! - [`transport`]: a generic reliable-datagram transport of our own design
//!   (sequence numbers, acks, RTO retransmission, fragmentation) over a
//!   pluggable [`transport::Datagram`].
//! - [`compat`]: documentation-only placeholder for a future, legally
//!   reviewed server-compatibility adapter.
//!
//! ```
//! use std::time::{Duration, Instant};
//! use tessera_net::{Command, Event, Session, sandbox::{SandboxBackend, SandboxConfig}};
//!
//! let mut session = Session::new(SandboxBackend::new(SandboxConfig::default())?);
//! let mut now = Instant::now();
//! session.send(now, Command::Connect)?;
//!
//! // One buffer, reused every frame; handling an event may send a command.
//! let mut events = Vec::new();
//! for _frame in 0..3 {
//!     session.poll_into(now, &mut events);
//!     for event in events.drain(..) {
//!         match event {
//!             Event::Connected { characters } => {
//!                 let character = characters[0].name.clone();
//!                 session.send(now, Command::EnterWorld { character })?;
//!             }
//!             Event::WorldEntered { name, .. } => assert_eq!(name, "Wanderer"),
//!             _ => {}
//!         }
//!     }
//!     now += Duration::from_millis(16);
//! }
//! assert!(session.player().is_some());
//! # Ok::<(), tessera_net::NetError>(())
//! ```

pub mod compat;
mod error;
pub mod model;
pub mod sandbox;
pub mod session;
pub mod transport;

pub use error::NetError;
pub use model::{
    CharacterSummary, ChatChannel, DisconnectReason, EntityId, EntityKind, Transform, Vec3,
};
pub use session::{Backend, Command, Event, Session, SessionState};
