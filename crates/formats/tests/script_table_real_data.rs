// SPDX-License-Identifier: GPL-3.0-or-later
//! `script-table.md` against a user's own install (read-only):
//!
//! - every script table (0x07) and scriptlet (0x0C) decodes to its last byte, apart from the
//!   failure category of §10.3 (CinematicCamera data), with the node census of §10;
//! - the observed invariants SI1–SI9 hold (`first_unusual_field`);
//! - idle, walk and run clips resolve for every playable species and body type (the race
//!   table, §8.3, §10.4) and for the creatures placed by landblock weenie objects (§10.4).
//!
//! Skipped unless `TESSERA_GAME_DIR` points at an install. A release build visits every
//! record; a debug build takes a sample unless `TESSERA_FULL=1`. Run with
//! `cargo test --release -p tessera-formats --test script_table_real_data -- --nocapture`.
//! Only counts, DIDs and error kinds are printed; no game data leaves the install.

use std::collections::{BTreeMap, BTreeSet, HashSet};
use std::path::PathBuf;
use std::sync::atomic::{AtomicUsize, Ordering};
use std::sync::{Mutex, OnceLock};

use tessera_dat::{ArchiveKind, ArchiveSet, Entry, SetOptions};
use tessera_formats::avatar::{RacePids, RaceTable};
use tessera_formats::script_table::resolve::{
    ClipResolution, ClipResolver, DeformerCandidate, MotionState, ResolveIssue, ScriptVocabulary,
    collect_entity_scripts,
};
use tessera_formats::script_table::{NodeCensus, NodeKind};
use tessera_formats::{
    DataIdMap, Did, EnumMapper, FormatError, LandblockInfo, PropertyDictionary, PropertyNames,
    Record, ScriptTable, Scriptlet,
};

const PROPERTY_DICTIONARY: u32 = 0x3400_0000;
const ROOT_DID_MAP: u32 = 0x2800_0000;
/// Largest share of records allowed to deviate from an observed invariant (0.1 %).
const MAX_UNUSUAL_RATE: f64 = 0.001;
/// Largest share of tables allowed in the §10.3 failure category (2 of 7,526 observed).
const MAX_CAMERA_FAILURE_RATE: f64 = 0.001;

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

/// Runs `f` over `items` on every core; results come back in no fixed order.
fn par_each<T: Sync, R: Send>(items: &[T], f: impl Fn(&T) -> R + Sync) -> Vec<R> {
    let next = AtomicUsize::new(0);
    let out = Mutex::new(Vec::with_capacity(items.len()));
    let threads = std::thread::available_parallelism().map_or(4, |n| n.get());
    std::thread::scope(|s| {
        for _ in 0..threads {
            s.spawn(|| {
                let mut local = Vec::new();
                loop {
                    let i = next.fetch_add(16, Ordering::Relaxed);
                    if i >= items.len() {
                        break;
                    }
                    for item in &items[i..(i + 16).min(items.len())] {
                        local.push(f(item));
                    }
                }
                out.lock().unwrap().extend(local);
            });
        }
    });
    out.into_inner().unwrap()
}

/// Every (member index, entry) with type prefix `prefix`; sampled in debug builds.
fn entries_with_prefix(set: &ArchiveSet, prefix: u8) -> Vec<(usize, Entry)> {
    let lo = u32::from(prefix) << 24;
    let mut out = Vec::new();
    for (i, m) in set.members().iter().enumerate() {
        out.extend(
            m.archive
                .entries_range(lo..=lo | 0x00FF_FFFF)
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

fn dids_with_prefix(set: &ArchiveSet, prefix: u8) -> HashSet<u32> {
    let lo = u32::from(prefix) << 24;
    let mut out = HashSet::new();
    for m in set.members() {
        out.extend(
            m.archive
                .entries_range(lo..=lo | 0x00FF_FFFF)
                .map(|e| e.expect("directory walk").did),
        );
    }
    out
}

/// Short error category, as `script-table.md` §10.3 and §4.5 name them.
fn category(e: &FormatError) -> String {
    match e.root_cause() {
        FormatError::Truncated { .. } => "truncated".into(),
        FormatError::TrailingBytes { .. } => "trailing bytes".into(),
        FormatError::Unsupported { what, value, .. } => format!("unsupported {what} {value}"),
        FormatError::Malformed { what, reason, .. } => format!("malformed {what}: {reason}"),
        FormatError::InvalidBool { .. } => "invalid bool8".into(),
        FormatError::UnknownProperty { .. } => "unknown PID".into(),
        FormatError::DepthLimit { .. } => "depth limit".into(),
        FormatError::MemoryLimit { .. } => "memory limit".into(),
        other => format!("{other:?}").chars().take(60).collect(),
    }
}

fn fetch(
    set: &'static ArchiveSet,
) -> impl FnMut(Did) -> Result<Option<Vec<u8>>, tessera_dat::Error> {
    move |d: Did| match set.locate(d.0)? {
        Some(l) => set.read_located(&l).map(Some),
        None => Ok(None),
    }
}

fn mapper(set: &ArchiveSet, did: Did) -> Option<EnumMapper> {
    let payload = set.read(did.0).ok()?;
    EnumMapper::parse_expecting(&payload, did).ok()
}

fn vocabulary(set: &'static ArchiveSet) -> ScriptVocabulary {
    ScriptVocabulary::from_data(dictionary(set), |d| mapper(set, d)).expect("vocabulary")
}

struct Totals {
    records: usize,
    decoded: usize,
    census: NodeCensus,
    by_kind: BTreeMap<u8, u64>,
    failed: BTreeMap<String, (usize, u32)>,
    unusual: BTreeMap<&'static str, (usize, u32)>,
}

fn add_census(t: &mut Totals, c: &NodeCensus) {
    t.census.nodes += c.nodes;
    t.census.ports += c.ports;
    t.census.targets += c.targets;
    t.census.animations += c.animations;
    t.census.max_depth = t.census.max_depth.max(c.max_depth);
    for (k, n) in &c.by_kind {
        *t.by_kind.entry(k.0).or_default() += n;
    }
}

fn run_census<T: Send>(
    set: &'static ArchiveSet,
    prefix: u8,
    decode: impl Fn(u32, &[u8]) -> Result<T, FormatError> + Sync,
    census: impl Fn(&T) -> NodeCensus,
    unusual: impl Fn(&T) -> Option<&'static str>,
) -> Totals {
    let work = entries_with_prefix(set, prefix);
    let outcomes = par_each(&work, |(i, e)| {
        let payload = set
            .member(*i)
            .archive
            .read_entry(e)
            .map_err(|err| (e.did, format!("read: {err}")))?;
        decode(e.did, &payload)
            .map(|r| (e.did, r))
            .map_err(|err| (e.did, category(&err)))
    });
    let mut t = Totals {
        records: outcomes.len(),
        decoded: 0,
        census: NodeCensus::default(),
        by_kind: BTreeMap::new(),
        failed: BTreeMap::new(),
        unusual: BTreeMap::new(),
    };
    for o in outcomes {
        match o {
            Ok((did, rec)) => {
                t.decoded += 1;
                add_census(&mut t, &census(&rec));
                if let Some(why) = unusual(&rec) {
                    t.unusual.entry(why).or_insert((0, did)).0 += 1;
                }
            }
            Err((did, why)) => t.failed.entry(why).or_insert((0, did)).0 += 1,
        }
    }
    t
}

fn report(what: &str, t: &Totals) {
    eprintln!(
        "{what}: {} records, {} decoded to the last byte; {} nodes, {} animation nodes, {} \
         ports, {} targets, deepest nesting {}",
        t.records,
        t.decoded,
        t.census.nodes,
        t.census.animations,
        t.census.ports,
        t.census.targets,
        t.census.max_depth
    );
    for (why, (n, did)) in &t.failed {
        eprintln!("  failed: {n} × {why} (e.g. {did:#010x})");
    }
    for (why, (n, did)) in &t.unusual {
        eprintln!("  unusual: {n} × {why} (e.g. {did:#010x})");
    }
    let kinds: Vec<String> = t.by_kind.iter().map(|(k, n)| format!("{k}:{n}")).collect();
    eprintln!("  nodes per kind: {}", kinds.join(" "));
}

#[test]
fn script_tables_decode() {
    let Some(set) = open_set() else { return };
    let dict = dictionary(set);
    let t = run_census(
        set,
        0x07,
        |did, p| ScriptTable::parse_expecting(p, did, dict),
        ScriptTable::census,
        ScriptTable::first_unusual_field,
    );
    report("script tables (0x07)", &t);
    assert!(t.records > 0);
    // The only accepted failure category is §10.3: CinematicCamera node data.
    let camera = "unsupported script node data (kind 7) 7";
    for (why, (n, did)) in &t.failed {
        assert_eq!(why, camera, "{n} tables fail otherwise (e.g. {did:#010x})");
    }
    let failed: usize = t.failed.values().map(|v| v.0).sum();
    assert!(
        failed as f64 <= MAX_CAMERA_FAILURE_RATE * t.records as f64,
        "{failed} of {} tables fail",
        t.records
    );
    let odd: usize = t.unusual.values().map(|v| v.0).sum();
    assert!(
        odd as f64 <= MAX_UNUSUAL_RATE * t.decoded as f64,
        "{odd} unusual"
    );
    assert!(t.census.max_depth < tessera_formats::script_table::MAX_NODE_DEPTH);
}

#[test]
fn scriptlets_decode() {
    let Some(set) = open_set() else { return };
    let dict = dictionary(set);
    let t = run_census(
        set,
        0x0C,
        |did, p| Scriptlet::parse_expecting(p, did, dict),
        Scriptlet::census,
        Scriptlet::first_unusual_field,
    );
    report("scriptlets (0x0C)", &t);
    assert!(t.records > 0);
    assert!(t.failed.is_empty(), "every scriptlet decodes (§10.1)");
    let odd: usize = t.unusual.values().map(|v| v.0).sum();
    assert!(
        odd as f64 <= MAX_UNUSUAL_RATE * t.decoded as f64,
        "{odd} unusual"
    );
    assert_eq!(
        t.by_kind.get(&NodeKind::CINEMATIC_CAMERA.0),
        None,
        "no camera node in scriptlets"
    );
}

/// `script-runtime.md` §30.1 and §30.5 (R43): every While node decodes to its typed fields,
/// and the census of this install (41 nodes in 17 tables, `Names` empty, `Inverse` 0, one
/// Default port and no other port each) is printed and compared; a census difference fails
/// only with `TESSERA_CENSUS=strict` (a data update may change it).
#[test]
fn while_nodes_decode_and_follow_the_census() {
    use tessera_formats::script_table::{NodeData, PortType};
    let Some(set) = open_set() else { return };
    let dict = dictionary(set);
    let work = entries_with_prefix(set, 0x07);
    // (tables with a While, nodes, untyped, with names, inverted, other than one Default
    // port and nothing else)
    let per = par_each(&work, |(i, e)| {
        let Ok(payload) = set.member(*i).archive.read_entry(e) else {
            return [0usize; 6];
        };
        let Ok(t) = ScriptTable::parse_expecting(&payload, e.did, dict) else {
            return [0; 6];
        };
        let mut c = [0usize; 6];
        for sc in &t.scripts {
            for r in &sc.roots {
                r.walk(&mut |n, _| {
                    if n.kind != NodeKind::WHILE {
                        return;
                    }
                    c[1] += 1;
                    match &n.data {
                        NodeData::While(w) => {
                            c[3] += usize::from(!w.names.is_empty());
                            c[4] += usize::from(w.inverse);
                        }
                        _ => c[2] += 1,
                    }
                    let defaults = n.ports_of(PortType::DEFAULT).count();
                    c[5] += usize::from(defaults != 1 || n.ports.len() != 1);
                });
            }
        }
        c[0] = usize::from(c[1] > 0);
        c
    });
    let mut sum = [0usize; 6];
    for c in per {
        for (a, b) in sum.iter_mut().zip(c) {
            *a += b;
        }
    }
    eprintln!(
        "While nodes: {} in {} tables; untyped {}, with names {}, inverted {}, other ports {}",
        sum[1], sum[0], sum[2], sum[3], sum[4], sum[5]
    );
    assert_eq!(sum[2], 0, "every While node decodes to its fields");
    let census = [17, 41, 0, 0, 0, 0];
    if sum != census {
        eprintln!("census differs from script-runtime.md §30.5: {sum:?} against {census:?}");
        assert!(
            !std::env::var_os("TESSERA_CENSUS").is_some_and(|v| v == "strict"),
            "census (strict)"
        );
    }
}

/// The (kind, port type) pairs of every port of the decoded tables and scriptlets
/// (`instance-scripts.md` §4.5.1 table, a census of this install).
const PORT_PAIRS: [(u8, u8); 32] = [
    (2, 6),
    (4, 5),
    (4, 6),
    (5, 9),
    (5, 14),
    (6, 6),
    (10, 6),
    (11, 6),
    (12, 5),
    (12, 6),
    (12, 22),
    (12, 28),
    (14, 10),
    (14, 11),
    (15, 6),
    (17, 6),
    (19, 6),
    (20, 6),
    (22, 6),
    (25, 6),
    (26, 6),
    (26, 30),
    (27, 5),
    (27, 6),
    (36, 6),
    (50, 6),
    (54, 6),
    (57, 6),
    (63, 6),
    (63, 22),
    (63, 24),
    (65, 6),
];

/// IS13 (`instance-scripts.md` §4.5.1; a census check of this install, not an invariant):
/// every port of every node of the decoded 0x07 and 0x0C records has a (kind, port type)
/// pair of the §4.5.1 table. Ports outside it are counted and reported; the count fails the
/// test only with `TESSERA_CENSUS=strict`. Decoding does not depend on the table.
#[test]
fn port_pairs_follow_the_census() {
    let Some(set) = open_set() else { return };
    let dict = dictionary(set);
    let known = |k: u8, t: u8| PORT_PAIRS.contains(&(k, t));
    let mut outside: BTreeMap<(u8, u8), usize> = BTreeMap::new();
    let mut ports = 0usize;
    for prefix in [0x07u8, 0x0C] {
        let work = entries_with_prefix(set, prefix);
        let per = par_each(&work, |(i, e)| {
            let mut out: Vec<(u8, u8)> = Vec::new();
            let mut n = 0usize;
            let Ok(payload) = set.member(*i).archive.read_entry(e) else {
                return (0, out);
            };
            let roots: Vec<tessera_formats::script_table::Node> = if prefix == 0x07 {
                match ScriptTable::parse_expecting(&payload, e.did, dict) {
                    Ok(t) => t.scripts.into_iter().flat_map(|s| s.roots).collect(),
                    Err(_) => return (0, out),
                }
            } else {
                match Scriptlet::parse_expecting(&payload, e.did, dict) {
                    Ok(s) => s.roots,
                    Err(_) => return (0, out),
                }
            };
            for r in &roots {
                r.walk(&mut |node, _| {
                    for p in &node.ports {
                        n += 1;
                        if !known(node.kind.0, p.port_type.0) {
                            out.push((node.kind.0, p.port_type.0));
                        }
                    }
                });
            }
            (n, out)
        });
        for (n, out) in per {
            ports += n;
            for pair in out {
                *outside.entry(pair).or_default() += 1;
            }
        }
    }
    let total: usize = outside.values().sum();
    eprintln!("ports: {ports}; outside the §4.5.1 census: {total} {outside:?}");
    if total > 0 {
        assert!(
            !std::env::var_os("TESSERA_CENSUS").is_some_and(|v| v == "strict"),
            "IS13 census (strict)"
        );
    }
}

/// `world-logic.md` §5.2.1 over every decoded table: WL-I11 (a table whose `Startup` holds
/// a GameCallback driven from `QuestState_QuestID` has a quest-state test built only of the
/// parts) and WL-I14 (every Switch on `QuestState_Test` has value ports for 0, 1 and 2 and
/// no Default port) always fail the test; the tables found (eight, two of them sharing a
/// combination of parts) are a census compared under `TESSERA_CENSUS=strict`.
#[test]
fn quest_state_tables_follow_their_parts() {
    use tessera_formats::gameplay::world_logic::{QuestStateTest, WorldLogicLabels};
    use tessera_formats::script_table::{NodeData, PortPayload, PortType};
    let Some(set) = open_set() else { return };
    let dict = dictionary(set);
    let labels = WorldLogicLabels::resolve(dict);
    let startup = vocabulary(set).startup_key;
    let (Some(quest_id), Some(test)) = (labels.quest.quest_id.id(), labels.quest.test.id()) else {
        eprintln!("QuestState labels missing; skipping");
        return;
    };
    let work = entries_with_prefix(set, 0x07);
    let per = par_each(&work, |(i, e)| {
        let Ok(payload) = set.member(*i).archive.read_entry(e) else {
            return (None, Vec::new(), 0usize);
        };
        let Ok(t) = ScriptTable::parse_expecting(&payload, e.did, dict) else {
            return (None, Vec::new(), 0);
        };
        let mut bad = Vec::new();
        let mut switches = 0usize;
        for sc in &t.scripts {
            for r in &sc.roots {
                r.walk(&mut |n, _| {
                    if n.kind != NodeKind::SWITCH {
                        return;
                    }
                    let NodeData::Switch(sw) = &n.data else {
                        return;
                    };
                    if sw.property != test {
                        return;
                    }
                    switches += 1;
                    let mut values: Vec<f64> = n
                        .ports_of(PortType::SWITCH_VALUE)
                        .flat_map(|p| match &p.payload {
                            PortPayload::SwitchCase(c) => c
                                .values
                                .iter()
                                .flatten()
                                .filter_map(|v| v.value.as_f64())
                                .collect::<Vec<_>>(),
                            _ => Vec::new(),
                        })
                        .collect();
                    values.sort_by(f64::total_cmp);
                    if values != [0.0, 1.0, 2.0] || n.ports_of(PortType::DEFAULT).next().is_some() {
                        bad.push(format!("WL-I14 {:#010x}", e.did));
                    }
                });
            }
        }
        let driven_from_quest = t.script(startup).is_some_and(|sc| {
            let mut hit = false;
            for r in &sc.roots {
                r.walk(&mut |n, _| {
                    hit |= matches!(n.data, NodeData::GameCallback(_))
                        && n.base.driven.iter().any(|&(_, src)| src == quest_id);
                });
            }
            hit
        });
        let parts = if driven_from_quest {
            match QuestStateTest::read(&t, &labels.quest, &labels.callback, startup, None) {
                Some(q) => Some((e.did, q)),
                None => {
                    bad.push(format!("WL-I11 {:#010x}", e.did));
                    None
                }
            }
        } else {
            None
        };
        (parts, bad, switches)
    });
    let mut tables = Vec::new();
    let mut bad = Vec::new();
    let mut switches = 0;
    for (p, b, n) in per {
        tables.extend(p);
        bad.extend(b);
        switches += n;
    }
    tables.sort_by_key(|(d, _)| *d);
    for (d, q) in &tables {
        eprintln!(
            "  {d:#010x}: delay {:?}, quest-ID guard {}, override {}, phased-out guard {}, \
             entries {}, true {:?}, false {:?}",
            q.delay,
            q.quest_id_guard,
            q.test_override,
            q.phased_out_guard,
            q.entries,
            q.true_leaf,
            q.false_leaf
        );
    }
    eprintln!(
        "quest-state tables: {}; Switches on QuestState_Test: {switches}",
        tables.len()
    );
    assert!(bad.is_empty(), "{bad:?}");
    if (tables.len(), switches) != (8, 4) {
        eprintln!("census differs from world-logic.md §5.2.1 (8 tables, 4 override Switches)");
        assert!(
            !std::env::var_os("TESSERA_CENSUS").is_some_and(|v| v == "strict"),
            "census (strict)"
        );
    }
}

/// The race table's player avatars (`skeleton-anim.md` §2.2).
fn race_table(set: &'static ArchiveSet) -> RaceTable {
    let dict = dictionary(set);
    let root = DataIdMap::parse_expecting(&set.read(ROOT_DID_MAP).unwrap(), ROOT_DID_MAP).unwrap();
    let weenie = root.by_label("WEENIECONTENT").expect("WEENIECONTENT map");
    let weenie = DataIdMap::parse_expecting(&set.read(weenie.0).unwrap(), weenie).unwrap();
    let directory = weenie
        .by_label("RaceTableDirectory")
        .expect("RaceTableDirectory");
    let pids = RacePids::from_names(dict).expect("race-table property names");
    RaceTable::read(directory, dict, &pids, fetch(set)).expect("race table")
}

fn summary(r: &ClipResolution, layer: u32) -> String {
    match r.primary(layer) {
        Some(c) => format!(
            "{} ({} cand., rate {:.2}, disp {:.2})",
            c.clip,
            r.on_layer(layer).count(),
            c.playback_rate(),
            c.displacement[1]
        ),
        None => "-".into(),
    }
}

/// `script-table.md` §8.3: (avatar entity, idle, walk forward, run forward) for the rows the
/// spec lists in full (Beorning rows say "as Human", the same clips).
const SPEC_8_3: [(u32, u32, u32, u32); 14] = [
    (0x4700_0606, 0x0500_0DF8, 0x0500_0E03, 0x0500_0E61), // Hobbit M
    (0x4700_0607, 0x0500_0DFD, 0x0500_11E2, 0x0500_11C1), // Hobbit F
    (0x4700_060A, 0x0500_1473, 0x0500_8D08, 0x0500_8D27), // Human M
    (0x4700_060B, 0x0500_15B4, 0x0500_8998, 0x0500_899A), // Human F
    (0x4700_060E, 0x0500_17FE, 0x0500_93BC, 0x0500_93C0), // Elf M
    (0x4700_060F, 0x0500_1804, 0x0500_93A0, 0x0500_9397), // Elf F
    (0x4700_0611, 0x0500_1D3C, 0x0500_1D3B, 0x0500_1D18), // Dwarf M
    (0x4700_7004, 0x0500_1473, 0x0500_8D08, 0x0500_8D27), // Beorning M (as Human M)
    (0x4700_7005, 0x0500_15B4, 0x0500_8998, 0x0500_899A), // Beorning F (as Human F)
    (0x4700_806C, 0x0500_17FE, 0x0500_93BC, 0x0500_93DD), // HighElf M
    (0x4700_8071, 0x0500_1804, 0x0500_93A0, 0x0500_93DB), // HighElf F
    (0x4700_979A, 0x0500_7A13, 0x0500_7A0D, 0x0500_7A14), // StoutAxe Dwarf
    (0x4700_CEA3, 0x0500_8EE8, 0x0500_8EA9, 0x0500_8EA1), // RiverHobbit M
    (0x4700_CEA4, 0x0500_8EE7, 0x0500_8EB6, 0x0500_8EAD), // RiverHobbit F
];

/// `script-table.md` §8.1 item 2: every player-avatar object adds this table.
const AVATAR_ADDITIONAL_TABLE: u32 = 0x0700_0000;

#[test]
fn avatars_resolve_idle_walk_and_run() {
    let Some(set) = open_set() else { return };
    let dict = dictionary(set);
    let vocab = vocabulary(set);
    let clips = dids_with_prefix(set, 0x05);
    let race = race_table(set);
    eprintln!("race table: {} gender entries", race.entries.len());
    assert!(race.entries.len() >= 14, "playable combinations (§2.2)");
    let mut resolver =
        ClipResolver::new(dict, vocab.clone(), fetch(set)).with_dictionary_defaults(dict);
    let mut avatars = BTreeSet::new();
    let mut failures = Vec::new();
    let mut checked_rows = BTreeSet::new();
    for entry in &race.entries {
        if !avatars.insert(entry.player_avatar) {
            continue; // DwarfStoutAxe names one avatar for both genders.
        }
        let chain = collect_entity_scripts(entry.player_avatar, dict, &vocab, fetch(set))
            .expect("avatar scripts");
        let scripts = &chain.scripts;
        // §8.1 item 2, §11.3 step 1: the game object's additional table is in the closure
        // roots (its elements are bare DIDs, §16 OPEN).
        if !scripts.tables().contains(&Did(AVATAR_ADDITIONAL_TABLE)) {
            failures.push(format!(
                "{}: tables {:?} lack the object's additional table {AVATAR_ADDITIONAL_TABLE:#010x}",
                entry.player_avatar,
                scripts.tables()
            ));
        }
        let spec_row = chain
            .entities
            .first()
            .and_then(|e| SPEC_8_3.iter().find(|r| r.0 == e.0));
        let mut row = format!(
            "  species {:#x} sex {:#06x} avatar {} entity {:?} tables {:?}:",
            entry.species,
            entry.sex,
            entry.player_avatar,
            chain.entities.first(),
            scripts.tables()
        );
        for state in [MotionState::Idle, MotionState::Walk, MotionState::Run] {
            let r = resolver.resolve(scripts, state).expect("resolve");
            let missing: Vec<&ResolveIssue> = r
                .issues
                .iter()
                .chain(&r.closure.issues)
                .filter(|i| !matches!(i, ResolveIssue::StartupScriptNotFound(_)))
                .collect();
            let (layer, what) = match state {
                MotionState::Idle => (10, "idle"),
                _ => (
                    20,
                    if state == MotionState::Walk {
                        "walk"
                    } else {
                        "run"
                    },
                ),
            };
            row += &format!(
                " {what} {} [{} tables, {} nodes, {} issues]",
                summary(&r, layer),
                r.closure.order.len(),
                r.nodes_visited,
                missing.len()
            );
            if state == MotionState::Idle {
                // §6.9: the species table's body-shape MeshDeformer has its time driven by
                // AC_BoneScale_Test; the pair is found by the field name
                // MeshDeformerScriptNode_AnimationTime (§4.2).
                let bone_scale = dict.pid_of("AC_BoneScale_Test");
                let driven = r.deformers.iter().any(|d| {
                    matches!(d, DeformerCandidate::Mesh { time_source, .. }
                        if time_source.is_some() && *time_source == bone_scale)
                });
                if !driven {
                    failures.push(format!(
                        "{}: no body-shape deformer driven by AC_BoneScale_Test",
                        entry.player_avatar
                    ));
                }
                // The idle emote: layer 15 on hobbits and dwarves, 18 (face emote) on the
                // human-sized bodies.
                let emote = r
                    .candidates
                    .iter()
                    .find(|c| c.layer != 10 && c.scripts.is_empty())
                    .map_or(15, |c| c.layer);
                row += &format!(" emote(layer {emote}) {}", summary(&r, emote));
                if std::env::var_os("TESSERA_SCRIPT_DEBUG").is_some() {
                    for c in &r.candidates {
                        row += &format!(
                            "\n      cand {} layer {} chance {} table {} scriptlet {} via {:x?}",
                            c.clip, c.layer, c.chance, c.table, c.scriptlet, c.scripts
                        );
                    }
                }
            }
            // §8.3 / §16: the resolved clip is the table's, not merely an existing clip.
            if let Some(&(entity, idle, walk, run)) = spec_row {
                let want = match state {
                    MotionState::Idle => idle,
                    MotionState::Walk => walk,
                    _ => run,
                };
                checked_rows.insert(entity);
                let got = r.primary(layer).map(|c| c.clip.0);
                if got != Some(want) {
                    failures.push(format!(
                        "{} {what}: resolved {got:x?}, §8.3 gives {want:#010x}",
                        entry.player_avatar
                    ));
                }
            }
            match r.primary(layer) {
                Some(c) if clips.contains(&c.clip.0) => {}
                Some(c) => failures.push(format!(
                    "{} {what}: clip {} missing",
                    entry.player_avatar, c.clip
                )),
                None => failures.push(format!("{} {what}: no clip", entry.player_avatar)),
            }
        }
        eprintln!("{row}");
    }
    eprintln!("  {} records cached", resolver.cached_records());
    assert!(failures.is_empty(), "{failures:#?}");
    assert_eq!(
        checked_rows.len(),
        SPEC_8_3.len(),
        "every §8.3 avatar entity was reached from the race table"
    );
}

#[test]
fn weenie_creatures_resolve() {
    let Some(set) = open_set() else { return };
    if !exhaustive() {
        eprintln!("debug build: skipping the landblock scan (set TESSERA_FULL=1)");
        return;
    }
    let dict = dictionary(set);
    let vocab = vocabulary(set);
    // Every 0x70 object placed by a landblock SED (`entity-descriptor.md` §4.3).
    let mut work = Vec::new();
    for (i, m) in set.members().iter().enumerate() {
        if !matches!(m.id.kind, ArchiveKind::Cell(_)) {
            continue;
        }
        let lo = 0x8020_0000;
        for e in m.archive.entries_range(lo..=lo | 0x000F_FFFF) {
            work.push((i, e.expect("directory walk")));
        }
    }
    let objects: BTreeSet<u32> = par_each(&work, |(i, e)| {
        let Ok(payload) = set.member(*i).archive.read_entry(e) else {
            return Vec::new();
        };
        let Ok(info) = LandblockInfo::parse(&payload, Some(e.stamp), dict) else {
            return Vec::new();
        };
        info.entities
            .iter()
            .filter_map(|s| s.object)
            .filter(|d| d.prefix() == 0x70)
            .map(|d| d.0)
            .collect::<Vec<_>>()
    })
    .into_iter()
    .flatten()
    .collect();
    eprintln!("{} distinct weenie objects placed by SEDs", objects.len());
    assert!(!objects.is_empty());
    let mut resolver =
        ClipResolver::new(dict, vocab.clone(), fetch(set)).with_dictionary_defaults(dict);
    let mut entities: BTreeMap<Did, Did> = BTreeMap::new();
    let mut outcome: BTreeMap<&'static str, Vec<Did>> = BTreeMap::new();
    for &o in &objects {
        let chain = match collect_entity_scripts(Did(o), dict, &vocab, fetch(set)) {
            Ok(c) => c,
            Err(e) => {
                eprintln!("  {o:#010x}: {e}");
                outcome.entry("chain error").or_default().push(Did(o));
                continue;
            }
        };
        let Some(&entity) = chain.entities.first() else {
            outcome.entry("no PhysObj").or_default().push(Did(o));
            continue;
        };
        if entities.insert(entity, Did(o)).is_some() {
            continue;
        }
        let scripts = &chain.scripts;
        if scripts.class_table.is_none() {
            // §10.4's category; such an entity may still animate through additional tables
            // of its own or of its object (§8.1 item 2).
            let animates = !scripts.tables().is_empty()
                && !resolver
                    .reachable(scripts)
                    .expect("resolve")
                    .candidates
                    .is_empty();
            let class = if animates {
                "no class table, additional tables animate"
            } else {
                "no class table"
            };
            outcome.entry(class).or_default().push(entity);
            continue;
        }
        // Classification of §10.4: what is reachable from Startup whatever the switch
        // values; idle = layers below 20, movement = layers 20–29.
        let all = resolver.reachable(scripts).expect("resolve");
        let has_idle = all.candidates.iter().any(|c| c.layer < 20);
        let moves = all.candidates.iter().any(|c| (20..30).contains(&c.layer));
        let class = match (has_idle, moves) {
            (true, true) => "idle and movement",
            (true, false) => "idle only",
            (false, true) => "movement only",
            (false, false) if all.candidates.is_empty() => "no Startup animation",
            (false, false) => "other Startup animations",
        };
        if !all.candidates.is_empty() {
            let idle = resolver
                .resolve(scripts, MotionState::Idle)
                .expect("resolve");
            let walk = resolver
                .resolve(scripts, MotionState::Walk)
                .expect("resolve");
            let run = resolver
                .resolve(scripts, MotionState::Run)
                .expect("resolve");
            let layers: BTreeSet<u32> = all.candidates.iter().map(|c| c.layer).collect();
            eprintln!(
                "  entity {entity} (object {o:#010x}): {} reachable clips on layers {layers:?}; \
                 idle state {} walk {} run {}",
                all.clips().len(),
                summary(&idle, idle.candidates.first().map_or(10, |c| c.layer)),
                summary(&walk, 20),
                summary(&run, 20)
            );
        }
        outcome.entry(class).or_default().push(entity);
    }
    eprintln!("{} distinct physics entities", entities.len());
    for (k, v) in &outcome {
        eprintln!("  {k}: {} (e.g. {:?})", v.len(), v.first());
    }
    assert!(
        outcome.contains_key("idle and movement"),
        "at least the monster type resolves idle and movement (§10.4)"
    );
}

/// The face-compositing and colour-edit nodes (`appearance.md` §3.5, `script-table.md`
/// §5.1): every TextureCompositor (53), ModifyCompositorLayer (44), ModifyCompositorColor
/// (33) and EditMaterial (60) node decodes to its typed fields; the EditMaterial colour
/// edits driven by `AC_Color_Hair` and `AC_Color_Skin` are found through the
/// `EditMaterialScript_ColorValue` field PID (A21: 255 and 42).
#[test]
fn look_nodes_decode_and_follow_the_census() {
    use tessera_formats::script_table::NodeData;
    let Some(set) = open_set() else { return };
    let dict = dictionary(set);
    let vocab = vocabulary(set);
    let colour_field = vocab.edit_material.color_value;
    let hair = dict.pid("AC_Color_Hair");
    let skin = dict.pid("AC_Color_Skin");
    let ac_colour = |pid: u32| dict.name(pid).is_some_and(|n| n.starts_with("AC_Color_"));
    let ac_texture = |pid: u32| dict.name(pid).is_some_and(|n| n.starts_with("AC_Texture_"));
    // [53, 44, 33, 60, untyped, 60 colour driven by hair, by skin, 33 driven by AC_Color_*,
    //  44 driven by AC_Texture_*]
    let count = |roots: &[tessera_formats::script_table::Node], c: &mut [usize; 9]| {
        for r in roots {
            r.walk(&mut |n, _| {
                let driven = |pred: &dyn Fn(u32) -> bool| {
                    n.base.driven.iter().any(|&(_, source)| pred(source))
                };
                match (n.kind.0, &n.data) {
                    (53, NodeData::TextureCompositor(_)) => c[0] += 1,
                    (44, NodeData::CompositorLayer(_)) => {
                        c[1] += 1;
                        c[8] += usize::from(driven(&ac_texture));
                    }
                    (33, NodeData::CompositorColour(_)) => {
                        c[2] += 1;
                        c[7] += usize::from(driven(&ac_colour));
                    }
                    (60, NodeData::EditMaterial(_)) => {
                        c[3] += 1;
                        for &(field, source) in &n.base.driven {
                            if Some(field) == colour_field {
                                c[5] += usize::from(Some(source) == hair);
                                c[6] += usize::from(Some(source) == skin);
                            }
                        }
                    }
                    (33 | 44 | 53 | 60, _) => c[4] += 1,
                    _ => {}
                }
            });
        }
    };
    let tables = par_each(&entries_with_prefix(set, 0x07), |(i, e)| {
        let mut c = [0usize; 9];
        if let Ok(payload) = set.member(*i).archive.read_entry(e)
            && let Ok(t) = ScriptTable::parse_expecting(&payload, e.did, dict)
        {
            for sc in &t.scripts {
                count(&sc.roots, &mut c);
            }
        }
        c
    });
    let scriptlets = par_each(&entries_with_prefix(set, 0x0C), |(i, e)| {
        let mut c = [0usize; 9];
        if let Ok(payload) = set.member(*i).archive.read_entry(e)
            && let Ok(s) = Scriptlet::parse_expecting(&payload, e.did, dict)
        {
            count(&s.roots, &mut c);
        }
        c
    });
    let sum = |v: Vec<[usize; 9]>| {
        v.into_iter().fold([0usize; 9], |mut a, c| {
            for (x, y) in a.iter_mut().zip(c) {
                *x += y;
            }
            a
        })
    };
    let (t, s) = (sum(tables), sum(scriptlets));
    eprintln!(
        "look nodes in tables: 53 {}, 44 {}, 33 {}, 60 {}, untyped {}; colour edits driven by \
         hair {}, skin {}; 33 driven by AC_Color_* {}; 44 driven by AC_Texture_* {}",
        t[0], t[1], t[2], t[3], t[4], t[5], t[6], t[7], t[8]
    );
    eprintln!(
        "look nodes in scriptlets: 53 {}, 44 {}, 33 {}, 60 {}, untyped {}; colour edits driven \
         by hair {}, skin {}",
        s[0], s[1], s[2], s[3], s[4], s[5], s[6]
    );
    assert_eq!(
        t[4] + s[4],
        0,
        "every look node decodes to its typed fields"
    );
    if exhaustive() {
        assert!(colour_field.is_some(), "EditMaterialScript_ColorValue");
        assert!(t[5] + s[5] > 0, "colour edits driven by AC_Color_Hair");
        assert!(t[6] + s[6] > 0, "colour edits driven by AC_Color_Skin");
        // script-table.md §5.1 and appearance.md A21.
        let found = [t[0], t[1], t[2], t[3], s[3], t[5] + s[5], t[6] + s[6]];
        let census = [39, 199, 218, 2873, 13, 255, 42];
        if found != census {
            eprintln!("census differs from the spec: {found:?} against {census:?}");
            assert!(
                !std::env::var_os("TESSERA_CENSUS").is_some_and(|v| v == "strict"),
                "census (strict)"
            );
        }
    }
}
