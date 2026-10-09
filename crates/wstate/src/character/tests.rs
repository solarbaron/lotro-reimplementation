// SPDX-License-Identifier: GPL-3.0-or-later
//! Synthetic tests of the player-character views (`player-character.md` §19 worked
//! examples and the view rules). Every value here is made up.

use std::collections::BTreeMap;

use tessera_formats::{Did, StringInfo, StringSource, Utf16String};
use tessera_props::{MemorySource, Property, PropertyRegistry, PropertySet, Value};

use super::*;
use crate::native::{NestedGroup, NestedInner, NestedOuter, NestedStringTable};
use crate::state::{DecodeContext, ObjectState};
use crate::testkit::{
    RecordBuilder, character_dictionary, did_map, property_set_record, put_tsize, put_utf16,
};
use crate::{ObjectStore, WStateError};

fn registry() -> PropertyRegistry {
    PropertyRegistry::parse(&character_dictionary()).expect("synthetic dictionary")
}

fn pid(reg: &PropertyRegistry, name: &str) -> u32 {
    reg.pid(name)
        .unwrap_or_else(|| panic!("{name} not in the synthetic dictionary"))
}

fn p(reg: &PropertyRegistry, name: &str, value: Value) -> Property {
    Property::new(pid(reg, name), value)
}

fn set(props: Vec<Property>) -> PropertySet {
    PropertySet::from_properties(props)
}

fn st(props: Vec<Property>) -> Value {
    Value::Struct(Box::new(set(props)))
}

fn lit(s: &str) -> StringInfo {
    StringInfo {
        source: StringSource::Literal(Utf16String::from(s)),
        args: None,
    }
}

/// A table reference whose token resolves nowhere (an "absent" token, §8.2).
fn absent() -> StringInfo {
    StringInfo {
        source: StringSource::Table {
            token: 7,
            table: Did(0x2500_0001),
        },
        args: None,
    }
}

fn resolve(info: &StringInfo) -> Option<String> {
    match &info.source {
        StringSource::Literal(u) => Some(u.to_string_lossy()),
        StringSource::Table { .. } => None,
    }
}

fn entry(e: &str, s: &str, l: &str, r: &str) -> ShiftEntry {
    ShiftEntry {
        end: e.into(),
        start: s.into(),
        left: l.into(),
        right: r.into(),
    }
}

/// The table of §19.1, with the absent *L′* of entry 4 already resolved to empty.
fn example_table() -> ShiftTable {
    ShiftTable::compile(&[
        entry("a", "e", "a", ""),
        entry("ia", "e", "i", "ye"),
        entry("n ", "d", "n", "nd"),
        entry("ia", "el", "", "al"),
    ])
}

#[test]
fn phonetic_shift_compiled_layout_19_1() {
    let t = example_table();
    assert_eq!(t.left_lengths, vec![3, 2]);
    assert_eq!(t.right_lengths, vec![3, 2]);
    assert!(t.is_well_formed());
    assert_eq!(t.len(), 4);
    let (l3, g3) = &t.by_left[0];
    assert_eq!(*l3, 3);
    assert_eq!(g3.len(), 1);
    assert_eq!(g3[0].text, "ia");
    assert_eq!(
        g3[0].by_right.iter().map(|(k, _)| *k).collect::<Vec<_>>(),
        vec![3, 2]
    );
    let (l2, g2) = &t.by_left[1];
    assert_eq!(*l2, 2);
    assert_eq!(
        g2.iter().map(|g| g.text.as_str()).collect::<Vec<_>>(),
        vec!["a", "n"]
    );
}

#[test]
fn phonetic_shift_applications_19_1() {
    let t = example_table();
    assert_eq!(
        t.apply("Kalia", "ero"),
        Some(("Kali".to_owned(), "yero".to_owned()))
    );
    assert_eq!(t.join("Kalia", "ero"), "Kaliyero");
    assert_eq!(
        t.apply("KALIA", "Elm"),
        Some(("KAL".to_owned(), "alm".to_owned()))
    );
    assert_eq!(t.join("KALIA", "Elm"), "KALalm");
    assert_eq!(
        t.apply("Toran", "dor"),
        Some(("Toran".to_owned(), "ndor".to_owned()))
    );
    assert_eq!(t.join("Toran", "dor"), "Toranndor");
    assert_eq!(t.apply("Bo", "ra"), None);
    assert_eq!(t.join("Bo", "ra"), "Bora");
}

#[test]
fn longer_end_match_beats_longer_start_match() {
    // §8.4: a longer E wins even when a shorter E has a longer S; stored order never decides.
    let t = ShiftTable::compile(&[entry("a", "xyz", "1", ""), entry("ba", "x", "2", "")]);
    assert_eq!(t.join("ba", "xyzq"), "2yzq");
    let t = ShiftTable::compile(&[entry("ba", "x", "2", ""), entry("a", "xyz", "1", "")]);
    assert_eq!(t.join("ba", "xyzq"), "2yzq");
    // When the longest E has no matching S, a shorter E is tried.
    let t = ShiftTable::compile(&[entry("ba", "q", "2", ""), entry("a", "x", "1", "")]);
    assert_eq!(t.join("ba", "xr"), "b1r");
}

#[test]
fn shift_application_is_local_pc17() {
    // PC17: only the end of A and the start of B change, and only when both match.
    let t = example_table();
    for (a, b) in [
        ("Kalia", "ero"),
        ("Toran", "dor"),
        ("KALIA", "Elm"),
        ("x", ""),
        ("", "y"),
    ] {
        if let Some((na, nb)) = t.apply(a, b) {
            let ac: Vec<char> = a.chars().collect();
            let common = ac
                .iter()
                .zip(na.chars())
                .take_while(|(x, y)| **x == *y)
                .count();
            assert!(common + 3 >= ac.len(), "{a}: more than the end changed");
            assert!(nb.len() + 3 >= b.len());
        }
    }
    assert_eq!(t.apply("", ""), None);
}

#[test]
fn definition_compile_resolves_trims_and_empties() {
    let def = ShiftTableDefinition {
        key: 1,
        entries: vec![
            ShiftEntryInfo {
                end_match: Some(lit("n\t")),
                start_match: Some(lit(" d")),
                left_replacement: Some(lit("n")),
                right_replacement: Some(lit("nd ")),
            },
            ShiftEntryInfo {
                end_match: Some(lit("ia")),
                start_match: Some(lit("el")),
                left_replacement: Some(absent()),
                right_replacement: None,
            },
        ],
    };
    let t = def.compile(resolve).expect("compiles");
    // Replacements are kept as stored (the trailing space of "nd " stays).
    assert_eq!(t.join("Toran", "dor"), "Torannd or");
    assert_eq!(t.join("Kalia", "elm"), "Kalm");
    // An end or start match that does not resolve fails the load (§8.4).
    let bad = ShiftTableDefinition {
        key: 2,
        entries: vec![ShiftEntryInfo {
            end_match: Some(absent()),
            start_match: Some(lit("a")),
            ..ShiftEntryInfo::default()
        }],
    };
    assert!(matches!(
        bad.compile(resolve),
        Err(WStateError::BadProperty { .. })
    ));
}

fn native_of(t: &ShiftTable) -> NestedStringTable {
    NestedStringTable {
        size: tessera_formats::TSize {
            hint: 1,
            count: t.by_left.len() as u32,
        },
        outer: t
            .by_left
            .iter()
            .map(|(k, groups)| NestedOuter {
                key: *k,
                groups: groups
                    .iter()
                    .map(|g| NestedGroup {
                        text: Utf16String::from(g.text.as_str()),
                        size: tessera_formats::TSize {
                            hint: 1,
                            count: g.by_right.len() as u32,
                        },
                        inner: g
                            .by_right
                            .iter()
                            .map(|(rk, triples)| NestedInner {
                                key: *rk,
                                triples: triples
                                    .iter()
                                    .map(|x| {
                                        [
                                            Utf16String::from(x.start.as_str()),
                                            Utf16String::from(x.left.as_str()),
                                            Utf16String::from(x.right.as_str()),
                                        ]
                                    })
                                    .collect(),
                            })
                            .collect(),
                    })
                    .collect(),
            })
            .collect(),
        list_a: t.left_lengths.clone(),
        list_b: t.right_lengths.clone(),
    }
}

fn encode_native(n: &NestedStringTable) -> Vec<u8> {
    let mut b = Vec::new();
    put_tsize(&mut b, n.size.hint, n.outer.len() as u32);
    for o in &n.outer {
        b.extend_from_slice(&o.key.to_le_bytes());
        b.extend_from_slice(&(o.groups.len() as u32).to_le_bytes());
        for g in &o.groups {
            put_utf16(&mut b, &g.text.to_string_lossy());
            put_tsize(&mut b, g.size.hint, g.inner.len() as u32);
            for i in &g.inner {
                b.extend_from_slice(&i.key.to_le_bytes());
                b.extend_from_slice(&(i.triples.len() as u32).to_le_bytes());
                for tr in &i.triples {
                    for s in tr {
                        put_utf16(&mut b, &s.to_string_lossy());
                    }
                }
            }
        }
    }
    for list in [&n.list_a, &n.list_b] {
        b.extend_from_slice(&(list.len() as u32).to_le_bytes());
        for v in list {
            b.extend_from_slice(&v.to_le_bytes());
        }
    }
    b
}

#[test]
fn class_3053_decodes_and_equals_its_source_pc11() {
    let t = example_table();
    let native = native_of(&t);
    let body = encode_native(&native);
    let rec = RecordBuilder::new(0x7000_0ABC, 3)
        .class(3, &[])
        .script(0, 3, &[])
        .object(1, 3053, &body);
    let state = ObjectState::parse(&rec.build(), Did(0x7000_0ABC), DecodeContext::bare())
        .expect("record decodes");
    assert!(state.is_complete());
    let decoded = state.items()[1]
        .as_native()
        .and_then(|n| n.as_nested_string_table())
        .expect("nested string table");
    assert_eq!(decoded, &native);
    let back = ShiftTable::from_native(decoded);
    assert!(back.is_well_formed());
    assert_eq!(back.canonical_entries(), t.canonical_entries());
    assert_eq!(back.join("Kalia", "ero"), "Kaliyero");
    // Truncated bodies fail cleanly.
    for cut in 0..body.len() {
        let rec = RecordBuilder::new(0x7000_0ABC, 3)
            .class(3, &[])
            .script(0, 3, &[])
            .object(1, 3053, &body[..cut]);
        let r = ObjectState::parse(&rec.build(), Did(0x7000_0ABC), DecodeContext::bare());
        assert!(
            r.is_err() || r.is_ok_and(|s| s.items().len() < 2),
            "cut {cut}"
        );
    }
}

#[test]
fn pip_effective_maximum_19_2() {
    let reg = registry();
    let modifier = 0x55;
    let pip = Pip::from_properties(
        &set(vec![
            p(&reg, "Pip_Type", Value::Enum(3)),
            p(&reg, "Pip_Max", Value::Int(20)),
            p(&reg, "Pip_Min", Value::Int(0)),
            p(&reg, "Pip_MaxModifier", Value::PropertyId(modifier)),
        ]),
        &reg,
    )
    .expect("pip");
    let cases: [(Option<Value>, i32); 5] = [
        (None, 20),
        (Some(Value::Int(-4)), 16),
        (Some(Value::Int(5)), 20),
        (Some(Value::Int(-30)), 0),
        (Some(Value::Float(-4.0)), 20),
    ];
    for (value, want) in cases {
        let got = pip.effective_max(|p| {
            assert_eq!(p, modifier);
            value.as_ref()
        });
        assert_eq!(got, want, "{value:?}");
    }
    // No modifier, or modifier 0: the stored maximum.
    let plain = Pip {
        max: Some(7),
        max_modifier: Some(0),
        ..Pip::default()
    };
    assert_eq!(plain.effective_max(|_| panic!("not read")), 7);
    assert_eq!(Pip::default().effective_max(|_| None), -1);
}

#[test]
fn experience_lookup_19_3() {
    let t = ExperienceTable {
        totals: vec![0, 0, 100, 250, 450],
    };
    assert!(t.is_valid());
    for (xp, level) in [(0, 1), (99, 1), (100, 2), (260, 3), (10_000, 4)] {
        assert_eq!(t.level_for(xp), Some(level), "{xp}");
    }
    assert_eq!(t.max_level(), Some(4));
    let ranked = ExperienceTable {
        totals: vec![0, 10, 20],
    };
    assert_eq!(ranked.level_for(5), None);
    assert!(!ExperienceTable { totals: vec![1, 2] }.is_valid());
    assert!(
        !ExperienceTable {
            totals: vec![0, 5, 4]
        }
        .is_valid()
    );
}

#[test]
fn experience_and_group_tables_from_compiled_half() {
    let mut arr = 3u32.to_le_bytes().to_vec();
    for v in [0u64, 0, 100] {
        arr.extend_from_slice(&v.to_le_bytes());
    }
    let mut map = Vec::new();
    put_tsize(&mut map, 1, 2);
    for (k, f) in [(1u32, 1.0f32), (2, 1.5)] {
        map.extend_from_slice(&k.to_le_bytes());
        map.extend_from_slice(&f.to_bits().to_le_bytes());
    }
    let rec = RecordBuilder::new(0x7000_0001, 3)
        .class(3, &[])
        .script(0, 3, &[])
        .object(1, 104, &arr)
        .object(2, 11, &map);
    let state =
        ObjectState::parse(&rec.build(), Did(0x7000_0001), DecodeContext::bare()).expect("decodes");
    let xp = ExperienceTable::from_state(&state)
        .expect("ok")
        .expect("table");
    assert_eq!(xp.totals, vec![0, 0, 100]);
    let g = GroupModifierTable::from_state(&state)
        .expect("ok")
        .expect("table");
    assert_eq!(g.factor(2), Some(1.5));
    let empty = RecordBuilder::new(0x7000_0002, 3)
        .class(3, &[])
        .script(0, 3, &[]);
    let state = ObjectState::parse(&empty.build(), Did(0x7000_0002), DecodeContext::bare())
        .expect("decodes");
    assert_eq!(ExperienceTable::from_state(&state).expect("ok"), None);
}

fn race_example(reg: &PropertyRegistry) -> PropertySet {
    let gender = |g: u32, avatar: u32| {
        p(
            reg,
            "RaceTable_GenderStruct",
            st(vec![
                p(reg, "RaceTable_GenderType", Value::Enum(g)),
                p(
                    reg,
                    "RaceTable_Gender_PlayerAvatar",
                    Value::DidRef(Did(avatar)),
                ),
                p(
                    reg,
                    "RaceTable_Gender_AdminAvatar",
                    Value::DidRef(Did(avatar + 1)),
                ),
            ]),
        )
    };
    set(vec![
        p(reg, "RaceTable_Race", Value::Enum(0x10)),
        p(
            reg,
            "RaceTable_GenderList",
            Value::Array(vec![
                gender(0x1000, 0x7000_0100),
                gender(0x2000, 0x7000_0200),
            ]),
        ),
        p(
            reg,
            "RaceTable_ClassList",
            Value::Array(vec![
                p(reg, "RaceTable_Class", Value::Enum(0x20)),
                p(reg, "RaceTable_Class", Value::Enum(0x21)),
            ]),
        ),
        p(
            reg,
            "RaceTable_NationalityList",
            Value::Array(vec![p(reg, "RaceTable_Nationality", Value::Enum(0x3))]),
        ),
        p(reg, "RaceTable_IsTallRace", Value::Bool(true)),
    ])
}

#[test]
fn creation_check_19_4() {
    let reg = registry();
    let race = RaceRecord::from_properties(&race_example(&reg), &reg).expect("race");
    assert_eq!(race.race, 0x10);
    assert_eq!(race.genders.len(), 2);
    assert_eq!(
        race.gender(0x2000).and_then(|g| g.player_avatar),
        Some(Did(0x7000_0200))
    );
    assert_eq!(race.tall_race, Some(true));
    let races = [race];
    let c = |species, nationality, class| CreationChoice {
        species,
        sex: 0x2000,
        nationality,
        class,
    };
    assert_eq!(
        check_choice(&races, &c(0x10, 0x3, 0x21)),
        ChoiceVerdict::Valid
    );
    assert_eq!(
        check_choice(&races, &c(0x10, 0x4, 0x21)),
        ChoiceVerdict::NationalityNotAllowed
    );
    assert_eq!(
        check_choice(&races, &c(0x10, 0x3, 0x22)),
        ChoiceVerdict::ClassNotAllowed
    );
    assert_eq!(
        check_choice(&races, &c(0x11, 0x3, 0x21)),
        ChoiceVerdict::NoSuchRaceAndSex
    );
}

#[test]
fn threshold_band_19_5() {
    let reg = registry();
    let state = |min: Option<i32>, max: Option<i32>| {
        let mut v = Vec::new();
        if let Some(m) = min {
            v.push(p(&reg, "Threshold_State_MinValue_Integer", Value::Int(m)));
        }
        if let Some(m) = max {
            v.push(p(&reg, "Threshold_State_MaxValue_Integer", Value::Int(m)));
        }
        p(&reg, "Threshold_EffectScript_State", st(v))
    };
    let pip = Pip::from_properties(
        &set(vec![
            p(&reg, "Pip_Type", Value::Enum(1)),
            p(&reg, "Pip_Max", Value::Int(8)),
            p(
                &reg,
                "Threshold_Pip_IntegerValue_Policy",
                st(vec![p(
                    &reg,
                    "Threshold_Policy_StateList",
                    Value::Array(vec![
                        state(None, None),
                        state(Some(1), Some(3)),
                        state(Some(4), Some(7)),
                        state(Some(8), Some(8)),
                    ]),
                )]),
            ),
        ]),
        &reg,
    )
    .expect("pip");
    let policy = pip.threshold.as_ref().expect("policy");
    assert_eq!(
        policy
            .states
            .iter()
            .map(ThresholdState::band)
            .collect::<Vec<_>>(),
        vec![(0, 0), (1, 3), (4, 7), (8, 8)]
    );
    // Value 5 lies in the third state; which index the state property holds is OPEN (Q15).
    assert_eq!(policy.state_position(5), Some(2));
    assert_eq!(policy.state_position(9), None);
    assert!(policy.bands_are_consistent(8));
    assert!(!policy.bands_are_consistent(9));
}

#[test]
fn views_reject_wrong_types_and_missing_keys() {
    let reg = registry();
    let no_race = set(vec![p(&reg, "RaceTable_IsTallRace", Value::Bool(false))]);
    assert!(matches!(
        RaceRecord::from_properties(&no_race, &reg),
        Err(WStateError::BadProperty {
            property: "RaceTable_Race",
            ..
        })
    ));
    let wrong = set(vec![p(&reg, "RaceTable_Race", Value::Int(4))]);
    assert!(matches!(
        RaceRecord::from_properties(&wrong, &reg),
        Err(WStateError::BadProperty { .. })
    ));
    let not_struct = set(vec![
        p(&reg, "RaceTable_Race", Value::Enum(4)),
        p(
            &reg,
            "RaceTable_GenderList",
            Value::Array(vec![p(&reg, "Test_EnumElement", Value::Enum(1))]),
        ),
    ]);
    assert!(RaceRecord::from_properties(&not_struct, &reg).is_err());
    // An empty set is a valid (empty) directory.
    assert_eq!(
        RaceDirectory::from_properties(&PropertySet::new(), &reg).expect("empty"),
        RaceDirectory::default()
    );
    // Too many entries.
    let many: Vec<Property> = (0..=MAX_VIEW_ENTRIES)
        .map(|i| {
            p(
                &reg,
                "PipControl_Entry",
                Value::DidRef(Did(0x7000_0000 + i as u32)),
            )
        })
        .collect();
    let big = set(vec![p(&reg, "PipControl_Directory", Value::Array(many))]);
    assert!(matches!(
        PipControl::from_properties(&big, &reg),
        Err(WStateError::TooMany { .. })
    ));
}

#[test]
fn nationality_view() {
    let reg = registry();
    let n = NationalityRecord::from_properties(
        &set(vec![
            p(&reg, "NationalityTable_Nationality", Value::Enum(3)),
            p(
                &reg,
                "NationalityTable_Naming_Guideline_Array",
                Value::Array(vec![
                    p(
                        &reg,
                        "NationalityTable_Naming_Guideline_Struct",
                        st(vec![p(&reg, "NationalityTable_Sex", Value::Enum(0x1000))]),
                    ),
                    p(
                        &reg,
                        "NationalityTable_Naming_Guideline_Struct",
                        st(vec![p(&reg, "NationalityTable_Sex", Value::Enum(0x2000))]),
                    ),
                ]),
            ),
            p(
                &reg,
                "AppearanceUI_DyeMapControlOverrides",
                Value::Array(vec![p(
                    &reg,
                    "AppearanceUI_DyeMapControlOverride",
                    st(vec![
                        p(
                            &reg,
                            "AppearanceUI_ControlPropertyName",
                            Value::PropertyId(9),
                        ),
                        p(
                            &reg,
                            "AppearanceUI_DyeMapImage",
                            Value::DidRef(Did(0x4100_0001)),
                        ),
                    ]),
                )]),
            ),
        ]),
        &reg,
    )
    .expect("nationality");
    assert_eq!(n.nationality, 3);
    assert_eq!(n.naming_sexes(), vec![0x1000, 0x2000]);
    assert_eq!(n.dye_overrides[0].control_property, Some(9));
    assert_eq!(n.title, None);
}

#[test]
fn mirror_names_and_default_character_set() {
    assert_eq!(mirror_base("Forward_Cached_Vital_X"), Some("Vital_X"));
    assert_eq!(mirror_base("Vital_X_Cached"), Some("Vital_X"));
    assert_eq!(mirror_base("ForwardSource_Movement_Y"), Some("Movement_Y"));
    assert_eq!(mirror_base("Movement_Y_ForwardSource"), Some("Movement_Y"));
    assert_eq!(mirror_base("A_Shadow"), Some("A"));
    assert_eq!(mirror_base("A_Base"), Some("A"));
    assert_eq!(mirror_base("_Cached"), None);
    assert_eq!(mirror_base("Agent_Species"), None);

    let props: Vec<(u32, &str, u32)> = vec![
        (1, "Sex", 8),
        (2, "Advancement_Level", 15),
        (3, "Advancement_Level_Cached", 15),
        (4, "Forward_Cached_Advancement_Level", 15),
    ];
    let reg =
        PropertyRegistry::parse(&crate::testkit::property_dictionary(&props)).expect("dictionary");
    let avatar = set(vec![
        Property::new(1, Value::Enum(0x1000)),
        Property::new(2, Value::Int(1)),
        Property::new(3, Value::Int(1)),
        Property::new(4, Value::Int(1)),
    ]);
    let out = default_character_properties(&avatar, &reg, 0x2000).expect("built");
    assert_eq!(out.get(1), Some(&Value::Enum(0x2000)));
    assert_eq!(out.get(2), Some(&Value::Int(1)));
    assert_eq!(out.len(), 2);
}

#[test]
fn class_priority_and_creation_fields() {
    let reg = registry();
    let entry = |class: u32, prio: i32| {
        p(
            &reg,
            "CharGen_ClassPriority_Entry",
            st(vec![
                p(&reg, "CharGen_ClassPriority_ClassType", Value::Enum(class)),
                p(
                    &reg,
                    "CharGen_ClassPriority_PriorityValue",
                    Value::Int(prio),
                ),
            ]),
        )
    };
    let cp = ClassPriority::from_properties(
        &set(vec![p(
            &reg,
            "CharGen_ClassPriority_Array",
            Value::Array(vec![entry(0x20, 3), entry(0x21, 1), entry(0x22, 2)]),
        )]),
        &reg,
    )
    .expect("priority");
    assert_eq!(cp.display_order(), vec![0x21, 0x22, 0x20]);

    let item = |did: u32, race: u32| {
        p(
            &reg,
            "AdvTable_StartingInventory_Entry",
            st(vec![
                p(
                    &reg,
                    "AdvTable_StartingInventory_Item",
                    Value::DidRef(Did(did)),
                ),
                p(&reg, "AdvTable_StartingInventory_Quantity", Value::Int(1)),
                p(
                    &reg,
                    "AdvTable_StartingInventory_StartsEquipped",
                    Value::Bool(true),
                ),
                p(
                    &reg,
                    "AdvTable_StartingInventory_RequiredRace",
                    Value::Enum(race),
                ),
            ]),
        )
    };
    let info = ClassCreationInfo::from_properties(
        &set(vec![
            p(&reg, "AdvTable_Class", Value::Enum(0x20)),
            p(
                &reg,
                "AdvTable_StartingInventory_List",
                Value::Array(vec![
                    item(0x7000_0010, 0),
                    item(0x7000_0011, 0x10),
                    item(0x7000_0012, 0x11),
                ]),
            ),
            p(
                &reg,
                "PipConfig_Pip_List",
                Value::Array(vec![p(&reg, "Test_EnumElement", Value::Enum(5))]),
            ),
        ]),
        &reg,
    )
    .expect("class");
    let mine: Vec<_> = info
        .starting_items_for(0x10)
        .filter_map(|e| e.item)
        .collect();
    assert_eq!(mine, vec![Did(0x7000_0010), Did(0x7000_0011)]);
    assert_eq!(info.pips, vec![5]);
}

#[test]
fn bank_schedule_and_allegiances_pc15_pc16() {
    let reg = registry();
    let step = |req: i32, inc: i32| {
        p(
            &reg,
            "BankRepository_Upgrade_Struct",
            st(vec![
                p(
                    &reg,
                    "BankRepository_Upgrade_RequiredCapacity",
                    Value::Int(req),
                ),
                p(
                    &reg,
                    "BankRepository_Upgrade_IncreasedCapacity",
                    Value::Int(inc),
                ),
            ]),
        )
    };
    let s = BankUpgradeSchedule::from_properties(
        &set(vec![p(
            &reg,
            "BankRepository_UpgradeProgression_Array",
            Value::Array(vec![step(30, 15), step(45, 15), step(60, 15)]),
        )]),
        &reg,
    )
    .expect("schedule");
    assert!(s.is_consistent());
    assert_eq!(s.final_capacity(), Some(75));
    let broken = BankUpgradeSchedule {
        steps: vec![
            UpgradeStep {
                required_capacity: Some(0),
                increase: Some(15),
                cost: None,
            },
            UpgradeStep {
                required_capacity: Some(16),
                increase: Some(15),
                cost: None,
            },
        ],
    };
    assert!(!broken.is_consistent());

    let a1 = Did(0x7000_0301);
    let a2 = Did(0x7000_0302);
    let control = AllegianceControl {
        visibility_level: Some(20),
        types: vec![a1, a2],
        progressions: vec![],
        groups: vec![
            AllegianceGroup {
                name: Some(1),
                min_level: Some(10),
                allegiances: vec![a1],
            },
            AllegianceGroup {
                name: Some(2),
                min_level: Some(30),
                allegiances: vec![a2],
            },
        ],
    };
    let good = Allegiance {
        group: Some(1),
        min_level: Some(10),
        ..Allegiance::default()
    };
    let bad = Allegiance {
        group: Some(1),
        min_level: Some(10),
        ..Allegiance::default()
    };
    assert!(allegiance_problems(&control, &[(a1, good.clone())]).is_empty());
    assert_eq!(
        allegiance_problems(&control, &[(a1, good), (a2, bad)]).len(),
        2
    );
}

fn generator_example(reg: &PropertyRegistry) -> PropertySet {
    let syllables = |key: u32, strings: &[&str]| {
        p(
            reg,
            "RandomNameGeneration_SyllableTable_Definition",
            st(vec![
                p(
                    reg,
                    "RandomNameGeneration_SyllableTable_Key",
                    Value::Enum(key),
                ),
                p(
                    reg,
                    "RandomNameGeneration_SyllableTable_StringsList",
                    Value::Array(
                        strings
                            .iter()
                            .map(|s| {
                                p(
                                    reg,
                                    "RandomNameGeneration_SyllableTable_String",
                                    Value::StringInfo(Box::new(lit(s))),
                                )
                            })
                            .collect(),
                    ),
                ),
            ]),
        )
    };
    let shift = |e: &str, s: &str, l: &str, r: &str| {
        let si = |v: &str| Value::StringInfo(Box::new(lit(v)));
        p(
            reg,
            "RandomNameGeneration_PhoneticShiftsTable_Struct",
            st(vec![
                p(
                    reg,
                    "RandomNameGeneration_PhoneticShiftsTable_LeftString_EndMatch",
                    si(e),
                ),
                p(
                    reg,
                    "RandomNameGeneration_PhoneticShiftsTable_RightString_StartMatch",
                    si(s),
                ),
                p(
                    reg,
                    "RandomNameGeneration_PhoneticShiftsTable_LeftString_Replacement",
                    si(l),
                ),
                p(
                    reg,
                    "RandomNameGeneration_PhoneticShiftsTable_RightString_Replacement",
                    si(r),
                ),
            ]),
        )
    };
    let enum_list = |name: &str, keys: &[u32]| {
        p(
            reg,
            name,
            Value::Array(
                keys.iter()
                    .map(|&k| p(reg, "Test_EnumElement", Value::Enum(k)))
                    .collect(),
            ),
        )
    };
    let option = |weight: i32, tables: &[u32], shifts: &[u32]| {
        p(
            reg,
            "NameGenerationCategory_Option",
            st(vec![
                p(
                    reg,
                    "NameGenerationCategory_Option_Weight",
                    Value::Int(weight),
                ),
                enum_list("NameGenerationCategory_Option_SyllableTables", tables),
                enum_list("NameGenerationCategory_Option_PhoneticShiftsTables", shifts),
            ]),
        )
    };
    set(vec![
        p(
            reg,
            "RandomNameGeneration_RaceToCategoriesArray",
            Value::Array(vec![p(
                reg,
                "RandomNameGeneration_RaceToCategories_Entry",
                st(vec![
                    p(reg, "NameGenerationCategory_Race", Value::Enum(0x10)),
                    p(reg, "NameGenerationCategory_Gender", Value::Enum(0x1000)),
                    p(reg, "RandomNameGeneration_CategoryType", Value::Enum(7)),
                ]),
            )]),
        ),
        p(
            reg,
            "RandomNameGenerationRules",
            Value::Array(vec![p(
                reg,
                "NameGenerationCategory",
                st(vec![
                    p(reg, "RandomNameGeneration_CategoryType", Value::Enum(7)),
                    p(
                        reg,
                        "NameGenerationCategory_Options",
                        Value::Array(vec![option(0, &[1, 2], &[1]), option(3, &[1, 2], &[1])]),
                    ),
                ]),
            )]),
        ),
        p(
            reg,
            "RandomNameGeneration_SyllableTableDefinitions",
            Value::Array(vec![
                syllables(1, &["Kalia", "Toran"]),
                syllables(2, &["ero", "dor"]),
            ]),
        ),
        p(
            reg,
            "RandomNameGeneration_PhoneticShiftsTableDefinitions",
            Value::Array(vec![p(
                reg,
                "RandomNameGeneration_PhoneticShiftsTable_Definition",
                st(vec![
                    p(
                        reg,
                        "RandomNameGeneration_PhoneticShiftsTable_Key",
                        Value::Enum(1),
                    ),
                    p(
                        reg,
                        "RandomNameGeneration_PhoneticShiftsTable_StructList",
                        Value::Array(vec![
                            shift("a", "e", "a", ""),
                            shift("ia", "e", "i", "ye"),
                            shift("n ", "d", "n", "nd"),
                        ]),
                    ),
                ]),
            )]),
        ),
        enum_list("RandomNameGeneration_ValidNameRules", &[1, 2]),
    ])
}

#[test]
fn name_generator_view_and_generation() {
    let reg = registry();
    let g = NameGenerator::from_properties(&generator_example(&reg), &reg).expect("generator");
    assert_eq!(g.category_for(0x10, 0x1000), Some(7));
    assert_eq!(g.category_for(0x10, 0x2000), None);
    assert_eq!(g.category(7).map(|c| c.options.len()), Some(2));
    assert_eq!(g.valid_rules, vec![1, 2]);
    let text = g.resolve(resolve).expect("resolves");
    assert_eq!(text.syllables.get(&2).map(Vec::len), Some(2));
    // Fixed draws: option draw 0 (the weight-0 option is never chosen), then syllables 0, 0.
    let mut draws = [0u32, 0, 0].into_iter();
    let name = generate_name(
        &g,
        &text,
        0x10,
        0x1000,
        |_| draws.next().unwrap_or(0),
        |_| true,
    );
    assert_eq!(
        name,
        GeneratedName {
            name: "Kaliyero".into(),
            accepted: true,
            attempts: 1
        }
    );
    let mut draws = [2u32, 1, 1].into_iter();
    let name = generate_name_once(&g, &text, 0x10, 0x1000, |_| draws.next().unwrap_or(0));
    assert_eq!(name, "Toranndor");
    // The check sees a lower-cased copy (§8.5, creation screen).
    let mut seen = Vec::new();
    let _ = generate_name(
        &g,
        &text,
        0x10,
        0x1000,
        |_| 0,
        |s| {
            seen.push(s.to_owned());
            true
        },
    );
    assert_eq!(seen, vec!["kaliyero".to_owned()]);
    // Rejected every time: MAX_NAME_ATTEMPTS attempts in all, the last name is kept.
    let mut attempts = 0;
    let none = generate_name(
        &g,
        &text,
        0x10,
        0x1000,
        |_| 0,
        |_| {
            attempts += 1;
            false
        },
    );
    assert_eq!(attempts, MAX_NAME_ATTEMPTS);
    assert_eq!(MAX_NAME_ATTEMPTS, 100);
    assert_eq!(
        none,
        GeneratedName {
            name: "Kaliyero".into(),
            accepted: false,
            attempts: 100
        }
    );
    // No category: generation fails, the empty name never reaches the check.
    let failed = generate_name(&g, &text, 0x11, 0x1000, |_| 0, |_| panic!("not called"));
    assert_eq!((failed.name.as_str(), failed.accepted), ("", false));
    assert_eq!(generate_name_once(&g, &text, 0x11, 0x1000, |_| 0), "");
    // An undefined shift table means no shifts (§8.4 step 1).
    let mut no_shift = text.clone();
    no_shift.shifts = BTreeMap::new();
    let name = generate_name_once(&g, &no_shift, 0x10, 0x1000, |_| 0);
    assert_eq!(name, "Kaliaero");
}

/// §19.6: a category with weights 3, 0, 2; option 3 names T1 = [Dor, Bel], T2 = [dor, in]
/// and the shift tables [9 (undefined), the table of §19.1].
fn example_19_6() -> (NameGenerator, NameText) {
    let option = |weight: i32| NameOption {
        weight,
        syllable_tables: vec![1, 2],
        shift_tables: vec![9, 4],
    };
    let g = NameGenerator {
        race_categories: vec![RaceCategory {
            species: 1,
            sex: 2,
            category: 5,
        }],
        categories: vec![NameCategory {
            key: 5,
            options: vec![option(3), option(0), option(2)],
        }],
        ..NameGenerator::default()
    };
    let mut text = NameText::default();
    text.syllables.insert(1, vec!["Dor".into(), "Bel".into()]);
    text.syllables.insert(2, vec!["dor".into(), "in".into()]);
    text.shifts.insert(4, example_table());
    (g, text)
}

#[test]
fn generation_example_19_6() {
    let (g, text) = example_19_6();
    // Option draw r = 3 selects option 3 (r 0–2 option 1, the weight-0 option never).
    // Part 1: index 0 (Dor). Part 2: index 0 (dor, rejected), index 1 (in).
    let mut draws = [3u32, 0, 0, 1].into_iter();
    let mut calls = 0;
    let name = generate_name_once(&g, &text, 1, 2, |_| {
        calls += 1;
        draws.next().unwrap_or(0)
    });
    assert_eq!(name, "Dorin");
    assert_eq!(calls, 4);
    // Option probabilities: over every r of 0..5, option 1 three times, option 3 twice.
    let category = g.category(5).expect("category");
    let mut counts = [0usize; 3];
    for r in 0..5 {
        let o = draw_option_for_test(category, r);
        counts[o] += 1;
    }
    assert_eq!(counts, [3, 0, 2]);
    // With parts Kalia and ero the second table applies entry #2; with KAL and in nothing
    // applies and the case step gives Kalin.
    let mut t2 = text.clone();
    t2.syllables.insert(1, vec!["Kalia".into()]);
    t2.syllables.insert(2, vec!["ero".into()]);
    assert_eq!(generate_name_once(&g, &t2, 1, 2, |_| 0), "Kaliyero");
    t2.syllables.insert(1, vec!["KAL".into()]);
    t2.syllables.insert(2, vec!["in".into()]);
    assert_eq!(generate_name_once(&g, &t2, 1, 2, |_| 0), "Kalin");
    // A part table whose every string equals an earlier part: 101 rejected draws, then
    // the empty name.
    t2.syllables.insert(2, vec!["kal".into(), "Kal".into()]);
    let mut part_draws = 0u32;
    let name = generate_name_once(&g, &t2, 1, 2, |n| {
        if n == 2 {
            part_draws += 1;
        }
        0
    });
    assert_eq!(name, "");
    assert_eq!(part_draws, MAX_PART_DRAWS);
    // Weights summing to 0 fail generation (Decision D2, substitution 3).
    let mut zero = g.clone();
    for o in &mut zero.categories[0].options {
        o.weight = 0;
    }
    assert_eq!(generate_name_once(&zero, &text, 1, 2, |_| 0), "");
    // A negative weight is never drawn.
    zero.categories[0].options[2].weight = -4;
    zero.categories[0].options[1].weight = 1;
    let mut draws = [0u32, 0, 1].into_iter();
    assert_eq!(
        generate_name_once(&zero, &text, 1, 2, |_| draws.next().unwrap_or(0)),
        "Dorin"
    );
    // A key without a table fails generation.
    let mut missing = text.clone();
    missing.syllables.remove(&2);
    assert_eq!(generate_name_once(&g, &missing, 1, 2, |_| 0), "");
}

/// The option index the weighted draw picks for `r` (stored-order mapping).
fn draw_option_for_test(c: &NameCategory, r: u32) -> usize {
    let mut r = r;
    for (i, o) in c.options.iter().enumerate() {
        let w = u32::try_from(o.weight).unwrap_or(0);
        if r < w {
            return i;
        }
        r -= w;
    }
    usize::MAX
}

#[test]
fn generated_names_have_the_case_form_pc20() {
    let (g, text) = example_19_6();
    let mut state = 99u32;
    for _ in 0..200 {
        let name = generate_name_once(&g, &text, 1, 2, |n| {
            state = state.wrapping_mul(1_664_525).wrapping_add(1_013_904_223);
            (state >> 8) % n
        });
        let mut chars = name.chars();
        if let Some(first) = chars.next() {
            assert!(first.is_uppercase(), "{name}");
            assert!(chars.all(char::is_lowercase), "{name}");
        }
        // The two parts are never equal without regard to case: never "Dordor".
        assert_ne!(name, "Dordor");
    }
}

#[test]
fn text_model_d6() {
    // UTF-16 code units, simple one-to-one mappings.
    assert_eq!(lower_unit(u16::from(b'K')), u16::from(b'k'));
    assert_eq!(upper_unit(u16::from(b'k')), u16::from(b'K'));
    assert_eq!(
        lower_unit(0x0130),
        0x0069,
        "U+0130 has the simple mapping to i"
    );
    assert_eq!(
        upper_unit(0x00DF),
        0x00DF,
        "sharp s has no one-to-one upper case"
    );
    assert_eq!(upper_unit(0x1F80), 0x1F88);
    assert_eq!(upper_unit(0x1FB3), 0x1FBC);
    assert_eq!(lower_unit(0xD835), 0xD835, "a surrogate stays");
    assert_eq!(upper_unit(0xDC00), 0xDC00);
    assert_eq!(lower_unit(0x00C9), 0x00E9);
    assert!(text_eq_ci("ÉLM", "élm"));
    assert!(!text_eq_ci("elm", "elms"));
    assert_eq!(lower_text("KALIA"), "kalia");
    // Lengths count code units: a supplementary character is two.
    let t = ShiftTable::compile(&[entry("\u{1D504}", "x", "a", "")]);
    assert_eq!(t.left_lengths, vec![3]);
    assert!(t.is_well_formed());
    assert_eq!(t.join("b\u{1D504}", "xy"), "bay");
    // The case step: whole name lower, first code unit upper.
    let mut g = NameGenerator {
        race_categories: vec![RaceCategory {
            species: 1,
            sex: 1,
            category: 1,
        }],
        categories: vec![NameCategory {
            key: 1,
            options: vec![NameOption {
                weight: 1,
                syllable_tables: vec![1, 2],
                shift_tables: vec![],
            }],
        }],
        ..NameGenerator::default()
    };
    let mut text = NameText::default();
    text.syllables.insert(1, vec!["éLA".into()]);
    text.syllables.insert(2, vec!["NOR".into()]);
    assert_eq!(generate_name_once(&g, &text, 1, 1, |_| 0), "Élanor");
    // §8.5 step 4 OPEN (Q16): an entry whose trimmed E is empty matches every A, the empty
    // one included, so the first part can be shifted.
    g.categories[0].options[0].shift_tables = vec![3];
    text.shifts
        .insert(3, ShiftTable::compile(&[entry(" ", "é", "", "E")]));
    assert_eq!(generate_name_once(&g, &text, 1, 1, |_| 0), "Elanor");
    // Shift tables are tried in order until one applies: table 3 never matches, table 4
    // does.
    g.categories[0].options[0].shift_tables = vec![3, 4];
    text.shifts
        .insert(3, ShiftTable::compile(&[entry("q", "q", "", "")]));
    text.shifts
        .insert(4, ShiftTable::compile(&[entry("a", "n", "o", "")]));
    assert_eq!(generate_name_once(&g, &text, 1, 1, |_| 0), "Éloor");
}

#[test]
fn name_rule_control_view() {
    let reg = registry();
    let rule = |k: u32| {
        p(
            &reg,
            "NameRule_NameRuleEntry",
            st(vec![p(&reg, "NameRule_NameRule", Value::Enum(k))]),
        )
    };
    let c = NameRuleControl::from_properties(
        &set(vec![p(
            &reg,
            "NameRule_NameRuleArray",
            Value::Array(vec![rule(1), rule(2)]),
        )]),
        &reg,
    )
    .expect("rules");
    assert_eq!(c.rules, vec![1, 2]);
}

#[test]
fn records_are_found_from_their_labels() {
    let reg = registry();
    let dict = character_dictionary();
    const DIR: u32 = 0x7000_0010;
    const RACE: u32 = 0x7000_0011;
    const PIPS: u32 = 0x7000_0020;
    const PIP: u32 = 0x7000_0021;
    let did_array = |elem: &str, dids: &[u32]| {
        let mut v = (dids.len() as u32).to_le_bytes().to_vec();
        for d in dids {
            v.extend_from_slice(&pid(&reg, elem).to_le_bytes());
            v.extend_from_slice(&d.to_le_bytes());
        }
        v
    };
    let set_rec = |did: u32, entries: &[(u32, Vec<u8>)]| {
        (
            did + 0x0900_0000,
            property_set_record(did + 0x0900_0000, entries),
        )
    };
    let mut src = MemorySource::new()
        .with(0x3400_0000, dict)
        .with(
            0x2800_0000,
            did_map(0x2800_0000, &[(0xA, 0x2800_000E, "WEENIECONTENT")]),
        )
        .with(
            0x2800_000E,
            did_map(
                0x2800_000E,
                &[(1, DIR, "RaceTableDirectory"), (2, PIPS, "PipControl")],
            ),
        );
    for (did, bytes) in [
        set_rec(
            DIR,
            &[(
                pid(&reg, "RaceTable_RaceTableList"),
                did_array("RaceTable_RaceTable", &[RACE]),
            )],
        ),
        set_rec(
            RACE,
            &[(pid(&reg, "RaceTable_Race"), 0x10u32.to_le_bytes().to_vec())],
        ),
        set_rec(
            PIPS,
            &[(
                pid(&reg, "PipControl_Directory"),
                did_array("PipControl_Entry", &[PIP]),
            )],
        ),
    ] {
        src.insert(did, bytes);
    }
    let store = ObjectStore::new(src);
    let found = CharacterRecords::collect(&store).expect("collects");
    assert_eq!(found.control("RaceTableDirectory"), Some(Did(DIR)));
    assert_eq!(found.control("PipControl"), Some(Did(PIPS)));
    assert_eq!(found.control("GuildControl"), None);
    assert_eq!(
        found.with_role(Role::RaceTable).collect::<Vec<_>>(),
        vec![Did(RACE)]
    );
    assert_eq!(
        found.with_role(Role::Pip).collect::<Vec<_>>(),
        vec![Did(PIP)]
    );
    assert_eq!(found.with_role(Role::Control).count(), 2);
}
