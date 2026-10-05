// SPDX-License-Identifier: GPL-3.0-or-later
//! Entity-storage benchmark of decision record `docs/decisions/0008-entity-storage.md`
//! (decision 0005): `tessera-entity`'s own store against a standalone `bevy_ecs` on the same
//! workload.
//!
//! ```text
//! cargo run --release -p tessera-entity --example entity_bench [--features bevy-bench] --
//!     [--frames 2439] [--characters 50] [--real] [--row Y] [--statics-mean M]
//!     [--rounds 1] [--no-pace] [--workers N]
//! ```
//!
//! Workload per frame (60 Hz, paced like the client):
//!
//! 1. **net**: a movement update for every character (looked up by network ID); every 120
//!    frames five characters leave and five new ones arrive.
//! 2. **stream**: the M3 straight-line flight (one region row at 1,000 m/s, default rings,
//!    at most 32 bundle integrations per frame): integrated landblocks spawn their statics,
//!    released ones despawn them. The trace is computed once, before timing, with
//!    `tessera-world`'s ring tracker, so both stores replay identical events.
//! 3. **anim**: collect finished poses, advance every animator (idle/walk/run speed blend),
//!    submit one `tessera-anim` sampling job per character to the worker pool.
//! 4. **render**: mirror new, moved and removed render instances into an instance table and
//!    copy each character's skinning palette.
//!
//! The four phases are the per-frame main-thread time. A full query over every renderable
//! (a cull-style scan) is timed separately. With `--real` and `TESSERA_GAME_DIR` set, the
//! statics of every landblock come from the install's landblock bundles (busiest row of
//! region 1 by default); only counts and timings are printed. Without it, synthetic statics
//! with `--statics-mean` per landblock are used.

mod own;
mod workload;

#[cfg(feature = "bevy-bench")]
mod bevy;

use std::time::{Duration, Instant};

use tessera_anim::{AnimationScheduler, SchedulerConfig};
use tessera_assets::{PoolConfig, WorkerPool};
use tessera_entity::Transform;

use workload::{Backend, Flight, Rig, Sink, Trace, busiest_row, real_statics};

struct Options {
    frames: usize,
    characters: usize,
    real: bool,
    row: Option<u8>,
    statics_mean: f64,
    rounds: usize,
    pace: bool,
    workers: usize,
}

fn parse_args() -> Options {
    let mut o = Options {
        frames: 2_439,
        characters: 50,
        real: false,
        row: None,
        statics_mean: 38.0,
        rounds: 1,
        pace: true,
        workers: 0,
    };
    let args: Vec<String> = std::env::args().skip(1).collect();
    let mut i = 0;
    while i < args.len() {
        let next = |i: usize| args.get(i + 1).cloned().unwrap_or_default();
        match args[i].as_str() {
            "--frames" => {
                o.frames = next(i).parse().unwrap_or(o.frames);
                i += 1;
            }
            "--characters" => {
                o.characters = next(i).parse().unwrap_or(o.characters);
                i += 1;
            }
            "--row" => {
                o.row = next(i).parse().ok();
                i += 1;
            }
            "--statics-mean" => {
                o.statics_mean = next(i).parse().unwrap_or(o.statics_mean);
                i += 1;
            }
            "--rounds" => {
                o.rounds = next(i).parse().unwrap_or(1).max(1);
                i += 1;
            }
            "--workers" => {
                o.workers = next(i).parse().unwrap_or(0);
                i += 1;
            }
            "--real" => o.real = true,
            "--no-pace" => o.pace = false,
            other => {
                eprintln!("unknown argument {other}");
                std::process::exit(2);
            }
        }
        i += 1;
    }
    o
}

#[derive(Default)]
struct PhaseTimes {
    net: Duration,
    stream: Duration,
    anim: Duration,
    render: Duration,
}

struct RunResult {
    name: &'static str,
    frames: Vec<Duration>,
    phases: PhaseTimes,
    stream_max: Duration,
    scan: Vec<Duration>,
    peak_entities: usize,
    final_counts: (usize, usize),
    sink_live: usize,
    writes: u64,
    palettes: u64,
    busy: u64,
    checksum: f64,
}

fn character_transform(focus: [f64; 3], i: usize, frame: usize) -> Transform {
    let a = i as f32 * 0.7 + frame as f32 * 0.01;
    let r = 5.0 + (i % 10) as f32 * 3.0;
    Transform::from_position_yaw(
        [
            focus[0] + 20.0 + f64::from(r * a.cos()),
            focus[1] + f64::from(r * a.sin()),
            0.0,
        ],
        a,
    )
}

fn run(
    mut b: Box<dyn Backend>,
    trace: &Trace,
    rig: &Rig,
    pool: &WorkerPool,
    o: &Options,
) -> RunResult {
    let mut sched = AnimationScheduler::new(pool, SchedulerConfig::default());
    let mut sink = Sink::new();
    let mut nets: std::collections::VecDeque<u64> = std::collections::VecDeque::new();
    let mut next_net = 1u64;
    let focus0 = trace.frames.first().map_or([0.0; 3], |f| f.focus);
    for i in 0..o.characters {
        let anim = rig.character(&mut sched, i);
        let key = sink.alloc();
        b.spawn_character(next_net, character_transform(focus0, i, 0), key, anim);
        nets.push_back(next_net);
        next_net += 1;
    }
    let dt = 1.0 / 60.0f32;
    let budget = Duration::from_secs_f64(1.0 / 60.0);
    let mut r = RunResult {
        name: b.name(),
        frames: Vec::with_capacity(trace.frames.len()),
        phases: PhaseTimes::default(),
        stream_max: Duration::ZERO,
        scan: Vec::with_capacity(trace.frames.len()),
        peak_entities: 0,
        final_counts: (0, 0),
        sink_live: 0,
        writes: 0,
        palettes: 0,
        busy: 0,
        checksum: 0.0,
    };
    let busy0 = sched.stats().busy;
    let start = Instant::now();
    for (n, f) in trace.frames.iter().enumerate() {
        let t0 = Instant::now();
        // 1. Network.
        if n > 0 && n % 120 == 0 {
            for _ in 0..5.min(nets.len()) {
                if let Some(id) = nets.pop_front() {
                    b.despawn_character(id, &mut sched, &mut sink);
                }
            }
            for i in 0..5 {
                let anim = rig.character(&mut sched, next_net as usize + i);
                let key = sink.alloc();
                b.spawn_character(next_net, character_transform(f.focus, i, n), key, anim);
                nets.push_back(next_net);
                next_net += 1;
            }
        }
        for (i, &id) in nets.iter().enumerate() {
            b.move_character(id, character_transform(f.focus, i, n));
        }
        let t1 = Instant::now();
        // 2. Streaming.
        for tag in &f.release {
            b.despawn_block(*tag, &mut sink);
        }
        for tag in &f.integrate {
            if let Some(s) = trace.blocks.get(tag) {
                b.spawn_block(*tag, s, &mut sink);
            }
        }
        let t2 = Instant::now();
        // 3. Animation.
        b.animate(&mut sched, n as u64 + 1, dt);
        let t3 = Instant::now();
        // 4. Render sync.
        b.sync_render(&mut sched, &mut sink);
        let t4 = Instant::now();
        r.phases.net += t1 - t0;
        r.phases.stream += t2 - t1;
        r.stream_max = r.stream_max.max(t2 - t1);
        r.phases.anim += t3 - t2;
        r.phases.render += t4 - t3;
        r.frames.push(t4 - t0);
        let ts = Instant::now();
        r.checksum += b.scan();
        r.scan.push(ts.elapsed());
        if n % 60 == 0 {
            r.peak_entities = r.peak_entities.max(b.counts().0);
        }
        if o.pace {
            let target = start + budget * (n as u32 + 1);
            let now = Instant::now();
            if target > now {
                std::thread::sleep(target - now);
            }
        }
    }
    sched.collect_all(Duration::from_secs(10));
    r.busy = sched.stats().busy - busy0;
    r.final_counts = b.counts();
    r.sink_live = sink.live;
    r.writes = sink.writes;
    r.palettes = sink.palettes;
    r
}

fn ms(d: Duration) -> f64 {
    d.as_secs_f64() * 1e3
}

fn pct(v: &[Duration], p: f64) -> Duration {
    let mut v = v.to_vec();
    v.sort_unstable();
    v.get(((v.len().max(1) - 1) as f64 * p).round() as usize)
        .copied()
        .unwrap_or_default()
}

fn report(r: &RunResult) {
    let n = r.frames.len().max(1) as u32;
    let mean = r.frames.iter().sum::<Duration>() / n;
    let max = r.frames.iter().max().copied().unwrap_or_default();
    let scan_mean = r.scan.iter().sum::<Duration>() / n;
    println!(
        "{:<15} | {:>7.3} {:>7.3} {:>7.3} | {:>6.3} {:>6.3} {:>6.3} {:>6.3} | {:>7.3} | {:>6.3} {:>6.3} | {:>8} {:>6}",
        r.name,
        ms(mean),
        ms(pct(&r.frames, 0.99)),
        ms(max),
        ms(r.phases.net / n),
        ms(r.phases.stream / n),
        ms(r.phases.anim / n),
        ms(r.phases.render / n),
        ms(r.stream_max),
        ms(scan_mean),
        ms(pct(&r.scan, 0.99)),
        r.peak_entities,
        r.busy,
    );
}

/// The backends compiled in: the own store, plus `bevy_ecs` with feature `bevy-bench`.
fn backends() -> Vec<Box<dyn Backend>> {
    #[cfg_attr(not(feature = "bevy-bench"), allow(unused_mut))]
    let mut v: Vec<Box<dyn Backend>> = vec![Box::new(own::Own::new())];
    #[cfg(feature = "bevy-bench")]
    v.push(Box::new(bevy::Bevy::new()));
    v
}

fn main() {
    let o = parse_args();
    let real_row = if o.real { busiest_row() } else { None };
    if o.real && real_row.is_none() {
        eprintln!("--real: TESSERA_GAME_DIR not set or unreadable; using synthetic statics");
    }
    let flight = Flight {
        region: 1,
        row: o.row.or(real_row).unwrap_or(118),
        speed: 1_000.0,
        frames: o.frames,
        max_integrations: 32,
    };
    let t = Instant::now();
    let mut source = "synthetic";
    let trace = workload::build_trace(flight, |keys| {
        if real_row.is_some()
            && let Some(v) = real_statics(keys)
        {
            source = "install";
            return v;
        }
        workload::synthetic_statics(keys, o.statics_mean)
    });
    let statics: usize = trace.blocks.values().map(Vec::len).sum();
    let drawn: usize = trace
        .blocks
        .values()
        .flat_map(|v| v.iter())
        .filter(|s| s.draws)
        .count();
    println!(
        "trace ({source} statics, {:?} to build): region {} row {}, {} frames at 60 Hz, \
         {} distinct landblocks ({} statics, {} drawn, {:.1} per landblock), {} integrations, \
         {} releases, peak {} resident landblocks with {} statics",
        t.elapsed(),
        flight.region,
        flight.row,
        trace.frames.len(),
        trace.blocks.len(),
        statics,
        drawn,
        statics as f64 / trace.blocks.len().max(1) as f64,
        trace.integrations,
        trace.releases,
        trace.peak_resident_blocks,
        trace.peak_resident_statics
    );
    let rig = workload::synthetic_rig(89);
    let pool = WorkerPool::new(PoolConfig {
        workers: o.workers,
        ..PoolConfig::default()
    });
    println!(
        "{} characters, {}-bone {} rig, {} workers, pacing {}",
        o.characters,
        rig.skeleton.bone_count(),
        rig.source,
        pool.workers(),
        if o.pace { "60 Hz" } else { "off" }
    );
    println!(
        "{:<15} | {:>7} {:>7} {:>7} | {:>6} {:>6} {:>6} {:>6} | {:>7} | {:>6} {:>6} | {:>8} {:>6}",
        "backend",
        "mean",
        "p99",
        "max",
        "net",
        "stream",
        "anim",
        "render",
        "strmax",
        "scan",
        "scan99",
        "peak ent",
        "busy"
    );
    let mut results = Vec::new();
    for _ in 0..o.rounds {
        for b in backends() {
            let r = run(b, &trace, &rig, &pool, &o);
            report(&r);
            results.push(r);
        }
    }
    println!("(milliseconds; mean/p99/max = per-frame main-thread time of net+stream+anim+render;");
    println!(" net/stream/anim/render = per-frame phase means; strmax = slowest stream phase;");
    println!(" scan = one full query over every renderable; busy = sampling jobs skipped)");
    for r in &results {
        println!(
            "check {:<15}: final entities {} renderables {} (instance table {}), instance \
             writes {}, palettes {}, scan checksum {:.6e}",
            r.name,
            r.final_counts.0,
            r.final_counts.1,
            r.sink_live,
            r.writes,
            r.palettes,
            r.checksum
        );
    }
}
