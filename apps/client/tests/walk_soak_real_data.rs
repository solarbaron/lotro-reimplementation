// SPDX-License-Identifier: GPL-3.0-or-later
//! ROADMAP M4.5 exit criterion 4, soak part (`docs/checklists/m4-5-visual.md` §2.5): a
//! seeded soak of at least 1,000 random walks of 60 s simulated time over the land
//! landblocks of every region with landblock data, on the collision walk mode over the
//! user's own install, ends with 0 fall-throughs (feet more than 0.05 m below the terrain
//! of their landblock while outdoors, or below the floor of the cell that contains them)
//! and 0 non-finite states.
//!
//! The walks are planned in order (one generator through all walks, each start checked
//! against the streamed world, §2.5 steps 3–4) on one thread and run on several, each with
//! its own streamer over the shared workers. The land lists, the seed and every failing
//! walk (number, landblock, start, heading) are printed, so a failing walk can be replayed
//! alone with `TESSERA_SOAK_ONLY=k`.
//!
//! Skipped (reported as passing) unless `TESSERA_GAME_DIR` points at an install. Run with
//! `cargo test --release -p tessera-client --test walk_soak_real_data -- --nocapture`.
//! `TESSERA_SOAK_WALKS=N` sets the walk count (default 1,000; a debug build walks 12 unless
//! `TESSERA_FULL=1`). Only counts, landblock numbers and positions are printed.

use std::path::PathBuf;
use std::sync::mpsc;
use std::sync::{Arc, Mutex};
use std::time::Instant;

use tessera_client::avatar::AvatarSpec;
use tessera_client::wait::{LoadWait, ProgressWait};
use tessera_client::walker::movement::run::{WalkSession, wait_blocks};
use tessera_client::walker::movement::soak::{
    SOAK_SEED, SOAK_WALKS, SoakOutcome, SoakPlanner, SoakWalk, find_start, land_landblocks,
    run_soak_walk, soak_regions,
};
use tessera_client::walker::movement::{PhysicsWalker, run::yaw_of};
use tessera_world::{LANDBLOCK_SIZE, RingConfig};

/// How long a wait for collision sets may last: as long as loading progresses. It gives up
/// after this many polls (about 1 ms apart, longer on a loaded machine) in a row in which
/// no loading work started or finished and the missing sets stayed the same, or after this
/// many polls in all (`walker.md` §7, implementer note on progress waits).
const LOAD_WAIT: LoadWait = if cfg!(debug_assertions) {
    LoadWait::Progress(ProgressWait::new(900_000, 14_400_000))
} else {
    LoadWait::Progress(ProgressWait::new(120_000, 3_600_000))
};

fn install() -> Option<PathBuf> {
    match std::env::var_os("TESSERA_GAME_DIR") {
        Some(d) if !d.is_empty() => Some(PathBuf::from(d)),
        _ => {
            eprintln!("TESSERA_GAME_DIR not set; skipping");
            None
        }
    }
}

fn walk_count() -> usize {
    if let Some(n) = std::env::var("TESSERA_SOAK_WALKS")
        .ok()
        .and_then(|v| v.parse().ok())
    {
        return n;
    }
    let full = std::env::var_os("TESSERA_FULL").is_some_and(|v| v == "1");
    if cfg!(debug_assertions) && !full {
        12
    } else {
        SOAK_WALKS
    }
}

/// Rings of a soak streamer: the mover's landblock and its neighbours.
fn soak_rings() -> RingConfig {
    RingConfig {
        near: 1.5 * LANDBLOCK_SIZE,
        mid: 1.5 * LANDBLOCK_SIZE,
        far: 1.5 * LANDBLOCK_SIZE,
        hysteresis: 0.5 * LANDBLOCK_SIZE,
    }
}

#[test]
fn soak_of_random_walks_never_falls_through() {
    let Some(dir) = install() else { return };
    let threads = std::thread::available_parallelism().map_or(4, |n| n.get());
    let started = Instant::now();
    let session =
        Arc::new(WalkSession::open(&dir, &AvatarSpec::default(), &[1], 0).expect("session"));
    eprintln!("session ready in {:.1} s", started.elapsed().as_secs_f64());

    // §2.5 steps 1–2.
    let t = Instant::now();
    let mut lists = Vec::new();
    let candidates = soak_regions(&session.ctx, |_| true);
    for r in candidates {
        let l = land_landblocks(&session.ctx, r, threads);
        if l.records == 0 {
            continue;
        }
        eprintln!(
            "region {r}: {} landblock-data records, {} land landblocks",
            l.records,
            l.land.len()
        );
        assert!(
            !l.land.is_empty(),
            "region {r} has data but no land landblock"
        );
        lists.push((r, Arc::new(l.land)));
    }
    eprintln!(
        "land lists in {:.1} s; regions {:?}; seed {SOAK_SEED:#018x}",
        t.elapsed().as_secs_f64(),
        lists.iter().map(|(r, _)| *r).collect::<Vec<_>>()
    );
    assert!(!lists.is_empty(), "no region with landblock data");

    let n = walk_count();
    let only: Option<usize> = std::env::var("TESSERA_SOAK_ONLY")
        .ok()
        .and_then(|v| v.parse().ok());
    let runners = (threads / 2).clamp(1, 8);
    let (tx, rx) = mpsc::sync_channel::<SoakWalk>(4 * runners);
    let rx = Arc::new(Mutex::new(rx));
    let results: Arc<Mutex<Vec<(SoakWalk, SoakOutcome)>>> = Arc::new(Mutex::new(Vec::new()));
    let mut no_start: Vec<usize> = Vec::new();

    std::thread::scope(|s| {
        for _ in 0..runners {
            let rx = Arc::clone(&rx);
            let results = Arc::clone(&results);
            let session = Arc::clone(&session);
            s.spawn(move || {
                let mut world = session.world_with(soak_rings()).expect("streamer");
                loop {
                    let walk = match rx.lock().expect("queue").recv() {
                        Ok(w) => w,
                        Err(_) => break,
                    };
                    let out = run_soak_walk(&walk, &mut world, &session.setup, LOAD_WAIT);
                    results.lock().expect("results").push((walk, out));
                }
            });
        }
        // Only the runners hold the queue: once all of them have ended (a panic too), the
        // planner's send fails instead of blocking on a full queue.
        drop(rx);
        // The planner (§2.5 steps 3–5), in walk order.
        let mut planner = SoakPlanner::new(SOAK_SEED, lists.clone());
        let mut world = session.world_with(soak_rings()).expect("streamer");
        let mut sent = 0usize;
        while sent < n {
            let block = planner.peek_block().expect("a landblock");
            let c = block.center();
            let probe = PhysicsWalker::new(
                &session.setup,
                block.region(),
                [c[0], c[1], 0.0],
                yaw_of(0.0),
            );
            let around: Vec<_> = (-1..=1)
                .flat_map(|dy| (-1..=1).filter_map(move |dx| block.offset(dx, dy)))
                .collect();
            wait_blocks(&probe, &mut world, &around, LOAD_WAIT).expect("start landblocks load");
            let shape = session.setup.shape;
            let walk = planner
                .plan(|key, xy| find_start(&world, &shape, key, xy))
                .expect("a walk");
            if walk.start_z.is_none() {
                // §2.5 step 4 redraws without a bound; a landblock without any valid start
                // within MAX_START_DRAWS draws is passed over (recorded as OPEN in §2.5).
                eprintln!(
                    "walk {} {}: no valid start in {} draws; passed over",
                    walk.index, walk.block, walk.start_draws
                );
                no_start.push(walk.index);
                continue;
            }
            sent += 1;
            if only.is_none_or(|o| o == walk.index) {
                tx.send(walk).expect("runners alive");
            }
            if sent.is_multiple_of(200) {
                eprintln!(
                    "planned {sent} walks in {:.1} s",
                    started.elapsed().as_secs_f64()
                );
            }
        }
        drop(tx);
    });

    let mut results = Arc::try_unwrap(results)
        .expect("runners done")
        .into_inner()
        .expect("results");
    results.sort_by_key(|(w, _)| w.index);
    let mut failures = 0usize;
    let mut skipped = 0usize;
    let (mut steps, mut water, mut slide, mut air, mut cell, mut boundary) =
        (0u64, 0u64, 0u64, 0u64, 0u64, 0u64);
    let mut deepest: f64 = 0.0;
    let mut travelled = 0.0;
    // A digest of every end position, to compare runs (same seed, same data: same digest).
    let mut digest: u64 = 0xCBF2_9CE4_8422_2325;
    for (w, o) in &results {
        for c in o.end {
            digest = (digest ^ c.to_bits()).wrapping_mul(0x0000_0100_0000_01B3);
        }
        steps += o.steps;
        water += o.water_steps;
        slide += o.slide_steps;
        air += o.air_steps;
        cell += o.cell_steps;
        boundary += o.boundary_steps;
        deepest = deepest.max(o.max_below_terrain);
        travelled += o.travelled;
        if let Some(why) = &o.skipped {
            skipped += 1;
            eprintln!(
                "walk {} {} ({:.2}, {:.2}) skipped: {why}",
                w.index, w.block, w.start_xy[0], w.start_xy[1]
            );
        }
        if !o.failures.is_empty() {
            failures += 1;
            eprintln!(
                "walk {} {} start ({:.2}, {:.2}) heading {:.1}: {:?}",
                w.index, w.block, w.start_xy[0], w.start_xy[1], w.heading, o.failures
            );
        }
    }
    let redraws: u32 = results.iter().map(|(w, _)| w.start_draws - 1).sum();
    eprintln!(
        "{} walks ({} skipped), {steps} steps, {failures} failing; deepest below terrain {deepest:.4} m; \
         steps in water {water}, sliding {slide}, in the air {air}, in cells {cell}, at a boundary {boundary}; \
         start redraws {redraws}; passed over (no start) {no_start:?}; mean distance {:.1} m; \
         digest {digest:016x}; {:.1} s",
        results.len(),
        skipped,
        travelled / results.len().max(1) as f64,
        started.elapsed().as_secs_f64()
    );
    assert_eq!(failures, 0, "fall-throughs or non-finite states");
    assert_eq!(results.len(), only.map_or(n, |_| 1));
    assert_eq!(skipped, 0, "every walk runs");
}
