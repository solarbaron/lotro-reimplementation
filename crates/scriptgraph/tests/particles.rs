// SPDX-License-Identifier: GPL-3.0-or-later
//! Particle nodes (`particle-runtime.md` §5.4–§5.8, §7.1): start gates, the never-ending
//! instance, the stop hook with `ImmediateDisposal`, the dropped path and the redirect
//! property. Every DID, key and PID is invented.

mod common;

use common::*;
use tessera_formats::script_table::{Node, NodeData, NodeKind, ParticleScriptNode};
use tessera_formats::{Did, PropertyValue};
use tessera_scriptgraph::{EntityId, RecordingSink, ScriptStart, SinkEvent};

const KEY: u32 = 0x1000_0001;
const FX: u32 = 0x3900_0001;

fn particle(p: ParticleScriptNode, never_ending: Option<bool>) -> Node {
    let mut n = node(
        NodeKind::PARTICLE,
        NodeData::Particle(Box::new(p)),
        Vec::new(),
    );
    n.particle_flag = never_ending;
    n
}

fn fx() -> ParticleScriptNode {
    ParticleScriptNode {
        particle_id: Did(FX),
        ..ParticleScriptNode::default()
    }
}

fn starts(sink: &RecordingSink) -> Vec<(u64, EntityId, bool)> {
    sink.events
        .iter()
        .filter_map(|e| match e {
            SinkEvent::ParticleStarted(h, s) => Some((h.0, s.target, s.never_ending)),
            _ => None,
        })
        .collect()
}

fn stops(sink: &RecordingSink) -> Vec<(u64, bool)> {
    sink.events
        .iter()
        .filter_map(|e| match e {
            SinkEvent::ParticleStopped(h, i) => Some((h.0, *i)),
            _ => None,
        })
        .collect()
}

#[test]
fn a_finite_effect_starts_and_its_node_completes_at_once() {
    let mut rt = single(vec![(KEY, vec![particle(fx(), Some(false))])]);
    let mut sink = RecordingSink::new();
    rt.spawn_entity(&mut sink, E, init(Vec::new())).unwrap();
    assert_eq!(
        rt.start_script(&mut sink, E, KEY).unwrap(),
        ScriptStart::Completed
    );
    let s = starts(&sink);
    assert_eq!(s.len(), 1);
    assert_eq!((s[0].1, s[0].2), (E, false));
    assert_eq!(
        sink.particles.values().next().map(|r| r.effect),
        Some(Did(FX))
    );
    // Finite effects are never stopped by their script (§7.1).
    rt.despawn(&mut sink, E).unwrap();
    assert!(stops(&sink).is_empty());
}

#[test]
fn intensity_gates_finite_effects_only() {
    let roots = vec![
        with_intensity(particle(fx(), Some(false)), 0.0, 0.0, true),
        with_intensity(particle(fx(), Some(true)), 0.0, 0.0, true),
    ];
    let mut rt = single(vec![(KEY, roots)]);
    let mut sink = RecordingSink::new();
    rt.spawn_entity(&mut sink, E, init(Vec::new())).unwrap();
    rt.start_script(&mut sink, E, KEY).unwrap();
    let s = starts(&sink);
    assert_eq!(s.len(), 1, "only the never-ending one starts");
    assert!(s[0].2);
    assert_eq!(rt.stats().particles_gated, 1);
}

#[test]
fn no_effect_did_starts_nothing() {
    let mut rt = single(vec![(
        KEY,
        vec![particle(ParticleScriptNode::default(), Some(true))],
    )]);
    let mut sink = RecordingSink::new();
    rt.spawn_entity(&mut sink, E, init(Vec::new())).unwrap();
    assert_eq!(
        rt.start_script(&mut sink, E, KEY).unwrap(),
        ScriptStart::Completed
    );
    assert!(starts(&sink).is_empty());
}

#[test]
fn a_never_ending_effect_runs_until_its_script_stops() {
    for immediate in [false, true] {
        let p = ParticleScriptNode {
            immediate_disposal: immediate,
            ..fx()
        };
        let mut rt = single(vec![(KEY, vec![particle(p, Some(true))])]);
        let mut sink = RecordingSink::new();
        rt.spawn_entity(&mut sink, E, init(Vec::new())).unwrap();
        assert_eq!(
            rt.start_script(&mut sink, E, KEY).unwrap(),
            ScriptStart::Running
        );
        let h = starts(&sink)[0].0;
        assert!(stops(&sink).is_empty());
        assert_eq!(rt.stop_script(&mut sink, E, KEY).unwrap(), 1);
        assert_eq!(stops(&sink), vec![(h, immediate)]);
        assert!(sink.particles.is_empty());
    }
}

#[test]
fn the_dropped_path_completes_at_once_even_for_a_never_ending_effect() {
    let p = ParticleScriptNode {
        drop_effect: true,
        redirect_property: pid::OTHER,
        ..fx()
    };
    let mut rt = single(vec![(KEY, vec![particle(p, Some(true))])]);
    let mut sink = RecordingSink::new();
    rt.spawn_entity(
        &mut sink,
        E,
        init(vec![(pid::OTHER, PropertyValue::Int(2))]),
    )
    .unwrap();
    rt.spawn_entity(&mut sink, EntityId(2), init(Vec::new()))
        .unwrap();
    assert_eq!(
        rt.start_script(&mut sink, E, KEY).unwrap(),
        ScriptStart::Completed
    );
    // §5.6 step 1: no redirect on the dropped path.
    assert_eq!(starts(&sink)[0].1, E);
}

#[test]
fn the_redirect_property_moves_a_never_ending_effect() {
    let p = ParticleScriptNode {
        redirect_property: pid::OTHER,
        ..fx()
    };
    let mut rt = single(vec![(KEY, vec![particle(p, Some(true))])]);
    let mut sink = RecordingSink::new();
    let other = EntityId(2);
    rt.spawn_entity(&mut sink, other, init(Vec::new())).unwrap();
    rt.spawn_entity(
        &mut sink,
        E,
        init(vec![(pid::OTHER, PropertyValue::Int(7))]),
    )
    .unwrap();
    rt.start_script(&mut sink, E, KEY).unwrap();
    // 7 names no object: the effect stays on the script's entity.
    let first = starts(&sink);
    assert_eq!(first.len(), 1);
    assert_eq!(first[0].1, E);
    // §5.8: a change stops the effect and starts it again on the named object.
    rt.set_property(&mut sink, E, pid::OTHER, PropertyValue::Int(2))
        .unwrap();
    assert_eq!(stops(&sink), vec![(first[0].0, false)]);
    let now = starts(&sink);
    assert_eq!(now.len(), 2);
    assert_eq!(now[1].1, other);
    assert_eq!(sink.particles.len(), 1);
    rt.despawn(&mut sink, E).unwrap();
    assert!(sink.particles.is_empty());
}
