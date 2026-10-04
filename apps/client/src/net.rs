// SPDX-License-Identifier: GPL-3.0-or-later
//! World session glue: drives a [`Session`] once per frame, logs its events and keeps a
//! tiny entity table for the debug view.

use std::collections::BTreeMap;
use std::time::{Duration, Instant};

use tessera_net::sandbox::{SandboxBackend, SandboxConfig};
use tessera_net::{
    ChatChannel, Command, EntityId, EntityKind, Event, NetError, Session, SessionState, Transform,
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

/// The client's view of the world source.
#[derive(Debug)]
pub struct World {
    session: Session,
    events: Vec<Event>,
    entities: BTreeMap<EntityId, EntityView>,
    player: Option<EntityId>,
    chat: Vec<ChatLine>,
    events_seen: u64,
    moves_seen: u64,
}

impl World {
    /// A session with the in-process sandbox backend, already asked to connect.
    pub fn sandbox(now: Instant) -> Result<Self, NetError> {
        let mut session = Session::new(SandboxBackend::new(SandboxConfig::default())?);
        tracing::info!(backend = session.backend_name(), "world session created");
        session.send(now, Command::Connect)?;
        Ok(Self {
            session,
            events: Vec::new(),
            entities: BTreeMap::new(),
            player: None,
            chat: Vec::new(),
            events_seen: 0,
            moves_seen: 0,
        })
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
            Event::Disconnected { reason } => {
                tracing::info!(?reason, "net: disconnected");
                self.entities.clear();
                self.player = None;
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
}
