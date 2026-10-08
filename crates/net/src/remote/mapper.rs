// SPDX-License-Identifier: GPL-3.0-or-later
//! The main-thread side of the test-server backend: turns [`Inbound`] items into domain
//! [`Event`]s (`tessera-protocol.md` Appendix A), keeps the entity table, the replicated
//! properties and the interpolation buffers, tracks the player's position, and streams the
//! statics of the client's own data around it.

use std::collections::BTreeMap;

use tessera_protocol::limits::{DEFAULT_RUN_SPEED, DEFAULT_WALK_SPEED};
use tessera_protocol::message::{ChatChannel as WireChannel, EntityCreate, LocationCause as Cause};
use tessera_protocol::types::{Motion, reduce_heading};
use tessera_protocol::value::{Emote, Property, Stance, Value, pid, speed_from_property};

use super::convert;
use super::driver::Inbound;
use super::stats::{RemoteFailure, ServerInfo};
use crate::interp::{InterpConfig, InterpolationSet, Snapshot};
use crate::model::{
    Anchor, CharacterSummary, ChatChannel, DataId, EntityId, Landblock, Location, LocationCause,
    Transform, Vec3,
};
use crate::sandbox::world::{StaticConfig, StaticStats, Statics, WorldSource};
use crate::session::Event;

/// Entity IDs the server assigns are used as [`EntityId`]s unchanged when they are below
/// this value. IDs from here up are local: statics streamed from the client's own data,
/// and (remapped) any server ID that would collide with them.
pub const LOCAL_ID_BASE: u64 = 1 << 63;

/// The replicated state of an entity: the protocol v1 properties (§7.3). Unknown property
/// IDs are skipped, as §7.1 requires, so this stays small whatever the server sends.
#[derive(Debug, Clone, Copy, PartialEq)]
#[non_exhaustive]
pub struct EntityState {
    /// `STANCE` (default Relaxed).
    pub stance: Stance,
    /// `EMOTE`, when one is playing. When the struct has no `started` field, §7.3 says it
    /// defaults to the message's `server_time`; `EntityUpdate` has none, so the backend
    /// uses the newest server time it has seen in any message (world, chat or
    /// `MotionBatch`) at that point (an open spec question, `tessera-protocol.md` §17.2).
    pub emote: Option<Emote>,
    /// `WALK_SPEED` in m/s (owner only; default 2).
    pub walk_speed: f32,
    /// `RUN_SPEED` in m/s (owner only; default 6).
    pub run_speed: f32,
    /// Motion byte of the newest movement state (§8.1).
    pub motion: Motion,
}

impl Default for EntityState {
    fn default() -> Self {
        Self {
            stance: Stance::Relaxed,
            emote: None,
            walk_speed: DEFAULT_WALK_SPEED,
            run_speed: DEFAULT_RUN_SPEED,
            motion: Motion::STANDING,
        }
    }
}

impl EntityState {
    fn apply(&mut self, props: &[Property], message_time: u64) {
        for p in props {
            let v = (!matches!(p.value, Value::None)).then_some(&p.value);
            match p.id {
                pid::STANCE => self.stance = Stance::from_property(v),
                pid::EMOTE => self.emote = Emote::from_property(v, message_time),
                pid::WALK_SPEED => self.walk_speed = speed_from_property(v, DEFAULT_WALK_SPEED),
                pid::RUN_SPEED => self.run_speed = speed_from_property(v, DEFAULT_RUN_SPEED),
                _ => {}
            }
        }
    }
}

#[derive(Debug)]
struct Tracked {
    local: EntityId,
    state: EntityState,
}

/// The main thread's view of one connection.
#[derive(Debug)]
pub(crate) struct Mapper {
    player_server: Option<u64>,
    player: Option<EntityId>,
    player_state: EntityState,
    location: Option<(Location, f32, Option<Anchor>)>,
    /// The epoch of the latest WorldEnter / Teleported / Corrected the main thread has
    /// surfaced; moves are stamped with it.
    pub(crate) epoch: u32,
    entities: BTreeMap<u64, Tracked>,
    /// Local ID → server ID, for remapped (high) server IDs only.
    remapped: BTreeMap<EntityId, u64>,
    next_local: u64,
    pub(crate) interp: InterpolationSet,
    pub(crate) world_clock: Option<tessera_protocol::message::WorldClock>,
    /// Newest server time seen in any message, µs.
    pub(crate) last_server_time: Option<u64>,
    statics: Statics,
    source: Option<Box<dyn WorldSource>>,
    pub(crate) failure: Option<RemoteFailure>,
    pub(crate) server: Option<ServerInfo>,
    pub(crate) ended: bool,
    scratch: Vec<Event>,
}

impl Mapper {
    pub(crate) fn new(
        statics: StaticConfig,
        interp: InterpConfig,
        source: Option<Box<dyn WorldSource>>,
    ) -> Self {
        Self {
            player_server: None,
            player: None,
            player_state: EntityState::default(),
            location: None,
            epoch: 0,
            entities: BTreeMap::new(),
            remapped: BTreeMap::new(),
            next_local: LOCAL_ID_BASE,
            interp: InterpolationSet::new(interp),
            world_clock: None,
            last_server_time: None,
            statics: Statics::new(statics),
            source,
            failure: None,
            server: None,
            ended: false,
            scratch: Vec::new(),
        }
    }

    #[cfg(test)]
    pub(crate) fn player(&self) -> Option<EntityId> {
        self.player
    }

    pub(crate) fn player_state(&self) -> &EntityState {
        &self.player_state
    }

    pub(crate) fn source(&self) -> Option<&dyn WorldSource> {
        self.source.as_deref()
    }

    pub(crate) fn static_stats(&self) -> StaticStats {
        self.statics.stats()
    }

    pub(crate) fn entity_count(&self) -> usize {
        self.entities.len()
    }

    /// The replicated state of `id` (the player included).
    pub(crate) fn state(&self, id: EntityId) -> Option<&EntityState> {
        if Some(id) == self.player {
            return Some(&self.player_state);
        }
        let server = self.server_id(id)?;
        self.entities.get(&server).map(|t| &t.state)
    }

    /// The server's ID for a local entity ID (identity below [`LOCAL_ID_BASE`]).
    pub(crate) fn server_id(&self, id: EntityId) -> Option<u64> {
        if id.0 < LOCAL_ID_BASE {
            Some(id.0)
        } else if Some(id) == self.player {
            self.player_server
        } else {
            self.remapped.get(&id).copied()
        }
    }

    /// The server's ID for a local entity ID that is currently known: the player or a
    /// created remote entity. `None` for statics, destroyed and never-created IDs.
    pub(crate) fn known_server_id(&self, id: EntityId) -> Option<u64> {
        if Some(id) == self.player {
            return self.player_server;
        }
        let server = self.server_id(id)?;
        self.entities
            .get(&server)
            .is_some_and(|t| t.local == id)
            .then_some(server)
    }

    /// The local ID of a server entity ID currently created (or the player).
    pub(crate) fn local_id(&self, server: u64) -> Option<EntityId> {
        if Some(server) == self.player_server {
            return self.player;
        }
        self.entities.get(&server).map(|t| t.local)
    }

    fn alloc(next: &mut u64) -> EntityId {
        let id = EntityId(*next);
        *next = next.saturating_add(1);
        id
    }

    fn map_id(&mut self, server: u64) -> EntityId {
        if server < LOCAL_ID_BASE {
            EntityId(server)
        } else {
            let id = Self::alloc(&mut self.next_local);
            self.remapped.insert(id, server);
            id
        }
    }

    /// The client moved the player (an accepted `Command::Move`): keep the region, drop an
    /// anchor that no longer owns the position (as the sandbox and `Session` do).
    pub(crate) fn player_moved(&mut self, to: Location, heading: f32) {
        if let Some((loc, h, anchor)) = &mut self.location {
            *loc = Location {
                region: loc.region,
                ..to
            };
            *h = heading;
            if anchor.is_some_and(|a| !a.owns(loc)) {
                *anchor = None;
            }
        }
    }

    fn note_time(&mut self, t: u64) {
        self.last_server_time = Some(self.last_server_time.map_or(t, |l| l.max(t)));
    }

    /// Maps one item; events go to `out`.
    pub(crate) fn map(&mut self, item: Inbound, out: &mut Vec<Event>) {
        if self.ended {
            return;
        }
        match item {
            Inbound::Greeted(info) => {
                // §8.6: D ≥ 2 / snapshot_rate + 1 / tick_rate, with the history raised
                // along with it so the bracket a render time needs is never pruned.
                let cfg = self
                    .interp
                    .config()
                    .for_server_rates(info.snapshot_rate, info.tick_rate);
                self.interp.set_config(cfg);
                self.server = Some(info);
            }
            Inbound::Connected { characters } => out.push(Event::Connected {
                characters: characters
                    .into_iter()
                    .map(|name| CharacterSummary { name })
                    .collect(),
            }),
            Inbound::WorldEntered(w) => {
                let (location, heading, anchor) = convert::pose(&w.pose);
                let player = self.map_id(w.player);
                self.player_server = Some(w.player);
                self.player = Some(player);
                self.location = Some((location, heading, anchor));
                self.epoch = w.epoch;
                self.note_time(w.server_time);
                out.push(Event::WorldEntered {
                    player,
                    name: w.name,
                    transform: convert::transform(&location, heading),
                });
                out.push(Event::AppearanceChanged {
                    id: player,
                    appearance: Box::new(convert::appearance(&w.appearance)),
                });
                out.push(Event::LocationReport {
                    id: player,
                    location,
                    heading,
                    anchor,
                    cause: LocationCause::Entered,
                });
            }
            Inbound::Location(r) => {
                let Some(player) = self.player else {
                    return;
                };
                self.note_time(r.server_time);
                let (location, heading, anchor) = convert::pose(&r.pose);
                let report = |cause| Event::LocationReport {
                    id: player,
                    location,
                    heading,
                    anchor,
                    cause,
                };
                let moved = Event::EntityMoved {
                    id: player,
                    transform: convert::transform(&location, heading),
                    velocity: Vec3::ZERO,
                };
                match r.cause {
                    Cause::Teleported => {
                        self.epoch = r.epoch;
                        self.location = Some((location, heading, anchor));
                        out.push(report(LocationCause::Teleported));
                        out.push(moved);
                    }
                    Cause::Corrected => {
                        self.epoch = r.epoch;
                        self.location = Some((location, heading, anchor));
                        out.push(moved);
                        out.push(report(LocationCause::Corrected));
                    }
                    Cause::Requested => out.push(report(LocationCause::Requested)),
                }
            }
            Inbound::System(text) => out.push(Event::ChatMessage {
                channel: ChatChannel::System,
                sender: None,
                text,
            }),
            Inbound::Created(c) => self.created(*c, out),
            Inbound::Updated(u) => {
                let t = self.last_server_time.unwrap_or(0);
                if Some(u.id) == self.player_server {
                    self.player_state.apply(&u.changes, t);
                } else if let Some(e) = self.entities.get_mut(&u.id) {
                    e.state.apply(&u.changes, t);
                }
            }
            Inbound::Appearance(a) => {
                if let Some(id) = self.local_id(a.id) {
                    out.push(Event::AppearanceChanged {
                        id,
                        appearance: Box::new(convert::appearance(&a.appearance)),
                    });
                }
            }
            Inbound::Destroyed(server) => self.destroyed(server, out),
            Inbound::Reset(ids) => {
                for server in ids {
                    self.destroyed(server, out);
                }
            }
            Inbound::Clock(c) => {
                self.note_time(c.server_time);
                self.world_clock = Some(c);
            }
            Inbound::Chat(m) => {
                self.note_time(m.server_time);
                let system = matches!(m.channel, WireChannel::System);
                out.push(Event::ChatMessage {
                    channel: convert::chat_channel(&m.channel),
                    sender: (!system).then_some(m.sender),
                    text: m.text,
                });
            }
            Inbound::Motion {
                server_time,
                entries,
            } => {
                self.note_time(server_time);
                for e in entries {
                    let Some(tracked) = self.entities.get_mut(&e.id) else {
                        continue;
                    };
                    let id = tracked.local;
                    tracked.state.motion = e.motion;
                    let position = convert::vec3(&e.position);
                    let velocity = convert::vec3(&e.velocity);
                    let heading = reduce_heading(e.heading);
                    self.interp.push(
                        id,
                        Snapshot {
                            server_time,
                            position,
                            heading,
                            velocity,
                            motion: e.motion,
                        },
                    );
                    out.push(Event::EntityMoved {
                        id,
                        transform: Transform::new(position, heading),
                        velocity,
                    });
                }
            }
            Inbound::Closed { reason, failure } => {
                self.end(failure);
                out.push(Event::Disconnected { reason });
            }
        }
    }

    fn created(&mut self, c: EntityCreate, out: &mut Vec<Event>) {
        if self.entities.contains_key(&c.id) || Some(c.id) == self.player_server {
            return;
        }
        self.note_time(c.server_time);
        let id = self.map_id(c.id);
        let (location, heading, _) = convert::pose(&c.pose);
        let velocity = convert::vec3(&c.velocity);
        let mut state = EntityState {
            motion: c.motion,
            ..EntityState::default()
        };
        state.apply(&c.properties, c.server_time);
        self.entities.insert(c.id, Tracked { local: id, state });
        self.interp.reset(
            id,
            Snapshot {
                server_time: c.server_time,
                position: location.position(),
                heading,
                velocity,
                motion: c.motion,
            },
        );
        out.push(Event::EntityCreated {
            id,
            kind: convert::kind(c.kind),
            name: c.name,
            class: c.class.map(DataId),
            location,
            heading,
            appearance: Box::new(convert::appearance(&c.appearance)),
            tag: None,
        });
        if velocity != Vec3::ZERO {
            out.push(Event::EntityMoved {
                id,
                transform: convert::transform(&location, heading),
                velocity,
            });
        }
    }

    fn destroyed(&mut self, server: u64, out: &mut Vec<Event>) {
        if let Some(t) = self.entities.remove(&server) {
            self.remapped.remove(&t.local);
            self.interp.remove(t.local);
            out.push(Event::EntityDespawned { id: t.local });
        }
    }

    /// The session is over: forget everything and release the source's blocks.
    pub(crate) fn end(&mut self, failure: Option<RemoteFailure>) {
        if self.ended {
            return;
        }
        self.ended = true;
        self.failure = failure;
        self.entities.clear();
        self.remapped.clear();
        self.interp.clear();
        if let Some(source) = self.source.as_mut() {
            self.statics.clear(source.as_mut());
        }
    }

    /// One statics streaming step around the player (no-op without a source, before the
    /// world was entered and after the end).
    pub(crate) fn update_statics(&mut self, out: &mut Vec<Event>) {
        if self.ended {
            return;
        }
        let (Some(source), Some((location, _, anchor))) = (self.source.as_mut(), self.location)
        else {
            return;
        };
        let centre = anchor
            .map(|a| a.block)
            .or_else(|| Landblock::containing(&location));
        let mut events = std::mem::take(&mut self.scratch);
        let next = &mut self.next_local;
        let mut alloc = || Self::alloc(next);
        self.statics
            .update(centre, source.as_mut(), &mut alloc, &mut events);
        out.append(&mut events);
        self.scratch = events;
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::model::DisconnectReason;
    use tessera_protocol::message::{EntityKind, EntityUpdate, WorldEnter};
    use tessera_protocol::types::{Appearance, Body, Location as WLoc, Pose, Vec3 as WVec};

    fn world_enter(player: u64) -> Inbound {
        Inbound::WorldEntered(Box::new(WorldEnter {
            player,
            name: "Aldith".into(),
            instance: 1,
            epoch: 1,
            server_time: 5_000_000,
            pose: Pose {
                location: WLoc::new(1, 20_250.5, 22_500.25, 401.0),
                heading: 0.0,
                anchor: None,
            },
            appearance: Appearance::new(Body::Character {
                species: 23,
                sex: 1,
            }),
        }))
    }

    fn create(id: u64) -> Inbound {
        Inbound::Created(Box::new(EntityCreate {
            id,
            kind: EntityKind::Player,
            name: "Bram".into(),
            class: None,
            server_time: 6_000_000,
            pose: Pose {
                location: WLoc::new(1, 20_260.0, 22_500.0, 400.5),
                heading: 3.0,
                anchor: None,
            },
            velocity: WVec {
                x: 2.0,
                y: 0.0,
                z: 0.0,
            },
            motion: Motion::STANDING,
            appearance: Appearance::new(Body::Character {
                species: 81,
                sex: 2,
            }),
            properties: vec![Property::new(pid::STANCE, Value::Enum(2))],
        }))
    }

    #[test]
    fn world_entry_and_entities() {
        let mut m = Mapper::new(StaticConfig::default(), InterpConfig::default(), None);
        let mut out = Vec::new();
        m.map(world_enter(42), &mut out);
        assert!(matches!(
            out[0],
            Event::WorldEntered {
                player: EntityId(42),
                ..
            }
        ));
        assert!(matches!(out[1], Event::AppearanceChanged { .. }));
        assert!(matches!(
            out[2],
            Event::LocationReport {
                cause: LocationCause::Entered,
                ..
            }
        ));
        out.clear();
        m.map(create(43), &mut out);
        assert!(matches!(
            out[0],
            Event::EntityCreated {
                id: EntityId(43),
                ..
            }
        ));
        assert!(
            matches!(out[1], Event::EntityMoved { .. }),
            "moving on creation"
        );
        assert_eq!(m.state(EntityId(43)).unwrap().stance, Stance::Seated);
        m.map(
            Inbound::Updated(EntityUpdate {
                id: 43,
                changes: vec![Property::new(pid::STANCE, Value::None)],
            }),
            &mut out,
        );
        assert_eq!(m.state(EntityId(43)).unwrap().stance, Stance::Relaxed);
        out.clear();
        m.map(Inbound::Reset(vec![43]), &mut out);
        assert_eq!(out, vec![Event::EntityDespawned { id: EntityId(43) }]);
        assert!(m.interp.is_empty());
    }

    #[test]
    fn high_ids_are_remapped() {
        let mut m = Mapper::new(StaticConfig::default(), InterpConfig::default(), None);
        let mut out = Vec::new();
        m.map(world_enter(u64::MAX), &mut out);
        let player = m.player().unwrap();
        assert!(player.0 >= LOCAL_ID_BASE);
        assert_eq!(m.server_id(player), Some(u64::MAX));
        m.map(create(u64::MAX - 1), &mut out);
        let id = m.local_id(u64::MAX - 1).unwrap();
        assert_ne!(id, player);
        assert_eq!(m.server_id(id), Some(u64::MAX - 1));
        m.map(Inbound::Destroyed(u64::MAX - 1), &mut out);
        assert_eq!(m.server_id(id), None);
    }

    fn greeted(snapshot_rate: u8) -> Inbound {
        Inbound::Greeted(ServerInfo {
            minor: 0,
            caps: tessera_protocol::message::ServerCaps::NONE,
            tick_rate: 30,
            snapshot_rate,
            interest_radius: 2,
            name: "t".into(),
            motd: String::new(),
        })
    }

    #[test]
    fn low_snapshot_rate_still_interpolates() {
        // Finding: HelloAck with snapshot_rate 1 raised the delay to 2.034 s but kept 1 s of
        // history, so every sample was Early (frozen, stepping once per snapshot).
        let mut m = Mapper::new(StaticConfig::default(), InterpConfig::default(), None);
        let mut out = Vec::new();
        m.map(greeted(1), &mut out);
        let cfg = *m.interp.config();
        assert!(cfg.validate().is_ok(), "{cfg:?}");
        assert_eq!(cfg.delay, crate::interp::recommended_delay(1, 30));
        m.map(world_enter(42), &mut out);
        m.map(create(43), &mut out);
        let id = EntityId(43);
        let mut interpolated = 0;
        for i in 1..=20u64 {
            let t = 6_000_000 + i * 1_000_000;
            m.map(
                Inbound::Motion {
                    server_time: t,
                    entries: vec![tessera_protocol::message::MotionEntry {
                        id: 43,
                        position: WVec {
                            x: 20_260.0 + i as f32,
                            y: 22_500.0,
                            z: 400.5,
                        },
                        heading: 0.0,
                        velocity: WVec {
                            x: 1.0,
                            y: 0.0,
                            z: 0.0,
                        },
                        motion: Motion::STANDING,
                    }],
                },
                &mut out,
            );
            if i < 4 {
                continue;
            }
            for k in 0..10u64 {
                let now = t + k * 100_000;
                let s = m
                    .interp
                    .sample(id, crate::interp::render_time(now, cfg.delay))
                    .unwrap();
                assert_ne!(s.kind, crate::interp::SampleKind::Early, "t={now}");
                interpolated += usize::from(s.kind == crate::interp::SampleKind::Interpolated);
            }
        }
        assert!(interpolated > 100, "{interpolated}");
    }

    #[test]
    fn server_ids_only_for_known_entities() {
        let mut m = Mapper::new(StaticConfig::default(), InterpConfig::default(), None);
        let mut out = Vec::new();
        assert_eq!(m.known_server_id(EntityId(999)), None);
        m.map(world_enter(42), &mut out);
        assert_eq!(m.known_server_id(EntityId(42)), Some(42));
        assert_eq!(m.known_server_id(EntityId(43)), None);
        m.map(create(43), &mut out);
        assert_eq!(m.known_server_id(EntityId(43)), Some(43));
        m.map(Inbound::Destroyed(43), &mut out);
        assert_eq!(m.known_server_id(EntityId(43)), None);
        assert_eq!(m.local_id(43), None);
    }

    #[test]
    fn correction_moves_then_reports() {
        let mut m = Mapper::new(StaticConfig::default(), InterpConfig::default(), None);
        let mut out = Vec::new();
        m.map(world_enter(42), &mut out);
        out.clear();
        m.map(
            Inbound::Location(Box::new(tessera_protocol::message::LocationReport {
                cause: Cause::Corrected,
                request_id: 0,
                epoch: 2,
                server_time: 7_000_000,
                pose: Pose {
                    location: WLoc::new(1, 20_240.0, 22_500.0, 401.0),
                    heading: 1.0,
                    anchor: None,
                },
            })),
            &mut out,
        );
        assert_eq!(m.epoch, 2);
        assert!(matches!(
            out[0],
            Event::EntityMoved {
                id: EntityId(42),
                ..
            }
        ));
        assert!(matches!(
            out[1],
            Event::LocationReport {
                id: EntityId(42),
                cause: LocationCause::Corrected,
                ..
            }
        ));
        assert_eq!(out.len(), 2);
    }

    #[test]
    fn closed_ends_once() {
        let mut m = Mapper::new(StaticConfig::default(), InterpConfig::default(), None);
        let mut out = Vec::new();
        let closed = || Inbound::Closed {
            reason: DisconnectReason::TimedOut,
            failure: None,
        };
        m.map(closed(), &mut out);
        m.map(closed(), &mut out);
        assert_eq!(out.len(), 1);
    }
}
