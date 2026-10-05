// SPDX-License-Identifier: GPL-3.0-or-later
//! Animation sampling benchmark (ROADMAP M4 exit criterion 4: 50 animated characters at
//! 60 fps with sampling on workers).
//!
//! Samples N animated instances per frame through the [`AnimationScheduler`] on a
//! `tessera-assets` worker pool, paced at 60 Hz, and reports the main thread's per-frame
//! cost (collect results, advance animators, submit jobs, copy palettes into a renderer
//! buffer) and the worker time per frame.
//!
//! ```text
//! cargo run --release -p tessera-anim --example anim_bench -- [--instances 50,200]
//!     [--frames 240] [--workers N] [--bones 89] [--real] [--baked]
//! ```
//!
//! Synthetic clips (default) are written by our own code with `tessera-havok`'s testkit:
//! an 89-bone skeleton with idle, walk and run cycles (60 fps spline clips of degree 3).
//! With `--real` and `TESSERA_GAME_DIR` set, the most common skeleton of 60 or more bones
//! in the install and clips bound to it are used instead (found at run time; nothing about
//! them is printed beyond counts). `--baked` plays baked frames instead of curves.

use std::sync::Arc;
use std::time::{Duration, Instant};

use tessera_anim::{
    AnimationScheduler, Animator, BakedClip, BoundClip, PALETTE_CAPACITY, SchedulerConfig,
    Skeleton, Source, SpeedBlend, write_palette_mat4,
};
use tessera_assets::{PoolConfig, WorkerPool};
use tessera_havok::testkit::{
    AnimationKindSpec, AnimationSpec, BoneSpec, ClipRecordSpec, SkeletonSpec, SplineClipSpec,
    quat_from_angles,
};
use tessera_havok::{Clip, Transform};

struct Options {
    instances: Vec<usize>,
    frames: usize,
    workers: usize,
    bones: usize,
    real: bool,
    baked: bool,
}

fn parse_args() -> Options {
    let mut o = Options {
        instances: vec![50, 200],
        frames: 240,
        workers: 0,
        bones: 89,
        real: false,
        baked: false,
    };
    let args: Vec<String> = std::env::args().skip(1).collect();
    let mut i = 0;
    while i < args.len() {
        let next = |i: usize| args.get(i + 1).cloned().unwrap_or_default();
        match args[i].as_str() {
            "--instances" => {
                o.instances = next(i)
                    .split(',')
                    .filter_map(|s| s.trim().parse().ok())
                    .collect();
                i += 1;
            }
            "--frames" => {
                o.frames = next(i).parse().unwrap_or(o.frames);
                i += 1;
            }
            "--workers" => {
                o.workers = next(i).parse().unwrap_or(0);
                i += 1;
            }
            "--bones" => {
                o.bones = next(i).parse().unwrap_or(o.bones).clamp(3, 256);
                i += 1;
            }
            "--real" => o.real = true,
            "--baked" => o.baked = true,
            other => {
                eprintln!("unknown argument {other}");
                std::process::exit(2);
            }
        }
        i += 1;
    }
    o
}

fn synthetic_skeleton(n: usize) -> Arc<Skeleton> {
    let bones = (0..n)
        .map(|i| BoneSpec {
            name: Some(format!("b{i}")),
            parent: if i == 0 { -1 } else { ((i - 1) / 2) as i32 },
            reference: Transform {
                translation: if i == 0 {
                    [0.0, 0.0, 1.0]
                } else {
                    [0.02 * (i % 5) as f32, 0.12, 0.03]
                },
                ..Transform::IDENTITY
            },
            lock_translation: false,
        })
        .collect();
    let spec = SkeletonSpec {
        bones,
        overrides: Vec::new(),
    };
    Arc::new(Skeleton::from_setup_record(&spec.to_setup_prefix(0x0400_0001)).expect("skeleton"))
}

fn synthetic_clip(s: &Arc<Skeleton>, frames: u32, amplitude: f32) -> Clip {
    let n = s.bone_count();
    let reference = s.reference_local().to_vec();
    let spec = SplineClipSpec::from_frames(frames, n, 1.0 / 60.0, 256, 3, |f, j| {
        let ph = f as f32 / (frames - 1) as f32 * std::f32::consts::TAU + j as f32 * 0.1;
        Transform {
            rotation: quat_from_angles(amplitude * ph.sin(), 0.1 * ph.cos(), 0.0),
            ..reference[j]
        }
    });
    let rec = ClipRecordSpec::new(AnimationSpec::new(AnimationKindSpec::Spline(spec))).to_bytes();
    Clip::parse(&rec).expect("clip")
}

/// The most common skeleton with at least 60 bones and up to three clips bound to it.
fn real_clips() -> Option<(Arc<Skeleton>, Vec<Clip>)> {
    use std::collections::HashMap;
    let dir = std::env::var_os("TESSERA_GAME_DIR").filter(|d| !d.is_empty())?;
    let set = tessera_dat::ArchiveSet::open(std::path::Path::new(&dir), Default::default()).ok()?;
    let mut counts: HashMap<u64, (usize, Skeleton)> = HashMap::new();
    for m in set.members() {
        for e in m.archive.entries_range(0x0400_0000..=0x04FF_FFFF).flatten() {
            let Ok(bytes) = m.archive.read_entry(&e) else {
                continue;
            };
            if let Ok(s) = Skeleton::from_setup_record(&bytes)
                && s.bone_count() >= 60
            {
                counts.entry(s.content_key()).or_insert((0, s)).0 += 1;
            }
        }
    }
    let (_, (_, skel)) = counts.into_iter().max_by_key(|(k, (c, _))| (*c, *k))?;
    let skel = Arc::new(skel);
    let mut clips = Vec::new();
    'outer: for m in set.members() {
        for e in m.archive.entries_range(0x0500_0000..=0x05FF_FFFF).flatten() {
            let Ok(bytes) = m.archive.read_entry(&e) else {
                continue;
            };
            let Ok(c) = Clip::parse(&bytes) else { continue };
            if c.track_count() != skel.bone_count() || c.duration() < 0.5 {
                continue;
            }
            // Frame-0 translations agree with the reference (§3.3), on 90 % of tracks.
            let mut f0 = vec![Transform::IDENTITY; c.track_count()];
            c.animation.sample_frame(0, &mut f0);
            let ok = f0
                .iter()
                .zip(skel.reference_local())
                .filter(|(a, b)| {
                    (0..3).all(|k| (a.translation[k] - b.translation[k]).abs() < 0.002)
                })
                .count();
            if ok * 10 >= f0.len() * 9 {
                clips.push(c);
                if clips.len() == 3 {
                    break 'outer;
                }
            }
        }
    }
    (clips.len() == 3).then_some((skel, clips))
}

fn percentile(v: &mut [Duration], p: f64) -> Duration {
    if v.is_empty() {
        return Duration::ZERO;
    }
    v.sort();
    let i = ((v.len() - 1) as f64 * p).round() as usize;
    v[i]
}

fn ms(d: Duration) -> f64 {
    d.as_secs_f64() * 1000.0
}

fn main() {
    let o = parse_args();
    let (skeleton, clips, source) = match o.real.then(real_clips).flatten() {
        Some((s, c)) => (s, c, "install"),
        None => {
            if o.real {
                eprintln!(
                    "--real: TESSERA_GAME_DIR not set or no suitable clips; using synthetic clips"
                );
            }
            let s = synthetic_skeleton(o.bones);
            let c = vec![
                synthetic_clip(&s, 901, 0.05), // idle, 15 s
                synthetic_clip(&s, 61, 0.4),   // walk, 1 s
                synthetic_clip(&s, 43, 0.7),   // run, 0.7 s
            ];
            (s, c, "synthetic")
        }
    };
    let bound: Vec<Arc<BoundClip>> = clips
        .into_iter()
        .map(|c| {
            let c = Arc::new(c);
            let mut b = BoundClip::bind(c.clone(), skeleton.clone()).expect("bind");
            if o.baked
                && let Some(baked) = BakedClip::bake(&c, 64 << 20)
            {
                b = b.with_baked(Arc::new(baked)).expect("baked");
            }
            Arc::new(b)
        })
        .collect();
    let pool = WorkerPool::new(PoolConfig {
        workers: o.workers,
        ..PoolConfig::default()
    });
    println!(
        "anim_bench: {} clips ({source}), {} bones, {} workers, {} frames at 60 Hz, {}",
        bound.len(),
        skeleton.bone_count(),
        pool.workers(),
        o.frames,
        if o.baked {
            "baked frames"
        } else {
            "curve evaluation"
        }
    );
    println!(
        "{:>9} | {:>10} {:>10} {:>10} | {:>12} {:>12} | {:>8} {:>8}",
        "instances",
        "main mean",
        "main p99",
        "palette",
        "worker/frame",
        "worker/job",
        "busy",
        "jobs"
    );
    let frame_budget = Duration::from_secs_f64(1.0 / 60.0);
    for &n in &o.instances {
        let mut sched = AnimationScheduler::new(&pool, SchedulerConfig::default());
        let mut inst = Vec::with_capacity(n);
        for i in 0..n {
            let id = sched.add(skeleton.clone(), None).expect("add");
            let mut a = Animator::new(skeleton.clone());
            let mut sb = SpeedBlend::new(vec![
                (0.0, bound[0].clone()),
                (2.0, bound[1].clone()),
                (6.0, bound[2].clone()),
            ])
            .expect("speed blend");
            sb.set_param((i % 7) as f32);
            a.set_source(0, Source::Speed(sb)).expect("source");
            inst.push((id, a, i as f32 * 0.37));
        }
        let mut palette = vec![[0.0f32; 16]; PALETTE_CAPACITY];
        let mut main_times = Vec::with_capacity(o.frames);
        let mut palette_time = Duration::ZERO;
        let mut worker = Duration::ZERO;
        let mut jobs = 0usize;
        let busy_before = sched.stats().busy;
        let start = Instant::now();
        for frame in 0..o.frames as u64 {
            let t0 = Instant::now();
            let c = sched.collect();
            worker += c.worker_time;
            jobs += c.completed;
            let tp = Instant::now();
            for (id, _, _) in &inst {
                if let Some(p) = sched.pose(*id) {
                    write_palette_mat4(&p.skin, &mut palette).expect("palette");
                }
            }
            palette_time += tp.elapsed();
            let dt = 1.0 / 60.0;
            for (id, a, speed) in &mut inst {
                // Vary the ground speed so blends shift between idle, walk and run.
                *speed = (*speed + dt * 0.5) % 7.0;
                if let Some(l) = a.layer_mut(0)
                    && let Some(Source::Speed(sb)) = l.source.as_mut()
                {
                    sb.set_param(*speed);
                }
                a.advance(dt);
                sched.submit(*id, a, frame + 1).expect("submit");
            }
            let spent = t0.elapsed();
            main_times.push(spent);
            // Pace at 60 Hz: the rest of the frame (render etc.) would run here.
            let target = start + frame_budget * (frame as u32 + 1);
            let now = Instant::now();
            if target > now {
                std::thread::sleep(target - now);
            }
        }
        let tail = sched.collect_all(Duration::from_secs(10));
        worker += tail.worker_time;
        jobs += tail.completed;
        let busy = sched.stats().busy - busy_before;
        let mean = main_times.iter().sum::<Duration>() / o.frames.max(1) as u32;
        let p99 = percentile(&mut main_times, 0.99);
        println!(
            "{:>9} | {:>8.3}ms {:>8.3}ms {:>8.3}ms | {:>10.3}ms {:>10.1}us | {:>8} {:>8}",
            n,
            ms(mean),
            ms(p99),
            ms(palette_time / o.frames.max(1) as u32),
            ms(worker / o.frames.max(1) as u32),
            worker.as_secs_f64() * 1e6 / jobs.max(1) as f64,
            busy,
            jobs
        );
    }
    println!(
        "main = collect + palette copy + advance + submit per frame; worker/frame = summed \
         worker time per frame across {} workers; busy = submissions skipped because the \
         previous frame's job was not back yet",
        pool.workers()
    );
}
