// SPDX-License-Identifier: GPL-3.0-or-later
//! Regression tests for review findings: Switch re-entry, Timer and Run runaway guards,
//! teardown, entity reuse and API defaults (`script-runtime.md` §6, §11, §12, §19–§21).

mod common;

use common::*;
use tessera_formats::property::PropertyEntry;
use tessera_formats::script_table::{
    Node, NodeData, PortPayload, PortType, SwitchCase, Target, TargetData, TargetValue,
};
use tessera_formats::{Did, PropertyValue};
use tessera_scriptgraph::{
    EntityId, EntityInit, MAX_ADVANCE, MemorySource, RecordingSink, Runtime, RuntimeIssue,
    ScriptStart,
};

const KEY: u32 = 0x1000_0001;
const J1: u32 = 0x1000_0011;
const J2: u32 = 0x1000_0012;
const S1: u32 = 0x2A00_0001;
const S2: u32 = 0x2A00_0002;
const S3: u32 = 0x2A00_0003;
const PLAYER: EntityId = EntityId(42);

fn int(v: i32) -> PropertyValue {
    PropertyValue::Int(v)
}

fn with_player_target(mut n: Node) -> Node {
    n.targets.push(Target {
        port_type: PortType::PARAMETER,
        value: TargetValue::U32(0),
        target_type: 4,
        data: TargetData::None,
        sub_targets: Vec::new(),
    });
    n
}

fn timer_with(forever: bool, duration: f32, loop_count: i32, speed: f32) -> Node {
    let mut t = timer(forever, duration, Vec::new());
    if let NodeData::Timer(tn) = &mut t.data {
        tn.loop_count = loop_count;
        tn.speed = speed;
    }
    t
}

// ----- Switch: one branch at a time (§6.6, §7) -----

#[test]
fn on_stop_changing_the_switch_property_leaves_one_case_running() {
    let sw = switch(
        pid::VALUE,
        true,
        vec![
            case(
                vec![int(1)],
                0.0,
                0.0,
                vec![on_stop(vec![set_property(pid::VALUE, int(3))])],
            ),
            case(vec![int(2)], 0.0, 0.0, vec![sound(S2, false, 0.0, 0)]),
            case(vec![int(3)], 0.0, 0.0, vec![sound(S3, false, 0.0, 0)]),
        ],
    );
    let mut rt = single(vec![(KEY, vec![sw])]);
    let mut sink = RecordingSink::new();
    rt.spawn_entity(&mut sink, E, init(vec![(pid::VALUE, int(1))]))
        .unwrap();
    rt.start_script(&mut sink, E, KEY).unwrap();
    rt.set_property(&mut sink, E, pid::VALUE, int(2)).unwrap();
    // Stopping case 1 set the value to 3: only case 3 runs.
    assert_eq!(rt.property(E, pid::VALUE), Some(int(3)));
    assert!(playing(&sink, S2).is_empty(), "case 2 was overtaken");
    assert_eq!(playing(&sink, S3).len(), 1);
    // Case 3 is the tracked case: the next change stops it.
    rt.set_property(&mut sink, E, pid::VALUE, int(1)).unwrap();
    assert!(sink.playing.is_empty(), "{:?}", sink.playing);
    assert!(
        !rt.issues()
            .iter()
            .any(|i| matches!(i, RuntimeIssue::Limit(_)))
    );
}

#[test]
fn on_stop_restoring_the_old_value_restarts_its_case() {
    // The OnStop of case 1 sets the value back to 1 while case 1 stops: case 1 starts
    // again (it is the case for the value now) and case 2 does not start.
    let sw = switch(
        pid::VALUE,
        true,
        vec![
            case(
                vec![int(1)],
                0.0,
                0.0,
                vec![
                    sound(S1, false, 0.0, 0),
                    on_stop(vec![set_property(pid::VALUE, int(1))]),
                ],
            ),
            case(vec![int(2)], 0.0, 0.0, vec![sound(S2, false, 0.0, 0)]),
        ],
    );
    let mut rt = single(vec![(KEY, vec![sw])]);
    let mut sink = RecordingSink::new();
    rt.spawn_entity(&mut sink, E, init(vec![(pid::VALUE, int(1))]))
        .unwrap();
    rt.start_script(&mut sink, E, KEY).unwrap();
    rt.set_property(&mut sink, E, pid::VALUE, int(2)).unwrap();
    assert_eq!(rt.property(E, pid::VALUE), Some(int(1)));
    assert!(playing(&sink, S2).is_empty());
    assert_eq!(playing(&sink, S1).len(), 1, "case 1 restarted");
    rt.set_property(&mut sink, E, pid::VALUE, int(5)).unwrap();
    assert_eq!(rt.property(E, pid::VALUE), Some(int(1)));
    rt.stop_script(&mut sink, E, KEY).unwrap();
    assert!(sink.playing.is_empty());
    assert_eq!(rt.stats().instances, 0);
}

#[test]
fn for_current_value_changing_the_property_at_start_leaves_one_case() {
    // The ForCurrentValue child sets the Switch's own property while FLAG is 1.
    let fcv_child = switch(
        pid::FLAG,
        false,
        vec![case(
            vec![int(1)],
            0.0,
            0.0,
            vec![set_property(pid::VALUE, int(2))],
        )],
    );
    let sw = switch(
        pid::VALUE,
        true,
        vec![
            port(
                PortType::FOR_CURRENT_VALUE,
                PortPayload::None,
                vec![fcv_child],
            ),
            case(vec![int(2)], 0.0, 0.0, vec![sound(S2, false, 0.0, 0)]),
            case(vec![int(3)], 0.0, 0.0, vec![sound(S3, false, 0.0, 0)]),
        ],
    );
    let mut rt = single(vec![(KEY, vec![sw])]);
    let mut sink = RecordingSink::new();
    rt.spawn_entity(
        &mut sink,
        E,
        init(vec![(pid::VALUE, int(1)), (pid::FLAG, int(1))]),
    )
    .unwrap();
    rt.start_script(&mut sink, E, KEY).unwrap();
    assert_eq!(rt.property(E, pid::VALUE), Some(int(2)));
    assert_eq!(playing(&sink, S2).len(), 1);
    rt.set_property(&mut sink, E, pid::FLAG, int(0)).unwrap();
    rt.set_property(&mut sink, E, pid::VALUE, int(3)).unwrap();
    assert!(playing(&sink, S2).is_empty(), "case 2 was the tracked case");
    assert_eq!(playing(&sink, S3).len(), 1);
}

// ----- Timer guards (§11.4, §21) -----

#[test]
fn timer_cycles_too_short_to_move_the_clock_are_cut() {
    // Duration 1e-30 and Speed 1e30 both give a cycle that does not change the scheduler
    // time: the zero-length guard applies, not only to an exact 0.
    for (duration, speed) in [(1e-30, 1.0), (1.0, 1e30), (0.0, 1.0)] {
        let mut rt = single(vec![(KEY, vec![timer_with(false, duration, -1, speed)])]);
        let mut sink = RecordingSink::new();
        rt.spawn_entity(&mut sink, E, init(Vec::new())).unwrap();
        rt.advance(&mut sink, 1.0);
        rt.start_script(&mut sink, E, KEY).unwrap();
        for _ in 0..3 {
            rt.advance(&mut sink, 1.0 / 60.0);
        }
        assert!(
            rt.issues()
                .contains(&RuntimeIssue::Limit("max_same_time_cycles")),
            "{duration} {speed}: {:?}",
            rt.issues()
        );
        assert!(!rt.is_running(E, KEY));
        assert!(rt.stats().events_delivered < 1000, "{duration} {speed}");
    }
}

#[test]
fn tiny_timer_cycles_are_bounded_per_advance() {
    // A cycle of 1e-9 s moves the clock, so the zero-length guard does not apply; the
    // per-advance bound keeps each frame short.
    let mut rt = single(vec![(KEY, vec![timer_with(false, 1e-9, -1, 1.0)])]);
    let mut sink = RecordingSink::new();
    rt.spawn_entity(&mut sink, E, init(Vec::new())).unwrap();
    rt.start_script(&mut sink, E, KEY).unwrap();
    let per_advance = u64::from(rt.limits().max_timer_cycles_per_advance);
    for n in 1..=3u64 {
        rt.advance(&mut sink, 1.0 / 60.0);
        assert!(rt.stats().events_delivered <= n * (per_advance + 1));
    }
    assert!(
        rt.issues()
            .contains(&RuntimeIssue::Limit("max_timer_cycles_per_advance"))
    );
    assert!(rt.is_running(E, KEY), "it lags, it is not cut");
    // The lag catch-up (§11.2) skips whole cycles once the lag exceeds 1.111 s; the loop
    // count (u32::MAX) is then used up and the Timer completes.
    for _ in 0..240 {
        rt.advance(&mut sink, 1.0);
        if !rt.is_running(E, KEY) {
            break;
        }
    }
    assert!(!rt.is_running(E, KEY));
}

// ----- Run chains (§12.4, §21) -----

#[test]
fn run_chains_are_bounded_by_the_instance_depth() {
    // Each run's Timer starts the same key again at once: a chain of runs, each stopping
    // the next when it stops. The chain continues the depth of the Run that started it.
    let t = timer(true, 0.1, vec![(0.0, vec![run(KEY, true, false)])]);
    let mut tab = table(2, Vec::new(), vec![(KEY, vec![t])]);
    script_property(&mut tab, KEY, pid::MAX_INSTANCES, int(1_000_000));
    let mut src = MemorySource::new();
    src.insert_table(Did(TABLE), tab);
    let mut rt = Runtime::new(src, catalog(), vocab()).with_seed(1);
    let mut sink = RecordingSink::new();
    rt.spawn_entity(&mut sink, E, init(Vec::new())).unwrap();
    rt.start_script(&mut sink, E, KEY).unwrap();
    rt.advance(&mut sink, 1.0 / 60.0);
    rt.advance(&mut sink, 1.0 / 60.0);
    let runs = rt.stats().runs;
    let depth = rt.limits().max_instance_depth as usize;
    assert!(runs > 10 && runs <= depth, "{runs}");
    assert!(
        rt.issues()
            .contains(&RuntimeIssue::Limit("max_instance_depth"))
    );
    // Stopping the first run stops the chain without exhausting the stack.
    assert_eq!(rt.stop_script(&mut sink, E, KEY).unwrap(), runs);
    assert_eq!(rt.stats().runs, 0);
    rt.despawn(&mut sink, E).unwrap();
    assert_eq!(rt.stats().instances, 0);
}

#[test]
fn cross_entity_run_ping_pong_takes_one_step_per_advance() {
    // A's Switch on the player's VALUE runs J1 or J2 on the player; J1 sets VALUE to 1 and
    // J2 sets it to 0. Run requests to another entity wait for the next advance.
    let sw = with_player_target(switch(
        pid::VALUE,
        true,
        vec![
            case(vec![int(0)], 0.0, 0.0, vec![run_on_player(J1, false)]),
            case(vec![int(1)], 0.0, 0.0, vec![run_on_player(J2, false)]),
        ],
    ));
    let mut rt = single(vec![
        (KEY, vec![sw]),
        (J1, vec![set_property(pid::VALUE, int(1))]),
        (J2, vec![set_property(pid::VALUE, int(0))]),
    ]);
    let mut sink = RecordingSink::new();
    rt.spawn_entity(&mut sink, PLAYER, init(vec![(pid::VALUE, int(0))]))
        .unwrap();
    rt.set_player(Some(PLAYER));
    rt.spawn_entity(&mut sink, E, init(Vec::new())).unwrap();
    rt.start_script(&mut sink, E, KEY).unwrap();
    let mut seen = Vec::new();
    for _ in 0..4 {
        rt.advance(&mut sink, 1.0 / 60.0);
        seen.push(rt.property(PLAYER, pid::VALUE));
    }
    assert_eq!(
        seen,
        vec![Some(int(1)), Some(int(0)), Some(int(1)), Some(int(0))]
    );
    assert_eq!(rt.stats().events_delivered, 4);
    assert!(rt.issues().is_empty(), "{:?}", rt.issues());
}

#[test]
fn deferred_run_made_outside_an_advance_runs_in_the_next_one() {
    let mut rt = single(vec![
        (KEY, vec![run_on_player(J1, false)]),
        (J1, vec![set_property(pid::VALUE, int(7))]),
    ]);
    let mut sink = RecordingSink::new();
    rt.spawn_entity(&mut sink, PLAYER, init(Vec::new()))
        .unwrap();
    rt.set_player(Some(PLAYER));
    rt.spawn_entity(&mut sink, E, init(Vec::new())).unwrap();
    rt.start_script(&mut sink, E, KEY).unwrap();
    rt.advance(&mut sink, 0.0);
    assert_eq!(rt.property(PLAYER, pid::VALUE), Some(int(7)));
}

// ----- the clock (§20.1) -----

#[test]
fn huge_time_steps_are_shortened() {
    let mut rt = single(vec![(
        KEY,
        vec![sequence(true, 1, vec![Vec::new(), Vec::new()])],
    )]);
    let mut sink = RecordingSink::new();
    rt.spawn_entity(&mut sink, E, init(Vec::new())).unwrap();
    rt.advance(&mut sink, 1e16);
    assert_eq!(rt.time(), MAX_ADVANCE);
    assert_eq!(
        rt.start_script(&mut sink, E, KEY).unwrap(),
        ScriptStart::Running
    );
    rt.advance(&mut sink, 1.0);
    assert!(rt.is_running(E, KEY));
}

// ----- teardown (§19) -----

#[test]
fn despawn_starts_nothing() {
    const TIMER_KEY: u32 = 0x10;
    const WATCH_KEY: u32 = 0x20;
    let mut rt = single(vec![
        (TIMER_KEY, vec![timer(true, 1.0, Vec::new())]),
        (
            WATCH_KEY,
            vec![when_running(
                vec![TIMER_KEY],
                true,
                Vec::new(),
                vec![sound(S1, true, 0.0, 0), set_property(pid::FLAG, int(1))],
            )],
        ),
    ]);
    let mut sink = RecordingSink::new();
    let mut i = init(Vec::new());
    i.startup_scripts = vec![TIMER_KEY, WATCH_KEY];
    rt.spawn_entity(&mut sink, E, i).unwrap();
    assert!(rt.is_running(E, TIMER_KEY) && rt.is_running(E, WATCH_KEY));
    assert_eq!(sink.sounds_started, 0);
    rt.despawn(&mut sink, E).unwrap();
    assert_eq!(sink.sounds_started, 0, "{:?}", sink.events);
    assert_eq!(rt.stats().instances, 0);
}

// ----- entity IDs reused (§19) -----

#[test]
fn listeners_follow_a_respawned_target() {
    let sw = with_player_target(switch(
        pid::AREA_MUSIC,
        true,
        vec![
            case(vec![int(1)], 0.0, 0.0, vec![sound(S1, false, 0.0, 2)]),
            case(vec![int(2)], 0.0, 0.0, vec![sound(S2, false, 0.0, 2)]),
        ],
    ));
    let mut rt = single(vec![(KEY, vec![sw])]);
    let mut sink = RecordingSink::new();
    let player = || init(vec![(pid::AREA_MUSIC, int(1))]);
    rt.spawn_entity(&mut sink, PLAYER, player()).unwrap();
    rt.set_player(Some(PLAYER));
    rt.spawn_entity(&mut sink, E, init(Vec::new())).unwrap();
    rt.start_script(&mut sink, E, KEY).unwrap();
    assert_eq!(playing(&sink, S1).len(), 1);
    // The player goes away: no value (no default), no case.
    rt.despawn(&mut sink, PLAYER).unwrap();
    assert!(sink.playing.is_empty());
    // The same ID comes back: the Switch hears it again.
    rt.spawn_entity(&mut sink, PLAYER, player()).unwrap();
    rt.set_player(Some(PLAYER));
    assert_eq!(playing(&sink, S1).len(), 1);
    rt.set_property(&mut sink, PLAYER, pid::AREA_MUSIC, int(2))
        .unwrap();
    assert!(playing(&sink, S1).is_empty());
    assert_eq!(playing(&sink, S2).len(), 1);
    rt.despawn(&mut sink, E).unwrap();
    rt.despawn(&mut sink, PLAYER).unwrap();
    assert_eq!(rt.stats().instances, 0);
}

// ----- Switch int lists and transitions (§6.3, §6.7, §6.9) -----

fn int_list_case(
    values: Vec<PropertyValue>,
    ints: Vec<i32>,
    children: Vec<Node>,
) -> tessera_formats::script_table::Port {
    port(
        PortType::SWITCH_VALUE,
        PortPayload::SwitchCase(Box::new(SwitchCase {
            values: values
                .into_iter()
                .map(|v| Some(PropertyEntry { pid: 0, value: v }))
                .collect(),
            ints,
            operator: 3,
            transition_time: 0.0,
            second_time: 0.0,
        })),
        children,
    )
}

#[test]
fn int_list_change_selects_the_case_without_a_transition() {
    let sw = switch(
        pid::VALUE,
        true,
        vec![
            int_list_case(
                vec![int(5)],
                vec![pid::OTHER.cast_signed()],
                vec![sound(S1, false, 0.0, 0)],
            ),
            switch_default(vec![sound(S2, false, 0.0, 0)]),
            transition(None, None, false, vec![sound(0x2A00_00EE, true, 0.0, 0)]),
        ],
    );
    let mut rt = single(vec![(KEY, vec![sw])]);
    let mut sink = RecordingSink::new();
    rt.spawn_entity(
        &mut sink,
        E,
        init(vec![(pid::VALUE, int(1)), (pid::OTHER, int(0))]),
    )
    .unwrap();
    rt.start_script(&mut sink, E, KEY).unwrap();
    assert_eq!(playing(&sink, S2).len(), 1, "Default");
    rt.set_property(&mut sink, E, pid::OTHER, int(1)).unwrap();
    assert_eq!(
        playing(&sink, S1).len(),
        1,
        "the int list matches VALUE now"
    );
    assert!(playing(&sink, S2).is_empty());
    let transition_sounds = sink
        .sound_starts()
        .filter(|(_, s)| s.sound == Did(0x2A00_00EE))
        .count();
    assert_eq!(transition_sounds, 0, "VALUE did not change");
    // A change of the value itself applies the transition.
    rt.set_property(&mut sink, E, pid::VALUE, int(5)).unwrap();
    let transition_sounds = sink
        .sound_starts()
        .filter(|(_, s)| s.sound == Did(0x2A00_00EE))
        .count();
    assert_eq!(transition_sounds, 1);
    assert_eq!(playing(&sink, S1).len(), 1, "case kept");
}

// ----- API defaults -----

#[test]
fn entity_init_default_runs_startup() {
    assert!(EntityInit::default().run_startup);
    let mut rt = single(vec![(STARTUP, vec![sound(S1, false, 0.0, 0)])]);
    let mut sink = RecordingSink::new();
    rt.spawn_entity(
        &mut sink,
        E,
        EntityInit {
            tables: vec![Did(TABLE)],
            ..EntityInit::default()
        },
    )
    .unwrap();
    assert_eq!(rt.running_scripts(E), vec![STARTUP]);
    assert_eq!(playing(&sink, S1).len(), 1);
}
