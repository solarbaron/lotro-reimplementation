// SPDX-License-Identifier: GPL-3.0-or-later
//! The other entities of an official world session, mapped to the neutral entity events
//! the walker draws (`official-messages-world.md` §5.1–§5.3, §5.7, §5.8.1, §6;
//! `official-messages-play.md` §4.6, §8; `ARCHITECTURE.md` §19.1).
//!
//! [`EntityFeed`] keeps what the mapping needs per entity: the template of its `0x98`, the
//! few properties the neutral model carries (display name, the NPC and inanimate-object
//! flags, species and sex, `official-messages-world.md` §5.2.4), its placement and whether
//! the client was told about it. The session driver ([`super::session_phase`]) feeds it the
//! decoded lifecycle and property events and the positions of entities it does not know
//! yet; the feed answers with neutral events and the `0xD0` queries to send.
//!
//! # Mapping (Tessera's reading, recorded where the specs are silent)
//!
//! - **Created, not spawned.** `0x98` with its `0x52` becomes [`Event::EntityCreated`] (id,
//!   class, name, location, appearance: the "entity created (ID, class, property set,
//!   position)" of `ARCHITECTURE.md` §19.1), not the [`Event::EntitySpawned`] of
//!   `official-messages-world.md` §6, which carries neither the template nor an appearance
//!   and which the walker does not draw. The event waits for the entity's first position
//!   (`0xCE`, `0xBF` or the destination of a `0x59` path): a neutral creation has a
//!   location. Inside a batch (`0x0D`, `0x4C`) the creation is made when the batch ends, so
//!   the properties of the whole batch are in it ([`EntityFeed::flush`]).
//! - **Baseline.** The properties merge over the template's baseline set (`0x70…` →
//!   `0x79…`, §6 row `0x98 + 0x52`), read from the install through
//!   [`super::data::InstallData::template_value`].
//! - **Kind.** The inanimate-object flag (`0x10000F53`) set: [`EntityKind::Object`]; else
//!   the NPC flag (`0x100014EA`): set [`EntityKind::NonPlayer`] (the flag covers NPCs and
//!   monsters alike), clear [`EntityKind::Player`]. Neither known: `NonPlayer`, counted
//!   ([`FeedCounters::unclassified`]). The classification enums `0x23000174` and
//!   `0x23000091` of §5.2.3 are not tied to any field of `0x98` by the spec, so they are not
//!   used.
//! - **Body.** A player with both species (`0x100000FC`) and sex (`0x10000A50`) has the
//!   playable body of those keys (`appearance.md` §2.2: the species keys of enum
//!   `0x2300000B`, the same mapper as the property); every other entity has its template
//!   game object as the body (`appearance.md` §7.2: server NPCs are game objects).
//! - **Appearance updates (`0x89`).** Only the scale (bit 5) has a defined meaning; a
//!   uniform scale becomes the appearance's scale. The fields whose rendering meaning is
//!   OPEN (`secondary_did`, tints, `param_a`, `param_b`) and the parts are decoded and
//!   counted ([`FeedCounters::appearance_fields_skipped`]); a descriptor with a base model
//!   (bit 0) cannot be decoded at all (§2.8 OPEN) and is counted by the driver.
//! - **Batch target.** The sub-events of a batch inherit its target (§1.1.3). A sub-event
//!   addressed to the default recipient (`2`, `official-session.md` §5.6.3) after a `0x98`
//!   in the same batch applies to the entity that `0x98` created: an entity-scoped event
//!   (`0x52`, `0x88`, `0x89`, …) has no other meaning for the session recipient. Engineering
//!   choice, spec silent.
//! - **Unknown entities.** A position or property update for an entity the session does
//!   not know sends `0xD0` once per entity (§5.3), and nothing reaches the client.
//! - **The local player** is drawn by the walker: a `0x98` for it creates nothing.

use std::collections::{HashMap, HashSet, VecDeque};

use tessera_net::model::{MAX_APPEARANCE_SCALE, MAX_NAME_LEN};
use tessera_net::{
    Appearance, Body, DataId, EntityId, EntityKind, Event, Location, Transform, Vec3,
};

use super::data::SchemaView;
use crate::play::chat::TextResolver;
use crate::play::movement::{quaternion_heading, wire_heading, wire_location};
use crate::play::{OutgoingEvent, clean_display_name};
use crate::world::{
    EncodedPropertyEntry, EncodedPropertyValue, EntityAppearanceDesc, EntityCreate,
    EntityPlacement, LocalizedTokenText, MissingEntityQuery, PID_AGENT_SPECIES,
    PID_ENTITY_DISPLAY_NAME, PID_SEX, PositionUpdate, WirePosition,
};

/// `is_npc_agent_flag` (`0x100014EA`, Boolean, `official-messages-world.md` §5.2.4).
pub const PID_IS_NPC_AGENT: u32 = 0x1000_14EA;
/// `is_inanimate_prop_flag` (`0x10000F53`, Boolean, §5.2.4).
pub const PID_IS_INANIMATE_PROP: u32 = 0x1000_0F53;

/// The properties the feed reads, from the wire and from a template's baseline.
pub const FEED_PIDS: [u32; 5] = [
    PID_ENTITY_DISPLAY_NAME,
    PID_IS_NPC_AGENT,
    PID_IS_INANIMATE_PROP,
    PID_AGENT_SPECIES,
    PID_SEX,
];

/// The default session recipient of a blob without `recipientId` (`official-session.md`
/// §5.6.3, `official-messages-world.md` §1.1.1).
pub const DEFAULT_RECIPIENT: u64 = 2;

/// Most entities tracked at once (created or waiting for a position). Engineering choice,
/// spec silent.
pub const MAX_ENTITIES: usize = 4096;

/// Most entities waiting for their first position; the oldest is dropped beyond it.
/// Engineering choice, spec silent.
pub const MAX_PENDING: usize = 1024;

/// Most entities a `0xD0` is sent for in one world (each once). Engineering choice.
pub const MAX_QUERIED: usize = 4096;

/// What the feed did, ignored or could not map, for diagnostics and tests.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub struct FeedCounters {
    /// `EntityCreated` events made.
    pub created: u64,
    /// `EntityDespawned` events made.
    pub despawned: u64,
    /// `0x40` for an entity the session does not know (or one still without a position).
    pub despawn_unknown: u64,
    /// `0x98` for the local player (the walker draws it).
    pub own_player: u64,
    /// `0x98` for an entity already tracked (the old one is removed first).
    pub recreated: u64,
    /// `0x98` refused because [`MAX_ENTITIES`] are tracked.
    pub overflow: u64,
    /// Entities dropped while waiting for a position (over [`MAX_PENDING`]).
    pub pending_dropped: u64,
    /// Entities created without a known NPC or object flag (shown as non-players).
    pub unclassified: u64,
    /// Display names that could not be resolved (the entity is created without a name).
    pub unresolved_names: u64,
    /// Null sub-event frames of a batch (their meaning is OPEN, §5.1).
    pub null_sub_events: u64,
    /// `0x0D` batches inside a batch, not read: whether their tail flag is present is OPEN.
    pub nested_aoi_batches: u64,
    /// Batches nested deeper than the driver reads.
    pub deep_batches: u64,
    /// Entity-scoped events addressed to the default recipient with no entity to apply to.
    pub no_target: u64,
    /// Position and property events for entities the session does not know.
    pub unknown_entity_events: u64,
    /// `0xD0` queries made.
    pub queries: u64,
    /// `AppearanceChanged` events made.
    pub appearance_changes: u64,
    /// `0x89` fields decoded but not shown (meaning OPEN, or a scale that is not uniform).
    pub appearance_fields_skipped: u64,
    /// `0x59` paths mapped to a move to their destination (interpolation along the path is
    /// OPEN, `official-messages-play.md` §4.6.1).
    pub paths: u64,
    /// `0x59` paths without a usable waypoint.
    pub empty_paths: u64,
    /// Placements of an entity without a usable location (attachments, invalid positions).
    pub unplaced: u64,
    /// `0xED` updates without a controller or with one cleared: no neutral event.
    pub motion_controllers: u64,
    /// `0xBD` animation-layer states: no neutral event (the layers' meaning for the neutral
    /// model is not specified).
    pub anim_layers: u64,
}

/// The facts of one entity the neutral model carries (module docs).
#[derive(Debug, Clone, Default, PartialEq)]
struct Facts {
    name: Option<LocalizedTokenText>,
    npc: Option<bool>,
    prop: Option<bool>,
    species: Option<u32>,
    sex: Option<u32>,
}

impl Facts {
    /// Takes `value` for `pid` when it is one of [`FEED_PIDS`] with the expected kind.
    fn apply(&mut self, pid: u32, value: &EncodedPropertyValue) {
        match (pid, value) {
            (PID_ENTITY_DISPLAY_NAME, EncodedPropertyValue::LocalizedTokenText(t)) => {
                self.name = Some(t.clone());
            }
            (PID_IS_NPC_AGENT, EncodedPropertyValue::Boolean(b)) => self.npc = Some(*b),
            (PID_IS_INANIMATE_PROP, EncodedPropertyValue::Boolean(b)) => self.prop = Some(*b),
            (PID_AGENT_SPECIES, EncodedPropertyValue::Enum(v)) => self.species = Some(*v),
            (PID_SEX, EncodedPropertyValue::Enum(v)) => self.sex = Some(*v),
            _ => {}
        }
    }

    /// Forgets `pid` (`0x0E`, `0xE7`).
    fn clear(&mut self, pid: u32) {
        match pid {
            PID_ENTITY_DISPLAY_NAME => self.name = None,
            PID_IS_NPC_AGENT => self.npc = None,
            PID_IS_INANIMATE_PROP => self.prop = None,
            PID_AGENT_SPECIES => self.species = None,
            PID_SEX => self.sex = None,
            _ => {}
        }
    }

    /// These facts over `base`.
    fn over(&self, base: &Self) -> Self {
        Self {
            name: self.name.clone().or_else(|| base.name.clone()),
            npc: self.npc.or(base.npc),
            prop: self.prop.or(base.prop),
            species: self.species.or(base.species),
            sex: self.sex.or(base.sex),
        }
    }
}

/// What the client was told about an entity.
#[derive(Debug, Clone, PartialEq)]
struct Shown {
    appearance: Appearance,
}

/// One tracked entity.
#[derive(Debug, Clone)]
struct Tracked {
    template: u32,
    wire: Facts,
    baseline: Facts,
    scale: Option<f32>,
    placed: Option<(Location, f32)>,
    heading: f32,
    shown: Option<Shown>,
}

/// The other entities of one world (module docs).
#[derive(Debug, Default)]
pub struct EntityFeed {
    player: Option<u64>,
    entities: HashMap<u64, Tracked>,
    pending: VecDeque<u64>,
    queried: HashSet<u64>,
    batch_created: Option<u64>,
    counters: FeedCounters,
}

/// What one input produced.
#[derive(Debug, Default)]
pub struct FeedOutput {
    /// Neutral events.
    pub events: Vec<Event>,
    /// Messages to send on the world connection (`0xD0`).
    pub outgoing: Vec<OutgoingEvent>,
}

/// The location, heading and velocity of a `0xCE` placement, `None` for an attachment or a
/// position that is not a valid location (the adapter's own reading, `play::adapter`).
#[must_use]
pub fn update_location(u: &PositionUpdate) -> Option<(Location, f32, [f32; 3])> {
    let (position, heading): (&WirePosition, Option<f32>) = match &u.placement {
        EntityPlacement::WorldWithHeading {
            position,
            heading_raw,
        } => (position, Some(wire_heading(*heading_raw))),
        EntityPlacement::WorldFull { position } => (position, None),
        EntityPlacement::Attachment { .. } => return None,
    };
    let (loc, _) = wire_location(position)?;
    let heading = heading
        .or_else(|| quaternion_heading(position.orientation))
        .unwrap_or(0.0);
    Some((loc, heading, u.velocity.map(|c| c.unwrap_or(0.0))))
}

/// The location and heading of a full wire position (`0xBF`).
#[must_use]
pub fn position_location(p: &WirePosition) -> Option<(Location, f32)> {
    let (loc, _) = wire_location(p)?;
    Some((loc, quaternion_heading(p.orientation).unwrap_or(0.0)))
}

fn moved(id: u64, loc: &Location, heading: f32, velocity: [f32; 3]) -> Event {
    let [x, y, z] = velocity;
    Event::EntityMoved {
        id: EntityId(id),
        transform: Transform::new(loc.position(), heading),
        velocity: Vec3::new(x, y, z),
    }
}

/// Tessera's heading of the step from `a` to `b` (radians, 0 = +X, counter-clockwise;
/// `tessera_net::model`), `None` when they are in different regions or the same point.
fn heading_between(a: &Location, b: &Location) -> Option<f32> {
    if a.region != b.region {
        return None;
    }
    let (dx, dy) = (b.x - a.x, b.y - a.y);
    if !(dx.is_finite() && dy.is_finite()) || dx.hypot(dy) < 1e-3 {
        return None;
    }
    #[allow(clippy::cast_possible_truncation)]
    Some(dy.atan2(dx) as f32)
}

impl EntityFeed {
    /// A feed for a new world with the local `player` (`None` outside the world): every
    /// entity of the previous world is forgotten (the client drops them with the world,
    /// `Event::ReturnedToCharacterSelect`).
    pub fn reset(&mut self, player: Option<u64>) {
        let counters = self.counters;
        *self = Self {
            player,
            counters,
            ..Self::default()
        };
    }

    /// The counters.
    #[must_use]
    pub fn counters(&self) -> FeedCounters {
        self.counters
    }

    /// The counters, for the driver's own counts of what reaches no event.
    pub fn counters_mut(&mut self) -> &mut FeedCounters {
        &mut self.counters
    }

    /// Whether `id` is the local player.
    #[must_use]
    pub fn is_player(&self, id: u64) -> bool {
        self.player == Some(id)
    }

    /// Whether the client was told about `id` (an `EntityCreated` went out).
    #[must_use]
    pub fn is_created(&self, id: u64) -> bool {
        self.entities.get(&id).is_some_and(|t| t.shown.is_some())
    }

    /// Whether `id` waits for its first position.
    #[must_use]
    pub fn is_pending(&self, id: u64) -> bool {
        self.entities.get(&id).is_some_and(|t| t.shown.is_none())
    }

    /// Entities tracked (created or pending).
    #[must_use]
    pub fn tracked(&self) -> usize {
        self.entities.len()
    }

    /// A top-level batch begins: no entity was created in it yet (module docs "Batch
    /// target").
    pub fn begin_batch(&mut self) {
        self.batch_created = None;
    }

    /// The target of a sub-event of the current batch addressed to `recipient`.
    #[must_use]
    pub fn batch_target(&self, recipient: u64) -> u64 {
        match self.batch_created {
            Some(id) if recipient == DEFAULT_RECIPIENT => id,
            _ => recipient,
        }
    }

    /// A batch ended.
    pub fn end_batch(&mut self) {
        self.batch_created = None;
    }

    /// An event for `id`, which the session does not know: `0xD0` once per entity and
    /// world, at most [`MAX_QUERIED`] per world (§5.3); counted either way.
    pub fn unknown_entity(&mut self, id: u64, out: &mut FeedOutput) {
        self.counters.unknown_entity_events += 1;
        if id == DEFAULT_RECIPIENT || self.is_player(id) {
            return;
        }
        if self.queried.len() >= MAX_QUERIED || !self.queried.insert(id) {
            return;
        }
        let payload = MissingEntityQuery { entity_id: id }.to_bytes();
        match payload
            .map_err(crate::play::PlayError::from)
            .and_then(|p| OutgoingEvent::new(MissingEntityQuery::EVENT_ID, p))
        {
            Ok(ev) => {
                self.counters.queries += 1;
                out.outgoing.push(ev);
            }
            Err(e) => tracing::debug!(error = %e, "official world: 0xD0 not encoded"),
        }
    }

    /// The baseline facts of `template` from the install (§6 row `0x98 + 0x52`).
    fn baseline(template: u32, data: &SchemaView) -> Facts {
        let mut f = Facts::default();
        if template == 0 {
            return f;
        }
        for pid in FEED_PIDS {
            if let Some(v) = data.0.template_value(template, pid) {
                f.apply(pid, &v);
            }
        }
        f
    }

    /// `0x98` (§5.2.1): a new entity, waiting for its position.
    pub fn create(&mut self, c: EntityCreate, data: &SchemaView, out: &mut FeedOutput) {
        self.batch_created = Some(c.entity_id);
        if self.is_player(c.entity_id) {
            self.counters.own_player += 1;
            return;
        }
        if c.entity_id == DEFAULT_RECIPIENT {
            self.counters.no_target += 1;
            return;
        }
        if self.entities.contains_key(&c.entity_id) {
            self.counters.recreated += 1;
            self.remove(c.entity_id, out);
        }
        if self.entities.len() >= MAX_ENTITIES {
            self.counters.overflow += 1;
            return;
        }
        if self.pending.len() >= MAX_PENDING
            && let Some(old) = self.pending.pop_front()
            && self.entities.remove(&old).is_some()
        {
            self.counters.pending_dropped += 1;
        }
        let baseline = Self::baseline(c.template_did, data);
        self.entities.insert(
            c.entity_id,
            Tracked {
                template: c.template_did,
                wire: Facts::default(),
                baseline,
                scale: None,
                placed: None,
                heading: 0.0,
                shown: None,
            },
        );
        self.pending.push_back(c.entity_id);
    }

    fn remove(&mut self, id: u64, out: &mut FeedOutput) -> bool {
        let Some(t) = self.entities.remove(&id) else {
            return false;
        };
        self.pending.retain(|p| *p != id);
        if t.shown.is_some() {
            self.counters.despawned += 1;
            out.events.push(Event::EntityDespawned { id: EntityId(id) });
            return true;
        }
        false
    }

    /// `0x40` (§5.3) for `recipient`.
    pub fn despawn(&mut self, recipient: u64, out: &mut FeedOutput) {
        if self.is_player(recipient) {
            // The walker's own avatar; the world ends with the teardown, not with this.
            self.counters.despawn_unknown += 1;
            return;
        }
        if !self.remove(recipient, out) {
            self.counters.despawn_unknown += 1;
        }
    }

    /// Property entries for `recipient` (`0x52`, `0x88`, `0x92`).
    pub fn properties<'a>(
        &mut self,
        recipient: u64,
        entries: impl IntoIterator<Item = &'a EncodedPropertyEntry>,
        data: &SchemaView,
        out: &mut FeedOutput,
    ) {
        if self.is_player(recipient) {
            return;
        }
        let Some(t) = self.entities.get_mut(&recipient) else {
            if recipient == DEFAULT_RECIPIENT {
                self.counters.no_target += 1;
            } else {
                self.unknown_entity(recipient, out);
            }
            return;
        };
        for e in entries {
            if let Some(v) = &e.value {
                t.wire.apply(e.pid, v);
            }
        }
        self.refresh(recipient, data, out);
    }

    /// `0x0E` / `0xE7` (§5.8.1) for `recipient`.
    pub fn remove_property(
        &mut self,
        recipient: u64,
        pid: u32,
        data: &SchemaView,
        out: &mut FeedOutput,
    ) {
        if self.is_player(recipient) {
            return;
        }
        let Some(t) = self.entities.get_mut(&recipient) else {
            if recipient == DEFAULT_RECIPIENT {
                self.counters.no_target += 1;
            } else {
                self.unknown_entity(recipient, out);
            }
            return;
        };
        t.wire.clear(pid);
        self.refresh(recipient, data, out);
    }

    /// `0x89` (§5.7) for `recipient` (module docs "Appearance updates").
    pub fn visual(
        &mut self,
        recipient: u64,
        desc: &EntityAppearanceDesc,
        data: &SchemaView,
        out: &mut FeedOutput,
    ) {
        if self.is_player(recipient) {
            return;
        }
        let skipped = u64::from(desc.secondary_did.is_some())
            + u64::from(desc.param_a.is_some())
            + u64::from(desc.tint_entries.is_some())
            + u64::from(desc.primary_part.is_some())
            + u64::from(desc.part_map.is_some())
            + u64::from(desc.param_b.is_some());
        self.counters.appearance_fields_skipped += skipped;
        let Some(t) = self.entities.get_mut(&recipient) else {
            if recipient == DEFAULT_RECIPIENT {
                self.counters.no_target += 1;
            } else {
                self.unknown_entity(recipient, out);
            }
            return;
        };
        match desc.scale {
            Some([x, y, z])
                if x == y && y == z && x.is_finite() && x > 0.0 && x <= MAX_APPEARANCE_SCALE =>
            {
                t.scale = Some(x);
            }
            Some(_) => self.counters.appearance_fields_skipped += 1,
            None => t.scale = None,
        }
        self.refresh(recipient, data, out);
    }

    /// A position for `id` (`0xCE`, `0xBF`): places an entity that waits for one (the
    /// creation follows at [`Self::flush`]) or moves a created one. Returns `false` for an
    /// entity the session does not know (counted, `0xD0` queued).
    pub fn place(
        &mut self,
        id: u64,
        at: Option<(Location, f32)>,
        velocity: [f32; 3],
        out: &mut FeedOutput,
    ) -> bool {
        let Some(t) = self.entities.get_mut(&id) else {
            self.unknown_entity(id, out);
            return false;
        };
        let Some((loc, heading)) = at else {
            self.counters.unplaced += 1;
            return true;
        };
        t.heading = heading;
        if t.shown.is_none() {
            t.placed = Some((loc, heading));
        } else {
            out.events.push(moved(id, &loc, heading, velocity));
        }
        true
    }

    /// A `0x59` path for `id`: a move to its destination (module docs, [`FeedCounters::paths`]).
    pub fn path(&mut self, id: u64, waypoints: &[Location], out: &mut FeedOutput) {
        if self.is_player(id) {
            return;
        }
        let Some(dest) = waypoints.last() else {
            self.counters.empty_paths += 1;
            return;
        };
        let previous = self
            .entities
            .get(&id)
            .map_or(0.0, |t| t.placed.map_or(t.heading, |(_, h)| h));
        let heading = waypoints
            .len()
            .checked_sub(2)
            .and_then(|i| waypoints.get(i))
            .and_then(|a| heading_between(a, dest))
            .unwrap_or(previous);
        if self.place(id, Some((*dest, heading)), [0.0; 3], out) {
            self.counters.paths += 1;
        }
    }

    /// What the client is shown for `t`; `creating` counts what a creation lacks.
    fn describe(
        &mut self,
        t: &Tracked,
        data: &SchemaView,
        creating: bool,
    ) -> (EntityKind, String, Appearance) {
        let f = t.wire.over(&t.baseline);
        let kind = if f.prop == Some(true) {
            EntityKind::Object
        } else {
            match f.npc {
                Some(true) => EntityKind::NonPlayer,
                Some(false) => EntityKind::Player,
                None => {
                    if creating {
                        self.counters.unclassified += 1;
                    }
                    EntityKind::NonPlayer
                }
            }
        };
        let name = match &f.name {
            Some(text) => match data.resolve(text) {
                Some(s) => clean_display_name(&s, MAX_NAME_LEN),
                None => {
                    if creating {
                        self.counters.unresolved_names += 1;
                    }
                    String::new()
                }
            },
            None => String::new(),
        };
        let body = match (kind, f.species, f.sex) {
            (EntityKind::Player, Some(species), Some(sex)) if species != 0 && sex != 0 => {
                Body::Character { species, sex }
            }
            _ => Body::Object(DataId(t.template)),
        };
        let mut appearance = Appearance::new(body);
        if let Some(s) = t.scale {
            appearance.scale = s;
        }
        (kind, name, appearance)
    }

    /// An `AppearanceChanged` for a created entity whose appearance changed.
    fn refresh(&mut self, id: u64, data: &SchemaView, out: &mut FeedOutput) {
        let Some(t) = self.entities.get(&id).cloned() else {
            return;
        };
        let Some(shown) = &t.shown else {
            return;
        };
        let (_, _, appearance) = self.describe(&t, data, false);
        if appearance != shown.appearance {
            if let Some(e) = self.entities.get_mut(&id) {
                e.shown = Some(Shown {
                    appearance: appearance.clone(),
                });
            }
            self.counters.appearance_changes += 1;
            out.events.push(Event::AppearanceChanged {
                id: EntityId(id),
                appearance: Box::new(appearance),
            });
        }
    }

    /// The creations of every waiting entity that has a position, in the order of their
    /// `0x98` (module docs "Created, not spawned").
    pub fn flush(&mut self, data: &SchemaView, out: &mut FeedOutput) {
        let waiting: Vec<u64> = self.pending.iter().copied().collect();
        for id in waiting {
            let Some(t) = self.entities.get(&id).cloned() else {
                continue;
            };
            let Some((location, heading)) = t.placed else {
                continue;
            };
            let (kind, name, appearance) = self.describe(&t, data, true);
            if let Some(e) = self.entities.get_mut(&id) {
                e.shown = Some(Shown {
                    appearance: appearance.clone(),
                });
            }
            self.pending.retain(|p| *p != id);
            self.counters.created += 1;
            out.events.push(Event::EntityCreated {
                id: EntityId(id),
                kind,
                name,
                class: (t.template != 0).then_some(DataId(t.template)),
                location,
                heading,
                appearance: Box::new(appearance),
                tag: None,
            });
        }
    }
}
