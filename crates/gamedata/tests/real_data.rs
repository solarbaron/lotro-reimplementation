// SPDX-License-Identifier: GPL-3.0-or-later
//! Real-data test of the record-family index and selector resolution against a user's own
//! install, read-only.
//!
//! Skipped (reported as passed) unless `TESSERA_GAME_DIR` points at an install. Builds the
//! index, resolves every selector form of `scenario-feed.md` §8 (every family with `all`,
//! `n=N` and `n=N seed=S`; every attribute of §8.3 with each value type; labels next to
//! the numeric values they name; bindings; `did`; the table families), checks that a
//! label filter gives the same records as its numeric value, and that two independent
//! stores give identical DID lists. Prints counts only, never game text. Values and labels
//! are taken from the install at run time; nothing here names a record. A debug build
//! takes minutes; use `--release`.

use std::collections::BTreeMap;
use std::path::PathBuf;
use std::time::Instant;

use tessera_formats::gameplay::common::{Pid, PropertyAccess, enum_mapper_of};
use tessera_formats::{Did, PropertyValue};
use tessera_gamedata::{
    Alt, AttrType, Attribute, Count, Family, FilterValue, GameDataStore, InstallSource,
    LabelContext, MAX_DISPLAY_NAME, ResolveNote, SelectorQuery, attributes,
};

tessera_formats::property_ids! {
    /// The properties this test reads values and labels from (`scenario-feed.md` §8.3,
    /// §8.6 `enum_value`).
    struct SelectorPids {
        default_slot = "Inventory_DefaultSlot",
        compatible_slot = "Inventory_CompatibleSlot",
        item_class = "Item_Class",
        item_quality = "Item_Quality",
        required_classes = "Usage_RequiredClassList",
        class_type = "Usage_ClassType",
        skill_type = "Skill_SkillType",
        trait_nature = "Trait_Nature",
        adv_class = "AdvTable_Class",
        species = "Agent_Species",
        genus = "Agent_Genus",
        alignment = "Agent_Alignment",
        agent_class = "Agent_Class",
        quest_category = "Quest_Category",
        recipe_tier = "CraftRecipe_Tier",
        map_note_type = "MapNote_Type",
        race = "RaceTable_Race",
        gender = "RaceTable_GenderType",
        damage_type = "Effect_DamageType",
    }
}

fn game_dir() -> Option<PathBuf> {
    let dir = PathBuf::from(std::env::var_os("TESSERA_GAME_DIR")?);
    dir.is_dir().then_some(dir)
}

type Store = GameDataStore<InstallSource>;

/// Named selectors and the (value, label) index pairs among them.
type Forms = (Vec<(String, SelectorQuery)>, Vec<(usize, usize)>);

/// Records looked at when picking a value for a filter.
const PICK_SCAN: usize = 4000;

/// The first value `read` finds in the members of `family`.
fn pick(
    s: &Store,
    family: Family,
    read: impl Fn(&tessera_formats::PropertySet) -> Option<u32>,
) -> Option<u32> {
    let members = s.family(family).ok()?;
    members.iter().take(PICK_SCAN).find_map(|&d| {
        let rec = s.object(d).ok().flatten()?;
        read(&rec.properties)
    })
}

/// The lowest set bit of a bitfield as its enum key (`property-system.md` §7.3).
fn first_bit_key(v: Option<u64>) -> Option<u32> {
    v.filter(|b| *b != 0).map(|b| b.trailing_zeros() + 1)
}

fn bits(set: &tessera_formats::PropertySet, pid: Pid) -> Option<u64> {
    match set.get_pid(pid)? {
        v @ (PropertyValue::Bitfield32(_)
        | PropertyValue::Bitfield64(_)
        | PropertyValue::BitfieldAny(_)) => v.as_bits(),
        _ => None,
    }
}

/// The mapper name of `key` for property `pid` (or its base chain), when the Label class of
/// §4.2 can spell it.
fn label_of(s: &Store, pid: Pid, key: u32) -> Option<String> {
    let data = s.object_store().game_data();
    let reg = data.registry().ok()?;
    let mut at = enum_mapper_of(reg.dictionary(), pid)?;
    for _ in 0..32 {
        let m = data.enum_mapper(at).ok()??;
        if let Some(n) = m.name(key) {
            return n
                .chars()
                .all(|c| c.is_ascii_alphanumeric() || c == '_')
                .then(|| format!("@{n}"));
        }
        if m.base.is_null() {
            return None;
        }
        at = m.base;
    }
    None
}

/// A value attribute's numeric value and (if spellable) its label.
struct Picked {
    value: u32,
    label: Option<String>,
}

#[allow(clippy::too_many_lines)]
fn value_picks(s: &Store, p: &SelectorPids) -> BTreeMap<(Family, Attribute), Picked> {
    let mut out = BTreeMap::new();
    let mut put = |f: Family, a: Attribute, label_pid: Pid, v: Option<u32>| {
        if let Some(value) = v {
            out.insert(
                (f, a),
                Picked {
                    value,
                    label: label_of(s, label_pid, value),
                },
            );
        }
    };
    put(
        Family::Item,
        Attribute::Slot,
        p.default_slot,
        pick(s, Family::Item, |x| first_bit_key(bits(x, p.default_slot))),
    );
    put(
        Family::Item,
        Attribute::Fits,
        p.compatible_slot,
        pick(s, Family::Item, |x| {
            first_bit_key(bits(x, p.compatible_slot))
        }),
    );
    put(
        Family::Item,
        Attribute::Category,
        p.item_class,
        pick(s, Family::Item, |x| x.enum_key(p.item_class)),
    );
    put(
        Family::Item,
        Attribute::Quality,
        p.item_quality,
        pick(s, Family::Item, |x| x.enum_key(p.item_quality)),
    );
    let class_of_list = |x: &tessera_formats::PropertySet| {
        x.elements(p.required_classes)
            .iter()
            .find_map(|e| match e.value {
                PropertyValue::Enum(k) => Some(k),
                _ => None,
            })
    };
    put(
        Family::Item,
        Attribute::Class,
        p.class_type,
        pick(s, Family::Item, class_of_list),
    );
    put(
        Family::Quest,
        Attribute::Class,
        p.class_type,
        pick(s, Family::Quest, class_of_list),
    );
    let adv_class = pick(s, Family::Class, |x| x.enum_key(p.adv_class));
    put(Family::Skill, Attribute::Class, p.adv_class, adv_class);
    put(Family::Trait, Attribute::Class, p.adv_class, adv_class);
    put(Family::Class, Attribute::Class, p.adv_class, adv_class);
    put(
        Family::Skill,
        Attribute::Type,
        p.skill_type,
        pick(s, Family::Skill, |x| first_bit_key(bits(x, p.skill_type))),
    );
    put(
        Family::Trait,
        Attribute::Nature,
        p.trait_nature,
        pick(s, Family::Trait, |x| x.enum_key(p.trait_nature)),
    );
    put(
        Family::Creature,
        Attribute::Species,
        p.species,
        pick(s, Family::Creature, |x| x.enum_key(p.species)),
    );
    put(
        Family::Creature,
        Attribute::Genus,
        p.genus,
        pick(s, Family::Creature, |x| first_bit_key(bits(x, p.genus))),
    );
    put(
        Family::Creature,
        Attribute::Alignment,
        p.alignment,
        pick(s, Family::Creature, |x| x.enum_key(p.alignment)),
    );
    put(
        Family::Creature,
        Attribute::AgentClass,
        p.agent_class,
        pick(s, Family::Creature, |x| x.enum_key(p.agent_class)),
    );
    put(
        Family::Quest,
        Attribute::Category,
        p.quest_category,
        pick(s, Family::Quest, |x| x.enum_key(p.quest_category)),
    );
    put(
        Family::Recipe,
        Attribute::Tier,
        p.recipe_tier,
        pick(s, Family::Recipe, |x| x.enum_key(p.recipe_tier)),
    );
    let note_type = s
        .map_notes()
        .and_then(|t| t.notes.iter().find_map(|n| n.type_keys().find(|&k| k > 1)));
    put(Family::MapNote, Attribute::Type, p.map_note_type, note_type);
    out
}

/// Every selector form, in a fixed order (bindings refer to earlier indices).
#[allow(clippy::too_many_lines)]
fn queries(s: &Store, p: &SelectorPids) -> Forms {
    let picks = value_picks(s, p);
    let mut qs: Vec<(String, SelectorQuery)> = Vec::new();
    let mut label_pairs = Vec::new();
    for f in Family::ALL {
        if f == Family::Did {
            continue;
        }
        qs.push((format!("{} all", f.keyword()), SelectorQuery::all(f)));
        qs.push((
            format!("{} n=5", f.keyword()),
            SelectorQuery::all(f).count(Count::First(5)),
        ));
        qs.push((
            format!("{} n=5 seed=7", f.keyword()),
            SelectorQuery::all(f).count(Count::Sample { n: 5, seed: 7 }),
        ));
        for &(a, ty) in attributes(f) {
            let name = format!("{} {}", f.keyword(), a.name());
            match ty {
                AttrType::Value => {
                    let Some(pk) = picks.get(&(f, a)) else {
                        qs.push((
                            format!("{name}=0"),
                            SelectorQuery::all(f)
                                .filter(a, FilterValue::Alternatives(vec![Alt::Value(0)])),
                        ));
                        continue;
                    };
                    qs.push((
                        format!("{name}=<value>"),
                        SelectorQuery::all(f)
                            .filter(a, FilterValue::Alternatives(vec![Alt::Value(pk.value)])),
                    ));
                    if let Some(l) = &pk.label {
                        qs.push((
                            format!("{name}=<label>"),
                            SelectorQuery::all(f)
                                .filter(a, FilterValue::Alternatives(vec![Alt::Label(l.clone())])),
                        ));
                        label_pairs.push((qs.len() - 2, qs.len() - 1));
                        qs.push((
                            format!("{name}=<label>|@NoSuchLabelX n=3 seed=11"),
                            SelectorQuery::all(f)
                                .filter(
                                    a,
                                    FilterValue::Alternatives(vec![
                                        Alt::Label(l.clone()),
                                        Alt::Label("@NoSuchLabelX".into()),
                                    ]),
                                )
                                .count(Count::Sample { n: 3, seed: 11 }),
                        ));
                    }
                }
                AttrType::Keyword(words) => {
                    for w in words {
                        qs.push((
                            format!("{name}={w}"),
                            SelectorQuery::all(f)
                                .filter(a, FilterValue::Keywords(vec![(*w).to_string()])),
                        ));
                    }
                }
                AttrType::Range => {
                    for (lo, hi) in [(Some(1), Some(50)), (None, Some(20)), (Some(100), None)] {
                        qs.push((
                            format!("{name}={lo:?}..{hi:?}"),
                            SelectorQuery::all(f).filter(a, FilterValue::Range { lo, hi }),
                        ));
                    }
                }
                AttrType::Integer => {
                    for r in [1i64, 2, 14] {
                        qs.push((
                            format!("{name}={r}"),
                            SelectorQuery::all(f).filter(a, FilterValue::Integers(vec![r])),
                        ));
                    }
                }
                AttrType::YesNo => {
                    for b in [true, false] {
                        qs.push((
                            format!("{name}={b}"),
                            SelectorQuery::all(f).filter(a, FilterValue::Bool(b)),
                        ));
                    }
                }
                AttrType::Binding(_) => {} // below
                _ => {}
            }
        }
    }
    // Bindings (each refers to the selector just before it).
    let mut bind = |label: &str, base: SelectorQuery, next: SelectorQuery| {
        qs.push((format!("{label} (bound)"), base));
        let i = qs.len() - 1;
        let mut next = next;
        for flt in &mut next.filters {
            if let FilterValue::Binding(b) = &mut flt.value {
                *b = i;
            }
        }
        qs.push((label.to_string(), next));
    };
    bind(
        "vendor-inventory of=$vendors",
        SelectorQuery::all(Family::Creature)
            .filter(Attribute::Vendor, FilterValue::Bool(true))
            .count(Count::Sample { n: 50, seed: 3 }),
        SelectorQuery::all(Family::VendorInventory).filter(Attribute::Of, FilterValue::Binding(0)),
    );
    bind(
        "item listed-by=$inventories",
        SelectorQuery::all(Family::VendorInventory).count(Count::Sample { n: 20, seed: 5 }),
        SelectorQuery::all(Family::Item).filter(Attribute::ListedBy, FilterValue::Binding(0)),
    );
    bind(
        "recipe profession=$professions",
        SelectorQuery::all(Family::Profession).count(Count::First(2)),
        SelectorQuery::all(Family::Recipe).filter(Attribute::Profession, FilterValue::Binding(0)),
    );
    bind(
        "item listed-by=$recipes",
        SelectorQuery::all(Family::Recipe).count(Count::Sample { n: 40, seed: 9 }),
        SelectorQuery::all(Family::Item).filter(Attribute::ListedBy, FilterValue::Binding(0)),
    );
    // The examples of §8.1.
    qs.push((
        "item kind=weapon n=3 seed=7".into(),
        SelectorQuery::all(Family::Item)
            .filter(
                Attribute::Kind,
                FilterValue::Keywords(vec!["weapon".into()]),
            )
            .count(Count::Sample { n: 3, seed: 7 }),
    ));
    if let Some(pk) = picks.get(&(Family::Skill, Attribute::Class))
        && let Some(l) = &pk.label
    {
        qs.push((
            "skill class=@Label n=5 seed=7".into(),
            SelectorQuery::all(Family::Skill)
                .filter(
                    Attribute::Class,
                    FilterValue::Alternatives(vec![Alt::Label(l.clone())]),
                )
                .count(Count::Sample { n: 5, seed: 7 }),
        ));
    }
    // `did` (local scenarios only): DIDs taken from the index at run time.
    let mut some: Vec<Did> = Vec::new();
    for f in [Family::Item, Family::Quest, Family::Skill] {
        some.extend(s.family(f).unwrap_or_default().into_iter().take(3));
    }
    some.push(Did(0x7000_0000));
    qs.push(("did <runtime list>".into(), SelectorQuery::dids(some)));
    (qs, label_pairs)
}

/// Resolves `list` in order with the earlier results as bindings. The forms together hold
/// more records than one scenario may (§8.6 item 4), so the total limit of
/// `resolve_all` does not apply here; each selector is resolved by itself.
fn resolve_each(s: &Store, list: &[SelectorQuery]) -> Vec<tessera_gamedata::Resolution> {
    let mut bound: Vec<Vec<tessera_gamedata::Candidate>> = Vec::new();
    let mut out = Vec::new();
    for q in list {
        let r = s.resolve(q, &bound).expect("resolves");
        bound.push(r.records.clone());
        out.push(r);
    }
    out
}

#[test]
fn every_selector_form_resolves_deterministically() {
    let Some(dir) = game_dir() else {
        eprintln!("TESSERA_GAME_DIR not set; skipping");
        return;
    };
    let t0 = Instant::now();
    let s = Store::open(&dir).expect("open install");
    let ix = s.index().expect("index");
    eprintln!(
        "gamedata: {} object records indexed in {:.1?}; {} failures {:?}",
        ix.object_records(),
        t0.elapsed(),
        ix.failures_total(),
        ix.failure_counts()
            .iter()
            .map(|(k, n)| (k.name(), *n))
            .collect::<Vec<_>>()
    );
    for f in Family::RECORD_FAMILIES {
        eprintln!("gamedata: family {:<20} {}", f.keyword(), ix.len(f));
    }
    let tf = s.table_failures();
    eprintln!(
        "gamedata: places {} entries, map notes {} rows, table failures {}",
        s.places().map_or(0, |t| t.places.len()),
        s.map_notes().map_or(0, |t| t.notes.len()),
        tf.len()
    );
    // Families every install of the observed shape has (decision 0024 §5 "every record").
    for f in [
        Family::Item,
        Family::Skill,
        Family::Effect,
        Family::Trait,
        Family::Class,
        Family::Creature,
        Family::Quest,
        Family::Recipe,
        Family::VendorInventory,
        Family::Area,
    ] {
        assert!(ix.len(f) > 0, "family {} is empty", f.keyword());
    }
    let reg = s.object_store().game_data().registry().expect("dictionary");
    let pids = SelectorPids::resolve(reg.dictionary());
    eprintln!("gamedata: selector labels undefined: {:?}", pids.missing());

    let t1 = Instant::now();
    let (qs, label_pairs) = queries(&s, &pids);
    let list: Vec<SelectorQuery> = qs.iter().map(|(_, q)| q.clone()).collect();
    let first = resolve_each(&s, &list);
    eprintln!(
        "gamedata: {} selectors resolved in {:.1?}",
        list.len(),
        t1.elapsed()
    );
    for ((name, q), r) in qs.iter().zip(&first) {
        let notes: Vec<String> = r
            .notes
            .iter()
            .map(|n| match n {
                ResolveNote::UnmatchedLabel { attribute, .. } => {
                    format!("unmatched-label({})", attribute.name())
                }
                other => format!("{other:?}"),
            })
            .collect();
        eprintln!(
            "gamedata: {name:<60} {:>7} {}",
            r.records.len(),
            notes.join(" ")
        );
        // Counts hold.
        match q.count {
            Count::First(n) | Count::Sample { n, .. } => {
                assert!(r.records.len() <= n as usize, "{name}");
            }
            Count::All => {}
        }
        // Canonical order (§8.4).
        assert!(
            r.records
                .windows(2)
                .all(|w| tessera_gamedata::canonical_cmp(&w[0], &w[1]).is_lt()),
            "{name}: not in canonical order"
        );
        if q.family.is_unsupported() {
            assert_eq!(r.notes, [ResolveNote::Unsupported], "{name}");
        }
    }
    // A label resolves to the records of its numeric value.
    for (a, b) in label_pairs {
        assert_eq!(
            first[a].records, first[b].records,
            "{} vs {}",
            qs[a].0, qs[b].0
        );
    }
    // Unknown labels are reported, the known alternative still matches.
    for ((name, _), r) in qs.iter().zip(&first) {
        if name.contains("NoSuchLabelX") {
            assert!(
                r.notes.iter().any(|n| matches!(n, ResolveNote::UnmatchedLabel { label, .. } if label == "@NoSuchLabelX")),
                "{name}"
            );
        }
    }
    // Same store, second run.
    let again = resolve_each(&s, &list);
    assert_eq!(
        first, again,
        "a second resolution on the same store differs"
    );
    // Independent store: identical DID lists.
    let t2 = Instant::now();
    let other = Store::open(&dir).expect("open install");
    let second = resolve_each(&other, &list);
    eprintln!(
        "gamedata: second store built and resolved in {:.1?}",
        t2.elapsed()
    );
    for (((name, _), a), b) in qs.iter().zip(&first).zip(&second) {
        assert_eq!(
            a.dids(),
            b.dids(),
            "{name}: two runs give different DID lists"
        );
        assert_eq!(a.records, b.records, "{name}");
    }
    assert_eq!(
        ix.dids(Family::Item),
        other.index().unwrap().dids(Family::Item)
    );
    eprintln!("gamedata: total {:.1?}", t0.elapsed());
}

/// A spellable label of the mapper of `pid` (the first key of the nearest mapper whose name
/// the Label class can spell).
fn some_label(s: &Store, pid: Pid) -> Option<(u32, String)> {
    let data = s.object_store().game_data();
    let reg = data.registry().ok()?;
    let at = enum_mapper_of(reg.dictionary(), pid)?;
    let m = data.enum_mapper(at).ok()??;
    m.names().iter().find_map(|(k, n)| {
        n.chars()
            .all(|c| c.is_ascii_alphanumeric() || c == '_')
            .then(|| (*k, format!("@{n}")))
    })
}

#[test]
fn enum_values_and_display_names() {
    let Some(dir) = game_dir() else {
        eprintln!("TESSERA_GAME_DIR not set; skipping");
        return;
    };
    let strings = tessera_text::Strings::open_install(&dir, None).expect("language archive");
    let s = Store::open(&dir)
        .expect("open install")
        .with_strings(strings);
    let reg = s.object_store().game_data().registry().expect("dictionary");
    let p = SelectorPids::resolve(reg.dictionary());
    // enum_value: a label read from each context's mapper resolves to its key.
    for (context, pid) in [
        (LabelContext::Class, p.adv_class),
        (LabelContext::Species, p.race),
        (LabelContext::Sex, p.gender),
        (LabelContext::EquipSlot, p.default_slot),
        (LabelContext::DamageType, p.damage_type),
    ] {
        let (key, label) = some_label(&s, pid).expect("a spellable label");
        assert_eq!(s.enum_value(context, &label), Some(key), "{context:?}");
        assert_eq!(s.enum_value(context, "@NoSuchLabelX"), None, "{context:?}");
    }
    let m = s.enum_value(LabelContext::Sex, "m");
    let f = s.enum_value(LabelContext::Sex, "f");
    eprintln!(
        "gamedata: sex m resolved {}, f resolved {}",
        m.is_some(),
        f.is_some()
    );
    assert!(
        m.is_some() && f.is_some(),
        "the race tables hold both genders"
    );
    // display_name: counts per family over a sample (no text printed).
    let ix = s.index().expect("index");
    for family in Family::RECORD_FAMILIES {
        let members = ix.dids(family);
        let step = (members.len() / 200).max(1);
        let sample: Vec<Did> = members.iter().step_by(step).take(200).copied().collect();
        let mut named = 0usize;
        for &d in &sample {
            if let Some(n) = s.display_name(d, None) {
                assert!(n.len() <= MAX_DISPLAY_NAME);
                assert!(!n.chars().any(char::is_control));
                named += 1;
            }
        }
        eprintln!(
            "gamedata: display names {:<20} {named} / {}",
            family.keyword(),
            sample.len()
        );
        if matches!(
            family,
            Family::Item
                | Family::Skill
                | Family::Trait
                | Family::Creature
                | Family::Quest
                | Family::TravelDestination
                | Family::Faction
                | Family::Profession
                | Family::Instance
        ) && !sample.is_empty()
        {
            assert!(named > 0, "no {} has a display name", family.keyword());
        }
    }
    // Table elements: places by their entry name, map notes by their entity.
    let places = s.places().map_or(0, |t| t.places.len());
    let named_places = (0..places.min(500))
        .filter(|&i| {
            let t = s.places().unwrap();
            s.display_name(t.places[i].area, Some(u32::try_from(i).unwrap()))
                .is_some()
        })
        .count();
    let notes = s.map_notes().map_or(0, |t| t.notes.len());
    let named_notes = (0..notes.min(500))
        .filter(|&i| {
            let t = s.map_notes().unwrap();
            s.display_name(t.notes[i].base.entity, Some(u32::try_from(i).unwrap()))
                .is_some()
        })
        .count();
    eprintln!(
        "gamedata: display names place {named_places} / {}, map-note {named_notes} / {}",
        places.min(500),
        notes.min(500)
    );
    assert!(places == 0 || named_places > 0);
}
