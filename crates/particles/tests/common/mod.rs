// SPDX-License-Identifier: GPL-3.0-or-later
//! Synthetic records shared by the integration tests. Every value is invented.

#![allow(dead_code)]

use tessera_formats::Did;
use tessera_formats::gameplay::particles::{
    Argb, Curve, CurveVec, Emitter, Keyframe, ParticleSystem,
};

/// The system of `particle-runtime.md` §14.1.
pub fn worked_example() -> ParticleSystem {
    let emitter = Emitter {
        shape: 4,
        primitive: 2,
        emit_interval: 0.25,
        speed: Curve::constant(2.0),
        direction: CurveVec::constant([0.0, 0.0, 1.0]),
        min_spread: Curve::constant(0.0),
        max_spread: Curve::constant(0.0),
        emission_limit: 4,
        max_particles: 10,
        burst_count: 2,
        lifespan: Curve::constant(1.0),
        particle_scale: Curve::constant(1.0),
        rotation: CurveVec::constant([0.0; 3]),
        streak: Curve::constant(0.0),
        start_time: 0.25,
        time_limit: 0.0,
        emission_distance: 0.0,
        active: true,
        fade_in: 0.5,
        fade_out: -1.0,
        frames_u: 1,
        frames_v: 1,
        holding_location: 0,
        bone: 0,
        keyframes: vec![
            Keyframe {
                time: 0.0,
                flags: 0x770,
                scale_x: 0.2,
                scale_y: 0.2,
                colour: Argb(0xFFFF_FFFF),
                mass: 1.0,
                controller: 2,
                physics_flags: 0x1,
                ..Keyframe::default()
            },
            Keyframe {
                time: 1.0,
                flags: 0x6,
                scale_x: 0.6,
                scale_y: 0.6,
                colour: Argb(0x00FF_0000),
                mass: 1.0,
                controller: 2,
                physics_flags: 0,
                ..Keyframe::default()
            },
        ],
        ..Emitter::default()
    };
    ParticleSystem {
        time_step: 0.1,
        fast_forward: 0.0,
        physics_duration: 0.0,
        world_space: true,
        force_draw: false,
        uses_video_post: false,
        draw_reflection: false,
        use_lighting: false,
        inherit_opacity: false,
        far_fade_start: 40.0,
        far_fade_end: 60.0,
        near_fade_start: 0.0,
        near_fade_end: 0.0,
        scale_mode: 1,
        emitters: vec![emitter],
        has_bounds: false,
        bounds_min: [0.0; 3],
        bounds_max: [0.0; 3],
        sorting_priority: 5,
        shareable: false,
        surface: Did(0x3100_0001),
        extra_surface: Did(0),
    }
}

/// Clock of frame `k` in the worked example: 10.00 + 0.04 k (§14.1).
pub fn clock(k: u64) -> f64 {
    10.0 + 0.04 * k as f64
}

/// Frame index of a clock reading of the worked example.
pub fn frame_at(t: f64) -> u64 {
    ((t - 10.0) / 0.04).round() as u64
}
