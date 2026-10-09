// SPDX-License-Identifier: GPL-3.0-or-later
//! Synthetic tests of the game-control views (`game-controls.md` §11 worked examples and
//! the rules of §2.1, §3, §4.10, §5.3.2, §5.7, §8.1). Every value here is made up.

use std::collections::BTreeSet;

use tessera_formats::{BitfieldAny, Did};
use tessera_props::{MemorySource, Property, PropertyRegistry, PropertySet, Value};

use super::*;
use crate::library::{ClassLibrary, ValueType};
use crate::native::Ref;
use crate::state::{DecodeContext, ObjectState, Value as SValue};
use crate::testkit::{
    LibraryBuilder, RecordBuilder, did_map, game_controls_dictionary, property_set_record,
    put_tsize,
};
use crate::{ObjectStore, WStateError};

fn registry() -> PropertyRegistry {
    PropertyRegistry::parse(&game_controls_dictionary()).expect("synthetic dictionary")
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

fn structs(reg: &PropertyRegistry, items: Vec<Vec<Property>>) -> Value {
    Value::Array(
        items
            .into_iter()
            .map(|s| p(reg, "Test_GcStruct", st(s)))
            .collect(),
    )
}

fn le(words: &[u32]) -> Vec<u8> {
    words.iter().flat_map(|w| w.to_le_bytes()).collect()
}

// --- §2.2 labels -------------------------------------------------------------------

#[test]
fn scope_labels_are_the_119_of_the_spec() {
    let unique: BTreeSet<&str> = SCOPE_LABELS.iter().map(|(l, _)| *l).collect();
    assert_eq!(unique.len(), 119);
    assert_eq!(
        SCOPE_LABELS
            .iter()
            .filter(|(l, _)| l.starts_with("Dialog_"))
            .count(),
        38
    );
    for l in WEAPON_STYLE_LABELS
        .iter()
        .chain(&MANAGER_LABELS)
        .chain(&DID_SET_LABELS)
    {
        assert!(unique.contains(l), "{l}");
    }
    assert_eq!(scope_group("GuildControl"), Some(Group::Social));
    assert_eq!(scope_group("RaceTableDirectory"), None);
}

// --- §2.1 kinds ---------------------------------------------------------------------

/// A library with the class roles of §2.1 at small indices: 1 root, 2 its child, 3 the
/// grandchild, 4 plain-object base, 5 entity base, 6 DID set, 7 dialog base, 8 skill,
/// 9 effect base, 10 manager base; leaves 11 (dialog), 12 (plain), 13 (effect),
/// 14 (manager), 15 (entity), 16 (plain), 17 (entity whose parent is not 10).
fn kind_library() -> (ClassLibrary, ClassIndices) {
    let mut b = LibraryBuilder::new(20);
    for c in 1..=17 {
        b = b.define(c, &[]);
    }
    let b = b
        .parent(2, 1)
        .parent(3, 2)
        .parent(4, 3)
        .parent(5, 3)
        .parent(6, 4)
        .parent(7, 4)
        .parent(8, 4)
        .parent(9, 4)
        .parent(10, 5)
        .parent(11, 7)
        .parent(12, 4)
        .parent(13, 9)
        .parent(14, 10)
        .parent(15, 5)
        .parent(16, 12)
        .parent(17, 15);
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
}

#[test]
fn kind_rule_rows_in_order() {
    let (lib, idx) = kind_library();
    let obj = Did(0x7000_0001);
    let k = |did: Did, c: Option<u32>, empty: bool| {
        classify(did, c, &lib, &idx, || Ok(empty)).expect("classify")
    };
    assert_eq!(k(Did(0), None, false), Some(EntryKind::Null));
    assert_eq!(
        k(Did(0x0700_0010), None, false),
        Some(EntryKind::ScriptTable)
    );
    // The DID-set class wins over an empty-marker answer (row 3 before row 5).
    assert_eq!(k(obj, Some(6), true), Some(EntryKind::DidSet));
    assert_eq!(k(obj, Some(11), true), Some(EntryKind::DialogMarker));
    assert_eq!(k(obj, Some(12), true), Some(EntryKind::EmptyMarker));
    assert_eq!(k(obj, Some(12), false), Some(EntryKind::PropertyControl));
    assert_eq!(k(obj, Some(8), false), Some(EntryKind::Skill));
    assert_eq!(k(obj, Some(9), false), Some(EntryKind::Effect));
    assert_eq!(k(obj, Some(13), false), Some(EntryKind::Effect));
    assert_eq!(k(obj, Some(14), false), Some(EntryKind::Manager));
    // Same ancestry as a manager but another parent: an "other entity" (§3.4).
    assert_eq!(k(obj, Some(17), false), Some(EntryKind::Entity));
    assert_eq!(k(obj, Some(15), false), Some(EntryKind::Entity));
    assert_eq!(k(obj, Some(16), false), Some(EntryKind::PropertyControl));
    // Bases themselves and the common ancestors match no row.
    assert_eq!(k(obj, Some(4), false), None);
    assert_eq!(k(obj, Some(3), false), None);
    assert_eq!(k(obj, None, false), None);
    // A class outside the library.
    assert_eq!(k(obj, Some(999), false), None);
    // The empty-marker test runs only when rows 1-4 fail.
    let called = std::cell::Cell::new(false);
    let _ = classify(obj, Some(11), &lib, &idx, || {
        called.set(true);
        Ok(true)
    });
    assert!(!called.get());
    assert_eq!(
        EntryKind::ALL.map(EntryKind::row),
        [1, 2, 3, 4, 5, 6, 7, 8, 9, 10]
    );
}

#[test]
fn class_index_cross_checks() {
    let (lib, idx) = kind_library();
    assert_eq!(idx.check(&lib), vec![]);
    let bad = ClassIndices {
        common_ancestors: [2, 1, 3],
        skill: 15,
        ..idx
    };
    let problems = bad.check(&lib);
    assert!(problems.contains(&IndexProblem::NotRoot(2)));
    assert!(problems.contains(&IndexProblem::NotDescendant {
        class: 15,
        ancestor: 4
    }));
    let unrelated = ClassIndices {
        entity_base: 12,
        ..idx
    };
    assert!(unrelated.check(&lib).contains(&IndexProblem::BasesRelated));
    assert_eq!(ClassIndices::default(), ClassIndices::OBSERVED);
}

fn empty_marker_record(did: u32, class: u16) -> Vec<u8> {
    RecordBuilder::new(did, u32::from(class))
        .class(class, &[])
        .script(0, class, &[])
        .build()
}

fn did_set_record(did: u32, class: u16, members: &[u32]) -> Vec<u8> {
    let mut body = u16::try_from(members.len())
        .expect("small")
        .to_le_bytes()
        .to_vec();
    body.extend([0, 0]);
    body.extend(le(members));
    RecordBuilder::new(did, u32::from(class))
        .class(class, &[(DID_SET_ATTRIBUTE, ValueType::Reference)])
        .script(0, class, &[SValue::Ref(Ref(1))])
        .object(1, 18, &body)
        .build()
}

#[test]
fn empty_marker_form() {
    let bytes = empty_marker_record(0x7000_0100, 12);
    assert_eq!(bytes.len(), EMPTY_MARKER_STATE_LEN);
    let state = ObjectState::parse(&bytes, Did(0x7000_0100), DecodeContext::bare()).expect("state");
    let set_bytes = property_set_record(0x7900_0100, &[]);
    assert_eq!(set_bytes.len(), EMPTY_PROPERTY_SET_LEN);
    let reg = registry();
    let empty = PropertySet::parse_record(&set_bytes, Did(0x7900_0100), &reg).expect("set");
    assert!(is_empty_marker(
        bytes.len(),
        &state,
        set_bytes.len(),
        &empty
    ));
    let one = set(vec![p(&reg, "Test_GcInt", Value::Int(1))]);
    assert!(!is_empty_marker(bytes.len(), &state, 12, &one));
    assert!(!is_empty_marker(37, &state, set_bytes.len(), &empty));
    // A root with an attribute is not empty.
    let with_attr = RecordBuilder::new(0x7000_0101, 12)
        .class(12, &[(1, ValueType::Int)])
        .script(0, 12, &[SValue::Int(0)])
        .build();
    let s = ObjectState::parse(&with_attr, Did(0x7000_0101), DecodeContext::bare()).expect("s");
    assert!(!is_empty_marker(36, &s, 6, &empty));
}

/// §11.5: a DID set of three members stored out of order.
#[test]
fn did_set_worked_example() {
    let bytes = did_set_record(0x7000_0200, 6, &[0x7000_2000, 0x7000_1000, 0x7000_1001]);
    let state = ObjectState::parse(&bytes, Did(0x7000_0200), DecodeContext::bare()).expect("state");
    let s = DidSet::from_state(&state).expect("DID set");
    assert_eq!(s.attribute, DID_SET_ATTRIBUTE);
    assert_eq!(s.item_count, 2);
    assert_eq!(s.len(), 3);
    assert_eq!(s.duplicates, 0);
    assert_eq!(
        s.members,
        vec![Did(0x7000_1000), Did(0x7000_1001), Did(0x7000_2000)]
    );
    assert!(s.contains(Did(0x7000_1001)));
    assert!(!s.contains(Did(0x7000_1002)));
    let dup = did_set_record(0x7000_0201, 6, &[5, 5, 6]);
    let st = ObjectState::parse(&dup, Did(0x7000_0201), DecodeContext::bare()).expect("state");
    let d = DidSet::from_state(&st).expect("set");
    assert_eq!((d.len(), d.duplicates), (2, 1));
    // An empty marker is not a DID set; neither is a set behind a non-reference.
    let marker = empty_marker_record(0x7000_0202, 12);
    let st = ObjectState::parse(&marker, Did(0x7000_0202), DecodeContext::bare()).expect("s");
    assert!(matches!(
        DidSet::from_state(&st),
        Err(WStateError::WrongKind { .. })
    ));
    let int_attr = RecordBuilder::new(0x7000_0203, 6)
        .class(6, &[(DID_SET_ATTRIBUTE, ValueType::Int)])
        .script(0, 6, &[SValue::Int(1)])
        .build();
    let st = ObjectState::parse(&int_attr, Did(0x7000_0203), DecodeContext::bare()).expect("s");
    assert!(matches!(
        DidSet::from_state(&st),
        Err(WStateError::BadAttribute { .. })
    ));
}

fn single_ref_record(did: u32, hash: u32, native_class: u16, body: &[u8]) -> ObjectState {
    let bytes = RecordBuilder::new(did, 40)
        .class(40, &[(hash, ValueType::Reference)])
        .script(0, 40, &[SValue::Ref(Ref(1))])
        .object(1, native_class, body)
        .build();
    ObjectState::parse(&bytes, Did(did), DecodeContext::bare()).expect("state")
}

#[test]
fn compiled_tables() {
    let mut body = le(&[3]);
    body.extend(le(&[0, 0, 100, 0, 250, 0]));
    let t = AdvancementTable::from_state(&single_ref_record(
        0x7000_0300,
        ADVANCEMENT_ATTRIBUTE,
        104,
        &body,
    ))
    .expect("table");
    assert_eq!(t.values, vec![0, 100, 250]);
    assert!(t.is_cumulative());
    let falling = AdvancementTable {
        attribute: 0,
        values: vec![0, 5, 4],
    };
    assert!(!falling.is_cumulative());
    assert!(!AdvancementTable::default().is_cumulative());

    let mut body = Vec::new();
    put_tsize(&mut body, 0, 2);
    body.extend(le(&[2, 1.5f32.to_bits(), 1, 1.0f32.to_bits()]));
    let m = ModifierTable::from_state(&single_ref_record(
        0x7000_0301,
        MODIFIER_ATTRIBUTE,
        11,
        &body,
    ))
    .expect("map");
    assert_eq!(m.entries, vec![(1, 1.0), (2, 1.5)]);
    assert_eq!(m.get(2), Some(1.5));
    assert_eq!(m.get(3), None);
    assert!(m.all_finite());
    let nan = ModifierTable {
        attribute: 0,
        entries: vec![(1, f32::NAN)],
    };
    assert!(!nan.all_finite());
    // The wrong native class behind the reference.
    assert!(matches!(
        ModifierTable::from_state(&single_ref_record(0x7000_0302, 1, 104, &le(&[0]))),
        Err(WStateError::BadAttribute { .. })
    ));
}

#[test]
fn slot_schedule_three_lists() {
    let (a, b, c) = (0x11u32, 0x22u32, 0x33u32);
    let bytes = RecordBuilder::new(0x7000_0400, 41)
        .class(
            41,
            &[
                (a, ValueType::Reference),
                (b, ValueType::Reference),
                (c, ValueType::Reference),
            ],
        )
        .script(
            0,
            41,
            &[
                SValue::Ref(Ref(1)),
                SValue::Ref(Ref(2)),
                SValue::Ref(Ref(3)),
            ],
        )
        .object(1, 17, &le(&[0]))
        .object(2, 17, &le(&[2, 1, 1]))
        .object(3, 17, &le(&[1, 1]))
        .build();
    let s = ObjectState::parse(&bytes, Did(0x7000_0400), DecodeContext::bare()).expect("state");
    let sched = SlotSchedule::from_state(&s).expect("schedule");
    assert_eq!(sched.lengths(), vec![0, 2, 1]);
    assert_eq!(sched.lists.first().map(|l| l.0), Some(a));
    // A null reference is an error, not a panic.
    let null = RecordBuilder::new(0x7000_0401, 41)
        .class(
            41,
            &[
                (a, ValueType::Reference),
                (b, ValueType::Reference),
                (c, ValueType::Reference),
            ],
        )
        .script(
            0,
            41,
            &[
                SValue::Ref(Ref::NULL),
                SValue::Ref(Ref::NULL),
                SValue::Ref(Ref::NULL),
            ],
        )
        .build();
    let s = ObjectState::parse(&null, Did(0x7000_0401), DecodeContext::bare()).expect("state");
    assert!(matches!(
        SlotSchedule::from_state(&s),
        Err(WStateError::BadAttribute { .. })
    ));
}

#[test]
fn manager_template_diff() {
    let reg = registry();
    let base = vec![
        p(&reg, "Test_GcInt", Value::Int(2)),
        p(&reg, "Test_GcDid", Value::DidRef(Did(0x4700_0001))),
    ];
    let a = set(base.clone());
    let mut b_props = base;
    b_props[1] = p(&reg, "Test_GcDid", Value::DidRef(Did(0x4700_0002)));
    b_props.push(p(&reg, "Test_GcFloat", Value::Float(1.0)));
    let b = set(b_props);
    let d = TemplateDiff::between(&a, &b);
    assert_eq!(d.different, vec![pid(&reg, "Test_GcDid")]);
    assert_eq!(d.only_second, vec![pid(&reg, "Test_GcFloat")]);
    assert!(d.only_first.is_empty());
    assert!(TemplateDiff::between(&a, &a).is_empty());

    // §3.4: the name is `Name`, looked up by label.
    let named = set(vec![p(
        &reg,
        ENTITY_NAME_LABEL,
        Value::StringInfo(Box::new(tessera_formats::StringInfo {
            source: tessera_formats::StringSource::Literal(tessera_formats::Utf16String::from("x")),
            args: None,
        })),
    )]);
    assert!(entity_name(&named, &reg).expect("name").is_some());
    assert_eq!(entity_name(&a, &reg).expect("no name"), None);
    let bad = set(vec![p(&reg, "Name", Value::Int(1))]);
    assert!(entity_name(&bad, &reg).is_err());
}

#[test]
fn weenie_index_classifies_a_synthetic_map() {
    let (lib, idx) = kind_library();
    let _ = lib;
    let mut b = LibraryBuilder::new(45);
    for c in 1..=17 {
        b = b.define(c, &[]);
    }
    b = b.define(40, &[(DID_SET_ATTRIBUTE, ValueType::Reference)]);
    let lib_bytes = b
        .parent(2, 1)
        .parent(3, 2)
        .parent(4, 3)
        .parent(5, 3)
        .parent(6, 4)
        .parent(7, 4)
        .parent(8, 4)
        .parent(9, 4)
        .parent(10, 5)
        .parent(11, 7)
        .parent(12, 4)
        .parent(13, 9)
        .parent(14, 10)
        .parent(15, 5)
        .parent(16, 12)
        .parent(17, 15)
        .build();
    let plain = |did: u32, class: u16| {
        RecordBuilder::new(did, u32::from(class))
            .class(class, &[(1, ValueType::Int)])
            .script(0, class, &[SValue::Int(7)])
            .build()
    };
    let reg = registry();
    let mut src = MemorySource::new()
        .with(0x3400_0000, game_controls_dictionary())
        .with(0x5600_0000, lib_bytes)
        .with(
            0x2800_0000,
            did_map(0x2800_0000, &[(0xA, 0x2800_000E, "WEENIECONTENT")]),
        )
        .with(
            0x2800_000E,
            did_map(
                0x2800_000E,
                &[
                    (1, 0, "Undef"),
                    (2, 0x0700_0001, "SomeScriptTable"),
                    (3, 0x7000_0003, "SomeDidSet"),
                    (4, 0x7000_0004, "Dialog_Example"),
                    (5, 0x7000_0005, "SomeMarker"),
                    (6, 0x7000_0006, "SomeSkill"),
                    (7, 0x7000_0007, "SomeManager"),
                    (8, 0x7000_0008, "SomeControl"),
                    (9, 0x7000_0009, "Dangling"),
                ],
            ),
        );
    src.insert(0x7000_0003, did_set_record(0x7000_0003, 6, &[0x7000_0008]));
    src.insert(0x7000_0004, empty_marker_record(0x7000_0004, 11));
    src.insert(0x7000_0005, empty_marker_record(0x7000_0005, 12));
    src.insert(0x7000_0006, plain(0x7000_0006, 8));
    src.insert(0x7000_0007, plain(0x7000_0007, 14));
    src.insert(0x7000_0008, plain(0x7000_0008, 16));
    for d in [3u32, 4, 5, 6, 7] {
        src.insert(0x7900_0000 + d, property_set_record(0x7900_0000 + d, &[]));
    }
    src.insert(
        0x7900_0008,
        property_set_record(
            0x7900_0008,
            &[(pid(&reg, "Test_GcInt"), 5i32.to_le_bytes().to_vec())],
        ),
    );
    let store = ObjectStore::new(src);
    let index = WeenieIndex::build(&store, &idx).expect("index");
    let kind = |l: &str| index.get(l).and_then(|e| e.kind);
    assert_eq!(kind("Undef"), Some(EntryKind::Null));
    assert_eq!(kind("SomeScriptTable"), Some(EntryKind::ScriptTable));
    assert_eq!(kind("SomeDidSet"), Some(EntryKind::DidSet));
    assert_eq!(kind("Dialog_Example"), Some(EntryKind::DialogMarker));
    assert_eq!(kind("SomeMarker"), Some(EntryKind::EmptyMarker));
    assert_eq!(kind("SomeSkill"), Some(EntryKind::Skill));
    assert_eq!(kind("SomeManager"), Some(EntryKind::Manager));
    assert_eq!(kind("SomeControl"), Some(EntryKind::PropertyControl));
    assert_eq!(kind("Dangling"), None);
    assert_eq!(index.counts().get(&None), Some(&1));
    assert_eq!(index.of_kind(EntryKind::Skill).count(), 1);
    assert_eq!(index.entries.len(), 9);
}

// --- §5 -----------------------------------------------------------------------------

/// §11.2: speed Walk 0.4, Trot 0.6, Canter 0.9, no Gallop; turn rate Walk 1.2,
/// Canter 0.5; the Canter add-mod names property P; no add-mod for Walk.
#[test]
fn gait_multipliers_worked_example() {
    let reg = registry();
    let actor_p = pid(&reg, "Test_GcFloat");
    let control = set(vec![
        p(
            &reg,
            "Mount_Control_Speed_Multiplier_Walk",
            Value::Float(0.4),
        ),
        p(
            &reg,
            "Mount_Control_Speed_Multiplier_Trot",
            Value::Float(0.6),
        ),
        p(
            &reg,
            "Mount_Control_Speed_Multiplier_Canter",
            Value::Float(0.9),
        ),
        p(
            &reg,
            "Mount_Control_Turn_Rate_Multiplier_Walk",
            Value::Float(1.2),
        ),
        p(
            &reg,
            "Mount_Control_Turn_Rate_Multiplier_Canter",
            Value::Float(0.5),
        ),
        p(
            &reg,
            "Mount_Control_Turn_Rate_AddMod_Property_Cantor",
            Value::PropertyId(actor_p),
        ),
    ]);
    let c = MountedCombatControl::from_properties(&control, &reg).expect("control");
    assert_eq!(c.speed_multiplier(GAIT_TROT), 0.6);
    assert_eq!(c.speed_multiplier(GAIT_GALLOP), 1.0);
    assert_eq!(c.speed_multiplier(GAIT_NEUTRAL), 1.0);
    let quarter = Value::Float(0.25);
    let with_p = |q: u32| (q == actor_p).then_some(&quarter);
    assert_eq!(c.turn_rate(GAIT_CANTER, with_p), 0.75);
    assert_eq!(c.turn_rate(GAIT_CANTER, |_| None), 0.5);
    assert_eq!(c.turn_rate(GAIT_NEUTRAL, with_p), 1.2);
    assert_eq!(c.turn_rate(GAIT_WALK, with_p), 1.2);
    assert_eq!(c.turn_rate(GAIT_TROT, with_p), 1.0);
    assert!((c.forward_speed_limit(Some(GAIT_CANTER), 10.0, None) - 9.0).abs() < 1e-5);
    // No locked gear: the Gallop multiplier (absent, 1.0), capped by a second limit.
    assert_eq!(c.forward_speed_limit(None, 10.0, None), 10.0);
    assert_eq!(c.forward_speed_limit(None, 10.0, Some(8.0)), 8.0);
    assert_eq!(MountedCombatControl::turn_gait(None), GAIT_NEUTRAL);
    assert_eq!(
        MountedCombatControl::turn_gait(Some(&Value::Enum(GAIT_TROT))),
        GAIT_TROT
    );
    // A non-float actor value adds 0; a non-float multiplier counts as absent.
    let int_value = Value::Int(3);
    assert_eq!(c.turn_rate(GAIT_CANTER, |_| Some(&int_value)), 0.5);
    let odd = set(vec![p(
        &reg,
        "Mount_Control_Turn_Rate_AddMod_Property_Walk",
        Value::Int(1),
    )]);
    let o = MountedCombatControl::from_properties(&odd, &reg).expect("lenient");
    assert_eq!(o.turn_rate(GAIT_WALK, |_| Some(&quarter)), 1.0);
}

#[test]
fn mounted_combat_control_tables() {
    let reg = registry();
    let control = set(vec![
        p(
            &reg,
            "Mount_Control_Appearance_Group_To_Property_Hash",
            structs(
                &reg,
                vec![vec![
                    p(&reg, "Trait_Group", Value::Enum(8)),
                    p(
                        &reg,
                        "Mount_Control_Apperance_Trait_Material_PropertyName",
                        Value::PropertyId(77),
                    ),
                ]],
            ),
        ),
        p(
            &reg,
            "Mount_Control_FlankConstraints_Array",
            structs(
                &reg,
                vec![vec![
                    p(&reg, "Mount_Control_FlankPosition", Value::Bitfield32(1)),
                    p(&reg, "Mount_Control_Flank_Heading", Value::Float(90.0)),
                    p(&reg, "Mount_Control_Flank_LeftArc", Value::Float(-45.0)),
                    p(&reg, "Mount_Control_Flank_RightArc", Value::Float(45.0)),
                ]],
            ),
        ),
        p(
            &reg,
            "Mount_Control_Tree_To_Skill_Hash",
            structs(
                &reg,
                vec![vec![
                    p(&reg, "Mount_Control_Trait_Tree", Value::Enum(1)),
                    p(
                        &reg,
                        "Mount_Control_Mount_Skill_Hash",
                        structs(
                            &reg,
                            vec![vec![
                                p(&reg, "Mount_Control_Player_Race", Value::Enum(23)),
                                p(
                                    &reg,
                                    "Mount_Control_Mount_Skill",
                                    Value::DidRef(Did(0x7000_0001)),
                                ),
                            ]],
                        ),
                    ),
                ]],
            ),
        ),
    ]);
    let c = MountedCombatControl::from_properties(&control, &reg).expect("control");
    assert_eq!(c.material_properties, vec![(Some(8), Some(77))]);
    assert_eq!(
        c.flanks,
        vec![FlankConstraint {
            position: Some(1),
            heading: Some(90.0),
            left_arc: Some(-45.0),
            right_arc: Some(45.0),
        }]
    );
    assert_eq!(
        c.tree_skills.first().map(|t| t.skills.clone()),
        Some(vec![(Some(23), Some(Did(0x7000_0001)))])
    );
    // The tree keys: the tree array is absent here, so the two key sets differ.
    assert!(!c.tree_keys_consistent());
    let mut both = c.clone();
    both.trees = vec![MountTree {
        tree: Some(1),
        ..MountTree::default()
    }];
    assert!(both.tree_keys_consistent());
    both.trees.push(MountTree {
        tree: Some(1),
        ..MountTree::default()
    });
    assert!(!both.tree_keys_consistent(), "a key twice in one array");
    // GCI18: the flank angles are floats; an int heading is an error of the view (§3.1).
    let int_heading = set(vec![p(
        &reg,
        "Mount_Control_FlankConstraints_Array",
        structs(
            &reg,
            vec![vec![p(&reg, "Mount_Control_Flank_Heading", Value::Int(90))]],
        ),
    )]);
    assert!(matches!(
        MountedCombatControl::from_properties(&int_heading, &reg),
        Err(WStateError::BadProperty {
            property: "Mount_Control_Flank_Heading",
            ..
        })
    ));
    // A struct list that is not an array is a typed error.
    let bad = set(vec![p(&reg, "Mount_Control_Tree_Array", Value::Int(1))]);
    assert!(matches!(
        MountedCombatControl::from_properties(&bad, &reg),
        Err(WStateError::BadProperty { .. })
    ));
}

#[test]
fn mount_directory_and_traits() {
    let reg = registry();
    let dir = set(vec![p(
        &reg,
        "Mount_SkillList",
        Value::Array(vec![
            p(&reg, "Mount_SkillToGrantTall", Value::DidRef(Did(1))),
            p(&reg, "Mount_SkillToGrantShort", Value::DidRef(Did(2))),
            p(&reg, "Mount_SkillToGrantTall", Value::DidRef(Did(3))),
        ]),
    )]);
    let d = MountDirectory::from_properties(&dir, &reg).expect("directory");
    assert_eq!((d.tall.len(), d.short.len(), d.other), (2, 1, 0));
    assert!(d.is_disjoint());
    let overlap = MountDirectory {
        tall: vec![Did(1)],
        short: vec![Did(1)],
        other: 0,
    };
    assert!(!overlap.is_disjoint());
    // §5.1: the race table's tall flag picks the group (reading, Q6).
    assert_eq!(d.skills_for_size(true), &[Did(1), Did(3)]);
    assert_eq!(d.skills_for_size(false), &[Did(2)]);

    let appearance = |k: u32| {
        p(
            &reg,
            "Trait_Appearance_Set",
            Value::Array(vec![p(
                &reg,
                "Trait_Appearance_Struct",
                st(vec![
                    p(&reg, "Trait_Appearance_APR_Key", Value::Enum(k)),
                    p(
                        &reg,
                        "Trait_Appearance_APR",
                        Value::DidRef(Did(0x2000_0001)),
                    ),
                    p(&reg, "Trait_Appearance_APR_Value", Value::Float(1.0)),
                ]),
            )]),
        )
    };
    let t = set(vec![
        p(
            &reg,
            "Trait_Groups",
            Value::Array(vec![p(&reg, "Trait_Group", Value::Enum(9))]),
        ),
        p(&reg, "Trait_Appearance_IsDyable", Value::Bool(true)),
        p(
            &reg,
            "Trait_Appearance_List",
            Value::Array(vec![appearance(1), appearance(2)]),
        ),
    ]);
    let tr = MountTrait::from_properties(&t, &reg).expect("trait");
    assert_eq!(tr.top_level_group, None);
    assert_eq!(tr.slot(), Some(9));
    assert_eq!(tr.appearance_sets.len(), 2);
    assert_eq!(
        tr.appearance_sets
            .first()
            .and_then(|s| s.first())
            .map(|a| a.apr_key),
        Some(Some(1))
    );
    assert_eq!(MountTrait::default().slot(), None);
}

#[test]
fn default_attack_and_duel_timings() {
    let reg = registry();
    let skill = set(vec![p(&reg, "Skill_IsDefaultAttack", Value::Bool(true))]);
    assert!(is_default_attack(&skill, &reg));
    assert!(!is_default_attack(&set(vec![]), &reg));
    let d = DuelTimings::from_properties(&set(vec![]), &reg);
    assert_eq!(
        (d.begin_delay_or_default(), d.length_or_default()),
        (10.0, 600.0)
    );
    let wrong = set(vec![
        p(&reg, "Combat_Control_DuelingBeginDelay", Value::Float(5.0)),
        p(&reg, "Combat_Control_LengthOfTheDuel", Value::Int(30)),
    ]);
    let d = DuelTimings::from_properties(&wrong, &reg);
    assert_eq!(
        (d.begin_delay_or_default(), d.length_or_default()),
        (5.0, 600.0)
    );
}

// --- §6 -----------------------------------------------------------------------------

#[test]
fn perks_collections_services_rewards() {
    let reg = registry();
    let channel = |c: u32, ui: u32| {
        vec![
            p(&reg, "PerkControl_Channel", Value::Enum(c)),
            p(&reg, "PerkControl_UICategory", Value::Enum(ui)),
            p(&reg, "PerkControl_Channel_Interval", Value::Int(86_400)),
            p(&reg, "PerkControl_Channel_Cap", Value::Int(5)),
        ]
    };
    let control = set(vec![p(
        &reg,
        "PerkControl_ChannelData",
        structs(&reg, vec![channel(1, 10), channel(2, 20)]),
    )]);
    let c = PerkControl::from_properties(&control, &reg).expect("perk control");
    assert!(c.keys_are_distinct());
    let perk = Perk::from_properties(
        &set(vec![
            p(&reg, "Perk_Channel", Value::Enum(2)),
            p(&reg, "Perk_UICategory", Value::Enum(20)),
        ]),
        &reg,
    )
    .expect("perk");
    assert!(c.admits(&perk));
    assert!(!c.admits(&Perk {
        channel: Some(3),
        ..perk.clone()
    }));
    let dup = PerkControl {
        channels: vec![PerkChannel::default(), PerkChannel::default()],
        available: vec![],
    };
    assert!(!dup.keys_are_distinct());

    let tier = vec![
        p(
            &reg,
            "RandomItemReward_Tier_NPC",
            Value::DidRef(Did(0x7000_0042)),
        ),
        p(
            &reg,
            "RandomItemReward_Tier_VXP",
            Value::DidRef(Did(0x7000_0043)),
        ),
    ];
    let rewards = set(vec![
        p(
            &reg,
            "RandomItemReward_Gold_Category",
            structs(&reg, vec![tier.clone(), tier]),
        ),
        p(
            &reg,
            "RandomItemRewardControl_ChanceToShowRarest",
            Value::Float(0.1),
        ),
    ]);
    let r = RandomItemRewardControl::from_properties(&rewards, &reg).expect("rewards");
    let gold = pid(&reg, "RandomItemReward_Gold_Category");
    let login = LoginReward::from_properties(
        &set(vec![p(
            &reg,
            "LoginReward_ControlCategory_PropertyName",
            Value::PropertyId(gold),
        )]),
        &reg,
    )
    .expect("login reward");
    let tiers = login
        .control_category
        .and_then(|c| r.tiers(c))
        .expect("category");
    assert_eq!(tiers.len(), 2);
    assert_eq!(tiers.first().and_then(|t| t.npc), Some(Did(0x7000_0042)));
    assert!(
        r.tiers(pid(&reg, "RandomItemReward_Silver_Category"))
            .is_none()
    );

    let service = PaperItemService::from_properties(
        &set(vec![p(&reg, "PaperItemServiceEntry_Type", Value::Enum(7))]),
        &reg,
    )
    .expect("service");
    assert_eq!(service.service_type, Some(7));
    let coll = Collection::from_properties(
        &set(vec![p(
            &reg,
            "Usage_RequiredRaces",
            Value::Array(vec![p(&reg, "Test_GcEnum", Value::Enum(23))]),
        )]),
        &reg,
    )
    .expect("collection");
    assert_eq!(coll.required_races, vec![23]);
    // A wrong type is a typed error.
    assert!(matches!(
        Perk::from_properties(&set(vec![p(&reg, "Perk_Channel", Value::Int(1))]), &reg),
        Err(WStateError::BadProperty { .. })
    ));
}

// --- §7, §8 -------------------------------------------------------------------------

/// §11.3: 0x0A is keys 2 and 4; 0x7F is keys 1–7.
#[test]
fn reset_day_worked_example() {
    assert_eq!(bit_keys(0x0A), vec![2, 4]);
    assert_eq!(bit_keys(0x7F), vec![1, 2, 3, 4, 5, 6, 7]);
    assert_eq!(bit_keys(0), Vec::<u32>::new());
    let reg = registry();
    let c = ResetScheduleControl::from_properties(
        &set(vec![p(
            &reg,
            "ResetScheduleControl_ResetScheduleArray",
            structs(
                &reg,
                vec![vec![
                    p(&reg, "ResetScheduleControl_ResetSchedule", Value::Enum(2)),
                    p(
                        &reg,
                        "ResetScheduleControl_ResetDays",
                        Value::Bitfield32(0x0A),
                    ),
                ]],
            ),
        )]),
        &reg,
    )
    .expect("control");
    assert_eq!(c.get(2).map(ResetSchedule::day_keys), Some(vec![2, 4]));
    assert!(c.get(1).is_none());
}

/// §11.4: only bit 9 set gives key 10; the override with key 10 is for monster-play
/// characters only.
#[test]
fn vital_display_unlock_worked_example() {
    let bits = |bytes: Vec<u8>| {
        Value::BitfieldAny(BitfieldAny {
            bits: u32::try_from(bytes.len() * 8).expect("small"),
            bytes,
        })
    };
    assert_eq!(unlocked_key(&bits(vec![0x00, 0x02])), Some(10));
    assert_eq!(unlocked_key(&bits(vec![0x00, 0x03])), None);
    assert_eq!(unlocked_key(&bits(vec![0, 0])), None);
    assert_eq!(unlocked_key(&Value::Int(4)), None);
    assert_eq!(unlocked_key(&Value::Bitfield32(1)), Some(1));
    let reg = registry();
    let o = VitalDisplayOverride::from_properties(
        &set(vec![
            p(&reg, "VitalDisplayOverride_RequiredEnum", Value::Enum(10)),
            p(&reg, "VitalDisplayOverride_UIState", Value::Enum(0x55)),
            p(
                &reg,
                "VitalDisplayOverride_IsForMonsterCharacters",
                Value::Bool(true),
            ),
            p(
                &reg,
                "VitalDisplayOverride_IsForFreepCharacters",
                Value::Bool(false),
            ),
        ]),
        &reg,
    )
    .expect("override");
    assert_eq!(o.key, unlocked_key(&bits(vec![0x00, 0x02])));

    // §7.1, GCI17: the unlock is the element of the item's top-level usage array; a
    // top-level value or one inside a struct is not an unlock, other elements are
    // skipped by label.
    let item = set(vec![
        p(
            &reg,
            "Usage_SetPropertyUsageData_Array",
            Value::Array(vec![
                p(&reg, "Test_GcInt", Value::Int(3)),
                p(
                    &reg,
                    "VitalDisplayOverride_UnlockedVDOs",
                    bits(vec![0x00, 0x02]),
                ),
            ]),
        ),
        p(&reg, "VitalDisplayOverride_UnlockedVDOs", bits(vec![0x01])),
    ]);
    assert_eq!(item_vdo_unlocks(&item, &reg).expect("item"), vec![Some(10)]);
    let nested = set(vec![p(
        &reg,
        "Usage_SetPropertyUsageData_Array",
        Value::Array(vec![p(
            &reg,
            "Test_GcStruct",
            st(vec![p(
                &reg,
                "VitalDisplayOverride_UnlockedVDOs",
                bits(vec![0x01]),
            )]),
        )]),
    )]);
    assert!(item_vdo_unlocks(&nested, &reg).expect("item").is_empty());
    assert!(
        item_vdo_unlocks(&set(vec![]), &reg)
            .expect("no array")
            .is_empty()
    );
    let wrong = set(vec![p(
        &reg,
        "Usage_SetPropertyUsageData_Array",
        Value::Int(1),
    )]);
    assert!(matches!(
        item_vdo_unlocks(&wrong, &reg),
        Err(WStateError::BadProperty { .. })
    ));
    assert_eq!(
        (o.ui_state, o.for_monsters, o.for_free_peoples),
        (Some(0x55), Some(true), Some(false))
    );
}

/// §11.1: list M names table A for elements 1–10 and table B for 11–120; a level-20
/// viewer considers a level-22 monster.
#[test]
fn consideration_worked_example() {
    const TRIVIAL: u32 = 10;
    const INSANELY_EASY: u32 = 20;
    const VERY_EASY: u32 = 30;
    const EASY: u32 = 40;
    const EVEN: u32 = 50;
    const TOUGH: u32 = 60;
    const VERY_TOUGH: u32 = 70;
    const INSANELY_TOUGH: u32 = 80;
    const IMPOSSIBLE: u32 = 90;
    const MONSTER: u32 = 2;
    const STATE_S: u32 = 0x1234;
    let (a, b) = (Did(0x7000_0A00), Did(0x7000_0B00));
    let reg = registry();
    let list: Vec<Property> = (1..=120)
        .map(|i| {
            p(
                &reg,
                "Test_GcDid",
                Value::DidRef(if i <= 10 { a } else { b }),
            )
        })
        .collect();
    let control = ConsiderationControl::from_properties(
        &set(vec![
            p(
                &reg,
                "ConsiderationControl_RankToUIState",
                structs(
                    &reg,
                    vec![vec![
                        p(
                            &reg,
                            "ConsiderationControl_ConsiderRank",
                            Value::Enum(TOUGH),
                        ),
                        p(&reg, "ConsiderationControl_UIStateID", Value::Enum(STATE_S)),
                    ]],
                ),
            ),
            p(
                &reg,
                "ConsiderationControl_CombatTableData",
                structs(
                    &reg,
                    vec![vec![
                        p(
                            &reg,
                            "ConsiderationControl_ConsiderType",
                            Value::Enum(MONSTER),
                        ),
                        p(
                            &reg,
                            "ConsiderationControl_CombatTableList",
                            Value::Array(list),
                        ),
                    ]],
                ),
            ),
        ]),
        &reg,
    )
    .expect("control");
    let mut ranks = Vec::new();
    for (n, r) in [
        (7, TRIVIAL),
        (3, INSANELY_EASY),
        (3, VERY_EASY),
        (2, EASY),
        (1, EVEN),
        (2, TOUGH),
        (2, VERY_TOUGH),
        (3, INSANELY_TOUGH),
        (8, IMPOSSIBLE),
    ] {
        ranks.extend(std::iter::repeat_n(r, n));
    }
    let table_b = ConsiderationTable::from_properties(
        &set(vec![p(
            &reg,
            "ConsiderationTable_RankPerDifferentialArray",
            Value::Array(
                ranks
                    .iter()
                    .map(|&r| p(&reg, "ConsiderationTable_ConsiderRank", Value::Enum(r)))
                    .collect(),
            ),
        )]),
        &reg,
    )
    .expect("table");
    assert!(table_b.is_well_formed(EVEN));
    assert!(!table_b.is_well_formed(TOUGH));
    let m = control.combat_list(MONSTER).expect("list M");
    assert_eq!(m.len(), 120);
    assert_eq!(ConsiderationControl::table_for_level(m, 20), Some(b));
    assert_eq!(ConsiderationControl::table_for_level(m, 10), Some(a));
    assert_eq!(ConsiderationControl::table_for_level(m, 0), None);
    assert_eq!(ConsiderationControl::table_for_level(m, 121), None);
    let rank = table_b.rank_for_difference(22 - 20).expect("rank");
    assert_eq!(rank, TOUGH);
    assert_eq!(control.ui_state(rank), Some(STATE_S));
    assert_eq!(table_b.rank_for_difference(-20), Some(TRIVIAL));
    assert_eq!(table_b.rank_for_difference(20), Some(IMPOSSIBLE));
    assert_eq!(table_b.rank_for_difference(i64::MIN), Some(TRIVIAL));
    assert_eq!(table_b.rank_for_difference(i64::MAX), Some(IMPOSSIBLE));
    assert_eq!(ConsiderationTable::default().rank_for_difference(0), None);
}

#[test]
fn alerts_tracking_and_small_controls() {
    let reg = registry();
    let alerts = AlertsControl::from_properties(
        &set(vec![
            p(&reg, "AlertControl_MaxNumberOfBigAlerts", Value::Int(5)),
            p(
                &reg,
                "AlertControl_TypeToOverflow_Array",
                structs(
                    &reg,
                    vec![
                        vec![
                            p(&reg, "Alert_Type", Value::Enum(1)),
                            p(&reg, "Alert_IsOverflow", Value::Bool(true)),
                        ],
                        vec![p(&reg, "Alert_Type", Value::Enum(2))],
                    ],
                ),
            ),
        ]),
        &reg,
    )
    .expect("alerts");
    assert!(alerts.types_are_distinct_and_non_zero());
    let zero = AlertsControl {
        max_big_alerts: None,
        overflow: vec![(Some(0), None)],
    };
    assert!(!zero.types_are_distinct_and_non_zero());
    let tracking = TrackingStateControl {
        entries: vec![(Some(1), Some(3)), (Some(2), Some(3))],
    };
    assert!(!tracking.states_are_distinct());
    let music = CombatMusicControl::from_properties(
        &set(vec![
            p(&reg, "CombatMusicControl_Floor", Value::Int(-5)),
            p(&reg, "CombatMusicControl_Ceiling", Value::Int(15)),
        ]),
        &reg,
    )
    .expect("music");
    assert_eq!((music.floor, music.ceiling), (Some(-5), Some(15)));
    let tapping = OpenTappingControl::from_properties(
        &set(vec![p(
            &reg,
            "OpenTapping_MaxCreditDistance",
            Value::Float(12.0),
        )]),
        &reg,
    )
    .expect("tapping");
    assert_eq!(tapping.max_credit_distance, Some(12.0));
}

#[test]
fn guild_player_note_default() {
    let reg = registry();
    let g = GuildControl::from_properties(
        &set(vec![p(&reg, "Guild_MaxMembers", Value::Int(100))]),
        &reg,
    )
    .expect("guild");
    assert_eq!(g.player_note_limit(), PLAYER_NOTE_DEFAULT);
    let g = GuildControl::from_properties(
        &set(vec![p(&reg, "Guild_PlayerNoteMaxLength", Value::Int(64))]),
        &reg,
    )
    .expect("guild");
    assert_eq!(g.player_note_limit(), 64);
    let g = GuildControl::from_properties(
        &set(vec![p(
            &reg,
            "Guild_PlayerNoteMaxLength",
            Value::Float(64.0),
        )]),
        &reg,
    )
    .expect("guild");
    assert_eq!(g.player_note_limit(), 48);
}

#[test]
fn views_never_panic_on_hostile_sets() {
    let reg = registry();
    // Every property of the dictionary with a value of the wrong type, alone.
    for (name, _) in crate::testkit::GAME_CONTROL_PROPERTIES {
        for v in [
            Value::Int(-1),
            Value::Array(vec![p(&reg, "Test_GcInt", Value::Int(1))]),
            Value::Struct(Box::default()),
        ] {
            let s = set(vec![p(&reg, name, v)]);
            let _ = MountDirectory::from_properties(&s, &reg);
            let _ = MountTrait::from_properties(&s, &reg);
            let _ = MountedCombatControl::from_properties(&s, &reg);
            let _ = DuelTimings::from_properties(&s, &reg);
            let _ = PerkControl::from_properties(&s, &reg);
            let _ = Perk::from_properties(&s, &reg);
            let _ = CollectionControl::from_properties(&s, &reg);
            let _ = Collection::from_properties(&s, &reg);
            let _ = PaperItemServices::from_properties(&s, &reg);
            let _ = PaperItemService::from_properties(&s, &reg);
            let _ = RandomItemRewardControl::from_properties(&s, &reg);
            let _ = LoginReward::from_properties(&s, &reg);
            let _ = VitalDisplayOverrideControl::from_properties(&s, &reg);
            let _ = VitalDisplayOverride::from_properties(&s, &reg);
            let _ = AlertsControl::from_properties(&s, &reg);
            let _ = ResetScheduleControl::from_properties(&s, &reg);
            let _ = ConsiderationControl::from_properties(&s, &reg);
            let _ = ConsiderationTable::from_properties(&s, &reg);
            let _ = TrackingStateControl::from_properties(&s, &reg);
            let _ = DiscoveryDistanceControl::from_properties(&s, &reg);
            let _ = CombatMusicControl::from_properties(&s, &reg);
            let _ = AggroPingControl::from_properties(&s, &reg);
            let _ = OpenTappingControl::from_properties(&s, &reg);
            let _ = GuildControl::from_properties(&s, &reg);
        }
    }
}
