// SPDX-License-Identifier: GPL-3.0-or-later
//! Synthetic tests of the particle decoders (`tessera_formats::gameplay::particles`,
//! `docs/specs/particles.md`): the worked examples of §12, the defaults of §4.7, the
//! runtime views of §8 and §10, and hostile inputs (§4.4). All values are invented.

mod common;

use common::B;
use proptest::prelude::*;
use tessera_formats::gameplay::particles::{
    Argb, ChannelState, Curve, CurveVec, EMITTER_MIN_LEN, Emitter, FALLBACK_TIME_STEP,
    ForkResourceKind, ForkSystem, KEY_CONTROLLER, KEYFRAME_MIN_LEN, KeyedCurve, Keyframe,
    MAX_EMISSION_CAP, MotionController, PARTICLE_SYSTEM_MIN_LEN, ParticleSystem, Primitive,
    ScaleMode, SpawnShape, is_fork_system_did, is_particle_system_did,
};
use tessera_formats::{Did, FormatError, NoSchema, PropertyType};

/// §12.1, verbatim.
const MINIMAL: [u8; 77] = [
    0xcd, 0xcc, 0x4c, 0x3d, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x01, 0x00, 0x00, 0x00,
    0x00, 0x00, 0x00, 0x00, 0xf0, 0x41, 0x00, 0x00, 0x70, 0x42, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00,
    0x00, 0x00, 0x01, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00,
    0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00,
    0x05, 0x00, 0x00, 0x00, 0x01, 0x34, 0x12, 0x00, 0x31, 0x00, 0x00, 0x00, 0x00,
];

fn minimal_system() -> ParticleSystem {
    ParticleSystem::decode(&MINIMAL).expect("§12.1 decodes")
}

/// The emitter of §12.2.
fn example_emitter() -> Emitter {
    Emitter {
        shape: 3,
        primitive: 2,
        emit_interval: 0.1,
        speed: Curve::constant(2.0),
        direction: CurveVec::constant([0.0, 0.0, 1.0]),
        emission_limit: 0,
        lifespan: Curve::constant(1.5),
        shape_size: Curve::constant(0.5),
        min_spread: Curve::constant(0.0),
        max_spread: Curve::constant(30.0),
        particle_scale: Curve::constant(1.0),
        max_particles: 50,
        burst_count: 2,
        snap: Curve::constant(0.1),
        inclusive: true,
        active: true,
        fade_in: 0.0,
        fade_out: -1.0,
        frames_u: 2,
        frames_v: 2,
        frame_rate: 8,
        keyframes: vec![Keyframe {
            time: 0.0,
            flags: 0x770,
            scale_x: 0.2,
            scale_y: 0.2,
            colour: Argb(0xFFFF_8000),
            mass: -0.1,
            controller: 2,
            physics_flags: 1,
            points: Vec::new(),
            position: CurveVec::default(),
        }],
        name: "ab".into(),
        ..Emitter::default()
    }
}

fn example_system() -> ParticleSystem {
    let mut sys = minimal_system();
    sys.emitters.push(example_emitter());
    sys
}

fn f32_at(bytes: &[u8], at: usize) -> f32 {
    f32::from_le_bytes(bytes[at..at + 4].try_into().unwrap())
}

fn u32_at(bytes: &[u8], at: usize) -> u32 {
    u32::from_le_bytes(bytes[at..at + 4].try_into().unwrap())
}

// ---------------------------------------------------------------------------------------------
// §12.1, §12.2: layouts
// ---------------------------------------------------------------------------------------------

#[test]
fn minimal_record_fields() {
    let s = minimal_system();
    assert_eq!(MINIMAL.len(), PARTICLE_SYSTEM_MIN_LEN);
    assert_eq!(s.time_step, 0.05);
    assert_eq!(s.fast_forward, 0.0);
    assert_eq!(s.physics_duration, 0.0);
    assert!(s.world_space);
    assert!(!s.force_draw && !s.uses_video_post && !s.draw_reflection);
    assert!(!s.use_lighting && !s.inherit_opacity);
    assert_eq!((s.far_fade_start, s.far_fade_end), (30.0, 60.0));
    assert_eq!((s.near_fade_start, s.near_fade_end), (0.0, 0.0));
    assert_eq!(s.scale_mode, 1);
    assert_eq!(s.scale_mode_kind(), Some(ScaleMode::Total));
    assert!(s.emitters.is_empty());
    assert!(!s.has_bounds);
    assert_eq!(s.bounds_min, [0.0; 3]);
    assert_eq!(s.bounds_max, [0.0; 3]);
    assert_eq!(s.sorting_priority, 5);
    assert!(s.shareable);
    assert_eq!(s.surface, Did(0x3100_1234));
    assert_eq!(s.extra_surface, Did(0));
    assert_eq!(s.references().collect::<Vec<_>>(), vec![Did(0x3100_1234)]);
    assert_eq!(s.encode().unwrap(), MINIMAL);
    // §8.2: no active emitter, so the system has ended at once.
    assert!(s.has_ended(0.0));
    assert!((s.frame_rate() - 20.0).abs() < 1e-4);
    assert_eq!(s.update_interval(), 0.05);
}

#[test]
fn one_emitter_offsets() {
    let bytes = example_system().encode().unwrap();
    assert_eq!(bytes.len(), 447);
    assert_eq!(bytes[38], 1, "emitter count");
    // origin at 39: mode 1, value 0.
    assert_eq!(u32_at(&bytes, 39), 1);
    assert_eq!(u32_at(&bytes, 111), 3, "shape");
    assert_eq!(u32_at(&bytes, 115), 2, "primitive");
    assert_eq!(f32_at(&bytes, 119), 0.1, "emit_interval");
    assert_eq!(f32_at(&bytes, 127), 2.0, "speed value");
    assert_eq!(f32_at(&bytes, 151), 1.0, "direction z value");
    assert_eq!(u32_at(&bytes, 155), 0, "emission_limit");
    assert_eq!(f32_at(&bytes, 163), 1.5, "lifespan value");
    assert_eq!(f32_at(&bytes, 171), 0.5, "shape_size value");
    assert_eq!(f32_at(&bytes, 187), 30.0, "max_spread value");
    assert_eq!(u32_at(&bytes, 223), 50, "max_particles");
    assert_eq!(&bytes[227..229], &[0, 0], "exploding, lock");
    assert_eq!(u32_at(&bytes, 285), 2, "burst_count");
    assert_eq!(f32_at(&bytes, 305), 0.1, "snap value");
    assert_eq!(&bytes[309..311], &[1, 1], "inclusive, active");
    assert_eq!(f32_at(&bytes, 315), -1.0, "fade_out");
    assert_eq!(&bytes[319..322], &[0, 0, 0], "constrain");
    assert_eq!(u32_at(&bytes, 322), 2, "frames_u");
    assert_eq!(u32_at(&bytes, 326), 2, "frames_v");
    assert_eq!(u32_at(&bytes, 330), 8, "frame_rate");
    assert_eq!(bytes[334], 0, "random_start_frame");
    assert_eq!(bytes[335], 1, "keyframe count");
    assert_eq!(u32_at(&bytes, 340), 0x770, "key flags");
    assert_eq!(f32_at(&bytes, 344), 0.2, "scale_x");
    assert_eq!(u32_at(&bytes, 352), 0xFFFF_8000, "colour");
    assert_eq!(f32_at(&bytes, 356), -0.1, "mass");
    assert_eq!(u32_at(&bytes, 360), 2, "controller");
    assert_eq!(u32_at(&bytes, 364), 1, "physics_flags");
    assert_eq!(u32_at(&bytes, 368), 0, "point_count");
    assert_eq!(u32_at(&bytes, 372), 1, "position x mode");
    assert_eq!(&bytes[406..409], &[2, b'a', b'b'], "name");
    assert_eq!(&bytes[409..], &MINIMAL[39..], "tail as in §12.1");
    let back = ParticleSystem::decode(&bytes).unwrap();
    assert_eq!(back, example_system());
    assert_eq!(back.encode().unwrap(), bytes);
}

#[test]
fn defaults_match_client() {
    let e = Emitter::default();
    assert_eq!(e.max_particles, 500);
    assert_eq!(e.spawn_shape(), Some(SpawnShape::Point));
    assert_eq!(e.primitive_kind(), Some(Primitive::Quad));
    assert_eq!(e.emit_interval, 0.1);
    assert_eq!(e.burst_count, 1);
    for c in [&e.lifespan, &e.speed, &e.shape_size, &e.particle_scale] {
        assert!(c.is_constant_value(1.0));
    }
    assert_eq!(e.direction.constant_value(), Some([0.0, 0.0, 1.0]));
    assert!(e.snap.is_constant_value(0.1));
    assert!(e.streak.is_constant_value(0.0));
    assert!(e.inclusive && e.active);
    assert_eq!((e.fade_in, e.fade_out), (0.0, -1.0));
    assert_eq!((e.frames_u, e.frames_v, e.frame_rate), (1, 1, 30));
    let k = Keyframe::default();
    assert_eq!(k.flags, 0x7F0);
    assert_eq!((k.scale_x, k.scale_y, k.mass), (1.0, 1.0, 1.0));
    assert_eq!(k.colour, Argb::WHITE);
    assert_eq!((k.controller, k.physics_flags), (2, 1));

    // §4.2: 308 bytes without keyframes and name; each mode-1 keyframe adds 60.
    let mut sys = minimal_system();
    sys.emitters.push(Emitter::default());
    assert_eq!(
        sys.encode().unwrap().len(),
        PARTICLE_SYSTEM_MIN_LEN + EMITTER_MIN_LEN
    );
    sys.emitters[0].keyframes.push(Keyframe::default());
    sys.emitters[0].keyframes.push(Keyframe::default());
    assert_eq!(
        sys.encode().unwrap().len(),
        PARTICLE_SYSTEM_MIN_LEN + EMITTER_MIN_LEN + 2 * KEYFRAME_MIN_LEN
    );
    let back = ParticleSystem::decode(&sys.encode().unwrap()).unwrap();
    assert_eq!(back, sys);
}

#[test]
fn curves_of_every_mode_and_points_round_trip() {
    let mut e = example_emitter();
    let mut w = [0.0f32; 10];
    for (i, v) in w.iter_mut().enumerate() {
        *v = i as f32 * 0.5;
    }
    e.speed = Curve {
        mode: 3,
        w,
        keyed: None,
    };
    e.lifespan = Curve {
        mode: 10,
        w,
        keyed: Some(Box::new(KeyedCurve {
            period: 18.0,
            looped: true,
            pairs: vec![(1.0, 0.0), (0.5, 45.0), (1.0, 100.0)],
        })),
    };
    e.rotation.z = Curve {
        mode: 9,
        w,
        keyed: None,
    };
    e.keyframes[0].points = vec![[1.0, 2.0, 3.0], [-1.0, 0.0, 0.5]];
    e.keyframes[0].position.y = Curve {
        mode: 4,
        w,
        keyed: None,
    };
    let mut sys = minimal_system();
    sys.emitters.push(e);
    let bytes = sys.encode().unwrap();
    // Mode 3 and 9 grow by 36 bytes each, mode 10 by 36 + 9 + 3 × 8, mode 4 by 36,
    // two points by 24.
    assert_eq!(bytes.len(), 447 + 36 + 36 + 36 + 9 + 24 + 36 + 24);
    let back = ParticleSystem::decode(&bytes).unwrap();
    assert_eq!(back, sys);
    assert_eq!(back.emitters[0].lifespan.base(), 0.0);
    assert_eq!(back.emitters[0].curves().count(), 29 + 3);
    assert_eq!(sys.emitters[0].lifespan.encoded_len(), 4 + 40 + 9 + 3 * 8);
}

// ---------------------------------------------------------------------------------------------
// §12.3: behaviour views
// ---------------------------------------------------------------------------------------------

#[test]
fn example_emission_and_end() {
    let sys = example_system();
    let e = &sys.emitters[0];
    assert!((e.particles_per_sec() - 10.0).abs() < 1e-4);
    assert_eq!(e.burst_count as f32 * e.particles_per_sec(), 20.0);
    assert_eq!(e.emission_cap(), 50);
    assert_eq!(e.timed_interval(), Some(0.1));
    assert!(!sys.has_ended(1.0e6), "no limit: runs until stopped");
    assert!(e.in_emission_window(0.0));
    assert!(!e.in_emission_window(-0.01));
    assert_eq!(e.primitive_vertices(), 4);
    assert_eq!(
        sys.draw_order().map(|(i, _)| i).collect::<Vec<_>>(),
        vec![0]
    );
}

#[test]
fn end_test_and_caps() {
    let mut e = example_emitter();
    e.time_limit = 3.0;
    e.start_time = 1.0;
    // time_limit + start_time + L = 5.5
    assert!(!e.has_finished(5.49));
    assert!(e.has_finished(5.5));
    assert!(e.in_emission_window(3.0));
    assert!(!e.in_emission_window(3.01));

    let mut e = example_emitter();
    e.emission_limit = 10; // 10 × 0.1 / 2 + 1.5 = 2.0
    assert_eq!(e.emission_cap(), 10);
    assert!(!e.has_finished(2.0));
    assert!(e.has_finished(2.0001));
    // §10.6: burst_count 0, the emission-limit condition never holds.
    e.burst_count = 0;
    assert!(!e.has_finished(1.0e9));
    // §10.8: hostile intervals have no timed emission.
    for bad in [-1.0, f32::NAN, f32::INFINITY, 0.0] {
        e.emit_interval = bad;
        assert_eq!(e.timed_interval(), None);
    }
    e.emission_limit = 0;
    e.max_particles = u32::MAX;
    assert_eq!(e.emission_cap(), MAX_EMISSION_CAP);
    e.emission_limit = 7;
    assert_eq!(e.emission_cap(), 7);
    // Inactive emitters are ignored by the end test.
    let mut sys = example_system();
    sys.emitters[0].active = false;
    assert!(sys.has_ended(0.0));

    // §10.5.
    let mut sys = minimal_system();
    for bad in [0.0, -1.0, f32::NAN, f32::INFINITY] {
        sys.time_step = bad;
        assert_eq!(sys.update_interval(), FALLBACK_TIME_STEP);
    }
}

#[test]
fn atlas_frames() {
    let e = example_emitter();
    let uv = e.atlas_uv(0.0, 0.0, 0.3, 0.0);
    assert_eq!(uv, [0.5, 0.0]);
    // Frame counter runs down the rows first: age 0.125 → frame 1 → row 1, column 0.
    assert_eq!(e.atlas_uv(0.0, 0.0, 0.125, 0.0), [0.0, 0.5]);
    assert_eq!(e.atlas_uv(1.0, 1.0, 0.0, 0.0), [0.5, 0.5]);
    // Loops after U × V frames: age 0.5 → frame 4 → cell (0, 0).
    assert_eq!(e.atlas_uv(0.0, 0.0, 0.5, 0.0), [0.0, 0.0]);
    // A fixed random cell with frame rate 0: k = ⌊2 × 0.75 × 2⌋ = 3 → column 1, row 1.
    let mut f = e.clone();
    f.frame_rate = 0;
    f.random_start_frame = true;
    assert_eq!(f.atlas_uv(0.0, 0.0, 9.0, 0.75), [0.5, 0.5]);
    // No atlas: corners unchanged.
    let g = Emitter::default();
    assert_eq!(g.atlas_uv(1.0, 0.0, 3.3, 0.5), [1.0, 0.0]);
    // Hostile counts do not panic.
    let mut h = e.clone();
    h.frames_v = 0;
    h.frames_u = 3;
    let _ = h.atlas_uv(1.0, 1.0, 0.7, 0.3);
    h.random_start_frame = true;
    h.frame_rate = 0;
    let _ = h.atlas_uv(1.0, 1.0, 0.7, 0.3);
}

#[test]
fn fade_envelope() {
    let mut e = example_emitter();
    e.fade_in = 0.5;
    e.fade_out = 2.0;
    e.time_limit = 3.0;
    assert_eq!(e.fade_alpha(0.0, 0.25), 0.5);
    assert_eq!(e.fade_alpha(0.0, 1.0), 1.0);
    assert_eq!(e.fade_alpha(0.0, 2.5), 0.5);
    assert_eq!(e.fade_alpha(0.0, 4.0), 0.0);
    // The common fade_out −1 with F = 0: no fade-out.
    let f = example_emitter();
    assert_eq!(f.fade_alpha(0.0, 100.0), 1.0);
    // fast_forward shifts the envelope.
    assert_eq!(e.fade_alpha(1.0, 1.25), 0.5);
}

#[test]
fn keyframe_channels() {
    let mut e = example_emitter();
    e.keyframes.push(Keyframe {
        time: 1.0,
        flags: 0x2,
        scale_x: 1.0,
        scale_y: 1.0,
        colour: Argb(0),
        mass: 5.0,
        controller: 6,
        physics_flags: 0,
        ..Keyframe::default()
    });
    let c = e.channel_state(0.25);
    let [sx, sy] = c.scale;
    assert!((sx - 0.4).abs() < 1e-6 && (sy - 0.4).abs() < 1e-6);
    let col = c.colour;
    assert_eq!(col, Argb(0xFFFF_8000).to_rgba(), "second key lacks 0x4");
    assert_eq!(c.mass, -0.1, "no later key with 0x8: current key's mass");
    assert_eq!(c.controller_key, Some(0));
    assert_eq!(c.physics_flags, 1);
    assert_eq!(c.key, Some(0));
    // At age 1 the second key is current for scale (0x2 counts), colour stays key 0.
    let c = e.channel_state(1.0);
    assert_eq!(c.key, Some(1));
    assert_eq!(c.scale, [1.0, 1.0]);
    assert_eq!(c.scale_key, Some(1));
    assert_eq!(c.colour, Argb(0xFFFF_8000).to_rgba());
    assert_eq!(c.colour_key, Some(0));
    // Mass interpolates towards the next key with 0x8.
    e.keyframes[1].flags = 0x8;
    let c = e.channel_state(0.5);
    assert!((c.mass - (-0.1 * 0.5 + 5.0 * 0.5)).abs() < 1e-6);
    assert_eq!(e.channel_target(0, 0x8), 1);
    assert_eq!(e.channel_target(1, 0x8), 1);
    // Controller and physics keys.
    e.keyframes[1].flags = 0x300;
    e.keyframes[1].physics_flags = 0x21;
    let c = e.channel_state(2.0);
    assert_eq!(c.controller_key, Some(1));
    assert_eq!(c.physics_flags, 0x21);
    assert_eq!(e.keyframes[1].motion(), MotionController::Static);
    assert!(e.uses_controller(MotionController::Static));
    assert!(e.uses_controller(MotionController::Ballistic));
    assert!(!e.uses_controller(MotionController::Stepped));
    // No keyframes: the values of rule §10.11.
    let mut g = example_emitter();
    g.keyframes.clear();
    assert_eq!(g.channel_state(0.5), ChannelState::default());
    assert_eq!(g.channel_state(0.5).scale, [1.0, 1.0]);
    assert_eq!(g.channel_state(0.5).mass, 1.0);
    // A negative age is raised to 0 for the key walk (§8.6.1 step 2): the first key
    // (time 0) is current.
    assert_eq!(e.channel_state(-1.0).key, Some(0));
    assert_eq!(e.channel_state(f32::NAN).key, Some(0));
}

/// §8.6.1 step 2 (BIN third pass): a key is current once the age is **at least** its
/// `time`; a first key with a later time leaves every channel at the values of rule
/// §10.11 until the age reaches it.
#[test]
fn keyframe_walk_edges_and_channels_without_a_key() {
    let mut e = example_emitter();
    e.keyframes[0].time = 0.5;
    let before = e.channel_state(0.25);
    assert_eq!(before.key, None);
    assert_eq!(before, ChannelState::default());
    assert_eq!(before.colour, [1.0; 4]);
    assert_eq!(before.physics_flags, 0);
    assert_eq!(before.controller_key, None);
    // An equal age counts.
    assert_eq!(e.channel_state(0.5).key, Some(0));
    assert_eq!(e.channel_state(0.5).controller_key, Some(0));
    // A first key without the channel bits: those channels keep the rule's values.
    let mut f = example_emitter();
    f.keyframes[0].flags = KEY_CONTROLLER;
    let c = f.channel_state(1.0);
    assert_eq!(c.key, Some(0));
    assert_eq!((c.scale, c.colour, c.mass), ([1.0, 1.0], [1.0; 4], 1.0));
    assert_eq!((c.scale_key, c.colour_key, c.mass_key), (None, None, None));
    assert_eq!(c.controller_key, Some(0));
}

#[test]
fn colour_and_enums() {
    let [r, g, b, a] = Argb(0xFFFF_8000).to_rgba();
    assert_eq!((r, b, a), (1.0, 0.0, 1.0));
    assert!((g - 0.502).abs() < 1e-3);
    assert_eq!(Argb(0x1234_5678).a(), 0x12);
    assert_eq!(Primitive::Triangle.vertex_count(), 3);
    assert_eq!(Primitive::from_key(3), None);
    assert_eq!(SpawnShape::from_key(5), Some(SpawnShape::Quad));
    assert_eq!(SpawnShape::from_key(0), None);
    assert_eq!(ScaleMode::from_key(2), Some(ScaleMode::DescriptorOnly));
    assert_eq!(MotionController::from_raw(1), MotionController::PointPath);
    assert_eq!(MotionController::from_raw(77), MotionController::Static);
    assert!(is_particle_system_did(0x3900_0001u32));
    assert!(!is_particle_system_did(0x4400_0091u32));
    assert!(is_fork_system_did(0x4400_0091u32));

    let mut e = example_emitter();
    e.primitive = 9;
    assert_eq!(e.primitive_vertices(), 0);
    let mut sys = minimal_system();
    let mut tri = example_emitter();
    tri.primitive = 1;
    sys.emitters = vec![tri.clone(), example_emitter(), e, tri];
    let order: Vec<usize> = sys.draw_order().map(|(i, _)| i).collect();
    assert_eq!(order, vec![1, 0, 3]);
}

// ---------------------------------------------------------------------------------------------
// Hostile input (§4.4)
// ---------------------------------------------------------------------------------------------

#[test]
fn rejects_bad_bool_loop_and_mode() {
    let mut bad = MINIMAL;
    bad[12] = 2;
    assert!(matches!(
        ParticleSystem::decode(&bad),
        Err(FormatError::InvalidBool {
            offset: 12,
            value: 2
        })
    ));

    let good = example_system().encode().unwrap();
    // Curve mode 0 and 11 at `origin` x.
    for mode in [0u32, 11, 0xFFFF_FFFF] {
        let mut b = good.clone();
        b[39..43].copy_from_slice(&mode.to_le_bytes());
        assert!(matches!(
            ParticleSystem::decode(&b),
            Err(FormatError::Unsupported { offset: 39, .. })
        ));
    }

    // Loop byte 2 in a mode-10 curve.
    let mut sys = minimal_system();
    let mut e = example_emitter();
    e.speed = Curve {
        mode: 10,
        w: [0.0; 10],
        keyed: Some(Box::new(KeyedCurve::default())),
    };
    sys.emitters.push(e);
    let mut b = sys.encode().unwrap();
    // speed at 123: mode 4 + 40 + period 4 → loop at 171.
    assert_eq!(b[171], 0);
    b[171] = 2;
    assert!(matches!(
        ParticleSystem::decode(&b),
        Err(FormatError::InvalidBool { offset: 171, .. })
    ));
}

#[test]
fn rejects_trailing_and_truncation() {
    let good = example_system().encode().unwrap();
    let mut long = good.clone();
    long.push(0);
    assert!(matches!(
        ParticleSystem::decode(&long),
        Err(FormatError::TrailingBytes { count: 1, .. })
    ));
    for n in 0..good.len() {
        assert!(ParticleSystem::decode(&good[..n]).is_err(), "prefix {n}");
    }
}

#[test]
fn hostile_counts_fail_before_allocating() {
    // Emitter count 0x3FFFFFFF (4-byte VLE) in a 77-byte record.
    let mut b = MINIMAL[..38].to_vec();
    b.extend_from_slice(&[0xFF, 0xFF, 0xFF, 0xFF]);
    b.extend_from_slice(&MINIMAL[39..]);
    assert!(matches!(
        ParticleSystem::decode(&b),
        Err(FormatError::Truncated { .. })
    ));
    // Point count u32::MAX.
    let mut good = example_system().encode().unwrap();
    good[368..372].copy_from_slice(&u32::MAX.to_le_bytes());
    assert!(matches!(
        ParticleSystem::decode(&good),
        Err(FormatError::Truncated { .. })
    ));
    // Mode-10 pair count u32::MAX.
    let mut sys = minimal_system();
    let mut e = example_emitter();
    e.speed = Curve {
        mode: 10,
        w: [0.0; 10],
        keyed: Some(Box::new(KeyedCurve::default())),
    };
    sys.emitters.push(e);
    let mut b = sys.encode().unwrap();
    b[172..176].copy_from_slice(&u32::MAX.to_le_bytes());
    assert!(matches!(
        ParticleSystem::decode(&b),
        Err(FormatError::Truncated { .. })
    ));
}

#[test]
fn emitter_count_vle_forms() {
    // §4.1: with the 2-byte form the emitter list starts at 40, with the 4-byte form at 42.
    let mut two = MINIMAL[..38].to_vec();
    two.extend_from_slice(&[0x80, 0x00]);
    two.extend_from_slice(&MINIMAL[39..]);
    assert_eq!(ParticleSystem::decode(&two).unwrap(), minimal_system());
    let mut four = MINIMAL[..38].to_vec();
    four.extend_from_slice(&[0xC0, 0x00, 0x00, 0x00]);
    four.extend_from_slice(&MINIMAL[39..]);
    assert_eq!(ParticleSystem::decode(&four).unwrap(), minimal_system());
    // No 0xE0 escape: E0 starts the 4-byte form (value 0x20000000), not a 5-byte form.
    let mut e0 = MINIMAL[..38].to_vec();
    e0.extend_from_slice(&[0xE0, 0x00, 0x00, 0x00, 0x00]);
    e0.extend_from_slice(&MINIMAL[39..]);
    assert!(matches!(
        ParticleSystem::decode(&e0),
        Err(FormatError::Truncated { .. })
    ));
}

#[test]
fn encode_refuses_inconsistent_curves_and_names() {
    let mut sys = example_system();
    sys.emitters[0].speed.mode = 10;
    assert!(sys.encode().is_err());
    let mut sys = example_system();
    sys.emitters[0].speed.mode = 0;
    assert!(sys.encode().is_err());
    let mut sys = example_system();
    sys.emitters[0].speed.keyed = Some(Box::default());
    assert!(sys.encode().is_err());
    let mut sys = example_system();
    sys.emitters[0].name = "\u{263A}".into();
    assert!(sys.encode().is_err());
    // Latin-1 above 0x7F round-trips.
    let mut sys = example_system();
    sys.emitters[0].name = "\u{e9}t\u{e9}".into();
    let back = ParticleSystem::decode(&sys.encode().unwrap()).unwrap();
    assert_eq!(back.emitters[0].name, "\u{e9}t\u{e9}");
}

// ---------------------------------------------------------------------------------------------
// §12.4: fork systems
// ---------------------------------------------------------------------------------------------

fn identity() -> B {
    let mut b = B::new().u32(64);
    for i in 0..16 {
        b = b.f32(if i % 5 == 0 { 1.0 } else { 0.0 });
    }
    b
}

fn fork_bytes(image: &[u8]) -> Vec<u8> {
    B::new()
        .u32(image.len() as u32)
        .bytes(image)
        .u8(0)
        .pascal("")
        .tsize(0, 1)
        .pascal("a.dds")
        .pascal("fx/a.dds")
        .tsize(0, 0)
        .bytes(&identity().build())
        .tsize(0, 1)
        .u32(0x4100_1234)
        .pascal("a.dds")
        .f64(1.5)
        .f32(-1.0)
        .f32(-1.0)
        .f32(0.0)
        .f32(1.0)
        .f32(1.0)
        .f32(2.0)
        .build()
}

#[test]
fn minimal_fork_record() {
    let image = vec![0u8; 488];
    let bytes = fork_bytes(&image);
    assert_eq!(&bytes[..4], &[0xe8, 0x01, 0x00, 0x00]);
    let f = ForkSystem::decode(&bytes, &NoSchema).unwrap();
    assert_eq!(f.image.len(), 488);
    assert!(!f.flag);
    assert!(f.name.is_empty());
    assert_eq!(f.name_table.len(), 1);
    assert_eq!(f.name_table[0].key, "a.dds");
    assert_eq!(f.name_table[0].path, "fx/a.dds");
    assert!(f.properties.is_empty());
    let m = f.transform_matrix().unwrap();
    assert_eq!((m[0], m[5], m[10], m[15], m[1]), (1.0, 1.0, 1.0, 1.0, 0.0));
    assert_eq!(f.resources.len(), 1);
    assert_eq!(f.resources[0].did, Did(0x4100_1234));
    assert_eq!(f.resources[0].kind(), ForkResourceKind::RenderSurface);
    assert_eq!(f.path_of(&f.resources[0]), Some("fx/a.dds"));
    assert_eq!(f.tail_value, 1.5);
    assert_eq!(f.bounds_min, [-1.0, -1.0, 0.0]);
    assert_eq!(f.bounds_max, [1.0, 1.0, 2.0]);
    assert_eq!(f.references().collect::<Vec<_>>(), vec![Did(0x4100_1234)]);
    let h = f.image_header().unwrap();
    assert_eq!((h.word0, h.word1), (0, 0));
    assert_eq!(f.encode().unwrap(), bytes);
}

#[test]
fn fork_image_header_and_nested_resources() {
    let mut image = vec![0u8; 488 + 16];
    image[0..4].copy_from_slice(&0x50u32.to_le_bytes());
    image[4..8].copy_from_slice(&0x1E8u32.to_le_bytes());
    image[0x11C..0x120].copy_from_slice(&16u32.to_le_bytes());
    image[0x120..0x124].copy_from_slice(&488u32.to_le_bytes());
    image[488..493].copy_from_slice(b"a.dds");
    let bytes = B::new()
        .u32(image.len() as u32)
        .bytes(&image)
        .u8(1)
        .pascal("")
        .tsize(0, 2)
        .pascal("a.dds")
        .pascal("fx/a.dds")
        .pascal("b.fx")
        .pascal("fx/b.fx")
        .tsize(0, 0)
        .bytes(&identity().build())
        .tsize(0, 2)
        .u32(0x4100_0001)
        .pascal("a.dds")
        .u32(0x4400_0002)
        .pascal("b")
        .f64(0.25)
        .bytes(&[0u8; 24])
        .build();
    let f = ForkSystem::decode(&bytes, &NoSchema).unwrap();
    assert!(f.flag);
    let h = f.image_header().unwrap();
    assert_eq!((h.word0, h.word1), (0x50, 0x1E8));
    assert_eq!((h.name_block_offset, h.name_block_len), (488, 16));
    let block = f.image_name_block().unwrap();
    assert_eq!(block.len(), 16);
    assert!(block.starts_with(b"a.dds"));
    assert_eq!(f.resources[1].kind(), ForkResourceKind::ForkSystem);
    assert_eq!(f.path_of(&f.resources[1]), Some("fx/b.fx"));
    assert_eq!(f.encode().unwrap(), bytes);

    // A short image has no header.
    let short = fork_bytes(&[1, 2, 3]);
    let f = ForkSystem::decode(&short, &NoSchema).unwrap();
    assert!(f.image_header().is_none());
    assert!(f.image_name_block().is_none());
}

#[test]
fn fork_hostile_inputs() {
    let good = fork_bytes(&[0u8; 488]);
    for n in 0..good.len() {
        assert!(
            ForkSystem::decode(&good[..n], &NoSchema).is_err(),
            "prefix {n}"
        );
    }
    let mut long = good.clone();
    long.push(0);
    assert!(matches!(
        ForkSystem::decode(&long, &NoSchema),
        Err(FormatError::TrailingBytes { .. })
    ));
    // Flag byte 2.
    let mut b = good.clone();
    b[4 + 488] = 2;
    assert!(matches!(
        ForkSystem::decode(&b, &NoSchema),
        Err(FormatError::InvalidBool { .. })
    ));
    // TSize hint 23.
    let mut b = good.clone();
    b[4 + 488 + 2] = 23;
    assert!(matches!(
        ForkSystem::decode(&b, &NoSchema),
        Err(FormatError::Malformed { .. })
    ));
    // A huge blob length.
    let mut b = good.clone();
    b[..4].copy_from_slice(&u32::MAX.to_le_bytes());
    assert!(matches!(
        ForkSystem::decode(&b, &NoSchema),
        Err(FormatError::Truncated { .. })
    ));
    // A non-empty property set needs a schema.
    let props_at = 4 + 488 + 1 + 1 + 2 + 6 + 9;
    assert_eq!(&good[props_at..props_at + 2], &[0, 0]);
    let mut b = good[..props_at].to_vec();
    b.extend_from_slice(&[0, 1]);
    b.extend_from_slice(&0x77u32.to_le_bytes());
    b.extend_from_slice(&0x77u32.to_le_bytes());
    b.extend_from_slice(&5u32.to_le_bytes());
    b.extend_from_slice(&good[props_at + 2..]);
    assert!(matches!(
        ForkSystem::decode(&b, &NoSchema),
        Err(FormatError::InPropertySet { .. })
    ));
    // With a schema it decodes, but it cannot be re-encoded.
    struct Ints;
    impl tessera_formats::PropertySchema for Ints {
        fn property_type(&self, _pid: u32) -> Option<PropertyType> {
            Some(PropertyType::from_code(15))
        }
    }
    if let Ok(f) = ForkSystem::decode(&b, &Ints) {
        assert_eq!(f.properties.len(), 1);
        assert!(f.encode().is_err());
    }
}

// ---------------------------------------------------------------------------------------------
// Property tests
// ---------------------------------------------------------------------------------------------

fn curve_strategy() -> impl Strategy<Value = Curve> {
    prop_oneof![
        4 => any::<f32>().prop_map(Curve::constant),
        1 => (2u32..=9, prop::array::uniform10(any::<f32>()))
            .prop_map(|(mode, w)| Curve { mode, w, keyed: None }),
        1 => (
            prop::array::uniform10(any::<f32>()),
            any::<f32>(),
            any::<bool>(),
            prop::collection::vec((any::<f32>(), any::<f32>()), 0..4)
        )
            .prop_map(|(w, period, looped, pairs)| Curve {
                mode: 10,
                w,
                keyed: Some(Box::new(KeyedCurve { period, looped, pairs })),
            }),
    ]
}

fn keyframe_strategy() -> impl Strategy<Value = Keyframe> {
    (
        any::<f32>(),
        any::<u32>(),
        any::<u32>(),
        prop::collection::vec(prop::array::uniform3(any::<f32>()), 0..3),
        curve_strategy(),
    )
        .prop_map(|(time, flags, colour, points, y)| Keyframe {
            time,
            flags,
            colour: Argb(colour),
            points,
            position: CurveVec {
                y,
                ..CurveVec::default()
            },
            ..Keyframe::default()
        })
}

fn emitter_strategy() -> impl Strategy<Value = Emitter> {
    (
        curve_strategy(),
        curve_strategy(),
        any::<u32>(),
        any::<f32>(),
        prop::collection::vec(keyframe_strategy(), 0..4),
        "[ -~\u{a0}-\u{ff}]{0,12}",
        any::<[bool; 3]>(),
    )
        .prop_map(
            |(speed, lifespan, max_particles, emit_interval, keyframes, name, constrain)| Emitter {
                speed,
                lifespan,
                max_particles,
                emit_interval,
                keyframes,
                name,
                constrain,
                ..Emitter::default()
            },
        )
}

proptest! {
    #[test]
    fn generated_systems_round_trip(
        emitters in prop::collection::vec(emitter_strategy(), 0..4),
        time_step in any::<f32>(),
        priority in any::<i32>(),
    ) {
        let mut sys = minimal_system();
        sys.emitters = emitters;
        sys.time_step = time_step;
        sys.sorting_priority = priority;
        let bytes = sys.encode().unwrap();
        let back = ParticleSystem::decode(&bytes).unwrap();
        prop_assert_eq!(back.encode().unwrap(), bytes);
    }

    #[test]
    fn arbitrary_bytes_never_panic(data in prop::collection::vec(any::<u8>(), 0..2048)) {
        if let Ok(s) = ParticleSystem::decode(&data) {
            let again = s.encode().unwrap();
            prop_assert!(again.len() <= data.len());
        }
        if let Ok(f) = ForkSystem::decode(&data, &NoSchema)
            && let Ok(again) = f.encode()
        {
            prop_assert!(again.len() <= data.len());
        }
    }

    #[test]
    fn mutated_examples_never_panic(idx in 0usize..447, byte in any::<u8>()) {
        let mut b = example_system().encode().unwrap();
        b[idx] = byte;
        if let Ok(s) = ParticleSystem::decode(&b) {
            for e in &s.emitters {
                let _ = e.channel_state(0.5);
                let _ = e.atlas_uv(1.0, 1.0, 0.5, 0.5);
                let _ = e.fade_alpha(s.fast_forward, 0.5);
                let _ = e.has_finished(1.0);
            }
        }
    }
}
