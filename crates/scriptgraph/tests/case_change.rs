// SPDX-License-Identifier: GPL-3.0-or-later
//! What a Switch case change stops (`script-runtime.md` §6.6 "What a case change stops",
//! R56): the whole left branch, and the scripts that its Run nodes started only through
//! Runs with `Persistent` 1 and `UnbindOnDeath` 0 that are still running.

mod common;

use common::*;
use tessera_formats::PropertyValue;
use tessera_formats::script_table::{Node, NodeData};
use tessera_scriptgraph::{RecordingSink, ScriptStart};

const KEY: u32 = 0x1000_0001;
const K_P0: u32 = 0x1000_0010;
const K_P1: u32 = 0x1000_0011;
const K_UNBIND: u32 = 0x1000_0012;
const K_OUTER: u32 = 0x1000_0013;
const K_INNER: u32 = 0x1000_0014;
const K_SHARED: u32 = 0x1000_0015;
const SND_OWN: u32 = 0x2A00_0001;
const SND_P0: u32 = 0x2A00_0002;
const SND_P1: u32 = 0x2A00_0003;
const SND_UNBIND: u32 = 0x2A00_0004;
const SND_INNER: u32 = 0x2A00_0005;
const SND_SHARED: u32 = 0x2A00_0006;
const SND_TIMER: u32 = 0x2A00_0007;

/// A Run node with `UnbindOnDeath` 1.
fn run_unbound(key: u32) -> Node {
    let mut n = run(key, true, false);
    if let NodeData::Run(r) = &mut n.data {
        r.unbind_on_death = true;
    }
    n
}

/// A tracked sound that never ends by itself (it has to be stopped).
fn bed(did: u32) -> Vec<Node> {
    vec![sound(did, false, 0.0, 7)]
}

/// A persistent Switch on [`pid::VALUE`]: case 1 holds the branch under test, case 2 is
/// empty.
fn switch_with(branch: Vec<Node>) -> Node {
    switch(
        pid::VALUE,
        true,
        vec![
            case(vec![PropertyValue::Int(1)], 0.0, 0.0, branch),
            case(vec![PropertyValue::Int(2)], 0.0, 0.0, Vec::new()),
        ],
    )
}

#[test]
fn a_case_change_stops_the_branch_and_only_its_bound_runs() {
    let branch = vec![
        // The branch's own nodes, of different kinds and depths.
        sound(SND_OWN, false, 0.0, 7),
        collection(vec![timer(true, 5.0, vec![(1.0, bed(SND_TIMER))])]),
        // `Persistent` 0: completes at its request; its run outlives the branch.
        run(K_P0, false, false),
        // `Persistent` 1, `UnbindOnDeath` 0, still running: stops its run.
        run(K_P1, true, false),
        // `UnbindOnDeath` 1: keeps its run.
        run_unbound(K_UNBIND),
        // A `Persistent` 0 run that itself holds a binding Run: the inner run belongs to
        // the kept outer run, not to the branch.
        run(K_OUTER, false, false),
    ];
    let mut rt = single(vec![
        (KEY, vec![switch_with(branch)]),
        (K_P0, bed(SND_P0)),
        (K_P1, bed(SND_P1)),
        (K_UNBIND, bed(SND_UNBIND)),
        (K_OUTER, vec![run(K_INNER, true, false)]),
        (K_INNER, bed(SND_INNER)),
    ]);
    let mut sink = RecordingSink::new();
    let mut i = init(vec![(pid::VALUE, PropertyValue::Int(1))]);
    i.startup_scripts.push(KEY);
    rt.spawn_entity(&mut sink, E, i).unwrap();
    run_until(&mut rt, &mut sink, 1.5);
    for (did, key) in [
        (SND_OWN, None),
        (SND_TIMER, None),
        (SND_P0, Some(K_P0)),
        (SND_P1, Some(K_P1)),
        (SND_UNBIND, Some(K_UNBIND)),
        (SND_INNER, Some(K_INNER)),
    ] {
        assert_eq!(playing(&sink, did).len(), 1, "{did:#x} before");
        if let Some(k) = key {
            assert!(rt.is_running(E, k), "{k:#x} before");
        }
    }
    rt.set_property(&mut sink, E, pid::VALUE, PropertyValue::Int(2))
        .unwrap();
    // The whole left branch stopped, every kind with its descendants, and the run of the
    // still-running `Persistent` 1 Run.
    for did in [SND_OWN, SND_TIMER, SND_P1] {
        assert!(playing(&sink, did).is_empty(), "{did:#x} stopped");
    }
    assert!(!rt.is_running(E, K_P1));
    // The runs of `Persistent` 0 and `UnbindOnDeath` 1 Runs keep running.
    for (did, key) in [(SND_P0, K_P0), (SND_UNBIND, K_UNBIND), (SND_INNER, K_INNER)] {
        assert_eq!(playing(&sink, did).len(), 1, "{did:#x} kept");
        assert!(rt.is_running(E, key), "{key:#x} kept");
    }
    assert!(rt.is_running(E, K_OUTER));
    // The timer of the left branch fires no more.
    let before = sink.sound_starts().count();
    run_until(&mut rt, &mut sink, 8.0);
    assert_eq!(sink.sound_starts().count(), before);
}

/// A `Persistent` 1 Run whose start was refused (the script was already running, the
/// instance limit being 1) has completed and stops nothing: the earlier run survives the
/// case change (§6.6, §12.3).
#[test]
fn a_refused_persistent_run_does_not_stop_the_earlier_run() {
    let mut rt = single(vec![
        (KEY, vec![switch_with(vec![run(K_SHARED, true, false)])]),
        (K_SHARED, bed(SND_SHARED)),
    ]);
    let mut sink = RecordingSink::new();
    rt.spawn_entity(
        &mut sink,
        E,
        init(vec![(pid::VALUE, PropertyValue::Int(2))]),
    )
    .unwrap();
    assert_eq!(
        rt.start_script(&mut sink, E, K_SHARED).unwrap(),
        ScriptStart::Running
    );
    assert_eq!(
        rt.start_script(&mut sink, E, KEY).unwrap(),
        ScriptStart::Running
    );
    // Case 2 first, then case 1: the Run's start is refused.
    rt.set_property(&mut sink, E, pid::VALUE, PropertyValue::Int(1))
        .unwrap();
    assert_eq!(playing(&sink, SND_SHARED).len(), 1);
    rt.set_property(&mut sink, E, pid::VALUE, PropertyValue::Int(2))
        .unwrap();
    assert!(rt.is_running(E, K_SHARED));
    assert_eq!(playing(&sink, SND_SHARED).len(), 1);

    // The same Run started first by the branch is bound to it and stops with it.
    let mut rt = single(vec![
        (KEY, vec![switch_with(vec![run(K_SHARED, true, false)])]),
        (K_SHARED, bed(SND_SHARED)),
    ]);
    let mut sink = RecordingSink::new();
    let mut i = init(vec![(pid::VALUE, PropertyValue::Int(1))]);
    i.startup_scripts.push(KEY);
    rt.spawn_entity(&mut sink, E, i).unwrap();
    assert!(rt.is_running(E, K_SHARED));
    rt.set_property(&mut sink, E, pid::VALUE, PropertyValue::Int(2))
        .unwrap();
    assert!(!rt.is_running(E, K_SHARED));
    assert!(sink.playing.is_empty());
}

/// A Run with `Stop` 1 in the left branch completed at once and has nothing to undo: what
/// it stopped stays stopped and what it did not touch keeps running.
#[test]
fn a_stop_run_in_the_left_branch_undoes_nothing() {
    let mut rt = single(vec![
        (KEY, vec![switch_with(vec![run(K_P0, false, true)])]),
        (K_P0, bed(SND_P0)),
        (K_P1, bed(SND_P1)),
    ]);
    let mut sink = RecordingSink::new();
    rt.spawn_entity(
        &mut sink,
        E,
        init(vec![(pid::VALUE, PropertyValue::Int(2))]),
    )
    .unwrap();
    rt.start_script(&mut sink, E, K_P0).unwrap();
    rt.start_script(&mut sink, E, K_P1).unwrap();
    rt.start_script(&mut sink, E, KEY).unwrap();
    rt.set_property(&mut sink, E, pid::VALUE, PropertyValue::Int(1))
        .unwrap();
    assert!(!rt.is_running(E, K_P0));
    rt.set_property(&mut sink, E, pid::VALUE, PropertyValue::Int(2))
        .unwrap();
    assert!(!rt.is_running(E, K_P0));
    assert!(rt.is_running(E, K_P1));
    assert_eq!(playing(&sink, SND_P1).len(), 1);
}
