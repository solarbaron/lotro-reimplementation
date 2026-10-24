// SPDX-License-Identifier: GPL-3.0-or-later
//! `Turbine.Gameplay` party views: `Party`, `PartyMember` and `Player:GetParty()`, over a
//! host-side fellowship model the client updates with
//! [`crate::PluginManager::set_fellowship`].
//!
//! Spec: `lua-plugin-api.md` §15 (queued delivery of game-raised events, frame-loop step
//! 7), §15.4 (the model, the view table, the event derivation, object identity), §9.3
//! (argument fields: `MemberAdded` and `MemberRemoved` carry `Player`, `LeaderChanged`
//! and `PartyChanged` carry nothing), §18 checks 90–99; `social-panels.md` §9 TR2 (who
//! the player is in a member list, a list without the player read with the player
//! appended, the leader).
//!
//! The client gives the plain member list of each `FellowshipChanged`
//! ([`FellowshipMember`]) and the player's identity ([`PartyPlayer`]); this module never
//! links a network crate. It applies TR2, keeps the current list, derives the §15.4 events
//! by comparing consecutive lists by name and queues them for step 7. The actor values of
//! members come from an optional [`PartyActorSource`]: by entity, and for a member without
//! an entity by name (morale and power from `FellowVitalChanged`, §15.4 revision 1).
//! `PartyMember` has no method of its own (no `GetRaid`, P95).
//!
//! Further rules 7–8 (§15.4, 2026-10-14): each `PartyMember` **follows** an entity (the
//! entity of its name's entry while the name is in its episode's list, afterwards the last
//! one it followed). The host keeps one `MemberLink` per (episode, name) that every
//! apartment's object of that member shares, updates it in `PartyModel::apply` and fans
//! the §15.2 effect-list events of the followed entity out to the members' own
//! `EffectList` objects ([`PartyViewEvent::MemberEffects`]); a change of the followed entity
//! queues `EffectsCleared` and `EffectAdded { 1 … n }` ([`MemberEffectsEvent::Refollowed`]).
//!
//! Object identity per apartment: one `Party` object per party episode (strongly held
//! while the episode is current, weakly afterwards), one `PartyMember` object per member
//! name per episode (strongly held while the name is in the current list or until the
//! member table reaches its prune threshold, weakly afterwards, as the inventory views do
//! with `Item` objects).

use std::collections::BTreeMap;
use std::sync::atomic::{AtomicUsize, Ordering};
use std::sync::{Arc, Mutex, Weak};

use mlua::{Lua, MultiValue, Table, Value};
use tessera_script::{api_fn, host_error};

use crate::apartment::Apartment;
use crate::gameplay::{GameplayEvent, LocalPlayerEvent};
use crate::object::{NativeKind, Payload, bind_native, native_of};
use crate::shared::{ApartmentCtx, lock};

/// Most entries of one member list that the model reads (the feed's `fellowship` step
/// carries 1 … 64, `scenario-feed.md` §6.4); further entries are ignored. The player,
/// when not among them, is appended after these (`social-panels.md` §9 TR2).
pub const MAX_PARTY_MEMBERS: usize = 64;

/// Registry name of the apartment's `Party` objects by episode (strong; only the current
/// episode's object stays here once the apartment has seen a later episode).
const PARTIES_KEY: &str = "tessera.plugins.party.parties";
/// Registry name of the `Party` objects of earlier episodes (weak values).
const PARTIES_OLD_KEY: &str = "tessera.plugins.party.parties.old";
/// Registry name of the apartment's `PartyMember` objects by "episode/name" (strong).
const MEMBERS_KEY: &str = "tessera.plugins.party.members";
/// Registry name of `PartyMember` objects no longer in the current list at a prune (weak
/// values).
const MEMBERS_DETACHED_KEY: &str = "tessera.plugins.party.members.detached";
/// Registry names of the number of entries in [`MEMBERS_KEY`] and its prune threshold.
const MEMBERS_COUNT_KEY: &str = "tessera.plugins.party.members.count";
const MEMBERS_PRUNE_KEY: &str = "tessera.plugins.party.members.prune";
/// Registry names of the `Party` and `PartyMember` class tables.
const PARTY_CLASS_KEY: &str = "tessera.plugins.class.party";
const MEMBER_CLASS_KEY: &str = "tessera.plugins.class.partymember";

/// Number of `PartyMember` objects an apartment keeps strongly before it detaches those
/// whose name left the current list (the threshold then doubles past the survivors).
const MEMBERS_PRUNE_AT: i64 = 128;

/// Number of follow records ([`MemberLink`]) the host keeps before it drops those no
/// object holds any more (the threshold then doubles past the survivors).
const LINKS_PRUNE_AT: usize = 128;

/// Most `EffectAdded` events one change of a member's followed entity delivers (the
/// effect bound per entity of [`crate::combat::CombatModel`]; a source reporting more is
/// cut there).
const MAX_REFOLLOW_EFFECTS: u32 = crate::combat::MAX_EFFECTS_PER_ACTOR as u32;

/// One entry of a `FellowshipChanged` member list (`scenario-feed.md` §9.2), as the client
/// passes it.
#[derive(Debug, Clone, PartialEq, Eq, Default)]
pub struct FellowshipMember {
    /// The member's name.
    pub name: String,
    /// The member's entity, when the session knows one.
    pub entity: Option<u64>,
    /// The member leads (the first such member is the leader, TR2).
    pub leader: bool,
}

/// Who the local player is, for finding the player in a member list (TR2: by entity,
/// else the first member with the character's name).
#[derive(Debug, Clone, PartialEq, Eq, Default)]
pub struct PartyPlayer {
    /// The character name.
    pub name: String,
    /// The player's entity, when known.
    pub entity: Option<u64>,
}

/// The `Actor` values of a member (`lua-plugin-api.md` §15.4: as the `Actor` getters for
/// its entity, from the events the feed sent for it; morale and power the later of its
/// `FellowVitalChanged` and its entity's `VitalChanged`). A member without an entity
/// answers what the source gives by name (morale and power), else the default (all 0).
#[derive(Debug, Clone, Copy, PartialEq, Default)]
pub struct MemberActorInfo {
    /// `GetLevel()`.
    pub level: u32,
    /// `GetMorale()`.
    pub morale: f64,
    /// `GetMaxMorale()`.
    pub max_morale: f64,
    /// `GetBaseMaxMorale()`.
    pub base_max_morale: f64,
    /// `GetTemporaryMorale()`.
    pub temporary_morale: f64,
    /// `GetMaxTemporaryMorale()`.
    pub max_temporary_morale: f64,
    /// `GetPower()`.
    pub power: f64,
    /// `GetMaxPower()`.
    pub max_power: f64,
    /// `GetBaseMaxPower()`.
    pub base_max_power: f64,
    /// `GetTemporaryPower()`.
    pub temporary_power: f64,
    /// `GetMaxTemporaryPower()`.
    pub max_temporary_power: f64,
}

/// What the client implements to give the `Actor` values of party members (read on
/// every getter call, on the plugin thread).
pub trait PartyActorSource: Send + Sync + 'static {
    /// The values of entity `entity`, or `None` when the client has none.
    fn member_actor(&self, entity: u64) -> Option<MemberActorInfo>;

    /// The values of the member named `name` that has no entity (morale and power from
    /// `FellowVitalChanged`, matched by name ignoring case; §15.4, check 98), or `None`
    /// when the client has none (the member then answers 0). Default: none.
    fn member_actor_named(&self, name: &str) -> Option<MemberActorInfo> {
        let _ = name;
        None
    }
}

/// What one [`crate::PluginManager::set_fellowship`] call did. The client must read
/// [`Self::player_appended`] (`social-panels.md` §9 TR2 asks for a warning once per run).
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
#[must_use = "`player_appended` drives the client's once-per-run TR2 warning"]
pub struct PartyUpdate {
    /// The list was non-empty and did not contain the player, who was appended (TR2: the
    /// client warns once per run).
    pub player_appended: bool,
    /// The list had more than [`MAX_PARTY_MEMBERS`] entries; the rest were ignored.
    pub truncated: bool,
    /// Number of plugin events queued for step 7 (`PartyChanged` included; a change of
    /// a member's followed entity, further rule 7, counts as one).
    pub events: usize,
    /// Number of derived events not queued because the plugin host's gameplay queue was
    /// full (each is also counted in the host's dropped-event total and logged).
    pub dropped: usize,
}

/// A `Party` event of `lua-plugin-api.md` §15.4, derived by the host from consecutive
/// member lists and delivered in step 7 to each apartment's `Party` object of that
/// episode while the episode is current (nothing when the apartment has none, or when the
/// episode ended before delivery). `LocalPlayer.PartyChanged` is a
/// [`LocalPlayerEvent::PartyChanged`].
///
/// Only [`crate::PluginManager::set_fellowship`] queues these; a host that passes one to
/// [`crate::PluginManager::raise_gameplay_event`] has it ignored.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum PartyViewEvent {
    /// `Party.MemberAdded { Player }`.
    MemberAdded {
        /// The party episode.
        episode: u64,
        /// The member's name (its `PartyMember` object is `Player`).
        name: String,
        /// The member's entity in the list (its actor values are read at delivery).
        entity: Option<u64>,
    },
    /// `Party.MemberRemoved { Player }`.
    MemberRemoved {
        /// The party episode.
        episode: u64,
        /// The member's name.
        name: String,
        /// The member's entity in the old list (its actor values, read at delivery,
        /// become the object's last values seen).
        entity: Option<u64>,
    },
    /// `Party.LeaderChanged` (empty argument table).
    LeaderChanged {
        /// The party episode.
        episode: u64,
    },
    /// `EffectList` events on the own effect lists of `PartyMember` objects (§15.4 further
    /// rules 7–8), delivered in each apartment to the list of each named member that
    /// existed when the event was raised, also after the member left or its episode ended.
    MemberEffects {
        /// The members whose lists receive the events: (episode, name), in order.
        members: Vec<(u64, String)>,
        /// The events.
        event: MemberEffectsEvent,
    },
}

/// The events of a [`PartyViewEvent::MemberEffects`].
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum MemberEffectsEvent {
    /// `EffectAdded { Index }`, as the followed entity's lists receive it (§15.2).
    Added {
        /// `Index`.
        index: u32,
    },
    /// `EffectRemoved { Index }`, as the followed entity's lists receive it.
    Removed {
        /// `Index`.
        index: u32,
    },
    /// `EffectsCleared`, as the followed entity's lists receive it.
    Cleared,
    /// The followed entity changed at a `FellowshipChanged` (further rule 7): one
    /// `EffectsCleared` when `cleared` (the list was not empty), then `EffectAdded { Index }`
    /// for `Index` 1 … `added` (the new entity's effects). One queue item, so the queue
    /// bound never cuts the sequence.
    Refollowed {
        /// The list was not empty before the change.
        cleared: bool,
        /// The number of effects of the newly followed entity.
        added: u32,
    },
}

/// One member of the current list.
#[derive(Debug, Clone, PartialEq, Eq)]
struct Entry {
    name: String,
    entity: Option<u64>,
    is_player: bool,
}

/// The host-side fellowship model (TR2) with its episode counter.
#[derive(Debug, Default)]
pub(crate) struct PartyModel {
    /// The current episode; 0 outside a party.
    episode: u64,
    /// The last episode number given out.
    last_episode: u64,
    members: Vec<Entry>,
    /// Index of the leader in `members`.
    leader: Option<usize>,
    /// The follow records of members of this and earlier episodes that some object holds.
    follows: Follows,
}

/// The host's follow records: one [`MemberLink`] per (episode, name) while an object holds
/// it, and the changes of followed entities the last [`PartyModel::apply`] made.
#[derive(Debug, Default)]
struct Follows {
    links: BTreeMap<(u64, String), Weak<MemberLink>>,
    prune_at: usize,
    changes: Vec<Refollow>,
}

impl Follows {
    fn prune(&mut self) {
        self.links.retain(|_, w| w.strong_count() > 0);
        self.prune_at = LINKS_PRUNE_AT.max(self.links.len().saturating_mul(2));
    }
}

/// A change of a member's followed entity whose member had an `EffectList` object at the
/// change (further rule 7).
#[derive(Debug)]
pub(crate) struct Refollow {
    pub link: Arc<MemberLink>,
    pub old: Option<u64>,
    pub new: Option<u64>,
}

/// A member as a getter sees it.
#[derive(Debug, Clone)]
pub(crate) struct MemberRef {
    pub name: String,
    pub is_player: bool,
}

impl PartyModel {
    /// The current episode, or `None` outside a party.
    pub(crate) fn episode(&self) -> Option<u64> {
        (self.episode != 0).then_some(self.episode)
    }

    fn leader_name(&self) -> Option<&str> {
        self.leader
            .and_then(|i| self.members.get(i))
            .map(|e| e.name.as_str())
    }

    /// Non-player names in list order, each once.
    fn other_names(&self) -> Vec<&str> {
        let mut out: Vec<&str> = Vec::new();
        for e in self.members.iter().filter(|e| !e.is_player) {
            if !out.contains(&e.name.as_str()) {
                out.push(&e.name);
            }
        }
        out
    }

    /// Is `name` a non-player member of the current list?
    fn has_other(&self, name: &str) -> bool {
        self.members.iter().any(|e| !e.is_player && e.name == name)
    }

    /// The entity of the first non-player member named `name` in the current list.
    fn entity_of(&self, name: &str) -> Option<Option<u64>> {
        self.members
            .iter()
            .find(|e| !e.is_player && e.name == name)
            .map(|e| e.entity)
    }

    fn member(&self, index: usize) -> Option<MemberRef> {
        self.members.get(index).map(|e| MemberRef {
            name: e.name.clone(),
            is_player: e.is_player,
        })
    }

    /// Apply a new member list (TR2) and return the plugin events of §15.4 in order (the
    /// caller fills [`PartyUpdate::events`] and [`PartyUpdate::dropped`] when it queues
    /// them).
    pub(crate) fn apply(
        &mut self,
        list: &[FellowshipMember],
        player: &PartyPlayer,
    ) -> (Vec<GameplayEvent>, PartyUpdate) {
        let mut update = PartyUpdate {
            truncated: list.len() > MAX_PARTY_MEMBERS,
            ..PartyUpdate::default()
        };
        let list = &list[..list.len().min(MAX_PARTY_MEMBERS)];
        let mut events = Vec::new();
        self.follows.changes.clear();
        if list.is_empty() {
            if self.episode != 0 {
                // Rule 2: the old `Party` gets no events. Further rule 8: the episode's
                // members keep following their last entities (their records stay).
                let mut follows = std::mem::take(&mut self.follows);
                follows.prune();
                *self = Self {
                    last_episode: self.last_episode,
                    follows,
                    ..Self::default()
                };
                events.push(GameplayEvent::LocalPlayer(LocalPlayerEvent::PartyChanged));
            }
            return (events, update);
        }

        let player_at = player
            .entity
            .and_then(|id| list.iter().position(|m| m.entity == Some(id)))
            .or_else(|| list.iter().position(|m| m.name == player.name));
        let mut members: Vec<Entry> = list
            .iter()
            .enumerate()
            .map(|(i, m)| Entry {
                name: m.name.clone(),
                entity: m.entity,
                is_player: Some(i) == player_at,
            })
            .collect();
        if player_at.is_none() {
            update.player_appended = true;
            members.push(Entry {
                name: player.name.clone(),
                entity: player.entity,
                is_player: true,
            });
        }
        let leader = list.iter().position(|m| m.leader);
        let mut new = Self {
            episode: self.episode,
            last_episode: self.last_episode,
            members,
            leader,
            follows: std::mem::take(&mut self.follows),
        };
        new.follows.prune();

        if self.episode == 0 {
            // Rule 1: a new episode, `PartyChanged`, no member events (no object of the
            // new episode exists yet, so no member follows anything to change).
            let episode = self.last_episode.saturating_add(1);
            *self = Self {
                episode,
                last_episode: episode,
                ..new
            };
            events.push(GameplayEvent::LocalPlayer(LocalPlayerEvent::PartyChanged));
            return (events, update);
        }

        // Rule 3 (rule 4: identical lists give nothing).
        let episode = self.episode;
        let (old_names, new_names) = (self.other_names(), new.other_names());
        for name in old_names.iter().filter(|n| !new_names.contains(n)) {
            let entity = self.entity_of(name).flatten();
            events.push(GameplayEvent::Party(PartyViewEvent::MemberRemoved {
                episode,
                name: (*name).to_owned(),
                entity,
            }));
        }
        for name in new_names.iter().filter(|n| !old_names.contains(n)) {
            let entity = new.entity_of(name).flatten();
            events.push(GameplayEvent::Party(PartyViewEvent::MemberAdded {
                episode,
                name: (*name).to_owned(),
                entity,
            }));
        }
        if self.leader_name() != new.leader_name() {
            events.push(GameplayEvent::Party(PartyViewEvent::LeaderChanged {
                episode,
            }));
        }
        new.refollow(episode);
        *self = new;
        (events, update)
    }

    /// Further rule 7: every member of `episode` whose name is in the (new) current list
    /// follows its entry's entity from now on; one whose name is not keeps its entity
    /// (rule 8). Changes of members with an `EffectList` object are kept for
    /// [`Self::take_refollows`].
    fn refollow(&mut self, episode: u64) {
        let mut changes = Vec::new();
        for ((e, name), weak) in self.follows.links.range((episode, String::new())..) {
            if *e != episode {
                break;
            }
            let Some(link) = weak.upgrade() else {
                continue;
            };
            let Some(now) = self.entity_of(name) else {
                continue;
            };
            let old = std::mem::replace(&mut *lock(&link.followed), now);
            if old != now && link.lists.load(Ordering::SeqCst) > 0 {
                changes.push(Refollow {
                    link,
                    old,
                    new: now,
                });
            }
        }
        self.follows.changes = changes;
    }

    /// The followed-entity changes of the last [`Self::apply`] (members with an
    /// `EffectList` object only), in (episode, name) order.
    pub(crate) fn take_refollows(&mut self) -> Vec<Refollow> {
        std::mem::take(&mut self.follows.changes)
    }

    /// The follow record of (`episode`, `name`), created on first use: following the
    /// entity of the name's entry when the name is in the current list of `episode`, else
    /// `fallback` (the entity of the name's last entry, carried by its member event).
    pub(crate) fn link(
        &mut self,
        episode: u64,
        name: &str,
        fallback: Option<u64>,
    ) -> Arc<MemberLink> {
        let key = (episode, name.to_owned());
        if let Some(link) = self.follows.links.get(&key).and_then(Weak::upgrade) {
            return link;
        }
        let followed = if self.episode() == Some(episode) {
            self.entity_of(name).unwrap_or(fallback)
        } else {
            fallback
        };
        let link = Arc::new(MemberLink {
            episode,
            name: name.to_owned(),
            followed: Mutex::new(followed),
            lists: AtomicUsize::new(0),
        });
        if self.follows.links.len() >= self.follows.prune_at.max(LINKS_PRUNE_AT) {
            self.follows.prune();
        }
        self.follows.links.insert(key, Arc::downgrade(&link));
        link
    }

    /// The members (episode, name) with an `EffectList` object that follow `entity` now,
    /// in (episode, name) order (further rule 7: the effect-list events of that entity are
    /// queued on their lists too).
    pub(crate) fn followers(&self, entity: u64) -> Vec<(u64, String)> {
        self.follows
            .links
            .iter()
            .filter_map(|(key, weak)| {
                let link = weak.upgrade()?;
                (link.lists.load(Ordering::SeqCst) > 0 && link.followed() == Some(entity))
                    .then(|| key.clone())
            })
            .collect()
    }
}

/// Payload of a `Party` object.
struct PartyPayload {
    episode: u64,
}

/// Payload of a `PartyMember` object: its episode, its name and the last actor values the
/// apartment saw.
struct MemberPayload {
    episode: u64,
    name: String,
    last: Mutex<MemberActorInfo>,
    link: Arc<MemberLink>,
}

/// The entity a `PartyMember` follows (§15.4 further rules 7–8: its `GetTarget()` and
/// its own `EffectList` show that entity). One per (episode, name), shared by the objects
/// of that member in every apartment and by their `EffectList` objects; the host updates
/// it at each member list. Holds no Lua value.
#[derive(Debug)]
pub(crate) struct MemberLink {
    episode: u64,
    name: String,
    /// The followed entity: while the name is in its episode's list, its entry's entity
    /// (`None` for an entry without one); afterwards the last one it followed.
    followed: Mutex<Option<u64>>,
    /// Number of `EffectList` objects of this member alive in all apartments.
    lists: AtomicUsize,
}

impl MemberLink {
    /// The entity the member follows now (`None`: none).
    pub(crate) fn followed(&self) -> Option<u64> {
        *lock(&self.followed)
    }

    /// The member: (episode, name).
    pub(crate) fn member(&self) -> (u64, String) {
        (self.episode, self.name.clone())
    }

    /// The member's registry key in an apartment ("episode/name").
    pub(crate) fn key(&self) -> String {
        member_key(self.episode, &self.name)
    }
}

/// Counts one `EffectList` object of a member while it exists (the host fans effect-list
/// events out only to members that have one, the queue bound of §15.2).
#[derive(Debug)]
pub(crate) struct MemberListGuard(Arc<MemberLink>);

impl MemberListGuard {
    pub(crate) fn new(link: Arc<MemberLink>) -> Self {
        link.lists.fetch_add(1, Ordering::SeqCst);
        Self(link)
    }

    pub(crate) fn link(&self) -> &MemberLink {
        &self.0
    }
}

impl Drop for MemberListGuard {
    fn drop(&mut self) {
        self.0.lists.fetch_sub(1, Ordering::SeqCst);
    }
}

/// The member-list events of a combat event (further rule 7): an `EffectAdded`,
/// `EffectRemoved` or `EffectsCleared` of another entity's lists, with that entity.
pub(crate) fn member_effects_of(
    ev: &crate::combat::CombatViewEvent,
) -> Option<(u64, MemberEffectsEvent)> {
    use crate::combat::{CombatViewEvent as E, ViewEntity};
    match *ev {
        E::EffectAdded {
            entity: ViewEntity::Entity(id),
            index,
        } => Some((id, MemberEffectsEvent::Added { index })),
        E::EffectRemoved {
            entity: ViewEntity::Entity(id),
            index,
        } => Some((id, MemberEffectsEvent::Removed { index })),
        E::EffectsCleared {
            entity: ViewEntity::Entity(id),
        } => Some((id, MemberEffectsEvent::Cleared)),
        _ => None,
    }
}

/// Recognises the apartment's `LocalPlayer` instance.
pub(crate) type IsLocalPlayer = Arc<dyn Fn(&Lua, &Table) -> mlua::Result<bool> + Send + Sync>;

/// Returns the apartment's `LocalPlayer` instance (as `LocalPlayer.GetInstance()` does).
pub(crate) type LocalPlayerInstance = Arc<dyn Fn(&Lua) -> mlua::Result<Value> + Send + Sync>;

/// Run `f` on the native payload of `this` if it is a `T`.
fn with_payload<T: 'static, R>(
    lua: &Lua,
    this: &Table,
    f: impl FnOnce(&T) -> R,
) -> mlua::Result<Option<R>> {
    Ok(match native_of(lua, this)? {
        Some(NativeKind::External(Payload(p))) => p.downcast_ref::<T>().map(f),
        _ => None,
    })
}

fn with_model<R>(ctx: &ApartmentCtx, f: impl FnOnce(&PartyModel) -> R) -> Option<R> {
    let shared = ctx.shared().ok()?;
    let model = lock(&shared.party);
    Some(f(&model))
}

fn actor_source(ctx: &ApartmentCtx) -> Option<Arc<dyn PartyActorSource>> {
    let shared = ctx.shared().ok()?;
    lock(&shared.party_actors).clone()
}

/// A member's actor values now: for a member without an entity the source's values by
/// name, else the default (all 0) (§15.4 revision 1); the source's values for one with an
/// entity, `None` when the source has no values for that entity (the object then keeps
/// the last values seen, §15.4).
pub(crate) fn actor_values(
    source: Option<&Arc<dyn PartyActorSource>>,
    name: &str,
    entity: Option<u64>,
) -> Option<MemberActorInfo> {
    match entity {
        None => Some(
            source
                .and_then(|s| s.member_actor_named(name))
                .unwrap_or_default(),
        ),
        Some(id) => source.and_then(|s| s.member_actor(id)),
    }
}

fn new_object(lua: &Lua, class: &Table, payload: Payload) -> mlua::Result<Table> {
    let obj = lua.create_table()?;
    let mt = lua.create_table()?;
    mt.raw_set("__index", class.clone())?;
    obj.set_metatable(Some(mt))?;
    bind_native(lua, &obj, NativeKind::External(payload), class.clone())?;
    Ok(obj)
}

fn weak_values(lua: &Lua) -> mlua::Result<Table> {
    let t = lua.create_table()?;
    let mt = lua.create_table()?;
    mt.raw_set("__mode", "v")?;
    t.set_metatable(Some(mt))?;
    Ok(t)
}

/// The apartment's existing `Party` object of `episode`, if any.
fn existing_party(lua: &Lua, episode: u64) -> mlua::Result<Option<Table>> {
    for key in [PARTIES_KEY, PARTIES_OLD_KEY] {
        let t: Table = lua.named_registry_value(key)?;
        if let Value::Table(obj) = t.raw_get::<Value>(episode)? {
            return Ok(Some(obj));
        }
    }
    Ok(None)
}

/// The apartment's `Party` object of the current episode `episode`, created on first use;
/// objects of earlier episodes move to the weak table.
fn party_object(lua: &Lua, episode: u64) -> mlua::Result<Table> {
    let parties: Table = lua.named_registry_value(PARTIES_KEY)?;
    if let Value::Table(t) = parties.raw_get::<Value>(episode)? {
        return Ok(t);
    }
    let old: Table = lua.named_registry_value(PARTIES_OLD_KEY)?;
    let mut earlier = Vec::new();
    for pair in parties.pairs::<u64, Table>() {
        earlier.push(pair?);
    }
    for (k, obj) in earlier {
        parties.raw_set(k, Value::Nil)?;
        old.raw_set(k, obj)?;
    }
    let obj = if let Value::Table(t) = old.raw_get::<Value>(episode)? {
        old.raw_set(episode, Value::Nil)?;
        t
    } else {
        let class: Table = lua.named_registry_value(PARTY_CLASS_KEY)?;
        new_object(lua, &class, Payload(Arc::new(PartyPayload { episode })))?
    };
    parties.raw_set(episode, obj.clone())?;
    Ok(obj)
}

fn member_key(episode: u64, name: &str) -> String {
    format!("{episode}/{name}")
}

/// The host's follow record of (`episode`, `name`) (see [`PartyModel::link`]); a record of
/// its own, following `fallback`, once the manager is gone.
fn link_of(ctx: &ApartmentCtx, episode: u64, name: &str, fallback: Option<u64>) -> Arc<MemberLink> {
    match ctx.shared() {
        Ok(shared) => lock(&shared.party).link(episode, name, fallback),
        Err(_) => Arc::new(MemberLink {
            episode,
            name: name.to_owned(),
            followed: Mutex::new(fallback),
            lists: AtomicUsize::new(0),
        }),
    }
}

/// The apartment's `PartyMember` object for `name` in `episode` (one per name per
/// episode), created on first use with last values `initial`; a new object follows the
/// entity of the name's entry, or `fallback` when the name is no longer listed.
fn member_object(
    lua: &Lua,
    ctx: &ApartmentCtx,
    episode: u64,
    name: &str,
    initial: MemberActorInfo,
    fallback: Option<u64>,
) -> mlua::Result<Table> {
    let members: Table = lua.named_registry_value(MEMBERS_KEY)?;
    let key = member_key(episode, name);
    if let Value::Table(t) = members.raw_get::<Value>(key.as_str())? {
        return Ok(t);
    }
    let detached: Table = lua.named_registry_value(MEMBERS_DETACHED_KEY)?;
    let obj = if let Value::Table(t) = detached.raw_get::<Value>(key.as_str())? {
        detached.raw_set(key.as_str(), Value::Nil)?;
        t
    } else {
        let class: Table = lua.named_registry_value(MEMBER_CLASS_KEY)?;
        new_object(
            lua,
            &class,
            Payload(Arc::new(MemberPayload {
                episode,
                name: name.to_owned(),
                last: Mutex::new(initial),
                link: link_of(ctx, episode, name, fallback),
            })),
        )?
    };
    members.raw_set(key, obj.clone())?;
    let count = lua.named_registry_value::<i64>(MEMBERS_COUNT_KEY)? + 1;
    lua.set_named_registry_value(MEMBERS_COUNT_KEY, count)?;
    if count >= lua.named_registry_value::<i64>(MEMBERS_PRUNE_KEY)? {
        prune_members(lua, ctx)?;
    }
    Ok(obj)
}

/// Move the `PartyMember` objects whose name is not in the current list of the current
/// episode to the weak table; set the next threshold.
fn prune_members(lua: &Lua, ctx: &ApartmentCtx) -> mlua::Result<()> {
    let members: Table = lua.named_registry_value(MEMBERS_KEY)?;
    let detached: Table = lua.named_registry_value(MEMBERS_DETACHED_KEY)?;
    let mut entries = Vec::new();
    for pair in members.pairs::<String, Table>() {
        entries.push(pair?);
    }
    let current: Vec<bool> = with_model(ctx, |m| {
        entries
            .iter()
            .map(|(_, obj)| {
                with_payload(lua, obj, |p: &MemberPayload| {
                    m.episode() == Some(p.episode) && m.has_other(&p.name)
                })
                .ok()
                .flatten()
                .unwrap_or(false)
            })
            .collect()
    })
    .unwrap_or_else(|| vec![false; entries.len()]);
    let mut kept: i64 = 0;
    for ((k, obj), keep) in entries.into_iter().zip(current) {
        if keep {
            kept += 1;
        } else {
            members.raw_set(k.as_str(), Value::Nil)?;
            detached.raw_set(k, obj)?;
        }
    }
    lua.set_named_registry_value(MEMBERS_COUNT_KEY, kept)?;
    lua.set_named_registry_value(
        MEMBERS_PRUNE_KEY,
        MEMBERS_PRUNE_AT.max(kept.saturating_mul(2)),
    )
}

/// A whole number in `1 ..= hi` from a Lua argument, else `None` (nil for a non-integer,
/// an out-of-range value or a non-number).
fn whole_index(v: &Value, hi: usize) -> Option<usize> {
    let x = match *v {
        Value::Integer(i) => i as f64,
        Value::Number(n) => n,
        _ => return None,
    };
    if x.fract() != 0.0 || x < 1.0 || x > hi as f64 {
        return None;
    }
    // In range by the bounds check.
    Some(x as usize)
}

/// The episode of the `Party` object `this`; a Lua error when `this` has no bound
/// `Party` object (§8.2).
fn party_episode(lua: &Lua, method: &str, this: &Value) -> mlua::Result<u64> {
    let episode = match this {
        Value::Table(t) => with_payload(lua, t, |p: &PartyPayload| p.episode)?,
        _ => None,
    };
    episode.ok_or_else(|| host_error(format!("{method}: expected a Party object as self")))
}

/// The episode of the `Party` object `this` when it is the current one (`None` for an
/// ended episode); a Lua error when `this` is not a `Party` object.
fn current_episode_of(
    ctx: &ApartmentCtx,
    lua: &Lua,
    method: &str,
    this: &Value,
) -> mlua::Result<Option<u64>> {
    let episode = party_episode(lua, method, this)?;
    Ok(is_current(ctx, episode).then_some(episode))
}

/// Is `episode` the model's current episode?
fn is_current(ctx: &ApartmentCtx, episode: u64) -> bool {
    with_model(ctx, |m| m.episode() == Some(episode)).unwrap_or(false)
}

/// The value of a member reference: the `LocalPlayer` instance or the `PartyMember`.
fn member_value(
    lua: &Lua,
    ctx: &ApartmentCtx,
    episode: u64,
    member: &MemberRef,
    local: &LocalPlayerInstance,
) -> mlua::Result<Value> {
    if member.is_player {
        return local(lua);
    }
    let initial = current_member_values(ctx, episode, &member.name)
        .flatten()
        .unwrap_or_default();
    Ok(Value::Table(member_object(
        lua,
        ctx,
        episode,
        &member.name,
        initial,
        None,
    )?))
}

/// The actor values of `name` while it is a member of the current episode `episode`:
/// `None` when it is not, `Some(None)` when it is but the source has no values for its
/// entity (see [`actor_values`]).
fn current_member_values(
    ctx: &ApartmentCtx,
    episode: u64,
    name: &str,
) -> Option<Option<MemberActorInfo>> {
    let entity = with_model(ctx, |m| {
        (m.episode() == Some(episode))
            .then(|| m.entity_of(name))
            .flatten()
    })
    .flatten()?;
    Some(actor_values(actor_source(ctx).as_ref(), name, entity))
}

/// The name of the `PartyMember` object `this`, or `None` when `this` is not one (for
/// `Entity:GetName()`).
pub(crate) fn member_name(lua: &Lua, this: &Table) -> mlua::Result<Option<String>> {
    with_payload(lua, this, |m: &MemberPayload| m.name.clone())
}

/// The actor values of the `PartyMember` object `this` (for the `Actor` getters), or
/// `None` when `this` is not one: the live values while the member is in the current
/// list and the source has them, else the last values seen (§15.4).
pub(crate) fn member_actor(
    ctx: &ApartmentCtx,
    lua: &Lua,
    this: &Table,
) -> mlua::Result<Option<MemberActorInfo>> {
    with_payload(lua, this, |p: &MemberPayload| {
        let mut last = lock(&p.last);
        if let Some(Some(now)) = current_member_values(ctx, p.episode, &p.name) {
            *last = now;
        }
        *last
    })
}

/// The entity link of the `PartyMember` object `this`, or `None` when `this` is not one
/// (for `Actor:GetTarget()` and `Actor:GetEffects()`, §15.4).
pub(crate) fn member_link(lua: &Lua, this: &Table) -> mlua::Result<Option<Arc<MemberLink>>> {
    with_payload(lua, this, |m: &MemberPayload| Arc::clone(&m.link))
}

/// `Player:GetParty()` of the `PartyMember` object `this`: `None` when `this` is not
/// one, else the episode's `Party` while the name is in the current list, nil otherwise.
fn member_party(ctx: &ApartmentCtx, lua: &Lua, this: &Table) -> mlua::Result<Option<Value>> {
    let Some((episode, name)) =
        with_payload(lua, this, |m: &MemberPayload| (m.episode, m.name.clone()))?
    else {
        return Ok(None);
    };
    let current =
        with_model(ctx, |m| m.episode() == Some(episode) && m.has_other(&name)).unwrap_or(false);
    Ok(Some(if current {
        Value::Table(party_object(lua, episode)?)
    } else {
        Value::Nil
    }))
}

/// The class tables the party views fill.
pub(crate) struct Classes<'a> {
    pub player: &'a Table,
    pub party: &'a Table,
    pub party_member: &'a Table,
}

/// Reads a number of a member's actor values.
pub(crate) type ActorNumber = fn(&MemberActorInfo) -> f64;

/// Install `Player:GetParty()` and the `Party` and `PartyMember` methods into a new
/// apartment.
pub(crate) fn install(
    lua: &Lua,
    ctx: &Arc<ApartmentCtx>,
    classes: &Classes<'_>,
    is_local: IsLocalPlayer,
    local: LocalPlayerInstance,
) -> mlua::Result<()> {
    lua.set_named_registry_value(PARTIES_KEY, lua.create_table()?)?;
    lua.set_named_registry_value(PARTIES_OLD_KEY, weak_values(lua)?)?;
    lua.set_named_registry_value(MEMBERS_KEY, lua.create_table()?)?;
    lua.set_named_registry_value(MEMBERS_DETACHED_KEY, weak_values(lua)?)?;
    lua.set_named_registry_value(MEMBERS_COUNT_KEY, 0_i64)?;
    lua.set_named_registry_value(MEMBERS_PRUNE_KEY, MEMBERS_PRUNE_AT)?;
    lua.set_named_registry_value(PARTY_CLASS_KEY, classes.party.clone())?;
    lua.set_named_registry_value(MEMBER_CLASS_KEY, classes.party_member.clone())?;

    // Player:GetParty() on the LocalPlayer instance and on `PartyMember` objects (the
    // method is `Player`'s own, §16; `PartyMember` has no own members, §8.6 rule 2).
    let (c, isl) = (Arc::clone(ctx), Arc::clone(&is_local));
    classes.player.raw_set(
        "GetParty",
        api_fn(lua, "GetParty", move |lua, this: Table| {
            if isl(lua, &this)? {
                return match with_model(&c, PartyModel::episode).flatten() {
                    Some(e) => Ok(Value::Table(party_object(lua, e)?)),
                    None => Ok(Value::Nil),
                };
            }
            Ok(member_party(&c, lua, &this)?.unwrap_or(Value::Nil))
        })?,
    )?;

    // `PartyMember` has no method of its own (§15.4 revision 1, P95: no `GetRaid` in this
    // build); its getters are the base classes' (§8.6 rule 2).
    install_party(lua, ctx, classes.party, &is_local, &local)
}

fn install_party(
    lua: &Lua,
    ctx: &Arc<ApartmentCtx>,
    party: &Table,
    is_local: &IsLocalPlayer,
    local: &LocalPlayerInstance,
) -> mlua::Result<()> {
    let c = Arc::clone(ctx);
    party.raw_set(
        "GetMemberCount",
        api_fn(lua, "GetMemberCount", move |lua, this: Value| {
            if current_episode_of(&c, lua, "GetMemberCount", &this)?.is_none() {
                return Ok(0.0);
            }
            Ok(with_model(&c, |m| m.members.len() as f64).unwrap_or(0.0))
        })?,
    )?;
    let (c, lp) = (Arc::clone(ctx), Arc::clone(local));
    party.raw_set(
        "GetMember",
        api_fn(lua, "GetMember", move |lua, (this, i): (Value, Value)| {
            let Some(episode) = current_episode_of(&c, lua, "GetMember", &this)? else {
                return Ok(Value::Nil);
            };
            let member = with_model(&c, |m| {
                whole_index(&i, m.members.len()).and_then(|i| m.member(i - 1))
            })
            .flatten();
            match member {
                Some(member) => member_value(lua, &c, episode, &member, &lp),
                None => Ok(Value::Nil),
            }
        })?,
    )?;
    let (c, lp) = (Arc::clone(ctx), Arc::clone(local));
    party.raw_set(
        "GetLeader",
        api_fn(lua, "GetLeader", move |lua, this: Value| {
            let Some(episode) = current_episode_of(&c, lua, "GetLeader", &this)? else {
                return Ok(Value::Nil);
            };
            let leader = with_model(&c, |m| m.leader.and_then(|i| m.member(i))).flatten();
            match leader {
                Some(member) => member_value(lua, &c, episode, &member, &lp),
                None => Ok(Value::Nil),
            }
        })?,
    )?;
    let (c, isl) = (Arc::clone(ctx), Arc::clone(is_local));
    party.raw_set(
        "IsMember",
        api_fn(lua, "IsMember", move |lua, (this, p): (Value, Value)| {
            let Some(episode) = current_episode_of(&c, lua, "IsMember", &this)? else {
                return Ok(false);
            };
            let Value::Table(p) = p else {
                return Ok(false);
            };
            if isl(lua, &p)? {
                return Ok(true);
            }
            let Some((e, name)) =
                with_payload(lua, &p, |m: &MemberPayload| (m.episode, m.name.clone()))?
            else {
                return Ok(false);
            };
            Ok(e == episode && with_model(&c, |m| m.has_other(&name)).unwrap_or(false))
        })?,
    )?;
    // The feed carries no assist targets (§15.4); `self` must still be a `Party` (§8.2).
    party.raw_set(
        "GetAssistTargetCount",
        api_fn(lua, "GetAssistTargetCount", |lua, args: MultiValue| {
            party_episode(lua, "GetAssistTargetCount", &first(&args))?;
            Ok(0.0)
        })?,
    )?;
    party.raw_set(
        "GetAssistTarget",
        api_fn(lua, "GetAssistTarget", |lua, args: MultiValue| {
            party_episode(lua, "GetAssistTarget", &first(&args))?;
            Ok(Value::Nil)
        })?,
    )?;
    party.raw_set(
        "IsAssistTarget",
        api_fn(lua, "IsAssistTarget", |lua, args: MultiValue| {
            party_episode(lua, "IsAssistTarget", &first(&args))?;
            Ok(false)
        })?,
    )
}

/// The first value of `args` (nil when there is none).
fn first(args: &MultiValue) -> Value {
    args.iter().next().cloned().unwrap_or(Value::Nil)
}

/// Deliver one queued party event (sequence number `seq`) to the apartment `ap`: to its
/// `Party` object of the event's episode (nothing when it has none), or, for
/// [`PartyViewEvent::MemberEffects`], to the named members' `EffectList` objects that
/// existed when it was raised.
pub(crate) fn deliver(ap: &Apartment, ev: &PartyViewEvent, seq: u64) {
    if let Err(e) = deliver_inner(ap, ev, seq) {
        ap.report(None, "delivering a party event", &e);
    }
}

/// Fire the events of `event` on the member list `list`.
fn fire_member_effects(
    ap: &Apartment,
    list: &Table,
    event: MemberEffectsEvent,
) -> mlua::Result<()> {
    let lua = ap.lua();
    let indexed = |name: &str, index: u32| -> mlua::Result<()> {
        let args = lua.create_table()?;
        args.raw_set("Index", f64::from(index))?;
        ap.fire_event(list, name, args, None);
        Ok(())
    };
    match event {
        MemberEffectsEvent::Added { index } => indexed("EffectAdded", index)?,
        MemberEffectsEvent::Removed { index } => indexed("EffectRemoved", index)?,
        MemberEffectsEvent::Cleared => {
            ap.fire_event(list, "EffectsCleared", lua.create_table()?, None);
        }
        MemberEffectsEvent::Refollowed { cleared, added } => {
            if cleared {
                ap.fire_event(list, "EffectsCleared", lua.create_table()?, None);
            }
            for index in 1..=added.min(MAX_REFOLLOW_EFFECTS) {
                indexed("EffectAdded", index)?;
            }
        }
    }
    Ok(())
}

fn deliver_inner(ap: &Apartment, ev: &PartyViewEvent, seq: u64) -> mlua::Result<()> {
    let lua = ap.lua();
    if let PartyViewEvent::MemberEffects { members, event } = ev {
        // Further rules 7–8: the members' own lists, whether or not the member is still
        // listed or its episode current.
        for (episode, name) in members {
            if let Some(list) =
                crate::combat::existing_member_list(lua, &member_key(*episode, name), seq)?
            {
                fire_member_effects(ap, &list, *event)?;
            }
        }
        return Ok(());
    }
    let (episode, name, member) = match ev {
        PartyViewEvent::MemberAdded {
            episode,
            name,
            entity,
        } => (*episode, "MemberAdded", Some((name, *entity))),
        PartyViewEvent::MemberRemoved {
            episode,
            name,
            entity,
        } => (*episode, "MemberRemoved", Some((name, *entity))),
        PartyViewEvent::LeaderChanged { episode } => (*episode, "LeaderChanged", None),
        PartyViewEvent::MemberEffects { .. } => return Ok(()),
    };
    // Rule 2: the `Party` of an episode that ended before delivery gets no events.
    if !is_current(&ap.ctx, episode) {
        return Ok(());
    }
    let Some(target) = existing_party(lua, episode)? else {
        return Ok(());
    };
    let args = lua.create_table()?;
    if let Some((member_name, entity)) = member {
        let values = actor_values(actor_source(&ap.ctx).as_ref(), member_name, entity);
        let obj = member_object(
            lua,
            &ap.ctx,
            episode,
            member_name,
            values.unwrap_or_default(),
            entity,
        )?;
        // The values at delivery become the object's last values seen; a source without
        // values for the entity leaves the last values the plugin saw.
        if let Some(values) = values {
            with_payload(lua, &obj, |p: &MemberPayload| *lock(&p.last) = values)?;
        }
        args.raw_set("Player", obj)?;
    }
    ap.fire_event(&target, name, args, None);
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;

    fn m(name: &str, entity: Option<u64>, leader: bool) -> FellowshipMember {
        FellowshipMember {
            name: name.into(),
            entity,
            leader,
        }
    }

    fn tam() -> PartyPlayer {
        PartyPlayer {
            name: "Tam".into(),
            entity: Some(1),
        }
    }

    fn kinds(ev: &[GameplayEvent]) -> Vec<String> {
        ev.iter()
            .map(|e| match e {
                GameplayEvent::LocalPlayer(l) => l.name().to_owned(),
                GameplayEvent::Party(PartyViewEvent::MemberAdded { name, .. }) => {
                    format!("+{name}")
                }
                GameplayEvent::Party(PartyViewEvent::MemberRemoved { name, .. }) => {
                    format!("-{name}")
                }
                GameplayEvent::Party(PartyViewEvent::LeaderChanged { .. }) => "leader".into(),
                other => format!("{other:?}"),
            })
            .collect()
    }

    #[test]
    fn worked_example() {
        let mut p = PartyModel::default();
        let (ev, u) = p.apply(
            &[m("Tam", Some(1), true), m("Brin", Some(2), false)],
            &tam(),
        );
        assert_eq!(kinds(&ev), ["PartyChanged"]);
        assert!(!u.player_appended);
        assert_eq!(p.episode(), Some(1));
        assert_eq!(p.leader_name(), Some("Tam"));
        let list = [
            m("Tam", Some(1), true),
            m("Brin", Some(2), false),
            m("Cael", None, false),
        ];
        let (ev, _) = p.apply(&list, &tam());
        assert_eq!(kinds(&ev), ["+Cael"]);
        // Identical list: nothing.
        assert!(p.apply(&list, &tam()).0.is_empty());
        let (ev, _) = p.apply(
            &[
                m("Tam", Some(1), false),
                m("Brin", Some(2), true),
                m("Cael", None, false),
            ],
            &tam(),
        );
        assert_eq!(kinds(&ev), ["leader"]);
        let (ev, _) = p.apply(
            &[m("Tam", Some(1), false), m("Brin", Some(2), true)],
            &tam(),
        );
        assert_eq!(kinds(&ev), ["-Cael"]);
        let (ev, _) = p.apply(&[], &tam());
        assert_eq!(kinds(&ev), ["PartyChanged"]);
        assert_eq!(p.episode(), None);
        assert!(p.apply(&[], &tam()).0.is_empty());
        let _ = p.apply(&[m("Tam", Some(1), true)], &tam());
        assert_eq!(p.episode(), Some(2));
    }

    #[test]
    fn order_of_removed_added_leader_and_order_only_change() {
        let mut p = PartyModel::default();
        let _ = p.apply(
            &[m("A", None, true), m("B", None, false), m("C", None, false)],
            &tam(),
        );
        // The player is appended (not listed, no entity match).
        assert_eq!(p.members.len(), 4);
        assert!(p.members[3].is_player);
        let (ev, u) = p.apply(
            &[m("D", None, false), m("B", None, true), m("E", None, false)],
            &tam(),
        );
        assert!(u.player_appended);
        assert_eq!(kinds(&ev), ["-A", "-C", "+D", "+E", "leader"]);
        let (ev, _) = p.apply(
            &[m("E", None, false), m("B", None, true), m("D", None, false)],
            &tam(),
        );
        assert!(ev.is_empty());
        assert_eq!(p.member(0).map(|r| r.name), Some("E".into()));
    }

    #[test]
    fn player_by_entity_then_name_and_truncation() {
        let mut p = PartyModel::default();
        // By entity: "Tam" at index 0 is another character; the player is entity 1.
        let _ = p.apply(
            &[m("Tam", Some(9), false), m("Other", Some(1), false)],
            &tam(),
        );
        assert!(!p.members[0].is_player && p.members[1].is_player);
        assert!(p.has_other("Tam"));
        // By name when no entity matches.
        let mut p = PartyModel::default();
        let _ = p.apply(&[m("X", None, false), m("Tam", None, false)], &tam());
        assert!(p.members[1].is_player);
        // Truncation.
        let mut p = PartyModel::default();
        let long: Vec<FellowshipMember> =
            (0..200).map(|i| m(&format!("N{i}"), None, false)).collect();
        let (_, u) = p.apply(&long, &tam());
        assert!(u.truncated && u.player_appended);
        assert_eq!(p.members.len(), MAX_PARTY_MEMBERS + 1);
    }

    #[test]
    fn duplicate_names_count_once() {
        let mut p = PartyModel::default();
        let _ = p.apply(&[m("Tam", None, true)], &tam());
        let (ev, _) = p.apply(
            &[
                m("Tam", None, true),
                m("B", None, false),
                m("B", None, false),
            ],
            &tam(),
        );
        assert_eq!(kinds(&ev), ["+B"]);
    }

    #[test]
    fn member_events_carry_the_entity() {
        let mut p = PartyModel::default();
        let _ = p.apply(&[m("Tam", Some(1), true)], &tam());
        let (ev, _) = p.apply(&[m("Tam", Some(1), true), m("B", Some(7), false)], &tam());
        assert!(matches!(
            &ev[0],
            GameplayEvent::Party(PartyViewEvent::MemberAdded {
                entity: Some(7),
                ..
            })
        ));
        let (ev, _) = p.apply(&[m("Tam", Some(1), true)], &tam());
        assert!(matches!(
            &ev[0],
            GameplayEvent::Party(PartyViewEvent::MemberRemoved {
                entity: Some(7),
                ..
            })
        ));
    }
}
