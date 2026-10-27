// SPDX-License-Identifier: GPL-3.0-or-later
//! The other entities of an official world session, mapped to the neutral entity events
//! the walker draws (`official-messages-world.md` §5.1–§5.3, §5.7, §5.8.1, §6;
//! `official-messages-play.md` §4.6, §8; `ARCHITECTURE.md` §19.1).
//!
//! [`EntityFeed`] keeps what the mapping needs per entity: the template of its `0x98`, the
//! few properties the neutral model carries (display name, the NPC and inanimate-object
//! flags, species and sex, `official-messages-world.md` §5.2.4), its last placement and
//! what the client was told about it. The session driver ([`super::session_phase`]) feeds
//! it the decoded lifecycle, property and position events; the feed answers with neutral
//! events and the `0xD0` queries to send.
//!
//! # Mapping (Tessera's reading, recorded where the specs are silent)
//!
//! - **Created, not spawned.** `0x98` with its `0x52` becomes [`Event::EntityCreated`] (id,
//!   class, name, location, appearance: the "entity created (ID, class, property set,
//!   position)" of `ARCHITECTURE.md` §19.1), not the [`Event::EntitySpawned`] of
//!   `official-messages-world.md` §6, which carries neither the template nor an appearance
//!   and which the walker does not draw. The event waits for the entity's first position
//!   (`0xCE`, `0xBF` or the destination of a `0x59` path): a neutral creation has a
//!   location. Engineering choice, spec silent / departs from world §6 (question in
//!   `docs/m8-remediation.md` §15).
//! - **One flush per message.** While one top-level message (a batch with all its
//!   sub-events, or a single event) is read, the feed only records: entities placed for
//!   the first time, entities whose shown appearance may have changed, and the last move of
//!   each shown entity. [`EntityFeed::flush`] then makes, in this order, the creations (in
//!   the order of their `0x98`), at most one [`Event::AppearanceChanged`] and at most one
//!   [`Event::EntityMoved`] per entity. Despawns go out at once. So one message makes at
//!   most one creation, one appearance change and one move per tracked entity (at most
//!   [`MAX_ENTITIES`] each) and one despawn per entity shown before it.
//! - **Baseline.** The properties merge over the template's baseline set (`0x70…` →
//!   `0x79…`, §6 row `0x98 + 0x52`), read from the install through
//!   [`super::data::InstallData::template_value`] when the entity is created. At most
//!   [`MAX_TEMPLATE_LOADS_PER_PASS`] templates the install has not decoded yet are read per
//!   driver pass ([`EntityFeed::begin_pass`]); a creation whose template is not read yet
//!   waits for a later pass ([`FeedCounters::template_loads_deferred`]). Engineering choice.
//! - **Kind.** The inanimate-object flag (`0x10000F53`) set: [`EntityKind::Object`]; else
//!   the NPC flag (`0x100014EA`): set [`EntityKind::NonPlayer`] (the flag covers NPCs and
//!   monsters alike), clear [`EntityKind::Player`]. Neither known: `NonPlayer`, counted
//!   ([`FeedCounters::unclassified`]). The classification enums `0x23000174` and
//!   `0x23000091` of §5.2.3 are not tied to any field of `0x98` by the spec, so they are not
//!   used.
//! - **Name.** The display name is rendered once, at the creation. The neutral model has no
//!   event for a later name or kind change: those are counted
//!   ([`FeedCounters::name_changes_unshown`], [`FeedCounters::kind_changes_unshown`]). Per
//!   entity only what the name needs is kept: the token and table of a reference, or a
//!   literal already shown as text (`string-table.md` §9.0 step 1 through the install's
//!   resolver) and cut to [`MAX_NAME_LEN`]; authoring strings and arguments are dropped.
//! - **Body.** A player with both species (`0x100000FC`) and sex (`0x10000A50`) has the
//!   playable body of those keys (`appearance.md` §2.2: the species keys of enum
//!   `0x2300000B`, the same mapper as the property); every other entity has its template
//!   game object as the body (`appearance.md` §7.2: server NPCs are game objects).
//! - **Appearance updates (`0x89`).** Only the scale (bit 5) has a defined meaning; a
//!   uniform scale becomes the appearance's scale. The fields whose rendering meaning is
//!   OPEN (`secondary_did`, tints, `param_a`, `param_b`) and the parts are decoded and
//!   counted ([`FeedCounters::appearance_fields_skipped`]); a descriptor with a base model
//!   (bit 0) cannot be decoded at all (§2.8 OPEN) and is counted by the driver. An
//!   appearance is recomputed only when a property the feed reads changed.
//! - **Batch target.** The sub-events of a batch take its target (§1.1.3). The one
//!   exception is the `0x52` directly after a `0x98` in a `0x0D` whose target is the
//!   default recipient (`2`, `official-session.md` §5.6.3): it applies to the entity that
//!   `0x98` created (§5.2.2, [`sub_event_target`]). Every other sub-event for the default
//!   recipient has no entity and is counted ([`FeedCounters::no_target`]).
//! - **Unknown entities.** A position or property update for an entity the session does
//!   not know sends `0xD0` once per entity (§5.3), and nothing reaches the client.
//! - **Paths (`0x59`).** A path is a move to its destination, facing along its last step
//!   (one waypoint: the last heading the entity had), with a velocity towards the
//!   destination from the entity's last place, so the walker animates and smooths it. The
//!   spec gives `speed_param` no unit (`official-messages-play.md` §4.6.1), so the speed is
//!   the distance over [`PATH_NOMINAL_SECS`], at most [`MAX_PATH_SPEED`]. Engineering
//!   choice; interpolation along the path is a question in `docs/m8-remediation.md` §15.
//! - **The local player** is drawn by the walker: a `0x98` for it creates nothing, a `0x40`
//!   for it removes nothing and its property events are dropped (each counted).

use std::collections::{HashMap, HashSet, VecDeque};

use tessera_net::model::{MAX_APPEARANCE_SCALE, MAX_NAME_LEN};
use tessera_net::{
    Appearance, Body, DataId, EntityId, EntityKind, Event, Location, Transform, Vec3,
};

use super::data::SchemaView;
use crate::play::chat::TextResolver;
use crate::play::{OutgoingEvent, clean_display_name};
use crate::world::{
    EncodedPropertyEntry, EncodedPropertyValue, EntityAppearanceDesc, EntityCreate,
    LocalizedTokenText, LocalizedTokenTextContent, MissingEntityQuery, PID_AGENT_SPECIES,
    PID_ENTITY_DISPLAY_NAME, PID_SEX, PropertyTableUpdate,
};

#[cfg(test)]
mod tests;

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

/// Most entities tracked at once (created, placed or waiting for a position). Engineering
/// choice, spec silent.
pub const MAX_ENTITIES: usize = 4096;

/// Most entities waiting for their first position; the oldest is dropped beyond it.
/// Engineering choice, spec silent.
pub const MAX_PENDING: usize = 1024;

/// Most entities a `0xD0` is sent for in one world (each once). Engineering choice.
pub const MAX_QUERIED: usize = 4096;

/// Most templates read from the install per driver pass when the install has not decoded
/// them yet ([`super::data::InstallData::template_cached`]). Engineering choice.
pub const MAX_TEMPLATE_LOADS_PER_PASS: usize = 32;

/// The time a `0x59` path's move is spread over for its velocity, in seconds (module docs
/// "Paths"). Engineering choice: `speed_param` has no unit in the spec.
pub const PATH_NOMINAL_SECS: f64 = 1.0;

/// Highest speed given to a `0x59` path's move, in metres per second. Engineering choice.
pub const MAX_PATH_SPEED: f64 = 7.0;

/// What the feed did, ignored or could not map, for diagnostics and tests.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub struct FeedCounters {
    /// `EntityCreated` events made.
    pub created: u64,
    /// `EntityDespawned` events made.
    pub despawned: u64,
    /// `0x40` for an entity the session does not know (or one not shown yet).
    pub despawn_unknown: u64,
    /// `0x40` for the local player (the walker keeps its avatar; the world ends with the
    /// teardown, not with this).
    pub player_despawns: u64,
    /// `0x98` for the local player (the walker draws it).
    pub own_player: u64,
    /// Property and appearance events for the local player, dropped (the feed tracks only
    /// other entities).
    pub player_property_events: u64,
    /// `0x98` for an entity already tracked (the old one is removed first).
    pub recreated: u64,
    /// `0x98` refused because [`MAX_ENTITIES`] are tracked, and `0x52` sub-events that
    /// followed such a `0x98` (dropped).
    pub overflow: u64,
    /// Entities dropped while waiting for a position (over [`MAX_PENDING`], or to make room
    /// below [`MAX_ENTITIES`]).
    pub pending_dropped: u64,
    /// Entities created without a known NPC or object flag (shown as non-players).
    pub unclassified: u64,
    /// Display names that could not be resolved (the entity is created without a name).
    pub unresolved_names: u64,
    /// Display-name changes of shown entities (no neutral event carries a new name).
    pub name_changes_unshown: u64,
    /// Kind changes of shown entities (no neutral event carries a new kind).
    pub kind_changes_unshown: u64,
    /// Null sub-event frames of a batch (their meaning is OPEN, §5.1).
    pub null_sub_events: u64,
    /// `0x0D` batches inside a batch read with a 1-byte tail flag after their frames.
    pub nested_aoi_tail: u64,
    /// `0x0D` batches inside a batch read without a tail flag (whether a nested `0x0D` has
    /// one is OPEN, §5.1).
    pub nested_aoi_no_tail: u64,
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
    /// `0x59` paths mapped to a move to their destination (module docs "Paths").
    pub paths: u64,
    /// `0x59` paths without a usable waypoint.
    pub empty_paths: u64,
    /// Placements of an entity without a usable location (attachments, invalid positions).
    pub unplaced: u64,
    /// Creations put off to a later pass because their template was not read yet
    /// ([`MAX_TEMPLATE_LOADS_PER_PASS`]); counted once per pass and entity.
    pub template_loads_deferred: u64,
    /// `0xED` updates without a controller or with one cleared: no neutral event.
    pub motion_controllers: u64,
    /// `0xBD` animation-layer states: no neutral event (the layers' meaning for the neutral
    /// model is not specified).
    pub anim_layers: u64,
}

/// What a `0x98` did, for the sub-event after it ([`sub_event_target`]).
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum CreateOutcome {
    /// An entity is tracked under this id.
    Tracked(u64),
    /// The `0x98` was for the local player.
    Player(u64),
    /// Refused ([`MAX_ENTITIES`]).
    Refused,
    /// The `0x98` named the default recipient: no entity.
    NoTarget,
}

/// Where a batch sub-event goes ([`sub_event_target`]).
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum SubTarget {
    /// To this recipient.
    To(u64),
    /// Nowhere: it followed a refused `0x98` (counted as [`FeedCounters::overflow`]).
    Skip,
}

/// The target of sub-event `sub_event_id` of a batch whose target is `inherited`, after a
/// sub-event that (if it was a `0x98`) had the outcome `previous` (module docs "Batch
/// target"): the batch's target (§1.1.3), except for a `0x52` directly after a `0x98` in an
/// `aoi_batch` (`0x0D`) sent to the default recipient, which applies to the entity that
/// `0x98` created (§5.2.2). A `0x52` after a refused `0x98` goes nowhere. Tessera's reading
/// of §5.2.2 "bundled immediately after `0x98`"; when the batch names an entity, §1.1.3's
/// inherited target is kept.
#[must_use]
pub fn sub_event_target(
    aoi_batch: bool,
    sub_event_id: u32,
    inherited: u64,
    previous: Option<CreateOutcome>,
) -> SubTarget {
    if !aoi_batch
        || sub_event_id != PropertyTableUpdate::EVENT_ID_ENTITY
        || inherited != DEFAULT_RECIPIENT
    {
        return SubTarget::To(inherited);
    }
    match previous {
        Some(CreateOutcome::Tracked(id) | CreateOutcome::Player(id)) => SubTarget::To(id),
        Some(CreateOutcome::Refused) => SubTarget::Skip,
        Some(CreateOutcome::NoTarget) | None => SubTarget::To(inherited),
    }
}

/// What the feed keeps of a display name (module docs "Name").
#[derive(Debug, Clone, PartialEq, Eq)]
enum NameSource {
    /// A string-table reference.
    Reference { mode: u8, token: u32, table: u32 },
    /// A literal, already rendered for display and cut to [`MAX_NAME_LEN`].
    Literal(String),
}

impl NameSource {
    fn of(text: &LocalizedTokenText, data: &SchemaView) -> Self {
        match &text.content {
            LocalizedTokenTextContent::Reference {
                mode,
                token_id,
                table_did,
            } => Self::Reference {
                mode: *mode,
                token: *token_id,
                table: *table_did,
            },
            // The install's resolver shows a literal (`string-table.md` §9.0 step 1); only
            // the cut result is kept.
            LocalizedTokenTextContent::Literal(_) => Self::Literal(
                data.resolve(text)
                    .map(|s| clean_display_name(&s, MAX_NAME_LEN))
                    .unwrap_or_default(),
            ),
        }
    }

    /// The name to show, `None` when a reference does not resolve.
    fn render(&self, data: &SchemaView) -> Option<String> {
        match self {
            Self::Literal(s) => Some(s.clone()),
            Self::Reference { mode, token, table } => {
                let text = LocalizedTokenText {
                    content: LocalizedTokenTextContent::Reference {
                        mode: *mode,
                        token_id: *token,
                        table_did: *table,
                    },
                    authoring_strings: None,
                    arguments: Vec::new(),
                };
                data.resolve(&text)
                    .map(|s| clean_display_name(&s, MAX_NAME_LEN))
            }
        }
    }

    #[cfg(test)]
    fn kept_bytes(&self) -> usize {
        match self {
            Self::Literal(s) => s.len(),
            Self::Reference { .. } => 0,
        }
    }
}

/// Sets `slot` to `value`; whether it changed.
fn set<T: PartialEq>(slot: &mut T, value: T) -> bool {
    if *slot == value {
        false
    } else {
        *slot = value;
        true
    }
}

/// The facts of one entity the neutral model carries (module docs).
#[derive(Debug, Clone, Default, PartialEq)]
struct Facts {
    name: Option<NameSource>,
    npc: Option<bool>,
    prop: Option<bool>,
    species: Option<u32>,
    sex: Option<u32>,
}

impl Facts {
    /// Takes `value` for `pid` when it is one of [`FEED_PIDS`] with the expected kind;
    /// whether a fact changed.
    fn apply(&mut self, pid: u32, value: &EncodedPropertyValue, data: &SchemaView) -> bool {
        match (pid, value) {
            (PID_ENTITY_DISPLAY_NAME, EncodedPropertyValue::LocalizedTokenText(t)) => {
                set(&mut self.name, Some(NameSource::of(t, data)))
            }
            (PID_IS_NPC_AGENT, EncodedPropertyValue::Boolean(b)) => set(&mut self.npc, Some(*b)),
            (PID_IS_INANIMATE_PROP, EncodedPropertyValue::Boolean(b)) => {
                set(&mut self.prop, Some(*b))
            }
            (PID_AGENT_SPECIES, EncodedPropertyValue::Enum(v)) => set(&mut self.species, Some(*v)),
            (PID_SEX, EncodedPropertyValue::Enum(v)) => set(&mut self.sex, Some(*v)),
            _ => false,
        }
    }

    /// Forgets `pid` (`0x0E`, `0xE7`); whether a fact changed.
    fn clear(&mut self, pid: u32) -> bool {
        match pid {
            PID_ENTITY_DISPLAY_NAME => set(&mut self.name, None),
            PID_IS_NPC_AGENT => set(&mut self.npc, None),
            PID_IS_INANIMATE_PROP => set(&mut self.prop, None),
            PID_AGENT_SPECIES => set(&mut self.species, None),
            PID_SEX => set(&mut self.sex, None),
            _ => false,
        }
    }
}

/// The wire facts over the baseline, by reference.
#[derive(Debug, Clone, Copy)]
struct View<'a> {
    name: Option<&'a NameSource>,
    npc: Option<bool>,
    prop: Option<bool>,
    species: Option<u32>,
    sex: Option<u32>,
}

impl<'a> View<'a> {
    fn of(wire: &'a Facts, base: Option<&'a Facts>) -> Self {
        Self {
            name: wire.name.as_ref().or_else(|| base?.name.as_ref()),
            npc: wire.npc.or_else(|| base?.npc),
            prop: wire.prop.or_else(|| base?.prop),
            species: wire.species.or_else(|| base?.species),
            sex: wire.sex.or_else(|| base?.sex),
        }
    }

    /// The kind and whether neither flag was known (module docs "Kind").
    fn kind(&self) -> (EntityKind, bool) {
        if self.prop == Some(true) {
            return (EntityKind::Object, false);
        }
        match self.npc {
            Some(true) => (EntityKind::NonPlayer, false),
            Some(false) => (EntityKind::Player, false),
            None => (EntityKind::NonPlayer, true),
        }
    }

    fn appearance(&self, kind: EntityKind, template: u32, scale: Option<f32>) -> Appearance {
        let body = match (kind, self.species, self.sex) {
            (EntityKind::Player, Some(species), Some(sex)) if species != 0 && sex != 0 => {
                Body::Character { species, sex }
            }
            _ => Body::Object(DataId(template)),
        };
        let mut appearance = Appearance::new(body);
        if let Some(s) = scale {
            appearance.scale = s;
        }
        appearance
    }
}

/// Where an entity is in its life.
#[derive(Debug, Clone, PartialEq)]
enum State {
    /// Waiting for its first position.
    Pending,
    /// Placed; its creation follows at the next flush (or a later one, module docs
    /// "Baseline").
    Placed,
    /// The client was told: the kind and appearance it was shown.
    Shown {
        kind: EntityKind,
        appearance: Appearance,
    },
}

/// One tracked entity.
#[derive(Debug, Clone)]
struct Tracked {
    /// Order of its `0x98` (also tells apart two entities with one id over time).
    seq: u64,
    template: u32,
    wire: Facts,
    /// The template's baseline, once read (at the creation).
    baseline: Option<Facts>,
    scale: Option<f32>,
    state: State,
    /// The last place and heading.
    last: Option<(Location, f32)>,
    /// Queued in [`EntityFeed::dirty`] for this flush.
    dirty: bool,
    /// Index of its move in [`EntityFeed::moves`] for this flush.
    move_slot: Option<usize>,
}

/// One move recorded for the next flush.
#[derive(Debug, Clone)]
struct Move {
    id: u64,
    seq: u64,
    location: Location,
    heading: f32,
    velocity: [f32; 3],
}

/// The other entities of one world (module docs).
#[derive(Debug)]
pub struct EntityFeed {
    player: Option<u64>,
    entities: HashMap<u64, Tracked>,
    next_seq: u64,
    /// `(id, seq)` in `0x98` order; entries of entities no longer waiting are skipped.
    pending: VecDeque<(u64, u64)>,
    /// Entities in [`State::Pending`].
    pending_live: usize,
    /// `(id, seq)` of entities placed since the last flush (or put off by it).
    placed: Vec<(u64, u64)>,
    /// `(id, seq)` of shown entities whose appearance may have changed.
    dirty: Vec<(u64, u64)>,
    moves: Vec<Move>,
    queried: HashSet<u64>,
    loads_left: usize,
    counters: FeedCounters,
}

impl Default for EntityFeed {
    fn default() -> Self {
        Self {
            player: None,
            entities: HashMap::new(),
            next_seq: 0,
            pending: VecDeque::new(),
            pending_live: 0,
            placed: Vec::new(),
            dirty: Vec::new(),
            moves: Vec::new(),
            queried: HashSet::new(),
            loads_left: MAX_TEMPLATE_LOADS_PER_PASS,
            counters: FeedCounters::default(),
        }
    }
}

/// What one input produced.
#[derive(Debug, Default)]
pub struct FeedOutput {
    /// Neutral events.
    pub events: Vec<Event>,
    /// Messages to send on the world connection (`0xD0`).
    pub outgoing: Vec<OutgoingEvent>,
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

/// The velocity of a path's move from `from` to `to` (module docs "Paths"); zero across
/// regions or for no distance.
fn path_velocity(from: &Location, to: &Location) -> [f32; 3] {
    if from.region != to.region {
        return [0.0; 3];
    }
    let d = [to.x - from.x, to.y - from.y, to.z - from.z];
    let dist = (d[0] * d[0] + d[1] * d[1] + d[2] * d[2]).sqrt();
    if !dist.is_finite() || dist < 1e-3 {
        return [0.0; 3];
    }
    let speed = (dist / PATH_NOMINAL_SECS).min(MAX_PATH_SPEED);
    #[allow(clippy::cast_possible_truncation)]
    d.map(|c| (c / dist * speed) as f32)
}

/// Whether `t` is the entity of the queued entry `seq`.
fn same(t: &Tracked, seq: u64) -> bool {
    t.seq == seq
}

impl EntityFeed {
    /// A feed for a new world with the local `player` (`None` outside the world): every
    /// entity of the previous world is forgotten (the client drops them with the world,
    /// `Event::ReturnedToCharacterSelect`), and the `0xD0` record starts again.
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
        self.entities
            .get(&id)
            .is_some_and(|t| matches!(t.state, State::Shown { .. }))
    }

    /// Whether `id` waits for its first position.
    #[must_use]
    pub fn is_pending(&self, id: u64) -> bool {
        self.entities
            .get(&id)
            .is_some_and(|t| t.state == State::Pending)
    }

    /// Entities tracked (created, placed or pending); at most [`MAX_ENTITIES`].
    #[must_use]
    pub fn tracked(&self) -> usize {
        self.entities.len()
    }

    /// Entities waiting for their first position; at most [`MAX_PENDING`].
    #[must_use]
    pub fn pending(&self) -> usize {
        self.pending_live
    }

    /// Whether creations wait for a later pass (module docs "Baseline").
    #[must_use]
    pub fn has_deferred(&self) -> bool {
        !self.placed.is_empty()
    }

    /// A driver pass begins: the template read budget is renewed.
    pub fn begin_pass(&mut self) {
        self.loads_left = MAX_TEMPLATE_LOADS_PER_PASS;
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

    /// The oldest entity still waiting for a position is dropped; whether there was one.
    /// Placed and shown entities are never dropped.
    fn evict_oldest_pending(&mut self) -> bool {
        while let Some((id, seq)) = self.pending.pop_front() {
            if self
                .entities
                .get(&id)
                .is_some_and(|t| same(t, seq) && t.state == State::Pending)
            {
                self.entities.remove(&id);
                self.pending_live = self.pending_live.saturating_sub(1);
                self.counters.pending_dropped += 1;
                return true;
            }
        }
        false
    }

    /// `0x98` (§5.2.1): a new entity, waiting for its position.
    pub fn create(&mut self, c: EntityCreate, out: &mut FeedOutput) -> CreateOutcome {
        let id = c.entity_id;
        if self.is_player(id) {
            self.counters.own_player += 1;
            return CreateOutcome::Player(id);
        }
        if id == DEFAULT_RECIPIENT {
            self.counters.no_target += 1;
            return CreateOutcome::NoTarget;
        }
        if self.entities.contains_key(&id) {
            self.counters.recreated += 1;
            self.remove(id, out);
        }
        // Waiting entities make room first; placed and shown ones never do.
        if self.pending_live >= MAX_PENDING || self.entities.len() >= MAX_ENTITIES {
            self.evict_oldest_pending();
        }
        if self.entities.len() >= MAX_ENTITIES {
            self.counters.overflow += 1;
            return CreateOutcome::Refused;
        }
        let seq = self.next_seq;
        self.next_seq += 1;
        self.entities.insert(
            id,
            Tracked {
                seq,
                template: c.template_did,
                wire: Facts::default(),
                baseline: None,
                scale: None,
                state: State::Pending,
                last: None,
                dirty: false,
                move_slot: None,
            },
        );
        self.pending.push_back((id, seq));
        self.pending_live += 1;
        if self.pending.len() > 2 * MAX_PENDING {
            let entities = &self.entities;
            self.pending.retain(|(id, seq)| {
                entities
                    .get(id)
                    .is_some_and(|t| same(t, *seq) && t.state == State::Pending)
            });
        }
        CreateOutcome::Tracked(id)
    }

    fn remove(&mut self, id: u64, out: &mut FeedOutput) -> bool {
        let Some(t) = self.entities.remove(&id) else {
            return false;
        };
        // Its queued entries are skipped later (their seq names no tracked entity).
        match t.state {
            State::Pending => {
                self.pending_live = self.pending_live.saturating_sub(1);
                false
            }
            State::Placed => false,
            State::Shown { .. } => {
                self.counters.despawned += 1;
                out.events.push(Event::EntityDespawned { id: EntityId(id) });
                true
            }
        }
    }

    /// `0x40` (§5.3) for `recipient`.
    pub fn despawn(&mut self, recipient: u64, out: &mut FeedOutput) {
        if self.is_player(recipient) {
            self.counters.player_despawns += 1;
            return;
        }
        if recipient == DEFAULT_RECIPIENT {
            self.counters.no_target += 1;
            return;
        }
        if !self.remove(recipient, out) {
            self.counters.despawn_unknown += 1;
        }
    }

    /// The entity for an entity-scoped event to `recipient`, counting the events that have
    /// none.
    fn target(&mut self, recipient: u64, out: &mut FeedOutput) -> Option<&mut Tracked> {
        if self.is_player(recipient) {
            self.counters.player_property_events += 1;
            return None;
        }
        if !self.entities.contains_key(&recipient) {
            if recipient == DEFAULT_RECIPIENT {
                self.counters.no_target += 1;
            } else {
                self.unknown_entity(recipient, out);
            }
            return None;
        }
        self.entities.get_mut(&recipient)
    }

    /// After facts of `id` changed: a shown entity is queued for an appearance check, and a
    /// name change of a shown entity is counted.
    fn touched(&mut self, id: u64, changed: bool, name_changed: bool) {
        let Some(t) = self.entities.get_mut(&id) else {
            return;
        };
        if !matches!(t.state, State::Shown { .. }) {
            return;
        }
        if name_changed {
            self.counters.name_changes_unshown += 1;
        }
        if changed && !t.dirty {
            t.dirty = true;
            self.dirty.push((id, t.seq));
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
        let Some(t) = self.target(recipient, out) else {
            return;
        };
        let (mut changed, mut name_changed) = (false, false);
        for e in entries {
            if let Some(v) = &e.value
                && t.wire.apply(e.pid, v, data)
            {
                changed = true;
                name_changed |= e.pid == PID_ENTITY_DISPLAY_NAME;
            }
        }
        self.touched(recipient, changed, name_changed);
    }

    /// `0x0E` / `0xE7` (§5.8.1) for `recipient`.
    pub fn remove_property(&mut self, recipient: u64, pid: u32, out: &mut FeedOutput) {
        let Some(t) = self.target(recipient, out) else {
            return;
        };
        let changed = t.wire.clear(pid);
        self.touched(
            recipient,
            changed,
            changed && pid == PID_ENTITY_DISPLAY_NAME,
        );
    }

    /// `0x89` (§5.7) for `recipient` (module docs "Appearance updates").
    pub fn visual(&mut self, recipient: u64, desc: &EntityAppearanceDesc, out: &mut FeedOutput) {
        let skipped = u64::from(desc.secondary_did.is_some())
            + u64::from(desc.param_a.is_some())
            + u64::from(desc.tint_entries.is_some())
            + u64::from(desc.primary_part.is_some())
            + u64::from(desc.part_map.is_some())
            + u64::from(desc.param_b.is_some());
        self.counters.appearance_fields_skipped += skipped;
        let Some(t) = self.target(recipient, out) else {
            return;
        };
        let (scale, odd) = match desc.scale {
            Some([x, y, z])
                if x == y && y == z && x.is_finite() && x > 0.0 && x <= MAX_APPEARANCE_SCALE =>
            {
                (Some(x), false)
            }
            Some(_) => (t.scale, true),
            None => (None, false),
        };
        let changed = set(&mut t.scale, scale);
        if odd {
            self.counters.appearance_fields_skipped += 1;
        }
        self.touched(recipient, changed, false);
    }

    /// A position for `id` (`0xCE`, `0xBF`): places an entity that waits for one (the
    /// creation follows at [`Self::flush`]) or records the move of a created one. Returns
    /// `false` for an entity the session does not know (counted, `0xD0` queued).
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
        let Some((location, heading)) = at else {
            self.counters.unplaced += 1;
            return true;
        };
        t.last = Some((location, heading));
        match t.state {
            State::Pending => {
                t.state = State::Placed;
                self.pending_live = self.pending_live.saturating_sub(1);
                self.placed.push((id, t.seq));
            }
            State::Placed => {}
            State::Shown { .. } => {
                let m = Move {
                    id,
                    seq: t.seq,
                    location,
                    heading,
                    velocity,
                };
                // Only the last move of the message is kept.
                match t.move_slot.and_then(|i| self.moves.get_mut(i)) {
                    Some(slot) => *slot = m,
                    None => {
                        t.move_slot = Some(self.moves.len());
                        self.moves.push(m);
                    }
                }
            }
        }
        true
    }

    /// A `0x59` path for `id`: a move to its destination (module docs "Paths").
    pub fn path(&mut self, id: u64, waypoints: &[Location], out: &mut FeedOutput) {
        if self.is_player(id) {
            return;
        }
        let Some(dest) = waypoints.last() else {
            self.counters.empty_paths += 1;
            return;
        };
        let last = self.entities.get(&id).and_then(|t| t.last);
        let heading = waypoints
            .len()
            .checked_sub(2)
            .and_then(|i| waypoints.get(i))
            .and_then(|a| heading_between(a, dest))
            .or(last.map(|(_, h)| h))
            .unwrap_or(0.0);
        let velocity = last.map_or([0.0; 3], |(from, _)| path_velocity(&from, dest));
        if self.place(id, Some((*dest, heading)), velocity, out) {
            self.counters.paths += 1;
        }
    }

    /// The template baseline of `id`, read when it is not yet (module docs "Baseline");
    /// `false` when the read is put off to a later pass.
    fn ensure_baseline(&mut self, id: u64, data: &SchemaView) -> bool {
        let Some(t) = self.entities.get(&id) else {
            return false;
        };
        if t.baseline.is_some() {
            return true;
        }
        let template = t.template;
        let mut base = Facts::default();
        if template != 0 {
            if !data.0.template_cached(template) {
                if self.loads_left == 0 {
                    self.counters.template_loads_deferred += 1;
                    return false;
                }
                self.loads_left -= 1;
            }
            for pid in FEED_PIDS {
                if let Some(v) = data.0.template_value(template, pid) {
                    base.apply(pid, &v, data);
                }
            }
        }
        if let Some(t) = self.entities.get_mut(&id) {
            t.baseline = Some(base);
        }
        true
    }

    /// The creation of the placed entity `id` (module docs "Created, not spawned").
    fn show(&mut self, id: u64, data: &SchemaView, out: &mut FeedOutput) {
        let Some(t) = self.entities.get(&id) else {
            return;
        };
        let Some((location, heading)) = t.last else {
            return;
        };
        let view = View::of(&t.wire, t.baseline.as_ref());
        let (kind, unknown) = view.kind();
        let name = view.name.map(|n| n.render(data));
        let appearance = view.appearance(kind, t.template, t.scale);
        let template = t.template;
        if unknown {
            self.counters.unclassified += 1;
        }
        let name = match name {
            Some(Some(n)) => n,
            Some(None) => {
                self.counters.unresolved_names += 1;
                String::new()
            }
            None => String::new(),
        };
        if let Some(t) = self.entities.get_mut(&id) {
            t.state = State::Shown {
                kind,
                appearance: appearance.clone(),
            };
        }
        self.counters.created += 1;
        out.events.push(Event::EntityCreated {
            id: EntityId(id),
            kind,
            name,
            class: (template != 0).then_some(DataId(template)),
            location,
            heading,
            appearance: Box::new(appearance),
            tag: None,
        });
    }

    /// An `AppearanceChanged` for the shown entity `id` when its appearance changed; a kind
    /// change is counted (module docs "Name").
    fn refresh(&mut self, id: u64, out: &mut FeedOutput) {
        let Some(t) = self.entities.get_mut(&id) else {
            return;
        };
        t.dirty = false;
        let view = View::of(&t.wire, t.baseline.as_ref());
        let (kind, _) = view.kind();
        let appearance = view.appearance(kind, t.template, t.scale);
        let State::Shown {
            kind: shown_kind,
            appearance: shown,
        } = &mut t.state
        else {
            return;
        };
        if *shown_kind != kind {
            *shown_kind = kind;
            self.counters.kind_changes_unshown += 1;
        }
        if appearance != *shown {
            shown.clone_from(&appearance);
            self.counters.appearance_changes += 1;
            out.events.push(Event::AppearanceChanged {
                id: EntityId(id),
                appearance: Box::new(appearance),
            });
        }
    }

    /// The events the message just read completed (module docs "One flush per message"):
    /// the creations of the placed entities whose template is read, in the order of their
    /// `0x98`, then one appearance change and one move per entity.
    pub fn flush(&mut self, data: &SchemaView, out: &mut FeedOutput) {
        if !self.placed.is_empty() {
            let mut placed = std::mem::take(&mut self.placed);
            let entities = &self.entities;
            placed.retain(|(id, seq)| {
                entities
                    .get(id)
                    .is_some_and(|t| same(t, *seq) && t.state == State::Placed)
            });
            placed.sort_unstable_by_key(|(_, seq)| *seq);
            let mut later = Vec::new();
            for (id, seq) in placed {
                if self.ensure_baseline(id, data) {
                    self.show(id, data, out);
                } else {
                    later.push((id, seq));
                }
            }
            self.placed = later;
        }
        for (id, seq) in std::mem::take(&mut self.dirty) {
            if self.entities.get(&id).is_some_and(|t| same(t, seq)) {
                self.refresh(id, out);
            }
        }
        for m in std::mem::take(&mut self.moves) {
            let Some(t) = self.entities.get_mut(&m.id) else {
                continue;
            };
            if !same(t, m.seq) || !matches!(t.state, State::Shown { .. }) {
                continue;
            }
            t.move_slot = None;
            out.events
                .push(moved(m.id, &m.location, m.heading, m.velocity));
        }
    }

    /// The bytes kept for the name of `id` (tests of the memory bound).
    #[cfg(test)]
    fn name_bytes(&self, id: u64) -> Option<usize> {
        let t = self.entities.get(&id)?;
        Some(t.wire.name.as_ref().map_or(0, NameSource::kept_bytes))
    }
}
