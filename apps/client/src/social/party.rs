// SPDX-License-Identifier: GPL-3.0-or-later
//! The plugins' party views (`lua-plugin-api.md` §15.4, `social-panels.md` §10) fed from the
//! world session's fellowship: every `FellowshipChanged` list reaches the plugin host's
//! [`tessera_plugins::PluginManager::set_fellowship`] (which derives the `Party` events), and
//! the members' `Actor` values (level, morale, power) of every listed member come from the
//! social state ([`SocialState::member_values`]) through a shared table ([`PartyActors`]).

use std::collections::HashMap;
use std::sync::{Arc, Mutex, PoisonError};

use tessera_plugins::{
    FellowshipMember, MemberActorInfo, PartyActorSource, PartyPlayer, PluginManager,
};

use super::SocialState;

/// Most members whose values the table keeps (the feed's list is at most 64 long).
pub const MAX_ACTORS: usize = 256;

/// The members' `Actor` values by entity, shared with the plugin thread.
#[derive(Debug, Default)]
pub struct PartyActors {
    values: Mutex<HashMap<u64, MemberActorInfo>>,
}

impl PartyActorSource for PartyActors {
    fn member_actor(&self, entity: u64) -> Option<MemberActorInfo> {
        self.values
            .lock()
            .unwrap_or_else(PoisonError::into_inner)
            .get(&entity)
            .copied()
    }
}

/// Feeds the plugin host's party views from a social state (module docs).
#[derive(Debug, Default)]
pub struct PartyBridge {
    actors: Arc<PartyActors>,
    warned_appended: bool,
    seen: Option<u64>,
}

impl PartyBridge {
    /// A bridge whose actor source is not yet given to a plugin host.
    #[must_use]
    pub fn new() -> Self {
        Self::default()
    }

    /// The members' actor values as the plugin host's party actor source; the game UI
    /// attaches it with the plugins' other sources
    /// ([`crate::ui_host::UiHost::attach_plugins`]).
    #[must_use]
    pub fn actor_source(&self) -> Arc<dyn PartyActorSource> {
        Arc::clone(&self.actors) as _
    }

    /// Per frame: the member list handed over when it changed, the members' values
    /// refreshed when the state changed.
    pub fn update(&mut self, state: &mut SocialState, manager: &mut PluginManager) {
        if let Some(list) = state.take_roster_change() {
            let members: Vec<FellowshipMember> = list
                .iter()
                .map(|m| FellowshipMember {
                    name: m.name.clone(),
                    entity: m.entity.map(|e| e.0),
                    leader: m.leader,
                })
                .collect();
            let player = PartyPlayer {
                name: state.player().to_owned(),
                entity: state.player_entity().map(|e| e.0),
            };
            let update = manager.set_fellowship(&members, &player);
            if update.player_appended && !self.warned_appended {
                // `social-panels.md` §9 TR2: one warning per run.
                self.warned_appended = true;
                tracing::warn!("fellowship list without the player; the player was appended");
            }
        }
        if self.seen != Some(state.generation()) {
            self.refresh_values(state);
        }
    }

    /// The members' values from `state` into the shared table.
    fn refresh_values(&mut self, state: &SocialState) {
        self.seen = Some(state.generation());
        let mut values = self
            .actors
            .values
            .lock()
            .unwrap_or_else(PoisonError::into_inner);
        values.clear();
        // Every listed member but the player, not only the panel's five rows
        // (`lua-plugin-api.md` §15.4: the feed's list may hold 64 members).
        let player = state.player_entity();
        for m in state.roster().iter().take(MAX_ACTORS) {
            let Some(e) = m.entity else { continue };
            if Some(e) == player || m.name.to_lowercase() == state.player().to_lowercase() {
                continue;
            }
            let v = state.member_values(m);
            let (mo, mm) = v.morale.unwrap_or((0.0, 0.0));
            let (p, pm) = v.power.unwrap_or((0.0, 0.0));
            values.insert(
                e.0,
                MemberActorInfo {
                    level: v.level.and_then(|l| u32::try_from(l).ok()).unwrap_or(0),
                    morale: f64::from(mo),
                    max_morale: f64::from(mm),
                    base_max_morale: f64::from(mm),
                    power: f64::from(p),
                    max_power: f64::from(pm),
                    base_max_power: f64::from(pm),
                    ..MemberActorInfo::default()
                },
            );
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use tessera_net::domain::{FellowMember, SocialEvent};
    use tessera_net::{EntityId, Event};

    #[test]
    fn members_reach_the_plugin_host() {
        let dir = tempfile::tempdir().expect("tempdir");
        let mut plugins = crate::plugins::Plugins::start(
            &dir.path().join("Plugins"),
            dir.path().join("data"),
            &[],
        );
        let mut s = SocialState::new(true);
        let _ = s.apply(
            &Event::WorldEntered {
                player: EntityId(1),
                name: "Tam".into(),
                transform: tessera_net::Transform::new(tessera_net::Vec3::ZERO, 0.0),
            },
            &|_| None,
        );
        let _ = s.apply(
            &Event::Social(SocialEvent::FellowshipChanged {
                members: vec![
                    FellowMember {
                        name: "Tam".into(),
                        entity: Some(EntityId(1)),
                        leader: true,
                    },
                    FellowMember {
                        name: "Brin".into(),
                        entity: Some(EntityId(2)),
                        leader: false,
                    },
                ],
            }),
            &|_| None,
        );
        let _ = s.apply(
            &Event::Combat(tessera_net::domain::CombatEvent::VitalChanged {
                entity: EntityId(2),
                vital: tessera_net::domain::Vital::Health,
                current: 40.0,
                max: 80.0,
            }),
            &|_| None,
        );
        let mut b = PartyBridge::new();
        b.update(&mut s, plugins.manager_mut());
        let v = b.actors.member_actor(2).expect("Brin's values");
        assert!((v.morale - 40.0).abs() < 1e-9 && (v.max_morale - 80.0).abs() < 1e-9);
        assert!(b.actors.member_actor(1).is_none(), "the player has no row");
        // The list was handed over once.
        assert!(s.take_roster_change().is_none());
    }

    /// Regression (P4 review): members past the panel's five rows have their values too.
    #[test]
    fn members_past_the_rows_have_values() {
        let mut s = SocialState::new(true);
        let _ = s.apply(
            &Event::WorldEntered {
                player: EntityId(1),
                name: "Tam".into(),
                transform: tessera_net::Transform::new(tessera_net::Vec3::ZERO, 0.0),
            },
            &|_| None,
        );
        let mut members = vec![FellowMember {
            name: "Tam".into(),
            entity: Some(EntityId(1)),
            leader: true,
        }];
        members.extend((2..=8u64).map(|i| FellowMember {
            name: format!("M{i}"),
            entity: Some(EntityId(i)),
            leader: false,
        }));
        let _ = s.apply(
            &Event::Social(SocialEvent::FellowshipChanged { members }),
            &|_| None,
        );
        assert_eq!(s.fellowship().rows().len(), 5, "the panel shows five rows");
        let _ = s.apply(
            &Event::Combat(tessera_net::domain::CombatEvent::VitalChanged {
                entity: EntityId(8),
                vital: tessera_net::domain::Vital::Health,
                current: 10.0,
                max: 20.0,
            }),
            &|_| None,
        );
        let _ = s.apply(
            &Event::Character(tessera_net::domain::CharacterEvent::LevelChanged {
                entity: EntityId(8),
                level: 33,
            }),
            &|_| None,
        );
        // A non-member's vital (a creature in combat) is not kept.
        let _ = s.apply(
            &Event::Combat(tessera_net::domain::CombatEvent::VitalChanged {
                entity: EntityId(99),
                vital: tessera_net::domain::Vital::Health,
                current: 1.0,
                max: 2.0,
            }),
            &|_| None,
        );
        let mut b = PartyBridge::default();
        // Only the values step (no plugin host): the same code path as `update`.
        b.refresh_values(&s);
        let v = b
            .actors
            .member_actor(8)
            .expect("the seventh member's values");
        assert!((v.morale - 10.0).abs() < 1e-9 && (v.max_morale - 20.0).abs() < 1e-9);
        assert_eq!(v.level, 33);
        assert!(b.actors.member_actor(99).is_none());
        assert_eq!(
            (2..=8)
                .filter(|e| b.actors.member_actor(*e).is_some())
                .count(),
            7
        );
    }
}
