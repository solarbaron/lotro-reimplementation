// SPDX-License-Identifier: GPL-3.0-or-later
//! Combat views (`tessera_formats::gameplay::combat`, `docs/specs/combat-data.md`) on
//! arbitrary bytes.
//!
//! Input byte 0 selects the mode:
//!
//! - **even**: byte 1 splits the rest into an object payload (at most 255 bytes) and a
//!   property-set payload, decoded with a total synthetic schema (type = PID mod 24). Every
//!   combat label resolves to a PID whose schema type is a plausible one for the label, so
//!   the views see well-typed and ill-typed trees alike. The object is read as every kind of
//!   combat object, with a class tree built from the same bytes.
//! - **odd**: the bytes drive synthetic progressions (§4), modifier entries (§5), the sandbox
//!   combination (D2) and trait-cell keys (§8.5), checking the evaluation invariants.
//!
//! Nothing may panic; allocations stay bounded by the decoder's limits.
//!
//! Run with a memory cap, e.g.
//! `cargo fuzz run formats_gameplay_combat -- -malloc_limit_mb=512 -rss_limit_mb=1024`.

#![no_main]

use std::collections::HashMap;

use libfuzzer_sys::fuzz_target;
use tessera_formats::gameplay::combat::{
    AdvancementLine, BattleStatistic, Channelling, ClassTable, CombatClasses, CombatControl,
    CombatFamily, CombatPids, ControlPoint, Effect, EffectControl, Induction, Lookup, ModEntry,
    ModOp, MonsterPlayTable, Progression, ProgressionValue, QualityTable, Skill,
    SparseDidProgression, ThresholdPolicy, Trait, TraitCell, TraitControl, TraitTree,
    effect_references, evaluate_progression, mod_arrays, mod_entries, ranged_groups,
    read_combat_object, sandbox_combine, segment_slope, value_arrays_present,
};
use tessera_formats::gameplay::common::build::{array, did, entry, set, structure};
use tessera_formats::gameplay::common::{ClassTree, GameObjectRecord, ObjectRoot};
use tessera_formats::{Did, PropertyType, PropertyValue};

fn total(pid: u32) -> Option<PropertyType> {
    Some(PropertyType::from_code(pid % 24))
}

/// A type code that suits the label, from its wording (fuzzing aid only).
fn code_for(label: &str) -> u32 {
    let has = |w: &[&str]| w.iter().any(|x| label.contains(x));
    if has(&["Array", "List", "Mod_Array"]) {
        12
    } else if has(&[
        "Struct",
        "Entry",
        "Info",
        "Data",
        "ControlPoint",
        "Policy",
        "Random",
        "Variable",
    ]) {
        11
    } else if has(&["_Name", "Desc", "Tooltip", "StringInfoProgression_Value"]) {
        13
    } else if has(&[
        "Progression",
        "Icon",
        "DID",
        "EffectID",
        "Effect_",
        "Trait",
        "Sorter",
        "Action",
        "State",
    ]) {
        20
    } else if has(&["Modified", "Property", "Prop", "Override"]) {
        10
    } else if has(&[
        "Op", "Type", "Class", "Category", "Nature", "Mode", "Quality", "Branch", "Location",
        "Anchor", "Channel", "Scope", "Policy",
    ]) {
        8
    } else if has(&[
        "Index", "Key", "Count", "Tier", "Level", "Rank", "Cost", "Floor", "Ceiling", "Number",
        "Depth", "Cap", "Targets",
    ]) {
        15
    } else if has(&[
        "Can",
        "Is",
        "Uses",
        "Harmful",
        "Debuff",
        "Permanent",
        "Instantaneous",
        "Requires",
        "Valid",
        "Allowed",
        "Immediate",
        "Ignores",
    ]) {
        21
    } else {
        9
    }
}

fn pids() -> &'static CombatPids {
    static PIDS: std::sync::OnceLock<CombatPids> = std::sync::OnceLock::new();
    PIDS.get_or_init(make_pids)
}

fn make_pids() -> CombatPids {
    let ids: HashMap<String, u32> = CombatPids::labels()
        .into_iter()
        .zip(1u32..)
        .map(|(l, i)| (l.to_owned(), i * 24 + code_for(l)))
        .collect();
    CombatPids::resolve(&ids)
}

fn u32s(bytes: &[u8]) -> impl Iterator<Item = u32> + '_ {
    bytes
        .as_chunks::<4>()
        .0
        .iter()
        .map(|&c| u32::from_le_bytes(c))
}

fn decoded_mode(rest: &[u8], pids: &CombatPids) {
    let [split, rest @ ..] = rest else {
        return;
    };
    let at = usize::from(*split).min(rest.len());
    let (object, set_bytes) = rest.split_at(at);
    let Ok(rec) = GameObjectRecord::decode(object, Some(set_bytes), &total) else {
        return;
    };
    let words: Vec<u32> = u32s(rest).collect();
    let tree = ClassTree::from_edges(
        words
            .as_chunks::<2>()
            .0
            .iter()
            .map(|&[a, b]| (a % 4096, b % 4096)),
    );
    let classes = CombatClasses::observed();
    let _ = read_combat_object(&rec, &classes, &tree, pids);
    for family in CombatFamily::ALL {
        let _ = pids.has_signature(family, &rec.properties);
    }
    let props = &rec.properties;
    let _ = value_arrays_present(props, &pids.progression);
    for nested in [false, true] {
        if let Ok(p) = Progression::from_properties(props, nested, &pids.progression) {
            check_progression(&p, &words);
        }
    }
    let _ = SparseDidProgression::from_properties(props, &pids.progression);
    for arr in mod_arrays(props, &pids.modifier) {
        for g in ranged_groups(&arr.entries) {
            check_group(&g);
        }
    }
    let _ = mod_entries(props, &pids.modifier);
    for r in effect_references(props, &pids.effect_ref) {
        let _ = (r.given_spellcraft(), r.given_duration(), r.is_placeholder());
    }
    let _ = Effect::read(&rec, pids).map(|e| e.duration.group());
    let _ = Skill::read(&rec, pids);
    let _ = Trait::read(&rec, pids);
    for e in &TraitTree::from_properties(props, &pids.traits).entries {
        if let Some(cell) = e.cell() {
            assert_eq!(TraitCell::from_key(cell.key()), cell);
        }
    }
    let _ = TraitControl::from_properties(props, &pids.traits);
    let _ = EffectControl::from_properties(props, pids);
    let _ = ClassTable::from_properties(props, &pids.class_table);
    let _ = AdvancementLine::from_properties(props, &pids.class_table);
    let _ = MonsterPlayTable::from_properties(props, &pids.class_table);
    let _ = CombatControl::from_properties(props, &pids.tables);
    let _ = QualityTable::dps_from_properties(props, &pids.tables).quality(1);
    let _ = QualityTable::value_from_properties(props, &pids.tables).quality(5);
    let _ = ThresholdPolicy::from_properties(props, &pids.tables);
    let _ = BattleStatistic::from_properties(props, &pids.tables);
    let _ = Induction::from_properties(props, &pids.timing);
    let _ = Channelling::from_properties(props, &pids.timing);
    let _ = ObjectRoot::read(object);
}

fn check_progression(p: &Progression, probes: &[u32]) {
    let range = p.covered_range();
    for &w in probes.iter().take(16) {
        let x = i64::from(w as i32);
        let clamped = p.value_at(x);
        match range {
            Some((lo, hi)) => {
                assert!(lo <= hi);
                // Debug form, so that NaN values compare equal.
                assert_eq!(
                    format!("{clamped:?}"),
                    format!("{:?}", p.lookup(x.clamp(lo, hi)).value())
                );
                if (lo..=hi).contains(&x) {
                    assert!(matches!(p.lookup(x), Lookup::Value(_)));
                }
            }
            None => assert_eq!(clamped, None),
        }
        let empty: HashMap<Did, Progression> = HashMap::new();
        let _ = p.evaluate(&empty, x, Some(x));
    }
    let segs = p.segments();
    if !segs.is_empty() {
        assert_eq!(segs.len() + 1, p.len());
    }
}

fn check_group(g: &tessera_formats::gameplay::combat::RangedGroup) {
    assert!(g.members.windows(2).all(|w| {
        let lo =
            |r: &tessera_formats::gameplay::combat::LevelRange| r.floor.map_or(i64::MIN, i64::from);
        lo(&w[0].1) <= lo(&w[1].1)
    }));
    for level in [-1i64, 0, 1, 50, 1000] {
        if let Some(i) = g.select(level) {
            assert!(g.members.iter().any(|&(j, r)| j == i && r.contains(level)));
        }
    }
}

fn synthetic_mode(rest: &[u8], pids: &CombatPids) {
    let pp = &pids.progression;
    let words: Vec<u32> = u32s(rest).take(64).collect();
    let Some((&head, body)) = words.split_first() else {
        return;
    };
    let id = |p: tessera_formats::gameplay::common::Pid| p.id().unwrap_or(1);
    let n = (head as usize >> 8) % 24;
    let min = (head >> 16) as i16;
    let values: Vec<u32> = body.iter().copied().take(n).collect();

    // Linear progression from (key, value) pairs; keys sorted or not.
    let mut keys: Vec<i32> = values.iter().map(|&v| (v as i32) >> 20).collect();
    if head & 1 == 0 {
        keys.sort_unstable();
        keys.dedup();
    }
    let linear = set(vec![entry(
        id(pp.linear_array),
        array(
            keys.iter()
                .zip(values.iter())
                .map(|(&k, &v)| {
                    entry(
                        id(pp.linear_point),
                        structure(vec![
                            entry(id(pp.linear_key), PropertyValue::Int(k)),
                            entry(id(pp.linear_value), PropertyValue::Float(f32::from_bits(v))),
                        ]),
                    )
                })
                .collect(),
        ),
    )]);
    if let Ok(p) = Progression::from_properties(&linear, false, pp) {
        check_progression(&p, body);
        for w in keys.windows(2) {
            let a = ControlPoint {
                key: w[0],
                value: 1.0,
            };
            let b = ControlPoint {
                key: w[1],
                value: 2.0,
            };
            let _ = segment_slope(a, b);
        }
    }
    // Float array with a minimum index.
    let floats = set(vec![
        entry(
            id(pp.float_array),
            array(
                values
                    .iter()
                    .map(|&v| entry(id(pp.float_value), PropertyValue::Float(f32::from_bits(v))))
                    .collect(),
            ),
        ),
        entry(id(pp.min_index), PropertyValue::Int(i32::from(min))),
    ]);
    let mut source: HashMap<Did, Progression> = HashMap::new();
    if let Ok(p) = Progression::from_properties(&floats, false, pp) {
        check_progression(&p, body);
        source.insert(Did(0x7000_0001), p);
    }
    // A progression of progressions over the two (and arbitrary) DIDs.
    let nested = set(vec![entry(
        id(pp.did_array),
        array(
            values
                .iter()
                .map(|&v| {
                    entry(
                        id(pp.did_value),
                        did(if v & 1 == 0 { 0x7000_0001 } else { v }),
                    )
                })
                .collect(),
        ),
    )]);
    if let Ok(p) = Progression::from_properties(&nested, true, pp) {
        source.insert(Did(0x7000_0002), p);
    }
    for &w in body.iter().take(8) {
        let x = i64::from(w as i32);
        let v = evaluate_progression(&source, Did(0x7000_0002), x, Some(x));
        assert!(matches!(v, None | Some(ProgressionValue::Float(_))));
        assert_eq!(
            evaluate_progression(&source, Did(0x7000_0002), x, None),
            None
        );
    }

    // Modifier entries and the sandbox.
    let m = &pids.modifier;
    let entries: Vec<ModEntry> = values
        .iter()
        .map(|&v| {
            let modified = v % 5;
            let mut fields = vec![
                entry(id(m.modified), PropertyValue::PropertyId(modified)),
                entry(id(m.op), PropertyValue::Enum(v % 13)),
            ];
            if v & 0x10 != 0 {
                fields.push(entry(
                    id(m.floor),
                    PropertyValue::Int((v >> 8) as i32 % 300),
                ));
            }
            if v & 0x20 != 0 {
                fields.push(entry(
                    id(m.ceiling),
                    PropertyValue::Int((v >> 16) as i32 % 1000),
                ));
            }
            if v & 0x40 != 0 && modified != 0 {
                fields.push(entry(modified, PropertyValue::Int(v as i32)));
            }
            ModEntry::from_struct(
                match &structure(fields) {
                    PropertyValue::Struct(s) => s,
                    _ => unreachable!(),
                },
                m,
            )
        })
        .collect();
    for g in ranged_groups(&entries) {
        check_group(&g);
        if g.is_disjoint() {
            for level in [0i64, 10, 100] {
                let hits = g.members.iter().filter(|(_, r)| r.contains(level)).count();
                assert!(hits <= 1);
            }
        }
    }
    let applied: Vec<(ModOp, PropertyValue)> = entries
        .iter()
        .filter_map(|e| Some((e.operator(None)?, e.value.clone()?)))
        .collect();
    let _ = sandbox_combine(&PropertyValue::Int(min.into()), &applied);
    let _ = sandbox_combine(&PropertyValue::Float(1.0), &applied);
    let _ = sandbox_combine(&PropertyValue::Bitfield32(head), &applied);

    for &w in &words {
        let cell = TraitCell::from_key(w);
        assert_eq!(cell.key(), w);
    }
}

fuzz_target!(|input: &[u8]| {
    let [mode, rest @ ..] = input else {
        return;
    };
    let pids = pids();
    if mode % 2 == 0 {
        decoded_mode(rest, pids);
    } else {
        synthetic_mode(rest, pids);
    }
});
