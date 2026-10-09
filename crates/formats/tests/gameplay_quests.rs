// SPDX-License-Identifier: GPL-3.0-or-later
//! Synthetic tests of the quest-family views (`tessera_formats::gameplay::quests`):
//! the worked examples of `docs/specs/quests.md` §20 and the reader rules of §2.1, §4.3,
//! §5.7, §6.2, §7.1–§7.2, §7.7, §8.2–§8.3, §9.1–§9.2 and §17.
//!
//! All PIDs here are synthetic: every label the module reads gets a made-up PID.

use std::collections::HashMap;

use tessera_formats::gameplay::common::build::{array, did, entry, set, structure};
use tessera_formats::gameplay::quests::{
    AccomplishmentDirectory, CompoundElement, ConditionElement, DirectoryEntry, EventOrigin,
    Operator, QUEST_MAP_NOTES_DID, QuestFamilyKind, QuestLabels, QuestMapNotes, QuestRecord,
    QuestRequirement, QuestSingletons, QuestStatus, RewardLevelTable, RewardList, RoleAction,
    StatusKind, choose_reward_table, listen_list, required_count_key,
};
use tessera_formats::{
    Did, FormatError, PropertyEntry, PropertySet, PropertyType, PropertyValue, StringInfo,
    StringSource,
};

/// Synthetic label → PID assignment covering every label the module reads.
struct Names {
    map: HashMap<String, u32>,
    labels: QuestLabels,
}

impl Names {
    fn new() -> Self {
        let mut map = HashMap::new();
        for (i, label) in QuestLabels::labels().enumerate() {
            map.entry(label.to_owned())
                .or_insert(0x1000 + u32::try_from(i).unwrap());
        }
        // Element labels the views do not need (they read elements by value type).
        for (i, label) in ["Usage_QuestRequirement", "WorldEvent_Condition"]
            .into_iter()
            .enumerate()
        {
            map.insert(label.to_owned(), 0x3000 + u32::try_from(i).unwrap());
        }
        let labels = QuestLabels::resolve(&map);
        assert!(labels.missing().is_empty());
        Self { map, labels }
    }

    fn pid(&self, label: &str) -> u32 {
        *self
            .map
            .get(label)
            .unwrap_or_else(|| panic!("label {label}"))
    }

    fn e(&self, label: &str, value: PropertyValue) -> PropertyEntry {
        entry(self.pid(label), value)
    }

    fn text(&self, label: &str, token: u32) -> PropertyEntry {
        self.e(
            label,
            PropertyValue::StringInfo(Box::new(StringInfo {
                source: StringSource::Table {
                    token,
                    table: Did(0x2500_0001),
                },
                args: None,
            })),
        )
    }

    /// A `QuestEvent_Entry` of type `ty`, with an order value when given.
    fn event(&self, ty: u32, order: Option<i32>, extra: Vec<PropertyEntry>) -> PropertyEntry {
        let mut fields = vec![self.e("QuestEvent_ID", PropertyValue::Enum(ty))];
        if let Some(o) = order {
            fields.push(self.e("QuestEvent_EventOrder", PropertyValue::Int(o)));
        }
        fields.extend(extra);
        self.e("QuestEvent_Entry", structure(fields))
    }

    fn objective(&self, index: i32, conditions: Vec<PropertyEntry>) -> PropertyEntry {
        self.e(
            "Quest_CompletionObjective",
            structure(vec![
                self.e("Quest_ObjectiveIndex", PropertyValue::Int(index)),
                self.e("Quest_CompletionConditionArray", array(conditions)),
            ]),
        )
    }

    fn quest(&self, objectives: Vec<PropertyEntry>, extra: Vec<PropertyEntry>) -> PropertySet {
        self.quest_with_roles(objectives, vec![], extra)
    }

    fn quest_with_roles(
        &self,
        objectives: Vec<PropertyEntry>,
        roles: Vec<PropertyEntry>,
        extra: Vec<PropertyEntry>,
    ) -> PropertySet {
        let mut entries = vec![
            self.text("Quest_Name", 1),
            self.e("Quest_ObjectiveArray", array(objectives)),
            self.e("Quest_RoleArray", array(roles)),
        ];
        entries.extend(extra);
        set(entries)
    }
}

// --- §20.1 event numbering -------------------------------------------------------------------

fn first_example(n: &Names) -> PropertySet {
    let objective_2 = n.objective(
        2,
        vec![
            n.e(
                "Quest_CompletionCondition",
                array(vec![n.e(
                    "QuestEvent_CompoundEvent",
                    array(vec![
                        n.event(11, Some(0), vec![]),
                        n.e(
                            "QuestEvent_Entry_Array",
                            array(vec![n.event(10, Some(1), vec![])]),
                        ),
                    ]),
                )]),
            ),
            n.e(
                "Quest_FailureCondition",
                array(vec![n.event(30, None, vec![])]),
            ),
        ],
    );
    let objective_1 = n.objective(
        1,
        vec![n.e(
            "Quest_CompletionCondition",
            array(vec![n.event(22, Some(0), vec![])]),
        )],
    );
    let failure = n.e(
        "Quest_FailureConditionArray",
        array(vec![n.e(
            "Quest_FailureCondition",
            array(vec![n.event(32, None, vec![])]),
        )]),
    );
    n.quest(vec![objective_2, objective_1], vec![failure])
}

#[test]
fn worked_example_1_numbering() {
    let n = Names::new();
    let set = first_example(&n);
    let quest = QuestRecord::new(&n.labels, &set).unwrap();
    let numbering = quest.number_events().unwrap();

    let got: Vec<(u32, i32, u32, u32, bool)> = numbering
        .events
        .iter()
        .map(|e| {
            (
                e.event_id,
                e.objective_id,
                e.condition_id,
                e.entry.event_type().unwrap(),
                e.is_failure(),
            )
        })
        .collect();
    assert_eq!(
        got,
        vec![
            (1, 2, 1, 11, false),
            (2, 2, 1, 10, false),
            (3, 2, 2, 30, true),
            (4, 1, 1, 22, false),
            (5, 0, 1, 32, true),
        ]
    );
    assert_eq!(numbering.events[4].origin, EventOrigin::QuestFailure);
    assert_eq!(numbering.events[2].origin, EventOrigin::ObjectiveFailure);

    // Sibling lists.
    assert_eq!(numbering.siblings(1), vec![2]);
    assert_eq!(numbering.siblings(2), vec![1]);
    for id in 3..=5 {
        assert!(numbering.siblings(id).is_empty());
    }

    // Listen lists (§6.2): event 1 does not wait for a drama.
    let lists: Vec<Option<Vec<u32>>> = numbering
        .events
        .iter()
        .map(|e| e.entry.listen_list())
        .collect();
    assert_eq!(
        lists,
        vec![
            Some(vec![11]),
            Some(vec![10, 31, 7]),
            Some(vec![30]),
            Some(vec![22]),
            Some(vec![32]),
        ]
    );
    assert_eq!(listen_list(11, true), Some(vec![11, 17, 12]));

    // Objective index of §6.1.
    let index = numbering.objective_index();
    assert_eq!(index.get(&2), Some(&vec![1, 2, 3]));
    assert_eq!(index.get(&1), Some(&vec![4]));
    assert_eq!(index.get(&0), Some(&vec![5]));

    // Order values: 3 and 5 have none.
    let orders: Vec<i32> = numbering
        .events
        .iter()
        .map(|e| e.entry.compiled_order())
        .collect();
    assert_eq!(orders, vec![0, 1, -1, 0, -1]);
}

#[test]
fn worked_example_1_drama_listen_list() {
    let n = Names::new();
    let entry = n.event(
        11,
        Some(0),
        vec![
            n.e("QuestEvent_WaitForDrama", PropertyValue::Bool(true)),
            n.e("QuestEvent_DramaName", PropertyValue::String("d".into())),
        ],
    );
    let set = n.quest(
        vec![n.objective(
            1,
            vec![n.e("Quest_CompletionCondition", array(vec![entry]))],
        )],
        vec![],
    );
    let quest = QuestRecord::new(&n.labels, &set).unwrap();
    let numbering = quest.number_events().unwrap();
    assert_eq!(
        numbering.events[0].entry.listen_list(),
        Some(vec![11, 17, 12])
    );
    // Waiting without a named drama keeps the plain list.
    assert_eq!(listen_list(5, false), Some(vec![5]));
    // Types 19 and 29 depend on an OPEN rule.
    assert_eq!(listen_list(19, false), None);
    assert_eq!(listen_list(29, true), None);
}

#[test]
fn worked_example_2_numbering() {
    let n = Names::new();
    let condition_a = n.e(
        "Quest_CompletionCondition",
        array(vec![
            n.event(21, Some(0), vec![]),
            n.e(
                "QuestEvent_CompoundEvent",
                array(vec![
                    n.event(22, Some(1), vec![]),
                    n.event(22, Some(2), vec![]),
                    n.text("QuestEvent_CompoundProgressOverride", 9),
                ]),
            ),
        ]),
    );
    let condition_b = n.e(
        "Quest_CompletionCondition",
        array(vec![
            n.e("Quest_Condition_NeverFinish", PropertyValue::Bool(true)),
        ]),
    );
    let condition_c = n.e(
        "Quest_CompletionCondition",
        array(vec![n.e(
            "QuestEvent_CompoundEvent",
            array(vec![
                n.event(11, Some(3), vec![]),
                n.e(
                    "QuestEvent_Entry_Array",
                    array(vec![n.event(7, Some(4), vec![])]),
                ),
                n.e("QuestEvent_Entry_Array", array(vec![])),
            ]),
        )]),
    );
    let set = n.quest(
        vec![n.objective(1, vec![condition_a, condition_b, condition_c])],
        vec![],
    );
    let quest = QuestRecord::new(&n.labels, &set).unwrap();

    let objectives = quest.objectives();
    assert_eq!(objectives.len(), 1);
    let conditions = &objectives[0].completion;
    assert_eq!(conditions.len(), 3);
    assert!(conditions[1].is_empty());
    assert_eq!(conditions[1].never_finish, Some(true));
    let ConditionElement::Compound(x) = &conditions[0].elements[1] else {
        panic!("compound X expected");
    };
    assert!(x.progress_override.is_some());
    let ConditionElement::Compound(y) = &conditions[2].elements[0] else {
        panic!("compound Y expected");
    };
    assert!(matches!(&y.elements[2], CompoundElement::EntryArray(v) if v.is_empty()));

    let numbering = quest.number_events().unwrap();
    let got: Vec<(u32, i32, u32, u32)> = numbering
        .events
        .iter()
        .map(|e| {
            (
                e.event_id,
                e.objective_id,
                e.condition_id,
                e.entry.event_type().unwrap(),
            )
        })
        .collect();
    assert_eq!(
        got,
        vec![
            (1, 1, 1, 21),
            (2, 1, 1, 22),
            (3, 1, 1, 22),
            (4, 1, 3, 11),
            (5, 1, 3, 7),
        ]
    );
    assert_eq!(numbering.compounds, vec![vec![2, 3], vec![4, 5]]);
    let siblings: Vec<Vec<u32>> = (1..=5).map(|id| numbering.siblings(id)).collect();
    assert_eq!(siblings, vec![vec![], vec![3], vec![2], vec![5], vec![4]]);
    let lists: Vec<Vec<u32>> = numbering
        .events
        .iter()
        .map(|e| e.entry.listen_list().unwrap())
        .collect();
    assert_eq!(lists, vec![vec![21], vec![22], vec![22], vec![11], vec![7]]);
    assert_eq!(
        numbering.objective_index().get(&1),
        Some(&vec![1, 2, 3, 4, 5])
    );
    // Condition B's flag applies to its (no) events; the others are not never-finish.
    assert!(numbering.events.iter().all(|e| !e.never_finish));
}

#[test]
fn tracked_events_and_required_counts() {
    let n = Names::new();
    let condition = n.e(
        "Quest_CompletionCondition",
        array(vec![
            n.event(22, Some(0), vec![]),
            n.event(22, Some(1), vec![]),
            n.e("Quest_CompletionConditionCount", PropertyValue::Int(1)),
        ]),
    );
    let tracked = n.e(
        "Quest_TrackedConditionsArray",
        array(vec![n.e(
            "Quest_TrackedCondition",
            array(vec![n.event(
                58,
                None,
                vec![n.e("QuestEvent_Points", PropertyValue::Int(-50))],
            )]),
        )]),
    );
    let failure = n.e(
        "Quest_FailureConditionArray",
        array(vec![n.e(
            "Quest_FailureCondition",
            array(vec![n.event(30, None, vec![])]),
        )]),
    );
    let set = n.quest(
        vec![n.objective(3, vec![condition])],
        vec![tracked, failure],
    );
    let quest = QuestRecord::new(&n.labels, &set).unwrap();
    let numbering = quest.number_events().unwrap();
    assert_eq!(numbering.len(), 4);
    // Quest-level failure first, then tracked; both use ObjectiveID 0 and ConditionID 1.
    let failure_event = numbering.get(3).unwrap();
    let tracked_event = numbering.get(4).unwrap();
    assert_eq!(failure_event.origin, EventOrigin::QuestFailure);
    assert_eq!(tracked_event.origin, EventOrigin::Tracked);
    assert_eq!(
        (failure_event.objective_id, failure_event.condition_id),
        (0, 1)
    );
    assert_eq!(
        (tracked_event.objective_id, tracked_event.condition_id),
        (0, 1)
    );
    assert!(failure_event.is_failure() && !tracked_event.is_failure());
    assert!(tracked_event.never_finish && !failure_event.never_finish);
    assert_eq!(tracked_event.entry.points(), Some(-50));
    assert_eq!(numbering.required_counts, vec![(3 * 65_536 + 1, 1)]);
    assert_eq!(required_count_key(3, 1), 196_609);
    assert!(numbering.get(0).is_none() && numbering.get(5).is_none());
}

#[test]
fn objective_without_index_is_an_error() {
    let n = Names::new();
    let objective = n.e(
        "Quest_CompletionObjective",
        structure(vec![n.e("Quest_CompletionConditionArray", array(vec![]))]),
    );
    let set = n.quest(vec![objective], vec![]);
    let quest = QuestRecord::new(&n.labels, &set).unwrap();
    assert!(matches!(
        quest.number_events(),
        Err(FormatError::MissingProperty { .. })
    ));
}

#[test]
fn quest_record_requires_identifying_properties() {
    let n = Names::new();
    let missing_roles = set(vec![
        n.text("Quest_Name", 1),
        n.e("Quest_ObjectiveArray", array(vec![])),
    ]);
    assert!(matches!(
        QuestRecord::new(&n.labels, &missing_roles),
        Err(FormatError::MissingProperty { .. })
    ));
    let wrong_type = set(vec![
        n.text("Quest_Name", 1),
        n.e("Quest_ObjectiveArray", PropertyValue::Int(3)),
        n.e("Quest_RoleArray", array(vec![])),
    ]);
    assert!(matches!(
        QuestRecord::new(&n.labels, &wrong_type),
        Err(FormatError::PropertyTypeMismatch { .. })
    ));
}

// --- §4 quest fields --------------------------------------------------------------------------

#[test]
fn effective_challenge_level_order() {
    let n = Names::new();
    let character_level_pid = 0x0777;
    let base = |extra: Vec<PropertyEntry>| n.quest(vec![], extra);

    let plain = base(vec![n.e("Quest_ChallengeLevel", PropertyValue::Int(12))]);
    let q = QuestRecord::new(&n.labels, &plain).unwrap();
    assert_eq!(q.effective_challenge_level(|_| Some(99)), 12);

    let overridden = base(vec![
        n.e("Quest_ChallengeLevel", PropertyValue::Int(12)),
        n.e(
            "Quest_ChallengeLevelOverrideProperty",
            PropertyValue::PropertyId(character_level_pid),
        ),
    ]);
    let q = QuestRecord::new(&n.labels, &overridden).unwrap();
    assert_eq!(
        q.effective_challenge_level(|p| (p == character_level_pid).then_some(57)),
        57
    );
    // The player read fails: the stored level.
    assert_eq!(q.effective_challenge_level(|_| None), 12);

    let hidden = base(vec![
        n.e("Quest_ChallengeLevel", PropertyValue::Int(12)),
        n.e("Quest_HideChallengeLevel", PropertyValue::Bool(true)),
        n.e(
            "Quest_ChallengeLevelOverrideProperty",
            PropertyValue::PropertyId(character_level_pid),
        ),
    ]);
    let q = QuestRecord::new(&n.labels, &hidden).unwrap();
    assert_eq!(q.effective_challenge_level(|_| Some(57)), 0);

    let zero_override = base(vec![n.e(
        "Quest_ChallengeLevelOverrideProperty",
        PropertyValue::PropertyId(0),
    )]);
    let q = QuestRecord::new(&n.labels, &zero_override).unwrap();
    assert_eq!(q.effective_challenge_level(|_| Some(57)), 0);
}

#[test]
fn deed_split_and_defaults() {
    let n = Names::new();
    let deed = n.quest(
        vec![],
        vec![
            n.e("Quest_IsAccomplishment", PropertyValue::Bool(true)),
            n.e("Accomplishment_UITab", PropertyValue::Enum(7)),
            n.e("Accomplishment_MinLevelToStart", PropertyValue::Int(20)),
        ],
    );
    let q = QuestRecord::new(&n.labels, &deed).unwrap();
    assert!(q.is_deed());
    let info = q.deed().unwrap();
    assert_eq!(info.ui_tab, Some(7));
    assert_eq!(info.min_level_to_start, Some(20));

    let quest = n.quest(vec![], vec![]);
    let q = QuestRecord::new(&n.labels, &quest).unwrap();
    assert!(!q.is_deed() && q.deed().is_none());
    // Dictionary default of Quest_MaxTimesCompletable applies when absent.
    let defaults: HashMap<u32, PropertyValue> =
        HashMap::from([(n.pid("Quest_MaxTimesCompletable"), PropertyValue::Int(1))]);
    assert_eq!(q.max_times_completable(&defaults), Some(1));
}

// --- §7 roles ---------------------------------------------------------------------------------

#[test]
fn role_objective_rule_keeps_absent_distinct_from_zero() {
    let n = Names::new();
    let role = |action: u32, objective: Option<i32>| {
        let mut fields = vec![
            n.e("QuestDispenser_Action", PropertyValue::Enum(action)),
            n.e("QuestDispenser_RoleName", PropertyValue::String("r".into())),
            n.e("QuestDispenser_NPC", did(0x7000_0042)),
        ];
        if let Some(o) = objective {
            fields.push(n.e("Quest_ObjectiveIndex", PropertyValue::Int(o)));
        }
        n.e("Quest_Role", structure(fields))
    };
    let roles = vec![
        role(6, Some(0)),
        role(5, None),
        role(2, Some(1)),
        role(5, Some(0)),
        role(6, None),
        role(1, None),
    ];
    let set = n.quest_with_roles(vec![], roles, vec![]);
    let q = QuestRecord::new(&n.labels, &set).unwrap();
    let got: Vec<(Option<RoleAction>, Option<i32>, bool)> = q
        .roles()
        .iter()
        .map(|r| (r.action(), r.objective_index(), r.objective_rule_holds()))
        .collect();
    assert_eq!(
        got,
        vec![
            (Some(RoleAction::Bestow), Some(0), true),
            (Some(RoleAction::LeaveInstance), None, true),
            (Some(RoleAction::Talk), Some(1), true),
            (Some(RoleAction::LeaveInstance), Some(0), false),
            (Some(RoleAction::Bestow), None, false),
            (Some(RoleAction::Comment), None, false),
        ]
    );
    assert_eq!(q.roles_named("r", Some(Did(0x7000_0042))).len(), 6);
    assert!(q.roles_named("r", Some(Did(0x7000_0043))).is_empty());
    assert_eq!(RoleAction::from_code(14).code(), 14);
    assert_eq!(RoleAction::from_code(8), RoleAction::Other(8));
}

// --- §8 prerequisites -------------------------------------------------------------------------

#[test]
fn worked_example_3_status_codes() {
    let completed = QuestStatus(0x3000_0000);
    assert_eq!(completed, QuestStatus::COMPLETED);
    assert_eq!(completed.kind(), StatusKind::Completed);
    let underway = QuestStatus(0x1000_0003);
    assert_eq!(underway, QuestStatus::underway(3));
    assert_eq!(underway.kind(), StatusKind::Underway);
    assert_eq!(underway.objective(), 3);
    assert_eq!(underway.raw_status(), 0x1000_0000);
    assert_eq!(QuestStatus(0x2000_0000).kind(), StatusKind::Failed);
    assert_eq!(QuestStatus(0x7000_0000).kind(), StatusKind::Other(7));

    let requirement = QuestRequirement {
        operator: 3,
        quest: Some(Did(0x7000_0100)),
        status: Some(QuestStatus::COMPLETED),
    };
    assert_eq!(requirement.operator(), Some(Operator::Equal));
    assert!(!requirement.is_empty_slot());
    assert!(
        QuestRequirement {
            operator: 0,
            ..requirement
        }
        .is_empty_slot()
    );
    assert!(
        QuestRequirement {
            quest: Some(Did(0)),
            ..requirement
        }
        .is_empty_slot()
    );
    assert!(Operator::GreaterOrEqual.compare(3, 3));
    assert!(Operator::Greater.compare(4, 3) && !Operator::Less.compare(4, 3));
    assert_eq!(Operator::from_code(0), None);
}

#[test]
fn permissions_read_requirements_and_world_events() {
    let n = Names::new();
    let req = |op: u32, quest: u32, status: u32| {
        n.e(
            "Usage_QuestRequirement",
            structure(vec![
                n.e("Usage_Operator", PropertyValue::Enum(op)),
                n.e("Usage_QuestID", did(quest)),
                n.e("Usage_QuestStatus", PropertyValue::Enum(status)),
            ]),
        )
    };
    let blob = n.e(
        "DefaultPermissionBlobStruct",
        structure(vec![
            n.e("Usage_MinLevel", PropertyValue::Int(20)),
            n.e(
                "Usage_QuestRequirements",
                array(vec![req(3, 0x7000_0100, 0x3000_0000), req(0, 0, 0)]),
            ),
            n.e(
                "Usage_WorldEvent_AllConditionList",
                array(vec![n.e(
                    "WorldEvent_Condition",
                    structure(vec![
                        n.e("WorldEvent_WorldEvent", did(0x7000_0200)),
                        n.e("WorldEvent_Operator", PropertyValue::Enum(3)),
                        n.e("WorldEvent_ConditionValue", PropertyValue::Int(1)),
                    ]),
                )]),
            ),
        ]),
    );
    let set = n.quest(vec![], vec![blob]);
    let q = QuestRecord::new(&n.labels, &set).unwrap();
    let p = q.permissions().unwrap();
    assert_eq!(p.min_level(), Some(20));
    let reqs = p.requirements();
    assert_eq!(reqs.len(), 2);
    assert_eq!(reqs[0].status, Some(QuestStatus::COMPLETED));
    assert!(reqs[1].is_empty_slot());
    let we = p.world_event_all();
    assert_eq!(we.len(), 1);
    assert_eq!(we[0].world_event, Did(0x7000_0200));
    assert_eq!(we[0].value, Some(1));
    assert!(!we[0].is_empty_slot());
}

// --- §9 rewards -------------------------------------------------------------------------------

#[test]
fn worked_example_2_reward_lookup() {
    let n = Names::new();
    // The element PID does not matter: list elements are read as structs.
    let tier = |t: u32, amount: i32| {
        entry(
            0x9999,
            structure(vec![
                n.e("QuestReward_Tier", PropertyValue::Enum(t)),
                n.e("QuestReward_ExpAmount", PropertyValue::Int(amount)),
            ]),
        )
    };
    let r = set(vec![n.e(
        "QuestReward_ExpEntryList",
        array(vec![tier(1, 100), tier(4, 900)]),
    )]);
    let table_r = Did(0x7000_0012);
    let mut control: Vec<Did> = (1..=20).map(|i| Did(0x7000_0000 + i)).collect();
    control[11] = table_r;

    let quest = n.quest(
        vec![],
        vec![
            n.e("Quest_ChallengeLevel", PropertyValue::Int(12)),
            n.e("Quest_ExpTier", PropertyValue::Enum(4)),
        ],
    );
    let q = QuestRecord::new(&n.labels, &quest).unwrap();
    let level = q.effective_challenge_level(|_| None);
    let chosen = choose_reward_table(q.reward_level(), |_| true, &control, level);
    assert_eq!(chosen, Some(table_r));
    let table = RewardLevelTable::new(&n.labels, &r);
    let tier_value = q.tier(RewardList::Exp).unwrap();
    assert_eq!(table.amount(RewardList::Exp, tier_value), Some(900.0));
    assert_eq!(
        n.labels.classify_set(&r),
        Some(QuestFamilyKind::RewardLevelTable)
    );

    // With Quest_RewardLevelDID = S, S is used regardless of the level.
    let s = Did(0x7000_0555);
    assert_eq!(
        choose_reward_table(Some(s), |_| true, &control, 12),
        Some(s)
    );
    // A named record that is not a table gives no table.
    assert_eq!(choose_reward_table(Some(s), |_| false, &control, 12), None);
    // Level 0 takes the highest level; levels past the table are OPEN.
    assert_eq!(
        choose_reward_table(None, |_| true, &control, 0),
        control.last().copied()
    );
    assert_eq!(choose_reward_table(None, |_| true, &control, 21), None);
    assert_eq!(choose_reward_table(None, |_| true, &control, -1), None);

    // Tier 2,000 without an element gives nothing; tier 0 means none.
    assert_eq!(table.amount(RewardList::Exp, 2000), None);
    assert_eq!(table.amount(RewardList::Exp, 0), None);
    assert_eq!(table.entries(RewardList::Exp).len(), 2);
}

#[test]
fn reputation_tier_comes_from_the_faction_struct() {
    let n = Names::new();
    let quest = n.quest(
        vec![],
        vec![n.e(
            "Quest_PositiveFaction",
            structure(vec![
                n.e("Quest_FactionDID", did(0x7000_0300)),
                n.e("Quest_RepTier", PropertyValue::Enum(3)),
            ]),
        )],
    );
    let q = QuestRecord::new(&n.labels, &quest).unwrap();
    assert_eq!(q.tier(RewardList::Rep), Some(3));
    assert_eq!(
        q.positive_faction().unwrap().faction,
        Some(Did(0x7000_0300))
    );
    assert_eq!(q.tier(RewardList::VirtueExp), None);
}

// --- §2.1 kinds, §10 directories ---------------------------------------------------------------

#[test]
fn kinds_by_identifying_properties() {
    let n = Names::new();
    let l = &n.labels;
    let quest = n.quest(vec![], vec![]);
    assert_eq!(l.classify_set(&quest), Some(QuestFamilyKind::Quest));

    let treasure = set(vec![
        n.e("QuestTreasure_FixedItemArray", array(vec![])),
        n.e("QuestTreasure_Virtue_XP_Tier", PropertyValue::Enum(3)),
    ]);
    assert_eq!(
        l.classify_set(&treasure),
        Some(QuestFamilyKind::QuestTreasure)
    );
    // A treasure property next to another property is not the treasure kind.
    let mixed = set(vec![
        n.e("QuestTreasure_FixedItemArray", array(vec![])),
        n.e("Quest_ChallengeLevel", PropertyValue::Int(3)),
    ]);
    assert_eq!(l.classify_set(&mixed), None);
    assert_eq!(l.classify_set(&set(vec![])), None);

    let bool_event = set(vec![
        n.e("WorldEvent_WorldPropertyName", PropertyValue::PropertyId(5)),
        n.e("WorldEvent_StartActive", PropertyValue::Bool(true)),
    ]);
    assert_eq!(
        l.classify_set(&bool_event),
        Some(QuestFamilyKind::WorldEventBool)
    );
    let int_event = set(vec![
        n.e("WorldEvent_WorldPropertyName", PropertyValue::PropertyId(5)),
        n.e("WorldEvent_StartActive", PropertyValue::Bool(true)),
        n.e("WorldEvent_MaxIntValue", PropertyValue::Int(5)),
    ]);
    assert_eq!(
        l.classify_set(&int_event),
        Some(QuestFamilyKind::WorldEventInt)
    );
    let list = set(vec![
        n.e("WorldEvent_WorldEventList", array(vec![])),
        n.e(
            "WorldEvent_Control_DecayEpsilon",
            PropertyValue::Float(300.0),
        ),
    ]);
    assert_eq!(l.classify_set(&list), Some(QuestFamilyKind::WorldEventList));
    let control = set(vec![
        n.e("NPC_TextDuration", PropertyValue::Float(10.0)),
        n.e("Quest_ColorTextInterval", PropertyValue::Int(5)),
    ]);
    // Singleton kinds come from the WEENIECONTENT labels, not from properties (§2.2, §2.3).
    assert_eq!(l.classify_set(&control), None);
    let singletons = QuestSingletons::resolve(|label| match label {
        "NPCColorControl" => Some(Did(0x7000_0010)),
        "ReputationControl" => Some(Did(0x7000_0020)),
        "WorldEventControl" => Some(Did(0x7000_0030)),
        "EmoteDirectory" => Some(Did(0)),
        _ => None,
    });
    assert_eq!(singletons.emote_directory, None, "a null DID is absent");
    assert_eq!(
        l.classify_record(Did(0x7000_0010), &control, &singletons),
        Some(QuestFamilyKind::ColourTextControl)
    );
    assert_eq!(
        l.classify_record(Did(0x7000_0011), &control, &singletons),
        None
    );
    let faction_table = set(vec![n.e(
        "Reputation_FactionTable",
        array(vec![n.e("Reputation_FactionTable", did(0x7000_0400))]),
    )]);
    assert_eq!(l.classify_set(&faction_table), None);
    assert!(l.matches_kind(&faction_table, QuestFamilyKind::ReputationControl));
    assert_eq!(
        l.classify_record(Did(0x7000_0020), &faction_table, &singletons),
        Some(QuestFamilyKind::ReputationControl)
    );
    assert_eq!(
        l.classify_record(Did(0x7000_0021), &faction_table, &singletons),
        None
    );
    // A labelled record of a kind that is not a singleton keeps that kind.
    assert_eq!(
        l.classify_record(Did(0x7000_0030), &list, &singletons),
        Some(QuestFamilyKind::WorldEventList)
    );
    for kind in QuestFamilyKind::ALL {
        assert_eq!(kind.is_singleton(), kind.weenie_content_label().is_some());
    }
    assert_eq!(
        QuestFamilyKind::ReputationControl.weenie_content_label(),
        Some("ReputationControl")
    );
    assert!(!QuestFamilyKind::WorldEventList.is_singleton());
}

// --- §5.1 element labels of the quest-level lists, §19 lenient numbering, §9.5 ---------------

#[test]
fn quest_level_lists_need_their_element_labels() {
    let n = Names::new();
    let failure = n.e(
        "Quest_FailureConditionArray",
        array(vec![
            n.e(
                "Quest_FailureCondition",
                array(vec![n.event(30, None, vec![])]),
            ),
            // A foreign array is not a condition and takes no ConditionID.
            n.e(
                "Quest_TrackedCondition",
                array(vec![n.event(32, None, vec![])]),
            ),
            n.e(
                "Quest_FailureCondition",
                array(vec![n.event(25, None, vec![])]),
            ),
        ]),
    );
    let tracked = n.e(
        "Quest_TrackedConditionsArray",
        array(vec![
            n.e(
                "Quest_FailureCondition",
                array(vec![n.event(58, None, vec![])]),
            ),
            n.e(
                "Quest_TrackedCondition",
                array(vec![n.event(22, None, vec![])]),
            ),
        ]),
    );
    let set = n.quest(
        vec![n.objective(
            1,
            vec![n.e(
                "Quest_CompletionCondition",
                array(vec![n.event(21, Some(0), vec![])]),
            )],
        )],
        vec![failure, tracked],
    );
    let quest = QuestRecord::new(&n.labels, &set).unwrap();
    assert_eq!(quest.failure_conditions().len(), 2);
    assert_eq!(quest.tracked_conditions().len(), 1);
    let numbering = quest.number_events().unwrap();
    let got: Vec<(u32, i32, u32, Option<u32>, EventOrigin)> = numbering
        .events
        .iter()
        .map(|e| {
            (
                e.event_id,
                e.objective_id,
                e.condition_id,
                e.entry.event_type(),
                e.origin,
            )
        })
        .collect();
    assert_eq!(
        got,
        vec![
            (1, 1, 1, Some(21), EventOrigin::ObjectiveCompletion),
            (2, 0, 1, Some(30), EventOrigin::QuestFailure),
            (3, 0, 2, Some(25), EventOrigin::QuestFailure),
            (4, 0, 1, Some(22), EventOrigin::Tracked),
        ]
    );
}

#[test]
fn lenient_numbering_skips_unindexed_objectives() {
    let n = Names::new();
    let condition = |ty| {
        n.e(
            "Quest_CompletionCondition",
            array(vec![n.event(ty, Some(0), vec![])]),
        )
    };
    let unindexed = n.e(
        "Quest_CompletionObjective",
        structure(vec![
            n.e("Quest_CompletionConditionArray", array(vec![condition(11)])),
        ]),
    );
    let set = n.quest(
        vec![
            n.objective(2, vec![condition(22)]),
            unindexed,
            n.objective(1, vec![condition(7)]),
        ],
        vec![],
    );
    let quest = QuestRecord::new(&n.labels, &set).unwrap();
    assert!(matches!(
        quest.number_events(),
        Err(FormatError::MissingProperty { .. })
    ));
    let lenient = quest.number_events_lenient();
    assert_eq!(lenient.unindexed_objectives, vec![1]);
    let got: Vec<(u32, i32, Option<u32>)> = lenient
        .events
        .iter()
        .map(|e| (e.event_id, e.objective_id, e.entry.event_type()))
        .collect();
    assert_eq!(got, vec![(1, 2, Some(22)), (2, 1, Some(7))]);

    // With every objective indexed both forms agree.
    let indexed = n.quest(vec![n.objective(1, vec![condition(7)])], vec![]);
    let quest = QuestRecord::new(&n.labels, &indexed).unwrap();
    let strict = quest.number_events().unwrap();
    let lenient = quest.number_events_lenient();
    assert!(lenient.unindexed_objectives.is_empty() && strict.unindexed_objectives.is_empty());
    assert_eq!(strict.len(), lenient.len());
}

#[test]
fn negative_reputation_tier() {
    let n = Names::new();
    let negative_only = n.quest(
        vec![],
        vec![n.e(
            "Quest_NegativeFaction",
            structure(vec![
                n.e("Quest_FactionDID", did(0x7000_0301)),
                n.e("Quest_RepTier", PropertyValue::Enum(2)),
            ]),
        )],
    );
    let q = QuestRecord::new(&n.labels, &negative_only).unwrap();
    assert_eq!(q.tier(RewardList::Rep), None);
    assert_eq!(q.negative_tier(), Some(2));
    assert_eq!(
        q.negative_faction().unwrap().faction,
        Some(Did(0x7000_0301))
    );
}

#[test]
fn accomplishment_directory_files_and_sets() {
    let n = Names::new();
    let dir = set(vec![n.e(
        "Accomplishment_List",
        array(vec![
            n.e("Accomplishment_File", did(0x7000_0001)),
            n.e(
                "Accomplishment_Set",
                array(vec![
                    n.e("Accomplishment_File", did(0x7000_0002)),
                    n.e("Accomplishment_File", did(0x7000_0003)),
                ]),
            ),
        ]),
    )]);
    assert_eq!(
        n.labels.classify_set(&dir),
        Some(QuestFamilyKind::AccomplishmentDirectory)
    );
    let view = AccomplishmentDirectory::new(&n.labels, &dir);
    assert_eq!(
        view.entries(),
        vec![
            DirectoryEntry::File(Did(0x7000_0001)),
            DirectoryEntry::Set(vec![Did(0x7000_0002), Did(0x7000_0003)]),
        ]
    );
    assert_eq!(view.dids().len(), 3);
}

// --- §17 map notes ----------------------------------------------------------------------------

/// The bytes of §20.4 with a count of 1.
fn worked_note() -> Vec<u8> {
    let mut b = vec![0x00, 0x00, 0x40, 0x0E, 0x01, 0x00, 0x00, 0x00];
    b.extend_from_slice(&[
        0x1B, 0x02, 0x10, 0x20, 0x05, 0x00, 0x00, 0x00, 0x20, 0x41, 0x00, 0x00, 0xA0, 0x41, 0x00,
        0x00, 0x00, 0x00,
    ]);
    b.extend_from_slice(&[0x00, 0x00, 0x00, 0x00]);
    b.extend_from_slice(&[0x34, 0x12, 0x00, 0x70]);
    b.extend_from_slice(&[0x78, 0x56, 0x00, 0x70]);
    b.extend_from_slice(&[0xCE, 0x04, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00]);
    b.extend_from_slice(&[0x00, 0x01]);
    b.extend_from_slice(&[
        0x70, 0x12, 0x00, 0x10, 0x70, 0x12, 0x00, 0x10, 0x03, 0x6B, 0x65, 0x79,
    ]);
    b
}

fn note_schema(pid: u32) -> Option<PropertyType> {
    match pid {
        0x4CE => Some(PropertyType::Array),
        0x1000_1270 => Some(PropertyType::String),
        _ => None,
    }
}

#[test]
fn worked_example_4_map_note() {
    let bytes = worked_note();
    let notes = QuestMapNotes::parse_expecting(&bytes, QUEST_MAP_NOTES_DID, &note_schema).unwrap();
    assert_eq!(notes.notes.len(), 1);
    let note = &notes.notes[0];
    assert_eq!(note.position.flags(), 0x1B);
    assert_eq!(note.position.region(), Some(2));
    assert_eq!(note.position.landblock(), Some((0x10, 0x20)));
    assert_eq!(note.position.cell(), Some(5));
    assert_eq!(note.position.offset(), Some([10.0, 20.0, 0.0]));
    assert!(note.area.is_null());
    assert_eq!(note.dungeon, Did(0x7000_1234));
    assert_eq!(note.target, Did(0x7000_5678));
    assert_eq!(note.layers.as_ref().map(|e| e.pid), Some(0x4CE));
    assert!(note.content_layers().is_empty());
    let mut names = HashMap::new();
    names.insert("QuestDispenser_RoleConstraint".to_owned(), 0x1000_1270u32);
    let labels = QuestLabels::resolve(&names);
    assert_eq!(note.role_constraint(&labels), Some("key"));
    assert!(note.quest_roles(&labels).is_empty());
    assert_eq!(notes.for_dungeon(Did(0x7000_1234)).count(), 1);
    assert_eq!(notes.dungeons().len(), 1);
}

#[test]
fn map_note_null_layers_pid_is_four_bytes() {
    let mut bytes = worked_note();
    // Replace "CE 04 00 00 | 00 00 00 00" (PID + empty array) by a null PID.
    let at = 8 + 18 + 12;
    bytes.splice(at..at + 8, [0, 0, 0, 0]);
    let notes = QuestMapNotes::parse(&bytes, &note_schema).unwrap();
    assert!(notes.notes[0].layers.is_none());
}

#[test]
fn map_notes_reject_hostile_input() {
    let bytes = worked_note();
    // Trailing byte (V10).
    let mut trailing = bytes.clone();
    trailing.push(0);
    assert!(matches!(
        QuestMapNotes::parse(&trailing, &note_schema),
        Err(FormatError::TrailingBytes { .. })
    ));
    // Every truncation fails cleanly.
    for len in 0..bytes.len() {
        assert!(
            QuestMapNotes::parse(&bytes[..len], &note_schema).is_err(),
            "{len}"
        );
    }
    // A huge count is rejected before allocating.
    let mut huge = bytes.clone();
    huge[4..8].copy_from_slice(&u32::MAX.to_le_bytes());
    assert!(QuestMapNotes::parse(&huge, &note_schema).is_err());
    // Unknown PID in the info set.
    assert!(
        QuestMapNotes::parse(&bytes, &|pid| (pid == 0x4CE).then_some(PropertyType::Array)).is_err()
    );
    // Wrong DID.
    assert!(matches!(
        QuestMapNotes::parse_expecting(&bytes, Did(0x0E40_0001), &note_schema),
        Err(FormatError::DidMismatch { .. })
    ));
}

#[test]
fn views_tolerate_mistyped_values() {
    let n = Names::new();
    // Every label holding an Int: no view may panic, and typed accessors give None.
    let entries: Vec<PropertyEntry> = QuestLabels::labels()
        .map(|l| (n.pid(l), n.e(l, PropertyValue::Int(-1))))
        .collect::<std::collections::BTreeMap<u32, PropertyEntry>>()
        .into_values()
        .collect();
    let bogus = set(entries);
    let q = QuestRecord::new_unchecked(&n.labels, &bogus);
    assert!(q.objectives().is_empty());
    assert!(q.roles().is_empty());
    assert!(q.name().is_none());
    assert!(q.number_events().unwrap().is_empty());
    assert_eq!(q.effective_challenge_level(|_| Some(3)), -1);
    let _ = n.labels.classify_set(&bogus);
}
