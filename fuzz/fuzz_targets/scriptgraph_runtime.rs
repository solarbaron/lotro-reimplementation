// SPDX-License-Identifier: GPL-3.0-or-later
//! The script-graph runtime (`docs/specs/script-runtime.md`) driven by arbitrary graphs and
//! operations. The first byte picks how the graph is made: either the input's next parts
//! decode as a script table (0x07) and a scriptlet (0x0C) with a total synthetic schema, or
//! the bytes are read as a recipe of nodes (every control-flow kind, sounds, environments,
//! particle nodes, with random ports, payloads, probabilities, variances and driven pairs). Every table DID
//! names that table and every scriptlet DID that scriptlet, so includes, Run nodes and
//! Scriptlet nodes reach them recursively. The remaining bytes are operations (property
//! changes, advances, voice completions, script starts and stops, respawns).
//!
//! Nothing may panic or overflow the stack, the per-entity instance bound holds, and once
//! the entities are removed no instance, run, tracked voice or never-ending attached
//! particle effect is left.

#![no_main]

use std::sync::Arc;

use libfuzzer_sys::fuzz_target;
use tessera_formats::property::PropertyEntry;
use tessera_formats::script_table::{
    BaseBlock, Node, NodeData, NodeKind, ParticleScriptNode, Port, PortPayload, PortType, RunNode,
    SoundNode, SwitchCase, SwitchNode, SwitchTransition, TimerNode, WhenTest,
};
use tessera_formats::{
    Did, PropertySet, PropertyType, PropertyValue, Script, ScriptTable, Scriptlet,
    decode_script_table, decode_scriptlet,
};
use tessera_scriptgraph::{
    EntityId, EntityInit, RecordingSink, Runtime, RuntimeLimits, RuntimeVocabulary, ScriptSource,
    SyntheticCatalog,
};

/// Every PID has a type: codes 0 and 23 map to `Unknown`, the rest to types 1-22.
fn total(pid: u32) -> Option<PropertyType> {
    Some(PropertyType::from_code(pid % 24))
}

struct Fixed {
    table: Option<Arc<ScriptTable>>,
    scriptlet: Option<Arc<Scriptlet>>,
}

impl ScriptSource for Fixed {
    fn table(&mut self, did: Did) -> Option<Arc<ScriptTable>> {
        (did.prefix() == 0x07).then(|| self.table.clone()).flatten()
    }
    fn scriptlet(&mut self, did: Did) -> Option<Arc<Scriptlet>> {
        (did.prefix() == 0x0C)
            .then(|| self.scriptlet.clone())
            .flatten()
    }
}

const PER_ENTITY: usize = 4096;

/// Reads a node recipe from bytes.
struct Recipe<'a> {
    bytes: std::slice::Iter<'a, u8>,
    nodes: u32,
}

impl Recipe<'_> {
    fn byte(&mut self) -> u8 {
        self.bytes.next().copied().unwrap_or(0)
    }

    fn f32(&mut self) -> f32 {
        f32::from(self.byte()) / 32.0
    }

    fn children(&mut self, depth: u32) -> Vec<Node> {
        let n = self.byte() % 4;
        (0..n).filter_map(|_| self.node(depth + 1)).collect()
    }

    fn port(&mut self, ty: PortType, payload: PortPayload, depth: u32) -> Port {
        Port {
            port_type: ty,
            payload,
            children: self.children(depth),
        }
    }

    fn value(&mut self) -> PropertyValue {
        PropertyValue::Int(i32::from(self.byte() % 4))
    }

    fn node(&mut self, depth: u32) -> Option<Node> {
        if depth > 12 || self.nodes > 400 || self.bytes.len() == 0 {
            return None;
        }
        self.nodes += 1;
        let k = self.byte();
        let pid = u32::from(self.byte() % 8);
        let mut ports = Vec::new();
        let mut particle_flag = None;
        let (kind, data) = match k % 15 {
            0 => {
                for i in 0..(self.byte() % 3) {
                    ports.push(self.port(PortType::DEFAULT, PortPayload::U32(u32::from(i)), depth));
                }
                (NodeKind::COLLECTION, NodeData::None)
            }
            1 => {
                for i in 0..(self.byte() % 4) {
                    ports.push(self.port(PortType::DEFAULT, PortPayload::U32(u32::from(i)), depth));
                }
                let f = self.byte();
                (
                    NodeKind::SEQUENCE,
                    NodeData::Sequence {
                        loop_forever: f & 1 == 1,
                        loop_count: i32::from(f >> 1) - 2,
                    },
                )
            }
            2 => {
                for _ in 0..(self.byte() % 4) {
                    let w = u32::from(self.byte() % 3);
                    ports.push(self.port(PortType::DEFAULT, PortPayload::U32(w), depth));
                }
                let f = self.byte();
                (
                    NodeKind::RANDOM,
                    NodeData::Random {
                        no_repeats: f & 1 == 1,
                        autonomous: false,
                    },
                )
            }
            3 => {
                for _ in 0..(self.byte() % 3) {
                    let t = self.f32();
                    ports.push(self.port(PortType::DEFAULT, PortPayload::F32(t), depth));
                }
                let f = self.byte();
                (
                    NodeKind::TIMER,
                    NodeData::Timer(TimerNode {
                        loop_forever: f & 1 == 1,
                        duration: self.f32(),
                        loop_count: i32::from(f >> 1) - 2,
                        autonomous: false,
                        speed: if f & 2 == 0 { 1.0 } else { self.f32() },
                        speed_multiplier: 1.0,
                        tools_animation_override: Did(0),
                    }),
                )
            }
            4 | 5 => {
                let when = k % 15 == 5;
                for _ in 0..(self.byte() % 3) {
                    let v = self.value();
                    let payload = if when {
                        PortPayload::WhenTest(Box::new(WhenTest {
                            constant: Some(PropertyEntry { pid, value: v }),
                            rhs_property: i32::from(self.byte() % 3),
                            operator: i32::from(self.byte() % 7),
                            tail: [0, 0],
                        }))
                    } else {
                        PortPayload::SwitchCase(Box::new(SwitchCase {
                            values: vec![Some(PropertyEntry { pid, value: v })],
                            ints: if self.byte() & 1 == 1 {
                                vec![2]
                            } else {
                                Vec::new()
                            },
                            operator: i32::from(self.byte() % 4),
                            transition_time: [0.0, 0.3, 0.0, 0.5][usize::from(self.byte() % 4)],
                            second_time: [0.0, 0.0, 0.2][usize::from(self.byte() % 3)],
                        }))
                    };
                    let ty = if when {
                        PortType::WHEN_TEST
                    } else {
                        PortType::SWITCH_VALUE
                    };
                    ports.push(self.port(ty, payload, depth));
                }
                match self.byte() % 4 {
                    0 => ports.push(self.port(
                        PortType::DEFAULT,
                        PortPayload::F32Pair(0.0, 0.0),
                        depth,
                    )),
                    1 => {
                        ports.push(self.port(PortType::FOR_CURRENT_VALUE, PortPayload::None, depth))
                    }
                    2 if !when => {
                        let f = self.byte();
                        let from = (f & 1 == 0).then(|| PropertyEntry {
                            pid,
                            value: PropertyValue::Int(i32::from(f >> 4 & 3)),
                        });
                        let to = (f & 2 == 0).then(|| PropertyEntry {
                            pid,
                            value: PropertyValue::Int(i32::from(f >> 6)),
                        });
                        let ramp_in = self.f32() / 4.0;
                        let payload = PortPayload::SwitchTransition(Box::new(SwitchTransition {
                            flags: [from.is_none(), to.is_none(), f & 4 != 0],
                            from,
                            to,
                            floats: [0.0, ramp_in, -1.0, if f & 8 != 0 { 0.1 } else { 0.0 }],
                        }));
                        let port = self.port(PortType::SWITCH_TRANSITION, payload, depth);
                        ports.push(port);
                    }
                    _ => {}
                }
                let kind = if when {
                    NodeKind::WHEN
                } else {
                    NodeKind::SWITCH
                };
                (
                    kind,
                    NodeData::Switch(SwitchNode {
                        property: pid,
                        persistent: self.byte() & 1 == 1,
                        autonomous: false,
                        synchronized: false,
                    }),
                )
            }
            6 => {
                let f = self.byte();
                (
                    NodeKind::SOUND,
                    NodeData::Sound(Box::new(SoundNode {
                        sound: Did(if f & 1 == 0 {
                            0x2A00_0000 | u32::from(f)
                        } else {
                            0
                        }),
                        fire_and_forget: f & 2 != 0,
                        min_random_distance: f32::from(f & 4),
                        max_random_distance: f32::from(f & 12),
                        pitch: self.f32() - 2.0,
                        pitch_variance: self.f32() - 1.0,
                        ramp_out_time: self.f32(),
                        ..SoundNode::default()
                    })),
                )
            }
            7 => (
                NodeKind::SET_PROPERTY,
                NodeData::SetProperty {
                    property: pid,
                    value: Some(Box::new(PropertyEntry {
                        pid,
                        value: self.value(),
                    })),
                    server_only: self.byte().is_multiple_of(5),
                },
            ),
            8 => {
                ports.push(self.port(PortType::DEFAULT, PortPayload::U32(1), depth));
                (
                    NodeKind::LABEL,
                    NodeData::Label {
                        name: format!("{}", pid % 2),
                    },
                )
            }
            9 => (
                NodeKind::GOTO,
                NodeData::Goto {
                    name: format!("{}", pid % 2),
                },
            ),
            10 => {
                let f = self.byte();
                (
                    NodeKind::RUN,
                    NodeData::Run(Box::new(RunNode {
                        version: 1,
                        parameters: Vec::new(),
                        script: 0x1000_0000 | u32::from(f % 4),
                        persistent: f & 4 != 0,
                        unbind_on_death: f & 8 != 0,
                        stop: f & 16 != 0,
                        expose_as_script_link: false,
                        queuing_type: 3,
                    })),
                )
            }
            11 => {
                ports.push(self.port(PortType::RUNNING, PortPayload::None, depth));
                ports.push(self.port(PortType::NOT_RUNNING, PortPayload::None, depth));
                (
                    NodeKind::WHEN_RUNNING,
                    NodeData::WhenRunning {
                        scripts: vec![0x1000_0000 | u32::from(self.byte() % 4)],
                        names: Vec::new(),
                        persistent: self.byte() & 1 == 1,
                    },
                )
            }
            12 => {
                ports.push(self.port(PortType::DEFAULT, PortPayload::None, depth));
                (NodeKind::ON_STOP, NodeData::None)
            }
            13 => {
                for _ in 0..(self.byte() % 3) {
                    let name = if self.byte() & 1 == 0 { "a" } else { "A" };
                    ports.push(self.port(PortType::DEFAULT, PortPayload::Name(name.into()), depth));
                }
                (
                    NodeKind::SCRIPTLET,
                    NodeData::Scriptlet {
                        file: Did(if self.byte() & 3 == 0 { 0 } else { 0x0C00_0001 }),
                        parameters: PropertySet {
                            size: Default::default(),
                            entries: vec![PropertyEntry {
                                pid: 6,
                                value: self.value(),
                            }],
                        },
                    },
                )
            }
            _ => match self.byte() % 4 {
                3 => {
                    let f = self.byte();
                    particle_flag = (f & 1 == 0).then_some(f & 2 != 0);
                    (
                        NodeKind::PARTICLE,
                        NodeData::Particle(Box::new(ParticleScriptNode {
                            particle_id: Did(if f & 4 == 0 { 0x3900_0001 } else { 0 }),
                            drop_effect: f & 8 != 0,
                            immediate_disposal: f & 16 != 0,
                            redirect_property: if f & 32 == 0 { 0 } else { pid },
                            ..ParticleScriptNode::default()
                        })),
                    )
                }
                0 => (
                    NodeKind::SCRIPTLET_PARAMETER,
                    NodeData::ScriptletParameter { name: "a".into() },
                ),
                1 => (
                    NodeKind::SOUND_ENVIRONMENT,
                    NodeData::SoundEnvironment {
                        name: pid,
                        priority: 0,
                    },
                ),
                _ => (NodeKind::DECAL, NodeData::Opaque { offset: 0, len: 0 }),
            },
        };
        let f = self.byte();
        let mut base = BaseBlock {
            use_parent_intensity: f & 1 == 0,
            intensity_multiplier: if f & 2 == 0 { 1.0 } else { self.f32() },
            intensity_variance: if f & 4 == 0 { 0.0 } else { self.f32() / 8.0 },
            probability: if f & 8 == 0 { 1.0 } else { self.f32() / 8.0 },
            node_id: self.nodes,
            ..BaseBlock::default()
        };
        if f & 16 != 0 {
            base.driven.push((3, 7));
        }
        if f & 32 != 0 {
            base.scriptlet_driven.push((3, 6));
        }
        Some(Node {
            kind,
            data,
            base,
            particle_flag,
            ports,
            targets: Vec::new(),
        })
    }
}

/// A table and a scriptlet built from a recipe.
fn from_recipe(bytes: &[u8]) -> (ScriptTable, Scriptlet, usize) {
    let mut r = Recipe {
        bytes: bytes.iter(),
        nodes: 0,
    };
    let mut scripts = Vec::new();
    for key in 0..4u32 {
        let roots = r.children(0);
        scripts.push(Script {
            key: 0x1000_0000 | key,
            roots,
        });
    }
    let scriptlet = Scriptlet {
        did: Did(0),
        roots: r.children(0),
        dependencies: Vec::new(),
    };
    let used = bytes.len() - r.bytes.len();
    let table = ScriptTable {
        kind: 2,
        includes: vec![Did(0x0700_0001)],
        scripts,
        ..ScriptTable::default()
    };
    (table, scriptlet, used)
}

fuzz_target!(|data: &[u8]| {
    if data.len() < 4 {
        return;
    }
    let (table, scriptlet, ops) = if data[0] & 1 == 0 {
        let (t, s, used) = from_recipe(&data[1..]);
        (t, Some(Arc::new(s)), &data[1 + used..])
    } else {
        let n = data.len() - 4;
        let a = usize::from(u16::from_le_bytes([data[0], data[1]])) % (n + 1);
        let b = usize::from(u16::from_le_bytes([data[2], data[3]])) % (n - a + 1);
        let body = &data[4..];
        let (table_bytes, rest) = body.split_at(a);
        let (scriptlet_bytes, ops) = rest.split_at(b);
        let Ok(table) = decode_script_table(table_bytes, &total) else {
            return;
        };
        (
            table,
            decode_scriptlet(scriptlet_bytes, &total).ok().map(Arc::new),
            ops,
        )
    };
    let keys: Vec<u32> = table.scripts.iter().map(|s| s.key).take(8).collect();
    if keys.is_empty() {
        return;
    }
    let source = Fixed {
        table: Some(Arc::new(table)),
        scriptlet,
    };
    let mut vocab = RuntimeVocabulary {
        startup_key: keys.first().copied(),
        intensity_multiplier: Some(3),
        max_num_instances: Some(4),
        combination_type: Some(5),
        combination_overriding: Some(1),
        ..RuntimeVocabulary::default()
    };
    vocab.sound.sound_id = Some(8);
    let mut catalog = SyntheticCatalog::default();
    catalog.script_parameters.insert(6);
    catalog.defaults.insert(7, PropertyValue::Float(1.0));
    // With bit 0 of the first operation byte every PID the recipes use has a definition
    // (every type, `Unknown` included), so unset Switch and When properties start from
    // their types' zero values (`script-runtime.md` §6.2); without it they have no value.
    if ops.first().is_some_and(|b| b & 1 == 1) {
        for pid in 0..16 {
            if let Some(ty) = total(pid) {
                catalog.types.insert(pid, ty);
            }
        }
    }
    let mut rt = Runtime::new(source, catalog, vocab)
        .with_seed(u64::from(ops.len() as u32))
        .with_limits(RuntimeLimits {
            max_instances_per_entity: PER_ENTITY,
            max_starts_per_call: 20_000,
            max_events_per_advance: 20_000,
            ..RuntimeLimits::default()
        });
    let mut sink = RecordingSink::new();
    sink.max_events = 64;
    let e = EntityId(1);
    let player = EntityId(2);
    let init = || EntityInit {
        tables: vec![Did(0x0700_0001)],
        run_startup: true,
        ..EntityInit::default()
    };
    let _ = rt.spawn_entity(&mut sink, player, init());
    rt.set_player(Some(player));
    // The entity sometimes has its own Random generator (`script-runtime.md` §8 OPEN-18).
    let seed = ops.first().copied().filter(|b| b & 1 == 1).map(u64::from);
    let _ = rt.spawn_entity(
        &mut sink,
        e,
        EntityInit {
            random_seed: seed,
            ..init()
        },
    );
    let mut it = ops.iter().copied();
    while let Some(op) = it.next() {
        let x = it.next().unwrap_or(0);
        match op % 7 {
            0 | 1 => {
                let pid = u32::from(x % 16);
                let v = it.next().unwrap_or(0);
                let value = match v % 3 {
                    0 => PropertyValue::Int(i32::from(v >> 2)),
                    1 => PropertyValue::Float(f32::from(v) / 64.0),
                    _ => PropertyValue::Bool(v & 4 != 0),
                };
                let _ = rt.set_property(&mut sink, e, pid, value);
            }
            2 => rt.advance(&mut sink, f64::from(x) / 16.0),
            3 => {
                let h = sink.playing.keys().nth(usize::from(x % 4)).copied();
                if let Some(h) = h {
                    sink.complete(h);
                    rt.sound_completed(&mut sink, h);
                }
            }
            4 => {
                let _ = rt.start_script(&mut sink, e, keys[usize::from(x) % keys.len()]);
            }
            5 => {
                let _ = rt.stop_script(&mut sink, e, keys[usize::from(x) % keys.len()]);
            }
            _ => {
                let _ = rt.despawn(&mut sink, e);
                // The entity sometimes has its own Random generator (`script-runtime.md` §8 OPEN-18).
                let seed = ops.first().copied().filter(|b| b & 1 == 1).map(u64::from);
                let _ = rt.spawn_entity(
                    &mut sink,
                    e,
                    EntityInit {
                        random_seed: seed,
                        ..init()
                    },
                );
            }
        }
        if let Some(n) = rt.entity_instances(e) {
            assert!(n <= PER_ENTITY);
        }
    }
    let _ = rt.despawn(&mut sink, e);
    let _ = rt.despawn(&mut sink, player);
    let s = rt.stats();
    assert_eq!(s.instances, 0);
    assert_eq!(s.runs, 0);
    assert_eq!(s.tracked_sounds, 0);
    assert!(sink.playing.is_empty());
    assert!(sink.environments.is_empty());
    // Only finite and dropped effects outlive their scripts (`particle-runtime.md` §5.5,
    // §5.6, §7.1).
    assert!(
        sink.particles
            .values()
            .all(|p| !p.never_ending || p.node.drop_effect)
    );
});
