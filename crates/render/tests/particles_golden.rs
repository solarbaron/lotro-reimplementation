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
    AlphaMode, AlphaThreshold, Camera, ColorSpace, Filter, FixedBlend, Fog, FrameLighting,
    Material, MeshData, MeshDraw, OffscreenFormat, ParticleDraw, ParticleLayers, ParticleShading,
    ParticleVertex, RenderError, Renderer, RgbaImage, TextureId, TextureUpload, UvTransform, Wrap,
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
            ..ParticleDraw::default()
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

/// One quad and one triangle covering the centre of a 16 × 16 target (vertex colour
/// `color`, texture coordinates at the texel centre).
fn centre_draw(color: [f32; 4]) -> ParticleDraw {
    let v = |x: f32, y: f32| ParticleVertex {
        position: [x, y, 0.0],
        color,
        uv: [0.5, 0.5],
    };
    let vertices = vec![
        v(-1.0, 1.0),
        v(-1.0, -1.0),
        v(1.0, -1.0),
        v(1.0, 1.0),
        v(-1.0, -1.0),
        v(1.0, -1.0),
        v(0.0, 1.0),
    ];
    ParticleDraw::new(vertices, 4)
}

/// Draws `draw` over a mid-grey background (behind a nearer blue wall when `behind_wall`,
/// in a lit pass with `fog` when given) and returns the centre pixel.
fn centre_pixel_of(
    r: &mut Renderer<'static>,
    draw: ParticleDraw,
    behind_wall: bool,
    fog: Option<Fog>,
) -> Result<[u8; 4], RenderError> {
    let target = r.create_offscreen_target(16, 16, OffscreenFormat::Rgba8Unorm)?;
    let wall = r.upload_mesh(&MeshData::plane(4.0, 4.0, 1, 1))?;
    let cam = Camera {
        eye: Vec3::new(0.0, 0.0, 5.0),
        target: Vec3::ZERO,
        up: Vec3::Y,
        ..Camera::default()
    };
    let mut frame = r.begin_offscreen_frame(&target)?;
    let pass = frame.world_pass("p", &cam);
    pass.clear([0.5, 0.5, 0.5, 1.0]);
    if let Some(f) = fog {
        pass.frame_lighting(FrameLighting {
            fog: Some(f),
            ..FrameLighting::default()
        });
    }
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
    pass.particles(draw);
    let stats = r.end_frame(frame)?;
    assert_eq!(stats.particle_draws, 1);
    Ok(r.read_pixels(&target)?.pixel(8, 8).unwrap_or_default())
}

/// [`centre_draw`] drawn with `alpha` and `tint`.
fn centre_pixel(
    r: &mut Renderer<'static>,
    alpha: AlphaMode,
    color: [f32; 4],
    tint: [f32; 4],
    behind_wall: bool,
) -> Result<[u8; 4], RenderError> {
    let draw = ParticleDraw {
        tint,
        alpha,
        ..centre_draw(color)
    };
    centre_pixel_of(r, draw, behind_wall, None)
}

/// A 1 × 1 linear texture of `rgba`.
fn texel(r: &mut Renderer<'static>, rgba: [u8; 4]) -> Result<TextureId, RenderError> {
    r.upload_texture(&TextureUpload {
        color_space: ColorSpace::Linear,
        ..TextureUpload::rgba8(1, 1, &rgba)
    })
}

fn near(p: u8, v: f32) -> bool {
    (i32::from(p) - (v * 255.0).round() as i32).abs() <= 2
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
    // The alpha test reads the texture's opacity alone (particle-runtime.md §9.1.1 rule 1):
    // a vertex alpha below the cut-off removes nothing.
    let p = centre_pixel(
        &mut r,
        AlphaMode::Mask { cutoff: 0.6 },
        [1.0, 0.0, 0.0, 0.5],
        [1.0; 4],
        false,
    )?;
    assert!(p[0] > 250 && p[1] < 5, "{p:?}");
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

/// `particle-runtime.md` §9.1.1 rule 1 with `material.md` §11.2 per draw: the texture's
/// opacity is tested against the reference, which the draw's *A* = material alpha × tint
/// alpha adjusts.
#[test]
fn alpha_test_reads_the_texture_with_the_per_draw_reference() -> Result<(), RenderError> {
    let Some(mut r) = gpu::headless_renderer(CLEAR) else {
        return Ok(());
    };
    let half = texel(&mut r, [255, 255, 255, 128])?;
    let clear = texel(&mut r, [255, 255, 255, 0])?;
    let red = [1.0, 0.0, 0.0, 1.0];
    // Texture opacity 0.5 at or below the cut-off 0.6: discarded.
    let draw = |alpha: AlphaMode, tex: TextureId, tint: [f32; 4]| ParticleDraw {
        texture: Some(tex),
        alpha,
        tint,
        ..centre_draw(red)
    };
    let p = centre_pixel_of(
        &mut r,
        draw(AlphaMode::Mask { cutoff: 0.6 }, half, [1.0; 4]),
        false,
        None,
    )?;
    assert!(near(p[0], 0.5) && near(p[1], 0.5), "{p:?}");
    // Depth write on: the reference becomes 0.6 × A; with A = 0.5 the texel is kept.
    let p = centre_pixel_of(
        &mut r,
        draw(
            AlphaMode::TestBlend { cutoff: 0.6 },
            half,
            [1.0, 1.0, 1.0, 0.5],
        ),
        false,
        None,
    )?;
    assert!(p[0] > 140 && p[1] < 120, "{p:?}");
    // Depth write off: the test is switched off while A < 0.9 (the reference becomes 0).
    let tested = AlphaMode::Fixed(FixedBlend {
        alpha_test: Some(AlphaThreshold(0.6)),
        ..FixedBlend::ADDITIVE
    });
    let p = centre_pixel_of(&mut r, draw(tested, half, [1.0; 4]), false, None)?;
    assert!(near(p[0], 0.5), "kept off at A = 1: {p:?}");
    // (two additive layers of red × 0.5 × 0.8 over grey saturate red)
    let p = centre_pixel_of(
        &mut r,
        draw(tested, half, [1.0, 1.0, 1.0, 0.8]),
        false,
        None,
    )?;
    assert!(p[0] > 250 && near(p[1], 0.5), "drawn at A = 0.8: {p:?}");
    // Alpha test off in a source-alpha blend: texels of opacity 0 leave the image as it is …
    let p = centre_pixel_of(&mut r, draw(AlphaMode::Blend, clear, [1.0; 4]), false, None)?;
    assert!(near(p[0], 0.5) && near(p[1], 0.5), "{p:?}");
    // … but a program that tests nothing and leaves the texture alpha out (0x2B0007F2)
    // draws them.
    let no_test = ParticleDraw {
        shading: ParticleShading {
            tests_texture: false,
            texture_alpha: false,
            ..ParticleShading::STANDARD
        },
        ..draw(AlphaMode::Blend, clear, [1.0; 4])
    };
    let p = centre_pixel_of(&mut r, no_test, false, None)?;
    assert!(p[0] > 250 && p[1] < 5, "{p:?}");
    Ok(())
}

/// `particle-runtime.md` §9.1.1: output colour and alpha per program.
#[test]
fn surface_programs_colour_and_alpha() -> Result<(), RenderError> {
    let Some(mut r) = gpu::headless_renderer(CLEAR) else {
        return Ok(());
    };
    // M = material colour 2000 × tint.
    let p = centre_pixel_of(
        &mut r,
        ParticleDraw {
            material: [0.0, 1.0, 0.0, 1.0],
            ..centre_draw([1.0; 4])
        },
        false,
        None,
    )?;
    assert!(p[0] < 5 && p[1] > 250, "{p:?}");
    // The material alpha enters the output alpha (two layers of red at 0.5 over grey).
    let p = centre_pixel_of(
        &mut r,
        ParticleDraw {
            material: [1.0, 0.0, 0.0, 0.5],
            ..centre_draw([1.0; 4])
        },
        false,
        None,
    )?;
    assert!(near(p[0], 0.875) && near(p[1], 0.125), "{p:?}");
    // Doubling: 2 × 0.25 red.
    let doubled = ParticleShading {
        doubled: true,
        ..ParticleShading::STANDARD
    };
    let p = centre_pixel_of(
        &mut r,
        ParticleDraw {
            shading: doubled,
            ..centre_draw([0.25, 0.0, 0.0, 1.0])
        },
        false,
        None,
    )?;
    assert!(near(p[0], 0.5) && p[1] < 5, "{p:?}");
    // No vertex colour (0x2B000755): T × M, alpha T.a × M.a.
    let p = centre_pixel_of(
        &mut r,
        ParticleDraw {
            shading: ParticleShading {
                vertex_colour: false,
                ..ParticleShading::STANDARD
            },
            ..centre_draw([0.0; 4])
        },
        false,
        None,
    )?;
    assert!(p[0] > 250 && p[1] > 250, "{p:?}");
    // Two layers: the alphas multiply (tested), the colours multiply or add.
    let l1 = texel(&mut r, [64, 0, 0, 255])?;
    let l2 = texel(&mut r, [255, 64, 0, 128])?;
    let layered = |layers| ParticleDraw {
        texture: Some(l1),
        second_texture: Some(l2),
        alpha: AlphaMode::Mask { cutoff: 0.6 },
        shading: ParticleShading {
            layers: Some(layers),
            ..ParticleShading::STANDARD
        },
        ..centre_draw([1.0; 4])
    };
    let p = centre_pixel_of(&mut r, layered(ParticleLayers::Product), false, None)?;
    assert!(
        near(p[0], 0.5) && near(p[1], 0.5),
        "product alpha 0.5 is discarded: {p:?}"
    );
    let sum = ParticleDraw {
        alpha: AlphaMode::Opaque,
        ..layered(ParticleLayers::Sum)
    };
    let p = centre_pixel_of(&mut r, sum, false, None)?;
    assert!(near(p[0], 1.0) && near(p[1], 0.25), "sum: {p:?}");
    let product = ParticleDraw {
        alpha: AlphaMode::Opaque,
        ..layered(ParticleLayers::Product)
    };
    let p = centre_pixel_of(&mut r, product, false, None)?;
    assert!(near(p[0], 0.25) && p[1] < 5, "product: {p:?}");
    // Fog (lit pass): fogged programs blend toward the fog colour, additive ones do not.
    let fog = Fog {
        near: -10_000.0,
        far: 20.0,
        color: [0.0, 0.0, 1.0],
    };
    let fogged = ParticleShading {
        fogged: true,
        ..ParticleShading::STANDARD
    };
    let p = centre_pixel_of(
        &mut r,
        ParticleDraw {
            shading: fogged,
            ..centre_draw([1.0, 0.0, 0.0, 1.0])
        },
        false,
        Some(fog),
    )?;
    assert!(p[2] > 240 && p[0] < 10, "fogged: {p:?}");
    let p = centre_pixel_of(&mut r, centre_draw([1.0, 0.0, 0.0, 1.0]), false, Some(fog))?;
    assert!(p[0] > 250 && p[2] < 5, "not fogged: {p:?}");
    Ok(())
}

/// `particle-runtime.md` RT-I12: a two-layer program reads its second layer through
/// texture matrix 1 (here a U translation by 0.5) and its first through matrix 0; a
/// one-layer program ignores both matrices.
#[test]
fn two_layer_programs_apply_texture_matrices_0_and_1() -> Result<(), RenderError> {
    let Some(mut r) = gpu::headless_renderer(CLEAR) else {
        return Ok(());
    };
    // Two texels: red at u = 0.25, green at u = 0.75.
    let strip = r.upload_texture(&TextureUpload {
        color_space: ColorSpace::Linear,
        ..TextureUpload::rgba8(2, 1, &[255, 0, 0, 255, 0, 255, 0, 255])
    })?;
    let shift = UvTransform {
        rows: [[1.0, 0.0, 0.5], [0.0, 1.0, 0.0]],
    };
    let at_quarter = |mut d: ParticleDraw| {
        for v in &mut d.vertices {
            v.uv = [0.25, 0.5];
        }
        d
    };
    let two_layer = |uv: [UvTransform; 2]| {
        at_quarter(ParticleDraw {
            second_texture: Some(strip),
            alpha: AlphaMode::Opaque,
            shading: ParticleShading {
                layers: Some(ParticleLayers::Product),
                ..ParticleShading::STANDARD
            },
            uv,
            ..centre_draw([1.0; 4])
        })
    };
    let p = centre_pixel_of(&mut r, two_layer([UvTransform::IDENTITY; 2]), false, None)?;
    assert!(p[0] > 250 && p[1] < 5, "identity: {p:?}");
    let p = centre_pixel_of(
        &mut r,
        two_layer([UvTransform::IDENTITY, shift]),
        false,
        None,
    )?;
    assert!(
        p[1] > 250 && p[0] < 5,
        "second layer at (u + 0.5, v): {p:?}"
    );
    // Matrix 0 moves the first layer (here the strip) and not the second (white).
    let first = at_quarter(ParticleDraw {
        texture: Some(strip),
        alpha: AlphaMode::Opaque,
        shading: ParticleShading {
            layers: Some(ParticleLayers::Product),
            ..ParticleShading::STANDARD
        },
        uv: [shift, UvTransform::IDENTITY],
        ..centre_draw([1.0; 4])
    });
    let p = centre_pixel_of(&mut r, first, false, None)?;
    assert!(p[1] > 250 && p[0] < 5, "first layer at matrix 0: {p:?}");
    // One layer: the matrices are not applied.
    let one = at_quarter(ParticleDraw {
        texture: Some(strip),
        alpha: AlphaMode::Opaque,
        uv: [shift, shift],
        ..centre_draw([1.0; 4])
    });
    let p = centre_pixel_of(&mut r, one, false, None)?;
    assert!(p[0] > 250 && p[1] < 5, "one layer at (u, v): {p:?}");
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

/// A soft white disc, 32 × 32, for the lit sheet.
fn disc() -> Vec<u8> {
    let size = 32u32;
    let mut out = Vec::with_capacity((size * size * 4) as usize);
    for y in 0..size {
        for x in 0..size {
            let u = x as f32 / 31.0 * 2.0 - 1.0;
            let v = y as f32 / 31.0 * 2.0 - 1.0;
            let a = (1.0 - (u * u + v * v).sqrt()).clamp(0.0, 1.0);
            out.extend_from_slice(&[255, 255, 255, (a * 255.0) as u8]);
        }
    }
    out
}

/// A lit system (`use_lighting` = 1): a 4 m square sheet of still, grey quads one metre
/// above the ground.
fn lit_sheet() -> ParticleSystem {
    let key = |time: f32, flags: u32| Keyframe {
        time,
        flags,
        scale_x: 0.22,
        scale_y: 0.22,
        colour: Argb(0xFFB0_B0B0),
        mass: 0.0,
        controller: 2,
        physics_flags: 0,
        ..Keyframe::default()
    };
    let sheet = Emitter {
        shape: 5,
        primitive: 2,
        emit_interval: 0.01,
        burst_count: 2,
        max_particles: 400,
        speed: Curve::constant(0.0),
        direction: CurveVec::constant([0.0, 0.0, 1.0]),
        lifespan: Curve::constant(100.0),
        shape_size: Curve::constant(4.0),
        inclusive: true,
        origin: CurveVec::constant([0.0, 0.0, 1.0]),
        keyframes: vec![key(0.0, 0x770), key(100.0, 0x4)],
        ..Emitter::default()
    };
    ParticleSystem {
        use_lighting: true,
        emitters: vec![sheet],
        ..fountain()
    }
}

/// The point light of the lit scene: warm, 3 m range, Falloff 1 (*k* = 10 / *R²*).
const LIT_LIGHT: Vec3 = Vec3::new(1.2, 0.0, 1.3);
const LIT_COLOUR: [f32; 3] = [1.0, 0.6, 0.2];
const LIT_RANGE: f32 = 3.0;
const LIT_AMBIENT: [f32; 3] = [0.2, 0.2, 0.25];

fn lit_camera() -> Camera {
    Camera {
        eye: Vec3::new(0.0, -5.0, 3.5),
        target: Vec3::new(0.0, 0.0, 0.9),
        ..Camera::default()
    }
}

/// The lit sheet simulated for 2 s at 30 frames per second (seed 77) with the frame
/// ambient and the point light (`particle-runtime.md` §12 rule 13).
fn lit_simulated() -> SystemDraw {
    use tessera_particles::{LightList, ParticleLight};
    let mut sim = SystemSim::new(Arc::new(lit_sheet()), 0.0);
    sim.set_lights(LightList::new([ParticleLight {
        position: LIT_LIGHT,
        colour: LIT_COLOUR,
        range: LIT_RANGE,
        attenuation: 10.0 / (LIT_RANGE * LIT_RANGE),
    }]));
    let env = Environment {
        ambient: [LIT_AMBIENT[0], LIT_AMBIENT[1], LIT_AMBIENT[2], 1.0],
        ..Environment::default()
    };
    let mut rng = Ran2::new(77);
    let cam = lit_camera();
    let view = CameraView::look_at(cam.eye, cam.target);
    let mut last = None;
    for k in 0..=60u64 {
        let now = k as f64 / 30.0;
        sim.place(
            k,
            now,
            Placement::default(),
            &env,
            &mut rng,
            &OriginAttachments,
        );
        last = sim.draw(now, &view, &mut rng);
    }
    last.expect("the sheet is drawn")
}

fn lit_scene(r: &mut Renderer<'static>) -> Result<RgbaImage, RenderError> {
    let texture = r.upload_texture(&TextureUpload {
        color_space: ColorSpace::Srgb,
        filter: Filter::Linear,
        wrap: Wrap::Clamp,
        ..TextureUpload::rgba8(32, 32, &disc())
    })?;
    let ground = r.upload_mesh(&MeshData::plane(12.0, 12.0, 8, 8))?;
    let sys = lit_simulated();
    let target = r.create_offscreen_target(WIDTH, HEIGHT, OffscreenFormat::Rgba8Srgb)?;
    let mut frame = r.begin_offscreen_frame(&target)?;
    let pass = frame.world_pass("world", &lit_camera());
    pass.clear(CLEAR).frame_lighting(FrameLighting {
        ambient: LIT_AMBIENT,
        lights: vec![tessera_render::Light::point(
            LIT_LIGHT,
            LIT_RANGE,
            10.0 / (LIT_RANGE * LIT_RANGE),
            LIT_COLOUR,
        )],
        ..FrameLighting::default()
    });
    pass.mesh(MeshDraw::new(
        ground,
        Mat4::IDENTITY,
        Material::color([0.5, 0.5, 0.5, 1.0]),
    ));
    for d in draws(&sys, Some(texture), AlphaMode::Blend) {
        pass.particles(d);
    }
    let stats = r.end_frame(frame)?;
    assert_eq!(stats.particle_draws, 1, "{stats:?}");
    r.read_pixels(&target)
}

/// `particle-runtime.md` §12 rule 13 (`particles.md` §8.14): a lit system next to a point
/// light: the particles near the light take its warm colour, the far ones fade towards the
/// ambient's with 1 / (1 + k·d²) and no cut-off at the range; the ground is lit by the same
/// light (with the world lights' range cut-off of `lighting.md` §8.3) for comparison.
#[test]
fn particles_lit_golden() -> Result<(), RenderError> {
    // On the CPU: near the light the red channel is the ambient's plus most of the light's.
    let sys = lit_simulated();
    let verts = &sys.instances[0].vertices;
    assert!(
        sys.instances[0].quads() > 100,
        "{}",
        sys.instances[0].quads()
    );
    let red_at = |near: bool| {
        let pick: Vec<f32> = verts
            .iter()
            .filter(|v| {
                let d = (Vec3::from_array(v.position) - LIT_LIGHT).length();
                if near { d < 0.8 } else { d > LIT_RANGE + 0.3 }
            })
            .map(|v| v.colour[0])
            .collect();
        assert!(!pick.is_empty(), "near {near}");
        pick.iter().sum::<f32>() / pick.len() as f32
    };
    let grey = 176.0 / 255.0;
    // Rule 13 has no range cut-off: a particle beyond the light's range still takes
    // colour × 1 / (1 + k·d²) at its centre's distance d (the mean of its quad's corners).
    let k = 10.0 / (LIT_RANGE * LIT_RANGE);
    let quads = verts
        .get(..sys.instances[0].quad_vertices)
        .unwrap_or_default();
    let mut beyond = 0usize;
    for quad in quads.as_chunks::<4>().0 {
        let centre = quad
            .iter()
            .map(|v| Vec3::from_array(v.position))
            .sum::<Vec3>()
            / 4.0;
        let d = (centre - LIT_LIGHT).length();
        if d <= LIT_RANGE + 0.3 {
            continue;
        }
        beyond += 1;
        let expected = (grey * (LIT_AMBIENT[0] + LIT_COLOUR[0] / (1.0 + k * d * d))).min(1.0);
        for v in quad {
            assert!(
                (v.colour[0] - expected).abs() < 1e-3,
                "beyond the range (d = {d}): {} vs {expected}",
                v.colour[0]
            );
            assert!(
                v.colour[0] > grey * LIT_AMBIENT[0] + 0.01,
                "beyond the range (d = {d}) still lit: {}",
                v.colour[0]
            );
        }
    }
    assert!(beyond > 0, "no particle beyond the range");
    // The far vertices are lit less than the near ones.
    assert!(red_at(false) < red_at(true), "{}", red_at(false));
    assert!(
        red_at(true) > grey * (LIT_AMBIENT[0] + 0.3),
        "{}",
        red_at(true)
    );
    let Some(mut r) = gpu::headless_renderer(CLEAR) else {
        return Ok(());
    };
    let img = lit_scene(&mut r)?;
    let again = lit_scene(&mut r)?;
    assert!(img.data == again.data, "render is not deterministic");
    golden::check(
        &Path::new(env!("CARGO_MANIFEST_DIR")).join("tests/golden/particles_lit.png"),
        &img,
        golden::Tolerance::default(),
    );
    Ok(())
}
