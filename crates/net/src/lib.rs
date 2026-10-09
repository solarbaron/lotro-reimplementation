// SPDX-License-Identifier: GPL-3.0-or-later
//! `tessera-net`: the boundary between the Tessera client and whatever
//! provides its world.
//!
//! - [`session`]: the [`Backend`] trait and the [`Session`] state machine.
//!   Poll-based, async-free: call [`Session::poll_into`] once per frame.
//! - [`model`]: Tessera's neutral world data model (entities, transforms,
//!   region-qualified locations, static placements, chat), independent of
//!   any wire format.
//! - [`domain`]: the gameplay domain events and commands, grouped by system
//!   ([`Event::Character`], [`Event::Inventory`], [`Event::Combat`],
//!   [`Event::Npc`], [`Event::Social`], [`Event::World`],
//!   [`Event::Progression`], and the same groups for [`Command`];
//!   `docs/specs/scenario-feed.md` §9). Record references travel as
//!   [`DataId`]s or literal labels for the client half to resolve.
//! - [`sandbox`]: an in-process fake world for offline development. With a
//!   [`sandbox::WorldSource`] supplied by the client it spawns the player at
//!   a landblock or named place and serves the static objects and placed
//!   figures (NPCs) around them; it can give the player an appearance and
//!   simulate wanderers and a crowd of animated characters. With a resolved
//!   scenario it plays the scenario feed.
//! - `scenario` (feature `scenario`, on by default): the scenario feed of
//!   decision 0024 §4 (`docs/specs/scenario-feed.md`): the hostile-input-safe
//!   parser of the scenario text format, the selector language and the
//!   `SelectorResolver` contract the client implements over its install (this
//!   crate decodes no game data), the resolved scenario, the deterministic
//!   playback engine the sandbox runs on a virtual clock, the canonical event
//!   log and the headless driver. `tessera-server` never links it (LEGAL.md
//!   §11.1 rule 5).
//! - `remote` (feature `remote`, on by default): the test-server backend. It
//!   connects to a `tessera-server` over Tessera's own protocol
//!   (`docs/specs/tessera-protocol.md`, QUIC per decision 0030) on one network
//!   thread per process and talks to the main thread over bounded channels.
//! - [`interp`]: snapshot interpolation for remote entities (render time,
//!   bounded extrapolation, snap on teleport).
//! - [`transport`]: a generic reliable-datagram transport of our own design
//!   (sequence numbers, acks, RTO retransmission, fragmentation) over a
//!   pluggable [`transport::Datagram`]. Like the session it is poll-driven on
//!   the caller's thread: it spawns no thread and runs no async runtime. The
//!   Tessera protocol does not use it (decision 0030).
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
//!             // The list may be empty (a new account): never index it.
//!             Event::Connected { characters } => {
//!                 if let Some(first) = characters.first() {
//!                     let character = first.name.clone();
//!                     session.send(now, Command::EnterWorld { character })?;
//!                 }
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
pub mod domain;
mod error;
pub mod interp;
pub mod model;
#[cfg(feature = "remote")]
pub mod remote;
pub mod sandbox;
#[cfg(feature = "scenario")]
pub mod scenario;
pub mod session;
pub mod transport;

pub use domain::{
    CharacterCommand, CharacterEvent, CombatCommand, CombatEvent, InventoryCommand, InventoryEvent,
    NpcCommand, NpcEvent, ProgressionCommand, ProgressionEvent, SocialCommand, SocialEvent,
    WorldCommand, WorldEvent,
};
pub use error::NetError;
pub use model::{
    Anchor, Appearance, Body, CharacterSummary, ChatChannel, DataId, Destination, DisconnectReason,
    EntityId, EntityKind, Equipped, Landblock, Location, LocationCause, Placement, RegionId,
    Rotation, Transform, Vec3,
};
pub use session::{Backend, Command, Event, Session, SessionState};
