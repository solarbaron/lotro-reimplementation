// SPDX-License-Identifier: GPL-3.0-or-later
//! Synthetic tests of the item views and rules (`tessera_formats::gameplay::items`), built
//! from the worked examples of `docs/specs/items.md` §13 and its invariants. Every PID, DID,
//! class index and number here is invented.

mod common;

use std::collections::HashMap;

use common::B;
use tessera_formats::gameplay::common::build::{array, did, entry, set, structure};
use tessera_formats::gameplay::common::{ClassIndex, ClassTree, Pid};
use tessera_formats::gameplay::items::{
    Binding, BindingFlags, COPY_LABELS, DERIVED_LABELS, DpsTable, DurationMap, ENTITY_STAMP_LABELS,
    EffectListKind, ItemClassKind, ItemClasses, ItemKind, ItemPropertyCategory, ItemSchema,
    ItemSetView, ItemView, ItemWeenieKinds, ModEntryKind, Progression, RelicView, SetMembership,
    ValueTable, WeaponTimingTable, WeaponTimings, WearControl, armour_value, damage_range,
    entity_labels, equipment_category_keys, item_references, item_value, legacy_effects,
    mod_entry_is_well_formed, property_category, round_half_up, runic_list, set_directory,
    truncate, weapon_damage, weapon_timing_table_dids,
};
use tessera_formats::{Did, EnumMapper, FormatError, PropertyEntry, PropertyValue, Record};

/// A synthetic dictionary: every label the module uses gets its own PID (one PID per label,
/// shared between tables that use the same label).
fn names() -> HashMap<String, u32> {
    let mut map = HashMap::new();
    let mut next = 0x1000_0001u32;
    for label in ItemSchema::labels() {
        map.entry(label.to_owned()).or_insert_with(|| {
            next += 1;
            next
        });
    }
    map
}

fn schema() -> ItemSchema {
    ItemSchema::resolve(&names())
}

fn id(p: Pid) -> u32 {
    p.id().expect("synthetic label resolved")
}

fn e(p: Pid, v: PropertyValue) -> PropertyEntry {
    entry(id(p), v)
}

#[test]
fn schema_resolves_every_label() {
    let s = schema();
    assert!(s.missing().is_empty());
    let empty = ItemSchema::resolve(&HashMap::<String, u32>::new());
    assert!(!empty.missing().is_empty());
}

// §13.1
#[test]
fn classification_example() {
    let tree = ClassTree::from_edges([(20, 10), (25, 20), (30, 25), (31, 25), (40, 30), (50, 20)]);
    let refs = std::iter::repeat_n(40, 900)
        .chain(std::iter::repeat_n(31, 95))
        .chain(std::iter::repeat_n(50, 5));
    let c = ItemClasses::discover(&tree, refs).expect("classes found");
    assert_eq!(c.item_parent, 20);
    assert_eq!(c.item_base, 25);
    assert_eq!(c.classify(&tree, 30), ItemClassKind::Item);
    assert_eq!(c.classify(&tree, 25), ItemClassKind::Item);
    assert_eq!(c.classify(&tree, 50), ItemClassKind::ItemLike);
    assert_eq!(c.classify(&tree, 10), ItemClassKind::Other);
    assert!(c.is_item(&tree, 40) && !c.is_item(&tree, 50));
    assert!(c.is_item_like(&tree, 50) && !c.is_item_like(&tree, 10));
    assert_eq!(ItemClasses::from_item_base(&tree, 25), Some(c));
    assert_eq!(ItemClasses::from_item_base(&tree, 10), None);

    // Through a class index: unindexed DIDs are ignored.
    let mut index = ClassIndex::new();
    index.insert(Did(0x7000_0001), 40);
    index.insert(Did(0x7000_0002), 31);
    index.insert(Did(0x7000_0003), 50);
    let refs = [
        Did(0x7000_0001),
        Did(0x7000_0001),
        Did(0x7000_0002),
        Did(0x7000_0003),
        Did(0x7000_0099),
    ];
    assert_eq!(
        ItemClasses::discover_from_index(&tree, &index, refs),
        Some(c)
    );
}

#[test]
fn classification_edge_cases() {
    let tree = ClassTree::from_edges([(2, 1), (3, 1), (4, 1)]);
    // No references.
    assert_eq!(ItemClasses::discover(&tree, []), None);
    // All references on P itself: no child.
    assert_eq!(ItemClasses::discover(&tree, [1, 1]), None);
    // Disjoint roots.
    assert_eq!(ItemClasses::discover(&tree, [2, 9]), None);
    // Tie: lower class index wins.
    let c = ItemClasses::discover(&tree, [3, 4]).unwrap();
    assert_eq!((c.item_parent, c.item_base), (1, 3));
    // A cyclic map ends.
    let cyclic = ClassTree::from_edges([(1, 2), (2, 1)]);
    let _ = ItemClasses::discover(&cyclic, [1, 2]);
    let c = ItemClasses {
        item_base: 7,
        item_parent: 8,
    };
    assert_eq!(c.classify(&cyclic, 1), ItemClassKind::Other);
}

#[test]
fn reference_sources_are_found_at_any_depth() {
    let s = schema();
    let r = &s.references;
    let other = 0x2000_0000;
    let props = set(vec![
        entry(
            other,
            array(vec![
                entry(id(r.vendor_item), did(0x7000_0010)),
                entry(id(r.vendor_item), did(0)),
            ]),
        ),
        entry(
            other + 1,
            array(vec![entry(
                other + 2,
                structure(vec![
                    entry(id(r.barter_item), did(0x7000_0011)),
                    entry(other + 3, did(0x7000_0099)),
                ]),
            )]),
        ),
        entry(id(r.treasure_item), did(0x7000_0012)),
    ]);
    assert_eq!(
        item_references(&props, r),
        vec![Did(0x7000_0010), Did(0x7000_0011), Did(0x7000_0012)]
    );
}

// §13.2
fn value_table_set(s: &ItemSchema) -> tessera_formats::PropertySet {
    let t = &s.tables;
    let base = |v: f32| {
        entry(
            0x3000_0001,
            structure(vec![e(t.base_value, PropertyValue::Float(v))]),
        )
    };
    let quality = |k: u32, m: f32| {
        entry(
            0x3000_0002,
            structure(vec![
                e(t.quality, PropertyValue::Enum(k)),
                e(t.quality_mod, PropertyValue::Float(m)),
            ]),
        )
    };
    set(vec![
        e(
            t.base_value_array,
            array(vec![base(2.0), base(4.0), base(6.5)]),
        ),
        e(
            t.quality_array,
            array(vec![quality(5, 1.0), quality(4, 1.25)]),
        ),
    ])
}

#[test]
fn value_example() {
    let s = schema();
    let table = ValueTable::from_set(&value_table_set(&s), &s.tables).unwrap();
    assert_eq!(table.base_values, vec![2.0, 4.0, 6.5]);
    assert_eq!(table.value(3, 5), Some(7));
    assert_eq!(table.value(3, 4), Some(8));
    assert_eq!(table.value(2, 4), Some(5));
    assert_eq!(item_value(None, 3, 5), Some(0));
    assert_eq!(item_value(Some(&table), 1, 5), Some(2));
    // Outside the array, or a quality without a factor.
    assert_eq!(table.value(0, 5), None);
    assert_eq!(table.value(4, 5), None);
    assert_eq!(table.value(-1, 5), None);
    assert_eq!(table.value(1, 3), None);
}

#[test]
fn value_table_errors() {
    let s = schema();
    let t = &s.tables;
    // Missing quality array.
    let only_base = set(vec![e(t.base_value_array, array(vec![]))]);
    assert!(matches!(
        ValueTable::from_set(&only_base, t),
        Err(FormatError::MissingProperty { .. })
    ));
    // Base element that is not a struct.
    let bad = set(vec![
        e(
            t.base_value_array,
            array(vec![entry(1, PropertyValue::Float(1.0))]),
        ),
        e(t.quality_array, array(vec![])),
    ]);
    assert!(matches!(
        ValueTable::from_set(&bad, t),
        Err(FormatError::Malformed { .. })
    ));
    // Wrongly typed array.
    let wrong = set(vec![e(t.base_value_array, PropertyValue::Int(3))]);
    assert!(matches!(
        ValueTable::from_set(&wrong, t),
        Err(FormatError::PropertyTypeMismatch { .. })
    ));
}

#[test]
fn rounding_rules() {
    assert_eq!(round_half_up(6.5), Some(7));
    assert_eq!(round_half_up(687.5), Some(688));
    assert_eq!(round_half_up(8.125), Some(8));
    assert_eq!(round_half_up(0.499_999_97), Some(0));
    assert_eq!(round_half_up(-6.5), Some(-6));
    assert_eq!(round_half_up(f32::NAN), None);
    assert_eq!(round_half_up(f32::INFINITY), None);
    assert_eq!(round_half_up(f32::MAX), None);
    assert_eq!(truncate(2.75), Some(2));
    assert_eq!(truncate(-2.75), Some(-2));
    assert_eq!(truncate(f32::NEG_INFINITY), None);
}

// §13.3
#[test]
fn dps_and_damage_example() {
    let s = schema();
    let t = &s.tables;
    let mut base = vec![entry(0x3000_0003, PropertyValue::Float(1.0)); 9];
    base.push(entry(0x3000_0003, PropertyValue::Float(20.0)));
    let table_set = set(vec![
        e(t.dps_array, array(base)),
        e(
            t.dps_quality_array,
            array(vec![entry(
                0x3000_0004,
                structure(vec![
                    e(t.dps_quality, PropertyValue::Enum(2)),
                    e(t.dps_mod, PropertyValue::Float(1.04)),
                ]),
            )]),
        ),
    ]);
    let table = DpsTable::from_set(&table_set, t).unwrap();

    let p = &s.item;
    let weapon = set(vec![
        e(p.level, PropertyValue::Int(12)),
        e(p.dps_level, PropertyValue::Int(10)),
        e(p.quality, PropertyValue::Enum(2)),
        e(p.dps_table, did(0x7000_1000)),
        e(p.base_dps, PropertyValue::Float(20.8)),
        e(p.damage, PropertyValue::Float(23.771_43)),
        e(p.damage_variance, PropertyValue::Float(0.25)),
        e(p.damage_type, PropertyValue::Enum(1)),
        e(p.weapon_speed, PropertyValue::Enum(1)),
        e(p.base_action_duration, PropertyValue::Float(1.0)),
    ]);
    let view = ItemView::new(&weapon, &s);
    assert_eq!(view.signature_kind(), ItemKind::Weapon);
    let w = view.weapon().unwrap();
    assert_eq!(w.dps_level, Some(10));
    let dps = table
        .dps(w.dps_level.unwrap(), view.quality().unwrap())
        .unwrap();
    assert_eq!(dps.to_bits(), (20.0f32 * 1.04f32).to_bits());
    assert_eq!(dps, 20.8);
    // Using the item level instead finds no element here.
    assert_eq!(table.dps(12, 2), None);

    let damage = weapon_damage(dps, 1.0, 0.25).unwrap();
    assert!((damage - 23.771_43).abs() / 23.771_43 < 3e-7);
    assert!((damage - w.damage.unwrap()).abs() / damage < 3e-7);
    let (min, max) = damage_range(damage, 0.25);
    assert!((min - 17.828_571).abs() < 1e-5);
    assert_eq!(max, damage);
    assert!(((min + max) / 2.0 - 20.8).abs() < 1e-5);
    assert_eq!(weapon_damage(1.0, 1.0, 2.0), None);

    // Without a DPS level, the item level drives the lookup.
    let plain = set(vec![
        e(p.level, PropertyValue::Int(10)),
        e(p.dps_table, did(0)),
    ]);
    assert_eq!(
        ItemView::new(&plain, &s).weapon().unwrap().dps_level,
        Some(10)
    );
    assert!(ItemView::new(&set(vec![]), &s).weapon().is_none());
}

// §13.4: armour progressions are read and evaluated by the `combat` reader
// (`combat-data.md` §4.4, §4.10).
fn linear(points: &[(i32, f32)]) -> Progression {
    let s = schema();
    let t = &s.progressions;
    let elems = points
        .iter()
        .map(|&(k, v)| {
            entry(
                0x3000_0005,
                structure(vec![
                    e(t.linear_key, PropertyValue::Int(k)),
                    e(t.linear_value, PropertyValue::Float(v)),
                ]),
            )
        })
        .collect();
    Progression::from_properties(&set(vec![e(t.linear_array, array(elems))]), false, t).unwrap()
}

fn at(p: &Progression, x: i64) -> Option<f32> {
    p.value_at(x).and_then(|v| v.as_f32())
}

#[test]
fn armour_example() {
    let p = linear(&[(1, 10.0), (11, 30.0), (21, 30.0)]);
    assert_eq!(at(&p, 6), Some(20.0));
    assert_eq!(armour_value(&p, 6), Some(20));
    assert_eq!(armour_value(&p, 7), Some(22));
    assert_eq!(armour_value(&p, 4), Some(16));
    assert_eq!(armour_value(&p, 25), Some(30));
    assert_eq!(at(&p, 11), Some(30.0));
    assert_eq!(at(&p, 21), Some(30.0));
    assert_eq!(armour_value(&p, -5), Some(10));
    let q = linear(&[(1, 2.0), (5, 5.0)]);
    assert_eq!(at(&q, 2), Some(2.75));
    assert_eq!(armour_value(&q, 2), Some(2));
    // One point: its value everywhere.
    assert_eq!(armour_value(&linear(&[(3, 7.5)]), 100), Some(7));
    // Malformed: no points, or keys that do not strictly increase.
    assert_eq!(armour_value(&linear(&[]), 1), None);
    assert_eq!(armour_value(&linear(&[(5, 1.0), (5, 2.0)]), 5), None);
    assert!(!linear(&[(5, 1.0), (4, 2.0)]).keys_increasing());
    // Extreme keys and levels do not overflow.
    let wide = linear(&[(i32::MIN, 0.0), (i32::MAX, 1.0)]);
    assert!(armour_value(&wide, 0).is_some());
    assert_eq!(armour_value(&p, i32::MIN), Some(10));
    assert_eq!(armour_value(&p, i32::MAX), Some(30));
}

// §13.5, with the `combat-data.md` §5.1 / §5.5 readings of −1 and null.
#[test]
fn mod_entry_example() {
    let s = schema();
    let m = &s.modifiers;
    let stat = 0x1000_0123;
    let constant = structure(vec![
        e(m.modified, PropertyValue::PropertyId(stat)),
        e(m.op, PropertyValue::Enum(7)),
        entry(stat, PropertyValue::Int(15)),
    ]);
    let progression = structure(vec![
        e(m.modified, PropertyValue::PropertyId(stat)),
        e(m.op, PropertyValue::Enum(7)),
        e(m.progression, did(0x7000_1000)),
        e(m.floor, PropertyValue::Int(10)),
        e(m.ceiling, PropertyValue::Int(20)),
        entry(0x1000_0999, PropertyValue::Int(1)),
    ]);
    let broken = structure(vec![e(m.op, PropertyValue::Enum(7))]);
    // A stored −1 end is open; a null progression is "none".
    let open_ends = structure(vec![
        e(m.modified, PropertyValue::PropertyId(stat)),
        e(m.op, PropertyValue::Enum(7)),
        entry(stat, PropertyValue::Int(3)),
        e(m.floor, PropertyValue::Int(-1)),
        e(m.ceiling, PropertyValue::Int(-1)),
    ]);
    let null_progression = structure(vec![
        e(m.modified, PropertyValue::PropertyId(stat)),
        e(m.op, PropertyValue::Enum(7)),
        e(m.progression, did(0)),
    ]);
    let item = set(vec![e(
        m.array,
        array(vec![
            entry(0x3000_0006, constant),
            entry(0x3000_0006, progression),
            entry(0x3000_0006, broken),
            entry(0x3000_0006, PropertyValue::Int(0)),
            entry(0x3000_0006, open_ends),
            entry(0x3000_0006, null_progression),
        ]),
    )]);
    let mods = ItemView::new(&item, &s).mods();
    assert_eq!(mods.len(), 5);
    assert_eq!(mods[0].modified, stat);
    assert_eq!(mods[0].op, Some(7));
    assert_eq!(mods[0].kind, ModEntryKind::Inline);
    assert_eq!(mods[0].value, Some(PropertyValue::Int(15)));
    assert_eq!(mods[0].progression, None);
    assert!(mods[0].unrelated_fields.is_empty());
    assert!(mod_entry_is_well_formed(&mods[0]));
    assert!(mods[0].applies_at(Some(1)));
    assert_eq!(mods[1].kind, ModEntryKind::ProgressionDriven);
    assert_eq!(mods[1].progression, Some(Did(0x7000_1000)));
    assert_eq!(mods[1].value, None);
    assert_eq!(mods[1].unrelated_fields, vec![0x1000_0999]);
    assert!(mod_entry_is_well_formed(&mods[1]));
    assert!(mods[1].applies_at(Some(10)) && mods[1].applies_at(Some(20)));
    assert!(!mods[1].applies_at(Some(9)) && !mods[1].applies_at(Some(21)));
    assert!(!mod_entry_is_well_formed(&mods[2]));
    assert!(mods[2].is_skipped());
    // −1 ends do not limit.
    assert!(!mods[3].range.is_ranged());
    assert!(mods[3].applies_at(Some(1)) && mods[3].applies_at(Some(500)));
    assert!(mod_entry_is_well_formed(&mods[3]));
    // A null progression is no value source: IT9 fails.
    assert_eq!(mods[4].progression, None);
    assert_eq!(mods[4].kind, ModEntryKind::Valueless);
    assert!(!mod_entry_is_well_formed(&mods[4]));
}

// §13.6 and the §5.4 table
#[test]
fn binding_readings() {
    let f = |a, t, e| BindingFlags {
        on_acquire: a,
        to_account: t,
        on_equip: e,
    };
    assert_eq!(
        f(Some(true), Some(true), Some(false)).binding(),
        Binding::AccountOnAcquire
    );
    assert_eq!(
        f(Some(true), Some(true), None).binding(),
        Binding::AccountOnAcquire
    );
    assert_eq!(
        f(Some(true), None, None).binding(),
        Binding::CharacterOnAcquire
    );
    assert_eq!(
        f(Some(true), Some(false), Some(false)).binding(),
        Binding::CharacterOnAcquire
    );
    assert_eq!(
        f(Some(false), None, Some(true)).binding(),
        Binding::CharacterOnEquip
    );
    assert_eq!(f(None, None, None).binding(), Binding::Unbound);
    let open = f(Some(false), Some(true), Some(true));
    assert_eq!(open.binding(), Binding::Other(open));

    let s = schema();
    let p = &s.item;
    let item = set(vec![
        e(p.bind_on_acquire, PropertyValue::Bool(false)),
        e(p.bind_on_equip, PropertyValue::Bool(true)),
    ]);
    let no_defaults: HashMap<u32, PropertyValue> = HashMap::new();
    assert_eq!(
        ItemView::new(&item, &s).binding(&no_defaults),
        Binding::CharacterOnEquip
    );
    assert_eq!(
        ItemView::new(&set(vec![]), &s).binding(&no_defaults),
        Binding::Unbound
    );
    // Absent flags take their dictionary defaults (§14 decision).
    let defaults: HashMap<u32, PropertyValue> = [
        (id(p.bind_on_acquire), PropertyValue::Bool(true)),
        (id(p.bind_to_account), PropertyValue::Bool(true)),
        (id(p.bind_on_equip), PropertyValue::Bool(false)),
    ]
    .into();
    let empty = set(vec![]);
    let v = ItemView::new(&empty, &s);
    assert_eq!(v.binding_flags(), BindingFlags::default());
    assert_eq!(v.binding(&defaults), Binding::AccountOnAcquire);
    // A stored flag wins over the default.
    let stored = set(vec![e(p.bind_to_account, PropertyValue::Bool(false))]);
    assert_eq!(
        ItemView::new(&stored, &s).binding(&defaults),
        Binding::CharacterOnAcquire
    );
}

#[test]
fn dictionary_defaults_fill_absent_properties() {
    let s = schema();
    let p = &s.item;
    let defaults: HashMap<u32, PropertyValue> = [
        (id(p.min_level), PropertyValue::Int(1)),
        (id(p.level), PropertyValue::Int(0)),
        (id(p.allowed_in_combat), PropertyValue::Bool(true)),
        (id(p.allowed_while_mounted), PropertyValue::Bool(false)),
        (id(p.min_usage_dist), PropertyValue::Float(2.0)),
    ]
    .into();
    let item = set(vec![
        e(p.allowed_in_combat, PropertyValue::Bool(false)),
        e(p.level, PropertyValue::Int(42)),
    ]);
    let v = ItemView::new(&item, &s);
    assert_eq!(v.min_level(), None);
    assert_eq!(v.min_level_or_default(&defaults), Some(1));
    assert_eq!(v.level_or_default(&defaults), Some(42));
    assert_eq!(v.allowed_in_combat(&defaults), Some(false));
    assert_eq!(v.allowed_while_mounted(&defaults), Some(false));
    assert_eq!(v.min_usage_distance(&defaults), Some(2.0));
    assert_eq!(v.destroy_on_use(&defaults), None);
    assert_eq!(
        v.value_or_default(p.min_level, &defaults),
        Some(&PropertyValue::Int(1))
    );
}

// §13.7 and IT11
#[test]
fn set_membership_example() {
    let (a, b) = (Did(0x7000_0A00), Did(0x7000_0B00));
    let item = |n: u32| Did(0x7000_0000 + n);
    let mut m = SetMembership::new();
    m.add_member_list(a, [item(1), item(2), item(3), Did(0)]);
    m.add_member_list(b, []);
    m.add_property_set_member(item(7), b);
    m.add_property_set_member(item(8), b);
    assert_eq!(m.members_of(a), &[item(1), item(2), item(3)]);
    assert_eq!(m.members_of(b), &[item(7), item(8)]);
    assert_eq!(m.set_of(item(2)), Some(a));
    assert_eq!(m.set_of(item(8)), Some(b));
    assert_eq!(m.set_of(item(9)), None);
    assert_eq!(m.len(), 5);
    assert!(m.conflicts().is_empty());
    // Adding the same pair again is not a conflict; a second set is.
    m.add_member_list(a, [item(1)]);
    assert!(m.conflicts().is_empty());
    m.add_member_list(b, [item(1)]);
    assert_eq!(m.conflicts(), &[(item(1), a, b)]);
    assert_eq!(m.set_of(item(1)), Some(a));
    assert_eq!(m.sets().collect::<Vec<_>>(), vec![a, b]);
}

#[test]
fn set_view_reads_tiers() {
    let s = schema();
    let p = &s.sets;
    let m = &s.modifiers;
    let fx = &s.effects;
    let tier = |count: i32| {
        entry(
            0x3000_0007,
            structure(vec![
                e(p.active_count, PropertyValue::Int(count)),
                e(
                    m.array,
                    array(vec![entry(
                        0x3000_0006,
                        structure(vec![
                            e(m.modified, PropertyValue::PropertyId(0x1000_0123)),
                            e(m.op, PropertyValue::Enum(7)),
                            entry(0x1000_0123, PropertyValue::Float(2.5)),
                            e(m.ceiling, PropertyValue::Int(-1)),
                        ]),
                    )]),
                ),
                e(
                    p.effects,
                    array(vec![entry(
                        0x3000_0008,
                        structure(vec![e(fx.effect_id, did(0x7000_2000))]),
                    )]),
                ),
            ]),
        )
    };
    let record = set(vec![
        e(p.level, PropertyValue::Int(50)),
        e(p.level_required, PropertyValue::Bool(true)),
        e(
            p.member_list,
            array(vec![
                e(p.member, did(0x7000_0001)),
                e(p.member, did(0x7000_0002)),
            ]),
        ),
        e(p.tiers, array(vec![tier(2), tier(4)])),
    ]);
    let v = ItemSetView::new(&record, &s);
    assert_eq!(v.level(), Some(50));
    assert_eq!(v.level_required(), Some(true));
    assert!(v.has_member_list());
    assert_eq!(v.members(), vec![Did(0x7000_0001), Did(0x7000_0002)]);
    let tiers = v.tiers();
    assert_eq!(tiers.len(), 2);
    assert_eq!(tiers[1].active_count, Some(4));
    assert_eq!(tiers[0].mods.len(), 1);
    assert!(mod_entry_is_well_formed(&tiers[0].mods[0]));
    assert!(tiers[0].mods[0].applies_at(Some(80)));
    assert_eq!(tiers[0].effects[0].effect, Some(Did(0x7000_2000)));
    assert!(v.tiers_ascending());

    let unordered = set(vec![e(p.tiers, array(vec![tier(3), tier(3)]))]);
    assert!(!ItemSetView::new(&unordered, &s).tiers_ascending());

    let dir = set(vec![e(
        p.directory,
        array(vec![
            entry(0x3000_0009, did(0x7000_0A00)),
            entry(0x3000_0009, did(0)),
        ]),
    )]);
    assert_eq!(set_directory(&dir, p), vec![Did(0x7000_0A00)]);
}

#[test]
fn item_view_groups_and_kinds() {
    let s = schema();
    let p = &s.item;
    let m = &s.effects;
    let recipe_effect = entry(
        0x3000_000A,
        structure(vec![
            e(m.effect_id, did(0x7000_3000)),
            e(m.effect_spellcraft, PropertyValue::Float(-1.0)),
            e(
                m.effect_data,
                array(vec![e(m.crafting_recipe_list, PropertyValue::Int(1))]),
            ),
        ]),
    );
    let socket = |bits: u64, extra: bool| {
        let mut fields = vec![
            e(p.socket_type, PropertyValue::Bitfield64(bits)),
            e(p.socket_unlock_level, PropertyValue::Int(50)),
        ];
        if extra {
            fields.push(entry(0x1000_0999, PropertyValue::Int(0)));
        }
        entry(0x3000_000B, structure(fields))
    };
    let item = set(vec![
        e(p.item_class, PropertyValue::Enum(3)),
        e(p.weenie_type, PropertyValue::Enum(0x81)),
        e(p.name, PropertyValue::Int(0)), // wrong type: reads as absent
        e(p.phys_obj, did(0x4700_0001)),
        e(p.usage_action, did(0x7000_0000)),
        e(p.icon_background, did(0x4100_0001)),
        e(p.equipment_category, PropertyValue::Bitfield64(0b1001)),
        e(p.level, PropertyValue::Int(5)),
        e(p.value, PropertyValue::Int(12)),
        e(p.value_table, did(0)),
        e(p.max_quantity, PropertyValue::Int(50)),
        e(p.durability_enum, PropertyValue::Enum(3)),
        e(p.max_structure_points_enum, PropertyValue::Enum(4)),
        e(p.max_structure_points, PropertyValue::Int(80)),
        e(p.cur_structure_points, PropertyValue::Int(80)),
        e(p.cooldown, PropertyValue::Enum(9)),
        e(
            p.socket_array,
            array(vec![socket(4, false), socket(6, false), socket(1, true)]),
        ),
        e(m.usage_effects, array(vec![recipe_effect])),
        e(p.advancement_item, PropertyValue::Bool(true)),
    ]);
    let v = ItemView::new(&item, &s);
    // IT1 tests presence; a mistyped Name still reads as absent.
    assert!(v.has_item_signature());
    assert_eq!(v.name(), None);
    assert_eq!(v.signature_kind(), ItemKind::MultiRecipeScroll);
    assert_eq!(v.item_class(), Some(3));
    assert_eq!(v.level(), Some(5));
    assert_eq!(v.value(), Some(12));
    assert_eq!(v.value_table(), Some(Did(0)));
    assert_eq!(v.max_quantity(), Some(50));
    assert_eq!(
        equipment_category_keys(v.equipment_category().unwrap()).collect::<Vec<_>>(),
        vec![1, 4]
    );
    let d = v.durability();
    assert_eq!(d.durability_key, Some(3));
    assert_eq!(d.max_structure_points, d.cur_structure_points);
    assert_eq!(v.cooldown(), Some(9));
    assert_eq!(v.behaviours().action, Some(Did(0x7000_0000)));
    let sockets = v.sockets();
    assert_eq!(sockets.len(), 3);
    assert!(sockets[0].is_well_formed());
    assert!(!sockets[1].is_well_formed()); // two bits
    assert!(!sockets[2].is_well_formed()); // three fields
    let usage = v.effects(EffectListKind::Usage);
    assert_eq!(usage.len(), 1);
    assert_eq!(usage[0].spellcraft, Some(-1.0));
    assert_eq!(
        usage[0].argument(m.crafting_recipe_list),
        Some(&PropertyValue::Int(1))
    );
    assert!(v.effects(EffectListKind::Equipper).is_empty());
    assert!(v.is_advancing());

    let base = |extra: Vec<PropertyEntry>| {
        let mut entries = vec![
            e(p.item_class, PropertyValue::Enum(1)),
            e(p.weenie_type, PropertyValue::Enum(0x81)),
            e(p.name, PropertyValue::Bool(false)),
        ];
        entries.extend(extra);
        set(entries)
    };
    let kind = |extra| ItemView::new(&base(extra), &s).signature_kind();
    assert_eq!(
        kind(vec![
            e(p.recipe, did(0x7000_0001)),
            e(p.barring_recipe, did(0x7000_0001))
        ]),
        ItemKind::RecipeScroll
    );
    assert_eq!(kind(vec![e(p.recipe, did(0x7000_0001))]), ItemKind::General);
    assert_eq!(
        kind(vec![e(p.package_trophy_list, did(0))]),
        ItemKind::Package
    );
    assert_eq!(
        kind(vec![
            e(p.text_title, PropertyValue::Int(0)),
            e(p.text_content, array(vec![]))
        ]),
        ItemKind::TextItem
    );
    assert_eq!(
        kind(vec![e(p.players_can_close, PropertyValue::Bool(true))]),
        ItemKind::Door
    );
    assert_eq!(
        kind(vec![e(p.key_ring, PropertyValue::Bool(true))]),
        ItemKind::Key
    );
    assert_eq!(
        kind(vec![e(p.worn_appearance_list, array(vec![]))]),
        ItemKind::Wearable
    );
    assert_eq!(kind(vec![]), ItemKind::General);
}

fn mapper(did: u32, base: u32, names: &[(u32, &str)]) -> EnumMapper {
    let count = u32::try_from(names.len()).unwrap();
    let mut b = B::new().u32(did).u32(base).tsize(0, count);
    for &(k, n) in names {
        b = b.u32(k).pascal(n);
    }
    EnumMapper::parse(&b.tsize(0, 0).build()).expect("synthetic mapper")
}

// §2.1 `WeenieType` column, §2.4 rule 4.
#[test]
fn weenie_type_labels_give_kinds() {
    let s = schema();
    let p = &s.item;
    // Invented keys; the labels are the interoperability names of §2.1.
    let base = mapper(
        0x2300_0002,
        0,
        &[(1, "Item"), (2, "Door"), (3, "Waypoint"), (9, "Weapon")],
    );
    let own = mapper(
        0x2300_0001,
        0x2300_0002,
        &[
            (4, "Jewelry"),
            (5, "Shield"),
            (6, "Milestone"),
            (7, "Clothing"),
            (8, "Key"),
            (2, "Letter"),
        ],
    );
    let kinds = ItemWeenieKinds::from_mapper_chain(&[own, base]);
    // Key 2 takes the nearest mapper's label.
    assert_eq!(kinds.kind(2), Some(ItemKind::TextItem));
    assert_eq!(kinds.kind(1), None);
    assert_eq!(kinds.kind(3), Some(ItemKind::TravelPoint));
    assert_eq!(kinds.len(), 8);
    assert!(ItemWeenieKinds::default().is_empty());

    let with = |key: u32, extra: Vec<PropertyEntry>| {
        let mut entries = vec![e(p.weenie_type, PropertyValue::Enum(key))];
        entries.extend(extra);
        set(entries)
    };
    let kind = |set: &tessera_formats::PropertySet| ItemView::new(set, &s).kind(&kinds);
    assert_eq!(kind(&with(4, vec![])), ItemKind::Jewellery);
    assert_eq!(kind(&with(5, vec![])), ItemKind::Shield);
    assert_eq!(kind(&with(6, vec![])), ItemKind::Milestone);
    assert_eq!(kind(&with(3, vec![])), ItemKind::TravelPoint);
    assert_eq!(kind(&with(8, vec![])), ItemKind::Key);
    assert_eq!(kind(&with(7, vec![])), ItemKind::Wearable);
    assert_eq!(kind(&with(1, vec![])), ItemKind::General);
    assert_eq!(kind(&with(99, vec![])), ItemKind::General);
    // A label without a kind falls through to the worn-look list.
    assert_eq!(
        kind(&with(1, vec![e(p.worn_appearance_list, array(vec![]))])),
        ItemKind::Wearable
    );
    // Signatures come first.
    assert_eq!(
        kind(&with(4, vec![e(p.package_trophy_list, did(0))])),
        ItemKind::Package
    );
    // Without labels, only signatures count.
    assert_eq!(
        ItemView::new(&with(4, vec![]), &s).signature_kind(),
        ItemKind::General
    );
    assert_eq!(ItemKind::ALL.len(), 13);
}

// §5.2–§5.4, §5.10, §5.11 groups.
#[test]
fn item_view_reads_requirements_slots_and_raw_groups() {
    let s = schema();
    let p = &s.item;
    let r = &s.requirements;
    let item = set(vec![
        e(p.compatible_slot, PropertyValue::Bitfield32(0b110)),
        e(p.default_slot, PropertyValue::Bitfield32(0b10)),
        e(p.unique, PropertyValue::Bool(true)),
        e(p.uniqueness_channel, PropertyValue::Enum(12)),
        e(p.munge_min_level, PropertyValue::Int(20)),
        e(
            p.munge_override_property,
            PropertyValue::PropertyId(0x1000_0042),
        ),
        e(p.ilevel_bonus_chance, PropertyValue::Float(0.25)),
        e(p.granted_skill, did(0x7000_0700)),
        e(p.cooldown_channel, PropertyValue::Enum(4)),
        e(p.socket_type, PropertyValue::Bitfield64(1 << 3)),
        e(p.gem_min_level, PropertyValue::Int(500)),
        e(p.gem_max_level, PropertyValue::Int(499)),
        e(p.socket_levelup_increment, PropertyValue::Int(2)),
        e(p.property_set, did(0x7000_0B00)),
        e(p.disenchant_value, PropertyValue::Int(-1)),
        e(p.disenchant_component, did(0x7000_0C00)),
        e(p.min_level, PropertyValue::Int(30)),
        e(
            r.required_classes,
            array(vec![
                entry(0x3000_0020, PropertyValue::Enum(23)),
                entry(0x3000_0020, PropertyValue::Int(5)), // mistyped: skipped
                entry(0x3000_0020, PropertyValue::Enum(162)),
            ]),
        ),
        e(
            r.required_faction,
            structure(vec![
                e(r.required_faction_did, did(0x7000_0D00)),
                e(r.required_faction_tier, PropertyValue::Enum(4)),
            ]),
        ),
        e(
            r.required_trait_array,
            array(vec![entry(
                0x3000_0021,
                structure(vec![
                    e(r.required_trait, did(0x7000_0E00)),
                    e(r.required_virtue_rank, PropertyValue::Int(3)),
                ]),
            )]),
        ),
        e(
            r.quest_requirements,
            array(vec![entry(
                0x3000_0022,
                structure(vec![
                    e(r.quest_operator, PropertyValue::Enum(3)),
                    e(r.quest_id, did(0x7000_0F00)),
                    e(r.quest_status, PropertyValue::Enum(0x1000_0000)),
                ]),
            )]),
        ),
        e(r.min_glory_rank, PropertyValue::Int(6)),
        e(r.required_craft_tier, PropertyValue::Enum(2)),
    ]);
    let v = ItemView::new(&item, &s);
    let slots = v.slots();
    assert_eq!(slots.compatible, Some(0b110));
    assert_eq!(slots.default, Some(0b10));
    assert_eq!(slots.precluded, None);
    assert_eq!(v.unique(), Some(true));
    assert_eq!(v.unique_equipped(), None);
    let m = v.munging();
    assert!(!m.is_empty());
    assert_eq!(m.min_level, Some(20));
    assert_eq!(m.override_property, Some(0x1000_0042));
    assert_eq!(m.bonus_chance, Some(0.25));
    assert!(ItemView::new(&set(vec![]), &s).munging().is_empty());
    assert_eq!(v.granted_skill(), Some(Did(0x7000_0700)));
    assert_eq!(v.granted_trait(), None);
    assert_eq!(v.cooldown_channel(), Some(4));
    let g = v.socketable();
    assert_eq!(g.socket_type, Some(1 << 3));
    assert_eq!((g.gem_min_level, g.gem_max_level), (Some(500), Some(499)));
    assert_eq!(g.levelup_increment, Some(2));
    assert_eq!(g.uniqueness_channel, Some(12));
    assert_eq!(g.item_set, Some(Did(0x7000_0B00)));
    let d = v.disenchant();
    assert_eq!(d.value, Some(-1));
    assert_eq!(d.component, Some(Did(0x7000_0C00)));
    assert_eq!(d.trophy_list, None);

    let req = v.requirements();
    assert_eq!(req.min_level, Some(30));
    assert_eq!(req.classes, vec![23, 162]);
    assert!(req.races.is_empty());
    let f = req.faction.expect("faction");
    assert_eq!((f.faction, f.tier), (Some(Did(0x7000_0D00)), Some(4)));
    assert_eq!(req.required_traits.len(), 1);
    assert_eq!(req.required_traits[0].virtue_rank, Some(3));
    assert_eq!(req.quests.len(), 1);
    assert_eq!(req.quests[0].operator, 3);
    assert_eq!(req.quests[0].quest, Some(Did(0x7000_0F00)));
    assert!(!req.quests[0].is_empty_slot());
    assert_eq!(req.min_glory_rank, Some(6));
    assert_eq!(req.craft_tier, Some(2));
    assert_eq!(req.required_effect, None);
}

#[test]
fn wear_timing_and_duration_tables() {
    let s = schema();
    let t = &s.tables;
    let keyed = |kp: Pid, k: u32, vp: Pid, v: PropertyValue| {
        entry(
            0x3000_000C,
            structure(vec![e(kp, PropertyValue::Enum(k)), e(vp, v)]),
        )
    };
    let wear = set(vec![
        e(
            t.wear_msp_array,
            array(vec![
                keyed(t.wear_msp_enum, 4, t.wear_msp_value, PropertyValue::Int(80)),
                keyed(
                    t.wear_msp_enum,
                    5,
                    t.wear_msp_value,
                    PropertyValue::Int(100),
                ),
            ]),
        ),
        e(
            t.wear_durability_array,
            array(vec![keyed(
                t.wear_durability_enum,
                3,
                t.wear_durability_value,
                PropertyValue::Float(0.75),
            )]),
        ),
        // Read by field type: first enum, first number.
        e(
            t.wear_state_array,
            array(vec![
                entry(
                    0x3000_000D,
                    structure(vec![
                        entry(0x3000_00F1, PropertyValue::Float(1.0)),
                        entry(0x3000_00F2, PropertyValue::Enum(2)),
                    ]),
                ),
                entry(
                    0x3000_000D,
                    structure(vec![entry(1, PropertyValue::Enum(2))]),
                ),
                entry(0x3000_000D, PropertyValue::Int(5)),
            ]),
        ),
    ]);
    let w = WearControl::from_set(&wear, t).unwrap();
    assert_eq!(w.max_structure_points(4), Some(80));
    assert_eq!(w.max_structure_points(6), None);
    assert_eq!(w.durability_value(3), Some(0.75));
    assert_eq!(w.wear_state_value, vec![(2, 1.0)]);
    assert!(w.repair_cost_quality.is_empty());
    assert!(WearControl::from_set(&set(vec![]), t).is_err());

    let timing = set(vec![
        e(t.timing_category, PropertyValue::Bitfield64(1 << 7)),
        e(
            t.timing_array,
            array(vec![entry(
                0x3000_000E,
                structure(vec![
                    e(t.timing_speed, PropertyValue::Enum(1)),
                    e(t.timing_duration, PropertyValue::Float(1.0)),
                    e(t.timing_anim_mod, PropertyValue::Float(-0.1)),
                ]),
            )]),
        ),
    ]);
    let table = WeaponTimingTable::from_set(&timing, t).unwrap();
    let timings = WeaponTimings {
        tables: vec![table],
    };
    let hit = timings.lookup(1 << 7, 1).unwrap();
    assert_eq!((hit.action_duration, hit.anim_duration_mod), (1.0, -0.1));
    assert_eq!(timings.lookup(1 << 6, 1), None);
    assert_eq!(timings.lookup(1 << 7, 2), None);
    let dir = set(vec![e(
        t.timing_directory,
        array(vec![entry(1, did(0x7000_0001)), entry(1, did(0))]),
    )]);
    assert_eq!(weapon_timing_table_dids(&dir, t), vec![Did(0x7000_0001)]);

    let cooldowns = set(vec![e(
        t.cooldown_map,
        array(vec![entry(
            0x3000_000F,
            structure(vec![
                entry(0x3000_00F3, PropertyValue::Enum(9)),
                entry(0x3000_00F4, PropertyValue::Float(30.0)),
            ]),
        )]),
    )]);
    let map = DurationMap::from_set(&cooldowns, t.cooldown_map);
    assert_eq!(map.seconds(9), Some(30.0));
    assert_eq!(map.seconds(0), Some(0.0));
    assert_eq!(map.seconds(8), None);
    assert!(map.covers(0) && map.covers(9) && !map.covers(8));
    assert!(
        DurationMap::from_set(&set(vec![]), t.decay_map)
            .rows
            .is_empty()
    );
}

#[test]
fn auxiliary_records() {
    let s = schema();
    let a = &s.aux;
    let runic = set(vec![e(
        a.runic_list_array,
        array(vec![entry(
            0x3000_0010,
            structure(vec![
                e(a.runic_list_frequency, PropertyValue::Enum(2)),
                e(a.runic_list_target, did(0x7000_4000)),
            ]),
        )]),
    )]);
    let list = runic_list(&runic, a);
    assert_eq!(list.len(), 1);
    assert_eq!(list[0].drop_frequency, Some(2));
    assert_eq!(list[0].target, Some(Did(0x7000_4000)));

    let legacy = set(vec![e(
        a.legacy_effect_array,
        array(vec![entry(
            0x3000_0011,
            structure(vec![
                e(a.legacy_effect, did(0x7000_5000)),
                e(a.legacy_weight, PropertyValue::Int(3)),
            ]),
        )]),
    )]);
    let effects = legacy_effects(&legacy, a);
    assert_eq!(effects[0].effect, Some(Did(0x7000_5000)));
    assert_eq!(effects[0].weight, Some(3.0));

    // A relic's modifiers use the `combat` reader: a −1 floor is open.
    let m = &s.modifiers;
    let relic = set(vec![
        e(a.runic_level, PropertyValue::Int(40)),
        e(a.runic_tier, PropertyValue::Enum(3)),
        e(
            m.array,
            array(vec![entry(
                0x3000_0006,
                structure(vec![
                    e(m.modified, PropertyValue::PropertyId(0x1000_0123)),
                    e(m.op, PropertyValue::Enum(7)),
                    e(m.progression, did(0x7000_1000)),
                    e(m.floor, PropertyValue::Int(-1)),
                    e(m.ceiling, PropertyValue::Int(50)),
                ]),
            )]),
        ),
    ]);
    let v = RelicView::new(&relic, &s);
    assert_eq!(v.level(), Some(40));
    assert_eq!(v.tier(), Some(3));
    let mods = v.mods();
    assert_eq!(mods[0].range.floor, None);
    assert!(mods[0].applies_at(Some(1)) && !mods[0].applies_at(Some(51)));
}

#[test]
fn property_categories() {
    assert_eq!(entity_labels().len(), 16);
    let mut unique: Vec<&str> = entity_labels().to_vec();
    unique.sort_unstable();
    unique.dedup();
    assert_eq!(unique.len(), 16);
    for l in ENTITY_STAMP_LABELS {
        assert_eq!(property_category(l), ItemPropertyCategory::EntityStamp);
    }
    for (c, _) in COPY_LABELS {
        assert_eq!(property_category(c), ItemPropertyCategory::Copy);
    }
    assert_eq!(
        property_category("Item_CurStructurePoints"),
        ItemPropertyCategory::Copy
    );
    for d in DERIVED_LABELS {
        assert_eq!(property_category(d), ItemPropertyCategory::Derived);
    }
    assert_eq!(
        property_category("Item_Level"),
        ItemPropertyCategory::Authored
    );
}
