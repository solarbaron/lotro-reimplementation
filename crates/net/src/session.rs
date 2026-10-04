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
    Anchor, CharacterSummary, ChatChannel, DataId, Destination, DisconnectReason, EntityId,
    EntityKind, Location, LocationCause, MAX_CHAT_LEN, MAX_NAME_LEN, Placement, Transform, Vec3,
    is_valid_name, sanitize_text,
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
        /// Where the player appears (`f32`; backends that know regions follow
        /// with an [`Event::LocationReport`] carrying the precise position).
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
    /// corrects the client's position (backends that know regions follow it
    /// with an [`Event::LocationReport`] of cause
    /// [`Corrected`](LocationCause::Corrected) carrying the precise position).
    EntityMoved {
        /// The entity that moved.
        id: EntityId,
        /// Its new transform.
        transform: Transform,
        /// Its velocity in metres per second, for extrapolation.
        velocity: Vec3,
    },
    /// A static object came into view: scenery placed by the world data
    /// (buildings, rocks, props), which never moves. Removed again with
    /// [`Event::EntityDespawned`].
    StaticSpawned {
        /// Session-local identifier.
        id: EntityId,
        /// The data record that describes the object (a DID), for the client
        /// to resolve to geometry.
        object: DataId,
        /// Where and how it is placed. Always in the player's current region.
        placement: Placement,
        /// An opaque value from the world source that lets the client match
        /// this object with its own copy of the data (for the sandbox, what
        /// the client's [`WorldSource`](crate::sandbox::world::WorldSource)
        /// put in [`StaticEntity::tag`](crate::sandbox::world::StaticEntity::tag)).
        tag: u64,
    },
    /// Every static object spawned so far ([`Event::StaticSpawned`]) is gone,
    /// as if each had been sent an [`Event::EntityDespawned`]. Sent instead of
    /// one despawn per object when the whole streamed neighbourhood is dropped
    /// at once (a region change, a far teleport, leaving the grid). Dynamic
    /// entities are not affected.
    StaticsCleared,
    /// The precise, region-qualified position of an entity (so far only the
    /// player). Sent right after [`Event::WorldEntered`] by backends that know
    /// regions, after a teleport (followed by an [`Event::EntityMoved`] with
    /// the same position) and in answer to [`Command::QueryLocation`]. A
    /// change of region means every [`Transform`] from now on is in the new
    /// region's frame.
    LocationReport {
        /// The entity.
        id: EntityId,
        /// Where it is.
        location: Location,
        /// Facing in radians.
        heading: f32,
        /// The landblock (and cell) that owns the position when its
        /// coordinates alone do not say (an interior), `None` for an outdoor
        /// position, which belongs to the block that contains it.
        anchor: Option<Anchor>,
        /// Why the report was sent.
        cause: LocationCause,
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
    ///
    /// The position is precise (`f64`) and must be in the player's current
    /// region (the one of the latest [`Event::LocationReport`]; [`Session`]
    /// rejects anything else, use [`Command::Teleport`] to change regions).
    /// After a teleport, the world source ignores moves the client sent
    /// before it received the [`Event::LocationReport`] of that teleport.
    Move {
        /// New position.
        location: Location,
        /// New facing in radians (model convention).
        heading: f32,
        /// Current velocity in metres per second.
        velocity: Vec3,
    },
    /// Ask to be moved to another place. The world source answers every
    /// request exactly once: with [`Event::LocationReport`] (cause
    /// [`Teleported`](LocationCause::Teleported)) or, if it refuses, with a
    /// system chat line. A request sent while an earlier one is still being
    /// resolved supersedes it; the earlier one is answered with a system chat
    /// line saying it was cancelled.
    Teleport {
        /// Where to go.
        destination: Destination,
    },
    /// Ask where the player is. Answered with [`Event::LocationReport`]
    /// (cause [`Requested`](LocationCause::Requested)) and possibly a system
    /// chat line describing the place.
    QueryLocation,
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
            Self::Teleport { .. } => "Teleport",
            Self::QueryLocation => "QueryLocation",
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

    /// The backend as [`Any`](std::any::Any), so [`Session::backend`] can
    /// hand out its concrete type for diagnostics (streaming counters, test
    /// doubles). Backends that offer nothing beyond the trait keep the
    /// default, `None`.
    fn as_any(&self) -> Option<&dyn std::any::Any> {
        None
    }

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
    /// The player's last known precise location and heading.
    player_location: Option<(Location, f32)>,
    /// The anchor of the player's last known location.
    player_anchor: Option<Anchor>,
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
            player_location: None,
            player_anchor: None,
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

    /// The player's last known region-qualified location and heading.
    ///
    /// Set by [`Event::LocationReport`] for the player and then kept current
    /// from accepted [`Command::Move`]s (precise) and [`Event::EntityMoved`]
    /// corrections for the player (which keep the region; a correction that
    /// merely repeats the current position keeps its full precision). `None`
    /// until the backend sent a report (backends without regions may never
    /// send one) and after [`Event::Disconnected`].
    #[must_use]
    pub fn player_location(&self) -> Option<(Location, f32)> {
        self.player_location
    }

    /// The anchor (owning landblock and cell) of
    /// [`player_location`](Self::player_location), from the latest
    /// [`Event::LocationReport`] for the player. Dropped when the player
    /// moves to a position the anchor no longer [owns](Anchor::owns), the
    /// same rule the sandbox applies.
    #[must_use]
    pub fn player_anchor(&self) -> Option<Anchor> {
        self.player_anchor
    }

    /// The wrapped backend's name.
    #[must_use]
    pub fn backend_name(&self) -> &str {
        self.backend.name()
    }

    /// The wrapped backend as its concrete type `T`, for diagnostics (for
    /// example [`SandboxBackend::static_stats`](crate::sandbox::SandboxBackend::static_stats)).
    /// `None` if the backend is not a `T` or does not expose itself through
    /// [`Backend::as_any`]. Read-only: commands still go through
    /// [`send`](Self::send).
    #[must_use]
    pub fn backend<T: Backend + 'static>(&self) -> Option<&T> {
        self.backend.as_any()?.downcast_ref::<T>()
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
            Command::Move { .. }
            | Command::Chat { .. }
            | Command::Teleport { .. }
            | Command::QueryLocation => self.state == S::InWorld,
            Command::Disconnect => !matches!(self.state, S::Idle | S::Closing | S::Closed),
        };
        if !allowed {
            return Err(NetError::InvalidState {
                command: command.name(),
                state: self.state,
            });
        }
        validate(&command)?;
        if let (Command::Move { location, .. }, Some((current, _))) =
            (&command, &self.player_location)
            && location.region != current.region
        {
            return Err(NetError::InvalidCommand(
                "move into another region; use Teleport",
            ));
        }

        let next = match &command {
            Command::Connect => S::Connecting,
            Command::EnterWorld { .. } => S::EnteringWorld,
            Command::Disconnect => S::Closing,
            _ => self.state,
        };
        let moved_to = match &command {
            Command::Move {
                location, heading, ..
            } => Some((*location, *heading)),
            _ => None,
        };
        self.backend.submit(now, command)?;
        if let Some((location, heading)) = moved_to {
            self.track_player(location, heading);
        }
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
    /// transforms or invalid placements (a backend bug), are dropped with a
    /// warning, so consumers can trust the ordering and the values. Inbound
    /// text is cleaned the same way outbound text is checked: chat text and
    /// disconnect reasons are cut to [`MAX_CHAT_LEN`] bytes and names (chat
    /// senders, tell peers, entity and player names) to [`MAX_NAME_LEN`]
    /// bytes, with control characters replaced by spaces; characters in
    /// [`Event::Connected`] whose names fail
    /// [`is_valid_name`] are dropped from the
    /// list (which may be empty).
    pub fn poll_into(&mut self, now: Instant, out: &mut Vec<Event>) {
        let now = self.monotonic(now);
        if self.state == SessionState::Closed {
            return;
        }
        let mut raw = std::mem::take(&mut self.scratch);
        raw.clear();
        self.backend.poll(now, &mut raw);
        for mut event in raw.drain(..) {
            if self.accept(&event) {
                if clean_text(&mut event) {
                    tracing::warn!(
                        backend = self.backend.name(),
                        "cleaned oversized or control-character text from backend"
                    );
                }
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

    /// Moves the tracked player location (keeping its region) and drops an
    /// anchor that no longer owns it.
    fn track_player(&mut self, to: Location, to_heading: f32) {
        if let Some((location, heading)) = &mut self.player_location {
            *location = Location {
                region: location.region,
                ..to
            };
            *heading = to_heading;
            if self.player_anchor.is_some_and(|a| !a.owns(location)) {
                self.player_anchor = None;
            }
        }
    }

    /// Applies an `f32` correction for the player, keeping full precision
    /// when it only repeats the current position.
    fn correct_player(&mut self, transform: Transform) {
        let Some((location, _)) = self.player_location else {
            return;
        };
        let to = if location.position() == transform.position {
            location
        } else {
            Location::from_position(location.region, transform.position)
        };
        self.track_player(to, transform.heading);
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
            Event::StaticSpawned { placement, .. } if !placement.is_valid() => {
                Verdict::Invalid("invalid static placement")
            }
            Event::LocationReport {
                location, heading, ..
            } if !location.is_valid() || !heading.is_finite() => {
                Verdict::Invalid("invalid location report")
            }
            Event::LocationReport {
                anchor: Some(anchor),
                location,
                ..
            } if !anchor.owns(location) => Verdict::Invalid("anchor does not own the location"),
            Event::LocationReport {
                id,
                location,
                heading,
                anchor,
                ..
            } => {
                let verdict = self.world_verdict();
                if matches!(verdict, Verdict::Surface) && Some(*id) == self.player {
                    self.player_location = Some((*location, *heading));
                    self.player_anchor = *anchor;
                }
                verdict
            }
            Event::EntityMoved { id, transform, .. } => {
                let verdict = self.world_verdict();
                if matches!(verdict, Verdict::Surface) && Some(*id) == self.player {
                    self.correct_player(*transform);
                }
                verdict
            }
            Event::EntitySpawned { .. }
            | Event::StaticSpawned { .. }
            | Event::StaticsCleared
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
                self.player_location = None;
                self.player_anchor = None;
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
            location,
            heading,
            velocity,
        } => {
            if !location.is_valid() || !heading.is_finite() || !velocity.is_finite() {
                return Err(NetError::InvalidCommand(
                    "movement not finite or out of range",
                ));
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
        Command::Teleport { destination } => {
            if !destination.is_valid() {
                return Err(NetError::InvalidCommand(
                    "teleport destination out of range or place name invalid",
                ));
            }
        }
        Command::Connect | Command::Disconnect | Command::QueryLocation => {}
    }
    Ok(())
}

/// Replaces `text` with its [`sanitize_text`] form; returns whether it changed.
fn clean(text: &mut String, max_len: usize) -> bool {
    match sanitize_text(text, max_len) {
        std::borrow::Cow::Borrowed(_) => false,
        std::borrow::Cow::Owned(cleaned) => {
            *text = cleaned;
            true
        }
    }
}

/// Cleans every inbound string of `event` (see [`Session::poll_into`]);
/// returns whether anything changed.
fn clean_text(event: &mut Event) -> bool {
    match event {
        Event::Connected { characters } => {
            let before = characters.len();
            characters.retain(|c| is_valid_name(&c.name));
            characters.len() != before
        }
        Event::WorldEntered { name, .. } | Event::EntitySpawned { name, .. } => {
            clean(name, MAX_NAME_LEN)
        }
        Event::ChatMessage {
            channel,
            sender,
            text,
        } => {
            let mut changed = clean(text, MAX_CHAT_LEN);
            if let Some(sender) = sender {
                changed |= clean(sender, MAX_NAME_LEN);
            }
            if let ChatChannel::Tell { peer } = channel {
                changed |= clean(peer, MAX_NAME_LEN);
            }
            changed
        }
        Event::Disconnected {
            reason: DisconnectReason::ServerClosed(why) | DisconnectReason::TransportFailed(why),
        } => clean(why, MAX_CHAT_LEN),
        _ => false,
    }
}
