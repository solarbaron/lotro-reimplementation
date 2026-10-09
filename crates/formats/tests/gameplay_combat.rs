// SPDX-License-Identifier: GPL-3.0-or-later
//! Synthetic checks of the combat views (`tessera_formats::gameplay::combat`) against the
//! worked examples and invariants of `docs/specs/combat-data.md`. Every property set is
//! built here; PIDs are synthetic (labels resolved against a made-up dictionary).

use std::collections::HashMap;

use tessera_formats::gameplay::combat::{
    CombatClasses, CombatFamily, CombatObject, CombatPids, ControlPoint, DetectionVolume,
    DurationGroup, Effect, EffectRefKind, EffectRole, LevelRange, Lookup, ModEntryKind, ModOp,
    Progression, ProgressionKind, ProgressionValue, QualityTable, RelatedObject, Skill, Trait,
    TraitCell, TraitTree, evaluate_progression, mod_entries, ranged_groups, read_combat_object,
    sandbox_combine, segment_slope, value_arrays_present,
};
use tessera_formats::gameplay::common::build::{array, did, entry, set, structure};
use tessera_formats::gameplay::common::{ClassTree, GameObjectRecord, ObjectRoot, Pid};
use tessera_formats::{Did, PropertyEntry, PropertySet, PropertyValue, StringInfo, StringSource};

/// A made-up dictionary: every combat label gets PID 0x2000_0000 + its position.
fn pids() -> CombatPids {
    let ids: HashMap<String, u32> = CombatPids::labels()
        .into_iter()
        .zip(0x2000_0000u32..)
        .map(|(l, i)| (l.to_owned(), i))
        .collect();
    let pids = CombatPids::resolve(&ids);
    assert!(pids.missing().is_empty());
    pids
}

fn id(p: Pid) -> u32 {
    p.id().expect("synthetic label resolved")
}

fn e(p: Pid, v: PropertyValue) -> PropertyEntry {
    entry(id(p), v)
}

fn text(token: u32) -> PropertyValue {
    PropertyValue::StringInfo(Box::new(StringInfo {
        source: StringSource::Table {
            token,
            table: Did(0x2500_0001),
        },
        args: None,
    }))
}

fn record(did_raw: u32, class: u32, entries: Vec<PropertyEntry>) -> GameObjectRecord {
    GameObjectRecord {
        root: ObjectRoot {
            did: Did(did_raw),
            root_class: class,
        },
        properties: set(entries),
    }
}

fn float_array(p: &CombatPids, min: Option<i32>, values: &[f32]) -> PropertySet {
    let pp = &p.progression;
    let mut entries = vec![e(
        pp.float_array,
        array(
            values
                .iter()
                .map(|&v| e(pp.float_value, PropertyValue::Float(v)))
                .collect(),
        ),
    )];
    if let Some(m) = min {
        entries.push(e(pp.min_index, PropertyValue::Int(m)));
    }
    set(entries)
}

fn linear(p: &CombatPids, points: &[(i32, f32)]) -> PropertySet {
    let pp = &p.progression;
    set(vec![e(
        pp.linear_array,
        array(
            points
                .iter()
                .map(|&(k, v)| {
                    e(
                        pp.linear_point,
                        structure(vec![
                            e(pp.linear_key, PropertyValue::Int(k)),
                            e(pp.linear_value, PropertyValue::Float(v)),
                        ]),
                    )
                })
                .collect(),
        ),
    )])
}

fn float_of(v: Option<ProgressionValue>) -> Option<f32> {
    match v? {
        ProgressionValue::Float(f) => Some(f),
        _ => None,
    }
}

// --- §4.12 worked examples -------------------------------------------------------------------

#[test]
fn float_array_with_minimum_index() {
    let p = pids();
    let prog = Progression::from_properties(
        &float_array(&p, Some(1), &[0.5, 0.75, 1.0]),
        false,
        &p.progression,
    )
    .unwrap();
    assert_eq!(prog.kind(), ProgressionKind::FloatArray);
    assert_eq!(prog.covered_range(), Some((1, 3)));
    assert_eq!(float_of(prog.value_at(1)), Some(0.5));
    assert_eq!(float_of(prog.value_at(3)), Some(1.0));
    // Out of range without the clamp (what a viewer shows, §15)...
    assert_eq!(prog.lookup(4), Lookup::AboveRange);
    assert_eq!(prog.lookup(0), Lookup::BelowRange);
    // ...and with D1.
    assert_eq!(float_of(prog.value_at(4)), Some(1.0));
    assert_eq!(float_of(prog.value_at(-50)), Some(0.5));
}

#[test]
fn float_array_without_minimum_index_starts_at_zero() {
    let p = pids();
    let prog =
        Progression::from_properties(&float_array(&p, None, &[2.0, 4.0]), false, &p.progression)
            .unwrap();
    assert_eq!(prog.stored_min_index, None);
    assert_eq!(prog.min_index(), 0);
    assert_eq!(float_of(prog.value_at(0)), Some(2.0));
    assert_eq!(float_of(prog.value_at(1)), Some(4.0));
}

#[test]
fn linear_example() {
    let p = pids();
    let prog = Progression::from_properties(
        &linear(&p, &[(1, 10.0), (11, 30.0), (21, 30.0)]),
        false,
        &p.progression,
    )
    .unwrap();
    assert_eq!(prog.kind(), ProgressionKind::LinearInterpolating);
    assert_eq!(prog.min_index(), 1); // P5: the first key
    let segs = prog.segments();
    assert_eq!(segs.len(), 2);
    assert_eq!(segs[0].slope.to_bits(), 2.0f32.to_bits());
    assert_eq!(segs[1].slope.to_bits(), 0.0f32.to_bits());
    assert_eq!(float_of(prog.value_at(6)), Some(20.0));
    assert_eq!(float_of(prog.value_at(11)), Some(30.0));
    assert_eq!(float_of(prog.value_at(15)), Some(30.0));
    assert_eq!(float_of(prog.value_at(21)), Some(30.0)); // D3
    assert_eq!(prog.lookup(25), Lookup::AboveRange);
    assert_eq!(float_of(prog.value_at(25)), Some(30.0)); // D1
}

#[test]
fn linear_value_at_last_key_is_the_stored_value() {
    // A case where the last segment's own formula would round differently (D3).
    let p = pids();
    let pts = [(1, 0.1f32), (4, 0.7f32)];
    let prog = Progression::from_properties(&linear(&p, &pts), false, &p.progression).unwrap();
    assert_eq!(float_of(prog.value_at(4)), Some(0.7));
    let s = segment_slope(
        ControlPoint { key: 1, value: 0.1 },
        ControlPoint { key: 4, value: 0.7 },
    );
    let inner = 0.1f32 + s * 2.0f32;
    assert_eq!(
        float_of(prog.value_at(3)).map(f32::to_bits),
        Some(inner.to_bits())
    );
}

#[test]
fn linear_single_point_and_malformed_keys() {
    let p = pids();
    let one =
        Progression::from_properties(&linear(&p, &[(5, 7.0)]), false, &p.progression).unwrap();
    for x in [-100, 0, 5, 9999] {
        assert_eq!(float_of(one.value_at(x)), Some(7.0));
    }
    // P3 broken: keys not strictly increasing → none, logged.
    let bad =
        Progression::from_properties(&linear(&p, &[(1, 1.0), (1, 2.0)]), false, &p.progression)
            .unwrap();
    assert!(!bad.keys_increasing());
    assert_eq!(bad.value_at(1), None);
    assert_eq!(bad.lookup(1), Lookup::Malformed);
    assert!(bad.segments().is_empty());
    let empty = Progression::from_properties(&linear(&p, &[]), false, &p.progression).unwrap();
    assert_eq!(empty.lookup(1), Lookup::Empty);
    assert_eq!(empty.value_at(1), None);
}

#[test]
fn progression_of_progressions_example() {
    let p = pids();
    let pp = &p.progression;
    let (a, b, outer) = (Did(0x7000_0A01), Did(0x7000_0A02), Did(0x7000_0A03));
    let nested = set(vec![
        e(
            pp.did_array,
            array(vec![e(pp.did_value, did(a.0)), e(pp.did_value, did(b.0))]),
        ),
        e(pp.min_index, PropertyValue::Int(1)),
    ]);
    let mut source: HashMap<Did, Progression> = HashMap::new();
    source.insert(
        a,
        Progression::from_properties(&float_array(&p, Some(1), &[10.0, 20.0, 30.0]), false, pp)
            .unwrap(),
    );
    source.insert(
        b,
        Progression::from_properties(&linear(&p, &[(1, 100.0), (3, 300.0)]), false, pp).unwrap(),
    );
    let prog = Progression::from_properties(&nested, true, pp).unwrap();
    assert_eq!(prog.kind(), ProgressionKind::ProgressionOfProgressions);
    source.insert(outer, prog.clone());

    let eval = |o, i| float_of(evaluate_progression(&source, outer, o, i));
    assert_eq!(eval(2, Some(2)), Some(200.0));
    assert_eq!(eval(1, Some(3)), Some(30.0));
    assert_eq!(eval(3, Some(1)), Some(100.0)); // outer clamped (D1)
    assert_eq!(eval(1, None), None); // D4
    // The same through the loaded record.
    assert_eq!(float_of(prog.evaluate(&source, 2, Some(2))), Some(200.0));
    // Without nesting the same array is a DID array.
    let flat = Progression::from_properties(&nested, false, pp).unwrap();
    assert_eq!(flat.kind(), ProgressionKind::DidArray);
    assert_eq!(flat.value_at(2), Some(ProgressionValue::Did(b)));
}

#[test]
fn chains_stop_and_missing_targets_give_none() {
    let p = pids();
    let pp = &p.progression;
    // A progression of progressions whose element is itself: one level, then none (D4).
    let me = Did(0x7000_0B01);
    let selfref = set(vec![e(
        pp.did_array,
        array(vec![e(pp.did_value, did(me.0))]),
    )]);
    let mut source: HashMap<Did, Progression> = HashMap::new();
    source.insert(
        me,
        Progression::from_properties(&selfref, true, pp).unwrap(),
    );
    assert_eq!(evaluate_progression(&source, me, 0, Some(0)), None);
    // Missing record, null DID, a closure source.
    assert_eq!(
        evaluate_progression(&source, Did(0x7000_0B02), 0, None),
        None
    );
    assert_eq!(evaluate_progression(&source, Did(0), 0, None), None);
    let loader = |d: Did| {
        (d == Did(0x7000_0B03))
            .then(|| {
                Progression::from_properties(&float_array(&p, Some(1), &[9.0]), false, pp).ok()
            })
            .flatten()
    };
    assert_eq!(
        float_of(evaluate_progression(&loader, Did(0x7000_0B03), 7, None)),
        Some(9.0)
    );
}

#[test]
fn property_array_shares_one_pid_and_rejects_nulls() {
    let p = pids();
    let pp = &p.progression;
    let produced = 0x1234_5678;
    let s = set(vec![
        e(
            pp.property_array,
            array(vec![
                entry(produced, PropertyValue::Int(3)),
                entry(produced, PropertyValue::Int(5)),
            ]),
        ),
        e(pp.min_index, PropertyValue::Int(1)),
    ]);
    let prog = Progression::from_properties(&s, false, pp).unwrap();
    assert_eq!(prog.produced_property(), Some(produced));
    assert_eq!(
        prog.value_at(2),
        Some(ProgressionValue::Property(PropertyValue::Int(5)))
    );
    assert_eq!(value_arrays_present(&s, pp), 1);

    // A null element would shift the indices: rejected.
    let mut arr = array(vec![e(pp.float_value, PropertyValue::Float(1.0))]);
    if let PropertyValue::Array(a) = &mut arr {
        a.count = 2;
    }
    let nulls = set(vec![e(pp.float_array, arr)]);
    assert!(Progression::from_properties(&nulls, false, pp).is_err());
    // Wrong element type, no value array.
    let wrong = set(vec![e(
        pp.float_array,
        array(vec![e(pp.float_value, PropertyValue::Int(1))]),
    )]);
    assert!(Progression::from_properties(&wrong, false, pp).is_err());
    assert!(Progression::from_properties(&set(vec![]), false, pp).is_err());
}

// --- §5 modifier entries -----------------------------------------------------------------------

fn mod_entry(p: &CombatPids, fields: Vec<PropertyEntry>) -> PropertyEntry {
    e(p.modifier.entry, structure(fields))
}

#[test]
fn worked_example_5_9() {
    let p = pids();
    let m = &p.modifier;
    let target = 0x1000_9999;
    let holder = set(vec![e(
        m.array,
        array(vec![
            mod_entry(
                &p,
                vec![
                    e(m.modified, PropertyValue::PropertyId(target)),
                    e(m.op, PropertyValue::Enum(7)),
                    entry(target, PropertyValue::Float(0.25)),
                ],
            ),
            mod_entry(
                &p,
                vec![
                    e(m.modified, PropertyValue::PropertyId(target)),
                    e(m.op, PropertyValue::Enum(8)),
                    entry(target, PropertyValue::Float(2.0)),
                ],
            ),
        ]),
    )]);
    let entries = mod_entries(&holder, m);
    assert_eq!(entries.len(), 2);
    let none: HashMap<Did, Progression> = HashMap::new();
    let applied: Vec<(ModOp, PropertyValue)> = entries
        .iter()
        .map(|en| {
            assert_eq!(en.kind, ModEntryKind::Inline);
            (
                en.operator(None).unwrap(),
                en.resolve_value(&none, 1, None).unwrap(),
            )
        })
        .collect();
    // Order does not matter in the sandbox (D2): multiply after add either way.
    let mut reversed = applied.clone();
    reversed.reverse();
    for list in [applied, reversed] {
        assert_eq!(
            sandbox_combine(&PropertyValue::Float(1.0), &list),
            PropertyValue::Float(2.5)
        );
    }
}

#[test]
fn sandbox_order_and_types() {
    use PropertyValue as V;
    let applied = vec![
        (ModOp::Multiply, V::Int(3)),
        (ModOp::Set, V::Int(10)),
        (ModOp::Subtract, V::Int(4)),
        (ModOp::Set, V::Int(2)),
        (ModOp::Add, V::Float(1.0)), // type mismatch: skipped
        (ModOp::Divide, V::Int(2)),  // no sandbox meaning: skipped
    ];
    // last Set (2), then − 4, then × 3.
    assert_eq!(sandbox_combine(&V::Int(100), &applied), V::Int(-6));
    assert_eq!(
        sandbox_combine(&V::Int(i32::MAX), &[(ModOp::Add, V::Int(1))]),
        V::Int(i32::MAX)
    );
    assert_eq!(
        sandbox_combine(
            &V::Bitfield32(0b0101),
            &[(ModOp::Or, V::Bitfield32(0b0010))]
        ),
        V::Bitfield32(0b0111)
    );
    assert_eq!(
        sandbox_combine(&V::Bool(false), &[(ModOp::Or, V::Bool(true))]),
        V::Bool(true)
    );
    let a = array(vec![entry(1, V::Int(1))]);
    let b = array(vec![entry(1, V::Int(2)), entry(1, V::Int(3))]);
    let PropertyValue::Array(out) = sandbox_combine(&a, &[(ModOp::ArrayAdd, b)]) else {
        panic!("array expected");
    };
    assert_eq!(out.elements.len(), 3);
    assert_eq!(out.count, 3);
}

#[test]
fn entry_kinds() {
    let p = pids();
    let m = &p.modifier;
    let target = 0x1000_0001;
    let prog = Did(0x7000_0C01);
    let holder = set(vec![e(
        m.array,
        array(vec![
            // Empty: Mod_Modified 0, with the placeholder fields of §5.2.
            mod_entry(
                &p,
                vec![
                    e(m.modified, PropertyValue::PropertyId(0)),
                    e(m.op, PropertyValue::Enum(0)),
                    e(m.progression, did(0)),
                    e(
                        m.modifier_list,
                        array(vec![
                            e(m.modifier_property, PropertyValue::PropertyId(0)),
                            e(m.modifier_property, PropertyValue::PropertyId(0)),
                        ]),
                    ),
                ],
            ),
            mod_entry(&p, vec![]),
            mod_entry(
                &p,
                vec![
                    e(m.modified, PropertyValue::PropertyId(target)),
                    e(m.op, PropertyValue::Enum(7)),
                    e(m.progression, did(prog.0)),
                    e(
                        m.modifier_list,
                        array(vec![
                            e(m.modifier_property, PropertyValue::PropertyId(0x1000_0002)),
                            e(m.modifier_property, PropertyValue::PropertyId(0)),
                        ]),
                    ),
                    e(m.modifier_op, PropertyValue::Enum(7)),
                ],
            ),
            mod_entry(
                &p,
                vec![
                    e(m.modified, PropertyValue::PropertyId(target)),
                    e(m.progression, did(prog.0)),
                    entry(target, PropertyValue::Float(1.0)),
                    entry(0x1000_7777, PropertyValue::Int(0)),
                ],
            ),
            mod_entry(&p, vec![e(m.modified, PropertyValue::PropertyId(target))]),
        ]),
    )]);
    let entries = mod_entries(&holder, m);
    let kinds: Vec<ModEntryKind> = entries.iter().map(|x| x.kind).collect();
    assert_eq!(
        kinds,
        [
            ModEntryKind::Empty,
            ModEntryKind::FieldLess,
            ModEntryKind::ProgressionDriven,
            ModEntryKind::Conflicting,
            ModEntryKind::Valueless,
        ]
    );
    assert!(entries[0].is_skipped() && entries[1].is_skipped());
    assert!(entries[0].modifier_list.is_empty());
    assert_eq!(entries[2].modifier_list, [0x1000_0002]);
    assert_eq!(entries[2].progression, Some(prog));
    assert_eq!(entries[3].unrelated_fields, [0x1000_7777]);

    // The progression-driven entry's value comes from the progression.
    let mut source = HashMap::new();
    source.insert(
        prog,
        Progression::from_properties(&linear(&p, &[(1, 5.0), (3, 7.0)]), false, &p.progression)
            .unwrap(),
    );
    assert_eq!(
        entries[2].resolve_value(&source, 2, None),
        Some(PropertyValue::Float(6.0))
    );
    assert_eq!(entries[0].resolve_value(&source, 2, None), None);
}

#[test]
fn ranged_groups_sort_and_select() {
    let p = pids();
    let m = &p.modifier;
    let a = 0x1000_0001;
    let b = 0x1000_0002;
    let ranged = |pid: u32, floor: i32, ceiling: i32| {
        mod_entry(
            &p,
            vec![
                e(m.modified, PropertyValue::PropertyId(pid)),
                e(m.op, PropertyValue::Enum(7)),
                entry(pid, PropertyValue::Float(1.0)),
                e(m.floor, PropertyValue::Int(floor)),
                e(m.ceiling, PropertyValue::Int(ceiling)),
            ],
        )
    };
    // The stored order of §5.5's 77 groups: [10, 19], [51, 399], [20, 50].
    let holder = set(vec![e(
        m.array,
        array(vec![
            ranged(a, 10, 19),
            ranged(a, 51, 399),
            ranged(a, 20, 50),
            ranged(b, 10, 19),
            ranged(b, 51, 399),
        ]),
    )]);
    let entries = mod_entries(&holder, m);
    let groups = ranged_groups(&entries);
    assert_eq!(groups.len(), 2);
    let ga = &groups[0];
    assert_eq!(ga.modified, a);
    assert!(!ga.stored_ascending);
    assert!(ga.is_disjoint() && ga.is_contiguous());
    assert_eq!(ga.unranged, 0);
    let floors: Vec<_> = ga.members.iter().map(|(_, r)| r.floor).collect();
    assert_eq!(floors, [Some(10), Some(20), Some(51)]);
    assert_eq!(ga.select(30), Some(2));
    assert_eq!(ga.select(9), None);
    let gb = &groups[1];
    assert!(gb.is_disjoint() && !gb.is_contiguous());
    assert_eq!(gb.select(30), None); // the 20–50 gap
    assert!(entries[0].applies_at(Some(15)) && !entries[0].applies_at(Some(20)));
    assert!(!entries[0].applies_at(None));

    let open = LevelRange {
        floor: Some(70),
        ceiling: None,
    };
    assert!(open.contains(5000) && !open.contains(69));
}

// --- §2 classification -------------------------------------------------------------------------

fn observed_tree() -> ClassTree {
    ClassTree::from_edges([
        (705, 698),
        (734, 698),
        (713, 734),
        (767, 705),
        (1477, 1479),
        (1494, 1477),
        (3712, 1494),
        (3438, 3712),
        (3920, 1386),
        (3918, 3920),
        (1385, 3920),
        (1389, 1386),
        (2519, 1386),
        (3810, 2519),
        (2390, 2389),
        (3531, 3530),
        (3590, 3584),
    ])
}

#[test]
fn classification_rules() {
    let c = CombatClasses::observed();
    let t = observed_tree();
    let fam = |class| c.family(&t, class);
    assert_eq!(fam(713), Some(CombatFamily::Effect));
    assert_eq!(fam(767), Some(CombatFamily::Effect));
    assert_eq!(fam(698), Some(CombatFamily::Effect));
    assert_eq!(fam(3438), Some(CombatFamily::Trait));
    assert_eq!(fam(3810), Some(CombatFamily::Progression));
    assert_eq!(fam(827), Some(CombatFamily::Skill));
    assert_eq!(fam(1932), Some(CombatFamily::MonsterPlayOrAdvancementLine));
    assert_eq!(fam(2390), Some(CombatFamily::MonsterPlayOrAdvancementLine));
    assert_eq!(fam(2389), None); // strict ancestor only
    assert_eq!(fam(853), Some(CombatFamily::ClassTable));
    assert_eq!(fam(3531), Some(CombatFamily::ThresholdPolicy));
    assert_eq!(fam(3530), None);
    assert_eq!(fam(3590), Some(CombatFamily::BattleStatistic));
    assert_eq!(fam(1195), None);
    assert_eq!(c.related(1195), Some(RelatedObject::Induction));
    assert_eq!(c.related(3486), Some(RelatedObject::TraitTree));
    assert_eq!(
        c.progression_kind(&t, 3810),
        Some(ProgressionKind::ProgressionOfProgressions)
    );
    assert_eq!(
        c.progression_kind(&t, 2519),
        Some(ProgressionKind::DidArray)
    );
    assert_eq!(
        c.progression_kind(&t, 3918),
        Some(ProgressionKind::LinearInterpolating)
    );
    assert_eq!(c.effect_role(734), EffectRole::PropertyModification);
    assert_eq!(c.effect_role(3943), EffectRole::Other);
}

// --- §6 effects --------------------------------------------------------------------------------

fn effect_entries(p: &CombatPids, extra: Vec<PropertyEntry>) -> Vec<PropertyEntry> {
    let ef = &p.effect;
    let mut v = vec![
        e(ef.name, text(1)),
        e(ef.definition_description, text(2)),
        e(ef.applied_description, text(3)),
    ];
    v.extend(extra);
    v
}

#[test]
fn effect_duration_groups() {
    let p = pids();
    let ef = &p.effect;
    let cases: Vec<(Vec<PropertyEntry>, DurationGroup)> = vec![
        (vec![], DurationGroup::Unspecified),
        (
            vec![e(ef.instantaneous, PropertyValue::Bool(true))],
            DurationGroup::Instantaneous,
        ),
        (
            vec![
                e(ef.duration_permanent, PropertyValue::Bool(true)),
                e(ef.instantaneous, PropertyValue::Bool(false)),
            ],
            DurationGroup::Permanent,
        ),
        (
            vec![e(ef.duration_interval, PropertyValue::Float(30.0))],
            DurationGroup::Interval,
        ),
        (
            vec![
                e(ef.duration_interval, PropertyValue::Float(30.0)),
                e(ef.duration_pulse_count, PropertyValue::Int(10)),
            ],
            DurationGroup::Pulsed,
        ),
        (
            vec![
                e(ef.duration_interval, PropertyValue::Float(30.0)),
                e(ef.duration_permanent, PropertyValue::Bool(true)),
            ],
            DurationGroup::IntervalPermanent,
        ),
        (
            vec![
                e(ef.duration_interval, PropertyValue::Float(30.0)),
                e(ef.instantaneous, PropertyValue::Bool(true)),
            ],
            DurationGroup::Other,
        ),
    ];
    for (extra, group) in cases {
        let rec = record(0x7000_1000, 734, effect_entries(&p, extra));
        let eff = Effect::read(&rec, &p).unwrap();
        assert_eq!(eff.duration.group(), group);
    }
}

#[test]
fn effect_requires_its_signature() {
    let p = pids();
    let rec = record(0x7000_1001, 734, vec![e(p.effect.name, text(1))]);
    assert!(Effect::read(&rec, &p).is_err());
    assert!(!p.has_signature(CombatFamily::Effect, &rec.properties));
    let ok = record(0x7000_1001, 734, effect_entries(&p, vec![]));
    assert!(p.has_signature(CombatFamily::Effect, &ok.properties));
    assert_eq!(
        Effect::read(&ok, &p)
            .unwrap()
            .application_probability_or_default(),
        1.0
    );
}

#[test]
fn effect_values_and_references() {
    let p = pids();
    let ef = &p.effect;
    let v = &p.vital;
    let r = &p.effect_ref;
    let target = Did(0x7000_2001);
    let nested = Did(0x7000_2002);
    let extra = vec![
        e(v.instant_constant, PropertyValue::Float(50.0)),
        e(v.instant_progression, did(0x7000_3001)),
        e(v.instant_variance, PropertyValue::Float(0.1)),
        e(
            v.instant_random,
            structure(vec![
                e(ef.random_min, PropertyValue::Float(1.0)),
                e(ef.random_max, PropertyValue::Float(2.0)),
            ]),
        ),
        e(ef.base_vital_type, PropertyValue::Enum(1)),
        e(ef.application_probability, PropertyValue::Float(0.5)),
        e(
            p.effect_extra.aura_applied,
            array(vec![]), // present but not an aura signature on its own
        ),
        e(p.effect_extra.aura_radius, PropertyValue::Float(12.0)),
        // A generator list with one struct holding a nested data list.
        entry(
            0x1000_5000,
            array(vec![e(
                r.generator_struct,
                structure(vec![
                    e(r.generator_id, did(target.0)),
                    e(r.generator_spellcraft, PropertyValue::Float(-1.0)),
                    e(
                        r.generator_data_list,
                        array(vec![e(
                            r.skill_effect_data,
                            structure(vec![
                                e(r.skill_effect, did(nested.0)),
                                e(r.skill_spellcraft, PropertyValue::Float(40.0)),
                                e(r.skill_duration, PropertyValue::Float(-1.0)),
                            ]),
                        )]),
                    ),
                ]),
            )]),
        ),
        // A bare list and a placeholder.
        entry(
            0x1000_5001,
            array(vec![
                e(r.bare_effect, did(target.0)),
                e(
                    r.skill_effect_data,
                    structure(vec![e(r.skill_effect, did(0))]),
                ),
            ]),
        ),
    ];
    let rec = record(0x7000_1002, 725, effect_entries(&p, extra));
    let eff = Effect::read(&rec, &p).unwrap();
    let amt = eff.instant_vital.unwrap();
    assert!(amt.has_constant_and_progression());
    assert_eq!(amt.variance, Some(0.1));
    assert_eq!(amt.random.unwrap().max, Some(2.0));
    assert!(eff.over_time_initial.is_none());
    assert_eq!(eff.vital_type, Some(1));
    assert_eq!(eff.application_probability_or_default(), 0.5);
    assert_eq!(eff.aura.unwrap().radius, Some(12.0));

    let refs = &eff.references;
    assert_eq!(refs.len(), 4);
    assert_eq!(refs[0].kind, EffectRefKind::Generator);
    assert_eq!(refs[0].effect, target);
    assert_eq!(refs[0].top, 0x1000_5000);
    assert_eq!(refs[0].given_spellcraft(), None);
    assert_eq!(refs[1].kind, EffectRefKind::SkillEffectData);
    assert_eq!(refs[1].effect, nested);
    assert_eq!(refs[1].given_spellcraft(), Some(40.0));
    assert_eq!(refs[1].given_duration(), None);
    assert_eq!(refs[2].kind, EffectRefKind::Bare);
    assert!(refs[3].is_placeholder());
}

// --- §7 skills -----------------------------------------------------------------------------------

#[test]
fn skill_view() {
    let p = pids();
    let s = &p.skill;
    let h = &p.hook;
    let r = &p.effect_ref;
    let user_effect = Did(0x7000_4001);
    let rec = record(
        0x7000_4000,
        827,
        vec![
            e(s.name, text(10)),
            e(s.skill_type, PropertyValue::Bitfield64(1 << 5)),
            e(s.target_selection_mode, PropertyValue::Enum(2)),
            e(s.max_range, PropertyValue::Float(-1.0)),
            e(s.min_range, PropertyValue::Float(2.0)),
            e(s.arc_radius, PropertyValue::Float(10.0)),
            e(s.arc_degrees, PropertyValue::Float(90.0)),
            e(
                s.vital_cost_list,
                array(vec![e(
                    s.vital_info,
                    structure(vec![
                        e(s.vital_type, PropertyValue::Enum(2)),
                        e(s.vital_points, PropertyValue::Float(15.0)),
                        e(s.vital_points_progression, did(0)),
                    ]),
                )]),
            ),
            e(
                h.list,
                array(vec![e(
                    h.info,
                    structure(vec![
                        e(h.damage_max, PropertyValue::Float(100.0)),
                        e(h.damage_max_progression, did(0x7000_5001)),
                        e(h.animation_hook_index, PropertyValue::Int(1)),
                    ]),
                )]),
            ),
            e(
                s.user_effects,
                array(vec![
                    e(
                        r.skill_effect_data,
                        structure(vec![e(r.skill_effect, did(user_effect.0))]),
                    ),
                    e(
                        r.skill_effect_data,
                        structure(vec![e(r.skill_effect, did(0))]),
                    ),
                ]),
            ),
            e(s.induction, did(0x7000_6001)),
        ],
    );
    assert!(p.has_signature(CombatFamily::Skill, &rec.properties));
    let skill = Skill::read(&rec, &p).unwrap();
    assert_eq!(skill.max_range, None);
    assert_eq!(skill.min_range, Some(2.0));
    assert_eq!(
        skill.detection_volumes,
        [DetectionVolume::Arc {
            radius: 10.0,
            degrees: Some(90.0)
        }]
    );
    assert_eq!(skill.vital_costs.len(), 1);
    assert_eq!(skill.vital_costs[0].points, Some(15.0));
    assert_eq!(skill.vital_costs[0].points_progression, None);
    assert_eq!(skill.attack_hooks.len(), 1);
    assert_eq!(
        skill.attack_hooks[0].damage_max_progression,
        Some(Did(0x7000_5001))
    );
    let users: Vec<Did> = skill
        .references_in(s.user_effects)
        .map(|r| r.effect)
        .collect();
    assert_eq!(users, [user_effect]);
    assert_eq!(skill.timing.induction, Some(Did(0x7000_6001)));
    // Missing signature property → error.
    let bad = record(0x7000_4002, 827, vec![e(s.name, text(1))]);
    assert!(Skill::read(&bad, &p).is_err());
}

// --- §8 traits -----------------------------------------------------------------------------------

#[test]
fn trait_and_tree() {
    let p = pids();
    let t = &p.traits;
    let rec = record(
        0x7000_7000,
        3438,
        vec![
            e(t.name, text(20)),
            e(t.nature, PropertyValue::Enum(4)),
            e(t.category, PropertyValue::Enum(3)),
            e(t.tier, PropertyValue::Int(2)),
            e(t.cost_to_slot, PropertyValue::Int(0)),
            e(t.point_cost_progression, did(0x7000_7101)),
            e(
                t.skill_array,
                array(vec![entry(0x1000_8000, did(0x7000_7201))]),
            ),
        ],
    );
    assert!(p.has_signature(CombatFamily::Trait, &rec.properties));
    let tr = Trait::read(&rec, &p).unwrap();
    assert_eq!(tr.tier, 2);
    assert_eq!(tr.point_cost_progression, Some(Did(0x7000_7101)));
    assert_eq!(tr.skills, [Did(0x7000_7201)]);

    let tree = set(vec![
        e(t.tree_type, PropertyValue::Enum(5)),
        e(
            t.tree_array,
            array(vec![e(
                t.tree_struct,
                structure(vec![
                    e(t.tree_trait, did(0x7000_7000)),
                    e(t.tree_branch, PropertyValue::Enum(1)),
                    e(t.tree_location, PropertyValue::Enum(0x0002_0003)),
                    e(
                        t.tree_dependencies,
                        array(vec![entry(
                            0x1000_8001,
                            structure(vec![
                                e(t.tree_dependency, PropertyValue::Enum(0x0001_0003)),
                                e(t.tree_dependency_rank, PropertyValue::Int(5)),
                            ]),
                        )]),
                    ),
                ]),
            )]),
        ),
    ]);
    let tree = TraitTree::from_properties(&tree, t);
    assert_eq!(tree.entries.len(), 1);
    assert_eq!(tree.entries[0].cell(), Some(TraitCell::Grid { a: 2, b: 3 }));
    assert_eq!(tree.entries[0].dependencies, [(0x0001_0003, Some(5))]);
}

// --- §10 tables --------------------------------------------------------------------------------

#[test]
fn quality_table_looks_qualities_up_by_key() {
    let p = pids();
    let x = &p.tables;
    let quality = |k: u32, f: f32| {
        e(
            x.dps_quality_data,
            structure(vec![
                e(x.dps_quality, PropertyValue::Enum(k)),
                e(x.dps_quality_mod, PropertyValue::Float(f)),
            ]),
        )
    };
    let s = set(vec![
        e(
            x.base_dps_array,
            array(
                (1..=4)
                    .map(|i| e(x.base_dps, PropertyValue::Float(i as f32)))
                    .collect(),
            ),
        ),
        e(
            x.dps_quality_array,
            array(vec![
                quality(5, 1.12),
                quality(4, 1.09),
                quality(2, 1.03),
                quality(3, 1.06),
                quality(1, 1.0),
            ]),
        ),
    ]);
    let table = QualityTable::dps_from_properties(&s, x);
    assert_eq!(table.base, [1.0, 2.0, 3.0, 4.0]);
    assert_eq!(table.quality(1), Some(1.0));
    assert_eq!(table.quality(5), Some(1.12));
    assert_eq!(table.quality(6), None);
}

#[test]
fn read_combat_object_dispatches() {
    let p = pids();
    let c = CombatClasses::observed();
    let t = observed_tree();
    let prog = GameObjectRecord {
        root: ObjectRoot {
            did: Did(0x7000_9001),
            root_class: 1385,
        },
        properties: float_array(&p, Some(1), &[1.0]),
    };
    assert!(matches!(
        read_combat_object(&prog, &c, &t, &p),
        Ok(Some(CombatObject::Progression(_)))
    ));
    let other = record(0x7000_9002, 1, vec![]);
    assert!(matches!(read_combat_object(&other, &c, &t, &p), Ok(None)));
    let induction = record(
        0x7000_9003,
        1195,
        vec![e(p.timing.induction_duration, PropertyValue::Float(1.5))],
    );
    match read_combat_object(&induction, &c, &t, &p) {
        Ok(Some(CombatObject::Induction(i))) => assert_eq!(i.duration, 1.5),
        other => panic!("{other:?}"),
    }
    // A required property missing is an error, not a panic.
    let bad = record(0x7000_9004, 961, vec![]);
    assert!(read_combat_object(&bad, &c, &t, &p).is_err());
}
