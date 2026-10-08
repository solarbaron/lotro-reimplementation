// SPDX-License-Identifier: GPL-3.0-or-later
//! `sound.md` §13 invariants against a user's own install (read-only):
//!
//! - SI1–SI3 on every sound-info record (0x2A): exact decode, the payload exists and is
//!   audio (never DirectMusic), the unused floats are −1, priority in 0–1, a positive
//!   maximum distance, no unknown flag bits;
//! - SN1–SN2 and, for WAVs, the strict chunk walk of §12.2 step 5 on the payload records
//!   (0x0A) read: every one in a release build (or with
//!   `TESSERA_FULL=1`), otherwise the small ones (which include the DirectMusic records) and
//!   a sample;
//! - ND1–ND3 on every Sound and SoundEnvironment node of every script table (0x07) and
//!   scriptlet (0x0C): no ports or targets, channel 0–8, non-negative ramp-out and random
//!   distances, and every non-zero `SoundScript_SoundID` an existing 0x2A record. Every
//!   scriptlet must decode; a script table may fail only on CinematicCamera node data
//!   (`script-table.md` §10.3), and any other decode failure fails the test.
//!
//! Skipped unless `TESSERA_GAME_DIR` points at an install. Run with
//! `cargo test --release -p tessera-formats --test sound_real_data -- --nocapture`.
//! Only counts, DIDs and error kinds are printed; no game data leaves the install.

use std::collections::{BTreeMap, HashSet};
use std::path::PathBuf;
use std::sync::atomic::{AtomicUsize, Ordering};
use std::sync::{Mutex, OnceLock};

use tessera_dat::{ArchiveSet, Entry, SetOptions};
use tessera_formats::script_table::{Node, NodeKind};
use tessera_formats::{
    AudioContainer, Did, FormatError, NodeData, PropertyDictionary, Record, ScriptTable, Scriptlet,
    SoundInfo, SoundRecord,
};

const PROPERTY_DICTIONARY: u32 = 0x3400_0000;
/// Payload records up to this size are always read (the DirectMusic records are far
/// smaller), so SI1's "never DMAP" is checked in every build.
const SMALL_PAYLOAD: u32 = 64 * 1024;
/// Highest `SoundScript_Channel` value (ND2; enum labelled "SoundChannel", `sound.md` §7.1).
const MAX_CHANNEL: u32 = 8;

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

/// Every (member index, entry) with type prefix `prefix`, in all archives of the set.
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
    out
}

fn read(set: &ArchiveSet, (i, e): &(usize, Entry)) -> Vec<u8> {
    set.member(*i)
        .archive
        .read_entry(e)
        .unwrap_or_else(|err| panic!("read {:#010x}: {err}", e.did))
}

/// What the payload pass learned: every 0x0A DID, the DirectMusic ones among those read,
/// and the number read.
struct Payloads {
    all: HashSet<u32>,
    dmap: HashSet<u32>,
    read: usize,
}

fn payloads(set: &'static ArchiveSet) -> &'static Payloads {
    static P: OnceLock<Payloads> = OnceLock::new();
    P.get_or_init(|| {
        let entries = entries_with_prefix(set, 0x0A);
        let all: HashSet<u32> = entries.iter().map(|(_, e)| e.did).collect();
        let full = exhaustive();
        let picked: Vec<_> = entries
            .iter()
            .enumerate()
            .filter(|(k, (_, e))| full || e.length <= SMALL_PAYLOAD || k % 23 == 0)
            .map(|(_, e)| *e)
            .collect();
        let results = par_each(&picked, |item| {
            let did = item.1.did;
            let data = read(set, item);
            // SN1 (length, DID) is enforced by the parser and re-checked by `validate`.
            let s = SoundRecord::parse_expecting(&data, did)
                .unwrap_or_else(|err| panic!("payload {did:#010x}: {err}"));
            if let Err(issue) = s.validate(did, data.len()) {
                panic!("payload {did:#010x}: SN1/SN2: {issue}");
            }
            // §12.2 step 5 (strict `data` length and block-align multiple) on every WAV read.
            if s.container() == AudioContainer::Wave {
                tessera_formats::WaveInfo::parse(s.audio)
                    .unwrap_or_else(|err| panic!("payload {did:#010x}: WAV: {err}"));
            }
            (did, s.container(), s.storage_format().raw())
        });
        let mut by: BTreeMap<(String, u32), usize> = BTreeMap::new();
        let mut dmap = HashSet::new();
        for (did, c, f) in &results {
            *by.entry((format!("{c:?}"), *f)).or_default() += 1;
            if *c == AudioContainer::DirectMusicAudioPath {
                dmap.insert(*did);
            }
        }
        eprintln!(
            "payloads (0x0A): {} records, {} read ({}); by (container, storage format): {by:?}",
            all.len(),
            results.len(),
            if full {
                "all"
            } else {
                "small ones and a sample"
            }
        );
        Payloads {
            all,
            dmap,
            read: results.len(),
        }
    })
}

fn sound_info_dids(set: &ArchiveSet) -> &'static HashSet<u32> {
    static S: OnceLock<HashSet<u32>> = OnceLock::new();
    S.get_or_init(|| {
        entries_with_prefix(set, 0x2A)
            .iter()
            .map(|(_, e)| e.did)
            .collect()
    })
}

#[test]
fn sound_infos_hold_si1_to_si3() {
    let Some(set) = open_set() else { return };
    let p = payloads(set);
    assert!(p.read > 0, "no payload records");
    // Every DirectMusic record is small and so always read; how many exist is a census
    // (`sound.md` §13: assert invariants, not counts), so it is only reported.
    eprintln!("DirectMusic payloads read: {}", p.dmap.len());
    let entries = entries_with_prefix(set, 0x2A);
    assert!(!entries.is_empty(), "no sound-info records");
    let infos = par_each(&entries, |item| {
        let did = item.1.did;
        let data = read(set, item);
        // SI1: 32 fixed bytes, the name, nothing after: the parser requires the end.
        SoundInfo::parse_expecting(&data, did)
            .unwrap_or_else(|err| panic!("sound info {did:#010x}: {err}"))
    });
    let mut flags: BTreeMap<u32, usize> = BTreeMap::new();
    let mut names = HashSet::new();
    let mut min_over_max = 0usize;
    for s in &infos {
        let did = s.did;
        // SI1 (payload prefix), SI2, SI3.
        if let Err(issue) = s.validate() {
            panic!("{did}: SI1-SI3: {issue}");
        }
        // SI1: the payload exists and is audio.
        assert!(
            p.all.contains(&s.payload.0),
            "{did}: payload {} missing",
            s.payload
        );
        assert!(
            !p.dmap.contains(&s.payload.0),
            "{did}: payload {} is DirectMusic",
            s.payload
        );
        *flags.entry(s.flags.bits()).or_default() += 1;
        names.insert(s.name.as_str());
        min_over_max += usize::from(s.min_distance > s.max_distance);
    }
    let flags: Vec<String> = flags
        .iter()
        .map(|(f, n)| format!("{f:#04x}: {n}"))
        .collect();
    eprintln!(
        "sound infos (0x2A): {} decoded exactly; {} distinct names; {} with min > max \
         distance; by flags: {}",
        infos.len(),
        names.len(),
        min_over_max,
        flags.join(", ")
    );
}

/// Per-record findings of the node pass.
#[derive(Default)]
struct NodeTotals {
    records: usize,
    decoded: usize,
    sound: usize,
    environment: usize,
    null_ids: usize,
    placeholders: usize,
    driven_ids: usize,
    tracked: usize,
    channels: BTreeMap<u32, usize>,
    environments: BTreeMap<(u32, i32), usize>,
}

/// Checks ND1–ND3 on one node and counts it.
fn check_node(n: &Node, owner: Did, infos: &HashSet<u32>, t: &mut NodeTotals) {
    match &n.data {
        NodeData::Sound(s) => {
            assert_eq!(n.kind, NodeKind::SOUND);
            t.sound += 1;
            // ND1.
            assert!(
                n.ports.is_empty() && n.targets.is_empty(),
                "{owner}: sound node {} has ports or targets",
                n.base.node_id
            );
            // ND2.
            assert!(s.channel <= MAX_CHANNEL, "{owner}: channel {}", s.channel);
            assert!(s.ramp_out_time >= 0.0, "{owner}: {}", s.ramp_out_time);
            assert!(
                s.min_random_distance >= 0.0 && s.max_random_distance >= 0.0,
                "{owner}: random distances {} {}",
                s.min_random_distance,
                s.max_random_distance
            );
            // ND3.
            t.placeholders += usize::from(n.is_sound_placeholder());
            if s.stored_sound_is_null() {
                t.null_ids += 1;
            } else {
                assert!(
                    infos.contains(&s.sound.0),
                    "{owner}: node {} names missing sound info {}",
                    n.base.node_id,
                    s.sound
                );
            }
            t.driven_ids += usize::from(
                n.base
                    .driven_source(tessera_formats::script_table::sound_pid::SOUND_ID)
                    .is_some(),
            );
            t.tracked += usize::from(!s.fire_and_forget);
            *t.channels.entry(s.channel).or_default() += 1;
        }
        NodeData::SoundEnvironment { name, priority } => {
            assert_eq!(n.kind, NodeKind::SOUND_ENVIRONMENT);
            t.environment += 1;
            assert!(
                n.ports.is_empty() && n.targets.is_empty(),
                "{owner}: sound-environment node has ports or targets"
            );
            *t.environments.entry((*name, *priority)).or_default() += 1;
        }
        _ => assert!(
            n.kind != NodeKind::SOUND && n.kind != NodeKind::SOUND_ENVIRONMENT,
            "{owner}: kind {} not typed",
            n.kind.0
        ),
    }
}

/// Walks every record of `prefix`. A record that fails to decode must fail with an error
/// `allowed` accepts (anything else panics with its DID); the failures are returned.
fn node_pass<T>(
    set: &'static ArchiveSet,
    prefix: u8,
    decode: impl Fn(Did, &[u8]) -> Result<T, FormatError> + Sync,
    roots: impl Fn(&T) -> Vec<&Node> + Sync,
    allowed: impl Fn(&FormatError) -> bool + Sync,
) -> (NodeTotals, Vec<(Did, String)>) {
    let infos = sound_info_dids(set);
    let entries = entries_with_prefix(set, prefix);
    let parts = par_each(&entries, |item| {
        let did = Did(item.1.did);
        let data = read(set, item);
        let mut t = NodeTotals {
            records: 1,
            ..NodeTotals::default()
        };
        let failure = match decode(did, &data) {
            Ok(rec) => {
                t.decoded = 1;
                for root in roots(&rec) {
                    root.walk(&mut |n, _| check_node(n, did, infos, &mut t));
                }
                None
            }
            Err(err) => {
                assert!(
                    allowed(err.root_cause()),
                    "{did}: unexpected decode failure: {err}"
                );
                Some((did, err.root_cause().to_string()))
            }
        };
        (t, failure)
    });
    let mut t = NodeTotals::default();
    let mut failures = Vec::new();
    for (p, failure) in parts {
        failures.extend(failure);
        t.records += p.records;
        t.decoded += p.decoded;
        t.sound += p.sound;
        t.environment += p.environment;
        t.null_ids += p.null_ids;
        t.placeholders += p.placeholders;
        t.driven_ids += p.driven_ids;
        t.tracked += p.tracked;
        for (k, v) in p.channels {
            *t.channels.entry(k).or_default() += v;
        }
        for (k, v) in p.environments {
            *t.environments.entry(k).or_default() += v;
        }
    }
    failures.sort();
    (t, failures)
}

/// The one decode failure script tables may have: CinematicCamera (kind 7) node data,
/// whose layout is OPEN (`script-table.md` §10.3). It is not a sound node, so no Sound or
/// SoundEnvironment node can hide behind it.
fn is_cinematic_camera(err: &FormatError) -> bool {
    matches!(err, FormatError::Unsupported { value: 7, what, .. } if what.contains("kind 7"))
}

fn report(what: &str, t: &NodeTotals) {
    eprintln!(
        "{what}: {} records, {} decoded; {} Sound nodes ({} tracked, {} with stored SoundID 0 \
         of which {} placeholders, {} with a driven SoundID), {} SoundEnvironment nodes; \
         channels {:?}; environments {:?}",
        t.records,
        t.decoded,
        t.sound,
        t.tracked,
        t.null_ids,
        t.placeholders,
        t.driven_ids,
        t.environment,
        t.channels,
        t.environments
    );
}

#[test]
fn sound_nodes_hold_nd1_to_nd3() {
    let Some(set) = open_set() else { return };
    let dict = dictionary(set);
    // Script tables may fail only in the CinematicCamera category (`script-table.md`
    // §10.3); any other failure (for example a Sound node the typed decoder rejects) fails
    // this test with its DID.
    let (tables, table_failures) = node_pass(
        set,
        0x07,
        |did, p| ScriptTable::parse_expecting(p, did, dict),
        |t: &ScriptTable| t.scripts.iter().flat_map(|s| s.roots.iter()).collect(),
        is_cinematic_camera,
    );
    report("script tables (0x07)", &tables);
    eprintln!("script tables not decoded (CinematicCamera): {table_failures:?}");
    let (scriptlets, _) = node_pass(
        set,
        0x0C,
        |did, p| Scriptlet::parse_expecting(p, did, dict),
        |s: &Scriptlet| s.roots.iter().collect(),
        |_| false,
    );
    report("scriptlets (0x0C)", &scriptlets);
    assert!(tables.records > 0 && scriptlets.records > 0);
    assert!(tables.decoded > 0, "no script table decodes");
    assert_eq!(
        tables.records - tables.decoded,
        table_failures.len(),
        "every undecoded script table is accounted for"
    );
    assert_eq!(
        scriptlets.decoded, scriptlets.records,
        "every scriptlet decodes"
    );
    assert!(tables.sound > 0 && scriptlets.sound > 0);
}
