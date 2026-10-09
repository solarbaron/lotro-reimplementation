// SPDX-License-Identifier: GPL-3.0-or-later
//! The worked synthetic example of `particle-runtime.md` §14: one emitter over 2 s, frame by
//! frame (§14.2, §14.3), the drawn values at clock 10.56 (§14.4) and the variants (§14.5).

mod common;

use std::sync::Arc;

use common::{clock, frame_at, worked_example};
use tessera_formats::gameplay::particles::ParticleSystem;
use tessera_particles::glam::{Mat4, Vec3};
use tessera_particles::runtime::{AttachSpec, dropped_frame, owner_scale_vectors, placement_from};
use tessera_particles::{
    CameraView, Classification, Effect, EffectManager, Environment, OriginAttachments, Placement,
    Ran2, SystemDraw, SystemSim, classify_system,
};

const EPS: f32 = 1e-4;

fn camera() -> CameraView {
    // 10 m from the owner along −Y, looking along +Y: R = +X, U = +Z.
    CameraView::look_at(Vec3::new(0.0, -10.0, 0.0), Vec3::ZERO)
}

/// Runs frames 0..=`last` (clock 10.00, 10.04, …) and returns the last frame's draw.
fn run_until(sim: &mut SystemSim, rng: &mut Ran2, last: u64, p: Placement) -> Option<SystemDraw> {
    let mut draw = None;
    for k in 0..=last {
        let now = clock(k);
        sim.place(k, now, p, &Environment::default(), rng, &OriginAttachments);
        draw = sim.draw(now, &camera(), rng);
    }
    draw
}

fn ballistic(tau: f32) -> f32 {
    2.0 * tau - 4.9 * tau * tau
}

#[test]
fn start_and_classification() {
    let ps = worked_example();
    // §14.2: v = max(0, (4 div 2) × 0.25) + 1.0 + 0.25 − 0 = 1.75 ≥ 0: finite.
    let (class, max) = classify_system(&ps);
    assert_eq!(class, Classification::Finite);
    assert!((max - 1.75).abs() < 1e-9, "{max}");
    let mut rng = Ran2::new(1);
    let mut sim = SystemSim::new(Arc::new(ps), clock(0));
    sim.place(
        0,
        clock(0),
        Placement::default(),
        &Environment::default(),
        &mut rng,
        &OriginAttachments,
    );
    assert_eq!(sim.start(), Some(10.0));
    assert!((sim.last_update(0).unwrap() - 10.25).abs() < 1e-12);
}

#[test]
fn timeline() {
    let mut rng = Ran2::new(7);
    let mut sim = SystemSim::new(Arc::new(worked_example()), clock(0));
    let p = Placement::default();
    // 10.00–10.24: nothing.
    run_until(&mut sim, &mut rng, frame_at(10.24), p);
    assert_eq!(sim.live_particles(), 0);
    // 10.28: P1, P2 spawned at 10.25, at the origin; no particle update yet.
    run_until_from(&mut sim, &mut rng, frame_at(10.28), frame_at(10.28), p);
    let ps = sim.particles(0);
    assert_eq!(ps.len(), 2);
    for q in ps {
        assert!((q.spawn_time - 10.25).abs() < 1e-12);
        assert!(q.position.length() < 1e-6);
    }
    assert_eq!(sim.emitted(0), 2);
    assert!((sim.last_update(0).unwrap() - 10.25).abs() < 1e-12);
    // 10.36: particle update, ages 0.11.
    run_until_from(&mut sim, &mut rng, frame_at(10.32), frame_at(10.36), p);
    assert!((sim.last_update(0).unwrap() - 10.36).abs() < 1e-9);
    let z = sim.particles(0)[0].position.z;
    assert!((z - ballistic(0.11)).abs() < EPS, "{z}");
    // 10.52: P3, P4 spawned at 10.50, advanced to 10.48 (age −0.02).
    run_until_from(&mut sim, &mut rng, frame_at(10.40), frame_at(10.52), p);
    let ps = sim.particles(0);
    assert_eq!(ps.len(), 4);
    assert_eq!(sim.emitted(0), 4);
    for q in &ps[2..] {
        assert!((q.spawn_time - 10.50).abs() < 1e-9);
        assert!((q.position.z - (-0.04196)).abs() < EPS, "{}", q.position.z);
    }
    // 10.56: the drawn values of §14.4.
    let draw = run_until_from(&mut sim, &mut rng, frame_at(10.56), frame_at(10.56), p)
        .expect("drawn at 10.56");
    check_drawn(&draw);
    // 10.76 and later: room 0, then no emission ever again.
    run_until_from(&mut sim, &mut rng, frame_at(10.60), frame_at(11.28), p);
    assert_eq!(sim.live_particles(), 4);
    assert_eq!(sim.emitted(0), 4);
    // 11.32: P1, P2 die; P4 moves into slot 0, P3 into slot 1 (§9.2, RT-I8).
    run_until_from(&mut sim, &mut rng, frame_at(11.32), frame_at(11.32), p);
    let serials: Vec<u32> = sim.particles(0).iter().map(|q| q.serial).collect();
    assert_eq!(serials, vec![3, 2]);
    // 11.56: P3, P4 die.
    run_until_from(&mut sim, &mut rng, frame_at(11.36), frame_at(11.56), p);
    assert_eq!(sim.live_particles(), 0);
    // The end test passes from the first frame with now > 11.75.
    run_until_from(&mut sim, &mut rng, frame_at(11.60), frame_at(11.72), p);
    assert!(!sim.has_ended(clock(frame_at(11.72))));
    assert!(sim.has_ended(clock(frame_at(11.76))));
}

fn run_until_from(
    sim: &mut SystemSim,
    rng: &mut Ran2,
    first: u64,
    last: u64,
    p: Placement,
) -> Option<SystemDraw> {
    let mut draw = None;
    for k in first..=last {
        let now = clock(k);
        sim.place(k, now, p, &Environment::default(), rng, &OriginAttachments);
        draw = sim.draw(now, &camera(), rng);
    }
    draw
}

/// §14.4: interpolation factor 0.8, fade envelope 0.62.
fn check_drawn(draw: &SystemDraw) {
    assert_eq!(draw.instances.len(), 1);
    let inst = &draw.instances[0];
    assert!((inst.opacity - 1.0).abs() < 1e-6);
    assert_eq!(inst.quads(), 4);
    assert_eq!(inst.triangles(), 0);
    assert_eq!(draw.sorting_priority, 5);
    // P1, P2 then P3, P4 in storage order.
    let expected = [
        (0.19277, 0.324, [1.0, 0.69, 0.69, 0.4278]),
        (0.19277, 0.324, [1.0, 0.69, 0.69, 0.4278]),
        (-0.04196, 0.224, [1.0, 0.94, 0.94, 0.5828]),
        (-0.04196, 0.224, [1.0, 0.94, 0.94, 0.5828]),
    ];
    for (q, (z, w, colour)) in inst.vertices.chunks(4).zip(expected) {
        let centre = q
            .iter()
            .fold(Vec3::ZERO, |a, v| a + Vec3::from_array(v.position))
            / 4.0;
        assert!((centre.z - z).abs() < EPS, "z {} vs {z}", centre.z);
        assert!(centre.x.abs() < EPS && centre.y.abs() < EPS);
        // Vertex 0 top-left, 3 top-right: width along R = +X, height along U = +Z.
        let width = q[3].position[0] - q[0].position[0];
        let height = q[0].position[2] - q[1].position[2];
        assert!((width - w).abs() < EPS, "width {width} vs {w}");
        assert!((height - w).abs() < EPS, "height {height} vs {w}");
        for (a, b) in q[0].colour.iter().zip(colour) {
            assert!(
                (a - b).abs() < 1e-3,
                "colour {:?} vs {colour:?}",
                q[0].colour
            );
        }
        // Corner texture coordinates (§8.11).
        let uv: Vec<[f32; 2]> = q.iter().map(|v| v.uv).collect();
        assert_eq!(uv, vec![[0.0, 0.0], [0.0, 1.0], [1.0, 1.0], [1.0, 0.0]]);
    }
}

/// §14.5 "Owner scale": render scale (2, 2, 2) and `Model_ParticleScale` 0.5.
#[test]
fn owner_scale_variants() {
    let (a, b) = owner_scale_vectors(Vec3::splat(2.0), Some(0.5));
    assert_eq!((a, b), (Vec3::splat(2.0), Vec3::splat(0.5)));
    let p = Placement {
        scale_a: a,
        scale_b: b,
        ..Placement::default()
    };
    let width_at_1056 = |mode: u32| {
        let mut ps = worked_example();
        ps.scale_mode = mode;
        let mut rng = Ran2::new(3);
        let mut sim = SystemSim::new(Arc::new(ps), clock(0));
        let draw = run_until(&mut sim, &mut rng, frame_at(10.56), p).expect("drawn");
        let q = &draw.instances[0].vertices[..4];
        q[3].position[0] - q[0].position[0]
    };
    // Total: S = (1, 1, 1), m = 1.
    assert!((width_at_1056(1) - 0.324).abs() < EPS);
    // VDescOnly: m = 0.5.
    assert!((width_at_1056(2) - 0.162).abs() < EPS);
    // None: m = 1.
    assert!((width_at_1056(3) - 0.324).abs() < EPS);
    // Absent or non-float particle scale is 1.
    assert_eq!(owner_scale_vectors(Vec3::ONE, None).1, Vec3::ONE);
    assert_eq!(owner_scale_vectors(Vec3::ONE, Some(f32::NAN)).1, Vec3::ONE);
}

/// §14.5 "Never-ending": stopped at 11.00, lingers, removed at the first sweep after 17.00;
/// with `ImmediateDisposal` every particle disappears at 11.00.
#[test]
fn never_ending_variant_lingers_six_seconds() {
    let mut ps = worked_example();
    ps.emitters[0].emission_limit = 0;
    assert_eq!(classify_system(&ps).0, Classification::NeverEnding);
    let desc = Arc::new(ps);
    for immediate in [false, true] {
        let mut mgr = EffectManager::new();
        let mut rng = Ran2::new(5);
        let env = Environment::default();
        let (handle, key) = mgr
            .start_attached(
                1,
                0x3900_0001,
                clock(0),
                || {
                    Some(Effect::Particles(SystemSim::new(
                        Arc::clone(&desc),
                        clock(0),
                    )))
                },
                AttachSpec::default(),
            )
            .expect("started");
        let mut removed_at = None;
        for k in 0..=frame_at(18.0) {
            let now = clock(k);
            mgr.begin_frame(k, now);
            if mgr.effect(key).is_none() {
                removed_at = Some(now);
                break;
            }
            mgr.place_owner(
                1,
                k,
                now,
                Mat4::IDENTITY,
                Vec3::ONE,
                Vec3::ONE,
                1.0,
                &|_| None,
                &env,
                &mut rng,
                &OriginAttachments,
            );
            mgr.place_lingering(k, now, &env, &mut rng, &OriginAttachments);
            mgr.housekeep(1, now);
            if k == frame_at(11.0) {
                let live = mgr
                    .effect(key)
                    .and_then(Effect::particles)
                    .map_or(0, SystemSim::live_particles);
                assert!(live > 0);
                mgr.stop(1, handle, immediate, now);
                if immediate {
                    assert!(mgr.effect(key).is_none(), "disposed at once");
                    assert!(mgr.lingering().is_empty());
                    removed_at = Some(now);
                    break;
                }
                assert_eq!(mgr.lingering().len(), 1);
            }
            if k > frame_at(11.0)
                && let Some(s) = mgr.effect(key).and_then(Effect::particles)
            {
                // No emission after the stop.
                assert_eq!(s.stopped_at(), Some(clock(frame_at(11.0))));
            }
        }
        let t = removed_at.expect("removed");
        if immediate {
            assert!((t - 11.0).abs() < 1e-9);
        } else {
            // RT-I5: the first sweep more than 6 s after the stop.
            assert!(t > 17.0 && t < 17.0 + 0.041, "{t}");
        }
    }
}

/// §14.5 "Dropped": `BoneOffset` (0, 0, 1) places the effect at about (0, 0, 2).
#[test]
fn dropped_variant_offsets_twice() {
    let attachment = Mat4::from_translation(Vec3::new(0.0, 0.0, 1.0));
    let frame = dropped_frame(attachment, Vec3::new(0.0, 0.0, 1.0));
    let p = placement_from(frame, Vec3::ONE, Vec3::ONE, 1.0);
    assert!(p.position.abs_diff_eq(Vec3::new(0.0, 0.0, 2.0), 1e-6));
    // Placed once; removed by its end test after 11.75.
    let mut mgr = EffectManager::new();
    let key = mgr.drop_effect(
        0x3900_0001,
        Effect::Particles(SystemSim::new(Arc::new(worked_example()), clock(0))),
        p,
        Classification::Finite,
        clock(0),
    );
    let mut rng = Ran2::new(9);
    let env = Environment::default();
    let mut removed = None;
    for k in 0..=frame_at(12.5) {
        let now = clock(k);
        mgr.begin_frame(k, now);
        if mgr.effect(key).is_none() {
            removed = Some(now);
            break;
        }
        mgr.place_lingering(k, now, &env, &mut rng, &OriginAttachments);
        if k == frame_at(10.52) {
            let s = mgr
                .effect(key)
                .and_then(Effect::particles)
                .expect("running");
            assert!(
                s.particles(0)
                    .iter()
                    .all(|q| (q.spawn_origin - Vec3::new(0.0, 0.0, 2.0)).length() < 1e-6)
            );
        }
    }
    let t = removed.expect("removed by its end test");
    assert!(t > 11.75 && t < 11.80, "{t}");
}

/// Rule §12.7 (`particle-runtime.md`, revised 2026-10-09: the removal-time reading): a
/// never-ending effect on the dropped path is not stopped at placement; it emits and is
/// removed at the first lingering sweep more than 6 s after it was placed.
#[test]
fn never_ending_dropped_effect_runs_then_goes_after_six_seconds() {
    let mut ps = worked_example();
    if let Some(e) = ps.emitters.first_mut() {
        e.emission_limit = 0;
    }
    let mut mgr = EffectManager::new();
    let key = mgr.drop_effect(
        0x3900_0001,
        Effect::Particles(SystemSim::new(Arc::new(ps), clock(0))),
        Placement::at(Vec3::ZERO),
        Classification::NeverEnding,
        clock(0),
    );
    let mut rng = Ran2::new(4);
    let env = Environment::default();
    let mut peak = 0;
    let mut removed = None;
    for k in 0..=frame_at(17.0) {
        let now = clock(k);
        mgr.begin_frame(k, now);
        let Some(s) = mgr.effect(key).and_then(Effect::particles) else {
            removed = Some(now);
            break;
        };
        assert!(s.stopped_at().is_none(), "not stopped at placement");
        peak = peak.max(s.live_particles());
        mgr.place_lingering(k, now, &env, &mut rng, &OriginAttachments);
    }
    assert!(peak > 0, "the effect emitted");
    let t = removed.expect("removed");
    assert!(t > 16.0 && t < 16.0 + 0.041, "{t}");
}

/// RT-I9: an attached finite effect is removed at most 2 s (plus one frame) after its end
/// test first passes.
#[test]
fn finite_attached_effect_is_housekept() {
    let mut mgr = EffectManager::new();
    let mut rng = Ran2::new(2);
    let env = Environment::default();
    let desc: Arc<ParticleSystem> = Arc::new(worked_example());
    let (_, key) = mgr
        .start_attached(
            7,
            0x3900_0001,
            clock(0),
            || {
                Some(Effect::Particles(SystemSim::new(
                    Arc::clone(&desc),
                    clock(0),
                )))
            },
            AttachSpec::default(),
        )
        .expect("started");
    let mut gone = None;
    for k in 0..=frame_at(15.0) {
        let now = clock(k);
        mgr.begin_frame(k, now);
        if mgr.effect(key).is_none() {
            gone = Some(now);
            break;
        }
        mgr.place_owner(
            7,
            k,
            now,
            Mat4::IDENTITY,
            Vec3::ONE,
            Vec3::ONE,
            1.0,
            &|_| None,
            &env,
            &mut rng,
            &OriginAttachments,
        );
        mgr.place_lingering(k, now, &env, &mut rng, &OriginAttachments);
        mgr.housekeep(7, now);
    }
    let t = gone.expect("removed");
    assert!(t > 11.75 && t <= 11.75 + 2.0 + 0.08, "{t}");
    assert!(mgr.component(7).is_some_and(|c| c.attachments().is_empty()));
}
