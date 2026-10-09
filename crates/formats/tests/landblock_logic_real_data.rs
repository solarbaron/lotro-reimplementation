// SPDX-License-Identifier: GPL-3.0-or-later
//! Real-data checks of the landblock-borne logic against a user's own install, read-only:
//! drama lists (`npc-drama.md` §2–§8, §12), instance script tables (`instance-scripts.md`
//! §2–§5, §10), quest-state phasing and placed logic (`world-logic.md` §2–§7, §11),
//! landblock riders (`mounts.md` §7, §9 MI11) and Decal SEDs (`decals.md` §2.1, §10
//! invariants 2–3).
//!
//! Skipped unless `TESSERA_GAME_DIR` points at an install. One pass reads every
//! landblock-info record of every cell archive. Invariants always fail the test; the census
//! counts of the specs are observations of one install, printed and compared, and fail the
//! test only with `TESSERA_CENSUS=strict`.

use std::collections::{BTreeMap, HashMap, HashSet};
use std::path::PathBuf;
use std::sync::atomic::{AtomicUsize, Ordering};
use std::sync::{Arc, Mutex, OnceLock};

use tessera_dat::{ArchiveKind, ArchiveSet};
use tessera_formats::gameplay::common::{Pid, PropertyAccess, enum_mapper_of};
use tessera_formats::gameplay::drama::{DramaKind, DramaLabels, DramaList, script_link_port};
use tessera_formats::gameplay::mounts::{MountLabels, RiderInstance};
use tessera_formats::gameplay::quests::{EntryGates, MapJournal};
use tessera_formats::gameplay::world_logic::{
    CustomCondition, LocalPlayer, PHASING_BOX_UNIQUEDB_KEY, PHASING_LINK_TYPE, PathMapVolume,
    PhasingBox, QuestStateScript, WorldLogicLabels,
};
use tessera_formats::landblock::{InstanceId, StaticEntityType};
use tessera_formats::script_table::instance::{
    InstanceScriptLabels, InstanceScriptTable, SCRIPT_LINK_TYPE, TreeNode, link_targets,
};
use tessera_formats::{
    Did, EnumMapper, LandblockInfo, PropertyDictionary, PropertyEntry, PropertySet, PropertyValue,
    StringSource,
};
use tessera_wstate::ObjectStore;

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
    const CHUNK: usize = 32;
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

/// Counters and bounded samples of invariant violations.
#[derive(Default)]
struct Stats {
    n: BTreeMap<&'static str, u64>,
    bad: BTreeMap<&'static str, (u64, Vec<String>)>,
}

impl Stats {
    fn add(&mut self, k: &'static str, v: u64) {
        *self.n.entry(k).or_default() += v;
    }
    fn inc(&mut self, k: &'static str) {
        self.add(k, 1);
    }
    fn fail(&mut self, k: &'static str, what: impl FnOnce() -> String) {
        let e = self.bad.entry(k).or_default();
        e.0 += 1;
        if e.1.len() < 5 {
            e.1.push(what());
        }
    }
    fn merge(&mut self, o: Stats) {
        for (k, v) in o.n {
            self.add(k, v);
        }
        for (k, (c, s)) in o.bad {
            let e = self.bad.entry(k).or_default();
            e.0 += c;
            for x in s {
                if e.1.len() < 5 {
                    e.1.push(x);
                }
            }
        }
    }
    fn get(&self, k: &str) -> u64 {
        self.n.get(k).copied().unwrap_or(0)
    }
}

struct World {
    store: ObjectStore,
    dict: PropertyDictionary,
    drama: DramaLabels,
    ist: InstanceScriptLabels,
    logic: WorldLogicLabels,
    mounts: MountLabels,
    box_did: Option<Did>,
    script_link_mapper: Option<Arc<EnumMapper>>,
    mount_appearance_mapper: Option<Arc<EnumMapper>>,
    decal_pids: Vec<u32>,
    link_name: Pid,
    work: Vec<(usize, tessera_dat::Entry)>,
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
        let drama = DramaLabels::resolve(&dict);
        let ist = InstanceScriptLabels::resolve(&dict);
        let logic = WorldLogicLabels::resolve(&dict);
        let mounts = MountLabels::resolve(&dict);
        eprintln!("unresolved drama labels: {:?}", drama.missing());
        eprintln!("unresolved instance-script labels: {:?}", ist.missing());
        eprintln!("unresolved world-logic labels: {:?}", logic.missing());
        eprintln!("unresolved mount labels: {:?}", mounts.missing());
        let uniquedb = data
            .did_map_by_label("UNIQUEDB")
            .expect("UNIQUEDB")
            .expect("UNIQUEDB present");
        let box_did = uniquedb.get(PHASING_BOX_UNIQUEDB_KEY).map(|e| e.value);
        let mapper =
            |pid: Pid| enum_mapper_of(&dict, pid).and_then(|m| data.enum_mapper(m).ok().flatten());
        let script_link_mapper = mapper(drama.drama.script_link);
        let mount_appearance_mapper = mapper(mounts.state.mount_appearance);
        let link_name = Pid::resolve(&dict, "LinkName");
        let decal_pids = dict
            .names()
            .iter()
            .filter(|(_, n)| n.starts_with("DecalVolume_"))
            .map(|(p, _)| *p)
            .collect();
        let set: &ArchiveSet = data.source();
        let mut work = Vec::new();
        for (i, m) in set.members().iter().enumerate() {
            if !matches!(m.id.kind, ArchiveKind::Cell(_)) {
                continue;
            }
            for e in m.archive.entries_range(0x8000_0000..=0x8FFF_FFFF) {
                let e = e.expect("directory");
                if (e.did >> 20) & 0xF == 2 {
                    work.push((i, e));
                }
            }
        }
        eprintln!("{} landblock-info records", work.len());
        Some(World {
            store,
            dict,
            drama,
            ist,
            logic,
            mounts,
            box_did,
            script_link_mapper,
            mount_appearance_mapper,
            decal_pids,
            link_name,
            work,
        })
    })
    .as_ref()
}

fn sorted(set: &PropertySet) -> Vec<&PropertyEntry> {
    let mut v: Vec<&PropertyEntry> = set.entries.iter().collect();
    v.sort_by_key(|e| e.pid);
    v
}

fn has_prefix(set: &PropertySet, dict: &PropertyDictionary, prefix: &str) -> bool {
    set.entries
        .iter()
        .any(|e| dict.name(e.pid).is_some_and(|n| n.starts_with(prefix)))
}

/// Every StringInfo of a value tree.
fn string_infos<'a>(v: &'a PropertyValue, out: &mut Vec<&'a tessera_formats::StringInfo>) {
    match v {
        PropertyValue::StringInfo(s) => out.push(s),
        _ => {
            for c in v.children() {
                string_infos(&c.value, out);
            }
        }
    }
}

fn walk_nodes<'a>(roots: &'a [TreeNode], f: &mut impl FnMut(&'a TreeNode)) {
    for r in roots {
        r.walk(f);
    }
}

fn scan(w: &World) -> Stats {
    let empty = MapJournal::default();
    let gates = EntryGates::default();
    let prereq = |_: Did| false;
    let parts = par_map(&w.work, |(i, e)| {
        let set: &ArchiveSet = w.store.game_data().source();
        let bytes = set.member(*i).archive.read_entry(e).expect("read");
        let info = LandblockInfo::parse_expecting(&bytes, Did(e.did), Some(e.stamp), &w.dict)
            .unwrap_or_else(|err| panic!("{:#010x}: {err}", e.did));
        let player = LocalPlayer {
            journal: &empty,
            gates: &gates,
            prerequisites_pass: &prereq,
        };
        landblock(w, &info, &player)
    });
    let mut total = Stats::default();
    for p in parts {
        total.merge(p);
    }
    total
}

#[allow(clippy::too_many_lines)]
fn landblock(w: &World, info: &LandblockInfo, player: &LocalPlayer<'_, MapJournal>) -> Stats {
    let mut s = Stats::default();
    let d = &w.drama.drama;
    let q = &w.logic.quest;
    let weenies: HashMap<InstanceId, &PropertySet> = info
        .weenies
        .iter()
        .map(|x| (x.iid, &x.properties))
        .collect();
    let mut sed_by_iid: HashMap<InstanceId, &tessera_formats::StaticEntity> = HashMap::new();
    let mut repeated: HashSet<InstanceId> = HashSet::new();
    for e in &info.entities {
        if let Some(iid) = e.iid
            && sed_by_iid.insert(iid, e).is_some()
        {
            repeated.insert(iid);
            s.inc("sed.repeated_iid");
        }
    }

    // ---- SEDs: Decal SEDs, own sets against instance sets (I2/IS8), drama on SEDs.
    for e in &info.entities {
        let own = e.properties.as_ref();
        let inst = e.iid.and_then(|i| weenies.get(&i).copied());
        if let (Some(own), Some(inst)) = (own, inst)
            && !e.iid.is_some_and(|i| repeated.contains(&i))
        {
            let inst_map: HashMap<u32, &PropertyValue> =
                inst.entries.iter().map(|x| (x.pid, &x.value)).collect();
            let subset = own
                .entries
                .iter()
                .all(|x| inst_map.get(&x.pid) == Some(&&x.value));
            if !subset {
                s.fail("IS8 SED set not a subset of its instance set", || {
                    format!("{:#x}", e.iid.map_or(0, |i| i.0))
                });
            } else if own.entries.len() == inst.entries.len() {
                s.inc("sed.own_equals_instance");
            } else {
                s.inc("sed.own_subset_instance");
            }
        }
        let decal_props =
            own.is_some_and(|o| o.entries.iter().any(|x| w.decal_pids.contains(&x.pid)));
        if e.kind() == Some(StaticEntityType::Decal) {
            s.inc("decal.seds");
            match e.flags {
                0x19E => s.inc("decal.flags_19e"),
                0x59E => s.inc("decal.flags_59e"),
                _ => s.inc("decal.flags_other"),
            }
            if e.object.is_some_and(|o| o.prefix() == 0x47) {
                s.inc("decal.object_47");
            }
            // DC3 / invariant 2: the instance set equals the SED's own set entry for entry.
            match (own, inst) {
                (Some(o), Some(i)) if sorted(o) == sorted(i) => s.inc("decal.instance_equal"),
                _ => s.fail("decals inv 2: instance set differs from own set", || {
                    format!("{:#x}", e.iid.map_or(0, |i| i.0))
                }),
            }
        } else if decal_props {
            s.inc("decal.props_on_non_decal_sed");
        }
        if let Some(o) = own
            && let Some(list) = DramaList::read(d, o)
        {
            s.inc("drama.sed_lists");
            // Invariant 10: an identical list in the instance set.
            let same = inst
                .and_then(|i| i.get_pid(d.list))
                .is_some_and(|v| Some(v) == o.get_pid(d.list));
            if !same {
                s.fail("drama inv 10: SED list differs from instance list", || {
                    format!("{:#x}", e.iid.map_or(0, |i| i.0))
                });
            }
            let _ = list;
        }
        // WL-I7: QuestState_* / PhasingBox_* of SED own sets equal the instance set's.
        if let (Some(o), Some(i)) = (own, inst) {
            for x in &i.entries {
                let name = w.dict.name(x.pid).unwrap_or("");
                if (name.starts_with("QuestState_") || name.starts_with("PhasingBox_"))
                    && o.get(x.pid) != Some(&x.value)
                {
                    s.fail("WL-I7 SED QuestState value differs", || name.to_owned());
                }
            }
        }
    }

    // ---- Links, cells and landblock set carry none of the groups (WL-I1, I1).
    let none_here = |set: &PropertySet| {
        has_prefix(set, &w.dict, "QuestState_")
            || has_prefix(set, &w.dict, "Drama_")
            || set.has(w.ist.instance_table)
    };
    for l in &info.links {
        if none_here(&l.properties) {
            s.fail("WL-I1 group property on a link set", || {
                format!("{:#x}", l.iid.0)
            });
        }
    }
    for c in info.cells() {
        if c.properties.as_ref().is_some_and(none_here) {
            s.fail("WL-I1 group property on a cell set", || {
                format!("cell {}", c.index)
            });
        }
    }
    if none_here(&info.properties) {
        s.fail("WL-I1 group property on a landblock set", || {
            format!("{}", info.did)
        });
    }

    // ---- Phasing (§4).
    let mut box_tests: HashMap<InstanceId, bool> = HashMap::new();
    let mut boxes: HashMap<InstanceId, (PhasingBox, bool)> = HashMap::new();
    for x in &info.weenies {
        let b = PhasingBox::read(q, &x.properties);
        let is_sed = sed_by_iid.contains_key(&x.iid);
        if b.has_check_list() {
            s.inc(if is_sed {
                "box.sed_sets"
            } else {
                "box.spawn_sets"
            });
            // WL-I3.
            for el in x.properties.elements(q.check_list) {
                let ok = el.value.as_struct().is_some_and(|st| {
                    st.has(q.quest_id) && st.has(q.quest_status) && st.has(q.quest_operator)
                });
                if !ok {
                    s.fail("WL-I3 check element without three fields", || {
                        format!("{:#x}", x.iid.0)
                    });
                }
            }
            for c in b.checks.iter().flatten() {
                s.inc("box.elements");
                let raw = c.status.0 >> 28;
                if !(1..=6).contains(&c.operator)
                    || !(raw == 1 || raw == 3)
                    || c.quest.prefix() != 0x70
                {
                    s.fail("WL-I3 check element values", || format!("{c:?}"));
                }
            }
            // WL-I5.
            if let Some(f) = b.false_draw {
                let expected = !b.true_draw.unwrap_or(false);
                if f != expected {
                    s.fail("WL-I5 False_Draw is not the negated True_Draw", || {
                        format!("{:#x}", x.iid.0)
                    });
                }
            }
            match b.combine {
                None => s.inc("box.op_absent"),
                Some(11) => s.inc("box.op_and"),
                Some(3) => s.inc("box.op_or"),
                Some(_) => s.inc("box.op_other"),
            }
            match b.true_draw {
                Some(true) => s.inc("box.true_draw_true"),
                Some(false) => s.inc("box.true_draw_false"),
                None => s.inc("box.true_draw_absent"),
            }
            let t = b.test(true, Some(player));
            s.inc(if t {
                "box.empty_journal_true"
            } else {
                "box.empty_journal_false"
            });
            box_tests.insert(x.iid, t);
            boxes.insert(x.iid, (b, is_sed));
        }
        if x.properties.has(q.phased_out) {
            s.fail("WL-I6 QuestState_PhasedOut stored", || {
                format!("{:#x}", x.iid.0)
            });
        }
        if has_prefix(&x.properties, &w.dict, "QuestState_") {
            s.inc("questate.sets");
        }
        let script = QuestStateScript::read(q, &x.properties);
        if x.properties.has(q.quest_id) {
            s.inc("questate.script_path_sets");
            let _ = script;
        }
        // WL-I8, WL-I9.
        for cc in CustomCondition::read_all(&w.logic, &x.properties) {
            s.inc("wiring.custom_conditions");
            if !cc.is_well_formed() {
                s.fail("WL-I8 custom condition", || {
                    format!("{:#x}: {:?}", x.iid.0, cc.link)
                });
            }
        }
        if let Some(v) = PathMapVolume::read(&w.logic.travel, &x.properties, &w.dict) {
            s.inc("pathmap.volumes");
            if !v.is_ordered() {
                s.fail("WL-I9 path-map box min > max", || format!("{v:?}"));
            }
        }
    }
    // WL-I2: every SED placing the box object has a check list.
    for e in &info.entities {
        if w.box_did.is_some() && e.object == w.box_did {
            s.inc("box.seds");
            let ok = e.iid.is_some_and(|i| boxes.contains_key(&i));
            if !ok {
                s.fail("WL-I2 box SED without a check list", || {
                    format!("{:#x}", e.iid.map_or(0, |i| i.0))
                });
            }
        }
    }
    for l in &info.links {
        if l.link_type != PHASING_LINK_TYPE {
            continue;
        }
        s.inc("phasing.links");
        // WL-I4.
        if boxes.contains_key(&l.to_iid) {
            s.fail("WL-I4 box is end 1", || format!("{:#x}", l.iid.0));
        }
        if l.to_iid.0 != 0 && l.from_iid.0 != 0 {
            s.inc("phasing.complete_links");
            match box_tests.get(&l.from_iid) {
                None => s.fail("WL-I4 end 2 has no check list", || {
                    format!("{:#x}", l.from_iid.0)
                }),
                Some(&shown) => {
                    let sed = sed_by_iid
                        .get(&l.to_iid)
                        .is_some_and(|e| e.kind() == Some(StaticEntityType::Physics));
                    let k = match (sed, shown) {
                        (true, true) => "phasing.sed_targets_shown",
                        (true, false) => "phasing.sed_targets_out",
                        (false, true) => "phasing.spawn_targets_shown",
                        (false, false) => "phasing.spawn_targets_out",
                    };
                    s.inc(k);
                }
            }
        }
    }

    // ---- Instance scripts (§2–§3, §10).
    let ist_counts = |set: &PropertySet, owner: InstanceId, is_sed: bool, s: &mut Stats| {
        let Some(v) = set.get_pid(w.ist.instance_table) else {
            return;
        };
        s.inc(if is_sed {
            "ist.sed_iid_sets"
        } else {
            "ist.server_only_sets"
        });
        let Some(ist) = InstanceScriptTable::parse(&w.ist, v) else {
            s.fail("IS1 IST is not a struct", || format!("{:#x}", owner.0));
            return;
        };
        if ist.skipped_entries > 0 || ist.replaced > 0 {
            s.fail("IS2 script entry without key or repeated key", || {
                format!("{:#x}", owner.0)
            });
        }
        s.add("ist.entries", ist.scripts.len() as u64);
        for sc in &ist.scripts {
            if sc.failed_roots > 0 {
                s.fail("IS3 node failed to build", || {
                    format!("{:#x} key {:#x}", owner.0, sc.key)
                });
            }
            walk_nodes(&sc.roots, &mut |n| {
                s.inc("ist.nodes");
                if n.kind.0 == 5 {
                    s.inc("ist.run_nodes");
                }
                for t in &n.targets {
                    s.inc("ist.targets");
                    let ok_type = matches!(t.target_type, Some(7 | 13));
                    if t.port_type != 6 || !t.payload.is_empty() || !ok_type {
                        s.fail("IS4 target shape", || format!("{:#x}: {t:?}", owner.0));
                    }
                    if t.target_type == Some(7) {
                        match t.link_name.as_deref() {
                            Some(name) if !name.is_empty() => {
                                s.inc("ist.link_targets");
                                let by_end2 = link_targets(&info.links, owner, name);
                                if !by_end2.is_empty() {
                                    s.inc("ist.link_match_end2");
                                } else if info.links.iter().any(|l| {
                                    l.link_type == SCRIPT_LINK_TYPE
                                        && l.to_iid == owner
                                        && l.name == name
                                }) {
                                    s.inc("ist.link_match_end1_only");
                                } else {
                                    s.inc("ist.link_match_none");
                                }
                                // IS5: every matching link (any end) is a Script link.
                                for l in &info.links {
                                    if l.name == name
                                        && (l.from_iid == owner || l.to_iid == owner)
                                        && l.link_type != SCRIPT_LINK_TYPE
                                    {
                                        s.fail("IS5 matching link is not a Script link", || {
                                            format!("{:#x} type {}", owner.0, l.link_type)
                                        });
                                    }
                                }
                            }
                            _ => s.fail("IS4 Link target without a name", || {
                                format!("{:#x}", owner.0)
                            }),
                        }
                    }
                }
            });
        }
    };
    for x in &info.weenies {
        ist_counts(
            &x.properties,
            x.iid,
            sed_by_iid.contains_key(&x.iid),
            &mut s,
        );
    }

    // ---- Drama in instance sets (§2–§4, §12).
    let actions_allowed: HashMap<u32, HashSet<u32>> = [d.action, d.action_instant, d.action_idle]
        .iter()
        .filter_map(|p| {
            let id = p.id()?;
            Some((
                id,
                w.dict.definition(id)?.children.iter().copied().collect(),
            ))
        })
        .collect();
    for x in &info.weenies {
        let set = &x.properties;
        let is_sed = sed_by_iid.contains_key(&x.iid);
        let Some(list) = DramaList::read(d, set) else {
            if set.has(d.list) {
                s.fail("drama: list is not an array", || format!("{:#x}", x.iid.0));
            }
            continue;
        };
        s.inc(if is_sed {
            "drama.instance_lists_sed"
        } else {
            "drama.instance_lists_spawn"
        });
        if list.has_duplicate_names() {
            s.fail("drama inv 1: duplicate names", || format!("{:#x}", x.iid.0));
        }
        if let Some(v) = set.get_pid(d.list)
            && v.as_array().is_some_and(|a| a.null_entries() > 0)
        {
            s.fail("drama inv 2: null entry", || format!("{:#x}", x.iid.0));
        }
        for dr in &list.dramas {
            match dr.kind {
                DramaKind::Named => s.inc("drama.named"),
                DramaKind::Instant => s.inc("drama.instant"),
                DramaKind::Idle => s.inc("drama.idle"),
                DramaKind::Other(_) => s.fail("drama inv 1: unknown element kind", || {
                    format!("{:#x}", x.iid.0)
                }),
            }
            if dr.name().is_none() {
                s.fail("drama inv 1: element without Drama_Name", || {
                    format!("{:#x}", x.iid.0)
                });
            }
            for a in dr.actions() {
                s.inc("drama.actions");
                let Some(raw) = a.raw else {
                    s.fail("drama inv 2: action is not a struct", || {
                        format!("{:#x}", x.iid.0)
                    });
                    continue;
                };
                if let Some(allowed) = a.element_pid.and_then(|p| actions_allowed.get(&p)) {
                    for f in &raw.fields {
                        if !allowed.contains(&f.pid) {
                            s.fail("drama inv 2: field not in the action's definition", || {
                                format!("{:#x}: {}", x.iid.0, w.dict.name(f.pid).unwrap_or("?"))
                            });
                        }
                    }
                }
                let f = a.facets();
                for dialog in f
                    .dialog
                    .iter()
                    .chain(f.direct_actors.iter().filter_map(|da| da.dialog.as_ref()))
                {
                    if !dialog.dialog_type.is_some_and(|t| t <= 10) {
                        s.fail("drama inv 3: dialog type", || format!("{:#x}", x.iid.0));
                    }
                }
                if let Some(k) = f.script_link.filter(|&k| k != 0) {
                    s.inc("drama.script_link_facets");
                    let label = w.script_link_mapper.as_ref().and_then(|m| m.name(k));
                    if label.and_then(script_link_port).is_none() {
                        s.fail("drama inv 7: script-link label", || {
                            format!("key {k}: {label:?}")
                        });
                    }
                }
                if let Some(da) = &f.direct_actors {
                    s.inc("drama.director_facets");
                    if da.actors.is_some_and(|m| m >> 6 != 0) {
                        s.fail("drama inv 8: actor bit above 5", || {
                            format!("{:#x}", x.iid.0)
                        });
                    }
                }
                if let Some(h) = &f.ai_hints {
                    let ok = |bits: &[u32], width: u32| bits.last().map_or(0, |b| b + 1) == width;
                    if !ok(&h.set, h.widths.0) || !ok(&h.mask, h.widths.1) {
                        s.fail("drama inv 9: AI hint width", || format!("{h:?}"));
                    }
                }
                if let Some(name) = &f.patrol_start {
                    s.inc("drama.patrol_start");
                    let matched = info.links.iter().any(|l| {
                        matches!(l.link_type, 1 | 2 | 11)
                            && l.from_iid == x.iid
                            && l.properties.string(w.link_name) == Some(name.as_str())
                    });
                    if matched {
                        s.inc("drama.patrol_start_matched");
                    }
                }
            }
        }
        // Invariant 4: every drama StringInfo is a table reference.
        let mut sis = Vec::new();
        if let Some(v) = set.get_pid(d.list) {
            string_infos(v, &mut sis);
        }
        for si in sis {
            s.inc("drama.string_infos");
            match si.source {
                StringSource::Table { token: 0, table } if table.is_null() => {
                    s.inc("drama.string_infos_null")
                }
                StringSource::Table { .. } => {}
                StringSource::Literal(_) => s.fail("drama inv 4: literal StringInfo", || {
                    format!("{:#x}", x.iid.0)
                }),
            }
        }
    }

    // ---- Riders (§7.1, MI11).
    for x in &info.weenies {
        let Some(r) = RiderInstance::read(&w.mounts, &x.properties) else {
            continue;
        };
        let physics_sed = sed_by_iid
            .get(&x.iid)
            .is_some_and(|e| e.kind() == Some(StaticEntityType::Physics));
        s.inc(if sed_by_iid.contains_key(&x.iid) {
            if physics_sed {
                "rider.physics_seds"
            } else {
                "rider.other_seds"
            }
        } else {
            "rider.spawn_sets"
        });
        if let Some(t) = r.rider_type
            && !sed_by_iid.contains_key(&x.iid)
            && t > 7
        {
            s.fail("MI11 rider type above 7", || format!("{:#x}: {t}", x.iid.0));
        }
        if let (Some(a), Some(m)) = (r.mount_appearance, &w.mount_appearance_mapper)
            && !sed_by_iid.contains_key(&x.iid)
            && a != 0
            && m.name(a).is_none()
        {
            s.fail("MI11 mount appearance not a mapper key", || format!("{a}"));
        }
    }
    s
}

fn report(s: &Stats, rows: &[(&str, &str, u64)]) {
    let mut differ = Vec::new();
    for (section, k, spec) in rows {
        let got = s.get(k);
        let mark = if got == *spec { "  " } else { "!!" };
        eprintln!("  {mark} [{section}] {k}: spec {spec}, measured {got}");
        if got != *spec {
            differ.push(*k);
        }
    }
    eprintln!("{} census rows, {} differ", rows.len(), differ.len());
    if strict() {
        assert!(differ.is_empty(), "census differs: {differ:?}");
    }
}

fn stats() -> Option<&'static Stats> {
    static S: OnceLock<Option<Stats>> = OnceLock::new();
    S.get_or_init(|| {
        let w = world()?;
        let t0 = std::time::Instant::now();
        let s = scan(w);
        eprintln!("scanned {} records in {:.1?}", w.work.len(), t0.elapsed());
        Some(s)
    })
    .as_ref()
}

fn assert_invariants(s: &Stats, prefixes: &[&str]) {
    let bad: Vec<_> = s
        .bad
        .iter()
        .filter(|(k, _)| prefixes.iter().any(|p| k.starts_with(p)))
        .collect();
    for (k, (n, samples)) in &bad {
        eprintln!("  FAIL {k}: {n} (e.g. {samples:?})");
    }
    assert!(bad.is_empty(), "{} invariants violated", bad.len());
}

#[test]
fn drama_lists() {
    let Some(s) = stats() else { return };
    report(
        s,
        &[
            ("npc-drama §2.1", "drama.instance_lists_spawn", 27_169),
            ("npc-drama §2.1", "drama.instance_lists_sed", 101),
            ("npc-drama §2.1", "drama.sed_lists", 101),
            ("npc-drama §3.2", "drama.named", 56_837),
            ("npc-drama §3.2", "drama.instant", 3_560),
            ("npc-drama §3.2", "drama.idle", 317),
            ("npc-drama §4.2", "drama.string_infos_null", 35),
            ("npc-drama §4.5", "drama.patrol_start", 10_707),
            ("npc-drama §4.5", "drama.patrol_start_matched", 10_309),
            ("npc-drama §4.6.4", "drama.director_facets", 10_330),
        ],
    );
    assert_invariants(s, &["drama"]);
    // Invariant 11: at least 95 % of patrol starts match a patrol link.
    let all = s.get("drama.patrol_start");
    assert!(all == 0 || s.get("drama.patrol_start_matched") * 100 >= all * 95);
}

#[test]
fn instance_scripts() {
    let Some(s) = stats() else { return };
    report(
        s,
        &[
            ("instance-scripts §2.2", "ist.sed_iid_sets", 275),
            ("instance-scripts §2.2", "ist.server_only_sets", 82_925),
            ("instance-scripts §2.1", "sed.repeated_iid", 9),
            (
                "instance-scripts §2.1",
                "sed.own_equals_instance",
                2_298_059,
            ),
            ("instance-scripts §2.1", "sed.own_subset_instance", 955_794),
            ("instance-scripts §3.4", "ist.entries", 104_531),
            ("instance-scripts §3.4", "ist.targets", 184_221),
            ("instance-scripts §3.5", "ist.link_targets", 184_214),
            ("instance-scripts §3.5", "ist.link_match_end2", 182_928),
            ("instance-scripts §3.5", "ist.link_match_end1_only", 50),
            ("instance-scripts §3.5", "ist.link_match_none", 1_236),
        ],
    );
    assert_invariants(s, &["IS"]);
}

#[test]
fn world_logic() {
    let Some(s) = stats() else { return };
    report(
        s,
        &[
            ("world-logic §4.1", "box.seds", 1_001),
            ("world-logic §4.1", "box.sed_sets", 1_001),
            ("world-logic §4.1", "box.spawn_sets", 6_039),
            ("world-logic §4.8", "box.elements", 9_725),
            ("world-logic §4.8", "box.op_absent", 6_238),
            ("world-logic §4.8", "box.op_and", 556),
            ("world-logic §4.8", "box.op_or", 246),
            ("world-logic §4.8", "box.true_draw_true", 6_576),
            ("world-logic §4.8", "box.true_draw_false", 431),
            ("world-logic §4.8", "box.true_draw_absent", 33),
            ("world-logic §4.2", "phasing.links", 23_376),
            ("world-logic §4.2", "phasing.complete_links", 22_942),
            ("world-logic §4.9", "box.empty_journal_true", 1_199),
            ("world-logic §4.9", "box.empty_journal_false", 5_841),
            ("world-logic §4.9", "phasing.sed_targets_shown", 2_164),
            ("world-logic §4.9", "phasing.sed_targets_out", 3_505),
            ("world-logic §4.9", "phasing.spawn_targets_shown", 3_488),
            ("world-logic §4.9", "phasing.spawn_targets_out", 13_785),
            ("world-logic §2", "questate.sets", 7_943),
            ("world-logic §5.6", "questate.script_path_sets", 741),
            ("world-logic §6.5", "wiring.custom_conditions", 9_505),
            ("world-logic §7.4", "pathmap.volumes", 608),
        ],
    );
    assert_invariants(s, &["WL"]);
}

#[test]
fn landblock_riders() {
    let Some(s) = stats() else { return };
    report(
        s,
        &[
            ("mounts §7.1", "rider.spawn_sets", 1_361),
            ("mounts §7.1", "rider.physics_seds", 188),
        ],
    );
    assert_invariants(s, &["MI"]);
}

#[test]
fn decal_seds() {
    let Some(s) = stats() else { return };
    report(
        s,
        &[
            ("decals §2.1", "decal.seds", 16_511),
            ("decals §2.1", "decal.flags_19e", 5_047),
            ("decals §2.1", "decal.flags_59e", 11_464),
            ("decals §2.1", "decal.instance_equal", 16_511),
            ("decals §2.1", "decal.object_47", 16_511),
            ("decals §10 inv 3", "decal.props_on_non_decal_sed", 1),
        ],
    );
    assert_invariants(s, &["decals"]);
}
