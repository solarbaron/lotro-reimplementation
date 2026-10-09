// SPDX-License-Identifier: GPL-3.0-or-later
//! The particle pipeline (`docs/specs/particle-runtime.md` §9; `docs/specs/particles.md`
//! §8.10–§8.12) on one synthetic particle system simulated by `tessera-particles`:
//!
//! - `particles_fountain.png`: a fountain of textured quads (ballistic under gravity, scale
//!   and colour keys, a 2 × 2 texture atlas with random start frames, the emitter fade-in)
//!   and a second emitter of rising, spinning triangles, drawn alpha-blended with the
//!   depth test against an opaque ground and a pillar that hides part of the fountain;
//! - pixel checks: blend modes (additive brightens, alpha covers), the colour multiplier,
//!   the depth test and the two draw calls of one draw.
//!
//! The system and texture are invented here; no game data. Regenerate with
//! `TESSERA_BLESS=1 cargo test -p tessera-render --test particles_golden`.

#[path = "support/golden.rs"]
mod golden;
#[path = "support/gpu.rs"]
mod gpu;

use std::path::Path;
use std::sync::Arc;

use tessera_formats::Did;
use tessera_formats::gameplay::particles::{
    Argb, Curve, CurveVec, Emitter, Keyframe, ParticleSystem,
};
use tessera_particles::{
    CameraView, Environment, OriginAttachments, Placement, Ran2, SystemDraw, SystemSim,
};
use tessera_render::glam::{Mat4, Vec3};
use tessera_render::{
    AlphaMode, Camera, ColorSpace, Filter, Material, MeshData, MeshDraw, OffscreenFormat,
    ParticleDraw, ParticleVertex, RenderError, Renderer, RgbaImage, TextureId, TextureUpload, Wrap,
};

const WIDTH: u32 = 320;
const HEIGHT: u32 = 240;
const CLEAR: [f32; 4] = [0.02, 0.03, 0.06, 1.0];

/// A 2 × 2 atlas of soft shapes (disc, ring, diamond, cross) in a 64 × 64 texture.
fn atlas() -> Vec<u8> {
    let size = 64u32;
    let mut out = Vec::with_capacity((size * size * 4) as usize);
    for y in 0..size {
        for x in 0..size {
            let cell = (x / 32) + 2 * (y / 32);
            let u = (x % 32) as f32 / 31.0 * 2.0 - 1.0;
            let v = (y % 32) as f32 / 31.0 * 2.0 - 1.0;
            let r = (u * u + v * v).sqrt();
            let a = match cell {
                0 => (1.0 - r).clamp(0.0, 1.0),
                1 => (1.0 - ((r - 0.6).abs() * 4.0)).clamp(0.0, 1.0),
                2 => (1.0 - (u.abs() + v.abs())).clamp(0.0, 1.0),
                _ => (1.0 - u.abs().min(v.abs()) * 5.0).clamp(0.0, 1.0) * (1.0 - r).clamp(0.0, 1.0),
            };
            let tint = [
                [255, 255, 255],
                [255, 220, 160],
                [200, 230, 255],
                [255, 200, 255],
            ][cell as usize];
            out.extend_from_slice(&[tint[0], tint[1], tint[2], (a * 255.0) as u8]);
        }
    }
    out
}

fn fountain() -> ParticleSystem {
    let sparks = Emitter {
        shape: 2,
        primitive: 2,
        emit_interval: 0.02,
        burst_count: 1,
        max_particles: 200,
        speed: Curve::constant(5.0),
        direction: CurveVec::constant([0.0, 0.0, 1.0]),
        min_spread: Curve::constant(5.0),
        max_spread: Curve::constant(20.0),
        lifespan: Curve::constant(1.4),
        shape_size: Curve::constant(0.3),
        particle_scale: Curve::constant(1.0),
        fade_in: 0.3,
        fade_out: -1.0,
        frames_u: 2,
        frames_v: 2,
        frame_rate: 0,
        random_start_frame: true,
        keyframes: vec![
            Keyframe {
                time: 0.0,
                flags: 0x770,
                scale_x: 0.25,
                scale_y: 0.25,
                colour: Argb(0xFFFF_E080),
                mass: 1.0,
                controller: 2,
                physics_flags: 1,
                ..Keyframe::default()
            },
            Keyframe {
                time: 1.4,
                flags: 0x6,
                scale_x: 0.6,
                scale_y: 0.6,
                colour: Argb(0x00FF_3010),
                mass: 1.0,
                controller: 2,
                physics_flags: 1,
                ..Keyframe::default()
            },
        ],
        ..Emitter::default()
    };
    let wisps = Emitter {
        shape: 3,
        primitive: 1,
        emit_interval: 0.1,
        burst_count: 1,
        max_particles: 40,
        speed: Curve::constant(0.6),
        direction: CurveVec::constant([0.0, 0.0, 1.0]),
        lifespan: Curve::constant(2.5),
        shape_size: Curve::constant(1.2),
        inclusive: true,
        rotation_velocity: CurveVec::constant([0.0, 0.0, 2.0]),
        origin: CurveVec::constant([0.0, 0.0, 0.3]),
        keyframes: vec![
            Keyframe {
                time: 0.0,
                flags: 0x770,
                scale_x: 0.35,
                scale_y: 0.35,
                colour: Argb(0xC060_C0FF),
                mass: 0.0,
                controller: 2,
                physics_flags: 0,
                ..Keyframe::default()
            },
            Keyframe {
                time: 2.5,
                flags: 0x4,
                colour: Argb(0x0020_40FF),
                ..Keyframe::default()
            },
        ],
        ..Emitter::default()
    };
    ParticleSystem {
        time_step: 1.0 / 30.0,
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
        emitters: vec![sparks, wisps],
        has_bounds: false,
        bounds_min: [0.0; 3],
        bounds_max: [0.0; 3],
        sorting_priority: 5,
        shareable: false,
        surface: Did(0x3100_0001),
        extra_surface: Did(0),
    }
}

fn camera() -> Camera {
    Camera {
        eye: Vec3::new(0.0, -4.2, 1.5),
        target: Vec3::new(0.0, 0.0, 0.9),
        ..Camera::default()
    }
}

/// The fountain simulated for 2 s at 30 frames per second with seed 1234, as drawn in the
/// last frame.
fn simulated() -> SystemDraw {
    let mut sim = SystemSim::new(Arc::new(fountain()), 0.0);
    let mut rng = Ran2::new(1234);
    let cam = camera();
    let view = CameraView::look_at(cam.eye, cam.target);
    let mut last = None;
    for k in 0..=60u64 {
        let now = k as f64 / 30.0;
        sim.place(
            k,
            now,
            Placement::default(),
            &Environment::default(),
            &mut rng,
            &OriginAttachments,
        );
        last = sim.draw(now, &view, &mut rng);
    }
    last.expect("the fountain is drawn")
}

fn draws(sys: &SystemDraw, texture: Option<TextureId>, alpha: AlphaMode) -> Vec<ParticleDraw> {
    sys.instances
        .iter()
        .map(|i| ParticleDraw {
            vertices: i
                .vertices
                .iter()
                .map(|v| ParticleVertex {
                    position: v.position,
                    color: v.colour,
                    uv: v.uv,
                })
                .collect(),
            quad_vertices: i.quad_vertices,
            texture,
            alpha,
            tint: [1.0, 1.0, 1.0, i.opacity],
        })
        .collect()
}

fn scene(r: &mut Renderer<'static>) -> Result<RgbaImage, RenderError> {
    let texture = r.upload_texture(&TextureUpload {
        color_space: ColorSpace::Srgb,
        filter: Filter::Linear,
        wrap: Wrap::Clamp,
        ..TextureUpload::rgba8(64, 64, &atlas())
    })?;
    let ground = r.upload_mesh(&MeshData::plane(12.0, 12.0, 1, 1))?;
    let pillar = r.upload_mesh(&MeshData::cube(0.3))?;
    let sys = simulated();
    assert!(sys.instances[0].quads() > 50 && sys.instances[0].triangles() > 10);
    let target = r.create_offscreen_target(WIDTH, HEIGHT, OffscreenFormat::Rgba8Srgb)?;
    let mut frame = r.begin_offscreen_frame(&target)?;
    let pass = frame.world_pass("world", &camera());
    pass.clear(CLEAR)
        .mesh(MeshDraw::new(
            ground,
            Mat4::IDENTITY,
            Material::color([0.12, 0.13, 0.12, 1.0]),
        ))
        .mesh(MeshDraw::new(
            pillar,
            Mat4::from_translation(Vec3::new(-0.45, -1.2, 0.3))
                * Mat4::from_scale(Vec3::new(0.5, 0.5, 4.0)),
            Material::color([0.35, 0.3, 0.28, 1.0]),
        ));
    for d in draws(&sys, Some(texture), AlphaMode::Blend) {
        pass.particles(d);
    }
    let stats = r.end_frame(frame)?;
    assert_eq!(stats.particle_draws, 1, "{stats:?}");
    r.read_pixels(&target)
}

#[test]
fn particles_fountain_golden() -> Result<(), RenderError> {
    let Some(mut r) = gpu::headless_renderer(CLEAR) else {
        return Ok(());
    };
    let img = scene(&mut r)?;
    let again = scene(&mut r)?;
    assert!(img.data == again.data, "render is not deterministic");
    golden::check(
        &Path::new(env!("CARGO_MANIFEST_DIR")).join("tests/golden/particles_fountain.png"),
        &img,
        golden::Tolerance::default(),
    );
    Ok(())
}

/// One quad and one triangle at the centre of a 16 × 16 target, drawn with `alpha` and
/// `tint` over a mid-grey background; returns the centre pixel.
fn centre_pixel(
    r: &mut Renderer<'static>,
    alpha: AlphaMode,
    color: [f32; 4],
    tint: [f32; 4],
    behind_wall: bool,
) -> Result<[u8; 4], RenderError> {
    let target = r.create_offscreen_target(16, 16, OffscreenFormat::Rgba8Unorm)?;
    let wall = r.upload_mesh(&MeshData::plane(4.0, 4.0, 1, 1))?;
    let cam = Camera {
        eye: Vec3::new(0.0, 0.0, 5.0),
        target: Vec3::ZERO,
        up: Vec3::Y,
        ..Camera::default()
    };
    let v = |x: f32, y: f32| ParticleVertex {
        position: [x, y, 0.0],
        color,
        uv: [0.5, 0.5],
    };
    // A quad covering the centre, then a triangle covering it too.
    let vertices = vec![
        v(-1.0, 1.0),
        v(-1.0, -1.0),
        v(1.0, -1.0),
        v(1.0, 1.0),
        v(-1.0, -1.0),
        v(1.0, -1.0),
        v(0.0, 1.0),
    ];
    let mut frame = r.begin_offscreen_frame(&target)?;
    let pass = frame.world_pass("p", &cam);
    pass.clear([0.5, 0.5, 0.5, 1.0]);
    if behind_wall {
        pass.mesh(MeshDraw::new(
            wall,
            Mat4::from_translation(Vec3::new(0.0, 0.0, 1.0)),
            Material {
                unlit: true,
                ..Material::color([0.0, 0.0, 1.0, 1.0])
            },
        ));
    }
    pass.particles(ParticleDraw {
        tint,
        alpha,
        ..ParticleDraw::new(vertices, 4)
    });
    let stats = r.end_frame(frame)?;
    assert_eq!(stats.particle_draws, 1);
    Ok(r.read_pixels(&target)?.pixel(8, 8).unwrap_or_default())
}

#[test]
fn blend_modes_tint_and_depth() -> Result<(), RenderError> {
    let Some(mut r) = gpu::headless_renderer(CLEAR) else {
        return Ok(());
    };
    // Alpha blend: two layers of red at alpha 0.5 over grey (quad, then triangle).
    let p = centre_pixel(
        &mut r,
        AlphaMode::Blend,
        [1.0, 0.0, 0.0, 0.5],
        [1.0; 4],
        false,
    )?;
    let expect = |c: f32| (c * 255.0).round() as i32;
    assert!((i32::from(p[0]) - expect(0.875)).abs() <= 2, "{p:?}");
    assert!((i32::from(p[1]) - expect(0.125)).abs() <= 2, "{p:?}");
    // Additive: grey + 2 × 0.25 green.
    let p = centre_pixel(
        &mut r,
        AlphaMode::Additive,
        [0.0, 0.5, 0.0, 0.5],
        [1.0; 4],
        false,
    )?;
    assert!(
        (i32::from(p[1]) - expect(1.0)).abs() <= 2 && (i32::from(p[0]) - 128).abs() <= 2,
        "{p:?}"
    );
    // The colour multiplier's alpha 0 hides the draw.
    let p = centre_pixel(
        &mut r,
        AlphaMode::Blend,
        [1.0, 0.0, 0.0, 1.0],
        [1.0, 1.0, 1.0, 0.0],
        false,
    )?;
    assert!(
        (i32::from(p[0]) - 128).abs() <= 2 && (i32::from(p[1]) - 128).abs() <= 2,
        "{p:?}"
    );
    // Alpha test discards below the cutoff.
    let p = centre_pixel(
        &mut r,
        AlphaMode::Mask { cutoff: 0.6 },
        [1.0, 0.0, 0.0, 0.5],
        [1.0; 4],
        false,
    )?;
    assert!((i32::from(p[0]) - 128).abs() <= 2, "{p:?}");
    // The depth test: a wall nearer the camera hides the particles.
    let p = centre_pixel(
        &mut r,
        AlphaMode::Blend,
        [1.0, 0.0, 0.0, 1.0],
        [1.0; 4],
        true,
    )?;
    assert!(p[2] > 200 && p[0] < 20, "{p:?}");
    Ok(())
}

#[test]
fn hostile_draws_do_not_fail() -> Result<(), RenderError> {
    let Some(mut r) = gpu::headless_renderer(CLEAR) else {
        return Ok(());
    };
    let target = r.create_offscreen_target(16, 16, OffscreenFormat::Rgba8Unorm)?;
    let gone = r.upload_texture(&TextureUpload::rgba8(1, 1, &[255; 4]))?;
    assert!(r.destroy_texture(gone));
    let bad = ParticleVertex {
        position: [f32::NAN, f32::INFINITY, 0.0],
        color: [f32::NAN; 4],
        uv: [f32::INFINITY; 2],
    };
    let mut frame = r.begin_offscreen_frame(&target)?;
    let pass = frame.world_pass("p", &camera());
    // More vertices than one draw holds, a quad count beyond the vertices, an unknown
    // texture, partial primitives and an empty draw.
    pass.particles(ParticleDraw::new(vec![bad; 40_000], 50_000))
        .particles(ParticleDraw {
            texture: Some(gone),
            ..ParticleDraw::new(vec![bad; 5], 2)
        })
        .particles(ParticleDraw::new(Vec::new(), 0));
    frame
        .pass("screen", tessera_render::PassSpace::Screen)
        .particles(ParticleDraw::new(vec![bad; 4], 4));
    let stats = r.end_frame(frame)?;
    // The cut draw and the triangle of the second; the empty one and the screen pass
    // draw nothing.
    assert_eq!(stats.particle_draws, 2, "{stats:?}");
    Ok(())
}

/// The pass's vertex budget ([`MAX_PARTICLE_VERTICES_PER_PASS`]): draws beyond it are
/// skipped whole instead of failing the frame with one oversized buffer.
#[test]
fn the_pass_vertex_budget_skips_whole_draws() -> Result<(), RenderError> {
    use tessera_render::{MAX_PARTICLE_DRAW_VERTICES, MAX_PARTICLE_VERTICES_PER_PASS};
    let Some(mut r) = gpu::headless_renderer(CLEAR) else {
        return Ok(());
    };
    let target = r.create_offscreen_target(16, 16, OffscreenFormat::Rgba8Unorm)?;
    let v = ParticleVertex {
        position: [0.0, 0.0, -50.0],
        color: [1.0; 4],
        uv: [0.0; 2],
    };
    let fit = MAX_PARTICLE_VERTICES_PER_PASS / MAX_PARTICLE_DRAW_VERTICES;
    let mut frame = r.begin_offscreen_frame(&target)?;
    let pass = frame.world_pass("p", &camera());
    for _ in 0..fit + 8 {
        pass.particles(ParticleDraw::new(
            vec![v; MAX_PARTICLE_DRAW_VERTICES],
            MAX_PARTICLE_DRAW_VERTICES,
        ));
    }
    let stats = r.end_frame(frame)?;
    assert!(stats.particle_draws as usize <= fit, "{stats:?}");
    assert!(stats.particle_vertices as usize <= MAX_PARTICLE_VERTICES_PER_PASS);
    Ok(())
}
