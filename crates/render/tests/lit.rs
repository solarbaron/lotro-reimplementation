// SPDX-License-Identifier: GPL-3.0-or-later
//! Lit passes (M4.5) beyond the golden images: hostile frame lighting, skinned lit draws
//! and shadow casters, debug views and overlays inside lit passes, several lit passes with
//! different shadow sizes in one frame, and lit passes next to M2 passes.

#[path = "support/gpu.rs"]
mod gpu;

use tessera_render::glam::{Mat4, Vec3};
use tessera_render::{
    AlphaMode, Camera, Fog, FrameLighting, Gloss, Light, LightKind, LightSelection, LitSurface,
    MAX_FRAME_LIGHTS, Material, MeshData, MeshDebug, MeshDraw, MeshShading, MeshSkin,
    OffscreenFormat, Opacity, Quad, RenderError, Renderer, Specular, SunLight, SunShadow,
    TextureUpload,
};

fn camera() -> Camera {
    Camera {
        eye: Vec3::new(0.0, -6.0, 3.0),
        target: Vec3::ZERO,
        up: Vec3::Z,
        ..Camera::default()
    }
}

fn lighting() -> FrameLighting {
    let toward = Vec3::new(0.3, -0.2, 0.9);
    FrameLighting {
        shadow: Some(SunShadow::fit(toward, Vec3::ZERO, 5.0, 40.0, 512)),
        sun: Some(SunLight {
            direction: toward,
            color: [0.5; 3],
            specular: [0.5; 3],
        }),
        fog: Some(Fog {
            near: 2.0,
            far: 50.0,
            color: [0.5; 3],
        }),
        lights: vec![Light::point(
            Vec3::new(0.0, -1.0, 1.0),
            4.0,
            0.2,
            [1.0, 0.8, 0.5],
        )],
        ..FrameLighting::default()
    }
}

#[test]
fn hostile_frame_lighting_draws_without_errors() -> Result<(), RenderError> {
    let Some(mut r) = gpu::headless_renderer([0.0, 0.0, 0.0, 1.0]) else {
        return Ok(());
    };
    let mesh = r.upload_mesh(&MeshData::uv_sphere(1.0, 16, 8))?;
    let target = r.create_offscreen_target(64, 64, OffscreenFormat::Rgba8Srgb)?;
    let mut lights: Vec<Light> = (0..MAX_FRAME_LIGHTS + 50)
        .map(|i| Light::point(Vec3::new(i as f32 * 0.01, 0.0, 1.0), 2.0, 0.1, [0.01; 3]))
        .collect();
    lights[0] = Light::point(Vec3::NAN, f32::INFINITY, f32::NAN, [f32::NAN; 3]);
    lights[1] = Light {
        kind: LightKind::Directional {
            direction: Vec3::ZERO,
        },
        color: [f32::MAX; 3],
        specular: [-1.0; 3],
    };
    let hostile = FrameLighting {
        up: Vec3::ZERO,
        ambient: [f32::NAN; 3],
        hemispheric: [f32::INFINITY; 3],
        sun: Some(SunLight {
            direction: Vec3::splat(f32::MAX),
            color: [f32::NEG_INFINITY; 3],
            specular: [f32::NAN; 3],
        }),
        fog: Some(Fog {
            near: f32::NAN,
            far: -5.0,
            color: [f32::NAN; 3],
        }),
        shadow: Some(SunShadow {
            view_proj: Mat4::from_cols_array(&[f32::NAN; 16]),
            map_size: u32::MAX,
            quality: u32::MAX,
            fade_start: f32::NAN,
            fade_scale: f32::NEG_INFINITY,
            depth_bias: f32::NAN,
            ..SunShadow::fit(Vec3::Z, Vec3::ZERO, 1.0, 1.0, 64)
        }),
        lights,
        time: f32::NAN,
    };
    // A destroyed texture: lit textures that do not resolve fall back to white.
    let stale = r.upload_texture(&TextureUpload::rgba8(1, 1, &[255; 4]))?;
    r.destroy_texture(stale);
    let mut frame = r.begin_offscreen_frame(&target)?;
    frame
        .world_pass("hostile", &camera())
        .frame_lighting(hostile)
        .mesh(
            MeshDraw::new(
                mesh,
                Mat4::IDENTITY,
                Material {
                    specular: Some(Specular {
                        color: [f32::NAN; 3],
                        shininess: f32::INFINITY,
                        texture: None,
                    }),
                    alpha: AlphaMode::TestBlend { cutoff: f32::NAN },
                    ..Material::color([f32::NAN; 4])
                },
            )
            .with_lit(LitSurface {
                glow: [f32::INFINITY; 3],
                lights: LightSelection::List {
                    indices: [255; 8],
                    count: u8::MAX,
                },
                emissive: Some(stale),
                gloss: Gloss::Texture(stale),
                opacity: Opacity::Texture(stale),
                ..LitSurface::default()
            }),
        );
    let stats = r.end_frame(frame)?;
    assert_eq!(stats.lit_draws, 1);
    Ok(())
}

#[test]
fn skinned_debug_and_mixed_passes() -> Result<(), RenderError> {
    let Some(mut r) = gpu::headless_renderer([0.0, 0.0, 0.0, 1.0]) else {
        return Ok(());
    };
    let ground = r.upload_mesh(&MeshData::plane(10.0, 10.0, 1, 1))?;
    let data = MeshData::tube(0.3, 2.0, 12);
    let skin = MeshSkin::rigid(data.positions.len(), 1, 2);
    let limb = r.upload_skinned_mesh(&data, &skin)?;
    let target = r.create_offscreen_target(96, 64, OffscreenFormat::Rgba8Srgb)?;
    let mut frame = r.begin_offscreen_frame(&target)?;
    let palette = frame.skin_palette(&[Mat4::IDENTITY, Mat4::from_rotation_x(0.6)])?;
    let lit = LitSurface {
        gloss: Gloss::BaseAlpha,
        opacity: Opacity::One,
        ..LitSurface::default()
    };
    // Pass 1: lit, with a skinned caster, an alpha-tested caster, quads, lines and a
    // wireframe overlay (group 0 switches between the frame group and the globals).
    {
        let pass = frame
            .world_pass("lit-1", &camera())
            .clear([0.1, 0.1, 0.1, 1.0])
            .frame_lighting(lighting())
            .mesh_debug(MeshDebug {
                wireframe: Some([1.0, 1.0, 1.0, 1.0]),
                ..MeshDebug::default()
            });
        pass.mesh(MeshDraw::new(ground, Mat4::IDENTITY, Material::default()).with_lit(lit))
            .mesh(
                MeshDraw::new(
                    limb,
                    Mat4::from_translation(Vec3::Z),
                    Material::color([0.8, 0.5, 0.3, 1.0]),
                )
                .with_skin(palette)
                .with_lit(lit),
            )
            .mesh(
                MeshDraw::new(
                    limb,
                    Mat4::from_translation(Vec3::new(1.0, 0.0, 1.0)),
                    Material {
                        alpha: AlphaMode::Mask { cutoff: 0.5 },
                        ..Material::default()
                    },
                )
                .with_lit(lit),
            )
            .quad(Quad::new(Mat4::from_translation(Vec3::new(-2.0, 0.0, 1.0))))
            .line(Vec3::ZERO, Vec3::Z, [1.0, 0.0, 0.0, 1.0]);
    }
    // Pass 2: an M2 pass in between.
    frame
        .world_pass("m2", &camera())
        .clear_depth()
        .mesh(MeshDraw::new(limb, Mat4::IDENTITY, Material::default()).with_skin(palette));
    // Pass 3: lit again with a larger shadow map and a debug shading (M2 pipeline).
    {
        let mut l = lighting();
        if let Some(s) = l.shadow.as_mut() {
            s.map_size = 2048;
        }
        frame
            .world_pass("lit-2", &camera())
            .clear_depth()
            .frame_lighting(l)
            .mesh(MeshDraw::new(ground, Mat4::IDENTITY, Material::default()).with_lit(lit))
            .mesh(
                MeshDraw::new(limb, Mat4::IDENTITY, Material::default())
                    .with_skin(palette)
                    .with_lit(lit),
            );
    }
    {
        let pass = frame
            .world_pass("lit-debug", &camera())
            .clear_depth()
            .frame_lighting(lighting())
            .mesh_debug(MeshDebug {
                shading: MeshShading::Normals,
                ..MeshDebug::default()
            });
        pass.mesh(MeshDraw::new(ground, Mat4::IDENTITY, Material::default()));
    }
    let stats = r.end_frame(frame)?;
    // 3 lit draws in pass 1 + 2 in pass 3; the debug pass draws through M2.
    assert_eq!(stats.lit_draws, 5);
    // The debug pass samples no shadow map, so it gets no shadow pass.
    assert_eq!(stats.shadow_passes, 2);
    assert_eq!(stats.shadow_map_size, 2048);
    // Casters: the ground and both limbs in pass 1, ground and limb in pass 3, the
    // ground in the debug pass is not lit-shaded so casts nothing.
    assert_eq!(stats.shadow_draws, 5);
    assert!(stats.skinned_draws >= 3);

    // A second frame with only a smaller shadow request reallocates cleanly.
    let mut frame = r.begin_offscreen_frame(&target)?;
    let mut l = lighting();
    if let Some(s) = l.shadow.as_mut() {
        s.map_size = 64;
    }
    frame
        .world_pass("small", &camera())
        .frame_lighting(l)
        .mesh(MeshDraw::new(ground, Mat4::IDENTITY, Material::default()));
    let stats = r.end_frame(frame)?;
    assert_eq!((stats.lit_draws, stats.shadow_passes), (1, 1));
    assert_eq!(stats.shadow_map_size, 64);
    Ok(())
}

#[test]
fn idle_shadow_map_is_freed() -> Result<(), RenderError> {
    let Some(mut r) = gpu::headless_renderer([0.0, 0.0, 0.0, 1.0]) else {
        return Ok(());
    };
    let ground = r.upload_mesh(&MeshData::plane(10.0, 10.0, 1, 1))?;
    let target = r.create_offscreen_target(16, 16, OffscreenFormat::Rgba8Unorm)?;
    let mut l = lighting();
    if let Some(s) = l.shadow.as_mut() {
        s.map_size = 4096;
    }
    let big = r
        .context()
        .device
        .limits()
        .max_texture_dimension_2d
        .min(4096);
    let mut frame = r.begin_offscreen_frame(&target)?;
    frame
        .world_pass("lit", &camera())
        .frame_lighting(l.clone())
        .mesh(MeshDraw::new(ground, Mat4::IDENTITY, Material::default()));
    assert_eq!(r.end_frame(frame)?.shadow_map_size, big);
    // Frames without a shadow request keep the map for a while, then free it.
    let idle = Renderer::SHADOW_IDLE_FRAMES;
    for n in 1..=idle {
        let mut frame = r.begin_offscreen_frame(&target)?;
        frame
            .world_pass("unlit", &camera())
            .frame_lighting(FrameLighting {
                shadow: None,
                ..l.clone()
            })
            .mesh(MeshDraw::new(ground, Mat4::IDENTITY, Material::default()));
        let stats = r.end_frame(frame)?;
        assert_eq!(stats.shadow_passes, 0);
        let expected = if n < idle { big } else { 1 };
        assert_eq!(stats.shadow_map_size, expected, "after {n} idle frames");
    }
    // A debug-shaded lit pass does not ask for a map.
    let mut frame = r.begin_offscreen_frame(&target)?;
    frame
        .world_pass("debug", &camera())
        .frame_lighting(l.clone())
        .mesh_debug(MeshDebug {
            shading: MeshShading::UvChecker,
            ..MeshDebug::default()
        })
        .mesh(MeshDraw::new(ground, Mat4::IDENTITY, Material::default()));
    let stats = r.end_frame(frame)?;
    assert_eq!((stats.shadow_passes, stats.shadow_map_size), (0, 1));
    // Asking again allocates again.
    let mut frame = r.begin_offscreen_frame(&target)?;
    frame
        .world_pass("lit", &camera())
        .frame_lighting(l)
        .mesh(MeshDraw::new(ground, Mat4::IDENTITY, Material::default()));
    let stats = r.end_frame(frame)?;
    assert_eq!((stats.shadow_passes, stats.shadow_map_size), (1, big));
    Ok(())
}
