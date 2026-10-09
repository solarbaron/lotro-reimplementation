// SPDX-License-Identifier: GPL-3.0-or-later
//! Real-data checks over the game objects of a user's own install, read-only: mount
//! effects, mount entities and template mount fields (`mounts.md` §2, §3, §6, §9),
//! template drama lists (`npc-drama.md` §2.1, §3.2), the version master list and the
//! property-response objects (`world-logic.md` §8, §9, §11 WL-I10).
//!
//! Skipped unless `TESSERA_GAME_DIR` points at an install. One pass reads every object
//! record (0x70) and its property set. Invariants always fail the test; census counts are
//! printed and compared, and fail only with `TESSERA_CENSUS=strict`.

use std::collections::{BTreeMap, BTreeSet};
use std::path::PathBuf;
use std::sync::atomic::{AtomicUsize, Ordering};
use std::sync::{Mutex, OnceLock};

use tessera_formats::gameplay::common::{
    ObjectRoot, PropertyAccess, decode_object_properties, enum_mapper_of,
};
use tessera_formats::gameplay::drama::{DramaKind, DramaLabels, DramaList};
use tessera_formats::gameplay::mounts::{
    MountEffect, MountGroup, MountLabels, MountType, TemplateMountStats, startup_effects,
    toggled_effects,
};
use tessera_formats::gameplay::world_logic::{
    ResponseCallback, ResponseMap, VersionMasterList, WorldLogicLabels,
};
use tessera_formats::{Did, PropertyDictionary, PropertySet, resolve_setup_did};
use tessera_wstate::{ObjectStore, object_dids};

fn game_dir() -> Option<PathBuf> {
    let dir = PathBuf::from(std::env::var_os("TESSERA_GAME_DIR")?);
    dir.is_dir().then_some(dir)
}

fn strict() -> bool {
    std::env::var_os("TESSERA_CENSUS").is_some_and(|v| v == "strict")
}

fn par_map<T: Sync, R: Send>(items: &[T], f: impl Fn(&T) -> R + Sync) -> Vec<R> {
    let next = AtomicUsize::new(0);
    let out: Mutex<Vec<(usize, Vec<R>)>> = Mutex::new(Vec::new());
    let threads = std::thread::available_parallelism().map_or(4, |n| n.get());
    const CHUNK: usize = 256;
    std::thread::scope(|s| {
        for _ in 0..threads {
            s.spawn(|| {
                loop {
                    let i = next.fetch_add(CHUNK, Ordering::Relaxed);
                    if i >= items.len() {
                        break;
                    }
                    let end = (i + CHUNK).min(items.len());
                    let chunk: Vec<R> = items[i..end].iter().map(&f).collect();
                    out.lock().unwrap().push((i, chunk));
                }
            });
        }
    });
    let mut parts = out.into_inner().unwrap();
    parts.sort_by_key(|(i, _)| *i);
    parts.into_iter().flat_map(|(_, v)| v).collect()
}

struct Obj {
    did: Did,
    class: u32,
    set: PropertySet,
}

struct World {
    store: ObjectStore,
    dict: PropertyDictionary,
    mounts: MountLabels,
    drama: DramaLabels,
    logic: WorldLogicLabels,
    /// Objects whose sets carry a property this test reads.
    objects: Vec<Obj>,
    all: BTreeSet<Did>,
    /// Integer world events (class 1887 here, `world-logic.md` §6.7): (event, stored
    /// minimum, stored maximum).
    int_events: Vec<(Did, Option<i64>, Option<i64>)>,
    /// Every world-event condition at any depth of every object set: (event, stored value,
    /// compares with another event).
    conditions: Vec<(Did, Option<i32>, bool)>,
}

/// The world-event conditions at any depth of `values` (structs holding
/// `WorldEvent_WorldEvent`), up to a nesting depth of 32.
fn collect_conditions(
    w: &tessera_formats::gameplay::quests::WorldEventPids,
    entries: &[tessera_formats::PropertyEntry],
    depth: usize,
    out: &mut Vec<(Did, Option<i32>, bool)>,
) {
    use tessera_formats::gameplay::quests::WorldEventCondition;
    if depth > 32 {
        return;
    }
    for e in entries {
        match &e.value {
            tessera_formats::PropertyValue::Struct(st) => {
                if st.has(w.world_event) {
                    let c = WorldEventCondition::from_struct(w, st);
                    out.push((c.world_event, c.value, c.compare_with.is_some()));
                }
                collect_conditions(w, &st.fields, depth + 1, out);
            }
            tessera_formats::PropertyValue::Array(a) => {
                collect_conditions(w, &a.elements, depth + 1, out);
            }
            _ => {}
        }
    }
}

fn world() -> Option<&'static World> {
    static W: OnceLock<Option<World>> = OnceLock::new();
    W.get_or_init(|| {
        let Some(dir) = game_dir() else {
            eprintln!("TESSERA_GAME_DIR not set; skipping");
            return None;
        };
        let store = ObjectStore::open(&dir).expect("open install");
        let data = store.game_data();
        let dict = data.registry().expect("dictionary").dictionary().clone();
        let mounts = MountLabels::resolve(&dict);
        let drama = DramaLabels::resolve(&dict);
        let logic = WorldLogicLabels::resolve(&dict);
        let dids: Vec<Did> = object_dids(data.source())
            .expect("object DIDs")
            .into_iter()
            .filter(|d| d.prefix() == 0x70)
            .collect();
        let all: BTreeSet<Did> = dids.iter().copied().collect();
        let interesting = [
            mounts.effect.slot_holding_location,
            mounts.effect.slot_entity,
            mounts.effect.mount_type,
            mounts.stats.max_speed,
            mounts.stats.toggle_effect_list,
            mounts.stats.init_effects,
            mounts.stats.startup_effects,
            drama.drama.list,
            drama.drama.can_interact,
            logic.version.data_list,
            logic.response.list,
            logic.response.drama,
            logic.response.ai_hint,
            logic.response.applied_effects,
            logic.response.calculate,
            logic.response.map_array,
        ];
        let t0 = std::time::Instant::now();
        let world_pids = &logic.world;
        type Scan = (
            Option<Obj>,
            Option<(Did, Option<i64>, Option<i64>)>,
            Vec<(Did, Option<i32>, bool)>,
        );
        let scanned: Vec<Scan> = par_map(&dids, |&did| {
            let Some(payload) = data.read(did).expect("read") else {
                return (None, None, Vec::new());
            };
            let root = ObjectRoot::read_expecting(&payload, did).expect("object root");
            let Some(set) = root
                .property_set_did()
                .and_then(|s| data.read(s).expect("read set"))
                .map(|b| {
                    decode_object_properties(did, &b, &dict)
                        .unwrap_or_else(|e| panic!("{did}: {e}"))
                })
            else {
                return (None, None, Vec::new());
            };
            let mut conds = Vec::new();
            collect_conditions(world_pids, &set.entries, 0, &mut conds);
            let int = |p: tessera_formats::gameplay::common::Pid| {
                set.get_pid(p)
                    .and_then(tessera_formats::PropertyValue::as_f64)
            };
            #[allow(clippy::cast_possible_truncation)]
            let event = (root.root_class == 1887).then(|| {
                (
                    did,
                    int(world_pids.min_int).map(|v| v as i64),
                    int(world_pids.max_int).map(|v| v as i64),
                )
            });
            let obj = interesting.iter().any(|p| set.has(*p)).then_some(Obj {
                did,
                class: root.root_class,
                set,
            });
            (obj, event, conds)
        });
        let mut objects: Vec<Obj> = Vec::new();
        let mut int_events = Vec::new();
        let mut conditions = Vec::new();
        for (o, e, c) in scanned {
            objects.extend(o);
            int_events.extend(e);
            conditions.extend(c);
        }
        eprintln!(
            "{} object records, {} with a property of interest, in {:.1?}",
            dids.len(),
            objects.len(),
            t0.elapsed()
        );
        Some(World {
            store,
            dict,
            mounts,
            drama,
            logic,
            objects,
            all,
            int_events,
            conditions,
        })
    })
    .as_ref()
}

fn report(rows: &[(&str, u64, u64)]) {
    let mut differ = Vec::new();
    for (k, spec, got) in rows {
        let mark = if got == spec { "  " } else { "!!" };
        eprintln!("  {mark} {k}: spec {spec}, measured {got}");
        if got != spec {
            differ.push(*k);
        }
    }
    eprintln!("{} census rows, {} differ", rows.len(), differ.len());
    if strict() {
        assert!(differ.is_empty(), "census differs: {differ:?}");
    }
}

fn label(w: &World, pid: tessera_formats::gameplay::common::Pid, key: u32) -> Option<String> {
    let mapper = enum_mapper_of(&w.dict, pid)?;
    w.store
        .game_data()
        .enum_label(mapper, key)
        .ok()
        .and_then(|l| l.name.map(|n| n.to_string()))
}

#[test]
#[allow(clippy::too_many_lines)]
fn mount_effects_and_templates() {
    let Some(w) = world() else { return };
    let m = &w.mounts;
    let mut bad: Vec<String> = Vec::new();
    let mut groups: BTreeMap<&str, u64> = BTreeMap::new();
    let mut effects: BTreeSet<Did> = BTreeSet::new();
    let mut entities: BTreeSet<Did> = BTreeSet::new();
    let mut by_type: BTreeMap<u32, u64> = BTreeMap::new();
    for o in &w.objects {
        let Some(e) = MountEffect::recognise(m, &o.set) else {
            continue;
        };
        effects.insert(o.did);
        let r = e.rider();
        let war = e.group() == MountGroup::WarSteed;
        *groups
            .entry(if war { "war-steed" } else { "durability" })
            .or_default() += 1;
        if e.is_hobby_horse(|k| {
            // Leaked only for the test's lifetime; labels are short.
            label(w, m.effect.animation_script, k).map(|s| &*Box::leak(s.into_boxed_str()))
        }) {
            *groups.entry("hobby-horse").or_default() += 1;
        }
        if e.is_startup_record(|k| {
            label(w, m.effect.equivalence_class, k).map(|s| &*Box::leak(s.into_boxed_str()))
        }) {
            *groups.entry("startup").or_default() += 1;
        }
        if let Some(t) = e.mount_type() {
            *by_type.entry(t.0).or_default() += 1;
        }
        if let Some(ent) = r.entity.filter(|d| !d.is_null()) {
            entities.insert(ent);
        }
        // MI2.
        let loc = r
            .holding_location
            .and_then(|k| label(w, m.effect.slot_holding_location, k));
        if loc.as_deref() != Some("Mount_Connection") || r.rotation.iter().any(|x| *x != Some(0.0))
        {
            bad.push(format!("MI2 {}: {loc:?} {:?}", o.did, r.rotation));
        }
        // MI3.
        let durability_war = e.durability_max() == Some(300) && e.durability_regen() == Some(1.5);
        if war != durability_war || (war && !e.durability_events().is_empty()) {
            bad.push(format!("MI3 {}", o.did));
        }
        // MI4.
        if !war {
            let ev = e.durability_events();
            let ids: Vec<Option<u32>> = ev.iter().map(|x| x.event).collect();
            if ids != [Some(22), Some(17), Some(31)]
                || ev.iter().any(|x| x.change.is_none_or(|c| c >= 0))
            {
                bad.push(format!("MI4 {}: {ev:?}", o.did));
            }
        }
        // MI10.
        if let Some(t) = e.mount_type() {
            let war_label = t.keys().any(|k| (4..=6).contains(&k));
            if t.is_war_steed() != war_label {
                bad.push(format!("MI10 {}", o.did));
            }
        }
    }
    // Skills that toggle mount effects (§4.1) and templates (§6.6).
    let mut toggling = 0u64;
    let mut toggled: BTreeSet<Did> = BTreeSet::new();
    let mut templates_with_entity = 0u64;
    let mut template_startups: BTreeSet<Did> = BTreeSet::new();
    let mut starting = 0u64;
    let mut max_speed: BTreeMap<String, u64> = BTreeMap::new();
    for o in &w.objects {
        let t = toggled_effects(&m.stats, &o.set);
        if t.iter().any(|d| effects.contains(d)) {
            toggling += 1;
            toggled.extend(t.into_iter().filter(|d| effects.contains(d)));
        }
        let starts: Vec<Did> = startup_effects(&m.stats, &o.set)
            .into_iter()
            .filter(|d| effects.contains(d))
            .collect();
        if !starts.is_empty() {
            starting += 1;
        }
        if !effects.contains(&o.did) && o.set.has(m.effect.slot_entity) {
            templates_with_entity += 1;
            if let Some(ent) = o.set.did_nonnull(m.effect.slot_entity) {
                entities.insert(ent);
            }
            // MI8: every such template has Mount_Type and starts a mount effect.
            if !o.set.has(m.effect.mount_type) || starts.is_empty() {
                bad.push(format!("MI8 {}", o.did));
            }
            template_startups.extend(starts);
        }
        let s = TemplateMountStats::read(&m.stats, &o.set);
        if let Some(v) = s.max_speed {
            *max_speed.entry(format!("{v}")).or_default() += 1;
        }
    }
    // MI6 (resolution part): every mount entity resolves to a setup.
    let data = w.store.game_data();
    let mut setups: BTreeSet<Did> = BTreeSet::new();
    for &ent in &entities {
        match resolve_setup_did(ent, |d| data.read(d)) {
            Ok(path) => {
                if let Some(s) = path.last() {
                    setups.insert(*s);
                }
            }
            Err(e) => bad.push(format!("MI6 {ent}: {e}")),
        }
    }
    eprintln!("mount types: {by_type:x?}");
    eprintln!("template Mount_Movement_Max_Speed values: {max_speed:?}");
    eprintln!("template startup mount effects: {template_startups:?}");
    let g = |k: &str| groups.get(k).copied().unwrap_or(0);
    report(&[
        ("mount effects (§2.1)", 722, effects.len() as u64),
        ("durability mounts", 691, g("durability")),
        ("war-steed records", 31, g("war-steed")),
        ("hobby-horse records", 8, g("hobby-horse")),
        ("startup records", 3, g("startup")),
        ("skills toggling a mount effect (§4.1)", 688, toggling),
        ("effects toggled by skills", 685, toggled.len() as u64),
        (
            "templates with a mount entity (§6.6)",
            280,
            templates_with_entity,
        ),
        ("templates starting a mount effect", 418, starting),
        ("mount entities (§3.1)", 607, entities.len() as u64),
        ("setups of mount entities", 13, setups.len() as u64),
        (
            "templates' startup effect DIDs",
            1,
            template_startups.len() as u64,
        ),
    ]);
    for b in bad.iter().take(10) {
        eprintln!("  FAIL {b}");
    }
    assert!(bad.is_empty(), "{} mount invariants violated", bad.len());
}

#[test]
fn template_drama_lists() {
    let Some(w) = world() else { return };
    let d = &w.drama.drama;
    let mut lists = 0u64;
    let mut kinds: BTreeMap<&str, u64> = BTreeMap::new();
    let mut by_class: BTreeMap<u32, u64> = BTreeMap::new();
    let mut can_interact = 0u64;
    let mut bad = Vec::new();
    for o in &w.objects {
        if o.set.has(d.can_interact) {
            can_interact += 1;
        }
        let Some(list) = DramaList::read(d, &o.set) else {
            continue;
        };
        lists += 1;
        *by_class.entry(o.class).or_default() += 1;
        if list.has_duplicate_names() {
            bad.push(format!("duplicate names in {}", o.did));
        }
        for x in &list.dramas {
            *kinds.entry(x.kind.name()).or_default() += 1;
            if matches!(x.kind, DramaKind::Other(_)) || x.name().is_none() {
                bad.push(format!("element of {}", o.did));
            }
        }
    }
    eprintln!("template drama lists by root class: {by_class:?}");
    let k = |n: &str| kinds.get(n).copied().unwrap_or(0);
    report(&[
        ("template lists (§2.1)", 433, lists),
        ("template named dramas (§3.2)", 927, k("named")),
        ("template instant dramas", 163, k("instant")),
        ("template idle dramas", 4, k("idle")),
        (
            "Drama_CanInteractDuringDrama holders (§9.2)",
            4,
            can_interact,
        ),
    ]);
    assert!(bad.is_empty(), "{bad:?}");
}

/// WL-I12 (`world-logic.md` §6.7, WL25): every integer world event stores
/// `WorldEvent_MaxIntValue` > 0, and every condition on an integer event without a stored
/// minimum has 0 ≤ *c* ≤ maximum (*c* = −1 when not stored); none of them compares with
/// another event. Counts are a census.
#[test]
fn world_event_bounds() {
    let Some(w) = world() else { return };
    let events: BTreeMap<Did, (Option<i64>, Option<i64>)> = w
        .int_events
        .iter()
        .map(|&(d, min, max)| (d, (min, max)))
        .collect();
    let no_max: Vec<&Did> = events
        .iter()
        .filter(|(_, (_, max))| max.is_none_or(|m| m <= 0))
        .map(|(d, _)| d)
        .collect();
    assert!(
        no_max.is_empty(),
        "WL-I12: integer events without a positive maximum {no_max:?}"
    );
    let no_min: BTreeSet<Did> = events
        .iter()
        .filter(|(_, (min, _))| min.is_none())
        .map(|(d, _)| *d)
        .collect();
    let mut on_no_min = 0u64;
    let mut bad = Vec::new();
    for &(event, value, compare) in &w.conditions {
        if !no_min.contains(&event) {
            continue;
        }
        on_no_min += 1;
        let max = events.get(&event).and_then(|(_, m)| *m).unwrap_or(0);
        let c = i64::from(value.unwrap_or(-1));
        if compare || c < 0 || c > max {
            bad.push(format!("{event}: c {c}, max {max}, compare {compare}"));
        }
    }
    assert!(bad.is_empty(), "WL-I12: {bad:?}");
    report(&[
        ("integer world events (§6.7)", 699, events.len() as u64),
        ("without a stored minimum", 91, no_min.len() as u64),
        ("conditions on those events", 36, on_no_min),
    ]);
}

#[test]
fn version_and_response_objects() {
    let Some(w) = world() else { return };
    let mut masters: Vec<(Did, VersionMasterList)> = Vec::new();
    let mut maps = 0u64;
    let mut callbacks: BTreeMap<&str, u64> = BTreeMap::new();
    for o in &w.objects {
        if let Some(m) = VersionMasterList::read(&w.logic.version, &o.set) {
            masters.push((o.did, m));
        }
        if ResponseMap::read(&w.logic.response, &o.set).is_some() {
            maps += 1;
        }
        if let Some(c) = ResponseCallback::read(&w.logic.response, &o.set, |p| w.dict.name(p)) {
            let k = match c {
                ResponseCallback::Drama { .. } => "drama",
                ResponseCallback::AiHint { .. } => "ai hint",
                ResponseCallback::AppliedEffects(_) => "applied effects",
                ResponseCallback::Calculate(_) => "calculate",
                ResponseCallback::MapProperty(_) => "map property",
            };
            *callbacks.entry(k).or_default() += 1;
        }
    }
    assert_eq!(masters.len(), 1, "one version master list");
    let (mdid, m) = &masters[0];
    if let Some(o) = w.objects.iter().find(|o| o.did == *mdid)
        && let Some(first) = o.set.structs(w.logic.version.data_list).next()
    {
        let types: Vec<String> = first
            .fields
            .iter()
            .map(|f| {
                format!(
                    "{}: {:?}",
                    w.dict.name(f.pid).unwrap_or("?"),
                    f.value.property_type()
                )
            })
            .collect();
        eprintln!(
            "version entry field types: {types:?}; numbers {:?}",
            m.entries.iter().take(3).map(|e| e.0).collect::<Vec<_>>()
        );
    }
    assert!(
        m.is_ascending(),
        "WL-I10: strictly ascending update numbers"
    );
    // WL-I13 (§9.1, WL26): every `Version_Number` value is a type-8 enum value and a key of
    // the mapper the property names.
    let number = w.logic.version.number;
    if let Some(o) = w.objects.iter().find(|o| o.did == *mdid) {
        let mut values: Vec<&tessera_formats::PropertyValue> = Vec::new();
        values.extend(o.set.get_pid(number));
        for st in o.set.structs(w.logic.version.data_list) {
            values.extend(st.get_pid(number));
        }
        let bad: Vec<String> = values
            .iter()
            .filter(|v| match v {
                tessera_formats::PropertyValue::Enum(k) => label(w, number, *k).is_none(),
                _ => true,
            })
            .map(|v| format!("{v:?}"))
            .collect();
        eprintln!("Version_Number values: {}", values.len());
        assert!(bad.is_empty(), "WL-I13: {bad:?}");
        report(&[(
            "Version_Number values (§9.1, WL26)",
            94,
            values.len() as u64,
        )]);
    }
    let listed: BTreeSet<Did> = m.objects().into_iter().collect();
    let missing: Vec<&Did> = listed.iter().filter(|d| !w.all.contains(d)).collect();
    assert!(
        missing.is_empty(),
        "WL-I10: listed objects exist: {missing:?}"
    );
    let c = |k: &str| callbacks.get(k).copied().unwrap_or(0);
    report(&[
        ("version entries (§9.1)", 93, m.entries.len() as u64),
        (
            "version objects listed + always-run",
            158,
            listed.len() as u64,
        ),
        ("response maps (§8.1)", 120, maps),
        ("drama callbacks (class 2225)", 11, c("drama")),
        ("AI-hint callbacks (class 2193)", 21, c("ai hint")),
        (
            "applied-effect callbacks (class 2353)",
            237,
            c("applied effects"),
        ),
        ("calculate callbacks (class 2517)", 9, c("calculate")),
        ("map-property callbacks (class 2518)", 21, c("map property")),
    ]);
    let _ = MountType(0);
}
