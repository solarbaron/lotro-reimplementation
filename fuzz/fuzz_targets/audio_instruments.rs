// SPDX-License-Identifier: GPL-3.0-or-later
//! Player-music instruments (`docs/specs/instruments.md` §5–§10): the instrument-table
//! builder fed with arbitrary script tables, and the player driven by arbitrary events.
//!
//! Mode 0 decodes the input as a script table (0x07) with a schema that types the music
//! PIDs and every other PID by its value, takes the two music scripts from it and builds
//! the table. Mode 1 builds a script tree from the input (node kinds, switch properties,
//! operators, hostile multipliers, variances, probabilities, driven fields, deep nesting,
//! sound infos that are missing, undecodable or looping), builds the table, then plays
//! arbitrary note events, performances, live notes, cancels, character removals and synced
//! starts through the offline engine.
//!
//! Nothing may panic, the table must agree with a live walk on sampled combinations, every
//! volume is in [0, 1], and every rendered sample is finite.

#![no_main]

use std::collections::HashMap;
use std::sync::Arc;

use libfuzzer_sys::fuzz_target;
use tessera_audio::abc::{self, PartRequest};
use tessera_audio::engine::{AudioEngine, EngineConfig, LivePosition};
use tessera_audio::instruments::{
    CharacterId, InstrumentTable, MusicEvent, MusicEventKind, MusicPlayer, MusicScript,
    MusicScripts, MusicVocabulary, PlayerConfig, SampleBank, ScriptDef, SyncGroup, TableLimits,
};
use tessera_audio::pcm::Pcm;
use tessera_formats::script_table::{
    BaseBlock, Node, NodeData, NodeKind, Port, PortPayload, PortType, SoundNode, SwitchCase,
    SwitchNode,
};
use tessera_formats::{Did, PropertyEntry, PropertyType, PropertyValue, ScriptTable};

const NOTE: u32 = 0x1159;
const INSTR: u32 = 0x1230;
const VOL: u32 = 0x1DCF;
const SONG: u32 = 0x1709;
const QUANT: u32 = 0x170A;
const INTENSITY: u32 = 0x3A3;
const MAX_INST: u32 = 0x3F2;
const Q_KEY: u32 = 0x1000_0BCD;
const N_KEY: u32 = 0x1000_0BCE;
const PROPS: [u32; 6] = [NOTE, INSTR, VOL, SONG, QUANT, 0x77];

struct Bytes<'a>(&'a [u8]);

impl Bytes<'_> {
    fn u8(&mut self) -> u8 {
        match self.0.split_first() {
            Some((b, rest)) => {
                self.0 = rest;
                *b
            }
            None => 0,
        }
    }
    fn u32(&mut self) -> u32 {
        u32::from_le_bytes([self.u8(), self.u8(), self.u8(), self.u8()])
    }
    fn f32(&mut self) -> f32 {
        let tag = self.u8();
        if tag & 0x80 != 0 {
            f32::from_bits(self.u32())
        } else {
            f32::from(tag) / 32.0
        }
    }
    fn bool(&mut self) -> bool {
        self.u8() & 1 != 0
    }
    fn empty(&self) -> bool {
        self.0.is_empty()
    }
}

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
        .map(|p| (u32::from(p - 47), abc::note_label(p).unwrap_or_default()))
        .collect();
    v.derive_pitch_keys();
    v
}

fn schema(pid: u32) -> Option<PropertyType> {
    Some(match pid {
        NOTE | INSTR => PropertyType::Enum,
        VOL | INTENSITY => PropertyType::Float,
        SONG | QUANT => PropertyType::Bool,
        MAX_INST => PropertyType::Int,
        p => PropertyType::from_code(p % 24),
    })
}

fn base(b: &mut Bytes<'_>) -> BaseBlock {
    let mut driven = Vec::new();
    if b.u8().is_multiple_of(5) {
        driven.push((
            [INTENSITY, 0x8C, 0x4EA][usize::from(b.u8() % 3)],
            [VOL, NOTE, 0x99][usize::from(b.u8() % 3)],
        ));
    }
    let tag = b.u8();
    BaseBlock {
        driven,
        use_parent_intensity: tag & 1 == 0,
        intensity_multiplier: if tag & 2 == 0 { 1.0 } else { b.f32() },
        intensity_variance: if tag & 4 == 0 { 0.0 } else { b.f32() },
        probability: if tag & 8 == 0 { 1.0 } else { b.f32() },
        node_id: u32::from(tag),
        ..BaseBlock::default()
    }
}

fn value(b: &mut Bytes<'_>, prop: u32) -> PropertyValue {
    match b.u8() % 4 {
        0 => PropertyValue::Bool(b.bool()),
        1 => PropertyValue::Float(b.f32()),
        2 => PropertyValue::Int(i32::from(b.u8()) - 3),
        _ => {
            let v = u32::from(b.u8() % 40);
            if prop == INSTR || prop == NOTE {
                PropertyValue::Enum(v)
            } else {
                PropertyValue::Enum(v % 2)
            }
        }
    }
}

fn tree(b: &mut Bytes<'_>, depth: usize, budget: &mut usize) -> Node {
    *budget = budget.saturating_sub(1);
    let kind = if *budget == 0 || b.empty() {
        3
    } else {
        b.u8() % 6
    };
    let mk = |kind, data, base, ports| Node {
        kind,
        data,
        base,
        particle_flag: None,
        ports,
        targets: Vec::new(),
    };
    match kind {
        0 | 5 if depth < 300 => {
            let n = usize::from(b.u8() % 4);
            let children = (0..n).map(|_| tree(b, depth + 1, budget)).collect();
            mk(
                NodeKind::COLLECTION,
                NodeData::None,
                base(b),
                vec![Port {
                    port_type: PortType::DEFAULT,
                    payload: PortPayload::None,
                    children,
                }],
            )
        }
        1 if depth < 300 => {
            let property = PROPS[usize::from(b.u8()) % PROPS.len()];
            let cases = usize::from(b.u8() % 5);
            let mut ports = Vec::new();
            for _ in 0..cases {
                let nvals = usize::from(b.u8() % 3);
                let values = (0..nvals)
                    .map(|_| {
                        if b.u8().is_multiple_of(7) {
                            None
                        } else {
                            Some(PropertyEntry {
                                pid: property,
                                value: value(b, property),
                            })
                        }
                    })
                    .collect();
                let operator = [3, 3, 3, 1, 2, 0, 9][usize::from(b.u8() % 7)];
                let n = usize::from(b.u8() % 3);
                ports.push(Port {
                    port_type: PortType::SWITCH_VALUE,
                    payload: PortPayload::SwitchCase(Box::new(SwitchCase {
                        values,
                        operator,
                        ..SwitchCase::default()
                    })),
                    children: (0..n).map(|_| tree(b, depth + 1, budget)).collect(),
                });
            }
            if b.bool() {
                ports.push(Port {
                    port_type: PortType::DEFAULT,
                    payload: PortPayload::None,
                    children: vec![tree(b, depth + 1, budget)],
                });
            }
            mk(
                NodeKind::SWITCH,
                NodeData::Switch(SwitchNode {
                    property,
                    persistent: false,
                    autonomous: false,
                    synchronized: false,
                }),
                base(b),
                ports,
            )
        }
        2 => mk(NodeKind::PARTICLE, NodeData::None, base(b), Vec::new()),
        4 => mk(
            NodeKind::TIMER,
            NodeData::Opaque { offset: 0, len: 0 },
            base(b),
            vec![Port {
                port_type: PortType::DEFAULT,
                payload: PortPayload::None,
                children: vec![tree(b, depth + 1, budget)],
            }],
        ),
        _ => {
            let s = SoundNode {
                sound: Did(0x2A00_0000 | u32::from(b.u8() % 6)),
                fire_and_forget: b.bool(),
                move_with_entity: b.bool(),
                use_camera_position: b.u8().is_multiple_of(5),
                min_random_distance: if b.bool() { 0.0 } else { b.f32() },
                max_random_distance: if b.bool() { 0.0 } else { b.f32() },
                offset: [0.0, if b.bool() { 0.0 } else { b.f32() }, 0.0],
                channel: u32::from(b.u8() % 12),
                ramp_out_time: b.f32(),
                pitch: if b.bool() { 0.0 } else { b.f32() },
                pitch_variance: if b.bool() { 0.0 } else { b.f32() },
            };
            mk(
                NodeKind::SOUND,
                NodeData::Sound(Box::new(s)),
                base(b),
                Vec::new(),
            )
        }
    }
}

fn sound_info(did: u32, flags: u32, min: f32, max: f32) -> Vec<u8> {
    let mut v = Vec::new();
    v.extend(did.to_le_bytes());
    v.extend(flags.to_le_bytes());
    v.extend((-1.0f32).to_le_bytes());
    v.extend((-1.0f32).to_le_bytes());
    v.extend(0.5f32.to_le_bytes());
    v.extend(min.to_le_bytes());
    v.extend(max.to_le_bytes());
    v.extend((0x0A00_0000 | (did & 0xF)).to_le_bytes());
    v.push(1);
    v.push(b'x');
    v
}

fn infos(b: &mut Bytes<'_>) -> HashMap<u32, Vec<u8>> {
    let mut m = HashMap::new();
    for i in 0..6u32 {
        let did = 0x2A00_0000 | i;
        match b.u8() % 5 {
            0 => {}
            1 => {
                m.insert(did, vec![1, 2, 3]);
            }
            _ => {
                let flags = [0, 1, 4, 5, 0x40, 0x41][usize::from(b.u8() % 6)];
                m.insert(did, sound_info(did, flags, b.f32() * 4.0, b.f32() * 40.0));
            }
        }
    }
    m
}

fn check_table(t: &InstrumentTable, b: &mut Bytes<'_>) {
    for _ in 0..8 {
        let script = if b.bool() {
            MusicScript::Quantized
        } else {
            MusicScript::Normal
        };
        let (song, q) = (b.bool(), b.bool());
        let instr = u32::from(b.u8() % 45);
        let key = b.u8();
        let ids = t.lookup_ids(script, song, q, instr, key).to_vec();
        if let Ok(w) = t.walk(script, song, q, instr, key) {
            assert_eq!(ids, w);
        }
    }
    for s in t.sounds() {
        assert!((0.0..=1.0).contains(&s.intensity()));
    }
}

fn play(t: InstrumentTable, b: &mut Bytes<'_>) {
    const RATE: u32 = 8_000;
    let Ok(mut engine) = AudioEngine::new(EngineConfig::offline(RATE)) else {
        return;
    };
    let mut bank = SampleBank::new();
    for i in 0..6u32 {
        if !b.u8().is_multiple_of(4) {
            let frames = usize::from(b.u8()) * 8;
            bank.insert(
                Did(0x0A00_0000 | i),
                Pcm {
                    sample_rate: RATE,
                    channels: 1,
                    samples: vec![0.25f32; frames].into(),
                },
            );
        }
    }
    let mut player = MusicPlayer::new(
        Arc::new(t),
        PlayerConfig {
            max_queued_events: 512,
            max_characters: 4,
            ..PlayerConfig::default()
        },
    );
    let mut group = SyncGroup::new();
    let perf = abc::parse(
        b"X:1\nL:1/16\nQ:1/4=240\nK:C\nCD[EG]c+ff+C,c'|\n",
        PartRequest::Unspecified,
    )
    .ok();
    let mut out = vec![0.0f32; 160 * 2];
    for _ in 0..64 {
        if b.empty() {
            break;
        }
        let who = CharacterId(u64::from(b.u8() % 5));
        let now = MusicPlayer::now(&engine);
        match b.u8() % 10 {
            0 => {
                let _ = player.add_character(who, LivePosition::new([b.f32(), b.f32(), 0.0]));
            }
            1 => player.remove_character(&mut engine, who),
            2 => player.stop(who),
            3 => {
                if let Some(p) = &perf {
                    let _ = player.play(who, p, u32::from(b.u8() % 45), now + f64::from(b.f32()));
                }
            }
            4 => {
                let _ = player.live_note(
                    who,
                    u32::from(b.u8() % 45),
                    u32::from(b.u8()),
                    b.bool(),
                    b.bool(),
                    now,
                );
            }
            5 => {
                if let Some(p) = &perf {
                    let _ = group.register(who, p.clone(), u32::from(b.u8() % 45));
                }
            }
            6 => {
                let _ = group.start(&mut player, SyncGroup::start_time(now));
            }
            _ => {
                let kind = match b.u8() % 3 {
                    0 => MusicEventKind::NoteOn,
                    1 => MusicEventKind::NoteOff,
                    _ => MusicEventKind::End,
                };
                let dt = f64::from(b.f32()) - 0.6;
                let _ = player.queue_event(MusicEvent {
                    time: now + dt,
                    seq: 0,
                    kind,
                    character: who,
                    instrument: u32::from(b.u8() % 45),
                    packed: b.u32(),
                    quantized: b.bool(),
                    from_song: b.bool(),
                });
            }
        }
        player.update(&mut engine, &mut bank);
        engine.update();
        if engine.render(&mut out).is_ok() {
            assert!(out.iter().all(|s| s.is_finite()));
        }
        engine.drain_events().for_each(drop);
    }
}

fuzz_target!(|data: &[u8]| {
    let Some((&mode, rest)) = data.split_first() else {
        return;
    };
    let v = vocab();
    let limits = TableLimits {
        max_work: 200_000,
        max_instruments: 64,
        max_sounds: 512,
        ..TableLimits::default()
    };
    if mode % 2 == 0 {
        let Ok(t) = ScriptTable::parse(rest, &schema) else {
            return;
        };
        let Ok(scripts) = MusicScripts::from_tables([&t], &v) else {
            return;
        };
        if let Ok(table) = InstrumentTable::build(scripts, &v, limits, |_d: Did| {
            Ok::<Option<Vec<u8>>, String>(None)
        }) {
            let mut b = Bytes(rest);
            check_table(&table, &mut b);
        }
        return;
    }
    let mut b = Bytes(rest);
    let mut budget = 400;
    let roots = |b: &mut Bytes<'_>, budget: &mut usize| -> Vec<Node> {
        let n = usize::from(b.u8() % 3);
        (0..n).map(|_| tree(b, 0, budget)).collect()
    };
    let q = roots(&mut b, &mut budget);
    let n = roots(&mut b, &mut budget);
    let max_instances = u32::from(b.u8() % 4);
    let scripts = MusicScripts {
        quantized: ScriptDef {
            key: Q_KEY,
            table: None,
            roots: q,
            max_instances,
        },
        normal: ScriptDef {
            key: N_KEY,
            table: None,
            roots: n,
            max_instances,
        },
        defining_tables: Vec::new(),
    };
    let info_map = infos(&mut b);
    let Ok(table) = InstrumentTable::build(scripts, &v, limits, |d: Did| {
        Ok::<_, String>(info_map.get(&d.0).cloned())
    }) else {
        return;
    };
    check_table(&table, &mut b);
    play(table, &mut b);
});
