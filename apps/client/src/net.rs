// SPDX-License-Identifier: GPL-3.0-or-later
//! World session glue: drives a [`Session`] once per frame, logs its events and keeps a
//! tiny entity table for the debug view. With [`World::capture_entity_events`] it also
//! queues the entity events (created, moved, appearance, despawned, cleared) for the world
//! walker's characters; moves are bounded by [`MAX_QUEUED_ENTITY_EVENTS`] between two
//! drains, and a drain hands out at most the number of events the caller asks for.

use std::collections::{BTreeMap, VecDeque};
use std::time::{Duration, Instant};

use tessera_net::sandbox::world::WorldSource;
use tessera_net::sandbox::{SandboxBackend, SandboxConfig};
use tessera_net::{
    ChatChannel, Command, Destination, EntityId, EntityKind, Event, Location, LocationCause,
    NetError, Session, SessionState, Transform, Vec3,
};

/// How long [`World::shutdown`] waits for the backend to confirm the disconnect.
pub const DISCONNECT_GRACE: Duration = Duration::from_millis(500);
/// Bound on the moves queued for the walker between two [`World::drain_entity_events`]:
/// once the queue holds this many events, every queued move is dropped and further moves
/// are dropped on arrival (the next move of an entity replaces them). Creations,
/// appearance changes and removals are never dropped, so the walker's entity set stays
/// exact; they are bounded by the backend's per-poll budgets (`walker.md` §10.1), not here.
pub const MAX_QUEUED_ENTITY_EVENTS: usize = 1 << 18;

/// A visible entity, as last reported by the backend.
#[derive(Debug, Clone, PartialEq)]
pub struct EntityView {
    /// Classification.
    pub kind: EntityKind,
    /// Display name.
    pub name: String,
    /// Last transform (metres, +Z up).
    pub transform: Transform,
}

/// A chat line for the plugin host.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ChatLine {
    /// Sender, empty for system messages.
    pub sender: String,
    /// Text.
    pub text: String,
}

/// The player's precise position, from the latest location report.
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct PlayerLocation {
    /// Where (region-qualified, f64).
    pub location: Location,
    /// Facing in radians (0 = east, counter-clockwise).
    pub heading: f32,
    /// Why it was reported.
    pub cause: LocationCause,
    /// Increases with every report for the lifetime of the [`World`] (also across a
    /// disconnect), so callers can tell a new report from an old one.
    pub seq: u64,
}

impl crate::walker::WalkerSession for World {
    fn player_location(&self) -> Option<PlayerLocation> {
        World::player_location(self)
    }

    fn send_move(&mut self, now: Instant, location: Location, heading: f32, velocity: Vec3) {
        World::send_move(self, now, location, heading, velocity);
    }

    fn player_id(&self) -> Option<EntityId> {
        self.player
    }

    fn drain_entity_events(&mut self, out: &mut Vec<Event>, max: usize) {
        World::drain_entity_events(self, out, max);
    }
}

/// The client's view of the world source.
#[derive(Debug)]
pub struct World {
    session: Session,
    events: Vec<Event>,
    entities: BTreeMap<EntityId, EntityView>,
    player: Option<EntityId>,
    location: Option<PlayerLocation>,
    /// Sequence number of the last location report; never reset (not even on a
    /// disconnect), so a report after a reconnect is always new to callers.
    location_seq: u64,
    chat: Vec<ChatLine>,
    events_seen: u64,
    moves_seen: u64,
    /// Entities placed by the world data (removed by `StaticsCleared`).
    placed: std::collections::BTreeSet<EntityId>,
    /// Entity events for the walker (`None`: not captured).
    entity_events: Option<VecDeque<Event>>,
    /// Moves in `entity_events`.
    queued_moves: usize,
    /// [`MAX_QUEUED_ENTITY_EVENTS`] (smaller in tests).
    queue_cap: usize,
    /// Moves dropped because the queue was full.
    dropped_moves: u64,
}

impl World {
    /// A session with the in-process sandbox backend, already asked to connect.
    pub fn sandbox(now: Instant) -> Result<Self, NetError> {
        Self::start(now, SandboxBackend::new(SandboxConfig::default())?)
    }

    /// A sandbox session with `config` and no world data, already asked to connect.
    pub fn sandbox_with(now: Instant, config: SandboxConfig) -> Result<Self, NetError> {
        Self::start(now, SandboxBackend::new(config)?)
    }

    /// A sandbox session that resolves places, heights and descriptions through `source`
    /// (the world walker), already asked to connect.
    pub fn sandbox_with_world(
        now: Instant,
        config: SandboxConfig,
        source: impl WorldSource + 'static,
    ) -> Result<Self, NetError> {
        Self::start(now, SandboxBackend::with_world(config, source)?)
    }

    fn start(now: Instant, backend: SandboxBackend) -> Result<Self, NetError> {
        let mut session = Session::new(backend);
        tracing::info!(backend = session.backend_name(), "world session created");
        session.send(now, Command::Connect)?;
        Ok(Self {
            session,
            events: Vec::new(),
            entities: BTreeMap::new(),
            player: None,
            location: None,
            location_seq: 0,
            chat: Vec::new(),
            events_seen: 0,
            moves_seen: 0,
            placed: std::collections::BTreeSet::new(),
            entity_events: None,
            queued_moves: 0,
            queue_cap: MAX_QUEUED_ENTITY_EVENTS,
            dropped_moves: 0,
        })
    }

    /// Starts (or stops) queueing entity events for [`World::drain_entity_events`].
    /// Stopping drops what is queued.
    pub fn capture_entity_events(&mut self, on: bool) {
        self.entity_events = on.then(VecDeque::new);
        self.queued_moves = 0;
    }

    /// Moves at most `max` of the queued entity events into `out` (appended, oldest
    /// first); the rest stay queued, in order, for the next call.
    pub fn drain_entity_events(&mut self, out: &mut Vec<Event>, max: usize) {
        let Some(q) = &mut self.entity_events else {
            return;
        };
        let n = q.len().min(max);
        for ev in q.drain(..n) {
            if matches!(ev, Event::EntityMoved { .. }) {
                self.queued_moves = self.queued_moves.saturating_sub(1);
            }
            out.push(ev);
        }
    }

    /// Entity events waiting for [`World::drain_entity_events`].
    #[must_use]
    pub fn queued_entity_events(&self) -> usize {
        self.entity_events.as_ref().map_or(0, VecDeque::len)
    }

    fn queue_entity_event(&mut self, ev: &Event) {
        let Some(q) = &mut self.entity_events else {
            return;
        };
        let is_move = matches!(ev, Event::EntityMoved { .. });
        if q.len() >= self.queue_cap {
            if self.queued_moves > 0 {
                // Drop every queued move at once (amortised O(1) per event; the
                // entities' next moves replace them).
                let before = q.len();
                q.retain(|e| !matches!(e, Event::EntityMoved { .. }));
                self.dropped_moves += (before - q.len()) as u64;
                self.queued_moves = 0;
                tracing::warn!(
                    dropped = self.dropped_moves,
                    "entity event queue full; queued moves dropped"
                );
            }
            if is_move && q.len() >= self.queue_cap {
                self.dropped_moves += 1;
                return;
            }
        }
        if is_move {
            self.queued_moves += 1;
        }
        q.push_back(ev.clone());
    }

    /// The player's latest precise position.
    #[must_use]
    pub fn player_location(&self) -> Option<PlayerLocation> {
        self.location
    }

    /// Reports the player's movement (ignored before the player is in the world).
    pub fn send_move(&mut self, now: Instant, location: Location, heading: f32, velocity: Vec3) {
        if self.player.is_none() || self.state() != SessionState::InWorld {
            return;
        }
        if let Err(e) = self.session.send(
            now,
            Command::Move {
                location,
                heading,
                velocity,
            },
        ) {
            tracing::debug!("move not sent: {e}");
        }
    }

    /// Asks to be moved to `destination`.
    pub fn teleport(&mut self, now: Instant, destination: Destination) {
        if let Err(e) = self.session.send(now, Command::Teleport { destination }) {
            tracing::warn!("teleport not sent: {e}");
        }
    }

    /// Asks where the player is (answered with a location report and a description).
    pub fn query_location(&mut self, now: Instant) {
        if let Err(e) = self.session.send(now, Command::QueryLocation) {
            tracing::debug!("location query not sent: {e}");
        }
    }

    /// Session state.
    #[must_use]
    pub fn state(&self) -> SessionState {
        self.session.state()
    }

    /// Known entities.
    #[must_use]
    pub fn entities(&self) -> &BTreeMap<EntityId, EntityView> {
        &self.entities
    }

    /// The player entity, once in the world.
    #[must_use]
    pub fn player(&self) -> Option<EntityId> {
        self.player
    }

    /// Total events processed.
    #[must_use]
    pub fn events_seen(&self) -> u64 {
        self.events_seen
    }

    /// Chat lines received since the last call.
    pub fn take_chat(&mut self) -> Vec<ChatLine> {
        std::mem::take(&mut self.chat)
    }

    /// Say something on the local channel.
    pub fn say(&mut self, now: Instant, text: &str) {
        if let Err(e) = self.session.send(
            now,
            Command::Chat {
                channel: ChatChannel::Say,
                text: text.to_owned(),
            },
        ) {
            tracing::warn!("chat not sent: {e}");
        }
    }

    /// Poll the backend and apply its events. Call once per frame.
    pub fn update(&mut self, now: Instant) {
        self.session.poll_into(now, &mut self.events);
        let events = std::mem::take(&mut self.events);
        for ev in &events {
            self.apply(now, ev);
        }
        self.events = events;
        self.events.clear();
    }

    fn apply(&mut self, now: Instant, ev: &Event) {
        self.events_seen += 1;
        if matches!(
            ev,
            Event::EntityCreated { .. }
                | Event::EntityMoved { .. }
                | Event::EntityDespawned { .. }
                | Event::AppearanceChanged { .. }
                | Event::StaticsCleared
                | Event::Disconnected { .. }
        ) {
            self.queue_entity_event(ev);
        }
        match ev {
            Event::Connected { characters } => {
                let names: Vec<&str> = characters.iter().map(|c| c.name.as_str()).collect();
                tracing::info!(characters = %names.join(","), "net: connected");
                if let Some(first) = characters.first() {
                    let character = first.name.clone();
                    if let Err(e) = self.session.send(now, Command::EnterWorld { character }) {
                        tracing::warn!("net: enter world failed: {e}");
                    }
                }
            }
            Event::WorldEntered {
                player,
                name,
                transform,
            } => {
                tracing::info!(player = player.0, %name, pos = ?transform.position, "net: world entered");
                self.player = Some(*player);
                self.entities.insert(
                    *player,
                    EntityView {
                        kind: EntityKind::Player,
                        name: name.clone(),
                        transform: *transform,
                    },
                );
            }
            Event::EntitySpawned {
                id,
                kind,
                name,
                transform,
            } => {
                tracing::info!(id = id.0, ?kind, %name, pos = ?transform.position, "net: entity spawned");
                self.entities.insert(
                    *id,
                    EntityView {
                        kind: *kind,
                        name: name.clone(),
                        transform: *transform,
                    },
                );
            }
            Event::EntityCreated {
                id,
                kind,
                name,
                location,
                heading,
                tag,
                ..
            } => {
                if tag.is_none() {
                    tracing::info!(id = id.0, ?kind, %name, "net: character created");
                } else {
                    tracing::trace!(id = id.0, ?kind, "net: placed figure created");
                    self.placed.insert(*id);
                }
                self.entities.insert(
                    *id,
                    EntityView {
                        kind: *kind,
                        name: name.clone(),
                        transform: Transform::new(location.position(), *heading),
                    },
                );
            }
            Event::AppearanceChanged { id, appearance } => {
                tracing::debug!(id = id.0, body = ?appearance.body, "net: appearance");
            }
            Event::EntityMoved { id, transform, .. } => {
                self.moves_seen += 1;
                // Movement is chatty; one in 64 at debug, the rest at trace.
                if self.moves_seen % 64 == 1 {
                    tracing::debug!(id = id.0, pos = ?transform.position, moves = self.moves_seen, "net: entity moved");
                } else {
                    tracing::trace!(id = id.0, pos = ?transform.position, "net: entity moved");
                }
                if let Some(e) = self.entities.get_mut(id) {
                    e.transform = *transform;
                }
            }
            Event::EntityDespawned { id } => {
                tracing::debug!(id = id.0, "net: entity despawned");
                self.entities.remove(id);
                self.placed.remove(id);
            }
            Event::ChatMessage {
                channel,
                sender,
                text,
            } => {
                tracing::info!(
                    ?channel,
                    sender = sender.as_deref().unwrap_or("-"),
                    "net chat: {text}"
                );
                self.chat.push(ChatLine {
                    sender: sender.clone().unwrap_or_default(),
                    text: text.clone(),
                });
            }
            Event::LocationReport {
                id,
                location,
                heading,
                cause,
                ..
            } => {
                if Some(*id) == self.player {
                    tracing::info!(
                        ?cause,
                        region = location.region.0,
                        x = location.x,
                        y = location.y,
                        z = location.z,
                        "net: player location"
                    );
                    self.location_seq += 1;
                    self.location = Some(PlayerLocation {
                        location: *location,
                        heading: *heading,
                        cause: *cause,
                        seq: self.location_seq,
                    });
                }
            }
            Event::StaticSpawned { .. } => {}
            Event::StaticsCleared => {
                for id in std::mem::take(&mut self.placed) {
                    self.entities.remove(&id);
                }
            }
            Event::Disconnected { reason } => {
                tracing::info!(?reason, "net: disconnected");
                self.entities.clear();
                self.placed.clear();
                self.player = None;
                self.location = None;
            }
            other => tracing::debug!(?other, "net: unhandled event"),
        }
    }

    /// Disconnect gracefully, polling for at most [`DISCONNECT_GRACE`].
    pub fn shutdown(&mut self) {
        let start = Instant::now();
        if matches!(self.state(), SessionState::Closed) {
            return;
        }
        if let Err(e) = self.session.send(start, Command::Disconnect) {
            tracing::debug!("net: disconnect not sent: {e}");
            return;
        }
        let mut now = start;
        while !matches!(self.state(), SessionState::Closed) && now - start < DISCONNECT_GRACE {
            self.update(now);
            std::thread::sleep(Duration::from_millis(5));
            now = Instant::now();
        }
        tracing::info!(state = ?self.state(), events = self.events_seen, "net: session shut down");
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn sandbox_enters_world_and_shuts_down() {
        let mut now = Instant::now();
        let mut world = World::sandbox(now).expect("sandbox");
        for _ in 0..30 {
            world.update(now);
            now += Duration::from_millis(16);
        }
        assert_eq!(world.state(), SessionState::InWorld);
        assert!(world.player().is_some());
        assert!(!world.entities().is_empty());
        world.say(now, "hello");
        for _ in 0..5 {
            world.update(now);
            now += Duration::from_millis(16);
        }
        assert!(world.take_chat().iter().any(|c| c.text == "hello"));
        world.shutdown();
        assert_eq!(world.state(), SessionState::Closed);
    }

    #[test]
    fn entity_events_are_captured_for_the_walker() {
        let avatar =
            tessera_net::Appearance::new(tessera_net::Body::Character { species: 1, sex: 2 });
        let config = SandboxConfig {
            avatar: Some(avatar.clone()),
            wanderers: 2,
            wanderer_appearances: vec![avatar],
            ..SandboxConfig::default()
        };
        let mut now = Instant::now();
        let mut world = World::sandbox_with(now, config).expect("sandbox");
        world.capture_entity_events(true);
        for _ in 0..20 {
            world.update(now);
            now += Duration::from_millis(16);
        }
        let mut out = Vec::new();
        world.drain_entity_events(&mut out, usize::MAX);
        let count = |f: &dyn Fn(&Event) -> bool| out.iter().filter(|e| f(e)).count();
        assert_eq!(count(&|e| matches!(e, Event::EntityCreated { .. })), 2);
        assert_eq!(count(&|e| matches!(e, Event::AppearanceChanged { .. })), 1);
        assert!(count(&|e| matches!(e, Event::EntityMoved { .. })) > 0);
        assert_eq!(world.entities().len(), 3, "player and two wanderers");
        let mut again = Vec::new();
        world.drain_entity_events(&mut again, usize::MAX);
        assert!(again.is_empty(), "drained");
        world.capture_entity_events(false);
        for _ in 0..10 {
            world.update(now);
            now += Duration::from_millis(16);
        }
        world.drain_entity_events(&mut again, usize::MAX);
        assert!(again.is_empty(), "not captured");
    }

    fn moved(id: u64) -> Event {
        Event::EntityMoved {
            id: EntityId(id),
            transform: Transform::default(),
            velocity: Vec3::default(),
        }
    }

    fn despawned(id: u64) -> Event {
        Event::EntityDespawned { id: EntityId(id) }
    }

    #[test]
    fn entity_event_queue_is_bounded_and_drained_in_slices() {
        let mut world = World::sandbox(Instant::now()).expect("sandbox");
        world.capture_entity_events(true);
        world.queue_cap = 8;
        for i in 0..6 {
            world.queue_entity_event(&moved(i));
        }
        world.queue_entity_event(&despawned(100));
        world.queue_entity_event(&moved(6));
        assert_eq!(world.queued_entity_events(), 8);
        // Full: the queued moves go at once, the removal stays, the new move is queued.
        world.queue_entity_event(&moved(7));
        assert_eq!(world.queued_entity_events(), 2);
        assert_eq!(world.dropped_moves, 7);
        // Non-moves past the cap are never dropped.
        for i in 0..10 {
            world.queue_entity_event(&despawned(200 + i));
        }
        assert_eq!(
            world.queued_entity_events(),
            11,
            "move 7 dropped, removals kept"
        );
        world.queue_entity_event(&moved(8));
        assert_eq!(
            world.queued_entity_events(),
            11,
            "a move past the cap is dropped"
        );
        assert_eq!(world.queued_moves, 0);
        // Slices keep the order.
        let mut out = Vec::new();
        world.drain_entity_events(&mut out, 4);
        assert_eq!(out.len(), 4);
        assert_eq!(out[0], despawned(100));
        assert_eq!(out[1], despawned(200));
        world.drain_entity_events(&mut out, usize::MAX);
        assert_eq!(out.len(), 11);
        assert_eq!(out[10], despawned(209));
        assert_eq!(world.queued_entity_events(), 0);
    }

    fn report(id: EntityId, x: f64, cause: LocationCause) -> Event {
        Event::LocationReport {
            id,
            location: Location::new(tessera_net::RegionId(1), x, 20.0, 0.0),
            heading: 0.0,
            anchor: None,
            cause,
        }
    }

    #[test]
    fn location_seq_survives_a_reconnect() {
        let now = Instant::now();
        let mut world = World::sandbox(now).expect("sandbox");
        let id = EntityId(7);
        world.player = Some(id);
        world.apply(now, &report(id, 10.0, LocationCause::Entered));
        let first = world.player_location().expect("location");
        assert_eq!(first.seq, 1);
        world.apply(
            now,
            &Event::Disconnected {
                reason: tessera_net::DisconnectReason::ClientRequested,
            },
        );
        assert!(world.player_location().is_none());
        world.player = Some(id);
        world.apply(now, &report(id, 30.0, LocationCause::Entered));
        let second = world.player_location().expect("location");
        assert!(second.seq > first.seq, "{second:?} after {first:?}");
        assert_eq!(second.location.x, 30.0);
    }

    #[test]
    fn implausible_move_is_corrected() {
        let mut now = Instant::now();
        let mut world = World::sandbox(now).expect("sandbox");
        for _ in 0..30 {
            world.update(now);
            now += Duration::from_millis(16);
        }
        let before = world.player_location().expect("entered");
        let far = Location::new(
            before.location.region,
            before.location.x + 1.0e6,
            before.location.y,
            before.location.z,
        );
        world.send_move(now, far, 0.0, Vec3::ZERO);
        for _ in 0..5 {
            now += Duration::from_millis(16);
            world.update(now);
        }
        let after = world.player_location().expect("location");
        assert!(after.seq > before.seq);
        assert_eq!(after.cause, LocationCause::Corrected);
        assert!((after.location.x - before.location.x).abs() < 1.0);
    }
}
