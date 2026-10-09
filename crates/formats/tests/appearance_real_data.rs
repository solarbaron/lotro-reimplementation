// SPDX-License-Identifier: GPL-3.0-or-later
//! `appearance.md` and `setup.md` part 2 against a user's own install (read-only):
//!
//! - appearance records: the key table equals the selection counts (AI2), the float after
//!   the last group is 0 (AI3), priorities are whole numbers 0–100 (AI5); entries without a
//!   base block (§4.11);
//! - appearance lists of all 0x47 entities: block choice by rule 1 / rules 2–3 / key missing
//!   / null DID (§3.2.1 census);
//! - worn items of all item property sets: map structs resolving to parts / material only /
//!   key missing (§4.5, §10), distinct wardrobe records (§4.2), dye blocks (§4.9), held
//!   items' slots and their location on the player setups (§5.2, §5.3);
//! - setups: `attach_map`, list A/B/C presence and their uniqueness invariants (§14);
//! - the 14 playable bodies: race table → setup, scales, appearance lists, slider records
//!   (§2.2, §3.3, §6).
//!
//! Failures are categorised as `appearance.md` §10 names them. Skipped unless
//! `TESSERA_GAME_DIR` is set; release builds visit every record. Run with
//! `cargo test --release -p tessera-formats --test appearance_real_data -- --nocapture`.
//! Only counts, DIDs and categories are printed.

use std::collections::{BTreeMap, BTreeSet, HashMap};
use std::path::PathBuf;
use std::sync::atomic::{AtomicUsize, Ordering};
use std::sync::{Mutex, OnceLock};

use tessera_dat::{ArchiveSet, Entry, SetOptions};
use tessera_formats::avatar::{
    AvatarPids, BoneSliderDefinition, BoneSliderPids, RacePids, RaceTable, parse_slider_values,
    resolve_avatar_body,
};
use tessera_formats::item::{HoldingRule, ItemLook, ItemPids, ItemVisual, WardrobeFallbacks};
use tessera_formats::setup::Anchor;
use tessera_formats::{
    AppearanceListPids, AppearanceRecord, DataIdMap, Did, EntityDescriptor, EnumMapper,
    PropertyDictionary, PropertySetRecord, Record, Setup,
};

const PROPERTY_DICTIONARY: u32 = 0x3400_0000;
const ROOT_DID_MAP: u32 = 0x2800_0000;

fn open_set() -> Option<&'static ArchiveSet> {
    static SET: OnceLock<Option<ArchiveSet>> = OnceLock::new();
    SET.get_or_init(|| {
        let dir = match std::env::var_os("TESSERA_GAME_DIR") {
            Some(d) if !d.is_empty() => PathBuf::from(d),
            _ => {
                eprintln!("TESSERA_GAME_DIR not set; skipping");
                return None;
            }
        };
        Some(ArchiveSet::open(&dir, SetOptions::default()).expect("open archive set"))
    })
    .as_ref()
}

fn dictionary(set: &ArchiveSet) -> &'static PropertyDictionary {
    static DICT: OnceLock<PropertyDictionary> = OnceLock::new();
    DICT.get_or_init(|| {
        let payload = set.read(PROPERTY_DICTIONARY).expect("read the dictionary");
        PropertyDictionary::parse_expecting(&payload, PROPERTY_DICTIONARY).expect("dictionary")
    })
}

fn exhaustive() -> bool {
    !cfg!(debug_assertions) || std::env::var_os("TESSERA_FULL").is_some_and(|v| v == "1")
}

fn par_each<T: Sync, R: Send>(items: &[T], f: impl Fn(&T) -> R + Sync) -> Vec<R> {
    let next = AtomicUsize::new(0);
    let out = Mutex::new(Vec::with_capacity(items.len()));
    let threads = std::thread::available_parallelism().map_or(4, |n| n.get());
    std::thread::scope(|s| {
        for _ in 0..threads {
            s.spawn(|| {
                let mut local = Vec::new();
                loop {
                    let i = next.fetch_add(64, Ordering::Relaxed);
                    if i >= items.len() {
                        break;
                    }
                    for item in &items[i..(i + 64).min(items.len())] {
                        local.push(f(item));
                    }
                }
                out.lock().unwrap().extend(local);
            });
        }
    });
    out.into_inner().unwrap()
}

fn entries(set: &ArchiveSet, lo: u32, hi: u32) -> Vec<(usize, Entry)> {
    let mut out = Vec::new();
    for (i, m) in set.members().iter().enumerate() {
        out.extend(
            m.archive
                .entries_range(lo..=hi)
                .map(|e| (i, e.expect("directory walk"))),
        );
    }
    let full = exhaustive();
    out.into_iter()
        .enumerate()
        .filter(|(i, _)| full || i % 23 == 0)
        .map(|(_, e)| e)
        .collect()
}

fn fetch(
    set: &'static ArchiveSet,
) -> impl FnMut(Did) -> Result<Option<Vec<u8>>, tessera_dat::Error> {
    move |d: Did| match set.locate(d.0)? {
        Some(l) => set.read_located(&l).map(Some),
        None => Ok(None),
    }
}

/// Every appearance record, decoded once.
fn appearances(set: &'static ArchiveSet) -> &'static HashMap<Did, AppearanceRecord> {
    static RECS: OnceLock<HashMap<Did, AppearanceRecord>> = OnceLock::new();
    RECS.get_or_init(|| {
        let mut all = Vec::new();
        for (i, m) in set.members().iter().enumerate() {
            all.extend(
                m.archive
                    .entries_range(0x2000_0000..=0x20FF_FFFF)
                    .map(|e| (i, e.expect("directory walk"))),
            );
        }
        par_each(&all, |(i, e)| {
            let p = set.member(*i).archive.read_entry(e).ok()?;
            AppearanceRecord::parse_expecting(&p, e.did)
                .ok()
                .map(|a| (a.did, a))
        })
        .into_iter()
        .flatten()
        .collect()
    })
}

/// The key named `label` in the enum mapper that `property`'s definition names, searching
/// base mappers.
fn enum_key(set: &ArchiveSet, property: &str, label: &str) -> Option<u32> {
    let dict = dictionary(set);
    EnumMapper::key_in_chain_from(
        dict.definition(dict.pid(property)?)?.data,
        |did: Did| EnumMapper::parse_expecting(&set.read(did.0).ok()?, did).ok(),
        label,
    )
}

#[test]
fn appearance_records_follow_part_two() {
    let Some(set) = open_set() else { return };
    let recs = appearances(set);
    let (mut entries, mut no_base, mut key_ok, mut ai3) = (0, 0, 0, 0);
    let mut part_prio: BTreeMap<i32, usize> = BTreeMap::new();
    let mut group_prio: BTreeMap<i32, usize> = BTreeMap::new();
    let mut unusual: BTreeMap<&str, usize> = BTreeMap::new();
    for a in recs.values() {
        if let Some(why) = a.first_unusual_field() {
            *unusual.entry(why).or_default() += 1;
        }
        key_ok += usize::from(a.key_table_matches_selection_counts());
        for e in &a.entries {
            entries += 1;
            no_base += usize::from(e.base_block().is_none());
            for b in &e.blocks {
                ai3 += usize::from(b.groups_end() == 0.0);
                for p in &b.parts {
                    *part_prio.entry(p.priority() as i32).or_default() += 1;
                }
                for (p, _) in b.groups_with_priority() {
                    *group_prio.entry(p as i32).or_default() += 1;
                }
            }
        }
    }
    let blocks: usize = recs
        .values()
        .flat_map(|a| &a.entries)
        .map(|e| e.blocks.len())
        .sum();
    eprintln!(
        "{} records, {entries} entries ({no_base} without a base block), {blocks} blocks; key \
         table = selection count in {key_ok} records (AI2); groups_end 0.0 in {ai3} blocks (AI3)",
        recs.len()
    );
    eprintln!("  part priorities: {part_prio:?}");
    eprintln!("  group priorities: {group_prio:?}");
    for (why, n) in &unusual {
        eprintln!("  unusual: {n} × {why}");
    }
    assert!(!recs.is_empty());
    assert_eq!(key_ok, recs.len(), "AI2 holds in every record");
    assert_eq!(ai3, blocks, "AI3 holds in every block");
    assert!(
        unusual.is_empty(),
        "AI2, AI3, AI5 and the prefixes hold everywhere"
    );
}

#[test]
fn appearance_lists_choose_blocks() {
    let Some(set) = open_set() else { return };
    let dict = dictionary(set);
    let recs = appearances(set);
    let pids = AppearanceListPids::from_names(dict).expect("appearance-list names");
    let work = entries(set, 0x4700_0000, 0x47FF_FFFF);
    let rows = par_each(&work, |(i, e)| {
        let p = set.member(*i).archive.read_entry(e).ok()?;
        let d = EntityDescriptor::parse_expecting(&p, e.did, dict).ok()?;
        Some(pids.read_lists(&d.properties))
    });
    let mut cats: BTreeMap<&str, usize> = BTreeMap::new();
    let mut entities = 0;
    for list in rows.into_iter().flatten() {
        entities += usize::from(!list.is_empty());
        for el in list {
            let cat = if el.file.is_null() {
                "null DID"
            } else if let Some(r) = recs.get(&el.file) {
                // §3.2.1 (A19) and the AI12 census categories.
                match r.entry(el.key) {
                    None => "key missing",
                    Some(e) => match e.block_for_modifier(el.modifier) {
                        Some(b) if b.variant == el.modifier => "exact",
                        Some(b) => {
                            assert!(b.variant < el.modifier, "a lower block");
                            "lower block (rule 2)"
                        }
                        None if el.modifier == -1.0 => "none (rule 1)",
                        None => {
                            assert!(e.blocks.first().is_none_or(|f| f.variant > el.modifier));
                            "none (rule 3)"
                        }
                    },
                }
            } else {
                "record missing (AI8)"
            };
            *cats.entry(cat).or_default() += 1;
        }
    }
    let total: usize = cats.values().sum();
    eprintln!("appearance lists: {entities} entities, {total} elements: {cats:?}");
    assert!(total > 0);
    assert_eq!(cats.get("record missing (AI8)"), None, "AI8");
}

#[test]
fn worn_and_held_items_resolve() {
    let Some(set) = open_set() else { return };
    let dict = dictionary(set);
    let recs = appearances(set);
    let pids = ItemPids::from_names(dict).expect("item names");
    let work = entries(set, 0x7900_0000, 0x79FF_FFFF);
    let items: Vec<ItemVisual> = par_each(&work, |(i, e)| {
        let p = set.member(*i).archive.read_entry(e).ok()?;
        let r = PropertySetRecord::parse_expecting(&p, e.did, dict).ok()?;
        let v = ItemVisual::from_property_set(&r.set, &pids);
        (!v.worn.is_empty() || v.phys_obj.is_some()).then_some(v)
    })
    .into_iter()
    .flatten()
    .collect();
    let dye_key = enum_key(set, "Item_AppearanceKey", "ClothingColor");
    let (mut structs, mut worn_items) = (0, 0);
    let mut cats: BTreeMap<&str, usize> = BTreeMap::new();
    let mut records = BTreeSet::new();
    let (mut dyed, mut dye_exact, mut dyed_nonzero) = (0, 0, 0);
    for it in &items {
        if it.worn.is_empty() {
            continue;
        }
        worn_items += 1;
        for w in &it.worn {
            structs += 1;
            records.insert(w.appearance);
            let cat = match recs.get(&w.appearance).and_then(|r| r.entry(w.key)) {
                None if recs.contains_key(&w.appearance) => "key missing",
                None => "record missing",
                Some(e) => match e.base_block() {
                    Some(b) if !b.parts.is_empty() => "parts",
                    Some(_) => "material only",
                    None => "no base block",
                },
            };
            *cats.entry(cat).or_default() += 1;
        }
        // §4.9: the default dye code selects a block of the same record's dye table.
        if let (Some(code), Some(dk), Some(w)) = (it.clothing_color, dye_key, it.worn.first())
            && let Some(e) = recs.get(&w.appearance).and_then(|r| r.entry(dk))
        {
            dyed += 1;
            dyed_nonzero += usize::from(code != 0.0);
            dye_exact += usize::from(e.blocks.iter().any(|b| b.variant == code));
            assert!(e.dye_block(code).is_some());
        }
    }
    eprintln!(
        "worn items: {worn_items} items, {structs} map structs, {} distinct records: {cats:?}",
        records.len()
    );
    eprintln!(
        "  items with a dye code and a dye table: {dyed} ({dyed_nonzero} non-zero); exact dye \
         block for {dye_exact}, {} without (§4.9)",
        dyed - dye_exact
    );
    assert!(worn_items > 0);
    assert_eq!(cats.get("record missing"), None, "AI1");
    assert_eq!(
        cats.get("no base block"),
        None,
        "AI1: every entry found has a 1.0 block"
    );

    // Held items (§5.2, §5.3): default slots and locations on the Human M setup.
    let fallbacks = WardrobeFallbacks::from_species_labels(|l| enum_key(set, "RaceTable_Race", l));
    eprintln!("  wardrobe fallbacks: {:x?}", fallbacks.pairs);
    assert_eq!(fallbacks.pairs.len(), 4, "the four species labels of §2.3");
    let human = enum_key(set, "RaceTable_Race", "Human");
    let slot_bit = |label: &str| {
        enum_key(set, "Inventory_DefaultSlot", label).map(|k| 1u64 << (k.saturating_sub(1)))
    };
    let loc = |label: &str| enum_key(set, "Inventory_ParentingLocation", label);
    let mut rule = HoldingRule::default();
    for (slot, location) in [
        ("Weapon_Primary", "Wpn_RHand"),
        ("Weapon_Secondary", "Wpn_LOffHand"),
        ("Weapon_Ranged", "Sheath_Back_Bow"),
    ] {
        if let (Some(s), Some(l)) = (slot_bit(slot), loc(location)) {
            rule.slot_locations.push((s, l));
        }
    }
    eprintln!(
        "  paperdoll rule (slot bits, location): {:x?}",
        rule.slot_locations
    );
    // The Human male body's setup, through the race table.
    let race = race_table(set);
    let body = human
        .and_then(|h| race.entry(h, 0x1000))
        .map(|e| {
            let pids = AvatarPids::from_names(dict).unwrap();
            resolve_avatar_body(e.player_avatar, dict, &pids, fetch(set)).expect("body")
        })
        .expect("a Human male in the race table");
    let human_setup = Setup::parse_expecting(&set.read(body.setup.0).unwrap(), body.setup);
    let mut held: BTreeMap<String, (usize, usize)> = BTreeMap::new();
    for it in &items {
        let look = it.look(human.unwrap_or(0), 0x1000, &fallbacks);
        if let ItemLook::Held { .. } = look {
            let slot = it.default_slot.map_or("none".into(), |s| format!("{s:#x}"));
            let e = held.entry(slot).or_default();
            e.0 += 1;
            if let (Ok(s), Some(l)) = (&human_setup, rule.paperdoll_location(it)) {
                e.1 += usize::from(s.holding_location(l).is_some());
            }
        }
    }
    eprintln!(
        "  held on a Human M (slot: items, location found on setup {}): {held:?}",
        body.setup
    );
}

#[test]
fn setups_carry_part_two_tables() {
    let Some(set) = open_set() else { return };
    let work = entries(set, 0x0400_0000, 0x04FF_FFFF);
    let rows = par_each(&work, |(i, e)| {
        let p = set.member(*i).archive.read_entry(e).ok()?;
        let s = Setup::parse_expecting(&p, e.did).ok()?;
        let max_bone = s
            .attachments()
            .map(|a| a.bone)
            .chain(s.holding_locations().filter_map(|h| match h.anchor {
                Anchor::Bone(b) => Some(b),
                Anchor::Origin => None,
            }))
            .max();
        Some((
            !s.attach_map.is_empty(),
            !s.bone_frames.is_empty(),
            !s.placements.is_empty(),
            !s.named_frames.is_empty(),
            s.holding_locations().count() == s.bone_frames.len(),
            max_bone,
            s.first_unusual_field(),
        ))
    });
    let mut counts = [0usize; 5];
    let mut unusual: BTreeMap<&str, usize> = BTreeMap::new();
    let mut max_bone = 0;
    for r in rows.into_iter().flatten() {
        counts[0] += 1;
        counts[1] += usize::from(r.0);
        counts[2] += usize::from(r.1);
        counts[3] += usize::from(r.2);
        counts[4] += usize::from(r.3);
        assert!(r.4, "every list A bone is -1 or an index (SI8)");
        max_bone = max_bone.max(r.5.unwrap_or(0));
        if let Some(why) = r.6 {
            *unusual.entry(why).or_default() += 1;
        }
    }
    eprintln!(
        "setups: {} decoded; attach_map in {}, list A in {}, list B in {}, list C in {}; \
         largest bone index named {max_bone}",
        counts[0], counts[1], counts[2], counts[3], counts[4]
    );
    for (why, n) in &unusual {
        eprintln!("  unusual: {n} × {why}");
    }
    assert!(max_bone < 256, "attach values are below 256 (§6.2)");
    assert!(
        unusual.is_empty(),
        "SI7/SI8 key and value uniqueness hold everywhere"
    );
}

fn race_table(set: &'static ArchiveSet) -> RaceTable {
    let dict = dictionary(set);
    let root = DataIdMap::parse_expecting(&set.read(ROOT_DID_MAP).unwrap(), ROOT_DID_MAP).unwrap();
    let weenie = root.by_label("WEENIECONTENT").expect("WEENIECONTENT");
    let weenie = DataIdMap::parse_expecting(&set.read(weenie.0).unwrap(), weenie).unwrap();
    let dir = weenie
        .by_label("RaceTableDirectory")
        .expect("RaceTableDirectory");
    RaceTable::read(dir, dict, &RacePids::from_names(dict).unwrap(), fetch(set))
        .expect("race table")
}

#[test]
fn playable_bodies_resolve() {
    let Some(set) = open_set() else { return };
    let dict = dictionary(set);
    let race = race_table(set);
    let avatars = race.avatars();
    eprintln!(
        "race table: {} entries, {} distinct avatars",
        race.entries.len(),
        avatars.len()
    );
    assert!(avatars.len() >= 14, "the exit-criterion list of §2.2");
    let pids = AvatarPids::from_names(dict).unwrap();
    let slider_pids = BoneSliderPids::from_names(dict).unwrap();
    let recs = appearances(set);
    for e in &race.entries {
        let b = resolve_avatar_body(e.player_avatar, dict, &pids, fetch(set)).expect("body");
        let files: Vec<String> = b
            .appearance_files()
            .iter()
            .map(|(k, f)| format!("{:#x}:{f}", k.key))
            .collect();
        let mut sliders = String::new();
        if let Some(d) = b.bone_slider_definitions {
            let r = PropertySetRecord::parse_expecting(&set.read(d.0).unwrap(), d, dict).unwrap();
            let defs = BoneSliderDefinition::read_all(&r.set, &slider_pids);
            let visible = defs.iter().filter(|d| d.visible).count();
            let bytes = b.slider_values.as_deref().and_then(parse_slider_values);
            sliders = format!(
                " sliders {d}: {} ({visible} visible), value bytes {:?}",
                defs.len(),
                bytes.as_ref().map(Vec::len)
            );
            if let Some(bytes) = bytes {
                assert_eq!(bytes.len(), visible, "one byte per visible slider (AI7)");
            }
            assert!(defs.iter().all(|d| d.bones.iter().all(|b| b.index >= 0)));
        }
        eprintln!(
            "  species {:#x} sex {:#06x}: entity {} setup {} scale {:.3} (template {:?}, entity \
             {:?}) appearance [{}]{sliders}",
            e.species,
            e.sex,
            b.entity,
            b.setup,
            b.model_scale()[2],
            b.template_scale,
            b.entity_scale,
            files.join(" ")
        );
        assert_eq!(b.setup.prefix(), 0x04);
        for (_, f) in b.appearance_files() {
            assert!(recs.contains_key(&f), "AI8: {f}");
        }
    }
}
