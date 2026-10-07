// SPDX-License-Identifier: GPL-3.0-or-later
//! The runtime on a user's own install (read-only): `sound.md` §10–§11.
//!
//! - The local player's avatar runs `SND_music_system_startup` from its table closure
//!   (`sound.md` §10.1). `Area_Music` is set in turn to a sample of the case values of the
//!   graph's `Area_Music` Switches and to a value with no case; simulated time advances and
//!   track voices are completed now and then. Tracked channel-2 sounds must start, area
//!   changes must stop the running track (its ramp-out is the sink's), and the instance
//!   count stays bounded.
//! - A sample of entity descriptors runs `Startup`; positional emitters (§11) must request
//!   sounds.
//!
//! Skipped unless `TESSERA_GAME_DIR` points at an install. A release build takes a larger
//! sample: `cargo test --release -p tessera-scriptgraph --test real_data -- --nocapture`.
//! Only counts are printed; no game data leaves the install.

use std::collections::BTreeSet;
use std::path::PathBuf;
use std::sync::OnceLock;

use tessera_dat::{ArchiveSet, SetOptions};
use tessera_formats::avatar::{RacePids, RaceTable};
use tessera_formats::script_table::resolve::{ScriptVocabulary, collect_entity_scripts};
use tessera_formats::script_table::{Node, NodeData, PortPayload, PortType};
use tessera_formats::{
    DataIdMap, Did, EnumMapper, PropertyDictionary, PropertyValue, Record, ScriptTable,
};
use tessera_scriptgraph::{
    EntityId, EntityInit, FetchSource, RecordingSink, Runtime, RuntimeVocabulary, SinkEvent,
    SoundHandle, UnsetPolicy, vocab,
};

const PROPERTY_DICTIONARY: u32 = 0x3400_0000;
const ROOT_DID_MAP: u32 = 0x2800_0000;
/// The `SoundChannel` value of music (`sound.md` §7.1; the channel's label is "Music").
const MUSIC_CHANNEL_LABEL: &str = "Music";

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

fn fetch(set: &'static ArchiveSet) -> impl FnMut(Did) -> Option<Vec<u8>> {
    move |d: Did| match set.locate(d.0) {
        Ok(Some(l)) => set.read_located(&l).ok(),
        _ => None,
    }
}

fn fetch_result(
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

fn release() -> bool {
    !cfg!(debug_assertions)
}

fn runtime(set: &'static ArchiveSet, policy: UnsetPolicy) -> Runtime<'static> {
    let dict = dictionary(set);
    let v = RuntimeVocabulary::from_data(dict, |d| mapper(set, d));
    assert!(v.startup_key.is_some(), "the Startup label");
    Runtime::new(FetchSource::new(dict, fetch(set)), dict, v)
        .with_seed(1)
        .with_unset_policy(policy)
}

/// The race table's player avatars (`skeleton-anim.md` §2.2), as game objects.
fn avatars(set: &'static ArchiveSet) -> Vec<Did> {
    let dict = dictionary(set);
    let root = DataIdMap::parse_expecting(&set.read(ROOT_DID_MAP).unwrap(), ROOT_DID_MAP).unwrap();
    let weenie = root.by_label("WEENIECONTENT").expect("WEENIECONTENT map");
    let weenie = DataIdMap::parse_expecting(&set.read(weenie.0).unwrap(), weenie).unwrap();
    let directory = weenie
        .by_label("RaceTableDirectory")
        .expect("RaceTableDirectory");
    let pids = RacePids::from_names(dict).expect("race-table property names");
    let race = RaceTable::read(directory, dict, &pids, fetch_result(set)).expect("race table");
    let mut seen = BTreeSet::new();
    race.entries
        .iter()
        .map(|e| e.player_avatar)
        .filter(|d| seen.insert(*d))
        .collect()
}

/// Every case value of the Switches on `pid` in the definitions of `key` found in `tables`
/// and their includes.
fn case_values(set: &'static ArchiveSet, tables: &[Did], key: u32, pid: u32) -> BTreeSet<u32> {
    let dict = dictionary(set);
    let mut out = BTreeSet::new();
    let mut seen = BTreeSet::new();
    let mut stack: Vec<Did> = tables.to_vec();
    while let Some(d) = stack.pop() {
        if !seen.insert(d) || seen.len() > 4096 {
            continue;
        }
        let Some(bytes) = fetch(set)(d) else { continue };
        let Ok(t) = ScriptTable::parse_expecting(&bytes, d, dict) else {
            continue;
        };
        stack.extend(t.includes.iter().copied());
        if let Some(s) = t.script(key) {
            for r in &s.roots {
                r.walk(&mut |n: &Node, _| {
                    if let NodeData::Switch(sw) = &n.data
                        && sw.property == pid
                    {
                        for p in n.ports_of(PortType::SWITCH_VALUE) {
                            if let PortPayload::SwitchCase(c) = &p.payload {
                                for v in c.values.iter().flatten() {
                                    if let Some(b) = v.value.as_bits() {
                                        out.insert(b as u32);
                                    }
                                }
                            }
                        }
                    }
                });
            }
        }
    }
    out
}

fn music_handles(sink: &RecordingSink, channel: u32) -> Vec<SoundHandle> {
    sink.playing
        .iter()
        .filter(|(_, s)| s.channel == channel)
        .map(|(h, _)| *h)
        .collect()
}

#[test]
fn player_avatar_plays_area_music() {
    let Some(set) = open_set() else { return };
    let dict = dictionary(set);
    let svocab = ScriptVocabulary::from_data(dict, |d| mapper(set, d)).expect("vocabulary");
    let mut mappers = |d| mapper(set, d);
    let script_id = dict
        .pid("ScriptTable_ScriptID")
        .expect("ScriptTable_ScriptID");
    let music_key = vocab::enum_key(dict, script_id, "SND_music_system_startup", &mut mappers)
        .expect("the SND_music_system_startup label");
    let area_music = dict.pid("Area_Music").expect("Area_Music");
    let is_player = dict.pid("Entity_IsThePlayer").expect("Entity_IsThePlayer");
    let channel_pid = dict
        .pid("SoundScript_Channel")
        .expect("SoundScript_Channel");
    let music_channel = vocab::enum_key(dict, channel_pid, MUSIC_CHANNEL_LABEL, &mut mappers)
        .expect("the Music channel label");

    let avatar = avatars(set)[0];
    let chain = collect_entity_scripts(avatar, dict, &svocab, fetch_result(set)).expect("avatar");
    assert!(
        chain.scripts.startup_scripts.contains(&music_key),
        "the avatar starts the music system (sound.md §10.1)"
    );
    let mut init = EntityInit::from_scripts(&chain.scripts);
    init.properties.set(is_player, PropertyValue::Bool(true));
    let tables = init.tables.clone();

    let all_values = case_values(set, &tables, music_key, area_music);
    assert!(
        all_values.len() > 100,
        "Area_Music cases: {}",
        all_values.len()
    );
    let want = if release() { 40 } else { 12 };
    let step = (all_values.len() / want).max(1);
    let mut sample: Vec<u32> = all_values
        .iter()
        .copied()
        .step_by(step)
        .take(want)
        .collect();
    let no_case = (1..)
        .map(|k| all_values.last().unwrap() + k)
        .find(|v| !all_values.contains(v))
        .unwrap();
    sample.push(no_case);

    let mut rt = runtime(set, UnsetPolicy::DefaultPort);
    let mut sink = RecordingSink::new();
    sink.max_events = 1_000_000;
    let player = EntityId(1);
    rt.spawn_entity(&mut sink, player, init).expect("spawn");
    rt.set_player(Some(player));
    rt.set_viewport_entity(Some(player));
    assert!(rt.is_running(player, music_key), "the music script runs");

    let mut max_instances = 0usize;
    let mut track_starts = 0u64;
    let mut ramp_stops = 0u64;
    let mut values_with_music = 0usize;
    for &v in &sample {
        let playing_before: BTreeSet<SoundHandle> =
            music_handles(&sink, music_channel).into_iter().collect();
        let mark = sink.events.len();
        rt.set_property(&mut sink, player, area_music, PropertyValue::Enum(v))
            .expect("set Area_Music");
        // The tracks playing before the change are stopped (with their ramp-out) or kept.
        ramp_stops += sink.events[mark..]
            .iter()
            .filter(|e| matches!(e, SinkEvent::SoundStopped(h) if playing_before.contains(h)))
            .count() as u64;
        let mut started_here = 0u64;
        for round in 0..3 {
            for _ in 0..(if round == 0 { 50 } else { 400 }) {
                rt.advance(&mut sink, 0.1);
            }
            // Tracks end: complete the playing music voices (§9.2 step 12).
            for h in music_handles(&sink, music_channel) {
                sink.complete(h);
                rt.sound_completed(&mut sink, h);
            }
            max_instances = max_instances.max(rt.entity_instances(player).unwrap());
        }
        for e in &sink.events[mark..] {
            if let SinkEvent::SoundStarted(Some(_), s) = e
                && s.tracked
                && s.channel == music_channel
            {
                started_here += 1;
            }
        }
        track_starts += started_here;
        if started_here > 0 {
            values_with_music += 1;
        }
        assert!(
            rt.is_running(player, music_key),
            "the music script never ends"
        );
    }
    let stats = rt.stats();
    eprintln!(
        "music: {} values sampled (+1 without a case), {values_with_music} with tracked channel-{music_channel} starts; \
         {track_starts} track starts, {ramp_stops} stops of a playing track on a change; \
         max {max_instances} instances on the avatar; {} node starts, {} events, {} sounds requested, \
         {} environments, {} compiled units / {} nodes; issues {:?}",
        sample.len() - 1,
        stats.node_starts,
        stats.events_delivered,
        stats.sounds_requested,
        stats.environments_registered,
        stats.compiled_units,
        stats.compiled_nodes,
        rt.issues()
            .iter()
            .map(|i| format!("{i:?}").chars().take(80).collect::<String>())
            .collect::<Vec<_>>()
    );
    assert!(track_starts > 0, "tracked music starts");
    assert!(
        values_with_music * 2 >= sample.len() - 1,
        "most sampled areas play music"
    );
    assert!(ramp_stops > 0, "area changes stop the running track");
    assert!(max_instances < 20_000, "bounded: {max_instances}");
    rt.despawn(&mut sink, player).unwrap();
    assert_eq!(rt.stats().instances, 0);
    assert!(sink.playing.is_empty());

    // The same with the default policy (`script-runtime.md` §6.2 OPEN-5 Decision): the
    // Switches on properties the walker leaves unset (no dictionary default) select nothing.
    let chain = collect_entity_scripts(avatar, dict, &svocab, fetch_result(set)).expect("avatar");
    let mut init = EntityInit::from_scripts(&chain.scripts);
    init.properties.set(is_player, PropertyValue::Bool(true));
    init.properties
        .set(area_music, PropertyValue::Enum(sample[0]));
    let mut rt = runtime(set, UnsetPolicy::NoPort);
    let mut sink = RecordingSink::new();
    rt.spawn_entity(&mut sink, player, init).expect("spawn");
    for _ in 0..100 {
        rt.advance(&mut sink, 0.1);
    }
    let starts = sink
        .sound_starts()
        .filter(|(_, s)| s.tracked && s.channel == music_channel)
        .count();
    eprintln!("music with UnsetPolicy::NoPort: {starts} tracked channel-{music_channel} starts");
}

#[test]
fn emitters_request_sounds() {
    let Some(set) = open_set() else { return };
    let dict = dictionary(set);
    let svocab = ScriptVocabulary::from_data(dict, |d| mapper(set, d)).expect("vocabulary");
    let lo = 0x47u32 << 24;
    let mut dids: Vec<u32> = Vec::new();
    for m in set.members() {
        for e in m.archive.entries_range(lo..=lo | 0x00FF_FFFF) {
            dids.push(e.expect("directory walk").did);
        }
    }
    dids.sort_unstable();
    dids.dedup();
    let (scan, want) = if release() {
        (dids.len(), 400)
    } else {
        (4000, 25)
    };
    let stride = (dids.len() / scan.max(1)).max(1);
    let mut rt = runtime(set, UnsetPolicy::NoPort);
    let mut sink = RecordingSink::new();
    sink.max_events = 10_000;
    let player = EntityId(u64::MAX);
    rt.spawn_entity(&mut sink, player, EntityInit::default())
        .unwrap();
    rt.set_player(Some(player));
    let (mut scanned, mut with_tables, mut emitters, mut positional, mut tracked) = (0, 0, 0, 0, 0);
    let mut max_instances = 0usize;
    for (n, did) in dids.iter().step_by(stride).enumerate() {
        if emitters >= want {
            break;
        }
        scanned += 1;
        let Ok(chain) = collect_entity_scripts(Did(*did), dict, &svocab, fetch_result(set)) else {
            continue;
        };
        if chain.scripts.tables().is_empty() {
            continue;
        }
        with_tables += 1;
        let id = EntityId(n as u64 + 1);
        let before = sink.sounds_started;
        let mark = sink.events.len();
        rt.spawn_entity(&mut sink, id, EntityInit::from_scripts(&chain.scripts))
            .unwrap();
        for _ in 0..30 {
            rt.advance(&mut sink, 0.1);
        }
        max_instances = max_instances.max(rt.entity_instances(id).unwrap_or(0));
        if sink.sounds_started > before {
            emitters += 1;
            for e in sink.events.get(mark..).unwrap_or_default() {
                if let SinkEvent::SoundStarted(Some(_), s) = e
                    && s.entity == id
                {
                    positional += 1;
                    tracked += u64::from(s.tracked);
                }
            }
        }
        rt.despawn(&mut sink, id).unwrap();
        sink.events.clear();
    }
    let stats = rt.stats();
    eprintln!(
        "emitters: {scanned} descriptors scanned, {with_tables} with tables, {emitters} requested sounds \
         ({positional} requests from the entity, {tracked} tracked); max {max_instances} instances on one; \
         {} node starts, {} sounds requested, {} limit hits",
        stats.node_starts, stats.sounds_requested, stats.limit_hits
    );
    assert!(
        emitters > 0,
        "some descriptors are sound emitters (sound.md §11)"
    );
    assert!(max_instances < 16_384);
    assert!(sink.playing.is_empty(), "despawn stops every tracked voice");
}
