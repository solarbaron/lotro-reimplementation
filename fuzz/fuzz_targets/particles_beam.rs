// SPDX-License-Identifier: GPL-3.0-or-later
//! Beams on hostile Beam nodes (`crates/particles` `beam`; `docs/specs/particle-runtime.md`
//! §11.1, rule 14).
//!
//! The input fills every field of a Beam node (floats from raw bits, so NaN, infinities
//! and huge values occur) and then drives up to 64 updates with frame steps, moving ends,
//! a stop, and the beam and head systems switched on and off. Nothing may panic or hang;
//! every spawn and span is finite; no update spawns more than `MAX_BEAM_SPAWNS` particles;
//! the ribbon layout stays within `MAX_RIBBON_SEGMENTS`; the ribbon's quads (four corners per
//! segment, every corner and both texture-coordinate pairs finite), its colour (each channel in 0..=1), the beam's frame and the
//! head entity's scale vector stay finite.

#![no_main]

use libfuzzer_sys::fuzz_target;
use tessera_formats::Did;
use tessera_formats::script_table::BeamNode;
use tessera_particles::Ran2;
use tessera_particles::beam::{
    Beam, BeamEnds, MAX_BEAM_SPAWNS, MAX_RIBBON_SEGMENTS, beam_rotation, head_scale_vector,
    ribbon_colour, ribbon_mesh, ribbon_segments,
};
use tessera_particles::glam::Vec3;

struct Input<'a> {
    data: &'a [u8],
    at: usize,
}

impl Input<'_> {
    fn u32(&mut self) -> u32 {
        let mut b = [0u8; 4];
        for x in &mut b {
            *x = self.data.get(self.at).copied().unwrap_or(0);
            self.at += 1;
        }
        u32::from_le_bytes(b)
    }
    fn f32(&mut self) -> f32 {
        f32::from_bits(self.u32())
    }
    fn i32(&mut self) -> i32 {
        self.u32().cast_signed()
    }
    fn b(&mut self) -> bool {
        self.u32() & 1 == 1
    }
    fn v3(&mut self) -> [f32; 3] {
        [self.f32(), self.f32(), self.f32()]
    }
    fn done(&self) -> bool {
        self.at >= self.data.len()
    }
}

fuzz_target!(|data: &[u8]| {
    let mut i = Input { data, at: 0 };
    let node = BeamNode {
        persistent: i.b(),
        use_target_position: i.b(),
        width: i.f32(),
        untargeted_length: i.f32(),
        segment_size: i.f32(),
        head_segments: i.i32(),
        tail_segments: i.i32(),
        texture_u_scale: i.f32(),
        head_start: i.f32(),
        head_start_time: i.f32(),
        head_speed: i.f32(),
        tail_start: i.f32(),
        tail_start_time: i.f32(),
        tail_speed: i.f32(),
        ramp_in: i.f32(),
        sustain: i.f32(),
        ramp_out: i.f32(),
        source_detach_time: i.f32(),
        target_detach_time: i.f32(),
        detach_before_ramping_out: i.b(),
        random_start_radius: i.f32(),
        random_impact_radius: i.f32(),
        beam_particles_per_second: i.i32(),
        head_particles_per_second: i.i32(),
        head_entity: Did(i.u32()),
        head_entity_scale_source: i.u32(),
        color: i.u32().to_le_bytes(),
        // Without a body material no ribbon is built (§11.1 RT45).
        body_material: Did(i.u32()),
        ..BeamNode::default()
    };
    let segs = ribbon_segments(&node, i.f32());
    assert!(segs.len() <= MAX_RIBBON_SEGMENTS);
    let mut rng = Ran2::new(i.u32());
    let mut now = f64::from(i.f32());
    let mut beam = Beam::new(&node, now, &mut rng);
    for _ in 0..64 {
        if i.done() {
            break;
        }
        let dt = i.f32();
        now += f64::from(dt);
        let flags = i.u32();
        if flags & 1 == 1 {
            beam.stop(now);
        }
        let ends = BeamEnds {
            source: (flags & 2 == 0).then(|| Vec3::from_array(i.v3())),
            source_forward: Vec3::from_array(i.v3()),
            target: (flags & 4 == 0).then(|| Vec3::from_array(i.v3())),
        };
        let u = beam.update(now, dt, ends, flags & 8 == 0, flags & 16 == 0, &mut rng);
        assert!(u.beam_spawns.len() <= MAX_BEAM_SPAWNS);
        for (t, p) in &u.beam_spawns {
            assert!(t.is_finite() && p.is_finite());
        }
        if let Some((t, p)) = u.head_spawn {
            assert!(t.is_finite() && p.is_finite());
        }
        if let Some(s) = u.span {
            assert!(s.tail.is_finite() && s.head.is_finite());
            assert!((0.0..=1.0).contains(&s.intensity));
            let eye = Vec3::from_array(i.v3());
            let mesh = ribbon_mesh(&node, &s, eye);
            let corners =
                mesh.tail.vertices.len() + mesh.body.vertices.len() + mesh.head.vertices.len();
            assert!(corners <= 4 * MAX_RIBBON_SEGMENTS);
            for part in [&mesh.tail, &mesh.body, &mesh.head] {
                assert_eq!(part.vertices.len() % 4, 0);
                for v in &part.vertices {
                    assert!(
                        v.position.is_finite() && v.uv.iter().chain(&v.uv2).all(|c| c.is_finite())
                    );
                }
            }
            assert!(
                ribbon_colour(&node, s.intensity)
                    .iter()
                    .all(|c| (0.0..=1.0).contains(c))
            );
            if let Some(q) = beam_rotation(s.source, s.target) {
                assert!(q.is_finite());
            }
            let scale = head_scale_vector(node.head_entity_scale_source, eye);
            assert!(scale.is_finite());
        }
        let _ = beam.finished(now);
    }
});
