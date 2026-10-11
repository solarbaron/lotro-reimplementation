// SPDX-License-Identifier: GPL-3.0-or-later
//! The local player's combat state from the session's events (decision 0024 P2,
//! `combat-ui.md` §11 "kept", `lua-plugin-api.md` §15.2): the selection, every entity's
//! kept vitals, level, combat state, defeat, target, effects and cast, the quickslots,
//! running cooldowns, learned skills, the last attacker and the previous selection.
//!
//! [`CombatState::apply`] takes every session event in order with the time of its arrival
//! (seconds of the caller's clock: the scenario's virtual clock in headless runs, CUI9) and
//! returns what changed as [`CombatChange`]s for the combat widgets. It never sends a
//! command (CUI8): selection is shown only when the session reports it
//! (`TargetChanged` for the player, TR3), and a selection whose entity is removed is
//! cleared locally (decision 0032 D2), with [`CombatChange::Selection`] marked `local`.
//!
//! Values are scripted by the session, never derived (decision 0024 §2.4): nothing here
//! computes damage, reactions or levels.
//!
//! # Hostile input
//!
//! Every collection is bounded: [`MAX_ENTITIES`] entities (spawns beyond it are ignored),
//! [`MAX_EFFECTS_PER_ENTITY`] effects per entity, [`MAX_QUICKSLOTS`] quickslots,
//! [`MAX_COOLDOWNS`] running cooldowns, [`MAX_SKILLS`] learned skills and
//! [`MAX_CHARACTER_LEVELS`] remembered character levels. Non-finite numbers are dropped.

use std::collections::{BTreeMap, HashMap};

use tessera_net::domain::{
    CastKind, CastOutcome, DeltaKind, HitResult, Ref, Shortcut, Vital as NetVital,
};
use tessera_net::{
    Body, CharacterEvent, CombatEvent, EntityId, EntityKind, Event, ProgressionEvent,
};

/// Most entities kept (spawns beyond it are ignored, with a warning).
pub const MAX_ENTITIES: usize = 65_536;
/// Most active effect instances kept per entity (`scenario-feed.md` §11.1: 1,024).
pub const MAX_EFFECTS_PER_ENTITY: usize = 1024;
/// Most quickslots kept (`scenario-feed.md` §6.2: slots 1 … 1,024).
pub const MAX_QUICKSLOTS: usize = 1024;
/// Most running cooldowns kept.
pub const MAX_COOLDOWNS: usize = 4096;
/// Most learned skills kept (`lua-plugin-api.md` §15.2 implementer note (8)).
pub const MAX_SKILLS: usize = 4096;
/// Most `CharacterInfo` levels remembered (for the entered character's level).
pub const MAX_CHARACTER_LEVELS: usize = 1024;
/// Most changes one [`CombatState::apply`] call adds, counted per call (not over the
/// caller's whole vector). Removals ([`CombatChange::Despawned`]) and the local selection
/// clear of a removal are exempt: [`MAX_ENTITIES`] already bounds them (`StaticsCleared`
/// removes every placed entity in one event).
pub const MAX_CHANGES_PER_EVENT: usize = 64;
/// Most distinct recent attackers remembered for TR4 *last attacker*.
pub const MAX_ATTACKERS: usize = 16;
/// Longest time (seconds) a duration, delay or cooldown may span; longer values are cut
/// (100 days; the feed's limits are far below).
pub const MAX_SPAN: f64 = 8_640_000.0;

/// A vital as last reported (`VitalChanged { current, max }`).
#[derive(Debug, Clone, Copy, PartialEq, Default)]
pub struct KeptVital {
    /// The current value.
    pub current: f32,
    /// The maximum.
    pub max: f32,
}

/// An effect instance as applied (`EffectApplied`, TR30).
#[derive(Debug, Clone, PartialEq)]
pub struct KeptEffect {
    /// The instance (`EffectId`).
    pub instance: u32,
    /// The effect.
    pub what: Ref,
    /// Arrival time, seconds.
    pub start: f64,
    /// Duration in seconds, `None` without one.
    pub duration: Option<f64>,
    /// The stack count.
    pub stacks: u16,
    /// Who applied it.
    pub source: Option<EntityId>,
}

/// A cast as started (`CastStarted`, TR26).
#[derive(Debug, Clone, PartialEq)]
pub struct KeptCast {
    /// The kind of bar.
    pub kind: CastKind,
    /// What is cast.
    pub what: Ref,
    /// Its target.
    pub target: Option<EntityId>,
    /// Arrival time, seconds.
    pub start: f64,
    /// Total duration in seconds (`duration_ms` plus every `added_ms` so far).
    pub total: f64,
}

/// What the client keeps of one entity (`combat-ui.md` §11).
#[derive(Debug, Clone, PartialEq)]
pub struct EntityCombat {
    /// The session's classification.
    pub kind: EntityKind,
    /// Its name (empty when none).
    pub name: String,
    /// Its body, when its creation or an appearance change carried one.
    pub body: Option<Body>,
    /// Its last position (the player's current region frame), when known.
    pub position: Option<[f32; 3]>,
    /// Health.
    pub health: Option<KeptVital>,
    /// Power.
    pub power: Option<KeptVital>,
    /// The class resource and its ref.
    pub resource: Option<(Ref, KeptVital)>,
    /// The last reported level.
    pub level: Option<u32>,
    /// In combat (`CombatState`).
    pub in_combat: bool,
    /// Defeated and not revived (TR39).
    pub defeated: bool,
    /// Its own target, as last reported.
    pub target: Option<EntityId>,
    /// Active effect instances, in arrival order.
    pub effects: Vec<KeptEffect>,
    /// The active cast.
    pub cast: Option<KeptCast>,
    /// Whether the entity is a static of the world data (cleared by `StaticsCleared`).
    pub placed: bool,
}

impl EntityCombat {
    fn new(kind: EntityKind, name: &str) -> Self {
        Self {
            kind,
            name: name.to_owned(),
            body: None,
            position: None,
            health: None,
            power: None,
            resource: None,
            level: None,
            in_combat: false,
            defeated: false,
            target: None,
            effects: Vec::new(),
            cast: None,
            placed: false,
        }
    }

    /// Whether the entity has vitals for display (TR12: a health report arrived).
    #[must_use]
    pub fn has_vitals(&self) -> bool {
        self.health.is_some()
    }
}

/// A running cooldown (`CooldownStarted`, TR35).
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct KeptCooldown {
    /// Arrival time, seconds.
    pub start: f64,
    /// Duration, seconds.
    pub duration: f64,
}

impl KeptCooldown {
    /// The remaining seconds at `now` (0 when over).
    #[must_use]
    pub fn remaining(&self, now: f64) -> f64 {
        (self.start + self.duration - now).max(0.0)
    }
}

/// A floating-text cause (`VitalDelta`, TR19–TR21).
#[derive(Debug, Clone, PartialEq)]
pub struct Delta {
    /// Who caused it.
    pub source: Option<EntityId>,
    /// Who took it.
    pub target: EntityId,
    /// Damage or heal.
    pub kind: DeltaKind,
    /// The amount.
    pub amount: f32,
    /// The result.
    pub result: HitResult,
}

/// What one event changed, for the combat widgets (`combat-ui.md` §11).
#[derive(Debug, Clone, PartialEq)]
pub enum CombatChange {
    /// The player entered the world (everything is fresh).
    WorldEntered,
    /// The session ended or the player left the world: every display clears.
    Left,
    /// The selection changed; `local` when the client cleared it itself (a despawn,
    /// decision 0032 D2), otherwise reported by `TargetChanged`.
    Selection {
        /// The selection before.
        previous: Option<EntityId>,
        /// The selection now.
        current: Option<EntityId>,
        /// Cleared by the client.
        local: bool,
    },
    /// An entity's frame values changed (name, kind, body, vitals, level, defeat).
    Entity(EntityId),
    /// Another entity's own target changed (E7, TR4 assist).
    Target(EntityId),
    /// An entity's combat state changed.
    InCombat(EntityId),
    /// A quickslot changed (feed slot number).
    Quickslot(u16),
    /// A cast started.
    CastStarted(EntityId),
    /// A cast was delayed by this many seconds.
    CastDelayed(EntityId, f64),
    /// A cast ended.
    CastEnded(EntityId, CastOutcome),
    /// A cooldown of this ref started.
    CooldownStarted(Ref),
    /// A cooldown of this ref ended.
    CooldownEnded(Ref),
    /// An effect instance was applied.
    EffectApplied(EntityId, u32),
    /// An effect instance was removed.
    EffectRemoved(EntityId, u32),
    /// A floating text's cause.
    Delta(Delta),
    /// The entity was defeated.
    Defeated(EntityId),
    /// The entity was revived.
    Revived(EntityId),
    /// The entity was removed (its kept values are gone).
    Despawned(EntityId),
    /// A skill was learned.
    SkillLearned(Ref),
    /// A skill was forgotten.
    SkillForgotten(Ref),
}

/// The local player's combat state (module docs).
#[derive(Debug, Clone, Default)]
pub struct CombatState {
    player: Option<EntityId>,
    entities: HashMap<EntityId, EntityCombat>,
    selection: Option<EntityId>,
    previous_selection: Option<EntityId>,
    /// Sources of damage on the player, most recent first, each once (TR4 *last
    /// attacker*), at most [`MAX_ATTACKERS`].
    attackers: Vec<EntityId>,
    quickslots: BTreeMap<u16, Shortcut>,
    cooldowns: HashMap<Ref, KeptCooldown>,
    skills: Vec<Ref>,
    character_levels: HashMap<String, u32>,
    warned: bool,
}

fn finite_secs(ms: u32) -> f64 {
    (f64::from(ms) / 1000.0).min(MAX_SPAN)
}

impl CombatState {
    /// An empty state (no player in the world).
    #[must_use]
    pub fn new() -> Self {
        Self::default()
    }

    /// The player's entity, while in the world.
    #[must_use]
    pub fn player(&self) -> Option<EntityId> {
        self.player
    }

    /// The current selection.
    #[must_use]
    pub fn selection(&self) -> Option<EntityId> {
        self.selection
    }

    /// The selection before the current one (TR4 *last*), when it still exists.
    #[must_use]
    pub fn previous_selection(&self) -> Option<EntityId> {
        self.previous_selection
            .filter(|e| self.entities.contains_key(e))
    }

    /// TR4 *last attacker*: the source of the most recent `VitalDelta` (target the player,
    /// kind `Damage`) whose source still exists. The player counts as a source too (TR4
    /// excludes nobody). Only the [`MAX_ATTACKERS`] most recent distinct sources are
    /// remembered.
    #[must_use]
    pub fn last_attacker(&self) -> Option<EntityId> {
        self.attackers
            .iter()
            .copied()
            .find(|e| self.entities.contains_key(e))
    }

    /// The local player's own position (its region frame, the frame of the other entities'
    /// positions), which the session does not echo while the player walks: the client
    /// reports it as it moves, so TR4 measures from where the player is. Non-finite values
    /// are ignored; nothing happens outside the world.
    pub fn set_player_position(&mut self, p: [f32; 3]) {
        if let Some(me) = self.player {
            self.set_position(me, tessera_net::Vec3::new(p[0], p[1], p[2]));
        }
    }

    /// The kept values of an entity (the player included).
    #[must_use]
    pub fn entity(&self, id: EntityId) -> Option<&EntityCombat> {
        self.entities.get(&id)
    }

    /// Every kept entity.
    pub fn entities(&self) -> impl Iterator<Item = (EntityId, &EntityCombat)> {
        self.entities.iter().map(|(k, v)| (*k, v))
    }

    /// The player's kept values.
    #[must_use]
    pub fn player_entity(&self) -> Option<&EntityCombat> {
        self.entity(self.player?)
    }

    /// Whether the player is in combat (TR37).
    #[must_use]
    pub fn in_combat(&self) -> bool {
        self.player_entity().is_some_and(|e| e.in_combat)
    }

    /// The content of feed slot `slot` (1 … 1,024).
    #[must_use]
    pub fn quickslot(&self, slot: u16) -> Option<&Shortcut> {
        self.quickslots.get(&slot)
    }

    /// Every filled quickslot, by feed slot number.
    pub fn quickslots(&self) -> impl Iterator<Item = (u16, &Shortcut)> {
        self.quickslots.iter().map(|(k, v)| (*k, v))
    }

    /// The running cooldown of `what`.
    #[must_use]
    pub fn cooldown(&self, what: &Ref) -> Option<KeptCooldown> {
        self.cooldowns.get(what).copied()
    }

    /// Every running cooldown.
    pub fn cooldowns(&self) -> impl Iterator<Item = (&Ref, KeptCooldown)> {
        self.cooldowns.iter().map(|(k, v)| (k, *v))
    }

    /// The learned skills, in the order of their first report.
    #[must_use]
    pub fn skills(&self) -> &[Ref] {
        &self.skills
    }

    /// The player's level: its last `LevelChanged`, else the `CharacterInfo.level` of the
    /// character entered.
    #[must_use]
    pub fn player_level(&self) -> Option<u32> {
        self.player_entity().and_then(|e| e.level)
    }

    /// Adds a capped change: `out` holds only the current call's changes (see
    /// [`Self::apply`]).
    fn change(out: &mut Vec<CombatChange>, c: CombatChange) {
        if out.len() < MAX_CHANGES_PER_EVENT {
            out.push(c);
        }
    }

    fn insert_entity(&mut self, id: EntityId, kind: EntityKind, name: &str) -> bool {
        if let Some(e) = self.entities.get_mut(&id) {
            e.kind = kind;
            name.clone_into(&mut e.name);
            return true;
        }
        if self.entities.len() >= MAX_ENTITIES {
            if !self.warned {
                self.warned = true;
                tracing::warn!("combat state: entity limit reached; further spawns ignored");
            }
            return false;
        }
        self.entities.insert(id, EntityCombat::new(kind, name));
        true
    }

    fn reset(&mut self) {
        let levels = std::mem::take(&mut self.character_levels);
        *self = Self {
            character_levels: levels,
            ..Self::default()
        };
    }

    fn remove_entity(&mut self, id: EntityId, out: &mut Vec<CombatChange>) {
        if Some(id) == self.player {
            return;
        }
        if self.entities.remove(&id).is_none() {
            return;
        }
        if self.selection == Some(id) {
            // Decision 0032 D2: cleared locally, no command. Never capped: the widgets
            // would keep showing the removed entity.
            self.previous_selection = None;
            self.selection = None;
            if let Some(p) = self.player.and_then(|p| self.entities.get_mut(&p)) {
                p.target = None;
            }
            out.push(CombatChange::Selection {
                previous: Some(id),
                current: None,
                local: true,
            });
        }
        if self.previous_selection == Some(id) {
            self.previous_selection = None;
        }
        // Not capped either: MAX_ENTITIES bounds the removals of one event.
        out.push(CombatChange::Despawned(id));
    }

    /// Applies one session event that arrived at `now` (seconds) and appends what changed
    /// to `out` (module docs). At most [`MAX_CHANGES_PER_EVENT`] capped changes are added
    /// per call, whatever `out` already holds.
    pub fn apply(&mut self, ev: &Event, now: f64, out: &mut Vec<CombatChange>) {
        let mut mine = Vec::new();
        self.apply_one(ev, now, &mut mine);
        out.append(&mut mine);
    }

    fn apply_one(&mut self, ev: &Event, now: f64, out: &mut Vec<CombatChange>) {
        let now = if now.is_finite() { now } else { 0.0 };
        match ev {
            Event::Connected { .. } => {}
            Event::WorldEntered {
                player,
                name,
                transform,
            } => {
                let was = self.player.is_some();
                self.reset();
                if was {
                    Self::change(out, CombatChange::Left);
                }
                self.player = Some(*player);
                self.insert_entity(*player, EntityKind::Player, name);
                let level = self
                    .character_levels
                    .get(&crate::net::name_key(name))
                    .copied();
                if let Some(e) = self.entities.get_mut(player) {
                    e.level = level;
                    let p = transform.position;
                    e.position = [p.x, p.y, p.z]
                        .iter()
                        .all(|v| v.is_finite())
                        .then_some([p.x, p.y, p.z]);
                }
                Self::change(out, CombatChange::WorldEntered);
            }
            Event::EntitySpawned {
                id,
                kind,
                name,
                transform,
            } => {
                if Some(*id) == self.player {
                    return;
                }
                if self.insert_entity(*id, *kind, name) {
                    self.set_position(*id, transform.position);
                    Self::change(out, CombatChange::Entity(*id));
                }
            }
            Event::EntityCreated {
                id,
                kind,
                name,
                location,
                appearance,
                tag,
                ..
            } => {
                if Some(*id) == self.player {
                    return;
                }
                if self.insert_entity(*id, *kind, name) {
                    if let Some(e) = self.entities.get_mut(id) {
                        e.body = Some(appearance.body);
                        e.placed = tag.is_some();
                    }
                    self.set_position(*id, location.position());
                    Self::change(out, CombatChange::Entity(*id));
                }
            }
            Event::AppearanceChanged { id, appearance } => {
                if let Some(e) = self.entities.get_mut(id) {
                    e.body = Some(appearance.body);
                    Self::change(out, CombatChange::Entity(*id));
                }
            }
            Event::EntityMoved { id, transform, .. } => {
                self.set_position(*id, transform.position);
            }
            Event::EntityDespawned { id } => self.remove_entity(*id, out),
            Event::StaticsCleared => {
                let mut placed: Vec<EntityId> = self
                    .entities
                    .iter()
                    .filter(|(_, e)| e.placed)
                    .map(|(k, _)| *k)
                    .collect();
                // The selection's removal (and its local clear) first, then by ID: a
                // stable order whatever the map's iteration order.
                let sel = self.selection;
                placed.sort_by_key(|e| (Some(*e) != sel, e.0));
                for id in placed {
                    self.remove_entity(id, out);
                }
            }
            Event::Disconnected { .. } => {
                let was = self.player.is_some();
                self.reset();
                if was {
                    Self::change(out, CombatChange::Left);
                }
            }
            Event::Character(c) => self.character(c, out),
            Event::Progression(p) => self.progression(p, out),
            Event::Combat(c) => self.combat(c, now, out),
            _ => {}
        }
    }

    fn set_position(&mut self, id: EntityId, p: tessera_net::Vec3) {
        if let Some(e) = self.entities.get_mut(&id)
            && p.x.is_finite()
            && p.y.is_finite()
            && p.z.is_finite()
        {
            e.position = Some([p.x, p.y, p.z]);
        }
    }

    fn character(&mut self, c: &CharacterEvent, out: &mut Vec<CombatChange>) {
        match c {
            CharacterEvent::CharacterInfo {
                name,
                level: Some(level),
                ..
            } => {
                let key = crate::net::name_key(name);
                if self.character_levels.contains_key(&key)
                    || self.character_levels.len() < MAX_CHARACTER_LEVELS
                {
                    self.character_levels.insert(key, *level);
                }
            }
            CharacterEvent::LevelChanged { entity, level } => {
                if let Some(e) = self.entities.get_mut(entity) {
                    e.level = Some(*level);
                    Self::change(out, CombatChange::Entity(*entity));
                }
            }
            _ => {}
        }
    }

    fn progression(&mut self, p: &ProgressionEvent, out: &mut Vec<CombatChange>) {
        match p {
            ProgressionEvent::SkillLearned { what } => {
                if !self.skills.contains(what) && self.skills.len() < MAX_SKILLS {
                    self.skills.push(what.clone());
                    Self::change(out, CombatChange::SkillLearned(what.clone()));
                }
            }
            ProgressionEvent::SkillForgotten { what } => {
                if let Some(i) = self.skills.iter().position(|s| s == what) {
                    self.skills.remove(i);
                    Self::change(out, CombatChange::SkillForgotten(what.clone()));
                }
            }
            _ => {}
        }
    }

    #[allow(clippy::too_many_lines)]
    fn combat(&mut self, c: &CombatEvent, now: f64, out: &mut Vec<CombatChange>) {
        match c {
            CombatEvent::TargetChanged { entity, target } => {
                if Some(*entity) == self.player {
                    if self.selection != *target {
                        let previous = self.selection;
                        if previous.is_some() {
                            self.previous_selection = previous;
                        }
                        self.selection = *target;
                        if let Some(p) = self.player.and_then(|p| self.entities.get_mut(&p)) {
                            p.target = *target;
                        }
                        Self::change(
                            out,
                            CombatChange::Selection {
                                previous,
                                current: *target,
                                local: false,
                            },
                        );
                    }
                } else if let Some(e) = self.entities.get_mut(entity)
                    && e.target != *target
                {
                    e.target = *target;
                    Self::change(out, CombatChange::Target(*entity));
                }
            }
            CombatEvent::VitalChanged {
                entity,
                vital,
                current,
                max,
            } => {
                if !(current.is_finite() && max.is_finite()) {
                    return;
                }
                let Some(e) = self.entities.get_mut(entity) else {
                    return;
                };
                let v = KeptVital {
                    current: *current,
                    max: *max,
                };
                match vital {
                    NetVital::Health => e.health = Some(v),
                    NetVital::Power => e.power = Some(v),
                    NetVital::Resource(r) => e.resource = Some((r.clone(), v)),
                }
                Self::change(out, CombatChange::Entity(*entity));
            }
            CombatEvent::CombatState { entity, in_combat } => {
                if let Some(e) = self.entities.get_mut(entity)
                    && e.in_combat != *in_combat
                {
                    e.in_combat = *in_combat;
                    Self::change(out, CombatChange::InCombat(*entity));
                }
            }
            CombatEvent::QuickslotChanged { slot, content } => {
                if *slot == 0 || usize::from(*slot) > MAX_QUICKSLOTS {
                    return;
                }
                match content {
                    Some(s) => {
                        self.quickslots.insert(*slot, s.clone());
                    }
                    None => {
                        self.quickslots.remove(slot);
                    }
                }
                Self::change(out, CombatChange::Quickslot(*slot));
            }
            CombatEvent::CastStarted {
                entity,
                cast,
                what,
                target,
                duration_ms,
            } => {
                let Some(e) = self.entities.get_mut(entity) else {
                    return;
                };
                e.cast = Some(KeptCast {
                    kind: *cast,
                    what: what.clone(),
                    target: *target,
                    start: now,
                    total: finite_secs(*duration_ms),
                });
                Self::change(out, CombatChange::CastStarted(*entity));
            }
            CombatEvent::CastDelayed { entity, added_ms } => {
                if let Some(c) = self.entities.get_mut(entity).and_then(|e| e.cast.as_mut()) {
                    let added = finite_secs(*added_ms);
                    c.total = (c.total + added).min(MAX_SPAN);
                    Self::change(out, CombatChange::CastDelayed(*entity, added));
                }
            }
            CombatEvent::CastEnded { entity, outcome } => {
                if let Some(e) = self.entities.get_mut(entity)
                    && e.cast.take().is_some()
                {
                    Self::change(out, CombatChange::CastEnded(*entity, *outcome));
                }
            }
            CombatEvent::CooldownStarted { what, duration_ms } => {
                if !self.cooldowns.contains_key(what) && self.cooldowns.len() >= MAX_COOLDOWNS {
                    return;
                }
                self.cooldowns.insert(
                    what.clone(),
                    KeptCooldown {
                        start: now,
                        duration: finite_secs(*duration_ms),
                    },
                );
                Self::change(out, CombatChange::CooldownStarted(what.clone()));
            }
            CombatEvent::CooldownEnded { what } => {
                self.cooldowns.remove(what);
                Self::change(out, CombatChange::CooldownEnded(what.clone()));
            }
            CombatEvent::EffectApplied {
                entity,
                effect,
                what,
                duration_ms,
                stacks,
                source,
            } => {
                let Some(e) = self.entities.get_mut(entity) else {
                    return;
                };
                let kept = KeptEffect {
                    instance: effect.0,
                    what: what.clone(),
                    start: now,
                    duration: duration_ms.map(finite_secs),
                    stacks: *stacks,
                    source: *source,
                };
                if let Some(old) = e.effects.iter_mut().find(|x| x.instance == effect.0) {
                    *old = kept;
                } else if e.effects.len() < MAX_EFFECTS_PER_ENTITY {
                    e.effects.push(kept);
                } else {
                    return;
                }
                Self::change(out, CombatChange::EffectApplied(*entity, effect.0));
            }
            CombatEvent::EffectRemoved { entity, effect } => {
                if let Some(e) = self.entities.get_mut(entity)
                    && let Some(i) = e.effects.iter().position(|x| x.instance == effect.0)
                {
                    e.effects.remove(i);
                    Self::change(out, CombatChange::EffectRemoved(*entity, effect.0));
                }
            }
            CombatEvent::VitalDelta {
                source,
                target,
                kind,
                amount,
                result,
                ..
            } => {
                if !amount.is_finite() {
                    return;
                }
                if Some(*target) == self.player
                    && *kind == DeltaKind::Damage
                    && let Some(s) = source
                {
                    self.attackers.retain(|a| a != s);
                    self.attackers.insert(0, *s);
                    self.attackers.truncate(MAX_ATTACKERS);
                }
                Self::change(
                    out,
                    CombatChange::Delta(Delta {
                        source: *source,
                        target: *target,
                        kind: *kind,
                        amount: *amount,
                        result: *result,
                    }),
                );
            }
            CombatEvent::Defeated { entity, .. } => {
                if let Some(e) = self.entities.get_mut(entity) {
                    e.defeated = true;
                    // TR39: an active cast ends as interrupted.
                    let had_cast = e.cast.take().is_some();
                    if had_cast {
                        Self::change(
                            out,
                            CombatChange::CastEnded(*entity, CastOutcome::Interrupted),
                        );
                    }
                    Self::change(out, CombatChange::Defeated(*entity));
                    Self::change(out, CombatChange::Entity(*entity));
                }
            }
            CombatEvent::Revived { entity } => {
                if let Some(e) = self.entities.get_mut(entity) {
                    e.defeated = false;
                    Self::change(out, CombatChange::Revived(*entity));
                    Self::change(out, CombatChange::Entity(*entity));
                }
            }
            _ => {}
        }
    }

    /// Clears the selection locally (decision 0032 D2; tests and tools). Returns the change
    /// when there was one.
    pub fn clear_selection_locally(&mut self) -> Option<CombatChange> {
        let previous = self.selection.take()?;
        self.previous_selection = Some(previous);
        if let Some(p) = self.player.and_then(|p| self.entities.get_mut(&p)) {
            p.target = None;
        }
        Some(CombatChange::Selection {
            previous: Some(previous),
            current: None,
            local: true,
        })
    }
}

/// The feed slot of shortcut index `index` (TR32: slot *N* is index *N* − 1).
#[must_use]
pub fn slot_of_index(index: i32) -> Option<u16> {
    u16::try_from(index.checked_add(1)?)
        .ok()
        .filter(|s| *s >= 1 && usize::from(*s) <= MAX_QUICKSLOTS)
}

/// The shortcut index of feed slot `slot` (TR32).
#[must_use]
pub fn index_of_slot(slot: u16) -> i32 {
    i32::from(slot) - 1
}

#[cfg(test)]
mod tests {
    use super::*;
    use tessera_net::domain::EffectId;
    use tessera_net::{Transform, Vec3};

    fn id(n: u64) -> EntityId {
        EntityId(n)
    }

    fn entered(s: &mut CombatState) -> Vec<CombatChange> {
        let mut out = Vec::new();
        s.apply(
            &Event::WorldEntered {
                player: id(1),
                name: "Tamsin".into(),
                transform: Transform::new(Vec3::ZERO, 0.0),
            },
            0.0,
            &mut out,
        );
        out
    }

    fn spawn(s: &mut CombatState, n: u64, kind: EntityKind, pos: (f32, f32)) {
        let mut out = Vec::new();
        s.apply(
            &Event::EntitySpawned {
                id: id(n),
                kind,
                name: format!("e{n}"),
                transform: Transform::new(Vec3::new(pos.0, pos.1, 0.0), 0.0),
            },
            0.0,
            &mut out,
        );
    }

    fn combat(s: &mut CombatState, c: CombatEvent, now: f64) -> Vec<CombatChange> {
        let mut out = Vec::new();
        s.apply(&Event::Combat(c), now, &mut out);
        out
    }

    #[test]
    fn selection_is_shown_only_when_reported_and_cleared_locally_on_despawn() {
        let mut s = CombatState::new();
        assert_eq!(entered(&mut s), [CombatChange::WorldEntered]);
        spawn(&mut s, 7, EntityKind::Creature, (0.0, 5.0));
        assert_eq!(s.selection(), None);
        let c = combat(
            &mut s,
            CombatEvent::TargetChanged {
                entity: id(1),
                target: Some(id(7)),
            },
            1.0,
        );
        assert_eq!(
            c,
            [CombatChange::Selection {
                previous: None,
                current: Some(id(7)),
                local: false
            }]
        );
        // A repeated report changes nothing.
        assert!(
            combat(
                &mut s,
                CombatEvent::TargetChanged {
                    entity: id(1),
                    target: Some(id(7)),
                },
                1.1,
            )
            .is_empty()
        );
        let mut out = Vec::new();
        s.apply(&Event::EntityDespawned { id: id(7) }, 2.0, &mut out);
        assert_eq!(
            out,
            [
                CombatChange::Selection {
                    previous: Some(id(7)),
                    current: None,
                    local: true
                },
                CombatChange::Despawned(id(7))
            ]
        );
        assert_eq!(s.selection(), None);
        // Created again: not selected again by itself (D2).
        spawn(&mut s, 7, EntityKind::Creature, (0.0, 5.0));
        assert_eq!(s.selection(), None);
    }

    #[test]
    fn kept_values_casts_effects_and_defeat() {
        let mut s = CombatState::new();
        entered(&mut s);
        spawn(&mut s, 7, EntityKind::Creature, (0.0, 5.0));
        combat(
            &mut s,
            CombatEvent::VitalChanged {
                entity: id(7),
                vital: NetVital::Health,
                current: 50.0,
                max: 100.0,
            },
            0.0,
        );
        assert!(s.entity(id(7)).unwrap().has_vitals());
        // Non-finite vitals are dropped.
        assert!(
            combat(
                &mut s,
                CombatEvent::VitalChanged {
                    entity: id(7),
                    vital: NetVital::Power,
                    current: f32::NAN,
                    max: 1.0,
                },
                0.0,
            )
            .is_empty()
        );
        combat(
            &mut s,
            CombatEvent::CastStarted {
                entity: id(7),
                cast: CastKind::Cast,
                what: Ref::Literal("Gore".into()),
                target: Some(id(1)),
                duration_ms: 1500,
            },
            4.1,
        );
        combat(
            &mut s,
            CombatEvent::CastDelayed {
                entity: id(7),
                added_ms: 500,
            },
            4.2,
        );
        let cast = s.entity(id(7)).unwrap().cast.clone().unwrap();
        assert!((cast.total - 2.0).abs() < 1e-9 && (cast.start - 4.1).abs() < 1e-9);
        combat(
            &mut s,
            CombatEvent::EffectApplied {
                entity: id(7),
                effect: EffectId(3),
                what: Ref::Literal("Bleeding".into()),
                duration_ms: Some(4000),
                stacks: 1,
                source: Some(id(1)),
            },
            3.4,
        );
        let c = combat(
            &mut s,
            CombatEvent::Defeated {
                entity: id(7),
                by: Some(id(1)),
            },
            5.0,
        );
        assert_eq!(
            c[0],
            CombatChange::CastEnded(id(7), CastOutcome::Interrupted)
        );
        let e = s.entity(id(7)).unwrap();
        assert!(e.defeated && e.cast.is_none());
        assert_eq!(e.effects.len(), 1, "TR39: effects stay");
        combat(&mut s, CombatEvent::Revived { entity: id(7) }, 6.0);
        assert!(!s.entity(id(7)).unwrap().defeated);
    }

    #[test]
    fn last_attacker_previous_selection_and_slots() {
        let mut s = CombatState::new();
        entered(&mut s);
        spawn(&mut s, 7, EntityKind::Creature, (0.0, 5.0));
        spawn(&mut s, 8, EntityKind::Object, (1.0, 1.0));
        for t in [Some(id(8)), Some(id(7))] {
            combat(
                &mut s,
                CombatEvent::TargetChanged {
                    entity: id(1),
                    target: t,
                },
                0.0,
            );
        }
        assert_eq!(s.previous_selection(), Some(id(8)));
        combat(
            &mut s,
            CombatEvent::VitalDelta {
                source: Some(id(7)),
                target: id(1),
                vital: NetVital::Health,
                kind: DeltaKind::Damage,
                amount: 5.0,
                result: HitResult::Normal,
                what: None,
                damage_type: None,
            },
            0.0,
        );
        assert_eq!(s.last_attacker(), Some(id(7)));
        combat(
            &mut s,
            CombatEvent::QuickslotChanged {
                slot: 0,
                content: Some(Shortcut::Skill(Ref::Literal("x".into()))),
            },
            0.0,
        );
        assert_eq!(s.quickslots().count(), 0, "slot 0 is out of range");
        combat(
            &mut s,
            CombatEvent::QuickslotChanged {
                slot: 1,
                content: Some(Shortcut::Skill(Ref::Literal("Jab".into()))),
            },
            0.0,
        );
        assert_eq!(slot_of_index(0), Some(1));
        assert_eq!(slot_of_index(-1), None);
        assert_eq!(slot_of_index(1024), None);
        assert_eq!(index_of_slot(12), 11);
        assert!(s.quickslot(1).is_some());
        let mut out = Vec::new();
        s.apply(
            &Event::Disconnected {
                reason: tessera_net::DisconnectReason::ClientRequested,
            },
            1.0,
            &mut out,
        );
        assert_eq!(out, [CombatChange::Left]);
        assert!(s.player().is_none() && s.quickslots().count() == 0);
    }

    fn damage(s: &mut CombatState, source: u64) {
        combat(
            s,
            CombatEvent::VitalDelta {
                source: Some(id(source)),
                target: id(1),
                vital: NetVital::Health,
                kind: DeltaKind::Damage,
                amount: 5.0,
                result: HitResult::Normal,
                what: None,
                damage_type: None,
            },
            0.0,
        );
    }

    #[test]
    fn last_attacker_falls_back_to_an_earlier_one_and_counts_the_player() {
        let mut s = CombatState::new();
        entered(&mut s);
        spawn(&mut s, 7, EntityKind::Creature, (0.0, 5.0));
        spawn(&mut s, 8, EntityKind::Creature, (0.0, 6.0));
        damage(&mut s, 7);
        damage(&mut s, 8);
        assert_eq!(s.last_attacker(), Some(id(8)));
        let mut out = Vec::new();
        s.apply(&Event::EntityDespawned { id: id(8) }, 1.0, &mut out);
        assert_eq!(
            s.last_attacker(),
            Some(id(7)),
            "the most recent that still exists"
        );
        damage(&mut s, 1);
        assert_eq!(
            s.last_attacker(),
            Some(id(1)),
            "self-inflicted damage counts"
        );
        for n in 0..(MAX_ATTACKERS as u64 + 5) {
            damage(&mut s, 100 + n);
        }
        assert!(s.attackers.len() <= MAX_ATTACKERS);
    }

    #[test]
    fn the_cap_is_per_call_and_statics_cleared_always_clears_the_selection() {
        let mut s = CombatState::new();
        entered(&mut s);
        let n_placed = MAX_CHANGES_PER_EVENT as u64 + 20;
        for n in 0..n_placed {
            let mut out = Vec::new();
            s.apply(
                &Event::EntityCreated {
                    id: id(1000 + n),
                    kind: EntityKind::NonPlayer,
                    name: format!("p{n}"),
                    class: None,
                    location: tessera_net::Location::new(tessera_net::RegionId(1), 0.0, 0.0, 0.0),
                    heading: 0.0,
                    appearance: Box::new(tessera_net::Appearance::new(tessera_net::Body::Object(
                        tessera_net::DataId(1),
                    ))),
                    tag: Some(n),
                },
                0.0,
                &mut out,
            );
        }
        // The selection is the placed entity with the largest ID (removed last by ID).
        let sel = id(1000 + n_placed - 1);
        combat(
            &mut s,
            CombatEvent::TargetChanged {
                entity: id(1),
                target: Some(sel),
            },
            0.0,
        );
        // A caller's vector that already holds more than the cap still gets this event's
        // changes.
        let mut out: Vec<CombatChange> = (0..MAX_CHANGES_PER_EVENT + 5)
            .map(|_| CombatChange::WorldEntered)
            .collect();
        let before = out.len();
        s.apply(&Event::StaticsCleared, 1.0, &mut out);
        let mine = &out[before..];
        assert_eq!(
            mine.first(),
            Some(&CombatChange::Selection {
                previous: Some(sel),
                current: None,
                local: true
            }),
            "the selection clear comes first"
        );
        let despawned = mine
            .iter()
            .filter(|c| matches!(c, CombatChange::Despawned(_)))
            .count();
        assert_eq!(despawned as u64, n_placed, "no removal is dropped");
        assert_eq!(s.selection(), None);
        assert_eq!(s.entities().count(), 1, "only the player is left");
        // Capped changes still count per call.
        let mut out: Vec<CombatChange> = (0..MAX_CHANGES_PER_EVENT)
            .map(|_| CombatChange::WorldEntered)
            .collect();
        spawn(&mut s, 7, EntityKind::Creature, (0.0, 5.0));
        s.apply(
            &Event::Combat(CombatEvent::CombatState {
                entity: id(7),
                in_combat: true,
            }),
            2.0,
            &mut out,
        );
        assert_eq!(out.last(), Some(&CombatChange::InCombat(id(7))));
    }

    #[test]
    fn the_players_own_position_is_kept_for_targeting() {
        let mut s = CombatState::new();
        entered(&mut s);
        s.set_player_position([0.0, 195.0, 0.0]);
        assert_eq!(s.player_entity().unwrap().position, Some([0.0, 195.0, 0.0]));
        s.set_player_position([f32::NAN, 0.0, 0.0]);
        assert_eq!(
            s.player_entity().unwrap().position,
            Some([0.0, 195.0, 0.0]),
            "non-finite positions are ignored"
        );
    }

    #[test]
    fn entity_and_effect_bounds_hold() {
        let mut s = CombatState::new();
        entered(&mut s);
        spawn(&mut s, 7, EntityKind::Creature, (0.0, 5.0));
        for n in 0..(MAX_EFFECTS_PER_ENTITY as u32 + 10) {
            combat(
                &mut s,
                CombatEvent::EffectApplied {
                    entity: id(7),
                    effect: EffectId(n),
                    what: Ref::Literal("e".into()),
                    duration_ms: None,
                    stacks: 1,
                    source: None,
                },
                0.0,
            );
        }
        assert_eq!(
            s.entity(id(7)).unwrap().effects.len(),
            MAX_EFFECTS_PER_ENTITY
        );
        for n in 0..10u32 {
            combat(
                &mut s,
                CombatEvent::CooldownStarted {
                    what: Ref::Literal(format!("c{n}")),
                    duration_ms: u32::MAX,
                },
                0.0,
            );
        }
        assert!(s.cooldowns().all(|(_, c)| c.duration <= MAX_SPAN));
    }
}
