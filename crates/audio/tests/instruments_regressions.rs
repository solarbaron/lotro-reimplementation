// SPDX-License-Identifier: GPL-3.0-or-later
//! Regression tests for review findings on `tessera_audio::instruments`: look-ahead notes
//! and cancel (§8.5), structural Sound-node identity (clones), hostile combination counts,
//! equal-time queue cost, per-character queue shares, refused and synced starts,
//! performances without an end event, shared-ancestor draws (§10.1), parameter types
//! (§5.2), and the data-driven constructors (`MusicScripts::from_tables`, the §8.6 limit,
//! `MusicVocabulary::from_data`).
//!
//! Every tree, record and sample here is synthetic; PIDs, DIDs and keys are made-up values
//! standing in for what the data supplies.

use std::collections::HashMap;
use std::sync::Arc;
use std::time::Instant;

use rand::SeedableRng;
use rand::rngs::SmallRng;
use tessera_audio::abc::{self, PITCH_SLOTS, PartRequest, note_label};
use tessera_audio::engine::{AudioEngine, EngineConfig, LivePosition};
use tessera_audio::instruments::{
    CharacterId, InstrumentError, InstrumentTable, MusicEvent, MusicEventKind, MusicPlayer,
    MusicScript, MusicScripts, MusicVocabulary, NoteDraws, NoteQueue, PlayerConfig, SampleBank,
    ScriptDef, SyncGroup, TableIssue, TableLimits, pack_note,
};
use tessera_audio::pcm::Pcm;
use tessera_formats::reader::write_vle;
use tessera_formats::script_table::{
    BaseBlock, Node, NodeData, NodeKind, Port, PortPayload, PortType, SWITCH_ANY_BIT_SET,
    SWITCH_EQUAL, SoundNode, SwitchCase, SwitchNode,
};
use tessera_formats::{
    Did, EnumMapper, PropertyDictionary, PropertyEntry, PropertySet, PropertyValue, Record, Script,
    ScriptTable, TSize,
};

const NOTE: u32 = 0x1000_1159;
const INSTR: u32 = 0x1000_1230;
const VOL: u32 = 0x1000_1DCF;
const SONG: u32 = 0x1000_1709;
const QUANT: u32 = 0x1000_170A;
const INTENSITY: u32 = 0x3A3;
const MAX_INST: u32 = 0x3F2;
const Q_KEY: u32 = 0x1000_0BCD;
const N_KEY: u32 = 0x1000_0BCE;

const SA: u32 = 0x2A00_0001;
const SB: u32 = 0x2A00_0002;
const SC: u32 = 0x2A00_0003;
const SAMPLE_A: u32 = 0x0A00_0001;
const SAMPLE_B: u32 = 0x0A00_0002;
const SAMPLE_C: u32 = 0x0A00_0003;

const LUTE: u32 = 4;
const BELL: u32 = 10;

const A: CharacterId = CharacterId(1);
const B: CharacterId = CharacterId(2);

// ---------------------------------------------------------------- helpers

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

fn sound(info: u32, fire_and_forget: bool, ramp: f32) -> Node {
    node(
        NodeKind::SOUND,
        NodeData::Sound(Box::new(SoundNode {
            sound: Did(info),
            fire_and_forget,
            ramp_out_time: ramp,
            channel: 6,
            ..SoundNode::default()
        })),
        base(1.0),
        Vec::new(),
    )
}

fn e(v: u32) -> PropertyValue {
    PropertyValue::Enum(v)
}

fn sound_info_bytes(did: u32, flags: u32, payload: u32) -> Vec<u8> {
    let mut b = Vec::new();
    b.extend(did.to_le_bytes());
    b.extend(flags.to_le_bytes());
    b.extend((-1.0f32).to_le_bytes());
    b.extend((-1.0f32).to_le_bytes());
    b.extend(1.0f32.to_le_bytes());
    b.extend(3.0f32.to_le_bytes());
    b.extend(30.0f32.to_le_bytes());
    b.extend(payload.to_le_bytes());
    b.push(1);
    b.push(b'x');
    b
}

fn infos() -> Vec<(u32, u32)> {
    vec![(SA, SAMPLE_A), (SB, SAMPLE_B), (SC, SAMPLE_C)]
}

fn build_full(
    vocab: &MusicVocabulary,
    quantized: Vec<Node>,
    normal: Vec<Node>,
    limits: TableLimits,
) -> Result<InstrumentTable, InstrumentError> {
    let scripts = MusicScripts {
        quantized: ScriptDef {
            key: Q_KEY,
            table: None,
            roots: quantized,
            max_instances: 10,
        },
        normal: ScriptDef {
            key: N_KEY,
            table: None,
            roots: normal,
            max_instances: 10,
        },
        defining_tables: Vec::new(),
    };
    // Flags 0: no distance law, so a character at the listener is heard at full level.
    let map: HashMap<u32, Vec<u8>> = infos()
        .into_iter()
        .map(|(d, p)| (d, sound_info_bytes(d, 0, p)))
        .collect();
    InstrumentTable::build(scripts, vocab, limits, |d: Did| {
        Ok::<_, String>(map.get(&d.0).cloned())
    })
}

fn build(normal: Vec<Node>) -> InstrumentTable {
    build_full(&vocab(), Vec::new(), normal, TableLimits::default()).unwrap()
}

fn dc(rate: u32, secs: f64) -> Pcm {
    Pcm {
        sample_rate: rate,
        channels: 1,
        samples: vec![1.0f32; (secs * f64::from(rate)) as usize].into(),
    }
}

fn bank(rate: u32, secs: f64) -> SampleBank {
    let mut b = SampleBank::new();
    for (_, p) in infos() {
        b.insert(Did(p), dc(rate, secs));
    }
    b
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

/// Renders without updating the player.
fn render_only(e: &mut AudioEngine, frames: usize) -> Vec<f32> {
    e.update();
    let mut buf = vec![0.0f32; frames * 2];
    e.render(&mut buf).unwrap();
    buf.iter().step_by(2).copied().collect()
}

fn peak(out: &[f32]) -> f32 {
    out.iter().fold(0.0f32, |m, s| m.max(s.abs()))
}

fn ev(
    kind: MusicEventKind,
    time: f64,
    who: CharacterId,
    instrument: u32,
    packed: u32,
) -> MusicEvent {
    MusicEvent {
        time,
        seq: 0,
        kind,
        character: who,
        instrument,
        packed,
        quantized: false,
        from_song: true,
    }
}

fn on(time: f64, who: CharacterId, instrument: u32, packed: u32) -> MusicEvent {
    ev(MusicEventKind::NoteOn, time, who, instrument, packed)
}

fn off(time: f64, who: CharacterId, packed: u32) -> MusicEvent {
    ev(MusicEventKind::NoteOff, time, who, 0, packed)
}

/// A lute-like fire-and-forget instrument: song notes of key 28 on `LUTE` reach Sound A.
fn lute_tree() -> Node {
    driven_root(vec![switch(
        SONG,
        vec![(
            PropertyValue::Bool(true),
            vec![switch(
                INSTR,
                vec![(
                    e(LUTE),
                    vec![switch(
                        NOTE,
                        vec![(e(28), vec![sound(SA, true, 0.0)])],
                        None,
                    )],
                )],
                None,
            )],
        )],
        None,
    )])
}

/// One fire-and-forget Sound for every song note on `BELL`.
fn bell_tree() -> Node {
    driven_root(vec![switch(
        SONG,
        vec![(
            PropertyValue::Bool(true),
            vec![switch(
                INSTR,
                vec![(e(BELL), vec![sound(SB, true, 0.0)])],
                None,
            )],
        )],
        None,
    )])
}

fn player(normal: Vec<Node>, config: PlayerConfig) -> MusicPlayer {
    let mut p = MusicPlayer::new(Arc::new(build(normal)), config);
    p.add_character(A, LivePosition::new([0.0; 3])).unwrap();
    p.add_character(B, LivePosition::new([0.0; 3])).unwrap();
    p
}

fn tune() -> abc::Performance {
    abc::parse(
        b"X:1\nT:Test\nL:1/8\nQ:1/4=120\nK:C\n+p+CDEF|\n",
        PartRequest::Unspecified,
    )
    .unwrap()
}

// ---------------------------------------------------------------- §8.5 and look-ahead

const RATE: u32 = 16_000;

/// A fire-and-forget note handed to the engine ahead of its time (0.08 s, inside the
/// 0.1 s look-ahead) and then cancelled before it is due must stay silent, as its queued
/// note-on would have been removed (§8.5).
#[test]
fn look_ahead_notes_are_withdrawn_by_a_cancel() {
    // Control: without a cancel the note sounds.
    let mut e = engine(RATE);
    let mut b = bank(RATE, 1.0);
    let mut p = player(vec![lute_tree()], PlayerConfig::default());
    p.queue_event(on(0.08, A, LUTE, pack_note(28, 127)))
        .unwrap();
    p.update(&mut e, &mut b);
    assert_eq!(p.stats().voices_accepted, 1, "dispatched ahead");
    assert!(peak(&run(&mut p, &mut e, &mut b, RATE as usize / 2, 160)) > 0.5);

    // stop(): withdrawn at the next update.
    let mut e = engine(RATE);
    let mut p = player(vec![lute_tree()], PlayerConfig::default());
    p.queue_event(on(0.08, A, LUTE, pack_note(28, 127)))
        .unwrap();
    p.update(&mut e, &mut b);
    assert_eq!(p.stats().voices_accepted, 1);
    p.stop(A);
    assert_eq!(
        peak(&run(&mut p, &mut e, &mut b, RATE as usize / 2, 160)),
        0.0
    );

    // stop_now(): withdrawn at once, without another update.
    let mut e = engine(RATE);
    let mut p = player(vec![lute_tree()], PlayerConfig::default());
    p.queue_event(on(0.08, A, LUTE, pack_note(28, 127)))
        .unwrap();
    p.update(&mut e, &mut b);
    p.stop_now(&mut e, A);
    assert_eq!(peak(&render_only(&mut e, RATE as usize / 2)), 0.0);

    // remove_character(): the same.
    let mut e = engine(RATE);
    let mut p = player(vec![lute_tree()], PlayerConfig::default());
    p.queue_event(on(0.08, A, LUTE, pack_note(28, 127)))
        .unwrap();
    p.update(&mut e, &mut b);
    p.remove_character(&mut e, A);
    assert_eq!(peak(&render_only(&mut e, RATE as usize / 2)), 0.0);

    // play() replacing a performance whose first note was handed ahead.
    let mut e = engine(RATE);
    let mut p = player(vec![bell_tree()], PlayerConfig::default());
    p.play(A, &tune(), BELL, 0.05).unwrap();
    p.update(&mut e, &mut b);
    assert_eq!(p.stats().voices_accepted, 1);
    p.play(A, &tune(), BELL, 10.0).unwrap();
    assert_eq!(
        peak(&run(&mut p, &mut e, &mut b, RATE as usize / 2, 160)),
        0.0
    );
    assert!(p.is_performing(A));
}

/// A note that has started before the cancel plays on (a fire-and-forget sound is not
/// stopped by the cancel, §8.5, §9.3).
#[test]
fn started_fire_and_forget_notes_play_on_after_a_cancel() {
    let mut e = engine(RATE);
    let mut b = bank(RATE, 1.0);
    let mut p = player(vec![lute_tree()], PlayerConfig::default());
    p.queue_event(on(0.0, A, LUTE, pack_note(28, 127))).unwrap();
    run(&mut p, &mut e, &mut b, 1600, 160);
    p.stop(A);
    assert!(peak(&run(&mut p, &mut e, &mut b, 1600, 160)) > 0.5);
}

// ---------------------------------------------------------------- table identity

fn tree_14_3() -> Node {
    driven_root(vec![switch(
        SONG,
        vec![(
            PropertyValue::Bool(true),
            vec![switch(
                INSTR,
                vec![(
                    e(4),
                    vec![collection(
                        0.5,
                        vec![switch(
                            NOTE,
                            vec![
                                (e(28), vec![sound(SA, true, 0.0)]),
                                (e(12), vec![sound(SB, false, 0.2)]),
                            ],
                            None,
                        )],
                    )],
                )],
                Some(vec![collection(
                    0.7,
                    vec![switch(
                        NOTE,
                        vec![(e(28), vec![sound(SC, false, 0.2)])],
                        None,
                    )],
                )]),
            )],
        )],
        None,
    )])
}

fn agree(t: &InstrumentTable, instruments: &[u32]) -> usize {
    let mut found = 0;
    for script in MusicScript::ALL {
        for song in [false, true] {
            for q in [false, true] {
                for &i in instruments {
                    for k in 0..=255u8 {
                        let w = t.walk(script, song, q, i, k).unwrap();
                        assert_eq!(t.lookup_ids(script, song, q, i, k), &w[..]);
                        found += w.len();
                    }
                }
            }
        }
    }
    found
}

/// A clone (with the original dropped) walks and looks up exactly like the original.
#[test]
fn a_cloned_table_answers_like_the_original() {
    let t = build(vec![tree_14_3()]);
    assert_eq!(
        t.walk(MusicScript::Normal, true, false, 4, 28).unwrap(),
        vec![0]
    );
    let reference = agree(&t, &[0, 4, 99]);
    assert!(reference > 0);
    let c = t.clone();
    drop(t);
    assert_eq!(
        c.walk(MusicScript::Normal, true, false, 4, 28).unwrap(),
        vec![0]
    );
    assert_eq!(agree(&c, &[0, 4, 99]), reference);

    // An unnamed instrument type with a bit-mask case is looked up by a walk.
    let mut instr = switch(
        INSTR,
        vec![(e(1), vec![sound(SA, true, 0.0)])],
        Some(vec![sound(SC, true, 0.0)]),
    );
    if let PortPayload::SwitchCase(c) = &mut instr.ports[0].payload {
        c.operator = SWITCH_ANY_BIT_SET;
    }
    let t = build(vec![collection(1.0, vec![instr])]);
    let ids = |t: &InstrumentTable| -> Vec<Did> {
        t.lookup(MusicScript::Normal, false, false, 3, 0)
            .map(|s| s.sound_info)
            .collect()
    };
    assert_eq!(ids(&t), vec![Did(SA)]);
    let c = t.clone();
    drop(t);
    assert_eq!(ids(&c), vec![Did(SA)]);
    agree(&c, &[0, 1, 2, 3, 7]);
}

// ---------------------------------------------------------------- hostile combination counts

/// Switches that never run (probability 0) or have no Sound below name no values, so they
/// cannot blow up the table; switches that do are bounded by `max_combinations`.
#[test]
fn combination_counts_are_bounded() {
    let mut dead_instr = switch(
        INSTR,
        (1..=4095).map(|v| (e(v), Vec::new())).collect(),
        None,
    );
    dead_instr.base.probability = 0.0;
    let mut dead_note = switch(NOTE, (0..=255).map(|v| (e(v), Vec::new())).collect(), None);
    dead_note.base.probability = 0.0;
    let silent_switch = switch(
        INSTR,
        (5000..5100).map(|v| (e(v), Vec::new())).collect(),
        None,
    );
    let root = collection(
        1.0,
        vec![dead_instr, dead_note, silent_switch, sound(SA, true, 0.0)],
    );
    let started = Instant::now();
    let t = build_full(
        &vocab(),
        vec![root.clone()],
        vec![root],
        TableLimits::default(),
    )
    .unwrap();
    assert!(t.instruments().is_empty());
    assert!(t.listed_notes().is_empty());
    assert!(t.combinations() <= 8, "{}", t.combinations());
    assert!(started.elapsed().as_secs() < 10);

    // A case port without a Sound is still a listed value when its switch has one: the
    // value changes what plays.
    let t = build(vec![switch(
        INSTR,
        vec![(e(4), Vec::new())],
        Some(vec![sound(SC, true, 0.0)]),
    )]);
    assert_eq!(t.instruments(), &[4]);
    assert_eq!(t.lookup(MusicScript::Normal, true, false, 4, 0).count(), 0);
    assert_eq!(t.lookup(MusicScript::Normal, true, false, 5, 0).count(), 1);

    // Many reachable values: refused before any combination is walked.
    let wide = collection(
        1.0,
        vec![
            switch(
                INSTR,
                (1..=600)
                    .map(|v| (e(v), vec![sound(SA, true, 0.0)]))
                    .collect(),
                None,
            ),
            switch(
                NOTE,
                (0..=255)
                    .map(|v| (e(v), vec![sound(SB, true, 0.0)]))
                    .collect(),
                None,
            ),
        ],
    );
    let started = Instant::now();
    assert!(matches!(
        build_full(&vocab(), Vec::new(), vec![wide], TableLimits::default()),
        Err(InstrumentError::Limit("instrument table combinations"))
    ));
    assert!(started.elapsed().as_secs() < 10);
    let tight = TableLimits {
        max_combinations: 1000,
        ..TableLimits::default()
    };
    let narrow = |n: u32| {
        collection(
            1.0,
            vec![
                switch(
                    INSTR,
                    (1..=n)
                        .map(|v| (e(v), vec![sound(SA, true, 0.0)]))
                        .collect(),
                    None,
                ),
                switch(
                    NOTE,
                    (0..=255)
                        .map(|v| (e(v), vec![sound(SB, true, 0.0)]))
                        .collect(),
                    None,
                ),
            ],
        )
    };
    // 8 × 41 × 257 combinations: within the default, beyond 1000.
    assert!(build_full(&vocab(), Vec::new(), vec![narrow(40)], tight).is_err());
    let t = build_full(
        &vocab(),
        Vec::new(),
        vec![narrow(40)],
        TableLimits::default(),
    )
    .unwrap();
    assert_eq!(t.instruments().len(), 40);
    assert_eq!(
        t.lookup_ids(MusicScript::Normal, false, false, 40, 255)
            .len(),
        2,
        "instrument 40, note 255 reach both"
    );
}

// ---------------------------------------------------------------- queue cost and shares

/// Many events with one time pop in time order and by sequence, without a per-pop scan of
/// the others; distinct times within the tolerance still go by sequence (§8.2).
#[test]
fn equal_times_pop_cheaply_and_in_order() {
    let mut q = NoteQueue::new();
    const N: u64 = 200_000;
    for seq in 0..N {
        q.push(MusicEvent {
            seq,
            ..off(0.05, A, 1)
        });
    }
    assert_eq!(q.count(A), N as usize);
    let started = Instant::now();
    let mut next = 0;
    while let Some(d) = q.pop_due(0.0, 1.0) {
        let tessera_audio::instruments::Due::Ready(ev) = d else {
            panic!("note-offs are never late")
        };
        assert_eq!(ev.seq, next);
        next += 1;
    }
    assert_eq!(next, N);
    assert_eq!(q.count(A), 0);
    assert!(!q.has_character(A));
    let took = started.elapsed();
    assert!(took.as_secs() < 10, "{took:?}");

    for (t, seq) in [(0.000_05, 1), (0.0, 5), (0.000_02, 3), (0.000_2, 0)] {
        q.push(MusicEvent {
            seq,
            ..off(t, A, 1)
        });
    }
    let order: Vec<u64> = std::iter::from_fn(|| q.pop_due(1.0, 1.0))
        .map(|d| match d {
            tessera_audio::instruments::Due::Ready(e)
            | tessera_audio::instruments::Due::Late(e) => e.seq,
        })
        .collect();
    assert_eq!(order, vec![1, 3, 5, 0]);
}

/// One update processes at most `max_events_per_update` events; the rest wait.
#[test]
fn one_update_processes_a_bounded_number_of_events() {
    let mut e = engine(8_000);
    let mut b = bank(8_000, 0.1);
    let mut p = player(
        Vec::new(),
        PlayerConfig {
            max_events_per_update: 5,
            ..PlayerConfig::default()
        },
    );
    for _ in 0..12 {
        p.queue_event(off(0.0, A, 1)).unwrap();
    }
    p.update(&mut e, &mut b);
    assert_eq!(p.queued(), 7);
    p.update(&mut e, &mut b);
    assert_eq!(p.queued(), 2);
    p.update(&mut e, &mut b);
    assert_eq!(p.queued(), 0);
    assert_eq!(p.stats().note_offs, 12);
}

/// One character cannot take the whole queue; a cancel's note-offs are headroom that
/// stays bounded.
#[test]
fn queue_shares_and_cancel_headroom() {
    let mut e = engine(8_000);
    let mut b = bank(8_000, 0.1);
    let mut p = player(
        Vec::new(),
        PlayerConfig {
            max_events_per_character: 3,
            max_queued_events: 4,
            ..PlayerConfig::default()
        },
    );
    for k in 1..=3 {
        p.queue_event(on(0.0, A, LUTE, pack_note(k, 90))).unwrap();
    }
    assert!(matches!(
        p.queue_event(on(0.0, A, LUTE, pack_note(4, 90))),
        Err(InstrumentError::CharacterQueueFull {
            character: A,
            queued: 3,
            adding: 1
        })
    ));
    assert!(matches!(
        p.play(A, &tune(), BELL, 5.0),
        Err(InstrumentError::CharacterQueueFull { character: A, .. })
    ));
    p.queue_event(off(50.0, B, 1)).unwrap();
    // A's three note-ons register three keys (no sound reached: the table is empty).
    p.update(&mut e, &mut b);
    assert_eq!(p.registered_keys(A), vec![1, 2, 3]);
    for _ in 0..2 {
        p.queue_event(off(60.0, B, 2)).unwrap();
    }
    p.queue_event(off(60.0, CharacterId(3), 2)).unwrap();
    assert_eq!(p.queued(), 4);
    assert!(matches!(
        p.queue_event(off(60.0, A, 9)),
        Err(InstrumentError::QueueFull { .. })
    ));
    // The cancel's note-offs are accepted beyond the limit, once per key however often
    // the stop is repeated.
    p.stop(A);
    assert_eq!(p.queued(), 7);
    p.stop(A);
    p.stop(A);
    assert_eq!(p.queued(), 7);
    p.update(&mut e, &mut b);
    assert_eq!(p.queued(), 4);
    assert!(p.registered_keys(A).is_empty());
}

// ---------------------------------------------------------------- refused and synced starts

/// A `play` that does not fit changes nothing: the running performance goes on. A
/// replacing `play` counts the events the cancel frees.
#[test]
fn a_refused_play_keeps_the_running_performance() {
    let n = tune().events.len();
    let mut p = player(
        vec![bell_tree()],
        PlayerConfig {
            max_queued_events: n + 1,
            ..PlayerConfig::default()
        },
    );
    assert_eq!(p.play(A, &tune(), BELL, 5.0).unwrap(), n);
    let long = abc::parse(
        b"X:1\nL:1/8\nK:C\nCDEFGABc|CDEFGABc|\n",
        PartRequest::Unspecified,
    )
    .unwrap();
    assert!(long.events.len() > n + 1);
    assert!(matches!(
        p.play(A, &long, BELL, 5.0),
        Err(InstrumentError::QueueFull { queued: 0, .. })
    ));
    assert!(p.is_performing(A));
    assert_eq!(p.queued(), n);
    // Replacing with one of the same size fits: the cancel frees the old events.
    assert_eq!(p.play(A, &tune(), BELL, 6.0).unwrap(), n);
    assert_eq!(p.queued(), n);
}

/// `/playstart` takes the parts in order, each with the room the earlier ones left: a part
/// that does not fit is refused alone and the start still empties every waiting slot
/// (§8.8.3 step 2, §12 P4 Decision 3; it was all or nothing before P4).
#[test]
fn a_synced_start_refuses_only_the_parts_that_do_not_fit() {
    let n = tune().events.len();
    let mut p = player(
        vec![bell_tree()],
        PlayerConfig {
            max_queued_events: n + 1,
            ..PlayerConfig::default()
        },
    );
    let mut group = SyncGroup::new();
    group.register(A, tune(), BELL).unwrap();
    group.register(B, tune(), BELL).unwrap();
    let s = group.start(&mut p, 1.0).unwrap();
    assert_eq!(s.characters, vec![A]);
    assert_eq!(s.refused.len(), 1);
    assert_eq!(
        (
            s.refused[0].character,
            s.refused[0].queued,
            s.refused[0].adding
        ),
        (B, n, n)
    );
    assert!(group.is_empty());
    assert_eq!(p.queued(), n);
    assert!(p.is_performing(A) && !p.is_performing(B));

    let mut p = player(
        vec![bell_tree()],
        PlayerConfig {
            max_queued_events: 2 * n,
            ..PlayerConfig::default()
        },
    );
    group.register(A, tune(), BELL).unwrap();
    group.register(B, tune(), BELL).unwrap();
    let s = group.start(&mut p, 1.0).unwrap();
    assert_eq!(s.characters, vec![A, B]);
    assert!(s.refused.is_empty() && group.is_empty());
    assert_eq!(p.queued(), 2 * n);
}

/// A performance without an end event (built by hand or carried by a transport) still
/// ends, so live notes work again afterwards.
#[test]
fn a_performance_without_an_end_event_ends() {
    let mut e = engine(8_000);
    let mut b = bank(8_000, 0.05);
    let mut p = player(vec![bell_tree()], PlayerConfig::default());
    let mut empty = tune();
    empty.events.clear();
    assert_eq!(p.play(A, &empty, BELL, 0.0).unwrap(), 1, "a synthetic end");
    let mut no_end = tune();
    no_end
        .events
        .retain(|e| e.kind != tessera_audio::abc::NoteEventKind::End);
    let n = no_end.events.len();
    assert_eq!(p.play(B, &no_end, BELL, 0.0).unwrap(), n + 1);
    assert!(p.is_performing(A) && p.is_performing(B));
    run(&mut p, &mut e, &mut b, 8_000 * 4, 160);
    assert!(!p.is_performing(A) && !p.is_performing(B));
    assert!(p.live_note(A, BELL, 7, true, false, 0.0).unwrap());
    assert_eq!(p.stats().voices_accepted, 4, "B's four notes played");
}

// ---------------------------------------------------------------- §10.1 shared draws

/// A probability or variance on an ancestor of several Sound nodes is drawn once per
/// note-on: the node runs or not, and has one intensity, for all of them.
#[test]
fn shared_ancestors_draw_once_per_note_on() {
    let mut root = collection(0.5, vec![sound(SA, true, 0.0), sound(SB, true, 0.0)]);
    root.base.probability = 0.5;
    root.base.intensity_variance = 0.4;
    let t = build(vec![root]);
    let sounds: Vec<_> = t
        .lookup(MusicScript::Normal, false, false, 1, 1)
        .cloned()
        .collect();
    assert_eq!(sounds.len(), 2);
    assert_ne!(sounds[0].path[0].node, sounds[0].path[1].node);
    assert_eq!(sounds[0].path[0].node, sounds[1].path[0].node);
    let mut rng = SmallRng::seed_from_u64(9);
    let (mut ran, mut levels) = (0, Vec::new());
    for _ in 0..400 {
        let mut draws = NoteDraws::new();
        let a = sounds[0].volume_shared(1.0, &mut rng, &mut draws);
        let b = sounds[1].volume_shared(1.0, &mut rng, &mut draws);
        assert_eq!(a, b);
        if let Some(v) = a {
            ran += 1;
            levels.push(v);
        }
    }
    assert!((100..300).contains(&ran), "{ran}");
    levels.dedup();
    assert!(levels.len() > 10, "the variance is drawn per note-on");

    // Through the player: voices and skipped paths come in pairs.
    let mut e = engine(8_000);
    let mut b = bank(8_000, 0.005);
    let mut p = MusicPlayer::new(Arc::new(t), PlayerConfig::default());
    p.add_character(A, LivePosition::new([0.0; 3])).unwrap();
    for i in 0..40 {
        let t0 = 0.05 * f64::from(i);
        p.queue_event(on(t0, A, 1, pack_note(1, 127))).unwrap();
        p.queue_event(off(t0 + 0.01, A, pack_note(1, 127))).unwrap();
    }
    run(&mut p, &mut e, &mut b, 8_000 * 3, 80);
    let s = p.stats();
    assert_eq!(s.note_ons, 40);
    assert_eq!(s.probability_skipped % 2, 0, "{s:?}");
    assert_eq!(s.voices_accepted % 2, 0, "{s:?}");
    assert!(s.probability_skipped > 0 && s.voices_accepted > 0, "{s:?}");
}

// ---------------------------------------------------------------- §5.2 parameter types

/// A parameter whose dictionary type is not the expected one is not set: a Switch on it
/// takes its Default port and a multiplier driven by it keeps its stored value.
#[test]
fn mistyped_parameters_are_unset() {
    let mut v = vocab();
    v.mistyped = vec![INSTR, VOL];
    let tree = driven_root(vec![switch(
        INSTR,
        vec![(e(4), vec![sound(SA, true, 0.0)])],
        Some(vec![sound(SC, true, 0.0)]),
    )]);
    let t = build_full(&v, Vec::new(), vec![tree.clone()], TableLimits::default()).unwrap();
    let got: Vec<Did> = t
        .lookup(MusicScript::Normal, false, false, 4, 0)
        .map(|s| s.sound_info)
        .collect();
    assert_eq!(got, vec![Did(SC)]);
    assert!(t.issues().contains(&TableIssue::MistypedParameter {
        script: MusicScript::Normal,
        property: INSTR
    }));
    assert!(t.issues().contains(&TableIssue::MistypedParameter {
        script: MusicScript::Normal,
        property: VOL
    }));
    assert!(t.sounds().iter().all(|s| !s.volume_driven()));
    // Typed as expected: the case matches and the volume drives the root.
    let t = build_full(&vocab(), Vec::new(), vec![tree], TableLimits::default()).unwrap();
    let s: Vec<_> = t.lookup(MusicScript::Normal, false, false, 4, 0).collect();
    assert_eq!(s[0].sound_info, Did(SA));
    assert!(s[0].volume_driven());
}

// ---------------------------------------------------------------- from_tables, §8.6

fn props(entries: Vec<(u32, PropertyValue)>) -> PropertySet {
    PropertySet {
        size: TSize {
            hint: 0,
            count: entries.len() as u32,
        },
        entries: entries
            .into_iter()
            .map(|(pid, value)| PropertyEntry { pid, value })
            .collect(),
    }
}

fn table(did: u32, scripts: Vec<(u32, Node, Option<PropertySet>)>) -> ScriptTable {
    ScriptTable {
        did: Did(did),
        kind: 1,
        includes: Vec::new(),
        scripts_size: TSize {
            hint: 0,
            count: scripts.len() as u32,
        },
        scripts: scripts
            .iter()
            .map(|(key, root, _)| Script {
                key: *key,
                roots: vec![root.clone()],
            })
            .collect(),
        properties_size: TSize::default(),
        script_properties: scripts
            .into_iter()
            .filter_map(|(key, _, p)| p.map(|p| (key, p)))
            .collect(),
        dids_size: TSize::default(),
        script_dids: Vec::new(),
    }
}

#[test]
fn scripts_come_from_the_first_defining_table() {
    let v = vocab();
    let t1 = table(
        0x0700_0001,
        vec![(
            N_KEY,
            sound(SA, true, 0.0),
            Some(props(vec![(MAX_INST, PropertyValue::Int(10))])),
        )],
    );
    let t2 = table(
        0x0700_0002,
        vec![
            (
                N_KEY,
                sound(SB, true, 0.0),
                Some(props(vec![(MAX_INST, PropertyValue::Int(3))])),
            ),
            (
                Q_KEY,
                sound(SC, true, 0.0),
                Some(props(vec![(MAX_INST, PropertyValue::Float(10.0))])),
            ),
        ],
    );
    let unrelated = table(0x0700_0003, vec![(0x1000_0001, sound(SA, true, 0.0), None)]);
    let s = MusicScripts::from_tables([&unrelated, &t1, &t2, &t1], &v).unwrap();
    assert_eq!(
        s.normal.table,
        Some(Did(0x0700_0001)),
        "the first table wins"
    );
    assert_eq!(s.normal.max_instances, 10);
    assert_eq!(s.normal.roots[0].sound().unwrap().sound, Did(SA));
    assert_eq!(s.quantized.table, Some(Did(0x0700_0002)));
    assert_eq!(s.quantized.max_instances, 1, "a float is not an int");
    assert_eq!(s.defining_tables, vec![Did(0x0700_0001), Did(0x0700_0002)]);
    assert_eq!(s.get(MusicScript::Quantized).key, Q_KEY);

    // §8.6: 1 when absent, not an int, or below 1.
    for (set, want) in [
        (None, 1),
        (Some(props(Vec::new())), 1),
        (Some(props(vec![(MAX_INST, PropertyValue::Int(0))])), 1),
        (Some(props(vec![(MAX_INST, PropertyValue::Int(-5))])), 1),
        (Some(props(vec![(MAX_INST, PropertyValue::Bool(true))])), 1),
        (Some(props(vec![(MAX_INST, PropertyValue::Int(10))])), 10),
    ] {
        let t = table(
            0x0700_0010,
            vec![
                (N_KEY, sound(SA, true, 0.0), set.clone()),
                (Q_KEY, sound(SA, true, 0.0), set),
            ],
        );
        let s = MusicScripts::from_tables([&t], &v).unwrap();
        assert_eq!(
            (s.normal.max_instances, s.quantized.max_instances),
            (want, want)
        );
    }
    // Without the property in the dictionary the limit is 1.
    let mut no_prop = v.clone();
    no_prop.max_num_instances = None;
    let s = MusicScripts::from_tables([&t1, &t2], &no_prop).unwrap();
    assert_eq!(s.normal.max_instances, 1);

    // A key no table defines.
    assert!(matches!(
        MusicScripts::from_tables([&t1], &v),
        Err(InstrumentError::ScriptNotFound { key: Q_KEY })
    ));
}

// ---------------------------------------------------------------- from_data

/// Little-endian payload builder.
#[derive(Default)]
struct Bytes(Vec<u8>);

impl Bytes {
    fn u32(mut self, v: u32) -> Self {
        self.0.extend(v.to_le_bytes());
        self
    }
    fn u16(mut self, v: u16) -> Self {
        self.0.extend(v.to_le_bytes());
        self
    }
    fn raw(mut self, v: &[u8]) -> Self {
        self.0.extend_from_slice(v);
        self
    }
    fn vle(mut self, v: u32) -> Self {
        write_vle(&mut self.0, v);
        self
    }
    fn tsize(self, count: u32) -> Self {
        self.raw(&[0]).vle(count)
    }
    fn pascal(self, s: &str) -> Self {
        self.vle(s.len() as u32).raw(s.as_bytes())
    }
}

const ENUM: u32 = 8;
const FLOAT: u32 = 9;
const INT: u32 = 15;
const BOOL: u32 = 21;

const P_SCRIPT_ID: u32 = 0x1000_0050;
const P_CHANNEL: u32 = 0x1000_0051;
const M_NOTE: u32 = 0x2300_0001;
const M_INSTR: u32 = 0x2300_0002;
const M_SCRIPT: u32 = 0x2300_0003;
const M_SCRIPT_BASE: u32 = 0x2300_0004;
const M_CHANNEL: u32 = 0x2300_0005;

/// A property dictionary (0x34000000) with `(pid, name, type, mapper)` definitions laid
/// out as `property-system.md` §2.1–§2.2 describe.
fn dictionary(props: &[(u32, &str, u32, u32)]) -> PropertyDictionary {
    let mut b = Bytes::default()
        .u32(0x3400_0000)
        .raw(&[0; 8])
        .tsize(props.len() as u32);
    for (pid, name, _, _) in props {
        b = b.u32(*pid).pascal(name);
    }
    b = b.u16(0).tsize(props.len() as u32);
    for &(pid, _, ty, data) in props {
        b = b
            .u32(pid)
            .u32(pid)
            .u32(ty)
            .u32(0)
            .u32(0)
            .u32(data)
            .u32(0)
            .u32(0)
            .u32(0x3FC0_0000)
            .raw(&[0; 5])
            .vle(0)
            .u32(0)
            .u32(0);
    }
    PropertyDictionary::parse(&b.0).unwrap()
}

fn mapper(did: u32, base: u32, names: &[(u32, &str)]) -> EnumMapper {
    let mut b = Bytes::default()
        .u32(did)
        .u32(base)
        .tsize(names.len() as u32);
    for (k, n) in names {
        b = b.u32(*k).pascal(n);
    }
    EnumMapper::parse(&b.tsize(0).0).unwrap()
}

fn music_props(volume_type: u32) -> Vec<(u32, &'static str, u32, u32)> {
    vec![
        (NOTE, "Music_Note", ENUM, M_NOTE),
        (INSTR, "Music_Instrument_Type", ENUM, M_INSTR),
        (VOL, "Music_Note_Volume", volume_type, 0),
        (SONG, "Music_Note_From_Song", BOOL, 0),
        (QUANT, "Music_Note_Quantized", BOOL, 0),
        (INTENSITY, "Script_IntensityMultiplier", FLOAT, 0),
        (MAX_INST, "Script_MaxNumInstances", INT, 0),
        (P_SCRIPT_ID, "ScriptTable_ScriptID", ENUM, M_SCRIPT),
        (P_CHANNEL, "SoundScript_Channel", ENUM, M_CHANNEL),
    ]
}

fn mappers() -> HashMap<Did, EnumMapper> {
    [
        mapper(M_NOTE, 0, &[(0, "Undef"), (11, "Note_A3"), (12, "Note_C4")]),
        mapper(M_INSTR, 0, &[(4, "Lute"), (10, "Cowbell")]),
        // The script keys sit in the base of the property's mapper.
        mapper(M_SCRIPT, M_SCRIPT_BASE, &[(0x1000_0001, "Other")]),
        mapper(
            M_SCRIPT_BASE,
            0,
            &[
                (Q_KEY, "SND_PLAYERMUSIC_PlayMusic_SUS_01"),
                (N_KEY, "SND_PLAYERMUSIC_PlayMusic_SUS_02"),
            ],
        ),
        mapper(M_CHANNEL, 0, &[(6, "PlayerMusic")]),
    ]
    .into_iter()
    .map(|m| (m.did, m))
    .collect()
}

#[test]
fn the_vocabulary_is_looked_up_by_name() {
    let dict = dictionary(&music_props(FLOAT));
    let maps = mappers();
    let v = MusicVocabulary::from_data(&dict, |d| maps.get(&d).cloned()).unwrap();
    assert_eq!(
        (
            v.music_note,
            v.music_instrument_type,
            v.music_note_volume,
            v.music_note_from_song,
            v.music_note_quantized,
            v.intensity_multiplier,
            v.max_num_instances,
        ),
        (NOTE, INSTR, VOL, SONG, QUANT, INTENSITY, Some(MAX_INST))
    );
    assert_eq!((v.quantized_script, v.normal_script), (Q_KEY, N_KEY));
    assert_eq!(v.pitch_key(69), 11, "Note_A3 by the label rule of §7.1");
    assert_eq!(v.pitch_key(72), 12);
    assert_eq!(v.pitch_key(70), 0, "no label: Undef");
    assert_eq!(v.instrument_key("Cowbell"), Some(10));
    assert_eq!(v.instrument_label(4), Some("Lute"));
    assert_eq!(v.note_label(11), Some("Note_A3"));
    assert_eq!(v.channel_label(6), Some("PlayerMusic"));
    assert!(v.mistyped.is_empty());

    // A parameter of another type is listed, not an error (§5.2).
    let dict = dictionary(&music_props(INT));
    let v = MusicVocabulary::from_data(&dict, |d| maps.get(&d).cloned()).unwrap();
    assert_eq!(v.mistyped, vec![VOL]);

    // A missing name is an error naming it.
    let mut partial = music_props(FLOAT);
    partial.retain(|p| p.1 != "Music_Note_Quantized");
    let dict = dictionary(&partial);
    assert!(matches!(
        MusicVocabulary::from_data(&dict, |d| maps.get(&d).cloned()),
        Err(InstrumentError::UnknownName {
            name: "Music_Note_Quantized",
            ..
        })
    ));
    // A script-key label missing from the mapper chain.
    let dict = dictionary(&music_props(FLOAT));
    let mut broken = mappers();
    broken.remove(&Did(M_SCRIPT_BASE));
    assert!(matches!(
        MusicVocabulary::from_data(&dict, |d| broken.get(&d).cloned()),
        Err(InstrumentError::UnknownName {
            what: "script-key label",
            ..
        })
    ));
}
