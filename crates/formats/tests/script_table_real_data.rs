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
    ClipResolution, ClipResolver, MotionState, ResolveIssue, ScriptVocabulary,
    collect_entity_scripts,
};
use tessera_formats::script_table::{NodeCensus, NodeKind};
use tessera_formats::{
    DataIdMap, Did, EnumMapper, FormatError, LandblockInfo, PropertyDictionary, Record,
    ScriptTable, Scriptlet,
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
