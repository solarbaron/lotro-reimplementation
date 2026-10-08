// SPDX-License-Identifier: GPL-3.0-or-later
//! Property tests: random graphs and random operations never panic, stay within the
//! limits, and leave nothing behind when the entity is removed (RI2: a stopped instance
//! has no running child).

mod common;

use common::*;
use proptest::prelude::*;
use tessera_formats::script_table::{Node, PortPayload, PortType};
use tessera_formats::{Did, PropertyValue};
use tessera_scriptgraph::{
    EntityId, MemorySource, RecordingSink, Runtime, RuntimeLimits, SoundHandle,
};

const KEYS: [u32; 3] = [STARTUP, 0x1000_0001, 0x1000_0002];
const PIDS: [u32; 3] = [pid::VALUE, pid::OTHER, pid::MUSIC_VOLUME];
const NAMES: [&str; 2] = ["a", "B"];
const SCRIPTLETS: [u32; 2] = [0x0C00_0001, 0x0C00_0002];

fn leaf() -> impl Strategy<Value = Node> {
    prop_oneof![
        (0u32..4, any::<bool>(), 0.0f32..5.0, 0u32..9).prop_map(|(d, faf, r, ch)| {
            sound(if d == 0 { 0 } else { 0x2A00_0000 + d }, faf, r, ch)
        }),
        (0usize..3, 0i32..4).prop_map(|(p, v)| set_property(PIDS[p], PropertyValue::Int(v))),
        (0usize..3, any::<bool>(), any::<bool>()).prop_map(|(k, p, s)| run(KEYS[k], p, s)),
        (0usize..2).prop_map(|n| goto(NAMES[n])),
        (0u32..30, 0i32..6).prop_map(|(n, p)| environment(n, p)),
        (0usize..2).prop_map(|n| parameter(NAMES[n])),
        (any::<bool>(), 0.0f32..3.0).prop_map(|(f, d)| timer(f, d, Vec::new())),
    ]
}

fn tweak(n: Node, prob: u8, var: u8, driven_volume: bool) -> Node {
    let mut n = match prob {
        0 => with_probability(n, 0.5),
        1 => with_probability(n, 0.0),
        _ => n,
    };
    if var == 0 {
        n.base.intensity_variance = 0.3;
    }
    if driven_volume {
        n.base
            .driven
            .push((pid::INTENSITY_MULTIPLIER, pid::MUSIC_VOLUME));
    }
    n
}

fn graph() -> impl Strategy<Value = Node> {
    let tree = leaf().prop_recursive(6, 96, 4, |inner| {
        let kids = prop::collection::vec(inner.clone(), 1..4);
        prop_oneof![
            kids.clone().prop_map(collection),
            (
                any::<bool>(),
                -1i32..3,
                prop::collection::vec(kids.clone(), 0..3)
            )
                .prop_map(|(f, c, s)| sequence(f, c, s)),
            (
                any::<bool>(),
                prop::collection::vec((0u32..3, kids.clone()), 0..4)
            )
                .prop_map(|(nr, p)| random(nr, p)),
            (
                any::<bool>(),
                0.0f32..2.0,
                prop::collection::vec((0.0f32..3.0, kids.clone()), 0..3)
            )
                .prop_map(|(f, d, p)| timer(f, d, p)),
            (
                0usize..3,
                any::<bool>(),
                prop::collection::vec((0i32..4, 0u8..3, 0u8..3, kids.clone()), 0..3),
                prop::option::of(kids.clone())
            )
                .prop_map(|(p, persistent, cases, def)| {
                    let mut ports: Vec<_> = cases
                        .into_iter()
                        .map(|(v, r, d, c)| {
                            case(
                                vec![PropertyValue::Int(v)],
                                [0.0, 0.3, 0.0][r as usize],
                                [0.0, 0.0, 0.2][d as usize],
                                c,
                            )
                        })
                        .collect();
                    if let Some(d) = def {
                        ports.push(switch_default(d));
                    }
                    switch(PIDS[p], persistent, ports)
                }),
            (
                any::<bool>(),
                prop::option::of(0i32..4),
                prop::option::of(0i32..4),
                kids.clone(),
                kids.clone()
            )
                .prop_map(|(persistent, from, to, t, c)| {
                    switch(
                        pid::VALUE,
                        persistent,
                        vec![
                            case(vec![PropertyValue::Int(1)], 0.0, 0.0, c),
                            transition(
                                from.map(PropertyValue::Int),
                                to.map(PropertyValue::Int),
                                true,
                                t,
                            ),
                        ],
                    )
                }),
            (0usize..2, kids.clone()).prop_map(|(n, c)| label(NAMES[n], c)),
            (0usize..3, any::<bool>(), kids.clone(), kids.clone())
                .prop_map(|(k, p, a, b)| when_running(vec![KEYS[k]], p, a, b)),
            kids.clone().prop_map(on_stop),
            (
                0usize..2,
                prop::collection::vec((0usize..2, kids.clone()), 0..3)
            )
                .prop_map(|(f, b)| scriptlet(
                    SCRIPTLETS[f],
                    vec![(pid::PARAM, PropertyValue::Int(1))],
                    b.into_iter().map(|(n, c)| (NAMES[n], c)).collect()
                )),
            (kids.clone(), 0usize..3).prop_map(|(c, p)| {
                let mut n = collection(Vec::new());
                n.ports = vec![port(PortType::FOR_CURRENT_VALUE, PortPayload::None, c)];
                n.kind = tessera_formats::script_table::NodeKind::SWITCH;
                n.data =
                    tessera_formats::NodeData::Switch(tessera_formats::script_table::SwitchNode {
                        property: PIDS[p],
                        persistent: true,
                        autonomous: false,
                        synchronized: false,
                    });
                n
            }),
        ]
    });
    (tree, 0u8..10, 0u8..3, any::<bool>()).prop_map(|(n, p, v, d)| tweak(n, p, v, d))
}

#[derive(Debug, Clone)]
enum Op {
    Set(usize, i32),
    Advance(f64),
    Complete(usize),
    Start(usize),
    Stop(usize),
}

fn op() -> impl Strategy<Value = Op> {
    prop_oneof![
        (0usize..3, 0i32..4).prop_map(|(p, v)| Op::Set(p, v)),
        (0.0f64..3.0).prop_map(Op::Advance),
        (0usize..8).prop_map(Op::Complete),
        (0usize..3).prop_map(Op::Start),
        (0usize..3).prop_map(Op::Stop),
    ]
}

fn build(scripts: Vec<Vec<Node>>, scriptlets: Vec<Vec<Node>>, seed: u64) -> Runtime<'static> {
    let mut src = MemorySource::new();
    src.insert_table(
        Did(TABLE),
        table(
            2,
            Vec::new(),
            scripts
                .into_iter()
                .enumerate()
                .map(|(i, r)| (KEYS[i], r))
                .collect(),
        ),
    );
    for (i, roots) in scriptlets.into_iter().enumerate() {
        src.insert_scriptlet(Did(SCRIPTLETS[i]), scriptlet_record(roots));
    }
    Runtime::new(src, catalog(), vocab())
        .with_seed(seed)
        .with_limits(RuntimeLimits {
            max_instances_per_entity: 2_000,
            max_starts_per_call: 50_000,
            max_events_per_advance: 50_000,
            ..RuntimeLimits::default()
        })
}

proptest! {
    #![proptest_config(ProptestConfig { cases: 96, ..ProptestConfig::default() })]

    #[test]
    fn random_graphs_never_panic_and_stay_bounded(
        scripts in prop::collection::vec(prop::collection::vec(graph(), 1..4), 3),
        scriptlets in prop::collection::vec(prop::collection::vec(graph(), 0..2), 2),
        ops in prop::collection::vec(op(), 0..40),
        seed in any::<u64>(),
    ) {
        let mut rt = build(scripts, scriptlets, seed);
        let mut sink = RecordingSink::new();
        sink.max_events = 1000;
        const P: EntityId = EntityId(2);
        rt.spawn_entity(&mut sink, P, init(Vec::new())).unwrap();
        rt.set_player(Some(P));
        rt.spawn_entity(&mut sink, E, init(vec![(pid::VALUE, PropertyValue::Int(1))])).unwrap();
        for o in ops {
            match o {
                Op::Set(p, v) => {
                    let value = if PIDS[p] == pid::MUSIC_VOLUME {
                        PropertyValue::Float(v as f32 / 3.0)
                    } else {
                        PropertyValue::Int(v)
                    };
                    rt.set_property(&mut sink, E, PIDS[p], value).unwrap();
                }
                Op::Advance(dt) => rt.advance(&mut sink, dt),
                Op::Complete(i) => {
                    let h: Option<SoundHandle> = sink.playing.keys().nth(i).copied();
                    if let Some(h) = h {
                        sink.complete(h);
                        rt.sound_completed(&mut sink, h);
                    }
                }
                Op::Start(k) => { rt.start_script(&mut sink, E, KEYS[k]).unwrap(); }
                Op::Stop(k) => { rt.stop_script(&mut sink, E, KEYS[k]).unwrap(); }
            }
            let s = rt.stats();
            prop_assert!(rt.entity_instances(E).unwrap() <= 2_000);
            prop_assert!(s.tracked_sounds <= sink.playing.len());
        }
        rt.despawn(&mut sink, E).unwrap();
        rt.despawn(&mut sink, P).unwrap();
        let s = rt.stats();
        prop_assert_eq!(s.instances, 0);
        prop_assert_eq!(s.runs, 0);
        prop_assert_eq!(s.tracked_sounds, 0);
        prop_assert!(sink.playing.is_empty(), "every tracked voice stopped");
        prop_assert!(sink.environments.is_empty(), "every environment unregistered");
    }
}
