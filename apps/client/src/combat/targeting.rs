// SPDX-License-Identifier: GPL-3.0-or-later
//! The targeting rules of `combat-ui.md` §3.3 (TR4, Tessera's own: the original's search
//! rules are OPEN, owner check OC1): which `SelectTarget` a targeting action sends, from
//! the kept combat state. Pure functions; the client sends the result once per press
//! (Impulse).
//!
//! Candidates are the eligible entities (§3.1, TR1) other than the player whose 3D distance
//! to the player is at most [`TARGET_RANGE`]. TR1's body-record test of objects needs the
//! walker's records; here every object is eligible (the scenario's and test server's
//! objects carry no body record).

use tessera_input::TargetAction;
use tessera_net::{EntityId, EntityKind};

use super::state::CombatState;

/// TR2: the targeting keys search within 50 m (Tessera's choice).
pub const TARGET_RANGE: f32 = 50.0;

/// What a targeting action asks for.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum TargetPick {
    /// Send nothing (an empty result, TR4 item 5).
    Nothing,
    /// Send `SelectTarget { target }` (`None` for *clear*).
    Select(Option<EntityId>),
}

/// The set a cycling action searches.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum Set {
    Foe,
    Player,
    Creature,
    Object,
}

fn in_set(set: Set, kind: EntityKind, defeated: bool) -> bool {
    match set {
        Set::Foe => kind == EntityKind::Creature && !defeated,
        Set::Player => kind == EntityKind::Player,
        Set::Creature => kind == EntityKind::Creature,
        Set::Object => kind == EntityKind::Object,
    }
}

fn distance(a: [f32; 3], b: [f32; 3]) -> f32 {
    let d = [a[0] - b[0], a[1] - b[1], a[2] - b[2]];
    (d[0] * d[0] + d[1] * d[1] + d[2] * d[2]).sqrt()
}

/// The candidates of `set`, ordered by distance (ties by the smaller entity ID).
fn ordered(state: &CombatState, set: Set) -> Vec<EntityId> {
    let Some(me) = state.player() else {
        return Vec::new();
    };
    let Some(origin) = state.entity(me).and_then(|e| e.position) else {
        return Vec::new();
    };
    let mut v: Vec<(f32, EntityId)> = state
        .entities()
        .filter(|(id, _)| *id != me)
        .filter(|(_, e)| in_set(set, e.kind, e.defeated))
        .filter_map(|(id, e)| {
            let d = distance(origin, e.position?);
            (d.is_finite() && d <= TARGET_RANGE).then_some((d, id))
        })
        .collect();
    v.sort_by(|a, b| a.0.total_cmp(&b.0).then(a.1.0.cmp(&b.1.0)));
    v.into_iter().map(|(_, id)| id).collect()
}

fn nearest(state: &CombatState, set: Set) -> TargetPick {
    ordered(state, set)
        .first()
        .map_or(TargetPick::Nothing, |e| TargetPick::Select(Some(*e)))
}

fn cycle(state: &CombatState, set: Set, forward: bool) -> TargetPick {
    let v = ordered(state, set);
    if v.is_empty() {
        return TargetPick::Nothing;
    }
    let pos = state
        .selection()
        .and_then(|s| v.iter().position(|e| *e == s));
    let i = match (pos, forward) {
        (Some(p), true) => (p + 1) % v.len(),
        (Some(p), false) => (p + v.len() - 1) % v.len(),
        (None, true) => 0,
        (None, false) => v.len() - 1,
    };
    TargetPick::Select(Some(v[i]))
}

/// The `SelectTarget` of a targeting action (TR4).
#[must_use]
pub fn pick(action: TargetAction, state: &CombatState) -> TargetPick {
    use TargetAction as A;
    match action {
        A::NearestEnemy => nearest(state, Set::Foe),
        A::NextEnemy => cycle(state, Set::Foe, true),
        A::PreviousEnemy => cycle(state, Set::Foe, false),
        A::Myself => state
            .player()
            .map_or(TargetPick::Nothing, |p| TargetPick::Select(Some(p))),
        A::Assist => state
            .selection()
            .and_then(|s| state.entity(s))
            .and_then(|e| e.target)
            .map_or(TargetPick::Nothing, |t| TargetPick::Select(Some(t))),
        A::NearestPlayer => nearest(state, Set::Player),
        A::NextPlayer => cycle(state, Set::Player, true),
        A::PreviousPlayer => cycle(state, Set::Player, false),
        A::NearestCreature => nearest(state, Set::Creature),
        A::NextCreature => cycle(state, Set::Creature, true),
        A::PreviousCreature => cycle(state, Set::Creature, false),
        A::NearestObject => nearest(state, Set::Object),
        A::Last => state
            .previous_selection()
            .map_or(TargetPick::Nothing, |e| TargetPick::Select(Some(e))),
        A::LastAttacker => state
            .last_attacker()
            .map_or(TargetPick::Nothing, |e| TargetPick::Select(Some(e))),
        A::Clear => TargetPick::Select(None),
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::combat::state::CombatChange;
    use tessera_net::{CombatEvent, Event, Transform, Vec3};

    fn state() -> CombatState {
        let mut s = CombatState::new();
        let mut out: Vec<CombatChange> = Vec::new();
        s.apply(
            &Event::WorldEntered {
                player: EntityId(1),
                name: "P".into(),
                transform: Transform::new(Vec3::ZERO, 0.0),
            },
            0.0,
            &mut out,
        );
        // §17.5: creatures at 5 m (ID 7), 12 m (ID 4), 12 m (ID 9) and 60 m (ID 2).
        for (id, d) in [(7, 5.0), (4, 12.0), (9, 12.0), (2, 60.0)] {
            s.apply(
                &Event::EntitySpawned {
                    id: EntityId(id),
                    kind: EntityKind::Creature,
                    name: String::new(),
                    transform: Transform::new(Vec3::new(0.0, d, 0.0), 0.0),
                },
                0.0,
                &mut out,
            );
        }
        s
    }

    fn select(s: &mut CombatState, t: TargetPick) {
        let TargetPick::Select(target) = t else {
            panic!("nothing picked");
        };
        let mut out = Vec::new();
        s.apply(
            &Event::Combat(CombatEvent::TargetChanged {
                entity: EntityId(1),
                target,
            }),
            0.0,
            &mut out,
        );
    }

    #[test]
    fn section_17_5_cycle() {
        let mut s = state();
        let next = |s: &CombatState| pick(TargetAction::NextEnemy, s);
        assert_eq!(next(&s), TargetPick::Select(Some(EntityId(7))));
        let t = next(&s);
        select(&mut s, t);
        assert_eq!(next(&s), TargetPick::Select(Some(EntityId(4))));
        let t = next(&s);
        select(&mut s, t);
        assert_eq!(next(&s), TargetPick::Select(Some(EntityId(9))));
        let t = next(&s);
        select(&mut s, t);
        assert_eq!(next(&s), TargetPick::Select(Some(EntityId(7))));
        let t = next(&s);
        select(&mut s, t);
        assert_eq!(
            pick(TargetAction::PreviousEnemy, &s),
            TargetPick::Select(Some(EntityId(9)))
        );
        assert_eq!(
            pick(TargetAction::NearestEnemy, &s),
            TargetPick::Select(Some(EntityId(7)))
        );
    }

    #[test]
    fn range_is_measured_from_where_the_player_walked() {
        let mut s = state();
        // The player walks 58 m north (positions the session never echoes): the creature
        // at 60 m is now 2 m away, the one at 5 m is 53 m away (out of range).
        s.set_player_position([0.0, 58.0, 0.0]);
        assert_eq!(
            pick(TargetAction::NearestEnemy, &s),
            TargetPick::Select(Some(EntityId(2)))
        );
        s.set_player_position([0.0, 200.0, 0.0]);
        assert_eq!(pick(TargetAction::NearestEnemy, &s), TargetPick::Nothing);
    }

    #[test]
    fn other_actions() {
        let mut s = state();
        assert_eq!(pick(TargetAction::Assist, &s), TargetPick::Nothing);
        assert_eq!(pick(TargetAction::NearestPlayer, &s), TargetPick::Nothing);
        assert_eq!(pick(TargetAction::Last, &s), TargetPick::Nothing);
        assert_eq!(pick(TargetAction::Clear, &s), TargetPick::Select(None));
        assert_eq!(
            pick(TargetAction::Myself, &s),
            TargetPick::Select(Some(EntityId(1)))
        );
        select(&mut s, TargetPick::Select(Some(EntityId(4))));
        select(&mut s, TargetPick::Select(Some(EntityId(9))));
        assert_eq!(
            pick(TargetAction::Last, &s),
            TargetPick::Select(Some(EntityId(4)))
        );
        let mut out = Vec::new();
        s.apply(
            &Event::Combat(CombatEvent::TargetChanged {
                entity: EntityId(9),
                target: Some(EntityId(1)),
            }),
            0.0,
            &mut out,
        );
        assert_eq!(
            pick(TargetAction::Assist, &s),
            TargetPick::Select(Some(EntityId(1)))
        );
        // A defeated creature is no foe but still a creature.
        s.apply(
            &Event::Combat(CombatEvent::Defeated {
                entity: EntityId(7),
                by: None,
            }),
            0.0,
            &mut out,
        );
        assert_eq!(
            pick(TargetAction::NearestEnemy, &s),
            TargetPick::Select(Some(EntityId(4)))
        );
        assert_eq!(
            pick(TargetAction::NearestCreature, &s),
            TargetPick::Select(Some(EntityId(7)))
        );
    }
}
