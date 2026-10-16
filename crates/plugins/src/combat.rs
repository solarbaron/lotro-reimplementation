// SPDX-License-Identifier: GPL-3.0-or-later
//! `Turbine.Gameplay` combat views: actors' morale, power, level and target, effect lists
//! and effects, `LocalPlayer:IsInCombat()`, and the trained-skill list with its skills,
//! fed by a [`CombatSource`] the client implements and by [`CombatViewEvent`]s it queues
//! with [`crate::PluginManager::raise_combat_events`].
//!
//! Spec: `lua-plugin-api.md` §15 (queued delivery of game-raised events, frame-loop step
//! 7), §15.2 (objects, actor values, combat state, effect lists and values, skills, leaving
//! the world), §9.3 (argument fields), §18 checks 70–76; `plugin-api-usage.md` §7.4
//! (empty views, the placeholder skill of §7.4.1 rule 1).
//!
//! The host never reads the install for these views: the client precomputes every value
//! (rendered names, record properties, the game time an effect arrived or a cooldown ends)
//! and the host applies the API rules that are its own: object identity per apartment,
//! index ranges and errors, the placeholder skill, and delivery only to view objects that
//! existed when an event was raised (§15.2: each object records the gameplay sequence
//! number at its creation). [`CombatModel`] is a ready-made source that derives the events
//! of §15.2 from session-level inputs; a client may use it or implement the trait itself.

use std::collections::{BTreeMap, HashMap};
use std::sync::atomic::Ordering;
use std::sync::{Arc, Mutex};

use mlua::{Lua, MultiValue, Table, Value};
use tessera_script::{api_fn, host_error};

use crate::apartment::Apartment;
use crate::gameplay::PLACEHOLDER_SKILL_NAME;
use crate::inventory::{new_object, weak_table, with_payload};
use crate::object::{Payload, native_class_of};
use crate::party::{self, MemberLink};
use crate::shared::{ApartmentCtx, lock};

// ---------------------------------------------------------------------------------------
// Values and the source trait
// ---------------------------------------------------------------------------------------

/// What kind of entity an object stands for; it picks the object's class (§15.2
/// "Objects": `Player`, `Actor`, or a plain `Entity` without combat getters).
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum ActorKind {
    /// Another player character (class `Player`).
    Player,
    /// A non-player character (class `Actor`).
    NonPlayer,
    /// A creature (class `Actor`).
    Creature,
    /// An inanimate object (class `Entity`).
    Object,
}

/// The values the client keeps for one entity (`combat-ui.md` §11, "kept"), as the
/// actor getters show them. Vitals are the feed's `f32` values converted exactly.
#[derive(Debug, Clone, PartialEq)]
pub struct ActorValues {
    /// The entity's kind.
    pub kind: ActorKind,
    /// `GetName()`: the name its spawn carried (empty when none).
    pub name: String,
    /// `GetMorale()`: `current` of the last health `VitalChanged` (0 before one).
    pub morale: f64,
    /// `GetMaxMorale()` and `GetBaseMaxMorale()`: `max` of that event.
    pub max_morale: f64,
    /// `GetPower()`.
    pub power: f64,
    /// `GetMaxPower()` and `GetBaseMaxPower()`.
    pub max_power: f64,
    /// `GetLevel()`.
    pub level: u32,
    /// The entity's target (for the player: the client's current selection).
    pub target: Option<u64>,
}

impl ActorValues {
    /// The values before any report: zero vitals, level 0, no target.
    #[must_use]
    pub fn new(kind: ActorKind, name: impl Into<String>) -> Self {
        Self {
            kind,
            name: name.into(),
            morale: 0.0,
            max_morale: 0.0,
            power: 0.0,
            max_power: 0.0,
            level: 0,
            target: None,
        }
    }
}

/// One effect instance as the `Effect` getters show it (§15.2 "Effect values"),
/// precomputed by the client from the effect record (read with inheritance) or a literal
/// ref; absent properties are given as empty, 0 or false.
#[derive(Debug, Clone, PartialEq, Default)]
pub struct EffectData {
    /// `GetID()`: the instance's `EffectId`.
    pub id: u32,
    /// `GetName()`: rendered `Effect_Name`, the literal text, or empty.
    pub name: String,
    /// `GetDescription()`: rendered `Effect_Applied_Description`, or empty.
    pub description: String,
    /// `GetIcon()`: `Effect_Icon` (0 when absent or literal).
    pub icon: u32,
    /// `GetCategory()`: the stored `Effect_Resist_Category` value, unchanged.
    pub category: u32,
    /// `IsDebuff()`: `Effect_Debuff`, or for a literal ref the harmful rule.
    pub debuff: bool,
    /// `IsCurable()`: `Effect_IsCurable`.
    pub curable: bool,
    /// `GetStartTime()`: the game time (§12) of the frame that received the effect.
    pub start_time: f64,
    /// `GetDuration()`: `duration_ms` ÷ 1000, 0 without a duration.
    pub duration: f64,
}

impl EffectData {
    /// The change events of an in-place update from `self` to `new`, in the order of
    /// §15.2 (P91); never `IsDispellableChanged`. `GetID` cannot change.
    #[must_use]
    pub fn changes(&self, new: &Self) -> Vec<EffectEvent> {
        let mut v = Vec::new();
        if self.name != new.name {
            v.push(EffectEvent::NameChanged);
        }
        if self.description != new.description {
            v.push(EffectEvent::DescriptionChanged);
        }
        if self.icon != new.icon {
            v.push(EffectEvent::IconChanged);
        }
        if self.curable != new.curable {
            v.push(EffectEvent::IsCurableChanged);
        }
        if self.debuff != new.debuff {
            v.push(EffectEvent::IsDebuffChanged);
        }
        if differs(self.start_time, new.start_time) {
            v.push(EffectEvent::StartTimeChanged);
        }
        if differs(self.duration, new.duration) {
            v.push(EffectEvent::DurationChanged);
        }
        if self.category != new.category {
            v.push(EffectEvent::CategoryChanged);
        }
        v
    }
}

/// A skill reference: one `Skill` object per reference per apartment.
#[derive(Debug, Clone, PartialEq, Eq, Hash, PartialOrd, Ord)]
pub enum SkillKey {
    /// A skill record (its DID).
    Record(u32),
    /// A literal label.
    Literal(String),
}

impl SkillKey {
    fn registry_key(&self) -> String {
        match self {
            Self::Record(did) => format!("r:{did}"),
            Self::Literal(s) => format!("l:{s}"),
        }
    }

    fn from_registry_key(k: &str) -> Option<Self> {
        if let Some(d) = k.strip_prefix("r:") {
            return d.parse().ok().map(Self::Record);
        }
        k.strip_prefix("l:").map(|s| Self::Literal(s.to_owned()))
    }
}

/// One learned skill as the `Skill` and `SkillInfo` getters show it (§15.2 "Skills"),
/// precomputed by the client from the skill record or a literal ref.
#[derive(Debug, Clone, PartialEq)]
pub struct SkillData {
    /// The reference.
    pub key: SkillKey,
    /// `SkillInfo:GetName()`: rendered `Skill_Name`, the literal text, or empty.
    pub name: String,
    /// `SkillInfo:GetDescription()`: rendered `Skill_Desc`, or empty.
    pub description: String,
    /// `SkillInfo:GetIconImageID()`: `Skill_SmallIcon`, else `Skill_Icon`, else 0.
    pub icon_image_id: u32,
    /// `ActiveSkill:GetCooldown()` and `GetBaseCooldown()`: `Skill_SkillRecoveryTime` in
    /// seconds (0 when absent or literal).
    pub cooldown: f64,
}

/// What the client implements to feed the combat views (read on every getter call, on
/// the plugin thread). Entity IDs are the session's; effect positions and skill positions
/// are 0-based here (the API's are 1-based).
pub trait CombatSource: Send + Sync + 'static {
    /// The player's entity, while a character is in the world (kept after leaving, so
    /// that its vitals and level keep their last values).
    fn player(&self) -> Option<u64>;
    /// The kept values of entity `id`, while the client knows it (spawned and not
    /// despawned; always for the player).
    fn actor(&self, id: u64) -> Option<ActorValues>;
    /// `LocalPlayer:IsInCombat()`.
    fn in_combat(&self) -> bool;
    /// The number of active effect instances of entity `entity`.
    fn effect_count(&self, entity: u64) -> u32;
    /// The effect at 0-based position `index` of `entity`'s list.
    fn effect_at(&self, entity: u64, index: u32) -> Option<Arc<EffectData>>;
    /// The current values of effect instance `id` of `entity`, while it is in the list.
    fn effect(&self, entity: u64, id: u32) -> Option<Arc<EffectData>>;
    /// The 0-based position of effect instance `id` in `entity`'s list.
    fn effect_position(&self, entity: u64, id: u32) -> Option<u32>;
    /// The number of learned skills (0: the list shows the placeholder skill).
    fn skill_count(&self) -> u32;
    /// The learned skill at 0-based position `index`.
    fn skill_at(&self, index: u32) -> Option<Arc<SkillData>>;
    /// The learned skill with reference `key`.
    fn skill(&self, key: &SkillKey) -> Option<Arc<SkillData>>;
    /// The game time at which the running cooldown of `key` ends, or `None`.
    fn reset_time(&self, key: &SkillKey) -> Option<f64>;
    /// The final values of entity `id` after the client forgot it (despawned, or left
    /// behind on leaving the world), while the source keeps them; a held object answers
    /// with them (§15.2 "Objects"). The default keeps none: a held object then answers
    /// with the values it last read.
    fn last_actor(&self, id: u64) -> Option<ActorValues> {
        let _ = id;
        None
    }
    /// The final values of effect instance `id` of `entity` after it left the list
    /// (removed or cleared), while the source keeps them; a held `Effect` answers with
    /// them. The default keeps none.
    fn last_effect(&self, entity: u64, id: u32) -> Option<Arc<EffectData>> {
        let _ = (entity, id);
        None
    }
}

// ---------------------------------------------------------------------------------------
// Events
// ---------------------------------------------------------------------------------------

/// Which entity's object an actor or effect-list event goes to.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum ViewEntity {
    /// The `LocalPlayer.GetInstance()` object.
    LocalPlayer,
    /// Another entity's object (the `LocalPlayer` object when it is the player's).
    Entity(u64),
}

/// An actor's `…Changed` event (§15.2 "Actor values"; empty argument table).
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
#[allow(missing_docs)]
pub enum ActorEvent {
    MoraleChanged,
    MaxMoraleChanged,
    BaseMaxMoraleChanged,
    PowerChanged,
    MaxPowerChanged,
    BaseMaxPowerChanged,
    LevelChanged,
    TargetChanged,
}

impl ActorEvent {
    /// The event's field name.
    #[must_use]
    pub fn name(self) -> &'static str {
        match self {
            Self::MoraleChanged => "MoraleChanged",
            Self::MaxMoraleChanged => "MaxMoraleChanged",
            Self::BaseMaxMoraleChanged => "BaseMaxMoraleChanged",
            Self::PowerChanged => "PowerChanged",
            Self::MaxPowerChanged => "MaxPowerChanged",
            Self::BaseMaxPowerChanged => "BaseMaxPowerChanged",
            Self::LevelChanged => "LevelChanged",
            Self::TargetChanged => "TargetChanged",
        }
    }
}

/// An `Effect`'s `…Changed` event (§15.2, in-place update; empty argument table).
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
#[allow(missing_docs)]
pub enum EffectEvent {
    NameChanged,
    DescriptionChanged,
    IconChanged,
    IsCurableChanged,
    IsDebuffChanged,
    StartTimeChanged,
    DurationChanged,
    CategoryChanged,
}

impl EffectEvent {
    /// The event's field name.
    #[must_use]
    pub fn name(self) -> &'static str {
        match self {
            Self::NameChanged => "NameChanged",
            Self::DescriptionChanged => "DescriptionChanged",
            Self::IconChanged => "IconChanged",
            Self::IsCurableChanged => "IsCurableChanged",
            Self::IsDebuffChanged => "IsDebuffChanged",
            Self::StartTimeChanged => "StartTimeChanged",
            Self::DurationChanged => "DurationChanged",
            Self::CategoryChanged => "CategoryChanged",
        }
    }
}

/// A combat-view event of `lua-plugin-api.md` §15.2, raised by the client at the change
/// and delivered in step 7 to the view objects that existed when it was raised.
#[derive(Debug, Clone, PartialEq)]
pub enum CombatViewEvent {
    /// An actor's `…Changed` event on its entity object.
    Actor {
        /// The entity.
        entity: ViewEntity,
        /// The event.
        event: ActorEvent,
    },
    /// `LocalPlayer.InCombatChanged`.
    InCombatChanged,
    /// `EffectList.EffectAdded { Index }` on the entity's effect list.
    EffectAdded {
        /// The entity.
        entity: ViewEntity,
        /// `Index`: the new count.
        index: u32,
    },
    /// `EffectList.EffectRemoved { Index }`.
    EffectRemoved {
        /// The entity.
        entity: ViewEntity,
        /// `Index`: the 1-based position before the removal.
        index: u32,
    },
    /// `EffectList.EffectsCleared` (empty argument table).
    EffectsCleared {
        /// The entity.
        entity: ViewEntity,
    },
    /// An `Effect`'s `…Changed` event, on the object of (`entity`, `effect`).
    EffectChanged {
        /// The entity.
        entity: u64,
        /// The `EffectId`.
        effect: u32,
        /// The event.
        event: EffectEvent,
    },
    /// `SkillList.SkillAdded { Index, Skill }` on the trained-skill list.
    SkillAdded {
        /// `Index`: the new count.
        index: u32,
        /// The skill's values; `None` for the placeholder skill.
        skill: Option<Arc<SkillData>>,
    },
    /// `SkillList.SkillRemoved { Index }`.
    SkillRemoved {
        /// `Index`: the 1-based position before the removal.
        index: u32,
    },
    /// The trained-skill list returning to the placeholder (leaving the world with
    /// `count` learned skills): delivered as `SkillRemoved { Index }` for `count`,
    /// `count` − 1, …, 1, then `SkillAdded { Index = 1 }` with the placeholder skill. It is
    /// one queue item, so that a long list never overflows the host's event queue part
    /// way through.
    SkillsReset {
        /// The number of learned skills removed.
        count: u32,
    },
    /// `ActiveSkill.ResetTimeChanged` on the skill object of `skill`.
    ResetTimeChanged {
        /// The skill (or item) reference of the cooldown.
        skill: SkillKey,
    },
}

/// Whether two values differ (NaN equals NaN, so a NaN never repeats an event).
fn differs(a: f64, b: f64) -> bool {
    !(a == b || (a.is_nan() && b.is_nan()))
}

// ---------------------------------------------------------------------------------------
// The ready-made model
// ---------------------------------------------------------------------------------------

/// Most entities a [`CombatModel`] keeps (further spawns are ignored).
pub const MAX_ACTORS: usize = 65_536;
/// Most effect instances a [`CombatModel`] keeps per entity (further ones are ignored).
pub const MAX_EFFECTS_PER_ACTOR: usize = 1024;
/// Most learned skills a [`CombatModel`] keeps.
pub const MAX_SKILLS: usize = 4096;
/// Most running cooldowns a [`CombatModel`] keeps.
pub const MAX_COOLDOWNS: usize = 4096;
/// Most effect instances a [`CombatModel`] keeps over all entities.
pub const MAX_EFFECTS_TOTAL: usize = 65_536;
/// Most forgotten entities whose final values a [`CombatModel`] keeps for held objects
/// (the oldest are dropped first).
pub const MAX_RETIRED_ACTORS: usize = 4096;
/// Most removed effect instances whose final values a [`CombatModel`] keeps for held
/// objects (the oldest are dropped first).
pub const MAX_RETIRED_EFFECTS: usize = 4096;

/// Final values of subjects that left the model, kept for the objects plugins hold; past
/// `CAP` entries the oldest is dropped.
#[derive(Debug)]
struct Retired<K, V, const CAP: usize> {
    next: u64,
    map: HashMap<K, (u64, V)>,
    order: BTreeMap<u64, K>,
}

impl<K, V, const CAP: usize> Default for Retired<K, V, CAP> {
    fn default() -> Self {
        Self {
            next: 0,
            map: HashMap::new(),
            order: BTreeMap::new(),
        }
    }
}

impl<K: Clone + Eq + std::hash::Hash, V, const CAP: usize> Retired<K, V, CAP> {
    fn insert(&mut self, key: K, value: V) {
        self.remove(&key);
        let seq = self.next;
        self.next += 1;
        self.order.insert(seq, key.clone());
        self.map.insert(key, (seq, value));
        while self.map.len() > CAP {
            let Some((_, old)) = self.order.pop_first() else {
                break;
            };
            self.map.remove(&old);
        }
    }

    fn remove(&mut self, key: &K) {
        if let Some((seq, _)) = self.map.remove(key) {
            self.order.remove(&seq);
        }
    }

    fn get(&self, key: &K) -> Option<&V> {
        self.map.get(key).map(|(_, v)| v)
    }

    #[cfg(test)]
    fn len(&self) -> usize {
        self.map.len()
    }
}

/// A vital the combat views show.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum CombatVital {
    /// Health (morale).
    Health,
    /// Power.
    Power,
}

/// The client's combat state for the plugin views, with the §15.2 rules that derive the
/// [`CombatViewEvent`]s from session events. Every input returns the events to queue, in
/// order. `Mutex<CombatModel>` implements [`CombatSource`].
///
/// Inputs for entities the model does not know (not spawned, not the player) change
/// nothing, except that a target is kept as reported. The final values of forgotten
/// entities and removed effect instances are kept for the objects plugins hold
/// ([`CombatSource::last_actor`], [`CombatSource::last_effect`]). All collections are
/// bounded ([`MAX_ACTORS`], [`MAX_EFFECTS_PER_ACTOR`], [`MAX_EFFECTS_TOTAL`],
/// [`MAX_SKILLS`], [`MAX_COOLDOWNS`], [`MAX_RETIRED_ACTORS`], [`MAX_RETIRED_EFFECTS`]).
///
/// Raise the events an input returns after releasing the model's lock: the plugin host
/// may read the source while it queues them.
#[derive(Debug, Default)]
pub struct CombatModel {
    player: Option<u64>,
    in_world: bool,
    actors: HashMap<u64, ActorValues>,
    effects: BTreeMap<u64, Vec<Arc<EffectData>>>,
    effects_total: usize,
    in_combat: bool,
    skills: Vec<Arc<SkillData>>,
    cooldowns: HashMap<SkillKey, f64>,
    retired_actors: Retired<u64, ActorValues, MAX_RETIRED_ACTORS>,
    retired_effects: Retired<(u64, u32), Arc<EffectData>, MAX_RETIRED_EFFECTS>,
}

impl CombatModel {
    /// An empty model (no character in the world).
    #[must_use]
    pub fn new() -> Self {
        Self::default()
    }

    fn who(&self, id: u64) -> ViewEntity {
        if self.player == Some(id) {
            ViewEntity::LocalPlayer
        } else {
            ViewEntity::Entity(id)
        }
    }

    /// `WorldEntered` for the character with entity `player`, its name and the
    /// `CharacterInfo.level` it carried. A second entry first leaves the world
    /// ([`Self::left_world`]). The player's values start fresh: zero vitals and the given
    /// level (0 without one).
    pub fn world_entered(
        &mut self,
        player: u64,
        name: &str,
        level: Option<u32>,
    ) -> Vec<CombatViewEvent> {
        let ev = if self.in_world {
            self.left_world()
        } else {
            Vec::new()
        };
        if let Some(old) = self.player
            && old != player
        {
            self.forget_actor(old);
        }
        self.retired_actors.remove(&player);
        let mut rec = ActorValues::new(ActorKind::Player, name);
        rec.level = level.unwrap_or(0);
        self.actors.insert(player, rec);
        self.player = Some(player);
        self.in_world = true;
        ev
    }

    /// `Disconnected` (or the leaving part of a second `WorldEntered`): every non-empty
    /// effect list is emptied with `EffectsCleared`, the selection becomes nil
    /// (`TargetChanged`), combat ends (`InCombatChanged`), the trained-skill list returns
    /// to the placeholder ([`CombatViewEvent::SkillsReset`]); vitals and levels keep their
    /// last values (§15.2 "Leaving the world"). Running cooldowns end
    /// (`ResetTimeChanged` each, in key order; a Tessera choice, the section is silent).
    /// Other entities are forgotten (their final values stay readable for held objects).
    pub fn left_world(&mut self) -> Vec<CombatViewEvent> {
        let mut ev = Vec::new();
        for (id, list) in std::mem::take(&mut self.effects) {
            if !list.is_empty() {
                ev.push(CombatViewEvent::EffectsCleared {
                    entity: self.who(id),
                });
            }
            self.retire_effects(id, list);
        }
        self.effects_total = 0;
        if let Some(p) = self.player
            && let Some(rec) = self.actors.get_mut(&p)
            && rec.target.take().is_some()
        {
            ev.push(CombatViewEvent::Actor {
                entity: ViewEntity::LocalPlayer,
                event: ActorEvent::TargetChanged,
            });
        }
        if std::mem::take(&mut self.in_combat) {
            ev.push(CombatViewEvent::InCombatChanged);
        }
        if !self.skills.is_empty() {
            ev.push(CombatViewEvent::SkillsReset {
                count: self.skills.len() as u32,
            });
            self.skills.clear();
        }
        let mut keys: Vec<SkillKey> = self.cooldowns.drain().map(|(k, _)| k).collect();
        keys.sort();
        ev.extend(
            keys.into_iter()
                .map(|skill| CombatViewEvent::ResetTimeChanged { skill }),
        );
        let others: Vec<u64> = self
            .actors
            .keys()
            .copied()
            .filter(|id| Some(*id) != self.player)
            .collect();
        for id in others {
            self.forget_actor(id);
        }
        self.in_world = false;
        ev
    }

    /// Forget entity `id`, keeping its final values for held objects.
    fn forget_actor(&mut self, id: u64) {
        if let Some(rec) = self.actors.remove(&id) {
            self.retired_actors.insert(id, rec);
        }
    }

    /// Keep the final values of `entity`'s effect instances that left its list.
    fn retire_effects(&mut self, entity: u64, list: Vec<Arc<EffectData>>) {
        for e in list {
            self.retired_effects.insert((entity, e.id), e);
        }
    }

    /// `EntitySpawned`/`EntityCreated` of entity `id`: the model keeps its values from
    /// now on. A repeated spawn updates the kind and name and keeps the values.
    pub fn entity_spawned(&mut self, id: u64, kind: ActorKind, name: &str) {
        if self.player == Some(id) {
            return;
        }
        if let Some(rec) = self.actors.get_mut(&id) {
            rec.kind = kind;
            name.clone_into(&mut rec.name);
        } else if self.actors.len() < MAX_ACTORS {
            self.retired_actors.remove(&id);
            self.actors.insert(id, ActorValues::new(kind, name));
        } else {
            tracing::warn!(id, "combat model: entity limit reached; spawn ignored");
        }
    }

    /// `EntityDespawned` of entity `id`: its kept effects are dropped (`EffectsCleared`
    /// when there were some) and, when it was the selection, the selection becomes nil
    /// (`TargetChanged` on the player). The entity is forgotten (unless it is the
    /// player); its final values stay readable for held objects.
    pub fn entity_despawned(&mut self, id: u64) -> Vec<CombatViewEvent> {
        let mut ev = Vec::new();
        if let Some(list) = self.effects.remove(&id) {
            self.effects_total = self.effects_total.saturating_sub(list.len());
            if !list.is_empty() {
                ev.push(CombatViewEvent::EffectsCleared {
                    entity: self.who(id),
                });
            }
            self.retire_effects(id, list);
        }
        if self.player == Some(id) {
            return ev;
        }
        if let Some(p) = self.player
            && let Some(rec) = self.actors.get_mut(&p)
            && rec.target == Some(id)
        {
            rec.target = None;
            ev.push(CombatViewEvent::Actor {
                entity: ViewEntity::LocalPlayer,
                event: ActorEvent::TargetChanged,
            });
        }
        self.forget_actor(id);
        ev
    }

    /// `VitalChanged` for health or power: the maximum's events first
    /// (`MaxMoraleChanged`, `BaseMaxMoraleChanged`), then the current value's, each only
    /// when its value changed.
    pub fn vital_changed(
        &mut self,
        id: u64,
        vital: CombatVital,
        current: f32,
        max: f32,
    ) -> Vec<CombatViewEvent> {
        let entity = self.who(id);
        let Some(rec) = self.actors.get_mut(&id) else {
            return Vec::new();
        };
        let (cur_slot, max_slot, events) = match vital {
            CombatVital::Health => (
                &mut rec.morale,
                &mut rec.max_morale,
                [
                    ActorEvent::MaxMoraleChanged,
                    ActorEvent::BaseMaxMoraleChanged,
                    ActorEvent::MoraleChanged,
                ],
            ),
            CombatVital::Power => (
                &mut rec.power,
                &mut rec.max_power,
                [
                    ActorEvent::MaxPowerChanged,
                    ActorEvent::BaseMaxPowerChanged,
                    ActorEvent::PowerChanged,
                ],
            ),
        };
        let (current, max) = (f64::from(current), f64::from(max));
        let mut ev = Vec::new();
        if differs(*max_slot, max) {
            *max_slot = max;
            ev.extend(
                events[..2]
                    .iter()
                    .map(|&event| CombatViewEvent::Actor { entity, event }),
            );
        }
        if differs(*cur_slot, current) {
            *cur_slot = current;
            ev.push(CombatViewEvent::Actor {
                entity,
                event: events[2],
            });
        }
        ev
    }

    /// `LevelChanged` of entity `id`.
    pub fn level_changed(&mut self, id: u64, level: u32) -> Vec<CombatViewEvent> {
        let entity = self.who(id);
        match self.actors.get_mut(&id) {
            Some(rec) if rec.level != level => {
                rec.level = level;
                vec![CombatViewEvent::Actor {
                    entity,
                    event: ActorEvent::LevelChanged,
                }]
            }
            _ => Vec::new(),
        }
    }

    /// `TargetChanged` of entity `id` (for the player: the selection, also when the
    /// client selects locally). The target is kept as reported: `GetTarget()` resolves
    /// it at the call, so a target the client has not seen spawned reads as nil until
    /// it spawns.
    pub fn target_changed(&mut self, id: u64, target: Option<u64>) -> Vec<CombatViewEvent> {
        let entity = self.who(id);
        match self.actors.get_mut(&id) {
            Some(rec) if rec.target != target => {
                rec.target = target;
                vec![CombatViewEvent::Actor {
                    entity,
                    event: ActorEvent::TargetChanged,
                }]
            }
            _ => Vec::new(),
        }
    }

    /// `CombatState` of entity `id`; only the player's reaches a view.
    pub fn combat_state(&mut self, id: u64, in_combat: bool) -> Vec<CombatViewEvent> {
        if self.player != Some(id) || self.in_combat == in_combat {
            return Vec::new();
        }
        self.in_combat = in_combat;
        vec![CombatViewEvent::InCombatChanged]
    }

    /// `EffectApplied` on `entity` with the instance's values: appended with
    /// `EffectAdded { Index = new count }`, or, for an instance already in the list,
    /// updated in place with one event per changed value (§15.2 effect rule 1).
    pub fn effect_applied(&mut self, entity: u64, effect: EffectData) -> Vec<CombatViewEvent> {
        if !self.actors.contains_key(&entity) {
            return Vec::new();
        }
        let who = self.who(entity);
        let total = self.effects_total;
        let list = self.effects.entry(entity).or_default();
        if let Some(pos) = list.iter().position(|e| e.id == effect.id) {
            let changes = list[pos].changes(&effect);
            let id = effect.id;
            list[pos] = Arc::new(effect);
            return changes
                .into_iter()
                .map(|event| CombatViewEvent::EffectChanged {
                    entity,
                    effect: id,
                    event,
                })
                .collect();
        }
        if list.len() >= MAX_EFFECTS_PER_ACTOR || total >= MAX_EFFECTS_TOTAL {
            tracing::warn!(entity, "combat model: effect limit reached; effect ignored");
            return Vec::new();
        }
        let id = effect.id;
        list.push(Arc::new(effect));
        let index = list.len() as u32;
        self.effects_total += 1;
        self.retired_effects.remove(&(entity, id));
        vec![CombatViewEvent::EffectAdded { entity: who, index }]
    }

    /// `EffectRemoved` of instance `effect` on `entity`: `EffectRemoved { Index }` with
    /// its position, then the last instance moves into its place (§15.2 effect rule 2).
    pub fn effect_removed(&mut self, entity: u64, effect: u32) -> Vec<CombatViewEvent> {
        let who = self.who(entity);
        let Some(list) = self.effects.get_mut(&entity) else {
            return Vec::new();
        };
        let Some(pos) = list.iter().position(|e| e.id == effect) else {
            return Vec::new();
        };
        let gone = list.swap_remove(pos);
        self.effects_total = self.effects_total.saturating_sub(1);
        self.retired_effects.insert((entity, effect), gone);
        vec![CombatViewEvent::EffectRemoved {
            entity: who,
            index: pos as u32 + 1,
        }]
    }

    /// `SkillLearned`: appended in the order of first report (`SkillAdded`); the first
    /// one replaces the placeholder (`SkillRemoved { 1 }` first). A reference already
    /// listed changes nothing.
    pub fn skill_learned(&mut self, skill: SkillData) -> Vec<CombatViewEvent> {
        if self.skills.iter().any(|s| s.key == skill.key) {
            return Vec::new();
        }
        if self.skills.len() >= MAX_SKILLS {
            tracing::warn!("combat model: skill limit reached; skill ignored");
            return Vec::new();
        }
        let mut ev = Vec::new();
        if self.skills.is_empty() {
            ev.push(CombatViewEvent::SkillRemoved { index: 1 });
        }
        let skill = Arc::new(skill);
        self.skills.push(Arc::clone(&skill));
        ev.push(CombatViewEvent::SkillAdded {
            index: self.skills.len() as u32,
            skill: Some(skill),
        });
        ev
    }

    /// `SkillForgotten`: `SkillRemoved { Index }` with its position, the later skills
    /// move up; forgetting the last one brings the placeholder back (`SkillAdded { 1 }`).
    pub fn skill_forgotten(&mut self, key: &SkillKey) -> Vec<CombatViewEvent> {
        let Some(pos) = self.skills.iter().position(|s| &s.key == key) else {
            return Vec::new();
        };
        self.skills.remove(pos);
        let mut ev = vec![CombatViewEvent::SkillRemoved {
            index: pos as u32 + 1,
        }];
        if self.skills.is_empty() {
            ev.push(CombatViewEvent::SkillAdded {
                index: 1,
                skill: None,
            });
        }
        ev
    }

    /// `CooldownStarted` of `key`, ending at game time `ends_at` (the game time of the
    /// frame that received it plus `duration_ms` ÷ 1000): `ResetTimeChanged`.
    pub fn cooldown_started(&mut self, key: SkillKey, ends_at: f64) -> Vec<CombatViewEvent> {
        if !self.cooldowns.contains_key(&key) && self.cooldowns.len() >= MAX_COOLDOWNS {
            tracing::warn!("combat model: cooldown limit reached; cooldown ignored");
            return Vec::new();
        }
        self.cooldowns.insert(key.clone(), ends_at);
        vec![CombatViewEvent::ResetTimeChanged { skill: key }]
    }

    /// `CooldownEnded` of `key`: `ResetTimeChanged` (`GetResetTime()` is −1 after it).
    pub fn cooldown_ended(&mut self, key: &SkillKey) -> Vec<CombatViewEvent> {
        self.cooldowns.remove(key);
        vec![CombatViewEvent::ResetTimeChanged { skill: key.clone() }]
    }
}

impl CombatSource for Mutex<CombatModel> {
    fn player(&self) -> Option<u64> {
        lock(self).player
    }
    fn actor(&self, id: u64) -> Option<ActorValues> {
        lock(self).actors.get(&id).cloned()
    }
    fn in_combat(&self) -> bool {
        lock(self).in_combat
    }
    fn effect_count(&self, entity: u64) -> u32 {
        lock(self)
            .effects
            .get(&entity)
            .map_or(0, |l| l.len() as u32)
    }
    fn effect_at(&self, entity: u64, index: u32) -> Option<Arc<EffectData>> {
        lock(self)
            .effects
            .get(&entity)?
            .get(index as usize)
            .cloned()
    }
    fn effect(&self, entity: u64, id: u32) -> Option<Arc<EffectData>> {
        lock(self)
            .effects
            .get(&entity)?
            .iter()
            .find(|e| e.id == id)
            .cloned()
    }
    fn effect_position(&self, entity: u64, id: u32) -> Option<u32> {
        lock(self)
            .effects
            .get(&entity)?
            .iter()
            .position(|e| e.id == id)
            .map(|p| p as u32)
    }
    fn skill_count(&self) -> u32 {
        lock(self).skills.len() as u32
    }
    fn skill_at(&self, index: u32) -> Option<Arc<SkillData>> {
        lock(self).skills.get(index as usize).cloned()
    }
    fn skill(&self, key: &SkillKey) -> Option<Arc<SkillData>> {
        lock(self).skills.iter().find(|s| &s.key == key).cloned()
    }
    fn reset_time(&self, key: &SkillKey) -> Option<f64> {
        lock(self).cooldowns.get(key).copied()
    }
    fn last_actor(&self, id: u64) -> Option<ActorValues> {
        lock(self).retired_actors.get(&id).cloned()
    }
    fn last_effect(&self, entity: u64, id: u32) -> Option<Arc<EffectData>> {
        lock(self).retired_effects.get(&(entity, id)).cloned()
    }
}

// ---------------------------------------------------------------------------------------
// Object identity
// ---------------------------------------------------------------------------------------

/// What a view object that receives events stands for, in the host's [`Watch`].
#[derive(Debug, Clone, PartialEq, Eq, Hash)]
pub(crate) enum WatchKey {
    /// An entity object (and so its effect list).
    Entity(u64),
    /// An `Effect` object of (entity, `EffectId`).
    Effect(u64, u32),
    /// A `Skill` (`ActiveSkill`) object.
    Skill(SkillKey),
}

/// The subjects that have a view object in some apartment, counted per object (an
/// object's payload holds a [`WatchGuard`]; the count drops when the collector frees the
/// object or its apartment closes). The host queues an event of an entity, effect or
/// skill only while its subject is here (§15.2: an event is queued only for view objects
/// that exist at the change), so changes no plugin watches never fill the queue. A
/// count can outlive its object until the collector runs; the filter then lets the event
/// through and delivery skips it, as before.
#[derive(Debug, Default)]
pub(crate) struct Watch {
    counts: Mutex<HashMap<WatchKey, u32>>,
}

/// Keeps one count of a [`Watch`] key while a view object exists.
pub(crate) struct WatchGuard {
    watch: Arc<Watch>,
    key: WatchKey,
}

impl Drop for WatchGuard {
    fn drop(&mut self) {
        let mut counts = lock(&self.watch.counts);
        if let Some(n) = counts.get_mut(&self.key) {
            *n -= 1;
            if *n == 0 {
                counts.remove(&self.key);
            }
        }
    }
}

impl Watch {
    /// Count one more object of `key`.
    pub(crate) fn register(this: &Arc<Self>, key: WatchKey) -> WatchGuard {
        let mut counts = lock(&this.counts);
        let n = counts.entry(key.clone()).or_insert(0);
        *n = n.saturating_add(1);
        WatchGuard {
            watch: Arc::clone(this),
            key,
        }
    }

    fn has(&self, key: &WatchKey) -> bool {
        lock(&self.counts).contains_key(key)
    }

    /// Whether `ev` may reach a view object: events of the `LocalPlayer`, its effect list
    /// and the trained-skill list always may; another entity's only while it has an
    /// object (or it is the player, `player` asked only then), an effect's or a skill's
    /// only while it has one.
    pub(crate) fn may_reach(
        &self,
        ev: &CombatViewEvent,
        player: impl FnOnce() -> Option<u64>,
    ) -> bool {
        use CombatViewEvent as E;
        match ev {
            E::Actor {
                entity: ViewEntity::Entity(id),
                ..
            }
            | E::EffectAdded {
                entity: ViewEntity::Entity(id),
                ..
            }
            | E::EffectRemoved {
                entity: ViewEntity::Entity(id),
                ..
            }
            | E::EffectsCleared {
                entity: ViewEntity::Entity(id),
            } => self.has(&WatchKey::Entity(*id)) || player() == Some(*id),
            E::EffectChanged { entity, effect, .. } => {
                self.has(&WatchKey::Effect(*entity, *effect))
            }
            E::ResetTimeChanged { skill } => self.has(&WatchKey::Skill(skill.clone())),
            E::Actor { .. }
            | E::InCombatChanged
            | E::EffectAdded { .. }
            | E::EffectRemoved { .. }
            | E::EffectsCleared { .. }
            | E::SkillAdded { .. }
            | E::SkillRemoved { .. }
            | E::SkillsReset { .. } => true,
        }
    }
}

/// Objects kept strongly before a prune detaches those whose subject the source no
/// longer holds (the threshold then doubles past the surviving count).
const PRUNE_AT: i64 = 1024;

/// One kind of object kept per key per apartment: a strong table while its subject
/// exists, a weak-valued table for detached objects a plugin may still hold (§15.2
/// "Objects"; the same scheme as the inventory's `Item` objects).
struct Ident {
    strong: &'static str,
    detached: &'static str,
    count: &'static str,
    prune_at: &'static str,
}

const ENTITIES: Ident = Ident {
    strong: "tessera.plugins.combat.entities",
    detached: "tessera.plugins.combat.entities.detached",
    count: "tessera.plugins.combat.entities.count",
    prune_at: "tessera.plugins.combat.entities.prune",
};

const EFFECTS: Ident = Ident {
    strong: "tessera.plugins.combat.effects",
    detached: "tessera.plugins.combat.effects.detached",
    count: "tessera.plugins.combat.effects.count",
    prune_at: "tessera.plugins.combat.effects.prune",
};

const SKILLS: Ident = Ident {
    strong: "tessera.plugins.combat.skills",
    detached: "tessera.plugins.combat.skills.detached",
    count: "tessera.plugins.combat.skills.count",
    prune_at: "tessera.plugins.combat.skills.prune",
};

impl Ident {
    fn install(&self, lua: &Lua) -> mlua::Result<()> {
        lua.set_named_registry_value(self.strong, lua.create_table()?)?;
        lua.set_named_registry_value(self.detached, weak_table(lua, "v")?)?;
        lua.set_named_registry_value(self.count, 0_i64)?;
        lua.set_named_registry_value(self.prune_at, PRUNE_AT)
    }

    /// The existing object of `key` (strong or detached), without creating or moving it.
    fn existing(&self, lua: &Lua, key: &str) -> mlua::Result<Option<Table>> {
        for name in [self.strong, self.detached] {
            let t: Table = lua.named_registry_value(name)?;
            if let Value::Table(obj) = t.raw_get::<Value>(key)? {
                return Ok(Some(obj));
            }
        }
        Ok(None)
    }

    /// The object of `key`, created by `make` on first use; a detached one becomes
    /// strong again. `held` tells a prune whose subject the source still holds.
    fn get(
        &self,
        lua: &Lua,
        key: &str,
        make: impl FnOnce() -> mlua::Result<Table>,
        held: &dyn Fn(&str) -> bool,
    ) -> mlua::Result<Table> {
        let strong: Table = lua.named_registry_value(self.strong)?;
        if let Value::Table(t) = strong.raw_get::<Value>(key)? {
            return Ok(t);
        }
        let detached: Table = lua.named_registry_value(self.detached)?;
        let obj = if let Value::Table(t) = detached.raw_get::<Value>(key)? {
            detached.raw_set(key, Value::Nil)?;
            t
        } else {
            make()?
        };
        strong.raw_set(key, obj.clone())?;
        let count = lua.named_registry_value::<i64>(self.count)? + 1;
        lua.set_named_registry_value(self.count, count)?;
        if count >= lua.named_registry_value::<i64>(self.prune_at)? {
            self.prune(lua, held)?;
        }
        Ok(obj)
    }

    /// Drop the apartment's object of `key` from both tables (a plugin holding it keeps
    /// it); the next [`Self::get`] creates a new one.
    fn forget(&self, lua: &Lua, key: &str) -> mlua::Result<()> {
        let strong: Table = lua.named_registry_value(self.strong)?;
        if let Value::Table(_) = strong.raw_get::<Value>(key)? {
            strong.raw_set(key, Value::Nil)?;
            let count = lua.named_registry_value::<i64>(self.count)? - 1;
            lua.set_named_registry_value(self.count, count.max(0))?;
        }
        let detached: Table = lua.named_registry_value(self.detached)?;
        detached.raw_set(key, Value::Nil)
    }

    fn prune(&self, lua: &Lua, held: &dyn Fn(&str) -> bool) -> mlua::Result<()> {
        let strong: Table = lua.named_registry_value(self.strong)?;
        let detached: Table = lua.named_registry_value(self.detached)?;
        let mut gone = Vec::new();
        let mut kept: i64 = 0;
        for pair in strong.pairs::<String, Value>() {
            let (k, obj) = pair?;
            if held(&k) {
                kept += 1;
            } else {
                gone.push((k, obj));
            }
        }
        for (k, obj) in gone {
            strong.raw_set(k.as_str(), Value::Nil)?;
            detached.raw_set(k, obj)?;
        }
        lua.set_named_registry_value(self.count, kept)?;
        lua.set_named_registry_value(self.prune_at, PRUNE_AT.max(kept.saturating_mul(2)))
    }
}

/// Registry name of the weak-keyed table actor object → its `EffectList` object.
const LISTS_KEY: &str = "tessera.plugins.combat.lists";
/// Registry name of the `LocalPlayer`'s `EffectList` object.
const PLAYER_LIST_KEY: &str = "tessera.plugins.combat.playerlist";
/// Registry name of the apartment's trained `SkillList` object.
const TRAINED_KEY: &str = "tessera.plugins.combat.trained";
/// Registry name of the weak-valued table skill key → `SkillInfo` object.
const SKILL_INFOS_KEY: &str = "tessera.plugins.combat.skillinfos";
/// Registry name of the table of class tables the views create objects of.
const CLASSES_KEY: &str = "tessera.plugins.combat.classes";
/// Registry key of the placeholder skill.
const PLACEHOLDER_KEY: &str = "p";

/// Payload of an entity object (not the `LocalPlayer`).
struct EntityObj {
    id: u64,
    created: u64,
    last: Mutex<ActorValues>,
    _watch: Option<WatchGuard>,
}

/// Whose effects an `EffectList` shows.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum Owner {
    /// The player, whatever its entity is at the call.
    LocalPlayer,
    Entity(u64),
    /// A `PartyMember`: its member's entity at the call (`ListObj::member`), none for a
    /// member without one (§15.4).
    Member,
}

/// Payload of an `EffectList`.
struct ListObj {
    owner: Owner,
    created: u64,
    /// The `PartyMember` link of an [`Owner::Member`] list.
    member: Option<Arc<MemberLink>>,
}

/// Payload of an `Effect`.
struct EffectObj {
    entity: u64,
    id: u32,
    created: u64,
    last: Mutex<Arc<EffectData>>,
    _watch: Option<WatchGuard>,
}

/// Payload of the trained-skill `SkillList`.
struct TrainedList {
    created: u64,
}

/// Payload of a `Skill` (`ActiveSkill`) or `SkillInfo`; `key` `None` is the placeholder.
struct SkillObj {
    key: Option<SkillKey>,
    created: u64,
    last: Mutex<Option<Arc<SkillData>>>,
    _watch: Option<WatchGuard>,
}

fn source(ctx: &ApartmentCtx) -> Option<Arc<dyn CombatSource>> {
    let shared = ctx.shared().ok()?;
    lock(&shared.combat).clone()
}

/// Record a new view object of `key` in the host's [`Watch`] (while the manager exists).
fn watch(ctx: &ApartmentCtx, key: WatchKey) -> Option<WatchGuard> {
    ctx.shared()
        .ok()
        .map(|s| Watch::register(&s.combat_watch, key))
}

/// The gameplay sequence number now: objects created now receive only events raised
/// later.
pub(crate) fn current_seq(ctx: &ApartmentCtx) -> u64 {
    ctx.shared()
        .map(|s| s.gameplay_seq.load(Ordering::SeqCst))
        .unwrap_or(0)
}

fn class(lua: &Lua, name: &str) -> mlua::Result<Table> {
    let classes: Table = lua.named_registry_value(CLASSES_KEY)?;
    classes.raw_get(name)
}

/// The player's kept values, when a combat source knows the player.
pub(crate) fn player_values(ctx: &ApartmentCtx) -> Option<ActorValues> {
    let src = source(ctx)?;
    src.actor(src.player()?)
}

/// Whether a combat source knows the player (the `LocalPlayer` then exists).
pub(crate) fn has_player(ctx: &ApartmentCtx) -> bool {
    source(ctx).is_some_and(|s| s.player().is_some())
}

/// `LocalPlayer:IsInCombat()` from the combat source, when one is set and knows the
/// player (the same condition as the other `LocalPlayer` getters).
pub(crate) fn in_combat(ctx: &ApartmentCtx) -> Option<bool> {
    source(ctx)
        .filter(|s| s.player().is_some())
        .map(|s| s.in_combat())
}

/// The values of the entity object `this` (not the `LocalPlayer`): the source's while it
/// knows the entity, else the final values the source kept when it forgot it, else the
/// last the object read. `None` when `this` is no entity object.
pub(crate) fn entity_values(
    lua: &Lua,
    ctx: &ApartmentCtx,
    this: &Table,
) -> mlua::Result<Option<ActorValues>> {
    with_payload(lua, this, |p: &EntityObj| {
        let src = source(ctx);
        if let Some(now) = src
            .as_ref()
            .and_then(|s| s.actor(p.id).or_else(|| s.last_actor(p.id)))
        {
            lock(&p.last).clone_from(&now);
            return now;
        }
        lock(&p.last).clone()
    })
}

/// The apartment's object of entity `id`: the `LocalPlayer` object for the player; an
/// existing object; a new one while the source knows the entity; else nil.
fn entity_object(lua: &Lua, ctx: &ApartmentCtx, id: u64) -> mlua::Result<Value> {
    let src = source(ctx);
    if src.as_ref().and_then(|s| s.player()) == Some(id) {
        return Ok(crate::gameplay::local_player_object(lua, ctx)?.map_or(Value::Nil, Value::Table));
    }
    let key = id.to_string();
    let Some(values) = src.as_ref().and_then(|s| s.actor(id)) else {
        return Ok(ENTITIES
            .existing(lua, &key)?
            .map_or(Value::Nil, Value::Table));
    };
    let held = |k: &str| {
        k.parse::<u64>()
            .ok()
            .zip(src.as_ref())
            .is_some_and(|(id, s)| s.actor(id).is_some())
    };
    let cls = class(
        lua,
        match values.kind {
            ActorKind::Player => "Player",
            ActorKind::NonPlayer | ActorKind::Creature => "Actor",
            ActorKind::Object => "Entity",
        },
    )?;
    // The kind picks the class: an object of another class (the entity respawned, or
    // its spawn was repeated, with another kind) is replaced by a new one.
    if let Some(old) = ENTITIES.existing(lua, &key)?
        && native_class_of(lua, &old)?.is_some_and(|c| c.to_pointer() != cls.to_pointer())
    {
        ENTITIES.forget(lua, &key)?;
    }
    let obj = ENTITIES.get(
        lua,
        &key,
        || {
            new_object(
                lua,
                &cls,
                Payload(Arc::new(EntityObj {
                    id,
                    created: current_seq(ctx),
                    last: Mutex::new(values.clone()),
                    _watch: watch(ctx, WatchKey::Entity(id)),
                })),
            )
        },
        &held,
    )?;
    Ok(Value::Table(obj))
}

/// `Actor:GetTarget()` (§15.2): for the `LocalPlayer` (`is_player`) the selection, for an
/// entity object its target, for a `PartyMember` the target of its member's entity (§15.4;
/// nil for a member without one); nil for other objects.
pub(crate) fn target(
    lua: &Lua,
    ctx: &ApartmentCtx,
    this: &Table,
    is_player: bool,
) -> mlua::Result<Value> {
    let values = if is_player {
        player_values(ctx)
    } else {
        match entity_values(lua, ctx, this)? {
            Some(v) => Some(v),
            None => party::member_link(lua, this)?
                .and_then(|m| m.entity(ctx))
                .zip(source(ctx))
                .and_then(|(id, s)| s.actor(id).or_else(|| s.last_actor(id))),
        }
    };
    match values.and_then(|v| v.target) {
        Some(id) => entity_object(lua, ctx, id),
        None => Ok(Value::Nil),
    }
}

/// `Actor:GetEffects()` (§15.2): one `EffectList` per actor object. `None` for an object
/// that is neither the `LocalPlayer`, an entity object nor a `PartyMember`.
pub(crate) fn effects(
    lua: &Lua,
    ctx: &ApartmentCtx,
    this: &Table,
    is_player: bool,
) -> mlua::Result<Option<Table>> {
    let cls = class(lua, "EffectList")?;
    if is_player {
        if let Value::Table(t) = lua.named_registry_value::<Value>(PLAYER_LIST_KEY)? {
            return Ok(Some(t));
        }
        let t = new_object(
            lua,
            &cls,
            Payload(Arc::new(ListObj {
                owner: Owner::LocalPlayer,
                created: current_seq(ctx),
                member: None,
            })),
        )?;
        lua.set_named_registry_value(PLAYER_LIST_KEY, t.clone())?;
        return Ok(Some(t));
    }
    // An entity object's list, or a `PartyMember`'s (§15.4: the `Actor` getters of the
    // member's entity; one list per member object, read live, with no events of its own).
    let (owner, member) = match with_payload(lua, this, |p: &EntityObj| p.id)? {
        Some(id) => (Owner::Entity(id), None),
        None => match party::member_link(lua, this)? {
            Some(link) => (Owner::Member, Some(link)),
            None => return Ok(None),
        },
    };
    let lists: Table = lua.named_registry_value(LISTS_KEY)?;
    if let Value::Table(t) = lists.raw_get::<Value>(this.clone())? {
        return Ok(Some(t));
    }
    let t = new_object(
        lua,
        &cls,
        Payload(Arc::new(ListObj {
            owner,
            created: current_seq(ctx),
            member,
        })),
    )?;
    lists.raw_set(this.clone(), t.clone())?;
    Ok(Some(t))
}

/// The entity an `EffectList` shows now (`None`: an empty list).
fn list_entity(lua: &Lua, ctx: &ApartmentCtx, this: &Table) -> mlua::Result<Option<u64>> {
    Ok(with_payload(lua, this, |p: &ListObj| match p.owner {
        Owner::LocalPlayer => source(ctx).and_then(|s| s.player()),
        Owner::Entity(id) => Some(id),
        Owner::Member => p.member.as_ref().and_then(|m| m.entity(ctx)),
    })?
    .flatten())
}

fn effect_key(entity: u64, id: u32) -> String {
    format!("{entity}:{id}")
}

/// The apartment's `Effect` object of (`entity`, `data.id`); its last values become
/// `data`.
fn effect_object(
    lua: &Lua,
    ctx: &ApartmentCtx,
    entity: u64,
    data: &Arc<EffectData>,
) -> mlua::Result<Table> {
    let src = source(ctx);
    let held = |k: &str| {
        k.split_once(':')
            .and_then(|(e, i)| Some((e.parse::<u64>().ok()?, i.parse::<u32>().ok()?)))
            .zip(src.as_ref())
            .is_some_and(|((e, i), s)| s.effect(e, i).is_some())
    };
    let obj = EFFECTS.get(
        lua,
        &effect_key(entity, data.id),
        || {
            new_object(
                lua,
                &class(lua, "Effect")?,
                Payload(Arc::new(EffectObj {
                    entity,
                    id: data.id,
                    created: current_seq(ctx),
                    last: Mutex::new(Arc::clone(data)),
                    _watch: watch(ctx, WatchKey::Effect(entity, data.id)),
                })),
            )
        },
        &held,
    )?;
    with_payload(lua, &obj, |p: &EffectObj| *lock(&p.last) = Arc::clone(data))?;
    Ok(obj)
}

/// The values of the `Effect` object `this` (the source's while the instance is in its
/// list, else the final values the source kept when it left, else the last the object
/// read) with its entity; `None` for other objects.
fn effect_values(
    lua: &Lua,
    ctx: &ApartmentCtx,
    this: &Table,
) -> mlua::Result<Option<(u64, Arc<EffectData>)>> {
    with_payload(lua, this, |p: &EffectObj| {
        let src = source(ctx);
        if let Some(now) = src.as_ref().and_then(|s| {
            s.effect(p.entity, p.id)
                .or_else(|| s.last_effect(p.entity, p.id))
        }) {
            *lock(&p.last) = Arc::clone(&now);
            return (p.entity, now);
        }
        (p.entity, Arc::clone(&lock(&p.last)))
    })
}

/// A 1-based index argument in 1 ..= `count` as a 0-based position; `None` for any other
/// value (not a number, not whole, out of range).
fn position_arg(v: &Value, count: u32) -> Option<u32> {
    let x = match *v {
        Value::Integer(i) => i as f64,
        Value::Number(n) => n,
        _ => return None,
    };
    if x.fract() != 0.0 || x < 1.0 || x > f64::from(count) {
        return None;
    }
    Some(x as u32 - 1)
}

fn skill_registry_key(key: Option<&SkillKey>) -> String {
    key.map_or_else(|| PLACEHOLDER_KEY.to_owned(), SkillKey::registry_key)
}

/// The apartment's `Skill` object (class `ActiveSkill`) of `key` (`None`: the
/// placeholder); its last values become `data` when given.
fn skill_object(
    lua: &Lua,
    ctx: &ApartmentCtx,
    key: Option<&SkillKey>,
    data: Option<&Arc<SkillData>>,
) -> mlua::Result<Table> {
    let src = source(ctx);
    let held = |k: &str| {
        k == PLACEHOLDER_KEY
            || SkillKey::from_registry_key(k)
                .zip(src.as_ref())
                .is_some_and(|(key, s)| s.skill(&key).is_some())
    };
    let obj = SKILLS.get(
        lua,
        &skill_registry_key(key),
        || {
            new_object(
                lua,
                &class(lua, "ActiveSkill")?,
                Payload(Arc::new(SkillObj {
                    key: key.cloned(),
                    created: current_seq(ctx),
                    last: Mutex::new(data.cloned()),
                    _watch: key.and_then(|k| watch(ctx, WatchKey::Skill(k.clone()))),
                })),
            )
        },
        &held,
    )?;
    if let Some(data) = data {
        with_payload(lua, &obj, |p: &SkillObj| {
            *lock(&p.last) = Some(Arc::clone(data));
        })?;
    }
    Ok(obj)
}

/// The key and values of a `Skill` or `SkillInfo` object: the source's while the skill
/// is learned, else the last seen; the placeholder (and an object without a skill
/// payload) gives `(None, None)`.
fn skill_values(
    lua: &Lua,
    ctx: &ApartmentCtx,
    this: &Table,
) -> mlua::Result<(Option<SkillKey>, Option<Arc<SkillData>>)> {
    Ok(with_payload(lua, this, |p: &SkillObj| {
        let Some(key) = &p.key else {
            return (None, None);
        };
        if let Some(now) = source(ctx).and_then(|s| s.skill(key)) {
            *lock(&p.last) = Some(Arc::clone(&now));
            return (Some(key.clone()), Some(now));
        }
        (Some(key.clone()), lock(&p.last).clone())
    })?
    .unwrap_or((None, None)))
}

/// The apartment's `SkillInfo` object of `key` (`None`: the placeholder), one per skill
/// reference per apartment, made with `data` as its last-seen values (§15.2 "Skills"; also
/// the `UntrainedSkill` objects of §15.6).
pub(crate) fn skill_info_object(
    lua: &Lua,
    key: Option<SkillKey>,
    data: Option<Arc<SkillData>>,
) -> mlua::Result<Table> {
    let infos: Table = lua.named_registry_value(SKILL_INFOS_KEY)?;
    let rkey = skill_registry_key(key.as_ref());
    if let Value::Table(t) = infos.raw_get::<Value>(rkey.as_str())? {
        if let Some(data) = data {
            with_payload(lua, &t, |p: &SkillObj| *lock(&p.last) = Some(data))?;
        }
        return Ok(t);
    }
    let t = new_object(
        lua,
        &class(lua, "SkillInfo")?,
        Payload(Arc::new(SkillObj {
            key,
            created: 0,
            last: Mutex::new(data),
            _watch: None,
        })),
    )?;
    infos.raw_set(rkey, t.clone())?;
    Ok(t)
}

/// `LocalPlayer:GetTrainedSkills()`: one `SkillList` per apartment.
pub(crate) fn trained_skills(lua: &Lua, ctx: &ApartmentCtx) -> mlua::Result<Table> {
    if let Value::Table(t) = lua.named_registry_value::<Value>(TRAINED_KEY)? {
        return Ok(t);
    }
    let t = new_object(
        lua,
        &class(lua, "SkillList")?,
        Payload(Arc::new(TrainedList {
            created: current_seq(ctx),
        })),
    )?;
    lua.set_named_registry_value(TRAINED_KEY, t.clone())?;
    Ok(t)
}

fn trained_count(ctx: &ApartmentCtx) -> u32 {
    source(ctx).map_or(0, |s| s.skill_count()).max(1)
}

/// `SkillList:GetCount()` of the trained list (`None` for other skill lists).
pub(crate) fn trained_list_count(
    lua: &Lua,
    ctx: &ApartmentCtx,
    this: &Table,
) -> mlua::Result<Option<u32>> {
    Ok(with_payload(lua, this, |_: &TrainedList| ())?.map(|()| trained_count(ctx)))
}

/// `SkillList:GetItem(i)` of the trained list (`None` for other skill lists); raises for
/// an index outside 1 … count.
pub(crate) fn trained_list_item(
    lua: &Lua,
    ctx: &ApartmentCtx,
    this: &Table,
    i: &Value,
) -> mlua::Result<Option<Table>> {
    if with_payload(lua, this, |_: &TrainedList| ())?.is_none() {
        return Ok(None);
    }
    let Some(pos) = position_arg(i, trained_count(ctx)) else {
        return Err(host_error("SkillList.GetItem: index out of range"));
    };
    let skill = source(ctx).and_then(|s| {
        if s.skill_count() == 0 {
            None
        } else {
            s.skill_at(pos)
        }
    });
    match skill {
        Some(data) => skill_object(lua, ctx, Some(&data.key), Some(&data)).map(Some),
        None => skill_object(lua, ctx, None, None).map(Some),
    }
}

// ---------------------------------------------------------------------------------------
// Installation
// ---------------------------------------------------------------------------------------

/// The class tables the combat views fill or create objects of.
pub(crate) struct Classes<'a> {
    pub entity: &'a Table,
    pub actor: &'a Table,
    pub player: &'a Table,
    pub effect_list: &'a Table,
    pub effect: &'a Table,
    pub skill_list: &'a Table,
    pub skill: &'a Table,
    pub active_skill: &'a Table,
    pub skill_info: &'a Table,
}

/// Install the methods of `EffectList`, `Effect`, `Skill`, `ActiveSkill` and `SkillInfo`
/// into a new apartment (the actor getters, `GetTarget`, `GetEffects`, `IsInCombat` and
/// the skill lists dispatch here from `gameplay`).
pub(crate) fn install(
    lua: &Lua,
    ctx: &Arc<ApartmentCtx>,
    classes: &Classes<'_>,
) -> mlua::Result<()> {
    ENTITIES.install(lua)?;
    EFFECTS.install(lua)?;
    SKILLS.install(lua)?;
    lua.set_named_registry_value(LISTS_KEY, weak_table(lua, "k")?)?;
    lua.set_named_registry_value(SKILL_INFOS_KEY, weak_table(lua, "v")?)?;
    let cls = lua.create_table()?;
    for (name, t) in [
        ("Entity", classes.entity),
        ("Actor", classes.actor),
        ("Player", classes.player),
        ("EffectList", classes.effect_list),
        ("Effect", classes.effect),
        ("SkillList", classes.skill_list),
        ("ActiveSkill", classes.active_skill),
        ("SkillInfo", classes.skill_info),
    ] {
        cls.raw_set(name, t.clone())?;
    }
    lua.set_named_registry_value(CLASSES_KEY, cls)?;
    install_effect_list(lua, ctx, classes.effect_list)?;
    install_effect(lua, ctx, classes.effect)?;
    install_skills(lua, ctx, classes)
}

fn install_effect_list(lua: &Lua, ctx: &Arc<ApartmentCtx>, list: &Table) -> mlua::Result<()> {
    let c = Arc::clone(ctx);
    list.raw_set(
        "GetCount",
        api_fn(lua, "GetCount", move |lua, this: Table| {
            let n = match (list_entity(lua, &c, &this)?, source(&c)) {
                (Some(e), Some(s)) => s.effect_count(e),
                _ => 0,
            };
            Ok(f64::from(n))
        })?,
    )?;
    let c = Arc::clone(ctx);
    list.raw_set(
        "Get",
        api_fn(lua, "Get", move |lua, (this, i): (Table, Value)| {
            let found = match (list_entity(lua, &c, &this)?, source(&c)) {
                (Some(e), Some(s)) => position_arg(&i, s.effect_count(e))
                    .and_then(|pos| s.effect_at(e, pos))
                    .map(|d| (e, d)),
                _ => None,
            };
            match found {
                Some((e, d)) => effect_object(lua, &c, e, &d),
                None => Err(host_error("EffectList.Get: index out of range")),
            }
        })?,
    )?;
    let c = Arc::clone(ctx);
    list.raw_set(
        "IndexOf",
        api_fn(lua, "IndexOf", move |lua, (this, e): (Table, Value)| {
            Ok(index_of(lua, &c, &this, &e)?.map_or(-1.0, |p| f64::from(p) + 1.0))
        })?,
    )?;
    let c = Arc::clone(ctx);
    list.raw_set(
        "Contains",
        api_fn(lua, "Contains", move |lua, (this, e): (Table, Value)| {
            Ok(index_of(lua, &c, &this, &e)?.is_some())
        })?,
    )
}

/// The 0-based position of the `Effect` object `e` in the list `this`, if it is there.
fn index_of(lua: &Lua, ctx: &ApartmentCtx, this: &Table, e: &Value) -> mlua::Result<Option<u32>> {
    let Value::Table(e) = e else {
        return Ok(None);
    };
    let Some((entity, id)) = with_payload(lua, e, |p: &EffectObj| (p.entity, p.id))? else {
        return Ok(None);
    };
    if list_entity(lua, ctx, this)? != Some(entity) {
        return Ok(None);
    }
    Ok(source(ctx).and_then(|s| s.effect_position(entity, id)))
}

fn install_effect(lua: &Lua, ctx: &Arc<ApartmentCtx>, effect: &Table) -> mlua::Result<()> {
    type Num = fn(&EffectData) -> f64;
    let numbers: [(&str, Num); 5] = [
        ("GetID", |d| f64::from(d.id)),
        ("GetIcon", |d| f64::from(d.icon)),
        ("GetCategory", |d| f64::from(d.category)),
        ("GetStartTime", |d| d.start_time),
        ("GetDuration", |d| d.duration),
    ];
    for (m, get) in numbers {
        let c = Arc::clone(ctx);
        effect.raw_set(
            m,
            api_fn(lua, m, move |lua, this: Table| {
                Ok(effect_values(lua, &c, &this)?.map_or(0.0, |(_, d)| get(&d)))
            })?,
        )?;
    }
    type Flag = fn(&EffectData) -> bool;
    let flags: [(&str, Flag); 2] = [("IsDebuff", |d| d.debuff), ("IsCurable", |d| d.curable)];
    for (m, get) in flags {
        let c = Arc::clone(ctx);
        effect.raw_set(
            m,
            api_fn(lua, m, move |lua, this: Table| {
                Ok(effect_values(lua, &c, &this)?.is_some_and(|(_, d)| get(&d)))
            })?,
        )?;
    }
    type Text = fn(&EffectData) -> &str;
    let texts: [(&str, Text); 2] = [
        ("GetName", |d| d.name.as_str()),
        ("GetDescription", |d| d.description.as_str()),
    ];
    for (m, get) in texts {
        let c = Arc::clone(ctx);
        effect.raw_set(
            m,
            api_fn(lua, m, move |lua, this: Table| {
                Ok(effect_values(lua, &c, &this)?
                    .map(|(_, d)| get(&d).to_owned())
                    .unwrap_or_default())
            })?,
        )?;
    }
    Ok(())
}

fn install_skills(lua: &Lua, ctx: &Arc<ApartmentCtx>, classes: &Classes<'_>) -> mlua::Result<()> {
    // Skill:GetSkillInfo(): one SkillInfo per skill reference per apartment.
    let c = Arc::clone(ctx);
    classes.skill.raw_set(
        "GetSkillInfo",
        api_fn(lua, "GetSkillInfo", move |lua, this: Table| {
            let (key, data) = skill_values(lua, &c, &this)?;
            skill_info_object(lua, key, data)
        })?,
    )?;

    // ActiveSkill.
    let c = Arc::clone(ctx);
    classes.active_skill.raw_set(
        "GetResetTime",
        api_fn(lua, "GetResetTime", move |lua, this: Table| {
            let (key, _) = skill_values(lua, &c, &this)?;
            Ok(key
                .and_then(|k| source(&c).and_then(|s| s.reset_time(&k)))
                .unwrap_or(-1.0))
        })?,
    )?;
    for m in ["GetCooldown", "GetBaseCooldown"] {
        let c = Arc::clone(ctx);
        classes.active_skill.raw_set(
            m,
            api_fn(lua, m, move |lua, this: Table| {
                Ok(skill_values(lua, &c, &this)?.1.map_or(0.0, |d| d.cooldown))
            })?,
        )?;
    }
    classes.active_skill.raw_set(
        "IsUsable",
        api_fn(lua, "IsUsable", |_, _: MultiValue| Ok(true))?,
    )?;

    // SkillInfo: the placeholder's values for the placeholder and for objects without a
    // skill payload.
    let c = Arc::clone(ctx);
    classes.skill_info.raw_set(
        "GetName",
        api_fn(lua, "GetName", move |lua, this: Table| {
            Ok(match skill_values(lua, &c, &this)? {
                (Some(_), Some(d)) => d.name.clone(),
                (Some(_), None) => String::new(),
                (None, _) => PLACEHOLDER_SKILL_NAME.to_owned(),
            })
        })?,
    )?;
    let c = Arc::clone(ctx);
    classes.skill_info.raw_set(
        "GetDescription",
        api_fn(lua, "GetDescription", move |lua, this: Table| {
            Ok(skill_values(lua, &c, &this)?
                .1
                .map(|d| d.description.clone())
                .unwrap_or_default())
        })?,
    )?;
    let c = Arc::clone(ctx);
    classes.skill_info.raw_set(
        "GetIconImageID",
        api_fn(lua, "GetIconImageID", move |lua, this: Table| {
            Ok(skill_values(lua, &c, &this)?
                .1
                .map_or(0.0, |d| f64::from(d.icon_image_id)))
        })?,
    )?;
    classes.skill_info.raw_set(
        "GetType",
        api_fn(lua, "GetType", |_, _: MultiValue| Ok(0.0))?,
    )
}

// ---------------------------------------------------------------------------------------
// Delivery
// ---------------------------------------------------------------------------------------

/// Deliver one queued combat event (sequence number `seq`) to the apartment `ap`: to the
/// view object it names when that object existed before the event was raised.
pub(crate) fn deliver(ap: &Apartment, ev: &CombatViewEvent, seq: u64) {
    if let Err(e) = deliver_inner(ap, ev, seq) {
        ap.report(None, "delivering a combat event", &e);
    }
}

/// The created sequence number of a combat view object (the `LocalPlayer`'s is kept by
/// `gameplay`).
fn created_of(lua: &Lua, t: &Table) -> mlua::Result<Option<u64>> {
    if let Some(c) = with_payload(lua, t, |p: &EntityObj| p.created)? {
        return Ok(Some(c));
    }
    if let Some(c) = with_payload(lua, t, |p: &ListObj| p.created)? {
        return Ok(Some(c));
    }
    if let Some(c) = with_payload(lua, t, |p: &EffectObj| p.created)? {
        return Ok(Some(c));
    }
    if let Some(c) = with_payload(lua, t, |p: &SkillObj| p.created)? {
        return Ok(Some(c));
    }
    with_payload(lua, t, |p: &TrainedList| p.created)
}

/// `t` if it existed before event `seq` was raised.
fn existed(lua: &Lua, t: Option<Table>, seq: u64) -> mlua::Result<Option<Table>> {
    let Some(t) = t else { return Ok(None) };
    Ok(match created_of(lua, &t)? {
        Some(c) if c < seq => Some(t),
        _ => None,
    })
}

/// The existing actor object of `entity` that existed before event `seq`.
fn actor_target(
    lua: &Lua,
    ctx: &ApartmentCtx,
    entity: ViewEntity,
    seq: u64,
) -> mlua::Result<Option<Table>> {
    let player = match entity {
        ViewEntity::LocalPlayer => true,
        ViewEntity::Entity(id) => source(ctx).and_then(|s| s.player()) == Some(id),
    };
    if player {
        return Ok(crate::gameplay::existing_local_player(lua)?
            .filter(|(_, created)| *created < seq)
            .map(|(t, _)| t));
    }
    let ViewEntity::Entity(id) = entity else {
        return Ok(None);
    };
    existed(lua, ENTITIES.existing(lua, &id.to_string())?, seq)
}

/// The existing effect list of `entity` that existed before event `seq`.
fn list_target(
    lua: &Lua,
    ctx: &ApartmentCtx,
    entity: ViewEntity,
    seq: u64,
) -> mlua::Result<Option<Table>> {
    let player = match entity {
        ViewEntity::LocalPlayer => true,
        ViewEntity::Entity(id) => source(ctx).and_then(|s| s.player()) == Some(id),
    };
    let list = if player {
        match lua.named_registry_value::<Value>(PLAYER_LIST_KEY)? {
            Value::Table(t) => Some(t),
            _ => None,
        }
    } else {
        let ViewEntity::Entity(id) = entity else {
            return Ok(None);
        };
        match ENTITIES.existing(lua, &id.to_string())? {
            Some(actor) => {
                let lists: Table = lua.named_registry_value(LISTS_KEY)?;
                match lists.raw_get::<Value>(actor)? {
                    Value::Table(t) => Some(t),
                    _ => None,
                }
            }
            None => None,
        }
    };
    existed(lua, list, seq)
}

fn deliver_inner(ap: &Apartment, ev: &CombatViewEvent, seq: u64) -> mlua::Result<()> {
    let lua = ap.lua();
    let ctx = &ap.ctx;
    use CombatViewEvent as E;
    let args = lua.create_table()?;
    let (target, name) = match ev {
        E::Actor { entity, event } => (actor_target(lua, ctx, *entity, seq)?, event.name()),
        E::InCombatChanged => (
            actor_target(lua, ctx, ViewEntity::LocalPlayer, seq)?,
            "InCombatChanged",
        ),
        E::EffectAdded { entity, index } => {
            args.raw_set("Index", f64::from(*index))?;
            (list_target(lua, ctx, *entity, seq)?, "EffectAdded")
        }
        E::EffectRemoved { entity, index } => {
            args.raw_set("Index", f64::from(*index))?;
            (list_target(lua, ctx, *entity, seq)?, "EffectRemoved")
        }
        E::EffectsCleared { entity } => (list_target(lua, ctx, *entity, seq)?, "EffectsCleared"),
        E::EffectChanged {
            entity,
            effect,
            event,
        } => (
            existed(
                lua,
                EFFECTS.existing(lua, &effect_key(*entity, *effect))?,
                seq,
            )?,
            event.name(),
        ),
        E::SkillAdded { index, skill } => {
            let list = match lua.named_registry_value::<Value>(TRAINED_KEY)? {
                Value::Table(t) => existed(lua, Some(t), seq)?,
                _ => None,
            };
            if list.is_some() {
                args.raw_set("Index", f64::from(*index))?;
                let key = skill.as_ref().map(|d| &d.key);
                args.raw_set("Skill", skill_object(lua, ctx, key, skill.as_ref())?)?;
            }
            (list, "SkillAdded")
        }
        E::SkillRemoved { index } => {
            args.raw_set("Index", f64::from(*index))?;
            let list = match lua.named_registry_value::<Value>(TRAINED_KEY)? {
                Value::Table(t) => existed(lua, Some(t), seq)?,
                _ => None,
            };
            (list, "SkillRemoved")
        }
        E::SkillsReset { count } => {
            let list = match lua.named_registry_value::<Value>(TRAINED_KEY)? {
                Value::Table(t) => existed(lua, Some(t), seq)?,
                _ => None,
            };
            if let Some(list) = list {
                for index in (1..=*count).rev() {
                    let a = lua.create_table()?;
                    a.raw_set("Index", f64::from(index))?;
                    ap.fire_event(&list, "SkillRemoved", a, None);
                }
                args.raw_set("Index", 1.0)?;
                args.raw_set("Skill", skill_object(lua, ctx, None, None)?)?;
                (Some(list), "SkillAdded")
            } else {
                (None, "SkillAdded")
            }
        }
        E::ResetTimeChanged { skill } => (
            existed(
                lua,
                SKILLS.existing(lua, &skill_registry_key(Some(skill)))?,
                seq,
            )?,
            "ResetTimeChanged",
        ),
    };
    if let Some(target) = target {
        ap.fire_event(&target, name, args, None);
    }
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;

    fn fx() -> CombatModel {
        let mut m = CombatModel::new();
        assert!(m.world_entered(1, "Me", Some(20)).is_empty());
        m.entity_spawned(2, ActorKind::Creature, "Wolf");
        m
    }

    fn effect(id: u32) -> EffectData {
        EffectData {
            id,
            name: format!("E{id}"),
            ..EffectData::default()
        }
    }

    #[test]
    fn vital_events_put_the_maximum_first() {
        let mut m = fx();
        let ev = m.vital_changed(1, CombatVital::Health, 500.0, 1000.0);
        let names: Vec<_> = ev
            .iter()
            .map(|e| match e {
                CombatViewEvent::Actor { event, .. } => event.name(),
                _ => "?",
            })
            .collect();
        assert_eq!(
            names,
            ["MaxMoraleChanged", "BaseMaxMoraleChanged", "MoraleChanged"]
        );
        assert_eq!(
            m.vital_changed(1, CombatVital::Health, 400.0, 1000.0).len(),
            1
        );
        assert!(
            m.vital_changed(1, CombatVital::Health, 400.0, 1000.0)
                .is_empty()
        );
        assert!(m.vital_changed(99, CombatVital::Power, 1.0, 2.0).is_empty());
        // NaN never repeats.
        assert_eq!(
            m.vital_changed(2, CombatVital::Power, f32::NAN, 1.0).len(),
            3
        );
        assert!(
            m.vital_changed(2, CombatVital::Power, f32::NAN, 1.0)
                .is_empty()
        );
    }

    #[test]
    fn effect_removal_moves_the_last_instance() {
        let mut m = fx();
        for id in [10, 11, 12] {
            m.effect_applied(1, effect(id));
        }
        let src = Mutex::new(m);
        assert_eq!(
            lock(&src).effect_removed(1, 10),
            [CombatViewEvent::EffectRemoved {
                entity: ViewEntity::LocalPlayer,
                index: 1
            }]
        );
        assert_eq!(src.effect_at(1, 0).unwrap().id, 12);
        assert_eq!(src.effect_at(1, 1).unwrap().id, 11);
        assert_eq!(src.effect_position(1, 10), None);
        // An in-place update gives the change events in the P91 order.
        let mut e = effect(11);
        e.name = "New".into();
        e.category = 8;
        e.start_time = 5.0;
        let ev = lock(&src).effect_applied(1, e);
        let names: Vec<_> = ev
            .iter()
            .map(|e| match e {
                CombatViewEvent::EffectChanged { event, .. } => event.name(),
                _ => "?",
            })
            .collect();
        assert_eq!(
            names,
            ["NameChanged", "StartTimeChanged", "CategoryChanged"]
        );
    }

    #[test]
    fn effects_are_bounded_per_entity() {
        let mut m = fx();
        for id in 0..(MAX_EFFECTS_PER_ACTOR as u32 + 10) {
            m.effect_applied(2, effect(id));
        }
        assert_eq!(Mutex::new(m).effect_count(2), MAX_EFFECTS_PER_ACTOR as u32);
    }

    #[test]
    fn skills_replace_and_restore_the_placeholder() {
        let mut m = fx();
        let s = SkillData {
            key: SkillKey::Record(0x7000_0001),
            name: "Strike".into(),
            description: String::new(),
            icon_image_id: 0,
            cooldown: 0.0,
        };
        let ev = m.skill_learned(s.clone());
        assert_eq!(ev[0], CombatViewEvent::SkillRemoved { index: 1 });
        assert!(matches!(
            ev[1],
            CombatViewEvent::SkillAdded {
                index: 1,
                skill: Some(_)
            }
        ));
        assert!(m.skill_learned(s.clone()).is_empty());
        let ev = m.skill_forgotten(&s.key);
        assert_eq!(
            ev,
            [
                CombatViewEvent::SkillRemoved { index: 1 },
                CombatViewEvent::SkillAdded {
                    index: 1,
                    skill: None
                }
            ]
        );
    }

    #[test]
    fn leaving_the_world_resets_the_session_state() {
        let mut m = fx();
        m.effect_applied(2, effect(1));
        m.target_changed(1, Some(2));
        m.combat_state(1, true);
        m.vital_changed(1, CombatVital::Health, 5.0, 10.0);
        let ev = m.left_world();
        assert_eq!(
            ev,
            [
                CombatViewEvent::EffectsCleared {
                    entity: ViewEntity::Entity(2)
                },
                CombatViewEvent::Actor {
                    entity: ViewEntity::LocalPlayer,
                    event: ActorEvent::TargetChanged
                },
                CombatViewEvent::InCombatChanged,
            ]
        );
        let src = Mutex::new(m);
        assert_eq!(src.actor(1).unwrap().morale, 5.0);
        assert!(src.actor(2).is_none());
    }

    #[test]
    fn targets_are_kept_as_reported() {
        let mut m = fx();
        // A target not yet spawned is kept (GetTarget resolves it at the call).
        assert_eq!(m.target_changed(2, Some(77)).len(), 1);
        assert!(m.target_changed(2, Some(77)).is_empty());
        assert_eq!(m.target_changed(2, Some(1)).len(), 1);
        assert_eq!(m.target_changed(1, Some(2)).len(), 1);
        assert_eq!(m.entity_despawned(2).len(), 1);
        assert!(Mutex::new(m).actor(1).unwrap().target.is_none());
    }

    #[test]
    fn despawn_before_world_entry_forgets_the_entity() {
        let mut m = CombatModel::new();
        m.entity_spawned(9, ActorKind::Creature, "Bear");
        assert!(m.entity_despawned(9).is_empty());
        m.world_entered(1, "Me", None);
        assert!(m.vital_changed(9, CombatVital::Health, 1.0, 2.0).is_empty());
        assert!(m.effect_applied(9, effect(1)).is_empty());
        let src = Mutex::new(m);
        assert!(src.actor(9).is_none());
        assert_eq!(src.last_actor(9).unwrap().name, "Bear");
    }

    #[test]
    fn forgotten_subjects_keep_their_final_values() {
        let mut m = fx();
        m.vital_changed(2, CombatVital::Health, 300.0, 500.0);
        m.level_changed(2, 9);
        m.effect_applied(1, effect(5));
        let mut e = effect(5);
        e.duration = 30.0;
        m.effect_applied(1, e);
        m.effect_removed(1, 5);
        m.effect_applied(2, effect(6));
        m.entity_despawned(2);
        let src = Mutex::new(m);
        assert!(src.actor(2).is_none());
        let last = src.last_actor(2).unwrap();
        assert_eq!(
            (last.morale, last.max_morale, last.level),
            (300.0, 500.0, 9)
        );
        assert_eq!(src.last_effect(1, 5).unwrap().duration, 30.0);
        assert_eq!(src.last_effect(2, 6).unwrap().name, "E6");
        // A respawn or a re-application makes the subject current again.
        let mut m = src.into_inner().unwrap();
        m.entity_spawned(2, ActorKind::Creature, "Wolf");
        m.effect_applied(1, effect(5));
        let src = Mutex::new(m);
        assert!(src.last_actor(2).is_none());
        assert_eq!(src.actor(2).unwrap().morale, 0.0);
        assert!(src.last_effect(1, 5).is_none());
    }

    #[test]
    fn retired_values_are_bounded() {
        let mut m = CombatModel::new();
        m.world_entered(1, "Me", None);
        for id in 10..(10 + MAX_RETIRED_ACTORS as u64 + 50) {
            m.entity_spawned(id, ActorKind::Creature, "C");
            m.effect_applied(id, effect(1));
            m.entity_despawned(id);
        }
        assert_eq!(m.retired_actors.len(), MAX_RETIRED_ACTORS);
        assert_eq!(m.retired_effects.len(), MAX_RETIRED_EFFECTS);
        let src = Mutex::new(m);
        // The oldest went first.
        assert!(src.last_actor(10).is_none());
        assert!(
            src.last_actor(10 + MAX_RETIRED_ACTORS as u64 + 49)
                .is_some()
        );
    }

    #[test]
    fn effects_are_bounded_in_total() {
        let mut m = CombatModel::new();
        m.world_entered(1, "Me", None);
        let per = MAX_EFFECTS_PER_ACTOR as u64;
        let actors = MAX_EFFECTS_TOTAL as u64 / per + 2;
        for a in 0..actors {
            m.entity_spawned(100 + a, ActorKind::Creature, "C");
            for id in 0..per as u32 {
                m.effect_applied(100 + a, effect(id));
            }
        }
        assert_eq!(m.effects_total, MAX_EFFECTS_TOTAL);
        let src = Mutex::new(m);
        let sum: u32 = (0..actors).map(|a| src.effect_count(100 + a)).sum();
        assert_eq!(sum as usize, MAX_EFFECTS_TOTAL);
        // Removals and despawns give room back.
        let mut m = src.into_inner().unwrap();
        m.entity_despawned(100);
        m.effect_removed(101, 0);
        assert_eq!(m.effects_total, MAX_EFFECTS_TOTAL - per as usize - 1);
        // One EffectsCleared per list left non-empty (the last two entities got none).
        assert_eq!(m.left_world().len(), MAX_EFFECTS_TOTAL / per as usize - 1);
        assert_eq!(m.effects_total, 0);
    }

    #[test]
    fn leaving_the_world_resets_skills_in_one_item_and_ends_cooldowns() {
        let mut m = fx();
        for did in 0..1100 {
            m.skill_learned(SkillData {
                key: SkillKey::Record(did),
                name: String::new(),
                description: String::new(),
                icon_image_id: 0,
                cooldown: 0.0,
            });
        }
        m.cooldown_started(SkillKey::Record(7), 10.0);
        m.cooldown_started(SkillKey::Record(3), 10.0);
        let ev = m.left_world();
        assert_eq!(
            ev,
            [
                CombatViewEvent::SkillsReset { count: 1100 },
                CombatViewEvent::ResetTimeChanged {
                    skill: SkillKey::Record(3)
                },
                CombatViewEvent::ResetTimeChanged {
                    skill: SkillKey::Record(7)
                },
            ]
        );
        let src = Mutex::new(m);
        assert_eq!(src.reset_time(&SkillKey::Record(7)), None);
        assert_eq!(src.skill_count(), 0);
    }

    #[test]
    fn the_watch_filters_events_without_view_objects() {
        let w = Arc::new(Watch::default());
        let wolf = CombatViewEvent::Actor {
            entity: ViewEntity::Entity(2),
            event: ActorEvent::MoraleChanged,
        };
        let changed = CombatViewEvent::EffectChanged {
            entity: 1,
            effect: 4,
            event: EffectEvent::NameChanged,
        };
        let reset = CombatViewEvent::ResetTimeChanged {
            skill: SkillKey::Record(9),
        };
        assert!(!w.may_reach(&wolf, || None));
        assert!(w.may_reach(&wolf, || Some(2)), "the player's entity");
        assert!(!w.may_reach(&changed, || None));
        assert!(!w.may_reach(&reset, || None));
        for always in [
            CombatViewEvent::InCombatChanged,
            CombatViewEvent::SkillsReset { count: 3 },
            CombatViewEvent::EffectsCleared {
                entity: ViewEntity::LocalPlayer,
            },
        ] {
            assert!(w.may_reach(&always, || None));
        }
        let g1 = Watch::register(&w, WatchKey::Entity(2));
        let g2 = Watch::register(&w, WatchKey::Entity(2));
        let g3 = Watch::register(&w, WatchKey::Effect(1, 4));
        let g4 = Watch::register(&w, WatchKey::Skill(SkillKey::Record(9)));
        assert!(w.may_reach(&wolf, || None));
        assert!(w.may_reach(&changed, || None));
        assert!(w.may_reach(&reset, || None));
        drop(g1);
        assert!(w.may_reach(&wolf, || None), "one object left");
        drop((g2, g3, g4));
        assert!(!w.may_reach(&wolf, || None));
        assert!(!w.may_reach(&changed, || None));
        assert!(!w.may_reach(&reset, || None));
        assert!(lock(&w.counts).is_empty());
    }

    #[test]
    fn index_arguments() {
        assert_eq!(position_arg(&Value::Number(1.0), 3), Some(0));
        assert_eq!(position_arg(&Value::Integer(3), 3), Some(2));
        for bad in [0.0, 4.0, 1.5, f64::NAN, f64::INFINITY, -1.0] {
            assert_eq!(position_arg(&Value::Number(bad), 3), None);
        }
        assert_eq!(position_arg(&Value::Nil, 3), None);
        assert_eq!(position_arg(&Value::Number(1.0), 0), None);
    }

    #[test]
    fn skill_registry_keys_round_trip() {
        for k in [SkillKey::Record(7), SkillKey::Literal("a:b".into())] {
            assert_eq!(SkillKey::from_registry_key(&k.registry_key()), Some(k));
        }
        assert_eq!(SkillKey::from_registry_key("p"), None);
    }
}
