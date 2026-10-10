// SPDX-License-Identifier: GPL-3.0-or-later
//! The keyframe walk of `particles.md` §8.6.1 (BIN third pass, P53) with rule §10.11, the
//! exploding direction of §8.5.2, and the engineering limits of `particle-runtime.md` §12
//! rule 9, on synthetic records. Seeded streams; the checks are on the rules, not on values
//! of the original's shared stream.

mod common;

use std::sync::Arc;

use common::worked_example;
use tessera_formats::gameplay::particles::{Argb, Curve, Emitter, Keyframe, ParticleSystem};
use tessera_particles::glam::{Mat4, Vec3};
use tessera_particles::{
    AttachmentSource, CameraView, Environment, OriginAttachments, Placement, Ran2, SimConfig,
    SystemSim,
};

fn camera() -> CameraView {
    CameraView::look_at(Vec3::new(0.0, -10.0, 1.0), Vec3::ZERO)
}

/// `rate` particles per second living `life` seconds from a point, speed 1 along +Z, one key.
fn emitter(rate: f32, life: f32, key: Keyframe) -> Emitter {
    Emitter {
        emit_interval: 1.0 / rate,
        burst_count: 1,
        max_particles: 100_000,
        lifespan: Curve::constant(life),
        speed: Curve::constant(1.0),
        keyframes: vec![key],
        ..Emitter::default()
    }
}

fn system(emitters: Vec<Emitter>) -> ParticleSystem {
    let mut ps = worked_example();
    ps.emitters = emitters;
    ps.time_step = 1.0 / 30.0;
    ps
}

fn place(sim: &mut SystemSim, rng: &mut Ran2, frame: u64, attach: &dyn AttachmentSource) {
    let now = frame as f64 / 30.0;
    sim.place(
        frame,
        now,
        Placement::default(),
        &Environment::default(),
        rng,
        attach,
    );
}

fn run(sim: &mut SystemSim, rng: &mut Ran2, frames: u64) {
    for k in 0..frames {
        place(sim, rng, k, &OriginAttachments);
    }
}

/// Steps 1–2: no key is current at creation, and a key becomes current only once the age
/// reaches its `time`. Until then the particle does not move (step 5) and its channels take
/// the values of rule §10.11 (scale (1, 1), opaque white).
#[test]
fn a_first_key_with_a_later_time_waits_for_the_age() {
    let key = Keyframe {
        time: 0.5,
        flags: 0x770,
        controller: 2,
        physics_flags: 0,
        scale_x: 0.25,
        scale_y: 0.25,
        colour: Argb(0xFFFF_0000),
        ..Keyframe::default()
    };
    let mut sim = SystemSim::new(Arc::new(system(vec![emitter(10.0, 5.0, key)])), 0.0);
    let mut rng = Ran2::new(7);
    run(&mut sim, &mut rng, 40);
    let now = sim.last_update(0).expect("emitter");
    let mut young = 0;
    let mut old = 0;
    for p in sim.particles(0) {
        let age = (now - p.spawn_time) as f32;
        if age < 0.5 - 1e-4 {
            young += 1;
            assert!(
                p.position.length() < 1e-5,
                "age {age}: no controller key, no motion ({})",
                p.position
            );
        } else if age > 0.5 + 1e-4 {
            old += 1;
            // Ballistic from the base fixed at the key time (no motion before it).
            assert!(
                (p.position.z - (age - 0.5)).abs() < 1e-3,
                "age {age}: {}",
                p.position
            );
        }
    }
    assert!(young > 0 && old > 0, "{young} young, {old} old");
    // Drawn values: white (rule §10.11) before the key, the key's red after it.
    let draw = sim.draw(now, &camera(), &mut rng).expect("drawn");
    let colours: Vec<[f32; 4]> = draw.instances[0]
        .vertices
        .iter()
        .map(|v| v.colour)
        .collect();
    assert!(
        colours.iter().any(|c| c[1] > 0.99 && c[2] > 0.99),
        "white vertices before the key"
    );
    assert!(
        colours.iter().any(|c| c[0] > 0.99 && c[1] < 0.01),
        "red vertices after the key"
    );
}

/// Step 2 with a first key at `time` 0: the update right after the spawn makes it current,
/// also at a negative age (the particle then moves under it, `particle-runtime.md` §14.3).
#[test]
fn a_first_key_at_time_zero_is_current_from_the_spawn_update() {
    let key = Keyframe {
        flags: 0x770,
        controller: 2,
        physics_flags: 0,
        ..Keyframe::default()
    };
    let mut sim = SystemSim::new(Arc::new(system(vec![emitter(30.0, 5.0, key)])), 0.0);
    let mut rng = Ran2::new(2);
    run(&mut sim, &mut rng, 30);
    let now = sim.last_update(0).expect("emitter");
    for p in sim.particles(0) {
        let age = (now - p.spawn_time) as f32;
        assert!(
            (p.position.z - age).abs() < 1e-3,
            "age {age}: {}",
            p.position
        );
    }
}

/// Steps 3–4: a key that sets controller 3 and physics flag 0x8 together: the point index
/// is drawn at the first update under the controller, after the key's physics flags, so the
/// key's counter is replaced by a random index after each particle takes it.
#[test]
fn the_controller_3_index_sees_the_same_keys_physics_flags() {
    let points: Vec<[f32; 3]> = (0..1000).map(|i| [i as f32, 0.0, 0.0]).collect();
    let key = |physics_flags: u32| Keyframe {
        flags: 0x770 | 0x200,
        controller: 3,
        physics_flags,
        points: points.clone(),
        ..Keyframe::default()
    };
    let indices = |physics_flags: u32| -> Vec<u32> {
        let mut e = emitter(30.0, 10.0, key(physics_flags));
        e.speed = Curve::constant(0.0);
        let mut sim = SystemSim::new(Arc::new(system(vec![e])), 0.0);
        let mut rng = Ran2::new(11);
        run(&mut sim, &mut rng, 40);
        sim.particles(0)
            .iter()
            .map(|p| p.position.x as u32)
            .collect()
    };
    let ordered = indices(0);
    assert!(ordered.len() > 20);
    for (i, x) in ordered.iter().enumerate() {
        assert_eq!(*x as usize, i, "counter order without 0x8");
    }
    let random = indices(0x8);
    assert_eq!(
        random.first(),
        Some(&0),
        "the first particle takes the counter 0"
    );
    let in_order = random
        .iter()
        .enumerate()
        .filter(|(i, x)| **x as usize == *i)
        .count();
    assert!(
        in_order < random.len() / 2,
        "with 0x8 the counter is re-drawn: {random:?}"
    );
}

/// Step 3 (the base is fixed with the previous controller) and step 4 (the index is drawn
/// when the new controller first runs): a later key switches from rising at 1 m/s to the
/// points of controller 3.
#[test]
fn a_later_controller_3_key_places_points_on_the_fixed_base() {
    let mut e = emitter(
        10.0,
        5.0,
        Keyframe {
            flags: 0x770,
            controller: 2,
            physics_flags: 0,
            ..Keyframe::default()
        },
    );
    e.keyframes.push(Keyframe {
        time: 0.3,
        flags: 0x100,
        controller: 3,
        points: vec![[1.0, 0.0, 0.0], [2.0, 0.0, 0.0]],
        ..Keyframe::default()
    });
    let mut sim = SystemSim::new(Arc::new(system(vec![e])), 0.0);
    let mut rng = Ran2::new(3);
    run(&mut sim, &mut rng, 60);
    let now = sim.last_update(0).expect("emitter");
    let mut switched = 0;
    for p in sim.particles(0) {
        let age = (now - p.spawn_time) as f32;
        if age > 0.3 + 1e-3 {
            switched += 1;
            assert!(
                (p.position.z - 0.3).abs() < 1e-3,
                "base fixed at 0.3 m: {}",
                p.position
            );
            assert!(
                p.position.x == 1.0 || p.position.x == 2.0,
                "a point of the key: {}",
                p.position
            );
        }
    }
    assert!(switched > 5);
}

/// Step 5: a key without the controller bit leaves the particle without motion, whatever
/// its spawn velocity; the channels it sets still apply.
#[test]
fn without_a_controller_key_a_particle_does_not_move() {
    let key = Keyframe {
        flags: 0x670,
        controller: 2,
        physics_flags: 1,
        ..Keyframe::default()
    };
    let mut sim = SystemSim::new(Arc::new(system(vec![emitter(10.0, 5.0, key)])), 0.0);
    let mut rng = Ran2::new(4);
    run(&mut sim, &mut rng, 45);
    assert!(sim.live_particles() > 5);
    for p in sim.particles(0) {
        assert!(p.position.length() < 1e-6, "{}", p.position);
        assert!(
            (p.velocity.z - 1.0).abs() < 1e-6,
            "the spawn velocity is kept"
        );
    }
}

/// Every attachment point 2 m above the owner's origin.
struct Raised;

impl AttachmentSource for Raised {
    fn frame(&self, _: u32, _: u32) -> Option<Mat4> {
        Some(Mat4::from_translation(Vec3::new(0.0, 0.0, 2.0)))
    }
}

/// §8.5.2 (BIN third pass): the radial direction of an exploding emitter uses the shape
/// point after the attachment of §8.5.5 and before `origin`. A `Point` emitter whose
/// holding location is 2 m up therefore explodes straight up, not in a random direction;
/// `origin` does not turn it.
#[test]
fn exploding_direction_uses_the_attached_point() {
    let mut e = emitter(
        30.0,
        5.0,
        Keyframe {
            flags: 0x770,
            controller: 6,
            ..Keyframe::default()
        },
    );
    e.exploding = true;
    e.shape = 4;
    e.holding_location = 1;
    e.origin = tessera_formats::gameplay::particles::CurveVec::constant([5.0, 0.0, 0.0]);
    let mut ps = system(vec![e.clone()]);
    ps.world_space = true;
    let mut sim = SystemSim::new(Arc::new(ps), 0.0);
    let mut rng = Ran2::new(9);
    for k in 0..30 {
        place(&mut sim, &mut rng, k, &Raised);
    }
    assert!(sim.live_particles() > 10);
    for p in sim.particles(0) {
        assert!(
            (p.velocity - Vec3::Z).length() < 1e-5,
            "radial from the attached point: {}",
            p.velocity
        );
        assert!(
            (p.base - Vec3::new(5.0, 0.0, 2.0)).length() < 1e-4,
            "{}",
            p.base
        );
    }
    // Without an attachment the point is 0 and the direction is random.
    e.holding_location = 0;
    let mut sim = SystemSim::new(Arc::new(system(vec![e])), 0.0);
    run(&mut sim, &mut rng, 30);
    assert!(
        sim.particles(0)
            .iter()
            .any(|p| (p.velocity - Vec3::Z).length() > 0.1)
    );
}

/// `particle-runtime.md` §12 rule 9: the engineering limits.
#[test]
fn rule_9_limits() {
    let c = SimConfig::default();
    assert_eq!(c.max_live_particles, 262_144);
    assert_eq!(c.max_stepped_steps, 64);
    assert_eq!(c.max_path_legs, 1_024);

    // The system budget: all emitters together stop at the limit.
    let key = Keyframe {
        flags: 0x770,
        controller: 6,
        ..Keyframe::default()
    };
    let config = SimConfig {
        max_live_particles: 25,
        ..SimConfig::default()
    };
    let e = emitter(300.0, 100.0, key.clone());
    let mut sim = SystemSim::with_config(Arc::new(system(vec![e.clone(), e])), 0.0, config);
    let mut rng = Ran2::new(1);
    run(&mut sim, &mut rng, 30);
    assert_eq!(sim.live_particles(), 25);

    // Controller 4 with a positive `time_step` far below 1/120 s: at most 64 steps per
    // particle and update, so a particle moves at most 64 steps' worth per update.
    let ts = 1e-6f32;
    let stepped = Keyframe {
        flags: 0x770,
        controller: 4,
        physics_flags: 0,
        ..Keyframe::default()
    };
    let mut ps = system(vec![emitter(30.0, 10.0, stepped)]);
    ps.time_step = ts;
    let mut sim = SystemSim::new(Arc::new(ps), 0.0);
    run(&mut sim, &mut rng, 2);
    let before: Vec<(u32, Vec3)> = sim
        .particles(0)
        .iter()
        .map(|p| (p.serial, p.position))
        .collect();
    place(&mut sim, &mut rng, 2, &OriginAttachments);
    for p in sim.particles(0) {
        if let Some((_, b)) = before.iter().find(|(s, _)| *s == p.serial) {
            let moved = (p.position - *b).length();
            assert!(moved <= 64.0 * ts * 1.0001, "{moved}");
            assert!(moved > 0.0, "the particle moves");
        }
    }

    // Controller 1 with repeated points and a huge speed: the walk ends after its legs.
    let path = Keyframe {
        flags: 0x770,
        controller: 1,
        points: vec![[0.0; 3]; 3],
        ..Keyframe::default()
    };
    let mut e = emitter(30.0, 5.0, path);
    e.speed = Curve::constant(1e30);
    let mut sim = SystemSim::new(Arc::new(system(vec![e])), 0.0);
    run(&mut sim, &mut rng, 30);
    assert!(sim.live_particles() > 10);
    assert!(sim.particles(0).iter().all(|p| p.position.length() < 1e-6));
}

/// `particles.md` PS10 (§8.6.1 base fix, P54): a controller-3 key followed 0.01 s later by
/// a controller-6 key. Particles dated at least 0.01 s before the emitter's last
/// particle-update time (here by distance emission, whose steps lie before the previous
/// update, §8.4) pass both keys in their spawn-time update; the second key's base fix fills
/// the empty slot from the first key's counter in order, without a random draw although
/// the key sets physics flag 0x8, and places the base on that point.
#[test]
fn ps10_the_base_fix_fills_an_empty_slot_in_order() {
    let first = Keyframe {
        time: 0.0,
        flags: 0x100 | 0x200,
        controller: 3,
        physics_flags: 0x8,
        points: vec![[0.0, 0.0, 1.0], [0.0, 0.0, 2.0]],
        ..Keyframe::default()
    };
    let mut e = emitter(1.0, 5.0, first);
    e.emit_interval = 0.0;
    e.emission_distance = 0.5;
    e.speed = Curve::constant(0.0);
    e.keyframes.push(Keyframe {
        time: 0.01,
        flags: 0x100,
        controller: 6,
        ..Keyframe::default()
    });
    let mut ps = system(vec![e]);
    ps.time_step = 0.05;
    let mut sim = SystemSim::new(Arc::new(ps), 0.0);
    let mut rng = Ran2::new(5);
    // Frames 0.06 s apart, so every frame runs a particle update (`time_step` 0.05).
    for frame in 0..20u64 {
        sim.place(
            frame,
            frame as f64 * 0.06,
            Placement::at(Vec3::new(frame as f32, 0.0, 0.0)),
            &Environment::default(),
            &mut rng,
            &OriginAttachments,
        );
    }
    let mut particles: Vec<_> = sim.particles(0).to_vec();
    assert!(particles.len() > 10, "{}", particles.len());
    particles.sort_by_key(|p| p.serial);
    for p in &particles {
        let expected = if p.serial % 2 == 0 { 1.0 } else { 2.0 };
        assert!(
            (p.position.z - expected).abs() < 1e-5 && (p.base.z - expected).abs() < 1e-5,
            "particle {} at {} (base {})",
            p.serial,
            p.position,
            p.base
        );
    }
}

/// §8.6.1 step 4 and rule §10.12: the slot filled under the first controller-3 key is
/// kept under a later controller-3 key with fewer points and reduced modulo its point
/// count; the base fix of the leaving key moves the base to the leaving key's point.
#[test]
fn the_point_slot_is_shared_and_reduced_modulo_the_point_count() {
    let mut e = emitter(
        30.0,
        5.0,
        Keyframe {
            flags: 0x770,
            controller: 3,
            physics_flags: 0,
            points: vec![[1.0, 0.0, 0.0], [2.0, 0.0, 0.0], [3.0, 0.0, 0.0]],
            ..Keyframe::default()
        },
    );
    e.speed = Curve::constant(0.0);
    e.keyframes.push(Keyframe {
        time: 0.3,
        flags: 0x100,
        controller: 3,
        points: vec![[10.0, 0.0, 0.0], [20.0, 0.0, 0.0]],
        ..Keyframe::default()
    });
    let mut sim = SystemSim::new(Arc::new(system(vec![e])), 0.0);
    let mut rng = Ran2::new(9);
    run(&mut sim, &mut rng, 60);
    let now = sim.last_update(0).expect("emitter");
    let mut switched = 0;
    for p in sim.particles(0) {
        let slot = p.serial % 3;
        let first = (slot + 1) as f32;
        let age = (now - p.spawn_time) as f32;
        let expected = if age >= 0.3 + 1e-4 {
            switched += 1;
            first + [10.0, 20.0][(slot % 2) as usize]
        } else if age < 0.3 - 1e-4 {
            first
        } else {
            continue;
        };
        assert!(
            (p.position.x - expected).abs() < 1e-4,
            "particle {} age {age}: {}",
            p.serial,
            p.position
        );
    }
    assert!(switched > 5);
}

/// Runs a system whose one emitter spawns a single particle at clock 0 (frames 0.06 s
/// apart, `time_step` 0.05, so every frame from the second on runs a particle update whose
/// previous update time is the frame before) and returns its position after `frames`
/// frames.
fn single_particle(e: Emitter, seed: u32, frames: u64) -> Vec3 {
    let mut e = e;
    e.emit_interval = 100.0;
    e.lifespan = Curve::constant(50.0);
    let mut ps = system(vec![e]);
    ps.time_step = 0.05;
    let mut sim = SystemSim::new(Arc::new(ps), 0.0);
    let mut rng = Ran2::new(seed);
    for frame in 0..frames {
        sim.place(
            frame,
            frame as f64 * 0.06,
            Placement::default(),
            &Environment::default(),
            &mut rng,
            &OriginAttachments,
        );
    }
    let p = sim.particles(0);
    assert_eq!(p.len(), 1, "one particle");
    assert!(
        p[0].spawn_time.abs() < 1e-9,
        "spawned at 0: {}",
        p[0].spawn_time
    );
    p[0].position
}

/// `particles.md` PS11 (§8.6.1 base fix of a leaving controller 1, §8.7, P57): keys at 0
/// (controller 6), 0.02 (controller 1 with the points (0, 0, 0) and (10, 0, 0)) and 0.03
/// (controller 6), speed 10. The particle, spawned at clock 0 and next updated at 0.06,
/// passes the second and third keys in that update's walk: the third key's base fix fills
/// the empty slot with index 0, places the particle on its spawn position and walks
/// 10 × (0 + 0.03 − 0) = 0.3, ending √0.3 m towards point 1, where it stays. With physics
/// flag 0x8 the start index is 0 or 1, and from index 1 the particle ends at 10 − √0.3.
#[test]
fn ps11_the_controller_1_base_fix_fills_places_and_walks() {
    let make = |flags: u32| {
        let mut e = emitter(
            1.0,
            50.0,
            Keyframe {
                time: 0.0,
                flags: 0x100 | 0x200,
                controller: 6,
                physics_flags: flags,
                ..Keyframe::default()
            },
        );
        e.speed = Curve::constant(10.0);
        e.keyframes.push(Keyframe {
            time: 0.02,
            flags: 0x100,
            controller: 1,
            points: vec![[0.0; 3], [10.0, 0.0, 0.0]],
            ..Keyframe::default()
        });
        e.keyframes.push(Keyframe {
            time: 0.03,
            flags: 0x100,
            controller: 6,
            ..Keyframe::default()
        });
        e
    };
    let near = 0.3f32.sqrt();
    let far = 10.0 - near;
    for frames in [2, 3, 10] {
        let p = single_particle(make(0), 1, frames);
        assert!(
            (p - Vec3::new(near, 0.0, 0.0)).length() < 1e-4,
            "after {frames} frames: {p}"
        );
    }
    let (mut at_near, mut at_far) = (0, 0);
    for seed in 1..=40 {
        let p = single_particle(make(0x8), seed, 4);
        if (p - Vec3::new(near, 0.0, 0.0)).length() < 1e-4 {
            at_near += 1;
        } else if (p - Vec3::new(far, 0.0, 0.0)).length() < 1e-4 {
            at_far += 1;
        } else {
            panic!("seed {seed}: {p}");
        }
    }
    assert!(at_near > 0 && at_far > 0, "{at_near} {at_far}");
}

/// §8.7 (P57): the first update under a controller-1 key walks its budget right after
/// the placement on the start point, and the spawn-time update walks nothing (span 0).
#[test]
fn controller_1_walks_on_its_first_update() {
    let mut e = emitter(
        1.0,
        50.0,
        Keyframe {
            time: 0.0,
            flags: 0x770,
            controller: 1,
            physics_flags: 0,
            points: vec![[0.0; 3], [10.0, 0.0, 0.0]],
            ..Keyframe::default()
        },
    );
    e.speed = Curve::constant(1.0);
    let step = 0.06f32.sqrt();
    // Frame 1 spawns the particle (spawn-time update: placed on point 0, span 0) and runs
    // its first particle update (budget 1 × 0.06); every later frame walks the same.
    for frames in 2..6u64 {
        let p = single_particle(e.clone(), 1, frames);
        let expected = (frames - 1) as f32 * step;
        assert!(
            (p - Vec3::new(expected, 0.0, 0.0)).length() < 1e-4,
            "after {frames} frames: {p}, expected {expected}"
        );
    }
}

/// §8.7: at every base fix the starting value of the angle triple the particle keeps
/// rises by the new key's `time` × `rotation_velocity`; the first controller key (no
/// leaving key) adds nothing. P58: the particle keeps `rotation` when `world_rotation` is
/// the constant 0, and the other triple is neither grown nor raised.
#[test]
fn the_rotation_jumps_at_a_base_fix() {
    let mut e = emitter(
        10.0,
        5.0,
        Keyframe {
            flags: 0x770,
            controller: 6,
            physics_flags: 0,
            ..Keyframe::default()
        },
    );
    e.rotation_velocity.z = Curve::constant(2.0);
    e.keyframes.push(Keyframe {
        time: 0.5,
        flags: 0x100,
        controller: 6,
        ..Keyframe::default()
    });
    let mut sim = SystemSim::new(Arc::new(system(vec![e])), 0.0);
    let mut rng = Ran2::new(4);
    run(&mut sim, &mut rng, 60);
    let now = sim.last_update(0).expect("emitter");
    let (mut before, mut after) = (0, 0);
    for p in sim.particles(0) {
        let age = (now - p.spawn_time) as f32;
        if age >= 0.5 + 1e-4 {
            after += 1;
            assert!((p.rotation[2] - 1.0).abs() < 1e-5, "{:?}", p.rotation);
            assert_eq!(p.world_rotation, [0.0; 3], "not kept, not raised");
        } else if age < 0.5 - 1e-4 {
            before += 1;
            assert_eq!(p.rotation, [0.0; 3]);
        }
    }
    assert!(before > 0 && after > 0);
}

/// P58: with a `world_rotation` that is not the constant 0 the particle keeps that triple,
/// and the base fix raises it instead of `rotation`.
#[test]
fn the_base_fix_raises_only_the_kept_world_rotation() {
    let mut e = emitter(
        10.0,
        5.0,
        Keyframe {
            flags: 0x770,
            controller: 6,
            physics_flags: 0,
            ..Keyframe::default()
        },
    );
    e.world_rotation.x = Curve::constant(0.25);
    e.rotation.z = Curve::constant(0.5);
    e.rotation_velocity.z = Curve::constant(2.0);
    e.keyframes.push(Keyframe {
        time: 0.5,
        flags: 0x100,
        controller: 6,
        ..Keyframe::default()
    });
    let mut sim = SystemSim::new(Arc::new(system(vec![e])), 0.0);
    let mut rng = Ran2::new(4);
    run(&mut sim, &mut rng, 60);
    let now = sim.last_update(0).expect("emitter");
    let mut after = 0;
    for p in sim.particles(0) {
        if ((now - p.spawn_time) as f32) >= 0.5 + 1e-4 {
            after += 1;
            assert!(
                (p.world_rotation[2] - 1.0).abs() < 1e-5
                    && (p.world_rotation[0] - 0.25).abs() < 1e-6,
                "{:?}",
                p.world_rotation
            );
            assert!((p.rotation[2] - 0.5).abs() < 1e-6, "{:?}", p.rotation);
        }
    }
    assert!(after > 0);
}

/// §8.6.1 base-fix table, controller 2: the force is scaled by the stored `mass` of the
/// current mass key, not by the mass interpolated towards the next key.
#[test]
fn the_ballistic_base_fix_uses_the_stored_mass() {
    let mut e = emitter(
        10.0,
        5.0,
        Keyframe {
            flags: 0x770,
            controller: 2,
            physics_flags: tessera_formats::gameplay::particles::PHYSICS_GRAVITY,
            mass: 1.0,
            ..Keyframe::default()
        },
    );
    e.speed = Curve::constant(0.0);
    e.keyframes.push(Keyframe {
        time: 0.5,
        flags: 0x100 | 0x08,
        controller: 6,
        mass: 3.0,
        ..Keyframe::default()
    });
    let mut sim = SystemSim::new(Arc::new(system(vec![e])), 0.0);
    let mut rng = Ran2::new(2);
    run(&mut sim, &mut rng, 60);
    let now = sim.last_update(0).expect("emitter");
    let mut switched = 0;
    for p in sim.particles(0) {
        if ((now - p.spawn_time) as f32) >= 0.5 + 1e-4 {
            switched += 1;
            let expected = -0.5 * 9.8 * 0.25;
            assert!((p.position.z - expected).abs() < 1e-4, "{}", p.position);
        }
    }
    assert!(switched > 5);
}
