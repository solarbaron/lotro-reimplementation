// SPDX-License-Identifier: GPL-3.0-or-later
//! Tests of the streamer's lookahead (module docs of [`crate::streamer`], "Lookahead").

use std::sync::Arc;
use std::time::{Duration, Instant};

use tessera_assets::{PoolConfig, Priority, WorkerPool};

use tessera_assets::AssetError;
use tessera_formats::Did;

use super::{
    JUMP_MIN_STEP, LOOKAHEAD_MIN_SPEED, MAX_LOOKAHEAD_DISTANCE, MAX_TRACKED_SPEED, Motion,
    READMIT_MARGIN_DIVISOR, RETRY_BASE, RETRY_MAX, SLOW_JOB_HALF_LIFE, SLOW_JOB_MAX,
    STORE_MAX_COMPLETIONS, SlotState, SlowJobs, VELOCITY_MAX_SAMPLES, retry_delay, segment_blocks,
    square_segment_distance,
};
use crate::testkit::SyntheticWorld;
use crate::*;

fn key(x: u8, y: u8) -> LandblockKey {
    LandblockKey::new(1, x, y).unwrap()
}

fn centre(x: u8, y: u8) -> WorldPos {
    LocalPos::new(key(x, y), [80.0, 80.0, 0.0]).to_world()
}

fn pool() -> WorkerPool {
    WorkerPool::new(PoolConfig {
        workers: 3,
        ..PoolConfig::default()
    })
}

/// A 61 × 7 strip of landblocks without world systems.
fn strip() -> Arc<WorldContext> {
    let src = SyntheticWorld::rectangle(1, 0..=60, 0..=6).build();
    Arc::new(
        WorldContext::load(
            Arc::new(src),
            BundleOptions::default().without_world_systems(),
        )
        .expect("synthetic context"),
    )
}

/// Rings of 1 / 2 / 3 landblocks, 1 s lookahead that does not follow job times (so the
/// counts below do not depend on the machine's speed), all of it urgent (no early part).
fn config() -> StreamConfig {
    StreamConfig {
        rings: RingConfig {
            near: 160.0,
            mid: 320.0,
            far: 480.0,
            hysteresis: 80.0,
        },
        lookahead: Duration::from_secs(1),
        lookahead_early: Duration::ZERO,
        lookahead_job_margin: 0.0,
        ..StreamConfig::default()
    }
}

/// The least distance from `key`'s square to points sampled along the segment.
fn sampled_distance(key: LandblockKey, p: [f64; 2], q: [f64; 2]) -> f64 {
    (0..=4_000)
        .map(|i| {
            let t = f64::from(i) / 4_000.0;
            key.distance_to([p[0] + t * (q[0] - p[0]), p[1] + t * (q[1] - p[1])])
        })
        .fold(f64::INFINITY, f64::min)
}

/// The bundle of `k` once it is built, whether `k` is a ring member or ahead of the rings
/// ([`WorldStreamer::bundle`] sees only ring members).
fn loaded(w: &WorldStreamer, k: LandblockKey) -> Option<Arc<LandblockBundle>> {
    match w.state_of(k) {
        Some(BlockState::Ready(b)) => Some(b),
        _ => None,
    }
}

/// Updates with `velocity` until `done` holds (or 20 s pass).
fn stream_until(
    w: &mut WorldStreamer,
    focus: WorldPos,
    velocity: [f64; 2],
    done: impl Fn(&WorldStreamer) -> bool,
) -> FrameReport {
    let deadline = Instant::now() + Duration::from_secs(20);
    loop {
        let r = w.update_with_velocity(focus, velocity);
        if done(w) {
            return r;
        }
        assert!(Instant::now() < deadline, "did not stream in time: {r:?}");
        std::thread::sleep(Duration::from_millis(1));
    }
}

#[test]
fn square_segment_distance_is_exact() {
    let k = key(10, 10); // [1600, 1760]²
    let cases: [([f64; 2], [f64; 2], f64); 5] = [
        // Through the square.
        ([1500.0, 1680.0], [1900.0, 1680.0], 0.0),
        // Parallel, 100 below.
        ([1500.0, 1500.0], [1900.0, 1500.0], 100.0),
        // Ends 100 before the square.
        ([1000.0, 1680.0], [1500.0, 1680.0], 100.0),
        // A point.
        ([1680.0, 2000.0], [1680.0, 2000.0], 240.0),
        // Diagonal past the north-west corner: corner (1600, 1760) to y = x + 400.
        (
            [1400.0, 1800.0],
            [1800.0, 2200.0],
            240.0 / std::f64::consts::SQRT_2,
        ),
    ];
    for (p, q, want) in cases {
        let d = square_segment_distance(k, p, q);
        assert!((d - want).abs() < 1e-9, "{p:?} -> {q:?}: {d} != {want}");
        assert!((square_segment_distance(k, q, p) - want).abs() < 1e-9);
    }
    // Against sampling, over segments in every direction around the square.
    for i in 0..48u32 {
        let a = f64::from(i) * std::f64::consts::TAU / 48.0;
        for (r, len) in [(150.0, 300.0), (400.0, 900.0), (90.0, 30.0)] {
            let p = [1680.0 + r * a.cos(), 1680.0 + r * a.sin()];
            let q = [p[0] + len * (a * 3.0).cos(), p[1] + len * (a * 3.0).sin()];
            let d = square_segment_distance(k, p, q);
            let s = sampled_distance(k, p, q);
            assert!(
                d <= s + 1e-9 && s - d <= len / 4_000.0 + 1e-9,
                "{p:?} {q:?}: {d} vs {s}"
            );
        }
    }
}

#[test]
fn segment_blocks_finds_every_block_in_reach() {
    // Shallow, steep, diagonal, axis-parallel (both axes), reversed, a point, and paths
    // ending on block edges or leaving the grid.
    let cases: [([f64; 2], [f64; 2], f64); 9] = [
        ([1680.0, 560.0], [3280.0, 900.0], 240.0),
        ([800.0, 300.0], [1100.0, 7_900.0], 640.0),
        ([80.0, 80.0], [8_565.0, 8_565.0], 640.0),
        ([4_000.0, 2_000.0], [4_000.0, 9_000.0], 160.0),
        ([9_000.0, 4_800.0], [1_000.0, 4_800.0], 300.0),
        ([7_000.0, 6_000.0], [2_000.0, 500.0], 640.0),
        ([3_200.0, 3_200.0], [3_200.0, 3_200.0], 160.0),
        ([0.0, 9_600.0], [9_600.0, 0.0], 0.0),
        ([9_000.0, 9_000.0], [12_000.0, 11_000.0], 400.0),
    ];
    for (p, q, reach) in cases {
        let got = segment_blocks(1, p, q, reach);
        for &(k, d) in &got {
            assert!(d <= reach, "{k} at {d}");
            assert!((d - square_segment_distance(k, p, q)).abs() < 1e-12);
        }
        let mut want = 0;
        for x in 0..=90u8 {
            for y in 0..=90u8 {
                if square_segment_distance(key(x, y), p, q) <= reach {
                    want += 1;
                    assert!(
                        got.iter().any(|(k, _)| *k == key(x, y)),
                        "({x}, {y}) missing for {p:?} to {q:?}"
                    );
                }
            }
        }
        assert_eq!(got.len(), want, "{p:?} to {q:?}");
    }
    // A segment leaving the region keeps to its blocks; hostile input gives nothing.
    for &(k, _) in &segment_blocks(1, [100.0, 100.0], [-5_000.0, -9_000.0], 160.0) {
        assert!(k.x() <= 2 && k.y() <= 2, "{k}");
    }
    assert!(segment_blocks(1, [f64::NAN, 0.0], [0.0, 0.0], 160.0).is_empty());
    assert!(segment_blocks(1, [0.0, 0.0], [f64::INFINITY, 0.0], 160.0).is_empty());
    assert!(segment_blocks(1, [0.0, 0.0], [10.0, 0.0], f64::NAN).is_empty());
    assert!(segment_blocks(1, [0.0, 0.0], [10.0, 0.0], -1.0).is_empty());
    assert!(segment_blocks(1, [1e300, 1e300], [2e300, 2e300], 160.0).is_empty());
    assert!(segment_blocks(0, [0.0, 0.0], [10.0, 0.0], 160.0).is_empty());
}

#[test]
fn motion_estimates_velocity_and_restarts_on_jumps() {
    let t0 = Instant::now();
    let at = |ms: u64| t0 + Duration::from_millis(ms);
    let mut m = Motion::default();
    // Too short a span gives no estimate.
    assert_eq!(m.push(at(0), 1, [0.0, 0.0]), None);
    assert_eq!(m.push(at(20), 1, [20.0, 10.0]), None);
    // 1,000 units per second east, 500 north.
    for i in 3..=60u32 {
        let ms = u64::from(i) * 20;
        let v = m.push(at(ms), 1, [f64::from(i) * 20.0, f64::from(i) * 10.0]);
        if ms - 20 >= 50 {
            let v = v.expect("estimate");
            assert!(
                (v[0] - 1_000.0).abs() < 1e-6 && (v[1] - 500.0).abs() < 1e-6,
                "{v:?}"
            );
        }
    }
    // Only the window (and one position before it) is kept.
    assert!(m.samples.len() <= 250 / 20 + 2, "{}", m.samples.len());
    // A teleport (a step faster than the tracked speed) or a region change starts over.
    assert_eq!(
        m.push(at(1_220), 1, [1_200.0 + 2.0 * JUMP_MIN_STEP + 1.0, 600.0]),
        None
    );
    assert_eq!(m.push(at(1_240), 2, [0.0, 0.0]), None);
    assert!(m.push(at(1_300), 2, [60.0, 0.0]).is_some());
    // A long frame of a fast flight is not a jump: 3,000 units per second with a 0.43 s
    // update (1,290 units in one step) keeps the estimate.
    let mut m = Motion::default();
    let mut t = 0u64;
    for i in 0..10u32 {
        let _ = m.push(at(t), 1, [f64::from(i) * 50.0, 0.0]);
        t += 16;
    }
    let x = 9.0 * 50.0 + 1_290.0;
    t += 430 - 16;
    let v = m
        .push(at(t), 1, [x, 0.0])
        .expect("kept across the long frame");
    assert!((v[0] - 3_000.0).abs() < 200.0, "{v:?}");
    // A short step in a very short update is not one either (crossing a block edge).
    assert!(!Motion::is_jump(
        Duration::from_micros(10),
        [0.0, 0.0],
        [JUMP_MIN_STEP, 0.0]
    ));
    // A focus moved three landblocks in one 16 ms update (the caller switching between
    // places) is, and so is a step that is not finite.
    assert!(Motion::is_jump(
        Duration::from_millis(16),
        [0.0, 0.0],
        [0.0, 3.0 * LANDBLOCK_SIZE]
    ));
    // A long frame is not a jump (ARCHITECTURE §9.2): a 2 s stall of a 3,000 units per
    // second flight is a 6,000-unit step, slower than the tracked speed over its time.
    assert!(!Motion::is_jump(
        Duration::from_secs(2),
        [0.0, 0.0],
        [6_000.0, 0.0]
    ));
    // Over the same 2 s, a step faster than the tracked speed is a jump.
    let fast = MAX_TRACKED_SPEED * 2.0;
    assert!(Motion::is_jump(
        Duration::from_secs(2),
        [0.0, 0.0],
        [fast + 1.0, 0.0]
    ));
    assert!(!Motion::is_jump(
        Duration::from_secs(2),
        [0.0, 0.0],
        [fast - 1.0, 0.0]
    ));
    // The estimate keeps going across such a stall.
    let mut m = Motion::default();
    for i in 0..10u32 {
        let _ = m.push(at(u64::from(i) * 16), 1, [f64::from(i) * 48.0, 0.0]);
    }
    let v = m
        .push(at(9 * 16 + 2_000), 1, [9.0 * 48.0 + 6_000.0, 0.0])
        .expect("kept across the stall");
    assert!(v[0] > 2_000.0, "{v:?}");
    assert!(Motion::is_jump(
        Duration::from_millis(16),
        [0.0, 0.0],
        [f64::INFINITY, 0.0]
    ));
    // Switching the focus to a place a few landblocks away and back gives no velocity
    // toward it: each switch restarts the estimate.
    let mut m = Motion::default();
    for i in 0..20u64 {
        let p = if (i / 5) % 2 == 0 {
            [100.0, 100.0]
        } else {
            [100.0 + 5.0 * LANDBLOCK_SIZE, 100.0]
        };
        let v = m.push(at(i * 16), 1, p);
        assert!(
            v.is_none_or(|v| v[0].hypot(v[1]) < 1.0),
            "spurious velocity {v:?} at {i}"
        );
    }
    // Standing still gives zero.
    let mut m = Motion::default();
    m.push(at(0), 1, [5.0, 5.0]);
    assert_eq!(m.push(at(100), 1, [5.0, 5.0]), Some([0.0, 0.0]));
    // Very frequent updates keep a bounded history.
    let mut m = Motion::default();
    for i in 0..10_000u32 {
        let _ = m.push(t0 + Duration::from_micros(u64::from(i)), 1, [0.0, 0.0]);
    }
    assert!(m.samples.len() <= VELOCITY_MAX_SAMPLES);
    m.clear();
    assert!(m.samples.is_empty() && m.region.is_none());
}

#[test]
fn lookahead_loads_the_path_beyond_the_far_ring() {
    let ctx = strip();
    let pool = pool();
    let mut w = WorldStreamer::new(Arc::clone(&ctx), &pool, config()).unwrap();
    let focus = centre(10, 3); // (1680, 560)
    let v = [1_600.0, 0.0];
    let r = w.update_with_velocity(focus, v);
    assert_eq!(w.velocity(), Some(v));
    // The path ends at x = 3280: blocks within 160 of it, beyond the far ring (x ≥ 14,
    // whose west edge is 560 from the focus), are desired ahead: x 14–21, y 2–4.
    assert_eq!(r.ahead, 24, "{r:?}");
    for x in 0..=30u8 {
        for y in 0..=6u8 {
            let k = key(x, y);
            let ahead = (14..=21).contains(&x) && (2..=4).contains(&y);
            if ahead {
                assert_eq!(w.ring_of(k), Some(Ring::Far), "{k}");
            } else if w.rings().members().get(&k).is_none() {
                assert!(w.state_of(k).is_none(), "{k} desired");
            }
        }
    }
    // The corridor counts its ring members too (x 9–13 of rows 2–4 are in the rings).
    assert_eq!(r.corridor, 24 + 15, "{r:?}");
    let r = stream_until(&mut w, focus, v, |w| loaded(w, key(21, 3)).is_some());
    assert!(w.stats().ahead_requested >= 24);
    // Bundles ahead are loaded but not ring members: `resident`, `bundle` and the lookups
    // leave them out (ARCHITECTURE §9.2).
    assert!(w.is_ahead(key(21, 3)) && !w.is_ahead(key(10, 3)));
    assert!(w.bundle(key(21, 3)).is_none());
    assert!(w.terrain_height_at(&centre(21, 3)).is_none());
    assert!(w.block_props_at(&centre(21, 3)).is_none());
    assert!(w.seam_neighbours(key(20, 3))[0].is_none());
    assert!(w.terrain_height_at(&centre(10, 3)).is_some());
    assert!(
        r.ahead_resident > 0 && r.ahead_resident <= r.resident_blocks,
        "{r:?}"
    );
    assert_eq!(w.resident().count(), r.resident_blocks - r.ahead_resident);
    for (k, ring, _) in w.resident() {
        assert!(!w.is_ahead(k), "{k}");
        assert_eq!(w.rings().members().get(&k), Some(&ring), "{k}");
    }

    // Moving on, an ahead block that enters the rings keeps its bundle.
    stream_until(&mut w, focus, v, |w| loaded(w, key(14, 3)).is_some());
    let before = loaded(&w, key(14, 3)).unwrap();
    assert!(w.bundle(key(14, 3)).is_none());
    let requested = w.stats().requested;
    let r = w.update_with_velocity(centre(11, 3), v);
    assert_eq!(w.ring_of(key(14, 3)), Some(Ring::Far));
    assert!(w.rings().members().contains_key(&key(14, 3)));
    assert!(Arc::ptr_eq(&before, w.bundle(key(14, 3)).unwrap()));
    // In the rings, it is no longer ahead: `resident` and the lookups have it.
    assert!(!w.is_ahead(key(14, 3)));
    assert!(w.resident().any(|(k, _, _)| k == key(14, 3)));
    assert!(w.terrain_height_at(&centre(14, 3)).is_some());
    // Only the new column x = 22 (y 2–4) was requested ahead; (14, 3) was not again.
    assert_eq!(r.ahead, 24, "{r:?}");
    assert!(w.stats().requested - requested <= 3 + 7, "{:?}", w.stats());

    // Standing still releases what is only ahead.
    let released = w.stats().ahead_released;
    let r = w.update_with_velocity(centre(11, 3), [0.0, 0.0]);
    assert_eq!((r.ahead, r.corridor), (0, 0), "{r:?}");
    assert_eq!(w.stats().ahead_released - released, 24);
    assert!(w.state_of(key(21, 3)).is_none());
    assert!(w.state_of(key(14, 3)).is_some());
}

#[test]
fn lookahead_respects_its_settings_and_bad_input() {
    let ctx = strip();
    let pool = pool();
    let focus = centre(10, 3);
    // Off.
    let mut w = WorldStreamer::new(
        Arc::clone(&ctx),
        &pool,
        StreamConfig {
            lookahead: Duration::ZERO,
            ..config()
        },
    )
    .unwrap();
    let r = w.update_with_velocity(focus, [1_600.0, 0.0]);
    assert_eq!((r.ahead, r.corridor), (0, 0));
    // Capped by lookahead_max: 480 units of path reach x = 14 only.
    let mut w = WorldStreamer::new(
        Arc::clone(&ctx),
        &pool,
        StreamConfig {
            lookahead_max: 480.0,
            ..config()
        },
    )
    .unwrap();
    let r = w.update_with_velocity(focus, [1_600.0, 0.0]);
    assert_eq!(r.ahead, 3, "{r:?}");
    // Slower than the minimum speed, or not finite: nothing ahead.
    let r = w.update_with_velocity(focus, [LOOKAHEAD_MIN_SPEED / 2.0, 0.0]);
    assert_eq!((r.ahead, r.corridor), (0, 0));
    let r = w.update_with_velocity(focus, [f64::NAN, 0.0]);
    assert_eq!((r.ahead, w.velocity()), (0, None));
    // A rejected focus keeps the previous velocity and the path.
    let r = w.update_with_velocity(focus, [1_600.0, 0.0]);
    assert_eq!(r.ahead, 3);
    let r = w.update(WorldPos::new(1, f64::NAN, 0.0, 0.0));
    assert!(r.focus_rejected);
    assert_eq!(r.ahead, 3);
    assert_eq!(w.velocity(), Some([1_600.0, 0.0]));
    // Settling stands still; clearing forgets the velocity.
    assert!(w.settle(focus, Duration::from_secs(20)));
    assert_eq!(w.update_with_velocity(focus, [0.0, 0.0]).ahead, 0);
    w.clear();
    assert_eq!(w.velocity(), None);
    // The setting is validated.
    StreamConfig {
        lookahead_max: MAX_LOOKAHEAD_DISTANCE,
        ..config()
    }
    .validate()
    .unwrap();
    for bad in [
        f64::NAN,
        -1.0,
        f64::INFINITY,
        MAX_LOOKAHEAD_DISTANCE + LANDBLOCK_SIZE,
    ] {
        let c = StreamConfig {
            lookahead_max: bad,
            ..config()
        };
        assert!(c.validate().is_err(), "{bad}");
    }
    for bad in [
        f64::NAN,
        -0.5,
        f64::INFINITY,
        MAX_LOOKAHEAD_JOB_MARGIN + 0.1,
    ] {
        let c = StreamConfig {
            lookahead_job_margin: bad,
            ..config()
        };
        assert!(c.validate().is_err(), "{bad}");
    }
    StreamConfig::default().validate().unwrap();
}

#[test]
fn slow_job_peak_holds_the_longest_and_halves() {
    let t0 = Instant::now();
    let at = |s: u64| t0 + Duration::from_secs(s);
    let close = |a: Duration, b: Duration| (a.as_secs_f64() - b.as_secs_f64()).abs() < 1e-6;
    let mut j = SlowJobs::default();
    assert_eq!(j.value(t0), Duration::ZERO);
    j.observe(t0, Duration::from_millis(200));
    j.observe(t0, Duration::from_secs(2));
    // A shorter job does not lower the peak.
    j.observe(t0, Duration::from_millis(500));
    assert_eq!(j.value(t0), Duration::from_secs(2));
    let h = SLOW_JOB_HALF_LIFE.as_secs();
    assert!(
        close(j.value(at(h)), Duration::from_secs(1)),
        "{:?}",
        j.value(at(h))
    );
    assert!(close(j.value(at(2 * h)), Duration::from_millis(500)));
    // A job longer than the decayed peak replaces it.
    j.observe(at(h), Duration::from_millis(1_200));
    assert_eq!(j.value(at(h)), Duration::from_millis(1_200));
    // Stuck jobs are capped; a clock that goes backwards does not grow the peak.
    j.observe(at(h), Duration::from_secs(3_600));
    assert_eq!(j.value(at(h)), SLOW_JOB_MAX);
    assert_eq!(j.value(t0), SLOW_JOB_MAX);
    assert_eq!(j.value(at(u64::from(u32::MAX))), Duration::ZERO);
}

#[test]
fn slow_jobs_lengthen_the_urgent_part_not_the_path() {
    let ctx = strip();
    let pool = pool();
    let focus = centre(10, 3); // (1680, 560)
    let v = [1_600.0, 0.0];
    // A 4 s path capped at 6,400 units: it ends at x = 8,080 (blocks x 14–51 of rows 2–4
    // ahead of the rings), whatever the job times (ARCHITECTURE §9.2).
    let adaptive = StreamConfig {
        lookahead_early: Duration::from_secs(4),
        lookahead_job_margin: 1.5,
        lookahead_max: 6_400.0,
        ..config()
    };
    let mut w = WorldStreamer::new(Arc::clone(&ctx), &pool, adaptive.clone()).unwrap();
    // No slow job yet: the urgent part is the configured 1 s (blocks x 14–21).
    w.observe_job_time(Duration::from_millis(100));
    let r = w.update_with_velocity(focus, v);
    assert_eq!((r.ahead, r.urgent), (38 * 3, 8 * 3 + 15), "{r:?}");
    assert_eq!(r.lookahead, Duration::from_secs(1));
    // A 2 s job: the horizon is 1.5 × 2 s, the urgent part 4,800 long, ending at x = 6,480
    // (less the decay of a few microseconds): blocks x 14–41. The path is unchanged.
    w.observe_job_time(Duration::from_secs(2));
    let horizon = w.lookahead_horizon();
    assert!(
        horizon <= Duration::from_secs(3) && horizon > Duration::from_millis(2_999),
        "{horizon:?}"
    );
    let r = w.update_with_velocity(focus, v);
    assert_eq!((r.ahead, r.urgent), (38 * 3, 28 * 3 + 15), "{r:?}");
    assert!(r.lookahead > Duration::from_millis(2_999), "{r:?}");
    assert_eq!(w.request_priority(key(41, 3)), Some(Priority::Visible));
    assert_eq!(w.request_priority(key(42, 3)), Some(Priority::Prefetch));
    assert_eq!(w.stats().max_lookahead, r.lookahead);
    // A 9 s job: the urgent part covers the whole path, which does not grow beyond 4 s.
    w.observe_job_time(Duration::from_secs(9));
    let r = w.update_with_velocity(focus, v);
    assert_eq!((r.ahead, r.urgent), (38 * 3, 38 * 3 + 15), "{r:?}");
    assert!(w.state_of(key(51, 3)).is_some() && w.state_of(key(52, 3)).is_none());
    // Without the early part (all of the path urgent) a slow job does not lengthen it
    // either: 1 s at 1,600 units per second, blocks x 14–21.
    let mut w = WorldStreamer::new(
        Arc::clone(&ctx),
        &pool,
        StreamConfig {
            lookahead_early: Duration::ZERO,
            ..adaptive.clone()
        },
    )
    .unwrap();
    w.observe_job_time(Duration::from_secs(9));
    let r = w.update_with_velocity(focus, v);
    assert_eq!((r.ahead, r.urgent), (24, 39), "{r:?}");
    assert!(r.lookahead > Duration::from_secs(13), "{r:?}");
    // Bundles ahead are the data's: the same as those of a streamer without lookahead.
    let mut w = WorldStreamer::new(Arc::clone(&ctx), &pool, adaptive.clone()).unwrap();
    stream_until(&mut w, focus, v, |w| loaded(w, key(51, 3)).is_some());
    let far = loaded(&w, key(51, 3)).unwrap();
    let mut plain = WorldStreamer::new(Arc::clone(&ctx), &pool, config()).unwrap();
    assert!(plain.settle(centre(51, 3), Duration::from_secs(20)));
    let near = plain.bundle(key(51, 3)).unwrap();
    assert_eq!(far.data, near.data);
    assert_eq!(far.heap_bytes, near.heap_bytes);

    // With the margin at 0 the slow job changes nothing; with lookahead off neither.
    let mut w = WorldStreamer::new(Arc::clone(&ctx), &pool, config()).unwrap();
    w.observe_job_time(Duration::from_secs(9));
    assert_eq!(w.lookahead_horizon(), Duration::from_secs(1));
    assert_eq!(w.update_with_velocity(focus, v).ahead, 24);
    let mut w = WorldStreamer::new(
        Arc::clone(&ctx),
        &pool,
        StreamConfig {
            lookahead: Duration::ZERO,
            ..adaptive
        },
    )
    .unwrap();
    w.observe_job_time(Duration::from_secs(9));
    assert_eq!(w.lookahead_horizon(), Duration::ZERO);
    let r = w.update_with_velocity(focus, v);
    assert_eq!((r.ahead, r.corridor, r.lookahead), (0, 0, Duration::ZERO));
    // Real jobs feed the peak, which is never longer than the time they had.
    let mut w = WorldStreamer::new(Arc::clone(&ctx), &pool, config()).unwrap();
    let started = Instant::now();
    assert!(w.settle(focus, Duration::from_secs(20)));
    assert!(w.stats().jobs > 0);
    assert!(w.slow_job_time() <= started.elapsed());
    assert!(w.slow_job_time() >= w.stats().max_job / 2);
}

#[test]
fn ahead_blocks_yield_to_the_rings_under_the_budget() {
    let ctx = strip();
    let pool = pool();
    let focus = centre(10, 3);
    // The rings' own bytes, settled.
    let mut w = WorldStreamer::new(Arc::clone(&ctx), &pool, config()).unwrap();
    assert!(w.settle(focus, Duration::from_secs(20)));
    let rings_bytes = w.resident_bytes();
    let largest = w.resident().map(|(_, _, b)| b.heap_bytes).max().unwrap();
    let members = w.rings().members().len();
    assert_eq!(w.resident().count(), members);
    // A budget that holds the rings but not one block more.
    let mut w = WorldStreamer::new(
        Arc::clone(&ctx),
        &pool,
        StreamConfig {
            residency_budget: rings_bytes + largest / 2,
            initial_size_estimate: largest,
            always_admit_near: false,
            ..config()
        },
    )
    .unwrap();
    let v = [1_600.0, 0.0];
    let r = stream_until(&mut w, focus, v, |w| w.resident().count() == members);
    assert_eq!(r.ahead, 24);
    assert_eq!(r.ahead_resident, 0, "{r:?}");
    assert_eq!(r.resident_blocks, members, "{r:?}");
    for x in 14..=21 {
        for y in 2..=4 {
            assert!(w.is_ahead(key(x, y)));
            assert!(
                matches!(w.state_of(key(x, y)), Some(BlockState::Deferred)),
                "({x}, {y})"
            );
        }
    }
}

#[test]
fn early_part_of_the_path_is_requested_at_prefetch_priority() {
    let ctx = strip();
    let pool = pool();
    let focus = centre(10, 3); // (1680, 560)
    let v = [1_600.0, 0.0];
    let two = StreamConfig {
        lookahead_early: Duration::from_secs(2),
        ..config()
    };
    let mut w = WorldStreamer::new(Arc::clone(&ctx), &pool, two.clone()).unwrap();
    let r = w.update_with_velocity(focus, v);
    // The urgent part ends at x = 3,280 (blocks x ≤ 21), the whole path at x = 4,880
    // (blocks x ≤ 31); rows 2–4, beyond the far ring from x = 14.
    assert_eq!(r.ahead, 18 * 3, "{r:?}");
    assert_eq!(r.corridor, 18 * 3 + 15, "{r:?}");
    assert_eq!(r.urgent, 8 * 3 + 15, "{r:?}");
    assert_eq!(r.lookahead, Duration::from_secs(1));
    for x in 14..=31u8 {
        let want = if x <= 21 {
            Priority::Visible
        } else {
            Priority::Prefetch
        };
        assert_eq!(w.request_priority(key(x, 3)), Some(want), "x {x}");
    }
    assert!(w.state_of(key(32, 3)).is_none());
    // Ring members on the path keep at least their ring's priority: the near ring
    // `Immediate`, the far ring on the urgent part `Visible`.
    assert_eq!(w.request_priority(key(10, 3)), Some(Priority::Immediate));
    assert_eq!(w.request_priority(key(13, 3)), Some(Priority::Visible));
    // Moving on, early blocks become urgent: x 22–23 now lie on the urgent part.
    let r = w.update_with_velocity(centre(12, 3), v);
    assert_eq!(
        w.request_priority(key(23, 3)),
        Some(Priority::Visible),
        "{r:?}"
    );
    assert_eq!(
        w.request_priority(key(24, 3)),
        Some(Priority::Prefetch),
        "{r:?}"
    );
    // An early horizon below the urgent one is the urgent one.
    let mut w = WorldStreamer::new(
        Arc::clone(&ctx),
        &pool,
        StreamConfig {
            lookahead_early: Duration::from_millis(10),
            ..config()
        },
    )
    .unwrap();
    let r = w.update_with_velocity(focus, v);
    assert_eq!((r.ahead, r.corridor, r.urgent), (24, 39, 39), "{r:?}");
    // The early part shares the lookahead_max cap: 2,400 units end at x = 4,080.
    let mut w = WorldStreamer::new(
        Arc::clone(&ctx),
        &pool,
        StreamConfig {
            lookahead_max: 2_400.0,
            ..two
        },
    )
    .unwrap();
    let r = w.update_with_velocity(focus, v);
    assert_eq!((r.ahead, r.urgent), (13 * 3, 8 * 3 + 15), "{r:?}");
    // The early bundles are the data's: streaming them changes nothing in them.
    stream_until(&mut w, focus, v, |w| loaded(w, key(26, 3)).is_some());
    let early = loaded(&w, key(26, 3)).unwrap();
    let mut plain = WorldStreamer::new(Arc::clone(&ctx), &pool, config()).unwrap();
    assert!(plain.settle(centre(26, 3), Duration::from_secs(20)));
    let near = plain.bundle(key(26, 3)).unwrap();
    assert_eq!(early.data, near.data);
    assert_eq!(early.heap_bytes, near.heap_bytes);
}

/// A source whose reads of one record take `delay` (a landblock whose bundle is slow to
/// build, like one with a large collision set).
struct OneSlowSource {
    inner: MemoryWorldSource,
    slow: Did,
    delay: Duration,
}

impl WorldSource for OneSlowSource {
    fn read_stamped(&self, did: Did) -> std::result::Result<Option<StampedRecord>, AssetError> {
        if did == self.slow {
            std::thread::sleep(self.delay);
        }
        self.inner.read_stamped(did)
    }
}

/// Flies east along row 3 of the strip from block 5 at `speed`, with one landblock,
/// `slow`, whose bundle takes `delay` to read, and returns the updates in which `slow`
/// was in the near ring without its bundle, and the streamer.
fn fly_past_a_slow_block(
    config: StreamConfig,
    slow: LandblockKey,
    delay: Duration,
    speed: f64,
) -> (usize, WorldStreamer) {
    let src = OneSlowSource {
        inner: SyntheticWorld::rectangle(1, 0..=60, 0..=6).build(),
        slow: slow.data_did(),
        delay,
    };
    let ctx = Arc::new(
        WorldContext::load(
            Arc::new(src),
            BundleOptions::default().without_world_systems(),
        )
        .expect("synthetic context"),
    );
    let pool = pool();
    let mut w = WorldStreamer::new(ctx, &pool, config).unwrap();
    let start = centre(5, 3);
    // The flight ends a block and a half past the slow block's near-ring edge.
    let end = slow.origin()[0] - 160.0 + 400.0;
    assert!(w.settle(start, Duration::from_secs(20)));
    assert!(
        w.state_of(slow).is_none(),
        "slow block desired before the flight"
    );
    let t0 = Instant::now();
    let mut holes = 0;
    loop {
        let x = start.x + speed * t0.elapsed().as_secs_f64();
        let p = WorldPos::new(1, x.min(end), start.y, 0.0);
        let _ = w.update_with_velocity(p, [speed, 0.0]);
        if w.rings().members().get(&slow) == Some(&Ring::Near) && w.bundle(slow).is_none() {
            holes += 1;
        }
        if x >= end {
            return (holes, w);
        }
        std::thread::sleep(Duration::from_millis(2));
    }
}

/// The regression test of ROADMAP M4.5 criterion 5 (flight M3-b): a bundle that takes
/// longer than the focus needs from the far to the near ring leaves a near-ring hole
/// without lookahead's early part, and none with it.
#[test]
fn early_part_closes_the_hole_of_a_slow_bundle() {
    let slow = key(15, 3); // x 2,400 to 2,560
    // 600 units per second crosses the 320 units from the far to the near radius in
    // 0.53 s; the slow bundle takes 0.8 s, so requested at the far ring it is late. The
    // urgent part is kept tiny (10 ms) so that only the early part differs.
    let delay = Duration::from_millis(800);
    let speed = 600.0;
    let base = StreamConfig {
        lookahead: Duration::from_millis(10),
        ..config()
    };
    let (holes, w) = fly_past_a_slow_block(base.clone(), slow, delay, speed);
    assert!(
        holes > 0,
        "the slow block was never a hole without the early part"
    );
    assert_eq!(w.stats().ahead_requested, 0, "{:?}", w.stats());

    // With a 4 s early part the slow block is requested at the start, 2.3 s before it
    // reaches the near ring, at `Prefetch` priority.
    let early = StreamConfig {
        lookahead_early: Duration::from_secs(4),
        ..base
    };
    let (holes, w) = fly_past_a_slow_block(early, slow, delay, speed);
    assert_eq!(
        holes,
        0,
        "the slow block was a near-ring hole: {:?}",
        w.stats()
    );
    assert!(w.stats().ahead_requested > 0);
    assert!(w.bundle(slow).is_some() && !w.is_ahead(slow));
}

/// A raised request in flight holds the raised priority (not only the slot's wish).
#[test]
fn requests_in_flight_follow_the_path_priority() {
    let slow = key(24, 3);
    let src = OneSlowSource {
        inner: SyntheticWorld::rectangle(1, 0..=60, 0..=6).build(),
        slow: slow.data_did(),
        delay: Duration::from_secs(3),
    };
    let ctx = Arc::new(
        WorldContext::load(
            Arc::new(src),
            BundleOptions::default().without_world_systems(),
        )
        .expect("synthetic context"),
    );
    // One worker, busy with a slow block, so later requests stay queued.
    let pool = WorkerPool::new(PoolConfig {
        workers: 1,
        ..PoolConfig::default()
    });
    let two = StreamConfig {
        lookahead_early: Duration::from_secs(2),
        ..config()
    };
    let mut w = WorldStreamer::new(ctx, &pool, two).unwrap();
    // Start the slow job: standing next to it.
    let _ = w.update_with_velocity(centre(24, 3), [0.0, 0.0]);
    std::thread::sleep(Duration::from_millis(50));
    // Far away along the strip, flying east: (25..=31, 3) early, (14..=21, 3) urgent.
    let focus = centre(10, 3);
    let v = [1_600.0, 0.0];
    let _ = w.update_with_velocity(focus, v);
    for x in 14..=31u8 {
        let k = key(x, 3);
        if k == slow {
            continue;
        }
        assert!(w.is_in_flight(k), "{k} not in flight");
        let want = if x <= 21 {
            Priority::Visible
        } else {
            Priority::Prefetch
        };
        assert_eq!(w.request_priority(k), Some(want), "{k}");
    }
    // Moving on: x 22–23 become urgent while still queued, and their requests are raised.
    let _ = w.update_with_velocity(centre(12, 3), v);
    for x in [22u8, 23] {
        let k = key(x, 3);
        assert!(w.is_in_flight(k), "{k} not in flight");
        assert_eq!(w.request_priority(k), Some(Priority::Visible), "{k}");
    }
    assert!(w.is_in_flight(key(25, 3)));
    assert_eq!(w.request_priority(key(25, 3)), Some(Priority::Prefetch));
}

#[test]
fn retries_wait_the_base_delay_first_and_double() {
    assert_eq!(retry_delay(1), RETRY_BASE);
    assert_eq!(retry_delay(1), Duration::from_millis(10));
    assert_eq!(retry_delay(2), 2 * RETRY_BASE);
    assert_eq!(retry_delay(3), 4 * RETRY_BASE);
    assert_eq!(retry_delay(8), RETRY_MAX.min(128 * RETRY_BASE));
    assert_eq!(retry_delay(u32::MAX), RETRY_MAX);
    // Defensive: no failure yet gives the base delay.
    assert_eq!(retry_delay(0), RETRY_BASE);
    for n in 1..40u32 {
        assert!(retry_delay(n) <= retry_delay(n + 1));
        assert!(retry_delay(n) <= RETRY_MAX);
    }
}

#[test]
fn admitted_landblocks_without_a_request_keep_settle_waiting() {
    // An admitted landblock left without a request at the end of an update (waiting to
    // retry, whatever the clock says by then, or a request that ended without a bundle)
    // is pending: `settle` must not report the near ring settled while it is.
    let ctx = strip();
    let pool = pool();
    let mut w = WorldStreamer::new(Arc::clone(&ctx), &pool, config()).unwrap();
    let focus = centre(10, 3);
    assert!(w.settle(focus, Duration::from_secs(20)));
    let r = w.update_with_velocity(focus, [0.0; 2]);
    assert_eq!((r.pending, r.near_missing), (0, 0), "{r:?}");
    let k = key(10, 3);
    let t0 = Instant::now();
    let old = {
        let slot = w.slots.get_mut(&k).unwrap();
        assert!(slot.admit);
        // A retry due exactly at the update's clock reading: no longer waiting, yet not
        // requested in this update.
        slot.retry_after = Some(t0);
        std::mem::replace(&mut slot.state, super::SlotState::Deferred)
    };
    let mut r = FrameReport::default();
    w.release(old, &mut r);
    w.finish(t0, &mut r);
    assert_eq!(
        (r.pending, r.near_missing, r.waiting_retry),
        (1, 1, 0),
        "{r:?}"
    );
    // The next update requests it, and settling brings it back.
    assert!(w.settle(focus, Duration::from_secs(20)));
    assert!(w.bundle(k).is_some());
}

#[test]
fn frill_jobs_stay_within_the_byte_budget() {
    // Many sector jobs may start in one frame; together with the sectors held they must
    // fit the frill budget (ARCHITECTURE §9.2: "within their own byte budget"), so a frame
    // cannot start more than the budget holds and land them all over it.
    let world = SyntheticWorld {
        scenery: true,
        ..SyntheticWorld::rectangle(1, 10..=13, 20..=21)
    };
    let ctx = Arc::new(
        WorldContext::load(Arc::new(world.build()), BundleOptions::default())
            .expect("synthetic context"),
    );
    let pool = pool();
    let budget = 8 * std::mem::size_of::<FrillSector>();
    let mut w = WorldStreamer::new(
        ctx,
        &pool,
        StreamConfig {
            frills: FrillSettings {
                distance_level: 1,
                ..FrillSettings::default()
            },
            frill_budget: budget,
            max_frill_jobs_per_frame: 64,
            ..config()
        },
    )
    .unwrap();
    let here = centre(10, 20);
    assert!(w.settle(here, Duration::from_secs(30)));
    let mut peak = 0usize;
    let mut largest = 0usize;
    let mut last = FrillReport::default();
    for _ in 0..400 {
        last = w.update_frills(&here, &here);
        peak = peak.max(w.frill_bytes());
        for (_, s) in w.frill_sectors() {
            largest = largest.max(super::sector_bytes(s));
        }
        assert!(last.started <= 64);
        std::thread::sleep(Duration::from_millis(1));
    }
    assert!(last.ready > 0, "{last:?}");
    assert!(last.waiting > 0, "the budget binds: {last:?}");
    assert_eq!(last.running, 0, "{last:?}");
    assert!(
        peak <= budget + largest,
        "peak {peak} over the budget {budget} by more than one sector ({largest})"
    );
    assert_eq!(w.stats().peak_frill_bytes, peak);
}

#[test]
fn velocity_spans_the_window_plus_one_older_position() {
    // `landblock.md` §8.1 lookahead OPEN, continued: with updates further apart than the
    // quarter-second window, the estimate spans the time between two updates.
    let t0 = Instant::now();
    let at = |ms: u64| t0 + Duration::from_millis(ms);
    let mut m = Motion::default();
    assert_eq!(m.push(at(0), 1, [0.0, 0.0]), None);
    let v = m
        .push(at(400), 1, [400.0, 0.0])
        .expect("estimate over 400 ms");
    assert!((v[0] - 1_000.0).abs() < 1e-9, "{v:?}");
    assert_eq!(m.samples.len(), 2);
    let v = m.push(at(800), 1, [1_200.0, 0.0]).expect("estimate");
    assert!(
        (v[0] - 2_000.0).abs() < 1e-9,
        "only the last two count: {v:?}"
    );
    assert_eq!(m.samples.len(), 2);
}

#[test]
fn a_given_velocity_replaces_the_estimate_and_its_history() {
    let ctx = strip();
    let pool = pool();
    let mut w = WorldStreamer::new(
        Arc::clone(&ctx),
        &pool,
        StreamConfig {
            lookahead: Duration::ZERO,
            ..config()
        },
    )
    .unwrap();
    let start = centre(10, 3);
    let mut x = start.x;
    let deadline = Instant::now() + Duration::from_secs(20);
    while w.velocity().is_none() {
        assert!(Instant::now() < deadline, "no estimate");
        x += 2.0;
        w.update(WorldPos::new(1, x, start.y, 0.0));
        std::thread::sleep(Duration::from_millis(10));
    }
    assert!(!w.motion.samples.is_empty());
    w.update_with_velocity(WorldPos::new(1, x, start.y, 0.0), [100.0, 0.0]);
    assert_eq!(w.velocity(), Some([100.0, 0.0]));
    assert!(
        w.motion.samples.is_empty(),
        "the estimate's positions are discarded"
    );
    // The estimate starts over: one position is no velocity.
    w.update(WorldPos::new(1, x + 2.0, start.y, 0.0));
    assert_eq!(w.velocity(), None);
}

#[test]
fn a_rejected_focus_ignores_a_given_velocity() {
    // A rejected focus keeps the previous velocity and the estimate's positions, also when
    // the update gives a velocity (module docs, "Lookahead"; `landblock.md` §8.1 OPEN,
    // lookahead (a) continued).
    let ctx = strip();
    let pool = pool();
    let mut w = WorldStreamer::new(
        Arc::clone(&ctx),
        &pool,
        StreamConfig {
            lookahead: Duration::ZERO,
            ..config()
        },
    )
    .unwrap();
    let start = centre(10, 3);
    let mut x = start.x;
    let deadline = Instant::now() + Duration::from_secs(20);
    while w.velocity().is_none() {
        assert!(Instant::now() < deadline, "no estimate");
        x += 2.0;
        w.update(WorldPos::new(1, x, start.y, 0.0));
        std::thread::sleep(Duration::from_millis(10));
    }
    let estimate = w.velocity();
    let samples = w.motion.samples.len();
    assert!(samples > 0);
    for bad in [
        WorldPos::new(1, f64::NAN, start.y, 0.0),
        WorldPos::new(0, x, start.y, 0.0),
    ] {
        let r = w.update_with_velocity(bad, [100.0, 0.0]);
        assert!(r.focus_rejected, "{bad:?}");
        assert_eq!(w.velocity(), estimate, "{bad:?}");
        assert_eq!(w.motion.samples.len(), samples, "{bad:?}");
        let r = w.update_with_velocity(bad, [f64::NAN, 0.0]);
        assert!(r.focus_rejected, "{bad:?}");
        assert_eq!(w.velocity(), estimate, "{bad:?}");
    }
    // A given velocity with a valid focus replaces it as before.
    w.update_with_velocity(WorldPos::new(1, x, start.y, 0.0), [100.0, 0.0]);
    assert_eq!(w.velocity(), Some([100.0, 0.0]));
    // And a rejected focus keeps that one too.
    w.update_with_velocity(WorldPos::new(1, f64::INFINITY, start.y, 0.0), [5.0, 5.0]);
    assert_eq!(w.velocity(), Some([100.0, 0.0]));
}

#[test]
fn a_closure_with_an_evicted_member_needs_the_margin_as_a_whole() {
    // `landblock.md` §8.1, OPEN on closures: the 1/64 re-admission margin applies to a
    // whole seam closure when any member is an evicted bundle, never-loaded members
    // included; without an evicted member it does not apply.
    let ctx = strip();
    let pool = pool();
    let each = 1_000usize;
    let budget = 2 * each + 10;
    assert!(2 * each > budget - budget / READMIT_MARGIN_DIVISOR);
    let mut w = WorldStreamer::new(
        Arc::clone(&ctx),
        &pool,
        StreamConfig {
            residency_budget: budget,
            initial_size_estimate: each,
            always_admit_near: false,
            lookahead: Duration::ZERO,
            ..config()
        },
    )
    .unwrap();
    let focus = centre(10, 3);
    w.update_with_velocity(focus, [0.0, 0.0]);
    let k = key(10, 3);
    let east = key(11, 3);
    assert_eq!(k.seam_neighbours()[0], Some(east));
    let mut order: Vec<(f64, LandblockKey)> =
        w.slots.iter().map(|(key, s)| (s.distance, *key)).collect();
    order.sort_unstable_by(|a, b| a.0.total_cmp(&b.0).then(a.1.cmp(&b.1)));
    assert_eq!(order[0].1, k);
    // Every landblock holds nothing (failed: charged 0) but `k` and its east neighbour,
    // each charged `each`: the closure of `k` costs 2 × `each`.
    let reset = |w: &mut WorldStreamer, evicted: bool| {
        for (key, slot) in &mut w.slots {
            if let SlotState::Requested(h) = &slot.state {
                h.cancel();
            }
            slot.state = if *key == k || *key == east {
                SlotState::Deferred
            } else {
                SlotState::Failed(tessera_assets::AssetError::QueueFull { capacity: 0 })
            };
            slot.known_size = None;
            slot.retry_after = None;
        }
        if evicted {
            w.slots.get_mut(&k).unwrap().known_size = Some(each);
        }
        // A job that finished and was integrated by the first update would make the
        // running mean of real bundle sizes the never-loaded charge instead of `each`.
        w.loaded_sizes = (0, 0);
        assert_eq!(w.size_estimate(), each);
    };
    reset(&mut w, true);
    let mut report = FrameReport::default();
    w.admit(&order, &mut report, Instant::now());
    assert!(!w.slots[&k].admit && !w.slots[&east].admit);
    reset(&mut w, false);
    let mut report = FrameReport::default();
    w.admit(&order, &mut report, Instant::now());
    assert!(w.slots[&k].admit && w.slots[&east].admit);
}

#[test]
fn stream_config_defaults_are_the_recorded_choices() {
    // `landblock.md` §8.1, OPEN on streamer defaults; ARCHITECTURE §9.2 (budget, lookahead).
    let c = StreamConfig::default();
    assert_eq!(c.residency_budget, 1 << 30);
    assert_eq!(c.cache_budget, 64 << 20);
    assert_eq!(c.max_integrations_per_frame, 32);
    assert_eq!(c.frame_budget, Duration::from_millis(2));
    assert_eq!(c.initial_size_estimate, 512 << 10);
    assert_eq!(c.frill_budget, 64 << 20);
    assert_eq!(c.max_frill_jobs_per_frame, 64);
    assert_eq!(c.lookahead, Duration::from_millis(1_500));
    assert_eq!(c.lookahead_early, Duration::from_secs(4));
    assert!((c.lookahead_max - 12_800.0).abs() < f64::EPSILON);
    assert!((c.lookahead_job_margin - 1.5).abs() < f64::EPSILON);
    assert!(c.always_admit_near);
    assert_eq!(STORE_MAX_COMPLETIONS, 4_096);
}
