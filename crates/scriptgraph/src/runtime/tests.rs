// SPDX-License-Identifier: GPL-3.0-or-later
//! Unit tests that need the runtime's internals (port groups, the scheduler clock).

#[path = "../../tests/common/mod.rs"]
mod common;

use common::*;
use tessera_formats::PropertyValue;

use crate::{RecordingSink, RuntimeIssue};

const KEY: u32 = 0x1000_0001;
const SND_T: u32 = 0x2A00_00EE;

/// Open port groups over all live instances.
fn open_groups(rt: &crate::Runtime<'_>) -> usize {
    rt.insts.iter().map(|i| i.live_tags.len()).sum()
}

#[test]
fn transition_without_wait_closes_its_group() {
    // A transition with Wait = 0 whose tracked child keeps running must not leave its port
    // group open: a Switch on a busy property would otherwise collect one group per change.
    let sw = switch(
        pid::VALUE,
        true,
        vec![
            case(vec![PropertyValue::Int(1)], 0.0, 0.0, Vec::new()),
            case(vec![PropertyValue::Int(2)], 0.0, 0.0, Vec::new()),
            transition(None, None, false, vec![sound(SND_T, false, 0.0, 0)]),
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
    let before = open_groups(&rt);
    for n in 0..50 {
        let v = if n % 2 == 0 { 2 } else { 1 };
        rt.set_property(&mut sink, E, pid::VALUE, PropertyValue::Int(v))
            .unwrap();
        let h = playing(&sink, SND_T);
        assert_eq!(h.len(), 1, "the transition's sound runs");
        complete(&mut rt, &mut sink, h[0]);
    }
    assert!(open_groups(&rt) <= before + 1, "{}", open_groups(&rt));
    assert_eq!(sink.sounds_started, 50);
}

#[test]
fn sequence_pacing_survives_an_absorbing_clock() {
    // At a clock where adding 0.1 s no longer changes it, the pass pacing cannot hold a
    // looping Sequence whose steps complete at once; the guard ends it.
    let mut rt = single(vec![(
        KEY,
        vec![sequence(true, 1, vec![Vec::new(), Vec::new()])],
    )]);
    let mut sink = RecordingSink::new();
    rt.spawn_entity(&mut sink, E, init(Vec::new())).unwrap();
    rt.sched.now = 1e16;
    rt.sched.frame_time = 1e16;
    rt.start_script(&mut sink, E, KEY).unwrap();
    assert!(
        rt.issues()
            .contains(&RuntimeIssue::Limit("max_same_time_cycles"))
    );
    assert!(!rt.is_running(E, KEY));
}

#[test]
fn looping_timer_survives_an_absorbing_clock() {
    let mut rt = single(vec![(KEY, vec![timer(true, 0.1, vec![(0.0, Vec::new())])])]);
    let mut sink = RecordingSink::new();
    rt.spawn_entity(&mut sink, E, init(Vec::new())).unwrap();
    rt.sched.now = 1e16;
    rt.sched.frame_time = 1e16;
    rt.start_script(&mut sink, E, KEY).unwrap();
    rt.advance(&mut sink, 0.0);
    assert!(
        rt.issues()
            .contains(&RuntimeIssue::Limit("max_same_time_cycles"))
    );
    assert!(!rt.is_running(E, KEY));
    assert!(rt.stats().events_delivered < 1000);
}
