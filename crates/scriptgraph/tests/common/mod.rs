// SPDX-License-Identifier: GPL-3.0-or-later
//! Builders for synthetic script graphs. Every DID, key, PID and value is invented.

#![allow(dead_code)]

use std::cell::Cell;
use std::collections::HashMap;

use tessera_formats::property::PropertyEntry;
use tessera_formats::script_table::resolve::PropertyState;
use tessera_formats::script_table::{
    BaseBlock, Node, NodeData, NodeKind, Port, PortPayload, PortType, RunNode, SoundNode,
    SwitchCase, SwitchNode, SwitchTransition, Target, TargetData, TimerNode, WhenTest,
};
use tessera_formats::{
    Did, PropertySet, PropertyType, PropertyValue, Script, ScriptTable, Scriptlet,
};
use tessera_scriptgraph::{
    EntityId, EntityInit, MemorySource, RecordingSink, Runtime, RuntimeVocabulary, SoundHandle,
    SyntheticCatalog,
};

/// Synthetic PIDs.
pub mod pid {
    pub const AREA_MUSIC: u32 = 0x1001;
    pub const MUSIC_VOLUME: u32 = 0x1002;
    pub const INTENSITY_MULTIPLIER: u32 = 0x1003;
    pub const MAX_INSTANCES: u32 = 0x1004;
    pub const COMBINATION: u32 = 0x1005;
    pub const VALUE: u32 = 0x1006;
    pub const COUNTER: u32 = 0x1007;
    pub const FLAG: u32 = 0x1008;
    pub const PARAM: u32 = 0x1009;
    pub const OTHER: u32 = 0x100A;
    pub const SOUND_ID: u32 = 0x100B;
    pub const OUTPUT_SOURCE: u32 = 0x100C;
    /// An enum without a dictionary default (like the music graph's monitor properties).
    pub const MONITOR: u32 = 0x100D;
    /// A tri-state without a dictionary default.
    pub const TRI: u32 = 0x100E;
    /// An int with the dictionary default 2.
    pub const LIST: u32 = 0x100F;
    /// A property ID that the dictionary does not define.
    pub const UNDEFINED: u32 = 0x1010;
}

/// The `Startup` key of the synthetic vocabulary.
pub const STARTUP: u32 = 5;
/// The `Overriding` key of the synthetic `Script_CombinationType`.
pub const OVERRIDING: u32 = 1;

thread_local! {
    static NEXT_ID: Cell<u32> = const { Cell::new(1) };
}

fn next_id() -> u32 {
    NEXT_ID.with(|c| {
        let v = c.get();
        c.set(v + 1);
        v
    })
}

/// The synthetic vocabulary.
pub fn vocab() -> RuntimeVocabulary {
    let names: HashMap<String, u32> = [
        ("Script_IntensityMultiplier", pid::INTENSITY_MULTIPLIER),
        ("Script_MaxNumInstances", pid::MAX_INSTANCES),
        ("Script_CombinationType", pid::COMBINATION),
        ("SoundScript_SoundID", pid::SOUND_ID),
        ("RunScriptOutput_Source", pid::OUTPUT_SOURCE),
    ]
    .into_iter()
    .map(|(n, p)| (n.to_string(), p))
    .collect();
    let mut v = RuntimeVocabulary::from_names(&names);
    v.startup_key = Some(STARTUP);
    v.combination_overriding = Some(OVERRIDING);
    v
}

/// The synthetic catalog: the music volume defaults to 1.0 and `LIST` to 2; `PARAM` is a
/// script parameter; every synthetic PID but `UNDEFINED` has a definition (an unset one
/// reads its default, else its type's zero value, `script-runtime.md` §6.2).
pub fn catalog() -> SyntheticCatalog {
    let mut c = SyntheticCatalog::default();
    c.defaults
        .insert(pid::MUSIC_VOLUME, PropertyValue::Float(1.0));
    c.defaults.insert(pid::LIST, PropertyValue::Int(2));
    c.script_parameters.insert(pid::PARAM);
    for (p, ty) in [
        (pid::AREA_MUSIC, PropertyType::Enum),
        (pid::INTENSITY_MULTIPLIER, PropertyType::Float),
        (pid::MAX_INSTANCES, PropertyType::Int),
        (pid::COMBINATION, PropertyType::Enum),
        (pid::VALUE, PropertyType::Int),
        (pid::COUNTER, PropertyType::Int),
        (pid::FLAG, PropertyType::Bool),
        (pid::PARAM, PropertyType::Int),
        (pid::OTHER, PropertyType::Int),
        (pid::SOUND_ID, PropertyType::DidRef),
        (pid::OUTPUT_SOURCE, PropertyType::InstanceId),
        (pid::MONITOR, PropertyType::Enum),
        (pid::TRI, PropertyType::TriState),
    ] {
        c.types.insert(p, ty);
    }
    c
}

pub fn base() -> BaseBlock {
    BaseBlock {
        driven_hint: 1,
        driven: Vec::new(),
        scriptlet_driven_hint: 1,
        scriptlet_driven: Vec::new(),
        scriptlet_params: Vec::new(),
        use_parent_intensity: true,
        intensity_multiplier: 1.0,
        intensity_variance: 0.0,
        probability: 1.0,
        node_id: next_id(),
    }
}

pub fn node(kind: NodeKind, data: NodeData, ports: Vec<Port>) -> Node {
    Node {
        kind,
        data,
        base: base(),
        particle_flag: None,
        ports,
        targets: Vec::new(),
    }
}

pub fn port(port_type: PortType, payload: PortPayload, children: Vec<Node>) -> Port {
    Port {
        port_type,
        payload,
        children,
    }
}

pub fn default_port(payload: PortPayload, children: Vec<Node>) -> Port {
    port(PortType::DEFAULT, payload, children)
}

pub fn collection(children: Vec<Node>) -> Node {
    node(
        NodeKind::COLLECTION,
        NodeData::None,
        vec![default_port(PortPayload::U32(1), children)],
    )
}

pub fn sequence(forever: bool, count: i32, steps: Vec<Vec<Node>>) -> Node {
    let ports = steps
        .into_iter()
        .enumerate()
        .map(|(i, c)| default_port(PortPayload::U32(i as u32 + 1), c))
        .collect();
    node(
        NodeKind::SEQUENCE,
        NodeData::Sequence {
            loop_forever: forever,
            loop_count: count,
        },
        ports,
    )
}

pub fn random(no_repeats: bool, ports: Vec<(u32, Vec<Node>)>) -> Node {
    node(
        NodeKind::RANDOM,
        NodeData::Random {
            no_repeats,
            autonomous: false,
        },
        ports
            .into_iter()
            .map(|(w, c)| default_port(PortPayload::U32(w), c))
            .collect(),
    )
}

pub fn timer(forever: bool, duration: f32, ports: Vec<(f32, Vec<Node>)>) -> Node {
    node(
        NodeKind::TIMER,
        NodeData::Timer(TimerNode {
            loop_forever: forever,
            duration,
            loop_count: 1,
            autonomous: true,
            speed: 1.0,
            speed_multiplier: 1.0,
            tools_animation_override: Did(0),
        }),
        ports
            .into_iter()
            .map(|(t, c)| default_port(PortPayload::F32(t), c))
            .collect(),
    )
}

pub fn case(values: Vec<PropertyValue>, ramp_out: f32, delay: f32, children: Vec<Node>) -> Port {
    port(
        PortType::SWITCH_VALUE,
        PortPayload::SwitchCase(Box::new(SwitchCase {
            values: values
                .into_iter()
                .map(|v| Some(PropertyEntry { pid: 0, value: v }))
                .collect(),
            ints: Vec::new(),
            operator: 3,
            transition_time: ramp_out,
            second_time: delay,
        })),
        children,
    )
}

pub fn switch_default(children: Vec<Node>) -> Port {
    default_port(PortPayload::F32Pair(0.0, 0.0), children)
}

pub fn switch(property: u32, persistent: bool, ports: Vec<Port>) -> Node {
    node(
        NodeKind::SWITCH,
        NodeData::Switch(SwitchNode {
            property,
            persistent,
            autonomous: true,
            synchronized: false,
        }),
        ports,
    )
}

pub fn transition(
    from: Option<PropertyValue>,
    to: Option<PropertyValue>,
    wait: bool,
    children: Vec<Node>,
) -> Port {
    port(
        PortType::SWITCH_TRANSITION,
        PortPayload::SwitchTransition(Box::new(transition_payload(from, to, wait))),
        children,
    )
}

fn transition_payload(
    from: Option<PropertyValue>,
    to: Option<PropertyValue>,
    wait: bool,
) -> SwitchTransition {
    SwitchTransition {
        flags: [from.is_none(), to.is_none(), wait],
        from: from.map(|v| PropertyEntry { pid: 0, value: v }),
        to: to.map(|v| PropertyEntry { pid: 0, value: v }),
        floats: [0.0, 0.0, -1.0, 0.0],
    }
}

pub fn when(
    property: u32,
    persistent: bool,
    tests: Vec<(i32, PropertyValue, Vec<Node>)>,
    default: Option<Vec<Node>>,
) -> Node {
    let mut ports: Vec<Port> = tests
        .into_iter()
        .map(|(op, c, children)| {
            port(
                PortType::WHEN_TEST,
                PortPayload::WhenTest(Box::new(WhenTest {
                    constant: Some(PropertyEntry {
                        pid: property,
                        value: c,
                    }),
                    rhs_property: 0,
                    operator: op,
                    tail: [0, 0],
                })),
                children,
            )
        })
        .collect();
    if let Some(d) = default {
        ports.push(default_port(PortPayload::None, d));
    }
    node(
        NodeKind::WHEN,
        NodeData::Switch(SwitchNode {
            property,
            persistent,
            autonomous: true,
            synchronized: false,
        }),
        ports,
    )
}

/// A Sound node: `faf` fire-and-forget, ramp-out, channel.
pub fn sound(did: u32, faf: bool, ramp_out: f32, channel: u32) -> Node {
    node(
        NodeKind::SOUND,
        NodeData::Sound(Box::new(SoundNode {
            sound: Did(did),
            fire_and_forget: faf,
            channel,
            ramp_out_time: ramp_out,
            ..SoundNode::default()
        })),
        Vec::new(),
    )
}

pub fn set_property(property: u32, value: PropertyValue) -> Node {
    node(
        NodeKind::SET_PROPERTY,
        NodeData::SetProperty {
            property,
            value: Some(Box::new(PropertyEntry {
                pid: property,
                value,
            })),
            server_only: false,
        },
        Vec::new(),
    )
}

pub fn label(name: &str, children: Vec<Node>) -> Node {
    node(
        NodeKind::LABEL,
        NodeData::Label { name: name.into() },
        vec![default_port(PortPayload::U32(1), children)],
    )
}

pub fn goto(name: &str) -> Node {
    node(
        NodeKind::GOTO,
        NodeData::Goto { name: name.into() },
        Vec::new(),
    )
}

pub fn run(key: u32, persistent: bool, stop: bool) -> Node {
    node(
        NodeKind::RUN,
        NodeData::Run(Box::new(RunNode {
            version: 1,
            parameters: Vec::new(),
            script: key,
            persistent,
            unbind_on_death: false,
            stop,
            expose_as_script_link: false,
            queuing_type: 3,
        })),
        Vec::new(),
    )
}

/// A Run node targeting the player (target type 4).
pub fn run_on_player(key: u32, persistent: bool) -> Node {
    let mut n = run(key, persistent, false);
    n.targets.push(Target {
        port_type: PortType::PARAMETER,
        value: tessera_formats::script_table::TargetValue::U32(0),
        target_type: 4,
        data: TargetData::None,
        sub_targets: Vec::new(),
    });
    n
}

pub fn when_running(
    keys: Vec<u32>,
    persistent: bool,
    running: Vec<Node>,
    not_running: Vec<Node>,
) -> Node {
    node(
        NodeKind::WHEN_RUNNING,
        NodeData::WhenRunning {
            scripts: keys,
            names: Vec::new(),
            persistent,
        },
        vec![
            port(PortType::RUNNING, PortPayload::None, running),
            port(PortType::NOT_RUNNING, PortPayload::None, not_running),
        ],
    )
}

pub fn on_stop(children: Vec<Node>) -> Node {
    node(
        NodeKind::ON_STOP,
        NodeData::None,
        vec![default_port(PortPayload::None, children)],
    )
}

pub fn scriptlet(
    file: u32,
    params: Vec<(u32, PropertyValue)>,
    bindings: Vec<(&str, Vec<Node>)>,
) -> Node {
    let entries: Vec<PropertyEntry> = params
        .into_iter()
        .map(|(pid, value)| PropertyEntry { pid, value })
        .collect();
    node(
        NodeKind::SCRIPTLET,
        NodeData::Scriptlet {
            file: Did(file),
            parameters: PropertySet {
                size: Default::default(),
                entries,
            },
        },
        bindings
            .into_iter()
            .map(|(n, c)| default_port(PortPayload::Name(n.into()), c))
            .collect(),
    )
}

pub fn parameter(name: &str) -> Node {
    node(
        NodeKind::SCRIPTLET_PARAMETER,
        NodeData::ScriptletParameter { name: name.into() },
        vec![default_port(
            PortPayload::None,
            vec![scriptlet(0, Vec::new(), Vec::new())],
        )],
    )
}

pub fn environment(name: u32, priority: i32) -> Node {
    node(
        NodeKind::SOUND_ENVIRONMENT,
        NodeData::SoundEnvironment { name, priority },
        Vec::new(),
    )
}

/// Drives `field` from `source`.
pub fn driven(mut n: Node, field: u32, source: u32) -> Node {
    n.base.driven.push((field, source));
    n
}

pub fn with_probability(mut n: Node, p: f32) -> Node {
    n.base.probability = p;
    n
}

pub fn with_intensity(mut n: Node, multiplier: f32, variance: f32, upi: bool) -> Node {
    n.base.intensity_multiplier = multiplier;
    n.base.intensity_variance = variance;
    n.base.use_parent_intensity = upi;
    n
}

/// A table of `kind` with scripts (key → roots).
pub fn table(kind: u32, includes: Vec<u32>, scripts: Vec<(u32, Vec<Node>)>) -> ScriptTable {
    ScriptTable {
        did: Did(0),
        kind,
        includes: includes.into_iter().map(Did).collect(),
        scripts_size: Default::default(),
        scripts: scripts
            .into_iter()
            .map(|(key, roots)| Script { key, roots })
            .collect(),
        properties_size: Default::default(),
        script_properties: Vec::new(),
        dids_size: Default::default(),
        script_dids: Vec::new(),
    }
}

/// Adds a per-script property to a table.
pub fn script_property(t: &mut ScriptTable, key: u32, pid: u32, value: PropertyValue) {
    match t.script_properties.iter_mut().find(|(k, _)| *k == key) {
        Some((_, set)) => set.entries.push(PropertyEntry { pid, value }),
        None => t.script_properties.push((
            key,
            PropertySet {
                size: Default::default(),
                entries: vec![PropertyEntry { pid, value }],
            },
        )),
    }
}

pub fn scriptlet_record(roots: Vec<Node>) -> Scriptlet {
    Scriptlet {
        did: Did(0),
        roots,
        dependencies: Vec::new(),
    }
}

/// The table DID used by [`single`].
pub const TABLE: u32 = 0x0700_0001;

/// A runtime over one table (kind 2) with the given scripts.
pub fn single(scripts: Vec<(u32, Vec<Node>)>) -> Runtime<'static> {
    let mut src = MemorySource::new();
    src.insert_table(Did(TABLE), table(2, Vec::new(), scripts));
    Runtime::new(src, catalog(), vocab()).with_seed(7)
}

/// Spawn options for an entity with the [`TABLE`] and properties.
pub fn init(props: Vec<(u32, PropertyValue)>) -> EntityInit {
    let mut p = PropertyState::new();
    for (k, v) in props {
        p.set(k, v);
    }
    EntityInit {
        tables: vec![Did(TABLE)],
        startup_scripts: Vec::new(),
        properties: p,
        parent: None,
        run_startup: true,
    }
}

pub const E: EntityId = EntityId(1);

/// Advances in 60 Hz frames until `until` (scheduler time).
pub fn run_until(rt: &mut Runtime<'_>, sink: &mut RecordingSink, until: f64) {
    let step = 1.0 / 60.0;
    while rt.time() + 1e-9 < until {
        let dt = (until - rt.time()).min(step);
        rt.advance(sink, dt);
    }
}

/// Handles of tracked sounds playing, by sound DID.
pub fn playing(sink: &RecordingSink, did: u32) -> Vec<SoundHandle> {
    sink.playing
        .iter()
        .filter(|(_, s)| s.sound == Did(did))
        .map(|(h, _)| *h)
        .collect()
}

/// Completes a voice in the sink and the runtime.
pub fn complete(rt: &mut Runtime<'_>, sink: &mut RecordingSink, h: SoundHandle) {
    sink.complete(h);
    assert!(
        rt.sound_completed(sink, h),
        "handle {h:?} belongs to a node"
    );
}
