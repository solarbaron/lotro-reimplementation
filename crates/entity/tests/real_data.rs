// SPDX-License-Identifier: GPL-3.0-or-later
//! Real-data test (gated on `TESSERA_GAME_DIR`; passes without it): streams the statics of
//! the install's landblocks along the M3 flight (busiest row of region 1) through the store
//! together with animated characters, the benchmark's workload unpaced, and checks the
//! store's bookkeeping against the trace every frame batch. Only counts are printed.
//!
//! `cargo test --release -p tessera-entity --test real_data -- --nocapture`

#[allow(dead_code)]
#[path = "../examples/entity_bench/workload.rs"]
mod workload;

#[allow(dead_code)]
#[path = "../examples/entity_bench/own.rs"]
mod own;

use std::collections::HashSet;
use std::time::Duration;

use tessera_anim::{AnimationScheduler, SchedulerConfig};
use tessera_assets::{PoolConfig, WorkerPool};
use tessera_entity::{Changes, EntityStore, LandblockTag, Transform};

use workload::{Backend, Flight, Sink};

fn install_set() -> bool {
    match std::env::var_os("TESSERA_GAME_DIR") {
        Some(d) if !d.is_empty() => true,
        _ => {
            eprintln!("TESSERA_GAME_DIR not set; skipping");
            false
        }
    }
}

fn frames() -> usize {
    if cfg!(debug_assertions) { 600 } else { 2_439 }
}

#[test]
fn flight_statics_bookkeeping() {
    if !install_set() {
        return;
    }
    let row = workload::busiest_row().expect("region 1 directory");
    let flight = Flight {
        region: 1,
        row,
        speed: 1_000.0,
        frames: frames(),
        max_integrations: 32,
    };
    let trace = workload::build_trace(flight, |keys| {
        workload::real_statics(keys).expect("bundles")
    });
    assert!(
        trace.peak_resident_blocks > 300,
        "{}",
        trace.peak_resident_blocks
    );
    let mut store: EntityStore<u32, ()> = EntityStore::new();
    let mut changes = Changes::new();
    let mut resident: HashSet<u32> = HashSet::new();
    let (mut expected, mut instances) = (0usize, 0i64);
    let mut peak = 0usize;
    for f in &trace.frames {
        for tag in &f.release {
            let n = store.despawn_landblock(LandblockTag(*tag));
            assert_eq!(n, trace.blocks[tag].len(), "{tag:#x}");
            assert!(resident.remove(tag));
            expected -= n;
        }
        for tag in &f.integrate {
            for (i, s) in trace.blocks[tag].iter().enumerate() {
                let r = s.draws.then_some(i as u32);
                store
                    .spawn_static(LandblockTag(*tag), s.transform, r)
                    .expect("spawn");
            }
            assert!(resident.insert(*tag));
            expected += trace.blocks[tag].len();
        }
        store.take_changes(&mut changes);
        instances += changes.render_added.len() as i64 - changes.render_removed.len() as i64;
        assert_eq!(store.len(), expected);
        peak = peak.max(store.len());
    }
    assert_eq!(peak, trace.peak_resident_statics);
    assert_eq!(instances, store.renderables().count() as i64);
    let with_statics = resident
        .iter()
        .filter(|t| !trace.blocks[*t].is_empty())
        .count();
    assert_eq!(store.landblock_count(), with_statics);
    for (tag, n) in store.landblocks() {
        assert_eq!(n, trace.blocks[&tag.0].len());
        assert!(
            store
                .landblock_entities(tag)
                .iter()
                .all(|&e| store.landblock_of(e) == Some(tag))
        );
    }
    eprintln!(
        "row {row}: {} frames, {} landblocks integrated, {} released, peak {} resident \
         landblocks / {} statics, {} live at the end",
        trace.frames.len(),
        trace.integrations,
        trace.releases,
        trace.peak_resident_blocks,
        peak,
        store.len()
    );
}

#[test]
fn flight_with_animated_characters() {
    if !install_set() {
        return;
    }
    let row = workload::busiest_row().expect("region 1 directory");
    let flight = Flight {
        region: 1,
        row,
        speed: 1_000.0,
        frames: frames().min(600),
        max_integrations: 32,
    };
    let trace = workload::build_trace(flight, |keys| {
        workload::real_statics(keys).expect("bundles")
    });
    let rig = workload::synthetic_rig(89);
    let pool = WorkerPool::new(PoolConfig::default());
    let mut sched = AnimationScheduler::new(&pool, SchedulerConfig::default());
    let mut sink = Sink::new();
    let mut b = own::Own::new();
    for i in 0..50u64 {
        let anim = rig.character(&mut sched, i as usize);
        let key = sink.alloc();
        b.spawn_character(i + 1, Transform::IDENTITY, key, anim);
    }
    for (n, f) in trace.frames.iter().enumerate() {
        for i in 0..50u64 {
            b.move_character(
                i + 1,
                Transform::from_position([f.focus[0], f.focus[1], n as f64]),
            );
        }
        for tag in &f.release {
            b.despawn_block(*tag, &mut sink);
        }
        for tag in &f.integrate {
            b.spawn_block(*tag, &trace.blocks[tag], &mut sink);
        }
        b.animate(&mut sched, n as u64 + 1, 1.0 / 60.0);
        b.sync_render(&mut sched, &mut sink);
    }
    sched.collect_all(Duration::from_secs(10));
    let (entities, renderables) = b.counts();
    assert_eq!(renderables, sink.live, "instance table mirrors the store");
    assert!(entities >= 50);
    assert_eq!(sched.len(), 50);
    assert!(sink.palettes > 0);
}
