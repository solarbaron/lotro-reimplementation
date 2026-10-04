// SPDX-License-Identifier: GPL-3.0-or-later
//! The boundary between the game client and a "world source".
//!
//! A world source is anything that can tell the client what exists in the
//! world: the in-process [`SandboxBackend`](crate::sandbox::SandboxBackend),
//! a replay file, or (in future, see [`compat`](crate::compat)) a network
//! protocol adapter. All of them implement [`Backend`].
//!
//! The model is **poll-based and async-free**: the client calls
//! [`Session::poll_into`] (or [`Session::poll`]) once per frame with the
//! current time and handles the events it returns. Commands are submitted
//! with [`Session::send`] and take effect no earlier than the next poll. No
//! backend may block in either call.
//!
//! Time is supplied by the caller and must be **monotonic**: every `now`
//! passed to a backend is at least the previous one. [`Session`] enforces
//! this by clamping a stale `now` (e.g. a cached frame time used in an input
//! handler after a newer poll) to the latest one it has seen.
//!
//! [`Session`] wraps a backend and enforces the session state machine:
//!
//! ```text
//!  Idle --Connect--> Connecting --Connected--> Connected --EnterWorld--> EnteringWorld
//!                                                                         |
//!                     Closed <--Disconnected-- (any) <------WorldEntered--+--> InWorld
//! ```
//!
//! so backends and UI code can rely on events arriving in a sensible order.
//! In particular, entity events are only surfaced after
//! [`Event::WorldEntered`], and only until [`Event::Disconnected`].

use std::time::Instant;

use crate::NetError;
use crate::model::{
    CharacterSummary, ChatChannel, DisconnectReason, EntityId, EntityKind, MAX_CHAT_LEN, Transform,
    Vec3, is_valid_name,
};

/// Something that happened in the world, reported by a backend.
#[derive(Debug, Clone, PartialEq)]
#[non_exhaustive]
pub enum Event {
    /// The world source accepted the connection.
    Connected {
        /// Characters that may enter the world.
        characters: Vec<CharacterSummary>,
    },
    /// The player character is in the world.
    WorldEntered {
        /// The entity that represents the player.
        player: EntityId,
        /// Name of the character that entered.
        name: String,
        /// Where the player appears.
        transform: Transform,
    },
    /// An entity came into view.
    EntitySpawned {
        /// The new entity.
        id: EntityId,
        /// Its broad classification.
        kind: EntityKind,
        /// Display name (may be empty).
        name: String,
        /// Where it is.
        transform: Transform,
    },
    /// An entity moved. Also sent for the player when the world source
    /// corrects the client's position.
    EntityMoved {
        /// The entity that moved.
        id: EntityId,
        /// Its new transform.
        transform: Transform,
        /// Its velocity in metres per second, for extrapolation.
        velocity: Vec3,
    },
    /// An entity left view.
    EntityDespawned {
        /// The entity that left.
        id: EntityId,
    },
    /// A chat line.
    ChatMessage {
        /// Channel it was sent on.
        channel: ChatChannel,
        /// Sender name; `None` for system messages.
        sender: Option<String>,
        /// The text.
        text: String,
    },
    /// The session ended. No further events follow.
    Disconnected {
        /// Why.
        reason: DisconnectReason,
    },
}

/// Something the client asks the world source to do.
#[derive(Debug, Clone, PartialEq)]
#[non_exhaustive]
pub enum Command {
    /// Open the session. Credentials, if any, are part of the backend's own
    /// construction, never of this neutral model.
    Connect,
    /// Enter the world with the named character.
    EnterWorld {
        /// A name from [`Event::Connected`]'s character list.
        character: String,
    },
    /// Report the player's movement.
    Move {
        /// New transform.
        transform: Transform,
        /// Current velocity in metres per second.
        velocity: Vec3,
    },
    /// Say something.
    Chat {
        /// Channel to use.
        channel: ChatChannel,
        /// Text, 1..=[`MAX_CHAT_LEN`] bytes.
        text: String,
    },
    /// Close the session gracefully. The backend answers with
    /// [`Event::Disconnected`].
    Disconnect,
}

impl Command {
    /// A short static name, for logs and errors.
    #[must_use]
    pub fn name(&self) -> &'static str {
        match self {
            Self::Connect => "Connect",
            Self::EnterWorld { .. } => "EnterWorld",
            Self::Move { .. } => "Move",
            Self::Chat { .. } => "Chat",
            Self::Disconnect => "Disconnect",
        }
    }
}

/// A world source.
///
/// Implementations must be non-blocking: both methods return promptly.
/// [`Session`] validates commands before forwarding them, but a backend must
/// still treat everything it receives from *its* side (network peers, files)
/// as hostile.
///
/// The `now` passed to [`submit`](Backend::submit) and
/// [`poll`](Backend::poll) is non-decreasing across all calls on one
/// backend; [`Session`] guarantees this, and a backend may rely on it.
pub trait Backend: Send + std::fmt::Debug {
    /// Short human-readable backend name, e.g. `"sandbox"`.
    fn name(&self) -> &str;

    /// Accepts a command. Its effects become visible through later
    /// [`poll`](Backend::poll) calls.
    ///
    /// # Errors
    ///
    /// Backend-specific; e.g. a full send queue.
    fn submit(&mut self, now: Instant, command: Command) -> Result<(), NetError>;

    /// Advances the backend to `now` and appends any new events to `out`.
    ///
    /// Fatal backend failures are reported as [`Event::Disconnected`], never
    /// as a panic.
    fn poll(&mut self, now: Instant, out: &mut Vec<Event>);
}

/// Where a [`Session`] is in its lifecycle.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum SessionState {
    /// Created, `Connect` not yet sent.
    Idle,
    /// `Connect` sent, waiting for [`Event::Connected`].
    Connecting,
    /// Connected; character selection.
    Connected,
    /// `EnterWorld` sent, waiting for [`Event::WorldEntered`].
    EnteringWorld,
    /// In the world.
    InWorld,
    /// `Disconnect` sent, waiting for [`Event::Disconnected`]. Answers to
    /// requests that were still outstanding (`Connected`, `WorldEntered`,
    /// and chat sent while connecting or entering the world) are dropped,
    /// and entity events are surfaced only if the world had been entered.
    Closing,
    /// Ended. Terminal.
    Closed,
}

/// The client's handle on a world source; see the module documentation.
#[derive(Debug)]
pub struct Session {
    backend: Box<dyn Backend>,
    state: SessionState,
    /// State before `Disconnect` moved the session to `Closing`.
    closing_from: SessionState,
    player: Option<EntityId>,
    /// Latest `now` handed to the backend.
    last_now: Option<Instant>,
    scratch: Vec<Event>,
}

/// What [`Session::accept`] decided about an event.
enum Verdict {
    Surface,
    /// Expected while closing (an answer to a request that was overtaken by
    /// `Disconnect`); dropped quietly.
    Stale,
    /// Makes no sense in this state, or is malformed: a backend bug.
    Invalid(&'static str),
}

impl Session {
    /// Wraps a backend. The session starts in [`SessionState::Idle`].
    pub fn new(backend: impl Backend + 'static) -> Self {
        Self::from_boxed(Box::new(backend))
    }

    /// Wraps an already boxed backend.
    #[must_use]
    pub fn from_boxed(backend: Box<dyn Backend>) -> Self {
        Self {
            backend,
            state: SessionState::Idle,
            closing_from: SessionState::Idle,
            player: None,
            last_now: None,
            scratch: Vec::new(),
        }
    }

    /// Current lifecycle state.
    #[must_use]
    pub fn state(&self) -> SessionState {
        self.state
    }

    /// The player's entity from [`Event::WorldEntered`] until
    /// [`Event::Disconnected`].
    #[must_use]
    pub fn player(&self) -> Option<EntityId> {
        self.player
    }

    /// The wrapped backend's name.
    #[must_use]
    pub fn backend_name(&self) -> &str {
        self.backend.name()
    }

    /// Validates `command` against the current state and forwards it.
    ///
    /// # Errors
    ///
    /// [`NetError::InvalidState`] if the command is not allowed now,
    /// [`NetError::InvalidCommand`] if its arguments are out of range, or
    /// whatever the backend returns.
    pub fn send(&mut self, now: Instant, command: Command) -> Result<(), NetError> {
        use SessionState as S;
        let now = self.monotonic(now);
        let allowed = match &command {
            Command::Connect => self.state == S::Idle,
            Command::EnterWorld { .. } => self.state == S::Connected,
            Command::Move { .. } | Command::Chat { .. } => self.state == S::InWorld,
            Command::Disconnect => !matches!(self.state, S::Idle | S::Closing | S::Closed),
        };
        if !allowed {
            return Err(NetError::InvalidState {
                command: command.name(),
                state: self.state,
            });
        }
        validate(&command)?;

        let next = match &command {
            Command::Connect => S::Connecting,
            Command::EnterWorld { .. } => S::EnteringWorld,
            Command::Disconnect => S::Closing,
            Command::Move { .. } | Command::Chat { .. } => self.state,
        };
        self.backend.submit(now, command)?;
        if next == S::Closing {
            self.closing_from = self.state;
        }
        self.state = next;
        Ok(())
    }

    /// Advances the backend to `now` and **appends** the new events, in
    /// order, to `out`. Reusing one buffer across frames avoids a per-frame
    /// allocation, and since `out` does not borrow the session, the caller
    /// can [`send`](Self::send) commands while handling the events:
    ///
    /// ```
    /// # use std::time::Instant;
    /// # use tessera_net::{Command, Event, Session, sandbox::{SandboxBackend, SandboxConfig}};
    /// # let mut session = Session::new(SandboxBackend::new(SandboxConfig::default())?);
    /// # let now = Instant::now();
    /// # session.send(now, Command::Connect)?;
    /// let mut events = Vec::new();
    /// session.poll_into(now, &mut events);
    /// for event in events.drain(..) {
    ///     if let Event::Connected { characters } = event {
    ///         let character = characters[0].name.clone();
    ///         session.send(now, Command::EnterWorld { character })?;
    ///     }
    /// }
    /// # Ok::<(), tessera_net::NetError>(())
    /// ```
    ///
    /// Events that make no sense in the current state, or carry non-finite
    /// transforms (a backend bug), are dropped with a warning, so consumers
    /// can trust the ordering and the values.
    pub fn poll_into(&mut self, now: Instant, out: &mut Vec<Event>) {
        let now = self.monotonic(now);
        if self.state == SessionState::Closed {
            return;
        }
        let mut raw = std::mem::take(&mut self.scratch);
        raw.clear();
        self.backend.poll(now, &mut raw);
        for event in raw.drain(..) {
            if self.accept(&event) {
                out.push(event);
            }
        }
        self.scratch = raw;
    }

    /// Like [`poll_into`](Self::poll_into), returning a fresh `Vec`.
    #[must_use]
    pub fn poll(&mut self, now: Instant) -> Vec<Event> {
        let mut out = Vec::new();
        self.poll_into(now, &mut out);
        out
    }

    /// Clamps `now` so the backend never sees time go backwards.
    fn monotonic(&mut self, now: Instant) -> Instant {
        let now = self.last_now.map_or(now, |last| last.max(now));
        self.last_now = Some(now);
        now
    }

    /// Entity events are meaningful only once the world was entered.
    fn world_verdict(&self) -> Verdict {
        match self.state {
            SessionState::InWorld => Verdict::Surface,
            SessionState::Closing if self.player.is_some() => Verdict::Surface,
            SessionState::Closing => Verdict::Stale,
            _ => Verdict::Invalid("entity event outside the world"),
        }
    }

    /// Applies an event to the state machine; returns whether to surface it.
    fn accept(&mut self, event: &Event) -> bool {
        use SessionState as S;
        let closing = self.state == S::Closing;
        let verdict = match event {
            Event::Connected { .. } => match self.state {
                S::Connecting => {
                    self.state = S::Connected;
                    Verdict::Surface
                }
                _ if closing => Verdict::Stale,
                _ => Verdict::Invalid("unexpected Connected"),
            },
            Event::WorldEntered {
                player, transform, ..
            } => {
                if !transform.is_finite() {
                    Verdict::Invalid("non-finite transform")
                } else if self.state == S::EnteringWorld {
                    self.state = S::InWorld;
                    self.player = Some(*player);
                    Verdict::Surface
                } else if closing {
                    Verdict::Stale
                } else {
                    Verdict::Invalid("unexpected WorldEntered")
                }
            }
            Event::EntitySpawned { transform, .. } if !transform.is_finite() => {
                Verdict::Invalid("non-finite transform")
            }
            Event::EntityMoved {
                transform,
                velocity,
                ..
            } if !transform.is_finite() || !velocity.is_finite() => {
                Verdict::Invalid("non-finite transform or velocity")
            }
            Event::EntitySpawned { .. }
            | Event::EntityMoved { .. }
            | Event::EntityDespawned { .. } => self.world_verdict(),
            Event::ChatMessage { .. } => match self.state {
                S::Connected | S::EnteringWorld | S::InWorld => Verdict::Surface,
                // Chat that answered an interrupted connect or world entry
                // (e.g. a welcome line) is stale once Disconnect was sent.
                S::Closing if matches!(self.closing_from, S::Connected | S::InWorld) => {
                    Verdict::Surface
                }
                S::Closing => Verdict::Stale,
                _ => Verdict::Invalid("chat before connecting"),
            },
            Event::Disconnected { .. } => {
                let ok = self.state != S::Closed;
                self.state = S::Closed;
                self.player = None;
                if ok {
                    Verdict::Surface
                } else {
                    Verdict::Invalid("already disconnected")
                }
            }
        };
        match verdict {
            Verdict::Surface => true,
            Verdict::Stale => {
                tracing::debug!(
                    backend = self.backend.name(),
                    ?event,
                    "dropping event overtaken by Disconnect"
                );
                false
            }
            Verdict::Invalid(why) => {
                tracing::warn!(
                    backend = self.backend.name(),
                    state = ?self.state,
                    ?event,
                    why,
                    "dropping invalid event from backend"
                );
                false
            }
        }
    }
}

fn validate(command: &Command) -> Result<(), NetError> {
    match command {
        Command::EnterWorld { character } => {
            if !is_valid_name(character) {
                return Err(NetError::InvalidCommand(
                    "character name empty, too long or has control characters",
                ));
            }
        }
        Command::Move {
            transform,
            velocity,
        } => {
            if !transform.is_finite() || !velocity.is_finite() {
                return Err(NetError::InvalidCommand("non-finite movement"));
            }
        }
        Command::Chat { channel, text } => {
            if text.is_empty() || text.len() > MAX_CHAT_LEN {
                return Err(NetError::InvalidCommand("chat text length out of range"));
            }
            if text.chars().any(|c| c.is_control()) {
                return Err(NetError::InvalidCommand(
                    "chat text contains control characters",
                ));
            }
            match channel {
                ChatChannel::System => {
                    return Err(NetError::InvalidCommand(
                        "cannot send on the system channel",
                    ));
                }
                ChatChannel::Tell { peer } if !is_valid_name(peer) => {
                    return Err(NetError::InvalidCommand(
                        "tell recipient empty, too long or has control characters",
                    ));
                }
                _ => {}
            }
        }
        Command::Connect | Command::Disconnect => {}
    }
    Ok(())
}
