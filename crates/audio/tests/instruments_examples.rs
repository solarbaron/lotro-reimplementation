// SPDX-License-Identifier: GPL-3.0-or-later
//! The synthetic worked examples of `docs/specs/instruments.md` §14 and the behaviour of
//! §5–§10 and `abc-dialect.md` §8.5/§11 on hand-built scripts, rendered through the
//! engine's offline backend where audio is involved.
//!
//! Every tree, sound info and sample here is synthetic (built by this file); PIDs and keys
//! are made-up values standing in for what the data supplies.

use std::collections::HashMap;
use std::sync::Arc;

use rand::SeedableRng;
use rand::rngs::SmallRng;
use tessera_audio::abc::{self, PITCH_SLOTS, PartRequest, note_label};
use tessera_audio::engine::{
    AudioEngine, Channel, EngineConfig, LivePosition, VoiceHandle, VoiceState,
};
use tessera_audio::instruments::{
    CharacterId, Due, InstrumentError, InstrumentTable, LATE_DROP_SECS, MusicEvent, MusicEventKind,
    MusicPlayer, MusicScript, MusicScripts, MusicVocabulary, NoteQueue, NoteSound, PathFactor,
    PlayerConfig, SampleBank, ScriptDef, SyncGroup, TableIssue, TableLimits, note_key, note_volume,
    pack_note,
};
use tessera_audio::pcm::Pcm;
use tessera_formats::script_table::{
    BaseBlock, Node, NodeData, NodeKind, Port, PortPayload, PortType, SWITCH_EQUAL, SoundNode,
    SwitchCase, SwitchNode,
};
use tessera_formats::{Did, PropertyEntry, PropertyValue, Record, SoundInfo};

// Stand-ins for the PIDs and keys the dictionary supplies.
const NOTE: u32 = 0x1000_1159;
const INSTR: u32 = 0x1000_1230;
const VOL: u32 = 0x1000_1DCF;
const SONG: u32 = 0x1000_1709;
const QUANT: u32 = 0x1000_170A;
const INTENSITY: u32 = 0x3A3;
const MAX_INST: u32 = 0x3F2;
const Q_KEY: u32 = 0x1000_0BCD;
const N_KEY: u32 = 0x1000_0BCE;

const CENTRE: f32 = std::f32::consts::FRAC_1_SQRT_2;

fn vocab() -> MusicVocabulary {
    let mut v = MusicVocabulary {
        music_note: NOTE,
        music_instrument_type: INSTR,
        music_note_volume: VOL,
        music_note_from_song: SONG,
        music_note_quantized: QUANT,
        intensity_multiplier: INTENSITY,
        max_num_instances: Some(MAX_INST),
        quantized_script: Q_KEY,
        normal_script: N_KEY,
        ..MusicVocabulary::default()
    };
    v.note_labels = (48u8..=84)
        .map(|p| {
            (
                u32::from(PITCH_SLOTS[usize::from(p - 48)]),
                note_label(p).unwrap(),
            )
        })
        .collect();
    v.note_labels.push((0, "Undef".into()));
    v.instrument_labels = vec![(1, "A".into()), (4, "B".into()), (10, "C".into())];
    v.derive_pitch_keys();
    v
}

fn base(mult: f32) -> BaseBlock {
    BaseBlock {
        use_parent_intensity: true,
        intensity_multiplier: mult,
        intensity_variance: 0.0,
        probability: 1.0,
        node_id: 1,
        ..BaseBlock::default()
    }
}

fn node(kind: NodeKind, data: NodeData, base: BaseBlock, ports: Vec<Port>) -> Node {
    Node {
        kind,
        data,
        base,
        particle_flag: None,
        ports,
        targets: Vec::new(),
    }
}

fn default_port(children: Vec<Node>) -> Port {
    Port {
        port_type: PortType::DEFAULT,
        payload: PortPayload::None,
        children,
    }
}

fn collection(mult: f32, children: Vec<Node>) -> Node {
    node(
        NodeKind::COLLECTION,
        NodeData::None,
        base(mult),
        vec![default_port(children)],
    )
}

/// The root Collection whose multiplier is driven by `Music_Note_Volume` (§5.6).
fn driven_root(children: Vec<Node>) -> Node {
    let mut n = collection(1.0, children);
    n.base.driven = vec![(INTENSITY, VOL)];
    n
}

fn switch(
    property: u32,
    cases: Vec<(PropertyValue, Vec<Node>)>,
    default: Option<Vec<Node>>,
) -> Node {
    let mut ports: Vec<Port> = cases
        .into_iter()
        .map(|(value, children)| Port {
            port_type: PortType::SWITCH_VALUE,
            payload: PortPayload::SwitchCase(Box::new(SwitchCase {
                values: vec![Some(PropertyEntry {
                    pid: property,
                    value,
                })],
                operator: SWITCH_EQUAL,
                ..SwitchCase::default()
            })),
            children,
        })
        .collect();
    if let Some(d) = default {
        ports.push(default_port(d));
    }
    node(
        NodeKind::SWITCH,
        NodeData::Switch(SwitchNode {
            property,
            persistent: false,
            autonomous: false,
            synchronized: false,
        }),
        base(1.0),
        ports,
    )
}

fn sound(info: u32, fire_and_forget: bool, ramp: f32, channel: u32) -> Node {
    node(
        NodeKind::SOUND,
        NodeData::Sound(Box::new(SoundNode {
            sound: Did(info),
            fire_and_forget,
            ramp_out_time: ramp,
            channel,
            ..SoundNode::default()
        })),
        base(1.0),
        Vec::new(),
    )
}

fn e(v: u32) -> PropertyValue {
    PropertyValue::Enum(v)
}

fn yes() -> PropertyValue {
    PropertyValue::Bool(true)
}

/// A sound-info record laid out as in `instruments.md` §4.2 / §14.1.
fn sound_info_bytes(did: u32, flags: u32, min: f32, max: f32, payload: u32) -> Vec<u8> {
    let mut b = Vec::new();
    b.extend(did.to_le_bytes());
    b.extend(flags.to_le_bytes());
    b.extend((-1.0f32).to_le_bytes());
    b.extend((-1.0f32).to_le_bytes());
    b.extend(1.0f32.to_le_bytes());
    b.extend(min.to_le_bytes());
    b.extend(max.to_le_bytes());
    b.extend(payload.to_le_bytes());
    b.push(1);
    b.push(b'x');
    b
}

/// (sound info, flags, min, max, payload)
type Info = (u32, u32, f32, f32, u32);

fn build_with(
    quantized: Vec<Node>,
    normal: Vec<Node>,
    infos: &[Info],
    max_instances: u32,
    limits: TableLimits,
) -> Result<InstrumentTable, InstrumentError> {
    let scripts = MusicScripts {
        quantized: ScriptDef {
            key: Q_KEY,
            table: None,
            roots: quantized,
            max_instances,
        },
        normal: ScriptDef {
            key: N_KEY,
            table: None,
            roots: normal,
            max_instances,
        },
        defining_tables: Vec::new(),
    };
    let map: HashMap<u32, Vec<u8>> = infos
        .iter()
        .map(|&(d, f, mn, mx, p)| (d, sound_info_bytes(d, f, mn, mx, p)))
        .collect();
    InstrumentTable::build(scripts, &vocab(), limits, |d: Did| {
        Ok::<_, String>(map.get(&d.0).cloned())
    })
}

fn build(
    quantized: Vec<Node>,
    normal: Vec<Node>,
    infos: &[Info],
    max_instances: u32,
) -> InstrumentTable {
    build_with(
        quantized,
        normal,
        infos,
        max_instances,
        TableLimits::default(),
    )
    .unwrap()
}

/// Constant 1.0 samples, mono.
fn dc(rate: u32, secs: f64) -> Pcm {
    Pcm {
        sample_rate: rate,
        channels: 1,
        samples: vec![1.0f32; (secs * f64::from(rate)) as usize].into(),
    }
}

fn engine(rate: u32) -> AudioEngine {
    AudioEngine::new(EngineConfig::offline(rate)).unwrap()
}

/// Updates the player and the engine and renders `frames` frames in blocks of `block`;
/// returns the left channel.
fn run(
    player: &mut MusicPlayer,
    e: &mut AudioEngine,
    bank: &mut SampleBank,
    frames: usize,
    block: usize,
) -> Vec<f32> {
    let mut left = Vec::with_capacity(frames);
    let mut done = 0;
    while done < frames {
        let n = block.min(frames - done);
        player.update(e, bank);
        e.update();
        let mut buf = vec![0.0f32; n * 2];
        e.render(&mut buf).unwrap();
        left.extend(buf.iter().step_by(2));
        e.drain_events().for_each(drop);
        done += n;
    }
    left
}

fn close(a: f32, b: f32, tol: f32) -> bool {
    (a - b).abs() <= tol
}

fn on(time: f64, who: CharacterId, instrument: u32, packed: u32) -> MusicEvent {
    MusicEvent {
        time,
        seq: 0,
        kind: MusicEventKind::NoteOn,
        character: who,
        instrument,
        packed,
        quantized: false,
        from_song: true,
    }
}

fn off(time: f64, who: CharacterId, packed: u32) -> MusicEvent {
    MusicEvent {
        kind: MusicEventKind::NoteOff,
        ..on(time, who, 0, packed)
    }
}

const A: CharacterId = CharacterId(1);
const B: CharacterId = CharacterId(2);

// ---------------------------------------------------------------- §14.1

#[test]
fn sound_info_14_1() {
    let bytes = sound_info_bytes(0x2A00_1234, 4, 3.0, 30.0, 0x0A00_5678);
    let spec: [u8; 34] = [
        0x34, 0x12, 0x00, 0x2A, 0x04, 0x00, 0x00, 0x00, 0x00, 0x00, 0x80, 0xBF, 0x00, 0x00, 0x80,
        0xBF, 0x00, 0x00, 0x80, 0x3F, 0x00, 0x00, 0x40, 0x40, 0x00, 0x00, 0xF0, 0x41, 0x78, 0x56,
        0x00, 0x0A, 0x01, 0x78,
    ];
    assert_eq!(bytes, spec);
    let si = SoundInfo::parse(&bytes).unwrap();
    assert!(!si.is_loop());
    assert_eq!((si.min_distance, si.max_distance), (3.0, 30.0));
    assert_eq!(si.payload, Did(0x0A00_5678));
    let looping = sound_info_bytes(0x2A00_1234, 5, 3.0, 30.0, 0x0A00_5678);
    assert!(SoundInfo::parse(&looping).unwrap().is_loop());
    for n in 0..bytes.len() {
        assert!(SoundInfo::parse(&bytes[..n]).is_err(), "truncated at {n}");
    }
}

// ---------------------------------------------------------------- §14.2

#[test]
fn pitch_key_and_packed_note_14_2() {
    let v = vocab();
    assert_eq!(note_label(69).as_deref(), Some("Note_A3"));
    assert_eq!(v.pitch_key(69), 11);
    // The label rule gives 37 distinct non-zero keys (IN10) equal to the pitch-slot table.
    let keys: Vec<u32> = (48u8..=84).map(|p| v.pitch_key(p)).collect();
    let mut sorted = keys.clone();
    sorted.sort_unstable();
    sorted.dedup();
    assert_eq!(sorted.len(), 37);
    assert!(keys.iter().all(|&k| k != 0));
    assert_eq!(v.pitch_key(47), 0);
    assert_eq!(v.pitch_key(85), 0);

    let w = pack_note(v.pitch_key(69), 105);
    assert_eq!(w, 0x6900_000B);
    assert!(close(note_volume(w), 0.826_772, 1e-6));
    assert_eq!(note_volume(0x8000_000B), 0.0);
    assert_eq!(pack_note(11, 128), 0x0000_000B);
    assert_eq!(note_key(0x6900_170B), 0x0B);
}

// ---------------------------------------------------------------- §14.3

const SA: u32 = 0x2A00_0001;
const SB: u32 = 0x2A00_0002;
const SC: u32 = 0x2A00_0003;

/// The tree of §14.3.
fn tree_14_3() -> Node {
    driven_root(vec![switch(
        SONG,
        vec![(
            yes(),
            vec![switch(
                INSTR,
                vec![(
                    e(4),
                    vec![collection(
                        0.5,
                        vec![switch(
                            NOTE,
                            vec![
                                (e(28), vec![sound(SA, true, 0.0, 6)]),
                                (e(12), vec![sound(SB, false, 0.2, 6)]),
                            ],
                            None,
                        )],
                    )],
                )],
                Some(vec![collection(
                    0.7,
                    vec![switch(
                        NOTE,
                        vec![(e(28), vec![sound(SC, false, 0.2, 6)])],
                        None,
                    )],
                )]),
            )],
        )],
        None,
    )])
}

fn infos_14_3() -> Vec<Info> {
    vec![
        (SA, 4, 3.0, 30.0, 0x0A00_0001),
        (SB, 4, 3.0, 30.0, 0x0A00_0002),
        (SC, 4, 3.0, 30.0, 0x0A00_0003),
    ]
}

#[test]
fn script_choice_and_table_lookup_14_3() {
    assert_eq!(MusicScript::for_note(true), MusicScript::Quantized);
    assert_eq!(MusicScript::for_note(false), MusicScript::Normal);
    let t = build(Vec::new(), vec![tree_14_3()], &infos_14_3(), 10);
    assert!(t.issues().is_empty(), "{:?}", t.issues());
    let n = MusicScript::Normal;
    let one = |song: bool, q: bool, instr: u32, key: u8| -> Option<&NoteSound> {
        let v: Vec<&NoteSound> = t.lookup(n, song, q, instr, key).collect();
        assert!(v.len() <= 1);
        v.first().copied()
    };
    let mut rng = SmallRng::seed_from_u64(1);
    let vol = note_volume(pack_note(28, 90));
    let a = one(true, false, 4, 28).expect("Sound A");
    assert_eq!(a.sound_info, Did(SA));
    assert!(a.fire_and_forget);
    assert!(a.volume_driven());
    assert!(close(a.intensity(), 0.5, 1e-6));
    assert!(close(
        a.volume(vol, &mut rng).unwrap(),
        0.5 * 90.0 / 127.0,
        1e-6
    ));
    let b = one(true, false, 4, 12).expect("Sound B");
    assert_eq!(b.sound_info, Did(SB));
    assert!(!b.fire_and_forget);
    assert!(close(b.ramp_out, 0.2, 1e-6));
    assert!(close(b.intensity(), 0.5, 1e-6));
    assert!(one(true, false, 4, 2).is_none());
    let c = one(true, false, 99, 28).expect("Sound C through the Default port");
    assert_eq!(c.sound_info, Did(SC));
    assert!(close(
        c.volume(vol, &mut rng).unwrap(),
        0.7 * 90.0 / 127.0,
        1e-6
    ));
    assert!(
        one(false, false, 4, 28).is_none(),
        "no Default on the from-song switch"
    );
    // The quantized script is empty here: nothing for any note.
    assert_eq!(
        t.lookup(MusicScript::Quantized, true, true, 4, 28).count(),
        0
    );
    // Sound infos were loaded through the fetch function.
    assert_eq!(t.sound_info(Did(SA)).unwrap().max_distance, 30.0);
    assert_eq!(t.instruments(), &[4]);
}

/// §5.7: the table and a live walk agree for every combination.
#[test]
fn table_agrees_with_the_walk() {
    let t = build(vec![tree_14_3()], vec![tree_14_3()], &infos_14_3(), 10);
    let mut checked = 0;
    for script in MusicScript::ALL {
        for song in [false, true] {
            for q in [false, true] {
                for instr in (0..=12).chain([99, 1000, u32::MAX]) {
                    for key in 0..=255u8 {
                        let walked = t.walk(script, song, q, instr, key).unwrap();
                        assert_eq!(t.lookup_ids(script, song, q, instr, key), &walked[..]);
                        checked += 1;
                    }
                }
            }
        }
    }
    assert!(checked > 10_000);
}

// ---------------------------------------------------------------- §14.4

const LUTE: u32 = 4;
const LUTE_INFO: u32 = 0x2A00_0010;
const LUTE_SAMPLE: u32 = 0x0A00_0010;

fn lute_tree() -> Node {
    driven_root(vec![switch(
        SONG,
        vec![(
            yes(),
            vec![switch(
                INSTR,
                vec![(
                    e(LUTE),
                    vec![collection(
                        0.34,
                        vec![switch(
                            NOTE,
                            vec![(e(28), vec![sound(LUTE_INFO, true, 0.0, 6)])],
                            None,
                        )],
                    )],
                )],
                None,
            )],
        )],
        None,
    )])
}

fn lute_player(flags: u32, rate: u32) -> (MusicPlayer, SampleBank) {
    let t = build(
        Vec::new(),
        vec![lute_tree()],
        &[(LUTE_INFO, flags, 3.0, 30.0, LUTE_SAMPLE)],
        10,
    );
    let mut bank = SampleBank::new();
    bank.insert(Did(LUTE_SAMPLE), dc(rate, 1.0));
    (MusicPlayer::new(Arc::new(t), PlayerConfig::default()), bank)
}

/// Plays one song note of key 28 at velocity 90 for a character at `pos`; returns its
/// voice, if any.
fn lute_note(flags: u32, pos: [f32; 3]) -> (Option<f32>, MusicPlayer) {
    let mut e = engine(16_000);
    e.set_channel_volume(Channel::PLAYER_MUSIC, 0.5);
    let (mut p, mut bank) = lute_player(flags, 16_000);
    p.add_character(A, LivePosition::new(pos)).unwrap();
    p.queue_event(on(0.0, A, LUTE, pack_note(28, 90))).unwrap();
    p.update(&mut e, &mut bank);
    let gain = p.voices(A).first().and_then(|&h| e.voice_gain(h));
    (gain, p)
}

#[test]
fn loudness_14_4() {
    // 10 m: volume 0.240945, distance factor 0.740741, sent gain 0.089239.
    let (g, _) = lute_note(4, [10.0, 0.0, 0.0]);
    assert!(close(g.unwrap(), 0.089_239, 1e-5), "{g:?}");
    // 2 m: factor 1, gain min(v, v × 0.5).
    let (g, _) = lute_note(4, [2.0, 0.0, 0.0]);
    assert!(close(g.unwrap(), 0.240_945 * 0.5, 1e-5), "{g:?}");
    // 31 m: not started (§9.3).
    let (g, p) = lute_note(4, [31.0, 0.0, 0.0]);
    assert!(g.is_none());
    assert_eq!(p.stats().voices_refused, 1);
    // Flags 0 instead of 4: no distance law.
    let (g, _) = lute_note(0, [31.0, 0.0, 0.0]);
    assert!(close(g.unwrap(), 0.240_945 * 0.5, 1e-5), "{g:?}");

    // Node intensity (§10.1): 0.6 (driven) × 0.5 × 0.8 = 0.24; a Sound multiplier of 2.0
    // with variance 0 is not clamped (0.6); with variance 0.1 the factor is clamped to 1.
    let f = |multiplier: f32, variance: f32, volume_driven: bool| PathFactor {
        multiplier,
        variance,
        use_parent: true,
        probability: 1.0,
        volume_driven,
        node: 0,
    };
    let mut s = NoteSound {
        sound_info: Did(LUTE_INFO),
        fire_and_forget: true,
        move_with_entity: true,
        use_camera_position: false,
        random_distance: (0.0, 0.0),
        offset: [0.0; 3],
        channel: 6,
        ramp_out: 0.0,
        pitch: 0.0,
        pitch_variance: 0.0,
        node_id: 0,
        path: vec![f(1.0, 0.0, true), f(0.5, 0.0, false), f(0.8, 0.0, false)],
    };
    let mut rng = SmallRng::seed_from_u64(3);
    assert!(close(s.volume(0.6, &mut rng).unwrap(), 0.24, 1e-6));
    s.path[2] = f(2.0, 0.0, false);
    assert!(close(s.volume(0.6, &mut rng).unwrap(), 0.6, 1e-6));
    s.path[2] = f(2.0, 0.1, false);
    for _ in 0..16 {
        assert!(close(s.volume(0.6, &mut rng).unwrap(), 0.3, 1e-6));
    }
}

/// A packed velocity above 127 plays at volume 0, so a fire-and-forget note is under the
/// audibility floor (§10.3).
#[test]
fn velocity_bits_above_127_are_silent() {
    let mut e = engine(16_000);
    let (mut p, mut bank) = lute_player(4, 16_000);
    p.add_character(A, LivePosition::new([0.0; 3])).unwrap();
    p.queue_event(on(0.0, A, LUTE, 0x8000_001C)).unwrap();
    p.update(&mut e, &mut bank);
    assert_eq!(p.stats().below_floor, 1);
    assert_eq!(p.stats().voices_accepted, 0);
}

// ---------------------------------------------------------------- §14.5 and §12

const SUS_INFO: u32 = 0x2A00_0020;
const SUS_SAMPLE: u32 = 0x0A00_0020;

/// A quantized-script instrument (velocity ignored) holding a 0.6 sustained note of key 29.
fn sustained_tree(info: u32) -> Node {
    collection(
        1.0,
        vec![switch(
            INSTR,
            vec![(
                e(1),
                vec![collection(
                    0.6,
                    vec![switch(
                        NOTE,
                        vec![(e(29), vec![sound(info, false, 0.2, 5)])],
                        None,
                    )],
                )],
            )],
            None,
        )],
    )
}

fn quantized_on(time: f64, packed: u32) -> MusicEvent {
    MusicEvent {
        quantized: true,
        from_song: false,
        ..on(time, A, 1, packed)
    }
}

fn quantized_off(time: f64, packed: u32) -> MusicEvent {
    MusicEvent {
        quantized: true,
        from_song: false,
        ..off(time, A, packed)
    }
}

#[test]
fn release_14_5_and_sample_accurate_timing() {
    const RATE: u32 = 48_000;
    let t = build(
        vec![sustained_tree(SUS_INFO)],
        Vec::new(),
        &[(SUS_INFO, 4, 3.0, 30.0, SUS_SAMPLE)],
        10,
    );
    let mut bank = SampleBank::new();
    bank.insert(Did(SUS_SAMPLE), dc(RATE, 2.0));
    let mut p = MusicPlayer::new(Arc::new(t), PlayerConfig::default());
    let mut e = engine(RATE);
    p.add_character(A, LivePosition::new([0.0; 3])).unwrap();
    // Velocity 30: ignored by the quantized script (§10.2), the volume is 0.6.
    p.queue_event(quantized_on(0.1, pack_note(29, 30))).unwrap();
    p.queue_event(quantized_off(0.5, pack_note(29, 30)))
        .unwrap();
    // Blocks of 7 ms do not line up with the event frames.
    let out = run(&mut p, &mut e, &mut bank, RATE as usize, 336);
    let start = 4800;
    let stop = 24_000;
    assert_eq!(out[start - 1], 0.0, "nothing before the note-on frame");
    let level = 0.6 * CENTRE;
    assert!(
        close(out[start], level, 1e-4),
        "starts at its frame: {}",
        out[start]
    );
    assert!(close(out[stop - 1], level, 1e-4));
    let at = |ms: usize| out[stop + ms * 48] / level;
    // 0.57, 0.45, 0.30, 0 of 0.6 after 10, 50, 100 and 200 ms (§14.5).
    assert!(close(at(10), 0.95, 0.01), "{}", at(10));
    assert!(close(at(50), 0.75, 0.01), "{}", at(50));
    assert!(close(at(100), 0.5, 0.01), "{}", at(100));
    assert!(close(at(200), 0.0, 0.01), "{}", at(200));
    assert!(out[stop + 210 * 48..].iter().all(|&s| s == 0.0));
    assert_eq!(p.stats().note_ons, 1);
    assert_eq!(p.stats().voices_accepted, 1);
}

#[test]
fn ramp_zero_stops_at_once() {
    const RATE: u32 = 16_000;
    let tree = collection(
        1.0,
        vec![switch(
            INSTR,
            vec![(
                e(1),
                vec![collection(
                    0.6,
                    vec![switch(
                        NOTE,
                        vec![(e(29), vec![sound(SUS_INFO, false, 0.0, 5)])],
                        None,
                    )],
                )],
            )],
            None,
        )],
    );
    let t = build(
        vec![tree],
        Vec::new(),
        &[(SUS_INFO, 4, 3.0, 30.0, SUS_SAMPLE)],
        10,
    );
    let mut bank = SampleBank::new();
    bank.insert(Did(SUS_SAMPLE), dc(RATE, 2.0));
    let mut p = MusicPlayer::new(Arc::new(t), PlayerConfig::default());
    let mut e = engine(RATE);
    p.add_character(A, LivePosition::new([0.0; 3])).unwrap();
    p.queue_event(quantized_on(0.1, pack_note(29, 90))).unwrap();
    p.queue_event(quantized_off(0.5, pack_note(29, 90)))
        .unwrap();
    let out = run(&mut p, &mut e, &mut bank, RATE as usize, 160);
    assert!(out[7999] > 0.4);
    assert!(out[8000..].iter().all(|&s| s == 0.0));
}

// ---------------------------------------------------------------- §14.6

#[test]
fn queue_order_and_late_drop_14_6() {
    let ev = |time: f64, seq: u64, kind: MusicEventKind, key: u32| MusicEvent {
        time,
        seq,
        kind,
        character: A,
        instrument: 1,
        packed: pack_note(key, 90),
        quantized: false,
        from_song: true,
    };
    let fill = || {
        let mut q = NoteQueue::new();
        q.push(ev(1.000_00, 7, MusicEventKind::NoteOff, 29));
        q.push(ev(1.000_04, 8, MusicEventKind::NoteOn, 29));
        q.push(ev(0.90, 5, MusicEventKind::NoteOn, 12));
        q
    };
    let describe = |d: Due| match d {
        Due::Ready(e) => (true, e.kind, e.key()),
        Due::Late(e) => (false, e.kind, e.key()),
    };
    let mut q = fill();
    let order: Vec<_> = std::iter::from_fn(|| q.pop_due(1.45, 1.45))
        .map(describe)
        .collect();
    assert_eq!(
        order,
        vec![
            (false, MusicEventKind::NoteOn, 12),
            (true, MusicEventKind::NoteOff, 29),
            (true, MusicEventKind::NoteOn, 29),
        ]
    );
    let mut q = fill();
    let order: Vec<_> = std::iter::from_fn(|| q.pop_due(1.30, 1.30))
        .map(describe)
        .collect();
    assert_eq!(
        order,
        vec![
            (true, MusicEventKind::NoteOn, 12),
            (true, MusicEventKind::NoteOff, 29),
            (true, MusicEventKind::NoteOn, 29),
        ]
    );
    // Nothing later than `until` comes out; note-offs are never late.
    let mut q = NoteQueue::new();
    q.push(ev(5.0, 1, MusicEventKind::NoteOff, 3));
    assert!(q.pop_due(4.0, 4.9).is_none());
    assert!(matches!(q.pop_due(100.0, 100.0), Some(Due::Ready(_))));
    assert!((LATE_DROP_SECS - 0.5).abs() < 1e-12);
}

#[test]
fn late_note_ons_are_dropped_by_the_player() {
    let mut e = engine(16_000);
    let (mut p, mut bank) = lute_player(4, 16_000);
    p.add_character(A, LivePosition::new([0.0; 3])).unwrap();
    run(&mut p, &mut e, &mut bank, 16_000, 160);
    let now = MusicPlayer::now(&e);
    p.queue_event(on(now - 0.6, A, LUTE, pack_note(28, 90)))
        .unwrap();
    p.queue_event(on(now - 0.4, A, LUTE, pack_note(28, 90)))
        .unwrap();
    p.update(&mut e, &mut bank);
    assert_eq!(p.stats().late_dropped, 1);
    assert_eq!(p.stats().note_ons, 1);
    assert_eq!(p.stats().voices_accepted, 1);
}

// ---------------------------------------------------------------- §14.7

const CLARINET: u32 = 1;
const CL_INFO: u32 = 0x2A00_0030;
const CL_SAMPLE: u32 = 0x0A00_0030;

fn clarinet_tree() -> Node {
    let cases = (1u32..=37)
        .map(|k| (e(k), vec![sound(CL_INFO, false, 0.2, 6)]))
        .collect();
    driven_root(vec![switch(
        SONG,
        vec![(
            yes(),
            vec![switch(
                INSTR,
                vec![(
                    e(CLARINET),
                    vec![collection(0.7, vec![switch(NOTE, cases, None)])],
                )],
                None,
            )],
        )],
        Some(vec![switch(
            INSTR,
            vec![(
                e(CLARINET),
                vec![collection(
                    0.7,
                    vec![switch(
                        NOTE,
                        (1u32..=37)
                            .map(|k| (e(k), vec![sound(CL_INFO, false, 0.2, 6)]))
                            .collect(),
                        None,
                    )],
                )],
            )],
            None,
        )]),
    )])
}

fn clarinet_player(rate: u32, sample_secs: f64) -> (MusicPlayer, SampleBank) {
    let t = build(
        Vec::new(),
        vec![clarinet_tree()],
        &[(CL_INFO, 4, 3.0, 30.0, CL_SAMPLE)],
        10,
    );
    let mut bank = SampleBank::new();
    bank.insert(Did(CL_SAMPLE), dc(rate, sample_secs));
    (MusicPlayer::new(Arc::new(t), PlayerConfig::default()), bank)
}

#[test]
fn instance_limit_14_7() {
    const RATE: u32 = 8_000;
    let (mut p, mut bank) = clarinet_player(RATE, 5.0);
    let mut e = engine(RATE);
    p.add_character(A, LivePosition::new([0.0; 3])).unwrap();
    for i in 0..11u32 {
        let t = 0.2 + 0.1 * f64::from(i);
        p.queue_event(on(t, A, CLARINET, pack_note(i + 1, 90)))
            .unwrap();
        p.queue_event(off(t + 3.0, A, pack_note(i + 1, 90)))
            .unwrap();
    }
    run(&mut p, &mut e, &mut bank, RATE as usize * 3 / 2, 80);
    let s = p.stats();
    assert_eq!(s.note_ons, 11);
    assert_eq!(s.limit_refused, 1, "note 11 is refused");
    assert_eq!(s.voices_accepted, 10);
    assert_eq!(p.voices(A).len(), 10);
    assert_eq!(p.active_instances(&e, A, MusicScript::Normal), 10);
    assert!(
        p.is_registered(A, 11),
        "the refused note is still registered"
    );
    assert_eq!(p.registered_keys(A).len(), 11);
    run(&mut p, &mut e, &mut bank, RATE as usize * 3, 80);
    assert_eq!(p.stats().note_offs, 11);
    assert!(p.registered_keys(A).is_empty());
    assert!(p.voices(A).is_empty());
    assert_eq!(p.active_instances(&e, A, MusicScript::Normal), 0);
    // Quantized notes count against the other script (§8.6): the normal-script limit does
    // not block them (the quantized script reaches no sound here).
    assert_eq!(p.table().max_instances(MusicScript::Quantized), 10);
}

/// Fire-and-forget instances complete at once and do not count (Q2 Decision).
#[test]
fn fire_and_forget_notes_do_not_hold_instances() {
    const RATE: u32 = 8_000;
    let mut e = engine(RATE);
    let (mut p, mut bank) = lute_player(4, RATE);
    p.add_character(A, LivePosition::new([0.0; 3])).unwrap();
    for i in 0..15 {
        p.queue_event(on(0.1 + 0.01 * f64::from(i), A, LUTE, pack_note(28, 90)))
            .unwrap();
        p.queue_event(off(0.105 + 0.01 * f64::from(i), A, pack_note(28, 90)))
            .unwrap();
    }
    run(&mut p, &mut e, &mut bank, RATE as usize / 2, 80);
    let s = p.stats();
    assert_eq!(s.limit_refused, 0);
    assert_eq!(s.voices_accepted, 15);
    // The note-off did not stop them: they play to the end of the sample (§9.3).
    assert_eq!(p.voices(A).len(), 15);
}

/// IN16: a packed word with bits 8–23 set registers and turns off under bits 0–7.
#[test]
fn packed_key_bits_in16() {
    const RATE: u32 = 8_000;
    let (mut p, mut bank) = clarinet_player(RATE, 5.0);
    let mut e = engine(RATE);
    p.add_character(A, LivePosition::new([0.0; 3])).unwrap();
    p.queue_event(on(0.0, A, CLARINET, 0x6900_170B)).unwrap();
    run(&mut p, &mut e, &mut bank, 800, 80);
    assert!(p.is_registered(A, 0x0B));
    let v = p.voices(A);
    assert_eq!(v.len(), 1);
    // Volume 105 / 127 × 0.7 at distance 0.
    assert!(close(
        e.voice_gain(v[0]).unwrap(),
        105.0 / 127.0 * 0.7,
        1e-5
    ));
    let now = MusicPlayer::now(&e);
    p.queue_event(off(now, A, 0x0000_000B)).unwrap();
    run(&mut p, &mut e, &mut bank, 80, 80);
    assert!(!p.is_registered(A, 0x0B));
    assert!(matches!(
        e.voice_state(v[0]),
        Some(VoiceState::Stopping) | None
    ));
}

/// §8.4: a second note-on of a registered key plays but is not registered; the note-off
/// stops only the first instance.
#[test]
fn restruck_key_keeps_the_old_entry() {
    const RATE: u32 = 8_000;
    let (mut p, mut bank) = clarinet_player(RATE, 5.0);
    let mut e = engine(RATE);
    p.add_character(A, LivePosition::new([0.0; 3])).unwrap();
    p.queue_event(on(0.0, A, CLARINET, pack_note(5, 90)))
        .unwrap();
    p.queue_event(on(0.05, A, CLARINET, pack_note(5, 90)))
        .unwrap();
    p.queue_event(off(0.2, A, pack_note(5, 90))).unwrap();
    run(&mut p, &mut e, &mut bank, 4000, 80);
    assert_eq!(p.stats().unregistered, 1);
    let live: Vec<VoiceHandle> = p
        .voices(A)
        .into_iter()
        .filter(|&h| e.voice_state(h) == Some(VoiceState::Playing))
        .collect();
    assert_eq!(live.len(), 1, "the second instance plays on");
    assert!(!p.is_registered(A, 5));
}

// ---------------------------------------------------------------- §8.5 cancel, §9.4 loop

#[test]
fn cancel_turns_registered_notes_off() {
    const RATE: u32 = 8_000;
    let (mut p, mut bank) = clarinet_player(RATE, 5.0);
    let mut e = engine(RATE);
    p.add_character(A, LivePosition::new([0.0; 3])).unwrap();
    for k in [3, 4, 5] {
        p.queue_event(on(0.0, A, CLARINET, pack_note(k, 90)))
            .unwrap();
        p.queue_event(off(2.0, A, pack_note(k, 90))).unwrap();
    }
    run(&mut p, &mut e, &mut bank, 800, 80);
    assert_eq!(p.registered_keys(A), vec![3, 4, 5]);
    assert_eq!(p.queued(), 3);
    p.stop(A);
    assert_eq!(
        p.queued(),
        3,
        "three note-offs due now replace the queued events"
    );
    let out = run(&mut p, &mut e, &mut bank, 4000, 80);
    assert!(p.registered_keys(A).is_empty());
    assert_eq!(p.stats().cancels, 1);
    // Released with the 0.2 s ramp: silent soon after.
    assert!(out[2400..].iter().all(|&s| s == 0.0));
}

#[test]
fn looping_samples_sound_until_the_note_off() {
    const RATE: u32 = 8_000;
    const DRONE: u32 = 0x2A00_0040;
    let tree = driven_root(vec![switch(
        SONG,
        vec![(
            yes(),
            vec![switch(
                INSTR,
                vec![(
                    e(9),
                    vec![collection(
                        0.65,
                        vec![switch(
                            NOTE,
                            vec![(e(28), vec![sound(DRONE, false, 0.2, 6)])],
                            None,
                        )],
                    )],
                )],
                None,
            )],
        )],
        None,
    )]);
    let t = build(
        Vec::new(),
        vec![tree],
        &[(DRONE, 5, 3.0, 30.0, 0x0A00_0040)],
        10,
    );
    let mut bank = SampleBank::new();
    bank.insert(Did(0x0A00_0040), dc(RATE, 0.25));
    let mut p = MusicPlayer::new(Arc::new(t), PlayerConfig::default());
    let mut e = engine(RATE);
    p.add_character(A, LivePosition::new([0.0; 3])).unwrap();
    p.queue_event(on(0.1, A, 9, pack_note(28, 127))).unwrap();
    p.queue_event(off(1.1, A, pack_note(28, 127))).unwrap();
    let out = run(&mut p, &mut e, &mut bank, RATE as usize * 2, 80);
    let level = 0.65 * CENTRE;
    assert!(
        close(out[(0.9 * 8000.0) as usize], level, 1e-4),
        "still looping"
    );
    assert!(out[(1.32 * 8000.0) as usize..].iter().all(|&s| s == 0.0));
}

// ---------------------------------------------------------------- live play, §12

#[test]
fn live_notes_use_velocity_90_and_wait_for_performances() {
    const RATE: u32 = 8_000;
    let (mut p, mut bank) = clarinet_player(RATE, 5.0);
    let mut e = engine(RATE);
    p.add_character(A, LivePosition::new([0.0; 3])).unwrap();
    // A live note runs the live branch (from song 0): the Default port of the from-song
    // switch here.
    assert!(p.live_note(A, CLARINET, 7, true, false, 0.0).unwrap());
    run(&mut p, &mut e, &mut bank, 160, 80);
    let v = p.voices(A);
    assert_eq!(v.len(), 1);
    assert!(close(e.voice_gain(v[0]).unwrap(), 90.0 / 127.0 * 0.7, 1e-5));
    assert!(p.live_note(A, CLARINET, 7, false, false, 0.0).unwrap());
    run(&mut p, &mut e, &mut bank, 160, 80);
    assert!(!p.is_registered(A, 7));
    // While a song performance runs, live notes are ignored (I41).
    let perf = abc::parse(b"X:1\nK:C\nC4|\n", PartRequest::Unspecified).unwrap();
    p.play(A, &perf, CLARINET, 10.0).unwrap();
    assert!(!p.live_note(A, CLARINET, 7, true, false, 0.0).unwrap());
    assert!(matches!(
        p.live_note(B, CLARINET, 7, true, false, 0.0),
        Err(InstrumentError::UnknownCharacter(B))
    ));
}

// ---------------------------------------------------------------- start, sync, end

const BELL: u32 = 10;
const BELL_INFO: u32 = 0x2A00_0050;
const BELL_SAMPLE: u32 = 0x0A00_0050;

/// One Sound node for every note, as the cowbells have (§5.5).
fn bell_player(rate: u32) -> (MusicPlayer, SampleBank) {
    let tree = driven_root(vec![switch(
        SONG,
        vec![(
            yes(),
            vec![switch(
                INSTR,
                vec![(
                    e(BELL),
                    vec![collection(1.0, vec![sound(BELL_INFO, true, 0.0, 6)])],
                )],
                None,
            )],
        )],
        None,
    )]);
    let t = build(
        Vec::new(),
        vec![tree],
        &[(BELL_INFO, 4, 3.0, 30.0, BELL_SAMPLE)],
        10,
    );
    let mut bank = SampleBank::new();
    bank.insert(Did(BELL_SAMPLE), dc(rate, 0.1));
    (MusicPlayer::new(Arc::new(t), PlayerConfig::default()), bank)
}

/// An original four-note tune (C D E F, eighth notes at 120 crotchets per minute, `p`).
fn tune() -> abc::Performance {
    abc::parse(
        b"X:1\nT:Test\nL:1/8\nQ:1/4=120\nK:C\n+p+CDEF|\n",
        PartRequest::Unspecified,
    )
    .unwrap()
}

fn first_sound(out: &[f32]) -> usize {
    out.iter()
        .position(|&s| s != 0.0)
        .expect("something sounds")
}

#[test]
fn solo_start_is_0_7_s_after_the_command() {
    const RATE: u32 = 16_000;
    let (mut p, mut bank) = bell_player(RATE);
    let mut e = engine(RATE);
    p.add_character(A, LivePosition::new([0.0; 3])).unwrap();
    run(&mut p, &mut e, &mut bank, 1600, 160);
    let start = p.play_solo(&e, A, &tune(), BELL).unwrap();
    assert!(close(start as f32, 0.8, 1e-9));
    assert!(p.is_performing(A));
    let out = run(&mut p, &mut e, &mut bank, RATE as usize * 2, 160);
    assert_eq!(first_sound(&out) + 1600, (0.8 * f64::from(RATE)) as usize);
    assert!(!p.is_performing(A), "the end event ends the performance");
    assert_eq!(p.stats().voices_accepted, 4);
}

#[test]
fn synced_start_is_common() {
    const RATE: u32 = 16_000;
    let (mut p, mut bank) = bell_player(RATE);
    let mut e = engine(RATE);
    p.add_character(A, LivePosition::new([0.0; 3])).unwrap();
    p.add_character(B, LivePosition::new([0.0; 3])).unwrap();
    let mut group = SyncGroup::new();
    group.register(A, tune(), BELL).unwrap();
    run(&mut p, &mut e, &mut bank, 800, 160);
    group.register(B, tune(), BELL).unwrap();
    group.register(B, tune(), BELL).unwrap();
    assert_eq!(group.len(), 2);
    assert!(group.is_waiting(B));
    run(&mut p, &mut e, &mut bank, 800, 160);
    assert_eq!(p.queued(), 0, "waiting performances are not queued");
    let start = SyncGroup::start_time(MusicPlayer::now(&e));
    let s = group.start(&mut p, start).unwrap();
    assert_eq!(s.characters, vec![A, B]);
    assert!(group.is_empty());
    let out = run(&mut p, &mut e, &mut bank, RATE as usize, 160);
    let first = first_sound(&out);
    assert_eq!(first + 1600, (start * f64::from(RATE)).round() as usize);
    // Both performers' first notes start on the same frame: twice one voice.
    assert!(
        close(out[first], 2.0 * CENTRE * 60.0 / 127.0, 1e-4),
        "{}",
        out[first]
    );
    assert!(!group.withdraw(A));
}

#[test]
fn removing_a_character_cancels_its_music() {
    const RATE: u32 = 8_000;
    let (mut p, mut bank) = clarinet_player(RATE, 5.0);
    let mut e = engine(RATE);
    p.add_character(A, LivePosition::new([0.0; 3])).unwrap();
    p.queue_event(on(0.0, A, CLARINET, pack_note(3, 90)))
        .unwrap();
    p.queue_event(off(3.0, A, pack_note(3, 90))).unwrap();
    run(&mut p, &mut e, &mut bank, 800, 80);
    let v = p.voices(A);
    p.remove_character(&mut e, A);
    assert_eq!(p.queued(), 0);
    assert!(!p.has_character(A));
    assert!(matches!(
        e.voice_state(v[0]),
        Some(VoiceState::Stopping) | None
    ));
    // Events for unknown characters do nothing.
    p.queue_event(on(0.0, A, CLARINET, pack_note(3, 90)))
        .unwrap();
    run(&mut p, &mut e, &mut bank, 80, 80);
    assert_eq!(p.stats().no_character, 1);
}

// ---------------------------------------------------------------- hostile and patched trees

#[test]
fn unsupported_nodes_and_switches_are_reported() {
    let timer = node(
        NodeKind::TIMER,
        NodeData::Opaque { offset: 0, len: 0 },
        base(1.0),
        vec![default_port(vec![sound(SA, true, 0.0, 6)])],
    );
    let odd_switch = switch(
        0x1234,
        vec![(e(1), vec![sound(SB, true, 0.0, 6)])],
        Some(vec![sound(SC, true, 0.0, 6)]),
    );
    let t = build(
        Vec::new(),
        vec![collection(1.0, vec![timer, odd_switch])],
        &infos_14_3(),
        10,
    );
    assert!(t.issues().contains(&TableIssue::UnsupportedNode {
        script: MusicScript::Normal,
        kind: NodeKind::TIMER.0
    }));
    assert!(t.issues().contains(&TableIssue::UnsupportedSwitchProperty {
        script: MusicScript::Normal,
        property: 0x1234
    }));
    // The unknown switch takes its Default port.
    let got: Vec<Did> = t
        .lookup(MusicScript::Normal, true, false, 1, 1)
        .map(|s| s.sound_info)
        .collect();
    assert_eq!(got, vec![Did(SC)]);
}

#[test]
fn missing_sound_info_is_an_issue_and_plays_nothing() {
    let mut infos = infos_14_3();
    infos.retain(|i| i.0 != SA);
    let t = build(Vec::new(), vec![tree_14_3()], &infos, 10);
    assert!(t.issues().contains(&TableIssue::MissingSoundInfo(Did(SA))));
    let mut p = MusicPlayer::new(Arc::new(t), PlayerConfig::default());
    let mut e = engine(8_000);
    let mut bank = SampleBank::new();
    p.add_character(A, LivePosition::new([0.0; 3])).unwrap();
    p.queue_event(on(0.0, A, 4, pack_note(28, 90))).unwrap();
    p.queue_event(on(0.0, A, 4, pack_note(12, 90))).unwrap();
    p.update(&mut e, &mut bank);
    assert_eq!(p.stats().missing_sound_info, 1);
    assert_eq!(
        p.stats().missing_sample,
        1,
        "Sound B's sample is not in the bank"
    );
}

#[test]
fn table_limits_bound_hostile_trees() {
    let limits = TableLimits {
        max_work: 50,
        ..TableLimits::default()
    };
    assert!(matches!(
        build_with(Vec::new(), vec![tree_14_3()], &infos_14_3(), 10, limits),
        Err(InstrumentError::Limit(_))
    ));
    // A deep chain of Collections stops at the depth limit with an issue.
    let mut n = sound(SA, true, 0.0, 6);
    for _ in 0..300 {
        n = collection(1.0, vec![n]);
    }
    let t = build(Vec::new(), vec![n], &infos_14_3(), 10);
    assert!(t.issues().contains(&TableIssue::TooDeep {
        script: MusicScript::Normal
    }));
    assert_eq!(t.sounds().len(), 0);
}

#[test]
fn queue_capacity_is_bounded() {
    let (_, _) = bell_player(8_000);
    let t = build(Vec::new(), Vec::new(), &[], 10);
    let mut p = MusicPlayer::new(
        Arc::new(t),
        PlayerConfig {
            max_queued_events: 3,
            ..PlayerConfig::default()
        },
    );
    p.add_character(A, LivePosition::new([0.0; 3])).unwrap();
    assert!(matches!(
        p.play(A, &tune(), BELL, 0.0),
        Err(InstrumentError::QueueFull { .. })
    ));
    assert_eq!(p.queued(), 0);
}

/// A bit-mask case matches values it does not name (found by fuzzing): the table must
/// still agree with the walk for unnamed instrument types and note keys.
#[test]
fn bit_mask_cases_stay_exact() {
    use tessera_formats::script_table::{SWITCH_ALL_BITS_SET, SWITCH_ANY_BIT_SET};
    let mut instr = switch(
        INSTR,
        vec![(e(1), vec![sound(SA, true, 0.0, 6)])],
        Some(vec![sound(SC, true, 0.0, 6)]),
    );
    let mut note = switch(NOTE, vec![(e(6), vec![sound(SB, true, 0.0, 6)])], None);
    for (n, op) in [
        (&mut instr, SWITCH_ANY_BIT_SET),
        (&mut note, SWITCH_ALL_BITS_SET),
    ] {
        if let PortPayload::SwitchCase(c) = &mut n.ports[0].payload {
            c.operator = op;
        }
    }
    let t = build(
        Vec::new(),
        vec![collection(1.0, vec![instr, note])],
        &infos_14_3(),
        10,
    );
    let ids = |i: u32, k: u8| -> Vec<Did> {
        t.lookup(MusicScript::Normal, false, false, i, k)
            .map(|s| s.sound_info)
            .collect()
    };
    // Instrument 3 has bit 1 set: Sound A, though no case names 3; 2 takes the Default.
    assert_eq!(ids(3, 0), vec![Did(SA)]);
    assert_eq!(ids(2, 0), vec![Did(SC)]);
    // Note 7 has the bits of 6: Sound B; note 5 does not.
    assert_eq!(ids(2, 7), vec![Did(SC), Did(SB)]);
    assert_eq!(ids(2, 5), vec![Did(SC)]);
    for i in 0..40 {
        for k in 0..=255u8 {
            let w = t.walk(MusicScript::Normal, false, false, i, k).unwrap();
            assert_eq!(
                t.lookup_ids(MusicScript::Normal, false, false, i, k),
                &w[..]
            );
        }
    }
}
