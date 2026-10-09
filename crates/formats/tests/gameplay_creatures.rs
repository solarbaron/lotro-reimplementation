// SPDX-License-Identifier: GPL-3.0-or-later
//! Synthetic tests of `tessera_formats::gameplay::creatures` (`docs/specs/creatures.md`):
//! the worked examples of §12, the template rules of §2.2 and §3.2, the AI-record readers of
//! §5, the profile grammar checks of §6.5, the landblock spawn index of §6.2–§6.4 and §6.9,
//! and the encounter table of §7.
//!
//! Every PID, DID, class index and enum key here is invented. Labels the spec abbreviates
//! (`…_RepeatMinTimes` and the like) get synthetic full names containing `Synthetic`.

use std::collections::{BTreeMap, HashMap};

use tessera_formats::gameplay::common::build::{array, did, entry, set, structure};
use tessera_formats::gameplay::common::{ClassParents, ClassTree, PropertyAccess};
use tessera_formats::gameplay::creatures::*;
use tessera_formats::landblock::{
    InstanceId, LandblockInfo, Link, StaticEntity, TSizeList, Weenie,
};
use tessera_formats::{
    Did, FormatError, PropertyEntry, PropertySet, PropertyValue, Record, StringInfo, StringSource,
};

// ---------------------------------------------------------------------------------------------
// Synthetic dictionary
// ---------------------------------------------------------------------------------------------

/// Labels added for the tests besides those the module resolves.
const EXTRA_LABELS: &[&str] = &[
    // Abbreviated in the spec; synthetic full names.
    "GeneratorProfile_Synthetic_RepeatMinTimes",
    "GeneratorProfile_Synthetic_RepeatMaxTimes",
    "GeneratorProfile_Synthetic_ExactPlacementX",
    "GeneratorProfile_Synthetic_ExactPlacementY",
    "GeneratorProfile_Synthetic_ExactPlacementZ",
    "GeneratorProfile_Synthetic_RandomPlacementMinDist",
    "GeneratorProfile_Synthetic_RandomPlacementMaxDist",
    "ClassificationFilter_Synthetic_Alignment",
    "ClassificationFilter_Synthetic_Genus",
    "ClassificationFilter_Synthetic_Species",
    "ClassificationFilter_Synthetic_Subspecies",
    // §12.1.
    "Health_MaxLevel_Cached",
    "AI_PerceptionRadius_ForwardSource",
    "Combat_CanBlock_Cached",
    "ForwardSource_Movement_SpeedMultiplier",
    // Element labels the readers do not name.
    "Synthetic_Element",
    "Synthetic_Struct",
];

struct Fixture {
    labels: LabelIndex,
    pids: CreaturePids,
}

impl Fixture {
    fn new() -> Self {
        let mut names: Vec<&str> = Vec::new();
        for l in TemplatePids::LABELS
            .iter()
            .chain(AiRecordPids::LABELS)
            .chain(ProfilePids::LABELS)
            .chain(SpawnPids::LABELS)
            .chain(EXTRA_LABELS)
        {
            if !names.contains(l) {
                names.push(l);
            }
        }
        let labels = LabelIndex::new(names.iter().zip(0x1000u32..).map(|(n, p)| (p, *n)));
        let pids = CreaturePids::resolve(&labels);
        assert!(pids.missing().is_empty(), "{:?}", pids.missing());
        Self { labels, pids }
    }

    fn p(&self, name: &str) -> u32 {
        self.labels
            .pid(name)
            .unwrap_or_else(|| panic!("label {name}"))
    }

    fn e(&self, name: &str, v: PropertyValue) -> PropertyEntry {
        entry(self.p(name), v)
    }
}

fn int(v: i32) -> PropertyValue {
    PropertyValue::Int(v)
}

fn float(v: f32) -> PropertyValue {
    PropertyValue::Float(v)
}

fn en(v: u32) -> PropertyValue {
    PropertyValue::Enum(v)
}

// ---------------------------------------------------------------------------------------------
// §2.2 Finding templates
// ---------------------------------------------------------------------------------------------

#[test]
fn template_kinds_and_subtree() {
    let kinds = WeenieKinds::from_labels([
        (1, "Monster"),
        (2, "RealNPC"),
        (3, "NPC"),
        (4, "GameplayContainer"),
        (5, "Admin"),
        (1, "Player"), // a later label for key 1 is ignored
    ]);
    assert_eq!(kinds.kind(1), Some(TemplateKind::Mob));
    assert_eq!(kinds.kind(2), Some(TemplateKind::Npc));
    assert_eq!(kinds.kind(3), Some(TemplateKind::Npc));
    assert_eq!(kinds.kind(4), Some(TemplateKind::Container));
    assert_eq!(kinds.kind(5), None);
    assert_eq!(kinds.len(), 4);

    // 50 ─┬─ 53 ─┬─ 51 (mobs)
    //     │      ├─ 52 (NPCs)
    //     │      └─ 55 (no kind; a template by ancestry)
    //     └─ 54 (containers)
    // 40 ── 60 (not a template)
    let tree = ClassTree::from_edges([(53, 50), (51, 53), (52, 53), (55, 53), (54, 50), (60, 40)]);
    let mut finder = TemplateFinder::new();
    finder.add(51, Some(TemplateKind::Mob));
    finder.add(51, Some(TemplateKind::Mob));
    finder.add(52, Some(TemplateKind::Npc));
    finder.add(54, Some(TemplateKind::Container));
    finder.add(55, None);
    finder.add(60, None);
    assert_eq!(finder.records(), 6);
    assert_eq!(
        finder.kind_classes().collect::<Vec<_>>(),
        vec![
            (51, TemplateKind::Mob, 2),
            (52, TemplateKind::Npc, 1),
            (54, TemplateKind::Container, 1)
        ]
    );
    let sub = finder.subtree(&tree).expect("subtree");
    assert_eq!(sub.root, 50);
    assert!(sub.contains(&tree, 55) && sub.contains(&tree, 51) && !sub.contains(&tree, 60));
    assert!(tree.is_a(55, 50));

    // The kind of a set by its WeenieType.
    let f = Fixture::new();
    let s = set(vec![f.e("WeenieType", en(2))]);
    assert_eq!(
        kinds.of_set(&s, f.pids.template.weenie_type),
        Some(TemplateKind::Npc)
    );
    assert_eq!(
        kinds.of_set(&set(vec![]), f.pids.template.weenie_type),
        None
    );
}

// ---------------------------------------------------------------------------------------------
// §12.1 Authored subset of a three-record class
// ---------------------------------------------------------------------------------------------

#[test]
fn worked_example_12_1_template_store() {
    let f = Fixture::new();
    let mirrors = MirrorTable::from_labels(&f.labels);
    for m in [
        "AI_PerceptionRadius_ForwardSource",
        "Combat_CanBlock_Cached",
        "ForwardSource_Movement_SpeedMultiplier",
        "Health_MaxLevel_Cached",
    ] {
        assert!(mirrors.is_mirror(f.p(m)), "{m}");
    }
    assert_eq!(
        mirrors.get(f.p("ForwardSource_Movement_SpeedMultiplier")),
        Some(MirrorInfo {
            pattern: MirrorPattern::ForwardSourcePrefix,
            base: Some(f.p("Movement_SpeedMultiplier")),
        })
    );

    let k = 7u32;
    let t = |radius: f32, quest: Option<u32>, speed: Option<f32>, mirror: f32| -> PropertySet {
        let mut v = vec![
            f.e("Health_MaxLevel", int(10)),
            f.e("AI_PerceptionRadius", float(radius)),
            f.e("AI_PerceptionRadius_ForwardSource", float(radius)),
            f.e("Combat_CanBlock", PropertyValue::Bool(true)),
            f.e("Combat_CanBlock_Cached", PropertyValue::Bool(true)),
            f.e("ForwardSource_Movement_SpeedMultiplier", float(mirror)),
        ];
        if let Some(q) = quest {
            v.push(f.e("Quest_RelatedQuest", did(q)));
        }
        if let Some(s) = speed {
            v.push(f.e("Movement_SpeedMultiplier", float(s)));
        }
        set(v)
    };
    let (d1, d2, d3) = (Did(0x7000_0001), Did(0x7000_0002), Did(0x7000_0003));
    let s1 = t(7.0, None, None, 1.0);
    let s2 = t(12.0, Some(0x7000_1234), Some(2.0), 1.0);
    let s3 = t(7.0, None, None, 1.5);
    let records = [(d1, k, &s1), (d2, k, &s2), (d3, k, &s3)];
    let store = TemplateStore::build(mirrors, records);

    // Class K: Health_MaxLevel and Combat_CanBlock constant; three varying; three mirrors.
    let info = store.constants().class(k).expect("class");
    assert_eq!(info.records, 3);
    let constants: Vec<u32> = info.constants.iter().map(|e| e.pid).collect();
    let mut want = vec![f.p("Health_MaxLevel"), f.p("Combat_CanBlock")];
    want.sort_unstable();
    assert_eq!(constants, want);
    let mut want = vec![
        f.p("AI_PerceptionRadius"),
        f.p("Quest_RelatedQuest"),
        f.p("Movement_SpeedMultiplier"),
    ];
    want.sort_unstable();
    assert_eq!(info.varying, want);
    assert_eq!(info.mirror_pids.len(), 3);
    assert_eq!(info.distinct_pids(), 8);

    // What each template stores.
    let stored = |d: Did| -> BTreeMap<u32, PropertyValue> {
        store
            .stored(d)
            .expect("stored")
            .stored()
            .iter()
            .map(|e| (e.pid, e.value.clone()))
            .collect()
    };
    let fs = f.p("ForwardSource_Movement_SpeedMultiplier");
    let radius = f.p("AI_PerceptionRadius");
    let speed = f.p("Movement_SpeedMultiplier");
    let quest = f.p("Quest_RelatedQuest");
    assert_eq!(
        stored(d1),
        BTreeMap::from([(radius, float(7.0)), (fs, float(1.0))])
    );
    assert_eq!(
        stored(d2),
        BTreeMap::from([
            (radius, float(12.0)),
            (quest, did(0x7000_1234)),
            (speed, float(2.0)),
            (fs, float(1.0)),
        ])
    );
    assert_eq!(
        stored(d3),
        BTreeMap::from([(radius, float(7.0)), (fs, float(1.5))])
    );
    let st = store.stats();
    assert_eq!(
        (st.templates, st.stored_mirrors, st.derived_mirrors),
        (3, 3, 6)
    );
    assert_eq!(store.stored(d2).map(StoredTemplate::varying), Some(3));

    // Lookups on T2.
    assert_eq!(
        store.lookup(d2, f.p("AI_PerceptionRadius_ForwardSource")),
        Some(&float(12.0))
    );
    assert_eq!(store.lookup(d2, fs), Some(&float(1.0)));
    assert_eq!(store.lookup(d2, speed), Some(&float(2.0)));
    assert_eq!(store.lookup(d2, f.p("Health_MaxLevel")), Some(&int(10)));
    // On T1 and T3 the base answers absent (the default applies), never the mirror.
    let defaults: HashMap<u32, PropertyValue> = HashMap::from([(speed, float(1.25))]);
    for d in [d1, d3] {
        assert_eq!(store.lookup(d, speed), None);
        assert_eq!(
            store.lookup_or_default(d, speed, &defaults),
            Some(&float(1.25))
        );
    }
    assert_eq!(store.lookup(d1, fs), Some(&float(1.0)));
    assert_eq!(store.lookup(d3, fs), Some(&float(1.5)));
    // A mirror the set does not hold is absent, not computed.
    assert_eq!(store.lookup(d1, f.p("Health_MaxLevel_Cached")), None);
    // The view answers like the store.
    let view = store.template(d2).expect("view");
    assert_eq!(view.float(f.pids.template.perception_radius), Some(12.0));
    assert_eq!(
        view.did_nonnull(f.pids.template.related_quest),
        Some(Did(0x7000_1234))
    );

    // CR13: every set comes back exactly.
    for (d, _, s) in records {
        let mut want = s.entries.clone();
        want.sort_by_key(|e| e.pid);
        assert_eq!(store.reconstruct(d), Some(want), "{d}");
    }
}

#[test]
fn template_store_handles_sets_outside_the_classification() {
    let f = Fixture::new();
    let mirrors = MirrorTable::from_labels(&f.labels);
    let a = set(vec![
        f.e("Health_MaxLevel", int(10)),
        f.e("Combat_CanBlock", PropertyValue::Bool(true)),
    ]);
    let mut b = ClassConstantsBuilder::new(&mirrors);
    b.add(1, &a);
    b.add(1, &a);
    let constants = b.finish();
    let mut store = TemplateStore::new(mirrors.clone(), constants);
    // A set that lacks one constant and holds another with a different value.
    let odd = set(vec![f.e("Health_MaxLevel", int(11))]);
    store.insert(Did(0x7000_0009), 1, &odd);
    assert_eq!(
        store.lookup(Did(0x7000_0009), f.p("Health_MaxLevel")),
        Some(&int(11))
    );
    assert_eq!(store.lookup(Did(0x7000_0009), f.p("Combat_CanBlock")), None);
    let mut want = odd.entries.clone();
    want.sort_by_key(|e| e.pid);
    assert_eq!(store.reconstruct(Did(0x7000_0009)), Some(want));
    // An unknown class stores everything.
    store.insert(Did(0x7000_000A), 99, &a);
    assert_eq!(
        store.stored(Did(0x7000_000A)).map(|t| t.stored().len()),
        Some(2)
    );
    assert_eq!(store.len(), 2);
}

// ---------------------------------------------------------------------------------------------
// §4 Template view
// ---------------------------------------------------------------------------------------------

#[test]
fn template_view_reads_every_group() {
    let f = Fixture::new();
    let name = StringInfo {
        source: StringSource::Table {
            token: 5,
            table: Did(0x2500_0001),
        },
        args: None,
    };
    let s = set(vec![
        f.e("Name", PropertyValue::StringInfo(Box::new(name.clone()))),
        f.e("WeenieType", en(1)),
        f.e("Agent_Species", en(3)),
        f.e("Agent_Genus", PropertyValue::Bitfield32(1 << 4)),
        f.e("Agent_Alignment", en(2)),
        f.e("Agent_Classification", did(0x7000_0100)),
        f.e("PhysObj", did(0x4700_0001)),
        f.e(
            "Appearance_ClassList",
            array(vec![f.e(
                "Synthetic_Struct",
                structure(vec![
                    f.e("Appearance_Key", en(9)),
                    f.e("Appearance_AprFile", did(0x2000_0001)),
                ]),
            )]),
        ),
        f.e(
            "AI_ActionTable_Array",
            array(vec![
                f.e(
                    "Synthetic_Struct",
                    structure(vec![
                        f.e("AI_ActionTable", did(0x7000_0200)),
                        f.e("AI_ActionTable_MinLevel", int(-1)),
                        f.e("AI_ActionTable_MaxLevel", int(30)),
                    ]),
                ),
                f.e(
                    "Synthetic_Struct",
                    structure(vec![f.e("AI_ActionTable", did(0))]),
                ),
                f.e(
                    "Synthetic_Struct",
                    structure(vec![f.e("AI_ActionTable", did(0x7000_0300))]),
                ),
            ]),
        ),
        f.e("Attitude_Table", did(0x7000_0400)),
        f.e("AI_PerceptionRadius", float(7.0)),
        f.e(
            "AI_Hints",
            PropertyValue::BitfieldAny(tessera_formats::BitfieldAny {
                bits: 16,
                bytes: vec![0b0000_0101, 0b1000_0000],
            }),
        ),
        f.e("Combat_CanBlock", PropertyValue::Bool(true)),
        f.e(
            "CombatState_CurrentStateImmunities",
            PropertyValue::Bitfield32((1 << 2) | (1 << 14)),
        ),
        f.e("LootGen_TrophyList_Override", did(0x7000_0500)),
        f.e("LootGen_TreasureList_Override", did(0)),
        f.e(
            "QuestDispenser_ActiveQuests",
            array(vec![f.e(
                "Synthetic_Struct",
                structure(vec![
                    f.e("QuestDispenser_Quest", did(0x7000_0600)),
                    f.e(
                        "QuestDispenser_RoleName",
                        PropertyValue::String("role".into()),
                    ),
                ]),
            )]),
        ),
        f.e(
            "Effect_MonsterStartupEffect_Array",
            array(vec![f.e(
                "Synthetic_Struct",
                structure(vec![
                    f.e("Effect_StartupEffectID", did(0x7000_0700)),
                    f.e("Effect_StartupEffectSpellcraft", float(1.5)),
                ]),
            )]),
        ),
        f.e(
            "Entity_AdditionalScriptTables",
            array(vec![
                f.e("Synthetic_Element", did(0x0700_0001)),
                f.e("Synthetic_Element", did(0)),
            ]),
        ),
    ]);
    let t = CreatureTemplate::read(&s, &f.pids.template);
    assert_eq!(t.identity.name, Some(name));
    assert_eq!(t.identity.weenie_type, Some(1));
    assert_eq!(t.identity.genus_bit(), Some(4));
    assert_eq!(t.identity.classification, Some(Did(0x7000_0100)));
    assert_eq!(t.presentation.phys_obj, Some(Did(0x4700_0001)));
    assert_eq!(
        t.presentation.appearances,
        vec![AppearanceKey {
            key: Some(9),
            apr_file: Some(Did(0x2000_0001)),
            modifier: None
        }]
    );
    assert_eq!(t.ai.action_tables.len(), 3);
    assert_eq!(t.ai.action_tables[0].min_bound(), None);
    assert_eq!(t.ai.action_tables[0].max_bound(), Some(30));
    let valid: Vec<Did> = t
        .valid_action_tables(|d| d != Did(0x7000_0300))
        .map(|r| r.table)
        .collect();
    assert_eq!(valid, vec![Did(0x7000_0200)]);
    assert_eq!(t.ai.hints, Some(vec![0, 2, 15]));
    assert_eq!(t.ai.perception_radius, Some(7.0));
    assert_eq!(t.combat.can_block, Some(true));
    assert_eq!(
        immunity_indicators(t.combat.state_immunities.unwrap_or(0), false),
        [true, false, false]
    );
    assert_eq!(t.loot.trophy_list, Some(Did(0x7000_0500)));
    assert_eq!(t.loot.treasure_list, None);
    assert_eq!(t.roles.active_quests.len(), 1);
    assert_eq!(t.roles.active_quests[0].role_name.as_deref(), Some("role"));
    assert_eq!(t.effects.startup_effects[0].spellcraft, Some(1.5));
    assert_eq!(t.effects.script_tables, vec![Did(0x0700_0001)]);
    // §4.8: no level in a template → shown as 1.
    assert_eq!(displayed_level(&s, f.pids.template.advancement_level), 1);
    let lvl = set(vec![f.e("Advancement_Level", int(40))]);
    assert_eq!(displayed_level(&lvl, f.pids.template.advancement_level), 40);
    let lvl = set(vec![f.e("Advancement_Level", float(40.0))]);
    assert_eq!(displayed_level(&lvl, f.pids.template.advancement_level), 1);

    // §5.4: a classification record that agrees, and one that does not.
    let c = Classification {
        species: Some(3),
        genus: Some(1 << 4),
        alignment: Some(2),
    };
    assert!(c.disagreements(&t.identity).is_empty());
    let c = Classification {
        alignment: Some(1),
        ..c
    };
    assert_eq!(c.disagreements(&t.identity), vec!["alignment"]);
}

// ---------------------------------------------------------------------------------------------
// §5 AI records
// ---------------------------------------------------------------------------------------------

#[test]
fn ai_records() {
    let f = Fixture::new();
    let a = &f.pids.ai;

    // §5.1 action table.
    let s = set(vec![f.e(
        "AIActionTable_Actions",
        array(vec![f.e(
            "Synthetic_Struct",
            structure(vec![
                f.e(
                    "AIAction_Chain",
                    array(vec![
                        f.e(
                            "Synthetic_Struct",
                            structure(vec![f.e("AIAction_Skill", did(0x7000_0827))]),
                        ),
                        f.e(
                            "Synthetic_Struct",
                            structure(vec![
                                f.e("AIAction_Skill", did(0x7000_0828)),
                                f.e("AIAction_Recovery", float(2.0)),
                            ]),
                        ),
                    ]),
                ),
                f.e("AIAction_Probability", en(4)),
                f.e("AIAction_Cooldown", float(-1.0)),
                f.e("AIAction_RequiredHints", PropertyValue::Bitfield64(1 << 40)),
            ]),
        )]),
    )]);
    assert_eq!(
        CreatureRecordKind::of_set(&s, &f.pids),
        Some(CreatureRecordKind::ActionTable)
    );
    let t = ActionTable::read(&s, a).expect("action table");
    assert_eq!(t.actions.len(), 1);
    let act = &t.actions[0];
    assert_eq!(act.chain.len(), 2);
    assert_eq!(act.chain[1].recovery, Some(2.0));
    assert_eq!(act.probability, Some(4));
    assert_eq!(act.required_hints, Some(vec![40]));
    assert!(matches!(
        ActionTable::read(&set(vec![]), a),
        Err(FormatError::MissingProperty { .. })
    ));
    assert!(matches!(
        ActionTable::read(&set(vec![f.e("AIActionTable_Actions", int(1))]), a),
        Err(FormatError::PropertyTypeMismatch { .. })
    ));

    // §5.2 probability table.
    let pt = |k: u32, v: f32| {
        f.e(
            "Synthetic_Struct",
            structure(vec![
                f.e("AIActionProbability_Type", en(k)),
                f.e("AIActionProbability_Value", float(v)),
            ]),
        )
    };
    let s = set(vec![f.e(
        "AIActionProbability_Array",
        array(vec![pt(1, 0.0), pt(4, 25.0), pt(9, -1.0), pt(4, 99.0)]),
    )]);
    let p = ProbabilityTable::read(&s, a).expect("probability table");
    assert_eq!(p.percentage(4), Some(25.0));
    assert_eq!(p.percentage(9), Some(-1.0));
    assert_eq!(p.percentage(0), None);
    assert_eq!(p.entries().len(), 3);

    // §5.3 attitude table with filter exceptions.
    let filter = |ty: u32, default: bool, alignment: Option<u32>| {
        let mut fields = vec![f.e("ClassificationFilter_Filter_Type", en(ty))];
        if default {
            fields.push(f.e(
                "ClassificationFilter_Filter_Default",
                PropertyValue::Bool(true),
            ));
        }
        if let Some(al) = alignment {
            fields.push(f.e(
                "ClassificationFilter_FilterExceptionList",
                array(vec![f.e(
                    "Synthetic_Struct",
                    structure(vec![
                        f.e("ClassificationFilter_Synthetic_Alignment", en(al)),
                        f.e(
                            "ClassificationFilter_Synthetic_Genus",
                            PropertyValue::Bitfield32(2),
                        ),
                    ]),
                )]),
            ));
        }
        f.e("Synthetic_Struct", structure(fields))
    };
    let s = set(vec![
        f.e(
            "Attitude_FriendlyFilterList",
            array(vec![filter(2, false, None)]),
        ),
        f.e(
            "Attitude_AggressiveFilterList",
            array(vec![filter(1, true, Some(3)), filter(4, true, None)]),
        ),
    ]);
    assert_eq!(
        CreatureRecordKind::of_set(&s, &f.pids),
        Some(CreatureRecordKind::AttitudeTable)
    );
    let at = AttitudeTable::read(&s, a, &f.pids.exceptions);
    let friendly = at.friendly.expect("friendly");
    assert_eq!(friendly.len(), 1);
    assert_eq!(friendly[0].default, None);
    let aggressive = at.aggressive.expect("aggressive");
    assert_eq!(aggressive.len(), 2);
    assert_eq!(
        aggressive[0].exceptions,
        vec![FilterException {
            alignment: Some(3),
            genus: Some(2),
            species: None,
            subspecies: None
        }]
    );
    // An empty attitude table reads as empty; its kind comes from its class.
    assert_eq!(
        AttitudeTable::read(&set(vec![]), a, &f.pids.exceptions),
        AttitudeTable::default()
    );
    let mut kinds = RecordKindIndex::new();
    kinds.add(884, CreatureRecordKind::AttitudeTable);
    assert_eq!(
        kinds.kind_of_class(884),
        Some(CreatureRecordKind::AttitudeTable)
    );
    assert_eq!(kinds.kind_of_class(1), None);
    kinds.add(884, CreatureRecordKind::Formation);
    assert_eq!(kinds.kind_of_class(884), None);
    assert_eq!(
        kinds.classes_of(CreatureRecordKind::Formation),
        vec![(884, 1)]
    );

    let s = set(vec![f.e(
        "ClassificationFilter_FilterList",
        array(vec![filter(1, true, None)]),
    )]);
    assert_eq!(
        CreatureRecordKind::of_set(&s, &f.pids),
        Some(CreatureRecordKind::FilterList)
    );
    let fl = ClassificationFilterList::read(&s, a, &f.pids.exceptions).expect("filter list");
    assert_eq!(fl.filters.len(), 1);

    // §5.4 classification.
    let s = set(vec![
        f.e("Classification_Species", en(3)),
        f.e("Classification_Genus", PropertyValue::Bitfield32(8)),
        f.e("Classification_Alignment", en(1)),
    ]);
    assert_eq!(
        CreatureRecordKind::of_set(&s, &f.pids),
        Some(CreatureRecordKind::Classification)
    );
    let c = Classification::read(&s, a);
    assert_eq!(
        (c.species, c.genus, c.alignment),
        (Some(3), Some(8), Some(1))
    );

    // §5.5 colour events with a top-level event (D7).
    let event = |ty: u32, sound: bool| {
        let mut line = vec![f.e(
            "AIColorEventTable_Event_String",
            PropertyValue::StringInfo(Box::new(StringInfo {
                source: StringSource::Table {
                    token: ty,
                    table: Did(0x2500_0002),
                },
                args: None,
            })),
        )];
        if sound {
            line.push(f.e("AIColorEventTable_Event_SoundID", did(0x2A00_0001)));
        }
        structure(vec![
            f.e("AIColorEventTable_Event_Type", en(ty)),
            f.e("AIColorEventTable_Event_Probability", float(0.5)),
            f.e("AIColorEventTable_Event_SoundChannel", int(4)),
            f.e(
                "AIColorEventTable_Event_Data",
                array(vec![f.e("Synthetic_Struct", structure(line))]),
            ),
        ])
    };
    let s = set(vec![
        f.e(
            "AIColorEventTable_Events",
            array(
                (1..=3)
                    .map(|k| f.e("AIColorEventTable_Event", event(k, false)))
                    .collect(),
            ),
        ),
        f.e("AIColorEventTable_Event", event(2, true)),
    ]);
    assert_eq!(
        CreatureRecordKind::of_set(&s, &f.pids),
        Some(CreatureRecordKind::ColorEventTable)
    );
    let ce = ColorEventTable::read(&s, a).expect("colour events");
    assert_eq!(ce.events.len(), 3);
    assert!(ce.covers_exactly(1..=3));
    assert!(!ce.covers_exactly(1..=4));
    let top = ce.top_level.as_ref().expect("top-level event kept");
    assert_eq!(top.event_type, Some(2));
    assert_eq!(top.lines[0].sound, Some(Did(0x2A00_0001)));
    assert_eq!(ce.all_events().count(), 4);
    assert_ne!(
        ce.events[1], *top,
        "the top-level event does not replace the array event"
    );

    // §5.6 formation: followers anywhere under Formation_Group.
    let s = set(vec![f.e(
        "Formation_Group",
        structure(vec![
            f.e("Formation_Follower", did(0x7000_1723)),
            f.e(
                "Synthetic_Element",
                array(vec![
                    f.e("Formation_Follower", did(0x7000_1724)),
                    f.e("Formation_Follower", did(0)),
                ]),
            ),
        ]),
    )]);
    assert_eq!(
        CreatureRecordKind::of_set(&s, &f.pids),
        Some(CreatureRecordKind::Formation)
    );
    let fm = Formation::read(&s, a).expect("formation");
    assert_eq!(fm.followers, vec![Did(0x7000_1723), Did(0x7000_1724)]);

    // §5.7 conjunction.
    let contribution = |skill: u32| {
        f.e(
            "Conjunction_Role_ContributionToActionMap",
            structure(vec![
                f.e("Conjunction_Role_ActionSkill", did(skill)),
                f.e("Conjunction_PvP_Role_ActionSkill", did(0)),
                f.e("Conjunction_Role_ContributionSkill", did(skill + 1)),
            ]),
        )
    };
    let s = set(vec![
        f.e("Conjunction_Category", en(2)),
        f.e("Conjunction_Icon_Small", did(0x4100_0001)),
        f.e(
            "Conjunction_RoleList",
            array(vec![
                f.e(
                    "Synthetic_Struct",
                    structure(vec![f.e(
                        "Synthetic_Element",
                        array(vec![contribution(0x7000_0010), contribution(0x7000_0020)]),
                    )]),
                ),
                f.e("Synthetic_Struct", structure(vec![])),
            ]),
        ),
    ]);
    assert_eq!(
        CreatureRecordKind::of_set(&s, &f.pids),
        Some(CreatureRecordKind::Conjunction)
    );
    let cj = Conjunction::read(&s, a);
    assert_eq!(cj.roles.len(), 2);
    assert_eq!(cj.roles[0].contributions.len(), 2);
    assert_eq!(
        cj.roles[0].contributions[1].contribution_skill,
        Some(Did(0x7000_0021))
    );
    assert_eq!(cj.roles[0].contributions[0].pvp_action_skill, Some(Did(0)));
    assert_eq!(cj.icons, [Some(Did(0x4100_0001)), None, None]);

    // §5.8 puppet package.
    let s = set(vec![f.e("AIPuppet_TargetRequestQueue_Type", en(2))]);
    assert_eq!(
        CreatureRecordKind::of_set(&s, &f.pids),
        Some(CreatureRecordKind::PuppetPackage)
    );
    assert_eq!(PuppetPackage::read(&s, a).queue_type, Some(2));
    assert_eq!(CreatureRecordKind::of_set(&set(vec![]), &f.pids), None);
}

// ---------------------------------------------------------------------------------------------
// §6.5, §12.2 Generator profiles
// ---------------------------------------------------------------------------------------------

const P1: Did = Did(0x7000_0B01);
const P2: Did = Did(0x7000_0B02);
const P3: Did = Did(0x7000_0B03);

struct ProfileBuilder<'f>(&'f Fixture);

impl ProfileBuilder<'_> {
    fn e(&self, name: &str, v: PropertyValue) -> PropertyEntry {
        self.0.e(name, v)
    }
    fn entry(&self, fields: Vec<PropertyEntry>) -> PropertyEntry {
        self.e("GeneratorProfile_Entry", structure(fields))
    }
    fn wsl(&self, d: u32) -> PropertyEntry {
        self.e("GeneratorProfile_WSLEntity", did(d))
    }
    fn def(&self, d: Did) -> PropertyEntry {
        self.e("GeneratorProfile_ProfileDefinition", did(d.0))
    }
    fn weight(&self, w: i32) -> PropertyEntry {
        self.e("GeneratorProfile_Weight", int(w))
    }
    fn probability(&self, p: i32) -> PropertyEntry {
        self.e("GeneratorProfile_Probability", int(p))
    }
    fn repeat(&self, min: i32, max: i32) -> PropertyEntry {
        self.e(
            "GeneratorProfile_Repeat",
            structure(vec![
                self.e("GeneratorProfile_Synthetic_RepeatMinTimes", int(min)),
                self.e("GeneratorProfile_Synthetic_RepeatMaxTimes", int(max)),
            ]),
        )
    }
    fn random(&self, min: f32, max: f32) -> PropertyEntry {
        self.e(
            "GeneratorProfile_RandomPlacement",
            structure(vec![
                self.e(
                    "GeneratorProfile_Synthetic_RandomPlacementMinDist",
                    float(min),
                ),
                self.e(
                    "GeneratorProfile_Synthetic_RandomPlacementMaxDist",
                    float(max),
                ),
            ]),
        )
    }
    fn exact(&self, x: f32, y: f32, z: f32) -> PropertyEntry {
        self.e(
            "GeneratorProfile_ExactPlacement",
            structure(vec![
                self.e("GeneratorProfile_Synthetic_ExactPlacementX", float(x)),
                self.e("GeneratorProfile_Synthetic_ExactPlacementY", float(y)),
                self.e("GeneratorProfile_Synthetic_ExactPlacementZ", float(z)),
            ]),
        )
    }
    fn one_of(&self, elements: Vec<PropertyEntry>) -> PropertyEntry {
        self.e("GeneratorProfile_OneOf", array(elements))
    }
    fn pse(&self, set_id: u32, fields: Vec<PropertyEntry>) -> PropertyEntry {
        let mut v = vec![self.e("GeneratorProfile_PositionSetID", en(set_id))];
        v.extend(fields);
        self.e("GeneratorProfile_PositionSetEntry", structure(v))
    }
    fn profile(&self, elements: Vec<PropertyEntry>) -> GeneratorProfile {
        let s = set(vec![self.e("GeneratorProfile_Profile", array(elements))]);
        assert_eq!(
            CreatureRecordKind::of_set(&s, &self.0.pids),
            Some(CreatureRecordKind::GeneratorProfile)
        );
        GeneratorProfile::read(&s, &self.0.pids).expect("profile")
    }
}

/// P1, P2 and P3 of §12.2.
fn example_profiles(f: &Fixture) -> BTreeMap<Did, GeneratorProfile> {
    let b = ProfileBuilder(f);
    let p1 = b.profile(vec![
        b.e("GeneratorProfile_RegenType", en(4)),
        b.one_of(vec![
            b.entry(vec![b.wsl(0x7000_0A01), b.weight(3)]),
            b.entry(vec![b.wsl(0x7000_0A02), b.weight(1)]),
        ]),
        b.pse(12, vec![b.entry(vec![b.def(P2), b.probability(500)])]),
    ]);
    let p2 = b.profile(vec![b.entry(vec![b.wsl(0x7000_0A03)])]);
    let p3 = b.profile(vec![
        b.one_of(vec![
            b.pse(
                12,
                vec![
                    b.weight(1),
                    b.one_of(vec![
                        b.repeat(2, 2),
                        b.entry(vec![b.wsl(0x7000_0A04), b.weight(1)]),
                        b.entry(vec![b.wsl(0x7000_0A05), b.weight(3)]),
                    ]),
                ],
            ),
            b.pse(
                12,
                vec![
                    b.weight(1),
                    b.one_of(vec![
                        b.repeat(1, 1),
                        b.entry(vec![b.wsl(0x7000_0A06), b.weight(1)]),
                    ]),
                ],
            ),
            b.pse(
                40,
                vec![
                    b.weight(2),
                    b.one_of(vec![
                        b.repeat(1, 1),
                        b.entry(vec![b.wsl(0x7000_0A07), b.weight(1)]),
                    ]),
                ],
            ),
        ]),
        b.one_of(vec![
            b.random(2.0, 5.0),
            b.entry(vec![b.wsl(0x7000_0A08), b.weight(1)]),
            b.entry(vec![b.def(P2), b.weight(1)]),
        ]),
    ]);
    BTreeMap::from([(P1, p1), (P2, p2), (P3, p3)])
}

/// A scripted random source: returns the queued values in order (panics when exhausted, so
/// a test also checks how many draws an evaluation makes).
struct Script(Vec<u32>, Vec<u32>);

impl Script {
    fn new(values: &[u32]) -> Self {
        Self(values.iter().rev().copied().collect(), Vec::new())
    }
}

impl SpawnRandom for Script {
    fn below(&mut self, n: u32) -> u32 {
        self.1.push(n);
        let v = self.0.pop().expect("more random draws than scripted");
        assert!(v < n, "scripted {v} not below {n}");
        v
    }
}

/// A small deterministic generator for frequency checks.
struct XorShift(u64);

impl SpawnRandom for XorShift {
    fn below(&mut self, n: u32) -> u32 {
        self.0 ^= self.0 << 13;
        self.0 ^= self.0 >> 7;
        self.0 ^= self.0 << 17;
        u32::try_from(self.0 % u64::from(n)).unwrap_or(0)
    }
}

fn objects(e: &Evaluation) -> Vec<u32> {
    e.objects.iter().map(|o| o.object.0).collect()
}

#[test]
fn worked_example_12_2_case_1() {
    let f = Fixture::new();
    let src = example_profiles(&f);
    let p1 = &src[&P1];
    assert_eq!(p1.regen_type(), Some(4));
    assert!(p1.issues().is_empty(), "{:?}", p1.issues());

    // S2 (set 40): A01 (weight 3) and A02 (weight 1); the PositionSetEntry is skipped.
    let r = resolve_candidates(&src, P1, Some(40));
    let got: Vec<(u32, Option<i32>, Option<i32>)> = r
        .candidates
        .iter()
        .map(|c| (c.object.0, c.weight, c.probability))
        .collect();
    assert_eq!(
        got,
        vec![(0x7000_0A01, Some(3), None), (0x7000_0A02, Some(1), None)]
    );
    assert_eq!(r.max_depth, 0);

    // S1 (set 12): the same two plus A03 with 500 ‰ through P2.
    let r = resolve_candidates(&src, P1, Some(12));
    assert_eq!(r.candidates.len(), 3);
    let a03 = r.candidates[2];
    assert_eq!(a03.object, Did(0x7000_0A03));
    assert_eq!(a03.probability, Some(500));
    assert!(a03.scoped);
    assert_eq!((a03.depth, a03.profile), (1, P2));
    assert_eq!(r.max_depth, 1);
    assert!(r.missing_profiles.is_empty() && r.cycles == 0 && !r.truncated);
    // Without a position set every entry is followed.
    assert_eq!(resolve_candidates(&src, P1, None).objects().len(), 3);

    // D3 at S1: one draw below 4 (weights 3 + 1), then the 500 ‰ test.
    let opts = EvalOptions::default();
    let mut rng = Script::new(&[2, 499]);
    let e = evaluate_profile(&src, P1, 12, &mut rng, opts);
    assert_eq!(objects(&e), vec![0x7000_0A01, 0x7000_0A03]);
    assert_eq!(rng.1, vec![4, 1000]);
    let mut rng = Script::new(&[3, 500]);
    assert_eq!(
        objects(&evaluate_profile(&src, P1, 12, &mut rng, opts)),
        vec![0x7000_0A02]
    );
    // At S2 only the OneOf draws.
    let mut rng = Script::new(&[0]);
    assert_eq!(
        objects(&evaluate_profile(&src, P1, 40, &mut rng, opts)),
        vec![0x7000_0A01]
    );

    // Frequencies: A01 with chance 3/4, A03 with chance 1/2.
    let mut rng = XorShift(0x9E37_79B9_7F4A_7C15);
    let n = 40_000;
    let (mut a01, mut a03) = (0, 0);
    for _ in 0..n {
        let o = objects(&evaluate_profile(&src, P1, 12, &mut rng, opts));
        a01 += usize::from(o.contains(&0x7000_0A01));
        a03 += usize::from(o.contains(&0x7000_0A03));
    }
    let share = |k: usize| k as f64 / f64::from(n);
    assert!((share(a01) - 0.75).abs() < 0.015, "{}", share(a01));
    assert!((share(a03) - 0.5).abs() < 0.015, "{}", share(a03));
}

#[test]
fn worked_example_12_2_case_2() {
    let f = Fixture::new();
    let src = example_profiles(&f);
    let p3 = &src[&P3];
    // P3's first OneOf names two position sets, which CR14 reports (this install has none).
    assert_eq!(p3.issues(), vec![ProfileIssue::SeveralPositionSets]);
    let opts = EvalOptions::default();
    let random = Some(Placement::Random {
        min_dist: 2.0,
        max_dist: 5.0,
    });

    // S1 (set 12): first OneOf draws below 2 (the set-40 choice is removed before the draw).
    // Choice 0: two draws below 4 (A04 weight 1, A05 weight 3). Second OneOf: below 2.
    let mut rng = Script::new(&[0, 0, 3, 1]);
    let e = evaluate_profile(&src, P3, 12, &mut rng, opts);
    assert_eq!(rng.1, vec![2, 4, 4, 2]);
    assert_eq!(objects(&e), vec![0x7000_0A04, 0x7000_0A05, 0x7000_0A03]);
    assert_eq!(e.objects[0].placement, None);
    assert_eq!(e.objects[1].placement, None);
    // The leading RandomPlacement applies to what P2 produces too.
    assert_eq!(e.objects[2].placement, random);

    let mut rng = Script::new(&[1, 0]);
    let e = evaluate_profile(&src, P3, 12, &mut rng, opts);
    assert_eq!(objects(&e), vec![0x7000_0A06, 0x7000_0A08]);
    assert_eq!(e.objects[1].placement, random);

    // S2 (set 40): the only eligible choice is taken with chance 1 (not 2/4) and without a
    // draw; its own OneOf has one choice too. Only the second OneOf draws.
    let mut rng = Script::new(&[1]);
    let e = evaluate_profile(&src, P3, 40, &mut rng, opts);
    assert_eq!(rng.1, vec![2]);
    assert_eq!(objects(&e), vec![0x7000_0A07, 0x7000_0A03]);

    // Set 99: the first OneOf produces nothing and draws nothing.
    let mut rng = Script::new(&[0]);
    let e = evaluate_profile(&src, P3, 99, &mut rng, opts);
    assert_eq!(rng.1, vec![2]);
    assert_eq!(objects(&e), vec![0x7000_0A08]);

    // Static resolution at S1 lists A04, A05, A06, A08 and A03 (through P2).
    let r = resolve_candidates(&src, P3, Some(12));
    let objs: Vec<u32> = r.candidates.iter().map(|c| c.object.0).collect();
    assert_eq!(
        objs,
        vec![
            0x7000_0A04,
            0x7000_0A05,
            0x7000_0A06,
            0x7000_0A08,
            0x7000_0A03
        ]
    );
    assert_eq!(
        r.candidates[0].repeat,
        Some(Repeat {
            min: Some(2),
            max: Some(2)
        })
    );
    assert_eq!(r.candidates[4].placement, random);

    // Frequencies at S1: 3 objects with chance 1/2; two A05 with 9/16 of the first choice.
    let mut rng = XorShift(0x2545_F491_4F6C_DD1D);
    let n = 40_000;
    let (mut three, mut first, mut two_a05) = (0, 0, 0);
    for _ in 0..n {
        let o = objects(&evaluate_profile(&src, P3, 12, &mut rng, opts));
        three += usize::from(o.len() == 3);
        if o.len() == 3 {
            first += 1;
            two_a05 += usize::from(o.iter().filter(|&&d| d == 0x7000_0A05).count() == 2);
        }
    }
    let share = |k: usize, of: usize| k as f64 / of as f64;
    assert!((share(three, n) - 0.5).abs() < 0.015);
    assert!((share(two_a05, first) - 9.0 / 16.0).abs() < 0.02);
}

#[test]
fn evaluation_limits_and_removed_entries() {
    let f = Fixture::new();
    let b = ProfileBuilder(&f);
    let opts = EvalOptions::default();
    let big = Did(0x7000_0C01);
    let self_ref = Did(0x7000_0C02);
    let removed = Did(0x7000_0C03);
    let chain_start = 0x7000_0D00u32;
    let mut src: BTreeMap<Did, GeneratorProfile> = BTreeMap::new();
    src.insert(
        big,
        b.profile(vec![b.entry(vec![
            b.wsl(0x7000_0A01),
            b.repeat(300, 300),
            b.exact(1.0, -2.0, 0.5),
        ])]),
    );
    src.insert(
        self_ref,
        b.profile(vec![
            b.entry(vec![b.def(self_ref)]),
            b.entry(vec![b.wsl(0x7000_0A02)]),
        ]),
    );
    src.insert(
        removed,
        b.profile(vec![
            b.one_of(vec![
                b.entry(vec![b.wsl(0x7000_0A03), b.weight(0)]),
                b.entry(vec![b.wsl(0x7000_0A04), b.weight(2)]),
            ]),
            b.entry(vec![b.wsl(0x7000_0A05), b.probability(0)]),
            b.entry(vec![b.def(Did(0x7000_0EEE))]),
        ]),
    );
    for i in 0..12 {
        src.insert(
            Did(chain_start + i),
            b.profile(vec![b.entry(vec![b.def(Did(chain_start + i + 1))])]),
        );
    }

    // At most 256 objects; the rest dropped. The exact placement is carried.
    let mut rng = Script::new(&[]);
    let e = evaluate_profile(&src, big, 1, &mut rng, opts);
    assert_eq!((e.objects.len(), e.dropped), (256, 44));
    assert_eq!(
        e.objects[0].placement,
        Some(Placement::Exact {
            offset: [1.0, -2.0, 0.5]
        })
    );

    // A profile on its own path is not expanded again.
    let mut rng = Script::new(&[]);
    let e = evaluate_profile(&src, self_ref, 1, &mut rng, opts);
    assert_eq!((objects(&e), e.cycles), (vec![0x7000_0A02], 1));
    assert_eq!(resolve_candidates(&src, self_ref, Some(1)).cycles, 1);

    // Weight and probability ≤ 0 remove entries before the draw; missing profiles reported.
    // The one choice left is taken without a draw.
    let mut rng = Script::new(&[]);
    let e = evaluate_profile(&src, removed, 1, &mut rng, opts);
    assert_eq!(objects(&e), vec![0x7000_0A04]);
    assert!(rng.1.is_empty());
    assert_eq!(e.missing_profiles, vec![Did(0x7000_0EEE)]);
    let r = resolve_candidates(&src, removed, None);
    assert_eq!(r.removed, 2);
    assert_eq!(
        r.objects().into_iter().collect::<Vec<_>>(),
        vec![Did(0x7000_0A04)]
    );

    // Nesting deeper than 8 stops.
    let r = resolve_candidates(&src, Did(chain_start), Some(1));
    assert_eq!(r.depth_limited, 1);
    assert_eq!(r.max_depth, MAX_PROFILE_DEPTH);
    let mut rng = Script::new(&[]);
    assert_eq!(
        evaluate_profile(&src, Did(chain_start), 1, &mut rng, opts).depth_limited,
        1
    );
}

#[test]
fn profile_grammar_checks() {
    let f = Fixture::new();
    let b = ProfileBuilder(&f);
    // CR9: a modifier after element 0; an entry in a OneOf without weight.
    let p = b.profile(vec![
        b.one_of(vec![b.entry(vec![b.wsl(0x7000_0A01)]), b.repeat(1, 1)]),
    ]);
    assert_eq!(
        p.issues(),
        vec![ProfileIssue::StrayModifier, ProfileIssue::ChoiceWeight]
    );
    // CR10: bounds out of order.
    let p = b.profile(vec![
        b.one_of(vec![
            b.repeat(3, 2),
            b.entry(vec![b.wsl(0x7000_0A01), b.weight(1)]),
        ]),
        b.one_of(vec![
            b.random(5.0, 2.0),
            b.entry(vec![b.wsl(0x7000_0A01), b.weight(1)]),
        ]),
    ]);
    assert_eq!(
        p.issues(),
        vec![ProfileIssue::RepeatBounds, ProfileIssue::PlacementBounds]
    );
    // CR14: mixed choices; a set choice without a fixed repeat.
    let p = b.profile(vec![b.one_of(vec![
        b.entry(vec![b.wsl(0x7000_0A01), b.weight(1)]),
        b.pse(
            3,
            vec![
                b.weight(1),
                b.one_of(vec![
                    b.repeat(1, 2),
                    b.entry(vec![b.wsl(0x7000_0A01), b.weight(1)]),
                ]),
            ],
        ),
    ])]);
    assert_eq!(
        p.issues(),
        vec![ProfileIssue::MixedOneOf, ProfileIssue::ChoiceRepeat]
    );
    // An entry with two targets, and a position-set entry without a body.
    let p = b.profile(vec![
        b.entry(vec![b.wsl(0x7000_0A01), b.def(P2)]),
        b.pse(3, vec![]),
        b.e("Synthetic_Element", int(1)),
    ]);
    assert_eq!(
        p.issues(),
        vec![ProfileIssue::EntryTargets(2), ProfileIssue::PositionSetBody]
    );
    assert!(matches!(p.elements[2], ProfileElement::Other { .. }));
    // A known element with the wrong type is an error.
    let s = set(vec![f.e(
        "GeneratorProfile_Profile",
        array(vec![f.e("GeneratorProfile_Entry", int(1))]),
    )]);
    assert!(matches!(
        GeneratorProfile::read(&s, &f.pids),
        Err(FormatError::PropertyTypeMismatch { .. })
    ));
    // Deep OneOf / PositionSetEntry nesting stops at the tree limit.
    let mut inner = b.one_of(vec![b.entry(vec![b.wsl(0x7000_0A01), b.weight(1)])]);
    for _ in 0..MAX_PROFILE_TREE_DEPTH {
        inner = b.one_of(vec![b.pse(1, vec![b.weight(1), inner])]);
    }
    let s = set(vec![f.e("GeneratorProfile_Profile", array(vec![inner]))]);
    assert!(matches!(
        GeneratorProfile::read(&s, &f.pids),
        Err(FormatError::DepthLimit { .. })
    ));
    // Missing profile array.
    assert!(matches!(
        GeneratorProfile::read(&set(vec![]), &f.pids),
        Err(FormatError::MissingProperty { .. })
    ));
}

// ---------------------------------------------------------------------------------------------
// §6.2–§6.4, §6.9 Landblock spawn data
// ---------------------------------------------------------------------------------------------

fn iid(serial: u32) -> InstanceId {
    InstanceId::new(4, 1, 0x20, 0x30, serial)
}

fn link(t: u32, from: InstanceId, to: InstanceId, props: PropertySet) -> Link {
    Link {
        link_type: t,
        from_iid: from,
        to_iid: to,
        properties: props,
        ..Link::default()
    }
}

#[test]
fn landblock_spawn_index() {
    let f = Fixture::new();
    let sp = &f.pids.spawn;
    let gen_set = set(vec![
        f.e("Generator_ProfileDID", did(P1.0)),
        f.e(
            "Generator_PositionSetLimitArray",
            array(vec![f.e(
                "Synthetic_Struct",
                structure(vec![
                    f.e("Generator_PositionSetID", en(12)),
                    f.e("Generator_PositionSetLimit", int(2)),
                ]),
            )]),
        ),
    ]);
    let (g, g2, s1, s2, s3, obj, pp1, pp2, pp3) = (
        iid(1),
        iid(2),
        iid(10),
        iid(11),
        iid(12),
        iid(20),
        iid(30),
        iid(31),
        iid(32),
    );
    let point = |i: InstanceId, marker: u32, props: Option<PropertySet>| StaticEntity {
        entity_type: Some(3),
        iid: Some(i),
        object: Some(Did(marker)),
        properties: props,
        ..StaticEntity::default()
    };
    let info = LandblockInfo {
        did: Did(0x8021_2030),
        flags: 0,
        size_hint: 0,
        cross_links: None,
        header_indices: None,
        cells: None,
        reserved_a: 0,
        entities: vec![
            point(s1, 0x4700_0001, None),
            point(
                s2,
                0x4700_0002,
                Some(set(vec![f.e("Generator_PositionSetID", en(41))])),
            ),
            point(s3, 0x4700_0003, None),
            StaticEntity {
                entity_type: Some(23),
                iid: Some(pp1),
                ..StaticEntity::default()
            },
        ],
        links: vec![
            link(7, g, s1, set(vec![])),
            link(7, g, s2, set(vec![])),
            link(7, g, s3, set(vec![])),
            link(7, g, InstanceId(0), set(vec![])),
            link(8, obj, g2, set(vec![])),
            link(8, InstanceId(0), g2, set(vec![])),
            link(2, obj, pp1, set(vec![f.e("PatrolLink_Speed", float(1.0))])),
            link(16, pp1, pp2, set(vec![])),
            link(16, pp2, pp3, set(vec![])),
            link(16, pp3, pp1, set(vec![])),
        ],
        properties: set(vec![]),
        reserved_b: 0,
        weenies: TSizeList {
            items: vec![
                Weenie {
                    iid: g,
                    properties: gen_set,
                },
                Weenie {
                    iid: s1,
                    properties: set(vec![f.e("Generator_PositionSetID", en(12))]),
                },
            ],
            ..TSizeList::default()
        },
        trailer: None,
        layout_rule: Default::default(),
    };
    let markers = |d: Did| match d.0 {
        0x4700_0001 => Some(13),
        0x4700_0002 => Some(40),
        0x4700_0003 => Some(40),
        _ => None,
    };
    let spawns = LandblockSpawns::build(&info, sp, markers);
    assert_eq!(spawns.generators.len(), 2);
    let gi = spawns.generator(g).expect("generator");
    assert_eq!(gi.profile, Some(P1));
    assert_eq!(gi.points, vec![s1, s2, s3]);
    assert_eq!(gi.cross_point_links, 1);
    assert!(gi.has_instance_set && !gi.has_static_entity);
    assert_eq!(
        gi.limits,
        vec![PositionSetLimit {
            position_set: Some(12),
            limit: Some(2)
        }]
    );
    let g2i = spawns.generator(g2).expect("generator without profile");
    assert_eq!(g2i.profile, None);
    assert_eq!((g2i.spawned.clone(), g2i.cross_spawn_links), (vec![obj], 1));
    assert!(!g2i.has_instance_set);

    // §6.4 precedence: instance set, then the entity's set, then the marker.
    let ps = |i| spawns.point(i).and_then(|p| p.position_set);
    assert_eq!(ps(s1), Some((12, PositionSetSource::InstanceSet)));
    assert_eq!(ps(s2), Some((41, PositionSetSource::EntitySet)));
    assert_eq!(ps(s3), Some((40, PositionSetSource::Marker)));
    assert_eq!(spawns.points.len(), 3);

    // §6.9 patrol: the start link and the chain back to the start.
    assert_eq!(spawns.patrol_starts.len(), 1);
    let start = spawns.patrol_starts[0];
    assert_eq!(
        (start.link_type, start.start, start.speed),
        (2, pp1, Some(1.0))
    );
    let chain = spawns.paths.follow(start.start);
    assert_eq!(chain.end, PatrolEnd::ReturnsToStart);
    assert_eq!(chain.points, vec![pp1, pp2, pp3]);
    assert_eq!(spawns.paths.len(), 3);

    // Candidates per point (§6.7) with the point's position set.
    let src = example_profiles(&f);
    let at = |i| {
        resolve_candidates(
            &src,
            P1,
            spawns.point(i).and_then(|p| p.position_set).map(|p| p.0),
        )
        .objects()
        .len()
    };
    assert_eq!((at(s1), at(s3)), (3, 2));
}

// ---------------------------------------------------------------------------------------------
// §7, §12.3 Encounter table
// ---------------------------------------------------------------------------------------------

/// The 50-byte record of §12.3.
const EXAMPLE_12_3: [u8; 50] = [
    0x99, 0x00, 0x00, 0x14, // own DID
    0, 0, 0, 0, // 0
    6, 0, 0, 0, // block-map key 6
    1, 0, 0, 0, // n = 1
    0, 0, 0, 0, // zone key 0
    0x04, 0x4D, 0x61, 0x69, 0x6E, // label, length 4
    1, 0, 0, 0, // m = 1
    0, 0, 0, 0, // slot index 0
    0, 0, 0, 0, // 0
    0x00, 0x00, 0x80, 0x3F, // 1.0
    1, 0, 0, 0, // 1
    0, 0, 0, 0, // 0
    0, // 0
];

#[test]
fn worked_example_12_3_encounter_table() {
    let t = EncounterTable::parse_expecting(&EXAMPLE_12_3, Did(0x1400_0099)).expect("decode");
    assert_eq!(t.did, Did(0x1400_0099));
    assert_eq!((t.reserved, t.block_map_key), (0, 6));
    assert_eq!(t.zones.len(), 1);
    let z = t.zone(0).expect("zone 0");
    assert_eq!(z.label, "Main");
    assert_eq!(
        z.slots,
        vec![EncounterSlot {
            index: 0,
            field_4: 0,
            field_8: 1.0,
            field_12: 1,
            field_16: 0,
            field_20: 0
        }]
    );
    assert!(t.keys_are_indices());
    assert_eq!(t.to_bytes(), EXAMPLE_12_3);

    // A 12-byte label gives 58 bytes.
    let mut long = t.clone();
    long.zones[0].label = "Twelve_chars".into();
    let bytes = long.to_bytes();
    assert_eq!(bytes.len(), 58);
    assert_eq!(
        decode_encounter_table(&bytes).expect("58-byte record"),
        long
    );

    // Several zones; keys that are not indices still decode.
    let mut many = t.clone();
    many.zones.push(EncounterZone {
        key: 5,
        label: String::new(),
        slots: vec![],
    });
    let back = decode_encounter_table(&many.to_bytes()).expect("two zones");
    assert_eq!(back, many);
    assert!(!back.keys_are_indices());
}

#[test]
fn encounter_table_rejects_bad_input() {
    for n in 0..EXAMPLE_12_3.len() {
        assert!(
            decode_encounter_table(&EXAMPLE_12_3[..n]).is_err(),
            "prefix {n}"
        );
    }
    let mut trailing = EXAMPLE_12_3.to_vec();
    trailing.push(0);
    assert!(matches!(
        decode_encounter_table(&trailing),
        Err(FormatError::TrailingBytes { .. })
    ));
    let mut wrong = EXAMPLE_12_3;
    wrong[3] = 0x15;
    assert!(matches!(
        decode_encounter_table(&wrong),
        Err(FormatError::WrongRecordType { .. })
    ));
    assert!(matches!(
        EncounterTable::parse_expecting(&EXAMPLE_12_3, Did(0x1400_0001)),
        Err(FormatError::DidMismatch { .. })
    ));
    // A hostile zone or slot count fails before allocating.
    let mut huge = EXAMPLE_12_3;
    huge[12..16].copy_from_slice(&u32::MAX.to_le_bytes());
    assert!(matches!(
        decode_encounter_table(&huge),
        Err(FormatError::Truncated { .. })
    ));
    let mut huge = EXAMPLE_12_3;
    huge[25..29].copy_from_slice(&u32::MAX.to_le_bytes());
    assert!(matches!(
        decode_encounter_table(&huge),
        Err(FormatError::Truncated { .. })
    ));
}
