// SPDX-License-Identifier: GPL-3.0-or-later
//! World-logic views and rules (`tessera_formats::gameplay::world_logic`,
//! `docs/specs/world-logic.md`) and the phasing manager (`tessera_world::phasing`, §4.5) on
//! arbitrary bytes.
//!
//! The input after byte 0 is an embedded property set decoded with a total synthetic schema
//! (type = PID mod 24) whose labels carry the types of §3 (lists arrays, check and condition
//! entries structs, quests and events DIDs, operators and statuses enums, flags booleans).
//! Every view reads it; the box test, the script-path branch for every graph shape and the
//! world-event conditions run against a journal and an event state derived from byte 0;
//! then the set drives the phasing manager as a box with fuzzed targets through the events
//! E1–E5.
//!
//! Nothing may panic. Before E4 every box tests false and every target is phased out
//! (WL-B1); a box without a check list tests false (WL-B2); a `QuestState_Test` of 2 gives
//! the False branch for every switched graph (WL-B8).
//!
//! Run with a memory cap, e.g.
//! `cargo fuzz run formats_world_logic -- -malloc_limit_mb=512 -rss_limit_mb=1024`.

#![no_main]

use std::collections::HashMap;

use libfuzzer_sys::fuzz_target;
use tessera_formats::gameplay::quests::{EntryGates, MapJournal};
use tessera_formats::gameplay::world_logic::{
    BranchVisuals, CounterBox, CustomCondition, EventLists, GeneratorMapping, LocalPlayer,
    LogicBoxMembers, LogicBoxTimer, PathMapVolume, PhasingBox, QuestStateBranch, QuestStateGraph,
    QuestStateScript, ResponseCallback, ResponseMap, SkirmishValues, TravelHooks,
    VersionMasterList, WorldEventKind, WorldEventState, WorldLogicLabels, branch_draw,
};
use tessera_formats::landblock::InstanceId;
use tessera_formats::{Did, PropertySet, PropertyType};
use tessera_world::phasing::{BoxSpec, CreatedEntity, PhasingChange, PhasingManager};

fn total(pid: u32) -> Option<PropertyType> {
    Some(PropertyType::from_code(pid % 24))
}

fn spec_type(label: &str) -> u32 {
    match label {
        l if l.ends_with("_List")
            || l.ends_with("Array")
            || l.ends_with("EventList")
            || l.ends_with("ConditionList")
            || l.ends_with("SingletonList")
            || l.ends_with("AlwaysRunOnPlayers") =>
        {
            12
        }
        l if l.ends_with("_Struct")
            || l.ends_with("Mapping")
            || l.ends_with("CustomCondition")
            || l.ends_with("WorldEvent_Condition")
            || l.ends_with("ConditionStruct")
            || l.ends_with("_Entry") =>
        {
            11
        }
        l if l.contains("QuestID")
            || l.ends_with("WorldEvent")
            || l.ends_with("Callback")
            || l.ends_with("Particle")
            || l.ends_with("Material")
            || l.ends_with("_Item")
            || l.ends_with("Animation")
            || l.ends_with("AprFile")
            || l.ends_with("SceneID")
            || l.ends_with("ResponseMap")
            || l.ends_with("Boolean")
            || l.ends_with("Integer")
            || l.ends_with("CompareWith") =>
        {
            20
        }
        l if l.ends_with("Status")
            || l.contains("Operator")
            || l.ends_with("_Test")
            || l.ends_with("Location")
            || l.ends_with("_Key")
            || l.ends_with("ResponseType")
            || l.ends_with("EntityType")
            || l.ends_with("Version_Number") =>
        {
            8
        }
        l if l.contains("m_e") || l.ends_with("AIHint") || l.ends_with("CanSeeVisibilityMask") => {
            18
        }
        l if l.ends_with("Min") || l.ends_with("Max") || l.ends_with("LowPoint") => 6,
        l if l.ends_with("Timer")
            || l.ends_with("Delay")
            || l.ends_with("Apr")
            || l.ends_with("Height")
            || l.ends_with("Radius")
            || l.ends_with("Modifier") =>
        {
            9
        }
        l if l.contains("Count")
            || l.contains("By")
            || l.contains("Condition") && l.starts_with("CounterBox")
            || l.ends_with("Link")
            || l.ends_with("Probability")
            || l.ends_with("Number")
            || l.ends_with("Value")
            || l.ends_with("ConditionValue")
            || l.ends_with("StateToTestValue") =>
        {
            15
        }
        l if l.ends_with("Size") => 3,
        l if l.ends_with("Drama") || l.ends_with("UsageString") => 1,
        _ => 21,
    }
}

struct Events(u8);

impl WorldEventState for Events {
    fn kind(&self, event: Did) -> Option<WorldEventKind> {
        match event.0 % 3 {
            0 => None,
            1 => Some(WorldEventKind::Boolean),
            _ => Some(WorldEventKind::Integer {
                min: (self.0 & 1 != 0).then_some(-5),
                max: (self.0 & 2 != 0).then_some(i64::from(self.0)),
            }),
        }
    }
    fn value(&self, event: Did) -> Option<i64> {
        (!event.0.is_multiple_of(5)).then_some(i64::from(event.0 % 7))
    }
}

fuzz_target!(|input: &[u8]| {
    let [mode, rest @ ..] = input else {
        return;
    };
    let Ok(set) = PropertySet::parse(rest, &total) else {
        return;
    };
    let map: HashMap<String, u32> = WorldLogicLabels::labels()
        .enumerate()
        .map(|(i, l)| {
            let i = u32::try_from(i).unwrap_or(0);
            (l.to_owned(), (i + 1) * 24 + spec_type(l))
        })
        .collect();
    let labels = WorldLogicLabels::resolve(&map);
    let q = &labels.quest;
    let journal = MapJournal {
        completed: [Did(u32::from(*mode))].into_iter().collect(),
        ..MapJournal::default()
    };
    let gates = EntryGates {
        weenie_type: u32::from(*mode),
        ..EntryGates::default()
    };
    let prereq = |_: Did| true;
    let player = LocalPlayer {
        journal: &journal,
        gates: &gates,
        prerequisites_pass: &prereq,
    };
    let b = PhasingBox::read(q, &set);
    assert!(!b.test(false, Some(&player)));
    if !b.has_check_list() {
        assert!(!b.test(true, Some(&player)));
    }
    let _ = (b.quests(), b.test::<MapJournal>(true, None));

    let script = QuestStateScript::read(q, &set);
    for g in [
        QuestStateGraph::GatedSingle,
        QuestStateGraph::Double,
        QuestStateGraph::Unswitched,
        QuestStateGraph::Single,
        QuestStateGraph::SingleUnlessPhasedOut,
    ] {
        let r = script.branch(g, |c| c.passes(&journal, &gates, |_| false));
        if script.test == Some(2)
            && g != QuestStateGraph::Unswitched
            && (g != QuestStateGraph::GatedSingle || script.has_quest())
        {
            assert_eq!(r, Some(QuestStateBranch::False));
        }
    }
    for br in [QuestStateBranch::True, QuestStateBranch::False] {
        let _ = (
            BranchVisuals::read(&labels, br, &set),
            branch_draw(q, br, &set, &HashMap::<u32, _>::new()),
        );
    }
    let events = Events(*mode);
    for c in CustomCondition::read_all(&labels, &set) {
        let _ = c.is_well_formed();
        for f in &c.forms {
            let _ = f.holds(&events);
        }
    }
    let l = &labels.logic;
    let _ = (
        CounterBox::read(l, &set),
        LogicBoxTimer::read(l, &set),
        LogicBoxMembers::read(l, &set),
    );
    let _ = (
        GeneratorMapping::read_all(&labels.wiring, &set),
        EventLists::read(&labels.wiring, &set),
    );
    let _ = (
        SkirmishValues::read(&labels.skirmish, &set),
        TravelHooks::read(&labels.travel, &set),
    );
    let _ = PathMapVolume::read(&labels.travel, &set, &HashMap::<u32, _>::new())
        .map(|v| v.is_ordered());
    let _ = (
        ResponseMap::read(&labels.response, &set),
        ResponseCallback::read(&labels.response, &set, |p| {
            (p % 2 == 0).then_some("X_Floor")
        }),
    );
    if let Some(v) = VersionMasterList::read(&labels.version, &set) {
        let _ = (v.objects(), v.is_ascending());
    }

    // The phasing manager with this set as a box and fuzzed targets.
    let targets: Vec<InstanceId> = rest
        .iter()
        .take(8)
        .map(|b| InstanceId(u64::from(*b) + 2))
        .collect();
    let spec = BoxSpec {
        iid: InstanceId(1),
        fields: b,
        targets: targets.clone(),
    };
    let mut m = PhasingManager::new();
    m.player_visibility_mask = Some(u32::from(*mode));
    let created = |iid, as_box| CreatedEntity {
        iid,
        as_box,
        force_refresh_on_enter_world: mode & 4 != 0,
        has_phasing_link: true,
    };
    for &t in &targets {
        let c = m.entity_created(created(t, None), Some(&player));
        assert!(
            c.iter()
                .all(|x| !matches!(x, PhasingChange::Target { state, .. } if !state.phased_out))
        );
    }
    let c = m.entity_created(created(InstanceId(1), Some(spec)), Some(&player));
    assert!(
        c.iter()
            .all(|x| !matches!(x, PhasingChange::Target { state, .. } if !state.phased_out))
    );
    let _ = m.quest_state_arrived(Some(&player));
    for q in [Did(u32::from(*mode)), Did(0)] {
        let _ = m.quest_changed(q, Some(&player));
    }
    if mode & 8 != 0 {
        m.entity_removed(InstanceId(1));
        assert_eq!(m.box_count(), 0);
    }
});
