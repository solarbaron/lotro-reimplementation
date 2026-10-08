// SPDX-License-Identifier: GPL-3.0-or-later
//! The instrument table built from the user's own install (`docs/specs/instruments.md`
//! §4.1, §5.7, §13; ROADMAP M6), read-only.
//!
//! 1. Finds the music scripts through the script-table closure of every player avatar of
//!    the race table and checks IN1 (one defining table, the same for every avatar).
//! 2. Builds the instrument table and checks IN2–IN5, IN7, IN8, IN10, IN13, IN14, that the
//!    table agrees with a live walk for every combination (§5.7), and reports how the
//!    per-instrument values compare with §6.1.
//! 3. Renders a short original tune (a chromatic run over the whole range and a chord)
//!    through every instrument with the offline engine, decoding the samples it needs
//!    (IN6), and requires that no sample is missing and no note is refused or dropped.
//!
//! Skipped (reported as passed) unless `TESSERA_GAME_DIR` names an install. Prints counts
//! only. A debug build plays every ninth pitch of the run unless `TESSERA_AUDIO_FULL=1`;
//! run it in release for the whole tune:
//!
//! ```text
//! export TESSERA_GAME_DIR="$(cargo run -q -p tessera-dattool -- detect --path)"
//! cargo test -p tessera-audio --release --test instruments_real_data -- --nocapture
//! ```

use std::collections::{BTreeMap, BTreeSet, HashSet};
use std::path::PathBuf;
use std::sync::{Arc, OnceLock};
use std::time::Instant;

use tessera_audio::abc::{self, PITCH_SLOTS, PartRequest};
use tessera_audio::decode::{Decoded, decode_record};
use tessera_audio::engine::{AudioEngine, EngineConfig, LivePosition};
use tessera_audio::instruments::{
    CharacterId, InstrumentTable, MusicPlayer, MusicScript, MusicScripts, MusicVocabulary,
    PlayerConfig, SampleBank, TableLimits,
};
use tessera_dat::{ArchiveSet, SetOptions};
use tessera_formats::avatar::{RacePids, RaceTable};
use tessera_formats::script_table::resolve::ScriptVocabulary;
use tessera_formats::script_table::{Node, NodeData, NodeKind, PortType, SWITCH_EQUAL};
use tessera_formats::{DataIdMap, Did, EnumMapper, PropertyDictionary, Record};

/// The property dictionary and the root DID map: the two fixed entry points of the
/// format (`property-system.md` §2.1, `records.md` §8.2).
const PROPERTY_DICTIONARY: u32 = 0x3400_0000;
const ROOT_DID_MAP: u32 = 0x2800_0000;

fn open_set() -> Option<&'static ArchiveSet> {
    static SET: OnceLock<Option<ArchiveSet>> = OnceLock::new();
    SET.get_or_init(|| {
        let dir = match std::env::var_os(tessera_install::GAME_DIR_ENV) {
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

fn full() -> bool {
    !cfg!(debug_assertions) || std::env::var_os("TESSERA_AUDIO_FULL").is_some_and(|v| v == "1")
}

fn every_node<'a>(roots: &'a [Node], f: &mut impl FnMut(&'a Node)) {
    for r in roots {
        r.walk(&mut |n, _| f(n));
    }
}

/// §6.1 per instrument: (label, fire-and-forget, ramp, intensity). Expectations from the
/// spec, compared and reported (patches may change them).
const SPEC_6_1: [(&str, bool, f32, f32); 23] = [
    ("Clarinet", false, 0.2, 0.70),
    ("Horn", false, 0.2, 0.70),
    ("Flute", false, 0.2, 0.65),
    ("Lute", true, 0.0, 0.34),
    ("Harp", true, 0.0, 1.0),
    ("Drum", true, 0.0, 1.0),
    ("Theorbo", true, 0.0, 0.455),
    ("MoreCowbell", true, 0.0, 1.0),
    ("Bagpipe", false, 0.2, 0.65),
    ("Cowbell", true, 0.0, 0.70),
    ("Pibgorn", false, 0.2, 0.70),
    ("Harp_Special", true, 0.0, 0.45),
    ("Bassoon", false, 0.2, 0.95),
    ("Lute_of_Ages", true, 0.0, 0.77),
    ("Fiddle_short", true, 0.2, 0.50),
    ("Fiddle_pizz", true, 0.2, 0.65),
    ("Fiddle_vib", false, 0.2, 1.0),
    ("Fiddle_long", false, 0.2, 1.0),
    ("Fiddle_strong", false, 0.2, 1.0),
    ("Fiddle_basic", false, 0.2, 1.0),
    ("Bassoon_vib", false, 0.2, 0.95),
    ("Bassoon_short", false, 0.2, 0.95),
    ("Handknells", true, 0.2, 1.0),
];

/// The three branches of §5.5 as (from song, quantized).
const BRANCHES: [(&str, bool, bool); 3] = [
    ("song", true, false),
    ("quantized", false, true),
    ("live", false, false),
];

/// An original test tune: a chromatic run from `C,` to `c'` in sixteenths, then a chord.
fn tune(step: usize) -> Vec<u8> {
    const NAMES: [&str; 12] = [
        "C", "^C", "D", "^D", "E", "F", "^F", "G", "^G", "A", "^A", "B",
    ];
    let mut body = String::from("X:1\nT:Range check\nM:4/4\nL:1/16\nQ:1/4=240\nK:C\n");
    for p in (48u8..=84).step_by(step) {
        let i = usize::from(p - 48);
        let (name, oct) = (NAMES[i % 12], i / 12);
        let note = match oct {
            0 => format!("{name},"),
            1 => name.to_string(),
            2 => name.to_lowercase(),
            _ => format!("{}'", name.to_lowercase()),
        };
        body.push_str(&note);
        body.push(' ');
    }
    body.push_str("[C4E4G4]|\n");
    body.into_bytes()
}

#[test]
fn instrument_table_from_the_install() {
    let Some(set) = open_set() else { return };
    let t0 = Instant::now();
    let payload = set.read(PROPERTY_DICTIONARY).expect("read the dictionary");
    let dict = PropertyDictionary::parse_expecting(&payload, PROPERTY_DICTIONARY).expect("dict");
    let vocab = MusicVocabulary::from_data(&dict, |d| mapper(set, d)).expect("music vocabulary");
    // §5.2: every note parameter has the expected type in this install.
    assert!(vocab.mistyped.is_empty(), "§5.2: {:?}", vocab.mistyped);
    let script_vocab = ScriptVocabulary::from_data(&dict, |d| mapper(set, d)).expect("vocabulary");

    // IN10 and §7.1: 37 distinct non-zero keys by the label rule, equal to the fixed table.
    let keys: BTreeSet<u32> = vocab.pitch_keys.iter().copied().collect();
    assert_eq!(keys.len(), 37, "IN10");
    assert!(!keys.contains(&0), "IN10");
    let fixed: Vec<u32> = PITCH_SLOTS.iter().map(|&s| u32::from(s)).collect();
    assert_eq!(
        vocab.pitch_keys.to_vec(),
        fixed,
        "label rule = fixed table (§7.1)"
    );

    // IN1: every avatar closure has exactly one defining table, the same one.
    let root = DataIdMap::parse_expecting(&set.read(ROOT_DID_MAP).unwrap(), ROOT_DID_MAP).unwrap();
    let weenie = root.by_label("WEENIECONTENT").expect("WEENIECONTENT map");
    let weenie = DataIdMap::parse_expecting(&set.read(weenie.0).unwrap(), weenie).unwrap();
    let directory = weenie
        .by_label("RaceTableDirectory")
        .expect("RaceTableDirectory");
    let pids = RacePids::from_names(&dict).expect("race-table names");
    let race = RaceTable::read(directory, &dict, &pids, fetch(set)).expect("race table");
    let avatars = race.avatars();
    assert!(!avatars.is_empty());
    let mut defining = BTreeSet::new();
    let mut scripts = None;
    for &a in &avatars {
        let s = MusicScripts::locate(a, &dict, &script_vocab, &vocab, fetch(set))
            .expect("music scripts");
        assert_eq!(s.defining_tables.len(), 1, "IN1 for avatar {a}");
        defining.extend(s.defining_tables.iter().copied());
        scripts.get_or_insert(s);
    }
    assert_eq!(defining.len(), 1, "IN1: the same table for every avatar");
    let scripts = scripts.unwrap();
    eprintln!(
        "avatars: {}; music table found in every closure; scripts: {} and {} roots",
        avatars.len(),
        scripts.quantized.roots.len(),
        scripts.normal.roots.len()
    );

    // IN8.
    assert_eq!(scripts.quantized.max_instances, 10, "IN8");
    assert_eq!(scripts.normal.max_instances, 10, "IN8");

    // IN2 and IN3 on the stored trees.
    let mut kinds: BTreeMap<u8, usize> = BTreeMap::new();
    let mut sound_nodes = 0;
    let mut variances: BTreeMap<String, usize> = BTreeMap::new();
    for def in [&scripts.quantized, &scripts.normal] {
        every_node(&def.roots, &mut |n| {
            *kinds.entry(n.kind.0).or_default() += 1;
            if let NodeData::Switch(sw) = &n.data {
                for p in n.ports_of(PortType::SWITCH_VALUE) {
                    if let tessera_formats::script_table::PortPayload::SwitchCase(c) = &p.payload {
                        assert_eq!(c.values.len(), 1, "IN2: one value per case");
                        assert_eq!(c.operator, SWITCH_EQUAL, "IN2: Equal");
                    }
                }
                if sw.property == vocab.music_note {
                    assert_eq!(
                        n.ports_of(PortType::DEFAULT).count(),
                        0,
                        "IN2: no note Default"
                    );
                }
            }
            if let Some(s) = n.sound() {
                sound_nodes += 1;
                assert!(s.move_with_entity && !s.use_camera_position, "IN3");
                assert_eq!(
                    (s.min_random_distance, s.max_random_distance),
                    (0.0, 0.0),
                    "IN3"
                );
                assert_eq!(s.offset, [0.0; 3], "IN3");
                assert_eq!(s.pitch, 0.0, "IN3");
                let label = vocab.channel_label(s.channel).unwrap_or("?");
                assert!(
                    label == "PlayerMusic" || label == "QuantizedPlayerMusic",
                    "IN3: channel {label}"
                );
                if s.pitch_variance != 0.0 {
                    *variances
                        .entry(format!("{:.2}", s.pitch_variance))
                        .or_default() += 1;
                }
            }
        });
    }
    let allowed: HashSet<u8> = [
        NodeKind::COLLECTION,
        NodeKind::SWITCH,
        NodeKind::SOUND,
        NodeKind::PARTICLE,
    ]
    .iter()
    .map(|k| k.0)
    .collect();
    assert!(
        kinds.keys().all(|k| allowed.contains(k)),
        "IN2: kinds {kinds:?}"
    );
    assert!(
        variances.keys().all(|v| v == "0.02" || v == "0.20"),
        "IN3: {variances:?}"
    );
    eprintln!(
        "nodes by kind: {kinds:?}; Sound nodes {sound_nodes}; non-zero pitch variances {variances:?}"
    );

    // The table.
    let t1 = Instant::now();
    let table =
        InstrumentTable::build(scripts, &vocab, TableLimits::default(), fetch(set)).expect("table");
    eprintln!(
        "instrument table: {} sound nodes, {} sound infos, {} combinations, {} instruments, built in {:.2?}",
        table.sounds().len(),
        table.sound_infos().count(),
        table.combinations(),
        table.instruments().len(),
        t1.elapsed()
    );
    assert!(table.issues().is_empty(), "IN2/IN4: {:?}", table.issues());
    // A clone answers like the original (Sound nodes are keyed structurally).
    let copy = table.clone();
    for script in MusicScript::ALL {
        for &i in table.instruments() {
            for k in table.listed_notes() {
                assert_eq!(
                    copy.walk(script, true, false, i, k).unwrap(),
                    table.lookup_ids(script, true, false, i, k).to_vec()
                );
            }
        }
    }

    // IN4, IN14, IN7, IN13.
    for s in table.sounds() {
        for f in &s.path {
            assert!(
                f.use_parent && f.variance == 0.0 && f.probability == 1.0,
                "IN4"
            );
            assert!((0.0..=1.0).contains(&f.multiplier), "IN14");
        }
        let driven: Vec<bool> = s.path.iter().map(|f| f.volume_driven).collect();
        assert!(
            driven.iter().skip(1).all(|d| !d),
            "IN4: only the root is driven"
        );
        let info = table
            .sound_info(s.sound_info)
            .expect("every reached sound info decodes");
        assert!(info.flags.bits() & 0x04 != 0, "IN13");
        assert_eq!(info.flags.bits() & !0x05, 0, "IN13");
        if info.is_loop() {
            assert!(!s.fire_and_forget, "IN7");
        }
    }
    for script in MusicScript::ALL {
        let ids: HashSet<u32> = table
            .instruments()
            .iter()
            .flat_map(|&i| (0..=255u8).flat_map(move |k| [(i, k)]))
            .flat_map(|(i, k)| table.lookup_ids(script, true, false, i, k).to_vec())
            .collect();
        let driven = ids
            .iter()
            .filter(|&&i| table.sounds()[i as usize].volume_driven())
            .count();
        match script {
            MusicScript::Normal => assert_eq!(driven, ids.len(), "IN4: the normal root is driven"),
            MusicScript::Quantized => {
                assert_eq!(driven, 0, "IN4: nothing driven in the quantized script")
            }
        }
    }

    // §5.7: the table and the walk agree for every combination.
    let mut agree = 0usize;
    let extra = [13u32, 14, 15, 27, 99, table.other_instrument()];
    for script in MusicScript::ALL {
        for song in [false, true] {
            for q in [false, true] {
                for &i in table.instruments().iter().chain(&extra) {
                    for k in 0..=255u8 {
                        let w = table.walk(script, song, q, i, k).unwrap();
                        assert_eq!(table.lookup_ids(script, song, q, i, k), &w[..], "§5.7");
                        agree += 1;
                    }
                }
            }
        }
    }
    eprintln!("table = walk for {agree} combinations");

    // IN5 and §6.1.
    let label_of = |k: u32| vocab.instrument_label(k).unwrap_or("?").to_string();
    let c4 = vocab.pitch_key(72) as u8;
    let mut mismatches_6_1 = 0;
    for script in MusicScript::ALL {
        for (branch, song, q) in BRANCHES {
            for &i in table.instruments() {
                let label = label_of(i);
                if label == "Undef" {
                    continue;
                }
                let infos: Vec<Option<Did>> = (1..=37u8)
                    .map(|k| {
                        table
                            .lookup(script, song, q, i, k)
                            .next()
                            .map(|s| s.sound_info)
                    })
                    .collect();
                let sounding = infos.iter().flatten().count();
                let distinct: BTreeSet<Did> = infos.iter().flatten().copied().collect();
                let expected = match (label.as_str(), script, branch) {
                    ("Fiddle_long", _, _) => 33,
                    ("Horn", MusicScript::Quantized, "song") => 36,
                    _ => 37,
                };
                assert_eq!(sounding, expected, "IN5: {label} {script:?} {branch}");
                if label.ends_with("Cowbell") {
                    assert_eq!(distinct.len(), 1, "IN5: cowbells play one sample");
                }
                if let Some(&(_, ff, ramp, intensity)) = SPEC_6_1.iter().find(|r| r.0 == label)
                    && let Some(s) = table.lookup(script, song, q, i, c4).next()
                    && (s.fire_and_forget != ff
                        || (s.ramp_out - ramp).abs() > 1e-4
                        || (s.intensity() - intensity).abs() > 1e-4)
                {
                    mismatches_6_1 += 1;
                }
            }
        }
    }
    eprintln!(
        "IN5 holds for {} instruments; {mismatches_6_1} (instrument, script, branch) rows differ from §6.1",
        table.instruments().len()
    );

    // Render the tune through every instrument (and the Default set).
    let step = if full() { 1 } else { 9 };
    let perf = abc::parse(&tune(step), PartRequest::Unspecified).expect("the test tune reads");
    let table = Arc::new(table);
    let mut instruments: Vec<u32> = table
        .instruments()
        .iter()
        .copied()
        .filter(|&i| label_of(i) != "Undef")
        .collect();
    instruments.push(table.other_instrument());
    let (mut decoded, mut notes, mut accepted, mut frames_out) = (0usize, 0u64, 0u64, 0usize);
    let mut decoded_dids = HashSet::new();
    for &instrument in &instruments {
        // Samples this instrument needs for the tune.
        let mut bank = SampleBank::new();
        let mut expected = 0u64;
        for e in perf.note_ons() {
            let key = table.pitch_key(e.pitch) as u8;
            let sounds: Vec<_> = table
                .lookup(MusicScript::Normal, true, false, instrument, key)
                .collect();
            if !sounds.is_empty() {
                expected += 1;
            }
            for s in sounds {
                let info = table.sound_info(s.sound_info).expect("sound info");
                if bank.contains(info.payload) {
                    continue;
                }
                let rec = set.read(info.payload.0).expect("IN6: the payload exists");
                let Decoded::Pcm(pcm) = decode_record(&rec).expect("IN6: the payload decodes")
                else {
                    panic!("IN6: payload {} is not audio", info.payload);
                };
                assert_eq!((pcm.channels, pcm.sample_rate), (1, 44_100), "IN6");
                decoded_dids.insert(info.payload);
                decoded += 1;
                bank.insert(info.payload, pcm);
            }
        }
        let mut engine = AudioEngine::new(EngineConfig {
            voice_slots: 1024,
            ..EngineConfig::offline(44_100)
        })
        .unwrap();
        engine.set_max_playing_sounds(512);
        let mut player = MusicPlayer::new(Arc::clone(&table), PlayerConfig::default());
        let who = CharacterId(1);
        player
            .add_character(who, LivePosition::new([0.0; 3]))
            .unwrap();
        player.play(who, &perf, instrument, 0.05).unwrap();
        let total = ((perf.end_time() + 0.6) * 44_100.0) as usize;
        let mut buf = vec![0.0f32; 1024 * 2];
        let (mut peak, mut done) = (0.0f32, 0);
        while done < total {
            player.update(&mut engine, &mut bank);
            engine.update();
            engine.render(&mut buf).unwrap();
            engine.drain_events().for_each(drop);
            for &s in &buf {
                assert!(s.is_finite());
                peak = peak.max(s.abs());
            }
            done += 1024;
        }
        frames_out += done;
        let st = player.stats();
        assert_eq!(st.missing_sample, 0, "{}", label_of(instrument));
        assert_eq!(st.missing_sound_info, 0);
        assert_eq!(st.late_dropped, 0);
        assert_eq!(
            st.voices_refused + st.voices_skipped,
            0,
            "{}",
            label_of(instrument)
        );
        assert_eq!(st.voices_accepted, expected, "{}", label_of(instrument));
        assert!(peak > 0.0, "{} is audible", label_of(instrument));
        notes += st.note_ons;
        accepted += st.voices_accepted;
    }
    eprintln!(
        "rendered {} instruments (+ default): {notes} note-ons, {accepted} voices, {decoded} sample decodes ({} distinct), {:.1} s of audio; total {:.2?}",
        instruments.len() - 1,
        decoded_dids.len(),
        frames_out as f64 / 44_100.0,
        t0.elapsed()
    );
}
