// SPDX-License-Identifier: GPL-3.0-or-later
//! Synthetic tests of the world-logic views and rules (`world-logic.md` §4–§6, §11, §12).

use std::collections::{BTreeMap, HashMap};

use super::*;
use crate::gameplay::common::build::{array, did, entry, set, structure};
use crate::gameplay::quests::MapJournal;
use crate::property_set::PropertySet;

const Q: u32 = 0x7000_0101;
const R: u32 = 0x7000_0102;

fn names() -> HashMap<String, u32> {
    WorldLogicLabels::labels()
        .enumerate()
        .map(|(i, l)| (l.to_owned(), 0x1000_0000 + u32::try_from(i).unwrap()))
        .collect()
}

fn labels() -> WorldLogicLabels {
    WorldLogicLabels::resolve(&names())
}

fn pid(l: Pid) -> u32 {
    l.id().unwrap()
}

fn check(p: &QuestStatePids, quest: u32, status: u32, op: u32) -> PropertyEntry {
    entry(
        pid(p.check_struct),
        structure(vec![
            entry(pid(p.quest_id), did(quest)),
            entry(pid(p.quest_status), PropertyValue::Enum(status)),
            entry(pid(p.quest_operator), PropertyValue::Enum(op)),
        ]),
    )
}

/// The box of §12.1: (Q, underway at 2, ≤), (R, completed, ≠), no operator, `True_Draw`.
fn example_box(l: &WorldLogicLabels, true_draw: Option<bool>, op: Option<u32>) -> PropertySet {
    let p = &l.quest;
    let mut entries = vec![entry(
        pid(p.check_list),
        array(vec![
            check(p, Q, 0x1000_0002, 5),
            check(p, R, 0x3000_0000, 2),
        ]),
    )];
    if let Some(t) = true_draw {
        entries.push(entry(pid(p.true_draw), PropertyValue::Bool(t)));
    }
    if let Some(o) = op {
        entries.push(entry(pid(p.check_operator), PropertyValue::Enum(o)));
    }
    set(entries)
}

fn journal(underway: &[(u32, u32)], completed: &[u32]) -> MapJournal {
    MapJournal {
        completed: completed.iter().map(|&d| Did(d)).collect(),
        failed: BTreeSet::new(),
        underway: underway
            .iter()
            .map(|&(d, o)| (Did(d), o))
            .collect::<BTreeMap<_, _>>(),
    }
}

fn test_box(b: &PhasingBox, ready: bool, j: &MapJournal) -> bool {
    let gates = EntryGates::default();
    let prereq = |_: Did| false;
    let player = LocalPlayer {
        journal: j,
        gates: &gates,
        prerequisites_pass: &prereq,
    };
    b.test(ready, Some(&player))
}

#[test]
fn worked_example_box() {
    let l = labels();
    let b = PhasingBox::read(&l.quest, &example_box(&l, Some(true), None));
    assert_eq!(b.operator(), CHECK_AND);
    assert_eq!(b.quests(), vec![Did(Q), Did(R)]);
    let at1 = journal(&[(Q, 1)], &[]);
    // Step 1: before quest state arrives every box tests false (WL-B1).
    assert!(!test_box(&b, false, &at1));
    // Step 2: entry 1 passes (1 ≤ 2), entry 2 passes (R not completed, ≠): shown.
    assert!(test_box(&b, true, &at1));
    // Step 3: Q advances to objective 3: entry 1 fails, phased out.
    let at3 = journal(&[(Q, 3)], &[]);
    assert!(!test_box(&b, true, &at3));
    // Step 4: `True_Draw` absent inverts every result.
    let inv = PhasingBox::read(&l.quest, &example_box(&l, None, None));
    assert!(!test_box(&inv, true, &at1));
    assert!(test_box(&inv, true, &at3));
    // `Or` in step 3: entry 2 still passes.
    let or = PhasingBox::read(&l.quest, &example_box(&l, Some(true), Some(CHECK_OR)));
    assert!(test_box(&or, true, &at3));
}

#[test]
fn box_without_list_and_without_player() {
    let l = labels();
    let p = &l.quest;
    // WL-B2: no check list → false whatever True_Draw says.
    for t in [true, false] {
        let s = set(vec![entry(pid(p.true_draw), PropertyValue::Bool(t))]);
        let b = PhasingBox::read(p, &s);
        assert!(!b.has_check_list());
        assert!(!test_box(&b, true, &MapJournal::default()));
    }
    // No local player: the list evaluates false; with True_Draw true → false.
    let b = PhasingBox::read(p, &example_box(&l, Some(true), None));
    assert!(!b.test::<MapJournal>(true, None));
    // … and with the polarity false the inversion gives true.
    let b = PhasingBox::read(p, &example_box(&l, Some(false), None));
    assert!(b.test::<MapJournal>(true, None));
}

#[test]
fn list_operators() {
    let pass = QuestCheck {
        quest: Did(Q),
        status: QuestStatus(0x3000_0000),
        operator: 2,
    };
    let fail = QuestCheck {
        operator: 3,
        ..pass
    };
    let j = MapJournal::default();
    let gates = EntryGates::default();
    let prereq = |_: Did| false;
    let player = LocalPlayer {
        journal: &j,
        gates: &gates,
        prerequisites_pass: &prereq,
    };
    let p = Some(&player);
    assert!(evaluate_check_list(&[], CHECK_AND, p));
    assert!(!evaluate_check_list(&[], CHECK_OR, p));
    assert!(!evaluate_check_list(&[], CHECK_XOR, p));
    assert!(evaluate_check_list(&[pass, fail], CHECK_OR, p));
    assert!(!evaluate_check_list(&[pass, fail], CHECK_AND, p));
    assert!(evaluate_check_list(&[fail, pass, fail], CHECK_XOR, p));
    assert!(!evaluate_check_list(&[pass, pass], CHECK_XOR, p));
    assert!(!evaluate_check_list(&[pass], 7, p));
}

#[test]
fn empty_journal_rule_d1() {
    // ≠ completed, ≠ failed, ≠/</≤ underway pass; everything else fails.
    let j = MapJournal::default();
    let g = EntryGates::default();
    let e = |op, status| entry_passes(&j, &g, |_| false, op, Did(Q), QuestStatus(status));
    assert!(e(2, 0x3000_0000));
    assert!(e(2, 0x2000_0000));
    for op in [2, 4, 5] {
        assert!(e(op, 0x1000_0003), "op {op}");
    }
    for op in [0, 1, 3, 6] {
        assert!(!e(op, 0x1000_0003), "op {op}");
        assert!(!e(op, 0x3000_0000), "op {op}");
    }
    assert!(!e(3, 0x4000_0000));
    assert!(!e(2, 0x5000_0000));
}

#[test]
fn entry_rule_underway_and_gates() {
    let j = journal(&[(Q, 3)], &[R]);
    let mut g = EntryGates::default();
    let e = |g: &EntryGates, op, q, status| {
        entry_passes(&j, g, |_| true, op, Did(q), QuestStatus(status))
    };
    // n = 0: ≠ fails, anything else passes.
    assert!(!e(&g, 2, Q, 0x1000_0000));
    assert!(e(&g, 3, Q, 0x1000_0000));
    assert!(e(&g, 0, Q, 0x1000_0000));
    // n > 0: c op n, unsigned; op 0 fails.
    assert!(e(&g, 1, Q, 0x1000_0003));
    assert!(e(&g, 6, Q, 0x1000_0002));
    assert!(!e(&g, 4, Q, 0x1000_0003));
    assert!(!e(&g, 0, Q, 0x1000_0003));
    assert!(e(&g, 2, Q, 0x1000_0004));
    // Not underway but completed: ≥ and > pass, < and ≤ fail.
    assert!(e(&g, 1, R, 0x1000_0002));
    assert!(!e(&g, 5, R, 0x1000_0002));
    // Disabled quests fail; bypassable quests pass for a weenie type with four low bits.
    g.disabled.insert(Did(R));
    assert!(!e(&g, 3, R, 0x3000_0000));
    g.bypassable.insert(Did(R));
    assert!(!e(&g, 3, R, 0x4000_0000));
    g.weenie_type = 0x1F;
    assert!(e(&g, 3, R, 0x4000_0000));
}

#[test]
fn phased_state_and_box_visibility() {
    let l = labels();
    let s = PhasedState::new(false);
    assert!(!s.render_draw && !s.selectable_override && s.phased_out);
    let e = s.entries(&l.quest);
    assert_eq!(e.len(), 3);
    assert!(box_visible(Some(3)));
    assert!(!box_visible(Some(1)));
    assert!(!box_visible(None));
}

fn script_set(l: &WorldLogicLabels, test: Option<u32>, quest: u32) -> PropertySet {
    let p = &l.quest;
    let mut e = vec![
        entry(pid(p.quest_id), did(quest)),
        entry(pid(p.quest_status), PropertyValue::Enum(0x3000_0000)),
        entry(pid(p.quest_operator), PropertyValue::Enum(3)),
    ];
    if let Some(t) = test {
        e.push(entry(pid(p.test), PropertyValue::Enum(t)));
    }
    set(e)
}

#[test]
fn worked_example_script_path() {
    let l = labels();
    let s = QuestStateScript::read(&l.quest, &script_set(&l, None, Q));
    let completed = journal(&[], &[Q]);
    let none = MapJournal::default();
    let g = EntryGates::default();
    assert_eq!(
        s.branch(QuestStateGraph::GatedSingle, |c| c.passes(
            &completed,
            &g,
            |_| false
        )),
        Some(QuestStateBranch::True)
    );
    assert_eq!(
        s.branch(QuestStateGraph::GatedSingle, |c| c.passes(
            &none,
            &g,
            |_| false
        )),
        Some(QuestStateBranch::False)
    );
    // Draw after the branch: absent True_Draw reads its default true, False_Draw false.
    let defaults: BTreeMap<u32, PropertyValue> = [
        (pid(l.quest.true_draw), PropertyValue::Bool(true)),
        (pid(l.quest.false_draw), PropertyValue::Bool(false)),
    ]
    .into_iter()
    .collect();
    let set0 = script_set(&l, None, Q);
    assert!(branch_draw(
        &l.quest,
        QuestStateBranch::True,
        &set0,
        &defaults
    ));
    assert!(!branch_draw(
        &l.quest,
        QuestStateBranch::False,
        &set0,
        &defaults
    ));
    // QuestState_Test = 1: True without the callback.
    let forced = QuestStateScript::read(&l.quest, &script_set(&l, Some(1), Q));
    assert_eq!(
        forced.branch(QuestStateGraph::GatedSingle, |_| panic!("not called")),
        Some(QuestStateBranch::True)
    );
    // WL-B8: QuestState_Test = 2 runs only the False script, whatever the quest.
    let f = QuestStateScript::read(&l.quest, &script_set(&l, Some(2), Q));
    for g in [
        QuestStateGraph::GatedSingle,
        QuestStateGraph::Double,
        QuestStateGraph::Single,
        QuestStateGraph::SingleUnlessPhasedOut,
    ] {
        assert_eq!(f.branch(g, |_| true), Some(QuestStateBranch::False));
    }
    // The gated graph needs a quest; the unswitched one ignores QuestState_Test.
    let noq = QuestStateScript::read(&l.quest, &script_set(&l, None, 0));
    assert_eq!(noq.branch(QuestStateGraph::GatedSingle, |_| true), None);
    assert_eq!(
        f.branch(QuestStateGraph::Unswitched, |_| true),
        Some(QuestStateBranch::True)
    );
    // Phased out: the decal graph tests nothing.
    let mut po = QuestStateScript::read(&l.quest, &script_set(&l, None, Q));
    po.phased_out = Some(true);
    assert_eq!(
        po.branch(QuestStateGraph::SingleUnlessPhasedOut, |_| true),
        None
    );
}

#[test]
fn branch_visuals_read_appearances_in_node_order() {
    let l = labels();
    let t = &l.true_visuals;
    let s = set(vec![
        entry(pid(t.apr1_file), did(0x2000_0001)),
        entry(pid(t.apr3_file), did(0x2000_0003)),
        entry(pid(t.apr3_key), PropertyValue::Enum(7)),
        entry(pid(t.particle), did(0x3900_0001)),
        entry(pid(t.cosmetic1_item), did(0x4700_0001)),
        entry(pid(t.cosmetic1_location), PropertyValue::Enum(1)),
    ]);
    let v = BranchVisuals::read(&l, QuestStateBranch::True, &s);
    assert_eq!(v.particle, Some(Did(0x3900_0001)));
    assert_eq!(v.cosmetics[0], (Some(Did(0x4700_0001)), Some(1)));
    assert_eq!(
        v.appearances,
        vec![
            (Did(0x2000_0003), Some(7), None),
            (Did(0x2000_0001), None, None)
        ]
    );
    assert_eq!(
        BranchVisuals::read(&l, QuestStateBranch::False, &s),
        BranchVisuals::default()
    );
}

struct Events;

impl WorldEventState for Events {
    fn kind(&self, event: Did) -> Option<WorldEventKind> {
        match event.0 {
            1 => Some(WorldEventKind::Integer {
                min: Some(0),
                max: Some(10),
            }),
            2 => Some(WorldEventKind::Boolean),
            _ => None,
        }
    }
    fn value(&self, event: Did) -> Option<i64> {
        match event.0 {
            1 => Some(4),
            2 => Some(1),
            _ => None,
        }
    }
}

fn cond(event: u32, operator: u32, value: i32) -> WorldEventCondition {
    WorldEventCondition {
        world_event: Did(event),
        operator,
        value: Some(value),
        compare_with: None,
    }
}

#[test]
fn worked_example_world_event_conditions() {
    let s = Events;
    assert!(world_condition_holds(&s, &cond(1, 1, 3), None));
    assert!(!world_condition_holds(&s, &cond(1, 6, 4), None));
    assert!(!world_condition_holds(&s, &cond(1, 3, 12), None));
    assert!(!world_condition_holds(&s, &cond(1, 0, 4), None));
    assert!(world_condition_holds(&s, &cond(2, 3, 1), None));
    assert!(!world_condition_holds(&s, &cond(2, 2, 1), None));
    assert!(!world_condition_holds(&s, &cond(2, 1, 1), None));
    assert!(!world_condition_holds(&s, &cond(2, 3, 2), None));
    // Unknown events and unknown comparison events are false.
    assert!(!world_condition_holds(&s, &cond(9, 3, 1), None));
    let mut c = cond(1, 3, 0);
    c.compare_with = Some(Did(9));
    assert!(!world_condition_holds(&s, &c, None));
    c.compare_with = Some(Did(1));
    assert!(world_condition_holds(&s, &c, None));
    // A stored state index reads the indexed value (none here).
    assert!(!world_condition_holds(&s, &cond(1, 1, 3), Some(0)));
    let all = ConditionForm::All(vec![(cond(1, 1, 3), None), (cond(2, 3, 1), None)]);
    let any = ConditionForm::Any(vec![(cond(1, 6, 4), None), (cond(2, 3, 1), None)]);
    assert!(all.holds(&s) && any.holds(&s));
}

#[test]
fn custom_conditions_and_logic_views() {
    let l = labels();
    let w = &l.wiring;
    let ev = |e: u32| {
        structure(vec![
            entry(pid(l.world.world_event), did(e)),
            entry(pid(l.world.operator), PropertyValue::Enum(3)),
            entry(pid(l.world.condition_value), PropertyValue::Int(1)),
        ])
    };
    let s = set(vec![
        entry(
            pid(w.custom_conditions),
            array(vec![
                entry(
                    pid(w.custom_condition),
                    structure(vec![
                        entry(pid(w.custom_link), PropertyValue::Int(1)),
                        entry(pid(w.condition), ev(2)),
                    ]),
                ),
                entry(
                    pid(w.custom_condition),
                    structure(vec![
                        entry(pid(w.custom_link), PropertyValue::Int(7)),
                        entry(
                            pid(l.world.any_conditions),
                            array(vec![entry(pid(w.condition), ev(2))]),
                        ),
                    ]),
                ),
            ]),
        ),
        entry(pid(l.logic.counter_max), PropertyValue::Int(3)),
        entry(pid(l.logic.timer_max), PropertyValue::Float(25.0)),
        entry(pid(w.activate), array(vec![entry(1, did(0x7000_0005))])),
    ]);
    let cc = CustomCondition::read_all(&l, &s);
    assert_eq!(cc.len(), 2);
    assert!(cc[0].is_well_formed() && !cc[1].is_well_formed());
    assert!(cc[0].forms[0].holds(&Events) && cc[1].forms[0].holds(&Events));
    assert_eq!(
        CounterBox::read(&l.logic, &s)
            .unwrap()
            .effective_max_count(),
        3
    );
    assert_eq!(LogicBoxTimer::read(&l.logic, &s).unwrap().max, Some(25.0));
    assert!(LogicBoxMembers::read(&l.logic, &s).is_none());
    let lists = EventLists::read(w, &s);
    assert_eq!(lists.lists, vec![("Activate", vec![Did(0x7000_0005)])]);
}

#[test]
fn path_map_volumes_take_defaults() {
    let l = labels();
    let t = &l.travel;
    let defaults: BTreeMap<u32, PropertyValue> = [
        (pid(t.box_min), PropertyValue::Vector3([-0.25, -1.0, 0.0])),
        (pid(t.box_max), PropertyValue::Vector3([0.25, 1.0, 1.0])),
    ]
    .into_iter()
    .collect();
    let s = set(vec![entry(
        pid(t.box_max),
        PropertyValue::Vector3([50.0, 50.0, 100.0]),
    )]);
    let v = PathMapVolume::read(t, &s, &defaults).unwrap();
    assert_eq!(
        v,
        PathMapVolume::Box {
            min: [-0.25, -1.0, 0.0],
            max: [50.0, 50.0, 100.0]
        }
    );
    assert!(v.is_ordered());
    assert!(PathMapVolume::read(t, &set(vec![]), &defaults).is_none());
}

#[test]
fn version_master_list() {
    let l = labels();
    let v = &l.version;
    let s = set(vec![
        entry(pid(v.number), PropertyValue::Enum(4920)),
        entry(pid(v.always_run), array(vec![entry(1, did(0x7000_0009))])),
        entry(
            pid(v.data_list),
            array(vec![
                entry(
                    1,
                    structure(vec![
                        entry(pid(v.number), PropertyValue::Enum(151)),
                        entry(
                            pid(v.singleton_list),
                            array(vec![entry(1, did(0x7000_0001))]),
                        ),
                    ]),
                ),
                entry(
                    1,
                    structure(vec![
                        entry(pid(v.number), PropertyValue::Int(152)),
                        entry(
                            pid(v.account_singleton_list),
                            array(vec![entry(1, did(0x7000_0002))]),
                        ),
                    ]),
                ),
            ]),
        ),
    ]);
    let m = VersionMasterList::read(v, &s).unwrap();
    assert!(m.is_ascending());
    assert_eq!(
        m.objects(),
        vec![Did(0x7000_0001), Did(0x7000_0002), Did(0x7000_0009)]
    );
    assert!(m.entries[1].2);
}
