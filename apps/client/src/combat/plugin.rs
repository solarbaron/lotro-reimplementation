// SPDX-License-Identifier: GPL-3.0-or-later
//! The plugin combat views' source (`lua-plugin-api.md` §15.2): `tessera-plugins`'
//! [`CombatModel`] fed from the session's events, with the effect and skill values the
//! client precomputes from the install's records (or the literal rules without one).
//!
//! Each frame the caller hands the events it applied ([`PluginCombat::observe`]) with the
//! frame's game time (`Turbine.Engine.GetGameTime`, §12); the derived view events are
//! raised on the plugin manager for step 7, after the model's lock is released (the host
//! may read the source while it queues them).

use std::sync::{Arc, Mutex, PoisonError};

use tessera_formats::Did;
use tessera_net::domain::{Ref, Vital};
use tessera_net::{CharacterEvent, CombatEvent, EntityKind, Event, ProgressionEvent};
use tessera_plugins::{
    ActorKind, CombatModel, CombatSource, CombatViewEvent, CombatVital, EffectData, PluginManager,
    SkillData, SkillKey,
};

use super::view::CombatInstall;

/// Most `CharacterInfo` levels remembered for the entered character's level.
const MAX_LEVELS: usize = 1024;
/// Most placed entities remembered for `StaticsCleared` (the combat state's entity bound).
const MAX_PLACED: usize = super::state::MAX_ENTITIES;

/// The plugin combat views' client side (module docs).
#[derive(Debug, Default)]
pub struct PluginCombat {
    model: Arc<Mutex<CombatModel>>,
    levels: std::collections::HashMap<String, u32>,
    /// The entities placed by the world data (an `EntityCreated` with a tag), which
    /// `StaticsCleared` removes (decision 0032 D2, as the combat state does).
    placed: std::collections::BTreeSet<u64>,
}

fn actor_kind(k: EntityKind) -> ActorKind {
    match k {
        EntityKind::Player => ActorKind::Player,
        EntityKind::NonPlayer => ActorKind::NonPlayer,
        EntityKind::Creature => ActorKind::Creature,
        _ => ActorKind::Object,
    }
}

/// The skill key of a ref.
#[must_use]
pub fn skill_key(r: &Ref) -> SkillKey {
    match r {
        Ref::Record(d) => SkillKey::Record(d.0),
        Ref::Literal(s) => SkillKey::Literal(s.clone()),
    }
}

/// §15.2 "Effect values" of an effect instance applied to `entity`.
#[must_use]
pub fn effect_data(
    entity: tessera_net::EntityId,
    id: u32,
    what: &Ref,
    duration_ms: Option<u32>,
    source: Option<tessera_net::EntityId>,
    start_time: f64,
    install: Option<&CombatInstall>,
) -> EffectData {
    let duration = duration_ms.map_or(0.0, |ms| f64::from(ms) / 1000.0);
    match what {
        Ref::Literal(s) => EffectData {
            id,
            name: s.clone(),
            description: String::new(),
            icon: 0,
            category: 0,
            debuff: source.is_some_and(|s| s != entity),
            curable: false,
            start_time,
            duration,
        },
        Ref::Record(d) => {
            let did = Did(d.0);
            let i = install;
            EffectData {
                id,
                name: i
                    .and_then(|i| i.text_property(did, "Effect_Name"))
                    .unwrap_or_default(),
                description: i
                    .and_then(|i| i.text_property(did, "Effect_Applied_Description"))
                    .unwrap_or_default(),
                icon: i
                    .and_then(|i| i.did_property(did, "Effect_Icon"))
                    .map_or(0, |d| d.0),
                #[allow(clippy::cast_possible_truncation, clippy::cast_sign_loss)]
                category: i
                    .and_then(|i| i.number_property(did, "Effect_Resist_Category"))
                    .map_or(0, |v| v as u32),
                debuff: i
                    .and_then(|i| i.bool_property(did, "Effect_Debuff"))
                    .unwrap_or(false),
                curable: i
                    .and_then(|i| i.bool_property(did, "Effect_IsCurable"))
                    .unwrap_or(false),
                start_time,
                duration,
            }
        }
    }
}

/// §15.2 "Skills" values of a learned skill.
#[must_use]
pub fn skill_data(what: &Ref, install: Option<&CombatInstall>) -> SkillData {
    match what {
        Ref::Literal(s) => SkillData {
            key: skill_key(what),
            name: s.clone(),
            description: String::new(),
            icon_image_id: 0,
            cooldown: 0.0,
        },
        Ref::Record(d) => {
            let did = Did(d.0);
            let i = install;
            SkillData {
                key: skill_key(what),
                name: i
                    .and_then(|i| i.text_property(did, "Skill_Name"))
                    .unwrap_or_default(),
                description: i
                    .and_then(|i| i.text_property(did, "Skill_Desc"))
                    .unwrap_or_default(),
                icon_image_id: i
                    .and_then(|i| {
                        i.did_property(did, "Skill_SmallIcon")
                            .or_else(|| i.did_property(did, "Skill_Icon"))
                    })
                    .map_or(0, |d| d.0),
                cooldown: i
                    .and_then(|i| i.number_property(did, "Skill_SkillRecoveryTime"))
                    .filter(|v| v.is_finite() && *v >= 0.0)
                    .unwrap_or(0.0),
            }
        }
    }
}

impl PluginCombat {
    /// An empty source.
    #[must_use]
    pub fn new() -> Self {
        Self::default()
    }

    /// The source for [`PluginManager::set_combat_source`].
    #[must_use]
    pub fn source(&self) -> Arc<dyn CombatSource> {
        Arc::clone(&self.model) as Arc<dyn CombatSource>
    }

    /// Hands the plugin manager the combat views' source.
    pub fn attach(&self, manager: &mut PluginManager) {
        manager.set_combat_source(Some(self.source()));
    }

    /// One frame's events, received at game time `game_time`: the model's inputs, then the
    /// derived view events raised on `manager` for step 7.
    pub fn observe(
        &mut self,
        events: &[Event],
        game_time: f64,
        install: Option<&CombatInstall>,
        manager: &mut PluginManager,
    ) {
        let mut raised = Vec::new();
        {
            let model = Arc::clone(&self.model);
            let mut m = model.lock().unwrap_or_else(PoisonError::into_inner);
            for ev in events {
                raised.extend(self.input(&mut m, ev, game_time, install));
            }
        }
        if !raised.is_empty() {
            manager.raise_combat_events(raised);
        }
    }

    fn input(
        &mut self,
        m: &mut CombatModel,
        ev: &Event,
        t: f64,
        install: Option<&CombatInstall>,
    ) -> Vec<CombatViewEvent> {
        match ev {
            Event::WorldEntered { player, name, .. } => {
                self.placed.clear();
                let level = self.levels.get(&crate::net::name_key(name)).copied();
                m.world_entered(player.0, name, level)
            }
            Event::Disconnected { .. } => {
                self.placed.clear();
                m.left_world()
            }
            Event::EntitySpawned { id, kind, name, .. } => {
                self.placed.remove(&id.0);
                m.entity_spawned(id.0, actor_kind(*kind), name);
                Vec::new()
            }
            Event::EntityCreated {
                id,
                kind,
                name,
                tag,
                ..
            } => {
                if tag.is_some() {
                    if self.placed.len() < MAX_PLACED {
                        self.placed.insert(id.0);
                    }
                } else {
                    self.placed.remove(&id.0);
                }
                m.entity_spawned(id.0, actor_kind(*kind), name);
                Vec::new()
            }
            Event::EntityDespawned { id } => {
                self.placed.remove(&id.0);
                m.entity_despawned(id.0)
            }
            Event::StaticsCleared => std::mem::take(&mut self.placed)
                .into_iter()
                .flat_map(|id| m.entity_despawned(id))
                .collect(),
            Event::Character(CharacterEvent::CharacterInfo {
                name,
                level: Some(level),
                ..
            }) => {
                let key = crate::net::name_key(name);
                if self.levels.contains_key(&key) || self.levels.len() < MAX_LEVELS {
                    self.levels.insert(key, *level);
                }
                Vec::new()
            }
            Event::Character(CharacterEvent::LevelChanged { entity, level, .. }) => {
                m.level_changed(entity.0, *level)
            }
            Event::Progression(ProgressionEvent::SkillLearned { what }) => {
                m.skill_learned(skill_data(what, install))
            }
            Event::Progression(ProgressionEvent::SkillForgotten { what }) => {
                m.skill_forgotten(&skill_key(what))
            }
            Event::Combat(c) => match c {
                CombatEvent::TargetChanged { entity, target } => {
                    m.target_changed(entity.0, target.map(|t| t.0))
                }
                CombatEvent::VitalChanged {
                    entity,
                    vital,
                    current,
                    max,
                } => match vital {
                    Vital::Health => m.vital_changed(entity.0, CombatVital::Health, *current, *max),
                    Vital::Power => m.vital_changed(entity.0, CombatVital::Power, *current, *max),
                    Vital::Resource(_) => Vec::new(),
                },
                CombatEvent::CombatState { entity, in_combat } => {
                    m.combat_state(entity.0, *in_combat)
                }
                CombatEvent::EffectApplied {
                    entity,
                    effect,
                    what,
                    duration_ms,
                    source,
                    ..
                } => m.effect_applied(
                    entity.0,
                    effect_data(*entity, effect.0, what, *duration_ms, *source, t, install),
                ),
                CombatEvent::EffectRemoved { entity, effect } => {
                    m.effect_removed(entity.0, effect.0)
                }
                CombatEvent::CooldownStarted { what, duration_ms } => {
                    m.cooldown_started(skill_key(what), t + f64::from(*duration_ms) / 1000.0)
                }
                CombatEvent::CooldownEnded { what } => m.cooldown_ended(&skill_key(what)),
                _ => Vec::new(),
            },
            _ => Vec::new(),
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use tessera_net::{EntityId, Transform, Vec3};
    use tessera_plugins::CombatSource;

    #[test]
    fn statics_cleared_removes_placed_entities_from_the_plugin_views() {
        let mut pc = PluginCombat::new();
        let model = Arc::clone(&pc.model);
        let mut input = |ev: Event| {
            let mut m = model.lock().unwrap_or_else(PoisonError::into_inner);
            pc.input(&mut m, &ev, 0.0, None)
        };
        input(Event::WorldEntered {
            player: EntityId(1),
            name: "P".into(),
            transform: Transform::new(Vec3::ZERO, 0.0),
        });
        let created = |id: u64, tag: Option<u64>| Event::EntityCreated {
            id: EntityId(id),
            kind: EntityKind::NonPlayer,
            name: format!("n{id}"),
            class: None,
            location: tessera_net::Location::new(tessera_net::RegionId(1), 0.0, 0.0, 0.0),
            heading: 0.0,
            appearance: Box::new(tessera_net::Appearance::new(tessera_net::Body::Object(
                tessera_net::DataId(1),
            ))),
            tag,
        };
        input(created(7, Some(70)));
        input(created(8, None));
        input(Event::Combat(CombatEvent::TargetChanged {
            entity: EntityId(1),
            target: Some(EntityId(7)),
        }));
        let source: &Mutex<CombatModel> = &model;
        assert_eq!(source.actor(1).and_then(|a| a.target), Some(7));
        let raised = input(Event::StaticsCleared);
        assert_eq!(
            source.actor(1).and_then(|a| a.target),
            None,
            "GetTarget() no longer names the removed NPC"
        );
        assert!(!raised.is_empty(), "TargetChanged is raised");
        assert!(source.actor(7).is_none(), "the placed NPC is gone");
        assert!(source.actor(8).is_some(), "a simulated entity stays");
    }
}
