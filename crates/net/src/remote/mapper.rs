// SPDX-License-Identifier: GPL-3.0-or-later
//! The main-thread side of the test-server backend: turns [`Inbound`] items into domain
//! [`Event`]s (`tessera-protocol.md` Appendix A), keeps the entity table, the replicated
//! properties and the interpolation buffers, tracks the player's position, and streams the
//! statics of the client's own data around it.

use std::collections::{BTreeMap, BTreeSet};

use tessera_protocol::codes::{
    CharacterCreateFailedCode, ChatRejectedCode, EnterWorldFailedCode, TeleportRefusedCode,
};
use tessera_protocol::limits::{DEFAULT_RUN_SPEED, DEFAULT_WALK_SPEED};
use tessera_protocol::message::{
    ChatChannel as WireChannel, EntityCreate, LocationCause as Cause, NoticeLevel as WireNotice,
};
use tessera_protocol::types::reduce_heading;
use tessera_protocol::value::{Emote, Property, Stance, Value, pid, speed_from_property};

use super::convert;
use super::driver::Inbound;
use super::stats::{RemoteFailure, ServerInfo};
use crate::NetError;
use crate::domain::{CharacterEvent, CombatEvent, SocialEvent, Text};
use crate::interp::{InterpConfig, InterpolationSet, Snapshot};
use crate::model::{
    Anchor, CharacterSummary, ChatChannel, ChatRejection, DataId, EntityId, EntityState, Landblock,
    Location, LocationCause, NoticeLevel, TeleportRefusal, Transform, Vec3,
};
use crate::sandbox::world::{StaticConfig, StaticStats, Statics, WorldSource};
use crate::session::Event;

/// Entity IDs the server assigns are used as [`EntityId`]s unchanged when they are below
/// this value. IDs from here up are local: statics streamed from the client's own data,
/// and (remapped) any server ID that would collide with them.
pub const LOCAL_ID_BASE: u64 = 1 << 63;

/// The replicated state of an entity is [`EntityState`]: the protocol v1 properties (§7.3).
/// Unknown property IDs are skipped, as §7.1 requires, so it stays small whatever the server
/// sends.
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
    /// The name of the `CharacterCreate` sent and not yet answered
    /// (`character-screens.md` §7.1.1 step 2).
    pub(crate) outstanding_create: Option<String>,
    /// Statics (and placed figures) streamed from the client's own data whose creation has
    /// been handed to the session and whose removal has not (`scenario-feed.md` §9.7, local
    /// selection item 1). Bounded by the streamer's resident budget.
    live_statics: BTreeSet<EntityId>,
    /// The local answer to the latest accepted `SelectTarget`, delivered first in the next
    /// poll (§9.7 items 2 and 3; at most one).
    pending_target: Option<Option<EntityId>>,
    /// `SelectTarget`s with an unknown target so far (for the rate-limited log, item 4).
    unknown_targets: u64,
    /// The character of the `EnterWorld` sent and not yet answered.
    pub(crate) outstanding_enter: Option<String>,
    /// The world instance of the latest `WorldEnter` (§6.3.1).
    pub(crate) instance: Option<u32>,
    /// The code of the latest `EnterWorldFailed`.
    pub(crate) enter_refusal: Option<EnterWorldFailedCode>,
    scratch: Vec<Event>,
}

/// The index of the list entry for the outstanding creation `name`
/// (`character-screens.md` §7.1.1 step 3): the first entry whose name is the same as `name`
/// in Unicode Normalization Form C (both normalised, compared code point for code point).
/// tessera-server stores and lists the NFC form (`test-server.md` §6.4, §3.5), so a name
/// sent in another normal form comes back changed.
fn find_created(
    characters: &[tessera_protocol::types::CharacterSummary],
    name: &str,
) -> Option<usize> {
    use unicode_normalization::UnicodeNormalization;
    let want: String = name.nfc().collect();
    characters
        .iter()
        .position(|c| c.name.nfc().eq(want.chars()))
}

/// Tessera's own text for a `CharacterCreateFailed` code (`character-screens.md` §7.1.1
/// step 4); unknown codes read as `SERVER_ERROR`.
#[must_use]
pub(crate) fn create_failed_text(code: CharacterCreateFailedCode) -> &'static str {
    match code {
        CharacterCreateFailedCode::NameTaken => "A character with this name already exists.",
        CharacterCreateFailedCode::NameInvalid => "This name is not allowed on this server.",
        CharacterCreateFailedCode::TooMany => "The character list is full.",
        CharacterCreateFailedCode::AppearanceRefused => {
            "This server does not accept this appearance."
        }
        CharacterCreateFailedCode::Disabled => "This server does not allow creating characters.",
        _ => "The server could not create the character.",
    }
}

/// `head`, then `: detail` when the server gave one.
fn with_detail(head: &str, detail: &str) -> String {
    if detail.is_empty() {
        head.to_owned()
    } else {
        format!("{head}: {detail}")
    }
}

/// The neutral reason of a `TeleportRefused` code (§14.6; unknown codes as `Other`).
fn teleport_refusal(code: TeleportRefusedCode) -> TeleportRefusal {
    match code.canonical() {
        TeleportRefusedCode::UnknownPlace => TeleportRefusal::UnknownPlace,
        TeleportRefusedCode::NotAllowed => TeleportRefusal::NotAllowed,
        TeleportRefusedCode::Cancelled => TeleportRefusal::Cancelled,
        TeleportRefusedCode::InvalidDestination => TeleportRefusal::InvalidDestination,
        _ => TeleportRefusal::Other,
    }
}

/// The neutral reason of a `ChatRejected` code (§14.7; unknown codes as `Other`). Not
/// canonicalised: below minor 2 the decoder reports code 6 as `Other(6)` on purpose.
fn chat_rejection(code: ChatRejectedCode) -> ChatRejection {
    match code {
        ChatRejectedCode::RateLimited => ChatRejection::RateLimited,
        ChatRejectedCode::UnknownRecipient => ChatRejection::UnknownRecipient,
        ChatRejectedCode::InvalidText => ChatRejection::InvalidText,
        ChatRejectedCode::Muted => ChatRejection::Muted,
        ChatRejectedCode::NotInWorld => ChatRejection::NotInWorld,
        ChatRejectedCode::NotJoined => ChatRejection::NotJoined,
        _ => ChatRejection::Other,
    }
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
            outstanding_create: None,
            live_statics: BTreeSet::new(),
            pending_target: None,
            unknown_targets: 0,
            outstanding_enter: None,
            instance: None,
            enter_refusal: None,
            scratch: Vec::new(),
        }
    }

    /// Whether the server takes named chat channels: negotiated minor ≥ 2 and
    /// `CHAT_CHANNELS` advertised (`scenario-feed.md` §9.8). False before `HelloAck`.
    pub(crate) fn chat_channels(&self) -> bool {
        use tessera_protocol::limits::MINOR_CHANNELS;
        use tessera_protocol::message::ServerCaps;
        self.server.as_ref().is_some_and(|s| {
            s.minor >= MINOR_CHANNELS
                && s.caps
                    .effective(s.minor)
                    .contains(ServerCaps::CHAT_CHANNELS)
        })
    }

    /// Whether `target` may be selected (`scenario-feed.md` §9.7, local selection item 1):
    /// none, the player, a remote entity created and not removed, or a streamed static
    /// spawned and not removed — all as of the events already handed to the session.
    fn known_target(&self, target: Option<EntityId>) -> bool {
        match target {
            None => true,
            Some(id) => self.known_server_id(id).is_some() || self.live_statics.contains(&id),
        }
    }

    /// Answers `SelectTarget` locally (`scenario-feed.md` §9.7, decision 0032 D1): a known
    /// target replaces the one pending answer; an unknown one is dropped with a rate-limited
    /// debug line. Nothing is sent to the server and no selection is kept.
    pub(crate) fn select_target(&mut self, target: Option<EntityId>) -> Result<(), NetError> {
        if self.player.is_none() || self.ended {
            // `Session` refuses this outside `InWorld` (§9.5); a backend driven directly
            // before world entry has no player to answer for.
            return Err(NetError::InvalidCommand(
                "selection needs a player in the world",
            ));
        }
        if self.known_target(target) {
            self.pending_target = Some(target);
        } else {
            self.unknown_targets = self.unknown_targets.saturating_add(1);
            let n = self.unknown_targets;
            if n <= 4 || n.is_power_of_two() {
                tracing::debug!(
                    target = ?target,
                    count = n,
                    "SelectTarget names an entity not (or no longer) known; no answer"
                );
            }
        }
        Ok(())
    }

    /// Moves the pending local selection answer to `out` (called first in each poll).
    pub(crate) fn deliver_local(&mut self, out: &mut Vec<Event>) {
        if let Some(target) = self.pending_target.take()
            && let Some(entity) = self.player
        {
            out.push(Event::Combat(CombatEvent::TargetChanged { entity, target }));
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
            Inbound::Connected { characters, looks } => {
                out.push(Event::Connected {
                    characters: characters
                        .iter()
                        .map(|(name, _)| CharacterSummary { name: name.clone() })
                        .collect(),
                });
                // One CharacterInfo per entry, in list order (scenario-feed.md §9.7).
                for (i, (name, body)) in characters.into_iter().enumerate() {
                    out.push(Event::Character(CharacterEvent::CharacterInfo {
                        name,
                        body: Some(convert::body(&body)),
                        class: None,
                        level: None,
                        look: looks.get(i).and_then(convert::look).map(Box::new),
                    }));
                }
            }
            Inbound::CharacterList { characters, looks } => {
                let Some(name) = self.outstanding_create.take() else {
                    tracing::debug!("CharacterList without an outstanding creation dropped");
                    return;
                };
                match find_created(&characters, &name) {
                    Some(i) => {
                        if characters.len() > 1 {
                            tracing::debug!(
                                entries = characters.len(),
                                "CharacterList: entries other than the new one emit nothing"
                            );
                        }
                        // §7.1.1 step 3: the entry's name (the NFC form the server
                        // stored), not the requested one; the screens compare NFC forms.
                        let entry = characters.get(i);
                        out.push(Event::Character(CharacterEvent::CharacterInfo {
                            name: entry.map_or(name, |c| c.name.clone()),
                            body: entry.map(|c| convert::body(&c.body)),
                            class: None,
                            level: None,
                            look: looks.get(i).and_then(convert::look).map(Box::new),
                        }));
                    }
                    None => out.push(Event::Character(CharacterEvent::CharacterRefused {
                        name,
                        reason: Text::Literal(
                            create_failed_text(CharacterCreateFailedCode::ServerError).into(),
                        ),
                    })),
                }
            }
            Inbound::CharacterCreateFailed { code, text } => {
                let Some(name) = self.outstanding_create.take() else {
                    tracing::debug!(
                        "CharacterCreateFailed without an outstanding creation dropped"
                    );
                    return;
                };
                tracing::info!(code = code.name(), text = %text, "character creation refused");
                out.push(Event::Character(CharacterEvent::CharacterRefused {
                    name,
                    reason: Text::Literal(create_failed_text(code).into()),
                }));
            }
            Inbound::WorldEntered(w) => {
                let (location, heading, anchor) = convert::pose(&w.pose);
                let player = self.map_id(w.player);
                self.outstanding_enter = None;
                self.instance = Some(w.instance);
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
                    appearance: Box::new(convert::appearance_with_look(&w.appearance, &w.look)),
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
            Inbound::EnterWorldFailed { code, text } => {
                let character = self.outstanding_enter.take().unwrap_or_default();
                tracing::info!(code = code.name(), text = %text, "world entry refused");
                self.enter_refusal = Some(code);
                out.push(Event::EnterWorldFailed {
                    character,
                    reason: with_detail(&format!("cannot enter the world: {}", code.name()), &text),
                });
            }
            Inbound::TeleportRefused { code, text } => out.push(Event::TeleportRefused {
                reason: teleport_refusal(code),
                text: with_detail(&format!("teleport refused: {}", code.name()), &text),
            }),
            Inbound::ChatRejected { code, text } => out.push(Event::ChatRejected {
                reason: chat_rejection(code),
                text: with_detail(&format!("chat not sent: {}", code.name()), &text),
            }),
            Inbound::Notice {
                level,
                seconds,
                text,
            } => out.push(Event::ServerNotice {
                level: match level {
                    WireNotice::Info => NoticeLevel::Info,
                    WireNotice::Warning => NoticeLevel::Warning,
                    WireNotice::Shutdown => NoticeLevel::Shutdown,
                },
                seconds: u32::from(seconds),
                text,
            }),
            Inbound::TimeSynced { offset, delay } => out.push(Event::TimeSynced {
                offset_us: offset,
                delay_us: delay,
            }),
            Inbound::Created(c) => self.created(*c, out),
            Inbound::Updated(u) => {
                let t = self.last_server_time.unwrap_or(0);
                let changed = if Some(u.id) == self.player_server {
                    let before = self.player_state;
                    self.player_state.apply(&u.changes, t);
                    self.player
                        .filter(|_| before != self.player_state)
                        .map(|id| (id, self.player_state))
                } else if let Some(e) = self.entities.get_mut(&u.id) {
                    let before = e.state;
                    e.state.apply(&u.changes, t);
                    (before != e.state).then_some((e.local, e.state))
                } else {
                    None
                };
                if let Some((id, state)) = changed {
                    out.push(Event::EntityStateChanged { id, state });
                }
            }
            Inbound::Appearance(a) => {
                if let Some(id) = self.local_id(a.id) {
                    out.push(Event::AppearanceChanged {
                        id,
                        appearance: Box::new(convert::appearance_with_look(&a.appearance, &a.look)),
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
                out.push(Event::WorldClock {
                    server_time_us: c.server_time,
                    game_clock: c.game_clock,
                    rate: c.rate,
                });
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
            Inbound::ChannelJoined(channel) => {
                out.push(Event::Social(SocialEvent::ChannelJoined { channel }));
            }
            Inbound::ChannelLeft { name, removed } => {
                let text = removed.then(|| format!("You were removed from the channel {name}."));
                out.push(Event::Social(SocialEvent::ChannelLeft { channel: name }));
                if let Some(text) = text {
                    out.push(Event::ChatMessage {
                        channel: ChatChannel::System,
                        sender: None,
                        text,
                    });
                }
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
        // The motion byte travels with every move; only the properties are announced.
        let announced = EntityState {
            motion: EntityState::default().motion,
            ..state
        } != EntityState::default();
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
            appearance: Box::new(convert::appearance_with_look(&c.appearance, &c.look)),
            tag: None,
        });
        if velocity != Vec3::ZERO {
            out.push(Event::EntityMoved {
                id,
                transform: convert::transform(&location, heading),
                velocity,
            });
        }
        if announced {
            out.push(Event::EntityStateChanged { id, state });
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
        // No answer comes after the end.
        self.outstanding_create = None;
        self.pending_target = None;
        self.live_statics.clear();
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
        // Every event here is about statics: keep the set of delivered ones (§9.7 item 1).
        for e in &events {
            match e {
                Event::StaticSpawned { id, .. } | Event::EntityCreated { id, .. } => {
                    self.live_statics.insert(*id);
                }
                Event::EntityDespawned { id } => {
                    self.live_statics.remove(id);
                }
                Event::StaticsCleared => self.live_statics.clear(),
                _ => {}
            }
        }
        out.append(&mut events);
        self.scratch = events;
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::model::DisconnectReason;
    use tessera_protocol::message::{EntityKind, EntityUpdate, WorldEnter};
    use tessera_protocol::types::Motion;
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
            look: tessera_protocol::types::Look::none(),
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
            look: tessera_protocol::types::Look::none(),
        }))
    }

    /// tessera-server lists the NFC form of a created name (`test-server.md` §6.4): a
    /// creation sent in another normal form is found, and reported under the entry's name
    /// (`character-screens.md` §7.1.1 step 3).
    #[test]
    fn created_name_matches_after_nfc() {
        use tessera_protocol::types::CharacterSummary as Wire;
        let entry = |name: &str| Wire {
            name: name.into(),
            body: Body::Character {
                species: 23,
                sex: 1,
            },
        };
        let mut m = Mapper::new(StaticConfig::default(), InterpConfig::default(), None);
        let mut out = Vec::new();
        m.outstanding_create = Some("E\u{301}owyn".into());
        m.map(
            Inbound::CharacterList {
                characters: vec![entry("Aldith"), entry("\u{c9}owyn")],
                looks: vec![],
            },
            &mut out,
        );
        assert!(m.outstanding_create.is_none());
        assert!(matches!(
            &out[..],
            [Event::Character(CharacterEvent::CharacterInfo { name, body: Some(_), .. })]
                if name == "\u{c9}owyn"
        ));
        // A list without the name (in any normal form) is still SERVER_ERROR.
        out.clear();
        m.outstanding_create = Some("Eowyn".into());
        m.map(
            Inbound::CharacterList {
                characters: vec![entry("\u{c9}owyn")],
                looks: vec![],
            },
            &mut out,
        );
        assert!(matches!(
            &out[..],
            [Event::Character(CharacterEvent::CharacterRefused { .. })]
        ));
    }

    /// The end clears an unanswered creation.
    #[test]
    fn end_clears_outstanding_creation() {
        let mut m = Mapper::new(StaticConfig::default(), InterpConfig::default(), None);
        m.outstanding_create = Some("Cora".into());
        m.end(None);
        assert!(m.outstanding_create.is_none());
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
