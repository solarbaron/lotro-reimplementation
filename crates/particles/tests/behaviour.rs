// SPDX-License-Identifier: GPL-3.0-or-later
//! Behaviour of the simulation and the run-time rules on synthetic records
//! (`particles.md` §8, §10; `particle-runtime.md` §5–§7, §9, §13). Seeded streams; checks
//! are on distributions and rules, not on values of the original's shared stream (§12
//! rule 1).

mod common;

use std::collections::HashSet;
use std::sync::Arc;

use common::worked_example;
use tessera_formats::gameplay::particles::{Curve, CurveVec, Emitter, Keyframe, ParticleSystem};
use tessera_particles::glam::{Mat4, Quat, Vec3};
use tessera_particles::runtime::{
    AttachSpec, GateOptions, OwnerRole, attachment_matrix, gate_allows, is_static_environment,
    local_matrix, water_height, water_probe_segment,
};
use tessera_particles::{
    CameraView, Classification, Effect, EffectManager, Environment, MAX_VERTICES, NoAttachments,
    OriginAttachments, ParticleNode, Placement, Ran2, SimConfig, StartPath, SystemSim,
    classify_system, start_path,
};

fn camera() -> CameraView {
    CameraView::look_at(Vec3::new(0.0, -10.0, 1.0), Vec3::ZERO)
}

/// A continuous emitter: `rate` particles per second for `life` seconds, controller 6 (no
/// motion) unless changed.
fn emitter(rate: f32, life: f32) -> Emitter {
    Emitter {
        emit_interval: 1.0 / rate,
        burst_count: 1,
        max_particles: 100_000,
        lifespan: Curve::constant(life),
        speed: Curve::constant(0.0),
        keyframes: vec![Keyframe {
            flags: 0x770,
            controller: 6,
            physics_flags: 0,
            ..Keyframe::default()
        }],
        ..Emitter::default()
    }
}

fn system(emitters: Vec<Emitter>) -> ParticleSystem {
    let mut ps = worked_example();
    ps.emitters = emitters;
    ps.time_step = 1.0 / 30.0;
    ps
}

fn run(sim: &mut SystemSim, rng: &mut Ran2, frames: u64, p: Placement) {
    for k in 0..frames {
        let now = k as f64 / 30.0;
        sim.place(k, now, p, &Environment::default(), rng, &OriginAttachments);
        let _ = sim.draw(now, &camera(), rng);
    }
}

#[test]
fn steady_rate_and_caps() {
    // 20 per second for 1.5 s: about 30 alive (particles.md §12.3).
    let mut sim = SystemSim::new(Arc::new(system(vec![emitter(20.0, 1.5)])), 0.0);
    let mut rng = Ran2::new(1);
    run(&mut sim, &mut rng, 150, Placement::default());
    let live = sim.live_particles();
    assert!((28..=31).contains(&live), "{live}");
    // max_particles caps the live count.
    let mut e = emitter(200.0, 10.0);
    e.max_particles = 50;
    let mut sim = SystemSim::new(Arc::new(system(vec![e])), 0.0);
    run(&mut sim, &mut rng, 90, Placement::default());
    assert_eq!(sim.live_particles(), 50);
    // emit_interval 0 (and hostile intervals) emit nothing by time (§8.3, §10.8).
    for interval in [0.0, -1.0, f32::NAN, f32::INFINITY] {
        let mut e = emitter(10.0, 1.0);
        e.emit_interval = interval;
        let mut sim = SystemSim::new(Arc::new(system(vec![e])), 0.0);
        run(&mut sim, &mut rng, 60, Placement::default());
        assert_eq!(sim.live_particles(), 0, "interval {interval}");
    }
    // burst_count 0 spawns nothing and the emission-limit end condition never holds.
    let mut e = emitter(10.0, 1.0);
    e.burst_count = 0;
    e.emission_limit = 5;
    let ps = system(vec![e]);
    assert!(!ps.has_ended(1e6));
    let mut sim = SystemSim::new(Arc::new(ps), 0.0);
    run(&mut sim, &mut rng, 60, Placement::default());
    assert_eq!(sim.live_particles(), 0);
}

#[test]
fn hostile_interval_is_bounded() {
    // A tiny positive interval and a huge cap: one update spawns at most the clamped cap
    // (§10.8) and the system budget.
    let mut e = emitter(1e30, 1e6);
    e.max_particles = u32::MAX;
    let config = SimConfig {
        max_live_particles: 5_000,
        ..SimConfig::default()
    };
    let mut sim = SystemSim::with_config(Arc::new(system(vec![e.clone(), e])), 0.0, config);
    let mut rng = Ran2::new(1);
    run(&mut sim, &mut rng, 5, Placement::default());
    assert!(sim.live_particles() <= 5_000);
    let draw = {
        sim.place(
            10,
            1.0,
            Placement::default(),
            &Environment::default(),
            &mut rng,
            &NoAttachments,
        );
        sim.draw(1.0, &camera(), &mut rng).expect("drawn")
    };
    assert!(draw.vertex_count() <= MAX_VERTICES);
}

#[test]
fn draw_buffer_holds_32768_vertices() {
    let e = emitter(30_000.0, 100.0);
    let mut sim = SystemSim::new(Arc::new(system(vec![e.clone(), e])), 0.0);
    let mut rng = Ran2::new(4);
    run(&mut sim, &mut rng, 15, Placement::default());
    assert!(sim.live_particles() > 8_192);
    sim.place(
        20,
        0.7,
        Placement::default(),
        &Environment::default(),
        &mut rng,
        &NoAttachments,
    );
    let draw = sim.draw(0.7, &camera(), &mut rng).expect("drawn");
    assert_eq!(draw.instances[0].vertices.len(), MAX_VERTICES);
    assert_eq!(draw.instances[0].quads(), 8_192);
}

#[test]
fn quads_are_drawn_before_triangles() {
    let mut tri = emitter(30.0, 1.0);
    tri.primitive = 1;
    let quad = emitter(30.0, 1.0);
    let mut sim = SystemSim::new(Arc::new(system(vec![tri, quad])), 0.0);
    let mut rng = Ran2::new(4);
    run(&mut sim, &mut rng, 30, Placement::default());
    sim.place(
        40,
        1.2,
        Placement::default(),
        &Environment::default(),
        &mut rng,
        &NoAttachments,
    );
    let draw = sim.draw(1.2, &camera(), &mut rng).expect("drawn");
    let inst = &draw.instances[0];
    assert!(inst.quads() > 0 && inst.triangles() > 0);
    assert_eq!(inst.quad_vertices, inst.quads() * 4);
    assert_eq!(
        inst.vertices.len(),
        inst.quad_vertices + inst.triangles() * 3
    );
}

#[test]
fn shapes_stay_in_their_bounds() {
    let mut rng = Ran2::new(11);
    for (shape, inclusive) in [
        (1, true),
        (2, true),
        (2, false),
        (3, true),
        (3, false),
        (5, true),
        (5, false),
    ] {
        let mut e = emitter(300.0, 5.0);
        e.shape = shape;
        e.inclusive = inclusive;
        e.shape_size = Curve::constant(2.0);
        e.direction = CurveVec::constant([1.0, 0.0, 0.0]);
        let mut sim = SystemSim::new(Arc::new(system(vec![e])), 0.0);
        run(&mut sim, &mut rng, 30, Placement::default());
        let ps = sim.particles(0);
        assert!(ps.len() > 200, "shape {shape}");
        for p in ps {
            let s = p.base;
            match shape {
                1 => assert!(s.y.abs() < 1e-5 && s.z.abs() < 1e-5 && (0.0..=2.0001).contains(&s.x)),
                2 => {
                    assert!(s.z.abs() < 1e-5);
                    let r = s.truncate().length();
                    if inclusive {
                        assert!(r <= 1.0001);
                    } else {
                        assert!((r - 1.0).abs() < 1e-4, "{r}");
                    }
                }
                3 => {
                    let r = s.length();
                    if inclusive {
                        assert!(r <= 1.0001);
                    } else {
                        assert!((r - 1.0).abs() < 1e-4);
                    }
                }
                _ => {
                    assert!(s.z.abs() < 1e-5 && s.x.abs() <= 1.0001 && s.y.abs() <= 1.0001);
                    if !inclusive {
                        assert!((s.x.abs() - 1.0).abs() < 1e-5 || (s.y.abs() - 1.0).abs() < 1e-5);
                    }
                }
            }
        }
    }
}

#[test]
fn spread_forms_a_cone() {
    let mut e = emitter(300.0, 5.0);
    e.speed = Curve::constant(1.0);
    e.min_spread = Curve::constant(10.0);
    e.max_spread = Curve::constant(30.0);
    let mut sim = SystemSim::new(Arc::new(system(vec![e])), 0.0);
    let mut rng = Ran2::new(12);
    run(&mut sim, &mut rng, 30, Placement::default());
    let mut x_pos = 0;
    for p in sim.particles(0) {
        let angle = p
            .velocity
            .normalize()
            .dot(Vec3::Z)
            .clamp(-1.0, 1.0)
            .acos()
            .to_degrees();
        assert!((9.99..=30.01).contains(&angle), "{angle}");
        if p.velocity.x > 0.0 {
            x_pos += 1;
        }
    }
    // The turn about the direction is uniform: both sides occur.
    let n = sim.particles(0).len();
    assert!(x_pos > n / 4 && x_pos < 3 * n / 4, "{x_pos} of {n}");
}

#[test]
fn rotated_owner_turns_direction_and_gravity() {
    // Exploding off, direction +X, owner turned 90° about Z: velocity along +Y.
    let mut e = emitter(30.0, 5.0);
    e.speed = Curve::constant(1.0);
    e.direction = CurveVec::constant([1.0, 0.0, 0.0]);
    let mut sim = SystemSim::new(Arc::new(system(vec![e])), 0.0);
    let mut rng = Ran2::new(1);
    let p = Placement {
        rotation: Quat::from_rotation_z(std::f32::consts::FRAC_PI_2),
        position: Vec3::new(5.0, 0.0, 0.0),
        ..Placement::default()
    };
    run(&mut sim, &mut rng, 10, p);
    for q in sim.particles(0) {
        assert!(q.velocity.abs_diff_eq(Vec3::Y, 1e-5), "{}", q.velocity);
        assert!(q.spawn_origin.abs_diff_eq(Vec3::new(5.0, 0.0, 0.0), 1e-6));
    }
}

#[test]
fn controllers_move_as_specified() {
    let mut rng = Ran2::new(5);
    // Controller 5: path curves at τ.
    let mut e = emitter(10.0, 5.0);
    e.keyframes[0].controller = 5;
    e.keyframes[0].position = CurveVec {
        x: Curve {
            mode: 2,
            w: [0.0, 0.0, 0.0, 0.0, 0.0, 0.0, 1.0, 0.0, 0.0, 0.0],
            keyed: None,
        },
        ..CurveVec::default()
    };
    let mut sim = SystemSim::new(Arc::new(system(vec![e])), 0.0);
    run(&mut sim, &mut rng, 31, Placement::default());
    let now_update = sim.last_update(0).unwrap();
    for p in sim.particles(0) {
        let tau = (now_update - p.spawn_time) as f32;
        // Mode 2 with w[6] = 1: x = θ = x·1 (a ramp of 1 m/s).
        assert!(
            (p.position.x - tau).abs() < 1e-3,
            "{} vs {tau}",
            p.position.x
        );
    }
    // Controller 3: points in order, counter wrapping.
    let mut e = emitter(30.0, 5.0);
    e.keyframes[0].controller = 3;
    e.keyframes[0].points = vec![[1.0, 0.0, 0.0], [2.0, 0.0, 0.0], [3.0, 0.0, 0.0]];
    let mut sim = SystemSim::new(Arc::new(system(vec![e])), 0.0);
    run(&mut sim, &mut rng, 20, Placement::default());
    let xs: Vec<f32> = sim.particles(0).iter().map(|p| p.position.x).collect();
    for (i, x) in xs.iter().enumerate() {
        assert_eq!(*x, (i % 3 + 1) as f32);
    }
    // Controller 1 walks towards the next point and never leaves the path.
    let mut e = emitter(5.0, 5.0);
    e.speed = Curve::constant(1.0);
    e.keyframes[0].controller = 1;
    e.keyframes[0].points = vec![[0.0, 0.0, 0.0], [1.0, 0.0, 0.0], [1.0, 1.0, 0.0]];
    let mut sim = SystemSim::new(Arc::new(system(vec![e])), 0.0);
    run(&mut sim, &mut rng, 60, Placement::default());
    assert!(sim.particles(0).iter().any(|p| p.position.length() > 0.01));
    for p in sim.particles(0) {
        assert!(p.position.is_finite() && p.position.z.abs() < 1e-5);
        assert!(p.position.x <= 1.0001 && p.position.y <= 1.0001);
    }
    // Controller 4 (stepped): gravity, damping, snap; finite and falling.
    let mut e = emitter(10.0, 5.0);
    e.keyframes[0].controller = 4;
    e.keyframes[0].physics_flags = 1;
    e.snap = Curve::constant(0.0);
    let mut sim = SystemSim::new(Arc::new(system(vec![e])), 0.0);
    run(&mut sim, &mut rng, 60, Placement::default());
    assert!(sim.particles(0).iter().any(|p| p.position.z < -0.1));
    // Point paths with repeated points and any controller value stay finite.
    for controller in [0, 1, 3, 4, 7, u32::MAX] {
        let mut e = emitter(30.0, 2.0);
        e.speed = Curve::constant(1e30);
        e.keyframes[0].controller = controller;
        e.keyframes[0].points = vec![[0.0; 3]; 4];
        let mut sim = SystemSim::new(Arc::new(system(vec![e])), 0.0);
        run(&mut sim, &mut rng, 30, Placement::default());
        assert!(sim.particles(0).iter().all(|p| p.position.is_finite()));
    }
}

#[test]
fn controller_key_switch_fixes_the_base() {
    // Key 0: ballistic upward at 1 m/s without gravity; key 1 at 0.5 s: static.
    let mut e = emitter(2.0, 5.0);
    e.speed = Curve::constant(1.0);
    e.keyframes[0].controller = 2;
    e.keyframes.push(Keyframe {
        time: 0.5,
        flags: 0x100,
        controller: 6,
        physics_flags: 0,
        ..Keyframe::default()
    });
    let mut sim = SystemSim::new(Arc::new(system(vec![e])), 0.0);
    let mut rng = Ran2::new(5);
    run(&mut sim, &mut rng, 60, Placement::default());
    let now = sim.last_update(0).unwrap();
    for p in sim.particles(0) {
        let age = (now - p.spawn_time) as f32;
        let expected = age.min(0.5);
        assert!(
            (p.position.z - expected).abs() < 1e-4,
            "age {age}: {}",
            p.position.z
        );
    }
}

#[test]
fn distance_emission_follows_the_path() {
    let mut e = emitter(1.0, 10.0);
    e.emit_interval = 0.0;
    e.emission_distance = 0.5;
    let mut sim = SystemSim::new(Arc::new(system(vec![e])), 0.0);
    let mut rng = Ran2::new(3);
    // Move 3 m along +X in 30 frames.
    for k in 0..=30u64 {
        let now = k as f64 / 30.0;
        let p = Placement::at(Vec3::new(k as f32 * 0.1, 0.0, 0.0));
        sim.place(k, now, p, &Environment::default(), &mut rng, &NoAttachments);
    }
    let n = sim.live_particles();
    assert!((5..=6).contains(&n), "{n}");
    for p in sim.particles(0) {
        assert!(p.position.y.abs() < 1e-5 && (0.0..=3.0001).contains(&p.position.x));
    }
    // A local-space system (Q8 Tessera choice): the anchors follow the instance's world
    // frame, and the offsets are turned back into the system's own frame.
    let mut e = emitter(1.0, 10.0);
    e.emit_interval = 0.0;
    e.emission_distance = 0.5;
    let mut ps = system(vec![e]);
    ps.world_space = false;
    let mut sim = SystemSim::new(Arc::new(ps), 0.0);
    let turn = Quat::from_rotation_z(std::f32::consts::FRAC_PI_2);
    for k in 0..=30u64 {
        let now = k as f64 / 30.0;
        let mut p = Placement::at(Vec3::new(k as f32 * 0.1, 0.0, 0.0));
        p.rotation = turn;
        sim.place(k, now, p, &Environment::default(), &mut rng, &NoAttachments);
    }
    let n = sim.live_particles();
    assert!((5..=6).contains(&n), "local space: {n}");
    for p in sim.particles(0) {
        // World offsets behind the owner (−X) are +Y in the frame turned by 90° about Z.
        assert!(
            p.position.x.abs() < 1e-4 && (-1e-4..=3.0001).contains(&p.position.y),
            "{}",
            p.position
        );
    }
    // A negative or non-finite distance is off (§10.9).
    for d in [-1.0, f32::NAN] {
        let mut e = emitter(1.0, 10.0);
        e.emit_interval = 0.0;
        e.emission_distance = d;
        let mut sim = SystemSim::new(Arc::new(system(vec![e])), 0.0);
        for k in 0..=30u64 {
            let p = Placement::at(Vec3::new(k as f32, 0.0, 0.0));
            sim.place(
                k,
                k as f64 / 30.0,
                p,
                &Environment::default(),
                &mut rng,
                &NoAttachments,
            );
        }
        assert_eq!(sim.live_particles(), 0);
    }
}

#[test]
fn far_systems_freeze_and_catch_up() {
    let mut sim = SystemSim::new(Arc::new(system(vec![emitter(30.0, 1.0)])), 0.0);
    let mut rng = Ran2::new(3);
    let far = CameraView::look_at(Vec3::new(0.0, -100.0, 0.0), Vec3::ZERO);
    let env = Environment::default();
    sim.place(0, 0.0, Placement::default(), &env, &mut rng, &NoAttachments);
    assert!(sim.draw(0.0, &far, &mut rng).is_none());
    assert!(sim.is_not_drawn());
    for k in 1..60u64 {
        sim.place(
            k,
            k as f64 / 30.0,
            Placement::default(),
            &env,
            &mut rng,
            &NoAttachments,
        );
        assert!(sim.draw(k as f64 / 30.0, &far, &mut rng).is_none());
    }
    assert_eq!(sim.live_particles(), 0, "frozen while not drawn");
    // Coming back: one update, then the draw; the catch-up keeps only living particles.
    sim.place(
        60,
        2.0,
        Placement::default(),
        &env,
        &mut rng,
        &NoAttachments,
    );
    let d = sim.draw(2.0, &camera(), &mut rng).expect("drawn again");
    assert!(!sim.is_not_drawn());
    assert!(sim.live_particles() > 0 && sim.live_particles() <= 31);
    assert!(d.vertex_count() > 0);
}

#[test]
fn fast_forward_prefills_and_restarts() {
    let mut ps = system(vec![emitter(20.0, 2.0)]);
    ps.fast_forward = 5.0;
    let mut sim = SystemSim::new(Arc::new(ps), 0.0);
    let mut rng = Ran2::new(3);
    let env = Environment::default();
    sim.place(
        0,
        100.0,
        Placement::default(),
        &env,
        &mut rng,
        &NoAttachments,
    );
    // Particles of all ages up to the lifespan from the first update.
    let n = sim.live_particles();
    assert!((38..=41).contains(&n), "{n}");
    assert_eq!(sim.start(), Some(95.0));
    // More than 100 frames without a placement: restart.
    sim.place(
        500,
        200.0,
        Placement::default(),
        &env,
        &mut rng,
        &NoAttachments,
    );
    assert_eq!(sim.start(), Some(195.0));
}

#[test]
fn physics_duration_stops_updates() {
    let mut ps = system(vec![emitter(30.0, 10.0)]);
    ps.physics_duration = 0.5;
    assert_eq!(classify_system(&ps).0, Classification::NeverEnding);
    let mut sim = SystemSim::new(Arc::new(ps), 0.0);
    let mut rng = Ran2::new(3);
    run(&mut sim, &mut rng, 60, Placement::default());
    let n = sim.live_particles();
    assert!((14..=17).contains(&n), "{n}");
}

#[test]
fn lighting_and_fade_envelope() {
    let mut ps = system(vec![emitter(30.0, 10.0)]);
    ps.use_lighting = true;
    ps.emitters[0].fade_in = 1.0;
    let mut sim = SystemSim::new(Arc::new(ps), 0.0);
    let mut rng = Ran2::new(3);
    let env = Environment {
        ambient: [0.5, 0.25, 1.0, 1.0],
        ..Environment::default()
    };
    for k in 0..15u64 {
        sim.place(
            k,
            k as f64 / 30.0,
            Placement::default(),
            &env,
            &mut rng,
            &NoAttachments,
        );
    }
    let d = sim.draw(14.0 / 30.0, &camera(), &mut rng).expect("drawn");
    let c = d.instances[0].vertices[0].colour;
    assert!((c[0] - 0.5).abs() < 1e-5 && (c[1] - 0.25).abs() < 1e-5 && (c[2] - 1.0).abs() < 1e-5);
    // Envelope 14/30 / 1.0.
    assert!((c[3] - 14.0 / 30.0).abs() < 1e-3, "{}", c[3]);
}

/// `particle-runtime.md` §12 rule 13 (`particles.md` §8.14): a lit particle's colour is
/// multiplied by the ambient colour plus each light's record colour × its attenuation at
/// the particle (1 / (1 + k d²) inside the range, nothing beyond), alpha by the ambient's
/// alpha; at most five lights count; a local-space particle is lit at its world position;
/// the owner's lights reach its attached systems through the manager.
#[test]
fn lit_systems_sum_the_owner_lights() {
    use tessera_particles::{LightList, ParticleLight};
    let grey = |ps: &mut ParticleSystem| {
        ps.use_lighting = true;
        ps.emitters[0].keyframes[0].colour =
            tessera_formats::gameplay::particles::Argb(0xFF80_8080);
    };
    let light = |x: f32, colour: [f32; 3]| ParticleLight {
        position: Vec3::new(x, 0.0, 0.0),
        colour,
        range: 10.0,
        attenuation: 0.05,
    };
    let lit_colour = |lit: bool, world_space: bool, at: Vec3, ambient: [f32; 4], l: LightList| {
        let mut ps = system(vec![emitter(30.0, 10.0)]);
        grey(&mut ps);
        ps.use_lighting = lit;
        ps.world_space = world_space;
        let mut sim = SystemSim::new(Arc::new(ps), 0.0);
        sim.set_lights(l);
        let env = Environment {
            ambient,
            ..Environment::default()
        };
        let mut rng = Ran2::new(3);
        for k in 0..5u64 {
            sim.place(
                k,
                k as f64 / 30.0,
                Placement::at(at),
                &env,
                &mut rng,
                &NoAttachments,
            );
        }
        let d = sim.draw(4.0 / 30.0, &camera(), &mut rng).expect("drawn");
        d.instances[0].vertices[0].colour
    };
    let half = 128.0 / 255.0;
    let ambient = [0.2, 0.2, 0.2, 0.5];
    // d = 2: a = 1 / (1 + 0.05 × 4) = 1 / 1.2; the second light is out of range.
    let lights = LightList::new([light(2.0, [1.0, 0.5, 0.0]), light(30.0, [1.0; 3])]);
    let a = 1.0 / 1.2;
    let c = lit_colour(true, true, Vec3::ZERO, ambient, lights);
    let expect = [half * (0.2 + a), half * (0.2 + 0.5 * a), half * 0.2];
    for i in 0..3 {
        assert!((c[i] - expect[i]).abs() < 1e-4, "{c:?} vs {expect:?}");
    }
    assert!(
        (c[3] - 0.5).abs() < 1e-4,
        "alpha × the ambient's alpha: {c:?}"
    );
    // A local-space system at (10, 0, 0) with its light moved along: the same colour.
    let at = Vec3::new(10.0, 0.0, 0.0);
    let c2 = lit_colour(true, false, at, ambient, lights.translated(at));
    for i in 0..4 {
        assert!((c2[i] - c[i]).abs() < 1e-4, "{c2:?} vs {c:?}");
    }
    // The 5-light cut: six lights at the particle add five, not six.
    let six = LightList::new((0..6).map(|_| light(0.0, [0.05; 3])));
    let c = lit_colour(true, true, Vec3::ZERO, [0.0, 0.0, 0.0, 1.0], six);
    assert!((c[0] - half * 0.25).abs() < 1e-4, "{c:?}");
    // An unlit system ignores ambient and lights.
    let c = lit_colour(false, true, Vec3::ZERO, ambient, lights);
    assert!(
        (c[0] - half).abs() < 1e-4 && (c[3] - 1.0).abs() < 1e-4,
        "{c:?}"
    );

    // Through the manager: the owner's list reaches its attached system.
    let mut ps = system(vec![emitter(30.0, 10.0)]);
    grey(&mut ps);
    let ps = Arc::new(ps);
    let mut m = EffectManager::new();
    let h = m.request(7, 0x3900_0001, 0.0);
    m.serve(
        7,
        h,
        || Some(Effect::Particles(SystemSim::new(Arc::clone(&ps), 0.0))),
        AttachSpec::default(),
    );
    m.set_owner_lights(7, &lights);
    let mut rng = Ran2::new(3);
    let env = Environment {
        ambient,
        ..Environment::default()
    };
    for k in 0..5u64 {
        m.begin_frame(k, k as f64 / 30.0);
        m.place_owner(
            7,
            k,
            k as f64 / 30.0,
            Mat4::IDENTITY,
            Vec3::ONE,
            Vec3::ONE,
            1.0,
            &|_| None,
            &env,
            &mut rng,
            &NoAttachments,
        );
    }
    let sim = m
        .effects_mut()
        .find_map(|(_, e)| e.particles_mut())
        .expect("a running system");
    assert_eq!(
        sim.lights().len(),
        2,
        "both lights are kept; the range is per particle"
    );
    let d = sim.draw(4.0 / 30.0, &camera(), &mut rng).expect("drawn");
    let c3 = d.instances[0].vertices[0].colour;
    assert!((c3[0] - expect[0]).abs() < 1e-4, "{c3:?}");
}

/// Rule 13 on the dropped path (§5.6 step 5): a dropped lit system has no owner, so the
/// manager gives every lingering system the list built at its fixed placement; an unlit
/// frame's empty list removes it again.
#[test]
fn dropped_lit_systems_get_a_light_list_at_their_placement() {
    use tessera_particles::{LightList, ParticleLight};
    let mut ps = system(vec![emitter(30.0, 10.0)]);
    ps.use_lighting = true;
    ps.emitters[0].keyframes[0].colour = tessera_formats::gameplay::particles::Argb(0xFF80_8080);
    let mut m = EffectManager::new();
    let at = Vec3::new(3.0, 4.0, 0.0);
    let key = m.drop_effect(
        0x3900_0001,
        Effect::Particles(SystemSim::new(Arc::new(ps), 0.0)),
        Placement::at(at),
        Classification::Finite,
        0.0,
    );
    let sun = [0.5, 0.5, 0.5];
    m.set_lingering_lights(|p| {
        assert_eq!(p, at, "the list is built at the placement");
        LightList::new([ParticleLight::global(sun)])
    });
    let env = Environment {
        ambient: [0.25, 0.25, 0.25, 1.0],
        ..Environment::default()
    };
    let mut rng = Ran2::new(3);
    for k in 0..5u64 {
        m.begin_frame(k, k as f64 / 30.0);
        m.place_lingering(k, k as f64 / 30.0, &env, &mut rng, &NoAttachments);
    }
    let sim = m
        .effect_mut(key)
        .and_then(|e| e.particles_mut())
        .expect("the dropped system runs");
    assert_eq!(sim.lights().len(), 1);
    let d = sim.draw(4.0 / 30.0, &camera(), &mut rng).expect("drawn");
    let c = d.instances[0].vertices[0].colour;
    let half = 128.0 / 255.0;
    assert!((c[0] - half * 0.75).abs() < 1e-4, "ambient + sun: {c:?}");
    m.set_lingering_lights(|_| LightList::EMPTY);
    let sim = m
        .effect_mut(key)
        .and_then(|e| e.particles_mut())
        .expect("running");
    assert!(sim.lights().is_empty(), "unlit frames clear the list");
}

/// The culling box of `particle-runtime.md` §9.3 (Q-RT3 Decision): the record's bounds
/// moved by the instance and grown by the largest drawn particle; none without bounds.
#[test]
fn instances_carry_their_culling_box() {
    let mut ps = system(vec![emitter(30.0, 10.0)]);
    ps.emitters[0].keyframes[0].scale_x = 0.5;
    ps.emitters[0].keyframes[0].scale_y = 0.5;
    let run_with = |ps: ParticleSystem, p: Placement| {
        let mut sim = SystemSim::new(Arc::new(ps), 0.0);
        let mut rng = Ran2::new(3);
        for k in 0..5u64 {
            sim.place(
                k,
                k as f64 / 30.0,
                p,
                &Environment::default(),
                &mut rng,
                &NoAttachments,
            );
        }
        sim.draw(4.0 / 30.0, &camera(), &mut rng).expect("drawn")
    };
    assert_eq!(
        run_with(ps.clone(), Placement::default()).instances[0].bounds,
        None
    );
    ps.has_bounds = true;
    ps.bounds_min = [-1.0, -2.0, 0.0];
    ps.bounds_max = [1.0, 2.0, 3.0];
    let place = Placement {
        rotation: Quat::from_rotation_z(std::f32::consts::FRAC_PI_2),
        ..Placement::at(Vec3::new(5.0, 0.0, 0.0))
    };
    let d = run_with(ps.clone(), place);
    let (lo, hi) = d.instances[0].bounds.expect("bounds");
    // A 0.5 m square billboard reaches √0.125 ≈ 0.3536 m from its centre.
    let g = 0.125f32.sqrt();
    let want_lo = [5.0 - 2.0 - g, -1.0 - g, 0.0 - g];
    let want_hi = [5.0 + 2.0 + g, 1.0 + g, 3.0 + g];
    for i in 0..3 {
        assert!((lo[i] - want_lo[i]).abs() < 1e-3, "{lo:?} vs {want_lo:?}");
        assert!((hi[i] - want_hi[i]).abs() < 1e-3, "{hi:?} vs {want_hi:?}");
    }
    // Hostile bounds give no box (never culled).
    ps.bounds_max = [f32::NAN, 0.0, 0.0];
    assert_eq!(run_with(ps, Placement::default()).instances[0].bounds, None);
}

/// The Q-RT3 culling box of a world-space system covers every particle drawn: a trail
/// left behind by a moving owner and the instances of every placement in the frame, so
/// that a visible part is never culled with an instance that left the view.
#[test]
fn world_space_culling_box_covers_trails_and_every_instance() {
    let mut ps = system(vec![emitter(30.0, 10.0)]);
    ps.world_space = true;
    ps.has_bounds = true;
    ps.bounds_min = [-1.0; 3];
    ps.bounds_max = [1.0; 3];
    let mut sim = SystemSim::new(Arc::new(ps), 0.0);
    let mut rng = Ran2::new(3);
    let env = Environment::default();
    // Five frames at the origin leave particles there, then the owner moves 8 m away.
    for k in 0..10u64 {
        let at = if k < 5 {
            Vec3::ZERO
        } else {
            Vec3::new(8.0, 0.0, 0.0)
        };
        sim.place(
            k,
            k as f64 / 30.0,
            Placement::at(at),
            &env,
            &mut rng,
            &NoAttachments,
        );
    }
    let d = sim.draw(9.0 / 30.0, &camera(), &mut rng).expect("drawn");
    let (lo, hi) = d.instances[0].bounds.expect("bounds");
    for v in &d.instances[0].vertices {
        for a in 0..3 {
            assert!(
                v.position[a] >= lo[a] - 1e-4 && v.position[a] <= hi[a] + 1e-4,
                "{:?} outside {lo:?}..{hi:?}",
                v.position
            );
        }
    }
    assert!(lo[0] <= 0.0 && hi[0] >= 8.0, "{lo:?}..{hi:?}");
    // Two instances in one frame: the single draw's box covers both placements.
    let mut ps = system(vec![emitter(30.0, 10.0)]);
    ps.world_space = true;
    ps.has_bounds = true;
    ps.bounds_min = [-1.0; 3];
    ps.bounds_max = [1.0; 3];
    let mut sim = SystemSim::new(Arc::new(ps), 0.0);
    for k in 0..5u64 {
        for at in [Vec3::new(-5.0, 0.0, 0.0), Vec3::new(5.0, 0.0, 0.0)] {
            sim.place(
                k,
                k as f64 / 30.0,
                Placement::at(at),
                &env,
                &mut rng,
                &NoAttachments,
            );
        }
    }
    let d = sim.draw(4.0 / 30.0, &camera(), &mut rng).expect("drawn");
    assert_eq!(d.instances.len(), 1);
    let (lo, hi) = d.instances[0].bounds.expect("bounds");
    assert!(lo[0] <= -6.0 && hi[0] >= 6.0, "{lo:?}..{hi:?}");
}

#[test]
fn local_space_systems_follow_their_instance() {
    let mut ps = system(vec![emitter(30.0, 10.0)]);
    ps.world_space = false;
    ps.emitters[0].speed = Curve::constant(0.0);
    let mut sim = SystemSim::new(Arc::new(ps), 0.0);
    let mut rng = Ran2::new(3);
    run(&mut sim, &mut rng, 10, Placement::default());
    // Two instances: the particle set is drawn through each transform.
    let env = Environment::default();
    sim.place(
        20,
        1.0,
        Placement::at(Vec3::new(-3.0, 0.0, 0.0)),
        &env,
        &mut rng,
        &NoAttachments,
    );
    sim.place(
        20,
        1.0,
        Placement::at(Vec3::new(3.0, 0.0, 0.0)),
        &env,
        &mut rng,
        &NoAttachments,
    );
    let d = sim.draw(1.0, &camera(), &mut rng).expect("drawn");
    assert_eq!(d.instances.len(), 2);
    let centre = |i: usize| {
        let v = &d.instances[i].vertices;
        v.iter()
            .fold(Vec3::ZERO, |a, v| a + Vec3::from_array(v.position))
            / v.len() as f32
    };
    assert!((centre(0).x + 3.0).abs() < 1e-3 && (centre(1).x - 3.0).abs() < 1e-3);
}

#[test]
fn attachments_need_world_space_and_a_point() {
    let mut e = emitter(30.0, 10.0);
    e.bone = 3;
    let mut sim = SystemSim::new(Arc::new(system(vec![e.clone()])), 0.0);
    let mut rng = Ran2::new(3);
    for k in 0..10u64 {
        sim.place(
            k,
            k as f64 / 30.0,
            Placement::default(),
            &Environment::default(),
            &mut rng,
            &NoAttachments,
        );
    }
    assert_eq!(
        sim.live_particles(),
        0,
        "unresolved attachment spawns nothing"
    );
    let mut ps = system(vec![e]);
    ps.world_space = false;
    let mut sim = SystemSim::new(Arc::new(ps), 0.0);
    run(&mut sim, &mut rng, 10, Placement::default());
    assert_eq!(
        sim.live_particles(),
        0,
        "local-space attached emitter spawns nothing"
    );
}

#[test]
fn same_seed_same_particles() {
    let mut e = emitter(60.0, 2.0);
    e.shape = 3;
    e.speed = Curve {
        mode: 3,
        w: [1.0, 0.0, 0.5, 0.0, 0.0, 0.0, 0.0, 0.0, 0.0, 0.0],
        keyed: None,
    };
    let desc = Arc::new(system(vec![e]));
    let positions = |seed: u32| {
        let mut sim = SystemSim::new(Arc::clone(&desc), 0.0);
        let mut rng = Ran2::new(seed);
        run(&mut sim, &mut rng, 45, Placement::default());
        sim.particles(0)
            .iter()
            .map(|p| p.position)
            .collect::<Vec<_>>()
    };
    assert_eq!(positions(9), positions(9));
    assert_ne!(positions(9), positions(10));
}

#[test]
fn node_start_gates() {
    let finite = Classification::Finite;
    let never = Classification::NeverEnding;
    let mut node = ParticleNode {
        particle_id: 0x3900_0001,
        ..ParticleNode::default()
    };
    assert_eq!(start_path(&node, finite, 1.0, 0), StartPath::Attached);
    assert_eq!(start_path(&node, finite, 0.0002, 0), StartPath::Skip);
    assert_eq!(start_path(&node, finite, 1.0, 0x40), StartPath::Skip);
    assert_eq!(start_path(&node, never, 0.0, 0x40), StartPath::Attached);
    node.drop_effect = true;
    assert_eq!(start_path(&node, finite, 1.0, 0), StartPath::Dropped);
    let none = ParticleNode::default();
    assert_eq!(start_path(&none, never, 1.0, 0), StartPath::Skip);
    // §7.1.1: the particle field wins, the fork field is the fallback.
    let both = ParticleNode {
        particle_id: 0x3900_0001,
        fork_particle_id: 0x4400_0001,
        ..ParticleNode::default()
    };
    assert_eq!(both.effect_did(), Some(0x3900_0001));
    let fork = ParticleNode {
        fork_particle_id: 0x4400_0001,
        ..ParticleNode::default()
    };
    assert_eq!(fork.effect_did(), Some(0x4400_0001));
    // Rule §12.2: the stored byte wins.
    let stored = ParticleNode {
        stored_never_ending: Some(false),
        ..ParticleNode::default()
    };
    assert_eq!(stored.classification(|| never), finite);
    assert_eq!(ParticleNode::default().classification(|| never), never);
}

#[test]
fn start_gating_options() {
    let wl: HashSet<u32> = [0x3900_0002].into_iter().collect();
    let opts = |d| GateOptions {
        dynamic_particles: d,
        static_environmental_objects: true,
    };
    use OwnerRole::*;
    assert!(gate_allows(Other, 1, &wl, opts(0)));
    assert!(!gate_allows(Other, 1, &wl, opts(1)) && gate_allows(LocalPlayer, 1, &wl, opts(1)));
    assert!(gate_allows(Selected, 1, &wl, opts(2)) && !gate_allows(Other, 1, &wl, opts(2)));
    assert!(gate_allows(Selected, 1, &wl, opts(3)) && !gate_allows(LocalPlayer, 1, &wl, opts(3)));
    assert!(!gate_allows(LocalPlayer, 1, &wl, opts(4)));
    assert!(gate_allows(Other, 0x3900_0002, &wl, opts(4)), "whitelisted");
    assert!(
        gate_allows(Other, 1, &wl, opts(9)),
        "unknown values act as 0"
    );
    let off = GateOptions {
        static_environmental_objects: false,
        ..GateOptions::default()
    };
    assert!(!gate_allows(StaticEnvironment, 0x3900_0002, &wl, off));
    assert!(is_static_environment(0x8100_0000_0000_0001));
    assert!(!is_static_environment(0x0200_0000_0000_0001));
}

#[test]
fn attachment_frames() {
    let node = ParticleNode {
        bone_type: 2,
        holding_location: 5,
        bone_offset: Vec3::new(0.0, 0.0, 1.0),
        ..ParticleNode::default()
    };
    let hold =
        Mat4::from_rotation_translation(Quat::from_rotation_z(1.0), Vec3::new(1.0, 0.0, 0.0));
    // Bone type wins: identity rotation, translation = offset.
    assert_eq!(
        attachment_matrix(&node, Some(hold)),
        Mat4::from_translation(node.bone_offset)
    );
    // Holding location: its frame, offset added unrotated.
    let h = ParticleNode {
        bone_type: 0,
        ..node.clone()
    };
    let m = attachment_matrix(&h, Some(hold));
    assert!(
        m.w_axis
            .truncate()
            .abs_diff_eq(Vec3::new(1.0, 0.0, 1.0), 1e-6)
    );
    // RT-I7: RemoveRotation keeps only the translation, for every pose.
    for angle in [0.3f32, 1.2, -2.0] {
        let bone =
            Mat4::from_rotation_translation(Quat::from_rotation_x(angle), Vec3::new(0.0, 2.0, 0.0));
        let l = local_matrix(m, Some(bone), true);
        let (_, r, t) = l.to_scale_rotation_translation();
        assert!(r.abs_diff_eq(Quat::IDENTITY, 1e-6));
        assert!(t.abs_diff_eq((bone * m).w_axis.truncate(), 1e-5));
    }
    // Water probe (§5.7).
    let (top, bottom) = water_probe_segment(Vec3::new(1.0, 2.0, 3.0), true);
    assert_eq!((top.z, bottom.z), (43.0, 2.0));
    let (top, bottom) = water_probe_segment(Vec3::new(1.0, 2.0, 3.0), false);
    assert_eq!((top.z, bottom.z), (3.0, -7.0));
    assert_eq!(
        water_height(Vec3::new(1.0, 2.0, 3.0), [0.5, 1.5, f32::NAN]),
        Some(Vec3::new(1.0, 2.0, 1.5))
    );
    assert_eq!(water_height(Vec3::ZERO, []), None);
}

#[test]
fn sharing_release_and_late_records() {
    // A shareable-kind record: shareable, local-space, unlit.
    let mut ps = system(vec![emitter(30.0, 10.0)]);
    ps.shareable = true;
    ps.world_space = false;
    let desc = Arc::new(ps);
    let make = || Some(Effect::Particles(SystemSim::new(Arc::clone(&desc), 0.0)));
    let mut mgr = EffectManager::new();
    let mut rng = Ran2::new(1);
    let env = Environment::default();
    mgr.begin_frame(0, 0.0);
    let (h1, k1) = mgr
        .start_attached(1, 0x3900_0009, 0.0, make, AttachSpec::default())
        .unwrap();
    let (h2, k2) = mgr
        .start_attached(2, 0x3900_0009, 0.0, make, AttachSpec::default())
        .unwrap();
    assert_eq!(k1, k2, "the second request shares the running copy");
    assert_eq!(mgr.running_count(), 1);
    for owner in [1, 2] {
        mgr.place_owner(
            owner,
            0,
            0.0,
            Mat4::IDENTITY,
            Vec3::ONE,
            Vec3::ONE,
            1.0,
            &|_| None,
            &env,
            &mut rng,
            &NoAttachments,
        );
    }
    assert_eq!(
        mgr.effect(k1)
            .and_then(Effect::particles)
            .map(|s| s.instances().len()),
        Some(2)
    );
    mgr.stop(1, h1, false, 0.0);
    assert!(mgr.effect(k1).is_some(), "still used by owner 2");
    // RT-I6: the shareable kind never lingers.
    mgr.stop(2, h2, false, 0.0);
    assert!(mgr.effect(k1).is_none());
    assert!(mgr.lingering().is_empty());
    // §7.5: a request stopped before its record loads is discarded.
    let h = mgr.request(3, 0x3900_0009, 0.0);
    mgr.stop(3, h, false, 0.0);
    assert!(mgr.serve(3, h, make, AttachSpec::default()).is_none());
    assert_eq!(mgr.running_count(), 0);
    // A local-space system without an instance this frame does not linger.
    let mut ps = system(vec![emitter(30.0, 10.0)]);
    ps.world_space = false;
    let local = Arc::new(ps);
    let (h, k) = mgr
        .start_attached(
            4,
            0x3900_000A,
            0.0,
            || Some(Effect::Particles(SystemSim::new(Arc::clone(&local), 0.0))),
            AttachSpec::default(),
        )
        .unwrap();
    mgr.begin_frame(5, 0.2);
    mgr.stop(4, h, false, 0.2);
    assert!(mgr.effect(k).is_none());
    // Removing an owner releases its effects.
    let (_, k) = mgr
        .start_attached(
            5,
            0x3900_000B,
            0.0,
            || {
                Some(Effect::Particles(SystemSim::new(
                    Arc::new(worked_example()),
                    0.0,
                )))
            },
            AttachSpec::default(),
        )
        .unwrap();
    mgr.remove_owner(5, 1.0);
    assert!(mgr.effect(k).is_none() || mgr.lingering().iter().any(|l| l.system == k));
}

#[test]
fn hostile_placements_are_ignored() {
    let mut sim = SystemSim::new(Arc::new(system(vec![emitter(30.0, 1.0)])), f64::NAN);
    let mut rng = Ran2::new(1);
    let env = Environment {
        wind: Vec3::splat(f32::NAN),
        ambient: [f32::NAN; 4],
    };
    let bad = Placement {
        position: Vec3::splat(f32::INFINITY),
        ..Placement::default()
    };
    sim.place(0, 0.0, bad, &env, &mut rng, &NoAttachments);
    assert!(sim.start().is_none());
    let odd = Placement {
        rotation: Quat::from_xyzw(0.0, 0.0, 0.0, 0.0),
        scale_a: Vec3::splat(f32::NAN),
        opacity: f32::NAN,
        ..Placement::default()
    };
    for k in 0..30u64 {
        sim.place(k, k as f64 / 30.0, odd, &env, &mut rng, &NoAttachments);
        let _ = sim.draw(f64::NAN, &camera(), &mut rng);
        if let Some(d) = sim.draw(k as f64 / 30.0, &camera(), &mut rng) {
            for v in d.instances.iter().flat_map(|i| &i.vertices) {
                assert!(
                    v.position
                        .iter()
                        .chain(&v.colour)
                        .chain(&v.uv)
                        .all(|c| c.is_finite())
                );
            }
        }
    }
    assert!(sim.live_particles() > 0);
}

/// Regression (found by the `particles_sim` fuzz target): a local-space system drawn
/// through an instance whose finite scale is huge overflowed to infinite vertices.
#[test]
fn huge_owner_scales_keep_vertices_finite() {
    for world_space in [false, true] {
        let mut ps = system(vec![emitter(30.0, 2.0)]);
        ps.world_space = world_space;
        ps.emitters[0].speed = Curve::constant(1.0);
        let mut sim = SystemSim::new(Arc::new(ps), 0.0);
        let mut rng = Ran2::new(1);
        let p = Placement {
            position: Vec3::new(3.0e38, -3.0e38, 1.0),
            scale_a: Vec3::splat(f32::MAX),
            scale_b: Vec3::splat(-f32::MAX),
            ..Placement::default()
        };
        let camera = CameraView::look_at(p.position - Vec3::Y * 10.0, p.position);
        for k in 0..20u64 {
            let now = k as f64 / 30.0;
            sim.place(k, now, p, &Environment::default(), &mut rng, &NoAttachments);
            if let Some(d) = sim.draw(now, &camera, &mut rng) {
                for v in d.instances.iter().flat_map(|i| &i.vertices) {
                    assert!(
                        v.position
                            .iter()
                            .chain(&v.colour)
                            .chain(&v.uv)
                            .all(|c| c.is_finite()),
                        "world space {world_space}: {v:?}"
                    );
                }
            }
        }
    }
}

/// `particles.md` §8.5.5: with `stick_to_bones` = 1 a particle **and its streak anchor**
/// are drawn at the attachment point, so a streaked particle collapses to the default
/// corners centred on the point.
#[test]
fn stuck_particles_draw_without_a_streak() {
    let mut e = emitter(10.0, 5.0);
    e.speed = Curve::constant(2.0);
    e.direction = CurveVec::constant([0.0, 0.0, 1.0]);
    e.streak = Curve::constant(0.5);
    e.bone = 1;
    e.stick_to_bones = true;
    if let Some(k) = e.keyframes.first_mut() {
        k.controller = 2;
    }
    let mut sim = SystemSim::new(Arc::new(system(vec![e])), 0.0);
    let mut rng = Ran2::new(8);
    let mut drawn = None;
    for k in 0..30u64 {
        let now = k as f64 / 30.0;
        sim.place(
            k,
            now,
            Placement::default(),
            &Environment::default(),
            &mut rng,
            &OriginAttachments,
        );
        drawn = sim.draw(now, &camera(), &mut rng);
    }
    let drawn = drawn.expect("drawn");
    let inst = drawn.instances.first().expect("an instance");
    assert!(inst.quads() > 0);
    for quad in inst.vertices.as_chunks::<4>().0 {
        let c = quad
            .iter()
            .fold(Vec3::ZERO, |a, v| a + Vec3::from_array(v.position))
            / 4.0;
        assert!(c.length() < 1e-4, "quad centred at {c}");
    }
}

/// `particle-runtime.md` §10.2, Tessera rule without the fork library: a fork effect counts
/// as active until the `tail_value` rule of `particles.md` §6.5 expires it, and on release
/// it is destroyed at once instead of lingering (it is local-space and has no instance).
#[test]
fn fork_effects_without_the_library() {
    use tessera_formats::PropertySet;
    use tessera_formats::gameplay::particles::ForkSystem;
    use tessera_particles::{ForkEffect, ForkFade};
    let record = ForkSystem {
        image: Vec::new(),
        flag: false,
        name: String::new(),
        name_table_hint: 0,
        name_table: Vec::new(),
        properties: PropertySet::default(),
        transform: Vec::new(),
        resources_hint: 0,
        resources: Vec::new(),
        tail_value: 2.0,
        bounds_min: [0.0; 3],
        bounds_max: [0.0; 3],
    };
    let fork = |now: f64| {
        Some(Effect::Fork(ForkEffect::new(
            &record,
            now,
            ForkFade::default(),
        )))
    };
    let probe = fork(1.0).expect("fork");
    assert!(!probe.has_ended(2.9), "active within its window");
    assert!(probe.has_ended(3.1), "expired after tail_value");
    let mut mgr = EffectManager::new();
    mgr.begin_frame(0, 1.0);
    let (h, k) = mgr
        .start_attached(1, 0x4400_0001, 1.0, || fork(1.0), AttachSpec::default())
        .expect("started");
    assert!(mgr.effect(k).is_some());
    mgr.stop(1, h, false, 1.5);
    assert!(mgr.effect(k).is_none(), "destroyed at once");
    assert!(mgr.lingering().is_empty(), "a fork effect never lingers");
}

/// `particle-runtime.md` §11.1 step 8 (RT46): a beam's systems skip the camera-distance fade
/// and cut of `particles.md` §8.13: drawn at any distance, at full opacity, never frozen.
#[test]
fn beam_systems_skip_the_distance_fade() {
    let mut sim = SystemSim::new(Arc::new(system(vec![emitter(30.0, 1.0)])), 0.0);
    assert!(sim.distance_fade_enabled());
    sim.set_distance_fade(false);
    let mut rng = Ran2::new(3);
    let far = CameraView::look_at(Vec3::new(0.0, -100_000.0, 0.0), Vec3::ZERO);
    let env = Environment::default();
    let mut drawn = 0usize;
    for k in 0..30u64 {
        let now = k as f64 / 30.0;
        let p = Placement {
            opacity: 0.5,
            ..Placement::default()
        };
        sim.place(k, now, p, &env, &mut rng, &NoAttachments);
        if let Some(d) = sim.draw(now, &far, &mut rng) {
            drawn += 1;
            assert!(
                d.instances.iter().all(|i| (i.opacity - 0.5).abs() < 1e-6),
                "no fade factor"
            );
        }
        assert!(!sim.is_not_drawn());
    }
    assert!(drawn > 0);
    assert!(sim.live_particles() > 0, "updated while far away");
}

/// `particle-runtime.md` §11.1 step 1: a beam makes its own systems; they are never shared
/// with another owner's request, nor does the beam take a running shared copy.
#[test]
fn beam_systems_are_not_shared() {
    let mut ps = system(vec![emitter(30.0, 10.0)]);
    ps.shareable = true;
    ps.world_space = false;
    let desc = Arc::new(ps);
    let make = || Some(Effect::Particles(SystemSim::new(Arc::clone(&desc), 0.0)));
    let mut mgr = EffectManager::new();
    mgr.begin_frame(0, 0.0);
    let (_, shared) = mgr
        .start_attached(1, 0x3900_0009, 0.0, make, AttachSpec::default())
        .unwrap();
    let (_, own) = mgr
        .start_attached_own(2, 0x3900_0009, 0.0, make, AttachSpec::default())
        .unwrap();
    assert_ne!(shared, own, "the beam does not take the running copy");
    mgr.effect_mut(own)
        .and_then(Effect::particles_mut)
        .unwrap()
        .set_distance_fade(false);
    assert_eq!(mgr.shared(0x3900_0009), Some(shared));
    let (_, third) = mgr
        .start_attached(3, 0x3900_0009, 0.0, make, AttachSpec::default())
        .unwrap();
    assert_eq!(
        third, shared,
        "a request shares the ordinary copy, not the beam's"
    );
}
