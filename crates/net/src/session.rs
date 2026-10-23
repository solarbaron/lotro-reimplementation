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
use crate::domain::{
    CharacterCommand, CharacterEvent, CombatCommand, CombatEvent, InventoryCommand, InventoryEvent,
    NpcCommand, NpcEvent, ProgressionCommand, ProgressionEvent, SocialCommand, SocialEvent,
    WorldCommand, WorldEvent,
};
use crate::model::{
    Anchor, Appearance, CharacterSummary, ChatChannel, ChatRejection, DataId, Destination,
    DisconnectReason, EntityId, EntityKind, EntityState, Location, LocationCause, MAX_CHAT_LEN,
    MAX_NAME_LEN, NoticeLevel, Placement, Stance, TeleportRefusal, Transform, Vec3, is_valid_name,
    sanitize_text,
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
    /// Every static object spawned so far ([`Event::StaticSpawned`]) and every
    /// entity created with a [`tag`](Event::EntityCreated::tag) (placed by the
    /// world data) is gone, as if each had been sent an
    /// [`Event::EntityDespawned`]. Sent instead of one despawn per object when
    /// the whole streamed neighbourhood is dropped at once (a region change, a
    /// far teleport, leaving the grid). Other entities are not affected.
    StaticsCleared,
    /// An entity came into view with a precise position and an appearance:
    /// a character, creature or object the client assembles and animates
    /// (`docs/ARCHITECTURE.md` §19.1 "entity created"). Moves follow as
    /// [`Event::EntityMoved`]; removal as [`Event::EntityDespawned`] (or
    /// [`Event::StaticsCleared`] for an entity with a tag).
    EntityCreated {
        /// The new entity.
        id: EntityId,
        /// Its broad classification.
        kind: EntityKind,
        /// Display name (may be empty).
        name: String,
        /// The data record of the entity's class (for an object placed by the
        /// world data, the game object it places), if there is one.
        class: Option<DataId>,
        /// Where it is (the player's current region).
        location: Location,
        /// Facing in radians (module convention of [`crate::model`]).
        heading: f32,
        /// How it looks.
        appearance: Box<Appearance>,
        /// `Some` for an entity placed by the world data: an opaque value from
        /// the world source that lets the client match it with its own copy of
        /// the data (for the sandbox, what the client's
        /// [`WorldSource`](crate::sandbox::world::WorldSource) put in
        /// [`StaticEntity::tag`](crate::sandbox::world::StaticEntity::tag)).
        /// `None` for an entity the world source simulates itself.
        tag: Option<u64>,
    },
    /// An entity's appearance changed, or (for the player, right after
    /// [`Event::WorldEntered`]) became known.
    AppearanceChanged {
        /// The entity.
        id: EntityId,
        /// How it looks now.
        appearance: Box<Appearance>,
    },
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
    /// The world source refused [`Command::EnterWorld`] (`docs/ARCHITECTURE.md` §19.1
    /// "world-state change"). The session is back in [`SessionState::Connected`] and may
    /// send another `EnterWorld`, a character command or `Disconnect`.
    EnterWorldFailed {
        /// The character that was asked for.
        character: String,
        /// Why, in Tessera's own words (the world source's detail appended when it gave
        /// one).
        reason: String,
    },
    /// The replicated state of an entity or the player changed (`docs/ARCHITECTURE.md`
    /// §19.1 "property changed"): its whole [`EntityState`] after the change.
    EntityStateChanged {
        /// The entity (the player included).
        id: EntityId,
        /// Its state now.
        state: EntityState,
    },
    /// The world source's clock (`tessera-protocol.md` §11.3): the game clock *G* in
    /// seconds at a server time, and how fast it runs. The client turns *G* into a time of
    /// day with its own data's constants.
    WorldClock {
        /// The server time at which `game_clock` holds, µs.
        server_time_us: u64,
        /// The game clock *G* in seconds (finite, ≥ 0).
        game_clock: f64,
        /// Game seconds per real second (finite, ≥ 0).
        rate: f32,
    },
    /// A clock-synchronisation sample was taken (`docs/ARCHITECTURE.md` §19.1 "time sync";
    /// `tessera-protocol.md` §11.2): the estimate in use after it.
    TimeSynced {
        /// Server clock minus client clock, µs.
        offset_us: i64,
        /// Smallest round-trip delay among the samples kept, µs.
        delay_us: i64,
    },
    /// A [`Command::Teleport`] was refused (its one answer, instead of a
    /// [`Event::LocationReport`]).
    TeleportRefused {
        /// Why.
        reason: TeleportRefusal,
        /// Tessera's line for it, with the world source's detail when it gave one.
        text: String,
    },
    /// A [`Command::Chat`] line was not delivered.
    ChatRejected {
        /// Why.
        reason: ChatRejection,
        /// Tessera's line for it, with the world source's detail when it gave one.
        text: String,
    },
    /// A notice from the world source's operator.
    ServerNotice {
        /// How urgent it is.
        level: NoticeLevel,
        /// For [`NoticeLevel::Shutdown`], seconds until the world source stops; else 0.
        seconds: u32,
        /// The notice as the operator wrote it.
        text: String,
    },
    /// A character event (character list details, levels;
    /// `docs/specs/scenario-feed.md` §9.2).
    Character(CharacterEvent),
    /// An inventory event (containers, items, wallet).
    Inventory(InventoryEvent),
    /// A combat event (targets, vitals, casts, effects, damage).
    Combat(CombatEvent),
    /// An NPC window event (dialogue, quests, vendors, loot).
    Npc(NpcEvent),
    /// A social event (channels, fellowship, trade, lists, music).
    Social(SocialEvent),
    /// A world event (markers, travel, mounts, instances).
    World(WorldEvent),
    /// A progression event (experience, skills, traits, crafting, reputation).
    Progression(ProgressionEvent),
}

impl Event {
    /// The variant's name, for logs; for a grouped event the inner variant's name.
    #[must_use]
    pub fn name(&self) -> &'static str {
        match self {
            Self::Connected { .. } => "Connected",
            Self::WorldEntered { .. } => "WorldEntered",
            Self::EntitySpawned { .. } => "EntitySpawned",
            Self::EntityMoved { .. } => "EntityMoved",
            Self::StaticSpawned { .. } => "StaticSpawned",
            Self::StaticsCleared => "StaticsCleared",
            Self::EntityCreated { .. } => "EntityCreated",
            Self::AppearanceChanged { .. } => "AppearanceChanged",
            Self::LocationReport { .. } => "LocationReport",
            Self::EntityDespawned { .. } => "EntityDespawned",
            Self::ChatMessage { .. } => "ChatMessage",
            Self::Disconnected { .. } => "Disconnected",
            Self::EnterWorldFailed { .. } => "EnterWorldFailed",
            Self::EntityStateChanged { .. } => "EntityStateChanged",
            Self::WorldClock { .. } => "WorldClock",
            Self::TimeSynced { .. } => "TimeSynced",
            Self::TeleportRefused { .. } => "TeleportRefused",
            Self::ChatRejected { .. } => "ChatRejected",
            Self::ServerNotice { .. } => "ServerNotice",
            Self::Character(e) => e.name(),
            Self::Inventory(e) => e.name(),
            Self::Combat(e) => e.name(),
            Self::Npc(e) => e.name(),
            Self::Social(e) => e.name(),
            Self::World(e) => e.name(),
            Self::Progression(e) => e.name(),
        }
    }
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
        /// Whether the player's controller stands on the ground. `false` while jumping
        /// or falling; the test-server backend then sends gait Airborne
        /// (`tessera-protocol.md` §8.1). Other backends may ignore it.
        on_ground: bool,
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
    /// Open the session like [`Command::Connect`], creating the account first: the
    /// backend's credentials are registered as a new account and then logged in
    /// (`docs/ARCHITECTURE.md` §19.1). The remote backend sends `Register`; the sandbox
    /// backend, which has no accounts, treats it as `Connect`. Other backends may not
    /// support it: one that ignores it leaves the session in
    /// [`SessionState::Connecting`], so send `Connect` to backends that create no
    /// accounts.
    Register,
    /// Play an emote (in the world): `anim` is an opaque animation key from the client's
    /// data, 0 stops the current emote. Answered with [`Event::EntityStateChanged`] for
    /// the player by backends that replicate it.
    Emote {
        /// The animation key; 0 stops.
        anim: u32,
        /// Play until stopped.
        looping: bool,
    },
    /// Set the player's stance (in the world). Answered with
    /// [`Event::EntityStateChanged`] for the player by backends that replicate it.
    Stance {
        /// The new stance.
        stance: Stance,
    },
    /// Ask to change the player's appearance (in the world). The world source answers
    /// with an [`Event::AppearanceChanged`] for the player carrying the result; one that
    /// does not allow it refuses the command with [`NetError::InvalidCommand`].
    SetAppearance {
        /// The appearance asked for ([`Appearance::is_valid`]).
        appearance: Box<Appearance>,
    },
    /// A character command (allowed while connected, before world entry).
    Character(CharacterCommand),
    /// An inventory command.
    Inventory(InventoryCommand),
    /// A combat command.
    Combat(CombatCommand),
    /// An NPC window command.
    Npc(NpcCommand),
    /// A social command.
    Social(SocialCommand),
    /// A world command.
    World(WorldCommand),
    /// A progression command.
    Progression(ProgressionCommand),
}

impl Command {
    /// A short static name, for logs and errors; for a grouped command the inner variant's
    /// name.
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
            Self::Register => "Register",
            Self::Emote { .. } => "Emote",
            Self::Stance { .. } => "Stance",
            Self::SetAppearance { .. } => "SetAppearance",
            Self::Character(c) => c.name(),
            Self::Inventory(c) => c.name(),
            Self::Combat(c) => c.name(),
            Self::Npc(c) => c.name(),
            Self::Social(c) => c.name(),
            Self::World(c) => c.name(),
            Self::Progression(c) => c.name(),
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
            Command::Connect | Command::Register => self.state == S::Idle,
            Command::EnterWorld { .. } => self.state == S::Connected,
            Command::Character(_) => self.state == S::Connected,
            Command::Move { .. }
            | Command::Chat { .. }
            | Command::Teleport { .. }
            | Command::QueryLocation
            | Command::Emote { .. }
            | Command::Stance { .. }
            | Command::SetAppearance { .. }
            | Command::Inventory(_)
            | Command::Combat(_)
            | Command::Npc(_)
            | Command::Social(_)
            | Command::World(_)
            | Command::Progression(_) => self.state == S::InWorld,
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
            Command::Connect | Command::Register => S::Connecting,
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
    ///     // The list may be empty (a new account): never index it.
    ///     if let Event::Connected { characters } = event
    ///         && let Some(first) = characters.first()
    ///     {
    ///         let character = first.name.clone();
    ///         session.send(now, Command::EnterWorld { character })?;
    ///     }
    /// }
    /// # Ok::<(), tessera_net::NetError>(())
    /// ```
    ///
    /// Events that make no sense in the current state, or carry non-finite
    /// transforms, invalid placements or invalid appearances
    /// ([`Appearance::is_valid`]; a backend bug), are dropped with a
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
            Event::EntityCreated {
                location,
                heading,
                appearance,
                ..
            } if !location.is_valid() || !heading.is_finite() || !appearance.is_valid() => {
                Verdict::Invalid("invalid created entity")
            }
            Event::AppearanceChanged { appearance, .. } if !appearance.is_valid() => {
                Verdict::Invalid("invalid appearance")
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
            | Event::EntityCreated { .. }
            | Event::AppearanceChanged { .. }
            | Event::StaticSpawned { .. }
            | Event::StaticsCleared
            | Event::EntityDespawned { .. } => self.world_verdict(),
            Event::EnterWorldFailed { .. } => match self.state {
                S::EnteringWorld => {
                    self.state = S::Connected;
                    Verdict::Surface
                }
                _ if closing => Verdict::Stale,
                _ => Verdict::Invalid("unexpected EnterWorldFailed"),
            },
            Event::EntityStateChanged { state, .. } if !state.is_valid() => {
                Verdict::Invalid("invalid entity state")
            }
            Event::WorldClock {
                game_clock, rate, ..
            } if !(game_clock.is_finite()
                && *game_clock >= 0.0
                && rate.is_finite()
                && *rate >= 0.0) =>
            {
                Verdict::Invalid("invalid world clock")
            }
            Event::EntityStateChanged { .. }
            | Event::WorldClock { .. }
            | Event::TeleportRefused { .. }
            | Event::ChatRejected { .. } => self.world_verdict(),
            Event::TimeSynced { .. } | Event::ServerNotice { .. } => match self.state {
                S::Connecting | S::Connected | S::EnteringWorld | S::InWorld => Verdict::Surface,
                S::Closing => Verdict::Stale,
                _ => Verdict::Invalid("session event before connecting"),
            },
            Event::Combat(e) if !e.is_valid() => Verdict::Invalid("non-finite vital or amount"),
            Event::World(e) if !e.is_valid() => Verdict::Invalid("invalid marker location"),
            Event::Character(e) if !e.is_valid() => {
                Verdict::Invalid("invalid look or property value")
            }
            Event::Inventory(e) if !e.is_valid() => Verdict::Invalid("non-finite property value"),
            Event::Social(e) if !e.is_valid() => Verdict::Invalid("non-finite fellow vital"),
            Event::Npc(e) if !e.is_valid() => Verdict::Invalid("invalid quest rewards"),
            Event::Progression(e) if !e.is_valid() => {
                Verdict::Invalid("progression value out of range")
            }
            Event::Character(e) if e.is_entity_event() => self.world_verdict(),
            Event::Inventory(_)
            | Event::Combat(_)
            | Event::Npc(_)
            | Event::Social(_)
            | Event::World(_)
            | Event::Progression(_) => self.world_verdict(),
            Event::ChatMessage { .. } | Event::Character(_) => match self.state {
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
            ..
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
                ChatChannel::Named { name } if !is_valid_name(name) => {
                    return Err(NetError::InvalidCommand(
                        "channel name empty, too long or has control characters",
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
        Command::SetAppearance { appearance } => {
            if !appearance.is_valid() {
                return Err(NetError::InvalidCommand("appearance out of range"));
            }
        }
        Command::Connect
        | Command::Register
        | Command::Disconnect
        | Command::QueryLocation
        | Command::Emote { .. }
        | Command::Stance { .. } => {}
        Command::Character(c) => c.validate().map_err(NetError::InvalidCommand)?,
        Command::Inventory(c) => c.validate().map_err(NetError::InvalidCommand)?,
        Command::Combat(c) => c.validate().map_err(NetError::InvalidCommand)?,
        Command::Npc(c) => c.validate().map_err(NetError::InvalidCommand)?,
        Command::Social(c) => c.validate().map_err(NetError::InvalidCommand)?,
        Command::World(c) => c.validate().map_err(NetError::InvalidCommand)?,
        Command::Progression(c) => c.validate().map_err(NetError::InvalidCommand)?,
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
        Event::WorldEntered { name, .. }
        | Event::EntitySpawned { name, .. }
        | Event::EntityCreated { name, .. } => clean(name, MAX_NAME_LEN),
        Event::ChatMessage {
            channel,
            sender,
            text,
        } => {
            let mut changed = clean(text, MAX_CHAT_LEN);
            if let Some(sender) = sender {
                changed |= clean(sender, MAX_NAME_LEN);
            }
            match channel {
                ChatChannel::Tell { peer } => changed |= clean(peer, MAX_NAME_LEN),
                ChatChannel::Named { name } => changed |= clean(name, MAX_NAME_LEN),
                _ => {}
            }
            changed
        }
        Event::Disconnected {
            reason: DisconnectReason::ServerClosed(why) | DisconnectReason::TransportFailed(why),
        } => clean(why, MAX_CHAT_LEN),
        Event::EnterWorldFailed { character, reason } => {
            clean(character, MAX_NAME_LEN) | clean(reason, MAX_CHAT_LEN)
        }
        Event::TeleportRefused { text, .. }
        | Event::ChatRejected { text, .. }
        | Event::ServerNotice { text, .. } => clean(text, MAX_CHAT_LEN),
        Event::Character(e) => e.clean(),
        Event::Inventory(e) => e.clean(),
        Event::Combat(e) => e.clean(),
        Event::Npc(e) => e.clean(),
        Event::Social(e) => e.clean(),
        Event::World(e) => e.clean(),
        Event::Progression(e) => e.clean(),
        _ => false,
    }
}

/// `Event::name` and `Command::name` give the inner variant's name of a grouped value, so
/// variant names must be unique among all events and among all commands, groups included
/// (`docs/ARCHITECTURE.md` §19.1). The test reads every `name()` of `Event`, `Command` and
/// their group enums from the source: each `match` is exhaustive (the compiler checks
/// that), so every variant has an arm, and a new variant is covered without editing the
/// test.
#[cfg(test)]
mod name_tests {
    use std::collections::{BTreeMap, BTreeSet};

    /// An arm of a `name()` match.
    #[derive(Debug)]
    enum Arm {
        /// `Self::Variant … => "Name",`
        Literal { variant: String, name: String },
        /// `Self::Group(x) => x.name(),`
        Group(String),
    }

    /// Every `name(&self) -> &'static str` in `src`: (impl type, its arms).
    fn name_fns(src: &str) -> Vec<(String, Vec<Arm>)> {
        let mut out = Vec::new();
        let mut current_impl: Option<String> = None;
        let mut lines = src.lines();
        while let Some(line) = lines.next() {
            if let Some(rest) = line.strip_prefix("impl ")
                && let Some(ty) = rest.strip_suffix(" {")
                && ty.chars().all(|c| c.is_ascii_alphanumeric() || c == '_')
            {
                current_impl = Some(ty.to_owned());
                continue;
            }
            if line.trim() != "pub fn name(&self) -> &'static str {" {
                continue;
            }
            let ty = current_impl.clone().expect("name() inside an impl");
            assert_eq!(lines.next().map(str::trim), Some("match self {"), "{ty}");
            let mut arms = Vec::new();
            for arm in lines.by_ref() {
                let arm = arm.trim();
                if arm == "}" {
                    break;
                }
                let (pattern, value) = arm.split_once(" => ").expect("an arm");
                let variant = pattern
                    .strip_prefix("Self::")
                    .expect("a Self:: pattern")
                    .split(|c: char| !(c.is_ascii_alphanumeric() || c == '_'))
                    .next()
                    .expect("a variant")
                    .to_owned();
                if let Some(name) = value.strip_prefix('"').and_then(|v| v.strip_suffix("\",")) {
                    arms.push(Arm::Literal {
                        variant,
                        name: name.to_owned(),
                    });
                } else {
                    assert!(value.ends_with(".name(),"), "{ty}: unexpected arm {arm}");
                    arms.push(Arm::Group(variant));
                }
            }
            assert!(!arms.is_empty(), "{ty}: no arms");
            out.push((ty, arms));
        }
        out
    }

    /// Checks one family (`Event` or `Command`): every literal equals its variant, every
    /// group arm has a group enum `GroupSuffix` with its own `name()`, and the names are
    /// unique across the family.
    fn check_family(fns: &BTreeMap<String, Vec<Arm>>, root: &str) -> usize {
        let top = fns
            .get(root)
            .unwrap_or_else(|| panic!("{root}::name not found"));
        let mut seen: BTreeMap<String, String> = BTreeMap::new();
        let mut add = |ty: &str, arms: &[Arm], groups: &mut BTreeSet<String>| {
            for arm in arms {
                match arm {
                    Arm::Literal { variant, name } => {
                        assert_eq!(variant, name, "{ty}::{variant} is logged as {name}");
                        if let Some(first) = seen.insert(name.clone(), ty.to_owned()) {
                            panic!("{root} name {name} is used by {first} and {ty}");
                        }
                    }
                    Arm::Group(g) => {
                        groups.insert(g.clone());
                    }
                }
            }
        };
        let mut groups = BTreeSet::new();
        add(root, top, &mut groups);
        assert!(!groups.is_empty(), "{root} has groups");
        for g in &groups {
            let ty = format!("{g}{root}");
            let arms = fns
                .get(&ty)
                .unwrap_or_else(|| panic!("{ty}::name not found"));
            let mut nested = BTreeSet::new();
            add(&ty, arms, &mut nested);
            assert!(nested.is_empty(), "{ty}: groups do not nest");
        }
        // Every group enum of the family is reached from the root.
        for ty in fns.keys() {
            if let Some(g) = ty.strip_suffix(root)
                && !g.is_empty()
            {
                assert!(groups.contains(g), "{ty} is not a group of {root}");
            }
        }
        seen.len()
    }

    #[test]
    fn event_and_command_names_are_unique() {
        let mut fns = BTreeMap::new();
        for src in [include_str!("session.rs"), include_str!("domain.rs")] {
            for (ty, arms) in name_fns(src) {
                assert!(fns.insert(ty.clone(), arms).is_none(), "{ty}: two name()s");
            }
        }
        let events = check_family(&fns, "Event");
        let commands = check_family(&fns, "Command");
        // The P0 vocabulary (`scenario-feed.md` §9) is far larger than this.
        assert!(
            events > 50 && commands > 40,
            "{events} events, {commands} commands"
        );
    }

    #[test]
    fn name_reports_the_inner_variant() {
        use crate::domain::{CharacterCommand, CharacterEvent};
        let e = super::Event::Character(CharacterEvent::LevelChanged {
            quiet: false,
            entity: crate::model::EntityId(1),
            level: 2,
        });
        assert_eq!(e.name(), "LevelChanged");
        let c = super::Command::Character(CharacterCommand::DeleteCharacter { name: "A".into() });
        assert_eq!(c.name(), "DeleteCharacter");
        assert_eq!(super::Command::Connect.name(), "Connect");
    }
}
