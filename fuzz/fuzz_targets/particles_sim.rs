// SPDX-License-Identifier: GPL-3.0-or-later
//! The particle runtime on hostile records (`crates/particles`; `docs/specs/particles.md`
//! §8, §10; `docs/specs/particle-runtime.md` §3–§9).
//!
//! Byte 0 selects the record: the rest as a 0x39 payload, or a valid synthetic system
//! (every emitter field and curve mode reachable) whose encoded bytes are overwritten by
//! (position, byte) pairs from the input, so decodable records carry hostile values (NaN
//! and huge times, intervals, counts, scales, controllers, frame counts). A decoded record
//! is then simulated for up to 48 frames with frame steps, placements, owner scales and
//! wind taken from the input, drawn every frame, classified (§5.3), and run through the
//! effect manager (start, stop, release, linger, sweep, housekeeping).
//!
//! Nothing may panic or hang; every vertex is finite; no draw exceeds 32,768 vertices; no
//! emitter exceeds its cap. Run with a memory cap, e.g.
//! `cargo fuzz run particles_sim -- -malloc_limit_mb=1024 -rss_limit_mb=2048`.

#![no_main]

use std::sync::Arc;

use libfuzzer_sys::fuzz_target;
use tessera_formats::Did;
use tessera_formats::gameplay::particles::{
    Argb, Curve, CurveVec, Emitter, KeyedCurve, Keyframe, MAX_EMISSION_CAP, ParticleSystem,
};
use tessera_particles::glam::{Mat4, Quat, Vec3};
use tessera_particles::runtime::AttachSpec;
use tessera_particles::{
    CameraView, Effect, EffectManager, Environment, MAX_VERTICES, OriginAttachments, Placement,
    Ran2, SimConfig, SystemSim, classify_system,
};

/// A synthetic system that reaches every shape, controller and curve family.
fn template() -> ParticleSystem {
    let wave = |mode: u32| Curve {
        mode,
        w: [0.5, 0.1, 0.3, 0.0, 0.2, 0.1, 2.0, 0.0, 0.4, 0.0],
        keyed: None,
    };
    let keyed = Curve {
        mode: 10,
        w: [0.0; 10],
        keyed: Some(Box::new(KeyedCurve {
            period: 2.0,
            looped: true,
            pairs: vec![(0.0, 0.0), (1.0, 50.0), (0.5, 100.0)],
        })),
    };
    let key = |time: f32, flags: u32, controller: u32| Keyframe {
        time,
        flags,
        scale_x: 0.3,
        scale_y: 0.4,
        colour: Argb(0x80FF_8040),
        mass: 1.0,
        controller,
        physics_flags: 0x29,
        points: vec![[0.0, 0.0, 0.0], [1.0, 0.0, 0.0], [1.0, 1.0, 0.0]],
        position: CurveVec {
            x: wave(4),
            y: wave(3),
            z: keyed.clone(),
        },
    };
    let emitter = |shape: u32, primitive: u32, controller: u32| Emitter {
        shape,
        primitive,
        emit_interval: 0.05,
        speed: wave(3),
        direction: CurveVec::constant([0.0, 0.3, 1.0]),
        emission_limit: 40,
        lifespan: Curve::constant(1.0),
        shape_size: wave(7),
        min_spread: Curve::constant(5.0),
        max_spread: wave(8),
        particle_scale: wave(3),
        rotation: CurveVec {
            z: wave(2),
            ..CurveVec::default()
        },
        max_particles: 64,
        exploding: shape == 3,
        streak: Curve::constant(0.1),
        world_rotation: CurveVec::default(),
        rotation_velocity: CurveVec {
            z: wave(5),
            ..CurveVec::default()
        },
        burst_count: 2,
        start_time: 0.1,
        time_limit: 1.5,
        emission_distance: 0.2,
        snap: wave(9),
        fade_in: 0.2,
        fade_out: 0.5,
        frames_u: 4,
        frames_v: 2,
        frame_rate: 12,
        random_start_frame: true,
        keyframes: vec![
            key(0.0, 0x770, controller),
            key(0.5, 0x37E, 5),
            key(0.8, 0x106, 4),
        ],
        bone: u32::from(shape == 5),
        velocity_facing: shape == 2,
        ..Emitter::default()
    };
    ParticleSystem {
        time_step: 1.0 / 30.0,
        fast_forward: 0.5,
        physics_duration: 0.0,
        world_space: true,
        force_draw: false,
        uses_video_post: false,
        draw_reflection: false,
        use_lighting: true,
        inherit_opacity: false,
        far_fade_start: 30.0,
        far_fade_end: 60.0,
        near_fade_start: 0.0,
        near_fade_end: 0.0,
        scale_mode: 1,
        emitters: vec![
            emitter(1, 2, 2),
            emitter(2, 2, 1),
            emitter(3, 1, 3),
            emitter(5, 2, 6),
        ],
        has_bounds: false,
        bounds_min: [0.0; 3],
        bounds_max: [0.0; 3],
        sorting_priority: 5,
        shareable: false,
        surface: Did(0x3100_0001),
        extra_surface: Did(0),
    }
}

/// Reads input bytes as numbers, giving zeros when they run out.
struct Bytes<'a>(&'a [u8]);

impl Bytes<'_> {
    fn u8(&mut self) -> u8 {
        match self.0.split_first() {
            Some((&b, rest)) => {
                self.0 = rest;
                b
            }
            None => 0,
        }
    }

    fn f32(&mut self) -> f32 {
        f32::from_le_bytes([self.u8(), self.u8(), self.u8(), self.u8()])
    }
}

fn simulate(ps: ParticleSystem, mut input: Bytes<'_>) {
    let _ = classify_system(&ps);
    let caps: Vec<usize> = ps
        .emitters
        .iter()
        .map(|e| e.max_particles.min(MAX_EMISSION_CAP) as usize)
        .collect();
    let config = SimConfig {
        max_live_particles: 4_096,
        ..SimConfig::default()
    };
    let desc = Arc::new(ps);
    let seed = u32::from_le_bytes([input.u8(), input.u8(), input.u8(), input.u8()]);
    let mut rng = Ran2::new(seed);
    let mut sim = SystemSim::with_config(Arc::clone(&desc), 0.0, config);
    let mut now = 0.0f64;
    let frames = 8 + usize::from(input.u8() % 41);
    let wild = input.u8() & 1 == 1;
    for k in 0..frames as u64 {
        let dt = f64::from(input.u8()) / 100.0;
        now += dt;
        let (position, scale_a, scale_b, wind) = if wild {
            (
                Vec3::new(input.f32(), input.f32(), input.f32()),
                Vec3::splat(input.f32()),
                Vec3::splat(input.f32()),
                Vec3::new(input.f32(), 0.0, 0.0),
            )
        } else {
            let b = f32::from(input.u8()) / 32.0;
            (
                Vec3::new(b, 0.0, 0.0),
                Vec3::splat(1.0 + b),
                Vec3::ONE,
                Vec3::X * b,
            )
        };
        let p = Placement {
            position,
            rotation: Quat::from_rotation_z(f32::from(input.u8()) / 40.0),
            scale_a,
            scale_b,
            opacity: 1.0,
        };
        let env = Environment {
            wind,
            ambient: [0.8, 0.8, 0.8, 1.0],
        };
        // Sometimes skip frames (restart after 100) or place twice (two instances).
        let frame = k * u64::from(1 + input.u8() % 3) * 40;
        sim.place(frame, now, p, &env, &mut rng, &OriginAttachments);
        if input.u8() & 3 == 0 {
            sim.place(
                frame,
                now,
                Placement::at(position + Vec3::X),
                &env,
                &mut rng,
                &OriginAttachments,
            );
        }
        if k == 5 {
            sim.recompute_forces(wind);
        }
        let camera = CameraView::look_at(position - Vec3::Y * 10.0, position);
        if let Some(d) = sim.draw(now, &camera, &mut rng) {
            for inst in &d.instances {
                assert!(inst.vertices.len() <= MAX_VERTICES);
                assert!(inst.quad_vertices <= inst.vertices.len());
                for v in &inst.vertices {
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
        for (i, cap) in caps.iter().enumerate() {
            assert!(sim.particles(i).len() <= *cap);
        }
        assert!(sim.live_particles() <= config.max_live_particles);
        if k == 20 {
            sim.stop(now);
        }
        let _ = sim.has_ended(now);
    }

    // The manager: attach, place, stop with or without disposal, sweep, housekeeping.
    let mut mgr = EffectManager::new();
    let immediate = input.u8() & 1 == 1;
    let make = || {
        Some(Effect::Particles(SystemSim::with_config(
            Arc::clone(&desc),
            0.0,
            config,
        )))
    };
    let started = mgr.start_attached(1, 0x3900_0001, 0.0, make, AttachSpec::default());
    let env = Environment::default();
    for k in 0..12u64 {
        let t = k as f64 * 0.75;
        mgr.begin_frame(k, t);
        mgr.place_owner(
            1,
            k,
            t,
            Mat4::IDENTITY,
            Vec3::ONE,
            Vec3::ONE,
            1.0,
            &|_| None,
            &env,
            &mut rng,
            &OriginAttachments,
        );
        mgr.place_lingering(k, t, &env, &mut rng, &OriginAttachments);
        mgr.housekeep(1, t);
        if k == 4
            && let Some((h, _)) = started
        {
            mgr.stop(1, h, immediate, t);
        }
    }
    mgr.remove_owner(1, 100.0);
    mgr.begin_frame(100, 100.0);
}

fuzz_target!(|input: &[u8]| {
    let [mode, rest @ ..] = input else {
        return;
    };
    if mode % 2 == 0 {
        if let Ok(ps) = ParticleSystem::decode(rest) {
            simulate(ps, Bytes(rest));
        }
        return;
    }
    let Ok(mut bytes) = template().encode() else {
        return;
    };
    let mut r = Bytes(rest);
    let edits = usize::from(r.u8() % 24);
    for _ in 0..edits {
        let pos = usize::from(u16::from_le_bytes([r.u8(), r.u8()])) % bytes.len().max(1);
        let b = r.u8();
        if let Some(slot) = bytes.get_mut(pos) {
            *slot = b;
        }
    }
    if let Ok(ps) = ParticleSystem::decode(&bytes) {
        simulate(ps, r);
    }
});
