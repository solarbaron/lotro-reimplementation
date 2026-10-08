// SPDX-License-Identifier: GPL-3.0-or-later
//! `tessera_wstate::game_controls` (`docs/specs/game-controls.md`): the typed control
//! views on arbitrary property sets, the compiled-only readers and the kind rule on
//! arbitrary object states. Nothing may panic.
//!
//! The first input byte selects what the rest is:
//!
//! - even: an embedded property set, decoded with a synthetic dictionary that defines
//!   every property the views read (`testkit::GAME_CONTROL_PROPERTIES`); every view runs
//!   on it, followed by the gait, consideration, unlock and reset-day rules on the
//!   decoded values;
//! - odd: an object-state payload, decoded without a library; the DID-set, slot-schedule,
//!   advancement-table and modifier-table readers and the empty-marker test run on it,
//!   and the kind rule classifies its root class against a small synthetic library.

#![no_main]

use std::sync::OnceLock;

use libfuzzer_sys::fuzz_target;
use tessera_formats::Did;
use tessera_props::{PropertyRegistry, PropertySet, Value};
use tessera_wstate::game_controls::{
    AdvancementTable, AggroPingControl, AlertsControl, ClassIndices, Collection, CollectionControl,
    CombatMusicControl, ConsiderationControl, ConsiderationTable, DidSet, DiscoveryDistanceControl,
    DuelTimings, GuildControl, LoginReward, ModifierTable, MountDirectory, MountTrait,
    MountedCombatControl, OpenTappingControl, PaperItemService, PaperItemServices, Perk,
    PerkControl, RandomItemRewardControl, ResetScheduleControl, SlotSchedule, TemplateDiff,
    TrackingStateControl, VitalDisplayOverride, VitalDisplayOverrideControl, bit_keys, classify,
    is_default_attack, is_empty_marker, unlocked_key,
};
use tessera_wstate::testkit::{LibraryBuilder, game_controls_dictionary};
use tessera_wstate::{ClassLibrary, DecodeContext, ObjectState};

fn registry() -> &'static PropertyRegistry {
    static R: OnceLock<PropertyRegistry> = OnceLock::new();
    R.get_or_init(|| PropertyRegistry::parse(&game_controls_dictionary()).expect("dictionary"))
}

/// Classes 1–17 with the roles of the kind rule at small indices.
fn library() -> &'static (ClassLibrary, ClassIndices) {
    static L: OnceLock<(ClassLibrary, ClassIndices)> = OnceLock::new();
    L.get_or_init(|| {
        let mut b = LibraryBuilder::new(20);
        for c in 1..=17 {
            b = b.define(c, &[]);
        }
        let parents = [
            (2, 1),
            (3, 2),
            (4, 3),
            (5, 3),
            (6, 4),
            (7, 4),
            (8, 4),
            (9, 4),
            (10, 5),
            (11, 7),
            (12, 4),
            (13, 9),
            (14, 10),
            (15, 5),
            (16, 12),
            (17, 15),
        ];
        for (c, p) in parents {
            b = b.parent(c, p);
        }
        let lib = ClassLibrary::parse(&b.build()).expect("library");
        let idx = ClassIndices {
            common_ancestors: [1, 2, 3],
            plain_object_base: 4,
            entity_base: 5,
            did_set: 6,
            dialog_base: 7,
            skill: 8,
            effect_base: 9,
            manager_base: 10,
        };
        (lib, idx)
    })
}

fn views(set: &PropertySet, reg: &PropertyRegistry) {
    if let Ok(d) = MountDirectory::from_properties(set, reg) {
        let _ = d.is_disjoint();
        let _ = d.skills().count();
    }
    if let Ok(t) = MountTrait::from_properties(set, reg) {
        let _ = t.slot();
    }
    if let Ok(c) = MountedCombatControl::from_properties(set, reg) {
        let v = Value::Float(0.25);
        for gait in 0..9 {
            let _ = c.speed_multiplier(gait);
            let _ = c.turn_rate(gait, |_| Some(&v));
            let _ = c.forward_speed_limit(Some(gait), 10.0, Some(5.0));
        }
        let _ = c.forward_speed_limit(None, f32::MAX, None);
        let _ = MountedCombatControl::turn_gait(set.iter().next().map(|p| &p.value));
    }
    let _ = is_default_attack(set, reg);
    let d = DuelTimings::from_properties(set, reg);
    let _ = (d.begin_delay_or_default(), d.length_or_default());
    if let Ok(c) = PerkControl::from_properties(set, reg) {
        let _ = c.keys_are_distinct();
        if let Ok(p) = Perk::from_properties(set, reg) {
            let _ = c.admits(&p);
        }
    }
    let _ = CollectionControl::from_properties(set, reg);
    let _ = Collection::from_properties(set, reg);
    let _ = PaperItemServices::from_properties(set, reg);
    let _ = PaperItemService::from_properties(set, reg);
    if let (Ok(r), Ok(l)) = (
        RandomItemRewardControl::from_properties(set, reg),
        LoginReward::from_properties(set, reg),
    ) {
        let _ = l.control_category.and_then(|c| r.tiers(c));
    }
    let _ = VitalDisplayOverrideControl::from_properties(set, reg);
    let _ = VitalDisplayOverride::from_properties(set, reg);
    if let Ok(a) = AlertsControl::from_properties(set, reg) {
        let _ = a.types_are_distinct_and_non_zero();
    }
    if let Ok(r) = ResetScheduleControl::from_properties(set, reg) {
        for s in &r.schedules {
            let _ = s.day_keys();
        }
    }
    if let (Ok(c), Ok(t)) = (
        ConsiderationControl::from_properties(set, reg),
        ConsiderationTable::from_properties(set, reg),
    ) {
        for (_, list) in &c.combat_tables {
            for level in [0, 1, 60, 120, 121, u32::MAX] {
                let _ = ConsiderationControl::table_for_level(list, level);
            }
        }
        for d in [i64::MIN, -16, 0, 16, i64::MAX] {
            if let Some(r) = t.rank_for_difference(d) {
                let _ = c.ui_state(r);
            }
        }
        let _ = t.is_well_formed(50);
    }
    if let Ok(t) = TrackingStateControl::from_properties(set, reg) {
        let _ = t.states_are_distinct();
    }
    let _ = DiscoveryDistanceControl::from_properties(set, reg);
    let _ = CombatMusicControl::from_properties(set, reg);
    let _ = AggroPingControl::from_properties(set, reg);
    let _ = OpenTappingControl::from_properties(set, reg);
    if let Ok(g) = GuildControl::from_properties(set, reg) {
        let _ = g.player_note_limit();
    }
    set.walk(|_, p| {
        let _ = unlocked_key(&p.value);
        if let Some(b) = p.value.as_bits() {
            let _ = bit_keys(b);
        }
    });
    let _ = TemplateDiff::between(set, &PropertySet::new());
}

fn state_mode(data: &[u8]) {
    let Ok(state) = ObjectState::parse_unchecked(data, DecodeContext::bare()) else {
        return;
    };
    if let Ok(s) = DidSet::from_state(&state) {
        let _ = s.contains(Did(0x7000_0001));
    }
    if let Ok(s) = SlotSchedule::from_state(&state) {
        let _ = s.lengths();
    }
    if let Ok(t) = AdvancementTable::from_state(&state) {
        let _ = t.is_cumulative();
    }
    if let Ok(t) = ModifierTable::from_state(&state) {
        let _ = t.all_finite();
        let _ = t.get(1);
    }
    let _ = is_empty_marker(data.len(), &state, 6, &PropertySet::new());
    let (lib, idx) = library();
    let empty = data.len().is_multiple_of(2);
    let _ = classify(state.did, Some(state.root_class), lib, idx, || Ok(empty));
    let _ = classify(state.did, Some(state.root_class % 20), lib, idx, || {
        Ok(empty)
    });
}

fuzz_target!(|data: &[u8]| {
    let Some((&mode, rest)) = data.split_first() else {
        return;
    };
    if mode & 1 == 0 {
        let reg = registry();
        if let Ok(set) = PropertySet::parse_embedded(rest, Did(0x7900_0001), reg) {
            views(&set, reg);
        }
    } else {
        state_mode(rest);
    }
});
