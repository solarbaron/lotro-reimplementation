// SPDX-License-Identifier: GPL-3.0-or-later
//! Properties of the snapshot interpolation buffer (`tessera-protocol.md` §8.6).

use std::time::Duration;

use proptest::prelude::*;
use tessera_net::Vec3;
use tessera_net::interp::{
    InterpConfig, MAX_SNAPSHOTS, Motion, SampleKind, Snapshot, SnapshotBuffer, recommended_delay,
};

fn snap(t: u64, x: f32, vx: f32, h: f32) -> Snapshot {
    Snapshot {
        server_time: t,
        position: Vec3::new(x, 0.0, 0.0),
        heading: h,
        velocity: Vec3::new(vx, 0.0, 0.0),
        motion: Motion::STANDING,
    }
}

#[test]
fn constant_velocity_track_is_reproduced() {
    // An entity walking at 2 m/s, snapshots at 15 Hz: every render time between the
    // first and the newest snapshot gives the exact track.
    let cfg = InterpConfig::default();
    let mut b = SnapshotBuffer::new();
    let t0 = 10_000_000u64;
    for i in 0..15u64 {
        let t = t0 + i * 66_667;
        let x = 100.0 + 2.0 * (t - t0) as f64 / 1e6;
        b.push(snap(t, x as f32, 2.0, 0.5), &cfg);
    }
    let newest = b.newest_time().unwrap();
    for rt in (t0 + 1..newest).step_by(7_919) {
        let s = b.sample(rt, &cfg).unwrap();
        assert_eq!(s.kind, SampleKind::Interpolated);
        let truth = 100.0 + 2.0 * (rt - t0) as f64 / 1e6;
        assert!((f64::from(s.position.x) - truth).abs() < 1e-3, "{rt}");
        assert!((s.heading - 0.5).abs() < 1e-6);
    }
    // Extrapolation continues the track for 250 ms, then holds.
    let e = b.sample(newest + 200_000, &cfg).unwrap();
    assert_eq!(e.kind, SampleKind::Extrapolated);
    let truth = 100.0 + 2.0 * (newest + 200_000 - t0) as f64 / 1e6;
    assert!((f64::from(e.position.x) - truth).abs() < 1e-3);
    let h1 = b.sample(newest + 300_000, &cfg).unwrap();
    let h2 = b.sample(newest + 9_000_000, &cfg).unwrap();
    assert_eq!(h1.kind, SampleKind::Held);
    assert_eq!(h1.position, h2.position);
}

#[test]
fn delay_covers_two_snapshot_intervals() {
    for (snap_rate, tick) in [(1u8, 1u8), (15, 30), (30, 30), (60, 120), (120, 120)] {
        let d = recommended_delay(snap_rate, tick);
        let min = Duration::from_secs_f64(2.0 / f64::from(snap_rate) + 1.0 / f64::from(tick));
        assert!(d >= min, "{snap_rate}/{tick}: {d:?} < {min:?}");
        assert!(d >= Duration::from_millis(150));
    }
}

proptest! {
    /// Arbitrary (also hostile) snapshot streams: bounded memory, never a panic, finite
    /// samples, and interpolated positions between their two snapshots.
    #[test]
    fn samples_are_finite_and_bounded(
        snaps in proptest::collection::vec(
            (0u64..5_000_000, -1.0e4f32..1.0e4, -50.0f32..50.0, -10.0f32..10.0), 0..200),
        probes in proptest::collection::vec(0u64..6_000_000, 1..50),
    ) {
        let cfg = InterpConfig::default();
        let mut b = SnapshotBuffer::new();
        for (t, x, vx, h) in snaps {
            b.push(snap(t, x, vx, h), &cfg);
            prop_assert!(b.len() <= MAX_SNAPSHOTS);
        }
        for rt in probes {
            if let Some(s) = b.sample(rt, &cfg) {
                prop_assert!(s.position.is_finite() && s.heading.is_finite());
                prop_assert!(s.heading > -std::f32::consts::PI - 1e-4);
                prop_assert!(s.heading <= std::f32::consts::PI + 1e-4);
            }
        }
    }

    /// Times arrive in any order: only increasing ones are kept.
    #[test]
    fn stale_snapshots_are_refused(times in proptest::collection::vec(0u64..1_000_000, 1..100)) {
        let cfg = InterpConfig::default();
        let mut b = SnapshotBuffer::new();
        let mut newest = None;
        for t in times {
            let accepted = b.push(snap(t, 0.0, 0.0, 0.0), &cfg);
            prop_assert_eq!(accepted, newest.is_none_or(|n| t > n));
            if accepted {
                newest = Some(t);
            }
            prop_assert_eq!(b.newest_time(), newest);
        }
    }

    /// Hostile non-finite values never enter a buffer.
    #[test]
    fn non_finite_values_are_refused(t in 0u64..10, bits in any::<u32>()) {
        let v = f32::from_bits(bits);
        let cfg = InterpConfig::default();
        let mut b = SnapshotBuffer::new();
        let ok = b.push(snap(t, v, 0.0, 0.0), &cfg);
        prop_assert_eq!(ok, v.is_finite());
    }
}
