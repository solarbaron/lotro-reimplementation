// SPDX-License-Identifier: GPL-3.0-or-later
//! The Random node's source (`script-runtime.md` §8 "Random source", R40, OPEN-18): by
//! default every entity's Random picks come from the runtime's single generator, so they
//! depend on everything drawn before; an entity spawned with its own seed (Tessera's
//! per-figure Decision) repeats its picks whatever the runtime's seed and history.

mod common;

use common::*;
use tessera_formats::Did;
use tessera_scriptgraph::{EntityId, EntityInit, RecordingSink, Runtime};

const KEY: u32 = 0x1000_0200;
const SOUNDS: [u32; 4] = [0x2A00_0011, 0x2A00_0012, 0x2A00_0013, 0x2A00_0014];

fn runtime(seed: u64) -> Runtime<'static> {
    let root = random(
        false,
        SOUNDS
            .iter()
            .map(|&s| (1, vec![sound(s, true, 0.0, 7)]))
            .collect(),
    );
    single(vec![(KEY, vec![root])]).with_seed(seed)
}

/// The sounds `e`'s Random picks in `n` starts.
fn picks(rt: &mut Runtime<'_>, sink: &mut RecordingSink, e: EntityId, n: usize) -> Vec<u32> {
    (0..n)
        .map(|_| {
            rt.start_script(sink, e, KEY).unwrap();
            sink.sound_starts().last().unwrap().1.sound.0
        })
        .collect()
}

fn spawn(rt: &mut Runtime<'_>, sink: &mut RecordingSink, e: EntityId, seed: Option<u64>) {
    let init = EntityInit {
        tables: vec![Did(TABLE)],
        random_seed: seed,
        ..EntityInit::default()
    };
    rt.spawn_entity(sink, e, init).unwrap();
}

#[test]
fn a_seeded_entity_repeats_its_picks() {
    let figure = EntityId(10);
    let other = EntityId(11);
    let mut first = Vec::new();
    for (global, burn) in [(1u64, 0usize), (99, 7), (12_345, 3)] {
        let mut rt = runtime(global);
        let mut sink = RecordingSink::new();
        // Other draws first (a different history of the single generator).
        spawn(&mut rt, &mut sink, other, None);
        let _ = picks(&mut rt, &mut sink, other, burn);
        spawn(&mut rt, &mut sink, figure, Some(0x0200_0000_1234));
        let p = picks(&mut rt, &mut sink, figure, 24);
        if first.is_empty() {
            first = p;
        } else {
            assert_eq!(p, first, "global seed {global}");
        }
    }
    // Every port is reachable.
    for s in SOUNDS {
        assert!(first.contains(&s), "{s:#x} never picked: {first:x?}");
    }
}

#[test]
fn unseeded_entities_share_the_runtime_generator() {
    let e = EntityId(20);
    let run = |global: u64, burn: usize| {
        let mut rt = runtime(global);
        let mut sink = RecordingSink::new();
        spawn(&mut rt, &mut sink, EntityId(21), None);
        let _ = picks(&mut rt, &mut sink, EntityId(21), burn);
        spawn(&mut rt, &mut sink, e, None);
        picks(&mut rt, &mut sink, e, 24)
    };
    assert_eq!(run(5, 0), run(5, 0), "same seed, same history");
    assert_ne!(run(5, 0), run(5, 1), "the history changes the picks");
}
