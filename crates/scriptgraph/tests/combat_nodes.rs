// SPDX-License-Identifier: GPL-3.0-or-later
//! The combat-facing node kinds (`script-table.md` §9.1, `combat-visuals.md` §3.2, §4.2,
//! §4.6, §11 rule 8, `particle-runtime.md` §11.1): client starts with run parameters,
//! StoryHook events with the run's story, Execute callbacks writing run parameters and
//! choosing ports, Property and Random targets, and Beam nodes. Every DID, key and PID is
//! invented.

mod common;

use common::*;
use tessera_formats::property::PropertyEntry;
use tessera_formats::script_table::{
    BeamNode, ExecuteNode, Node, NodeData, NodeKind, ParticleScriptNode, PortPayload, PortType,
    Target, TargetData, TargetValue,
};
use tessera_formats::{Did, PropertyArray, PropertySet, PropertyValue};
use tessera_scriptgraph::{
    BeamHandle, BeamStart, EffectSink, EntityId, EnvironmentHandle, EnvironmentRequest,
    ExecuteReply, ExecuteRequest, MemorySource, ParticleHandle, ParticleStart, RecordingSink,
    Runtime, ScriptStart, SinkEvent, SoundHandle, SoundStart, SoundStartOutcome, StoryHookEvent,
};

const KEY: u32 = 0x1000_0001;
const FX: u32 = 0x3900_0001;
/// `StorySystem_StoryID` (a script parameter).
const STORY: u32 = 0x2001;
/// The callback's target array output (a script parameter).
const TARGETS: u32 = 0x2002;
/// The callback's result flag (a script parameter).
const RESULT: u32 = 0x2003;
const ATTACK: u32 = 2;
const DEFEND: u32 = 1;
const OTHER_ENTITY: EntityId = EntityId(2);

fn runtime(scripts: Vec<(u32, Vec<Node>)>) -> Runtime<'static> {
    let mut src = MemorySource::new();
    src.insert_table(Did(TABLE), table(2, Vec::new(), scripts));
    let mut c = catalog();
    for p in [STORY, TARGETS, RESULT] {
        c.script_parameters.insert(p);
    }
    let mut v = vocab();
    v.story_id = Some(STORY);
    Runtime::new(src, c, v).with_seed(7)
}

fn hook(ty: u32) -> Node {
    node(
        NodeKind::STORY_HOOK,
        NodeData::StoryHook { hook_type: ty },
        Vec::new(),
    )
}

fn particle_at(targets: Vec<Target>) -> Node {
    let mut n = node(
        NodeKind::PARTICLE,
        NodeData::Particle(Box::new(ParticleScriptNode {
            particle_id: Did(FX),
            ..ParticleScriptNode::default()
        })),
        Vec::new(),
    );
    n.particle_flag = Some(false);
    n.targets = targets;
    n
}

fn property_target(pid: u32) -> Target {
    Target {
        port_type: tessera_formats::script_table::PortType::TARGET,
        value: TargetValue::None,
        target_type: 3,
        data: TargetData::U32(pid),
        sub_targets: Vec::new(),
    }
}

fn random_target(cap: i32, subs: Vec<Target>) -> Target {
    Target {
        port_type: tessera_formats::script_table::PortType::TARGET,
        value: TargetValue::None,
        target_type: 6,
        data: TargetData::Cap(cap),
        sub_targets: subs,
    }
}

fn execute(
    default: Option<PropertyEntry>,
    ports: Vec<tessera_formats::script_table::Port>,
) -> Node {
    node(
        NodeKind::EXECUTE,
        NodeData::Execute(Box::new(ExecuteNode {
            callback: 3,
            default_value: default,
            call_properties: PropertySet::default(),
        })),
        ports,
    )
}

fn result_port(value: u8, children: Vec<Node>) -> tessera_formats::script_table::Port {
    port(
        PortType::SWITCH_VALUE,
        PortPayload::ExecuteResult(RESULT, value),
        children,
    )
}

fn hooks(sink: &RecordingSink) -> Vec<StoryHookEvent> {
    sink.events
        .iter()
        .filter_map(|e| match e {
            SinkEvent::StoryHook(h) => Some(*h),
            _ => None,
        })
        .collect()
}

fn particle_targets(sink: &RecordingSink) -> Vec<EntityId> {
    sink.events
        .iter()
        .filter_map(|e| match e {
            SinkEvent::ParticleStarted(_, s) => Some(s.target),
            _ => None,
        })
        .collect()
}

/// A recording sink whose Execute callback answers with fixed outputs.
struct CallbackSink {
    inner: RecordingSink,
    reply: Option<ExecuteReply>,
    calls: Vec<(u32, Option<i64>)>,
}

impl CallbackSink {
    fn new(reply: Option<ExecuteReply>) -> Self {
        Self {
            inner: RecordingSink::new(),
            reply,
            calls: Vec::new(),
        }
    }
}

impl EffectSink for CallbackSink {
    fn start_sound(&mut self, r: &SoundStart) -> SoundStartOutcome {
        self.inner.start_sound(r)
    }
    fn stop_sound(&mut self, h: SoundHandle) {
        self.inner.stop_sound(h);
    }
    fn set_sound_volume(&mut self, h: SoundHandle, v: f32) {
        self.inner.set_sound_volume(h, v);
    }
    fn register_environment(&mut self, r: &EnvironmentRequest) -> EnvironmentHandle {
        self.inner.register_environment(r)
    }
    fn unregister_environment(&mut self, h: EnvironmentHandle) {
        self.inner.unregister_environment(h);
    }
    fn start_particle(&mut self, r: &ParticleStart) -> Option<ParticleHandle> {
        self.inner.start_particle(r)
    }
    fn story_hook(&mut self, e: &StoryHookEvent) {
        self.inner.story_hook(e);
    }
    fn execute(&mut self, r: &ExecuteRequest<'_>) -> Option<ExecuteReply> {
        self.calls.push((r.callback, r.story));
        self.reply.clone()
    }
    fn start_beam(&mut self, r: &BeamStart) -> Option<BeamHandle> {
        self.inner.start_beam(r)
    }
    fn stop_beam(&mut self, h: BeamHandle) {
        self.inner.stop_beam(h);
    }
}

fn targets_array(ids: &[u64]) -> PropertyValue {
    PropertyValue::Array(PropertyArray {
        count: ids.len() as u32,
        elements: ids
            .iter()
            .map(|&i| PropertyEntry {
                pid: TARGETS,
                value: PropertyValue::InstanceId(i),
            })
            .collect(),
    })
}

#[test]
fn hooks_carry_the_story_of_their_run() {
    // A swing: a Timer with a Defend hook at 0.371 s and an Attack hook at 0.416 s.
    let swing = timer(
        false,
        1.0,
        vec![(0.371, vec![hook(DEFEND)]), (0.416, vec![hook(ATTACK)])],
    );
    let mut rt = runtime(vec![(KEY, vec![swing])]);
    let mut sink = RecordingSink::new();
    rt.spawn_entity(&mut sink, E, init(Vec::new())).unwrap();
    let run = rt
        .start_script_with(&mut sink, E, KEY, &[(STORY, PropertyValue::Int64(7))])
        .unwrap();
    assert_eq!(run.start, ScriptStart::Running);
    let id = run.run.expect("a live run");
    assert!(rt.run_live(id));
    run_until(&mut rt, &mut sink, 0.5);
    let h = hooks(&sink);
    assert_eq!(h.len(), 2);
    assert_eq!(
        (h[0].hook_type, h[0].story, h[0].entity),
        (DEFEND, Some(7), E)
    );
    assert_eq!((h[1].hook_type, h[1].story), (ATTACK, Some(7)));
    run_until(&mut rt, &mut sink, 1.1);
    assert!(!rt.run_live(id), "the run ended with its Timer");
    assert!(
        !rt.stop_run(&mut sink, id),
        "an ended run cannot be stopped"
    );

    // A run without the parameter belongs to no story.
    let other = rt.start_script_with(&mut sink, E, KEY, &[]).unwrap();
    run_until(&mut rt, &mut sink, 1.6);
    assert_eq!(hooks(&sink).last().map(|h| h.story), Some(None));
    assert!(rt.stop_run(&mut sink, other.run.unwrap()));
}

#[test]
fn execute_outputs_reach_the_nodes_below_and_choose_the_port() {
    let graph = execute(
        None,
        vec![
            result_port(1, vec![particle_at(vec![property_target(TARGETS)])]),
            default_port(PortPayload::None, vec![particle_at(Vec::new())]),
        ],
    );
    let mut rt = runtime(vec![(KEY, vec![graph])]);
    let reply = ExecuteReply {
        outputs: vec![
            (TARGETS, targets_array(&[OTHER_ENTITY.0])),
            (RESULT, PropertyValue::Bool(true)),
        ],
    };
    let mut sink = CallbackSink::new(Some(reply));
    rt.spawn_entity(&mut sink, E, init(Vec::new())).unwrap();
    rt.spawn_entity(&mut sink, OTHER_ENTITY, init(Vec::new()))
        .unwrap();
    rt.start_script_with(&mut sink, E, KEY, &[(STORY, PropertyValue::Int(3))])
        .unwrap();
    assert_eq!(sink.calls, vec![(3, Some(3))]);
    assert_eq!(particle_targets(&sink.inner), vec![OTHER_ENTITY]);

    // A false flag matches no result port: the Default port fires instead.
    let mut rt = runtime(vec![(
        KEY,
        vec![execute(
            None,
            vec![
                result_port(1, vec![particle_at(vec![property_target(TARGETS)])]),
                default_port(PortPayload::None, vec![particle_at(Vec::new())]),
            ],
        )],
    )]);
    let reply = ExecuteReply {
        outputs: vec![(RESULT, PropertyValue::Bool(false))],
    };
    let mut sink = CallbackSink::new(Some(reply));
    rt.spawn_entity(&mut sink, E, init(Vec::new())).unwrap();
    rt.start_script(&mut sink, E, KEY).unwrap();
    assert_eq!(particle_targets(&sink.inner), vec![E]);
}

#[test]
fn an_unanswered_execute_follows_its_default_value() {
    let default = PropertyEntry {
        pid: RESULT,
        value: PropertyValue::Bool(true),
    };
    let graph = execute(
        Some(default),
        vec![
            result_port(1, vec![particle_at(Vec::new())]),
            result_port(0, vec![particle_at(Vec::new()), particle_at(Vec::new())]),
        ],
    );
    let mut rt = runtime(vec![(KEY, vec![graph])]);
    let mut sink = RecordingSink::new();
    rt.spawn_entity(&mut sink, E, init(Vec::new())).unwrap();
    rt.start_script(&mut sink, E, KEY).unwrap();
    assert_eq!(
        particle_targets(&sink),
        vec![E],
        "only the port matching 1 fired"
    );

    // Without a DefaultValue the node completes at once and fires nothing.
    let mut rt = runtime(vec![(
        KEY,
        vec![execute(
            None,
            vec![result_port(1, vec![particle_at(Vec::new())])],
        )],
    )]);
    let mut sink = RecordingSink::new();
    rt.spawn_entity(&mut sink, E, init(Vec::new())).unwrap();
    assert_eq!(
        rt.start_script(&mut sink, E, KEY).unwrap(),
        ScriptStart::Completed
    );
    assert!(particle_targets(&sink).is_empty());
}

fn beam(children: Vec<Node>, targets: Vec<Target>) -> Node {
    let mut n = node(
        NodeKind::BEAM,
        NodeData::Beam(Box::new(BeamNode {
            head_speed: 40.0,
            sustain: 1.0,
            ramp_out: 0.25,
            ..BeamNode::default()
        })),
        vec![default_port(PortPayload::None, children)],
    );
    n.targets = targets;
    n
}

#[test]
fn a_beam_fires_its_port_once_on_arrival_and_ends_with_its_beam() {
    let graph = beam(
        vec![hook(ATTACK)],
        vec![random_target(1, vec![property_target(TARGETS)])],
    );
    let mut rt = runtime(vec![(KEY, vec![graph])]);
    let mut sink = RecordingSink::new();
    rt.spawn_entity(&mut sink, E, init(Vec::new())).unwrap();
    let run = rt
        .start_script_with(
            &mut sink,
            E,
            KEY,
            &[
                (STORY, PropertyValue::Int(9)),
                (TARGETS, targets_array(&[5, 6, 5])),
            ],
        )
        .unwrap();
    let id = run.run.expect("the beam keeps the run live");
    let (&h, b) = sink.beams.iter().next().expect("a beam");
    assert!(matches!(b.target, Some(EntityId(5 | 6))));
    assert_eq!(b.entity, E);
    assert!(
        hooks(&sink).is_empty(),
        "nothing fires before the head arrives"
    );
    assert!(rt.beam_arrived(&mut sink, h));
    assert!(!rt.beam_arrived(&mut sink, h), "the port fires once");
    let fired = hooks(&sink);
    assert_eq!(fired.len(), 1);
    assert_eq!((fired[0].hook_type, fired[0].story), (ATTACK, Some(9)));
    assert!(rt.run_live(id));
    assert!(rt.beam_finished(&mut sink, h));
    assert!(!rt.run_live(id), "the node completed with its beam");
    assert!(sink.beams.is_empty(), "the stop hook stopped the beam");

    // Without a target (and without UseTargetPosition) no beam is made.
    let mut rt = runtime(vec![(
        KEY,
        vec![beam(vec![hook(ATTACK)], vec![property_target(TARGETS)])],
    )]);
    let mut sink = RecordingSink::new();
    rt.spawn_entity(&mut sink, E, init(Vec::new())).unwrap();
    assert_eq!(
        rt.start_script(&mut sink, E, KEY).unwrap(),
        ScriptStart::Completed
    );
    assert!(sink.beams.is_empty());

    // Stopping the run stops the beam.
    let mut rt = runtime(vec![(
        KEY,
        vec![beam(Vec::new(), vec![property_target(TARGETS)])],
    )]);
    let mut sink = RecordingSink::new();
    rt.spawn_entity(&mut sink, E, init(Vec::new())).unwrap();
    let run = rt
        .start_script_with(&mut sink, E, KEY, &[(TARGETS, targets_array(&[5]))])
        .unwrap();
    assert_eq!(sink.beams.len(), 1);
    assert!(rt.stop_run(&mut sink, run.run.unwrap()));
    assert!(sink.beams.is_empty());
    assert!(
        sink.events
            .iter()
            .any(|e| matches!(e, SinkEvent::BeamStopped(_)))
    );
}

#[test]
fn hostile_targets_and_outputs_are_bounded() {
    // A Random target with a huge cap over a long array, and an Execute reply with far
    // more outputs than kept: nothing panics, the counts stay bounded.
    let ids: Vec<u64> = (1..=5000).collect();
    let graph = execute(
        None,
        vec![result_port(
            1,
            vec![particle_at(vec![random_target(
                i32::MAX,
                vec![property_target(TARGETS)],
            )])],
        )],
    );
    let mut rt = runtime(vec![(KEY, vec![graph])]);
    let mut outputs: Vec<(u32, PropertyValue)> = vec![
        (TARGETS, targets_array(&ids)),
        (RESULT, PropertyValue::Bool(true)),
    ];
    outputs.extend((0..10_000).map(|i| (0x3000 + i, PropertyValue::Int(1))));
    let mut sink = CallbackSink::new(Some(ExecuteReply { outputs }));
    rt.spawn_entity(&mut sink, E, init(Vec::new())).unwrap();
    rt.start_script(&mut sink, E, KEY).unwrap();
    assert_eq!(particle_targets(&sink.inner).len(), 1);
}

#[test]
fn switch_cases_lists_the_values_a_definition_switches_on() {
    // A death script: a Switch on the dead-script property with three cases, one repeated,
    // under a Collection; another Switch on another property is not listed.
    let dead = pid::VALUE;
    let graph = collection(vec![
        switch(
            dead,
            false,
            vec![
                case(vec![PropertyValue::Int(11)], 0.0, 0.0, Vec::new()),
                case(
                    vec![PropertyValue::Int(12), PropertyValue::Int(11)],
                    0.0,
                    0.0,
                    Vec::new(),
                ),
                switch_default(Vec::new()),
            ],
        ),
        switch(
            pid::OTHER,
            false,
            vec![case(vec![PropertyValue::Int(99)], 0.0, 0.0, Vec::new())],
        ),
    ]);
    let mut rt = runtime(vec![(KEY, vec![graph])]);
    let mut sink = RecordingSink::new();
    rt.spawn_entity(&mut sink, E, init(Vec::new())).unwrap();
    assert_eq!(
        rt.switch_cases(E, KEY, dead),
        vec![PropertyValue::Int(11), PropertyValue::Int(12)]
    );
    assert!(rt.switch_cases(E, 0x7777, dead).is_empty(), "undefined key");
    assert!(
        rt.switch_cases(EntityId(99), KEY, dead).is_empty(),
        "unknown entity"
    );
}

/// A Particle node over a Property target that names no entity (an empty upcoming-targets
/// array) starts nothing, instead of playing on the running entity; a node aimed at This
/// or ThePlayer keeps `particle-runtime.md` §5.5 step 1 (the script's entity).
#[test]
fn particle_targets_that_resolve_to_nothing_start_nothing() {
    let graph = execute(
        None,
        vec![default_port(
            PortPayload::None,
            vec![particle_at(vec![property_target(TARGETS)])],
        )],
    );
    let mut rt = runtime(vec![(KEY, vec![graph])]);
    let reply = ExecuteReply {
        outputs: vec![
            (TARGETS, targets_array(&[])),
            (RESULT, PropertyValue::Bool(false)),
        ],
    };
    let mut sink = CallbackSink::new(Some(reply));
    rt.spawn_entity(&mut sink, E, init(Vec::new())).unwrap();
    rt.start_script(&mut sink, E, KEY).unwrap();
    assert!(particle_targets(&sink.inner).is_empty());

    let player = Target {
        port_type: PortType::TARGET,
        value: TargetValue::None,
        target_type: 4,
        data: TargetData::None,
        sub_targets: Vec::new(),
    };
    let mut rt = runtime(vec![(KEY, vec![particle_at(vec![player])])]);
    let mut sink = RecordingSink::new();
    rt.spawn_entity(&mut sink, E, init(Vec::new())).unwrap();
    rt.spawn_entity(&mut sink, OTHER_ENTITY, init(Vec::new()))
        .unwrap();
    rt.set_player(Some(OTHER_ENTITY));
    rt.start_script(&mut sink, E, KEY).unwrap();
    assert_eq!(particle_targets(&sink), vec![E]);
}
