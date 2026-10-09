// SPDX-License-Identifier: GPL-3.0-or-later
//! `docs/specs/instances.md` with synthetic fixtures: the worked examples of §17, the
//! client rules of §15.1 B1–B5, the record-local invariants of §16 and hostile inputs.
//! Every fixture is built here; none comes from game data.

mod common;

use std::collections::BTreeSet;

use common::B;
use tessera_formats::instance::battle::{FigureCounts, Removal, RemovalEffect};
use tessera_formats::instance::finder::{
    FlagBits, MatchKind, MatchingIndex, MatchingRules, MatchingTemplate,
};
use tessera_formats::instance::labels as l;
use tessera_formats::instance::skirmish::combine_best_time;
use tessera_formats::instance::template::ContainedLandblock;
use tessera_formats::instance::testkit::{
    self as tk, P_BOOL, P_ENUM, P_INT, P_STRUCT, P_VECTOR, array, did_array, entry, enum_array,
    raw, set, strukt,
};
use tessera_formats::instance::{
    DistantBattleUnit, InstanceKind, InstanceRecord, InstanceVocab, MatchParams, SkirmishControl,
    StatValue, Template, TemplateKind, WorldJoinControl, classify, decode_instance_set,
};
use tessera_formats::{Did, FormatError, PropertyType, PropertyValue, StringInfo};

fn si() -> PropertyValue {
    let bytes = B::new().si_ref(1, 0x25F0_0001).build();
    PropertyValue::StringInfo(Box::new(StringInfo::parse(&bytes).unwrap()))
}

fn d(x: u32) -> PropertyValue {
    PropertyValue::DidRef(Did(x))
}

// ---------------------------------------------------------------- §17.1, N-I4

#[test]
fn landblocks_of_a_template() {
    let lb = |r: f32, x: f32, y: f32| raw(P_VECTOR, PropertyValue::Vector3([r, x, y]));
    let s = set(vec![
        entry(l::TPL_CONTENT_LAYER, PropertyValue::Enum(7)),
        entry(
            l::TPL_LANDBLOCKS,
            array(vec![lb(2.0, 984.0, 904.0), lb(2.0, 979.0, 911.0)]),
        ),
    ]);
    let t = Template::read(&s, &tk::vocab());
    let keys: Vec<_> = t.landblocks.iter().map(ContainedLandblock::key).collect();
    assert_eq!(keys, vec![(2, 123, 113), (2, 122, 113)]);
    assert!(t.landblocks[0].is_aligned());
    assert!(!t.landblocks[1].is_aligned());
    assert!(t.bad_landblocks.is_empty());
    assert!(!t.has_duplicate_landblock());
}

#[test]
fn landblock_vectors_that_break_n_i4() {
    for v in [
        [-1.0, 8.0, 8.0],
        [1.0, 8.5, 8.0],
        [1.0, f32::NAN, 8.0],
        [1.0, 2048.0, 8.0],
        [1.0, 8.0, f32::INFINITY],
    ] {
        assert_eq!(ContainedLandblock::from_vector(v), None, "{v:?}");
    }
    assert_eq!(
        ContainedLandblock::from_vector([1.0, 2047.0, 0.0]).map(|b| b.key()),
        Some((1, 255, 0))
    );
    let s = set(vec![entry(
        l::TPL_LANDBLOCKS,
        array(vec![
            raw(P_VECTOR, PropertyValue::Vector3([-1.0, 0.0, 0.0])),
            raw(P_INT, PropertyValue::Int(3)),
        ]),
    )]);
    let t = Template::read(&s, &tk::vocab());
    assert_eq!(t.bad_landblocks.len(), 2);
    let n_i4 = t
        .deviations()
        .iter()
        .filter(|x| x.invariant == "N-I4")
        .count();
    assert_eq!(n_i4, 2);
}

// ---------------------------------------------------------------- §4.1, N-I3, N-I5

fn skirmish_template(
    tiers: &[u32],
    sizes: &[u32],
    max_players: Option<i32>,
) -> Vec<tessera_formats::PropertyEntry> {
    let tier_min = array(
        (1..=tiers.len() as u32)
            .map(|k| {
                raw(
                    P_STRUCT,
                    strukt(vec![
                        entry(l::SK_DIFFICULTY_TIER, PropertyValue::Enum(k)),
                        entry(l::SK_GROUP_SIZE, PropertyValue::Enum(0)),
                    ]),
                )
            })
            .collect(),
    );
    let mut e = vec![
        entry(l::TPL_NAME, si()),
        entry(l::TPL_DESCRIPTION, si()),
        entry(l::TPL_CONTENT_LAYER, PropertyValue::Enum(12)),
        entry(l::SK_TIER_MIN, tier_min),
        entry(
            l::SK_TIER_ARRAY,
            enum_array(tk::pid(l::SK_DIFFICULTY_TIER), tiers),
        ),
        entry(
            l::SK_GROUP_SIZE_ARRAY,
            enum_array(tk::pid(l::SK_GROUP_SIZE), sizes),
        ),
        entry(l::SK_MIN_LEVEL, PropertyValue::Int(20)),
        entry(l::SK_MAX_LEVEL, PropertyValue::Int(150)),
    ];
    if let Some(m) = max_players {
        e.push(entry(l::TPL_MAX_PLAYERS, PropertyValue::Int(m)));
    }
    e
}

#[test]
fn template_kinds_and_invariants() {
    let v = tk::vocab();
    let ordinary = Template::read(
        &set(vec![entry(l::TPL_CONTENT_LAYER, PropertyValue::Enum(1))]),
        &v,
    );
    assert_eq!(ordinary.kind(), TemplateKind::Ordinary);
    // N-I3: no name, no description.
    assert_eq!(ordinary.deviations().len(), 2);
    assert!(ordinary.content_layer_known(|k| k == 1));
    assert!(!ordinary.content_layer_known(|k| k == 2));

    let mut e = skirmish_template(&[1, 2, 3], &[3, 6], Some(6));
    let t = Template::read(&set(e.clone()), &v);
    assert_eq!(t.kind(), TemplateKind::OtherSkirmish);
    assert!(t.deviations().is_empty(), "{:?}", t.deviations());

    e.push(entry(l::TPL_ENCOUNTER_TYPE, PropertyValue::Enum(1)));
    assert_eq!(Template::read(&set(e), &v).kind(), TemplateKind::Mission);

    let mut e = skirmish_template(&[1], &[1, 2], None);
    e.push(entry(l::WJ_CATEGORY, PropertyValue::Enum(5)));
    assert_eq!(
        Template::read(&set(e), &v).kind(),
        TemplateKind::WorldJoinEncounter
    );

    // N-I5 broken three ways: tiers not 1..n, MaxPlayers not the largest size, min > max.
    let mut e = skirmish_template(&[1, 3], &[3, 6], Some(3));
    e.retain(|x| x.pid != tk::pid(l::SK_MIN_LEVEL));
    e.push(entry(l::SK_MIN_LEVEL, PropertyValue::Int(200)));
    let devs = Template::read(&set(e), &v).deviations();
    assert_eq!(
        devs.iter().filter(|x| x.invariant == "N-I5").count(),
        3,
        "{devs:?}"
    );
}

// ---------------------------------------------------------------- §3.4 classify, bytes

#[test]
fn classify_by_characteristic_property() {
    let v = tk::vocab();
    let cases = [
        (
            l::TPL_CONTENT_LAYER,
            PropertyValue::Enum(1),
            InstanceKind::Template,
        ),
        (
            l::MP_LEVEL_MIN,
            PropertyValue::Int(1),
            InstanceKind::MatchParams,
        ),
        (
            l::ST_ALLOWED_TRAITS,
            did_array(&[1]),
            InstanceKind::SkillTrainerTraits,
        ),
        (
            l::PE_LOCK_ARRAY,
            array(vec![]),
            InstanceKind::EncounterControl,
        ),
        (
            l::EF_REJECT_ERROR,
            PropertyValue::Enum(3),
            InstanceKind::EntryFilter,
        ),
        (
            l::SC_TRAIT_GATES,
            array(vec![]),
            InstanceKind::SkirmishControl,
        ),
        (l::WJC_GROUPS, array(vec![]), InstanceKind::WorldJoinControl),
        (l::TK_MAX_ENCOUNTERS, d(1), InstanceKind::WorldJoinTicket),
        (l::BB_MEDALS, array(vec![]), InstanceKind::BigBattleControl),
        (
            l::DB_WAVES,
            array(vec![]),
            InstanceKind::DistantBattleScript,
        ),
        (
            l::DBU_RESPAWN,
            PropertyValue::Float(1.0),
            InstanceKind::DistantBattleUnit,
        ),
        (l::FC_WIDTHS, array(vec![]), InstanceKind::FormationControl),
        (l::SP_LIST, array(vec![]), InstanceKind::SessionPlayControl),
        (
            l::FM_SIZES,
            array(vec![]),
            InstanceKind::FellowshipMakerControl,
        ),
        (
            l::PB_UPDATE_ONLY,
            PropertyValue::Bool(true),
            InstanceKind::PhasingBox,
        ),
        (l::BS_NAME, si(), InstanceKind::BattleStat),
    ];
    for (label, value, kind) in cases {
        let s = set(vec![entry(label, value)]);
        assert_eq!(classify(&s, &v), Some(kind), "{label}");
        assert_eq!(InstanceRecord::read(&s, &v).map(|r| r.kind()), Some(kind));
    }
    assert_eq!(
        classify(&set(vec![raw(P_INT, PropertyValue::Int(1))]), &v),
        None
    );
    // A dictionary without the labels classifies nothing.
    let empty = InstanceVocab::from_names(&std::collections::BTreeMap::<String, u32>::new());
    assert_eq!(empty.missing().len(), l::ALL.len());
    assert_eq!(
        classify(
            &set(vec![entry(l::TPL_CONTENT_LAYER, PropertyValue::Enum(1))]),
            &empty
        ),
        None
    );
}

/// A 0x79 record holding match parameters, byte for byte.
fn match_params_bytes() -> Vec<u8> {
    let mut b = B::new().u32(0x79F0_0001).tsize(0, 6);
    for (label, value) in [
        (l::MP_LEVEL_MIN, 60u32),
        (l::MP_LEVEL_MAX, 64),
        (l::MP_GROUP_MIN, 1),
        (l::MP_GROUP_MAX, 3),
        (l::MP_TIER_MIN, 1),
        (l::MP_TIER_MAX, 3),
    ] {
        b = b.u32(tk::pid(label)).u32(tk::pid(label)).u32(value);
    }
    b.build()
}

#[test]
fn decode_match_params_from_bytes() {
    let data = match_params_bytes();
    let (rec, view) =
        decode_instance_set(&data, Some(Did(0x79F0_0001)), &tk::schema(), &tk::vocab()).unwrap();
    assert_eq!(rec.set.len(), 6);
    let Some(InstanceRecord::MatchParams(mp)) = view else {
        panic!("not match parameters: {view:?}");
    };
    assert_eq!(mp.level, (Some(60), Some(64)));
    assert!(mp.matches(60, 3, 2));
    assert!(mp.matches(64, 1, 1));
    assert!(!mp.matches(65, 1, 1));
    assert!(!mp.matches(60, 6, 1));

    let other = MatchParams {
        level: (Some(65), Some(65)),
        ..mp
    };
    assert!(!mp.level_overlaps(&other));
    let wide = MatchParams {
        level: (Some(60), Some(65)),
        tier: (Some(2), Some(2)),
        ..mp
    };
    assert!(mp.level_overlaps(&wide) && wide.level_overlaps(&other));
    assert!(mp.tier_overlaps(&wide) && mp.group_overlaps(&wide));

    // Hostile: every truncation fails cleanly; a wrong DID is rejected.
    for cut in 0..data.len() {
        assert!(decode_instance_set(&data[..cut], None, &tk::schema(), &tk::vocab()).is_err());
    }
    assert!(matches!(
        decode_instance_set(&data, Some(Did(0x79F0_0002)), &tk::schema(), &tk::vocab()),
        Err(FormatError::DidMismatch { .. })
    ));
}

#[test]
fn wrongly_typed_fields_read_as_absent() {
    // A schema that disagrees with the spec's types: every label is a string.
    let schema: std::collections::HashMap<u32, PropertyType> = l::ALL
        .iter()
        .map(|(label, _)| (tk::pid(label), PropertyType::String))
        .collect();
    let mut b = B::new().u32(0x79F0_0003).tsize(0, 3);
    for label in [l::TPL_CONTENT_LAYER, l::TPL_MAX_PLAYERS, l::TPL_LANDBLOCKS] {
        b = b.u32(tk::pid(label)).u32(tk::pid(label)).pascal("x");
    }
    let (_, view) = decode_instance_set(&b.build(), None, &schema, &tk::vocab()).unwrap();
    let Some(InstanceRecord::Template(t)) = view else {
        panic!("{view:?}");
    };
    assert_eq!((t.content_layer, t.max_players), (None, None));
    assert!(t.landblocks.is_empty() && t.has_landblock_list);
}

// ---------------------------------------------------------------- §5.1

#[test]
fn lock_reset_days() {
    let v = tk::vocab();
    let lock = |size: u32, days: u32| {
        raw(
            P_STRUCT,
            strukt(vec![
                entry(l::PE_LOCK_SIZE, PropertyValue::Enum(size)),
                entry(l::PE_LOCK_RESET_DAYS, PropertyValue::Bitfield32(days)),
            ]),
        )
    };
    let s = set(vec![
        entry(
            l::PE_LOCK_ARRAY,
            array(vec![lock(5, 0x7F), lock(3, 0b0100_0010)]),
        ),
        entry(l::PE_LOCKABLE, did_array(&[0x70F0_0010, 0x70F0_0011])),
    ]);
    let c = tessera_formats::instance::EncounterControl::read(&s, &v);
    assert_eq!(
        c.lock(5).unwrap().reset_day_keys(),
        vec![1, 2, 3, 4, 5, 6, 7]
    );
    // Bit k − 1 is day k: bits 1 and 6 are Monday (2) and Saturday (7).
    assert_eq!(c.lock(3).unwrap().reset_day_keys(), vec![2, 7]);
    assert!(!c.lock(3).unwrap().resets_on(0) && !c.lock(3).unwrap().resets_on(99));
    let with_size: BTreeSet<Did> = [Did(0x70F0_0010), Did(0x70F0_0012)].into();
    let devs = c.check_lockable(&with_size);
    assert_eq!(devs.len(), 2);
    assert!(devs.iter().all(|x| x.invariant == "N-I6"));
}

// ---------------------------------------------------------------- §17.2, §17.3 (B4)

fn derived_points_per_hour() -> SkirmishControl {
    let op = |stat: u32, mod_op: u32| {
        raw(
            P_STRUCT,
            strukt(vec![
                entry(l::STAT_SECONDARY, PropertyValue::Enum(stat)),
                entry(l::MOD_OP, PropertyValue::Enum(mod_op)),
            ]),
        )
    };
    let stat = strukt(vec![
        entry(l::STAT_DERIVED, PropertyValue::Enum(17)),
        entry(l::STAT_PRIMARY, PropertyValue::Enum(1)),
        entry(l::STAT_OPERATIONS, array(vec![op(2, 7), op(13, 2)])),
        entry(l::STAT_SCALE, PropertyValue::Int(3600)),
    ]);
    let s = set(vec![
        entry(l::SC_TRAIT_GATES, array(vec![])),
        entry(l::SC_STATS, array(vec![raw(P_STRUCT, stat)])),
        entry(l::SC_FASTEST, PropertyValue::Float(0.5)),
    ]);
    SkirmishControl::read(&s, &tk::vocab())
}

#[test]
fn derived_statistic_worked_example() {
    let sc = derived_points_per_hour();
    let stat = &sc.derived_stats[0];
    assert_eq!(stat.derived, Some(17));
    let values = |played: f32| {
        move |k: u32| match k {
            1 => StatValue::Int(120),
            2 => StatValue::Int(30),
            13 => StatValue::Float(played),
            _ => StatValue::Int(0),
        }
    };
    let StatValue::Float(r) = stat.evaluate(values(5400.0)) else {
        panic!("divide forces floating point");
    };
    assert!((r - 100.0).abs() < 1e-4, "{r}");
    assert_eq!(stat.evaluate(values(0.0)), StatValue::Float(0.0));

    // Integer arithmetic without a divide or float statistic; scale 1 leaves it alone.
    let mut int_stat = stat.clone();
    int_stat.operations = vec![(2, 7), (3, 6), (4, 8), (5, 99)];
    int_stat.scale = 1;
    let r = int_stat.evaluate(|k| StatValue::Int(i32::try_from(k).unwrap() * 10));
    // ((10 + 20) − 30) × 40 = 0; the unknown op leaves the value unchanged.
    assert_eq!(r, StatValue::Int(0));
    int_stat.operations.truncate(1);
    int_stat.scale = 2;
    assert_eq!(
        int_stat.evaluate(|k| StatValue::Int(i32::try_from(k).unwrap())),
        StatValue::Int(6)
    );
}

#[test]
fn derived_statistic_integer_arithmetic_wraps_17_7() {
    // §17.7: signed 32-bit two's complement, wrapping.
    let mut stat = derived_points_per_hour().derived_stats[0].clone();
    stat.scale = 1;
    stat.operations = vec![(2, 7)];
    let v = |_| StatValue::Int(2_000_000_000);
    assert_eq!(stat.evaluate(v), StatValue::Int(-294_967_296));
    stat.operations = vec![(2, 8)];
    let v = |_| StatValue::Int(70_000);
    assert_eq!(stat.evaluate(v), StatValue::Int(605_032_704));
}

#[test]
fn derived_statistic_reading_rules_b4() {
    let v = tk::vocab();
    let op = |sec: PropertyValue, mod_op: PropertyValue| {
        raw(
            P_STRUCT,
            strukt(vec![
                entry(l::STAT_SECONDARY, sec),
                entry(l::MOD_OP, mod_op),
            ]),
        )
    };
    let read = |scale: Option<PropertyValue>, ops: Vec<tessera_formats::PropertyEntry>| {
        let mut f = vec![
            entry(l::STAT_DERIVED, PropertyValue::PropertyId(18)),
            entry(l::STAT_PRIMARY, PropertyValue::PropertyId(1)),
            entry(l::STAT_OPERATIONS, array(ops)),
        ];
        if let Some(s) = scale {
            f.push(entry(l::STAT_SCALE, s));
        }
        SkirmishControl::read(
            &set(vec![
                entry(l::SC_TRAIT_GATES, array(vec![])),
                entry(l::SC_STATS, array(vec![raw(P_STRUCT, strukt(f))])),
            ]),
            &v,
        )
        .derived_stats
    };
    // Keys may be enum or PID values; reading stops at the first bad operation.
    let stats = read(
        None,
        vec![
            op(PropertyValue::Enum(2), PropertyValue::Enum(7)),
            op(PropertyValue::PropertyId(3), PropertyValue::PropertyId(6)),
            op(PropertyValue::Int(4), PropertyValue::Enum(7)),
            op(PropertyValue::Enum(5), PropertyValue::Enum(7)),
        ],
    );
    assert_eq!(stats.len(), 1);
    assert_eq!((stats[0].derived, stats[0].primary), (Some(18), Some(1)));
    assert_eq!(stats[0].operations, vec![(2, 7), (3, 6)]);
    assert_eq!(stats[0].scale, 1, "absent scale is 1");
    // An element that is not a struct also ends the list.
    let stats = read(
        Some(PropertyValue::Int(3600)),
        vec![
            op(PropertyValue::Enum(2), PropertyValue::Enum(7)),
            raw(P_INT, PropertyValue::Int(1)),
            op(PropertyValue::Enum(5), PropertyValue::Enum(7)),
        ],
    );
    assert_eq!((stats[0].operations.len(), stats[0].scale), (1, 3600));
    // A scale of another type: the entry is malformed and left out (Tessera Decision).
    assert!(read(Some(PropertyValue::Float(2.0)), vec![]).is_empty());
}

#[test]
fn best_time_worked_example() {
    let times = [0.0, 812.5, 790.0];
    assert_eq!(combine_best_time(&times, 0.5, false), Some(790.0));
    assert_eq!(combine_best_time(&times, 0.5, true), Some(812.5));
    assert_eq!(combine_best_time(&[0.0, 0.5], 0.5, false), None);
    assert_eq!(combine_best_time(&[], 0.5, true), None);
}

#[test]
fn conversion_hash_n_i12() {
    let entry_of = |stat: u32, size: u32, on: bool| {
        raw(
            P_STRUCT,
            strukt(vec![
                entry(
                    l::CV_PROPERTY,
                    PropertyValue::PropertyId(0x3000_0000 + stat * 100 + size),
                ),
                entry(l::CV_STAT, PropertyValue::Enum(stat)),
                entry(l::CV_SIZE, PropertyValue::Enum(size)),
                entry(l::CV_ON_LEVEL, PropertyValue::Bool(on)),
            ]),
        )
    };
    let sizes = [1, 2, 3, 6, 12, 24];
    let mut elems = Vec::new();
    for stat in 1..=19 {
        for size in sizes {
            elems.push(entry_of(stat, size, true));
            elems.push(entry_of(stat, size, false));
        }
    }
    let make = |elems: Vec<_>| {
        SkirmishControl::read(
            &set(vec![
                entry(l::SC_TRAIT_GATES, array(vec![])),
                entry(l::SC_CONVERSION, array(elems)),
            ]),
            &tk::vocab(),
        )
    };
    let sc = make(elems.clone());
    assert_eq!(sc.conversion.len(), 228);
    assert!(
        sc.check_conversion().is_empty(),
        "{:?}",
        sc.check_conversion()
    );
    assert_eq!(sc.stat_property(3, 6, false), Some(0x3000_0000 + 306));
    elems.pop();
    assert!(!make(elems).check_conversion().is_empty());
}

// ---------------------------------------------------------------- §7.1, §13.3, B1, B3

fn world_join() -> WorldJoinControl {
    let group = |label: u32, templates: &[u32]| {
        raw(
            P_STRUCT,
            strukt(vec![
                entry(l::WJC_GROUP_LABEL, PropertyValue::Enum(label)),
                entry(l::WJC_GROUP_LIST, did_array(templates)),
            ]),
        )
    };
    let s = set(vec![
        entry(l::WJC_AVAILABLE, did_array(&[0x70F0_0001, 0x70F0_0002])),
        entry(
            l::WJC_GROUPS,
            array(vec![
                group(3, &[0x70F0_0100, 0x70F0_0101]),
                group(1, &[0x70F0_0110]),
                group(2, &[0x70F0_0120]),
            ]),
        ),
        entry(l::WJC_FEATURED, did_array(&[0x70F0_0002, 0x70F0_0001])),
        entry(l::WJC_SEASONAL, enum_array(P_ENUM, &[9, 10])),
        entry(l::WJC_CAPPED, PropertyValue::Enum(4)),
    ]);
    WorldJoinControl::read(&s, &tk::vocab())
}

#[test]
fn finder_order_takes_groups_by_ascending_label() {
    let w = world_join();
    let order: Vec<u32> = w.finder_order().iter().map(|d| d.0).collect();
    assert_eq!(
        order,
        vec![
            0x70F0_0001,
            0x70F0_0002,
            0x70F0_0110,
            0x70F0_0120,
            0x70F0_0100,
            0x70F0_0101
        ]
    );
    assert_eq!(w.sort_index(Did(0x70F0_0120)), Some(3));
    assert_eq!(w.sort_index(Did(0x70F0_9999)), None);
    assert!(w.deviations().is_empty());

    // §13.3 Tessera Decision: a group without a label is left out of the grouped part;
    // groups with equal labels keep their stored order.
    let mut odd = w.clone();
    odd.groups
        .push(tessera_formats::instance::control::InstanceGroup {
            label: None,
            templates: vec![Did(0x70F0_0130)],
            has_list: true,
        });
    odd.groups
        .push(tessera_formats::instance::control::InstanceGroup {
            label: Some(1),
            templates: vec![Did(0x70F0_0140)],
            has_list: true,
        });
    let order: Vec<u32> = odd.finder_order().iter().map(|d| d.0).collect();
    assert_eq!(
        order,
        vec![
            0x70F0_0001,
            0x70F0_0002,
            0x70F0_0110,
            0x70F0_0140,
            0x70F0_0120,
            0x70F0_0100,
            0x70F0_0101
        ]
    );

    // N-I7: a featured template outside the available list, a grouped one inside it.
    let mut bad = w.clone();
    bad.featured.push(Some(Did(0x70F0_0100)));
    bad.available.push(Did(0x70F0_0110));
    assert_eq!(bad.deviations().len(), 2);
}

#[test]
fn featured_instance_b1() {
    let w = world_join();
    assert_eq!(w.featured_instance(None), None);
    assert_eq!(w.featured_instance(Some(&PropertyValue::Int(0))), None);
    assert_eq!(w.featured_instance(Some(&PropertyValue::Int(-3))), None);
    assert_eq!(w.featured_instance(Some(&PropertyValue::Float(1.0))), None);
    assert_eq!(
        w.featured_instance(Some(&PropertyValue::Int(1))),
        Some(Did(0x70F0_0002))
    );
    assert_eq!(
        w.featured_instance(Some(&PropertyValue::Int(2))),
        Some(Did(0x70F0_0001))
    );
    // Only a 32-bit integer value counts (type 15); a 64-bit integer is "not an integer".
    assert_eq!(w.featured_instance(Some(&PropertyValue::Int64(2))), None);
    assert_eq!(w.featured_instance(Some(&PropertyValue::Int(3))), None);
    let mut odd = w.clone();
    odd.featured = vec![None];
    assert_eq!(odd.featured_instance(Some(&PropertyValue::Int(1))), None);
}

#[test]
fn instance_group_worked_example() {
    let w = world_join();
    let sizes = |t: Did| match t.0 {
        0x70F0_0100 => vec![6, 12],
        0x70F0_0101 => vec![12, 24],
        _ => vec![],
    };
    assert_eq!(w.group_template(3, 12, sizes), Some(Did(0x70F0_0100)));
    assert_eq!(w.group_template(3, 24, sizes), Some(Did(0x70F0_0101)));
    assert_eq!(w.group_template(3, 3, sizes), None);
    assert_eq!(w.group_template(8, 12, sizes), None);

    // B3: the label → group table ignores a group without a label or without a list and
    // keeps the first group of a repeated label.
    let v = tk::vocab();
    let group = |label: Option<PropertyValue>, list: Option<PropertyValue>| {
        let mut f = Vec::new();
        if let Some(lv) = label {
            f.push(entry(l::WJC_GROUP_LABEL, lv));
        }
        if let Some(lv) = list {
            f.push(entry(l::WJC_GROUP_LIST, lv));
        }
        raw(P_STRUCT, strukt(f))
    };
    let s = set(vec![entry(
        l::WJC_GROUPS,
        array(vec![
            group(None, Some(did_array(&[0x70F0_0100]))),
            group(Some(PropertyValue::Enum(5)), Some(PropertyValue::Int(1))),
            group(
                Some(PropertyValue::PropertyId(5)),
                Some(did_array(&[0x70F0_0101])),
            ),
            group(
                Some(PropertyValue::Enum(5)),
                Some(did_array(&[0x70F0_0100])),
            ),
            group(
                Some(PropertyValue::Float(6.0)),
                Some(did_array(&[0x70F0_0100])),
            ),
        ]),
    )]);
    let w = WorldJoinControl::read(&s, &v);
    let table = w.group_table();
    assert_eq!(table.len(), 1);
    assert_eq!(table[&5].templates, vec![Did(0x70F0_0101)]);
    assert_eq!(w.group_template(5, 24, sizes), Some(Did(0x70F0_0101)));
    assert_eq!(w.group_template(0, 12, sizes), None, "no default label 0");
}

#[test]
fn level_scaling_level_b2() {
    let v = tk::vocab();
    let with = |n: Option<i32>| {
        let mut e = vec![entry(l::TPL_CONTENT_LAYER, PropertyValue::Enum(1))];
        if let Some(n) = n {
            e.push(entry(l::SK_LEVEL_SCALING, PropertyValue::Int(n)));
        }
        Template::read(&set(e), &v)
    };
    let over = PropertyValue::Int(140);
    assert_eq!(with(None).effective_level_scaling_level(Some(&over)), 0);
    assert_eq!(
        with(Some(-2)).effective_level_scaling_level(Some(&over)),
        -2
    );
    assert_eq!(with(Some(100)).effective_level_scaling_level(None), 100);
    assert_eq!(
        with(Some(100)).effective_level_scaling_level(Some(&over)),
        140
    );
    let not_int = PropertyValue::Float(5.0);
    assert_eq!(
        with(Some(100)).effective_level_scaling_level(Some(&not_int)),
        100
    );
    // A 64-bit integer override does not count (type 15 only).
    let wide = PropertyValue::Int64(140);
    assert_eq!(
        with(Some(100)).effective_level_scaling_level(Some(&wide)),
        100
    );
}

// ---------------------------------------------------------------- §17.5 (§10.2)

#[test]
fn fellowship_maker_entries_worked_example() {
    let t = MatchingTemplate {
        did: Did(0x70F0_0200),
        encounter_type: Some(4),
        category: Some(20),
        big_battle: false,
        group_sizes: vec![1, 2, 6],
        tiers: vec![1, 2],
    };
    let rules = MatchingRules {
        bits: tk::flag_bits(),
        valid_sizes: vec![2, 3, 6],
        capped: Some(4),
        seasonal: [9, 10].into(),
    };
    let lead = MatchingTemplate {
        did: Did(0x70F0_0199),
        encounter_type: Some(3),
        group_sizes: vec![3],
        tiers: vec![1],
        ..MatchingTemplate::default()
    };
    let idx = MatchingIndex::build(&[lead, t], &rules);
    let flags: Vec<u32> = idx.entries.iter().skip(1).map(|e| e.flags).collect();
    assert_eq!(flags, vec![0x124, 0x224, 0x184, 0x284]);
    assert_eq!(idx.entries[0].flags, 0x1 | 0x40 | 0x100);
    let i = 1;
    let map = &idx.encounters[&Did(0x70F0_0200)];
    assert_eq!(map[&2][&1], i);
    assert_eq!(map[&2][&2], i + 1);
    assert_eq!(map[&6][&1], i + 2);
    assert_eq!(map[&6][&2], i + 3);
    assert!(!map.contains_key(&1));
    // Item 3: one key per non-zero mapper key (20), 0x4000 absent, unused keys empty.
    assert_eq!(idx.by_flag.len(), 20);
    assert!(!idx.by_flag.contains_key(&0x4000));
    assert!(idx.by_flag[&0x8].is_empty());
    let classic = rules.bits.kind_bit(MatchKind::Classic).unwrap();
    assert_eq!(classic, 0x4);
    assert_eq!(idx.by_flag[&classic], vec![1, 2, 3, 4]);
    // B8: the panel lists entries with a kind bit; a group row ors its entries' words.
    assert_eq!(rules.bits.kind_mask(), 0x18_000F);
    assert!(idx.entries.iter().all(|e| rules.bits.is_listed(e.flags)));
    assert!(!rules.bits.is_listed(0x20 | 0x100));
    assert_eq!(
        idx.group_flags(&[Did(0x70F0_0199), Did(0x70F0_0200)]),
        0x1 | 0x4 | 0x20 | 0x40 | 0x80 | 0x100 | 0x200
    );
    assert_eq!(idx.by_flag[&0x20], vec![1, 2]);
    for (bit, marked) in &idx.by_flag {
        for (n, e) in idx.entries.iter().enumerate() {
            assert_eq!(marked.contains(&n), e.flags & bit != 0);
        }
    }
}

#[test]
fn fellowship_maker_kind_conflicts_and_unencodable() {
    let rules = MatchingRules {
        bits: tk::flag_bits(),
        valid_sizes: vec![2, 3, 6, 12],
        capped: Some(4),
        seasonal: [4].into(),
    };
    let both = MatchingTemplate {
        did: Did(0x70F0_0300),
        category: Some(4),
        group_sizes: vec![2],
        tiers: vec![1],
        ..MatchingTemplate::default()
    };
    let wide = MatchingTemplate {
        did: Did(0x70F0_0301),
        group_sizes: vec![12, 2],
        tiers: vec![11, 6, 1],
        ..MatchingTemplate::default()
    };
    let idx = MatchingIndex::build(&[both, wide.clone()], &rules);
    assert_eq!(idx.kind_conflicts, vec![Did(0x70F0_0300)]);
    // Size 12 has no Size_ label, tier 11 no Tier_ label; tier 6 now has a bit (0x2000).
    assert_eq!(
        idx.unencodable,
        vec![(Did(0x70F0_0301), 12), (Did(0x70F0_0301), 11)]
    );
    let flags: Vec<u32> = idx.entries.iter().map(|e| e.flags).collect();
    assert_eq!(flags, vec![0x4 | 0x20 | 0x2000, 0x4 | 0x20 | 0x100]);
    // A kind without a labelled bit gives no entries.
    let partial = MatchingRules {
        bits: FlagBits::from_keys(
            tk::configuration_keys()
                .into_iter()
                .filter(|(l, _)| *l != "Type_ScalingInstance"),
        ),
        ..rules
    };
    let idx = MatchingIndex::build(&[wide], &partial);
    assert_eq!(idx.unlabelled_kinds, vec![Did(0x70F0_0301)]);
    assert!(idx.entries.is_empty());
}

// ---------------------------------------------------------------- §8.4, §17.6 (B5), N-I10

fn unit() -> DistantBattleUnit {
    let offsets = |n: usize, lateral: f32| {
        array(
            (0..n)
                .map(|i| {
                    raw(
                        P_VECTOR,
                        PropertyValue::Vector3([lateral, 0.0, i as f32 * 0.5]),
                    )
                })
                .collect(),
        )
    };
    let group = |u: u32, n: usize, lateral: f32| {
        raw(
            P_STRUCT,
            strukt(vec![
                entry(l::DBU_UNIT, d(u)),
                entry(l::DBU_COSMETIC, d(0x47F0_0001)),
                entry(l::DBU_OFFSETS, offsets(n, lateral)),
                entry(l::DBU_JITTER, PropertyValue::Float(1.5)),
            ]),
        )
    };
    let formation = |u: u32| {
        raw(
            P_STRUCT,
            strukt(vec![
                entry(l::DBU_UNIT, d(u)),
                entry(l::DBU_JITTER, PropertyValue::Float(1.5)),
            ]),
        )
    };
    let mut composition = vec![0x70F0_0301; 10];
    composition.extend(vec![0x70F0_0300; 40]);
    let s = set(vec![
        entry(l::DBU_RESPAWN, PropertyValue::Float(5.0)),
        entry(
            l::DBU_ENTITY_COMPOSITION,
            array(vec![
                group(0x70F0_0300, 40, -4.0),
                group(0x70F0_0301, 10, 4.0),
            ]),
        ),
        entry(
            l::DBU_FORMATION,
            array(vec![formation(0x70F0_0301), formation(0x70F0_0300)]),
        ),
        entry(l::DBU_COMPOSITION, did_array(&composition)),
        entry(l::DBU_DIMENSIONS, PropertyValue::Vector3([-5.0, 5.0, 20.0])),
        entry(l::DBU_WIDTH, PropertyValue::Enum(2)),
        entry(l::DBU_GROUND_CLAMP, PropertyValue::Bool(false)),
    ]);
    DistantBattleUnit::read(&s, &tk::vocab())
}

#[test]
fn distant_battle_unit_display_and_invariants() {
    let u = unit();
    assert_eq!(u.figure_count(), 50);
    assert!(!u.ground_clamp());
    assert_eq!(u.capture_quality(), 0xFFFF_FFFF);
    assert_eq!(u.lane_type(), 2);
    assert!(
        u.deviations(Some(10.0)).is_empty(),
        "{:?}",
        u.deviations(Some(10.0))
    );
    assert_eq!(u.deviations(Some(9.0)).len(), 1);
    let b = u.bounding_box([100.0, 200.0, 30.0]);
    assert_eq!(b.min, [95.0, 200.0, 30.0]);
    assert_eq!(b.max, [105.0, 220.0, 31.0]);

    let mut broken = u.clone();
    broken.composition.pop();
    broken.groups[0].offsets[0] = [-6.0, 0.0, 0.0];
    broken.formation[0].jitter = Some(2.0);
    assert_eq!(broken.deviations(None).len(), 3);

    let mut no_dims = u.clone();
    no_dims.dimensions = None;
    let p = no_dims.bounding_box([1.0, 2.0, 3.0]);
    assert_eq!((p.min, p.max), ([1.0, 2.0, 3.0], [1.0, 2.0, 3.0]));
    assert!(DistantBattleUnit::default().ground_clamp());
}

#[test]
fn removal_worked_example() {
    // §17.6.
    let mut u = unit();
    let mut counts = FigureCounts::new(&u);
    let first = Removal {
        unit: Some(Did(0x70F0_0300)),
        count: 1,
        script_key: 9,
        script_parameters: None,
    };
    let applied = counts.apply(std::slice::from_ref(&first));
    assert_eq!(applied.len(), 1);
    // The figure at offset 40 (index 39, the last stored) goes, and script 9 plays.
    assert_eq!(applied[0].removed[0].offsets, vec![39]);
    assert_eq!(applied[0].effect, RemovalEffect::PlayScript(9));
    let second = Removal {
        unit: Some(Did(0x70F0_0301)),
        count: 4,
        script_key: 9,
        script_parameters: None,
    };
    let applied = counts.apply(&[first.clone(), second.clone()]);
    assert_eq!(applied.len(), 1, "the first element is not applied again");
    assert_eq!(applied[0].element, 1);
    assert_eq!(applied[0].removed[0].group, 1);
    assert_eq!(applied[0].removed[0].offsets, vec![9, 8, 7, 6]);
    assert_eq!(
        (counts.remaining(0), counts.remaining(1)),
        (Some(39), Some(6))
    );
    // The list shrinks to its first element: nothing happens.
    assert!(counts.apply(std::slice::from_ref(&first)).is_empty());
    // It grows again: position 1 is applied again; no script key drops the 6 at once.
    let third = Removal {
        unit: Some(Did(0x70F0_0301)),
        count: 20,
        script_key: 0,
        script_parameters: None,
    };
    let applied = counts.apply(&[first.clone(), third]);
    assert_eq!(applied.len(), 1);
    assert_eq!(applied[0].figure_count(), 6);
    assert_eq!(applied[0].effect, RemovalEffect::DropAtOnce);
    counts.write_back(&mut u);
    assert_eq!(
        (u.groups[0].offsets.len(), u.groups[1].offsets.len()),
        (39, 0)
    );
    // Nothing left to remove: no script plays.
    let again = Removal {
        unit: Some(Did(0x70F0_0301)),
        count: 1,
        script_key: 9,
        script_parameters: None,
    };
    let applied = counts.apply(&[first.clone(), first.clone(), again]);
    assert_eq!(applied[0].effect, RemovalEffect::Silent);
    // An unknown unit removes nothing and plays nothing; a missing unit is skipped.
    let stray = Removal {
        unit: Some(Did(0x70F0_0999)),
        ..first.clone()
    };
    let none = Removal {
        unit: None,
        ..first.clone()
    };
    let n = counts.apply(&[first.clone(), first.clone(), first.clone(), stray, none]);
    assert!(n.is_empty());
}

#[test]
fn removal_first_display_is_silent_and_matches_every_group() {
    let mut u = unit();
    // Two groups with the same unit: both lose figures.
    u.groups[1].unit = Some(Did(0x70F0_0300));
    let r = Removal {
        unit: Some(Did(0x70F0_0300)),
        count: 3,
        script_key: 9,
        script_parameters: None,
    };
    let (counts, applied) = FigureCounts::first_display(&u, std::slice::from_ref(&r));
    assert_eq!(applied.len(), 1);
    assert_eq!(applied[0].removed.len(), 2);
    assert_eq!(applied[0].effect, RemovalEffect::Silent);
    assert_eq!(
        (counts.remaining(0), counts.remaining(1)),
        (Some(37), Some(7))
    );
    // The elements already shown are not applied again on the next change.
    let mut counts = counts;
    assert!(counts.apply(std::slice::from_ref(&r)).is_empty());
}

#[test]
fn removal_element_value_types() {
    let v = tk::vocab();
    let element = |fields: Vec<tessera_formats::PropertyEntry>| raw(P_STRUCT, strukt(fields));
    let s = set(vec![
        entry(l::DBU_RESPAWN, PropertyValue::Float(5.0)),
        entry(
            l::DBU_REMOVED,
            array(vec![
                element(vec![entry(l::DBU_UNIT, d(0x70F0_0300))]),
                element(vec![
                    entry(l::DBU_UNIT, d(0x70F0_0300)),
                    entry(l::DBU_REMOVAL_COUNT, PropertyValue::Float(4.0)),
                ]),
                element(vec![
                    entry(l::DBU_UNIT, d(0x70F0_0300)),
                    entry(l::DBU_REMOVAL_COUNT, PropertyValue::Int(-1)),
                    entry(l::DBU_REMOVAL_SCRIPT, PropertyValue::Enum(12)),
                    entry(l::DBU_REMOVAL_PARAMS, strukt(vec![])),
                ]),
                element(vec![entry(l::DBU_UNIT, d(0))]),
                raw(P_INT, PropertyValue::Int(3)),
            ]),
        ),
    ]);
    let u = DistantBattleUnit::read(&s, &v);
    assert_eq!(u.removals.len(), 5, "every element keeps its position");
    assert_eq!(u.removals[0].count, 1, "absent count is 1");
    assert_eq!(u.removals[1].count, 1, "a count of another type is 1");
    assert_eq!(
        u.removals[2].count,
        u32::MAX,
        "the count is read as unsigned"
    );
    assert_eq!(u.removals[2].script_key, 12);
    assert!(u.removals[2].script_parameters.is_some());
    assert_eq!(u.removals[0].script_key, 0, "absent script key is 0");
    assert_eq!(u.removals[3].unit, None, "unit 0 is no unit");
    assert_eq!(u.removals[4], Removal::default());
    // A negative count removes all figures of the group.
    let mut counts = FigureCounts::new(&unit());
    let applied = counts.apply(&u.removals);
    assert_eq!(applied.len(), 3);
    assert_eq!(counts.remaining(0), Some(0));
    // A removed list that is not an array reads as empty.
    let s = set(vec![
        entry(l::DBU_RESPAWN, PropertyValue::Float(5.0)),
        entry(l::DBU_REMOVED, PropertyValue::Int(1)),
    ]);
    assert!(DistantBattleUnit::read(&s, &v).removals.is_empty());
    // Ground clamping needs a boolean value; another type counts as true.
    let s = set(vec![
        entry(l::DBU_RESPAWN, PropertyValue::Float(5.0)),
        entry(l::DBU_GROUND_CLAMP, PropertyValue::Int(0)),
    ]);
    assert!(DistantBattleUnit::read(&s, &v).ground_clamp());
}

// ---------------------------------------------------------------- remaining views

#[test]
fn small_views_read_their_fields() {
    let v = tk::vocab();
    let fm = set(vec![
        entry(l::FM_SIZES, enum_array(P_ENUM, &[2, 3, 6])),
        entry(
            l::FM_MATCH_TIME,
            array(vec![raw(
                P_STRUCT,
                strukt(vec![
                    entry(l::FM_ENCOUNTER_SIZE, PropertyValue::Enum(1)),
                    entry(
                        l::FM_MATCH_TIME_PROPERTY,
                        PropertyValue::PropertyId(0x3000_0001),
                    ),
                ]),
            )]),
        ),
        entry(
            l::FM_BONUS,
            array(vec![raw(
                P_STRUCT,
                strukt(vec![
                    entry(l::FM_BONUS_EFFECT, d(0x70F0_0500)),
                    entry(l::FM_BONUS_THRESHOLD, PropertyValue::Float(0.25)),
                ]),
            )]),
        ),
    ]);
    let Some(InstanceRecord::FellowshipMakerControl(f)) = InstanceRecord::read(&fm, &v) else {
        panic!();
    };
    assert_eq!(f.valid_sizes, vec![2, 3, 6]);
    assert_eq!(f.match_time, vec![(Some(1), Some(0x3000_0001))]);
    assert_eq!(
        f.bonus_thresholds,
        vec![(Some(Did(0x70F0_0500)), Some(0.25))]
    );

    let fc = set(vec![entry(
        l::FC_WIDTHS,
        array(vec![raw(
            P_STRUCT,
            strukt(vec![
                entry(l::DBU_WIDTH, PropertyValue::Enum(2)),
                entry(l::FC_ACTUAL_WIDTH, PropertyValue::Float(10.0)),
            ]),
        )]),
    )]);
    let Some(InstanceRecord::FormationControl(c)) = InstanceRecord::read(&fc, &v) else {
        panic!();
    };
    assert_eq!(c.actual_width(2), Some(10.0));
    assert_eq!(c.actual_width(3), None);

    let bb = set(vec![
        entry(
            l::BB_MEDALS,
            array(vec![raw(
                P_STRUCT,
                strukt(vec![
                    entry(l::BB_MEDAL, PropertyValue::Enum(5)),
                    entry(l::BB_TRAIT_POINTS, PropertyValue::Int(4)),
                ]),
            )]),
        ),
        entry(
            l::BB_QUEST_ENCOUNTERS,
            array(vec![raw(
                P_STRUCT,
                strukt(vec![
                    entry(l::BB_QUEST, d(0x70F0_0600)),
                    entry(l::BB_ENCOUNTER, d(0x70F0_0601)),
                ]),
            )]),
        ),
    ]);
    let Some(InstanceRecord::BigBattleControl(b)) = InstanceRecord::read(&bb, &v) else {
        panic!();
    };
    assert_eq!(b.medal(5).and_then(|m| m.trait_points), Some(4));
    assert_eq!(
        b.quest_encounters,
        vec![(Some(Did(0x70F0_0600)), Some(Did(0x70F0_0601)))]
    );

    let ph = set(vec![
        entry(l::PB_UPDATE_ONLY, PropertyValue::Bool(true)),
        entry(l::PB_DELAY, PropertyValue::Bool(true)),
    ]);
    let Some(InstanceRecord::PhasingBox(p)) = InstanceRecord::read(&ph, &v) else {
        panic!();
    };
    assert!(!p.registers_for_updates() && !p.refreshes_immediately());
    assert!(!p.force_refresh_on_enter);
    // B6: only a boolean value counts; an integer 1 is false.
    let ph = set(vec![
        entry(l::PB_UPDATE_ONLY, PropertyValue::Bool(false)),
        entry(l::PB_DELAY, PropertyValue::Int(1)),
        entry(l::PH_FORCE_REFRESH, PropertyValue::Int(1)),
    ]);
    let Some(InstanceRecord::PhasingBox(p)) = InstanceRecord::read(&ph, &v) else {
        panic!();
    };
    assert!(p.refreshes_immediately() && !p.force_refresh_on_enter);

    // §6.3: the group-tier map, read by its two labels (both enums of one mapper).
    let sc = set(vec![
        entry(l::SC_TRAIT_GATES, array(vec![])),
        entry(
            l::SC_GROUP_TIERS,
            array(vec![raw(
                P_STRUCT,
                strukt(vec![
                    entry(l::SC_GT_TIER, PropertyValue::Enum(3)),
                    entry(l::SC_GT_SIZE, PropertyValue::Enum(2)),
                ]),
            )]),
        ),
        entry(
            l::SC_DIFFICULTY_MULT,
            array(vec![raw(
                P_STRUCT,
                strukt(vec![
                    entry(l::SC_POINT_MULT, PropertyValue::Float(1.25)),
                    entry(l::SC_DIFF_TIER, PropertyValue::Enum(3)),
                ]),
            )]),
        ),
    ]);
    let s = SkirmishControl::read(&sc, &v);
    assert_eq!(s.group_tier(2), Some(3));
    assert_eq!(s.group_tier(3), None);
    assert_eq!(s.difficulty_multiplier(3), Some(1.25));
}

#[test]
fn testkit_schema_covers_the_labels() {
    let schema = tk::schema();
    let v: InstanceVocab = tk::vocab();
    assert!(v.missing().is_empty());
    assert!(v.type_mismatches(&schema).is_empty());
    let _ = (P_INT, P_BOOL);
    // The synthetic configuration keys give the bits of §10.2 item 2.
    let b = tk::flag_bits();
    assert_eq!(b.keys.len(), 20);
    assert_eq!(b.sizes.get(&6), Some(&0x80));
    assert_eq!(b.tiers.get(&7), Some(&0x8000));
    assert_eq!(b.tiers.get(&10), Some(&0x4_0000));
    assert_eq!(
        (b.big_battles, b.seasonal),
        (Some(0x8_0000), Some(0x10_0000))
    );
}

#[test]
fn mutated_records_never_panic() {
    let v = tk::vocab();
    let schema = tk::schema();
    let base = match_params_bytes();
    for i in 0..base.len() {
        for x in [0u8, 1, 0x7F, 0x80, 0xFF] {
            let mut m = base.clone();
            m[i] = x;
            if let Ok((rec, _)) = decode_instance_set(&m, None, &schema, &v) {
                let s = &rec.set;
                let _ = Template::read(s, &v).deviations();
                let _ = WorldJoinControl::read(s, &v).finder_order();
                let _ = DistantBattleUnit::read(s, &v).deviations(Some(1.0));
                let _ = SkirmishControl::read(s, &v).check_conversion();
                let _ = MatchParams::read(s, &v).matches(1, 1, 1);
            }
        }
    }
}
