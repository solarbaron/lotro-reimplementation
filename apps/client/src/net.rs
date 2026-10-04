// SPDX-License-Identifier: GPL-3.0-or-later
//! World session glue: drives a [`Session`] once per frame, logs its events and keeps a
//! tiny entity table for the debug view.

use std::collections::BTreeMap;
use std::time::{Duration, Instant};

use tessera_net::sandbox::world::WorldSource;
use tessera_net::sandbox::{SandboxBackend, SandboxConfig};
use tessera_net::{
    ChatChannel, Command, Destination, EntityId, EntityKind, Event, Location, LocationCause,
    NetError, Session, SessionState, Transform, Vec3,
};

/// How long [`World::shutdown`] waits for the backend to confirm the disconnect.
pub const DISCONNECT_GRACE: Duration = Duration::from_millis(500);

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
}

impl World {
    /// A session with the in-process sandbox backend, already asked to connect.
    pub fn sandbox(now: Instant) -> Result<Self, NetError> {
        Self::start(now, SandboxBackend::new(SandboxConfig::default())?)
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
        })
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
                tracing::info!(id = id.0, "net: entity despawned");
                self.entities.remove(id);
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
            Event::StaticSpawned { .. } | Event::StaticsCleared => {}
            Event::Disconnected { reason } => {
                tracing::info!(?reason, "net: disconnected");
                self.entities.clear();
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
