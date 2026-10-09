// SPDX-License-Identifier: GPL-3.0-or-later
//! Real-data checks of the item views and rules (`tessera_formats::gameplay::items`)
//! against a user's own install, read-only.
//!
//! Skipped unless `TESSERA_GAME_DIR` points at an install. Every object record is read: the
//! headers and property sets of all records find the item classes (§2.4) and check the
//! entity stamps (IT2); then every item and item-like record is checked against the
//! invariants of `docs/specs/items.md` §12 (IT1–IT13; IT12 decodes the object-state record
//! of every item with a worn-look list), and every item class against the kinds of §2.1.
//! Counts are reported, never asserted. Both passes run on several threads; a debug build
//! still takes minutes, a `--release` build well under one.

use std::collections::{BTreeMap, BTreeSet, HashMap, HashSet};
use std::path::PathBuf;
use std::time::Instant;

use tessera_formats::gameplay::common::{
    ClassIndex, ClassParents, ObjectRoot, Pid, PropertyAccess, PropertyDefaults,
    decode_object_properties, did_references, enum_mapper_of,
};
use tessera_formats::gameplay::items::{
    COPY_LABELS, DpsTable, DurationMap, EffectListKind, ItemClassKind, ItemClasses, ItemKind,
    ItemSchema, ItemSetView, ItemView, ItemWeenieKinds, LegacyGroup, LegendaryTitleView,
    Progression, SetMembership, ValueTable, WeaponStats, WeaponTimingTable, WeaponTimings,
    WearControl, armour_value, control_labels, entity_labels, examination_list, item_references,
    item_value, legacy_effects, merge_list, mod_entry_is_well_formed, set_directory, weapon_damage,
    weapon_timing_table_dids,
};
use tessera_formats::item::{ItemPids as VisualPids, ItemVisual};
use tessera_formats::{Did, EnumMapper, PropertyDictionary, PropertySet, PropertyValue};
use tessera_wstate::{ObjectStore, object_dids};

/// Name hash of the root attribute that holds the worn-appearance mirror (`items.md` §3.3).
const WORN_MAP_ATTRIBUTE: u32 = 0x0905_B9D8;

fn game_dir() -> Option<PathBuf> {
    let dir = PathBuf::from(std::env::var_os("TESSERA_GAME_DIR")?);
    dir.is_dir().then_some(dir)
}

/// Runs `f` on chunks of `items` on up to eight threads and returns the per-chunk results.
fn par_chunks<T: Sync, R: Send>(items: &[T], f: impl Fn(&[T]) -> R + Sync) -> Vec<R> {
    let threads = std::thread::available_parallelism()
        .map_or(4, std::num::NonZero::get)
        .min(8);
    let chunk = items.len().div_ceil(threads).max(1);
    std::thread::scope(|s| {
        let f = &f;
        let handles: Vec<_> = items.chunks(chunk).map(|c| s.spawn(move || f(c))).collect();
        handles
            .into_iter()
            .map(|h| h.join().expect("worker thread"))
            .collect()
    })
}

/// Property sets of game objects, decoded with this crate's decoder.
struct Sets<'a> {
    store: &'a ObjectStore,
    dict: &'a PropertyDictionary,
}

impl Sets<'_> {
    /// The property set of object `did`; `None` when it has none. Panics on read or decode
    /// failures.
    fn props(&self, did: Did) -> Option<PropertySet> {
        let set_did = ObjectRoot { did, root_class: 0 }.property_set_did()?;
        let bytes = self.store.game_data().read(set_did).expect("read set")?;
        Some(
            decode_object_properties(did, &bytes, self.dict)
                .unwrap_or_else(|e| panic!("{did}: {e}")),
        )
    }
}

/// Loads and converts a table record once per DID; `None` when it is absent or does not
/// convert.
struct TableCache<T> {
    map: HashMap<Did, Option<T>>,
}

impl<T> TableCache<T> {
    fn new() -> Self {
        Self {
            map: HashMap::new(),
        }
    }

    fn get(
        &mut self,
        src: &Sets<'_>,
        did: Did,
        f: impl FnOnce(&PropertySet) -> Option<T>,
    ) -> Option<&T> {
        self.map
            .entry(did)
            .or_insert_with(|| f(&src.props(did)?))
            .as_ref()
    }
}

// ---- Pass 1 ----

struct Pass1 {
    roots: Vec<ObjectRoot>,
    references: Vec<Did>,
    with_stamps: Vec<Did>,
    partial_stamps: Vec<Did>,
    stamp_values: Vec<Option<PropertyValue>>,
    stamp_conflicts: usize,
    failures: Vec<(Did, String)>,
}

impl Pass1 {
    fn new(labels: usize) -> Self {
        Self {
            roots: Vec::new(),
            references: Vec::new(),
            with_stamps: Vec::new(),
            partial_stamps: Vec::new(),
            stamp_values: vec![None; labels],
            stamp_conflicts: 0,
            failures: Vec::new(),
        }
    }

    fn note_value(&mut self, slot: usize, v: Option<PropertyValue>) {
        if let Some(s) = self.stamp_values.get_mut(slot) {
            match s {
                None => *s = v,
                Some(prev) if Some(&*prev) != v.as_ref() => self.stamp_conflicts += 1,
                Some(_) => {}
            }
        }
    }

    fn merge(&mut self, other: Self) {
        self.roots.extend(other.roots);
        self.references.extend(other.references);
        self.with_stamps.extend(other.with_stamps);
        self.partial_stamps.extend(other.partial_stamps);
        self.stamp_conflicts += other.stamp_conflicts;
        self.failures.extend(other.failures);
        for (i, v) in other.stamp_values.into_iter().enumerate() {
            if v.is_some() {
                self.note_value(i, v);
            }
        }
    }
}

fn pass1(sets: &Sets<'_>, schema: &ItemSchema, stamp_pids: &[Pid], dids: &[Did]) -> Pass1 {
    let data = sets.store.game_data();
    let mut out = Pass1::new(stamp_pids.len());
    for &did in dids {
        let payload = data.read(did).expect("read").expect("listed record exists");
        let root = ObjectRoot::read_expecting(&payload, did).expect("object root");
        out.roots.push(root);
        let Some(set_did) = root.property_set_did() else {
            continue;
        };
        let Some(set_bytes) = data.read(set_did).expect("read set") else {
            continue;
        };
        let set = match decode_object_properties(did, &set_bytes, sets.dict) {
            Ok(s) => s,
            Err(e) => {
                out.failures.push((did, e.to_string()));
                continue;
            }
        };
        out.references
            .extend(item_references(&set, &schema.references));
        let present = stamp_pids.iter().filter(|&&p| set.has(p)).count();
        if present == stamp_pids.len() {
            out.with_stamps.push(did);
            for (i, &p) in stamp_pids.iter().enumerate() {
                out.note_value(i, set.get_pid(p).cloned());
            }
        } else if present > 0 {
            out.partial_stamps.push(did);
        }
    }
    out
}

// ---- Pass 2 ----

/// Shared inputs of the per-item checks.
struct Ctx<'a> {
    sets: &'a Sets<'a>,
    schema: &'a ItemSchema,
    wear: &'a WearControl,
    timings: &'a WeaponTimings,
    cooldowns: &'a DurationMap,
    decays: &'a DurationMap,
    copy_pids: &'a [(Pid, Pid, &'static str)],
    weenie: &'a ItemWeenieKinds,
    visual: &'a VisualPids,
}

#[derive(Default)]
struct Census {
    items: usize,
    item_like: usize,
    per_class: BTreeMap<u32, usize>,
    kinds: BTreeMap<ItemKind, BTreeMap<u32, usize>>,
    signature_missing: Vec<Did>,
    copy_checks: usize,
    copy_mismatches: Vec<(Did, &'static str)>,
    value_checked: usize,
    value_null_table: usize,
    value_mismatch: Vec<Did>,
    value_bad_table: usize,
    value_tables: HashSet<Did>,
    weapons: usize,
    dps_mismatch: Vec<Did>,
    damage_mismatch: Vec<Did>,
    dps_tables: HashSet<Did>,
    armour_checked: usize,
    armour_mismatch: Vec<Did>,
    armour_without_table_nonzero: usize,
    armour_tables: HashSet<Did>,
    msp_checked: usize,
    msp_mismatch: Vec<Did>,
    durability_checked: usize,
    durability_mismatch: Vec<Did>,
    timing_checked: usize,
    timing_mismatch: Vec<Did>,
    did_refs: usize,
    did_targets: HashSet<Did>,
    did_missing: Vec<(Did, Did)>,
    mod_entries: usize,
    mod_malformed: Vec<Did>,
    mod_other_fields: usize,
    sockets: usize,
    socket_malformed: Vec<Did>,
    effect_structs: BTreeMap<EffectListKind, usize>,
    cooldown_uncovered: Vec<(Did, u32)>,
    decay_uncovered: Vec<(Did, u32)>,
    behaviour_targets: HashSet<Did>,
    property_set_members: Vec<(Did, Did)>,
    bindings: BTreeMap<String, usize>,
    worn_lists: BTreeMap<u32, usize>,
    worn_mirrors: BTreeMap<u32, usize>,
    worn_mirror_mismatch: Vec<Did>,
    worn_list_unread: Vec<Did>,
    /// F47: (requirement holders, race entries, of them not bare enums).
    races: (usize, usize, usize),
    /// F47: (trait-requirement structs, of them with an int virtue rank).
    trait_requirements: (usize, usize),
    /// §7.3 targets named by items: legendary titles and legacy-group overrides.
    titles: BTreeSet<Did>,
    groups: BTreeSet<Did>,
}

fn add_maps<K: Ord>(a: &mut BTreeMap<K, usize>, b: BTreeMap<K, usize>) {
    for (k, v) in b {
        *a.entry(k).or_default() += v;
    }
}

impl Census {
    fn merge(&mut self, o: Self) {
        self.items += o.items;
        self.item_like += o.item_like;
        add_maps(&mut self.per_class, o.per_class);
        for (k, m) in o.kinds {
            add_maps(self.kinds.entry(k).or_default(), m);
        }
        self.signature_missing.extend(o.signature_missing);
        self.copy_checks += o.copy_checks;
        self.copy_mismatches.extend(o.copy_mismatches);
        self.value_checked += o.value_checked;
        self.value_null_table += o.value_null_table;
        self.value_mismatch.extend(o.value_mismatch);
        self.value_bad_table += o.value_bad_table;
        self.value_tables.extend(o.value_tables);
        self.weapons += o.weapons;
        self.dps_mismatch.extend(o.dps_mismatch);
        self.damage_mismatch.extend(o.damage_mismatch);
        self.dps_tables.extend(o.dps_tables);
        self.armour_checked += o.armour_checked;
        self.armour_mismatch.extend(o.armour_mismatch);
        self.armour_without_table_nonzero += o.armour_without_table_nonzero;
        self.armour_tables.extend(o.armour_tables);
        self.msp_checked += o.msp_checked;
        self.msp_mismatch.extend(o.msp_mismatch);
        self.durability_checked += o.durability_checked;
        self.durability_mismatch.extend(o.durability_mismatch);
        self.timing_checked += o.timing_checked;
        self.timing_mismatch.extend(o.timing_mismatch);
        self.did_refs += o.did_refs;
        self.did_targets.extend(o.did_targets);
        self.did_missing.extend(o.did_missing);
        self.mod_entries += o.mod_entries;
        self.mod_malformed.extend(o.mod_malformed);
        self.mod_other_fields += o.mod_other_fields;
        self.sockets += o.sockets;
        self.socket_malformed.extend(o.socket_malformed);
        add_maps(&mut self.effect_structs, o.effect_structs);
        self.cooldown_uncovered.extend(o.cooldown_uncovered);
        self.decay_uncovered.extend(o.decay_uncovered);
        self.behaviour_targets.extend(o.behaviour_targets);
        self.property_set_members.extend(o.property_set_members);
        add_maps(&mut self.bindings, o.bindings);
        add_maps(&mut self.worn_lists, o.worn_lists);
        add_maps(&mut self.worn_mirrors, o.worn_mirrors);
        self.worn_mirror_mismatch.extend(o.worn_mirror_mismatch);
        self.worn_list_unread.extend(o.worn_list_unread);
        self.races.0 += o.races.0;
        self.races.1 += o.races.1;
        self.races.2 += o.races.2;
        self.trait_requirements.0 += o.trait_requirements.0;
        self.trait_requirements.1 += o.trait_requirements.1;
        self.titles.extend(o.titles);
        self.groups.extend(o.groups);
    }
}

/// Per-thread caches of pass 2.
struct Caches {
    value: TableCache<ValueTable>,
    dps: TableCache<DpsTable>,
    armour: TableCache<Progression>,
    exists: HashMap<Did, bool>,
}

#[allow(clippy::too_many_lines)]
fn check_record(
    ctx: &Ctx<'_>,
    caches: &mut Caches,
    c: &mut Census,
    (did, kind, class): (Did, ItemClassKind, u32),
) {
    let sets = ctx.sets;
    let tp = &ctx.schema.tables;
    let set = sets
        .props(did)
        .unwrap_or_else(|| panic!("IT1: {did} has a property set"));
    let v = ItemView::new(&set, ctx.schema);
    let item_kind = v.kind(ctx.weenie);
    if kind == ItemClassKind::Item {
        c.items += 1;
        *c.per_class.entry(class).or_default() += 1;
        *c.kinds
            .entry(item_kind)
            .or_default()
            .entry(class)
            .or_default() += 1;
        if !v.has_item_signature() {
            c.signature_missing.push(did);
        }
        *c.bindings
            .entry(format!("{:?}", v.binding(sets.dict)))
            .or_default() += 1;
    } else {
        c.item_like += 1;
    }

    // F47: requirement value types.
    let rp = &ctx.schema.requirements;
    let races = set.elements(rp.required_races);
    if !races.is_empty() {
        c.races.0 += 1;
        c.races.1 += races.len();
        c.races.2 += races
            .iter()
            .filter(|e| !matches!(e.value, PropertyValue::Enum(_)))
            .count();
    }
    let req = v.requirements();
    c.trait_requirements.0 += req.required_traits.len();
    c.trait_requirements.1 += req
        .required_traits
        .iter()
        .filter(|t| t.virtue_rank.is_some())
        .count();
    c.titles
        .extend(v.advancement_title().filter(|d| !d.is_null()));
    c.groups
        .extend(v.progression_group().filter(|d| !d.is_null()));

    // IT3 copies.
    for &(copy, source, label) in ctx.copy_pids {
        if let Some(cv) = set.get_pid(copy) {
            c.copy_checks += 1;
            if set.get_pid(source) != Some(cv) {
                c.copy_mismatches.push((did, label));
            }
        }
    }

    // IT4 value (§9.2), items and item-like alike.
    if let Some(table) = v.value_table() {
        let level = v.level().unwrap_or(0);
        let quality = v.quality().unwrap_or(0);
        let expected = if table.is_null() {
            c.value_null_table += 1;
            item_value(None, level, quality)
        } else {
            c.value_tables.insert(table);
            match caches
                .value
                .get(sets, table, |s| ValueTable::from_set(s, tp).ok())
            {
                Some(t) => item_value(Some(t), level, quality),
                None => {
                    c.value_bad_table += 1;
                    None
                }
            }
        };
        if let Some(stored) = v.value() {
            c.value_checked += 1;
            if expected != Some(i64::from(stored)) {
                c.value_mismatch.push(did);
            }
        }
    }

    // IT5 weapons (§9.3, §9.4) and IT7 timing (§9.7).
    if item_kind == ItemKind::Weapon {
        c.weapons += 1;
        match v.weapon() {
            Some(w) => check_weapon(ctx, caches, c, did, &v, &w),
            None => c.dps_mismatch.push(did),
        }
    }

    // IT6 armour (§9.5).
    if let Some(stored) = v.armour_value() {
        match v.armour_table().filter(|d| !d.is_null()) {
            Some(table) => {
                c.armour_checked += 1;
                c.armour_tables.insert(table);
                let computed = caches
                    .armour
                    .get(sets, table, |s| {
                        Progression::from_properties(s, false, &ctx.schema.progressions).ok()
                    })
                    .and_then(|prog| armour_value(prog, v.level()?));
                if computed != Some(i64::from(stored)) {
                    c.armour_mismatch.push(did);
                }
            }
            None if stored != 0 => c.armour_without_table_nonzero += 1,
            None => {}
        }
    }

    // IT7 wear (§9.6).
    let d = v.durability();
    if let (Some(key), Some(stored)) = (d.max_structure_points_key, d.max_structure_points) {
        c.msp_checked += 1;
        if ctx.wear.max_structure_points(key) != Some(stored) {
            c.msp_mismatch.push(did);
        }
    }
    if let (Some(key), Some(stored)) = (d.durability_key, d.durability_value) {
        c.durability_checked += 1;
        if ctx.wear.durability_value(key) != Some(stored) {
            c.durability_mismatch.push(did);
        }
    }

    // IT8 DID references.
    for r in did_references(&set) {
        c.did_refs += 1;
        c.did_targets.insert(r.target);
        let ok = *caches.exists.entry(r.target).or_insert_with(|| {
            sets.store
                .game_data()
                .source()
                .locate(r.target.0)
                .is_ok_and(|l| l.is_some())
        });
        if !ok {
            c.did_missing.push((did, r.target));
        }
    }

    // IT9 modifiers, IT10 sockets, §5.9 effects.
    for m in v.mods() {
        c.mod_entries += 1;
        if !mod_entry_is_well_formed(&m) {
            c.mod_malformed.push(did);
        }
        if !m.unrelated_fields.is_empty() {
            c.mod_other_fields += 1;
        }
    }
    for s in v.sockets() {
        c.sockets += 1;
        if !s.is_well_formed() {
            c.socket_malformed.push(did);
        }
    }
    for k in EffectListKind::ALL {
        *c.effect_structs.entry(k).or_default() += v.effects(k).len();
    }

    // §9.8 coverage.
    for key in [v.cooldown(), v.equip_cooldown()].into_iter().flatten() {
        if !ctx.cooldowns.covers(key) {
            c.cooldown_uncovered.push((did, key));
        }
    }
    if let Some(key) = v.decay()
        && !ctx.decays.covers(key)
    {
        c.decay_uncovered.push((did, key));
    }

    // IT13 and §9.9 inputs.
    let b = v.behaviours();
    for t in [b.permission, b.take_permission, b.action_handler]
        .into_iter()
        .flatten()
        .filter(|d| !d.is_null())
    {
        c.behaviour_targets.insert(t);
    }
    if let Some(s) = v.item_set().filter(|d| !d.is_null()) {
        c.property_set_members.push((did, s));
    }

    // IT12 (§3.3): the object-state worn map mirrors `Item_WornAppearanceMapList`.
    if let Some(list) = set.array(v.pids().worn_appearance_list) {
        *c.worn_lists.entry(class).or_default() += 1;
        let visual = ItemVisual::from_property_set(&set, ctx.visual);
        if visual.worn.len() != list.elements.len() {
            c.worn_list_unread.push(did);
        }
        match worn_mirror(sets, did) {
            None => {}
            Some(None) => {
                *c.worn_mirrors.entry(class).or_default() += 1;
                c.worn_mirror_mismatch.push(did);
            }
            Some(Some(mirror)) => {
                *c.worn_mirrors.entry(class).or_default() += 1;
                let mut expected = BTreeMap::new();
                let mut duplicate = false;
                for w in &visual.worn {
                    let mut pair = [w.key, w.appearance.0];
                    pair.sort_unstable();
                    duplicate |= expected.insert(w.species | w.sex, pair).is_some();
                }
                if duplicate || mirror != expected {
                    c.worn_mirror_mismatch.push(did);
                }
            }
        }
    }
}

/// IT5 and the timing part of IT7 for one weapon.
fn check_weapon(
    ctx: &Ctx<'_>,
    caches: &mut Caches,
    c: &mut Census,
    did: Did,
    v: &ItemView<'_>,
    w: &WeaponStats,
) {
    let sets = ctx.sets;
    let tp = &ctx.schema.tables;
    let table = w.dps_table.filter(|d| !d.is_null());
    if let Some(t) = table {
        c.dps_tables.insert(t);
    }
    let dps = table.and_then(|t| {
        caches
            .dps
            .get(sets, t, |s| DpsTable::from_set(s, tp).ok())
            .and_then(|t| t.dps(w.dps_level?, v.quality()?))
    });
    if dps.map(f32::to_bits) != w.base_dps.map(f32::to_bits) {
        c.dps_mismatch.push(did);
    }
    let damage = w
        .base_dps
        .zip(w.action_duration)
        .zip(w.variance)
        .and_then(|((d, t), var)| weapon_damage(d, t, var));
    let ok = match (damage, w.damage) {
        (Some(a), Some(b)) => ((a - b) / b).abs() <= 3e-7,
        _ => false,
    };
    if !ok {
        c.damage_mismatch.push(did);
    }
    c.timing_checked += 1;
    let hit = v
        .equipment_category()
        .zip(w.speed)
        .and_then(|(cat, sp)| ctx.timings.lookup(cat, sp));
    let ok = hit.is_some_and(|h| {
        Some(h.action_duration) == w.action_duration
            && Some(h.anim_duration_mod) == w.anim_duration_mod
    });
    if !ok {
        c.timing_mismatch.push(did);
    }
}

/// The worn-appearance mirror of object `did` (§3.3) as key → the two integers of the
/// class-2065 value object, sorted (the spec names the two values, not their order).
/// `None` when the root attribute is absent or null; `Some(None)` when it is present but not
/// a `refmap_u32` of objects with exactly two integers and distinct keys.
fn worn_mirror(sets: &Sets<'_>, did: Did) -> Option<Option<BTreeMap<u32, [u32; 2]>>> {
    let bytes = sets
        .store
        .game_data()
        .read(did)
        .expect("read")
        .expect("record");
    let state = sets
        .store
        .decode(did, &bytes)
        .expect("object state decodes");
    let root = state.root_object()?;
    let target = root.follow(WORN_MAP_ATTRIBUTE)?.item()?;
    let read = || {
        let entries = target.as_native()?.as_ref_map()?;
        let mut out = BTreeMap::new();
        for &(key, r) in entries {
            let object = state.resolve(r).item()?.as_script()?;
            let ints: Vec<u32> = object
                .attributes()
                .filter_map(|a| a.value.as_int())
                .collect();
            let [a, b] = ints.as_slice() else {
                return None;
            };
            let mut pair = [*a, *b];
            pair.sort_unstable();
            if out.insert(key, pair).is_some() {
                return None;
            }
        }
        Some(out)
    };
    Some(read())
}

/// The enum mapper `start` and its base chain, nearest first.
fn mapper_chain(store: &ObjectStore, start: Did) -> Vec<std::sync::Arc<EnumMapper>> {
    let data = store.game_data();
    let mut chain = Vec::new();
    let mut at = start;
    let mut seen = HashSet::new();
    while !at.is_null() && seen.insert(at) && chain.len() < 32 {
        let Some(m) = data.enum_mapper(at).expect("enum mapper") else {
            break;
        };
        at = m.base;
        chain.push(m);
    }
    chain
}

fn first<T: std::fmt::Debug>(v: &[T]) -> &[T] {
    &v[..v.len().min(5)]
}

#[test]
#[allow(clippy::too_many_lines)]
fn items_census_and_invariants() {
    let Some(dir) = game_dir() else {
        eprintln!("TESSERA_GAME_DIR not set; skipping");
        return;
    };
    let started = Instant::now();
    let store = ObjectStore::open(&dir).expect("open install");
    let library = store.library().expect("class library");
    let data = store.game_data();
    let dict = data.registry().expect("dictionary").dictionary().clone();
    let schema = ItemSchema::resolve(&dict);
    let sets = Sets {
        store: &store,
        dict: &dict,
    };
    let missing = schema.missing();
    eprintln!("items: {} labels undefined: {missing:?}", missing.len());
    assert!(missing.is_empty(), "labels the spec names are undefined");

    // ---- Pass 1: every object record: classes, item references, entity stamps (IT2). ----
    let dids = object_dids(data.source()).expect("object DIDs");
    let stamp_pids: Vec<Pid> = entity_labels()
        .iter()
        .map(|&l| Pid::resolve(&dict, l))
        .collect();
    assert!(stamp_pids.iter().all(|p| p.is_resolved()));
    let mut p1 = Pass1::new(stamp_pids.len());
    for part in par_chunks(&dids, |chunk| pass1(&sets, &schema, &stamp_pids, chunk)) {
        p1.merge(part);
    }
    let mut index = ClassIndex::new();
    for &r in &p1.roots {
        index.insert_root(r);
    }
    let references = &p1.references;
    eprintln!(
        "items: pass 1 read {} object records ({} root classes) in {:.1?}; {} item \
         references; {} set decode failures",
        dids.len(),
        index.classes().count(),
        started.elapsed(),
        references.len(),
        p1.failures.len()
    );
    assert!(
        p1.failures.is_empty(),
        "property sets that do not decode: {:?}",
        first(&p1.failures)
    );

    // §2.4: the item classes.
    // Decision F51: a tie between children goes to the one holding `BankFillItem`.
    let bank_class = store
        .weenie(control_labels::BANK_FILL_ITEM)
        .expect("weenie map")
        .and_then(|d| index.root_class(d));
    let classes = ItemClasses::discover_with_bank_fill(
        &*library,
        references.iter().filter_map(|&d| index.root_class(d)),
        bank_class,
    )
    .expect("item classes discovered");
    assert_eq!(
        Some(classes),
        ItemClasses::discover_from_index(&*library, &index, references.iter().copied()),
        "§2.4: no tie on this install"
    );
    let count_under = |ancestor: u32| {
        references
            .iter()
            .filter(|&&d| {
                index
                    .root_class(d)
                    .is_some_and(|c| library.is_a(c, ancestor))
            })
            .count()
    };
    let unresolved_refs = references
        .iter()
        .filter(|&&d| index.root_class(d).is_none())
        .count();
    let under_parent = count_under(classes.item_parent);
    eprintln!(
        "items: P = {}, I = {}; references {} ({} under P, {} under I, {} name no record)",
        classes.item_parent,
        classes.item_base,
        references.len(),
        under_parent,
        count_under(classes.item_base),
        unresolved_refs
    );
    assert_eq!(unresolved_refs, 0, "§2.4: every reference names a record");
    assert_eq!(
        under_parent,
        references.len(),
        "§2.4: all references under P"
    );

    // §2.4 fast hint: the BankFillItem record is an item.
    if let Some(bank) = store
        .weenie(control_labels::BANK_FILL_ITEM)
        .expect("weenie map")
    {
        let c = index.root_class(bank).expect("BankFillItem indexed");
        assert_eq!(classes.classify(&*library, c), ItemClassKind::Item);
    }

    // IT2: the sixteen labels are in exactly the records under one ancestor.
    let with_stamps: HashSet<Did> = p1.with_stamps.iter().copied().collect();
    let stamp_root = library
        .common_ancestor(with_stamps.iter().filter_map(|&d| index.root_class(d)))
        .expect("stamped records share an ancestor");
    let under_root: HashSet<Did> = index.dids_under(&*library, stamp_root).collect();
    let stamp_default_mismatch = stamp_pids
        .iter()
        .zip(&p1.stamp_values)
        .take(8)
        .filter(|(p, v)| {
            let default = p.id().and_then(|id| dict.default_value(id));
            v.as_ref() != default
        })
        .count();
    eprintln!(
        "items: IT2 {} records with all 16 labels, ancestor {} holds {} records; {} partial; \
         {} value conflicts; {} stamps differ from the dictionary default",
        with_stamps.len(),
        stamp_root,
        under_root.len(),
        p1.partial_stamps.len(),
        p1.stamp_conflicts,
        stamp_default_mismatch
    );
    assert!(
        p1.partial_stamps.is_empty(),
        "IT2: some records have only part"
    );
    assert_eq!(
        with_stamps, under_root,
        "IT2: exactly the records under the ancestor"
    );
    assert_eq!(p1.stamp_conflicts, 0, "IT2: one value each");
    assert_eq!(
        stamp_default_mismatch, 0,
        "§4.2: stamps equal their defaults"
    );
    assert!(library.is_a(classes.item_base, stamp_root));

    // ---- Control tables. ----
    let tp = &schema.tables;
    let control = |label: &str| {
        store
            .weenie(label)
            .expect("weenie map")
            .unwrap_or_else(|| panic!("{label} is labelled"))
    };
    let wear = WearControl::from_set(
        &sets
            .props(control(control_labels::ITEM_WEAR_CONTROL))
            .expect("wear set"),
        tp,
    )
    .expect("wear control decodes");
    let timing_dir = sets
        .props(control(control_labels::WEAPON_ACTION_DURATION_TABLE))
        .expect("timing directory set");
    let timings = WeaponTimings {
        tables: weapon_timing_table_dids(&timing_dir, tp)
            .into_iter()
            .map(|d| {
                let s = sets.props(d).expect("timing set");
                WeaponTimingTable::from_set(&s, tp).expect("timing table decodes")
            })
            .collect(),
    };
    let cooldowns = sets
        .props(control(control_labels::COOLDOWN_CONTROL))
        .map(|s| DurationMap::cooldown(&s, tp))
        .unwrap_or_default();
    let decays = sets
        .props(control(control_labels::ITEM_DECAY_CONTROL))
        .map(|s| DurationMap::decay(&s, tp))
        .unwrap_or_default();
    // §6.4 (F53): the examination list is the exam PIDs followed by the merge PIDs.
    let inventory = sets
        .props(control(control_labels::INVENTORY_CONTROL))
        .expect("inventory control set");
    let exam = examination_list(&inventory, tp);
    let merge = merge_list(&inventory, tp);
    eprintln!(
        "items: examination list {} PIDs ({} exam + {} merge)",
        exam.len(),
        exam.len() - merge.len(),
        merge.len()
    );
    assert!(
        exam.ends_with(&merge),
        "§6.4: the merge PIDs close the list"
    );
    assert!(!merge.is_empty() && exam.len() > merge.len());
    eprintln!(
        "items: wear control {} structure-point rows, {} durability rows, {}/{}/{} other \
         rows; {} timing tables ({} entries); {} cooldown rows; {} decay rows",
        wear.max_structure_points.len(),
        wear.durability.len(),
        wear.repair_cost_quality.len(),
        wear.wear_state_value.len(),
        wear.skill_gate.len(),
        timings.tables.len(),
        timings
            .tables
            .iter()
            .map(|t| t.entries.len())
            .sum::<usize>(),
        cooldowns.rows.len(),
        decays.rows.len()
    );
    assert!(!cooldowns.rows.is_empty() && !decays.rows.is_empty());

    // ---- Pass 2: every item and item-like record. ----
    let pass2 = Instant::now();
    let copy_pids: Vec<(Pid, Pid, &'static str)> = COPY_LABELS
        .iter()
        .map(|&(c, s)| (Pid::resolve(&dict, c), Pid::resolve(&dict, s), c))
        .collect();
    // §2.4 rule 4, §11: the `WeenieType` labels, from the mapper its definition names.
    let weenie_mapper =
        enum_mapper_of(&dict, schema.item.weenie_type).expect("WeenieType has a mapper");
    let weenie = ItemWeenieKinds::from_mapper_chain(&mapper_chain(&store, weenie_mapper));
    eprintln!("items: {} WeenieType keys give a kind", weenie.len());
    assert!(!weenie.is_empty(), "WeenieType labels of §2.1 found");
    let visual = VisualPids::from_names(&dict).expect("worn-look labels");
    let ctx = Ctx {
        sets: &sets,
        schema: &schema,
        wear: &wear,
        timings: &timings,
        cooldowns: &cooldowns,
        decays: &decays,
        copy_pids: &copy_pids,
        weenie: &weenie,
        visual: &visual,
    };
    let item_like: Vec<(Did, ItemClassKind, u32)> = index
        .dids_under(&*library, classes.item_parent)
        .filter_map(|d| {
            let class = index.root_class(d)?;
            Some((d, classes.classify(&*library, class), class))
        })
        .collect();
    let mut c = Census::default();
    for part in par_chunks(&item_like, |chunk| {
        let mut caches = Caches {
            value: TableCache::new(),
            dps: TableCache::new(),
            armour: TableCache::new(),
            exists: HashMap::new(),
        };
        let mut census = Census::default();
        for &rec in chunk {
            check_record(&ctx, &mut caches, &mut census, rec);
        }
        census
    }) {
        c.merge(part);
    }
    let items: HashSet<Did> = item_like
        .iter()
        .filter(|(_, k, _)| *k == ItemClassKind::Item)
        .map(|&(d, _, _)| d)
        .collect();

    eprintln!(
        "items: pass 2 checked {} items and {} other item-like records in {:.1?}",
        c.items,
        c.item_like,
        pass2.elapsed()
    );
    eprintln!("items: records per root class: {:?}", c.per_class);
    for (kind, per) in &c.kinds {
        eprintln!("items: kind {kind:?}: {per:?}");
    }
    eprintln!("items: binding readings: {:?}", c.bindings);
    eprintln!(
        "items: IT1 {} without the item signature; IT3 {} copy checks, {} mismatches",
        c.signature_missing.len(),
        c.copy_checks,
        c.copy_mismatches.len()
    );
    eprintln!(
        "items: IT4 {} values checked ({} null tables), {} mismatches, {} undecodable table \
         uses, {} distinct tables",
        c.value_checked,
        c.value_null_table,
        c.value_mismatch.len(),
        c.value_bad_table,
        c.value_tables.len()
    );
    eprintln!(
        "items: IT5 {} weapons, {} DPS mismatches, {} damage mismatches ({} DPS tables)",
        c.weapons,
        c.dps_mismatch.len(),
        c.damage_mismatch.len(),
        c.dps_tables.len()
    );
    eprintln!(
        "items: IT6 {} armour values with a table, {} mismatches, {} non-zero without a \
         table ({} armour tables)",
        c.armour_checked,
        c.armour_mismatch.len(),
        c.armour_without_table_nonzero,
        c.armour_tables.len()
    );
    eprintln!(
        "items: IT7 {} structure points ({} mismatches), {} durability values ({} \
         mismatches), {} timings ({} mismatches)",
        c.msp_checked,
        c.msp_mismatch.len(),
        c.durability_checked,
        c.durability_mismatch.len(),
        c.timing_checked,
        c.timing_mismatch.len()
    );
    eprintln!(
        "items: IT8 {} DID references to {} targets, {} missing",
        c.did_refs,
        c.did_targets.len(),
        c.did_missing.len()
    );
    eprintln!(
        "items: IT9 {} modifier entries, {} malformed, {} with extra fields; IT10 {} \
         sockets, {} malformed; effect structs {:?}",
        c.mod_entries,
        c.mod_malformed.len(),
        c.mod_other_fields,
        c.sockets,
        c.socket_malformed.len(),
        c.effect_structs
    );
    eprintln!(
        "items: §9.8 {} cooldown keys and {} decay keys without a row",
        c.cooldown_uncovered.len(),
        c.decay_uncovered.len()
    );
    eprintln!(
        "items: IT12 worn-look lists per class {:?}; object-state mirrors per class {:?}; {} \
         mirrors differ; {} lists with unreadable structs",
        c.worn_lists,
        c.worn_mirrors,
        c.worn_mirror_mismatch.len(),
        c.worn_list_unread.len()
    );

    assert!(c.items > 0);
    assert!(
        c.signature_missing.is_empty(),
        "IT1: {:?}",
        first(&c.signature_missing)
    );
    assert!(
        c.copy_mismatches.is_empty(),
        "IT3: {:?}",
        first(&c.copy_mismatches)
    );
    assert_eq!(c.value_bad_table, 0, "IT4: value tables decode");
    assert!(
        c.value_mismatch.is_empty(),
        "IT4: {:?}",
        first(&c.value_mismatch)
    );
    assert!(c.weapons > 0);
    assert!(
        c.dps_mismatch.is_empty(),
        "IT5 DPS: {:?}",
        first(&c.dps_mismatch)
    );
    assert!(
        c.damage_mismatch.is_empty(),
        "IT5 damage: {:?}",
        first(&c.damage_mismatch)
    );
    assert!(
        c.armour_mismatch.is_empty(),
        "IT6: {:?}",
        first(&c.armour_mismatch)
    );
    assert_eq!(
        c.armour_without_table_nonzero, 0,
        "§4.5: armour without a table is 0"
    );
    assert!(c.msp_mismatch.is_empty(), "IT7 structure points");
    assert!(c.durability_mismatch.is_empty(), "IT7 durability");
    assert!(
        c.timing_mismatch.is_empty(),
        "IT7 timing: {:?}",
        first(&c.timing_mismatch)
    );
    assert!(c.did_missing.is_empty(), "IT8: {:?}", first(&c.did_missing));
    assert!(c.mod_malformed.is_empty(), "IT9");
    assert!(c.socket_malformed.is_empty(), "IT10");
    assert!(
        c.cooldown_uncovered.is_empty(),
        "§9.8 cooldowns: {:?}",
        first(&c.cooldown_uncovered)
    );
    assert!(
        c.decay_uncovered.is_empty(),
        "§9.8 decay: {:?}",
        first(&c.decay_uncovered)
    );

    // IT12: every mirror equals its list, and every list-bearing record of a class with
    // mirrors has one (§3.3; the apparel class and the weapons with a list, F46). Which
    // classes have mirrors is reported, not asserted.
    assert!(
        c.worn_mirror_mismatch.is_empty(),
        "IT12: {:?}",
        first(&c.worn_mirror_mismatch)
    );
    assert!(
        c.worn_list_unread.is_empty(),
        "IT12: worn-look structs read: {:?}",
        first(&c.worn_list_unread)
    );
    assert!(!c.worn_mirrors.is_empty(), "IT12: mirrors found");
    for (class, n) in &c.worn_mirrors {
        assert_eq!(
            Some(n),
            c.worn_lists.get(class),
            "IT12: every list of class {class} is mirrored"
        );
    }

    // §2.1 / §2.4 rule 4 (Decision F51): every item class has one dominant kind (all but a
    // handful of its records); the kinds whose signature §2.1 marks "all, exclusive" cover
    // their class whole; every kind occurs; and no kind other than General (several classes
    // of plain items) dominates two classes.
    let mut dominant: BTreeMap<ItemKind, Vec<u32>> = BTreeMap::new();
    for (&class, &total) in &c.per_class {
        let (kind, n) = c
            .kinds
            .iter()
            .filter_map(|(k, per)| per.get(&class).map(|&n| (*k, n)))
            .max_by_key(|&(_, n)| n)
            .expect("class has a kind");
        eprintln!("items: class {class}: {kind:?} {n} of {total}");
        assert!(
            n * 100 >= total * 99,
            "class {class}: dominant kind {kind:?} covers {n} of {total}"
        );
        dominant.entry(kind).or_default().push(class);
    }
    for kind in ItemKind::ALL {
        assert!(c.kinds.contains_key(&kind), "kind {kind:?} occurs");
        let classes = dominant.get(&kind).map_or(0, Vec::len);
        if kind != ItemKind::General {
            assert!(classes <= 1, "{kind:?} dominates {classes} classes");
        }
    }
    let off_kind: usize = c
        .per_class
        .keys()
        .map(|class| {
            let per: Vec<usize> = c
                .kinds
                .values()
                .filter_map(|m| m.get(class).copied())
                .collect();
            per.iter().sum::<usize>() - per.iter().max().copied().unwrap_or(0)
        })
        .sum();
    eprintln!("items: §2.4 rule 4 places {off_kind} items outside their class's kind (spec: 6)");
    for kind in [
        ItemKind::Weapon,
        ItemKind::RecipeScroll,
        ItemKind::MultiRecipeScroll,
        ItemKind::Package,
        ItemKind::TextItem,
        ItemKind::PropertyToken,
    ] {
        let per = c.kinds.get(&kind).cloned().unwrap_or_default();
        assert_eq!(per.len(), 1, "{kind:?} spans one root class: {per:?}");
        let (&class, &n) = per.iter().next().expect("one class");
        assert_eq!(n, c.per_class[&class], "{kind:?} covers its whole class");
    }

    // IT13: behaviour objects are the only 0x70 record of their class and small. (A class
    // may also have its 0x76 singleton record, which is not counted.)
    let mut behaviour_bad = Vec::new();
    for &t in &c.behaviour_targets {
        let records = index.root_class(t).map_or(0, |cl| {
            index
                .dids_of_class(cl)
                .iter()
                .filter(|d| d.prefix() == 0x70)
                .count()
        });
        let properties = sets.props(t).map_or(0, |s| s.len());
        if records != 1 || properties > 6 {
            behaviour_bad.push((t, records, properties));
        }
    }
    eprintln!(
        "items: IT13 {} behaviour targets, {} violate",
        c.behaviour_targets.len(),
        behaviour_bad.len()
    );
    assert!(behaviour_bad.is_empty(), "IT13: {behaviour_bad:?}");

    // F47: race entries are bare enums; trait requirements carry an int virtue rank.
    eprintln!(
        "items: F47 {} items with races ({} entries, {} not enums); {} trait-requirement \
         structs, {} with an int virtue rank",
        c.races.0, c.races.1, c.races.2, c.trait_requirements.0, c.trait_requirements.1
    );
    assert_eq!(c.races.2, 0, "F47: race entries are bare enums");
    assert_eq!(
        c.trait_requirements.0, c.trait_requirements.1,
        "F47: every trait requirement has an int virtue rank"
    );

    // §7.3 (F49): legendary titles and legacy groups named by items, and the legacy tables
    // the groups name.
    let mut table_dids = BTreeSet::new();
    let mut titles_ok = 0usize;
    for &t in &c.titles {
        let Some(set) = sets.props(t) else { continue };
        let v = LegendaryTitleView::new(&set, &schema);
        titles_ok += usize::from(
            v.name().is_some()
                && v.category().is_some()
                && v.tier().is_some()
                && v.slayer_genus().is_some()
                && v.damage_type().is_some(),
        );
    }
    let mut pulls = 0usize;
    for &g in &c.groups {
        let Some(set) = sets.props(g) else { continue };
        let group = LegacyGroup::from_set(&set, &schema.legacy);
        pulls += group.pull_counts.len();
        table_dids.extend(group.lists.iter().filter_map(|l| l.list));
    }
    let mut weights = (0usize, 0usize);
    for &t in &table_dids {
        let Some(set) = sets.props(t) else { continue };
        for e in legacy_effects(&set, &schema.aux) {
            weights.0 += 1;
            weights.1 += usize::from(e.weight.is_some());
        }
    }
    eprintln!(
        "items: F49 {} titles ({} with all five fields), {} legacy groups ({} pull counts), \
         {} legacy tables ({} effects, {} with an int weight)",
        c.titles.len(),
        titles_ok,
        c.groups.len(),
        pulls,
        table_dids.len(),
        weights.0,
        weights.1
    );
    assert_eq!(
        titles_ok,
        c.titles.len(),
        "F49: every title has its five fields"
    );
    assert_eq!(weights.0, weights.1, "F49: every legacy weight is an int");

    // IT11 and §9.9: sets.
    let dir_set = sets
        .props(control(control_labels::GAME_SET_DIRECTORY))
        .expect("directory set");
    let set_dids = set_directory(&dir_set, &schema.sets);
    let mut class_member_kinds: BTreeMap<u32, (usize, usize)> = BTreeMap::new();
    let mut not_ascending = Vec::new();
    let mut without_list = 0usize;
    let mut set_records: Vec<(Did, u32, Vec<Did>)> = Vec::new();
    for &s in &set_dids {
        let props = sets
            .props(s)
            .unwrap_or_else(|| panic!("set {s} has properties"));
        let view = ItemSetView::new(&props, &schema);
        if !view.tiers_ascending() {
            not_ascending.push(s);
        }
        if !view.has_member_list() {
            without_list += 1;
        }
        let class = index.root_class(s).expect("set indexed");
        let members = view.members();
        let entry = class_member_kinds.entry(class).or_default();
        for m in &members {
            if items.contains(m) {
                entry.0 += 1;
            } else {
                entry.1 += 1;
            }
        }
        set_records.push((s, class, members));
    }
    // Item-set classes: those whose members are items.
    let item_set_classes: HashSet<u32> = class_member_kinds
        .iter()
        .filter(|(_, (n, _))| *n > 0)
        .map(|(&cl, _)| cl)
        .collect();
    let mut membership = SetMembership::new();
    for (s, class, members) in &set_records {
        if item_set_classes.contains(class) {
            membership.add_member_list(*s, members.iter().copied());
        }
    }
    let list_conflicts = membership.conflicts().len();
    let directory: HashSet<Did> = set_dids.iter().copied().collect();
    let mut property_set_outside = 0usize;
    for &(item, s) in &c.property_set_members {
        if !directory.contains(&s) {
            property_set_outside += 1;
        }
        membership.add_property_set_member(item, s);
    }
    let sets_with_members = membership
        .sets()
        .filter(|&s| !membership.members_of(s).is_empty())
        .count();
    eprintln!(
        "items: IT11 {} directory sets ({} without a member list); members per set class \
         (items, non-items) {:?}; {} items in sets ({} via Item_PropertySet), {} sets with \
         members; {} not ascending; {} list conflicts; {} total conflicts; {} \
         Item_PropertySet targets outside the directory",
        set_dids.len(),
        without_list,
        class_member_kinds,
        membership.len(),
        c.property_set_members.len(),
        sets_with_members,
        not_ascending.len(),
        list_conflicts,
        membership.conflicts().len(),
        property_set_outside
    );
    assert!(
        not_ascending.is_empty(),
        "IT11 ascending: {not_ascending:?}"
    );
    for (class, (n_items, others)) in &class_member_kinds {
        assert!(
            *n_items == 0 || *others == 0,
            "IT11: set class {class} mixes item and non-item members"
        );
    }
    assert_eq!(membership.conflicts(), &[], "IT11 / §9.9 step 4");
    assert_eq!(
        property_set_outside, 0,
        "§7.1: Item_PropertySet names a listed set"
    );
    eprintln!("items: total {:.1?}", started.elapsed());
}
