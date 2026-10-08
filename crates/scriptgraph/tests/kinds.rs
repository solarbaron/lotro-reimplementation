// SPDX-License-Identifier: GPL-3.0-or-later
//! Control-flow kinds, run lookup, entity removal and guards (`script-runtime.md` §3–§21).

mod common;

use common::*;
use tessera_formats::script_table::when_op;
use tessera_formats::{Did, PropertyValue};
use tessera_scriptgraph::{
    EntityId, MemorySource, RecordingSink, Runtime, RuntimeIssue, RuntimeLimits, ScriptStart,
    SinkEvent,
};

const KEY: u32 = 0x1000_0001;
const OTHER_KEY: u32 = 0x1000_0002;
const SND: u32 = 0x2A00_0001;
const SND2: u32 = 0x2A00_0002;

fn spawn(rt: &mut Runtime<'_>, sink: &mut RecordingSink) {
    rt.spawn_entity(sink, E, init(Vec::new())).unwrap();
}

#[test]
fn startup_runs_on_spawn_and_teardown_stops_everything() {
    let roots = vec![
        sound(SND, false, 3.0, 7),
        environment(8, 5),
        on_stop(vec![set_property(pid::FLAG, PropertyValue::Bool(true))]),
    ];
    let mut rt = single(vec![(STARTUP, roots)]);
    let mut sink = RecordingSink::new();
    spawn(&mut rt, &mut sink);
    assert_eq!(rt.running_scripts(E), vec![STARTUP]);
    assert_eq!(sink.playing.len(), 1);
    assert_eq!(sink.environments.len(), 1);
    let env = *sink.environments.values().next().unwrap();
    assert_eq!((env.name, env.priority, env.intensity), (8, 5, 1.0));
    rt.despawn(&mut sink, E).unwrap();
    assert!(sink.playing.is_empty(), "tracked sound stopped");
    assert!(sink.environments.is_empty(), "environment unregistered");
    assert!(sink.events.contains(&SinkEvent::EntityRemoved(E)));
    assert!(!rt.has_entity(E));
    assert_eq!(rt.stats().instances, 0);
    assert!(rt.despawn(&mut sink, E).is_err());
}

#[test]
fn on_stop_runs_on_an_ordinary_stop_only() {
    let roots = vec![on_stop(vec![
        set_property(pid::FLAG, PropertyValue::Bool(true)),
        sound(SND, false, 0.0, 7),
    ])];
    let mut rt = single(vec![(KEY, roots)]);
    let mut sink = RecordingSink::new();
    spawn(&mut rt, &mut sink);
    assert_eq!(
        rt.start_script(&mut sink, E, KEY).unwrap(),
        ScriptStart::Running
    );
    assert_eq!(rt.stop_script(&mut sink, E, KEY).unwrap(), 1);
    assert_eq!(rt.property(E, pid::FLAG), Some(PropertyValue::Bool(true)));
    // The tracked sound started in the stop and was stopped at once.
    assert_eq!(sink.sounds_started, 1);
    assert!(sink.playing.is_empty());
}

#[test]
fn collection_starts_ports_by_ordinal_and_completes_with_its_children() {
    let c = tessera_formats::script_table::Node {
        ports: vec![
            default_port(
                tessera_formats::script_table::PortPayload::U32(2),
                vec![sound(SND2, true, 0.0, 0)],
            ),
            default_port(
                tessera_formats::script_table::PortPayload::U32(1),
                vec![sound(SND, false, 0.0, 0)],
            ),
        ],
        ..collection(Vec::new())
    };
    let mut rt = single(vec![(KEY, vec![c])]);
    let mut sink = RecordingSink::new();
    spawn(&mut rt, &mut sink);
    rt.start_script(&mut sink, E, KEY).unwrap();
    let order: Vec<u32> = sink.sound_starts().map(|(_, s)| s.sound.0).collect();
    assert_eq!(order, vec![SND, SND2], "ascending ordinal");
    assert!(rt.is_running(E, KEY));
    let h = playing(&sink, SND)[0];
    complete(&mut rt, &mut sink, h);
    assert!(!rt.is_running(E, KEY), "completion propagates to the run");
}

#[test]
fn goto_calls_its_label_and_stops_nothing() {
    let roots = vec![collection(vec![
        label("silence", vec![sound(SND, false, 0.0, 0)]),
        goto("silence"),
        goto("nowhere"),
    ])];
    let mut rt = single(vec![(KEY, roots)]);
    let mut sink = RecordingSink::new();
    spawn(&mut rt, &mut sink);
    rt.start_script(&mut sink, E, KEY).unwrap();
    // The Label in place and the Goto's call each start the sound: two voices.
    assert_eq!(playing(&sink, SND).len(), 2);
    assert!(
        rt.issues()
            .contains(&RuntimeIssue::LabelNotFound("nowhere".into()))
    );
}

#[test]
fn self_reaching_goto_is_bounded() {
    // A Label whose subtree reaches a Goto of its own name recurses synchronously in the
    // original (§21); the nesting guard ends it.
    let roots = vec![label("loop", vec![goto("loop")])];
    let mut rt = single(vec![(KEY, roots)]);
    let mut sink = RecordingSink::new();
    spawn(&mut rt, &mut sink);
    rt.start_script(&mut sink, E, KEY).unwrap();
    assert!(
        rt.issues()
            .iter()
            .any(|i| matches!(i, RuntimeIssue::Limit(_))),
        "{:?}",
        rt.issues()
    );
    rt.despawn(&mut sink, E).unwrap();
    assert_eq!(rt.stats().instances, 0);
}

#[test]
fn scriptlet_parameters_bind_by_name_ignoring_case() {
    const TEMPLATE: u32 = 0x0C00_0001;
    let mut src = MemorySource::new();
    src.insert_scriptlet(
        Did(TEMPLATE),
        scriptlet_record(vec![collection(vec![parameter("Walk"), parameter("Run")])]),
    );
    let call = scriptlet(
        TEMPLATE,
        Vec::new(),
        vec![("WALK", vec![sound(SND, true, 0.0, 0)])],
    );
    src.insert_table(Did(TABLE), table(2, Vec::new(), vec![(KEY, vec![call])]));
    let mut rt = Runtime::new(src, catalog(), vocab());
    let mut sink = RecordingSink::new();
    spawn(&mut rt, &mut sink);
    rt.start_script(&mut sink, E, KEY).unwrap();
    let started: Vec<u32> = sink.sound_starts().map(|(_, s)| s.sound.0).collect();
    assert_eq!(
        started,
        vec![SND],
        "Walk binds to WALK; Run is unbound and runs nothing"
    );
}

#[test]
fn nested_templates_bind_lexically() {
    // Outer template O holds a parameter "x" and invokes inner template I, binding I's
    // parameter "y" to a node list that itself holds a ScriptletParameter "x": that one
    // belongs to O's record and binds to the caller of O.
    const OUTER: u32 = 0x0C00_0010;
    const INNER: u32 = 0x0C00_0011;
    let mut src = MemorySource::new();
    src.insert_scriptlet(Did(INNER), scriptlet_record(vec![parameter("y")]));
    src.insert_scriptlet(
        Did(OUTER),
        scriptlet_record(vec![scriptlet(
            INNER,
            Vec::new(),
            vec![("y", vec![parameter("x")])],
        )]),
    );
    let call = scriptlet(
        OUTER,
        Vec::new(),
        vec![("x", vec![sound(SND, true, 0.0, 0)])],
    );
    src.insert_table(Did(TABLE), table(2, Vec::new(), vec![(KEY, vec![call])]));
    let mut rt = Runtime::new(src, catalog(), vocab());
    let mut sink = RecordingSink::new();
    spawn(&mut rt, &mut sink);
    rt.start_script(&mut sink, E, KEY).unwrap();
    assert_eq!(sink.sounds_started, 1);
}

#[test]
fn scriptlet_parameters_reach_script_parameter_reads_and_driven_pairs() {
    const TEMPLATE: u32 = 0x0C00_0020;
    let mut inner_switch = switch(
        pid::PARAM,
        false,
        vec![case(
            vec![PropertyValue::Int(3)],
            0.0,
            0.0,
            vec![sound(SND, true, 0.0, 0)],
        )],
    );
    inner_switch.base.node_id = 900;
    // A Sound whose SoundID is scriptlet-driven from parameter OTHER.
    let mut s = sound(0, true, 0.0, 0);
    s.base.scriptlet_driven.push((pid::SOUND_ID, pid::OTHER));
    let mut src = MemorySource::new();
    src.insert_scriptlet(Did(TEMPLATE), scriptlet_record(vec![inner_switch, s]));
    let call = scriptlet(
        TEMPLATE,
        vec![
            (pid::PARAM, PropertyValue::Int(3)),
            (pid::OTHER, PropertyValue::DidRef(Did(SND2))),
        ],
        Vec::new(),
    );
    src.insert_table(Did(TABLE), table(2, Vec::new(), vec![(KEY, vec![call])]));
    let mut rt = Runtime::new(src, catalog(), vocab());
    let mut sink = RecordingSink::new();
    spawn(&mut rt, &mut sink);
    rt.start_script(&mut sink, E, KEY).unwrap();
    let started: Vec<u32> = sink.sound_starts().map(|(_, s)| s.sound.0).collect();
    assert_eq!(started, vec![SND, SND2]);
}

#[test]
fn persistent_run_lives_with_its_branch() {
    // A case's persistent Run keeps the bed alive exactly as long as the case (§12.4).
    let bed = vec![sound(SND, false, 3.0, 7)];
    let sw = switch(
        pid::VALUE,
        true,
        vec![
            case(
                vec![PropertyValue::Int(1)],
                0.0,
                0.0,
                vec![run(OTHER_KEY, true, false)],
            ),
            case(vec![PropertyValue::Int(2)], 0.0, 0.0, Vec::new()),
        ],
    );
    let mut rt = single(vec![(KEY, vec![sw]), (OTHER_KEY, bed)]);
    let mut sink = RecordingSink::new();
    let mut i = init(vec![(pid::VALUE, PropertyValue::Int(1))]);
    i.startup_scripts.push(KEY);
    rt.spawn_entity(&mut sink, E, i).unwrap();
    assert!(rt.is_running(E, OTHER_KEY));
    // The bed's sound completing ends its run, but the Run node stays running.
    let h = playing(&sink, SND)[0];
    complete(&mut rt, &mut sink, h);
    assert!(!rt.is_running(E, OTHER_KEY));
    // Restart the case: the bed runs again; then switch away: it stops.
    rt.set_property(&mut sink, E, pid::VALUE, PropertyValue::Int(2))
        .unwrap();
    rt.set_property(&mut sink, E, pid::VALUE, PropertyValue::Int(1))
        .unwrap();
    assert!(rt.is_running(E, OTHER_KEY));
    rt.set_property(&mut sink, E, pid::VALUE, PropertyValue::Int(2))
        .unwrap();
    assert!(!rt.is_running(E, OTHER_KEY));
    assert!(sink.playing.is_empty());
}

#[test]
fn run_stop_and_instance_limit() {
    let mut src = MemorySource::new();
    let mut t = table(
        2,
        Vec::new(),
        vec![
            (KEY, vec![sound(SND, false, 0.0, 6)]),
            (OTHER_KEY, vec![run(KEY, false, true)]),
        ],
    );
    script_property(&mut t, KEY, pid::MAX_INSTANCES, PropertyValue::Int(2));
    src.insert_table(Did(TABLE), t);
    let mut rt = Runtime::new(src, catalog(), vocab());
    let mut sink = RecordingSink::new();
    spawn(&mut rt, &mut sink);
    assert_eq!(
        rt.start_script(&mut sink, E, KEY).unwrap(),
        ScriptStart::Running
    );
    assert_eq!(
        rt.start_script(&mut sink, E, KEY).unwrap(),
        ScriptStart::Running
    );
    assert_eq!(
        rt.start_script(&mut sink, E, KEY).unwrap(),
        ScriptStart::LimitReached,
        "Script_MaxNumInstances 2"
    );
    assert_eq!(sink.playing.len(), 2);
    // A Run with Stop = 1 stops every run of the key.
    assert_eq!(
        rt.start_script(&mut sink, E, OTHER_KEY).unwrap(),
        ScriptStart::Completed
    );
    assert!(!rt.is_running(E, KEY));
    assert!(sink.playing.is_empty());
    assert_eq!(
        rt.start_script(&mut sink, E, 0x1000_0999).unwrap(),
        ScriptStart::NotDefined
    );
}

#[test]
fn when_running_follows_runs_of_watched_keys() {
    let wr = when_running(
        vec![OTHER_KEY],
        true,
        vec![set_property(pid::VALUE, PropertyValue::Int(1))],
        vec![set_property(pid::VALUE, PropertyValue::Int(0))],
    );
    let other = vec![sound(SND, false, 0.0, 0)];
    let mut rt = single(vec![(KEY, vec![wr]), (OTHER_KEY, other)]);
    let mut sink = RecordingSink::new();
    spawn(&mut rt, &mut sink);
    rt.start_script(&mut sink, E, KEY).unwrap();
    assert_eq!(rt.property(E, pid::VALUE), Some(PropertyValue::Int(0)));
    rt.start_script(&mut sink, E, OTHER_KEY).unwrap();
    assert_eq!(rt.property(E, pid::VALUE), Some(PropertyValue::Int(1)));
    rt.stop_script(&mut sink, E, OTHER_KEY).unwrap();
    assert_eq!(rt.property(E, pid::VALUE), Some(PropertyValue::Int(0)));
    assert!(
        rt.is_running(E, KEY),
        "persistent WhenRunning never completes"
    );
}

#[test]
fn when_takes_the_first_holding_test() {
    let w = when(
        pid::VALUE,
        true,
        vec![
            (
                when_op::GREATER_THAN_OR_EQUAL,
                PropertyValue::Int(5),
                vec![sound(SND, false, 0.0, 0)],
            ),
            (
                when_op::GREATER_THAN,
                PropertyValue::Int(1),
                vec![sound(SND2, false, 0.0, 0)],
            ),
        ],
        Some(vec![set_property(pid::FLAG, PropertyValue::Bool(true))]),
    );
    let mut rt = single(vec![(KEY, vec![w])]);
    let mut sink = RecordingSink::new();
    rt.spawn_entity(
        &mut sink,
        E,
        init(vec![(pid::VALUE, PropertyValue::Int(7))]),
    )
    .unwrap();
    rt.start_script(&mut sink, E, KEY).unwrap();
    assert_eq!(playing(&sink, SND).len(), 1);
    assert!(
        playing(&sink, SND2).is_empty(),
        "only the first holding test runs"
    );
    rt.set_property(&mut sink, E, pid::VALUE, PropertyValue::Int(3))
        .unwrap();
    assert!(playing(&sink, SND).is_empty());
    assert_eq!(playing(&sink, SND2).len(), 1);
    rt.set_property(&mut sink, E, pid::VALUE, PropertyValue::Int(0))
        .unwrap();
    assert!(sink.playing.is_empty());
    assert_eq!(rt.property(E, pid::FLAG), Some(PropertyValue::Bool(true)));
}

#[test]
fn switch_on_an_unset_property_matches_its_default_or_zero_value() {
    // §6.2 (R27): an unset property starts from its dictionary default, else its type's zero
    // value, and goes through the normal case match, with the first Default port when no
    // case matches. The example of §6.2: an enum without a default whose cases never hold 0.
    let snd = |n: u32| sound(SND + 0x10 + n, false, 0.0, 0);
    let enum_case = |v: u32, n: u32| case(vec![PropertyValue::Enum(v)], 0.0, 0.0, vec![snd(n)]);
    // A: cases 3 and 4 and a Default port: the Default port.
    let a = switch(
        pid::MONITOR,
        true,
        vec![
            enum_case(3, 0),
            enum_case(4, 1),
            switch_default(vec![snd(2)]),
        ],
    );
    // B: case 3 and no Default port: nothing.
    let b = switch(pid::MONITOR, true, vec![enum_case(3, 3)]);
    // C: a case for 0 before the Default port: the case for the zero value.
    let c = switch(
        pid::MONITOR,
        false,
        vec![switch_default(vec![snd(4)]), enum_case(0, 5)],
    );
    // D: a dictionary default (MUSIC_VOLUME 1.0) is matched like a value.
    let d = switch(
        pid::MUSIC_VOLUME,
        false,
        vec![
            case(vec![PropertyValue::Float(1.0)], 0.0, 0.0, vec![snd(6)]),
            switch_default(vec![snd(7)]),
        ],
    );
    // E: a tri-state reads 255, "undefined".
    let e = switch(
        pid::TRI,
        false,
        vec![
            case(vec![PropertyValue::TriState(1)], 0.0, 0.0, vec![snd(8)]),
            case(vec![PropertyValue::TriState(255)], 0.0, 0.0, vec![snd(9)]),
        ],
    );
    let mut rt = single(vec![(KEY, vec![a, b, c, d, e])]);
    let mut sink = RecordingSink::new();
    spawn(&mut rt, &mut sink);
    rt.start_script(&mut sink, E, KEY).unwrap();
    let on = |sink: &RecordingSink, n: u32| playing(sink, SND + 0x10 + n).len();
    let started: Vec<u32> = (0..10).filter(|&n| on(&sink, n) == 1).collect();
    assert_eq!(started, vec![2, 5, 6, 9]);
    // A value with a case: A and B take it.
    rt.set_property(&mut sink, E, pid::MONITOR, PropertyValue::Enum(3))
        .unwrap();
    let started: Vec<u32> = (0..10).filter(|&n| on(&sink, n) == 1).collect();
    assert_eq!(started, vec![0, 3, 5, 6, 9]);
    // Unset again: back to the zero value (A's Default port; B selects nothing).
    rt.clear_property(&mut sink, E, pid::MONITOR).unwrap();
    let started: Vec<u32> = (0..10).filter(|&n| on(&sink, n) == 1).collect();
    assert_eq!(started, vec![2, 5, 6, 9]);
}

#[test]
fn when_on_an_unset_property_tests_its_zero_value() {
    // §7: a When prepares its value like a Switch (§6.2): VALUE (int, no default) reads 0.
    let w = when(
        pid::VALUE,
        true,
        vec![
            (
                when_op::GREATER_THAN,
                PropertyValue::Int(0),
                vec![sound(SND, false, 0.0, 0)],
            ),
            (
                when_op::LESS_THAN,
                PropertyValue::Int(1),
                vec![sound(SND2, false, 0.0, 0)],
            ),
        ],
        None,
    );
    let mut rt = single(vec![(KEY, vec![w])]);
    let mut sink = RecordingSink::new();
    spawn(&mut rt, &mut sink);
    rt.start_script(&mut sink, E, KEY).unwrap();
    assert!(playing(&sink, SND).is_empty());
    assert_eq!(playing(&sink, SND2).len(), 1, "0 < 1 holds");
    rt.set_property(&mut sink, E, pid::VALUE, PropertyValue::Int(5))
        .unwrap();
    assert_eq!(playing(&sink, SND).len(), 1);
    assert!(playing(&sink, SND2).is_empty());
}

#[test]
fn switch_on_an_undefined_property_selects_nothing() {
    // §6.2, §6.3: "no value" only for a property ID without a dictionary definition: no
    // port, not even Default. A When on such a property selects nothing either (Tessera
    // choice, `script-runtime.md` §29).
    let sw = switch(
        pid::UNDEFINED,
        true,
        vec![
            case(
                vec![PropertyValue::Int(0)],
                0.0,
                0.0,
                vec![sound(SND, false, 0.0, 0)],
            ),
            switch_default(vec![sound(SND, false, 0.0, 0)]),
        ],
    );
    let w = when(
        pid::UNDEFINED,
        false,
        vec![(
            when_op::EQUAL,
            PropertyValue::Int(0),
            vec![sound(SND2, false, 0.0, 0)],
        )],
        Some(vec![sound(SND2, false, 0.0, 0)]),
    );
    let mut rt = single(vec![(KEY, vec![sw, w])]);
    let mut sink = RecordingSink::new();
    spawn(&mut rt, &mut sink);
    rt.start_script(&mut sink, E, KEY).unwrap();
    assert_eq!(sink.sounds_started, 0);
    // A value set on the entity is read even without a definition.
    rt.set_property(&mut sink, E, pid::UNDEFINED, PropertyValue::Int(9))
        .unwrap();
    assert_eq!(
        playing(&sink, SND).len(),
        1,
        "a value without a case: Default"
    );
}

#[test]
fn case_int_list_properties_without_a_value_are_skipped_with_no_default() {
    // §6.2, §6.3 step 1: LIST has the dictionary default 2 but no value on the entity, so
    // the case does not match the Switch value 2 through it; once set, it does. (LIST is
    // set to 3 and VALUE follows: setting LIST to its default value 2 is no change of the
    // value the entity reads, so it notifies nothing, §29 item 5.)
    let mut c = case(Vec::new(), 0.0, 0.0, vec![sound(SND, false, 0.0, 0)]);
    if let tessera_formats::script_table::PortPayload::SwitchCase(sc) = &mut c.payload {
        sc.ints = vec![pid::LIST as i32];
    }
    let sw = switch(
        pid::VALUE,
        true,
        vec![c, switch_default(vec![sound(SND2, false, 0.0, 0)])],
    );
    let mut rt = single(vec![(KEY, vec![sw])]);
    let mut sink = RecordingSink::new();
    rt.spawn_entity(
        &mut sink,
        E,
        init(vec![(pid::VALUE, PropertyValue::Int(2))]),
    )
    .unwrap();
    rt.start_script(&mut sink, E, KEY).unwrap();
    assert!(playing(&sink, SND).is_empty());
    assert_eq!(playing(&sink, SND2).len(), 1);
    rt.set_property(&mut sink, E, pid::LIST, PropertyValue::Int(3))
        .unwrap();
    assert!(playing(&sink, SND).is_empty());
    rt.set_property(&mut sink, E, pid::VALUE, PropertyValue::Int(3))
        .unwrap();
    assert_eq!(
        playing(&sink, SND).len(),
        1,
        "the int-list value now matches"
    );
    assert!(playing(&sink, SND2).is_empty());
}

#[test]
fn the_same_node_rule_comes_before_the_probability_draw() {
    // §3.1 (R1): steps in the order 3, 4, 1, 2, 5, 6, 7. A looping Timer fires its port every
    // 0.1 s; the child is a tracked Sound with Probability 0.5. Once it plays, every further
    // start is skipped by the same-node rule (step 3) before any draw (step 2), so the
    // probability-skip count stops growing.
    let t = timer(
        true,
        0.1,
        vec![(0.0, vec![with_probability(sound(SND, false, 0.0, 0), 0.5)])],
    );
    let mut rt = single(vec![(KEY, vec![t])]);
    let mut sink = RecordingSink::new();
    spawn(&mut rt, &mut sink);
    rt.start_script(&mut sink, E, KEY).unwrap();
    let mut guard = 0;
    while playing(&sink, SND).is_empty() {
        rt.advance(&mut sink, 0.1);
        guard += 1;
        assert!(guard < 1000, "the sound starts eventually");
    }
    let skips = rt.stats().probability_skips;
    let starts = rt.stats().node_starts;
    for _ in 0..50 {
        rt.advance(&mut sink, 0.1);
    }
    assert_eq!(playing(&sink, SND).len(), 1, "one copy (RI1)");
    assert!(rt.stats().node_starts > starts, "the Timer kept firing");
    assert_eq!(
        rt.stats().probability_skips,
        skips,
        "no draw for a start the same-node rule skips"
    );
}

#[test]
fn transition_ports_run_before_the_case_switch() {
    // The music graph's transitions: Wait 1, RampIn 0, children that complete at once
    // (§6.9): the SetProperty runs, then the case switch follows in the same call.
    let sw = switch(
        pid::VALUE,
        true,
        vec![
            case(
                vec![PropertyValue::Int(1)],
                0.0,
                0.0,
                vec![sound(SND, false, 0.0, 0)],
            ),
            case(
                vec![PropertyValue::Int(2)],
                0.0,
                0.0,
                vec![sound(SND2, false, 0.0, 0)],
            ),
            transition(
                Some(PropertyValue::Int(1)),
                Some(PropertyValue::Int(2)),
                true,
                vec![set_property(pid::COUNTER, PropertyValue::Int(12))],
            ),
            transition(
                None,
                None,
                true,
                vec![set_property(pid::COUNTER, PropertyValue::Int(99))],
            ),
        ],
    );
    let mut rt = single(vec![(KEY, vec![sw])]);
    let mut sink = RecordingSink::new();
    rt.spawn_entity(
        &mut sink,
        E,
        init(vec![(pid::VALUE, PropertyValue::Int(1))]),
    )
    .unwrap();
    rt.start_script(&mut sink, E, KEY).unwrap();
    rt.set_property(&mut sink, E, pid::VALUE, PropertyValue::Int(2))
        .unwrap();
    assert_eq!(
        rt.property(E, pid::COUNTER),
        Some(PropertyValue::Int(12)),
        "tier 1"
    );
    assert_eq!(playing(&sink, SND2).len(), 1);
    assert!(playing(&sink, SND).is_empty());
    rt.set_property(&mut sink, E, pid::VALUE, PropertyValue::Int(1))
        .unwrap();
    assert_eq!(
        rt.property(E, pid::COUNTER),
        Some(PropertyValue::Int(99)),
        "tier 4"
    );
    assert_eq!(playing(&sink, SND).len(), 1);
}

#[test]
fn a_waiting_transition_defers_the_case() {
    // Wait 1 with a running child: the case starts when the transition's children finish.
    let sw = switch(
        pid::VALUE,
        true,
        vec![
            case(
                vec![PropertyValue::Int(1)],
                0.0,
                0.0,
                vec![sound(SND, false, 0.0, 0)],
            ),
            case(
                vec![PropertyValue::Int(2)],
                0.0,
                0.0,
                vec![sound(SND2, false, 0.0, 0)],
            ),
            transition(None, None, true, vec![timer(false, 1.0, Vec::new())]),
        ],
    );
    let mut rt = single(vec![(KEY, vec![sw])]);
    let mut sink = RecordingSink::new();
    rt.spawn_entity(
        &mut sink,
        E,
        init(vec![(pid::VALUE, PropertyValue::Int(1))]),
    )
    .unwrap();
    rt.start_script(&mut sink, E, KEY).unwrap();
    rt.set_property(&mut sink, E, pid::VALUE, PropertyValue::Int(2))
        .unwrap();
    assert!(playing(&sink, SND2).is_empty(), "deferred");
    assert!(
        sink.events
            .iter()
            .any(|e| matches!(e, SinkEvent::SoundVolume(_, v) if *v == 0.0))
    );
    run_until(&mut rt, &mut sink, 0.5);
    assert!(
        playing(&sink, SND).is_empty(),
        "the outgoing branch stopped at the next pass"
    );
    run_until(&mut rt, &mut sink, 1.01);
    assert_eq!(playing(&sink, SND2).len(), 1);
}

#[test]
fn for_current_value_runs_on_every_new_value() {
    let fcv = port(
        tessera_formats::script_table::PortType::FOR_CURRENT_VALUE,
        tessera_formats::script_table::PortPayload::None,
        vec![sound(SND, true, 0.0, 0)],
    );
    let sw = switch(pid::VALUE, true, vec![fcv]);
    let mut rt = single(vec![(KEY, vec![sw])]);
    let mut sink = RecordingSink::new();
    rt.spawn_entity(
        &mut sink,
        E,
        init(vec![(pid::VALUE, PropertyValue::Int(1))]),
    )
    .unwrap();
    rt.start_script(&mut sink, E, KEY).unwrap();
    rt.set_property(&mut sink, E, pid::VALUE, PropertyValue::Int(2))
        .unwrap();
    rt.set_property(&mut sink, E, pid::VALUE, PropertyValue::Int(2))
        .unwrap();
    rt.set_property(&mut sink, E, pid::VALUE, PropertyValue::Int(3))
        .unwrap();
    assert_eq!(sink.sounds_started, 3);
}

#[test]
fn probability_zero_never_runs_and_intensity_is_unclamped_without_variance() {
    let roots = vec![
        with_probability(sound(SND, true, 0.0, 0), 0.0),
        collection(vec![with_intensity(
            collection(vec![sound(SND2, false, 0.0, 0)]),
            400_000.0,
            0.0,
            true,
        )]),
    ];
    let mut rt = single(vec![(KEY, roots)]);
    let mut sink = RecordingSink::new();
    spawn(&mut rt, &mut sink);
    for _ in 0..50 {
        rt.start_script(&mut sink, E, KEY).unwrap();
        rt.stop_script(&mut sink, E, KEY).unwrap();
    }
    assert!(sink.sound_starts().all(|(_, s)| s.sound != Did(SND)));
    // The voice volume is clamped to 1 (sound.md §9.2 step 7).
    assert!(sink.sound_starts().all(|(_, s)| s.volume == 1.0));
}

#[test]
fn same_node_is_not_started_twice_under_one_parent() {
    // A looping Timer (cycle 0.1 s) firing a tracked sound that keeps playing: one voice.
    let t = timer(true, 0.0, vec![(0.0, vec![sound(SND, false, 0.0, 0)])]);
    let mut rt = single(vec![(KEY, vec![t])]);
    let mut sink = RecordingSink::new();
    spawn(&mut rt, &mut sink);
    rt.start_script(&mut sink, E, KEY).unwrap();
    run_until(&mut rt, &mut sink, 2.0);
    assert_eq!(sink.sounds_started, 1);
    let h = playing(&sink, SND)[0];
    complete(&mut rt, &mut sink, h);
    run_until(&mut rt, &mut sink, 2.2);
    assert_eq!(
        sink.sounds_started, 2,
        "a finished voice is started by the next cycle"
    );
}

#[test]
fn lookup_runs_every_definition_by_table_kind_and_respects_overriding() {
    // Class table A (kind 2) includes B (kind 1) and C (kind 2); all define KEY. B's roots
    // run first (kind 1). Then A overrides OTHER_KEY: C's definition of it is not reached.
    const A: u32 = 0x0700_00A0;
    const B: u32 = 0x0700_00B0;
    const C: u32 = 0x0700_00C0;
    let mut src = MemorySource::new();
    let mut a = table(
        2,
        vec![B, C],
        vec![
            (KEY, vec![sound(0x2A00_00A0, true, 0.0, 0)]),
            (OTHER_KEY, vec![sound(0x2A00_00A1, true, 0.0, 0)]),
        ],
    );
    script_property(
        &mut a,
        OTHER_KEY,
        pid::COMBINATION,
        PropertyValue::Enum(OVERRIDING),
    );
    src.insert_table(Did(A), a);
    src.insert_table(
        Did(B),
        table(
            1,
            vec![C],
            vec![(KEY, vec![sound(0x2A00_00B0, true, 0.0, 0)])],
        ),
    );
    src.insert_table(
        Did(C),
        table(
            2,
            Vec::new(),
            vec![
                (KEY, vec![sound(0x2A00_00C0, true, 0.0, 0)]),
                (OTHER_KEY, vec![sound(0x2A00_00C1, true, 0.0, 0)]),
            ],
        ),
    );
    let mut rt = Runtime::new(src, catalog(), vocab());
    let mut sink = RecordingSink::new();
    let mut i = init(Vec::new());
    i.tables = vec![Did(A)];
    rt.spawn_entity(&mut sink, E, i).unwrap();
    rt.start_script(&mut sink, E, KEY).unwrap();
    rt.start_script(&mut sink, E, OTHER_KEY).unwrap();
    let order: Vec<u32> = sink.sound_starts().map(|(_, s)| s.sound.0).collect();
    assert_eq!(
        order,
        vec![0x2A00_00B0, 0x2A00_00A0, 0x2A00_00C0, 0x2A00_00A1],
        "kind 1 first, then kind 2 in pre-order (C once); Overriding stops the descent"
    );
}

#[test]
fn a_switch_setting_its_own_property_is_cut_by_the_reentry_guard() {
    let sw = switch(
        pid::VALUE,
        true,
        vec![
            case(
                vec![PropertyValue::Int(1)],
                0.0,
                0.0,
                vec![set_property(pid::VALUE, PropertyValue::Int(2))],
            ),
            case(
                vec![PropertyValue::Int(2)],
                0.0,
                0.0,
                vec![set_property(pid::VALUE, PropertyValue::Int(1))],
            ),
        ],
    );
    let mut rt = single(vec![(KEY, vec![sw])]);
    let mut sink = RecordingSink::new();
    rt.spawn_entity(
        &mut sink,
        E,
        init(vec![(pid::VALUE, PropertyValue::Int(0))]),
    )
    .unwrap();
    rt.start_script(&mut sink, E, KEY).unwrap();
    rt.set_property(&mut sink, E, pid::VALUE, PropertyValue::Int(1))
        .unwrap();
    assert!(
        rt.issues()
            .contains(&RuntimeIssue::PropertyReentry { pid: pid::VALUE })
    );
    assert!(rt.is_running(E, KEY));
}

#[test]
fn run_on_another_entity_is_deferred_to_the_next_pass() {
    const PLAYER: EntityId = EntityId(42);
    let mut rt = single(vec![
        (KEY, vec![run_on_player(OTHER_KEY, true)]),
        (OTHER_KEY, vec![sound(SND, false, 0.0, 0)]),
    ]);
    let mut sink = RecordingSink::new();
    spawn(&mut rt, &mut sink);
    rt.spawn_entity(&mut sink, PLAYER, init(Vec::new()))
        .unwrap();
    rt.set_player(Some(PLAYER));
    rt.start_script(&mut sink, E, KEY).unwrap();
    assert!(!rt.is_running(PLAYER, OTHER_KEY), "deferred");
    rt.advance(&mut sink, 0.0);
    assert!(rt.is_running(PLAYER, OTHER_KEY));
    // Stopping the persistent Run stops the script it started on the other entity.
    rt.stop_script(&mut sink, E, KEY).unwrap();
    assert!(!rt.is_running(PLAYER, OTHER_KEY));
    assert!(sink.playing.is_empty());
}

#[test]
fn limits_bound_hostile_graphs() {
    // Ten thousand children of one port, each a tracked sound: the per-entity bound holds.
    let many: Vec<_> = (0..10_000)
        .map(|i| sound(0x2A00_0000 + i, false, 0.0, 0))
        .collect();
    let mut rt = single(vec![(KEY, vec![collection(many)])]).with_limits(RuntimeLimits {
        max_instances_per_entity: 100,
        ..RuntimeLimits::default()
    });
    let mut sink = RecordingSink::new();
    spawn(&mut rt, &mut sink);
    rt.start_script(&mut sink, E, KEY).unwrap();
    assert!(rt.entity_instances(E).unwrap() <= 100);
    assert!(
        rt.issues()
            .contains(&RuntimeIssue::Limit("max_instances_per_entity"))
    );
}

#[test]
fn zero_length_timer_loops_are_cut() {
    let mut t = timer(false, 0.0, vec![(0.0, vec![sound(SND, true, 0.0, 0)])]);
    if let tessera_formats::NodeData::Timer(tn) = &mut t.data {
        tn.loop_count = -1;
    }
    let mut rt = single(vec![(KEY, vec![t])]);
    let mut sink = RecordingSink::new();
    spawn(&mut rt, &mut sink);
    rt.start_script(&mut sink, E, KEY).unwrap();
    rt.advance(&mut sink, 1.0);
    assert!(
        rt.issues()
            .contains(&RuntimeIssue::Limit("max_same_time_cycles"))
    );
    assert!(!rt.is_running(E, KEY));
}

#[test]
fn when_running_ping_pong_is_bounded() {
    // A persistent WhenRunning that stops the watched script while it runs and starts it
    // while it does not: a synchronous loop the original does not guard (§21). The stack
    // and nesting guards end it.
    let wr = when_running(
        vec![OTHER_KEY],
        true,
        vec![run(OTHER_KEY, false, true)],
        vec![run(OTHER_KEY, false, false)],
    );
    let mut rt = single(vec![
        (KEY, vec![wr]),
        (OTHER_KEY, vec![sound(SND, false, 0.0, 0)]),
    ]);
    let mut sink = RecordingSink::new();
    spawn(&mut rt, &mut sink);
    rt.start_script(&mut sink, E, KEY).unwrap();
    assert!(
        rt.issues()
            .iter()
            .any(|i| matches!(i, RuntimeIssue::Limit(_))),
        "{:?}",
        rt.issues()
    );
    rt.despawn(&mut sink, E).unwrap();
    assert_eq!(rt.stats().instances, 0);
    assert!(sink.playing.is_empty());
}
