// SPDX-License-Identifier: GPL-3.0-or-later
//! While (50) and PhysicsImpulse (9) at run time (`script-runtime.md` §30), with the shape
//! of the avatar's jump scripts (§30.7) built from synthetic nodes.

mod common;

use common::*;
use tessera_formats::PropertyValue;
use tessera_formats::property::PropertyEntry;
use tessera_formats::script_table::{
    Node, NodeData, NodeKind, PhysicsImpulseNode, PortPayload, WhileNode,
};
use tessera_scriptgraph::{RecordingSink, Runtime, ScriptStart, SinkEvent};

const JUMPING: u32 = 0x1000_0001;
const JUMP: u32 = 0x1000_0002;
const KEY: u32 = 0x1000_0003;
const SND: u32 = 0x2A00_0001;
const SND2: u32 = 0x2A00_0002;

/// A While on `property` with `Values` and `Inverse`, one Default port.
fn while_node(
    property: u32,
    values: Vec<PropertyValue>,
    inverse: bool,
    children: Vec<Node>,
) -> Node {
    node(
        NodeKind::WHILE,
        NodeData::While(Box::new(WhileNode {
            property,
            values: values
                .into_iter()
                .map(|value| {
                    Some(PropertyEntry {
                        pid: property,
                        value,
                    })
                })
                .collect(),
            names: Vec::new(),
            inverse,
        })),
        vec![default_port(PortPayload::None, children)],
    )
}

fn impulse(is_jump: bool, vector: [f32; 3]) -> Node {
    node(
        NodeKind::PHYSICS_IMPULSE,
        NodeData::PhysicsImpulse(PhysicsImpulseNode {
            is_jump,
            impulse_type: 2,
            global_space: false,
            impulse: vector,
            air_time: 0.2,
            expected_gravity: 15.0,
        }),
        Vec::new(),
    )
}

/// A Collection with explicit ordinals.
fn ordered(ports: Vec<(u32, Vec<Node>)>) -> Node {
    Node {
        ports: ports
            .into_iter()
            .map(|(o, c)| default_port(PortPayload::U32(o), c))
            .collect(),
        ..collection(Vec::new())
    }
}

fn impulses(sink: &RecordingSink) -> Vec<[f32; 3]> {
    sink.events
        .iter()
        .filter_map(|e| match e {
            SinkEvent::Impulse(r) => Some(r.vector),
            _ => None,
        })
        .collect()
}

/// `Loc_Jumping` on foot (§30.7 item 3): the flag set by ordinal 1, the While on it by
/// ordinal 4; inside, a one-pass Sequence: the jump impulse and a persistent Run of `Jump`,
/// a 0.2 s Timer, the clear of the flag. `Jump` holds a tracked sound, so it keeps running.
fn jump_runtime() -> Runtime<'static> {
    let seq = sequence(
        false,
        1,
        vec![
            vec![impulse(true, [0.0; 3]), run(JUMP, true, false)],
            vec![timer(false, 0.2, Vec::new())],
            vec![set_property(pid::FLAG, PropertyValue::Bool(false))],
        ],
    );
    let jumping = ordered(vec![
        (
            4,
            vec![while_node(
                pid::FLAG,
                vec![PropertyValue::Bool(true)],
                false,
                vec![seq],
            )],
        ),
        (1, vec![set_property(pid::FLAG, PropertyValue::Bool(true))]),
    ]);
    single(vec![
        (JUMPING, vec![jumping]),
        (JUMP, vec![sound(SND, false, 0.0, 0)]),
    ])
}

#[test]
fn the_jump_path_requests_the_impulse_in_the_start_and_holds_the_flag() {
    let mut rt = jump_runtime();
    let mut sink = RecordingSink::new();
    rt.spawn_entity(&mut sink, E, init(Vec::new())).unwrap();
    assert_eq!(
        rt.start_script(&mut sink, E, JUMPING).unwrap(),
        ScriptStart::Running
    );
    // §30.7 a: the impulse is requested in the call that starts the script; the vertical
    // value is (AirTime + 0.2) × ExpectedGravity = 6.0, not scaled by the intensity.
    let v = impulses(&sink);
    assert_eq!(v.len(), 1);
    assert!((v[0][2] - 6.0).abs() < 1e-5, "{v:?}");
    assert!(rt.is_running(E, JUMP));
    // §30.7 b: the persistent Run holds step 1, so the clear is never reached.
    run_until(&mut rt, &mut sink, 2.0);
    assert_eq!(rt.property(E, pid::FLAG), Some(PropertyValue::Bool(true)));
    assert!(rt.is_running(E, JUMPING));
    // §30.7 d: clearing the flag stops the branch (reason 0): `Jump` stops, and the While,
    // the Collection and the run end in the same call.
    rt.set_property(&mut sink, E, pid::FLAG, PropertyValue::Bool(false))
        .unwrap();
    assert!(!rt.is_running(E, JUMP));
    assert!(!rt.is_running(E, JUMPING));
    assert!(sink.playing.is_empty());
    // Later changes do nothing more.
    rt.set_property(&mut sink, E, pid::FLAG, PropertyValue::Bool(true))
        .unwrap();
    assert_eq!(impulses(&sink).len(), 1);
}

#[test]
fn a_failing_test_completes_the_while_at_once() {
    let w = while_node(
        pid::VALUE,
        vec![PropertyValue::Int(3)],
        false,
        vec![sound(SND, false, 0.0, 0)],
    );
    let mut rt = single(vec![(KEY, vec![w])]);
    let mut sink = RecordingSink::new();
    rt.spawn_entity(&mut sink, E, init(Vec::new())).unwrap();
    // VALUE is unset: its type's zero value (0) matches nothing (§30.2 step 1).
    assert_eq!(
        rt.start_script(&mut sink, E, KEY).unwrap(),
        ScriptStart::Completed
    );
    assert_eq!(sink.sounds_started, 0);
    // Inverse: the same test now holds.
    let w = while_node(
        pid::VALUE,
        vec![PropertyValue::Int(3)],
        true,
        vec![sound(SND, false, 0.0, 0)],
    );
    let mut rt = single(vec![(KEY, vec![w])]);
    let mut sink = RecordingSink::new();
    rt.spawn_entity(&mut sink, E, init(Vec::new())).unwrap();
    assert_eq!(
        rt.start_script(&mut sink, E, KEY).unwrap(),
        ScriptStart::Running
    );
    assert_eq!(sink.sounds_started, 1);
}

#[test]
fn a_finished_branch_is_not_restarted_and_the_while_outlives_it() {
    // A fire-and-forget sound completes at once: the branch is done, the While keeps
    // running with no child while the test holds (§30.4).
    let w = while_node(
        pid::VALUE,
        vec![PropertyValue::Int(1), PropertyValue::Int(2)],
        false,
        vec![sound(SND2, true, 0.0, 0)],
    );
    let mut rt = single(vec![(KEY, vec![w])]);
    let mut sink = RecordingSink::new();
    rt.spawn_entity(
        &mut sink,
        E,
        init(vec![(pid::VALUE, PropertyValue::Int(1))]),
    )
    .unwrap();
    assert_eq!(
        rt.start_script(&mut sink, E, KEY).unwrap(),
        ScriptStart::Running
    );
    assert_eq!(sink.sounds_started, 1);
    rt.set_property(&mut sink, E, pid::VALUE, PropertyValue::Int(2))
        .unwrap();
    assert!(rt.is_running(E, KEY));
    assert_eq!(sink.sounds_started, 1, "not started a second time");
    rt.set_property(&mut sink, E, pid::VALUE, PropertyValue::Int(5))
        .unwrap();
    assert!(!rt.is_running(E, KEY), "the first failing test ends it");
}

#[test]
fn a_change_inside_the_branch_stops_its_later_siblings() {
    // §30.4: the SetProperty that makes the test fail stops the branch inside its own
    // start, so the sound of the next port never starts.
    let body = ordered(vec![
        (1, vec![set_property(pid::FLAG, PropertyValue::Bool(false))]),
        (2, vec![sound(SND, false, 0.0, 0)]),
    ]);
    let w = while_node(
        pid::FLAG,
        vec![PropertyValue::Bool(true)],
        false,
        vec![body],
    );
    let mut rt = single(vec![(KEY, vec![w])]);
    let mut sink = RecordingSink::new();
    rt.spawn_entity(
        &mut sink,
        E,
        init(vec![(pid::FLAG, PropertyValue::Bool(true))]),
    )
    .unwrap();
    assert_eq!(
        rt.start_script(&mut sink, E, KEY).unwrap(),
        ScriptStart::Completed
    );
    assert_eq!(sink.sounds_started, 0);
}

#[test]
fn a_plain_impulse_is_scaled_by_the_intensity() {
    let n = with_intensity(impulse(false, [1.0, 2.0, -4.0]), 0.5, 0.0, true);
    let mut rt = single(vec![(KEY, vec![n])]);
    let mut sink = RecordingSink::new();
    rt.spawn_entity(&mut sink, E, init(Vec::new())).unwrap();
    assert_eq!(
        rt.start_script(&mut sink, E, KEY).unwrap(),
        ScriptStart::Completed,
        "the node completes at once (OPEN-20 Decision)"
    );
    assert_eq!(impulses(&sink), vec![[0.5, 1.0, -2.0]]);
    let SinkEvent::Impulse(r) = sink
        .events
        .iter()
        .find(|e| matches!(e, SinkEvent::Impulse(_)))
        .unwrap()
    else {
        unreachable!()
    };
    assert_eq!((r.entity, r.target, r.node.impulse_type), (E, E, 2));
    assert!(rt.issues().is_empty(), "{:?}", rt.issues());
}
