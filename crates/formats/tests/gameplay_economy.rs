// SPDX-License-Identifier: GPL-3.0-or-later
//! Synthetic tests of the economy views (`tessera_formats::gameplay::economy`): the worked
//! examples of `docs/specs/economy.md` §13 and the invariants of §12. Every PID, DID and value
//! is invented; the DIDs use the spec's 0x70F0… range.

use std::collections::{BTreeMap, HashMap};

use proptest::prelude::*;
use tessera_formats::gameplay::common::build::{array, did, entry, set, structure};
use tessera_formats::gameplay::economy::{
    BARTER_ENTRY_LEN, BarterCost, BarterElement, BarterEntry, BarterReward, BarterShape, CraftPids,
    CrossLinks, DROPS_FROM_DEPTH, EconomyDefaults, EconomyKind, EconomyPids, EconomyReader,
    EconomyRecord, EntityFilter, FloatProgression, Invariant, KindByClass, LootPids, NpcVendor,
    RecipeVariant, VendorInventory, VendorPids, WeightedTable, check_drop_frequencies,
    cumulative_pick, lookup_quantity, species_key,
};
use tessera_formats::{Did, FormatError, PropertyEntry, PropertySet, PropertyValue};

/// Every label of the three label tables, numbered from 0x1000 in declaration order.
fn names() -> HashMap<String, u32> {
    let mut out = HashMap::new();
    let labels = LootPids::LABELS
        .iter()
        .chain(CraftPids::LABELS)
        .chain(VendorPids::LABELS);
    for (label, pid) in labels.zip(0x1000u32..) {
        out.entry((*label).to_owned()).or_insert(pid);
    }
    out
}

fn reader() -> EconomyReader {
    EconomyReader::from_parts(EconomyPids::resolve(&names()), EconomyDefaults::SPEC)
}

fn pid(label: &str) -> u32 {
    names()[label]
}

fn e(label: &str, value: PropertyValue) -> PropertyEntry {
    entry(pid(label), value)
}

fn s(label: &str, fields: Vec<PropertyEntry>) -> PropertyEntry {
    entry(pid(label), structure(fields))
}

fn a(label: &str, elements: Vec<PropertyEntry>) -> PropertyEntry {
    entry(pid(label), array(elements))
}

fn d(label: &str, raw: u32) -> PropertyEntry {
    entry(pid(label), did(raw))
}

fn int(label: &str, v: i32) -> PropertyEntry {
    e(label, PropertyValue::Int(v))
}

fn key(label: &str, v: u32) -> PropertyEntry {
    e(label, PropertyValue::Enum(v))
}

fn flag(label: &str, v: bool) -> PropertyEntry {
    e(label, PropertyValue::Bool(v))
}

fn float(label: &str, v: f32) -> PropertyEntry {
    e(label, PropertyValue::Float(v))
}

fn control_set(rows: &[(u32, f32)]) -> PropertySet {
    set(vec![a(
        "LootGenControl_DropFrequencyTable",
        rows.iter()
            .map(|&(k, f)| {
                s(
                    "LootGenControl_DropFrequencyTable",
                    vec![
                        key("LootGenControl_DropFrequency_Label", k),
                        float("LootGenControl_DropFrequency_Percentage", f),
                    ],
                )
            })
            .collect(),
    )])
}

fn trophy_example() -> PropertySet {
    set(vec![a(
        "LootGen_TrophyList",
        vec![
            s(
                "LootGen_TrophyList",
                vec![
                    d("LootGen_TrophyList_ItemOrProfile", 0x70F0_0010),
                    key("LootGen_TrophyList_DropFrequency", 9),
                ],
            ),
            s(
                "LootGen_TrophyList",
                vec![
                    d("LootGen_TrophyList_ItemOrProfile", 0x70F0_0020),
                    key("LootGen_TrophyList_DropFrequency", 2),
                    flag("LootGen_TrophyList_GroupDrop", true),
                ],
            ),
        ],
    )])
}

#[test]
fn worked_example_trophy_list_with_defaults() {
    // §13.1.
    let r = reader();
    let list = trophy_example();
    assert_eq!(r.kind(&list), Some(EconomyKind::TrophyList));
    let Some(EconomyRecord::TrophyList(t)) = r.decode(&list).unwrap() else {
        panic!("trophy list expected");
    };
    let control = r.loot_control(&control_set(&[(9, 0.7), (2, 0.1)])).unwrap();
    assert_eq!(t.entries.len(), 2);
    let (e0, e1) = (t.entries[0], t.entries[1]);
    assert_eq!(e0.target, Did(0x70F0_0010));
    assert_eq!(
        (e0.quantity, e0.quantity_stored, e0.group_drop),
        (1, false, false)
    );
    assert_eq!(control.drop_fraction(e0.drop_frequency.unwrap()), Some(0.7));
    assert_eq!(e1.target, Did(0x70F0_0020));
    assert_eq!((e1.quantity, e1.group_drop), (1, true));
    assert_eq!(control.drop_fraction(e1.drop_frequency.unwrap()), Some(0.1));
    assert_eq!(e0.mirror(), [0, 9, 1, 0x70F0_0010, 0]);
    assert_eq!(e1.mirror(), [1, 2, 1, 0x70F0_0020, 0]);

    let rec = EconomyRecord::TrophyList(t);
    assert!(rec.check().is_empty());
    assert!(check_drop_frequencies(&rec, &control).is_empty());
    // E-I3: a key without a control entry.
    let partial = r.loot_control(&control_set(&[(9, 0.7)])).unwrap();
    let v = check_drop_frequencies(&rec, &partial);
    assert_eq!(v.len(), 1);
    assert_eq!(v[0].invariant, Invariant::TrophyEntries);
    assert_eq!(v[0].entry, Some(1));
}

#[test]
fn dictionary_defaults_replace_spec_defaults() {
    let names = names();
    let pids = EconomyPids::resolve(&names);
    let defaults: BTreeMap<u32, PropertyValue> = [(
        names["LootGen_TrophyList_ItemQuantity"],
        PropertyValue::Int(3),
    )]
    .into();
    let resolved = EconomyDefaults::resolve(&defaults, &pids);
    assert_eq!(resolved.trophy_quantity, 3);
    assert_eq!(resolved.profile_quantity, 1);
    assert_eq!(resolved.munge_level_override, -1);
    let r = EconomyReader::from_parts(pids, resolved);
    let t = r.trophy_list(&trophy_example()).unwrap();
    assert_eq!(t.entries[0].quantity, 3);
}

#[test]
fn worked_example_subtree_sum_table() {
    // §13.2: count 4, (0, 2, 12), (1, 5, 9), (2, 1, 1), (3, 4, 4), then 0.
    let words: [u32; 14] = [4, 0, 2, 12, 1, 5, 9, 2, 1, 1, 3, 4, 4, 0];
    let bytes: Vec<u8> = words.iter().flat_map(|w| w.to_le_bytes()).collect();
    let t = WeightedTable::parse(&bytes).unwrap();
    assert_eq!(t, WeightedTable::from_weights(&[2, 5, 1, 4]));
    assert_eq!(t.to_bytes(), bytes);
    assert_eq!(t.total(), 12);
    assert_eq!(t.weights(), vec![2, 5, 1, 4]);
    // Entry 1 has probability 5/12: five of the twelve values select it.
    let hits = (0..12).filter(|&r| cumulative_pick(&t.weights(), r) == Some(1));
    assert_eq!(hits.count(), 5);

    // E-I12 violations.
    let mut bad = t.clone();
    bad.elements[1].sum = 8;
    bad.trailing = 1;
    let v = bad.check();
    assert_eq!(v.len(), 2);
    assert!(v.iter().all(|v| v.invariant == Invariant::SubtreeSums));
    let mut moved = t.clone();
    moved.elements[2].index = 7;
    assert!(moved.sums_hold() && !moved.indices_hold());

    // Hostile bodies.
    assert!(matches!(
        WeightedTable::parse(&bytes[..bytes.len() - 1]),
        Err(FormatError::Truncated { .. })
    ));
    assert!(WeightedTable::parse(&u32::MAX.to_le_bytes()).is_err());
    let mut extra = bytes.clone();
    extra.push(0);
    assert!(matches!(
        WeightedTable::parse(&extra),
        Err(FormatError::TrailingBytes { .. })
    ));
}

fn cost(item: u32, quantity: i32) -> PropertyEntry {
    s(
        "Barter_ItemStruct",
        vec![d("Barter_Item", item), int("Barter_ItemQuantity", quantity)],
    )
}

fn lookup_cost(item: u32, table: u32) -> PropertyEntry {
    s(
        "Barter_ItemStruct",
        vec![
            d("Barter_Item", item),
            d("Barter_ItemQuantity_LookupTable", table),
        ],
    )
}

fn reward(item: u32, quantity: i32) -> PropertyEntry {
    s(
        "Barter_RewardItemStruct",
        vec![d("Barter_Item", item), int("Barter_ItemQuantity", quantity)],
    )
}

fn reputation(faction: u32, tier: u32) -> PropertyEntry {
    s(
        "Barter_RewardReputationStruct",
        vec![
            d("Barter_ReputationFaction", faction),
            key("Barter_ReputationRewardTier", tier),
        ],
    )
}

fn barter(lists: Vec<Vec<PropertyEntry>>) -> PropertySet {
    set(vec![a(
        "Barter_ItemListArray",
        lists
            .into_iter()
            .map(|l| a("Barter_ItemListArray", l))
            .collect(),
    )])
}

#[test]
fn worked_example_barter_entry() {
    // §13.3.
    let r = reader();
    let profile = barter(vec![vec![
        cost(0x70F0_0031, 3),
        cost(0x70F0_0032, 10),
        reward(0x70F0_0040, 2),
    ]]);
    assert_eq!(r.kind(&profile), Some(EconomyKind::BarterProfile));
    let b = r.barter_profile(&profile).unwrap();
    assert_eq!(b.munge_level_override, -1);
    let list = &b.lists[0];
    assert_eq!(list.shape(), Some(BarterShape::Item { costs: 2 }));
    let entry = list
        .expected_entry(Did(0x70F0_0030), |c| {
            c.quantity.and_then(|q| u32::try_from(q).ok())
        })
        .unwrap();
    let bytes = entry.to_bytes();
    assert_eq!(bytes.len(), BARTER_ENTRY_LEN);
    assert_eq!(
        &bytes[..8],
        &[0x30, 0x00, 0xF0, 0x70, 0x40, 0x00, 0xF0, 0x70]
    );
    assert_eq!(&bytes[8..12], &2u32.to_le_bytes());
    assert_eq!(bytes[12], 0);
    assert_eq!(&bytes[13..21], &[0; 8]);
    assert_eq!(&bytes[21..25], &0x70F0_0031u32.to_le_bytes());
    assert_eq!(&bytes[25..29], &3u32.to_le_bytes());
    assert_eq!(&bytes[29..33], &0x70F0_0032u32.to_le_bytes());
    assert_eq!(&bytes[33..37], &10u32.to_le_bytes());
    assert_eq!(&bytes[37..49], &[0; 12]);
    assert_eq!(BarterEntry::parse(&bytes).unwrap(), entry);

    // Offset 12 is a bool8; anything else is rejected, as are short and long bodies.
    let mut bad = bytes;
    bad[12] = 2;
    assert!(matches!(
        BarterEntry::parse(&bad),
        Err(FormatError::InvalidBool { .. })
    ));
    assert!(BarterEntry::parse(&bytes[..48]).is_err());
    let mut long = bytes.to_vec();
    long.push(0);
    assert!(BarterEntry::parse(&long).is_err());
}

#[test]
fn barter_shapes_and_reputation_entries() {
    // §6.1 list shapes and E-I9, E-I10.
    let r = reader();
    let sound = key("Barter_SoundCategory", 45);
    let profile = barter(vec![
        vec![cost(1, 1), reward(9, 1)],
        vec![cost(1, 1), cost(2, 1), cost(3, 1), reward(9, 1)],
        vec![cost(1, 1), reputation(0x70F0_0050, 3)],
        vec![cost(1, 1), reputation(0x70F0_0050, 4), sound.clone()],
        // Not shapes: no cost, four costs, reward first, two rewards, sound after an item.
        vec![reward(9, 1)],
        vec![cost(1, 1), cost(2, 1), cost(3, 1), cost(4, 1), reward(9, 1)],
        vec![reward(9, 1), cost(1, 1)],
        vec![cost(1, 1), reward(9, 1), reward(8, 1)],
        vec![cost(1, 1), reward(9, 1), sound],
        // E-I10: both quantity and table, then neither.
        vec![
            s(
                "Barter_ItemStruct",
                vec![
                    d("Barter_Item", 1),
                    int("Barter_ItemQuantity", 1),
                    d("Barter_ItemQuantity_LookupTable", 0x70F0_0060),
                ],
            ),
            s("Barter_ItemStruct", vec![]),
            reward(9, 1),
        ],
    ]);
    let b = r.barter_profile(&profile).unwrap();
    let shapes: Vec<_> = b.lists.iter().map(|l| l.shape()).collect();
    assert_eq!(
        shapes,
        vec![
            Some(BarterShape::Item { costs: 1 }),
            Some(BarterShape::Item { costs: 3 }),
            Some(BarterShape::Reputation {
                costs: 1,
                sound: false
            }),
            Some(BarterShape::Reputation {
                costs: 1,
                sound: true
            }),
            None,
            None,
            None,
            None,
            None,
            Some(BarterShape::Item { costs: 2 }),
        ]
    );
    // A cost struct without an item reads as DID 0 (§6.1).
    let empty_cost = b.lists[9].costs().nth(1).unwrap();
    assert_eq!(empty_cost.item, Did(0));

    // Reputation entry with a sound category (§9.3).
    let rep = b.lists[3]
        .expected_entry(Did(0x70F0_0030), |c| c.quantity.map(|q| q as u32))
        .unwrap();
    assert_eq!(rep.reward_item, Did(0));
    assert_eq!(rep.reward_quantity, 0);
    assert_eq!(rep.reputation_faction, Did(0x70F0_0050));
    assert_eq!(rep.reputation_tier, 4);
    assert_eq!(rep.sound_category, 45);
    assert_eq!(rep.costs[0], (Did(1), 1));
    assert_eq!(rep.costs[1], (Did(0), 0));
    // Four costs or two rewards: no entry.
    assert!(b.lists[5].expected_entry(Did(1), |_| Some(1)).is_none());
    assert!(b.lists[7].expected_entry(Did(1), |_| Some(1)).is_none());

    let v = EconomyRecord::BarterProfile(b).check();
    let shape: Vec<_> = v
        .iter()
        .filter(|v| v.invariant == Invariant::BarterShape)
        .map(|v| v.entry)
        .collect();
    assert_eq!(shape, vec![Some(4), Some(5), Some(6), Some(7), Some(8)]);
    let quantity = v
        .iter()
        .filter(|v| v.invariant == Invariant::BarterCostQuantity)
        .count();
    assert_eq!(quantity, 2);
}

#[test]
fn worked_example_lookup_cost() {
    // §13.4 and §6.2.
    let r = reader();
    let progression = set(vec![
        int("Progression_MinimumIndexValue", 1),
        a(
            "FloatProgression_Array",
            [3.0, 3.0, 4.0, 5.0, 6.0]
                .iter()
                .map(|&v| float("FloatProgression_FloatValue", v))
                .collect(),
        ),
    ]);
    let prog = r.float_progression(&progression).unwrap();
    assert_eq!(
        prog,
        FloatProgression {
            minimum_index: 1,
            values: vec![3.0, 3.0, 4.0, 5.0, 6.0]
        }
    );
    assert_eq!(lookup_quantity(&prog, 3), Some(4));
    assert_eq!(lookup_quantity(&prog, 9), Some(6));
    assert_eq!(lookup_quantity(&prog, -40), Some(3));
    let empty = FloatProgression {
        minimum_index: 0,
        values: Vec::new(),
    };
    assert_eq!(lookup_quantity(&empty, 3), None);
    let fractional = FloatProgression {
        minimum_index: 0,
        values: vec![1.5],
    };
    assert_eq!(lookup_quantity(&fractional, 0), None);

    // The reward item's level: Item_Level, or Usage_MinLevel (top level or in the
    // permission struct) when the reward uses the minimum usage level.
    let item = set(vec![
        int("Item_Level", 3),
        s(
            "DefaultPermissionBlobStruct",
            vec![int("Usage_MinLevel", 9)],
        ),
    ]);
    assert_eq!(r.reward_level(&item, false).unwrap(), Some(3));
    assert_eq!(r.reward_level(&item, true).unwrap(), Some(9));
    let top = set(vec![int("Usage_MinLevel", 5)]);
    assert_eq!(r.reward_level(&top, true).unwrap(), Some(5));
    assert_eq!(r.reward_level(&top, false).unwrap(), None);

    // A barter list with a look-up cost resolves to the progression value.
    let b = r
        .barter_profile(&barter(vec![vec![
            lookup_cost(0x70F0_0031, 0x70F0_0070),
            reward(0x70F0_0040, 1),
        ]]))
        .unwrap();
    let c = *b.lists[0].costs().next().unwrap();
    assert_eq!(
        c,
        BarterCost {
            item: Did(0x70F0_0031),
            quantity: None,
            lookup_table: Some(Did(0x70F0_0070)),
        }
    );
    let level = r.reward_level(&item, false).unwrap().unwrap();
    let entry = b.lists[0]
        .expected_entry(Did(0x70F0_0030), |c| match c.lookup_table {
            Some(_) => lookup_quantity(&prog, level),
            None => c.quantity.map(|q| q as u32),
        })
        .unwrap();
    assert_eq!(entry.costs[0], (Did(0x70F0_0031), 4));
}

fn species_entry(species: u32, subspecies: u32, exam: u32, table: u32) -> PropertyEntry {
    s(
        "SpeciesLevelLootTable",
        vec![
            key("Agent_Species", species),
            key("Agent_Subspecies", subspecies),
            key("MonsterLevel_ExaminationModStatType", exam),
            d("LevelBasedLootTable_LevelTable", table),
        ],
    )
}

#[test]
fn worked_example_species_key() {
    // §13.5 and E-I6.
    let r = reader();
    let table = set(vec![a(
        "SpeciesLevelLootTable",
        vec![
            species_entry(35, 5, 7, 0x70F0_0080),
            species_entry(35, 0, 7, 0x70F0_0081),
        ],
    )]);
    let t = r.species_level_loot_table(&table).unwrap();
    assert_eq!(t.entries[0].key(), Some(0x0000_0007_0230_0050));
    assert_eq!(species_key(35, 5, 7), Some(0x0000_0007_0230_0050));
    assert_eq!(t.mirror_map().len(), 2);
    assert!(EconomyRecord::SpeciesLevelLootTable(t).check().is_empty());

    let dup = set(vec![a(
        "SpeciesLevelLootTable",
        vec![
            species_entry(35, 5, 7, 0x70F0_0080),
            species_entry(35, 5, 7, 0x70F0_0082),
        ],
    )]);
    let v = r
        .decode(&dup)
        .unwrap()
        .map(|rec| rec.check())
        .unwrap_or_default();
    assert_eq!(v.len(), 1);
    assert_eq!(v[0].invariant, Invariant::SpeciesKeys);

    // PVP keys: species × 2^16 while the class is 0 (§9.5).
    let pvp = set(vec![a(
        "PVPLevelLootTable",
        vec![
            s(
                "PVPLevelLootTable",
                vec![
                    key("Agent_Species", 4),
                    key("Agent_Class", 0),
                    d("LevelBasedLootTable_LevelTable", 1),
                ],
            ),
            s(
                "PVPLevelLootTable",
                vec![key("Agent_Species", 4), key("Agent_Class", 2)],
            ),
        ],
    )]);
    let p = r.pvp_level_loot_table(&pvp).unwrap();
    assert_eq!(p.entries[0].key(), Some(4 << 16));
    assert_eq!(p.entries[1].key(), None);
}

#[test]
fn every_kind_is_recognised_by_its_characteristic_property() {
    let r = reader();
    for kind in EconomyKind::ALL {
        let label = kind.characteristic(&r.pids).label();
        let value = match label {
            "CraftRecipe_ResultItem"
            | "CraftGuild_Profession"
            | "SkirmishTreasureLookup_TrophyListTemplateDID" => did(0x70F0_0001),
            "Currency_Value" => PropertyValue::Int(100),
            "Currency_Control_GarnishmentPercentage" => PropertyValue::Float(0.25),
            _ => array(vec![]),
        };
        let one = set(vec![e(label, value)]);
        assert_eq!(r.kind(&one), Some(kind), "{label}");
        let rec = r.decode(&one).unwrap().unwrap();
        assert_eq!(rec.kind(), kind);
    }
    assert_eq!(r.kind(&set(vec![])), None);
    assert_eq!(r.decode(&set(vec![int("Item_Level", 3)])).unwrap(), None);

    // Unresolved labels never match.
    let blind = EconomyReader::from_parts(EconomyPids::unresolved(), EconomyDefaults::SPEC);
    assert_eq!(blind.kind(&trophy_example()), None);
    assert!(!EconomyPids::unresolved().missing().is_empty());
    assert!(EconomyPids::resolve(&names()).missing().is_empty());

    // An empty vendor inventory has no property; its class is learned from the others.
    let mut by_class = KindByClass::new();
    by_class.learn(40, EconomyKind::VendorInventory);
    by_class.learn(41, EconomyKind::TrophyList);
    by_class.learn(41, EconomyKind::Recipe);
    assert_eq!(by_class.kind(40), Some(EconomyKind::VendorInventory));
    assert_eq!(by_class.kind(41), None);
    assert_eq!(by_class.classes_of(EconomyKind::VendorInventory), vec![40]);
    let empty = r
        .decode_as(EconomyKind::VendorInventory, &set(vec![]))
        .unwrap();
    assert_eq!(
        empty,
        EconomyRecord::VendorInventory(VendorInventory::default())
    );
}

#[test]
fn wrong_value_types_are_errors_not_panics() {
    let r = reader();
    let bad = set(vec![a(
        "LootGen_TrophyList",
        vec![s(
            "LootGen_TrophyList",
            vec![int("LootGen_TrophyList_ItemOrProfile", 5)],
        )],
    )]);
    assert!(matches!(
        r.trophy_list(&bad),
        Err(FormatError::PropertyTypeMismatch {
            label: "LootGen_TrophyList_ItemOrProfile",
            ..
        })
    ));
    let not_struct = set(vec![a(
        "LootGen_TrophyList",
        vec![int("LootGen_TrophyList", 5)],
    )]);
    assert!(r.trophy_list(&not_struct).is_err());
    let not_array = set(vec![int("LootGen_TrophyList", 5)]);
    assert!(r.decode(&not_array).is_err());
}

/// Shapes and types the spec states are enforced; nothing is dropped silently.
#[test]
fn stated_shapes_are_strict() {
    let r = reader();
    let level = pid("Usage_MinLevel");
    let range = |min: Vec<PropertyEntry>| {
        set(vec![a(
            "LootGen_FilteredTreasureListArray",
            vec![s(
                "LootGen_FilteredTreasureListArray",
                vec![
                    d("LootGen_FilteredTrophyTable_EntryDID", 0x70F0_0300),
                    a(
                        "EntityFilter_Array",
                        vec![s(
                            "EntityFilter_PropertyRange",
                            vec![
                                a("EntityFilter_PropertyRange_Min", min),
                                a(
                                    "EntityFilter_PropertyRange_Max",
                                    vec![entry(level, PropertyValue::Int(19))],
                                ),
                            ],
                        )],
                    ),
                ],
            )],
        )])
    };
    // §4.5: each bound is a one-element array.
    assert!(
        r.filtered_treasure_list(&range(vec![entry(level, PropertyValue::Int(10))]))
            .is_ok()
    );
    for bad in [
        vec![],
        vec![
            entry(level, PropertyValue::Int(10)),
            entry(level, PropertyValue::Int(11)),
        ],
    ] {
        assert!(matches!(
            r.filtered_treasure_list(&range(bad)),
            Err(FormatError::PropertyTypeMismatch {
                label: "EntityFilter_PropertyRange_Min",
                ..
            })
        ));
    }

    // §4.5, §4.8: the operator is an enum key, the value an int (E1).
    let directory_with = |operator: PropertyEntry, value: PropertyEntry| {
        set(vec![a(
            "WorldEventLootTableDirectory_Array",
            vec![s(
                "WorldEventLootTableDirectory_Array",
                vec![
                    d("WorldEventLootTableDirectory_LevelTable", 0x70F0_0600),
                    a(
                        "WorldEvent_AllConditionList",
                        vec![s(
                            "WorldEvent_Condition",
                            vec![d("WorldEvent_WorldEvent", 0x70F0_0400), operator, value],
                        )],
                    ),
                ],
            )],
        )])
    };
    let directory =
        |operator: PropertyEntry| directory_with(operator, int("WorldEvent_ConditionValue", 2));
    assert!(matches!(
        r.world_event_directory(&directory_with(
            key("WorldEvent_Operator", 3),
            key("WorldEvent_ConditionValue", 2)
        )),
        Err(FormatError::PropertyTypeMismatch {
            label: "WorldEvent_ConditionValue",
            ..
        })
    ));
    let ok = r
        .world_event_directory(&directory(key("WorldEvent_Operator", 3)))
        .unwrap();
    assert_eq!(ok.entries[0].conditions[0].operator, Some(3));
    assert_eq!(ok.entries[0].conditions[0].value, Some(2));
    assert!(matches!(
        r.world_event_directory(&directory(int("WorldEvent_Operator", 3))),
        Err(FormatError::PropertyTypeMismatch {
            label: "WorldEvent_Operator",
            ..
        })
    ));

    // §8: a store bundle holds DID elements.
    let store = set(vec![a(
        "WebStoreAccountItem_BundleItemArray",
        vec![
            d("WebStoreAccountItem_DataID", 0x70F0_0A01),
            d("WebStoreAccountItem_DataID", 0),
        ],
    )]);
    assert_eq!(
        r.store_links(&store).unwrap().bundle,
        vec![Did(0x70F0_0A01)]
    );
    let wrapped = set(vec![a(
        "WebStoreAccountItem_BundleItemArray",
        vec![s(
            "WebStoreAccountItem_BundleItemArray",
            vec![d("WebStoreAccountItem_DataID", 0x70F0_0A01)],
        )],
    )]);
    assert!(r.store_links(&wrapped).is_err());

    // §6.2: a progression element that is not numeric fails.
    let progression = set(vec![a(
        "FloatProgression_Array",
        vec![flag("FloatProgression_FloatValue", true)],
    )]);
    assert!(r.float_progression(&progression).is_err());
}

#[test]
fn trophy_entry_invariants() {
    let r = reader();
    let list = set(vec![a(
        "LootGen_TrophyList",
        vec![s(
            "LootGen_TrophyList",
            vec![int("LootGen_TrophyList_ItemQuantity", 4)],
        )],
    )]);
    let t = r.trophy_list(&list).unwrap();
    assert_eq!(t.entries[0].quantity, 4);
    assert!(t.entries[0].quantity_stored);
    let v = EconomyRecord::TrophyList(t).check();
    assert_eq!(v.len(), 2);
    assert!(v.iter().all(|v| v.invariant == Invariant::TrophyEntries));
}

#[test]
fn pick_one_tables_and_weights() {
    // §4.2–§4.4, E-I4, §9.4 order of the 3307 mirror.
    let r = reader();
    let tgp = set(vec![a(
        "TreasureGroupProfile_ItemTable",
        vec![
            s(
                "TreasureGroupProfile_ItemTable",
                vec![
                    d("TreasureGroupProfile_ItemTable_Item", 0x70F0_0010),
                    int("TreasureGroupProfile_ItemTable_Weight", 3),
                ],
            ),
            s(
                "TreasureGroupProfile_ItemTable",
                vec![
                    d("TreasureGroupProfile_ItemTable_Item", 0x70F0_0011),
                    int("TreasureGroupProfile_ItemTable_Weight", 0),
                    int("TreasureGroupProfile_ItemTable_Quantity", 6),
                ],
            ),
        ],
    )]);
    let p = r.treasure_group_profile(&tgp).unwrap();
    assert_eq!(p.entries[0].mirror(), [1, 0x70F0_0010]);
    assert_eq!(p.entries[1].mirror(), [6, 0x70F0_0011]);
    assert_eq!(p.weights(), vec![3, 0]);
    let v = EconomyRecord::TreasureGroupProfile(p).check();
    assert_eq!(v.len(), 1);
    assert_eq!((v[0].invariant, v[0].entry), (Invariant::Weights, Some(1)));

    let weighted = |w: &[i32]| {
        set(vec![a(
            "LootGen_WeightedTreasureTable",
            w.iter()
                .zip(0x70F0_0100u32..)
                .map(|(&w, t)| {
                    s(
                        "LootGen_WeightedTreasureTable",
                        vec![
                            d("LootGen_WeightedTreasureTable_TrophyList", t),
                            int("LootGen_WeightedTreasureTable_TrophyList_Weight", w),
                        ],
                    )
                })
                .collect(),
        )])
    };
    let t = r.weighted_treasure_table(&weighted(&[5, 1, 3])).unwrap();
    let sorted: Vec<i32> = t.sorted_by_weight().iter().map(|e| e.weight).collect();
    assert_eq!(sorted, vec![1, 3, 5]);
    assert!(EconomyRecord::WeightedTreasureTable(t).check().is_empty());
    let zero = r.weighted_treasure_table(&weighted(&[0, 0])).unwrap();
    assert_eq!(EconomyRecord::WeightedTreasureTable(zero).check().len(), 1);
    let none = r.treasure_list(&set(vec![a("LootGen_TreasureList", vec![])]));
    assert_eq!(
        EconomyRecord::TreasureList(none.unwrap()).check()[0].invariant,
        Invariant::Weights
    );
}

#[test]
fn level_tables() {
    // §4.6, §4.8, §4.9, E-I5.
    let r = reader();
    let level_table = |n: i32, skip: Option<i32>| {
        set(vec![a(
            "LevelBasedLootTable_LootTable",
            (1..=n)
                .map(|l| {
                    let level = if skip == Some(l) { l + 1 } else { l };
                    let mut f = vec![int("LevelBasedLootTable_Level", level)];
                    if l % 2 == 0 {
                        f.push(d("LevelBasedLootTable_TrophyList", 0x70F0_1000));
                    } else {
                        f.push(d("LevelBasedLootTable_TrophyList", 0));
                        f.push(d("LevelBasedLootTable_TreasureProfile", 0x70F0_2000));
                    }
                    s("LevelBasedLootTable_LootTable", f)
                })
                .collect(),
        )])
    };
    let t = r.level_based_loot_table(&level_table(160, None)).unwrap();
    assert_eq!(t.at_level(2).unwrap().trophy_list, Some(Did(0x70F0_1000)));
    assert_eq!(t.at_level(1).unwrap().trophy_list, None);
    assert_eq!(t.at_level(1).unwrap().treasure_list, Some(Did(0x70F0_2000)));
    let (trophies, treasures) = t.mirror_maps();
    assert_eq!((trophies.len(), treasures.len()), (80, 80));
    assert!(EconomyRecord::LevelBasedLootTable(t).check().is_empty());
    let short = r.level_based_loot_table(&level_table(159, None)).unwrap();
    assert_eq!(EconomyRecord::LevelBasedLootTable(short).check().len(), 1);
    let gap = r
        .level_based_loot_table(&level_table(160, Some(7)))
        .unwrap();
    let v = EconomyRecord::LevelBasedLootTable(gap).check();
    assert_eq!(v.len(), 1);
    assert_eq!(v[0].entry, Some(6));

    // An absent level takes the default 1 (§4.6, §1); a negative level has no map key.
    let defaulted = r
        .level_based_loot_table(&set(vec![a(
            "LevelBasedLootTable_LootTable",
            vec![
                s(
                    "LevelBasedLootTable_LootTable",
                    vec![d("LevelBasedLootTable_TrophyList", 0x70F0_1000)],
                ),
                s(
                    "LevelBasedLootTable_LootTable",
                    vec![
                        int("LevelBasedLootTable_Level", -2),
                        d("LevelBasedLootTable_TrophyList", 0x70F0_1001),
                    ],
                ),
            ],
        )]))
        .unwrap();
    assert_eq!(
        (
            defaulted.entries[0].level,
            defaulted.entries[0].level_stored
        ),
        (1, false)
    );
    assert_eq!(defaulted.entries[1].level, -2);
    let (trophies, _) = defaulted.mirror_maps();
    assert_eq!(trophies, BTreeMap::from([(1, 0x70F0_1000)]));

    // §4.9 (E2): every entry stores `RunicLevelLootTable_Level` = position + 1.
    let runic = |n: usize, stored: bool| {
        set(vec![a(
            "RunicLevelLootTable_LootTable",
            (0..n)
                .map(|i| {
                    let mut fields = vec![
                        d("RunicLevelLootTable_TrophyList", 0x70F0_1000),
                        d("RunicLevelLootTable_RunicTrophyList", 0),
                    ];
                    if stored {
                        fields.push(int(
                            "RunicLevelLootTable_Level",
                            i32::try_from(i + 1).unwrap(),
                        ));
                    }
                    s("RunicLevelLootTable_LootTable", fields)
                })
                .collect(),
        )])
    };
    let t = r.runic_level_loot_table(&runic(80, true)).unwrap();
    assert_eq!(t.entries[79].level, 80);
    assert!(t.entries[79].level_stored);
    assert_eq!(t.entries[0].runic_list, None);
    assert!(EconomyRecord::RunicLevelLootTable(t).check().is_empty());
    let t = r.runic_level_loot_table(&runic(3, true)).unwrap();
    assert_eq!(EconomyRecord::RunicLevelLootTable(t).check().len(), 1);
    // Absent levels take the default 1, so every entry after the first violates E-I5.
    let t = r.runic_level_loot_table(&runic(80, false)).unwrap();
    assert_eq!((t.entries[5].level, t.entries[5].level_stored), (1, false));
    assert_eq!(EconomyRecord::RunicLevelLootTable(t).check().len(), 79);
}

#[test]
fn filtered_list_with_the_three_filter_kinds() {
    // §4.5.
    let r = reader();
    let class = pid("Agent_Class");
    let level = pid("Usage_MinLevel");
    let list = set(vec![a(
        "LootGen_FilteredTreasureListArray",
        vec![s(
            "LootGen_FilteredTreasureListArray",
            vec![
                d("LootGen_FilteredTrophyTable_EntryDID", 0x70F0_0300),
                a(
                    "EntityFilter_Array",
                    vec![
                        a(
                            "EntityFilter_PropertySet",
                            vec![
                                entry(class, PropertyValue::Enum(23)),
                                entry(class, PropertyValue::Enum(24)),
                            ],
                        ),
                        a("EntityFilter_PropertySet", vec![]),
                        s(
                            "EntityFilter_PropertyRange",
                            vec![
                                a(
                                    "EntityFilter_PropertyRange_Min",
                                    vec![entry(level, PropertyValue::Int(10))],
                                ),
                                a(
                                    "EntityFilter_PropertyRange_Max",
                                    vec![entry(level, PropertyValue::Int(19))],
                                ),
                            ],
                        ),
                        s(
                            "EntityFilter_WorldEvent",
                            vec![s(
                                "WorldEvent_Condition",
                                vec![
                                    d("WorldEvent_WorldEvent", 0x70F0_0400),
                                    key("WorldEvent_Operator", 3),
                                    int("WorldEvent_ConditionValue", 1),
                                ],
                            )],
                        ),
                        int("Item_Level", 1),
                    ],
                ),
            ],
        )],
    )]);
    let f = r.filtered_treasure_list(&list).unwrap();
    let entry0 = &f.entries[0];
    assert_eq!(entry0.table, Did(0x70F0_0300));
    assert_eq!(entry0.filters.len(), 5);
    assert_eq!(entry0.filters[0].tested_property(), Some(class));
    assert_eq!(entry0.filters[1], EntityFilter::PropertySet(vec![]));
    assert_eq!(entry0.filters[2].tested_property(), Some(level));
    let EntityFilter::PropertyRange {
        min: Some(min),
        max: Some(max),
    } = &entry0.filters[2]
    else {
        panic!("range expected");
    };
    assert_eq!(
        (&min.value, &max.value),
        (&PropertyValue::Int(10), &PropertyValue::Int(19))
    );
    let EntityFilter::WorldEvent(Some(c)) = &entry0.filters[3] else {
        panic!("world event expected");
    };
    assert_eq!(c.world_event, Did(0x70F0_0400));
    assert_eq!((c.operator, c.value), (Some(3), Some(1)));
    assert!(matches!(entry0.filters[4], EntityFilter::Other { .. }));
}

fn recipe(extra: Vec<PropertyEntry>) -> PropertySet {
    let mut fields = vec![
        d("CraftRecipe_Profession", 0x70F0_0500),
        d("CraftRecipe_ResultItem", 0x70F0_0010),
        a(
            "CraftRecipe_IngredientList",
            vec![s(
                "CraftRecipe_IngredientList",
                vec![
                    d("CraftRecipe_Ingredient", 0x70F0_0011),
                    int("CraftRecipe_IngredientQuantity", 2),
                ],
            )],
        ),
    ];
    fields.extend(extra);
    set(fields)
}

#[test]
fn recipe_variants_and_rules() {
    // §5.1 rules 1–4, E-I7.
    let r = reader();
    let single = recipe(vec![
        key("CraftRecipe_Tier", 3),
        int("CraftRecipe_Cooldown_MithrilResetCost", 0),
        d("CraftRecipe_CriticalResultItem", 0x70F0_0012),
        float("CraftRecipe_CriticalSuccessChance", 0.05),
        a(
            "CraftRecipe_OptionalIngredientList",
            vec![s(
                "CraftRecipe_OptionalIngredientList",
                vec![
                    d("CraftRecipe_Ingredient", 0x70F0_0013),
                    int("CraftRecipe_IngredientQuantity", 1),
                    float("CraftRecipe_IngredientCritBonus", 0.25),
                ],
            )],
        ),
        d("CraftRecipe_IngredientPack", 0x70F0_0014),
        int("CraftRecipe_IngredientPackQuantity", 1),
        a(
            "CraftRecipe_MultiOutputArray",
            vec![s(
                "CraftRecipe_MultiOutputArray",
                vec![
                    d("CraftRecipe_Ingredient", 0x70F0_0015),
                    d("CraftRecipe_ResultItem", 0x70F0_0016),
                    d("CraftRecipe_CriticalResultItem", 0),
                ],
            )],
        ),
    ]);
    let rec = r.recipe(&single).unwrap();
    assert_eq!(rec.variant(), Some(RecipeVariant::SingleTier));
    assert_eq!(rec.tiers(), vec![3]);
    assert_eq!((rec.result_quantity, rec.critical_quantity), (1, 1));
    assert!(rec.ingredients[0].consumed);
    assert_eq!(rec.ingredients[0].crit_bonus, 0.0);
    assert_eq!(rec.optional_ingredients[0].crit_bonus, 0.25);
    assert_eq!(rec.multi_outputs[0].critical_result, None);
    assert_eq!(
        rec.produced_items(),
        vec![Did(0x70F0_0010), Did(0x70F0_0012), Did(0x70F0_0016)]
    );
    assert_eq!(
        rec.used_items(),
        vec![
            Did(0x70F0_0011),
            Did(0x70F0_0013),
            Did(0x70F0_0015),
            Did(0x70F0_0014)
        ]
    );
    assert!(EconomyRecord::Recipe(Box::new(rec)).check().is_empty());

    let array_variant = recipe(vec![
        a("CraftRecipe_TierArray", vec![key("CraftRecipe_Tier", 10)]),
        int("CraftRecipe_XPAward_Driver", 10),
        flag("CraftRecipe_LevelMungeOutput", false),
        int("CraftRecipe_ResultItemQuantity", 5),
    ]);
    let rec = r.recipe(&array_variant).unwrap();
    assert_eq!(rec.variant(), Some(RecipeVariant::TierArray));
    assert_eq!(rec.tiers(), vec![10]);
    assert_eq!(rec.result_quantity, 5);
    assert!(EconomyRecord::Recipe(Box::new(rec)).check().is_empty());

    // Both variants, a chance without a critical result, no result item.
    let broken = set(vec![
        key("CraftRecipe_Tier", 3),
        a("CraftRecipe_TierArray", vec![]),
        float("CraftRecipe_CriticalSuccessChance", 0.05),
        d("CraftRecipe_ResultItem", 0),
    ]);
    let rec = r.recipe(&broken).unwrap();
    assert_eq!(rec.variant(), None);
    let v = EconomyRecord::Recipe(Box::new(rec)).check();
    assert_eq!(v.len(), 3);
    assert!(v.iter().all(|v| v.invariant == Invariant::Recipe));
}

#[test]
fn profession_tiers() {
    // §5.2, E-I8.
    let r = reader();
    let profession = |tiers: Vec<u32>| {
        set(vec![
            a(
                "CraftProfession_TierArray",
                tiers
                    .into_iter()
                    .map(|t| {
                        s(
                            "CraftProfession_TierArray",
                            vec![
                                key("CraftProfession_Tier", t),
                                a(
                                    "CraftProfession_RecipeArray",
                                    vec![d("CraftProfession_RecipeArray", 0x70F0_0600 + t)],
                                ),
                            ],
                        )
                    })
                    .collect(),
            ),
            e(
                "CraftProfession_RequiredTool",
                PropertyValue::Bitfield32(0b100),
            ),
            e(
                "CraftProfession_OpenLevel_PropertyName",
                PropertyValue::PropertyId(0x0777),
            ),
        ])
    };
    let p = r.profession(&profession((1..=16).rev().collect())).unwrap();
    assert_eq!(p.required_tool_keys(), vec![3]);
    assert_eq!(p.state_properties.open_level, Some(0x0777));
    assert_eq!(p.state_properties.all().iter().flatten().count(), 1);
    assert_eq!(p.recipes().count(), 16);
    assert!(EconomyRecord::Profession(p).check().is_empty());
    let mut repeated: Vec<u32> = (1..=15).collect();
    repeated.push(15);
    let p = r.profession(&profession(repeated)).unwrap();
    assert_eq!(
        EconomyRecord::Profession(p).check()[0].invariant,
        Invariant::ProfessionTiers
    );
}

#[test]
fn vendor_item_list_is_the_union_of_inventories() {
    // §6.3, E-I11.
    let r = reader();
    let npc = set(vec![
        a(
            "Vendor_InventoryList",
            vec![
                d("Vendor_InventoryList", 0x70F0_0700),
                d("Vendor_InventoryList", 0x70F0_0701),
            ],
        ),
        a(
            "Vendor_ItemList",
            vec![
                d("Vendor_ItemList", 3),
                d("Vendor_ItemList", 1),
                d("Vendor_ItemList", 2),
            ],
        ),
        e(
            "Vendor_DiscountList",
            array(vec![e(
                "Vendor_DiscountList",
                PropertyValue::PropertyId(77),
            )]),
        ),
    ]);
    let v: NpcVendor = r.npc_vendor(&npc).unwrap().unwrap();
    assert_eq!(v.sell_modifier, -1.0);
    assert_eq!(v.discounts, vec![77]);
    let inv_a = VendorInventory {
        items: vec![Did(1), Did(2)],
    };
    let inv_b = VendorInventory {
        items: vec![Did(2), Did(3)],
    };
    let lookup = |d: Did| match d.0 {
        0x70F0_0700 => Some(&inv_a),
        0x70F0_0701 => Some(&inv_b),
        _ => None,
    };
    assert!(v.item_list_matches(lookup));
    let mut dup = v.clone();
    dup.item_list.push(Did(1));
    assert!(!dup.item_list_matches(lookup));
    let mut missing = v.clone();
    missing.inventories.push(Did(0x70F0_0799));
    assert!(!missing.item_list_matches(lookup));
    assert_eq!(
        r.npc_vendor(&set(vec![int("Item_Level", 1)])).unwrap(),
        None
    );
}

#[test]
fn currency_and_singletons() {
    let r = reader();
    let c = r
        .currency(&set(vec![
            int("Currency_Value", 100),
            d("Currency_ObjectDID", 0x70F0_0800),
        ]))
        .unwrap();
    assert_eq!((c.value, c.object), (Some(100), Some(Did(0x70F0_0800))));
    let money = set(vec![a(
        "LootGenControl_MoneyDropTable",
        vec![s(
            "LootGenControl_MoneyDropTable",
            vec![
                int("LootGenControl_MoneyDropTable_DropAmountMaximum", 10),
                int("LootGenControl_MoneyDropTable_DropAmountVariance", 5),
            ],
        )],
    )]);
    let ctl = r.loot_control(&money).unwrap();
    assert_eq!(ctl.money_drops[0].maximum, Some(10));
    let craft = set(vec![a(
        "CraftControl_XPRewardArray",
        vec![s(
            "CraftControl_XPRewardArray",
            vec![
                key("CraftControl_XPRewardEnum", 4),
                int("CraftControl_XPRewardValue", 12),
            ],
        )],
    )]);
    let cc = r.craft_control(&craft).unwrap();
    assert_eq!(cc.xp_value(4), Some(12));
    assert_eq!(cc.xp_value(5), None);
}

/// A small loot graph:
/// mob → filtered list → weighted table → trophy list → treasure list → profile → item,
/// plus a vendor, a barter profile, a recipe and a recipe item.
#[test]
fn cross_links() {
    let r = reader();
    let item = 0x70F0_0010;
    let (mob, filtered, weighted, trophy, treasure, profile) = (
        0x70F0_0901,
        0x70F0_0902,
        0x70F0_0903,
        0x70F0_0904,
        0x70F0_0905,
        0x70F0_0906,
    );
    let records: Vec<(u32, PropertySet)> = vec![
        (
            filtered,
            set(vec![a(
                "LootGen_FilteredTreasureListArray",
                vec![s(
                    "LootGen_FilteredTreasureListArray",
                    vec![
                        d("LootGen_FilteredTrophyTable_EntryDID", weighted),
                        a("EntityFilter_Array", vec![]),
                    ],
                )],
            )]),
        ),
        (
            weighted,
            set(vec![a(
                "LootGen_WeightedTreasureTable",
                vec![s(
                    "LootGen_WeightedTreasureTable",
                    vec![
                        d("LootGen_WeightedTreasureTable_TrophyList", trophy),
                        int("LootGen_WeightedTreasureTable_TrophyList_Weight", 1),
                    ],
                )],
            )]),
        ),
        (
            trophy,
            set(vec![a(
                "LootGen_TrophyList",
                vec![s(
                    "LootGen_TrophyList",
                    vec![
                        d("LootGen_TrophyList_ItemOrProfile", treasure),
                        key("LootGen_TrophyList_DropFrequency", 2),
                    ],
                )],
            )]),
        ),
        (
            treasure,
            set(vec![a(
                "LootGen_TreasureList",
                vec![s(
                    "LootGen_TreasureList",
                    vec![
                        d("LootGen_TreasureList_GroupProfile", profile),
                        int("LootGen_TreasureList_Weight", 1),
                    ],
                )],
            )]),
        ),
        (
            profile,
            set(vec![a(
                "TreasureGroupProfile_ItemTable",
                vec![s(
                    "TreasureGroupProfile_ItemTable",
                    vec![
                        d("TreasureGroupProfile_ItemTable_Item", item),
                        int("TreasureGroupProfile_ItemTable_Weight", 1),
                    ],
                )],
            )]),
        ),
        (
            0x70F0_0907,
            set(vec![a(
                "VendorInventory_Items",
                vec![d("VendorInventory_Items", item)],
            )]),
        ),
        (
            0x70F0_0908,
            barter(vec![vec![cost(0x70F0_0011, 2), reward(item, 1)]]),
        ),
        (0x70F0_0909, recipe(vec![])),
        // Mob: attaches the filtered list, sells from the inventory, trades the profile.
        (
            mob,
            set(vec![
                d("LootGen_TrophyList_ItemOrProfile", filtered),
                a(
                    "Vendor_InventoryList",
                    vec![d("Vendor_InventoryList", 0x70F0_0907)],
                ),
                a(
                    "Barter_ProfileArray",
                    vec![d("Barter_ProfileArray", 0x70F0_0908)],
                ),
            ]),
        ),
        // Recipe item.
        (0x70F0_090A, set(vec![d("RecipeItem_Recipe", 0x70F0_0909)])),
        // The item itself names a trophy list (a package): not a loot table, walk stops.
        (
            item,
            set(vec![d("LootGen_TrophyList_ItemOrProfile", trophy)]),
        ),
    ];
    let decoded: Vec<_> = records
        .iter()
        .map(|(did, s)| (Did(*did), s, r.decode(s).unwrap()))
        .collect();
    let mut links = CrossLinks::new();
    for (did, _, rec) in &decoded {
        if let Some(rec) = rec {
            links.note_kind(*did, rec.kind());
        }
    }
    for (did, s, rec) in &decoded {
        links.add(&r, *did, s, rec.as_ref()).unwrap();
    }

    let sources = links.drops_from(Did(item), DROPS_FROM_DEPTH);
    let mob_hit = sources.iter().find(|s| s.source == Did(mob)).unwrap();
    assert_eq!(mob_hit.depth, 6);
    assert_eq!(mob_hit.kind, None);
    assert_eq!(
        mob_hit.path,
        vec![
            Did(filtered),
            Did(weighted),
            Did(trophy),
            Did(treasure),
            Did(profile)
        ]
    );
    // The package item reaches itself only through the visited set.
    assert!(sources.iter().all(|s| s.source != Did(item)));
    assert_eq!(sources.len(), 1);
    // A shallow walk does not reach the mob.
    assert!(links.drops_from(Did(item), 5).is_empty());

    assert_eq!(links.sold_by(Did(item)), vec![(Did(mob), Did(0x70F0_0907))]);
    assert_eq!(links.bartered_for(Did(item)), vec![Did(0x70F0_0908)]);
    assert_eq!(
        links.barter_currency_in(Did(0x70F0_0011)),
        vec![Did(0x70F0_0908)]
    );
    assert_eq!(links.traders(Did(0x70F0_0908)), vec![Did(mob)]);
    assert_eq!(links.crafted_by(Did(item)), vec![Did(0x70F0_0909)]);
    assert_eq!(links.used_in(Did(0x70F0_0011)), vec![Did(0x70F0_0909)]);
    assert_eq!(links.learned_from(Did(0x70F0_0909)), vec![Did(0x70F0_090A)]);

    // E-I14.
    let stats = links.loot_graph();
    assert!(stats.acyclic);
    assert_eq!(stats.tables, 5);
    assert_eq!(stats.longest_path, 5);

    // A cycle among loot tables is detected.
    let back = set(vec![a(
        "TreasureGroupProfile_ItemTable",
        vec![s(
            "TreasureGroupProfile_ItemTable",
            vec![d("TreasureGroupProfile_ItemTable_Item", filtered)],
        )],
    )]);
    let rec = r.decode(&back).unwrap();
    links.add(&r, Did(profile), &back, rec.as_ref()).unwrap();
    assert!(!links.loot_graph().acyclic);
}

#[test]
fn barter_elements_of_other_kinds_are_kept() {
    let r = reader();
    let profile = barter(vec![vec![cost(1, 1), int("Item_Level", 1), reward(2, 1)]]);
    let b = r.barter_profile(&profile).unwrap();
    assert!(matches!(b.lists[0].elements[1], BarterElement::Other(_)));
    assert_eq!(b.lists[0].shape(), None);
    assert!(matches!(
        b.lists[0].reward(),
        Some(BarterReward::Item {
            quantity: Some(1),
            ..
        })
    ));
}

proptest! {
    #[test]
    fn weighted_tables_round_trip(weights in proptest::collection::vec(0u32..1_000_000, 0..64)) {
        let t = WeightedTable::from_weights(&weights);
        prop_assert!(t.sums_hold());
        prop_assert!(t.indices_hold());
        prop_assert_eq!(u64::from(t.total()), weights.iter().map(|&w| u64::from(w)).sum::<u64>());
        prop_assert_eq!(WeightedTable::parse(&t.to_bytes()).unwrap(), t);
    }

    #[test]
    fn cumulative_pick_matches_weights(weights in proptest::collection::vec(0u32..50, 1..16)) {
        let total: u64 = weights.iter().map(|&w| u64::from(w)).sum();
        let mut counts = vec![0u64; weights.len()];
        for r in 0..total {
            let i = cumulative_pick(&weights, r).unwrap();
            counts[i] += 1;
        }
        let expected: Vec<u64> = weights.iter().map(|&w| u64::from(w)).collect();
        prop_assert_eq!(counts, expected);
        prop_assert_eq!(cumulative_pick(&weights, total), None);
    }

    #[test]
    fn barter_entries_round_trip(words in proptest::array::uniform12(any::<u32>()), flag in any::<bool>()) {
        let e = BarterEntry {
            profile: Did(words[0]),
            reward_item: Did(words[1]),
            reward_quantity: words[2],
            uses_min_usage_level: flag,
            reputation_faction: Did(words[3]),
            reputation_tier: words[4],
            costs: [
                (Did(words[5]), words[6]),
                (Did(words[7]), words[8]),
                (Did(words[9]), words[10]),
            ],
            sound_category: words[11],
        };
        prop_assert_eq!(BarterEntry::parse(&e.to_bytes()).unwrap(), e);
    }
}
